// Render Settings: the Common tab (file output, cameras, image size) on defaultRenderGlobals and defaultResolution,
// the Harold tab on defaultArnoldRenderOptions, its filter and driver, and the AOVs tab on aiAOV nodes. Every control
// reads with getAttr and writes with setAttr, so each change is undoable and shows in the history.
#include "renderui.h"
#include "host.h"
#include "icons.h"
#include "panels.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDoubleSpinBox>
#include <QFormLayout>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QLineEdit>
#include <QListWidget>
#include <QMenu>
#include <QPointer>
#include <QPushButton>
#include <QScrollArea>
#include <QSpinBox>
#include <QTabWidget>
#include <QToolButton>
#include <QVBoxLayout>
#include <functional>

namespace az {

namespace {

const char* kGlobals = "defaultRenderGlobals";
const char* kRes = "defaultResolution";
const char* kOpt = "defaultArnoldRenderOptions";
const char* kFilter = "defaultArnoldFilter";
const char* kDriver = "defaultArnoldDriver";

// The AOVs Harold makes, in the order the list shows them.
const char* const kAovNames[] = {"RGBA", "direct", "indirect", "diffuse", "diffuse_direct", "diffuse_indirect", "specular", "specular_direct", "specular_indirect",
                                 "coat", "coat_direct", "coat_indirect", "sheen", "sheen_direct", "sheen_indirect", "sss", "sss_direct", "sss_indirect",
                                 "transmission", "transmission_direct", "transmission_indirect", "emission", "background", "volume", "albedo", "diffuse_albedo",
                                 "N", "P", "Z", "ID", "object_id", "material_id", "shadow_matte", "A"};

QJsonObject one(const QString& a) {
    return QJsonObject{{"args", QJsonArray{a}}};
}

double number(const QJsonValue& v) {
    if (v.isBool()) return v.toBool() ? 1 : 0;
    if (v.isArray()) return number(v.toArray().at(0));
    return v.toDouble();
}

QJsonValue get(const QString& plug) {
    return ask("getAttr", one(plug));
}

void set(const QString& plug, const QJsonValue& v) {
    QJsonObject c{{"args", QJsonArray{plug, v}}};
    if (v.isString()) c["type"] = "string";
    exec("setAttr", c);
}

class RenderSettings : public Panel {
public:
    explicit RenderSettings(QWidget* parent) : Panel(parent) {
        setWindowTitle("Render Settings");
        setObjectName("renderSettingsWindow");
        auto* v = new QVBoxLayout(this);
        v->setContentsMargins(4, 4, 4, 4);
        auto* top = new QHBoxLayout();
        top->addWidget(new QLabel("Render using: <b>Harold</b>", this));
        top->addStretch(1);
        v->addLayout(top);
        auto* tabs = new QTabWidget(this);
        tabs->addTab(scrolled(build_common()), "Common");
        tabs->addTab(scrolled(build_harold()), "Harold");
        tabs->addTab(build_aovs(), "AOVs");
        v->addWidget(tabs, 1);
        auto* close = new QPushButton("Close", this);
        auto* bottom = new QHBoxLayout();
        bottom->addStretch(1);
        bottom->addWidget(close);
        v->addLayout(bottom);
        connect(close, &QPushButton::clicked, this, &QWidget::close);
        resize(520, 760);
    }

protected:
    void refresh() override {
        loading_ = true;
        for (auto& r : readers_) r();
        loading_ = false;
        update_samples();
        refresh_cameras();
        refresh_path();
        refresh_atmosphere();
        refresh_aovs();
    }

private:
    // ---- builders ---------------------------------------------------------------------------------------------------

    static QWidget* scrolled(QWidget* body) {
        auto* s = new QScrollArea();
        s->setWidgetResizable(true);
        s->setFrameShape(QFrame::NoFrame);
        s->setWidget(body);
        return s;
    }

