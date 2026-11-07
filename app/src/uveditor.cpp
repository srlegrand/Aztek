#include "uveditor.h"
#include "aztek.h"
#include "host.h"
#include "icons.h"

#include <QActionGroup>
#include <QContextMenuEvent>
#include <QJsonArray>
#include <QKeyEvent>
#include <QLabel>
#include <QMenu>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QTimer>
#include <QToolBar>
#include <QVBoxLayout>
#include <QWheelEvent>
#include <algorithm>
#include <cmath>

namespace az {

static QString id_list(QList<int> ids) {
    std::sort(ids.begin(), ids.end());
    QStringList parts;
    for (int i = 0; i < ids.size();) {
        int j = i;
        while (j + 1 < ids.size() && ids[j + 1] == ids[j] + 1) ++j;
        parts << (j > i ? QString("%1:%2").arg(ids[i]).arg(ids[j]) : QString::number(ids[i]));
        i = j + 1;
    }
    return parts.join(",");
}

// ---------------------------------------------------------------------------------------------------------------------
// Canvas

UvCanvas::UvCanvas(QWidget* parent) : QWidget(parent) {
    setFocusPolicy(Qt::StrongFocus);
    setMouseTracking(true);
    setMinimumSize(240, 240);
}

QPointF UvCanvas::to_screen(QPointF uv) const {
    return QPointF(width() / 2.0 + (uv.x() - center_.x()) * scale_, height() / 2.0 - (uv.y() - center_.y()) * scale_);
}

QPointF UvCanvas::to_uv(QPointF p) const {
    return QPointF(center_.x() + (p.x() - width() / 2.0) / scale_, center_.y() - (p.y() - height() / 2.0) / scale_);
}

void UvCanvas::set_data(const QJsonObject& d) {
    scene_mode_ = d["mode"].toString();
    meshes_.clear();
    for (const QJsonValue& mv : d["meshes"].toArray()) {
        QJsonObject o = mv.toObject();
        UvMesh m;
        m.path = o["path"].toString();
        for (const QJsonValue& p : o["maps"].toArray()) {
            QJsonArray a = p.toArray();
            m.maps.append(QPointF(a[0].toDouble(), a[1].toDouble()));
            m.map_point.append(a[2].toInt());
        }
        for (const QJsonValue& fv : o["faces"].toArray()) {
            QJsonArray a = fv.toArray();
            UvMesh::Face f;
            f.id = a[0].toInt();
            f.shell = a[1].toInt();
            for (const QJsonValue& x : a[2].toArray()) f.maps.append(x.toInt());
            for (const QJsonValue& x : a[3].toArray()) f.edges.append(x.toInt());
            m.faces.append(f);
        }
        for (const QJsonValue& bv : o["borders"].toArray()) {
            QJsonArray a = bv.toArray();
            m.borders.append({a[0].toInt(), a[1].toInt(), a[2].toInt()});
        }
        for (const QJsonValue& x : o["selFaces"].toArray()) m.sel_faces.insert(x.toInt());
        for (const QJsonValue& x : o["selEdges"].toArray()) m.sel_edges.insert(x.toInt());
        for (const QJsonValue& x : o["selPoints"].toArray()) m.sel_points.insert(x.toInt());
        // Selected UVs that no longer exist go.
        if (uv_sel_.contains(m.path)) {
            QSet<int>& s = uv_sel_[m.path];
            for (auto it = s.begin(); it != s.end();) it = *it >= m.maps.size() ? s.erase(it) : std::next(it);
        }
        meshes_.append(m);
    }
    if (!framed_ && !meshes_.isEmpty()) {
        framed_ = true;
        center_ = QPointF(0.5, 0.5);
        scale_ = qMin(width(), height()) * 0.85;
    }
    update();
}

void UvCanvas::set_mode(const QString& m) {
    mode_ = m;
    if (m == "face" || m == "edge") exec("selectMode", QJsonObject{{"args", QJsonArray{m}}});
    update();
}

void UvCanvas::set_tool(const QString& t) {
    // The shell tool is the move tool picking whole shells.
    if (t == "shell") {
        set_mode("shell");
        tool_ = "move";
    } else {
        tool_ = t;
    }
    lasso_.clear();
    path_map_ = -1;
    update();
}

// A left press with one of the tools that are not a manipulator. True when it was taken.
void UvCanvas::tool_press(QMouseEvent* e) {
    QPointF pos = e->position();
    bool ctrl = e->modifiers() & Qt::ControlModifier;
    if (tool_ == "lasso") {
        drag_ = Mode::Lasso;
        lasso_.clear();
        lasso_ << pos;
    } else if (tool_ == "window") {
        drag_ = Mode::Pan;
    } else if (tool_ == "tweak") {
        // The nearest UV, picked and dragged at once.
        double best = 14;
        QString mesh;
        int id = -1;
        for (const UvMesh& m : meshes_)
            for (int i = 0; i < m.maps.size(); ++i) {
                double d = QLineF(to_screen(m.maps[i]), pos).length();
                if (d < best) best = d, mesh = m.path, id = i;
            }
        if (id < 0) return;
        uv_sel_.clear();
        uv_sel_[mesh].insert(id);
        mode_ = "uv";
        drag_ = Mode::Drag;
        handle_ = 3;
        pivot_ = selection_center();
        applied_ = false;
        drag_targets_ = targets();
        emit picked();
        update();
    } else if (tool_ == "cut") {
        // Click an edge: cut it. Ctrl-click: sew it.
        double best = 8;
        QString mesh;
        int edge = -1;
        for (const UvMesh& m : meshes_)
            for (const auto& fc : m.faces)
                for (int k = 0; k < fc.edges.size(); ++k) {
                    QLineF l(to_screen(m.maps[fc.maps[k]]), to_screen(m.maps[fc.maps[(k + 1) % fc.maps.size()]]));
                    double t = qBound(0.0, QPointF::dotProduct(pos - l.p1(), l.p2() - l.p1()) / qMax(1e-9, l.length() * l.length()), 1.0);
                    double d = QLineF(l.pointAt(t), pos).length();
                    if (d < best) best = d, mesh = m.path, edge = fc.edges[k];
                }
        if (edge < 0) return;
        exec(ctrl ? "polyMapSew" : "polyMapCut", QJsonObject{{"args", QJsonArray{QString("%1.e[%2]").arg(mesh).arg(edge)}}});
        Host::get()->poke();
    } else if (tool_ == "path") {
        pick_path(pos);
    } else if (tool_ == "smooth") {
        // A press runs the unfold over the selected UVs, or the meshes shown.
        QJsonArray t = targets();
        exec("u3dUnfold", t.isEmpty() ? QJsonObject() : QJsonObject{{"args", t}});
        Host::get()->poke();
    }
}

// What the lasso drew round, as a UV selection.
void UvCanvas::pick_lasso(Qt::KeyboardModifiers mods) {
    bool ctrl = mods & Qt::ControlModifier, shift = mods & Qt::ShiftModifier;
    if (!ctrl && !shift) uv_sel_.clear();
    mode_ = "uv";
    for (const UvMesh& m : meshes_)
        for (int i = 0; i < m.maps.size(); ++i)
            if (lasso_.containsPoint(to_screen(m.maps[i]), Qt::OddEvenFill)) {
                if (ctrl) uv_sel_[m.path].remove(i);
                else uv_sel_[m.path].insert(i);
            }
    lasso_.clear();
    emit picked();
    update();
}

// The nearest UV: the shortest way along the UV edges from the last one picked is selected.
void UvCanvas::pick_path(QPointF pos) {
    double best = 14;
    int mi = -1, id = -1;
    for (int k = 0; k < meshes_.size(); ++k)
        for (int i = 0; i < meshes_[k].maps.size(); ++i) {
            double d = QLineF(to_screen(meshes_[k].maps[i]), pos).length();
            if (d < best) best = d, mi = k, id = i;
        }
    if (mi < 0) return;
    const UvMesh& m = meshes_[mi];
    mode_ = "uv";
    if (path_mesh_ != m.path || path_map_ < 0) {
        uv_sel_.clear();
        uv_sel_[m.path].insert(id);
    } else {
        // Dijkstra over the maps, by length in UV space.
        QHash<int, QList<int>> next;
        for (const auto& fc : m.faces)
            for (int k = 0; k < fc.maps.size(); ++k) {
                int a = fc.maps[k], b = fc.maps[(k + 1) % fc.maps.size()];
                next[a] << b;
                next[b] << a;
            }
        QHash<int, double> dist{{path_map_, 0.0}};
        QHash<int, int> from;
        QSet<int> done;
        while (true) {
            int cur = -1;
            double low = 1e300;
            for (auto it = dist.begin(); it != dist.end(); ++it)
                if (!done.contains(it.key()) && it.value() < low) low = it.value(), cur = it.key();
            if (cur < 0 || cur == id) break;
            done.insert(cur);
            for (int n : next.value(cur)) {
                double d = low + QLineF(m.maps[cur], m.maps[n]).length();
                if (!dist.contains(n) || d < dist[n]) {
                    dist[n] = d;
                    from[n] = cur;
                }
            }
        }
        QSet<int>& s = uv_sel_[m.path];
        s.insert(id);
        for (int at = id; from.contains(at); at = from[at]) s.insert(from[at]);
    }
    path_mesh_ = m.path;
    path_map_ = id;
    emit picked();
    update();
}

QHash<int, QSet<int>> UvCanvas::active_maps() const {
    QHash<int, QSet<int>> out;
    for (int mi = 0; mi < meshes_.size(); ++mi) {
        const UvMesh& m = meshes_[mi];
        QSet<int> s;
        if (mode_ == "uv" || mode_ == "shell") {
            s = uv_sel_.value(m.path);
        } else if (mode_ == "face") {
            for (const auto& f : m.faces)
                if (m.sel_faces.contains(f.id))
                    for (int x : f.maps) s.insert(x);
        } else if (mode_ == "edge") {
            for (const auto& f : m.faces)
                for (int k = 0; k < f.edges.size(); ++k)
                    if (m.sel_edges.contains(f.edges[k])) {
                        s.insert(f.maps[k]);
                        s.insert(f.maps[(k + 1) % f.maps.size()]);
                    }
        }
        if (!s.isEmpty()) out[mi] = s;
    }
    return out;
}

QJsonArray UvCanvas::targets() const {
    QJsonArray out;
    auto am = active_maps();
    for (auto it = am.begin(); it != am.end(); ++it)
        out.append(QString("%1.map[%2]").arg(meshes_[it.key()].path, id_list(it.value().values())));
    return out;
}

QPointF UvCanvas::selection_center() const {
    auto am = active_maps();
    double lx = 1e18, ly = 1e18, hx = -1e18, hy = -1e18;
    for (auto it = am.begin(); it != am.end(); ++it)
        for (int id : it.value()) {
            QPointF p = meshes_[it.key()].maps[id];
            lx = qMin(lx, p.x()), ly = qMin(ly, p.y()), hx = qMax(hx, p.x()), hy = qMax(hy, p.y());
        }
    if (lx > hx) return QPointF(-1e9, -1e9);
    return QPointF((lx + hx) / 2, (ly + hy) / 2);
}

void UvCanvas::frame(bool selected) {
    double lx = 1e18, ly = 1e18, hx = -1e18, hy = -1e18;
    auto am = active_maps();
    for (int mi = 0; mi < meshes_.size(); ++mi)
        for (int i = 0; i < meshes_[mi].maps.size(); ++i) {
            if (selected && !am.value(mi).contains(i)) continue;
            QPointF p = meshes_[mi].maps[i];
            lx = qMin(lx, p.x()), ly = qMin(ly, p.y()), hx = qMax(hx, p.x()), hy = qMax(hy, p.y());
        }
    if (lx > hx) {
        if (selected) return frame(false);
        lx = ly = 0, hx = hy = 1;
    }
    center_ = QPointF((lx + hx) / 2, (ly + hy) / 2);
    double span = qMax(qMax(hx - lx, hy - ly), 0.05);
    scale_ = qMin(width(), height()) * 0.85 / span;
    update();
}

void UvCanvas::set_image(const QString& path, QPointF repeat, QPointF offset) {
    image_repeat_ = repeat;
    image_offset_ = offset;
    if (path == image_path_) return;
    image_path_ = path;
    image_ = QImage();
    if (path.isEmpty()) return;
    int w = 0, h = 0;
    QByteArray utf = path.toUtf8();
    if (unsigned char* data = aztek_image_load(utf.constData(), &w, &h)) {
        image_ = QImage(data, w, h, QImage::Format_RGBA8888).copy();
        aztek_image_free(data, w, h);
    }
}

void UvCanvas::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.fillRect(rect(), QColor(64, 64, 64));
    // The 0-1 square: checker or plain, with the tenths grid.
    QRectF unit(to_screen(QPointF(0, 1)), to_screen(QPointF(1, 0)));
    if (show_checker) {
        int n = 8;
        for (int i = 0; i < n; ++i)
            for (int j = 0; j < n; ++j) {
                QRectF c(unit.left() + unit.width() * i / n, unit.top() + unit.height() * j / n, unit.width() / n, unit.height() / n);
                p.fillRect(c, (i + j) % 2 ? QColor(150, 150, 150) : QColor(105, 105, 105));
            }
    } else {
        p.fillRect(unit, QColor(78, 78, 78));
    }
    // The image: one tile per repeat over the 0-1 square, as the material maps it.
    if (show_image && !image_.isNull() && image_repeat_.x() > 0 && image_repeat_.y() > 0) {
        double tu = 1.0 / image_repeat_.x(), tv = 1.0 / image_repeat_.y();
        double u0 = -image_offset_.x() * tu, v0 = -image_offset_.y() * tv;
        p.save();
        p.setClipRect(unit);
        p.setRenderHint(QPainter::SmoothPixmapTransform);
        for (double v = v0 - std::ceil(v0 / tv) * tv; v < 1.0; v += tv)
            for (double u = u0 - std::ceil(u0 / tu) * tu; u < 1.0; u += tu)
                p.drawImage(QRectF(to_screen(QPointF(u, v + tv)), to_screen(QPointF(u + tu, v))), image_);
        p.restore();
    }
    QPointF lo = to_uv(QPointF(0, height())), hi = to_uv(QPointF(width(), 0));
    double step = scale_ > 150 ? 0.1 : 1.0;
    for (double u = std::floor(lo.x() / step) * step; u <= hi.x(); u += step) {
        bool whole = std::abs(u - std::round(u)) < 1e-6;
        p.setPen(whole ? QColor(40, 40, 40) : QColor(86, 86, 86));
        double x = to_screen(QPointF(u, 0)).x();
        p.drawLine(QPointF(x, 0), QPointF(x, height()));
    }
    for (double v = std::floor(lo.y() / step) * step; v <= hi.y(); v += step) {
        bool whole = std::abs(v - std::round(v)) < 1e-6;
        p.setPen(whole ? QColor(40, 40, 40) : QColor(86, 86, 86));
        double y = to_screen(QPointF(0, v)).y();
        p.drawLine(QPointF(0, y), QPointF(width(), y));
    }
    p.setPen(QPen(QColor(25, 25, 25), 1.5));
    p.setBrush(Qt::NoBrush);
    p.drawRect(unit);
    QFont f = font();
    f.setPointSizeF(qMax(6.5, f.pointSizeF() - 1));
    p.setFont(f);
    p.setPen(QColor(160, 160, 160));
    p.drawText(unit.bottomLeft() + QPointF(2, 12), "0");
    p.drawText(unit.bottomRight() + QPointF(2, 12), "1");
    p.drawText(unit.topLeft() + QPointF(-10, 4), "1");

