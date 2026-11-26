// Named actions (the reference's runtime commands), the hotkey table that runs them and the Hotkey Editor. A hotkey runs a named
#include <tuple>
// action or MEL; bindings the user changes are saved in the settings over the defaults.
#include "external.h"
#include "themes.h"
#include "envvars.h"
#include "editors.h"
#include "mainwindow.h"
#include "uveditor.h"
#include "grapheditor.h"
#include "dopesheet.h"
#include "aztek.h"
#include "feedback.h"
#include "host.h"
#include "usdui.h"
#include "markingmenu.h"
#include "panels.h"
#include "renderui.h"
#include "scriptui.h"
#include "shelves.h"
#include "toolsettings.h"
#include "viewport.h"
#include "widgets.h"

#include <QApplication>
#include <QDesktopServices>
#include <QUrl>
#include <QFileInfo>
#include <QImage>
#include <QPixmap>
#include <QTimer>
#include <QVBoxLayout>
#include <memory>
#include <QCheckBox>
#include <QComboBox>
#include <QDateTime>
#include <QDialog>
#include <QDockWidget>
#include <QFile>
#include <QMenuBar>
#include <QFileDialog>
#include <QFormLayout>
#include <QHBoxLayout>
#include <QHeaderView>
#include <QJsonArray>
#include <QJsonDocument>
#include <QKeyEvent>
#include <QKeySequence>
#include <QLabel>
#include <QLineEdit>
#include <QMenu>
#include <QDesktopServices>
#include <QUrl>
#include <QCursor>
#include <QMessageBox>
#include <QPushButton>
#include <QSet>
#include <QSettings>
#include <QToolBar>
#include <QStatusBar>
#include <QCoreApplication>
#include <QTimer>
#include <QTreeWidget>
#include <QVBoxLayout>

