#include "toolsettings.h"
#include <QSettings>
#include "host.h"

#include <QButtonGroup>
#include <QCheckBox>
#include <QComboBox>
#include <QDoubleSpinBox>
#include <QFormLayout>
#include <QLabel>
#include <QListWidget>
#include <QTreeWidget>
#include <QTreeWidgetItemIterator>
#include <QHeaderView>
#include <QMouseEvent>
#include <QLineEdit>
#include <QToolButton>
#include <QStandardItemModel>
#include <QSignalBlocker>
#include <QSet>
#include <algorithm>
#include "icons.h"
#include <QColorDialog>
#include <QSpinBox>
#include <QPushButton>
#include <QMessageBox>
#include <QRadioButton>
#include <QScrollArea>
#include <QSlider>
#include <QStackedLayout>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QVBoxLayout>
#include <QApplication>
#include <QDialog>
#include <QFrame>
#include <QMenu>
#include <QMenuBar>
#include <QPointer>
#include <QTimer>

namespace az {

namespace {
// A titled part of the page that folds away.
void fold(QVBoxLayout* page, const QString& title, QFormLayout*& form) {
    auto* head = new QToolButton();
    head->setObjectName("foldHead");
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
}

QJsonValue paint_query(const char* flag) {
    return ask("artAttrSkinPaintCtx", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}});
}
} // namespace

ToolSettings::ToolSettings(QWidget* parent) : Panel(parent) {
    auto* root = new QVBoxLayout(this);
    root->setContentsMargins(6, 6, 6, 6);
    title_ = new QLabel(this);
    title_->setObjectName("toolTitle");
    QFont f = title_->font();
    f.setBold(true);
    title_->setFont(f);
    root->addWidget(title_);
    auto* holder = new QWidget(this);
    stack_ = new QStackedLayout(holder);
    stack_->setContentsMargins(0, 0, 0, 0);
    transform_page_ = build_transform_page();
    paint_page_ = build_paint_page();
    joint_page_ = build_joint_page();
    curve_page_ = build_curve_page();
    stack_->addWidget(curve_page_);
    sculpt_page_ = build_sculpt_page();
    stack_->addWidget(sculpt_page_);
    groom_page_ = build_groom_page();
    stack_->addWidget(groom_page_);
    stack_->addWidget(transform_page_);
    stack_->addWidget(paint_page_);
    stack_->addWidget(joint_page_);
    ik_page_ = build_ik_page(false);
    stack_->addWidget(ik_page_);
    ik_spline_page_ = build_ik_page(true);
    stack_->addWidget(ik_spline_page_);
    attr_page_ = build_attr_page();
    stack_->addWidget(attr_page_);
    member_page_ = build_member_page();
    stack_->addWidget(member_page_);
    root->addWidget(holder, 1);
    set_tool("select");
}

QDoubleSpinBox* ToolSettings::slider_row(QFormLayout* form, const QString& label, double lo, double hi, double step, std::function<void(double)> on) {
    auto* row = new QWidget(this);
    auto* h = new QHBoxLayout(row);
    h->setContentsMargins(0, 0, 0, 0);
    auto* spin = new QDoubleSpinBox(row);
    spin->setRange(lo, hi);
    spin->setSingleStep(step);
    spin->setDecimals(4);
    spin->setFixedWidth(84);
    auto* slider = new QSlider(Qt::Horizontal, row);
    slider->setRange(0, 1000);
    h->addWidget(spin);
    h->addWidget(slider, 1);
    // The slider covers the spin box's range, which Min/Max value can change.
    connect(slider, &QSlider::valueChanged, this, [=](int v) {
        if (loading_) return;
        const double lo = spin->minimum(), hi = spin->maximum();
        double x = lo + (hi - lo) * v / 1000.0;
        spin->blockSignals(true);
        spin->setValue(x);
        spin->blockSignals(false);
        on(x);
    });
    connect(spin, &QDoubleSpinBox::valueChanged, this, [=](double x) {
        const double lo = spin->minimum(), hi = spin->maximum();
        slider->blockSignals(true);
        slider->setValue(hi > lo ? int((x - lo) / (hi - lo) * 1000.0) : 0);
        slider->blockSignals(false);
        if (!loading_) on(x);
    });
    form->addRow(label, row);
    return spin;
}

QWidget* ToolSettings::build_sculpt_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    auto* form = new QFormLayout();
    auto set = [this](const char* flag, const QJsonValue& val) {
        if (loading_) return;
        QJsonObject c{{"edit", true}};
        c[flag] = val;
        exec_quiet("sculptMeshCacheCtx", c);
        Host::get()->poke();
    };
    sculpt_tool_ = new QComboBox(w);
    sculpt_tool_->addItems({"sculpt", "smooth", "relax", "grab", "pinch", "flatten", "foamy", "spray", "repeat", "imprint", "wax", "scrape", "fill", "knife", "smear", "bulge", "amplify", "freeze"});
    connect(sculpt_tool_, &QComboBox::currentTextChanged, this, [set](const QString& t) { set("sculptTool", t); });
    form->addRow("Brush:", sculpt_tool_);
    sculpt_size_ = slider_row(form, "Size:", 0.001, 20.0, 0.05, [set](double x) { set("size", x); });
    sculpt_strength_ = slider_row(form, "Strength:", 0.0, 1.0, 0.05, [set](double x) { set("strength", x); });
    sculpt_invert_ = new QCheckBox("Invert", w);
    connect(sculpt_invert_, &QCheckBox::toggled, this, [set](bool on) { set("invert", on); });
    form->addRow("", sculpt_invert_);
    sculpt_sym_ = new QCheckBox("Symmetry across X", w);
    connect(sculpt_sym_, &QCheckBox::toggled, this, [set](bool on) { set("symmetry", on); });
    form->addRow("", sculpt_sym_);
    auto* clear = new QPushButton("Unfreeze All", w);
    connect(clear, &QPushButton::clicked, this, [] { exec_quiet("sculptMeshCacheCtx", QJsonObject{{"edit", true}, {"clearFreeze", true}}); });
    form->addRow("", clear);
    v->addLayout(form);
    v->addStretch(1);
    return w;
}

// The Interactive Groom tools' common settings, as the reference's Tool Settings show them.
QWidget* ToolSettings::build_groom_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    auto* form = new QFormLayout();
    auto set = [this](const char* flag, const QJsonValue& val) {
        if (loading_) return;
        QJsonObject c;
        c[flag] = val;
        exec_quiet("xjenGroomCtx", c);
        Host::get()->poke();
    };
    groom_tool_ = new QComboBox(w);
    for (const char* t : {"Density", "Place", "Length", "Cut", "Width", "Twist", "Comb", "Grab", "Smooth", "Noise", "Clump", "Part", "Freeze", "Select"})
        groom_tool_->addItem(t, QString(t).toLower());
    connect(groom_tool_, &QComboBox::currentIndexChanged, this, [this, set](int) { set("tool", groom_tool_->currentData().toString()); });
    form->addRow("Tool:", groom_tool_);
    groom_size_ = slider_row(form, "Size:", 1.0, 500.0, 1.0, [set](double x) { set("size", x); });
    groom_strength_ = slider_row(form, "Strength:", 0.0, 1.0, 0.05, [set](double x) { set("strength", x); });
    groom_collide_ = new QCheckBox("Collide with Meshes", w);
    connect(groom_collide_, &QCheckBox::toggled, this, [set](bool on) { set("collide", on); });
    form->addRow("", groom_collide_);
    groom_distance_ = slider_row(form, "Collision Distance:", 0.0, 10.0, 0.01, [set](double x) { set("collisionDistance", x); });
    groom_invert_frozen_ = new QCheckBox("Invert Frozen Effect", w);
    connect(groom_invert_frozen_, &QCheckBox::toggled, this, [set](bool on) { set("invertFrozen", on); });
    form->addRow("", groom_invert_frozen_);
    groom_alt_ = new QCheckBox("Alternate Mode (N)", w);
    connect(groom_alt_, &QCheckBox::toggled, this, [set](bool on) { set("altMode", on); });
    form->addRow("", groom_alt_);
    v->addLayout(form);
    auto* keys = new QLabel("B drag: size. Ctrl: invert the tool. Shift: its other action. N: alternate mode. Ctrl+Shift+I: invert frozen. Ctrl+1 to Ctrl+0: the first ten tools.", w);
    keys->setWordWrap(true);
    v->addWidget(keys);
    v->addStretch(1);
    return w;
}