    p.setRenderHint(QPainter::Antialiasing);
    auto am = active_maps();
    for (int mi = 0; mi < meshes_.size(); ++mi) {
        const UvMesh& m = meshes_[mi];
        // Faces.
        for (const auto& fc : m.faces) {
            QPolygonF poly;
            for (int id : fc.maps) poly << to_screen(m.maps[id]);
            bool sel = mode_ == "face" && m.sel_faces.contains(fc.id);
            if (sel) {
                p.setPen(Qt::NoPen);
                p.setBrush(QColor(235, 140, 40, 130));
                p.drawPolygon(poly);
            } else if (shaded) {
                p.setPen(Qt::NoPen);
                p.setBrush(QColor(60, 105, 180, 70));
                p.drawPolygon(poly);
            }
            p.setPen(QPen(QColor(170, 175, 195), 1));
            p.setBrush(Qt::NoBrush);
            p.drawPolygon(poly);
        }
        // Where UVs split: thicker, as the reference's texture borders.
        p.setPen(QPen(QColor(235, 235, 245), 2));
        for (auto& b : m.borders) p.drawLine(to_screen(m.maps[b[0]]), to_screen(m.maps[b[1]]));
        if (mode_ == "edge") {
            p.setPen(QPen(QColor(255, 150, 30), 2.5));
            for (const auto& fc : m.faces)
                for (int k = 0; k < fc.edges.size(); ++k)
                    if (m.sel_edges.contains(fc.edges[k])) p.drawLine(to_screen(m.maps[fc.maps[k]]), to_screen(m.maps[fc.maps[(k + 1) % fc.maps.size()]]));
        }
        if (mode_ == "uv" || mode_ == "shell") {
            const QSet<int> s = am.value(mi);
            for (int i = 0; i < m.maps.size(); ++i) {
                bool on = s.contains(i);
                p.setPen(Qt::NoPen);
                p.setBrush(on ? QColor(80, 240, 80) : QColor(190, 120, 230));
                double r = on ? 3 : 2;
                p.drawEllipse(to_screen(m.maps[i]), r, r);
            }
        }
    }
    // The manipulator at the selection's middle.
    QPointF c = selection_center();
    if (c.x() > -1e8 && tool_ != "select") {
        QPointF o = to_screen(c);
        p.setRenderHint(QPainter::Antialiasing);
        if (tool_ == "move") {
            p.setPen(QPen(QColor(230, 60, 60), 2));
            p.drawLine(o, o + QPointF(60, 0));
            p.setBrush(QColor(230, 60, 60));
            p.drawPolygon(QPolygonF({o + QPointF(66, 0), o + QPointF(56, -5), o + QPointF(56, 5)}));
            p.setPen(QPen(QColor(80, 220, 80), 2));
            p.drawLine(o, o + QPointF(0, -60));
            p.setBrush(QColor(80, 220, 80));
            p.drawPolygon(QPolygonF({o + QPointF(0, -66), o + QPointF(-5, -56), o + QPointF(5, -56)}));
            p.setPen(QPen(QColor(240, 220, 60), 1.5));
            p.setBrush(Qt::NoBrush);
            p.drawRect(QRectF(o.x() - 6, o.y() - 6, 12, 12));
        } else if (tool_ == "rotate") {
            p.setPen(QPen(QColor(80, 140, 240), 2));
            p.setBrush(Qt::NoBrush);
            p.drawEllipse(o, 50, 50);
        } else if (tool_ == "scale") {
            p.setPen(QPen(QColor(230, 60, 60), 2));
            p.drawLine(o, o + QPointF(60, 0));
            p.fillRect(QRectF(o.x() + 56, o.y() - 4, 8, 8), QColor(230, 60, 60));
            p.setPen(QPen(QColor(80, 220, 80), 2));
            p.drawLine(o, o + QPointF(0, -60));
            p.fillRect(QRectF(o.x() - 4, o.y() - 64, 8, 8), QColor(80, 220, 80));
            p.fillRect(QRectF(o.x() - 6, o.y() - 6, 12, 12), QColor(240, 220, 60));
        }
    }
    if (drag_ == Mode::Marquee) {
        p.setPen(QPen(QColor(230, 230, 230), 1, Qt::DashLine));
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(press_, last_).normalized());
    }
    if (meshes_.isEmpty()) {
        p.setPen(QColor(170, 170, 170));
        p.drawText(rect(), Qt::AlignCenter, "Select a mesh to see its UVs.");
    }
}

