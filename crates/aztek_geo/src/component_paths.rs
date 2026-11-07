//! Paths and regions between picked components.

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeMap, BinaryHeap};

use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;
use crate::topology::{edge_loop, edge_loop_next, edge_ring, face_components, make_mask, quad_opposite_edge};

/// A (distance, id) heap entry ordered like std::pair<double, Index>.
#[derive(Clone, Copy, PartialEq)]
struct Item(f64, Index);

impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> Ordering {
        self.0.total_cmp(&o.0).then(self.1.cmp(&o.1))
    }
}

fn shared_point(m: &Mesh, a: Index, b: Index) -> Index {
    for i in 0..2 {
        if m.edge_point(a, i) == m.edge_point(b, 0) || m.edge_point(a, i) == m.edge_point(b, 1) {
            return m.edge_point(a, i);
        }
    }
    INVALID
}

/// The quad holding a with b opposite it, or INVALID.
fn shared_quad(m: &Mesh, a: Index, b: Index) -> Index {
    let mut found = INVALID;
    m.for_each_edge_corner(a, |c| {
        let f = m.corner_face(c);
        if found == INVALID && quad_opposite_edge(m, a, f) == b {
            found = f;
        }
    });
    found
}

fn loop_closed(m: &Mesh, lp: &[Index]) -> bool {
    if lp.len() < 3 {
        return false;
    }
    let last = *lp.last().unwrap();
    let shared = shared_point(m, last, lp[lp.len() - 2]);
    if shared == INVALID {
        return false;
    }
    edge_loop_next(m, last, m.edge_other_point(last, shared)) == lp[0]
}

/// The points an ordered edge chain runs through; a closed chain does not repeat its first point.
fn chain_points(m: &Mesh, edges: &[Index], closed: bool) -> Vec<Index> {
    if edges.is_empty() {
        return Vec::new();
    }
    if edges.len() == 1 {
        return vec![m.edge_point(edges[0], 0), m.edge_point(edges[0], 1)];
    }
    let shared = shared_point(m, edges[0], edges[1]);
    let mut cur = m.edge_other_point(edges[0], shared);
    let mut out = vec![cur];
    for &e in edges {
        cur = m.edge_other_point(e, cur);
        out.push(cur);
    }
    if closed {
        out.pop();
    }
    out
}

struct Faces {
    faces: Vec<Index>,
    closed: bool,
}

fn ring_faces(m: &Mesh, ring: &[Index]) -> Faces {
    let mut out = Faces { faces: Vec::new(), closed: false };
    for k in 0..ring.len().saturating_sub(1) {
        let f = shared_quad(m, ring[k], ring[k + 1]);
        if f != INVALID {
            out.faces.push(f);
        }
    }
    if ring.len() >= 3 {
        let f = shared_quad(m, *ring.last().unwrap(), ring[0]);
        if f != INVALID && (out.faces.is_empty() || f != out.faces[0]) {
            out.faces.push(f);
            out.closed = true;
        }
    }
    out
}

fn ring_closed(m: &Mesh, ring: &[Index]) -> bool {
    ring_faces(m, ring).closed
}

/// s[i] to s[j] inclusive, the shorter way round when closed.
fn arc(s: &[Index], i: usize, j: usize, closed: bool) -> Vec<Index> {
    let mut out = Vec::new();
    if !closed {
        let mut k = i;
        loop {
            out.push(s[k]);
            if k == j {
                break;
            }
            if i <= j {
                k += 1;
            } else {
                k -= 1;
            }
        }
        return out;
    }
    let n = s.len();
    let fwd = (j + n - i) % n;
    let bwd = (i + n - j) % n;
    for k in 0..=fwd.min(bwd) {
        out.push(s[if fwd <= bwd { (i + k) % n } else { (i + n - k) % n }]);
    }
    out
}

