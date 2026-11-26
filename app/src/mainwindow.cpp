#include <QWidgetAction>
#include <tuple>
#include "fileoptions.h"
#include "mainwindow.h"
#include <QDoubleSpinBox>
#include <QInputDialog>
#include <QSignalBlocker>
#include <QFormLayout>
#include <QCheckBox>
#include <QDialogButtonBox>
#include <QShortcut>
#include <QRandomGenerator>
#include <QColorDialog>
#include <QHeaderView>
#include <QTableWidget>
#include "updates.h"
#include "renderui.h"
#include "aztek.h"
#include "feedback.h"
#include "host.h"
#include "usdui.h"
#include "icons.h"
#include "markingmenu.h"
#include "panels.h"
#include "scriptui.h"
#include "viewport.h"
#include "widgets.h"
#include "grapheditor.h"
#include "uveditor.h"
#include "toolsettings.h"
#include "dopesheet.h"
#include "nodeeditor.h"
#include "shelves.h"
#include "editors.h"
#include "themes.h"

#include <QAction>
#include <cstdio>
#include <QActionGroup>
#include <QApplication>
#include <QFileInfo>
#include <QCursor>
#include <QCloseEvent>
#include <QComboBox>
#include <QDateTime>
#include <QDockWidget>
#include <QGuiApplication>
#include <QChildEvent>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFileDialog>
#include <QMimeData>
#include <QDrag>
#include <QTextEdit>
#include <QUrl>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonDocument>
#include <QKeyEvent>
#include <QIcon>
#include <QImage>
#include <QPolygonF>
#include <QLabel>
#include <QMouseEvent>
#include <QLineEdit>
#include <QMenuBar>
#include <QMessageBox>
#include <QPointer>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QPainter>
#include <QFile>
#include <QDialog>
#include <QResizeEvent>
#include <QSettings>
#include <QSpinBox>
#include <QSplitter>
#include <QStatusBar>
#include <QTabWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QVBoxLayout>

namespace az {

namespace {

QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}

bool has_command(const QString& name) {
    static QSet<QString> names;
    if (names.isEmpty())
        for (const QJsonValue& c : query("commands").toArray()) names.insert(c.toObject()["name"].toString());
    return names.contains(name);
}

bool outliner_key(const QKeyEvent* k) {
    const bool plain = !(k->modifiers() & (Qt::ControlModifier | Qt::AltModifier));
    switch (k->key()) {
    case Qt::Key_F: return plain && !(k->modifiers() & Qt::ShiftModifier);
    case Qt::Key_Up: case Qt::Key_Down: case Qt::Key_Left: case Qt::Key_Right:
    case Qt::Key_Home: case Qt::Key_End: case Qt::Key_PageUp: case Qt::Key_PageDown:
    case Qt::Key_Return: case Qt::Key_Enter: case Qt::Key_F2:
        return true;
    default: return false;
    }
}

bool typing_widget(QWidget* w) {
    return qobject_cast<QLineEdit*>(w) || qobject_cast<QPlainTextEdit*>(w) || qobject_cast<QAbstractSpinBox*>(w) || qobject_cast<QComboBox*>(w);
}

} // namespace

MainWindow::MainWindow() {
    setAcceptDrops(true);
    setWindowTitle("Aztek");
    // The name $gMainWindow holds, as the reference's main window has: tools find the window by it through Qt alone,
    // with QApplication.topLevelWidgets(), without shiboken (#142).
    setObjectName("MayaWindow");
    setDockOptions(QMainWindow::AnimatedDocks | QMainWindow::AllowTabbedDocks | QMainWindow::AllowNestedDocks);
    // The bottom right goes to the Time and Range Sliders, which run the window's width as the reference's do.
    setCorner(Qt::BottomRightCorner, Qt::BottomDockWidgetArea);
    setCorner(Qt::TopRightCorner, Qt::RightDockWidgetArea);

    panels_ = new PanelLayout(this);
    connect(panels_, &PanelLayout::rightClick, this, &MainWindow::marking_menu);

    build_runtime();
    ScriptUi::get()->set_main(this, [this](const QString& panel) { return view_for_panel(panel); }, [this] {
        for (const QJsonValue& r : query("uiRequests").toArray()) handle_ui_request(r.toString());
    });
    load_hotkeys();
    build_status_line();
    addToolBarBreak();
    QToolBar* shelf_holder = addToolBar("Shelf");
    shelf_holder->setObjectName("shelf");
    shelf_holder->setMovable(false);
    shelf_bar_ = new ShelfBar([this](const QString& code, const QString& lang) {
        run_repeatable(code, [this, code, lang] {
            if (lang == "python") python(code);
            else if (lang == "menu") run_menu_path(code);
            else mel_action(code);
        });
    }, shelf_holder);
    MarkingMenu::around() = [this](const QString& label, const std::function<void()>& fn) {
        if (label.startsWith("Repeat") || label == "Undo" || label == "Redo") fn();
        else run_repeatable(label, fn);
    };
    shelf_capture() = [this](QAction* a) { add_to_shelf(a); };
    shelf_holder->addWidget(shelf_bar_);
    shelf_bar_->set_corner(make_feedback_button(shelf_bar_));
    // Scripts reach the views' pane as $gMainPane and the shelves as $gShelfTopLevel, by the reference's names.
    ScriptUi::get()->add_builtin_editor("MainPane", "paneLayout", panels_);
    shelf_bar_->fire_script = [](const QString& path) {
        QByteArray text = QJsonDocument(QJsonObject{{"path", path}, {"fire", QJsonArray{"command"}}}).toJson(QJsonDocument::Compact);
        aztek_ui_event(text.constData());
        Host::get()->poke();
        ScriptUi::get()->sync();
    };
    auto shelves_for_scripts = [this] {
        ScriptUi::ShelfHooks hooks;
        hooks.add = [this](const QString& shelf, const QString& path, const QJsonObject& props) { shelf_bar_->add_script_button(shelf, path, props); };
        hooks.remove = [this](const QString& path) { shelf_bar_->remove_script_button(path); };
        hooks.select = [this](const QString& shelf) { shelf_bar_->select_shelf(shelf); };
        ScriptUi::get()->set_shelves(shelf_bar_, shelf_bar_->shelf_names(), shelf_bar_->current(), hooks);
    };
    connect(shelf_bar_, &ShelfBar::shelves_changed, this, shelves_for_scripts);
    shelves_for_scripts();
    watch_menus();
    build_toolbox();
    build_docks();
    build_bottom();
    build_menus();

    // The centre: model panels, between two thin handles that fold away everything docked on their side at a click
    // and bring it back at the next (#97): the toolbox and left docks, the right docks.
    {
        auto* centre = new QWidget(this);
        auto* row = new QHBoxLayout(centre);
        row->setContentsMargins(0, 0, 0, 0);
        row->setSpacing(0);
        auto handle = [this, centre](bool left) {
            auto* b = new QToolButton(centre);
            b->setObjectName(left ? "collapseLeft" : "collapseRight");
            b->setAutoRaise(true);
            b->setFixedWidth(9);
            b->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Expanding);
            b->setArrowType(left ? Qt::LeftArrow : Qt::RightArrow);
            b->setToolTip(left ? "Collapse the toolbox and the left panels" : "Collapse the right panels");
            connect(b, &QToolButton::clicked, this, [this, b, left] { toggle_side(left, b); });
            return b;
        };
        row->addWidget(handle(true));
        row->addWidget(panels_, 1);
        row->addWidget(handle(false));
        setCentralWidget(centre);
    }
    in_view_ = new InViewEditor(panels_);

    connect(Host::get(), &Host::sceneChanged, this, &MainWindow::refresh_state);
    connect(Host::get(), &Host::sceneChanged, in_view_, &InViewEditor::update_values);
    connect(Host::get(), &Host::sceneChanged, this, &MainWindow::update_title);
    connect(Host::get(), &Host::logLines, this, [this](const QJsonArray& lines) {
        // The history is kept from the start, as the reference's is, for the Script Editor whenever it opens.
        if (script_editor_) script_editor_->append(lines);
        else {
            for (const QJsonValue& l : lines) log_before_editor_.append(l);
            while (log_before_editor_.size() > 20000) log_before_editor_.removeFirst();
        }
        // The command line's result field shows the latest result or error.
        for (const QJsonValue& v : lines) {
            QJsonObject l = v.toObject();
            int k = l["kind"].toInt();
            // Printed output too, as the reference's field shows the Script Editor's last line: a shelf script that
            // prints says so there (#117).
            if (k == 1 || k == 2 || k == 4) command_line_->show_result(l["text"].toString(), k == 2);
            else if (k == 3 && !l["text"].toString().trimmed().isEmpty()) command_line_->show_result(l["text"].toString().trimmed().section('\n', -1), false);
            if (k == 0) {
                remember_command(l["text"].toString());
                QString t = l["text"].toString();
                if (t.startsWith("select") && tool_ != "showManip") in_view_->hide();
            }
        }
    });

    qApp->installEventFilter(this);
    apply_prefs();
    resize(1600, 980);
    refresh_state();
    update_title();
}

// ---------------------------------------------------------------------------------------------------------------------
// Menus

// A divider with a title, as the menus have: Qt's own section text does not show under this style.
static void menu_title(QMenu* m, const QString& title) {
    m->addSeparator();
    auto* label = new QLabel("  " + title, m);
    label->setObjectName("menuSection");
    auto* a = new QWidgetAction(m);
    a->setDefaultWidget(label);
    m->addAction(a);
}

QAction* MainWindow::cmd(QMenu* m, const QString& label, const QString& command, const QJsonObject& call, const QString& shortcut, bool options) {
    bool ok = has_command(command);
    auto run = [this, command, call, options, label] {
        // Interactive Creation: the primitive is drawn in the view rather than made at the origin.
        static const QHash<QString, QString> drawn{{"polyCube", "createCube"}, {"polyPlane", "createPlane"}, {"polySphere", "createSphere"}, {"polyCylinder", "createCylinder"}, {"polyCone", "createCone"}, {"polyTorus", "createTorus"}};
        if (drawn.contains(command) && QSettings().value("prefs/interactiveCreation", false).toBool()) {
            set_tool(drawn[command]);
            return;
        }
        QJsonObject c = call;
        // An option box's remembered settings apply to the plain item too, as in the reference.
        if (options) {
            QJsonObject saved = OptionsDialog::saved(command);
            for (auto it = saved.begin(); it != saved.end(); ++it)
                if (!c.contains(it.key())) c[it.key()] = it.value();
        }
        run_repeatable(label, [command, c] { exec(command, c); });
    };
    QAction* a;
    if (options && ok) {
        a = new OptionRow(label, shortcut, run, [command, label, call] { OptionsDialog::open(command, label, call); }, m);
        m->addAction(a);
    } else {
        a = m->addAction(label);
        if (!shortcut.isEmpty()) a->setText(label + "\t" + shortcut);
        connect(a, &QAction::triggered, this, run);
    }
    a->setEnabled(ok);
    // What Ctrl+Shift+click puts on the shelf.
    QJsonObject echo{{"name", command}, {"call", call}};
    a->setProperty("melCommand", query("melEcho", QString::fromUtf8(QJsonDocument(echo).toJson(QJsonDocument::Compact))).toString());
    a->setStatusTip(ok ? QString("%1: %2").arg(label, command) : QString("%1: not available yet").arg(label));
    return a;
}

QAction* MainWindow::todo(QMenu* m, const QString& label) {
    QAction* a = m->addAction(label);
    a->setEnabled(false);
    a->setStatusTip(label + ": not available yet");
    return a;
}

QMenu* MainWindow::create_menu() {
    auto* m = new QMenu("Create", this);
    usd_create_menu(m, this);
    m->addSeparator();
    QMenu* polys = m->addMenu("Polygon Primitives");
    struct P { const char* label; const char* cmd; };
    for (P p : {P{"Sphere", "polySphere"}, P{"Cube", "polyCube"}, P{"Cylinder", "polyCylinder"}, P{"Cone", "polyCone"}, P{"Torus", "polyTorus"},
                P{"Plane", "polyPlane"}, P{"Disc", "polyDisc"}, P{"Pipe", "polyPipe"}, P{"Capsule", "polyCapsule"}, P{"Platonic Solid", "polyIcoSphere"},
                P{"Quad Sphere", "polyQuadSphere"}, P{"Pyramid", "polyPyramid"}, P{"Prism", "polyPrism"}, P{"Helix", "polyHelix"}, P{"Soccer Ball", "polySoccerBall"}})
        cmd(polys, p.label, p.cmd, {}, QString(), true);
    polys->addSeparator();
    // With this on, the primitive menu items start a drag in the view instead of making one at the origin.
    auto* interactive = polys->addAction("Interactive Creation");
    interactive->setCheckable(true);
    interactive->setChecked(QSettings().value("prefs/interactiveCreation", false).toBool());
    connect(interactive, &QAction::toggled, this, [](bool on) { QSettings().setValue("prefs/interactiveCreation", on); });
    cmd(m, "Text", "textCurves", QJsonObject{{"text", "Text"}});
    QMenu* nurbs = m->addMenu("NURBS Primitives");
    const QJsonArray up{0, 1, 0};
    cmd(nurbs, "Sphere", "sphere", QJsonObject{{"axis", up}}, QString(), true);
    cmd(nurbs, "Cube", "nurbsCube", QJsonObject{{"axis", up}}, QString(), true);
    cmd(nurbs, "Cylinder", "cylinder", QJsonObject{{"axis", up}}, QString(), true);
    cmd(nurbs, "Cone", "cone", QJsonObject{{"axis", up}}, QString(), true);
    cmd(nurbs, "Plane", "nurbsPlane", QJsonObject{{"axis", up}}, QString(), true);
    cmd(nurbs, "Torus", "torus", QJsonObject{{"axis", up}}, QString(), true);
    nurbs->addSeparator();
    cmd(nurbs, "Circle", "circle", QJsonObject{{"normal", up}}, QString(), true);
    cmd(nurbs, "Square", "nurbsSquare", {}, QString(), true);
    QMenu* lights = m->addMenu("Lights");
    cmd(lights, "Ambient Light", "CreateAmbientLight", {}, QString(), true);
    cmd(lights, "Directional Light", "CreateDirectionalLight", {}, QString(), true);
    cmd(lights, "Point Light", "CreatePointLight", {}, QString(), true);
    cmd(lights, "Spot Light", "CreateSpotLight", {}, QString(), true);
    cmd(lights, "Area Light", "CreateAreaLight", {}, QString(), true);
    cmd(lights, "Volume Light", "CreateVolumeLight", {}, QString(), true);
    harold_create_menu(m, this);
    QMenu* curves = m->addMenu("Curve Tools");
    curves->addAction("CV Curve Tool", [this] { set_tool("curveCv"); });
    curves->addAction("EP Curve Tool", [this] { set_tool("curveEp"); });
    curves->addAction("Bezier Curve Tool", [this] { set_tool("curveBezier"); });
    QMenu* arcs = curves->addMenu("Arc Tools");
    arcs->addAction("Two Point Circular Arc", [this] { set_tool("arcTwo"); });
    arcs->addAction("Three Point Circular Arc", [this] { set_tool("arcThree"); });
    m->addSeparator();
    cmd(m, "Camera", "camera");
    cmd(m, "Locator", "spaceLocator");
    m->addAction("Free Image Plane", [this] {
        QString f = QFileDialog::getOpenFileName(this, "Free Image Plane", QString(), "Images (*.png *.jpg *.jpeg *.tif *.tiff *.exr *.tga *.bmp);;All files (*)");
        exec("imagePlane", f.isEmpty() ? QJsonObject() : QJsonObject{{"fileName", f}});
    })->setEnabled(has_command("imagePlane"));
    cmd(m, "Annotation", "annotate", QJsonObject{{"text", "Annotation"}});
    QMenu* measure = m->addMenu("Measure Tools");
    cmd(measure, "Distance Tool", "distanceDimension", QJsonObject{{"startPoint", QJsonArray{0, 0, 0}}, {"endPoint", QJsonArray{1, 0, 0}}});
    cmd(measure, "Parameter Tool", "paramDimension");
    cmd(m, "Empty Group", "group", QJsonObject{{"empty", true}}, "Ctrl+G");
    cmd(m, "Sets", "createSet", {}, QString(), true);
    return m;
}