int UvCanvas::handle_at(QPointF pos) const {
    QPointF c = selection_center();
    if (c.x() < -1e8 || tool_ == "select") return -1;
    QPointF o = to_screen(c);
    QPointF d = pos - o;
    if (tool_ == "rotate") return std::abs(std::hypot(d.x(), d.y()) - 50) < 8 || std::hypot(d.x(), d.y()) < 50 ? 3 : -1;
    if (std::abs(d.x()) < 8 && std::abs(d.y()) < 8) return 3;
    if (d.x() > 4 && d.x() < 70 && std::abs(d.y()) < 7) return 1;
    if (-d.y() > 4 && -d.y() < 70 && std::abs(d.x()) < 7) return 2;
    return -1;
}

static bool inside(const QPolygonF& poly, QPointF p) {
    return poly.containsPoint(p, Qt::OddEvenFill);
}

static double seg_dist(QPointF p, QPointF a, QPointF b) {
    QPointF ab = b - a;
    double l2 = ab.x() * ab.x() + ab.y() * ab.y();
    double t = l2 < 1e-12 ? 0 : qBound(0.0, QPointF::dotProduct(p - a, ab) / l2, 1.0);
    QPointF q = a + ab * t;
    return std::hypot(p.x() - q.x(), p.y() - q.y());
}

