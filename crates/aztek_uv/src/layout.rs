//! Orienting, transforming, straightening and fitting charts.

use std::collections::{BTreeMap, HashMap};
use std::f64::consts::{FRAC_PI_2, PI};

use aztek_geo::{Index, INVALID};
use glam::{DVec2, IVec2};

use crate::chart::Chart;
use crate::common::edge_key;
use crate::sparse::{Ldlt, Triplet};
use crate::unwrap::{folded_triangles, slim, unwrap_chart, UnwrapOptions};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OrientMode {
    /// Smallest bounding rectangle, long side along U.
    #[default]
    BoundingBox,
    /// Principal axis of the uv area along U.
    Principal,
    /// Longest boundary edge snapped to the nearest axis.
    LongestEdge,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StraightenResult {
    Failed,
    /// Every face a quad in a regular grid: rows and columns made straight.
    Grid,
    /// Boundary laid on a rectangle through its four sharpest corners, interior relaxed.
    Rectangle,
}

fn orient2d(o: DVec2, a: DVec2, b: DVec2) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn rotate(p: DVec2, c: f64, s: f64) -> DVec2 {
    DVec2::new(p.x * c - p.y * s, p.x * s + p.y * c)
}

fn valid_uvs(ch: &Chart) -> bool {
    ch.area_uv().abs() > 1e-12 && folded_triangles(ch) == 0
}

/// glm::mix.
pub(crate) fn mix2(a: DVec2, b: DVec2, t: f64) -> DVec2 {
    a * (1.0 - t) + b * t
}

/// Integer grid coordinates per vertex for an all-quad disk chart; None when it is not a regular grid.
fn grid_coords(ch: &Chart) -> Option<Vec<IVec2>> {
    let face_count = ch.faces.len();
    let mut quad = vec![[0 as Index; 4]; face_count];
    for f in 0..face_count {
        if ch.face_start[f + 1] - ch.face_start[f] != 4 {
            return None;
        }
        for k in 0..4 {
            quad[f][k] = ch.corner_vertex[(ch.face_start[f] + k as Index) as usize];
        }
    }
    let mut edge_faces: HashMap<u64, [Index; 2]> = HashMap::new();
    for (f, q) in quad.iter().enumerate() {
        for k in 0..4 {
            let key = edge_key(q[k], q[(k + 1) % 4]);
            match edge_faces.get_mut(&key) {
                None => {
                    edge_faces.insert(key, [f as Index, INVALID]);
                }
                Some(e) => {
                    if e[1] != INVALID {
                        return None;
                    }
                    e[1] = f as Index;
                }
            }
        }
    }

    let unset = IVec2::splat(i32::MIN);
    let mut coord = vec![unset; ch.vertex_count() as usize];
    let mut done = vec![false; face_count];
    let start = [IVec2::new(0, 0), IVec2::new(1, 0), IVec2::new(1, 1), IVec2::new(0, 1)];
    for k in 0..4 {
        coord[quad[0][k] as usize] = start[k];
    }
    let mut queue = vec![0 as Index];
    done[0] = true;
    let mut qi = 0;
    while qi < queue.len() {
        let q = quad[queue[qi] as usize];
        for k in 0..4 {
            let (a, b) = (q[k], q[(k + 1) % 4]);
            let ef = edge_faces[&edge_key(a, b)];
            let g = if ef[0] == queue[qi] { ef[1] } else { ef[0] };
            if g == INVALID {
                continue;
            }
            let out = coord[a as usize] - coord[q[(k + 3) % 4] as usize];
            let w = quad[g as usize];
            let mut m = None;
            for j in 0..4 {
                if w[j] == b && w[(j + 1) % 4] == a {
                    m = Some(j);
                }
            }
            let m = m?;
            let (ca, cb) = (coord[a as usize], coord[b as usize]);
            let mut assign = |v: Index, c: IVec2| {
                if coord[v as usize] == unset {
                    coord[v as usize] = c;
                    true
                } else {
                    coord[v as usize] == c
                }
            };
            if !assign(w[(m + 2) % 4], ca + out) || !assign(w[(m + 3) % 4], cb + out) {
                return None;
            }
            if !done[g as usize] {
                done[g as usize] = true;
                queue.push(g);
            }
        }
        qi += 1;
    }
    if queue.len() != face_count {
        return None;
    }
    let mut seen = BTreeMap::new();
    for (v, c) in coord.iter().enumerate() {
        if seen.insert((c.x, c.y), v).is_some() {
            return None;
        }
    }
    Some(coord)
}

fn lay_grid(ch: &mut Chart, coord: &[IVec2]) {
    let (mut lo, mut hi) = (IVec2::splat(i32::MAX), IVec2::splat(i32::MIN));
    for &c in coord {
        lo = lo.min(c);
        hi = hi.max(c);
    }
    let mut width = vec![0.0; (hi.x - lo.x) as usize + 1];
    let mut wn = vec![0.0; width.len()];
    let mut height = vec![0.0; (hi.y - lo.y) as usize + 1];
    let mut hn = vec![0.0; height.len()];
    for f in 0..ch.face_start.len().saturating_sub(1) {
        let (s, e) = (ch.face_start[f], ch.face_start[f + 1]);
        for k in s..e {
            let a = ch.corner_vertex[k as usize] as usize;
            let b = ch.corner_vertex[if k + 1 < e { k + 1 } else { s } as usize] as usize;
            let d = coord[b] - coord[a];
            let len = (ch.pos[b] - ch.pos[a]).length();
            if d.y == 0 {
                let i = (coord[a].x.min(coord[b].x) - lo.x) as usize;
                width[i] += len;
                wn[i] += 1.0;
            } else {
                let j = (coord[a].y.min(coord[b].y) - lo.y) as usize;
                height[j] += len;
                hn[j] += 1.0;
            }
        }
    }
    let mut u = vec![0.0; width.len() + 1];
    let mut v = vec![0.0; height.len() + 1];
    for i in 0..width.len() {
        u[i + 1] = u[i] + if wn[i] > 0.0 { width[i] / wn[i] } else { 0.0 };
    }
    for j in 0..height.len() {
        v[j + 1] = v[j] + if hn[j] > 0.0 { height[j] / hn[j] } else { 0.0 };
    }
    for p in 0..ch.vertex_count() as usize {
        ch.uv[p] = DVec2::new(u[(coord[p].x - lo.x) as usize], v[(coord[p].y - lo.y) as usize]);
    }
}

fn lay_rectangle(ch: &mut Chart) -> bool {
    let loops = ch.boundary_loops();
    if loops.is_empty() || loops[0].len() < 4 {
        return false;
    }
    if !valid_uvs(ch) && !unwrap_chart(ch, &UnwrapOptions::default()).ok {
        return false;
    }
    let lp = &loops[0];
    let n = lp.len();
    let turn: Vec<f64> = (0..n)
        .map(|i| {
            let (a, b, c) = (ch.uv[lp[(i + n - 1) % n] as usize], ch.uv[lp[i] as usize], ch.uv[lp[(i + 1) % n] as usize]);
            let (d1, d2) = (b - a, c - b);
            (d1.x * d2.y - d1.y * d2.x).atan2(d1.dot(d2))
        })
        .collect();
    // Four sharpest left turns, kept apart along the loop.
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&x, &y| turn[y].partial_cmp(&turn[x]).unwrap_or(std::cmp::Ordering::Equal));
    let mut corners: Vec<usize> = Vec::new();
    let gap = (n / 8).max(1);
    for &i in &order {
        let close = corners.iter().any(|&c| {
            let d = if i > c { i - c } else { c - i };
            d.min(n - d) < gap
        });
        if !close {
            corners.push(i);
        }
        if corners.len() == 4 {
            break;
        }
    }
    if corners.len() < 4 {
        return false;
    }
    corners.sort_unstable();

    let mut side = [0.0; 4];
    let mut along = vec![0.0; n];
    for s in 0..4 {
        let (from, to) = (corners[s], corners[(s + 1) % 4]);
        let mut len = 0.0;
        let mut i = from;
        while i != to {
            along[i] = len;
            len += (ch.pos[lp[(i + 1) % n] as usize] - ch.pos[lp[i] as usize]).length();
            i = (i + 1) % n;
        }
        side[s] = len;
    }
    let (w, h) = (0.5 * (side[0] + side[2]), 0.5 * (side[1] + side[3]));
    if w <= 0.0 || h <= 0.0 {
        return false;
    }
    let rect = [DVec2::ZERO, DVec2::new(w, 0.0), DVec2::new(w, h), DVec2::new(0.0, h), DVec2::ZERO];
    let pin_count = ch.pinned.len();
    let saved_pins = std::mem::replace(&mut ch.pinned, vec![false; pin_count]);
    for s in 0..4 {
        let (from, to) = (corners[s], corners[(s + 1) % 4]);
        let mut i = from;
        while i != to {
            let t = if side[s] > 0.0 { along[i] / side[s] } else { 0.0 };
            ch.uv[lp[i] as usize] = mix2(rect[s], rect[s + 1], t);
            ch.pinned[lp[i] as usize] = true;
            i = (i + 1) % n;
        }
    }

    // Interior at neighbour averages: fold free on a convex boundary.
    let v_count = ch.vertex_count() as usize;
    let mut free_id = vec![INVALID; v_count];
    let mut nf = 0usize;
    for v in 0..v_count {
        if !ch.pinned[v] {
            free_id[v] = nf as Index;
            nf += 1;
        }
    }
    if nf > 0 {
        let mut edges: Vec<u64> = Vec::new();
        for t in &ch.tris {
            for i in 0..3 {
                edges.push(edge_key(t[i], t[(i + 1) % 3]));
            }
        }
        edges.sort_unstable();
        edges.dedup();
        let mut trips = Vec::new();
        let (mut bu, mut bv) = (vec![0.0; nf], vec![0.0; nf]);
        for e in edges {
            let (a, b) = ((e >> 32) as usize, (e & 0xffff_ffff) as usize);
            for (p, q) in [(a, b), (b, a)] {
                if free_id[p] == INVALID {
                    continue;
                }
                let fp = free_id[p] as usize;
                trips.push(Triplet::new(fp, fp, 1.0));
                if free_id[q] != INVALID {
                    trips.push(Triplet::new(fp, free_id[q] as usize, -1.0));
                } else {
                    bu[fp] += ch.uv[q].x;
                    bv[fp] += ch.uv[q].y;
                }
            }
        }
        let solver = Ldlt::new(nf, &trips);
        if solver.ok() {
            if let (Some(xu), Some(xv)) = (solver.solve(&bu), solver.solve(&bv)) {
                for v in 0..v_count {
                    if free_id[v] != INVALID {
                        let f = free_id[v] as usize;
                        ch.uv[v] = DVec2::new(xu[f], xv[f]);
                    }
                }
            }
        }
        slim(ch, 12, true);
    }
    ch.pinned = saved_pins;
    true
}

