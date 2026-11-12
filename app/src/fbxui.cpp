// The FBX Import and FBX Export options windows. Sections and labels follow the reference's FBX plug-in; each control
// runs that setting's command, and the Current Preset menu runs FBXResetImport/FBXResetExport (Autodesk Media &
// Entertainment) or the MotionBuilder preset.

#include "fbxui.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QFormLayout>
#include <QGroupBox>
#include <QInputDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QPointer>
#include <QPushButton>
#include <QScrollArea>
#include <QSettings>
#include <QSpinBox>
#include <QVBoxLayout>
#include <functional>

#include "host.h"
#include "usdui.h"

namespace az {

namespace {

const char* const kGroup = "fbx";

bool fbx_loaded() {
    return ask("pluginInfo", QJsonObject{{"args", QJsonArray{"fbx"}}, {"query", true}, {"loaded", true}}).toBool();
}

void remember(const QString& cmd, const QJsonValue& v) {
    QSettings s;
    s.beginGroup(kGroup);
    s.setValue(cmd, v.toVariant());
}

void forget_all(const QString& prefix) {
    QSettings s;
    s.beginGroup(kGroup);
    for (const QString& k : s.childKeys())
        if (k.startsWith(prefix)) s.remove(k);
}

QJsonValue query(const QString& cmd) {
    return ask(cmd, QJsonObject{{"query", true}});
}

void set_value(const QString& cmd, const QJsonValue& v) {
    exec(cmd, QJsonObject{{"value", v}});
    remember(cmd, v);
}

// A window of sections over the setting commands. Controls refresh from the plug-in when a preset changes them.
class FbxOptions : public QDialog {
public:
    FbxOptions(QWidget* parent, const QString& title, bool import) : QDialog(parent), import_(import) {
        setWindowTitle(title);
        resize(460, 680);
        auto* outer = new QVBoxLayout(this);
        auto* scroll = new QScrollArea(this);
        scroll->setWidgetResizable(true);
        auto* body = new QWidget(scroll);
        body_ = new QVBoxLayout(body);
        scroll->setWidget(body);
        outer->addWidget(scroll);
        auto* b = new QDialogButtonBox(QDialogButtonBox::Close, this);
        auto* reset = b->addButton("Reset Settings", QDialogButtonBox::ResetRole);
        connect(reset, &QPushButton::clicked, this, [this] { pick_preset(0); });
        connect(b, &QDialogButtonBox::rejected, this, &QDialog::reject);
        outer->addWidget(b);

        auto* presets = section("Presets");
        preset_ = new QComboBox(this);
        preset_->addItems({"Autodesk Media & Entertainment", "Autodesk MotionBuilder", "User defined"});
        preset_->setCurrentIndex(QSettings().value(QString(kGroup) + (import ? "/importPreset" : "/exportPreset"), 0).toInt());
        connect(preset_, QOverload<int>::of(&QComboBox::activated), this, [this](int k) { pick_preset(k); });
        presets->addRow("Current Preset", preset_);
    }

    QFormLayout* section(const QString& title, QFormLayout* in = nullptr) {
        auto* box = new QGroupBox(title, this);
        auto* form = new QFormLayout(box);
        if (in) in->addRow(box);
        else body_->addWidget(box);
        return form;
    }

    void finish() { body_->addStretch(1); }

    QCheckBox* check(QFormLayout* f, const QString& label, const QString& cmd) {
        auto* c = new QCheckBox(label, this);
        auto load = [c, cmd] {
            QSignalBlocker b(c);
            c->setChecked(query(cmd).toBool());
        };
        load();
        loaders_.push_back(load);
        connect(c, &QCheckBox::toggled, this, [this, cmd](bool on) {
            set_value(cmd, on);
            user_defined();
        });
        f->addRow(c);
        return c;
    }

    // Shown words and the values the command takes, in the same order.
    QComboBox* choice(QFormLayout* f, const QString& label, const QString& cmd, const QStringList& shown, const QStringList& values) {
        auto* c = new QComboBox(this);
        c->addItems(shown);
        auto load = [c, cmd, values] {
            QSignalBlocker b(c);
            const int k = values.indexOf(query(cmd).toVariant().toString());
            if (k >= 0) c->setCurrentIndex(k);
        };
        load();
        loaders_.push_back(load);
        connect(c, QOverload<int>::of(&QComboBox::activated), this, [this, cmd, values](int k) {
            set_value(cmd, values.value(k));
            user_defined();
        });
        f->addRow(label, c);
        return c;
    }

