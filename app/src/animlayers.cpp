// The Animation Layer Editor, the Anim tab beside the display layers. A row per layer, top of the stack first: the active
// dot, the name, M S L and ghost, the weight with its key button. Everything goes through animLayer and the layerEditor
// commands; the rows are rebuilt from animLayerOptions' snapshot whenever the scene moves.
#include "panels.h"

#include <QAbstractItemView>
#include <QActionGroup>
#include <QApplication>
#include <QPushButton>
#include <functional>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDoubleSpinBox>
#include <QDropEvent>
#include <QFormLayout>
#include <QCheckBox>
#include <QComboBox>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QInputDialog>
#include <QJsonArray>
#include <QLabel>
#include <QMenu>
#include <QMenuBar>
#include <QSettings>
#include <QSlider>
#include <QToolBar>
#include <QToolButton>
#include <QTreeWidget>
#include <QVBoxLayout>

#include "host.h"

namespace az {

namespace {

QJsonObject on_layer(const QString& layer, const QJsonObject& more = QJsonObject()) {
    QJsonObject call = more;
    call["args"] = QJsonArray{layer};
    call["edit"] = true;
    return call;
}

QJsonObject flag(const char* name, const QJsonValue& v) {
    QJsonObject o;
    o[name] = v;
    return o;
}

// A drop on a row's middle parents; between rows moves. The list shows the top of the stack first.
class LayerTree : public QTreeWidget {
public:
    using QTreeWidget::QTreeWidget;
protected:
    void dropEvent(QDropEvent* e) override {
        auto* moved = currentItem();
        auto* at = itemAt(e->position().toPoint());
        e->setDropAction(Qt::IgnoreAction);
        e->accept();
        if (!moved || !at || moved == at) return;
        const QString layer = moved->data(0, Qt::UserRole).toString();
        const QString other = at->data(0, Qt::UserRole).toString();
        switch (dropIndicatorPosition()) {
        case QAbstractItemView::OnItem:
            exec("animLayer", on_layer(layer, flag("parent", other)));
            break;
        case QAbstractItemView::AboveItem:
            exec("animLayer", on_layer(layer, flag("moveLayerAfter", other)));
            break;
        case QAbstractItemView::BelowItem:
            exec("animLayer", on_layer(layer, flag("moveLayerBefore", other)));
            break;
        default:
            break;
        }
    }
};

QToolButton* toggle(const QString& text, bool on, const QString& tip) {
    auto* b = new QToolButton;
    b->setText(text);
    b->setCheckable(true);
    b->setChecked(on);
    b->setToolTip(tip);
    b->setFixedSize(20, 20);
    b->setAutoRaise(true);
    return b;
}

QColor ghost_colour(int index) {
    static const QColor table[] = {QColor(120, 120, 120), QColor(40, 40, 40), QColor(70, 70, 70), QColor(150, 150, 150), QColor(155, 0, 40), QColor(0, 4, 96), QColor(0, 0, 255), QColor(0, 70, 25), QColor(38, 0, 67), QColor(200, 0, 200)};
    return table[qBound(0, index, 9)];
}

} // namespace

AnimLayerEditor::AnimLayerEditor(QWidget* parent) : Panel(parent) {
    auto* lay = new QVBoxLayout(this);
    lay->setContentsMargins(0, 0, 0, 0);
    lay->setSpacing(0);
    auto* menus = new QMenuBar(this);
    menus->setNativeMenuBar(false);
    build_layers_menu(menus->addMenu("Layers"));
    build_options_menu(menus->addMenu("Options"));
    build_show_menu(menus->addMenu("Show"));
    lay->addWidget(menus);

    auto* bar = new QToolBar(this);
    bar->setIconSize(QSize(16, 16));
    bar->setToolButtonStyle(Qt::ToolButtonTextOnly);
    bar->addAction("Zero Key", this, [this] { zero_key(); })->setToolTip("A key that nullifies the selected layers on the selected objects");
    bar->addAction("Weight Key", this, [this] { key_weight(current()); })->setToolTip("Key the selected layer's weight");
    bar->addSeparator();
    bar->addAction("Empty", this, [this] { create(false, false); })->setToolTip("Create Empty Layer");
    bar->addAction("Selected", this, [this] { create(true, false); })->setToolTip("Create Layer from Selected");
    lay->addWidget(bar);

    list_ = new LayerTree(this);
    list_->setColumnCount(1);
    list_->setHeaderHidden(true);
    list_->setIndentation(12);
    list_->setDragDropMode(QAbstractItemView::InternalMove);
    list_->setSelectionMode(QAbstractItemView::ExtendedSelection);
    list_->setContextMenuPolicy(Qt::CustomContextMenu);
    lay->addWidget(list_, 1);
    connect(list_, &QTreeWidget::itemSelectionChanged, this, [this] {
        if (updating_) return;
        QSet<QString> picked;
        for (auto* it : list_->selectedItems()) picked.insert(it->data(0, Qt::UserRole).toString());
        const QJsonArray layers = ask("animLayerOptions").toObject()["layers"].toArray();
        for (const auto& v : layers) {
            const QString name = v.toObject()["name"].toString();
            const bool on = picked.contains(name);
            if (v.toObject()["selected"].toBool() != on) exec_quiet("animLayer", on_layer(name, QJsonObject{{"selected", on}, {"preferred", on}}));
        }
        Host::get()->poke();
    });
    connect(list_, &QWidget::customContextMenuRequested, this, [this](const QPoint& at) {
        QMenu m;
        build_layers_menu(&m);
        m.exec(list_->viewport()->mapToGlobal(at));
    });
    schedule();
}

QString AnimLayerEditor::current() const {
    auto* it = list_->currentItem();
    return it ? it->data(0, Qt::UserRole).toString() : QString();
}

void AnimLayerEditor::create(bool from_selected, bool override_mode) {
    QSettings prefs;
    QJsonObject call;
    if (from_selected) call["addSelectedObjects"] = true;
    if (override_mode) call["override"] = true;
    call["passthrough"] = prefs.value("animLayers/passthrough", true).toBool();
    call["rotationAccumulationMode"] = prefs.value("animLayers/byLayer", false).toBool() ? 1 : 0;
    call["scaleAccumulationMode"] = prefs.value("animLayers/scaleAdditive", false).toBool() ? 0 : 1;
    const QString made = exec("animLayer", call).toString();
    if (made.isEmpty()) return;
    if (prefs.value("animLayers/insertAfter", false).toBool() && !current().isEmpty() && current() != "BaseAnimation")
        exec("animLayer", on_layer(made, flag("moveLayerAfter", current())));
    if (prefs.value("animLayers/makeCurrent", true).toBool()) {
        const QJsonArray layers = ask("animLayerOptions").toObject()["layers"].toArray();
        for (const auto& v : layers) {
            const QString name = v.toObject()["name"].toString();
            exec_quiet("animLayer", on_layer(name, QJsonObject{{"selected", name == made}, {"preferred", name == made}}));
        }
    }
    Host::get()->poke();
}

void AnimLayerEditor::zero_key() {
    for (auto* it : list_->selectedItems()) exec("setKeyframe", QJsonObject{{"animLayer", it->data(0, Qt::UserRole).toString()}, {"identity", true}});
}

void AnimLayerEditor::key_weight(const QString& layer) {
    if (!layer.isEmpty()) exec("setKeyframe", QJsonObject{{"args", QJsonArray{layer + ".weight"}}});
}

void AnimLayerEditor::for_selected(const std::function<void(const QString&)>& f) {
    for (auto* it : list_->selectedItems()) f(it->data(0, Qt::UserRole).toString());
}

void AnimLayerEditor::merge_options() {
    QDialog d(this);
    d.setWindowTitle("Merge Layers Options");
    auto* form = new QFormLayout(&d);
    auto* to = new QComboBox;
    to->addItems({"Bottom Selected Layer", "Top Selected Layer"});
    auto* children = new QCheckBox;
    children->setChecked(true);
    auto* mode = new QComboBox;
    mode->addItems({"Automatic", "Additive", "Override"});
    auto* del = new QCheckBox;
    del->setChecked(true);
    auto* by = new QDoubleSpinBox;
    by->setRange(0.01, 1000);
    by->setValue(1);
    auto* smart = new QCheckBox;
    auto* tol = new QDoubleSpinBox;
    tol->setDecimals(4);
    tol->setValue(0.01);
    form->addRow("Merge To", to);
    form->addRow("Include Children", children);
    form->addRow("Result Layer Mode", mode);
    form->addRow("Delete Baked Layers", del);
    form->addRow("Sample By", by);
    form->addRow("Smart Bake", smart);
    form->addRow("Fidelity Keys Tolerance", tol);
    auto* box = new QDialogButtonBox(QDialogButtonBox::Ok | QDialogButtonBox::Cancel);
    box->button(QDialogButtonBox::Ok)->setText("Merge");
    connect(box, &QDialogButtonBox::accepted, &d, &QDialog::accept);
    connect(box, &QDialogButtonBox::rejected, &d, &QDialog::reject);
    form->addRow(box);
    if (d.exec() != QDialog::Accepted) return;
    QJsonArray names;
    for_selected([&](const QString& n) { names.append(n); });
    exec("animLayerMerge", QJsonObject{{"args", names}, {"mergeTo", to->currentIndex() ? "top" : "bottom"}, {"includeChildren", children->isChecked()}, {"resultLayerMode", mode->currentText().toLower()}, {"deleteBaked", del->isChecked()}, {"sampleBy", by->value()}, {"smart", smart->isChecked()}, {"fidelity", tol->value()}});
}

void AnimLayerEditor::build_layers_menu(QMenu* m) {
    m->addAction("Create Empty Layer", this, [this] { create(false, false); });
    m->addAction("Create Layer from Selected", this, [this] { create(true, false); });
    m->addAction("Create Override Layer", this, [this] { create(false, true); });
    m->addAction("Create Override Layer from Selected", this, [this] { create(true, true); });
    m->addSeparator();
    m->addAction("Add Selected Objects", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("addSelectedObjects", true))); }); });
    m->addAction("Remove Selected Objects", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("removeSelectedObjects", true))); }); });
    m->addAction("Extract Selected Objects", this, [] { exec("layerEditorExtractObjects"); });
    m->addAction("Move Selected Objects to New Layer", this, [] { exec("layerEditorMoveToNewLayer"); });
    m->addAction("Select Objects", this, [] { exec("layerEditorSelectObjects"); });
    m->addSeparator();
    m->addAction("Copy Layer", this, [] { exec("layerEditorCopyLayer"); });
    m->addAction("Copy Layer (No Animation)", this, [] { exec("layerEditorCopyLayer", flag("noAnimation", true)); });
    m->addSeparator();
    m->addAction("Merge Layers", this, [this] {
        QJsonArray names;
        for_selected([&](const QString& n) { names.append(n); });
        exec("animLayerMerge", QJsonObject{{"args", names}});
    });
    m->addAction("Merge Layers Options...", this, [this] { merge_options(); });
    m->addAction("Extract Non-Layered Animation for Selected Objects", this, [] { exec("layerEditorExtractNonLayered"); });
    m->addAction("Extract Non-Layered Animation for All Objects", this, [] { exec("layerEditorExtractNonLayered", flag("all", true)); });
    m->addSeparator();
    m->addAction("Select All", this, [this] { list_->selectAll(); });
    m->addAction("Select Layer Node", this, [this] { for_selected([](const QString& n) { exec("select", QJsonObject{{"args", QJsonArray{n}}}); }); });
    m->addSeparator();
    m->addAction("Delete", this, [this] { for_selected([](const QString& n) { if (n != "BaseAnimation") exec("delete", QJsonObject{{"args", QJsonArray{n}}}); }); });
    m->addAction("Empty", this, [] { exec("layerEditorEmpty"); });
    m->addAction("Delete Empty Layers", this, [] { exec("layerEditorDeleteEmpty"); });
    m->addSeparator();
    auto* mode = m->addMenu("Layer Mode");
    mode->addAction("Additive", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("override", false))); }); });
    mode->addAction("Override", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, QJsonObject{{"override", true}, {"passthrough", false}})); }); });
    mode->addAction("Passthrough", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, QJsonObject{{"override", true}, {"passthrough", true}})); }); });
    auto* rot = m->addMenu("Rotation Accumulation");
    rot->addAction("By Component", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("rotationAccumulationMode", 0))); }); });
    rot->addAction("By Layer", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("rotationAccumulationMode", 1))); }); });
    auto* sc = m->addMenu("Scale Accumulation");
    sc->addAction("Multiply", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("scaleAccumulationMode", 1))); }); });
    sc->addAction("Additive", this, [this] { for_selected([](const QString& n) { exec("animLayer", on_layer(n, flag("scaleAccumulationMode", 0))); }); });
}

