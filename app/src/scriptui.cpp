#include "scriptui.h"
#include "aztek.h"
#include "host.h"
#include "viewport.h"
#include "widgets.h"
#include "panels.h"
#include "grapheditor.h"
#include "dopesheet.h"
#include "nodeeditor.h"
#include "shapeeditor.h"
#include "editors.h"
#include "uveditor.h"

#include <QAction>
#include <QActionGroup>
#include <QApplication>
#include <QButtonGroup>
#include <QCheckBox>
#include <QCloseEvent>
#include <QColorDialog>
#include <QComboBox>
#include <QDialog>
#include <QDialogButtonBox>
#include <QDir>
#include <QDockWidget>
#include <QDoubleSpinBox>
#include <QDrag>
#include <QDragEnterEvent>
#include <QDropEvent>
#include <QMimeData>
#include <QFileDialog>
#include <QFile>
#include <QFrame>
#include <QGridLayout>
#include <QHBoxLayout>
#include <QIcon>
#include <QImage>
#include <QJsonDocument>
#include <QKeyEvent>
#include <QLabel>
#include <QLayout>
#include <QLineEdit>
#include <QListWidget>
#include <QMainWindow>
#include <QMenu>
#include <QMenuBar>
#include <QMouseEvent>
#include <QPainter>
#include <QPlainTextEdit>
#include <QProgressBar>
#include <QPushButton>
#include <QRadioButton>
#include <QResizeEvent>
#include <QScrollArea>
#include <QScrollBar>
#include <QSignalBlocker>
#include <QSlider>
#include <QSpinBox>
#include <QStyle>
#include <QTabBar>
#include <QTableWidget>
#include <QHeaderView>
#include <QTabWidget>
#include <QTemporaryDir>
#include <QTimer>
#include <QToolButton>
#include <QValidator>
#include <QVBoxLayout>
#include <cmath>

