//! The interactive modelling tools: Multi-Cut, Insert Edge Loop, Offset Edge Loop, Target Weld, Slide Edge, Crease, Create
//! Polygon, Append to Polygon, Quad Draw. Each turns mouse events into the commands behind it, with a preview drawn by the
//! viewport from `tool_lines` and `tool_points`.

use aztek_geo::{Index, Mesh};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::plug::NodeId;
use glam::{DVec2, DVec3};
use serde_json::{json, Value};

use crate::core::Core;
use crate::view::{View, CTRL, SHIFT};
use crate::viewport::{DrawItem, Tool};

const GREEN: [u8; 4] = [70, 255, 120, 255];
const WHITE: [u8; 4] = [255, 255, 255, 255];
const RED: [u8; 4] = [255, 60, 60, 255];

/// What's under a pixel on a mesh.
#[derive(Clone, Debug)]
pub struct Snap {
    pub item: usize,
    pub face: Index,
    pub world: DVec3,
    /// Nearest vertex of the face: id, world place, pixels away.
    pub vertex: (Index, DVec3, f64),
    /// Nearest edge of the face: id, fraction from its first point, world place, pixels away.
    pub edge: (Index, f64, DVec3, f64),
}

fn seg_t(p: DVec2, a: DVec2, b: DVec2) -> (f64, f64) {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    (t, (p - (a + ab * t)).length())
}

#[derive(Default)]
pub struct ToolState {
    /// Points placed so far: Multi-Cut's, Create Polygon's, Append's.
    pub cut_mesh: Option<NodeId>,
    pub cut_points: Vec<Value>,
    pub cut_world: Vec<DVec3>,
    pub cut_scene: u64,
    /// Press on an edge, its fraction; or a slide's start.
    pub press: Option<DVec2>,
    pub edge: Option<(NodeId, Index, f64)>,
    pub applied: bool,
    pub last_call: Option<(String, Value)>,
    /// Quad Draw: the dots waiting to become faces, world space.
    pub dots: Vec<DVec3>,
    pub quad_mesh: Option<NodeId>,
    pub dragging_dot: Option<usize>,
    pub dragging_vertex: Option<(NodeId, Index)>,
    pub comp_drag: Option<crate::tools_quad::CompDrag>,
    /// A middle drag along a normal with soft selection on: every point's place, the mesh's world, the points it moves.
    pub normal_soft: Option<(Vec<DVec3>, glam::DMat4, Vec<(Index, f32)>)>,
    pub vertex_extrude: Option<crate::tools_quad::VertexExtrude>,
    pub tab: bool,
    pub append_edges: Vec<Index>,
    pub weld_source: Option<(NodeId, Index, bool)>,
    /// 3D Cut and Sew: the mesh and the edges dragged over so far.
    pub cut_sew: Option<(NodeId, Vec<Index>)>,
    /// Quad Draw's and Multi-Cut's options, from the Modeling Toolkit (#140).
    pub qd: crate::tools_quad::QuadOpts,
    pub mc: crate::tools_quad::CutOpts,
    /// Quad Draw: a quad strip being drawn, border edges being extended, a strip width being dragged, a relax.
    pub strip: Option<crate::tools_quad::Strip>,
    pub extend: Option<crate::tools_quad::Extend>,
    pub strip_resize: Option<(DVec2, f64)>,
    pub relaxing: bool,
    /// The way the mouse last moved on screen, and where it was.
    pub last_dir: DVec2,
    pub last_px: Option<DVec2>,
    /// Multi-Cut: where a middle-button slice started.
    pub slice_from: Option<DVec2>,
    /// Quad Draw: a middle drag moving a dot or a vertex along its normal, and a Shift+middle drag relaxing the
    /// picked points, with where it last stepped.
    pub normal_drag: Option<crate::tools_quad::NormalDrag>,
    pub relax_picked: Option<DVec2>,
    /// IK Handle and IK Spline Handle tools: the start joint clicked, then the end joint.
    pub ik_start: Option<NodeId>,
    pub ik_end: Option<NodeId>,
}

/// Screen-space hit on any drawn mesh.
pub fn snap_at(v: &mut View, items: &[DrawItem], px: DVec2) -> Option<Snap> {
    let (i, face, world) = v.vp.raycast(items, px)?;
    let it = &items[i];
    let m: &Mesh = &it.mesh;
    let screen = v.vp.screen();
    let proj = |p: Index| screen.project(it.world.transform_point3(m.position(p)));
    let (mut bv, mut be): (Option<(Index, DVec3, f64)>, Option<(Index, f64, DVec3, f64)>) = (None, None);
    for c in m.face_corners(face) {
        let p = m.corner_point(c);
        if let Some(q) = proj(p) {
            let d = (q - px).length();
            if bv.as_ref().is_none_or(|b| d < b.2) {
                bv = Some((p, it.world.transform_point3(m.position(p)), d));
            }
        }
        let e = m.corner_edge(c);
        let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
        if let (Some(pa), Some(pb)) = (proj(a), proj(b)) {
            let (t, d) = seg_t(px, pa, pb);
            if be.as_ref().is_none_or(|x| d < x.3) {
                let w = it.world.transform_point3(m.position(a).lerp(m.position(b), t));
                be = Some((e, t, w, d));
            }
        }
    }
    Some(Snap { item: i, face, world, vertex: bv?, edge: be? })
}

/// The polyline a new edge loop would follow across an edge's quad ring.
pub fn ring_preview(m: &Mesh, e: Index, t: f64) -> Vec<(DVec3, DVec3)> {
    let mut out = Vec::new();
    let at = |e: Index, t: f64| m.position(m.edge_point(e, 0)).lerp(m.position(m.edge_point(e, 1)), t);
    // Walk one way, then the other, across quads.
    let mut start_corners = Vec::new();
    m.for_each_edge_corner(e, |c| start_corners.push(c));
    for &c0 in &start_corners {
        let mut seen = std::collections::HashSet::new();
        seen.insert(e);
        let mut cur_edge = e;
        let mut cur_t = t;
        let mut c = c0;
        loop {
            let f = m.corner_face(c);
            if m.face_size(f) != 4 {
                break;
            }
            // The opposite edge in the quad.
            let opposite_corner = m.corner_next(m.corner_next(c));
            let oe = m.corner_edge(opposite_corner);
            // Keep the fraction measured along the same side of the ring.
            let here_first = m.corner_point(c) == m.edge_point(cur_edge, 0);
            let along = if here_first { cur_t } else { 1.0 - cur_t };
            let other_first = m.corner_point(opposite_corner) == m.edge_point(oe, 0);
            // The opposite edge runs the other way round the face.
            let ot = if other_first { 1.0 - along } else { along };
            out.push((at(cur_edge, cur_t), at(oe, ot)));
            if !seen.insert(oe) {
                break;
            }
            let mut next = None;
            m.for_each_edge_corner(oe, |x| {
                if x != opposite_corner {
                    next = Some(x);
                }
            });
            let Some(n) = next else { break };
            cur_edge = oe;
            cur_t = ot;
            c = n;
        }
    }
    out
}

