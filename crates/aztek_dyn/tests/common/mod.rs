//! Meshes and checks the solver tests share.
#![allow(dead_code)]

use aztek_dyn::{BodySettings, Solver};
use glam::DVec3;

/// A square grid of quads in XZ at height y, centred on the origin.
pub fn grid(n: usize, size: f64, y: f64) -> (Vec<DVec3>, Vec<Vec<u32>>) {
    let mut pts = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            pts.push(DVec3::new((i as f64 / n as f64 - 0.5) * size, y, (j as f64 / n as f64 - 0.5) * size));
        }
    }
    let mut faces = Vec::new();
    let w = (n + 1) as u32;
    for j in 0..n as u32 {
        for i in 0..n as u32 {
            faces.push(vec![j * w + i, (j + 1) * w + i, (j + 1) * w + i + 1, j * w + i + 1]);
        }
    }
    (pts, faces)
}

/// A UV sphere with outward faces.
pub fn sphere(r: f64, centre: DVec3, seg: usize, rings: usize) -> (Vec<DVec3>, Vec<Vec<u32>>) {
    let mut pts = vec![centre + DVec3::new(0.0, r, 0.0)];
    for k in 1..rings {
        let th = std::f64::consts::PI * k as f64 / rings as f64;
        for s in 0..seg {
            let ph = std::f64::consts::TAU * s as f64 / seg as f64;
            pts.push(centre + DVec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin()) * r);
        }
    }
    pts.push(centre - DVec3::new(0.0, r, 0.0));
    let bottom = (pts.len() - 1) as u32;
    let ring = |k: usize, s: usize| (1 + (k - 1) * seg + s % seg) as u32;
    let mut faces = Vec::new();
    for s in 0..seg {
        faces.push(vec![0, ring(1, s + 1), ring(1, s)]);
    }
    for k in 1..rings - 1 {
        for s in 0..seg {
            faces.push(vec![ring(k, s), ring(k, s + 1), ring(k + 1, s + 1), ring(k + 1, s)]);
        }
    }
    for s in 0..seg {
        faces.push(vec![ring(rings - 1, s), ring(rings - 1, s + 1), bottom]);
    }
    (pts, faces)
}

/// An axis-aligned box with outward faces.
pub fn cube(centre: DVec3, half: DVec3) -> (Vec<DVec3>, Vec<Vec<u32>>) {
    let mut pts = Vec::new();
    for k in 0..8 {
        let s = DVec3::new(if k & 1 != 0 { 1.0 } else { -1.0 }, if k & 2 != 0 { 1.0 } else { -1.0 }, if k & 4 != 0 { 1.0 } else { -1.0 });
        pts.push(centre + s * half);
    }
    let faces = vec![vec![0, 2, 3, 1], vec![4, 5, 7, 6], vec![0, 1, 5, 4], vec![2, 6, 7, 3], vec![0, 4, 6, 2], vec![1, 3, 7, 5]];
    (pts, faces)
}

pub fn cloth_settings(thickness: f64) -> BodySettings {
    BodySettings { thickness, self_thickness: thickness, ..Default::default() }
}

pub fn all_finite(s: &Solver) -> bool {
    s.pt.x.iter().chain(&s.pt.v).all(|p| p.is_finite())
}

/// The longest edge over its rest length, for a cloth body.
pub fn worst_stretch(s: &Solver, b: usize) -> f64 {
    let body = &s.bodies[b];
    let x = s.positions(b);
    body.topo.edges.iter().map(|e| (x[e[0] as usize] - x[e[1] as usize]).length() / (body.rest[e[0] as usize] - body.rest[e[1] as usize]).length().max(1e-12)).fold(0.0, f64::max)
}
