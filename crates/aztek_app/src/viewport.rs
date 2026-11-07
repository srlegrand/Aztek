//! The 3D view: what to draw from the scene, its GPU buffers, picking, and the move/rotate/scale manipulators.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::render_mesh::{build_render_mesh, RenderMeshOptions};
use aztek_geo::{Index, Mesh};
use aztek_scene::component_pick::{self, Screen};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::graph::Graph;
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::plug::{NodeId, NO_NODE};
use aztek_scene::transform::DagNode;
use aztek_scene::Session;
use ash::vk;
use glam::{DMat4, DVec2, DVec3, Vec3};

use crate::camera::Camera;
use crate::gpu::{Buffer, Gpu};
use crate::render::{c8, Kind, LineVertex, Push, Renderer, ShadedVertex};

pub struct DrawItem {
    pub shape: NodeId,
    pub xf: NodeId,
    pub world: DMat4,
    pub mesh: Arc<Mesh>,
}

fn visible(g: &Graph, id: NodeId) -> bool {
    let mut n = id;
    while n != NO_NODE {
        let Some(node) = g.node(n) else { return false };
        if node.ty().dag && !node.bool(DagNode::VISIBILITY) {
            return false;
        }
        if aztek_scene::draw_style::hidden_by_override(g, n) {
            return false;
        }
        n = g.parent(n);
    }
    true
}

/// Every visible, non-intermediate mesh shape with its evaluated mesh.
pub fn gather(g: &mut Graph) -> Vec<DrawItem> {
    g.evaluate();
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if !n.ty().is_a("mesh") || n.bool(DagNode::INTERMEDIATE_OBJECT) || !visible(g, id) {
            continue;
        }
        let Some(mesh) = n.data().mesh(MeshNode::OUT_MESH).cloned() else { continue };
        let world = n.data().matrix(DagNode::WORLD_MATRIX);
        out.push(DrawItem { shape: id, xf: aztek_scene::dag::transform_of(g, id), world, mesh });
    }
    out
}

struct GpuMesh {
    mesh_key: usize,
    tris: Option<(Buffer, Buffer, u32)>,
    /// Per triangle source face, for the selected-face tint.
    tri_faces: Vec<Index>,
    tri_indices: Vec<Index>,
    bvh: Option<MeshBvh>,
    sel_key: u64,
    lines: Option<(Buffer, u32)>,
    points: Option<(Buffer, u32)>,
    tint: Option<(Buffer, u32)>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Move,
    Rotate,
    Scale,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Handle {
    Axis(usize),
    /// Move in the view plane / uniform scale.
    Center,
}

pub struct Drag {
    pub handle: Handle,
    pub start: DVec2,
    pub pivot: DVec3,
    /// Whether a preview command has run and must be undone before the next.
    pub applied: bool,
    pub components: bool,
    pub targets: Vec<String>,
}

pub struct Viewport {
    pub cam: Camera,
    meshes: HashMap<NodeId, GpuMesh>,
    grid: Option<(Buffer, u32)>,
    pub wireframe: bool,
    pub wire_on_shaded: bool,
    pub tool: Tool,
    pub drag: Option<Drag>,
    pub hover: Option<Handle>,
    /// Viewport rect in physical pixels: x, y, w, h.
    pub rect: [f64; 4],
    pub marquee: Option<(DVec2, DVec2)>,
    pub grid_spacing: f64,
}

const LEAD: [u8; 4] = [66, 255, 163, 255];
const SELECTED: [u8; 4] = [255, 255, 255, 255];
const WIRE: [u8; 4] = [10, 10, 30, 255];
const VERT: [u8; 4] = [170, 30, 170, 255];
const SEL_COMP: [u8; 4] = [255, 160, 30, 255];
const UNSEL_EDGE: [u8; 4] = [40, 70, 200, 255];

impl Viewport {
    pub fn new() -> Viewport {
        Viewport {
            cam: Camera::default(),
            meshes: HashMap::new(),
            grid: None,
            wireframe: false,
            wire_on_shaded: false,
            tool: Tool::Select,
            drag: None,
            hover: None,
            rect: [0.0, 0.0, 1.0, 1.0],
            marquee: None,
            grid_spacing: 1.0,
        }
    }

