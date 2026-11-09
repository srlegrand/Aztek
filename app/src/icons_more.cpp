// More icons drawn with QPainter, on the same 32 unit canvas as icons.cpp: the status line, panel bar and shelves.
#include <QPainter>
#include <QPainterPath>
#include <QPolygonF>
#include <QRadialGradient>
#include <utility>
#include <QString>

namespace az {

namespace {
const QColor kInk(215, 215, 215);
const QColor kBlue(95, 160, 220);
const QColor kOrange(240, 160, 50);
const QColor kGreen(110, 200, 120);
const QColor kRed(220, 90, 80);
const QColor kPurple(180, 120, 220);
const QColor kYellow(235, 215, 90);

void line_pen(QPainter& p, const QColor& c, double w = 1.8) {
    p.setPen(QPen(c, w, Qt::SolidLine, Qt::RoundCap, Qt::RoundJoin));
    p.setBrush(Qt::NoBrush);
}

void wave(QPainter& p, const QColor& c) {
    line_pen(p, c, 2.2);
    QPainterPath path(QPointF(4, 24));
    path.cubicTo(QPointF(10, 2), QPointF(20, 30), QPointF(28, 7));
    p.drawPath(path);
}

void brush(QPainter& p, const QColor& c) {
    // The sculpt brushes: a handle and a coloured tip over a surface line.
    line_pen(p, kInk, 2.4);
    p.drawLine(QPointF(25, 4), QPointF(14, 17));
    p.setBrush(c);
    p.setPen(Qt::NoPen);
    p.drawEllipse(QPointF(11, 20), 5, 5);
    line_pen(p, QColor(150, 150, 150), 1.4);
    QPainterPath s(QPointF(2, 27));
    s.cubicTo(QPointF(10, 24), QPointF(20, 30), QPointF(30, 26));
    p.drawPath(s);
}

} // namespace

bool draw_more(const QString& name, QPainter& p) {
    if (name == "paintSelect") {
        // A brush stroke of picked points.
        line_pen(p, kInk, 2.4);
        p.drawLine(QPointF(27, 4), QPointF(17, 15));
        p.setPen(Qt::NoPen);
        p.setBrush(kOrange);
        p.drawEllipse(QPointF(14, 18), 4.5, 4.5);
        p.setBrush(kBlue);
        for (QPointF c : {QPointF(5, 27), QPointF(10, 26), QPointF(15, 27), QPointF(20, 26)}) p.drawEllipse(c, 2, 2);
        return true;
    }
    if (name == "hypershade") {
        // Three shader balls.
        for (auto [c, col] : {std::pair{QPointF(10, 11), kRed}, std::pair{QPointF(22, 11), kGreen}, std::pair{QPointF(16, 22), kBlue}}) {
            QRadialGradient g(c - QPointF(2, 2), 7);
            g.setColorAt(0, col.lighter(160));
            g.setColorAt(1, col.darker(150));
            p.setPen(Qt::NoPen);
            p.setBrush(g);
            p.drawEllipse(c, 6, 6);
        }
        return true;
    }
    if (name == "humanIK") {
        // A figure, as the reference's Character Controls button.
        p.setPen(Qt::NoPen);
        p.setBrush(kInk);
        p.drawEllipse(QPointF(16, 6), 3.5, 3.5);
        line_pen(p, kInk, 3.2);
        p.drawLine(QPointF(16, 11), QPointF(16, 20));
        p.drawLine(QPointF(7, 13), QPointF(25, 13));
        p.drawLine(QPointF(16, 20), QPointF(10, 29));
        p.drawLine(QPointF(16, 20), QPointF(22, 29));
        line_pen(p, kBlue, 1.6);
        p.drawEllipse(QPointF(16, 16), 13, 14);
        return true;
    }
    if (name == "defaultMaterial") {
        QRadialGradient g(QPointF(13, 13), 12);
        g.setColorAt(0, QColor(225, 225, 225));
        g.setColorAt(1, QColor(110, 110, 110));
        p.setPen(Qt::NoPen);
        p.setBrush(g);
        p.drawEllipse(QPointF(16, 16), 11, 11);
        return true;
    }
    if (name == "shadows") {
        p.setPen(Qt::NoPen);
        p.setBrush(QColor(40, 40, 40, 200));
        p.drawEllipse(QPointF(19, 26), 10, 3.5);
        QRadialGradient g(QPointF(12, 10), 10);
        g.setColorAt(0, QColor(230, 230, 230));
        g.setColorAt(1, QColor(120, 120, 120));
        p.setBrush(g);
        p.drawEllipse(QPointF(15, 15), 8, 8);
        line_pen(p, kYellow, 1.8);
        p.drawLine(QPointF(3, 3), QPointF(7, 7));
        return true;
    }
    if (name.startsWith("brush")) {
        // Each brush its own tint, from its name.
        uint h = qHash(name);
        brush(p, QColor::fromHsv(int(h % 360), 150, 220));
        return true;
    }
    if (name == "scopeHierarchy") {
        line_pen(p, kInk);
        p.drawRect(QRectF(12, 3, 8, 7));
        p.drawLine(QPointF(16, 10), QPointF(16, 15));
        p.drawLine(QPointF(7, 15), QPointF(25, 15));
        p.drawLine(QPointF(7, 15), QPointF(7, 20));
        p.drawLine(QPointF(25, 15), QPointF(25, 20));
        p.drawRect(QRectF(3, 20, 8, 7));
        p.drawRect(QRectF(21, 20, 8, 7));
    } else if (name == "scopeObject") {
        line_pen(p, kInk);
        p.setBrush(QColor(120, 140, 170));
        p.drawRect(QRectF(7, 10, 15, 15));
        p.drawPolygon(QPolygonF({{7, 10}, {12, 5}, {27, 5}, {22, 10}}));
        p.drawPolygon(QPolygonF({{22, 10}, {27, 5}, {27, 20}, {22, 25}}));
    } else if (name == "scopeComponent") {
        line_pen(p, kInk, 1.2);
        p.drawRect(QRectF(7, 10, 15, 15));
        p.drawPolyline(QPolygonF({{7, 10}, {12, 5}, {27, 5}, {27, 20}, {22, 25}}));
        p.drawLine(QPointF(22, 10), QPointF(27, 5));
        p.setBrush(kPurple);
        p.setPen(Qt::NoPen);
        for (QPointF c : {QPointF(7, 10), QPointF(22, 10), QPointF(7, 25), QPointF(22, 25), QPointF(27, 5), QPointF(12, 5), QPointF(27, 20)}) p.drawEllipse(c, 2.2, 2.2);
    } else if (name == "maskHandle") {
        line_pen(p, kYellow);
        p.drawLine(QPointF(16, 5), QPointF(16, 27));
        p.drawLine(QPointF(5, 16), QPointF(27, 16));
        p.drawEllipse(QPointF(16, 16), 5, 5);
    } else if (name == "maskJoint") {
        line_pen(p, kBlue);
        p.drawEllipse(QPointF(9, 22), 4.5, 4.5);
        p.drawEllipse(QPointF(24, 8), 3.5, 3.5);
        p.drawLine(QPointF(12, 18), QPointF(21, 10));
    } else if (name == "maskCurve") {
        wave(p, kBlue);
    } else if (name == "maskSurface") {
        line_pen(p, kInk, 1.3);
        p.setBrush(QColor(110, 130, 150));
        p.drawPolygon(QPolygonF({{4, 21}, {12, 8}, {28, 11}, {21, 26}}));
        p.drawLine(QPointF(8, 15), QPointF(24, 18));
        p.drawLine(QPointF(20, 9), QPointF(13, 24));
    } else if (name == "maskDeformer") {
        line_pen(p, kPurple, 1.6);
        p.drawRect(QRectF(5, 5, 22, 22));
        p.drawEllipse(QPointF(16, 16), 6, 3);
    } else if (name == "maskMisc") {
        line_pen(p, kInk);
        p.drawLine(QPointF(16, 6), QPointF(16, 26));
        p.drawLine(QPointF(6, 16), QPointF(26, 16));
        p.drawLine(QPointF(9, 9), QPointF(23, 23));
    } else if (name == "snapGrid") {
        line_pen(p, kInk, 1.2);
        for (int i = 0; i < 3; ++i) {
            p.drawLine(QPointF(5 + i * 11, 5), QPointF(5 + i * 11, 27));
            p.drawLine(QPointF(5, 5 + i * 11), QPointF(27, 5 + i * 11));
        }
        p.setBrush(kOrange);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 16), 3.2, 3.2);
    } else if (name == "snapCurve") {
        wave(p, kInk);
        p.setBrush(kOrange);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 16), 3.2, 3.2);
    } else if (name == "snapPoint") {
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        for (QPointF c : {QPointF(7, 8), QPointF(25, 8), QPointF(7, 25), QPointF(25, 25)}) p.drawEllipse(c, 2.2, 2.2);
        p.setBrush(kOrange);
        p.drawEllipse(QPointF(16, 16), 3.6, 3.6);
    } else if (name == "snapPlane") {
        line_pen(p, kInk, 1.4);
        p.drawPolygon(QPolygonF({{4, 22}, {11, 8}, {28, 8}, {21, 22}}));
        p.setBrush(kOrange);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 15), 3.2, 3.2);
    } else if (name == "snapLive") {
        line_pen(p, kGreen, 1.6);
        p.drawEllipse(QPointF(16, 16), 11, 11);
        p.drawEllipse(QPointF(16, 16), 11, 4);
        p.setBrush(kOrange);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 16), 3, 3);
    } else if (name == "lockSel") {
        line_pen(p, kInk, 2);
        p.drawArc(QRectF(10, 4, 12, 14), 0, 180 * 16);
        p.setBrush(kInk);
        p.setPen(Qt::NoPen);
        p.drawRoundedRect(QRectF(7, 13, 18, 14), 2, 2);
    } else if (name == "highlightSel") {
        p.setBrush(QColor(100, 255, 160));
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 16), 6, 6);
        line_pen(p, QColor(100, 255, 160), 1.6);
        for (int i = 0; i < 8; ++i) {
            p.save();
            p.translate(16, 16);
            p.rotate(i * 45.0);
            p.drawLine(QPointF(9, 0), QPointF(13, 0));
            p.restore();
        }
    } else if (name == "softSelect") {
        p.setPen(Qt::NoPen);
        QRadialGradient g(QPointF(16, 16), 13);
        g.setColorAt(0, QColor(255, 240, 80));
        g.setColorAt(0.5, QColor(240, 90, 30));
        g.setColorAt(1, QColor(60, 0, 0, 0));
        p.setBrush(g);
        p.drawEllipse(QPointF(16, 16), 13, 13);
    } else if (name == "toolSettings") {
        line_pen(p, kInk);
        p.drawEllipse(QPointF(16, 16), 5, 5);
        for (int i = 0; i < 6; ++i) {
            p.save();
            p.translate(16, 16);
            p.rotate(i * 60.0);
            p.drawLine(QPointF(7, 0), QPointF(11, 0));
            p.restore();
        }
    } else if (name == "attrEditor" || name == "channelBoxIcon") {
        line_pen(p, kInk, 1.4);
        p.drawRect(QRectF(5, 5, 22, 22));
        for (int i = 0; i < 3; ++i) p.drawLine(QPointF(9, 11 + i * 5), QPointF(name == "attrEditor" ? 16 : 14, 11 + i * 5));
        p.setBrush(name == "attrEditor" ? kBlue : kRed);
        p.setPen(Qt::NoPen);
        for (int i = 0; i < 3; ++i) p.drawRect(QRectF(18, 9.5 + i * 5, 6, 3));
    } else if (name == "outlinerIcon") {
        line_pen(p, kInk, 1.4);
        p.drawLine(QPointF(7, 6), QPointF(7, 24));
        p.drawLine(QPointF(7, 11), QPointF(12, 11));
        p.drawLine(QPointF(7, 18), QPointF(12, 18));
        p.drawLine(QPointF(7, 24), QPointF(12, 24));
        p.setBrush(kBlue);
        p.setPen(Qt::NoPen);
        for (int y : {11, 18, 24}) p.drawRect(QRectF(14, y - 2, 13, 4));
        p.drawRect(QRectF(5, 3, 10, 4));
    } else if (name == "selectCamera" || name == "bookmark") {
        line_pen(p, kInk, 1.6);
        p.drawRoundedRect(QRectF(4, 10, 17, 13), 2, 2);
        p.drawPolygon(QPolygonF({{21, 15}, {28, 11}, {28, 22}, {21, 18}}));
        if (name == "bookmark") {
            p.setBrush(kYellow);
            p.setPen(Qt::NoPen);
            p.drawEllipse(QPointF(12, 16), 3, 3);
        }
    } else if (name == "filmGate" || name == "resGate") {
        line_pen(p, name == "filmGate" ? kInk : kOrange, 1.6);
        p.drawRect(QRectF(4, 8, 24, 16));
        line_pen(p, QColor(150, 150, 150), 1);
        p.drawRect(QRectF(8, 11, 16, 10));
    } else if (name == "xray" || name == "xrayJoints" || name == "xrayActive") {
        p.setBrush(QColor(150, 180, 220, 90));
        p.setPen(QPen(kInk, 1.2));
        p.drawEllipse(QPointF(16, 16), 11, 11);
        if (name == "xrayJoints") {
            line_pen(p, kBlue, 2);
            p.drawLine(QPointF(10, 22), QPointF(22, 10));
        } else if (name == "xrayActive") {
            p.setBrush(kPurple);
            p.setPen(Qt::NoPen);
            for (QPointF c : {QPointF(11, 12), QPointF(21, 12), QPointF(16, 21)}) p.drawEllipse(c, 2.4, 2.4);
        }
    } else if (name == "backface") {
        p.setBrush(QColor(130, 150, 170));
        p.setPen(QPen(kInk, 1.2));
        p.drawPolygon(QPolygonF({{5, 25}, {16, 5}, {27, 25}}));
        line_pen(p, kRed, 1.8);
        p.drawLine(QPointF(5, 27), QPointF(27, 5));
    } else if (name == "isolate") {
        line_pen(p, kInk, 1.4);
        p.drawRect(QRectF(4, 4, 24, 24));
        p.setBrush(kYellow);
        p.setPen(Qt::NoPen);
        p.drawEllipse(QPointF(16, 16), 5.5, 5.5);
    } else if (name == "multiCut") {
        line_pen(p, kInk, 1.4);
        p.drawRect(QRectF(6, 8, 20, 18));
        line_pen(p, kOrange, 1.8);
        p.drawLine(QPointF(4, 22), QPointF(28, 12));
    } else if (name == "quadDraw") {
        line_pen(p, kGreen, 1.8);
        p.drawPolygon(QPolygonF({{7, 9}, {25, 7}, {27, 25}, {5, 23}}));
        p.setBrush(kOrange);
        p.setPen(Qt::NoPen);
        for (QPointF c : {QPointF(7, 9), QPointF(25, 7), QPointF(27, 25), QPointF(5, 23)}) p.drawEllipse(c, 2.2, 2.2);
    } else if (name == "curveCv" || name == "curveEp") {
        wave(p, kBlue);
        p.setBrush(name == "curveCv" ? kPurple : kOrange);
        p.setPen(Qt::NoPen);
        for (QPointF c : {QPointF(4, 24), QPointF(11, 8), QPointF(21, 22), QPointF(28, 7)}) p.drawEllipse(c, 2.2, 2.2);
    } else if (name == "circle") {
        line_pen(p, kBlue, 2.2);
        p.drawEllipse(QPointF(16, 16), 11, 11);
    } else if (name == "square") {
        line_pen(p, kBlue, 2.2);
        p.drawRect(QRectF(6, 6, 20, 20));
    } else if (name == "attachCurve" || name == "detachCurve") {
        line_pen(p, kBlue, 2.2);
        p.drawLine(QPointF(4, 22), QPointF(13, 14));
        p.drawLine(QPointF(19, 14), QPointF(28, 8));
        line_pen(p, name == "attachCurve" ? kGreen : kRed, 2);
        p.drawLine(QPointF(13, 14), QPointF(19, 14));
    } else if (name == "rebuildCurve" || name == "reverseCurve" || name == "offsetCurve") {
        wave(p, kBlue);
        line_pen(p, kOrange, 1.8);
        if (name == "reverseCurve") {
            p.drawLine(QPointF(22, 26), QPointF(9, 26));
            p.drawLine(QPointF(9, 26), QPointF(13, 23));
        } else if (name == "offsetCurve") {
            QPainterPath path(QPointF(4, 30));
            path.cubicTo(QPointF(10, 8), QPointF(20, 36), QPointF(28, 13));
            p.drawPath(path);
        } else {
            p.drawArc(QRectF(20, 18, 9, 9), 30 * 16, 300 * 16);
        }
    } else if (name == "loft" || name == "revolve" || name == "planarSurface" || name == "extrudeCurve") {
        line_pen(p, kInk, 1.3);
        p.setBrush(QColor(110, 140, 170));
        if (name == "revolve") {
            p.drawEllipse(QRectF(5, 5, 22, 8));
            p.drawEllipse(QRectF(9, 20, 14, 7));
            p.drawLine(QPointF(5, 9), QPointF(9, 23));
            p.drawLine(QPointF(27, 9), QPointF(23, 23));
        } else if (name == "planarSurface") {
            p.drawEllipse(QRectF(5, 10, 22, 13));
        } else if (name == "loft") {
            p.drawPolygon(QPolygonF({{5, 24}, {7, 8}, {25, 6}, {27, 22}}));
            line_pen(p, kBlue, 2);
            p.drawLine(QPointF(5, 24), QPointF(7, 8));
            p.drawLine(QPointF(25, 6), QPointF(27, 22));
        } else {
            p.drawRect(QRectF(5, 12, 22, 14));
            line_pen(p, kBlue, 2);
            p.drawLine(QPointF(5, 12), QPointF(27, 12));
            p.drawLine(QPointF(16, 12), QPointF(16, 4));
        }
    } else if (name == "paintSkin") {
        brush(p, QColor(240, 240, 240));
    } else if (name == "scaleCon" || name == "aimCon") {
        line_pen(p, kInk, 1.6);
        p.drawEllipse(QPointF(10, 22), 5, 5);
        p.drawEllipse(QPointF(23, 9), 5, 5);
        line_pen(p, name == "scaleCon" ? kOrange : kGreen, 2);
        p.drawLine(QPointF(13, 19), QPointF(20, 12));
    } else if (name == "cluster") {
        p.setPen(Qt::NoPen);
        p.setBrush(kGreen);
        for (QPointF c : {QPointF(10, 11), QPointF(21, 9), QPointF(16, 18), QPointF(9, 23), QPointF(24, 22)}) p.drawEllipse(c, 2.8, 2.8);
    } else if (name == "blendShape") {
        line_pen(p, kInk, 1.4);
        p.setBrush(QColor(130, 150, 170));
        p.drawEllipse(QPointF(11, 16), 7, 7);
        p.setBrush(QColor(220, 150, 110, 180));
        p.drawRoundedRect(QRectF(15, 9, 13, 14), 2, 2);
    } else if (name == "dopeSheet" || name == "drivenKey") {
        line_pen(p, kInk, 1.2);
        for (int i = 0; i < 3; ++i) p.drawLine(QPointF(4, 8 + i * 8), QPointF(28, 8 + i * 8));
        p.setBrush(name == "dopeSheet" ? kRed : kBlue);
        p.setPen(Qt::NoPen);
        p.drawRect(QRectF(8, 6, 4, 4));
        p.drawRect(QRectF(19, 14, 4, 4));
        p.drawRect(QRectF(13, 22, 4, 4));
    } else if (name == "keyMove" || name == "keyRotate" || name == "keyScale") {
        p.setBrush(kRed);
        p.setPen(Qt::NoPen);
        p.drawPolygon(QPolygonF({{9, 6}, {16, 13}, {9, 20}, {2, 13}}));
        line_pen(p, kInk, 1.8);
        if (name == "keyMove") {
            p.drawLine(QPointF(20, 26), QPointF(20, 8));
            p.drawLine(QPointF(20, 8), QPointF(17, 12));
            p.drawLine(QPointF(20, 8), QPointF(23, 12));
        } else if (name == "keyRotate") {
            p.drawEllipse(QPointF(21, 19), 7, 7);
        } else {
            p.drawRect(QRectF(16, 14, 11, 11));
        }
    } else {
        return false;
    }
    return true;
}

} // namespace az
