// Model panels: a native child window the Rust renderer draws into, with the reference's panel menus and icon bar above it.
#pragma once
#include <functional>
#include <QHash>
#include <QWidget>

class QStackedLayout;
class QMenuBar;
class QToolBar;
class QGridLayout;

namespace az {

class ViewportWidget : public QWidget {
    Q_OBJECT
public:
    explicit ViewportWidget(const QString& camera, QWidget* parent = nullptr);
    ~ViewportWidget() override;
    int id() const { return id_; }
    QPaintEngine* paintEngine() const override { return nullptr; }
    void set(const QString& key, const QString& json);
    QString get(const QString& key) const;
    QString camera() const { return camera_; }
    bool capture(const QString& path);
signals:
    void rightClick(QPoint global, int mods);
    void activated(ViewportWidget* self);
protected:
    void showEvent(QShowEvent*) override;
    void paintEvent(QPaintEvent*) override;
    void resizeEvent(QResizeEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void mouseDoubleClickEvent(QMouseEvent*) override;
    void leaveEvent(QEvent*) override;
    void wheelEvent(QWheelEvent*) override;
    bool event(QEvent*) override;
private:
    void ensure();
    void mouse(int kind, QMouseEvent* e);
    // Moves are handled once per pass of the event loop, the latest one: a fast mouse sends hundreds a second, and
    // each would evaluate the scene for a place the pointer has already left.
    void queue_move(QMouseEvent* e);
    void flush_move();
    bool move_queued_ = false;
    QPointF move_at_;
    int move_mods_ = 0, move_buttons_ = 0;
    // Settings sent before the view's surface exists, latest per key.
    QHash<QString, QString> pending_;
    int id_ = 0;
    QString camera_;
    bool rmb_pending_ = false;
    QPoint rmb_pos_;
    // The reference's virtual slider: middle-drag with channels picked in the channel box changes them.
    bool sliding_ = false;
    QPointF slide_start_;
    QList<QPair<QString, double>> slide_plugs_;
    int slide_applied_ = 0;
    void slide_to(double dx, bool final);
};

class ModelPanel : public QWidget {
    Q_OBJECT
public:
    explicit ModelPanel(const QString& camera, QWidget* parent = nullptr);
    ViewportWidget* view() const { return view_; }
    // What the panel shows: model, outliner, graph, dope or node.
    void set_panel_type(const QString& type);
    QString panel_type() const { return type_; }
private:
    void build_menus();
    QMenuBar* menus_;
    QToolBar* bar_;
    // Anti-aliasing from the toolbar: on or off, and a button to keep in step.
    std::function<void(bool)> aa_apply_;
    std::function<void(QAction*)> aa_register_;
    ViewportWidget* view_;
    QStackedLayout* stack_ = nullptr;
    QHash<QString, QWidget*> others_;
    QString type_ = "model";
};

// The central area: one panel, or the four views. Tracks the panel under the cursor for hotkeys.
class PanelLayout : public QWidget {
    Q_OBJECT
public:
    explicit PanelLayout(QWidget* parent = nullptr);
    void set_single(bool single);
    bool single() const { return kind_ == 0; }
    // 0 single, 1 two side by side, 2 two stacked, 3 three, 4 four.
    void set_layout(int kind);
    int layout_kind() const { return kind_; }
    QString state_json() const;
    void restore_json(const QString& json);
    // The view under the cursor, else the last one used.
    ViewportWidget* active() const;
    QList<ViewportWidget*> views() const;
    // Single view of the panel under the cursor, or back to four (space tap).
    void toggle();
    void redraw_all();
    void set_all(const QString& key, const QString& json);
signals:
    void rightClick(QPoint global, int mods);
private:
    void relayout();
    QList<ModelPanel*> panels_;
    QGridLayout* grid_;
    int kind_ = 0;
    int last_multi_ = 4;
    int focus_ = 0;
    mutable ViewportWidget* last_ = nullptr;
};

} // namespace az
