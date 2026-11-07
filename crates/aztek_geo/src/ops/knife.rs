//! Knife / multi-cut: cuts faces along a path of points on the surface, splitting the edges it crosses. The path
//! between two points follows the surface under the straight screen line joining them (the reference Multi-Cut, Blender
//! Knife), or with `through` cuts every face under that line, back faces included.

use std::collections::{BTreeMap, BTreeSet};

use glam::{DVec2, DVec3};

use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;
use crate::mix::mix3;
use crate::symmetry::SymmetryMap;
use crate::topology::quad_opposite_edge;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum KnifeKind {
    Point,
    Edge,
    #[default]
    Face,
}

/// A spot on the surface: a point, a spot along an edge, or a spot inside a face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnifePoint {
    pub kind: KnifeKind,
    pub id: Index,
    /// Edge: 0 at edge_point(id, 0), 1 at edge_point(id, 1).
    pub t: f64,
    /// Face: the spot, on the face. Filled in for every kind by knife_plan.
    pub position: DVec3,
}

impl Default for KnifePoint {
    fn default() -> Self {
        Self { kind: KnifeKind::Face, id: INVALID, t: 0.0, position: DVec3::ZERO }
    }
}

impl KnifePoint {
    pub fn point(p: Index) -> Self {
        Self { kind: KnifeKind::Point, id: p, t: 0.0, position: DVec3::ZERO }
    }
    pub fn edge(e: Index, t: f64) -> Self {
        Self { kind: KnifeKind::Edge, id: e, t, position: DVec3::ZERO }
    }
    pub fn face(f: Index, p: DVec3) -> Self {
        Self { kind: KnifeKind::Face, id: f, t: 0.0, position: p }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct KnifeOptions<'a> {
    /// Eye of a perspective view, in mesh space: the cut between two points lies in the plane through it and both.
    pub eye: Option<DVec3>,
    /// View direction of an orthographic view, used without an eye. Neither: the plane holds the points' average
    /// normal.
    pub view_dir: DVec3,
    /// Also cut from the last point back to the first.
    pub closed: bool,
    /// Cut every face under each screen line, back faces too, instead of walking the surface between the points.
    pub through: bool,
    /// Also make the mirror image of the cut.
    pub symmetry: Option<&'a SymmetryMap>,
}

/// One straight piece of the cut across one face of the mesh as it was planned on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KnifeSegment {
    pub face: Index,
    pub a: KnifePoint,
    pub b: KnifePoint,
    pub mirrored: bool,
}

#[derive(Clone, Debug, Default)]
pub struct KnifePlan {
    pub segments: Vec<KnifeSegment>,
    /// Indices of the point pairs (i, i+1) that no path along the surface joins.
    pub failed: Vec<i32>,
}

fn mesh_scale(m: &Mesh) -> f64 {
    let b = m.bounds();
    let s = if b.valid() { b.size().length() } else { 0.0 };
    if s > 0.0 {
        s
    } else {
        1.0
    }
}

fn valid_point(m: &Mesh, k: &KnifePoint) -> bool {
    match k.kind {
        KnifeKind::Point => m.point_valid(k.id),
        KnifeKind::Edge => m.edge_valid(k.id) && (0.0..=1.0).contains(&k.t),
        KnifeKind::Face => m.face_valid(k.id),
    }
}

fn faces_at(m: &Mesh, k: &KnifePoint) -> Vec<Index> {
    match k.kind {
        KnifeKind::Point => m.point_faces(k.id),
        KnifeKind::Edge => {
            let mut out = Vec::new();
            m.for_each_edge_corner(k.id, |c| out.push(m.corner_face(c)));
            out
        }
        KnifeKind::Face => vec![k.id],
    }
}

/// k's image under symmetry element g.
fn image_knife_point(m: &Mesh, sym: &SymmetryMap, k: &KnifePoint, g: i32) -> Option<KnifePoint> {
    let mut out = *k;
    match k.kind {
        KnifeKind::Point => out.id = sym.image(g, k.id),
        KnifeKind::Edge => {
            out.id = sym.image_edge(m, g, k.id);
            if out.id != INVALID && m.edge_point(out.id, 0) != sym.image(g, m.edge_point(k.id, 0)) {
                out.t = 1.0 - k.t;
            }
        }
        KnifeKind::Face => out.id = sym.image_face(m, g, k.id),
    }
    if out.id == INVALID {
        return None;
    }
    out.position = sym.transform(g, k.position);
    Some(out)
}