void ToolSettings::refresh_groom() {
    loading_ = true;
    auto q = [](const char* flag) { return ask("xjenGroomCtx", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}); };
    int i = groom_tool_->findData(q("tool").toString());
    if (i >= 0) groom_tool_->setCurrentIndex(i);
    if (!groom_size_->hasFocus()) groom_size_->setValue(q("size").toDouble());
    if (!groom_strength_->hasFocus()) groom_strength_->setValue(q("strength").toDouble());
    groom_collide_->setChecked(q("collide").toBool());
    if (!groom_distance_->hasFocus()) groom_distance_->setValue(q("collisionDistance").toDouble());
    groom_invert_frozen_->setChecked(q("invertFrozen").toBool());
    groom_alt_->setChecked(q("altMode").toBool());
    loading_ = false;
}

void ToolSettings::refresh_sculpt() {
    loading_ = true;
    auto q = [](const char* flag) { return ask("sculptMeshCacheCtx", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}); };
    sculpt_tool_->setCurrentText(q("sculptTool").toString());
    if (!sculpt_size_->hasFocus()) sculpt_size_->setValue(q("size").toDouble());
    if (!sculpt_strength_->hasFocus()) sculpt_strength_->setValue(q("strength").toDouble());
    sculpt_invert_->setChecked(q("invert").toBool());
    sculpt_sym_->setChecked(q("symmetry").toBool());
    loading_ = false;
}

QWidget* ToolSettings::build_curve_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    auto* form = new QFormLayout();
    auto* degree = new QComboBox(w);
    for (auto [label, d] : {std::pair{"1 Linear", 1}, {"2", 2}, {"3 Cubic", 3}, {"5", 5}, {"7", 7}}) degree->addItem(label, d);
    degree->setCurrentIndex(2);
    connect(degree, &QComboBox::currentIndexChanged, this, [this, degree] { emit curveDegree(degree->currentData().toInt()); });
    form->addRow("Curve degree:", degree);
    v->addLayout(form);
    v->addWidget(new QLabel("Click to place points. Backspace takes the last back.\nEnter makes the curve.", w));
    v->addStretch(1);
    return w;
}

void joint_orient_rows(QFormLayout* form, QWidget* parent, const QString& order, const QString& sao, std::function<void(const QString&, const QString&)> changed) {
    auto radios = [parent](QStringList labels, QButtonGroup*& group) {
        auto* row = new QWidget(parent);
        auto* h = new QHBoxLayout(row);
        h->setContentsMargins(0, 0, 0, 0);
        group = new QButtonGroup(row);
        for (int k = 0; k < labels.size(); ++k) {
            auto* r = new QRadioButton(labels[k], row);
            group->addButton(r, k);
            h->addWidget(r);
        }
        return std::pair{row, h};
    };
    auto* world = new QCheckBox(parent);
    QButtonGroup *primary, *secondary, *axis;
    const auto prow_ph = radios({"X", "Y", "Z"}, primary);
    const auto prow = prow_ph.first;
    const auto ph = prow_ph.second;
    ph->addStretch(1);
    const auto srow_sh = radios({"X", "Y", "Z", "None"}, secondary);
    const auto srow = srow_sh.first;
    const auto sh = srow_sh.second;
    sh->addStretch(1);
    const auto arow_ah = radios({"X", "Y", "Z"}, axis);
    const auto arow = arow_ah.first;
    const auto ah = arow_ah.second;
    auto* sign = new QComboBox(arow);
    sign->addItems({"+", "-"});
    ah->addWidget(sign);
    ah->addStretch(1);
    form->addRow("Orient Joint to World:", world);
    form->addRow("Primary Axis:", prow);
    form->addRow("Secondary Axis:", srow);
    form->addRow("Secondary Axis World Orientation:", arow);

    const QString xyz = "xyz";
    const bool to_world = order == "none";
    const QString o = to_world || order.size() != 3 ? QString("xyz") : order;
    primary->button(std::max<qsizetype>(0, xyz.indexOf(o[0])))->setChecked(true);
    if (sao == "none" && !to_world) secondary->button(3)->setChecked(true);
    else secondary->button(std::max<qsizetype>(0, xyz.indexOf(o[1])))->setChecked(true);
    const QString a = sao.size() >= 3 && sao != "none" ? sao : QString("yup");
    axis->button(std::max<qsizetype>(0, xyz.indexOf(a[0])))->setChecked(true);
    sign->setCurrentIndex(a.endsWith("down") ? 1 : 0);
    world->setChecked(to_world);

    auto emit_now = [=] {
        // With the world on, the rest has no say, as in the reference.
        for (QWidget* w : {prow, srow, arow}) w->setEnabled(!world->isChecked());
        const int p = primary->checkedId();
        const int s2 = secondary->checkedId();
        QString ord;
        if (s2 == 3 || s2 < 0) ord = xyz.mid(p, 1) + xyz.mid((p + 1) % 3, 1) + xyz.mid((p + 2) % 3, 1);
        else ord = xyz.mid(p, 1) + xyz.mid(s2, 1) + xyz.mid(3 - p - s2, 1);
        QString up = s2 == 3 ? QString("none") : xyz.mid(axis->checkedId(), 1) + (sign->currentIndex() == 1 ? "down" : "up");
        changed(world->isChecked() ? QString("none") : ord, up);
    };
    // The primary and secondary axes can't be the same: choosing one already in use moves the other along.
    QObject::connect(primary, &QButtonGroup::idClicked, parent, [=](int id) {
        if (secondary->checkedId() == id) secondary->button((id + 1) % 3)->setChecked(true);
        emit_now();
    });
    QObject::connect(secondary, &QButtonGroup::idClicked, parent, [=](int id) {
        if (primary->checkedId() == id) primary->button((id + 1) % 3)->setChecked(true);
        emit_now();
    });
    QObject::connect(axis, &QButtonGroup::idClicked, parent, [=](int) { emit_now(); });
    QObject::connect(sign, &QComboBox::currentIndexChanged, parent, [=](int) { emit_now(); });
    QObject::connect(world, &QCheckBox::toggled, parent, [=](bool) { emit_now(); });
    for (QWidget* w : {prow, srow, arow}) w->setEnabled(!to_world);
}

namespace {
// The Orient Joint options box's settings, kept between sessions.
QJsonObject orient_settings() {
    QSettings st;
    st.beginGroup("options/orientJointMenu");
    return QJsonObject{{"order", st.value("order", "xyz").toString()}, {"sao", st.value("sao", "yup").toString()}, {"children", st.value("children", true).toBool()}, {"zso", st.value("zso", true).toBool()}};
}

void store_orient_settings(const QJsonObject& o) {
    QSettings st;
    st.beginGroup("options/orientJointMenu");
    for (auto it = o.begin(); it != o.end(); ++it) st.setValue(it.key(), it.value().toVariant());
}

void orient_with(const QJsonObject& o) {
    QJsonArray args{o["order"].toString()};
    for (const QJsonValue& v : state()["selection"].toArray()) args.append(v);
    exec("orientJoint", QJsonObject{{"args", args}, {"secondaryAxisOrient", o["sao"].toString()}, {"children", o["children"].toBool()}, {"zeroScaleOrient", o["zso"].toBool()}});
}
} // namespace

void orient_selected_joints() { orient_with(orient_settings()); }

