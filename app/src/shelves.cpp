#include "shelves.h"
#include "aztek.h"
#include "envvars.h"
#include "host.h"
#include "icons.h"
#include "icon_names.h"

#include <QApplication>
#include <QComboBox>
#include <QDialog>
#include <QDir>
#include <QDirIterator>
#include <QStandardPaths>
#include <QDrag>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QFile>
#include <QFileDialog>
#include <QFileInfo>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QInputDialog>
#include <QJsonDocument>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QMimeData>
#include <QMouseEvent>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QRegularExpression>
#include <QSettings>
#include <QTabBar>
#include <QTabWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QVBoxLayout>

namespace az {

namespace {

QJsonObject item(const char* label, const char* ic, const char* command, const char* language = "mel") {
    return QJsonObject{{"label", label}, {"icon", ic}, {"command", command}, {"language", language}};
}

QJsonObject shelf(const char* name, const QJsonArray& items) {
    return QJsonObject{{"name", name}, {"items", items}};
}

const char* kMoveMime = "application/x-aztek-shelf-move";

// Where the shelves are kept as files, as the reference keeps shelf_Name.mel in its prefs folder (#117).
QString shelves_folder() {
    return take(aztek_preferences_dir()) + "/prefs/shelves";
}

// Pictures the user keeps for shelf buttons: searched by the icon browser, which opens files there.
QString icons_folder() {
    return take(aztek_preferences_dir()) + "/prefs/icons";
}

QString mel_quote(QString v) {
    v.replace("\\", "\\\\").replace("\"", "\\\"").replace("\n", "\\n");
    return "\"" + v + "\"";
}

// A drawn icon's name, or a picture's file found on the icon path or in the icons folder.
bool known_icon(const QString& name) {
    if (name.isEmpty()) return false;
    for (const char* n : kIconNames)
        if (name == n) return true;
    return !find_image(name).isEmpty() || QFileInfo(QDir(icons_folder()).filePath(name)).isFile();
}

QString image_file(const QString& name) {
    QString f = find_image(name);
    if (f.isEmpty() && QFileInfo(QDir(icons_folder()).filePath(name)).isFile()) f = QDir(icons_folder()).filePath(name);
    return f;
}

// The icon browser: the drawn icons, and pictures on the icon path and in the icons folder, with a search; Files...
// takes any picture. Empty when cancelled.
QString pick_icon(QWidget* parent, const QString& current) {
    QDialog d(parent);
    d.setWindowTitle("Choose an Icon");
    auto* v = new QVBoxLayout(&d);
    auto* search = new QLineEdit(&d);
    search->setPlaceholderText("Search icons...");
    search->setClearButtonEnabled(true);
    v->addWidget(search);
    auto* list = new QListWidget(&d);
    list->setViewMode(QListView::IconMode);
    list->setIconSize(QSize(32, 32));
    list->setGridSize(QSize(92, 64));
    list->setResizeMode(QListView::Adjust);
    list->setWordWrap(true);
    list->setUniformItemSizes(true);
    v->addWidget(list, 1);
    for (const char* n : kIconNames) {
        auto* it = new QListWidgetItem(icon(n), n, list);
        it->setData(Qt::UserRole, QString(n));
        it->setToolTip(n);
    }
    QStringList dirs{icons_folder()};
    for (QString dir : qEnvironmentVariable("XBMLANGPATH").split(QDir::listSeparator(), Qt::SkipEmptyParts)) {
        if (dir.endsWith("%B")) dir.chop(2);
        dirs << dir;
    }
    for (const QString& dir : dirs) {
        QDirIterator files(dir, {"*.png", "*.svg", "*.xpm", "*.jpg", "*.bmp"}, QDir::Files);
        while (files.hasNext()) {
            const QString f = files.next();
            auto* it = new QListWidgetItem(QIcon(f), QFileInfo(f).fileName(), list);
            it->setData(Qt::UserRole, f);
            it->setToolTip(f);
        }
    }
    for (int i = 0; i < list->count(); ++i)
        if (list->item(i)->data(Qt::UserRole).toString() == current) list->setCurrentRow(i);
    QObject::connect(search, &QLineEdit::textChanged, &d, [list](const QString& t) {
        for (int i = 0; i < list->count(); ++i) list->item(i)->setHidden(!list->item(i)->text().contains(t, Qt::CaseInsensitive));
    });
    QString chosen;
    auto* row = new QHBoxLayout();
    auto* files = new QPushButton("Files...", &d);
    files->setToolTip("A picture from disk; the browser opens in the icons folder, " + icons_folder());
    auto* ok = new QPushButton("Choose", &d);
    auto* cancel = new QPushButton("Cancel", &d);
    ok->setDefault(true);
    row->addWidget(files);
    row->addStretch(1);
    row->addWidget(ok);
    row->addWidget(cancel);
    v->addLayout(row);
    QObject::connect(files, &QPushButton::clicked, &d, [&d, &chosen] {
        QDir().mkpath(icons_folder());
        const QString f = QFileDialog::getOpenFileName(&d, "Icon File", icons_folder(), "Pictures (*.png *.svg *.xpm *.jpg *.bmp)");
        if (f.isEmpty()) return;
        chosen = f;
        d.accept();
    });
    auto take = [&d, &chosen, list] {
        if (!list->currentItem()) return;
        chosen = list->currentItem()->data(Qt::UserRole).toString();
        d.accept();
    };
    QObject::connect(ok, &QPushButton::clicked, &d, take);
    QObject::connect(list, &QListWidget::itemDoubleClicked, &d, take);
    QObject::connect(cancel, &QPushButton::clicked, &d, &QDialog::reject);
    d.resize(620, 480);
    return d.exec() == QDialog::Accepted ? chosen : QString();
}

// An icon field with a button that opens the browser.
QWidget* icon_field(QWidget* parent, QLineEdit* field) {
    auto* w = new QWidget(parent);
    auto* h = new QHBoxLayout(w);
    h->setContentsMargins(0, 0, 0, 0);
    h->addWidget(field, 1);
    auto* b = new QPushButton("Browse...", w);
    b->setToolTip("Choose from Aztek's icons, or a picture on disk");
    QObject::connect(b, &QPushButton::clicked, w, [w, field] {
        const QString c = pick_icon(w, field->text());
        if (!c.isEmpty()) field->setText(c);
    });
    h->addWidget(b);
    return w;
}
const char* kLangMime = "application/x-aztek-script-language";

QJsonObject rendering_shelf() {
    QJsonArray items{
        item("Render", "renderFrame", "RenderIntoNewWindow;"), item("IPR", "renderIpr", "IPRRenderIntoNewWindow;"), item("Render Settings", "renderSettings", "RenderGlobalsWindow;"),
        item("Render View", "renderView", "RenderViewWindow;"), item("Sky Dome Light", "skyDome", "shadingNode -asLight aiSkyDomeLight;"),
        item("Area Light", "haroldAreaLight", "shadingNode -asLight aiAreaLight;"), item("Standard Surface", "standardSurface", "shadingNode -asShader standardSurface;")};
    return shelf("Rendering", items);
}

// The Hair shelf (XJEN): the editor, a new description, and the grooming tools (icons borrowed from the sculpting brushes).
QJsonObject hair_shelf() {
    QJsonArray items{item("Interactive Groom Editor", "layer", "InteractiveGroomEditor;"), item("Create Interactive Groom Splines", "brushSculpt", "xjenCreateGroom;")};
    struct T { const char* id; const char* label; const char* icon; };
    for (T t : {T{"density", "Density", "brushSpray"}, T{"place", "Place", "brushImprint"}, T{"length", "Length", "brushBulge"}, T{"cut", "Cut", "brushKnife"},
                T{"width", "Width", "brushWax"}, T{"twist", "Twist", "brushRelax"}, T{"comb", "Comb", "brushSmear"}, T{"grab", "Grab", "brushGrab"},
                T{"smooth", "Smooth", "brushSmooth"}, T{"noise", "Noise", "brushFoamy"}, T{"clump", "Clump", "brushPinch"}, T{"part", "Part", "brushScrape"},
                T{"freeze", "Freeze", "brushFreeze"}, T{"select", "Select", "select"}})
        items.append(QJsonObject{{"label", t.label}, {"icon", t.icon}, {"command", QString("xjenGroomCtx -tool \"%1\"; InteractiveGroomTool;").arg(t.id)}, {"language", "mel"}});
    return shelf("Hair", items);
}

} // namespace

QJsonArray ShelfBar::defaults() {
    QJsonArray poly{
        item("Sphere", "sphere", "polySphere;"), item("Cube", "cube", "polyCube;"), item("Cylinder", "cylinder", "polyCylinder;"), item("Cone", "cone", "polyCone;"),
        item("Torus", "torus", "polyTorus;"), item("Plane", "plane", "polyPlane;"), item("Disc", "disc", "polyDisc;"), item("Pipe", "pipe", "polyPipe;"),
        item("Combine", "combine", "polyCombine;"), item("Separate", "separate", "polySeparate;"), item("Smooth", "smooth", "polySubdivide;"),
        item("Union", "boolean", "polyCBoolOp -op 1;"), item("Mirror", "mirror", "polyMirror;"),
        item("Extrude", "extrude", "polyExtrude;"), item("Bridge", "bridge", "polyBridge;"), item("Bevel", "bevel", "polyBevel;"),
        item("Insert Edge Loop", "insertLoop", "SplitEdgeRingTool;"), item("Offset Edge Loop", "offsetLoop", "OffsetEdgeLoopTool;"), item("Multi-Cut", "multiCut", "MultiCutTool;"),
        item("Quad Draw", "quadDraw", "QuadDrawTool;"), item("Merge", "merge", "polyMerge;"), item("Merge to Center", "collapse", "polyMergeCenter;")};
    QJsonArray curves{
        item("CV Curve", "curveCv", "CVCurveTool;"), item("EP Curve", "curveEp", "EPCurveTool;"), item("Circle", "circle", "circle -normal 0 1 0;"), item("Square", "square", "nurbsSquare;"),
        item("Attach", "attachCurve", "attachCurve;"), item("Detach", "detachCurve", "detachCurve;"), item("Rebuild", "rebuildCurve", "rebuildCurve;"),
        item("Reverse", "reverseCurve", "reverseCurve;"), item("Offset", "offsetCurve", "offsetCurve;"),
        item("Loft", "loft", "loft;"), item("Planar", "planarSurface", "planar;"), item("Revolve", "revolve", "revolve;"), item("Extrude", "extrudeCurve", "extrude;")};
    QJsonArray sculpt;
    for (const char* b : {"sculpt", "smooth", "relax", "grab", "pinch", "flatten", "foamy", "spray", "repeat", "imprint", "wax", "scrape", "fill", "knife", "smear", "bulge", "amplify", "freeze"}) {
        QString brush = b;
        QString label = brush.left(1).toUpper() + brush.mid(1);
        QJsonObject o;
        o["label"] = label;
        o["icon"] = "brush" + label;
        o["command"] = QString("sculptMeshCacheCtx -edit -sculptTool \"%1\"; SculptMeshActivateBrushSculpt;").arg(brush);
        o["language"] = "mel";
        sculpt.append(o);
    }
    QJsonArray rig{
        item("Create Joints", "joint", "JointTool;"), item("IK Handle", "ik", "IKHandleTool;"), item("Bind Skin", "skin", "skinCluster;"), item("Paint Weights", "paintSkin", "ArtPaintSkinWeightsToolOptions;"),
        item("Parent", "parentCon", "parentConstraint -maintainOffset;"), item("Point", "pointCon", "pointConstraint;"), item("Orient", "orientCon", "orientConstraint;"),
        item("Scale", "scaleCon", "scaleConstraint;"), item("Aim", "aimCon", "aimConstraint;"), item("Locator", "locator", "spaceLocator;"), item("Group", "group", "group;"),
        item("Cluster", "cluster", "cluster;"), item("Blend Shape", "blendShape", "blendShape;")};
    QJsonArray anim{
        item("Set Key", "key", "setKeyframe;"), item("Delete Keys", "cutKey", "cutKey;"), item("Graph Editor", "graph", "GraphEditor;"), item("Dope Sheet", "dopeSheet", "DopeSheetEditor;"),
        item("Set Driven Key", "drivenKey", "SetDrivenKeyOptions;"), item("Key Translate", "keyMove", "SetKeyTranslate;"), item("Key Rotate", "keyRotate", "SetKeyRotate;"), item("Key Scale", "keyScale", "SetKeyScale;")};
    QJsonArray uv{
        item("UV Editor", "uvEditor", "UVEditor;"), item("Automatic", "uvAuto", "polyAutoProjection;"), item("Planar", "uvPlanar", "polyProjection -type \"planar\";"),
        item("Unfold", "uvUnfold", "u3dUnfold;"), item("Layout", "uvLayout", "u3dLayout;"), item("Cut", "uvCut", "polyMapCut;"), item("Sew", "uvSew", "polyMapSew;")};
    QJsonArray dyn{
        item("Cloth", "cloth", "createCloth;"), item("Collider", "collider", "collider;"), item("Particles", "particles", "particle -p 0 0 0;"),
        item("Emitter", "emitter", "CreateEmitter;"), item("Active Rigid Body", "rigid", "rigidBody -active;"), item("Passive Rigid Body", "rigidPassive", "rigidBody -passive;"),
        item("Gravity", "gravity", "gravity;"), item("Turbulence", "turbulence", "turbulence;"), item("Air", "air", "air;"),
        item("Create Cache", "cache", "dynCache -create;"), item("Interactive Playback", "playDyn", "InteractivePlayback;")};
    QJsonArray general{
        item("Locator", "locator", "spaceLocator;"), item("Group", "group", "group;"), item("Delete History", "history", "DeleteHistory;"), item("Freeze", "transform", "freezeTransforms;"),
        item("Center Pivot", "frameSel", "centerPivot;"), item("New Layer", "layer", "createDisplayLayer;")};
    return QJsonArray{shelf("Poly Modeling", poly), shelf("Curves / Surfaces", curves), shelf("Sculpting", sculpt), shelf("Rigging", rig), shelf("Animation", anim),
                      shelf("UV Editing", uv), shelf("Dynamics", dyn), hair_shelf(), rendering_shelf(), shelf("General", general), shelf("Custom", QJsonArray())};
}

// One shelf's buttons: takes dropped scripts, and buttons dragged with the middle button.
class ShelfPage : public QToolBar {
public:
    ShelfPage(ShelfBar* bar, int index, QWidget* parent) : QToolBar(parent), bar_(bar), index_(index) {
        setIconSize(QSize(32, 32));
        setToolButtonStyle(Qt::ToolButtonIconOnly);
        setAcceptDrops(true);
    }
protected:
    void dragEnterEvent(QDragEnterEvent* e) override {
        if (e->mimeData()->hasText() || e->mimeData()->hasFormat(kMoveMime)) e->acceptProposedAction();
    }
    void dragMoveEvent(QDragMoveEvent* e) override { e->acceptProposedAction(); }
    void dropEvent(QDropEvent* e) override;
    void contextMenuEvent(QContextMenuEvent* e) override { bar_->tab_menu(e->globalPos(), index_); }
private:
    ShelfBar* bar_;
    int index_;
};

void ShelfPage::dropEvent(QDropEvent* e) {
    QJsonArray items = bar_->shelves_[index_].toObject()["items"].toArray();
    // Which button it lands before.
    int at = items.size();
    int x = e->position().toPoint().x();
    int i = 0;
    for (QToolButton* b : findChildren<QToolButton*>(QString(), Qt::FindDirectChildrenOnly)) {
        if (b->property("shelfItem").isValid() && x < b->geometry().center().x()) {
            at = i;
            break;
        }
        if (b->property("shelfItem").isValid()) ++i;
    }
    if (e->mimeData()->hasFormat(kMoveMime)) {
        QStringList p = QString::fromUtf8(e->mimeData()->data(kMoveMime)).split(':');
        int from_shelf = p[0].toInt(), from = p[1].toInt();
        QJsonObject s0 = bar_->shelves_[from_shelf].toObject();
        QJsonArray src = s0["items"].toArray();
        QJsonValue moved = src.at(from);
        src.removeAt(from);
        s0["items"] = src;
        bar_->shelves_[from_shelf] = s0;
        items = bar_->shelves_[index_].toObject()["items"].toArray();
        if (from_shelf == index_ && from < at) --at;
        items.insert(qBound(0, at, int(items.size())), moved);
    } else {
        QString code = e->mimeData()->text();
        QString lang = e->mimeData()->hasFormat(kLangMime) ? QString::fromUtf8(e->mimeData()->data(kLangMime)) : QString("mel");
        QString label = code.trimmed().section(QRegularExpression("[\\s(;]"), 0, 0).left(12);
        QJsonObject o{{"label", label}, {"icon", ""}, {"command", code}, {"language", lang}};
        items.insert(qBound(0, at, int(items.size())), o);
    }
    QJsonObject s = bar_->shelves_[index_].toObject();
    s["items"] = items;
    bar_->shelves_[index_] = s;
    e->acceptProposedAction();
    bar_->save();
    QTimer::singleShot(0, bar_, [b = bar_] { b->rebuild(); });
}

namespace {

// A shelf button: left runs, middle drags it to a new place, right is its menu.
class ShelfButton : public QToolButton {
public:
    std::function<void(QPoint)> on_menu;
    int shelf = 0, index = 0;
protected:
    void mousePressEvent(QMouseEvent* e) override {
        if (e->button() == Qt::MiddleButton) {
            auto* drag = new QDrag(this);
            auto* m = new QMimeData;
            m->setData(kMoveMime, QString("%1:%2").arg(shelf).arg(index).toUtf8());
            drag->setMimeData(m);
            drag->setPixmap(icon().pixmap(32, 32));
            drag->exec(Qt::MoveAction);
            return;
        }
        QToolButton::mousePressEvent(e);
    }
    void contextMenuEvent(QContextMenuEvent* e) override {
        if (on_menu) on_menu(e->globalPos());
    }
};

} // namespace

ShelfBar::ShelfBar(std::function<void(const QString&, const QString&)> run, QWidget* parent) : QWidget(parent), run_(std::move(run)) {
    auto* lay = new QHBoxLayout(this);
    lay->setContentsMargins(0, 0, 0, 0);
    lay->setSpacing(2);
    // The reference's two buttons left of the shelves: a list of the shelves to jump to, and the shelf menu.
    auto* side = new QVBoxLayout();
    side->setContentsMargins(2, 2, 0, 2);
    side->setSpacing(2);
    auto* list = new QToolButton(this);
    list->setObjectName("shelfSide");
    list->setText(QString::fromUtf8("\u2261"));
    list->setToolTip("Shelves");
    list->setAutoRaise(true);
    connect(list, &QToolButton::clicked, this, [this, list] {
        QMenu m;
        for (int i = 0; i < tabs_->count(); ++i) {
            QAction* a = m.addAction(tabs_->tabText(i), [this, i] { tabs_->setCurrentIndex(i); });
            a->setCheckable(true);
            a->setChecked(i == tabs_->currentIndex());
        }
        m.exec(list->mapToGlobal(QPoint(list->width(), 0)));
    });
    auto* gear = new QToolButton(this);
    gear->setObjectName("shelfSide");
    gear->setText(QString::fromUtf8("\u2699"));
    gear->setToolTip("Shelf menu");
    gear->setAutoRaise(true);
    connect(gear, &QToolButton::clicked, this, [this, gear] { tab_menu(gear->mapToGlobal(QPoint(gear->width(), 0)), tabs_->currentIndex()); });
    side->addWidget(list);
    side->addWidget(gear);
    side->addStretch(1);
    lay->addLayout(side);
    tabs_ = new QTabWidget(this);
    tabs_->setObjectName("shelfTabs");
    connect(tabs_, &QTabWidget::currentChanged, this, [this] { emit shelves_changed(); });
    tabs_->setDocumentMode(true);
    tabs_->tabBar()->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(tabs_->tabBar(), &QWidget::customContextMenuRequested, this, [this](const QPoint& p) {
        tab_menu(tabs_->tabBar()->mapToGlobal(p), tabs_->tabBar()->tabAt(p));
    });
    lay->addWidget(tabs_);
    QSettings s;
    QJsonDocument d = QJsonDocument::fromJson(s.value("shelves/json").toByteArray());
    shelves_ = d.isArray() && !d.array().isEmpty() ? d.array() : defaults();
    // Shelves saved before there was a Rendering shelf get it once, before General; removing it afterwards sticks.
    if (!s.value("shelves/hasRendering", false).toBool()) {
        bool there = false;
        int at = shelves_.size();
        for (int k = 0; k < shelves_.size(); ++k) {
            QString n = shelves_.at(k).toObject()["name"].toString();
            if (n == "Rendering") there = true;
            if (n == "General" && at == shelves_.size()) at = k;
        }
        if (!there) shelves_.insert(at, rendering_shelf());
        s.setValue("shelves/hasRendering", true);
        if (!there && d.isArray() && !d.array().isEmpty()) save();
    }
    // Shelves saved before there was a Hair shelf get it once, before Rendering. One saved under its first name, XGen,
    // takes the new name.
    if (!s.value("shelves/hasHair", false).toBool()) {
        bool there = false, renamed = false;
        int at = shelves_.size();
        for (int k = 0; k < shelves_.size(); ++k) {
            QJsonObject o = shelves_.at(k).toObject();
            QString n = o["name"].toString();
            if (n == "XGen" && !o.contains("shared")) {
                o["name"] = "Hair";
                shelves_[k] = o;
                n = "Hair";
                renamed = true;
            }
            if (n == "Hair") there = true;
            if (n == "Rendering" && at == shelves_.size()) at = k;
        }
        if (!there) shelves_.insert(at, hair_shelf());
        s.setValue("shelves/hasHair", true);
        if ((renamed || !there) && d.isArray() && !d.array().isEmpty()) save();
    }
    // Shelf files put in the prefs shelves folder by hand, or kept from another machine: the ones not saved here are
    // read in (#117).
    {
        const QStringList mine = s.value("shelves/files").toStringList();
        QDirIterator files(shelves_folder(), {"shelf_*.mel"}, QDir::Files);
        bool added = false;
        while (files.hasNext()) {
            const QString file = files.next();
            if (mine.contains(QFileInfo(file).fileName())) continue;
            QJsonObject shelf;
            if (!read_mel(file, &shelf)) continue;
            bool there = false;
            for (const QJsonValue& have : std::as_const(shelves_)) there = there || have.toObject()["name"] == shelf["name"];
            if (there) continue;
            shelves_.append(shelf);
            added = true;
        }
        if (added || !s.contains("shelves/files")) save();
    }
    // The shelf path's shelves: shown after the saved ones, read again at each start and never saved.
    for (const QJsonValue& file : ask("aztekEnvironment", QJsonObject{{"shelfFiles", true}}).toArray()) {
        QJsonObject shelf;
        if (!read_mel(file.toString(), &shelf)) continue;
        bool there = false;
        for (const QJsonValue& have : std::as_const(shelves_)) there = there || have.toObject()["name"] == shelf["name"];
        if (there) continue;
        shelf["shared"] = file.toString();
        shelves_.append(shelf);
    }
    rebuild();
    tabs_->setCurrentIndex(s.value("shelves/current", 0).toInt());
    connect(tabs_, &QTabWidget::currentChanged, this, [](int i) { QSettings().setValue("shelves/current", i); });
}

void ShelfBar::set_corner(QWidget* w) {
    tabs_->setUsesScrollButtons(true);
    // It's as tall as the tab row and the shelf together: it sits over their right end, with a gap kept for it in the tabs.
    corner_ = w;
    w->setParent(this);
    corner_gap_ = new QWidget(tabs_);
    tabs_->setCornerWidget(corner_gap_, Qt::TopRightCorner);
    w->installEventFilter(this);  // it can change size (the feedback button folds away)
    w->raise();
    place_corner();
}

bool ShelfBar::eventFilter(QObject* o, QEvent* e) {
    if (o == corner_ && e->type() == QEvent::LayoutRequest) place_corner();
    return QWidget::eventFilter(o, e);
}

void ShelfBar::place_corner() {
    if (!corner_) return;
    QSize size = corner_->sizeHint();
    corner_->setGeometry(width() - size.width() - 4, 4, size.width(), size.height());
    if (corner_gap_ && corner_gap_->width() != size.width() + 8) {
        corner_gap_->setFixedSize(size.width() + 8, 1);
        tabs_->setCornerWidget(corner_gap_, Qt::TopRightCorner);  // the tab row takes the new width
    }
}

void ShelfBar::resizeEvent(QResizeEvent* e) {
    QWidget::resizeEvent(e);
    place_corner();
}

void ShelfBar::save() const {
    QJsonArray own;
    for (const QJsonValue& shelf : shelves_) {
        if (shelf.toObject().contains("shared")) continue;
        // A script's button that runs a function of the script lasts as long as the script's session.
        QJsonObject o = shelf.toObject();
        QJsonArray kept;
        for (const QJsonValue& it : o["items"].toArray())
            if (!it.toObject()["transient"].toBool()) kept.append(it);
        o["items"] = kept;
        own.append(o);
    }
    QSettings settings;
    settings.setValue("shelves/json", QJsonDocument(own).toJson(QJsonDocument::Compact));
    // Each shelf also as the reference's shelf_Name.mel in the prefs shelves folder (#117), which another Aztek or
    // the reference reads; files this wrote for shelves since removed go.
    const QString dir = shelves_folder();
    QDir().mkpath(dir);
    QStringList written;
    for (const QJsonValue& v : own) {
        const QJsonObject shelf = v.toObject();
        const QString file = "shelf_" + shelf["name"].toString().replace(QRegularExpression("[^A-Za-z0-9_]"), "_") + ".mel";
        if (write_mel(QDir(dir).filePath(file), shelf)) written << file;
    }
    for (const QString& old : settings.value("shelves/files").toStringList())
        if (!written.contains(old)) QFile::remove(QDir(dir).filePath(old));
    settings.setValue("shelves/files", written);
}

bool ShelfBar::write_mel(const QString& path, const QJsonObject& shelf) {
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) return false;
    QString name = QFileInfo(path).baseName();
    QString out = "global proc " + name + " () {\n    global string $gBuffStr;\n    global string $gBuffStr0;\n    global string $gBuffStr1;\n\n";
    for (const QJsonValue& v : shelf["items"].toArray()) {
        const QJsonObject it = v.toObject();
        const QString ic = it["icon"].toString();
        out += "    shelfButton\n        -enableCommandRepeat 1\n        -enable 1\n        -width 35\n        -height 34\n        -manage 1\n        -visible 1\n";
        out += "        -label " + mel_quote(it["label"].toString()) + "\n";
        out += "        -annotation " + mel_quote(it["label"].toString()) + "\n";
        if (!ic.isEmpty()) out += "        -image " + mel_quote(ic) + "\n        -image1 " + mel_quote(ic) + "\n";
        out += "        -style \"iconOnly\"\n";
        out += "        -command " + mel_quote(it["command"].toString()) + "\n";
        out += "        -sourceType " + mel_quote(it["language"].toString("mel") == "python" ? "python" : "mel") + "\n    ;\n";
    }
    out += "\n}\n";
    f.write(out.toUtf8());
    return true;
}