/// The cutting plane of one stretch of the path, and how faces cross it.
struct Cutter<'a> {
    m: &'a Mesh,
    n: DVec3,
    offset: f64,
    eps: f64,
}

impl Cutter<'_> {
    fn distance(&self, p: DVec3) -> f64 {
        self.n.dot(p) - self.offset
    }

    /// Where the face's border meets the plane, in corner order. A point on the plane counts once.
    fn crossings(&self, f: Index) -> Vec<KnifePoint> {
        let m = self.m;
        let mut out = Vec::new();
        m.for_each_face_corner(f, |c| {
            let p = m.corner_point(c);
            let q = m.corner_next_point(c);
            let sp = self.distance(m.position(p));
            let sq = self.distance(m.position(q));
            if sp.abs() <= self.eps {
                let mut k = KnifePoint::point(p);
                k.position = m.position(p);
                out.push(k);
                return;
            }
            if sq.abs() <= self.eps || (sp > 0.0) == (sq > 0.0) {
                return;
            }
            let s = sp / (sp - sq);
            let e = m.corner_edge(c);
            let mut k = KnifePoint::edge(e, if m.edge_point(e, 0) == p { s } else { 1.0 - s });
            k.position = mix3(m.position(p), m.position(q), s);
            out.push(k);
        });
        out
    }
}

/// Walks the surface from a to b through the faces the plane crosses, always moving towards b.
fn walk(cut: &Cutter, a: &KnifePoint, b: &KnifePoint, pos_eps: f64, mirrored: bool, out: &mut Vec<KnifeSegment>) -> bool {
    let m = cut.m;
    let starts = faces_at(m, a);
    let targets = faces_at(m, b);
    if let Some(&f) = starts.iter().find(|f| targets.contains(f)) {
        out.push(KnifeSegment { face: f, a: *a, b: *b, mirrored });
        return true;
    }
    let dir = b.position - a.position;
    let len2 = dir.dot(dir);
    if len2 <= 0.0 {
        return false;
    }
    let param = |x: DVec3| (x - a.position).dot(dir) / len2;

    // The next crossing along the line after the entry, never past b.
    let forward_exit = |f: Index, entry: &KnifePoint| -> Option<KnifePoint> {
        let from = param(entry.position);
        let mut best: Option<KnifePoint> = None;
        let mut best_param = 0.0;
        for k in cut.crossings(f) {
            if k.position.distance(entry.position) <= pos_eps {
                continue;
            }
            let p = param(k.position);
            if p <= from + 1e-9 || p > 1.0 + 1e-6 {
                continue;
            }
            if best.is_none() || p < best_param {
                best = Some(k);
                best_param = p;
            }
        }
        best
    };

    let mut best: Vec<KnifeSegment> = Vec::new();
    let mut found = false;
    for &start in &starts {
        let mut visited = vec![false; m.face_capacity() as usize];
        let mut segs = Vec::new();
        let mut f = start;
        let mut entry = *a;
        let mut ok = false;
        for _ in 0..=m.num_faces() {
            visited[f as usize] = true;
            if targets.contains(&f) {
                segs.push(KnifeSegment { face: f, a: entry, b: *b, mirrored });
                ok = true;
                break;
            }
            let Some(exit) = forward_exit(f, &entry) else { break };
            segs.push(KnifeSegment { face: f, a: entry, b: exit, mirrored });
            let mut next = INVALID;
            if exit.kind == KnifeKind::Edge {
                next = m.opposite_face(exit.id, f);
                if next != INVALID && visited[next as usize] {
                    next = INVALID;
                }
            } else {
                for g in m.point_faces(exit.id) {
                    if g == f || visited[g as usize] {
                        continue;
                    }
                    if targets.contains(&g) {
                        next = g;
                        break;
                    }
                    if next == INVALID && forward_exit(g, &exit).is_some() {
                        next = g;
                    }
                }
            }
            if next == INVALID {
                break;
            }
            entry = exit;
            f = next;
        }
        if ok && (!found || segs.len() < best.len()) {
            best = segs;
            found = true;
        }
    }
    if found {
        out.extend(best);
    }
    found
}

