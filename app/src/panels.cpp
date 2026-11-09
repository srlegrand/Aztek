#include "panels.h"
#include "colorchooser.h"
#include <QStyle>
#include <QMouseEvent>
#include "host.h"
#include "icons.h"

#include <QApplication>
#include <QCheckBox>
#include <QComboBox>
#include <QDropEvent>
#include <QColorDialog>
#include <QFormLayout>
#include <QHash>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QMenuBar>
#include <QPlainTextEdit>
#include <QPushButton>
#include <QScrollArea>
#include <QScrollBar>
#include <QSplitter>
#include <QTabWidget>
#include <QTimer>
#include <QToolBar>
#include <QToolButton>
#include <QTreeWidget>
#include <QVBoxLayout>

namespace az {

namespace {

QJsonObject args(const QJsonArray& a) {
    QJsonObject o;
    o["args"] = a;
    return o;
}

QString fmt(double v) {
    // The reference shows three decimals in the channel box.
    QString s = QString::number(v, 'f', 3);
    if (s == "-0.000") s = "0.000";
    return s;
}

QString value_text(const QJsonObject& ch) {
    QJsonValue v = ch["value"];
    QString t = ch["type"].toString();
    if (t == "bool") return v.toBool() ? "on" : "off";
    if (t == "enum") {
        QJsonArray names = ch["enum"].toArray();
        int i = v.toInt();
        if (i >= 0 && i < names.size()) return names[i].toString();
        return QString::number(i);
    }
    if (t == "int") return QString::number(v.toInteger());
    if (t == "float") return fmt(v.toDouble());
    if (t == "string") return v.toString();
    return QString();
}

// Text typed into a field to the setAttr value for its type.
QJsonValue typed(const QString& t, const QString& text, const QJsonObject& ch) {
    QString s = text.trimmed();
    if (t == "bool") return QJsonValue(s == "on" || s == "1" || s == "true" || s == "yes");
    if (t == "int") return QJsonValue(qint64(s.toDouble()));
    if (t == "enum") {
        QJsonArray names = ch["enum"].toArray();
        for (int i = 0; i < names.size(); ++i)
            if (names[i].toString().compare(s, Qt::CaseInsensitive) == 0) return QJsonValue(i);
        return QJsonValue(qint64(s.toDouble()));
    }
    if (t == "float") return QJsonValue(s.toDouble());
    return QJsonValue(s);
}

void set_attr(const QString& plug, const QJsonValue& v, const QString& type) {
    QJsonObject call = args(QJsonArray{plug, v});
    if (type == "string") call["type"] = "string";
    exec("setAttr", call);
}

} // namespace

// ---------------------------------------------------------------------------------------------------------------------

Panel::Panel(QWidget* parent) : QWidget(parent) {
    connect(Host::get(), &Host::sceneChanged, this, &Panel::schedule);
}

void Panel::schedule() {
    if (pending_) return;
    pending_ = true;
    QTimer::singleShot(0, this, [this] {
        pending_ = false;
        if (!isVisible()) return;
        // Don't pull a field out from under the user's typing.
        QWidget* f = QApplication::focusWidget();
        if (f && isAncestorOf(f) && f->property("editing").toBool()) return;
        refresh();
    });
}

void Panel::showEvent(QShowEvent*) {
    schedule();
}

// ---------------------------------------------------------------------------------------------------------------------

// Channel names picked in the channel box (the reference's highlighted channels): their attribute names, kept across refreshes.
static QStringList g_channel_sel;
static QList<QLabel*> g_channel_labels;
// Where a Shift+click or a drag over the names starts its range.
static QString g_channel_anchor;
// attr to whole plug for the picked channels under INPUTS.
static QHash<QString, QString> g_input_plugs;

QStringList selected_channels() {
    return g_channel_sel;
}

QStringList selected_input_plugs() {
    QStringList out;
    for (const QString& a : g_channel_sel)
        if (g_input_plugs.contains(a)) out << g_input_plugs[a];
    return out;
}

void clear_channel_selection() {
    g_channel_sel.clear();
    g_input_plugs.clear();
    g_channel_anchor.clear();
}

namespace {

void restyle_channel_labels() {
    for (QLabel* l : g_channel_labels) {
        l->setProperty("picked", g_channel_sel.contains(l->property("attr").toString()));
        l->style()->unpolish(l);
        l->style()->polish(l);
    }
}

// The channel names from the anchor's row to this one, in the order the box shows them.
QStringList channel_range(const QString& to) {
    int a = -1, b = -1;
    for (int i = 0; i < g_channel_labels.size(); ++i) {
        QString n = g_channel_labels[i]->property("attr").toString();
        if (a < 0 && n == g_channel_anchor) a = i;
        if (b < 0 && n == to) b = i;
    }
    if (a < 0 || b < 0) return {to};
    QStringList out;
    for (int i = qMin(a, b); i <= qMax(a, b); ++i) {
        QString n = g_channel_labels[i]->property("attr").toString();
        if (!out.contains(n)) out << n;
    }
    return out;
}

// The reference's picking, on a name or a value: click picks one, Ctrl+click toggles, Shift+click picks the run from the
// last click. A click on a value that is already picked keeps the others, so the value typed there goes to all of them.
void pick_channel(const QString& a, Qt::KeyboardModifiers mods, bool on_value) {
    if (mods & Qt::ControlModifier) {
        if (g_channel_sel.contains(a)) g_channel_sel.removeAll(a);
        else g_channel_sel << a;
        g_channel_anchor = a;
    } else if ((mods & Qt::ShiftModifier) && !g_channel_anchor.isEmpty()) {
        g_channel_sel = channel_range(a);
    } else if (!(on_value && g_channel_sel.contains(a))) {
        g_channel_sel = {a};
        g_input_plugs.clear();
        g_channel_anchor = a;
    }
}

// A value field: a click picks its channel the same way, before the field takes the focus.
class ChannelValueClicks : public QObject {
public:
    using QObject::QObject;
protected:
    bool eventFilter(QObject* o, QEvent* e) override {
        if (e->type() == QEvent::MouseButtonPress && static_cast<QMouseEvent*>(e)->button() == Qt::LeftButton) {
            pick_channel(o->property("attr").toString(), static_cast<QMouseEvent*>(e)->modifiers(), true);
            restyle_channel_labels();
        }
        return false;
    }
};

// A channel name: click picks it, Ctrl+click toggles, Shift+click or a drag down the names picks a run.
class ChannelLabel : public QLabel {
public:
    ChannelLabel(const QString& text, QWidget* parent) : QLabel(text, parent) {}
protected:
    void mouseMoveEvent(QMouseEvent* e) override {
        if (!(e->buttons() & Qt::LeftButton) || g_channel_anchor.isEmpty()) return QLabel::mouseMoveEvent(e);
        auto* over = qobject_cast<QLabel*>(QApplication::widgetAt(e->globalPosition().toPoint()));
        if (!over || !g_channel_labels.contains(over)) return;
        QStringList run = channel_range(over->property("attr").toString());
        if (run == g_channel_sel) return;
        g_channel_sel = run;
        restyle_channel_labels();
    }
    void mousePressEvent(QMouseEvent* e) override {
        if (e->button() != Qt::LeftButton) return QLabel::mousePressEvent(e);
        QString a = property("attr").toString();
        pick_channel(a, e->modifiers(), false);
        if (property("inputPlug").isValid()) {
            g_input_plugs[a] = property("inputPlug").toString();
            // The channel's handle lights up, and a middle drag takes it.
            QString plug = property("inputPlug").toString();
            query("uiShowManip", plug.section('.', 0, -2));
            query("uiShowManipAttr", plug.section('.', -1));
        }
        else g_input_plugs.remove(a);
        restyle_channel_labels();
    }
};

// A value typed into a field. When its channel is picked, it goes to every picked channel of that node, and for the
// objects' own channels to every selected object, as one undo step. +=, -=, *= and /= work on each plug's own value.
void commit_channels(QLabel* edited, const QString& plug, const QString& text) {
    QString attr = edited->property("attr").toString();
    QString node = edited->property("node").toString();
    QList<QJsonObject> chans;
    if (g_channel_sel.contains(attr)) {
        for (QLabel* l : g_channel_labels)
            if (l->property("node").toString() == node && g_channel_sel.contains(l->property("attr").toString()))
                chans << QJsonObject::fromVariantMap(l->property("channel").toMap());
    } else {
        chans << QJsonObject::fromVariantMap(edited->property("channel").toMap());
    }
    QStringList nodes{node};
    if (edited->property("main").toBool()) {
        nodes.clear();
        for (const QJsonValue& v : state()["selection"].toArray()) {
            QString n = v.toString().section('.', 0, 0);
            if (!nodes.contains(n)) nodes << n;
        }
        if (nodes.isEmpty()) nodes << node;
    }
    QString t = text.trimmed();
    QString op;
    for (QString o : {"+=", "-=", "*=", "/="})
        if (t.startsWith(o)) op = o;
    bool many = chans.size() * nodes.size() > 1;
    if (many) exec_quiet("undoInfo", QJsonObject{{"openChunk", true}});
    for (const QJsonObject& c : chans) {
        QString type = c["type"].toString();
        if (c["state"].toString() == "locked" && c["plug"].toString() != plug) continue;
        for (const QString& n : nodes) {
            QString p = n + "." + c["attr"].toString();
            QString value = t;
            if (!op.isEmpty() && (type == "float" || type == "int")) {
                double current = p == c["plug"].toString() ? c["value"].toDouble() : ask("getAttr", args(QJsonArray{p})).toDouble();
                double x = t.mid(2).toDouble();
                double r = op == "+=" ? current + x : op == "-=" ? current - x : op == "*=" ? current * x : (x != 0 ? current / x : current);
                value = QString::number(r, 'g', 12);
            }
            set_attr(p, typed(type, value, c), type);
        }
    }
    if (many) exec_quiet("undoInfo", QJsonObject{{"closeChunk", true}});
}

// A channel row: right-aligned name, value field. Enter commits a setAttr.
QWidget* channel_row(const QJsonObject& ch, QWidget* parent, int label_width) {
    auto* row = new QWidget(parent);
    auto* h = new QHBoxLayout(row);
    h->setContentsMargins(0, 0, 0, 0);
    h->setSpacing(4);
    auto* label = new ChannelLabel(ch["nice"].toString(), row);
    label->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    label->setFixedWidth(label_width);
    label->setToolTip(ch["plug"].toString());
    label->setObjectName("channelName");
    label->setProperty("attr", ch["attr"].toString());
    if (parent->property("inInputs").toBool()) label->setProperty("inputPlug", ch["plug"].toString());
    label->setProperty("picked", g_channel_sel.contains(ch["attr"].toString()));
    // The node the row belongs to, and whether it is the selected objects' own channel (the value goes to every
    // selected object) or a shape's or an input's (the value goes to that node).
    QString node = ch["plug"].toString().section('.', 0, -2);
    label->setProperty("node", node);
    label->setProperty("main", parent->property("mainNode").toString() == node);
    label->setProperty("channel", ch.toVariantMap());
    g_channel_labels << label;
    auto* edit = new QLineEdit(value_text(ch), row);
    edit->setProperty("attr", ch["attr"].toString());
    static auto* clicks = new ChannelValueClicks(qApp);
    edit->installEventFilter(clicks);
    // The reference's channel colours: keyed, key on this frame, changed since keyed, driven, locked.
    QString st = ch["state"].toString();
    edit->setObjectName(st == "keyed" ? "channelKeyed" : st == "keyOnFrame" ? "channelKeyOnFrame" : st == "changed" ? "channelChanged"
                        : st == "layered" ? "channelLayered" : st == "muted" ? "channelMuted" : st == "locked" ? "channelLocked" : (st == "connected" || ch["connected"].toBool()) ? "channelConnected" : "channelValue");
    edit->setFixedHeight(18);
    // Right-click is the channel box's menu, not the text field's.
    edit->setContextMenuPolicy(Qt::NoContextMenu);
    QString plug = ch["plug"].toString();
    QObject::connect(edit, &QLineEdit::textEdited, edit, [edit] { edit->setProperty("editing", true); });
    QObject::connect(edit, &QLineEdit::editingFinished, edit, [edit, label, plug] {
        if (!edit->property("editing").toBool()) return;
        edit->setProperty("editing", false);
        commit_channels(label, plug, edit->text());
    });
    h->addWidget(label);
    h->addWidget(edit, 1);
    return row;
}

QWidget* section(const QString& text, QWidget* parent) {
    auto* l = new QLabel(text, parent);
    l->setObjectName("channelSection");
    return l;
}

} // namespace

namespace {

// The reference's Channels menu, also the channel box's right-click menu: keys, mutes, connections and locks on the
// picked channels of the selected objects.
void fill_channels_menu(QMenu* m) {
    QJsonArray picked;
    for (const QString& a : g_channel_sel) picked.append(a);
    bool any = !picked.isEmpty();
    auto plugs = [] {
        QJsonArray out;
        QJsonArray sel = state()["selection"].toArray();
        for (const QJsonValue& o : sel)
            for (const QString& a : g_channel_sel) out.append(o.toString().section('.', 0, 0) + "." + a);
        return out;
    };
    m->addAction("Key Selected", [picked] { exec("setKeyframe", QJsonObject{{"attribute", picked}}); })->setEnabled(any);
    m->addAction("Key All", [] { exec("setKeyframe"); });
    m->addAction("Breakdown Selected", [picked] { exec("setKeyframe", QJsonObject{{"attribute", picked}, {"breakdown", true}}); })->setEnabled(any);
    m->addAction("Breakdown All", [] { exec("setKeyframe", QJsonObject{{"breakdown", true}}); });
    m->addSeparator();
    m->addAction("Mute Selected", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("mute", QJsonObject{{"args", QJsonArray{pl}}});
    })->setEnabled(any);
    m->addAction("Mute All", [] { exec("mute"); });
    m->addAction("Unmute Selected", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("mute", QJsonObject{{"args", QJsonArray{pl}}, {"disable", true}, {"force", true}});
    })->setEnabled(any);
    m->addAction("Unmute All", [] { exec("mute", QJsonObject{{"disable", true}, {"force", true}}); });
    m->addSeparator();
    m->addAction("Delete Selected Keys", [picked] { exec("cutKey", QJsonObject{{"attribute", picked}}); })->setEnabled(any);
    m->addAction("Break Connections", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("disconnectAttr", QJsonObject{{"args", QJsonArray{pl}}});
    })->setEnabled(any);
    m->addSeparator();
    m->addAction("Lock Selected", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("setAttr", QJsonObject{{"args", QJsonArray{pl}}, {"lock", true}});
    })->setEnabled(any);
    m->addAction("Unlock Selected", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("setAttr", QJsonObject{{"args", QJsonArray{pl}}, {"lock", false}});
    })->setEnabled(any);
    m->addAction("Hide Selected", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("setAttr", QJsonObject{{"args", QJsonArray{pl}}, {"keyable", false}});
    })->setEnabled(any);
    m->addAction("Lock and Hide Selected", [plugs] {
        for (const QJsonValue& pl : plugs()) exec("setAttr", QJsonObject{{"args", QJsonArray{pl}}, {"lock", true}, {"keyable", false}});
    })->setEnabled(any);
    m->addSeparator();
    m->addAction("Select All Channels", [] {
        g_channel_sel.clear();
        for (QLabel* l : g_channel_labels) g_channel_sel << l->property("attr").toString();
        restyle_channel_labels();
    });
    m->addAction("Deselect Channels", [] {
        g_channel_sel.clear();
        restyle_channel_labels();
    });
}

} // namespace