void ShelfBar::rebuild() {
    int cur = tabs_->currentIndex();
    tabs_->blockSignals(true);
    while (tabs_->count()) {
        QWidget* w = tabs_->widget(0);
        tabs_->removeTab(0);
        w->deleteLater();
    }
    for (int si = 0; si < shelves_.size(); ++si) {
        QJsonObject sh = shelves_[si].toObject();
        auto* page = new ShelfPage(this, si, tabs_);
        QJsonArray items = sh["items"].toArray();
        for (int i = 0; i < items.size(); ++i) {
            QJsonObject it = items[i].toObject();
            auto* b = new ShelfButton;
            b->shelf = si;
            b->index = i;
            b->setProperty("shelfItem", i);
            QString label = it["label"].toString(), name = it["icon"].toString();
            const QString image = image_file(name);
            b->setIcon(image.isEmpty() ? az::icon(name.isEmpty() ? label : name) : QIcon(image));
            b->setIconSize(QSize(32, 32));
            b->setAutoRaise(true);
            b->setToolTip(label + "\n" + it["command"].toString());
            b->setStatusTip(label);
            QString command = it["command"].toString(), lang = it["language"].toString("mel");
            const QString script = it["scriptPath"].toString();
            if (it["transient"].toBool() && !script.isEmpty())
                connect(b, &QToolButton::clicked, this, [this, script] { if (fire_script) fire_script(script); });
            else
                connect(b, &QToolButton::clicked, this, [this, command, lang] { run_(command, lang); });
            b->on_menu = [this, si, i](QPoint g) { button_menu(g, si, i); };
            page->addWidget(b);
        }
        tabs_->addTab(page, sh["name"].toString());
    }
    tabs_->setCurrentIndex(qBound(0, cur, int(tabs_->count()) - 1));
    tabs_->blockSignals(false);
    emit shelves_changed();
}

