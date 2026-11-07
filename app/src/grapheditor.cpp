#include "grapheditor.h"
#include "aztek.h"
#include "host.h"
#include "icons.h"

#include <QApplication>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QMenuBar>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QSplitter>
#include <QToolBar>
#include <QTreeWidget>
#include <QVBoxLayout>
#include <QWheelEvent>
#include <cmath>

namespace az {

static QColor channel_color(const QString& ch, int component) {
    if (component == 0 || ch.endsWith("X")) return QColor(232, 74, 74);
    if (component == 1 || ch.endsWith("Y")) return QColor(102, 214, 82);
    if (component == 2 || ch.endsWith("Z")) return QColor(82, 140, 236);
    if (ch == "visibility") return QColor(196, 196, 110);
    return QColor(110, 210, 210);
}

// ---------------------------------------------------------------------------------------------------------------------
// Canvas

CurveCanvas::CurveCanvas(QWidget* parent) : QWidget(parent) {
    setFocusPolicy(Qt::StrongFocus);
    setMouseTracking(true);
    setMinimumSize(200, 120);
}

QPointF CurveCanvas::to_screen(double t, double v) const {
    return QPointF((t - t0_) / tpp_, (v0_ - v) / vpp_);
}

QPointF CurveCanvas::to_graph(QPointF p) const {
    return QPointF(t0_ + p.x() * tpp_, v0_ - p.y() * vpp_);
}

QPointF CurveCanvas::curve_point(int ci, double t, double v) const {
    if (ci >= 0 && ci < maps_.size()) v = v * maps_[ci].first + maps_[ci].second;
    return to_screen(t, v);
}

// Normalized fits each curve between -1 and 1; stacked also gives each its own row.
void CurveCanvas::update_maps() {
    maps_.clear();
    int row = 0;
    for (const CurveView& c : curves_) {
        if (display_ == CurveDisplay::Absolute || !c.visible) {
            maps_.append({1.0, 0.0});
            continue;
        }
        double lo = 1e18, hi = -1e18;
        for (const QPointF& p : c.samples) lo = qMin(lo, p.y()), hi = qMax(hi, p.y());
        for (const auto& k : c.keys) lo = qMin(lo, k[1]), hi = qMax(hi, k[1]);
        double span = hi - lo;
        double scale = span > 1e-9 ? 2.0 / span : 1.0;
        double offset = span > 1e-9 ? -(hi + lo) / span : -lo;
        if (display_ == CurveDisplay::Stacked) offset -= 2.4 * row++;
        maps_.append({scale, offset});
    }
}

void CurveCanvas::set_display(CurveDisplay d) {
    display_ = d;
    update_maps();
    frame(false);
}

QList<QPair<QString, double>> CurveCanvas::selected_times() const {
    QList<QPair<QString, double>> out;
    for (auto [ci, ki] : sel_)
        if (ci < curves_.size() && ki < curves_[ci].keys.size()) out.append({curves_[ci].curve, curves_[ci].keys[ki][0]});
    return out;
}

void CurveCanvas::set_curves(const QVector<CurveView>& c, double time) {
    // Keep the selection on the same curves where they're still there.
    QSet<QPair<int, int>> keep;
    for (auto [ci, ki] : sel_) {
        if (ci >= curves_.size()) continue;
        QString name = curves_[ci].curve;
        for (int j = 0; j < c.size(); ++j)
            if (c[j].curve == name && ki < c[j].keys.size()) keep.insert({j, ki});
    }
    curves_ = c;
    sel_ = keep;
    time_ = time;
    update_maps();
    if (!framed_ && !curves_.isEmpty()) {
        framed_ = true;
        frame(false);
    }
    update();
}

void CurveCanvas::frame(bool selected_only) {
    double t_lo = 1e18, t_hi = -1e18, v_lo = 1e18, v_hi = -1e18;
    for (int ci = 0; ci < curves_.size(); ++ci) {
        if (!curves_[ci].visible) continue;
        for (int ki = 0; ki < curves_[ci].keys.size(); ++ki) {
            if (selected_only && !sel_.contains({ci, ki})) continue;
            auto& k = curves_[ci].keys[ki];
            double shown = ci < maps_.size() ? k[1] * maps_[ci].first + maps_[ci].second : k[1];
            t_lo = qMin(t_lo, k[0]), t_hi = qMax(t_hi, k[0]);
            v_lo = qMin(v_lo, shown), v_hi = qMax(v_hi, shown);
        }
    }
    if (t_lo > t_hi) {
        if (selected_only) frame(false);
        return;
    }
    if (t_hi - t_lo < 1) t_lo -= 5, t_hi += 5;
    if (v_hi - v_lo < 1e-3) v_lo -= 1, v_hi += 1;
    double mt = (t_hi - t_lo) * 0.08, mv = (v_hi - v_lo) * 0.12;
    t_lo -= mt, t_hi += mt, v_lo -= mv, v_hi += mv;
    tpp_ = (t_hi - t_lo) / qMax(1, width());
    vpp_ = (v_hi - v_lo) / qMax(1, height() - 20);
    t0_ = t_lo;
    v0_ = v_hi + 20 * vpp_;
    emit selectionChanged();
    update();
}

std::array<double, 3> CurveCanvas::sample_window() const {
    double w = qMax(1, width());
    double lo = t0_ - w * tpp_ * 0.25, hi = t0_ + w * tpp_ * 1.25;
    double step = qMax(tpp_ * 3, (hi - lo) / 4000);
    // Before the first framing the view is a guess: cover a wide span.
    if (!framed_) return {-200, 400, 0.5};
    return {std::floor(lo), std::ceil(hi), step};
}

QList<QPair<QString, int>> CurveCanvas::selected_keys() const {
    QList<QPair<QString, int>> out;
    for (auto [ci, ki] : sel_)
        if (ci < curves_.size()) out.append({curves_[ci].curve, ki});
    return out;
}

void CurveCanvas::select_all_keys_of(const QSet<QString>& names) {
    sel_.clear();
    for (int ci = 0; ci < curves_.size(); ++ci)
        if (names.contains(curves_[ci].curve))
            for (int ki = 0; ki < curves_[ci].keys.size(); ++ki) sel_.insert({ci, ki});
    emit selectionChanged();
    update();
}

static double nice_step(double units_per_px, double min_px) {
    double raw = units_per_px * min_px;
    double p = std::pow(10.0, std::floor(std::log10(raw)));
    for (double m : {1.0, 2.0, 5.0, 10.0})
        if (p * m >= raw) return p * m;
    return p * 10;
}

void CurveCanvas::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.fillRect(rect(), QColor(57, 57, 57));
    // Grid with labels: frames along the top, values down the left.
    double ts = qMax(1.0, nice_step(tpp_, 60)), vs = nice_step(vpp_, 40);
    QFont f = font();
    f.setPointSizeF(qMax(6.5, f.pointSizeF() - 1));
    p.setFont(f);
    QPointF g0 = to_graph(QPointF(0, height())), g1 = to_graph(QPointF(width(), 0));
    for (double t = std::floor(g0.x() / ts) * ts; t <= g1.x(); t += ts) {
        double x = to_screen(t, 0).x();
        p.setPen(QColor(66, 66, 66));
        p.drawLine(QPointF(x, 0), QPointF(x, height()));
        p.setPen(QColor(150, 150, 150));
        p.drawText(QPointF(x + 2, 12), QString::number(t));
    }
    for (double v = std::floor(g0.y() / vs) * vs; v <= g1.y(); v += vs) {
        double y = to_screen(0, v).y();
        p.setPen(std::abs(v) < vs * 1e-6 ? QColor(30, 30, 30) : QColor(66, 66, 66));
        p.drawLine(QPointF(0, y), QPointF(width(), y));
        p.setPen(QColor(150, 150, 150));
        p.drawText(QPointF(3, y - 2), QString::number(std::abs(v) < vs * 1e-6 ? 0 : v, 'g', 5));
    }
    // The current frame.
    double tx = to_screen(time_, 0).x();
    p.setPen(QPen(QColor(200, 40, 40), 1.5));
    p.drawLine(QPointF(tx, 0), QPointF(tx, height()));
    p.setRenderHint(QPainter::Antialiasing);
    for (int ci = 0; ci < curves_.size(); ++ci) {
        const CurveView& c = curves_[ci];
        if (!c.visible || c.samples.isEmpty()) continue;
        // The buffer curve sits behind, grey.
        if (!c.buffer.isEmpty()) {
            QPainterPath held;
            held.moveTo(curve_point(ci, c.buffer[0].x(), c.buffer[0].y()));
            for (int i = 1; i < c.buffer.size(); ++i) held.lineTo(curve_point(ci, c.buffer[i].x(), c.buffer[i].y()));
            p.setPen(QPen(QColor(120, 120, 120), 1));
            p.setBrush(Qt::NoBrush);
            p.drawPath(held);
        }
        QPainterPath path;
        path.moveTo(curve_point(ci, c.samples[0].x(), c.samples[0].y()));
        for (int i = 1; i < c.samples.size(); ++i) path.lineTo(curve_point(ci, c.samples[i].x(), c.samples[i].y()));
        bool any_sel = false;
        for (int ki = 0; ki < c.keys.size(); ++ki) any_sel |= sel_.contains({ci, ki});
        p.setPen(QPen(any_sel ? c.color.lighter(125) : c.color, any_sel ? 1.6 : 1.2));
        p.setBrush(Qt::NoBrush);
        p.drawPath(path);
        for (int ki = 0; ki < c.keys.size(); ++ki) {
            auto& k = c.keys[ki];
            QPointF at = curve_point(ci, k[0], k[1]);
            bool s = sel_.contains({ci, ki});
            if (s) {
                double scale = ci < maps_.size() ? maps_[ci].first : 1.0;
                // Tangent handles: as long as they are on a weighted curve, a fixed length on screen otherwise.
                for (int side : {-1, 1}) {
                    double slope = side < 0 ? k[2] : k[3];
                    QPointF d(1 / tpp_, -slope * scale / vpp_);
                    double len = std::hypot(d.x(), d.y());
                    QPointF h = at + d / len * 40 * side;
                    if (c.weighted && ki < c.handles.size()) {
                        const auto& w = c.handles[ki];
                        double hx = side < 0 ? w[0] : w[2], hy = side < 0 ? w[1] : w[3];
                        h = at + QPointF(hx / tpp_, -hy * scale / vpp_) * side;
                    }
                    p.setPen(QPen(QColor(150, 110, 60), 1));
                    p.drawLine(at, h);
                    p.setBrush(QColor(170, 120, 70));
                    p.drawEllipse(h, 2.5, 2.5);
                }
            }
            p.setPen(QPen(Qt::black, 1));
            p.setBrush(s ? QColor(255, 220, 40) : QColor(20, 20, 20));
            p.drawRect(QRectF(at.x() - 3, at.y() - 3, 6, 6));
        }
    }
    if (mode_ == Mode::Marquee) {
        p.setPen(QPen(QColor(220, 220, 220), 1, Qt::DashLine));
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(press_, last_).normalized());
    }
    if (curves_.isEmpty()) {
        p.setPen(QColor(150, 150, 150));
        p.drawText(rect(), Qt::AlignCenter, "Select animated objects to see their curves.");
    }
}