    // A collapsible frame, as the Attribute Editor's sections.
    QFormLayout* frame(QVBoxLayout* v, QWidget* parent, const QString& title, bool open = true) {
        auto* head = new QToolButton(parent);
        head->setObjectName("aeSectionButton");
        head->setText(title);
        head->setCheckable(true);
        head->setChecked(open);
        head->setArrowType(open ? Qt::DownArrow : Qt::RightArrow);
        head->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
        head->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        v->addWidget(head);
        auto* box = new QWidget(parent);
        auto* f = new QFormLayout(box);
        f->setLabelAlignment(Qt::AlignRight);
        f->setContentsMargins(8, 2, 2, 6);
        f->setVerticalSpacing(3);
        box->setVisible(open);
        v->addWidget(box);
        QObject::connect(head, &QToolButton::toggled, box, [box, head](bool on) {
            box->setVisible(on);
            head->setArrowType(on ? Qt::DownArrow : Qt::RightArrow);
        });
        return f;
    }

    QDoubleSpinBox* real(QFormLayout* f, const QString& label, const QString& plug, double lo, double hi, int decimals = 3, double step = 0.1) {
        auto* s = new QDoubleSpinBox();
        s->setRange(lo, hi);
        s->setDecimals(decimals);
        s->setSingleStep(step);
        s->setKeyboardTracking(false);
        s->setToolTip(plug);
        connect(s, &QDoubleSpinBox::valueChanged, this, [this, plug](double x) {
            if (!loading_) set(plug, x);
        });
        readers_ << [s, plug] { s->setValue(number(get(plug))); };
        f->addRow(label, s);
        return s;
    }

    QSpinBox* whole(QFormLayout* f, const QString& label, const QString& plug, int lo, int hi) {
        auto* s = new QSpinBox();
        s->setRange(lo, hi);
        s->setKeyboardTracking(false);
        s->setToolTip(plug);
        connect(s, &QSpinBox::valueChanged, this, [this, plug](int x) {
            if (!loading_) set(plug, x);
        });
        readers_ << [s, plug] { s->setValue(int(number(get(plug)))); };
        f->addRow(label, s);
        return s;
    }

    QCheckBox* tick(QFormLayout* f, const QString& label, const QString& plug, bool as_int = false) {
        auto* c = new QCheckBox();
        c->setToolTip(plug);
        connect(c, &QCheckBox::toggled, this, [this, plug, as_int](bool on) {
            if (!loading_) set(plug, as_int ? QJsonValue(on ? 1 : 0) : QJsonValue(on));
        });
        readers_ << [c, plug] { c->setChecked(number(get(plug)) != 0); };
        f->addRow(label, c);
        return c;
    }

    // An enum attribute: the item's index is the value.
    QComboBox* choice(QFormLayout* f, const QString& label, const QString& plug, const QStringList& items) {
        auto* b = new QComboBox();
        b->addItems(items);
        b->setToolTip(plug);
        connect(b, &QComboBox::activated, this, [this, plug](int i) {
            if (!loading_) set(plug, i);
        });
        readers_ << [b, plug] { b->setCurrentIndex(int(number(get(plug)))); };
        f->addRow(label, b);
        return b;
    }

    // A string attribute picked from a list: shown labels, written values.
    QComboBox* string_choice(QFormLayout* f, const QString& label, const QString& plug, const QList<QPair<QString, QString>>& items) {
        auto* b = new QComboBox();
        for (const auto& it : items) b->addItem(it.first, it.second);
        b->setToolTip(plug);
        connect(b, &QComboBox::activated, this, [this, b, plug](int i) {
            if (!loading_) set(plug, b->itemData(i).toString());
        });
        readers_ << [b, plug] {
            int k = b->findData(get(plug).toString());
            b->setCurrentIndex(k < 0 ? 0 : k);
        };
        f->addRow(label, b);
        return b;
    }

