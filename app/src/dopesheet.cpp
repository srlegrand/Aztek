#include "dopesheet.h"
#include "host.h"

#include <QHBoxLayout>
#include <QHeaderView>
#include <QKeyEvent>
#include <QLabel>
#include <QListWidget>
#include <QMouseEvent>
#include <QPainter>
#include <QPushButton>
#include <QSplitter>
#include <QTreeWidget>
#include <QVBoxLayout>
#include <QWheelEvent>
#include <cmath>

namespace az {

DopeCanvas::DopeCanvas(QWidget* parent) : QWidget(parent) {
    setFocusPolicy(Qt::StrongFocus);
    setMouseTracking(false);
    setMinimumWidth(200);
}

void DopeCanvas::set_data(const QVector<DopeRow>& rows, const QHash<QString, QVector<QPair<double, bool>>>& keys, double time) {
    rows_ = rows;
    keys_ = keys;
    time_ = time;
    if (!framed_ && !keys.isEmpty()) frame_all();
    update();
}

void DopeCanvas::frame_all() {
    double lo = 1e18, hi = -1e18;
    for (const auto& v : keys_)
        for (const auto& k : v) {
            lo = qMin(lo, k.first);
            hi = qMax(hi, k.first);
        }
    if (lo > hi) return;
    double span = qMax(10.0, hi - lo);
    tpp_ = span * 1.1 / qMax(50, width());
    t0_ = lo - span * 0.05;
    framed_ = true;
    update();
}

int DopeCanvas::row_at(double y) const {
    return int(std::floor((y - 24 + scroll_) / row_h_));
}

void DopeCanvas::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.fillRect(rect(), QColor(58, 58, 58));
    // Frame ruler along the top; a line per frame when there's room.
    int step = 1;
    for (int s : {1, 2, 5, 10, 20, 50, 100, 200, 500, 1000})
        if (s / tpp_ >= 40) {
            step = s;
            break;
        }
    int first = int(std::floor(t0_ / step)) * step;
    p.setPen(QColor(150, 150, 150));
    for (int fr = first; x_of(fr) < width(); fr += step) {
        double x = x_of(fr);
        p.setPen(QColor(72, 72, 72));
        p.drawLine(QPointF(x, 24), QPointF(x, height()));
        p.setPen(QColor(190, 190, 190));
        p.drawText(QPointF(x + 2, 15), QString::number(fr));
    }
    p.setPen(QColor(90, 90, 90));
    p.drawLine(0, 24, width(), 24);
    for (int i = 0; i < rows_.size(); ++i) {
        double y = 24 + i * row_h_ - scroll_;
        if (y + row_h_ < 24 || y > height()) continue;
        const DopeRow& r = rows_[i];
        p.fillRect(QRectF(0, y, width(), row_h_ - 1), r.depth == 0 ? QColor(70, 70, 70) : r.is_channel ? QColor(60, 60, 60) : QColor(66, 66, 66));
        // A row shows the union of its curves' keys; a key is selected only if it's selected on every curve at that frame.
        QHash<double, QPair<bool, bool>> shown;
        for (const QString& c : r.curves)
            for (const auto& k : keys_.value(c)) {
                auto it = shown.find(k.first);
                bool s = sel_.contains({c, k.first});
                if (it == shown.end()) shown[k.first] = {s, k.second};
                else it->first = it->first && s;
            }
        double fw = qMax(2.0, 1.0 / tpp_ * 0.6);
        for (auto it = shown.begin(); it != shown.end(); ++it) {
            double t = it.key() + (it.value().first && mode_ == Mode::Move ? drag_frames_ : 0);
            double x = x_of(t);
            QColor col = it.value().first ? QColor(250, 230, 60) : it.value().second ? QColor(60, 190, 80) : r.depth == 0 ? QColor(210, 210, 210) : QColor(30, 30, 30);
            p.fillRect(QRectF(x - fw / 2, y + 2, fw, row_h_ - 5), col);
        }
    }
    double x = x_of(time_);
    p.fillRect(QRectF(x - 1, 0, 2, height()), QColor(220, 220, 80));
    if (mode_ == Mode::Marquee) {
        p.setPen(QPen(Qt::white, 1, Qt::DashLine));
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(press_, last_));
    }
}

void DopeCanvas::select_in(QRectF rect, bool add) {
    if (!add) sel_.clear();
    rect = rect.normalized();
    for (int i = 0; i < rows_.size(); ++i) {
        double y = 24 + i * row_h_ - scroll_;
        if (y + row_h_ < rect.top() || y > rect.bottom()) continue;
        for (const QString& c : rows_[i].curves)
            for (const auto& k : keys_.value(c)) {
                double x = x_of(k.first);
                double fw = qMax(4.0, 1.0 / tpp_ * 0.6);
                if (x + fw / 2 >= rect.left() && x - fw / 2 <= rect.right()) sel_.insert({c, k.first});
            }
    }
}

