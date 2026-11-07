//! Bounding volume hierarchy over a mesh's triangles: binned SAH, built in parallel.

use std::sync::atomic::{AtomicU32, Ordering};

use glam::{DVec3, Vec3};
use rayon::prelude::*;

use crate::attribute::{Index, INVALID};
use crate::mesh::{Bounds, Mesh};
use crate::par::Disjoint;
use crate::triangulate::{triangulate, TriangulateOptions, Triangulation};

const BINS: usize = 16;
const MAX_LEAF: u32 = 8;
const PARALLEL_THRESHOLD: u32 = 16384;
const MAX_SAH_DEPTH: i32 = 40;
const STACK: usize = 96;

#[derive(Clone, Copy, Debug)]
pub struct BvhHit {
    pub t: f64,
    pub triangle: Index,
    pub face: Index,
    /// Weights of the triangle's three corners at the hit.
    pub bary: DVec3,
    pub point: DVec3,
    pub front_facing: bool,
}

impl Default for BvhHit {
    fn default() -> Self {
        Self {
            t: f64::MAX,
            triangle: INVALID,
            face: INVALID,
            bary: DVec3::ZERO,
            point: DVec3::ZERO,
            front_facing: true,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct BvhClosest {
    pub distance: f64,
    pub triangle: Index,
    pub face: Index,
    pub bary: DVec3,
    pub point: DVec3,
}

impl Default for BvhClosest {
    fn default() -> Self {
        Self { distance: f64::MAX, triangle: INVALID, face: INVALID, bary: DVec3::ZERO, point: DVec3::ZERO }
    }
}

/// 32 bytes. Interior nodes (count 0) have children first and first + 1; leaves hold count slots from first.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BvhNode {
    pub min: [f32; 3],
    pub first: u32,
    pub max: [f32; 3],
    pub count: u32,
}

/// A MeshBvh as flat arrays a GPU traversal uploads as they are (std430).
#[derive(Clone, Debug, Default)]
pub struct FlatBvh {
    pub nodes: Vec<BvhNode>,
    /// Three corners per leaf slot, in leaf order. w of the first corner is the slot's triangle index (bit cast),
    /// else 0.
    pub corners: Vec<[f32; 4]>,
}

#[derive(Clone, Copy)]
struct BoxF {
    min: Vec3,
    max: Vec3,
}

impl Default for BoxF {
    fn default() -> Self {
        Self { min: Vec3::splat(f32::MAX), max: Vec3::splat(-f32::MAX) }
    }
}

impl BoxF {
    fn extend_point(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    fn extend(&mut self, b: &BoxF) {
        self.min = self.min.min(b.min);
        self.max = self.max.max(b.max);
    }
    fn area(&self) -> f32 {
        if self.min.x > self.max.x {
            return 0.0;
        }
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }
}

/// Float box that contains the double one.
fn outward(lo: DVec3, hi: DVec3) -> BoxF {
    let mut b = BoxF::default();
    for k in 0..3 {
        b.min[k] = (lo[k] as f32).next_down();
        b.max[k] = (hi[k] as f32).next_up();
    }
    b
}

/// Slab test against a node; returns the entry distance or +inf.
fn hit_box(bmin: &[f32; 3], bmax: &[f32; 3], o: DVec3, inv: DVec3, t_max: f64) -> f64 {
    let (mut t0, mut t1) = (0.0f64, t_max);
    for k in 0..3 {
        let mut a = (bmin[k] as f64 - o[k]) * inv[k];
        let mut b = (bmax[k] as f64 - o[k]) * inv[k];
        if a > b {
            std::mem::swap(&mut a, &mut b);
        }
        // NaN from 0 * inf means the ray runs inside the slab plane; keep the interval.
        if !a.is_nan() {
            t0 = t0.max(a);
        }
        if !b.is_nan() {
            t1 = t1.min(b);
        }
        if t0 > t1 {
            return f64::INFINITY;
        }
    }
    t0
}

fn box_distance2(bmin: &[f32; 3], bmax: &[f32; 3], p: DVec3) -> f64 {
    let mut d2 = 0.0;
    for k in 0..3 {
        let (lo, hi) = (bmin[k] as f64, bmax[k] as f64);
        let v = if p[k] < lo {
            lo - p[k]
        } else if p[k] > hi {
            p[k] - hi
        } else {
            0.0
        };
        d2 += v * v;
    }
    d2
}

/// Moller-Trumbore. Returns (t, u, v, front) with barycentrics u, v for corners 1 and 2.
#[allow(clippy::too_many_arguments)]
fn intersect_tri(
    o: DVec3,
    d: DVec3,
    a: DVec3,
    b: DVec3,
    c: DVec3,
    t_max: f64,
    backfaces: bool,
) -> Option<(f64, f64, f64, bool)> {
    let e1 = b - a;
    let e2 = c - a;
    let p = d.cross(e2);
    let det = e1.dot(p);
    let front = det > 0.0;
    if det == 0.0 || (!backfaces && !front) {
        return None;
    }
    let inv = 1.0 / det;
    let s = o - a;
    let u = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let q = s.cross(e1);
    let v = d.dot(q) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t >= 0.0 && t <= t_max).then_some((t, u, v, front))
}

/// Closest point on triangle abc to p (Ericson), with barycentrics.
pub fn closest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> (DVec3, DVec3) {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (a, DVec3::new(1.0, 0.0, 0.0));
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (b, DVec3::new(0.0, 1.0, 0.0));
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (a + v * ab, DVec3::new(1.0 - v, v, 0.0));
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (c, DVec3::new(0.0, 0.0, 1.0));
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (a + w * ac, DVec3::new(1.0 - w, 0.0, w));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (b + w * (c - b), DVec3::new(0.0, 1.0 - w, w));
    }
    let denom = 1.0 / (va + vb + vc);
    let v = vb * denom;
    let w = vc * denom;
    (a + ab * v + ac * w, DVec3::new(1.0 - v - w, v, w))
}

fn set_node(nd: &mut BvhNode, b: &BoxF, first: u32, count: u32) {
    nd.min = b.min.to_array();
    nd.max = b.max.to_array();
    nd.first = first;
    nd.count = count;
}

/// Holds a copy of the positions, so rebuild (or refit) after the mesh changes.
#[derive(Clone, Debug, Default)]
pub struct MeshBvh {
    tris: Triangulation,
    tri_points: Vec<Index>,
    positions: Vec<DVec3>,
    nodes: Vec<BvhNode>,
    order: Vec<Index>,
    // Built on the first refit_points call.
    parent: Vec<u32>,
    tri_leaf: Vec<u32>,
    point_tri_offsets: Vec<u32>,
    point_tris: Vec<Index>,
    node_dirty: Vec<u8>,
}

struct BuildCtx<'a> {
    tri_box: &'a [BoxF],
    centroid: &'a [Vec3],
    nodes: Disjoint<'a, BvhNode>,
    node_count: AtomicU32,
}

