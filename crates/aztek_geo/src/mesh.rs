//! Polygon mesh with BMesh-style topology.

use std::sync::atomic::{AtomicU64, Ordering};

use glam::DVec3;
use rayon::prelude::*;

use crate::attribute::{names, AttributeSet, Domain, Index, INVALID};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: DVec3,
    pub max: DVec3,
}

impl Default for Bounds {
    fn default() -> Self {
        Self {
            min: DVec3::splat(f64::MAX),
            max: DVec3::splat(-f64::MAX),
        }
    }
}

impl Bounds {
    pub fn valid(&self) -> bool {
        self.min.x <= self.max.x
    }
    pub fn center(&self) -> DVec3 {
        (self.min + self.max) * 0.5
    }
    pub fn size(&self) -> DVec3 {
        if self.valid() {
            self.max - self.min
        } else {
            DVec3::ZERO
        }
    }
    pub fn extend(&mut self, p: DVec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn extend_bounds(&mut self, b: &Bounds) {
        self.min = self.min.min(b.min);
        self.max = self.max.max(b.max);
    }
}

/// Color set state that is not per element: the set displayed and painted, and the sets kept at half precision.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorSetInfo {
    pub current: String,
    pub half: Vec<String>,
}

/// Old-to-new id maps produced by `Mesh::compact`. INVALID marks removed elements.
#[derive(Debug, Clone, Default)]
pub struct MeshRemap {
    pub points: Vec<Index>,
    pub edges: Vec<Index>,
    pub faces: Vec<Index>,
    pub corners: Vec<Index>,
}

pub mod validate {
    /// Broken links; always checked.
    pub const STRUCTURE: u32 = 1 << 0;
    /// Edges with more than two faces.
    pub const MANIFOLD: u32 = 1 << 1;
    /// Neighbouring faces with opposite orientation.
    pub const WINDING: u32 = 1 << 2;
    /// Edges without faces, points without edges.
    pub const LOOSE: u32 = 1 << 3;
    /// Zero-area faces, zero-length edges.
    pub const DEGENERATE: u32 = 1 << 4;
    pub const DEFAULT: u32 = STRUCTURE | MANIFOLD | WINDING | LOOSE;
    pub const ALL: u32 = u32::MAX;
}

#[derive(Debug, Clone, Copy)]
struct Edge {
    v: [Index; 2],
    next: [Index; 2],
    prev: [Index; 2],
    corner: Index,
}

const DEAD_EDGE: Edge = Edge {
    v: [INVALID; 2],
    next: [INVALID; 2],
    prev: [INVALID; 2],
    corner: INVALID,
};

#[derive(Debug, Clone, Copy)]
struct Corner {
    point: Index,
    edge: Index,
    face: Index,
    next: Index,
    prev: Index,
    radial_next: Index,
    radial_prev: Index,
}

const DEAD_CORNER: Corner = Corner {
    point: INVALID,
    edge: INVALID,
    face: INVALID,
    next: INVALID,
    prev: INVALID,
    radial_next: INVALID,
    radial_prev: INVALID,
};

#[derive(Debug, Clone, Copy)]
struct Face {
    corner: Index,
    size: Index,
}

fn has_repeats(pts: &[Index]) -> bool {
    let n = pts.len();
    if n <= 16 {
        for i in 0..n {
            for j in i + 1..n {
                if pts[i] == pts[j] {
                    return true;
                }
            }
        }
        return false;
    }
    let mut sorted = pts.to_vec();
    sorted.sort_unstable();
    sorted.windows(2).any(|w| w[0] == w[1])
}

fn new_topology_id() -> u64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    NEXT.fetch_add(1, Ordering::Relaxed)
}

/// Exclusive prefix sum of v in place; returns the total.
fn exclusive_scan(v: &mut [Index]) -> Index {
    let mut sum: Index = 0;
    for x in v.iter_mut() {
        let t = *x;
        *x = sum;
        sum += t;
    }
    sum
}

/// item(i) for i in 0..n, grouped by key(i) (INVALID drops it): group k is `[offsets[k], offsets[k + 1])`, sorted.
fn bucket_by<T: Ord + Copy + Send + Default>(
    n: usize,
    keys: usize,
    key: impl Fn(usize) -> Index,
    item: impl Fn(usize) -> T,
    offsets: &mut Vec<Index>,
) -> Vec<T> {
    let mut counts = vec![0 as Index; keys + 1];
    for i in 0..n {
        let k = key(i);
        if k != INVALID {
            counts[k as usize] += 1;
        }
    }
    *offsets = counts;
    exclusive_scan(offsets);
    let mut at = offsets.clone();
    let mut items = vec![T::default(); offsets[keys] as usize];
    for i in 0..n {
        let k = key(i);
        if k != INVALID {
            items[at[k as usize] as usize] = item(i);
            at[k as usize] += 1;
        }
    }
    let mut groups: Vec<&mut [T]> = Vec::with_capacity(keys);
    let mut rest = items.as_mut_slice();
    for k in 0..keys {
        let (g, r) = rest.split_at_mut((offsets[k + 1] - offsets[k]) as usize);
        groups.push(g);
        rest = r;
    }
    groups.par_iter_mut().for_each(|g| g.sort_unstable());
    items
}

/// Polygon mesh with BMesh-style topology: every point has a disk cycle of edges, every edge a radial cycle of face
/// corners, every face a cyclic list of corners. Non-manifold edges and points are representable. Corner c of a face
/// runs from `corner_point(c)` to `corner_point(corner_next(c))` along `corner_edge(c)`.
///
/// Removing elements only marks them deleted, so ids of everything else stay stable until `compact`.
#[derive(Debug, Clone)]
pub struct Mesh {
    point_pos: Shared<Vec<DVec3>>,
    point_edge: Shared<Vec<Index>>,
    point_dead: Shared<Vec<u8>>,
    edges: Shared<Vec<Edge>>,
    corners: Shared<Vec<Corner>>,
    faces: Shared<Vec<Face>>,
    num_points: Index,
    num_edges: Index,
    num_faces: Index,
    num_corners: Index,
    last_split_edge: Index,
    topology_id: u64,
    attrs: Shared<[AttributeSet; 4]>,
    material_names: Shared<Vec<String>>,
    color_set_info: Shared<ColorSetInfo>,
}

/// Data a copy of a mesh shares until one of them writes to it. A deformer's result differs from its input only in where
/// the points are, so copying a mesh costs next to nothing and each layer of a deformer stack doesn't copy the topology.
#[derive(Debug, Default)]
struct Shared<T>(std::sync::Arc<T>);

impl<T> Clone for Shared<T> {
    fn clone(&self) -> Self {
        Shared(self.0.clone())
    }
}

impl<T> std::ops::Deref for Shared<T> {
    type Target = T;
    #[inline]
    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: Clone> std::ops::DerefMut for Shared<T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        std::sync::Arc::make_mut(&mut self.0)
    }
}

impl<T> From<T> for Shared<T> {
    fn from(v: T) -> Self {
        Shared(std::sync::Arc::new(v))
    }
}

impl Default for Mesh {
    fn default() -> Self {
        Self {
            point_pos: Default::default(),
            point_edge: Default::default(),
            point_dead: Default::default(),
            edges: Default::default(),
            corners: Default::default(),
            faces: Default::default(),
            num_points: 0,
            num_edges: 0,
            num_faces: 0,
            num_corners: 0,
            last_split_edge: INVALID,
            topology_id: new_topology_id(),
            attrs: Default::default(),
            material_names: Default::default(),
            color_set_info: Default::default(),
        }
    }
}

