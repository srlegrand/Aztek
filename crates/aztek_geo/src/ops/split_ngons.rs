//! Cutting n-gons into quads along their own points.

use std::f64::consts::{FRAC_PI_2, TAU};

use glam::{DVec2, DVec3};

use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;

#[derive(Clone, Debug)]
pub struct NgonSplitOptions {
    /// Faces with more sides than this are cut.
    pub max_sides: i32,
    /// No cut leaves a corner narrower than this many degrees; a face with no such cut stays as it is.
    pub min_angle: f64,
}

impl Default for NgonSplitOptions {
    fn default() -> Self {
        Self { max_sides: 4, min_angle: 30.0 }
    }
}

fn cross2(a: DVec2, b: DVec2) -> f64 {
    a.x * b.y - a.y * b.x
}

/// Counter-clockwise angle from a to b, in [0, 2pi).
fn ccw_angle(a: DVec2, b: DVec2) -> f64 {
    let t = cross2(a, b).atan2(a.dot(b));
    if t < 0.0 {
        t + TAU
    } else {
        t
    }
}

fn segments_cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2, eps: f64) -> bool {
    let d1 = cross2(b - a, c - a);
    let d2 = cross2(b - a, d - a);
    let d3 = cross2(d - c, a - c);
    let d4 = cross2(d - c, b - c);
    ((d1 > eps && d2 < -eps) || (d1 < -eps && d2 > eps)) && ((d3 > eps && d4 < -eps) || (d3 < -eps && d4 > eps))
}

fn point_segment_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = ((p - a).dot(ab) / ab.dot(ab).max(1e-300)).clamp(0.0, 1.0);
    (p - (a + t * ab)).length()
}

/// Smallest sine of the corner angles of a polygon given in order; 0 when any corner is reflex.
fn shape_quality(p: &[DVec2]) -> f64 {
    let n = p.len();
    let mut q = 1.0f64;
    for i in 0..n {
        let a = p[(i + n - 1) % n] - p[i];
        let b = p[(i + 1) % n] - p[i];
        let (la, lb) = (a.length(), b.length());
        if la < 1e-300 || lb < 1e-300 {
            return 0.0;
        }
        let s = cross2(b, a) / (la * lb);
        if s <= 0.0 {
            return 0.0;
        }
        q = q.min(s);
    }
    q
}

struct Chord {
    i: usize,
    j: usize,
    score: f64,
}