impl BuildCtx<'_> {
    fn write(&self, id: u32, b: &BoxF, first: u32, count: u32) {
        let mut nd = BvhNode::default();
        set_node(&mut nd, b, first, count);
        // SAFETY: each node id is handed out once by node_count and written once.
        unsafe { self.nodes.write(id as usize, nd) };
    }

    /// order is the slice [begin, begin + order.len()) of the full leaf order.
    fn recurse(&self, id: u32, order: &mut [u32], begin: u32, depth: i32) {
        let mut bounds = BoxF::default();
        let mut cbounds = BoxF::default();
        for &t in order.iter() {
            bounds.extend(&self.tri_box[t as usize]);
            cbounds.extend_point(self.centroid[t as usize]);
        }
        let count = order.len() as u32;
        if count <= 2 {
            self.write(id, &bounds, begin, count);
            return;
        }
        let ext = cbounds.max - cbounds.min;
        let axis = if ext.x >= ext.y && ext.x >= ext.z {
            0
        } else if ext.y >= ext.z {
            1
        } else {
            2
        };
        let mut mid = 0usize;
        // Past this depth, split by count so traversal stacks stay small.
        if ext[axis] > 0.0 && depth < MAX_SAH_DEPTH {
            let mut bin_box = [BoxF::default(); BINS];
            let mut bin_count = [0u32; BINS];
            let scale = BINS as f32 / ext[axis];
            let cmin = cbounds.min[axis];
            let centroid = self.centroid;
            let bin_of =
                |t: u32| ((((centroid[t as usize][axis] - cmin) * scale) as i32).min(BINS as i32 - 1)) as usize;
            for &t in order.iter() {
                let b = bin_of(t);
                bin_count[b] += 1;
                bin_box[b].extend(&self.tri_box[t as usize]);
            }
            let mut right_area = [0.0f32; BINS];
            let mut right_count = [0u32; BINS];
            let mut acc = BoxF::default();
            let mut cnt = 0;
            for b in (1..BINS).rev() {
                acc.extend(&bin_box[b]);
                cnt += bin_count[b];
                right_area[b] = acc.area();
                right_count[b] = cnt;
            }
            let mut best_cost = f32::MAX;
            let mut best_split: i32 = -1;
            acc = BoxF::default();
            cnt = 0;
            for b in 1..BINS {
                acc.extend(&bin_box[b - 1]);
                cnt += bin_count[b - 1];
                if cnt == 0 || right_count[b] == 0 {
                    continue;
                }
                let cost = acc.area() * cnt as f32 + right_area[b] * right_count[b] as f32;
                if cost < best_cost {
                    best_cost = cost;
                    best_split = b as i32;
                }
            }
            let leaf_cost = bounds.area() * count as f32;
            if count <= MAX_LEAF && (best_split < 0 || leaf_cost <= best_cost) {
                self.write(id, &bounds, begin, count);
                return;
            }
            if best_split > 0 {
                mid = partition(order, |t| (bin_of(t) as i32) < best_split);
            }
        }
        if mid == 0 || mid == order.len() {
            if count <= MAX_LEAF {
                self.write(id, &bounds, begin, count);
                return;
            }
            // Coincident centroids: split by count.
            mid = order.len() / 2;
            let centroid = self.centroid;
            order.select_nth_unstable_by(mid, |&a, &b| {
                centroid[a as usize][axis].partial_cmp(&centroid[b as usize][axis]).unwrap_or(std::cmp::Ordering::Equal)
            });
        }
        let left = self.node_count.fetch_add(2, Ordering::Relaxed);
        self.write(id, &bounds, left, 0);
        let (lo, hi) = order.split_at_mut(mid);
        let mid_abs = begin + mid as u32;
        if count > PARALLEL_THRESHOLD {
            rayon::join(|| self.recurse(left, lo, begin, depth + 1), || self.recurse(left + 1, hi, mid_abs, depth + 1));
        } else {
            self.recurse(left, lo, begin, depth + 1);
            self.recurse(left + 1, hi, mid_abs, depth + 1);
        }
    }
}

