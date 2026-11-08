//! Prediction and the constraints projected each substep: cloth stretch, compression, bending and area, attraction to
//! the input, and links. Each correction is checked before it is applied; one that is not finite is dropped.

use glam::DVec3;
use rayon::prelude::*;

use crate::util::PAR_CHUNK;

use crate::body::{Body, BodyKind};
use crate::solver::{Anchor, Solver};
use crate::topology::Colouring;
use crate::util::{clamp_len, finite, Shared};

/// The unsigned angle between the two triangles of a bending stencil: edge (0, 1), opposite corners (2, 3).
pub fn dihedral_angle(pos: &[DVec3], s: &[u32; 4]) -> f64 {
    let (p0, p1, p2, p3) = (pos[s[2] as usize], pos[s[3] as usize], pos[s[0] as usize], pos[s[1] as usize]);
    let n1 = (p2 - p0).cross(p3 - p0).normalize_or_zero();
    let n2 = (p3 - p1).cross(p2 - p1).normalize_or_zero();
    if n1 == DVec3::ZERO || n2 == DVec3::ZERO {
        return 0.0;
    }
    n1.dot(n2).clamp(-1.0, 1.0).acos()
}

pub fn area(pos: &[DVec3], t: &[u32; 3]) -> f64 {
    let [a, b, c] = t.map(|k| pos[k as usize]);
    (b - a).cross(c - a).length() * 0.5
}

/// Rest lengths from this frame's input, for cloth that takes its rest shape from it.
pub(crate) fn rest_from_input(b: &Body) -> Option<Vec<f64>> {
    if b.kind != BodyKind::Cloth || !b.settings.rest_from_input {
        return None;
    }
    Some(b.cloth.dist.iter().map(|e| (b.anim[e[0] as usize] - b.anim[e[1] as usize]).length()).collect())
}

/// Cloth vertex normals at the frame's start, for drag; zero elsewhere.
pub(crate) fn vertex_normals(s: &Solver) -> Vec<DVec3> {
    let mut out = vec![DVec3::ZERO; s.pt.len()];
    for body in &s.bodies {
        if body.kind != BodyKind::Cloth {
            continue;
        }
        for t in &body.topo.triangles {
            let [a, b, c] = t.map(|k| body.start + k as usize);
            let n = (s.pt.x[b] - s.pt.x[a]).cross(s.pt.x[c] - s.pt.x[a]);
            if finite(n) {
                out[a] += n;
                out[b] += n;
                out[c] += n;
            }
        }
    }
    out.par_iter_mut().with_min_len(PAR_CHUNK).for_each(|n| *n = n.normalize_or_zero());
    out
}

/// Predicts where each particle goes: gravity, fields, air, damping, inside the speed limit.
pub(crate) fn integrate(s: &mut Solver, h: f64) {
    let vmax = s.settings.max_velocity;
    let (g, wind, air) = (s.settings.gravity, s.settings.wind, s.settings.air_density);
    let res: Vec<(DVec3, DVec3)> = (0..s.pt.len())
        .into_par_iter()
            .with_min_len(PAR_CHUNK)
        .map(|i| {
            let x = s.pt.x[i];
            let bi = s.pt.body[i];
            let body = &s.bodies[bi as usize];
            let st = &body.settings;
            if s.pt.inv_mass[i] <= 0.0 {
                // Pinned or massless: follows its input.
                let p = if body.kind == BodyKind::Cloth { s.anim_now[i] } else { x };
                return (p, DVec3::ZERO);
            }
            let mass = 1.0 / s.pt.inv_mass[i];
            let v = s.pt.v[i];
            let mut a = g * st.gravity_scale + s.field_accel(bi, x, v, mass);
            // Air drag, quadratic as on a plate: cloth by each point's share of the surface across the flow and along
            // it, particles and rigid points by their cross-section.
            let rel = v - wind;
            let n = s.normals[i];
            let force = if n != DVec3::ZERO && body.kind == BodyKind::Cloth {
                let share = body.cloth.point_area.get(s.pt.local[i] as usize).copied().unwrap_or(0.0);
                let vn = n * rel.dot(n);
                let vt = rel - vn;
                (vn * vn.length() * st.drag + vt * vt.length() * st.tangential_drag) * (0.5 * air * share)
            } else {
                let r = s.pt.radius[i];
                rel * rel.length() * (0.5 * air * st.drag * std::f64::consts::PI * r * r)
            };
            // Drag can stop a point but never turn it round within a step.
            let dv = crate::util::clamp_len(force / mass * h, rel.length());
            a -= dv / h;
            let mut v = v + a * h;
            v *= (1.0 - st.damping * h).max(0.0);
            v = clamp_len(v, vmax);
            if !finite(v) {
                v = DVec3::ZERO;
            }
            (x + v * h, v)
        })
        .collect();
    s.v_int = Vec::with_capacity(res.len());
    for (i, (p, v)) in res.into_iter().enumerate() {
        s.pt.p[i] = p;
        s.v_int.push(v);
        s.pt.sig[i].normal = DVec3::ZERO;
    }
}