impl Mesh {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn point_capacity(&self) -> Index {
        self.point_pos.len() as Index
    }
    pub fn edge_capacity(&self) -> Index {
        self.edges.len() as Index
    }
    pub fn face_capacity(&self) -> Index {
        self.faces.len() as Index
    }
    pub fn corner_capacity(&self) -> Index {
        self.corners.len() as Index
    }
    pub fn num_points(&self) -> Index {
        self.num_points
    }
    pub fn num_edges(&self) -> Index {
        self.num_edges
    }
    pub fn num_faces(&self) -> Index {
        self.num_faces
    }
    pub fn num_corners(&self) -> Index {
        self.num_corners
    }
    pub fn has_deleted(&self) -> bool {
        self.num_points != self.point_capacity()
            || self.num_edges != self.edge_capacity()
            || self.num_faces != self.face_capacity()
            || self.num_corners != self.corner_capacity()
    }
    /// Same for every copy of this mesh until one of them changes its points, edges, faces or corners (moving points
    /// keeps it), so data derived from topology alone can be reused across copies. Unique per process.
    pub fn topology_id(&self) -> u64 {
        self.topology_id
    }
    fn touch_topology(&mut self) {
        self.topology_id = new_topology_id();
    }
    /// Approximate heap bytes held: element arrays and attribute layers.
    pub fn memory_bytes(&self) -> usize {
        let mut n = std::mem::size_of::<Mesh>()
            + self.point_pos.capacity() * std::mem::size_of::<DVec3>()
            + self.point_edge.capacity() * 4
            + self.point_dead.capacity()
            + self.edges.capacity() * std::mem::size_of::<Edge>()
            + self.corners.capacity() * std::mem::size_of::<Corner>()
            + self.faces.capacity() * std::mem::size_of::<Face>();
        for set in self.attrs.iter() {
            for l in set.layers() {
                n += l.len() * l.stride();
            }
        }
        n + self
            .material_names
            .iter()
            .map(String::capacity)
            .sum::<usize>()
    }
    pub fn is_empty(&self) -> bool {
        self.num_points == 0
    }

    pub fn point_valid(&self, p: Index) -> bool {
        (p as usize) < self.point_dead.len() && self.point_dead[p as usize] == 0
    }
    pub fn edge_valid(&self, e: Index) -> bool {
        (e as usize) < self.edges.len() && self.edges[e as usize].v[0] != INVALID
    }
    pub fn face_valid(&self, f: Index) -> bool {
        (f as usize) < self.faces.len() && self.faces[f as usize].corner != INVALID
    }
    pub fn corner_valid(&self, c: Index) -> bool {
        (c as usize) < self.corners.len() && self.corners[c as usize].face != INVALID
    }

    // Points
    pub fn position(&self, p: Index) -> DVec3 {
        self.point_pos[p as usize]
    }
    pub fn set_position(&mut self, p: Index, pos: DVec3) {
        self.point_pos[p as usize] = pos;
    }
    pub fn positions(&self) -> &[DVec3] {
        &self.point_pos
    }
    pub fn positions_mut(&mut self) -> &mut [DVec3] {
        &mut self.point_pos
    }
    /// Whether the two differ at most in where their points are: same topology, and attributes that are the very same data.
    pub fn only_points_differ(&self, other: &Mesh) -> bool {
        self.topology_id == other.topology_id && std::sync::Arc::ptr_eq(&self.attrs.0, &other.attrs.0) && std::sync::Arc::ptr_eq(&self.corners.0, &other.corners.0)
    }

    /// This mesh with its points somewhere else, sharing everything else. `positions` holds one per point capacity.
    pub fn with_positions(&self, positions: Vec<DVec3>) -> Mesh {
        debug_assert_eq!(positions.len(), self.point_pos.len());
        let mut m = self.clone();
        m.point_pos = positions.into();
        m
    }
    /// Any edge of the point's disk cycle, INVALID for a loose point.
    pub fn point_edge(&self, p: Index) -> Index {
        self.point_edge[p as usize]
    }

    // Edges
    pub fn edge_point(&self, e: Index, i: usize) -> Index {
        self.edges[e as usize].v[i]
    }
    pub fn edge_other_point(&self, e: Index, p: Index) -> Index {
        let ed = &self.edges[e as usize];
        if ed.v[0] == p {
            ed.v[1]
        } else {
            ed.v[0]
        }
    }
    /// Any corner of the edge's radial cycle, INVALID for a loose edge.
    pub fn edge_corner(&self, e: Index) -> Index {
        self.edges[e as usize].corner
    }
    /// Next edge in the disk cycle around p, which must be an endpoint of e.
    pub fn edge_next_around(&self, e: Index, p: Index) -> Index {
        self.edges[e as usize].next[self.side(e, p)]
    }

    // Corners
    pub fn corner_point(&self, c: Index) -> Index {
        self.corners[c as usize].point
    }
    pub fn corner_edge(&self, c: Index) -> Index {
        self.corners[c as usize].edge
    }
    pub fn corner_face(&self, c: Index) -> Index {
        self.corners[c as usize].face
    }
    pub fn corner_next(&self, c: Index) -> Index {
        self.corners[c as usize].next
    }
    pub fn corner_prev(&self, c: Index) -> Index {
        self.corners[c as usize].prev
    }
    /// Next corner on the same edge, in another face (or itself on a boundary edge).
    pub fn corner_radial(&self, c: Index) -> Index {
        self.corners[c as usize].radial_next
    }
    pub fn corner_next_point(&self, c: Index) -> Index {
        self.corners[self.corners[c as usize].next as usize].point
    }

    // Faces
    pub fn face_corner(&self, f: Index) -> Index {
        self.faces[f as usize].corner
    }
    pub fn face_size(&self, f: Index) -> Index {
        self.faces[f as usize].size
    }

    fn side(&self, e: Index, p: Index) -> usize {
        if self.edges[e as usize].v[0] == p {
            0
        } else {
            1
        }
    }

    pub fn for_each_face_corner(&self, f: Index, mut fun: impl FnMut(Index)) {
        let first = self.faces[f as usize].corner;
        let mut c = first;
        loop {
            fun(c);
            c = self.corners[c as usize].next;
            if c == first {
                break;
            }
        }
    }

    pub fn for_each_point_edge(&self, p: Index, mut fun: impl FnMut(Index)) {
        let first = self.point_edge[p as usize];
        if first == INVALID {
            return;
        }
        let mut e = first;
        loop {
            let next = self.edges[e as usize].next[self.side(e, p)];
            fun(e);
            e = next;
            if e == first {
                break;
            }
        }
    }

    pub fn for_each_edge_corner(&self, e: Index, mut fun: impl FnMut(Index)) {
        let first = self.edges[e as usize].corner;
        if first == INVALID {
            return;
        }
        let mut c = first;
        loop {
            let next = self.corners[c as usize].radial_next;
            fun(c);
            c = next;
            if c == first {
                break;
            }
        }
    }

    /// Every corner whose point is p, each once.
    pub fn for_each_point_corner(&self, p: Index, mut fun: impl FnMut(Index)) {
        self.for_each_point_edge(p, |e| {
            self.for_each_edge_corner(e, |c| {
                if self.corners[c as usize].point == p {
                    fun(c);
                }
            })
        });
    }

