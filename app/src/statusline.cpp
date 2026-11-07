#include <QAbstractItemView>
// The status line, left to right as the reference's: menu set, file, undo, selection scope and masks, snapping, selection locks,
// construction history, symmetry, soft select, then the numeric input and select by name field.
#include "mainwindow.h"
#include "aztek.h"
#include "host.h"
#include "icons.h"
#include "panels.h"
#include "viewport.h"

#include <QAction>
#include <QActionGroup>
#include <QComboBox>
#include <QDockWidget>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QPushButton>
#include <QStandardItemModel>
#include <QToolBar>
#include <QToolButton>

namespace az {

namespace {
QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}
} // namespace

void MainWindow::build_status_line() {
    QToolBar* tb = addToolBar("Status Line");
    tb->setObjectName("statusLine");
    tb->setMovable(false);
    tb->setIconSize(QSize(20, 20));
    menu_set_ = new QComboBox(tb);
    menu_set_->addItems({"Modeling", "Rigging", "Animation", "Rendering"});
    menu_set_->setToolTip("Menu set");
    connect(menu_set_, &QComboBox::currentTextChanged, this, &MainWindow::build_menu_set);
    tb->addWidget(menu_set_);
    tb->addSeparator();
    tb->addAction(icon("new"), "New Scene", [this] { new_scene(); });
    tb->addAction(icon("open"), "Open Scene", this, &MainWindow::open_scene);
    tb->addAction(icon("save"), "Save Scene", [this] { save_scene(false); });
    tb->addSeparator();
    tb->addAction(icon("undo"), "Undo", [] { aztek_undo(); Host::get()->poke(); });
    tb->addAction(icon("redo"), "Redo", [] { aztek_redo(); Host::get()->poke(); });
    tb->addSeparator();

    // Scope: hierarchy picks roots, object picks objects, component shows the kind buttons and masks.
    auto* scopes = new QActionGroup(this);
    struct S { const char* id; const char* ic; const char* tip; };
    for (S s : {S{"hierarchy", "scopeHierarchy", "Select by hierarchy: picks the top of the hierarchy"},
                S{"object", "scopeObject", "Select by object type"},
                S{"component", "scopeComponent", "Select by component type"}}) {
        QString id = s.id;
        QAction* a = tb->addAction(icon(s.ic), s.tip);
        a->setCheckable(true);
        scopes->addAction(a);
        connect(a, &QAction::triggered, this, [this, id] {
            exec_quiet("selectScope", args(QJsonArray{id}));
            if (id == "component" && state()["mode"].toString() == "object") set_mode("vertex");
            Host::get()->poke();
        });
        scope_buttons_[id] = a;
    }
    scope_buttons_["object"]->setChecked(true);

    // Object masks: each switches a kind of object on or off for selection. Right-click sets them all.
    auto mask = [this, tb](const char* ic, const char* tip, QStringList flags) {
        QAction* a = tb->addAction(icon(ic), tip);
        a->setCheckable(true);
        a->setChecked(true);
        connect(a, &QAction::toggled, this, [flags](bool on) {
            QJsonObject c;
            for (const QString& f : flags) c[f] = on;
            exec_quiet("selectType", c);
        });
        object_masks_.append(a);
    };
    tb->addSeparator();
    mask("maskHandle", "Select handles", {"handle", "ikHandle"});
    mask("maskJoint", "Select joints", {"joint"});
    mask("maskCurve", "Select curves", {"nurbsCurve"});
    mask("maskSurface", "Select surfaces", {"polymesh", "nurbsSurface"});
    mask("maskDeformer", "Select deformations", {"cluster", "nonlinear"});
    mask("maskMisc", "Select miscellaneous: locators and groups", {"locator", "group"});
    mask("camera", "Select rendering nodes: cameras and lights", {"camera", "light"});
    // Component kinds, in place of the object masks in component scope.
    auto* modes = new QActionGroup(this);
    for (const char* m : {"object", "vertex", "edge", "face"}) {
        QString mode = m;
        QAction* a = tb->addAction(icon(mode), QString("Select by %1").arg(mode == "object" ? "object" : mode));
        a->setCheckable(true);
        modes->addAction(a);
        connect(a, &QAction::triggered, this, [this, mode] { set_mode(mode); });
        mode_buttons_[mode] = a;
    }
    tb->addSeparator();

    // Snapping: stays on until clicked off, as well as the hold keys.
    auto snap = [this, tb](const char* id, const char* ic, const char* tip) {
        QString mode = id;
        QAction* a = tb->addAction(icon(ic), tip);
        a->setCheckable(true);
        connect(a, &QAction::toggled, this, [this, mode](bool on) {
            if (on) snap_on_.insert(mode);
            else snap_on_.remove(mode);
            apply_snap();
        });
    };
    snap("grid", "snapGrid", "Snap to grids (hold X)");
    snap("curve", "snapCurve", "Snap to curves (hold C)");
    snap("point", "snapPoint", "Snap to points (hold V)");
    snap("viewPlane", "snapPlane", "Snap to view planes");
    snap("live", "snapLive", "Make the selected object live");
    tb->addSeparator();

    QAction* lock = tb->addAction(icon("lockSel"), "Lock selection: clicks leave it as it is");
    lock->setCheckable(true);
    connect(lock, &QAction::toggled, this, [](bool on) { exec_quiet("selectPref", QJsonObject{{"lockSelection", on}}); });
    QAction* hi = tb->addAction(icon("highlightSel"), "Highlight selection");
    hi->setCheckable(true);
    hi->setChecked(true);
    connect(hi, &QAction::toggled, this, [this](bool on) {
        for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"highlight\":%1}").arg(on ? "true" : "false"));
    });
    tb->addSeparator();
    history_button_ = tb->addAction(icon("history"), "Construction history on/off");
    history_button_->setCheckable(true);
    connect(history_button_, &QAction::triggered, this, [](bool on) { exec("constructionHistory", QJsonObject{{"state", on}}); });
    tb->addSeparator();

    auto* sym_label = new QLabel(" Symmetry:", tb);
    tb->addWidget(sym_label);
    symmetry_ = new QComboBox(tb);
    symmetry_->addItems({"Off", "Object X", "Object Y", "Object Z", "World X", "World Y", "World Z", "Topology"});
    symmetry_->setToolTip("Symmetry on the selected meshes");
    connect(symmetry_, &QComboBox::activated, this, [this](int i) {
        // Topology pairs the mesh from the seam edge selected.
        if (i == 7) {
            exec("symmetricModelling", QJsonObject{{"edit", true}, {"topoSymmetry", true}});
            return;
        }
        if (i == 0) {
            exec("symmetricModelling", QJsonObject{{"edit", true}, {"symmetry", false}});
            return;
        }
        static const char* axis[] = {"", "x", "y", "z", "x", "y", "z"};
        exec("symmetricModelling", QJsonObject{{"edit", true}, {"symmetry", true}, {"about", i >= 4 ? "world" : "object"}, {"axis", axis[i]}});
    });
    tb->addWidget(symmetry_);
    soft_button_ = tb->addAction(icon("softSelect"), "Soft select (tap B; hold B and drag sizes the falloff)");
    soft_button_->setCheckable(true);
    connect(soft_button_, &QAction::triggered, this, [](bool on) { exec("softSelect", QJsonObject{{"softSelectEnabled", on ? 1 : 0}}); });

    auto* spacer = new QWidget(tb);
    spacer->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Preferred);
    tb->addWidget(spacer);

    // The field: absolute and relative transforms by the current tool, rename, select by name.
    input_mode_ = new QComboBox(tb);
    input_mode_->addItems({"Absolute", "Relative", "Rename", "Select by Name"});
    input_mode_->setCurrentIndex(3);
    input_mode_->setToolTip("What the field does");
    tb->addWidget(input_mode_);
    input_field_ = new QLineEdit(tb);
    input_field_->setFixedWidth(170);
    input_field_->setPlaceholderText("name or x y z");
    connect(input_field_, &QLineEdit::returnPressed, this, [this] {
        input_entered(input_field_->text());
        input_field_->clear();
    });
    tb->addWidget(input_field_);
    tb->addSeparator();
    // The editors' show and hide buttons, at the far right.
    tb->addAction(icon("toolSettings"), "Show or hide the Tool Settings", [this] { tool_dock_->setVisible(!tool_dock_->isVisible()); });
    tb->addAction(icon("attrEditor"), "Show or hide the Attribute Editor", [this] { attr_dock_->setVisible(!attr_dock_->isVisible()); if (attr_dock_->isVisible()) attr_dock_->raise(); });
    tb->addAction(icon("channelBoxIcon"), "Show or hide the Channel Box", [this] { channel_dock_->setVisible(!channel_dock_->isVisible()); if (channel_dock_->isVisible()) channel_dock_->raise(); });
    tb->addAction(icon("outlinerIcon"), "Show or hide the Outliner", [this] { outliner_dock_->setVisible(!outliner_dock_->isVisible()); if (outliner_dock_->isVisible()) outliner_dock_->raise(); });
}

