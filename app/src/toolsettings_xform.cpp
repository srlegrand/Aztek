// Tool Settings for the Select, Move, Rotate and Scale tools: orientation, step snap, symmetry, soft selection and pivot.
// Options are the manip contexts' flags, so scripts and this page see the same values. They are saved between sessions.
#include "toolsettings.h"
#include "host.h"

#include <QCheckBox>
#include <QComboBox>
#include <QDoubleSpinBox>
#include <QFormLayout>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QLabel>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QPushButton>
#include <QScrollArea>
#include <QSettings>
#include <QToolButton>
#include <QVBoxLayout>

#include <algorithm>
#include <cmath>

namespace az {

namespace {

// A titled part of the page that folds away.
QWidget* section(QVBoxLayout* page, const QString& title, QFormLayout*& form) {
    auto* head = new QToolButton();
    head->setText(title);
    head->setCheckable(true);
    head->setChecked(true);
    head->setArrowType(Qt::DownArrow);
    head->setToolButtonStyle(Qt::ToolButtonTextBesideIcon);
    head->setAutoRaise(true);
    head->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    auto* body = new QWidget();
    form = new QFormLayout(body);
    form->setContentsMargins(8, 2, 2, 6);
    QObject::connect(head, &QToolButton::toggled, body, [head, body](bool open) {
        body->setVisible(open);
        head->setArrowType(open ? Qt::DownArrow : Qt::RightArrow);
    });
    page->addWidget(head);
    page->addWidget(body);
    return body;
}

// Soft select's falloff: weight up, distance across. Drag a key, double-click adds one, right-click takes it away.
class FalloffCurve : public QWidget {
public:
    explicit FalloffCurve(QWidget* parent) : QWidget(parent) {
        setMinimumHeight(90);
        setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
    }
    void reload() {
        if (grabbed_ >= 0) return;
        keys_ = ask("softSelectCurve").toObject()["keys"].toArray();
        update();
    }
protected:
    QPointF to_px(double pos, double value) const { return {6 + pos * (width() - 12), height() - 6 - value * (height() - 12)}; }
    int key_at(const QPointF& p) const {
        for (int i = 0; i < keys_.size(); ++i) {
            QJsonArray k = keys_[i].toArray();
            if (QLineF(p, to_px(k[0].toDouble(), k[1].toDouble())).length() < 7) return i;
        }
        return -1;
    }
    void send() { exec_quiet("softSelectCurve", QJsonObject{{"keys", keys_}}); }
    void paintEvent(QPaintEvent*) override {
        QPainter p(this);
        p.setRenderHint(QPainter::Antialiasing);
        p.fillRect(rect(), QColor(42, 42, 42));
        p.setPen(QColor(70, 70, 70));
        p.drawRect(rect().adjusted(0, 0, -1, -1));
        QPainterPath path;
        for (int i = 0; i < keys_.size(); ++i) {
            QJsonArray k = keys_[i].toArray();
            double x0 = k[0].toDouble(), y0 = k[1].toDouble();
            if (i == 0) {
                path.moveTo(to_px(x0, y0));
                continue;
            }
            // Eased between keys, as the falloff is; linear and flat keys as they say.
            QJsonArray prev = keys_[i - 1].toArray();
            double xp = prev[0].toDouble(), yp = prev[1].toDouble();
            QString how = prev[2].toString();
            for (int s = 1; s <= 16; ++s) {
                double t = s / 16.0;
                double e = how == "linear" ? t : how == "flat" ? (s == 16 ? 1.0 : 0.0) : t * t * (3 - 2 * t);
                path.lineTo(to_px(xp + (x0 - xp) * t, yp + (y0 - yp) * e));
            }
        }
        p.setPen(QPen(QColor(230, 200, 90), 1.5));
        p.drawPath(path);
        p.setBrush(QColor(240, 240, 240));
        p.setPen(Qt::NoPen);
        for (const QJsonValue& v : keys_) {
            QJsonArray k = v.toArray();
            p.drawEllipse(to_px(k[0].toDouble(), k[1].toDouble()), 3.5, 3.5);
        }
    }
    void mousePressEvent(QMouseEvent* e) override {
        int i = key_at(e->position());
        if (e->button() == Qt::RightButton) {
            if (i > 0 && i < keys_.size() - 1) {
                keys_.removeAt(i);
                send();
                update();
            }
            return;
        }
        grabbed_ = i;
    }
    void mouseDoubleClickEvent(QMouseEvent* e) override {
        if (key_at(e->position()) >= 0) return;
        double pos = std::clamp((e->position().x() - 6) / std::max(1, width() - 12), 0.0, 1.0);
        double val = std::clamp((height() - 6 - e->position().y()) / std::max(1, height() - 12), 0.0, 1.0);
        int at = 0;
        while (at < keys_.size() && keys_[at].toArray()[0].toDouble() < pos) ++at;
        keys_.insert(at, QJsonArray{pos, val, "spline"});
        send();
        update();
    }
    void mouseMoveEvent(QMouseEvent* e) override {
        if (grabbed_ < 0) return;
        QJsonArray k = keys_[grabbed_].toArray();
        double pos = std::clamp((e->position().x() - 6) / std::max(1, width() - 12), 0.0, 1.0);
        // The end keys stay at the ends.
        if (grabbed_ == 0) pos = 0;
        else if (grabbed_ == keys_.size() - 1) pos = 1;
        else pos = std::clamp(pos, keys_[grabbed_ - 1].toArray()[0].toDouble() + 0.001, keys_[grabbed_ + 1].toArray()[0].toDouble() - 0.001);
        k[0] = pos;
        k[1] = std::clamp((height() - 6 - e->position().y()) / std::max(1, height() - 12), 0.0, 1.0);
        keys_[grabbed_] = k;
        send();
        update();
    }
    void mouseReleaseEvent(QMouseEvent*) override { grabbed_ = -1; }
private:
    QJsonArray keys_;
    int grabbed_ = -1;
};

// Orientation labels to each tool's mode numbers, as the command reference has them.
struct Orient { const char* label; int mode; };
const QList<Orient> kMoveModes = {{"Object", 0}, {"World", 2}, {"Component", 10}, {"Parent", 1}, {"Normal", 3}, {"Along Rotation Axis", 4}, {"Along Live Object Axis", 5}, {"Custom", 6}};
const QList<Orient> kRotateModes = {{"Object", 0}, {"World", 1}, {"Gimbal", 2}, {"Component", 10}, {"Custom", 3}};

} // namespace

QString ToolSettings::ctx_kind() const {
    if (tool_ == "rotate") return "manipRotateContext";
    if (tool_ == "scale") return "manipScaleContext";
    return "manipMoveContext";
}

QString ToolSettings::ctx_name() const {
    if (tool_ == "rotate") return "Rotate";
    if (tool_ == "scale") return "Scale";
    return "Move";
}

void ToolSettings::ctx_set(const QString& flag, const QJsonValue& v) {
    if (loading_) return;
    QSettings().setValue(QString("toolOptions/%1/%2").arg(ctx_name(), flag), v.toVariant());
    QJsonObject c{{"edit", true}, {"args", QJsonArray{ctx_name()}}};
    c[flag] = v;
    exec_quiet(ctx_kind(), c);
}

QJsonValue ToolSettings::ctx_get(const QString& flag) const {
    return ask(ctx_kind(), QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}, {"args", QJsonArray{ctx_name()}}});
}

