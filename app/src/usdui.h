// USD in the interface: import and export options, the USD Layer Editor, and menus for prims.
#pragma once
#include <QMenu>
#include <QString>
#include <QStringList>
#include <QWidget>
#include <functional>

namespace az {

// The file filter for USD, to add to a file dialog's.
extern const char* const kUsdFilter;
bool is_usd_file(const QString& path);

// Import a USD file with the remembered options.
void usd_import(const QString& path);
// Export to a USD file with the remembered options: the whole scene, or the selection.
void usd_export(const QString& path, bool selection);
// The USD parts of the Import and Export options windows: the widget, what keeps its settings, and what forgets them.
struct OptionsPanel {
    QWidget* widget;
    std::function<void()> save;
    std::function<void()> reset;
};
OptionsPanel usd_import_panel(QWidget* parent);
OptionsPanel usd_export_panel(QWidget* parent);

// Windows > USD Layer Editor.
void show_usd_layer_editor(QWidget* parent);

// Windows > USD Stats.
void show_usd_stats(QWidget* parent);

// Create > USD.
void usd_create_menu(QMenu* m, QWidget* parent);

// What a prim's context menu offers, for names like shape,/path.
void usd_prim_menu(QMenu* m, const QString& prim, QWidget* parent);

// What a node pulled from USD offers: Merge to USD, Discard Edits.
void usd_node_menu(QMenu* m, const QString& node);

// Before a save: asks what to do with unsaved layers. False cancels the save.
bool usd_confirm_dirty_layers(QWidget* parent);

} // namespace az