void open_orient_joint_options() {
    static QPointer<QDialog> open;
    if (open) {
        open->raise();
        open->activateWindow();
        return;
    }
    // The main window owns it, so it stays above it, also when Reset Settings opens it again (#139).
    auto* d = new QDialog(main_window_widget());
    open = d;
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("Orient Joint Options");
    auto* lay = new QVBoxLayout(d);
    auto* bar = new QMenuBar(d);
    QMenu* edit = bar->addMenu("Edit");
    QMenu* help = bar->addMenu("Help");
    help->addAction("Help on Orient Joint...")->setEnabled(false);
    lay->setMenuBar(bar);
    auto* settings = new QJsonObject(orient_settings());
    QObject::connect(d, &QObject::destroyed, [settings] { delete settings; });
    auto* body = new QWidget(d);
    auto* page = new QVBoxLayout(body);
    page->setContentsMargins(0, 0, 0, 0);
    QFormLayout* form = nullptr;
    fold(page, "Orientation Settings", form);
    form->setLabelAlignment(Qt::AlignRight);
    joint_orient_rows(form, body, (*settings)["order"].toString(), (*settings)["sao"].toString(), [settings](const QString& order, const QString& sao) {
        (*settings)["order"] = order;
        (*settings)["sao"] = sao;
    });
    auto* auto_sec = new QCheckBox("Auto orient secondary axis", body);
    auto_sec->setEnabled(false);
    auto_sec->setToolTip("Not in Aztek yet.");
    form->addRow("", auto_sec);
    auto* line = new QFrame(body);
    line->setFrameShape(QFrame::HLine);
    form->addRow(line);
    auto* children = new QCheckBox("Orient children of selected joints", body);
    children->setChecked((*settings)["children"].toBool());
    auto* zso = new QCheckBox("Reorient the local scale axes", body);
    zso->setChecked((*settings)["zso"].toBool());
    form->addRow("", children);
    form->addRow("", zso);
    QObject::connect(children, &QCheckBox::toggled, d, [settings](bool on) { (*settings)["children"] = on; });
    QObject::connect(zso, &QCheckBox::toggled, d, [settings](bool on) { (*settings)["zso"] = on; });
    QFormLayout* axes = nullptr;
    fold(page, "Local Rotation Axes", axes);
    auto* buttons_row = new QWidget(body);
    auto* bh = new QHBoxLayout(buttons_row);
    bh->setContentsMargins(0, 0, 0, 0);
    auto* toggle = new QPushButton("Toggle", buttons_row);
    toggle->setToolTip("Shows or hides the local rotation axes of the selected joints.");
    auto* modify = new QPushButton("Modify", buttons_row);
    auto* freeze = new QPushButton("Freeze", buttons_row);
    for (QPushButton* b : {modify, freeze}) {
        b->setEnabled(false);
        b->setToolTip("Not in Aztek yet.");
    }
    bh->addStretch(1);
    bh->addWidget(toggle);
    bh->addWidget(modify);
    bh->addWidget(freeze);
    bh->addStretch(1);
    axes->addRow(buttons_row);
    QObject::connect(toggle, &QPushButton::clicked, d, [] {
        QJsonArray sel = state()["selection"].toArray();
        if (sel.isEmpty()) return;
        const bool on = !ask("getAttr", QJsonObject{{"args", QJsonArray{sel.first().toString() + ".displayLocalAxis"}}}).toBool();
        for (const QJsonValue& v : sel) exec("setAttr", QJsonObject{{"args", QJsonArray{v.toString() + ".displayLocalAxis", on ? 1 : 0}}});
    });
    page->addStretch(1);
    lay->addWidget(body, 1);
    auto* buttons = new QHBoxLayout();
    auto* orient = new QPushButton("Orient", d);
    auto* apply = new QPushButton("Apply", d);
    auto* close = new QPushButton("Close", d);
    buttons->addWidget(orient);
    buttons->addWidget(apply);
    buttons->addWidget(close);
    lay->addLayout(buttons);
    QObject::connect(orient, &QPushButton::clicked, d, [d, settings] {
        store_orient_settings(*settings);
        orient_with(*settings);
        d->close();
    });
    QObject::connect(apply, &QPushButton::clicked, d, [settings] {
        store_orient_settings(*settings);
        orient_with(*settings);
    });
    QObject::connect(close, &QPushButton::clicked, d, &QDialog::close);
    edit->addAction("Save Settings", d, [settings] { store_orient_settings(*settings); });
    edit->addAction("Reset Settings", d, [d] {
        QSettings().remove("options/orientJointMenu");
        d->close();
        QTimer::singleShot(0, [] { open_orient_joint_options(); });
    });
    d->resize(520, d->sizeHint().height());
    d->show();
}

// The Joint Tool's settings as the reference lays them out, kept between sessions and handed to the joint tool's
// context whenever the tool is chosen.
void ToolSettings::push_joint_settings() {
    QSettings st;
    st.beginGroup("tools/joint");
    QJsonObject c{{"edit", true}, {"args", QJsonArray{"jointContext"}}};
    for (const QString& k : st.childKeys()) {
        const QVariant v = st.value(k);
        if (v.typeId() == QMetaType::Bool || v.toString() == "true" || v.toString() == "false") c[k] = v.toBool();
        else c[k] = v.toString();
    }
    exec_quiet("jointCtx", c);
}

QWidget* ToolSettings::build_joint_page() {
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* w = new QWidget(scroll);
    scroll->setWidget(w);
    auto* page = new QVBoxLayout(w);
    page->setContentsMargins(0, 0, 0, 0);
    page->setSpacing(2);
    auto value = [](const char* k, const QVariant& dflt) {
        QSettings st;
        return st.value(QString("tools/joint/") + k, dflt);
    };
    auto set = [this](const QString& k, const QVariant& v) {
        QSettings().setValue("tools/joint/" + k, v);
        QJsonObject c{{"edit", true}, {"args", QJsonArray{"jointContext"}}};
        c[k] = QJsonValue::fromVariant(v);
        exec_quiet("jointCtx", c);
    };
    QFormLayout* form = nullptr;

    fold(page, "Joint Settings", form);
    form->setLabelAlignment(Qt::AlignRight);
    auto* dof = new QWidget(w);
    auto* dh = new QHBoxLayout(dof);
    dh->setContentsMargins(0, 0, 0, 0);
    QList<QCheckBox*> dofs;
    const QString had = value("degreeOfFreedomJ", "xyz").toString();
    for (const char* a : {"X", "Y", "Z"}) {
        auto* cb = new QCheckBox(a, dof);
        cb->setChecked(had.contains(QString(a).toLower()));
        dofs << cb;
        dh->addWidget(cb);
    }
    dh->addStretch(1);
    for (QCheckBox* cb : dofs)
        connect(cb, &QCheckBox::toggled, this, [dofs, set] {
            QString d;
            for (int k = 0; k < 3; ++k)
                if (dofs[k]->isChecked()) d += QString("xyz").mid(k, 1);
            set("degreeOfFreedomJ", d.isEmpty() ? QString("none") : d);
        });
    form->addRow("Degrees of freedom:", dof);
    auto* sym = new QComboBox(w);
    sym->addItems({"Off", "X-axis", "Y-axis", "Z-axis"});
    sym->setCurrentIndex(value("symmetry", false).toBool() ? 1 + QString("xyz").indexOf(value("symmetryAxis", "x").toString()) : 0);
    connect(sym, &QComboBox::currentIndexChanged, this, [set](int i) {
        set("symmetry", i > 0);
        if (i > 0) set("symmetryAxis", QString("xyz").mid(i - 1, 1));
    });
    form->addRow("Symmetry:", sym);
    auto check = [&](const char* flag, const QString& label, bool dflt, bool works) {
        auto* cb = new QCheckBox(label, w);
        cb->setChecked(value(flag, dflt).toBool());
        if (!works) {
            cb->setEnabled(false);
            cb->setToolTip("Not in Aztek yet.");
        }
        connect(cb, &QCheckBox::toggled, this, [set, flag](bool on) { set(flag, on); });
        form->addRow("", cb);
        return cb;
    };
    check("scaleCompensateJ", "Scale compensate", true, true);
    check("jointAutoLimits", "Auto joint limits", false, false);
    QCheckBox* ik = check("createIKHandle", "Create IK handle", false, true);
    check("variableBoneSize", "Variable bone radius", false, false);
    auto* centering = new QCheckBox("Projected centering", w);
    centering->setEnabled(false);
    centering->setToolTip("Not in Aztek yet.");
    form->addRow("", centering);

    fold(page, "Orientation Settings", form);
    form->setLabelAlignment(Qt::AlignRight);
    // The panel is narrow: long labels go above their buttons rather than push them out of sight.
    form->setRowWrapPolicy(QFormLayout::WrapLongRows);
    joint_orient_rows(form, w, value("autoJointOrient", "xyz").toString(), value("secondaryAxisOrient", "yup").toString(), [set](const QString& order, const QString& sao) {
        set("autoJointOrient", order);
        set("secondaryAxisOrient", sao);
    });

    fold(page, "IK Handle Settings", form);
    form->setLabelAlignment(Qt::AlignRight);
    auto* solver = new QComboBox(w);
    solver->addItem("Rotate-Plane Solver", "ikRPsolver");
    solver->addItem("Single-Chain Solver", "ikSCsolver");
    solver->setCurrentIndex(std::max(0, solver->findData(value("solverTypeH", "ikRPsolver").toString())));
    connect(solver, &QComboBox::currentIndexChanged, this, [set, solver](int) { set("solverTypeH", solver->currentData().toString()); });
    form->addRow("Current solver:", solver);
    solver->setEnabled(ik->isChecked());
    connect(ik, &QCheckBox::toggled, solver, &QWidget::setEnabled);

    fold(page, "Bone Radius Settings", form);
    form->setLabelAlignment(Qt::AlignRight);
    for (const char* label : {"Short bone length:", "Short bone radius:", "Long bone length:", "Long bone radius:"}) {
        auto* e = new QLineEdit(w);
        e->setEnabled(false);
        e->setToolTip("Not in Aztek yet.");
        form->addRow(label, e);
    }
    page->addStretch(1);
    push_joint_settings();
    return scroll;
}

