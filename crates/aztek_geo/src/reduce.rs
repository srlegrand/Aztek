//! Quadric edge-collapse simplification.

use std::cmp::Ordering;
use std::collections::{BTreeSet, BinaryHeap};

use glam::{DVec2, DVec3};

use crate::attribute::{names, Index, INVALID};
use crate::mesh::Mesh;
use crate::normals::{point_normals, NormalWeight};
use crate::ops::basic::{
    delete_faces, mirror_mesh, quadrangulate_faces, triangulate_faces, MirrorOptions, QuadrangulateOptions,
};
use crate::primitives::mark_uv_seams;
use crate::symmetry::{axis_plane, clear_symmetry_keys};
use crate::triangulate::TriangulateOptions;

#[derive(Clone, Debug)]
pub struct ReduceOptions {
    /// Fraction of triangles to keep, 0 to 1.
    pub ratio: f64,
    /// Triangles to keep; positive overrides ratio.
    pub target_triangles: Index,
    /// Open borders stay exactly where they are.
    pub keep_borders: bool,
    /// Hard edges stay intact like UV seams, material borders and colour splits always do: points slide along them
    /// but never off them, and their corners stay. Off lets hard edges collapse for a lower count.
    pub keep_hard_edges: bool,
    /// -1 off, 0/1/2 to reduce the +x/+y/+z half and mirror it, so the result stays exactly symmetric.
    pub symmetry_axis: i32,
    /// Where the symmetry plane lies along its axis (the object's pivot).
    pub symmetry_offset: f64,
    /// Points this close to the symmetry plane count as on it.
    pub symmetry_tolerance: f64,
    /// Join triangle pairs back into quads where they make good ones.
    pub quads: bool,
}

