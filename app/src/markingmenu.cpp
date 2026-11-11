#include "markingmenu.h"

#include <QDateTime>
#include <QKeyEvent>
#include <QMouseEvent>
#include <QPainter>
#include <QScreen>
#include <cmath>

namespace az {

namespace {
const int kRadius = 92;
const int kH = 22;
} // namespace

void MarkingMenu::show_at(QPoint global, const QList<MarkItem>& items) {
    auto* m = new MarkingMenu(global, items);
    m->setAttribute(Qt::WA_DeleteOnClose);
    m->show();
    m->grabMouse();
    m->setFocus();
}

MarkingMenu::MarkingMenu(QPoint global, const QList<MarkItem>& items) : QWidget(nullptr, Qt::Popup | Qt::FramelessWindowHint), items_(items) {
    setAttribute(Qt::WA_TranslucentBackground);
    setMouseTracking(true);
    QFontMetrics fm(font());
    // Lay out round a centre, then size the window to fit.
    QList<QRect> rs;
    // A menu with nothing round the compass (a submenu's list) starts just under the cursor.
    bool compass = false;
    int list_w = 140;
    for (const MarkItem& it : items_) {
        compass |= it.dir >= 0;
        if (it.dir < 0) list_w = qMax(list_w, fm.horizontalAdvance(it.label) + 24 + (it.checkable ? 18 : 0) + (it.sub.isEmpty() ? 0 : 14));
    }
    int list_y = compass ? kRadius + kH : 14;
    QList<QRect> rules;
    for (const MarkItem& it : items_) {
        int w = fm.horizontalAdvance(it.label) + 24 + (it.checkable ? 18 : 0) + (it.sub.isEmpty() ? 0 : 14);
        QRect r;
        if (it.dir >= 0) {
            double a = it.dir * M_PI / 4.0;
            int x = int(std::sin(a) * kRadius), y = int(-std::cos(a) * kRadius * 0.82);
            // Side items anchor their near edge to the circle, as the reference's do.
            int left = it.dir == 0 || it.dir == 4 ? x - w / 2 : (it.dir < 4 ? x - 14 : x - w + 14);
            r = QRect(left, y - kH / 2, w, kH);
        } else {
            if (it.separator) {
                rules.append(QRect(-70, list_y, list_w, 7));
                list_y += 7;
            }
            r = QRect(-70, list_y, list_w, kH);
            list_y += kH;
        }
        rs.append(r);
    }
    QRect bound(-6, -6, 12, 12);
    for (const QRect& r : rs) bound = bound.united(r);
    bound.adjust(-4, -4, 4, 4);
    center_ = -bound.topLeft();
    for (QRect& r : rs) r.translate(center_);
    for (QRect& r : rules) r.translate(center_);
    rects_ = rs;
    rules_ = rules;
    resize(bound.size());
    move(global - center_);
    // Without a compositor the translucent background shows black: show only the items and the centre.
    QRegion mask(QRect(center_ - QPoint(6, 6), QSize(12, 12)), QRegion::Ellipse);
    for (const QRect& r : rects_) mask += r.adjusted(-1, -1, 1, 1);
    for (const QRect& r : rules_) mask += r.adjusted(-1, 0, 1, 0);
    setMask(mask);
    opened_ms_ = QDateTime::currentMSecsSinceEpoch();
}

int MarkingMenu::item_at(QPoint p) const {
    for (int i = 0; i < rects_.size(); ++i)
        if (rects_[i].contains(p) && items_[i].enabled) return i;
    // Past the dead zone, the direction picks the compass item.
    QPoint d = p - center_;
    if (std::hypot(d.x(), d.y()) > 28) {
        double a = std::atan2(double(d.x()), double(-d.y()));
        int dir = int(std::lround(a / (M_PI / 4.0)) + 8) % 8;
        for (int i = 0; i < items_.size(); ++i)
            if (items_[i].dir == dir && items_[i].enabled && p.y() < rects_.value(i).bottom() + kRadius) return i;
    }
    return -1;
}

std::function<void(const QString&, const std::function<void()>&)>& MarkingMenu::around() {
    static std::function<void(const QString&, const std::function<void()>&)> hook;
    return hook;
}

void MarkingMenu::open_sub(int i) {
    const QList<MarkItem> sub = items_[i].sub;
    const QPoint at = mapToGlobal(rects_[i].center());
    releaseMouse();
    close();
    show_at(at, sub);
}

void MarkingMenu::pick(int i) {
    if (i >= 0 && !items_[i].sub.isEmpty()) {
        open_sub(i);
        return;
    }
    releaseMouse();
    close();
    if (i >= 0 && items_[i].fn) {
        auto fn = items_[i].fn;
        if (around()) around()(items_[i].label, fn);
        else fn();
    }
}

void MarkingMenu::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.setRenderHint(QPainter::Antialiasing);
    p.setPen(QPen(QColor(30, 30, 30), 1.0));
    p.setBrush(QColor(200, 200, 200));
    p.drawEllipse(center_, 4, 4);
    if (hover_ >= 0 && items_[hover_].dir >= 0) {
        p.setPen(QPen(QColor(230, 230, 230), 1.5));
        p.drawLine(center_, rects_[hover_].center());
    }
    for (int i = 0; i < items_.size(); ++i) {
        const MarkItem& it = items_[i];
        QRect r = rects_[i];
        bool hot = i == hover_;
        p.setPen(QPen(QColor(25, 25, 25), 1.0));
        p.setBrush(hot ? QColor(82, 133, 166) : QColor(78, 78, 78));
        p.drawRoundedRect(r, 2, 2);
        const QColor ink = it.enabled ? (hot ? QColor(Qt::white) : QColor(225, 225, 225)) : QColor(130, 130, 130);
        p.setPen(ink);
        QString text = it.label;
        int left = 10;
        if (it.checkable) {
            // A box, ticked when on, as the reference draws its option items.
            QRect box(r.left() + 8, r.center().y() - 5, 11, 11);
            p.setPen(QPen(QColor(20, 20, 20), 1.0));
            p.setBrush(QColor(48, 48, 48));
            p.drawRect(box);
            if (it.checked) {
                p.setPen(QPen(ink, 1.6));
                p.drawPolyline(QPolygon({box.topLeft() + QPoint(2, 5), box.topLeft() + QPoint(5, 8), box.topLeft() + QPoint(9, 2)}));
            }
            p.setPen(ink);
            left = 26;
        } else if (it.checked) {
            text = QString::fromUtf8("✓ ") + text;
        }
        p.drawText(r.adjusted(left, 0, -6, 0), Qt::AlignVCenter | Qt::AlignLeft, text);
        if (!it.sub.isEmpty()) {
            const int x = r.right() - 11, y = r.center().y();
            p.setPen(Qt::NoPen);
            p.setBrush(ink);
            p.drawPolygon(QPolygon({QPoint(x, y - 4), QPoint(x + 5, y), QPoint(x, y + 4)}));
        }
    }
    for (const QRect& r : rules_) {
        p.setPen(Qt::NoPen);
        p.setBrush(QColor(78, 78, 78));
        p.drawRect(r);
        p.setPen(QPen(QColor(112, 112, 112), 1.0));
        p.drawLine(r.left() + 6, r.center().y(), r.right() - 6, r.center().y());
    }
}

void MarkingMenu::mouseMoveEvent(QMouseEvent* e) {
    QPoint pos = e->position().toPoint();
    if ((pos - center_).manhattanLength() > 8) dragged_ = true;
    int h = item_at(pos);
    // On a submenu's own item, its menu opens there and the drag goes on in it.
    if (h >= 0 && !items_[h].sub.isEmpty() && rects_[h].contains(pos)) {
        open_sub(h);
        return;
    }
    if (h != hover_) {
        hover_ = h;
        update();
    }
}

void MarkingMenu::mouseReleaseEvent(QMouseEvent* e) {
    QPoint pos = e->position().toPoint();
    int i = item_at(pos);
    qint64 held = QDateTime::currentMSecsSinceEpoch() - opened_ms_;
    // A quick click leaves the menu open for a second click.
    if (i < 0 && !dragged_ && held < 400) return;
    pick(i);
}

void MarkingMenu::mousePressEvent(QMouseEvent* e) {
    QPoint pos = e->position().toPoint();
    if (!rect().contains(pos)) {
        pick(-1);
        return;
    }
    dragged_ = true;
}

void MarkingMenu::keyPressEvent(QKeyEvent* e) {
    if (e->key() == Qt::Key_Escape) pick(-1);
}

} // namespace az
