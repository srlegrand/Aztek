// Render nodes for the interface: what can be created, sorted as the Hypershade's Create panel sorts them, the panel
// itself, and the Create Render Node window a map button opens.
#pragma once
#include <QList>
#include <QString>
#include <QWidget>

class QLineEdit;
class QListWidget;
class QTreeWidget;

namespace az {

struct RenderNodeKind {
    QString label, type;
    // shadingNode's flag: asShader, asTexture, asUtility or asLight.
    QString flag;
};

struct RenderNodeGroup {
    // "Aztek/Surface", "Harold/Shader": the group and the entry under it.
    QString path;
    QList<RenderNodeKind> kinds;
};

// Every creatable render node, by group, in the Create panel's order.
const QList<RenderNodeGroup>& render_node_catalog();
QString render_node_label(const QString& type);
// A small picture for a node type: a ball for shaders, a checker for textures, a bulb for lights, a block for the rest.
QIcon render_node_icon(const QString& type, int size);
// Makes the node, with what the reference makes beside it (a file texture's placement). Returns its name.
QString make_render_node(const RenderNodeKind& kind);
// Create Render Node: choose a node, which is made and connected into the plug. Returns the node, or nothing.
QString create_render_node(QWidget* parent, const QString& plug);

// The Create panel: groups on the left, their nodes on the right, a filter above.
class CreateBar : public QWidget {
    Q_OBJECT
public:
    explicit CreateBar(QWidget* parent = nullptr);
signals:
    void picked(const az::RenderNodeKind& kind);
private:
    void fill();
    QLineEdit* filter_;
    QTreeWidget* groups_;
    QListWidget* nodes_;
};

} // namespace az
