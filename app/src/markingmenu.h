// Reference-style marking menus: items at the eight compass points round the cursor plus a list below. Drag toward an
// item and release to pick it, or release in place and click.
#pragma once
#include <QIcon>
#include <QRegion>
#include <QWidget>
#include <functional>

class QTimer;

namespace az {

struct MarkItem {
    QString label;
    // 0 north, then clockwise to 7 north-west; -1 in the list below.
    int dir = -1;
    std::function<void()> fn;
    bool enabled = true;
    bool checked = false;
    // Shows a box, ticked when checked.
    bool checkable = false;
    // A rule above this item, in the list.
    bool separator = false;
    // A marking menu of its own, opened when the cursor reaches the item.
    QList<MarkItem> sub = {};
    // The option box at the right end, as the reference's items have: released over it, this runs instead (#183).
    std::function<void()> option = {};
    QIcon icon = {};
};

class MarkingMenu : public QWidget {
    Q_OBJECT
public:
    /// What a picked item runs inside, so G can repeat it.
    static std::function<void(const QString&, const std::function<void()>&)>& around();
    static void show_at(QPoint global, const QList<MarkItem>& items, QWidget* owner = nullptr, bool hotbox_zone = false, bool center_list = false);
    // How long the pointer rests on a submenu's item before it opens.
    static constexpr int kSubmenuDelayMs = 350;
protected:
    void paintEvent(QPaintEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void keyPressEvent(QKeyEvent*) override;
    void keyReleaseEvent(QKeyEvent*) override;
private:
    MarkingMenu(QPoint global, const QList<MarkItem>& items, QWidget* owner, bool hotbox_zone, bool center_list);
    static void open(QPoint global, const QList<MarkItem>& items, QWidget* owner, bool hotbox_zone, bool center_list);
    void back_to_parent();
    int item_at(QPoint local, bool stroke) const;
    void pick(int i, bool option = false);
    // Where item i draws its option box.
    QRect option_box(int i) const;
    void open_sub(int i);
    void update_shape();
    QList<QRect> rules_;
    QList<MarkItem> items_;
    QList<QRect> rects_;
    QPoint center_;
    QPoint opening_point_;
    QPoint cursor_;
    QRegion shape_;
    bool hotbox_zone_;
    bool center_list_;
    int hover_ = -1;
    bool dragged_ = false;
    qint64 opened_ms_ = 0;
    // A submenu's way back: a dot where the menu it came from was centred.
    bool has_back_ = false;
    QPoint back_;
    QTimer* sub_timer_ = nullptr;
    int sub_pending_ = -1;
};

} // namespace az