void UvCanvas::pick(QPointF pos, QPointF to, bool rect, Qt::KeyboardModifiers mods) {
    bool shift = mods & Qt::ShiftModifier, ctrl = mods & Qt::ControlModifier;
    QRectF r = QRectF(pos, to).normalized();
    if (mode_ == "uv" || mode_ == "shell") {
        QHash<QString, QSet<int>> hit;
        for (const UvMesh& m : meshes_) {
            if (mode_ == "uv") {
                int best = -1;
                double bd = 8;
                for (int i = 0; i < m.maps.size(); ++i) {
                    QPointF s = to_screen(m.maps[i]);
                    if (rect) {
                        if (r.contains(s)) hit[m.path].insert(i);
                    } else {
                        double d = std::hypot(s.x() - pos.x(), s.y() - pos.y());
                        if (d < bd) bd = d, best = i;
                    }
                }
                if (best >= 0) hit[m.path] = {best};
            } else {
                QSet<int> shells;
                for (const auto& fc : m.faces) {
                    QPolygonF poly;
                    for (int id : fc.maps) poly << to_screen(m.maps[id]);
                    if (rect ? r.intersects(poly.boundingRect()) : inside(poly, pos)) shells.insert(fc.shell);
                }
                for (const auto& fc : m.faces)
                    if (shells.contains(fc.shell))
                        for (int id : fc.maps) hit[m.path].insert(id);
            }
        }
        // A single click on UV takes the nearest across meshes only.
        if (!shift && !ctrl) uv_sel_.clear();
        for (auto it = hit.begin(); it != hit.end(); ++it) {
            QSet<int>& s = uv_sel_[it.key()];
            for (int id : it.value()) {
                if (ctrl) s.remove(id);
                else if (shift && !ctrl && !rect && s.contains(id)) s.remove(id);
                else s.insert(id);
            }
        }
        update();
        emit picked();
        return;
    }
    // Faces and edges are the scene's selection.
    QJsonArray names;
    for (const UvMesh& m : meshes_) {
        QList<int> ids;
        if (mode_ == "face") {
            for (const auto& fc : m.faces) {
                QPolygonF poly;
                for (int id : fc.maps) poly << to_screen(m.maps[id]);
                if (rect ? r.contains(poly.boundingRect().center()) : inside(poly, pos)) ids << fc.id;
            }
            if (!rect && ids.size() > 1) ids = {ids.last()};
        } else {
            int best = -1;
            double bd = 7;
            for (const auto& fc : m.faces)
                for (int k = 0; k < fc.edges.size(); ++k) {
                    QPointF a = to_screen(m.maps[fc.maps[k]]), b = to_screen(m.maps[fc.maps[(k + 1) % fc.maps.size()]]);
                    if (rect) {
                        if (r.contains((a + b) / 2) && !ids.contains(fc.edges[k])) ids << fc.edges[k];
                    } else {
                        double d = seg_dist(pos, a, b);
                        if (d < bd) bd = d, best = fc.edges[k];
                    }
                }
            if (best >= 0) ids = {best};
        }
        if (!ids.isEmpty()) names.append(QString("%1.%2[%3]").arg(m.path, mode_ == "face" ? "f" : "e", id_list(ids)));
    }
    QJsonObject call{{"args", names}};
    if (ctrl && shift) call["add"] = true;
    else if (ctrl) call["deselect"] = true;
    else if (shift) call["toggle"] = true;
    if (names.isEmpty()) {
        if (shift || ctrl) return;
        // Nothing hit: keep the objects, drop the components.
        QJsonArray objs;
        for (const UvMesh& m : meshes_) objs.append(m.path);
        exec("select", QJsonObject{{"args", objs}});
        exec("selectMode", QJsonObject{{"args", QJsonArray{mode_}}});
        return;
    }
    exec("select", call);
}

