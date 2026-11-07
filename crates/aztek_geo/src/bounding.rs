//! Shapes fitted around a point cloud, for collision and proxies. Orientation comes from the points alone, never from
//! a transform. Every fit is measured against every point at the end, so containment is exact.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use glam::{DMat3, DVec2, DVec3};

use crate::attribute::Index;
use crate::mesh::Mesh;

/// A frame (orthonormal, right-handed columns), its centre, and the full size along each column.
#[derive(Clone, Copy, Debug)]
pub struct OrientedBox {
    pub axes: DMat3,
    pub center: DVec3,
    pub size: DVec3,
}

impl Default for OrientedBox {
    fn default() -> Self {
        Self { axes: DMat3::IDENTITY, center: DVec3::ZERO, size: DVec3::ZERO }
    }
}

/// Round shapes run along axes column 1, so they line up with the Y-up primitives.
#[derive(Clone, Copy, Debug)]
pub struct RoundShape {
    pub axes: DMat3,
    pub center: DVec3,
    pub radius: f64,
    /// Full length along the axis; for a capsule it includes both caps.
    pub height: f64,
}

impl Default for RoundShape {
    fn default() -> Self {
        Self { axes: DMat3::IDENTITY, center: DVec3::ZERO, radius: 0.0, height: 0.0 }
    }
}

type Points = Vec<DVec3>;
type Tri = [usize; 3];

fn outer3(c: DVec3, r: DVec3) -> DMat3 {
    DMat3::from_cols(c * r.x, c * r.y, c * r.z)
}

fn frame_from_normal(normal: DVec3, hint: DVec3) -> Option<DMat3> {
    let len = normal.length();
    if len < 1e-30 {
        return None;
    }
    let w = normal / len;
    let mut u = hint - hint.dot(w) * w;
    if u.length() < 1e-30 {
        u = w.cross(if w.x.abs() < 0.9 { DVec3::X } else { DVec3::Y });
    }
    let u = u.normalize();
    Some(DMat3::from_cols(u, w.cross(u), w))
}