/// Every face piece under the screen line from a to b: the plane, clipped to the wedge between the two view rays.
#[allow(clippy::too_many_arguments)]
fn slice(
    cut: &Cutter,
    a: &KnifePoint,
    b: &KnifePoint,
    o0: DVec3,
    n0: DVec3,
    o1: DVec3,
    n1: DVec3,
    pos_eps: f64,
    mirrored: bool,
    out: &mut Vec<KnifeSegment>,
) -> bool {
    let m = cut.m;
    let before = out.len();
    let snap_to_click = |k: KnifePoint, f: Index| {
        for c in [a, b] {
            if c.kind == KnifeKind::Face && c.id == f && c.position.distance(k.position) <= pos_eps * 10.0 {
                return *c;
            }
        }
        k
    };
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let mut cr = cut.crossings(f);
        if cr.len() < 2 || cr.len() % 2 != 0 {
            continue;
        }
        let along = cut.n.cross(m.face_normal(f));
        if along.dot(along) < 1e-20 {
            continue;
        }
        cr.sort_by(|x, y| x.position.dot(along).partial_cmp(&y.position.dot(along)).unwrap_or(std::cmp::Ordering::Equal));
        for i in (0..cr.len() - 1).step_by(2) {
            let x0 = cr[i];
            let x1 = cr[i + 1];
            let (mut s0, mut s1) = (0.0f64, 1.0f64);
            let mut empty = false;
            for (o, n) in [(o0, n0), (o1, n1)] {
                let g0 = n.dot(x0.position - o);
                let g1 = n.dot(x1.position - o);
                let tol = cut.eps;
                if g0 < -tol && g1 < -tol {
                    empty = true;
                    break;
                }
                if g0 < -tol {
                    s0 = s0.max(g0 / (g0 - g1));
                }
                if g1 < -tol {
                    s1 = s1.min(g0 / (g0 - g1));
                }
            }
            if empty || s1 - s0 <= 1e-9 {
                continue;
            }
            let end = |s: f64, k: &KnifePoint| {
                if s <= 1e-9 || s >= 1.0 - 1e-9 {
                    return *k;
                }
                snap_to_click(KnifePoint::face(f, mix3(x0.position, x1.position, s)), f)
            };
            out.push(KnifeSegment { face: f, a: end(s0, &x0), b: end(s1, &x1), mirrored });
        }
    }
    out.len() > before
}

fn plan_chain(
    m: &Mesh,
    pts: &[KnifePoint],
    opts: &KnifeOptions,
    eye: Option<DVec3>,
    view_dir: DVec3,
    mirrored: bool,
    plan: &mut KnifePlan,
) {
    let scale = mesh_scale(m);
    let pos_eps = scale * 1e-7;
    let n = pts.len();
    let pairs = if opts.closed && n > 2 { n } else { n.saturating_sub(1) };
    for i in 0..pairs {
        let a = &pts[i];
        let b = &pts[(i + 1) % n];
        let mut avg_normal = DVec3::ZERO;
        for k in [a, b] {
            for f in faces_at(m, k) {
                avg_normal += m.face_normal(f);
            }
        }
        let look = if eye.is_some() {
            DVec3::ZERO
        } else if view_dir.dot(view_dir) > 0.0 {
            view_dir
        } else {
            avg_normal
        };
        let mut normal = match eye {
            Some(e) => (a.position - e).cross(b.position - e),
            None => (b.position - a.position).cross(look),
        };
        let len = normal.length();
        let mut ok = false;
        let mut segs = Vec::new();
        if len > 1e-12 * scale * scale {
            normal /= len;
            let cut = Cutter { m, n: normal, offset: normal.dot(a.position), eps: scale * 1e-9 };
            if opts.through {
                let ray_a = eye.map_or(look, |e| a.position - e);
                let ray_b = eye.map_or(look, |e| b.position - e);
                let o0 = eye.unwrap_or(a.position);
                let o1 = eye.unwrap_or(b.position);
                let mut n0 = normal.cross(ray_a);
                let mut n1 = normal.cross(ray_b);
                if n0.dot(b.position - o0) < 0.0 {
                    n0 = -n0;
                }
                if n1.dot(a.position - o1) < 0.0 {
                    n1 = -n1;
                }
                ok = slice(&cut, a, b, o0, n0, o1, n1, pos_eps, mirrored, &mut segs);
            } else {
                ok = walk(&cut, a, b, pos_eps, mirrored, &mut segs);
            }
        } else {
            let fb = faces_at(m, b);
            if let Some(f) = faces_at(m, a).into_iter().find(|f| fb.contains(f)) {
                segs.push(KnifeSegment { face: f, a: *a, b: *b, mirrored });
                ok = true;
            }
        }
        plan.segments.extend(segs);
        if !ok && !mirrored {
            plan.failed.push(i as i32);
        }
    }
}