QJsonArray DopeCanvas::selected_refs() const {
    QJsonArray out;
    for (const auto& s : sel_) {
        const auto keys = keys_.value(s.first);
        for (int i = 0; i < keys.size(); ++i)
            if (keys[i].first == s.second) out.append(QJsonArray{s.first, i});
    }
    return out;
}

// What an animation curve drives, as setKeyframe takes it.
static QJsonArray plugs_of_curve(const QString& curve) {
    return ask("listConnections", QJsonObject{{"args", QJsonArray{curve}}, {"plugs", true}, {"destination", true}, {"source", false}}).toArray();
}

static QString g_dope_tool = "select";

void DopeCanvas::set_tool(const QString& t) { g_dope_tool = t; }

void DopeCanvas::mousePressEvent(QMouseEvent* e) {
    setFocus();
    press_ = last_ = e->position();
    drag_frames_ = 0;
    if (e->modifiers() & Qt::AltModifier) {
        mode_ = e->button() == Qt::RightButton ? Mode::Zoom : Mode::Pan;
        return;
    }
    if (e->button() == Qt::LeftButton) {
        if (g_dope_tool == "dolly") {
            mode_ = Mode::Zoom;
            return;
        }
        if (g_dope_tool == "track") {
            mode_ = Mode::Pan;
            return;
        }
        if (g_dope_tool == "time") {
            mode_ = Mode::Scrub;
            exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{std::round(t_of(e->position().x()))}}});
            Host::get()->poke();
            return;
        }
        if (g_dope_tool == "move" && !sel_.isEmpty()) {
            mode_ = Mode::Move;
            return;
        }
        if (g_dope_tool == "set" || g_dope_tool == "insert") {
            int r = row_at(e->position().y());
            if (r >= 0 && r < rows_.size()) {
                QJsonArray curves;
                for (const QString& c : rows_[r].curves)
                    for (const QJsonValue& p : plugs_of_curve(c)) curves.append(p);
                if (!curves.isEmpty()) {
                    exec("setKeyframe", QJsonObject{{"args", curves}, {"time", std::round(t_of(e->position().x()))}, {"insert", true}});
                    emit edited();
                }
            }
            return;
        }
    }
    if (e->position().y() < 24) {
        mode_ = Mode::Scrub;
        exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{std::round(t_of(e->position().x()))}}});
        Host::get()->poke();
        return;
    }
    if (e->button() == Qt::MiddleButton) {
        if (!sel_.isEmpty()) mode_ = Mode::Move;
        return;
    }
    // A press on a selected key drags the selection.
    if (e->button() == Qt::LeftButton) {
        int r = row_at(e->position().y());
        if (r >= 0 && r < rows_.size()) {
            for (const QString& c : rows_[r].curves)
                for (const auto& k : keys_.value(c))
                    if (std::abs(x_of(k.first) - e->position().x()) < qMax(4.0, 0.5 / tpp_) && sel_.contains({c, k.first})) {
                        mode_ = Mode::Move;
                        return;
                    }
        }
        mode_ = Mode::Marquee;
    }
}

void DopeCanvas::mouseMoveEvent(QMouseEvent* e) {
    QPointF d = e->position() - last_;
    switch (mode_) {
    case Mode::Pan:
        t0_ -= d.x() * tpp_;
        scroll_ = qMax(0, scroll_ - int(d.y()));
        break;
    case Mode::Zoom: {
        double f = std::exp(-d.x() * 0.005);
        double t = t_of(press_.x());
        tpp_ *= f;
        t0_ = t - press_.x() * tpp_;
        break;
    }
    case Mode::Move:
        drag_frames_ = std::round((e->position().x() - press_.x()) * tpp_);
        break;
    case Mode::Scrub:
        exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{std::round(t_of(e->position().x()))}}});
        Host::get()->poke();
        break;
    default:
        break;
    }
    last_ = e->position();
    update();
}

void DopeCanvas::mouseReleaseEvent(QMouseEvent* e) {
    if (mode_ == Mode::Marquee) {
        QRectF r(press_, e->position());
        if (r.width() < 3 && r.height() < 3) r = QRectF(press_ - QPointF(3, 3), QSizeF(6, 6));
        select_in(r, e->modifiers() & (Qt::ShiftModifier | Qt::ControlModifier));
    } else if (mode_ == Mode::Move && drag_frames_ != 0) {
        QJsonArray refs = selected_refs();
        if (!refs.isEmpty()) {
            exec("editKeys", QJsonObject{{"keys", refs}, {"timeOffset", drag_frames_}});
            QSet<QPair<QString, double>> moved;
            for (const auto& s : sel_) moved.insert({s.first, s.second + drag_frames_});
            sel_ = moved;
            emit edited();
        }
    }
    mode_ = Mode::None;
    drag_frames_ = 0;
    update();
}

