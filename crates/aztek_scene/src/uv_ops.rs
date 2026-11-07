//! UV editing as the reference does it: projections, unfold, layout, cut and sew, moving UVs, normalize and flip. Each edit is a
//! history node (polyPlanarProj1, polyMapCut1, polyTweakUV1...) holding the UVs and seams it changed, or goes straight
//! into the mesh when history is off.
//!
//! UVs live per corner in "uv0". A map (the reference's UV, `pCube1.map[3]`) is the corners around one point that share a UV:
//! joined across edges that are not seams and whose UVs agree. A shell is the faces joined the same way.

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

use aztek_geo::{Index, Mesh, INVALID};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_uv::chart::{build_charts, write_charts, Chart, ChartOptions, IslandSource};
use aztek_uv::layout::{fit_chart, orient_chart, OrientMode};
use aztek_uv::project::{project, ProjectAxis, ProjectOptions, Projection};
use aztek_uv::unwrap::{unwrap_charts, UnwrapOptions};
use glam::{DVec2, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::mesh_attr;
use crate::components::{self, ComponentMode};
use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

pub const UV: &str = "uv0";
const SEAM: &str = "seam";

// ---------------------------------------------------------------------------------------------------------------------
// Maps and shells

struct Uf(Vec<u32>);

impl Uf {
    fn new(n: usize) -> Uf {
        Uf((0..n as u32).collect())
    }
    fn find(&mut self, mut a: u32) -> u32 {
        while self.0[a as usize] != a {
            self.0[a as usize] = self.0[self.0[a as usize] as usize];
            a = self.0[a as usize];
        }
        a
    }
    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.find(a), self.find(b));
        if a != b {
            self.0[a.max(b) as usize] = a.min(b);
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UvTopo {
    /// Map id per corner (INVALID for dead corners).
    pub corner_map: Vec<Index>,
    /// Corners per map, maps ordered by their first corner.
    pub maps: Vec<Vec<Index>>,
    /// Shell id per face (INVALID for dead faces).
    pub face_shell: Vec<Index>,
    pub shells: Vec<Vec<Index>>,
    /// Per edge: the UVs split here (a seam, a border, or the two sides disagree).
    pub split: Vec<bool>,
}

fn uv_of(uv: &[[f32; 2]], c: Index) -> DVec2 {
    uv.get(c as usize).map_or(DVec2::ZERO, |u| DVec2::new(u[0] as f64, u[1] as f64))
}

fn same(a: DVec2, b: DVec2) -> bool {
    (a - b).length_squared() < 1e-12
}

/// The corner of face(r) at point p, where r lies on an edge through p.
fn corner_at(m: &Mesh, r: Index, p: Index) -> Index {
    if m.corner_point(r) == p {
        r
    } else {
        m.corner_next(r)
    }
}

pub fn topo(m: &Mesh) -> UvTopo {
    let nc = m.corner_capacity() as usize;
    let uv = m.corner_attrs().get::<[f32; 2]>(UV);
    let seam = m.edge_attrs().get::<bool>(SEAM);
    let mut split = vec![true; m.edge_capacity() as usize];
    let mut corners = Uf::new(nc);
    let mut faces = Uf::new(m.face_capacity() as usize);
    for c in 0..nc as Index {
        if !m.corner_valid(c) || !m.face_valid(m.corner_face(c)) {
            continue;
        }
        let r = m.corner_radial(c);
        if r == c || r == INVALID {
            continue;
        }
        // Only edges with exactly two faces join.
        if m.corner_radial(r) != c {
            continue;
        }
        let e = m.corner_edge(c);
        let (p, q) = (m.corner_point(c), m.corner_next_point(c));
        let (cp, cq) = (c, m.corner_next(c));
        let (rp, rq) = (corner_at(m, r, p), corner_at(m, r, q));
        let joined = !seam.get(e as usize).copied().unwrap_or(false)
            && same(uv_of(uv, cp), uv_of(uv, rp))
            && same(uv_of(uv, cq), uv_of(uv, rq));
        split[e as usize] = !joined;
        if joined {
            corners.join(cp, rp);
            corners.join(cq, rq);
            faces.join(m.corner_face(c), m.corner_face(r));
        }
    }
    let mut corner_map = vec![INVALID; nc];
    let mut maps: Vec<Vec<Index>> = Vec::new();
    let mut root_map: BTreeMap<u32, Index> = BTreeMap::new();
    for c in 0..nc as Index {
        if !m.corner_valid(c) || !m.face_valid(m.corner_face(c)) {
            continue;
        }
        let root = corners.find(c);
        let id = *root_map.entry(root).or_insert_with(|| {
            maps.push(Vec::new());
            (maps.len() - 1) as Index
        });
        maps[id as usize].push(c);
        corner_map[c as usize] = id;
    }
    let mut face_shell = vec![INVALID; m.face_capacity() as usize];
    let mut shells: Vec<Vec<Index>> = Vec::new();
    let mut root_shell: BTreeMap<u32, Index> = BTreeMap::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let root = faces.find(f);
        let id = *root_shell.entry(root).or_insert_with(|| {
            shells.push(Vec::new());
            (shells.len() - 1) as Index
        });
        shells[id as usize].push(f);
        face_shell[f as usize] = id;
    }
    UvTopo { corner_map, maps, face_shell, shells, split }
}

/// The shells (as face lists) touching any of the faces.
pub(crate) fn shells_of(t: &UvTopo, faces: &[Index]) -> Vec<Vec<Index>> {
    let mut ids: Vec<Index> = faces.iter().filter_map(|&f| t.face_shell.get(f as usize).copied()).filter(|&s| s != INVALID).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.into_iter().map(|s| t.shells[s as usize].clone()).collect()
}

fn all_faces(m: &Mesh) -> Vec<Index> {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()
}

fn charts_for(m: &Mesh, faces: &[Index]) -> Vec<Chart> {
    let t = topo(m);
    let opts = ChartOptions { source: IslandSource::Seams, uv_set: UV, extra_seams: &t.split, use_seam_attr: true, pin_attr: "uvPin" };
    build_charts(m, faces, &opts)
}

// ---------------------------------------------------------------------------------------------------------------------
// The edits

/// Marks the edges as UV seams: their two sides become separate UVs.
pub fn cut(m: &mut Mesh, edges: &[Index]) {
    let s = m.edge_attrs_mut().get_or_add::<bool>(SEAM);
    for &e in edges {
        if let Some(x) = s.get_mut(e as usize) {
            *x = true;
        }
    }
}

/// Joins the edges' two sides: each end goes to the average of the two UVs there, and the seam goes.
pub fn sew(m: &mut Mesh, edges: &[Index]) {
    let t = topo(m);
    for &e in edges {
        if !m.edge_valid(e) {
            continue;
        }
        let c = m.edge_corner(e);
        if c == INVALID {
            continue;
        }
        let r = m.corner_radial(c);
        if r == c || m.corner_radial(r) != c {
            continue;
        }
        let uv = m.corner_attrs().get::<[f32; 2]>(UV).to_vec();
        let mut writes: Vec<(Index, DVec2)> = Vec::new();
        for p in [m.corner_point(c), m.corner_next_point(c)] {
            let (a, b) = (corner_at(m, c, p), corner_at(m, r, p));
            let avg = (uv_of(&uv, a) + uv_of(&uv, b)) * 0.5;
            for x in [a, b] {
                let map = t.corner_map[x as usize];
                for &k in &t.maps[map as usize] {
                    writes.push((k, avg));
                }
            }
        }
        let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV);
        for (k, v) in writes {
            layer[k as usize] = [v.x as f32, v.y as f32];
        }
        if let Some(s) = m.edge_attrs_mut().get_or_add::<bool>(SEAM).get_mut(e as usize) {
            *s = false;
        }
    }
}