namespace az {

namespace {

// The hook the core calls for what must answer at once.
void ui_hook(const char* kind, const char* json) {
    QJsonObject p = parse(QString::fromUtf8(json)).toObject();
    ScriptUi::get()->hook(QString::fromUtf8(kind), p);
}

void answer(const QJsonValue& v) {
    QByteArray b = QJsonDocument(QJsonArray{v}).toJson(QJsonDocument::Compact);
    // Without the wrapping array, as the core reads one value.
    b = b.mid(1, b.size() - 2);
    aztek_ui_answer(b.constData());
}

double as_number(const QJsonValue& v, double fallback = 0.0) {
    if (v.isBool()) return v.toBool() ? 1.0 : 0.0;
    if (v.isString()) {
        bool ok = false;
        double d = v.toString().toDouble(&ok);
        return ok ? d : fallback;
    }
    return v.toDouble(fallback);
}

bool as_bool(const QJsonValue& v, bool fallback = false) {
    if (v.isUndefined() || v.isNull()) return fallback;
    if (v.isBool()) return v.toBool();
    if (v.isDouble()) return v.toDouble() != 0.0;
    QString s = v.toString();
    return s == "true" || s == "on" || s == "yes" || s == "1";
}

QStringList strings(const QJsonValue& v) {
    QStringList out;
    if (v.isArray()) {
        for (const QJsonValue& x : v.toArray()) {
            if (x.isArray()) out += strings(x);
            else out << (x.isDouble() ? QString::number(x.toDouble()) : x.toString());
        }
    } else if (v.isString()) {
        out << v.toString();
    }
    return out;
}

// A flat list of values whichever way it came: tuples or a run.
QJsonArray flat(const QJsonValue& v) {
    QJsonArray out;
    if (v.isArray()) {
        for (const QJsonValue& x : v.toArray()) {
            if (x.isArray())
                for (const QJsonValue& y : flat(x)) out.append(y);
            else
                out.append(x);
        }
    } else if (!v.isUndefined() && !v.isNull()) {
        out.append(v);
    }
    return out;
}

Qt::Alignment alignment(const QString& s) {
    if (s == "right") return Qt::AlignRight | Qt::AlignVCenter;
    if (s == "center" || s == "centre") return Qt::AlignHCenter | Qt::AlignVCenter;
    return Qt::AlignLeft | Qt::AlignVCenter;
}

QString short_name(const QString& path) { return path.mid(path.lastIndexOf('|') + 1); }

QColor color_of(const QJsonValue& v) {
    QJsonArray a = flat(v);
    if (a.size() < 3) return QColor();
    return QColor::fromRgbF(float(as_number(a[0])), float(as_number(a[1])), float(as_number(a[2])));
}

// Lays out children as they flow: left to right, wrapping when asked.
class FlowLayout : public QLayout {
public:
    FlowLayout(QWidget* parent, bool vertical, bool wrap, int spacing) : QLayout(parent), vertical_(vertical), wrap_(wrap), spacing_(spacing) {
        setContentsMargins(0, 0, 0, 0);
    }
    ~FlowLayout() override {
        while (QLayoutItem* i = takeAt(0)) delete i;
    }
    void addItem(QLayoutItem* i) override { items_.append(i); }
    int count() const override { return items_.size(); }
    QLayoutItem* itemAt(int i) const override { return items_.value(i); }
    QLayoutItem* takeAt(int i) override { return i >= 0 && i < items_.size() ? items_.takeAt(i) : nullptr; }
    Qt::Orientations expandingDirections() const override { return {}; }
    bool hasHeightForWidth() const override { return wrap_ && !vertical_; }
    int heightForWidth(int w) const override { return run(QRect(0, 0, w, 0), false).height(); }
    QSize sizeHint() const override { return run(QRect(0, 0, wrap_ ? 300 : 100000, wrap_ ? 300 : 100000), false); }
    QSize minimumSize() const override {
        QSize s;
        for (auto* i : items_) s = s.expandedTo(i->minimumSize());
        return s;
    }
    void setGeometry(const QRect& r) override {
        QLayout::setGeometry(r);
        run(r, true);
    }
    void set_spacing(int s) { spacing_ = s; invalidate(); }
private:
    QSize run(const QRect& r, bool apply) const {
        int x = r.x(), y = r.y(), across = 0, far_x = 0, far_y = 0;
        for (auto* i : items_) {
            if (i->isEmpty()) continue;
            QSize s = i->sizeHint();
            if (vertical_) {
                if (wrap_ && y + s.height() > r.bottom() && y > r.y()) {
                    y = r.y();
                    x += across + spacing_;
                    across = 0;
                }
                if (apply) i->setGeometry(QRect(QPoint(x, y), s));
                y += s.height() + spacing_;
                across = qMax(across, s.width());
                far_y = qMax(far_y, y - spacing_ - r.y());
                far_x = qMax(far_x, x + across - r.x());
            } else {
                if (wrap_ && x + s.width() > r.right() && x > r.x()) {
                    x = r.x();
                    y += across + spacing_;
                    across = 0;
                }
                if (apply) i->setGeometry(QRect(QPoint(x, y), s));
                x += s.width() + spacing_;
                across = qMax(across, s.height());
                far_x = qMax(far_x, x - spacing_ - r.x());
                far_y = qMax(far_y, y + across - r.y());
            }
        }
        return QSize(far_x, far_y);
    }
    QList<QLayoutItem*> items_;
    bool vertical_, wrap_;
    int spacing_;
};

// formLayout: children placed by attachments to the form, to positions and to each other.
class FormWidget : public QWidget {
public:
    explicit FormWidget(QWidget* parent) : QWidget(parent) {}
    QJsonArray attachments;
    int divisions = 100;
    QHash<QString, QWidget*> kids;
    QSize sizeHint() const override { return measure(); }
    QSize minimumSizeHint() const override { return QSize(10, 10); }
    void relayout() { solve(width(), height(), true); }
protected:
    void resizeEvent(QResizeEvent* e) override {
        QWidget::resizeEvent(e);
        relayout();
    }
private:
    struct Box { double l = NAN, r = NAN, t = NAN, b = NAN; };
    QSize measure() const {
        QSize used(10, 10);
        // Without attachments to the far edges: how much the children want.
        QHash<QString, Box> boxes = solve(0, 0, false);
        for (const QString& k : boxes.keys()) {
            const Box& b = boxes[k];
            if (!std::isnan(b.r)) used.setWidth(qMax(used.width(), int(b.r) + 5));
            if (!std::isnan(b.b)) used.setHeight(qMax(used.height(), int(b.b) + 5));
        }
        return used;
    }
    QHash<QString, Box> solve(int w, int h, bool apply) const {
        QHash<QString, Box> boxes;
        // Measuring: far edges are left to the size hints.
        bool measuring = !apply;
        for (auto it = kids.begin(); it != kids.end(); ++it) boxes[it.key()] = Box();
        // Each pass reads the last pass's boxes, so chains of controls settle.
        for (int pass = 0; pass < 8; ++pass) {
            QHash<QString, Box> cur;
            for (auto it = kids.begin(); it != kids.end(); ++it) cur[it.key()] = Box();
            for (const QJsonValue& av : attachments) {
                QJsonObject a = av.toObject();
                QString c = a["control"].toString(), e = a["edge"].toString(), kind = a["kind"].toString();
                if (!cur.contains(c) || kind == "attachNone") continue;
                Box& b = cur[c];
                double off = a["offset"].toDouble();
                double size = (e == "left" || e == "right") ? w : h;
                bool far_edge = e == "right" || e == "bottom";
                double v = NAN;
                if (kind == "attachForm") {
                    if (far_edge && measuring) continue;
                    v = far_edge ? size - off : off;
                } else if (kind == "attachOppositeForm") {
                    if (measuring) continue;
                    v = far_edge ? off : size - off;
                } else if (kind == "attachPosition") {
                    if (measuring && a["position"].toDouble() != 0.0) continue;
                    v = size * a["position"].toDouble() / qMax(1, divisions) + (far_edge ? -off : off);
                } else if (kind == "attachControl" || kind == "attachOppositeControl") {
                    QString other = a["other"].toString();
                    if (!boxes.contains(other)) continue;
                    const Box& o = boxes[other];
                    bool opposite = kind == "attachOppositeControl";
                    double got = NAN;
                    if (e == "left") got = opposite ? o.l : o.r;
                    if (e == "right") got = opposite ? o.r : o.l;
                    if (e == "top") got = opposite ? o.t : o.b;
                    if (e == "bottom") got = opposite ? o.b : o.t;
                    if (std::isnan(got)) continue;
                    if (measuring && far_edge && !opposite) continue;
                    v = far_edge ? got - off : got + off;
                }
                if (std::isnan(v)) continue;
                if (e == "left") b.l = v;
                else if (e == "right") b.r = v;
                else if (e == "top") b.t = v;
                else b.b = v;
            }
            // What a child has one edge of, its size hint finishes.
            for (auto it = kids.begin(); it != kids.end(); ++it) {
                Box& b = cur[it.key()];
                QSize s = it.value()->sizeHint();
                if (std::isnan(b.l) && std::isnan(b.r)) b.l = 0;
                if (std::isnan(b.t) && std::isnan(b.b)) b.t = 0;
                if (std::isnan(b.r)) b.r = b.l + s.width();
                else if (std::isnan(b.l)) b.l = b.r - s.width();
                if (std::isnan(b.b)) b.b = b.t + s.height();
                else if (std::isnan(b.t)) b.t = b.b - s.height();
            }
            boxes = cur;
        }
        if (apply)
            for (auto it = kids.begin(); it != kids.end(); ++it) {
                const Box& b = boxes[it.key()];
                it.value()->setGeometry(QRect(int(b.l), int(b.t), qMax(1, int(b.r - b.l)), qMax(1, int(b.b - b.t))));
            }
        return boxes;
    }
};

// A script's window.
class ScriptWindow : public QWidget {
public:
    using QWidget::QWidget;
    std::function<void()> on_close;
    std::function<void(int, int)> on_resize;
protected:
    void closeEvent(QCloseEvent* e) override {
        if (on_close) on_close();
        e->accept();
    }
    void resizeEvent(QResizeEvent* e) override {
        QWidget::resizeEvent(e);
        if (on_resize) on_resize(width(), height());
    }
};

// A workspace or dock control's place in the main window.
class ScriptDock : public QDockWidget {
public:
    using QDockWidget::QDockWidget;
    std::function<void()> on_close;
protected:
    void closeEvent(QCloseEvent* e) override {
        if (on_close) on_close();
        e->accept();
    }
};

// The cells of a paneLayout configuration: (row, column, row span, column span), pane by pane.
struct PaneCell { int r, c, rs, cs; };

QList<PaneCell> pane_cells(const QString& cfg) {
    if (cfg == "vertical2") return {{0, 0, 1, 1}, {0, 1, 1, 1}};
    if (cfg == "horizontal2") return {{0, 0, 1, 1}, {1, 0, 1, 1}};
    if (cfg == "vertical3") return {{0, 0, 1, 1}, {0, 1, 1, 1}, {0, 2, 1, 1}};
    if (cfg == "horizontal3") return {{0, 0, 1, 1}, {1, 0, 1, 1}, {2, 0, 1, 1}};
    if (cfg == "top3") return {{0, 0, 1, 2}, {1, 0, 1, 1}, {1, 1, 1, 1}};
    if (cfg == "left3") return {{0, 0, 2, 1}, {0, 1, 1, 1}, {1, 1, 1, 1}};
    if (cfg == "bottom3") return {{0, 0, 1, 1}, {0, 1, 1, 1}, {1, 0, 1, 2}};
    if (cfg == "right3") return {{0, 0, 1, 1}, {1, 0, 1, 1}, {0, 1, 2, 1}};
    if (cfg == "vertical4") return {{0, 0, 1, 1}, {0, 1, 1, 1}, {0, 2, 1, 1}, {0, 3, 1, 1}};
    if (cfg == "horizontal4") return {{0, 0, 1, 1}, {1, 0, 1, 1}, {2, 0, 1, 1}, {3, 0, 1, 1}};
    if (cfg == "top4") return {{0, 0, 1, 3}, {1, 0, 1, 1}, {1, 1, 1, 1}, {1, 2, 1, 1}};
    if (cfg == "left4") return {{0, 0, 3, 1}, {0, 1, 1, 1}, {1, 1, 1, 1}, {2, 1, 1, 1}};
    if (cfg == "bottom4") return {{0, 0, 1, 1}, {0, 1, 1, 1}, {0, 2, 1, 1}, {1, 0, 1, 3}};
    if (cfg == "right4") return {{0, 0, 1, 1}, {1, 0, 1, 1}, {2, 0, 1, 1}, {0, 1, 3, 1}};
    if (cfg == "quad") return {{0, 0, 1, 1}, {0, 1, 1, 1}, {1, 0, 1, 1}, {1, 1, 1, 1}};
    return {{0, 0, 1, 1}};
}

const char* const CONTROL_MIME = "application/x-aztek-control";

// A frame's header: the arrow when it can collapse, then its label.
QString frame_title(const QString& label, bool collapsable, bool collapsed) {
    if (!collapsable) return label;
    return QString(collapsed ? QChar(0x25B8) : QChar(0x25BE)) + "  " + label;
}

// A slider with a float range, in the integer steps Qt has.
const int SLIDER_STEPS = 10000;

} // namespace

struct ScriptUi::Item {
    QString path, kind, name;
    QWidget* w = nullptr;
    // Where children go, and the layout they go in, for containers.
    QWidget* body = nullptr;
    QLayout* layout = nullptr;
    QGridLayout* grid = nullptr;
    QMenuBar* bar = nullptr;
    QMenu* menu = nullptr;
    QAction* action = nullptr;
    QTabWidget* tabs = nullptr;
    Item* parent = nullptr;
    QList<Item*> kids;
    QJsonObject props;
    QJsonValue prop(const QString& key) const { return props.value(key); }
    int next_cell = 0;
    // A group control's parts, by role.
    QHash<QString, QWidget*> parts;
    QButtonGroup* buttons = nullptr;
    // Option menus: the combo box, and frame layouts' collapse arrow.
    QPushButton* arrow = nullptr;
    QFrame* frame = nullptr;
    // Workspace and dock controls, and a window a dock control holds.
    QPointer<QDockWidget> dock;
    // Popup menus: the button and modifiers that bring them up.
    int popup_button = 3;
};

ScriptUi* ScriptUi::get() {
    static ScriptUi* s = new ScriptUi();
    return s;
}

ScriptUi::ScriptUi() {
    aztek_ui_set_hook(&ui_hook);
    pump_timer_.start();
}

void scriptui_sync() { ScriptUi::get()->sync(); }

void ScriptUi::set_main(QMainWindow* w, std::function<ViewportWidget*(const QString&)> view_for_panel, std::function<void()> take_requests) {
    main_ = w;
    view_for_panel_ = std::move(view_for_panel);
    take_requests_ = std::move(take_requests);
    auto* it = new Item();
    it->path = it->name = "MayaWindow";
    it->kind = "mainWindow";
    it->w = w;
    it->bar = w->menuBar();
    items_.insert(it->path, it);
}

void ScriptUi::add_builtin_editor(const QString& name, const QString& kind, QWidget* w) {
    auto* it = new Item();
    it->path = it->name = name;
    it->kind = kind;
    it->w = w;
    items_.insert(name, it);
}

ViewportWidget* ScriptUi::embedded_view(const QString& panel) const {
    return embedded_.value(panel);
}

void ScriptUi::sync() {
    char* raw = aztek_ui_take_ops();
    if (!raw) return;
    QJsonArray ops = parse(take(raw)).toArray();
    for (const QJsonValue& op : ops) apply_op(op.toObject());
}

void ScriptUi::apply_op(const QJsonObject& op) {
    QString what = op["op"].toString(), path = op["path"].toString();
    if (what == "create") {
        create(op);
    } else if (what == "edit") {
        if (Item* it = items_.value(path)) edit(it, op["props"].toObject());
    } else if (what == "delete") {
        if (Item* it = items_.value(path)) remove(it);
    } else if (what == "show") {
        if (Item* it = items_.value(path)) show_window(it);
    } else if (what == "waitCursor") {
        bool on = op["on"].toBool();
        if (on != wait_cursor_) {
            if (on) QApplication::setOverrideCursor(Qt::WaitCursor);
            else QApplication::restoreOverrideCursor();
            wait_cursor_ = on;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Creating

void ScriptUi::create(const QJsonObject& op) {
    auto* it = new Item();
    it->path = op["path"].toString();
    it->kind = op["kind"].toString();
    it->name = short_name(it->path);
    QJsonObject props = op["props"].toObject();
    it->props = props;
    Item* parent = items_.value(op["parent"].toString());
    it->parent = parent;
    if (parent) parent->kids.append(it);
    items_.insert(it->path, it);

    const QString& k = it->kind;
    if (k == "window" || k == "workspaceControl" || k == "dockControl" || k == "progressWindow") {
        build_window(it, props);
    } else if (k == "menu" || k == "popupMenu") {
        build_menu(it, parent, props);
    } else if (k == "menuItem") {
        build_menu_item(it, parent, props);
        return;
    } else if (k == "radioCollection" || k == "radioMenuItemCollection") {
        return;
    } else if (k.endsWith("Layout") || k == "scriptedPanel") {
        build_layout(it, props);
        if (parent) place(parent, it);
    } else if (k.endsWith("Grp")) {
        build_group(it, props);
        if (parent) place(parent, it);
    } else {
        build_control(it, props);
        if (parent) place(parent, it);
    }
    if (it->w && it->w != main_) apply_common(it, props);
}

void ScriptUi::build_window(Item* it, const QJsonObject& props) {
    auto* win = new ScriptWindow(main_);
    Qt::WindowFlags fl = Qt::Window | Qt::WindowTitleHint | Qt::WindowSystemMenuHint | Qt::WindowCloseButtonHint;
    if (it->kind != "progressWindow") {
        if (as_bool(props["toolbox"])) fl = Qt::Tool | Qt::WindowTitleHint | Qt::WindowCloseButtonHint;
        if (as_bool(props["minimizeBox"], true)) fl |= Qt::WindowMinimizeButtonHint;
        if (as_bool(props["maximizeBox"], true)) fl |= Qt::WindowMaximizeButtonHint;
    }
    win->setWindowFlags(fl);
    win->setAttribute(Qt::WA_DeleteOnClose, false);
    QString title = props.contains("title") ? props["title"].toString() : props.contains("label") ? props["label"].toString() : it->name;
    win->setWindowTitle(title);
    auto* lay = new QVBoxLayout(win);
    lay->setContentsMargins(0, 0, 0, 0);
    lay->setSpacing(0);
    it->w = win;
    it->body = win;
    it->layout = lay;
    QString path = it->path;
    win->on_close = [this, path] {
        // The core decides what closing means; it sends back the delete.
        fire(path, QJsonObject{{"__closed", true}}, QJsonArray(), QJsonArray());
    };
    win->on_resize = [path](int w, int h) {
        QJsonObject ev{{"path", path}, {"set", QJsonObject{{"width", w}, {"height", h}}}};
        aztek_ui_event(QJsonDocument(ev).toJson(QJsonDocument::Compact).constData());
    };
    if (it->kind == "progressWindow") {
        auto* box = new QWidget(win);
        auto* v = new QVBoxLayout(box);
        v->setContentsMargins(12, 12, 12, 12);
        auto* status = new QLabel(box);
        auto* bar = new QProgressBar(box);
        bar->setMinimumWidth(320);
        auto* cancel = new QPushButton("Cancel", box);
        cancel->hide();
        connect(cancel, &QPushButton::clicked, this, [] { aztek_ui_progress_cancel(); });
        v->addWidget(status);
        v->addWidget(bar);
        v->addWidget(cancel, 0, Qt::AlignRight);
        lay->addWidget(box);
        it->parts["status"] = status;
        it->parts["bar"] = bar;
        it->parts["cancel"] = cancel;
        win->setWindowModality(Qt::ApplicationModal);
        win->on_close = [] { aztek_ui_progress_cancel(); };
        win->on_resize = nullptr;
        win->show();
    }
    if (it->kind == "workspaceControl" || it->kind == "dockControl") {
        auto* dock = new ScriptDock(title, main_);
        dock->setObjectName(it->path);
        win->setWindowFlags(Qt::Widget);
        dock->setWidget(win);
        dock->on_close = win->on_close;
        win->on_close = nullptr;
        it->dock = dock;
        QString own = it->path;
        connect(dock, &QDockWidget::topLevelChanged, this, [own](bool floating) {
            QJsonObject ev{{"path", own}, {"set", QJsonObject{{"floating", floating}}}};
            QByteArray text = QJsonDocument(ev).toJson(QJsonDocument::Compact);
            aztek_ui_event(text.constData());
        });
        main_->addDockWidget(Qt::RightDockWidgetArea, dock);
        dock->hide();
        place_dock(it, props, true);
    }
    if (as_bool(props["menuBar"])) menu_bar_of(it);
}

// A control scripts name for docking to: another script's, or one of the application's by the reference's name for it.
QDockWidget* ScriptUi::dock_named(const QString& name) const {
    if (Item* i = items_.value(name))
        if (i->dock) return i->dock;
    static const QHash<QString, QString> titles{{"Outliner", "Outliner"}, {"ChannelBoxLayerEditor", "Channel Box / Layer Editor"}, {"AttributeEditor", "Attribute Editor"}, {"ToolSettings", "Tool Settings"}};
    QString title = titles.value(name, name);
    for (QDockWidget* d : main_->findChildren<QDockWidget*>())
        if (d->windowTitle() == title) return d;
    return nullptr;
}

void ScriptUi::place_dock(Item* it, const QJsonObject& props, bool made) {
    QDockWidget* dock = it->dock;
    if (!dock) return;
    auto area_of = [](const QString& s) {
        return s == "left" ? Qt::LeftDockWidgetArea : s == "top" ? Qt::TopDockWidgetArea : s == "bottom" ? Qt::BottomDockWidgetArea : Qt::RightDockWidgetArea;
    };
    // Beside another: second comes after first in Qt's split, so left and top change them round.
    auto beside = [this, dock](QDockWidget* other, const QString& side) {
        if (!other) return;
        // Qt leaves a hidden dock in the tab group of the other: it has to be showing to split.
        dock->show();
        main_->addDockWidget(main_->dockWidgetArea(other), dock);
        Qt::Orientation o = (side == "left" || side == "right") ? Qt::Horizontal : Qt::Vertical;
        if (side == "left" || side == "top") {
            main_->splitDockWidget(other, dock, o);
            main_->splitDockWidget(dock, other, o);
        } else {
            main_->splitDockWidget(other, dock, o);
        }
    };
    bool placed = false;
    if (props.contains("label")) dock->setWindowTitle(props["label"].toString());
    if (props.contains("dockToMainWindow")) {
        QJsonArray v = flat(props["dockToMainWindow"]);
        Qt::DockWidgetArea area = area_of(v.isEmpty() ? QString() : v[0].toString());
        bool tab = v.size() > 1 && as_bool(v[1]);
        dock->setFloating(false);
        QDockWidget* neighbour = nullptr;
        for (QDockWidget* d : main_->findChildren<QDockWidget*>())
            if (d != dock && d->isVisible() && !d->isFloating() && main_->dockWidgetArea(d) == area) {
                neighbour = d;
                break;
            }
        main_->addDockWidget(area, dock);
        if (tab && neighbour) main_->tabifyDockWidget(neighbour, dock);
        placed = true;
    }
    if (props.contains("dockToControl")) {
        QJsonArray v = flat(props["dockToControl"]);
        if (v.size() >= 2) {
            dock->setFloating(false);
            beside(dock_named(v[0].toString()), v[1].toString());
            placed = true;
        }
    }
    if (props.contains("dockToPanel")) {
        QJsonArray v = flat(props["dockToPanel"]);
        if (v.size() >= 2) {
            dock->setFloating(false);
            main_->addDockWidget(area_of(v[1].toString()), dock);
            placed = true;
        }
    }
    if (props.contains("tabToControl")) {
        QJsonArray v = flat(props["tabToControl"]);
        if (!v.isEmpty()) {
            if (QDockWidget* other = dock_named(v[0].toString())) {
                dock->setFloating(false);
                main_->tabifyDockWidget(other, dock);
                placed = true;
            }
        }
    }
    if (props.contains("area")) {
        dock->setFloating(false);
        main_->addDockWidget(area_of(props["area"].toString()), dock);
        placed = true;
    }
    if (props.contains("allowedArea")) {
        Qt::DockWidgetAreas allowed;
        for (const QString& s : strings(props["allowedArea"])) {
            if (s == "all") allowed = Qt::AllDockWidgetAreas;
            else allowed |= area_of(s);
        }
        dock->setAllowedAreas(allowed);
    }
    if (props.contains("floating")) {
        dock->setFloating(as_bool(props["floating"]));
    } else if (made && !placed && it->kind == "workspaceControl") {
        dock->setFloating(true);
    }
    if (dock->isFloating() && (props.contains("initialWidth") || props.contains("initialHeight"))) {
        dock->resize(int(as_number(props["initialWidth"], dock->width())), int(as_number(props["initialHeight"], dock->height())));
    }
    // A dock control holds a window made before it.
    if (it->kind == "dockControl" && props.contains("content")) {
        if (Item* c = items_.value(props["content"].toString())) {
            if (c->w && c->w != it->w) {
                c->w->setWindowFlags(Qt::Widget);
                it->layout->addWidget(c->w);
                c->dock = dock;
                c->w->show();
            }
        }
    }
    if (!made && (placed || props.contains("floating"))) {
        dock->show();
        dock->raise();
    }
}

void ScriptUi::show_window(Item* it) {
    QWidget* w = it->w;
    if (!w) return;
    if (it->dock) {
        it->dock->show();
        it->dock->raise();
        return;
    }
    bool sized = it->props.contains("width") || it->props.contains("height");
    if (!sized || as_bool(it->prop("resizeToFitChildren"))) {
        w->adjustSize();
        if (!sized) w->resize(qMax(w->width(), 240), w->height());
    }
    if (!as_bool(it->prop("sizeable"), true)) w->setFixedSize(w->size());
    w->show();
    w->raise();
}

QMenuBar* ScriptUi::menu_bar_of(Item* it) {
    for (Item* i = it; i; i = i->parent) {
        if (i->bar) return i->bar;
        if (i->kind == "menuBarLayout" || !i->parent) {
            if (!i->layout) return nullptr;
            auto* bar = new QMenuBar(i->w);
            bar->setNativeMenuBar(false);
            i->layout->setMenuBar(bar);
            i->bar = bar;
            return bar;
        }
    }
    return nullptr;
}

void ScriptUi::build_menu(Item* it, Item* parent, const QJsonObject& props) {
    QString label = props.contains("label") ? props["label"].toString() : it->name;
    if (it->kind == "popupMenu") {
        it->menu = new QMenu(parent ? parent->w : main_);
        it->popup_button = int(as_number(props["button"], 3));
        if (parent && parent->w) {
            // Children ignore the buttons they don't use, so the press reaches the parent.
            parent->w->installEventFilter(this);
            parent->w->setProperty("azPopupOwner", true);
        }
    } else {
        QMenuBar* bar = menu_bar_of(parent ? parent : it);
        if (!bar) return;
        it->menu = bar->addMenu(label);
        it->action = it->menu->menuAction();
    }
    QString path = it->path;
    connect(it->menu, &QMenu::aboutToShow, this, [this, path] {
        fire(path, QJsonObject(), QJsonArray{"postMenuCommand"}, QJsonArray());
    });
    it->w = it->menu;
}

void ScriptUi::build_menu_item(Item* it, Item* parent, const QJsonObject& props) {
    if (!parent) return;
    QString label = props.contains("label") ? props["label"].toString() : it->name;
    // An item of an option menu is a row in its combo box.
    if (parent->kind == "optionMenu" || parent->kind == "optionMenuGrp") {
        rebuild_option_menu(parent);
        return;
    }
    QMenu* m = parent->menu;
    if (!m) return;
    if (as_bool(props["divider"])) {
        it->action = m->addSeparator();
        if (props.contains("dividerLabel")) it->action->setText(props["dividerLabel"].toString());
        return;
    }
    if (as_bool(props["subMenu"])) {
        it->menu = m->addMenu(label);
        it->action = it->menu->menuAction();
        return;
    }
    if (as_bool(props["optionBox"])) {
        // The box goes on the row of the item before it; with none to go on, it is an item of its own.
        int at = parent->kids.indexOf(it);
        Item* before = at > 0 ? parent->kids[at - 1] : nullptr;
        if (before && before->action && !before->menu && !as_bool(before->prop("divider")) && !as_bool(before->prop("optionBox"))) {
            QString main_path = before->path, box_path = it->path;
            QString text = before->action->text();
            auto* row = new OptionRow(text, QString(), [this, main_path] { fire(main_path, QJsonObject(), QJsonArray{"command"}, QJsonArray{false}); }, [this, box_path] {
                // The box closes the menu as a choice does.
                while (QWidget* p = QApplication::activePopupWidget()) p->close();
                fire(box_path, QJsonObject(), QJsonArray{"command"}, QJsonArray{false});
            }, m);
            m->insertAction(before->action, row);
            m->removeAction(before->action);
            before->action->deleteLater();
            before->action = row;
            return;
        }
        label = "Options...";
    }
    it->action = m->addAction(label);
    QString path = it->path;
    connect(it->action, &QAction::triggered, this, [this, path](bool checked) {
        auto* i = items_.value(path);
        QJsonObject set;
        bool checkable = i && i->action && i->action->isCheckable();
        if (checkable) set.insert(i->props.contains("radioButton") ? "radioButton" : "checkBox", checked);
        fire(path, set, QJsonArray{"command"}, QJsonArray{checked});
    });
    apply_menu_item(it, props);
}

void ScriptUi::apply_menu_item(Item* it, const QJsonObject& props) {
    if (it->parent && (it->parent->kind == "optionMenu" || it->parent->kind == "optionMenuGrp")) {
        rebuild_option_menu(it->parent);
        return;
    }
    QAction* a = it->action;
    if (!a) return;
    QSignalBlocker block(a);
    if (props.contains("label")) a->setText(props["label"].toString());
    if (props.contains("enable")) a->setEnabled(as_bool(props["enable"], true));
    if (props.contains("annotation")) {
        a->setToolTip(props["annotation"].toString());
        a->setStatusTip(props["annotation"].toString());
    }
    if (props.contains("checkBox")) {
        a->setCheckable(true);
        a->setChecked(as_bool(props["checkBox"]));
    }
    if (props.contains("radioButton")) {
        a->setCheckable(true);
        QString group = props["collection"].toString();
        if (group.isEmpty() && it->parent) group = it->parent->path;
        QActionGroup*& g = radio_groups_[group];
        if (!g) g = new QActionGroup(this);
        g->addAction(a);
        a->setChecked(as_bool(props["radioButton"]));
    }
    if (props.contains("boldFont") || props.contains("italicized")) {
        QFont f = a->font();
        if (props.contains("boldFont")) f.setBold(as_bool(props["boldFont"]));
        if (props.contains("italicized")) f.setItalic(as_bool(props["italicized"]));
        a->setFont(f);
    }
    if (props.contains("keyEquivalent")) {
        QString key;
        if (as_bool(props["ctrlModifier"])) key += "Ctrl+";
        if (as_bool(props["altModifier"])) key += "Alt+";
        if (as_bool(props["shiftModifier"])) key += "Shift+";
        a->setShortcut(QKeySequence(key + props["keyEquivalent"].toString().toUpper()));
    }
    if (props.contains("image")) {
        QString p = props["image"].toString();
        if (QFile::exists(p)) a->setIcon(QIcon(p));
    }
}

void ScriptUi::rebuild_option_menu(Item* it) {
    auto* combo = qobject_cast<QComboBox*>(it->parts.value("combo"));
    if (!combo) return;
    QSignalBlocker block(combo);
    int keep = combo->currentIndex();
    combo->clear();
    for (Item* k : it->kids) {
        if (k->kind != "menuItem" || as_bool(k->prop("divider"))) continue;
        combo->addItem(k->props.contains("label") ? k->prop("label").toString() : k->name);
    }
    int sel = it->props.contains("select") ? int(as_number(it->prop("select"))) - 1 : keep;
    if (sel < 0) sel = 0;
    if (sel < combo->count()) combo->setCurrentIndex(sel);
}

void ScriptUi::build_layout(Item* it, const QJsonObject& props) {
    const QString& k = it->kind;
    QWidget* w = nullptr;
    if (k == "formLayout") {
        auto* f = new FormWidget(nullptr);
        w = f;
    } else if (k == "frameLayout") {
        auto* outer = new QWidget();
        auto* v = new QVBoxLayout(outer);
        v->setContentsMargins(0, 0, 0, 0);
        v->setSpacing(0);
        auto* head = new QPushButton(outer);
        head->setObjectName("frameHeader");
        head->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        head->setStyleSheet("QPushButton#frameHeader { text-align: left; font-weight: bold; background: #5d5d5d; border: 0; padding: 3px 6px; }");
        auto* frame = new QFrame(outer);
        auto* inner = new QVBoxLayout(frame);
        inner->setContentsMargins(int(as_number(props["marginWidth"])), int(as_number(props["marginHeight"])), int(as_number(props["marginWidth"])), int(as_number(props["marginHeight"])));
        inner->setSpacing(0);
        inner->setAlignment(Qt::AlignTop);
        v->addWidget(head);
        v->addWidget(frame);
        it->w = outer;
        it->body = frame;
        it->layout = inner;
        it->arrow = head;
        it->frame = frame;
        QString path = it->path;
        connect(head, &QPushButton::clicked, this, [this, path] {
            Item* i = items_.value(path);
            if (!i || !as_bool(i->prop("collapsable"))) return;
            bool now = !as_bool(i->prop("collapse"));
            i->props.insert("collapse", now);
            i->frame->setVisible(!now);
            i->arrow->setText(frame_title(i->prop("label").toString(i->name), true, now));
            // The window closes up round a collapsed frame.
            if (QWidget* top = i->w->window()) {
                if (top != main_) QTimer::singleShot(0, top, [top] { top->resize(top->width(), top->minimumSizeHint().height() > 0 ? qMax(top->minimumSizeHint().height(), 40) : top->height()); });
            }
            fire(path, QJsonObject{{"collapse", now}}, QJsonArray{now ? "preCollapseCommand" : "preExpandCommand", now ? "collapseCommand" : "expandCommand"}, QJsonArray());
        });
        apply_layout(it, props);
        return;
    } else if (k == "scrollLayout") {
        auto* area = new QScrollArea();
        area->setWidgetResizable(true);
        area->setFrameShape(QFrame::NoFrame);
        auto* body = new QWidget();
        auto* v = new QVBoxLayout(body);
        v->setContentsMargins(0, 0, 0, 0);
        v->setSpacing(0);
        v->setAlignment(Qt::AlignTop);
        area->setWidget(body);
        it->w = area;
        it->body = body;
        it->layout = v;
        apply_layout(it, props);
        return;
    } else if (k == "tabLayout") {
        auto* tabs = new QTabWidget();
        it->tabs = tabs;
        it->w = tabs;
        it->body = tabs;
        QString path = it->path;
        connect(tabs, &QTabWidget::currentChanged, this, [this, path](int i) {
            fire(path, QJsonObject{{"selectTabIndex", i + 1}}, QJsonArray{"preSelectCommand", "selectCommand", "changeCommand"}, QJsonArray());
        });
        connect(tabs, &QTabWidget::tabBarDoubleClicked, this, [this, path](int) { fire(path, QJsonObject(), QJsonArray{"doubleClickCommand"}, QJsonArray()); });
        apply_layout(it, props);
        return;
    } else {
        w = new QWidget();
    }
    it->w = w;
    it->body = w;
    if (k == "columnLayout" || k == "scriptedPanel") {
        auto* v = new QVBoxLayout(w);
        v->setContentsMargins(0, 0, 0, 0);
        v->setSpacing(int(as_number(props["rowSpacing"])));
        // A column keeps its rows at the top: children go in before this stretch.
        v->addStretch(1);
        it->layout = v;
    } else if (k == "rowLayout" || k == "rowColumnLayout" || k == "gridLayout" || k == "paneLayout") {
        auto* g = new QGridLayout(w);
        g->setContentsMargins(0, 0, 0, 0);
        g->setHorizontalSpacing(k == "paneLayout" ? 3 : k == "gridLayout" ? 0 : 2);
        g->setVerticalSpacing(k == "paneLayout" ? 3 : 0);
        it->layout = g;
        it->grid = g;
    } else if (k == "flowLayout" || k == "shelfLayout") {
        bool wrap = k == "shelfLayout" || as_bool(props["wrap"]);
        it->layout = new FlowLayout(w, as_bool(props["vertical"]), wrap, int(as_number(props["columnSpacing"], k == "shelfLayout" ? 2 : 0)));
    } else if (k == "menuBarLayout") {
        auto* v = new QVBoxLayout(w);
        v->setContentsMargins(0, 0, 0, 0);
        v->setSpacing(0);
        it->layout = v;
    }
    apply_layout(it, props);
}

void ScriptUi::apply_layout(Item* it, const QJsonObject& props) {
    const QString& k = it->kind;
    if (k == "formLayout") {
        auto* f = static_cast<FormWidget*>(it->w);
        if (props.contains("numberOfDivisions")) f->divisions = int(as_number(props["numberOfDivisions"], 100));
        if (props.contains("attachments")) {
            f->attachments = props["attachments"].toArray();
            f->relayout();
        }
    } else if (k == "columnLayout") {
        auto* v = static_cast<QVBoxLayout*>(it->layout);
        if (props.contains("rowSpacing")) v->setSpacing(int(as_number(props["rowSpacing"])));
        QJsonArray cat = flat(props.contains("columnAttach") ? props["columnAttach"] : props["columnOffset"]);
        if (cat.size() >= 2) {
            QString side = cat[0].toString();
            int off = int(as_number(cat[1]));
            QMargins m = v->contentsMargins();
            if (side == "both" || side == "left") m.setLeft(off);
            if (side == "both" || side == "right") m.setRight(off);
            v->setContentsMargins(m);
        }
    } else if (k == "frameLayout") {
        bool collapsable = as_bool(it->prop("collapsable"));
        bool collapsed = as_bool(it->prop("collapse"));
        it->arrow->setText(frame_title(it->prop("label").toString(it->name), collapsable, collapsed));
        it->frame->setVisible(!collapsed);
        if (props.contains("labelVisible")) it->arrow->setVisible(as_bool(props["labelVisible"], true));
        if (props.contains("marginWidth") || props.contains("marginHeight")) {
            int mw = int(as_number(it->prop("marginWidth"))), mh = int(as_number(it->prop("marginHeight")));
            it->layout->setContentsMargins(mw, mh, mw, mh);
        }
    } else if (k == "tabLayout") {
        QSignalBlocker block(it->tabs);
        if (props.contains("tabLabels")) {
            QJsonObject labels = props["tabLabels"].toObject();
            for (int i = 0; i < it->kids.size(); ++i)
                if (labels.contains(it->kids[i]->path)) {
                    int at = it->tabs->indexOf(it->kids[i]->w);
                    if (at >= 0) it->tabs->setTabText(at, labels[it->kids[i]->path].toString());
                }
        }
        if (props.contains("selectTabIndex")) it->tabs->setCurrentIndex(int(as_number(props["selectTabIndex"])) - 1);
        if (props.contains("tabsVisible")) it->tabs->tabBar()->setVisible(as_bool(props["tabsVisible"], true));
    } else if (k == "paneLayout") {
        // Sizes in percent of the layout: what is not given is shared by the rest of the row or column.
        QJsonObject sizes = it->prop("paneSizes").toObject();
        if (it->grid && !sizes.isEmpty()) {
            QList<PaneCell> cells = pane_cells(it->prop("configuration").toString("single"));
            QHash<int, int> cols, rows;
            int ncols = 0, nrows = 0;
            for (const PaneCell& c : cells) {
                ncols = qMax(ncols, c.c + c.cs);
                nrows = qMax(nrows, c.r + c.rs);
            }
            for (int i = 0; i < cells.size(); ++i) {
                QJsonArray s = sizes[QString::number(i + 1)].toArray();
                if (s.size() < 2) continue;
                if (cells[i].cs == 1) cols[cells[i].c] = s[0].toInt();
                if (cells[i].rs == 1) rows[cells[i].r] = s[1].toInt();
            }
            auto spread = [](QGridLayout* g, const QHash<int, int>& given, int n, bool columns) {
                int used = 0;
                for (int v : given) used += v;
                int rest = n - given.size();
                int share = rest > 0 ? qMax(1, (100 - used) / rest) : 0;
                for (int i = 0; i < n; ++i) {
                    int v = qMax(1, given.value(i, share));
                    if (columns) g->setColumnStretch(i, v);
                    else g->setRowStretch(i, v);
                }
            };
            spread(it->grid, cols, ncols, true);
            spread(it->grid, rows, nrows, false);
        }
    } else if (it->grid && k != "paneLayout") {
        // Column widths: cw (index, width) pairs or the numbered family; widths hold, the adjustable column takes the rest.
        QJsonArray cw = flat(it->prop("columnWidth"));
        for (int i = 0; i + 1 < cw.size(); i += 2) it->grid->setColumnMinimumWidth(int(as_number(cw[i])) - 1, int(as_number(cw[i + 1])));
        if (k == "rowLayout")
            for (int n = 1; n <= 6; ++n) {
                QJsonArray w = flat(it->prop("columnWidth" + QString::number(n)));
                for (int i = 0; i < w.size(); ++i) it->grid->setColumnMinimumWidth(i, int(as_number(w[i])));
            }
        int nc = int(as_number(it->prop("numberOfColumns"), 0));
        int adj = int(as_number(it->prop("adjustableColumn"), 0));
        for (int n = 2; n <= 6; ++n)
            if (it->props.contains("adjustableColumn" + QString::number(n))) adj = int(as_number(it->prop("adjustableColumn" + QString::number(n))));
        for (int i = 0; i <= qMax(nc, 6); ++i) it->grid->setColumnStretch(i, 0);
        if (adj > 0) it->grid->setColumnStretch(adj - 1, 1);
        else if (k != "gridLayout") it->grid->setColumnStretch(qMax(nc, it->next_cell), 1);
        if (k == "rowColumnLayout" && !props.isEmpty()) {
            QJsonArray rs = flat(it->prop("rowSpacing"));
            if (rs.size() >= 2) it->grid->setVerticalSpacing(int(as_number(rs[1])));
            QJsonArray cs = flat(it->prop("columnSpacing"));
            if (cs.size() >= 2) it->grid->setHorizontalSpacing(int(as_number(cs[1])));
        }
        // Rows stay their own height, left at the top.
        it->grid->setRowStretch(qMax(99, 0), 1);
    }
}

void ScriptUi::place(Item* parent, Item* child) {
    QWidget* w = child->w;
    if (!w) return;
    const QString& k = parent->kind;
    if (k == "window" || k == "workspaceControl" || k == "dockControl") {
        parent->layout->addWidget(w);
    } else if (k == "columnLayout" || k == "scriptedPanel") {
        auto* v = static_cast<QVBoxLayout*>(parent->layout);
        int at = v->count() - 1;
        if (as_bool(parent->prop("adjustableColumn"))) {
            v->insertWidget(at, w);
        } else {
            QString al = parent->prop("columnAlign").toString("left");
            v->insertWidget(at, w, 0, al == "right" ? Qt::AlignRight : al == "center" ? Qt::AlignHCenter : Qt::AlignLeft);
            if (parent->props.contains("columnWidth")) w->setFixedWidth(int(as_number(parent->prop("columnWidth"))));
        }
    } else if (k == "rowLayout" || k == "rowColumnLayout") {
        int nc = int(as_number(parent->prop("numberOfColumns"), k == "rowLayout" ? 2 : 0));
        int nr = int(as_number(parent->prop("numberOfRows"), 0));
        QJsonArray nrc = flat(parent->prop("numberOfRowsColumns"));
        if (nrc.size() >= 2) {
            nr = int(as_number(nrc[0]));
            nc = int(as_number(nrc[1]));
        }
        int row, col;
        if (nc > 0) {
            row = parent->next_cell / nc;
            col = parent->next_cell % nc;
        } else if (nr > 0) {
            col = parent->next_cell / nr;
            row = parent->next_cell % nr;
        } else {
            row = parent->next_cell;
            col = 0;
        }
        ++parent->next_cell;
        Qt::Alignment al;
        QJsonArray cal = flat(parent->prop("columnAlign"));
        for (int i = 0; i + 1 < cal.size(); i += 2)
            if (int(as_number(cal[i])) - 1 == col) al = alignment(cal[i + 1].toString());
        for (int n = 1; n <= 6; ++n) {
            QJsonArray list = flat(parent->prop("columnAlign" + QString::number(n)));
            if (col < list.size()) al = alignment(list[col].toString());
        }
        parent->grid->addWidget(w, row, col, al);
        apply_layout(parent, QJsonObject());
    } else if (k == "gridLayout") {
        QJsonArray cwh = flat(parent->prop("cellWidthHeight"));
        int cw = cwh.size() >= 2 ? int(as_number(cwh[0])) : int(as_number(parent->prop("cellWidth"), 64));
        int ch = cwh.size() >= 2 ? int(as_number(cwh[1])) : int(as_number(parent->prop("cellHeight"), 64));
        QJsonArray nrc = flat(parent->prop("numberOfRowsColumns"));
        int nc = nrc.size() >= 2 ? int(as_number(nrc[1])) : int(as_number(parent->prop("numberOfColumns"), 1));
        nc = qMax(nc, 1);
        w->setFixedSize(cw, ch);
        parent->grid->addWidget(w, parent->next_cell / nc, parent->next_cell % nc, Qt::AlignLeft | Qt::AlignTop);
        // The cells keep their size: what is spare goes past the last column.
        parent->grid->setColumnStretch(nc, 1);
        ++parent->next_cell;
    } else if (k == "flowLayout" || k == "shelfLayout" || k == "menuBarLayout" || k == "frameLayout" || k == "scrollLayout") {
        parent->layout->addWidget(w);
        // A frame fills with an editor rather than keeping it at the top.
        const QString& ck = child->kind;
        if (k == "frameLayout" && (ck.endsWith("Editor") || ck == "keyframeOutliner" || ck == "textureWindow")) parent->layout->setAlignment(Qt::Alignment());
    } else if (k == "tabLayout") {
        QJsonObject labels = parent->prop("tabLabels").toObject();
        QSignalBlocker block(parent->tabs);
        parent->tabs->addTab(w, labels.contains(child->path) ? labels[child->path].toString() : child->name);
    } else if (k == "paneLayout") {
        QList<PaneCell> cells = pane_cells(parent->prop("configuration").toString("single"));
        if (parent->next_cell < cells.size()) {
            PaneCell c = cells[parent->next_cell];
            parent->grid->addWidget(w, c.r, c.c, c.rs, c.cs);
            for (int i = 0; i < c.rs; ++i) parent->grid->setRowStretch(c.r + i, 1);
            for (int i = 0; i < c.cs; ++i) parent->grid->setColumnStretch(c.c + i, 1);
            apply_layout(parent, QJsonObject());
        } else {
            // More than it has panes: the reference keeps them unmanaged.
            w->setParent(parent->body);
            w->hide();
        }
        ++parent->next_cell;
    } else if (k == "formLayout") {
        auto* f = static_cast<FormWidget*>(parent->w);
        w->setParent(f);
        f->kids.insert(child->path, w);
        w->show();
        f->relayout();
        f->updateGeometry();
    } else if (parent->layout) {
        parent->layout->addWidget(w);
    }
}

void ScriptUi::remove(Item* it) {
    for (Item* k : QList<Item*>(it->kids)) remove(k);
    if (it->parent) {
        it->parent->kids.removeOne(it);
        if (it->parent->kind == "formLayout") static_cast<FormWidget*>(it->parent->w)->kids.remove(it->path);
        if (it->parent->kind == "optionMenu" || it->parent->kind == "optionMenuGrp") rebuild_option_menu(it->parent);
    }
    items_.remove(it->path);
    embedded_.remove(it->name);
    if (it->dock && (it->kind == "workspaceControl" || it->kind == "dockControl")) {
        main_->removeDockWidget(it->dock);
        it->dock->hide();
        it->dock->deleteLater();
    }
    if (it->action && !it->menu) {
        it->action->deleteLater();
    } else if (it->menu) {
        if (it->action) it->action->setVisible(false);
        it->menu->deleteLater();
    } else if (it->w && it->w != main_) {
        // A callback may be deleting the control that called it: gone from view now, freed by the event loop.
        it->w->hide();
        it->w->setParent(nullptr);
        it->w->deleteLater();
    }
    delete it;
}

// ---------------------------------------------------------------------------------------------------------------------
// Controls

void ScriptUi::fire(const QString& path, const QJsonObject& set, const QJsonArray& flags, const QJsonArray& args) {
    QJsonObject ev{{"path", path}};
    QJsonObject values = set;
    if (values.contains("__closed")) {
        values.remove("__closed");
        ev.insert("closed", true);
    }
    if (!values.isEmpty()) ev.insert("set", values);
    if (!flags.isEmpty()) ev.insert("fire", flags);
    if (!args.isEmpty()) ev.insert("args", args);
    // Local copies of what the event loop can change under us: the callback may delete what called it.
    QByteArray text = QJsonDocument(ev).toJson(QJsonDocument::Compact);
    aztek_ui_event(text.constData());
    Host::get()->poke();
    sync();
}

void ScriptUi::build_control(Item* it, const QJsonObject& props) {
    const QString& k = it->kind;
    const QString path = it->path;
    QString label = props.contains("label") ? props["label"].toString() : it->name;
    if (k == "button") {
        auto* b = new QPushButton(label);
        connect(b, &QPushButton::clicked, this, [this, path](bool) { fire(path, QJsonObject(), QJsonArray{"command"}, QJsonArray{false}); });
        it->w = b;
    } else if (k == "iconTextButton" || k == "symbolButton" || k == "shelfButton") {
        auto* b = new QToolButton();
        b->setText(k == "symbolButton" ? QString() : label);
        connect(b, &QToolButton::clicked, this, [this, path](bool) { fire(path, QJsonObject(), QJsonArray{"command"}, QJsonArray{false}); });
        b->installEventFilter(this);
        it->w = b;
    } else if (k == "iconTextCheckBox" || k == "symbolCheckBox") {
        auto* b = new QToolButton();
        b->setCheckable(true);
        b->setText(k == "symbolCheckBox" ? QString() : label);
        connect(b, &QToolButton::toggled, this, [this, path](bool on) {
            fire(path, QJsonObject{{"value", on}}, QJsonArray{"changeCommand", on ? "onCommand" : "offCommand"}, QJsonArray{on});
        });
        it->w = b;
    } else if (k == "iconTextRadioButton") {
        auto* b = new QToolButton();
        b->setCheckable(true);
        b->setText(label);
        connect(b, &QToolButton::toggled, this, [this, path](bool on) {
            fire(path, QJsonObject{{"select", on}}, QJsonArray{"changeCommand", on ? "onCommand" : "offCommand"}, QJsonArray{on});
        });
        it->w = b;
    } else if (k == "text") {
        auto* l = new QLabel(label);
        it->w = l;
    } else if (k == "textField") {
        auto* e = new QLineEdit();
        connect(e, &QLineEdit::textEdited, this, [this, path](const QString& t) { fire(path, QJsonObject{{"text", t}}, QJsonArray{"textChangedCommand"}, QJsonArray{t}); });
        connect(e, &QLineEdit::editingFinished, this, [this, path, e] {
            auto* i = items_.value(path);
            if (!i) return;
            QString t = e->text();
            if (i->prop("__committed").toString() == t && i->props.contains("__committed")) return;
            i->props.insert("__committed", t);
            fire(path, QJsonObject{{"text", t}}, QJsonArray{"changeCommand"}, QJsonArray{t});
        });
        connect(e, &QLineEdit::returnPressed, this, [this, path, e] { fire(path, QJsonObject{{"text", e->text()}}, QJsonArray{"enterCommand"}, QJsonArray{e->text()}); });
        it->w = e;
    } else if (k == "scrollField") {
        auto* e = new QPlainTextEdit();
        e->installEventFilter(this);
        connect(e, &QPlainTextEdit::textChanged, this, [path, e] {
            QJsonObject ev{{"path", path}, {"set", QJsonObject{{"text", e->toPlainText()}}}};
            aztek_ui_event(QJsonDocument(ev).toJson(QJsonDocument::Compact).constData());
        });
        it->w = e;
    } else if (k == "intField") {
        auto* s = new QSpinBox();
        s->setButtonSymbols(QAbstractSpinBox::NoButtons);
        s->setRange(-2147483647, 2147483647);
        s->setKeyboardTracking(false);
        connect(s, QOverload<int>::of(&QSpinBox::valueChanged), this, [this, path](int v) { fire(path, QJsonObject{{"value", v}}, QJsonArray{"changeCommand"}, QJsonArray{v}); });
        it->w = s;
    } else if (k == "floatField") {
        auto* s = new QDoubleSpinBox();
        s->setButtonSymbols(QAbstractSpinBox::NoButtons);
        s->setRange(-1e12, 1e12);
        s->setDecimals(3);
        s->setKeyboardTracking(false);
        connect(s, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [this, path](double v) { fire(path, QJsonObject{{"value", v}}, QJsonArray{"changeCommand"}, QJsonArray{v}); });
        it->w = s;
    } else if (k == "intSlider" || k == "intScrollBar" || k == "floatSlider" || k == "floatScrollBar") {
        bool is_int = k.startsWith("int");
        bool horizontal = as_bool(props["horizontal"], true);
        auto* s = new QSlider(horizontal ? Qt::Horizontal : Qt::Vertical);
        s->setRange(0, SLIDER_STEPS);
        s->setPageStep(SLIDER_STEPS / 10);
        s->setMinimumWidth(horizontal ? 100 : 0);
        auto value_of = [path, is_int, this](int pos) -> QJsonValue {
            auto* i = items_.value(path);
            double lo = i ? as_number(i->prop("minValue"), 0) : 0, hi = i ? as_number(i->prop("maxValue"), 100) : 100;
            double v = lo + (hi - lo) * pos / SLIDER_STEPS;
            if (is_int) return int(std::round(v));
            return v;
        };
        connect(s, &QSlider::sliderMoved, this, [this, path, value_of](int pos) {
            QJsonValue v = value_of(pos);
            fire(path, QJsonObject{{"value", v}}, QJsonArray{"dragCommand"}, QJsonArray{v});
        });
        connect(s, &QSlider::sliderReleased, this, [this, path, s, value_of] {
            QJsonValue v = value_of(s->value());
            fire(path, QJsonObject{{"value", v}}, QJsonArray{"changeCommand"}, QJsonArray{v});
        });
        connect(s, &QSlider::actionTriggered, this, [this, path, s, value_of](int a) {
            // A click on the groove or a key: no drag happens, so this is the change.
            if (a == QAbstractSlider::SliderMove) return;
            QTimer::singleShot(0, this, [this, path, s, value_of] {
                if (!items_.contains(path)) return;
                QJsonValue v = value_of(s->value());
                fire(path, QJsonObject{{"value", v}}, QJsonArray{"changeCommand"}, QJsonArray{v});
            });
        });
        it->w = s;
    } else if (k == "checkBox") {
        auto* c = new QCheckBox(label);
        connect(c, &QCheckBox::toggled, this, [this, path](bool on) {
            fire(path, QJsonObject{{"value", on}}, QJsonArray{"changeCommand", on ? "onCommand" : "offCommand"}, QJsonArray{on});
        });
        it->w = c;
    } else if (k == "radioButton") {
        auto* r = new QRadioButton(label);
        // Exclusivity is the collection's, in the core.
        r->setAutoExclusive(false);
        connect(r, &QRadioButton::clicked, this, [this, path, r](bool) {
            // Clicking the one selected keeps it selected.
            auto* i = items_.value(path);
            if (i && as_bool(i->prop("select"))) {
                QSignalBlocker block(r);
                r->setChecked(true);
                return;
            }
            fire(path, QJsonObject{{"select", true}}, QJsonArray{"changeCommand", "onCommand"}, QJsonArray{true});
        });
        it->w = r;
    } else if (k == "optionMenu") {
        auto* box = new QWidget();
        auto* h = new QHBoxLayout(box);
        h->setContentsMargins(0, 0, 0, 0);
        h->setSpacing(4);
        if (props.contains("label")) {
            auto* l = new QLabel(props["label"].toString(), box);
            h->addWidget(l);
            it->parts["label"] = l;
        }
        auto* c = new QComboBox(box);
        h->addWidget(c);
        it->parts["combo"] = c;
        connect(c, QOverload<int>::of(&QComboBox::activated), this, [this, path, c](int i) {
            fire(path, QJsonObject{{"select", i + 1}}, QJsonArray{"changeCommand"}, QJsonArray{c->itemText(i)});
        });
        it->w = box;
    } else if (k == "separator") {
        auto* f = new QFrame();
        bool horizontal = as_bool(props["horizontal"], true);
        QString style = props["style"].toString("single");
        f->setFrameShape(style == "none" ? QFrame::NoFrame : horizontal ? QFrame::HLine : QFrame::VLine);
        f->setFrameShadow(QFrame::Sunken);
        it->w = f;
    } else if (k == "textScrollList") {
        auto* l = new QListWidget();
        l->setUniformItemSizes(true);
        connect(l, &QListWidget::itemSelectionChanged, this, [this, path, l] {
            QJsonArray sel;
            for (int i = 0; i < l->count(); ++i)
                if (l->item(i)->isSelected()) sel.append(i);
            fire(path, QJsonObject{{"selected", sel}}, QJsonArray{"selectCommand"}, QJsonArray());
        });
        connect(l, &QListWidget::itemDoubleClicked, this, [this, path](QListWidgetItem*) { fire(path, QJsonObject(), QJsonArray{"doubleClickCommand"}, QJsonArray()); });
        l->installEventFilter(this);
        it->w = l;
    } else if (k == "progressBar") {
        it->w = new QProgressBar();
    } else if (k == "image" || k == "picture") {
        it->w = new QLabel();
    } else if (k == "canvas") {
        auto* f = new QFrame();
        f->setAutoFillBackground(true);
        f->installEventFilter(this);
        f->setMinimumSize(20, 20);
        it->w = f;
    } else if (k == "outlinerEditor") {
        auto* o = new Outliner();
        o->setMinimumSize(120, 80);
        it->w = o;
    } else if (k == "nodeEditor") {
        auto* n = new NodeEditor();
        n->setMinimumSize(200, 120);
        it->w = n;
    } else if (k == "dopeSheetEditor") {
        auto* d = new DopeSheet();
        d->setMinimumSize(200, 120);
        d->refresh();
        it->w = d;
    } else if (k == "animCurveEditor") {
        auto* g = new GraphEditor();
        g->setMinimumSize(200, 120);
        g->refresh();
        it->w = g;
    } else if (k == "blendShapeEditor") {
        auto* b = new ShapeEditor();
        b->setMinimumSize(160, 80);
        it->w = b;
    } else if (k == "componentEditor" || k == "spreadSheetEditor") {
        QWidget* e = make_editor_window(k == "componentEditor" ? "componentEditor" : "spreadSheet", nullptr);
        if (e) {
            // A window made to stand alone, here in a layout.
            e->setWindowFlags(Qt::Widget);
            e->setMinimumSize(200, 100);
        }
        it->w = e ? e : new QWidget();
    } else if (k == "textureWindow") {
        auto* u = new UvEditor();
        u->setMinimumSize(240, 200);
        u->refresh();
        it->w = u;
    } else if (k == "poseEditor") {
        auto* l = new QListWidget();
        l->setMinimumSize(160, 80);
        auto update = [l] {
            QStringList names;
            for (const QJsonValue& v : ask("ls", QJsonObject{{"type", "poseInterpolator"}}).toArray()) names << v.toString();
            if (names.isEmpty()) names << "No pose interpolators in the scene.";
            l->clear();
            l->addItems(names);
        };
        update();
        connect(Host::get(), &Host::sceneChanged, l, update);
        it->w = l;
    } else if (k == "keyframeOutliner") {
        auto* l = new QListWidget();
        l->setMinimumSize(120, 60);
        QString curve_path = path;
        auto update = [this, l, curve_path] {
            auto* i = items_.value(curve_path);
            if (!i) return;
            QString curve = i->prop("animCurve").toString();
            l->clear();
            if (curve.isEmpty()) return;
            QJsonArray t = ask("keyframe", QJsonObject{{"args", QJsonArray{curve}}, {"query", true}, {"timeChange", true}}).toArray();
            QJsonArray v = ask("keyframe", QJsonObject{{"args", QJsonArray{curve}}, {"query", true}, {"valueChange", true}}).toArray();
            for (int n = 0; n < t.size() && n < v.size(); ++n) l->addItem(QString("%1: %2").arg(t[n].toDouble()).arg(v[n].toDouble()));
        };
        connect(Host::get(), &Host::sceneChanged, l, update);
        QTimer::singleShot(0, l, update);
        it->w = l;
    } else if (k == "toolButton") {
        auto* b = new QToolButton();
        b->setCheckable(true);
        b->setMinimumSize(32, 32);
        b->setStyleSheet("QToolButton:checked { background: #5285a6; }");
        connect(b, &QToolButton::clicked, this, [this, path, b](bool) {
            auto* i = items_.value(path);
            // Clicking the one current keeps it current.
            if (i && as_bool(i->prop("select"))) {
                QSignalBlocker block(b);
                b->setChecked(true);
                return;
            }
            fire(path, QJsonObject{{"select", true}}, QJsonArray{"changeCommand", "onCommand"}, QJsonArray{true});
        });
        b->installEventFilter(this);
        it->w = b;
    } else if (k == "layerButton") {
        auto* box = new QWidget();
        auto* h = new QHBoxLayout(box);
        h->setContentsMargins(0, 0, 0, 0);
        h->setSpacing(1);
        auto* vis = new QToolButton(box);
        vis->setCheckable(true);
        vis->setText("V");
        vis->setFixedSize(20, 20);
        auto* type = new QToolButton(box);
        type->setFixedSize(20, 20);
        auto* name = new QToolButton(box);
        name->setCheckable(true);
        name->setSizePolicy(QSizePolicy::Expanding, QSizePolicy::Fixed);
        name->setStyleSheet("QToolButton:checked { background: #5285a6; }");
        h->addWidget(vis);
        h->addWidget(type);
        h->addWidget(name, 1);
        it->parts["visible"] = vis;
        it->parts["type"] = type;
        it->parts["name"] = name;
        connect(vis, &QToolButton::clicked, this, [this, path](bool on) { fire(path, QJsonObject{{"layerVisible", on}}, QJsonArray{"visibleCommand"}, QJsonArray{on}); });
        connect(type, &QToolButton::clicked, this, [this, path](bool) {
            auto* i = items_.value(path);
            if (!i) return;
            QString now = i->prop("layerState").toString("normal");
            QString next = now == "normal" ? "template" : now == "template" ? "reference" : "normal";
            QJsonObject set{{"layerState", next}};
            i->props.insert("layerState", next);
            apply_control(i, set);
            fire(path, set, QJsonArray{"typeCommand"}, QJsonArray{next});
        });
        connect(name, &QToolButton::clicked, this, [this, path](bool on) { fire(path, QJsonObject{{"select", on}}, QJsonArray{"command", "changedCommand"}, QJsonArray{on}); });
        name->installEventFilter(this);
        it->w = box;
    } else if (k == "scriptTable") {
        auto* t = new QTableWidget();
        t->setMinimumSize(120, 60);
        connect(t, &QTableWidget::cellChanged, this, [this, path, t](int r, int c) {
            QTableWidgetItem* cell = t->item(r, c);
            QJsonObject ev{{"path", path}, {"cellEdit", QJsonArray{r + 1, c + 1, cell ? cell->text() : QString()}}};
            QByteArray text = QJsonDocument(ev).toJson(QJsonDocument::Compact);
            aztek_ui_event(text.constData());
            Host::get()->poke();
            sync();
        });
        connect(t, &QTableWidget::itemSelectionChanged, this, [this, path, t] {
            QJsonArray sel;
            for (const QModelIndex& i : t->selectionModel()->selectedIndexes()) {
                sel.append(i.row() + 1);
                sel.append(i.column() + 1);
            }
            fire(path, QJsonObject{{"selectedCells", sel}}, QJsonArray{"selectionChangedCmd"}, QJsonArray());
        });
        it->w = t;
    } else if (k == "timePort") {
        auto* t = new TimeSlider();
        t->setMinimumHeight(30);
        auto update = [t] {
            auto q = [](const char* flag) { return ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}).toDouble(); };
            t->blockSignals(true);
            t->set_range(q("minTime"), q("maxTime"));
            t->set_time(state()["time"].toDouble(t->time()));
            t->blockSignals(false);
        };
        update();
        connect(Host::get(), &Host::sceneChanged, t, update);
        connect(t, &TimeSlider::timeChanged, this, [](double f) { exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{f}}}); });
        it->w = t;
    } else if (k == "rangeControl") {
        auto* r = new RangeSlider();
        r->setMinimumHeight(20);
        auto update = [r] {
            auto q = [](const char* flag) { return ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{flag}}}).toDouble(); };
            r->set(q("animationStartTime"), q("minTime"), q("maxTime"), q("animationEndTime"));
        };
        update();
        connect(Host::get(), &Host::sceneChanged, r, update);
        connect(r, &RangeSlider::rangeChanged, this, [this, path](double a, double b) {
            exec_quiet("playbackOptions", QJsonObject{{"minTime", a}, {"maxTime", b}});
            fire(path, QJsonObject{{"minRange", a}, {"maxRange", b}}, QJsonArray{"changedCommand"}, QJsonArray());
        });
        it->w = r;
    } else if (k == "modelPanel") {
        QString cam = props["camera"].toString("persp");
        auto* v = new ViewportWidget(cam);
        v->setMinimumSize(100, 100);
        embedded_.insert(it->name, v);
        it->w = v;
    } else {
        it->w = new QWidget();
    }
    apply_control(it, props);
}