    // Topology primitives

    fn disk_insert(&mut self, e: Index, p: Index) {
        let s = self.side(e, p);
        let first = self.point_edge[p as usize];
        if first == INVALID {
            let ed = &mut self.edges[e as usize];
            ed.next[s] = e;
            ed.prev[s] = e;
            self.point_edge[p as usize] = e;
            return;
        }
        let fs = self.side(first, p);
        let after = self.edges[first as usize].next[fs];
        self.edges[e as usize].prev[s] = first;
        self.edges[e as usize].next[s] = after;
        self.edges[first as usize].next[fs] = e;
        let as_ = self.side(after, p);
        self.edges[after as usize].prev[as_] = e;
    }

    fn disk_remove(&mut self, e: Index, p: Index) {
        let s = self.side(e, p);
        let next = self.edges[e as usize].next[s];
        let prev = self.edges[e as usize].prev[s];
        if next == e {
            self.point_edge[p as usize] = INVALID;
        } else {
            let ns = self.side(next, p);
            self.edges[next as usize].prev[ns] = prev;
            let ps = self.side(prev, p);
            self.edges[prev as usize].next[ps] = next;
            if self.point_edge[p as usize] == e {
                self.point_edge[p as usize] = next;
            }
        }
        self.edges[e as usize].next[s] = INVALID;
        self.edges[e as usize].prev[s] = INVALID;
    }

    fn radial_insert(&mut self, c: Index, e: Index) {
        self.corners[c as usize].edge = e;
        let first = self.edges[e as usize].corner;
        if first == INVALID {
            self.corners[c as usize].radial_next = c;
            self.corners[c as usize].radial_prev = c;
            self.edges[e as usize].corner = c;
            return;
        }
        let after = self.corners[first as usize].radial_next;
        self.corners[c as usize].radial_prev = first;
        self.corners[c as usize].radial_next = after;
        self.corners[first as usize].radial_next = c;
        self.corners[after as usize].radial_prev = c;
    }

    fn radial_remove(&mut self, c: Index) {
        let cr = self.corners[c as usize];
        let e = cr.edge;
        if cr.radial_next == c {
            self.edges[e as usize].corner = INVALID;
        } else {
            self.corners[cr.radial_next as usize].radial_prev = cr.radial_prev;
            self.corners[cr.radial_prev as usize].radial_next = cr.radial_next;
            if self.edges[e as usize].corner == c {
                self.edges[e as usize].corner = cr.radial_next;
            }
        }
        self.corners[c as usize].radial_next = INVALID;
        self.corners[c as usize].radial_prev = INVALID;
    }

    fn kill_corner(&mut self, c: Index) {
        self.corners[c as usize] = DEAD_CORNER;
        self.num_corners -= 1;
    }

    fn new_edge(&mut self, a: Index, b: Index) -> Index {
        let e = self.edges.len() as Index;
        self.edges.push(Edge {
            v: [a, b],
            ..DEAD_EDGE
        });
        let n = self.edges.len();
        self.edge_attrs_mut().resize(n);
        self.disk_insert(e, a);
        self.disk_insert(e, b);
        self.num_edges += 1;
        e
    }

    fn new_corner(&mut self, point: Index, face: Index) -> Index {
        let c = self.corners.len() as Index;
        self.corners.push(Corner {
            point,
            face,
            ..DEAD_CORNER
        });
        self.num_corners += 1;
        c
    }

    // Queries

    pub fn find_edge(&self, a: Index, b: Index) -> Index {
        let first = self.point_edge[a as usize];
        if first == INVALID || a == b {
            return INVALID;
        }
        let mut e = first;
        loop {
            let ed = &self.edges[e as usize];
            if ed.v[0] == b || ed.v[1] == b {
                return e;
            }
            e = ed.next[if ed.v[0] == a { 0 } else { 1 }];
            if e == first {
                return INVALID;
            }
        }
    }

    /// The corner of face f at point p, INVALID if p is not on f.
    pub fn find_corner(&self, f: Index, p: Index) -> Index {
        let mut found = INVALID;
        self.for_each_face_corner(f, |c| {
            if self.corners[c as usize].point == p {
                found = c;
            }
        });
        found
    }

    pub fn edge_face_count(&self, e: Index) -> Index {
        let mut n = 0;
        self.for_each_edge_corner(e, |_| n += 1);
        n
    }

    pub fn is_boundary_edge(&self, e: Index) -> bool {
        let c = self.edges[e as usize].corner;
        c != INVALID && self.corners[c as usize].radial_next == c
    }

    pub fn is_manifold_edge(&self, e: Index) -> bool {
        let c = self.edges[e as usize].corner;
        if c == INVALID {
            return false;
        }
        let r = self.corners[c as usize].radial_next;
        r != c && self.corners[r as usize].radial_next == c
    }

    pub fn is_boundary_point(&self, p: Index) -> bool {
        let mut boundary = false;
        self.for_each_point_edge(p, |e| boundary |= !self.is_manifold_edge(e));
        boundary
    }

    pub fn point_valence(&self, p: Index) -> Index {
        let mut n = 0;
        self.for_each_point_edge(p, |_| n += 1);
        n
    }

    /// The face across edge e from face f when e has exactly two faces.
    pub fn opposite_face(&self, e: Index, f: Index) -> Index {
        if !self.is_manifold_edge(e) {
            return INVALID;
        }
        let c = self.edges[e as usize].corner;
        let r = self.corners[c as usize].radial_next;
        if self.corners[c as usize].face == f {
            return self.corners[r as usize].face;
        }
        if self.corners[r as usize].face == f {
            return self.corners[c as usize].face;
        }
        INVALID
    }

    pub fn edge_length(&self, e: Index) -> f64 {
        let ed = &self.edges[e as usize];
        (self.point_pos[ed.v[1] as usize] - self.point_pos[ed.v[0] as usize]).length()
    }

    pub fn face_centroid(&self, f: Index) -> DVec3 {
        let mut sum = DVec3::ZERO;
        self.for_each_face_corner(f, |c| {
            sum += self.point_pos[self.corners[c as usize].point as usize]
        });
        sum / self.faces[f as usize].size as f64
    }

    /// Newell vector, twice the projected area in length.
    pub fn face_area_vector(&self, f: Index) -> DVec3 {
        let mut n = DVec3::ZERO;
        // Relative to the first point keeps precision for faces far from the origin.
        let origin =
            self.point_pos[self.corners[self.faces[f as usize].corner as usize].point as usize];
        self.for_each_face_corner(f, |c| {
            let cr = &self.corners[c as usize];
            let a = self.point_pos[cr.point as usize] - origin;
            let b = self.point_pos[self.corners[cr.next as usize].point as usize] - origin;
            n += a.cross(b);
        });
        n
    }

    /// Newell normal: unit length, kept stable for concave and non-planar faces.
    pub fn face_normal(&self, f: Index) -> DVec3 {
        let n = self.face_area_vector(f);
        let len = n.length();
        if len > 0.0 {
            n / len
        } else {
            DVec3::Y
        }
    }

    pub fn face_area(&self, f: Index) -> f64 {
        self.face_area_vector(f).length() * 0.5
    }

    pub fn face_points(&self, f: Index) -> Vec<Index> {
        let mut out = Vec::with_capacity(self.faces[f as usize].size as usize);
        self.for_each_face_corner(f, |c| out.push(self.corners[c as usize].point));
        out
    }

