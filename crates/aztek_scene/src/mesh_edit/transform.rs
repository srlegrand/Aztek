//! Selection centres, normals and frames, soft selection weights, and
//! moving, rotating and scaling selected components.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap};
use std::cmp::Reverse;
use std::sync::{Arc, Mutex, RwLock, Weak};

use aztek_core::PrefStore;
use aztek_geo::bvh::MeshBvh;
use aztek_geo::symmetry::SymmetryMap;
use aztek_geo::{names, Index, Mesh};
use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec2, DVec3};
use rayon::prelude::*;
use serde_json::Value;

use super::*;
use crate::components::{self, ComponentMode, Ids, PointProvider};
use crate::graph::Graph;
use crate::mesh_nodes::{symmetry_spec, MeshNode};
use crate::modifiers;
use crate::plug::{MeshPtr, NodeId, PlugValue};
use crate::session::Session;
use crate::soft_select;
use crate::transform::{euler_to_matrix, RotateOrder, Transform};

/// Where a point moves: from its index and rest position (object space) to its target.
pub type PointTarget = Arc<dyn Fn(u32, DVec3) -> DVec3 + Send + Sync>;
/// Sets up the targets for one mesh's selection; None skips the mesh.
pub type PointTargetSetup<'a> = dyn Fn(&Mesh, &DMat4, ComponentMode, &Ids) -> Option<PointTarget> + 'a;
/// Maps a world point onto a surface.
pub type SurfaceProjector = Arc<dyn Fn(DVec3) -> DVec3 + Send + Sync>;

/// How many pieces the points make, joined by the mesh's edges between them.
fn selection_pieces(m: &Mesh, points: &[Index]) -> i32 {
    let mut open: BTreeSet<Index> = points.iter().copied().collect();
    let mut pieces = 0;
    while let Some(first) = open.pop_first() {
        pieces += 1;
        let mut stack = vec![first];
        while let Some(p) = stack.pop() {
            m.for_each_point_edge(p, |e| {
                let q = m.edge_other_point(e, p);
                if open.remove(&q) {
                    stack.push(q);
                }
            });
        }
    }
    pieces
}

/// The wedge holding most of points; the one last picked with symmetry when it holds as many.
fn drive_wedge(g: &Graph, node: NodeId, sym: &SymmetryMap, points: &[Index]) -> i32 {
    let picked = components::picked_wedge(g, node);
    if picked < 0 || picked >= sym.order() {
        return sym.majority_wedge(points);
    }
    let mut count = vec![0i64; sym.order() as usize];
    for &p in points {
        let w = sym.wedge(p);
        if w >= 0 {
            count[w as usize] += 1;
        }
    }
    if count[picked as usize] == *count.iter().max().unwrap() {
        picked
    } else {
        sym.majority_wedge(points)
    }
}

#[derive(PartialEq)]
struct Dist(f64);
impl PartialOrd for Dist {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Eq for Dist {}
impl Ord for Dist {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&o.0)
    }
}

/// World-space distance along the edges from the nearest selected point, up to limit (infinity beyond it).
fn surface_distances(m: &Mesh, world: &DMat4, selected: &[Index], limit: f64) -> Vec<f64> {
    let mut dist = vec![f64::INFINITY; m.point_capacity() as usize];
    let mut open = BinaryHeap::new();
    for &p in selected {
        if (p as usize) < dist.len() && m.point_valid(p) {
            dist[p as usize] = 0.0;
            open.push(Reverse((Dist(0.0), p)));
        }
    }
    let wp = |p: Index| world.transform_point3(m.position(p));
    while let Some(Reverse((Dist(d), p))) = open.pop() {
        if d > dist[p as usize] {
            continue;
        }
        let at = wp(p);
        m.for_each_point_edge(p, |e| {
            let q = if m.edge_point(e, 0) == p { m.edge_point(e, 1) } else { m.edge_point(e, 0) };
            let nd = d + (wp(q) - at).length();
            if nd < dist[q as usize] && nd < limit {
                dist[q as usize] = nd;
                open.push(Reverse((Dist(nd), q)));
            }
        });
    }
    dist
}

fn is_mesh(g: &Graph, id: NodeId) -> bool {
    components::is_mesh(g, id)
}

struct CenterCache {
    cage: Weak<Mesh>,
    serial: u64,
    kind: ComponentMode,
    sum: DVec3,
    count: usize,
}

static CENTER_CACHE: Mutex<Option<HashMap<NodeId, CenterCache>>> = Mutex::new(None);

/// World centre of the selected components; zero in object mode.
pub fn selection_center(g: &mut Graph) -> DVec3 {
    g.evaluate();
    let kind = components::mode(g);
    let (mut sum, mut count) = (DVec3::ZERO, 0usize);
    if kind == ComponentMode::Object {
        return sum;
    }
    for (node, ids) in components::selected_all(g, kind).clone() {
        let Some(n) = g.node(node) else { continue };
        let w = n.matrix(Transform::WORLD_MATRIX);
        if let Some(pp) = components::point_provider(n) {
            let want: BTreeSet<u32> = ids.iter().copied().collect();
            if let Some(points) = &pp.points {
                for (id, p) in points(g, node) {
                    if want.contains(&id) {
                        sum += w.transform_point3(p);
                        count += 1;
                    }
                }
            }
            continue;
        }
        if !is_mesh(g, node) {
            continue;
        }
        let Some(cage) = modifiers::cage(g, node) else { continue };
        // The object-space sum only changes with the mesh or the selection; the manipulator asks every frame.
        let serial = components::serial(g);
        let mut guard = CENTER_CACHE.lock().unwrap();
        let cache = guard.get_or_insert_with(HashMap::new);
        let c = cache.entry(node).or_insert(CenterCache { cage: Weak::new(), serial: 0, kind: ComponentMode::Object, sum: DVec3::ZERO, count: 0 });
        if !c.cage.upgrade().is_some_and(|m| Arc::ptr_eq(&m, &cage)) || c.serial != serial || c.kind != kind {
            *c = CenterCache { cage: Arc::downgrade(&cage), serial, kind, sum: DVec3::ZERO, count: 0 };
            for p in components::convert(&cage, kind, &ids, ComponentMode::Vertex, false) {
                if cage.point_valid(p) {
                    c.sum += cage.position(p);
                    c.count += 1;
                }
            }
        }
        // w * (sum, count): the count weights the translation.
        sum += w.transform_vector3(c.sum) + w.w_axis.truncate() * c.count as f64;
        count += c.count;
    }
    if count > 0 {
        sum / count as f64
    } else {
        sum
    }
}