fn ring_plan(m: &Mesh, edge: Index, t: f64, mirrored: bool, plan: &mut KnifePlan) {
    let at = |e: Index, start_point: Index| {
        let mut k = KnifePoint::edge(e, if m.edge_point(e, 0) == start_point { t } else { 1.0 - t });
        k.position = knife_position(m, &k);
        k
    };
    let mut faces = Vec::new();
    m.for_each_edge_corner(edge, |c| faces.push(m.corner_face(c)));
    let mut seen = vec![false; m.face_capacity() as usize];
    for dir in 0..faces.len().min(2) {
        let mut e = edge;
        let mut start = m.edge_point(edge, 0);
        let mut f = faces[dir];
        while f != INVALID && !seen[f as usize] {
            seen[f as usize] = true;
            let opp = quad_opposite_edge(m, e, f);
            if opp == INVALID {
                break;
            }
            // The start side continues along the quad's side edge from start.
            let mut next = INVALID;
            m.for_each_face_corner(f, |c| {
                let p = m.corner_point(c);
                let q = m.corner_next_point(c);
                if m.corner_edge(c) == e || m.corner_edge(c) == opp {
                    return;
                }
                if p == start {
                    next = q;
                } else if q == start {
                    next = p;
                }
            });
            if next == INVALID {
                break;
            }
            plan.segments.push(KnifeSegment { face: f, a: at(e, start), b: at(opp, next), mirrored });
            f = m.opposite_face(opp, f);
            e = opp;
            start = next;
        }
    }
}

/// Barycentric source corners and weights of a spot inside a face, from a fan of its corners.
struct FaceWeights {
    corners: [Index; 3],
    points: [Index; 3],
    w: [f32; 3],
}

fn plane_axes(normal: DVec3) -> (DVec3, DVec3) {
    let u = if normal.x.abs() < 0.9 { normal.cross(DVec3::X) } else { normal.cross(DVec3::Y) }.normalize();
    (u, normal.cross(u))
}

fn face_weights(m: &Mesh, corners: &[Index], normal: DVec3, p: DVec3) -> FaceWeights {
    let (u, v) = plane_axes(normal);
    let flat = |x: DVec3| DVec2::new(x.dot(u), x.dot(v));
    let q = flat(p);
    let mut best = FaceWeights { corners: [INVALID; 3], points: [INVALID; 3], w: [1.0, 0.0, 0.0] };
    let mut best_violation = f64::MAX;
    for k in 1..corners.len().saturating_sub(1) {
        let cs = [corners[0], corners[k], corners[k + 1]];
        let a = flat(m.position(m.corner_point(cs[0])));
        let b = flat(m.position(m.corner_point(cs[1])));
        let c = flat(m.position(m.corner_point(cs[2])));
        let area = (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x);
        if area.abs() < 1e-20 {
            continue;
        }
        let wa = ((b.x - q.x) * (c.y - q.y) - (b.y - q.y) * (c.x - q.x)) / area;
        let wb = ((c.x - q.x) * (a.y - q.y) - (c.y - q.y) * (a.x - q.x)) / area;
        let wc = 1.0 - wa - wb;
        let violation = 0.0f64.max(-wa).max(-wb).max(-wc);
        if violation < best_violation {
            best_violation = violation;
            let ws = [wa.max(0.0), wb.max(0.0), wc.max(0.0)];
            let sum = ws[0] + ws[1] + ws[2];
            for i in 0..3 {
                best.corners[i] = cs[i];
                best.points[i] = m.corner_point(cs[i]);
                best.w[i] = if sum > 0.0 { ws[i] / sum } else { 1.0 / 3.0 } as f32;
            }
        }
    }
    best
}

