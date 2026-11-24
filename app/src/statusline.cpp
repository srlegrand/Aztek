#include <QSignalBlocker>
#include <QAbstractItemView>
// The status line, laid out as the reference's: the menu set, the file icons, the selection modes and masks, snapping,
// symmetry, history, rendering, the quick input fields and, at the far right, the editor toggles. Every group folds
// behind the thin handle before it, with the reference's own tooltips, sizes and order.
#include "mainwindow.h"
#include <QSettings>
#include "aztek.h"
#include "host.h"
#include "icons.h"
#include "panels.h"
#include "renderui.h"
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
#include <QTimer>
#include <QToolButton>
#include <algorithm>

namespace az {

namespace {

QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}

QToolButton* bar_button(QWidget* parent, QAction* a, int w = 22) {
    auto* b = new QToolButton(parent);
    b->setDefaultAction(a);
    b->setFixedSize(w, 22);
    b->setIconSize(QSize(20, 20));
    b->setToolButtonStyle(Qt::ToolButtonIconOnly);
    b->setAutoRaise(true);
    b->setFocusPolicy(Qt::NoFocus);
    return b;
}

// The 5 pixel gap the reference puts around each collapse handle.
QWidget* bar_gap(QWidget* parent) {
    auto* w = new QWidget(parent);
    w->setFixedSize(5, 22);
    return w;
}

QLineEdit* bar_field(QWidget* parent, int w, const QString& text, bool read_only) {
    auto* e = new QLineEdit(text, parent);
    e->setFixedSize(w, 22);
    e->setReadOnly(read_only);
    e->setFrame(false);
    return e;
}

} // namespace