/// selection_center over the driving side only: with symmetry, a pair clear of the plane centres on the side that
/// drives.
pub fn symmetric_selection_center(g: &mut Graph) -> DVec3 {
    g.evaluate();
    let kind = components::mode(g);
    if kind == ComponentMode::Object {
        return DVec3::ZERO;
    }
    let (mut sum, mut count) = (DVec3::ZERO, 0usize);
    for (node, ids) in components::selected_all(g, kind).clone() {
        let cage = if is_mesh(g, node) { modifiers::cage(g, node) } else { None };
        let Some(cage) = cage else { return selection_center(g) };
        let w = g.node(node).unwrap().matrix(Transform::WORLD_MATRIX);
        let side = driving_side(g, node, kind, &ids, Some(cage.clone()));
        for p in components::convert(&cage, kind, &side, ComponentMode::Vertex, false) {
            if cage.point_valid(p) {
                sum += w.transform_point3(cage.position(p));
                count += 1;
            }
        }
    }
    if count > 0 {
        sum / count as f64
    } else {
        selection_center(g)
    }
}

fn driving_side_of(g: &Graph, node: NodeId, mesh: &Mesh, kind: ComponentMode, ids: &Ids) -> Ids {
    let mut m = mesh.clone();
    let Some(sym) = symmetry_for(g, node, &mut m) else { return ids.clone() };
    if ids.is_empty() {
        return ids.clone();
    }
    let pts = components::convert(&m, kind, ids, ComponentMode::Vertex, false);
    // Radial: a ring round the axis is one connected selection and stays whole; separate pieces (a petal in every
    // wedge) each act in place.
    if pts.iter().any(|&p| sym.on_plane(p)) || (sym.is_radial() && selection_pieces(&m, &pts) <= 1) {
        return ids.clone();
    }
    let drive = drive_wedge(g, node, &sym, &pts);
    let mut out = Vec::new();
    for &id in ids {
        let own = if kind == ComponentMode::Vertex { vec![id] } else { components::convert(&m, kind, &[id], ComponentMode::Vertex, false) };
        if own.first().is_some_and(|&p| sym.wedge(p) == drive) {
            out.push(id);
        }
    }
    if out.is_empty() {
        ids.clone()
    } else {
        out
    }
}

struct SideCache {
    mesh: Weak<Mesh>,
    kind: ComponentMode,
    spec: (i32, u64, i32),
    picked: i32,
    ids: Ids,
    out: Ids,
}

static SIDE_CACHE: Mutex<Vec<((NodeId, usize), SideCache)>> = Mutex::new(Vec::new());

/// With symmetry, the part of a selection on the side (wedge) that drives edits; the whole selection when it touches
/// the plane, is one radial ring, or symmetry is off.
pub fn driving_side(g: &mut Graph, node: NodeId, kind: ComponentMode, ids: &Ids, mesh: Option<MeshPtr>) -> Ids {
    if !is_mesh(g, node) || kind == ComponentMode::Object {
        return ids.clone();
    }
    let mesh = match mesh {
        Some(m) => Some(m),
        None => modifiers::cage(g, node),
    };
    let spec = symmetry_spec(g, node);
    let (Some(mesh), Some(spec)) = (mesh, spec) else { return ids.clone() };
    let key = (spec.axis, spec.offset.to_bits(), spec.radial.as_ref().map_or(2, |r| r.order()));
    let picked = components::picked_wedge(g, node);
    // Building the symmetry map is the cost, and gizmos ask every frame.
    let mut cache = SIDE_CACHE.lock().unwrap();
    cache.retain(|(_, c)| c.mesh.strong_count() > 0);
    let slot = (node, Arc::as_ptr(&mesh) as usize);
    let at = match cache.iter().position(|(k, _)| *k == slot) {
        Some(i) => i,
        None => {
            cache.push((slot, SideCache { mesh: Weak::new(), kind: ComponentMode::Object, spec: (0, 0, 0), picked: -1, ids: Vec::new(), out: Vec::new() }));
            cache.len() - 1
        }
    };
    let c = &mut cache[at].1;
    if !c.mesh.upgrade().is_some_and(|m| Arc::ptr_eq(&m, &mesh)) || c.kind != kind || c.spec != key || c.picked != picked || &c.ids != ids {
        let out = driving_side_of(g, node, &mesh, kind, ids);
        *c = SideCache { mesh: Arc::downgrade(&mesh), kind, spec: key, picked, ids: ids.clone(), out };
    }
    c.out.clone()
}

/// Modulates soft selection weights by a field; registered by the fields module.
pub type SoftModulator = Arc<dyn Fn(&Graph, &Mesh, &DMat4, &mut Vec<f32>) + Send + Sync>;

static SOFT_MODULATOR: RwLock<Option<SoftModulator>> = RwLock::new(None);

pub fn set_soft_modulator(f: Option<SoftModulator>) {
    *SOFT_MODULATOR.write().unwrap() = f;
}

fn soft_enabled() -> bool {
    PrefStore::get().bool("general", soft_select::ENABLED, false)
}

fn soft_radius() -> f64 {
    PrefStore::get().f64("general", soft_select::RADIUS, 5.0)
}