// Options saved last time go back to the contexts.
void ToolSettings::restore_options() {
    const QList<QPair<QString, QString>> tools = {{"Move", "manipMoveContext"}, {"Rotate", "manipRotateContext"}, {"Scale", "manipScaleContext"}};
    QSettings s;
    for (const auto& [name, kind] : tools) {
        s.beginGroup("toolOptions/" + name);
        for (const QString& flag : s.childKeys()) {
            QJsonObject c{{"edit", true}, {"args", QJsonArray{name}}};
            c[flag] = QJsonValue::fromVariant(s.value(flag));
            exec_quiet(kind, c);
        }
        s.endGroup();
    }
}

QWidget* ToolSettings::build_transform_page() {
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* w = new QWidget(scroll);
    scroll->setWidget(w);
    auto* page = new QVBoxLayout(w);
    page->setContentsMargins(0, 0, 0, 0);
    page->setSpacing(2);

    auto check = [this, w](QFormLayout* form, const QString& label, const QString& flag, QList<QPair<QCheckBox*, QString>>& into) {
        auto* b = new QCheckBox(label, w);
        connect(b, &QCheckBox::toggled, this, [this, flag](bool on) { ctx_set(flag, on); });
        form->addRow("", b);
        into.append({b, flag});
        return b;
    };

    QFormLayout* form = nullptr;
    transform_section_ = section(page, "Transform Settings", form);
    axis_ = new QComboBox(w);
    connect(axis_, &QComboBox::activated, this, [this](int) { ctx_set("mode", axis_->currentData().toInt()); });
    form->addRow("Axis Orientation:", axis_);
    custom_row_ = new QWidget(w);
    auto* custom = new QGridLayout(custom_row_);
    custom->setContentsMargins(0, 0, 0, 0);
    int custom_at = 0;
    for (auto [label, kind] : {std::pair{"Set to Point", "point"}, {"Set to Edge", "edge"}, {"Set to Face", "face"}}) {
        auto* b = new QPushButton(label, custom_row_);
        QString k = kind;
        connect(b, &QPushButton::clicked, this, [this, k] {
            QJsonValue e = query("uiCustomAxis", k);
            if (!e.isArray()) {
                log(QString("// Warning: Select a %1 first. //").arg(k), 4);
                return;
            }
            ctx_set("orientAxes", e);
            ctx_set("mode", tool_ == "rotate" ? 3 : 6);
            schedule();
        });
        custom->addWidget(b, custom_at / 2, custom_at % 2);
        ++custom_at;
    }
    auto* reset_axis = new QPushButton("Reset", custom_row_);
    connect(reset_axis, &QPushButton::clicked, this, [this] {
        ctx_set("mode", tool_ == "rotate" ? 0 : 2);
        schedule();
    });
    custom->addWidget(reset_axis, custom_at / 2, custom_at % 2);
    form->addRow("", custom_row_);

    step_mode_ = new QComboBox(w);
    step_mode_->addItems({"Off", "Relative", "Absolute"});
    step_size_ = new QDoubleSpinBox(w);
    step_size_->setRange(0.001, 100000);
    step_size_->setDecimals(3);
    auto apply_step = [this] {
        int m = step_mode_->currentIndex();
        step_size_->setEnabled(m != 0);
        ctx_set("snapRelative", m != 2);
        ctx_set("snap", m != 0);
        ctx_set("snapValue", step_size_->value());
    };
    connect(step_mode_, &QComboBox::activated, this, apply_step);
    connect(step_size_, &QDoubleSpinBox::valueChanged, this, apply_step);
    form->addRow("Step Snap:", step_mode_);
    form->addRow("Step size:", step_size_);

    check(form, "Tweak Mode", "tweakMode", checks_);
    preserve_children_ = check(form, "Preserve Children", "preserveChildPosition", checks_);
    preserve_uvs_ = check(form, "Preserve UVs", "preserveUV", checks_);
    keep_spacing_ = check(form, "Keep Spacing", "snapComponentsRelative", checks_);
    update_triad_ = check(form, "Update Triad", "interactiveUpdate", checks_);
    // What a Shift drag on a handle makes: kept as preferences, the same ones the Ctrl+Shift marking menu ticks.
    auto shift_pref = [this, w, form](const QString& label, const QString& pref) {
        auto* b = new QCheckBox(label, w);
        connect(b, &QCheckBox::toggled, this, [this, pref](bool on) {
            if (!loading_) exec_quiet("uiPref", QJsonObject{{"args", QJsonArray{pref, on}}});
        });
        form->addRow("", b);
        return b;
    };
    shift_extrude_ = shift_pref("Shift Extrude", "shiftExtrude");
    shift_duplicate_ = shift_pref("Shift Duplicate", "shiftDuplicate");
    negative_ = check(form, "Prevent Negative Scale", "preventNegativeScale", checks_);
    constraint_ = new QComboBox(w);
    constraint_->addItem("Off", "none");
    constraint_->addItem("Edge", "edge");
    constraint_->addItem("Surface", "surface");
    connect(constraint_, &QComboBox::activated, this, [this](int) { exec_quiet("xformConstraint", QJsonObject{{"type", constraint_->currentData().toString()}}); });
    form->addRow("Constraint:", constraint_);

    // Symmetry: one setting for all the tools, the same one the status line sets.
    section(page, "Symmetry Settings", form);
    sym_mode_ = new QComboBox(w);
    sym_mode_->addItems({"Off", "Object", "World", "Topology"});
    sym_axis_ = new QComboBox(w);
    sym_axis_->addItems({"x", "y", "z"});
    sym_tol_ = new QDoubleSpinBox(w);
    sym_tol_->setRange(0, 100);
    sym_tol_->setDecimals(4);
    sym_tol_->setSingleStep(0.001);
    sym_seam_ = new QCheckBox("Preserve Seam", w);
    auto apply_sym = [this] {
        if (loading_) return;
        int m = sym_mode_->currentIndex();
        QJsonObject c{{"edit", true}, {"symmetry", m != 0}, {"tolerance", sym_tol_->value()}, {"preserveSeam", sym_seam_->isChecked() ? 1 : 0}};
        if (m == 3) {
            c["topoSymmetry"] = true;
        } else if (m != 0) {
            c["about"] = m == 2 ? "world" : "object";
            c["axis"] = sym_axis_->currentText();
        }
        exec("symmetricModelling", c);
    };
    connect(sym_mode_, &QComboBox::activated, this, apply_sym);
    connect(sym_axis_, &QComboBox::activated, this, apply_sym);
    connect(sym_tol_, &QDoubleSpinBox::editingFinished, this, apply_sym);
    connect(sym_seam_, &QCheckBox::toggled, this, apply_sym);
    form->addRow("Symmetry:", sym_mode_);
    form->addRow("Axis:", sym_axis_);
    form->addRow("Tolerance:", sym_tol_);
    form->addRow("", sym_seam_);

    section(page, "Soft Selection", form);
    soft_ = new QCheckBox("Soft Select", w);
    connect(soft_, &QCheckBox::toggled, this, [this](bool on) {
        if (!loading_) exec("softSelect", QJsonObject{{"softSelectEnabled", on ? 1 : 0}});
    });
    form->addRow("", soft_);
    soft_mode_ = new QComboBox(w);
    soft_mode_->addItems({"Volume", "Surface", "Global", "Object"});
    connect(soft_mode_, &QComboBox::currentIndexChanged, this, [this](int i) {
        if (!loading_) exec("softSelect", QJsonObject{{"softSelectFalloff", i}});
    });
    form->addRow("Falloff mode:", soft_mode_);
    soft_radius_ = slider_row(form, "Falloff radius:", 0.0, 20.0, 0.1, [](double x) { exec_quiet("softSelect", QJsonObject{{"softSelectDistance", x}}); });
    auto* curve = new FalloffCurve(w);
    falloff_curve_ = curve;
    form->addRow("Falloff curve:", curve);
    auto* presets = new QComboBox(w);
    presets->addItems({"Smooth", "Linear", "Sharp", "Dome", "Bell", "Spike", "Plateau", "Inverse", "Ring"});
    connect(presets, &QComboBox::activated, this, [presets, curve](int) {
        exec_quiet("softSelectCurve", QJsonObject{{"preset", presets->currentText()}});
        curve->reload();
    });
    form->addRow("Curve presets:", presets);

    section(page, "Pivot", form);
    auto* edit_pivot = new QPushButton("Edit Pivot", w);
    edit_pivot->setCheckable(true);
    connect(edit_pivot, &QPushButton::toggled, this, [this](bool on) { emit pivotMode(on); });
    form->addRow("", edit_pivot);
    auto* row = new QWidget(w);
    auto* buttons = new QHBoxLayout(row);
    buttons->setContentsMargins(0, 0, 0, 0);
    auto* reset = new QPushButton("Reset", row);
    connect(reset, &QPushButton::clicked, this, [] { exec("resetPivot"); });
    auto* bake = new QPushButton("Bake Pivot", row);
    connect(bake, &QPushButton::clicked, this, [] { exec("bakePivot"); });
    buttons->addWidget(reset);
    buttons->addWidget(bake);
    form->addRow("", row);
    auto* pin = new QCheckBox("Pin Component Pivot", w);
    connect(pin, &QCheckBox::toggled, this, [this](bool on) {
        if (!loading_) exec_quiet("manipPivot", QJsonObject{{"pinPivot", on}});
    });
    form->addRow("", pin);
    check(form, "Snap Pivot Position", "snapPivotPos", checks_);
    check(form, "Snap Pivot Orientation", "snapPivotOri", checks_);
    use_manip_pivot_ = check(form, "Use Manipulator Pivot", "useManipPivot", checks_);

    page->addStretch(1);
    restore_options();
    return scroll;
}