    pub fn face_corners(&self, f: Index) -> Vec<Index> {
        let mut out = Vec::with_capacity(self.faces[f as usize].size as usize);
        self.for_each_face_corner(f, |c| out.push(c));
        out
    }

    pub fn point_faces(&self, p: Index) -> Vec<Index> {
        let mut out = Vec::new();
        self.for_each_point_corner(p, |c| out.push(self.corners[c as usize].face));
        out.sort_unstable();
        out.dedup();
        out
    }

    pub fn bounds(&self) -> Bounds {
        self.point_pos
            .par_iter()
            .zip(self.point_dead.par_iter())
            .with_min_len(1 << 14)
            .fold(Bounds::default, |mut b, (p, &dead)| {
                if dead == 0 {
                    b.extend(*p);
                }
                b
            })
            .reduce(Bounds::default, |mut a, b| {
                a.extend_bounds(&b);
                a
            })
    }

    pub fn face_bounds(&self, faces: &[Index]) -> Bounds {
        let mut b = Bounds::default();
        for &f in faces {
            self.for_each_face_corner(f, |c| {
                b.extend(self.point_pos[self.corners[c as usize].point as usize])
            });
        }
        b
    }

    // Editing. New elements get default attributes unless a source is given.

    pub fn add_point(&mut self, pos: DVec3) -> Index {
        self.touch_topology();
        let p = self.point_pos.len() as Index;
        self.point_pos.push(pos);
        self.point_edge.push(INVALID);
        self.point_dead.push(0);
        let n = self.point_pos.len();
        self.point_attrs_mut().resize(n);
        self.num_points += 1;
        p
    }

    /// Returns the existing edge between a and b if there is one.
    pub fn add_edge(&mut self, a: Index, b: Index) -> Index {
        self.touch_topology();
        if a == b {
            return INVALID;
        }
        let e = self.find_edge(a, b);
        if e != INVALID {
            e
        } else {
            self.new_edge(a, b)
        }
    }

    /// INVALID if fewer than 3 points or a point repeats.
    pub fn add_face(&mut self, points: &[Index]) -> Index {
        self.touch_topology();
        let n = points.len();
        if n < 3 || has_repeats(points) || points.iter().any(|&p| !self.point_valid(p)) {
            return INVALID;
        }
        let f = self.faces.len() as Index;
        let first = self.corners.len() as Index;
        self.faces.push(Face {
            corner: first,
            size: n as Index,
        });
        for &p in points {
            self.new_corner(p, f);
        }
        for i in 0..n {
            let c = &mut self.corners[first as usize + i];
            c.next = first + ((i + 1) % n) as Index;
            c.prev = first + ((i + n - 1) % n) as Index;
            let e = self.add_edge(points[i], points[(i + 1) % n]);
            self.radial_insert(first + i as Index, e);
        }
        let (nc, nf) = (self.corners.len(), self.faces.len());
        self.corner_attrs_mut().resize(nc);
        self.face_attrs_mut().resize(nf);
        self.num_faces += 1;
        f
    }

    /// Same, copying face attributes from src_face and corner attributes from src_corners[i] into corner i (either may
    /// be INVALID; src_corners may be empty). Sources may be deleted elements.
    pub fn add_face_from(
        &mut self,
        points: &[Index],
        src_face: Index,
        src_corners: &[Index],
    ) -> Index {
        let f = self.add_face(points);
        if f == INVALID {
            return f;
        }
        if src_face != INVALID {
            self.face_attrs_mut().copy(f as usize, src_face as usize);
        }
        let first = self.faces[f as usize].corner;
        for (i, &sc) in src_corners.iter().enumerate().take(points.len()) {
            if sc != INVALID {
                self.corner_attrs_mut()
                    .copy(first as usize + i, sc as usize);
            }
        }
        f
    }

    /// Edges and points stay, possibly loose.
    pub fn remove_face(&mut self, f: Index) {
        self.touch_topology();
        if !self.face_valid(f) {
            return;
        }
        let mut c = self.faces[f as usize].corner;
        for _ in 0..self.faces[f as usize].size {
            let next = self.corners[c as usize].next;
            self.radial_remove(c);
            self.kill_corner(c);
            c = next;
        }
        self.faces[f as usize] = Face {
            corner: INVALID,
            size: 0,
        };
        self.num_faces -= 1;
    }

    /// Removes the edge and its faces.
    pub fn remove_edge(&mut self, e: Index) {
        self.touch_topology();
        if !self.edge_valid(e) {
            return;
        }
        while self.edges[e as usize].corner != INVALID {
            let f = self.corners[self.edges[e as usize].corner as usize].face;
            self.remove_face(f);
        }
        let [a, b] = self.edges[e as usize].v;
        self.disk_remove(e, a);
        self.disk_remove(e, b);
        self.edges[e as usize] = DEAD_EDGE;
        self.num_edges -= 1;
    }

    /// Removes the point, its edges and their faces.
    pub fn remove_point(&mut self, p: Index) {
        self.touch_topology();
        if !self.point_valid(p) {
            return;
        }
        while self.point_edge[p as usize] != INVALID {
            self.remove_edge(self.point_edge[p as usize]);
        }
        self.point_dead[p as usize] = 1;
        self.num_points -= 1;
    }

    /// Removes edges without faces, then points without edges, among the candidates.
    pub fn remove_loose(&mut self, edges: &[Index], points: &[Index]) {
        self.touch_topology();
        for &e in edges {
            if self.edge_valid(e) && self.edges[e as usize].corner == INVALID {
                self.remove_edge(e);
            }
        }
        for &p in points {
            if self.point_valid(p) && self.point_edge[p as usize] == INVALID {
                self.remove_point(p);
            }
        }
    }

    pub fn remove_all_loose(&mut self) {
        self.touch_topology();
        for e in 0..self.edge_capacity() {
            if self.edge_valid(e) && self.edges[e as usize].corner == INVALID {
                self.remove_edge(e);
            }
        }
        for p in 0..self.point_capacity() {
            if self.point_valid(p) && self.point_edge[p as usize] == INVALID {
                self.remove_point(p);
            }
        }
    }

    /// Splits e at t (0 at edge_point(e,0)) with a new point, returned. e keeps edge_point(e,0); the new edge runs from
    /// the new point to the old edge_point(e,1) and copies e's attributes.
    pub fn split_edge(&mut self, e: Index, t: f64) -> Index {
        self.touch_topology();
        let [a, b] = self.edges[e as usize].v;
        let m = self.add_point(crate::mix::mix3(
            self.point_pos[a as usize],
            self.point_pos[b as usize],
            t,
        ));
        self.point_attrs_mut()
            .mix(m as usize, &[a, b], &[(1.0 - t) as f32, t as f32]);

        let mut radial = Vec::new();
        self.for_each_edge_corner(e, |c| radial.push(c));

        // e becomes a-m, the new edge m-b.
        self.disk_remove(e, b);
        self.edges[e as usize].v[1] = m;
        self.disk_insert(e, m);
        let e2 = self.new_edge(m, b);
        self.edge_attrs_mut().copy(e2 as usize, e as usize);
        self.last_split_edge = e2;

        for c in radial {
            let f = self.corners[c as usize].face;
            let next = self.corners[c as usize].next;
            let n = self.new_corner(m, f);
            let nc = self.corners.len();
            self.corner_attrs_mut().resize(nc);
            self.corners[n as usize].prev = c;
            self.corners[n as usize].next = next;
            self.corners[c as usize].next = n;
            self.corners[next as usize].prev = n;
            self.faces[f as usize].size += 1;

            let tc = if self.corners[c as usize].point == a {
                t as f32
            } else {
                (1.0 - t) as f32
            };
            self.corner_attrs_mut()
                .mix(n as usize, &[c, next], &[1.0 - tc, tc]);

            if self.corners[c as usize].point == a {
                self.radial_insert(n, e2);
            } else {
                self.radial_remove(c);
                self.radial_insert(c, e2);
                self.radial_insert(n, e);
            }
        }
        m
    }

