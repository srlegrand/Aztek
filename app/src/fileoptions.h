// The Import, Export All and Export Selection options windows, as the reference's: a File type, the options of that
// type, and the action, Apply and Close buttons. The action and Apply open the file browser on that type and do it.
#pragma once
#include <QString>
#include <QWidget>
#include <functional>

namespace az {

enum class FileAction { Import, ExportAll, ExportSelection };

// The window. `run` imports or exports, through the file browser, as `type` (a File type name).
void file_options_window(QWidget* parent, FileAction action, std::function<void(const QString& type)> run);

// The File type the window was last left on, which the menu item without its option box goes by.
QString chosen_file_type(FileAction action);
// The file dialog filter for a File type, and its extension.
QString file_type_filter(const QString& type);
QString file_type_extension(const QString& type);
// Whether the export adds the type's extension to a name given without one.
bool default_file_extensions();

} // namespace az