/// std::partition: elements satisfying pred first; returns their count.
fn partition(v: &mut [u32], pred: impl Fn(u32) -> bool) -> usize {
    let mut first = 0;
    for i in 0..v.len() {
        if pred(v[i]) {
            v.swap(first, i);
            first += 1;
        }
    }
    first
}

impl MeshBvh {
    pub fn new(mesh: &Mesh) -> Self {
        let mut b = Self::default();
        b.build(mesh);
        b
    }

    pub fn build(&mut self, mesh: &Mesh) {
        self.build_with(mesh, triangulate(mesh, &TriangulateOptions::default()));
    }

    pub fn build_with(&mut self, mesh: &Mesh, tris: Triangulation) {
        self.tris = tris;
        self.parent.clear();
        self.tri_leaf.clear();
        self.point_tri_offsets.clear();
        self.point_tris.clear();
        self.node_dirty.clear();
        self.positions = mesh.positions().to_vec();
        self.tri_points = self.tris.corners.par_iter().map(|&c| mesh.corner_point(c)).collect();
        self.build_tree();
    }

    fn tri_points(&self, t: Index) -> (DVec3, DVec3, DVec3) {
        let p = &self.tri_points[t as usize * 3..t as usize * 3 + 3];
        (self.positions[p[0] as usize], self.positions[p[1] as usize], self.positions[p[2] as usize])
    }