QStringList ShelfBar::shelf_names() const {
    QStringList out;
    for (const QJsonValue& v : shelves_) out << v.toObject()["name"].toString();
    return out;
}

int ShelfBar::current() const {
    return qMax(0, tabs_->currentIndex());
}

void ShelfBar::add_script_button(const QString& shelf, const QString& path, const QJsonObject& props) {
    int si = shelf_names().indexOf(shelf);
    if (si < 0) return;
    // As the reference's shelfButton: -label, -image (or -image1), -command with -sourceType, -annotation.
    QString label = props.value("label").toString(props.value("annotation").toString());
    QString image = props.value("image").toString(props.value("image1").toString());
    QJsonValue cmd = props.value("command");
    QString lang = props.value("sourceType").toString("mel").toLower() == "python" ? "python" : "mel";
    QJsonObject item{{"label", label}, {"icon", image}, {"language", lang}, {"scriptPath", path}};
    if (cmd.isString()) item["command"] = cmd.toString();
    else item["transient"] = true;
    if (props.contains("annotation")) item["annotation"] = props["annotation"];
    QJsonObject s = shelves_[si].toObject();
    QJsonArray items = s["items"].toArray();
    items.append(item);
    s["items"] = items;
    shelves_[si] = s;
    save();
    rebuild();
}

