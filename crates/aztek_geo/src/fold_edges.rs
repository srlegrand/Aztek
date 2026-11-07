//! Edge rules for automatic bevelling: which edges of a mesh are folds worth rounding off, and how to find an edge
//! again after the mesh around it was rebuilt.

use std::cell::OnceCell;

use glam::DVec3;
use rayon::prelude::*;

use crate::attribute::{names, Index, INVALID};
use crate::mesh::Mesh;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FoldSide {
    #[default]
    Both,
    /// Convex.
    Outside,
    /// Concave.
    Inside,
}

#[derive(Clone, Copy, Debug)]
pub struct FoldEdgeOptions {
    /// Range of the angle between the two face normals, degrees (0 is flat).
    pub min_angle: f64,
    pub max_angle: f64,
    pub side: FoldSide,
    /// Thickness edges are skipped: shorter than this, or with a face narrower than this at the edge (distance from
    /// the edge midpoint to the nearest face edge not touching it). 0 keeps everything.
    pub thin: f64,
    pub min_length: f64,
    /// Where folds meet at an angle, follow quad loops from the corner until they reach another fold, and add them.
    pub complete_corners: bool,
}

impl Default for FoldEdgeOptions {
    fn default() -> Self {
        Self { min_angle: 30.0, max_angle: 180.0, side: FoldSide::Both, thin: 0.0, min_length: 0.0, complete_corners: true }
    }
}