    /// Edge created by the last split_edge or split_face call.
    pub fn last_split_edge(&self) -> Index {
        self.last_split_edge
    }

    /// Cuts face f between the points of its corners a and b, which must not be neighbours. f keeps a..b, the returned
    /// new face gets b..a. The new edge is `last_split_edge()`.
    pub fn split_face(&mut self, f: Index, ca: Index, cb: Index) -> Index {
        self.touch_topology();
        let (cra, crb) = (self.corners[ca as usize], self.corners[cb as usize]);
        if ca == cb || cra.face != f || crb.face != f || cra.next == cb || crb.next == ca {
            return INVALID;
        }
        let (pa, pb) = (cra.point, crb.point);
        let ne = self.add_edge(pa, pb);
        self.last_split_edge = ne;
        let f2 = self.faces.len() as Index;
        self.faces.push(Face {
            corner: cb,
            size: 0,
        });
        let nf = self.faces.len();
        self.face_attrs_mut().resize(nf);
        self.face_attrs_mut().copy(f2 as usize, f as usize);
        self.num_faces += 1;

        let na = self.new_corner(pa, f2);
        let nb = self.new_corner(pb, f);
        let nc = self.corners.len();
        self.corner_attrs_mut().resize(nc);
        self.corner_attrs_mut().copy(na as usize, ca as usize);
        self.corner_attrs_mut().copy(nb as usize, cb as usize);

        let prev_a = self.corners[ca as usize].prev;
        let prev_b = self.corners[cb as usize].prev;
        // f: ca .. prevB, nb(pb -> pa)
        self.corners[prev_b as usize].next = nb;
        self.corners[nb as usize].prev = prev_b;
        self.corners[nb as usize].next = ca;
        self.corners[ca as usize].prev = nb;
        // f2: cb .. prevA, na(pa -> pb)
        self.corners[prev_a as usize].next = na;
        self.corners[na as usize].prev = prev_a;
        self.corners[na as usize].next = cb;
        self.corners[cb as usize].prev = na;

        self.radial_insert(na, ne);
        self.radial_insert(nb, ne);

        let mut size2 = 0;
        let mut c = cb;
        loop {
            self.corners[c as usize].face = f2;
            size2 += 1;
            if c == na {
                break;
            }
            c = self.corners[c as usize].next;
        }
        self.faces[f2 as usize].size = size2;
        self.faces[f as usize].size = self.faces[f as usize].size + 2 - size2;
        self.faces[f as usize].corner = ca;
        f2
    }

    /// Removes e and merges its two faces into one, returned. INVALID if that is not possible.
    pub fn join_faces(&mut self, e: Index) -> Index {
        self.touch_topology();
        if !self.is_manifold_edge(e) {
            return INVALID;
        }
        let c1 = self.edges[e as usize].corner;
        let c2 = self.corners[c1 as usize].radial_next;
        let f1 = self.corners[c1 as usize].face;
        let f2 = self.corners[c2 as usize].face;
        if f1 == f2 || self.corners[c1 as usize].point == self.corners[c2 as usize].point {
            return INVALID;
        }
        // The merged loop must not visit a point twice.
        let mut pts = Vec::new();
        let mut c = self.corners[c1 as usize].next;
        while c != c1 {
            pts.push(self.corners[c as usize].point);
            c = self.corners[c as usize].next;
        }
        let mut c = self.corners[self.corners[c2 as usize].next as usize].next;
        while c != c2 {
            pts.push(self.corners[c as usize].point);
            c = self.corners[c as usize].next;
        }
        if has_repeats(&pts) {
            return INVALID;
        }
        let n1 = self.corners[c1 as usize].next;
        let n2 = self.corners[c2 as usize].next;
        let p1 = self.corners[c1 as usize].prev;
        let p2 = self.corners[c2 as usize].prev;
        self.corners[p1 as usize].next = n2;
        self.corners[n2 as usize].prev = p1;
        self.corners[p2 as usize].next = n1;
        self.corners[n1 as usize].prev = p2;
        let mut c = n2;
        while c != n1 {
            self.corners[c as usize].face = f1;
            c = self.corners[c as usize].next;
        }
        self.faces[f1 as usize].size += self.faces[f2 as usize].size - 2;
        self.faces[f1 as usize].corner = n1;
        self.faces[f2 as usize] = Face {
            corner: INVALID,
            size: 0,
        };
        self.num_faces -= 1;
        self.radial_remove(c1);
        self.radial_remove(c2);
        self.kill_corner(c1);
        self.kill_corner(c2);
        self.remove_edge(e);
        f1
    }

    /// Moves the corners of f onto new points (same count, in order), keeping face and corner ids and their
    /// attributes. Edges left without faces stay, possibly loose. False if the points are invalid.
    pub fn set_face_points(&mut self, f: Index, points: &[Index]) -> bool {
        self.touch_topology();
        if !self.face_valid(f)
            || points.len() != self.faces[f as usize].size as usize
            || has_repeats(points)
            || points.iter().any(|&p| !self.point_valid(p))
        {
            return false;
        }
        let corners = self.face_corners(f);
        let n = corners.len();
        for &c in &corners {
            self.radial_remove(c);
        }
        for i in 0..n {
            self.corners[corners[i] as usize].point = points[i];
        }
        for i in 0..n {
            let e = self.add_edge(points[i], points[(i + 1) % n]);
            self.radial_insert(corners[i], e);
        }
        true
    }

    /// Reverses the winding of f, keeping corner attributes on their points.
    pub fn flip_face(&mut self, f: Index) {
        self.touch_topology();
        let corners = self.face_corners(f);
        let n = corners.len();
        let mut edges = vec![0; n];
        for i in 0..n {
            edges[i] = self.corners[corners[i] as usize].edge;
            self.radial_remove(corners[i]);
        }
        for i in 0..n {
            let c = &mut self.corners[corners[i] as usize];
            c.next = corners[(i + n - 1) % n];
            c.prev = corners[(i + 1) % n];
            self.radial_insert(corners[i], edges[(i + n - 1) % n]);
        }
    }

    pub fn clear(&mut self) {
        *self = Mesh::default();
    }

    /// Gives the points new ids: new_to_old[n] is the point that becomes n. It must name every point, on a mesh with
    /// no dead points. Faces, edges and corners keep their ids.
    pub fn reorder_points(&mut self, new_to_old: &[Index]) {
        self.touch_topology();
        let mut old_to_new = vec![INVALID; self.point_capacity() as usize];
        for (n, &o) in new_to_old.iter().enumerate() {
            old_to_new[o as usize] = n as Index;
        }
        self.point_pos = new_to_old.iter().map(|&o| self.point_pos[o as usize]).collect::<Vec<_>>().into();
        self.point_edge = new_to_old.iter().map(|&o| self.point_edge[o as usize]).collect::<Vec<_>>().into();
        for e in self.edges.iter_mut() {
            if e.v[0] != INVALID {
                e.v = [old_to_new[e.v[0] as usize], old_to_new[e.v[1] as usize]];
            }
        }
        for c in self.corners.iter_mut() {
            if c.point != INVALID {
                c.point = old_to_new[c.point as usize];
            }
        }
        self.attrs[0].reorder(new_to_old);
    }

