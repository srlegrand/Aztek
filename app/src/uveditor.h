// The UV Editor: the selected meshes' UVs in 0-1 space, picked and moved by UV, shell, edge or face, with the reference's
// projection, unfold, layout, cut and sew tools.
#pragma once
#include <QHash>
#include <QImage>
#include <QJsonArray>
#include <array>
#include <QJsonObject>
#include <QPointF>
#include <QPolygonF>
#include <QSet>
#include <QVector>
#include <QWidget>

namespace az {

struct UvMesh {
    QString path;
    QVector<QPointF> maps;
    QVector<int> map_point;
    struct Face {
        int id = 0, shell = 0;
        QVector<int> maps, edges;
    };
    QVector<Face> faces;
    // map a, map b, edge
    QVector<std::array<int, 3>> borders;
    QSet<int> sel_faces, sel_edges, sel_points;
};

class UvCanvas : public QWidget {
    Q_OBJECT
public:
    explicit UvCanvas(QWidget* parent = nullptr);
    void set_data(const QJsonObject& d);
    void set_mode(const QString& m);
    QString mode() const { return mode_; }
    void set_tool(const QString& t);
    QString tool() const { return tool_; }
    void frame(bool selected);
    // The UVs to act on, as polyEditUV arguments (pCube1.map[...]); falls back to the scene selection.
    QJsonArray targets() const;
    bool show_checker = false;
    // The material's image behind the UVs; what Image > Display Image turns off.
    bool show_image = true;
    void set_image(const QString& path, QPointF repeat, QPointF offset);
    bool shaded = true;
signals:
    void picked();
protected:
    void paintEvent(QPaintEvent*) override;
    void mousePressEvent(QMouseEvent*) override;
    void mouseMoveEvent(QMouseEvent*) override;
    void mouseReleaseEvent(QMouseEvent*) override;
    void wheelEvent(QWheelEvent*) override;
    void keyPressEvent(QKeyEvent*) override;
    void contextMenuEvent(QContextMenuEvent*) override;
private:
    QImage image_;
    QString image_path_;
    QPointF image_repeat_{1, 1}, image_offset_{0, 0};
    QPointF to_screen(QPointF uv) const;
    QPointF to_uv(QPointF p) const;
    // Selected maps per mesh, from the local UV selection or the scene's components.
    QHash<int, QSet<int>> active_maps() const;
    QPointF selection_center() const;
    void pick(QPointF pos, QPointF to, bool rect, Qt::KeyboardModifiers mods);
    void tool_press(QMouseEvent* e);
    void pick_lasso(Qt::KeyboardModifiers mods);
    void pick_path(QPointF pos);
    void drag_to(QPointF pos, bool final);
    int handle_at(QPointF pos) const;

    QVector<UvMesh> meshes_;
    QString scene_mode_ = "object";
    QString mode_ = "uv";
    QString tool_ = "move";
    // Local UV selection: mesh index -> maps.
    QHash<QString, QSet<int>> uv_sel_;
    // View: uv at the canvas centre and pixels per uv unit.
    QPointF center_{0.5, 0.5};
    double scale_ = 400;
    bool framed_ = false;
    enum class Mode { None, Pan, Zoom, Marquee, Drag, Lasso } drag_ = Mode::None;
    QPolygonF lasso_;
    // The path tool's last pick: mesh and map.
    QString path_mesh_;
    int path_map_ = -1;
    QPointF press_, last_;
    QPointF pivot_;
    int handle_ = 0;
    bool applied_ = false;
    QJsonArray drag_targets_;
};

class UvEditor : public QWidget {
    Q_OBJECT
public:
    explicit UvEditor(QWidget* parent = nullptr);
    void refresh();
    // The tool, from the texture contexts; every UV Editor follows it.
    static void set_tool_everywhere(const QString& t);
    // What textureWindow's flags ask for.
    void apply_flags(const QJsonObject& props);
private:
    void run(const QString& cmd, const QJsonObject& call = QJsonObject(), bool on_selection = true);
    UvCanvas* canvas_ = nullptr;
    QHash<QString, QAction*> mode_buttons_;
    QHash<QString, QAction*> tool_buttons_;
    void show_tool(const QString& t);
};

} // namespace az