void MainWindow::build_status_line() {
    QToolBar* tb = addToolBar("Status Line");
    tb->setObjectName("statusLine");
    tb->setMovable(false);
    tb->setFloatable(false);
    tb->setIconSize(QSize(20, 20));
    tb->setFixedHeight(23);

    auto* bar = new QWidget(tb);
    bar->setObjectName("statusBar");
    bar->setFixedHeight(22);
    auto* row = new QHBoxLayout(bar);
    row->setContentsMargins(1, 0, 1, 0);
    row->setSpacing(1);

    // The left part gives way first when the window is narrow; the editor toggles at the right stay.
    auto* left = new QWidget(bar);
    left->setObjectName("statusLeft");
    left->setSizePolicy(QSizePolicy::Ignored, QSizePolicy::Fixed);
    left->setFixedHeight(22);
    auto* l = new QHBoxLayout(left);
    l->setContentsMargins(0, 0, 0, 0);
    l->setSpacing(1);
    auto add = [l](QWidget* w) { l->addWidget(w, 0, Qt::AlignVCenter); return w; };
    auto action = [this](const char* ic, const QString& tip) {
        auto* a = new QAction(icon(ic), tip, this);
        a->setToolTip(tip);
        return a;
    };

    // The menu set, 100 pixels wide: the reference's option menu as a button over its list, with the Customize entry
    // (no menu set editor here yet).
    menu_set_button_ = new QToolButton(left);
    menu_set_button_->setFixedSize(100, 22);
    menu_set_button_->setToolButtonStyle(Qt::ToolButtonTextOnly);
    menu_set_button_->setPopupMode(QToolButton::InstantPopup);
    menu_set_button_->setText(menu_set_);
    auto* menu_set_menu = new QMenu(menu_set_button_);
    menu_set_button_->setMenu(menu_set_menu);
    for (const QString& name : menu_set_names()) {
        QAction* a = menu_set_menu->addAction(name);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [this, name] {
            set_menu_set(name);
            if (ViewportWidget* v = panels_->active()) v->setFocus(Qt::OtherFocusReason);
        });
    }
    menu_set_menu->addSeparator();
    QAction* customize = menu_set_menu->addAction("Customize ...");
    customize->setEnabled(false);
    connect(menu_set_menu, &QMenu::aboutToShow, this, [this, menu_set_menu] {
        for (QAction* a : menu_set_menu->actions()) a->setChecked(a->text() == menu_set_);
    });
    add(menu_set_button_);

    // The file group, folded by the first handle.
    QList<QWidget*> files;
    auto file_action = [this, action](const char* ic, const QString& tip, std::function<void()> fn) {
        auto* a = action(ic, tip);
        connect(a, &QAction::triggered, this, fn);
        return a;
    };
    QAction* a_new = file_action("new", "Create a new scene", [this] { new_scene(); });
    QAction* a_open = file_action("open", "Open a scene", [this] { open_scene(); });
    QAction* a_save = file_action("save", "Save the current scene", [this] { save_scene(false); });
    QAction* a_undo = file_action("undo", "Undo the last action", [] { aztek_undo(); Host::get()->poke(); });
    QAction* a_redo = file_action("redo", "Redo the last undone action", [] { aztek_redo(); Host::get()->poke(); });

    // The selection scope, the masks, the locks, snapping, symmetry, history, rendering and the fields.
    auto* preset_form = new QWidget(left);
    preset_form->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* preset_l = new QHBoxLayout(preset_form);
    preset_l->setContentsMargins(0, 0, 0, 0);
    preset_l->setSpacing(1);
    auto* preset_arrow = new QToolButton(preset_form);
    preset_arrow->setFixedSize(9, 22);
    preset_arrow->setIcon(icon("menuArrow", 16));
    preset_arrow->setIconSize(QSize(7, 20));
    preset_arrow->setToolTip("Set the selection mask");
    preset_arrow->setAutoRaise(true);
    preset_arrow->setPopupMode(QToolButton::InstantPopup);
    auto* preset_menu = new QMenu(preset_arrow);
    preset_arrow->setMenu(preset_menu);
    connect(preset_menu, &QMenu::aboutToShow, this, [this, preset_menu] {
        preset_menu->clear();
        preset_menu->addAction("All Objects", this, [this] {
            exec_quiet("selectType", QJsonObject{{"allObjects", true}});
            exec_quiet("selectScope", args(QJsonArray{"object"}));
            sync_status_line();
        });
        preset_menu->addAction("Animation", this, [this] {
            QJsonObject c{{"animCurve", true}, {"animKeyframe", true}};
            exec_quiet("selectType", c);
            sync_status_line();
        });
        preset_menu->addAction("Polygons", this, [this] {
            QJsonObject c{{"polymesh", true}, {"polymeshVertex", true}, {"polymeshEdge", true}, {"polymeshFace", true}};
            exec_quiet("selectType", c);
            sync_status_line();
        });
        preset_menu->addAction("NURBS", this, [this] {
            QJsonObject c{{"nurbsCurve", true}, {"nurbsSurface", true}, {"controlVertex", true}, {"hull", true}};
            exec_quiet("selectType", c);
            sync_status_line();
        });
        preset_menu->addAction("Deform", this, [this] {
            QJsonObject c{{"cluster", true}, {"nonlinear", true}, {"lattice", true}};
            exec_quiet("selectType", c);
            sync_status_line();
        });
        preset_menu->addAction("Dynamics", this, [this] {
            QJsonObject c{{"nCloth", true}, {"particleShape", true}, {"rigidBody", true}, {"emitter", true}, {"field", true}};
            exec_quiet("selectType", c);
            sync_status_line();
        });
        preset_menu->addAction("Rendering", this, [this] {
            QJsonObject c{{"camera", true}, {"light", true}, {"texture", true}};
            exec_quiet("selectType", c);
            sync_status_line();
        });
        preset_menu->addSeparator();
        preset_menu->addAction("Initial Default", this, [this] {
            exec_quiet("selectType", QJsonObject{{"allObjects", true}});
            exec_quiet("selectType", QJsonObject{{"vertex", true}, {"edge", true}, {"facet", true}});
            sync_status_line();
        });
    });
    preset_l->addWidget(preset_arrow);
    auto* preset_text = bar_field(preset_form, 94, "Root", true);
    preset_text->setToolTip("Set the selection mask");
    preset_l->addWidget(preset_text);
    files << preset_form;

    auto* select_modes = new QWidget(left);
    select_modes->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* mode_l = new QHBoxLayout(select_modes);
    mode_l->setContentsMargins(0, 0, 0, 0);
    mode_l->setSpacing(1);
    QList<QWidget*> mode_buttons;
    for (auto [id, ic, tip] : {std::tuple{"hierarchy", "scopeHierarchy", "Select by hierarchy and combinations"},
                               {"object", "scopeObject", "Select by object type"},
                               {"component", "scopeComponent", "Select by component type"}}) {
        const QString sid = id;
        QAction* a = action(ic, tip);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [this, sid] {
            exec_quiet("selectScope", args(QJsonArray{sid}));
            if (sid == "component" && state()["mode"].toString() == "object") set_mode("vertex");
            sync_object_masks();
            sync_status_line();
            Host::get()->poke();
        });
        scope_buttons_[sid] = a;
        auto* b = bar_button(select_modes, a);
        mode_l->addWidget(b);
        mode_buttons << b;
    }
    scope_buttons_["object"]->setChecked(true);

    // The selection mask group: hierarchy, object or component masks, then the lock and highlight buttons.
    auto* mask_set = new QWidget(left);
    mask_set->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* mask_set_l = new QHBoxLayout(mask_set);
    mask_set_l->setContentsMargins(0, 0, 0, 0);
    mask_set_l->setSpacing(1);

    auto mask_popup = [this](QWidget* parent, const QString& tip, const QList<QAction*>& masks, bool component) {
        auto* b = new QToolButton(parent);
        b->setFixedSize(9, 22);
        b->setIcon(icon("menuArrow", 16));
        b->setIconSize(QSize(7, 20));
        b->setToolTip(tip);
        b->setAutoRaise(true);
        b->setPopupMode(QToolButton::InstantPopup);
        auto* m = new QMenu(b);
        b->setMenu(m);
        connect(m, &QMenu::aboutToShow, this, [this, m, masks, component] {
            m->clear();
            m->addAction("All On", this, [this, masks, component] {
                for (QAction* a : masks) {
                    const QSignalBlocker block(a);
                    a->setChecked(true);
                }
                exec_quiet("selectType", QJsonObject{{component ? "allComponents" : "allObjects", true}});
                sync_status_line();
            });
            m->addAction("All Off", this, [this, masks, component] {
                for (QAction* a : masks) {
                    const QSignalBlocker block(a);
                    a->setChecked(false);
                }
                exec_quiet("selectType", QJsonObject{{component ? "allComponents" : "allObjects", false}});
                sync_status_line();
            });
            m->addSeparator();
            for (QAction* a : masks) {
                QAction* c = m->addAction(a->text());
                c->setCheckable(true);
                c->setChecked(a->isChecked());
                connect(c, &QAction::triggered, a, &QAction::trigger);
            }
        });
        return b;
    };

    hierarchy_masks_ = new QWidget(mask_set);
    hierarchy_masks_->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* hier_l = new QHBoxLayout(hierarchy_masks_);
    hier_l->setContentsMargins(0, 0, 0, 0);
    hier_l->setSpacing(1);
    QList<QAction*> hier_actions;
    for (auto [mode, ic, tip, checked] : {std::tuple{"root", "select", "Select by hierarchy: Root", true},
                                          {"leaf", "selectComp", "Select by hierarchy: Leaf", false},
                                          {"template", "maskMisc", "Select by hierarchy: Template", false}}) {
        QAction* a = action(ic, tip);
        a->setCheckable(true);
        a->setChecked(checked);
        a->setData(QString::fromUtf8(mode));
        hier_actions << a;
        hier_l->addWidget(bar_button(hierarchy_masks_, a));
    }
    for (QAction* a : hier_actions) {
        connect(a, &QAction::triggered, this, [this, a, hier_actions] {
            for (QAction* o : hier_actions) {
                const QSignalBlocker block(o);
                o->setChecked(o == a);
            }
            exec_quiet("selectScope", args(QJsonArray{"hierarchy"}));
            exec_quiet("selectMode", args(QJsonArray{a->data().toString()}));
            sync_status_line();
        });
    }
    hier_l->insertWidget(0, mask_popup(hierarchy_masks_, "Set the hierarchy selection mask", hier_actions, false));

    auto add_mask = [&](QWidget* parent, QHBoxLayout* lay, const char* ic, const QString& tip, const QStringList& flags,
                        bool checked, const QString& mode) {
        QAction* a = action(ic, tip);
        a->setCheckable(true);
        a->setChecked(checked);
        a->setProperty("maskFlags", flags);
        connect(a, &QAction::toggled, this, [flags](bool on) {
            QJsonObject c;
            for (const QString& f : flags) c[f] = on;
            exec_quiet("selectType", c);
        });
        if (!mode.isEmpty() && (mode == "vertex" || mode == "edge" || mode == "face")) mode_buttons_[mode] = a;
        lay->addWidget(bar_button(parent, a));
        return a;
    };

    object_masks_widget_ = new QWidget(mask_set);
    object_masks_widget_->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* obj_l = new QHBoxLayout(object_masks_widget_);
    obj_l->setContentsMargins(0, 0, 0, 0);
    obj_l->setSpacing(1);
    object_masks_ << add_mask(object_masks_widget_, obj_l, "maskHandle", "Select handle objects (RMB for more info)", {"handle", "ikHandle"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "maskJoint", "Select joint objects (RMB for more info)", {"joint"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "maskCurve", "Select curve objects (RMB for more info)", {"nurbsCurve"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "maskSurface", "Select surface objects (RMB for more info)", {"polymesh", "nurbsSurface"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "maskDeformer", "Select deformation objects (RMB for more info)", {"cluster", "nonlinear"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "particles", "Select dynamic objects (RMB for more info)", {"nCloth", "particleShape", "nParticleShape", "rigidBody", "emitter", "field", "spring", "dynamicConstraint"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "camera", "Select rendering objects (RMB for more info)", {"camera", "light"}, true, QString());
    object_masks_ << add_mask(object_masks_widget_, obj_l, "maskMisc", "Select miscellaneous objects (RMB for more info)", {"locator", "group"}, true, QString());
    obj_l->insertWidget(0, mask_popup(object_masks_widget_, "Set the object selection mask", object_masks_, false));

    component_masks_widget_ = new QWidget(mask_set);
    component_masks_widget_->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* comp_l = new QHBoxLayout(component_masks_widget_);
    comp_l->setContentsMargins(0, 0, 0, 0);
    comp_l->setSpacing(1);
    component_masks_ << add_mask(component_masks_widget_, comp_l, "vertex", "Select point components (RMB for more info)", {"vertex", "controlVertex"}, true, "vertex");
    component_masks_ << add_mask(component_masks_widget_, comp_l, "curveCv", "Select parm points components (RMB for more info)", {"curveParameterPoint", "surfaceParameterPoint"}, false, QString());
    component_masks_ << add_mask(component_masks_widget_, comp_l, "edge", "Select line components (RMB for more info)", {"edge", "isoparm"}, true, "edge");
    component_masks_ << add_mask(component_masks_widget_, comp_l, "face", "Select face components (RMB for more info)", {"facet", "surfaceFace"}, true, "face");
    component_masks_ << add_mask(component_masks_widget_, comp_l, "maskSurface", "Select hull components (RMB for more info)", {"hull"}, false, QString());
    component_masks_ << add_mask(component_masks_widget_, comp_l, "select", "Select pivot components (RMB for more info)", {"rotatePivot", "scalePivot", "jointPivot"}, false, QString());
    component_masks_ << add_mask(component_masks_widget_, comp_l, "maskHandle", "Select handle components (RMB for more info)", {"selectHandle"}, false, QString());
    component_masks_ << add_mask(component_masks_widget_, comp_l, "maskMisc", "Select miscellaneous components (RMB for more info)", {"latticePoint", "polymeshUV", "surfaceUV", "isoparm"}, false, QString());
    comp_l->insertWidget(0, mask_popup(component_masks_widget_, "Set the component selection mask", component_masks_, true));

    mask_set_l->addWidget(hierarchy_masks_);
    mask_set_l->addWidget(object_masks_widget_);
    mask_set_l->addWidget(component_masks_widget_);
    QAction* lock = action("lockSel", "Lock/Unlock current selection");
    lock_action_ = lock;
    lock->setCheckable(true);
    connect(lock, &QAction::toggled, this, [](bool on) { exec_quiet("selectPref", QJsonObject{{"lockSelection", on}}); });
    mask_set_l->addWidget(bar_button(mask_set, lock));
    QAction* hi = action("highlightSel", "Highlight Selection mode is on");
    hi->setCheckable(true);
    hi->setChecked(true);
    hi->setProperty("highlightAction", true);
    connect(hi, &QAction::toggled, this, [this, hi](bool on) {
        hi->setToolTip(on ? "Highlight Selection mode is on (Affects component selection mode)" : "Highlight Selection mode is off (Affects component selection mode)");
        for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"highlight\":%1}").arg(on ? "true" : "false"));
    });
    mask_set_l->addWidget(bar_button(mask_set, hi));

    // Snapping and Make Live, with the live object's name after them.
    auto* snap_group = new QWidget(left);
    snap_group->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* snap_l = new QHBoxLayout(snap_group);
    snap_l->setContentsMargins(0, 0, 0, 0);
    snap_l->setSpacing(1);
    auto snap = [this, snap_group, snap_l](const QString& mode, const char* ic, const QString& tip) {
        QAction* a = new QAction(icon(ic), tip, this);
        a->setToolTip(tip);
        a->setCheckable(true);
        connect(a, &QAction::toggled, this, [this, mode](bool on) {
            if (on) snap_on_.insert(mode);
            else snap_on_.remove(mode);
            exec_quiet("snapMode", QJsonObject{{mode, on}});
            apply_snap();
        });
        a->setProperty("snapMode", mode);
        a->setProperty("snapLabel", tip);
        snap_actions_ << a;
        snap_l->addWidget(bar_button(snap_group, a));
        return a;
    };
    snap("grid", "snapGrid", "Snap to grids");
    snap("curve", "snapCurve", "Snap to curves");
    snap("point", "snapPoint", "Snap to points");
    snap("meshCenter", "mesh", "Snap to Projected Center");
    snap("viewPlane", "snapPlane", "Snap to view planes");
    live_button_ = new QAction(icon("snapLive"), "Make the selected object(s) live", this);
    live_button_->setToolTip("Make the selected object(s) live");
    live_button_->setCheckable(true);
    connect(live_button_, &QAction::triggered, this, [this](bool on) {
        if (on) {
            if (state()["selection"].toArray().isEmpty()) {
                log("// Warning: select a mesh to make live //", 4);
                const QSignalBlocker block(live_button_);
                live_button_->setChecked(false);
                return;
            }
            exec("makeLive");
        } else {
            exec("makeLive", QJsonObject{{"none", true}});
        }
        sync_status_line();
    });
    auto* make_live = bar_button(snap_group, live_button_);
    make_live->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(make_live, &QToolButton::customContextMenuRequested, this, [this, make_live](const QPoint& at) {
        QMenu m;
        m.addAction("Add Selected", this, [this] { exec("makeLive", QJsonObject{{"edit", true}, {"addObjects", true}}); sync_status_line(); });
        m.addAction("Remove Selected", this, [this] { exec("makeLive", QJsonObject{{"edit", true}, {"removeObjects", true}}); sync_status_line(); });
        m.exec(make_live->mapToGlobal(at));
    });
    snap_l->addWidget(make_live);
    auto* live_menu_button = new QToolButton(snap_group);
    live_menu_button->setFixedSize(17, 22);
    live_menu_button->setIcon(icon("menuArrow", 16));
    live_menu_button->setIconSize(QSize(9, 20));
    live_menu_button->setAutoRaise(true);
    live_menu_button->setPopupMode(QToolButton::InstantPopup);
    auto* live_menu = new QMenu(live_menu_button);
    live_menu_button->setMenu(live_menu);
    connect(live_menu, &QMenu::aboutToShow, this, [this, live_menu] {
        live_menu->clear();
        live_menu->addAction("Add Selected", this, [this] { exec("makeLive", QJsonObject{{"edit", true}, {"addObjects", true}}); sync_status_line(); });
        live_menu->addAction("Remove Selected", this, [this] { exec("makeLive", QJsonObject{{"edit", true}, {"removeObjects", true}}); sync_status_line(); });
        live_menu->addSeparator();
        const QJsonArray live = ask("makeLive", QJsonObject{{"query", true}}).toArray();
        if (live.isEmpty()) live_menu->addAction("No previous live surfaces")->setEnabled(false);
        else
            for (const QJsonValue& n : live) live_menu->addAction(n.toString(), this, [n] { exec("select", args(QJsonArray{n})); });
    });
    snap_l->addWidget(live_menu_button);
    live_text_ = bar_field(snap_group, 100, "No Live Surface", true);
    live_text_->setToolTip("Displays the name of the live surface");
    snap_l->addWidget(live_text_);

    // Symmetry: the arrow menu and the mode text.
    auto* symmetry_group = new QWidget(left);
    symmetry_group->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* sym_l = new QHBoxLayout(symmetry_group);
    sym_l->setContentsMargins(0, 0, 0, 0);
    sym_l->setSpacing(1);
    auto* sym_button = new QToolButton(symmetry_group);
    sym_button->setFixedSize(17, 22);
    sym_button->setIcon(icon("menuArrow", 16));
    sym_button->setIconSize(QSize(9, 20));
    sym_button->setToolTip("Set symmetry");
    sym_button->setAutoRaise(true);
    sym_button->setPopupMode(QToolButton::InstantPopup);
    auto* sym_menu = new QMenu(sym_button);
    sym_button->setMenu(sym_menu);
    auto set_sym = [this](const char* about, const char* axis, bool on) {
        exec("symmetricModelling", QJsonObject{{"edit", true}, {"symmetry", on}, {"about", about}, {"axis", axis}});
        sync_status_line();
    };
    connect(sym_menu, &QMenu::aboutToShow, this, [this, sym_menu, set_sym] {
        sym_menu->clear();
        sym_menu->addAction("Off", this, [set_sym] { set_sym("object", "x", false); });
        for (auto [label, about, axis] : {std::tuple{"Object X", "object", "x"}, {"Object Y", "object", "y"}, {"Object Z", "object", "z"},
                                           {"World X", "world", "x"}, {"World Y", "world", "y"}, {"World Z", "world", "z"}})
            sym_menu->addAction(label, this, [set_sym, about, axis] { set_sym(about, axis, true); });
        sym_menu->addAction("Topology", this, [this] { exec("symmetricModelling", QJsonObject{{"edit", true}, {"topoSymmetry", true}}); sync_status_line(); });
    });
    sym_l->addWidget(sym_button);
    symmetry_text_ = bar_field(symmetry_group, 100, "Symmetry: Off", true);
    symmetry_text_->setToolTip("Symmetric modeling");
    sym_l->addWidget(symmetry_text_);

    // History: inputs, outputs and construction history.
    auto* history_group = new QWidget(left);
    history_group->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* hist_l = new QHBoxLayout(history_group);
    hist_l->setContentsMargins(0, 0, 0, 0);
    hist_l->setSpacing(1);
    auto history_menu = [this](QToolButton* b, bool future) {
        auto* m = new QMenu(b);
        b->setMenu(m);
        b->setPopupMode(QToolButton::InstantPopup);
        connect(m, &QMenu::aboutToShow, this, [this, m, future] {
            m->clear();
            const QJsonArray sel = state()["selection"].toArray();
            if (sel.isEmpty()) {
                m->addAction("Nothing selected")->setEnabled(false);
                return;
            }
            const QString lead = sel.last().toString();
            const QJsonArray nodes = ask("listHistory", QJsonObject{{"args", QJsonArray{lead}}, {"future", future}, {"fullNodeName", true}}).toArray();
            for (const QJsonValue& n : nodes) m->addAction(n.toString(), this, [n] { exec("select", QJsonObject{{"args", QJsonArray{n}}, {"add", true}}); });
            if (nodes.isEmpty()) m->addAction("Nothing")->setEnabled(false);
        });
        return b;
    };
    auto* history_button = new QToolButton(history_group);
    history_button->setFixedSize(22, 22);
    history_button->setIcon(icon("history"));
    history_button->setIconSize(QSize(20, 20));
    history_button->setToolTip("Inputs to the selected object");
    history_button->setAutoRaise(true);
    history_menu(history_button, false);
    hist_l->addWidget(history_button);
    auto* future_button = new QToolButton(history_group);
    future_button->setFixedSize(22, 22);
    future_button->setIcon(icon("future"));
    future_button->setIconSize(QSize(20, 20));
    future_button->setToolTip("Outputs from the selected object");
    future_button->setAutoRaise(true);
    history_menu(future_button, true);
    hist_l->addWidget(future_button);
    history_button_ = action("constructHistory", "Construction history on/off");
    history_button_->setCheckable(true);
    connect(history_button_, &QAction::triggered, this, [](bool on) { exec("constructionHistory", QJsonObject{{"state", on}}); });
    hist_l->addWidget(bar_button(history_group, history_button_));

    // The rendering group.
    auto* render_group = new QWidget(left);
    render_group->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* render_l = new QHBoxLayout(render_group);
    render_l->setContentsMargins(0, 0, 0, 0);
    render_l->setSpacing(1);
    auto render_action = [this, action, render_group, render_l](const char* ic, const QString& tip, std::function<void()> fn) {
        QAction* a = action(ic, tip);
        connect(a, &QAction::triggered, this, fn);
        QToolButton* b = bar_button(render_group, a);
        render_l->addWidget(b);
        return a;
    };
    render_action("renderView", "Display rendering image", [this] { show_render_view(this); });
    render_action("renderFrame", "Render the current frame (Harold)", [this] { render_current_frame(this); });
    render_action("renderIpr", "IPR render the current frame (Harold)", [this] { render_ipr(this); });
    render_action("renderSettings", "Display Render Settings window (Harold)", [this] { show_render_settings(this); });
    render_action("hypershade", "Display and edit connections in shading networks", [this] { show_hypershade(); });
    QAction* a_setup = render_action("renderSettings", "Launches the Render Setup editor to create render layers, collections, and overrides.", [] {});
    a_setup->setEnabled(false);
    QAction* a_light_editor = render_action("pointLight", "Open the Light Editor", [] {});
    a_light_editor->setEnabled(false);
    QAction* a_pause = render_action("pause", "Toggle pausing Viewport 2 display update", [] {});
    a_pause->setCheckable(true);
    a_pause->setEnabled(false);

    // The quick input fields, with the mode button and its menu.
    auto* input_group = new QWidget(left);
    input_group->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    auto* input_l = new QHBoxLayout(input_group);
    input_l->setContentsMargins(0, 0, 0, 0);
    input_l->setSpacing(1);
    input_mode_button_ = new QToolButton(input_group);
    input_mode_button_->setFixedSize(34, 22);
    input_mode_button_->setIconSize(QSize(20, 20));
    input_mode_button_->setToolTip("Menu of input line operations");
    input_mode_button_->setAutoRaise(true);
    input_mode_button_->setPopupMode(QToolButton::InstantPopup);
    auto* mode_menu = new QMenu(input_mode_button_);
    input_mode_button_->setMenu(mode_menu);
    input_l->addWidget(input_mode_button_);
    input_field_ = bar_field(input_group, 166, QString(), false);
    input_l->addWidget(input_field_);
    auto numbers = new QWidget(input_group);
    numbers->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    numbers->setFixedWidth(170);
    auto* num_l = new QHBoxLayout(numbers);
    num_l->setContentsMargins(0, 0, 0, 0);
    num_l->setSpacing(1);
    auto num = [numbers, num_l](const QString& label, QLineEdit** field) {
        auto* t = new QLabel(label, numbers);
        t->setFixedWidth(14);
        t->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
        num_l->addWidget(t);
        *field = bar_field(numbers, 40, QString(), false);
        num_l->addWidget(*field);
        return t;
    };
    QLabel* lx = num("X:", &num_x_);
    QLabel* ly = num("Y:", &num_y_);
    QLabel* lz = num("Z:", &num_z_);
    input_l->addWidget(numbers);
    numbers->hide();
    auto apply_mode = [this, mode_menu, lx, ly, lz, numbers] {
        const QString mode = input_mode_;
        const char* ic = mode == "Rename" ? "quickRename" : mode == "Select" ? "quickSelect" : mode == "Rel" ? "relative" : "absolute";
        input_mode_button_->setIcon(icon(ic));
        const bool textual = mode == "Select" || mode == "Rename";
        input_field_->setVisible(textual);
        numbers->setVisible(!textual);
        lx->setVisible(!textual);
        ly->setVisible(!textual);
        lz->setVisible(!textual);
        if (textual) input_field_->setToolTip(mode == "Rename" ? "Rename the selected object(s)" : "Type text with wildcards to select objects");
        const QString numTip = mode == "Rel" ? "Relative values used for current tool" : "Absolute values used for current tool";
        for (QLineEdit* f : {num_x_, num_y_, num_z_}) f->setToolTip(numTip);
        if (mode == "Rename") {
            const QJsonArray sel = state()["selection"].toArray();
            input_field_->setText(sel.isEmpty() ? QString() : sel.last().toString().section('|', -1));
        } else if (mode == "Select") {
            input_field_->clear();
        }
        QSettings().setValue("statusLine/quickFieldMode", mode);
        QSignalBlocker block(mode_menu);
        for (QAction* a : mode_menu->actions()) a->setChecked(a->data().toString() == mode);
    };
    auto set_mode_action = [this, apply_mode](const QString& mode) {
        input_mode_ = mode;
        apply_mode();
    };
    auto* group = new QActionGroup(mode_menu);
    for (auto [label, mode] : {std::pair{"Absolute transform", "Abs"}, {"Relative transform", "Rel"}, {"Rename", "Rename"}, {"Select by name", "Select"}}) {
        QAction* a = mode_menu->addAction(label, this, [set_mode_action, mode] { set_mode_action(mode); });
        a->setData(mode);
        a->setCheckable(true);
        group->addAction(a);
    }
    input_mode_ = QSettings().value("statusLine/quickFieldMode", "Select").toString();
    apply_mode();
    connect(input_field_, &QLineEdit::returnPressed, this, [this] {
        input_entered(input_field_->text());
        if (input_mode_ == "Select") input_field_->clear();
    });
    for (QLineEdit* f : {num_x_, num_y_, num_z_}) connect(f, &QLineEdit::returnPressed, this, &MainWindow::numeric_input_entered);

    // The groups, each behind its handle, in the reference's order. A handle's checked state is folded.
    auto collapse = [this, left](const QString& key, const QString& tip, const QList<QWidget*>& members, bool visible = true) {
        auto* b = new QToolButton(left);
        b->setObjectName("statusHandle");
        b->setCheckable(true);
        b->setFixedSize(9, 22);
        b->setToolTip(tip);
        b->setFocusPolicy(Qt::NoFocus);
        b->setStyleSheet("QToolButton{border:none;background:transparent;color:#c0c0c0;font-size:8px;padding:0;}"
                         "QToolButton:hover{background:#4a4a4a;}");
        bool shown = QSettings().value("statusLine/" + key, visible).toBool();
        auto apply = [b, members, key](bool folded) {
            b->setText(folded ? QString::fromUtf8("\u25B6") : QString::fromUtf8("\u25C0"));
            for (QWidget* w : members) w->setVisible(!folded);
            QSettings().setValue("statusLine/" + key, !folded);
        };
        connect(b, &QToolButton::toggled, this, [apply](bool folded) { apply(folded); });
        b->setChecked(!shown);
        apply(!shown);
        return b;
    };
    auto add_group = [&](const QString& key, const QString& tip, const QList<QWidget*>& members, bool visible = true) {
        add(bar_gap(left));
        add(collapse(key, tip, members, visible));
        add(bar_gap(left));
        for (QWidget* w : members) add(w);
    };

    QList<QWidget*> file_widgets;
    for (QAction* a : {a_new, a_open, a_save, a_undo, a_redo}) file_widgets << bar_button(left, a);
    // Open's recent files, as the reference's Open button has.
    auto* open_button = qobject_cast<QToolButton*>(file_widgets[1]);
    open_button->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(open_button, &QToolButton::customContextMenuRequested, this, [this, open_button](const QPoint& at) {
        QMenu m;
        for (const QString& f : QSettings().value("recentFiles").toStringList())
            m.addAction(f, this, [this, f] { if (confirm_unsaved()) open_file(f); });
        if (m.isEmpty()) m.addAction("(none)")->setEnabled(false);
        m.exec(open_button->mapToGlobal(at));
    });
    add_group("showStatusFiles", "Show/Hide the file icons", file_widgets);

    QList<QWidget*> preset_widgets{preset_form};
    add_group("showStatusSelectionSet", "Show/Hide the selection set icons", preset_widgets, false);
    add_group("showStatusSelectMode", "Show/Hide the selection mode icons", mode_buttons);
    add_group("showStatusSelectMasks", "Show/Hide the selection mask icons", QList<QWidget*>{mask_set});
    add_group("showStatusSnap", "Show/Hide the snap icons", QList<QWidget*>{snap_group});
    add_group("showStatusSymmetry", "Show/Hide the symmetry control", QList<QWidget*>{symmetry_group});
    add_group("showStatusHistory", "Show/Hide the history icons", QList<QWidget*>{history_group}, false);
    add_group("showStatusRender", "Show/Hide the rendering icons", QList<QWidget*>{render_group});
    add_group("showStatusInputField", "Show/Hide the select field", QList<QWidget*>{input_group});
    // The IPM button has nothing to hold here, as the reference's placeholder on a build without it.
    auto* ipm = new QWidget(left);
    ipm->setSizePolicy(QSizePolicy::Fixed, QSizePolicy::Fixed);
    ipm->setFixedWidth(0);
    ipm->setFixedHeight(22);
    add_group("showStatusIPMButton", "Show/Hide the IPM Button", QList<QWidget*>{ipm});

    l->addStretch(1);
    row->addWidget(left, 1);

    // The editor toggles at the far right.
    auto* right = new QWidget(bar);
    auto* r = new QHBoxLayout(right);
    r->setContentsMargins(0, 0, 0, 0);
    r->setSpacing(1);
    auto toggle = [this, right, r](const char* ic, const QString& tip, std::function<void(bool)> fn, bool shown = true) {
        QAction* a = new QAction(icon(ic), tip, this);
        a->setToolTip(tip);
        a->setCheckable(true);
        connect(a, &QAction::toggled, this, fn);
        auto* b = bar_button(right, a);
        b->setVisible(shown);
        r->addWidget(b);
        return a;
    };
    mtk_button_ = toggle("modelingToolkit", "Toggle the Modeling Toolkit on/off", [this](bool on) {
        if (on) show_modeling_toolkit();
        else if (mtk_dock_) mtk_dock_->hide();
    });
    hik_button_ = toggle("humanIK", "Toggle the character controls", [this](bool on) {
        if (on) show_human_ik();
        else if (hik_dock_) hik_dock_->hide();
    });
    ae_button_ = toggle("attrEditor", "Toggle the Attribute Editor on/off", [this](bool on) {
        attr_dock_->setVisible(on);
        if (on) attr_dock_->raise();
    });
    tool_settings_button_ = toggle("toolSettings", "Toggle the Tool Settings on/off", [this](bool on) { tool_dock_->setVisible(on); });
    channel_box_button_ = toggle("channelBoxIcon", "Toggle the Channel Box / Layer Editor on/off", [this](bool on) {
        channel_dock_->setVisible(on);
        if (on) channel_dock_->raise();
    });
    layer_editor_button_ = toggle("layer", "Toggle the Layer Editor on/off", [](bool) {}, false);
    row->addWidget(right);

    tb->addWidget(bar);
    // Qt can leave an empty, unmanaged container over the first button (it is not in the bar's layout and has no
    // children); it would swallow the menu set button's clicks.
    auto clear_strays = [left] {
        for (QWidget* c : left->findChildren<QWidget*>(QString(), Qt::FindDirectChildrenOnly))
            if (c->metaObject()->className() == QLatin1String("QWidget") && c->layout() && c->layout()->count() == 0) c->hide();
    };
    clear_strays();
    QTimer::singleShot(0, this, clear_strays);
    QTimer::singleShot(250, this, clear_strays);
    home_button_ = nullptr;
}

void MainWindow::input_entered(const QString& raw) {
    QString text = raw.trimmed();
    if (text.isEmpty()) return;
    if (input_mode_ == "Rename") {
        QJsonArray sel = state()["selection"].toArray();
        if (sel.isEmpty()) return;
        for (int i = 0; i < sel.size(); ++i) exec("rename", args(QJsonArray{sel[i], sel.size() > 1 ? text + QString::number(i + 1) : text}));
        return;
    }
    // Select by name: wildcards as the outliner's filter, pCube* selecting every match.
    QJsonValue hits = ask("ls", args(QJsonArray{text}));
    if (hits.toArray().isEmpty()) {
        log(QString("// Warning: No object matches name: %1 //").arg(text), 4);
        return;
    }
    exec("select", args(hits.toArray()));
}

void MainWindow::numeric_input_entered() {
    bool given[3] = {false, false, false};
    double v[3] = {0, 0, 0};
    QLineEdit* fields[] = {num_x_, num_y_, num_z_};
    for (int i = 0; i < 3; ++i) {
        const QString t = fields[i]->text().trimmed();
        if (t.isEmpty()) continue;
        bool ok = false;
        v[i] = t.toDouble(&ok);
        given[i] = ok;
    }
    if (!given[0] && !given[1] && !given[2]) return;
    const QString t = tool_ == "rotate" ? "rotate" : tool_ == "scale" ? "scale" : "move";
    const double same = t == "scale" ? 1.0 : 0.0;
    for (int i = 0; i < 3; ++i)
        if (!given[i]) v[i] = same;
    QJsonArray by{v[0], v[1], v[2]};
    const bool comps = state()["mode"].toString() != "object";
    if (input_mode_ == "Rel" || comps) {
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
    for (const QJsonValue& o : state()["selection"].toArray())
        for (int i = 0; i < 3; ++i)
            if (given[i]) exec("setAttr", args(QJsonArray{o.toString() + "." + t + axis[i], v[i]}));
}

void MainWindow::sync_object_masks() {
    for (QAction* action : object_masks_) {
        const auto flags = action->property("maskFlags").toStringList();
        const QSignalBlocker block(action);
        action->setChecked(std::any_of(flags.begin(), flags.end(), [](const QString& flag) {
            return ask("selectType", QJsonObject{{"query", true}, {flag, true}}).toBool();
        }));
    }
    for (QAction* action : component_masks_) {
        const auto flags = action->property("maskFlags").toStringList();
        const QSignalBlocker block(action);
        action->setChecked(std::any_of(flags.begin(), flags.end(), [](const QString& flag) {
            return ask("selectType", QJsonObject{{"query", true}, {flag, true}}).toBool();
        }));
    }
}

void MainWindow::sync_status_line() {
    const QString scope = ask("selectScope").toString();
    if (scope_buttons_.contains(scope)) scope_buttons_[scope]->setChecked(true);
    const bool components = scope == "component";
    if (hierarchy_masks_) hierarchy_masks_->setVisible(scope == "hierarchy");
    if (object_masks_widget_) object_masks_widget_->setVisible(scope == "object");
    if (component_masks_widget_) component_masks_widget_->setVisible(components);
    sync_object_masks();
    if (lock_action_) {
        const QSignalBlocker block(lock_action_);
        lock_action_->setChecked(ask("selectPref", QJsonObject{{"query", true}, {"lockSelection", true}}).toBool());
    }
    if (input_mode_ == "Rename" && input_field_ && !input_field_->hasFocus()) {
        const QJsonArray sel = state()["selection"].toArray();
        input_field_->setText(sel.isEmpty() ? QString() : sel.last().toString().section('|', -1));
    }

    // Snapping, in the reference's order.
    auto snap = [](const char* flag) { return ask("snapMode", QJsonObject{{"query", true}, {flag, true}}).toBool(); };
    snap_on_.clear();
    for (QAction* a : snap_actions_) {
        const QString mode = a->property("snapMode").toString();
        const QSignalBlocker block(a);
        const bool on = mode == "grid" ? snap("grid") : mode == "curve" ? snap("curve") : mode == "point" ? snap("point")
                       : mode == "viewPlane" ? snap("viewPlane") : snap("meshCenter");
        a->setChecked(on);
        if (on) snap_on_.insert(mode);
    }
    {
        const QSignalBlocker block(live_button_);
        const QJsonArray live = ask("makeLive", QJsonObject{{"query", true}}).toArray();
        live_button_->setChecked(!live.isEmpty());
        if (live_text_) {
            if (live.isEmpty()) live_text_->setText("No Live Surface");
            else {
                QString name = live.first().toString().section('|', -1);
                if (live.size() > 1) name += QString(" and %1 more.").arg(live.size() - 1);
                live_text_->setText(name);
            }
        }
    }

    // Symmetry: the text and its highlight while on.
    auto sym = [](const char* flag) { return ask("symmetricModelling", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}); };
    if (symmetry_text_) {
        QString text = "Symmetry: Off";
        bool on = sym("symmetry").toBool();
        if (on) {
            const QString about = sym("about").toString(), axis = sym("axis").toString().toUpper();
            if (about == "object") text = "Object " + axis;
            else if (about == "world") text = "World " + axis;
            else if (about == "topo") text = "Topology";
        }
        symmetry_text_->setText(text);
        symmetry_text_->setStyleSheet(on ? "background:#5285a6;color:#ffffff;" : QString());
    }

    history_button_->setChecked(state()["history"].toBool());

    // The editor toggles' states, as the reference's updateEditorToggleCheckboxes.
    auto shown = [](QAction* a, bool on) {
        if (!a) return;
        const QSignalBlocker block(a);
        a->setChecked(on);
    };
    shown(mtk_button_, mtk_dock_ && mtk_dock_->isVisible());
    shown(hik_button_, hik_dock_ && hik_dock_->isVisible());
    shown(ae_button_, attr_dock_ && attr_dock_->isVisible());
    shown(tool_settings_button_, tool_dock_ && tool_dock_->isVisible());
    shown(channel_box_button_, channel_dock_ && channel_dock_->isVisible());
}

QStringList MainWindow::menu_set_names() const {
    return {"Modeling", "Rigging", "Animation", "FX", "Rendering"};
}

void MainWindow::set_menu_set(const QString& set) {
    const QString name = set == "Dynamics" ? QString("FX") : set;
    if (name.isEmpty() || name == "Customize ...") return;
    if (menu_set_button_) menu_set_button_->setText(name);
    if (name == menu_set_) return;
    menu_set_ = name;
    build_menu_set(name);
}

} // namespace az
