//! Rigid bodies as particles held in shape: after each substep's constraints and contacts, the best rotation and centre
//! for the particles are found and the particles are put back on the rigid shape there (shape matching).

use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::body::{Body, BodyKind, RigidShape};
use crate::geom::{self, Aabb};
use crate::solver::Solver;

pub struct Samples {
    pub points: Vec<DVec3>,
    pub radius: f64,
    /// Each point's own radius: the filling's, or smaller for the mesh's own corners on the surface.
    pub radii: Vec<f64>,
}

/// Fills the body's shape with particles, in world space.
pub fn sample(body: &Body, matrix: DMat4) -> Samples {
    let inv = matrix.inverse();
    let world = &body.rest;
    let local: Vec<DVec3> = world.iter().map(|&p| inv.transform_point3(p)).collect();
    let b = Aabb::of(&local);
    let size = b.size();
    let longest = size.max_element();
    if !(longest > 1e-9) || !inv.is_finite() {
        let r = body.settings.radius.max(1e-3);
        let points = if world.is_empty() { vec![matrix.w_axis.truncate()] } else { world.clone() };
        let radii = vec![r; points.len()];
        return Samples { points, radius: r, radii };
    }
    let res = body.settings.resolution.clamp(2, 40) as f64;
    let spacing = longest / res;
    let r = spacing * 0.5;
    let centre = (b.lo + b.hi) * 0.5;
    let half = size * 0.5;
    let inside = |q: DVec3| -> bool {
        match body.settings.shape {
            RigidShape::Box => true,
            RigidShape::Sphere => (q - centre).length() <= half.max_element() + 1e-9,
            RigidShape::Capsule => {
                let axis = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
                let rad = (0..3).filter(|&k| k != axis).map(|k| half[k]).fold(0.0f64, f64::max);
                let mut d = q - centre;
                let len = (half[axis] - rad).max(0.0);
                d[axis] = (d[axis].abs() - len).max(0.0);
                d.length() <= rad + 1e-9
            }
            RigidShape::Mesh => {
                let w: f64 = body.topo.triangles.iter().map(|t| geom::winding(q, local[t[0] as usize], local[t[1] as usize], local[t[2] as usize])).sum();
                w.abs() > 0.5
            }
        }
    };
    let counts = (size / spacing).ceil().max(DVec3::ONE);
    let mut pts = Vec::new();
    for ix in 0..counts.x as usize {
        for iy in 0..counts.y as usize {
            for iz in 0..counts.z as usize {
                let q = b.lo + (DVec3::new(ix as f64, iy as f64, iz as f64) + 0.5) * (size / counts);
                if inside(q) {
                    pts.push(matrix.transform_point3(q));
                }
            }
        }
    }
    if pts.len() < 4 {
        pts = world.clone();
    }
    if pts.is_empty() {
        pts.push(matrix.w_axis.truncate());
    }
    let mut radii = vec![r; pts.len()];
    // The shape's own corners, so edges and corners outside the filling still collide. A mesh's vertices, a few
    // thousand at most; a box's eight corners.
    let corners: Vec<DVec3> = match body.settings.shape {
        RigidShape::Mesh => {
            let step = (world.len() / 2000).max(1);
            world.iter().step_by(step).copied().collect()
        }
        RigidShape::Box => (0..8).map(|k| matrix.transform_point3(DVec3::new(if k & 1 == 0 { b.lo.x } else { b.hi.x }, if k & 2 == 0 { b.lo.y } else { b.hi.y }, if k & 4 == 0 { b.lo.z } else { b.hi.z }))).collect(),
        _ => Vec::new(),
    };
    for c in corners {
        pts.push(c);
        radii.push(r * 0.25);
    }
    Samples { points: pts, radius: r, radii }
}

/// The rotation nearest to A, starting from q: Müller et al., A Robust Method to Extract the Rotational Part of
/// Deformations, 2016.
pub fn extract_rotation(a: &DMat3, mut q: DQuat, iterations: usize) -> DQuat {
    for _ in 0..iterations {
        let r = DMat3::from_quat(q);
        let denom = (r.x_axis.dot(a.x_axis) + r.y_axis.dot(a.y_axis) + r.z_axis.dot(a.z_axis)).abs() + 1e-9;
        let omega = (r.x_axis.cross(a.x_axis) + r.y_axis.cross(a.y_axis) + r.z_axis.cross(a.z_axis)) / denom;
        let w = omega.length();
        if !(w > 1e-12) || !w.is_finite() {
            break;
        }
        q = (DQuat::from_axis_angle(omega / w, w) * q).normalize();
    }
    q
}

pub(crate) fn shape_match(s: &mut Solver) {
    for bi in 0..s.bodies.len() {
        let body = &s.bodies[bi];
        if body.kind != BodyKind::Rigid || !body.settings.enabled || body.count == 0 {
            continue;
        }
        let Some(rd) = &body.rigid else { continue };
        if rd.offsets.len() != body.count {
            continue;
        }
        let o = body.start;
        let n = body.count;
        let com = (0..n).map(|l| s.pt.p[o + l]).sum::<DVec3>() / n as f64;
        if !crate::util::finite(com) {
            continue;
        }
        let mut a = DMat3::ZERO;
        for l in 0..n {
            let d = s.pt.p[o + l] - com;
            let q = rd.offsets[l];
            a.x_axis += d * q.x;
            a.y_axis += d * q.y;
            a.z_axis += d * q.z;
        }
        let rot = extract_rotation(&a, rd.rot, 8);
        let r = DMat3::from_quat(rot);
        let goals: Vec<DVec3> = rd.offsets.iter().map(|q| com + r * *q).collect();
        if goals.iter().all(|g| crate::util::finite(*g)) {
            s.pt.p[o..o + n].copy_from_slice(&goals);
            let rd = s.bodies[bi].rigid.as_mut().unwrap();
            rd.com = com;
            rd.rot = rot;
        }
    }
}

/// Keeps the body's place in step with its particles at the end of a frame.
pub(crate) fn publish(s: &mut Solver, b: usize) {
    let body = &s.bodies[b];
    if body.kind != BodyKind::Rigid || body.count == 0 {
        return;
    }
    let com = s.pt.x[body.start..body.start + body.count].iter().sum::<DVec3>() / body.count as f64;
    if let (Some(rd), true) = (s.bodies[b].rigid.as_mut(), crate::util::finite(com)) {
        rd.com = com;
    }
}