namespace az {

namespace {

// Keys Shift makes: the character says it all, so the binding carries no Shift.
bool shifted_symbol(int key) {
    switch (key) {
    case Qt::Key_Greater: case Qt::Key_Less: case Qt::Key_Question: case Qt::Key_Colon: case Qt::Key_BraceLeft: case Qt::Key_BraceRight:
    case Qt::Key_Bar: case Qt::Key_Plus: case Qt::Key_Underscore: case Qt::Key_AsciiTilde: case Qt::Key_Exclam: case Qt::Key_At:
    case Qt::Key_NumberSign: case Qt::Key_Dollar: case Qt::Key_Percent: case Qt::Key_AsciiCircum: case Qt::Key_Ampersand:
    case Qt::Key_Asterisk: case Qt::Key_ParenLeft: case Qt::Key_ParenRight: case Qt::Key_QuoteDbl:
        return true;
    default:
        return false;
    }
}

QString key_name(int key) {
    return QKeySequence(key).toString();
}

QString combo(const QString& key, bool ctrl, bool shift, bool alt) {
    QString out;
    if (ctrl) out += "Ctrl+";
    if (alt) out += "Alt+";
    if (shift) out += "Shift+";
    return out + key;
}

} // namespace

void MainWindow::add_runtime(const QString& name, const QString& category, const QString& annotation, std::function<void()> fn) {
    runtime_[name] = RuntimeCommand{name, category, annotation, std::move(fn)};
}

bool MainWindow::run_command(const QString& command) {
    if (command.isEmpty()) return false;
    auto it = runtime_.find(command);
    if (it != runtime_.end()) {
        if (command == "RepeatLast" || command == "Undo" || command == "Redo") {
            it->fn();
            return true;
        }
        // A copy: the command may rebuild this table.
        auto fn = it->fn;
        run_repeatable(command, fn);
        return true;
    }
    run_repeatable(command, [this, command] { mel(command); });
    return true;
}

void MainWindow::view_key(const QString& key, const QString& json) {
    if (ViewportWidget* v = active_view()) v->set(key, json);
}

void MainWindow::apply_snap() {
    // A held key wins; otherwise the first toggle that's on.
    QString m = snap_held_;
    if (m.isEmpty())
        for (const char* s : {"point", "curve", "grid", "viewPlane", "meshCenter", "live"})
            if (snap_on_.contains(s)) {
                m = s;
                break;
            }
    for (ViewportWidget* v : panels_->views()) v->set("snap", "\"" + (m.isEmpty() ? QString("none") : m) + "\"");
}

void MainWindow::build_runtime() {
    auto r = [this](const char* name, const char* cat, const char* ann, std::function<void()> fn) { add_runtime(name, cat, ann, std::move(fn)); };
    auto tool = [this](const char* name, const char* id, const char* ann) {
        QString t = id;
        add_runtime(name, "Tools", ann, [this, t] { set_tool(t); });
    };
    auto m = [this](const char* name, const char* cat, const char* ann, const char* code) {
        QString c = code;
        add_runtime(name, cat, ann, [c] { mel(c); });
    };
    tool("SelectTool", "select", "Select Tool");
    tool("MoveTool", "move", "Move Tool");
    tool("RotateTool", "rotate", "Rotate Tool");
    tool("ScaleTool", "scale", "Scale Tool");
    tool("LassoTool", "lasso", "Lasso Tool");
    tool("PaintSelectTool", "paintSelect", "Paint Selection Tool");
    tool("ShortestEdgePathTool", "shortestPath", "Shortest Edge Path Tool");
    tool("JointTool", "joint", "Joint Tool");
    tool("InsertJointTool", "insertJoint", "Insert Joint Tool");
    tool("IKHandleTool", "ikHandle", "IK Handle Tool");
    tool("IKSplineHandleTool", "ikSplineHandle", "IK Spline Handle Tool");
    tool("SculptMeshActivateBrushSculpt", "sculpt", "Sculpt brush");
    tool("MultiCutTool", "multiCut", "Multi-Cut");
    tool("SplitEdgeRingTool", "insertLoop", "Insert Edge Loop");
    tool("OffsetEdgeLoopTool", "offsetLoop", "Offset Edge Loop");
    tool("TargetWeldTool", "targetWeld", "Target Weld");
    tool("SlideEdgeTool", "slideEdge", "Slide Edge");
    tool("PolyCreaseTool", "crease", "Crease");
    tool("CreatePolygonTool", "createPolygon", "Create Polygon");
    tool("AppendToPolygonTool", "appendPolygon", "Append to Polygon");
    tool("QuadDrawTool", "quadDraw", "Quad Draw");
    tool("SetCutSewUVTool", "cutSewUv", "3D Cut and Sew UV Tool");
    tool("CVCurveTool", "curveCv", "CV Curve Tool");
    tool("EPCurveTool", "curveEp", "EP Curve Tool");
    tool("ArtPaintSkinWeightsTool", "paintSkin", "Paint Skin Weights");
    tool("ArtPaintAttrTool", "paintAttr", "Paint Attribute");
    tool("TumbleTool", "camTumble", "Tumble Tool");
    tool("TrackTool", "camTrack", "Track Tool");
    tool("DollyTool", "camDolly", "Dolly Tool");
    tool("ZoomTool", "camZoom", "Zoom Tool");
    tool("RollTool", "camRoll", "Roll Tool");
    tool("YawPitchTool", "camYawPitch", "Yaw-Pitch Tool");
    r("ArtPaintAttrToolOptions", "Tools", "Paint Attribute and its settings", [this] { set_tool("paintAttr"); show_tool_settings(); });
    r("JointToolOptions", "Skeleton", "Create Joints and its settings", [this] { set_tool("joint"); show_tool_settings(); });
    r("IKHandleToolOptions", "Skeleton", "Create IK Handle and its settings", [this] { set_tool("ikHandle"); show_tool_settings(); });
    r("IKSplineHandleToolOptions", "Skeleton", "Create IK Spline Handle and its settings", [this] { set_tool("ikSplineHandle"); show_tool_settings(); });
    r("OrientJoint", "Skeleton", "Orient Joint", [] { orient_selected_joints(); });
    r("OrientJointOptions", "Skeleton", "Orient Joint options", [] { open_orient_joint_options(); });
    r("EditMembershipTool", "Tools", "Edit Membership", [this] { set_tool("editMembership"); show_tool_settings(); });
    r("ArtPaintSkinWeightsToolOptions", "Tools", "Paint Skin Weights and its settings", [this] { set_tool("paintSkin"); show_tool_settings(); });
    r("AztekThemeEditor", "Window", "Theme Editor: the interface's colours, fonts and shapes", [this] { show_theme_editor(this); });
    r("RepeatLast", "Edit", "Repeat the last command", [this] { repeat_last(); });
    r("CreateReference", "File", "Create a reference", [this] { create_reference_dialog(false); });
    r("CreateReferenceOptions", "File", "Create a reference, with options", [this] { create_reference_dialog(true); });
    r("ReferenceEditor", "Windows", "Reference Editor", [this] { show_reference_editor(); });
    r("LastTool", "Tools", "The tool used before this one", [this] { set_tool(last_tool_); });
    r("ToolSettingsWindow", "Windows", "Tool Settings", [this] { show_tool_settings(); });
    r("ScriptEditor", "Windows", "Script Editor", [this] { show_script_editor(); });
    r("GraphEditor", "Windows", "Graph Editor", [this] { show_graph_editor(); });
    r("UsdLayerEditor", "Windows", "USD Layer Editor", [this] { show_usd_layer_editor(this); });
    r("UsdStats", "Windows", "USD Stats", [this] { show_usd_stats(this); });
    r("AztekFeedback", "Help", "This doesn't feel right!", [this] { open_feedback(this); });
    r("DopeSheetEditor", "Windows", "Dope Sheet", [this] { show_dope_sheet(); });
    r("NodeEditorWindow", "Windows", "Node Editor", [this] { show_node_editor(); });
    r("SetDrivenKeyOptions", "Windows", "Set Driven Key", [this] { show_set_driven_key(); });
    r("UVEditor", "Windows", "UV Editor", [this] { show_uv_editor(); });
    r("HypershadeWindow", "Windows", "Hypershade", [this] { show_hypershade(); });
    r("RenderViewWindow", "Windows", "Harold RenderView", [this] { show_render_view(this); });
    r("RenderGlobalsWindow", "Windows", "Render Settings", [this] { show_render_settings(this); });
    r("RenderIntoNewWindow", "Rendering", "Render the current frame", [this] { render_current_frame(this); });
    r("RenderCurrentFrame", "Rendering", "Render the current frame", [this] { render_current_frame(this); });
    r("IPRRenderIntoNewWindow", "Rendering", "IPR render the current frame", [this] { render_ipr(this); });
    r("RedoPreviousRender", "Rendering", "Redo the previous render", [this] { redo_previous_render(this); });
    r("RenderSequence", "Rendering", "Render a sequence of frames", [this] { render_sequence_dialog(this); });
    r("RenderSequenceOptions", "Rendering", "Render a sequence of frames", [this] { render_sequence_dialog(this); });
    r("TextureViewWindow", "Windows", "UV Editor", [this] { show_uv_editor(); });
    r("UVSetEditor", "Windows", "UV Set Editor", [this] { show_named_window("uvSetEditor"); });
    r("DeletePolyElements", "Polygons", "Delete Edge/Vertex", [] { delete_edge_vertex(); });
    // As the reference's menu items: the emitter, particles for it to emit into, and the two connected (#96). The
    // emitter command alone makes only the emitter.
    m("CreateEmitter", "Dynamics", "Create Emitter",
      "{ string $e[] = `emitter -type omni`; string $p[] = `nParticle`; connectDynamic -em $e[0] $p[0]; select -r $e[0]; }");
    m("EmitFromObject", "Dynamics", "Emit from Object",
      "{ string $s[] = `ls -sl -tr`; if (size($s) == 0) error \"Emit from Object: select the objects to emit from\"; "
      "string $p[] = `nParticle`; string $made[]; for ($o in $s) { string $e[] = `emitter -type surface $o`; "
      "string $one = $e[size($e) - 1]; connectDynamic -em $one $p[0]; $made[size($made)] = $one; } select -r $made; }");
    // The reference's names for windows Aztek has, so scripts and hotkeys that open them work (#88).
    r("ExpressionEditor", "Windows", "Expression Editor", [this] { show_named_window("expressionEditor"); });
    r("ComponentEditor", "Windows", "Component Editor", [this] { show_named_window("componentEditor"); });
    r("BlendShapeEditor", "Windows", "Shape Editor", [this] { show_named_window("shapeEditor"); });
    r("PolygonSelectionConstraints", "Select", "Selection constraints", [this] { show_named_window("polySelectConstraint"); });
    r("ToggleAttributeEditor", "Windows", "Show or hide the Attribute Editor", [this] { attr_dock_->setVisible(!attr_dock_->isVisible()); if (attr_dock_->isVisible()) attr_dock_->raise(); });
    r("ToggleChannelBox", "Windows", "Show or hide the Channel Box", [this] { channel_dock_->setVisible(!channel_dock_->isVisible()); if (channel_dock_->isVisible()) channel_dock_->raise(); });
    r("ToggleToolSettings", "Windows", "Show or hide the Tool Settings", [this] { tool_dock_->setVisible(!tool_dock_->isVisible()); });
    r("ToggleOutliner", "Windows", "Show or hide the Outliner", [this] { outliner_dock_->setVisible(!outliner_dock_->isVisible()); if (outliner_dock_->isVisible()) outliner_dock_->raise(); });
    r("ToggleModelingToolkit", "Windows", "Show or hide the Modeling Toolkit", [this] { if (mtk_dock_ && mtk_dock_->isVisible()) mtk_dock_->hide(); else show_modeling_toolkit(); });
    r("PreferencesWindow", "Windows", "Preferences", [this] { show_preferences(); });
    r("HotkeyPreferencesWindow", "Windows", "Hotkey Editor", [this] { show_hotkey_editor(); });
    r("PluginManager", "Windows", "Plug-in Manager", [this] { show_plugin_manager(); });
    r("AttributeEditor", "Windows", "Attribute Editor", [this] { show_attribute_editor(); });
    auto window = [this](const char* name, const char* id, const char* ann) {
        QString w = id;
        add_runtime(name, "Windows", ann, [this, w] { show_named_window(w); });
    };
    window("SpreadSheetEditor", "spreadSheet", "Attribute Spread Sheet");
    window("ComponentEditorWindow", "componentEditor", "Component Editor");
    window("ConnectionEditor", "connectionEditor", "Connection Editor");
    window("HypergraphHierarchyWindow", "hypergraphHierarchy", "Hypergraph: Hierarchy");
    window("HypergraphDGWindow", "hypergraphConnections", "Hypergraph: Connections");
    window("NamespaceEditor", "namespaceEditor", "Namespace Editor");
    window("ChannelControlEditor", "channelControl", "Channel Control");
    window("ShapeEditor", "shapeEditor", "Shape Editor");
    window("HumanIK", "humanIK", "Humanoid IK");
    window("SkinToolsWindow", "skinTools", "Skin Tools");
    window("HIKCharacterControlsTool", "humanIK", "Humanoid IK");
    window("ModelingToolkitWindow", "modelingToolkit", "Modeling Toolkit");
    window("GridOptions", "gridOptions", "Grid Options");
    window("AddAttribute", "addAttribute", "Add Attribute");
    window("EditAttribute", "editAttribute", "Edit Attribute");
    window("DeleteAttribute", "deleteAttribute", "Delete Attribute");
    r("ShelfEditor", "Windows", "Shelf Editor", [this] { shelf_bar_->open_editor(); });
    r("OutlinerWindow", "Windows", "Outliner", [this] { outliner_dock_->show(); outliner_dock_->raise(); });
    r("ToggleChannelsLayers", "Windows", "Channel Box / Layer Editor", [this] { channel_dock_->setVisible(!channel_dock_->isVisible()); });
    r("FourViewLayout", "Panels", "Four views", [this] { panels_->set_single(false); });
    r("SingleViewLayout", "Panels", "One view", [this] { panels_->set_single(true); });
    r("FrameSelected", "View", "Frame the selection", [this] { view_key("frame", "\"selection\""); });
    r("FrameAll", "View", "Frame everything", [this] { view_key("frame", "\"all\""); });
    r("FrameSelectedInAllViews", "View", "Frame the selection in every view", [this] { panels_->set_all("frame", "\"selection\""); });
    r("FrameAllInAllViews", "View", "Frame everything in every view", [this] { panels_->set_all("frame", "\"all\""); });
    r("PreviousView", "View", "Back through the camera moves", [this] { view_key("viewUndo", "true"); });
    r("NextView", "View", "Forward through the camera moves", [this] { view_key("viewRedo", "true"); });
    r("ToggleMainPane", "Panels", "Main pane only", [this] { toggle_main_pane(); });
    r("IsolateSelect", "View", "Isolate Select in the view under the cursor", [this] { view_key("isolate", "{}"); });
    r("XRayToggle", "View", "X-Ray", [this] { toggle_view_flag("xray"); });
    r("BackgroundCycle", "View", "Cycle the view's background", [this] {
        // Gradient, black, dark grey, light grey, and round again.
        QSettings s;
        double bg = s.value("prefs/background", 0.36).toDouble();
        bool gradient = s.value("prefs/backgroundGradient", false).toBool();
        if (gradient) { gradient = false; bg = 0.0; }
        else if (bg < 0.2) bg = 0.36;
        else if (bg < 0.5) bg = 0.8;
        else { gradient = true; bg = 0.36; }
        s.setValue("prefs/background", bg);
        s.setValue("prefs/backgroundGradient", gradient);
        // Only the background changes (#123): the rest of the preferences, the views' cameras among what they
        // touch, are left as they are.
        for (ViewportWidget* v : panels_->views()) v->set("background", QString::number(bg));
        exec_quiet("displayPref", QJsonObject{{"displayGradient", gradient}});
    });
    r("WireframeView", "View", "Wireframe", [this] { view_key("wireframe", "true"); });
    r("ShadedView", "View", "Smooth shade all", [this] { view_key("wireframe", "false"); view_key("disp", "{\"textured\":false,\"displayLights\":\"default\"}"); });
    r("TexturedView", "View", "Hardware texturing", [this] { view_key("wireframe", "false"); view_key("disp", "{\"textured\":true,\"displayLights\":\"default\"}"); });
    r("DisplayLight", "View", "Use all lights", [this] { view_key("wireframe", "false"); view_key("disp", "{\"displayLights\":\"all\"}"); });
    r("PlaybackToggle", "Animation", "Play or stop", [this] { toggle_play(1); });
    r("GoToMinFrame", "Animation", "Go to the start of the playback range", [this] { go_to_time(time_slider_->start()); });
    r("NextFrame", "Animation", "Next frame", [this] { step_time(1); });
    r("PreviousFrame", "Animation", "Previous frame", [this] { step_time(-1); });
    r("NextKey", "Animation", "Next key", [this] { step_key("next"); });
    r("PreviousKey", "Animation", "Previous key", [this] { step_key("previous"); });
    r("SetKey", "Animation", "Set a key", [] { exec("setKeyframe"); });
    r("SetKeyTranslate", "Animation", "Key translate", [] { exec("setKeyframe", QJsonObject{{"attribute", "translate"}}); });
    r("SetKeyRotate", "Animation", "Key rotate", [] { exec("setKeyframe", QJsonObject{{"attribute", "rotate"}}); });
    r("SetKeyScale", "Animation", "Key scale", [] { exec("setKeyframe", QJsonObject{{"attribute", "scale"}}); });
    r("Undo", "Edit", "Undo", [] { aztek_undo(); Host::get()->poke(); });
    r("Redo", "Edit", "Redo", [] { aztek_redo(); Host::get()->poke(); });
    r("Group", "Edit", "Group", [] { exec("group"); });
    r("Duplicate", "Edit", "Duplicate", [] { exec("duplicate"); });
    r("DuplicateSpecialOptions", "Edit", "Duplicate Special", [] { OptionsDialog::open("duplicate", "Duplicate Special"); });
    r("DuplicateWithTransform", "Edit", "Duplicate with Transform", [] { exec("duplicateWithTransform"); });
    r("DeleteHistory", "Edit", "Delete history", [] { exec("delete", QJsonObject{{"constructionHistory", true}}); });
    // Edit > Delete by Type and Delete All by Type > Non-Deformer History, through the reference's script procedure so
    // scripts that call either get the same (#172). The option box's choice: all of it, or what's before the deformers.
    auto bake_how = [] { return OptionsDialog::saved("bakePartialHistory").value("prePostDeformers").toBool(true) ? QString("prePost") : QString("pre"); };
    r("BakeNonDefHistory", "Edit", "Delete non-deformer history", [bake_how] { mel(QString("doBakeNonDefHistory(1, {\"%1\"});").arg(bake_how())); });
    r("BakeAllNonDefHistory", "Edit", "Delete all non-deformer history", [bake_how] {
        mel(QString("select -r `ls -type mesh -noIntermediate`; doBakeNonDefHistory(1, {\"%1\"});").arg(bake_how()));
    });
    r("BakeNonDefHistoryOptions", "Edit", "Delete non-deformer history options", [] { OptionsDialog::open("bakePartialHistory", "Delete Non Deformer History"); });
    r("Parent", "Edit", "Parent", [] { exec("parent"); });
    r("Unparent", "Edit", "Unparent", [] { exec("parent", QJsonObject{{"world", true}}); });
    r("HideSelected", "Display", "Hide the selection", [] { exec("hide"); });
    r("ShowLastHidden", "Display", "Show what was last hidden", [] { exec("showLastHidden"); });
    r("ToggleVisibility", "Display", "Show or hide", [] { exec("toggleVisibility"); });
    r("ShowSelectedObjects", "Display", "Show the selection", [] {
        const QJsonObject st = state();
        const QJsonArray sel = st["selection"].toArray();
        // In a component mode: the selected components, or with none selected every hidden one on the objects.
        if (st["mode"].toString() != "object") exec("showHidden");
        else if (!sel.isEmpty()) exec("showHidden", QJsonObject{{"args", sel}});
    });
    r("ShowAllComponents", "Display", "Show every hidden component of the selected objects", [] {
        QJsonArray all;
        QSet<QString> seen;
        for (const QJsonValue& v : state()["selection"].toArray()) {
            const QString n = v.toString();
            const int dot = n.indexOf('[') >= 0 ? n.lastIndexOf('.', n.indexOf('[')) : -1;
            const QString obj = dot > 0 ? n.left(dot) : n;
            if (!seen.contains(obj)) { seen.insert(obj); all.append(obj + ".f[*]"); }
        }
        if (!all.isEmpty()) exec("showHidden", QJsonObject{{"args", all}});
    });
    r("NewScene", "File", "New scene", [this] { new_scene(); });
    r("OpenScene", "File", "Open a scene", [this] { open_scene(); });
    r("SaveScene", "File", "Save the scene", [this] { save_scene(false); });
    r("SaveSceneAs", "File", "Save the scene under a new name", [this] { save_scene(true); });
    r("IncrementAndSave", "File", "Save as the next version of the file", [this] { increment_save(); });
    r("SelectAll", "Select", "Select everything", [] {
        QJsonArray all;
        for (const QJsonValue& n : query("outliner").toArray())
            if (n.toObject()["type"].toString() != "objectSet" && n.toObject()["icon"].toString() != "camera") all.append(n.toObject()["path"].toString());
        exec("select", QJsonObject{{"args", all}});
    });
    r("InvertSelection", "Select", "Invert the selection", [] { exec("invertSelection"); });
    r("GrowSelection", "Select", "Grow", [] { exec("growSelection"); });
    r("ShrinkSelection", "Select", "Shrink", [] { exec("shrinkSelection"); });
    m("PickWalkUp", "Select", "Walk up the hierarchy", "pickWalk -direction up;");
    m("PickWalkDown", "Select", "Walk down the hierarchy", "pickWalk -direction down;");
    m("PickWalkLeft", "Select", "Walk to the previous sibling", "pickWalk -direction left;");
    m("PickWalkRight", "Select", "Walk to the next sibling", "pickWalk -direction right;");
    r("NudgeLeft", "Modify", "Move the selection a pixel left", [this] { nudge(-1, 0); });
    r("NudgeRight", "Modify", "Move the selection a pixel right", [this] { nudge(1, 0); });
    r("NudgeUp", "Modify", "Move the selection a pixel up", [this] { nudge(0, 1); });
    r("NudgeDown", "Modify", "Move the selection a pixel down", [this] { nudge(0, -1); });
    r("ToggleSelectMode", "Select", "Object or component mode", [this] { set_mode(state()["mode"].toString() == "object" ? "vertex" : "object"); });
    r("SelectVertexMask", "Select", "Vertices", [this] { set_mode("vertex"); });
    r("SelectEdgeMask", "Select", "Edges", [this] { set_mode("edge"); });
    r("SelectFacetMask", "Select", "Faces", [this] { set_mode("face"); });
    r("ConvertSelectionToVertices", "Select", "To vertices", [] { exec("convertSelection", QJsonObject{{"args", QJsonArray{"vertex"}}}); });
    r("ConvertSelectionToEdges", "Select", "To edges", [] { exec("convertSelection", QJsonObject{{"args", QJsonArray{"edge"}}}); });
    r("ConvertSelectionToFaces", "Select", "To faces", [] { exec("convertSelection", QJsonObject{{"args", QJsonArray{"face"}}}); });
    r("Extrude", "Mesh", "Extrude", [] { exec("polyExtrude", OptionsDialog::saved("polyExtrude")); });
    r("Bevel", "Mesh", "Bevel", [] { exec("polyBevel", OptionsDialog::saved("polyBevel")); });
    r("DeleteSelection", "Edit", "Delete", [this] {
        if (tool_.startsWith("curve") || tool_ == "multiCut" || tool_ == "createPolygon" || tool_ == "appendPolygon" || tool_ == "quadDraw") {
            for (ViewportWidget* v : panels_->views()) v->set("undoToolPoint", "true");
            return;
        }
        if (state()["mode"].toString() == "object") exec("delete");
        else exec("deleteComponents");
    });
    r("DeleteEdgeVertex", "Mesh", "Delete edges or vertices", [] { delete_edge_vertex(); });
    r("FinishTool", "Tools", "Finish the current tool", [this] {
        // The joint tool ends its chain once, in one view: an IK handle down it when the tool makes one, else the
        // selection cleared. Then it quits to the Select tool, as the reference's does (#106).
        if (tool_ == "joint") {
            if (!panels_->views().isEmpty()) panels_->views().first()->set("finishTool", "true");
            set_tool("select");
            return;
        }
        for (ViewportWidget* v : panels_->views()) v->set("finishTool", "true");
        // A scripted tool finishes at its next idle.
        exec_quiet("ctxCompletion");
        Host::get()->poke();
    });
    r("CancelPlayback", "Animation", "Stop playback", [this] { stop_play(); });
    auto smoothness = [](int div, int wire, int shaded) {
        exec_quiet("displaySmoothness", QJsonObject{{"divisionsU", div}, {"divisionsV", div}, {"pointsWire", wire}, {"pointsShaded", shaded}});
    };
    r("SmoothPreviewOff", "Display", "Base mesh", [smoothness] {
        exec("smoothPreview", QJsonObject{{"level", 0}});
        smoothness(0, 4, 1);
    });
    r("SmoothPreviewCage", "Display", "Cage and smooth mesh", [this, smoothness] {
        for (ViewportWidget* v : panels_->views()) v->set("smoothCage", "true");
        exec("smoothPreview", QJsonObject{{"level", 2}});
        smoothness(1, 8, 2);
    });
    r("SmoothPreviewSmooth", "Display", "Smooth mesh", [this, smoothness] {
        for (ViewportWidget* v : panels_->views()) v->set("smoothCage", "false");
        exec("smoothPreview", QJsonObject{{"level", 2}});
        smoothness(3, 16, 4);
    });
    r("ManipBigger", "Display", "Bigger manipulator", [this] { scale_manip(0.25); });
    r("ManipSmaller", "Display", "Smaller manipulator", [this] { scale_manip(-0.25); });
    r("MenuSetAnimation", "Menus", "Animation menus", [this] { set_menu_set("Animation"); });
    r("MenuSetModeling", "Menus", "Modeling menus", [this] { set_menu_set("Modeling"); });
    r("MenuSetRigging", "Menus", "Rigging menus", [this] { set_menu_set("Rigging"); });
    r("MenuSetRendering", "Menus", "Rendering menus", [this] { set_menu_set("Rendering"); });
    r("InteractiveGroomEditor", "Windows", "Interactive Groom Editor", [this] { show_named_window("interactiveGroomEditor"); });
    r("InteractiveGroomTool", "Tools", "The Interactive Groom tools, with the tool xjenGroomCtx names", [this] { set_tool("groom"); });
    r("MenuSetDynamics", "Menus", "Dynamics menus", [this] { set_menu_set("FX"); });
    // H as the reference's: a tap shows or hides the selection (Display > Toggle Show/Hide), held with a left click in a
    // view it's the menu sets (#175).
    r("MenuSetHoldPress", "Menus", "H: hold with a left click in a view for the menu sets", [this] {
        view_hold_ = "menuSet";
        hold_used_ = false;
    });
    r("MenuSetHoldRelease", "Display", "H: a tap shows or hides the selection, held with a click the menu sets", [this] {
        const bool used = hold_used_;
        view_hold_.clear();
        hold_used_ = false;
        if (!used) exec("toggleVisibility", QJsonObject{});
    });
    // The menu sets at the pointer, as the reference's H marking menu offers them.
    r("MenuSetMenu", "Menus", "Choose a menu set at the pointer", [this] {
        QMenu menu(this);
        for (const QString& t : menu_set_names()) {
            QAction* a = menu.addAction(t, [this, t] { set_menu_set(t); });
            a->setCheckable(true);
            a->setChecked(t == menu_set_);
        }
        menu.exec(QCursor::pos());
    });
    r("ToggleMainMenubar", "Menus", "Main menu bar shown or not", [this] { menuBar()->setVisible(!menuBar()->isVisible()); });
    r("SelectUVMask", "Select", "UVs", [this] { set_mode("uv"); });
    r("SelectVertexFaceMask", "Select", "Vertex faces", [this] { set_mode("vertexFace"); });
    r("ConvertSelectionToUVs", "Select", "To UVs", [] { exec("convertSelection", QJsonObject{{"args", QJsonArray{"uv"}}}); });
    r("Help", "Help", "Aztek's documentation", [] { QDesktopServices::openUrl(QUrl("https://github.com/srlegrand/Aztek/tree/main/docs")); });
    // Plays every frame, looping, so the solver sees each one while things are moved in the view.
    r("InteractivePlayback", "Dynamics", "Play every frame and keep solving while objects are moved", [this] {
        exec("playbackOptions", QJsonObject{{"playbackSpeed", 0}, {"loop", "continuous"}});
        toggle_play(1);
    });
    r("TogglePivotMode", "Tools", "Move the pivot rather than the object", [this] { set_pivot_mode(!pivot_mode_); });
    r("ShowManipulators", "Tools", "The history node's handles", [this] { show_history_manips(); });

    // Held keys: a press and a release each.
    auto hold_snap = [this](const char* press, const char* release, const char* mode, const char* help) {
        QString md = mode, h = help;
        add_runtime(press, "Snapping", h, [this, md, h] { snap_held_ = md; apply_snap(); help_line_->setText(h); });
        add_runtime(release, "Snapping", h, [this] { snap_held_.clear(); apply_snap(); });
    };
    hold_snap("SnapToGridPress", "SnapToGridRelease", "grid", "Snap to grids");
    hold_snap("SnapToCurvePress", "SnapToCurveRelease", "curve", "Snap to curves");
    hold_snap("SnapToPointPress", "SnapToPointRelease", "point", "Snap to points");
    r("PivotHoldPress", "Tools", "Edit Pivot: a tap turns it on or off, a hold edits the pivot while held", [this] {
        pivot_hold_ms_ = QDateTime::currentMSecsSinceEpoch();
        for (ViewportWidget* v : panels_->views()) v->set("pivotMode", "true");
        help_line_->setText("Pivot: drag the manipulator to move the pivot");
    });
    r("PivotHoldRelease", "Tools", "Edit Pivot: a tap turns it on or off, a hold edits the pivot while held", [this] {
        // A tap is the toggle, as the reference's D and Insert are; held through a drag, the mode it was in comes back.
        const bool tap = QDateTime::currentMSecsSinceEpoch() - pivot_hold_ms_ < 300;
        set_pivot_mode(tap ? !pivot_mode_ : pivot_mode_);
    });
    r("DiscreteRotatePress", "Tools", "Rotate in steps while held", [this] {
        for (ViewportWidget* v : panels_->views()) v->set("discreteHold", "true");
    });
    r("DiscreteRotateRelease", "Tools", "Rotate in steps while held", [this] {
        for (ViewportWidget* v : panels_->views()) v->set("discreteHold", "false");
    });
    r("DragSelectPress", "Select", "Drag over components to select them while held", [this] {
        for (ViewportWidget* v : panels_->views()) {
            v->set("toolTab", "true");
            v->set("tabHeld", "true");
        }
    });
    r("DragSelectRelease", "Select", "Drag over components to select them while held", [this] {
        for (ViewportWidget* v : panels_->views()) {
            v->set("toolTab", "false");
            v->set("tabHeld", "false");
        }
    });
    r("SoftSelectPress", "Select", "B: a tap toggles soft select, a drag sizes its falloff", [this] {
        for (ViewportWidget* v : panels_->views()) v->set("brushAdjust", "true");
    });
    r("SoftSelectRelease", "Select", "B: a tap toggles soft select, a drag sizes its falloff", [this] {
        bool dragged = false;
        for (ViewportWidget* v : panels_->views()) {
            dragged = dragged || v->get("brushDragged") == "true";
            v->set("brushAdjust", "false");
        }
        if (dragged || tool_ == "groom") return;
        bool on = ask("softSelect", QJsonObject{{"query", true}, {"queried", QJsonArray{"softSelectEnabled"}}}).toInt() != 0;
        exec("softSelect", QJsonObject{{"softSelectEnabled", on ? 0 : 1}});
        for (ViewportWidget* v : panels_->views()) v->update();
        help_line_->setText(on ? "Soft Select off" : "Soft Select on: hold B and drag to size the falloff");
    });
    r("HotBoxPress", "Menus", "Hold for the hotbox, tap for one or four views", [this] {
        space_held_ = true;
        // At once, as the reference's: a quick tap with nothing picked still toggles the view.
        if (hotbox_ && !hotbox_->isVisible()) show_hotbox(QCursor::pos(), false);
    });
    r("HotBoxRelease", "Menus", "Hold for the hotbox, tap for one or four views", [this] {
        space_held_ = false;
        QWidget* popup = QApplication::activePopupWidget();
        // finish() reports the release to hotBox and toggles the view on a tap; one popped with -noKeyPress stays up.
        if (hotbox_ && hotbox_->isVisible() && !hotbox_->holding() && !(popup && popup->inherits("QMenu"))) hotbox_->finish();
    });
    // Holding a tool key with a left press in a view opens that tool's marking menu; K and I work a drag in the view.
    auto held_key = [this](const char* name, const char* id) {
        QString n = name, t = id;
        add_runtime(n + "Press", "Tools", n, [this, t] {
            if (t == "select" || t == "move" || t == "rotate" || t == "scale") set_tool(t);
            view_hold_ = t;
        });
        add_runtime(n + "Release", "Tools", n, [this] { view_hold_.clear(); });
    };
    held_key("SelectToolKey", "select");
    held_key("MoveToolKey", "move");
    held_key("RotateToolKey", "rotate");
    held_key("ScaleToolKey", "scale");
    held_key("ScrubTimeKey", "scrub");
    held_key("NudgeKeysKey", "nudgeKeys");

    // Names the reference's hotkey files, shelves and marking menus use (PLAN 2.1), each doing what its menu item does.
    auto add_if = [this](const char* name, const char* cat, const char* ann, const QString& code) {
        if (runtime_.contains(name)) return;
        QString c = code;
        add_runtime(name, cat, ann, [c] { mel(c); });
    };
    add_if("GrowPolygonSelectionRegion", "Select", "Grow Selection", "growSelection;");
    add_if("ShrinkPolygonSelectionRegion", "Select", "Shrink Selection", "shrinkSelection;");
    add_if("FreezeTransformations", "Modify", "Freeze Transformations", "makeIdentity -apply true -t 1 -r 1 -s 1;");
    add_if("ResetTransformations", "Modify", "Reset Transformations",
           "string $rt[] = `ls -sl -type transform`; for ($o in $rt) { setAttr ($o + \".t\") 0 0 0; setAttr ($o + \".r\") 0 0 0; setAttr ($o + \".s\") 1 1 1; }");
    add_if("SelectHierarchy", "Select", "Select Hierarchy", "string $hs[] = `ls -sl`; select -hi $hs;");
    add_if("SelectAllPolygonGeometry", "Select", "Select All Polygon Geometry",
           "string $pm[] = `ls -type mesh -ni`; if (size($pm)) { string $pp[] = `listRelatives -p -f $pm`; select -r $pp; }");
    add_if("HideSelectedObjects", "Display", "Hide Selection", "string $hd[] = `ls -sl`; if (size($hd)) hide $hd;");
    add_if("ShowAll", "Display", "Show All", "showHidden -all;");
    add_if("ToggleIsolateSelect", "Display", "Isolate Select",
           "string $ip = `getPanel -wf`; isolateSelect -state (!`isolateSelect -q -state $ip`) $ip;");
    add_if("SnapToGrid", "Modify", "Snap to grids on or off", "snapMode -grid (!`snapMode -q -grid`);");
    add_if("MatchPivots", "Modify", "Match Pivots", "matchTransform -pivots;");
    add_if("BakeCustomPivot", "Modify", "Bake Pivot", "bakeCustomToolPivot;");
    add_if("PolyExtrude", "Mesh", "Extrude", "polyExtrudeFacet;");
    add_if("BevelPolygon", "Mesh", "Bevel", "polyBevel3;");
    add_if("ReversePolygonNormals", "Mesh Display", "Reverse normals", "polyNormal -normalMode 0;");
    add_if("ToggleBackfaceCulling", "Display", "Backface culling on or off",
           "string $bp = `getPanel -wf`; modelEditor -e -backfaceCulling (!`modelEditor -q -backfaceCulling $bp`) $bp;");
    add_if("ToggleWireframeOnShaded", "Display", "Wireframe on Shaded on or off",
           "string $wp = `getPanel -wf`; modelEditor -e -wireframeOnShaded (!`modelEditor -q -wireframeOnShaded $wp`) $wp;");
    add_if("DisplayShadedAndTextured", "Display", "Shaded and textured",
           "string $dp = `getPanel -wf`; modelEditor -e -displayAppearance \"smoothShaded\" -displayTextures true $dp;");
    add_if("ToggleAutoKey", "Animation", "Auto Key on or off", "autoKeyframe -state (!`autoKeyframe -q -state`);");
    add_if("Ungroup", "Edit", "Ungroup", "ungroup;");
    add_if("Hypershade", "Windows", "Hypershade", "HypershadeWindow;");
    add_if("CreateNURBSCircle", "Create", "NURBS Circle", "circle -nr 0 1 0;");
    add_if("CreateImagePlane", "Create", "Free Image Plane", "imagePlane;");
    // Cut, Copy and Paste of objects: the selection written to a scene file in the user's temp folder and imported
    // back with the pasted_ prefix, so pasted__pCube1, as the reference names them; a paste survives the originals
    // being deleted and works across sessions.
    add_if("CopySelected", "Edit", "Copy",
           "if (size(`ls -sl`)) { string $f = `internalVar -userTmpDir` + \"aztekClipboard.ma\";"
           " file -force -exportSelected -type \"mayaAscii\" $f; }");
    add_if("CutSelected", "Edit", "Cut",
           "if (size(`ls -sl`)) { string $f = `internalVar -userTmpDir` + \"aztekClipboard.ma\";"
           " file -force -exportSelected -type \"mayaAscii\" $f; delete; }");
    add_if("PasteSelected", "Edit", "Paste",
           "string $f = `internalVar -userTmpDir` + \"aztekClipboard.ma\";"
           " if (`filetest -r $f`) { string $new[] = `file -import -type \"mayaAscii\" -renameAll true"
           " -renamingPrefix \"pasted_\" -returnNewNodes $f`; string $top[] = `ls -assemblies $new`;"
           " if (size($top)) select -r $top; }");
    // UV > Split: each face round the selected UVs (or the UVs at the selected vertices) gets its own.
    add_if("SplitUV", "Polygon UVs", "Split UVs", "polyForceUV -unshare `polyListComponentConversion -tuv`;");
    // Deform > Edit > Wrap and Wire: the deformed object first, the influence or curve last.
    auto on_deformer = [](const char* type, const char* what, const char* edit) {
        return QString("{ string $s[] = `ls -sl -o`; if (size($s) < 2) error \"Select the deformed object, then the %2\";"
                       " string $d[] = `ls -type %1 (listHistory($s[0]))`; if (!size($d)) error (\"No %1 on \" + $s[0]);"
                       " %1 -e %3 $s[size($s) - 1] $d[0]; }")
            .arg(type, what, edit);
    };
    add_if("AddWrapInfluence", "Deform", "Add the last selected mesh as an influence of the first one's wrap", on_deformer("wrap", "influence", "-inf"));
    add_if("RemoveWrapInfluence", "Deform", "Remove the last selected influence from the first one's wrap", on_deformer("wrap", "influence", "-ri"));
    // Deform > Edit > Blend Shape > Add and Remove: the targets first, the base last.
    const QString blend_of = "string $s[] = `ls -sl -o`; if (size($s) < 2) error \"Select the targets, then the base\"; string $base = $s[size($s) - 1];"
                             " string $b[] = `ls -type blendShape (listHistory($base))`; if (!size($b)) error (\"No blend shape on \" + $base);";
    add_if("AddBlendShapeTarget", "Deform", "Add the selected meshes as targets of the last one's blend shape",
           "{ " + blend_of + " int $n = `blendShape -q -wc $b[0]`; for ($i = 0; $i < size($s) - 1; $i++) blendShape -e -t $base ($n + $i) $s[$i] 1.0 $b[0]; }");
    add_if("RemoveBlendShapeTarget", "Deform", "Remove the selected targets from the last one's blend shape",
           "{ " + blend_of + " for ($i = 0; $i < size($s) - 1; $i++) blendShape -e -rmt $s[$i] $b[0]; }");
    add_if("AddWire", "Deform", "Add the last selected curve to the first one's wire", on_deformer("wire", "curve", "-w"));
    // Deform > Membership > Prune Membership: each selected object's deformer of that kind loses the points it doesn't move.
    for (auto [rt, type, cmd] : {std::tuple{"PruneCluster", "cluster", "cluster"}, {"PruneLattice", "ffd", "lattice"},
                                 {"PruneSculpt", "sculpt", "sculpt"}, {"PruneWire", "wire", "wire"}}) {
        add_if(rt, "Deform", "Prune Membership",
               QString("{ string $d[] = `ls -type %1 (listHistory(`ls -sl -o`))`; if (!size($d)) error \"Select an object with a %1\";"
                       " for ($x in $d) %2 -e -prune true $x; }")
                   .arg(type, cmd));
    }
    // Key > IK/FK Keys, on the selected IK handles.
    const QString handles = "string $h[] = `ls -sl -type ikHandle`; if (!size($h)) error \"Select an IK handle\";";
    add_if("SetIKFKKeyframe", "Animation", "Set IK/FK Key", "{ " + handles + " for ($x in $h) { setKeyframe $x; setKeyframe -at ikBlend $x; setKeyframe -at rotate `ikHandle -q -jl $x`; } }");
    add_if("EnableIKSolvers", "Animation", "Enable IK Solver",
           "{ " + handles + " for ($x in $h) setAttr ($x + \".solverEnable\") (!`getAttr ($x + \".solverEnable\")`); }");
    add_if("MoveIKtoFK", "Animation", "Move IK to FK", "{ " + handles + " for ($x in $h) ikHandle -e -see $x; }");
    add_if("ToggleNurbsInView", "Display", "NURBS shown or not in the view under the pointer",
           "string $p = `getPanel -wf`; if (`getPanel -to $p` == \"modelPanel\") { int $on = !`modelEditor -q -nurbsSurfaces $p`; modelEditor -e -nurbsSurfaces $on -nurbsCurves $on $p; }");
    add_if("TogglePolygonsInView", "Display", "Polygons shown or not in the view under the pointer",
           "string $p = `getPanel -wf`; if (`getPanel -to $p` == \"modelPanel\") { int $on = !`modelEditor -q -polymeshes $p`; modelEditor -e -polymeshes $on $p; }");
    add_if("ToggleImagePlanesInView", "Display", "Image planes shown or not in the view under the pointer",
           "string $p = `getPanel -wf`; if (`getPanel -to $p` == \"modelPanel\") { int $on = !`modelEditor -q -imagePlane $p`; modelEditor -e -imagePlane $on $p; }");
    add_if("OptimizeScene", "File", "Optimize Scene Size", "cleanUpScene;");
    add_if("ArchiveScene", "File", "Archive Scene", "archiveScene;");
    r("TogglePolyDisplayHardEdges", "Display", "Hard edges shown or not", [this] {
        hard_edges_shown_ = !hard_edges_shown_;
        for (ViewportWidget* v : panels_->views()) v->set("disp", QString("{\"hardEdges\":%1}").arg(hard_edges_shown_ ? "true" : "false"));
    });
    add_if("HideUnselectedObjects", "Display", "Hide Unselected Objects",
           "string $sel[] = `ls -sl -l`; string $all[] = `ls -assemblies -l`;"
           "for ($a in $all) { if (size(`listRelatives -s -type camera $a`)) continue; int $keep = 0;"
           " for ($x in $sel) { if ($x == $a || gmatch($x, $a + \"|*\")) $keep = 1; }"
           " if (!$keep) hide $a; }");
    add_if("SmoothingLevelIncrease", "Display", "One more smooth preview level",
           "string $m[] = `ls -sl -dag -type mesh -l`; for ($x in $m) { int $l = `getAttr ($x + \".smoothLevel\")`;"
           " if ($l < 4) setAttr ($x + \".smoothLevel\") ($l + 1); }");
    add_if("SmoothingLevelDecrease", "Display", "One fewer smooth preview level",
           "string $m[] = `ls -sl -dag -type mesh -l`; for ($x in $m) { int $l = `getAttr ($x + \".smoothLevel\")`;"
           " if ($l > 0) setAttr ($x + \".smoothLevel\") ($l - 1); }");
    if (!runtime_.contains("DuplicateSpecial")) r("DuplicateSpecial", "Edit", "Duplicate Special", [] { exec("duplicate", OptionsDialog::saved("duplicate")); });
    if (!runtime_.contains("InsertEdgeLoopTool")) tool("InsertEdgeLoopTool", "insertLoop", "Insert Edge Loop Tool");
    if (!runtime_.contains("MergeVertexTool")) tool("MergeVertexTool", "targetWeld", "Merge Vertex Tool");
    if (!runtime_.contains("PlayblastOptions")) r("PlayblastOptions", "Windows", "Playblast options", [] { OptionsDialog::open("playblast", "Playblast"); });

    // Scripts can call all of these by name, as the reference's runtime commands.
    QJsonArray names;
    for (auto it = runtime_.begin(); it != runtime_.end(); ++it) names.append(QJsonArray{it.key(), it->annotation, it->category});
    query("registerUiNames", QString::fromUtf8(QJsonDocument(names).toJson(QJsonDocument::Compact)));
}

// ---------------------------------------------------------------------------------------------------------------------
// The table

QList<HotkeyBinding> MainWindow::default_hotkeys() const {
    struct D { const char* key; int mods; const char* press; const char* release; };
    enum { C = 1, S = 2, A = 4 };
    static const D d[] = {
        {"Space", 0, "HotBoxPress", "HotBoxRelease"}, {"Q", 0, "SelectToolKeyPress", "SelectToolKeyRelease"}, {"W", 0, "MoveToolKeyPress", "MoveToolKeyRelease"},
        {"E", 0, "RotateToolKeyPress", "RotateToolKeyRelease"}, {"R", 0, "ScaleToolKeyPress", "ScaleToolKeyRelease"}, {"Y", 0, "LastTool", ""},
        {"T", 0, "ShowManipulators", ""}, {"X", 0, "SnapToGridPress", "SnapToGridRelease"}, {"C", 0, "SnapToCurvePress", "SnapToCurveRelease"},
        {"V", 0, "SnapToPointPress", "SnapToPointRelease"}, {"D", 0, "PivotHoldPress", "PivotHoldRelease"}, {"J", 0, "DiscreteRotatePress", "DiscreteRotateRelease"},
        {"B", 0, "SoftSelectPress", "SoftSelectRelease"}, {"Tab", 0, "DragSelectPress", "DragSelectRelease"}, {"K", 0, "ScrubTimeKeyPress", "ScrubTimeKeyRelease"},
        {"I", 0, "NudgeKeysKeyPress", "NudgeKeysKeyRelease"}, {"Ins", 0, "TogglePivotMode", ""},
        {"W", S, "SetKeyTranslate", ""}, {"E", S, "SetKeyRotate", ""}, {"R", S, "SetKeyScale", ""}, {"S", 0, "SetKey", ""},
        {"V", A, "PlaybackToggle", ""}, {"V", A | S, "GoToMinFrame", ""}, {".", 0, "NextKey", ""}, {",", 0, "PreviousKey", ""}, {".", A, "NextFrame", ""}, {",", A, "PreviousFrame", ""},
        {"F", 0, "FrameSelected", ""}, {"A", 0, "FrameAll", ""}, {"F", S, "FrameSelectedInAllViews", ""}, {"A", S, "FrameAllInAllViews", ""}, {"[", 0, "PreviousView", ""}, {"]", 0, "NextView", ""},
        {"Z", 0, "Undo", ""}, {"Z", C, "Undo", ""}, {"Z", S, "Redo", ""}, {"Z", C | S, "Redo", ""}, {"Y", C, "Redo", ""}, {"G", 0, "RepeatLast", ""}, {"R", C, "CreateReference", ""}, {"G", C, "Group", ""},
        {"D", C, "Duplicate", ""}, {"D", C | S, "DuplicateSpecialOptions", ""}, {"D", S, "DuplicateWithTransform", ""}, {"D", A | S, "DeleteHistory", ""},
        {"P", 0, "Parent", ""}, {"P", S, "Unparent", ""}, {"H", 0, "MenuSetHoldPress", "MenuSetHoldRelease"}, {"H", S, "ShowSelectedObjects", ""}, {"H", C, "HideSelected", ""}, {"H", C | S, "ShowLastHidden", ""}, {"H", A, "HideUnselectedObjects", ""}, {"H", C | A, "ShowAllComponents", ""},
        {"N", C, "NewScene", ""}, {"O", C, "OpenScene", ""}, {"S", C, "SaveScene", ""}, {"S", C | S, "SaveSceneAs", ""}, {"S", C | A, "IncrementAndSave", ""},
        {"A", C, "AttributeEditor", ""}, {"A", C | S, "SelectAll", ""}, {"I", C | S, "InvertSelection", ""}, {"T", C | S, "ToolSettingsWindow", ""},
        {"E", C, "Extrude", ""}, {"B", C, "Bevel", ""}, {">", 0, "GrowSelection", ""}, {"<", 0, "ShrinkSelection", ""},
        {"Up", 0, "PickWalkUp", ""}, {"Down", 0, "PickWalkDown", ""}, {"Left", 0, "PickWalkLeft", ""}, {"Right", 0, "PickWalkRight", ""},
        {"Up", A, "NudgeUp", ""}, {"Down", A, "NudgeDown", ""}, {"Left", A, "NudgeLeft", ""}, {"Right", A, "NudgeRight", ""},
        {"F1", 0, "Help", ""}, {"F2", 0, "MenuSetModeling", ""}, {"F3", 0, "MenuSetRigging", ""}, {"F4", 0, "MenuSetAnimation", ""}, {"F5", 0, "MenuSetDynamics", ""}, {"F6", 0, "MenuSetRendering", ""}, {"F8", 0, "ToggleSelectMode", ""},
        {"F9", 0, "SelectVertexMask", ""}, {"F10", 0, "SelectEdgeMask", ""}, {"F11", 0, "SelectFacetMask", ""}, {"F12", 0, "SelectUVMask", ""}, {"F9", A, "SelectVertexFaceMask", ""},
        {"F9", C, "ConvertSelectionToVertices", ""}, {"F10", C, "ConvertSelectionToEdges", ""}, {"F11", C, "ConvertSelectionToFaces", ""}, {"F12", C, "ConvertSelectionToUVs", ""},
        {"PgUp", 0, "SmoothingLevelIncrease", ""}, {"PgDown", 0, "SmoothingLevelDecrease", ""}, {"Q", C | S, "QuadDrawTool", ""}, {"X", C | S, "MultiCutTool", ""},
        {"X", C, "CutSelected", ""}, {"C", C, "CopySelected", ""}, {"V", C, "PasteSelected", ""},
        {"1", A, "ToggleNurbsInView", ""}, {"2", A, "TogglePolygonsInView", ""}, {"4", A, "ToggleImagePlanesInView", ""}, {"M", C, "ToggleMainMenubar", ""},
        {"1", 0, "SmoothPreviewOff", ""}, {"2", 0, "SmoothPreviewCage", ""}, {"3", 0, "SmoothPreviewSmooth", ""}, {"4", 0, "WireframeView", ""}, {"5", 0, "ShadedView", ""}, {"6", 0, "TexturedView", ""}, {"7", 0, "DisplayLight", ""},
        {"1", C, "IsolateSelect", ""}, {"A", A, "XRayToggle", ""}, {"B", A, "BackgroundCycle", ""}, {"Space", C, "ToggleMainPane", ""},
        {"Del", 0, "DeleteSelection", ""}, {"Backspace", 0, "DeleteSelection", ""}, {"Del", C, "DeleteEdgeVertex", ""},
        {"Return", 0, "FinishTool", ""}, {"Enter", 0, "FinishTool", ""}, {"Esc", 0, "CancelPlayback", ""},
        {"=", 0, "ManipBigger", ""}, {"+", 0, "ManipBigger", ""}, {"-", 0, "ManipSmaller", ""},
    };
    QList<HotkeyBinding> out;
    // MAYA_DISABLE_BACKSPACE_DELETE: Backspace no longer deletes the selection.
    const bool no_backspace = az::env_on("MAYA_DISABLE_BACKSPACE_DELETE");
    for (const D& x : d) {
        if (no_backspace && QString(x.key) == "Backspace" && QString(x.press) == "DeleteSelection") continue;
        out.append(HotkeyBinding{x.key, (x.mods & C) != 0, (x.mods & S) != 0, (x.mods & A) != 0, x.press, x.release});
    }
    return out;
}

// Every key bound now, as a hotkey set file: hotkey lines naming the runtime commands, which import takes as they are.
void MainWindow::export_hotkeys(const QString& path) const {
    auto q = [](QString t) { return "\"" + t.replace('\\', "\\\\").replace('"', "\\\"") + "\""; };
    QString set = mel("hotkeySet -q -current;").toString();
    QString out = "//Aztek hotkey set\n//\nhotkeySet " + q(set.isEmpty() ? "Aztek_Default" : set) + " -current;\n";
    for (const HotkeyBinding& h : hotkeys_) {
        QString key = h.key.size() == 1 ? h.key.toLower() : h.key;
        QString line = "hotkey -keyShortcut " + q(key);
        if (h.ctrl) line += " -ctl";
        if (h.alt) line += " -alt";
        if (h.shift) line += " -sht";
        if (!h.press.isEmpty()) line += " -name (" + q(h.press) + ")";
        if (!h.release.isEmpty()) line += " -releaseName (" + q(h.release) + ")";
        out += line + ";\n";
    }
    QFile f(path);
    if (!f.open(QIODevice::WriteOnly | QIODevice::Truncate)) { log("// Error: can't write " + path + " //", 3); return; }
    f.write(out.toUtf8());
}

void MainWindow::load_hotkeys() {
    hotkeys_ = default_hotkeys();
    QJsonArray saved = parse(QSettings().value("hotkeys/user").toString()).toArray();
    for (const QJsonValue& v : saved) set_binding(v.toObject());
}

void MainWindow::save_user_hotkey(const QJsonObject& o) {
    QSettings s;
    QJsonArray saved = parse(s.value("hotkeys/user", "[]").toString()).toArray();
    QString k = combo(o["key"].toString(), o["ctrl"].toBool(), o["shift"].toBool(), o["alt"].toBool());
    for (int i = saved.size() - 1; i >= 0; --i) {
        QJsonObject e = saved[i].toObject();
        if (combo(e["key"].toString(), e["ctrl"].toBool(), e["shift"].toBool(), e["alt"].toBool()) == k) saved.removeAt(i);
    }
    saved.append(o);
    s.setValue("hotkeys/user", QString::fromUtf8(QJsonDocument(saved).toJson(QJsonDocument::Compact)));
}

void MainWindow::set_binding(const QJsonObject& o) {
    HotkeyBinding b{o["key"].toString(), o["ctrl"].toBool(), o["shift"].toBool(), o["alt"].toBool(), o["press"].toString(), o["release"].toString()};
    for (int i = hotkeys_.size() - 1; i >= 0; --i) {
        const HotkeyBinding& h = hotkeys_[i];
        if (h.key == b.key && h.ctrl == b.ctrl && h.shift == b.shift && h.alt == b.alt) hotkeys_.removeAt(i);
    }
    // No command: the key is left unbound.
    if (!b.press.isEmpty() || !b.release.isEmpty()) hotkeys_.append(b);
}

void MainWindow::set_pivot_mode(bool on) {
    pivot_mode_ = on;
    for (ViewportWidget* v : panels_->views()) v->set("pivotMode", on ? "true" : "false");
    if (tool_settings_) tool_settings_->show_pivot_mode(on);
    help_line_->setText(on ? "Edit Pivot: drag the manipulator to move the pivot. D or Insert leaves." : QString());
}

bool MainWindow::hotkey(QKeyEvent* k, bool press) {
    int code = k->key();
    QString key = key_name(code);
    if (k->isAutoRepeat()) return held_.contains(code);
    if (!press) {
        // The release runs what the press started, whatever the modifiers are now.
        auto it = held_.find(code);
        if (it == held_.end()) return false;
        QString rel = *it;
        held_.erase(it);
        if (!rel.isEmpty()) run_command(rel);
        return true;
    }
    Qt::KeyboardModifiers m = k->modifiers() & ~Qt::KeypadModifier;
    bool ctrl = m & Qt::ControlModifier, shift = (m & Qt::ShiftModifier) && !shifted_symbol(code), alt = m & Qt::AltModifier;
    for (const HotkeyBinding& h : hotkeys_) {
        if (h.key.compare(key, Qt::CaseInsensitive) != 0 || h.ctrl != ctrl || h.shift != shift || h.alt != alt) continue;
        // Modelling tools take Tab themselves.
        held_[code] = h.release;
        run_command(h.press);
        return true;
    }
    return false;
}

// ---------------------------------------------------------------------------------------------------------------------
// Hotkey Editor

void MainWindow::show_hotkey_editor() {
    auto* d = new QDialog(this);
    d->setAttribute(Qt::WA_DeleteOnClose);
    d->setWindowTitle("Hotkey Editor");
    auto* lay = new QVBoxLayout(d);
    auto* filter = new QLineEdit(d);
    filter->setPlaceholderText("Search commands and keys...");
    lay->addWidget(filter);
    auto* tree = new QTreeWidget(d);
    tree->setHeaderLabels({"Command", "Hotkey", "Release", "What it does"});
    tree->setRootIsDecorated(true);
    tree->header()->resizeSection(0, 260);
    tree->header()->resizeSection(1, 130);
    tree->header()->resizeSection(2, 170);
    lay->addWidget(tree, 1);

    auto* form = new QFormLayout();
    auto* key = new QLineEdit(d);
    key->setPlaceholderText("Key: W, F8, Space, Up...");
    auto* ctrl = new QCheckBox("Ctrl", d);
    auto* alt = new QCheckBox("Alt", d);
    auto* shift = new QCheckBox("Shift", d);
    auto* row = new QHBoxLayout();
    row->addWidget(key, 1);
    row->addWidget(ctrl);
    row->addWidget(alt);
    row->addWidget(shift);
    form->addRow("Hotkey:", row);
    auto* press = new QLineEdit(d);
    press->setPlaceholderText("A runtime command's name, or MEL");
    form->addRow("On press:", press);
    auto* release = new QLineEdit(d);
    release->setPlaceholderText("Optional: runs when the key comes up");
    form->addRow("On release:", release);
    auto* warn = new QLabel(d);
    warn->setObjectName("optionsDoc");
    form->addRow("", warn);
    lay->addLayout(form);
    auto* buttons = new QHBoxLayout();
    auto* assign = new QPushButton("Assign", d);
    auto* unassign = new QPushButton("Unassign", d);
    auto* reset = new QPushButton("Restore Defaults", d);
    auto* close = new QPushButton("Close", d);
    buttons->addWidget(assign);
    buttons->addWidget(unassign);
    auto* import_set = new QPushButton("Import...", d);
    auto* export_set = new QPushButton("Export...", d);
    buttons->addStretch(1);
    buttons->addWidget(import_set);
    buttons->addWidget(export_set);
    buttons->addWidget(reset);
    buttons->addWidget(close);
    lay->addLayout(buttons);

    auto fill = [this, tree, filter] {
        tree->clear();
        QString f = filter->text().trimmed();
        QMap<QString, QTreeWidgetItem*> cats;
        QHash<QString, QString> keys, rels;
        for (const HotkeyBinding& h : hotkeys_) {
            QString c = combo(h.key, h.ctrl, h.shift, h.alt);
            keys[h.press] = keys.contains(h.press) ? keys[h.press] + ", " + c : c;
            rels[h.press] = h.release;
        }
        auto add = [&](const QString& cat, const QString& name, const QString& ann) {
            QString hk = keys.value(name);
            if (!f.isEmpty() && !name.contains(f, Qt::CaseInsensitive) && !hk.contains(f, Qt::CaseInsensitive) && !ann.contains(f, Qt::CaseInsensitive)) return;
            if (!cats.contains(cat)) {
                auto* c = new QTreeWidgetItem(tree, {cat});
                c->setFirstColumnSpanned(true);
                cats[cat] = c;
            }
            auto* it = new QTreeWidgetItem(cats[cat], {name, hk, rels.value(name), ann});
            it->setData(0, Qt::UserRole, name);
        };
        for (auto it = runtime_.begin(); it != runtime_.end(); ++it) add(it->category, it.key(), it->annotation);
        QJsonObject user = query("runtimeCommands").toObject();
        for (const QJsonValue& v : user["runTime"].toArray()) add(v.toObject()["category"].toString("User"), v.toObject()["name"].toString(), v.toObject()["annotation"].toString());
        for (const QJsonValue& v : user["nameCommands"].toArray()) add("Name Commands", v.toObject()["name"].toString(), v.toObject()["annotation"].toString());
        // A binding to MEL has no name in the list.
        for (const HotkeyBinding& h : hotkeys_)
            if (!runtime_.contains(h.press)) add("Scripted", h.press, QString());
        for (auto* c : cats) c->setExpanded(!f.isEmpty());
        tree->sortItems(0, Qt::AscendingOrder);
    };
    fill();
    connect(filter, &QLineEdit::textChanged, d, fill);
    connect(tree, &QTreeWidget::currentItemChanged, d, [this, key, ctrl, alt, shift, press, release](QTreeWidgetItem* it) {
        if (!it || it->data(0, Qt::UserRole).isNull()) return;
        QString name = it->data(0, Qt::UserRole).toString();
        press->setText(name);
        release->clear();
        for (const HotkeyBinding& h : hotkeys_)
            if (h.press == name) {
                key->setText(h.key);
                ctrl->setChecked(h.ctrl);
                alt->setChecked(h.alt);
                shift->setChecked(h.shift);
                release->setText(h.release);
                return;
            }
        key->clear();
        ctrl->setChecked(false);
        alt->setChecked(false);
        shift->setChecked(false);
        // Paired names release themselves.
        if (name.endsWith("Press")) release->setText(name.left(name.size() - 5) + "Release");
    });
    auto show_clash = [this, key, ctrl, alt, shift, press, warn] {
        warn->clear();
        for (const HotkeyBinding& h : hotkeys_)
            if (h.key.compare(key->text().trimmed(), Qt::CaseInsensitive) == 0 && h.ctrl == ctrl->isChecked() && h.alt == alt->isChecked() && h.shift == shift->isChecked() && h.press != press->text())
                warn->setText(QString("Now on %1; assigning replaces it.").arg(h.press));
    };
    connect(key, &QLineEdit::textChanged, d, show_clash);
    connect(ctrl, &QCheckBox::toggled, d, show_clash);
    connect(alt, &QCheckBox::toggled, d, show_clash);
    connect(shift, &QCheckBox::toggled, d, show_clash);
    connect(press, &QLineEdit::textChanged, d, show_clash);
    auto store = [this, key, ctrl, alt, shift, fill](const QString& p, const QString& r) {
        QString k = key->text().trimmed();
        if (k.isEmpty()) return;
        // Letter keys are named in capitals by the key events.
        if (k.size() == 1) k = k.toUpper();
        QJsonObject o{{"key", k}, {"ctrl", ctrl->isChecked()}, {"alt", alt->isChecked()}, {"shift", shift->isChecked()}, {"press", p}, {"release", r}};
        set_binding(o);
        save_user_hotkey(o);
        fill();
    };
    connect(assign, &QPushButton::clicked, d, [store, press, release] { store(press->text().trimmed(), release->text().trimmed()); });
    connect(unassign, &QPushButton::clicked, d, [store] { store(QString(), QString()); });
    connect(reset, &QPushButton::clicked, d, [this, fill] {
        QSettings().remove("hotkeys/user");
        load_hotkeys();
        fill();
    });
    connect(import_set, &QPushButton::clicked, d, [d, fill] {
        QString f = QFileDialog::getOpenFileName(d, "Import Hotkey Set", QString(), "Hotkey sets (*.mhk *.mel);;All files (*)");
        if (f.isEmpty()) return;
        mel(QString("hotkeySet -import \"%1\";").arg(QString(f).replace('\\', '/')));
        fill();
    });
    connect(export_set, &QPushButton::clicked, d, [this, d] {
        QString f = QFileDialog::getSaveFileName(d, "Export Hotkey Set", "Aztek_Hotkeys.mhk", "Hotkey sets (*.mhk)");
        if (f.isEmpty()) return;
        export_hotkeys(f);
    });
    connect(close, &QPushButton::clicked, d, &QDialog::close);
    d->resize(820, 640);
    d->show();
}

// The four views' panel names, as the reference has them: top, side, front, persp. views() runs top, persp, front, side.
ViewportWidget* MainWindow::view_for_panel(const QString& panel) const {
    // The view with focus, for what goes over the active view (inViewMessage).
    if (panel == "<active>") return active_view();
    if (ViewportWidget* v = ScriptUi::get()->embedded_view(panel)) return v;
    static const QStringList names{"modelPanel1", "modelPanel4", "modelPanel3", "modelPanel2"};
    int i = names.indexOf(panel);
    QList<ViewportWidget*> views = panels_->views();
    return i >= 0 && i < views.size() ? views[i] : nullptr;
}

// What the views show now, for getPanel and modelEditor -q. Only when something changed.
void MainWindow::sync_panels() {
    static const QStringList names{"modelPanel1", "modelPanel4", "modelPanel3", "modelPanel2"};
    QList<ViewportWidget*> views = panels_->views();
    QJsonArray list;
    QString focus = names[1], pointer;
    ViewportWidget* act = active_view();
    for (int i = 0; i < views.size() && i < names.size(); ++i) {
        ViewportWidget* v = views[i];
        if (v == act) focus = names[i];
        QJsonObject o{{"name", names[i]}, {"visible", v->isVisible()}};
        QString cam = v->get("camera");
        // A view not drawn yet has nothing to say.
        if (!cam.isEmpty()) {
            o.insert("camera", parse(cam));
            o.insert("disp", parse(v->get("disp")));
            o.insert("wireframe", parse(v->get("wireframe")));
            o.insert("wireOnShaded", parse(v->get("wireOnShaded")));
            o.insert("grid", parse(v->get("grid")));
        }
        list.append(o);
    }
    pointer = focus;
    QString text = QString::fromUtf8(QJsonDocument(QJsonObject{{"focus", focus}, {"underPointer", pointer}, {"panels", list}}).toJson(QJsonDocument::Compact));
    if (text != panels_synced_) {
        panels_synced_ = text;
        query("panelSync", text);
    }
}

// A MEL `hotkey` command, and everything else scripts ask the shell for.
void MainWindow::handle_ui_request(const QString& r) {
    int bar = r.indexOf('|');
    QString name = bar < 0 ? r : r.left(bar), payload = bar < 0 ? QString() : r.mid(bar + 1);
    if (name == "mel") {
        mel(payload);
    } else if (name == "py") {
        python(payload);
    } else if (name == "quit") {
        QJsonObject o = parse(payload).toObject();
        if (o["force"].toBool()) no_prompts_ = true;
        int code = o["exitCode"].toInt();
        QTimer::singleShot(0, this, [this, code] {
            if (close()) QCoreApplication::exit(code);
        });
    } else if (name == "hotBox") {
        // hotBox from a script: show at -position or the cursor, hide on -release, -updateMenus builds the rows again.
        if (payload.startsWith("show|")) {
            QJsonObject o = parse(payload.mid(5)).toObject();
            QJsonArray at = o["position"].toArray();
            QPoint global = at.size() == 2 ? QPoint(at[0].toInt(), at[1].toInt()) : QCursor::pos();
            if (hotbox_ && hotbox_->isVisible()) hotbox_->hide();
            show_hotbox(global, o["noKeyPress"].toBool());
        } else if (payload == "hide") {
            if (hotbox_ && hotbox_->isVisible()) hotbox_->hide();
        } else if (payload == "update") {
            // The menus and the hotbox's rows made again, once the request is done.
            QTimer::singleShot(0, this, [this] { build_menu_set(menu_set_); });
        }
    } else if (name == "scriptEditorClear") {
        // scriptEditorInfo -clearHistory: the history pane, and what was kept for it before it opened.
        log_before_editor_ = QJsonArray();
        if (script_editor_) script_editor_->clear_history();
    } else if (name == "shelfAdd") {
        // A field's -saveSelectionToShelf: a button on the current shelf.
        QJsonObject o = parse(payload).toObject();
        if (shelf_bar_) shelf_bar_->add_to_current(o["label"].toString(), "script", o["command"].toString(), o["sourceType"].toString("mel"));
    } else if (name == "scriptEditorInput") {
        if (script_editor_) script_editor_->set_input(payload);
    } else if (name == "aboutBox") {
        // license -showProductInfoDialog.
        show_about(this);
    } else if (name == "launch") {
        // launch: a page, movie, PDF or folder in the application the system has for it.
        QUrl url = payload.contains("://") ? QUrl(payload) : QUrl::fromLocalFile(payload);
        az::open_external(url);
    } else if (name == "fcheck") {
        // fcheck: an image in its own window, or a numbered sequence played at the scene's rate.
        QJsonObject o = parse(payload).toObject();
        QStringList files;
        for (const QJsonValue& v : o["files"].toArray()) files << v.toString();
        if (files.isEmpty()) return;
        auto* w = new QWidget(nullptr, Qt::Window);
        w->setAttribute(Qt::WA_DeleteOnClose);
        w->setWindowTitle("fcheck: " + o["title"].toString());
        auto* lay = new QVBoxLayout(w);
        lay->setContentsMargins(0, 0, 0, 0);
        lay->setSpacing(0);
        auto* picture = new QLabel;
        picture->setAlignment(Qt::AlignCenter);
        picture->setStyleSheet("background: #000;");
        auto* status = new QLabel;
        status->setContentsMargins(6, 3, 6, 3);
        lay->addWidget(picture, 1);
        lay->addWidget(status);
        auto show = [picture, status, files](int i) {
            QImage img(files[i]);
            if (img.isNull()) {
                picture->setText("can't read this image");
            } else {
                picture->setPixmap(QPixmap::fromImage(img));
            }
            status->setText(QString("%1   %2 of %3   %4 x %5").arg(QFileInfo(files[i]).fileName()).arg(i + 1).arg(files.size()).arg(img.width()).arg(img.height()));
        };
        show(0);
        QImage first(files[0]);
        w->resize(std::max(first.width(), 240), std::max(first.height(), 120) + 24);
        if (o["sequence"].toBool() && files.size() > 1) {
            double fps = ask("playbackOptions", QJsonObject{{"query", true}, {"queried", QJsonArray{"framesPerSecond"}}}).toDouble(24);
            if (fps <= 0) fps = 24.0;
            auto* timer = new QTimer(w);
            auto at = std::make_shared<int>(0);
            QObject::connect(timer, &QTimer::timeout, w, [at, show, n = int(files.size())] {
                *at = (*at + 1) % n;
                show(*at);
            });
            timer->start(int(1000.0 / fps));
        }
        w->show();
    } else if (name == "setTool") {
        set_tool(payload);
    } else if (name == "repeatAdd") {
        // repeatLast -addCommand: Edit > Repeat (G) runs this script next, under its label.
        QJsonObject o = parse(payload).toObject();
        const QString cmd = o["command"].toString();
        last_repeat_ = [this, cmd] { mel(cmd); };
        last_repeat_label_ = o["label"].toString(cmd);
    } else if (name == "repeatLast") {
        repeat_last();
    } else if (name == "openUrl") {
        az::open_external(QUrl::fromUserInput(payload));
    } else if (name == "ctxCompleted") {
        for (ViewportWidget* v : panels_->views()) v->set("finishTool", "true");
    } else if (name == "uvTool") {
        UvEditor::set_tool_everywhere(payload);
    } else if (name == "keyTool") {
        CurveCanvas::set_tool(payload);
        DopeCanvas::set_tool(payload);
        help_line_->setText(QString("Key tool: %1").arg(payload));
    } else if (name == "pixelMove") {
        QJsonObject o = parse(payload).toObject();
        nudge(int(o["x"].toDouble()), int(o["y"].toDouble()));
    } else if (name == "panelSet") {
        QJsonObject o = parse(payload).toObject();
        if (ViewportWidget* v = view_for_panel(o["panel"].toString())) {
            QByteArray text = QJsonDocument(QJsonArray{o.value("value")}).toJson(QJsonDocument::Compact);
            v->set(o["key"].toString(), QString::fromUtf8(text.mid(1, text.size() - 2)));
        }
    } else if (name == "panelFocus") {
        if (ViewportWidget* v = view_for_panel(payload)) emit v->activated(v);
    } else if (name == "panelFocusUndo" || name == "panelFocusRedo") {
        if (ViewportWidget* v = active_view()) v->set(name == "panelFocusUndo" ? "viewUndo" : "viewRedo", "true");
    } else if (name == "viewAll") {
        QJsonObject o = parse(payload).toObject();
        QByteArray text = QJsonDocument(QJsonArray{o.value("value")}).toJson(QJsonDocument::Compact);
        for (ViewportWidget* v : panels_->views()) v->set(o["key"].toString(), QString::fromUtf8(text.mid(1, text.size() - 2)));
    } else if (name == "inViewMessage") {
        ScriptUi::get()->show_in_view_message(parse(payload).toObject());
    } else if (name == "lookThru") {
        if (ViewportWidget* v = active_view()) v->set("camera", "\"" + payload + "\"");
    } else if (name == "appHome") {
        app_home(parse(payload).toObject());
    } else if (name == "viewCube") {
        if (ViewportWidget* v = active_view()) v->set("viewCube", payload);
    } else if (name == "viewCubeMenu") {
        // The ViewCube's menu, at the mouse, for the view it was opened from.
        ViewportWidget* v = active_view();
        if (!v) return;
        const QJsonObject o = parse(payload).toObject();
        QMenu menu;
        auto act = [&menu, v](const QString& text, const char* action) {
            QAction* a = menu.addAction(text);
            QObject::connect(a, &QAction::triggered, v, [v, action] { v->set("viewCube", QString("{\"action\":\"%1\"}").arg(action)); });
            return a;
        };
        // As the reference's: Default View, Orthographic View, Preferences, the two help pages, Hide ViewCube.
        act("Default View", "default");
        const bool is_ortho = o["orthographic"].toBool();
        QAction* ortho = act("Orthographic View", is_ortho ? "perspective" : "orthographic");
        ortho->setCheckable(true);
        ortho->setChecked(is_ortho);
        menu.addSeparator();
        QObject::connect(menu.addAction("Preferences..."), &QAction::triggered, this, [this] { show_preferences_at("ViewCube"); });
        menu.addSeparator();
        for (const char* text : {"Help on Viewport Navigation...", "Help on the ViewCube..."})
            QObject::connect(menu.addAction(text), &QAction::triggered, this, [] { QDesktopServices::openUrl(QUrl("https://srlegrand.github.io/Aztek/")); });
        menu.addSeparator();
        QObject::connect(menu.addAction("Hide ViewCube"), &QAction::triggered, this, [this] {
            QSettings().setValue("prefs/viewCubeShow", false);
            apply_prefs();
        });
        menu.exec(QCursor::pos());
    } else if (name == "polyOptions") {
        for (ViewportWidget* v : panels_->views()) v->set("disp", payload);
    } else if (name == "isolate") {
        if (ViewportWidget* v = active_view()) v->set("isolate", payload);
    } else if (name == "workspace") {
        apply_workspace(payload);
    } else if (name == "workspaceSave") {
        // workspaceLayoutManager -save and -saveAs: the arrangement kept under the name.
        QSettings().setValue("workspaces/" + payload, saveState());
        QSettings().setValue("workspaces_current", payload);
        if (workspace_box_) {
            QSignalBlocker b(workspace_box_);
            if (workspace_box_->findText(payload) < 0) workspace_box_->insertItem(0, payload);
            workspace_box_->setCurrentText(payload);
        }
    } else if (name == "workspaceDelete") {
        QSettings().remove("workspaces/" + payload);
        if (workspace_box_ && !workspace_names().contains(payload)) {
            int i = workspace_box_->findText(payload);
            if (i >= 0) workspace_box_->removeItem(i);
        }
    } else if (name == "workspaceCollapse") {
        // Every docked control but the one named is put away; the arrangement is kept to restore.
        const QString keep = payload.section('|', 0, 0);
        const bool hide_bars = payload.section('|', 1, 1) == "1";
        QSettings().setValue("workspaces_collapsed", saveState());
        // The Time and Range Sliders count as bars, as the toolbars they once were.
        for (QDockWidget* d : findChildren<QDockWidget*>())
            if (d->objectName() != keep && d->windowTitle() != keep && !d->isFloating() &&
                (hide_bars || (d->objectName() != "TimeSlider" && d->objectName() != "RangeSlider")))
                d->hide();
        if (hide_bars)
            for (QToolBar* b : findChildren<QToolBar*>()) b->hide();
    } else if (name == "workspaceRestore") {
        QByteArray kept = QSettings().value("workspaces_collapsed").toByteArray();
        if (!kept.isEmpty()) restoreState(kept);
        QSettings().remove("workspaces_collapsed");
    } else if (name == "workspaceReset") {
        if (workspace_names().contains(payload)) QSettings().remove("workspaces/" + payload);
        apply_workspace(payload.isEmpty() ? QString("General") : payload);
    } else if (name == "saveShelf") {
        // saveShelf on one of the window's shelves: written as the reference's shelf file to the path given.
        const QJsonObject o = parse(payload).toObject();
        if (shelf_bar_)
            for (const QJsonValue& v : shelf_bar_->data())
                if (v.toObject()["name"].toString() == o["shelf"].toString()) ShelfBar::write_mel(o["path"].toString(), v.toObject());
    } else if (name == "saveAllShelves") {
        if (shelf_bar_) shelf_bar_->save();
    } else if (name == "hotkeyFactory") {
        QSettings().remove("hotkeys/user");
        load_hotkeys();
    } else if (name == "hotkey") {
        QJsonObject h = parse(payload).toObject();
        auto named = [](const QString& n) -> QString {
            if (n.isEmpty()) return n;
            for (const QJsonValue& v : query("runtimeCommands").toObject()["nameCommands"].toArray())
                if (v.toObject()["name"].toString() == n) return v.toObject()["command"].toString();
            return n;
        };
        QString key = h["key"].toString();
        if (key.size() == 1) key = key.toUpper();
        bool upper_shift = h["key"].toString().size() == 1 && h["key"].toString()[0].isUpper();
        QJsonObject o{{"key", key}, {"ctrl", h["ctrl"]}, {"alt", h["alt"]}, {"shift", h["shift"].toBool() || upper_shift}, {"press", named(h["name"].toString())}, {"release", named(h["releaseName"].toString())}};
        set_binding(o);
        save_user_hotkey(o);
    } else if (runtime_.contains(name)) {
        runtime_[name].fn();
    }
}

} // namespace az