void ShelfBar::remove_script_button(const QString& path) {
    for (int si = 0; si < shelves_.size(); ++si) {
        QJsonObject s = shelves_[si].toObject();
        QJsonArray items = s["items"].toArray();
        for (int i = 0; i < items.size(); ++i) {
            if (items[i].toObject()["scriptPath"].toString() != path) continue;
            items.removeAt(i);
            s["items"] = items;
            shelves_[si] = s;
            save();
            rebuild();
            return;
        }
    }
}

void ShelfBar::select_shelf(const QString& name) {
    for (int i = 0; i < tabs_->count(); ++i)
        if (tabs_->tabText(i) == name) tabs_->setCurrentIndex(i);
}

void ShelfBar::add_to_current(const QString& label, const QString& icon_name, const QString& command, const QString& language) {
    int si = qMax(0, tabs_->currentIndex());
    QJsonObject s = shelves_[si].toObject();
    QJsonArray items = s["items"].toArray();
    items.append(QJsonObject{{"label", label}, {"icon", icon_name}, {"command", command}, {"language", language}});
    s["items"] = items;
    shelves_[si] = s;
    save();
    rebuild();
}

void ShelfBar::tab_menu(QPoint global, int index) {
    QMenu m;
    m.addAction("Shelf Editor...", [this] { open_editor(); });
    m.addSeparator();
    m.addAction("New Shelf...", [this] {
        bool ok = false;
        QString n = QInputDialog::getText(this, "New Shelf", "Name:", QLineEdit::Normal, QString("Shelf%1").arg(shelves_.size() + 1), &ok);
        if (!ok || n.trimmed().isEmpty()) return;
        shelves_.append(QJsonObject{{"name", n.trimmed()}, {"items", QJsonArray()}});
        save();
        rebuild();
        tabs_->setCurrentIndex(tabs_->count() - 1);
    });
    if (index >= 0) {
        m.addAction("Rename Shelf...", [this, index] {
            bool ok = false;
            QJsonObject s = shelves_[index].toObject();
            QString n = QInputDialog::getText(this, "Rename Shelf", "Name:", QLineEdit::Normal, s["name"].toString(), &ok);
            if (!ok || n.trimmed().isEmpty()) return;
            s["name"] = n.trimmed();
            shelves_[index] = s;
            save();
            rebuild();
        });
        m.addAction("Delete Shelf", [this, index] {
            if (shelves_.size() < 2) return;
            shelves_.removeAt(index);
            save();
            rebuild();
        });
    }
    m.addSeparator();
    m.addAction("Load Shelf (shelf_*.mel)...", [this] {
        QString f = QFileDialog::getOpenFileName(this, "Load Shelf", QString(), "Shelf files (*.mel)");
        if (!f.isEmpty()) import_mel(f);
    });
    m.addAction("Restore Default Shelves", [this] {
        shelves_ = defaults();
        save();
        rebuild();
    });
    m.exec(global);
}