    QWidget* build_common() {
        auto* body = new QWidget();
        auto* v = new QVBoxLayout(body);
        v->setSpacing(2);
        path_ = new QLabel(body);
        path_->setWordWrap(true);
        path_->setTextInteractionFlags(Qt::TextSelectableByMouse);
        path_->setContentsMargins(4, 4, 4, 6);
        v->addWidget(path_);

        QFormLayout* f = frame(v, body, "File Output");
        auto* prefix = new QLineEdit(body);
        const QString tokens = "Tokens: <Scene> the scene's name, <RenderLayer> the layer, <Camera> the camera, <AOV> the AOV. A / makes a folder.";
        prefix->setToolTip(tokens);
        prefix->setPlaceholderText("(not set; using the scene name)");
        connect(prefix, &QLineEdit::editingFinished, this, [this, prefix] {
            if (!loading_ && prefix->text() != get(QString(kGlobals) + ".imageFilePrefix").toString()) set(QString(kGlobals) + ".imageFilePrefix", prefix->text());
        });
        readers_ << [prefix] {
            if (!prefix->hasFocus()) prefix->setText(get(QString(kGlobals) + ".imageFilePrefix").toString());
        };
        auto* token_button = new QToolButton(body);
        token_button->setText("<>");
        token_button->setToolTip("Insert a token");
        token_button->setPopupMode(QToolButton::InstantPopup);
        auto* token_menu = new QMenu(token_button);
        for (const char* t : {"<Scene>", "<RenderLayer>", "<Camera>", "<AOV>"}) {
            QString tk = t;
            token_menu->addAction(tk, [this, prefix, tk] {
                prefix->insert(tk);
                set(QString(kGlobals) + ".imageFilePrefix", prefix->text());
            });
        }
        token_button->setMenu(token_menu);
        auto* prow = new QHBoxLayout();
        prow->addWidget(prefix, 1);
        prow->addWidget(token_button);
        f->addRow("File name prefix", prow);
        auto* hint = new QLabel(tokens, body);
        hint->setWordWrap(true);
        hint->setStyleSheet("color: #999;");
        f->addRow("", hint);
        string_choice(f, "Image format", QString(kDriver) + ".ai_translator", {{"exr", "exr"}, {"png", "png"}, {"tif", "tif"}, {"jpeg", "jpg"}, {"tga", "tga"}});
        // Frame/Animation ext: whether there's a frame number, and where it goes.
        ext_ = new QComboBox(body);
        ext_->addItems({"name.ext (Single Frame)", "name.#.ext", "name.ext.#", "name#.ext", "name_#.ext"});
        connect(ext_, &QComboBox::activated, this, [this](int i) {
            if (loading_) return;
            QString g = kGlobals;
            set(g + ".animation", i != 0);
            if (i == 0) return;
            set(g + ".putFrameBeforeExt", i != 2);
            set(g + ".periodInExt", i == 3 ? 0 : i == 4 ? 2 : 1);
        });
        readers_ << [this] {
            QString g = kGlobals;
            int k = 0;
            if (number(get(g + ".animation")) != 0) {
                if (number(get(g + ".putFrameBeforeExt")) == 0) k = 2;
                else {
                    int period = int(number(get(g + ".periodInExt")));
                    k = period == 0 ? 3 : period == 2 ? 4 : 1;
                }
            }
            ext_->setCurrentIndex(k);
        };
        f->addRow("Frame/Animation ext", ext_);
        QWidget* frames[3];
        frames[0] = real(f, "Start frame", QString(kGlobals) + ".startFrame", -100000, 100000, 2, 1);
        frames[1] = real(f, "End frame", QString(kGlobals) + ".endFrame", -100000, 100000, 2, 1);
        frames[2] = real(f, "By frame", QString(kGlobals) + ".byFrameStep", 0.01, 10000, 2, 1);
        connect(ext_, &QComboBox::currentIndexChanged, this, [frames](int i) {
            for (QWidget* w : frames) w->setEnabled(i != 0);
        });
        whole(f, "Frame padding", QString(kGlobals) + ".extensionPadding", 1, 10);

        f = frame(v, body, "Renderable Cameras");
        camera_ = new QComboBox(body);
        camera_->setToolTip("The camera renders go through. Others are made not renderable.");
        connect(camera_, &QComboBox::activated, this, [this](int i) {
            if (loading_) return;
            QString pick = camera_->itemData(i).toString();
            for (int k = 0; k < camera_->count(); ++k) {
                QString shape = camera_->itemData(k).toString();
                bool on = shape == pick;
                if ((number(get(shape + ".renderable")) != 0) != on) set(shape + ".renderable", on);
            }
        });
        f->addRow("Renderable camera", camera_);

        f = frame(v, body, "Image Size");
        preset_ = new QComboBox(body);
        struct P { const char* name; int w, h; };
        static const P presets[] = {{"Custom", 0, 0}, {"640x480", 640, 480}, {"HD 720", 1280, 720}, {"HD 1080", 1920, 1080}, {"4K UHD", 3840, 2160}, {"1k Square", 1024, 1024}, {"2k Square", 2048, 2048}};
        for (const P& p : presets) preset_->addItem(p.name, QSize(p.w, p.h));
        connect(preset_, &QComboBox::activated, this, [this](int i) {
            QSize s = preset_->itemData(i).toSize();
            if (loading_ || s.isEmpty()) return;
            set(QString(kRes) + ".width", s.width());
            set(QString(kRes) + ".height", s.height());
            set(QString(kRes) + ".deviceAspectRatio", double(s.width()) / s.height() * number(get(QString(kRes) + ".pixelAspect")));
        });
        f->addRow("Presets", preset_);
        auto* w = whole(f, "Width", QString(kRes) + ".width", 1, 32768);
        auto* h = whole(f, "Height", QString(kRes) + ".height", 1, 32768);
        // The device aspect follows the size, as when Maintain ratio is off.
        auto aspect = [this] {
            if (loading_) return;
            double w = number(get(QString(kRes) + ".width")), h = number(get(QString(kRes) + ".height"));
            if (h > 0) set(QString(kRes) + ".deviceAspectRatio", w / h * number(get(QString(kRes) + ".pixelAspect")));
        };
        connect(w, &QSpinBox::valueChanged, this, aspect);
        connect(h, &QSpinBox::valueChanged, this, aspect);
        readers_ << [this] {
            QSize now(int(number(get(QString(kRes) + ".width"))), int(number(get(QString(kRes) + ".height"))));
            int k = preset_->findData(now);
            preset_->setCurrentIndex(k < 0 ? 0 : k);
        };
        auto* dar = new QLabel(body);
        readers_ << [dar] { dar->setText(QString::number(number(get(QString(kRes) + ".deviceAspectRatio")), 'f', 3)); };
        f->addRow("Device aspect ratio", dar);
        real(f, "Pixel aspect ratio", QString(kRes) + ".pixelAspect", 0.01, 10, 3, 0.1);

        f = frame(v, body, "Render Options", false);
        tick(f, "Enable default light", QString(kGlobals) + ".enableDefaultLight");
        v->addStretch(1);
        return body;
    }