// The IK Handle Tool's and IK Spline Handle Tool's settings (#317), laid out as the reference's, kept between sessions
// and handed to the tool's context whenever the tool is chosen.
static QString ik_group(bool spline) { return spline ? "tools/ikSplineHandle" : "tools/ikHandle"; }
static QString ik_ctx(bool spline) { return spline ? "ikSplineHandleContext" : "ikHandleContext"; }
static QString ik_ctx_cmd(bool spline) { return spline ? "ikSplineHandleCtx" : "ikHandleCtx"; }

void ToolSettings::push_ik_settings(bool spline) {
    QSettings st;
    st.beginGroup(ik_group(spline));
    QJsonObject c{{"edit", true}, {"args", QJsonArray{ik_ctx(spline)}}};
    for (const QString& k : st.childKeys()) {
        const QVariant v = st.value(k);
        if (v.typeId() == QMetaType::Bool || v.toString() == "true" || v.toString() == "false") c[k] = v.toBool();
        else if (v.typeId() == QMetaType::Int || v.typeId() == QMetaType::Double) c[k] = v.toDouble();
        else c[k] = v.toString();
    }
    exec_quiet(ik_ctx_cmd(spline), c);
}

QWidget* ToolSettings::build_ik_page(bool spline) {
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* w = new QWidget(scroll);
    scroll->setWidget(w);
    auto* page = new QVBoxLayout(w);
    page->setContentsMargins(0, 0, 0, 0);
    page->setSpacing(2);
    auto value = [spline](const char* k, const QVariant& dflt) { return QSettings().value(ik_group(spline) + "/" + k, dflt); };
    auto set = [spline](const QString& k, const QVariant& v) {
        QSettings().setValue(ik_group(spline) + "/" + k, v);
        QJsonObject c{{"edit", true}, {"args", QJsonArray{ik_ctx(spline)}}};
        c[k] = QJsonValue::fromVariant(v);
        exec_quiet(ik_ctx_cmd(spline), c);
    };
    QFormLayout* form = nullptr;
    auto check = [&](const char* flag, const QString& label, bool dflt) {
        auto* cb = new QCheckBox(label, w);
        cb->setChecked(value(flag, dflt).toBool());
        connect(cb, &QCheckBox::toggled, this, [set, flag](bool on) { set(flag, on); });
        form->addRow("", cb);
        return cb;
    };
    auto number = [&](const char* flag, const QString& label, double dflt, double lo, double hi, int decimals) {
        auto* sb = new QDoubleSpinBox(w);
        sb->setRange(lo, hi);
        sb->setDecimals(decimals);
        sb->setSingleStep(decimals ? 0.1 : 1.0);
        sb->setValue(value(flag, dflt).toDouble());
        connect(sb, &QDoubleSpinBox::valueChanged, this, [set, flag, decimals](double v) { set(flag, decimals ? QVariant(v) : QVariant(int(v))); });
        form->addRow(label, sb);
        return sb;
    };
    if (!spline) {
        fold(page, "IK Handle Settings", form);
        form->setLabelAlignment(Qt::AlignRight);
        auto* solver = new QComboBox(w);
        solver->addItem("Rotate-Plane Solver", "ikRPsolver");
        solver->addItem("Single-Chain Solver", "ikSCsolver");
        solver->setCurrentIndex(std::max(0, solver->findData(value("solverTypeH", "ikRPsolver").toString())));
        connect(solver, &QComboBox::currentIndexChanged, this, [set, solver](int) { set("solverTypeH", solver->currentData().toString()); });
        form->addRow("Current solver:", solver);
        check("autoPriorityH", "Autopriority", false);
        check("forceSolverH", "Solver enable", true);
        check("snapHandleH", "Snap enable", true);
        auto* sticky = new QCheckBox("Sticky", w);
        sticky->setChecked(value("stickyH", "off").toString() == "sticky");
        connect(sticky, &QCheckBox::toggled, this, [set](bool on) { set("stickyH", on ? QString("sticky") : QString("off")); });
        form->addRow("", sticky);
        number("priorityH", "Priority:", 1, 0, 1000, 0);
        number("weightH", "Weight:", 1, 0, 1000, 3);
        number("poWeightH", "POWeight:", 1, 0, 1, 3);
    } else {
        fold(page, "IK Spline Handle Settings", form);
        form->setLabelAlignment(Qt::AlignRight);
        QCheckBox* root = check("rootOnCurve", "Root on curve", true);
        QCheckBox* axis = check("createRootAxis", "Auto create root axis", false);
        check("parentCurve", "Auto parent curve", true);
        QCheckBox* snap = check("snapCurve", "Snap curve to root", true);
        QCheckBox* create = check("createCurve", "Auto create curve", true);
        QCheckBox* simplify = check("simplifyCurve", "Auto simplify curve", true);
        QDoubleSpinBox* spans = number("numSpans", "Number of spans:", 1, 1, 1000, 0);
        check("rootTwistMode", "Root twist mode", false);
        auto* twist = new QComboBox(w);
        twist->addItem("Linear", "linear");
        twist->addItem("Ease In", "easeIn");
        twist->addItem("Ease Out", "easeOut");
        twist->addItem("Ease In Out", "easeInOut");
        twist->setCurrentIndex(std::max(0, twist->findData(value("twistType", "linear").toString())));
        connect(twist, &QComboBox::currentIndexChanged, this, [set, twist](int) { set("twistType", twist->currentData().toString()); });
        form->addRow("Twist type:", twist);
        // What the reference greys: the root axis only with the root off the curve, the snap only for a curve of your
        // own, simplifying and spans only for a curve the tool makes.
        auto grey = [root, axis, snap, create, simplify, spans] {
            axis->setEnabled(!root->isChecked());
            snap->setEnabled(!create->isChecked());
            simplify->setEnabled(create->isChecked());
            spans->setEnabled(create->isChecked() && simplify->isChecked());
        };
        for (QCheckBox* cb : {root, create, simplify}) connect(cb, &QCheckBox::toggled, this, grey);
        grey();
    }
    page->addStretch(1);
    push_ik_settings(spline);
    return scroll;
}

void ToolSettings::paint_set(const QString& flag, const QJsonValue& val) {
    if (loading_) return;
    QJsonObject c{{"edit", true}};
    c[flag] = val;
    exec_quiet("artAttrSkinPaintCtx", c);
    Host::get()->poke();
}