    /// Drops deleted elements and makes face corners contiguous.
    pub fn compact(&mut self, remap: Option<&mut MeshRemap>) {
        self.touch_topology();
        let mut local = MeshRemap::default();
        let r = remap.unwrap_or(&mut local);
        fn build(
            cap: Index,
            alive: impl Fn(Index) -> bool,
            old_to_new: &mut Vec<Index>,
        ) -> Vec<Index> {
            *old_to_new = vec![INVALID; cap as usize];
            let mut new_to_old = Vec::new();
            for i in 0..cap {
                if alive(i) {
                    old_to_new[i as usize] = new_to_old.len() as Index;
                    new_to_old.push(i);
                }
            }
            new_to_old
        }
        let p_old = build(
            self.point_capacity(),
            |i| self.point_valid(i),
            &mut r.points,
        );
        let e_old = build(self.edge_capacity(), |i| self.edge_valid(i), &mut r.edges);
        let f_old = build(self.face_capacity(), |i| self.face_valid(i), &mut r.faces);

        r.corners = vec![INVALID; self.corner_capacity() as usize];
        let mut c_old = Vec::with_capacity(self.num_corners as usize);
        for &f in &f_old {
            self.for_each_face_corner(f, |c| {
                r.corners[c as usize] = c_old.len() as Index;
                c_old.push(c);
            });
        }
        let map = |m: &[Index], i: Index| if i == INVALID { INVALID } else { m[i as usize] };

        let pos: Vec<DVec3> = p_old.iter().map(|&p| self.point_pos[p as usize]).collect();
        let p_edge: Vec<Index> = p_old
            .iter()
            .map(|&p| map(&r.edges, self.point_edge[p as usize]))
            .collect();
        let edges: Vec<Edge> = e_old
            .iter()
            .map(|&e| {
                let o = &self.edges[e as usize];
                let mut n = DEAD_EDGE;
                for s in 0..2 {
                    n.v[s] = r.points[o.v[s] as usize];
                    n.next[s] = r.edges[o.next[s] as usize];
                    n.prev[s] = r.edges[o.prev[s] as usize];
                }
                n.corner = map(&r.corners, o.corner);
                n
            })
            .collect();
        let faces: Vec<Face> = f_old
            .iter()
            .map(|&f| Face {
                corner: r.corners[self.faces[f as usize].corner as usize],
                size: self.faces[f as usize].size,
            })
            .collect();
        let corners: Vec<Corner> = c_old
            .iter()
            .map(|&c| {
                let o = &self.corners[c as usize];
                Corner {
                    point: r.points[o.point as usize],
                    edge: r.edges[o.edge as usize],
                    face: r.faces[o.face as usize],
                    next: r.corners[o.next as usize],
                    prev: r.corners[o.prev as usize],
                    radial_next: r.corners[o.radial_next as usize],
                    radial_prev: r.corners[o.radial_prev as usize],
                }
            })
            .collect();

        self.point_dead = (vec![0; pos.len()]).into();
        self.point_pos = (pos).into();
        self.point_edge = (p_edge).into();
        self.edges = (edges).into();
        self.faces = (faces).into();
        self.corners = (corners).into();
        self.attrs[0].reorder(&p_old);
        self.attrs[1].reorder(&e_old);
        self.attrs[2].reorder(&f_old);
        self.attrs[3].reorder(&c_old);
        self.last_split_edge = INVALID;
    }

    /// Appends other's elements; returns the point id offset. Attribute layers are merged by name.
    pub fn append(&mut self, other: &Mesh) -> Index {
        self.touch_topology();
        for d in 0..4 {
            self.attrs[d].merge_layout(&other.attrs[d]);
        }
        if self.color_set_info.current.is_empty() {
            self.color_set_info.current = other.color_set_info.current.clone();
        }
        for h in &other.color_set_info.half {
            if !self.color_set_info.half.contains(h) {
                self.color_set_info.half.push(h.clone());
            }
        }
        // Material ids are remapped by name.
        let mat_map: Vec<i32> = other
            .material_names
            .iter()
            .map(
                |name| match self.material_names.iter().position(|m| m == name) {
                    Some(i) => i as i32,
                    None => {
                        self.material_names.push(name.clone());
                        self.material_names.len() as i32 - 1
                    }
                },
            )
            .collect();

        let offset = self.point_capacity();
        let mut pmap = vec![INVALID; other.point_capacity() as usize];
        for p in 0..other.point_capacity() {
            if other.point_valid(p) {
                let np = self.add_point(other.point_pos[p as usize]);
                pmap[p as usize] = np;
                self.attrs[0].copy_from(&other.attrs[0], np as usize, p as usize);
            }
        }
        let first_new_face = self.face_capacity();
        for f in 0..other.face_capacity() {
            if !other.face_valid(f) {
                continue;
            }
            let pts: Vec<Index> = other
                .face_points(f)
                .iter()
                .map(|&p| pmap[p as usize])
                .collect();
            let nf = self.add_face(&pts);
            if nf == INVALID {
                continue;
            }
            self.attrs[2].copy_from(&other.attrs[2], nf as usize, f as usize);
            let mut c = self.faces[nf as usize].corner;
            for oc in other.face_corners(f) {
                self.attrs[3].copy_from(&other.attrs[3], c as usize, oc as usize);
                c = self.corners[c as usize].next;
            }
        }
        for e in 0..other.edge_capacity() {
            if !other.edge_valid(e) {
                continue;
            }
            let [a, b] = other.edges[e as usize].v;
            let ne = self.add_edge(pmap[a as usize], pmap[b as usize]);
            if ne != INVALID {
                self.attrs[1].copy_from(&other.attrs[1], ne as usize, e as usize);
            }
        }
        if !mat_map.is_empty() {
            let cap = self.face_capacity();
            let mat = self.attrs[2].get_mut::<i32>(names::MATERIAL);
            if !mat.is_empty() {
                for f in first_new_face..cap {
                    let m = mat[f as usize];
                    if m >= 0 && (m as usize) < mat_map.len() {
                        mat[f as usize] = mat_map[m as usize];
                    }
                }
            }
        }
        offset
    }