void ShelfBar::button_menu(QPoint global, int si, int i) {
    QMenu m;
    QJsonObject it = shelves_[si].toObject()["items"].toArray()[i].toObject();
    m.addAction("Run", [this, it] { run_(it["command"].toString(), it["language"].toString("mel")); });
    m.addAction("Edit...", [this, si, i] { edit_item(si, i); });
    m.addAction("Delete", [this, si, i] {
        QJsonObject s = shelves_[si].toObject();
        QJsonArray items = s["items"].toArray();
        items.removeAt(i);
        s["items"] = items;
        shelves_[si] = s;
        save();
        QTimer::singleShot(0, this, [this] { rebuild(); });
    });
    m.exec(global);
}

void ShelfBar::edit_item(int si, int i) {
    QDialog d(this);
    d.setWindowTitle("Edit Shelf Button");
    auto* form = new QFormLayout(&d);
    QJsonObject s = shelves_[si].toObject();
    QJsonArray items = s["items"].toArray();
    QJsonObject it = items[i].toObject();
    auto* label = new QLineEdit(it["label"].toString(), &d);
    auto* ic = new QLineEdit(it["icon"].toString(), &d);
    ic->setToolTip("A drawn icon's name; empty shows the label's initials");
    auto* lang = new QComboBox(&d);
    lang->addItems({"mel", "python"});
    lang->setCurrentText(it["language"].toString("mel"));
    auto* cmd = new QPlainTextEdit(it["command"].toString(), &d);
    auto* ok = new QPushButton("Save", &d);
    form->addRow("Label:", label);
    form->addRow("Icon:", icon_field(&d, ic));
    form->addRow("Language:", lang);
    form->addRow("Command:", cmd);
    form->addRow("", ok);
    connect(ok, &QPushButton::clicked, &d, &QDialog::accept);
    if (d.exec() != QDialog::Accepted) return;
    it["label"] = label->text();
    it["icon"] = ic->text();
    it["language"] = lang->currentText();
    it["command"] = cmd->toPlainText();
    items[i] = it;
    s["items"] = items;
    shelves_[si] = s;
    save();
    QTimer::singleShot(0, this, [this] { rebuild(); });
}