QMenu* MainWindow::mesh_menu() {
    auto* m = new QMenu("Mesh", this);
    menu_title(m, "Combine");
    QMenu* b = m->addMenu("Booleans");
    // A live polyCBoolOp node, as the reference's Booleans make.
    cmd(b, "Union", "polyCBoolOp", QJsonObject{{"operation", 1}});
    cmd(b, "Difference (A - B)", "polyCBoolOp", QJsonObject{{"operation", 2}});
    cmd(b, "Difference (B - A)", "polyCBoolOp", QJsonObject{{"operation", 4}});
    cmd(b, "Intersection", "polyCBoolOp", QJsonObject{{"operation", 3}});
    b->addSeparator();
    cmd(b, "Slice", "polyCBoolOp", QJsonObject{{"operation", 5}});
    cmd(b, "Hole Punch", "polyCBoolOp", QJsonObject{{"operation", 7}});
    cmd(b, "Cut Out", "polyCBoolOp", QJsonObject{{"operation", 8}});
    cmd(b, "Split Edges", "polyCBoolOp", QJsonObject{{"operation", 6}});
    cmd(m, "Combine", "polyUnite", {}, QString(), true);
    cmd(m, "Separate", "polySeparate");
    m->addSeparator();
    // Conform as the reference's, onto the live surface (#165); Aztek's own conform to another object beside it.
    cmd(m, "Conform", "polyConformToLive", {}, QString(), true);
    cmd(m, "Conform to Object", "polyConformToObject", {}, QString(), true);
    cmd(m, "Fill Hole", "polyFillHole", {}, QString(), true);
    cmd(m, "Reduce", "polyReduce", {}, QString(), true);
    cmd(m, "Remesh", "polyRemesh", {}, QString(), true);
    cmd(m, "Retopologize", "polyRetopo", {}, QString(), true);
    cmd(m, "Smooth", "polySmooth", {}, QString(), true);
    cmd(m, "Unsmooth", "polyUnsmooth", {}, QString(), true);
    cmd(m, "Average Vertices", "polyAverageVertex", {}, QString(), true);
    cmd(m, "Triangulate", "polyTriangulate", {}, QString(), true);
    cmd(m, "Quadrangulate", "polyQuadrangulate", {}, QString(), true);
    m->addSeparator();
    menu_title(m, "Mirror");
    cmd(m, "Mirror", "polyMirrorFace", {}, QString(), true);
    cmd(m, "Symmetrize", "symmetrize", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Transfer Attributes", "transferAttributes", {}, QString(), true);
    cmd(m, "Cleanup...", "polyCleanup", QJsonObject{{"selectOnly", true}, {"nsided", true}, {"nonManifold", true}, {"lamina", true}, {"zeroGeometry", true}}, QString(), true);
    cmd(m, "Mesh Check", "meshCheck", {}, QString(), true);
    QMenu* proxy = m->addMenu("Smooth Proxy");
    proxy->addAction("Subdiv Proxy", [] {
        // A smoothed copy that follows the original's edits.
        QJsonValue made = exec("polyDuplicateAndConnect");
        QString copy = made.isArray() ? made.toArray().at(0).toString() : made.toString();
        if (!copy.isEmpty()) exec("polySmooth", QJsonObject{{"args", QJsonArray{copy}}, {"divisions", 2}});
    });
    return m;
}

QMenu* MainWindow::edit_mesh_menu() {
    auto* m = new QMenu("Edit Mesh", this);
    menu_title(m, "Components");
    cmd(m, "Bevel", "polyBevel", {}, "Ctrl+B", true);
    cmd(m, "Add Divisions", "polySubdivideFacet", {}, QString(), true);
    cmd(m, "Bridge", "polyBridge", {}, QString(), true);
    cmd(m, "Chamfer Vertices", "polyChamferVtx", {}, QString(), true);
    cmd(m, "Collapse", "polyCollapse");
    cmd(m, "Connect", "polyConnectComponents", {}, QString(), true);
    cmd(m, "Detach", "polyDetachEdges");
    cmd(m, "Extrude", "polyExtrude", {}, "Ctrl+E", true);
    cmd(m, "Inset", "polyInset", {}, QString(), true);
    cmd(m, "Merge", "polyMerge", {}, QString(), true);
    cmd(m, "Merge to Center", "polyMergeCenter");
    // Transform: the Move Component command for what is selected, with its handles straight away.
    m->addAction("Transform", [this] {
        QString mode = state()["mode"].toString();
        const char* c = mode == "face" ? "polyMoveFacet" : mode == "edge" ? "polyMoveEdge" : "polyMoveVertex";
        exec(c);
    });
    cmd(m, "Flip", "polyFlip");
    cmd(m, "Spherize", "polySpherize", {}, QString(), true);
    cmd(m, "Circularize", "polyCircularize", {}, QString(), true);
    cmd(m, "Straighten", "polyStraighten", {}, QString(), true);
    menu_title(m, "Vertex");
    cmd(m, "Reorder Vertices", "meshReorder");
    menu_title(m, "Edge");
    m->addAction("Delete Edge/Vertex\tCtrl+Delete", [this] { run_repeatable("Delete Edge/Vertex", [] { delete_edge_vertex(); }); });
    cmd(m, "Edit Edge Flow", "polyEditEdgeFlow", {}, QString(), true);
    cmd(m, "Flip Triangle Edge", "polyFlipEdge");
    cmd(m, "Spin Edge Backward", "polySpinEdge", QJsonObject{{"offset", -1}});
    cmd(m, "Spin Edge Forward", "polySpinEdge", QJsonObject{{"offset", 1}});
    menu_title(m, "Face");
    cmd(m, "Poke", "polyPoke", {}, QString(), true);
    cmd(m, "Wedge", "polyWedgeFace", {}, QString(), true);
    cmd(m, "Assign Invisible Faces", "polyHole", {}, QString(), true);
    cmd(m, "Duplicate", "polyChipOff", QJsonObject{{"duplicate", true}}, QString(), true);
    cmd(m, "Extract", "polyExtract", {}, QString(), true);
    menu_title(m, "Curve");
    cmd(m, "Project Curve on Mesh", "polyProjectCurve", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Grid Fill", "polyGridFill", {}, QString(), true);
    cmd(m, "Split Rings", "polySplitRings", {}, QString(), true);
    return m;
}

QMenu* MainWindow::mesh_tools_menu() {
    auto* m = new QMenu("Mesh Tools", this);
    auto tool = [this, m](const QString& label, const QString& id) { m->addAction(label, [this, id] { set_tool(id); }); };
    tool("Append to Polygon", "appendPolygon");
    cmd(m, "Connect", "polyConnectComponents");
    tool("Crease", "crease");
    tool("Create Polygon", "createPolygon");
    tool("Insert Edge Loop", "insertLoop");
    tool("Multi-Cut", "multiCut");
    tool("Offset Edge Loop", "offsetLoop");
    tool("Quad Draw", "quadDraw");
    tool("Slide Edge", "slideEdge");
    tool("Target Weld", "targetWeld");
    QMenu* sculpt = m->addMenu("Sculpting Tools");
    for (const char* b : {"sculpt", "smooth", "relax", "grab", "pinch", "flatten", "foamy", "spray", "repeat", "imprint", "wax", "scrape", "fill", "knife", "smear", "bulge", "amplify", "freeze"}) {
        QString brush = b;
        QString label = brush.left(1).toUpper() + brush.mid(1) + " Tool";
        sculpt->addAction(label, [this, brush] {
            exec_quiet("sculptMeshCacheCtx", QJsonObject{{"edit", true}, {"sculptTool", brush}});
            set_tool("sculpt");
        });
    }
    m->addSeparator();
    m->addAction("Make Live", [this] { exec("makeLive"); sync_status_line(); });
    m->addAction("Make Not Live", [this] { exec("makeLive", QJsonObject{{"none", true}}); sync_status_line(); });
    m->addSeparator();
    cmd(m, "Symmetry: Toggle", "toggleSymmetry");
    cmd(m, "Symmetry: World X", "setSymmetry", QJsonObject{{"axis", "x"}});
    return m;
}

QMenu* MainWindow::mesh_display_menu() {
    auto* m = new QMenu("Mesh Display", this);
    // Conform makes normals agree (polyNormal -nm 2); Reverse flips them (polyNormal -nm 0).
    cmd(m, "Conform", "polyNormal", QJsonObject{{"normalMode", 2}});
    cmd(m, "Reverse", "polyNormal", QJsonObject{{"normalMode", 0}});
    m->addSeparator();
    cmd(m, "Soften Edge", "polySoftEdge", QJsonObject{{"angle", 180.0}});
    cmd(m, "Harden Edge", "polySoftEdge", QJsonObject{{"angle", 0.0}});
    cmd(m, "Soften/Harden Edge", "polySoftEdge", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Average", "polyAverageNormal", {}, QString(), true);
    cmd(m, "Set to Face", "polySetToFaceNormal");
    cmd(m, "Lock Normals", "polyLockNormals");
    cmd(m, "Unlock Normals", "polyUnlockNormals");
    cmd(m, "Face Weighted Normals", "polyWeightedNormals", {}, QString(), true);
    return m;
}

QMenu* MainWindow::curves_menu() {
    auto* m = new QMenu("Curves", this);
    menu_title(m, "Edit");
    cmd(m, "Attach", "attachCurve", {}, QString(), true);
    cmd(m, "Detach", "detachCurve", {}, QString(), true);
    cmd(m, "Open/Close", "closeCurve", {}, QString(), true);
    cmd(m, "Fillet", "filletCurve", {}, QString(), true);
    cmd(m, "Intersect", "curveIntersect", {}, QString(), true);
    m->addAction("Add Points Tool", [this] { set_tool("curveAddPt"); });
    m->addSeparator();
    cmd(m, "Insert Knot", "insertKnotCurve", {}, QString(), true);
    cmd(m, "Extend", "extendCurve", {}, QString(), true);
    cmd(m, "Offset", "offsetCurve", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Reverse Direction", "reverseCurve");
    cmd(m, "Rebuild", "rebuildCurve", {}, QString(), true);
    cmd(m, "Fit B-spline", "fitBspline");
    cmd(m, "Smooth", "smoothCurve", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Polygon Edges to Curve", "polyToCurve", {}, QString(), true);
    return m;
}

// XJEN: hair and fur, as the reference's Generate menu. Items come with the xjenToolkit plug-in.
QMenu* MainWindow::generate_menu() {
    auto* m = new QMenu("Generate", this);
    menu_title(m, "Interactive Groom");
    m->addAction("Interactive Groom Editor", [this] { show_named_window("interactiveGroomEditor"); })->setEnabled(has_command("xjenGroomInfo"));
    cmd(m, "Create Interactive Groom Splines", "xjenCreateGroom", {}, QString(), true);
    QMenu* tools = m->addMenu("Interactive Groom Tools");
    for (const char* t : {"Density", "Place", "Length", "Cut", "Width", "Twist", "Comb", "Grab", "Smooth", "Noise", "Clump", "Part", "Freeze", "Select"}) {
        QString id = QString(t).toLower();
        QAction* a = tools->addAction(QString(t), [this, id] {
            exec("xjenGroomCtx", QJsonObject{{"tool", id}});
            set_tool("groom");
            if (tool_settings_) tool_settings_->set_tool(tool_);
        });
        a->setEnabled(has_command("xjenGroomCtx"));
    }
    QMenu* mods = m->addMenu("Add Modifier");
    for (auto [label, id] : std::initializer_list<std::pair<const char*, const char*>>{{"Clump", "clump"}, {"Noise", "noise"}, {"Cut", "cut"}, {"Collide", "collide"}, {"Displace", "displace"}, {"Guide", "guide"}, {"Linear Wire", "linearWire"}, {"Curve to Spline", "curveToSpline"}, {"Scale", "scale"}, {"Sculpt", "sculpt"}}) {
        QString t = id;
        QAction* a = mods->addAction(label, [t] {
            QJsonObject c{{"type", t}};
            // Guide, Linear Wire and Curve to Spline take the curves selected; Collide the meshes.
            QJsonArray picked = ask("ls", QJsonObject{{"selection", true}}).toArray();
            QJsonArray curves, meshes;
            for (const QJsonValue& v : picked) {
                QString n = v.toString();
                QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{n}}, {"shapes", true}}).toArray();
                QString st = shapes.isEmpty() ? QString() : ask("nodeType", QJsonObject{{"args", QJsonArray{shapes[0]}}}).toString();
                if (st == "nurbsCurve") curves.append(n);
                if (st == "mesh") meshes.append(n);
            }
            if (!curves.isEmpty()) c["curves"] = curves;
            if (!meshes.isEmpty() && t == "collide") c["meshes"] = meshes;
            exec("xjenAddModifier", c);
        });
        a->setEnabled(has_command("xjenAddModifier"));
    }
    m->addSeparator();
    cmd(m, "Convert Interactive Groom to Polygons", "xjenConvertToPolygons", {}, QString(), true);
    m->addAction("Transfer to Selected Mesh", [] {
        QJsonArray sel = ask("ls", QJsonObject{{"selection", true}}).toArray();
        if (sel.size() >= 2) exec("xjenTransfer", QJsonObject{{"args", QJsonArray{sel[0]}}, {"mesh", sel[sel.size() - 1]}});
    })->setToolTip("Select the description, then the mesh");
    QMenu* cache = m->addMenu("Cache");
    cache->addAction("Export Cache...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(true, "Export Cache", "groom.abc", "Alembic (*.abc)", QString(), [=](const QString& f) {
            QJsonArray range = QJsonArray{ask("playbackOptions", QJsonObject{{"query", true}, {"minTime", true}}), ask("playbackOptions", QJsonObject{{"query", true}, {"maxTime", true}})};
            if (!f.isEmpty()) exec("xjenExportCache", QJsonObject{{"file", f}, {"frameRange", range}});
        });
    })->setEnabled(has_command("xjenExportCache"));
    cache->addAction("Import Cache...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Import Cache", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (!f.isEmpty()) exec("xjenImportCache", QJsonObject{{"file", f}});
        });
    })->setEnabled(has_command("xjenImportCache"));
    cache->addSeparator();
    cache->addAction("Export Selection to Alembic...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(true, "Export Selection to Alembic", "scene.abc", "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (f.isEmpty()) return;
            QString job = QString("-frameRange %1 %2 -uvWrite -worldSpace").arg(ask("playbackOptions", QJsonObject{{"query", true}, {"minTime", true}}).toDouble()).arg(ask("playbackOptions", QJsonObject{{"query", true}, {"maxTime", true}}).toDouble());
            for (const QJsonValue& n : ask("ls", QJsonObject{{"selection", true}, {"long", true}}).toArray()) job += " -root " + n.toString();
            exec("AbcExport", QJsonObject{{"jobArg", job + " -file \"" + f + "\""}});
        });
    })->setEnabled(has_command("AbcExport"));
    cache->addAction("Import Alembic...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Import Alembic", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (!f.isEmpty()) exec("AbcImport", QJsonObject{{"args", QJsonArray{f}}});
        });
    })->setEnabled(has_command("AbcImport"));
    QMenu* sel = m->addMenu("Select");
    sel->addAction("Select Splines on Selected Faces", [] { exec("xgmSplineSelect", QJsonObject{{"replaceBySelectedFaces", true}}); });
    sel->addAction("Freeze Selected Splines", [] { exec("xgmSplineSelect", QJsonObject{{"convertToFreeze", true}}); });
    return m;
}