fn span(points: &[DVec3]) -> f64 {
    let mut lo = points[0];
    let mut hi = points[0];
    for &p in points {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let d = hi - lo;
    d.x.max(d.y).max(d.z)
}

/// Indices of the points extreme along evenly spread (Fibonacci) directions.
fn extreme_indices(points: &[DVec3], directions: i32) -> BTreeSet<usize> {
    let mut keep = BTreeSet::new();
    let golden = PI * (3.0 - 5f64.sqrt());
    for k in 0..directions {
        let y = 1.0 - (2.0 * k as f64 + 1.0) / directions as f64;
        let r = (1.0 - y * y).max(0.0).sqrt();
        let d = DVec3::new((golden * k as f64).cos() * r, y, (golden * k as f64).sin() * r);
        let (mut lo, mut hi) = (0, 0);
        let mut vlo = points[0].dot(d);
        let mut vhi = vlo;
        for (i, p) in points.iter().enumerate().skip(1) {
            let v = p.dot(d);
            if v < vlo {
                vlo = v;
                lo = i;
            } else if v > vhi {
                vhi = v;
                hi = i;
            }
        }
        keep.insert(lo);
        keep.insert(hi);
    }
    keep
}

fn extreme_subset(points: &[DVec3], directions: i32) -> Points {
    extreme_indices(points, directions).into_iter().map(|i| points[i]).collect()
}

/// Points the orientation search runs on; large clouds are cut down to their extremes.
fn search_set(points: &[DVec3]) -> Points {
    if points.len() <= 6000 {
        points.to_vec()
    } else {
        extreme_subset(points, 160)
    }
}

/// Incremental 3D hull: outward triangles indexing `points`, or empty when the cloud is flat or a line.
fn convex_hull(points: &[DVec3]) -> Vec<Tri> {
    let n = points.len();
    if n < 4 {
        return Vec::new();
    }
    let eps = span(points).max(1.0) * 1e-10;

    let (mut i0, mut i1) = (0, 0);
    for i in 0..n {
        if points[i].x < points[i0].x {
            i0 = i;
        }
        if points[i].x > points[i1].x {
            i1 = i;
        }
    }
    let base = points[i1] - points[i0];
    if base.length() < eps {
        return Vec::new();
    }
    let mut i2 = usize::MAX;
    let mut best = eps;
    for i in 0..n {
        let d = base.cross(points[i] - points[i0]).length();
        if d > best {
            best = d;
            i2 = i;
        }
    }
    if i2 == usize::MAX {
        return Vec::new();
    }
    let normal = base.cross(points[i2] - points[i0]);
    let mut i3 = usize::MAX;
    best = eps;
    for i in 0..n {
        let d = normal.dot(points[i] - points[i0]).abs();
        if d > best {
            best = d;
            i3 = i;
        }
    }
    if i3 == usize::MAX {
        return Vec::new();
    }

    let seed = [i0, i1, i2, i3];
    let centroid = (points[i0] + points[i1] + points[i2] + points[i3]) * 0.25;
    let mut faces: Vec<Tri> = Vec::new();
    for mut f in [[i0, i1, i2], [i0, i2, i3], [i0, i3, i1], [i1, i3, i2]] {
        let (a, b, c) = (points[f[0]], points[f[1]], points[f[2]]);
        if (b - a).cross(c - a).dot(centroid - a) > 0.0 {
            f.swap(1, 2);
        }
        faces.push(f);
    }

    let mut hidden: Vec<Tri> = Vec::new();
    let mut edges: BTreeSet<(usize, usize)> = BTreeSet::new();
    for idx in 0..n {
        if seed.contains(&idx) {
            continue;
        }
        let p = points[idx];
        hidden.clear();
        edges.clear();
        for f in &faces {
            let a = points[f[0]];
            let nrm = (points[f[1]] - a).cross(points[f[2]] - a);
            let mag = nrm.length();
            if mag >= 1e-30 && nrm.dot(p - a) / mag > eps {
                for e in 0..3 {
                    edges.insert((f[e], f[(e + 1) % 3]));
                }
            } else {
                hidden.push(*f);
            }
        }
        if edges.is_empty() {
            continue;
        }
        for &(a, b) in &edges {
            if !edges.contains(&(b, a)) {
                hidden.push([a, b, idx]);
            }
        }
        std::mem::swap(&mut faces, &mut hidden);
    }
    if faces.len() >= 4 {
        faces
    } else {
        Vec::new()
    }
}

/// Andrew's monotone chain, counter-clockwise.
fn hull_2d(mut pts: Vec<DVec2>) -> Vec<DVec2> {
    pts.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    pts.dedup();
    if pts.len() <= 2 {
        return pts;
    }
    let cross = |o: DVec2, a: DVec2, b: DVec2| (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x);
    let mut out = vec![DVec2::ZERO; pts.len() * 2];
    let mut k = 0usize;
    for &p in &pts {
        while k >= 2 && cross(out[k - 2], out[k - 1], p) <= 0.0 {
            k -= 1;
        }
        out[k] = p;
        k += 1;
    }
    let t = k + 1;
    for i in (1..pts.len()).rev() {
        while k >= t && cross(out[k - 2], out[k - 1], pts[i - 1]) <= 0.0 {
            k -= 1;
        }
        out[k] = pts[i - 1];
        k += 1;
    }
    out.truncate(k - 1);
    out
}

/// Rotating calipers: the unit edge direction whose bounding rectangle has the least area.
fn min_area_rect_direction(pts: &[DVec2]) -> DVec2 {
    let hull = hull_2d(pts.to_vec());
    if hull.len() < 3 {
        if hull.len() == 2 && (hull[1] - hull[0]).length() > 0.0 {
            return (hull[1] - hull[0]).normalize();
        }
        return DVec2::X;
    }
    let mut best_area = f64::MAX;
    let mut best_dir = DVec2::X;
    for i in 0..hull.len() {
        let d = hull[(i + 1) % hull.len()] - hull[i];
        let len = d.length();
        if len < 1e-30 {
            continue;
        }
        let u = d / len;
        let mut lo = DVec2::splat(f64::MAX);
        let mut hi = DVec2::splat(-f64::MAX);
        for &p in &hull {
            let q = DVec2::new(p.dot(u), -p.x * u.y + p.y * u.x);
            lo = lo.min(q);
            hi = hi.max(q);
        }
        let area = (hi.x - lo.x) * (hi.y - lo.y);
        if area < best_area {
            best_area = area;
            best_dir = u;
        }
    }
    best_dir
}

/// Jacobi eigen-decomposition of a symmetric 3x3; eigenvectors are the columns, in no particular order.
pub fn jacobi(a: DMat3) -> (DVec3, DMat3) {
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
    (DVec3::new(a[0][0], a[1][1], a[2][2]), v)
}

fn principal_axes(points: &[DVec3]) -> Option<DMat3> {
    let mean = points.iter().fold(DVec3::ZERO, |a, &p| a + p) / points.len() as f64;
    let mut cov = DMat3::ZERO;
    for &p in points {
        let d = p - mean;
        cov += outer3(d, d);
    }
    let (values, vectors) = jacobi(cov);
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| values[b].partial_cmp(&values[a]).unwrap_or(std::cmp::Ordering::Equal));
    frame_from_normal(vectors.col(order[2]), vectors.col(order[0]))
}