/// Counter-clockwise convex hull (Andrew's monotone chain).
pub fn convex_hull(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap().then(a.y.partial_cmp(&b.y).unwrap()));
    pts.dedup();
    if pts.len() < 3 {
        return pts;
    }
    let mut hull = vec![DVec2::ZERO; pts.len() * 2];
    let mut k = 0;
    for &p in &pts {
        while k >= 2 && orient2d(hull[k - 2], hull[k - 1], p) <= 0.0 {
            k -= 1;
        }
        hull[k] = p;
        k += 1;
    }
    let t = k + 1;
    for i in (1..pts.len()).rev() {
        while k >= t && orient2d(hull[k - 2], hull[k - 1], pts[i - 1]) <= 0.0 {
            k -= 1;
        }
        hull[k] = pts[i - 1];
        k += 1;
    }
    hull.truncate(k - 1);
    hull
}

/// Rotation (radians) that orients the points' minimum-area rectangle to the axes, long side along U, with the
/// rectangle's size.
pub fn min_area_rect_angle(points: &[DVec2]) -> (f64, DVec2) {
    let hull = convex_hull(points.to_vec());
    let (mut best_area, mut best_angle, mut best_size) = (f64::MAX, 0.0, DVec2::ZERO);
    for i in 0..hull.len() {
        let e = hull[(i + 1) % hull.len()] - hull[i];
        if e.length() <= 0.0 {
            continue;
        }
        let a = -e.y.atan2(e.x);
        let (c, s) = (a.cos(), a.sin());
        let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(-f64::MAX));
        for &p in &hull {
            let r = rotate(p, c, s);
            lo = lo.min(r);
            hi = hi.max(r);
        }
        let ext = hi - lo;
        // Ties go to the smaller rotation so the result is stable.
        let area = ext.x * ext.y;
        if area < best_area * (1.0 - 1e-9) {
            best_area = area;
            best_angle = a;
            best_size = ext;
        }
    }
    if best_size.y > best_size.x {
        best_angle += FRAC_PI_2;
        best_size = DVec2::new(best_size.y, best_size.x);
    }
    // Keep the turn within a quarter either way: same rectangle, less rotation.
    while best_angle > FRAC_PI_2 + 1e-9 {
        best_angle -= PI;
    }
    while best_angle < -FRAC_PI_2 - 1e-9 {
        best_angle += PI;
    }
    (best_angle, best_size)
}

