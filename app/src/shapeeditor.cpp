#include "shapeeditor.h"
#include "host.h"
#include <QDoubleSpinBox>
#include <QFrame>
#include <QHBoxLayout>
#include <QJsonArray>
#include <QJsonObject>
#include <QLabel>
#include <QPushButton>
#include <QScrollArea>
#include <QSlider>
#include <QToolButton>
#include <QVBoxLayout>

namespace az {

namespace {

// Alias, attribute pairs of a blend shape's targets: {"pSphere2", 0}, by index.
QList<QPair<QString, int>> targets_of(const QString& blend) {
    QList<QPair<QString, int>> out;
    QJsonArray pairs = ask("aliasAttr", QJsonObject{{"args", QJsonArray{blend}}, {"query", true}}).toArray();
    for (int i = 0; i + 1 < pairs.size(); i += 2) {
        QString attr = pairs.at(i + 1).toString();
        if (!attr.startsWith("weight[")) continue;
        out.append({pairs.at(i).toString(), attr.mid(7, attr.size() - 8).toInt()});
    }
    return out;
}

double plug_value(const QString& plug) {
    return ask("getAttr", QJsonObject{{"args", QJsonArray{plug}}}).toDouble();
}

} // namespace

ShapeEditor::ShapeEditor(QWidget* parent) : Panel(parent) {
    auto* root = new QVBoxLayout(this);
    root->setContentsMargins(6, 6, 6, 6);
    auto* bar = new QHBoxLayout();
    auto* create = new QPushButton("Create Blend Shape", this);
    create->setToolTip("From the selection: any targets, then the base.");
    connect(create, &QPushButton::clicked, this, [] { exec("blendShape"); });
    bar->addWidget(create);
    bar->addStretch(1);
    root->addLayout(bar);
    auto* scroll = new QScrollArea(this);
    scroll->setWidgetResizable(true);
    scroll->setFrameShape(QFrame::NoFrame);
    auto* holder = new QWidget(scroll);
    list_ = new QVBoxLayout(holder);
    list_->setContentsMargins(0, 0, 0, 0);
    scroll->setWidget(holder);
    root->addWidget(scroll, 1);
}

void ShapeEditor::add_target(const QString& blend) {
    QJsonArray geo = ask("blendShape", QJsonObject{{"args", QJsonArray{blend}}, {"query", true}, {"queried", QJsonArray{"geometry"}}}).toArray();
    if (geo.isEmpty()) return;
    int next = 0;
    for (const auto& t : targets_of(blend)) next = qMax(next, t.second + 1);
    QJsonArray sel = state()["selection"].toArray();
    if (sel.isEmpty()) {
        log("// Warning: select the meshes to add as targets //", 4);
        return;
    }
    for (const QJsonValue& s : sel) {
        QString name = s.toString().section('.', 0, 0);
        exec("blendShape", QJsonObject{{"args", QJsonArray{blend}}, {"edit", true}, {"target", QJsonArray{geo.at(0), next++, name, 1.0}}});
    }
}

void ShapeEditor::rebuild(const QStringList& shapes) {
    while (QLayoutItem* it = list_->takeAt(0)) {
        if (it->widget()) it->widget()->deleteLater();
        delete it;
    }
    rows_.clear();
    for (const QString& blend : shapes) {
        auto* box = new QFrame(this);
        box->setFrameShape(QFrame::StyledPanel);
        auto* v = new QVBoxLayout(box);
        auto* head = new QHBoxLayout();
        auto* title = new QLabel(blend, box);
        QFont f = title->font();
        f.setBold(true);
        title->setFont(f);
        head->addWidget(title, 1);
        auto* add = new QPushButton("Add Target", box);
        add->setToolTip("The selected meshes become targets.");
        connect(add, &QPushButton::clicked, this, [this, blend] { add_target(blend); });
        head->addWidget(add);
        v->addLayout(head);
        for (const auto& t : targets_of(blend)) {
            QString plug = blend + "." + t.first;
            int index = t.second;
            auto* line = new QHBoxLayout();
            auto* name = new QLabel(t.first, box);
            name->setMinimumWidth(110);
            line->addWidget(name);
            Row r;
            r.slider = new QSlider(Qt::Horizontal, box);
            r.slider->setRange(0, 1000);
            r.spin = new QDoubleSpinBox(box);
            r.spin->setRange(-10.0, 10.0);
            r.spin->setDecimals(3);
            r.spin->setSingleStep(0.05);
            r.key = new QToolButton(box);
            r.key->setText("Key");
            r.key->setToolTip("Key this weight at the current time.");
            r.edit = new QToolButton(box);
            r.edit->setText("Edit");
            r.edit->setCheckable(true);
            r.edit->setToolTip("Edits of the base go into this target until this is off.");
            auto* del = new QToolButton(box);
            del->setText("X");
            del->setToolTip("Remove this target.");
            line->addWidget(r.slider, 1);
            line->addWidget(r.spin);
            line->addWidget(r.key);
            line->addWidget(r.edit);
            line->addWidget(del);
            v->addLayout(line);
            connect(r.slider, &QSlider::valueChanged, this, [this, plug](int x) {
                if (loading_) return;
                exec_quiet("setAttr", QJsonObject{{"args", QJsonArray{plug, x / 1000.0}}});
                Host::get()->poke();
            });
            connect(r.slider, &QSlider::sliderReleased, this, [this, plug] {
                if (rows_.contains(plug)) exec("setAttr", QJsonObject{{"args", QJsonArray{plug, rows_[plug].slider->value() / 1000.0}}});
            });
            connect(r.spin, &QDoubleSpinBox::editingFinished, this, [this, plug] {
                if (!loading_ && rows_.contains(plug)) exec("setAttr", QJsonObject{{"args", QJsonArray{plug, rows_[plug].spin->value()}}});
            });
            connect(r.key, &QToolButton::clicked, this, [plug] { exec("setKeyframe", QJsonObject{{"args", QJsonArray{plug}}}); });
            connect(r.edit, &QToolButton::clicked, this, [blend, index](bool on) {
                exec("sculptTarget", QJsonObject{{"args", QJsonArray{blend}}, {"edit", true}, {"target", on ? index : -1}});
            });
            connect(del, &QToolButton::clicked, this, [blend, t] {
                exec("blendShape", QJsonObject{{"args", QJsonArray{blend}}, {"edit", true}, {"resetTargetDelta", t.first}});
            });
            rows_.insert(plug, r);
        }
        list_->addWidget(box);
    }
    list_->addStretch(1);
}

void ShapeEditor::refresh() {
    QStringList shapes, key;
    for (const QJsonValue& b : ask("ls", QJsonObject{{"type", "blendShape"}}).toArray()) {
        QString blend = b.toString();
        shapes << blend;
        key << blend;
        for (const auto& t : targets_of(blend)) key << blend + "." + t.first;
    }
    if (key != layout_key_) {
        layout_key_ = key;
        rebuild(shapes);
    }
    loading_ = true;
    for (const QString& blend : shapes) {
        int editing = ask("sculptTarget", QJsonObject{{"args", QJsonArray{blend}}, {"query", true}}).toInt(-1);
        for (const auto& t : targets_of(blend)) {
            QString plug = blend + "." + t.first;
            if (!rows_.contains(plug)) continue;
            Row& r = rows_[plug];
            double x = plug_value(plug);
            if (!r.slider->isSliderDown()) r.slider->setValue(int(x * 1000.0 + (x < 0 ? -0.5 : 0.5)));
            if (!r.spin->hasFocus()) r.spin->setValue(x);
            r.edit->setChecked(editing == t.second);
        }
    }
    loading_ = false;
}

} // namespace az