void ScriptUi::build_group(Item* it, const QJsonObject& props) {
    const QString& k = it->kind;
    const QString path = it->path;
    auto* box = new QWidget();
    auto* g = new QGridLayout(box);
    g->setContentsMargins(0, 0, 0, 0);
    g->setHorizontalSpacing(4);
    g->setVerticalSpacing(0);
    it->w = box;
    it->grid = g;
    int col = 0;
    // A label column, as most groups start with; without a label it is there only if a width asks.
    auto* label = new QLabel(props["label"].toString(), box);
    label->setAlignment(Qt::AlignRight | Qt::AlignVCenter);
    g->addWidget(label, 0, col++);
    it->parts["label"] = label;
    auto extra_label = [&](void) {
        auto* e = new QLabel(props["extraLabel"].toString(), box);
        g->addWidget(e, 0, col++);
        it->parts["extra"] = e;
    };
    if (k == "textFieldGrp" || k == "textFieldButtonGrp") {
        auto* e = new QLineEdit(box);
        g->addWidget(e, 0, col++);
        it->parts["field"] = e;
        connect(e, &QLineEdit::textEdited, this, [this, path](const QString& t) { fire(path, QJsonObject{{"text", t}}, QJsonArray{"textChangedCommand"}, QJsonArray{t}); });
        connect(e, &QLineEdit::editingFinished, this, [this, path, e] {
            auto* i = items_.value(path);
            if (!i) return;
            QString t = e->text();
            if (i->props.contains("__committed") && i->prop("__committed").toString() == t) return;
            i->props.insert("__committed", t);
            fire(path, QJsonObject{{"text", t}}, QJsonArray{"changeCommand"}, QJsonArray{t});
        });
        if (k == "textFieldButtonGrp") {
            auto* b = new QPushButton(props["buttonLabel"].toString("Button"), box);
            g->addWidget(b, 0, col++);
            it->parts["button"] = b;
            connect(b, &QPushButton::clicked, this, [this, path](bool) { fire(path, QJsonObject(), QJsonArray{"buttonCommand"}, QJsonArray()); });
        }
    } else if (k == "intFieldGrp" || k == "floatFieldGrp") {
        bool is_int = k == "intFieldGrp";
        int n = qBound(1, int(as_number(props["numberOfFields"], 1)), 4);
        // All four exist, those past the count hidden: a rebound attribute can have more or fewer.
        for (int i = 1; i <= 4; ++i) {
            QString slot = QString::number(i);
            QString val_flag = "value" + slot;
            auto changed = [this, path, val_flag, slot](const QJsonValue& v) {
                fire(path, QJsonObject{{val_flag, v}}, QJsonArray{"changeCommand", "changeCommand" + slot}, QJsonArray{v});
            };
            if (is_int) {
                auto* s = new QSpinBox(box);
                s->setButtonSymbols(QAbstractSpinBox::NoButtons);
                s->setRange(-2147483647, 2147483647);
                s->setKeyboardTracking(false);
                connect(s, QOverload<int>::of(&QSpinBox::valueChanged), this, [changed](int v) { changed(v); });
                g->addWidget(s, 0, col++);
                it->parts["field" + slot] = s;
                if (i > n) s->hide();
            } else {
                auto* s = new QDoubleSpinBox(box);
                s->setButtonSymbols(QAbstractSpinBox::NoButtons);
                s->setRange(-1e12, 1e12);
                s->setDecimals(int(as_number(props["precision"], 3)));
                s->setKeyboardTracking(false);
                connect(s, QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [changed](double v) { changed(v); });
                g->addWidget(s, 0, col++);
                it->parts["field" + slot] = s;
                if (i > n) s->hide();
            }
        }
        if (props.contains("extraLabel")) extra_label();
    } else if (k == "intSliderGrp" || k == "floatSliderGrp" || k == "floatSliderButtonGrp" || k == "colorIndexSliderGrp") {
        bool is_int = k != "floatSliderGrp" && k != "floatSliderButtonGrp";
        bool with_field = as_bool(props["field"], k == "colorIndexSliderGrp");
        QWidget* field = nullptr;
        if (with_field) {
            if (is_int) {
                auto* s = new QSpinBox(box);
                s->setButtonSymbols(QAbstractSpinBox::NoButtons);
                s->setKeyboardTracking(false);
                s->setRange(-2147483647, 2147483647);
                field = s;
            } else {
                auto* s = new QDoubleSpinBox(box);
                s->setButtonSymbols(QAbstractSpinBox::NoButtons);
                s->setKeyboardTracking(false);
                s->setRange(-1e12, 1e12);
                s->setDecimals(int(as_number(props["precision"], 3)));
                field = s;
            }
            g->addWidget(field, 0, col++);
            it->parts["field"] = field;
        }
        auto* s = new QSlider(Qt::Horizontal, box);
        s->setRange(0, SLIDER_STEPS);
        s->setPageStep(SLIDER_STEPS / 10);
        s->setMinimumWidth(60);
        g->addWidget(s, 0, col++);
        g->setColumnStretch(col - 1, 1);
        it->parts["slider"] = s;
        auto value_of = [this, path, is_int](int pos) -> QJsonValue {
            auto* i = items_.value(path);
            double lo = i ? as_number(i->prop("minValue"), 0) : 0, hi = i ? as_number(i->prop("maxValue"), 10) : 10;
            double v = lo + (hi - lo) * pos / SLIDER_STEPS;
            if (is_int) return int(std::round(v));
            return v;
        };
        auto set_field = [field, is_int](const QJsonValue& v) {
            if (!field) return;
            QSignalBlocker block(field);
            if (is_int) static_cast<QSpinBox*>(field)->setValue(v.toInt());
            else static_cast<QDoubleSpinBox*>(field)->setValue(v.toDouble());
        };
        connect(s, &QSlider::sliderMoved, this, [this, path, value_of, set_field](int pos) {
            QJsonValue v = value_of(pos);
            set_field(v);
            fire(path, QJsonObject{{"value", v}}, QJsonArray{"dragCommand"}, QJsonArray{v});
        });
        connect(s, &QSlider::sliderReleased, this, [this, path, s, value_of, set_field] {
            QJsonValue v = value_of(s->value());
            set_field(v);
            fire(path, QJsonObject{{"value", v}}, QJsonArray{"changeCommand"}, QJsonArray{v});
        });
        connect(s, &QSlider::actionTriggered, this, [this, path, s, value_of, set_field](int a) {
            if (a == QAbstractSlider::SliderMove) return;
            QTimer::singleShot(0, this, [this, path, s, value_of, set_field] {
                if (!items_.contains(path)) return;
                QJsonValue v = value_of(s->value());
                set_field(v);
                fire(path, QJsonObject{{"value", v}}, QJsonArray{"changeCommand"}, QJsonArray{v});
            });
        });
        auto field_changed = [this, path, s](double v, const QJsonValue& out) {
            auto* i = items_.value(path);
            if (!i) return;
            double lo = as_number(i->prop("minValue"), 0), hi = as_number(i->prop("maxValue"), 10);
            QSignalBlocker block(s);
            s->setValue(hi > lo ? int((v - lo) / (hi - lo) * SLIDER_STEPS) : 0);
            fire(path, QJsonObject{{"value", out}}, QJsonArray{"changeCommand"}, QJsonArray{out});
        };
        if (field && is_int) connect(static_cast<QSpinBox*>(field), QOverload<int>::of(&QSpinBox::valueChanged), this, [field_changed](int v) { field_changed(v, v); });
        else if (field) connect(static_cast<QDoubleSpinBox*>(field), QOverload<double>::of(&QDoubleSpinBox::valueChanged), this, [field_changed](double v) { field_changed(v, v); });
        if (props.contains("extraLabel")) extra_label();
        if (k == "floatSliderButtonGrp") {
            auto* b = new QPushButton(props["buttonLabel"].toString("Button"), box);
            g->addWidget(b, 0, col++);
            it->parts["button"] = b;
            connect(b, &QPushButton::clicked, this, [this, path](bool) { fire(path, QJsonObject(), QJsonArray{"buttonCommand"}, QJsonArray()); });
            if (as_bool(props["symbolButtonDisplay"])) {
                auto* sb = new QToolButton(box);
                QString image = props["image"].toString();
                if (QFile::exists(image)) sb->setIcon(QIcon(image));
                g->addWidget(sb, 0, col++);
                it->parts["symbol"] = sb;
                connect(sb, &QToolButton::clicked, this, [this, path](bool) { fire(path, QJsonObject(), QJsonArray{"symbolButtonCommand"}, QJsonArray()); });
            }
        }
    } else if (k == "colorSliderGrp") {
        auto* swatch = new QPushButton(box);
        swatch->setFixedSize(40, 20);
        g->addWidget(swatch, 0, col++);
        it->parts["swatch"] = swatch;
        auto* s = new QSlider(Qt::Horizontal, box);
        s->setRange(0, SLIDER_STEPS);
        s->setPageStep(SLIDER_STEPS / 10);
        g->addWidget(s, 0, col++);
        g->setColumnStretch(col - 1, 1);
        it->parts["slider"] = s;
        connect(swatch, &QPushButton::clicked, this, [this, path] {
            auto* i = items_.value(path);
            if (!i) return;
            QColor now = color_of(i->prop("rgbValue"));
            QColor c = QColorDialog::getColor(now.isValid() ? now : QColor(0, 0, 0), i->w, "Colour");
            if (!c.isValid()) return;
            QJsonArray rgb{c.redF(), c.greenF(), c.blueF()};
            i->props.insert("rgbValue", rgb);
            QJsonObject ch{{"rgbValue", rgb}};
            apply_control(i, ch);
            fire(path, ch, QJsonArray{"changeCommand"}, QJsonArray());
        });
        connect(s, &QSlider::sliderMoved, this, [this, path](int pos) {
            auto* i = items_.value(path);
            if (!i) return;
            QColor c = color_of(i->prop("rgbValue"));
            if (!c.isValid()) c = QColor(255, 255, 255);
            c.setHsvF(c.hsvHueF() < 0 ? 0 : c.hsvHueF(), c.hsvSaturationF(), float(pos) / SLIDER_STEPS);
            QJsonArray rgb{c.redF(), c.greenF(), c.blueF()};
            i->props.insert("rgbValue", rgb);
            QJsonObject ch{{"rgbValue", rgb}};
            apply_control(i, ch);
            fire(path, ch, QJsonArray{"dragCommand"}, QJsonArray());
        });
        connect(s, &QSlider::sliderReleased, this, [this, path] { fire(path, QJsonObject(), QJsonArray{"changeCommand"}, QJsonArray()); });
    } else if (k == "checkBoxGrp") {
        int n = qBound(1, int(as_number(props["numberOfCheckBoxes"], 1)), 4);
        for (int i = 1; i <= n; ++i) {
            QString slot = QString::number(i);
            auto* c = new QCheckBox(box);
            g->addWidget(c, 0, col++);
            it->parts["check" + slot] = c;
            connect(c, &QCheckBox::toggled, this, [this, path, slot](bool on) {
                QJsonArray flags{"changeCommand", "changeCommand" + slot, on ? "onCommand" : "offCommand", (on ? "onCommand" : "offCommand") + slot};
                fire(path, QJsonObject{{"value" + slot, on}}, flags, QJsonArray{on});
            });
        }
    } else if (k == "radioButtonGrp") {
        int n = qBound(1, int(as_number(props["numberOfRadioButtons"], 1)), 4);
        auto* group = new QButtonGroup(box);
        group->setExclusive(true);
        it->buttons = group;
        for (int i = 1; i <= n; ++i) {
            QString slot = QString::number(i);
            auto* r = new QRadioButton(box);
            group->addButton(r, i);
            g->addWidget(r, 0, col++);
            it->parts["radio" + slot] = r;
        }
        connect(group, &QButtonGroup::idClicked, this, [this, path](int id) {
            QString slot = QString::number(id);
            auto* i = items_.value(path);
            int was = i ? int(as_number(i->prop("select"))) : 0;
            QJsonArray flags{"changeCommand", "changeCommand" + slot, "onCommand", "onCommand" + slot};
            if (was > 0 && was != id) flags.append("offCommand" + QString::number(was));
            fire(path, QJsonObject{{"select", id}}, flags, QJsonArray{true});
        });
    } else if (k == "optionMenuGrp") {
        auto* c = new QComboBox(box);
        g->addWidget(c, 0, col++);
        it->parts["combo"] = c;
        connect(c, QOverload<int>::of(&QComboBox::activated), this, [this, path, c](int i) {
            fire(path, QJsonObject{{"select", i + 1}}, QJsonArray{"changeCommand"}, QJsonArray{c->itemText(i)});
        });
        if (props.contains("extraLabel")) extra_label();
    }
    apply_control(it, props);
    apply_group_columns(it);
}

