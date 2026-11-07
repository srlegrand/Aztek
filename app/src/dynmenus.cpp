// The Dynamics menu set: cloth, particles, rigid bodies, fields, constraints, caches and the solver, all one system.
#include "mainwindow.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QJsonObject>
#include <QMenu>

#include "host.h"

namespace az {

namespace {
// The selection's names.
QJsonArray selected() { return state()["selection"].toArray(); }
} // namespace

QList<QMenu*> MainWindow::dynamics_menus() {
    QList<QMenu*> out;

    auto* cloth = new QMenu("Cloth", this);
    cmd(cloth, "Create Cloth", "createCloth", {}, QString(), true);
    cmd(cloth, "Create Collider", "collider", {}, QString(), true);
    cloth->addSeparator();
    QMenu* presets = cloth->addMenu("Material Presets");
    struct P { const char* label; const char* name; };
    for (P p : {P{"Silk", "silk"}, P{"Cotton", "cotton"}, P{"Leather", "leather"}, P{"Heavy Denim", "heavyDenim"}, P{"Rubber", "rubber"}})
        cmd(presets, p.label, "clothPreset", QJsonObject{{"preset", p.name}});
    cloth->addSeparator();
    cloth->addAction("Vertices Out of Simulation", [] {
        QJsonValue v = ask("dynInfo", QJsonObject{});
        log(QString("// %1 //").arg(QString::fromUtf8(QJsonDocument(v.toObject()).toJson(QJsonDocument::Compact))));
    });
    out << cloth;

    auto* particles = new QMenu("Particles", this);
    cmd(particles, "Create Particles", "particle", QJsonObject{{"position", QJsonArray{QJsonArray{0, 0, 0}}}}, QString(), true);
    cmd(particles, "Create Emitter", "emitter", QJsonObject{{"type", "omni"}}, QString(), true);
    cmd(particles, "Emit from Object", "emitter", QJsonObject{{"type", "surf"}}, QString(), true);
    out << particles;

    auto* rigid = new QMenu("Rigid Bodies", this);
    cmd(rigid, "Create Active Rigid Body", "rigidBody", QJsonObject{{"active", true}}, QString(), true);
    cmd(rigid, "Create Passive Rigid Body", "rigidBody", QJsonObject{{"passive", true}}, QString(), true);
    out << rigid;

    auto* fields = new QMenu("Fields", this);
    struct F { const char* label; const char* cmd; };
    for (F f : {F{"Air", "air"}, F{"Drag", "drag"}, F{"Gravity", "gravity"}, F{"Newton", "newton"}, F{"Radial", "radial"}, F{"Turbulence", "turbulence"},
                F{"Uniform", "uniform"}, F{"Vortex", "vortex"}, F{"Volume Axis", "volumeAxis"}})
        cmd(fields, f.label, f.cmd, {}, QString(), true);
    fields->addSeparator();
    fields->addAction("Affect Selected Objects", [] {
        // The field last, the bodies before it.
        QJsonArray sel = selected();
        if (sel.size() < 2) { log("// Warning: select the objects, then the field //", 4); return; }
        QJsonArray bodies = sel;
        QString field = bodies.last().toString();
        bodies.removeLast();
        exec("connectDynamic", QJsonObject{{"args", bodies}, {"fields", QJsonArray{field}}});
    });
    out << fields;

    auto* cons = new QMenu("Constraints", this);
    struct C { const char* label; const char* type; };
    for (C c : {C{"Component to Component", "componentToComponent"}, C{"Point to Surface", "pointToSurface"}, C{"Weld Adjacent Borders", "weld"}, C{"Tearable Surface", "tear"},
                C{"Nail", "nail"}, C{"Pin", "pin"}, C{"Hinge", "hinge"}, C{"Spring", "spring"}})
        cmd(cons, c.label, "dynamicConstraint", QJsonObject{{"type", c.type}}, QString(), true);
    out << cons;

    auto* cache = new QMenu("Cache", this);
    cmd(cache, "Create New Cache", "dynCache", QJsonObject{{"create", true}}, QString(), true);
    cmd(cache, "Append to Cache", "dynCache", QJsonObject{{"append", true}});
    cmd(cache, "Replace Cache", "dynCache", QJsonObject{{"replace", true}, {"create", true}});
    cmd(cache, "Delete Cache", "dynCache", QJsonObject{{"delete", true}});
    out << cache;

    auto* solver = new QMenu("Solver", this);
    cmd(solver, "Create Solver", "createSolver");
    solver->addAction("Interactive Playback", [] { exec("InteractivePlayback"); });
    out << solver;
    return out;
}

} // namespace az