QMenu* MainWindow::surfaces_menu() {
    auto* m = new QMenu("Surfaces", this);
    cmd(m, "Loft", "loft", {}, QString(), true);
    cmd(m, "Planar", "planar");
    cmd(m, "Revolve", "revolve", {}, QString(), true);
    cmd(m, "Birail", "singleProfileBirailSurface", {}, QString(), true);
    cmd(m, "Extrude", "extrude", {}, QString(), true);
    cmd(m, "Boundary", "boundary", {}, QString(), true);
    cmd(m, "Square", "square", {}, QString(), true);
    cmd(m, "Sweep Mesh", "sweepMeshFromCurve", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Attach", "attachSurface", {}, QString(), true);
    cmd(m, "Detach", "detachSurface", {}, QString(), true);
    cmd(m, "Open/Close", "closeSurface", {}, QString(), true);
    cmd(m, "Insert Isoparms", "insertKnotSurface", {}, QString(), true);
    cmd(m, "Offset", "offsetSurface", {}, QString(), true);
    cmd(m, "Rebuild", "rebuildSurface", {}, QString(), true);
    cmd(m, "Reverse Direction", "reverseSurface", {}, QString(), true);
    return m;
}

QMenu* MainWindow::uv_menu() {
    auto* m = new QMenu("UV", this);
    menu_title(m, "Editors");
    m->addAction("UV Editor", this, &MainWindow::show_uv_editor);
    m->addAction("UV Set Editor", [this] { show_named_window("uvSetEditor"); });
    menu_title(m, "Create");
    cmd(m, "Automatic", "polyAutoProjection", {}, QString(), true);
    cmd(m, "Contour Stretch", "polyContourProjection", {}, QString(), true);
    cmd(m, "Planar", "polyProjection", QJsonObject{{"type", "planar"}}, QString(), true);
    cmd(m, "Cylindrical", "polyProjection", QJsonObject{{"type", "cylindrical"}}, QString(), true);
    cmd(m, "Spherical", "polyProjection", QJsonObject{{"type", "spherical"}}, QString(), true);
    menu_title(m, "Cut and Sew");
    cmd(m, "Auto Seams", "u3dAutoSeam", {}, QString(), true);
    cmd(m, "Cut UV Edges", "polyMapCut");
    m->addAction("Split", [] { mel_action("SplitUV;"); });
    m->addAction("3D Cut and Sew UV Tool", [this] { set_tool("cutSewUv"); });
    cmd(m, "Sew UV Edges", "polyMapSew");
    cmd(m, "Move and Sew", "polyMapSewMove");
    cmd(m, "Merge UVs", "polyMergeUV", {}, QString(), true);
    cmd(m, "Delete UVs", "polyMapDel");
    menu_title(m, "Modify");
    cmd(m, "Unfold", "u3dUnfold", {}, QString(), true);
    cmd(m, "Layout", "u3dLayout", {}, QString(), true);
    cmd(m, "Optimize", "polyOptUvs", {}, QString(), true);
    cmd(m, "Straighten", "polyStraightenUVBorder", {}, QString(), true);
    cmd(m, "Normalize", "polyNormalizeUV", {}, QString(), true);
    cmd(m, "Flip", "polyFlipUV", {}, QString(), true);
    return m;
}

QMenu* MainWindow::deform_menu() {
    auto* m = new QMenu("Deform", this);
    cmd(m, "Blend Shape", "blendShape", {}, QString(), true);
    cmd(m, "Cluster", "cluster", {}, QString(), true);
    auto* nl = m->addMenu("Nonlinear");
    for (const char* t : {"bend", "flare", "sine", "squash", "twist", "wave"}) {
        QString label = QString(t);
        label[0] = label[0].toUpper();
        cmd(nl, label, "nonLinear", QJsonObject{{"type", t}}, QString(), true);
    }
    m->addSeparator();
    cmd(m, "Lattice", "lattice", QJsonObject{{"divisions", QJsonArray{2, 5, 2}}, {"objectCentered", true}}, QString(), true);
    cmd(m, "Wrap", "wrap", {}, QString(), true);
    cmd(m, "Wire", "wire", {}, QString(), true);
    cmd(m, "Soft Modification Tool", "softMod", {}, QString(), true);
    cmd(m, "Delta Mush", "deltaMush", {}, QString(), true);
    cmd(m, "Tension", "tension", {}, QString(), true);
    cmd(m, "Sculpt", "sculpt", {}, QString(), true);
    cmd(m, "Jiggle", "jiggle", {}, QString(), true);
    cmd(m, "ShrinkWrap", "shrinkWrap", {}, QString(), true);
    cmd(m, "Morph", "morph", {}, QString(), true);
    cmd(m, "Wrinkle", "wrinkle", {}, QString(), true);
    cmd(m, "Proximity Wrap", "proximityWrap", {}, QString(), true);
    cmd(m, "Solidify", "solidify", {}, QString(), true);
    cmd(m, "Point On Curve", "pointOnCurveDeformer", {}, QString(), true);
    cmd(m, "Texture", "textureDeformer", {}, QString(), true);
    cmd(m, "Curve Warp", "curveWarp", {}, QString(), true);
    m->addSeparator();
    auto* psd = m->addMenu("Pose Space Deformation");
    cmd(psd, "Create Pose Interpolator", "createPoseInterpolator");
    psd->addAction("Add Pose", [] {
        // A pose of the drivers as they are now, on each selected pose interpolator, under the default name.
        QJsonArray sel = state()["selection"].toArray();
        if (sel.isEmpty()) { log("// Warning: select a pose interpolator //", 4); return; }
        for (const auto& n : sel) exec("poseInterpolator", QJsonObject{{"args", QJsonArray{n}}, {"addPose", ""}});
    });
    m->addSeparator();
    auto* paint = m->addMenu("Paint Weights");
    for (auto [label, type] : {std::pair{"Blend Shape", "blendShape"}, {"Cluster", "cluster"}, {"Delta Mush", "deltaMush"}, {"Jiggle", "jiggle"}, {"Lattice", "ffd"}, {"Nonlinear", "nonLinear"}, {"Soft Modification", "softMod"}, {"Tension", "tension"}, {"Wire", "wire"}, {"Wrap", "wrap"}}) {
        QString ty = type;
        paint->addAction(label, [this, ty] {
            QString found;
            for (const QJsonValue& v : ask("artAttrCtx", QJsonObject{{"query", true}, {"queried", QJsonArray{"paintableAttrs"}}}).toArray()) {
                QString a = v.toString();
                bool match = a.section('.', 0, 0) == ty;
                if (match) { found = a; break; }
            }
            if (found.isEmpty()) { log("// Warning: select a mesh with that deformer //", 4); return; }
            exec_quiet("artAttrCtx", QJsonObject{{"edit", true}, {"attrSelected", found}});
            set_tool("paintAttr");
            show_tool_settings();
        });
    }
    cmd(m, "Mirror Deformer Weights", "copyDeformerWeights", {}, QString(), true);
    cmd(m, "Export Weights", "deformerWeights", QJsonObject{{"export", true}}, QString(), true);
    cmd(m, "Import Weights", "deformerWeights", QJsonObject{{"import", true}}, QString(), true);
    menu_title(m, "Edit");
    QMenu* edit_blend = m->addMenu("Blend Shape");
    edit_blend->addAction("Add", [] { mel_action("AddBlendShapeTarget;"); });
    edit_blend->addAction("Remove", [] { mel_action("RemoveBlendShapeTarget;"); });
    QMenu* edit_lattice = m->addMenu("Lattice");
    cmd(edit_lattice, "Remove Lattice Tweaks", "lattice", QJsonObject{{"edit", true}, {"removeTweaks", true}});
    cmd(edit_lattice, "Reset Lattice", "lattice", QJsonObject{{"edit", true}, {"latticeReset", true}});
    QMenu* edit_wrap = m->addMenu("Wrap");
    edit_wrap->addAction("Add Influence", [] { mel_action("AddWrapInfluence;"); });
    edit_wrap->addAction("Remove Influence", [] { mel_action("RemoveWrapInfluence;"); });
    QMenu* edit_wire = m->addMenu("Wire");
    edit_wire->addAction("Add Wire", [] { mel_action("AddWire;"); });
    menu_title(m, "Membership");
    m->addAction("Edit Membership Tool", [this] { set_tool("editMembership"); show_tool_settings(); });
    QMenu* prune = m->addMenu("Prune Membership");
    for (auto [label, rt] : {std::pair{"Cluster", "PruneCluster;"}, {"Lattice", "PruneLattice;"}, {"Sculpt", "PruneSculpt;"}, {"Wire", "PruneWire;"}}) {
        QString code = rt;
        prune->addAction(label, [code] { mel_action(code); });
    }
    menu_title(m, "Intermediate Objects");
    auto intermediates = [](bool show) {
        return [show] {
            for (const QJsonValue& o : state()["selection"].toArray())
                for (const QJsonValue& s : mel(QString("listRelatives -s %1;").arg(o.toString().section('.', 0, 0))).toArray()) {
                    QString shape = s.toString();
                    if (shape.endsWith("Orig") || shape.contains("Orig"))
                        exec("setAttr", QJsonObject{{"args", QJsonArray{shape + ".intermediateObject", show ? 0 : 1}}});
                }
        };
    };
    m->addAction("Display Intermediate Objects", intermediates(true));
    m->addAction("Hide Intermediate Objects", intermediates(false));
    m->addAction("Shape Editor", [this] { show_named_window("shapeEditor"); });
    return m;
}

QMenu* MainWindow::skeleton_menu() {
    auto* m = new QMenu("Skeleton", this);
    m->addAction("Create Joints", [this] { set_tool("joint"); });
    // Runs on the selected joint until there is a tool to drag the new joint with.
    m->addAction("Insert Joints", [this] { set_tool("insertJoint"); });
    cmd(m, "Mirror Joints", "mirrorJoint", {}, QString(), true);
    m->addAction(new OptionRow("Orient Joint", QString(), [] { orient_selected_joints(); }, [] { open_orient_joint_options(); }, m));
    m->addSeparator();
    cmd(m, "Remove Joints", "removeJoint");
    cmd(m, "Connect Joint", "connectJoint", QJsonObject{{"connectMode", true}});
    cmd(m, "Parent Joint", "connectJoint", QJsonObject{{"parentMode", true}});
    cmd(m, "Disconnect Joint", "disconnectJoint");
    cmd(m, "Reroot Skeleton", "reroot");
    QMenu* label = m->addMenu("Joint Labelling");
    auto set_on_joints = [](const QString& attr, int value) {
        return [attr, value] {
            for (const QJsonValue& j : state()["selection"].toArray())
                exec("setAttr", QJsonObject{{"args", QJsonArray{j.toString() + "." + attr, value}}});
        };
    };
    QMenu* side = label->addMenu("Side");
    for (auto [name, v] : {std::pair{"Center", 0}, {"Left", 1}, {"Right", 2}, {"None", 3}}) side->addAction(name, set_on_joints("side", v));
    QMenu* type = label->addMenu("Type");
    int type_value = 0;
    for (const char* name : {"None", "Root", "Hip", "Knee", "Foot", "Toe", "Spine", "Neck", "Head", "Collar", "Shoulder", "Elbow", "Hand", "Finger", "Thumb"}) type->addAction(name, set_on_joints("type", type_value++));
    label->addAction("Show Selected Labels", set_on_joints("drawLabel", 1));
    label->addAction("Hide Selected Labels", set_on_joints("drawLabel", 0));
    m->addSeparator();
    cmd(m, "Set Preferred Angle", "joint", QJsonObject{{"edit", true}, {"setPreferredAngles", true}, {"children", true}});
    cmd(m, "Assume Preferred Angle", "joint", QJsonObject{{"edit", true}, {"assumePreferredAngles", true}, {"children", true}});
    m->addSeparator();
    m->addAction("Humanoid IK", [this] { show_named_window("humanIK"); });
    QMenu* hik = m->addMenu("Humanoid IK Tools");
    cmd(hik, "Create Skeleton", "hikCreateSkeleton", QJsonObject{{"character", true}}, QString(), true);
    cmd(hik, "Define Character from Selection", "hikCreateCharacter");
    hik->addAction("Create Control Rig", [] { exec("hikCreateControlRig"); });
    m->addSeparator();
    // The two IK tools: click the start joint, then the end joint; the option boxes open their tool settings (#317).
    m->addAction(new OptionRow("Create IK Handle", QString(), [this] { set_tool("ikHandle"); }, [this] { set_tool("ikHandle"); show_tool_settings(); }, m));
    m->addAction(new OptionRow("Create IK Spline Handle", QString(), [this] { set_tool("ikSplineHandle"); }, [this] { set_tool("ikSplineHandle"); show_tool_settings(); }, m));
    cmd(m, "Enable Selected IK Handles", "ikHandle", QJsonObject{{"enableHandles", true}});
    cmd(m, "Disable Selected IK Handles", "ikHandle", QJsonObject{{"disableHandles", true}});
    return m;
}

QMenu* MainWindow::skin_menu() {
    auto* m = new QMenu("Skin", this);
    cmd(m, "Bind Skin", "skinCluster", {}, QString(), true);
    cmd(m, "Unbind Skin", "skinCluster", QJsonObject{{"edit", true}, {"unbind", true}});
    cmd(m, "Go to Bind Pose", "dagPose", QJsonObject{{"restore", true}, {"global", true}, {"bindPose", true}});
    m->addSeparator();
    m->addAction("Paint Skin Weights", [this] { set_tool("paintSkin"); show_tool_settings(); });
    // Skin Tools: layered weights on the skinCluster, painted with the same brush.
    m->addAction("Skin Tools", [this] { show_named_window("skinTools"); });
    m->addAction("Initialize Skin Layers", [] {
        bool any = false;
        for (const QJsonValue& s : state()["selection"].toArray()) {
            QString obj = s.toString().section('.', 0, 0);
            if (ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{obj}}}).toString().isEmpty()) continue;
            exec("skinLayers", QJsonObject{{"args", QJsonArray{obj}}, {"initialize", true}});
            any = true;
        }
        if (!any) log("// Warning: select a skinned mesh //", 4);
    });
    m->addAction("Delete Skin Layers", [] {
        for (const QJsonValue& s : state()["selection"].toArray()) {
            QString obj = s.toString().section('.', 0, 0);
            if (ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{obj}}}).toString().isEmpty()) continue;
            exec("skinLayers", QJsonObject{{"args", QJsonArray{obj}}, {"delete", true}});
        }
    });
    cmd(m, "Hammer Skin Weights", "weightHammerVerts", {});
    cmd(m, "Remove Unused Influences", "removeUnusedInfluences", {});
    cmd(m, "Substitute Geometry", "substituteGeometry", {});
    // Mirror and copy work on the selection: one skinned mesh mirrors onto itself (copySkinWeights with the option box's
    // plane and direction), two copy from the first to the second.
    cmd(m, "Mirror Skin Weights", "copySkinWeights", {}, QString(), true);
    cmd(m, "Copy Vertex Weights", "artAttrSkinWeightCopy");
    cmd(m, "Paste Vertex Weights", "artAttrSkinWeightPaste");
    m->addAction("Copy Skin Weights", [] {
        QJsonArray sel = state()["selection"].toArray();
        if (sel.size() < 2) { log("// Warning: select the source mesh, then the destination //", 4); return; }
        exec("copySkinWeights", QJsonObject{{"args", QJsonArray{sel.at(0), sel.at(sel.size() - 1)}}, {"noMirror", true}, {"influenceAssociation", QJsonArray{"closestJoint", "oneToOne"}}});
    });
    auto on_selected = [](const QString& what, std::function<QJsonObject()> call, const QString& command) {
        return [what, call, command] {
            QString obj = state()["selection"].toArray().at(0).toString().section('.', 0, 0);
            QString sk = ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{obj}}}).toString();
            if (sk.isEmpty()) { log("// Warning: select a skinned mesh //", 4); return; }
            QJsonObject c = call();
            c["args"] = QJsonArray{sk};
            exec(command, c);
        };
    };
    m->addAction("Smooth Skin Weights", on_selected("smooth", [] { return QJsonObject{{"edit", true}, {"smoothWeights", 0.5}, {"smoothWeightsMaxIterations", 2}}; }, "skinCluster"));
    m->addAction("Prune Small Weights", on_selected("prune", [] { return QJsonObject{{"pruneWeights", 0.01}}; }, "skinPercent"));
    m->addAction("Normalize Weights", on_selected("normalize", [] { return QJsonObject{{"edit", true}, {"forceNormalizeWeights", true}}; }, "skinCluster"));
    m->addAction("Reset Default Weights", on_selected("reset", [] { return QJsonObject{{"resetToDefault", true}}; }, "skinPercent"));
    m->addSeparator();
    // Selection order: the skinned mesh and the joints.
    auto influence = [](const char* flag) {
        QString fl = flag;
        return [fl] {
            QString sk;
            QJsonArray joints;
            for (const QJsonValue& s : state()["selection"].toArray()) {
                QString found = ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{s.toString().section('.', 0, 0)}}}).toString();
                if (!found.isEmpty()) sk = found;
                else joints.append(s);
            }
            if (sk.isEmpty() || joints.isEmpty()) { log("// Warning: select a skinned mesh and joints //", 4); return; }
            QJsonObject c{{"args", QJsonArray{sk}}, {"edit", true}};
            c[fl] = joints;
            exec("skinCluster", c);
        };
    };
    QMenu* infl = m->addMenu("Edit Influences");
    infl->addAction("Add Influence", influence("addInfluence"));
    infl->addAction("Remove Influence", influence("removeInfluence"));
    cmd(m, "Export Weight Maps", "deformerWeights", QJsonObject{{"export", true}}, QString(), true);
    cmd(m, "Import Weight Maps", "deformerWeights", QJsonObject{{"import", true}}, QString(), true);
    return m;
}

QMenu* MainWindow::constrain_menu() {
    auto* m = new QMenu("Constrain", this);
    cmd(m, "Parent", "parentConstraint", {}, QString(), true);
    cmd(m, "Point", "pointConstraint", {}, QString(), true);
    cmd(m, "Orient", "orientConstraint", {}, QString(), true);
    cmd(m, "Scale", "scaleConstraint", {}, QString(), true);
    cmd(m, "Aim", "aimConstraint", {}, QString(), true);
    cmd(m, "Pole Vector", "poleVectorConstraint", {}, QString(), true);
    m->addAction("Rivet", [] {
        // A locator held to the selected components.
        QJsonArray comps = ask("ls", QJsonObject{{"selection", true}, {"flatten", true}}).toArray();
        if (comps.isEmpty()) { log("// Warning: select a vertex, edge or face //", 4); return; }
        QString loc = exec("spaceLocator", QJsonObject{{"name", "rivet1"}}).toArray().at(0).toString();
        QJsonArray a{comps.at(0), loc};
        exec("pointOnPolyConstraint", QJsonObject{{"args", a}});
    });
    m->addSeparator();
    // The reference's order (#121): Motion Paths, Closest Point, Point On Poly, Geometry, Normal, Tangent, then the
    // matrix constraints and the editing of those made already.
    auto* path = m->addMenu("Motion Paths");
    cmd(path, "Attach to Motion Path", "pathAnimation", QJsonObject{{"follow", true}, {"fractionMode", true}, {"startTimeU", 1}, {"endTimeU", 120}});
    cmd(path, "Flow Path Object", "flow");
    m->addAction("Closest Point", [] {
        // Objects then a mesh: a locator for each object, held where the mesh is nearest it.
        const QJsonArray sel = state()["selection"].toArray();
        if (sel.size() < 2) { log("// Warning: select one or more objects, then a mesh //", 4); return; }
        const QString mesh = sel.last().toString();
        const QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{mesh}}, {"shapes", true}, {"type", "mesh"}}).toArray();
        if (shapes.isEmpty()) { log("// Warning: the last object selected is not a mesh //", 4); return; }
        const QString shape = shapes.at(0).toString();
        QJsonArray made;
        for (int i = 0; i < sel.size() - 1; ++i) {
            const QString obj = sel.at(i).toString();
            const QString cp = exec("createNode", QJsonObject{{"args", QJsonArray{"closestPointOnMesh"}}, {"name", obj.section('|', -1) + "_closestPoint1"}}).toString();
            const QString dm = exec("createNode", QJsonObject{{"args", QJsonArray{"decomposeMatrix"}}}).toString();
            const QString loc = exec("spaceLocator", QJsonObject{{"name", obj.section('|', -1) + "_closestPointLocator1"}}).toArray().at(0).toString();
            exec("connectAttr", QJsonObject{{"args", QJsonArray{shape + ".worldMesh[0]", cp + ".inMesh"}}});
            exec("connectAttr", QJsonObject{{"args", QJsonArray{mesh + ".worldMatrix[0]", cp + ".inputMatrix"}}});
            exec("connectAttr", QJsonObject{{"args", QJsonArray{obj + ".worldMatrix[0]", dm + ".inputMatrix"}}});
            exec("connectAttr", QJsonObject{{"args", QJsonArray{dm + ".outputTranslate", cp + ".inPosition"}}});
            exec("connectAttr", QJsonObject{{"args", QJsonArray{cp + ".position", loc + ".translate"}}});
            made.append(loc);
        }
        exec("select", QJsonObject{{"args", made}});
    });
    cmd(m, "Point On Poly", "pointOnPolyConstraint", {}, QString(), true);
    cmd(m, "Geometry", "geometryConstraint", {}, QString(), true);
    cmd(m, "Normal", "normalConstraint", {}, QString(), true);
    cmd(m, "Tangent", "tangentConstraint", {}, QString(), true);
    m->addSection("Matrix Constraints");
    cmd(m, "UV Pin", "UVPin");
    cmd(m, "Proximity Pin", "ProximityPin");
    m->addSection("Modify");
    cmd(m, "Remove Target", "RemoveConstraintTarget");
    m->addAction("Set Rest Position", [] {
        // Each selected constraint, or those under the selected objects: the object as it stands is where it rests.
        int done = 0;
        for (const QJsonValue& v : state()["selection"].toArray()) {
            const QString n = v.toString();
            QJsonArray cs;
            if (ask("objectType", QJsonObject{{"args", QJsonArray{n}}, {"isAType", "constraint"}}).toBool()) cs.append(n);
            else cs = ask("listRelatives", QJsonObject{{"args", QJsonArray{n}}, {"type", "constraint"}, {"fullPath", true}}).toArray();
            for (const QJsonValue& c : cs) {
                const QString con = c.toString();
                const QJsonArray up = ask("listRelatives", QJsonObject{{"args", QJsonArray{con}}, {"parent", true}, {"fullPath", true}}).toArray();
                if (up.isEmpty()) continue;
                const QString obj = up.at(0).toString();
                for (auto pr : {std::pair{"restTranslate", "translate"}, std::pair{"restRotate", "rotate"}}) {
                    if (!ask("attributeQuery", QJsonObject{{"args", QJsonArray{pr.first}}, {"node", con}, {"exists", true}}).toBool()) continue;
                    if (ask("attributeQuery", QJsonObject{{"args", QJsonArray{pr.first}}, {"node", con}, {"hidden", true}}).toBool()) continue;
                    QJsonArray val = ask("getAttr", QJsonObject{{"args", QJsonArray{obj + "." + pr.second}}}).toArray();
                    if (val.size() == 1 && val.at(0).isArray()) val = val.at(0).toArray();
                    if (val.size() != 3) continue;
                    exec("setAttr", QJsonObject{{"args", QJsonArray{con + "." + pr.first, val.at(0), val.at(1), val.at(2)}}});
                }
                ++done;
            }
        }
        if (!done) log("// Warning: select constraints, or objects with constraints //", 4);
    });
    m->addAction("Modify Constrained Axis...", [this] {
        // Which channels of the selected objects their constraints drive: each axis on reconnects the constraint's
        // output to it, off disconnects it, as the reference's window does.
        QDialog d(this);
        d.setWindowTitle("Modify Constrained Axis");
        auto* form = new QFormLayout(&d);
        QHash<QString, QCheckBox*> boxes;
        for (const char* ch : {"translate", "rotate", "scale"}) {
            auto* row = new QWidget(&d);
            auto* h = new QHBoxLayout(row);
            h->setContentsMargins(0, 0, 0, 0);
            for (const char* ax : {"X", "Y", "Z"}) {
                auto* b = new QCheckBox(ax, row);
                b->setChecked(true);
                h->addWidget(b);
                boxes.insert(QString(ch) + ax, b);
            }
            form->addRow(QString(ch).left(1).toUpper() + QString(ch).mid(1) + ":", row);
        }
        auto* bb = new QDialogButtonBox(QDialogButtonBox::Apply | QDialogButtonBox::Close, &d);
        form->addRow(bb);
        connect(bb, &QDialogButtonBox::rejected, &d, &QDialog::reject);
        connect(bb->button(QDialogButtonBox::Apply), &QPushButton::clicked, &d, [boxes] {
            for (const QJsonValue& v : state()["selection"].toArray()) {
                const QString obj = v.toString();
                for (const QJsonValue& c : ask("listRelatives", QJsonObject{{"args", QJsonArray{obj}}, {"type", "constraint"}, {"fullPath", true}}).toArray()) {
                    const QString con = c.toString();
                    for (auto it = boxes.begin(); it != boxes.end(); ++it) {
                        const QString ch = it.key().chopped(1), ax = it.key().right(1);
                        const QString out = con + ".constraint" + ch.left(1).toUpper() + ch.mid(1) + ax;
                        if (!ask("attributeQuery", QJsonObject{{"args", QJsonArray{"constraint" + ch.left(1).toUpper() + ch.mid(1)}}, {"node", con}, {"exists", true}}).toBool()) continue;
                        if (ask("attributeQuery", QJsonObject{{"args", QJsonArray{"constraint" + ch.left(1).toUpper() + ch.mid(1)}}, {"node", con}, {"hidden", true}}).toBool()) continue;
                        const QString in = obj + "." + it.key();
                        const bool linked = ask("isConnected", QJsonObject{{"args", QJsonArray{out, in}}}).toBool();
                        if (it.value()->isChecked() && !linked) exec("connectAttr", QJsonObject{{"args", QJsonArray{out, in}}, {"force", true}});
                        if (!it.value()->isChecked() && linked) exec("disconnectAttr", QJsonObject{{"args", QJsonArray{out, in}}});
                    }
                }
            }
        });
        d.exec();
    });
    return m;
}