// The column flags, as groups and rowLayout share them: -cw2 and family, -cw pairs, -adj, -cal and -cat.
void ScriptUi::apply_group_columns(Item* it) {
    QGridLayout* g = it->grid;
    if (!g) return;
    int columns = g->columnCount();
    int adj = int(as_number(it->prop("adjustableColumn"), 0));
    bool any_stretch = false;
    for (int n = 2; n <= 6; ++n) {
        QString key = "adjustableColumn" + QString::number(n);
        if (it->props.contains(key)) adj = int(as_number(it->prop(key)));
    }
    QList<int> widths;
    for (int i = 0; i < columns; ++i) widths << -1;
    for (int n = 1; n <= 6; ++n) {
        QJsonArray w = flat(it->prop("columnWidth" + QString::number(n)));
        for (int i = 0; i < w.size() && i < columns; ++i) widths[i] = int(as_number(w[i]));
    }
    QJsonArray cw = flat(it->prop("columnWidth"));
    for (int i = 0; i + 1 < cw.size(); i += 2) {
        int at = int(as_number(cw[i])) - 1;
        if (at >= 0 && at < columns) widths[at] = int(as_number(cw[i + 1]));
    }
    for (int i = 0; i < columns; ++i) {
        QLayoutItem* li = g->itemAtPosition(0, i);
        QWidget* w = li ? li->widget() : nullptr;
        bool stretch = adj == i + 1;
        if (stretch) any_stretch = true;
        g->setColumnStretch(i, stretch ? 1 : 0);
        if (widths[i] >= 0 && !stretch) {
            g->setColumnMinimumWidth(i, widths[i]);
            if (w) w->setFixedWidth(widths[i]);
        }
    }
    if (!any_stretch && adj == 0) {
        // Without -adj the field or slider gets what is left.
        for (int i = columns - 1; i >= 0; --i) {
            QLayoutItem* li = g->itemAtPosition(0, i);
            QWidget* w = li ? li->widget() : nullptr;
            if (qobject_cast<QSlider*>(w) || qobject_cast<QLineEdit*>(w)) {
                if (widths[i] < 0) g->setColumnStretch(i, 1);
                break;
            }
        }
    }
    // Hide the label column of a group given no label and no width for it.
    if (auto* label = qobject_cast<QLabel*>(it->parts.value("label"))) {
        bool wanted = it->props.contains("label") || widths.value(0, -1) > 0;
        label->setVisible(wanted);
    }
    auto align = [&](int col, const QString& side) {
        QLayoutItem* li = g->itemAtPosition(0, col);
        if (auto* l = li ? qobject_cast<QLabel*>(li->widget()) : nullptr) l->setAlignment(alignment(side));
    };
    QJsonArray cal = flat(it->prop("columnAlign"));
    for (int i = 0; i + 1 < cal.size(); i += 2) align(int(as_number(cal[i])) - 1, cal[i + 1].toString());
    for (int n = 1; n <= 6; ++n) {
        QJsonArray list = flat(it->prop("columnAlign" + QString::number(n)));
        for (int i = 0; i < list.size(); ++i) align(i, list[i].toString());
    }
}