void DopeCanvas::wheelEvent(QWheelEvent* e) {
    double f = std::exp(-e->angleDelta().y() / 600.0);
    double x = e->position().x();
    double t = t_of(x);
    tpp_ *= f;
    t0_ = t - x * tpp_;
    update();
}

void DopeCanvas::keyPressEvent(QKeyEvent* e) {
    if (e->key() == Qt::Key_Delete || e->key() == Qt::Key_Backspace) {
        QJsonArray refs = selected_refs();
        if (!refs.isEmpty()) {
            exec("editKeys", QJsonObject{{"keys", refs}, {"delete", true}});
            sel_.clear();
            emit edited();
        }
    } else if (e->key() == Qt::Key_F || e->key() == Qt::Key_A) {
        frame_all();
    } else {
        QWidget::keyPressEvent(e);
    }
}

// ---------------------------------------------------------------------------------------------------------------------

DopeSheet::DopeSheet(QWidget* parent) : QWidget(parent) {
    auto* l = new QVBoxLayout(this);
    l->setContentsMargins(0, 0, 0, 0);
    auto* split = new QSplitter(Qt::Horizontal, this);
    tree_ = new QTreeWidget(split);
    // The header stands level with the canvas's frame ruler, so the rows line up.
    tree_->setHeaderLabels({""});
    tree_->header()->setFixedHeight(24);
    tree_->setIndentation(14);
    tree_->setMinimumWidth(180);
    tree_->setUniformRowHeights(true);
    canvas_ = new DopeCanvas(split);
    split->addWidget(tree_);
    split->addWidget(canvas_);
    split->setStretchFactor(1, 1);
    l->addWidget(split);
    connect(Host::get(), &Host::sceneChanged, this, [this] {
        if (isVisible()) refresh();
    });
    connect(canvas_, &DopeCanvas::edited, this, [this] { refresh(); });
    connect(tree_, &QTreeWidget::itemExpanded, this, [this] { tree_key_.clear(); refresh(); });
    connect(tree_, &QTreeWidget::itemCollapsed, this, [this] { tree_key_.clear(); refresh(); });
    refresh();
}

void DopeSheet::refresh() {
    QJsonObject r = query("animCurves", "1:0:1").toObject();
    QHash<QString, QVector<QPair<double, bool>>> keys;
    QJsonArray objs = r["objects"].toArray();
    QString key;
    for (const QJsonValue& o : objs) {
        key += o.toObject()["node"].toString() + ";";
        for (const QJsonValue& c : o.toObject()["curves"].toArray()) {
            QJsonObject co = c.toObject();
            key += co["channel"].toString() + ",";
            QVector<QPair<double, bool>> list;
            for (const QJsonValue& k : co["keys"].toArray()) list.append({k.toArray().at(0).toDouble(), k.toArray().at(6).toBool()});
            keys[co["curve"].toString()] = list;
        }
    }
    // The tree: Dope Sheet Summary, then each object and its channels.
    if (key != tree_key_) {
        QSet<QString> collapsed;
        for (int i = 0; i < tree_->topLevelItemCount(); ++i) {
            QTreeWidgetItem* s = tree_->topLevelItem(i);
            for (int j = 0; j < s->childCount(); ++j)
                if (!s->child(j)->isExpanded()) collapsed.insert(s->child(j)->text(0));
        }
        tree_key_ = key;
        tree_->blockSignals(true);
        tree_->clear();
        auto* summary = new QTreeWidgetItem(tree_, QStringList{"Dope Sheet Summary"});
        for (const QJsonValue& o : objs) {
            QString node = o.toObject()["node"].toString().section('|', -1);
            auto* n = new QTreeWidgetItem(summary, QStringList{node});
            for (const QJsonValue& c : o.toObject()["curves"].toArray()) {
                QString ch = c.toObject()["channel"].toString();
                new QTreeWidgetItem(n, QStringList{ch.left(1).toUpper() + ch.mid(1)});
            }
            n->setExpanded(!collapsed.contains(node));
        }
        summary->setExpanded(true);
        tree_->blockSignals(false);
    }
    // Rows in tree order, skipping what's collapsed.
    QVector<DopeRow> rows;
    if (tree_->topLevelItemCount() > 0) {
        DopeRow s;
        s.label = "Dope Sheet Summary";
        for (auto it = keys.begin(); it != keys.end(); ++it) s.curves << it.key();
        rows.append(s);
        QTreeWidgetItem* sum = tree_->topLevelItem(0);
        if (sum->isExpanded()) {
            int i = 0;
            for (const QJsonValue& o : objs) {
                QJsonArray cs = o.toObject()["curves"].toArray();
                QTreeWidgetItem* n = sum->child(i++);
                DopeRow row;
                row.label = n ? n->text(0) : QString();
                row.depth = 1;
                for (const QJsonValue& c : cs) row.curves << c.toObject()["curve"].toString();
                rows.append(row);
                if (n && n->isExpanded())
                    for (const QJsonValue& c : cs) {
                        DopeRow ch;
                        ch.label = c.toObject()["channel"].toString();
                        ch.depth = 2;
                        ch.is_channel = true;
                        ch.curves << c.toObject()["curve"].toString();
                        rows.append(ch);
                    }
            }
        }
    }
    // Rows line up with the tree's.
    QTreeWidgetItem* top = tree_->topLevelItem(0);
    int h = top ? tree_->visualItemRect(top).height() : 20;
    canvas_->set_row_height(qMax(16, h));
    canvas_->set_data(rows, keys, r["time"].toDouble());
}