    QWidget* build_harold() {
        auto* body = new QWidget();
        auto* v = new QVBoxLayout(body);
        v->setSpacing(2);
        QString o = kOpt;

        QFormLayout* f = frame(v, body, "Sampling");
        counts_ = new QLabel(body);
        counts_->setStyleSheet("color: #bbb;");
        f->addRow(counts_);
        aa_ = whole(f, "Camera (AA)", o + ".AASamples", -3, 100);
        diffuse_ = whole(f, "Diffuse", o + ".GIDiffuseSamples", 0, 100);
        specular_ = whole(f, "Specular", o + ".GISpecularSamples", 0, 100);
        transmission_ = whole(f, "Transmission", o + ".GITransmissionSamples", 0, 100);
        sss_ = whole(f, "SSS", o + ".GISssSamples", 0, 100);
        volume_ = whole(f, "Volume Indirect", o + ".GIVolumeSamples", 0, 100);
        for (QSpinBox* s : {aa_, diffuse_, specular_, transmission_, sss_, volume_}) connect(s, &QSpinBox::valueChanged, this, [this] { update_samples(); });
        tick(f, "Lock sampling pattern", o + ".lock_sampling_noise");
        whole(f, "Seed", o + ".AASeed", -100000, 100000);

        f = frame(v, body, "Adaptive Sampling", false);
        tick(f, "Enable", o + ".enableAdaptiveSampling");
        whole(f, "Max. Camera (AA)", o + ".AASamplesMax", 0, 100);
        real(f, "Adaptive threshold", o + ".AAAdaptiveThreshold", 0, 1, 4, 0.005);

        f = frame(v, body, "Clamping", false);
        tick(f, "Clamp sample values", o + ".use_sample_clamp");
        tick(f, "Affect AOVs", o + ".use_sample_clamp_AOVs");
        real(f, "AA max. value", o + ".AASampleClamp", 0, 1000, 3, 1);
        real(f, "Indirect max. value", o + ".indirectSampleClamp", 0, 1000, 3, 1);

        f = frame(v, body, "Filter", false);
        string_choice(f, "Type", QString(kFilter) + ".ai_translator",
                      {{"Gaussian", "gaussian"}, {"Box", "box"}, {"Triangle", "triangle"}, {"Blackman-Harris", "blackman_harris"}, {"Mitchell-Netravali", "mitnet"}, {"Catmull-Rom", "catrom"}, {"Sinc", "sinc"}, {"Closest", "closest"}});
        real(f, "Width", QString(kFilter) + ".width", 0, 10, 2, 0.5);

        f = frame(v, body, "Ray Depth", false);
        whole(f, "Total", o + ".GITotalDepth", 0, 100);
        whole(f, "Diffuse", o + ".GIDiffuseDepth", 0, 100);
        whole(f, "Specular", o + ".GISpecularDepth", 0, 100);
        whole(f, "Transmission", o + ".GITransmissionDepth", 0, 100);
        whole(f, "Volume", o + ".GIVolumeDepth", 0, 100);
        whole(f, "Transparency depth", o + ".autoTransparencyDepth", 0, 100);

        f = frame(v, body, "Environment", false);
        atmosphere_ = new QLabel(body);
        auto* make = new QPushButton("Create", body);
        make->setToolTip("A new atmosphere volume, made the scene's atmosphere");
        connect(make, &QPushButton::clicked, this, [] { create_atmosphere(); });
        auto* pick = new QPushButton("Select", body);
        connect(pick, &QPushButton::clicked, this, [this] {
            QString n = atmosphere_node();
            if (!n.isEmpty()) exec("select", one(n));
        });
        auto* cut = new QPushButton("Break", body);
        cut->setToolTip("Stop using this atmosphere; the node stays");
        connect(cut, &QPushButton::clicked, this, [this] {
            QString n = atmosphere_node();
            if (!n.isEmpty()) exec("disconnectAttr", QJsonObject{{"args", QJsonArray{n + ".message", QString(kOpt) + ".atmosphere[0]"}}});
        });
        auto* arow = new QHBoxLayout();
        arow->addWidget(atmosphere_, 1);
        arow->addWidget(make);
        arow->addWidget(pick);
        arow->addWidget(cut);
        f->addRow("Atmosphere", arow);

        f = frame(v, body, "Motion Blur", false);
        tick(f, "Enable", o + ".motion_blur_enable");
        tick(f, "Deformation", o + ".mb_object_deform_enable");
        tick(f, "Camera", o + ".mb_camera_enable");
        whole(f, "Keys", o + ".motion_steps", 2, 100);
        choice(f, "Position", o + ".range_type", {"Start On Frame", "Center On Frame", "End On Frame", "Custom"});
        real(f, "Length", o + ".motion_frames", 0, 10, 3, 0.1);
        real(f, "Start", o + ".motion_start", -10, 10, 3, 0.05);
        real(f, "End", o + ".motion_end", -10, 10, 3, 0.05);

        f = frame(v, body, "Lights", false);
        real(f, "Low light threshold", o + ".lowLightThreshold", 0, 1, 4, 0.001);

        f = frame(v, body, "Textures", false);
        tick(f, "Auto-mipmap", o + ".textureAutomip");
        whole(f, "Max. cache size (MB)", o + ".textureMaxMemoryMB", 1, 1 << 20);

        f = frame(v, body, "Subdivision", false);
        whole(f, "Max. subdivisions", o + ".maxSubdivisions", 0, 999);

        f = frame(v, body, "Operators", false);
        auto* none = new QLabel("Harold has no operators; nothing to set here.", body);
        none->setStyleSheet("color: #999;");
        f->addRow(none);

        f = frame(v, body, "Diagnostics", false);
        tick(f, "Autodetect threads", o + ".threads_autodetect", true);
        whole(f, "Threads", o + ".threads", -256, 256);
        whole(f, "Bucket size", o + ".bucketSize", 8, 1024);
        choice(f, "Bucket scanning", o + ".bucketScanning", {"Top", "Left", "Random", "Spiral", "Hilbert"});
        choice(f, "Verbosity", o + ".log_verbosity", {"Errors", "Warnings", "Info", "Debug"});
        struct I { const char* label; const char* attr; };
        for (I i : {I{"Ignore textures", "ignoreTextures"}, I{"Ignore shaders", "ignoreShaders"}, I{"Ignore atmosphere", "ignoreAtmosphere"}, I{"Ignore lights", "ignoreLights"},
                    I{"Ignore shadows", "ignoreShadows"}, I{"Ignore subdivision", "ignoreSubdivision"}, I{"Ignore displacement", "ignoreDisplacement"}, I{"Ignore bump", "ignoreBump"},
                    I{"Ignore motion blur", "ignoreMotionBlur"}, I{"Ignore depth of field", "ignoreDof"}, I{"Ignore SSS", "ignoreSss"}, I{"Ignore smoothing", "ignoreSmoothing"}})
            tick(f, i.label, o + "." + i.attr);

        f = frame(v, body, "Denoiser", false);
        tick(f, "Denoise the beauty", o + ".denoiseBeauty");
        v->addStretch(1);
        return body;
    }