/// World positions of every selected component: Global's falloff starts from all of them.
fn global_seeds(g: &Graph) -> Vec<DVec3> {
    let kind = components::mode(g);
    if kind == ComponentMode::Object {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (node, ids) in components::selected_all(g, kind) {
        let Some(n) = g.node(*node) else { continue };
        if !is_mesh(g, *node) {
            continue;
        }
        let Some(m) = modifiers::cage_cached(g, *node) else { continue };
        let world = n.matrix(Transform::WORLD_MATRIX);
        for p in components::convert(&m, kind, ids, ComponentMode::Vertex, false) {
            if m.point_valid(p) {
                out.push(world.transform_point3(m.position(p)));
            }
        }
    }
    out
}

fn falloff_weights(g: &Graph, m: &Mesh, world: &DMat4, selected: &[Index]) -> Vec<f32> {
    let mut w = vec![0.0f32; m.point_capacity() as usize];
    for &p in selected {
        if (p as usize) < w.len() {
            w[p as usize] = 1.0;
        }
    }
    let mode = soft_select::falloff_name();
    let global = mode == "global";
    if !soft_enabled() || (selected.is_empty() && !global) {
        return w;
    }
    let radius = soft_radius();
    if radius <= 0.0 {
        return w;
    }
    let curve = soft_select::current_curve();
    if soft_select::surface_falloff() {
        let dist = surface_distances(m, world, selected, radius);
        for p in 0..m.point_capacity() as usize {
            if w[p] < 1.0 && dist[p] < radius {
                w[p] = curve.evaluate(dist[p] / radius) as f32;
            }
        }
        return w;
    }
    let mut seeds: Vec<DVec3> = selected.iter().filter(|&&p| m.point_valid(p)).map(|&p| world.transform_point3(m.position(p))).collect();
    if global {
        seeds.extend(global_seeds(g));
    }
    if seeds.is_empty() {
        return w;
    }
    // Many seeds on a dense mesh: thin them out; the falloff is smooth, so a subset barely shows.
    let stride = (seeds.len() / 4096).max(1);
    let r2 = radius * radius;
    w.par_iter_mut().enumerate().for_each(|(p, wp)| {
        let p = p as Index;
        if !m.point_valid(p) || *wp >= 1.0 {
            return;
        }
        let q = world.transform_point3(m.position(p));
        let mut best = r2;
        for s in seeds.iter().step_by(stride) {
            best = best.min((q - *s).length_squared());
        }
        if best >= r2 {
            return;
        }
        *wp = curve.evaluate(best.sqrt() / radius) as f32;
    });
    w
}

/// Soft selection weights for loose world points, selected ones 1.
pub fn soft_weights_points(points: &[DVec3], selected: &[bool]) -> Vec<f32> {
    let mut w = vec![0.0f32; points.len()];
    let mut seeds = Vec::new();
    for (i, p) in points.iter().enumerate() {
        if selected.get(i).copied().unwrap_or(false) {
            w[i] = 1.0;
            seeds.push(*p);
        }
    }
    let radius = soft_radius();
    if !soft_enabled() || seeds.is_empty() || radius <= 0.0 {
        return w;
    }
    let curve = soft_select::current_curve();
    for (i, p) in points.iter().enumerate() {
        if w[i] >= 1.0 {
            continue;
        }
        let best = seeds.iter().fold(radius * radius, |b, s| b.min((*p - *s).length_squared()));
        if best < radius * radius {
            w[i] = curve.evaluate(best.sqrt() / radius) as f32;
        }
    }
    w
}

/// Soft selection weight of every point of a mesh: 1 on the selected points, the falloff around them.
pub fn soft_weights(g: &Graph, m: &Mesh, world: &DMat4, selected: &[Index]) -> Vec<f32> {
    let mut w = falloff_weights(g, m, world, selected);
    if let Some(f) = SOFT_MODULATOR.read().unwrap().clone() {
        f(g, m, world, &mut w);
    }
    w
}

/// Per point, the unit normal edits along normals push it by: a face selection's points use only the selected faces.
pub fn component_normals(m: &Mesh, kind: ComponentMode, ids: &[Index]) -> Vec<DVec3> {
    let mut n = vec![DVec3::ZERO; m.point_capacity() as usize];
    // A face selection's points take only the selected faces, so a region offsets as a whole.
    let mut from_selection = vec![false; m.point_capacity() as usize];
    if kind == ComponentMode::Face {
        for &f in ids {
            if m.face_valid(f) {
                let a = m.face_area_vector(f);
                m.for_each_face_corner(f, |c| {
                    n[m.corner_point(c) as usize] += a;
                    from_selection[m.corner_point(c) as usize] = true;
                });
            }
        }
    }
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            let a = m.face_area_vector(f);
            m.for_each_face_corner(f, |c| {
                if !from_selection[m.corner_point(c) as usize] {
                    n[m.corner_point(c) as usize] += a;
                }
            });
        }
    }
    for v in &mut n {
        let len = v.length();
        *v = if len > 1e-12 { *v / len } else { DVec3::ZERO };
    }
    n
}

/// An object-space point frame in world space, kept orthonormal under non-uniform scale; zero stays zero.
fn world_frame(f: &DMat3, linear: &DMat3) -> DMat3 {
    let n = linear.inverse().transpose() * f.z_axis;
    if n.length() < 1e-12 {
        return DMat3::ZERO;
    }
    let n = n.normalize();
    let mut u = *linear * f.x_axis;
    u -= n * u.dot(n);
    if u.length() < 1e-12 {
        return DMat3::ZERO;
    }
    let u = u.normalize();
    DMat3::from_cols(u, n.cross(u), n)
}

/// World frames of a point provider's given components, those without one left out.
fn provided_world_frames(g: &mut Graph, node: NodeId, pp: &PointProvider, ids: &[u32]) -> Vec<DMat3> {
    let Some(frames) = &pp.frames else { return Vec::new() };
    let linear = DMat3::from_mat4(g.node(node).unwrap().matrix(Transform::WORLD_MATRIX));
    frames(g, node, ids).iter().map(|f| world_frame(f, &linear)).filter(|w| w.z_axis.length() > 0.5).collect()
}

/// Ordered sum over chunks, so the same input always gives the same bits.
fn chunked_sum<T: Sync>(items: &[T], f: impl Fn(&T) -> DVec3 + Sync) -> DVec3 {
    let parts: Vec<DVec3> = items.par_chunks(4096).map(|c| c.iter().map(&f).fold(DVec3::ZERO, |a, b| a + b)).collect();
    parts.into_iter().fold(DVec3::ZERO, |a, b| a + b)
}