// ---------------------------------------------------------------------------------------------------------------------

SetDrivenKeyWindow::SetDrivenKeyWindow(QWidget* parent) : QDialog(parent) {
    setWindowTitle("Set Driven Key");
    resize(560, 460);
    auto* root = new QVBoxLayout(this);
    auto section = [this, root](const QString& title, QListWidget*& objs, QListWidget*& attrs) {
        root->addWidget(new QLabel(title, this));
        auto* row = new QHBoxLayout();
        objs = new QListWidget(this);
        attrs = new QListWidget(this);
        attrs->setSelectionMode(QAbstractItemView::ExtendedSelection);
        row->addWidget(objs);
        row->addWidget(attrs);
        root->addLayout(row, 1);
        QListWidget* o = objs;
        QListWidget* a = attrs;
        connect(o, &QListWidget::currentRowChanged, this, [this, o, a] { show_attrs(o, a); });
    };
    section("Driver", driver_objs_, driver_attrs_);
    driver_attrs_->setSelectionMode(QAbstractItemView::SingleSelection);
    section("Driven", driven_objs_, driven_attrs_);
    auto* buttons = new QHBoxLayout();
    auto* key_btn = new QPushButton("Key", this);
    auto* load_driver = new QPushButton("Load Driver", this);
    auto* load_driven = new QPushButton("Load Driven", this);
    auto* close = new QPushButton("Close", this);
    for (auto* b : {key_btn, load_driver, load_driven, close}) buttons->addWidget(b);
    root->addLayout(buttons);
    connect(key_btn, &QPushButton::clicked, this, [this] { key(); });
    connect(load_driver, &QPushButton::clicked, this, [this] { load(driver_objs_, driver_attrs_); });
    connect(load_driven, &QPushButton::clicked, this, [this] { load(driven_objs_, driven_attrs_); });
    connect(close, &QPushButton::clicked, this, &QDialog::hide);
}

void SetDrivenKeyWindow::load(QListWidget* objs, QListWidget* attrs) {
    objs->clear();
    for (const QJsonValue& v : state()["selection"].toArray()) objs->addItem(v.toString().section('.', 0, 0));
    if (objs->count()) objs->setCurrentRow(0);
    show_attrs(objs, attrs);
}

void SetDrivenKeyWindow::show_attrs(QListWidget* objs, QListWidget* attrs) {
    attrs->clear();
    if (!objs->currentItem()) return;
    QJsonValue list = ask("listAttr", QJsonObject{{"args", QJsonArray{objs->currentItem()->text()}}, {"keyable", true}});
    for (const QJsonValue& v : list.toArray()) attrs->addItem(v.toString());
}

void SetDrivenKeyWindow::key() {
    if (!driver_objs_->currentItem() || !driver_attrs_->currentItem()) {
        log("// Warning: load a driver and pick its attribute //", 4);
        return;
    }
    QString driver = driver_objs_->currentItem()->text() + "." + driver_attrs_->currentItem()->text();
    QJsonArray plugs;
    for (int i = 0; i < driven_objs_->count(); ++i) {
        QString obj = driven_objs_->item(i)->text();
        // Each driven object takes the attributes picked.
        for (QListWidgetItem* a : driven_attrs_->selectedItems()) plugs.append(obj + "." + a->text());
    }
    if (plugs.isEmpty()) {
        log("// Warning: load what is driven and pick its attributes //", 4);
        return;
    }
    az::exec("setDrivenKeyframe", QJsonObject{{"args", plugs}, {"currentDriver", driver}});
}

} // namespace az