ChannelBox::ChannelBox(QWidget* parent) : Panel(parent) {
    auto* lay = new QVBoxLayout(this);
    lay->setContentsMargins(0, 0, 0, 0);
    lay->setSpacing(0);
    // The reference's menu bar over the channels: Channels, Edit, Object.
    auto* bar = new QMenuBar(this);
    bar->setNativeMenuBar(false);
    bar->setObjectName("panelMenus");
    QMenu* channels = bar->addMenu("Channels");
    connect(channels, &QMenu::aboutToShow, channels, [channels] {
        channels->clear();
        fill_channels_menu(channels);
    });
    QMenu* edit = bar->addMenu("Edit");
    edit->addAction("Expressions...", [] { mel_action("ExpressionEditor"); });
    edit->addAction("Set Driven Key...", [] { mel_action("SetDrivenKeyOptions"); });
    edit->addAction("Connection Editor...", [] { mel_action("ConnectionEditor"); });
    edit->addAction("Graph Editor...", [] { mel_action("GraphEditor"); });
    edit->addSeparator();
    edit->addAction("Channel Control...", [] { mel_action("ChannelControlEditor"); });
    // Object: the selected objects; picking one shows it here.
    QMenu* object = bar->addMenu("Object");
    connect(object, &QMenu::aboutToShow, object, [object] {
        object->clear();
        QJsonArray sel = state()["selection"].toArray();
        QStringList nodes;
        for (const QJsonValue& v : sel) {
            QString n = v.toString().section('.', 0, 0);
            if (!nodes.contains(n)) nodes << n;
        }
        if (nodes.isEmpty()) object->addAction("Nothing selected")->setEnabled(false);
        for (const QString& n : nodes) {
            QAction* a = object->addAction(n.section('|', -1), [nodes, n] {
                // The lead object is the last selected: move this one to the end.
                QJsonArray order;
                for (const QString& o : nodes)
                    if (o != n) order.append(o);
                order.append(n);
                exec("select", QJsonObject{{"args", order}, {"replace", true}});
            });
            a->setCheckable(true);
            a->setChecked(n == nodes.last());
        }
    });
    lay->addWidget(bar);
    scroll_ = new QScrollArea(this);
    scroll_->setWidgetResizable(true);
    scroll_->setFrameShape(QFrame::NoFrame);
    lay->addWidget(scroll_);
    setObjectName("channelBox");
    setContextMenuPolicy(Qt::CustomContextMenu);
    connect(this, &QWidget::customContextMenuRequested, this, [this](const QPoint& p) {
        QMenu m;
        fill_channels_menu(&m);
        m.exec(mapToGlobal(p));
    });
}

