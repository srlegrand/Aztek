// The sculpting brushes' icons: a slice through clay, on a 32 by 32 grid. The shape before is a dashed ghost, the shape after
// is filled, and arrows or a tool show what the brush does to get there.

#include <QPainter>
#include <QPainterPath>
#include <QString>
#include <QtMath>

#include <functional>

namespace az {

namespace {

const QColor kClay(196, 170, 140);
const QColor kClayDark(120, 98, 76);
const QColor kInk(235, 235, 235);
const QColor kGhost(235, 235, 235, 150);
const QColor kAccent(255, 170, 60);
const QColor kCold(120, 190, 255);

using Height = std::function<double(double)>;

QPainterPath line_of(const Height& h) {
    QPainterPath path;
    for (double x = 2; x <= 30.01; x += 0.5) {
        if (x == 2) path.moveTo(x, h(x));
        else path.lineTo(x, h(x));
    }
    return path;
}

void clay(QPainter& p, const Height& h, const QColor& top = kClay) {
    QPainterPath body = line_of(h);
    body.lineTo(30, 30);
    body.lineTo(2, 30);
    body.closeSubpath();
    QLinearGradient g(0, 6, 0, 30);
    g.setColorAt(0, top);
    g.setColorAt(1, top.darker(190));
    p.setBrush(g);
    p.setPen(QPen(top.lighter(125), 1.0));
    p.drawPath(body);
}

void ghost(QPainter& p, const Height& h) {
    p.setBrush(Qt::NoBrush);
    p.setPen(QPen(kGhost, 1.0, Qt::DashLine));
    p.drawPath(line_of(h));
}

void arrow(QPainter& p, QPointF a, QPointF b, const QColor& c = kAccent) {
    p.setPen(QPen(c, 1.8, Qt::SolidLine, Qt::RoundCap));
    p.drawLine(a, b);
    QPointF d = b - a;
    double len = qSqrt(d.x() * d.x() + d.y() * d.y());
    if (len < 0.01) return;
    d /= len;
    QPointF n(-d.y(), d.x());
    QPolygonF head{b + d * 1.5, b - d * 3.2 + n * 2.6, b - d * 3.2 - n * 2.6};
    p.setBrush(c);
    p.setPen(Qt::NoPen);
    p.drawPolygon(head);
}

double bump(double x, double at, double width, double height) {
    double t = (x - at) / width;
    return height * qExp(-t * t);
}

} // namespace

bool draw_sculpt_icon(QPainter& p, const QString& tool) {
    const double base = 23;
    Height flat = [=](double) { return base; };
    if (tool == "sculpt") {
        ghost(p, flat);
        clay(p, [=](double x) { return base - bump(x, 16, 6.5, 11); });
        arrow(p, {16, 18}, {16, 5});
    } else if (tool == "smooth") {
        Height rough = [=](double x) { return base - 3 - 3.5 * qSin(x * 1.5) - 1.5 * qSin(x * 3.1); };
        clay(p, [=](double x) { return base - 3 - 0.8 * qSin(x * 0.5); });
        ghost(p, rough);
    } else if (tool == "relax") {
        // Bunched mesh lines spread out evenly; the surface itself stays where it is.
        clay(p, flat);
        p.setPen(QPen(kGhost, 1.0, Qt::DashLine));
        for (double x : {11.0, 13.0, 15.0, 17.0, 19.0, 21.0}) p.drawLine(QPointF(x, 12), QPointF(x, 20));
        p.setPen(QPen(kClayDark.darker(130), 1.3));
        for (double x : {5.0, 11.0, 16.0, 21.0, 27.0}) p.drawLine(QPointF(x, 23), QPointF(x, 30));
        arrow(p, {13, 8}, {5, 8});
        arrow(p, {19, 8}, {27, 8});
    } else if (tool == "grab") {
        ghost(p, flat);
        clay(p, [=](double x) { return base - bump(x, 21 - (x < 21 ? 0 : 2), x < 21 ? 9 : 3.5, 13); });
        arrow(p, {10, 15}, {25, 5});
    } else if (tool == "pinch") {
        Height wide = [=](double x) { return base - 6 + bump(x, 16, 6, 7); };
        clay(p, [=](double x) { return base - 6 + bump(x, 16, 1.8, 9); });
        ghost(p, wide);
        arrow(p, {3, 9}, {11, 9});
        arrow(p, {29, 9}, {21, 9});
    } else if (tool == "flatten") {
        Height lumpy = [=](double x) { return base - 5 - bump(x, 10, 4, 7) - bump(x, 22, 4, 5) + bump(x, 16, 2, 2); };
        clay(p, [=](double) { return base - 7; });
        ghost(p, lumpy);
        p.setPen(QPen(kAccent, 2.0, Qt::SolidLine, Qt::RoundCap));
        p.drawLine(QPointF(3, 15.5), QPointF(29, 15.5));
    } else if (tool == "foamy") {
        // Soft and full: rounded puffs, with no sharp edge anywhere.
        clay(p, [=](double x) { return base - bump(x, 9, 5.5, 8) - bump(x, 18, 5, 11) - bump(x, 26, 4.5, 6); }, kClay.lighter(115));
        arrow(p, {17, 11}, {17, 3}, kInk);
    } else if (tool == "spray") {
        clay(p, [=](double x) { return base - bump(x, 7, 1.4, 3) - bump(x, 13, 1.4, 4) - bump(x, 18, 1.4, 3) - bump(x, 24, 1.4, 4); });
        p.setBrush(kAccent);
        p.setPen(Qt::NoPen);
        for (QPointF d : {QPointF(9, 13), QPointF(14, 8), QPointF(18, 13), QPointF(23, 9), QPointF(6, 7), QPointF(27, 13), QPointF(16, 3), QPointF(22, 4)}) p.drawEllipse(d, 1.1, 1.1);
    } else if (tool == "repeat") {
        clay(p, [=](double x) { return base - bump(x, 7, 2.3, 8) - bump(x, 16, 2.3, 8) - bump(x, 25, 2.3, 8); });
        arrow(p, {5, 8}, {27, 8});
    } else if (tool == "imprint") {
        // A stamp's shape pressed into the surface.
        clay(p, [=](double x) { return base - 4 + ((x > 9 && x < 23) ? 4.5 : 0); });
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(10, 8, 12, 5));
        p.drawRect(QRectF(13.5, 3, 5, 5));
        arrow(p, {6, 5}, {6, 15});
    } else if (tool == "wax") {
        // Laid on in layers.
        clay(p, flat);
        p.setPen(QPen(kClay.lighter(130), 0.8));
        p.setBrush(kClay.lighter(110));
        p.drawRoundedRect(QRectF(6, 18.5, 20, 4.5), 2.2, 2.2);
        p.setBrush(kClay.lighter(125));
        p.drawRoundedRect(QRectF(9, 14, 14, 4.5), 2.2, 2.2);
        arrow(p, {16, 3}, {16, 12});
    } else if (tool == "scrape") {
        Height lumpy = [=](double x) { return base - 6 - bump(x, 11, 4, 8) - bump(x, 22, 4, 6); };
        clay(p, [=](double x) { return qMax(base - 7.0, lumpy(x)); });
        ghost(p, lumpy);
        // The blade, shearing the tops off.
        QPolygonF blade{QPointF(25, 16), QPointF(29, 16), QPointF(31, 5), QPointF(27.5, 5)};
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        p.drawPolygon(blade);
        arrow(p, {23, 13}, {6, 13});
    } else if (tool == "fill") {
        // Hollows filled up to level, in a lighter colour.
        Height hollow = [=](double x) { return base - 7 + bump(x, 16, 6, 9); };
        clay(p, [=](double) { return base - 7; }, kAccent.lighter(120));
        clay(p, hollow);
        arrow(p, {16, 3}, {16, 13});
    } else if (tool == "knife") {
        clay(p, [=](double x) { return base - 8 + bump(x, 16, 1.1, 14); });
        QPolygonF blade{QPointF(16, 22), QPointF(13.5, 7), QPointF(13.5, 2), QPointF(18.5, 2), QPointF(18.5, 7)};
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        p.drawPolygon(blade);
    } else if (tool == "smear") {
        ghost(p, [=](double x) { return base - bump(x, 10, 4, 11); });
        clay(p, [=](double x) { return base - bump(x, x < 21 ? 21 : 21, x < 21 ? 12 : 3, 11); });
        arrow(p, {7, 7}, {26, 7});
    } else if (tool == "bulge") {
        // Swells outward along its own surface.
        clay(p, flat);
        p.setBrush(kClay);
        p.setPen(QPen(kClay.lighter(125), 1.0));
        p.drawEllipse(QRectF(9.5, 11, 13, 14));
        arrow(p, {16, 11}, {16, 3});
        arrow(p, {11, 13}, {5.5, 7.5});
        arrow(p, {21, 13}, {26.5, 7.5});
    } else if (tool == "amplify") {
        // What's there gets bigger: highs higher, lows lower.
        Height shape = [=](double x) { return base - 6 - 3 * qSin(x * 0.85); };
        clay(p, [=](double x) { return base - 6 - 8 * qSin(x * 0.85); });
        ghost(p, shape);
        arrow(p, {9, 15}, {9, 7});
        arrow(p, {16, 16}, {16, 24}, kInk);
    } else if (tool == "freeze") {
        // Held still: cold clay and a snowflake.
        clay(p, [=](double x) { return base - bump(x, 16, 9, 2); }, kCold);
        p.setPen(QPen(kCold.lighter(135), 1.5, Qt::SolidLine, Qt::RoundCap));
        const QPointF c(16, 10);
        for (int i = 0; i < 3; ++i) {
            const double a = i * M_PI / 3.0;
            const QPointF d(qCos(a) * 7.5, qSin(a) * 7.5);
            p.drawLine(c - d, c + d);
            for (int side : {-1, 1}) {
                const QPointF tip = c + d * side;
                const QPointF back = c + d * side * 0.62;
                const QPointF n(-d.y() * 0.28, d.x() * 0.28);
                p.drawLine(back + n, tip);
                p.drawLine(back - n, tip);
            }
        }
    } else {
        return false;
    }
    return true;
}

} // namespace az