QPair<int, int> CurveCanvas::key_at(QPointF pos) const {
    QPair<int, int> best{-1, -1};
    double bd = 7;
    for (int ci = 0; ci < curves_.size(); ++ci) {
        if (!curves_[ci].visible) continue;
        for (int ki = 0; ki < curves_[ci].keys.size(); ++ki) {
            auto& k = curves_[ci].keys[ki];
            double d = QLineF(curve_point(ci, k[0], k[1]), pos).length();
            if (d < bd) bd = d, best = {ci, ki};
        }
    }
    return best;
}

// What an animation curve drives, as setKeyframe takes it.
static QJsonArray plugs_of_curve(const QString& curve) {
    return ask("listConnections", QJsonObject{{"args", QJsonArray{curve}}, {"plugs", true}, {"destination", true}, {"source", false}}).toArray();
}

static QString g_curve_tool = "select";

void CurveCanvas::set_tool(const QString& t) { g_curve_tool = t; }
QString CurveCanvas::tool() { return g_curve_tool; }

void CurveCanvas::mousePressEvent(QMouseEvent* e) {
    press_ = last_ = e->position();
    bool alt = e->modifiers() & Qt::AltModifier;
    if (alt && e->button() == Qt::MiddleButton) mode_ = Mode::Pan;
    else if (alt && e->button() == Qt::RightButton) mode_ = Mode::Zoom;
    else if (e->button() == Qt::LeftButton && g_curve_tool == "dolly") mode_ = Mode::Zoom;
    else if (e->button() == Qt::LeftButton && g_curve_tool == "track") mode_ = Mode::Pan;
    else if (e->button() == Qt::LeftButton && g_curve_tool == "time") {
        mode_ = Mode::Scrub;
        emit timeDragged(std::round(to_graph(e->position()).x()));
    } else if (e->button() == Qt::LeftButton && (g_curve_tool == "move") && !sel_.isEmpty() && key_at(e->position()).first < 0) {
        // The move tool drags the picked keys from anywhere.
        mode_ = Mode::Move;
        drag_applied_ = false;
        axis_lock_ = (e->modifiers() & Qt::ShiftModifier) ? -1 : 0;
    } else if (e->button() == Qt::LeftButton && (g_curve_tool == "set" || g_curve_tool == "insert") && !curves_.isEmpty()) {
        // A key on the nearest curve at this frame: insert keeps the curve's shape, set puts the key where the click is.
        QPointF g = to_graph(e->position());
        int best = -1;
        double bd = 1e300;
        for (int i = 0; i < curves_.size(); ++i)
            for (const QPointF& s : curves_[i].samples) {
                double d = QLineF(curve_point(i, s.x(), s.y()), e->position()).length();
                if (d < bd) bd = d, best = i;
            }
        if (best >= 0) {
            const double t = std::round(g.x());
            QJsonObject call{{"args", plugs_of_curve(curves_[best].curve)}, {"time", t}};
            if (g_curve_tool == "insert") call["insert"] = true;
            else call["value"] = g.y();
            exec("setKeyframe", call);
            emit keysChanged();
        }
    }
    else if (e->button() == Qt::LeftButton && e->position().y() < 16) {
        mode_ = Mode::Scrub;
        emit timeDragged(std::round(to_graph(e->position()).x()));
    } else if (e->button() == Qt::LeftButton) {
        auto hit = key_at(e->position());
        bool shift = e->modifiers() & Qt::ShiftModifier, ctrl = e->modifiers() & Qt::ControlModifier;
        if (hit.first >= 0) {
            if (shift && !ctrl) {
                if (sel_.contains(hit)) sel_.remove(hit);
                else sel_.insert(hit);
            } else if (ctrl) {
                sel_.remove(hit);
            } else if (!sel_.contains(hit)) {
                sel_ = {hit};
            }
            // Dragging a picked key moves the selection, as with the reference's move tool.
            mode_ = Mode::Move;
            drag_applied_ = false;
            axis_lock_ = shift ? -1 : 0;
            emit selectionChanged();
        } else {
            mode_ = Mode::Marquee;
        }
        update();
    } else if (e->button() == Qt::MiddleButton && !sel_.isEmpty()) {
        mode_ = Mode::Move;
        drag_applied_ = false;
        axis_lock_ = (e->modifiers() & Qt::ShiftModifier) ? -1 : 0;
    }
}