void ChannelBox::add_channels(QVBoxLayout* lay, const QJsonArray& channels) {
    QFontMetrics fm(font());
    int w = 60;
    for (const QJsonValue& c : channels) w = qMax(w, fm.horizontalAdvance(c.toObject()["nice"].toString()) + 8);
    for (const QJsonValue& c : channels) lay->addWidget(channel_row(c.toObject(), body_, w));
}

void ChannelBox::refresh() {
    QJsonObject cb = query("channelBox").toObject();
    g_channel_labels.clear();
    int scroll = scroll_->verticalScrollBar()->value();
    body_ = new QWidget();
    auto* lay = new QVBoxLayout(body_);
    lay->setContentsMargins(4, 4, 4, 4);
    lay->setSpacing(1);
    if (cb.isEmpty()) {
        lay->addStretch(1);
        scroll_->setWidget(body_);
        return;
    }
    // Object name, editable to rename.
    auto* name = new QLineEdit(cb["name"].toString(), body_);
    name->setObjectName("channelHeader");
    QString path = cb["path"].toString();
    connect(name, &QLineEdit::textEdited, name, [name] { name->setProperty("editing", true); });
    connect(name, &QLineEdit::editingFinished, name, [name, path] {
        if (!name->property("editing").toBool()) return;
        name->setProperty("editing", false);
        exec("rename", args(QJsonArray{path, name->text().trimmed()}));
    });
    lay->addWidget(name);
    body_->setProperty("mainNode", path);
    add_channels(lay, cb["channels"].toArray());
    QJsonArray shapes = cb["shapes"].toArray();
    if (!shapes.isEmpty()) {
        lay->addSpacing(6);
        lay->addWidget(section("SHAPES", body_));
        for (const QJsonValue& s : shapes) {
            auto* l = new QLabel(s.toObject()["name"].toString(), body_);
            l->setObjectName("channelNode");
            lay->addWidget(l);
            add_channels(lay, s.toObject()["channels"].toArray());
        }
    }
    QJsonArray inputs = cb["inputs"].toArray();
    if (!inputs.isEmpty()) {
        lay->addSpacing(6);
        lay->addWidget(section("INPUTS", body_));
        for (const QJsonValue& v : inputs) {
            QJsonObject in = v.toObject();
            QString p = in["path"].toString();
            auto* b = new QPushButton(in["name"].toString(), body_);
            b->setObjectName("channelNodeButton");
            b->setFlat(true);
            connect(b, &QPushButton::clicked, this, [this, p] {
                // The Show Manipulator tool shows the node picked here.
                query("uiShowManip", p);
                if (open_inputs_.contains(p))
                    open_inputs_.remove(p);
                else
                    open_inputs_.insert(p);
                refresh();
            });
            lay->addWidget(b);
            if (open_inputs_.contains(p)) {
                body_->setProperty("inInputs", true);
                add_channels(lay, in["channels"].toArray());
                body_->setProperty("inInputs", false);
            }
        }
    }
    lay->addStretch(1);
    scroll_->setWidget(body_);
    scroll_->verticalScrollBar()->setValue(scroll);
}