/// Keeps the shorter of best and the arc from a to b along seq, if both lie on it.
fn consider(best: &mut Option<Vec<Index>>, seq: &[Index], closed: bool, a: Index, b: Index) {
    let (Some(ia), Some(ib)) = (seq.iter().position(|&x| x == a), seq.iter().position(|&x| x == b)) else { return };
    let path = arc(seq, ia, ib, closed);
    if best.as_ref().map_or(true, |b| path.len() < b.len()) {
        *best = Some(path);
    }
}

struct PointPath {
    points: Vec<Index>,
    /// edges[i] joins points[i] and points[i + 1].
    edges: Vec<Index>,
}

/// Shortest path by edge length from any source point to any target point.
fn shortest_points(m: &Mesh, sources: &[Index], targets: &[Index]) -> Option<PointPath> {
    let pc = m.point_capacity() as usize;
    let mut dist = vec![f64::INFINITY; pc];
    let mut via = vec![INVALID; pc];
    let target = make_mask(pc, targets);
    let mut queue = BinaryHeap::new();
    for &s in sources {
        if m.point_valid(s) {
            dist[s as usize] = 0.0;
            queue.push(Reverse(Item(0.0, s)));
        }
    }
    while let Some(Reverse(Item(d, p))) = queue.pop() {
        if d > dist[p as usize] {
            continue;
        }
        if target[p as usize] != 0 {
            let mut out = PointPath { points: Vec::new(), edges: Vec::new() };
            let mut cur = p;
            loop {
                out.points.push(cur);
                let e = via[cur as usize];
                if e == INVALID {
                    break;
                }
                out.edges.push(e);
                cur = m.edge_other_point(e, cur);
            }
            out.points.reverse();
            out.edges.reverse();
            return Some(out);
        }
        m.for_each_point_edge(p, |e| {
            let q = m.edge_other_point(e, p);
            let nd = d + m.edge_length(e);
            if nd < dist[q as usize] {
                dist[q as usize] = nd;
                via[q as usize] = e;
                queue.push(Reverse(Item(nd, q)));
            }
        });
    }
    None
}

fn unique_in_order(ids: &[Index]) -> Vec<Index> {
    let mut out: Vec<Index> = Vec::new();
    for &i in ids {
        if !out.contains(&i) {
            out.push(i);
        }
    }
    out
}

/// Faces crossed by edge e's ring, in ring order: the quads on either side of e and on through the ring.
pub fn face_loop(mesh: &Mesh, e: Index) -> Vec<Index> {
    if !mesh.edge_valid(e) {
        return Vec::new();
    }
    ring_faces(mesh, &edge_ring(mesh, e)).faces
}

/// The components from a to b inclusive, in order: the shorter way along a loop (or ring, for edges) the two share,
/// else the shortest path across the surface (by length for points and edges, by centroid distance for faces). Empty
/// when they are not connected.
pub fn point_path_between(m: &Mesh, a: Index, b: Index) -> Vec<Index> {
    if !m.point_valid(a) || !m.point_valid(b) {
        return Vec::new();
    }
    if a == b {
        return vec![a];
    }
    let mut best = None;
    m.for_each_point_edge(a, |e| {
        let lp = edge_loop(m, e);
        let closed = loop_closed(m, &lp);
        consider(&mut best, &chain_points(m, &lp, closed), closed, a, b);
    });
    if let Some(b) = best {
        return b;
    }
    shortest_points(m, &[a], &[b]).map(|p| p.points).unwrap_or_default()
}