/// The best cut of one polygon. A cut scores for carrying on an edge that arrives at either end from outside the
/// face, for leaving both pieces with an even side count, for cutting off a well shaped quad, for not making thin
/// corners, and for being short.
fn best_chord(mesh: &Mesh, f: Index, corners: &[Index], min_angle: f64) -> Chord {
    let none = Chord { i: 0, j: 0, score: -1e300 };
    let n = corners.len();
    let pts: Vec<Index> = corners.iter().map(|&c| mesh.corner_point(c)).collect();
    let pos: Vec<DVec3> = pts.iter().map(|&p| mesh.position(p)).collect();
    let mut nrm = mesh.face_area_vector(f);
    let area = nrm.length();
    if area < 1e-300 {
        return none;
    }
    nrm /= area;
    let mut ax = pos[1] - pos[0];
    ax -= nrm * nrm.dot(ax);
    if ax.length() < 1e-300 {
        ax = if nrm.x.abs() < 0.6 { DVec3::X } else { DVec3::Y };
    }
    ax = (ax - nrm * nrm.dot(ax)).normalize();
    let ay = nrm.cross(ax);
    let p: Vec<DVec2> = pos
        .iter()
        .map(|&q| {
            let d = q - pos[0];
            DVec2::new(d.dot(ax), d.dot(ay))
        })
        .collect();
    let mut perimeter = 0.0;
    for k in 0..n {
        perimeter += (p[(k + 1) % n] - p[k]).length();
    }
    let mean_edge = perimeter / n as f64;
    let eps = 1e-9 * mean_edge * mean_edge;

    // Directions arriving at each point along edges outside this face, flattened into the face plane.
    let mut arriving: Vec<Vec<DVec2>> = vec![Vec::new(); n];
    for k in 0..n {
        let prev = pts[(k + n - 1) % n];
        let next = pts[(k + 1) % n];
        mesh.for_each_point_edge(pts[k], |e| {
            let o = mesh.edge_other_point(e, pts[k]);
            if o == prev || o == next {
                return;
            }
            let d = pos[k] - mesh.position(o);
            let d2 = DVec2::new(d.dot(ax), d.dot(ay));
            let l = d2.length();
            if l > 1e-12 * mean_edge {
                arriving[k].push(d2 / l);
            }
        });
    }
    let carry_on = |k: usize, dir: DVec2| {
        let best = arriving[k].iter().fold(0.0f64, |b, a| b.max(a.dot(dir)));
        best * best
    };

    let min_rad = min_angle.to_radians();
    let even_total = n % 2 == 0;
    let mut best = none;
    let mut piece: Vec<DVec2> = Vec::new();
    for i in 0..n {
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            if mesh.find_edge(pts[i], pts[j]) != INVALID || pts[i] == pts[j] {
                continue;
            }
            let c = p[j] - p[i];
            let len = c.length();
            if len < 1e-12 * mean_edge {
                continue;
            }
            let dir = c / len;

            // Inside the corner wedge at both ends, and the corners left on both sides open enough.
            let mut smallest = TAU;
            let mut inside = true;
            for end in 0..2 {
                if !inside {
                    break;
                }
                let k = if end == 0 { i } else { j };
                let v = if end == 0 { dir } else { -dir };
                let to_next = p[(k + 1) % n] - p[k];
                let to_prev = p[(k + n - 1) % n] - p[k];
                let wedge = ccw_angle(to_next, to_prev);
                let a = ccw_angle(to_next, v);
                if a <= 0.0 || a >= wedge {
                    inside = false;
                }
                smallest = smallest.min(a).min(wedge - a);
            }
            if !inside || smallest < min_rad {
                continue;
            }
            let mut clear = true;
            for k in 0..n {
                if !clear {
                    break;
                }
                let k1 = (k + 1) % n;
                if k != i && k != j && k1 != i && k1 != j && segments_cross(p[i], p[j], p[k], p[k1], eps) {
                    clear = false;
                }
                if k != i && k != j && point_segment_distance(p[k], p[i], p[j]) < 1e-6 * mean_edge {
                    clear = false;
                }
            }
            if !clear {
                continue;
            }

            let mut score = 2.0 * (carry_on(i, dir) + carry_on(j, -dir));
            let a = j - i + 1;
            let b = n - (j - i) + 1;
            if even_total && a % 2 == 0 && b % 2 == 0 {
                score += 1.0;
            }
            for side in 0..2 {
                let size = if side == 0 { a } else { b };
                if size > 4 {
                    continue;
                }
                piece.clear();
                for k in 0..size {
                    piece.push(p[if side == 0 { i + k } else { (j + k) % n }]);
                }
                let q = shape_quality(&piece);
                score += if size == 4 { q } else { 0.2 * q };
            }
            score += 0.5 * smallest.min(FRAC_PI_2).sin();
            score -= 0.3 * len / mean_edge;
            if score > best.score {
                best = Chord { i, j, score };
            }
        }
    }
    best
}

/// Cuts each face with more than max_sides sides into quads, and a triangle where the count is odd, between its own
/// points. Cuts that carry on an edge arriving from outside the face win, so edge loops run on through it. A face that
/// can't be cut without thin corners keeps its remaining sides. Returns the resulting faces.
pub fn split_ngons(mesh: &mut Mesh, faces: &[Index], opts: &NgonSplitOptions) -> Vec<Index> {
    let max_sides = opts.max_sides.max(3) as usize;
    let mut work: Vec<Index> = faces.to_vec();
    let mut out = Vec::new();
    while let Some(f) = work.pop() {
        if !mesh.face_valid(f) {
            continue;
        }
        let corners = mesh.face_corners(f);
        if corners.len() <= max_sides {
            out.push(f);
            continue;
        }
        let c = best_chord(mesh, f, &corners, opts.min_angle);
        if c.score <= -1e300 {
            out.push(f);
            continue;
        }
        let g = mesh.split_face(f, corners[c.i], corners[c.j]);
        work.push(f);
        if g != INVALID {
            work.push(g);
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}
