// The Graph Editor: the selection's animation curves, their keys and tangents, edited as in the reference.
#pragma once
#include <array>
#include <QJsonArray>
#include <QJsonObject>
#include <QPointF>
#include <QSet>
#include <QWidget>

class QTreeWidget;
class QToolBar;
class QLineEdit;
class QVBoxLayout;

namespace az {

struct CurveView {
    QString curve, channel, node;
    int component = -1;
    QColor color;
    QVector<QPointF> samples;
    // time, value, in slope, out slope
    QVector<std::array<double, 4>> keys;
    // Handle lengths in frames and value: in x, in y, out x, out y. Drawn on weighted curves.
    QVector<std::array<double, 4>> handles;
    bool weighted = false;
    QVector<QPointF> buffer;
    QStringList itt, ott;
    bool visible = true;
};

enum class CurveDisplay { Absolute, Stacked, Normalized };

class CurveCanvas : public QWidget {
    Q_OBJECT
public:
    explicit CurveCanvas(QWidget* parent = nullptr);
    void set_curves(const QVector<CurveView>& c, double time);
    void frame(bool selected_only);
    QList<QPair<QString, int>> selected_keys() const;
    void select_all_keys_of(const QSet<QString>& curves);
    QVector<CurveView>& curves() { return curves_; }
    void set_display(CurveDisplay d);
    // What a left drag does here, as the key contexts set it: select, move, scale, set, insert, dolly, track or time.
    static void set_tool(const QString& t);
    static QString tool();
    // The selected keys as frames, curve by curve.
    QList<QPair<QString, double>> selected_times() const;
    // Frames to sample curves over, and the step.
    std::array<double, 3> sample_window() const;
signals:
    void keysChanged();
    void selectionChanged();
    void timeDragged(double t);
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void wheelEvent(QWheelEvent*) override;
    void keyPressEvent(QKeyEvent*) override;
private:
    QPointF to_screen(double t, double v) const;
    QPointF to_graph(QPointF p) const;
    // A curve's point on screen, through the stacked or normalized view.
    QPointF curve_point(int ci, double t, double v) const;
    void update_maps();
    void nudge_keys(double frames);
    void retime_keys(double factor);
    QPair<int, int> key_at(QPointF p) const;
    void apply_drag(QPointF to, bool final);

    QVector<CurveView> curves_;
    QSet<QPair<int, int>> sel_;
    double time_ = 1;
    // View: graph point at the canvas origin and units per pixel.
    double t0_ = -2, v0_ = 12, tpp_ = 0.2, vpp_ = 0.05;
    enum class Mode { None, Pan, Zoom, Marquee, Move, Scrub } mode_ = Mode::None;
    QPointF press_, last_;
    QPointF drag_delta_;
    bool drag_applied_ = false;
    int axis_lock_ = 0;
    bool framed_ = false;
    CurveDisplay display_ = CurveDisplay::Absolute;
    // Per curve: value * first + second is what the view shows.
    QVector<QPair<double, double>> maps_;
};

class GraphEditor : public QWidget {
    Q_OBJECT
public:
    explicit GraphEditor(QWidget* parent = nullptr);
    void refresh();
private:
    void rebuild_tree();
    void set_tangents(const QString& in, const QString& out);
    void build_menus(QVBoxLayout* v);
    // The curves of the selected keys, or of the tree's selection.
    QJsonArray target_curves() const;
    // The canvas's picked keys become the scene's key selection, for commands taking -sl.
    bool sync_selection();
    void isolate(bool on);
    QTreeWidget* tree_ = nullptr;
    CurveCanvas* canvas_ = nullptr;
    QLineEdit *stats_t_ = nullptr, *stats_v_ = nullptr;
    QString tree_key_;
};

} // namespace az
