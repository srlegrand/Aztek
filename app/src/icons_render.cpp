// Icons for rendering, on the same 32 unit canvas as icons.cpp: the Rendering shelf, the Render View's toolbar.
#include <QPainter>
#include <QPainterPath>
#include <QPolygonF>
#include <QRadialGradient>
#include <QString>
#include <cmath>

namespace az {

namespace {
const QColor kInk(215, 215, 215);
const QColor kBlue(95, 160, 220);
const QColor kOrange(240, 160, 50);
const QColor kGreen(110, 200, 120);
const QColor kRed(220, 90, 80);
const QColor kYellow(235, 215, 90);

void line_pen(QPainter& p, const QColor& c, double w = 1.8) {
    p.setPen(QPen(c, w, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    p.setBrush(Qt::NoBrush);
}

// A lit ball: the standard surface, and the image in the render icons.
void ball(QPainter& p, QPointF c, double r, const QColor& base) {
    QRadialGradient g(c + QPointF(-r * 0.35, -r * 0.4), r * 1.3);
    g.setColorAt(0.0, base.lighter(170));
    g.setColorAt(0.5, base);
    g.setColorAt(1.0, base.darker(260));
    p.setPen(Qt::NoPen);
    p.setBrush(g);
    p.drawEllipse(c, r, r);
    p.setBrush(QColor(255, 255, 255, 220));
    p.drawEllipse(c + QPointF(-r * 0.38, -r * 0.42), r * 0.18, r * 0.13);
}

// A picture: a frame with a hill and a sun, which the render icons share.
void picture(QPainter& p, QRectF r) {
    line_pen(p, kInk, 1.6);
    p.setBrush(QColor(45, 55, 70));
    p.drawRoundedRect(r, 2, 2);
    p.setPen(Qt::NoPen);
    p.setBrush(kYellow);
    p.drawEllipse(QPointF(r.right() - r.width() * 0.27, r.top() + r.height() * 0.3), r.width() * 0.1, r.width() * 0.1);
    QPainterPath hill(QPointF(r.left() + 1, r.bottom() - 1));
    hill.lineTo(r.left() + r.width() * 0.35, r.top() + r.height() * 0.45);
    hill.lineTo(r.left() + r.width() * 0.6, r.top() + r.height() * 0.75);
    hill.lineTo(r.left() + r.width() * 0.75, r.top() + r.height() * 0.6);
    hill.lineTo(r.right() - 1, r.bottom() - 1);
    hill.closeSubpath();
    p.setBrush(kGreen.darker(120));
    p.drawPath(hill);
}

// A sweep of rays: a light's spread.
void rays(QPainter& p, QPointF from, double angle0, double angle1, int n, double len, const QColor& c) {
    line_pen(p, c, 1.4);
    for (int k = 0; k < n; ++k) {
        double a = angle0 + (angle1 - angle0) * k / qMax(1, n - 1);
        p.drawLine(from, from + QPointF(std::cos(a), std::sin(a)) * len);
    }
}

void circular_arrows(QPainter& p, QPointF c, double r, const QColor& col) {
    line_pen(p, col, 1.8);
    QRectF box(c.x() - r, c.y() - r, 2 * r, 2 * r);
    p.drawArc(box, 30 * 16, 120 * 16);
    p.drawArc(box, 210 * 16, 120 * 16);
    p.setBrush(col);
    p.setPen(Qt::NoPen);
    auto head = [&](double deg, double dir) {
        double a = deg * M_PI / 180.0;
        QPointF at = c + QPointF(std::cos(a), -std::sin(a)) * r;
        QPointF t(-std::sin(a) * dir, -std::cos(a) * dir);
        QPointF n(std::cos(a), -std::sin(a));
        p.drawPolygon(QPolygonF({at + t * 3.5, at - n * 2.6, at + n * 2.6}));
    };
    head(150, 1);
    head(330, 1);
}

} // namespace

bool draw_render_icon(const QString& name, QPainter& p) {
    if (name == "renderFrame") {
        // Render: a picture with a spark at its corner.
        picture(p, QRectF(3, 7, 22, 18));
        p.setPen(Qt::NoPen);
        p.setBrush(kOrange);
        QPolygonF star;
        for (int k = 0; k < 10; ++k) {
            double a = -M_PI / 2 + k * M_PI / 5;
            double rr = k % 2 ? 2.6 : 6.5;
            star << QPointF(24.5 + std::cos(a) * rr, 7.5 + std::sin(a) * rr);
        }
        p.drawPolygon(star);
        return true;
    }
    if (name == "renderIpr") {
        // Interactive: the picture with arrows going round.
        picture(p, QRectF(3, 7, 20, 16));
        p.setBrush(QColor(40, 40, 40));
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(23, 22), 8, 8);
        circular_arrows(p, QPointF(23, 22), 5.5, kOrange);
        return true;
    }
    if (name == "renderView") {
        // The Render View: a picture in a window.
        line_pen(p, kInk, 1.4);
        p.setBrush(QColor(70, 70, 70));
        p.drawRoundedRect(QRectF(2, 4, 28, 24), 2, 2);
        p.setPen(Qt::NoPen);
        p.setBrush(kBlue);
        p.drawRect(QRectF(3, 5, 26, 4));
        picture(p, QRectF(6, 12, 20, 13));
        return true;
    }
    if (name == "renderSettings") {
        // Settings: three sliders before a picture's corner.
        picture(p, QRectF(13, 3, 16, 13));
        line_pen(p, kInk, 1.6);
        for (int k = 0; k < 3; ++k) {
            double y = 15 + k * 6;
            p.drawLine(QPointF(4, y), QPointF(26, y));
        }
        p.setPen(QPen(QColor(40, 40, 40), 1.0));
        for (auto [x, k] : {std::pair{9.0, 0}, {20.0, 1}, {13.0, 2}}) {
            p.setBrush(k == 1 ? kOrange : kInk);
            p.drawRoundedRect(QRectF(x - 2.5, 15 + k * 6 - 3, 5, 6), 1, 1);
        }
        return true;
    }
    if (name == "skyDome") {
        // A dome of light over the ground, the sun in it.
        QRadialGradient g(QPointF(16, 26), 15);
        g.setColorAt(0.0, QColor(150, 200, 245));
        g.setColorAt(1.0, QColor(60, 110, 180));
        p.setPen(QPen(kInk, 1.4));
        p.setBrush(g);
        QPainterPath dome(QPointF(2, 26));
        dome.arcTo(QRectF(2, 11, 28, 30), 180, -180);
        dome.closeSubpath();
        p.drawPath(dome);
        p.setPen(Qt::NoPen);
        p.setBrush(kYellow);
        p.drawEllipse(QPointF(21, 18), 3, 3);
        line_pen(p, kInk, 1.6);
        p.drawLine(QPointF(1, 26.5), QPointF(31, 26.5));
        return true;
    }
    if (name == "haroldAreaLight") {
        // A lit quad and its spread.
        rays(p, QPointF(16, 13), M_PI * 0.3, M_PI * 0.7, 5, 16, kYellow);
        p.setPen(QPen(kInk, 1.4));
        p.setBrush(QColor(250, 240, 190));
        p.drawPolygon(QPolygonF({{6, 6}, {24, 4}, {27, 13}, {8, 15}}));
        return true;
    }
    if (name == "meshLight") {
        // A glowing shape.
        rays(p, QPointF(16, 17), 0, 2 * M_PI * 7 / 8, 8, 14, kYellow);
        p.setPen(QPen(kInk, 1.2));
        p.setBrush(QColor(250, 235, 170));
        p.drawPolygon(QPolygonF({{16, 8}, {24, 13}, {22, 23}, {11, 24}, {8, 14}}));
        return true;
    }
    if (name == "photometricLight") {
        // A lamp with its uneven spread.
        line_pen(p, kYellow, 1.4);
        QPainterPath lobe(QPointF(16, 12));
        lobe.cubicTo(QPointF(2, 22), QPointF(6, 31), QPointF(16, 26));
        lobe.cubicTo(QPointF(26, 31), QPointF(30, 22), QPointF(16, 12));
        p.drawPath(lobe);
        p.setPen(QPen(kInk, 1.4));
        p.setBrush(QColor(250, 240, 190));
        p.drawEllipse(QPointF(16, 9), 5, 5);
        return true;
    }
    if (name == "atmosphere") {
        // Fog: soft bands across a beam.
        p.setPen(Qt::NoPen);
        for (int k = 0; k < 4; ++k) {
            p.setBrush(QColor(200, 210, 225, 60 + k * 30));
            p.drawRoundedRect(QRectF(3 + k * 2, 6 + k * 6, 26 - k * 3, 4), 2, 2);
        }
        rays(p, QPointF(6, 3), M_PI * 0.3, M_PI * 0.45, 3, 28, kYellow);
        return true;
    }
    if (name == "standardSurface") {
        ball(p, QPointF(16, 16), 12, QColor(200, 120, 70));
        return true;
    }
    if (name == "snapshot") {
        // Keep: a picture with a pin.
        picture(p, QRectF(3, 8, 22, 18));
        p.setPen(QPen(QColor(40, 40, 40), 1.0));
        p.setBrush(kRed);
        p.drawEllipse(QPointF(24, 8), 4.5, 4.5);
        line_pen(p, kInk, 1.6);
        p.drawLine(QPointF(24, 12.5), QPointF(24, 16));
        return true;
    }
    if (name == "renderStop") {
        p.setPen(QPen(kInk, 1.2));
        p.setBrush(kRed);
        QPolygonF oct;
        for (int k = 0; k < 8; ++k) {
            double a = M_PI / 8 + k * M_PI / 4;
            oct << QPointF(16 + std::cos(a) * 12, 16 + std::sin(a) * 12);
        }
        p.drawPolygon(oct);
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(10, 14.5, 12, 3));
        return true;
    }
    if (name == "renderRefresh") {
        circular_arrows(p, QPointF(16, 16), 10, kGreen);
        return true;
    }
    if (name == "renderRegion") {
        // A dashed box on the picture.
        picture(p, QRectF(3, 5, 26, 22));
        p.setPen(QPen(kRed, 1.6, Qt::DashLine));
        p.setBrush(Qt::NoBrush);
        p.drawRect(QRectF(10, 10, 12, 10));
        return true;
    }
    if (name == "renderWipe") {
        // A and B either side of a line.
        p.setPen(Qt::NoPen);
        p.setBrush(QColor(200, 120, 70));
        p.drawRect(QRectF(3, 6, 13, 20));
        p.setBrush(QColor(90, 140, 200));
        p.drawRect(QRectF(16, 6, 13, 20));
        line_pen(p, kInk, 2.0);
        p.drawLine(QPointF(16, 3), QPointF(16, 29));
        return true;
    }
    if (name == "zoomFit") {
        line_pen(p, kInk, 1.6);
        for (auto [x, y, dx, dy] : {std::tuple{4, 4, 1, 1}, {28, 4, -1, 1}, {4, 28, 1, -1}, {28, 28, -1, -1}}) {
            p.drawLine(QPointF(x, y), QPointF(x + dx * 7, y));
            p.drawLine(QPointF(x, y), QPointF(x, y + dy * 7));
        }
        p.drawRect(QRectF(10, 10, 12, 12));
        return true;
    }
    if (name == "zoomOne") {
        line_pen(p, kInk, 1.4);
        p.drawRect(QRectF(4, 4, 24, 24));
        QFont f = p.font();
        f.setPixelSize(12);
        f.setBold(true);
        p.setFont(f);
        p.drawText(QRectF(4, 4, 24, 24), Qt::AlignCenter, "1:1");
        return true;
    }
    if (name == "saveImage") {
        picture(p, QRectF(3, 3, 20, 16));
        p.setPen(Qt::NoPen);
        p.setBrush(kBlue);
        p.drawPolygon(QPolygonF({{20, 18}, {30, 18}, {30, 22}, {25, 29}, {20, 22}}));
        return true;
    }
    return false;
}

} // namespace az
