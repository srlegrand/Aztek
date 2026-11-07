//! The window's panels: menus, toolbox, outliner, channel box, script editor and command line, and the viewport's
//! mouse and key handling.

use aztek_ops::{execute, CallSource};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::plug::{flags, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use aztek_scene::transform::{DagNode, Transform};
use aztek_scene::Session;
use egui::{Color32, Key, Modifiers, PointerButton, RichText};
use glam::DVec2;
use serde_json::{json, Value};

use crate::mel;
use crate::viewport::{gather, Drag, DrawItem, Tool, Viewport};

#[derive(Clone)]
pub struct Line {
    pub text: String,
    pub kind: LineKind,
}

#[derive(Clone, Copy, PartialEq)]
pub enum LineKind {
    Echo,
    Result,
    Error,
    Info,
}

pub struct Ui {
    pub history: Vec<Line>,
    pub command_line: String,
    pub script: String,
    pub show_script_editor: bool,
    pub show_outliner: bool,
    pub show_channel_box: bool,
    pub show_shapes: bool,
    pub feedback: String,
    pub file_dialog: Option<FileDialog>,
    pub current_file: Option<String>,
    /// Channel box drag in progress: the plug whose last setAttr is undone before the next.
    cb_drag: Option<String>,
    /// Viewport mouse state.
    nav: Option<(PointerButton, DVec2)>,
    marquee_start: Option<DVec2>,
    rmb_menu: Option<egui::Pos2>,
    pub quit: bool,
    /// Set by setToolTo; the viewport picks it up.
    pending_tool: Option<Tool>,
}

pub struct FileDialog {
    pub kind: FileKind,
    pub path: String,
}

#[derive(Clone, Copy, PartialEq)]
pub enum FileKind {
    Open,
    SaveAs,
    ImportObj,
    ExportObj,
}

impl Ui {
    pub fn new() -> Ui {
        Ui {
            history: vec![Line { text: "// Aztek. MEL-style commands: polyCube -w 2; select -r pCube1.f[0:3];".into(), kind: LineKind::Info }],
            command_line: String::new(),
            script: String::new(),
            show_script_editor: false,
            show_outliner: true,
            show_channel_box: true,
            show_shapes: false,
            feedback: String::new(),
            file_dialog: None,
            current_file: None,
            cb_drag: None,
            nav: None,
            marquee_start: None,
            rmb_menu: None,
            quit: false,
            pending_tool: None,
        }
    }

    fn log(&mut self, text: impl Into<String>, kind: LineKind) {
        let text = text.into();
        if kind != LineKind::Echo {
            self.feedback = text.clone();
        }
        self.history.push(Line { text, kind });
        if self.history.len() > 5000 {
            self.history.drain(..1000);
        }
    }

    /// Runs a command as the UI does: echoed, result or error logged.
    pub fn exec(&mut self, s: &mut Session, name: &str, call: Value) -> Option<Value> {
        self.log(mel::echo(name, &call), LineKind::Echo);
        match execute(s, name, &call, CallSource::Ui) {
            Ok(v) => {
                if let Some(r) = mel::result(&v) {
                    self.log(r, LineKind::Result);
                }
                Some(v)
            }
            Err(e) => {
                self.log(format!("// Error: {e} //"), LineKind::Error);
                None
            }
        }
    }

    /// Runs MEL-style source: one or more statements.
    pub fn run_source(&mut self, s: &mut Session, src: &str) {
        for stmt in mel::statements(src) {
            // UI-side commands.
            let words: Vec<&str> = stmt.split_whitespace().collect();
            if words.first() == Some(&"setToolTo") {
                self.history.push(Line { text: format!("{stmt};"), kind: LineKind::Echo });
                let tool = match words.get(1).copied().unwrap_or("") {
                    "moveSuperContext" | "Move" | "move" => Some(Tool::Move),
                    "RotateSuperContext" | "Rotate" | "rotate" => Some(Tool::Rotate),
                    "scaleSuperContext" | "Scale" | "scale" => Some(Tool::Scale),
                    "selectSuperContext" | "Select" | "select" => Some(Tool::Select),
                    _ => None,
                };
                match tool {
                    Some(t) => self.pending_tool = Some(t),
                    None => self.log("// Error: setToolTo: unknown tool //", LineKind::Error),
                }
                continue;
            }
            match mel::parse(&s.commands, &stmt) {
                Ok((name, call)) => {
                    self.history.push(Line { text: format!("{stmt};"), kind: LineKind::Echo });
                    match execute(s, &name, &call, CallSource::Script) {
                        Ok(v) => {
                            if let Some(r) = mel::result(&v) {
                                self.log(r, LineKind::Result);
                            }
                        }
                        Err(e) => self.log(format!("// Error: {e} //"), LineKind::Error),
                    }
                }
                Err(e) => self.log(format!("// Error: {e} //"), LineKind::Error),
            }
        }
    }

    fn has(s: &Session, name: &str) -> bool {
        s.commands.find(name).is_some()
    }

    fn item(&mut self, ui: &mut egui::Ui, s: &mut Session, label: &str, cmd: &str, call: Value) {
        if ui.add_enabled(Self::has(s, cmd), egui::Button::new(label)).clicked() {
            self.exec(s, cmd, call);
            ui.close();
        }
    }

    // -----------------------------------------------------------------------------------------------------------------

    pub fn build(&mut self, root: &mut egui::Ui, s: &mut Session, vp: &mut Viewport) {
        let ctx = root.ctx().clone();
        if let Some(t) = self.pending_tool.take() {
            vp.tool = t;
        }
        self.menus(root, s, vp);
        self.toolbox(root, s, vp);
        self.bottom(root, s);
        if self.show_outliner {
            self.outliner(root, s);
        }
        if self.show_channel_box {
            self.channel_box(root, s);
        }
        if self.show_script_editor {
            self.script_editor(&ctx, s);
        }
        self.file_dialog(&ctx, s);
        self.viewport(root, s, vp);
        self.keys(&ctx, s, vp);
    }

    fn menus(&mut self, root: &mut egui::Ui, s: &mut Session, vp: &mut Viewport) {
        egui::Panel::top("menus").show(root, |ui| {
            egui::MenuBar::new().ui(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("New Scene").clicked() {
                        self.exec(s, "newScene", json!({}));
                        self.current_file = None;
                        ui.close();
                    }
                    if ui.button("Open Scene...").clicked() {
                        self.file_dialog = Some(FileDialog { kind: FileKind::Open, path: self.current_file.clone().unwrap_or_default() });
                        ui.close();
                    }
                    if ui.button("Save Scene").clicked() {
                        match self.current_file.clone() {
                            Some(p) => self.save(s, &p),
                            None => self.file_dialog = Some(FileDialog { kind: FileKind::SaveAs, path: "scene.aztek".into() }),
                        }
                        ui.close();
                    }
                    if ui.button("Save Scene As...").clicked() {
                        self.file_dialog = Some(FileDialog { kind: FileKind::SaveAs, path: self.current_file.clone().unwrap_or("scene.aztek".into()) });
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Import OBJ...").clicked() {
                        self.file_dialog = Some(FileDialog { kind: FileKind::ImportObj, path: String::new() });
                        ui.close();
                    }
                    if ui.button("Export Selection to OBJ...").clicked() {
                        self.file_dialog = Some(FileDialog { kind: FileKind::ExportObj, path: "export.obj".into() });
                        ui.close();
                    }
                    ui.separator();
                    if ui.button("Exit").clicked() {
                        self.quit = true;
                    }
                });
                ui.menu_button("Edit", |ui| {
                    self.item(ui, s, "Undo  (Z)", "undo", json!({}));
                    self.item(ui, s, "Redo  (Shift+Z)", "redo", json!({}));
                    ui.separator();
                    self.item(ui, s, "Delete", "delete", json!({}));
                    ui.menu_button("Delete by Type", |ui| {
                        self.item(ui, s, "History", "delete", json!({"constructionHistory": true}));
                    });
                    ui.separator();
                    self.item(ui, s, "Duplicate  (Ctrl+D)", "duplicate", json!({}));
                    self.item(ui, s, "Group  (Ctrl+G)", "group", json!({}));
                    self.item(ui, s, "Ungroup", "ungroup", json!({}));
                    self.item(ui, s, "Parent  (P)", "parent", json!({}));
                    ui.separator();
                    if ui.button("Select All").clicked() {
                        let all: Vec<String> = s.graph.roots().iter().filter(|&&r| is_shown_in_outliner(s, r)).map(|&r| s.graph.path(r)).collect();
                        self.exec(s, "select", json!({"args": all}));
                        ui.close();
                    }
                    self.item(ui, s, "Select Hierarchy", "selectHierarchy", json!({}));
                    self.item(ui, s, "Invert Selection", "invertSelection", json!({}));
                });
                ui.menu_button("Create", |ui| {
                    ui.menu_button("Polygon Primitives", |ui| {
                        for (label, cmd) in [
                            ("Sphere", "polySphere"),
                            ("Cube", "polyCube"),
                            ("Cylinder", "polyCylinder"),
                            ("Cone", "polyCone"),
                            ("Torus", "polyTorus"),
                            ("Plane", "polyPlane"),
                            ("Disc", "polyDisc"),
                            ("Pipe", "polyPipe"),
                            ("Capsule", "polyCapsule"),
                            ("Platonic (Ico)", "polyIcoSphere"),
                            ("Quad Sphere", "polyQuadSphere"),
                        ] {
                            self.item(ui, s, label, cmd, json!({}));
                        }
                    });
                    ui.separator();
                    self.item(ui, s, "Locator", "spaceLocator", json!({}));
                    self.item(ui, s, "Empty Group", "group", json!({"empty": true}));
                });
                ui.menu_button("Modify", |ui| {
                    self.item(ui, s, "Freeze Transformations", "freezeTransforms", json!({}));
                    self.item(ui, s, "Reset Transformations", "resetTransforms", json!({}));
                    ui.separator();
                    self.item(ui, s, "Center Pivot", "centerPivot", json!({}));
                    self.item(ui, s, "Bake Pivot", "bakePivot", json!({}));
                });
                ui.menu_button("Display", |ui| {
                    if ui.checkbox(&mut vp.wireframe, "Wireframe  (4)").clicked() {
                        ui.close();
                    }
                    if ui.checkbox(&mut vp.wire_on_shaded, "Wireframe on Shaded").clicked() {
                        ui.close();
                    }
                    ui.separator();
                    self.item(ui, s, "Hide Selection  (Ctrl+H)", "hide", json!({}));
                    self.item(ui, s, "Show Last Hidden", "showLastHidden", json!({}));
                });
                ui.menu_button("Mesh", |ui| {
                    ui.menu_button("Booleans", |ui| {
                        for op in ["union", "difference", "intersection"] {
                            self.item(ui, s, &title(op), "polyBoolean", json!({"operation": op}));
                        }
                    });
                    self.item(ui, s, "Combine", "polyCombine", json!({}));
                    self.item(ui, s, "Separate", "polySeparate", json!({}));
                    self.item(ui, s, "Extract", "polyExtract", json!({}));
                    ui.separator();
                    self.item(ui, s, "Fill Hole", "polyFillHole", json!({}));
                    self.item(ui, s, "Reduce", "polyReduce", json!({}));
                    self.item(ui, s, "Smooth", "polySubdivide", json!({}));
                    self.item(ui, s, "Triangulate", "polyTriangulate", json!({}));
                    self.item(ui, s, "Quadrangulate", "polyQuadrangulate", json!({}));
                    ui.separator();
                    self.item(ui, s, "Mirror", "polyMirror", json!({}));
                    self.item(ui, s, "Symmetrize", "symmetrize", json!({}));
                });
                ui.menu_button("Edit Mesh", |ui| {
                    self.item(ui, s, "Extrude  (Ctrl+E)", "polyExtrude", json!({}));
                    self.item(ui, s, "Bridge", "polyBridge", json!({}));
                    self.item(ui, s, "Bevel  (Ctrl+B)", "polyBevel", json!({}));
                    self.item(ui, s, "Inset", "polyInset", json!({}));
                    ui.separator();
                    self.item(ui, s, "Merge", "polyMerge", json!({}));
                    self.item(ui, s, "Merge to Center", "polyMergeCenter", json!({}));
                    self.item(ui, s, "Collapse", "polyCollapse", json!({}));
                    self.item(ui, s, "Target Weld", "polyTargetWeld", json!({}));
                    ui.separator();
                    self.item(ui, s, "Insert Edge Loop", "polyInsertLoop", json!({}));
                    self.item(ui, s, "Offset Edge Loop", "polyOffsetLoop", json!({}));
                    self.item(ui, s, "Connect", "polyConnect", json!({}));
                    self.item(ui, s, "Poke", "polyPoke", json!({}));
                    self.item(ui, s, "Spin Edge", "polySpinEdge", json!({}));
                    ui.separator();
                    self.item(ui, s, "Delete Edge/Vertex", "polyDissolve", json!({}));
                    self.item(ui, s, "Duplicate Faces", "polyDuplicateFaces", json!({}));
                    self.item(ui, s, "Grid Fill", "polyGridFill", json!({}));
                });
                ui.menu_button("Mesh Display", |ui| {
                    self.item(ui, s, "Soften Edge", "polySoftEdge", json!({"angle": 180.0}));
                    self.item(ui, s, "Harden Edge", "polySoftEdge", json!({"angle": 0.0}));
                    ui.separator();
                    self.item(ui, s, "Reverse", "polyFlip", json!({}));
                    self.item(ui, s, "Conform", "polyConform", json!({}));
                    ui.separator();
                    self.item(ui, s, "Lock Normals", "polyLockNormals", json!({}));
                    self.item(ui, s, "Unlock Normals", "polyUnlockNormals", json!({}));
                });
                ui.menu_button("Select", |ui| {
                    self.item(ui, s, "Grow  (>)", "growSelection", json!({}));
                    self.item(ui, s, "Shrink  (<)", "shrinkSelection", json!({}));
                    self.item(ui, s, "Edge Loop", "selectLoop", json!({}));
                    self.item(ui, s, "Border", "selectBorder", json!({}));
                    self.item(ui, s, "Select All Components", "selectAllComponents", json!({}));
                    ui.separator();
                    self.item(ui, s, "Convert to Vertices", "convertSelection", json!({"args": ["vertex"]}));
                    self.item(ui, s, "Convert to Edges", "convertSelection", json!({"args": ["edge"]}));
                    self.item(ui, s, "Convert to Faces", "convertSelection", json!({"args": ["face"]}));
                });
                ui.menu_button("Windows", |ui| {
                    ui.checkbox(&mut self.show_outliner, "Outliner");
                    ui.checkbox(&mut self.show_channel_box, "Channel Box");
                    ui.checkbox(&mut self.show_script_editor, "Script Editor");
                });
            });
        });
    }

    fn toolbox(&mut self, root: &mut egui::Ui, s: &mut Session, vp: &mut Viewport) {
        egui::Panel::left("toolbox").resizable(false).exact_size(44.0).show(root, |ui| {
            ui.add_space(4.0);
            for (t, label, tip) in [(Tool::Select, "Q", "Select (Q)"), (Tool::Move, "W", "Move (W)"), (Tool::Rotate, "E", "Rotate (E)"), (Tool::Scale, "R", "Scale (R)")] {
                if ui.add_sized([34.0, 30.0], egui::Button::selectable(vp.tool == t, label)).on_hover_text(tip).clicked() {
                    vp.tool = t;
                }
            }
            ui.separator();
            let mode = components::mode(&s.graph);
            for (m, label, tip) in [
                (ComponentMode::Object, "Obj", "Object mode (F8)"),
                (ComponentMode::Vertex, "Vtx", "Vertex (F9)"),
                (ComponentMode::Edge, "Edg", "Edge (F10)"),
                (ComponentMode::Face, "Fce", "Face (F11)"),
            ] {
                if ui.add_sized([34.0, 26.0], egui::Button::selectable(mode == m, label)).on_hover_text(tip).clicked() {
                    self.set_mode(s, m);
                }
            }
        });
    }

    fn set_mode(&mut self, s: &mut Session, m: ComponentMode) {
        let name = match m {
            ComponentMode::Object => "object",
            ComponentMode::Vertex => "vertex",
            ComponentMode::Edge => "edge",
            ComponentMode::Face => "face",
        };
        self.exec(s, "selectMode", json!({"args": [name]}));
    }

    fn bottom(&mut self, root: &mut egui::Ui, s: &mut Session) {
        egui::Panel::bottom("command_line").show(root, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("MEL").monospace().strong());
                let r = ui.add(egui::TextEdit::singleline(&mut self.command_line).desired_width(ui.available_width() * 0.5).font(egui::TextStyle::Monospace));
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    let src = std::mem::take(&mut self.command_line);
                    self.run_source(s, &src);
                    r.request_focus();
                }
                let color = match self.history.last().map(|l| l.kind) {
                    Some(LineKind::Error) => Color32::from_rgb(255, 120, 120),
                    _ => ui.visuals().text_color(),
                };
                ui.label(RichText::new(&self.feedback).monospace().color(color));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Script Editor").clicked() {
                        self.show_script_editor = !self.show_script_editor;
                    }
                });
            });
        });
    }

    fn outliner(&mut self, root: &mut egui::Ui, s: &mut Session) {
        egui::Panel::left("outliner").default_size(200.0).show(root, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Outliner");
                ui.checkbox(&mut self.show_shapes, "Shapes");
            });
            ui.separator();
            egui::ScrollArea::vertical().show(ui, |ui| {
                let roots = s.graph.roots().to_vec();
                for r in roots {
                    if is_shown_in_outliner(s, r) {
                        self.outliner_node(ui, s, r);
                    }
                }
            });
        });
    }

    fn outliner_node(&mut self, ui: &mut egui::Ui, s: &mut Session, id: NodeId) {
        let g = &s.graph;
        let Some(n) = g.node(id) else { return };
        let name = n.name().to_string();
        let selected = g.selection().contains(&id);
        let kids: Vec<NodeId> = g
            .children(id)
            .iter()
            .copied()
            .filter(|&c| {
                g.node(c).is_some_and(|cn| {
                    let shape = cn.ty().is_a("shape");
                    (!shape || self.show_shapes) && !(shape && cn.bool(DagNode::INTERMEDIATE_OBJECT) && !self.show_shapes)
                })
            })
            .collect();
        let hidden = n.ty().dag && !n.bool(DagNode::VISIBILITY);
        let mut text = RichText::new(&name);
        if hidden {
            text = text.color(Color32::GRAY);
        }
        let path = g.path(id);
        let clicked = |ui: &mut egui::Ui| ui.add(egui::Button::selectable(selected, text.clone()).frame(false)).clicked();
        if kids.is_empty() {
            ui.horizontal(|ui| {
                ui.add_space(18.0);
                if clicked(ui) {
                    self.outliner_click(ui, s, &path);
                }
            });
        } else {
            let state_id = ui.make_persistent_id(("outliner", id));
            egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), state_id, false)
                .show_header(ui, |ui| {
                    if clicked(ui) {
                        self.outliner_click(ui, s, &path);
                    }
                })
                .body(|ui| {
                    for k in kids {
                        self.outliner_node(ui, s, k);
                    }
                });
        }
    }

    fn outliner_click(&mut self, ui: &egui::Ui, s: &mut Session, path: &str) {
        let m = ui.input(|i| i.modifiers);
        let call = if m.ctrl || m.command {
            json!({"args": [path], "deselect": true})
        } else if m.shift {
            json!({"args": [path], "toggle": true})
        } else {
            json!({"args": [path]})
        };
        self.exec(s, "select", call);
    }

    fn channel_box(&mut self, root: &mut egui::Ui, s: &mut Session) {
        egui::Panel::right("channel_box").default_size(250.0).show(root, |ui| {
            ui.heading("Channel Box");
            ui.separator();
            let lead = s.graph.selection().last().copied();
            let Some(lead) = lead else {
                ui.label("Nothing selected.");
                return;
            };
            egui::ScrollArea::vertical().show(ui, |ui| {
                let g = &s.graph;
                let Some(n) = g.node(lead) else { return };
                ui.label(RichText::new(n.name()).strong());
                let is_xf = n.ty().is_a("transform");
                let path = g.path(lead);
                if is_xf {
                    egui::Grid::new("cb_xf").num_columns(2).striped(true).show(ui, |ui| {
                        for (attr, label, idx) in [
                            ("translate", "Translate", Transform::TRANSLATE),
                            ("rotate", "Rotate", Transform::ROTATE),
                            ("scale", "Scale", Transform::SCALE),
                        ] {
                            let v = s.graph.node(lead).unwrap().vec3(idx);
                            for (k, ax) in ["X", "Y", "Z"].iter().enumerate() {
                                ui.label(format!("{label} {ax}"));
                                let mut x = v[k];
                                let r = ui.add(egui::DragValue::new(&mut x).speed(if attr == "scale" { 0.01 } else { 0.1 }).max_decimals(3));
                                self.cb_edit(s, &r, &format!("{path}.{attr}{ax}"), json!(x));
                                ui.end_row();
                            }
                        }
                        ui.label("Visibility");
                        let mut vis = s.graph.node(lead).unwrap().bool(DagNode::VISIBILITY);
                        let r = ui.checkbox(&mut vis, "");
                        if r.changed() {
                            self.exec(s, "setAttr", json!({"args": [format!("{path}.visibility"), vis]}));
                        }
                        ui.end_row();
                    });
                }
                // Shapes and their history.
                let g = &s.graph;
                let shapes: Vec<NodeId> = if is_xf { aztek_scene::dag::shapes(g, lead, false) } else { vec![lead] };
                if !shapes.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("SHAPES").small().strong());
                    for &sh in &shapes {
                        ui.label(s.graph.node(sh).unwrap().name());
                    }
                }
                let mut inputs = Vec::new();
                for &sh in &shapes {
                    if s.graph.node(sh).is_some_and(|n| n.ty().is_a("mesh")) {
                        inputs.extend(aztek_scene::history::history_of(&s.graph, sh));
                    }
                }
                if !inputs.is_empty() {
                    ui.separator();
                    ui.label(RichText::new("INPUTS").small().strong());
                    for h in inputs {
                        self.node_attrs(ui, s, h);
                    }
                }
            });
        });
    }

    /// One collapsible node in the channel box's INPUTS: its editable settings.
    fn node_attrs(&mut self, ui: &mut egui::Ui, s: &mut Session, id: NodeId) {
        let g = &s.graph;
        let Some(n) = g.node(id) else { return };
        if n.ty().dag {
            ui.label(n.name());
            return;
        }
        let name = n.name().to_string();
        let path = g.path(id);
        let attrs: Vec<(usize, String, PlugType)> = n
            .ty()
            .attrs
            .iter()
            .enumerate()
            .filter(|(_, a)| a.flags & (flags::HIDDEN | flags::OUTPUT) == 0)
            .filter(|(_, a)| matches!(a.ty, PlugType::Float | PlugType::Int | PlugType::Bool | PlugType::String | PlugType::Vec3 | PlugType::Enum))
            .map(|(i, a)| (i, a.name.clone(), a.ty))
            .collect();
        egui::CollapsingHeader::new(&name).id_salt(("inputs", id)).default_open(true).show(ui, |ui| {
            egui::Grid::new(("cb_in", id)).num_columns(2).striped(true).show(ui, |ui| {
                for (i, attr, ty) in attrs {
                    let plug = format!("{path}.{attr}");
                    let v = s.graph.node(id).unwrap().value(i).clone();
                    match (ty, v) {
                        (PlugType::Float, PlugValue::Float(mut x)) => {
                            ui.label(nice(&attr));
                            let r = ui.add(egui::DragValue::new(&mut x).speed(0.05).max_decimals(3));
                            self.cb_edit(s, &r, &plug, json!(x));
                        }
                        (PlugType::Int | PlugType::Enum, PlugValue::Int(mut x)) => {
                            ui.label(nice(&attr));
                            let r = ui.add(egui::DragValue::new(&mut x).speed(0.1));
                            self.cb_edit(s, &r, &plug, json!(x));
                        }
                        (PlugType::Bool, PlugValue::Bool(mut x)) => {
                            ui.label(nice(&attr));
                            if ui.checkbox(&mut x, "").changed() {
                                self.exec(s, "setAttr", json!({"args": [plug, x]}));
                            }
                        }
                        (PlugType::String, PlugValue::String(mut x)) => {
                            ui.label(nice(&attr));
                            let r = ui.text_edit_singleline(&mut x);
                            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                                self.exec(s, "setAttr", json!({"args": [plug, x], "type": "string"}));
                            }
                        }
                        (PlugType::Vec3, PlugValue::Vec3(v)) => {
                            for (k, ax) in ["X", "Y", "Z"].iter().enumerate() {
                                ui.label(format!("{} {ax}", nice(&attr)));
                                let mut x = v[k];
                                let r = ui.add(egui::DragValue::new(&mut x).speed(0.05).max_decimals(3));
                                self.cb_edit(s, &r, &format!("{plug}{ax}"), json!(x));
                                ui.end_row();
                            }
                            continue;
                        }
                        _ => continue,
                    }
                    ui.end_row();
                }
            });
        });
    }

    /// Applies a channel box field edit: a drag replaces its own previous step so it stays one undo.
    fn cb_edit(&mut self, s: &mut Session, r: &egui::Response, plug: &str, v: Value) {
        if r.changed() {
            let dragging = r.dragged();
            if dragging && self.cb_drag.as_deref() == Some(plug) {
                s.undo();
                self.history.pop();
            }
            self.exec(s, "setAttr", json!({"args": [plug, v]}));
            self.cb_drag = dragging.then(|| plug.to_string());
        }
        if r.drag_stopped() {
            self.cb_drag = None;
        }
    }

    fn script_editor(&mut self, ctx: &egui::Context, s: &mut Session) {
        let mut open = self.show_script_editor;
        egui::Window::new("Script Editor").open(&mut open).default_size([640.0, 420.0]).show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Execute  (Ctrl+Enter)").clicked() {
                    let src = self.script.clone();
                    self.run_source(s, &src);
                }
                if ui.button("Clear History").clicked() {
                    self.history.clear();
                }
                ui.label(RichText::new("MEL").small());
            });
            let h = ui.available_height();
            egui::ScrollArea::vertical().id_salt("se_hist").max_height(h * 0.55).stick_to_bottom(true).show(ui, |ui| {
                for l in &self.history {
                    let c = match l.kind {
                        LineKind::Error => Color32::from_rgb(255, 110, 110),
                        LineKind::Result => Color32::from_rgb(160, 210, 255),
                        LineKind::Info => Color32::GRAY,
                        LineKind::Echo => ui.visuals().text_color(),
                    };
                    ui.label(RichText::new(&l.text).monospace().color(c));
                }
            });
            ui.separator();
            let r = egui::ScrollArea::vertical()
                .id_salt("se_in")
                .show(ui, |ui| ui.add(egui::TextEdit::multiline(&mut self.script).code_editor().desired_width(f32::INFINITY).desired_rows(8)))
                .inner;
            if r.has_focus() && ui.input(|i| i.key_pressed(Key::Enter) && (i.modifiers.ctrl || i.modifiers.command)) {
                let src = self.script.clone();
                self.run_source(s, &src);
            }
        });
        self.show_script_editor = open;
    }

    fn file_dialog(&mut self, ctx: &egui::Context, s: &mut Session) {
        let Some(d) = &mut self.file_dialog else { return };
        let title = match d.kind {
            FileKind::Open => "Open Scene",
            FileKind::SaveAs => "Save Scene As",
            FileKind::ImportObj => "Import OBJ",
            FileKind::ExportObj => "Export Selection to OBJ",
        };
        let mut go = false;
        let mut cancel = false;
        egui::Window::new(title).collapsible(false).resizable(false).show(ctx, |ui| {
            ui.label("Path:");
            let r = ui.add(egui::TextEdit::singleline(&mut d.path).desired_width(420.0));
            if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                go = true;
            }
            ui.horizontal(|ui| {
                go |= ui.button("OK").clicked();
                cancel = ui.button("Cancel").clicked();
            });
        });
        if cancel {
            self.file_dialog = None;
        } else if go {
            let d = self.file_dialog.take().unwrap();
            match d.kind {
                FileKind::Open => self.open_path(s, &d.path),
                FileKind::SaveAs => self.save(s, &d.path),
                FileKind::ImportObj => {
                    self.exec(s, "importObj", json!({"args": [d.path]}));
                }
                FileKind::ExportObj => {
                    self.exec(s, "exportObj", json!({"args": [d.path]}));
                }
            }
        }
    }

    fn save(&mut self, s: &mut Session, path: &str) {
        s.graph.evaluate();
        let doc = aztek_scene::scene_file::serialize(&s.graph, false);
        match serde_json::to_string_pretty(&doc).map_err(|e| e.to_string()).and_then(|t| std::fs::write(path, t).map_err(|e| e.to_string())) {
            Ok(()) => {
                self.current_file = Some(path.to_string());
                self.log(format!("// Saved {path} //"), LineKind::Info);
            }
            Err(e) => self.log(format!("// Error: saving {path}: {e} //"), LineKind::Error),
        }
    }

    pub fn open_path(&mut self, s: &mut Session, path: &str) {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                self.log(format!("// Error: {path}: {e} //"), LineKind::Error);
                return;
            }
        };
        let doc: Value = match serde_json::from_str(&text) {
            Ok(d) => d,
            Err(e) => {
                self.log(format!("// Error: {path}: {e} //"), LineKind::Error);
                return;
            }
        };
        let _ = execute(s, "newScene", &json!({}), CallSource::Script);
        match aztek_scene::scene_file::deserialize(&mut s.graph, &doc) {
            Ok(warnings) => {
                for w in warnings {
                    self.log(format!("// Warning: {w} //"), LineKind::Info);
                }
                let _ = execute(s, "flushUndo", &json!({}), CallSource::Script);
                self.current_file = Some(path.to_string());
                self.log(format!("// Opened {path} //"), LineKind::Info);
            }
            Err(e) => self.log(format!("// Error: {path}: {e} //"), LineKind::Error),
        }
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Viewport

    fn viewport(&mut self, root: &mut egui::Ui, s: &mut Session, vp: &mut Viewport) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        egui::CentralPanel::no_frame().show(root, |ui| {
            let rect = ui.max_rect();
            let ppp = ctx.pixels_per_point() as f64;
            vp.rect = [rect.min.x as f64 * ppp, rect.min.y as f64 * ppp, rect.width() as f64 * ppp, rect.height() as f64 * ppp];
            let resp = ui.interact(rect, ui.id().with("viewport"), egui::Sense::click_and_drag());
            let items = gather(&mut s.graph);
            let to_px = |p: egui::Pos2| DVec2::new((p.x - rect.min.x) as f64 * ppp, (p.y - rect.min.y) as f64 * ppp);
            let (pos, mods, down) = ui.input(|i| {
                (
                    i.pointer.interact_pos().or(i.pointer.hover_pos()),
                    i.modifiers,
                    [i.pointer.primary_down(), i.pointer.middle_down(), i.pointer.secondary_down()],
                )
            });
            let pressed = |b: PointerButton| ui.input(|i| i.pointer.button_pressed(b));
            let released = |b: PointerButton| ui.input(|i| i.pointer.button_released(b));
            let over = resp.hovered() || resp.dragged() || self.nav.is_some() || vp.drag.is_some() || self.marquee_start.is_some();

            // Navigation: Alt with left (tumble), middle (track), right (dolly).
            if let Some(p) = pos.filter(|_| over) {
                let px = to_px(p);
                if mods.alt && self.nav.is_none() {
                    for b in [PointerButton::Primary, PointerButton::Middle, PointerButton::Secondary] {
                        if pressed(b) && resp.hovered() {
                            self.nav = Some((b, px));
                        }
                    }
                }
                if let Some((b, last)) = self.nav {
                    let d = px - last;
                    match b {
                        PointerButton::Primary => vp.cam.tumble(d / ppp),
                        PointerButton::Middle => {
                            let aspect = vp.aspect();
                            vp.cam.track(d, vp.rect[3], aspect)
                        }
                        _ => vp.cam.dolly((d.x + d.y) / ppp),
                    }
                    self.nav = Some((b, px));
                    let still = match b {
                        PointerButton::Primary => down[0],
                        PointerButton::Middle => down[1],
                        _ => down[2],
                    };
                    if !still {
                        self.nav = None;
                    }
                } else {
                    let scroll = ui.input(|i| i.smooth_scroll_delta.y) as f64;
                    if scroll != 0.0 && resp.hovered() {
                        vp.cam.dolly(scroll * 0.5);
                    }
                    // Manipulator.
                    let scale = ppp;
                    let pivot = vp.manip_pivot(s, &items);
                    if vp.drag.is_none() {
                        vp.hover = pivot.and_then(|pv| vp.manip_hit(pv, px, scale));
                    }
                    let mpress = pressed(PointerButton::Primary) || (pressed(PointerButton::Middle) && pivot.is_some());
                    if mpress && resp.hovered() && vp.drag.is_none() && !mods.alt {
                        let handle = vp.hover.or_else(|| {
                            // Middle drag anywhere moves along the last-used axis; plain view-plane here.
                            (pressed(PointerButton::Middle) && pivot.is_some()).then_some(crate::viewport::Handle::Center)
                        });
                        if let (Some(h), Some(pv)) = (handle, pivot) {
                            let comps = components::mode(&s.graph) != ComponentMode::Object;
                            let targets = s.graph.selection().iter().map(|&id| s.graph.path(id)).collect();
                            vp.drag = Some(Drag { handle: h, start: px, pivot: pv, applied: false, components: comps, targets });
                        } else if pressed(PointerButton::Primary) {
                            self.marquee_start = Some(px);
                        }
                    }
                    if let Some(d) = &vp.drag {
                        if px != d.start {
                            if let Some((cmd, call)) = vp.drag_call(d, px, scale) {
                                if d.applied {
                                    s.undo();
                                    self.history.pop();
                                }
                                let ok = self.exec(s, &cmd, call).is_some();
                                vp.drag.as_mut().unwrap().applied = ok;
                            }
                        }
                        if !down[0] && !down[1] {
                            vp.drag = None;
                        }
                    }
                    if let Some(start) = self.marquee_start {
                        vp.marquee = Some((start, px));
                        if released(PointerButton::Primary) || !down[0] {
                            self.marquee_start = None;
                            vp.marquee = None;
                            let picked = if (px - start).length() < 4.0 * ppp {
                                vp.pick(s, &items, px)
                            } else {
                                vp.pick_rect(s, &items, start, px)
                            };
                            self.select_picked(s, picked, mods);
                        }
                    }
                }
                // Right click: the selection-mode marking menu.
                if resp.secondary_clicked() && !mods.alt {
                    self.rmb_menu = Some(p);
                }
            }
            if let Some((a, b)) = vp.marquee {
                let to_pt = |v: DVec2| egui::pos2(rect.min.x + (v.x / ppp) as f32, rect.min.y + (v.y / ppp) as f32);
                ui.painter().rect_stroke(
                    egui::Rect::from_two_pos(to_pt(a), to_pt(b)),
                    0.0,
                    egui::Stroke::new(1.0, Color32::WHITE),
                    egui::StrokeKind::Middle,
                );
            }
            // HUD.
            let mode = components::mode(&s.graph);
            let hud = format!(
                "{}   {}   {} objects",
                match mode {
                    ComponentMode::Object => "Object",
                    ComponentMode::Vertex => "Vertex",
                    ComponentMode::Edge => "Edge",
                    ComponentMode::Face => "Face",
                },
                format!("{:?}", vp.tool),
                items.len()
            );
            ui.painter().text(rect.left_top() + egui::vec2(8.0, 6.0), egui::Align2::LEFT_TOP, hud, egui::FontId::monospace(12.0), Color32::from_gray(220));
            ui.painter().text(rect.left_bottom() + egui::vec2(8.0, -6.0), egui::Align2::LEFT_BOTTOM, "persp", egui::FontId::proportional(13.0), Color32::from_gray(220));
        });
        if let Some(p) = self.rmb_menu {
            let mut close = false;
            egui::Area::new(egui::Id::new("rmb_menu")).fixed_pos(p).order(egui::Order::Foreground).show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    for (label, m) in [
                        ("Vertex", ComponentMode::Vertex),
                        ("Edge", ComponentMode::Edge),
                        ("Face", ComponentMode::Face),
                        ("Object Mode", ComponentMode::Object),
                    ] {
                        if ui.button(label).clicked() {
                            self.set_mode(s, m);
                            close = true;
                        }
                    }
                });
            });
            if close || ctx.input(|i| i.pointer.primary_clicked() || i.key_pressed(Key::Escape)) && !close {
                self.rmb_menu = None;
            }
        }
    }

    fn select_picked(&mut self, s: &mut Session, picked: Vec<String>, mods: Modifiers) {
        let comps = components::mode(&s.graph) != ComponentMode::Object;
        if picked.is_empty() {
            if !mods.shift && !mods.ctrl {
                if comps {
                    // Clicking empty space in a component mode clears the components, keeping the objects hilited.
                    let objs: Vec<String> = s.graph.selection().iter().map(|&id| s.graph.path(id)).collect();
                    self.exec(s, "select", json!({"args": objs}));
                    let m = components::mode(&s.graph);
                    let _ = m;
                    components::set_all(&mut s.graph, Default::default(), Default::default(), Default::default());
                } else {
                    self.exec(s, "select", json!({"clear": true}));
                }
            }
            return;
        }
        let call = if mods.ctrl && mods.shift {
            json!({"args": picked, "add": true})
        } else if mods.ctrl {
            json!({"args": picked, "deselect": true})
        } else if mods.shift {
            json!({"args": picked, "toggle": true})
        } else {
            json!({"args": picked})
        };
        self.exec(s, "select", call);
    }

    fn keys(&mut self, ctx: &egui::Context, s: &mut Session, vp: &mut Viewport) {
        if ctx.egui_wants_keyboard_input() {
            return;
        }
        let ev: Vec<(Key, Modifiers)> = ctx.input(|i| {
            i.events
                .iter()
                .filter_map(|e| match e {
                    egui::Event::Key { key, pressed: true, modifiers, repeat: false, .. } => Some((*key, *modifiers)),
                    _ => None,
                })
                .collect()
        });
        let items: Vec<DrawItem> = gather(&mut s.graph);
        for (k, m) in ev {
            let ctrl = m.ctrl || m.command;
            match k {
                Key::Z if ctrl && m.shift => {
                    self.exec(s, "redo", json!({}));
                }
                Key::Z if ctrl => {
                    self.exec(s, "undo", json!({}));
                }
                Key::Z if m.shift => {
                    self.exec(s, "redo", json!({}));
                }
                Key::Z => {
                    self.exec(s, "undo", json!({}));
                }
                Key::Y if ctrl => {
                    self.exec(s, "redo", json!({}));
                }
                Key::Q => vp.tool = Tool::Select,
                Key::W => vp.tool = Tool::Move,
                Key::E if ctrl => {
                    self.exec(s, "polyExtrude", json!({}));
                }
                Key::E => vp.tool = Tool::Rotate,
                Key::R => vp.tool = Tool::Scale,
                Key::F => vp.frame(s, &items, false),
                Key::A => vp.frame(s, &items, true),
                Key::Num4 => vp.wireframe = true,
                Key::Num5 => vp.wireframe = false,
                Key::F8 => {
                    let next = if components::mode(&s.graph) == ComponentMode::Object { ComponentMode::Vertex } else { ComponentMode::Object };
                    self.set_mode(s, next);
                }
                Key::F9 => self.set_mode(s, ComponentMode::Vertex),
                Key::F10 => self.set_mode(s, ComponentMode::Edge),
                Key::F11 => self.set_mode(s, ComponentMode::Face),
                Key::Delete | Key::Backspace => {
                    if components::mode(&s.graph) == ComponentMode::Object {
                        self.exec(s, "delete", json!({}));
                    } else {
                        self.exec(s, "deleteComponents", json!({}));
                    }
                }
                Key::D if ctrl => {
                    self.exec(s, "duplicate", json!({}));
                }
                Key::G if ctrl => {
                    self.exec(s, "group", json!({}));
                }
                Key::H if ctrl => {
                    self.exec(s, "hide", json!({}));
                }
                Key::B if ctrl => {
                    self.exec(s, "polyBevel", json!({}));
                }
                Key::P if m.shift => {
                    self.exec(s, "parent", json!({"world": true}));
                }
                Key::P => {
                    self.exec(s, "parent", json!({}));
                }
                Key::Period if m.shift => {
                    self.exec(s, "growSelection", json!({}));
                }
                Key::Comma if m.shift => {
                    self.exec(s, "shrinkSelection", json!({}));
                }
                Key::Escape => {
                    self.rmb_menu = None;
                }
                _ => {}
            }
        }
    }
}

fn is_shown_in_outliner(s: &Session, id: NodeId) -> bool {
    s.graph.node(id).is_some_and(|n| n.ty().dag) && s.graph.parent(id) == NO_NODE
}

fn title(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

/// camelCase to "Camel Case".
fn nice(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else if ch.is_uppercase() {
            out.push(' ');
            out.push(ch);
        } else {
            out.push(ch);
        }
    }
    out
}

#[allow(dead_code)]
fn plug_of(id: NodeId, attr: usize) -> PlugRef {
    PlugRef::new(id, attr as i32)
}