QWidget* ToolSettings::build_paint_page() {
    // As the reference lays it out: Skin Clusters, Influences with the weight buttons, Brush, Visualization, Stroke and
    // Display, each folding away. What isn't here yet shows greyed.
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
    auto* w = new QWidget(scroll);
    scroll->setWidget(w);
    auto* page = new QVBoxLayout(w);
    page->setContentsMargins(0, 0, 0, 0);
    page->setSpacing(2);
    QFormLayout* form = nullptr;

    fold(page, "Skin Clusters", form);
    skins_ = new QListWidget(w);
    skins_->setMaximumHeight(60);
    connect(skins_, &QListWidget::itemClicked, this, [this](QListWidgetItem* it) {
        // The skinned mesh of that skinCluster becomes the one painted.
        QJsonArray geo = ask("skinCluster", QJsonObject{{"args", QJsonArray{it->text()}}, {"query", true}, {"geometry", true}}).toArray();
        if (!geo.isEmpty()) exec("select", QJsonObject{{"args", QJsonArray{geo.at(0)}}});
    });
    form->addRow(skins_);

    fold(page, "Influences", form);
    auto* sorts = new QWidget(w);
    auto* sl = new QGridLayout(sorts);
    sl->setContentsMargins(0, 0, 0, 0);
    sort_ = new QButtonGroup(sorts);
    int k = 0;
    for (const char* label : {"Alphabetically", "By Hierarchy", "Flat"}) {
        auto* r = new QRadioButton(label, sorts);
        r->setChecked(k == 1);
        sort_->addButton(r, k);
        sl->addWidget(r, k / 2, k % 2);
        ++k;
    }
    connect(sort_, &QButtonGroup::idClicked, this, [this](int) {
        last_influences_.clear();
        refresh_paint();
    });
    form->addRow("Sort:", sorts);
    search_ = new QLineEdit(w);
    search_->setPlaceholderText("Search...");
    search_->setClearButtonEnabled(true);
    connect(search_, &QLineEdit::textChanged, this, [this] { filter_influences(); });
    form->addRow(search_);
    influences_ = new QTreeWidget(w);
    // A column of lock buttons at the far left, aligned whatever the depth, then the names in their hierarchy (#105).
    influences_->setColumnCount(2);
    influences_->setTreePosition(1);
    influences_->header()->setStretchLastSection(true);
    influences_->header()->setSectionResizeMode(0, QHeaderView::Fixed);
    influences_->header()->resizeSection(0, 26);
    influences_->setHeaderHidden(true);
    influences_->setMinimumHeight(160);
    influences_->setSelectionMode(QAbstractItemView::ExtendedSelection);
    influences_->setRootIsDecorated(true);
    influences_->setIconSize(QSize(16, 16));
    // A click on a lock holds or releases that influence, or every selected one when it is among them, and leaves the
    // selection as it is: two influences picked, Invert Selection, a click on a lock, and only the two take paint.
    struct LockClicks : QObject {
        ToolSettings* ts;
        explicit LockClicks(ToolSettings* t, QObject* parent) : QObject(parent), ts(t) {}
        bool eventFilter(QObject* o, QEvent* e) override {
            if (e->type() != QEvent::MouseButtonPress && e->type() != QEvent::MouseButtonDblClick) return false;
            auto* me = static_cast<QMouseEvent*>(e);
            auto* tree = qobject_cast<QTreeWidget*>(o->parent());
            if (!tree || me->button() != Qt::LeftButton) return false;
            const QPoint at = me->position().toPoint();
            QTreeWidgetItem* it = tree->itemAt(at);
            if (!it || tree->columnAt(at.x()) != 0) return false;
            if (e->type() == QEvent::MouseButtonPress) ts->toggle_lock(it);
            return true;
        }
    };
    influences_->viewport()->installEventFilter(new LockClicks(this, influences_));
    connect(influences_, &QTreeWidget::currentItemChanged, this, [this](QTreeWidgetItem* cur, QTreeWidgetItem*) {
        if (cur) exec_quiet("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"influence", cur->data(0, Qt::UserRole).toString()}});
    });
    // The order influences are picked in: the first is the one Move weights takes from.
    connect(influences_, &QTreeWidget::itemSelectionChanged, this, [this] {
        QStringList now;
        for (QTreeWidgetItem* it : influences_->selectedItems()) now << it->data(0, Qt::UserRole).toString();
        QStringList kept;
        for (const QString& n : pick_order_)
            if (now.contains(n)) kept << n;
        for (const QString& n : now)
            if (!kept.contains(n)) kept << n;
        pick_order_ = kept;
    });
    form->addRow(influences_);

    // The buttons under the list.
    auto* buttons = new QWidget(w);
    auto* bl = new QHBoxLayout(buttons);
    bl->setContentsMargins(0, 0, 0, 0);
    bl->setSpacing(2);
    auto button = [&](const QString& ic, const QString& tip, std::function<void()> fn) {
        auto* b = new QToolButton(buttons);
        b->setIcon(icon(ic));
        b->setIconSize(QSize(20, 20));
        b->setAutoRaise(true);
        b->setToolTip(tip);
        connect(b, &QToolButton::clicked, this, fn);
        bl->addWidget(b);
    };
    bl->addStretch(1);
    button("copyWeights", "Copy the weights of the selected vertex", [] { exec("artAttrSkinWeightCopy"); });
    button("pasteWeights", "Paste the copied weights onto the selected vertices", [] { exec("artAttrSkinWeightPaste"); });
    button("hammerWeights", "Weight Hammer: the selected vertices take the weights around them", [] { exec("weightHammerVerts"); });
    button("moveWeights", "Move weights: the first selected influence gives the selected vertices' weight to the other selected influences",
           [this] { move_weights(); });
    button("showInfluences", "Show influences on selected vertices", [this] { show_vertex_influences(); });
    bl->addSpacing(16);
    button("revealSelected", "Reveal Selected", [this] {
        if (QTreeWidgetItem* it = influences_->currentItem()) influences_->scrollToItem(it);
        else if (!influences_->selectedItems().isEmpty()) influences_->scrollToItem(influences_->selectedItems().first());
    });
    button("invertSelection", "Invert Selection", [this] {
        QSignalBlocker block(influences_);
        for (QTreeWidgetItemIterator i(influences_); *i; ++i)
            if (!(*i)->isHidden()) (*i)->setSelected(!(*i)->isSelected());
    });
    bl->addStretch(1);
    form->addRow(buttons);
    auto* holds = new QHBoxLayout();
    for (auto [label, on] : {std::pair{"Hold All", true}, {"Release All", false}}) {
        auto* b = new QPushButton(label, w);
        bool lock = on;
        connect(b, &QPushButton::clicked, this, [this, lock] {
            if (!last_skin_.isEmpty()) exec("skinCluster", QJsonObject{{"args", QJsonArray{last_skin_}}, {"edit", true}, {"lockWeights", lock}});
        });
        holds->addWidget(b);
    }
    form->addRow(holds);

    fold(page, "Brush", form);
    auto radios = [&](const QStringList& labels, QButtonGroup*& group) {
        auto* box = new QWidget(w);
        auto* l = new QGridLayout(box);
        l->setContentsMargins(0, 0, 0, 0);
        group = new QButtonGroup(box);
        for (int i = 0; i < labels.size(); ++i) {
            auto* r = new QRadioButton(labels[i], box);
            r->setChecked(i == 0);
            group->addButton(r, i);
            l->addWidget(r, i / 2, i % 2);
        }
        return box;
    };
    // Mode: -spm 1 paints, 0 selects with a marquee, 2 paint selects; the points are picked as vertices.
    form->addRow("Mode:", radios({"Paint", "Select", "Paint Select"}, skin_mode_));
    QWidget* ps = radios({"Add", "Remove", "Toggle"}, paint_select_);
    form->addRow("Paint Select:", ps);
    connect(skin_mode_, &QButtonGroup::idClicked, this, [this, ps](int id) {
        const int spm[] = {1, 0, 2};
        exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"skinPaintMode", spm[id]}});
        if (id != 0) exec("selectMode", QJsonObject{{"args", QJsonArray{"vertex"}}});
        ps->setEnabled(id == 2);
    });
    ps->setEnabled(false);
    connect(paint_select_, &QButtonGroup::idClicked, this, [](int id) { exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"paintSelectMode", id + 1}}); });
    auto* sel_geo = new QPushButton("Select Geometry", w);
    // The whole mesh again: the components picked in Select mode let go, so the brush paints everywhere.
    connect(sel_geo, &QPushButton::clicked, this, [this] {
        QString mesh;
        if (!last_skin_.isEmpty()) {
            QJsonArray geo = ask("skinCluster", QJsonObject{{"args", QJsonArray{last_skin_}}, {"query", true}, {"geometry", true}}).toArray();
            if (!geo.isEmpty()) mesh = ask("listRelatives", QJsonObject{{"args", QJsonArray{geo.at(0).toString()}}, {"parent", true}, {"fullPath", true}}).toArray().at(0).toString();
        }
        exec("selectMode", QJsonObject{{"args", QJsonArray{"object"}}});
        if (!mesh.isEmpty()) exec("select", QJsonObject{{"args", QJsonArray{mesh}}});
        if (skin_mode_ && skin_mode_->button(0)) {
            skin_mode_->button(0)->setChecked(true);
            exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"skinPaintMode", 1}});
        }
    });
    form->addRow("", sel_geo);
    op_ = new QButtonGroup(w);
    auto* ops = new QWidget(w);
    auto* grid = new QGridLayout(ops);
    grid->setContentsMargins(0, 0, 0, 0);
    int i = 0;
    for (auto [label, name] : {std::pair{"Replace", "absolute"}, {"Add", "additive"}, {"Scale", "scale"}, {"Smooth", "smooth"}}) {
        auto* r = new QRadioButton(label, ops);
        r->setProperty("op", QString(name));
        op_->addButton(r, i);
        grid->addWidget(r, i / 2, i % 2);
        ++i;
    }
    connect(op_, &QButtonGroup::idClicked, this, [this](int id) { paint_set("selectedattroper", op_->button(id)->property("op").toString()); });
    form->addRow("Paint operation:", ops);
    stamp_ = new QComboBox(w);
    stamp_->addItems({"gaussian", "poly", "solid", "square"});
    connect(stamp_, &QComboBox::currentTextChanged, this, [this](const QString& t) { paint_set("stampProfile", t); });
    form->addRow("Profile:", stamp_);
    auto* weight_type = new QComboBox(w);
    weight_type->addItems({"Skin Weight", "DQ Blend Weight"});
    if (auto* m = qobject_cast<QStandardItemModel*>(weight_type->model())) m->item(1)->setEnabled(false);
    form->addRow("Weight Type:", weight_type);
    normalize_ = new QComboBox(w);
    normalize_->addItems({"Off", "Interactive", "Post"});
    // The skinCluster's own setting. Turned to Interactive from Off or Post, the weights can be normalized there and
    // then, as the reference asks.
    connect(normalize_, &QComboBox::activated, this, [this](int n) {
        if (loading_ || last_skin_.isEmpty()) return;
        const int was = ask("getAttr", QJsonObject{{"args", QJsonArray{last_skin_ + ".normalizeWeights"}}}).toInt(1);
        exec("setAttr", QJsonObject{{"args", QJsonArray{last_skin_ + ".normalizeWeights", n}}});
        if (n == 1 && was != 1) {
            const auto answer = QMessageBox::question(this, "Normalize Weights", "Normalize the weights of " + last_skin_.section('|', -1) + " now?", QMessageBox::Yes | QMessageBox::No, QMessageBox::Yes);
            if (answer == QMessageBox::Yes) {
                QJsonArray geo = ask("skinCluster", QJsonObject{{"args", QJsonArray{last_skin_}}, {"query", true}, {"geometry", true}}).toArray();
                if (!geo.isEmpty()) exec("skinPercent", QJsonObject{{"args", QJsonArray{last_skin_, geo.at(0).toString()}}, {"normalize", true}});
            }
        }
    });
    form->addRow("Normalize Weights:", normalize_);
    opacity_ = slider_row(form, "Opacity:", 0.0, 1.0, 0.05, [this](double x) { paint_set("opacity", x); });
    value_ = slider_row(form, "Value:", 0.0, 1.0, 0.05, [this](double x) { paint_set("value", x); });
    // Min/Max value: the range the value slider covers.
    auto* range = new QWidget(w);
    auto* rl = new QHBoxLayout(range);
    rl->setContentsMargins(0, 0, 0, 0);
    auto* lo = new QDoubleSpinBox(range);
    auto* hi = new QDoubleSpinBox(range);
    for (QDoubleSpinBox* b : {lo, hi}) {
        b->setDecimals(4);
        b->setRange(-1000, 1000);
        b->setSingleStep(0.1);
        rl->addWidget(b);
    }
    lo->setValue(0.0);
    hi->setValue(1.0);
    rl->addStretch(1);
    auto set_range = [this, lo, hi] {
        if (hi->value() <= lo->value()) return;
        exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"minvalue", lo->value()}, {"maxvalue", hi->value()}});
        const double v = value_->value();
        value_->setRange(lo->value(), hi->value());
        // The slider follows the new range.
        value_->setValue(v);
        emit value_->valueChanged(value_->value());
    };
    connect(lo, &QDoubleSpinBox::valueChanged, this, set_range);
    connect(hi, &QDoubleSpinBox::valueChanged, this, set_range);
    form->addRow("Min/Max value:", range);
    auto* flood = new QPushButton("Flood", w);
    connect(flood, &QPushButton::clicked, this, [] {
        exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"clear", true}});
    });
    form->addRow("", flood);

    fold(page, "Visualization", form);
    feedback_ = new QCheckBox("Color feedback", w);
    connect(feedback_, &QCheckBox::toggled, this, [this](bool on) { paint_set("colorfeedback", on); });
    form->addRow("", feedback_);
    ramp_ = new QCheckBox("Use Color Ramp", w);
    connect(ramp_, &QCheckBox::toggled, this, [this](bool on) { paint_set("useColorRamp", on); });
    form->addRow("", ramp_);
    // The ramp the weights go through, 0 on the left, 1 on the right.
    auto* swatch = new QLabel(w);
    swatch->setFixedHeight(12);
    swatch->setStyleSheet("background: qlineargradient(x1:0, y1:0, x2:1, y2:0, stop:0 #000000, stop:0.25 #0040ff, stop:0.5 #00d933, stop:0.75 #ffe600, stop:1 #ff1a0d); border: 1px solid #222;");
    form->addRow("Weight Color:", swatch);
    // Numeric: the painted influence's weight beside each point.
    auto* numbers = new QCheckBox("Show Numbers", w);
    connect(numbers, &QCheckBox::toggled, this, [](bool on) { exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"useNumericDisplay", on}}); });
    form->addRow("", numbers);
    auto* decimals = new QSpinBox(w);
    decimals->setRange(0, 6);
    decimals->setValue(2);
    connect(decimals, &QSpinBox::valueChanged, this, [](int n) { exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"numericDisplayPrecision", n}}); });
    form->addRow("Decimal Places:", decimals);
    auto* number_color = new QPushButton(w);
    number_color->setFixedWidth(60);
    number_color->setStyleSheet("background: white;");
    connect(number_color, &QPushButton::clicked, this, [this, number_color] {
        const QColor c = QColorDialog::getColor(Qt::white, this, "Number Color");
        if (!c.isValid()) return;
        number_color->setStyleSheet(QString("background: %1;").arg(c.name()));
        exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"numericDisplayColor", QJsonArray{c.redF(), c.greenF(), c.blueF()}}});
    });
    form->addRow("Number Color:", number_color);

    fold(page, "Stroke", form);
    radius_ = slider_row(form, "Radius(U):", 0.001, 50.0, 0.05, [this](double x) { paint_set("radius", x); });
    // Radius(L) is a tablet's; Reflection is off for this tool, as the reference's (Skin > Mirror Skin Weights).
    for (const char* label : {"Screen projection", "Reflection"}) {
        auto* c = new QCheckBox(label, w);
        c->setEnabled(false);
        form->addRow("", c);
    }

    fold(page, "Stylus Pressure", form);
    auto* stylus = new QCheckBox("Stylus pressure", w);
    stylus->setEnabled(false);
    form->addRow("", stylus);

    fold(page, "Display", form);
    for (const char* label : {"Draw brush", "Show wireframe"}) {
        auto* c = new QCheckBox(label, w);
        c->setChecked(true);
        c->setEnabled(false);
        form->addRow("", c);
    }
    auto* xray = new QCheckBox("X-Ray Joints", w);
    connect(xray, &QCheckBox::toggled, this, [](bool on) {
        for (const QJsonValue& p : ask("getPanel", QJsonObject{{"type", "modelPanel"}}).toArray())
            exec_quiet("modelEditor", QJsonObject{{"args", QJsonArray{p}}, {"edit", true}, {"jointXray", on}});
    });
    form->addRow("", xray);
    page->addStretch(1);
    return scroll;
}