void UvCanvas::mousePressEvent(QMouseEvent* e) {
    press_ = last_ = e->position();
    bool alt = e->modifiers() & Qt::AltModifier;
    if (alt && e->button() == Qt::MiddleButton) {
        drag_ = Mode::Pan;
        return;
    }
    if (alt && e->button() == Qt::RightButton) {
        drag_ = Mode::Zoom;
        return;
    }
    if (e->button() == Qt::LeftButton && (tool_ == "lasso" || tool_ == "window" || tool_ == "tweak" || tool_ == "cut" || tool_ == "path" || tool_ == "smooth")) {
        tool_press(e);
        return;
    }
    if (e->button() == Qt::LeftButton || e->button() == Qt::MiddleButton) {
        int h = handle_at(e->position());
        bool mid = e->button() == Qt::MiddleButton;
        if ((h >= 0 || mid) && tool_ != "select" && !active_maps().isEmpty()) {
            drag_ = Mode::Drag;
            handle_ = mid ? (h > 0 ? h : 3) : h;
            pivot_ = selection_center();
            applied_ = false;
            drag_targets_ = targets();
            return;
        }
        if (e->button() == Qt::LeftButton) drag_ = Mode::Marquee;
    }
}

void UvCanvas::drag_to(QPointF pos, bool final) {
    if (applied_) aztek_retract();
    applied_ = false;
    QJsonObject call{{"args", drag_targets_}};
    QPointF d = to_uv(pos) - to_uv(press_);
    double px = pos.x() - press_.x();
    if (tool_ == "move" || tool_ == "tweak") {
        if (handle_ == 1) d.setY(0);
        if (handle_ == 2) d.setX(0);
        if (std::abs(d.x()) < 1e-9 && std::abs(d.y()) < 1e-9) return;
        call["u"] = d.x();
        call["v"] = d.y();
    } else if (tool_ == "rotate") {
        double a = -px * 0.5;
        if (std::abs(a) < 1e-9) return;
        call["angle"] = a;
        call["pivotU"] = pivot_.x();
        call["pivotV"] = pivot_.y();
    } else if (tool_ == "scale") {
        double s = std::exp(px * 0.01);
        call["scaleU"] = handle_ == 2 ? 1.0 : s;
        call["scaleV"] = handle_ == 1 ? 1.0 : s;
        call["pivotU"] = pivot_.x();
        call["pivotV"] = pivot_.y();
    }
    QJsonValue r = final ? exec("polyEditUV", call) : exec_quiet("polyEditUV", call);
    applied_ = !final && !r.isUndefined();
    if (!final) {
        // Show the preview now rather than at the next poll.
        Host::get()->poke();
    }
}