    fn tri_box(&self, t: Index) -> BoxF {
        let (a, b, c) = self.tri_points(t);
        outward(a.min(b.min(c)), a.max(b.max(c)))
    }

    fn build_tree(&mut self) {
        let n = self.tris.faces.len() as u32;
        self.nodes.clear();
        self.order = (0..n).collect();
        if n == 0 {
            return;
        }
        let tri_box: Vec<BoxF> = (0..n).into_par_iter().with_min_len(4096).map(|t| self.tri_box(t)).collect();
        let centroid: Vec<Vec3> = tri_box.par_iter().map(|b| (b.min + b.max) * 0.5).collect();
        let mut nodes = vec![BvhNode::default(); n as usize * 2];
        let mut order = std::mem::take(&mut self.order);
        let used = {
            let ctx = BuildCtx {
                tri_box: &tri_box,
                centroid: &centroid,
                nodes: Disjoint::new(&mut nodes),
                node_count: AtomicU32::new(1),
            };
            ctx.recurse(0, &mut order, 0, 0);
            ctx.node_count.load(Ordering::Relaxed)
        };
        nodes.truncate(used as usize);
        self.nodes = nodes;
        self.order = order;
    }

    fn refit_node(&mut self, i: usize) {
        let nd = self.nodes[i];
        let mut b = BoxF::default();
        if nd.count != 0 {
            for k in nd.first..nd.first + nd.count {
                b.extend(&self.tri_box(self.order[k as usize]));
            }
        } else {
            for k in nd.first..nd.first + 2 {
                let ch = &self.nodes[k as usize];
                b.extend_point(Vec3::from_array(ch.min));
                b.extend_point(Vec3::from_array(ch.max));
            }
        }
        self.nodes[i].min = b.min.to_array();
        self.nodes[i].max = b.max.to_array();
    }

    /// Updates bounds after points moved, keeping the tree. Positions must have the same capacity.
    pub fn refit(&mut self, mesh: &Mesh) {
        if mesh.point_capacity() as usize != self.positions.len() || self.nodes.is_empty() {
            self.build(mesh);
            return;
        }
        self.positions.copy_from_slice(mesh.positions());
        // Children always come after their parent, so a reverse sweep sees children first.
        for i in (0..self.nodes.len()).rev() {
            self.refit_node(i);
        }
    }

    /// Updates only the given points and the nodes above their triangles, for small edits of a large mesh (sculpt
    /// dabs). Topology must not have changed since build; the first call builds the lookup tables it needs.
    pub fn refit_points(&mut self, mesh: &Mesh, points: &[Index]) {
        if mesh.point_capacity() as usize != self.positions.len() || self.nodes.is_empty() {
            self.build(mesh);
            return;
        }
        if self.parent.len() != self.nodes.len() {
            self.parent = vec![0; self.nodes.len()];
            self.tri_leaf = vec![0; self.tris.faces.len()];
            for (i, nd) in self.nodes.iter().enumerate() {
                if nd.count != 0 {
                    for k in nd.first..nd.first + nd.count {
                        self.tri_leaf[self.order[k as usize] as usize] = i as u32;
                    }
                } else {
                    self.parent[nd.first as usize] = i as u32;
                    self.parent[nd.first as usize + 1] = i as u32;
                }
            }
            self.point_tri_offsets = vec![0; self.positions.len() + 1];
            for &p in &self.tri_points {
                self.point_tri_offsets[p as usize + 1] += 1;
            }
            for i in 1..self.point_tri_offsets.len() {
                self.point_tri_offsets[i] += self.point_tri_offsets[i - 1];
            }
            self.point_tris = vec![0; self.tri_points.len()];
            let mut fill = self.point_tri_offsets[..self.point_tri_offsets.len() - 1].to_vec();
            for (i, &p) in self.tri_points.iter().enumerate() {
                self.point_tris[fill[p as usize] as usize] = (i / 3) as Index;
                fill[p as usize] += 1;
            }
            self.node_dirty = vec![0; self.nodes.len()];
        }

        let mut dirty = Vec::new();
        for &p in points {
            if p as usize >= self.positions.len() {
                continue;
            }
            self.positions[p as usize] = mesh.position(p);
            for k in self.point_tri_offsets[p as usize]..self.point_tri_offsets[p as usize + 1] {
                let mut n = self.tri_leaf[self.point_tris[k as usize] as usize];
                loop {
                    if self.node_dirty[n as usize] != 0 {
                        break;
                    }
                    self.node_dirty[n as usize] = 1;
                    dirty.push(n);
                    if n == 0 {
                        break;
                    }
                    n = self.parent[n as usize];
                }
            }
        }
        // Children have larger ids than their parent, so descending order refits children first.
        dirty.sort_unstable_by(|a, b| b.cmp(a));
        for i in dirty {
            self.refit_node(i as usize);
            self.node_dirty[i as usize] = 0;
        }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::default();
        if let Some(n) = self.nodes.first() {
            b.extend(Vec3::from_array(n.min).as_dvec3());
            b.extend(Vec3::from_array(n.max).as_dvec3());
        }
        b
    }