// A value on a float or int field widget, whichever it is.
static void set_number(QWidget* w, const QJsonValue& v) {
    if (!w) return;
    QSignalBlocker block(w);
    if (auto* i = qobject_cast<QSpinBox*>(w)) i->setValue(int(as_number(v)));
    else if (auto* d = qobject_cast<QDoubleSpinBox*>(w)) d->setValue(as_number(v));
}

void ScriptUi::apply_control(Item* it, const QJsonObject& props) {
    const QString& k = it->kind;
    QWidget* w = it->w;
    if (k == "button" || k == "iconTextButton" || k == "shelfButton" || k == "iconTextCheckBox" || k == "iconTextRadioButton") {
        if (props.contains("label")) {
            if (auto* b = qobject_cast<QAbstractButton*>(w)) b->setText(props["label"].toString());
        }
    }
    if (k == "textureWindow") {
        if (auto* u = qobject_cast<UvEditor*>(w)) u->apply_flags(props);
    } else if (k == "outlinerEditor") {
        if (auto* o = qobject_cast<Outliner*>(w)) o->set_editor_options(props);
    } else if (k == "nodeEditor") {
        if (auto* n = qobject_cast<NodeEditor*>(w)) {
            if (as_bool(props["rootsFromSelection"])) n->graph_selection(as_bool(it->prop("upstream")) || as_bool(it->prop("downstream")) ? 2 : 1);
            if (props.contains("addNode")) n->add_nodes(strings(props["addNode"]));
            if (props.contains("rootNode")) {
                n->clear_graph();
                n->add_nodes(strings(props["rootNode"]));
            }
            n->refresh();
        }
    }
    if (k == "toolButton") {
        auto* b = static_cast<QToolButton*>(w);
        QSignalBlocker block(b);
        QString image = it->prop("image1").toString();
        if (!image.isEmpty() && QFile::exists(image)) {
            b->setIcon(QIcon(image));
            b->setIconSize(QSize(32, 32));
            b->setToolButtonStyle(Qt::ToolButtonIconOnly);
        } else {
            // No picture: the tool's name, so the button can be told from the others.
            QStringList tools = strings(it->prop("tool"));
            b->setText(tools.isEmpty() ? it->name : tools.first());
            b->setToolButtonStyle(Qt::ToolButtonTextOnly);
        }
        if (props.contains("select")) b->setChecked(as_bool(props["select"]));
    } else if (k == "layerButton") {
        auto* vis = static_cast<QToolButton*>(it->parts["visible"]);
        auto* type = static_cast<QToolButton*>(it->parts["type"]);
        auto* name = static_cast<QToolButton*>(it->parts["name"]);
        QSignalBlocker b1(vis), b2(name);
        vis->setChecked(as_bool(it->prop("layerVisible"), true));
        QString state = it->prop("layerState").toString("normal");
        type->setText(state == "template" ? "T" : state == "reference" ? "R" : "");
        name->setText(it->props.contains("label") ? it->prop("label").toString() : it->prop("name").toString(it->name));
        name->setChecked(as_bool(it->prop("select")));
        if (it->props.contains("labelWidth")) name->setFixedWidth(int(as_number(it->prop("labelWidth"))));
        QFont f = name->font();
        f.setBold(as_bool(it->prop("current")));
        name->setFont(f);
        w->setEnabled(as_bool(it->prop("enable"), true));
    } else if (k == "scriptTable") {
        auto* t = static_cast<QTableWidget*>(w);
        QSignalBlocker block(t);
        int rows = int(as_number(it->prop("rows"))), cols = int(as_number(it->prop("columns")));
        t->setRowCount(rows);
        t->setColumnCount(cols);
        QJsonArray labels = flat(it->prop("label"));
        for (int i = 0; i + 1 < labels.size(); i += 2) t->setHorizontalHeaderItem(int(as_number(labels[i])) - 1, new QTableWidgetItem(labels[i + 1].toString()));
        QJsonArray widths = flat(it->prop("columnWidth"));
        for (int i = 0; i + 1 < widths.size(); i += 2) t->setColumnWidth(int(as_number(widths[i])) - 1, int(as_number(widths[i + 1])));
        if (props.contains("rowHeight")) t->verticalHeader()->setDefaultSectionSize(int(as_number(props["rowHeight"])));
        if (props.contains("useDoubleClickEdit")) t->setEditTriggers(as_bool(props["useDoubleClickEdit"]) ? QAbstractItemView::DoubleClicked : QAbstractItemView::AllEditTriggers);
        if (props.contains("multiEditEnabled") || props.contains("selectionMode")) t->setSelectionMode(as_bool(it->prop("multiEditEnabled")) ? QAbstractItemView::ExtendedSelection : QAbstractItemView::ContiguousSelection);
        if (props.contains("cells") || props.contains("rows") || props.contains("columns")) {
            QJsonArray cells = it->prop("cells").toArray();
            for (int r = 0; r < rows; ++r) {
                QJsonArray row = r < cells.size() ? cells[r].toArray() : QJsonArray();
                for (int c = 0; c < cols; ++c) {
                    QString v = c < row.size() ? row[c].toString() : QString();
                    if (QTableWidgetItem* cell = t->item(r, c)) cell->setText(v);
                    else t->setItem(r, c, new QTableWidgetItem(v));
                }
            }
        }
    }
    if (k == "iconTextButton" || k == "symbolButton" || k == "shelfButton" || k == "iconTextCheckBox" || k == "symbolCheckBox" || k == "iconTextRadioButton") {
        auto* b = qobject_cast<QToolButton*>(w);
        if (b) {
            QString image = props.contains("image") ? props["image"].toString() : props.contains("image1") ? props["image1"].toString() : QString();
            if (!image.isEmpty() && QFile::exists(image)) {
                b->setIcon(QIcon(image));
                b->setIconSize(QSize(32, 32));
            }
            QString style = it->prop("style").toString(k == "symbolButton" ? "iconOnly" : "textOnly");
            b->setToolButtonStyle(style == "iconOnly" ? Qt::ToolButtonIconOnly : style == "iconAndTextVertical" ? Qt::ToolButtonTextUnderIcon : style == "iconAndTextHorizontal" ? Qt::ToolButtonTextBesideIcon : Qt::ToolButtonTextOnly);
            if (props.contains("value")) {
                QSignalBlocker block(b);
                b->setChecked(as_bool(props["value"]));
            }
            if (props.contains("select")) {
                QSignalBlocker block(b);
                b->setChecked(as_bool(props["select"]));
            }
        }
    } else if (k == "text") {
        auto* l = static_cast<QLabel*>(w);
        if (props.contains("label")) l->setText(props["label"].toString());
        if (props.contains("align")) l->setAlignment(alignment(props["align"].toString()));
        if (props.contains("wordWrap")) l->setWordWrap(as_bool(props["wordWrap"]));
        if (props.contains("font")) {
            QString fn = props["font"].toString();
            QFont f = l->font();
            f.setBold(fn.contains("bold", Qt::CaseInsensitive));
            f.setItalic(fn.contains("oblique", Qt::CaseInsensitive));
            if (fn.startsWith("small")) f.setPointSizeF(f.pointSizeF() * 0.85);
            if (fn.startsWith("tiny")) f.setPointSizeF(f.pointSizeF() * 0.75);
            l->setFont(f);
        }
    } else if (k == "textField") {
        auto* e = static_cast<QLineEdit*>(w);
        QSignalBlocker block(e);
        if (props.contains("text")) {
            e->setText(props["text"].toString());
            it->props.insert("__committed", props["text"].toString());
        }
        if (props.contains("placeholderText")) e->setPlaceholderText(props["placeholderText"].toString());
        if (props.contains("editable")) e->setReadOnly(!as_bool(props["editable"], true));
    } else if (k == "scrollField") {
        auto* e = static_cast<QPlainTextEdit*>(w);
        QSignalBlocker block(e);
        if (props.contains("text")) e->setPlainText(props["text"].toString());
        if (props.contains("editable")) e->setReadOnly(!as_bool(props["editable"], true));
        if (props.contains("wordWrap")) e->setLineWrapMode(as_bool(props["wordWrap"]) ? QPlainTextEdit::WidgetWidth : QPlainTextEdit::NoWrap);
    } else if (k == "intField" || k == "floatField") {
        if (auto* d = qobject_cast<QDoubleSpinBox*>(w)) {
            if (props.contains("precision")) d->setDecimals(int(as_number(props["precision"])));
            if (props.contains("step")) d->setSingleStep(as_number(props["step"]));
            QSignalBlocker block(d);
            if (props.contains("minValue")) d->setMinimum(as_number(props["minValue"]));
            if (props.contains("maxValue")) d->setMaximum(as_number(props["maxValue"]));
        }
        if (auto* i = qobject_cast<QSpinBox*>(w)) {
            QSignalBlocker block(i);
            if (props.contains("minValue")) i->setMinimum(int(as_number(props["minValue"])));
            if (props.contains("maxValue")) i->setMaximum(int(as_number(props["maxValue"])));
        }
        if (props.contains("value")) set_number(w, props["value"]);
        if (props.contains("editable")) static_cast<QAbstractSpinBox*>(w)->setReadOnly(!as_bool(props["editable"], true));
    } else if (k.endsWith("Slider") || k.endsWith("ScrollBar")) {
        auto* s = static_cast<QSlider*>(w);
        QSignalBlocker block(s);
        double lo = as_number(it->prop("minValue"), 0), hi = as_number(it->prop("maxValue"), 100), v = as_number(it->prop("value"), lo);
        s->setValue(hi > lo ? int((v - lo) / (hi - lo) * SLIDER_STEPS) : 0);
    } else if (k == "checkBox") {
        auto* c = static_cast<QCheckBox*>(w);
        QSignalBlocker block(c);
        if (props.contains("label")) c->setText(props["label"].toString());
        if (props.contains("value")) c->setChecked(as_bool(props["value"]));
    } else if (k == "radioButton") {
        auto* r = static_cast<QRadioButton*>(w);
        QSignalBlocker block(r);
        if (props.contains("label")) r->setText(props["label"].toString());
        if (props.contains("select")) r->setChecked(as_bool(props["select"]));
    } else if (k == "optionMenu") {
        if (props.contains("label")) {
            if (auto* l = qobject_cast<QLabel*>(it->parts.value("label"))) l->setText(props["label"].toString());
        }
        if (props.contains("select")) {
            auto* c = static_cast<QComboBox*>(it->parts["combo"]);
            QSignalBlocker block(c);
            c->setCurrentIndex(int(as_number(props["select"])) - 1);
        }
    } else if (k == "textScrollList") {
        auto* l = static_cast<QListWidget*>(w);
        QSignalBlocker block(l);
        if (props.contains("allowMultiSelection")) l->setSelectionMode(as_bool(props["allowMultiSelection"]) ? QAbstractItemView::ExtendedSelection : QAbstractItemView::SingleSelection);
        if (props.contains("numberOfRows")) l->setFixedHeight(int(as_number(props["numberOfRows"])) * 18 + 4);
        if (props.contains("items")) {
            l->clear();
            l->addItems(strings(props["items"]));
        }
        if (props.contains("selected")) {
            l->clearSelection();
            for (const QJsonValue& v : props["selected"].toArray())
                if (auto* i = l->item(v.toInt())) i->setSelected(true);
        }
        if (props.contains("showIndexedItem"))
            if (auto* i = l->item(int(as_number(props["showIndexedItem"])) - 1)) l->scrollToItem(i);
    } else if (k == "progressBar") {
        auto* b = static_cast<QProgressBar*>(w);
        b->setRange(int(as_number(it->prop("minValue"), 0)), int(as_number(it->prop("maxValue"), 100)));
        b->setValue(int(as_number(it->prop("progress"), 0)));
    } else if (k == "progressWindow") {
        if (auto* b = qobject_cast<QProgressBar*>(it->parts.value("bar"))) {
            b->setRange(int(as_number(it->prop("minValue"), 0)), int(as_number(it->prop("maxValue"), 100)));
            b->setValue(int(as_number(it->prop("progress"), 0)));
        }
        if (auto* s = qobject_cast<QLabel*>(it->parts.value("status"))) {
            if (props.contains("status")) s->setText(props["status"].toString());
        }
        if (props.contains("title")) w->setWindowTitle(props["title"].toString());
        if (props.contains("isInterruptable")) {
            if (QWidget* c = it->parts.value("cancel")) c->setVisible(as_bool(props["isInterruptable"]));
        }
    } else if (k == "image" || k == "picture") {
        if (props.contains("image")) {
            QPixmap px(props["image"].toString());
            static_cast<QLabel*>(w)->setPixmap(px);
        }
    } else if (k == "canvas") {
        QColor c = color_of(it->prop("rgbValue"));
        if (c.isValid()) {
            QPalette p = w->palette();
            p.setColor(QPalette::Window, c);
            w->setPalette(p);
        }
    } else if (k == "modelPanel") {
        if (props.contains("camera")) static_cast<ViewportWidget*>(w)->set("camera", "\"" + props["camera"].toString() + "\"");
    }

    // Group parts.
    if (auto* label = qobject_cast<QLabel*>(it->parts.value("label"))) {
        if (k.endsWith("Grp") && props.contains("label")) label->setText(props["label"].toString());
    }
    if (auto* extra = qobject_cast<QLabel*>(it->parts.value("extra"))) {
        if (props.contains("extraLabel")) extra->setText(props["extraLabel"].toString());
    }
    if (k == "textFieldGrp" || k == "textFieldButtonGrp") {
        auto* e = static_cast<QLineEdit*>(it->parts["field"]);
        QSignalBlocker block(e);
        if (props.contains("text")) {
            e->setText(props["text"].toString());
            it->props.insert("__committed", props["text"].toString());
        }
        if (props.contains("placeholderText")) e->setPlaceholderText(props["placeholderText"].toString());
        if (props.contains("editable")) e->setReadOnly(!as_bool(props["editable"], true));
        if (auto* b = qobject_cast<QPushButton*>(it->parts.value("button"))) {
            if (props.contains("buttonLabel")) b->setText(props["buttonLabel"].toString());
            if (props.contains("enableButton")) b->setEnabled(as_bool(props["enableButton"], true));
        }
    } else if (k == "intFieldGrp" || k == "floatFieldGrp") {
        if (props.contains("numberOfFields")) {
            int n = qBound(1, int(as_number(props["numberOfFields"], 1)), 4);
            for (int i = 1; i <= 4; ++i)
                if (QWidget* f = it->parts.value("field" + QString::number(i))) f->setVisible(i <= n);
        }
        for (int i = 1; i <= 4; ++i) {
            QString slot = QString::number(i);
            if (props.contains("value" + slot)) set_number(it->parts.value("field" + slot), props["value" + slot]);
            if (props.contains("enable" + slot))
                if (QWidget* f = it->parts.value("field" + slot)) f->setEnabled(as_bool(props["enable" + slot], true));
        }
    } else if (k == "intSliderGrp" || k == "floatSliderGrp" || k == "floatSliderButtonGrp" || k == "colorIndexSliderGrp") {
        if (auto* b = qobject_cast<QPushButton*>(it->parts.value("button"))) {
            if (props.contains("buttonLabel")) b->setText(props["buttonLabel"].toString());
        }
        double lo = as_number(it->prop("minValue"), k == "colorIndexSliderGrp" ? 1 : 0), hi = as_number(it->prop("maxValue"), k == "colorIndexSliderGrp" ? 31 : 10);
        double v = as_number(it->prop("value"), lo);
        auto* s = static_cast<QSlider*>(it->parts["slider"]);
        {
            QSignalBlocker block(s);
            s->setValue(hi > lo ? int((v - lo) / (hi - lo) * SLIDER_STEPS) : 0);
        }
        if (QWidget* f = it->parts.value("field")) {
            QSignalBlocker block(f);
            // The field may go past the slider when -fmn and -fmx say so; otherwise it holds to the range.
            double flo = it->props.contains("fieldMinValue") ? as_number(it->prop("fieldMinValue")) : lo;
            double fhi = it->props.contains("fieldMaxValue") ? as_number(it->prop("fieldMaxValue")) : hi;
            if (auto* i = qobject_cast<QSpinBox*>(f)) i->setRange(int(flo), int(fhi));
            if (auto* d = qobject_cast<QDoubleSpinBox*>(f)) {
                if (props.contains("precision")) d->setDecimals(int(as_number(props["precision"])));
                d->setRange(flo, fhi);
            }
            if (props.contains("value")) set_number(f, props["value"]);
            else if (props.contains("minValue") || props.contains("maxValue")) set_number(f, v);
        }
    } else if (k == "colorSliderGrp") {
        QColor c = color_of(it->prop("rgbValue"));
        if (!c.isValid()) c = QColor(0, 0, 0);
        it->parts["swatch"]->setStyleSheet(QString("background-color: %1; border: 1px solid #222;").arg(c.name()));
        auto* s = static_cast<QSlider*>(it->parts["slider"]);
        QSignalBlocker block(s);
        s->setValue(int(c.valueF() * SLIDER_STEPS));
    } else if (k == "checkBoxGrp") {
        for (int i = 1; i <= 4; ++i) {
            QString slot = QString::number(i);
            auto* c = qobject_cast<QCheckBox*>(it->parts.value("check" + slot));
            if (!c) continue;
            QSignalBlocker block(c);
            if (props.contains("label" + slot)) c->setText(props["label" + slot].toString());
            if (props.contains("value" + slot)) c->setChecked(as_bool(props["value" + slot]));
            if (props.contains("enable" + slot)) c->setEnabled(as_bool(props["enable" + slot], true));
        }
    } else if (k == "radioButtonGrp") {
        for (int i = 1; i <= 4; ++i) {
            QString slot = QString::number(i);
            auto* r = qobject_cast<QRadioButton*>(it->parts.value("radio" + slot));
            if (!r) continue;
            QSignalBlocker block(r);
            if (props.contains("label" + slot)) r->setText(props["label" + slot].toString());
            if (props.contains("enable" + slot)) r->setEnabled(as_bool(props["enable" + slot], true));
        }
        if (props.contains("select") && it->buttons) {
            if (auto* b = it->buttons->button(int(as_number(props["select"])))) {
                QSignalBlocker block(b);
                b->setChecked(true);
            }
        }
    } else if (k == "optionMenuGrp") {
        if (props.contains("select")) {
            auto* c = static_cast<QComboBox*>(it->parts["combo"]);
            QSignalBlocker block(c);
            c->setCurrentIndex(int(as_number(props["select"])) - 1);
        }
    }

    // A bound control says how its plug is driven, in the channel box's colours.
    if (props.contains("plugState")) {
        const QString state = props["plugState"].toString();
        QString style;
        if (state == "keyed") style = "background: #dd727a; color: #1e1e1e;";
        else if (state == "keyOnFrame") style = "background: #cd2729; color: #f0f0f0;";
        else if (state == "connected") style = "background: #f1f1a5; color: #1e1e1e;";
        else if (state == "changed") style = "background: #e3ac79; color: #1e1e1e;";
        else if (state == "muted") style = "background: #8a6a48; color: #1e1e1e;";
        else if (state == "locked") style = "background: #5c6874; color: #c8c8c8;";
        QList<QWidget*> fields;
        if (QWidget* f = it->parts.value("field")) fields << f;
        for (int i = 1; i <= 4; ++i)
            if (QWidget* f = it->parts.value("field" + QString::number(i))) fields << f;
        if (qobject_cast<QAbstractSpinBox*>(w)) fields << w;
        for (QWidget* f : fields) f->setStyleSheet(style);
    }
}