    QWidget* build_aovs() {
        auto* body = new QWidget();
        auto* v = new QVBoxLayout(body);
        auto* g = new QGridLayout();
        g->addWidget(new QLabel("Available AOVs", body), 0, 0);
        g->addWidget(new QLabel("Active AOVs", body), 0, 2);
        available_ = new QListWidget(body);
        available_->setSelectionMode(QAbstractItemView::ExtendedSelection);
        active_ = new QListWidget(body);
        active_->setSelectionMode(QAbstractItemView::ExtendedSelection);
        g->addWidget(available_, 1, 0);
        g->addWidget(active_, 1, 2);
        auto* mid = new QVBoxLayout();
        auto* add = new QPushButton(">>", body);
        add->setToolTip("Make the picked AOVs active: an aiAOV node each");
        auto* remove = new QPushButton("<<", body);
        remove->setToolTip("Remove the picked AOVs: deletes their aiAOV nodes");
        mid->addStretch(1);
        mid->addWidget(add);
        mid->addWidget(remove);
        mid->addStretch(1);
        g->addLayout(mid, 1, 1);
        v->addLayout(g, 1);
        auto* note = new QLabel("Light group AOVs (RGBA_<group>) come from each light's AOV light group (aiAov). Untick an active AOV to keep its node but not render it. Double-click to edit its node.", body);
        note->setWordWrap(true);
        note->setStyleSheet("color: #999;");
        v->addWidget(note);
        auto activate = [this] {
            for (QListWidgetItem* it : available_->selectedItems()) {
                QString name = it->text();
                QString node = exec("createNode", QJsonObject{{"args", QJsonArray{"aiAOV"}}, {"name", "aiAOV_" + name}}).toString();
                if (!node.isEmpty()) set(node + ".name", name);
            }
        };
        auto deactivate = [this] {
            QJsonArray nodes;
            for (QListWidgetItem* it : active_->selectedItems()) nodes.append(it->data(Qt::UserRole).toString());
            if (!nodes.isEmpty()) exec("delete", QJsonObject{{"args", nodes}});
        };
        connect(add, &QPushButton::clicked, this, activate);
        connect(available_, &QListWidget::itemDoubleClicked, this, activate);
        connect(remove, &QPushButton::clicked, this, deactivate);
        connect(active_, &QListWidget::itemChanged, this, [this](QListWidgetItem* it) {
            if (loading_) return;
            QString node = it->data(Qt::UserRole).toString();
            bool on = it->checkState() == Qt::Checked;
            if ((number(get(node + ".enabled")) != 0) != on) set(node + ".enabled", on);
        });
        connect(active_, &QListWidget::itemDoubleClicked, this, [](QListWidgetItem* it) { exec("select", one(it->data(Qt::UserRole).toString())); });
        return body;
    }

