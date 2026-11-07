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
#include <QPushButton>
#include <QRadioButton>
#include <QScrollArea>
#include <QSlider>
#include <QStackedLayout>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QVBoxLayout>

namespace az {

namespace {
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
    stack_->addWidget(transform_page_);
    stack_->addWidget(paint_page_);
    stack_->addWidget(joint_page_);
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
    connect(slider, &QSlider::valueChanged, this, [=](int v) {
        if (loading_) return;
        double x = lo + (hi - lo) * v / 1000.0;
        spin->blockSignals(true);
        spin->setValue(x);
        spin->blockSignals(false);
        on(x);
    });
    connect(spin, &QDoubleSpinBox::valueChanged, this, [=](double x) {
        slider->blockSignals(true);
        slider->setValue(int((x - lo) / (hi - lo) * 1000.0));
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

QWidget* ToolSettings::build_joint_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    v->addWidget(new QLabel("Click to place joints, each under the last.\nEnter ends the chain.", w));
    v->addStretch(1);
    return w;
}

void ToolSettings::paint_set(const QString& flag, const QJsonValue& val) {
    if (loading_) return;
    QJsonObject c{{"edit", true}};
    c[flag] = val;
    exec_quiet("artAttrSkinPaintCtx", c);
    Host::get()->poke();
}

QWidget* ToolSettings::build_paint_page() {
    auto* w = new QWidget(this);
    auto* v = new QVBoxLayout(w);
    v->setContentsMargins(0, 0, 0, 0);
    v->addWidget(new QLabel("Influences", w));
    influences_ = new QListWidget(w);
    influences_->setMinimumHeight(120);
    connect(influences_, &QListWidget::currentItemChanged, this, [this](QListWidgetItem* cur, QListWidgetItem*) {
        if (cur) paint_set("influence", cur->data(Qt::UserRole).toString());
    });
    connect(influences_, &QListWidget::itemChanged, this, [this](QListWidgetItem* it) {
        if (loading_) return;
        bool lock = it->checkState() == Qt::Checked;
        QString sk = ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{state()["selection"].toArray().at(0).toString().section('.', 0, 0)}}}).toString();
        exec("skinCluster", QJsonObject{{"args", QJsonArray{sk}}, {"edit", true}, {"influence", it->data(Qt::UserRole).toString()}, {"lockWeights", lock}});
    });
    v->addWidget(influences_, 1);
    auto* hint = new QLabel("A checked influence holds its weights: it shows blue, and the brush leaves it alone.", w);
    hint->setWordWrap(true);
    v->addWidget(hint);
    auto* holds = new QHBoxLayout();
    for (auto [label, on] : {std::pair{"Hold All", true}, {"Release All", false}}) {
        auto* b = new QPushButton(label, w);
        bool lock = on;
        connect(b, &QPushButton::clicked, this, [this, lock] {
            if (!last_skin_.isEmpty()) exec("skinCluster", QJsonObject{{"args", QJsonArray{last_skin_}}, {"edit", true}, {"lockWeights", lock}});
        });
        holds->addWidget(b);
    }
    v->addLayout(holds);
    auto* form = new QFormLayout();
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
    value_ = slider_row(form, "Value:", 0.0, 1.0, 0.05, [this](double x) { paint_set("value", x); });
    opacity_ = slider_row(form, "Opacity:", 0.0, 1.0, 0.05, [this](double x) { paint_set("opacity", x); });
    radius_ = slider_row(form, "Radius(U):", 0.001, 20.0, 0.05, [this](double x) { paint_set("radius", x); });
    feedback_ = new QCheckBox("Color feedback", w);
    connect(feedback_, &QCheckBox::toggled, this, [this](bool on) { paint_set("colorfeedback", on); });
    form->addRow("", feedback_);
    auto* flood = new QPushButton("Flood", w);
    connect(flood, &QPushButton::clicked, this, [] {
        exec("artAttrSkinPaintCtx", QJsonObject{{"edit", true}, {"clear", true}});
    });
    form->addRow("", flood);
    v->addLayout(form);
    return w;
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
    } else if (tool == "joint") {
        title_->setText("Joint Tool");
        stack_->setCurrentWidget(joint_page_);
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
    // The influence list of the selected skinned mesh.
    QString obj;
    QJsonArray sel = state()["selection"].toArray();
    QString sk;
    for (const QJsonValue& s : sel) {
        obj = s.toString().section('.', 0, 0);
        sk = ask("findRelatedSkinCluster", QJsonObject{{"args", QJsonArray{obj}}}).toString();
        if (!sk.isEmpty()) break;
    }
    QStringList names;
    if (!sk.isEmpty())
        for (const QJsonValue& v : ask("skinCluster", QJsonObject{{"args", QJsonArray{sk}}, {"query", true}, {"influence", true}}).toArray()) names << v.toString();
    QString current = paint_query("influence").toString();
    if (sk != last_skin_ || names != last_influences_) {
        last_skin_ = sk;
        last_influences_ = names;
        influences_->clear();
        for (const QString& n : names) {
            auto* it = new QListWidgetItem(n, influences_);
            it->setData(Qt::UserRole, n);
            it->setFlags(it->flags() | Qt::ItemIsUserCheckable);
            it->setCheckState(Qt::Unchecked);
        }
    }
    for (int i = 0; i < influences_->count(); ++i) {
        auto* it = influences_->item(i);
        QString n = it->data(Qt::UserRole).toString();
        bool locked = ask("skinCluster", QJsonObject{{"args", QJsonArray{sk}}, {"query", true}, {"influence", n}, {"lockWeights", true}}).toBool();
        it->setCheckState(locked ? Qt::Checked : Qt::Unchecked);
        it->setText(locked ? n + "  (hold)" : n);
        it->setForeground(locked ? QBrush(QColor(120, 160, 230)) : QBrush(palette().color(QPalette::Text)));
        if (n == current || (current.isEmpty() && i == 0)) influences_->setCurrentItem(it);
    }
    loading_ = false;
}

} // namespace az