/// The selection's average world normal; +Y in object mode or when it has none.
pub fn selection_normal(g: &mut Graph) -> DVec3 {
    g.evaluate();
    let kind = components::mode(g);
    if kind == ComponentMode::Object {
        return DVec3::Y;
    }
    let mut sum = DVec3::ZERO;
    for (node, ids) in components::selected_all(g, kind).clone() {
        let Some(n) = g.node(node) else { continue };
        if let Some(pp) = components::point_provider(n) {
            for f in provided_world_frames(g, node, &pp, &ids) {
                sum += f.z_axis;
            }
            continue;
        }
        if !is_mesh(g, node) {
            continue;
        }
        let Some(cage) = modifiers::cage(g, node) else { continue };
        let m = &*cage;
        let to_world = DMat3::from_mat4(g.node(node).unwrap().matrix(Transform::WORLD_MATRIX)).inverse().transpose();
        let side = driving_side(g, node, kind, &ids, Some(cage.clone()));
        // Runs on every manipulator refresh, so only the selection's own faces are visited.
        let local = if kind == ComponentMode::Face {
            chunked_sum(&side, |&f| if m.face_valid(f) { m.face_area_vector(f) } else { DVec3::ZERO })
        } else {
            let points = components::convert(m, kind, &side, ComponentMode::Vertex, false);
            chunked_sum(&points, |&p| {
                if !m.point_valid(p) {
                    return DVec3::ZERO;
                }
                // Each face once.
                let mut seen: Vec<Index> = Vec::with_capacity(8);
                let mut acc = DVec3::ZERO;
                m.for_each_point_corner(p, |c| {
                    let f = m.corner_face(c);
                    if !seen.contains(&f) {
                        seen.push(f);
                        acc += m.face_area_vector(f);
                    }
                });
                acc
            })
        };
        sum += to_world * local;
    }
    let len = sum.length();
    if len > 1e-12 {
        sum / len
    } else {
        DVec3::Y
    }
}

/// The world axis least aligned with the normal: the U fallback, shared by every point so a region's tangents agree.
fn tangent_reference(normal: DVec3) -> DVec3 {
    let mut axis = 0;
    for i in 1..3 {
        if normal[i].abs() < normal[axis].abs() - 1e-9 {
            axis = i;
        }
    }
    let mut r = DVec3::ZERO;
    r[axis] = 1.0;
    r
}

/// Direction of increasing u across face f (dP/du over its triangle fan), weighted by area; zero where its uvs are
/// degenerate.
fn face_u_tangent(m: &Mesh, f: Index, uv: &[[f32; 2]]) -> DVec3 {
    let mut sum = DVec3::ZERO;
    let c0 = m.face_corner(f);
    let p0 = m.position(m.corner_point(c0));
    let t0 = DVec2::new(uv[c0 as usize][0] as f64, uv[c0 as usize][1] as f64);
    let mut c1 = m.corner_next(c0);
    let mut c2 = m.corner_next(c1);
    while c2 != c0 {
        let e1 = m.position(m.corner_point(c1)) - p0;
        let e2 = m.position(m.corner_point(c2)) - p0;
        let d1 = DVec2::new(uv[c1 as usize][0] as f64, uv[c1 as usize][1] as f64) - t0;
        let d2 = DVec2::new(uv[c2 as usize][0] as f64, uv[c2 as usize][1] as f64) - t0;
        let det = d1.x * d2.y - d2.x * d1.y;
        if det.abs() >= 1e-14 {
            let t = (e1 * d2.y - e2 * d1.y) / det;
            let len = t.length();
            if len > 1e-12 {
                sum += t / len * e1.cross(e2).length();
            }
        }
        c1 = c2;
        c2 = m.corner_next(c2);
    }
    sum
}

struct FrameContext<'a> {
    m: &'a Mesh,
    uv: &'a [[f32; 2]],
    sel_face: Vec<bool>,
    sel_edge: Vec<bool>,
    reference: DVec3,
}

fn frame_context<'a>(m: &'a Mesh, kind: ComponentMode, ids: &[Index], reference: DVec3) -> FrameContext<'a> {
    let uv_name = names::uv(0);
    let mut uv: &[[f32; 2]] = if m.corner_attrs().has(&uv_name) { m.corner_attrs().get::<[f32; 2]>(&uv_name) } else { &[] };
    if uv.len() < m.corner_capacity() as usize {
        uv = &[];
    }
    let mut x = FrameContext { m, uv, sel_face: Vec::new(), sel_edge: Vec::new(), reference: DVec3::X };
    if matches!(kind, ComponentMode::Face | ComponentMode::Edge) {
        let cap = if kind == ComponentMode::Face { m.face_capacity() } else { m.edge_capacity() } as usize;
        let mut sel = vec![false; cap];
        for &i in ids {
            if (i as usize) < cap {
                sel[i as usize] = true;
            }
        }
        if kind == ComponentMode::Face {
            x.sel_face = sel;
        } else {
            x.sel_edge = sel;
        }
    }
    let len = reference.length();
    if len > 1e-12 {
        x.reference = reference / len;
    }
    x
}

fn point_frame(x: &FrameContext, p: Index) -> DMat3 {
    let m = x.m;
    let mut on_selected = false;
    if !x.sel_face.is_empty() {
        m.for_each_point_corner(p, |c| on_selected = on_selected || x.sel_face[m.corner_face(c) as usize]);
    }
    let (mut n, mut t) = (DVec3::ZERO, DVec3::ZERO);
    let mut t_weight = 0.0;
    m.for_each_point_corner(p, |c| {
        let f = m.corner_face(c);
        if on_selected && !x.sel_face[f as usize] {
            return;
        }
        n += m.face_area_vector(f);
        if !x.uv.is_empty() {
            let ft = face_u_tangent(m, f, x.uv);
            t += ft;
            t_weight += ft.length();
        }
    });
    let len = n.length();
    if len <= 1e-12 {
        return DMat3::ZERO;
    }
    n /= len;
    let in_plane = |v: DVec3| v - n * v.dot(n);
    t = in_plane(t);
    // Uv islands mirrored across the point cancel out, which counts as having no uvs.
    let mut u = if t_weight > 0.0 && t.length() > 0.25 * t_weight { t } else { in_plane(x.reference) };
    if u.length() < 1e-3 {
        u = in_plane(if n.y.abs() < 0.9 { DVec3::Y } else { DVec3::X });
    }
    if !x.sel_edge.is_empty() {
        let mut along = DVec3::ZERO;
        m.for_each_point_edge(p, |e| {
            if !x.sel_edge[e as usize] {
                return;
            }
            let d = in_plane(m.position(m.edge_other_point(e, p)) - m.position(p));
            along += if d.dot(u) < 0.0 { -d } else { d };
        });
        if along.length() > 1e-9 {
            u = along;
        }
    }
    let u = u.normalize();
    DMat3::from_cols(u, n.cross(u), n)
}