    QDoubleSpinBox* number(QFormLayout* f, const QString& label, const QString& cmd, double lo, double hi, int decimals) {
        auto* e = new QDoubleSpinBox(this);
        e->setRange(lo, hi);
        e->setDecimals(decimals);
        auto load = [e, cmd] {
            QSignalBlocker b(e);
            e->setValue(query(cmd).toDouble());
        };
        load();
        loaders_.push_back(load);
        connect(e, &QDoubleSpinBox::editingFinished, this, [this, e, cmd] {
            set_value(cmd, decimals_of(e) == 0 ? QJsonValue(qRound(e->value())) : QJsonValue(e->value()));
            user_defined();
        });
        f->addRow(label, e);
        return e;
    }

    // A check that greys out what depends on it.
    void governs(QCheckBox* c, const QList<QWidget*>& below, bool when_on = true) {
        auto apply = [c, below, when_on] {
            for (QWidget* w : below) w->setEnabled(c->isChecked() == when_on);
        };
        apply();
        connect(c, &QCheckBox::toggled, this, apply);
        refreshers_.push_back(apply);
    }

private:
    static int decimals_of(QDoubleSpinBox* e) { return e->decimals(); }

    void user_defined() {
        preset_->setCurrentIndex(2);
        QSettings().setValue(QString(kGroup) + (import_ ? "/importPreset" : "/exportPreset"), 2);
    }

    void pick_preset(int k) {
        const QString prefix = import_ ? "FBXImport" : "FBXExport";
        if (k == 0) az::exec(import_ ? "FBXResetImport" : "FBXResetExport");
        else if (k == 1) az::exec(import_ ? "FBXLoadMBImportPresetFile" : "FBXLoadMBExportPresetFile");
        if (k < 2) {
            forget_all(prefix);
            if (k == 1) {
                // The preset's own changes, remembered as settings.
                if (import_) {
                    remember("FBXImportConstraints", true);
                    remember("FBXImportSkeletonType", "humanik");
                } else {
                    remember("FBXExportConstraints", true);
                    remember("FBXExportSkeletonDefinitions", true);
                }
            }
        }
        preset_->setCurrentIndex(k);
        QSettings().setValue(QString(kGroup) + (import_ ? "/importPreset" : "/exportPreset"), k);
        for (auto& l : loaders_) l();
        for (auto& r : refreshers_) r();
    }