void AnimLayerEditor::build_options_menu(QMenu* m) {
    auto* keys = new QActionGroup(m);
    const int mode = ask("animLayerOptions").toObject()["keyMode"].toInt(2);
    const struct { const char* label; const char* word; int index; } modes[] = {{"Key in Selected Layer(s)", "selected", 0}, {"Key in Last Active Layer(s)", "active", 1}, {"Hybrid", "hybrid", 2}};
    for (const auto& k : modes) {
        auto* a = m->addAction(k.label);
        a->setCheckable(true);
        a->setChecked(mode == k.index);
        keys->addAction(a);
        const QString word = k.word;
        connect(a, &QAction::triggered, this, [word] { exec("animLayerOptions", flag("keyMode", word)); });
    }
    m->addSeparator();
    QSettings prefs;
    const auto pref = [&](const char* label, const char* key, bool def) {
        auto* a = m->addAction(label);
        a->setCheckable(true);
        a->setChecked(prefs.value(key, def).toBool());
        const QString k = key;
        connect(a, &QAction::toggled, this, [k](bool on) { QSettings().setValue(k, on); });
        return a;
    };
    pref("Make New Layers Current", "animLayers/makeCurrent", true);
    pref("Insert New Layers After Current", "animLayers/insertAfter", false);
    auto* lock = m->addAction("Lock Muted Layer");
    lock->setCheckable(true);
    connect(lock, &QAction::toggled, this, [](bool on) { exec("animLayerOptions", flag("lockMuted", on)); });
    m->addSeparator();
    pref("New Layers: Passthrough", "animLayers/passthrough", true);
    pref("New Layers: Rotation by Layer", "animLayers/byLayer", false);
    pref("New Layers: Scale Additive", "animLayers/scaleAdditive", false);
    m->addSeparator();
    auto* rev = pref("Reverse Layer Stack", "animLayers/reverse", false);
    connect(rev, &QAction::toggled, this, [this] { schedule(); });
}