/// The ikHandle call the IK Handle Tool (spline: the IK Spline Handle Tool) makes from a start joint to an end joint, with
/// the tool context's settings; `curve` is the one clicked when the spline tool doesn't make its own.
pub fn ik_handle_call(start: &str, end: &str, curve: Option<&str>, spline: bool) -> Value {
    let ctx = if spline { "ikSplineHandleContext" } else { "ikHandleContext" };
    let setting = |k: &str| crate::ctx_cmds::prop(ctx, k);
    let mut call = json!({"startJoint": start, "endEffector": end});
    let number = |v: Value| v.as_f64().or_else(|| v.as_str().and_then(|t| t.trim().parse().ok()));
    let flag = |v: Value| v.as_bool().or_else(|| v.as_str().map(|t| t == "true" || t == "1")).or_else(|| v.as_i64().map(|k| k != 0));
    if let Some(sol) = setting("solverTypeH").as_str().filter(|t| !t.is_empty()) {
        call["solver"] = json!(if spline { "ikSplineSolver" } else { sol });
    }
    if let Some(v) = number(setting("priorityH")) {
        call["priority"] = json!(v as i64);
    }
    for (k, to) in [("weightH", "weight"), ("poWeightH", "positionWeight")] {
        if let Some(v) = number(setting(k)) {
            call[to] = json!(v);
        }
    }
    for (k, to) in [("snapHandleH", "snapHandleFlagToggle"), ("forceSolverH", "forceSolver")] {
        if let Some(v) = flag(setting(k)) {
            call[to] = json!(v);
        }
    }
    if flag(setting("autoPriorityH")) == Some(true) {
        call["autoPriority"] = json!(true);
    }
    if let Some(v) = setting("stickyH").as_str().filter(|t| *t == "off" || *t == "sticky") {
        call["sticky"] = json!(v);
    }
    if spline {
        for k in ["rootOnCurve", "parentCurve", "createCurve", "simplifyCurve", "rootTwistMode"] {
            if let Some(v) = flag(setting(k)) {
                call[k] = json!(v);
            }
        }
        // Auto create root axis can only be on with Root on curve off, as the reference greys it.
        if flag(setting("createRootAxis")) == Some(true) && flag(setting("rootOnCurve")) == Some(false) {
            call["createRootAxis"] = json!(true);
        }
        if let Some(v) = number(setting("numSpans")) {
            call["numSpans"] = json!((v as i64).max(1));
        }
        if let Some(v) = setting("twistType").as_str() {
            call["twistType"] = json!(v);
        }
        if let Some(c) = curve {
            call["curve"] = json!(c);
            call["createCurve"] = json!(false);
            if let Some(v) = flag(setting("snapCurve")) {
                call["snapCurve"] = json!(v);
            }
        }
    }
    call
}

impl View {
    pub(crate) fn tool_preview_clear(&mut self) {
        self.vp.tool_lines.clear();
        self.vp.tool_points.clear();
    }

    pub(crate) fn apply_preview(&mut self, core: &mut Core, cmd: &str, call: Value) {
        if self.ts.applied {
            core.retract_preview();
        }
        let ok = core.exec_silent(cmd, call.clone()).is_ok();
        self.ts.applied = ok;
        self.ts.last_call = ok.then(|| (cmd.to_string(), call));
    }

    pub(crate) fn commit_preview(&mut self, core: &mut Core) {
        if self.ts.applied {
            if let Some((cmd, call)) = self.ts.last_call.take() {
                core.retract_preview();
                let _ = core.exec(&cmd, call);
            }
        }
        self.ts.applied = false;
        self.ts.last_call = None;
    }

    /// A mouse event for the current modelling tool. None when it isn't one of them.
    pub fn tool_mouse(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, items: &[DrawItem]) -> Option<bool> {
        match self.vp.tool {
            Tool::InsertJoint => Some(self.insert_joint(core, kind, button, px)),
            Tool::IkHandle => Some(self.ik_handle_tool(core, kind, button, px, items, false)),
            Tool::IkSplineHandle => Some(self.ik_handle_tool(core, kind, button, px, items, true)),
            Tool::MultiCut => Some(self.multi_cut(core, kind, button, px, mods, items)),
            Tool::InsertLoop => Some(self.insert_loop(core, kind, button, px, items)),
            Tool::OffsetLoop => Some(self.offset_loop(core, kind, button, px, items)),
            Tool::TargetWeld => Some(self.target_weld(core, kind, button, px, items)),
            Tool::SlideEdge => Some(self.slide_drag(core, kind, button, px, mods, items, "polySlideEdge", 0.005)),
            Tool::Crease => Some(self.slide_drag(core, kind, button, px, mods, items, "polyCrease", 0.02)),
            Tool::CreatePolygon => Some(self.create_polygon(core, kind, button, px, items)),
            Tool::AppendPolygon => Some(self.append_polygon(kind, button, px, items)),
            Tool::QuadDraw => Some(self.quad_draw(core, kind, button, px, mods, items)),
            Tool::CutSew => Some(self.cut_sew(core, kind, button, px, mods, items)),
            _ => None,
        }
    }

    /// Enter in a modelling tool.
    pub fn tool_finish(&mut self, core: &mut Core, items: &[DrawItem]) {
        match self.vp.tool {
            Tool::MultiCut => {
                let points = std::mem::take(&mut self.ts.cut_points);
                self.ts.cut_world.clear();
                if let (Some(mesh), false) = (self.ts.cut_mesh.take(), points.is_empty()) {
                    if self.ts.cut_scene != core.s.graph.scene_serial() || !multi_cut_targets(core).contains(&mesh) || !items.iter().any(|it| it.shape == mesh) {
                        self.tool_preview_clear();
                        return;
                    }
                    let name = core.s.graph.path(mesh);
                    let eye = self.vp.cam.eye();
                    let call = json!({"mesh": name, "points": points, "eye": [eye.x, eye.y, eye.z], "select": false});
                    // On a live surface the cut's new points go onto it (#163).
                    let shape = aztek_scene::components::as_mesh(&core.s.graph, mesh).unwrap_or(mesh);
                    let from = crate::tools_quad::point_count(core, shape);
                    let _ = core.exec_silent("undoInfo", json!({"openChunk": true}));
                    let _ = core.exec("polyKnife", call);
                    crate::tools_quad::snap_new_points_to_live(core, shape, from);
                    let _ = core.exec_silent("undoInfo", json!({"closeChunk": true}));
                }
            }
            Tool::CreatePolygon => {
                let pts: Vec<[f64; 3]> = self.ts.cut_world.drain(..).map(|p| p.to_array()).collect();
                if pts.len() >= 3 {
                    // The new polygon, selected in object mode, as the reference leaves it (#124).
                    let _ = core.exec("selectMode", json!({"object": true}));
                    let _ = core.exec("polyCreateFacet", json!({"point": pts}));
                }
            }
            Tool::AppendPolygon => {
                let edges = std::mem::take(&mut self.ts.append_edges);
                let world: Vec<DVec3> = self.ts.cut_world.drain(..).collect();
                if let Some(mesh) = self.ts.cut_mesh.take() {
                    if let Some(it) = items.iter().find(|i| i.shape == mesh) {
                        let call = append_call(core, it, &edges, &world);
                        if let Some(call) = call {
                            let _ = core.exec("polyAppendVertex", call);
                        }
                    }
                }
            }
            _ => {}
        }
        self.tool_preview_clear();
    }