void UvCanvas::mouseMoveEvent(QMouseEvent* e) {
    QPointF pos = e->position();
    QPointF d = pos - last_;
    switch (drag_) {
    case Mode::Pan:
        center_ -= QPointF(d.x() / scale_, -d.y() / scale_);
        update();
        break;
    case Mode::Zoom: {
        double f = std::exp((d.x() - d.y()) * 0.01);
        scale_ = qBound(5.0, scale_ * f, 1e6);
        update();
        break;
    }
    case Mode::Marquee: update(); break;
    case Mode::Lasso:
        if (lasso_.isEmpty() || QLineF(lasso_.last(), pos).length() > 2) lasso_ << pos;
        update();
        break;
    case Mode::Drag: drag_to(pos, false); break;
    default: break;
    }
    last_ = pos;
}

void UvCanvas::mouseReleaseEvent(QMouseEvent* e) {
    if (drag_ == Mode::Lasso) {
        pick_lasso(e->modifiers());
    } else if (drag_ == Mode::Marquee) {
        bool rect = QLineF(press_, e->position()).length() > 4;
        pick(press_, e->position(), rect, e->modifiers());
    } else if (drag_ == Mode::Drag) {
        if (applied_) {
            aztek_retract();
            applied_ = false;
            drag_to(e->position(), true);
        }
    }
    drag_ = Mode::None;
    update();
}

