// The smaller editor windows of the Windows and Modify menus, and the In-View Editor.
#pragma once
#include <QHash>
#include <QFrame>
#include <QString>

class QVBoxLayout;

class QDoubleSpinBox;
class QCheckBox;

namespace az {

// A window by id: spreadSheet, componentEditor, connectionEditor, hypergraphHierarchy, hypergraphConnections,
// namespaceEditor, channelControl, modelingToolkit, shapeEditor, expressionEditor, polySelectConstraint, alignObjects,
// searchReplace, addAttribute, editAttribute, deleteAttribute, gridOptions. Null for an id it doesn't know.
QWidget* make_editor_window(const QString& id, QWidget* parent);

// Whether the core has a command, for items that stay disabled until it does.
bool has_command_named(const QString& name);

// A small panel over the view with the last history node's attributes, after an edit.
class InViewEditor : public QFrame {
    Q_OBJECT
public:
    explicit InViewEditor(QWidget* parent);
    // The newest history node of the lead selection; hides when there's none.
    void show_last();
    bool show_fields();
    // The numbers again after the scene changed, as a handle drag changes them.
    void update_values();
private:
    QVBoxLayout* body_;
    QString shown_node_;
    QHash<QString, QDoubleSpinBox*> spins_;
    QHash<QString, QCheckBox*> boxes_;
    QWidget* rows_ = nullptr;
};

} // namespace az
