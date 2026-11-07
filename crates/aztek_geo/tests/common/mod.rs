//! tests/geo/TestUtil.h.
#![allow(dead_code)]

use aztek_geo::{names, validate, Index, Mesh};
use glam::{DVec2, DVec3};

pub fn is_valid(m: &Mesh, flags: u32) -> bool {
    let issues = m.validate(flags);
    for i in issues.iter().take(10) {
        eprintln!("{i}");
    }
    issues.is_empty()
}

pub fn valid(m: &Mesh) -> bool {
    is_valid(m, validate::DEFAULT)
}

pub fn boundary_edge_count(m: &Mesh) -> Index {
    (0..m.edge_capacity())
        .filter(|&e| m.edge_valid(e) && m.is_boundary_edge(e))
        .count() as Index
}

/// Valid edges marked hard.
pub fn hard_count(m: &Mesh) -> Index {
    let hard = m.edge_attrs().get::<bool>(names::HARD);
    (0..m.edge_capacity())
        .filter(|&e| m.edge_valid(e) && !hard.is_empty() && hard[e as usize])
        .count() as Index
}

pub fn euler(m: &Mesh) -> i64 {
    m.num_points() as i64 - m.num_edges() as i64 + m.num_faces() as i64
}

/// Signed volume of a closed mesh; positive when faces point outward.
pub fn signed_volume(m: &Mesh) -> f64 {
    let mut v = 0.0;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let c0 = m.face_corner(f);
        let a = m.position(m.corner_point(c0));
        let mut c = m.corner_next(c0);
        while m.corner_next(c) != c0 {
            v += a.dot(
                m.position(m.corner_point(c))
                    .cross(m.position(m.corner_next_point(c))),
            ) / 6.0;
            c = m.corner_next(c);
        }
    }
    v
}

/// True when no two faces cover the same uv area and no face is mirrored (fan triangulation).
pub fn uvs_dont_overlap(m: &Mesh) -> bool {
    uvs_dont_overlap_set(m, "uv0", 256)
}

pub fn uvs_dont_overlap_set(m: &Mesh, set: &str, res: i32) -> bool {
    let uv = m.corner_attrs().get::<[f32; 2]>(set);
    if uv.is_empty() {
        return false;
    }
    let at = |c: Index| DVec2::new(uv[c as usize][0] as f64, uv[c as usize][1] as f64);
    let n = (res * res) as usize;
    let mut cover = vec![0u8; n];
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let c0 = m.face_corner(f);
        let mut mine = vec![0u8; n];
        let mut area = 0.0;
        let mut c = m.corner_next(c0);
        while m.corner_next(c) != c0 {
            let (a, b, d) = (at(c0), at(c), at(m.corner_next(c)));
            let cr = (b.x - a.x) * (d.y - a.y) - (b.y - a.y) * (d.x - a.x);
            area += cr;
            c = m.corner_next(c);
            if cr.abs() < 1e-14 {
                continue;
            }
            let lo = a.min(b.min(d)) * res as f64;
            let hi = a.max(b.max(d)) * res as f64;
            for y in 0.max(lo.y as i32 - 1)..res.min(hi.y as i32 + 2) {
                for x in 0.max(lo.x as i32 - 1)..res.min(hi.x as i32 + 2) {
                    let p =
                        DVec2::new((x as f64 + 0.5) / res as f64, (y as f64 + 0.5) / res as f64);
                    let w0 = ((b.x - p.x) * (d.y - p.y) - (b.y - p.y) * (d.x - p.x)) / cr;
                    let w1 = ((d.x - p.x) * (a.y - p.y) - (d.y - p.y) * (a.x - p.x)) / cr;
                    let w2 = 1.0 - w0 - w1;
                    if w0 > 1e-9 && w1 > 1e-9 && w2 > 1e-9 {
                        mine[(y * res + x) as usize] = 1;
                    }
                }
            }
        }
        if area < 0.0 {
            eprintln!("face {f} has mirrored uvs");
            return false;
        }
        for i in 0..n {
            if mine[i] != 0 {
                if cover[i] != 0 {
                    eprintln!("face {f} overlaps in uv");
                    return false;
                }
                cover[i] = 1;
            }
        }
    }
    true
}

/// doctest's Approx: epsilon (float precision * 100) times (scale + max(|a|, |b|)); scale defaults to 1.
pub fn approx(a: f64, b: f64) -> bool {
    approx_scale(a, b, 1.0)
}

pub fn approx_scale(a: f64, b: f64, scale: f64) -> bool {
    let eps = f32::EPSILON as f64 * 100.0;
    (a - b).abs() < eps * (scale + a.abs().max(b.abs()))
}

pub fn quad_grid(nx: u32, ny: u32) -> Mesh {
    let mut m = Mesh::new();
    for j in 0..=ny {
        for i in 0..=nx {
            m.add_point(DVec3::new(i as f64, 0.0, -(j as f64)));
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let a = j * (nx + 1) + i;
            m.add_face(&[a, a + 1, a + 1 + nx + 1, a + nx + 1]);
        }
    }
    m
}

/// doctest's Approx(b).epsilon(eps).
#[allow(dead_code)]
pub fn approx_eps(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps * (1.0 + a.abs().max(b.abs()))
}