void UvCanvas::wheelEvent(QWheelEvent* e) {
    QPointF at = e->position();
    QPointF before = to_uv(at);
    scale_ = qBound(5.0, scale_ * std::exp(e->angleDelta().y() / 120.0 * 0.15), 1e6);
    QPointF after = to_uv(at);
    center_ += before - after;
    update();
}

void UvCanvas::keyPressEvent(QKeyEvent* e) {
    switch (e->key()) {
    case Qt::Key_F: frame(true); return;
    case Qt::Key_A: frame(false); return;
    case Qt::Key_Q: set_tool("select"); return;
    case Qt::Key_W: set_tool("move"); return;
    case Qt::Key_E: set_tool("rotate"); return;
    case Qt::Key_R: set_tool("scale"); return;
    case Qt::Key_F10: set_mode("edge"); return;
    case Qt::Key_F11: set_mode("face"); return;
    case Qt::Key_F12: set_mode(e->modifiers() & Qt::ShiftModifier ? "shell" : "uv"); return;
    default: QWidget::keyPressEvent(e);
    }
}

void UvCanvas::contextMenuEvent(QContextMenuEvent* e) {
    if (e->modifiers() & Qt::AltModifier) return;
    QMenu m(this);
    for (auto [label, mode] : {std::pair{"UV", "uv"}, {"UV Shell", "shell"}, {"Edge", "edge"}, {"Face", "face"}}) {
        QAction* a = m.addAction(label, [this, mode = QString(mode)] { set_mode(mode); });
        a->setCheckable(true);
        a->setChecked(mode_ == mode);
    }
    m.exec(e->globalPos());
}

// ---------------------------------------------------------------------------------------------------------------------
// Editor

// Every UV Editor there is, and the tool they are on.
static QList<UvEditor*>& uv_editors() {
    static QList<UvEditor*> all;
    return all;
}
static QString g_uv_tool = "move";

void UvEditor::set_tool_everywhere(const QString& t) {
    g_uv_tool = t;
    for (UvEditor* e : uv_editors()) e->show_tool(t);
}

void UvEditor::show_tool(const QString& t) {
    canvas_->set_tool(t);
    QString shown = t == "shell" ? "move" : t;
    if (QAction* a = tool_buttons_.value(shown)) {
        a->setChecked(true);
    } else {
        // A tool with no button: none of the four stays lit.
        for (QAction* b : tool_buttons_) {
            if (b->actionGroup()) b->actionGroup()->setExclusive(false);
            b->setChecked(false);
            if (b->actionGroup()) b->actionGroup()->setExclusive(true);
        }
    }
}

void UvEditor::apply_flags(const QJsonObject& p) {
    if (p.contains("displayCheckered")) {
        canvas_->show_checker = p["displayCheckered"].toBool();
        canvas_->update();
    }
    if (p.contains("displayUVShaded")) {
        canvas_->shaded = p["displayUVShaded"].toBool();
        canvas_->update();
    }
    if (p["frameAll"].toBool()) canvas_->frame(false);
    if (p["frameSelected"].toBool()) canvas_->frame(true);
}