impl Default for ReduceOptions {
    fn default() -> Self {
        Self {
            ratio: 0.5,
            target_triangles: 0,
            keep_borders: true,
            keep_hard_edges: true,
            symmetry_axis: -1,
            symmetry_offset: 0.0,
            symmetry_tolerance: 1e-4,
            quads: false,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReduceResult {
    pub mesh: Mesh,
    /// Shape error reached, as a fraction of the mesh's size.
    pub error: f64,
    /// Symmetry was asked for but faces straddle the plane, so the mesh was reduced whole.
    pub symmetry_ignored: bool,
}

fn live_faces(m: &Mesh) -> Vec<Index> {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()
}

fn find_root(parent: &mut [Index], mut x: Index) -> Index {
    while parent[x as usize] != x {
        parent[x as usize] = parent[parent[x as usize] as usize];
        x = parent[x as usize];
    }
    x
}

/// Per corner: an id shared by the corners around a point that a run of soft edges joins (one smoothing fan).
fn smooth_fans(m: &Mesh, use_hard_edges: bool) -> Vec<Index> {
    let mut parent: Vec<Index> = (0..m.corner_capacity()).collect();
    let hard = m.edge_attrs().get::<bool>(names::HARD);
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        m.for_each_face_corner(f, |c| {
            let e = m.corner_edge(c);
            if !m.is_manifold_edge(e) || (use_hard_edges && (e as usize) < hard.len() && hard[e as usize]) {
                return;
            }
            let other = m.corner_radial(c);
            let p = m.corner_point(c);
            let q = m.corner_next_point(c);
            let at_p = if m.corner_point(other) == p { other } else { m.corner_next(other) };
            let at_q = if m.corner_point(other) == q { other } else { m.corner_next(other) };
            let a = find_root(&mut parent, c);
            let b = find_root(&mut parent, at_p);
            parent[a as usize] = b;
            let a = find_root(&mut parent, m.corner_next(c));
            let b = find_root(&mut parent, at_q);
            parent[a as usize] = b;
        });
    }
    for c in 0..parent.len() as Index {
        parent[c as usize] = find_root(&mut parent, c);
    }
    parent
}

struct Wedges {
    of_corner: Vec<Index>,
    corner: Vec<Index>,
    point: Vec<Index>,
    fan: Vec<Index>,
}

/// Splits each point into one wedge per distinct look: smoothing fan, corner attributes (tangents aside), material.
fn build_wedges(m: &Mesh, use_hard_edges: bool) -> Wedges {
    let mut w = Wedges { of_corner: vec![INVALID; m.corner_capacity() as usize], corner: vec![], point: vec![], fan: vec![] };
    let fans = smooth_fans(m, use_hard_edges);
    let layers: Vec<_> = (0..m.corner_attrs().layer_count())
        .map(|i| m.corner_attrs().layer(i))
        .filter(|l| l.name() != names::TANGENT)
        .collect();
    let material = m.face_attrs().get::<i32>(names::MATERIAL);
    let same = |a: Index, b: Index| {
        if fans[a as usize] != fans[b as usize] {
            return false;
        }
        if !material.is_empty() && material[m.corner_face(a) as usize] != material[m.corner_face(b) as usize] {
            return false;
        }
        layers.iter().all(|l| l.raw(a as usize) == l.raw(b as usize))
    };
    let mut mine: Vec<Index> = Vec::new();
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        mine.clear();
        m.for_each_point_corner(p, |c| {
            for &k in &mine {
                if same(w.corner[k as usize], c) {
                    w.of_corner[c as usize] = k;
                    return;
                }
            }
            let k = w.corner.len() as Index;
            w.corner.push(c);
            w.point.push(p);
            w.fan.push(fans[c as usize]);
            w.of_corner[c as usize] = k;
            mine.push(k);
        });
    }
    w
}

/// Symmetric 4x4 error quadric, upper triangle.
#[derive(Clone, Copy, Default)]
struct Quadric {
    q: [f64; 10],
}

impl Quadric {
    fn add_plane(&mut self, n: DVec3, d: f64, w: f64) {
        let v = [n.x, n.y, n.z, d];
        let mut k = 0;
        for i in 0..4 {
            for j in i..4 {
                self.q[k] += w * v[i] * v[j];
                k += 1;
            }
        }
    }
    fn add(&mut self, o: &Quadric) {
        for k in 0..10 {
            self.q[k] += o.q[k];
        }
    }
    fn eval(&self, p: DVec3) -> f64 {
        let q = &self.q;
        let (x, y, z) = (p.x, p.y, p.z);
        q[0] * x * x + 2.0 * q[1] * x * y + 2.0 * q[2] * x * z + 2.0 * q[3] * x + q[4] * y * y + 2.0 * q[5] * y * z
            + 2.0 * q[6] * y
            + q[7] * z * z
            + 2.0 * q[8] * z
            + q[9]
    }
}

#[derive(Clone, Copy)]
struct Entry {
    cost: f64,
    from: Index,
    to: Index,
    version: u32,
}

impl PartialEq for Entry {
    fn eq(&self, o: &Self) -> bool {
        self.cmp(o) == Ordering::Equal
    }
}
impl Eq for Entry {}
impl PartialOrd for Entry {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Entry {
    /// Reversed, so the max-heap pops the cheapest first; ties as std::tuple with std::greater.
    fn cmp(&self, o: &Self) -> Ordering {
        o.cost
            .total_cmp(&self.cost)
            .then(o.from.cmp(&self.from))
            .then(o.to.cmp(&self.to))
            .then(o.version.cmp(&self.version))
    }
}

/// Half-edge collapse simplifier over wedges. A point merges into a neighbour, which keeps its position and
/// attributes, so nothing is interpolated. A point on a seam, hard edge or border may only slide along it, and never
/// past where it ends or branches. Every collapse is checked: the link condition (no pinched or doubled edges), and no
/// triangle may turn over, collapse to a sliver, or turn over in uv.
struct Collapser<'a> {
    mesh: &'a Mesh,
    w: &'a Wedges,
    uv: &'a [[f32; 2]],
    locked: &'a [u8],
    keep_borders: bool,
    tris: Vec<[Index; 3]>,
    alive: Vec<u8>,
    start_normal: Vec<DVec3>,
    smooth: Vec<DVec3>,
    point_tris: Vec<Vec<Index>>,
    q: Vec<Quadric>,
    version: Vec<u32>,
    alive_count: Index,
    min_area: f64,
    max_cost: f64,
}

impl<'a> Collapser<'a> {
    fn new(m: &'a Mesh, w: &'a Wedges, uv: &'a [[f32; 2]], locked: &'a [u8], keep_borders: bool) -> Self {
        let np = m.point_capacity() as usize;
        let mut s = Collapser {
            mesh: m,
            w,
            uv,
            locked,
            keep_borders,
            tris: vec![],
            alive: vec![],
            start_normal: vec![],
            smooth: vec![],
            point_tris: vec![Vec::new(); np],
            q: vec![Quadric::default(); np],
            version: vec![0; np],
            alive_count: 0,
            min_area: 0.0,
            max_cost: 0.0,
        };
        for f in 0..m.face_capacity() {
            if !m.face_valid(f) {
                continue;
            }
            let mut t = [0; 3];
            let mut k = 0;
            m.for_each_face_corner(f, |c| {
                t[k] = w.of_corner[c as usize];
                k += 1;
            });
            let id = s.tris.len() as Index;
            s.tris.push(t);
            s.alive.push(1);
            s.start_normal.push(s.tri_normal(&t));
            for x in t {
                s.point_tris[w.point[x as usize] as usize].push(id);
            }
        }
        s.alive_count = s.tris.len() as Index;
        s.smooth = point_normals(m, NormalWeight::AreaAngle).iter().map(|n| n.as_dvec3()).collect();
        let size = m.bounds().size().length();
        s.min_area = 1e-12 * size * size;
        for t in 0..s.tris.len() {
            let n = s.tri_normal(&s.tris[t]);
            let len = n.length();
            if len <= 0.0 {
                continue;
            }
            let u = n / len;
            for x in s.tris[t] {
                let p = w.point[x as usize];
                let pp = s.pos(p);
                s.q[p as usize].add_plane(u, -u.dot(pp), 0.5 * len);
            }
        }
        // Planes standing on the feature edges keep seams, borders and hard lines straight as points slide along them.
        for p in 0..np as Index {
            for o in s.neighbours(p) {
                if p < o && s.is_feature(p, o, true) {
                    let t = if s.point_tris[p as usize].is_empty() { INVALID } else { s.first_shared(p, o) };
                    if t == INVALID {
                        continue;
                    }
                    let e = s.pos(o) - s.pos(p);
                    let mut n = e.cross(s.tri_normal(&s.tris[t as usize]).normalize());
                    let len = n.length();
                    if len <= 0.0 {
                        continue;
                    }
                    n /= len;
                    let weight = 10.0 * e.dot(e);
                    let (pp, po) = (s.pos(p), s.pos(o));
                    s.q[p as usize].add_plane(n, -n.dot(pp), weight);
                    s.q[o as usize].add_plane(n, -n.dot(po), weight);
                }
            }
        }
        s
    }