void CurveCanvas::apply_drag(QPointF to, bool final) {
    QPointF d = to - press_;
    if (axis_lock_ == -1 && QLineF(press_, to).length() > 4) axis_lock_ = std::abs(d.x()) > std::abs(d.y()) ? 1 : 2;
    double dt = std::round(d.x() * tpp_);
    double dv = -d.y() * vpp_;
    // Value units differ curve to curve in these views: keys move in time only.
    if (axis_lock_ == 1 || display_ != CurveDisplay::Absolute) dv = 0;
    if (axis_lock_ == 2) dt = 0;
    if (drag_applied_) aztek_retract();
    drag_applied_ = false;
    if (dt == 0 && std::abs(dv) < 1e-12) {
        Host::get()->poke();
        return;
    }
    QJsonArray keys;
    for (auto& k : selected_keys()) keys.append(QJsonArray{k.first, k.second});
    QJsonObject call{{"keys", keys}, {"timeOffset", dt}, {"valueOffset", dv}};
    QJsonValue r = final ? exec("editKeys", call) : exec_quiet("editKeys", call);
    drag_applied_ = !r.isUndefined() && !final;
    // Draw the preview straight away from the offsets; the next refresh brings the real curves.
    emit keysChanged();
}

void CurveCanvas::mouseMoveEvent(QMouseEvent* e) {
    QPointF pos = e->position();
    QPointF d = pos - last_;
    switch (mode_) {
    case Mode::Pan:
        t0_ -= d.x() * tpp_;
        v0_ += d.y() * vpp_;
        update();
        break;
    case Mode::Zoom: {
        double fx = std::exp(-d.x() * 0.01), fy = std::exp(d.y() * 0.01);
        QPointF c = to_graph(press_);
        tpp_ *= fx, vpp_ *= fy;
        t0_ = c.x() - press_.x() * tpp_;
        v0_ = c.y() + press_.y() * vpp_;
        update();
        break;
    }
    case Mode::Marquee: update(); break;
    case Mode::Move: apply_drag(pos, false); break;
    case Mode::Scrub: emit timeDragged(std::round(to_graph(pos).x())); break;
    default: break;
    }
    last_ = pos;
}