UvEditor::UvEditor(QWidget* parent) : QWidget(parent) {
    uv_editors().append(this);
    connect(this, &QObject::destroyed, [this] { uv_editors().removeOne(this); });
    // Made after a context chose a tool: on that tool.
    QTimer::singleShot(0, this, [this] {
        if (g_uv_tool != "move") show_tool(g_uv_tool);
    });
    setWindowTitle("UV Editor");
    auto* v = new QVBoxLayout(this);
    v->setContentsMargins(0, 0, 0, 0);
    v->setSpacing(0);
    canvas_ = new UvCanvas(this);
    auto* bar = new QToolBar(this);
    auto* modes = new QActionGroup(this);
    for (auto [label, mode] : {std::pair{"UV", "uv"}, {"Shell", "shell"}, {"Edge", "edge"}, {"Face", "face"}}) {
        QAction* a = bar->addAction(label, [this, mode = QString(mode)] { canvas_->set_mode(mode); });
        a->setCheckable(true);
        a->setChecked(QString(mode) == "uv");
        modes->addAction(a);
        mode_buttons_[mode] = a;
    }
    bar->addSeparator();
    auto* tools = new QActionGroup(this);
    for (auto [label, tool] : {std::pair{"select", "select"}, {"move", "move"}, {"rotate", "rotate"}, {"scale", "scale"}}) {
        QAction* a = bar->addAction(icon(label), QString(label), [this, tool = QString(tool)] { canvas_->set_tool(tool); });
        a->setCheckable(true);
        a->setChecked(QString(tool) == "move");
        tools->addAction(a);
        tool_buttons_.insert(tool, a);
    }
    bar->addSeparator();
    bar->addAction("Planar", [this] { run("polyProjection", {{"type", "planar"}, {"mapDirection", "b"}}); });
    bar->addAction("Automatic", [this] { run("polyAutoProjection"); });
    bar->addAction("Cylindrical", [this] { run("polyProjection", {{"type", "cylindrical"}}); });
    bar->addAction("Spherical", [this] { run("polyProjection", {{"type", "spherical"}}); });
    bar->addSeparator();
    bar->addAction("Unfold", [this] { run("u3dUnfold"); });
    bar->addAction("Layout", [this] { run("u3dLayout"); });
    bar->addAction("Cut", [this] { run("polyMapCut"); });
    bar->addAction("Sew", [this] { run("polyMapSew"); });
    bar->addAction("Move and Sew", [this] { run("polyMapSewMove"); });
    bar->addSeparator();
    bar->addAction("Normalize", [this] { run("polyNormalizeUV", {{"normalizeType", 1}}); });
    bar->addAction("Flip U", [this] { run("polyFlipUV", {{"flipType", 0}}); });
    bar->addAction("Flip V", [this] { run("polyFlipUV", {{"flipType", 1}}); });
    bar->addAction("-90", [this] { run("polyEditUV", {{"angle", -90.0}}); });
    bar->addAction("+90", [this] { run("polyEditUV", {{"angle", 90.0}}); });
    bar->addSeparator();
    QAction* display_image = bar->addAction("Image", [this](bool on) {
        canvas_->show_image = on;
        canvas_->update();
    });
    display_image->setCheckable(true);
    display_image->setChecked(true);
    display_image->setToolTip("Display Image: the material's texture behind the UVs");
    QAction* checker = bar->addAction("Checker", [this](bool on) {
        canvas_->show_checker = on;
        canvas_->update();
    });
    checker->setCheckable(true);
    QAction* shade = bar->addAction("Shaded", [this](bool on) {
        canvas_->shaded = on;
        canvas_->update();
    });
    shade->setCheckable(true);
    shade->setChecked(true);
    v->addWidget(bar);
    v->addWidget(canvas_, 1);
    connect(Host::get(), &Host::sceneChanged, this, [this] {
        if (isVisible()) refresh();
    });
    resize(760, 720);
}

void UvEditor::run(const QString& cmd, const QJsonObject& call, bool on_selection) {
    QJsonObject c = call;
    QJsonArray t = canvas_->targets();
    if (on_selection && !t.isEmpty()) c["args"] = t;
    if (cmd == "polyEditUV" && c.contains("angle")) {
        // Turns go about the middle of what turns.
        QJsonArray a{c["angle"]};
        for (const QJsonValue& x : t) a.append(x);
        exec("polyRotateUVs", QJsonObject{{"args", a}});
        return;
    }
    exec(cmd, c);
}

void UvEditor::refresh() {
    canvas_->set_data(query("uvData").toObject());
    QJsonObject t = query("uvTexture").toObject();
    QJsonArray r = t["repeat"].toArray(), o = t["offset"].toArray();
    canvas_->set_image(t["path"].toString(), r.isEmpty() ? QPointF(1, 1) : QPointF(r.at(0).toDouble(), r.at(1).toDouble()), o.isEmpty() ? QPointF(0, 0) : QPointF(o.at(0).toDouble(), o.at(1).toDouble()));
    canvas_->update();
}

} // namespace az
