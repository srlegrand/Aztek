// A model panel's menus: View, Shading, Lighting, Show, Panels, and its icon bar. Each panel keeps its own settings.
#include "viewport.h"
#include "dopesheet.h"
#include "grapheditor.h"
#include "host.h"
#include "icons.h"
#include "nodeeditor.h"
#include "panels.h"

#include <QPointer>
#include <QAction>
#include <QActionGroup>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonDocument>
#include <QLineEdit>
#include <QMenu>
#include <QMenuBar>
#include <QSettings>
#include <cmath>
#include <QStackedLayout>
#include <QToolBar>

namespace az {

namespace {

QString disp_json(const char* key, bool on) {
    return QString("{\"%1\":%2}").arg(key, on ? "true" : "false");
}

// A checkable item for one display setting, reading the view when its menu opens.
QAction* flag_item(QMenu* m, ViewportWidget* v, const QString& label, const char* key, QHash<QString, QAction*>& items) {
    QAction* a = m->addAction(label);
    a->setCheckable(true);
    QString k = key;
    QObject::connect(a, &QAction::triggered, m, [v, k](bool on) { v->set("disp", disp_json(k.toUtf8().constData(), on)); });
    items[k] = a;
    return a;
}

PanelLayout* layout_of(QWidget* w) {
    for (QWidget* p = w; p; p = p->parentWidget())
        if (auto* pl = qobject_cast<PanelLayout*>(p)) return pl;
    return nullptr;
}

} // namespace

void ModelPanel::set_panel_type(const QString& type) {
    type_ = type;
    QWidget* show = view_;
    if (type != "model") {
        if (!others_.contains(type)) {
            QWidget* w = nullptr;
            if (type == "outliner") w = new Outliner(this);
            else if (type == "graph") w = new GraphEditor(this);
            else if (type == "dope") w = new DopeSheet(this);
            else if (type == "node") w = new NodeEditor(this);
            if (!w) return;
            stack_->addWidget(w);
            others_[type] = w;
        }
        show = others_[type];
    }
    stack_->setCurrentWidget(show);
    bar_->setVisible(type == "model");
}

void ModelPanel::build_menus() {
    auto* v = view_;
    auto set = [v](const char* k, const QString& j) { return [v, k, j] { v->set(k, j); }; };
    auto* flags = new QHash<QString, QAction*>();
    auto sync_flags = [v, flags] {
        QJsonObject d = parse(v->get("disp")).toObject();
        for (auto it = flags->begin(); it != flags->end(); ++it)
            if (d.contains(it.key())) it.value()->setChecked(d[it.key()].toBool());
    };

    // View: cameras first.
    QMenu* view = menus_->addMenu("View");
    QMenu* select_cam = nullptr;
    view->addAction("Select Camera", [v] { exec("select", QJsonObject{{"args", QJsonArray{v->get("camera").remove('"')}}}); });
    view->addAction("Previous View\t[", set("viewUndo", "true"));
    view->addAction("Next View\t]", set("viewRedo", "true"));
    view->addAction("Default View", set("home", "true"));
    view->addAction("Look at Selection", set("frame", "\"selection\""));
    view->addAction("Frame All\tA", set("frame", "\"all\""));
    view->addAction("Frame Selection\tF", set("frame", "\"selection\""));
    view->addSeparator();
    view->addAction("Look Through Selected", [v] {
        QJsonArray sel = state()["selection"].toArray();
        if (sel.isEmpty()) {
            log("// Warning: select a camera to look through //", 4);
            return;
        }
        QString c = sel.last().toString();
        if (!ask("listLights", QJsonObject{{"selection", true}}).toArray().isEmpty()) {
            exec("lookThru", QJsonObject{{"args", QJsonArray{c}}});
            return;
        }
        if (ask("camera", QJsonObject{{"args", QJsonArray{c}}, {"query", true}, {"queried", QJsonArray{"focalLength"}}}).isUndefined()) {
            log(QString("// Warning: %1 is not a camera //").arg(c), 4);
            return;
        }
        exec("lookThru", QJsonObject{{"args", QJsonArray{c}}});
    });
    view->addAction("Camera Attribute Editor...", [v] {
        exec("select", QJsonObject{{"args", QJsonArray{v->get("camera").remove('"')}}});
        mel("AttributeEditor;");
    });
    QMenu* tools = view->addMenu("Camera Tools");
    struct T { const char* label; const char* cmd; };
    for (T t : {T{"Tumble Tool", "TumbleTool;"}, T{"Track Tool", "TrackTool;"}, T{"Dolly Tool", "DollyTool;"}, T{"Zoom Tool", "ZoomTool;"}, T{"Roll Tool", "RollTool;"}, T{"Yaw-Pitch Tool", "YawPitchTool;"}}) {
        QString c = t.cmd;
        tools->addAction(t.label, [c] { mel(c); });
    }
    QMenu* bookmarks = view->addMenu("Bookmarks");
    connect(bookmarks, &QMenu::aboutToShow, this, [this, v, bookmarks] {
        bookmarks->clear();
        bookmarks->addAction("Add Bookmark...", [this, v] {
            bool ok = false;
            QString n = QInputDialog::getText(this, "Add Bookmark", "Name:", QLineEdit::Normal, "cameraView1", &ok);
            if (!ok || n.isEmpty()) return;
            QSettings s;
            s.beginGroup("bookmarks/" + v->get("camera").remove('"'));
            s.setValue(n, v->get("camState"));
        });
        bookmarks->addSeparator();
        QSettings s;
        s.beginGroup("bookmarks/" + v->get("camera").remove('"'));
        for (const QString& k : s.childKeys()) {
            QString st = s.value(k).toString();
            bookmarks->addAction(k, [v, st] { v->set("camState", st); });
        }
    });
    QMenu* settings = view->addMenu("Camera Settings");
    flag_item(settings, v, "Film Gate", "filmGate", *flags);
    flag_item(settings, v, "Resolution Gate", "resGate", *flags);
    flag_item(settings, v, "Safe Action", "safeAction", *flags);
    connect(settings, &QMenu::aboutToShow, this, sync_flags);
    (void)select_cam;

    // Shading.
    QMenu* shading = menus_->addMenu("Shading");
    auto* group = new QActionGroup(this);
    auto shade = [this, v, shading, group](const QString& label, bool wire, bool flat, bool box) {
        QAction* a = shading->addAction(label, [v, wire, flat, box] {
            v->set("wireframe", wire ? "true" : "false");
            v->set("disp", QString("{\"flat\":%1,\"boundingBox\":%2}").arg(flat ? "true" : "false", box ? "true" : "false"));
        });
        a->setCheckable(true);
        group->addAction(a);
        return a;
    };
    QAction* wire = shade("Wireframe\t4", true, false, false);
    QAction* smooth = shade("Smooth Shade All\t5", false, false, false);
    shade("Flat Shade All", false, true, false);
    shade("Bounding Box", false, false, true);
    smooth->setChecked(true);
    (void)wire;
    shading->addSeparator();
    flag_item(shading, v, "Hardware Texturing\t6", "textured", *flags);
    flag_item(shading, v, "Use Default Material", "defaultMaterial", *flags);
    shading->addSeparator();
    QAction* wos = shading->addAction("Wireframe on Shaded");
    wos->setCheckable(true);
    connect(wos, &QAction::toggled, this, [v](bool on) { v->set("wireOnShaded", on ? "true" : "false"); });
    flag_item(shading, v, "X-Ray\tAlt+A", "xray", *flags);
    flag_item(shading, v, "X-Ray Joints", "xrayJoints", *flags);
    flag_item(shading, v, "X-Ray Active Components", "xrayActive", *flags);
    flag_item(shading, v, "Backface Culling", "backface", *flags);
    connect(shading, &QMenu::aboutToShow, this, sync_flags);

    // Lighting: what lights the view, modelEditor -displayLights.
    QMenu* lighting = menus_->addMenu("Lighting");
    auto* light_group = new QActionGroup(this);
    auto* light_items = new QHash<QString, QAction*>();
    auto light_item = [v, lighting, light_group, light_items](const QString& label, const QString& mode) {
        QAction* a = lighting->addAction(label, [v, mode] { v->set("disp", QString("{\"displayLights\":\"%1\"}").arg(mode)); });
        a->setCheckable(true);
        light_group->addAction(a);
        (*light_items)[mode] = a;
    };
    light_item("Use Default Lighting", "default");
    light_item("Use All Lights\t7", "all");
    light_item("Use Selected Lights", "active");
    light_item("Use Flat Lighting", "flat");
    light_item("Use No Lights", "none");
    lighting->addSeparator();
    flag_item(lighting, v, "Two Sided Lighting", "twoSided", *flags);
    flag_item(lighting, v, "Shadows", "shadows", *flags);
    // Ambient occlusion: this view's switch, with its reach and strength under it. The choice is kept for new views.
    QAction* ao = lighting->addAction("Ambient Occlusion");
    ao->setCheckable(true);
    connect(ao, &QAction::toggled, this, [v](bool on) {
        v->set("disp", QString("{\"ambientOcclusion\":%1}").arg(on ? "true" : "false"));
        QSettings().setValue("viewport/ambientOcclusion", on);
    });
    QMenu* ao_menu = lighting->addMenu("Ambient Occlusion Settings");
    auto ao_choice = [v, ao_menu](const QString& title, const QString& key, const QList<QPair<QString, double>>& values) {
        QMenu* m = ao_menu->addMenu(title);
        auto* group = new QActionGroup(m);
        for (const auto& item : values) {
            QAction* a = m->addAction(item.first, [v, key, value = item.second] {
                v->set("disp", QString("{\"%1\":%2}").arg(key).arg(value));
                QSettings().setValue("viewport/" + key, value);
            });
            a->setCheckable(true);
            a->setData(item.second);
            group->addAction(a);
        }
        connect(m, &QMenu::aboutToShow, m, [v, m, key] {
            const double now = parse(v->get("disp")).toObject()[key].toDouble();
            for (QAction* a : m->actions()) a->setChecked(std::abs(a->data().toDouble() - now) < 1e-6);
        });
    };
    ao_choice("Radius", "aoRadius", {{"0.5", 0.5}, {"1", 1.0}, {"2", 2.0}, {"4", 4.0}, {"8", 8.0}, {"16", 16.0}, {"32", 32.0}});
    ao_choice("Intensity", "aoIntensity", {{"0.5", 0.5}, {"1", 1.0}, {"1.5", 1.5}, {"2", 2.0}, {"3", 3.0}});
    // Anti-aliasing: samples a pixel, changed at once in every view and kept for the next start. The tick is what this
    // view draws with, which is less than asked where the device has no more to give.
    QMenu* aa = lighting->addMenu("Anti-aliasing");
    auto* aa_group = new QActionGroup(aa);
    // Every view's toolbar button, so they all show what the views draw with.
    static QList<QPointer<QAction>> aa_buttons;
    auto apply_samples = [this, v](int n) {
        QSettings st;
        st.setValue("viewport/samples", n);
        if (n > 1) st.setValue("viewport/samplesOn", n);
        QList<ViewportWidget*> views{v};
        if (PanelLayout* pl = layout_of(this)) views = pl->views();
        for (ViewportWidget* each : views) {
            each->set("samples", QString::number(n));
            each->update();
        }
        const int got = parse(v->get("disp")).toObject()["samples"].toInt();
        if (got != n)
            log(QString("// Warning: this graphics device gives %1 a pixel where %2 were asked for //")
                    .arg(got == 1 ? QString("1 sample") : QString("%1 samples").arg(got))
                    .arg(n),
                4);
        for (const QPointer<QAction>& b : aa_buttons)
            if (b) {
                QSignalBlocker block(b);
                b->setChecked(got > 1);
            }
    };
    for (int n : {1, 2, 4, 8}) {
        QAction* a = aa->addAction(n == 1 ? QString("Off") : QString("%1 Samples").arg(n), [apply_samples, n] { apply_samples(n); });
        a->setCheckable(true);
        a->setData(n);
        aa_group->addAction(a);
    }
    aa_apply_ = [apply_samples](bool on) { apply_samples(on ? qMax(2, QSettings().value("viewport/samplesOn", 4).toInt()) : 1); };
    aa_register_ = [](QAction* b) { aa_buttons.append(b); };
    connect(aa, &QMenu::aboutToShow, aa, [v, aa] {
        const int now = parse(v->get("disp")).toObject()["samples"].toInt();
        for (QAction* a : aa->actions()) a->setChecked(a->data().toInt() == now);
    });
    // A new view takes the occlusion last chosen.
    {
        QSettings st;
        ao->blockSignals(true);
        ao->setChecked(st.value("viewport/ambientOcclusion", false).toBool());
        ao->blockSignals(false);
        v->set("disp", QString("{\"ambientOcclusion\":%1,\"aoRadius\":%2,\"aoIntensity\":%3}")
                           .arg(ao->isChecked() ? "true" : "false")
                           .arg(st.value("viewport/aoRadius", 2.0).toDouble())
                           .arg(st.value("viewport/aoIntensity", 1.5).toDouble()));
    }
    connect(lighting, &QMenu::aboutToShow, this, [v, light_items, sync_flags, ao] {
        ao->blockSignals(true);
        ao->setChecked(parse(v->get("disp")).toObject()["ambientOcclusion"].toBool());
        ao->blockSignals(false);
        sync_flags();
        QAction* a = light_items->value(parse(v->get("disp")).toObject()["displayLights"].toString());
        if (a) a->setChecked(true);
    });

    // Show: what this view draws.
    QMenu* show = menus_->addMenu("Show");
    show->addAction("All", [v] { v->set("disp", "{\"polygons\":true,\"surfaces\":true,\"curves\":true,\"joints\":true,\"ikHandles\":true,\"deformers\":true,\"locators\":true,\"cameras\":true,\"lights\":true}"); });
    show->addAction("None", [v] { v->set("disp", "{\"polygons\":false,\"surfaces\":false,\"curves\":false,\"joints\":false,\"ikHandles\":false,\"deformers\":false,\"locators\":false,\"cameras\":false,\"lights\":false}"); });
    show->addSeparator();
    QMenu* iso = show->addMenu("Isolate Select");
    QAction* iso_view = iso->addAction("View Selected\tCtrl+1");
    iso_view->setCheckable(true);
    connect(iso_view, &QAction::triggered, this, [v](bool on) { v->set("isolate", QString("{\"state\":%1}").arg(on ? "true" : "false")); });
    connect(iso, &QMenu::aboutToShow, this, [v, iso_view] { iso_view->setChecked(v->get("isolated") == "true"); });
    iso->addAction("Add Selected Objects", [v] { v->set("isolate", "{\"addSelected\":true}"); });
    iso->addAction("Remove Selected Objects", [v] { v->set("isolate", "{\"removeSelected\":true}"); });
    show->addSeparator();
    flag_item(show, v, "Polygons", "polygons", *flags);
    flag_item(show, v, "NURBS Curves", "curves", *flags);
    flag_item(show, v, "NURBS Surfaces", "surfaces", *flags);
    flag_item(show, v, "Joints", "joints", *flags);
    flag_item(show, v, "IK Handles", "ikHandles", *flags);
    flag_item(show, v, "Deformers", "deformers", *flags);
    flag_item(show, v, "Locators", "locators", *flags);
    flag_item(show, v, "Cameras", "cameras", *flags);
    flag_item(show, v, "Lights", "lights", *flags);
    show->addSeparator();
    flag_item(show, v, "Manipulators", "manipulators", *flags);
    QAction* grid = show->addAction("Grid");
    grid->setCheckable(true);
    grid->setChecked(true);
    connect(grid, &QAction::toggled, this, [v](bool on) { v->set("grid", on ? "true" : "false"); });
    connect(show, &QMenu::aboutToShow, this, sync_flags);

    // Panels: which camera or editor is here, and the layout.
    QMenu* panels = menus_->addMenu("Panels");
    QMenu* persp = panels->addMenu("Perspective");
    QMenu* ortho = panels->addMenu("Orthographic");
    auto fill_cams = [this, v](QMenu* m, bool want_ortho) {
        m->clear();
        for (const QJsonValue& n : ask("ls", QJsonObject{{"type", "camera"}}).toArray()) {
            QString shape = n.toString();
            QString xf = ask("listRelatives", QJsonObject{{"args", QJsonArray{shape}}, {"parent", true}}).toArray().at(0).toString();
            if (xf.isEmpty()) continue;
            bool is_ortho = ask("getAttr", QJsonObject{{"args", QJsonArray{shape + ".orthographic"}}}).toBool();
            if (is_ortho != want_ortho) continue;
            m->addAction(xf, [this, v, xf] {
                set_panel_type("model");
                v->set("camera", "\"" + xf + "\"");
            });
        }
    };
    connect(persp, &QMenu::aboutToShow, this, [fill_cams, persp] { fill_cams(persp, false); });
    connect(ortho, &QMenu::aboutToShow, this, [fill_cams, ortho] { fill_cams(ortho, true); });
    QMenu* type = panels->addMenu("Panel");
    struct P { const char* label; const char* id; };
    for (P p : {P{"Model Panel", "model"}, P{"Outliner", "outliner"}, P{"Graph Editor", "graph"}, P{"Dope Sheet", "dope"}, P{"Node Editor", "node"}}) {
        QString id = p.id;
        type->addAction(p.label, [this, id] { set_panel_type(id); });
    }
    panels->addSeparator();
    QMenu* lay = panels->addMenu("Layouts");
    struct L { const char* label; int kind; };
    for (L l : {L{"Single Pane", 0}, L{"Two Panes Side by Side", 1}, L{"Two Panes Stacked", 2}, L{"Three Panes", 3}, L{"Four Panes", 4}}) {
        int k = l.kind;
        lay->addAction(l.label, [this, k] {
            if (PanelLayout* pl = layout_of(this)) pl->set_layout(k);
        });
    }
    QMenu* saved = panels->addMenu("Saved Layouts");
    connect(saved, &QMenu::aboutToShow, this, [this, saved] {
        saved->clear();
        saved->addAction("Save Current Layout...", [this] {
            bool ok = false;
            QString n = QInputDialog::getText(this, "Save Layout", "Name:", QLineEdit::Normal, "layout1", &ok);
            PanelLayout* pl = layout_of(this);
            if (ok && !n.isEmpty() && pl) QSettings().setValue("panelLayouts/" + n, pl->state_json());
        });
        saved->addSeparator();
        QSettings s;
        s.beginGroup("panelLayouts");
        for (const QString& k : s.childKeys()) {
            QString st = s.value(k).toString();
            saved->addAction(k, [this, st] {
                if (PanelLayout* pl = layout_of(this)) pl->restore_json(st);
            });
        }
    });
    panels->addSeparator();
    panels->addAction("Single Pane / Four Panes\tSpace", [this] {
        if (PanelLayout* pl = layout_of(this)) pl->toggle();
    });

    // The icon bar.
    bar_->addAction(icon("selectCamera"), "Select Camera", [v] { exec("select", QJsonObject{{"args", QJsonArray{v->get("camera").remove('"')}}}); });
    bar_->addAction(icon("bookmark"), "Previous View ([)", set("viewUndo", "true"));
    bar_->addAction(icon("frameAll"), "Frame All (A)", set("frame", "\"all\""));
    bar_->addAction(icon("frameSel"), "Frame Selection (F)", set("frame", "\"selection\""));
    bar_->addSeparator();
    QAction* grid2 = bar_->addAction(icon("grid"), "Grid");
    grid2->setCheckable(true);
    grid2->setChecked(true);
    connect(grid2, &QAction::toggled, grid, &QAction::setChecked);
    connect(grid, &QAction::toggled, grid2, &QAction::setChecked);
    auto bar_flag = [this, v](const char* ic, const char* tip, const char* key) {
        QString k = key;
        QAction* a = bar_->addAction(icon(ic), tip);
        a->setCheckable(true);
        connect(a, &QAction::triggered, this, [v, k](bool on) { v->set("disp", disp_json(k.toUtf8().constData(), on)); });
    };
    bar_flag("filmGate", "Film Gate", "filmGate");
    bar_flag("resGate", "Resolution Gate", "resGate");
    bar_->addSeparator();
    bar_->addAction(icon("wireframe"), "Wireframe (4)", set("wireframe", "true"));
    bar_->addAction(icon("shaded"), "Smooth Shade All (5)", set("wireframe", "false"));
    bar_flag("defaultMaterial", "Use Default Material", "defaultMaterial");
    bar_flag("textured", "Textured (6)", "textured");
    QAction* all_lights = bar_->addAction(icon("lightsAll"), "Use All Lights (7)");
    all_lights->setCheckable(true);
    connect(all_lights, &QAction::triggered, this, [v](bool on) { v->set("disp", QString("{\"displayLights\":\"%1\"}").arg(on ? "all" : "default")); });
    connect(lighting, &QMenu::aboutToShow, all_lights, [v, all_lights] { all_lights->setChecked(parse(v->get("disp")).toObject()["displayLights"].toString() == "all"); });
    bar_flag("shadows", "Shadows", "shadows");
    // Anti-aliasing, as the reference's toolbar has it: on at the count last chosen, or off.
    QAction* aa_button = bar_->addAction(icon("antiAlias"), "Multisample Anti-aliasing");
    aa_button->setCheckable(true);
    // What the view starts with: the environment's count, else the saved one. The view itself is not made yet.
    aa_button->setChecked((qEnvironmentVariableIsEmpty("AZTEK_MSAA") ? QSettings().value("viewport/samples", 4).toInt() : qEnvironmentVariableIntValue("AZTEK_MSAA")) > 1);
    aa_register_(aa_button);
    connect(aa_button, &QAction::triggered, this, [this](bool on) { aa_apply_(on); });
    QAction* wos2 = bar_->addAction(icon("wireOnShaded"), "Wireframe on Shaded");
    wos2->setCheckable(true);
    connect(wos2, &QAction::toggled, wos, &QAction::setChecked);
    connect(wos, &QAction::toggled, wos2, &QAction::setChecked);
    bar_flag("xray", "X-Ray (Alt+A)", "xray");
    bar_flag("xrayJoints", "X-Ray Joints", "xrayJoints");
    bar_flag("xrayActive", "X-Ray Active Components", "xrayActive");
    bar_flag("backface", "Backface Culling", "backface");
    bar_->addSeparator();
    QAction* iso2 = bar_->addAction(icon("isolate"), "Isolate Select (Ctrl+1)");
    iso2->setCheckable(true);
    connect(iso2, &QAction::triggered, this, [v](bool on) { v->set("isolate", QString("{\"state\":%1}").arg(on ? "true" : "false")); });
}

} // namespace az
