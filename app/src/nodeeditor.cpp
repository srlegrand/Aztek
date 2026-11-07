#include "nodeeditor.h"
#include "host.h"

#include <QJsonArray>
#include <QJsonDocument>
#include <QKeyEvent>
#include <QLineEdit>
#include <QMenu>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QWheelEvent>
#include <cmath>

namespace az {

namespace {
const int kHeader = 22, kRow = 16, kWidth = 170;

QColor node_color(const QString& type) {
    if (type == "transform" || type == "joint") return QColor(120, 90, 60);
    if (type == "mesh" || type == "nurbsCurve" || type == "camera" || type == "locator") return QColor(70, 110, 70);
    if (type.startsWith("animCurve")) return QColor(120, 60, 120);
    if (type.startsWith("poly")) return QColor(60, 90, 130);
    if (type == "skinCluster" || type == "blendShape" || type == "cluster") return QColor(130, 80, 60);
    return QColor(85, 85, 90);
}
} // namespace

NodeEditor::NodeEditor(QWidget* parent) : QWidget(parent) {
    setFocusPolicy(Qt::StrongFocus);
    setMinimumSize(400, 300);
    connect(Host::get(), &Host::sceneChanged, this, [this] {
        if (isVisible() && !roots_.isEmpty()) refresh();
    });
}

void NodeEditor::graph_selection(int depth) {
    depth_ = depth;
    roots_.clear();
    for (const QJsonValue& v : state()["selection"].toArray()) roots_ << v.toString().section('.', 0, 0);
    refresh();
    // Lay out from scratch: a new graph.
    update();
}

void NodeEditor::add_selected() {
    for (const QJsonValue& v : state()["selection"].toArray()) {
        QString n = v.toString().section('.', 0, 0);
        if (!roots_.contains(n)) roots_ << n;
    }
    refresh();
}

void NodeEditor::add_nodes(const QStringList& names) {
    for (const QString& n : names)
        if (!n.isEmpty() && !roots_.contains(n)) roots_ << n;
    refresh();
}

void NodeEditor::clear_graph() {
    roots_.clear();
    nodes_.clear();
    edges_.clear();
    sel_.clear();
    update();
}

void NodeEditor::refresh() {
    QJsonObject arg{{"roots", QJsonArray::fromStringList(roots_)}, {"depth", depth_}};
    QString text = QString::fromUtf8(QJsonDocument(arg).toJson(QJsonDocument::Compact));
    load(query("nodeGraph", text).toObject());
}

void NodeEditor::load(const QJsonObject& data) {
    QHash<QString, int> modes;
    for (const NodeBox& n : nodes_) modes[n.name] = n.mode;
    nodes_.clear();
    for (const QJsonValue& v : data["nodes"].toArray()) {
        QJsonObject o = v.toObject();
        NodeBox b;
        b.name = o["name"].toString();
        b.type = o["type"].toString();
        for (const QJsonValue& a : o["attrs"].toArray()) {
            QJsonObject ao = a.toObject();
            QString an = ao["name"].toString();
            b.attrs << an;
            if (ao["input"].toBool()) b.inputs << an;
            if (ao["output"].toBool()) b.outputs << an;
        }
        b.mode = modes.value(b.name, 2);
        b.pos = placed_.value(b.name, QPointF(1e9, 0));
        nodes_.append(b);
    }
    edges_.clear();
    for (const QJsonValue& v : data["edges"].toArray()) {
        QJsonArray e = v.toArray();
        edges_.append({e.at(0).toString(), e.at(1).toString(), e.at(2).toString(), e.at(3).toString()});
    }
    for (NodeBox& b : nodes_)
        for (const Edge& e : edges_) {
            if (e.src == b.name && !b.connected.contains(e.src_attr)) b.connected << e.src_attr;
            if (e.dst == b.name && !b.connected.contains(e.dst_attr)) b.connected << e.dst_attr;
        }
    layout_new();
    update();
}

// New nodes go in columns by how many connections upstream they sit.
void NodeEditor::layout_new() {
    QHash<QString, int> depth;
    for (const NodeBox& n : nodes_) depth[n.name] = 0;
    for (int pass = 0; pass < nodes_.size() + 1; ++pass)
        for (const Edge& e : edges_)
            if (depth.contains(e.dst) && depth.contains(e.src) && depth[e.dst] < depth[e.src] + 1 && depth[e.src] < nodes_.size()) depth[e.dst] = depth[e.src] + 1;
    QHash<int, double> y;
    for (NodeBox& n : nodes_) {
        int c = depth[n.name];
        if (n.pos.x() < 1e8) {
            y[c] = qMax(y.value(c), n.pos.y() + box_rect(n).height() / zoom_ + 20);
            continue;
        }
        n.pos = QPointF(c * (kWidth + 90), y.value(c));
        y[c] = n.pos.y() + kHeader + shown_attrs(n).size() * kRow + 24;
        placed_[n.name] = n.pos;
    }
}

QStringList NodeEditor::shown_attrs(const NodeBox& n) const {
    if (n.mode == 1) return {};
    if (n.mode == 3) return n.attrs;
    QStringList out;
    for (const QString& a : n.attrs)
        if (n.connected.contains(a)) out << a;
    for (const QString& c : n.connected)
        if (!out.contains(c)) out << c;
    return out;
}

QRectF NodeEditor::box_rect(const NodeBox& n) const {
    QPointF p = to_screen(n.pos);
    return QRectF(p, QSizeF(kWidth * zoom_, (kHeader + shown_attrs(n).size() * kRow + 4) * zoom_));
}

QPointF NodeEditor::port_pos(const NodeBox& n, const QString& attr, bool output) const {
    QRectF r = box_rect(n);
    QStringList s = shown_attrs(n);
    int i = s.indexOf(attr);
    double y = i < 0 ? r.top() + kHeader * zoom_ * 0.5 : r.top() + (kHeader + (i + 0.5) * kRow) * zoom_;
    return QPointF(output ? r.right() : r.left(), y);
}

void NodeEditor::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    p.fillRect(rect(), QColor(50, 50, 52));
    QHash<QString, const NodeBox*> by;
    for (const NodeBox& n : nodes_) by[n.name] = &n;
    auto wire = [&](QPointF a, QPointF b, QColor c, double w) {
        QPainterPath path(a);
        double dx = qMax(40.0 * zoom_, std::abs(b.x() - a.x()) * 0.5);
        path.cubicTo(a + QPointF(dx, 0), b - QPointF(dx, 0), b);
        p.setPen(QPen(c, w));
        p.setBrush(Qt::NoBrush);
        p.drawPath(path);
    };
    for (int i = 0; i < edges_.size(); ++i) {
        const Edge& e = edges_[i];
        if (!by.contains(e.src) || !by.contains(e.dst)) continue;
        bool on = sel_edges_.contains(i) || sel_.contains(e.src) || sel_.contains(e.dst);
        wire(port_pos(*by[e.src], e.src_attr, true), port_pos(*by[e.dst], e.dst_attr, false), sel_edges_.contains(i) ? QColor(255, 220, 90) : on ? QColor(150, 220, 255) : QColor(140, 180, 200), sel_edges_.contains(i) ? 2.5 : 1.5);
    }
    if (mode_ == Mode::Wire) {
        const NodeBox* n = by.value(wire_node_);
        if (n) {
            QPointF a = port_pos(*n, wire_attr_, wire_from_output_);
            if (wire_from_output_) wire(a, wire_to_, QColor(255, 255, 160), 1.5);
            else wire(wire_to_, a, QColor(255, 255, 160), 1.5);
        }
    }
    QFont title = font();
    title.setBold(true);
    title.setPointSizeF(qMax(5.0, title.pointSizeF() * zoom_));
    QFont row = font();
    row.setPointSizeF(qMax(5.0, row.pointSizeF() * zoom_ * 0.95));
    for (const NodeBox& n : nodes_) {
        QRectF r = box_rect(n);
        if (!r.intersects(rect())) continue;
        p.setPen(QPen(sel_.contains(n.name) ? QColor(255, 255, 255) : QColor(25, 25, 25), sel_.contains(n.name) ? 2 : 1));
        p.setBrush(QColor(62, 62, 66));
        p.drawRoundedRect(r, 4, 4);
        QRectF head(r.topLeft(), QSizeF(r.width(), kHeader * zoom_));
        p.setPen(Qt::NoPen);
        p.setBrush(node_color(n.type));
        p.drawRoundedRect(head, 4, 4);
        p.setPen(QColor(240, 240, 240));
        p.setFont(title);
        p.drawText(head.adjusted(6 * zoom_, 0, -4, 0), Qt::AlignVCenter | Qt::AlignLeft, QFontMetrics(title).elidedText(n.name, Qt::ElideMiddle, int(head.width() - 10 * zoom_)));
        p.setFont(row);
        QStringList s = shown_attrs(n);
        for (int i = 0; i < s.size(); ++i) {
            double y = r.top() + (kHeader + i * kRow) * zoom_;
            QRectF rr(r.left() + 8 * zoom_, y, r.width() - 16 * zoom_, kRow * zoom_);
            p.setPen(n.connected.contains(s[i]) ? QColor(240, 240, 200) : QColor(195, 195, 195));
            p.drawText(rr, Qt::AlignVCenter | Qt::AlignLeft, s[i]);
            auto port = [&](QPointF c, bool on) {
                p.setPen(QPen(QColor(25, 25, 25), 1));
                p.setBrush(on ? QColor(240, 200, 90) : QColor(150, 150, 150));
                p.drawEllipse(c, 3.5 * zoom_, 3.5 * zoom_);
            };
            double cy = y + kRow * zoom_ * 0.5;
            if (n.inputs.contains(s[i]) || !n.outputs.contains(s[i])) port(QPointF(r.left(), cy), n.connected.contains(s[i]));
            if (n.outputs.contains(s[i]) || !n.inputs.contains(s[i])) port(QPointF(r.right(), cy), n.connected.contains(s[i]));
        }
    }
    if (mode_ == Mode::Marquee) {
        p.setPen(QPen(Qt::white, 1, Qt::DashLine));
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(press_, last_));
    }
    p.setPen(QColor(170, 170, 170));
    p.setFont(font());
    if (nodes_.isEmpty()) p.drawText(rect(), Qt::AlignCenter, "Select nodes, then press the Input and Output Connections button or Tab to create a node.");
}