struct Extents {
    lo: DVec3,
    hi: DVec3,
}

impl Extents {
    fn size(&self) -> DVec3 {
        self.hi - self.lo
    }
}

fn extents(points: &[DVec3], axes: &DMat3) -> Extents {
    let t = axes.transpose();
    let mut e = Extents { lo: t * points[0], hi: t * points[0] };
    for &p in points {
        let q = t * p;
        e.lo = e.lo.min(q);
        e.hi = e.hi.max(q);
    }
    e
}

fn volume(size: DVec3, floor: f64) -> f64 {
    size.x.max(floor) * size.y.max(floor) * size.z.max(floor)
}

fn rotate_about(axes: &DMat3, index: usize, angle: f64) -> DMat3 {
    let i = (index + 1) % 3;
    let j = (index + 2) % 3;
    let (s, c) = angle.sin_cos();
    let mut cols = [axes.x_axis, axes.y_axis, axes.z_axis];
    let (ai, aj) = (cols[i], cols[j]);
    cols[i] = c * ai + s * aj;
    cols[j] = -s * ai + c * aj;
    DMat3::from_cols(cols[0], cols[1], cols[2])
}

fn polish(points: &[DVec3], mut best: DMat3, floor: f64) -> DMat3 {
    let mut best_volume = volume(extents(points, &best).size(), floor);
    let mut sweep = 4f64.to_radians();
    const STEPS: i32 = 10;
    for _ in 0..6 {
        let mut moved = false;
        for index in 0..3 {
            for k in -STEPS..=STEPS {
                if k == 0 {
                    continue;
                }
                let trial = rotate_about(&best, index, sweep * k as f64 / STEPS as f64);
                let v = volume(extents(points, &trial).size(), floor);
                if v < best_volume * (1.0 - 1e-12) {
                    best_volume = v;
                    best = trial;
                    moved = true;
                }
            }
        }
        sweep *= 0.35;
        if !moved && sweep < 0.002f64.to_radians() {
            break;
        }
    }
    frame_from_normal(best.z_axis, best.x_axis).unwrap_or(best)
}

/// Permutes and flips the columns to sit closest to X, Y and Z, keeping the frame right-handed.
fn canonical(axes: &DMat3) -> DMat3 {
    const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let col = |i: usize| axes.col(i);
    let mut best_perm = PERMS[0];
    let mut best_score = -1.0;
    for perm in PERMS {
        let score: f64 = (0..3).map(|i| col(perm[i])[i].abs()).sum();
        if score > best_score + 1e-12 {
            best_score = score;
            best_perm = perm;
        }
    }
    let mut out = [DVec3::ZERO; 3];
    for i in 0..3 {
        let c = col(best_perm[i]);
        out[i] = c * if c[i] < 0.0 { -1.0 } else { 1.0 };
    }
    if DMat3::from_cols(out[0], out[1], out[2]).determinant() < 0.0 {
        let mut weakest = 0;
        for i in 1..3 {
            if out[i][i].abs() < out[weakest][weakest].abs() {
                weakest = i;
            }
        }
        out[weakest] = -out[weakest];
    }
    DMat3::from_cols(out[0], out[1], out[2])
}

