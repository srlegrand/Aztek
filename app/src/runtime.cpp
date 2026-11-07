// Named actions (the reference's runtime commands), the hotkey table that runs them and the Hotkey Editor. A hotkey runs a named
// action or MEL; bindings the user changes are saved in the settings over the defaults.
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
#include <QCheckBox>
#include <QComboBox>
#include <QDateTime>
#include <QDialog>
#include <QDockWidget>
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
#include <QMessageBox>
#include <QPushButton>
#include <QSettings>
#include <QStatusBar>
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
        for (const char* s : {"point", "curve", "grid", "viewPlane", "live"})
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
    r("EditMembershipTool", "Tools", "Edit Membership", [this] { set_tool("editMembership"); show_tool_settings(); });
    r("ArtPaintSkinWeightsToolOptions", "Tools", "Paint Skin Weights and its settings", [this] { set_tool("paintSkin"); show_tool_settings(); });
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
    r("PreferencesWindow", "Windows", "Preferences", [this] { show_preferences(); });
    r("HotkeyPreferencesWindow", "Windows", "Hotkey Editor", [this] { show_hotkey_editor(); });
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
    window("HumanIK", "humanIK", "Human IK");
    window("HIKCharacterControlsTool", "humanIK", "Human IK");
    window("ModelingToolkitWindow", "modelingToolkit", "Modeling Toolkit");
    window("GridOptions", "gridOptions", "Grid Options");
    window("AddAttribute", "addAttribute", "Add Attribute");
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
        QSettings s;
        double bg = s.value("prefs/background", 0.36).toDouble();
        bg = bg > 0.5 ? 0.0 : bg > 0.2 ? 0.8 : 0.36;
        s.setValue("prefs/background", bg);
        apply_prefs();
    });
    r("WireframeView", "View", "Wireframe", [this] { view_key("wireframe", "true"); });
    r("ShadedView", "View", "Smooth shade all", [this] { view_key("wireframe", "false"); view_key("disp", "{\"textured\":false,\"displayLights\":\"default\"}"); });
    r("TexturedView", "View", "Hardware texturing", [this] { view_key("wireframe", "false"); view_key("disp", "{\"textured\":true,\"displayLights\":\"default\"}"); });
    r("DisplayLight", "View", "Use all lights", [this] { view_key("wireframe", "false"); view_key("disp", "{\"displayLights\":\"all\"}"); });
    r("PlaybackToggle", "Animation", "Play or stop", [this] { toggle_play(1); });
    r("GoToMinFrame", "Animation", "Go to the start of the playback range", [this] { time_slider_->set_time(time_slider_->start()); });
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
    r("Parent", "Edit", "Parent", [] { exec("parent"); });
    r("Unparent", "Edit", "Unparent", [] { exec("parent", QJsonObject{{"world", true}}); });
    r("HideSelected", "Display", "Hide the selection", [] { exec("hide"); });
    r("ShowLastHidden", "Display", "Show what was last hidden", [] { exec("showLastHidden"); });
    r("ToggleVisibility", "Display", "Show or hide", [] { exec("toggleVisibility"); });
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
    r("DeleteEdgeVertex", "Mesh", "Delete edges or vertices", [] { exec("polyDissolve"); });
    r("FinishTool", "Tools", "Finish the current tool", [this] {
        if (tool_ == "joint") { exec("select", QJsonObject{{"clear", true}}); return; }
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
    r("MenuSetAnimation", "Menus", "Animation menus", [this] { menu_set_->setCurrentText("Animation"); });
    r("MenuSetModeling", "Menus", "Modeling menus", [this] { menu_set_->setCurrentText("Modeling"); });
    r("MenuSetRigging", "Menus", "Rigging menus", [this] { menu_set_->setCurrentText("Rigging"); });
    r("MenuSetRendering", "Menus", "Rendering menus", [this] { menu_set_->setCurrentText("Rendering"); });
    r("TogglePivotMode", "Tools", "Move the pivot rather than the object", [this] {
        pivot_mode_ = !pivot_mode_;
        for (ViewportWidget* v : panels_->views()) v->set("pivotMode", pivot_mode_ ? "true" : "false");
    });
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
    r("PivotHoldPress", "Tools", "Move the pivot while held", [this] {
        for (ViewportWidget* v : panels_->views()) v->set("pivotMode", "true");
        help_line_->setText("Pivot: drag the manipulator to move the pivot");
    });
    r("PivotHoldRelease", "Tools", "Move the pivot while held", [this] {
        for (ViewportWidget* v : panels_->views()) v->set("pivotMode", pivot_mode_ ? "true" : "false");
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
        if (dragged) return;
        bool on = ask("softSelect", QJsonObject{{"query", true}, {"queried", QJsonArray{"softSelectEnabled"}}}).toInt() != 0;
        exec("softSelect", QJsonObject{{"softSelectEnabled", on ? 0 : 1}});
        for (ViewportWidget* v : panels_->views()) v->update();
        help_line_->setText(on ? "Soft Select off" : "Soft Select on: hold B and drag to size the falloff");
    });
    r("HotBoxPress", "Menus", "Hold for the hotbox, tap for one or four views", [this] {
        space_down_ms_ = QDateTime::currentMSecsSinceEpoch();
        space_held_ = true;
        QTimer::singleShot(220, this, [this] {
            if (space_held_ && hotbox_) hotbox_->popup(QCursor::pos());
        });
    });
    r("HotBoxRelease", "Menus", "Hold for the hotbox, tap for one or four views", [this] {
        space_held_ = false;
        qint64 held = QDateTime::currentMSecsSinceEpoch() - space_down_ms_;
        QWidget* popup = QApplication::activePopupWidget();
        if (hotbox_ && hotbox_->isVisible() && !(popup && popup->inherits("QMenu"))) hotbox_->hide();
        if (held < 220) panels_->toggle();
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

    // Scripts can call all of these by name, as the reference's runtime commands.
    QJsonArray names;
    for (auto it = runtime_.begin(); it != runtime_.end(); ++it) names.append(QJsonArray{it.key(), it->annotation});
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
        {"P", 0, "Parent", ""}, {"P", S, "Unparent", ""}, {"H", 0, "ToggleVisibility", ""}, {"H", C, "HideSelected", ""}, {"H", C | S, "ShowLastHidden", ""},
        {"N", C, "NewScene", ""}, {"O", C, "OpenScene", ""}, {"S", C, "SaveScene", ""}, {"S", C | S, "SaveSceneAs", ""}, {"S", C | A, "IncrementAndSave", ""},
        {"A", C, "AttributeEditor", ""}, {"A", C | S, "SelectAll", ""}, {"I", C | S, "InvertSelection", ""}, {"T", C | S, "ToolSettingsWindow", ""},
        {"E", C, "Extrude", ""}, {"B", C, "Bevel", ""}, {">", 0, "GrowSelection", ""}, {"<", 0, "ShrinkSelection", ""},
        {"Up", 0, "PickWalkUp", ""}, {"Down", 0, "PickWalkDown", ""}, {"Left", 0, "PickWalkLeft", ""}, {"Right", 0, "PickWalkRight", ""},
        {"Up", A, "NudgeUp", ""}, {"Down", A, "NudgeDown", ""}, {"Left", A, "NudgeLeft", ""}, {"Right", A, "NudgeRight", ""},
        {"F2", 0, "MenuSetAnimation", ""}, {"F3", 0, "MenuSetModeling", ""}, {"F4", 0, "MenuSetRigging", ""}, {"F6", 0, "MenuSetRendering", ""}, {"F8", 0, "ToggleSelectMode", ""},
        {"F9", 0, "SelectVertexMask", ""}, {"F10", 0, "SelectEdgeMask", ""}, {"F11", 0, "SelectFacetMask", ""}, {"F12", 0, "UVEditor", ""},
        {"F9", C, "ConvertSelectionToVertices", ""}, {"F10", C, "ConvertSelectionToEdges", ""}, {"F11", C, "ConvertSelectionToFaces", ""},
        {"1", 0, "SmoothPreviewOff", ""}, {"2", 0, "SmoothPreviewCage", ""}, {"3", 0, "SmoothPreviewSmooth", ""}, {"4", 0, "WireframeView", ""}, {"5", 0, "ShadedView", ""}, {"6", 0, "TexturedView", ""}, {"7", 0, "DisplayLight", ""},
        {"1", C, "IsolateSelect", ""}, {"A", A, "XRayToggle", ""}, {"B", A, "BackgroundCycle", ""}, {"Space", C, "ToggleMainPane", ""},
        {"Del", 0, "DeleteSelection", ""}, {"Backspace", 0, "DeleteSelection", ""}, {"Del", C, "DeleteEdgeVertex", ""},
        {"Return", 0, "FinishTool", ""}, {"Enter", 0, "FinishTool", ""}, {"Esc", 0, "CancelPlayback", ""},
        {"=", 0, "ManipBigger", ""}, {"+", 0, "ManipBigger", ""}, {"-", 0, "ManipSmaller", ""},
    };
    QList<HotkeyBinding> out;
    for (const D& x : d) out.append(HotkeyBinding{x.key, (x.mods & C) != 0, (x.mods & S) != 0, (x.mods & A) != 0, x.press, x.release});
    return out;
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
    buttons->addStretch(1);
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
    connect(close, &QPushButton::clicked, d, &QDialog::close);
    d->resize(820, 640);
    d->show();
}

// The four views' panel names, as the reference has them: top, side, front, persp. views() runs top, persp, front, side.
ViewportWidget* MainWindow::view_for_panel(const QString& panel) const {
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
    } else if (name == "setTool") {
        set_tool(payload);
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
    } else if (name == "polyOptions") {
        for (ViewportWidget* v : panels_->views()) v->set("disp", payload);
    } else if (name == "isolate") {
        if (ViewportWidget* v = active_view()) v->set("isolate", payload);
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
