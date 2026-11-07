// The menus every menu set has (File, Edit, Select, Modify, Display, Windows), the marking menus, workspaces and the small
// actions they share.
#include "mainwindow.h"
#include "aztek.h"
#include "colorchooser.h"
#include "editors.h"
#include "host.h"
#include "usdui.h"
#include "icons.h"
#include "markingmenu.h"
#include "panels.h"
#include "shelves.h"
#include "toolsettings.h"
#include "viewport.h"
#include "widgets.h"

#include <QCoreApplication>
#include <QDir>
#include <QFileInfo>
#include <QAction>
#include <QApplication>
#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDir>
#include <QDockWidget>
#include <QDoubleSpinBox>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLabel>
#include <QLineEdit>
#include <QMenuBar>
#include <QMessageBox>
#include <QMouseEvent>
#include <QPushButton>
#include <QRegularExpression>
#include <QSettings>
#include <QStatusBar>
#include <QTimer>
#include <QToolBar>
#include <QVBoxLayout>
#include <algorithm>
#include <cmath>

namespace az {

namespace {

QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}

QStringList selection_names() {
    QStringList out;
    for (const QJsonValue& v : state()["selection"].toArray()) out << v.toString();
    return out;
}

// Each item of the hierarchy under a node, itself first.
void hierarchy_of(const QJsonObject& n, QStringList& out) {
    out << n["path"].toString();
    for (const QJsonValue& c : n["children"].toArray()) hierarchy_of(c.toObject(), out);
}

void walk(const QJsonArray& nodes, const std::function<void(const QJsonObject&)>& fn) {
    for (const QJsonValue& v : nodes) {
        fn(v.toObject());
        walk(v.toObject()["children"].toArray(), fn);
    }
}

// The window asks a name and acts on each selected node's hierarchy.
void rename_hierarchy(QWidget* parent, const std::function<QString(const QString&)>& next) {
    QStringList sel = selection_names();
    if (sel.isEmpty()) return;
    QStringList all;
    walk(query("outliner", "shapes").toArray(), [&](const QJsonObject& n) {
        for (const QString& s : sel)
            if (n["path"].toString() == s || n["path"].toString().startsWith(s + "|") || n["name"].toString() == s) {
                hierarchy_of(n, all);
                return;
            }
    });
    all.removeDuplicates();
    // Deepest first, so paths above stay good while renaming.
    std::sort(all.begin(), all.end(), [](const QString& a, const QString& b) { return a.count('|') > b.count('|'); });
    (void)parent;
    for (const QString& p : all) {
        QString leaf = p.section('|', -1);
        QString to = next(leaf);
        if (to != leaf) exec("rename", args(QJsonArray{p, to}));
    }
}

} // namespace

// ---------------------------------------------------------------------------------------------------------------------
// File

void MainWindow::increment_save() {
    QString f = state()["file"].toString();
    if (f.isEmpty()) {
        save_scene(true);
        return;
    }
    // name.0003.aztek after name.0002.aztek; name.aztek is copied to name.0001.aztek, as the reference's.
    QFileInfo fi(f);
    QString stem = fi.completeBaseName();
    QRegularExpression re("^(.*?)\\.?(\\d+)$");
    auto m = re.match(stem);
    QString base = m.hasMatch() ? m.captured(1) : stem;
    int width = m.hasMatch() ? m.captured(2).size() : 4;
    int n = m.hasMatch() ? m.captured(2).toInt() + 1 : 1;
    QString next = fi.dir().filePath(QString("%1.%2.%3").arg(base).arg(n, width, 10, QChar('0')).arg(fi.suffix()));
    QString err = take(aztek_file("save", next.toUtf8().constData()));
    Host::get()->poke();
    if (err.isEmpty()) update_title();
}

void MainWindow::set_project() {
    QString d = QFileDialog::getExistingDirectory(this, "Set Project", QSettings().value("project/current").toString());
    if (d.isEmpty()) return;
    exec("workspace", QJsonObject{{"openWorkspace", d}});
    add_recent_project(d);
}

void MainWindow::add_recent_project(const QString& dir) {
    QSettings s;
    s.setValue("project/current", dir);
    QStringList r = s.value("project/recent").toStringList();
    r.removeAll(dir);
    r.prepend(dir);
    s.setValue("project/recent", r.mid(0, 10));
}

