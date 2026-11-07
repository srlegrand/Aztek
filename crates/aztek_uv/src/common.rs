//! Small helpers the uv sources share, and the per-triangle
//! geometry of the uv solvers (local frames, Jacobians, 2x2 SVDs).

use aztek_geo::Index;
use glam::{DVec2, DVec3};

use crate::chart::Chart;
use crate::unwrap::folded_triangles;

pub const TWO_PI: f64 = std::f64::consts::TAU;

/// A triangle's three uvs.
pub type Tri2 = [DVec2; 3];

/// One key for the edge between points a and b, either way round.
pub fn edge_key(a: Index, b: Index) -> u64 {
    ((a.min(b) as u64) << 32) | a.max(b) as u64
}

/// The chart has real uv area and no folded triangles, so its layout is worth keeping.
pub fn usable(ch: &Chart) -> bool {
    let a = ch.area_uv().abs();
    a.is_finite() && a > 1e-14 * ch.area_3d().max(1.0) && folded_triangles(ch) == 0
}

/// Gradient basis of a triangle in its own plane: grad f = sum f_j * g[j].
#[derive(Clone, Copy, Debug, Default)]
pub struct TriFrame {
    pub g: [DVec2; 3],
    pub area: f64,
}

pub fn tri_frame(a: DVec3, b: DVec3, c: DVec3) -> TriFrame {
    let mut t = TriFrame::default();
    let (e1, e2) = (b - a, c - a);
    let l1 = e1.length();
    if l1 <= 0.0 {
        return t;
    }
    let x = e1 / l1;
    let px = e2.dot(x);
    let py = (e2 - px * x).length();
    let p = [DVec2::ZERO, DVec2::new(l1, 0.0), DVec2::new(px, py)];
    t.area = 0.5 * l1 * py;
    if t.area <= 0.0 {
        return t;
    }
    for j in 0..3 {
        let e = p[(j + 2) % 3] - p[(j + 1) % 3];
        t.g[j] = DVec2::new(-e.y, e.x) / (2.0 * t.area);
    }
    t
}

pub fn tri_frames(ch: &Chart) -> Vec<TriFrame> {
    ch.tris.iter().map(|t| tri_frame(ch.pos[t[0] as usize], ch.pos[t[1] as usize], ch.pos[t[2] as usize])).collect()
}

/// [a b; c d]: rows are grad u and grad v.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jac2 {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
}

impl Default for Jac2 {
    fn default() -> Self {
        Self { a: 1.0, b: 0.0, c: 0.0, d: 1.0 }
    }
}

impl Jac2 {
    pub fn det(&self) -> f64 {
        self.a * self.d - self.b * self.c
    }
}

pub fn jacobian(f: &TriFrame, u0: DVec2, u1: DVec2, u2: DVec2) -> Jac2 {
    let gu = u0.x * f.g[0] + u1.x * f.g[1] + u2.x * f.g[2];
    let gv = u0.y * f.g[0] + u1.y * f.g[1] + u2.y * f.g[2];
    Jac2 { a: gu.x, b: gu.y, c: gv.x, d: gv.y }
}

/// Signed 2x2 SVD: M = Rot(phi) * diag(s1, s2) * Rot(theta), s1 >= |s2|; s2 < 0 when M flips.
#[derive(Clone, Copy, Debug)]
pub struct Svd2 {
    pub s1: f64,
    pub s2: f64,
    pub phi: f64,
    pub theta: f64,
}

pub fn svd2(m: &Jac2) -> Svd2 {
    let (e, f, g, h) = ((m.a + m.d) * 0.5, (m.a - m.d) * 0.5, (m.c + m.b) * 0.5, (m.c - m.b) * 0.5);
    let (q, r) = ((e * e + h * h).sqrt(), (f * f + g * g).sqrt());
    let (a1, a2) = (g.atan2(f), h.atan2(e));
    Svd2 { s1: q + r, s2: q - r, phi: (a2 + a1) * 0.5, theta: (a2 - a1) * 0.5 }
}

/// Rot(phi) * diag(t1, t2) * Rot(theta).
pub fn compose(s: &Svd2, t1: f64, t2: f64) -> Jac2 {
    let (cp, sp, ct, st) = (s.phi.cos(), s.phi.sin(), s.theta.cos(), s.theta.sin());
    // Rot(phi) * diag(t1, t2)
    let (m00, m01, m10, m11) = (cp * t1, -sp * t2, sp * t1, cp * t2);
    Jac2 { a: m00 * ct + m01 * st, b: -m00 * st + m01 * ct, c: m10 * ct + m11 * st, d: -m10 * st + m11 * ct }
}

pub fn signed_area2(a: DVec2, b: DVec2, c: DVec2) -> f64 {
    let (e1, e2) = (b - a, c - a);
    0.5 * (e1.x * e2.y - e1.y * e2.x)
}

/// Smallest positive root of t where the Jacobian j0 + t * jd stops being positive (infinity when none).
pub(crate) fn flip_root(j0: &Jac2, jd: &Jac2, linear_eps: f64) -> f64 {
    let qa = jd.det();
    let qb = j0.a * jd.d + j0.d * jd.a - j0.b * jd.c - j0.c * jd.b;
    let qc = j0.det();
    let mut root = f64::INFINITY;
    if qa.abs() < linear_eps {
        if qb < 0.0 {
            root = -qc / qb;
        }
    } else {
        let disc = qb * qb - 4.0 * qa * qc;
        if disc >= 0.0 {
            let sq = disc.sqrt();
            let (r1, r2) = ((-qb - sq) / (2.0 * qa), (-qb + sq) / (2.0 * qa));
            if r1 > 0.0 {
                root = root.min(r1);
            }
            if r2 > 0.0 {
                root = root.min(r2);
            }
        }
    }
    root
}

/// Largest t in (0, 1] (times a safety factor) such that moving uv by t * dir flips no triangle that is currently
/// positive. dir and uv are per chart vertex.
pub fn max_flip_free_step(ch: &Chart, fr: &[TriFrame], uv: &[DVec2], dir: &[DVec2]) -> f64 {
    let mut t_max = 1.0f64;
    for (i, t) in ch.tris.iter().enumerate() {
        if fr[i].area <= 0.0 {
            continue;
        }
        let (a, b, c) = (t[0] as usize, t[1] as usize, t[2] as usize);
        let j0 = jacobian(&fr[i], uv[a], uv[b], uv[c]);
        if j0.det() <= 0.0 {
            continue;
        }
        let jd = jacobian(&fr[i], dir[a], dir[b], dir[c]);
        let root = flip_root(&j0, &jd, 1e-14 * 1f64.max(j0.det().abs()));
        t_max = t_max.min(0.9 * root.min(1e30));
    }
    t_max
}