#[derive(Clone, Copy)]
struct Ball {
    center: DVec3,
    radius2: f64,
}

impl Default for Ball {
    fn default() -> Self {
        Self { center: DVec3::ZERO, radius2: -1.0 }
    }
}

impl Ball {
    fn contains(&self, p: DVec3, eps: f64) -> bool {
        (p - self.center).dot(p - self.center) <= self.radius2 + eps
    }
}

/// The smallest ball with every support point on its surface (circumsphere of up to four points).
fn ball_through(s: &[DVec3]) -> Option<Ball> {
    if s.is_empty() {
        return Some(Ball::default());
    }
    let p0 = s[0];
    let k = s.len() - 1;
    if k == 0 {
        return Some(Ball { center: p0, radius2: 0.0 });
    }
    // Solve for c - p0 = sum(l_j v_j) with 2 v_i.(c - p0) = v_i.v_i.
    let mut v = [DVec3::ZERO; 3];
    for i in 0..k {
        v[i] = s[i + 1] - p0;
    }
    let mut m = [[0.0f64; 4]; 3];
    for i in 0..k {
        for j in 0..k {
            m[i][j] = 2.0 * v[i].dot(v[j]);
        }
        m[i][k] = v[i].dot(v[i]);
    }
    let mut scale = 0.0f64;
    for i in 0..k {
        scale = scale.max(m[i][i]);
    }
    for c in 0..k {
        let mut pivot = c;
        for r in c + 1..k {
            if m[r][c].abs() > m[pivot][c].abs() {
                pivot = r;
            }
        }
        if m[pivot][c].abs() <= scale * 1e-14 {
            return None;
        }
        m.swap(c, pivot);
        for r in 0..k {
            if r != c {
                let f = m[r][c] / m[c][c];
                for j in c..=k {
                    m[r][j] -= f * m[c][j];
                }
            }
        }
    }
    let mut offset = DVec3::ZERO;
    for i in 0..k {
        offset += v[i] * (m[i][k] / m[i][i]);
    }
    Some(Ball { center: p0 + offset, radius2: offset.dot(offset) })
}

/// Move-to-front Welzl (Gaertner's miniball): recursion depth is bounded by the four support points.
fn welzl(pts: &mut [DVec3], end: usize, support: &mut Vec<DVec3>, eps: f64) -> Ball {
    let mut ball = ball_through(support).unwrap_or_default();
    if support.len() == 4 {
        return ball;
    }
    for i in 0..end {
        if ball.radius2 >= 0.0 && ball.contains(pts[i], eps) {
            continue;
        }
        let p = pts[i];
        support.push(p);
        let grown = welzl(pts, i, support, eps);
        support.pop();
        if grown.radius2 >= 0.0 && grown.contains(p, eps) {
            ball = grown;
        } else {
            // Degenerate support: grow the current ball just enough to take p in.
            let d = p - ball.center;
            let r = ball.radius2.max(0.0).sqrt();
            let len = d.length();
            let nr = (r + len) * 0.5;
            ball.center += d / len.max(1e-300) * (nr - r);
            ball.radius2 = nr * nr;
        }
        pts[..=i].rotate_right(1);
    }
    ball
}

fn minimum_ball(mut pts: Vec<DVec3>) -> Ball {
    // A fixed shuffle keeps the expected linear time and makes the result repeatable.
    let mut state: u64 = 0x9E3779B97F4A7C15;
    for i in (2..=pts.len()).rev() {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        pts.swap(i - 1, (state % i as u64) as usize);
    }
    let mut support = Vec::new();
    let eps = span(&pts).max(1.0) * 1e-12;
    let end = pts.len();
    welzl(&mut pts, end, &mut support, eps)
}