    pub fn size(&self) -> DVec2 {
        DVec2::new(self.rect[2].max(1.0), self.rect[3].max(1.0))
    }

    pub fn aspect(&self) -> f64 {
        self.rect[2].max(1.0) / self.rect[3].max(1.0)
    }

    pub fn view_proj(&self) -> DMat4 {
        self.cam.proj(self.aspect()) * self.cam.view()
    }

    pub fn screen(&self) -> Screen {
        Screen { view_proj: self.view_proj(), width: self.rect[2], height: self.rect[3] }
    }

    fn grid_lines(&self) -> Vec<LineVertex> {
        let n = 12i32;
        let s = self.grid_spacing as f32;
        let ext = n as f32 * s;
        let mut v = Vec::new();
        for i in -n..=n {
            let t = i as f32 * s;
            let c = if i == 0 { [20, 20, 20, 255] } else if i % 5 == 0 { [95, 95, 95, 255] } else { [120, 120, 120, 255] };
            v.push(LineVertex { pos: [t, 0.0, -ext], color: c8(c) });
            v.push(LineVertex { pos: [t, 0.0, ext], color: c8(c) });
            v.push(LineVertex { pos: [-ext, 0.0, t], color: c8(c) });
            v.push(LineVertex { pos: [ext, 0.0, t], color: c8(c) });
        }
        v
    }

    /// Drops GPU buffers of shapes no longer drawn.
    fn prune(&mut self, gpu: &mut Gpu, items: &[DrawItem]) {
        let live: Vec<NodeId> = items.iter().map(|i| i.shape).collect();
        let dead: Vec<NodeId> = self.meshes.keys().copied().filter(|k| !live.contains(k)).collect();
        for k in dead {
            let m = self.meshes.remove(&k).unwrap();
            drop_mesh(gpu, m);
        }
    }