void CurveCanvas::mouseReleaseEvent(QMouseEvent* e) {
    if (mode_ == Mode::Marquee) {
        QRectF r = QRectF(press_, e->position()).normalized();
        bool shift = e->modifiers() & Qt::ShiftModifier, ctrl = e->modifiers() & Qt::ControlModifier;
        if (!shift && !ctrl) sel_.clear();
        for (int ci = 0; ci < curves_.size(); ++ci) {
            if (!curves_[ci].visible) continue;
            for (int ki = 0; ki < curves_[ci].keys.size(); ++ki) {
                auto& k = curves_[ci].keys[ki];
                if (!r.contains(curve_point(ci, k[0], k[1]))) continue;
                if (ctrl) sel_.remove({ci, ki});
                else if (shift && sel_.contains({ci, ki})) sel_.remove({ci, ki});
                else sel_.insert({ci, ki});
            }
        }
        emit selectionChanged();
    } else if (mode_ == Mode::Move) {
        bool was = drag_applied_;
        if (was) {
            aztek_retract();
            drag_applied_ = false;
            apply_drag(e->position(), true);
        }
    }
    mode_ = Mode::None;
    update();
}

void CurveCanvas::wheelEvent(QWheelEvent* e) {
    double f = std::exp(-e->angleDelta().y() / 120.0 * 0.15);
    QPointF at = e->position();
    QPointF c = to_graph(at);
    tpp_ *= f, vpp_ *= f;
    t0_ = c.x() - at.x() * tpp_;
    v0_ = c.y() + at.y() * vpp_;
    update();
}