QMenu* MainWindow::control_menu() {
    auto* m = new QMenu("Control", this);
    cmd(m, "Add Attribute...", "addAttr", {}, QString(), true);
    cmd(m, "Delete Attribute", "deleteAttr");
    cmd(m, "Connect Attribute", "connectAttr", {}, QString(), true);
    return m;
}

QMenu* MainWindow::key_menu() {
    auto* m = new QMenu("Key", this);
    cmd(m, "Set Key", "setKeyframe", {}, "S", true);
    cmd(m, "Hold Current Keys", "setKeyframe", QJsonObject{{"hierarchy", "none"}});
    auto* tk = m->addMenu("Set Transform Keys");
    cmd(tk, "Translate", "setKeyframe", QJsonObject{{"attribute", "translate"}}, "Shift+W");
    cmd(tk, "Rotate", "setKeyframe", QJsonObject{{"attribute", "rotate"}}, "Shift+E");
    cmd(tk, "Scale", "setKeyframe", QJsonObject{{"attribute", "scale"}}, "Shift+R");
    m->addAction("Auto Key", [this](bool) { auto_key_->trigger(); });
    m->addSeparator();
    cmd(m, "Delete Keys", "cutKey", {}, QString(), true);
    m->addAction("Delete Keys at Current Frame", [this] {
        exec("cutKey", QJsonObject{{"time", time_slider_->time()}});
    });
    m->addSeparator();
    QMenu* ikfk = m->addMenu("IK/FK Keys");
    ikfk->addAction("Set IK/FK Key", [] { mel_action("SetIKFKKeyframe;"); });
    ikfk->addAction("Enable IK Solver", [] { mel_action("EnableIKSolvers;"); });
    ikfk->addAction("Move IK to FK", [] { mel_action("MoveIKtoFK;"); });
    m->addSeparator();
    cmd(m, "Set Breakdown", "setKeyframe", QJsonObject{{"breakdown", true}}, QString(), false);
    m->addAction("Convert to Breakdown", [this] { exec("keyframe", QJsonObject{{"edit", true}, {"time", time_slider_->time()}, {"breakdown", true}}); });
    m->addAction("Convert to Key", [this] { exec("keyframe", QJsonObject{{"edit", true}, {"time", time_slider_->time()}, {"breakdown", false}}); });
    m->addSeparator();
    m->addAction("Copy Keys", [this] { exec("copyKey", QJsonObject{{"time", QString("%1").arg(time_slider_->time())}}); });
    m->addAction("Paste Keys", [this] { exec("pasteKey", QJsonObject{{"time", time_slider_->time()}, {"option", "merge"}}); });
    cmd(m, "Scale Keys", "scaleKey", {}, QString(), true);
    cmd(m, "Snap Keys", "snapKey", {}, QString(), true);
    cmd(m, "Bake Animation", "bakeResults", {}, QString(), true);
    m->addSeparator();
    auto* sdk = m->addMenu("Set Driven Key");
    sdk->addAction("Set...", this, &MainWindow::show_set_driven_key);
    m->addSeparator();
    auto* tan = m->addMenu("Tangents");
    for (const char* t : {"auto", "spline", "clamped", "linear", "flat", "step", "plateau"}) {
        QString label = QString(t);
        label[0] = label[0].toUpper();
        cmd(tan, label, "keyTangent", QJsonObject{{"inTangentType", t == QString("step") ? "clamped" : t}, {"outTangentType", t}});
    }
    auto* def = m->addMenu("Default Tangents");
    for (const char* t : {"auto", "spline", "clamped", "linear", "flat", "step", "plateau"}) {
        QString label = QString(t);
        label[0] = label[0].toUpper();
        cmd(def, label, "keyTangent", QJsonObject{{"global", true}, {"inTangentType", t == QString("step") ? "clamped" : t}, {"outTangentType", t}});
    }
    m->addSeparator();
    auto* path = m->addMenu("Motion Paths");
    cmd(path, "Attach to Motion Path", "pathAnimation", QJsonObject{{"follow", true}, {"fractionMode", true}, {"startTimeU", 1}, {"endTimeU", 120}});
    cmd(path, "Flow Path Object", "flow");
    auto* layers = m->addMenu("Animation Layers");
    cmd(layers, "Create Layer", "animLayer");
    cmd(layers, "Create Layer from Selected", "animLayer", QJsonObject{{"addSelectedObjects", true}});
    cmd(layers, "Merge Layers", "mergeAnimLayers");
    cmd(m, "Create Character Set", "character");
    m->addSeparator();
    m->addAction("Graph Editor...", [this] { show_graph_editor(); });
    m->addAction("Dope Sheet...", [this] { show_dope_sheet(); });
    return m;
}

QMenu* MainWindow::visualize_menu() {
    auto* vis = new QMenu("Visualize", this);
    cmd(vis, "Create Motion Trail", "snapshot", QJsonObject{{"motionTrail", true}});
    cmd(vis, "Animation Snapshot", "snapshot", QJsonObject{{"constructionHistory", true}});
    vis->addSeparator();
    cmd(vis, "Ghost Selected", "GhostSelected");
    cmd(vis, "Unghost Selected", "UnghostSelected");
    cmd(vis, "Unghost All", "UnghostAll");
    return vis;
}

QMenu* MainWindow::audio_menu() {
    auto* m = new QMenu("Audio", this);
    m->addAction("Import Audio...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Import Audio", QString(), "WAV (*.wav)", QString(), [=](const QString& f) {
            if (f.isEmpty()) return;
            QJsonValue n = exec("sound", QJsonObject{{"file", f}, {"offset", time_slider_->time()}});
            if (n.isString()) exec("timeControl", QJsonObject{{"sound", n.toString()}, {"displaySound", true}});
            time_slider_->update();
        });
    });
    // Which audio node's waveform the time slider shows, as its right-click menu offers.
    QMenu* shown = m->addMenu("Show in Time Slider");
    connect(shown, &QMenu::aboutToShow, this, [this, shown] {
        shown->clear();
        QString now = ask("timeControl", QJsonObject{{"query", true}, {"queried", QJsonArray{"sound"}}}).toString();
        QAction* none = shown->addAction("None", [this] { exec("timeControl", QJsonObject{{"sound", ""}}); time_slider_->update(); });
        none->setCheckable(true);
        none->setChecked(now.isEmpty());
        for (const QJsonValue& v : ask("ls", QJsonObject{{"type", "audio"}}).toArray()) {
            QString n = v.toString();
            QAction* a = shown->addAction(n, [this, n] { exec("timeControl", QJsonObject{{"sound", n}, {"displaySound", true}}); time_slider_->update(); });
            a->setCheckable(true);
            a->setChecked(n == now);
        }
    });
    m->addAction("Delete All Audio", [this] {
        QJsonArray all = ask("ls", QJsonObject{{"type", "audio"}}).toArray();
        if (all.isEmpty()) return;
        exec("timeControl", QJsonObject{{"sound", ""}});
        exec("delete", QJsonObject{{"args", all}});
        time_slider_->update();
    });
    return m;
}

QMenu* MainWindow::cache_menu() {
    auto* m = new QMenu("Cache", this);
    QMenu* abc = m->addMenu("Alembic Cache");
    auto range = [] {
        return QString("-frameRange %1 %2").arg(ask("playbackOptions", QJsonObject{{"query", true}, {"minTime", true}}).toDouble()).arg(ask("playbackOptions", QJsonObject{{"query", true}, {"maxTime", true}}).toDouble());
    };
    abc->addAction("Export All to Alembic...", [this, range] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(true, "Export All to Alembic", "scene.abc", "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (!f.isEmpty()) exec("AbcExport", QJsonObject{{"jobArg", range() + " -uvWrite -worldSpace -file \"" + f + "\""}});
        });
    })->setEnabled(has_command("AbcExport"));
    abc->addAction("Export Selection to Alembic...", [this, range] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(true, "Export Selection to Alembic", "scene.abc", "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (f.isEmpty()) return;
            QString job = range() + " -uvWrite -worldSpace";
            for (const QJsonValue& n : ask("ls", QJsonObject{{"selection", true}, {"long", true}}).toArray()) job += " -root " + n.toString();
            exec("AbcExport", QJsonObject{{"jobArg", job + " -file \"" + f + "\""}});
        });
    })->setEnabled(has_command("AbcExport"));
    abc->addAction("Import Alembic...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Import Alembic", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (!f.isEmpty()) exec("AbcImport", QJsonObject{{"args", QJsonArray{f}}});
        });
    })->setEnabled(has_command("AbcImport"));
    // As the reference's Alembic Cache menu (#87): open in a new scene, reference, drive the selection by name, and
    // point the selected AlembicNodes at another file.
    abc->insertAction(abc->actions().value(0), [&] {
        auto* a = new QAction("Open Alembic...", abc);
        connect(a, &QAction::triggered, this, [this] {
            if (!confirm_unsaved()) return;
            // Leaves Aztek usable while it is open, over the main window (#109).
            browse_file(false, "Open Alembic", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
                if (!f.isEmpty()) exec("AbcImport", QJsonObject{{"args", QJsonArray{f}}, {"mode", "open"}});
            });
        });
        a->setEnabled(has_command("AbcImport"));
        return a;
    }());
    abc->insertAction(abc->actions().value(1), [&] {
        auto* a = new QAction("Reference Alembic...", abc);
        connect(a, &QAction::triggered, this, [this] {
            // Leaves Aztek usable while it is open, over the main window (#109).
            browse_file(false, "Reference Alembic", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
                if (!f.isEmpty()) exec("file", QJsonObject{{"args", QJsonArray{f}}, {"reference", true}, {"type", "Alembic"}, {"namespace", QFileInfo(f).completeBaseName()}});
            });
        });
        a->setEnabled(has_command("AbcImport"));
        return a;
    }());
    abc->addAction("Import Alembic onto Selection...", [this] {
        // The file's animated objects drive the selected hierarchies' nodes of the same names.
        QJsonArray sel = ask("ls", QJsonObject{{"selection", true}, {"long", true}}).toArray();
        if (sel.isEmpty()) { log("// Warning: select the hierarchies the cache drives //", 4); return; }
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Import Alembic onto Selection", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
            QStringList roots;
            for (const QJsonValue& v : sel) roots << v.toString();
            if (!f.isEmpty()) exec("AbcImport", QJsonObject{{"args", QJsonArray{f}}, {"connect", roots.join(' ')}});
        });
    })->setEnabled(has_command("AbcImport"));
    abc->addAction("Replace Alembic...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Replace Alembic", QString(), "Alembic (*.abc)", QString(), [=](const QString& f) {
            if (!f.isEmpty()) exec("AbcImport", QJsonObject{{"args", QJsonArray{f}}, {"mode", "replace"}});
        });
    })->setEnabled(has_command("AbcImport"));
    // Export after the reading ones, as the reference orders them.
    for (QAction* a : abc->actions())
        if (a->text().startsWith("Export")) {
            abc->removeAction(a);
            abc->addAction(a);
        }
    abc->insertSeparator(abc->actions().value(abc->actions().size() - 2));
    return m;
}

QMenu* MainWindow::playback_menu() {
    auto* m = new QMenu("Playback", this);
    m->addAction("Play / Stop\tAlt+V", [this] { toggle_play(1); });
    m->addAction("Play Backwards", [this] { toggle_play(-1); });
    m->addSeparator();
    m->addAction("Go to Start of Playback Range", [this] { go_to_time(time_slider_->start()); });
    m->addAction("Go to End of Playback Range", [this] { go_to_time(time_slider_->end()); });
    m->addAction("Next Frame\tAlt+.", [this] { step_time(1); });
    m->addAction("Previous Frame\tAlt+,", [this] { step_time(-1); });
    m->addAction("Next Key\t.", [this] { step_key("next"); });
    m->addAction("Previous Key\t,", [this] { step_key("previous"); });
    return m;
}

void MainWindow::build_menus() {
    QMenuBar* mb = menuBar();
    mb->setNativeMenuBar(false);
    // Themes, top right of the menu bar: the interface's colours, fonts and shapes, and the Theme Editor.
    QComboBox* themes = make_theme_box(mb);
    // Beside it, the lock of the window's layout, as the reference's workspace lock (#90): locked, the docked panels
    // can't be dragged off, moved or floated, only closed.
    auto* corner = new QWidget(mb);
    auto* row = new QHBoxLayout(corner);
    row->setContentsMargins(0, 0, 2, 0);
    row->setSpacing(2);
    // The reference's Workspace box (#134): its workspaces, those saved here, and Reset, Save and Delete.
    row->addWidget(new QLabel("Workspace:", corner));
    workspace_box_ = new QComboBox(corner);
    workspace_box_->setObjectName("workspaceBox");
    workspace_box_->setSizeAdjustPolicy(QComboBox::AdjustToContents);
    auto fill_workspaces = [this] {
        QSignalBlocker b(workspace_box_);
        workspace_box_->clear();
        workspace_box_->addItems(workspace_names());
        QSettings s;
        s.beginGroup("workspaces");
        const QStringList mine = s.childKeys();
        s.endGroup();
        for (const QString& k : mine)
            if (!workspace_names().contains(k)) workspace_box_->addItem(k);
        workspace_box_->insertSeparator(workspace_box_->count());
        for (const char* a : {"Reset Current Workspace", "Save Current Workspace As...", "Delete Current Workspace"}) workspace_box_->addItem(a, QString("action"));
        workspace_box_->setCurrentText(s.value("workspaces_current", "General").toString());
    };
    fill_workspaces();
    connect(workspace_box_, &QComboBox::activated, this, [this, fill_workspaces](int i) {
        const QString current = QSettings().value("workspaces_current", "General").toString();
        const QString picked = workspace_box_->itemText(i);
        if (workspace_box_->itemData(i).toString() != "action") {
            apply_workspace(picked);
            return;
        }
        if (picked.startsWith("Reset")) {
            if (workspace_names().contains(current)) QSettings().remove("workspaces/" + current);
            apply_workspace(current);
        } else if (picked.startsWith("Save")) {
            bool ok = false;
            const QString n = QInputDialog::getText(this, "Save Workspace", "Name:", QLineEdit::Normal, current, &ok).trimmed();
            // As workspaceLayoutManager -saveAs: the core keeps the layout's file, the window its arrangement.
            if (ok && !n.isEmpty()) exec("workspaceLayoutManager", QJsonObject{{"saveAs", n}});
        } else if (!workspace_names().contains(current)) {
            exec("workspaceLayoutManager", QJsonObject{{"delete", current}});
            QSettings().remove("workspaces/" + current);
            QSettings().setValue("workspaces_current", "General");
        }
        fill_workspaces();
    });
    row->addWidget(workspace_box_);
    row->addWidget(themes);
    auto* lock = new QToolButton(corner);
    lock->setObjectName("uiLock");
    lock->setAutoRaise(true);
    lock->setCheckable(true);
    lock->setIconSize(QSize(16, 16));
    auto show_lock = [lock](bool on) {
        lock->setIcon(icon(on ? "uiLocked" : "uiUnlocked"));
        lock->setToolTip(on ? "The layout is locked: panels stay where they are. Click to unlock." : "Lock the layout: panels can't be moved, docked or floated");
    };
    const bool locked = QSettings().value("window/uiLocked", false).toBool();
    lock->setChecked(locked);
    show_lock(locked);
    connect(lock, &QToolButton::toggled, this, [this, show_lock](bool on) {
        QSettings().setValue("window/uiLocked", on);
        show_lock(on);
        lock_layout(on);
    });
    row->addWidget(lock);
    mb->setCornerWidget(corner, Qt::TopRightCorner);
    lock_layout(locked);
    build_menu_set(menu_set_);
}

void MainWindow::build_menu_set(const QString& set) {
    // The reference calls the dynamics menu set FX; scripts still say Dynamics.
    const QString wanted = set == "Dynamics" ? QString("FX") : set;
    QMenuBar* mb = menuBar();
    mb->clear();
    for (QMenu* m : set_menus_) m->deleteLater();
    for (QMenu* m : hotbox_menus_) m->deleteLater();
    hotbox_menus_.clear();
    QList<QMenu*> common = {file_menu(), edit_menu(), create_menu(), select_menu(), modify_menu(), display_menu(), windows_menu()};
    auto* recents = common[1]->findChild<QMenu*>("recentCommands");
    auto* light = lighting_shading_menu();
    auto* deform = deform_menu();
    auto* generate = generate_menu();
    auto* playback = playback_menu();
    auto* constrain = constrain_menu();
    QList<QPair<QString, QList<QMenu*>>> sets = {
        {"Modeling", {mesh_menu(), edit_mesh_menu(), mesh_tools_menu(), mesh_display_menu(), curves_menu(), surfaces_menu(), uv_menu(), deform, generate}},
        {"Rigging", {skeleton_menu(), skin_menu(), deform, constrain, control_menu()}},
        {"Animation", {key_menu(), playback, audio_menu(), visualize_menu(), deform, constrain, cache_menu()}},
        {"FX", dynamics_menus() + QList<QMenu*>{generate, playback}},
        {"Rendering", {render_menu(this), harold_menu(this)}},
    };
    set_menus_ = common;
    set_menus_.append(light);
    for (const auto& [name, menus] : sets)
        if (name == wanted) set_menus_.append(menus);
    for (const auto& entry : sets)
        for (auto* menu : entry.second)
            if (!set_menus_.contains(menu) && !hotbox_menus_.contains(menu)) hotbox_menus_.append(menu);
    for (QMenu* m : set_menus_) {
        m->setToolTipsVisible(true);
        mb->addMenu(m);
    }
    auto* help = new QMenu("Help", this);
    help->addAction("Welcome Screen", [this] { show_welcome(); });
    help->addAction("This doesn't feel right!", [this] { open_feedback(this); });
    help->addAction("Check for Updates...", [this] { check_for_updates(this, [this] { return close(); }); });
    // Tools animators and riggers add to the reference, each built and checked for Aztek.
    QMenu* install = help->addMenu("Install");
    install->addAction("mGear...", [this] { install_mgear(this); });
    help->addAction("Thanks", [this] { show_thanks(this); });
    help->addAction("About Aztek", [this] { show_about(this); });
    mb->addMenu(help);
    set_menus_.append(help);
    // The menus scripts hung on the main window stay, after Help, whatever the menu set.
    for (QAction* a : ScriptUi::get()->main_window_menus()) mb->addAction(a);
    if (hotbox_) hotbox_->deleteLater();
    common.append(help);
    sets.last().second.prepend(light);
    // hotBox's polygons and surfaces rows are these of the Modeling row's menus.
    for (QMenu* m : sets.first().second) {
        const QString t = QString(m->title()).remove('&');
        if (t == "Mesh" || t == "Edit Mesh" || t == "Mesh Tools" || t == "Mesh Display" || t == "UV") m->setProperty("azHotboxGroup", "polygons");
        else if (t == "Curves" || t == "Surfaces") m->setProperty("azHotboxGroup", "surfaces");
    }
    hotbox_ = new Hotbox(common, sets, recents, this);
    hotbox_->set_zones([this](int zone, QPoint origin) { return hotbox_zone(zone, origin); });
    connect(hotbox_, &Hotbox::spaceReleased, this, [this](bool tap) {
        held_.remove(Qt::Key_Space);
        space_held_ = false;
        exec_quiet("hotBox", QJsonObject{{"release", true}});
        if (tap) panels_->toggle();
    });
    connect(hotbox_, &Hotbox::used, this, [] { ask("aztekHotBox", QJsonObject{{"used", true}}); });
    // Hotbox Controls rows go to hotBox, so they hold for the next pop and scripts see them.
    connect(hotbox_, &Hotbox::rowToggled, this, [](const QString& row, bool on) {
        exec_quiet("hotBox", QJsonObject{{row == "pane" ? QString("PaneToggleMenus") : row + "ToggleMenus", on}});
    });
}

