// Reference-style marking menus: items at the eight compass points round the cursor plus a list below. Drag toward an
// item and release to pick it, or release in place and click.
#pragma once
#include <QWidget>
#include <functional>

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
};

class MarkingMenu : public QWidget {
    Q_OBJECT
public:
    /// What a picked item runs inside, so G can repeat it.
    static std::function<void(const QString&, const std::function<void()>&)>& around();
    static void show_at(QPoint global, const QList<MarkItem>& items);
protected:
    void paintEvent(QPaintEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void keyPressEvent(QKeyEvent*) override;
private:
    MarkingMenu(QPoint global, const QList<MarkItem>& items);
    int item_at(QPoint local) const;
    void pick(int i);
    void open_sub(int i);
    QList<QRect> rules_;
    QList<MarkItem> items_;
    QList<QRect> rects_;
    QPoint center_;
    int hover_ = -1;
    bool dragged_ = false;
    qint64 opened_ms_ = 0;
};

} // namespace az
