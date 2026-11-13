// FBX in the interface: the FBX Import and FBX Export options, laid out as the reference's FBX plug-in lays them out.
// Every control sets the plug-in's own setting command (FBXImportSkins, FBXExportUpAxis and the rest), so a script and
// the window see the same settings. What was set is remembered for the next session.
#pragma once
#include <QWidget>
#include <functional>

namespace az {

// The FBX sections of the Import and Export options windows, and what their Reset Settings does. The controls set the
// plug-in's settings as they change, as the reference's do.
struct FbxPanel {
    QWidget* widget;
    std::function<void()> reset;
};
FbxPanel fbx_import_panel(QWidget* parent);
FbxPanel fbx_export_panel(QWidget* parent);
// The settings remembered from earlier sessions, sent to the plug-in. Safe to call often.
void fbx_apply_saved();

} // namespace az