void MainWindow::show_hotbox(QPoint global, bool hold) {
    if (!hotbox_) return;
    const QJsonObject st = ask("aztekHotBox").toObject();
    const QJsonObject on = st["rows"].toObject();
    QHash<QString, bool> rows;
    for (auto it = on.begin(); it != on.end(); ++it) rows[it.key()] = it.value().toBool(true);
    hotbox_->set_look(st["style"].toString("dh"), st["transparency"].toInt(25), st["rmbPopups"].toBool(true),
                      set_menus_.mid(7, std::max(0, int(set_menus_.size()) - 8)));
    hotbox_->set_rows_shown(rows);
    hotbox_->set_hold(hold);
    ModelPanel* panel = panels_ ? panels_->panel_at(global) : nullptr;
    hotbox_->popup(global, panel ? panel->panel_menu_actions() : QList<QAction*>());
    ask("aztekHotBox", QJsonObject{{"shown", QJsonArray{global.x(), global.y()}}});
}

// ---------------------------------------------------------------------------------------------------------------------
// Status line, shelf, toolbox

void MainWindow::build_toolbox() {
    QToolBar* tb = new QToolBar("Toolbox", this);
    tb->setObjectName("toolbox");
    tb->setMovable(false);
    tb->setIconSize(QSize(26, 26));
    addToolBar(Qt::LeftToolBarArea, tb);
    auto* tools = new QActionGroup(this);
    struct T { const char* id; const char* ic; const char* tip; };
    for (T t : {T{"select", "select", "Select Tool (Q)"}, T{"lasso", "lasso", "Lasso Tool"}, T{"paintSelect", "paintSelect", "Paint Selection Tool"},
                T{"move", "move", "Move Tool (W)"}, T{"rotate", "rotate", "Rotate Tool (E)"}, T{"scale", "scale", "Scale Tool (R)"}}) {
        QString id = t.id;
        QAction* a = tb->addAction(icon(t.ic), t.tip, [this, id] { set_tool(id); });
        a->setCheckable(true);
        tools->addAction(a);
        tool_buttons_[id] = a;
    }
    tool_buttons_["select"]->setChecked(true);
    // Double-clicking a tool opens its settings, as in the reference.
    for (const QString& id : tool_buttons_.keys())
        if (QWidget* w = tb->widgetForAction(tool_buttons_[id])) {
            w->setProperty("toolId", id);
            w->installEventFilter(this);
        }
    tb->addSeparator();
    tb->addAction(icon("single"), "Single Perspective View", [this] { panels_->set_layout(0); });
    tb->addAction(icon("four"), "Four View", [this] { panels_->set_layout(4); });
    tb->addAction(icon("perspOutliner"), "Perspective/Outliner", [this] {
        panels_->set_layout(0);
        outliner_dock_->show();
    });
    QAction* last = tb->addAction(icon("lastTool"), "Last Tool (Y)", [this] { set_tool(last_tool_); });
    Q_UNUSED(last);
    tb->addAction(icon("outlinerIcon"), "Outliner", [this] {
        outliner_dock_->setVisible(!outliner_dock_->isVisible());
        if (outliner_dock_->isVisible()) outliner_dock_->raise();
    });
    // The application's mark at the foot of the toolbox, where the reference has its own.
    auto* fill = new QWidget(tb);
    fill->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Expanding);
    tb->addWidget(fill);
    auto* mark = new QLabel(tb);
    mark->setObjectName("toolboxMark");
    mark->setAlignment(Qt::AlignCenter);
    {
        // The app icon's mark, the dog's head forming the A, in the brand orange without its black tile: the icon's
        // white turned to orange, its black to nothing.
        const qreal dpr = devicePixelRatioF();
        QImage src = QImage(":/aztek-icon.png").convertToFormat(QImage::Format_ARGB32);
        QImage out(src.size(), QImage::Format_ARGB32);
        const QColor orange(0xff, 0x8a, 0x00);
        for (int y = 0; y < src.height(); ++y) {
            const QRgb* in = reinterpret_cast<const QRgb*>(src.constScanLine(y));
            QRgb* o = reinterpret_cast<QRgb*>(out.scanLine(y));
            for (int x = 0; x < src.width(); ++x) {
                const int a = qGray(in[x]) * qAlpha(in[x]) / 255;
                o[x] = qRgba(orange.red(), orange.green(), orange.blue(), a);
            }
        }
        QPixmap pm = QPixmap::fromImage(out.scaled(QSize(32, 32) * dpr, Qt::KeepAspectRatio, Qt::SmoothTransformation));
        pm.setDevicePixelRatio(dpr);
        mark->setPixmap(pm);
    }
    mark->setToolTip("Aztek");
    tb->addWidget(mark);
}

// The grab handle down the left of the Time and Range Sliders: a column of dots, and room either side of it. Clicks on
// it go to the dock, which drags it and docks or floats it on a double-click.
class SliderGrip : public QWidget {
public:
    static constexpr QDockWidget::DockWidgetFeatures features =
        QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable | QDockWidget::DockWidgetVerticalTitleBar;
    explicit SliderGrip(QWidget* parent) : QWidget(parent) {
        setFixedWidth(10);
        setCursor(Qt::SizeAllCursor);
        setToolTip("Drag to move, or off the window to float; double-click to dock");
    }
    QSize sizeHint() const override { return {10, 10}; }
    QSize minimumSizeHint() const override { return {10, 10}; }

protected:
    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        const QColor c = palette().color(QPalette::Mid);
        for (int y = 3; y + 2 <= height() - 2; y += 4)
            for (int x : {2, 6}) p.fillRect(x, y, 2, 2, c);
    }
};

void MainWindow::lock_layout(bool on) {
    for (QDockWidget* d : findChildren<QDockWidget*>())
        if (d != time_dock_ && d != range_dock_)
            d->setFeatures(on ? QDockWidget::DockWidgetClosable : QDockWidget::DockWidgetClosable | QDockWidget::DockWidgetMovable | QDockWidget::DockWidgetFloatable);
    // The sliders keep their grab handles, which stay as padding but no longer drag while locked.
    for (QDockWidget* d : {time_dock_, range_dock_})
        if (d) d->setFeatures(on ? QDockWidget::DockWidgetVerticalTitleBar : SliderGrip::features);
}

QDockWidget* MainWindow::slider_dock(const QString& title, const QString& id) {
    auto* d = new QDockWidget(title, this);
    d->setObjectName(id);
    d->setTitleBarWidget(new SliderGrip(d));
    d->setFeatures(SliderGrip::features);
    d->setAllowedAreas(Qt::TopDockWidgetArea | Qt::BottomDockWidgetArea);
    // Pulled off, it opens half the window wide rather than squeezed to its least.
    connect(d, &QDockWidget::topLevelChanged, d, [this, d](bool floating) {
        if (floating) d->resize(qMax(d->width(), width() / 2), d->sizeHint().height());
    });
    return d;
}

// A saved window layout. One kept before the sliders were docks of their own (#272) has no place for them and is not
// restored: the window keeps the fresh layout it was built with. Restoring it and then taking the sliders out and docking
// them again, before the window was first shown, left the main window's layout holding a freed item, and Aztek crashed
// at start on Windows wherever such a layout was saved.
void MainWindow::restore_layout(const QByteArray& state) {
    QByteArray name;
    for (QChar c : QStringLiteral("TimeSlider")) name.append(char(c.unicode() >> 8)).append(char(c.unicode() & 0xff));
    if (!state.contains(name)) return;
    restoreState(state);
    // A layout keeps the corners it was saved with; the bottom right is the sliders' now.
    setCorner(Qt::BottomRightCorner, Qt::BottomDockWidgetArea);
}

// The slider docks go back under whatever a workspace docked below the views, as the reference keeps them.
void MainWindow::seat_slider_docks() {
    for (QDockWidget* d : {time_dock_, range_dock_}) {
        if (!d || d->isFloating() || dockWidgetArea(d) != Qt::BottomDockWidgetArea) continue;
        const bool shown = !d->isHidden();
        removeDockWidget(d);
        addDockWidget(Qt::BottomDockWidgetArea, d, Qt::Vertical);
        d->setVisible(shown);
    }
}

void MainWindow::toggle_side(bool left, QToolButton* handle) {
    const Qt::DockWidgetArea area = left ? Qt::LeftDockWidgetArea : Qt::RightDockWidgetArea;
    QList<QPointer<QWidget>>& folded = left ? folded_left_ : folded_right_;
    QList<QPointer<QWidget>>& raised = left ? raised_left_ : raised_right_;
    if (folded.isEmpty()) {
        // Fold: what is open on that side now is remembered and hidden, tabs behind others too (hiding the front tab
        // alone would bring the next one forward), and which tabs were in front.
        for (QDockWidget* d : findChildren<QDockWidget*>())
            if (dockWidgetArea(d) == area && !d->isHidden() && !d->isFloating()) {
                folded << d;
                if (d->isVisible()) raised << d;
            }
        if (left)
            for (QToolBar* t : findChildren<QToolBar*>())
                if (toolBarArea(t) == Qt::LeftToolBarArea && !t->isHidden()) folded << t;
        for (QWidget* w : folded) w->hide();
    } else {
        for (QWidget* w : folded)
            if (w) w->show();
        for (QWidget* w : raised)
            if (w) w->raise();
        folded.clear();
        raised.clear();
    }
    const bool now_folded = !folded.isEmpty();
    handle->setArrowType(left == now_folded ? Qt::RightArrow : Qt::LeftArrow);
    handle->setToolTip(now_folded ? (left ? "Bring back the toolbox and the left panels" : "Bring back the right panels")
                                  : (left ? "Collapse the toolbox and the left panels" : "Collapse the right panels"));
}

void MainWindow::build_docks() {
    outliner_dock_ = new QDockWidget("Outliner", this);
    outliner_dock_->setObjectName("outlinerDock");
    // A second Outliner under the first, closed until its edge is dragged up from the bottom, as the reference's has:
    // one part of a long hierarchy in each (#188).
    auto* outliners = new QSplitter(Qt::Vertical, outliner_dock_);
    auto* outliner = new Outliner(outliners);
    outliners->addWidget(outliner);
    outliners->addWidget(new Outliner(outliners));
    outliners->setCollapsible(0, false);
    outliners->setCollapsible(1, true);
    outliners->setHandleWidth(6);
    outliners->setSizes({1, 0});
    outliner_dock_->setWidget(outliners);
    ScriptUi::get()->add_builtin_editor("outlinerPanel1", "outlinerEditor", outliner);
    addDockWidget(Qt::LeftDockWidgetArea, outliner_dock_);
    resizeDocks({outliner_dock_}, {220}, Qt::Horizontal);

    channel_dock_ = new QDockWidget("Channel Box / Layer Editor", this);
    channel_dock_->setObjectName("channelDock");
    auto* split = new QSplitter(Qt::Vertical, channel_dock_);
    auto* channel_box = new ChannelBox(split);
    split->addWidget(channel_box);
    ScriptUi::get()->add_builtin_editor("mainChannelBox", "channelBox", channel_box);
    auto* layers = new QWidget(split);
    auto* ll = new QVBoxLayout(layers);
    ll->setContentsMargins(0, 0, 0, 0);
    auto* tabs = new QTabWidget(layers);
    tabs->setDocumentMode(true);
    tabs->addTab(new LayerEditor(tabs), "Display");
    tabs->addTab(new AnimLayerEditor(tabs), "Anim");
    tabs->setCurrentIndex(1);
    ll->addWidget(tabs);
    split->addWidget(layers);
    split->setStretchFactor(0, 3);
    split->setStretchFactor(1, 1);
    channel_dock_->setWidget(split);
    addDockWidget(Qt::RightDockWidgetArea, channel_dock_);

    attr_dock_ = new QDockWidget("Attribute Editor", this);
    attr_dock_->setObjectName("attrDock");
    attr_dock_->setWidget(new AttributeEditor(attr_dock_));
    addDockWidget(Qt::RightDockWidgetArea, attr_dock_);
    tabifyDockWidget(channel_dock_, attr_dock_);
    channel_dock_->raise();
    resizeDocks({channel_dock_}, {290}, Qt::Horizontal);
    keep_dock_widths();

    tool_dock_ = new QDockWidget("Tool Settings", this);
    tool_dock_->setObjectName("toolDock");
    tool_settings_ = new ToolSettings(tool_dock_);
    connect(tool_settings_, &ToolSettings::axisOrientation, this, [this](const QString& m) {
        for (ViewportWidget* v : panels_->views()) v->set("axisOrientation", "\"" + m + "\"");
    });
    connect(tool_settings_, &ToolSettings::stepSnap, this, [this](const QString& k, double x) {
        for (ViewportWidget* v : panels_->views()) v->set(k, QString::number(x));
    });
    connect(tool_settings_, &ToolSettings::pivotMode, this, [this](bool on) { set_pivot_mode(on); });
    connect(tool_settings_, &ToolSettings::curveDegree, this, [this](int d) {
        for (ViewportWidget* v : panels_->views()) v->set("curveDegree", QString::number(d));
    });
    tool_dock_->setWidget(tool_settings_);
    addDockWidget(Qt::LeftDockWidgetArea, tool_dock_);
    tabifyDockWidget(outliner_dock_, tool_dock_);
    tool_dock_->hide();
    // As the reference's default workspace: tabbed editors on the window's outer edges, the Outliner open on the left
    // of the views. A saved window state overrides both.
    setTabPosition(Qt::RightDockWidgetArea, QTabWidget::East);
    setTabPosition(Qt::LeftDockWidgetArea, QTabWidget::West);
    outliner_dock_->show();
    outliner_dock_->raise();
}