/// Per point, an object-space frame (columns U, V, N): N the vertex normal (a face selection's own faces), U along uv0's
/// u where there are uvs, along the selected edges in edge mode, else reference flattened onto the surface.
pub fn component_frames(m: &Mesh, kind: ComponentMode, ids: &[Index], reference: DVec3) -> Vec<DMat3> {
    let x = frame_context(m, kind, ids, reference);
    (0..m.point_capacity()).into_par_iter().map(|p| if m.point_valid(p) { point_frame(&x, p) } else { DMat3::ZERO }).collect()
}

/// world_frame(f, linear).x_axis, with linear's inverse transpose worked out once by the caller.
fn world_tangent(f: &DMat3, linear: &DMat3, normals: &DMat3) -> DVec3 {
    let n = *normals * f.z_axis;
    if n.length() < 1e-12 {
        return DVec3::ZERO;
    }
    let n = n.normalize();
    let mut u = *linear * f.x_axis;
    u -= n * u.dot(n);
    let len = u.length();
    if len < 1e-12 {
        DVec3::ZERO
    } else {
        u / len
    }
}

/// The selection's world frame: N its normal, U the average of its points' tangents.
pub fn selection_normal_frame(g: &mut Graph) -> DMat3 {
    let n = selection_normal(g);
    let reference = tangent_reference(n);
    let kind = components::mode(g);
    let mut sum = DVec3::ZERO;
    if kind != ComponentMode::Object {
        for (node, ids) in components::selected_all(g, kind).clone() {
            let Some(nd) = g.node(node) else { continue };
            if let Some(pp) = components::point_provider(nd) {
                for f in provided_world_frames(g, node, &pp, &ids) {
                    sum += f.x_axis;
                }
                continue;
            }
            if !is_mesh(g, node) {
                continue;
            }
            let Some(cage) = modifiers::cage(g, node) else { continue };
            let m = &*cage;
            let linear = DMat3::from_mat4(g.node(node).unwrap().matrix(Transform::WORLD_MATRIX));
            // Only the selection's own points, since this runs on every manipulator refresh.
            let side = driving_side(g, node, kind, &ids, Some(cage.clone()));
            let x = frame_context(m, kind, &side, linear.inverse() * reference);
            let points = components::convert(m, kind, &side, ComponentMode::Vertex, false);
            let normals = linear.inverse().transpose();
            // Deterministic, so the same selection always gets the same frame.
            sum += chunked_sum(&points, |&p| if m.point_valid(p) { world_tangent(&point_frame(&x, p), &linear, &normals) } else { DVec3::ZERO });
        }
    }
    let mut u = sum - n * sum.dot(n);
    if u.length() < 1e-6 {
        u = reference - n * reference.dot(n);
    }
    let u = u.normalize();
    DMat3::from_cols(u, n.cross(u), n)
}

/// Sets up a point provider node's targets by provider id, for edits that need more than a loose point mesh; None
/// skips the node.
type ProvidedSetup<'a> = dyn Fn(&mut Graph, NodeId, &PointProvider, &[u32]) -> Option<PointTarget> + 'a;

/// deform_selected for a point provider's node: its selected points moved to their targets, from provided when set,
/// else from setup given them as a loose point mesh.
fn deform_provided_points(
    g: &mut Graph,
    node: NodeId,
    pp: &PointProvider,
    ids: &[u32],
    setup: &PointTargetSetup,
    provided: Option<&ProvidedSetup>,
) -> bool {
    let (Some(points), Some(set_points)) = (&pp.points, &pp.set_points) else { return false };
    let all: BTreeMap<u32, DVec3> = points(g, node).into_iter().collect();
    let mut m = Mesh::default();
    let (mut local, mut source) = (Vec::new(), Vec::new());
    for &id in ids {
        if let Some(&p) = all.get(&id) {
            local.push(m.add_point(p));
            source.push(id);
        }
    }
    if source.is_empty() {
        return false;
    }
    let world = g.node(node).unwrap().matrix(Transform::WORLD_MATRIX);
    // Soft selection pulls unselected points part of the way. They join the point mesh before setup sees it, but not
    // the selection; provided edits key their targets by id and only know the selection, so they stay hard.
    struct SoftPoint {
        id: u32,
        local: Index,
        weight: f32,
    }
    let mut soft = Vec::new();
    if provided.is_none() && soft_enabled() {
        let chosen: BTreeSet<u32> = source.iter().copied().collect();
        let (mut order, mut world_pts, mut picked) = (Vec::new(), Vec::new(), Vec::new());
        for (&id, &p) in &all {
            if chosen.contains(&id) || pp.soft.as_ref().is_none_or(|s| s(id)) {
                order.push(id);
                world_pts.push(world.transform_point3(p));
                picked.push(chosen.contains(&id));
            }
        }
        let w = soft_weights_points(&world_pts, &picked);
        for k in 0..order.len() {
            if !picked[k] && w[k] > 0.0 {
                soft.push(SoftPoint { id: order[k], local: m.add_point(all[&order[k]]), weight: w[k] });
            }
        }
    }
    let target_of = match provided {
        Some(p) => p(g, node, pp, &source),
        None => setup(&m, &world, ComponentMode::Vertex, &local),
    };
    let Some(target_of) = target_of else { return false };
    let mut moves = Vec::new();
    for i in 0..source.len() {
        let key = if provided.is_some() { source[i] } else { local[i] };
        moves.push((source[i], target_of(key, m.position(local[i]))));
    }
    for sp in &soft {
        let at = m.position(sp.local);
        moves.push((sp.id, at.lerp(target_of(sp.local, at), sp.weight as f64)));
    }
    set_points(g, node, &moves);
    true
}