void ToolSettings::move_weights() {
    // As the reference: the first influence picked gives its weight on the selected vertices to the others picked.
    QStringList picked;
    for (const QString& n : pick_order_)
        for (QTreeWidgetItem* it : influences_->selectedItems())
            if (it->data(0, Qt::UserRole).toString() == n) picked << n;
    if (picked.size() < 2 || last_skin_.isEmpty()) {
        log("// Warning: Move weights: select the influence to take weight from, then the ones to give it to. //", 4);
        return;
    }
    QJsonArray tmw;
    for (const QString& n : picked) tmw.append(n);
    QJsonArray call{last_skin_};
    for (const QJsonValue& v : ask("filterExpand", QJsonObject{{"selectionMask", 31}, {"expand", true}}).toArray()) call.append(v);
    if (call.size() < 2) return;
    exec("skinPercent", QJsonObject{{"args", call}, {"transformMoveWeights", tmw}});
}

void ToolSettings::show_vertex_influences() {
    // Selects in the list the influences weighting the selected vertices.
    QJsonArray verts = ask("filterExpand", QJsonObject{{"selectionMask", 31}, {"expand", true}}).toArray();
    if (verts.isEmpty() || last_skin_.isEmpty()) return;
    QJsonArray call{last_skin_};
    for (const QJsonValue& v : verts) call.append(v);
    QSet<QString> on;
    for (const QJsonValue& v : ask("skinPercent", QJsonObject{{"args", call}, {"query", true}, {"transform", true}, {"ignoreBelow", 1e-6}}).toArray())
        on.insert(v.toString());
    QSignalBlocker block(influences_);
    QTreeWidgetItem* first = nullptr;
    for (QTreeWidgetItemIterator i(influences_); *i; ++i) {
        QString n = (*i)->data(0, Qt::UserRole).toString();
        bool hit = on.contains(n) || on.contains(n.section('|', -1));
        (*i)->setSelected(hit);
        if (hit && !first) first = *i;
    }
    if (first) influences_->scrollToItem(first);
}