/// Moves the shell on one side of the first edge onto the other side (turned and scaled to match), then sews.
pub fn move_and_sew(m: &mut Mesh, edges: &[Index]) {
    let t = topo(m);
    let Some(&e) = edges.iter().find(|&&e| m.edge_valid(e) && t.split[e as usize]) else { return };
    let c = m.edge_corner(e);
    let r = m.corner_radial(c);
    if r == c || r == INVALID {
        return;
    }
    let (sa, sb) = (t.face_shell[m.corner_face(c) as usize], t.face_shell[m.corner_face(r) as usize]);
    let uv = m.corner_attrs().get::<[f32; 2]>(UV).to_vec();
    if sa != sb {
        // The smaller shell moves.
        let (_fixed, moving, fc, mc) =
            if t.shells[sa as usize].len() >= t.shells[sb as usize].len() { (sa, sb, c, r) } else { (sb, sa, r, c) };
        let (p, q) = (m.corner_point(fc), m.corner_next_point(fc));
        let (a0, a1) = (uv_of(&uv, corner_at(m, fc, p)), uv_of(&uv, corner_at(m, fc, q)));
        let (b0, b1) = (uv_of(&uv, corner_at(m, mc, p)), uv_of(&uv, corner_at(m, mc, q)));
        let (da, db) = (a1 - a0, b1 - b0);
        if db.length_squared() > 1e-18 {
            let s = da.length() / db.length();
            let ang = da.y.atan2(da.x) - db.y.atan2(db.x);
            let (cs, sn) = (ang.cos(), ang.sin());
            let corners: Vec<Index> = t.shells[moving as usize].iter().flat_map(|&f| m.face_corners(f)).collect();
            let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV);
            for k in corners {
                let v = uv_of(layer, k) - b0;
                let v = DVec2::new(v.x * cs - v.y * sn, v.x * sn + v.y * cs) * s + a0;
                layer[k as usize] = [v.x as f32, v.y as f32];
            }
        }
    }
    sew(m, edges);
}

/// Projects the faces' UVs through a plane, cylinder or sphere fitted to them (or along an axis), cutting seams.
pub fn project_faces(m: &mut Mesh, faces: &[Index], ty: Projection, axis: ProjectAxis) {
    let opts = ProjectOptions { ty, axis, normalize: true, mark_seams: true, keep_seams: true, ..Default::default() };
    project(m, faces, UV, &opts);
}

/// The UV change that keeps a texture on the surface as points went from `was` to `now`: at each corner of a moved point, the
/// move in the terms of the corner's two edges, then the same in UV.
pub fn follow_surface(m: &mut Mesh, was: &[glam::DVec3], now: &[glam::DVec3]) {
    let uv: Vec<[f32; 2]> = m.corner_attrs().get::<[f32; 2]>(UV).to_vec();
    if uv.is_empty() {
        return;
    }
    let mut edit = UvEdit::default();
    let n = was.len().min(now.len());
    for c in 0..m.corner_capacity() {
        if !m.corner_valid(c) {
            continue;
        }
        let p = m.corner_point(c) as usize;
        if p >= n || (now[p] - was[p]).length_squared() < 1e-18 {
            continue;
        }
        let (next, prev) = (m.corner_next(c), m.corner_prev(c));
        let (Some(pn), Some(pp)) = (was.get(m.corner_point(next) as usize), was.get(m.corner_point(prev) as usize)) else { continue };
        let (e1, e2) = (*pn - was[p], *pp - was[p]);
        let d = now[p] - was[p];
        let (a11, a12, a22) = (e1.dot(e1), e1.dot(e2), e2.dot(e2));
        let det = a11 * a22 - a12 * a12;
        if det.abs() < 1e-18 {
            continue;
        }
        let (b1, b2) = (d.dot(e1), d.dot(e2));
        let (s, t) = ((a22 * b1 - a12 * b2) / det, (a11 * b2 - a12 * b1) / det);
        let u = uv[c as usize];
        let (un, up) = (uv[next as usize], uv[prev as usize]);
        edit.uvs.insert(c, [u[0] + (s as f32) * (un[0] - u[0]) + (t as f32) * (up[0] - u[0]), u[1] + (s as f32) * (un[1] - u[1]) + (t as f32) * (up[1] - u[1])]);
    }
    edit.apply(m);
}

/// Flattens the shells of the faces with least distortion, keeping each where it was and its size.
pub fn unfold(m: &mut Mesh, faces: &[Index]) {
    let mut charts = charts_for(m, faces);
    let before: Vec<(DVec2, f64)> = charts.iter().map(|c| (c.uv_centroid(), c.area_uv().abs())).collect();
    unwrap_charts(&mut charts, &UnwrapOptions::default());
    for (ch, (c, a)) in charts.iter_mut().zip(before) {
        let area = if a > 1e-12 { a } else { ch.area_3d() };
        fit_chart(ch, c, area);
    }
    write_charts(m, &charts, UV);
}