fn inside_face(m: &Mesh, f: Index, p: DVec3) -> bool {
    let (u, v) = plane_axes(m.face_normal(f));
    let poly: Vec<DVec2> = m
        .face_points(f)
        .iter()
        .map(|&x| {
            let w = m.position(x);
            DVec2::new(w.dot(u), w.dot(v))
        })
        .collect();
    super::common::inside_polygon(&poly, DVec2::new(p.dot(u), p.dot(v)))
}

/// Resolves a knife point's position on the mesh.
pub fn knife_position(mesh: &Mesh, p: &KnifePoint) -> DVec3 {
    match p.kind {
        KnifeKind::Point => mesh.position(p.id),
        KnifeKind::Edge => mix3(mesh.position(mesh.edge_point(p.id, 0)), mesh.position(mesh.edge_point(p.id, 1)), p.t),
        KnifeKind::Face => p.position,
    }
}

/// The pieces a cut through the points would make, without changing the mesh: for previews.
pub fn knife_plan(mesh: &Mesh, points: &[KnifePoint], opts: &KnifeOptions) -> KnifePlan {
    let mut plan = KnifePlan::default();
    let pts: Vec<KnifePoint> = points
        .iter()
        .filter(|k| valid_point(mesh, k))
        .map(|k| KnifePoint { position: knife_position(mesh, k), ..*k })
        .collect();
    plan_chain(mesh, &pts, opts, opts.eye, opts.view_dir, false, &mut plan);
    if let Some(sym) = opts.symmetry.filter(|s| !s.is_empty()) {
        for g in 1..sym.order() {
            let mirrored: Vec<KnifePoint> = pts.iter().filter_map(|k| image_knife_point(mesh, sym, k, g)).collect();
            if mirrored.len() != pts.len() {
                continue;
            }
            let eye = opts.eye.map(|e| sym.transform(g, e));
            plan_chain(mesh, &mirrored, opts, eye, sym.transform_vector(g, opts.view_dir), true, &mut plan);
        }
    }
    plan
}

/// Plan of an edge loop across the quad ring through edge, at t along it (and its mirror with symmetry).
pub fn knife_loop_plan(mesh: &Mesh, edge: Index, t: f64, symmetry: Option<&SymmetryMap>) -> KnifePlan {
    let mut plan = KnifePlan::default();
    if !mesh.edge_valid(edge) {
        return plan;
    }
    let t = t.clamp(0.0, 1.0);
    ring_plan(mesh, edge, t, false, &mut plan);
    let Some(sym) = symmetry.filter(|s| !s.is_empty()) else { return plan };
    // A ring an element maps onto itself at the same spot (round the axis) is planned once.
    let planned = |plan: &KnifePlan, k: &KnifePoint| {
        plan.segments.iter().any(|s| {
            [&s.a, &s.b].iter().any(|q| q.kind == KnifeKind::Edge && q.id == k.id && (q.t - k.t).abs() < 1e-9)
        })
    };
    for g in 1..sym.order() {
        if let Some(mk) = image_knife_point(mesh, sym, &KnifePoint::edge(edge, t), g) {
            if !planned(&plan, &mk) {
                ring_plan(mesh, mk.id, mk.t, true, &mut plan);
            }
        }
    }
    plan
}

/// Adds each segment's image under every other symmetry element, skipping segments with no image.
pub fn add_symmetric_images(mesh: &Mesh, plan: &mut KnifePlan, symmetry: &SymmetryMap) {
    if symmetry.is_empty() {
        return;
    }
    let own = plan.segments.len();
    for g in 1..symmetry.order() {
        for i in 0..own {
            let s = plan.segments[i];
            let f = symmetry.image_face(mesh, g, s.face);
            let a = image_knife_point(mesh, symmetry, &s.a, g);
            let b = image_knife_point(mesh, symmetry, &s.b, g);
            if let (true, Some(a), Some(b)) = (f != INVALID, a, b) {
                plan.segments.push(KnifeSegment { face: f, a, b, mirrored: true });
            }
        }
    }
}

