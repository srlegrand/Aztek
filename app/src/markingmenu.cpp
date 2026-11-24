#include "markingmenu.h"

#include <QCoreApplication>
#include <QDateTime>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QLineF>
#include <QMouseEvent>
#include <QPainter>
#include <QPainterPath>
#include <QScreen>
#include <QTimer>
#include <cmath>

namespace az {

namespace {
const int kRadius = 92;
const int kH = 22;
// Between compass rows, and from the dot to a side item's near edge.
const int kRow = 27;
const int kSide = 55;
// The menus a submenu was opened from, outermost first: where each was centred and what it held.
QList<QPair<QPoint, QList<MarkItem>>>& parents() {
    static QList<QPair<QPoint, QList<MarkItem>>> list;
    return list;
}
} // namespace

void MarkingMenu::show_at(QPoint global, const QList<MarkItem>& items, QWidget* owner, bool hotbox_zone, bool center_list) {
    parents().clear();
    open(global, items, owner, hotbox_zone, center_list);
}

void MarkingMenu::open(QPoint global, const QList<MarkItem>& items, QWidget* owner, bool hotbox_zone, bool center_list) {
    auto* m = new MarkingMenu(global, items, owner, hotbox_zone, center_list);
    m->setAttribute(Qt::WA_DeleteOnClose);
    m->show();
    m->grabMouse();
    m->setFocus();
}

MarkingMenu::MarkingMenu(QPoint global, const QList<MarkItem>& items, QWidget* owner, bool hotbox_zone, bool center_list)
    : QWidget(owner, Qt::Popup | Qt::FramelessWindowHint | Qt::NoDropShadowWindowHint), items_(items),
      opening_point_(global), hotbox_zone_(hotbox_zone), center_list_(center_list) {
    setAttribute(Qt::WA_TranslucentBackground);
    setMouseTracking(true);
    if (hotbox_zone_) {
        QFont text = font();
        text.setPixelSize(11);
        text.setBold(true);
        setFont(text);
    }
    QFontMetrics fm(font());
    // Lay out round a centre, then size the window to fit.
    QList<QRect> rs;
    // A menu with nothing round the compass (a submenu's list) starts just under the cursor.
    bool compass = false;
    int list_w = 140;
    for (const MarkItem& it : items_) {
        compass |= it.dir >= 0;
        if (it.dir < 0) list_w = qMax(list_w, fm.horizontalAdvance(it.label) + 24 + (it.checkable || !it.icon.isNull() ? 18 : 0) + (it.sub.isEmpty() ? 0 : 14) + (it.option ? 22 : 0));
    }
    const int list_x = compass && center_list_ ? -list_w / 2 : -85;
    int list_y = compass ? 2 * kRow + kH + 10 : 14;
    QList<QRect> rules;
    for (const MarkItem& it : items_) {
        int w = fm.horizontalAdvance(it.label) + (hotbox_zone_ ? 50 : 24)
            + (it.checkable || !it.icon.isNull() ? 18 : 0) + (it.sub.isEmpty() || hotbox_zone_ ? 0 : 14) + (it.option ? 22 : 0);
        QRect r;
        if (it.dir >= 0) {
            // The reference's compass (#183): five rows a little over an item apart, north and south centred over the
            // dot, the sides with their near edge on a narrow ellipse.
            static const int rows[8] = {-2, -1, 0, 1, 2, 1, 0, -1};
            const double a = it.dir * M_PI / 4.0;
            const int x = int(std::lround(std::sin(a) * kSide)), y = rows[it.dir] * kRow;
            int left = it.dir == 0 || it.dir == 4 ? x - w / 2 : (it.dir < 4 ? x : x - w);
            const int h = hotbox_zone_ ? 21 : kH;
            r = QRect(left, y - h / 2, w, h);
        } else {
            if (it.separator) {
                rules.append(QRect(list_x, list_y, list_w, 7));
                list_y += 7;
            }
            r = QRect(list_x, list_y, list_w, kH);
            list_y += kH;
        }
        rs.append(r);
    }
    QRect bound(-6, -6, 12, 12);
    for (const QRect& r : rs) bound = bound.united(r);
    if (hotbox_zone_ || center_list_) {
        if (QScreen* s = QGuiApplication::screenAt(global)) {
            const QRect area = s->availableGeometry().adjusted(4, 4, -4, -4);
            if (area.width() >= bound.width())
                global.setX(qBound(area.left() - bound.left(), global.x(), area.right() - bound.right()));
            if (area.height() >= bound.height())
                global.setY(qBound(area.top() - bound.top(), global.y(), area.bottom() - bound.bottom()));
        }
    }
    if (!parents().isEmpty()) {
        has_back_ = true;
        back_ = parents().last().first - global;
        bound = bound.united(QRect(back_ - QPoint(7, 7), QSize(14, 14)));
    }
    bound.adjust(-4, -4, 4, 4);
    center_ = -bound.topLeft();
    for (QRect& r : rs) r.translate(center_);
    for (QRect& r : rules) r.translate(center_);
    rects_ = rs;
    rules_ = rules;
    resize(bound.size());
    move(global - center_);
    cursor_ = opening_point_ - (global - center_);
    // Without a compositor the translucent background shows black: show only the items and the centre.
    shape_ = QRegion(QRect(center_ - QPoint(7, 7), QSize(14, 14)), QRegion::Ellipse);
    for (const QRect& r : rects_) shape_ += r.adjusted(-1, -1, 1, 1);
    for (const QRect& r : rules_) shape_ += r.adjusted(-1, 0, 1, 0);
    if (has_back_) {
        back_ += center_;
        shape_ += QRegion(QRect(back_ - QPoint(7, 7), QSize(14, 14)), QRegion::Ellipse);
    }
    // XWayland composites the alpha buffer. A native shape can make the popup's backing rectangle visible.
    update_shape();
    sub_timer_ = new QTimer(this);
    sub_timer_->setSingleShot(true);
    sub_timer_->setInterval(kSubmenuDelayMs);
    connect(sub_timer_, &QTimer::timeout, this, [this] {
        if (sub_pending_ >= 0 && sub_pending_ == hover_) open_sub(sub_pending_);
    });
    opened_ms_ = QDateTime::currentMSecsSinceEpoch();
}

void MarkingMenu::update_shape() {
    if (!qEnvironmentVariableIsEmpty("WAYLAND_DISPLAY")) return;
    QRegion mask = shape_;
    if (hotbox_zone_) {
        auto line = [&mask](QPoint from, QPoint to) {
            QPainterPath path;
            path.moveTo(from);
            path.lineTo(to);
            QPainterPathStroker stroke;
            // Only the line pixels belong in the native mask, or the backing rectangle shows.
            stroke.setWidth(2);
            mask += QRegion(stroke.createStroke(path).toFillPolygon().toPolygon());
        };
        if (has_back_) line(back_, center_);
        if (hover_ >= 0 && dragged_) line(center_, cursor_);
    }
    setMask(mask);
}

int MarkingMenu::item_at(QPoint p, bool stroke) const {
    for (int i = 0; i < rects_.size(); ++i)
        if (rects_[i].contains(p)) return items_[i].enabled ? i : -1;

    const QPoint d = p - center_;
    if (!stroke || d.manhattanLength() <= 8) return -1;

    // Mouse events can jump over a whole row. Use the last item crossed on the way to this sample.
    if (p != cursor_) {
        const QLineF segment(p, cursor_);
        qreal nearest = segment.length();
        int crossed = -1;
        for (int i = 0; i < rects_.size(); ++i) {
            const QRect& r = rects_[i];
            if (!items_[i].enabled || r.contains(cursor_)) continue;
            const QPolygonF border{QRectF(r)};
            for (int edge = 1; edge < border.size(); ++edge) {
                QPointF hit;
                if (segment.intersects(QLineF(border[edge - 1], border[edge]), &hit) != QLineF::BoundedIntersection) continue;
                const qreal distance = QLineF(p, hit).length();
                if (distance >= nearest) continue;
                nearest = distance;
                crossed = i;
            }
        }
        if (crossed >= 0) return crossed;
    }

    // A compass mark can also pick an item by direction before reaching its box.
    if (std::hypot(d.x(), d.y()) > 28) {
        double a = std::atan2(double(d.x()), double(-d.y()));
        int dir = int(std::lround(a / (M_PI / 4.0)) + 8) % 8;
        for (int i = 0; i < items_.size(); ++i)
            if (items_[i].dir == dir && items_[i].enabled && p.y() < rects_.value(i).bottom() + kRadius) return i;
    }
    return hover_;
}

std::function<void(const QString&, const std::function<void()>&)>& MarkingMenu::around() {
    static std::function<void(const QString&, const std::function<void()>&)> hook;
    return hook;
}

void MarkingMenu::open_sub(int i) {
    const QList<MarkItem> sub = items_[i].sub;
    QPoint at = mapToGlobal(rects_[i].center());
    if (hotbox_zone_ && items_[i].dir == 2) at.setX(mapToGlobal(rects_[i].topRight()).x() + 40);
    parents().append({mapToGlobal(center_), items_});
    releaseMouse();
    close();
    open(at, sub, parentWidget(), hotbox_zone_, center_list_);
}

// Back over the dot: the menu this one came from opens again where it was.
void MarkingMenu::back_to_parent() {
    if (parents().isEmpty()) return;
    const auto up = parents().takeLast();
    releaseMouse();
    close();
    open(up.first, up.second, parentWidget(), hotbox_zone_, center_list_);
}

QRect MarkingMenu::option_box(int i) const {
    const QRect r = rects_[i];
    return QRect(r.right() - 17, r.center().y() - 5, 11, 11);
}

void MarkingMenu::pick(int i, bool option) {
    if (i >= 0 && !items_[i].sub.isEmpty()) {
        open_sub(i);
        return;
    }
    releaseMouse();
    close();
    parents().clear();
    if (i < 0) return;
    if (option && items_[i].option) {
        items_[i].option();
        return;
    }
    if (items_[i].fn) {
        auto fn = items_[i].fn;
        if (around()) around()(items_[i].label, fn);
        else fn();
    }
}

void MarkingMenu::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.setCompositionMode(QPainter::CompositionMode_Source);
    p.fillRect(rect(), Qt::transparent);
    p.setCompositionMode(QPainter::CompositionMode_SourceOver);
    p.setRenderHint(QPainter::Antialiasing);
    if (hotbox_zone_) {
        p.setPen(QPen(QColor(215, 215, 215), 1));
        if (has_back_) p.drawLine(back_, center_);
        if (hover_ >= 0 && dragged_) p.drawLine(center_, cursor_);
    }
    p.setPen(QPen(QColor(30, 30, 30), 1.0));
    const QColor hotbox_dot(112, 112, 112);
    p.setBrush(hotbox_zone_ ? hotbox_dot : QColor(200, 200, 200));
    const int dot = hotbox_zone_ ? 6 : 4;
    p.drawEllipse(center_, dot, dot);
    if (!hotbox_zone_ && hover_ >= 0 && items_[hover_].dir >= 0) {
        p.setPen(QPen(QColor(230, 230, 230), 1.5));
        p.drawLine(center_, rects_[hover_].center());
    }
    if (has_back_) {
        p.setPen(QPen(QColor(30, 30, 30), 1.0));
        p.setBrush(hotbox_zone_ ? hotbox_dot : QColor(160, 160, 160));
        p.drawEllipse(back_, dot, dot);
    }
    for (int i = 0; i < items_.size(); ++i) {
        const MarkItem& it = items_[i];
        QRect r = rects_[i];
        bool hot = i == hover_;
        p.setPen(QPen(QColor(25, 25, 25), 1.0));
        p.setBrush(hot ? QColor(82, 133, 166) : QColor(78, 78, 78));
        if (hotbox_zone_) p.drawRect(r.adjusted(0, 0, -1, -1));
        else p.drawRoundedRect(r, 2, 2);
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
        } else if (!it.icon.isNull()) {
            it.icon.paint(&p, QRect(r.left() + 6, r.center().y() - 7, 14, 14),
                          Qt::AlignCenter, it.enabled ? QIcon::Normal : QIcon::Disabled);
            left = 24;
        } else if (it.checked) {
            text = QString::fromUtf8("✓ ") + text;
        }
        if (hotbox_zone_) p.drawText(r, Qt::AlignCenter, text);
        else p.drawText(r.adjusted(left, 0, -6, 0), Qt::AlignVCenter | Qt::AlignLeft, text);
        if (it.option) {
            const QRect box = option_box(i);
            p.setPen(QPen(ink, 1.0));
            p.setBrush(Qt::NoBrush);
            p.drawRect(box);
        }
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
    const QPoint movement = (hotbox_zone_ || center_list_) && !has_back_ ? e->globalPosition().toPoint() - opening_point_ : pos - center_;
    if (movement.manhattanLength() > 8) dragged_ = true;
    if (has_back_ && (pos - back_).manhattanLength() < 9) {
        back_to_parent();
        return;
    }
    const int h = item_at(pos, e->buttons() != Qt::NoButton);
    const bool repaint = h != hover_ || (hotbox_zone_ && pos != cursor_);
    hover_ = h;
    cursor_ = pos;
    if (repaint) {
        if (hotbox_zone_) update_shape();
        update();
    }
    // On a submenu's own item, its menu opens there once the pointer rests on it, so the list under a compass
    // item can still be reached by passing over it, as in the reference.
    if (h >= 0 && !items_[h].sub.isEmpty() && rects_[h].contains(pos)) {
        if (sub_pending_ != h) {
            sub_pending_ = h;
            sub_timer_->start();
        }
    } else {
        sub_pending_ = -1;
        sub_timer_->stop();
    }
}