void CurveCanvas::nudge_keys(double frames) {
    QJsonArray keys;
    for (auto& k : selected_keys()) keys.append(QJsonArray{k.first, k.second});
    if (keys.isEmpty()) return;
    exec("editKeys", QJsonObject{{"keys", keys}, {"timeOffset", frames}});
    emit keysChanged();
}

// Stretches the selected keys in time about the first of them.
void CurveCanvas::retime_keys(double factor) {
    auto keys = selected_times();
    if (keys.isEmpty()) return;
    double pivot = keys[0].second;
    for (auto& k : keys) pivot = qMin(pivot, k.second);
    ask("selectKey", QJsonObject{{"clear", true}});
    for (auto& k : keys) ask("selectKey", QJsonObject{{"args", QJsonArray{k.first}}, {"addTo", true}, {"time", k.second}});
    exec("scaleKey", QJsonObject{{"selected", true}, {"timeScale", factor}, {"timePivot", pivot}});
    emit keysChanged();
}

void CurveCanvas::keyPressEvent(QKeyEvent* e) {
    bool alt = e->modifiers() & Qt::AltModifier;
    switch (e->key()) {
    case Qt::Key_F: frame(!sel_.isEmpty()); return;
    case Qt::Key_A: frame(false); return;
    // Retiming: [ and ] move the selected keys a frame, with Alt they stretch or squeeze in time.
    case Qt::Key_BracketLeft:
        if (alt) retime_keys(0.9);
        else nudge_keys(-1);
        return;
    case Qt::Key_BracketRight:
        if (alt) retime_keys(1.1);
        else nudge_keys(1);
        return;
    case Qt::Key_Delete:
    case Qt::Key_Backspace: {
        QJsonArray keys;
        for (auto& k : selected_keys()) keys.append(QJsonArray{k.first, k.second});
        if (!keys.isEmpty()) exec("editKeys", QJsonObject{{"keys", keys}, {"delete", true}});
        sel_.clear();
        emit keysChanged();
        return;
    }
    default: QWidget::keyPressEvent(e);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Editor

GraphEditor::GraphEditor(QWidget* parent) : QWidget(parent) {
    setWindowTitle("Graph Editor");
    auto* v = new QVBoxLayout(this);
    v->setContentsMargins(0, 0, 0, 0);
    v->setSpacing(0);
    build_menus(v);
    auto* bar = new QToolBar(this);
    bar->setIconSize(QSize(18, 18));
    auto tan = [this, bar](const QString& label, const QString& in, const QString& out) {
        QAction* a = bar->addAction(label, [this, in, out] { set_tangents(in, out); });
        a->setToolTip(QString("%1 tangents on the selected keys").arg(label));
    };
    tan("Auto", "auto", "auto");
    tan("Spline", "spline", "spline");
    tan("Clamped", "clamped", "clamped");
    tan("Linear", "linear", "linear");
    tan("Flat", "flat", "flat");
    tan("Step", "clamped", "step");
    tan("Plateau", "plateau", "plateau");
    bar->addSeparator();
    bar->addAction("Frame All", [this] { canvas_->frame(false); });
    bar->addAction("Frame Selection", [this] { canvas_->frame(true); });
    bar->addSeparator();
    bar->addWidget(new QLabel(" Stats ", bar));
    stats_t_ = new QLineEdit(bar);
    stats_v_ = new QLineEdit(bar);
    for (QLineEdit* le : {stats_t_, stats_v_}) {
        le->setFixedWidth(70);
        bar->addWidget(le);
    }
    auto set_stat = [this](const char* flag, QLineEdit* le) {
        bool ok = false;
        double x = le->text().toDouble(&ok);
        if (!ok) return;
        QJsonArray keys;
        for (auto& k : canvas_->selected_keys()) keys.append(QJsonArray{k.first, k.second});
        if (!keys.isEmpty()) exec("editKeys", QJsonObject{{"keys", keys}, {flag, x}});
    };
    connect(stats_t_, &QLineEdit::returnPressed, this, [=] { set_stat("time", stats_t_); });
    connect(stats_v_, &QLineEdit::returnPressed, this, [=] { set_stat("value", stats_v_); });
    v->addWidget(bar);
    auto* split = new QSplitter(this);
    tree_ = new QTreeWidget(split);
    tree_->setHeaderHidden(true);
    tree_->setSelectionMode(QAbstractItemView::ExtendedSelection);
    tree_->setMinimumWidth(150);
    canvas_ = new CurveCanvas(split);
    split->addWidget(tree_);
    split->addWidget(canvas_);
    split->setStretchFactor(1, 1);
    split->setSizes({180, 700});
    v->addWidget(split, 1);
    connect(tree_, &QTreeWidget::itemSelectionChanged, this, [this] {
        QSet<QString> shown;
        for (QTreeWidgetItem* it : tree_->selectedItems()) {
            if (it->childCount())
                for (int i = 0; i < it->childCount(); ++i) shown.insert(it->child(i)->data(0, Qt::UserRole).toString());
            else
                shown.insert(it->data(0, Qt::UserRole).toString());
        }
        for (CurveView& c : canvas_->curves()) c.visible = shown.isEmpty() || shown.contains(c.curve);
        canvas_->update();
    });
    connect(canvas_, &CurveCanvas::keysChanged, this, [this] { refresh(); });
    connect(canvas_, &CurveCanvas::selectionChanged, this, [this] {
        auto keys = canvas_->selected_keys();
        if (keys.size() == 1) {
            for (const CurveView& c : canvas_->curves())
                if (c.curve == keys[0].first && keys[0].second < c.keys.size()) {
                    stats_t_->setText(QString::number(c.keys[keys[0].second][0]));
                    stats_v_->setText(QString::number(c.keys[keys[0].second][1], 'g', 6));
                }
        } else {
            stats_t_->clear();
            stats_v_->clear();
        }
        refresh();
    });
    connect(canvas_, &CurveCanvas::timeDragged, this, [](double t) {
        exec_quiet("currentTime", QJsonObject{{"args", QJsonArray{t}}});
        Host::get()->poke();
    });
    connect(Host::get(), &Host::sceneChanged, this, [this] {
        if (isVisible()) refresh();
    });
    resize(900, 480);
}

QJsonArray GraphEditor::target_curves() const {
    QSet<QString> names;
    for (auto& k : canvas_->selected_keys()) names.insert(k.first);
    if (names.isEmpty())
        for (const CurveView& c : canvas_->curves())
            if (c.visible) names.insert(c.curve);
    QJsonArray out;
    for (const QString& n : names) out.append(n);
    return out;
}

bool GraphEditor::sync_selection() {
    auto keys = canvas_->selected_times();
    ask("selectKey", QJsonObject{{"clear", true}});
    for (auto& k : keys) ask("selectKey", QJsonObject{{"args", QJsonArray{k.first}}, {"addTo", true}, {"time", k.second}});
    return !keys.isEmpty();
}

void GraphEditor::isolate(bool on) {
    QSet<QString> keep;
    for (auto& k : canvas_->selected_keys()) keep.insert(k.first);
    for (QTreeWidgetItem* it : tree_->selectedItems()) keep.insert(it->data(0, Qt::UserRole).toString());
    for (CurveView& c : canvas_->curves()) c.visible = !on || keep.isEmpty() || keep.contains(c.curve);
    canvas_->set_display(CurveDisplay::Absolute);
}

void GraphEditor::build_menus(QVBoxLayout* v) {
    auto* menus = new QMenuBar(this);
    v->addWidget(menus);
    // Runs a command on the target curves.
    auto on_curves = [this](QMenu* m, const QString& label, const QString& name, QJsonObject call) {
        m->addAction(label, [this, name, call]() mutable {
            QJsonArray curves = target_curves();
            if (curves.isEmpty()) return;
            call["args"] = curves;
            exec(name, call);
            refresh();
        });
    };
    // Runs a command on the selected keys.
    auto on_keys = [this](QMenu* m, const QString& label, const QString& name, const QJsonObject& call) {
        m->addAction(label, [this, name, call] {
            if (!sync_selection()) return;
            QJsonObject c = call;
            c["selected"] = true;
            exec(name, c);
            refresh();
        });
    };

    auto* view = menus->addMenu("View");
    view->addAction("Frame All\tA", [this] { canvas_->frame(false); });
    view->addAction("Frame Selection\tF", [this] { canvas_->frame(true); });
    view->addSeparator();
    view->addAction("Absolute View", [this] { canvas_->set_display(CurveDisplay::Absolute); });
    view->addAction("Stacked View", [this] { canvas_->set_display(CurveDisplay::Stacked); });
    view->addAction("Normalized View", [this] { canvas_->set_display(CurveDisplay::Normalized); });
    view->addSeparator();
    QAction* iso = view->addAction("Isolate Curve Display");
    iso->setCheckable(true);
    connect(iso, &QAction::toggled, this, [this](bool on) { isolate(on); });

    auto* curves = menus->addMenu("Curves");
    const QList<QPair<QString, QString>> infinities{{"Cycle", "cycle"}, {"Cycle with Offset", "cycleRelative"}, {"Oscillate", "oscillate"}, {"Linear", "linear"}, {"Constant", "constant"}};
    auto* pre = curves->addMenu("Pre Infinity");
    for (auto& i : infinities) on_curves(pre, i.first, "setInfinity", QJsonObject{{"preInfinite", i.second}});
    auto* post = curves->addMenu("Post Infinity");
    for (auto& i : infinities) on_curves(post, i.first, "setInfinity", QJsonObject{{"postInfinite", i.second}});
    curves->addSeparator();
    on_curves(curves, "Euler Filter", "filterCurve", QJsonObject{{"filter", "euler"}});
    on_curves(curves, "Simplify Curve", "simplify", QJsonObject{});
    on_curves(curves, "Resample Curve", "filterCurve", QJsonObject{{"filter", "resample"}, {"period", 1.0 / 24.0}, {"keepKeysOnFrame", true}});
    curves->addSeparator();
    on_curves(curves, "Buffer Curve Snapshot", "bufferCurve", QJsonObject{{"overwrite", true}});
    on_curves(curves, "Swap Buffer Curve", "bufferCurve", QJsonObject{{"swap", true}});

    auto* keys = menus->addMenu("Keys");
    on_keys(keys, "Copy", "copyKey", QJsonObject{});
    keys->addAction("Paste", [this] {
        QJsonArray plugs;
        for (const CurveView& c : canvas_->curves())
            if (c.visible) plugs.append(c.node + "." + c.channel);
        if (!plugs.isEmpty()) exec("pasteKey", QJsonObject{{"args", plugs}, {"option", "merge"}});
        refresh();
    });
    on_keys(keys, "Delete", "cutKey", QJsonObject{{"clear", true}});
    keys->addSeparator();
    on_keys(keys, "Snap", "snapKey", QJsonObject{{"timeMultiple", 1.0}});
    keys->addAction("Move Back a Frame\t[", [this] { canvas_->setFocus(); QKeyEvent e(QEvent::KeyPress, Qt::Key_BracketLeft, Qt::NoModifier); QApplication::sendEvent(canvas_, &e); });
    keys->addAction("Move On a Frame\t]", [this] { canvas_->setFocus(); QKeyEvent e(QEvent::KeyPress, Qt::Key_BracketRight, Qt::NoModifier); QApplication::sendEvent(canvas_, &e); });
    keys->addAction("Squeeze in Time\tAlt+[", [this] { canvas_->setFocus(); QKeyEvent e(QEvent::KeyPress, Qt::Key_BracketLeft, Qt::AltModifier); QApplication::sendEvent(canvas_, &e); });
    keys->addAction("Stretch in Time\tAlt+]", [this] { canvas_->setFocus(); QKeyEvent e(QEvent::KeyPress, Qt::Key_BracketRight, Qt::AltModifier); QApplication::sendEvent(canvas_, &e); });
    keys->addSeparator();
    on_keys(keys, "Breakdown", "keyframe", QJsonObject{{"edit", true}, {"breakdown", true}});
    on_keys(keys, "Key", "keyframe", QJsonObject{{"edit", true}, {"breakdown", false}});

    auto* tangents = menus->addMenu("Tangents");
    for (const char* t : {"auto", "spline", "clamped", "linear", "flat", "plateau", "step", "stepnext", "fixed"}) {
        QString label(t);
        label[0] = label[0].toUpper();
        QJsonObject call{{"edit", true}, {"outTangentType", t}};
        // Step tangents only say how a key leaves.
        if (QString(t) != "step" && QString(t) != "stepnext") call["inTangentType"] = t;
        on_keys(tangents, label, "keyTangent", call);
    }
    tangents->addSeparator();
    on_keys(tangents, "Break Tangents", "keyTangent", QJsonObject{{"edit", true}, {"lock", false}});
    on_keys(tangents, "Unify Tangents", "keyTangent", QJsonObject{{"edit", true}, {"unify", true}});
    tangents->addSeparator();
    on_curves(tangents, "Weighted Tangents", "keyTangent", QJsonObject{{"edit", true}, {"weightedTangents", true}});
    on_curves(tangents, "Non-Weighted Tangents", "keyTangent", QJsonObject{{"edit", true}, {"weightedTangents", false}});
    on_keys(tangents, "Free Tangent Weight", "keyTangent", QJsonObject{{"edit", true}, {"weightLock", false}});
    on_keys(tangents, "Lock Tangent Weight", "keyTangent", QJsonObject{{"edit", true}, {"weightLock", true}});
}

void GraphEditor::set_tangents(const QString& in, const QString& out) {
    QJsonArray keys;
    for (auto& k : canvas_->selected_keys()) keys.append(QJsonArray{k.first, k.second});
    if (keys.isEmpty()) return;
    exec("editKeys", QJsonObject{{"keys", keys}, {"inTangentType", in}, {"outTangentType", out}});
}

void GraphEditor::refresh() {
    // The visible frames (and a margin), sampled a few pixels apart.
    auto [lo, hi, step] = canvas_->sample_window();
    QJsonObject r = query("animCurves", QString("%1:%2:%3").arg(lo).arg(hi).arg(step)).toObject();
    QVector<CurveView> curves;
    QStringList key_list;
    for (const QJsonValue& ov : r["objects"].toArray()) {
        QJsonObject o = ov.toObject();
        for (const QJsonValue& cv : o["curves"].toArray()) {
            QJsonObject c = cv.toObject();
            CurveView view;
            view.curve = c["curve"].toString();
            view.channel = c["channel"].toString();
            view.node = o["node"].toString();
            view.component = c["component"].toInt(-1);
            view.color = channel_color(view.channel, view.component);
            for (const QJsonValue& s : c["samples"].toArray()) view.samples.append(QPointF(s[0].toDouble(), s[1].toDouble()));
            for (const QJsonValue& s : c["buffer"].toArray()) view.buffer.append(QPointF(s[0].toDouble(), s[1].toDouble()));
            view.weighted = c["weighted"].toBool();
            for (const QJsonValue& k : c["keys"].toArray()) {
                QJsonArray ka = k.toArray();
                view.keys.append({ka[0].toDouble(), ka[1].toDouble(), ka[2].toDouble(), ka[3].toDouble()});
                view.handles.append({ka[7].toDouble(), ka[8].toDouble(), ka[9].toDouble(), ka[10].toDouble()});
                view.itt.append(ka[4].toString());
                view.ott.append(ka[5].toString());
            }
            key_list.append(view.node + "|" + view.curve);
            curves.append(view);
        }
    }
    // Keep each curve's shown/hidden state.
    for (CurveView& c : curves)
        for (const CurveView& o : canvas_->curves())
            if (o.curve == c.curve) c.visible = o.visible;
    canvas_->set_curves(curves, r["time"].toDouble(1));
    QString key = key_list.join(",");
    if (key != tree_key_) {
        tree_key_ = key;
        rebuild_tree();
    }
}

void GraphEditor::rebuild_tree() {
    tree_->blockSignals(true);
    tree_->clear();
    QHash<QString, QTreeWidgetItem*> nodes;
    for (const CurveView& c : canvas_->curves()) {
        QTreeWidgetItem* n = nodes.value(c.node);
        if (!n) {
            n = new QTreeWidgetItem(tree_, QStringList{c.node.section('|', -1)});
            n->setIcon(0, icon("transform"));
            nodes[c.node] = n;
            n->setExpanded(true);
        }
        auto* it = new QTreeWidgetItem(n, QStringList{c.channel.left(1).toUpper() + c.channel.mid(1)});
        it->setForeground(0, c.color);
        it->setData(0, Qt::UserRole, c.curve);
    }
    tree_->expandAll();
    tree_->blockSignals(false);
}

} // namespace az