/// Applies a plan made on this mesh. Pieces joined only through face-inside spots that do not reach the face's
/// border at both ends are dropped, as are pieces along existing edges. Returns the new edges.
pub fn knife_apply(mesh: &mut Mesh, plan: &KnifePlan) -> Vec<Index> {
    const T_TOL: f64 = 1e-7;
    let pos_eps = mesh_scale(mesh) * 1e-7;

    let normalize = |mesh: &Mesh, mut k: KnifePoint| {
        if k.kind == KnifeKind::Edge {
            if k.t <= T_TOL {
                k = KnifePoint::point(mesh.edge_point(k.id, 0));
            } else if k.t >= 1.0 - T_TOL {
                k = KnifePoint::point(mesh.edge_point(k.id, 1));
            }
        }
        k.position = knife_position(mesh, &k);
        k
    };

    let segs: Vec<KnifeSegment> = plan
        .segments
        .iter()
        .filter(|s| mesh.face_valid(s.face) && valid_point(mesh, &s.a) && valid_point(mesh, &s.b))
        .map(|s| KnifeSegment { face: s.face, a: normalize(mesh, s.a), b: normalize(mesh, s.b), mirrored: s.mirrored })
        .collect();

    // Split every crossed edge once per distinct spot, from its first point on.
    let mut cuts: BTreeMap<Index, Vec<f64>> = BTreeMap::new();
    for s in &segs {
        for k in [&s.a, &s.b] {
            if k.kind == KnifeKind::Edge {
                cuts.entry(k.id).or_default().push(k.t);
            }
        }
    }
    let mut made: BTreeMap<Index, Vec<(f64, Index)>> = BTreeMap::new();
    for (&e, ts) in cuts.iter_mut() {
        ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut cur = e;
        let mut prev = 0.0;
        let list = made.entry(e).or_default();
        for &x in ts.iter() {
            if list.last().is_some_and(|l| x - l.0 <= T_TOL) {
                continue;
            }
            let p = mesh.split_edge(cur, (x - prev) / (1.0 - prev));
            list.push((x, p));
            cur = mesh.last_split_edge();
            prev = x;
        }
    }
    let vertex_of = |k: &KnifePoint| -> Index {
        match k.kind {
            KnifeKind::Point => k.id,
            KnifeKind::Face => INVALID,
            KnifeKind::Edge => {
                let mut best = INVALID;
                let mut best_d = f64::MAX;
                if let Some(list) = made.get(&k.id) {
                    for &(x, p) in list {
                        if (x - k.t).abs() < best_d {
                            best_d = (x - k.t).abs();
                            best = p;
                        }
                    }
                }
                best
            }
        }
    };

    let mut by_face: BTreeMap<Index, Vec<&KnifeSegment>> = BTreeMap::new();
    for s in &segs {
        by_face.entry(s.face).or_default().push(s);
    }

    let mut origin: Vec<Index> = (0..mesh.face_capacity()).collect();
    let mut out = Vec::new();

    for (&face, list) in &by_face {
        // Path graph inside the face: border points, and face-inside spots joined by the pieces.
        struct Node {
            vertex: Index,
            pos: DVec3,
            links: Vec<usize>,
        }
        let mut nodes: Vec<Node> = Vec::new();
        let node_of = |nodes: &mut Vec<Node>, k: &KnifePoint| -> usize {
            let v = vertex_of(k);
            for (i, n) in nodes.iter().enumerate() {
                let same =
                    if v != INVALID { n.vertex == v } else { n.vertex == INVALID && n.pos.distance(k.position) <= pos_eps };
                if same {
                    return i;
                }
            }
            nodes.push(Node { vertex: v, pos: if v != INVALID { mesh.position(v) } else { k.position }, links: Vec::new() });
            nodes.len() - 1
        };
        let mut link_set: BTreeSet<(usize, usize)> = BTreeSet::new();
        for s in list {
            let a = node_of(&mut nodes, &s.a);
            let b = node_of(&mut nodes, &s.b);
            if a != b {
                link_set.insert((a.min(b), a.max(b)));
            }
        }
        let links: Vec<(usize, usize)> = link_set.into_iter().collect();
        for (l, &(a, b)) in links.iter().enumerate() {
            nodes[a].links.push(l);
            nodes[b].links.push(l);
        }

        let mut paths: Vec<Vec<usize>> = Vec::new();
        let mut used = vec![false; links.len()];
        for start in 0..nodes.len() {
            if nodes[start].vertex == INVALID {
                continue;
            }
            for li in 0..nodes[start].links.len() {
                let l0 = nodes[start].links[li];
                if used[l0] {
                    continue;
                }
                let mut path = vec![start];
                let mut link = l0;
                let mut cur = start;
                let mut dead = false;
                loop {
                    used[link] = true;
                    cur = if links[link].0 == cur { links[link].1 } else { links[link].0 };
                    path.push(cur);
                    if nodes[cur].vertex != INVALID {
                        break;
                    }
                    if nodes[cur].links.len() != 2 {
                        dead = true;
                        break;
                    }
                    link = if nodes[cur].links[0] == link { nodes[cur].links[1] } else { nodes[cur].links[0] };
                    if used[link] {
                        dead = true;
                        break;
                    }
                }
                if !dead {
                    paths.push(path);
                }
            }
        }
        if paths.is_empty() {
            continue;
        }

        let src_corners = mesh.face_corners(face);
        let src_normal = mesh.face_normal(face);

        for path in &paths {
            let va = nodes[path[0]].vertex;
            let vb = nodes[*path.last().unwrap()].vertex;
            let mut inner: Vec<DVec3> = path[1..path.len() - 1].iter().map(|&i| nodes[i].pos).collect();
            if va == vb || mesh.find_edge(va, vb) != INVALID {
                continue;
            }

            // The piece of the original face that still holds both ends with room between them.
            let probe = inner.first().copied().unwrap_or((mesh.position(va) + mesh.position(vb)) * 0.5);
            let mut target = INVALID;
            let mut ca = INVALID;
            let mut cb = INVALID;
            for f in mesh.point_faces(va) {
                if f as usize >= origin.len() || origin[f as usize] != face {
                    continue;
                }
                let a = mesh.find_corner(f, va);
                let b = mesh.find_corner(f, vb);
                if b == INVALID || mesh.corner_next(a) == b || mesh.corner_next(b) == a {
                    continue;
                }
                if target == INVALID || inside_face(mesh, f, probe) {
                    target = f;
                    ca = a;
                    cb = b;
                }
            }
            if target == INVALID {
                continue;
            }
            let f2 = mesh.split_face(target, ca, cb);
            if f2 == INVALID {
                continue;
            }
            origin.resize(mesh.face_capacity() as usize, INVALID);
            origin[f2 as usize] = face;
            let mut e = mesh.last_split_edge();
            if mesh.edge_point(e, 0) != va {
                inner.reverse();
            }
            for i in 0..inner.len() {
                let p = mesh.split_edge(e, 1.0 / (inner.len() - i + 1) as f64);
                mesh.set_position(p, inner[i]);
                let fw = face_weights(mesh, &src_corners, src_normal, inner[i]);
                if fw.corners[0] != INVALID {
                    mesh.point_attrs_mut().mix(p as usize, &fw.points, &fw.w);
                    let mut pcs = Vec::new();
                    mesh.for_each_point_corner(p, |c| pcs.push(c));
                    for c in pcs {
                        mesh.corner_attrs_mut().mix(c as usize, &fw.corners, &fw.w);
                    }
                }
                out.push(e);
                e = mesh.last_split_edge();
            }
            out.push(e);
        }
    }
    out
}

/// knife_plan then knife_apply.
pub fn knife_cut(mesh: &mut Mesh, points: &[KnifePoint], opts: &KnifeOptions) -> Vec<Index> {
    let plan = knife_plan(mesh, points, opts);
    knife_apply(mesh, &plan)
}