void NodeEditor::mousePressEvent(QMouseEvent* e) {
    setFocus();
    press_ = last_ = e->position();
    if (e->modifiers() & Qt::AltModifier) {
        mode_ = e->button() == Qt::RightButton ? Mode::Zoom : Mode::Pan;
        return;
    }
    if (e->button() == Qt::MiddleButton) {
        mode_ = Mode::Pan;
        return;
    }
    if (e->button() != Qt::LeftButton) return;
    QPointF at = e->position();
    // Ports first.
    for (int k = nodes_.size() - 1; k >= 0; --k) {
        const NodeBox& n = nodes_[k];
        for (const QString& a : shown_attrs(n))
            for (bool out : {true, false})
                if ((port_pos(n, a, out) - at).manhattanLength() < 10) {
                    mode_ = Mode::Wire;
                    wire_node_ = n.name;
                    wire_attr_ = a;
                    wire_from_output_ = out;
                    wire_to_ = at;
                    return;
                }
    }
    for (int k = nodes_.size() - 1; k >= 0; --k) {
        const NodeBox& n = nodes_[k];
        if (box_rect(n).contains(at)) {
            bool shift = e->modifiers() & (Qt::ShiftModifier | Qt::ControlModifier);
            if (!sel_.contains(n.name)) {
                if (!shift) sel_.clear();
                sel_.insert(n.name);
            } else if (shift) {
                sel_.remove(n.name);
            }
            sel_edges_.clear();
            mode_ = Mode::Move;
            QJsonArray names;
            for (const QString& s : sel_) names.append(s);
            exec_quiet("select", QJsonObject{{"args", names}});
            Host::get()->poke();
            update();
            return;
        }
    }
    // A connection under the cursor.
    QHash<QString, const NodeBox*> by;
    for (const NodeBox& n : nodes_) by[n.name] = &n;
    sel_edges_.clear();
    for (int i = 0; i < edges_.size(); ++i) {
        const Edge& ed = edges_[i];
        if (!by.contains(ed.src) || !by.contains(ed.dst)) continue;
        QPointF a = port_pos(*by[ed.src], ed.src_attr, true), b = port_pos(*by[ed.dst], ed.dst_attr, false);
        double dx = qMax(40.0 * zoom_, std::abs(b.x() - a.x()) * 0.5);
        for (int s = 0; s <= 24; ++s) {
            double t = s / 24.0, u = 1 - t;
            QPointF c = a * u * u * u + (a + QPointF(dx, 0)) * 3 * u * u * t + (b - QPointF(dx, 0)) * 3 * u * t * t + b * t * t * t;
            if ((c - at).manhattanLength() < 7) {
                sel_edges_.insert(i);
                update();
                return;
            }
        }
    }
    mode_ = Mode::Marquee;
}