/// Constraints a thread takes at least, within one colour.
const COLOUR_CHUNK: usize = 256;

/// Runs `f` over each colour's constraints at once; the last colour one at a time when colouring ran out.
fn by_colour(col: &Colouring, f: impl Fn(usize) + Sync) {
    for (ci, r) in col.ranges.iter().enumerate() {
        if col.serial && ci + 1 == col.ranges.len() {
            r.clone().for_each(&f);
        } else if r.len() < COLOUR_CHUNK * 2 {
            r.clone().for_each(&f);
        } else {
            r.clone().into_par_iter().with_min_len(COLOUR_CHUNK).for_each(&f);
        }
    }
}

thread_local! {
    static SUMS: std::cell::RefCell<Vec<(DVec3, u32)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Stretch limits, Jacobi style: each holds the two ends of a path no further apart than the path's rest length
/// along the mesh, so it never acts on folds, only on stretch. Corrections are averaged per vertex.
/// (Pulling in along the whole path was tried: the inner gradients cancel on a straight path, so the ends jump.)
fn paths(c: &crate::body::ClothData, o: usize, im: &[f64], p: &Shared<DVec3>, alpha: f64) {
    if c.path_rest.is_empty() || crate::util::off("limit") {
        return;
    }
    let fixes: Vec<[(u32, DVec3); 2]> = (0..c.path_rest.len())
        .into_par_iter()
        .with_min_len(PAR_CHUNK)
        .map(|k| {
            let none = [(u32::MAX, DVec3::ZERO); 2];
            let chain = &c.path[c.path_start[k] as usize..c.path_start[k + 1] as usize];
            let (a, b) = (chain[0], chain[chain.len() - 1]);
            let (ia, ib) = (o + a as usize, o + b as usize);
            let (wa, wb) = (im[ia], im[ib]);
            if wa + wb <= 0.0 {
                return none;
            }
            let d = p.get(ia) - p.get(ib);
            let l = d.length();
            let cval = l - c.path_rest[k];
            if !(cval > 0.0) || !l.is_finite() {
                return none;
            }
            let n = d / l;
            let dl = -cval / (wa + wb + alpha);
            [(a, n * (wa * dl)), (b, -n * (wb * dl))]
        })
        .collect();
    // Summed in path order into the vertex's slot: the same on any number of threads.
    let mut touched: Vec<u32> = Vec::new();
    SUMS.with(|cell| {
        let mut sums = cell.borrow_mut();
        for f in &fixes {
            for &(v, d) in f {
                if v != u32::MAX && finite(d) {
                    let v = v as usize;
                    if sums.len() <= v {
                        sums.resize(v + 1, (DVec3::ZERO, 0));
                    }
                    if sums[v].1 == 0 {
                        touched.push(v as u32);
                    }
                    sums[v].0 += d;
                    sums[v].1 += 1;
                }
            }
        }
        for &v in &touched {
            let (d, n) = std::mem::take(&mut sums[v as usize]);
            let i = o + v as usize;
            let to = p.get(i) + d / n as f64;
            if finite(to) {
                p.set(i, to);
            }
        }
    });
}

/// Stretch, compression, bending and area for every cloth body.
pub(crate) fn cloth(s: &mut Solver, h: f64, stretch_passes: u32) {
    let h2 = h * h;
    let p = Shared::new(&mut s.pt.p);
    let im = &s.im;
    for (bi, body) in s.bodies.iter().enumerate() {
        if body.kind != BodyKind::Cloth || !body.settings.enabled {
            continue;
        }
        let o = body.start;
        let c = &body.cloth;
        let st = &body.settings;
        let rest_now = s.rest_now.get(bi).and_then(|r| r.as_ref());
        let torn = &body.torn;
        let limit_alpha = 0.0;
        for _ in 0..stretch_passes {
            by_colour(&c.dist_colour, |k| {
                if torn[k] {
                    return;
                }
                let [a, b] = c.dist[k];
                let (ia, ib) = (o + a as usize, o + b as usize);
                let (wa, wb) = (im[ia], im[ib]);
                let wsum = wa + wb;
                if wsum <= 0.0 {
                    return;
                }
                let (pa, pb) = (p.get(ia), p.get(ib));
                let d = pa - pb;
                let l = d.length();
                if !(l > 1e-12) {
                    return;
                }
                let rest = rest_now.map_or(c.dist_rest[k], |r| r[k]);
                let cval = l - rest;
                // Compliance is one over the edge's stiffness: the material's times its geometric factor.
                let stiff = if cval > 0.0 { st.stretch_stiffness } else { st.compression_stiffness } * c.dist_geo[k];
                let alpha = if stiff > 0.0 { 1.0 / (stiff * h2) } else { f64::INFINITY };
                if !alpha.is_finite() {
                    return;
                }
                let dl = -cval / (wsum + alpha);
                let n = d / l;
                let (na, nb) = (pa + n * (wa * dl), pb - n * (wb * dl));
                if finite(na) && finite(nb) {
                    p.set(ia, na);
                    p.set(ib, nb);
                }
            });
        }
        paths(c, o, im, &p, limit_alpha);
        // Long range attachments: straight onto the ball around the anchor, which does not move for them.
        if !c.tether.is_empty() && !crate::util::off("tether") {
            let moved: Vec<(usize, DVec3)> = c
                .tether
                .par_iter()
                .with_min_len(PAR_CHUNK)
                .filter_map(|&(v, a, d)| {
                    let (iv, ia) = (o + v as usize, o + a as usize);
                    if im[iv] <= 0.0 {
                        return None;
                    }
                    let (pv, pa) = (p.get(iv), p.get(ia));
                    let off = pv - pa;
                    let l = off.length();
                    (l > d && l.is_finite()).then(|| (iv, pa + off * (d / l)))
                })
                .collect();
            for (i, to) in moved {
                if finite(to) {
                    p.set(i, to);
                }
            }
        }
        let bend_k = st.bend_stiffness;
        by_colour(&c.bend_colour, |k| {
            if crate::util::off("bend") {
                return;
            }
            let st4 = c.bend[k];
            let [i2, i3, i0, i1] = st4.map(|x| o + x as usize);
            let (w0, w1, w2, w3) = (im[i0], im[i1], im[i2], im[i3]);
            if w0 + w1 + w2 + w3 <= 0.0 {
                return;
            }
            let (p0, p1, p2, p3) = (p.get(i0), p.get(i1), p.get(i2), p.get(i3));
            let e = p3 - p2;
            let elen = e.length();
            if !(elen > 1e-9) {
                return;
            }
            let inv_e = 1.0 / elen;
            let mut n1 = (p2 - p0).cross(p3 - p0);
            let mut n2 = (p3 - p1).cross(p2 - p1);
            let (l1, l2) = (n1.length_squared(), n2.length_squared());
            if !(l1 > 1e-18) || !(l2 > 1e-18) {
                return;
            }
            n1 /= l1;
            n2 /= l2;
            let d0 = n1 * elen;
            let d1 = n2 * elen;
            let d2 = n1 * ((p0 - p3).dot(e) * inv_e) + n2 * ((p1 - p3).dot(e) * inv_e);
            let d3 = n1 * ((p2 - p0).dot(e) * inv_e) + n2 * ((p2 - p1).dot(e) * inv_e);
            let (u1, u2) = (n1.normalize(), n2.normalize());
            let phi = u1.dot(u2).clamp(-1.0, 1.0).acos();
            let denom = w0 * d0.length_squared() + w1 * d1.length_squared() + w2 * d2.length_squared() + w3 * d3.length_squared();
            if !(denom > 1e-18) {
                return;
            }
            let kb = bend_k * c.bend_geo[k];
            if !(kb > 0.0) {
                return;
            }
            let mut dl = (phi - c.bend_rest[k]) / (denom + 1.0 / (kb * h2));
            if u1.cross(u2).dot(e) > 0.0 {
                dl = -dl;
            }
            let q = [(i0, p0 - d0 * (w0 * dl)), (i1, p1 - d1 * (w1 * dl)), (i2, p2 - d2 * (w2 * dl)), (i3, p3 - d3 * (w3 * dl))];
            if q.iter().all(|(_, v)| finite(*v)) {
                for (i, v) in q {
                    p.set(i, v);
                }
            }
        });
        if let Some(ak) = st.area_stiffness.filter(|&k| k > 0.0) {
            by_colour(&c.area_colour, |k| {
                let t = c.area[k].map(|x| o + x as usize);
                let w = t.map(|i| im[i]);
                let x = t.map(|i| p.get(i));
                let nn = (x[1] - x[0]).cross(x[2] - x[0]);
                let a2 = nn.length();
                if !(a2 > 1e-18) {
                    return;
                }
                let n = nn / a2;
                let g = [n.cross(x[2] - x[1]) * 0.5, n.cross(x[0] - x[2]) * 0.5, n.cross(x[1] - x[0]) * 0.5];
                let denom: f64 = (0..3).map(|j| w[j] * g[j].length_squared()).sum();
                if !(denom > 1e-18) {
                    return;
                }
                let cval = a2 * 0.5 - c.area_rest[k];
                // Energy (k / A0) dA^2 / 2: compliance A0 / k.
                let alpha = c.area_rest[k].max(1e-18) / (ak * h2);
                let dl = -cval / (denom + alpha);
                let q: Vec<DVec3> = (0..3).map(|j| x[j] + g[j] * (w[j] * dl)).collect();
                if q.iter().all(|v| finite(*v)) {
                    for j in 0..3 {
                        p.set(t[j], q[j]);
                    }
                }
            });
        }
    }
}

/// Pulls cloth towards its animated input by the attraction setting and map.
pub(crate) fn attract(s: &mut Solver, h: f64) {
    for body in &s.bodies {
        if body.kind != BodyKind::Cloth {
            continue;
        }
        let st = &body.settings;
        if st.attract <= 0.0 && st.attract_map.is_none() {
            continue;
        }
        for l in 0..body.count {
            let i = body.start + l;
            let a = (st.attract * body.map(&st.attract_map, l, 1.0)).clamp(0.0, 1.0);
            if a <= 0.0 || s.im[i] <= 0.0 {
                continue;
            }
            // The same pull per second whatever the step: a = 1 follows exactly.
            let k = if a >= 1.0 { 1.0 } else { 1.0 - (1.0 - a).powf(h * 24.0) };
            let to = s.pt.p[i] + (s.anim_now[i] - s.pt.p[i]) * k;
            if finite(to) {
                s.pt.p[i] = to;
            }
        }
    }
}

fn anchor_point(s: &Solver, a: &Anchor, a1: f64) -> Option<(DVec3, Option<usize>)> {
    match *a {
        Anchor::Particle(b, l) => {
            let body = s.bodies.get(b as usize)?;
            if body.kind == BodyKind::Collider {
                let p = body.anim_prev.get(l as usize)?.lerp(*body.anim.get(l as usize)?, a1);
                return Some((p, None));
            }
            let i = body.start + l as usize;
            ((l as usize) < body.count && i < s.pt.len()).then_some(())?;
            Some((s.pt.p[i], Some(i)))
        }
        Anchor::World(p) => Some((p, None)),
        Anchor::Surface(b, t, w) => {
            let body = s.bodies.get(b as usize)?;
            let tri = body.topo.triangles.get(t as usize)?;
            let pts = tri.map(|k| {
                if body.kind == BodyKind::Collider {
                    body.anim_prev[k as usize].lerp(body.anim[k as usize], a1)
                } else {
                    s.pt.p[body.start + k as usize]
                }
            });
            Some((crate::geom::at(w, pts[0], pts[1], pts[2]), None))
        }
    }
}

/// Links, one after another: they are few.
pub(crate) fn links(s: &mut Solver, h: f64, a1: f64) {
    for k in 0..s.links.len() {
        let l = &s.links[k];
        if !l.enabled || l.broken {
            continue;
        }
        let (Some((pa, ia)), Some((pb, ib))) = (anchor_point(s, &l.a, a1), anchor_point(s, &l.b, a1)) else { continue };
        let wa = ia.map_or(0.0, |i| s.im[i]);
        let wb = ib.map_or(0.0, |i| s.im[i]);
        let wsum = wa + wb;
        if wsum <= 0.0 {
            continue;
        }
        let d = pa - pb;
        let len = d.length();
        let cval = len - l.rest;
        if l.tear > 0.0 && cval > l.tear {
            s.links[k].broken = true;
            continue;
        }
        let n = if len > 1e-12 { d / len } else { DVec3::ZERO };
        if n == DVec3::ZERO {
            continue;
        }
        let dl = -cval / (wsum + l.compliance / (h * h));
        if let Some(i) = ia {
            let v = pa + n * (wa * dl);
            if finite(v) {
                s.pt.p[i] = v;
            }
        }
        if let Some(i) = ib {
            let v = pb - n * (wb * dl);
            if finite(v) {
                s.pt.p[i] = v;
            }
        }
    }
}

/// Tears edges stretched past their body's tear ratio.
pub(crate) fn tear(s: &mut Solver) {
    for bi in 0..s.bodies.len() {
        let body = &s.bodies[bi];
        if body.kind != BodyKind::Cloth || body.settings.tear_ratio <= 0.0 {
            continue;
        }
        let (o, ratio) = (body.start, body.settings.tear_ratio);
        let c = body.cloth.clone();
        let x = &s.pt.x;
        let newly: Vec<usize> = (0..c.dist.len())
            .filter(|&k| {
                let [a, b] = c.dist[k];
                let l = (x[o + a as usize] - x[o + b as usize]).length();
                l > c.dist_rest[k] * ratio
            })
            .collect();
        for k in newly {
            s.bodies[bi].torn[k] = true;
        }
    }
}