void MainWindow::build_bottom() {
    QToolBar* cl = new QToolBar("Command Line", this);
    cl->setObjectName("commandLine");
    cl->setMovable(false);
    addToolBar(Qt::BottomToolBarArea, cl);
    command_line_ = new CommandLine(cl);
    connect(command_line_, &CommandLine::openScriptEditor, this, &MainWindow::show_script_editor);
    cl->addWidget(command_line_);

    // The Time Slider and the Range Slider are docked controls of their own, TimeSlider and RangeSlider as the
    // reference names them: each has a grab handle down its left that keeps it off the window's edge, drags it above
    // or below the views, or pulls it off as a window, and a double-click on it docks it again (#272).
    time_dock_ = slider_dock("Time Slider", "TimeSlider");
    range_dock_ = slider_dock("Range Slider", "RangeSlider");
    addDockWidget(Qt::BottomDockWidgetArea, time_dock_);
    splitDockWidget(time_dock_, range_dock_, Qt::Vertical);
    lock_layout(QSettings().value("window/uiLocked", false).toBool());
    auto* time_row = new QWidget(time_dock_);
    time_row->setObjectName("timeSliderRow");
    auto* row1 = new QHBoxLayout(time_row);
    row1->setContentsMargins(0, 0, 0, 0);
    time_slider_ = new TimeSlider(time_row);
    auto* current = new QSpinBox(time_row);
    current_frame_ = current;
    current->setRange(-100000, 100000);
    current->setValue(1);
    current->setFixedWidth(64);
    connect(time_slider_, &TimeSlider::timeChanged, current, [current](double t) {
        current->blockSignals(true);
        current->setValue(int(t));
        current->blockSignals(false);
        exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{t}}});
        Host::get()->poke();
    });
    connect(current, &QSpinBox::valueChanged, time_slider_, [this](int t) { time_slider_->set_time(t); });
    connect(time_slider_, &TimeSlider::contextMenuAt, this, &MainWindow::time_slider_menu);
    connect(time_slider_, &TimeSlider::rangeMoved, this, [](double from, double to, double by) {
        QJsonObject call{{"edit", true}, {"time", QString("%1:%2").arg(from).arg(to)}, {"relative", true}, {"timeChange", by}};
        const QJsonValue curves = query("timeSliderCurves");
        if (curves.isArray()) call["args"] = curves;
        exec("keyframe", call);
    });
    row1->addWidget(time_slider_, 1);
    row1->addWidget(current);
    auto* play = new QToolBar(time_row);
    play->setObjectName("playback");
    play->setIconSize(QSize(16, 16));
    play_timer_ = new QTimer(this);
    play_timer_->setInterval(1000 / 24);
    connect(play_timer_, &QTimer::timeout, this, [this] {
        // Real time goes by the clock, dropping frames to keep up; every frame goes one at a time.
        double t;
        if (play_every_frame_) {
            t = time_slider_->time() + play_dir_;
        } else {
            double fr = ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{"framesPerSecond"}}}).toDouble(24);
            t = play_started_frame_ + play_dir_ * (QDateTime::currentMSecsSinceEpoch() - play_started_ms_) / 1000.0 * fr * play_speed_;
        }
        double a = time_slider_->start(), b = time_slider_->end();
        if (t > b || t < a) {
            if (loop_mode_ == "Once") {
                time_slider_->set_time(t > b ? b : a);
                stop_play();
                return;
            }
            if (loop_mode_ == "Oscillate") play_dir_ = -play_dir_;
            t = loop_mode_ == "Oscillate" ? (t > b ? b : a) : (t > b ? a : b);
            play_started_frame_ = t;
            play_started_ms_ = QDateTime::currentMSecsSinceEpoch();
        }
        time_slider_->set_time(t);
    });
    // The step and range buttons stop playback first (#199).
    play->addAction(icon("first"), "Go to start of playback range", [this] { go_to_time(time_slider_->start()); });
    play->addAction(icon("prev"), "Step back one frame", [this] { step_time(-1); });
    play->addAction(icon("prevKey"), "Step back one key", [this] { step_key("previous"); });
    play_back_ = play->addAction(icon("playBack"), "Play backwards", [this] { toggle_play(-1); });
    play_fwd_ = play->addAction(icon("play"), "Play forwards", [this] { toggle_play(1); });
    play_back_->setCheckable(true);
    play_fwd_->setCheckable(true);
    play->addAction(icon("nextKey"), "Step forward one key", [this] { step_key("next"); });
    play->addAction(icon("next"), "Step forward one frame", [this] { step_time(1); });
    play->addAction(icon("last"), "Go to end of playback range", [this] { go_to_time(time_slider_->end()); });
    row1->addWidget(play);
    // The sliders keep their own height, as the reference's do: a dock area left tall by an editor docked under the
    // views (the Rendering workspace's Script Editor) gives the space back rather than stretching them, and the edge
    // between them and the views doesn't drag (#312).
    time_row->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Fixed);
    time_dock_->setWidget(time_row);
    auto* range_row = new QWidget(range_dock_);
    range_row->setObjectName("rangeSliderRow");
    auto* row2 = new QHBoxLayout(range_row);
    row2->setContentsMargins(0, 0, 0, 0);
    auto field = [range_row](double val) {
        auto* s = new QSpinBox(range_row);
        s->setRange(-100000, 100000);
        s->setValue(int(val));
        s->setFixedWidth(64);
        return s;
    };
    anim_start_ = field(1), play_start_ = field(1), play_end_ = field(120), anim_end_ = field(200);
    range_slider_ = new RangeSlider(range_row);
    auto push = [this] {
        exec_quiet("playbackOptions", QJsonObject{{"animationStartTime", anim_start_->value()},
                                                  {"minTime", play_start_->value()},
                                                  {"maxTime", play_end_->value()},
                                                  {"animationEndTime", anim_end_->value()}});
        sync_time_range();
    };
    for (QSpinBox* sb : {anim_start_, play_start_, play_end_, anim_end_}) connect(sb, &QSpinBox::editingFinished, this, push);
    connect(range_slider_, &RangeSlider::rangeChanged, this, [this](double a, double b) {
        exec_quiet("playbackOptions", QJsonObject{{"minTime", a}, {"maxTime", b}});
        sync_time_range();
    });
    connect(range_slider_, &RangeSlider::toggleFullRange, this, [this] {
        double mn = play_start_->value(), mx = play_end_->value();
        bool full = mn == anim_start_->value() && mx == anim_end_->value();
        if (full && last_full_max_ > last_full_min_) {
            exec_quiet("playbackOptions", QJsonObject{{"minTime", last_full_min_}, {"maxTime", last_full_max_}});
        } else {
            last_full_min_ = mn;
            last_full_max_ = mx;
            exec_quiet("playbackOptions", QJsonObject{{"minTime", anim_start_->value()}, {"maxTime", anim_end_->value()}});
        }
        sync_time_range();
    });
    row2->addWidget(anim_start_);
    row2->addWidget(play_start_);
    row2->addWidget(range_slider_, 1);
    // The bookmark icon: a click makes one, a right-click has the manager and framing.
    auto* bookmark = new QToolButton(range_row);
    bookmark->setText(QString::fromUtf8("\u2691"));
    bookmark->setToolTip("Create Bookmark (Alt+T); right-click for the Bookmark Manager");
    bookmark->setAutoRaise(true);
    bookmark->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(bookmark, &QToolButton::clicked, this, [this] { create_bookmark(false); });
    connect(bookmark, &QToolButton::customContextMenuRequested, this, [this, bookmark](const QPoint& at) {
        QMenu m;
        m.addAction("Create Bookmark\tAlt+T", [this] { create_bookmark(false); });
        m.addAction("Bookmark Manager", [this] { bookmark_manager(); });
        m.addSeparator();
        m.addAction("Frame Bookmark", [this] { frame_bookmark(0); });
        m.addAction("Frame Previous Bookmark", [this] { frame_bookmark(-1); });
        m.addAction("Frame Next Bookmark", [this] { frame_bookmark(1); });
        m.exec(bookmark->mapToGlobal(at));
    });
    row2->addWidget(bookmark);
    auto* bm_create = new QShortcut(QKeySequence("Alt+T"), this);
    connect(bm_create, &QShortcut::activated, this, [this] { create_bookmark(false); });
    auto* bm_quick = new QShortcut(QKeySequence("Alt+Shift+T"), this);
    connect(bm_quick, &QShortcut::activated, this, [this] { create_bookmark(true); });
    connect(time_slider_, &TimeSlider::bookmarkMoved, this, [](const QString& node, double a, double b) {
        mel_action(QString("setAttr %1.timeRangeStart %2; setAttr %1.timeRangeStop %3;").arg(node).arg(a).arg(b));
        Host::get()->poke();
    });
    row2->addWidget(play_end_);
    row2->addWidget(anim_end_);
    // Time units by name, as the Settings have them.
    auto* fps = new QComboBox(range_row);
    fps_box_ = fps;
    fps->setToolTip("Time unit");
    struct U { const char* label; double rate; };
    for (U u : {U{"Film (24 fps)", 24}, U{"PAL (25 fps)", 25}, U{"NTSC (30 fps)", 30}, U{"Show (48 fps)", 48}, U{"PAL Field (50 fps)", 50}, U{"NTSC Field (60 fps)", 60}}) fps->addItem(u.label, u.rate);
    connect(fps, &QComboBox::activated, this, [fps](int i) {
        exec("playbackOptions", QJsonObject{{"framesPerSecond", fps->itemData(i).toDouble()}});
    });
    row2->addWidget(fps);
    auto* speed = new QComboBox(range_row);
    speed_box_ = speed;
    speed->setToolTip("Playback speed");
    speed->addItems({"Play every frame", "Real-time", "Half", "Twice"});
    speed->setCurrentText(QSettings().value("prefs/playbackSpeed", "Real-time").toString());
    connect(speed, &QComboBox::activated, this, [this, speed](int) {
        QSettings().setValue("prefs/playbackSpeed", speed->currentText());
        apply_prefs();
    });
    row2->addWidget(speed);
    auto* loop = new QComboBox(range_row);
    loop->setToolTip("Looping");
    loop->addItems({"Once", "Oscillate", "Continuous"});
    loop->setCurrentText(QSettings().value("prefs/playbackLoop", "Continuous").toString());
    connect(loop, &QComboBox::activated, this, [this, loop](int) {
        QSettings().setValue("prefs/playbackLoop", loop->currentText());
        loop_mode_ = loop->currentText();
    });
    row2->addWidget(loop);
    auto* tb = new QToolBar(range_row);
    tb->setIconSize(QSize(16, 16));
    auto_key_ = tb->addAction(icon("autokey"), "Auto keyframe toggle", [this](bool on) {
        exec("autoKeyframe", QJsonObject{{"state", on}});
    });
    auto_key_->setCheckable(true);
    auto_key_->setObjectName("autoKey");
    row2->addWidget(tb);
    range_row->setSizePolicy(QSizePolicy::Preferred, QSizePolicy::Fixed);
    range_dock_->setWidget(range_row);

    help_line_ = new QLabel(this);
    statusBar()->addWidget(help_line_, 1);
    statusBar()->setObjectName("helpLine");
    statusBar()->setSizeGripEnabled(false);
}


// ---------------------------------------------------------------------------------------------------------------------
// State

void MainWindow::toggle_play(int dir) {
    if (play_timer_->isActive() && play_dir_ == dir) {
        stop_play();
        return;
    }
    play_dir_ = dir;
    ask("play", QJsonObject{{"state", true}, {"forward", dir > 0}});
    double fps = ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{"framesPerSecond"}}}).toDouble(24);
    play_timer_->setInterval(int(1000 / qMax(1.0, fps * play_speed_)));
    play_started_ms_ = QDateTime::currentMSecsSinceEpoch();
    play_started_frame_ = time_slider_->time();
    play_timer_->start();
    show_play_state();
}

void MainWindow::stop_play() {
    play_timer_->stop();
    ask("play", QJsonObject{{"state", false}});
    show_play_state();
}

void MainWindow::show_play_state() {
    const bool on = play_timer_ && play_timer_->isActive();
    for (auto [a, dir, name, tip] : {std::tuple{play_fwd_, 1, "play", "Play forwards"}, std::tuple{play_back_, -1, "playBack", "Play backwards"}}) {
        if (!a) continue;
        const bool playing = on && play_dir_ == dir;
        a->setChecked(playing);
        a->setIcon(icon(playing ? "pause" : name));
        a->setText(playing ? "Stop playback" : tip);
        a->setToolTip(a->text());
    }
}

void MainWindow::sync_time_range() {
    if (fps_box_) {
        double fr = ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{"framesPerSecond"}}}).toDouble(24);
        int i = fps_box_->findData(fr);
        if (i >= 0) fps_box_->setCurrentIndex(i);
    }
    auto q = [](const char* flag) {
        return ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}).toDouble();
    };
    double ast = q("animationStartTime"), mn = q("minTime"), mx = q("maxTime"), aet = q("animationEndTime");
    for (auto [sb, val] : {std::pair{anim_start_, ast}, {play_start_, mn}, {play_end_, mx}, {anim_end_, aet}}) {
        if (sb->hasFocus()) continue;
        sb->blockSignals(true);
        sb->setValue(int(val));
        sb->blockSignals(false);
    }
    range_slider_->set(ast, mn, mx, aet);
    time_slider_->set_range(mn, mx);
}

void MainWindow::refresh_state() {
    QJsonObject st = state();
    for (const QJsonValue& r : query("uiRequests").toArray()) {
        QString n = r.toString();
        handle_ui_request(n);
    }
    sync_panels();
    // A script may have started or stopped playback with the play command.
    bool script_plays = ask("play", QJsonObject{{"query", true}, {"queried", QJsonArray{"state"}}}).toBool();
    if (script_plays && !play_timer_->isActive())
        toggle_play(ask("play", QJsonObject{{"query", true}, {"queried", QJsonArray{"forward"}}}).toBool(true) ? 1 : -1);
    else if (!script_plays && play_timer_->isActive())
        play_timer_->stop();
    // The time: scripts may have moved it, changed the range or set keys.
    double now = ask("currentTime", QJsonObject{{"query", true}}).toDouble(1);
    sync_time_range();
    if (now != time_slider_->time() && !play_timer_->isActive()) {
        time_slider_->blockSignals(true);
        time_slider_->set_time(now);
        time_slider_->blockSignals(false);
        current_frame_->blockSignals(true);
        current_frame_->setValue(int(now));
        current_frame_->blockSignals(false);
    }
    QJsonValue keys = ask("keyTimes");
    QVector<double> kt;
    for (const QJsonValue& k : keys.toArray()) kt.append(k.toDouble());
    time_slider_->set_keys(kt);
    {
        QVector<TimeSlider::Bookmark> bms;
        for (const QJsonValue& v : query("bookmarks").toArray()) {
            const QJsonObject o = v.toObject();
            const QJsonArray c = o["color"].toArray();
            bms.append({o["node"].toString(), o["name"].toString(), o["start"].toDouble(), o["stop"].toDouble(),
                        QColor::fromRgbF(float(c[0].toDouble()), float(c[1].toDouble()), float(c[2].toDouble()))});
        }
        time_slider_->set_bookmarks(bms);
    }
    QVector<double> bd;
    if (!kt.isEmpty())
        for (const QJsonValue& k : ask("keyframe", QJsonObject{{"query", true}, {"queried", QJsonArray{"breakdown"}}}).toArray()) bd.append(k.toDouble());
    time_slider_->set_breakdowns(bd);
    auto_key_->blockSignals(true);
    auto_key_->setChecked(ask("autoKeyframe", QJsonObject{{"query", true}}).toBool());
    auto_key_->blockSignals(false);
    QString mode = st["mode"].toString();
    if (mode_buttons_.contains(mode)) {
        const QSignalBlocker block(mode_buttons_[mode]);
        mode_buttons_[mode]->setChecked(true);
    }
    history_button_->setChecked(st["history"].toBool());
    sync_status_line();
    update_title();
}

/// Notes the scene's state as a menu opens, so that what a menu item did can be told when it's triggered.
class MenuWatcher : public QObject {
public:
    MenuWatcher(double* mark, std::function<double()> now, QObject* parent) : QObject(parent), mark_(mark), now_(std::move(now)) {}
    bool eventFilter(QObject* o, QEvent* e) override {
        if (e->type() == QEvent::Show && qobject_cast<QMenu*>(o)) *mark_ = now_();
        return false;
    }

private:
    double* mark_;
    std::function<double()> now_;
};

void MainWindow::watch_menus() {
    qApp->installEventFilter(new MenuWatcher(&menu_mark_, [this] { return state()["undoId"].toDouble(); }, this));
    connect(menuBar(), &QMenuBar::triggered, this, [this](QAction* a) {
        QString text = a->text().section('\t', 0, 0);
        if (text.startsWith("Repeat") || text.startsWith("Undo") || text.startsWith("Redo") || text == last_repeat_label_) return;
        QPointer<QAction> held(a);
        note_repeat(text, [held] { if (held) held->trigger(); }, menu_mark_);
    });
}

void MainWindow::update_title() {
    QString file = state()["file"].toString();
    // A scene saved or opened some other way, by a script say, joins the recent files too.
    if (file != titled_file_) {
        titled_file_ = file;
        add_recent_file(file);
    }
    setWindowTitle(QString("Aztek - %1%2").arg(file.isEmpty() ? "untitled" : file, state()["modified"].toBool() ? "*" : ""));
}

ViewportWidget* MainWindow::active_view() const {
    return panels_->active();
}

void MainWindow::set_tool(const QString& tool) {
    // Choosing a tool leaves Edit Pivot, as the reference does: W in pivot mode is the Move tool again, on the object.
    if (pivot_mode_) set_pivot_mode(false);
    if (tool != tool_) last_tool_ = tool_;
    // G goes back to the last tool, other than the four everyone's a key away from.
    if (!repeating_ && !QStringList{"select", "move", "rotate", "scale"}.contains(tool)) {
        last_repeat_ = [this, tool] { set_tool(tool); };
        last_repeat_label_ = tool;
    }
    tool_ = tool;
    if (tool_buttons_.contains(tool)) tool_buttons_[tool]->setChecked(true);
    // Tools that place points in the view show the reference's cross-hair.
    const bool cross = tool == "joint" || tool == "multiCut";
    for (ViewportWidget* v : panels_->views()) {
        v->set("tool", "\"" + tool + "\"");
        if (cross) v->setCursor(Qt::CrossCursor);
        else v->unsetCursor();
    }
    query("uiToolChanged", tool);
    if (tool_settings_) tool_settings_->set_tool(tool);
    mtk_follow_tool(tool);
    // The tool brings the node's numbers with it; any other puts them away.
    bool off = QSettings().value("prefs/inViewEditorOff", false).toBool();
    if (tool == "showManip" && !off) QTimer::singleShot(0, this, [this] { in_view_->show_last(); });
    else if (tool != "showManip") in_view_->hide();
    static const QHash<QString, QString> tool_help{
        {"showManip", "Show Manipulator Tool: drag a handle to change the node's attribute. Middle-drag takes the highlighted channel's handle."},
        {"lasso", "Lasso Tool: drag round what to select. Shift toggles, Ctrl deselects, Ctrl+Shift adds."},
        {"paintSelect", "Paint Selection Tool: drag over components to select them; Ctrl deselects."},
        {"shortestPath", "Shortest Edge Path Tool: click a component, then another: the path between them is selected."},
        {"camTumble", "Tumble Tool: drag to tumble the view."},
        {"camTrack", "Track Tool: drag to track the view."},
        {"camDolly", "Dolly Tool: drag to dolly the view."},
        {"camZoom", "Zoom Tool: drag to change the lens."},
        {"camRoll", "Roll Tool: drag to roll the view."},
        {"camYawPitch", "Yaw-Pitch Tool: drag to turn the camera where it stands."},
        {"sculpt", "Sculpting: drag to sculpt, Ctrl inverts, Shift smooths, B-drag sizes the brush."},
        {"camBoxDolly", "Box Dolly Tool: drag a box left to right to dolly in on it, right to left to dolly out."},
        {"camWalk", "Walk Tool: left drag looks round, middle drag walks: up and down forward and back, sideways to the side."},
        {"modelTime", "Time Drag: drag in the view to change the current time."},
        {"curveAddPt", "Add Points Tool: click to add points to the end of the selected curve, Enter adds them."},
        {"curveBezier", "Bezier Curve Tool: click for a sharp point, drag out for a smooth one. Backspace takes the last back, Enter makes the curve."},
        {"arcTwo", "Two Point Arc: click the two ends."},
        {"arcThree", "Three Point Arc: click the start, a point on the arc, and the end."},
        {"mergeEdge", "Merge Edge Tool: click two border edges of a mesh."},
        {"mergeFacet", "Merge Face Tool: click two faces of different shells."},
        {"createCube", "Polygon Cube: drag on the grid for the base, move for the height, click to finish."},
        {"createPlane", "Polygon Plane: drag on the grid."},
        {"createSphere", "Polygon Sphere: drag on the grid for the radius."},
        {"createCylinder", "Polygon Cylinder: drag on the grid for the radius, move for the height, click to finish."},
        {"createCone", "Polygon Cone: drag on the grid for the radius, move for the height, click to finish."},
        {"createTorus", "Polygon Torus: drag on the grid for the radius."},
        {"snapTogether", "Snap Together Tool: click a point on the object to move, then the point it goes to."},
        {"insertJoint", "Insert Joint: drag from a joint to add a joint along the bone."},
        {"ikHandle", "IK Handle Tool: click the start joint, then the end joint."},
        {"ikSplineHandle", "IK Spline Handle Tool: click the start joint, then the end joint. With Auto create curve off, then click the curve."},
        {"multiCut", "Multi-Cut: click to place cut points, Shift snaps along edges by the snap step, Ctrl+Shift squares to the last edge, Ctrl-click inserts a loop (Ctrl+middle centred), middle-drag slices, Backspace takes the last back, Enter cuts."},
        {"insertLoop", "Insert Edge Loop: hover an edge to see the loop, click to insert it."},
        {"offsetLoop", "Offset Edge Loop: drag from an edge to add loops either side."},
        {"targetWeld", "Target Weld: drag from a vertex or edge onto another."},
        {"slideEdge", "Slide Edge: pick edges, then middle-drag to slide them."},
        {"crease", "Crease: pick edges, then middle-drag to set their crease."},
        {"createPolygon", "Create Polygon: click to place points, Enter makes the face."},
        {"appendPolygon", "Append to Polygon: click border edges and points, Enter makes the face."},
        {"cutSewUv", "3D Cut and Sew UV: drag over edges to cut their UVs, Ctrl-drag to sew them."},
        {"quadDraw", "Quad Draw: click places dots, Shift-click fills a quad, Shift-drag relaxes, Ctrl-click inserts a loop (Ctrl+middle centred), Ctrl+Shift-click deletes a face or an edge's loop, Tab-drag draws a quad strip, extends a border edge or pulls a quad out of a border vertex, left or middle drag moves a vertex, edge or face."},
    };
    if (tool_help.contains(tool))
        help_line_->setText(tool_help[tool]);
    else if (tool == "curveCv" || tool == "curveEp")
        help_line_->setText("Curve Tool: click to place points, Backspace takes the last back, Enter makes the curve. Hold X to snap to the grid.");
    else if (tool == "paintAttr")
        help_line_->setText("Paint Attribute Tool: drag to paint, Ctrl paints the inverse, Shift smooths, B-drag sizes the brush.");
    else if (tool == "editMembership")
        help_line_->setText("Edit Membership Tool: pick a deformer in the Tool Settings, select components, then add them or take them out.");
    else if (tool == "paintSkin")
        help_line_->setText("Paint Skin Weights Tool: drag to paint, Ctrl paints the inverse, Shift smooths, B-drag sizes the brush.");
    else if (tool == "joint")
        help_line_->setText("Joint Tool: click to place joints, each under the last. Enter ends the chain.");
    else
        help_line_->setText(QString("%1 Tool: drag a handle; middle-drag anywhere moves the active handle.").arg(tool.left(1).toUpper() + tool.mid(1)));
}

void MainWindow::set_mode(const QString& mode) {
    exec("selectMode", args(QJsonArray{mode}));
}

void MainWindow::repeat_last() {
    if (!last_repeat_) return;
    // Copied: what runs may be what's replaced.
    auto fn = last_repeat_;
    repeating_ = true;
    fn();
    repeating_ = false;
}

