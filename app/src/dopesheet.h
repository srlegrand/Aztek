// The Dope Sheet: the selection's keys as ticks over time, per object and channel. And the Set Driven Key window.
#pragma once
#include <QDialog>
#include <QJsonArray>
#include <QJsonObject>
#include <QSet>
#include <QWidget>

class QLineEdit;
class QListWidget;
class QTreeWidget;
class QTreeWidgetItem;
class QLabel;

namespace az {

struct DopeRow {
    QString label;
    int depth = 0;
    // Curves the row covers: its keys are theirs.
    QStringList curves;
    bool is_channel = false;
};

class DopeCanvas : public QWidget {
    Q_OBJECT
public:
    explicit DopeCanvas(QWidget* parent = nullptr);
    void set_data(const QVector<DopeRow>& rows, const QHash<QString, QVector<QPair<double, bool>>>& keys, double time);
    void frame_all();
    void set_row_height(int h) { row_h_ = h; update(); }
    void set_scroll(int y) { scroll_ = y; update(); }
    // What a left drag does here, as the key contexts set it.
    static void set_tool(const QString& t);
signals:
    void edited();
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void wheelEvent(QWheelEvent*) override;
    void keyPressEvent(QKeyEvent*) override;
private:
    double x_of(double t) const { return (t - t0_) / tpp_; }
    double t_of(double x) const { return t0_ + x * tpp_; }
    int row_at(double y) const;
    QJsonArray selected_refs() const;
    void select_in(QRectF rect, bool add);
    QVector<DopeRow> rows_;
    QHash<QString, QVector<QPair<double, bool>>> keys_;
    // Selected keys: curve and frame.
    QSet<QPair<QString, double>> sel_;
    double time_ = 1, t0_ = -2, tpp_ = 0.25;
    int row_h_ = 20, scroll_ = 0;
    enum class Mode { None, Marquee, Move, Pan, Zoom, Scrub } mode_ = Mode::None;
    QPointF press_, last_;
    double drag_frames_ = 0;
    bool framed_ = false;
};

class DopeSheet : public QWidget {
    Q_OBJECT
public:
    explicit DopeSheet(QWidget* parent = nullptr);
    void refresh();
private:
    QTreeWidget* tree_ = nullptr;
    DopeCanvas* canvas_ = nullptr;
    QString tree_key_;
};

class SetDrivenKeyWindow : public QDialog {
    Q_OBJECT
public:
    explicit SetDrivenKeyWindow(QWidget* parent = nullptr);
private:
    void load(QListWidget* objs, QListWidget* attrs);
    void show_attrs(QListWidget* objs, QListWidget* attrs);
    void key();
    QListWidget *driver_objs_, *driver_attrs_, *driven_objs_, *driven_attrs_;
};

} // namespace az