    /// Problems found, one line each. Empty means valid for the given checks (see `validate`).
    pub fn validate(&self, flags: u32) -> Vec<String> {
        let mut out = Vec::new();
        let mut bad = |s: String| {
            if out.len() < 200 {
                out.push(s);
            }
        };
        let (pc, ec, cc) = (
            self.point_capacity(),
            self.edge_capacity(),
            self.corner_capacity(),
        );
        let (mut live_points, mut live_edges, mut live_faces, mut live_corners) = (0, 0, 0, 0);
        for p in 0..pc {
            if !self.point_valid(p) {
                continue;
            }
            live_points += 1;
            let e = self.point_edge[p as usize];
            if e != INVALID
                && (!self.edge_valid(e)
                    || (self.edges[e as usize].v[0] != p && self.edges[e as usize].v[1] != p))
            {
                bad(format!("point {p}: edge {e} does not contain it"));
            } else if e == INVALID && flags & validate::LOOSE != 0 {
                bad(format!("point {p}: loose"));
            }
        }
        for e in 0..ec {
            if !self.edge_valid(e) {
                continue;
            }
            live_edges += 1;
            let ed = self.edges[e as usize];
            if !self.point_valid(ed.v[0]) || !self.point_valid(ed.v[1]) || ed.v[0] == ed.v[1] {
                bad(format!("edge {e}: bad points {} {}", ed.v[0], ed.v[1]));
                continue;
            }
            for s in 0..2 {
                let p = ed.v[s];
                let n = ed.next[s];
                if !self.edge_valid(n)
                    || (self.edges[n as usize].v[0] != p && self.edges[n as usize].v[1] != p)
                    || self.edges[n as usize].prev[self.side(n, p)] != e
                {
                    bad(format!("edge {e}: broken disk link at point {p}"));
                    continue;
                }
                let (mut found, mut steps) = (false, 0);
                self.for_each_point_edge(p, |x| {
                    found |= x == e;
                    steps += 1;
                });
                if !found || steps > ec {
                    bad(format!("edge {e}: not in disk cycle of point {p}"));
                }
            }
            if ed.corner == INVALID {
                if flags & validate::LOOSE != 0 {
                    bad(format!("edge {e}: loose"));
                }
                continue;
            }
            let (mut count, mut ok) = (0, true);
            let mut c = ed.corner;
            loop {
                if !self.corner_valid(c)
                    || self.corners[c as usize].edge != e
                    || self.corners[self.corners[c as usize].radial_next as usize].radial_prev != c
                {
                    ok = false;
                    break;
                }
                c = self.corners[c as usize].radial_next;
                if c == ed.corner {
                    break;
                }
                count += 1;
                if count > cc {
                    break;
                }
            }
            if !ok || count > cc {
                bad(format!("edge {e}: broken radial cycle"));
            } else if flags & validate::MANIFOLD != 0 && count + 1 > 2 {
                bad(format!("edge {e}: non-manifold, {} faces", count + 1));
            } else if flags & validate::WINDING != 0 && count + 1 == 2 {
                let r = self.corners[ed.corner as usize].radial_next;
                if self.corners[r as usize].point == self.corners[ed.corner as usize].point {
                    bad(format!(
                        "edge {e}: faces {} and {} wound inconsistently",
                        self.corners[ed.corner as usize].face, self.corners[r as usize].face
                    ));
                }
            }
            if flags & validate::DEGENERATE != 0 && self.edge_length(e) <= 0.0 {
                bad(format!("edge {e}: zero length"));
            }
        }
        for f in 0..self.face_capacity() {
            if !self.face_valid(f) {
                continue;
            }
            live_faces += 1;
            let n = self.faces[f as usize].size;
            let (mut count, mut ok) = (0, true);
            let mut c = self.faces[f as usize].corner;
            let mut pts = Vec::new();
            loop {
                if !self.corner_valid(c)
                    || self.corners[c as usize].face != f
                    || self.corners[self.corners[c as usize].next as usize].prev != c
                {
                    ok = false;
                    break;
                }
                let cr = self.corners[c as usize];
                let np = self.corners[cr.next as usize].point;
                let ce = cr.edge;
                if !self.edge_valid(ce)
                    || !((self.edges[ce as usize].v[0] == cr.point
                        && self.edges[ce as usize].v[1] == np)
                        || (self.edges[ce as usize].v[1] == cr.point
                            && self.edges[ce as usize].v[0] == np))
                {
                    bad(format!(
                        "face {f}: corner {c} edge does not join its points"
                    ));
                }
                pts.push(cr.point);
                c = cr.next;
                if c == self.faces[f as usize].corner {
                    break;
                }
                count += 1;
                if count > cc {
                    break;
                }
            }
            if !ok || count + 1 != n {
                bad(format!("face {f}: broken corner loop (size {n})"));
                continue;
            }
            if n < 3 {
                bad(format!("face {f}: only {n} corners"));
            }
            if has_repeats(&pts) {
                bad(format!("face {f}: repeated point"));
            }
            if flags & validate::DEGENERATE != 0 && self.face_area(f) <= 1e-12 {
                bad(format!("face {f}: zero area"));
            }
        }
        for c in 0..cc {
            if self.corner_valid(c) {
                live_corners += 1;
            }
        }
        if live_points != self.num_points
            || live_edges != self.num_edges
            || live_faces != self.num_faces
            || live_corners != self.num_corners
        {
            bad(format!(
                "counts out of sync: points {live_points}/{} edges {live_edges}/{} faces {live_faces}/{} corners {live_corners}/{}",
                self.num_points, self.num_edges, self.num_faces, self.num_corners
            ));
        }
        for (d, dom) in Domain::ALL.iter().enumerate() {
            let want = [pc, ec, self.face_capacity(), cc][d] as usize;
            if self.attrs[d].len() != want {
                bad(format!(
                    "{} attributes sized {} for {want} elements",
                    dom.name(),
                    self.attrs[d].len()
                ));
            }
        }
        out
    }

    pub fn attributes(&self, d: Domain) -> &AttributeSet {
        &self.attrs[d as usize]
    }
    pub fn attributes_mut(&mut self, d: Domain) -> &mut AttributeSet {
        &mut self.attrs[d as usize]
    }
    pub fn point_attrs(&self) -> &AttributeSet {
        &self.attrs[0]
    }
    pub fn edge_attrs(&self) -> &AttributeSet {
        &self.attrs[1]
    }
    pub fn face_attrs(&self) -> &AttributeSet {
        &self.attrs[2]
    }
    pub fn corner_attrs(&self) -> &AttributeSet {
        &self.attrs[3]
    }
    pub fn point_attrs_mut(&mut self) -> &mut AttributeSet {
        &mut self.attrs[0]
    }
    pub fn edge_attrs_mut(&mut self) -> &mut AttributeSet {
        &mut self.attrs[1]
    }
    pub fn face_attrs_mut(&mut self) -> &mut AttributeSet {
        &mut self.attrs[2]
    }
    pub fn corner_attrs_mut(&mut self) -> &mut AttributeSet {
        &mut self.attrs[3]
    }

    /// Names for face "material" ids.
    pub fn material_names(&self) -> &Vec<String> {
        &self.material_names
    }
    pub fn material_names_mut(&mut self) -> &mut Vec<String> {
        &mut self.material_names
    }
    pub fn color_set_info(&self) -> &ColorSetInfo {
        &self.color_set_info
    }
    pub fn color_set_info_mut(&mut self) -> &mut ColorSetInfo {
        &mut self.color_set_info
    }

    pub fn reserve(&mut self, points: usize, edges: usize, faces: usize, corners: usize) {
        self.point_pos.reserve(points);
        self.point_edge.reserve(points);
        self.point_dead.reserve(points);
        self.edges.reserve(edges);
        self.faces.reserve(faces);
        self.corners.reserve(corners);
    }