    /// Backspace in a modelling tool.
    pub fn tool_back(&mut self) {
        self.ts.cut_points.pop();
        self.ts.cut_world.pop();
        self.ts.dots.pop();
        self.tool_preview_clear();
    }

    /// IK Handle Tool and IK Spline Handle Tool (#317): a click on the start joint, then one on the end joint makes the
    /// handle with the tool's settings, as the reference does. The spline tool with Auto create curve off waits for a
    /// click on the curve as well. The tool stays on for the next handle.
    fn ik_handle_tool(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, items: &[DrawItem], spline: bool) -> bool {
        if kind != 0 || button != 1 {
            return false;
        }
        let ctx = if spline { "ikSplineHandleContext" } else { "ikHandleContext" };
        let setting = |k: &str| crate::ctx_cmds::prop(ctx, k);
        // A joint picked earlier that an undo took away starts the tool over.
        let alive = |core: &Core, id: Option<NodeId>| id.filter(|&j| core.s.graph.node(j).is_some());
        self.ts.ik_start = alive(core, self.ts.ik_start);
        self.ts.ik_end = alive(core, self.ts.ik_end);
        let joint = self.vp.pick_rig(&core.s.graph, px).filter(|&j| aztek_scene::rig::is_joint(&core.s.graph, j));
        let mut curve = None;
        match (self.ts.ik_start, self.ts.ik_end) {
            (None, _) => {
                let Some(j) = joint else { return false };
                self.ts.ik_start = Some(j);
                let _ = core.exec("select", json!({"args": [core.s.graph.path(j)], "replace": true}));
                return true;
            }
            (Some(start), None) => {
                let Some(j) = joint.filter(|&j| j != start) else { return false };
                self.ts.ik_end = Some(j);
                let wait = spline && !setting("createCurve").as_bool().unwrap_or(true);
                if wait {
                    let names = [core.s.graph.path(start), core.s.graph.path(j)];
                    let _ = core.exec("select", json!({"args": names, "replace": true}));
                    return true;
                }
            }
            (Some(_), Some(_)) => {
                let names = self.vp.pick(&core.s, items, px);
                let g = &core.s.graph;
                curve = names.iter().find(|n| g.find(n).ok().flatten().and_then(|id| aztek_scene::curves::as_curve(g, id)).is_some()).cloned();
                if curve.is_none() {
                    return false;
                }
            }
        }
        let (Some(start), Some(end)) = (self.ts.ik_start.take(), self.ts.ik_end.take()) else { return false };
        let g = &core.s.graph;
        let call = ik_handle_call(&g.path(start), &g.path(end), curve.as_deref(), spline);
        let _ = core.exec("ikHandle", call);
        true
    }

    /// Insert Joint Tool: a drag from a joint places a new one on the way to its child, sliding along the bone.
    fn insert_joint(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2) -> bool {
        match kind {
            0 if button == 1 => {
                let Some(j) = self.vp.pick_rig(&core.s.graph, px).filter(|&j| aztek_scene::rig::is_joint(&core.s.graph, j)) else { return false };
                let name = core.s.graph.path(j);
                if let Ok(v) = core.exec("insertJoint", json!({"args": [name]})) {
                    let made = v.as_str().map(str::to_string).or_else(|| v.get(0).and_then(Value::as_str).map(str::to_string));
                    if let Some(n) = made {
                        if let Ok(Some(id)) = core.s.graph.find(&n) {
                            self.ts.edge = Some((id, 0, 0.0));
                            self.ts.press = Some(px);
                        }
                    }
                }
                true
            }
            1 => {
                let (Some((j, ..)), Some(_)) = (self.ts.edge, self.ts.press) else { return false };
                let g = &mut core.s.graph;
                g.evaluate();
                let Some(kid) = g.children(j).iter().copied().find(|&k| aztek_scene::rig::is_joint(g, k)) else { return false };
                let parent = g.parent(j);
                let a = aztek_scene::rig::world(g, parent).w_axis.truncate();
                let b = aztek_scene::rig::world(g, kid).w_axis.truncate();
                let screen = self.vp.screen();
                let (Some(pa), Some(pb)) = (screen.project(a), screen.project(b)) else { return false };
                let (t, _) = seg_t(px, pa, pb);
                let at = a.lerp(b, t.clamp(0.02, 0.98));
                let name = g.path(j);
                self.apply_preview(core, "joint", json!({"args": [name], "edit": true, "position": at.to_array()}));
                true
            }
            2 if button == 1 => {
                self.ts.press = None;
                self.ts.edge = None;
                self.commit_preview(core);
                true
            }
            _ => false,
        }
    }

    pub(crate) fn sync_multi_cut_scene(&mut self, core: &Core) {
        let scene = core.s.graph.scene_serial();
        if self.ts.cut_scene != scene {
            self.ts.cut_scene = scene;
            self.ts.cut_mesh = None;
            self.ts.cut_points.clear();
            self.ts.cut_world.clear();
            self.ts.slice_from = None;
            self.tool_preview_clear();
        }
    }