void ScriptUi::apply_common(Item* it, const QJsonObject& props) {
    QWidget* w = it->w;
    if (!w || it->menu) return;
    if (props.contains("width")) {
        int v = int(as_number(props["width"]));
        if (it->kind == "window" || it->kind == "workspaceControl") w->resize(v, w->height());
        else w->setFixedWidth(v);
    }
    if (props.contains("height")) {
        int v = int(as_number(props["height"]));
        if (it->kind == "window" || it->kind == "workspaceControl") w->resize(w->width(), v);
        else w->setFixedHeight(v);
    }
    if (props.contains("enable")) w->setEnabled(as_bool(props["enable"], true));
    if (it->kind != "window" && it->kind != "workspaceControl") {
        if (props.contains("visible")) w->setVisible(as_bool(props["visible"], true));
        if (props.contains("manage")) w->setVisible(as_bool(props["manage"], true));
    } else if (props.contains("visible")) {
        if (as_bool(props["visible"])) show_window(it);
        else if (it->dock) it->dock->hide();
        else w->hide();
    }
    if (props.contains("annotation")) w->setToolTip(props["annotation"].toString());
    if (props.contains("hasDrag") || props.contains("hasDrop")) {
        // Children ignore a middle drag only if told to: the group's parts report as the group.
        QList<QWidget*> all = w->findChildren<QWidget*>();
        all << w;
        for (QWidget* x : all) {
            x->setProperty("azDragPath", it->path);
            x->installEventFilter(this);
            if (props.contains("hasDrop") || it->props.contains("hasDrop")) x->setAcceptDrops(true);
        }
    }
    if (props.contains("title") && it->kind != "progressWindow") w->setWindowTitle(props["title"].toString());
    if (props.contains("topLeftCorner")) {
        QJsonArray a = flat(props["topLeftCorner"]);
        if (a.size() >= 2) w->move(int(as_number(a[1])), int(as_number(a[0])));
    }
    if (props.contains("backgroundColor")) {
        QColor c = color_of(props["backgroundColor"]);
        if (c.isValid()) {
            if (qobject_cast<QAbstractButton*>(w)) {
                w->setStyleSheet(QString("background-color: %1;").arg(c.name()));
            } else {
                QPalette p = w->palette();
                p.setColor(QPalette::Window, c);
                p.setColor(QPalette::Base, c);
                w->setPalette(p);
                w->setAutoFillBackground(true);
            }
        }
    }
}