    fn run(&mut self, target_tris: Index) {
        let mut heap = BinaryHeap::new();
        fn push(s: &Collapser, heap: &mut BinaryHeap<Entry>, u: Index) {
            let (c, to) = s.best_collapse(u);
            if to != INVALID {
                heap.push(Entry { cost: c, from: u, to, version: s.version[u as usize] });
            }
        }
        for p in 0..self.point_tris.len() as Index {
            push(self, &mut heap, p);
        }
        while self.alive_count > target_tris {
            let Some(Entry { cost, from: u, to: v, version }) = heap.pop() else { break };
            if version != self.version[u as usize] || self.point_tris[u as usize].is_empty() {
                continue;
            }
            if !self.can_collapse(u, v) {
                self.version[u as usize] += 1;
                push(self, &mut heap, u);
                continue;
            }
            self.collapse(u, v);
            self.max_cost = self.max_cost.max(cost);
            // u's neighbours all became v's, so v's ring is everything whose options changed.
            let mut touched = self.neighbours(v);
            touched.push(v);
            for p in touched {
                self.version[p as usize] += 1;
                push(self, &mut heap, p);
            }
        }
    }

    fn triangles(&self) -> Vec<[Index; 3]> {
        (0..self.tris.len()).filter(|&t| self.alive[t] != 0).map(|t| self.tris[t]).collect()
    }

    fn pos(&self, p: Index) -> DVec3 {
        self.mesh.position(p)
    }

    fn tri_normal(&self, t: &[Index; 3]) -> DVec3 {
        let a = self.pos(self.w.point[t[0] as usize]);
        let b = self.pos(self.w.point[t[1] as usize]);
        let c = self.pos(self.w.point[t[2] as usize]);
        (b - a).cross(c - a)
    }