fn deform_selected(
    g: &mut Graph,
    label: &str,
    setup: &PointTargetSetup,
    provided: Option<&ProvidedSetup>,
    allow_whole: bool,
) -> CmdResult<i32> {
    let mut kind = components::mode(g);
    let whole = kind == ComponentMode::Object && allow_whole;
    let selection: BTreeMap<NodeId, Ids> = if whole {
        kind = ComponentMode::Vertex;
        g.selection().iter().filter_map(|&id| components::as_mesh(g, id)).map(|id| (id, Vec::new())).collect()
    } else if kind == ComponentMode::Object {
        fail!("{label}: no components selected");
    } else {
        components::selected_all(g, kind).clone()
    };
    let mut selection = selection;
    // Global: every other visible mesh within reach takes its share, with nothing of its own selected.
    if soft_enabled() && soft_select::falloff_name() == "global" && !whole && provided.is_none() {
        for id in g.nodes() {
            if selection.contains_key(&id) || !is_mesh(g, id) || g.node(id).is_none_or(|n| n.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT)) {
                continue;
            }
            let Some(m) = modifiers::cage(g, id) else { continue };
            let world = g.node(id).unwrap().matrix(Transform::WORLD_MATRIX);
            if soft_weights(g, &m, &world, &[]).iter().any(|&x| x > 0.0) {
                selection.insert(id, Vec::new());
            }
        }
    }
    g.open_chunk(label);
    let r = (|| -> CmdResult<i32> {
        let mut edited = 0;
        for (node, mut ids) in selection {
            let Some(n) = g.node(node) else { continue };
            if let Some(pp) = components::point_provider(n) {
                if kind == ComponentMode::Vertex {
                    edited += deform_provided_points(g, node, &pp, &ids, setup, provided) as i32;
                    continue;
                }
            }
            if !is_mesh(g, node) {
                continue;
            }
            // With history the moves go into a polyTweak after it; without, into the stored mesh.
            let tweak = crate::history::tweak_node(g, node)?;
            let base = if tweak.is_none() { Some(base_plug(g, node)?) } else { None };
            let mut m = match &base {
                Some(b) => base_mesh(g, b),
                None => modifiers::cage(g, node).map(|m| (*m).clone()).unwrap_or_default(),
            };
            if whole {
                ids.extend((0..m.point_capacity()).filter(|&p| m.point_valid(p)));
            }
            let mut pts = components::convert(&m, kind, &ids, ComponentMode::Vertex, false);
            let world = g.node(node).unwrap().matrix(Transform::WORLD_MATRIX);
            let Some(target_of) = setup(&m, &world, kind, &ids) else { continue };
            let sym = symmetry_for(g, node, &mut m);

            // With symmetry, the wedge (side) the selection is mostly in drives and the others follow as its exact
            // images.
            let mut drive = 0;
            if let Some(sym) = &sym {
                drive = drive_wedge(g, node, sym, &pts);
                pts = sym.expand_points(&pts);
            }
            let weight = soft_weights(g, &m, &world, &pts);
            let before = m.positions().to_vec();
            let mut moved = Vec::new();
            for p in 0..m.point_capacity() {
                if !m.point_valid(p) || weight[p as usize] <= 0.0 {
                    continue;
                }
                moved.push(p);
                if sym.as_ref().is_some_and(|s| !s.drives(p, drive)) {
                    continue;
                }
                let b = before[p as usize];
                m.set_position(p, b.lerp(target_of(p, b), weight[p as usize] as f64));
            }
            // The Constraint row: each point stays on what it had, sliding along edges or on faces.
            let constraint = g.move_opts.constraint.clone();
            if !constraint.is_empty() {
                let reference = m.clone();
                for &p in &moved {
                    let wanted = m.position(p);
                    let on = constrained(&reference, &before, p, wanted, constraint == "edge");
                    m.set_position(p, on);
                }
            }
            if let Some(sym) = &sym {
                sym.enforce_wedge(&mut m, &moved, drive);
            }
            let keep_uvs = if g.move_opts.preserve_uv { Some((before.clone(), m.positions().to_vec())) } else { None };
            match (base, tweak) {
                (Some(b), _) => {
                    g.set_value(b, PlugValue::Mesh(Some(Arc::new(m))))?;
                }
                (None, Some(t)) => crate::history::set_tweak(g, t, &m)?,
                (None, None) => {}
            }
            // Preserve UVs: the same move in the texture's terms, as a UV edit after it.
            if let Some((was, now)) = keep_uvs {
                g.evaluate();
                crate::uv_ops::edit_mesh(g, node, "polyTweakUV", |mesh| crate::uv_ops::follow_surface(mesh, &was, &now))?;
            }
            edited += 1;
        }
        Ok(edited)
    })();
    g.close_chunk();
    r
}