/// Packs the shells of the faces into 0-1: scaled to their 3D size, turned to their smallest rectangle, in rows.
pub fn layout(m: &mut Mesh, faces: &[Index], spacing: f64, rotate: bool) {
    let mut charts = charts_for(m, faces);
    if charts.is_empty() {
        return;
    }
    for ch in charts.iter_mut() {
        let c = ch.uv_centroid();
        let a3 = ch.area_3d();
        fit_chart(ch, c, a3);
        if rotate {
            orient_chart(ch, OrientMode::BoundingBox);
        }
    }
    let boxes: Vec<(DVec2, DVec2)> = charts.iter().map(|c| (c.uv_min(), c.uv_max())).collect();
    let total: f64 = boxes.iter().map(|(a, b)| (b.x - a.x) * (b.y - a.y)).sum();
    let mut order: Vec<usize> = (0..charts.len()).collect();
    order.sort_by(|&a, &b| (boxes[b].1.y - boxes[b].0.y).total_cmp(&(boxes[a].1.y - boxes[a].0.y)));
    // Rows of a given width; the square that holds them at the best width wins.
    let pack = |width: f64, pad: f64| -> (Vec<DVec2>, f64, f64) {
        let mut at = vec![DVec2::ZERO; charts.len()];
        let (mut x, mut y, mut row_h, mut used_w) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
        for &i in &order {
            let size = boxes[i].1 - boxes[i].0;
            if x > 0.0 && x + size.x > width {
                y += row_h + pad;
                x = 0.0;
                row_h = 0.0;
            }
            at[i] = DVec2::new(x, y);
            x += size.x + pad;
            used_w = used_w.max(x - pad);
            row_h = row_h.max(size.y);
        }
        (at, used_w, y + row_h)
    };
    let pad = spacing * total.sqrt();
    let widest = boxes.iter().map(|(a, b)| b.x - a.x).fold(0.0, f64::max);
    let mut best: Option<(f64, Vec<DVec2>)> = None;
    for k in 0..40 {
        let w = widest.max(total.sqrt() * (0.6 + k as f64 * 0.05));
        let (at, uw, h) = pack(w, pad);
        let side = uw.max(h);
        if best.as_ref().is_none_or(|b| side < b.0) {
            best = Some((side, at));
        }
    }
    let (side, at) = best.unwrap();
    let margin = spacing;
    let s = (1.0 - 2.0 * margin) / side.max(1e-12);
    for (i, ch) in charts.iter_mut().enumerate() {
        let off = at[i] - boxes[i].0;
        for p in ch.uv.iter_mut() {
            *p = (*p + off) * s + DVec2::splat(margin);
        }
    }
    write_charts(m, &charts, UV);
}