void MainWindow::run_repeatable(const QString& label, const std::function<void()>& fn) {
    double before = state()["undoId"].toDouble();
    fn();
    note_repeat(label, fn, before);
}

void MainWindow::note_repeat(const QString& label, const std::function<void()>& fn, double undo_before) {
    if (repeating_) return;
    QJsonObject now = state();
    if (now["undoId"].toDouble() == undo_before) return;
    QString what = now["undoLabel"].toString();
    for (const char* no : {"select", "hilite", "undo", "redo", "currentTime"})
        if (what.startsWith(no)) return;
    last_repeat_ = fn;
    last_repeat_label_ = label;
}

namespace {
// Files a drop brings in: scenes and the formats the file command imports.
bool droppable(const QString& f) {
    static const QStringList ext = {"ma", "mb", "obj", "fbx", "gltf", "glb", "abc", "ply", "stl"};
    return is_usd_file(f) || ext.contains(QFileInfo(f).suffix().toLower());
}
} // namespace

void MainWindow::dragEnterEvent(QDragEnterEvent* e) {
    if (e->mimeData()->hasUrls()) {
        // With performFileDropAction redefined, any file is offered to it.
        const QString owner = ask("whatIs", QJsonObject{{"args", QJsonArray{"performFileDropAction"}}}).toString();
        if (owner.startsWith("Mel procedure") && !owner.contains("performFileDropAction.mel")) {
            e->acceptProposedAction();
            return;
        }
        for (const QUrl& u : e->mimeData()->urls()) {
            if (droppable(u.toLocalFile())) {
                e->acceptProposedAction();
                return;
            }
        }
    }
}

// A dropped file is imported, as the reference does, with the import options last used for its type; a USD file
// becomes a stage, as the reference's USD plug-in makes it.
void MainWindow::dropEvent(QDropEvent* e) {
    // A tool that redefined performFileDropAction (mGear's guide templates and skin packs) takes the files first, as
    // the reference hands them to it; Aztek's own definition is the import below.
    const QString owner = ask("whatIs", QJsonObject{{"args", QJsonArray{"performFileDropAction"}}}).toString();
    const bool redefined = owner.startsWith("Mel procedure") && !owner.contains("performFileDropAction.mel");
    for (const QUrl& u : e->mimeData()->urls()) {
        const QString f = u.toLocalFile();
        if (f.isEmpty()) continue;
        if (redefined) {
            QString quoted = f;
            quoted.replace("\\", "/").replace("\"", "\\\"");
            mel("performFileDropAction \"" + quoted + "\";");
            continue;
        }
        // Read as File > Import reads it, by the import options kept for its type (#110).
        if (droppable(f)) import_file(f);
    }
    e->acceptProposedAction();
    Host::get()->poke();
}

bool MainWindow::confirm_unsaved() {
    if (no_prompts_ || !state()["modified"].toBool()) return true;
    QString file = state()["file"].toString();
    QMessageBox box(QMessageBox::Warning, "Save Changes", file.isEmpty() ? "Save changes to the untitled scene?" : QString("Save changes to %1?").arg(file), QMessageBox::NoButton, this);
    QPushButton* save = box.addButton("Save", QMessageBox::AcceptRole);
    QPushButton* discard = box.addButton("Don't Save", QMessageBox::DestructiveRole);
    box.addButton("Cancel", QMessageBox::RejectRole);
    box.setDefaultButton(save);
    box.exec();
    if (box.clickedButton() == discard) return true;
    if (box.clickedButton() == save) return save_scene(false);
    return false;
}

void MainWindow::new_scene() {
    if (!confirm_unsaved()) return;
    aztek_free(aztek_file("new", ""));
    Host::get()->poke();
    update_title();
}

void MainWindow::open_scene() {
    if (!confirm_unsaved()) return;
    // The reference's scene types, then the plug-in types that open a file while their plug-ins are loaded.
    auto plugin_loaded = [](const char* name) { return ask("pluginInfo", QJsonObject{{"args", QJsonArray{name}}, {"query", true}, {"loaded", true}}).toBool(); };
    QString filter = "Maya Scenes (*.ma *.mb);;mayaAscii (*.ma);;mayaBinary (*.mb)";
    if (plugin_loaded("fbx")) filter += ";;FBX (*.fbx)";
    if (plugin_loaded("objExport")) filter += ";;OBJ (*.obj)";
    if (plugin_loaded("mayaUsdPlugin")) filter += ";;USD Import (*.usd *.usda *.usdc *.usdz)";
    filter += ";;All Files (*)";
    QString f = QFileDialog::getOpenFileName(this, "Open Scene", QString(), filter);
    if (!f.isEmpty()) open_file(f);
}

void MainWindow::open_file(const QString& path) {
    QString err = take(aztek_file("open", path.toUtf8().constData()));
    Host::get()->poke();
    if (err.isEmpty()) add_recent_file(path);
    update_title();
}

bool MainWindow::save_scene(bool as) {
    QString f = state()["file"].toString();
    // A scene that came from another format (OBJ, FBX, USD) is saved as a scene file under a name asked for.
    if (!f.isEmpty() && !f.endsWith(".ma", Qt::CaseInsensitive) && !f.endsWith(".mb", Qt::CaseInsensitive)) as = true;
    if (as || f.isEmpty()) {
        // The reference's two scene types; mayaAscii is its default.
        QString filter = f.endsWith(".mb", Qt::CaseInsensitive) ? "mayaBinary (*.mb)" : "mayaAscii (*.ma)";
        f = QFileDialog::getSaveFileName(this, "Save Scene As", f.isEmpty() ? "untitled.ma" : f, "mayaAscii (*.ma);;mayaBinary (*.mb)", &filter);
        if (f.isEmpty()) return false;
        if (!f.section('/', -1).contains('.')) {
            f += filter.contains("*.mb") ? ".mb" : ".ma";
            // The dialog only asked about the name as typed.
            if (QFile::exists(f) && QMessageBox::warning(this, "Save Scene As", QString("%1 already exists. Do you want to replace it?").arg(f), QMessageBox::Yes | QMessageBox::No, QMessageBox::No) != QMessageBox::Yes) return false;
        }
    }
    if (!no_prompts_ && !usd_confirm_dirty_layers(this)) return false;
    QString err = take(aztek_file("save", f.toUtf8().constData()));
    Host::get()->poke();
    if (err.isEmpty()) add_recent_file(f);
    update_title();
    if (!err.isEmpty()) {
        QMessageBox::critical(this, "Save Scene", err);
        return false;
    }
    return true;
}

void MainWindow::show_script_editor() {
    if (!script_window_) {
        script_window_ = new QWidget(this, editor_window_flags());
        script_window_->setWindowTitle("Script Editor");
        auto* l = new QVBoxLayout(script_window_);
        l->setContentsMargins(0, 0, 0, 0);
        script_editor_ = new ScriptEditor(script_window_);
        l->addWidget(script_editor_);
        script_window_->resize(760, 560);
        // Everything so far, kept while the editor wasn't open.
        script_editor_->append(log_before_editor_);
        log_before_editor_ = QJsonArray();
    }
    script_window_->show();
    script_window_->raise();
    script_window_->activateWindow();
}

void MainWindow::show_node_editor() {
    if (!node_window_) {
        node_window_ = new QWidget(this, editor_window_flags());
        node_window_->setWindowTitle("Node Editor");
        auto* l = new QVBoxLayout(node_window_);
        l->setContentsMargins(0, 0, 0, 0);
        l->setSpacing(0);
        auto* bar = new QToolBar(node_window_);
        auto* ed = new NodeEditor(node_window_);
        bar->addAction("Input and Output Connections", [ed] { ed->graph_selection(1); });
        bar->addAction("Input Connections Deep", [ed] { ed->graph_selection(4); });
        bar->addAction("Add Selected", [ed] { ed->add_selected(); });
        bar->addAction("Clear", [ed] { ed->clear_graph(); });
        l->addWidget(bar);
        l->addWidget(ed, 1);
        node_window_->resize(1000, 640);
    }
    float_dockable("nodeEditor", node_window_, "Node Editor");
    if (auto* ed = node_window_->findChild<NodeEditor*>()) ed->graph_selection(1);
}

void MainWindow::show_dope_sheet() {
    if (!dope_window_) {
        dope_window_ = new QWidget(this, editor_window_flags());
        dope_window_->setWindowTitle("Dope Sheet");
        auto* l = new QVBoxLayout(dope_window_);
        l->setContentsMargins(0, 0, 0, 0);
        l->addWidget(new DopeSheet(dope_window_));
        dope_window_->resize(1000, 460);
    }
    dope_window_->show();
    dope_window_->raise();
    dope_window_->activateWindow();
}

void MainWindow::show_set_driven_key() {
    if (!sdk_window_) sdk_window_ = new SetDrivenKeyWindow(this);
    sdk_window_->show();
    sdk_window_->raise();
    sdk_window_->activateWindow();
}

void MainWindow::create_bookmark(bool quick) {
    const auto [a, b] = time_slider_->picked_range();
    QString name;
    QColor color = QColor::fromHsv(QRandomGenerator::global()->bounded(360), 180, 230);
    double start = a, stop = b;
    if (!quick) {
        QDialog d(this);
        d.setWindowTitle("Create Bookmark");
        auto* f = new QFormLayout(&d);
        auto* n = new QLineEdit(&d);
        auto* s0 = new QDoubleSpinBox(&d);
        auto* s1 = new QDoubleSpinBox(&d);
        for (QDoubleSpinBox* x : {s0, s1}) {
            x->setRange(-1e6, 1e6);
            x->setDecimals(0);
        }
        s0->setValue(a);
        s1->setValue(b);
        auto* pick = new QPushButton(&d);
        auto paint = [pick](const QColor& c) { pick->setStyleSheet(QString("background: %1; min-width: 60px;").arg(c.name())); };
        paint(color);
        connect(pick, &QPushButton::clicked, &d, [&color, &d, paint] {
            const QColor c = QColorDialog::getColor(color, &d, "Bookmark Colour");
            if (c.isValid()) {
                color = c;
                paint(c);
            }
        });
        f->addRow("Name:", n);
        f->addRow("Color:", pick);
        f->addRow("Start:", s0);
        f->addRow("End:", s1);
        auto* bb = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel, &d);
        bb->button(QDialogButtonBox::Ok)->setText("Create");
        connect(bb, &QDialogButtonBox::accepted, &d, &QDialog::accept);
        connect(bb, &QDialogButtonBox::rejected, &d, &QDialog::reject);
        f->addRow(bb);
        if (d.exec() != QDialog::Accepted) return;
        name = n->text();
        start = std::min(s0->value(), s1->value());
        stop = std::max(s0->value(), s1->value());
    }
    QString esc = name;
    esc.replace("\\", "\\\\").replace("\"", "\\\"");
    mel_action(QString("{ string $b = `createNode timeSliderBookmark`; setAttr ($b + \".name\") -type \"string\" \"%1\"; "
                       "setAttr ($b + \".timeRangeStart\") %2; setAttr ($b + \".timeRangeStop\") %3; setAttr ($b + \".color\") %4 %5 %6; }")
                   .arg(esc).arg(start).arg(stop).arg(color.redF()).arg(color.greenF()).arg(color.blueF()));
    Host::get()->poke();
}

void MainWindow::frame_bookmark(int dir) {
    const auto& bms = time_slider_->bookmarks();
    if (bms.isEmpty()) return;
    const double t = time_slider_->time();
    const TimeSlider::Bookmark* pick = nullptr;
    for (const auto& b : bms) {
        if (dir == 0 && t >= b.start && t <= b.stop && (!pick || b.start > pick->start)) pick = &b;
        if (dir < 0 && b.stop < t && (!pick || b.stop > pick->stop)) pick = &b;
        if (dir > 0 && b.start > t && (!pick || b.start < pick->start)) pick = &b;
    }
    if (!pick && dir == 0) {
        // None at the current frame: the nearest.
        for (const auto& b : bms)
            if (!pick || std::abs(b.start - t) < std::abs(pick->start - t)) pick = &b;
    }
    if (!pick) return;
    exec_quiet("playbackOptions", QJsonObject{{"minTime", pick->start}, {"maxTime", pick->stop}});
    sync_time_range();
    time_slider_->set_time(pick->start);
}

void MainWindow::bookmark_manager() {
    auto* d = new QDialog(this);
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("Bookmark Manager");
    auto* v = new QVBoxLayout(d);
    auto* bar = new QToolBar(d);
    auto* table = new QTableWidget(0, 6, d);
    table->setHorizontalHeaderLabels({"", "Name", "Start", "End", "", ""});
    table->horizontalHeader()->setSectionResizeMode(1, QHeaderView::Stretch);
    table->verticalHeader()->hide();
    v->addWidget(bar);
    v->addWidget(table, 1);
    std::function<void()> fill;
    auto set = [](const QString& node, const QString& attr, const QString& value) {
        mel_action(QString("setAttr %1.%2 %3;").arg(node, attr, value));
        Host::get()->poke();
    };
    fill = [this, table, set, &fill, d]() {
        Q_UNUSED(d);
        QSignalBlocker hold(table);
        const auto bms = time_slider_->bookmarks();
        table->setRowCount(bms.size());
        for (int r = 0; r < bms.size(); ++r) {
            const auto& b = bms[r];
            auto* sw = new QPushButton(table);
            sw->setStyleSheet(QString("background: %1;").arg(b.color.name()));
            sw->setToolTip("Its colour");
            connect(sw, &QPushButton::clicked, table, [this, b, set] {
                const QColor c = QColorDialog::getColor(b.color, this, "Bookmark Colour");
                if (c.isValid()) set(b.node, "color", QString("%1 %2 %3").arg(c.redF()).arg(c.greenF()).arg(c.blueF()));
            });
            table->setCellWidget(r, 0, sw);
            auto* item = new QTableWidgetItem(b.name);
            item->setData(Qt::UserRole, b.node);
            table->setItem(r, 1, item);
            table->setItem(r, 2, new QTableWidgetItem(QString::number(b.start)));
            table->setItem(r, 3, new QTableWidgetItem(QString::number(b.stop)));
            auto* upd = new QPushButton("Use Selection", table);
            upd->setToolTip("Update the bookmark to the frames picked on the Time Slider");
            connect(upd, &QPushButton::clicked, table, [this, b] {
                const auto [a, z] = time_slider_->picked_range();
                mel_action(QString("setAttr %1.timeRangeStart %2; setAttr %1.timeRangeStop %3;").arg(b.node).arg(a).arg(z));
                Host::get()->poke();
            });
            table->setCellWidget(r, 4, upd);
            auto* del = new QPushButton("Delete", table);
            connect(del, &QPushButton::clicked, table, [b] {
                mel_action(QString("delete %1;").arg(b.node));
                Host::get()->poke();
            });
            table->setCellWidget(r, 5, del);
        }
    };
    connect(table, &QTableWidget::itemChanged, d, [table, set](QTableWidgetItem* it) {
        const QString node = table->item(it->row(), 1)->data(Qt::UserRole).toString();
        if (it->column() == 1) {
            QString t = it->text();
            t.replace("\\", "\\\\").replace("\"", "\\\"");
            set(node, "name", QString("-type \"string\" \"%1\"").arg(t));
        } else if (it->column() == 2) {
            set(node, "timeRangeStart", QString::number(it->text().toDouble()));
        } else if (it->column() == 3) {
            set(node, "timeRangeStop", QString::number(it->text().toDouble()));
        }
    });
    bar->addAction("Create", [this] { create_bookmark(false); });
    QAction* shown = bar->addAction("Show Bookmarks", [this](bool on) {
        time_slider_->bookmarks_shown = on;
        time_slider_->update();
    });
    shown->setCheckable(true);
    shown->setChecked(time_slider_->bookmarks_shown);
    bar->addAction("Frame Bookmark", [this, table] {
        const int r = table->currentRow();
        const auto& bms = time_slider_->bookmarks();
        if (r < 0 || r >= bms.size()) return frame_bookmark(0);
        exec_quiet("playbackOptions", QJsonObject{{"minTime", bms[r].start}, {"maxTime", bms[r].stop}});
        sync_time_range();
    });
    bar->addAction("Delete All", [this] {
        QStringList nodes;
        for (const auto& b : time_slider_->bookmarks()) nodes << b.node;
        if (!nodes.isEmpty()) mel_action("delete " + nodes.join(" ") + ";");
        Host::get()->poke();
    });
    // The list follows the scene.
    auto* keep = new std::function<void()>(fill);
    connect(d, &QObject::destroyed, [keep] { delete keep; });
    connect(Host::get(), &Host::sceneChanged, d, [keep] { (*keep)(); });
    (*keep)();
    d->resize(560, 300);
    d->show();
}

void MainWindow::time_slider_menu(QPoint global) {
    QMenu m;
    QString range = time_slider_->has_range() ? time_slider_->range_text() : QString::number(time_slider_->time());
    auto keyed = [range](const QString& command, QJsonObject call = QJsonObject()) {
        call["time"] = range;
        return [command, call] { exec(command, call); };
    };
    m.addAction("Cut", keyed("cutKey"));
    m.addAction("Copy", keyed("copyKey"));
    m.addAction("Paste", [this] { exec("pasteKey", QJsonObject{{"time", time_slider_->time()}, {"option", "merge"}}); });
    m.addAction("Delete", keyed("cutKey", QJsonObject{{"clear", true}}));
    m.addSeparator();
    m.addAction("Snap", keyed("snapKey"));
    m.addAction("Convert to Breakdown", keyed("keyframe", QJsonObject{{"edit", true}, {"breakdown", true}}));
    m.addAction("Convert to Key", keyed("keyframe", QJsonObject{{"edit", true}, {"breakdown", false}}));
    m.addSeparator();
    auto* weight = m.addMenu("Display Key Ticks")->addAction("Show Layer Weight");
    weight->setCheckable(true);
    weight->setChecked(ask("timeControl", QJsonObject{{"query", true}, {"animLayerShowWeight", true}}).toBool());
    connect(weight, &QAction::toggled, this, [](bool on) {
        exec("timeControl", QJsonObject{{"edit", true}, {"animLayerShowWeight", on}});
    });
    QMenu* tan = m.addMenu("Tangents");
    for (const char* t : {"auto", "spline", "clamped", "linear", "flat", "step", "plateau"}) {
        QString label = QString(t);
        label[0] = label[0].toUpper();
        tan->addAction(label, keyed("keyTangent", QJsonObject{{"inTangentType", t == QString("step") ? "clamped" : t}, {"outTangentType", t}}));
    }
    // Sound: which audio node's waveform shows, and importing one.
    m.addSeparator();
    QMenu* snd = m.addMenu("Sound");
    QString shown = ask("timeControl", QJsonObject{{"query", true}, {"queried", QJsonArray{"sound"}}}).toString();
    QAction* none = snd->addAction("None", [this] { exec("timeControl", QJsonObject{{"sound", ""}}); time_slider_->update(); });
    none->setCheckable(true);
    none->setChecked(shown.isEmpty());
    for (const QJsonValue& v : ask("ls", QJsonObject{{"type", "audio"}}).toArray()) {
        QString n = v.toString();
        QAction* a = snd->addAction(n, [this, n] { exec("timeControl", QJsonObject{{"sound", n}, {"displaySound", true}}); time_slider_->update(); });
        a->setCheckable(true);
        a->setChecked(n == shown);
    }
    snd->addSeparator();
    snd->addAction("Import Audio...", [this] {
        // Leaves Aztek usable while it is open, over the main window (#109).
        browse_file(false, "Import Audio", QString(), "WAV (*.wav)", QString(), [=](const QString& f) {
            if (f.isEmpty()) return;
            QJsonValue n = ask("sound", QJsonObject{{"file", f}, {"offset", time_slider_->time()}});
            if (n.isString()) exec("timeControl", QJsonObject{{"sound", n.toString()}, {"displaySound", true}});
            time_slider_->update();
        });
    });
    m.exec(global);
}

