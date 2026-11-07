
use std::collections::HashMap;

use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuadSplit {
    /// Order-independent, what MikkTSpace recommends.
    ShortestDiagonal,
    /// Fan from the first corner.
    FirstCorner,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TriangulateOptions {
    pub quads: QuadSplit,
    /// N-gons: flip diagonals toward the constrained Delaunay triangulation (largest smallest angle).
    pub beauty: bool,
}

impl Default for TriangulateOptions {
    fn default() -> Self {
        Self {
            quads: QuadSplit::ShortestDiagonal,
            beauty: true,
        }
    }
}

/// Triangles of a whole mesh. Every live face of n corners gets exactly n-2 triangles.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Triangulation {
    /// 3 corner ids per triangle, wound like the face.
    pub corners: Vec<Index>,
    /// Source face per triangle.
    pub faces: Vec<Index>,
    /// Per face capacity: first triangle of the face, INVALID if deleted.
    pub face_first: Vec<Index>,
}

impl Triangulation {
    pub fn triangle_count(&self) -> Index {
        self.faces.len() as Index
    }
}

fn orient2d(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    (b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)
}

/// Orthonormal u, v with u x v along n.
fn plane_axes(n: DVec3) -> (DVec3, DVec3) {
    let a = if n.x.abs() < 0.6 {
        DVec3::X
    } else if n.y.abs() < 0.6 {
        DVec3::Y
    } else {
        DVec3::Z
    };
    let u = a.cross(n).normalize();
    (u, n.cross(u))
}

/// Largest coordinate extent, so tolerances scale with the polygon.
fn polygon_size(poly: &[DVec2]) -> f64 {
    let (mut lo, mut hi) = (DVec2::splat(1e300), DVec2::splat(-1e300));
    for &p in poly {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let e = (hi.x - lo.x).max(hi.y - lo.y);
    if e > 0.0 {
        e
    } else {
        1.0
    }
}

/// p inside or on the closed triangle abc (counter-clockwise), with a tolerance that blocks touching.
fn in_closed_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2, eps: f64) -> bool {
    orient2d(a, b, p) >= -eps && orient2d(b, c, p) >= -eps && orient2d(c, a, p) >= -eps
}