void ShelfBar::open_editor() {
    auto* d = new QDialog(this);
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("Shelf Editor");
    auto* lay = new QHBoxLayout(d);
    auto column = [d, lay](const QString& title, QListWidget*& list) {
        auto* v = new QVBoxLayout();
        v->addWidget(new QLabel(title, d));
        list = new QListWidget(d);
        v->addWidget(list, 1);
        lay->addLayout(v);
        return v;
    };
    QListWidget* shelves = nullptr;
    QListWidget* items = nullptr;
    QVBoxLayout* left = column("Shelves", shelves);
    QVBoxLayout* mid = column("Shelf Contents", items);
    auto* form = new QFormLayout();
    auto* label = new QLineEdit(d);
    auto* ic = new QLineEdit(d);
    auto* lang = new QComboBox(d);
    lang->addItems({"mel", "python"});
    auto* cmd = new QPlainTextEdit(d);
    form->addRow("Label:", label);
    form->addRow("Icon:", icon_field(d, ic));
    form->addRow("Language:", lang);
    form->addRow("Command:", cmd);
    lay->addLayout(form, 1);

    auto fill_shelves = [this, shelves] {
        int r = shelves->currentRow();
        shelves->clear();
        for (const QJsonValue& s : shelves_) shelves->addItem(s.toObject()["name"].toString());
        shelves->setCurrentRow(qBound(0, r, int(shelves->count()) - 1));
    };
    auto fill_items = [this, shelves, items] {
        int r = items->currentRow();
        items->clear();
        int si = shelves->currentRow();
        if (si < 0) return;
        for (const QJsonValue& it : shelves_[si].toObject()["items"].toArray()) {
            QJsonObject o = it.toObject();
            const QString pic = image_file(o["icon"].toString());
            items->addItem(new QListWidgetItem(!pic.isEmpty() ? QIcon(pic) : az::icon(o["icon"].toString().isEmpty() ? o["label"].toString() : o["icon"].toString()), o["label"].toString()));
        }
        items->setCurrentRow(qBound(-1, r, int(items->count()) - 1));
    };
    auto commit = [this, fill_shelves, fill_items] {
        save();
        rebuild();
        fill_shelves();
        fill_items();
    };
    auto move_in = [this, commit](QJsonArray& a, int from, int to) {
        if (from < 0 || to < 0 || from >= a.size() || to >= a.size()) return false;
        QJsonValue v = a.at(from);
        a.removeAt(from);
        a.insert(to, v);
        return true;
    };
    auto row = [d](QVBoxLayout* into, const QString& t, std::function<void()> fn) {
        auto* b = new QPushButton(t, d);
        QObject::connect(b, &QPushButton::clicked, d, fn);
        into->addWidget(b);
    };
    row(left, "New Shelf", [this, commit, shelves] {
        shelves_.append(QJsonObject{{"name", QString("Shelf%1").arg(shelves_.size() + 1)}, {"items", QJsonArray()}});
        commit();
        shelves->setCurrentRow(shelves->count() - 1);
    });
    row(left, "Rename", [this, commit, shelves] {
        int si = shelves->currentRow();
        if (si < 0) return;
        bool ok = false;
        QJsonObject s = shelves_[si].toObject();
        QString n = QInputDialog::getText(shelves, "Rename Shelf", "Name:", QLineEdit::Normal, s["name"].toString(), &ok);
        if (!ok || n.trimmed().isEmpty()) return;
        s["name"] = n.trimmed();
        shelves_[si] = s;
        commit();
    });
    row(left, "Move Up", [this, commit, shelves, move_in] {
        int si = shelves->currentRow();
        if (move_in(shelves_, si, si - 1)) { commit(); shelves->setCurrentRow(si - 1); }
    });
    row(left, "Move Down", [this, commit, shelves, move_in] {
        int si = shelves->currentRow();
        if (move_in(shelves_, si, si + 1)) { commit(); shelves->setCurrentRow(si + 1); }
    });
    row(left, "Delete Shelf", [this, commit, shelves] {
        int si = shelves->currentRow();
        if (si >= 0 && shelves_.size() > 1) {
            shelves_.removeAt(si);
            commit();
        }
    });
    auto with_items = [this, shelves, commit](std::function<void(QJsonArray&)> fn) {
        int si = shelves->currentRow();
        if (si < 0) return;
        QJsonObject s = shelves_[si].toObject();
        QJsonArray a = s["items"].toArray();
        fn(a);
        s["items"] = a;
        shelves_[si] = s;
        commit();
    };
    row(mid, "New Item", [with_items, items] {
        with_items([](QJsonArray& a) { a.append(QJsonObject{{"label", "New"}, {"icon", ""}, {"command", ""}, {"language", "mel"}}); });
        items->setCurrentRow(items->count() - 1);
    });
    row(mid, "Move Up", [with_items, items, move_in] {
        int r = items->currentRow();
        with_items([=](QJsonArray& a) { move_in(a, r, r - 1); });
        items->setCurrentRow(r - 1);
    });
    row(mid, "Move Down", [with_items, items, move_in] {
        int r = items->currentRow();
        with_items([=](QJsonArray& a) { move_in(a, r, r + 1); });
        items->setCurrentRow(qMin(r + 1, items->count() - 1));
    });
    row(mid, "Delete Item", [with_items, items] {
        int r = items->currentRow();
        if (r >= 0) with_items([=](QJsonArray& a) { a.removeAt(r); });
    });
    auto* save_item = new QPushButton("Save Item", d);
    form->addRow("", save_item);
    connect(save_item, &QPushButton::clicked, d, [with_items, items, label, ic, lang, cmd] {
        int r = items->currentRow();
        if (r < 0) return;
        with_items([=](QJsonArray& a) {
            a[r] = QJsonObject{{"label", label->text()}, {"icon", ic->text()}, {"language", lang->currentText()}, {"command", cmd->toPlainText()}};
        });
        items->setCurrentRow(r);
    });
    connect(shelves, &QListWidget::currentRowChanged, d, fill_items);
    connect(items, &QListWidget::currentRowChanged, d, [this, shelves, label, ic, lang, cmd](int r) {
        int si = shelves->currentRow();
        if (si < 0 || r < 0) return;
        QJsonObject o = shelves_[si].toObject()["items"].toArray().at(r).toObject();
        label->setText(o["label"].toString());
        ic->setText(o["icon"].toString());
        lang->setCurrentText(o["language"].toString("mel"));
        cmd->setPlainText(o["command"].toString());
    });
    fill_shelves();
    fill_items();
    d->resize(860, 480);
    d->show();
}