    fn uv_area(&self, t: &[Index; 3]) -> f64 {
        if self.uv.is_empty() {
            return 0.0;
        }
        let g = |k: usize| -> DVec2 { DVec2::from(self.uv[self.w.corner[t[k] as usize] as usize].map(f64::from)) };
        let (a, b, c) = (g(0), g(1), g(2));
        (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
    }

    fn slot(&self, t: Index, p: Index) -> i32 {
        for k in 0..3 {
            if self.w.point[self.tris[t as usize][k] as usize] == p {
                return k as i32;
            }
        }
        -1
    }

    fn neighbours(&self, p: Index) -> Vec<Index> {
        let mut out = Vec::new();
        for &t in &self.point_tris[p as usize] {
            for &x in &self.tris[t as usize] {
                let q = self.w.point[x as usize];
                if q != p && !out.contains(&q) {
                    out.push(q);
                }
            }
        }
        out
    }

    fn first_shared(&self, a: Index, b: Index) -> Index {
        self.point_tris[a as usize].iter().copied().find(|&t| self.slot(t, b) >= 0).unwrap_or(INVALID)
    }

    /// Border, non-manifold, or the two faces disagree on a wedge at a (or, with both ends, at either end). A split
    /// only at the far end, like the fan of uv corners at a sphere's pole, doesn't hold a back.
    fn is_feature(&self, a: Index, b: Index, both_ends: bool) -> bool {
        let mut shared = [0; 2];
        let mut n = 0;
        for &t in &self.point_tris[a as usize] {
            if self.slot(t, b) >= 0 {
                if n == 2 {
                    return true;
                }
                shared[n] = t;
                n += 1;
            }
        }
        if n != 2 {
            return true;
        }
        let at = |t: Index, p: Index| self.tris[t as usize][self.slot(t, p) as usize];
        at(shared[0], a) != at(shared[1], a) || (both_ends && at(shared[0], b) != at(shared[1], b))
    }

    fn is_border(&self, a: Index, b: Index) -> bool {
        self.point_tris[a as usize].iter().filter(|&&t| self.slot(t, b) >= 0).count() == 1
    }

    fn best_collapse(&self, u: Index) -> (f64, Index) {
        if self.locked[u as usize] != 0 || self.point_tris[u as usize].is_empty() {
            return (0.0, INVALID);
        }
        let nb = self.neighbours(u);
        let mut features = 0;
        let mut borders = 0;
        let mut ends = [INVALID; 2];
        for &o in &nb {
            if self.is_feature(u, o, false) {
                if features < 2 {
                    ends[features] = o;
                }
                features += 1;
                borders += self.is_border(u, o) as i32;
            }
        }
        if (features != 0 && features != 2) || (borders != 0 && self.keep_borders) || borders == 1 {
            return (0.0, INVALID);
        }
        // Where a feature line turns sharply the point is a corner of it, and stays.
        if features == 2 {
            let a = self.pos(u) - self.pos(ends[0]);
            let b = self.pos(ends[1]) - self.pos(u);
            if a.dot(b) < 45f64.to_radians().cos() * a.length() * b.length() {
                return (0.0, INVALID);
            }
        }
        let mut cand: Vec<(f64, Index)> = nb
            .iter()
            .filter(|&&v| features != 2 || v == ends[0] || v == ends[1])
            .map(|&v| {
                let pv = self.pos(v);
                (self.q[u as usize].eval(pv) + self.q[v as usize].eval(pv) + 1e-9 * self.pos(u).distance(pv), v)
            })
            .collect();
        cand.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        for (cost, v) in cand {
            if self.can_collapse(u, v) {
                return (cost, v);
            }
        }
        (0.0, INVALID)
    }

    /// The v wedge each u wedge becomes: the one the triangles across edge uv use on the same side.
    fn wedge_map(&self, u: Index, v: Index) -> Option<Vec<(Index, Index)>> {
        let mut map: Vec<(Index, Index)> = Vec::new();
        for &t in &self.point_tris[u as usize] {
            let sv = self.slot(t, v);
            if sv < 0 {
                continue;
            }
            let wu = self.tris[t as usize][self.slot(t, u) as usize];
            let wv = self.tris[t as usize][sv as usize];
            match map.iter().find(|e| e.0 == wu) {
                None => map.push((wu, wv)),
                Some(e) if e.1 != wv => return None,
                _ => {}
            }
        }
        for &t in &self.point_tris[u as usize] {
            let wu = self.tris[t as usize][self.slot(t, u) as usize];
            if !map.iter().any(|e| e.0 == wu) {
                return None;
            }
        }
        (!map.is_empty()).then_some(map)
    }

    fn can_collapse(&self, u: Index, v: Index) -> bool {
        let Some(map) = self.wedge_map(u, v) else { return false };
        // Link condition: the only points u and v share are the far corners of the triangles on edge uv.
        let mut across = Vec::new();
        for &t in &self.point_tris[u as usize] {
            if self.slot(t, v) >= 0 {
                for &x in &self.tris[t as usize] {
                    let q = self.w.point[x as usize];
                    if q != u && q != v {
                        across.push(q);
                    }
                }
            }
        }
        let nu = self.neighbours(u);
        let nv = self.neighbours(v);
        for &q in &nu {
            if q != v && nv.contains(&q) && !across.contains(&q) {
                return false;
            }
        }
        if across.is_empty() {
            return false;
        }
        for &t in &self.point_tris[u as usize] {
            if self.slot(t, v) >= 0 {
                continue;
            }
            let orig = self.tris[t as usize];
            let mut moved = orig;
            let su = self.slot(t, u) as usize;
            moved[su] = map.iter().find(|e| e.0 == moved[su]).unwrap().1;
            let before = self.tri_normal(&orig);
            let after = self.tri_normal(&moved);
            let la = after.length();
            if la <= self.min_area
                || after.dot(before) <= 0.2 * la * before.length()
                || after.dot(self.start_normal[t as usize]) <= 0.0
            {
                return false;
            }
            // Against the source's smooth normals too: a triangle that started out backwards (a folded quad) mustn't
            // spread.
            let around = self.smooth[self.w.point[moved[0] as usize] as usize]
                + self.smooth[self.w.point[moved[1] as usize] as usize]
                + self.smooth[self.w.point[moved[2] as usize] as usize];
            if after.dot(around) < -0.3 * la * around.length() {
                return false;
            }
            let uv_before = self.uv_area(&orig);
            let uv_after = self.uv_area(&moved);
            if uv_before != 0.0 && (uv_after == 0.0 || (uv_after > 0.0) != (uv_before > 0.0)) {
                return false;
            }
        }
        true
    }

    fn collapse(&mut self, u: Index, v: Index) {
        let map = self.wedge_map(u, v).unwrap_or_default();
        let tris = std::mem::take(&mut self.point_tris[u as usize]);
        for &t in &tris {
            if self.slot(t, v) >= 0 {
                self.alive[t as usize] = 0;
                self.alive_count -= 1;
                for x in self.tris[t as usize] {
                    let q = self.w.point[x as usize];
                    if q != u {
                        self.point_tris[q as usize].retain(|&y| y != t);
                    }
                }
                continue;
            }
            let su = self.slot(t, u) as usize;
            let cur = self.tris[t as usize][su];
            self.tris[t as usize][su] = map.iter().find(|e| e.0 == cur).unwrap().1;
            self.point_tris[v as usize].push(t);
        }
        let qu = self.q[u as usize];
        self.q[v as usize].add(&qu);
    }
}

fn minmax(a: Index, b: Index) -> (Index, Index) {
    if b < a {
        (b, a)
    } else {
        (a, b)
    }
}

/// Seam flags as point pairs, which survive the faces being rebuilt.
fn seam_pairs(m: &Mesh) -> BTreeSet<(Index, Index)> {
    let seam = m.edge_attrs().get::<bool>(names::SEAM);
    (0..seam.len() as Index)
        .filter(|&e| m.edge_valid(e) && seam[e as usize])
        .map(|e| minmax(m.edge_point(e, 0), m.edge_point(e, 1)))
        .collect()
}

/// Marks edges where the two faces' corners differ: hard where the smoothing fans part, seam where the uvs do.
fn mark_splits(m: &mut Mesh, fan_of_corner: &[Index], hard_from_fans: bool, had_seams: bool) {
    let n = m.edge_capacity() as usize;
    let mut hard_out: Vec<Option<bool>> = vec![None; n];
    let mut seam_out: Vec<bool> = vec![false; n];
    {
        let uvs: Vec<&[[f32; 2]]> = (0..8)
            .map(|i| m.corner_attrs().get::<[f32; 2]>(&names::uv(i)))
            .filter(|uv| !uv.is_empty())
            .collect();
        let old_seam = m.edge_attrs().get::<bool>(names::SEAM);
        for e in 0..m.edge_capacity() {
            if had_seams && (e as usize) < old_seam.len() {
                seam_out[e as usize] = old_seam[e as usize];
            }
            if !m.edge_valid(e) || !m.is_manifold_edge(e) {
                continue;
            }
            let c = m.edge_corner(e);
            let o = m.corner_radial(c);
            let cn = m.corner_next(c);
            let o_at_c = if m.corner_point(o) == m.corner_point(c) { o } else { m.corner_next(o) };
            let o_at_n = if o_at_c == o { m.corner_next(o) } else { o };
            if hard_from_fans {
                hard_out[e as usize] = Some(
                    fan_of_corner[c as usize] != fan_of_corner[o_at_c as usize]
                        && fan_of_corner[cn as usize] != fan_of_corner[o_at_n as usize],
                );
            }
            if had_seams {
                for uv in &uvs {
                    let s = &mut seam_out[e as usize];
                    *s = *s || uv[c as usize] != uv[o_at_c as usize] || uv[cn as usize] != uv[o_at_n as usize];
                }
            }
        }
    }
    if hard_from_fans {
        let hard = m.edge_attrs_mut().get_or_add::<bool>(names::HARD);
        for (e, h) in hard_out.iter().enumerate() {
            if let Some(h) = h {
                hard[e] = *h;
            }
        }
    }
    if had_seams {
        let seam = m.edge_attrs_mut().get_or_add::<bool>(names::SEAM);
        seam.copy_from_slice(&seam_out[..seam.len()]);
    }
}

/// True when no face has points strictly on both sides of the plane.
fn splits_cleanly(m: &Mesh, axis: usize, tol: f64) -> bool {
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let (mut neg, mut pos) = (false, false);
        m.for_each_face_corner(f, |c| {
            let x = m.position(m.corner_point(c))[axis];
            neg |= x < -tol;
            pos |= x > tol;
        });
        if neg && pos {
            return false;
        }
    }
    true
}

fn join_quads(m: &mut Mesh) {
    // Quads never join across a uv split, so the seam flags must cover every one while they run.
    let seam_layer = m.edge_attrs().has(names::SEAM);
    let user_seams = seam_pairs(m);
    for i in 0..8 {
        if m.corner_attrs().has(&names::uv(i)) {
            mark_uv_seams(m, &names::uv(i));
        }
    }
    let qo = QuadrangulateOptions { max_face_angle: 30.0, ..Default::default() };
    let faces = live_faces(m);
    quadrangulate_faces(m, &faces, &qo);
    if !seam_layer {
        m.edge_attrs_mut().remove(names::SEAM);
    } else {
        let flags: Vec<bool> = (0..m.edge_capacity())
            .map(|e| m.edge_valid(e) && user_seams.contains(&minmax(m.edge_point(e, 0), m.edge_point(e, 1))))
            .collect();
        let seam = m.edge_attrs_mut().get_or_add::<bool>(names::SEAM);
        for (e, f) in flags.into_iter().enumerate() {
            seam[e] = f;
        }
    }
    m.compact(None);
}

/// Quadric edge-collapse simplification. Surviving points stay where they were with all their attributes, and every
/// corner keeps its own UVs, normals and colours, so textures don't swim. No collapse may pinch an edge, turn a
/// triangle over (in space or in UV) or leave a sliver. The result is triangles unless quads is set; a mesh that runs
/// out of legal collapses stops above the target.
pub fn reduce_mesh(mesh: &Mesh, opts: &ReduceOptions) -> ReduceResult {
    let mut r = ReduceResult::default();
    let m = &mut r.mesh;
    *m = mesh.clone();
    m.compact(None);
    let tol = opts.symmetry_tolerance.max(0.0);
    let mut symmetric = (0..3).contains(&opts.symmetry_axis);
    let axis = opts.symmetry_axis.clamp(0, 2) as usize;
    // The halves are found and mirrored about the origin, so the plane is moved there for the run.
    let shift = if symmetric { opts.symmetry_offset } else { 0.0 };
    let shift_by = |m: &mut Mesh, d: f64| {
        if d != 0.0 {
            for p in m.positions_mut() {
                p[axis] += d;
            }
        }
    };
    shift_by(m, -shift);
    if symmetric && !splits_cleanly(m, axis, tol) {
        symmetric = false;
        r.symmetry_ignored = true;
    }
    if symmetric {
        let negative: Vec<Index> = (0..m.face_capacity())
            .filter(|&f| {
                let mut neg = false;
                m.for_each_face_corner(f, |c| neg |= m.position(m.corner_point(c))[axis] < -tol);
                neg
            })
            .collect();
        delete_faces(m, &negative, None);
        m.compact(None);
    }

    let source_tris: Index = live_faces(m).iter().map(|&f| m.face_size(f) - 2).sum();
    let faces = live_faces(m);
    triangulate_faces(m, &faces, &TriangulateOptions::default());
    m.compact(None);
    let had_seams = m.edge_attrs().has(names::SEAM);
    let seams_before = seam_pairs(m);

    let w = build_wedges(m, opts.keep_hard_edges);
    let locked: Vec<u8> = (0..m.point_capacity())
        .map(|p| (symmetric && m.point_valid(p) && m.position(p)[axis].abs() <= tol) as u8)
        .collect();

    let wanted = if opts.target_triangles > 0 {
        opts.target_triangles as f64 * if symmetric { 0.5 } else { 1.0 }
    } else {
        source_tris as f64 * opts.ratio.clamp(0.0, 1.0)
    };
    let (tris, max_cost) = {
        let uv = m.corner_attrs().get::<[f32; 2]>(&names::uv(0));
        let mut col = Collapser::new(m, &w, uv, &locked, opts.keep_borders);
        col.run(wanted.round().max(1.0) as Index);
        (col.triangles(), col.max_cost)
    };
    let size = m.bounds().size().length().max(1e-300);
    r.error = max_cost.max(0.0).sqrt() / size;

    for f in live_faces(m) {
        m.remove_face(f);
    }
    let mut fan_of_corner = vec![INVALID; m.corner_capacity() as usize + tris.len() * 3];
    for t in &tris {
        let pts = [w.point[t[0] as usize], w.point[t[1] as usize], w.point[t[2] as usize]];
        let src = [w.corner[t[0] as usize], w.corner[t[1] as usize], w.corner[t[2] as usize]];
        let nf = m.add_face_from(&pts, m.corner_face(src[0]), &src);
        if nf == INVALID {
            continue;
        }
        let mut k = 0;
        m.for_each_face_corner(nf, |c| {
            if c as usize >= fan_of_corner.len() {
                fan_of_corner.resize(c as usize + 1, INVALID);
            }
            fan_of_corner[c as usize] = w.fan[t[k] as usize];
            k += 1;
        });
    }
    fan_of_corner.resize(m.corner_capacity() as usize, INVALID);
    mark_splits(m, &fan_of_corner, opts.keep_hard_edges, had_seams);
    if had_seams {
        let flags: Vec<bool> = (0..m.edge_capacity())
            .map(|e| m.edge_valid(e) && seams_before.contains(&minmax(m.edge_point(e, 0), m.edge_point(e, 1))))
            .collect();
        let seam = m.edge_attrs_mut().get_or_add::<bool>(names::SEAM);
        for (e, f) in flags.into_iter().enumerate() {
            if f {
                seam[e] = true;
            }
        }
    }
    m.remove_all_loose();
    m.compact(None);
    if opts.quads {
        join_quads(m);
    }

    if symmetric {
        let mo = MirrorOptions {
            plane: axis_plane(opts.symmetry_axis, 0.0),
            weld: true,
            tolerance: tol.max(1e-9),
            ..Default::default()
        };
        let first_new = m.point_capacity();
        mirror_mesh(m, &mo);
        clear_symmetry_keys(m, first_new);
        m.compact(None);
    }
    shift_by(m, shift);
    r
}