/// > 0 when d lies inside the circle through the counter-clockwise triangle abc.
fn in_circle(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> f64 {
    let (ad, bd, cd) = (a - d, b - d, c - d);
    let (a2, b2, c2) = (ad.dot(ad), bd.dot(bd), cd.dot(cd));
    ad.x * (bd.y * c2 - b2 * cd.y) - ad.y * (bd.x * c2 - b2 * cd.x)
        + a2 * (bd.x * cd.y - bd.y * cd.x)
}

fn is_forbidden(forbidden: &[u64], n: Index, a: Index, b: Index) -> bool {
    !forbidden.is_empty()
        && forbidden
            .binary_search(&(a.min(b) as u64 * n as u64 + a.max(b) as u64))
            .is_ok()
}

/// Lawson flips toward the constrained Delaunay triangulation of the polygon: an interior diagonal is swapped when the
/// quad around it is strictly convex and the far point is inside the circumcircle.
fn delaunay_flips(poly: &[DVec2], tris: &mut [Index], forbidden: &[u64]) {
    let n = poly.len() as Index;
    let nt = tris.len() / 3;
    if nt < 2 {
        return;
    }
    let ext = polygon_size(poly);
    let area_eps = 1e-12 * ext * ext;
    let circle_eps = 1e-10 * ext * ext * ext * ext;
    let key = |a: Index, b: Index| a as u64 * n as u64 + b as u64;

    // Directed edge a->b to the triangle holding it.
    let mut owner: HashMap<u64, Index> = HashMap::with_capacity(nt * 3);
    for t in 0..nt {
        for k in 0..3 {
            owner.insert(key(tris[t * 3 + k], tris[t * 3 + (k + 1) % 3]), t as Index);
        }
    }
    let mut stack = Vec::new();
    for t in (0..nt).rev() {
        for k in (0..3).rev() {
            let (a, b) = (tris[t * 3 + k], tris[t * 3 + (k + 1) % 3]);
            if a < b && owner.contains_key(&key(b, a)) {
                stack.push((a, b));
            }
        }
    }
    let third = |tris: &[Index], t: Index, a: Index, b: Index| {
        (0..3)
            .map(|k| tris[t as usize * 3 + k])
            .find(|&v| v != a && v != b)
            .unwrap_or(INVALID)
    };
    // Each flip strictly improves the triangulation, the cap only guards against float noise.
    let mut budget = nt * nt * 4 + 64;
    while let Some((a, b)) = stack.pop() {
        if budget == 0 {
            break;
        }
        budget -= 1;
        let (Some(&t1), Some(&t2)) = (owner.get(&key(a, b)), owner.get(&key(b, a))) else {
            continue;
        };
        let (x, y) = (third(tris, t1, a, b), third(tris, t2, a, b));
        if x == INVALID
            || y == INVALID
            || x == y
            || owner.contains_key(&key(x, y))
            || owner.contains_key(&key(y, x))
            || is_forbidden(forbidden, n, x, y)
        {
            continue;
        }
        let p = |i: Index| poly[i as usize];
        // t1 = (a, b, x), t2 = (b, a, y): the quad is a, y, b, x.
        if orient2d(p(a), p(y), p(x)) <= area_eps || orient2d(p(y), p(b), p(x)) <= area_eps {
            continue;
        }
        if in_circle(p(a), p(b), p(x), p(y)) <= circle_eps {
            continue;
        }
        owner.remove(&key(a, b));
        owner.remove(&key(b, a));
        for (t, [q0, q1, q2]) in [(t1, [a, y, x]), (t2, [y, b, x])] {
            let t0 = t as usize * 3;
            tris[t0] = q0;
            tris[t0 + 1] = q1;
            tris[t0 + 2] = q2;
            owner.insert(key(q0, q1), t);
            owner.insert(key(q1, q2), t);
            owner.insert(key(q2, q0), t);
        }
        for (p0, q0) in [(a, y), (y, b), (b, x), (x, a)] {
            stack.push((p0.min(q0), p0.max(q0)));
        }
    }
}

/// Orders a diagonal by its endpoints' absolute coordinates. Mirroring across any axis plane keeps it, so equal-length
/// diagonals of mirrored quads (squares on a symmetric mesh) split the mirrored way.
fn mirror_key(a: DVec3, b: DVec3) -> [f64; 6] {
    let mut x = [a.x.abs(), a.y.abs(), a.z.abs()];
    let mut y = [b.x.abs(), b.y.abs(), b.z.abs()];
    if x < y {
        std::mem::swap(&mut x, &mut y);
    }
    [x[0], x[1], x[2], y[0], y[1], y[2]]
}

#[derive(Default)]
struct FaceScratch {
    corners: Vec<Index>,
    pos: Vec<DVec3>,
    poly: Vec<DVec2>,
    local: Vec<Index>,
    touch: Vec<(Index, Index)>,
    forbidden: Vec<u64>,
}

/// Some other face, or a loose edge, already joins points u and v.
fn joined_elsewhere(mesh: &Mesh, f: Index, u: Index, v: Index) -> bool {
    let mut found = mesh.find_edge(u, v) != INVALID;
    if !found {
        mesh.for_each_point_corner(u, |c| {
            let g = mesh.corner_face(c);
            found = found || (g != f && mesh.find_corner(g, v) != INVALID);
        });
    }
    found
}

/// Diagonals of face f (local a * n + b, a < b, sorted) whose two points another face also spans, as at a valence-2
/// point between two faces. Taking one would build the same triangle or edge twice.
fn shared_diagonals(mesh: &Mesh, f: Index, s: &mut FaceScratch) {
    s.forbidden.clear();
    s.touch.clear();
    let n = s.corners.len() as Index;
    for i in 0..n {
        mesh.for_each_point_corner(mesh.corner_point(s.corners[i as usize]), |c| {
            if mesh.corner_face(c) != f {
                s.touch.push((mesh.corner_face(c), i));
            }
        });
    }
    for i in 0..n {
        for j in i + 2..n {
            if !(i == 0 && j == n - 1)
                && mesh.find_edge(
                    mesh.corner_point(s.corners[i as usize]),
                    mesh.corner_point(s.corners[j as usize]),
                ) != INVALID
            {
                s.forbidden.push(i as u64 * n as u64 + j as u64);
            }
        }
    }
    s.touch.sort_unstable();
    let mut a = 0;
    while a < s.touch.len() {
        let mut b = a;
        while b < s.touch.len() && s.touch[b].0 == s.touch[a].0 {
            b += 1;
        }
        for x in a..b {
            for y in x + 1..b {
                let (i, j) = (
                    s.touch[x].1.min(s.touch[y].1),
                    s.touch[x].1.max(s.touch[y].1),
                );
                if j - i >= 2 && !(i == 0 && j == n - 1) {
                    s.forbidden.push(i as u64 * n as u64 + j as u64);
                }
            }
        }
        a = b;
    }
    s.forbidden.sort_unstable();
    s.forbidden.dedup();
}

fn triangulate_face_impl(
    mesh: &Mesh,
    f: Index,
    out: &mut Vec<Index>,
    opts: &TriangulateOptions,
    s: &mut FaceScratch,
) {
    let n = mesh.face_size(f);
    s.corners.clear();
    mesh.for_each_face_corner(f, |c| s.corners.push(c));
    let c = s.corners.clone();
    if n == 3 {
        out.extend_from_slice(&[c[0], c[1], c[2]]);
        return;
    }
    let nu = n as usize;
    s.pos.clear();
    s.pos
        .extend(c.iter().map(|&k| mesh.position(mesh.corner_point(k))));
    let mut newell = DVec3::ZERO;
    for i in 0..nu {
        newell += (s.pos[i] - s.pos[0]).cross(s.pos[(i + 1) % nu] - s.pos[0]);
    }
    let newell_len = newell.length();
    if newell_len.is_nan() || newell_len <= 0.0 {
        for i in 1..nu - 1 {
            out.extend_from_slice(&[c[0], c[i], c[i + 1]]);
        }
        return;
    }
    let (u, v) = plane_axes(newell / newell_len);
    s.poly.clear();
    for i in 0..nu {
        let d = s.pos[i] - s.pos[0];
        s.poly.push(DVec2::new(d.dot(u), d.dot(v)));
    }

    if n == 4 {
        let ext = polygon_size(&s.poly);
        let eps = 1e-12 * ext * ext;
        let q = &s.poly;
        // A diagonal folds when one of its triangles faces against the polygon.
        let ok02 = orient2d(q[0], q[1], q[2]) > eps && orient2d(q[0], q[2], q[3]) > eps;
        let ok13 = orient2d(q[1], q[2], q[3]) > eps && orient2d(q[1], q[3], q[0]) > eps;
        let mut alt = false;
        if opts.quads == QuadSplit::ShortestDiagonal {
            let d13 = s.pos[1].distance(s.pos[3]);
            let d02 = s.pos[0].distance(s.pos[2]);
            alt = if (d13 - d02).abs() > 1e-9 * (d13 + d02) {
                d13 < d02
            } else {
                mirror_key(s.pos[1], s.pos[3]) > mirror_key(s.pos[0], s.pos[2])
            };
        }
        if if alt { !ok13 && ok02 } else { !ok02 && ok13 } {
            alt = !alt;
        }
        let p: Vec<Index> = c.iter().map(|&k| mesh.corner_point(k)).collect();
        let swap = if alt {
            joined_elsewhere(mesh, f, p[1], p[3])
                && (ok02 || !ok13)
                && !joined_elsewhere(mesh, f, p[0], p[2])
        } else {
            joined_elsewhere(mesh, f, p[0], p[2])
                && (ok13 || !ok02)
                && !joined_elsewhere(mesh, f, p[1], p[3])
        };
        if swap {
            alt = !alt;
        }
        if alt {
            out.extend_from_slice(&[c[1], c[2], c[3], c[1], c[3], c[0]]);
        } else {
            out.extend_from_slice(&[c[0], c[1], c[2], c[0], c[2], c[3]]);
        }
        return;
    }

    s.local.clear();
    shared_diagonals(mesh, f, s);
    ear_clip_impl(&s.poly, &mut s.local, &s.forbidden);
    if opts.beauty {
        delaunay_flips(&s.poly, &mut s.local, &s.forbidden);
    }
    out.extend(s.local.iter().map(|&i| c[i as usize]));
}

fn ear_clip_impl(poly: &[DVec2], out: &mut Vec<Index>, forbidden: &[u64]) {
    let n = poly.len() as Index;
    if n < 3 {
        return;
    }
    let ext = polygon_size(poly);
    let area_eps = 1e-12 * ext * ext;
    let mut next: Vec<Index> = (0..n).map(|i| (i + 1) % n).collect();
    let mut prev: Vec<Index> = (0..n).map(|i| (i + n - 1) % n).collect();
    let mut remaining = n;
    // Flat (collinear) tips never make ears, so the extra points bevels and loops leave on straight edges get joined
    // across the polygon instead of into zero-area slivers.
    let is_ear = |i: Index, next: &[Index], prev: &[Index], remaining: Index| {
        let (ia, ic) = (prev[i as usize], next[i as usize]);
        let (a, b, c) = (poly[ia as usize], poly[i as usize], poly[ic as usize]);
        if orient2d(a, b, c) <= area_eps || (remaining > 3 && is_forbidden(forbidden, n, ia, ic)) {
            return false;
        }
        let (lo, hi) = (a.min(b.min(c)), a.max(b.max(c)));
        let mut j = next[ic as usize];
        while j != ia {
            let p = poly[j as usize];
            j = next[j as usize];
            if p.x < lo.x || p.y < lo.y || p.x > hi.x || p.y > hi.y {
                continue;
            }
            // A point on top of a corner is the polygon touching itself there, not blocking.
            if p == a || p == b || p == c {
                continue;
            }
            if in_closed_triangle(p, a, b, c, area_eps) {
                return false;
            }
        }
        true
    };
    let mut i: Index = 0;
    let mut since_clip: Index = 0;
    while remaining > 3 {
        let stuck = since_clip >= remaining;
        if stuck {
            // No clean ear (self-overlapping projection or float noise): clip the most convex tip.
            let (mut best, mut best_cross) = (i, -1e300);
            let mut j = i;
            for _ in 0..remaining {
                let cr = orient2d(
                    poly[prev[j as usize] as usize],
                    poly[j as usize],
                    poly[next[j as usize] as usize],
                );
                if cr > best_cross {
                    best_cross = cr;
                    best = j;
                }
                j = next[j as usize];
            }
            i = best;
        }
        if stuck || is_ear(i, &next, &prev, remaining) {
            let (pi, ni) = (prev[i as usize], next[i as usize]);
            out.extend_from_slice(&[pi, i, ni]);
            next[pi as usize] = ni;
            prev[ni as usize] = pi;
            remaining -= 1;
            i = pi;
            since_clip = 0;
            continue;
        }
        i = next[i as usize];
        since_clip += 1;
    }
    out.extend_from_slice(&[prev[i as usize], i, next[i as usize]]);
}

/// Ear clipping of a simple polygon given in 2D, counter-clockwise. Appends 3*(n-2) local indices.
pub fn ear_clip(poly: &[DVec2], out: &mut Vec<Index>) {
    ear_clip_impl(poly, out, &[]);
}

/// Appends 3*(n-2) corner ids for face f.
pub fn triangulate_face(mesh: &Mesh, f: Index, out: &mut Vec<Index>, opts: &TriangulateOptions) {
    triangulate_face_impl(mesh, f, out, opts, &mut FaceScratch::default());
}

/// Deterministic: the same face gives the same triangles, so display, export, tangents and picking all agree. Faces are
/// projected onto their Newell plane. Quads take opts.quads unless that diagonal folds, larger faces are ear-clipped
/// (collinear corners are never ear tips) and then optionally flipped toward Delaunay. Never adds points. Parallel.
pub fn triangulate(mesh: &Mesh, opts: &TriangulateOptions) -> Triangulation {
    let fc = mesh.face_capacity();
    let mut t = Triangulation {
        face_first: vec![INVALID; fc as usize],
        ..Default::default()
    };
    let mut total: Index = 0;
    for f in 0..fc {
        if mesh.face_valid(f) {
            t.face_first[f as usize] = total;
            total += mesh.face_size(f) - 2;
        }
    }
    let per_face: Vec<Vec<Index>> = (0..fc)
        .into_par_iter()
        .with_min_len(4096)
        .map_init(FaceScratch::default, |s, f| {
            let mut out = Vec::new();
            if mesh.face_valid(f) {
                triangulate_face_impl(mesh, f, &mut out, opts, s);
            }
            out
        })
        .collect();
    t.corners.reserve(total as usize * 3);
    t.faces.reserve(total as usize);
    for (f, tris) in per_face.into_iter().enumerate() {
        if mesh.face_valid(f as Index) {
            t.faces.extend(std::iter::repeat_n(
                f as Index,
                mesh.face_size(f as Index) as usize - 2,
            ));
            t.corners.extend(tris);
        }
    }
    t
}