void ToolSettings::refresh_transform() {
    loading_ = true;
    bool xf = tool_ == "move" || tool_ == "rotate" || tool_ == "scale";
    transform_section_->setVisible(xf);
    if (xf) {
        const QList<Orient>& modes = tool_ == "rotate" ? kRotateModes : kMoveModes;
        if (axis_tool_ != tool_) {
            axis_tool_ = tool_;
            axis_->clear();
            for (const Orient& o : modes) axis_->addItem(o.label, o.mode);
            preserve_uvs_->setVisible(true);
            keep_spacing_->setVisible(tool_ == "move");
            update_triad_->setVisible(tool_ == "move");
            negative_->setVisible(tool_ == "scale");
            use_manip_pivot_->setVisible(tool_ != "move");
        }
        int at = axis_->findData(ctx_get("mode").toInt());
        if (at >= 0) axis_->setCurrentIndex(at);
        bool snap = ctx_get("snap").toBool();
        bool rel = ctx_get("snapRelative").isNull() || ctx_get("snapRelative").toBool();
        step_mode_->setCurrentIndex(!snap ? 0 : rel ? 1 : 2);
        step_size_->setEnabled(snap);
        if (!step_size_->hasFocus()) step_size_->setValue(ctx_get("snapValue").toDouble(1.0));
        for (const auto& [box, flag] : checks_) {
            QJsonValue v = ctx_get(flag);
            // Prevent Negative Scale starts on.
            box->setChecked(v.isNull() ? flag == "preventNegativeScale" : (v.isBool() ? v.toBool() : v.toInt() != 0));
        }
        for (auto [box, pref] : {std::pair{shift_extrude_, "shiftExtrude"}, {shift_duplicate_, "shiftDuplicate"}}) {
            const QJsonValue v = ask("uiPref", QJsonObject{{"args", QJsonArray{pref}}});
            box->setChecked(v.isNull() || v.isUndefined() ? true : (v.isBool() ? v.toBool() : v.isString() ? v.toString() != "false" && v.toString() != "0" : v.toDouble() != 0));
        }
    }
    QJsonValue on = ask("softSelect", QJsonObject{{"query", true}, {"softSelectEnabled", true}});
    if (!on.isUndefined()) soft_->setChecked(on.toInt(on.toBool() ? 1 : 0) != 0);
    QJsonValue dist = ask("softSelect", QJsonObject{{"query", true}, {"queried", QJsonArray{"softSelectDistance"}}});
    if (dist.isDouble() && !soft_radius_->hasFocus()) soft_radius_->setValue(dist.toDouble());
    QJsonValue mode = ask("softSelect", QJsonObject{{"query", true}, {"queried", QJsonArray{"softSelectFalloff"}}});
    if (mode.isDouble()) soft_mode_->setCurrentIndex(mode.toInt());
    static_cast<FalloffCurve*>(falloff_curve_)->reload();
    loading_ = false;
}

} // namespace az
