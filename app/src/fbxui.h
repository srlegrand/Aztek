// FBX in the interface: the FBX Import and FBX Export options, laid out as the reference's FBX plug-in lays them out.
// Every control sets the plug-in's own setting command (FBXImportSkins, FBXExportUpAxis and the rest), so a script and
// the window see the same settings. What was set is remembered for the next session.
#pragma once
#include <QWidget>

namespace az {

void fbx_import_options(QWidget* parent);
void fbx_export_options(QWidget* parent);
// The settings remembered from earlier sessions, sent to the plug-in. Safe to call often.
void fbx_apply_saved();

// The option box of File > Import and of the two exports: asks which file type's options to show.
void import_options(QWidget* parent);
void export_options(QWidget* parent);

} // namespace az