    pub fn draw(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session, items: &[DrawItem], scale: f64) {
        self.prune(gpu, items);
        let [x, y, w, h] = self.rect;
        if w < 2.0 || h < 2.0 {
            return;
        }
        r.set_viewport(gpu, x as f32, y as f32, w as f32, h as f32);
        let vp = self.view_proj();
        let view = self.cam.view();
        if self.grid.is_none() {
            let lines = self.grid_lines();
            self.grid = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lines), lines.len() as u32));
        }
        r.bind(gpu, Kind::Lines);
        r.set_line_width(gpu, 1.0);
        r.push(gpu, &push(vp, DMat4::IDENTITY, view, [1.0; 4], 0.0, 1.0));
        let (gb, gn) = self.grid.as_ref().unwrap();
        r.draw(gpu, gb, None, *gn);

        let g = &s.graph;
        let mode = components::mode(g);
        let sel = g.selection().to_vec();
        let lead = sel.last().copied();
        for it in items {
            let entry = self.meshes.entry(it.shape).or_insert_with(|| GpuMesh {
                mesh_key: 0,
                tris: None,
                tri_faces: Vec::new(),
                tri_indices: Vec::new(),
                bvh: None,
                sel_key: u64::MAX,
                lines: None,
                points: None,
                tint: None,
            });
            let key = Arc::as_ptr(&it.mesh) as usize;
            let mesh_changed = entry.mesh_key != key;
            if mesh_changed {
                if let Some((a, b, _)) = entry.tris.take() {
                    gpu.drop_buffer(a);
                    gpu.drop_buffer(b);
                }
                let rm = build_render_mesh(&it.mesh, &RenderMeshOptions { max_uv_sets: 0, ..Default::default() });
                let verts: Vec<ShadedVertex> = rm
                    .positions
                    .iter()
                    .zip(&rm.normals)
                    .map(|(p, n)| ShadedVertex { pos: p.to_array(), nrm: n.to_array() })
                    .collect();
                if !rm.indices.is_empty() {
                    entry.tris = Some((
                        gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts),
                        gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &rm.indices),
                        rm.indices.len() as u32,
                    ));
                }
                entry.tri_faces = rm.triangle_faces.clone();
                entry.tri_indices = rm.indices.clone();
                entry.bvh = None;
                entry.mesh_key = key;
                entry.sel_key = u64::MAX;
                // Tint reuses the triangle vertex buffer; rebuilt with the selection key below.
            }
            let selected = sel.contains(&it.xf) || sel.contains(&it.shape);
            let hilited = selected && mode != ComponentMode::Object;
            let comps: &[u32] = if hilited { components::selected(g, it.shape, mode) } else { &[] };
            let comp_set: std::collections::HashSet<u32> = comps.iter().copied().collect();
            let mut h = DefaultHasher::new();
            (mode as i32, selected, lead == Some(it.xf), self.wireframe, self.wire_on_shaded, comps).hash(&mut h);
            let sel_key = h.finish();
            if sel_key != entry.sel_key {
                entry.sel_key = sel_key;
                for b in [entry.lines.take(), entry.points.take(), entry.tint.take()].into_iter().flatten() {
                    gpu.drop_buffer(b.0);
                }
                let m = &it.mesh;
                let edge_color = |e: Index| -> [u8; 4] {
                    if hilited {
                        if mode == ComponentMode::Edge && comp_set.contains(&e) {
                            SEL_COMP
                        } else {
                            UNSEL_EDGE
                        }
                    } else if selected {
                        if lead == Some(it.xf) {
                            LEAD
                        } else {
                            SELECTED
                        }
                    } else {
                        WIRE
                    }
                };
                let want_lines = selected || self.wireframe || self.wire_on_shaded;
                if want_lines {
                    let mut lv = Vec::new();
                    for e in 0..m.edge_capacity() {
                        if !m.edge_valid(e) {
                            continue;
                        }
                        let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
                        let c = edge_color(e);
                        lv.push(LineVertex { pos: m.position(a).as_vec3().to_array(), color: c8(c) });
                        lv.push(LineVertex { pos: m.position(b).as_vec3().to_array(), color: c8(c) });
                    }
                    if !lv.is_empty() {
                        entry.lines = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lv), lv.len() as u32));
                    }
                }
                if hilited && mode == ComponentMode::Vertex {
                    let mut pv = Vec::new();
                    for p in 0..m.point_capacity() {
                        if m.point_valid(p) {
                            let c = if comp_set.contains(&p) { SEL_COMP } else { VERT };
                            pv.push(LineVertex { pos: m.position(p).as_vec3().to_array(), color: c8(c) });
                        }
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                }
                if hilited && mode == ComponentMode::Face && !comps.is_empty() {
                    let mut ti = Vec::new();
                    for (t, &f) in entry.tri_faces.iter().enumerate() {
                        if comp_set.contains(&f) {
                            ti.extend_from_slice(&entry.tri_indices[t * 3..t * 3 + 3]);
                        }
                    }
                    if !ti.is_empty() {
                        entry.tint = Some((gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &ti), ti.len() as u32));
                    }
                }
            }
            let mvp = vp * it.world;
            if !self.wireframe {
                if let Some((vb, ib, n)) = &entry.tris {
                    r.bind(gpu, Kind::Shaded);
                    r.push(gpu, &push(mvp, it.world, view, [0.5, 0.5, 0.5, 1.0], 0.0, 1.0));
                    r.draw(gpu, vb, Some((ib, *n)), 0);
                    if let Some((ti, tn)) = &entry.tint {
                        r.bind(gpu, Kind::Tint);
                        r.push(gpu, &push(mvp, it.world, view, [1.0, 0.45, 0.05, 0.45], 2e-4, 1.0));
                        r.draw(gpu, vb, Some((ti, *tn)), 0);
                    }
                }
            }
            if let Some((lb, ln)) = &entry.lines {
                r.bind(gpu, Kind::Lines);
                r.set_line_width(gpu, if selected && mode == ComponentMode::Object { 1.5 } else { 1.0 });
                r.push(gpu, &push(mvp, it.world, view, [1.0; 4], 4e-4, 1.0));
                r.draw(gpu, lb, None, *ln);
            }
            if let Some((pb, pn)) = &entry.points {
                r.bind(gpu, Kind::Points);
                r.push(gpu, &push(mvp, it.world, view, [1.0; 4], 8e-4, (5.0 * scale) as f32));
                r.draw(gpu, pb, None, *pn);
            }
        }
        self.draw_manip(gpu, r, s, scale);
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Picking

    fn bvh_for(&mut self, it: &DrawItem) -> Option<&MeshBvh> {
        let e = self.meshes.get_mut(&it.shape)?;
        if e.bvh.is_none() {
            e.bvh = Some(MeshBvh::new(&it.mesh));
        }
        e.bvh.as_ref()
    }

    /// The nearest mesh hit under a viewport pixel: (item index, face, world point).
    pub fn raycast(&mut self, items: &[DrawItem], px: DVec2) -> Option<(usize, Index, DVec3)> {
        let (o, d) = self.cam.ray(px, self.size());
        let mut best: Option<(usize, Index, DVec3, f64)> = None;
        for (i, it) in items.iter().enumerate() {
            let inv = it.world.inverse();
            let lo = inv.transform_point3(o);
            let ld = inv.transform_vector3(d);
            let len = ld.length();
            if len < 1e-12 {
                continue;
            }
            let Some(bvh) = self.bvh_for(it) else { continue };
            if let Some(hit) = bvh.raycast_with(lo, ld / len, f64::MAX, true) {
                let wp = it.world.transform_point3(lo + ld / len * hit.t);
                let dist = (wp - o).length();
                if best.as_ref().is_none_or(|b| dist < b.3) {
                    best = Some((i, hit.face, wp, dist));
                }
            }
        }
        best.map(|b| (b.0, b.1, b.2))
    }

    /// What a click at px picks, as select arguments: object names or component strings.
    pub fn pick(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Vec<String> {
        let g = &s.graph;
        let mode = components::mode(g);
        if mode == ComponentMode::Object {
            return self
                .raycast(items, px)
                .map(|(i, _, _)| vec![g.path(items[i].xf)])
                .unwrap_or_default();
        }
        let sel = g.selection();
        let hil: Vec<usize> = (0..items.len()).filter(|&i| sel.contains(&items[i].xf)).collect();
        if mode == ComponentMode::Face {
            let hit_items: Vec<DrawItem> =
                hil.iter().map(|&i| DrawItem { shape: items[i].shape, xf: items[i].xf, world: items[i].world, mesh: items[i].mesh.clone() }).collect();
            return self
                .raycast(&hit_items, px)
                .map(|(i, f, _)| vec![format!("{}.f[{f}]", g.path(hit_items[i].xf))])
                .unwrap_or_default();
        }
        let screen = self.screen();
        let mut best: Option<(f64, String)> = None;
        for &i in &hil {
            let it = &items[i];
            let c = component_pick::nearby(&it.mesh, &it.world, mode, &screen, px, component_pick::radius(mode));
            if let Some(c) = c.first() {
                if best.as_ref().is_none_or(|b| c.px < b.0) {
                    let p = if mode == ComponentMode::Vertex { "vtx" } else { "e" };
                    best = Some((c.px, format!("{}.{p}[{}]", g.path(it.xf), c.id)));
                }
            }
        }
        best.map(|b| vec![b.1]).unwrap_or_default()
    }

    /// Everything inside a viewport-pixel rectangle.
    pub fn pick_rect(&self, s: &Session, items: &[DrawItem], a: DVec2, b: DVec2) -> Vec<String> {
        let (lo, hi) = (a.min(b), a.max(b));
        let inside = |p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y;
        let g = &s.graph;
        let mode = components::mode(g);
        let screen = self.screen();
        let mut out = Vec::new();
        for it in items {
            let m = &it.mesh;
            let proj = |p: Index| screen.project(it.world.transform_point3(m.position(p)));
            match mode {
                ComponentMode::Object => {
                    if (0..m.point_capacity()).any(|p| m.point_valid(p) && proj(p).is_some_and(inside)) {
                        out.push(g.path(it.xf));
                    }
                }
                _ if !g.selection().contains(&it.xf) => {}
                ComponentMode::Vertex => {
                    let ids: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.point_valid(p) && proj(p).is_some_and(inside)).collect();
                    push_ranges(&mut out, &g.path(it.xf), "vtx", &ids);
                }
                ComponentMode::Edge => {
                    let ids: Vec<Index> = (0..m.edge_capacity())
                        .filter(|&e| {
                            m.edge_valid(e) && {
                                let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
                                screen.project(it.world.transform_point3((m.position(a) + m.position(b)) * 0.5)).is_some_and(inside)
                            }
                        })
                        .collect();
                    push_ranges(&mut out, &g.path(it.xf), "e", &ids);
                }
                ComponentMode::Face => {
                    let ids: Vec<Index> = (0..m.face_capacity())
                        .filter(|&f| {
                            m.face_valid(f) && {
                                let pts = m.face_points(f);
                                let c = pts.iter().map(|&p| m.position(p)).sum::<DVec3>() / pts.len().max(1) as f64;
                                screen.project(it.world.transform_point3(c)).is_some_and(inside)
                            }
                        })
                        .collect();
                    push_ranges(&mut out, &g.path(it.xf), "f", &ids);
                }
            }
        }
        out
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Manipulators

    /// Where the manipulator sits: the lead object's pivot, or the selected components' centre.
    pub fn manip_pivot(&self, s: &mut Session, items: &[DrawItem]) -> Option<DVec3> {
        if self.tool == Tool::Select {
            return None;
        }
        let mode = components::mode(&s.graph);
        if mode == ComponentMode::Object {
            let lead = *s.graph.selection().last()?;
            return Some(aztek_scene::transform_ops::world_pivot(&mut s.graph, lead));
        }
        let g = &s.graph;
        let (mut sum, mut n) = (DVec3::ZERO, 0.0);
        for it in items {
            if !g.selection().contains(&it.xf) {
                continue;
            }
            let ids = components::selected(g, it.shape, mode);
            if ids.is_empty() {
                continue;
            }
            let pts = components::convert(&it.mesh, mode, ids, ComponentMode::Vertex, false);
            for p in pts {
                sum += it.world.transform_point3(it.mesh.position(p));
                n += 1.0;
            }
        }
        (n > 0.0).then(|| sum / n)
    }

    /// World units per pixel at a point.
    fn units_per_px(&self, at: DVec3) -> f64 {
        let d = (at - self.cam.eye()).length().max(1e-6);
        2.0 * d * (self.cam.vfov(self.aspect()) * 0.5).tan() / self.rect[3].max(1.0)
    }

    pub fn handle_len(&self, pivot: DVec3, scale: f64) -> f64 {
        self.units_per_px(pivot) * 100.0 * scale
    }

    fn draw_manip(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session, scale: f64) {
        let items = gather(&mut s.graph);
        let Some(pivot) = self.manip_pivot(s, &items) else { return };
        let len = self.handle_len(pivot, scale) as f32;
        let p = pivot.as_vec3();
        let colors = [[230, 40, 40, 255], [40, 230, 40, 255], [40, 80, 255, 255]];
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hover);
        let mut v = Vec::new();
        let axes = [Vec3::X, Vec3::Y, Vec3::Z];
        match self.tool {
            Tool::Rotate => {
                for (i, ax) in axes.iter().enumerate() {
                    let c = if active == Some(Handle::Axis(i)) { [255, 255, 0, 255] } else { colors[i] };
                    let (u, w) = ax.any_orthonormal_pair();
                    let n = 64;
                    for k in 0..n {
                        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
                        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
                        v.push(LineVertex { pos: (p + (u * a0.cos() + w * a0.sin()) * len).to_array(), color: c8(c) });
                        v.push(LineVertex { pos: (p + (u * a1.cos() + w * a1.sin()) * len).to_array(), color: c8(c) });
                    }
                }
            }
            _ => {
                for (i, ax) in axes.iter().enumerate() {
                    let c = if active == Some(Handle::Axis(i)) { [255, 255, 0, 255] } else { colors[i] };
                    let tip = p + *ax * len;
                    v.push(LineVertex { pos: p.to_array(), color: c8(c) });
                    v.push(LineVertex { pos: tip.to_array(), color: c8(c) });
                    // A small cross at the tip: arrow for move, box for scale.
                    let (u, w) = ax.any_orthonormal_pair();
                    let k = len * 0.08;
                    let base = if self.tool == Tool::Move { tip - *ax * len * 0.2 } else { tip };
                    for d in [u, -u, w, -w] {
                        v.push(LineVertex { pos: tip.to_array(), color: c8(c) });
                        v.push(LineVertex { pos: (base + d * k).to_array(), color: c8(c) });
                    }
                }
                let c = if active == Some(Handle::Center) { [255, 255, 0, 255] } else { [230, 230, 120, 255] };
                let view = self.cam.view();
                let right = Vec3::new(view.x_axis.x as f32, view.y_axis.x as f32, view.z_axis.x as f32);
                let up = Vec3::new(view.x_axis.y as f32, view.y_axis.y as f32, view.z_axis.y as f32);
                let k = len * 0.1;
                let corners = [p + (right + up) * k, p + (up - right) * k, p - (right + up) * k, p + (right - up) * k];
                for i in 0..4 {
                    v.push(LineVertex { pos: corners[i].to_array(), color: c8(c) });
                    v.push(LineVertex { pos: corners[(i + 1) % 4].to_array(), color: c8(c) });
                }
            }
        }
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
        r.bind(gpu, Kind::LinesOnTop);
        r.set_line_width(gpu, 2.0);
        r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
        r.draw(gpu, &b, None, v.len() as u32);
        gpu.drop_buffer(b);
    }

    /// The manipulator handle under a pixel.
    pub fn manip_hit(&self, pivot: DVec3, px: DVec2, scale: f64) -> Option<Handle> {
        let screen = self.screen();
        let len = self.handle_len(pivot, scale);
        let c = screen.project(pivot)?;
        if self.tool != Tool::Rotate && (px - c).abs().max_element() < 10.0 * scale {
            return Some(Handle::Center);
        }
        let mut best: Option<(f64, Handle)> = None;
        for i in 0..3 {
            let ax = DVec3::AXES[i];
            let d = if self.tool == Tool::Rotate {
                let (u, w) = ax.any_orthonormal_pair();
                (0..48)
                    .filter_map(|k| {
                        let a = k as f64 / 48.0 * std::f64::consts::TAU;
                        let b = (k + 1) as f64 / 48.0 * std::f64::consts::TAU;
                        let (pa, pb) = screen.project_segment(pivot + (u * a.cos() + w * a.sin()) * len, pivot + (u * b.cos() + w * b.sin()) * len)?;
                        Some(seg_dist(px, pa, pb))
                    })
                    .fold(f64::MAX, f64::min)
            } else {
                let Some((pa, pb)) = screen.project_segment(pivot, pivot + ax * len) else { continue };
                seg_dist(px, pa, pb)
            };
            if d < 8.0 * scale && best.is_none_or(|b| d < b.0) {
                best = Some((d, Handle::Axis(i)));
            }
        }
        best.map(|b| b.1)
    }

    /// The command for a drag from d.start to px: (command, call).
    pub fn drag_call(&self, d: &Drag, px: DVec2, scale: f64) -> Option<(String, serde_json::Value)> {
        let screen = self.screen();
        let c = screen.project(d.pivot)?;
        let delta = px - d.start;
        let upp = self.units_per_px(d.pivot);
        let comps = d.components;
        let targets = serde_json::json!(d.targets);
        match self.tool {
            Tool::Move => {
                let world = match d.handle {
                    Handle::Axis(i) => {
                        let ax = DVec3::AXES[i];
                        let tip = screen.project(d.pivot + ax * upp * 100.0)?;
                        let sdir = tip - c;
                        let l2 = sdir.length_squared();
                        if l2 < 1e-6 {
                            return None;
                        }
                        ax * (delta.dot(sdir) / l2) * upp * 100.0
                    }
                    Handle::Center => {
                        let view = self.cam.view();
                        let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
                        let up = DVec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
                        (right * delta.x - up * delta.y) * upp
                    }
                };
                let by = serde_json::json!(world.to_array());
                Some(if comps {
                    ("moveComponents".into(), serde_json::json!({ "by": by }))
                } else {
                    ("move".into(), serde_json::json!({ "args": targets, "by": by }))
                })
            }
            Tool::Rotate => {
                let Handle::Axis(i) = d.handle else { return None };
                // Angle swept about the pivot on screen.
                let a0 = (d.start - c).to_angle();
                let a1 = (px - c).to_angle();
                let mut ang = (a1 - a0).to_degrees();
                // Screen y runs down: flip so the ring turns the way the cursor does when the axis faces the camera.
                let toward = (self.cam.eye() - d.pivot).dot(DVec3::AXES[i]).signum();
                ang *= -toward;
                let mut by = [0.0; 3];
                by[i] = ang;
                Some(if comps {
                    ("rotateComponents".into(), serde_json::json!({ "by": by, "pivot": d.pivot.to_array() }))
                } else {
                    ("rotate".into(), serde_json::json!({ "args": targets, "by": by }))
                })
            }
            Tool::Scale => {
                let f = (1.0 + (delta.x - delta.y) / (100.0 * scale)).max(0.001);
                let by = match d.handle {
                    Handle::Axis(i) => {
                        let mut b = [1.0; 3];
                        b[i] = f;
                        b
                    }
                    Handle::Center => [f; 3],
                };
                Some(if comps {
                    ("scaleComponents".into(), serde_json::json!({ "by": by, "pivot": d.pivot.to_array() }))
                } else {
                    ("scale".into(), serde_json::json!({ "args": targets, "by": by }))
                })
            }
            Tool::Select => None,
        }
    }

    /// Frames the selection (or everything when nothing is selected or all is true).
    pub fn frame(&mut self, s: &mut Session, items: &[DrawItem], all: bool) {
        let g = &s.graph;
        let mode = components::mode(g);
        let mut lo = DVec3::splat(f64::MAX);
        let mut hi = DVec3::splat(f64::MIN);
        let sel = g.selection().to_vec();
        let mut add = |p: DVec3| {
            lo = lo.min(p);
            hi = hi.max(p);
        };
        let mut any = false;
        for it in items {
            let picked = sel.contains(&it.xf);
            if !all && !sel.is_empty() && !picked {
                continue;
            }
            let ids: Vec<Index> = if !all && picked && mode != ComponentMode::Object {
                let c = components::selected(g, it.shape, mode);
                if c.is_empty() {
                    (0..it.mesh.point_capacity()).filter(|&p| it.mesh.point_valid(p)).collect()
                } else {
                    components::convert(&it.mesh, mode, c, ComponentMode::Vertex, false)
                }
            } else {
                (0..it.mesh.point_capacity()).filter(|&p| it.mesh.point_valid(p)).collect()
            };
            for p in ids {
                add(it.world.transform_point3(it.mesh.position(p)));
                any = true;
            }
        }
        if !any {
            if all || sel.is_empty() {
                let aspect = self.aspect();
                self.cam.frame(DVec3::ZERO, self.grid_spacing * 12.0, aspect);
            }
            return;
        }
        let aspect = self.aspect();
        self.cam.frame((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1e-3), aspect);
    }

    pub fn release(&mut self, gpu: &mut Gpu) {
        for (_, m) in self.meshes.drain() {
            drop_mesh(gpu, m);
        }
        if let Some((b, _)) = self.grid.take() {
            gpu.drop_buffer(b);
        }
    }
}