// ---------------------------------------------------------------------------------------------------------------------

LayerEditor::LayerEditor(QWidget* parent) : Panel(parent) {
    auto* lay = new QVBoxLayout(this);
    lay->setContentsMargins(2, 2, 2, 2);
    lay->setSpacing(2);
    auto* bar = new QToolBar(this);
    bar->setIconSize(QSize(16, 16));
    bar->addAction(icon("layer"), "Create a new layer", [] { exec("createDisplayLayer", QJsonObject{{"empty", true}}); });
    bar->addAction(icon("layerSelected"), "Create a new layer and assign selected objects", [] { exec("createDisplayLayer"); });
    lay->addWidget(bar);
    list_ = new QTreeWidget(this);
    // V, P, T or R, colour, name.
    list_->setColumnCount(5);
    list_->setHeaderHidden(true);
    list_->setRootIsDecorated(false);
    list_->header()->setStretchLastSection(true);
    for (int i = 0; i < 4; ++i) list_->setColumnWidth(i, 22);
    list_->setDragDropMode(QAbstractItemView::InternalMove);
    list_->setSelectionMode(QAbstractItemView::SingleSelection);
    lay->addWidget(list_, 1);
    // A drag in the list is the new order, top first.
    connect(list_->model(), &QAbstractItemModel::rowsMoved, this, [this] {
        for (int i = 0; i < list_->topLevelItemCount(); ++i)
            exec_quiet("reorderDisplayLayer", QJsonObject{{"args", QJsonArray{list_->topLevelItem(i)->data(0, Qt::UserRole).toString(), list_->topLevelItemCount() - 1 - i}}});
        Host::get()->poke();
    });
    connect(list_, &QTreeWidget::itemClicked, this, [this](QTreeWidgetItem* it, int col) {
        QString layer = it->data(0, Qt::UserRole).toString();
        if (col == 0) {
            bool vis = it->data(0, Qt::UserRole + 1).toBool();
            exec("setAttr", args(QJsonArray{layer + ".visibility", !vis}));
        } else if (col == 1) {
            bool play = it->data(1, Qt::UserRole).toBool();
            exec("setAttr", args(QJsonArray{layer + ".hideOnPlayback", play}));
        } else if (col == 2) {
            // Cycles normal, template, reference as the reference's display type button.
            int t = (it->data(2, Qt::UserRole).toInt() + 1) % 3;
            exec("setAttr", args(QJsonArray{layer + ".displayType", t}));
        } else if (col == 3) {
            ColorChoice c = choose_display_color(this, "Layer Color", it->data(3, Qt::UserRole).toInt(), it->data(3, Qt::UserRole + 1).toBool(), it->data(3, Qt::UserRole + 2).value<QColor>());
            if (c.accepted) {
                exec("setAttr", args(QJsonArray{layer + ".overrideRGBColors", c.use_rgb}));
                if (c.use_rgb) {
                    exec("setAttr", QJsonObject{{"args", QJsonArray{layer + ".overrideColorRGB", c.rgb.redF(), c.rgb.greenF(), c.rgb.blueF()}}, {"type", "double3"}});
                } else {
                    exec("setAttr", args(QJsonArray{layer + ".color", c.index}));
                }
            }
        }
    });
    connect(list_, &QTreeWidget::itemDoubleClicked, this, [](QTreeWidgetItem* it, int col) {
        if (col == 4) exec("select", args(it->data(4, Qt::UserRole).toJsonArray()));
    });
    list_->setContextMenuPolicy(Qt::CustomContextMenu);
    connect(list_, &QWidget::customContextMenuRequested, this, [this](const QPoint& p) {
        QTreeWidgetItem* it = list_->itemAt(p);
        if (!it) return;
        QString layer = it->data(0, Qt::UserRole).toString();
        QJsonArray members = it->data(4, Qt::UserRole).toJsonArray();
        QMenu m;
        m.addAction("Add Selected Objects", [layer] {
            QJsonArray a{layer};
            for (const QJsonValue& s : state()["selection"].toArray()) a.append(s);
            exec("editDisplayLayerMembers", args(a));
        });
        m.addAction("Remove Selected Objects", [] { exec("removeFromDisplayLayer"); });
        m.addAction("Select Objects", [members] { exec("select", args(members)); });
        m.addSeparator();
        m.addAction("Delete Layer", [layer] { exec("deleteDisplayLayer", args(QJsonArray{layer})); });
        m.exec(list_->viewport()->mapToGlobal(p));
    });
}