pub fn edge_path_between(m: &Mesh, a: Index, b: Index) -> Vec<Index> {
    if !m.edge_valid(a) || !m.edge_valid(b) {
        return Vec::new();
    }
    if a == b {
        return vec![a];
    }
    let mut best = None;
    let lp = edge_loop(m, a);
    consider(&mut best, &lp, loop_closed(m, &lp), a, b);
    let ring = edge_ring(m, a);
    consider(&mut best, &ring, ring_closed(m, &ring), a, b);
    if let Some(b) = best {
        return b;
    }
    let from = [m.edge_point(a, 0), m.edge_point(a, 1)];
    let to = [m.edge_point(b, 0), m.edge_point(b, 1)];
    let Some(path) = shortest_points(m, &from, &to) else { return Vec::new() };
    let mut out = vec![a];
    out.extend_from_slice(&path.edges);
    out.push(b);
    unique_in_order(&out)
}

pub fn face_path_between(m: &Mesh, a: Index, b: Index) -> Vec<Index> {
    if !m.face_valid(a) || !m.face_valid(b) {
        return Vec::new();
    }
    if a == b {
        return vec![a];
    }
    let mut best = None;
    for c in m.face_corners(a) {
        let lp = ring_faces(m, &edge_ring(m, m.corner_edge(c)));
        consider(&mut best, &lp.faces, lp.closed, a, b);
    }
    if let Some(b) = best {
        return b;
    }

    // Across shared edges, weighted by the distance between face centres.
    let fc = m.face_capacity() as usize;
    let mut dist = vec![f64::INFINITY; fc];
    let mut via = vec![INVALID; fc];
    let mut queue = BinaryHeap::new();
    dist[a as usize] = 0.0;
    queue.push(Reverse(Item(0.0, a)));
    while let Some(Reverse(Item(d, f))) = queue.pop() {
        if d > dist[f as usize] {
            continue;
        }
        if f == b {
            break;
        }
        let cf = m.face_centroid(f);
        m.for_each_face_corner(f, |c| {
            m.for_each_edge_corner(m.corner_edge(c), |r| {
                let g = m.corner_face(r);
                if g == f {
                    return;
                }
                let nd = d + (m.face_centroid(g) - cf).length();
                if nd < dist[g as usize] {
                    dist[g as usize] = nd;
                    via[g as usize] = f;
                    queue.push(Reverse(Item(nd, g)));
                }
            });
        });
    }
    if via[b as usize] == INVALID {
        return Vec::new();
    }
    let mut out = Vec::new();
    let mut f = b;
    while f != INVALID {
        out.push(f);
        f = via[f as usize];
    }
    out.reverse();
    out
}

/// Faces enclosed by the given edges, flood filled without crossing them (mesh boundaries are walls too). Per
/// connected piece of the mesh the edges split into regions: smaller keeps every region but the largest by area,
/// otherwise only the largest is kept. A piece the edges do not split contributes nothing. Sorted.
pub fn contained_faces(mesh: &Mesh, edges: &[Index], smaller: bool) -> Vec<Index> {
    let mut piece = Vec::new();
    let mut region = Vec::new();
    face_components(mesh, &mut piece, &[]);
    let wall = make_mask(mesh.edge_capacity() as usize, edges);
    let regions = face_components(mesh, &mut region, &wall) as usize;

    let mut area = vec![0.0; regions];
    let mut piece_of = vec![INVALID; regions];
    for f in 0..mesh.face_capacity() {
        if mesh.face_valid(f) {
            let r = region[f as usize] as usize;
            area[r] += mesh.face_area(f);
            piece_of[r] = piece[f as usize];
        }
    }
    let mut by_piece: BTreeMap<Index, Vec<usize>> = BTreeMap::new();
    for r in 0..regions {
        by_piece.entry(piece_of[r]).or_default().push(r);
    }

    let mut keep = vec![false; regions];
    for rs in by_piece.values() {
        if rs.len() < 2 {
            continue;
        }
        let mut largest = rs[0];
        for &r in &rs[1..] {
            if area[largest] < area[r] {
                largest = r;
            }
        }
        for &r in rs {
            keep[r] = if smaller { r != largest } else { r == largest };
        }
    }
    (0..mesh.face_capacity()).filter(|&f| mesh.face_valid(f) && keep[region[f as usize] as usize]).collect()
}