/// Where a point asked to go to `wanted` ends up on its own edges (the nearest point on any) or faces (the nearest on any
/// of their triangles), going by where everything was before.
fn constrained(m: &Mesh, before: &[DVec3], p: Index, wanted: DVec3, edges: bool) -> DVec3 {
    let at = before[p as usize];
    let mut best = at;
    let mut best_d = (at - wanted).length_squared();
    let mut try_point = |q: DVec3| {
        let d = (q - wanted).length_squared();
        if d < best_d {
            best_d = d;
            best = q;
        }
    };
    if edges {
        let mut ends = Vec::new();
        m.for_each_point_edge(p, |e| ends.push(m.edge_other_point(e, p)));
        for o in ends {
            let b = before[o as usize];
            let ab = b - at;
            let t = if ab.length_squared() < 1e-18 { 0.0 } else { ((wanted - at).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
            try_point(at + ab * t);
        }
    } else {
        for f in m.point_faces(p) {
            let pts = m.face_points(f);
            for k in 1..pts.len().saturating_sub(1) {
                let (a, b, c) = (before[pts[0] as usize], before[pts[k] as usize], before[pts[k + 1] as usize]);
                try_point(closest_on_triangle(wanted, a, b, c));
            }
        }
    }
    best
}

fn closest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let den = 1.0 / (va + vb + vc);
    a + ab * (vb * den) + ac * (vc * den)
}

/// Points of a mesh set to the positions a script worked out, through a polyTweak when the mesh has history.
pub fn set_points(g: &mut Graph, node: NodeId, moves: &[(Index, DVec3)]) -> CmdResult<()> {
    let tweak = crate::history::tweak_node(g, node)?;
    let base = if tweak.is_none() { Some(base_plug(g, node)?) } else { None };
    let mut m = match &base {
        Some(b) => base_mesh(g, b),
        None => modifiers::cage(g, node).map(|m| (*m).clone()).unwrap_or_default(),
    };
    for &(p, at) in moves {
        if m.point_valid(p) {
            m.set_position(p, at);
        }
    }
    match (base, tweak) {
        (Some(b), _) => {
            g.set_value(b, PlugValue::Mesh(Some(Arc::new(m))))?;
        }
        (None, Some(t)) => crate::history::set_tweak(g, t, &m)?,
        (None, None) => {}
    }
    Ok(())
}

/// Provided points moved by uvn along their own world frames; points without a frame stay put.
fn along_provided_frames(uvn: DVec3) -> impl Fn(&mut Graph, NodeId, &PointProvider, &[u32]) -> Option<PointTarget> {
    move |g, node, pp, ids| {
        let frames_fn = pp.frames.as_ref()?;
        let linear = DMat3::from_mat4(g.node(node).unwrap().matrix(Transform::WORLD_MATRIX));
        let to_local = linear.inverse();
        let frames = frames_fn(g, node, ids);
        let mut offsets = BTreeMap::new();
        for (i, f) in frames.iter().enumerate().take(ids.len()) {
            offsets.insert(ids[i], to_local * (world_frame(f, &linear) * uvn));
        }
        Some(Arc::new(move |id: u32, at: DVec3| at + offsets.get(&id).copied().unwrap_or(DVec3::ZERO)))
    }
}

/// Moves the selected components (every point of the selected meshes with allow_whole in object mode) to the targets
/// setup gives, with soft selection and symmetry. Returns how many nodes were edited.
pub fn deform_selected_components(g: &mut Graph, label: &str, setup: &PointTargetSetup, allow_whole: bool) -> CmdResult<i32> {
    deform_selected(g, label, setup, None, allow_whole)
}

pub fn transform_selected_components(g: &mut Graph, label: &str, world_xform: DMat4) -> CmdResult<()> {
    deform_selected_components(
        g,
        label,
        &|_m, world, _k, _ids| {
            let local = world.inverse() * world_xform * *world;
            Some(Arc::new(move |_p, at| local.transform_point3(at)))
        },
        false,
    )?;
    Ok(())
}

pub fn transform_selected_components_onto(g: &mut Graph, label: &str, world_xform: DMat4, onto: SurfaceProjector) -> CmdResult<()> {
    deform_selected_components(
        g,
        label,
        &|_m, world, _k, _ids| {
            let local = world.inverse() * world_xform * *world;
            let (to_local, world) = (world.inverse(), *world);
            let onto = onto.clone();
            Some(Arc::new(move |_p, at| {
                let moved = (world * local).transform_point3(at);
                to_local.transform_point3(onto(moved))
            }))
        },
        false,
    )?;
    Ok(())
}

/// Maps a world point to the closest point on any of the meshes' surfaces.
pub fn closest_on_meshes(g: &mut Graph, meshes: &[NodeId]) -> SurfaceProjector {
    struct Surface {
        bvh: MeshBvh,
        world: DMat4,
        to_local: DMat4,
    }
    g.evaluate();
    let mut surfaces = Vec::new();
    for &id in meshes {
        if !is_mesh(g, id) {
            continue;
        }
        let n = g.node(id).unwrap();
        let Some(mesh) = n.data().mesh(MeshNode::OUT_MESH) else { continue };
        if mesh.num_faces() == 0 {
            continue;
        }
        let world = n.matrix(Transform::WORLD_MATRIX);
        surfaces.push(Surface { bvh: MeshBvh::new(mesh), world, to_local: world.inverse() });
    }
    let surfaces = Arc::new(surfaces);
    Arc::new(move |p: DVec3| {
        let mut best = p;
        let mut best_dist = f64::MAX;
        for s in surfaces.iter() {
            let Some(c) = s.bvh.closest_point(s.to_local.transform_point3(p)) else { continue };
            let w = s.world.transform_point3(c.point);
            let d = (w - p).length();
            if d < best_dist {
                best_dist = d;
                best = w;
            }
        }
        best
    })
}

/// Pushes the selected components along their normals by a world distance.
pub fn offset_selected_components(g: &mut Graph, label: &str, distance: f64) -> CmdResult<()> {
    let provided = along_provided_frames(DVec3::new(0.0, 0.0, distance));
    deform_selected(
        g,
        label,
        &|m, world, kind, ids| {
            let normals = Arc::new(component_normals(m, kind, ids));
            let to_world = DMat3::from_mat4(*world).inverse().transpose();
            let to_local = DMat3::from_mat4(*world).inverse();
            // The distance is world space, so a scaled object still moves by it.
            Some(Arc::new(move |p, at| {
                let nw = to_world * normals[p as usize];
                let len = nw.length();
                if len > 1e-12 {
                    at + to_local * (nw / len * distance)
                } else {
                    at
                }
            }))
        },
        Some(&provided),
        false,
    )?;
    Ok(())
}

/// Moves the selected components by uvn along each point's own tangent frame (see component_frames).
pub fn offset_selected_components_uvn(g: &mut Graph, label: &str, uvn: DVec3) -> CmdResult<()> {
    if uvn.x == 0.0 && uvn.y == 0.0 {
        return offset_selected_components(g, label, uvn.z);
    }
    let reference = tangent_reference(selection_normal(g));
    let provided = along_provided_frames(uvn);
    deform_selected(
        g,
        label,
        &|m, world, kind, ids| {
            let linear = DMat3::from_mat4(*world);
            let to_local = linear.inverse();
            let frames = Arc::new(component_frames(m, kind, ids, to_local * reference));
            Some(Arc::new(move |p, at| at + to_local * (world_frame(&frames[p as usize], &linear) * uvn)))
        },
        Some(&provided),
        false,
    )?;
    Ok(())
}

/// The components each moved by `by` and rounded to whole steps of the world.
fn snap_each(g: &mut Graph, by: DMat4, step: f64) -> CmdResult<i32> {
    deform_selected(
        g,
        "Move Components",
        &move |_m: &Mesh, world: &DMat4, _k: ComponentMode, _ids: &Ids| {
            let (w, inv) = (*world, world.inverse());
            let t: PointTarget = Arc::new(move |_p: u32, at: DVec3| inv.transform_point3((by.transform_point3(w.transform_point3(at)) / step).round() * step));
            Some(t)
        },
        None,
        false,
    )
}

fn translate(v: DVec3) -> DMat4 {
    DMat4::from_translation(v)
}

pub fn register(c: &mut Commands<Session>) {
    type T = ArgType;
    let flag = |name: &str, ty, doc: &str| FlagDef::new(name, "", ty, Value::Null, doc);
    c.register(
        CommandDef::<Session>::new(
            "moveComponents",
            "Move the selected components by an offset in world space, or with normal= push each point along its own \
             vertex normal (a face selection uses only the selected faces' normals, so the region inflates as a whole). \
             uvn= also slides each point along its own surface tangents: U follows the uv0 u direction where the mesh has \
             uvs (in edge mode, the selected edges), else the world axis least aligned with the selection's normal, and V \
             completes the frame. Curve points use the curve's frame: U its tangent (a handle's own direction), N across it \
             within its plane, V out of the plane.",
            |s, a| {
                let given = a.has("by") as i32 + a.has("normal") as i32 + a.has("uvn") as i32;
                if given != 1 {
                    fail!("moveComponents: give one of by=, normal= or uvn=");
                }
                let g = &mut s.graph;
                if a.has("normal") {
                    offset_selected_components(g, "Move Components", a.float("normal", 0.0))?;
                    return Ok(Value::Null);
                }
                if a.has("uvn") {
                    offset_selected_components_uvn(g, "Move Components", DVec3::from_array(a.vec3("uvn").unwrap()))?;
                    return Ok(Value::Null);
                }
                let by = translate(DVec3::from_array(a.vec3("by").unwrap()));
                // Keep Spacing off, with a grid snap: each point lands on the grid by itself.
                if let Some(step) = a.get("snapEach").and_then(Value::as_f64).filter(|s| *s > 0.0) {
                    return snap_each(g, by, step).map(|_| Value::Null);
                }
                if let Some(list) = a.get("onto").and_then(Value::as_array) {
                    let mut onto = Vec::new();
                    for name in list {
                        onto.push(mesh_arg(g, name.as_str().unwrap_or(""), "moveComponents")?);
                    }
                    let proj = closest_on_meshes(g, &onto);
                    transform_selected_components_onto(g, "Move Components", by, proj)?;
                    return Ok(Value::Null);
                }
                transform_selected_components(g, "Move Components", by)?;
                Ok(Value::Null)
            },
        )
        .flag(flag("snapEach", T::Float, "Each point lands on whole steps of this, by itself."))
        .flag(flag("by", T::Vec3, "Offset, cm."))
        .flag(flag("normal", T::Float, "Distance along the vertex normals, cm."))
        .flag(flag("uvn", T::Vec3, "Distances along each point's own U tangent, V tangent and normal, cm."))
        .flag(flag("onto", T::List, "Meshes whose surface each moved point then lands on, at its closest point.")),
    );

    c.register(
        CommandDef::<Session>::new(
            "rotateComponents",
            "Rotate the selected components about a world pivot (default: their centre; with symmetry, a pair clear of the \
             mirror plane turns about each side's own centre). Degrees, XYZ order.",
            |s, a| {
                let Some(by) = a.vec3("by") else { fail!("rotateComponents: give by=") };
                let g = &mut s.graph;
                let pivot = match a.vec3("pivot") {
                    Some(p) => DVec3::from_array(p),
                    None => symmetric_selection_center(g),
                };
                let r = euler_to_matrix(DVec3::from_array(by), RotateOrder::Xyz);
                transform_selected_components(g, "Rotate Components", translate(pivot) * r * translate(-pivot))?;
                Ok(Value::Null)
            },
        )
        .flag(flag("by", T::Vec3, "Rotation, degrees."))
        .flag(flag("pivot", T::Vec3, "World pivot.")),
    );

    c.register(
        CommandDef::<Session>::new(
            "scaleComponents",
            "Scale the selected components about a world pivot (default: their centre; with symmetry, a pair clear of the \
             mirror plane scales about each side's own centre).",
            |s, a| {
                let Some(by) = a.vec3("by") else { fail!("scaleComponents: give by=") };
                let g = &mut s.graph;
                let pivot = match a.vec3("pivot") {
                    Some(p) => DVec3::from_array(p),
                    None => symmetric_selection_center(g),
                };
                let r = match a.vec3("orient") {
                    Some(o) => euler_to_matrix(DVec3::from_array(o), RotateOrder::Xyz),
                    None => DMat4::IDENTITY,
                };
                let m = translate(pivot) * r * DMat4::from_scale(DVec3::from_array(by)) * r.transpose() * translate(-pivot);
                transform_selected_components(g, "Scale Components", m)?;
                Ok(Value::Null)
            },
        )
        .flag(flag("by", T::Vec3, "Factor per axis: the world axes, or the axes orient= turns them to."))
        .flag(flag("pivot", T::Vec3, "World pivot."))
        .flag(flag("orient", T::Vec3, "Rotation of the scale axes from the world axes, degrees, XYZ order.")),
    );
}