void ScriptUi::edit(Item* it, const QJsonObject& props) {
    for (auto i = props.begin(); i != props.end(); ++i) it->props.insert(i.key(), i.value());
    const QString& k = it->kind;
    if (k == "menuItem") {
        apply_menu_item(it, props);
        return;
    }
    if (k == "menu" || k == "popupMenu") {
        if (it->menu && props.contains("label")) it->menu->setTitle(props["label"].toString());
        if (props.contains("button")) it->popup_button = int(as_number(props["button"], 3));
        return;
    }
    if (k == "workspaceControl" || k == "dockControl") {
        place_dock(it, props, false);
    } else if (k.endsWith("Layout") || k == "scriptedPanel") {
        apply_layout(it, props);
    } else {
        apply_control(it, props);
        if (k.endsWith("Grp")) apply_group_columns(it);
    }
    if (k == "optionMenu" || k == "optionMenuGrp") rebuild_option_menu(it);
    apply_common(it, props);
}

// ---------------------------------------------------------------------------------------------------------------------
// Events on controls

bool ScriptUi::eventFilter(QObject* o, QEvent* e) {
    auto* w = qobject_cast<QWidget*>(o);
    if (!w) return false;
    // Drags between controls: the middle button, as in the reference.
    const QString drag_path = w->property("azDragPath").toString();
    if (!drag_path.isEmpty()) {
        Item* di = items_.value(drag_path);
        if (e->type() == QEvent::MouseButtonPress && static_cast<QMouseEvent*>(e)->button() == Qt::MiddleButton) {
            drag_from_ = static_cast<QMouseEvent*>(e)->globalPosition().toPoint();
            if (di && di->props.contains("hasDrag")) return true;
        } else if (e->type() == QEvent::MouseMove && (static_cast<QMouseEvent*>(e)->buttons() & Qt::MiddleButton) && di && di->props.contains("hasDrag")) {
            auto* me = static_cast<QMouseEvent*>(e);
            if ((me->globalPosition().toPoint() - drag_from_).manhattanLength() > QApplication::startDragDistance()) {
                int mods = (me->modifiers() & Qt::ShiftModifier ? 1 : 0) | (me->modifiers() & Qt::ControlModifier ? 4 : 0) | (me->modifiers() & Qt::AltModifier ? 8 : 0);
                QPoint at = me->position().toPoint();
                QJsonObject ask_it{{"path", drag_path}, {"x", at.x()}, {"y", at.y()}, {"modifiers", mods}};
                QJsonValue msgs = query("uiDrag", QString::fromUtf8(QJsonDocument(ask_it).toJson(QJsonDocument::Compact)));
                QJsonObject data{{"path", drag_path}, {"messages", msgs}};
                auto* mime = new QMimeData();
                mime->setData(CONTROL_MIME, QJsonDocument(data).toJson(QJsonDocument::Compact));
                auto* drag = new QDrag(w);
                drag->setMimeData(mime);
                drag->exec(Qt::CopyAction);
                return true;
            }
        } else if (e->type() == QEvent::DragEnter || e->type() == QEvent::DragMove) {
            auto* de = static_cast<QDropEvent*>(e);
            if (de->mimeData()->hasFormat(CONTROL_MIME) && di && di->props.contains("hasDrop")) {
                static_cast<QDragMoveEvent*>(e)->acceptProposedAction();
                return true;
            }
        } else if (e->type() == QEvent::Drop) {
            auto* de = static_cast<QDropEvent*>(e);
            if (de->mimeData()->hasFormat(CONTROL_MIME) && di && di->props.contains("hasDrop")) {
                QJsonObject data = QJsonDocument::fromJson(de->mimeData()->data(CONTROL_MIME)).object();
                QPoint at = de->position().toPoint();
                QJsonObject ev{{"path", drag_path}, {"dropFrom", data["path"]}, {"messages", data["messages"]}, {"x", at.x()}, {"y", at.y()}, {"dragType", 1}};
                de->acceptProposedAction();
                QByteArray text = QJsonDocument(ev).toJson(QJsonDocument::Compact);
                aztek_ui_event(text.constData());
                Host::get()->poke();
                sync();
                return true;
            }
        }
    }
    // Popup menus on what they are parented to.
    if (e->type() == QEvent::MouseButtonPress && w->property("azPopupOwner").toBool()) {
        auto* me = static_cast<QMouseEvent*>(e);
        for (Item* it : items_) {
            if (it->w != w) continue;
            for (Item* k : it->kids) {
                if (k->kind != "popupMenu" || !k->menu) continue;
                int want = k->popup_button;
                int got = me->button() == Qt::LeftButton ? 1 : me->button() == Qt::MiddleButton ? 2 : me->button() == Qt::RightButton ? 3 : 0;
                bool mods = as_bool(k->prop("ctrlModifier")) == bool(me->modifiers() & Qt::ControlModifier) && as_bool(k->prop("altModifier")) == bool(me->modifiers() & Qt::AltModifier) && as_bool(k->prop("shiftModifier")) == bool(me->modifiers() & Qt::ShiftModifier);
                if ((want == 0 || want == got) && mods) {
                    k->menu->popup(me->globalPosition().toPoint());
                    return true;
                }
            }
            break;
        }
    }
    if (e->type() == QEvent::MouseButtonDblClick && qobject_cast<QToolButton*>(w)) {
        for (Item* it : items_)
            if (it->w == w || (it->kind == "layerButton" && it->parts.value("name") == w)) {
                fire(it->path, QJsonObject(), QJsonArray{"doubleClickCommand"}, QJsonArray());
                break;
            }
    }
    if (e->type() == QEvent::MouseButtonPress && qobject_cast<QFrame*>(w)) {
        for (Item* it : items_)
            if (it->w == w && it->kind == "canvas") {
                fire(it->path, QJsonObject(), QJsonArray{"pressCommand"}, QJsonArray());
                break;
            }
    }
    if (e->type() == QEvent::KeyPress) {
        auto* ke = static_cast<QKeyEvent*>(e);
        for (Item* it : items_) {
            if (it->w != w) continue;
            if (it->kind == "textScrollList" && (ke->key() == Qt::Key_Delete || ke->key() == Qt::Key_Backspace)) {
                fire(it->path, QJsonObject(), QJsonArray{"deleteKeyCommand"}, QJsonArray());
                return true;
            }
            if (it->kind == "scrollField") {
                if ((ke->key() == Qt::Key_Enter) || (ke->key() == Qt::Key_Return && (ke->modifiers() & Qt::ControlModifier))) {
                    fire(it->path, QJsonObject(), QJsonArray{"enterCommand"}, QJsonArray{static_cast<QPlainTextEdit*>(w)->toPlainText()});
                    return true;
                }
            }
            break;
        }
    }
    if (e->type() == QEvent::FocusOut && qobject_cast<QPlainTextEdit*>(w)) {
        for (Item* it : items_)
            if (it->w == w) {
                fire(it->path, QJsonObject(), QJsonArray{"changeCommand"}, QJsonArray{static_cast<QPlainTextEdit*>(w)->toPlainText()});
                break;
            }
    }
    return false;
}

