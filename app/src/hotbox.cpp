// The hotbox: Space held, or hotBox from a script. Rows of menus at the cursor, the zones around them each with a
// marking menu on a press. Which rows show, how much is drawn and how see-through it is come from hotBox's settings.
#include "markingmenu.h"
#include "widgets.h"

#include <QDateTime>
#include <QBoxLayout>
#include <QGraphicsOpacityEffect>
#include <QGridLayout>
#include <QGuiApplication>
#include <QKeyEvent>
#include <QLabel>
#include <QMenu>
#include <QMouseEvent>
#include <QPainter>
#include <QScreen>
#include <QSignalBlocker>
#include <QToolButton>
#include <algorithm>
#include <cstdlib>

namespace az {

namespace {

// hotBox's row for a menu set's label.
QString set_row(const QString& label) {
    const QString n = label.toLower();
    return n == "fx" ? QString("dynamics") : n;
}

} // namespace

Hotbox::Hotbox(const QList<QMenu*>& common, const QList<QPair<QString, QList<QMenu*>>>& sets, QMenu* recent, QWidget* parent)
    : QWidget(parent, Qt::Popup | Qt::FramelessWindowHint) {
    setAttribute(Qt::WA_TranslucentBackground);
    setMouseTracking(true);
    box_ = new QWidget(this);
    auto* rows = new QVBoxLayout(box_);
    rows->setContentsMargins(0, 0, 0, 0);
    rows->setSpacing(6);
    auto button = [this](const QString& title, QMenu* menu = nullptr) {
        auto* b = new QToolButton;
        b->setText(QString(title).remove('&'));
        b->setMenu(menu);
        b->setPopupMode(QToolButton::InstantPopup);
        b->setObjectName("hotboxButton");
        if (menu) connect(menu, &QMenu::aboutToShow, this, &Hotbox::mark_used, Qt::UniqueConnection);
        return b;
    };
    auto row = [rows] {
        auto* strip = new QWidget;
        auto* items = new QHBoxLayout(strip);
        items->setContentsMargins(0, 0, 0, 0);
        items->setSpacing(0);
        rows->addWidget(strip, 0, Qt::AlignHCenter);
        return items;
    };
    auto menu_row = [this, &row, &button](const QList<QMenu*>& menus) {
        auto* items = row();
        for (auto* menu : menus) {
            auto* b = button(menu->title(), menu);
            items->addWidget(b);
            // The Modeling row's polygon and surface menus show alone under hotBox's polygons and surfaces rows.
            const QString group = menu->property("azHotboxGroup").toString();
            if (!group.isEmpty()) groups_.append({b, group});
        }
        return items->parentWidget();
    };
    strips_["common"] = menu_row(common);

    auto* pane = row();
    for (const char* label : {"View", "Shading", "Lighting", "Show", "Renderer", "Panels"}) {
        auto* b = button(label);
        pane->addWidget(b);
        panel_buttons_.append(b);
    }
    strips_["pane"] = pane->parentWidget();

    auto* center = new QWidget;
    auto* controls_row = new QGridLayout(center);
    controls_row->setContentsMargins(0, 6, 0, 6);
    controls_row->setHorizontalSpacing(54);
    auto* recents = button("Recent Commands", recent);
    auto* controls = new QMenu("Hotbox Controls", this);
    auto* options = button(controls->title(), controls);
    auto* mid = new QLabel("Aztek");
    mid->setObjectName("hotboxCenter");
    mid->setAlignment(Qt::AlignCenter);
    middle_ = mid;
    controls_row->addWidget(recents, 0, 0, Qt::AlignRight | Qt::AlignVCenter);
    controls_row->addWidget(mid, 0, 1);
    controls_row->addWidget(options, 0, 2, Qt::AlignLeft | Qt::AlignVCenter);
    const int side = qMax(recents->sizeHint().width(), options->sizeHint().width());
    controls_row->setColumnMinimumWidth(0, side);
    controls_row->setColumnMinimumWidth(2, side);
    rows->addWidget(center, 0, Qt::AlignHCenter);

    for (const auto& [label, menus] : sets) {
        const QString name = set_row(label);
        strips_[name] = menu_row(menus);
        auto* action = controls->addAction(label);
        action->setCheckable(true);
        action->setChecked(true);
        toggles_[name] = action;
        // A Hotbox Controls row goes to hotBox, so the setting holds; Modeling stands for the polygon and surface rows
        // too, FX for cloth.
        connect(action, &QAction::toggled, this, [this, name](bool on) {
            QStringList names = {name};
            if (name == "modeling") names << "polygons" << "surfaces";
            if (name == "dynamics") names << "cloth";
            for (const QString& n : names) {
                rows_[n] = on;
                emit rowToggled(n, on);
            }
            apply_rows();
            if (isVisible()) place(center_);
        });
    }
}

void Hotbox::mark_used() {
    // The menus are the menu bar's too: only a use while the hotbox is up counts.
    if (!isVisible()) return;
    if (!used_) emit used();
    used_ = true;
}

void Hotbox::set_rows_shown(const QHash<QString, bool>& rows) {
    rows_ = rows;
}

void Hotbox::set_look(const QString& style, int transparency, bool rmb, const QList<QMenu*>& current) {
    style_ = style.isEmpty() ? QString("dh") : style;
    transparency_ = std::clamp(transparency, 0, 100);
    rmb_ = rmb;
    current_ = current;
}

void Hotbox::apply_rows() {
    auto on = [this](const char* r) { return rows_.value(r, true); };
    // Zones only and center only: no rows of menus.
    const bool menus = style_ == "dh";
    for (auto it = strips_.begin(); it != strips_.end(); ++it) {
        const QString& name = it.key();
        bool shown = on(name.toUtf8().constData());
        if (name == "modeling") shown = shown || on("polygons") || on("surfaces");
        if (name == "dynamics") shown = shown || on("cloth");
        it.value()->setVisible(menus && shown);
    }
    for (const auto& [b, group] : groups_) b->setVisible(on("modeling") || on(group.toUtf8().constData()));
    // The Modeling row's other menus (Deform, Generate) only with the whole row.
    if (auto* strip = strips_.value("modeling")) {
        for (auto* b : strip->findChildren<QToolButton*>(Qt::FindDirectChildrenOnly)) {
            const bool grouped = std::any_of(groups_.begin(), groups_.end(), [b](const auto& g) { return g.first == b; });
            if (!grouped) b->setVisible(on("modeling"));
        }
    }
    for (auto it = toggles_.begin(); it != toggles_.end(); ++it) {
        QSignalBlocker block(it.value());
        it.value()->setChecked(on(it.key().toUtf8().constData()));
    }
    // Center only: nothing of it is drawn.
    box_->setVisible(style_ != "dco");
    // -transparenyLevel: up to the default (25) as solid as the menus are drawn, then fading to 100.
    const double fade = std::clamp((transparency_ - 25) / 75.0, 0.0, 1.0);
    if (fade <= 0.0) {
        box_->setGraphicsEffect(nullptr);
    } else {
        auto* effect = qobject_cast<QGraphicsOpacityEffect*>(box_->graphicsEffect());
        if (!effect) {
            effect = new QGraphicsOpacityEffect(box_);
            box_->setGraphicsEffect(effect);
        }
        effect->setOpacity(1.0 - 0.6 * fade);
    }
}

void Hotbox::place(QPoint target) {
    // Center on the camera tile after Qt has laid out the nested rows.
    box_->layout()->invalidate();
    box_->layout()->activate();
    box_->adjustSize();
    middle_->parentWidget()->layout()->activate();
    const QPoint mid = middle_->mapTo(box_, middle_->rect().center());
    const QPoint pos = target - mid;
    box_->move(qBound(0, pos.x(), qMax(0, width() - box_->width())),
               qBound(0, pos.y(), qMax(0, height() - box_->height())));
    center_ = box_->pos() + mid;
    update();
}

void Hotbox::popup(QPoint global, const QList<QAction*>& panel_actions) {
    origin_ = global;
    for (auto* b : panel_buttons_) {
        QMenu* menu = nullptr;
        for (auto* action : panel_actions) {
            if (QString(action->text()).remove('&') != b->text()) continue;
            menu = action->menu();
            break;
        }
        b->setMenu(menu);
        b->setEnabled(menu);
        b->setToolTip(menu ? QString() : QStringLiteral("Unavailable in this panel"));
        if (menu) connect(menu, &QMenu::aboutToShow, this, &Hotbox::mark_used, Qt::UniqueConnection);
    }
    apply_rows();
    QScreen* s = QGuiApplication::screenAt(global);
    QRect area = s ? s->geometry() : parentWidget()->window()->geometry();
    setGeometry(area);
    center_ = global - area.topLeft();
    shown_ms_ = QDateTime::currentMSecsSinceEpoch();
    used_ = false;
    show();
    place(center_);
    activateWindow();
    setFocus();
}

int Hotbox::zone_at(QPoint p) const {
    // Center only: the middle's marking menu anywhere.
    if (style_ == "dco") return 4;
    // The Aztek box; the rest of the menus' block; outside it the screen splits along its diagonals, as the reference's
    // zones do.
    QRect mid = box_->geometry();
    if (middle_->rect().contains(middle_->mapFrom(this, p))) return 4;
    QPoint d = p - center_;
    if (std::abs(d.x()) * mid.height() > std::abs(d.y()) * mid.width()) return d.x() > 0 ? 1 : 3;
    return d.y() > 0 ? 2 : 0;
}

void Hotbox::paintEvent(QPaintEvent*) {
    QPainter p(this);
    p.setCompositionMode(QPainter::CompositionMode_Source);
    // Nearly invisible, but still hit-testable in the marking zones on Windows.
    p.fillRect(rect(), QColor(0, 0, 0, 1));
    p.setCompositionMode(QPainter::CompositionMode_SourceOver);
    if (style_ == "dco") return;
    p.setRenderHint(QPainter::Antialiasing);
    QRect mid = box_->geometry();
    // Short diagonal guides mark the outer zones without covering the viewport.
    p.setPen(QPen(QColor(230, 230, 230, 165), 1));
    for (QPoint c : {mid.topLeft(), mid.topRight(), mid.bottomLeft(), mid.bottomRight()}) {
        const QPointF d(c.x() < center_.x() ? -1 : 1, c.y() < center_.y() ? -1 : 1);
        p.drawLine(QPointF(c) + d * 10, QPointF(c) + d * 26);
    }
}

void Hotbox::mousePressEvent(QMouseEvent* e) {
    QPoint at = e->position().toPoint();
    int zone = zone_at(at);
    mark_used();
    QPoint global = e->globalPosition().toPoint();
    // -rmbPopups: a right press in the middle lists the current menu set's menus.
    if (zone == 4 && e->button() == Qt::RightButton && rmb_ && !current_.isEmpty()) {
        auto* list = new QMenu(this);
        list->setAttribute(Qt::WA_DeleteOnClose);
        for (QMenu* m : current_)
            if (m) list->addMenu(m);
        list->popup(global);
        return;
    }
    if (!zones_) return;
    QList<MarkItem> items = zones_(zone, origin_);
    if (!items.isEmpty()) MarkingMenu::show_at(global, items, this, zone < 4);
}

void Hotbox::finish() {
    const bool tap = !used_ && QDateTime::currentMSecsSinceEpoch() - shown_ms_ < 220;
    hide();
    emit spaceReleased(tap);
}

void Hotbox::keyReleaseEvent(QKeyEvent* e) {
    // Popped with hotBox -noKeyPress, the key's release leaves it up.
    if (e->key() == Qt::Key_Space && !e->isAutoRepeat() && !hold_) finish();
}

} // namespace az