QMenu* MainWindow::file_menu() {
    auto* m = new QMenu("File", this);
    m->addAction(icon("new"), "New Scene\tCtrl+N", [this] { new_scene(); });
    m->addAction(icon("open"), "Open Scene...\tCtrl+O", this, &MainWindow::open_scene);
    m->addAction(icon("save"), "Save Scene\tCtrl+S", [this] { save_scene(false); });
    m->addAction("Save Scene As...\tCtrl+Shift+S", [this] { save_scene(true); });
    m->addAction("Increment and Save\tCtrl+Alt+S", [this] { increment_save(); });
    m->addAction("Revert", [this] {
        QString f = state()["file"].toString();
        if (f.isEmpty()) {
            log("// Warning: the scene has no file to revert to //", 4);
            return;
        }
        if (QMessageBox::question(this, "Revert", "Discard changes and open " + f + " again?") == QMessageBox::Yes) open_file(f);
    });
    m->addAction("Optimize Scene Size", [] { exec("cleanUpScene"); });
    m->addSeparator();
    auto do_import = [] {
        QString f = QFileDialog::getOpenFileName(nullptr, "Import", QString(), QString("Scenes and meshes (*.ma *.obj *.ply *.stl *.usd *.usda *.usdc *.usdz);;ASCII scenes (*.ma);;%1;;Meshes (*.obj *.ply *.stl)").arg(kUsdFilter));
        if (f.isEmpty()) return;
        if (is_usd_file(f)) {
            usd_import(f);
            return;
        }
        if (f.endsWith(".ma", Qt::CaseInsensitive)) {
            exec("file", QJsonObject{{"args", QJsonArray{f}}, {"import", true}});
            return;
        }
        QString c = f.endsWith(".ply", Qt::CaseInsensitive) ? "importPly" : f.endsWith(".stl", Qt::CaseInsensitive) ? "importStl" : "importObj";
        exec(c, args(QJsonArray{f}));
    };
    m->addAction(new OptionRow("Import...", QString(), do_import, [this] { usd_import_options(this); }, m));
    m->addSeparator();
    {
        auto* row = new OptionRow("Create Reference...", "Ctrl+R", [this] { create_reference_dialog(false); }, [this] { create_reference_dialog(true); }, m);
        m->addAction(row);
    }
    m->addAction("Reference Editor", this, &MainWindow::show_reference_editor);
    m->addSeparator();
    auto do_export = [](bool all) {
        QString f = QFileDialog::getSaveFileName(nullptr, all ? "Export All" : "Export Selection", "export.ma", QString("ASCII scenes (*.ma);;%1;;OBJ (*.obj);;PLY (*.ply);;STL (*.stl)").arg(kUsdFilter));
        if (f.isEmpty()) return;
        if (is_usd_file(f)) {
            usd_export(f, !all);
            return;
        }
        if (f.endsWith(".ma", Qt::CaseInsensitive)) {
            exec("file", QJsonObject{{"args", QJsonArray{f}}, {all ? "exportAll" : "exportSelected", true}, {"type", "mayaAscii"}});
            return;
        }
        QStringList keep = selection_names();
        if (all) mel("SelectAll;");
        QString c = f.endsWith(".ply", Qt::CaseInsensitive) ? "exportPly" : f.endsWith(".stl", Qt::CaseInsensitive) ? "exportStl" : "exportObj";
        exec(c, args(QJsonArray{f}));
        if (all) {
            QJsonArray a;
            for (const QString& k : keep) a.append(k);
            exec("select", a.isEmpty() ? QJsonObject{{"clear", true}} : args(a));
        }
    };
    m->addAction(new OptionRow("Export All...", QString(), [do_export] { do_export(true); }, [this] { usd_export_options(this); }, m));
    m->addAction(new OptionRow("Export Selection...", QString(), [do_export] { do_export(false); }, [this] { usd_export_options(this); }, m));
    m->addSeparator();
    QMenu* project = m->addMenu("Project");
    project->addAction("Project Window...", [this] {
        QDialog d(this);
        d.setWindowTitle("Project Window");
        auto* form = new QFormLayout(&d);
        auto* where = new QLineEdit(QSettings().value("project/current", QDir::homePath() + "/aztekProject").toString(), &d);
        auto* browse = new QPushButton("Browse...", &d);
        connect(browse, &QPushButton::clicked, &d, [&] {
            QString x = QFileDialog::getExistingDirectory(&d, "Location", where->text());
            if (!x.isEmpty()) where->setText(x);
        });
        form->addRow("Location:", where);
        form->addRow("", browse);
        form->addRow(new QLabel("Accept creates scenes, scripts, images, sourceimages and data folders there.", &d));
        auto* ok = new QPushButton("Accept", &d);
        form->addRow("", ok);
        connect(ok, &QPushButton::clicked, &d, &QDialog::accept);
        if (d.exec() != QDialog::Accepted || where->text().isEmpty()) return;
        exec("workspace", QJsonObject{{"create", where->text()}});
        exec("workspace", QJsonObject{{"openWorkspace", where->text()}});
        add_recent_project(where->text());
    });
    project->addAction("Set Project...", [this] { set_project(); });
    QMenu* examples = m->addMenu("Examples");
    connect(examples, &QMenu::aboutToShow, this, [this, examples] {
        examples->clear();
        // Beside the program, in the source tree when running from a build, and where the Linux packages put them.
        const QString base = QCoreApplication::applicationDirPath();
        for (const QString& where : {base + "/examples", base + "/../examples", base + "/../share/aztek/examples"}) {
            const QStringList names = QDir(where).entryList({"*.ma"}, QDir::Files, QDir::Name);
            if (names.isEmpty()) continue;
            for (const QString& name : names) {
                // Those that start ref_ are what the references example points at.
                if (name.startsWith("ref_")) continue;
                QString label = QFileInfo(name).completeBaseName().replace('_', ' ');
                label[0] = label[0].toUpper();
                const QString path = QDir(where).absoluteFilePath(name);
                examples->addAction(label, [this, path] { if (confirm_unsaved()) open_file(path); });
            }
            break;
        }
        if (examples->isEmpty()) examples->addAction("(none found)")->setEnabled(false);
    });
    QMenu* recent = m->addMenu("Recent Files");
    connect(recent, &QMenu::aboutToShow, this, [this, recent] {
        recent->clear();
        for (const QString& f : QSettings().value("recentFiles").toStringList()) recent->addAction(f, [this, f] { if (confirm_unsaved()) open_file(f); });
        if (recent->isEmpty()) recent->addAction("(none)")->setEnabled(false);
    });
    QMenu* projects = m->addMenu("Recent Projects");
    connect(projects, &QMenu::aboutToShow, this, [this, projects] {
        projects->clear();
        for (const QString& f : QSettings().value("project/recent").toStringList())
            projects->addAction(f, [this, f] {
                exec("workspace", QJsonObject{{"openWorkspace", f}});
                add_recent_project(f);
            });
        if (projects->isEmpty()) projects->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    m->addAction("Run Script...", [] {
        QString f = QFileDialog::getOpenFileName(nullptr, "Run Script", QSettings().value("project/current").toString(), "Scripts (*.mel *.py)");
        if (f.isEmpty()) return;
        QFile file(f);
        if (!file.open(QIODevice::ReadOnly)) return;
        QString src = QString::fromUtf8(file.readAll());
        if (f.endsWith(".py", Qt::CaseInsensitive)) python(src);
        else mel(src);
    });
    m->addSeparator();
    m->addAction("Exit\tCtrl+Q", this, &QWidget::close);
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Edit

void MainWindow::remember_command(const QString& text) {
    QString t = text.trimmed();
    if (t.isEmpty() || t.startsWith("//") || t.startsWith("select ") || t.startsWith("selectMode")) return;
    recent_commands_.removeAll(t);
    recent_commands_.prepend(t);
    while (recent_commands_.size() > 30) recent_commands_.removeLast();
}

QMenu* MainWindow::edit_menu() {
    auto* m = new QMenu("Edit", this);
    m->addAction(icon("undo"), "Undo\tZ", [] { aztek_undo(); Host::get()->poke(); });
    m->addAction(icon("redo"), "Redo\tShift+Z", [] { aztek_redo(); Host::get()->poke(); });
    m->addAction("Repeat\tG", this, &MainWindow::repeat_last);
    QMenu* recents = m->addMenu("Recent Commands List");
    connect(recents, &QMenu::aboutToShow, this, [this, recents] {
        recents->clear();
        for (const QString& c : recent_commands_) recents->addAction(c.left(70), [c] { mel(c); });
        if (recents->isEmpty()) recents->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    m->addAction("Cut\tCtrl+X", [this] {
        clipboard_ = selection_names();
        exec("delete");
    });
    m->addAction("Copy\tCtrl+C", [this] { clipboard_ = selection_names(); });
    m->addAction("Paste\tCtrl+V", [this] {
        QJsonArray a;
        for (const QString& n : clipboard_) a.append(n);
        if (!a.isEmpty()) exec("duplicate", args(a));
    });
    m->addSeparator();
    cmd(m, "Delete", "delete", {}, "Delete");
    QMenu* by_type = m->addMenu("Delete by Type");
    cmd(by_type, "History", "delete", QJsonObject{{"constructionHistory", true}}, "Alt+Shift+D");
    cmd(by_type, "Non-Deformer History", "bakePartialHistory", QJsonObject{{"prePostDeformers", true}});
    cmd(by_type, "Channels", "cutKey");
    cmd(by_type, "Static Channels", "delete", QJsonObject{{"staticChannels", true}});
    by_type->addAction("Constraints", [] {
        QJsonArray con;
        for (const QJsonValue& s : state()["selection"].toArray())
            for (const QJsonValue& c : ask("listRelatives", QJsonObject{{"args", QJsonArray{s}}, {"type", "constraint"}}).toArray()) con.append(c);
        if (!con.isEmpty()) exec("delete", args(con));
    });
    QMenu* all_type = m->addMenu("Delete All by Type");
    all_type->addAction("History", [] {
        QJsonArray all;
        for (const QJsonValue& v : query("outliner").toArray()) all.append(v.toObject()["path"].toString());
        QJsonObject c = args(all);
        c["constructionHistory"] = true;
        exec("delete", c);
    });
    for (const char* t : {"joint", "ikHandle", "nurbsCurve", "locator", "camera"}) {
        QString ty = t;
        QString label = ty.left(1).toUpper() + ty.mid(1) + "s";
        all_type->addAction(label, [ty] {
            QJsonArray hits;
            for (const QJsonValue& n : ask("ls", QJsonObject{{"type", ty}}).toArray()) {
                QString p = n.toString();
                if (ty == "camera" && (p.startsWith("persp") || p.startsWith("top") || p.startsWith("front") || p.startsWith("side"))) continue;
                hits.append(p);
            }
            if (!hits.isEmpty()) exec("delete", args(hits));
        });
    }
    m->addSeparator();
    cmd(m, "Duplicate", "duplicate", {}, "Ctrl+D");
    m->addAction("Duplicate Special\tCtrl+Shift+D", [] { OptionsDialog::open("duplicate", "Duplicate Special"); });
    cmd(m, "Duplicate with Transform", "duplicateWithTransform", {}, "Shift+D");
    cmd(m, "Instance", "instance", {}, QString(), true);
    cmd(m, "Transfer Attribute Values", "transferAttributes", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Group", "group", {}, "Ctrl+G", true);
    cmd(m, "Ungroup", "ungroup", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Parent", "parent", {}, "P", true);
    cmd(m, "Unparent", "parent", QJsonObject{{"world", true}}, "Shift+P");
    m->addSeparator();
    QMenu* tools = m->addMenu("Tools");
    tools->addAction("Select Tool\tQ", [this] { set_tool("select"); });
    tools->addAction("Lasso Tool", [this] { set_tool("lasso"); });
    tools->addAction("Paint Selection Tool", [this] { set_tool("paintSelect"); });
    tools->addAction("Move Tool\tW", [this] { set_tool("move"); });
    tools->addAction("Rotate Tool\tE", [this] { set_tool("rotate"); });
    tools->addAction("Scale Tool\tR", [this] { set_tool("scale"); });
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Select

void MainWindow::select_by_type(const QString& type) {
    QJsonArray hits = ask("ls", QJsonObject{{"type", type}}).toArray();
    if (hits.isEmpty()) {
        log(QString("// Warning: nothing of type %1 //").arg(type), 4);
        return;
    }
    // Shapes select through their transforms, as the reference's All by Type does.
    QJsonArray out;
    for (const QJsonValue& h : hits) {
        QString p = h.toString();
        QJsonArray parents = (type == "joint" || type == "ikHandle") ? QJsonArray() : ask("listRelatives", QJsonObject{{"args", QJsonArray{p}}, {"parent", true}}).toArray();
        out.append(parents.isEmpty() ? p : parents.at(0));
    }
    exec("select", args(out));
}

void MainWindow::select_shell(bool border) {
    QStringList sel = selection_names();
    QRegularExpression re("^(.*)\\.(vtx|e|f)\\[(\\d+)");
    QStringList done;
    bool first = true;
    for (const QString& s : sel) {
        auto m = re.match(s);
        if (!m.hasMatch() || done.contains(m.captured(1))) continue;
        done << m.captured(1);
        QString kind = m.captured(2) == "vtx" ? "vertex" : m.captured(2) == "e" ? "edge" : "face";
        QJsonObject c{{"args", QJsonArray{m.captured(1)}}, {"id", m.captured(3).toInt()}, {"kind", kind}};
        if (!first) c["add"] = true;
        exec("selectConnected", c);
        first = false;
    }
    if (done.isEmpty()) log("// Warning: select a component of the shell //", 4);
    else if (border) exec("selectBorder");
}

QMenu* MainWindow::select_menu() {
    auto* m = new QMenu("Select", this);
    m->addAction("All\tCtrl+Shift+A", [] { mel("SelectAll;"); });
    QMenu* by = m->addMenu("All by Type");
    struct T { const char* label; const char* type; };
    for (T t : {T{"Joints", "joint"}, T{"IK Handles", "ikHandle"}, T{"NURBS Curves", "nurbsCurve"}, T{"Polygon Geometry", "mesh"}, T{"Locators", "locator"}, T{"Cameras", "camera"}, T{"Clusters", "cluster"}, T{"Sets", "objectSet"}}) {
        QString ty = t.type;
        by->addAction(t.label, [this, ty] { select_by_type(ty); });
    }
    cmd(m, "Deselect All", "select", QJsonObject{{"clear", true}});
    cmd(m, "Hierarchy", "selectHierarchy");
    cmd(m, "Inverse", "invertSelection", {}, "Ctrl+Shift+I");
    m->addAction("Similar", [] {
        // Everything of the lead's type.
        QStringList sel = selection_names();
        if (sel.isEmpty()) return;
        QString lead = sel.last().section('.', 0, 0);
        QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{lead}}, {"shapes", true}}).toArray();
        QString ty = ask("nodeType", args(QJsonArray{shapes.isEmpty() ? lead : shapes.at(0).toString()})).toString();
        QJsonArray hits;
        for (const QJsonValue& h : ask("ls", QJsonObject{{"type", ty}}).toArray()) {
            QJsonArray ps = shapes.isEmpty() ? QJsonArray() : ask("listRelatives", QJsonObject{{"args", QJsonArray{h}}, {"parent", true}}).toArray();
            hits.append(ps.isEmpty() ? h : ps.at(0));
        }
        exec("select", args(hits));
    });
    m->addSeparator();
    m->addAction("Object/Component\tF8", [] { mel("ToggleSelectMode;"); });
    cmd(m, "Vertex", "selectMode", args(QJsonArray{"vertex"}), "F9");
    cmd(m, "Edge", "selectMode", args(QJsonArray{"edge"}), "F10");
    cmd(m, "Face", "selectMode", args(QJsonArray{"face"}), "F11");
    auto* multi = m->addAction("Multi");
    multi->setCheckable(true);
    connect(multi, &QAction::toggled, this, [this](bool on) {
        if (on && state()["mode"].toString() == "object") set_mode("vertex");
        for (ViewportWidget* v : panels_->views()) v->set("multiSelect", on ? "true" : "false");
    });
    m->addAction("UV Editor\tF12", [this] { show_uv_editor(); });
    m->addSeparator();
    cmd(m, "Grow", "growSelection", {}, ">");
    cmd(m, "Shrink", "shrinkSelection", {}, "<");
    cmd(m, "Grow Along Loop", "growSelection", QJsonObject{{"alongLoop", true}});
    cmd(m, "Shrink Along Loop", "shrinkSelection", QJsonObject{{"alongLoop", true}});
    m->addSeparator();
    cmd(m, "Contiguous Edges", "selectLoop");
    cmd(m, "Edge Ring", "selectLoop", QJsonObject{{"ring", true}});
    cmd(m, "Border", "selectBorder");
    m->addAction("Shell", [this] { select_shell(false); });
    m->addAction("Shell Border", [this] { select_shell(true); });
    cmd(m, "Select by Angle", "selectByAngle", {}, QString(), true);
    m->addAction("Shortest Edge Path Tool", [this] { set_tool("shortestPath"); });
    if (!has_command_named("polySelectConstraint"))
        todo(m, "Use Constraints...");
    else
        m->addAction("Use Constraints...", [this] { show_named_window("polySelectConstraint"); });
    m->addSeparator();
    m->addAction("Lasso Tool", [this] { set_tool("lasso"); });
    m->addAction("Paint Selection Tool", [this] { set_tool("paintSelect"); });
    m->addSeparator();
    m->addAction("Create Quick Select Set...", [this] {
        bool ok = false;
        QString n = QInputDialog::getText(this, "Create Quick Select Set", "Name:", QLineEdit::Normal, "set1", &ok);
        if (ok && !n.isEmpty()) exec("createSet", QJsonObject{{"name", n}});
    });
    QMenu* quick = m->addMenu("Quick Select Sets");
    connect(quick, &QMenu::aboutToShow, this, [quick] {
        quick->clear();
        for (const QJsonValue& v : query("outliner").toArray()) {
            QJsonObject o = v.toObject();
            if (o["type"].toString() != "objectSet" || o["name"].toString().startsWith("default")) continue;
            QString name = o["name"].toString();
            quick->addAction(name, [name] { exec("selectSetMembers", args(QJsonArray{name})); });
        }
        if (quick->isEmpty()) quick->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    QMenu* conv = m->addMenu("Convert Selection");
    cmd(conv, "To Vertices", "convertSelection", args(QJsonArray{"vertex"}), "Ctrl+F9");
    cmd(conv, "To Edges", "convertSelection", args(QJsonArray{"edge"}), "Ctrl+F10");
    cmd(conv, "To Faces", "convertSelection", args(QJsonArray{"face"}), "Ctrl+F11");
    conv->addSeparator();
    cmd(conv, "To Edge Loop", "selectLoop");
    cmd(conv, "To Edge Ring", "selectLoop", QJsonObject{{"ring", true}});
    cmd(conv, "To Shell Border", "selectBorder");
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Modify

QMenu* MainWindow::modify_menu() {
    auto* m = new QMenu("Modify", this);
    QMenu* xf = m->addMenu("Transformation Tools");
    xf->addAction("Move Tool\tW", [this] { set_tool("move"); });
    xf->addAction("Rotate Tool\tE", [this] { set_tool("rotate"); });
    xf->addAction("Scale Tool\tR", [this] { set_tool("scale"); });
    xf->addAction("Move Normal Tool", [this] {
        set_tool("move");
        exec_quiet("manipMoveContext", QJsonObject{{"edit", true}, {"args", QJsonArray{"Move"}}, {"mode", 3}});
    });
    xf->addAction("Show Manipulator Tool\tT", [] { mel("ShowManipulators;"); });
    cmd(m, "Freeze Transformations", "freezeTransforms", {}, QString(), true);
    cmd(m, "Reset Transformations", "resetTransforms", {}, QString(), true);
    m->addSeparator();
    QMenu* pivot = m->addMenu("Pivot");
    cmd(pivot, "Center Pivot", "centerPivot");
    cmd(pivot, "Bake Pivot", "bakePivot");
    cmd(pivot, "Reset Pivot", "resetPivot");
    cmd(pivot, "Zero Pivot", "zeroPivot");
    QMenu* match = m->addMenu("Match Transformations");
    cmd(match, "Match Translation", "matchTransform", QJsonObject{{"position", true}});
    cmd(match, "Match Rotation", "matchTransform", QJsonObject{{"rotation", true}});
    cmd(match, "Match Scaling", "matchTransform", QJsonObject{{"scale", true}});
    cmd(match, "Match All Transforms", "matchTransform");
    cmd(match, "Match Pivots", "matchPivot");
    QMenu* snap = m->addMenu("Snap Align");
    snap->addAction("Align Objects...", [this] { show_named_window("alignObjects"); });
    snap->addAction("Point to Point", [] {
        // Two vertices: the first's object moves so the first lands on the second.
        QJsonArray sel = ask("ls", QJsonObject{{"selection", true}, {"flatten", true}}).toArray();
        if (sel.size() != 2) {
            log("// Warning: select a vertex on the object to move, then the vertex to snap it to //", 4);
            return;
        }
        QJsonArray a = ask("pointPosition", args(QJsonArray{sel[0]})).toArray(), b = ask("pointPosition", args(QJsonArray{sel[1]})).toArray();
        QString obj = sel[0].toString().section('.', 0, 0);
        exec("selectMode", args(QJsonArray{"object"}));
        exec("select", args(QJsonArray{obj}));
        exec("move", QJsonObject{{"by", QJsonArray{b[0].toDouble() - a[0].toDouble(), b[1].toDouble() - a[1].toDouble(), b[2].toDouble() - a[2].toDouble()}}});
    });
    snap->addAction("Snap Together", [] {
        // Whole objects: the first's pivot onto the second's.
        QStringList sel = selection_names();
        if (sel.size() != 2) {
            log("// Warning: select the object to move, then the one to snap it to //", 4);
            return;
        }
        QJsonArray to = ask("xform", QJsonObject{{"args", QJsonArray{sel[1]}}, {"query", true}, {"worldSpace", true}, {"rotatePivot", true}}).toArray();
        if (to.size() < 3) return;
        exec("select", args(QJsonArray{sel[0]}));
        exec("move", QJsonObject{{"to", QJsonArray{to[0], to[1], to[2]}}});
    });
    m->addSeparator();
    m->addAction("Prefix Hierarchy Names...", [this] {
        bool ok = false;
        QString p = QInputDialog::getText(this, "Prefix Hierarchy Names", "Prefix:", QLineEdit::Normal, "prefix_", &ok);
        if (ok && !p.isEmpty()) rename_hierarchy(this, [p](const QString& n) { return p + n; });
    });
    m->addAction("Search and Replace Names...", [this] { show_named_window("searchReplace"); });
    m->addSeparator();
    m->addAction("Add Attribute...", [this] { show_named_window("addAttribute"); });
    m->addAction("Edit Attribute...", [this] { show_named_window("editAttribute"); });
    m->addAction("Delete Attribute...", [this] { show_named_window("deleteAttribute"); });
    m->addSeparator();
    QMenu* convert = m->addMenu("Convert");
    cmd(convert, "NURBS to Polygons", "nurbsToPoly", QJsonObject{{"format", 3}, {"polygonType", 1}}, QString(), true);
    cmd(convert, "Instance to Object", "makeUnique");
    cmd(convert, "Object to Instance", "convertToInstance");
    cmd(convert, "Polygon Edges to Curve", "polyToCurve", {}, QString(), true);
    m->addSeparator();
    cmd(m, "Soft Select", "softSelect", QJsonObject{{"softSelectEnabled", 1}}, "B", true);
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Display

QMenu* MainWindow::display_menu() {
    auto* m = new QMenu("Display", this);
    QMenu* grid = m->addMenu("Grid");
    grid->addAction("Show", [this] { for (ViewportWidget* v : panels_->views()) v->set("grid", "true"); });
    grid->addAction("Hide", [this] { for (ViewportWidget* v : panels_->views()) v->set("grid", "false"); });
    grid->addAction("Grid Options...", [this] { show_named_window("gridOptions"); });
    QMenu* hud = m->addMenu("Heads Up Display");
    struct H { const char* label; const char* id; };
    QList<QPair<QAction*, QString>> hud_items;
    for (H h : {H{"Poly Count", "polyCount"}, H{"Object Details", "objectDetails"}, H{"Selection", "selection"}, H{"Frame Rate", "frameRate"}, H{"Current Frame", "currentFrame"}, H{"Camera Names", "cameraNames"}, H{"View Axis", "viewAxis"}}) {
        QString id = h.id;
        QAction* a = hud->addAction(h.label);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [this, id](bool on) {
            exec("headsUpDisplay", QJsonObject{{"args", QJsonArray{id}}, {"visible", on}});
            panels_->redraw_all();
        });
        hud_items.append({a, id});
    }
    connect(hud, &QMenu::aboutToShow, this, [hud_items] {
        for (const auto& it : hud_items) it.first->setChecked(ask("headsUpDisplay", QJsonObject{{"args", QJsonArray{it.second}}, {"exists", true}}).toBool());
    });
    QMenu* ui = m->addMenu("UI Elements");
    auto ui_item = [ui, this](const QString& label, QWidget* w) {
        QAction* a = ui->addAction(label);
        a->setCheckable(true);
        connect(ui, &QMenu::aboutToShow, a, [a, w] { a->setChecked(w->isVisible()); });
        connect(a, &QAction::triggered, w, [w](bool on) { w->setVisible(on); });
    };
    ui_item("Status Line", findChild<QToolBar*>("statusLine"));
    ui_item("Shelf", findChild<QToolBar*>("shelf"));
    ui_item("Tool Box", findChild<QToolBar*>("toolbox"));
    ui_item("Time and Range Sliders", findChild<QToolBar*>("timeBar"));
    ui_item("Command Line", findChild<QToolBar*>("commandLine"));
    ui_item("Help Line", statusBar());
    ui->addSeparator();
    ui_item("Attribute Editor", attr_dock_);
    ui_item("Tool Settings", tool_dock_);
    ui_item("Channel Box / Layer Editor", channel_dock_);
    ui_item("Outliner", outliner_dock_);
    m->addSeparator();
    QMenu* hide = m->addMenu("Hide");
    cmd(hide, "Hide Selection", "hide", {}, "Ctrl+H");
    hide->addAction("Hide Unselected Objects\tAlt+H", [] {
        // Top level objects other than what is selected and its parents.
        QJsonArray sel = state()["selection"].toArray();
        QJsonArray hidden;
        for (const QJsonValue& o : mel("ls -assemblies;").toArray()) {
            QString name = o.toString();
            bool keep = false;
            for (const QJsonValue& s : sel) {
                QString path = s.toString().section('.', 0, 0);
                if (path == name || path.startsWith("|" + name) || path.startsWith(name + "|")) keep = true;
            }
            if (!keep && !QStringList{"persp", "top", "front", "side"}.contains(name)) hidden.append(name);
        }
        if (!hidden.isEmpty()) exec("hide", QJsonObject{{"args", hidden}});
    });
    QMenu* show = m->addMenu("Show");
    cmd(show, "Show Selection", "show");
    cmd(show, "All", "showHidden", QJsonObject{{"all", true}});
    cmd(show, "Show Last Hidden", "showLastHidden", {}, "Ctrl+Shift+H");
    cmd(m, "Toggle Show/Hide", "toggleVisibility", {}, "H");
    m->addSeparator();
    QMenu* obj = m->addMenu("Object Display");
    cmd(obj, "Template", "toggle", QJsonObject{{"template", true}, {"state", true}});
    cmd(obj, "Untemplate", "toggle", QJsonObject{{"template", true}, {"state", false}});
    // A node's drawing override shows it as its box.
    auto box = [](bool on) {
        return [on] {
            for (const QJsonValue& s : state()["selection"].toArray()) {
                QString o = s.toString().section('.', 0, 0);
                exec("setAttr", QJsonObject{{"args", QJsonArray{o + ".overrideEnabled", on}}});
                exec("setAttr", QJsonObject{{"args", QJsonArray{o + ".overrideLevelOfDetail", on ? 1 : 0}}});
            }
        };
    };
    obj->addAction("Bounding Box", box(true));
    obj->addAction("No Bounding Box", box(false));
    m->addAction("Wireframe Color...", this, [this] { show_wireframe_color_window(this); });
    QMenu* xf = m->addMenu("Transform Display");
    cmd(xf, "Local Rotation Axes", "toggle", QJsonObject{{"localAxis", true}});
    cmd(xf, "Rotate Pivots", "toggle", QJsonObject{{"rotatePivot", true}});
    cmd(xf, "Scale Pivots", "toggle", QJsonObject{{"scalePivot", true}});
    cmd(xf, "Selection Handles", "toggle", QJsonObject{{"selectHandle", true}});
    QMenu* poly = m->addMenu("Polygons");
    auto poly_flag = [this, poly](const QString& label, const char* key) {
        QString k = key;
        QAction* a = poly->addAction(label);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [this, k](bool on) {
            for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"%1\":%2}").arg(k, on ? "true" : "false"));
        });
        return a;
    };
    poly_flag("Backface Culling", "backface");
    poly_flag("Border Edges", "borderEdges");
    poly_flag("Crease Edges", "creaseEdges");
    poly_flag("Hard Edges", "hardEdges");
    poly_flag("Face Normals", "faceNormals");
    poly_flag("Vertex Normals", "vertexNormals");
    poly_flag("Limit to Selected", "limitToSelected")->setChecked(true);
    poly->addAction("Normals Size...", [this] {
        bool ok = false;
        double x = QInputDialog::getDouble(this, "Normals Size", "Length:", 0.4, 0.001, 100, 3, &ok);
        if (ok)
            for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"normalSize\":%1}").arg(x));
    });
    poly->addSeparator();
    cmd(poly, "Soften Edge", "polySoftEdge", QJsonObject{{"angle", 180.0}});
    cmd(poly, "Harden Edge", "polySoftEdge", QJsonObject{{"angle", 0.0}});
    QMenu* nurbs = m->addMenu("NURBS");
    auto curve_attr = [nurbs](const QString& label, const char* attr, bool on) {
        QString a = attr;
        nurbs->addAction(label, [a, on] {
            for (const QString& s : selection_names())
                for (const QJsonValue& sh : ask("listRelatives", QJsonObject{{"args", QJsonArray{s}}, {"shapes", true}}).toArray())
                    exec("setAttr", args(QJsonArray{sh.toString() + "." + a, on}));
        });
    };
    curve_attr("CVs", "dispCV", true);
    curve_attr("No CVs", "dispCV", false);
    curve_attr("Hulls", "dispHull", true);
    curve_attr("No Hulls", "dispHull", false);
    QMenu* anim = m->addMenu("Animation");
    auto size = [this, anim](const QString& label, const char* pref) {
        QString p = pref;
        anim->addAction(label, [this, label, p] {
            bool ok = false;
            double cur = ask("uiPref", args(QJsonArray{p})).toDouble(1.0);
            double x = QInputDialog::getDouble(this, label, "Size:", cur, 0.01, 100, 2, &ok);
            if (!ok) return;
            exec_quiet("uiPref", args(QJsonArray{p, x}));
            QSettings().setValue("prefs/" + p, x);
            panels_->redraw_all();
        });
    };
    size("Joint Size...", "jointScale");
    size("IK Handle Size...", "ikHandleScale");
    QAction* jx = anim->addAction("Joint X-Ray");
    jx->setCheckable(true);
    connect(jx, &QAction::triggered, this, [this](bool on) {
        for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"xrayJoints\":%1}").arg(on ? "true" : "false"));
    });
    m->addSeparator();
    QMenu* sp = m->addMenu("Smooth Preview");
    sp->addAction("Off\t1", [] { mel("SmoothPreviewOff;"); });
    sp->addAction("Cage + Smooth\t2", [] { mel("SmoothPreviewCage;"); });
    sp->addAction("Smooth\t3", [] { mel("SmoothPreviewSmooth;"); });
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Windows and workspaces

void MainWindow::show_named_window(const QString& name) {
    QWidget* w = named_windows_.value(name);
    if (!w) {
        w = make_editor_window(name, this);
        if (!w) return;
        named_windows_[name] = w;
        connect(w, &QObject::destroyed, this, [this, name] { named_windows_.remove(name); });
    }
    w->show();
    w->raise();
    w->activateWindow();
}

void MainWindow::apply_workspace(const QString& name) {
    // A saved one first: the arrangement the user kept under this name.
    QSettings s;
    QByteArray saved = s.value("workspaces/" + name).toByteArray();
    if (!saved.isEmpty()) {
        restoreState(saved);
        return;
    }
    auto only = [this](bool outliner, bool channels, bool attr, bool tool) {
        outliner_dock_->setVisible(outliner);
        channel_dock_->setVisible(channels);
        attr_dock_->setVisible(attr);
        tool_dock_->setVisible(tool);
    };
    if (name == "General") {
        only(false, true, true, false);
        channel_dock_->raise();
        panels_->set_layout(0);
    } else if (name == "Modeling") {
        only(true, true, true, false);
        outliner_dock_->raise();
        channel_dock_->raise();
        panels_->set_layout(0);
        menu_set_->setCurrentText("Modeling");
    } else if (name == "Rigging") {
        only(true, true, true, false);
        outliner_dock_->raise();
        attr_dock_->raise();
        panels_->set_layout(0);
        menu_set_->setCurrentText("Rigging");
    } else if (name == "Animation") {
        only(true, true, false, false);
        channel_dock_->raise();
        panels_->set_layout(0);
        menu_set_->setCurrentText("Animation");
        show_graph_editor();
    } else if (name == "UV Editing") {
        only(false, true, true, false);
        panels_->set_layout(0);
        show_uv_editor();
    }
}

void MainWindow::toggle_main_pane() {
    if (!main_pane_) {
        main_pane_state_ = saveState();
        for (QToolBar* t : findChildren<QToolBar*>(QString(), Qt::FindDirectChildrenOnly)) t->hide();
        for (QDockWidget* d : findChildren<QDockWidget*>(QString(), Qt::FindDirectChildrenOnly)) d->hide();
        statusBar()->hide();
        main_pane_ = true;
    } else {
        restoreState(main_pane_state_);
        statusBar()->setVisible(QSettings().value("prefs/helpLine", true).toBool());
        main_pane_ = false;
    }
}

QMenu* MainWindow::windows_menu() {
    auto* m = new QMenu("Windows", this);
    m->addAction("USD Layer Editor", [this] { show_usd_layer_editor(this); });
    m->addAction("USD Stats", [this] { show_usd_stats(this); });
    QMenu* ws = m->addMenu("Workspaces");
    for (const char* n : {"General", "Modeling", "Rigging", "Animation", "UV Editing"}) {
        QString name = n;
        ws->addAction(name, [this, name] { apply_workspace(name); });
    }
    ws->addSeparator();
    ws->addAction("Save Current Workspace As...", [this] {
        bool ok = false;
        QString n = QInputDialog::getText(this, "Save Workspace", "Name:", QLineEdit::Normal, "myWorkspace", &ok);
        if (ok && !n.isEmpty()) QSettings().setValue("workspaces/" + n, saveState());
    });
    QMenu* mine = ws->addMenu("Saved");
    connect(mine, &QMenu::aboutToShow, this, [this, mine] {
        mine->clear();
        QSettings s;
        s.beginGroup("workspaces");
        for (const QString& k : s.childKeys()) mine->addAction(k, [this, k] { apply_workspace(k); });
        if (mine->isEmpty()) mine->addAction("(none)")->setEnabled(false);
    });
    m->addSeparator();
    QMenu* ge = m->addMenu("General Editors");
    auto named = [this](QMenu* menu, const QString& label, const QString& id) { menu->addAction(label, [this, id] { show_named_window(id); }); };
    named(ge, "Attribute Spread Sheet", "spreadSheet");
    named(ge, "Component Editor", "componentEditor");
    named(ge, "Connection Editor", "connectionEditor");
    named(ge, "Hypergraph: Hierarchy", "hypergraphHierarchy");
    named(ge, "Hypergraph: Connections", "hypergraphConnections");
    named(ge, "Namespace Editor", "namespaceEditor");
    named(ge, "Channel Control", "channelControl");
    ge->addSeparator();
    ge->addAction("Script Editor", this, &MainWindow::show_script_editor);
    ge->addAction("Attribute Editor\tCtrl+A", this, &MainWindow::show_attribute_editor);
    ge->addAction("Channel Box / Layer Editor", [this] { channel_dock_->show(); channel_dock_->raise(); });
    QMenu* model_ed = m->addMenu("Modeling Editors");
    model_ed->addAction("UV Set Editor", [this] { show_named_window("uvSetEditor"); });
    model_ed->addAction("UV Editor\tF12", this, &MainWindow::show_uv_editor);
    named(model_ed, "Modeling Toolkit", "modelingToolkit");
    QMenu* anim_ed = m->addMenu("Animation Editors");
    anim_ed->addAction("Graph Editor", this, &MainWindow::show_graph_editor);
    anim_ed->addAction("Dope Sheet", this, &MainWindow::show_dope_sheet);
    named(anim_ed, "Shape Editor", "shapeEditor");
    if (has_command_named("expression")) named(anim_ed, "Expression Editor", "expressionEditor");
    else todo(anim_ed, "Expression Editor");
    QMenu* render_ed = m->addMenu("Rendering Editors");
    render_ed->addAction("Hypershade", this, &MainWindow::show_hypershade);
    QMenu* rig_ed = m->addMenu("Rigging Editors");
    rig_ed->addAction("Node Editor", this, &MainWindow::show_node_editor);
    rig_ed->addAction("Set Driven Key", this, &MainWindow::show_set_driven_key);
    rig_ed->addAction("Paint Skin Weights", [this] { set_tool("paintSkin"); show_tool_settings(); });
    m->addAction("Outliner", [this] { outliner_dock_->show(); outliner_dock_->raise(); });
    m->addSeparator();
    QMenu* layouts = m->addMenu("View Arrangement");
    struct L { const char* label; int kind; };
    for (L l : {L{"Single Pane", 0}, L{"Two Panes Side by Side", 1}, L{"Two Panes Stacked", 2}, L{"Three Panes", 3}, L{"Four Panes", 4}}) {
        int k = l.kind;
        layouts->addAction(l.label, [this, k] { panels_->set_layout(k); });
    }
    m->addAction("Main Pane Only\tCtrl+Space", [this] { toggle_main_pane(); });
    m->addSeparator();
    m->addAction("Tool Settings...\tCtrl+Shift+T", this, &MainWindow::show_tool_settings);
    QMenu* prefs = m->addMenu("Settings/Preferences");
    prefs->addAction("Preferences...", this, &MainWindow::show_preferences);
    prefs->addAction("Hotkey Editor...", [this] { show_hotkey_editor(); });
    prefs->addAction("Shelf Editor...", [this] { shelf_bar_->open_editor(); });
    return m;
}

// ---------------------------------------------------------------------------------------------------------------------
// Small actions

void MainWindow::toggle_view_flag(const QString& key) {
    ViewportWidget* v = active_view();
    if (!v) return;
    bool on = parse(v->get("disp")).toObject()[key].toBool();
    v->set("disp", QString("{\"%1\":%2}").arg(key, on ? "false" : "true"));
}

void MainWindow::step_time(int by) {
    time_slider_->set_time(time_slider_->time() + by);
}

void MainWindow::step_key(const QString& which) {
    QJsonValue f = ask("findKeyframe", QJsonObject{{"which", which}});
    if (f.isDouble()) time_slider_->set_time(f.toDouble());
}

void MainWindow::scale_manip(double by) {
    QSettings s;
    double m = qBound(0.25, s.value("prefs/manipSize", 1.0).toDouble() + by, 4.0);
    s.setValue("prefs/manipSize", m);
    apply_prefs();
}

void MainWindow::nudge(int dx, int dy) {
    // A pixel in the view's plane, as the reference's Alt+arrow.
    ViewportWidget* v = active_view();
    if (!v) return;
    double upp = parse(v->get("unitsPerPixel")).toDouble(0.01);
    QJsonObject ax = parse(v->get("viewAxes")).toObject();
    QJsonArray r = ax["right"].toArray(), u = ax["up"].toArray();
    QJsonArray by;
    for (int i = 0; i < 3; ++i) by.append((r[i].toDouble() * dx + u[i].toDouble() * dy) * upp);
    bool comps = state()["mode"].toString() != "object";
    exec(comps ? "moveComponents" : "move", QJsonObject{{"by", by}});
}

void MainWindow::show_history_manips() {
    // The node's handles in the view, and its numbers in the in-view editor.
    set_tool("showManip");
    help_line_->setText("Show Manipulator Tool: drag a handle to change the node's attribute.");
}

void MainWindow::add_to_shelf(QAction* a) {
    QString label = a->text().section('\t', 0, 0);
    QString code = a->property("melCommand").toString();
    if (code.isEmpty()) {
        // Items without a command of their own run by their place in the menus.
        QStringList path{label};
        for (QObject* o = a->parent(); o; o = o->parent())
            if (auto* m = qobject_cast<QMenu*>(o)) path.prepend(m->title());
        shelf_bar_->add_to_current(label, QString(), path.join("/"), "menu");
    } else {
        shelf_bar_->add_to_current(label, a->property("shelfIcon").toString(), code, "mel");
    }
    log(QString("// Added %1 to the shelf //").arg(label));
}

void MainWindow::run_menu_path(const QString& path) {
    QStringList parts = path.split('/');
    QList<QAction*> level;
    for (QMenu* m : set_menus_) level << m->menuAction();
    QAction* found = nullptr;
    for (const QString& part : parts) {
        found = nullptr;
        for (QAction* a : level)
            if (a->text().section('\t', 0, 0) == part) {
                found = a;
                break;
            }
        if (!found) {
            log(QString("// Warning: %1 is not in these menus //").arg(path), 4);
            return;
        }
        if (found->menu()) level = found->menu()->actions();
    }
    if (found) found->trigger();
}

void MainWindow::view_scrub(QObject* o, QEvent* e) {
    auto* me = static_cast<QMouseEvent*>(e);
    Q_UNUSED(o);
    if (e->type() == QEvent::MouseButtonPress && me->button() == Qt::LeftButton) {
        scrubbing_ = true;
        scrub_from_ = me->position().x();
        scrub_time_ = time_slider_->time();
    } else if (e->type() == QEvent::MouseMove && scrubbing_) {
        // A frame per four pixels.
        double to = scrub_time_ + std::round((me->position().x() - scrub_from_) / 4.0);
        if (view_hold_ == "scrub") time_slider_->set_time(to);
    } else if (e->type() == QEvent::MouseButtonRelease && scrubbing_) {
        scrubbing_ = false;
        double by = std::round((me->position().x() - scrub_from_) / 4.0);
        if (view_hold_ == "nudgeKeys" && by != 0) {
            // I: the keys on this frame move with the drag.
            exec("keyframe", QJsonObject{{"edit", true}, {"time", QString("%1").arg(scrub_time_)}, {"relative", true}, {"timeChange", by}});
            time_slider_->set_time(scrub_time_ + by);
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Marking menus

void MainWindow::tool_marking_menu(QPoint global, const QString& hold) {
    QList<MarkItem> items;
    QString kind = hold == "rotate" ? "manipRotateContext" : hold == "scale" ? "manipScaleContext" : "manipMoveContext";
    QString ctx = hold == "rotate" ? "Rotate" : hold == "scale" ? "Scale" : "Move";
    auto mode = [kind, ctx](int m) {
        return [kind, ctx, m] { exec_quiet(kind, QJsonObject{{"edit", true}, {"args", QJsonArray{ctx}}, {"mode", m}}); };
    };
    auto flip = [kind, ctx](const char* flag) {
        QString f = flag;
        return [kind, ctx, f] {
            bool on = ask(kind, QJsonObject{{"query", true}, {"queried", QJsonArray{f}}, {"args", QJsonArray{ctx}}}).toBool();
            exec_quiet(kind, QJsonObject{{"edit", true}, {"args", QJsonArray{ctx}}, {f, !on}});
        };
    };
    auto soft = [](bool on) { return [on] { exec("softSelect", QJsonObject{{"softSelectEnabled", on ? 1 : 0}}); }; };
    auto step = [this](const char* key, double val) {
        QString k = key;
        return [this, k, val] {
            for (ViewportWidget* v : panels_->views()) v->set(k, QString::number(val));
        };
    };
    if (hold == "select") {
        items = {{"Select Tool", 0, [this] { set_tool("select"); }}, {"Lasso Tool", 2, [this] { set_tool("lasso"); }}, {"Paint Selection Tool", 6, [this] { set_tool("paintSelect"); }},
                 {"Soft Select", 4, soft(true)}, {"Soft Select Off", -1, soft(false)}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    } else if (hold == "move") {
        items = {{"World", 0, mode(2)}, {"Object", 2, mode(0)}, {"Component", 4, mode(10)}, {"Parent", 6, mode(1)}, {"Normal", 1, mode(3)},
                 {"Soft Select", 3, soft(true)}, {"Soft Select Off", 5, soft(false)}, {"Custom", 7, mode(6)},
                 {"Along Rotation Axis", -1, mode(4)}, {"Along Live Object Axis", -1, mode(5)}, {"Tweak Mode", -1, flip("tweakMode")}, {"Preserve Children", -1, flip("preserveChildPosition")},
                 {"Step Snap Off", -1, step("stepMove", 0)}, {"Step Snap 1", -1, step("stepMove", 1)}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    } else if (hold == "rotate") {
        items = {{"World", 0, mode(1)}, {"Object", 2, mode(0)}, {"Component", 4, mode(10)}, {"Gimbal", 6, mode(2)}, {"Custom", 7, mode(3)},
                 {"Step Snap Off", 3, step("stepRotate", 0)}, {"Step Snap 15", 5, step("stepRotate", 15)}, {"Preserve Children", -1, flip("preserveChildPosition")}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    } else if (hold == "scale") {
        items = {{"World", 0, mode(2)}, {"Object", 2, mode(0)}, {"Component", 4, mode(10)}, {"Normal", 6, mode(3)}, {"Parent", 1, mode(1)}, {"Custom", 7, mode(6)},
                 {"Soft Select", 3, soft(true)}, {"Soft Select Off", 5, soft(false)}, {"Preserve Children", -1, flip("preserveChildPosition")}, {"Tool Settings...", -1, [this] { show_tool_settings(); }}};
    }
    if (!items.isEmpty()) MarkingMenu::show_at(global, items);
}

QList<MarkItem> MainWindow::hotbox_zone(int zone) {
    QList<MarkItem> items;
    auto lay = [this](int k) { return [this, k] { panels_->set_layout(k); }; };
    auto cam = [this](const char* c) {
        QString n = c;
        return [this, n] { if (ViewportWidget* v = active_view()) v->set("camera", "\"" + n + "\""); };
    };
    auto shade = [this](const char* key, const char* val) {
        QString k = key, j = val;
        return [this, k, j] { if (ViewportWidget* v = active_view()) v->set(k, j); };
    };
    auto mask_all = [](bool on) {
        return [on] {
            QJsonObject c;
            for (const char* f : {"polymesh", "nurbsSurface", "nurbsCurve", "joint", "ikHandle", "handle", "cluster", "nonlinear", "locator", "camera", "group"}) c[f] = on;
            exec_quiet("selectType", c);
        };
    };
    switch (zone) {
    case 0:
        items = {{"Single Pane", 0, lay(0)}, {"Two Side by Side", 2, lay(1)}, {"Two Stacked", 4, lay(2)}, {"Four Panes", 6, lay(4)}, {"Three Panes", 1, lay(3)}};
        break;
    case 1:
        items = {{"Wireframe", 0, shade("wireframe", "true")}, {"Smooth Shade", 4, shade("wireframe", "false")}, {"X-Ray", 2, [this] { toggle_view_flag("xray"); }},
                 {"Wireframe on Shaded", 6, [this] { if (ViewportWidget* v = active_view()) v->set("wireOnShaded", v->get("wireOnShaded") == "true" ? "false" : "true"); }}};
        break;
    case 2:
        items = {{"Top View", 0, cam("top")}, {"Right View", 2, cam("side")}, {"Front View", 4, cam("front")}, {"Perspective", 6, cam("persp")}};
        break;
    case 3:
        items = {{"All Objects On", 0, mask_all(true)}, {"All Objects Off", 4, mask_all(false)}, {"Vertex", 2, [this] { set_mode("vertex"); }}, {"Edge", 1, [this] { set_mode("edge"); }},
                 {"Face", 3, [this] { set_mode("face"); }}, {"Object Mode", 6, [this] { set_mode("object"); }}};
        break;
    default: break;
    }
    return items;
}

void MainWindow::marking_menu(QPoint global, int mods) {
    ViewportWidget* v = active_view();
    QString mode = state()["mode"].toString();
    QList<MarkItem> items;
    auto run = [](const QString& c, const QJsonObject& call = QJsonObject()) { return [c, call] { exec(c, call); }; };
    if ((mods & 1) && (mods & 2)) {
        // Ctrl+Shift: primitives, as the reference's.
        items = {{"Sphere", 0, run("polySphere")}, {"Cube", 2, run("polyCube")}, {"Cylinder", 4, run("polyCylinder")}, {"Plane", 6, run("polyPlane")},
                 {"Cone", 1, run("polyCone")}, {"Torus", 3, run("polyTorus")}, {"Disc", 5, run("polyDisc")}, {"Pipe", 7, run("polyPipe")}};
    } else if (mods & 1) {
        // Shift: the modelling tools for the current component type.
        if (mode == "face") {
            items = {{"Extrude Face", 0, run("polyExtrude")}, {"Bevel Face", 4, run("polyBevel")}, {"Bridge Face", 2, run("polyBridge")},
                     {"Inset Face", 6, run("polyInset")}, {"Poke Face", 1, run("polyPoke")}, {"Duplicate Face", 7, run("polyDuplicateFaces")},
                     {"Extract Face", 3, run("polyExtract")}, {"Delete Face", 5, run("deleteComponents")},
                     {"Triangulate", -1, run("polyTriangulate")}, {"Quadrangulate", -1, run("polyQuadrangulate")}, {"Flip", -1, run("polyFlip")}, {"Multi-Cut", -1, [this] { set_tool("multiCut"); }}};
        } else if (mode == "edge") {
            items = {{"Extrude Edge", 0, run("polyExtrude")}, {"Bevel Edge", 4, run("polyBevel")}, {"Bridge", 2, run("polyBridge")},
                     {"Insert Edge Loop", 6, [this] { set_tool("insertLoop"); }}, {"Collapse Edge", 1, run("polyCollapse")}, {"Delete Edge", 5, run("polyDissolve")},
                     {"Offset Edge Loop", 7, [this] { set_tool("offsetLoop"); }}, {"Connect", 3, run("polyConnect")},
                     {"Spin Edge", -1, run("polySpinEdge")}, {"Fill Hole", -1, run("polyFillHole")}, {"Soften Edge", -1, run("polySoftEdge", {{"angle", 180.0}})},
                     {"Harden Edge", -1, run("polySoftEdge", {{"angle", 0.0}})}, {"Slide Edge", -1, [this] { set_tool("slideEdge"); }}, {"Multi-Cut", -1, [this] { set_tool("multiCut"); }}};
        } else if (mode == "vertex") {
            items = {{"Merge Vertices", 0, run("polyMerge")}, {"Merge to Center", 4, run("polyMergeCenter")}, {"Connect", 2, run("polyConnect")},
                     {"Delete Vertex", 6, run("polyDissolve")}, {"Target Weld", 1, [this] { set_tool("targetWeld"); }}, {"Average Vertices", 7, run("polyAverageVertex")},
                     {"Multi-Cut", -1, [this] { set_tool("multiCut"); }}};
        } else {
            items = {{"Combine", 0, run("polyCombine")}, {"Separate", 4, run("polySeparate")}, {"Smooth", 2, run("polySmooth")},
                     {"Mirror", 6, run("polyMirror")}, {"Boolean Union", 1, run("polyBoolean", {{"operation", "union"}})},
                     {"Boolean Difference", 7, run("polyBoolean", {{"operation", "difference"}})},
                     {"Delete History", -1, run("delete", {{"constructionHistory", true}})}, {"Center Pivot", -1, run("centerPivot")},
                     {"Freeze Transformations", -1, run("freezeTransforms")}, {"Create Polygon", -1, [this] { set_tool("createPolygon"); }}, {"Quad Draw", -1, [this] { set_tool("quadDraw"); }}};
        }
    } else if (mods & 2) {
        // Ctrl: selection conversion.
        items = {{"To Edges", 0, run("convertSelection", args(QJsonArray{"edge"}))}, {"To Faces", 4, run("convertSelection", args(QJsonArray{"face"}))},
                 {"To Vertices", 6, run("convertSelection", args(QJsonArray{"vertex"}))}, {"Grow", 2, run("growSelection")},
                 {"Edge Ring", 1, run("selectLoop", {{"ring", true}})}, {"Edge Loop", 7, run("selectLoop")},
                 {"Shrink", -1, run("shrinkSelection")}, {"Border", -1, run("selectBorder")}, {"Shell", -1, [this] { select_shell(false); }}, {"Shell Border", -1, [this] { select_shell(true); }}};
    } else {
        QString hit;
        if (v) {
            QPoint local = v->mapFromGlobal(global);
            double dpr = v->devicePixelRatioF();
            hit = take(aztek_view_hit(v->id(), local.x() * dpr, local.y() * dpr));
        }
        // Not on a mesh: the object nearest the cursor might be a joint or curve under the selection.
        QString target = hit;
        if (target.isEmpty()) {
            QStringList sel = selection_names();
            if (!sel.isEmpty() && (mode == "object")) {
                QString t = ask("nodeType", args(QJsonArray{sel.last()})).toString();
                bool curve = !ask("listRelatives", QJsonObject{{"args", QJsonArray{sel.last()}}, {"shapes", true}, {"type", "nurbsCurve"}}).toArray().isEmpty();
                if (t == "joint" || curve) target = sel.last();
            }
        }
        if (!target.isEmpty()) {
            object_marking_items(target, global, items);
        } else {
            items = {{"Select All", -1, [] { mel("SelectAll;"); }},
                     {"Deselect", -1, run("select", {{"clear", true}})},
                     {"Undo", -1, [] { aztek_undo(); Host::get()->poke(); }},
                     {"Redo", -1, [] { aztek_redo(); Host::get()->poke(); }},
                     {"Repeat Last", -1, [this] { repeat_last(); }},
                     {"Frame All", -1, [v] { if (v) v->set("frame", "\"all\""); }},
                     {"Object Mode", 1, [this] { set_mode("object"); }}};
        }
    }
    MarkingMenu::show_at(global, items);
}

void MainWindow::object_marking_items(const QString& hit, QPoint, QList<MarkItem>& items) {
    auto mode_to = [hit](const QString& m) {
        return [hit, m] {
            // Hilite the object under the cursor, then switch.
            bool in = false;
            for (const QJsonValue& s : state()["selection"].toArray())
                if (s.toString().section('.', 0, 0) == hit) in = true;
            if (!in) exec("select", QJsonObject{{"args", QJsonArray{hit}}});
            exec("selectMode", QJsonObject{{"args", QJsonArray{m}}});
        };
    };
    auto run = [](const QString& c, const QJsonObject& call = QJsonObject()) { return [c, call] { exec(c, call); }; };
    QString type = ask("nodeType", args(QJsonArray{hit})).toString();
    QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{hit}}, {"shapes", true}}).toArray();
    QString shape_type = shapes.isEmpty() ? QString() : ask("nodeType", args(QJsonArray{shapes.at(0)})).toString();
    QList<MarkItem> common{{"Select", -1, run("select", args(QJsonArray{hit}))},
                           {"Select Hierarchy", -1, [hit] {
                                exec("select", QJsonObject{{"args", QJsonArray{hit}}});
                                exec("selectHierarchy");
                            }},
                           {"Attribute Editor...", -1, [this, hit] {
                                exec("select", QJsonObject{{"args", QJsonArray{hit}}});
                                show_attribute_editor();
                            }},
                           {"Delete", -1, run("delete", args(QJsonArray{hit}))}};
    // Each history node, as the reference's RMB menu lists inputs.
    QList<MarkItem> history;
    for (const QJsonValue& h : ask("listHistory", args(QJsonArray{hit})).toArray()) {
        QString node = h.toString();
        if (node == hit || shapes.contains(node)) continue;
        if (history.size() < 8) history.append({"Input: " + node, -1, [this, node] {
            exec("select", QJsonObject{{"args", QJsonArray{node}}});
            show_attribute_editor();
        }});
    }
    if (type == "joint") {
        items = {{"Joint", 0, run("select", args(QJsonArray{hit}))}, {"Object Mode", 1, mode_to("object")},
                 {"Select Hierarchy", 2, [hit] { exec("select", QJsonObject{{"args", QJsonArray{hit}}}); exec("selectHierarchy"); }}};
        // A joint that drives a skin: Paint Weights opens the brush on that influence.
        QJsonArray dest = ask("listConnections", QJsonObject{{"args", QJsonArray{hit}}, {"type", "skinCluster"}}).toArray();
        if (!dest.isEmpty())
            items.append({"Paint Weights", 4, [this, hit, dest] {
                QJsonArray geo = ask("skinCluster", QJsonObject{{"args", QJsonArray{dest.at(0)}}, {"query", true}, {"geometry", true}}).toArray();
                if (!geo.isEmpty()) exec("select", args(QJsonArray{geo.at(0)}));
                set_tool("paintSkin");
                show_tool_settings();
                exec_quiet("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"influence", hit}});
            }});
    } else if (shape_type == "nurbsCurve") {
        items = {{"Control Vertex", 0, mode_to("vertex")}, {"Object Mode", 1, mode_to("object")},
                 {"Edit Point", 2, {}, false}, {"Hull", 6, [hit] {
                      for (const QJsonValue& s : ask("listRelatives", QJsonObject{{"args", QJsonArray{hit}}, {"shapes", true}}).toArray()) exec("setAttr", args(QJsonArray{s.toString() + ".dispHull", true}));
                  }},
                 {"Curve Point", 4, {}, false}};
    } else {
        items = {{"Edge", 0, mode_to("edge")}, {"Object Mode", 1, mode_to("object")}, {"UV", 2, {}, false}, {"Vertex Face", 3, {}, false},
                 {"Face", 4, mode_to("face")}, {"UV Shell", 5, {}, false}, {"Vertex", 6, mode_to("vertex")}, {"Multi", 7, {}, false}};
    }
    items.append(common);
    if (shape_type == "mesh" || shape_type == "nurbsSurface") {
        items.append({"Assign New Material...", -1, [this, hit] { assign_new_material(hit); }});
        items.append({"Assign Existing Material...", -1, [this, hit] {
            QMenu menu;
            assign_existing_material_menu(&menu, hit);
            menu.exec(QCursor::pos());
        }});
        items.append({"Material Attributes...", -1, [this, hit] { material_attributes(hit); }});
    }
    items.append(history);
}

} // namespace az