    /// Builds from a polygon soup: face_sizes[i] corners per face, indices into points. Bad faces are skipped.
    pub fn from_polygons(points: &[DVec3], face_sizes: &[Index], face_indices: &[Index]) -> Mesh {
        let mut m = Mesh::new();
        if face_indices.len() < (1 << 16) && points.len() < (1 << 16) {
            m.reserve(
                points.len(),
                face_indices.len(),
                face_sizes.len(),
                face_indices.len(),
            );
            for &p in points {
                m.add_point(p);
            }
            let mut at = 0;
            for &n in face_sizes {
                let n = n as usize;
                if at + n > face_indices.len() {
                    break;
                }
                m.add_face(&face_indices[at..at + n]);
                at += n;
            }
            return m;
        }
        Self::from_polygons_bulk(points, face_sizes, face_indices)
    }

    /// The mesh `add_face` builds face by face, ids and disk and radial order included, built in bulk.
    fn from_polygons_bulk(points: &[DVec3], face_sizes: &[Index], face_indices: &[Index]) -> Mesh {
        let mut m = Mesh::new();
        let mut in_start = Vec::with_capacity(face_sizes.len());
        let mut at = 0usize;
        for &n in face_sizes {
            if at + n as usize > face_indices.len() {
                break;
            }
            in_start.push(at as Index);
            at += n as usize;
        }
        let in_faces = in_start.len();
        let num_points = points.len() as Index;
        let valid: Vec<bool> = (0..in_faces)
            .into_par_iter()
            .map(|f| {
                let pts = &face_indices
                    [in_start[f] as usize..in_start[f] as usize + face_sizes[f] as usize];
                pts.len() >= 3 && pts.iter().all(|&p| p < num_points) && !has_repeats(pts)
            })
            .collect();
        let mut face_of: Vec<Index> = valid.iter().map(|&v| v as Index).collect();
        let mut corner_of: Vec<Index> = (0..in_faces)
            .map(|f| if valid[f] { face_sizes[f] } else { 0 })
            .collect();
        let num_faces = exclusive_scan(&mut face_of);
        let num_corners = exclusive_scan(&mut corner_of);

        m.point_pos = (points.to_vec()).into();
        m.point_edge = (vec![INVALID; points.len()]).into();
        m.point_dead = (vec![0; points.len()]).into();
        m.faces = (vec![Face { corner: 0, size: 0 }; num_faces as usize]).into();
        m.corners = (vec![DEAD_CORNER; num_corners as usize]).into();
        for f in 0..in_faces {
            if !valid[f] {
                continue;
            }
            let (n, face, first) = (face_sizes[f], face_of[f], corner_of[f]);
            m.faces[face as usize] = Face {
                corner: first,
                size: n,
            };
            for i in 0..n {
                m.corners[(first + i) as usize] = Corner {
                    point: face_indices[(in_start[f] + i) as usize],
                    edge: INVALID,
                    face,
                    next: first + (i + 1) % n,
                    prev: first + (i + n - 1) % n,
                    radial_next: INVALID,
                    radial_prev: INVALID,
                };
            }
        }

        // Corners on one edge, grouped under the edge's lower point and sorted by (higher point, corner): the first
        // corner of each run makes the edge.
        let corners = &m.corners;
        let mut low_start = Vec::new();
        let by_low = bucket_by(
            num_corners as usize,
            num_points as usize,
            |c| {
                corners[c]
                    .point
                    .min(corners[corners[c].next as usize].point)
            },
            |c| {
                ((corners[c]
                    .point
                    .max(corners[corners[c].next as usize].point) as u64)
                    << 32)
                    | c as u64
            },
            &mut low_start,
        );
        let groups = |p: usize| {
            let mut out = Vec::new();
            let (mut i, end) = (low_start[p], low_start[p + 1]);
            while i < end {
                let mut j = i + 1;
                while j < end && by_low[j as usize] >> 32 == by_low[i as usize] >> 32 {
                    j += 1;
                }
                out.push((i, j - i));
                i = j;
            }
            out
        };
        let corner_at = |i: Index| by_low[i as usize] as u32;
        let mut edge_of = vec![0 as Index; num_corners as usize];
        for p in 0..num_points as usize {
            for (i, _) in groups(p) {
                edge_of[corner_at(i) as usize] = 1;
            }
        }
        let num_edges = exclusive_scan(&mut edge_of);
        m.edges = (vec![DEAD_EDGE; num_edges as usize]).into();
        for p in 0..num_points as usize {
            for (at, k) in groups(p) {
                let c0 = corner_at(at);
                let e = edge_of[c0 as usize];
                let c0r = m.corners[c0 as usize];
                m.edges[e as usize] = Edge {
                    v: [c0r.point, m.corners[c0r.next as usize].point],
                    corner: c0,
                    ..DEAD_EDGE
                };
                // radial_insert links each new corner straight after the first: c0, then the rest newest first.
                for i in 0..k {
                    let c = &mut m.corners[corner_at(at + i) as usize];
                    c.edge = e;
                    c.radial_next = corner_at(at + if i == 0 { k - 1 } else { i - 1 });
                    c.radial_prev = corner_at(at + if i == k - 1 { 0 } else { i + 1 });
                }
            }
        }

        // Disk cycles in disk_insert's order: the point's first edge, then the rest newest first.
        let edges = &m.edges;
        let mut disk_start = Vec::new();
        let by_point = bucket_by(
            num_edges as usize * 2,
            num_points as usize,
            |i| edges[i / 2].v[i % 2],
            |i| i as Index,
            &mut disk_start,
        );
        for p in 0..num_points {
            let lo = disk_start[p as usize];
            let k = disk_start[p as usize + 1] - lo;
            if k == 0 {
                continue;
            }
            let edge = |i: Index| by_point[(lo + i) as usize] / 2;
            m.point_edge[p as usize] = edge(0);
            for i in 0..k {
                let ed = &mut m.edges[edge(i) as usize];
                let s = if ed.v[0] == p { 0 } else { 1 };
                ed.next[s] = edge(if i == 0 { k - 1 } else { i - 1 });
                ed.prev[s] = edge(if i == k - 1 { 0 } else { i + 1 });
            }
        }

        m.num_points = num_points;
        m.num_edges = num_edges;
        m.num_faces = num_faces;
        m.num_corners = num_corners;
        m.attrs[0].resize(num_points as usize);
        m.attrs[1].resize(num_edges as usize);
        m.attrs[2].resize(num_faces as usize);
        m.attrs[3].resize(num_corners as usize);
        m
    }

    #[doc(hidden)]
    /// Test hook: the bulk builder regardless of size, to check it against the face-by-face one.
    pub fn from_polygons_bulk_for_tests(
        points: &[DVec3],
        face_sizes: &[Index],
        face_indices: &[Index],
    ) -> Mesh {
        Self::from_polygons_bulk(points, face_sizes, face_indices)
    }

    /// Topology fingerprint for comparing builders: every corner's links and every edge's links.
    #[doc(hidden)]
    pub fn topology_dump(&self) -> Vec<[Index; 8]> {
        let mut out: Vec<[Index; 8]> = self
            .corners
            .iter()
            .map(|c| {
                [
                    c.point,
                    c.edge,
                    c.face,
                    c.next,
                    c.prev,
                    c.radial_next,
                    c.radial_prev,
                    0,
                ]
            })
            .collect();
        out.extend(self.edges.iter().map(|e| {
            [
                e.v[0], e.v[1], e.next[0], e.next[1], e.prev[0], e.prev[1], e.corner, 1,
            ]
        }));
        out.extend(self.point_edge.iter().map(|&e| [e, 0, 0, 0, 0, 0, 0, 2]));
        out
    }
}
