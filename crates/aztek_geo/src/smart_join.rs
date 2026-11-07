//! Joining a limb mesh to a body mesh the way a modeller would by hand.

use std::cmp::{Ordering, Reverse};
use std::collections::{BTreeSet, BinaryHeap};

use glam::{DVec2, DVec3};

use crate::attribute::{Index, INVALID};
use crate::bvh::{BvhHit, MeshBvh};
use crate::mesh::{validate, Mesh, MeshRemap};
use crate::normals::set_soft_edges;
use crate::ops::basic::delete_faces;
use crate::ops::common::inside_polygon;
use crate::ops::loops::{insert_edge_loop, EdgeLoopOptions};
use crate::topology::{
    boundary_loops, chain_edges, edge_loop, edges_of_faces, face_components, faces_of_edges, make_mask, points_of_edges,
    points_of_faces, region_boundary_edges, EdgePath,
};

type V = DVec3;
type V2 = DVec2;

#[derive(Clone, Debug)]
pub struct SmartJoinOptions {
    /// Rows of quads in the bridge; 0 picks them from the gap and the loop's edge length.
    pub segments: i32,
    /// Scales the Hermite tangents leaving the limb and entering the body; 0 is a straight bridge.
    pub tension: f64,
    /// Rotates the loop correspondence by this many points.
    pub twist: i32,
    /// Scales the projected loop about its centre on the body.
    pub hole_scale: f64,
    /// Slides the hole across the body surface, in world units along the hole's tangent frame.
    pub offset_u: f64,
    pub offset_v: f64,
    /// Which limb loop to use: -1 auto (the nearest loop fully outside the body), else the index into the candidates
    /// nearest first.
    pub limb_loop: i32,
    /// Removes the limb beyond the loop (the part toward or inside the body); off detaches it as a separate shell.
    pub delete_buried: bool,
    /// Laplacian passes over the bridge's inner rows and the hole ring.
    pub relax: i32,
    /// Where a trim loop goes past the intersection, as a fraction of the limb's radius.
    pub trim_margin: f64,
}