/// The hull's vertices, which is all any of the fits need to look at.
fn hull_points(points: &[DVec3]) -> Points {
    let sample = search_set(points);
    let faces = convex_hull(&sample);
    if faces.is_empty() {
        return sample;
    }
    let used: BTreeSet<usize> = faces.iter().flat_map(|f| f.iter().copied()).collect();
    used.into_iter().map(|i| sample[i]).collect()
}

/// Frame whose column 1 is `frame[axis]`, still right-handed.
fn along_axis(frame: &DMat3, axis: usize) -> DMat3 {
    DMat3::from_cols(frame.col((axis + 2) % 3), frame.col(axis), frame.col((axis + 1) % 3))
}

/// Smallest circle around the points projected onto the plane across `axes[1]`, as a 2D centre and radius.
fn cross_section_circle(pts: &[DVec3], axes: &DMat3) -> (DVec2, f64) {
    let flat: Vec<DVec3> = pts.iter().map(|&p| DVec3::new(p.dot(axes.x_axis), p.dot(axes.z_axis), 0.0)).collect();
    let b = minimum_ball(flat.clone());
    let c = DVec2::new(b.center.x, b.center.y);
    let mut r2 = 0.0f64;
    for q in &flat {
        let d = DVec2::new(q.x, q.y) - c;
        r2 = r2.max(d.dot(d));
    }
    (c, r2.sqrt())
}

fn cylinder_along(hull: &[DVec3], all: &[DVec3], axes: &DMat3) -> RoundShape {
    let (c, radius) = cross_section_circle(hull, axes);
    let mut lo = f64::MAX;
    let mut hi = -lo;
    let mut r2 = radius * radius;
    for &p in all {
        let t = p.dot(axes.y_axis);
        lo = lo.min(t);
        hi = hi.max(t);
        let q = DVec2::new(p.dot(axes.x_axis) - c.x, p.dot(axes.z_axis) - c.y);
        r2 = r2.max(q.dot(q));
    }
    RoundShape {
        axes: *axes,
        center: axes.x_axis * c.x + axes.z_axis * c.y + axes.y_axis * ((lo + hi) * 0.5),
        radius: r2.sqrt(),
        height: hi - lo,
    }
}

/// For a radius, the shortest spine (t1 <= t0 means a sphere) whose capsule holds every point.
fn capsule_spine(pts: &[DVec3], axes: &DMat3, c: DVec2, r: f64) -> (f64, f64) {
    let mut top = -f64::MAX;
    let mut bottom = f64::MAX;
    for &p in pts {
        let t = p.dot(axes.y_axis);
        let q = DVec2::new(p.dot(axes.x_axis) - c.x, p.dot(axes.z_axis) - c.y);
        let reach = (r * r - q.dot(q)).max(0.0).sqrt();
        top = top.max(t - reach);
        bottom = bottom.min(t + reach);
    }
    (bottom, top)
}

fn capsule_along(hull: &[DVec3], all: &[DVec3], axes: &DMat3) -> RoundShape {
    let (c, mut r0) = cross_section_circle(hull, axes);
    for &p in all {
        let q = DVec2::new(p.dot(axes.x_axis) - c.x, p.dot(axes.z_axis) - c.y);
        r0 = r0.max(q.length());
    }
    let e = extents(hull, axes);
    let r_max = r0.max(e.size().y * 0.5);
    let mut best_volume = f64::MAX;
    let mut best_r = r0;
    const STEPS: i32 = 64;
    for s in 0..=STEPS {
        let r = r0 + (r_max - r0) * s as f64 / STEPS as f64;
        let (bottom, top) = capsule_spine(hull, axes, c, r);
        let v = PI * r * r * ((top - bottom).max(0.0) + 4.0 / 3.0 * r);
        if v < best_volume {
            best_volume = v;
            best_r = r;
        }
    }
    let (bottom, top) = capsule_spine(all, axes, c, best_r);
    let length = (top - bottom).max(0.0);
    RoundShape {
        axes: *axes,
        center: axes.x_axis * c.x + axes.z_axis * c.y + axes.y_axis * ((bottom + top) * 0.5),
        radius: best_r,
        height: length + 2.0 * best_r,
    }
}