fn drop_mesh(gpu: &mut Gpu, m: GpuMesh) {
    if let Some((a, b, _)) = m.tris {
        gpu.drop_buffer(a);
        gpu.drop_buffer(b);
    }
    for b in [m.lines, m.points, m.tint].into_iter().flatten() {
        gpu.drop_buffer(b.0);
    }
}

fn push_ranges(out: &mut Vec<String>, path: &str, p: &str, ids: &[Index]) {
    if ids.is_empty() {
        return;
    }
    let kind = match p {
        "vtx" => ComponentMode::Vertex,
        "e" => ComponentMode::Edge,
        _ => ComponentMode::Face,
    };
    for c in aztek_scene::history::format_components(kind, ids).split_whitespace() {
        out.push(format!("{path}.{c}"));
    }
}

fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * t - p).length()
}

fn push(mvp: DMat4, model: DMat4, view: DMat4, color: [f32; 4], bias: f32, point: f32) -> Push {
    let nm = (view * model).inverse().transpose();
    let m = nm.as_mat4();
    // Rows of the upper 3x3.
    let row = |i: usize| [m.x_axis[i], m.y_axis[i], m.z_axis[i]];
    let (r0, r1, r2) = (row(0), row(1), row(2));
    Push {
        mvp: mvp.as_mat4().to_cols_array(),
        n0: [r0[0], r0[1], r0[2], bias],
        n1: [r1[0], r1[1], r1[2], point],
        n2: [r2[0], r2[1], r2[2], 0.0],
        color,
    }
}