    // ---- refreshing -------------------------------------------------------------------------------------------------

    void update_samples() {
        int aa = qMax(1, aa_->value());
        int a2 = aa * aa;
        auto sq = [](int x) { return x * x; };
        counts_->setText(QString("Samples per pixel, at most:  Camera (AA) %1   Diffuse %2   Specular %3   Transmission %4   SSS %5   Volume %6")
                             .arg(a2)
                             .arg(a2 * sq(diffuse_->value()))
                             .arg(a2 * sq(specular_->value()))
                             .arg(a2 * sq(transmission_->value()))
                             .arg(a2 * sq(sss_->value()))
                             .arg(a2 * sq(volume_->value())));
        counts_->setWordWrap(true);
    }

    void refresh_cameras() {
        loading_ = true;
        camera_->clear();
        int pick = -1;
        for (const QJsonValue& c : ask("listCameras").toArray()) {
            QString tr = c.toString();
            QJsonArray shapes = ask("listRelatives", QJsonObject{{"args", QJsonArray{tr}}, {"shapes", true}, {"fullPath", true}}).toArray();
            QString shape = shapes.isEmpty() ? tr : shapes.at(0).toString();
            camera_->addItem(tr.section('|', -1), shape);
            if (pick < 0 && number(get(shape + ".renderable")) != 0) pick = camera_->count() - 1;
        }
        camera_->setCurrentIndex(pick);
        loading_ = false;
    }