void ToolSettings::toggle_lock(QTreeWidgetItem* clicked) {
    if (last_skin_.isEmpty()) return;
    const bool lock = !clicked->data(0, Qt::UserRole + 1).toBool();
    QList<QTreeWidgetItem*> items = clicked->isSelected() ? influences_->selectedItems() : QList<QTreeWidgetItem*>{clicked};
    for (QTreeWidgetItem* it : items) {
        exec("skinCluster", QJsonObject{{"args", QJsonArray{last_skin_}}, {"edit", true}, {"influence", it->data(0, Qt::UserRole).toString()}, {"lockWeights", lock}});
        it->setData(0, Qt::UserRole + 1, lock);
        it->setIcon(0, icon(lock ? "influenceLocked" : "influenceUnlocked"));
        if (lock) it->setForeground(1, QBrush(QColor(120, 160, 230)));
        else it->setData(1, Qt::ForegroundRole, QVariant());
    }
}

void ToolSettings::filter_influences() {
    QString t = search_->text().trimmed();
    for (QTreeWidgetItemIterator i(influences_); *i; ++i) {
        bool show = t.isEmpty() || (*i)->text(1).contains(t, Qt::CaseInsensitive);
        // A parent stays to hold a matching child.
        if (!show)
            for (QTreeWidgetItemIterator c(*i); *c; ++c) {
                QTreeWidgetItem* p = *c;
                bool under = false;
                for (QTreeWidgetItem* up = p->parent(); up; up = up->parent()) under |= up == *i;
                if (!under) break;
                if (p->text(1).contains(t, Qt::CaseInsensitive)) {
                    show = true;
                    break;
                }
            }
        (*i)->setHidden(!show);
    }
}

void ToolSettings::attr_set(const QString& flag, const QJsonValue& val) {
    if (loading_) return;
    QJsonObject c{{"edit", true}};
    c[flag] = val;
    exec_quiet("artAttrCtx", c);
    Host::get()->poke();
}

QWidget* ToolSettings::build_attr_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    auto* form = new QFormLayout();
    attr_list_ = new QComboBox(w);
    connect(attr_list_, &QComboBox::currentTextChanged, this, [this](const QString& t) { if (!t.isEmpty()) attr_set("attrSelected", t); });
    form->addRow("Attribute:", attr_list_);
    attr_op_ = new QButtonGroup(w);
    auto* ops = new QWidget(w);
    auto* grid = new QGridLayout(ops);
    grid->setContentsMargins(0, 0, 0, 0);
    int i = 0;
    for (auto [label, name] : {std::pair{"Replace", "absolute"}, {"Add", "additive"}, {"Scale", "scale"}, {"Smooth", "smooth"}}) {
        auto* r = new QRadioButton(label, ops);
        r->setProperty("op", QString(name));
        attr_op_->addButton(r, i);
        grid->addWidget(r, i / 2, i % 2);
        ++i;
    }
    connect(attr_op_, &QButtonGroup::idClicked, this, [this](int id) { attr_set("selectedattroper", attr_op_->button(id)->property("op").toString()); });
    form->addRow("Paint operation:", ops);
    attr_stamp_ = new QComboBox(w);
    attr_stamp_->addItems({"gaussian", "poly", "solid", "square"});
    connect(attr_stamp_, &QComboBox::currentTextChanged, this, [this](const QString& t) { attr_set("stampProfile", t); });
    form->addRow("Profile:", attr_stamp_);
    attr_value_ = slider_row(form, "Value:", 0.0, 1.0, 0.05, [this](double x) { attr_set("value", x); });
    attr_opacity_ = slider_row(form, "Opacity:", 0.0, 1.0, 0.05, [this](double x) { attr_set("opacity", x); });
    attr_radius_ = slider_row(form, "Radius(U):", 0.001, 20.0, 0.05, [this](double x) { attr_set("radius", x); });
    attr_feedback_ = new QCheckBox("Color feedback", w);
    connect(attr_feedback_, &QCheckBox::toggled, this, [this](bool on) { attr_set("colorfeedback", on); });
    form->addRow("", attr_feedback_);
    auto* flood = new QPushButton("Flood", w);
    connect(flood, &QPushButton::clicked, this, [] { exec("artAttrCtx", QJsonObject{{"edit", true}, {"clear", true}}); });
    form->addRow("", flood);
    v->addLayout(form);
    v->addStretch(1);
    return w;
}

void ToolSettings::refresh_attr() {
    loading_ = true;
    auto q = [](const char* flag) { return ask("artAttrCtx", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}); };
    QStringList names;
    for (const QJsonValue& x : q("paintableAttrs").toArray()) names << x.toString();
    QStringList had;
    for (int i = 0; i < attr_list_->count(); ++i) had << attr_list_->itemText(i);
    if (had != names) {
        attr_list_->clear();
        attr_list_->addItems(names);
    }
    QString current = q("attrSelected").toString();
    int at = attr_list_->findText(current);
    if (at >= 0) attr_list_->setCurrentIndex(at);
    QString op = q("selectedattroper").toString();
    for (QAbstractButton* b : attr_op_->buttons())
        if (b->property("op").toString() == op) b->setChecked(true);
    attr_stamp_->setCurrentText(q("stampProfile").toString());
    if (!attr_value_->hasFocus()) attr_value_->setValue(q("value").toDouble());
    if (!attr_opacity_->hasFocus()) attr_opacity_->setValue(q("opacity").toDouble());
    if (!attr_radius_->hasFocus()) attr_radius_->setValue(q("radius").toDouble());
    attr_feedback_->setChecked(q("colorfeedback").toBool());
    loading_ = false;
    // With nothing chosen yet the brush is on the first the mesh offers: say so to the tool.
    if (current.isEmpty() && !names.isEmpty()) attr_set("attrSelected", names.first());
}

QWidget* ToolSettings::build_member_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    auto* form = new QFormLayout();
    member_list_ = new QComboBox(w);
    form->addRow("Deformer:", member_list_);
    v->addLayout(form);
    auto set_of = [this]() -> QString {
        QString d = member_list_->currentText();
        if (d.isEmpty()) return QString();
        QJsonArray sets = ask("listConnections", QJsonObject{{"args", QJsonArray{d}}, {"type", "objectSet"}}).toArray();
        return sets.isEmpty() ? d + "Set" : sets.at(0).toString();
    };
    auto* select = new QPushButton("Select Members", w);
    connect(select, &QPushButton::clicked, this, [set_of] {
        QString s = set_of();
        if (!s.isEmpty()) exec("selectSetMembers", QJsonObject{{"args", QJsonArray{s}}});
    });
    v->addWidget(select);
    auto* add = new QPushButton("Add Selected Points", w);
    connect(add, &QPushButton::clicked, this, [set_of] {
        QString s = set_of();
        if (!s.isEmpty()) exec("sets", QJsonObject{{"addElement", s}});
    });
    v->addWidget(add);
    auto* rm = new QPushButton("Remove Selected Points", w);
    connect(rm, &QPushButton::clicked, this, [set_of] {
        QString s = set_of();
        if (!s.isEmpty()) exec("sets", QJsonObject{{"remove", s}});
    });
    v->addWidget(rm);
    v->addStretch(1);
    return w;
}