void MarkingMenu::mouseReleaseEvent(QMouseEvent* e) {
    QPoint pos = e->position().toPoint();
    const bool opening_click = (hotbox_zone_ || center_list_) && !has_back_ && !dragged_
        && (e->globalPosition().toPoint() - opening_point_).manhattanLength() <= 8;
    const int i = opening_click ? -1 : (pos == cursor_ ? hover_ : item_at(pos, true));
    qint64 held = QDateTime::currentMSecsSinceEpoch() - opened_ms_;
    // A quick click leaves the menu open for a second click.
    if (i < 0 && !dragged_ && held < 400) return;
    pick(i, i >= 0 && items_[i].option && option_box(i).adjusted(-4, -6, 6, 6).contains(pos));
}

void MarkingMenu::mousePressEvent(QMouseEvent* e) {
    QPoint pos = e->position().toPoint();
    if (!rect().contains(pos)) {
        pick(-1, false);
        return;
    }
    dragged_ = true;
}

void MarkingMenu::keyPressEvent(QKeyEvent* e) {
    if (e->key() == Qt::Key_Escape) pick(-1, false);
}

void MarkingMenu::keyReleaseEvent(QKeyEvent* e) {
    if (auto* owner = parentWidget(); owner && e->key() == Qt::Key_Space && !e->isAutoRepeat()) {
        pick(-1, false);
        QCoreApplication::sendEvent(owner, e);
        return;
    }
    QWidget::keyReleaseEvent(e);
}

} // namespace az