void ShelfBar::import_mel(const QString& path) {
    QJsonObject shelf;
    if (!read_mel(path, &shelf)) {
        log("// Error: can't read " + path + " //", 2);
        return;
    }
    shelves_.append(shelf);
    save();
    rebuild();
    tabs_->setCurrentIndex(tabs_->count() - 1);
    log(QString("// Loaded shelf %1: %2 buttons //").arg(shelf["name"].toString()).arg(shelf["items"].toArray().size()));
}

// A shelf_*.mel file as a shelf: its shelfButton statements' label, command, language and image.
bool ShelfBar::read_mel(const QString& path, QJsonObject* shelf) {
    QFile f(path);
    if (!f.open(QIODevice::ReadOnly)) return false;
    QString src = QString::fromUtf8(f.readAll());
    QJsonArray items;
    // Each shelfButton runs to its closing semicolon outside quotes.
    int at = 0;
    while ((at = src.indexOf("shelfButton", at)) >= 0) {
        int end = at;
        bool quoted = false;
        for (; end < src.size(); ++end) {
            QChar c = src[end];
            if (c == '\\') { ++end; continue; }
            if (c == '"') quoted = !quoted;
            if (c == ';' && !quoted) break;
        }
        QString block = src.mid(at, end - at);
        auto flag = [&block](const char* name) {
            QRegularExpression re(QString("-%1\\s+\"((?:[^\"\\\\]|\\\\.)*)\"").arg(name));
            QString v = re.match(block).captured(1);
            v.replace("\\n", "\n").replace("\\\"", "\"").replace("\\\\", "\\");
            return v;
        };
        QString label = flag("label"), cmd = flag("command"), lang = flag("sourceType");
        if (label.isEmpty()) label = flag("imageOverlayLabel");
        // The button's picture, when it is a file on the icon path: the reference's own pictures are not there.
        QString image = flag("image1");
        if (image.isEmpty()) image = flag("image");
        if (!known_icon(image)) image.clear();
        if (!cmd.isEmpty())
            items.append(QJsonObject{{"label", label}, {"icon", image}, {"command", cmd}, {"language", lang == "python" ? "python" : "mel"}});
        at = end;
    }
    QString name = QFileInfo(path).baseName();
    if (name.startsWith("shelf_")) name = name.mid(6);
    *shelf = QJsonObject{{"name", name}, {"items", items}};
    return true;
}

} // namespace az