void LayerEditor::refresh() {
    list_->clear();
    for (const QJsonValue& v : query("layers").toArray()) {
        QJsonObject l = v.toObject();
        auto* it = new QTreeWidgetItem(list_);
        bool vis = l["visible"].toBool();
        bool play = l.contains("playback") ? l["playback"].toBool() : true;
        int type = l["displayType"].toInt();
        it->setText(0, vis ? "V" : "");
        it->setText(1, play ? "P" : "");
        it->setText(2, type == 1 ? "T" : type == 2 ? "R" : "");
        it->setText(4, l["name"].toString());
        it->setData(0, Qt::UserRole, l["name"].toString());
        it->setData(0, Qt::UserRole + 1, vis);
        it->setData(1, Qt::UserRole, play);
        it->setData(2, Qt::UserRole, type);
        it->setData(4, Qt::UserRole, l["members"].toArray());
        for (int c = 0; c < 3; ++c) it->setTextAlignment(c, Qt::AlignCenter);
        if (l["color"].isArray()) {
            QJsonArray c = l["color"].toArray();
            it->setBackground(3, QColor::fromRgbF(c[0].toDouble(), c[1].toDouble(), c[2].toDouble()));
        }
        QJsonArray rgb = l["rgb"].toArray();
        it->setData(3, Qt::UserRole, l["colorIndex"].toInt());
        it->setData(3, Qt::UserRole + 1, l["useRgb"].toBool());
        it->setData(3, Qt::UserRole + 2, QColor::fromRgbF(rgb[0].toDouble(), rgb[1].toDouble(), rgb[2].toDouble()));
        it->setToolTip(3, "Layer color. Click to choose.");
        it->setFlags((it->flags() | Qt::ItemIsDragEnabled) & ~Qt::ItemIsDropEnabled);
        it->setToolTip(4, QString("%1 members. Double-click to select them.").arg(l["members"].toArray().size()));
    }
}

} // namespace az
