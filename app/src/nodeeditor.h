// The Node Editor: nodes as boxes with their attributes as ports, wired by their connections.
#pragma once
#include <QHash>
#include <QJsonObject>
#include <QPointF>
#include <QSet>
#include <QVector>
#include <QWidget>

class QLineEdit;

namespace az {

struct NodeBox {
    QString name, type;
    QPointF pos;
    int mode = 2; // 1 no attributes, 2 connected ones, 3 all. Every mode keeps the two ports on the title.
    QStringList attrs;
    QStringList connected;
    QStringList inputs, outputs;
};

class NodeEditor : public QWidget {
    Q_OBJECT
public:
    explicit NodeEditor(QWidget* parent = nullptr);
    void graph_selection(int depth);
    void add_selected();
    // Puts the named nodes in the graph, as -addNode does.
    // `mode` is how much of each shows (1, 2 or 3); 0 leaves it as the editor would have it.
    void add_nodes(const QStringList& names, int mode = 0);
    void clear_graph();
    void refresh();
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void wheelEvent(QWheelEvent*) override;
    void keyPressEvent(QKeyEvent*) override;
private:
    struct Edge { QString src, src_attr, dst, dst_attr; };
    QRectF box_rect(const NodeBox& n) const;
    QStringList shown_attrs(const NodeBox& n) const;
    QPointF port_pos(const NodeBox& n, const QString& attr, bool output) const;
    QPointF to_screen(QPointF p) const { return p * zoom_ + pan_; }
    QPointF to_graph(QPointF p) const { return (p - pan_) / zoom_; }
    void layout_new();
    void load(const QJsonObject& data);
    void new_node_field(QPoint at);

    QVector<NodeBox> nodes_;
    QVector<Edge> edges_;
    QSet<QString> sel_;
    QSet<int> sel_edges_;
    QHash<QString, QPointF> placed_;
    QHash<QString, int> asked_modes_;
    QPointF pan_{20, 20};
    double zoom_ = 1.0;
    enum class Mode { None, Pan, Zoom, Move, Marquee, Wire } mode_ = Mode::None;
    QPointF press_, last_;
    QString wire_node_, wire_attr_;
    bool wire_from_output_ = true;
    QPointF wire_to_;
    int depth_ = 1;
    QStringList roots_;
};

} // namespace az