void MainWindow::input_entered(const QString& raw) {
    QString text = raw.trimmed();
    if (text.isEmpty()) return;
    int mode = input_mode_->currentIndex();
    if (mode == 3) {
        // Wildcards as the outliner's filter: pCube* selects every match.
        QJsonValue hits = ask("ls", args(QJsonArray{text}));
        if (hits.toArray().isEmpty()) {
            log(QString("// Warning: No object matches name: %1 //").arg(text), 4);
            return;
        }
        exec("select", args(hits.toArray()));
        return;
    }
    if (mode == 2) {
        // Numbered when several are selected, as the reference's field.
        QJsonArray sel = state()["selection"].toArray();
        if (sel.isEmpty()) return;
        for (int i = 0; i < sel.size(); ++i) exec("rename", args(QJsonArray{sel[i], sel.size() > 1 ? text + QString::number(i + 1) : text}));
        return;
    }
    // x y z: a dot or blank leaves that axis.
    QStringList parts = text.split(' ', Qt::SkipEmptyParts);
    bool rel = mode == 1;
    QString t = tool_ == "rotate" || tool_ == "scale" ? tool_ : QString("move");
    QJsonArray sel = state()["selection"].toArray();
    bool comps = state()["mode"].toString() != "object";
    double same = t == "scale" ? 1.0 : 0.0;
    double v[3] = {same, same, same};
    bool given[3] = {false, false, false};
    for (int i = 0; i < 3 && i < parts.size(); ++i) {
        bool ok = false;
        double x = parts[i].toDouble(&ok);
        if (ok) {
            v[i] = x;
            given[i] = true;
        }
    }
    QJsonArray by{v[0], v[1], v[2]};
    if (rel || comps) {
        if (comps) exec(t == "move" ? "moveComponents" : t == "rotate" ? "rotateComponents" : "scaleComponents", QJsonObject{{"by", by}});
        else exec(t, QJsonObject{{"by", by}});
        return;
    }
    if (t == "move") {
        QJsonObject c;
        static const char* n[] = {"x", "y", "z"};
        for (int i = 0; i < 3; ++i)
            if (given[i]) c[n[i]] = v[i];
        exec("move", c);
        return;
    }
    static const char* axis[] = {"X", "Y", "Z"};
    for (const QJsonValue& o : sel)
        for (int i = 0; i < 3; ++i)
            if (given[i]) exec("setAttr", args(QJsonArray{o.toString() + "." + t + axis[i], v[i]}));
}

void MainWindow::sync_status_line() {
    QString scope = ask("selectScope").toString();
    if (scope_buttons_.contains(scope)) scope_buttons_[scope]->setChecked(true);
    bool on = ask("softSelect", QJsonObject{{"query", true}, {"queried", QJsonArray{"softSelectEnabled"}}}).toInt() != 0;
    soft_button_->setChecked(on);
    if (!symmetry_->view()->isVisible()) {
        auto sym = [](const char* flag) { return ask("symmetricModelling", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}); };
        int at = 0;
        if (sym("symmetry").toBool()) {
            QString about = sym("about").toString(), axis = sym("axis").toString();
            int k = axis == "z" ? 2 : axis == "y" ? 1 : 0;
            at = about == "topo" ? 7 : about == "world" ? 4 + k : 1 + k;
        }
        symmetry_->setCurrentIndex(at);
    }
    bool comps = state()["mode"].toString() != "object";
    for (QAction* a : object_masks_) a->setVisible(!comps);
    mode_buttons_["object"]->setVisible(false);
    for (const char* m : {"vertex", "edge", "face"}) mode_buttons_[m]->setVisible(comps);
}

} // namespace az