    bool import_;
    QVBoxLayout* body_;
    QComboBox* preset_;
    std::vector<std::function<void()>> loaders_;
    std::vector<std::function<void()>> refreshers_;
};

const QStringList kUnitsShown = {"Millimeters", "Centimeters", "Decimeters", "Meters", "Kilometers", "Inches", "Feet", "Yards", "Miles"};
const QStringList kUnits = {"mm", "cm", "dm", "m", "km", "In", "ft", "yd", "mi"};

} // namespace

void fbx_apply_saved() {
    if (!fbx_loaded()) return;
    QSettings s;
    s.beginGroup(kGroup);
    for (const QString& k : s.childKeys()) {
        if (!k.startsWith("FBX")) continue;
        exec_quiet(k, QJsonObject{{"value", QJsonValue::fromVariant(s.value(k))}});
    }
}

void fbx_import_options(QWidget* parent) {
    if (!fbx_loaded()) {
        log("// Error: the FBX options belong to the fbx plug-in, which is not loaded. Load it in Windows > Settings/Preferences > Plug-in Manager. //", 2);
        return;
    }
    fbx_apply_saved();
    FbxOptions d(parent, "FBX Import Options", true);
    auto* include = d.section("Include");
    d.choice(include, "File content", "FBXImportMode", {"Add", "Add and update animation", "Update animation"}, {"add", "merge", "exmerge"});
    auto* geo = d.section("Geometry", include);
    d.check(geo, "Smoothing Groups", "FBXImportSmoothingGroups");
    d.check(geo, "Unlock Normals", "FBXImportUnlockNormals");
    d.check(geo, "Combine per-vertex Normals", "FBXImportHardEdges");
    auto* anim = d.section("Animation", include);
    auto* anim_on = d.check(anim, "Animation", "FBXImportAnimation");
    auto* extra = d.section("Extra Options", anim);
    QList<QWidget*> under = {
        d.check(extra, "Fill Timeline", "FBXImportFillTimeline"),
        d.check(extra, "Bake Animation Layers", "FBXImportMergeAnimationLayers"),
        d.check(extra, "Optical Markers", "FBXImportOpticalMarkers"),
        d.choice(extra, "Quaternion Interpolation Mode", "FBXImportQuaternion", {"Resample As Euler Interpolation", "Retain Quaternion Interpolation", "Set As Euler Interpolation"}, {"resample", "quaternion", "euler"}),
        d.check(extra, "Protect Driven Keys", "FBXImportProtectDrivenKeys"),
        d.check(extra, "Deforming Elements to Joints", "FBXImportConvertDeformingNullsToJoint"),
        d.check(extra, "Update Pivots from Nulls", "FBXImportMergeBackNullPivots"),
        d.check(extra, "Geometry Cache File(s)", "FBXImportCacheFile"),
    };
    d.governs(anim_on, under);
    auto* deformed = d.section("Deformed Models", anim);
    d.check(deformed, "Skins", "FBXImportSkins");
    d.check(deformed, "Blend Shapes", "FBXImportShapes");
    d.check(deformed, "Pre-Normalize Weights", "FBXImportPreNormalizeWeights");
    auto* cons = d.section("Constraints", anim);
    d.check(cons, "Constraints", "FBXImportConstraints");
    d.choice(cons, "Skeleton Definitions", "FBXImportSkeletonType", {"None", "FBIK", "HumanIK"}, {"none", "fbik", "humanik"});
    auto* other = d.section("Cameras, Lights and Audio", include);
    d.check(other, "Cameras", "FBXImportCameras");
    d.check(other, "Lights", "FBXImportLights");
    d.check(other, "Audio", "FBXImportAudio");
    auto* adv = d.section("Advanced Options");
    auto* units = d.section("Units", adv);
    auto* automatic = d.check(units, "Automatic", "FBXImportAutoUnits");
    auto* to = d.choice(units, "File units converted to", "FBXImportConvertUnitString", kUnitsShown, kUnits);
    d.governs(automatic, {to}, false);
    auto* axis = d.section("Axis Conversion", adv);
    d.check(axis, "Axis conversion", "FBXImportAxisConversionEnable");
    d.choice(axis, "Up Axis", "FBXImportUpAxis", {"Y", "Z"}, {"y", "z"});
    auto* ui = d.section("UI", adv);
    d.check(ui, "Show Warning Manager", "FBXImportShowUI");
    d.check(ui, "Generate log data", "FBXImportGenerateLog");
    auto* perf = d.section("Performance", adv);
    d.check(perf, "Test meshes and remove bad polygons", "FBXImportRemoveBadPolysFromMesh");
    d.finish();
    d.exec();
}

void fbx_export_options(QWidget* parent) {
    if (!fbx_loaded()) {
        log("// Error: the FBX options belong to the fbx plug-in, which is not loaded. Load it in Windows > Settings/Preferences > Plug-in Manager. //", 2);
        return;
    }
    fbx_apply_saved();
    FbxOptions d(parent, "FBX Export Options", false);
    auto* include = d.section("Include");
    auto* geo = d.section("Geometry", include);
    d.check(geo, "Smoothing Groups", "FBXExportSmoothingGroups");
    d.check(geo, "Split per-vertex Normals", "FBXExportHardEdges");
    d.check(geo, "Tangents and Binormals", "FBXExportTangents");
    d.check(geo, "Smooth Mesh", "FBXExportSmoothMesh");
    d.check(geo, "Selection Sets", "FBXExportSelectionSets");
    d.check(geo, "Convert to Null Objects", "FBXExportNullsAsLocators");
    d.check(geo, "Preserve Instances", "FBXExportInstances");
    d.check(geo, "Referenced Asset Content", "FBXExportReferencedAssetsContent");
    d.check(geo, "Triangulate", "FBXExportTriangulate");
    d.choice(geo, "Convert NURBS surface to", "FBXExportConvertNurbsSurfaceTo", {"NURBS", "Interactive Display Mesh", "Software Render Mesh"}, {"nurbs", "interactiveDisplayMesh", "softwareRenderMesh"});
    auto* anim = d.section("Animation", include);
    auto* anim_on = d.check(anim, "Animation", "FBXExportAnimation");
    auto* extra = d.section("Extra Options", anim);
    QList<QWidget*> under = {
        d.check(extra, "Use scene name", "FBXExportUseSceneName"),
        d.check(extra, "Remove single key", "FBXExportRemoveSingleKey"),
        d.choice(extra, "Quaternion Interpolation Mode", "FBXExportQuaternion", {"Resample As Euler Interpolation", "Retain Quaternion Interpolation", "Set As Euler Interpolation"}, {"resample", "quaternion", "euler"}),
    };
    auto* bake = d.section("Bake Animation", anim);
    auto* bake_on = d.check(bake, "Bake Animation", "FBXExportBakeComplexAnimation");
    QList<QWidget*> baking = {
        d.number(bake, "Start", "FBXExportBakeComplexStart", -1e6, 1e6, 0),
        d.number(bake, "End", "FBXExportBakeComplexEnd", -1e6, 1e6, 0),
        d.number(bake, "Step", "FBXExportBakeComplexStep", 1, 1e4, 0),
        d.check(bake, "Resample All", "FBXExportBakeResampleAnimation"),
    };
    d.governs(bake_on, baking);
    auto* deformed = d.section("Deformed Models", anim);
    under << d.check(deformed, "Skins", "FBXExportSkins") << d.check(deformed, "Blend Shapes", "FBXExportShapes");
    auto* filters = d.section("Curve Filters", anim);
    under << d.check(filters, "Curve filters", "FBXExportCurveFilters") << d.check(filters, "Constant Key Reducer", "FBXExportApplyConstantKeyReducer");
    auto* cache = d.section("Geometry Cache File(s)", anim);
    under << d.check(cache, "Geometry cache file(s)", "FBXExportCacheFile");
    auto* cons = d.section("Constraints", anim);
    under << d.check(cons, "Constraints", "FBXExportConstraints") << d.check(cons, "Skeleton Definitions", "FBXExportSkeletonDefinitions");
    d.governs(anim_on, under);
    auto* other = d.section("Cameras, Lights, Audio and Media", include);
    d.check(other, "Cameras", "FBXExportCameras");
    d.check(other, "Lights", "FBXExportLights");
    d.check(other, "Audio", "FBXExportAudio");
    d.check(other, "Embed Media", "FBXExportEmbeddedTextures");
    auto* conn = d.section("Connections", include);
    d.check(conn, "Include Children", "FBXExportIncludeChildren");
    d.check(conn, "Input Connections", "FBXExportInputConnections");
    auto* adv = d.section("Advanced Options");
    auto* units = d.section("Units", adv);
    d.number(units, "Scale Factor", "FBXExportScaleFactor", 1e-6, 1e6, 6);
    auto* automatic = d.check(units, "Automatic", "FBXExportAutoUnits");
    auto* to = d.choice(units, "File units converted to", "FBXExportConvertUnitString", kUnitsShown, kUnits);
    d.governs(automatic, {to}, false);
    auto* axis = d.section("Axis Conversion", adv);
    d.choice(axis, "Up Axis", "FBXExportUpAxis", {"Y", "Z"}, {"y", "z"});
    auto* ui = d.section("UI", adv);
    d.check(ui, "Show Warning Manager", "FBXExportShowUI");
    d.check(ui, "Generate log data", "FBXExportGenerateLog");
    auto* format = d.section("FBX File Format", adv);
    d.choice(format, "Type", "FBXExportInAscii", {"Binary", "ASCII"}, {"false", "true"});
    d.choice(format, "Version", "FBXExportFileVersion", {"FBX 2020", "FBX 2019", "FBX 2018", "FBX 2016/2017", "FBX 2014/2015", "FBX 2013", "FBX 2012", "FBX 2011", "FBX 2010", "FBX 2009", "FBX 2006"},
             {"FBX202000", "FBX201900", "FBX201800", "FBX201600", "FBX201400", "FBX201300", "FBX201200", "FBX201100", "FBX201000", "FBX200900", "FBX200611"});
    d.finish();
    d.exec();
}

namespace {

QString pick_type(QWidget* parent, const QString& key, const QString& title) {
    QSettings s;
    const QStringList types = {"FBX", "USD"};
    bool ok = false;
    const QString last = s.value(key, "FBX").toString();
    const QString t = QInputDialog::getItem(parent, title, "File type:", types, qMax(0, types.indexOf(last)), false, &ok);
    if (!ok) return QString();
    s.setValue(key, t);
    return t;
}

} // namespace

void import_options(QWidget* parent) {
    const QString t = pick_type(parent, "import/optionsType", "Import Options");
    if (t == "FBX") fbx_import_options(parent);
    else if (t == "USD") usd_import_options(parent);
}

void export_options(QWidget* parent) {
    const QString t = pick_type(parent, "export/optionsType", "Export Options");
    if (t == "FBX") fbx_export_options(parent);
    else if (t == "USD") usd_export_options(parent);
}

} // namespace az