fn point_segment_distance(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let len2 = ab.dot(ab);
    let t = if len2 > 0.0 { ((p - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
    (p - (a + ab * t)).length()
}

/// The edge's two faces, None unless it has exactly two.
fn two_faces(m: &Mesh, e: Index) -> Option<(Index, Index)> {
    if !m.edge_valid(e) || !m.is_manifold_edge(e) {
        return None;
    }
    let c = m.edge_corner(e);
    let f0 = m.corner_face(c);
    let f1 = m.corner_face(m.corner_radial(c));
    (f0 != f1).then_some((f0, f1))
}

fn fold_angle(n0: DVec3, n1: DVec3) -> f64 {
    n0.dot(n1).clamp(-1.0, 1.0).acos().to_degrees()
}

/// s < 0 on a convex edge: each face's centroid lies behind the other face's plane.
fn convexity(m: DVec3, c0: DVec3, n0: DVec3, c1: DVec3, n1: DVec3) -> f64 {
    (c1 - m).dot(n0) + (c0 - m).dot(n1)
}

fn width_at(m: &Mesh, f: Index, e: Index) -> f64 {
    let a = m.edge_point(e, 0);
    let b = m.edge_point(e, 1);
    let mid = (m.position(a) + m.position(b)) * 0.5;
    let mut best = f64::MAX;
    if m.face_size(f) == 3 {
        m.for_each_face_corner(f, |c| {
            let p = m.corner_point(c);
            if p != a && p != b {
                best = best.min((m.position(p) - mid).length());
            }
        });
        return best;
    }
    m.for_each_face_corner(f, |c| {
        let p = m.corner_point(c);
        let q = m.corner_next_point(c);
        if p != a && p != b && q != a && q != b {
            best = best.min(point_segment_distance(mid, m.position(p), m.position(q)));
        }
    });
    best
}

/// At a valence-4 interior point, the edge across from e (sharing no face with it).
fn opposite_edge(m: &Mesh, e: Index, p: Index) -> Index {
    let Some((f0, f1)) = two_faces(m, e) else { return INVALID };
    let mut found = INVALID;
    let mut count = 0;
    m.for_each_point_edge(p, |o| {
        if o == e {
            return;
        }
        let mut touches = false;
        m.for_each_edge_corner(o, |c| touches |= m.corner_face(c) == f0 || m.corner_face(c) == f1);
        if !touches {
            found = o;
            count += 1;
        }
    });
    if count == 1 {
        found
    } else {
        INVALID
    }
}

fn mask_to_list(mask: &[bool]) -> Vec<Index> {
    (0..mask.len() as Index).filter(|&e| mask[e as usize]).collect()
}

fn complete_corners(m: &Mesh, sel: &mut [bool], usable: &[bool]) {
    let base = sel.to_vec();
    let selected_at = |p: Index, out: Option<&mut Vec<Index>>| {
        let mut n = 0;
        let mut list = Vec::new();
        m.for_each_point_edge(p, |e| {
            if base[e as usize] {
                n += 1;
                list.push(e);
            }
        });
        if let Some(o) = out {
            o.extend(list);
        }
        n
    };

    let mut seen = vec![false; m.point_capacity() as usize];
    let mut here = Vec::new();
    for e0 in 0..m.edge_capacity() {
        if !base[e0 as usize] {
            continue;
        }
        for s in 0..2 {
            let v = m.edge_point(e0, s);
            if seen[v as usize] {
                continue;
            }
            seen[v as usize] = true;
            here.clear();
            let n = selected_at(v, Some(&mut here));
            let mut corner = n >= 3;
            if n == 2 {
                let d0 = (m.position(m.edge_other_point(here[0], v)) - m.position(v)).normalize();
                let d1 = (m.position(m.edge_other_point(here[1], v)) - m.position(v)).normalize();
                corner = d0.dot(d1) > -0.94;
            }
            if !corner {
                continue;
            }
            let mut starts = Vec::new();
            m.for_each_point_edge(v, |e| starts.push(e));
            for start in starts {
                if base[start as usize] || !usable[start as usize] {
                    continue;
                }
                let mut path = vec![start];
                let mut prev = start;
                let mut at = m.edge_other_point(start, v);
                loop {
                    if selected_at(at, None) > 0 {
                        for &pe in &path {
                            sel[pe as usize] = true;
                        }
                        break;
                    }
                    if m.is_boundary_point(at) || m.point_valence(at) != 4 {
                        break;
                    }
                    let next = opposite_edge(m, prev, at);
                    if next == INVALID || !usable[next as usize] || base[next as usize] || path.contains(&next) {
                        break;
                    }
                    path.push(next);
                    at = m.edge_other_point(next, at);
                    prev = next;
                }
            }
        }
    }
}

/// Angle between the normals of the edge's two faces, degrees; negative unless the edge has exactly two faces.
pub fn edge_fold_angle(m: &Mesh, e: Index) -> f64 {
    match two_faces(m, e) {
        Some((f0, f1)) => fold_angle(m.face_normal(f0), m.face_normal(f1)),
        None => -1.0,
    }
}

/// True for a convex edge (the faces fold away from their normals).
pub fn edge_is_convex(m: &Mesh, e: Index) -> bool {
    let Some((f0, f1)) = two_faces(m, e) else { return false };
    let mid = (m.position(m.edge_point(e, 0)) + m.position(m.edge_point(e, 1))) * 0.5;
    convexity(mid, m.face_centroid(f0), m.face_normal(f0), m.face_centroid(f1), m.face_normal(f1)) < 0.0
}

/// The face's width across from edge e, as the thin test measures it.
pub fn face_width_at(m: &Mesh, f: Index, e: Index) -> f64 {
    width_at(m, f, e)
}

/// Fold edges by the rule. Sorted.
pub fn fold_edges(m: &Mesh, o: &FoldEdgeOptions) -> Vec<Index> {
    let nf = m.face_capacity();
    let ne = m.edge_capacity();
    let frames: Vec<(DVec3, DVec3)> = (0..nf)
        .into_par_iter()
        .map(|f| if m.face_valid(f) { (m.face_normal(f), m.face_centroid(f)) } else { (DVec3::ZERO, DVec3::ZERO) })
        .collect();

    // 1 = fold, 2 = usable for corner loops but not a fold.
    let state: Vec<u8> = (0..ne)
        .into_par_iter()
        .map(|e| {
            let Some((f0, f1)) = two_faces(m, e) else { return 0 };
            let a = m.position(m.edge_point(e, 0));
            let b = m.position(m.edge_point(e, 1));
            let len = (b - a).length();
            if len < o.min_length || len <= 0.0 {
                return 0;
            }
            if o.thin > 0.0 && (len < o.thin || width_at(m, f0, e) < o.thin || width_at(m, f1, e) < o.thin) {
                return 0;
            }
            let angle = fold_angle(frames[f0 as usize].0, frames[f1 as usize].0);
            if angle < o.min_angle || angle > o.max_angle {
                return 2;
            }
            if o.side != FoldSide::Both {
                let convex = convexity(
                    (a + b) * 0.5,
                    frames[f0 as usize].1,
                    frames[f0 as usize].0,
                    frames[f1 as usize].1,
                    frames[f1 as usize].0,
                ) < 0.0;
                if convex != (o.side == FoldSide::Outside) {
                    return 2;
                }
            }
            1
        })
        .collect();

    let mut sel: Vec<bool> = state.iter().map(|&s| s == 1).collect();
    let usable: Vec<bool> = state.iter().map(|&s| s != 0).collect();
    if o.complete_corners {
        complete_corners(m, &mut sel, &usable);
    }
    mask_to_list(&sel)
}

/// Blender's bevel limit "Angle": two-face edges whose normals differ by more than the angle. Sorted.
pub fn angle_edges(m: &Mesh, degrees: f64) -> Vec<Index> {
    let cos_limit = degrees.to_radians().cos();
    let sel: Vec<bool> = (0..m.edge_capacity())
        .into_par_iter()
        .map(|e| two_faces(m, e).is_some_and(|(f0, f1)| m.face_normal(f0).dot(m.face_normal(f1)) < cos_limit))
        .collect();
    mask_to_list(&sel)
}

/// Edges whose float "bevelWeight" is above the threshold. Sorted.
pub fn weighted_edges(m: &Mesh, threshold: f64) -> Vec<Index> {
    let w = m.edge_attrs().get::<f32>(names::BEVEL_WEIGHT);
    (0..(w.len() as Index).min(m.edge_capacity()))
        .filter(|&e| m.edge_valid(e) && w[e as usize] as f64 > threshold)
        .collect()
}

/// Remembers one edge well enough to find it again in a changed mesh.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EdgeRef {
    pub edge: Index,
    pub p0: Index,
    pub p1: Index,
    pub a: DVec3,
    pub b: DVec3,
}

impl Default for EdgeRef {
    fn default() -> Self {
        Self { edge: INVALID, p0: INVALID, p1: INVALID, a: DVec3::ZERO, b: DVec3::ZERO }
    }
}

pub fn edge_ref(m: &Mesh, e: Index) -> EdgeRef {
    let p0 = m.edge_point(e, 0);
    let p1 = m.edge_point(e, 1);
    EdgeRef { edge: e, p0, p1, a: m.position(p0), b: m.position(p1) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeMatch {
    Lost,
    /// Same id joining the same points (topology unchanged).
    Id,
    /// Same id or another one with the same endpoints.
    Position,
    /// A collinear edge overlapping the old one, nearest its midpoint.
    Overlap,
}

fn cell_key(x: i64, y: i64, z: i64) -> u64 {
    (x as u64).wrapping_mul(0x9E3779B97F4A7C15) ^ (y as u64).wrapping_mul(0xC2B2AE3D27D4EB4F) ^ (z as u64).wrapping_mul(0x165667B19E3779F9)
}

/// Sorted (cell, element) pairs; a cell's elements are an equal range.
#[derive(Default)]
struct CellList {
    items: Vec<(u64, Index)>,
}

impl CellList {
    fn sort(&mut self) {
        self.items.sort_unstable();
    }
    fn each(&self, key: u64, mut f: impl FnMut(Index)) {
        let start = self.items.partition_point(|&(k, _)| k < key);
        for &(k, e) in &self.items[start..] {
            if k != key {
                break;
            }
            f(e);
        }
    }
}

struct Grids {
    point_cell: f64,
    points: CellList,
    edge_cell: f64,
    edges: CellList,
    /// Edges spanning too many cells to list in each.
    big: Vec<Index>,
}

fn cell_of(v: f64, cell: f64) -> i64 {
    (v / cell).floor() as i64
}

/// Finds remembered edges in a mesh: by id when the topology is unchanged, else by exact endpoints, else by the
/// collinear overlapping edge nearest the old midpoint. Spatial grids are built on first use, so lookups stay near
/// constant time on big meshes.
pub struct EdgeMatcher<'a> {
    mesh: &'a Mesh,
    tol: f64,
    grids: OnceCell<Grids>,
}

impl<'a> EdgeMatcher<'a> {
    pub fn new(mesh: &'a Mesh) -> Self {
        let b = mesh.bounds();
        Self { mesh, tol: (b.size().length() * 1e-7).max(1e-9), grids: OnceCell::new() }
    }

    pub fn tolerance(&self) -> f64 {
        self.tol
    }

    fn grids(&self) -> &Grids {
        self.grids.get_or_init(|| {
            let m = self.mesh;
            let mut g = Grids {
                point_cell: self.tol * 4.0,
                points: CellList::default(),
                edge_cell: 1.0,
                edges: CellList::default(),
                big: Vec::new(),
            };
            for p in 0..m.point_capacity() {
                if m.point_valid(p) {
                    let q = m.position(p);
                    let pc = g.point_cell;
                    g.points.items.push((cell_key(cell_of(q.x, pc), cell_of(q.y, pc), cell_of(q.z, pc)), p));
                }
            }
            g.points.sort();
            let mut total = 0.0;
            let mut count = 0usize;
            for e in 0..m.edge_capacity() {
                if m.edge_valid(e) {
                    total += m.edge_length(e);
                    count += 1;
                }
            }
            g.edge_cell = (self.tol * 16.0).max(if count > 0 { 2.0 * total / count as f64 } else { 1.0 });
            for e in 0..m.edge_capacity() {
                if !m.edge_valid(e) {
                    continue;
                }
                let a = m.position(m.edge_point(e, 0));
                let b = m.position(m.edge_point(e, 1));
                let lo = a.min(b) - self.tol;
                let hi = a.max(b) + self.tol;
                let mut c0 = [0i64; 3];
                let mut c1 = [0i64; 3];
                let mut cells = 1i64;
                for k in 0..3 {
                    c0[k] = cell_of(lo[k], g.edge_cell);
                    c1[k] = cell_of(hi[k], g.edge_cell);
                    cells *= c1[k] - c0[k] + 1;
                }
                if cells > 64 {
                    g.big.push(e);
                    continue;
                }
                for x in c0[0]..=c1[0] {
                    for y in c0[1]..=c1[1] {
                        for z in c0[2]..=c1[2] {
                            g.edges.items.push((cell_key(x, y, z), e));
                        }
                    }
                }
            }
            g.edges.sort();
            g
        })
    }

    fn by_endpoints(&self, a: DVec3, b: DVec3) -> Index {
        let g = self.grids();
        let m = self.mesh;
        let mut found = INVALID;
        let (cx, cy, cz) = (cell_of(a.x, g.point_cell), cell_of(a.y, g.point_cell), cell_of(a.z, g.point_cell));
        for x in cx - 1..=cx + 1 {
            for y in cy - 1..=cy + 1 {
                for z in cz - 1..=cz + 1 {
                    if found != INVALID {
                        return found;
                    }
                    g.points.each(cell_key(x, y, z), |p| {
                        if found != INVALID || (m.position(p) - a).length() > self.tol {
                            return;
                        }
                        m.for_each_point_edge(p, |e| {
                            if found == INVALID && (m.position(m.edge_other_point(e, p)) - b).length() <= self.tol {
                                found = e;
                            }
                        });
                    });
                }
            }
        }
        found
    }

    fn by_overlap(&self, a: DVec3, b: DVec3) -> Index {
        let g = self.grids();
        let m = self.mesh;
        let ab = b - a;
        let len = ab.length();
        if len <= self.tol {
            return INVALID;
        }
        let dir = ab / len;
        let mid = (a + b) * 0.5;
        let line_tol = (self.tol * 10.0).max(len * 1e-5);

        let mut best = INVALID;
        let mut best_dist = f64::MAX;
        let mut consider = |e: Index| {
            let c = m.position(m.edge_point(e, 0));
            let d = m.position(m.edge_point(e, 1));
            let off_line = |p: DVec3| (p - a).cross(dir).length();
            if off_line(c) > line_tol || off_line(d) > line_tol {
                return;
            }
            let tc = (c - a).dot(dir);
            let td = (d - a).dot(dir);
            let lo = tc.min(td).max(0.0);
            let hi = tc.max(td).min(len);
            if hi - lo <= line_tol {
                return;
            }
            let dist = ((c + d) * 0.5 - mid).length();
            if dist < best_dist {
                best_dist = dist;
                best = e;
            }
        };

        let lo = a.min(b) - line_tol;
        let hi = a.max(b) + line_tol;
        let mut c0 = [0i64; 3];
        let mut c1 = [0i64; 3];
        let mut cells = 1i64;
        for k in 0..3 {
            c0[k] = cell_of(lo[k], g.edge_cell);
            c1[k] = cell_of(hi[k], g.edge_cell);
            cells *= c1[k] - c0[k] + 1;
        }
        if cells > 4096 {
            for e in 0..m.edge_capacity() {
                if m.edge_valid(e) {
                    consider(e);
                }
            }
            return best;
        }
        for x in c0[0]..=c1[0] {
            for y in c0[1]..=c1[1] {
                for z in c0[2]..=c1[2] {
                    g.edges.each(cell_key(x, y, z), &mut consider);
                }
            }
        }
        for &e in &g.big {
            consider(e);
        }
        best
    }

    /// same_topology: the mesh has the element counts the ref was taken at, so its ids still mean the same edges.
    pub fn find(&self, r: &EdgeRef, same_topology: bool) -> (Index, EdgeMatch) {
        let m = self.mesh;
        let result = |e: Index, kind: EdgeMatch| (e, if e == INVALID { EdgeMatch::Lost } else { kind });
        if r.edge != INVALID && m.edge_valid(r.edge) {
            let q0 = m.edge_point(r.edge, 0);
            let q1 = m.edge_point(r.edge, 1);
            if same_topology && ((q0 == r.p0 && q1 == r.p1) || (q0 == r.p1 && q1 == r.p0)) {
                return result(r.edge, EdgeMatch::Id);
            }
            let qa = m.position(q0);
            let qb = m.position(q1);
            if ((qa - r.a).length() <= self.tol && (qb - r.b).length() <= self.tol)
                || ((qa - r.b).length() <= self.tol && (qb - r.a).length() <= self.tol)
            {
                return result(r.edge, EdgeMatch::Position);
            }
        }
        let e = self.by_endpoints(r.a, r.b);
        if e != INVALID {
            return result(e, EdgeMatch::Position);
        }
        result(self.by_overlap(r.a, r.b), EdgeMatch::Overlap)
    }
}