void ToolSettings::refresh_member() {
    QStringList names;
    for (const QJsonValue& x : ask("ls", QJsonObject{{"type", "geometryFilter"}}).toArray()) names << x.toString();
    QStringList had;
    for (int i = 0; i < member_list_->count(); ++i) had << member_list_->itemText(i);
    if (had != names) {
        QString keep = member_list_->currentText();
        member_list_->clear();
        member_list_->addItems(names);
        if (names.contains(keep)) member_list_->setCurrentText(keep);
    }
}

void ToolSettings::set_tool(const QString& tool) {
    tool_ = tool;
    if (tool == "paintAttr") {
        title_->setText("Paint Attribute Tool");
        stack_->setCurrentWidget(attr_page_);
    } else if (tool == "editMembership") {
        title_->setText("Edit Membership Tool");
        stack_->setCurrentWidget(member_page_);
    } else if (tool == "paintSkin") {
        title_->setText("Paint Skin Weights Tool");
        stack_->setCurrentWidget(paint_page_);
    } else if (tool == "sculpt") {
        title_->setText("Sculpting Tools");
        stack_->setCurrentWidget(sculpt_page_);
    } else if (tool == "groom") {
        title_->setText("Interactive Groom Tools");
        stack_->setCurrentWidget(groom_page_);
    } else if (tool == "joint") {
        title_->setText("Joint Tool");
        stack_->setCurrentWidget(joint_page_);
        push_joint_settings();
    } else if (tool == "ikHandle" || tool == "ikSplineHandle") {
        const bool spline = tool == "ikSplineHandle";
        title_->setText(spline ? "IK Spline Handle Tool" : "IK Handle Tool");
        stack_->setCurrentWidget(spline ? ik_spline_page_ : ik_page_);
        push_ik_settings(spline);
    } else if (tool == "curveCv" || tool == "curveEp") {
        title_->setText(tool == "curveCv" ? "CV Curve Tool" : "EP Curve Tool");
        stack_->setCurrentWidget(curve_page_);
    } else {
        QString n = tool.left(1).toUpper() + tool.mid(1);
        title_->setText(n + " Tool");
        stack_->setCurrentWidget(transform_page_);
    }
    schedule();
}

void ToolSettings::refresh() {
    if (tool_ == "paintAttr") refresh_attr();
    else if (tool_ == "editMembership") refresh_member();
    else if (tool_ == "paintSkin") refresh_paint();
    else if (tool_ == "sculpt") refresh_sculpt();
    else if (tool_ == "groom") refresh_groom();
    else if (stack_->currentWidget() == transform_page_) refresh_transform();
}

void ToolSettings::refresh_paint() {
    loading_ = true;
    QString op = paint_query("selectedattroper").toString();
    for (QAbstractButton* b : op_->buttons())
        if (b->property("op").toString() == op) b->setChecked(true);
    stamp_->setCurrentText(paint_query("stampProfile").toString());
    if (!value_->hasFocus()) value_->setValue(paint_query("value").toDouble());
    if (!opacity_->hasFocus()) opacity_->setValue(paint_query("opacity").toDouble());
    if (!radius_->hasFocus()) radius_->setValue(paint_query("radius").toDouble());
    feedback_->setChecked(paint_query("colorfeedback").toBool());
    if (ramp_) ramp_->setChecked(paint_query("useColorRamp").toBool());
    // The skinClusters of the scene, the selected mesh's current.
    QStringList skins;
    for (const QJsonValue& x : ask("ls", QJsonObject{{"type", "skinCluster"}}).toArray()) skins << x.toString();
    QJsonArray sel = state()["selection"].toArray();
    QString sk;
    for (const QJsonValue& v : sel) {
        sk = ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{v.toString().section('.', 0, 0)}}}).toString();
        if (!sk.isEmpty()) break;
    }
    QStringList had;
    for (int i = 0; i < skins_->count(); ++i) had << skins_->item(i)->text();
    if (had != skins) {
        skins_->clear();
        skins_->addItems(skins);
    }
    for (int i = 0; i < skins_->count(); ++i) skins_->item(i)->setSelected(skins_->item(i)->text() == sk.section('|', -1) || skins_->item(i)->text() == sk);
    QStringList names;
    if (!sk.isEmpty())
        for (const QJsonValue& v : ask("skinCluster", QJsonObject{{"args", QJsonArray{sk}}, {"query", true}, {"influence", true}}).toArray()) names << v.toString();
    if (!sk.isEmpty() && !normalize_->hasFocus()) normalize_->setCurrentIndex(ask("getAttr", QJsonObject{{"args", QJsonArray{sk + ".normalizeWeights"}}}).toInt(1));
    QString current = paint_query("influence").toString();
    if (sk != last_skin_ || names != last_influences_) {
        last_skin_ = sk;
        last_influences_ = names;
        influences_->clear();
        int sort = sort_->checkedId();
        QStringList order = names;
        if (sort == 0) std::sort(order.begin(), order.end(), [](const QString& a, const QString& b) { return a.section('|', -1).compare(b.section('|', -1), Qt::CaseInsensitive) < 0; });
        QHash<QString, QTreeWidgetItem*> made;
        // By Hierarchy: under the nearest parent that is an influence too.
        auto parent_of = [&](const QString& n) -> QString {
            QString up = n;
            for (int guard = 0; guard < 64; ++guard) {
                QJsonArray p = ask("listRelatives", QJsonObject{{"args", QJsonArray{up}}, {"parent", true}, {"fullPath", true}}).toArray();
                if (p.isEmpty()) return QString();
                up = p.at(0).toString();
                for (const QString& m : names)
                    if (m == up || m == up.section('|', -1) || ("|" + m) == up || up.endsWith("|" + m)) return m;
            }
            return QString();
        };
        std::function<QTreeWidgetItem*(const QString&)> item_for = [&](const QString& n) -> QTreeWidgetItem* {
            if (QTreeWidgetItem* it = made.value(n)) return it;
            QTreeWidgetItem* parent = nullptr;
            if (sort == 1) {
                QString p = parent_of(n);
                if (!p.isEmpty() && p != n) parent = item_for(p);
            }
            auto* it = parent ? new QTreeWidgetItem(parent) : new QTreeWidgetItem(influences_);
            it->setText(1, n.section('|', -1));
            it->setData(0, Qt::UserRole, n);
            it->setToolTip(0, "Lock: a held influence keeps its weights while the brush paints. On a selected influence, every selected one.");
            made.insert(n, it);
            return it;
        };
        for (const QString& n : order) item_for(n);
        influences_->setRootIsDecorated(sort == 1);
        influences_->expandAll();
        filter_influences();
    }
    for (QTreeWidgetItemIterator i(influences_); *i; ++i) {
        QTreeWidgetItem* it = *i;
        QString n = it->data(0, Qt::UserRole).toString();
        bool locked = ask("skinCluster", QJsonObject{{"args", QJsonArray{sk}}, {"query", true}, {"influence", n}, {"lockWeights", true}}).toBool();
        it->setData(0, Qt::UserRole + 1, locked);
        it->setIcon(0, icon(locked ? "influenceLocked" : "influenceUnlocked"));
        // Held ones in blue; the rest take the theme's text colour as it is when drawn, so a theme change leaves them readable.
        if (locked) it->setForeground(1, QBrush(QColor(120, 160, 230)));
        else it->setData(1, Qt::ForegroundRole, QVariant());
        if (n == current || (current.isEmpty() && influences_->currentItem() == nullptr)) {
            if (influences_->currentItem() != it) influences_->setCurrentItem(it);
        }
    }
    loading_ = false;
}

} // namespace az
