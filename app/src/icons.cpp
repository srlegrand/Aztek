#include "icons.h"

#include <QHash>
#include <QPainter>
#include <QPainterPath>
#include <QPixmap>
#include <cmath>

namespace az {

bool draw_sculpt_icon(QPainter& p, const QString& tool);


bool draw_more(const QString& name, QPainter& p);

namespace {

const QColor kInk(215, 215, 215);
const QColor kBlue(95, 160, 220);
const QColor kOrange(240, 160, 50);
const QColor kGreen(110, 200, 120);
const QColor kRed(220, 90, 80);
const QColor kFill(150, 170, 190);

// A cube in a simple oblique view, on a 32 unit canvas.
void cube(QPainter& p, QRectF r, const QColor& c, bool fill = true) {
    double s = r.width(), x = r.left(), y = r.top();
    QPointF a(x + s * 0.2, y + s * 0.38), b(x + s * 0.62, y + s * 0.38), cc(x + s * 0.62, y + s * 0.85), d(x + s * 0.2, y + s * 0.85);
    QPointF o(s * 0.18, -s * 0.2);
    if (fill) {
        p.setBrush(c.darker(160));
        p.drawPolygon(QPolygonF({a, b, cc, d}));
        p.setBrush(c.darker(120));
        p.drawPolygon(QPolygonF({a, a + o, b + o, b}));
        p.setBrush(c.darker(200));
        p.drawPolygon(QPolygonF({b, b + o, cc + o, cc}));
    } else {
        p.setBrush(Qt::NoBrush);
        p.drawPolygon(QPolygonF({a, b, cc, d}));
        p.drawPolyline(QPolygonF({a, a + o, b + o, b}));
        p.drawPolyline(QPolygonF({b + o, cc + o, cc}));
    }
}

void arrow(QPainter& p, QPointF from, QPointF to, double head) {
    p.drawLine(from, to);
    QPointF d = to - from;
    double l = std::hypot(d.x(), d.y());
    if (l < 1e-6) return;
    QPointF u = d / l, n(-u.y(), u.x());
    QPolygonF h({to, to - u * head + n * head * 0.5, to - u * head - n * head * 0.5});
    p.setBrush(p.pen().color());
    p.drawPolygon(h);
}

QPixmap draw(const QString& name, int size) {
    QPixmap pm(size, size);
    pm.fill(Qt::transparent);
    QPainter p(&pm);
    p.setRenderHint(QPainter::Antialiasing);
    p.scale(size / 32.0, size / 32.0);
    QPen pen(kInk, 1.6, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin);
    p.setPen(pen);
    QRectF full(0, 0, 32, 32);
    auto pen_color = [&](const QColor& c, double w = 1.6) { p.setPen(QPen(c, w, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin)); };

    // brushSculpt, brushSmooth and the rest: the sculpting tools.
    if (name.startsWith("brush") && name.size() > 5 && draw_sculpt_icon(p, name.mid(5).toLower())) return pm;

    if (name == "select") {
        QPolygonF a({{9, 4}, {9, 25}, {14, 20}, {18, 28}, {21, 27}, {17, 19}, {24, 19}});
        p.setBrush(kInk);
        p.setPen(QPen(QColor(40, 40, 40), 1.2));
        p.drawPolygon(a);
    } else if (name == "lasso") {
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(5, 6, 22, 14));
        p.drawLine(QPointF(10, 19), QPointF(8, 27));
    } else if (name == "move") {
        pen_color(kRed);
        arrow(p, {16, 16}, {29, 16}, 6);
        pen_color(kGreen);
        arrow(p, {16, 16}, {16, 3}, 6);
        pen_color(kBlue);
        arrow(p, {16, 16}, {6, 26}, 6);
    } else if (name == "rotate") {
        p.setBrush(Qt::NoBrush);
        pen_color(kRed);
        p.drawEllipse(QRectF(5, 12, 22, 8));
        pen_color(kGreen);
        p.drawEllipse(QRectF(12, 5, 8, 22));
        pen_color(kBlue);
        p.drawEllipse(QRectF(5, 5, 22, 22));
    } else if (name == "scale") {
        pen_color(kRed);
        p.drawLine(QPointF(16, 16), QPointF(27, 16));
        p.setBrush(kRed);
        p.drawRect(QRectF(25, 14, 5, 5));
        pen_color(kGreen);
        p.drawLine(QPointF(16, 16), QPointF(16, 5));
        p.setBrush(kGreen);
        p.drawRect(QRectF(14, 2, 5, 5));
        pen_color(kBlue);
        p.drawLine(QPointF(16, 16), QPointF(7, 25));
        p.setBrush(kBlue);
        p.drawRect(QRectF(4, 23, 5, 5));
        p.setBrush(kOrange);
        pen_color(kOrange);
        p.drawRect(QRectF(13.5, 13.5, 5, 5));
    } else if (name == "lastTool") {
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(6, 6, 20, 20));
        p.drawText(full, Qt::AlignCenter, "T");
    } else if (name == "cube" || name == "mesh") {
        pen_color(kInk, 1.0);
        cube(p, QRectF(2, 2, 28, 28), kFill);
    } else if (name == "sphere") {
        QRadialGradient g(QPointF(12, 11), 16);
        g.setColorAt(0, kFill.lighter(130));
        g.setColorAt(1, kFill.darker(220));
        p.setBrush(g);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QRectF(4, 4, 24, 24));
    } else if (name == "cylinder") {
        p.setBrush(kFill.darker(170));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(7, 8, 18, 17));
        p.drawEllipse(QRectF(7, 21, 18, 7));
        p.setBrush(kFill.darker(110));
        p.drawEllipse(QRectF(7, 4, 18, 7));
    } else if (name == "cone") {
        p.setBrush(kFill.darker(160));
        p.setPen(Qt::NoPen);
        p.drawPolygon(QPolygonF({{16, 3}, {26, 25}, {6, 25}}));
        p.drawEllipse(QRectF(6, 21, 20, 7));
    } else if (name == "plane") {
        p.setBrush(kFill.darker(140));
        pen_color(kInk, 1.0);
        QPolygonF q({{3, 20}, {17, 12}, {29, 16}, {15, 25}});
        p.drawPolygon(q);
        p.drawLine(QPointF(10, 16), QPointF(22, 20.5));
        p.drawLine(QPointF(9, 22.5), QPointF(23, 14));
    } else if (name == "torus") {
        p.setBrush(kFill.darker(150));
        p.setPen(Qt::NoPen);
        QPainterPath path;
        path.addEllipse(QRectF(3, 8, 26, 16));
        path.addEllipse(QRectF(11, 13, 10, 6));
        path.setFillRule(Qt::OddEvenFill);
        p.drawPath(path);
    } else if (name == "disc") {
        p.setBrush(kFill.darker(140));
        p.setPen(Qt::NoPen);
        p.drawEllipse(QRectF(3, 10, 26, 13));
    } else if (name == "pipe") {
        p.setBrush(kFill.darker(170));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(6, 8, 20, 16));
        p.drawEllipse(QRectF(6, 20, 20, 8));
        p.setBrush(kFill.darker(110));
        p.drawEllipse(QRectF(6, 4, 20, 8));
        p.setBrush(QColor(40, 40, 40));
        p.drawEllipse(QRectF(11, 5.5, 10, 5));
    } else if (name == "new") {
        p.setBrush(QColor(230, 230, 230));
        p.drawPolygon(QPolygonF({{7, 3}, {20, 3}, {25, 8}, {25, 29}, {7, 29}}));
    } else if (name == "open") {
        p.setBrush(QColor(210, 170, 80));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(3, 9, 26, 18));
        p.drawRect(QRectF(3, 6, 10, 4));
        p.setBrush(QColor(240, 200, 110));
        p.drawPolygon(QPolygonF({{7, 14}, {31, 14}, {27, 27}, {3, 27}}));
    } else if (name == "save") {
        p.setBrush(kBlue.darker(130));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(4, 4, 24, 24));
        p.setBrush(QColor(230, 230, 230));
        p.drawRect(QRectF(9, 4, 14, 8));
        p.drawRect(QRectF(8, 17, 16, 11));
    } else if (name == "undo" || name == "redo") {
        p.setBrush(Qt::NoBrush);
        if (name == "redo") {
            p.translate(32, 0);
            p.scale(-1, 1);
        }
        QPainterPath path;
        path.moveTo(9, 13);
        path.cubicTo(18, 6, 29, 10, 26, 24);
        p.drawPath(path);
        p.setBrush(kInk);
        p.drawPolygon(QPolygonF({{4, 13}, {12, 8}, {12, 18}}));
    } else if (name == "object") {
        pen_color(kInk, 1.0);
        cube(p, QRectF(2, 2, 28, 28), kFill);
    } else if (name == "vertex" || name == "edge" || name == "face" || name == "uv") {
        pen_color(QColor(170, 170, 200), 1.0);
        cube(p, QRectF(2, 2, 28, 28), kFill, name == "face");
        if (name == "vertex") {
            p.setBrush(QColor(200, 80, 200));
            p.setPen(Qt::NoPen);
            for (QPointF c : {QPointF(7.6, 12.6), QPointF(19.4, 12.6), QPointF(19.4, 25.8), QPointF(7.6, 25.8), QPointF(12.6, 7), QPointF(24.4, 7), QPointF(24.4, 20.2)})
                p.drawEllipse(c, 2.4, 2.4);
        } else if (name == "edge") {
            pen_color(kOrange, 2.4);
            p.drawLine(QPointF(19.4, 12.6), QPointF(19.4, 25.8));
        } else if (name == "uv") {
            pen_color(kGreen, 1.0);
            p.drawText(full, Qt::AlignCenter, "UV");
        }
    } else if (name == "history") {
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(5, 5, 22, 22));
        p.drawLine(QPointF(16, 16), QPointF(16, 9));
        p.drawLine(QPointF(16, 16), QPointF(21, 19));
    } else if (name == "single" || name == "four" || name == "perspOutliner") {
        p.setBrush(Qt::NoBrush);
        pen_color(kInk, 1.4);
        p.drawRect(QRectF(4, 6, 24, 20));
        if (name == "four") {
            p.drawLine(QPointF(16, 6), QPointF(16, 26));
            p.drawLine(QPointF(4, 16), QPointF(28, 16));
        } else if (name == "perspOutliner") {
            p.drawLine(QPointF(11, 6), QPointF(11, 26));
        }
    } else if (name == "transform") {
        pen_color(kRed, 1.4);
        p.drawLine(QPointF(16, 16), QPointF(27, 16));
        pen_color(kGreen, 1.4);
        p.drawLine(QPointF(16, 16), QPointF(16, 5));
        pen_color(kBlue, 1.4);
        p.drawLine(QPointF(16, 16), QPointF(8, 24));
    } else if (name == "group") {
        p.setBrush(QColor(200, 200, 200));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(5, 5, 9, 9));
        p.drawRect(QRectF(18, 5, 9, 9));
        p.drawRect(QRectF(5, 18, 9, 9));
        p.drawRect(QRectF(18, 18, 9, 9));
    } else if (name == "locator") {
        pen_color(kInk, 1.6);
        p.drawLine(QPointF(16, 3), QPointF(16, 29));
        p.drawLine(QPointF(3, 16), QPointF(29, 16));
        p.drawLine(QPointF(8, 24), QPointF(24, 8));
    } else if (name == "pointLight" || name == "ambientLight" || name == "lightsAll") {
        const QColor sun(240, 215, 110);
        pen_color(sun, 1.6);
        p.setBrush(name == "ambientLight" ? Qt::NoBrush : QBrush(sun));
        p.drawEllipse(QPointF(16, 16), 6, 6);
        for (int i = 0; i < 8; ++i) {
            const double a = i * 3.14159265 / 4;
            p.drawLine(QPointF(16 + 9 * std::cos(a), 16 + 9 * std::sin(a)), QPointF(16 + 13 * std::cos(a), 16 + 13 * std::sin(a)));
        }
    } else if (name == "directionalLight") {
        pen_color(QColor(240, 215, 110), 1.6);
        for (int x : {8, 16, 24}) {
            p.drawLine(QPointF(x, 5), QPointF(x, 26));
            p.drawLine(QPointF(x, 26), QPointF(x - 3, 20));
            p.drawLine(QPointF(x, 26), QPointF(x + 3, 20));
        }
    } else if (name == "spotLight") {
        pen_color(QColor(240, 215, 110), 1.6);
        p.drawLine(QPointF(16, 4), QPointF(5, 25));
        p.drawLine(QPointF(16, 4), QPointF(27, 25));
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(5, 21, 22, 7));
    } else if (name == "areaLight") {
        pen_color(QColor(240, 215, 110), 1.6);
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(6, 6, 20, 14));
        p.drawLine(QPointF(16, 20), QPointF(16, 29));
    } else if (name == "volumeLight") {
        pen_color(QColor(240, 215, 110), 1.6);
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QPointF(16, 16), 12, 12);
        p.drawEllipse(QRectF(4, 12, 24, 8));
    } else if (name == "camera") {
        p.setBrush(QColor(170, 170, 170));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(4, 10, 17, 13));
        p.drawPolygon(QPolygonF({{21, 14}, {28, 10}, {28, 23}, {21, 19}}));
    } else if (name == "set" || name == "objectSet") {
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(4, 8, 14, 14));
        p.drawEllipse(QRectF(14, 8, 14, 14));
    } else if (name == "layer") {
        p.setBrush(kBlue.darker(120));
        p.setPen(Qt::NoPen);
        p.drawPolygon(QPolygonF({{16, 6}, {28, 12}, {16, 18}, {4, 12}}));
        p.setBrush(kBlue.darker(170));
        p.drawPolygon(QPolygonF({{4, 17}, {16, 23}, {28, 17}, {28, 20}, {16, 26}, {4, 20}}));
    } else if (name == "extrude") {
        pen_color(kInk, 1.0);
        cube(p, QRectF(2, 8, 24, 24), kFill);
        pen_color(kOrange, 1.8);
        arrow(p, {18, 12}, {18, 2}, 5);
    } else if (name == "bevel") {
        p.setBrush(kFill.darker(150));
        p.setPen(QPen(kInk, 1.0));
        p.drawPolygon(QPolygonF({{4, 28}, {4, 12}, {12, 4}, {28, 4}, {28, 28}}));
        pen_color(kOrange, 2.0);
        p.drawLine(QPointF(4, 12), QPointF(12, 4));
    } else if (name == "bridge") {
        p.setBrush(kFill.darker(150));
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(2, 10, 8, 14));
        p.drawRect(QRectF(22, 10, 8, 14));
        p.setBrush(kOrange.darker(130));
        p.drawRect(QRectF(10, 13, 12, 8));
    } else if (name == "insertLoop" || name == "offsetLoop") {
        p.setBrush(kFill.darker(160));
        p.setPen(QPen(kInk, 1.0));
        p.drawRect(QRectF(4, 6, 24, 20));
        pen_color(kOrange, 1.8);
        p.drawLine(QPointF(16, 6), QPointF(16, 26));
        if (name == "offsetLoop") {
            p.drawLine(QPointF(10, 6), QPointF(10, 26));
            p.drawLine(QPointF(22, 6), QPointF(22, 26));
        }
    } else if (name == "merge" || name == "collapse") {
        p.setBrush(kOrange);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 16), 4, 4);
        pen_color(kInk, 1.4);
        arrow(p, {4, 4}, {12, 12}, 4);
        arrow(p, {28, 28}, {20, 20}, 4);
    } else if (name == "combine" || name == "separate") {
        pen_color(kInk, 1.0);
        cube(p, QRectF(0, 6, 20, 20), kFill);
        cube(p, QRectF(12, 6, 20, 20), kOrange);
    } else if (name == "smooth") {
        QRadialGradient g(QPointF(12, 11), 16);
        g.setColorAt(0, kFill.lighter(130));
        g.setColorAt(1, kFill.darker(220));
        p.setBrush(g);
        p.setPen(QPen(kInk, 0.8));
        p.drawRoundedRect(QRectF(4, 4, 24, 24), 9, 9);
    } else if (name == "boolean") {
        p.setBrush(kFill.darker(140));
        p.setPen(Qt::NoPen);
        p.drawEllipse(QRectF(3, 6, 18, 18));
        p.setBrush(kOrange.darker(120));
        p.drawEllipse(QRectF(12, 9, 18, 18));
    } else if (name == "mirror") {
        pen_color(kInk, 1.0);
        p.setBrush(kFill.darker(140));
        p.drawPolygon(QPolygonF({{14, 6}, {4, 16}, {14, 26}}));
        p.setBrush(kOrange.darker(130));
        p.drawPolygon(QPolygonF({{18, 6}, {28, 16}, {18, 26}}));
        p.setPen(QPen(kInk, 1.0, Qt::DashLine));
        p.drawLine(QPointF(16, 3), QPointF(16, 29));
    } else if (name == "play" || name == "playBack") {
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        if (name == "play")
            p.drawPolygon(QPolygonF({{10, 7}, {25, 16}, {10, 25}}));
        else
            p.drawPolygon(QPolygonF({{22, 7}, {7, 16}, {22, 25}}));
    } else if (name == "joint") {
        p.setPen(QPen(QColor(90, 130, 230), 1.6));
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(4, 20, 8, 8));
        p.drawEllipse(QRectF(20, 4, 8, 8));
        p.drawPolygon(QPolygonF({{11, 21}, {21, 11}, {13, 23}}));
        p.drawLine(QPointF(11, 21), QPointF(21, 11));
    } else if (name == "ik") {
        p.setPen(QPen(QColor(90, 130, 230), 1.6));
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(3, 22, 6, 6));
        p.drawEllipse(QRectF(13, 6, 6, 6));
        p.drawLine(QPointF(7, 23), QPointF(15, 11));
        p.drawLine(QPointF(18, 11), QPointF(26, 24));
        p.setPen(QPen(QColor(220, 220, 255), 1.4, Qt::DashLine));
        p.drawLine(QPointF(6, 25), QPointF(26, 25));
        p.setPen(QPen(QColor(220, 220, 255), 1.6));
        p.drawLine(QPointF(22, 25), QPointF(30, 25));
        p.drawLine(QPointF(26, 21), QPointF(26, 29));
    } else if (name == "skin") {
        p.setPen(QPen(kInk, 1.0));
        p.setBrush(kFill.darker(120));
        p.drawRoundedRect(QRectF(4, 8, 24, 16), 6, 6);
        p.setPen(QPen(QColor(90, 130, 230), 1.8));
        p.drawLine(QPointF(8, 16), QPointF(24, 16));
        p.setBrush(QColor(90, 130, 230));
        p.drawEllipse(QPointF(8, 16), 2.5, 2.5);
        p.drawEllipse(QPointF(24, 16), 2.5, 2.5);
    } else if (name == "parentCon" || name == "pointCon" || name == "orientCon") {
        p.setPen(QPen(kInk, 1.2));
        p.setBrush(kFill.darker(140));
        p.drawRect(QRectF(4, 18, 10, 10));
        p.setBrush(kOrange.darker(110));
        p.drawEllipse(QRectF(19, 4, 10, 10));
        p.setPen(QPen(kGreen, 1.6, Qt::DashLine));
        p.drawLine(QPointF(10, 18), QPointF(22, 13));
        QFont f = p.font();
        f.setPixelSize(9);
        f.setBold(true);
        p.setFont(f);
        p.setPen(kInk);
        p.drawText(QRectF(16, 18, 16, 14), Qt::AlignCenter, name == "parentCon" ? "P" : name == "pointCon" ? "T" : "R");
    } else if (name == "graph") {
        p.setPen(QPen(QColor(110, 110, 110), 1));
        p.setBrush(QColor(55, 55, 55));
        p.drawRect(QRectF(3, 5, 26, 22));
        QPainterPath path;
        path.moveTo(5, 24);
        path.cubicTo(14, 24, 12, 8, 27, 8);
        p.setPen(QPen(QColor(232, 74, 74), 1.6));
        p.setBrush(Qt::NoBrush);
        p.drawPath(path);
    } else if (name == "cutKey") {
        p.setPen(QPen(QColor(220, 70, 70), 2.2));
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(4, 10, 11, 11));
        p.drawLine(QPointF(15, 15.5), QPointF(28, 15.5));
        p.setPen(QPen(kInk, 2));
        p.drawLine(QPointF(18, 6), QPointF(28, 26));
    } else if (name.startsWith("uv")) {
        p.setPen(QPen(QColor(110, 110, 110), 1));
        p.setBrush(QColor(78, 78, 78));
        p.drawRect(QRectF(3, 3, 26, 26));
        p.setPen(QPen(QColor(200, 200, 220), 1.2));
        p.setBrush(QColor(60, 105, 180, 160));
        if (name == "uvEditor") {
            p.drawRect(QRectF(7, 7, 8, 8));
            p.drawRect(QRectF(17, 7, 8, 8));
            p.drawRect(QRectF(7, 17, 8, 8));
        } else if (name == "uvCut" || name == "uvSew") {
            p.drawRect(QRectF(6, 8, 9, 16));
            p.drawRect(QRectF(name == "uvCut" ? 18 : 15, 8, 9, 16));
        } else if (name == "uvUnfold") {
            p.drawPolygon(QPolygonF({{6, 24}, {16, 6}, {26, 24}}));
        } else {
            p.drawRect(QRectF(6, 6, 20, 20));
        }
        QFont f = p.font();
        f.setPixelSize(8);
        p.setFont(f);
        p.setPen(QColor(240, 240, 240));
        QString t = name == "uvAuto" ? "A" : name == "uvPlanar" ? "P" : name == "uvLayout" ? "L" : "";
        if (!t.isEmpty()) p.drawText(QRectF(3, 3, 26, 26), Qt::AlignCenter, t);
    } else if (name == "autokey" || name == "key") {
        // A key: ring bow, shaft and teeth.
        QColor c = name == "autokey" ? QColor(200, 200, 200) : QColor(220, 70, 70);
        p.setPen(QPen(c, 2.2));
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(4, 10, 11, 11));
        p.drawLine(QPointF(15, 15.5), QPointF(28, 15.5));
        p.drawLine(QPointF(24, 15.5), QPointF(24, 21));
        p.drawLine(QPointF(28, 15.5), QPointF(28, 20));
    } else if (name == "stop") {
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(9, 9, 14, 14));
    } else if (name == "first" || name == "last" || name == "prevKey" || name == "nextKey" || name == "prev" || name == "next") {
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        bool back = name == "first" || name == "prevKey" || name == "prev";
        if (back) {
            p.translate(32, 0);
            p.scale(-1, 1);
        }
        p.drawPolygon(QPolygonF({{9, 8}, {20, 16}, {9, 24}}));
        if (name == "first" || name == "last") p.drawRect(QRectF(21, 8, 3, 16));
        if (name == "prevKey" || name == "nextKey") p.drawPolygon(QPolygonF({{17, 8}, {28, 16}, {17, 24}}));
    } else if (name == "script") {
        p.setBrush(QColor(60, 60, 60));
        p.setPen(QPen(kInk, 1.0));
        p.drawRect(QRectF(3, 5, 26, 22));
        p.setPen(QPen(kGreen, 1.6));
        p.drawLine(QPointF(7, 11), QPointF(11, 15));
        p.drawLine(QPointF(11, 15), QPointF(7, 19));
        p.drawLine(QPointF(13, 20), QPointF(20, 20));
    } else if (name == "grid") {
        pen_color(kInk, 1.0);
        for (int i = 0; i < 5; ++i) {
            p.drawLine(QPointF(4 + i * 6, 4), QPointF(4 + i * 6, 28));
            p.drawLine(QPointF(4, 4 + i * 6), QPointF(28, 4 + i * 6));
        }
    } else if (name == "wireframe") {
        pen_color(kInk, 1.0);
        cube(p, QRectF(2, 2, 28, 28), kFill, false);
    } else if (name == "shaded") {
        pen_color(Qt::transparent, 0);
        cube(p, QRectF(2, 2, 28, 28), kFill);
    } else if (name == "textured") {
        // A checkered tile.
        p.fillRect(QRectF(4, 4, 24, 24), QColor(200, 200, 200));
        for (int y = 0; y < 2; y++)
            for (int x = 0; x < 2; x++)
                if ((x + y) % 2 == 0) p.fillRect(QRectF(4 + x * 12, 4 + y * 12, 12, 12), QColor(70, 110, 170));
    } else if (name == "wireOnShaded") {
        pen_color(kInk, 1.0);
        cube(p, QRectF(2, 2, 28, 28), kFill);
    } else if (name == "frameAll" || name == "frameSel") {
        p.setBrush(Qt::NoBrush);
        pen_color(kInk, 1.4);
        p.drawRect(QRectF(5, 5, 22, 22));
        if (name == "frameSel") {
            p.setBrush(kOrange);
            p.setPen(Qt::NoPen);
            p.drawRect(QRectF(12, 12, 8, 8));
        } else {
            p.setBrush(kFill);
            p.setPen(Qt::NoPen);
            p.drawRect(QRectF(8, 8, 6, 6));
            p.drawRect(QRectF(17, 16, 7, 7));
        }
    } else if (name == "options") {
        p.setBrush(Qt::NoBrush);
        pen_color(kInk, 1.2);
        p.drawRect(QRectF(8, 8, 16, 16));
    } else if (name == "visible") {
        p.setBrush(Qt::NoBrush);
        p.drawEllipse(QRectF(4, 10, 24, 12));
        p.setBrush(kInk);
        p.drawEllipse(QPointF(16, 16), 3.5, 3.5);
    } else if (draw_more(name, p)) {
        // Drawn in icons_more.cpp.
    } else {
        // Fallback: the name's initials in a box.
        p.setBrush(QColor(80, 80, 80));
        p.setPen(QPen(QColor(120, 120, 120), 1.0));
        p.drawRoundedRect(QRectF(2, 2, 28, 28), 4, 4);
        QFont f = p.font();
        f.setPixelSize(11);
        f.setBold(true);
        p.setFont(f);
        p.setPen(kInk);
        QString t = name.left(1).toUpper();
        for (QChar c : name.mid(1))
            if (c.isUpper()) t += c;
        p.drawText(full, Qt::AlignCenter, t.left(3));
    }
    return pm;
}

} // namespace

QIcon icon(const QString& name, int size) {
    static QHash<QString, QIcon> cache;
    QString key = name + "@" + QString::number(size);
    auto it = cache.find(key);
    if (it != cache.end()) return *it;
    QIcon ic;
    for (int s : {size, size * 2}) ic.addPixmap(draw(name, s));
    cache.insert(key, ic);
    return ic;
}

} // namespace az