void NodeEditor::mouseMoveEvent(QMouseEvent* e) {
    QPointF d = e->position() - last_;
    switch (mode_) {
    case Mode::Pan:
        pan_ += d;
        break;
    case Mode::Zoom: {
        double f = std::exp(d.x() * 0.005);
        QPointF g = to_graph(press_);
        zoom_ = qBound(0.1, zoom_ * f, 3.0);
        pan_ = press_ - g * zoom_;
        break;
    }
    case Mode::Move:
        for (NodeBox& n : nodes_)
            if (sel_.contains(n.name)) {
                n.pos += d / zoom_;
                placed_[n.name] = n.pos;
            }
        break;
    case Mode::Wire:
        wire_to_ = e->position();
        break;
    default:
        break;
    }
    last_ = e->position();
    update();
}

void NodeEditor::mouseReleaseEvent(QMouseEvent* e) {
    if (mode_ == Mode::Wire) {
        QPointF at = e->position();
        for (const NodeBox& n : nodes_) {
            if (n.name == wire_node_) continue;
            QString hit;
            QStringList list = shown_attrs(n);
            for (const QString& a : list)
                if ((port_pos(n, a, !wire_from_output_) - at).manhattanLength() < 12) hit = a;
            // Dropped on a box: let the user pick an attribute.
            if (hit.isEmpty() && box_rect(n).contains(at)) {
                QMenu m(this);
                for (const QString& a : (wire_from_output_ ? n.inputs : n.outputs)) m.addAction(a);
                if (QAction* c = m.exec(mapToGlobal(at.toPoint()))) hit = c->text();
            }
            if (!hit.isEmpty()) {
                QString src = wire_from_output_ ? wire_node_ + "." + wire_attr_ : n.name + "." + hit;
                QString dst = wire_from_output_ ? n.name + "." + hit : wire_node_ + "." + wire_attr_;
                exec("connectAttr", QJsonObject{{"args", QJsonArray{src, dst}}, {"force", true}});
                break;
            }
        }
        refresh();
    } else if (mode_ == Mode::Marquee) {
        QRectF r = QRectF(press_, e->position()).normalized();
        if (!(e->modifiers() & Qt::ShiftModifier)) sel_.clear();
        for (const NodeBox& n : nodes_)
            if (r.intersects(box_rect(n)) && r.width() + r.height() > 4) sel_.insert(n.name);
        QJsonArray names;
        for (const QString& s : sel_) names.append(s);
        exec_quiet("select", names.isEmpty() ? QJsonObject{{"clear", true}} : QJsonObject{{"args", names}});
        Host::get()->poke();
    }
    mode_ = Mode::None;
    update();
}