/// Rotates by angle (radians) and scales about a pivot.
pub fn transform_chart(ch: &mut Chart, pivot: DVec2, angle: f64, scale: DVec2, offset: DVec2) {
    let (c, s) = (angle.cos(), angle.sin());
    for p in ch.uv.iter_mut() {
        *p = pivot + offset + rotate((*p - pivot) * scale, c, s);
    }
}

/// Rotates the chart about its uv centroid.
pub fn orient_chart(ch: &mut Chart, mode: OrientMode) {
    if ch.uv.len() < 2 {
        return;
    }
    let pivot = ch.uv_centroid();
    let mut angle = 0.0;
    match mode {
        OrientMode::BoundingBox => angle = min_area_rect_angle(&ch.uv).0,
        OrientMode::Principal => {
            let (mut sxx, mut syy, mut sxy) = (0.0, 0.0, 0.0);
            for t in &ch.tris {
                let (u0, u1, u2) = (ch.uv[t[0] as usize], ch.uv[t[1] as usize], ch.uv[t[2] as usize]);
                let (e1, e2) = (u1 - u0, u2 - u0);
                let a = (e1.x * e2.y - e1.y * e2.x).abs();
                let d = (u0 + u1 + u2) / 3.0 - pivot;
                sxx += a * d.x * d.x;
                syy += a * d.y * d.y;
                sxy += a * d.x * d.y;
            }
            angle = -0.5 * (2.0 * sxy).atan2(sxx - syy);
        }
        OrientMode::LongestEdge => {
            let mut best = -1.0;
            for lp in ch.boundary_loops() {
                for i in 0..lp.len() {
                    let e = ch.uv[lp[(i + 1) % lp.len()] as usize] - ch.uv[lp[i] as usize];
                    let l = e.length();
                    if l > best {
                        best = l;
                        let a = e.y.atan2(e.x);
                        // Snap to whichever axis is nearest.
                        angle = -(a - (a / FRAC_PI_2).round() * FRAC_PI_2);
                    }
                }
            }
        }
    }
    transform_chart(ch, pivot, angle, DVec2::ONE, DVec2::ZERO);
}