void MainWindow::show_tool_settings() {
    tool_dock_->show();
    tool_dock_->raise();
}


void MainWindow::show_uv_editor() {
    if (!uv_window_) {
        uv_window_ = new UvEditor(this);
        uv_window_->setWindowFlags(editor_window_flags());
        // Over the right of the main window, as the reference opens its editors over the panels.
        QRect g = geometry();
        uv_window_->move(g.x() + g.width() - uv_window_->width() - 340, g.y() + 140);
    }
    float_dockable("uvEditor", uv_window_, "UV Editor");
    static_cast<UvEditor*>(uv_window_)->refresh();
}

void MainWindow::show_graph_editor() {
    if (!graph_window_) {
        graph_window_ = new GraphEditor(this);
        graph_window_->setWindowFlags(editor_window_flags());
        QRect g = geometry();
        graph_window_->move(g.x() + (g.width() - graph_window_->width()) / 2, g.y() + g.height() - graph_window_->height() - 140);
    }
    graph_window_->show();
    graph_window_->raise();
    static_cast<GraphEditor*>(graph_window_)->refresh();
}

// The Channel Box and the Attribute Editor share a side but not a width (#159): the Channel Box as narrow as its numbers
// need, the Attribute Editor about twice that. Each keeps the width it was last dragged to, and takes it again when it
// comes to the front of the tab.
namespace {
class DockWidth : public QObject {
public:
    DockWidth(QDockWidget* d, QString key, int fallback) : QObject(d), dock_(d), key_(std::move(key)), fallback_(fallback) { d->installEventFilter(this); }
    int width() const { return QSettings().value(key_, fallback_).toInt(); }
    bool applying = false;

protected:
    bool eventFilter(QObject*, QEvent* e) override {
        if (e->type() == QEvent::Resize && !applying && !dock_->isFloating() && dock_->isVisible() && !dock_->visibleRegion().isEmpty()) {
            const int w = static_cast<QResizeEvent*>(e)->size().width();
            if (w > 60) QSettings().setValue(key_, w);
        }
        return false;
    }

private:
    QDockWidget* dock_;
    QString key_;
    int fallback_;
};
} // namespace

void MainWindow::keep_dock_widths() {
    const QList<QPair<QDockWidget*, int>> docks = {{channel_dock_, 260}, {attr_dock_, 520}};
    for (const auto& entry : docks) {
        QDockWidget* d = entry.first;
        auto* keep = new DockWidth(d, "docks/" + d->objectName() + "Width", entry.second);
        connect(d, &QDockWidget::visibilityChanged, this, [this, d, keep](bool on) {
            if (!on || d->isFloating()) return;
            QTimer::singleShot(0, this, [this, d, keep] {
                if (!d->isVisible() || d->visibleRegion().isEmpty() || d->isFloating() || d->width() == keep->width()) return;
                keep->applying = true;
                resizeDocks({d}, {keep->width()}, Qt::Horizontal);
                keep->applying = false;
            });
        });
    }
}

void MainWindow::show_attribute_editor() {
    if (attr_dock_->isVisible() && !attr_dock_->visibleRegion().isEmpty()) {
        channel_dock_->raise();
        return;
    }
    attr_dock_->show();
    attr_dock_->raise();
}

// A panel floated off the main window is a window of the system's own, with its border, title and buttons, as the
// reference's are; its title bar inside still docks it when dragged onto the main window (#197). On X11 Qt draws a
// floating panel with no border of its own, so the flags are set once the mouse is let go, not mid-drag.
static void frame_floating(QPointer<QDockWidget> d) {
    if (!d || !d->isFloating()) return;
    if (QGuiApplication::mouseButtons() != Qt::NoButton) {
        QTimer::singleShot(50, d, [d] { frame_floating(d); });
        return;
    }
    const Qt::WindowFlags framed = Qt::Tool | Qt::CustomizeWindowHint | Qt::WindowTitleHint | Qt::WindowSystemMenuHint |
                                   Qt::WindowMinMaxButtonsHint | Qt::WindowCloseButtonHint;
    if ((d->windowFlags() & framed) == framed && !(d->windowFlags() & Qt::FramelessWindowHint)) return;
    const QRect g = d->geometry();
    d->setWindowFlags(framed);
    d->setGeometry(g);
    d->show();
    d->raise();
}

static void framed_when_floating(QDockWidget* d) {
    if (d->property("aztekFramed").toBool()) return;
    d->setProperty("aztekFramed", true);
    QObject::connect(d, &QDockWidget::topLevelChanged, d, [d](bool floating) {
        if (floating) QTimer::singleShot(0, d, [p = QPointer<QDockWidget>(d)] { frame_floating(p); });
    });
    if (d->isFloating()) frame_floating(d);
}

// ---------------------------------------------------------------------------------------------------------------------
// Marking menus and hotkeys

bool MainWindow::eventFilter(QObject* o, QEvent* e) {
    if (e->type() == QEvent::ChildPolished && o == this) {
        if (auto* d = qobject_cast<QDockWidget*>(static_cast<QChildEvent*>(e)->child())) framed_when_floating(d);
    }
    // Text dragged with the middle button from any text field or editor, as the reference allows: dropped on a shelf
    // it makes a button (#117). Script Editor tabs carry their own language; elsewhere Python is told by its look.
    if (e->type() == QEvent::MouseButtonPress && static_cast<QMouseEvent*>(e)->button() == Qt::MiddleButton) {
        QString text;
        QWidget* src = nullptr;
        QWidget* w = qobject_cast<QWidget*>(o);
        if (auto* pe = w ? qobject_cast<QPlainTextEdit*>(w->parentWidget()) : nullptr; pe && pe->viewport() == w && pe->textCursor().hasSelection()) {
            text = pe->textCursor().selectedText().replace(QChar(0x2029), '\n');
            src = pe;
        } else if (auto* te = w ? qobject_cast<QTextEdit*>(w->parentWidget()) : nullptr; te && te->viewport() == w && te->textCursor().hasSelection()) {
            text = te->textCursor().selectedText().replace(QChar(0x2029), '\n');
            src = te;
        } else if (auto* le = qobject_cast<QLineEdit*>(o); le && le->hasSelectedText()) {
            text = le->selectedText();
            src = le;
        }
        if (!text.trimmed().isEmpty() && src) {
            QString lang = src->property("scriptLanguage").toString();
            if (lang.isEmpty()) {
                static const QRegularExpression py("^\\s*(import |from |def |print\\(|cmds\\.|pm\\.|for .* in )", QRegularExpression::MultilineOption);
                lang = py.match(text).hasMatch() ? "python" : "mel";
            }
            auto* drag = new QDrag(src);
            auto* m = new QMimeData;
            m->setText(text);
            m->setData("application/x-aztek-script-language", lang.toUtf8());
            drag->setMimeData(m);
            drag->exec(Qt::CopyAction);
            return true;
        }
    }
    // Ctrl+Shift+click on a menu item puts it on the shelf.
    if (e->type() == QEvent::MouseButtonRelease) {
        if (auto* menu = qobject_cast<QMenu*>(o)) {
            auto* me = static_cast<QMouseEvent*>(e);
            if ((me->modifiers() & (Qt::ControlModifier | Qt::ShiftModifier)) == (Qt::ControlModifier | Qt::ShiftModifier)) {
                if (QAction* a = menu->actionAt(me->position().toPoint())) {
                    add_to_shelf(a);
                    while (QWidget* p = QApplication::activePopupWidget()) p->close();
                    return true;
                }
            }
        }
    }
    // Esc stops a render under way, wherever the keyboard is, as in the reference (#186). With nothing running it goes on
    // to whatever had it.
    if (e->type() == QEvent::KeyPress) {
        auto* ke = static_cast<QKeyEvent*>(e);
        if (ke->key() == Qt::Key_Escape && !ke->isAutoRepeat() && ke->modifiers() == Qt::NoModifier && abort_render()) {
            mel("print \"Render aborted.\\n\";");
            return true;
        }
    }
    // A tool key held with a left press in a view: the tool's marking menu. K and I work a drag.
    if (!view_hold_.isEmpty() && qobject_cast<ViewportWidget*>(o) && (e->type() == QEvent::MouseButtonPress || e->type() == QEvent::MouseMove || e->type() == QEvent::MouseButtonRelease)) {
        auto* me = static_cast<QMouseEvent*>(e);
        if (view_hold_ == "scrub" || view_hold_ == "nudgeKeys") {
            view_scrub(o, e);
            return true;
        }
        if (e->type() == QEvent::MouseButtonPress && me->button() == Qt::LeftButton && !(me->modifiers() & Qt::AltModifier)) {
            if (view_hold_ == "menuSet") {
                hold_used_ = true;
                mel("MenuSetMenu;");
                return true;
            }
            tool_marking_menu(me->globalPosition().toPoint(), view_hold_);
            return true;
        }
    }
    if (e->type() == QEvent::MouseButtonDblClick && o->property("toolId").isValid()) {
        set_tool(o->property("toolId").toString());
        show_tool_settings();
        return true;
    }
    // Another window took the keyboard with a key held: its release won't come, so run it now.
    if (e->type() == QEvent::ApplicationDeactivate && !held_.isEmpty()) {
        QHash<int, QString> held = held_;
        held_.clear();
        for (const QString& rel : held)
            if (!rel.isEmpty()) run_command(rel);
    }
    if (e->type() == QEvent::KeyPress || e->type() == QEvent::KeyRelease) {
        // Only for this window's widgets, and never while typing in a field.
        QWidget* w = qobject_cast<QWidget*>(o);
        if (!w || w->window() != this) return QMainWindow::eventFilter(o, e);
        auto* k = static_cast<QKeyEvent*>(e);
        QWidget* focus = QApplication::focusWidget();
        // A key held down lets go wherever the focus has gone since.
        if (e->type() == QEvent::KeyRelease && held_.contains(k->key()) && !k->isAutoRepeat() && (o == focus || !focus)) {
            if (hotkey(k, false)) return true;
        }
        // Enter in a panel's number field hands the keyboard back.
        if (e->type() == QEvent::KeyRelease && (k->key() == Qt::Key_Return || k->key() == Qt::Key_Enter) && qobject_cast<QAbstractSpinBox*>(focus)) focus->clearFocus();
        // A number field has no use for a plain letter: it goes to the hotkeys.
        bool letter_in_number = qobject_cast<QAbstractSpinBox*>(focus) && k->key() >= Qt::Key_A && k->key() <= Qt::Key_Z && !(k->modifiers() & (Qt::ControlModifier | Qt::AltModifier));
        if (typing_widget(focus) && !letter_in_number) return QMainWindow::eventFilter(o, e);
        // A key goes to the panel under the pointer, as the reference's hotkeys do (#112): the keyboard follows the
        // pointer into another panel before the key is read, so F over a view frames the view whatever had the
        // keyboard before.
        if (e->type() == QEvent::KeyPress && focus && o == focus && !QApplication::activePopupWidget()) {
            QWidget* under = QApplication::widgetAt(QCursor::pos());
            QWidget* panel = nullptr;
            for (QWidget* w = under; w && w != this; w = w->parentWidget())
                if (qobject_cast<ViewportWidget*>(w) || w->objectName() == "outliner" || w->objectName() == "uvCanvas" || w->objectName() == "graphCanvas" || w->objectName() == "renderImage") {
                    panel = w;
                    break;
                }
            if (panel && panel != focus && !panel->isAncestorOf(focus) && panel->window() == focus->window()) {
                panel->setFocus(Qt::MouseFocusReason);
                QKeyEvent copy(QEvent::KeyPress, k->key(), k->modifiers(), k->text(), k->isAutoRepeat(), k->count());
                QApplication::sendEvent(QApplication::focusWidget() ? QApplication::focusWidget() : panel, &copy);
                return true;
            }
        }
        // The Outliner keeps its own keys while it has the keyboard: F frames its selection, the arrows walk its list.
        if (focus && focus->objectName() == "outliner" && outliner_key(k)) return QMainWindow::eventFilter(o, e);
        if (o != focus && focus) return QMainWindow::eventFilter(o, e);
        // F goes to the panel under the pointer, as the reference's hotkeys do: over an Outliner it frames the
        // Outliner's selection there, wherever the keyboard is.
        if (e->type() == QEvent::KeyPress && k->key() == Qt::Key_F && !(k->modifiers() & (Qt::ControlModifier | Qt::AltModifier | Qt::ShiftModifier))) {
            for (QWidget* under = QApplication::widgetAt(QCursor::pos()); under; under = under->parentWidget())
                if (under->objectName() == "outliner" || under->objectName() == "renderImage") {
                    QKeyEvent copy(QEvent::KeyPress, k->key(), k->modifiers(), k->text());
                    QApplication::sendEvent(under, &copy);
                    return true;
                }
        }
        if (e->type() == QEvent::KeyPress && tool_ == "groom" && groom_key(k)) return true;
        if (hotkey(k, e->type() == QEvent::KeyPress)) return true;
    }
    return QMainWindow::eventFilter(o, e);
}

// The grooming tools' keys: N the tool's other mode, Ctrl+Shift+I invert frozen, Ctrl+1 to Ctrl+0 the first ten tools.
bool MainWindow::groom_key(QKeyEvent* k) {
    const auto mods = k->modifiers() & (Qt::ControlModifier | Qt::ShiftModifier | Qt::AltModifier);
    auto flip = [this](const char* flag) {
        bool on = ask("xjenGroomCtx", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}).toBool();
        exec("xjenGroomCtx", QJsonObject{{flag, !on}});
        help_line_->setText(QString(flag) + (on ? " off" : " on"));
        if (tool_settings_) tool_settings_->set_tool(tool_);
    };
    if (k->key() == Qt::Key_N && mods == Qt::NoModifier) {
        flip("altMode");
        return true;
    }
    if (k->key() == Qt::Key_I && mods == (Qt::ControlModifier | Qt::ShiftModifier)) {
        flip("invertFrozen");
        return true;
    }
    if (mods == Qt::ControlModifier && k->key() >= Qt::Key_0 && k->key() <= Qt::Key_9) {
        static const char* ten[] = {"density", "place", "length", "cut", "width", "twist", "comb", "grab", "smooth", "noise"};
        int i = k->key() == Qt::Key_0 ? 9 : k->key() - Qt::Key_1;
        exec("xjenGroomCtx", QJsonObject{{"tool", ten[i]}});
        help_line_->setText(QString("Groom: ") + ten[i]);
        if (tool_settings_) tool_settings_->set_tool(tool_);
        return true;
    }
    return false;
}

void MainWindow::closeEvent(QCloseEvent* e) {
    if (!confirm_unsaved()) {
        e->ignore();
        return;
    }
    QSettings s;
    s.setValue("window/geometry", saveGeometry());
    s.setValue("window/state", saveState());
    s.setValue("window/workspace", s.value("workspaces_current", "General"));
    s.setValue("session/running", false);
    e->accept();
}

void MainWindow::restore_saved_window() {
    QSettings s;
    if (!s.contains("window/geometry")) return;
    // Restore the docks before maximising. Qt can keep a pending copy of a maximised layout that refers to
    // the old dock items when restore_layout reseats the sliders from a legacy layout (#289).
    restore_layout(s.value("window/state").toByteArray());
    restoreGeometry(s.value("window/geometry").toByteArray());
}

void MainWindow::run_startup(const QString& script, const QString& screenshot) {
    no_prompts_ = !screenshot.isEmpty();
    QSettings s;
    // The saved geometry and panels were restored before the window was shown (#225).
    exec_quiet("constructionHistory", QJsonObject{{"state", s.value("prefs/constructionHistory", true).toBool()}});
    const QString workspace = s.value("workspaces_current", "General").toString();
    const QString saved_workspace = s.value("window/workspace").toString();
    if (screenshot.isEmpty() && (workspace != "General" || s.contains("workspaces/General")
                               || (!saved_workspace.isEmpty() && saved_workspace != workspace))) {
        // Workspace editors must exist before Qt can put their docks back where the user left them.
        apply_workspace(workspace);
        if (saved_workspace == workspace)
            restore_layout(s.value("window/state").toByteArray());
    }
    // Last time ended without closing: offer the newest autosave. Not when a script or a screenshot drives the run.
    const bool crashed = s.value("session/running", false).toBool();
    s.setValue("session/running", true);
    // The welcome screen follows, unless it was turned off or a scene is open by then: after an unclean end as well,
    // which had kept it from ever showing for those whose Aztek doesn't get to close itself.
    const bool welcome = script.isEmpty() && screenshot.isEmpty() && state()["file"].toString().isEmpty();
    if (crashed && script.isEmpty() && screenshot.isEmpty()) {
        QTimer::singleShot(800, this, [this, welcome] {
            QString latest = ask("autoSave", QJsonObject{{"query", true}, {"queried", QJsonArray{"latestFile"}}}).toString();
            if (!latest.isEmpty() && QMessageBox::question(this, "Recover", "Aztek did not close properly last time.\n\nOpen the newest autosave?\n" + latest,
                                                           QMessageBox::Open | QMessageBox::Ignore, QMessageBox::Open) == QMessageBox::Open) {
                exec("file", QJsonObject{{"args", QJsonArray{latest}}, {"open", true}, {"force", true}});
                return;
            }
            if (welcome) show_welcome(true);
        });
    }
    // New versions: asked about once, then checked in the background. Not when a script or a screenshot drives the run.
    if (script.isEmpty() && screenshot.isEmpty())
        QTimer::singleShot(2500, this, [this] { updates_at_startup(this, [this] { return close(); }); });
    if (welcome && !crashed) QTimer::singleShot(600, this, [this] { show_welcome(true); });
    if (!script.isEmpty()) {
        QTimer::singleShot(300, this, [this, script] {
            QFile f(script);
            if (f.open(QIODevice::ReadOnly)) {
                QString src = QString::fromUtf8(f.readAll());
                if (script.endsWith(".py"))
                    python(src);
                else
                    mel(src);
            }
        });
    }
    if (!screenshot.isEmpty()) {
        QTimer::singleShot(1500, this, [this, screenshot] {
            // The whole window: grab the Qt widgets, then paint each viewport's own pixels over its spot.
            QImage img = grab().toImage();
            QPainter p(&img);
            for (ViewportWidget* v : panels_->views()) {
                if (!v->isVisible()) continue;
                QString tmp = screenshot + ".view.ppm";
                if (!v->capture(tmp)) continue;
                QImage vi(tmp);
                QFile::remove(tmp);
                QPoint at = v->mapTo(this, QPoint(0, 0));
                p.drawImage(QRect(at, v->size()), vi);
            }
            p.end();
            img.save(screenshot);
            qApp->quit();
        });
    }
}

} // namespace az