void AnimLayerEditor::build_show_menu(QMenu* m) {
    auto* which = new QActionGroup(m);
    auto* all = m->addAction("All Layers");
    auto* affected = m->addAction("All Affected Layers");
    all->setCheckable(true);
    affected->setCheckable(true);
    all->setChecked(true);
    which->addAction(all);
    which->addAction(affected);
    connect(affected, &QAction::toggled, this, [this](bool on) { only_affected_ = on; schedule(); });
    auto* other = m->addMenu("Other Animation Tools");
    auto* tools = new QActionGroup(other);
    const struct { const char* label; const char* word; } filters[] = {{"All Affecting", "allAffecting"}, {"Active", "active"}, {"Selected", "selected"}, {"Active and Selected", "activeAndSelected"}};
    for (const auto& x : filters) {
        auto* a = other->addAction(x.label);
        a->setCheckable(true);
        a->setChecked(QString(x.word) == "allAffecting");
        tools->addAction(a);
        const QString word = x.word;
        connect(a, &QAction::triggered, this, [word] { exec("animLayerOptions", flag("showFilter", word)); });
    }
}

QWidget* AnimLayerEditor::make_row(const QJsonObject& l) {
    const QString name = l["name"].toString();
    const bool base = l["base"].toBool();
    auto* row = new QWidget;
    auto* h = new QHBoxLayout(row);
    h->setContentsMargins(2, 0, 2, 0);
    h->setSpacing(1);
    // Green takes keys, red holds channels of the selection but doesn't.
    auto* dot = new QLabel("●");
    dot->setStyleSheet(QString("color: %1").arg(l["active"].toBool() ? "#3fd14a" : l["affected"].toBool() ? "#d9443f" : "transparent"));
    h->addWidget(dot);
    auto* label = new QLabel(name);
    label->setMinimumWidth(40);
    QString mode = l["override"].toBool() ? (l["passthrough"].toBool() ? "Override, passthrough" : "Override") : "Additive";
    label->setToolTip(base ? "The base animation" : mode + QString(", %1 channels").arg(l["channels"].toInt()));
    h->addWidget(label, 1);
    if (!base) {
        auto* tag = new QLabel(l["override"].toBool() ? "O" : "A");
        tag->setToolTip(mode);
        tag->setStyleSheet("color: #9a9a9a");
        h->addWidget(tag);
        const auto wire = [&](const QString& text, bool on, const char* word, const QString& tip) {
            auto* b = toggle(text, on, tip);
            const QString f = word;
            connect(b, &QToolButton::toggled, this, [name, f](bool now) { exec("animLayer", on_layer(name, flag(f.toUtf8().constData(), now))); });
            h->addWidget(b);
        };
        wire("M", l["mute"].toBool(), "mute", "Mute");
        wire("S", l["solo"].toBool(), "solo", "Solo");
    }
    auto* lock = toggle("L", l["lock"].toBool(), "Lock");
    connect(lock, &QToolButton::toggled, this, [name](bool now) { exec("animLayer", on_layer(name, flag("lock", now))); });
    h->addWidget(lock);
    if (base) return row;
    auto* ghost = toggle("G", l["ghost"].toBool(), "Ghost; the right button sets the colour");
    ghost->setStyleSheet(QString("QToolButton { color: white; background: %1 }").arg(ghost_colour(l["ghostColor"].toInt()).name()));
    ghost->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(ghost, &QToolButton::toggled, this, [name](bool now) { exec("animLayer", on_layer(name, flag("ghost", now))); });
    connect(ghost, &QWidget::customContextMenuRequested, this, [name] {
        bool ok = false;
        int c = QInputDialog::getInt(nullptr, "Ghost Colour", "Colour index", 5, 0, 31, 1, &ok);
        if (ok) exec("animLayer", on_layer(name, flag("ghostColor", c)));
    });
    h->addWidget(ghost);
    auto* slider = new QSlider(Qt::Horizontal);
    slider->setRange(0, 100);
    slider->setValue(qRound(l["weight"].toDouble() * 100));
    slider->setFixedWidth(60);
    auto* spin = new QDoubleSpinBox;
    spin->setRange(0, 1);
    spin->setDecimals(2);
    spin->setSingleStep(0.05);
    spin->setButtonSymbols(QAbstractSpinBox::NoButtons);
    spin->setValue(l["weight"].toDouble());
    spin->setFixedWidth(38);
    if (l["weightKey"].toBool()) spin->setStyleSheet("background: #cd2222; color: white");
    else if (l["weightAnimated"].toBool()) spin->setStyleSheet("background: #dd727a; color: black");
    connect(slider, &QSlider::valueChanged, this, [name, spin](int v) {
        const QSignalBlocker block(spin);
        spin->setValue(v / 100.0);
        exec_quiet("animLayer", on_layer(name, flag("weight", v / 100.0)));
        Host::get()->poke();
    });
    connect(slider, &QSlider::sliderReleased, this, [name, slider] { exec("animLayer", on_layer(name, flag("weight", slider->value() / 100.0))); });
    connect(spin, &QDoubleSpinBox::editingFinished, this, [name, spin] { exec("animLayer", on_layer(name, flag("weight", spin->value()))); });
    h->addWidget(slider);
    h->addWidget(spin);
    auto* key = new QToolButton;
    key->setText("K");
    key->setToolTip("Key the weight");
    key->setFixedSize(20, 20);
    key->setAutoRaise(true);
    connect(key, &QToolButton::clicked, this, [this, name] { key_weight(name); });
    h->addWidget(key);
    return row;
}