    pub fn triangle_count(&self) -> Index {
        self.tris.faces.len() as Index
    }

    pub fn triangles(&self) -> &Triangulation {
        &self.tris
    }

    /// Nearest hit along origin + t * dir for t in [0, t_max]. dir need not be unit length.
    pub fn raycast_with(&self, origin: DVec3, dir: DVec3, t_max: f64, backfaces: bool) -> Option<BvhHit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv = 1.0 / dir;
        let mut best = t_max;
        let mut best_tri = INVALID;
        let (mut bu, mut bv) = (0.0, 0.0);
        let mut best_front = true;
        let mut stack = [0u32; STACK];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let nd = &self.nodes[stack[sp] as usize];
            if hit_box(&nd.min, &nd.max, origin, inv, best) == f64::INFINITY {
                continue;
            }
            if nd.count != 0 {
                for k in nd.first..nd.first + nd.count {
                    let t = self.order[k as usize];
                    let (a, b, c) = self.tri_points(t);
                    if let Some((tt, u, v, front)) = intersect_tri(origin, dir, a, b, c, best, backfaces) {
                        best = tt;
                        best_tri = t;
                        bu = u;
                        bv = v;
                        best_front = front;
                    }
                }
                continue;
            }
            let l = &self.nodes[nd.first as usize];
            let r = &self.nodes[nd.first as usize + 1];
            let tl = hit_box(&l.min, &l.max, origin, inv, best);
            let tr = hit_box(&r.min, &r.max, origin, inv, best);
            // Push the far child first so the near one is tested first.
            if tl < tr {
                if tr != f64::INFINITY {
                    stack[sp] = nd.first + 1;
                    sp += 1;
                }
                stack[sp] = nd.first;
                sp += 1;
            } else {
                if tl != f64::INFINITY {
                    stack[sp] = nd.first;
                    sp += 1;
                }
                if tr != f64::INFINITY {
                    stack[sp] = nd.first + 1;
                    sp += 1;
                }
            }
        }
        if best_tri == INVALID {
            return None;
        }
        Some(BvhHit {
            t: best,
            triangle: best_tri,
            face: self.tris.faces[best_tri as usize],
            bary: DVec3::new(1.0 - bu - bv, bu, bv),
            point: origin + dir * best,
            front_facing: best_front,
        })
    }

    pub fn raycast(&self, origin: DVec3, dir: DVec3) -> Option<BvhHit> {
        self.raycast_with(origin, dir, f64::MAX, true)
    }

    /// True if anything lies along the segment; cheaper than raycast.
    pub fn occluded(&self, origin: DVec3, dir: DVec3, t_max: f64) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        let inv = 1.0 / dir;
        let mut stack = [0u32; STACK];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let nd = &self.nodes[stack[sp] as usize];
            if hit_box(&nd.min, &nd.max, origin, inv, t_max) == f64::INFINITY {
                continue;
            }
            if nd.count != 0 {
                for k in nd.first..nd.first + nd.count {
                    let (a, b, c) = self.tri_points(self.order[k as usize]);
                    if intersect_tri(origin, dir, a, b, c, t_max, true).is_some() {
                        return true;
                    }
                }
                continue;
            }
            stack[sp] = nd.first;
            stack[sp + 1] = nd.first + 1;
            sp += 2;
        }
        false
    }

    /// Closest surface point within max_distance.
    pub fn closest_point_within(&self, p: DVec3, max_distance: f64) -> Option<BvhClosest> {
        if self.nodes.is_empty() {
            return None;
        }
        let mut best2 = if max_distance == f64::MAX { max_distance } else { max_distance * max_distance };
        let mut best_tri = INVALID;
        let mut best_point = DVec3::ZERO;
        let mut best_bary = DVec3::ZERO;
        let mut stack = [0u32; STACK];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let nd = &self.nodes[stack[sp] as usize];
            if box_distance2(&nd.min, &nd.max, p) > best2 {
                continue;
            }
            if nd.count != 0 {
                for k in nd.first..nd.first + nd.count {
                    let t = self.order[k as usize];
                    let (a, b, c) = self.tri_points(t);
                    let (q, bary) = closest_on_triangle(p, a, b, c);
                    let d2 = (q - p).length_squared();
                    if d2 < best2 {
                        best2 = d2;
                        best_tri = t;
                        best_point = q;
                        best_bary = bary;
                    }
                }
                continue;
            }
            let l = &self.nodes[nd.first as usize];
            let r = &self.nodes[nd.first as usize + 1];
            let dl = box_distance2(&l.min, &l.max, p);
            let dr = box_distance2(&r.min, &r.max, p);
            if dl < dr {
                stack[sp] = nd.first + 1;
                stack[sp + 1] = nd.first;
            } else {
                stack[sp] = nd.first;
                stack[sp + 1] = nd.first + 1;
            }
            sp += 2;
        }
        if best_tri == INVALID {
            return None;
        }
        Some(BvhClosest {
            distance: best2.sqrt(),
            triangle: best_tri,
            face: self.tris.faces[best_tri as usize],
            point: best_point,
            bary: best_bary,
        })
    }

    pub fn closest_point(&self, p: DVec3) -> Option<BvhClosest> {
        self.closest_point_within(p, f64::MAX)
    }

    /// Triangles whose bounds overlap the box, sorted.
    pub fn overlapping(&self, bx: &Bounds) -> Vec<Index> {
        let mut out = Vec::new();
        if self.nodes.is_empty() {
            return out;
        }
        let mut stack = [0u32; STACK];
        let mut sp = 1;
        while sp > 0 {
            sp -= 1;
            let nd = &self.nodes[stack[sp] as usize];
            let overlap = (0..3).all(|k| nd.min[k] as f64 <= bx.max[k] && nd.max[k] as f64 >= bx.min[k]);
            if !overlap {
                continue;
            }
            if nd.count != 0 {
                out.extend_from_slice(&self.order[nd.first as usize..(nd.first + nd.count) as usize]);
                continue;
            }
            stack[sp] = nd.first;
            stack[sp + 1] = nd.first + 1;
            sp += 2;
        }
        out.sort_unstable();
        out
    }

    /// The tree and its triangles in leaf order, positions as floats. Triangle indices are this BVH's.
    pub fn export_flat(&self) -> FlatBvh {
        let corners = self
            .order
            .par_iter()
            .flat_map_iter(|&tri| {
                let (a, b, c) = self.tri_points(tri);
                let w = f32::from_bits(tri);
                [a, b, c].into_iter().enumerate().map(move |(k, p)| {
                    let p = p.as_vec3();
                    [p.x, p.y, p.z, if k == 0 { w } else { 0.0 }]
                })
            })
            .collect();
        FlatBvh { nodes: self.nodes.clone(), corners }
    }

    /// Blend of a corner attribute at a hit, e.g. the uv under the cursor.
    pub fn interpolate<const N: usize>(&self, corner_values: &[[f32; N]], triangle: Index, bary: DVec3) -> [f32; N] {
        let c = &self.tris.corners[triangle as usize * 3..triangle as usize * 3 + 3];
        let w = [bary.x as f32, bary.y as f32, bary.z as f32];
        std::array::from_fn(|k| {
            corner_values[c[0] as usize][k] * w[0] + corner_values[c[1] as usize][k] * w[1]
                + corner_values[c[2] as usize][k] * w[2]
        })
    }
}