/// The move that carries a vertex `offset` out along every one of its face planes (least squares past three).
fn offset_move(normals: &[DVec3], offset: f64) -> DVec3 {
    match normals.len() {
        0 => DVec3::ZERO,
        1 => normals[0] * offset,
        2 => (normals[0] + normals[1]) * (offset / (1.0 + normals[0].dot(normals[1]))),
        _ => {
            let mut ata = DMat3::ZERO;
            let mut atb = DVec3::ZERO;
            for &n in normals {
                ata += outer3(n, n);
                atb += n * offset;
            }
            if ata.determinant().abs() < 1e-12 {
                let sum: DVec3 = normals.iter().copied().sum();
                return sum.normalize() * offset;
            }
            ata.inverse() * atb
        }
    }
}

fn best_round(points: &[DVec3], frame: &DMat3, axis: i32, fit: fn(&[DVec3], &[DVec3], &DMat3) -> RoundShape) -> RoundShape {
    let hull = hull_points(points);
    let mut best = RoundShape::default();
    let mut best_volume = f64::MAX;
    for a in 0..3 {
        if axis >= 0 && a != axis {
            continue;
        }
        let s = fit(&hull, points, &along_axis(frame, a as usize));
        let v = s.radius * s.radius * s.height;
        if v < best_volume {
            best_volume = v;
            best = s;
        }
    }
    best
}

/// The tight box of the points in a given frame.
pub fn box_in_frame(points: &[DVec3], axes: &DMat3) -> OrientedBox {
    let e = extents(points, axes);
    OrientedBox { axes: *axes, center: *axes * ((e.lo + e.hi) * 0.5), size: e.size() }
}

/// Minimum-volume oriented box: each convex hull face plane is tried with a rotating-calipers rectangle, plus the
/// principal axes, and the best frame is polished by a narrowing rotation sweep. The frame's columns are then
/// permuted and flipped to sit as close to X, Y and Z as they can, so a near-aligned cloud gets near-zero rotation.
/// None when there are fewer than two points.
pub fn minimum_volume_box(points: &[DVec3]) -> Option<OrientedBox> {
    if points.len() < 2 {
        return None;
    }
    let sample = search_set(points);
    let floor = span(&sample).max(1.0) * 1e-9;

    let mut candidates: Vec<DMat3> = Vec::new();
    let mut hull = sample.clone();
    let faces = convex_hull(&sample);
    if !faces.is_empty() {
        let used: BTreeSet<usize> = faces.iter().flat_map(|f| f.iter().copied()).collect();
        hull = used.into_iter().map(|i| sample[i]).collect();

        let mut scored: Vec<(f64, DVec3)> = Vec::new();
        for f in &faces {
            let nrm = (sample[f[1]] - sample[f[0]]).cross(sample[f[2]] - sample[f[0]]);
            let mag = nrm.length();
            if mag >= 1e-24 {
                scored.push((mag, nrm / mag));
            }
        }
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Normals within about 3 degrees of one already tried give the same box.
        let mut normals: Vec<DVec3> = Vec::new();
        for &(_, nrm) in &scored {
            if normals.iter().any(|e| nrm.dot(*e).abs() > 0.9986) {
                continue;
            }
            normals.push(nrm);
            if normals.len() >= 400 {
                break;
            }
        }
        for nrm in normals {
            let Some(frame) = frame_from_normal(nrm, DVec3::X) else { continue };
            let flat: Vec<DVec2> = hull.iter().map(|&p| DVec2::new(p.dot(frame.x_axis), p.dot(frame.y_axis))).collect();
            let d = min_area_rect_direction(&flat);
            let u = frame.x_axis * d.x + frame.y_axis * d.y;
            candidates.push(DMat3::from_cols(u, frame.z_axis.cross(u), frame.z_axis));
        }
    }
    if let Some(seed) = principal_axes(&sample) {
        candidates.push(seed);
    }
    if candidates.is_empty() {
        return None;
    }

    let mut best = candidates[0];
    let mut best_volume = f64::MAX;
    for axes in &candidates {
        let v = volume(extents(&hull, axes).size(), floor);
        if v < best_volume {
            best_volume = v;
            best = *axes;
        }
    }
    Some(box_in_frame(points, &canonical(&polish(&hull, best, floor))))
}

