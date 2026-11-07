//! Planar cuts through placed meshes.

use std::collections::HashMap;

use glam::{DMat4, DVec2, DVec3};

use crate::mesh::Mesh;
use crate::ops::common::inside_polygon;

/// A mesh placed in the world for slicing.
#[derive(Clone, Copy)]
pub struct SectionMesh<'a> {
    pub mesh: Option<&'a Mesh>,
    pub world: DMat4,
}

impl<'a> SectionMesh<'a> {
    pub fn new(mesh: &'a Mesh, world: DMat4) -> Self {
        Self { mesh: Some(mesh), world }
    }
}

/// One closed loop of a planar cut, in world space.
#[derive(Clone, Debug, Default)]
pub struct SectionContour {
    pub points: Vec<DVec3>,
    /// Unsigned.
    pub area: f64,
    pub centroid: DVec3,
    /// The cut ran off a mesh boundary or between meshes and was closed across the gap.
    pub bridged: bool,
}

/// One chain of a planar cut, world space: a closed loop, or an open run from a mesh boundary or junction to another.
#[derive(Clone, Debug, Default)]
pub struct SectionChain {
    pub points: Vec<DVec3>,
    pub closed: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SectionPick {
    /// Area centroid of the chosen loop, on the cutting plane.
    pub center: DVec3,
    pub area: f64,
    /// The loop contains the point; otherwise it is the loop nearest to it.
    pub inside: bool,
}

struct Basis {
    n: DVec3,
    u: DVec3,
    v: DVec3,
}

fn plane_basis(normal: DVec3) -> Basis {
    let n = normal.normalize();
    let r = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    let u = n.cross(r).normalize();
    Basis { n, u, v: n.cross(u) }
}

fn project(b: &Basis, origin: DVec3, p: DVec3) -> DVec2 {
    let d = p - origin;
    DVec2::new(d.dot(b.u), d.dot(b.v))
}

fn cross2(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

fn distance_to(poly: &[DVec2], q: DVec2) -> f64 {
    let mut best = f64::MAX;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[j];
        let ab = poly[i] - a;
        let len2 = ab.dot(ab);
        let t = if len2 > 0.0 { ((q - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        best = best.min((a + ab * t - q).length());
        j = i;
    }
    best
}

/// The cut as a graph: one vertex per crossed mesh edge, one edge per face the plane passes through.
#[derive(Default)]
struct CutGraph {
    pos: Vec<DVec3>,
    edges: Vec<(u32, u32)>,
    adj: Vec<Vec<u32>>,
    keys: HashMap<u64, u32>,
}

impl CutGraph {
    fn vertex(&mut self, key: u64, p: DVec3) -> u32 {
        let next = self.pos.len() as u32;
        let id = *self.keys.entry(key).or_insert(next);
        if id == next {
            self.pos.push(p);
            self.adj.push(Vec::new());
        }
        id
    }

    fn connect(&mut self, a: u32, b: u32) {
        if a == b {
            return;
        }
        let id = self.edges.len() as u32;
        self.adj[a as usize].push(id);
        self.adj[b as usize].push(id);
        self.edges.push((a, b));
    }
}

fn cut_mesh(g: &mut CutGraph, mesh_index: u32, sm: &SectionMesh, origin: DVec3, n: DVec3) {
    let Some(m) = sm.mesh else { return };
    let pc = m.point_capacity() as usize;
    let mut wp = vec![DVec3::ZERO; pc];
    let mut dist = vec![0.0; pc];
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            wp[p as usize] = sm.world.transform_point3(m.position(p));
            dist[p as usize] = (wp[p as usize] - origin).dot(n);
        }
    }

    let mut hits: Vec<(u32, DVec3)> = Vec::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        hits.clear();
        m.for_each_face_corner(f, |c| {
            let a = m.corner_point(c) as usize;
            let b = m.corner_next_point(c) as usize;
            // On-plane points count as in front, so every crossing lies on an edge with a strict sign change.
            if (dist[a] >= 0.0) == (dist[b] >= 0.0) {
                return;
            }
            let t = dist[a] / (dist[a] - dist[b]);
            let p = wp[a] * (1.0 - t) + wp[b] * t;
            let key = ((mesh_index as u64) << 32) | m.corner_edge(c) as u64;
            hits.push((g.vertex(key, p), p));
        });
        if hits.len() < 2 {
            continue;
        }
        if hits.len() > 2 {
            // A concave n-gon crosses several times: its pieces are consecutive pairs along the cut line.
            let mut newell = DVec3::ZERO;
            m.for_each_face_corner(f, |c| {
                newell += wp[m.corner_point(c) as usize].cross(wp[m.corner_next_point(c) as usize]);
            });
            let line = n.cross(newell);
            if line.dot(line) > 1e-30 {
                hits.sort_by(|x, y| x.1.dot(line).total_cmp(&y.1.dot(line)));
            }
        }
        let mut i = 0;
        while i + 1 < hits.len() {
            g.connect(hits[i].0, hits[i + 1].0);
            i += 2;
        }
    }
}

type Chains = Vec<Vec<DVec3>>;

/// Splits the cut graph into chains; closed ones are loops, the rest end at a boundary or junction.
fn chains(g: &CutGraph) -> (Chains, Chains) {
    let mut closed = Vec::new();
    let mut open = Vec::new();
    let mut used = vec![false; g.edges.len()];
    let next_edge = |used: &[bool], v: u32| g.adj[v as usize].iter().copied().find(|&e| !used[e as usize]);
    let mut walk = |used: &mut Vec<bool>, start: u32| {
        let mut chain = vec![g.pos[start as usize]];
        let mut cur = start;
        let mut is_loop = false;
        while let Some(e) = next_edge(used, cur) {
            used[e as usize] = true;
            let (a, b) = g.edges[e as usize];
            cur = if a == cur { b } else { a };
            if cur == start {
                is_loop = true;
                break;
            }
            chain.push(g.pos[cur as usize]);
        }
        if is_loop {
            closed.push(chain);
        } else {
            open.push(chain);
        }
    };
    // Open ends first, so a chain is walked whole from one end rather than split in two.
    for v in 0..g.pos.len() as u32 {
        if g.adj[v as usize].len() % 2 == 1 {
            while next_edge(&used, v).is_some() {
                walk(&mut used, v);
            }
        }
    }
    for v in 0..g.pos.len() as u32 {
        while next_edge(&used, v).is_some() {
            walk(&mut used, v);
        }
    }
    (closed, open)
}

fn finish(pts: Vec<DVec3>, bridged: bool, b: &Basis, origin: DVec3) -> SectionContour {
    let mut a2 = 0.0;
    let mut sum = DVec2::ZERO;
    let mut avg = DVec2::ZERO;
    for i in 0..pts.len() {
        let p = project(b, origin, pts[i]);
        let q = project(b, origin, pts[(i + 1) % pts.len()]);
        let w = cross2(p, q);
        a2 += w;
        sum += (p + q) * w;
        avg += p;
    }
    let c2 = if f64::abs(a2) > 1e-18 { sum / (3.0 * a2) } else { avg / pts.len() as f64 };
    SectionContour { points: pts, area: a2.abs() * 0.5, centroid: origin + b.u * c2.x + b.v * c2.y, bridged }
}

fn cut_all(meshes: &[SectionMesh], origin: DVec3, b: &Basis) -> CutGraph {
    let mut g = CutGraph::default();
    for (i, m) in meshes.iter().enumerate() {
        if m.mesh.is_some() {
            cut_mesh(&mut g, i as u32, m, origin, b.n);
        }
    }
    g
}

/// Cuts the meshes with the plane through `origin` perpendicular to `normal` and chains the cut into closed loops.
/// N-gons are cut along their own plane; open and non-manifold meshes give open chains, which are joined to the
/// nearest other open end when that is closer than closing the chain on itself. Points exactly on the plane count as
/// in front.
pub fn cross_sections(meshes: &[SectionMesh], origin: DVec3, normal: DVec3) -> Vec<SectionContour> {
    let mut out = Vec::new();
    if normal.length() < 1e-12 {
        return out;
    }
    let b = plane_basis(normal);
    let g = cut_all(meshes, origin, &b);
    let (closed, mut open) = chains(&g);
    for c in closed {
        if c.len() >= 3 {
            out.push(finish(c, false, &b, origin));
        }
    }

    // Open chains join whichever other open end is nearer than their own start, else close on themselves.
    while let Some(mut cur) = open.pop() {
        loop {
            let back = *cur.last().unwrap();
            let mut best_dist = (back - cur[0]).length();
            let mut best = usize::MAX;
            let mut reversed = false;
            for (i, o) in open.iter().enumerate() {
                let df = (o[0] - back).length();
                let db = (*o.last().unwrap() - back).length();
                if df < best_dist {
                    best_dist = df;
                    best = i;
                    reversed = false;
                }
                if db < best_dist {
                    best_dist = db;
                    best = i;
                    reversed = true;
                }
            }
            if best == usize::MAX {
                break;
            }
            let mut next = open.remove(best);
            if reversed {
                next.reverse();
            }
            cur.extend(next);
        }
        if cur.len() >= 3 {
            out.push(finish(cur, true, &b, origin));
        }
    }
    out
}

/// The cut cross_sections makes, with open chains kept open rather than bridged into loops.
pub fn section_chains(meshes: &[SectionMesh], origin: DVec3, normal: DVec3) -> Vec<SectionChain> {
    let mut out = Vec::new();
    if normal.length() < 1e-12 {
        return out;
    }
    let b = plane_basis(normal);
    let g = cut_all(meshes, origin, &b);
    let (closed, open) = chains(&g);
    for c in closed {
        if c.len() >= 3 {
            out.push(SectionChain { points: c, closed: true });
        }
    }
    for c in open {
        if c.len() >= 2 {
            out.push(SectionChain { points: c, closed: false });
        }
    }
    out
}

/// Centre of the cross-section around `point` among already cut contours: the smallest loop containing the point wins
/// (the nearest loop when none does).
pub fn section_center_of(contours: &[SectionContour], point: DVec3, direction: DVec3) -> Option<SectionPick> {
    if contours.is_empty() || direction.length() < 1e-12 {
        return None;
    }
    let b = plane_basis(direction);
    let q = DVec2::ZERO;
    let mut inside: Option<&SectionContour> = None;
    let mut nearest: Option<&SectionContour> = None;
    let mut nearest_dist = f64::MAX;
    for c in contours {
        let poly: Vec<DVec2> = c.points.iter().map(|&p| project(&b, point, p)).collect();
        if inside_polygon(&poly, q) {
            // Nested loops (a sleeve over an arm, a hollow pipe) all contain it; the tightest is the one it sits in.
            if inside.map_or(true, |i| c.area < i.area) {
                inside = Some(c);
            }
        } else {
            let d = distance_to(&poly, q);
            if d < nearest_dist {
                nearest_dist = d;
                nearest = Some(c);
            }
        }
    }
    let pick = inside.or(nearest)?;
    Some(SectionPick { center: pick.centroid, area: pick.area, inside: inside.is_some() })
}

/// Centre of the cross-section around `point`: the meshes are cut by the plane through it perpendicular to
/// `direction`, and the smallest loop containing the point wins (the nearest loop when none does). None when the
/// plane misses.
pub fn section_center(meshes: &[SectionMesh], point: DVec3, direction: DVec3) -> Option<SectionPick> {
    let contours = cross_sections(meshes, point, direction);
    section_center_of(&contours, point, direction)
}