void NodeEditor::wheelEvent(QWheelEvent* e) {
    QPointF g = to_graph(e->position());
    zoom_ = qBound(0.1, zoom_ * std::exp(e->angleDelta().y() / 800.0), 3.0);
    pan_ = e->position() - g * zoom_;
    update();
}

void NodeEditor::new_node_field(QPoint at) {
    auto* f = new QLineEdit(this);
    f->setPlaceholderText("node type");
    f->setGeometry(at.x(), at.y(), 200, 24);
    f->show();
    f->setFocus();
    connect(f, &QLineEdit::returnPressed, this, [this, f] {
        QString t = f->text().trimmed();
        f->deleteLater();
        if (t.isEmpty()) return;
        QJsonValue r = exec("createNode", QJsonObject{{"args", QJsonArray{t}}});
        if (r.isString()) {
            placed_[r.toString()] = to_graph(mapFromGlobal(QCursor::pos()));
            roots_ << r.toString();
            refresh();
        }
    });
    connect(f, &QLineEdit::editingFinished, f, &QObject::deleteLater);
}

void NodeEditor::keyPressEvent(QKeyEvent* e) {
    switch (e->key()) {
    case Qt::Key_Delete:
    case Qt::Key_Backspace: {
        for (int i : sel_edges_) {
            const Edge& ed = edges_[i];
            exec("disconnectAttr", QJsonObject{{"args", QJsonArray{ed.src + "." + ed.src_attr, ed.dst + "." + ed.dst_attr}}});
        }
        if (sel_edges_.isEmpty() && !sel_.isEmpty()) {
            QJsonArray names;
            for (const QString& s : sel_) names.append(s);
            exec("delete", QJsonObject{{"args", names}});
            for (const QString& s : sel_) roots_.removeAll(s);
            sel_.clear();
        }
        sel_edges_.clear();
        refresh();
        break;
    }
    case Qt::Key_Tab:
        new_node_field(mapFromGlobal(QCursor::pos()));
        break;
    case Qt::Key_1:
    case Qt::Key_2:
    case Qt::Key_3:
        // How much of each node shows: none, connected, all its attributes.
        for (NodeBox& n : nodes_)
            if (sel_.isEmpty() || sel_.contains(n.name)) n.mode = e->key() - Qt::Key_0;
        update();
        break;
    case Qt::Key_F:
    case Qt::Key_A: {
        if (nodes_.isEmpty()) break;
        QRectF all;
        for (const NodeBox& n : nodes_) {
            QRectF g(n.pos, QSizeF(kWidth, kHeader + shown_attrs(n).size() * kRow));
            all = all.isNull() ? g : all.united(g);
        }
        zoom_ = qBound(0.1, qMin(width() / (all.width() + 60), height() / (all.height() + 60)), 1.5);
        pan_ = QPointF(30, 30) - all.topLeft() * zoom_;
        update();
        break;
    }
    default:
        QWidget::keyPressEvent(e);
    }
}

} // namespace az