/// Smallest enclosing sphere (Welzl), axes left at identity.
pub fn minimum_sphere(points: &[DVec3]) -> RoundShape {
    if points.is_empty() {
        return RoundShape::default();
    }
    let b = minimum_ball(hull_points(points));
    let r2 = points.iter().fold(0.0f64, |a, &p| a.max((p - b.center).dot(p - b.center)));
    let r = r2.sqrt();
    RoundShape { axes: DMat3::IDENTITY, center: b.center, radius: r, height: 2.0 * r }
}

/// `axis` picks the box column the shape runs along; -1 tries all three and keeps the smallest.
pub fn fit_cylinder(points: &[DVec3], frame: &DMat3, axis: i32) -> RoundShape {
    if points.is_empty() {
        return RoundShape::default();
    }
    best_round(points, frame, axis, cylinder_along)
}

pub fn fit_capsule(points: &[DVec3], frame: &DMat3, axis: i32) -> RoundShape {
    if points.is_empty() {
        return RoundShape::default();
    }
    best_round(points, frame, axis, capsule_along)
}

/// Convex hull as a mesh of outward triangles. Above `max_points` the hull is built from the points that are extreme
/// along evenly spread directions, so it can sit slightly inside the cloud. `offset` pushes every face out.
pub fn convex_hull_mesh(points: &[DVec3], max_points: i32, offset: f64) -> Mesh {
    if points.len() < 4 {
        return Mesh::new();
    }
    let mut verts = hull_points(points);
    let max_points = max_points.max(4);
    if verts.len() as i32 > max_points {
        let mut reduced = Points::new();
        for dirs in 3..=max_points * 4 {
            let trial = extreme_subset(&verts, dirs);
            if trial.len() as i32 > max_points {
                break;
            }
            reduced = trial;
        }
        if reduced.len() >= 4 {
            verts = reduced;
        }
    }
    // Corners first: a point on a face or edge then never lands outside the hull built so far, so it never becomes a
    // vertex.
    {
        let first = extreme_indices(&verts, 256);
        let mut ordered: Points = first.iter().map(|&i| verts[i]).collect();
        for (i, &v) in verts.iter().enumerate() {
            if !first.contains(&i) {
                ordered.push(v);
            }
        }
        verts = ordered;
    }
    let mut faces = convex_hull(&verts);
    if faces.is_empty() {
        return Mesh::new();
    }
    let mut remap = vec![usize::MAX; verts.len()];
    let mut used = Points::new();
    for f in &mut faces {
        for i in f.iter_mut() {
            if remap[*i] == usize::MAX {
                remap[*i] = used.len();
                used.push(verts[*i]);
            }
            *i = remap[*i];
        }
    }
    let mut verts = used;

    if offset != 0.0 {
        let mut planes: Vec<Vec<DVec3>> = vec![Vec::new(); verts.len()];
        for f in &faces {
            let n = (verts[f[1]] - verts[f[0]]).cross(verts[f[2]] - verts[f[0]]).normalize();
            for &i in f {
                if !planes[i].iter().any(|e| e.dot(n) > 1.0 - 1e-9) {
                    planes[i].push(n);
                }
            }
        }
        let moved: Points = verts.iter().enumerate().map(|(i, &v)| v + offset_move(&planes[i], offset)).collect();
        verts = moved;
    }

    let sizes = vec![3 as Index; faces.len()];
    let indices: Vec<Index> = faces.iter().flat_map(|f| f.iter().map(|&i| i as Index)).collect();
    Mesh::from_polygons(&verts, &sizes, &indices)
}