/// The corners of the faces, or of the maps.
fn corners_of(m: &Mesh, t: &UvTopo, faces: &[Index], maps: &[Index]) -> Vec<Index> {
    let mut out: Vec<Index> = faces.iter().filter(|&&f| m.face_valid(f)).flat_map(|&f| m.face_corners(f)).collect();
    for &mp in maps {
        if let Some(cs) = t.maps.get(mp as usize) {
            out.extend(cs.iter().copied());
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn bounds(uv: &[[f32; 2]], corners: &[Index]) -> (DVec2, DVec2) {
    let mut lo = DVec2::splat(f64::MAX);
    let mut hi = DVec2::splat(f64::MIN);
    for &c in corners {
        let v = uv_of(uv, c);
        lo = lo.min(v);
        hi = hi.max(v);
    }
    (lo, hi)
}

/// Moves, scales and turns UVs about a pivot: v' = pivot + rotate((v - pivot) * scale) + offset.
pub fn transform(m: &mut Mesh, corners: &[Index], offset: DVec2, scale: DVec2, degrees: f64, pivot: DVec2) {
    let (c, s) = (degrees.to_radians().cos(), degrees.to_radians().sin());
    let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV);
    for &k in corners {
        let v = (uv_of(layer, k) - pivot) * scale;
        let v = pivot + DVec2::new(v.x * c - v.y * s, v.x * s + v.y * c) + offset;
        layer[k as usize] = [v.x as f32, v.y as f32];
    }
}

pub fn set_uv(m: &mut Mesh, corners: &[Index], u: Option<f64>, v: Option<f64>) {
    let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV);
    for &k in corners {
        if let Some(u) = u {
            layer[k as usize][0] = u as f32;
        }
        if let Some(v) = v {
            layer[k as usize][1] = v as f32;
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// History nodes

/// UVs and seams a UV edit set, by corner and edge id.
#[derive(Debug, Clone, Default)]
pub struct UvEdit {
    pub uvs: BTreeMap<Index, [f32; 2]>,
    pub seams: BTreeMap<Index, bool>,
}

impl crate::plug::Data for UvEdit {
    fn kind(&self) -> &str {
        "polyUvEdit"
    }
    fn to_json(&self) -> Value {
        let uvs: Vec<Value> = self.uvs.iter().map(|(&c, v)| json!([c, v[0], v[1]])).collect();
        let seams: Vec<Value> = self.seams.iter().map(|(&e, &s)| json!([e, s])).collect();
        json!({"kind": "polyUvEdit", "uvs": uvs, "seams": seams})
    }
    fn bytes(&self) -> usize {
        self.uvs.len() * 16 + self.seams.len() * 8
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl UvEdit {
    pub fn from_json(j: &Value) -> UvEdit {
        let mut e = UvEdit::default();
        for x in j.get("uvs").and_then(Value::as_array).into_iter().flatten() {
            let f = |i: usize| x.get(i).and_then(Value::as_f64).unwrap_or(0.0);
            e.uvs.insert(f(0) as Index, [f(1) as f32, f(2) as f32]);
        }
        for x in j.get("seams").and_then(Value::as_array).into_iter().flatten() {
            if let (Some(i), Some(s)) = (x.get(0).and_then(Value::as_u64), x.get(1).and_then(Value::as_bool)) {
                e.seams.insert(i as Index, s);
            }
        }
        e
    }

    /// What turned before into after.
    pub fn diff(before: &Mesh, after: &Mesh) -> UvEdit {
        let mut e = UvEdit::default();
        let (a, b) = (before.corner_attrs().get::<[f32; 2]>(UV), after.corner_attrs().get::<[f32; 2]>(UV));
        for c in 0..after.corner_capacity() {
            if !after.corner_valid(c) {
                continue;
            }
            let n = b.get(c as usize).copied().unwrap_or([0.0; 2]);
            if a.get(c as usize).copied() != Some(n) {
                e.uvs.insert(c, n);
            }
        }
        let (sa, sb) = (before.edge_attrs().get::<bool>(SEAM), after.edge_attrs().get::<bool>(SEAM));
        for i in 0..after.edge_capacity() {
            let n = sb.get(i as usize).copied().unwrap_or(false);
            if sa.get(i as usize).copied().unwrap_or(false) != n {
                e.seams.insert(i, n);
            }
        }
        e
    }

    pub fn apply(&self, m: &mut Mesh) {
        if !self.uvs.is_empty() {
            let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV);
            for (&c, v) in &self.uvs {
                if let Some(x) = layer.get_mut(c as usize) {
                    *x = *v;
                }
            }
        }
        if !self.seams.is_empty() {
            let s = m.edge_attrs_mut().get_or_add::<bool>(SEAM);
            for (&e, &v) in &self.seams {
                if let Some(x) = s.get_mut(e as usize) {
                    *x = v;
                }
            }
        }
    }
}

const IN: usize = 0;
const OUT: usize = 1;
const EDIT: usize = 2;

pub const NODE_TYPES: &[(&str, &str)] = &[
    ("polyTweakUV", "UVs moved, scaled or turned."),
    ("polyMapCut", "UV seams cut along edges."),
    ("polyMapSew", "UV seams sewn together."),
    ("polyMapSewMove", "A UV shell moved onto its neighbour and sewn."),
    ("polyPlanarProj", "UVs projected through a plane."),
    ("polyCylProj", "UVs projected from a cylinder."),
    ("polySphProj", "UVs projected from a sphere."),
    ("polyAutoProj", "UVs projected from the six box planes and laid out."),
    ("Unfold3DUnfold", "UV shells flattened with least distortion."),
    ("polyLayoutUV", "UV shells packed into 0 to 1."),
    ("polyFlipUV", "UVs mirrored."),
    ("polyNormalizeUV", "UVs fitted into 0 to 1."),
    ("polyMoveUV", "UVs moved, scaled or turned."),
    ("polyMergeUV", "UVs merged where they are close."),
    ("polyCopyUV", "UVs copied from another set."),
    ("polyForceUV", "UVs flipped, fitted or made one per face."),
    ("polyContourProj", "UVs stretched along a run of quads."),
    ("polyTransfer", "UVs copied from a mesh of the same topology."),
    ("polyMapDel", "UVs of faces cleared."),
    ("polyOptUvs", "UVs relaxed."),
    ("polyUVRectangle", "A shell's UVs squared onto its bounding box."),
];

pub struct UvEditNode {
    /// Which projection the node reprojects with once its attributes change.
    pub projection: Option<Projection>,
}

// The projection nodes' attributes, after the shared three.
const PJ_CENTER: usize = 3;
const PJ_ROTATE: usize = 4;
const PJ_WIDTH: usize = 5;
const PJ_HEIGHT: usize = 6;
const PJ_IMAGE_CENTER: usize = 7;
const PJ_IMAGE_SCALE: usize = 8;
const PJ_SWEEP: usize = 9;
/// The attributes as the projection left them: while they match, the stored UVs stand.
const PJ_FITTED: usize = 10;
const PJ_FACES: usize = 11;

fn projection_state(d: &NodeData) -> String {
    let v = |a: usize| d.vec3(a);
    format!("{:.6?} {:.6?} {:.6} {:.6} {:.6?} {:.6?} {:.6}", v(PJ_CENTER), v(PJ_ROTATE), d.float(PJ_WIDTH), d.float(PJ_HEIGHT), v(PJ_IMAGE_CENTER), v(PJ_IMAGE_SCALE), d.float(PJ_SWEEP))
}

fn projection_axes(rotate: glam::DVec3) -> glam::DMat3 {
    let r = rotate * (std::f64::consts::PI / 180.0);
    glam::DMat3::from_quat(glam::DQuat::from_euler(glam::EulerRot::ZYX, r.z, r.y, r.x))
}

/// The UVs the attributes give, for the faces the node holds.
fn reproject(d: &NodeData, ty: Projection, m: &mut Mesh) {
    let axes = projection_axes(d.vec3(PJ_ROTATE));
    let inv = axes.transpose();
    let c = d.vec3(PJ_CENTER);
    let (w, h) = (d.float(PJ_WIDTH).abs().max(1e-9), d.float(PJ_HEIGHT).abs().max(1e-9));
    let (ic, is) = (d.vec3(PJ_IMAGE_CENTER), d.vec3(PJ_IMAGE_SCALE));
    let sweep = d.float(PJ_SWEEP).abs().max(1e-3).to_radians();
    let faces = crate::history::parse_components(d.string(PJ_FACES));
    let mut edit = UvEdit::default();
    for f in faces {
        if !m.face_valid(f) {
            continue;
        }
        for corner in m.face_corners(f) {
            let p = inv * (m.position(m.corner_point(corner)) - c);
            let (u, v) = match ty {
                Projection::Cylindrical => (p.x.atan2(p.z) / sweep + 0.5, p.y / h + 0.5),
                Projection::Spherical => (p.x.atan2(p.z) / sweep + 0.5, (p.y / p.length().max(1e-9)).asin() / (sweep * 0.5) + 0.5),
                _ => (p.x / w + 0.5, p.y / h + 0.5),
            };
            edit.uvs.insert(corner, [(u * is.x + ic.x) as f32, (v * is.y + ic.y) as f32]);
        }
    }
    edit.apply(m);
}

/// After a projection: the node's attributes hold the box it fitted, so handles have something to move.
pub fn fit_projection(g: &mut Graph, node: NodeId, m: &Mesh, faces: &[Index], ty: Projection, axis: ProjectAxis) -> CmdResult<()> {
    let mut lo = DVec3::splat(f64::MAX);
    let mut hi = DVec3::splat(f64::MIN);
    let mut area = DVec3::ZERO;
    for &f in faces {
        if !m.face_valid(f) {
            continue;
        }
        area += m.face_area_vector(f);
        for p in m.face_points(f) {
            lo = lo.min(m.position(p));
            hi = hi.max(m.position(p));
        }
    }
    if lo.x > hi.x {
        return Ok(());
    }
    let centre = (lo + hi) * 0.5;
    let ext = hi - lo;
    let a = area.abs();
    let normal = match axis {
        ProjectAxis::X => 0,
        ProjectAxis::Y => 1,
        ProjectAxis::Z => 2,
        _ if a.x >= a.y && a.x >= a.z => 0,
        _ if a.y >= a.z => 1,
        _ => 2,
    };
    let (rotate, w, h) = match (ty, normal) {
        (Projection::Planar, 0) => (DVec3::new(0.0, 90.0, 0.0), ext.z, ext.y),
        (Projection::Planar, 1) => (DVec3::new(-90.0, 0.0, 0.0), ext.x, ext.z),
        (Projection::Planar, _) => (DVec3::ZERO, ext.x, ext.y),
        _ => (DVec3::ZERO, ext.x.max(ext.z), ext.y),
    };
    let sweep = if ty == Projection::Spherical || ty == Projection::Cylindrical { 360.0 } else { 0.0 };
    g.set_value(plug(node, PJ_CENTER), PlugValue::Vec3(centre))?;
    g.set_value(plug(node, PJ_ROTATE), PlugValue::Vec3(rotate))?;
    g.set_value(plug(node, PJ_WIDTH), PlugValue::Float(w.max(1e-6)))?;
    g.set_value(plug(node, PJ_HEIGHT), PlugValue::Float(h.max(1e-6)))?;
    g.set_value(plug(node, PJ_SWEEP), PlugValue::Float(sweep))?;
    g.set_value(plug(node, PJ_FACES), PlugValue::String(crate::history::format_components(ComponentMode::Face, faces)))?;
    let state = {
        let n = g.node(node).unwrap();
        projection_state(n.data())
    };
    g.set_value(plug(node, PJ_FITTED), PlugValue::String(state))?;
    Ok(())
}

impl NodeBehavior for UvEditNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        // Moved from where the projection fitted: reproject from the attributes.
        if let Some(ty) = self.projection {
            let fitted = d.string(PJ_FITTED);
            if !fitted.is_empty() && fitted != projection_state(d) {
                let mut m = (*input).clone();
                reproject(d, ty, &mut m);
                d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
                return;
            }
        }
        let edit = match d.value(EDIT) {
            PlugValue::Data(Some(t)) => t.as_any().downcast_ref::<UvEdit>().cloned(),
            _ => None,
        };
        let Some(e) = edit.filter(|e| !e.uvs.is_empty() || !e.seams.is_empty()) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let mut m = (*input).clone();
        e.apply(&mut m);
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register_types_now);
}

fn register_types_now() {
    crate::plug::data::register_kind("polyUvEdit", Arc::new(|j| Ok(Arc::new(UvEdit::from_json(j)) as Arc<dyn crate::plug::Data>)));
    for (name, doc) in NODE_TYPES {
        let projection = match *name {
            "polyPlanarProj" => Some(Projection::Planar),
            "polyCylProj" => Some(Projection::Cylindrical),
            "polySphProj" => Some(Projection::Spherical),
            _ => None,
        };
        let t = NodeType {
            name: name.to_string(),
            icon: "texture".into(),
            doc: doc.to_string(),
            own_attrs: {
                let mut own = vec![
                    mesh_attr("inputPolymesh", "ip", flags::INPUT | flags::HIDDEN, "The mesh before the UV edit."),
                    mesh_attr("output", "out", flags::OUTPUT | flags::HIDDEN, "The mesh after the UV edit."),
                    AttrDef {
                        name: "uvEdit".into(),
                        short_name: "uve".into(),
                        ty: PlugType::Data,
                        default_value: PlugValue::Data(None),
                        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                        doc: "The UVs and seams the edit set.".into(),
                        ..Default::default()
                    },
                ];
                if projection.is_some() {
                    let k = flags::INPUT | flags::STORABLE | flags::KEYABLE;
                    own.push(crate::node::vec3_attr("projectionCenter", "pc", DVec3::ZERO, k, "Where the projection sits, in the object's space."));
                    own.push(crate::node::vec3_attr("rotate", "ro", DVec3::ZERO, k, "How it is turned, degrees."));
                    own.push(crate::attr_defs::float_attr_short("projectionWidth", "pw", 1.0, "Its width.", None, None));
                    own.push(crate::attr_defs::float_attr_short("projectionHeight", "ph", 1.0, "Its height.", None, None));
                    own.push(crate::node::vec3_attr("imageCenter", "ic", DVec3::ZERO, k, "Offset in UV space; the third number is unused."));
                    own.push(crate::node::vec3_attr("imageScale", "is", DVec3::ONE, k, "Scale in UV space; the third number is unused."));
                    own.push(crate::attr_defs::float_attr_short("projectionHorizontalSweep", "phs", 360.0, "How far round a cylinder or sphere it wraps, degrees.", None, None));
                    for (nm, sh, doc) in [("fittedState", "fts", "The attributes as the projection left them."), ("inputComponents", "ics", "The faces projected.")] {
                        own.push(AttrDef { name: nm.into(), short_name: sh.into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: doc.into(), ..Default::default() });
                    }
                }
                own
            },
            create: Some(Arc::new(move || Box::new(UvEditNode { projection }))),
            ..Default::default()
        };
        types::register_type(t).expect(name);
    }
}

fn plug(n: NodeId, a: usize) -> PlugRef {
    PlugRef::new(n, a as i32)
}

/// Runs a UV edit on a mesh: as a history node named node_type when history records, else into the mesh itself.
pub fn edit_mesh(g: &mut Graph, mesh: NodeId, node_type: &str, f: impl FnOnce(&mut Mesh)) -> CmdResult<Option<NodeId>> {
    g.evaluate();
    let Some(cage) = crate::modifiers::cage(g, mesh) else { fail!("{}: not a mesh", g.path(mesh)) };
    let before = (*cage).clone();
    let mut after = before.clone();
    f(&mut after);
    let edit = UvEdit::diff(&before, &after);
    if edit.uvs.is_empty() && edit.seams.is_empty() {
        return Ok(None);
    }
    if crate::history::can_record(g, mesh) {
        let t = types::find(node_type).unwrap();
        let (base, src) = crate::history::upstream(g, mesh)?;
        let h = g.create_node(&t, "", NO_NODE, true)?;
        g.set_value(plug(h, EDIT), PlugValue::Data(Some(Arc::new(edit))))?;
        g.disconnect(&base);
        g.connect(&src, &plug(h, IN), false)?;
        g.connect(&plug(h, OUT), &base, false)?;
        return Ok(Some(h));
    }
    let base = crate::mesh_edit::base_plug(g, mesh)?;
    let mut m = crate::mesh_edit::base_mesh(g, &base);
    edit.apply(&mut m);
    g.set_value(base, PlugValue::Mesh(Some(Arc::new(m))))?;
    Ok(None)
}

// ---------------------------------------------------------------------------------------------------------------------
// Targets

/// What a UV command works on, per mesh shape: faces, edges and maps named, or the whole mesh.
#[derive(Default, Debug, Clone)]
pub struct Target {
    pub faces: Vec<Index>,
    pub edges: Vec<Index>,
    pub maps: Vec<Index>,
    pub whole: bool,
}

fn parse_ids(s: &str) -> Option<Vec<Index>> {
    let mut out = Vec::new();
    for part in s.split(',') {
        let part = part.trim();
        if part == "*" {
            return None;
        }
        match part.split_once(':') {
            Some((a, b)) => {
                let (a, b) = (a.trim().parse::<Index>().ok()?, b.trim().parse::<Index>().ok()?);
                out.extend(a.min(b)..=a.max(b));
            }
            None => out.push(part.parse().ok()?),
        }
    }
    Some(out)
}

pub fn targets(s: &mut Session, a: &Args, cmd: &str) -> CmdResult<BTreeMap<NodeId, Target>> {
    let g = &mut s.graph;
    let mut out: BTreeMap<NodeId, Target> = BTreeMap::new();
    let named = a.named();
    if named.is_empty() {
        let mode = components::mode(g);
        if mode == ComponentMode::Object {
            for id in g.selection().to_vec() {
                if let Some(m) = components::as_mesh(g, id) {
                    out.entry(m).or_default().whole = true;
                }
            }
        } else {
            for (node, ids) in components::selected_all(g, mode).clone() {
                let Some(m) = components::as_mesh(g, node) else { continue };
                let t = out.entry(m).or_default();
                match mode {
                    ComponentMode::Face => t.faces.extend(ids),
                    ComponentMode::Edge => t.edges.extend(ids),
                    _ => {
                        // Vertices: every map at those points.
                        if let Some(cage) = crate::modifiers::cage(g, m) {
                            let tp = topo(&cage);
                            for (c, &mp) in tp.corner_map.iter().enumerate() {
                                if mp != INVALID && ids.contains(&cage.corner_point(c as Index)) {
                                    t.maps.push(mp);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    for name in named {
        let (obj, comp) = match name.rfind('.') {
            Some(d) if name.ends_with(']') => (&name[..d], Some(&name[d + 1..])),
            _ => (name.as_str(), None),
        };
        let Some(id) = g.find(obj)? else { fail!("{cmd}: no object named {obj}") };
        let Some(m) = components::as_mesh(g, id) else { fail!("{cmd}: {obj} is not a mesh") };
        let t = out.entry(m).or_default();
        match comp {
            None => t.whole = true,
            Some(c) => {
                let open = c.find('[').unwrap_or(c.len());
                let kind = &c[..open];
                let ids = parse_ids(c[open..].trim_start_matches('[').trim_end_matches(']'));
                let Some(ids) = ids else {
                    t.whole = true;
                    continue;
                };
                match kind {
                    "f" => t.faces.extend(ids),
                    "e" => t.edges.extend(ids),
                    "map" => t.maps.extend(ids),
                    "vtx" => {
                        if let Some(cage) = crate::modifiers::cage(g, m) {
                            let tp = topo(&cage);
                            for (c, &mp) in tp.corner_map.iter().enumerate() {
                                if mp != INVALID && ids.contains(&cage.corner_point(c as Index)) {
                                    t.maps.push(mp);
                                }
                            }
                        }
                    }
                    _ => fail!("{cmd}: can't work on {name}"),
                }
            }
        }
    }
    if out.is_empty() {
        fail!("{cmd}: select meshes, faces, edges or UVs");
    }
    for t in out.values_mut() {
        for v in [&mut t.faces, &mut t.edges, &mut t.maps] {
            v.sort_unstable();
            v.dedup();
        }
    }
    Ok(out)
}

/// The faces a target covers: named faces, faces of named edges and maps, or all.
pub(crate) fn target_faces(m: &Mesh, t: &Target, tp: &UvTopo) -> Vec<Index> {
    if t.whole {
        return all_faces(m);
    }
    let mut f = t.faces.clone();
    for &e in &t.edges {
        if !m.edge_valid(e) {
            continue;
        }
        let c0 = m.edge_corner(e);
        let mut c = c0;
        loop {
            f.push(m.corner_face(c));
            c = m.corner_radial(c);
            if c == c0 || c == INVALID {
                break;
            }
        }
    }
    for &mp in &t.maps {
        for &c in tp.maps.get(mp as usize).map(Vec::as_slice).unwrap_or(&[]) {
            f.push(m.corner_face(c));
        }
    }
    f.sort_unstable();
    f.dedup();
    f
}

/// The edges a target names, the edges with both ends' UVs among its maps, or the edges of its faces.
fn target_edges(m: &Mesh, t: &Target, tp: &UvTopo) -> Vec<Index> {
    if !t.edges.is_empty() {
        return t.edges.clone();
    }
    if !t.maps.is_empty() && t.faces.is_empty() && !t.whole {
        let sel: std::collections::HashSet<Index> = t.maps.iter().copied().collect();
        let mut out: Vec<Index> = (0..m.corner_capacity())
            .filter(|&c| m.corner_valid(c) && m.face_valid(m.corner_face(c)))
            .filter(|&c| sel.contains(&tp.corner_map[c as usize]) && sel.contains(&tp.corner_map[m.corner_next(c) as usize]))
            .map(|c| m.corner_edge(c))
            .collect();
        out.sort_unstable();
        out.dedup();
        return out;
    }
    let faces = if t.whole { all_faces(m) } else { t.faces.clone() };
    let mut e: Vec<Index> = faces.iter().flat_map(|&f| m.face_corners(f)).map(|c| m.corner_edge(c)).collect();
    e.sort_unstable();
    e.dedup();
    e
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, def: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, def, doc)
}

pub(crate) fn each(s: &mut Session, a: &Args, cmd: &str, label: &str, node_type: &str, mut run: impl FnMut(&mut Mesh, &Target, &UvTopo)) -> CmdResult {
    let targets = targets(s, a, cmd)?;
    let g = &mut s.graph;
    g.open_chunk(label);
    let mut made = Vec::new();
    let r = (|| -> CmdResult<()> {
        for (mesh, t) in targets {
            let r = edit_mesh(g, mesh, node_type, |m| {
                let tp = topo(m);
                run(m, &t, &tp);
            })?;
            if let Some(h) = r {
                made.push(json!(g.path(h)));
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(Value::Array(made))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(
        Cmd::new(
            "setMeshUVs",
            "Set the UV of every face corner of a mesh at once, face by face in order: -u and -v, one number per corner. One \
             polyTweakUV history node.",
            |s, a| {
                let names = a.named();
                let Some(name) = names.first() else { fail!("setMeshUVs: name a mesh") };
                let g = &mut s.graph;
                let id = crate::rig::find(g, name, "setMeshUVs")?;
                let Some(mesh) = components::as_mesh(g, id) else { fail!("setMeshUVs: {name} is not a mesh") };
                let list = |k: &str| -> Vec<f64> { a.get(k).and_then(Value::as_array).map(|l| l.iter().filter_map(Value::as_f64).collect()).unwrap_or_default() };
                let (us, vs) = (list("u"), list("v"));
                if us.len() != vs.len() {
                    fail!("setMeshUVs: {} u values and {} v values", us.len(), vs.len());
                }
                let mut bad = None;
                edit_mesh(g, mesh, "polyTweakUV", |m| {
                    let corners: Vec<Index> = all_faces(m).into_iter().flat_map(|f| m.face_corners(f)).collect();
                    if corners.len() != us.len() {
                        bad = Some(corners.len());
                        return;
                    }
                    let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(UV);
                    for (i, c) in corners.into_iter().enumerate() {
                        layer[c as usize] = [us[i] as f32, vs[i] as f32];
                    }
                })?;
                if let Some(n) = bad {
                    fail!("setMeshUVs: {name} has {n} face corners, got {}", us.len());
                }
                Ok(json!(us.len()))
            },
        )
        .positional("a mesh")
        .flag(f("u", "", ArgType::List, Value::Null, "U per face corner."))
        .flag(f("v", "", ArgType::List, Value::Null, "V per face corner.")),
    );

    c.register(
        Cmd::new(
            "polyProjection",
            "Project UVs of the selected faces (or meshes) through a plane, cylinder or sphere. -md picks the plane's axis \
             (x, y, z) or b for best fit.",
            |s, a| {
                let ty = match a.string("type", "planar").to_lowercase().as_str() {
                    "planar" => Projection::Planar,
                    "cylindrical" => Projection::Cylindrical,
                    "spherical" => Projection::Spherical,
                    t => fail!("polyProjection: type {t} is not planar, cylindrical or spherical"),
                };
                let axis = match a.string("mapDirection", "b").to_lowercase().as_str() {
                    "x" => ProjectAxis::X,
                    "y" => ProjectAxis::Y,
                    "z" => ProjectAxis::Z,
                    _ => ProjectAxis::BestFit,
                };
                let node = match ty {
                    Projection::Cylindrical => "polyCylProj",
                    Projection::Spherical => "polySphProj",
                    _ => "polyPlanarProj",
                };
                let mut fits: Vec<(Mesh, Vec<Index>)> = Vec::new();
                let made = each(s, a, "polyProjection", "Projection", node, |m, t, tp| {
                    let faces = target_faces(m, t, tp);
                    fits.push((m.clone(), faces.clone()));
                    project_faces(m, &faces, ty, axis);
                })?;
                for (name, (m, faces)) in made.as_array().into_iter().flatten().zip(fits) {
                    if let Some(n) = name.as_str().and_then(|n| s.graph.find(n).ok().flatten()) {
                        fit_projection(&mut s.graph, n, &m, &faces, ty, axis)?;
                    }
                }
                Ok(made)
            },
        )
        .positional("faces or meshes")
        .flag(f("type", "t", ArgType::String, Value::Null, "planar, cylindrical or spherical."))
        .flag(f("mapDirection", "md", ArgType::String, Value::Null, "x, y, z, or b (best plane)."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, Value::Null, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "polyAutoProjection",
            "Project UVs from the six box planes, each face from the one it faces most, then lay the shells out in 0 to 1.",
            |s, a| {
                let spacing = a.float("percentageSpace", 0.2) / 100.0 * 10.0;
                each(s, a, "polyAutoProjection", "Automatic Projection", "polyAutoProj", |m, t, tp| {
                    let faces = target_faces(m, t, tp);
                    project_faces(m, &faces, Projection::Box, ProjectAxis::BestFit);
                    layout(m, &faces, spacing.clamp(0.0, 0.2), true);
                })
            },
        )
        .positional("faces or meshes")
        .flag(f("percentageSpace", "ps", ArgType::Float, Value::Null, "Space between shells, percent of the square."))
        .flag(f("layoutMethod", "lm", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("layout", "l", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("percentageSpaceLegacy", "pb", ArgType::Any, Value::Null, "Accepted for existing scripts."))
        .flag(f("insertBeforeDeformers", "ibd", ArgType::Any, Value::Null, "Accepted for existing scripts."))
        .flag(f("createNewMap", "cm", ArgType::Any, Value::Null, "Accepted for existing scripts."))
        .flag(f("scaleMode", "sc", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("optimize", "o", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("worldSpace", "ws", ArgType::Any, Value::Null, "Accepted for existing scripts."))
        .flag(f("uvSetName", "uvs", ArgType::String, Value::Null, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Any, Value::Null, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, Value::Null, "Accepted for existing scripts."))
        .flag(f("planes", "p", ArgType::Int, Value::Null, "Accepted for existing scripts (six planes).")),
    );

    c.register(
        Cmd::new(
            "u3dUnfold",
            "Unfold: flatten the UV shells of the selection with the least stretch, keeping each shell's place and size.",
            |s, a| {
                each(s, a, "u3dUnfold", "Unfold", "Unfold3DUnfold", |m, t, tp| {
                    let faces = target_faces(m, t, tp);
                    unfold(m, &faces);
                })
            },
        )
        .positional("faces, UVs or meshes")
        .flag(f("iterations", "ite", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("pack", "p", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("borderintersection", "bi", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("triangleflip", "tf", ArgType::Int, Value::Null, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "u3dLayout",
            "Layout: pack the UV shells of the selection into 0 to 1, each scaled to its size in 3D and turned to fit.",
            |s, a| {
                let spacing = a.float("shellSpacing", 0.0039);
                let rotate = a.int("rotateStep", 90) != 0;
                each(s, a, "u3dLayout", "Layout", "polyLayoutUV", |m, t, tp| {
                    let faces = target_faces(m, t, tp);
                    let shells: Vec<Index> = shells_of(tp, &faces).concat();
                    layout(m, &shells, spacing.max(0.002) * 2.0, rotate);
                })
            },
        )
        .positional("faces, UVs or meshes")
        .flag(f("shellSpacing", "spc", ArgType::Float, Value::Null, "Space between shells, in UV units."))
        .flag(f("rotateStep", "rst", ArgType::Float, Value::Null, "0 keeps shells unturned."))
        .flag(f("resolution", "res", ArgType::Int, Value::Null, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "polyMapCut",
            "Cut UVs along the selected edges: each side gets its own UVs, splitting shells.",
            |s, a| {
                each(s, a, "polyMapCut", "Cut UV Edges", "polyMapCut", |m, t, tp| {
                    let e = target_edges(m, t, tp);
                    cut(m, &e);
                })
            },
        )
        .positional("edges"),
    );

    c.register(
        Cmd::new(
            "polyMapSew",
            "Sew UVs along the selected edges: the two sides meet halfway.",
            |s, a| {
                each(s, a, "polyMapSew", "Sew UV Edges", "polyMapSew", |m, t, tp| {
                    let e = target_edges(m, t, tp);
                    sew(m, &e);
                })
            },
        )
        .positional("edges"),
    );

    c.register(
        Cmd::new(
            "polyMapSewMove",
            "Move and Sew: the smaller shell moves onto the other along the selected edges, then they're sewn.",
            |s, a| {
                each(s, a, "polyMapSewMove", "Move and Sew UVs", "polyMapSewMove", |m, t, tp| {
                    let e = target_edges(m, t, tp);
                    move_and_sew(m, &e);
                })
            },
        )
        .positional("edges")
        .flag(f("nf", "nf", ArgType::Int, Value::Null, "Accepted for existing scripts."))
        .flag(f("lps", "lps", ArgType::Int, Value::Null, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "polyEditUV",
            "Move (-u -v), scale (-su -sv) or turn (-a degrees) UVs about the pivot (-pu -pv). Relative by default; -r 0 \
             sets -u/-v. -q returns u, v of each UV.",
            |s, a| {
                if a.bool("query") {
                    let targets = targets(s, a, "polyEditUV")?;
                    let mut out = Vec::new();
                    for (mesh, t) in targets {
                        let Some(m) = crate::modifiers::cage(&mut s.graph, mesh) else { continue };
                        let tp = topo(&m);
                        let uv = m.corner_attrs().get::<[f32; 2]>(UV);
                        let maps: Vec<Index> = if t.maps.is_empty() {
                            let mut v: Vec<Index> = corners_of(&m, &tp, &target_faces(&m, &t, &tp), &[]).iter().map(|&c| tp.corner_map[c as usize]).collect();
                            v.sort_unstable();
                            v.dedup();
                            v
                        } else {
                            t.maps.clone()
                        };
                        for mp in maps {
                            if let Some(&c) = tp.maps.get(mp as usize).and_then(|v| v.first()) {
                                let p = uv_of(uv, c);
                                out.push(json!(p.x));
                                out.push(json!(p.y));
                            }
                        }
                    }
                    return Ok(Value::Array(out));
                }
                let rel = a.get("relative").and_then(Value::as_bool).unwrap_or(true);
                let (u, v) = (a.get("u").and_then(Value::as_f64), a.get("v").and_then(Value::as_f64));
                let scale = DVec2::new(a.float("scaleU", 1.0), a.float("scaleV", 1.0));
                let angle = a.float("angle", 0.0);
                let pivot = DVec2::new(a.float("pivotU", 0.0), a.float("pivotV", 0.0));
                each(s, a, "polyEditUV", "Edit UVs", "polyTweakUV", |m, t, tp| {
                    let faces = if t.whole { all_faces(m) } else { t.faces.clone() };
                    let corners = corners_of(m, tp, &faces, &t.maps);
                    if rel {
                        transform(m, &corners, DVec2::new(u.unwrap_or(0.0), v.unwrap_or(0.0)), scale, angle, pivot);
                    } else {
                        set_uv(m, &corners, u, v);
                        if scale != DVec2::ONE || angle != 0.0 {
                            transform(m, &corners, DVec2::ZERO, scale, angle, pivot);
                        }
                    }
                })
            },
        )
        .positional("UVs (pCube1.map[0:3]), faces or meshes")
        .flag(f("u", "u", ArgType::Float, Value::Null, "U offset (or value with -r 0)."))
        .flag(f("v", "v", ArgType::Float, Value::Null, "V offset (or value with -r 0)."))
        .flag(f("scaleU", "su", ArgType::Float, Value::Null, "Scale in U about the pivot."))
        .flag(f("scaleV", "sv", ArgType::Float, Value::Null, "Scale in V about the pivot."))
        .flag(f("angle", "a", ArgType::Float, Value::Null, "Turn, degrees, about the pivot."))
        .flag(f("pivotU", "pu", ArgType::Float, Value::Null, "Pivot U."))
        .flag(f("pivotV", "pv", ArgType::Float, Value::Null, "Pivot V."))
        .flag(f("relative", "r", ArgType::Bool, Value::Null, "Offsets (default) or values."))
        .flag(f("query", "q", ArgType::Bool, Value::Null, "Return u, v of each UV.")),
    );

    c.register(
        Cmd::new(
            "polyNormalizeUV",
            "Fit UVs into 0 to 1: all together (-nt 1) or each shell on its own (-nt 0), keeping proportions unless -pa 0.",
            |s, a| {
                let each_shell = a.int("normalizeType", 1) == 0;
                let keep = a.get("preserveAspectRatio").and_then(Value::as_bool).unwrap_or(true);
                each(s, a, "polyNormalizeUV", "Normalize UVs", "polyNormalizeUV", |m, t, tp| {
                    let faces = target_faces(m, t, tp);
                    let groups: Vec<Vec<Index>> = if each_shell { shells_of(tp, &faces) } else { vec![faces] };
                    for fs in groups {
                        let corners = corners_of(m, tp, &fs, &[]);
                        let (lo, hi) = bounds(m.corner_attrs().get::<[f32; 2]>(UV), &corners);
                        let size = hi - lo;
                        if size.x <= 0.0 && size.y <= 0.0 {
                            continue;
                        }
                        let mut sc = DVec2::new(1.0 / size.x.max(1e-12), 1.0 / size.y.max(1e-12));
                        if keep {
                            sc = DVec2::splat(sc.x.min(sc.y));
                        }
                        transform(m, &corners, -lo, sc, 0.0, lo);
                        // pivot lo: v' = lo + (v - lo) * s - lo = (v - lo) * s
                    }
                })
            },
        )
        .positional("faces or meshes")
        .flag(f("normalizeType", "nt", ArgType::Int, Value::Null, "1 all together, 0 each shell."))
        .flag(f("preserveAspectRatio", "pa", ArgType::Bool, Value::Null, "Keep proportions (default on).")),
    );

    c.register(
        Cmd::new(
            "polyFlipUV",
            "Mirror UVs across the middle of their bounds: -ft 0 horizontally (in U), 1 vertically.",
            |s, a| {
                let vertical = a.int("flipType", 0) == 1;
                let local = a.bool("local");
                each(s, a, "polyFlipUV", "Flip UVs", "polyFlipUV", |m, t, tp| {
                    let faces = if t.whole { all_faces(m) } else { t.faces.clone() };
                    let groups: Vec<Vec<Index>> = if local && t.maps.is_empty() { shells_of(tp, &faces) } else { vec![faces] };
                    for fs in groups {
                        let corners = corners_of(m, tp, &fs, &t.maps);
                        let (lo, hi) = bounds(m.corner_attrs().get::<[f32; 2]>(UV), &corners);
                        let c = (lo + hi) * 0.5;
                        let sc = if vertical { DVec2::new(1.0, -1.0) } else { DVec2::new(-1.0, 1.0) };
                        transform(m, &corners, DVec2::ZERO, sc, 0.0, c);
                    }
                })
            },
        )
        .positional("UVs, faces or meshes")
        .flag(f("flipType", "ft", ArgType::Int, Value::Null, "0 horizontal, 1 vertical."))
        .flag(f("local", "l", ArgType::Bool, Value::Null, "Each shell about its own middle.")),
    );

    c.register(
        Cmd::new(
            "polyRotateUVs",
            "Turn UVs about the middle of their bounds, degrees counterclockwise.",
            |s, a| {
                let angle = a.positional.first().and_then(Value::as_f64).unwrap_or(90.0);
                each(s, a, "polyRotateUVs", "Rotate UVs", "polyTweakUV", |m, t, tp| {
                    let faces = if t.whole { all_faces(m) } else { t.faces.clone() };
                    let corners = corners_of(m, tp, &faces, &t.maps);
                    let (lo, hi) = bounds(m.corner_attrs().get::<[f32; 2]>(UV), &corners);
                    transform(m, &corners, DVec2::ZERO, DVec2::ONE, angle, (lo + hi) * 0.5);
                })
            },
        )
        .positional("angle, then UVs, faces or meshes"),
    );
}

/// What the UV editor draws for a mesh: each map's UV, each face's maps, the edges where UVs split, shells.
pub fn editor_data(g: &mut Graph, mesh: NodeId) -> Option<Value> {
    let m = crate::modifiers::cage(g, mesh)?;
    let tp = topo(&m);
    let uv = m.corner_attrs().get::<[f32; 2]>(UV);
    let maps: Vec<Value> = tp.maps.iter().map(|cs| {
        let p = uv_of(uv, cs[0]);
        json!([p.x, p.y, m.corner_point(cs[0])])
    }).collect();
    let mut faces = Vec::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let cs = m.face_corners(f);
        let ids: Vec<Index> = cs.iter().map(|&c| tp.corner_map[c as usize]).collect();
        let edges: Vec<Index> = cs.iter().map(|&c| m.corner_edge(c)).collect();
        faces.push(json!([f, tp.face_shell[f as usize], ids, edges]));
    }
    let mut borders = Vec::new();
    for c in 0..m.corner_capacity() {
        if !m.corner_valid(c) || !m.face_valid(m.corner_face(c)) {
            continue;
        }
        let e = m.corner_edge(c);
        if tp.split[e as usize] {
            borders.push(json!([tp.corner_map[c as usize], tp.corner_map[m.corner_next(c) as usize], e]));
        }
    }
    Some(json!({"path": components::display_path(g, mesh), "maps": maps, "faces": faces, "borders": borders, "shells": tp.shells.len(), "hasUVs": m.corner_attrs().has(UV)}))
}

pub fn is_uv_node(type_name: &str) -> bool {
    NODE_TYPES.iter().any(|(n, _)| *n == type_name)
}