    void refresh_path() {
        QJsonValue first = ask("renderSettings", QJsonObject{{"firstImageName", true}, {"fullPath", true}});
        QString p = first.isArray() ? first.toArray().at(0).toString() : first.toString();
        int w = int(number(get(QString(kRes) + ".width"))), h = int(number(get(QString(kRes) + ".height")));
        path_->setText(QString("<b>Path:</b> %1<br><b>Image size:</b> %2 x %3").arg(p.toHtmlEscaped()).arg(w).arg(h));
    }

    QString atmosphere_node() const {
        QJsonArray c = ask("listConnections", QJsonObject{{"args", QJsonArray{QString(kOpt) + ".atmosphere"}}, {"source", true}, {"destination", false}}).toArray();
        return c.isEmpty() ? QString() : c.at(0).toString();
    }

    void refresh_atmosphere() {
        QString n = atmosphere_node();
        atmosphere_->setText(n.isEmpty() ? QString("(none)") : n);
    }

    void refresh_aovs() {
        loading_ = true;
        QSet<QString> on;
        active_->clear();
        for (const QJsonValue& v : ask("ls", QJsonObject{{"type", "aiAOV"}}).toArray()) {
            QString node = v.toString();
            QString name = get(node + ".name").toString();
            on.insert(name);
            auto* it = new QListWidgetItem(name.isEmpty() ? node : name, active_);
            it->setData(Qt::UserRole, node);
            it->setToolTip(node);
            it->setFlags(it->flags() | Qt::ItemIsUserCheckable);
            it->setCheckState(number(get(node + ".enabled")) != 0 ? Qt::Checked : Qt::Unchecked);
        }
        QStringList names;
        for (const char* n : kAovNames) names << n;
        // Light groups: each group a light names gets its own beauty.
        QStringList groups;
        for (const QJsonValue& l : ask("listLights", QJsonObject{{"shapes", true}}).toArray()) {
            QJsonValue g = get(l.toString() + ".aiAov");
            QString s = g.isString() ? g.toString().trimmed() : QString();
            if (!s.isEmpty() && s != "default" && !groups.contains(s)) groups << s;
        }
        groups.sort();
        for (const QString& gname : groups) names << "RGBA_" + gname;
        available_->clear();
        for (const QString& n : names)
            if (!on.contains(n)) new QListWidgetItem(n, available_);
        loading_ = false;
    }

    QList<std::function<void()>> readers_;
    bool loading_ = false;
    QLabel* path_ = nullptr;
    QComboBox* ext_ = nullptr;
    QComboBox* camera_ = nullptr;
    QComboBox* preset_ = nullptr;
    QLabel* counts_ = nullptr;
    QSpinBox *aa_ = nullptr, *diffuse_ = nullptr, *specular_ = nullptr, *transmission_ = nullptr, *sss_ = nullptr, *volume_ = nullptr;
    QLabel* atmosphere_ = nullptr;
    QListWidget* available_ = nullptr;
    QListWidget* active_ = nullptr;
};

QPointer<RenderSettings> g_settings;

} // namespace

void show_render_settings(QWidget* parent) {
    if (!g_settings) {
        g_settings = new RenderSettings(parent);
        g_settings->setWindowFlag(Qt::Window);
    }
    g_settings->show();
    g_settings->raise();
}

} // namespace az
