//! Planar, box, cylindrical, spherical and camera projections with fitted frames.

use std::f64::consts::PI;

use aztek_geo::bounding::jacobi;
use aztek_geo::{names, Index, Mesh};
use glam::{DMat3, DMat4, DVec2, DVec3, DVec4};

use crate::chart::{live_faces, mark_seams_from_uvs, uv_layer};
use crate::common::TWO_PI;
use crate::layout::convex_hull;
use crate::sparse::solve3_ldlt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Projection {
    #[default]
    Planar,
    /// Each face takes the plane its normal points at most, seams between the six.
    Box,
    Cylindrical,
    Spherical,
    /// Through a view-projection matrix, screen space to 0-1.
    Camera,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProjectAxis {
    X,
    Y,
    Z,
    /// Planar: area-weighted normal; cylinder and sphere: longest principal axis.
    #[default]
    BestFit,
}

/// Where an unspecified projection frame comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ProjectFit {
    /// Fitted to the faces: principal axes, normals, minimum-area rectangle.
    #[default]
    Auto,
    /// World axes (through ProjectOptions::world_linear), sized to the faces.
    World,
    /// The mesh's own axes, sized to the faces.
    Object,
}

/// The shape a projection maps through, in object space. Z is the projection axis: the plane's normal, the cylinder's
/// axis, the sphere's pole. Planar and box use width (X), height (Y) and depth (Z); cylinder radius and height (Z);
/// sphere radius. U wraps at seam_angle about Z from X.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProjectFrame {
    pub center: DVec3,
    /// Right handed, columns X, Y, Z.
    pub axes: DMat3,
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub radius: f64,
    /// Radians.
    pub seam_angle: f64,
}

impl Default for ProjectFrame {
    fn default() -> Self {
        Self { center: DVec3::ZERO, axes: DMat3::IDENTITY, width: 1.0, height: 1.0, depth: 1.0, radius: 1.0, seam_angle: 0.0 }
    }
}

#[derive(Clone, Debug)]
pub struct ProjectOptions {
    pub ty: Projection,
    pub axis: ProjectAxis,
    pub fit: ProjectFit,
    /// Object space to clip space, for Camera.
    pub camera: DMat4,
    /// Object to world, for ProjectFit::World.
    pub world_linear: DMat3,
    /// Fit the frame's rectangle into 0-1 keeping proportions; otherwise uvs are in object units. Not for Camera.
    pub normalize: bool,
    /// Seams where the projection splits, cleared where it joins.
    pub mark_seams: bool,
    /// With mark_seams, the seams already cut inside the faces stay: the projection only adds.
    pub keep_seams: bool,
    /// Overrides of the fitted frame. rotation is XYZ euler degrees of the frame's axes; seam_angle is degrees.
    pub center: Option<DVec3>,
    pub rotation: Option<DVec3>,
    pub width: Option<f64>,
    pub height: Option<f64>,
    pub depth: Option<f64>,
    pub radius: Option<f64>,
    pub seam_angle: Option<f64>,
    /// Applied last: uv = uv * scale + offset.
    pub scale: DVec2,
    pub offset: DVec2,
    /// Object space direction the camera looks along; the seam goes on the far side.
    pub view_dir: Option<DVec3>,
}

impl Default for ProjectOptions {
    fn default() -> Self {
        Self {
            ty: Projection::Planar,
            axis: ProjectAxis::BestFit,
            fit: ProjectFit::Auto,
            camera: DMat4::IDENTITY,
            world_linear: DMat3::IDENTITY,
            normalize: true,
            mark_seams: true,
            keep_seams: true,
            center: None,
            rotation: None,
            width: None,
            height: None,
            depth: None,
            radius: None,
            seam_angle: None,
            scale: DVec2::ONE,
            offset: DVec2::ZERO,
            view_dir: None,
        }
    }
}

fn wrap_angle(mut a: f64) -> f64 {
    while a > PI {
        a -= TWO_PI;
    }
    while a < -PI {
        a += TWO_PI;
    }
    a
}

fn wrap_positive(a: f64) -> f64 {
    let a = a % TWO_PI;
    if a < 0.0 {
        a + TWO_PI
    } else {
        a
    }
}

