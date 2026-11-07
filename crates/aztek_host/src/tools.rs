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
    pub extend_edge: Option<(NodeId, Index)>,
    pub tab: bool,
    pub append_edges: Vec<Index>,
    pub weld_source: Option<(NodeId, Index, bool)>,
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

impl View {
    fn tool_preview_clear(&mut self) {
        self.vp.tool_lines.clear();
        self.vp.tool_points.clear();
    }

    fn apply_preview(&mut self, core: &mut Core, cmd: &str, call: Value) {
        if self.ts.applied {
            core.retract_preview();
        }
        let ok = core.exec_silent(cmd, call.clone()).is_ok();
        self.ts.applied = ok;
        self.ts.last_call = ok.then(|| (cmd.to_string(), call));
    }

    fn commit_preview(&mut self, core: &mut Core) {
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
            Tool::MultiCut => Some(self.multi_cut(core, kind, button, px, mods, items)),
            Tool::InsertLoop => Some(self.insert_loop(core, kind, button, px, items)),
            Tool::OffsetLoop => Some(self.offset_loop(core, kind, button, px, items)),
            Tool::TargetWeld => Some(self.target_weld(core, kind, button, px, items)),
            Tool::SlideEdge => Some(self.slide_drag(core, kind, button, px, mods, items, "polySlideEdge", 0.005)),
            Tool::Crease => Some(self.slide_drag(core, kind, button, px, mods, items, "polyCrease", 0.02)),
            Tool::CreatePolygon => Some(self.create_polygon(core, kind, button, px, items)),
            Tool::AppendPolygon => Some(self.append_polygon(kind, button, px, items)),
            Tool::QuadDraw => Some(self.quad_draw(core, kind, button, px, mods, items)),
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
                    let name = core.s.graph.path(mesh);
                    let eye = self.vp.cam.eye();
                    let call = json!({"mesh": name, "points": points, "eye": [eye.x, eye.y, eye.z], "select": false});
                    let _ = core.exec("polyKnife", call);
                }
            }
            Tool::CreatePolygon => {
                let pts: Vec<[f64; 3]> = self.ts.cut_world.drain(..).map(|p| p.to_array()).collect();
                if pts.len() >= 3 {
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

    fn multi_cut(&mut self, core: &mut Core, kind: i32, button: i32, px: DVec2, mods: i32, items: &[DrawItem]) -> bool {
        let snap = snap_at(self, items, px);
        let shift = mods & SHIFT != 0;
        match kind {
            1 => {
                self.tool_preview_clear();
                let list = self.ts.cut_world.clone();
                for w in list.windows(2) {
                    self.vp.tool_lines.push((w[0], w[1], GREEN));
                }
                for p in &list {
                    self.vp.tool_points.push((*p, WHITE));
                }
                if let Some(s) = &snap {
                    let at = if s.vertex.2 < 12.0 {
                        s.vertex.1
                    } else if s.edge.3 < 12.0 {
                        if shift {
                            let t = (s.edge.1 * 10.0).round() / 10.0;
                            let it = &items[s.item];
                            let (a, b) = (it.mesh.edge_point(s.edge.0, 0), it.mesh.edge_point(s.edge.0, 1));
                            it.world.transform_point3(it.mesh.position(a).lerp(it.mesh.position(b), t))
                        } else {
                            s.edge.2
                        }
                    } else {
                        s.world
                    };
                    self.vp.tool_points.push((at, GREEN));
                    if let Some(l) = list.last() {
                        self.vp.tool_lines.push((*l, at, WHITE));
                    }
                    if mods & CTRL != 0 {
                        let it = &items[s.item];
                        for (a, b) in ring_preview(&it.mesh, s.edge.0, s.edge.1) {
                            self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), GREEN));
                        }
                    }
                }
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
                if mods & CTRL != 0 {
                    // Ctrl: an edge loop, at the middle with Ctrl+middle as the reference offers.
                    let name = core.s.graph.path(it.shape);
                    let _ = core.exec("polyKnife", json!({"mesh": name, "loopEdge": s.edge.0, "fraction": s.edge.1, "select": false}));
                    return true;
                }
                if s.vertex.2 < 12.0 {
                    self.ts.cut_points.push(json!({"vertex": s.vertex.0}));
                    self.ts.cut_world.push(s.vertex.1);
                } else if s.edge.3 < 12.0 {
                    let t = if shift { (s.edge.1 * 10.0).round() / 10.0 } else { s.edge.1 };
                    self.ts.cut_points.push(json!({"edge": s.edge.0, "t": t}));
                    self.ts.cut_world.push(s.edge.2);
                } else {
                    self.ts.cut_points.push(json!({"face": s.face, "position": s.world.to_array()}));
                    self.ts.cut_world.push(s.world);
                }
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
                let _ = core.exec("polyInsertLoop", json!({"fraction": t}));
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
    fn live_point(&mut self, core: &Core, items: &[DrawItem], px: DVec2) -> Option<DVec3> {
        let live = &core.s.live_surface;
        if live.is_empty() {
            return Some(self.vp.place_point(px));
        }
        let hits: Vec<DrawItem> = items
            .iter()
            .filter(|i| live.contains(&i.shape))
            .map(|i| DrawItem { shape: i.shape, xf: i.xf, world: i.world, mesh: i.mesh.clone(), smooth: 0 })
            .collect();
        self.vp.raycast(&hits, px).map(|h| h.2)
    }

    fn quad_target<'a>(&self, core: &Core, items: &'a [DrawItem]) -> Option<&'a DrawItem> {
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
                self.tool_preview_clear();
                for d in self.ts.dots.clone() {
                    self.vp.tool_points.push((d, GREEN));
                }
                if let Some(i) = self.ts.dragging_dot {
                    if let Some(p) = self.live_point(core, items, px) {
                        if let Some(d) = self.ts.dots.get_mut(i) {
                            *d = p;
                        }
                    }
                    return true;
                }
                if let Some((mesh, v)) = self.ts.dragging_vertex {
                    if let Some(p) = self.live_point(core, items, px) {
                        let name = components::display_path(&core.s.graph, mesh);
                        self.apply_preview(core, "xform", json!({"args": [format!("{name}.vtx[{v}]")], "worldSpace": true, "translation": p.to_array()}));
                    }
                    return true;
                }
                if let Some((mesh, e)) = self.ts.extend_edge {
                    if let (Some(it), Some(p)) = (items.iter().find(|i| i.shape == mesh), self.live_point(core, items, px)) {
                        let (a, b) = (it.mesh.edge_point(e, 0), it.mesh.edge_point(e, 1));
                        let (wa, wb) = (it.world.transform_point3(it.mesh.position(a)), it.world.transform_point3(it.mesh.position(b)));
                        let mid = (wa + wb) * 0.5;
                        let (na, nb) = (p + (wa - mid), p + (wb - mid));
                        for (x, y) in [(wa, na), (na, nb), (nb, wb)] {
                            self.vp.tool_lines.push((x, y, GREEN));
                        }
                    }
                    return true;
                }
                if shift {
                    let poly = self.quad_preview(core, items, px);
                    for i in 0..poly.len() {
                        self.vp.tool_lines.push((poly[i].0, poly[(i + 1) % poly.len()].0, GREEN));
                    }
                } else if ctrl {
                    if let Some(it) = self.quad_target(core, items) {
                        let shape = it.shape;
                        let list = [DrawItem { shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px) {
                            let it = &list[0];
                            for (a, b) in ring_preview(&it.mesh, s.edge.0, s.edge.1) {
                                self.vp.tool_lines.push((it.world.transform_point3(a), it.world.transform_point3(b), GREEN));
                            }
                        }
                    }
                }
                true
            }
            0 if button == 1 => {
                let target = self.quad_target(core, items).map(|i| (i.shape, i.xf, i.world, i.mesh.clone()));
                if shift && ctrl {
                    // Delete: a dot, or the face under the cursor.
                    let screen = self.vp.screen();
                    if let Some(i) = self.ts.dots.iter().position(|d| screen.project(*d).is_some_and(|q| (q - px).length() < 12.0 * self.dpr)) {
                        self.ts.dots.remove(i);
                        return true;
                    }
                    if let Some((shape, xf, world, mesh)) = target {
                        let list = [DrawItem { shape, xf, world, mesh, smooth: 0 }];
                        if let Some((_, f, _)) = self.vp.raycast(&list, px) {
                            let name = components::display_path(&core.s.graph, shape);
                            let _ = core.exec("select", json!({"args": [format!("{name}.f[{f}]")]}));
                            let _ = core.exec("deleteComponents", json!({}));
                        }
                    }
                    return true;
                }
                if shift {
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
                            let name = core.s.graph.path(shape);
                            let _ = core.exec("polyKnife", json!({"mesh": name, "loopEdge": s.edge.0, "fraction": s.edge.1, "select": false}));
                        }
                    }
                    return true;
                }
                if self.ts.tab {
                    if let Some((shape, xf, world, mesh)) = target {
                        let list = [DrawItem { shape, xf, world, mesh, smooth: 0 }];
                        if let Some(s) = snap_at(self, &list, px) {
                            if list[0].mesh.is_boundary_edge(s.edge.0) {
                                self.ts.extend_edge = Some((shape, s.edge.0));
                            }
                        }
                    }
                    return true;
                }
                // A drag on what's there moves it; a click on nothing places a dot.
                let screen = self.vp.screen();
                if let Some(i) = self.ts.dots.iter().position(|d| screen.project(*d).is_some_and(|q| (q - px).length() < 12.0 * self.dpr)) {
                    self.ts.dragging_dot = Some(i);
                    return true;
                }
                if let Some((shape, _, world, mesh)) = &target {
                    let near = (0..mesh.point_capacity()).filter(|&p| mesh.point_valid(p)).find(|&p| screen.project(world.transform_point3(mesh.position(p))).is_some_and(|q| (q - px).length() < 12.0 * self.dpr));
                    if let Some(p) = near {
                        self.ts.dragging_vertex = Some((*shape, p));
                        self.ts.applied = false;
                        return true;
                    }
                }
                if let Some(p) = self.live_point(core, items, px) {
                    self.ts.dots.push(p);
                }
                true
            }
            2 if button == 1 => {
                self.ts.dragging_dot = None;
                if self.ts.dragging_vertex.take().is_some() {
                    self.commit_preview(core);
                }
                if let Some((mesh, e)) = self.ts.extend_edge.take() {
                    if let (Some(it), Some(p)) = (items.iter().find(|i| i.shape == mesh), self.live_point(core, items, px)) {
                        let (a, b) = (it.mesh.edge_point(e, 0), it.mesh.edge_point(e, 1));
                        let (wa, wb) = (it.world.transform_point3(it.mesh.position(a)), it.world.transform_point3(it.mesh.position(b)));
                        let mid = (wa + wb) * 0.5;
                        let (na, nb) = (p + (wa - mid), p + (wb - mid));
                        let name = core.s.graph.path(mesh);
                        let _ = core.exec("polyAppendVertex", json!({"mesh": name, "items": [{"vertex": a}, {"vertex": b}, {"position": nb.to_array()}, {"position": na.to_array()}]}));
                    }
                }
                true
            }
            _ => false,
        }
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