/// Moves and scales the chart so its centroid and area match the reference ones (when positive).
pub fn fit_chart(ch: &mut Chart, centroid: DVec2, area: f64) {
    let cur = ch.area_uv().abs();
    let s = if area > 0.0 && cur > 0.0 { (area / cur).sqrt() } else { 1.0 };
    let c = ch.uv_centroid();
    for p in ch.uv.iter_mut() {
        *p = centroid + (*p - c) * s;
    }
}

/// RizomUV-style rectify. Quad grids (strips, panels, cylinders cut once) become exact grids with column widths and row
/// heights from their 3D edge lengths; other disk islands are fitted to a rectangle. The result keeps the island's
/// previous uv centroid and area when it had valid uvs.
pub fn straighten_chart(ch: &mut Chart) -> StraightenResult {
    let had_uvs = valid_uvs(ch);
    let centroid = ch.uv_centroid();
    let area = ch.area_uv().abs();

    let mut r = StraightenResult::Failed;
    if ch.euler() == 1 {
        if let Some(coord) = grid_coords(ch) {
            lay_grid(ch, &coord);
            r = StraightenResult::Grid;
        }
    }
    if r == StraightenResult::Failed && ch.euler() == 1 && lay_rectangle(ch) {
        r = StraightenResult::Rectangle;
    }
    if r == StraightenResult::Failed {
        return r;
    }
    if ch.area_uv() < 0.0 {
        for p in ch.uv.iter_mut() {
            p.x = -p.x;
        }
    }
    if had_uvs {
        fit_chart(ch, centroid, area);
    }
    r
}