void AnimLayerEditor::refresh() {
    const QJsonObject info = ask("animLayerOptions").toObject();
    const QJsonArray layers = info["layers"].toArray();
    // The sliders are being dragged: their own row is up to date, and rebuilding would drop the grab.
    if (auto* s = qobject_cast<QSlider*>(QApplication::focusWidget()); s && s->isSliderDown()) return;
    updating_ = true;
    const QString was = current();
    list_->clear();
    QHash<QString, QTreeWidgetItem*> items;
    const bool reverse = QSettings().value("animLayers/reverse", false).toBool();
    // The stack reads top down: later layers first, unless reversed. Parents exist before their children either way.
    QList<QJsonObject> rows;
    for (const auto& v : layers) rows.append(v.toObject());
    QList<QTreeWidgetItem*> tops;
    for (const auto& l : rows) {
        if (only_affected_ && !l["active"].toBool() && !l["affected"].toBool() && !l["base"].toBool()) continue;
        auto* it = new QTreeWidgetItem;
        it->setData(0, Qt::UserRole, l["name"].toString());
        items[l["name"].toString()] = it;
        auto* up = items.value(l["parent"].toString());
        if (up && !up->data(0, Qt::UserRole + 1).toBool()) {
            if (reverse) up->addChild(it);
            else up->insertChild(0, it);
        } else if (reverse) {
            tops.append(it);
        } else {
            tops.prepend(it);
        }
        if (l["base"].toBool()) it->setData(0, Qt::UserRole + 1, true);
    }
    list_->addTopLevelItems(tops);
    for (const auto& l : rows) {
        auto* it = items.value(l["name"].toString());
        if (!it) continue;
        it->setSizeHint(0, QSize(0, 24));
        if (l["base"].toBool()) it->setFlags(it->flags() & ~Qt::ItemIsDragEnabled);
        list_->setItemWidget(it, 0, make_row(l));
        it->setExpanded(!l["collapse"].toBool());
        it->setSelected(l["selected"].toBool());
        if (l["name"].toString() == was) list_->setCurrentItem(it, 0, QItemSelectionModel::NoUpdate);
    }
    updating_ = false;
}

} // namespace az