void ScriptUi::show_in_view_message(const QJsonObject& o) {
    if (o["clear"].toBool()) {
        if (in_view_) in_view_->deleteLater();
        return;
    }
    if (!main_) return;
    if (in_view_) in_view_->deleteLater();
    // Its own small window: a child of the main window would sit under the views, which are native.
    auto* l = new QLabel(main_, Qt::ToolTip | Qt::FramelessWindowHint);
    l->setAttribute(Qt::WA_ShowWithoutActivating);
    l->setText(o["message"].toString());
    l->setStyleSheet("background: rgba(30, 30, 30, 200); color: #e6e6e6; padding: 8px 14px; border-radius: 4px;");
    l->setAttribute(Qt::WA_TransparentForMouseEvents);
    l->adjustSize();
    QString pos = o["position"].toString("topCenter");
    int x = (main_->width() - l->width()) / 2, y = main_->height() / 2 - l->height() / 2;
    if (pos.startsWith("top")) y = 110;
    if (pos.startsWith("bot")) y = main_->height() - 150;
    if (pos.endsWith("Left")) x = 60;
    if (pos.endsWith("Right")) x = main_->width() - l->width() - 60;
    l->move(main_->mapToGlobal(QPoint(x, y)));
    l->show();
    l->raise();
    in_view_ = l;
    double time = o["time"].toDouble(1.0);
    QTimer::singleShot(int(time * 1000), l, [l] { l->deleteLater(); });
}

// ---------------------------------------------------------------------------------------------------------------------
// What must answer at once

void ScriptUi::hook(const QString& kind, const QJsonObject& payload) {
    // What the script built up to here is on screen first.
    sync();
    if (kind == "confirmDialog") confirm_dialog(payload);
    else if (kind == "promptDialog") prompt_dialog(payload);
    else if (kind == "fileDialog2") file_dialog2(payload);
    else if (kind == "fileDialog") file_dialog(payload);
    else if (kind == "playblast") {
        // What the script set on the views just before this has to show in the frames.
        if (take_requests_) take_requests_();
        playblast(payload);
    }
    else if (kind == "pump") pump();
}

void ScriptUi::pump() {
    // Long loops report often: redraw at a human rate, and take input only for a Cancel button.
    if (pump_timer_.elapsed() < 30) return;
    pump_timer_.restart();
    QApplication::processEvents();
    sync();
}

// The button row the two dialogs share. Pressing one ends the dialog with its label.
static void add_dialog_buttons(QDialog* d, QLayout* into, const QJsonObject& p, QString* picked) {
    QStringList names = strings(p["button"]);
    if (names.isEmpty()) names << "Confirm";
    auto* row = new QHBoxLayout();
    row->addStretch(1);
    QString def = p["defaultButton"].toString();
    for (const QString& n : names) {
        auto* b = new QPushButton(n, d);
        b->setAutoDefault(false);
        if (n == def) b->setDefault(true);
        QObject::connect(b, &QPushButton::clicked, d, [d, n, picked] {
            *picked = n;
            d->accept();
        });
        row->addWidget(b);
    }
    static_cast<QBoxLayout*>(into)->addLayout(row);
}

void ScriptUi::confirm_dialog(const QJsonObject& p) {
    QDialog d(QApplication::activeWindow() ? QApplication::activeWindow() : main_);
    d.setWindowTitle(p["title"].toString("Confirm"));
    auto* v = new QVBoxLayout(&d);
    auto* m = new QLabel(p["message"].toString("Are you sure?"), &d);
    m->setWordWrap(true);
    m->setAlignment(alignment(p["messageAlign"].toString("center")));
    m->setMinimumWidth(240);
    v->addWidget(m);
    QString picked;
    add_dialog_buttons(&d, v, p, &picked);
    if (d.exec() != QDialog::Accepted) {
        picked = p.contains("cancelButton") ? p["cancelButton"].toString() : p["dismissString"].toString("dismiss");
    }
    answer(picked);
}

void ScriptUi::prompt_dialog(const QJsonObject& p) {
    QDialog d(QApplication::activeWindow() ? QApplication::activeWindow() : main_);
    d.setWindowTitle(p["title"].toString("Prompt"));
    auto* v = new QVBoxLayout(&d);
    if (p.contains("message")) v->addWidget(new QLabel(p["message"].toString(), &d));
    QLineEdit* line = nullptr;
    QPlainTextEdit* area = nullptr;
    if (as_bool(p["scrollableField"])) {
        area = new QPlainTextEdit(p["text"].toString(), &d);
        v->addWidget(area);
    } else {
        line = new QLineEdit(p["text"].toString(), &d);
        line->setMinimumWidth(260);
        QString style = p["style"].toString();
        if (style == "integer") line->setValidator(new QIntValidator(line));
        if (style == "float") line->setValidator(new QDoubleValidator(line));
        v->addWidget(line);
    }
    QString picked;
    add_dialog_buttons(&d, v, p, &picked);
    if (d.exec() != QDialog::Accepted) {
        picked = p.contains("cancelButton") ? p["cancelButton"].toString() : p["dismissString"].toString("dismiss");
    }
    answer(QJsonObject{{"button", picked}, {"text", line ? line->text() : area->toPlainText()}});
}

void ScriptUi::file_dialog2(const QJsonObject& p) {
    QFileDialog d(QApplication::activeWindow() ? QApplication::activeWindow() : main_, p["caption"].toString("Choose"), p["startingDirectory"].toString(), p["fileFilter"].toString());
    d.setOption(QFileDialog::DontUseNativeDialog, true);
    int mode = int(as_number(p["fileMode"], 1));
    switch (mode) {
    case 0:
        d.setFileMode(QFileDialog::AnyFile);
        d.setAcceptMode(QFileDialog::AcceptSave);
        break;
    case 2:
        d.setFileMode(QFileDialog::Directory);
        break;
    case 3:
        d.setFileMode(QFileDialog::Directory);
        d.setOption(QFileDialog::ShowDirsOnly, true);
        break;
    case 4:
        d.setFileMode(QFileDialog::ExistingFiles);
        break;
    default:
        d.setFileMode(QFileDialog::ExistingFile);
    }
    if (p.contains("okCaption")) d.setLabelText(QFileDialog::Accept, p["okCaption"].toString());
    if (p.contains("cancelCaption")) d.setLabelText(QFileDialog::Reject, p["cancelCaption"].toString());
    if (p.contains("selectFileFilter")) d.selectNameFilter(p["selectFileFilter"].toString());
    QJsonArray out;
    if (d.exec() == QDialog::Accepted)
        for (const QString& f : d.selectedFiles()) out.append(f);
    answer(out);
}

void ScriptUi::file_dialog(const QJsonObject& p) {
    QString mask = p["directoryMask"].toString();
    QWidget* parent = QApplication::activeWindow() ? QApplication::activeWindow() : main_;
    QString dir = mask.contains('/') ? mask.left(mask.lastIndexOf('/')) : QString();
    QString filter = mask.contains('/') ? mask.mid(mask.lastIndexOf('/') + 1) : mask;
    QString f;
    if (int(as_number(p["mode"])) == 1) f = QFileDialog::getSaveFileName(parent, p["title"].toString("Save"), p["defaultFileName"].toString(dir), filter);
    else f = QFileDialog::getOpenFileName(parent, p["title"].toString("Open"), dir, filter);
    answer(f.isEmpty() ? QJsonArray() : QJsonArray{f});
}

void ScriptUi::playblast(const QJsonObject& p) {
    QString panel = p["panel"].toString();
    ViewportWidget* v = embedded_.value(panel);
    if (!v && view_for_panel_) v = view_for_panel_(panel);
    if (!v) {
        log(QString("// Warning: playblast: no view for %1 //").arg(panel), 4);
        answer(QJsonArray());
        return;
    }
    QTemporaryDir tmp;
    QString ppm = tmp.path() + "/frame.ppm";
    QJsonArray frames = p["frames"].toArray(), files = p["files"].toArray();
    int width = p["width"].toInt(), height = p["height"].toInt();
    double percent = p["percent"].toDouble(50.0);
    int quality = p["quality"].toInt(90);
    QJsonArray written;
    for (int i = 0; i < frames.size() && i < files.size(); ++i) {
        exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{frames[i].toDouble()}}});
        if (!v->capture(ppm)) continue;
        QImage img(ppm);
        if (img.isNull()) continue;
        QSize out(width > 0 ? width : img.width(), height > 0 ? height : img.height());
        // Percent scales what -wh or the view gave, as the reference's does.
        out = QSize(qMax(1, int(out.width() * percent / 100.0)), qMax(1, int(out.height() * percent / 100.0)));
        if (out != img.size()) img = img.scaled(out, Qt::IgnoreAspectRatio, Qt::SmoothTransformation);
        QString path = files[i].toString();
        QDir().mkpath(path.left(path.lastIndexOf('/')));
        if (img.save(path, nullptr, quality)) written.append(path);
        else log(QString("// Warning: playblast: could not write %1 //").arg(path), 4);
        QApplication::processEvents(QEventLoop::ExcludeUserInputEvents);
    }
    answer(written);
}

} // namespace az
