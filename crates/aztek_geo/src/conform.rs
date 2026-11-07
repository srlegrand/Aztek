//! Shape conform: fits a component selection to a line, circle, rectangle, sphere, cylinder or plane, or to another
//! mesh. Each op only works out where the points go (object space); the caller blends, mirrors and writes them.

use std::f64::consts::{FRAC_PI_2, TAU};

use glam::{DMat3, DMat4, DVec2, DVec3, DVec4};
use rayon::prelude::*;

use crate::attribute::{Index, INVALID};
use crate::bvh::MeshBvh;
use crate::mesh::Mesh;
use crate::topology::{
    boundary_loops, chain_edges, edge_loop, make_mask, points_of_faces, region_boundary_edges, unique_sorted, EdgePath,
};

/// A selection seen as point chains: edge chains, the borders of face regions, or chains through selected points.
#[derive(Clone, Debug, Default)]
pub struct ShapeSelection {
    pub chains: Vec<EdgePath>,
    /// Inside selected face regions, off their borders: they follow the border's moves.
    pub interior: Vec<Index>,
    /// Selected points on no chain and in no region: fitted as a point set.
    pub loose: Vec<Index>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointMove {
    pub point: Index,
    pub position: DVec3,
}

impl Default for PointMove {
    fn default() -> Self {
        Self { point: INVALID, position: DVec3::ZERO }
    }
}

pub type PointMoves = Vec<PointMove>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Spacing {
    /// As far along the shape as the point was along its chain.
    #[default]
    Keep,
    /// Equal steps.
    Even,
    /// The nearest point of the shape.
    Project,
}

#[derive(Clone, Debug, Default)]
pub struct StraightenOptions {
    pub spacing: Spacing,
    /// Through all the chain's points rather than between its ends.
    pub fit_line: bool,
}

#[derive(Clone, Debug)]
pub struct CircleOptions {
    pub spacing: Spacing,
    /// 0: the best fit's.
    pub radius: f64,
    /// Degrees about the circle's normal, past the least-movement alignment.
    pub rotation: f64,
    /// 3 or more: a regular polygon with its corners on the circle.
    pub sides: i32,
    /// False keeps each point's height off the circle's plane.
    pub flatten: bool,
}

impl Default for CircleOptions {
    fn default() -> Self {
        Self { spacing: Spacing::Even, radius: 0.0, rotation: 0.0, sides: 0, flatten: true }
    }
}

#[derive(Clone, Debug)]
pub struct RectangleOptions {
    pub spacing: Spacing,
    pub square: bool,
    /// Degrees.
    pub rotation: f64,
    pub flatten: bool,
}

impl Default for RectangleOptions {
    fn default() -> Self {
        Self { spacing: Spacing::Keep, square: false, rotation: 0.0, flatten: true }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SphereFit {
    /// Least-squares sphere through the points.
    #[default]
    Best,
    /// Centred on their centroid.
    Center,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum SphereRadius {
    /// The least-squares sphere's radius (the mean distance when centred on the centroid).
    Fit,
    /// The points' mean distance from the centre.
    #[default]
    Average,
    /// The nearest point's distance.
    Min,
    /// The farthest point's distance.
    Max,
    /// SpherizeOptions::radius.
    Explicit,
}

#[derive(Clone, Debug)]
pub struct SpherizeOptions<'a> {
    pub fit: SphereFit,
    pub radius_mode: SphereRadius,
    /// How far each point goes toward the sphere, 0 to 1.
    pub amount: f64,
    /// Above 0 this radius is used whatever radius_mode says.
    pub radius: f64,
    /// Object space; unset takes the fitted centre.
    pub center: Option<DVec3>,
    /// Ellipsoid semi-axes relative to the radius, along the columns of frame.
    pub scale: DVec3,
    /// Orthonormal ellipsoid axes in object space, one per column.
    pub frame: DMat3,
    /// Each point slides along its own normal to the sphere instead of straight toward the centre.
    pub along_normals: bool,
    /// Per-point normals (object space, indexed by point) for along_normals; unset takes the area-weighted ones.
    pub normals: Option<&'a [DVec3]>,
}

impl Default for SpherizeOptions<'_> {
    fn default() -> Self {
        Self {
            fit: SphereFit::Best,
            radius_mode: SphereRadius::Average,
            amount: 1.0,
            radius: 0.0,
            center: None,
            scale: DVec3::ONE,
            frame: DMat3::IDENTITY,
            along_normals: false,
            normals: None,
        }
    }
}

/// The sphere (or ellipsoid, with SpherizeOptions::scale) the points were fitted to.
#[derive(Clone, Copy, Debug, Default)]
pub struct SpherizeResult {
    pub center: DVec3,
    /// In the ellipsoid's unit space; the semi-axes are radius * scale.
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ConformMode {
    /// Closed chains one to one onto the target's nearest closed loop.
    #[default]
    Loop,
    /// Every point to the nearest point of the target.
    Surface,
    /// Every point to the nearest target vertex.
    Vertex,
}

#[derive(Clone, Debug)]
pub struct ConformOptions {
    pub mode: ConformMode,
    /// Loop mode, when the counts differ: how the points spread along the target loop.
    pub spacing: Spacing,
}

impl Default for ConformOptions {
    fn default() -> Self {
        Self { mode: ConformMode::Loop, spacing: Spacing::Even }
    }
}

type PointList = Vec<DVec3>;

fn centroid_of(pts: &[DVec3]) -> DVec3 {
    let c: DVec3 = pts.iter().fold(DVec3::ZERO, |a, &p| a + p);
    if pts.is_empty() {
        c
    } else {
        c / pts.len() as f64
    }
}

fn outer3(c: DVec3, r: DVec3) -> DMat3 {
    DMat3::from_cols(c * r.x, c * r.y, c * r.z)
}

/// Eigen decomposition of a symmetric 3x3 by cyclic Jacobi rotations; values descending, vectors as columns.
fn symmetric_eigen(a: DMat3) -> (DVec3, DMat3) {
    // m[col][row], as glm.
    let mut a = a.to_cols_array_2d();
    let mut v = DMat3::IDENTITY;
    for _ in 0..64 {
        let (mut p, mut q) = (0usize, 1usize);
        let mut big = a[1][0].abs();
        if a[2][0].abs() > big {
            p = 0;
            q = 2;
            big = a[2][0].abs();
        }
        if a[2][1].abs() > big {
            p = 1;
            q = 2;
            big = a[2][1].abs();
        }
        if big < 1e-14 {
            break;
        }
        let theta = (a[q][q] - a[p][p]) / (2.0 * a[q][p]);
        let t = if theta >= 0.0 { 1.0 } else { -1.0 } / (theta.abs() + (theta * theta + 1.0).sqrt());
        let c = 1.0 / (t * t + 1.0).sqrt();
        let s = t * c;
        let mut r = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        r[p][p] = c;
        r[q][q] = c;
        r[q][p] = s;
        r[p][q] = -s;
        let rm = DMat3::from_cols_array_2d(&r);
        a = (rm.transpose() * DMat3::from_cols_array_2d(&a) * rm).to_cols_array_2d();
        v = v * rm;
    }
    let mut order = [0usize, 1, 2];
    order.sort_by(|&i, &j| a[j][j].partial_cmp(&a[i][i]).unwrap_or(std::cmp::Ordering::Equal));
    let mut values = DVec3::ZERO;
    let mut cols = [DVec3::ZERO; 3];
    for k in 0..3 {
        values[k] = a[order[k]][order[k]];
        cols[k] = v.col(order[k]);
    }
    (values, DMat3::from_cols(cols[0], cols[1], cols[2]))
}

/// Principal axes of the points about their centroid: column 0 the longest spread, column 2 the plane normal.
fn sorted_principal_axes(pts: &[DVec3]) -> (DMat3, DVec3) {
    let c = centroid_of(pts);
    let mut cov = DMat3::ZERO;
    for &p in pts {
        cov += outer3(p - c, p - c);
    }
    let (values, vectors) = symmetric_eigen(cov);
    (vectors, values)
}

fn axes(pts: &[DVec3]) -> DMat3 {
    sorted_principal_axes(pts).0
}

fn any_perpendicular(n: DVec3) -> DVec3 {
    let a = if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y };
    n.cross(a).normalize()
}

/// An orthonormal frame in a plane through origin; (u, v, n) is right-handed.
#[derive(Clone, Copy)]
struct PlaneFrame {
    origin: DVec3,
    u: DVec3,
    v: DVec3,
    n: DVec3,
}

impl Default for PlaneFrame {
    fn default() -> Self {
        Self { origin: DVec3::ZERO, u: DVec3::X, v: DVec3::Y, n: DVec3::Z }
    }
}

impl PlaneFrame {
    fn new(o: DVec3, normal: DVec3) -> Self {
        let n = normal.normalize();
        let u = any_perpendicular(n);
        Self { origin: o, u, v: n.cross(u), n }
    }
    fn flat(&self, p: DVec3) -> DVec2 {
        DVec2::new((p - self.origin).dot(self.u), (p - self.origin).dot(self.v))
    }
    fn height(&self, p: DVec3) -> f64 {
        (p - self.origin).dot(self.n)
    }
    fn lift(&self, q: DVec2, h: f64) -> DVec3 {
        self.origin + self.u * q.x + self.v * q.y + self.n * h
    }
}

/// Newell normal of a closed polygon, unnormalised; zero when it encloses no area.
fn newell(pts: &[DVec3]) -> DVec3 {
    let c = centroid_of(pts);
    let mut sum = DVec3::ZERO;
    for i in 0..pts.len() {
        sum += (pts[i] - c).cross(pts[(i + 1) % pts.len()] - c);
    }
    sum
}

fn extent_of(pts: &[DVec3]) -> f64 {
    if pts.is_empty() {
        return 0.0;
    }
    let mut lo = DVec3::splat(f64::MAX);
    let mut hi = DVec3::splat(-f64::MAX);
    for &p in pts {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    (hi - lo).length()
}

/// Frame of a loop's plane: its winding normal when it has one, else the points' flattest direction.
fn loop_frame(pts: &[DVec3], closed: bool) -> PlaneFrame {
    let c = centroid_of(pts);
    let nw = if closed { newell(pts) } else { DVec3::ZERO };
    let scale = extent_of(pts);
    if nw.length() > 1e-9 * scale * scale {
        return PlaneFrame::new(c, nw);
    }
    PlaneFrame::new(c, axes(pts).col(2))
}

/// Least-squares (Kasa) circle through 2D points. None when they are near collinear.
fn fit_circle_2d(q: &[DVec2]) -> Option<(DVec2, f64)> {
    if q.len() < 3 {
        return None;
    }
    let mean = q.iter().fold(DVec2::ZERO, |a, &p| a + p) / q.len() as f64;
    let mut s = 0.0;
    for &p in q {
        s += (p - mean).dot(p - mean);
    }
    s = (s / q.len() as f64).sqrt();
    if s < 1e-12 {
        return None;
    }
    let mut m = DMat3::ZERO;
    let mut rhs = DVec3::ZERO;
    for &p in q {
        let d = (p - mean) / s;
        let row = DVec3::new(d.x, d.y, 1.0);
        m += outer3(row, row);
        rhs -= row * d.dot(d);
    }
    let n = q.len() as f64;
    if m.determinant().abs() < 1e-9 * n * n * n {
        return None;
    }
    let sol = m.inverse() * rhs;
    let c = DVec2::new(-sol.x * 0.5, -sol.y * 0.5);
    let r2 = c.dot(c) - sol.z;
    if r2 <= 0.0 || c.length() > 1e3 {
        return None;
    }
    Some((mean + c * s, r2.sqrt() * s))
}

/// Least-squares sphere. None for near coplanar points, whose sphere is undetermined.
fn fit_sphere(pts: &[DVec3]) -> Option<(DVec3, f64)> {
    if pts.len() < 4 {
        return None;
    }
    let (_, spread) = sorted_principal_axes(pts);
    if spread[2] < 1e-8 * spread[0] {
        return None;
    }
    let mean = centroid_of(pts);
    let mut s = 0.0;
    for &p in pts {
        s += (p - mean).dot(p - mean);
    }
    s = (s / pts.len() as f64).sqrt();
    let mut m = DMat4::ZERO;
    let mut rhs = DVec4::ZERO;
    for &p in pts {
        let d = (p - mean) / s;
        let row = d.extend(1.0);
        m += DMat4::from_cols(row * row.x, row * row.y, row * row.z, row * row.w);
        rhs -= row * d.dot(d);
    }
    if m.determinant().abs() < 1e-12 * (pts.len() as f64).powf(4.0) {
        return None;
    }
    let sol = m.inverse() * rhs;
    let c = DVec3::new(-sol.x * 0.5, -sol.y * 0.5, -sol.z * 0.5);
    let r2 = c.dot(c) - sol.w;
    if r2 <= 0.0 || c.length() > 1e2 {
        return None;
    }
    Some((mean + c * s, r2.sqrt() * s))
}

fn positions_of(mesh: &Mesh, ids: &[Index]) -> PointList {
    ids.iter().map(|&p| mesh.position(p)).collect()
}

/// Fraction of the way along the chain at each point: by length (keep) or by count (even). Closed chains end short
/// of 1.
fn chain_fractions(pts: &[DVec3], closed: bool, spacing: Spacing) -> Vec<f64> {
    let n = pts.len();
    let mut f = vec![0.0; n];
    if n < 2 {
        return f;
    }
    let steps = if closed { n } else { n - 1 };
    if steps == 0 {
        return f;
    }
    let mut cum = vec![0.0; n + 1];
    for i in 0..steps {
        cum[i + 1] = cum[i] + (pts[(i + 1) % n] - pts[i]).length();
    }
    for i in 0..n {
        f[i] = if spacing == Spacing::Even || cum[steps] <= 1e-12 {
            i as f64 / steps as f64
        } else {
            cum[i] / cum[steps]
        };
    }
    f
}

fn wrap_angle(a: f64) -> f64 {
    let a = a % TAU;
    if a < 0.0 {
        a + TAU
    } else {
        a
    }
}

/// IEEE remainder, as std::remainder.
fn remainder(x: f64, y: f64) -> f64 {
    x - (x / y).round_ties_even() * y
}

/// Start angle that puts points at start + tau * f[i] as near as possible to their angles phi[i].
fn best_start(phi: &[f64], f: &[f64]) -> f64 {
    let (mut s, mut c) = (0.0, 0.0);
    for i in 0..phi.len() {
        s += (phi[i] - TAU * f[i]).sin();
        c += (phi[i] - TAU * f[i]).cos();
    }
    s.atan2(c)
}

fn mix2(a: DVec2, b: DVec2, t: f64) -> DVec2 {
    a * (1.0 - t) + b * t
}

fn mix3(a: DVec3, b: DVec3, t: f64) -> DVec3 {
    a * (1.0 - t) + b * t
}

fn on_polygon(theta: f64, start: f64, sides: i32, c: DVec2, r: f64) -> DVec2 {
    let step = TAU / sides as f64;
    let rel = wrap_angle(theta - start) / step;
    let k = (rel as i32).min(sides - 1);
    let t = rel - k as f64;
    let a0 = start + step * k as f64;
    let a1 = a0 + step;
    let p0 = c + r * DVec2::new(a0.cos(), a0.sin());
    let p1 = c + r * DVec2::new(a1.cos(), a1.sin());
    mix2(p0, p1, t)
}

fn circle_of_loop(pts: &[DVec3], o: &CircleOptions, radii: &mut Option<&mut Vec<f64>>) -> PointList {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let frame = loop_frame(pts, true);
    let q: Vec<DVec2> = pts.iter().map(|&p| frame.flat(p)).collect();
    let c = match fit_circle_2d(&q) {
        Some((c, _)) => c,
        None => q.iter().fold(DVec2::ZERO, |a, &p| a + p) / n as f64,
    };
    let mut r = 0.0;
    let mut phi = vec![0.0; n];
    for i in 0..n {
        r += (q[i] - c).length();
        phi[i] = (q[i].y - c.y).atan2(q[i].x - c.x);
    }
    r = if o.radius > 0.0 { o.radius } else { r / n as f64 };
    if let Some(radii) = radii {
        radii.push(r);
    }
    let f = chain_fractions(pts, true, if o.spacing == Spacing::Even { Spacing::Even } else { Spacing::Keep });
    let rot = o.rotation.to_radians();
    let start = best_start(&phi, &f) + rot;
    (0..n)
        .map(|i| {
            let theta = if o.spacing == Spacing::Project { phi[i] + rot } else { start + TAU * f[i] };
            let at = if o.sides >= 3 {
                on_polygon(theta, start, o.sides, c, r)
            } else {
                c + r * DVec2::new(theta.cos(), theta.sin())
            };
            frame.lift(at, if o.flatten { 0.0 } else { frame.height(pts[i]) })
        })
        .collect()
}

/// An open chain onto the arc of its best-fit circle between its ends; unchanged when it is straight.
fn circle_of_arc(pts: &[DVec3], o: &CircleOptions, radii: &mut Option<&mut Vec<f64>>) -> PointList {
    let n = pts.len();
    if n < 3 {
        return pts.to_vec();
    }
    let frame = PlaneFrame::new(centroid_of(pts), axes(pts).col(2));
    let q: Vec<DVec2> = pts.iter().map(|&p| frame.flat(p)).collect();
    let Some((c, fitted)) = fit_circle_2d(&q) else { return pts.to_vec() };
    let r = if o.radius > 0.0 { o.radius } else { fitted };
    if let Some(radii) = radii {
        radii.push(r);
    }
    let mut phi = vec![0.0; n];
    phi[0] = (q[0].y - c.y).atan2(q[0].x - c.x);
    for i in 1..n {
        let a = (q[i].y - c.y).atan2(q[i].x - c.x);
        phi[i] = phi[i - 1] + remainder(a - phi[i - 1], TAU);
    }
    let f = chain_fractions(pts, false, o.spacing);
    (0..n)
        .map(|i| {
            let theta = if o.spacing == Spacing::Project { phi[i] } else { phi[0] + (phi[n - 1] - phi[0]) * f[i] };
            frame.lift(c + r * DVec2::new(theta.cos(), theta.sin()), if o.flatten { 0.0 } else { frame.height(pts[i]) })
        })
        .collect()
}

/// Loose points ordered by their angle about their centroid in their best-fit plane, as a loop.
fn angle_order(mesh: &Mesh, ids: &[Index]) -> Vec<Index> {
    let pts = positions_of(mesh, ids);
    let frame = PlaneFrame::new(centroid_of(&pts), axes(&pts).col(2));
    let mut keyed: Vec<(f64, Index)> = ids
        .iter()
        .enumerate()
        .map(|(i, &id)| {
            let q = frame.flat(pts[i]);
            (q.y.atan2(q.x), id)
        })
        .collect();
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
    keyed.into_iter().map(|k| k.1).collect()
}

fn add_moves(moves: &mut PointMoves, ids: &[Index], to: &[DVec3]) {
    for (i, &id) in ids.iter().enumerate() {
        moves.push(PointMove { point: id, position: to[i] });
    }
}

fn rectangle_of_loop(pts: &[DVec3], o: &RectangleOptions) -> PointList {
    let n = pts.len();
    if n < 4 {
        return pts.to_vec();
    }
    let frame = loop_frame(pts, true);
    let q: Vec<DVec2> = pts.iter().map(|&p| frame.flat(p)).collect();

    // The smallest bounding rectangle has a side along a hull edge, and every hull edge is some chord of the loop's
    // own edges' directions.
    let bounds_at = |a: f64| {
        let e1 = DVec2::new(a.cos(), a.sin());
        let e2 = DVec2::new(-e1.y, e1.x);
        let mut lo = DVec2::splat(f64::MAX);
        let mut hi = -lo;
        for &p in &q {
            let l = DVec2::new(p.dot(e1), p.dot(e2));
            lo = lo.min(l);
            hi = hi.max(l);
        }
        ((hi.x - lo.x) * (hi.y - lo.y), lo, hi)
    };
    let mut best_angle = 0.0;
    let mut best_area = f64::MAX;
    for i in 0..n {
        let d = q[(i + 1) % n] - q[i];
        if d.dot(d) < 1e-24 {
            continue;
        }
        let a = (d.y.atan2(d.x) + TAU) % FRAC_PI_2;
        let (area, _, _) = bounds_at(a);
        if area < best_area - 1e-12 * area.abs() {
            best_area = area;
            best_angle = a;
        }
    }
    let (_, lo, hi) = bounds_at(best_angle);
    let e1 = DVec2::new(best_angle.cos(), best_angle.sin());
    let e2 = DVec2::new(-e1.y, e1.x);
    let center = e1 * ((lo.x + hi.x) * 0.5) + e2 * ((lo.y + hi.y) * 0.5);
    let mut hx = (hi.x - lo.x) * 0.5;
    let mut hy = (hi.y - lo.y) * 0.5;
    if o.square {
        hx = (hx + hy) * 0.5;
        hy = hx;
    }
    let rot = o.rotation.to_radians();
    let r1 = e1 * rot.cos() + e2 * rot.sin();
    let r2 = DVec2::new(-r1.y, r1.x);
    let corner = [center - r1 * hx - r2 * hy, center + r1 * hx - r2 * hy, center + r1 * hx + r2 * hy, center - r1 * hx + r2 * hy];

    let mut at = [0usize; 4];
    for k in 0..4 {
        let mut best = f64::MAX;
        for i in 0..n {
            let d = (q[i] - corner[k]).length();
            if d < best {
                best = d;
                at[k] = i;
            }
        }
    }
    let ahead = |at: &[usize; 4], k: usize| (at[k] + n - at[0]) % n;
    if !(ahead(&at, 1) > 0 && ahead(&at, 1) < ahead(&at, 2) && ahead(&at, 2) < ahead(&at, 3)) {
        for k in 1..4 {
            at[k] = (at[0] + (k as f64 * n as f64 / 4.0).round() as usize) % n;
        }
    }

    let mut out = vec![DVec3::ZERO; n];
    for k in 0..4 {
        let from = at[k];
        let count = (at[(k + 1) % 4] + n - from) % n;
        let a = corner[k];
        let b = corner[(k + 1) % 4];
        let side: PointList = (0..=count).map(|i| pts[(from + i) % n]).collect();
        let f = chain_fractions(&side, false, if o.spacing == Spacing::Even { Spacing::Even } else { Spacing::Keep });
        for i in 0..count {
            let idx = (from + i) % n;
            let mut t = f[i];
            if o.spacing == Spacing::Project && i > 0 {
                let ab = b - a;
                let len2 = ab.dot(ab);
                t = if len2 > 1e-24 { ((q[idx] - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
            }
            out[idx] = frame.lift(mix2(a, b, t), if o.flatten { 0.0 } else { frame.height(pts[idx]) });
        }
    }
    out
}

/// A polyline with its cumulative lengths, for walking along a loop.
struct Polyline {
    p: PointList,
    /// cum[i]: length to point i; last: whole length.
    cum: Vec<f64>,
    closed: bool,
}

impl Polyline {
    fn new(p: PointList, closed: bool) -> Self {
        let steps = if closed { p.len() } else { p.len() - 1 };
        let mut cum = vec![0.0; steps + 1];
        for i in 0..steps {
            cum[i + 1] = cum[i] + (p[(i + 1) % p.len()] - p[i]).length();
        }
        Self { p, cum, closed }
    }
    fn length(&self) -> f64 {
        *self.cum.last().unwrap()
    }
    fn segments(&self) -> usize {
        self.cum.len() - 1
    }

    /// Nearest point, its arc length and squared distance.
    fn nearest(&self, x: DVec3) -> (DVec3, f64, f64) {
        let mut best = self.p[0];
        let mut best_d = (x - self.p[0]).dot(x - self.p[0]);
        let mut best_s = 0.0;
        for i in 0..self.segments() {
            let a = self.p[i];
            let b = self.p[(i + 1) % self.p.len()];
            let ab = b - a;
            let len2 = ab.dot(ab);
            let t = if len2 > 1e-24 { ((x - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
            let at = a + ab * t;
            let d = (x - at).dot(x - at);
            if d < best_d {
                best_d = d;
                best = at;
                best_s = self.cum[i] + (self.cum[i + 1] - self.cum[i]) * t;
            }
        }
        (best, best_s, best_d)
    }

    fn at(&self, s: f64) -> DVec3 {
        let len = self.length();
        if len <= 0.0 {
            return self.p[0];
        }
        let s = if self.closed { ((s % len) + len) % len } else { s.clamp(0.0, len) };
        let i = self.cum.partition_point(|&c| c <= s);
        let i = i.clamp(1, self.segments()) - 1;
        let seg = self.cum[i + 1] - self.cum[i];
        let t = if seg > 0.0 { (s - self.cum[i]) / seg } else { 0.0 };
        mix3(self.p[i], self.p[(i + 1) % self.p.len()], t)
    }
}

/// Path through ordered edges, or nothing when they do not join up end to end.
fn path_of_edges(mesh: &Mesh, edges: &[Index]) -> Option<EdgePath> {
    if edges.len() < 2 {
        return None;
    }
    let (e0, e1) = (edges[0], edges[1]);
    let mut start = mesh.edge_point(e0, 0);
    if start == mesh.edge_point(e1, 0) || start == mesh.edge_point(e1, 1) {
        start = mesh.edge_point(e0, 1);
    }
    let mut path = EdgePath::default();
    let mut p = start;
    for &e in edges {
        if mesh.edge_point(e, 0) != p && mesh.edge_point(e, 1) != p {
            return None;
        }
        path.points.push(p);
        path.edges.push(e);
        p = mesh.edge_other_point(e, p);
    }
    path.closed = p == start;
    if !path.closed {
        path.points.push(p);
    }
    Some(path)
}

fn faces_covered_by(mesh: &Mesh, inside: &[u8], points: &[Index]) -> Vec<Index> {
    let mut faces = Vec::new();
    for &p in points {
        for f in mesh.point_faces(p) {
            let mut all = true;
            mesh.for_each_face_corner(f, |c| all = all && inside[mesh.corner_point(c) as usize] != 0);
            if all {
                faces.push(f);
            }
        }
    }
    unique_sorted(&faces)
}

fn append_shape(to: &mut ShapeSelection, from: ShapeSelection) {
    to.chains.extend(from.chains);
    to.interior.extend(from.interior);
    to.loose.extend(from.loose);
}

/// Nearest point on any of the target's edges, for targets without faces.
fn nearest_on_edges(target: &Mesh, x: DVec3) -> DVec3 {
    let mut best = x;
    let mut best_d = f64::MAX;
    for e in 0..target.edge_capacity() {
        if !target.edge_valid(e) {
            continue;
        }
        let a = target.position(target.edge_point(e, 0));
        let ab = target.position(target.edge_point(e, 1)) - a;
        let len2 = ab.dot(ab);
        let t = if len2 > 1e-24 { ((x - a).dot(ab) / len2).clamp(0.0, 1.0) } else { 0.0 };
        let at = a + ab * t;
        let d = (x - at).dot(x - at);
        if d < best_d {
            best_d = d;
            best = at;
        }
    }
    best
}

/// Closed chain points spread onto a loop: one to one when the counts match, else by spacing at the best offset and
/// direction.
fn onto_loop(src: &[DVec3], lp: &Polyline, spacing: Spacing) -> PointList {
    let n = src.len();
    let m = lp.p.len();
    let mut out = vec![DVec3::ZERO; n];
    if spacing == Spacing::Project {
        for i in 0..n {
            out[i] = lp.nearest(src[i]).0;
        }
        return out;
    }
    let mut best_cost = f64::MAX;
    if n == m {
        for forward in [true, false] {
            for shift in 0..m {
                let slot = |i: usize| if forward { (shift + i) % m } else { (shift + m - i % m) % m };
                let mut cost = 0.0;
                for i in 0..n {
                    if cost >= best_cost {
                        break;
                    }
                    let d = src[i] - lp.p[slot(i)];
                    cost += d.dot(d);
                }
                if cost < best_cost {
                    best_cost = cost;
                    for i in 0..n {
                        out[i] = lp.p[slot(i)];
                    }
                }
            }
        }
        return out;
    }
    let len = lp.length();
    let f = chain_fractions(src, true, spacing);
    let along: Vec<f64> = src.iter().map(|&s| lp.nearest(s).1).collect();
    for dir in [1.0, -1.0] {
        let phi: Vec<f64> = (0..n).map(|i| TAU * along[i] / len).collect();
        let g: Vec<f64> = (0..n).map(|i| dir * f[i]).collect();
        let start = best_start(&phi, &g) / TAU * len;
        let mut cand = vec![DVec3::ZERO; n];
        let mut cost = 0.0;
        for i in 0..n {
            cand[i] = lp.at(start + dir * f[i] * len);
            cost += (src[i] - cand[i]).dot(src[i] - cand[i]);
        }
        if cost < best_cost {
            best_cost = cost;
            out = cand;
        }
    }
    out
}

fn mean_distance(pts: &[DVec3], lp: &Polyline) -> f64 {
    let sum: f64 = pts.iter().map(|&p| lp.nearest(p).2.sqrt()).sum();
    sum / pts.len().max(1) as f64
}

pub fn shape_of_edges(mesh: &Mesh, edges: &[Index]) -> ShapeSelection {
    let valid: Vec<Index> = edges.iter().copied().filter(|&e| mesh.edge_valid(e)).collect();
    ShapeSelection { chains: chain_edges(mesh, &unique_sorted(&valid)), ..Default::default() }
}

/// Each face region's border loops, with its other points as interior.
pub fn shape_of_faces(mesh: &Mesh, faces: &[Index]) -> ShapeSelection {
    let valid: Vec<Index> = faces.iter().copied().filter(|&f| mesh.face_valid(f)).collect();
    let valid = unique_sorted(&valid);
    let mut s = ShapeSelection { chains: chain_edges(mesh, &region_boundary_edges(mesh, &valid)), ..Default::default() };
    let mut on_border = vec![0u8; mesh.point_capacity() as usize];
    for c in &s.chains {
        for &p in &c.points {
            on_border[p as usize] = 1;
        }
    }
    for p in points_of_faces(mesh, &valid) {
        if on_border[p as usize] == 0 {
            s.interior.push(p);
        }
    }
    s
}

/// Points covering whole faces act as those faces; the rest chain along the edges joining selected points.
pub fn shape_of_points(mesh: &Mesh, points: &[Index]) -> ShapeSelection {
    let valid: Vec<Index> = points.iter().copied().filter(|&p| mesh.point_valid(p)).collect();
    let valid = unique_sorted(&valid);
    let mut inside = make_mask(mesh.point_capacity() as usize, &valid);

    let mut s = ShapeSelection::default();
    let covered = faces_covered_by(mesh, &inside, &valid);
    if !covered.is_empty() {
        for p in points_of_faces(mesh, &covered) {
            inside[p as usize] = 0;
        }
        append_shape(&mut s, shape_of_faces(mesh, &covered));
    }
    let rest: Vec<Index> = valid.iter().copied().filter(|&p| inside[p as usize] != 0).collect();
    let mut joining = Vec::new();
    for &p in &rest {
        mesh.for_each_point_edge(p, |e| {
            if inside[mesh.edge_other_point(e, p) as usize] != 0 {
                joining.push(e);
            }
        });
    }
    let chains = chain_edges(mesh, &unique_sorted(&joining));
    for c in &chains {
        for &p in &c.points {
            inside[p as usize] = 0;
        }
    }
    s.chains.extend(chains);
    for p in rest {
        if inside[p as usize] != 0 {
            s.loose.push(p);
        }
    }
    s
}

/// Every point of the selection, once.
pub fn shape_points(sel: &ShapeSelection) -> Vec<Index> {
    let mut all: Vec<Index> = sel.interior.clone();
    all.extend_from_slice(&sel.loose);
    for c in &sel.chains {
        all.extend_from_slice(&c.points);
    }
    unique_sorted(&all)
}

/// Open chains onto the line between their ends; loose points onto their best-fit line. Closed chains are left.
pub fn straighten(mesh: &Mesh, sel: &ShapeSelection, opts: &StraightenOptions) -> PointMoves {
    let mut moves = PointMoves::new();
    let on_line = |moves: &mut PointMoves, ids: &[Index], a: DVec3, b: DVec3, f: &[f64]| {
        let ab = b - a;
        let len2 = ab.dot(ab);
        for (i, &id) in ids.iter().enumerate() {
            let mut t = f[i];
            if opts.spacing == Spacing::Project {
                t = if len2 > 1e-24 { (mesh.position(id) - a).dot(ab) / len2 } else { 0.0 };
            }
            moves.push(PointMove { point: id, position: a + ab * t });
        }
    };
    for c in &sel.chains {
        if c.closed || c.points.len() < 3 {
            continue;
        }
        let pts = positions_of(mesh, &c.points);
        let mut a = pts[0];
        let mut b = *pts.last().unwrap();
        if opts.fit_line {
            let o = centroid_of(&pts);
            let d = axes(&pts).col(0);
            a = o + d * (a - o).dot(d);
            b = o + d * (b - o).dot(d);
        }
        on_line(&mut moves, &c.points, a, b, &chain_fractions(&pts, false, opts.spacing));
    }
    if sel.loose.len() >= 2 {
        let pts = positions_of(mesh, &sel.loose);
        let o = centroid_of(&pts);
        let d = axes(&pts).col(0);
        let mut keyed: Vec<(f64, Index)> = pts.iter().zip(&sel.loose).map(|(&p, &id)| ((p - o).dot(d), id)).collect();
        keyed.sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.cmp(&b.1)));
        let ids: Vec<Index> = keyed.iter().map(|k| k.1).collect();
        let front = keyed[0].0;
        let back = keyed.last().unwrap().0;
        let a = o + d * front;
        let b = o + d * back;
        let f: Vec<f64> = (0..ids.len())
            .map(|i| {
                if opts.spacing == Spacing::Even {
                    i as f64 / (ids.len() - 1) as f64
                } else {
                    (keyed[i].0 - front) / (back - front).max(1e-12)
                }
            })
            .collect();
        on_line(&mut moves, &ids, a, b, &f);
    }
    moves
}

/// Closed chains onto their best-fit circle, aligned to move the points least. Open chains become an arc between
/// their ends; loose points are taken in angle order as one loop. `radii`, when given, receives each circle's radius
/// used.
pub fn circularize(mesh: &Mesh, sel: &ShapeSelection, opts: &CircleOptions, mut radii: Option<&mut Vec<f64>>) -> PointMoves {
    let mut moves = PointMoves::new();
    for c in &sel.chains {
        let pts = positions_of(mesh, &c.points);
        let to = if c.closed { circle_of_loop(&pts, opts, &mut radii) } else { circle_of_arc(&pts, opts, &mut radii) };
        add_moves(&mut moves, &c.points, &to);
    }
    if sel.loose.len() >= 3 {
        let ids = angle_order(mesh, &sel.loose);
        let to = circle_of_loop(&positions_of(mesh, &ids), opts, &mut radii);
        add_moves(&mut moves, &ids, &to);
    }
    moves
}

/// Closed chains onto their smallest oriented bounding rectangle in their plane. The four points nearest its corners
/// become the corners; the rest spread along the sides between them.
pub fn rectangularize(mesh: &Mesh, sel: &ShapeSelection, opts: &RectangleOptions) -> PointMoves {
    let mut moves = PointMoves::new();
    for c in &sel.chains {
        if c.closed {
            add_moves(&mut moves, &c.points, &rectangle_of_loop(&positions_of(mesh, &c.points), opts));
        }
    }
    if sel.loose.len() >= 4 {
        let ids = angle_order(mesh, &sel.loose);
        add_moves(&mut moves, &ids, &rectangle_of_loop(&positions_of(mesh, &ids), opts));
    }
    moves
}

/// Points moved onto a sphere or ellipsoid. A point at the centre stays put. With along_normals a point whose normal
/// line misses the sphere is pushed radially instead. `fitted`, when given, receives the sphere used.
pub fn spherize(mesh: &Mesh, points: &[Index], opts: &SpherizeOptions, fitted: Option<&mut SpherizeResult>) -> PointMoves {
    let pts = positions_of(mesh, points);
    if pts.len() < 2 {
        return PointMoves::new();
    }
    let s = opts.scale.abs().max(DVec3::splat(1e-6));
    let to_unit = DMat3::from_diagonal(DVec3::new(1.0 / s.x, 1.0 / s.y, 1.0 / s.z)) * opts.frame.transpose();
    let from_unit = opts.frame * DMat3::from_diagonal(s);
    let unit: PointList = pts.iter().map(|&p| to_unit * p).collect();

    let mut c = centroid_of(&unit);
    let mut fit_radius = 0.0;
    let mut fitted_best = false;
    if opts.fit == SphereFit::Best {
        if let Some((fc, fr)) = fit_sphere(&unit) {
            if fr <= 100.0 * extent_of(&unit) {
                c = fc;
                fit_radius = fr;
                fitted_best = true;
            }
        }
    }
    if !fitted_best {
        c = centroid_of(&unit);
    }
    if let Some(center) = opts.center {
        c = to_unit * center;
    }

    let mut mean = 0.0;
    let mut nearest = f64::MAX;
    let mut farthest = 0.0f64;
    for &p in &unit {
        let d = (p - c).length();
        mean += d;
        nearest = nearest.min(d);
        farthest = farthest.max(d);
    }
    mean /= unit.len() as f64;
    let r = if opts.radius > 0.0 {
        opts.radius
    } else if opts.radius_mode == SphereRadius::Fit && fitted_best && opts.center.is_none() {
        fit_radius
    } else if opts.radius_mode == SphereRadius::Min {
        nearest
    } else if opts.radius_mode == SphereRadius::Max {
        farthest
    } else {
        mean
    };
    if let Some(fitted) = fitted {
        *fitted = SpherizeResult { center: from_unit * c, radius: r };
    }

    let amount = opts.amount.clamp(0.0, 1.0);
    let mut moves = PointMoves::new();
    for i in 0..unit.len() {
        let d = unit[i] - c;
        let mut target: Option<DVec3> = None;
        if opts.along_normals {
            let mut n = DVec3::ZERO;
            if let Some(normals) = opts.normals {
                if (points[i] as usize) < normals.len() {
                    n = normals[points[i] as usize];
                }
            } else {
                for f in mesh.point_faces(points[i]) {
                    n += mesh.face_area_vector(f);
                }
            }
            n = to_unit * n;
            let a = n.dot(n);
            let b = 2.0 * d.dot(n);
            let k = d.dot(d) - r * r;
            let disc = b * b - 4.0 * a * k;
            if a > 1e-18 && disc >= 0.0 {
                let root = disc.sqrt();
                let t1 = (-b - root) / (2.0 * a);
                let t2 = (-b + root) / (2.0 * a);
                target = Some(unit[i] + n * if t1.abs() < t2.abs() { t1 } else { t2 });
            }
        }
        if target.is_none() && d.length() > 1e-12 {
            target = Some(c + d.normalize() * r);
        }
        if let Some(t) = target {
            moves.push(PointMove { point: points[i], position: mix3(pts[i], from_unit * t, amount) });
        }
    }
    moves
}

/// Points pushed radially onto a cylinder. axis -1 fits the axis; 0..2 runs it along object X, Y or Z.
pub fn cylinderize(mesh: &Mesh, points: &[Index], axis: i32, radius: f64) -> PointMoves {
    let pts = positions_of(mesh, points);
    if pts.len() < 3 {
        return PointMoves::new();
    }
    let candidates: Vec<DVec3> = if (0..=2).contains(&axis) {
        let mut a = DVec3::ZERO;
        a[axis as usize] = 1.0;
        vec![a]
    } else {
        let pa = axes(&pts);
        vec![pa.col(0), pa.col(1), pa.col(2)]
    };
    let o = centroid_of(&pts);
    let mut best_residual = f64::MAX;
    let mut best = PlaneFrame::default();
    let mut best_center = DVec2::ZERO;
    let mut best_radius = 0.0;
    for &a in &candidates {
        let frame = PlaneFrame::new(o, a);
        let q: Vec<DVec2> = pts.iter().map(|&p| frame.flat(p)).collect();
        let Some((c, r)) = fit_circle_2d(&q) else { continue };
        let mut residual = 0.0;
        let mut mean = 0.0;
        for &p in &q {
            let d = (p - c).length();
            residual += (d - r) * (d - r);
            mean += d;
        }
        residual /= r * r;
        if residual < best_residual {
            best_residual = residual;
            best = frame;
            best_center = c;
            best_radius = mean / q.len() as f64;
        }
    }
    if best_residual == f64::MAX {
        return PointMoves::new();
    }
    let r = if radius > 0.0 { radius } else { best_radius };
    let mut moves = PointMoves::new();
    for i in 0..pts.len() {
        let d = best.flat(pts[i]) - best_center;
        if d.length() > 1e-12 {
            moves.push(PointMove { point: points[i], position: best.lift(best_center + d.normalize() * r, best.height(pts[i])) });
        }
    }
    moves
}

/// Points onto their best-fit plane.
pub fn flatten_to_plane(mesh: &Mesh, points: &[Index]) -> PointMoves {
    let pts = positions_of(mesh, points);
    if pts.len() < 3 {
        return PointMoves::new();
    }
    let o = centroid_of(&pts);
    let n = axes(&pts).col(2);
    (0..pts.len()).map(|i| PointMove { point: points[i], position: pts[i] - n * (pts[i] - o).dot(n) }).collect()
}

/// Points onto their best-fit line.
pub fn align_to_line(mesh: &Mesh, points: &[Index]) -> PointMoves {
    let pts = positions_of(mesh, points);
    if pts.len() < 2 {
        return PointMoves::new();
    }
    let o = centroid_of(&pts);
    let d = axes(&pts).col(0);
    (0..pts.len()).map(|i| PointMove { point: points[i], position: o + d * (pts[i] - o).dot(d) }).collect()
}

/// Closed loops a ring could land on: borders, closed edge loops, outlines of faces with five or more sides, and
/// closed chains of loose edges.
pub fn candidate_loops(target: &Mesh) -> Vec<EdgePath> {
    let mut out = boundary_loops(target);
    let mut seen = vec![0u8; target.edge_capacity() as usize];
    for b in &out {
        for &e in &b.edges {
            seen[e as usize] = 1;
        }
    }
    let mut loose = Vec::new();
    for e in 0..target.edge_capacity() {
        if !target.edge_valid(e) || seen[e as usize] != 0 {
            continue;
        }
        if target.edge_corner(e) == INVALID {
            loose.push(e);
            continue;
        }
        let lp = edge_loop(target, e);
        for &x in &lp {
            seen[x as usize] = 1;
        }
        if let Some(path) = path_of_edges(target, &lp) {
            if path.closed && path.points.len() >= 3 {
                out.push(path);
            }
        }
    }
    for f in 0..target.face_capacity() {
        if target.face_valid(f) && target.face_size(f) >= 5 {
            out.push(EdgePath { points: target.face_points(f), closed: true, ..Default::default() });
        }
    }
    for c in chain_edges(target, &loose) {
        if c.closed && c.points.len() >= 3 {
            out.push(c);
        }
    }
    out
}

/// Selection onto `target`, given in the selection's object space. A target without faces is used through its
/// edges. In loop mode equal counts land one to one on the loop's vertices, in order, at the rotation that moves them
/// least.
pub fn conform_to_mesh(mesh: &Mesh, sel: &ShapeSelection, target: &Mesh, opts: &ConformOptions) -> PointMoves {
    let has_faces = target.num_faces() > 0;
    let bvh = if has_faces { Some(MeshBvh::new(target)) } else { None };
    let nearest_surface = |x: DVec3| match &bvh {
        None => nearest_on_edges(target, x),
        Some(b) => b.closest_point(x).map_or(x, |c| c.point),
    };

    let all = shape_points(sel);
    if opts.mode == ConformMode::Vertex {
        let target_points: Vec<Index> = (0..target.point_capacity()).filter(|&p| target.point_valid(p)).collect();
        if target_points.is_empty() {
            return PointMoves::new();
        }
        return all
            .par_iter()
            .map(|&id| {
                let x = mesh.position(id);
                let mut best = x;
                let mut best_d = f64::MAX;
                for &t in &target_points {
                    let d = (target.position(t) - x).dot(target.position(t) - x);
                    if d < best_d {
                        best_d = d;
                        best = target.position(t);
                    }
                }
                PointMove { point: id, position: best }
            })
            .collect();
    }

    let mut loops: Vec<Polyline> = Vec::new();
    if opts.mode == ConformMode::Loop {
        for c in candidate_loops(target) {
            loops.push(Polyline::new(positions_of(target, &c.points), true));
        }
    }
    if opts.mode == ConformMode::Surface || loops.is_empty() {
        return all.par_iter().map(|&id| PointMove { point: id, position: nearest_surface(mesh.position(id)) }).collect();
    }

    let mut moves = PointMoves::new();
    let mut done = vec![0u8; mesh.point_capacity() as usize];
    for c in &sel.chains {
        let pts = positions_of(mesh, &c.points);
        let mut best: Option<&Polyline> = None;
        let mut best_d = f64::MAX;
        for l in &loops {
            // Equal counts break ties, so a ring picks the target ring it can land on one to one.
            let d = mean_distance(&pts, l) * if l.p.len() == pts.len() { 1.0 } else { 1.0 + 1e-6 };
            if d < best_d {
                best_d = d;
                best = Some(l);
            }
        }
        let Some(best) = best else { continue };
        let to: PointList = if c.closed {
            onto_loop(&pts, best, opts.spacing)
        } else {
            pts.iter().map(|&p| best.nearest(p).0).collect()
        };
        for (i, &p) in c.points.iter().enumerate() {
            if done[p as usize] == 0 {
                done[p as usize] = 1;
                moves.push(PointMove { point: p, position: to[i] });
            }
        }
    }
    for &p in &sel.loose {
        if done[p as usize] == 0 {
            moves.push(PointMove { point: p, position: nearest_surface(mesh.position(p)) });
        }
    }
    moves
}

/// Adds moves for the interior points: the moved points' displacement spread smoothly (harmonic) through them.
pub fn follow_interior(mesh: &Mesh, interior: &[Index], moves: &mut PointMoves) {
    if interior.is_empty() || moves.is_empty() {
        return;
    }
    const OUT: u8 = 0;
    const FIXED: u8 = 1;
    const FREE: u8 = 2;
    let pc = mesh.point_capacity() as usize;
    let mut state = vec![OUT; pc];
    let mut d = vec![DVec3::ZERO; pc];
    for m in moves.iter() {
        state[m.point as usize] = FIXED;
        d[m.point as usize] = m.position - mesh.position(m.point);
    }
    let mut free = Vec::new();
    for &p in interior {
        if mesh.point_valid(p) && state[p as usize] == OUT {
            state[p as usize] = FREE;
            free.push(p);
        }
    }
    if free.is_empty() {
        return;
    }

    // Start from an inverse-distance blend so the iterations only have to smooth, not travel.
    let stride = (moves.len() / 256).max(1);
    let start: Vec<DVec3> = free
        .par_iter()
        .map(|&fp| {
            let x = mesh.position(fp);
            let mut sum = DVec3::ZERO;
            let mut wsum = 0.0;
            for k in (0..moves.len()).step_by(stride) {
                let diff = mesh.position(moves[k].point) - x;
                let w = 1.0 / diff.dot(diff).max(1e-12);
                sum += d[moves[k].point as usize] * w;
                wsum += w;
            }
            sum / wsum
        })
        .collect();
    for (i, &p) in free.iter().enumerate() {
        d[p as usize] = start[i];
    }

    let tol = (extent_of(&positions_of(mesh, &free)) * 1e-9).max(1e-12);
    let iterations = (20_000_000 / free.len()).clamp(50, 5000);
    for _ in 0..iterations {
        let mut change = 0.0f64;
        for &p in &free {
            let mut sum = DVec3::ZERO;
            let mut count = 0;
            mesh.for_each_point_edge(p, |e| {
                let q = mesh.edge_other_point(e, p);
                if state[q as usize] != OUT {
                    sum += d[q as usize];
                    count += 1;
                }
            });
            if count == 0 {
                continue;
            }
            let next = sum / count as f64;
            change = change.max((next - d[p as usize]).length());
            d[p as usize] = next;
        }
        if change < tol {
            break;
        }
    }
    for &p in &free {
        moves.push(PointMove { point: p, position: mesh.position(p) + d[p as usize] });
    }
}