/// Eigenpairs of a symmetric matrix, largest eigenvalue first, as columns.
fn eigen(m: DMat3) -> (DVec3, DMat3) {
    let (values, vectors) = jacobi(m);
    let mut order = [0usize, 1, 2];
    order.sort_by(|&a, &b| values[b].partial_cmp(&values[a]).unwrap_or(std::cmp::Ordering::Equal));
    let vals = DVec3::new(values[order[0]], values[order[1]], values[order[2]]);
    let vecs = DMat3::from_cols(
        vectors.col(order[0]).normalize(),
        vectors.col(order[1]).normalize(),
        vectors.col(order[2]).normalize(),
    );
    (vals, vecs)
}

fn outer(a: DVec3, b: DVec3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// What the fits read from the faces, gathered once.
struct FaceSet {
    faces: Vec<Index>,
    area: Vec<f64>,
    normal: Vec<DVec3>,
    /// Each once.
    points: Vec<Index>,
    total_area: f64,
}

fn gather(mesh: &Mesh, faces: &[Index]) -> FaceSet {
    let mut s = FaceSet { faces: live_faces(mesh, faces), area: Vec::new(), normal: Vec::new(), points: Vec::new(), total_area: 0.0 };
    let mut seen = vec![false; mesh.point_capacity() as usize];
    for &f in &s.faces {
        let a = mesh.face_area(f);
        s.area.push(a);
        s.normal.push(mesh.face_normal(f));
        s.total_area += a;
        mesh.for_each_face_corner(f, |c| {
            let p = mesh.corner_point(c);
            if !seen[p as usize] {
                seen[p as usize] = true;
                s.points.push(p);
            }
        });
    }
    s
}

/// Principal axes of the faces' points, area weighted: columns by decreasing variance.
fn principal_axes(mesh: &Mesh, s: &FaceSet) -> (DMat3, DVec3) {
    let mut w = 0.0;
    let mut c = DVec3::ZERO;
    for (i, &f) in s.faces.iter().enumerate() {
        c += s.area[i] * mesh.face_centroid(f);
        w += s.area[i];
    }
    let center = if w > 0.0 { c / w } else { DVec3::ZERO };
    let mut cov = DMat3::ZERO;
    for (i, &f) in s.faces.iter().enumerate() {
        let a = s.area[i] / (mesh.face_size(f) as f64).max(1.0);
        mesh.for_each_face_corner(f, |cr| {
            let v = mesh.position(mesh.corner_point(cr)) - center;
            cov += outer(v, v) * a;
        });
    }
    let (vals, vecs) = eigen(cov);
    (vecs, vals)
}

/// Right-handed orthonormal columns from roughly orthogonal ones, keeping the first column's direction.
fn orthonormal(m: DMat3) -> DMat3 {
    let x = m.x_axis.normalize();
    let y = (m.y_axis - m.y_axis.dot(x) * x).normalize();
    DMat3::from_cols(x, y, x.cross(y))
}

/// The axes permuted and flipped (still right handed) so each column lines up best with the reference's.
fn align_to(m: &DMat3, reference: &DMat3) -> DMat3 {
    const PERMS: [[usize; 3]; 6] = [[0, 1, 2], [0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
    let mut best = *m;
    let mut best_score = -1e30;
    for perm in PERMS {
        for signs in 0..8 {
            let col = |i: usize| m.col(perm[i]) * if (signs >> i) & 1 == 1 { -1.0 } else { 1.0 };
            let c = DMat3::from_cols(col(0), col(1), col(2));
            if c.determinant() <= 0.0 {
                continue;
            }
            let score = c.x_axis.dot(reference.x_axis) + c.y_axis.dot(reference.y_axis) + c.z_axis.dot(reference.z_axis);
            if score > best_score + 1e-12 {
                best_score = score;
                best = c;
            }
        }
    }
    best
}

/// X and Y for a Z, through plane_frame in the basis' own coordinates, so object or world axes stay put.
fn frame_from_z(z: DVec3, basis: &DMat3) -> DMat3 {
    let zl = (basis.transpose() * z).normalize();
    let (ul, vl) = plane_frame(zl);
    orthonormal(DMat3::from_cols(*basis * ul, *basis * vl, z))
}

/// Area weighted covariance of the normals of faces within 60 degrees of perpendicular to axis, with the side area
/// and the residual.
fn side_normal_covariance(s: &FaceSet, axis: DVec3) -> (DMat3, f64, f64) {
    let mut cov = DMat3::ZERO;
    let (mut side_area, mut residual) = (0.0, 0.0);
    for i in 0..s.faces.len() {
        let d = s.normal[i].dot(axis);
        if d.abs() >= 0.5 {
            continue;
        }
        cov += outer(s.normal[i], s.normal[i]) * s.area[i];
        side_area += s.area[i];
        residual += s.area[i] * d * d;
    }
    (cov, side_area, residual)
}

/// The cylinder axis: the direction most perpendicular to the side faces' normals (smallest eigenvector of their
/// covariance), refined from several seeds so caps and ends fall out of the side set. Scored by how perpendicular the
/// side normals are and how much of the surface is side.
fn cylinder_axis(s: &FaceSet, longest: DVec3) -> DVec3 {
    let mut all = DMat3::ZERO;
    for i in 0..s.faces.len() {
        all += outer(s.normal[i], s.normal[i]) * s.area[i];
    }
    let (_, vecs) = eigen(all);
    let seeds = [vecs.z_axis, vecs.y_axis, vecs.x_axis, longest];
    let mut best = longest;
    let mut best_score = f64::MAX;
    let total = s.total_area.max(1e-30);
    for mut a in seeds {
        for _ in 0..8 {
            let (cov, side_area, _) = side_normal_covariance(s, a);
            if side_area <= 0.0 {
                break;
            }
            let (_, vecs) = eigen(cov);
            let next = vecs.z_axis * if vecs.z_axis.dot(a) < 0.0 { -1.0 } else { 1.0 };
            let done = next.dot(a) > 1.0 - 1e-12;
            a = next;
            if done {
                break;
            }
        }
        let (_, side_area, residual) = side_normal_covariance(s, a);
        if side_area <= 0.0 {
            continue;
        }
        let score = residual / total + (1.0 - side_area / total);
        if score < best_score - 1e-9 {
            best_score = score;
            best = a;
        }
    }
    best
}

/// The sign that makes the vector's largest component positive, so a Y-up cylinder points up.
fn canonical_sign(v: DVec3) -> DVec3 {
    let a = v.abs();
    let k = if a.x >= a.y && a.x >= a.z {
        0
    } else if a.y >= a.z {
        1
    } else {
        2
    };
    if v[k] < 0.0 {
        -v
    } else {
        v
    }
}

/// The basis column closest to a direction, signed to match it.
fn nearest_column(basis: &DMat3, d: DVec3) -> DVec3 {
    let mut k = 0;
    for i in 1..3 {
        if basis.col(i).dot(d).abs() > basis.col(k).dot(d).abs() {
            k = i;
        }
    }
    if basis.col(k).dot(d) < 0.0 {
        -basis.col(k)
    } else {
        basis.col(k)
    }
}

/// Box axes from the principal axes, falling back to object axes where the variances tie (a cube has no principal
/// axis).
fn box_axes(pca: &DMat3, vals: DVec3) -> DMat3 {
    let tie = |a: usize, b: usize| vals[a] - vals[b] <= 0.1 * vals[0].max(1e-30);
    let id = DMat3::IDENTITY;
    if tie(0, 2) {
        return id;
    }
    let m = if tie(0, 1) || tie(1, 2) {
        let unique = if tie(0, 1) { pca.z_axis } else { pca.x_axis };
        let mut other = id.x_axis;
        for i in 1..3 {
            if id.col(i).dot(unique).abs() < other.dot(unique).abs() {
                other = id.col(i);
            }
        }
        orthonormal(DMat3::from_cols(unique, other, DVec3::ZERO))
    } else {
        orthonormal(*pca)
    };
    align_to(&m, &id)
}

/// The in-plane X of a planar frame: along a side of the minimum-area rectangle around the points, whichever of its
/// four directions is nearest the plane's default u, so upright content stays upright.
fn rectangle_axis(mesh: &Mesh, s: &FaceSet, z: DVec3) -> DVec3 {
    let (u0, v0) = plane_frame(z);
    let pts: Vec<DVec2> = s.points.iter().map(|&p| DVec2::new(mesh.position(p).dot(u0), mesh.position(p).dot(v0))).collect();
    let hull = convex_hull(pts);
    if hull.len() < 3 {
        return u0;
    }
    let mut best_dir = DVec2::new(1.0, 0.0);
    let mut best_area = f64::MAX;
    for i in 0..hull.len() {
        let mut e = hull[(i + 1) % hull.len()] - hull[i];
        let len = e.length();
        if len < 1e-12 {
            continue;
        }
        e /= len;
        let n = DVec2::new(-e.y, e.x);
        let (mut lo0, mut hi0, mut lo1, mut hi1) = (1e300f64, -1e300f64, 1e300f64, -1e300f64);
        for q in &hull {
            lo0 = lo0.min(q.dot(e));
            hi0 = hi0.max(q.dot(e));
            lo1 = lo1.min(q.dot(n));
            hi1 = hi1.max(q.dot(n));
        }
        let area = (hi0 - lo0) * (hi1 - lo1);
        if area < best_area * (1.0 - 1e-9) {
            best_area = area;
            best_dir = e;
        }
    }
    let options = [best_dir, -best_dir, DVec2::new(-best_dir.y, best_dir.x), DVec2::new(best_dir.y, -best_dir.x)];
    // std::max_element keeps the first of equal maxima.
    let mut pick = options[0];
    for o in &options[1..] {
        if pick.x < o.x {
            pick = *o;
        }
    }
    (pick.x * u0 + pick.y * v0).normalize()
}

/// Ritter's bounding sphere, then one pass pulling the centre toward the farthest points.
fn bounding_sphere(mesh: &Mesh, s: &FaceSet) -> Option<(DVec3, f64)> {
    let first = *s.points.first()?;
    let farthest = |from: DVec3| {
        let mut best = from;
        let mut d = -1.0;
        for &p in &s.points {
            let l = (mesh.position(p) - from).length();
            if l > d {
                d = l;
                best = mesh.position(p);
            }
        }
        best
    };
    let a = farthest(mesh.position(first));
    let b = farthest(a);
    let mut center = (a + b) * 0.5;
    let mut radius = (b - a).length() * 0.5;
    for &p in &s.points {
        let q = mesh.position(p);
        let d = (q - center).length();
        if d > radius {
            let grown = (radius + d) * 0.5;
            center += (q - center) * ((grown - radius) / d);
            radius = grown;
        }
    }
    Some((center, radius))
}

/// Least squares circle through 2D points (Kasa). None when they are nearly collinear.
fn fit_circle(pts: &[DVec2]) -> Option<(DVec2, f64)> {
    if pts.len() < 3 {
        return None;
    }
    let mut a = [[0.0; 3]; 3];
    let mut b = [0.0; 3];
    let (mut lo, mut hi) = (DVec2::splat(1e300), DVec2::splat(-1e300));
    for p in pts {
        let r = [p.x, p.y, 1.0];
        let z = -(p.x * p.x + p.y * p.y);
        for i in 0..3 {
            for j in 0..3 {
                a[i][j] += r[i] * r[j];
            }
            b[i] += r[i] * z;
        }
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let x = solve3_ldlt(a, b);
    if x.iter().any(|v| !v.is_finite()) {
        return None;
    }
    let center = DVec2::new(-x[0] / 2.0, -x[1] / 2.0);
    let r2 = center.dot(center) - x[2];
    let extent = (hi - lo).length();
    if !(r2 > 0.0) || r2.sqrt() > 10.0 * extent {
        return None;
    }
    Some((center, r2.sqrt()))
}

fn is_side(n: DVec3, axis: DVec3) -> bool {
    n.dot(axis).abs() < 0.7
}

fn xy(v: DVec3) -> DVec2 {
    DVec2::new(v.x, v.y)
}

/// Where U wraps for a cylinder or sphere, as an angle about Z from X. Seams cost most where they would cut through
/// faces, less on the side the camera sees, and are cheaper along existing seams and hard edges.
fn choose_seam(mesh: &Mesh, s: &FaceSet, fr: &ProjectFrame, view_local: DVec3) -> f64 {
    let to_local = fr.axes.transpose();
    let local = |p: Index| to_local * (mesh.position(p) - fr.center);
    let eps = 1e-9 * fr.radius.max(fr.height).max(1e-9);

    struct Span {
        lo: f64,
        hi: f64,
        w: f64,
    }
    let mut spans = Vec::new();
    let mut cand = Vec::new();
    let mut side_area = 0.0;
    for (i, &f) in s.faces.iter().enumerate() {
        let nl = to_local * s.normal[i];
        let cl = to_local * (mesh.face_centroid(f) - fr.center);
        if nl.z.abs() > 0.8 || xy(cl).length() <= eps {
            continue;
        }
        let base = cl.y.atan2(cl.x);
        let (mut lo, mut hi) = (0.0f64, 0.0f64);
        let mut on_axis = false;
        mesh.for_each_face_corner(f, |c| {
            if on_axis {
                return;
            }
            let q = local(mesh.corner_point(c));
            if xy(q).length() <= eps {
                on_axis = true;
                return;
            }
            let th = q.y.atan2(q.x);
            let d = wrap_angle(th - base);
            lo = lo.min(d);
            hi = hi.max(d);
            cand.push(th);
        });
        if on_axis || hi - lo > PI {
            continue;
        }
        spans.push(Span { lo: base + lo, hi: base + hi, w: s.area[i] });
        side_area += s.area[i];
    }
    for k in 0..720 {
        cand.push(-PI + PI * k as f64 / 360.0);
    }
    cand.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut c: Vec<f64> = Vec::new();
    for a in cand {
        if c.last().is_none_or(|&l| a - l > 1e-7) {
            c.push(a);
        }
    }
    let n = c.len();

    // Area of the faces each candidate cuts through, by a difference array over the sorted candidates.
    let mut diff = vec![0.0; n + 1];
    let mut add_range = |lo: f64, hi: f64, w: f64| {
        let a = c.partition_point(|&x| x <= lo);
        let b = c.partition_point(|&x| x < hi);
        if a < b {
            diff[a] += w;
            diff[b] -= w;
        }
    };
    let inset = 1e-6;
    for sp in &spans {
        let (lo, hi) = (sp.lo + inset, sp.hi - inset);
        if lo >= hi {
            continue;
        }
        add_range(lo, hi.min(PI), sp.w);
        if lo < -PI {
            add_range(lo + TWO_PI, PI, sp.w);
        }
        if hi > PI {
            add_range(-PI - 1.0, hi - TWO_PI, sp.w);
        }
    }

    // Existing seams and hard edges running along the axis at a candidate angle.
    let mut bonus = vec![0.0; n];
    let seam = mesh.edge_attrs().get::<bool>(names::SEAM);
    let hard = mesh.edge_attrs().get::<bool>(names::HARD);
    if !seam.is_empty() || !hard.is_empty() {
        let mut done = vec![false; mesh.edge_capacity() as usize];
        for &f in &s.faces {
            mesh.for_each_face_corner(f, |cr| {
                let e = mesh.corner_edge(cr) as usize;
                if done[e] || !(seam.get(e).copied().unwrap_or(false) || hard.get(e).copied().unwrap_or(false)) {
                    return;
                }
                done[e] = true;
                let (a, b) = (local(mesh.edge_point(e as Index, 0)), local(mesh.edge_point(e as Index, 1)));
                if xy(a).length() <= eps || xy(b).length() <= eps {
                    return;
                }
                let ta = a.y.atan2(a.x);
                if wrap_angle(ta - b.y.atan2(b.x)).abs() > 1e-5 {
                    return;
                }
                let k = c.partition_point(|&x| x < ta - 1e-6);
                if k < n && (c[k] - ta).abs() < 1e-5 {
                    bonus[k] += (a.z - b.z).abs() / fr.height.max(eps);
                }
            });
        }
    }

    let mut view = xy(view_local);
    let has_view = view.length() > 1e-6;
    if has_view {
        view = view.normalize();
    }
    let (mut best, mut best_cost, mut run) = (0.0, f64::MAX, 0.0);
    for k in 0..n {
        run += diff[k];
        let cut = if side_area > 0.0 { run.max(0.0) / side_area } else { 0.0 };
        let seen = if has_view { (1.0 - (c[k].cos() * view.x + c[k].sin() * view.y)) * 0.5 } else { 0.0 };
        let cost = 4.0 * cut + seen - 2.0 * bonus[k].min(1.0);
        if cost < best_cost - 1e-9 {
            best_cost = cost;
            best = c[k];
        }
    }
    best
}

/// Object-space basis the World and Object fits use.
fn fit_basis(o: &ProjectOptions) -> DMat3 {
    if o.fit != ProjectFit::World {
        return DMat3::IDENTITY;
    }
    if o.world_linear.determinant().abs() > 1e-30 {
        orthonormal(o.world_linear.inverse())
    } else {
        DMat3::IDENTITY
    }
}

fn fit_axes(mesh: &Mesh, s: &FaceSet, o: &ProjectOptions) -> DMat3 {
    let (pca, vals) = principal_axes(mesh, s);
    let basis = fit_basis(o);
    let auto_fit = o.fit == ProjectFit::Auto;
    let k: Option<usize> = match o.axis {
        ProjectAxis::X => Some(0),
        ProjectAxis::Y => Some(1),
        ProjectAxis::Z => Some(2),
        ProjectAxis::BestFit => None,
    };
    match o.ty {
        Projection::Planar => {
            let sum = s.faces.iter().fold(DVec3::ZERO, |acc, &f| acc + mesh.face_area_vector(f));
            let scale: f64 = s.area.iter().sum();
            let n = if sum.length() > 1e-6 * scale { sum.normalize() } else { pca.z_axis };
            let z = match k {
                Some(k) => basis.col(k),
                None if auto_fit => n,
                None => nearest_column(&basis, n),
            };
            if !auto_fit {
                return frame_from_z(z, &basis);
            }
            let x = rectangle_axis(mesh, s, z);
            DMat3::from_cols(x, z.cross(x), z)
        }
        Projection::Box => {
            if auto_fit {
                box_axes(&pca, vals)
            } else {
                basis
            }
        }
        Projection::Cylindrical => {
            let axis = canonical_sign(cylinder_axis(s, pca.x_axis));
            let z = match k {
                Some(k) => basis.col(k),
                None if auto_fit => axis,
                None => nearest_column(&basis, axis),
            };
            frame_from_z(z, &basis)
        }
        Projection::Spherical => {
            let distinct = vals[0] > 1.1 * vals[1];
            let z = match k {
                Some(k) => basis.col(k),
                None if auto_fit && distinct => canonical_sign(pca.x_axis),
                None => basis.y_axis,
            };
            frame_from_z(z, &basis)
        }
        Projection::Camera => DMat3::IDENTITY,
    }
}

fn fit_sizes(mesh: &Mesh, s: &FaceSet, o: &ProjectOptions, fr: &mut ProjectFrame) {
    let to_local = fr.axes.transpose();
    let (mut lo, mut hi) = (DVec3::splat(1e300), DVec3::splat(-1e300));
    for &p in &s.points {
        let q = to_local * mesh.position(p);
        lo = lo.min(q);
        hi = hi.max(q);
    }
    if s.points.is_empty() {
        lo = DVec3::ZERO;
        hi = DVec3::ZERO;
    }
    let mut mid = (lo + hi) * 0.5;
    let mut ext = hi - lo;
    if let Some(center) = o.center {
        let cl = to_local * center;
        ext = 2.0 * (hi - cl).abs().max((lo - cl).abs());
        mid = cl;
    }
    let tiny = 1e-9 * ext.x.max(ext.y).max(ext.z).max(1e-9);

    match o.ty {
        Projection::Planar | Projection::Box => {
            fr.center = fr.axes * mid;
            fr.width = ext.x;
            fr.height = ext.y;
            fr.depth = ext.z;
        }
        Projection::Cylindrical => {
            let mut side = Vec::new();
            let mut seen = vec![false; mesh.point_capacity() as usize];
            for (i, &f) in s.faces.iter().enumerate() {
                if is_side(s.normal[i], fr.axes.z_axis) {
                    mesh.for_each_face_corner(f, |c| {
                        let p = mesh.corner_point(c);
                        if !seen[p as usize] {
                            seen[p as usize] = true;
                            side.push(xy(to_local * mesh.position(p)));
                        }
                    });
                }
            }
            if side.is_empty() {
                side = s.points.iter().map(|&p| xy(to_local * mesh.position(p))).collect();
            }
            let mut c2 = xy(mid);
            let fitted = if o.center.is_none() { fit_circle(&side) } else { None };
            match fitted {
                Some((c, _)) => c2 = c,
                None if o.center.is_none() => {
                    c2 = side.iter().fold(DVec2::ZERO, |a, &q| a + q) / side.len().max(1) as f64;
                }
                None => {}
            }
            let r: f64 = side.iter().map(|q| (*q - c2).length()).sum();
            fr.radius = if side.is_empty() { 1.0 } else { r / side.len() as f64 };
            fr.center = fr.axes * DVec3::new(c2.x, c2.y, mid.z);
            fr.height = ext.z;
        }
        Projection::Spherical => {
            if let Some(center) = o.center {
                fr.center = center;
                fr.radius = s.points.iter().map(|&p| (mesh.position(p) - center).length()).fold(0.0, f64::max);
            } else if let Some((c, r)) = bounding_sphere(mesh, s) {
                fr.center = c;
                fr.radius = r;
            }
        }
        Projection::Camera => {}
    }
    fr.width = fr.width.max(tiny);
    fr.height = fr.height.max(tiny);
    fr.depth = fr.depth.max(tiny);
    fr.radius = fr.radius.max(tiny);
}

fn resolve(mesh: &Mesh, s: &FaceSet, o: &ProjectOptions) -> ProjectFrame {
    let mut fr = ProjectFrame::default();
    if o.ty == Projection::Camera || s.faces.is_empty() {
        return fr;
    }
    fr.axes = match o.rotation {
        Some(r) => euler_frame(r),
        None => fit_axes(mesh, s, o),
    };
    fit_sizes(mesh, s, o, &mut fr);
    if let Some(c) = o.center {
        fr.center = c;
    }
    let use_value = |v: Option<f64>, out: &mut f64| {
        if let Some(v) = v.filter(|&v| v > 0.0) {
            *out = v;
        }
    };
    use_value(o.width, &mut fr.width);
    use_value(o.height, &mut fr.height);
    use_value(o.depth, &mut fr.depth);
    use_value(o.radius, &mut fr.radius);
    if let Some(a) = o.seam_angle {
        fr.seam_angle = a.to_radians();
    } else if matches!(o.ty, Projection::Cylindrical | Projection::Spherical) {
        // Without a camera the seam goes at the back, as seen from the front view.
        let view = match o.view_dir {
            Some(v) if v.length() > 1e-12 => v.normalize(),
            _ => DVec3::new(0.0, 0.0, -1.0),
        };
        fr.seam_angle = choose_seam(mesh, s, &fr, fr.axes.transpose() * view);
    }
    fr
}

/// u, v with u x v = n, Y-up friendly: +Z gives X/Y, +Y gives X/-Z.
pub fn plane_frame(n: DVec3) -> (DVec3, DVec3) {
    let up = if n.y.abs() > 0.99 { DVec3::new(0.0, 0.0, if n.y > 0.0 { -1.0 } else { 1.0 }) } else { DVec3::Y };
    let u = up.cross(n).normalize();
    (u, n.cross(u))
}

/// XYZ euler degrees of a rotation frame (glm's extractEulerAngleXYZ).
pub fn frame_euler(axes: &DMat3) -> DVec3 {
    let m = |c: usize, r: usize| axes.col(c)[r];
    let t1 = m(2, 1).atan2(m(2, 2));
    let c2 = (m(0, 0) * m(0, 0) + m(1, 0) * m(1, 0)).sqrt();
    let t2 = (-m(2, 0)).atan2(c2);
    let (s1, c1) = (t1.sin(), t1.cos());
    let t3 = (s1 * m(0, 2) - c1 * m(0, 1)).atan2(c1 * m(1, 1) - s1 * m(1, 2));
    DVec3::new(-t1, -t2, -t3) * (180.0 / PI)
}

/// glm's eulerAngleXYZ: rotate X, then Y, then Z, multiplied as Rx * Ry * Rz.
pub fn euler_frame(degrees: DVec3) -> DMat3 {
    let r = degrees * (PI / 180.0);
    DMat3::from_rotation_x(r.x) * DMat3::from_rotation_y(r.y) * DMat3::from_rotation_z(r.z)
}

/// The frame project() would use for the faces (all when empty): fitted, then the options' overrides applied.
pub fn fit_projection(mesh: &Mesh, faces: &[Index], opts: &ProjectOptions) -> ProjectFrame {
    resolve(mesh, &gather(mesh, faces), opts)
}

/// Projects the faces (all when empty) into the uv set. None on unknown input, else the frame used.
pub fn project(mesh: &mut Mesh, face_ids: &[Index], uv_set: &str, opts: &ProjectOptions) -> Option<ProjectFrame> {
    let s = gather(mesh, face_ids);
    if s.faces.is_empty() {
        return None;
    }
    let fr = resolve(mesh, &s, opts);
    let to_local = fr.axes.transpose();
    // The frame's rectangle in object units; normalize fits it into 0-1.
    let mut rect = DVec2::ONE;
    let mut out = uv_layer(mesh, uv_set).to_vec();
    let m: &Mesh = mesh;
    let local = |p: Index| to_local * (m.position(p) - fr.center);

    match opts.ty {
        Projection::Planar => {
            rect = DVec2::new(fr.width, fr.height);
            for &f in &s.faces {
                m.for_each_face_corner(f, |c| {
                    let q = local(m.corner_point(c));
                    out[c as usize] = [(q.x + fr.width * 0.5) as f32, (q.y + fr.height * 0.5) as f32];
                });
            }
        }
        Projection::Box => {
            let size = DVec3::new(fr.width, fr.height, fr.depth);
            let side = size.x.max(size.y).max(size.z);
            rect = DVec2::splat(side);
            for (i, &f) in s.faces.iter().enumerate() {
                let fnl = to_local * s.normal[i];
                let a = fnl.abs();
                let k = if a.x >= a.y && a.x >= a.z {
                    0
                } else if a.y >= a.z {
                    1
                } else {
                    2
                };
                let mut n = DVec3::ZERO;
                n[k] = if fnl[k] >= 0.0 { 1.0 } else { -1.0 };
                let (u, v) = plane_frame(n);
                let (eu, ev) = (u.abs().dot(size), v.abs().dot(size));
                m.for_each_face_corner(f, |c| {
                    let q = local(m.corner_point(c));
                    out[c as usize] = [(q.dot(u) + eu * 0.5) as f32, (q.dot(v) + ev * 0.5) as f32];
                });
            }
        }
        Projection::Cylindrical | Projection::Spherical => {
            let sphere = opts.ty == Projection::Spherical;
            let r = fr.radius;
            rect = if sphere { DVec2::new(TWO_PI * r, PI * r) } else { DVec2::new(TWO_PI * r, fr.height) };
            let angle_of = |q: DVec3| q.y.atan2(q.x) - fr.seam_angle;
            for &f in &s.faces {
                let fc = to_local * (m.face_centroid(f) - fr.center);
                let base = if xy(fc).length() > 1e-9 * r { wrap_positive(angle_of(fc)) } else { 0.0 };
                m.for_each_face_corner(f, |c| {
                    let q = local(m.corner_point(c));
                    // Points on the axis have no angle of their own; they take the face's.
                    let th = if xy(q).length() > 1e-9 * r { base + wrap_angle(angle_of(q) - base) } else { base };
                    let mut second = q.z + fr.height * 0.5;
                    if sphere {
                        let len = q.length();
                        second = if len > 0.0 { (q.z / len).clamp(-1.0, 1.0).asin() } else { 0.0 } * r + PI * r * 0.5;
                    }
                    out[c as usize] = [(th * r) as f32, second as f32];
                });
            }
        }
        Projection::Camera => {
            for &f in &s.faces {
                m.for_each_face_corner(f, |c| {
                    let clip = opts.camera * DVec4::from((m.position(m.corner_point(c)), 1.0));
                    let iw = if clip.w.abs() > 1e-12 { 1.0 / clip.w } else { 1.0 };
                    out[c as usize] = [(clip.x * iw * 0.5 + 0.5) as f32, (clip.y * iw * 0.5 + 0.5) as f32];
                });
            }
        }
    }

    let camera = opts.ty == Projection::Camera;
    let ext = rect.x.max(rect.y);
    let s0 = if opts.normalize && !camera && ext > 0.0 { (1.0 / ext) as f32 } else { 1.0 };
    let scale = [opts.scale.x as f32 * s0, opts.scale.y as f32 * s0];
    let offset = [opts.offset.x as f32, opts.offset.y as f32];
    if scale != [1.0, 1.0] || offset != [0.0, 0.0] {
        for &f in &s.faces {
            m.for_each_face_corner(f, |c| {
                let p = out[c as usize];
                out[c as usize] = [p[0] * scale[0] + offset[0], p[1] * scale[1] + offset[1]];
            });
        }
    }
    uv_layer(mesh, uv_set).copy_from_slice(&out);
    if opts.mark_seams {
        let kept: Vec<bool> = if opts.keep_seams { mesh.edge_attrs().get::<bool>(names::SEAM).to_vec() } else { Vec::new() };
        mark_seams_from_uvs(mesh, &s.faces, uv_set);
        let seam = mesh.edge_attrs_mut().get_or_add::<bool>(names::SEAM);
        for (e, &k) in kept.iter().enumerate() {
            if e < seam.len() {
                seam[e] = seam[e] || k;
            }
        }
    }
    Some(fr)
}