impl Default for SmartJoinOptions {
    fn default() -> Self {
        Self {
            segments: 0,
            tension: 1.0,
            twist: 0,
            hole_scale: 1.0,
            offset_u: 0.0,
            offset_v: 0.0,
            limb_loop: -1,
            delete_buried: true,
            relax: 0,
            trim_margin: 0.25,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct SmartJoinResult {
    pub ok: bool,
    pub mesh: Mesh,
    /// 0 to 100.
    pub confidence: i32,
    /// Why the confidence is what it is, most important first; the error when not ok.
    pub reasons: Vec<String>,
    /// One line, e.g. "matching 8-point loops, square-on, clean: 98%".
    pub summary: String,
    pub limb_loop_size: i32,
    pub hole_size: i32,
    /// Among the new faces.
    pub triangles: i32,
    /// Among the new faces.
    pub ngons: i32,
    pub support_loops: i32,
    pub segments: i32,
    pub loop_index: i32,
    pub loop_candidates: i32,
    pub trimmed: bool,
    pub flipped_limb: bool,
    /// Input points kept unchanged.
    pub preserved_points: i32,
    pub original_points: i32,
}

/// Plane frame with u x v = n.
#[derive(Clone, Copy)]
struct Frame {
    o: V,
    u: V,
    v: V,
    n: V,
}

impl Frame {
    fn to2(&self, p: V) -> V2 {
        let d = p - self.o;
        V2::new(d.dot(self.u), d.dot(self.v))
    }
}

fn frame_around(o: V, normal: V) -> Frame {
    let n = normal.normalize();
    let a = if n.x.abs() < 0.9 { V::X } else { V::Y };
    let u = a.cross(n).normalize();
    Frame { o, u, v: n.cross(u), n }
}

fn signed_area(p: &[V2]) -> f64 {
    let mut a = 0.0;
    for i in 0..p.len() {
        let x = p[i];
        let y = p[(i + 1) % p.len()];
        a += x.x * y.y - y.x * x.y;
    }
    0.5 * a
}

fn sense(f: &Frame, m: &Mesh, pts: &[Index]) -> i32 {
    let p: Vec<V2> = pts.iter().map(|&i| f.to2(m.position(i))).collect();
    if signed_area(&p) >= 0.0 {
        1
    } else {
        -1
    }
}

fn sense_of(f: &Frame, pts: &[V]) -> i32 {
    let p: Vec<V2> = pts.iter().map(|&x| f.to2(x)).collect();
    if signed_area(&p) >= 0.0 {
        1
    } else {
        -1
    }
}

fn distance_to_polygon(poly: &[V2], q: V2) -> f64 {
    let mut best = f64::MAX;
    for i in 0..poly.len() {
        let a = poly[i];
        let ab = poly[(i + 1) % poly.len()] - a;
        let len2 = ab.dot(ab);
        let t = if len2 > 0.0 { ((q - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        best = best.min((q - (a + ab * t)).length());
    }
    best
}

/// Distance from the origin along unit dir to the polygon, 0 if the ray misses it.
fn border_distance(poly: &[V2], dir: V2) -> f64 {
    let mut best = 0.0;
    for i in 0..poly.len() {
        let a = poly[i];
        let e = poly[(i + 1) % poly.len()] - a;
        let den = dir.x * e.y - dir.y * e.x;
        if den.abs() < 1e-15 {
            continue;
        }
        let t = (a.x * e.y - a.y * e.x) / den;
        let u = (a.x * dir.y - a.y * dir.x) / den;
        if t > 0.0 && (0.0..=1.0).contains(&u) && (best == 0.0 || t < best) {
            best = t;
        }
    }
    best
}

/// Ray parity, majority of three skewed directions, so a ray grazing an edge cannot decide alone.
fn inside_solid(bvh: &MeshBvh, p: V, eps: f64) -> bool {
    let dirs = [
        V::new(0.577, 0.613, 0.539).normalize(),
        V::new(-0.61, 0.37, -0.70).normalize(),
        V::new(0.29, -0.83, 0.47).normalize(),
    ];
    let mut votes = 0;
    for d in dirs {
        let mut hits = 0;
        let mut o = p;
        for _ in 0..256 {
            let Some(h) = bvh.raycast(o, d) else { break };
            hits += 1;
            o += d * (h.t + eps);
        }
        votes += hits & 1;
    }
    votes >= 2
}

#[derive(Clone, Default)]
struct Candidate {
    points: Vec<Index>,
    edges: Vec<Index>,
    remove_faces: Vec<Index>,
    #[allow(dead_code)]
    face_border: bool,
    centroid: V,
    axis: V,
    radius: f64,
    gap: f64,
    planarity: f64,
}

/// From outside the ray must arrive at the body's front.
fn hit_body(body: &Mesh, bvh: &MeshBvh, o: V, d: V) -> Option<BvhHit> {
    bvh.raycast(o, d).filter(|h| body.face_normal(h.face).dot(d) < 0.0)
}

fn gather_candidates(limb: &Mesh, body: &Mesh, bvh: &MeshBvh, eps: f64, face_borders: bool) -> Vec<Candidate> {
    let inside: Vec<bool> =
        (0..limb.point_capacity()).map(|p| limb.point_valid(p) && inside_solid(bvh, limb.position(p), eps)).collect();

    let mut out: Vec<Candidate> = Vec::new();
    let mut seen: BTreeSet<Vec<Index>> = BTreeSet::new();
    let mut add = |path: &EdgePath, face_border: bool| {
        if !path.closed || path.points.len() < 3 {
            return;
        }
        let mut key = path.edges.clone();
        key.sort_unstable();
        if !seen.insert(key) {
            return;
        }
        let mut c = Candidate { points: path.points.clone(), edges: path.edges.clone(), face_border, ..Default::default() };
        let mut nl = V::ZERO;
        for i in 0..c.points.len() {
            let a = limb.position(c.points[i]);
            let b = limb.position(c.points[(i + 1) % c.points.len()]);
            c.centroid += a;
            nl += a.cross(b);
            if inside[c.points[i] as usize] {
                return;
            }
        }
        c.centroid /= c.points.len() as f64;
        if nl.length() < 1e-12 {
            return;
        }
        let nl = nl.normalize();
        for &p in &c.points {
            c.radius += (limb.position(p) - c.centroid).length();
        }
        c.radius /= c.points.len() as f64;
        for &p in &c.points {
            c.planarity = c.planarity.max((limb.position(p) - c.centroid).dot(nl).abs());
        }
        c.planarity /= c.radius.max(1e-12);

        let boundary = c.edges.iter().all(|&e| limb.is_boundary_edge(e));
        let mean_of = |faces: &[Index]| {
            let pts = points_of_faces(limb, faces);
            if pts.is_empty() {
                return c.centroid;
            }
            pts.iter().fold(V::ZERO, |m, &p| m + limb.position(p)) / pts.len() as f64
        };
        if boundary {
            let k = mean_of(&faces_of_edges(limb, &c.edges));
            c.axis = if nl.dot(c.centroid - k) >= 0.0 { nl } else { -nl };
            let Some(h) = hit_body(body, bvh, c.centroid, c.axis) else { return };
            c.gap = h.t;
        } else {
            let block = make_mask(limb.edge_capacity() as usize, &c.edges);
            let mut comp = Vec::new();
            face_components(limb, &mut comp, &block);
            let edge_faces = faces_of_edges(limb, &c.edges);
            let sides: BTreeSet<Index> = edge_faces.iter().map(|&f| comp[f as usize]).collect();
            if sides.len() != 2 {
                return;
            }
            let mut best = f64::MAX;
            let mut best_side = INVALID;
            let mut axis = c.axis;
            for &s in &sides {
                let adj: Vec<Index> = edge_faces.iter().copied().filter(|&f| comp[f as usize] == s).collect();
                let d = if (mean_of(&adj) - c.centroid).dot(nl) >= 0.0 { nl } else { -nl };
                if let Some(h) = hit_body(body, bvh, c.centroid, d) {
                    if h.t < best {
                        best = h.t;
                        best_side = s;
                        axis = d;
                    }
                }
            }
            if best_side == INVALID {
                return;
            }
            c.axis = axis;
            c.gap = best;
            c.remove_faces =
                (0..limb.face_capacity()).filter(|&f| limb.face_valid(f) && comp[f as usize] == best_side).collect();
        }
        out.push(c);
    };

    let mut loop_seen = vec![false; limb.edge_capacity() as usize];
    for p in boundary_loops(limb) {
        add(&p, false);
    }
    for e in 0..limb.edge_capacity() {
        if !limb.edge_valid(e) || loop_seen[e as usize] {
            continue;
        }
        let lp = edge_loop(limb, e);
        for &x in &lp {
            loop_seen[x as usize] = true;
        }
        let paths = chain_edges(limb, &lp);
        if paths.len() == 1 {
            add(&paths[0], false);
        }
    }
    for f in 0..limb.face_capacity() {
        if !limb.face_valid(f) || (!face_borders && limb.face_size(f) < 5) {
            continue;
        }
        let mut path = EdgePath { closed: true, ..Default::default() };
        limb.for_each_face_corner(f, |c| {
            path.points.push(limb.corner_point(c));
            path.edges.push(limb.corner_edge(c));
        });
        add(&path, limb.face_size(f) < 5);
    }
    out.sort_by(|a, b| a.gap.partial_cmp(&b.gap).unwrap_or(Ordering::Equal));
    out
}

/// Inserts an edge loop across the ring of limb edges that pierce the body, just outside the surface. Returns the new
/// loop's point count, 0 when the limb does not cross the body along one ring.
fn trim_at_body(limb: &mut Mesh, bvh: &MeshBvh, eps: f64, margin_frac: f64) -> i32 {
    let inside: Vec<bool> =
        (0..limb.point_capacity()).map(|p| limb.point_valid(p) && inside_solid(bvh, limb.position(p), eps)).collect();
    let crossing: Vec<Index> = (0..limb.edge_capacity())
        .filter(|&e| limb.edge_valid(e) && inside[limb.edge_point(e, 0) as usize] != inside[limb.edge_point(e, 1) as usize])
        .collect();
    if crossing.is_empty() {
        return 0;
    }
    let mid_of = |e: Index| 0.5 * (limb.position(limb.edge_point(e, 0)) + limb.position(limb.edge_point(e, 1)));
    let mid = crossing.iter().fold(V::ZERO, |a, &e| a + mid_of(e)) / crossing.len() as f64;
    let radius = crossing.iter().map(|&e| (mid_of(e) - mid).length()).sum::<f64>() / crossing.len() as f64;

    // The crossing edge reaching furthest out decides, so every ring edge's cut lands outside.
    for &e0 in &crossing {
        let first_outside = !inside[limb.edge_point(e0, 0) as usize];
        let out = limb.position(limb.edge_point(e0, if first_outside { 0 } else { 1 }));
        let inn = limb.position(limb.edge_point(e0, if first_outside { 1 } else { 0 }));
        let len = (inn - out).length();
        if len <= 0.0 {
            continue;
        }
        let d = bvh.raycast_with(out, inn - out, 1.0, true).map_or(0.5 * len, |h| h.t * len);
        let mut s = (d - margin_frac * radius).max(0.05 * d) / len;
        for _ in 0..6 {
            let mut copy = limb.clone();
            let edges = insert_edge_loop(
                &mut copy,
                e0,
                &EdgeLoopOptions { fraction: if first_outside { s } else { 1.0 - s }, ..Default::default() },
            );
            if edges.is_empty() {
                break;
            }
            let pts = points_of_edges(&copy, &edges);
            if !pts.iter().any(|&p| inside_solid(bvh, copy.position(p), eps)) {
                *limb = copy;
                return pts.len() as i32;
            }
            s *= 0.6;
        }
    }
    0
}

#[derive(Clone, Default)]
struct Region {
    faces: Vec<Index>,
    /// Ordered points of its outer border.
    border: Vec<Index>,
    max_r: f64,
    enclosed: bool,
    poles: i32,
    non_quads: i32,
}

fn eval_region(m: &Mesh, faces: Vec<Index>, fr: &Frame, hole: &[V2], hole_r: f64) -> Option<Region> {
    let border = region_boundary_edges(m, &faces);
    if border.iter().any(|&e| m.edge_face_count(e) != 2) {
        return None;
    }
    let paths = chain_edges(m, &border);
    if paths.len() != 1 || !paths[0].closed {
        return None;
    }
    let pts = points_of_faces(m, &faces);
    let edges = edges_of_faces(m, &faces);
    if pts.len() as i64 - edges.len() as i64 + faces.len() as i64 != 1 {
        return None;
    }
    let mut out = Region { faces, border: paths[0].points.clone(), ..Default::default() };
    let mut poly = Vec::new();
    for &p in &out.border {
        poly.push(fr.to2(m.position(p)));
        out.max_r = out.max_r.max(poly.last().unwrap().length());
    }
    out.enclosed = hole.iter().all(|&q| inside_polygon(&poly, q) && distance_to_polygon(&poly, q) >= 0.03 * hole_r);
    out.poles = pts.iter().filter(|&&p| m.point_valence(p) != 4).count() as i32;
    out.non_quads = out.faces.iter().filter(|&&f| m.face_size(f) != 4).count() as i32;
    Some(out)
}

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

/// Faces grown from f0 nearest first; returns the first disc whose border count is n and that holds the hole, else
/// the closest fallback seen.
fn grow_region(m: &Mesh, f0: Index, fr: &Frame, hole: &[V2], hole_r: f64, n: i32) -> (Option<Region>, Option<Region>) {
    let key = |f: Index| fr.to2(m.face_centroid(f)).length();
    let mut frontier = BinaryHeap::new();
    let mut queued = vec![false; m.face_capacity() as usize];
    let mut faces: Vec<Index> = Vec::new();
    frontier.push(Reverse(Item(key(f0), f0)));
    queued[f0 as usize] = true;
    let mut fallback_diff = i32::MAX;
    let mut fallback = None;
    while faces.len() < 80 {
        let Some(Reverse(Item(k, f))) = frontier.pop() else { break };
        if k > 3.0 * hole_r && !faces.is_empty() {
            break;
        }
        faces.push(f);
        m.for_each_face_corner(f, |c| {
            m.for_each_edge_corner(m.corner_edge(c), |r| {
                let g = m.corner_face(r);
                if !queued[g as usize] {
                    queued[g as usize] = true;
                    frontier.push(Reverse(Item(key(g), g)));
                }
            });
        });
        let Some(r) = eval_region(m, faces.clone(), fr, hole, hole_r) else { continue };
        if !r.enclosed {
            continue;
        }
        let count = r.border.len() as i32;
        if count == n && r.max_r <= 2.2 * hole_r {
            return (Some(r), fallback);
        }
        let diff = (count - n).abs();
        if r.max_r <= 3.0 * hole_r && diff < fallback_diff {
            fallback_diff = diff;
            fallback = Some(r);
        }
    }
    (None, fallback)
}

/// Closest face to p.
fn face_at(bvh: &MeshBvh, p: V) -> Index {
    bvh.closest_point(p).map_or(INVALID, |c| c.face)
}

/// Edge positions along dir of body edges running along other near the centre, sorted.
fn existing_lines(m: &Mesh, center: V, dir: V, other: V, reach: f64, other_reach: f64) -> Vec<f64> {
    let mut out = Vec::new();
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) {
            continue;
        }
        let p0 = m.position(m.edge_point(e, 0));
        let p1 = m.position(m.edge_point(e, 1));
        let d = p1 - p0;
        if d.length() <= 0.0 || d.normalize().dot(other).abs() < 0.8 {
            continue;
        }
        let mid = 0.5 * (p0 + p1) - center;
        if mid.dot(dir).abs() < reach && mid.dot(other).abs() < other_reach + 0.5 * d.length() {
            out.push(mid.dot(dir));
        }
    }
    out.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    out
}

/// Line positions for one grid axis: extents just outside the hole (an existing edge there is reused), existing edges
/// between them, and new lines splitting the widest gaps until there are segs + 1.
fn plan_lines(existing: &[f64], hole: f64, segs: i32) -> Vec<f64> {
    let mut lo = -1.3 * hole;
    let mut hi = 1.3 * hole;
    if let Some(&c) = existing.iter().find(|&&c| c >= 0.75 * hole && c <= 1.7 * hole) {
        hi = c;
    }
    if let Some(&c) = existing.iter().rev().find(|&&c| c <= -0.75 * hole && c >= -1.7 * hole) {
        lo = c;
    }
    let tol = 0.08 * (hi - lo);
    let mut lines = vec![lo];
    for &c in existing {
        if c > lo + tol && c < hi - tol && (lines.len() == 1 || c - lines.last().unwrap() > tol) {
            lines.push(c);
        }
    }
    lines.push(hi);
    while (lines.len() as i32) < segs + 1 {
        let mut widest = 0;
        for i in 1..lines.len() - 1 {
            if lines[i + 1] - lines[i] > lines[widest + 1] - lines[widest] {
                widest = i;
            }
        }
        let mid = 0.5 * (lines[widest] + lines[widest + 1]);
        lines.insert(widest + 1, mid);
    }
    lines
}

/// Inserts an edge loop through the body at center + dir * x, cutting the edges that run along dir. False if not
/// needed.
fn insert_line(m: &mut Mesh, center: V, dir: V, normal: V, x: f64) -> bool {
    let bvh = MeshBvh::new(m);
    // Straight down onto the surface, so a curved body is cut where the hole's frame says, not at the nearest facet.
    let p = center + dir * x;
    let lift = 4.0 * x.abs() + 1.0;
    let (point, face) = if let Some(h) = bvh
        .raycast_with(p + normal * lift, -normal, 2.0 * lift, true)
        .or_else(|| bvh.raycast_with(p - normal * lift, normal, 2.0 * lift, true))
    {
        (h.point, h.face)
    } else if let Some(c) = bvh.closest_point(p) {
        (c.point, c.face)
    } else {
        return false;
    };
    if m.face_size(face) != 4 {
        return false;
    }
    let mut best = INVALID;
    let mut best_dot = 0.0;
    m.for_each_face_corner(face, |k| {
        let e = m.corner_edge(k);
        let d = m.position(m.edge_point(e, 1)) - m.position(m.edge_point(e, 0));
        let dd = d.normalize().dot(dir).abs();
        if dd > best_dot {
            best_dot = dd;
            best = e;
        }
    });
    if best == INVALID {
        return false;
    }
    let p0 = m.position(m.edge_point(best, 0));
    let e = m.position(m.edge_point(best, 1)) - p0;
    let t = (point - p0).dot(e) / e.dot(e);
    // An existing edge this close already serves as the line.
    if !(0.08..=0.92).contains(&t) {
        return false;
    }
    !insert_edge_loop(m, best, &EdgeLoopOptions { fraction: t, ..Default::default() }).is_empty()
}

/// Support loops through the body so the faces around the hole form an a x b quad block (2(a+b) border points).
/// Returns the loops inserted and the lines the block spans.
#[allow(clippy::too_many_arguments)]
fn insert_support_grid(m: &mut Mesh, center: V, da: V, db: V, hole_a: f64, hole_b: f64, a: i32, b: i32) -> (i32, Vec<f64>, Vec<f64>) {
    let have_a = existing_lines(m, center, da, db, 2.0 * hole_a, 1.7 * hole_b);
    let have_b = existing_lines(m, center, db, da, 2.0 * hole_b, 1.7 * hole_a);
    let lines_a = plan_lines(&have_a, hole_a, a);
    let lines_b = plan_lines(&have_b, hole_b, b);
    let normal = da.cross(db).normalize();
    let mut inserted = 0;
    for &x in &lines_a {
        if !have_a.contains(&x) {
            inserted += insert_line(m, center, da, normal, x) as i32;
        }
    }
    for &y in &lines_b {
        if !have_b.contains(&y) {
            inserted += insert_line(m, center, db, normal, y) as i32;
        }
    }
    (inserted, lines_a, lines_b)
}

/// Quads between a hole border (fill order) and an inner loop running the same way, triangles where counts differ.
fn stitch(m: &mut Mesh, outer: &[Index], inner: &[Index]) -> Vec<Index> {
    let mut faces = Vec::new();
    let (mm, n) = (outer.len(), inner.len());
    let mut add = |m: &mut Mesh, pts: &[Index]| {
        let f = m.add_face(pts);
        if f != INVALID {
            faces.push(f);
        }
    };
    // The longer side advances every step; the shorter one evenly often, so the extra steps become spread triangles.
    if mm >= n {
        for k in 0..mm {
            let j = k * n / mm;
            let next = (k + 1) * n / mm;
            if next > j {
                add(m, &[outer[k], outer[(k + 1) % mm], inner[next % n], inner[j % n]]);
            } else {
                add(m, &[outer[k], outer[(k + 1) % mm], inner[j % n]]);
            }
        }
    } else {
        for k in 0..n {
            let i = k * mm / n;
            let next = (k + 1) * mm / n;
            if next > i {
                add(m, &[outer[i % mm], outer[next % mm], inner[(k + 1) % n], inner[k]]);
            } else {
                add(m, &[outer[i % mm], inner[(k + 1) % n], inner[k]]);
            }
        }
    }
    faces
}

fn hermite(p0: V, m0: V, p1: V, m1: V, t: f64) -> V {
    let t2 = t * t;
    let t3 = t2 * t;
    (2.0 * t3 - 3.0 * t2 + 1.0) * p0 + (t3 - 2.0 * t2 + t) * m0 + (-2.0 * t3 + 3.0 * t2) * p1 + (t3 - t2) * m1
}

fn mix(a: V, b: V, t: f64) -> V {
    a * (1.0 - t) + b * t
}

fn fail(why: &str) -> SmartJoinResult {
    SmartJoinResult { reasons: vec![why.to_string()], summary: why.to_string(), ..Default::default() }
}

/// Joins a limb mesh to a body mesh (both in the same space) the way a modeller would by hand, touching only a local
/// region of each.
///
/// Approach: pick a closed loop on the limb (open border, edge loop, n-gon border, or a new loop inserted just outside
/// where the limb enters the body), ranked by distance to the body along the loop's axis. Project that loop onto the
/// body along the axis to get the hole's shape. Carve a disc of body faces around the hit whose border has as many
/// points as the loop: first by growing a region over the existing faces, otherwise by inserting support edge loops
/// that make an a x b quad block with 2(a+b) border points (the circularize-a-quad-block workflow). The region is
/// replaced by one quad ring from its border to the projected loop, and the limb loop is bridged to that ring with
/// rows of quads along a Hermite curve that leaves the limb along its rails and meets the body along its normal.
/// Counts that cannot match (an odd loop) are stitched with the fewest triangles.
///
/// The confidence drops for count mismatches, grazing angles, poles or n-gons in the hole region, holes the region
/// cannot contain, projection misses, non-planar loops and long bridges.
pub fn smart_join(limb_in: &Mesh, body_in: &Mesh, opts: &SmartJoinOptions) -> SmartJoinResult {
    let mut limb = limb_in.clone();
    let mut body = body_in.clone();
    limb.compact(None);
    body.compact(None);
    if limb.num_faces() == 0 || body.num_faces() == 0 {
        return fail("both meshes need faces");
    }

    let mut res = SmartJoinResult { original_points: (limb.num_points() + body.num_points()) as i32, ..Default::default() };
    let limb_cap = limb.point_capacity();
    let mut penalties: Vec<(f64, String)> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    let body_bvh = MeshBvh::new(&body);
    let scale = body.bounds().size().length() + limb.bounds().size().length();
    let eps = 1e-7 * scale.max(1.0);

    // Limb: choose the loop, trimming the limb at the body first when it pierces it and no loop sits close outside.
    let mut cands = gather_candidates(&limb, &body, &body_bvh, eps, false);
    {
        let pierces =
            (0..limb.point_capacity()).any(|p| limb.point_valid(p) && inside_solid(&body_bvh, limb.position(p), eps));
        if pierces
            && opts.limb_loop < 0
            && (cands.is_empty() || cands[0].gap > 0.6 * cands[0].radius)
            && trim_at_body(&mut limb, &body_bvh, eps, opts.trim_margin) > 0
        {
            res.trimmed = true;
            cands = gather_candidates(&limb, &body, &body_bvh, eps, false);
        }
    }
    let mut face_border_loop = false;
    if cands.is_empty() {
        cands = gather_candidates(&limb, &body, &body_bvh, eps, true);
        face_border_loop = !cands.is_empty();
    }
    if cands.is_empty() {
        return fail("no loop on the limb faces the body (is the limb pointing at it?)");
    }
    res.loop_candidates = cands.len() as i32;
    let pick = if opts.limb_loop < 0 { 0 } else { opts.limb_loop.min(cands.len() as i32 - 1) };
    res.loop_index = pick;
    let cand = cands[pick as usize].clone();
    if opts.limb_loop >= cands.len() as i32 {
        notes.push(format!("only {} limb loops face the body; used the last", cands.len()));
    }

    // Remove (or detach) the limb beyond the loop, then read the loop back as a border in fill order.
    if !cand.remove_faces.is_empty() {
        if opts.delete_buried {
            delete_faces(&mut limb, &cand.remove_faces, None);
        } else {
            let mut dup = vec![INVALID; limb.point_capacity() as usize];
            for &p in &cand.points {
                dup[p as usize] = limb.add_point(limb.position(p));
            }
            for &f in &cand.remove_faces {
                let pts: Vec<Index> = limb
                    .face_points(f)
                    .iter()
                    .map(|&p| if (p as usize) < dup.len() && dup[p as usize] != INVALID { dup[p as usize] } else { p })
                    .collect();
                limb.set_face_points(f, &pts);
            }
            // Edges that joined the loop to the detached side are left without faces.
            limb.remove_all_loose();
        }
    }
    let mut a_loop: Vec<Index> = Vec::new();
    {
        let want: BTreeSet<Index> = cand.points.iter().copied().collect();
        for p in boundary_loops(&limb) {
            if p.closed && p.points.len() == want.len() && p.points.iter().all(|x| want.contains(x)) {
                a_loop = p.points;
            }
        }
    }
    if a_loop.is_empty() {
        return fail("the limb loop does not become an open border (non-manifold limb?)");
    }
    let limb_preserved = (0..limb_cap).filter(|&p| limb.point_valid(p)).count() as i32;
    {
        let mut remap = MeshRemap::default();
        limb.compact(Some(&mut remap));
        for p in &mut a_loop {
            *p = remap.points[*p as usize];
        }
    }
    let n = a_loop.len() as i32;
    let nu = a_loop.len();
    res.limb_loop_size = n;
    let axis = cand.axis;

    // Rails: the limb edge leaving each loop point, carried on toward the body.
    let mut rails = vec![axis; nu];
    {
        let loop_pts: BTreeSet<Index> = a_loop.iter().copied().collect();
        for k in 0..nu {
            let mut best = -2.0;
            let ak = a_loop[k];
            limb.for_each_point_edge(ak, |e| {
                let q = limb.edge_other_point(e, ak);
                if loop_pts.contains(&q) {
                    return;
                }
                let d = limb.position(ak) - limb.position(q);
                if d.length() <= 0.0 {
                    return;
                }
                let dn = d.normalize();
                if dn.dot(axis) > best {
                    best = dn.dot(axis);
                    rails[k] = dn;
                }
            });
            if best < 0.0 {
                rails[k] = axis;
            }
        }
    }

    // Project the loop onto the body along the axis, then scale and slide it about its centre.
    let loop_centre = a_loop.iter().fold(V::ZERO, |s, &p| s + limb.position(p)) / n as f64;
    let (centre_point, centre_face) = match body_bvh.raycast(loop_centre, axis) {
        Some(h) => (h.point, h.face),
        None => {
            let c = body_bvh.closest_point(loop_centre).unwrap_or_default();
            (c.point, c.face)
        }
    };
    let body_normal = body.face_normal(centre_face);
    let cos_angle = axis.dot(-body_normal).clamp(-1.0, 1.0);
    let mut tangent = frame_around(centre_point, body_normal);

    let mut q_pts = vec![V::ZERO; nu];
    let mut misses = 0;
    for k in 0..nu {
        let p = limb.position(a_loop[k]);
        match body_bvh.raycast(p, axis) {
            Some(h) if h.t < cand.gap + 4.0 * cand.radius => q_pts[k] = h.point,
            _ => {
                q_pts[k] = body_bvh.closest_point(p).unwrap_or_default().point;
                misses += 1;
            }
        }
    }
    let mut hole_centre = centre_point + tangent.u * opts.offset_u + tangent.v * opts.offset_v;
    if let Some(c) = body_bvh.closest_point(hole_centre) {
        hole_centre = c.point;
    }
    let mut body_normals = vec![body_normal; nu];
    for k in 0..nu {
        let mut q = hole_centre + (q_pts[k] - centre_point) * opts.hole_scale.max(0.01);
        if let Some(c) = body_bvh.closest_point(q) {
            q = c.point;
            body_normals[k] = body.face_normal(c.face);
        }
        q_pts[k] = q;
    }
    {
        // Averaged over the hole so a hit on an edge or a ridge between facets does not tilt the frame.
        let mut avg = V::ZERO;
        let reach = 1.5 * cand.radius * opts.hole_scale.max(0.01);
        for f in 0..body.face_capacity() {
            if body.face_valid(f) && (body.face_centroid(f) - hole_centre).length() < reach {
                avg += body.face_area_vector(f);
            }
        }
        let nrm = if avg.length() > 0.0 { avg.normalize() } else { body.face_normal(face_at(&body_bvh, hole_centre)) };
        tangent = frame_around(hole_centre, nrm);
    }
    let mut hole2: Vec<V2> = Vec::new();
    let mut hole_r = 0.0f64;
    for &q in &q_pts {
        hole2.push(tangent.to2(q));
        hole_r = hole_r.max(hole2.last().unwrap().length());
    }
    if hole_r <= eps {
        return fail("the loop projects to a point on the body");
    }

    // Body: find or make a disc of faces with as many border points as the loop.
    let body_cap = body.point_capacity();
    let mut region: Region;
    {
        let bvh = MeshBvh::new(&body);
        let f0 = face_at(&bvh, hole_centre);
        if f0 == INVALID {
            return fail("the body around the hole is not a simple surface");
        }
        let (exact, fallback) = grow_region(&body, f0, &tangent, &hole2, hole_r, n);
        region = Region::default();
        if let Some(r) = exact {
            region = r;
        } else if body.face_size(f0) == 4 {
            // Grid axes follow the hit face's own edge flow.
            let c0 = body.face_corner(f0);
            let edge_dir = |c: Index| {
                let mut d = body.position(body.corner_next_point(c)) - body.position(body.corner_point(c));
                d -= tangent.n * d.dot(tangent.n);
                d.normalize()
            };
            let da = edge_dir(c0);
            let db = edge_dir(body.corner_next(c0));
            let (mut ea, mut eb) = (0.0f64, 0.0f64);
            for &q in &q_pts {
                ea = ea.max((q - hole_centre).dot(da).abs());
                eb = eb.max((q - hole_centre).dot(db).abs());
            }
            let half = (n + (n & 1)) / 2;
            let a = ((half as f64 * ea / (ea + eb).max(1e-12)).round() as i32).clamp(1, (half - 1).max(1));
            let b = (half - a).max(1);
            let mut trial = body.clone();
            let (loops, lines_a, lines_b) = insert_support_grid(&mut trial, hole_centre, da, db, ea, eb, a, b);
            let tb = MeshBvh::new(&trial);
            let g0 = face_at(&tb, hole_centre);
            let mut block = Vec::new();
            let mut seen = vec![false; trial.face_capacity() as usize];
            let in_block = |f: Index| {
                let d = trial.face_centroid(f) - hole_centre;
                let x = d.dot(da);
                let y = d.dot(db);
                x > lines_a[0] && x < *lines_a.last().unwrap() && y > lines_b[0] && y < *lines_b.last().unwrap()
            };
            let mut stack = vec![g0];
            seen[g0 as usize] = true;
            while let Some(f) = stack.pop() {
                block.push(f);
                trial.for_each_face_corner(f, |c| {
                    trial.for_each_edge_corner(trial.corner_edge(c), |r| {
                        let g = trial.corner_face(r);
                        if !seen[g as usize] && in_block(g) {
                            seen[g as usize] = true;
                            stack.push(g);
                        }
                    });
                });
            }
            let limit = fallback.as_ref().map_or(1 << 20, |f| (f.border.len() as i32 - n).abs());
            match eval_region(&trial, block, &tangent, &hole2, hole_r) {
                Some(r) if (r.border.len() as i32 - n).abs() < limit => {
                    body = trial;
                    region = r;
                    res.support_loops = loops;
                }
                _ => {
                    if let Some(f) = fallback {
                        region = f;
                    }
                }
            }
        } else if let Some(f) = fallback {
            region = f;
        }
        if region.faces.is_empty() {
            match eval_region(&body, vec![f0], &tangent, &hole2, hole_r) {
                Some(r) => region = r,
                None => return fail("the body around the hole is not a simple surface"),
            }
        }
    }
    let m_count = region.border.len() as i32;

    // A hole wider than the faces around it is pulled in to sit inside their border.
    let mut shrunk = false;
    if !region.enclosed {
        let poly: Vec<V2> = region.border.iter().map(|&p| tangent.to2(body.position(p))).collect();
        for k in 0..nu {
            let q = hole2[k];
            let len = q.length();
            if len <= 0.0 {
                continue;
            }
            let reach = border_distance(&poly, q / len);
            if reach > 0.0 && len > 0.85 * reach {
                let p = hole_centre + (q_pts[k] - hole_centre) * (0.85 * reach / len);
                q_pts[k] = body_bvh.closest_point(p).map_or(p, |c| c.point);
                hole2[k] = tangent.to2(q_pts[k]);
                shrunk = true;
            }
        }
        region.enclosed = hole2.iter().all(|&q| inside_polygon(&poly, q));
    }
    res.hole_size = m_count;

    // Carve the region and read its border back in fill order.
    let border_start = region.border[0];
    delete_faces(&mut body, &region.faces, None);
    let mut o_loop: Vec<Index> = Vec::new();
    for p in boundary_loops(&body) {
        if p.closed && p.points.contains(&border_start) {
            o_loop = p.points;
        }
    }
    if o_loop.len() as i32 != m_count {
        return fail("carving the hole left an unexpected border");
    }

    // Orientation: the limb border and the hole border must circle the axis in opposite senses.
    let axial = frame_around(hole_centre, axis);
    let sigma_o = sense(&axial, &body, &o_loop);
    if sense(&axial, &limb, &a_loop) == sigma_o {
        for f in 0..limb.face_capacity() {
            if limb.face_valid(f) {
                limb.flip_face(f);
            }
        }
        a_loop.reverse();
        rails.reverse();
        q_pts.reverse();
        body_normals.reverse();
        res.flipped_limb = true;
    }
    let sigma_a = -sigma_o;
    let sigma_i = sense_of(&axial, &q_pts);

    let mut i_loop: Vec<Index> = q_pts.iter().map(|&q| body.add_point(q)).collect();
    let mut j_loop = i_loop.clone();
    if sigma_i != sigma_o {
        j_loop.reverse();
    }
    {
        // Start the inner loop at the point matching the border's first point.
        let mut best = 0;
        let mut best_cost = f64::MAX;
        for s in 0..j_loop.len() {
            let cost = if m_count == n {
                (0..m_count as usize)
                    .map(|i| (body.position(o_loop[i]) - body.position(j_loop[(i + s) % j_loop.len()])).length())
                    .sum()
            } else {
                (body.position(o_loop[0]) - body.position(j_loop[s])).length()
            };
            if cost < best_cost {
                best_cost = cost;
                best = s;
            }
        }
        j_loop.rotate_left(best);
    }
    let mut ring_faces = stitch(&mut body, &o_loop, &j_loop);

    // Inner ring relax happens on the surface, before the body is merged with the limb.
    for _ in 0..opts.relax {
        let jl = j_loop.len();
        let next: Vec<V> = (0..jl)
            .map(|j| {
                let avg = 0.5 * (body.position(j_loop[(j + jl - 1) % jl]) + body.position(j_loop[(j + 1) % jl]));
                let p = mix(body.position(j_loop[j]), avg, 0.5);
                body_bvh.closest_point(p).map_or(p, |c| c.point)
            })
            .collect();
        for (j, p) in next.into_iter().enumerate() {
            body.set_position(j_loop[j], p);
        }
    }

    let mut preserved = (0..body_cap.min(body.point_capacity())).filter(|&p| body.point_valid(p)).count() as i32;
    // Support loops add points with ids past the original capacity, and carving removes some originals.
    {
        let mut remap = MeshRemap::default();
        body.compact(Some(&mut remap));
        for p in &mut i_loop {
            *p = remap.points[*p as usize];
        }
        for f in &mut ring_faces {
            *f = remap.faces[*f as usize];
        }
    }
    preserved += limb_preserved;
    res.preserved_points = preserved;

    // Merge and bridge.
    let mut w = body;
    let offset = w.append(&limb);
    for p in &mut a_loop {
        *p += offset;
    }

    let mut mean_gap = 0.0;
    let mut mean_edge = 0.0;
    let mut partner = vec![0usize; nu];
    for i in 0..n {
        let m = if sigma_i == sigma_a { i + opts.twist } else { opts.twist - i };
        partner[i as usize] = (((m % n) + n) % n) as usize;
        let iu = i as usize;
        mean_gap += (w.position(i_loop[partner[iu]]) - w.position(a_loop[iu])).length();
        mean_edge += (w.position(a_loop[(iu + 1) % nu]) - w.position(a_loop[iu])).length();
    }
    mean_gap /= n as f64;
    mean_edge /= n as f64;
    let s_rows = if opts.segments > 0 {
        opts.segments.min(64)
    } else {
        ((mean_gap / mean_edge.max(1e-12)).round() as i32).clamp(1, 8)
    };
    res.segments = s_rows;
    let su = s_rows as usize;

    let mut rows: Vec<Vec<Index>> = vec![vec![0; nu]; su + 1];
    rows[0] = a_loop.clone();
    for i in 0..nu {
        let k = partner[i];
        rows[su][i] = i_loop[k];
        let p0 = w.position(a_loop[i]);
        let p1 = w.position(i_loop[k]);
        let l = (p1 - p0).length() * opts.tension;
        let m0 = rails[i] * l;
        let m1 = -body_normals[k] * l;
        for r in 1..su {
            rows[r][i] = w.add_point(hermite(p0, m0, p1, m1, r as f64 / s_rows as f64));
        }
    }
    if s_rows > 1 {
        for _ in 0..opts.relax {
            let mut next: Vec<Vec<V>> = vec![Vec::new(); su + 1];
            for r in 1..su {
                for i in 0..nu {
                    let avg = 0.25
                        * (w.position(rows[r][(i + 1) % nu])
                            + w.position(rows[r][(i + nu - 1) % nu])
                            + w.position(rows[r - 1][i])
                            + w.position(rows[r + 1][i]));
                    next[r].push(mix(w.position(rows[r][i]), avg, 0.5));
                }
            }
            for r in 1..su {
                for i in 0..nu {
                    w.set_position(rows[r][i], next[r][i]);
                }
            }
        }
    }
    let mut new_faces = ring_faces;
    for r in 0..su {
        for i in 0..nu {
            let f = w.add_face(&[rows[r][i], rows[r][(i + 1) % nu], rows[r + 1][(i + 1) % nu], rows[r + 1][i]]);
            if f != INVALID {
                new_faces.push(f);
            }
        }
    }
    let soft = edges_of_faces(&w, &new_faces);
    set_soft_edges(&mut w, &soft);
    for &f in &new_faces {
        if !w.face_valid(f) {
            continue;
        }
        match w.face_size(f) {
            3 => res.triangles += 1,
            s if s > 4 => res.ngons += 1,
            _ => {}
        }
    }
    let issues = w.validate(validate::DEFAULT);
    w.compact(None);
    res.mesh = w;

    // Confidence.
    if m_count != n {
        penalties.push((
            10.0 + 6.0 * (m_count - n).abs() as f64,
            format!(
                "hole border has {} points for a {}-point loop: {} triangle{}",
                m_count,
                n,
                res.triangles,
                if res.triangles == 1 { "" } else { "s" }
            ),
        ));
    }
    let angle = cos_angle.abs().acos().to_degrees();
    if angle > 30.0 {
        penalties.push((
            ((angle - 30.0) * 0.9).min(45.0),
            format!("limb meets the body at a grazing angle ({:.0} deg off its normal)", angle),
        ));
    }
    if region.poles > 0 {
        penalties.push(((4.0 * region.poles as f64).min(20.0), format!("hole region touches {} poles", region.poles)));
    }
    if region.non_quads > 0 {
        penalties.push((
            (5.0 * region.non_quads as f64).min(15.0),
            format!("hole region replaced {} non-quad faces", region.non_quads),
        ));
    }
    if shrunk {
        penalties.push((3.0, "hole pulled in to fit inside the faces around it".into()));
    }
    if !region.enclosed {
        penalties.push((25.0, "the hole is larger than the faces around it; the ring may overlap".into()));
    }
    if misses > 0 {
        penalties.push(((3.0 * misses as f64).min(20.0), format!("{} loop points missed the body along the axis", misses)));
    }
    if cand.planarity > 0.25 {
        penalties.push((10.0, "the limb loop is far from planar".into()));
    }
    if cand.gap > 4.0 * cand.radius {
        penalties.push((10.0, "long bridge: the limb is far from the body".into()));
    }
    if face_border_loop {
        penalties.push((10.0, "no edge loop on the limb faces the body; used a face border".into()));
    }
    if n < 4 {
        penalties.push((10.0, format!("only {} points in the loop", n)));
    }
    if res.support_loops > 0 {
        penalties.push((
            (res.support_loops as f64).min(6.0),
            format!("added {} support loops to the body", res.support_loops),
        ));
    }
    if res.flipped_limb {
        penalties.push((5.0, "limb faces pointed inward; flipped them".into()));
    }
    if sigma_i != sigma_a {
        penalties.push((20.0, "the projected loop folds over itself".into()));
    }
    if let Some(first) = issues.first() {
        penalties.push((30.0, format!("result is not clean: {first}")));
    }
    penalties.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

    let mut score = 100.0;
    for (points, why) in &penalties {
        score -= points;
        res.reasons.push(why.clone());
    }
    res.confidence = (score.round() as i32).clamp(0, 100);

    let mut good: Vec<String> = Vec::new();
    if m_count == n {
        good.push(format!("matching {}-point loops", n));
    }
    if angle <= 15.0 {
        good.push("square-on".into());
    }
    if res.trimmed {
        good.push("trimmed at the intersection".into());
    }
    if res.triangles == 0 && res.ngons == 0 && issues.is_empty() {
        good.push("clean quads".into());
    }
    res.reasons.extend(notes);
    res.reasons.extend(good.iter().cloned());

    // A confident join leads with what went right; a doubtful one with what went wrong.
    let mut lead: Vec<String> = Vec::new();
    if res.confidence >= 85 {
        lead = good.clone();
    }
    lead.extend(penalties.iter().map(|p| p.1.clone()));
    if res.confidence < 85 {
        lead.extend(good);
    }
    let head = lead.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
    res.summary = format!("{}: {}%", if head.is_empty() { "joined".to_string() } else { head }, res.confidence);
    res.ok = true;
    res
}