    fn multi_cut(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, items: &[DrawItem]) -> bool {
        self.sync_multi_cut_scene(core);
        let scene = self.ts.cut_scene;
        let targets = multi_cut_targets(core);
        if targets.is_empty() {
            if kind == 0 && button == 1 && mods & (SHIFT | CTRL) == 0 {
                if let Some((i, _, _)) = self.vp.raycast(items, px) {
                    let it = &items[i];
                    if components::as_mesh(&core.s.graph, it.shape).is_none() || !self.vp.pickable(&core.s.graph, it.shape) {
                        return true;
                    }
                    let name = core.s.graph.path(it.xf);
                    if core.exec("select", json!({"args": [name]})).is_ok() && core.s.graph.scene_serial() == scene {
                        aztek_scene::components::set_mode_exact(&mut core.s.graph, aztek_scene::components::ComponentMode::Edge);
                        core.multi_cut = Some((scene, vec![it.shape]));
                    }
                }
            }
            return true;
        }
        let snap = snap_at(self, items, px).filter(|s| targets.contains(&items[s.item].shape));
        let shift = mods & SHIFT != 0;
        let ctrl = mods & CTRL != 0;
        let near_px = 12.0 * self.dpr;
        // Where a click would put the next point: a vertex, a place along an edge (Shift: in snap steps; Ctrl+Shift after
        // a point on an edge: square to that edge), or on the face.
        let place = |v: &View, s: &Snap| -> (Value, DVec3) {
            let it = &items[s.item];
            if s.vertex.2 < near_px {
                return (json!({"vertex": s.vertex.0}), s.vertex.1);
            }
            if s.edge.3 < near_px {
                let (a, b) = (it.mesh.edge_point(s.edge.0, 0), it.mesh.edge_point(s.edge.0, 1));
                let (pa, pb) = (it.mesh.position(a), it.mesh.position(b));
                let mut t = s.edge.1;
                if shift && ctrl && v.ts.cut_mesh == Some(it.shape) {
                    if let (Some(&last), Some(prev)) = (v.ts.cut_world.last(), v.ts.cut_points.last()) {
                        if let Some(e0) = prev.get("edge").and_then(Value::as_u64).filter(|e| it.mesh.edge_valid(*e as Index)) {
                            let (qa, qb) = (it.mesh.position(it.mesh.edge_point(e0 as Index, 0)), it.mesh.position(it.mesh.edge_point(e0 as Index, 1)));
                            let d0 = it.world.transform_vector3(qb - qa).normalize_or_zero();
                            let (wa, wb) = (it.world.transform_point3(pa), it.world.transform_point3(pb));
                            let den = (wb - wa).dot(d0);
                            if den.abs() > 1e-9 {
                                let k = (last - wa).dot(d0) / den;
                                if (0.0..=1.0).contains(&k) {
                                    t = k;
                                }
                            }
                        }
                    }
                } else if shift {
                    t = v.ts.mc.snap(t);
                }
                let w = it.world.transform_point3(pa.lerp(pb, t));
                return (json!({"edge": s.edge.0, "t": t}), w);
            }
            (json!({"face": s.face, "position": s.world.to_array()}), s.world)
        };
        match kind {
            1 => {
                self.tool_preview_clear();
                if let Some(from) = self.ts.slice_from {
                    let to = if shift { crate::tools_quad::snap45(from, px) } else { px };
                    self.slice_preview(items, from, to);
                    return true;
                }
                // Color Settings: the line, and each point by what it sits on (#140).
                let c = self.ts.mc.clone();
                let kind_color = |p: &Value| if p.get("vertex").is_some() { c.vertex_color } else if p.get("edge").is_some() { c.edge_color } else { c.face_color };
                let list = self.ts.cut_world.clone();
                for w in list.windows(2) {
                    self.vp.tool_lines.push((w[0], w[1], c.line_color));
                }
                for (p, at) in self.ts.cut_points.clone().iter().zip(&list) {
                    self.vp.tool_points.push((*at, kind_color(p)));
                }
                if let Some(s) = &snap {
                    let (point, at) = place(self, s);
                    self.vp.tool_points.push((at, kind_color(&point)));
                    if let Some(l) = list.last() {
                        self.vp.tool_lines.push((*l, at, c.line_color));
                    }
                    if ctrl && !shift {
                        let it = &items[s.item];
                        for (a, b) in ring_preview(&it.mesh, s.edge.0, s.edge.1) {
                            self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), GREEN));
                        }
                    }
                }
                true
            }
            0 if button == 2 => {
                if ctrl {
                    // Ctrl+middle click: a loop through the middle of the edge's ring.
                    let Some(s) = snap else { return false };
                    let shape = items[s.item].shape;
                    self.insert_loop_at(core, shape, s.edge.0, 0.5);
                    return true;
                }
                if self.ts.cut_points.is_empty() {
                    // A middle drag across the surface slices it.
                    self.ts.slice_from = Some(px);
                    return true;
                }
                false
            }
            2 if button == 2 => {
                let Some(from) = self.ts.slice_from.take() else { return false };
                self.tool_preview_clear();
                let to = if shift { crate::tools_quad::snap45(from, px) } else { px };
                if (to - from).length() < 4.0 * self.dpr {
                    return true;
                }
                let (at, n) = crate::tools_quad::screen_plane(self, from, to);
                let shapes: Vec<_> = self.meshes_under_line(items, from, to).into_iter().filter(|m| targets.contains(m)).map(|m| (m, at, n)).collect();
                self.slice(core, &shapes);
                true
            }
            0 if button == 1 => {
                let Some(s) = snap else { return false };
                let it = &items[s.item];
                if self.ts.cut_mesh.is_some_and(|m| m != it.shape) {
                    self.ts.cut_points.clear();
                    self.ts.cut_world.clear();
                }
                self.ts.cut_mesh = Some(it.shape);
                if ctrl && !shift {
                    // Ctrl: an edge loop, at the middle with Ctrl+middle as the reference offers.
                    let shape = it.shape;
                    self.insert_loop_at(core, shape, s.edge.0, s.edge.1);
                    return true;
                }
                let (point, at) = place(self, &s);
                self.ts.cut_points.push(point);
                self.ts.cut_world.push(at);
                true
            }
            _ => false,
        }
    }

    /// 3D Cut and Sew UV Tool: hovering shows the edge under the pointer; a drag gathers every edge it passes over on
    /// one mesh, and the release cuts their UVs (polyMapCut), or sews them with Ctrl (polyMapSew), as one step.
    fn cut_sew(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, items: &[DrawItem]) -> bool {
        let snap = snap_at(self, items, px).filter(|s| s.edge.3 < 12.0);
        let colour = if mods & CTRL != 0 { GREEN } else { RED };
        let edge_line = |it: &DrawItem, e: Index| {
            let m: &Mesh = &it.mesh;
            (it.world.transform_point3(m.position(m.edge_point(e, 0))), it.world.transform_point3(m.position(m.edge_point(e, 1))))
        };
        match kind {
            0 if button == 1 => {
                let Some(s) = snap else { return false };
                self.ts.cut_sew = Some((items[s.item].shape, vec![s.edge.0]));
                true
            }
            1 => {
                self.tool_preview_clear();
                // Over a vertex the nearest edge could be any of its edges: wait until the pointer is along one.
                if let (Some(s), Some((mesh, edges))) = (snap.as_ref().filter(|s| s.vertex.2 > 8.0), self.ts.cut_sew.as_mut()) {
                    if items[s.item].shape == *mesh && !edges.contains(&s.edge.0) {
                        edges.push(s.edge.0);
                    }
                }
                if let Some((mesh, edges)) = &self.ts.cut_sew {
                    if let Some(it) = items.iter().find(|i| i.shape == *mesh) {
                        for &e in edges {
                            let (a, b) = edge_line(it, e);
                            self.vp.tool_lines.push((a, b, colour));
                        }
                    }
                } else if let Some(s) = &snap {
                    let (a, b) = edge_line(&items[s.item], s.edge.0);
                    self.vp.tool_lines.push((a, b, colour));
                }
                true
            }
            2 if button == 1 => {
                self.tool_preview_clear();
                let Some((mesh, edges)) = self.ts.cut_sew.take() else { return false };
                let name = components::display_path(&core.s.graph, mesh);
                let list: Vec<String> = edges.iter().map(|e| format!("{name}.e[{e}]")).collect();
                let cmd = if mods & CTRL != 0 { "polyMapSew" } else { "polyMapCut" };
                let _ = core.exec(cmd, json!({"args": list}));
                true
            }
            _ => false,
        }
    }

    fn insert_loop(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, items: &[DrawItem]) -> bool {
        let snap = snap_at(self, items, px);
        match kind {
            1 => {
                self.tool_preview_clear();
                if let Some(s) = &snap {
                    let it = &items[s.item];
                    for (a, b) in ring_preview(&it.mesh, s.edge.0, s.edge.1) {
                        self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), GREEN));
                    }
                    self.ts.edge = Some((it.shape, s.edge.0, s.edge.1));
                }
                true
            }
            0 if button == 1 => {
                self.ts.press = Some(px);
                snap.is_some()
            }
            2 if button == 1 => {
                self.ts.press = None;
                let Some((mesh, e, t)) = self.ts.edge.take() else { return false };
                let name = components::display_path(&core.s.graph, mesh);
                let _ = core.exec("select", json!({"args": [format!("{name}.e[{e}]")]}));
                let from = crate::tools_quad::point_count(core, mesh);
                let _ = core.exec_silent("undoInfo", json!({"openChunk": true}));
                let _ = core.exec("polyInsertLoop", json!({"fraction": t}));
                crate::tools_quad::snap_new_points_to_live(core, mesh, from);
                let _ = core.exec_silent("undoInfo", json!({"closeChunk": true}));
                self.tool_preview_clear();
                true
            }
            _ => false,
        }
    }

    fn offset_loop(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, items: &[DrawItem]) -> bool {
        match kind {
            0 if button == 1 => {
                let Some(s) = snap_at(self, items, px) else { return false };
                let it = &items[s.item];
                let name = components::display_path(&core.s.graph, it.shape);
                let _ = core.exec("select", json!({"args": [format!("{name}.e[{}]", s.edge.0)]}));
                self.ts.press = Some(px);
                self.ts.applied = false;
                true
            }
            1 => {
                let Some(p) = self.ts.press else { return false };
                let d = ((px - p).length() / 200.0).clamp(0.01, 0.95);
                self.apply_preview(core, "polyOffsetLoop", json!({"distance": d}));
                true
            }
            2 if button == 1 => {
                self.ts.press = None;
                self.commit_preview(core);
                true
            }
            _ => false,
        }
    }

    fn target_weld(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, items: &[DrawItem]) -> bool {
        let edge_mode = components::mode(&core.s.graph) == ComponentMode::Edge;
        let snap = snap_at(self, items, px);
        match kind {
            0 if button == 1 => {
                let Some(s) = &snap else { return false };
                let id = if edge_mode { s.edge.0 } else { s.vertex.0 };
                self.ts.weld_source = Some((items[s.item].shape, id, edge_mode));
                self.ts.cut_world = vec![if edge_mode { s.edge.2 } else { s.vertex.1 }];
                true
            }
            1 => {
                self.tool_preview_clear();
                if let Some(s) = &snap {
                    let at = if edge_mode { s.edge.2 } else { s.vertex.1 };
                    self.vp.tool_points.push((at, GREEN));
                    if let Some(&from) = self.ts.cut_world.first() {
                        self.vp.tool_lines.push((from, at, GREEN));
                    }
                }
                true
            }
            2 if button == 1 => {
                let Some((mesh, src, edge)) = self.ts.weld_source.take() else { return false };
                self.ts.cut_world.clear();
                self.tool_preview_clear();
                let Some(s) = snap else { return true };
                let tgt = if edge { s.edge.0 } else { s.vertex.0 };
                if items[s.item].shape == mesh && tgt != src {
                    let name = core.s.graph.path(mesh);
                    let _ = core.exec("polyTargetWeld", json!({"mesh": name, "source": src, "target": tgt, "edge": edge}));
                }
                true
            }
            _ => false,
        }
    }

    /// Slide Edge and Crease: edges picked as usual; a drag (middle anywhere, or left on a picked edge) sets the value.
    #[allow(clippy::too_many_arguments)]
    fn slide_drag(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, items: &[DrawItem], cmd: &str, per_px: f64) -> bool {
        match kind {
            0 => {
                if button == 2 || button == 1 {
                    if button == 1 {
                        let picked = self.vp.pick(&core.s, items, px);
                        let sel = components::component_strings(&core.s.graph, true);
                        if picked.is_empty() || !picked.iter().any(|p| sel.contains(p)) {
                            crate::view::select_picked(core, picked, mods);
                            return true;
                        }
                    }
                    self.ts.press = Some(px);
                    self.ts.applied = false;
                }
                true
            }
            1 => {
                let Some(p) = self.ts.press else { return false };
                let v = (px.x - p.x) * per_px;
                let call = if cmd == "polyCrease" { json!({"value": v.max(0.0)}) } else { json!({"value": v.clamp(-1.0, 1.0)}) };
                self.apply_preview(core, cmd, call);
                true
            }
            2 => {
                if self.ts.press.take().is_some() {
                    self.commit_preview(core);
                }
                true
            }
            _ => false,
        }
    }

    fn create_polygon(&mut self, _core: &mut Core, kind: i32, button: i32, px: DVec2, _items: &[DrawItem]) -> bool {
        let at = self.vp.place_point(px);
        match kind {
            0 if button == 1 => {
                self.ts.cut_world.push(at);
                true
            }
            1 => {
                self.tool_preview_clear();
                let list = self.ts.cut_world.clone();
                for w in list.windows(2) {
                    self.vp.tool_lines.push((w[0], w[1], GREEN));
                }
                for p in &list {
                    self.vp.tool_points.push((*p, WHITE));
                }
                if let (Some(l), Some(f)) = (list.last(), list.first()) {
                    self.vp.tool_lines.push((*l, at, WHITE));
                    self.vp.tool_lines.push((at, *f, WHITE));
                }
                !list.is_empty()
            }
            _ => false,
        }
    }

    fn append_polygon(&mut self, kind: i32, button: i32, px: DVec2, items: &[DrawItem]) -> bool {
        let snap = snap_at(self, items, px);
        match kind {
            1 => {
                self.tool_preview_clear();
                // The border of the mesh under the cursor, in red, to show what can be picked.
                if let Some(s) = &snap {
                    let it = &items[s.item];
                    let m = &it.mesh;
                    for e in 0..m.edge_capacity() {
                        if m.edge_valid(e) && m.is_boundary_edge(e) {
                            self.vp.tool_lines.push((it.world.transform_point3(m.position(m.edge_point(e, 0))), it.world.transform_point3(m.position(m.edge_point(e, 1))), RED));
                        }
                    }
                }
                for p in self.ts.cut_world.clone() {
                    self.vp.tool_points.push((p, WHITE));
                }
                true
            }
            0 if button == 1 => {
                if let Some(s) = &snap {
                    let it = &items[s.item];
                    if it.mesh.is_boundary_edge(s.edge.0) && s.edge.3 < 14.0 {
                        self.ts.cut_mesh = Some(it.shape);
                        self.ts.append_edges.push(s.edge.0);
                        return true;
                    }
                }
                self.ts.cut_world.push(self.vp.place_point(px));
                true
            }
            _ => false,
        }
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Quad Draw

    /// Where the cursor is on the live surface, or on the view plane with nothing live.
    pub(crate) fn live_point(&mut self, core: &Core, items: &[DrawItem], px: DVec2) -> Option<DVec3> {
        let live = &core.s.live_surface;
        if live.is_empty() {
            return Some(self.vp.place_point(px));
        }
        let hits: Vec<DrawItem> = items
            .iter()
            .filter(|i| live.contains(&i.shape))
            .map(|i| DrawItem { shape: i.shape, xf: i.xf, world: i.world, mesh: i.mesh.clone(), smooth: 0 })
            .collect();
        // Live Constraint Options: back faces only when Snap to Backfaces is on, and the Surface Offset off the
        // surface along its normal, toward the eye (#140).
        let (i, f, at) = self.vp.raycast_faces(&hits, px, self.vp.live.backfaces)?;
        let Some(off) = self.vp.live.offset.filter(|o| *o != 0.0) else { return Some(at) };
        let it = &hits[i];
        let n = it.world.inverse().transpose().transform_vector3(it.mesh.face_normal(f)).normalize_or_zero();
        let n = if n.dot(self.vp.cam.eye() - at) < 0.0 { -n } else { n };
        Some(at + n * off)
    }

    pub(crate) fn quad_target<'a>(&self, core: &Core, items: &'a [DrawItem]) -> Option<&'a DrawItem> {
        let live = &core.s.live_surface;
        if let Some(m) = self.ts.quad_mesh {
            if let Some(it) = items.iter().find(|i| i.shape == m) {
                return Some(it);
            }
        }
        let sel = core.s.graph.selection();
        items.iter().find(|i| (sel.contains(&i.xf) || sel.contains(&i.shape)) && !live.contains(&i.shape))
    }

    /// Dots and target vertices near the cursor: world place, screen place, item for the face call.
    fn quad_candidates(&self, core: &Core, items: &[DrawItem], px: DVec2, reach: f64) -> Vec<(DVec3, DVec2, Value)> {
        let screen = self.vp.screen();
        let mut out = Vec::new();
        for d in &self.ts.dots {
            if let Some(q) = screen.project(*d) {
                if (q - px).length() < reach {
                    out.push((*d, q, json!({"position": d.to_array()})));
                }
            }
        }
        if let Some(it) = self.quad_target(core, items) {
            for p in 0..it.mesh.point_capacity() {
                if !it.mesh.point_valid(p) {
                    continue;
                }
                let w = it.world.transform_point3(it.mesh.position(p));
                if let Some(q) = screen.project(w) {
                    if (q - px).length() < reach {
                        out.push((w, q, json!({"vertex": p})));
                    }
                }
            }
        }
        out.sort_by(|a, b| (a.1 - px).length().total_cmp(&(b.1 - px).length()));
        out
    }

    /// The face Shift would make: the nearest 4 candidates around the cursor, in order round it.
    fn quad_preview(&self, core: &Core, items: &[DrawItem], px: DVec2) -> Vec<(DVec3, Value)> {
        let mut c = self.quad_candidates(core, items, px, 220.0 * self.dpr);
        c.truncate(4);
        if c.len() < 3 {
            return Vec::new();
        }
        c.sort_by(|a, b| {
            let (x, y) = (a.1 - px, b.1 - px);
            y.y.atan2(y.x).total_cmp(&x.y.atan2(x.x))
        });
        c.into_iter().map(|x| (x.0, x.2)).collect()
    }

    fn quad_draw(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, items: &[DrawItem]) -> bool {
        let (shift, ctrl) = (mods & SHIFT != 0, mods & CTRL != 0);
        match kind {
            1 => {
                if let Some(last) = self.ts.last_px {
                    let d = px - last;
                    if d.length() > 2.0 {
                        self.ts.last_dir = d.normalize();
                    }
                }
                self.ts.last_px = Some(px);
                self.tool_preview_clear();
                for d in self.ts.dots.clone() {
                    self.vp.tool_points.push((d, GREEN));
                }
                if self.ts.normal_drag.is_some() {
                    self.quad_normal_drag(core, px);
                    return true;
                }
                if let Some(last) = self.ts.relax_picked {
                    if (px - last).length() >= 4.0 * self.dpr {
                        self.ts.relax_picked = Some(px);
                        self.quad_relax_picked(core, items);
                    }
                    return true;
                }
                if let Some((from, width)) = self.ts.strip_resize {
                    // Tab+middle drag: the strip width follows the mouse sideways.
                    self.ts.qd.strip_px = (width + (px.x - from.x) / self.dpr).clamp(2.0, 400.0);
                    self.quad_strip_preview(core, items, from);
                    return true;
                }
                if self.ts.strip.is_some() {
                    self.quad_strip_drag(core, items, px);
                    return true;
                }
                if self.ts.extend.is_some() {
                    self.quad_extend_drag(core, items, px);
                    return true;
                }
                if self.ts.relaxing {
                    self.quad_relax(core, items, px);
                    return true;
                }
                if let Some(i) = self.ts.dragging_dot {
                    if let Some(p) = self.live_point(core, items, px) {
                        if let Some(d) = self.ts.dots.get_mut(i) {
                            *d = p;
                        }
                    }
                    return true;
                }
                if self.ts.comp_drag.is_some() {
                    self.quad_comp_drag(core, items, px);
                    return true;
                }
                if self.ts.vertex_extrude.is_some() {
                    self.quad_vertex_extrude_drag(core, items, px);
                    return true;
                }
                if let Some((mesh, v)) = self.ts.dragging_vertex {
                    if let Some(p) = self.live_point(core, items, px) {
                        let name = components::display_path(&core.s.graph, mesh);
                        self.apply_preview(core, "xform", json!({"args": [format!("{name}.vtx[{v}]")], "worldSpace": true, "translation": p.to_array()}));
                    }
                    return true;
                }
                if self.ts.tab {
                    // Over a border edge: the edges Tab+drag would extend; elsewhere the strip's next quad.
                    if let Some(it) = self.quad_target(core, items) {
                        let list = [DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px).filter(|s| s.edge.3 < 12.0 * self.dpr && list[0].mesh.is_boundary_edge(s.edge.0)) {
                            let it = &list[0];
                            for e in crate::tools_quad::extend_edges(&it.mesh, s.edge.0, self.ts.qd.extend) {
                                let (a, b) = (it.mesh.position(it.mesh.edge_point(e, 0)), it.mesh.position(it.mesh.edge_point(e, 1)));
                                self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), GREEN));
                            }
                            return true;
                        }
                    }
                    self.quad_strip_preview(core, items, px);
                } else if shift && !ctrl {
                    let poly = self.quad_preview(core, items, px);
                    for i in 0..poly.len() {
                        self.vp.tool_lines.push((poly[i].0, poly[(i + 1) % poly.len()].0, GREEN));
                    }
                } else if !ctrl {
                    self.quad_highlight(core, items, px);
                } else if ctrl {
                    if let Some(it) = self.quad_target(core, items) {
                        let shape = it.shape;
                        let list = [DrawItem { shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px) {
                            let it = &list[0];
                            if shift {
                                // Ctrl+Shift on an edge: the loop that would go.
                                if s.edge.3 >= 6.0 * self.dpr || s.vertex.2 <= 10.0 * self.dpr {
                                    return true;
                                }
                                for e in aztek_geo::topology::edge_loop(&it.mesh, s.edge.0) {
                                    let (a, b) = (it.mesh.position(it.mesh.edge_point(e, 0)), it.mesh.position(it.mesh.edge_point(e, 1)));
                                    self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), RED));
                                }
                            } else {
                                for (a, b) in ring_preview(&it.mesh, s.edge.0, s.edge.1) {
                                    self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), GREEN));
                                }
                            }
                        }
                    }
                }
                true
            }
            0 if button == 2 => {
                let target = self.quad_target(core, items).map(|i| (i.shape, i.xf, i.world, i.mesh.clone()));
                if ctrl {
                    // Ctrl+middle click: a loop through the middle of the edge's ring.
                    if let Some((shape, xf, world, mesh)) = target {
                        let list = [DrawItem { shape, xf, world, mesh, smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px) {
                            self.insert_loop_at(core, shape, s.edge.0, 0.5);
                        }
                    }
                    return true;
                }
                if self.ts.tab {
                    if let Some((shape, xf, world, mesh)) = target {
                        let list = [DrawItem { shape, xf, world, mesh, smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px).filter(|s| s.edge.3 < 12.0 * self.dpr && list[0].mesh.is_boundary_edge(s.edge.0)) {
                            // Tab+middle on a border edge: its loop along the border.
                            let edges = crate::tools_quad::extend_edges(&list[0].mesh, s.edge.0, 1);
                            self.ts.extend = Some(crate::tools_quad::Extend { mesh: shape, edges, from: s.edge.2 });
                            self.ts.applied = false;
                            return true;
                        }
                    }
                    // Tab+middle elsewhere: the strip width.
                    self.ts.strip_resize = Some((px, self.ts.qd.strip_px));
                    return true;
                }
                if shift {
                    // Shift+middle drag: the picked points relax, a step each few pixels.
                    let _ = core.exec_silent("undoInfo", json!({"openChunk": true}));
                    self.ts.relax_picked = Some(px);
                    self.quad_relax_picked(core, items);
                    return true;
                }
                // With a live surface a middle drag moves what's under it over the surface, as a left drag does (#164).
                if !core.s.live_surface.is_empty() {
                    return self.quad_comp_press(core, items, px);
                }
                // Middle drag on a dot or a vertex, with no live surface: along its normal.
                self.quad_normal_press(core, items, px)
            }
            0 if button == 1 => {
                let target = self.quad_target(core, items).map(|i| (i.shape, i.xf, i.world, i.mesh.clone()));
                if shift && ctrl {
                    // Delete: a dot, an edge's loop when on an edge, or the face under the cursor.
                    let screen = self.vp.screen();
                    if let Some(i) = self.ts.dots.iter().position(|d| screen.project(*d).is_some_and(|q| (q - px).length() < 12.0 * self.dpr)) {
                        self.ts.dots.remove(i);
                        return true;
                    }
                    if let Some((shape, xf, world, mesh)) = target {
                        let list = [DrawItem { shape, xf, world, mesh, smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px) {
                            if s.edge.3 < 6.0 * self.dpr && s.vertex.2 > 10.0 * self.dpr {
                                self.quad_delete_loop(core, shape, &list[0].mesh, s.edge.0);
                                return true;
                            }
                            let name = components::display_path(&core.s.graph, shape);
                            let _ = core.exec("select", json!({"args": [format!("{name}.f[{}]", s.face)]}));
                            let _ = core.exec("deleteComponents", json!({}));
                        }
                    }
                    return true;
                }
                if shift {
                    // Inside the quad mesh: a drag relaxes; outside it a click fills between the dots.
                    if let Some((shape, xf, world, mesh)) = &target {
                        let list = [DrawItem { shape: *shape, xf: *xf, world: *world, mesh: mesh.clone(), smooth: 0 }];
                        if self.vp.raycast(&list, px).is_some() {
                            self.ts.relaxing = true;
                            let _ = core.exec_silent("undoInfo", json!({"openChunk": true}));
                            self.quad_relax(core, items, px);
                            return true;
                        }
                    }
                    let poly = self.quad_preview(core, items, px);
                    if poly.len() >= 3 {
                        let items_json: Vec<Value> = poly.iter().map(|p| p.1.clone()).collect();
                        let mut call = json!({"items": items_json});
                        if let Some((shape, ..)) = &target {
                            call["mesh"] = json!(core.s.graph.path(*shape));
                        } else {
                            let _ = core.exec_silent("select", json!({"clear": true}));
                        }
                        if let Ok(v) = core.exec("polyAppendVertex", call) {
                            if self.ts.quad_mesh.is_none() {
                                if let Some(n) = v.get(0).and_then(Value::as_str) {
                                    if let Ok(Some(id)) = core.s.graph.find(n) {
                                        self.ts.quad_mesh = components::as_mesh(&core.s.graph, id);
                                    }
                                }
                            }
                            // Dots used by the face are spent.
                            let used: Vec<DVec3> = poly.iter().filter(|p| p.1.get("position").is_some()).map(|p| p.0).collect();
                            self.ts.dots.retain(|d| !used.iter().any(|u| (*u - *d).length() < 1e-9));
                        }
                    }
                    return true;
                }
                if ctrl {
                    if let Some((shape, xf, world, mesh)) = target {
                        let list = [DrawItem { shape, xf, world, mesh, smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px) {
                            self.insert_loop_at(core, shape, s.edge.0, s.edge.1);
                        }
                    }
                    return true;
                }
                if self.ts.tab {
                    // Tab+drag on a border vertex: a quad out of its corner (#164).
                    if self.quad_vertex_extrude_press(core, items, px) {
                        return true;
                    }
                    if let Some((shape, xf, world, mesh)) = &target {
                        let list = [DrawItem { shape: *shape, xf: *xf, world: *world, mesh: mesh.clone(), smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px).filter(|s| s.edge.3 < 12.0 * self.dpr) {
                            if list[0].mesh.is_boundary_edge(s.edge.0) {
                                let edges = crate::tools_quad::extend_edges(&list[0].mesh, s.edge.0, self.ts.qd.extend);
                                self.ts.extend = Some(crate::tools_quad::Extend { mesh: *shape, edges, from: s.edge.2 });
                                self.ts.applied = false;
                                return true;
                            }
                        }
                    }
                    // Off the mesh's border: a quad strip from here.
                    self.ts.strip = Some(crate::tools_quad::Strip { last_px: px, dir: self.ts.last_dir, edge: None, started: false });
                    return true;
                }
                // A drag on what's there moves it; a click on nothing places a dot.
                let screen = self.vp.screen();
                if let Some(i) = self.ts.dots.iter().position(|d| screen.project(*d).is_some_and(|q| (q - px).length() < 12.0 * self.dpr)) {
                    self.ts.dragging_dot = Some(i);
                    return true;
                }
                // A vertex, an edge or a face of the quad mesh: it moves over the surface (#164), soft selection
                // carrying the points round it (#166).
                if target.is_some() && self.quad_comp_press(core, items, px) {
                    return true;
                }
                if let Some(p) = self.live_point(core, items, px) {
                    self.ts.dots.push(p);
                }
                true
            }
            2 if button == 1 || button == 2 => {
                // A middle release the tool didn't start is the view's (a manipulator drag).
                let mine = button == 1 || self.ts.strip_resize.is_some() || self.ts.extend.is_some() || self.ts.normal_drag.is_some() || self.ts.relax_picked.is_some();
                if self.ts.relax_picked.take().is_some() {
                    let _ = core.exec_silent("undoInfo", json!({"closeChunk": true}));
                }
                if let Some(d) = self.ts.normal_drag.take() {
                    if matches!(d.what, crate::tools_quad::Dragged::Vertex(..)) {
                        self.commit_preview(core);
                    }
                }
                self.ts.dragging_dot = None;
                self.ts.strip = None;
                self.ts.strip_resize = None;
                if std::mem::take(&mut self.ts.relaxing) {
                    let _ = core.exec_silent("undoInfo", json!({"closeChunk": true}));
                }
                if let Some((mesh, v)) = self.ts.dragging_vertex.take() {
                    self.commit_preview(core);
                    self.quad_auto_weld(core, mesh, v);
                }
                let mine = mine || self.ts.comp_drag.is_some() || self.ts.vertex_extrude.is_some();
                if self.ts.comp_drag.is_some() {
                    self.quad_comp_release(core);
                }
                if self.ts.vertex_extrude.take().is_some() {
                    self.commit_preview(core);
                }
                if self.ts.extend.take().is_some() {
                    self.commit_preview(core);
                }
                mine
            }
            _ => false,
        }
    }
}

pub(crate) fn multi_cut_targets(core: &Core) -> &[NodeId] {
    core.multi_cut.as_ref().filter(|(scene, _)| *scene == core.s.graph.scene_serial())
        .map_or(&[], |(_, targets)| targets.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_cut_forgets_targets_when_the_scene_is_cleared() {
        let mut core = Core::new();
        core.mel_checked("polyCube -n original; select original;").unwrap();
        let targets = components::hilited_meshes(&core.s.graph);
        assert_eq!(targets.len(), 1);
        core.multi_cut = Some((core.s.graph.scene_serial(), targets));

        core.mel_checked("polyCube -n another;").unwrap();
        assert_eq!(multi_cut_targets(&core).len(), 1);
        core.mel_checked("newScene; polyCube -n original;").unwrap();
        assert!(multi_cut_targets(&core).is_empty());
    }
}

/// The call that appends a face onto the picked border edges, ordered so the face is simple.
fn append_call(core: &Core, it: &DrawItem, edges: &[Index], world: &[DVec3]) -> Option<Value> {
    let m = &it.mesh;
    let mut items: Vec<Value> = Vec::new();
    let mut last = world.first().copied();
    let first_edge = edges.first()?;
    // First edge: end nearest what follows comes last.
    let next_target = world.first().copied().or_else(|| edges.get(1).map(|&e| it.world.transform_point3((m.position(m.edge_point(e, 0)) + m.position(m.edge_point(e, 1))) * 0.5)));
    let (a0, a1) = (m.edge_point(*first_edge, 0), m.edge_point(*first_edge, 1));
    let (w0, w1) = (it.world.transform_point3(m.position(a0)), it.world.transform_point3(m.position(a1)));
    let (x0, x1) = match next_target {
        Some(t) if (w0 - t).length() < (w1 - t).length() => (a1, a0),
        _ => (a0, a1),
    };
    items.push(json!({"vertex": x0}));
    items.push(json!({"vertex": x1}));
    for p in world {
        items.push(json!({"position": p.to_array()}));
        last = Some(*p);
    }
    let mut tail = it.world.transform_point3(m.position(x1));
    if let Some(l) = last {
        tail = l;
    }
    for &e in &edges[1..] {
        let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
        let (wa, wb) = (it.world.transform_point3(m.position(a)), it.world.transform_point3(m.position(b)));
        let (first, second, end) = if (wa - tail).length() < (wb - tail).length() { (a, b, wb) } else { (b, a, wa) };
        items.push(json!({"vertex": first}));
        items.push(json!({"vertex": second}));
        tail = end;
    }
    if items.len() < 3 {
        return None;
    }
    Some(json!({"mesh": core.s.graph.path(it.shape), "items": items}))
}
