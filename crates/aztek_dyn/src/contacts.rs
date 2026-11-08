//! Contacts. Once a frame, every particle gets the collider triangles it could reach, and pairs are found between
//! particles and cloth triangles (self and between bodies, collider vertices included) and between particles. Each
//! substep then resolves them by position: no velocity is ever added to push things apart, so a contact cannot blow up.
//!
//! Colliders: each particle remembers which side of each nearby collider it belongs on, learnt where it first meets it.
//! A particle found on the wrong side of a face, no deeper than the collider moved this frame plus twice the thickness,
//! is put back on its side: a fast collider cannot tunnel through. Deeper than that it is not pushed (which way is
//! unknown) and is reported as an impossible state. The thickness is a soft zone: the part of a contact inside the
//! surface is undone at once; the part inside the zone is undone with the collider compliance.

use glam::DVec3;
use rayon::prelude::*;

use crate::util::PAR_CHUNK;

use crate::body::{BodyKind, Phase};
use crate::geom::{self, Aabb, Feature};
use crate::hash::SpatialHash;
use crate::solver::{Sides, Signal, Solver};
use crate::util::finite;

/// How far into its thickness a contact may open per substep beyond where it started. Pushing a point back out to
/// the full thickness at once, from wherever it began the substep, turns into outward speed: energy the contact never
/// had. Below the surface it goes back at once; in the soft zone it rises this much of the thickness at most.
const SOFT_RISE: f64 = 0.25;

/// The gap a contact should have after this substep: the thickness, but no more than a little above where it started
/// or is now, and never below the surface.
#[inline]
fn zone_target(gap_now: f64, gap_start: f64, thick: f64) -> f64 {
    thick.min(gap_start.max(gap_now.max(0.0)) + SOFT_RISE * thick)
}

/// A particle (or collider vertex, when `kin`) against a cloth triangle.
#[derive(Clone, Copy, Debug)]
pub struct PtTri {
    pub point: u32,
    pub kin: bool,
    pub tri: u32,
    pub side: f64,
    pub thick: f64,
    pub friction: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct PtPt {
    pub a: u32,
    pub b: u32,
    pub r: f64,
    pub friction: f64,
}

#[derive(Default)]
pub struct FrameData {
    /// Collider triangles: (body, triangle).
    pub col_tris: Vec<(u32, u32)>,
    /// How far each moved this frame.
    pub col_motion: Vec<f64>,
    pub col_hash: Option<SpatialHash>,
    /// Per particle, its candidate collider triangles: cand[cand_start[i]..cand_start[i + 1]].
    pub cand_start: Vec<u32>,
    pub cand: Vec<u32>,
    /// Cloth triangles in particle indices.
    pub dyn_tris: Vec<[u32; 3]>,
    pub kin: Vec<(u32, u32)>,
    pub pt_tri: Vec<PtTri>,
    pub pt_pt: Vec<PtPt>,
}

impl FrameData {
    pub fn pairs(&self) -> usize {
        self.pt_tri.len() + self.pt_pt.len() + self.cand.len()
    }

    fn collider_tris(s: &Solver, swept: bool) -> (Vec<(u32, u32)>, Vec<f64>, Vec<Aabb>) {
        let mut tris = Vec::new();
        let mut motion = Vec::new();
        let mut bounds = Vec::new();
        for (bi, b) in s.bodies.iter().enumerate() {
            if b.kind != BodyKind::Collider || !b.settings.enabled || !b.settings.collide {
                continue;
            }
            for (ti, t) in b.topo.triangles.iter().enumerate() {
                let mut bb = Aabb::EMPTY;
                let mut m = 0.0f64;
                for &k in t {
                    let (p0, p1) = (b.anim_prev[k as usize], b.anim[k as usize]);
                    bb.add(p1);
                    if swept {
                        bb.add(p0);
                        m = m.max((p1 - p0).length());
                    }
                }
                tris.push((bi as u32, ti as u32));
                motion.push(m);
                bounds.push(bb.grow(b.settings.thickness));
            }
        }
        (tris, motion, bounds)
    }

    fn cell_for(bounds: &[Aabb], least: f64) -> f64 {
        if bounds.is_empty() {
            return least.max(1e-3);
        }
        let mut sizes: Vec<f64> = bounds.iter().map(|b| b.size().max_element()).filter(|x| x.is_finite()).collect();
        if sizes.is_empty() {
            return least.max(1e-3);
        }
        let mid = sizes.len() / 2;
        let (_, m, _) = sizes.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
        m.max(least).max(1e-4)
    }

    /// The collider triangles where they are now, for learning sides.
    pub fn colliders_only(s: &Solver) -> FrameData {
        let (col_tris, col_motion, bounds) = Self::collider_tris(s, false);
        let least = s.pt.radius.iter().fold(0.0f64, |m, &r| m.max(r)) * 2.0;
        let col_hash = Some(SpatialHash::build(Self::cell_for(&bounds, least), &bounds));
        FrameData { col_tris, col_motion, col_hash, ..Default::default() }
    }

    /// Contacts for the next `dt` seconds, starting `a0` of the way through the frame.
    pub fn build(s: &mut Solver, dt: f64, a0: f64) -> FrameData {
        let t0 = std::time::Instant::now();
        let (col_tris, col_motion, bounds) = Self::collider_tris(s, true);
        let n = s.pt.len();
        let rmax = s.pt.radius.iter().fold(0.0f64, |m, &r| m.max(r));
        let col_thick = s.bodies.iter().filter(|b| b.kind == BodyKind::Collider).fold(0.0f64, |m, b| m.max(b.settings.thickness));
        let col_hash = SpatialHash::build(Self::cell_for(&bounds, rmax * 2.0), &bounds);
        let vmax = s.settings.max_velocity;
        let collides: Vec<bool> = s.bodies.iter().map(|b| b.settings.enabled && b.settings.collide).collect();
        // Where each particle may get to this frame.
        let reach: Vec<Aabb> = (0..n)
            .into_par_iter()
            .with_min_len(PAR_CHUNK)
            .map(|i| {
                let x = s.pt.x[i];
                let to = x + crate::util::clamp_len(s.pt.v[i] * dt, vmax * dt);
                let mut b = Aabb::EMPTY;
                b.add(x);
                b.add(to);
                // A vertex out of the simulation goes to its input, which must be checked too.
                if s.pt.phase[i] == Phase::Cloth && s.pt.rs[i].mode != crate::solver::Mode::Sim {
                    b.add(s.anim_target_end(i));
                }
                // Wide enough to see a point that went through, not only one touching.
                b.grow((s.pt.radius[i] + col_thick) * 4.0 + rmax)
            })
            .collect();
        let per: Vec<Vec<u32>> = (0..n)
            .into_par_iter()
            .with_min_len(PAR_CHUNK)
            .map_init(|| (Vec::new(), crate::hash::Seen::default()), |(buf, seen), i| {
                if !collides[s.pt.body[i] as usize] || col_tris.is_empty() {
                    return Vec::new();
                }
                // A bucket holds whatever hashed to it: only what really comes near is kept.
                col_hash.query_marked(&reach[i], &bounds, seen, buf);
                buf.clone()
            })
            .collect();
        s.stats.add("prep collider candidates", t0);
        let t0 = std::time::Instant::now();
        let mut cand_start = Vec::with_capacity(n + 1);
        let mut cand = Vec::new();
        cand_start.push(0u32);
        for (i, list) in per.iter().enumerate() {
            cand.extend_from_slice(list);
            cand_start.push(cand.len() as u32);
            // Far from a collider, a particle forgets which side it was on.
            let sides = &mut s.pt.sides[i];
            let mut k = 0;
            while k < sides.n as usize {
                let b = sides.body[k];
                if list.iter().any(|&c| col_tris[c as usize].0 == b) {
                    k += 1;
                } else {
                    sides.forget(b);
                }
            }
        }

        s.stats.add("prep forget sides", t0);
        let t0 = std::time::Instant::now();
        // Cloth triangles and what may touch them.
        let mut dyn_tris = Vec::new();
        let mut dyn_body = Vec::new();
        for (bi, b) in s.bodies.iter().enumerate() {
            if b.kind == BodyKind::Cloth && b.settings.enabled {
                for t in &b.topo.triangles {
                    dyn_tris.push(t.map(|k| (b.start + k as usize) as u32));
                    dyn_body.push(bi as u32);
                }
            }
        }
        let mut kin = Vec::new();
        let mut pt_tri = Vec::new();
        let mut pt_pt = Vec::new();
        // Pairs need two things that can meet: a cloth against itself, against another body, or against particles or
        // a collider's corners.
        let cloths = s.bodies.iter().filter(|b| b.kind == BodyKind::Cloth && b.settings.enabled && b.settings.collide).count();
        let others = s.bodies.iter().any(|b| b.settings.enabled && b.settings.collide && matches!(b.kind, BodyKind::Particles | BodyKind::Rigid | BodyKind::Collider));
        let selfish = s.bodies.iter().any(|b| b.kind == BodyKind::Cloth && b.settings.self_collide && b.settings.collide);
        let any_cloth = !dyn_tris.is_empty() && (cloths > 1 || others || selfish);
        let fric: Vec<f64> = s.bodies.iter().map(|b| b.settings.friction).collect();
        if any_cloth {
            // Motion relative to the mean of everything moving: what brings things together. A garment falling as one
            // keeps its bounds small.
            let movers: Vec<DVec3> = (0..n).filter(|&i| s.pt.inv_mass[i] > 0.0).map(|i| s.pt.v[i]).filter(|v| finite(*v)).collect();
            let mean_v = if movers.is_empty() { DVec3::ZERO } else { movers.iter().sum::<DVec3>() / movers.len() as f64 };
            let motion = |i: usize| crate::util::clamp_len((s.pt.v[i] - mean_v) * dt, vmax * dt).length();
            let tri_bounds: Vec<Aabb> = dyn_tris
                .iter()
                .zip(&dyn_body)
                .map(|(t, &b)| {
                    let mut bb = Aabb::EMPTY;
                    let mut m = 0.0f64;
                    for &k in t {
                        bb.add(s.pt.x[k as usize]);
                        m = m.max(motion(k as usize));
                    }
                    let st = &s.bodies[b as usize].settings;
                    bb.grow(st.thickness.max(st.self_thickness) + m)
                })
                .collect();
            let mut all = Aabb::EMPTY;
            for b in &tri_bounds {
                all.add(b.lo);
                all.add(b.hi);
            }
            let th = std::time::Instant::now();
            let hash = SpatialHash::build(Self::cell_for(&tri_bounds, rmax), &tri_bounds);
            if std::env::var("DYN_DEBUG").is_ok() {
                eprintln!("cloth hash: {} tris, cell {:.4}, built in {:.1} ms, {}", tri_bounds.len(), Self::cell_for(&tri_bounds, rmax), th.elapsed().as_secs_f64() * 1e3, hash.describe());
            }
            // Collider vertices near cloth, as points that cannot move.
            for (bi, b) in s.bodies.iter().enumerate().filter(|_| !crate::util::off("kin")) {
                if b.kind == BodyKind::Collider && b.settings.enabled && b.settings.collide {
                    let grown = all.grow(b.settings.thickness + rmax);
                    for (l, p) in b.anim.iter().enumerate() {
                        let q = b.anim_prev[l];
                        if (p.cmpge(grown.lo).all() && p.cmple(grown.hi).all()) || (q.cmpge(grown.lo).all() && q.cmple(grown.hi).all()) {
                            kin.push((bi as u32, l as u32));
                        }
                    }
                }
            }
            let points = n + kin.len();
            s.stats.add("prep pairs: bounds, hash, corners", t0);
            let tq = std::time::Instant::now();
            let found: Vec<Vec<PtTri>> = (0..points)
                .into_par_iter()
            .with_min_len(PAR_CHUNK)
                .map_init(|| (Vec::new(), crate::hash::Seen::default()), |(buf, seen), k| {
                    let mut out = Vec::new();
                    let (x, xb, is_kin, reach_r, pbody) = if k < n {
                        let b = s.pt.body[k];
                        if !collides[b as usize] {
                            return out;
                        }
                        (s.pt.x[k], b, false, s.pt.radius[k] + motion(k), b)
                    } else {
                        let (b, l) = kin[k - n];
                        let body = &s.bodies[b as usize];
                        let (p0, p1) = (body.anim_prev[l as usize], body.anim[l as usize]);
                        (p0.lerp(p1, a0), b, true, body.settings.thickness + (p1 - p0).length(), b)
                    };
                    let mut bb = Aabb::EMPTY;
                    bb.add(x);
                    let reach_box = bb.grow(reach_r);
                    hash.query_marked(&reach_box, &tri_bounds, seen, buf);
                    for &t in buf.iter() {
                        let tri = dyn_tris[t as usize];
                        let tb = dyn_body[t as usize];
                        if !is_kin && tri.contains(&(k as u32)) {
                            continue;
                        }
                        let same = !is_kin && tb == xb;
                        let tst = &s.bodies[tb as usize].settings;
                        if same && !tst.self_collide {
                            continue;
                        }
                        let thick = if is_kin {
                            tst.thickness + s.bodies[pbody as usize].settings.thickness
                        } else if same {
                            tst.self_thickness
                        } else if s.pt.phase[k] == Phase::Cloth {
                            tst.thickness.max(s.pt.radius[k])
                        } else {
                            tst.thickness + s.pt.radius[k]
                        };
                        let [a, b, c] = tri.map(|g| s.pt.x[g as usize]);
                        let (w, feat) = geom::closest_on_triangle(x, a, b, c);
                        if feat != Feature::Face && (x - geom::at(w, a, b, c)).length() > thick {
                            continue;
                        }
                        let d = x - geom::at(w, a, b, c);
                        let nrm = geom::normal(a, b, c);
                        if nrm == DVec3::ZERO {
                            continue;
                        }
                        let tri_motion = tri.iter().fold(0.0f64, |m, &g| m.max(motion(g as usize)));
                        if d.length() > thick + reach_r + tri_motion {
                            continue;
                        }
                        if same {
                            // Points that are close on the rest shape stay out of each other's way by the cloth itself.
                            let body = &s.bodies[tb as usize];
                            let l = s.pt.local[k] as usize;
                            let ring = &body.topo.neighbours[l];
                            if tri.iter().any(|&g| ring.binary_search(&(g - body.start as u32)).is_ok()) {
                                continue;
                            }
                            let rt = tri.map(|g| body.rest[g as usize - body.start]);
                            let (w, _) = geom::closest_on_triangle(body.rest[l], rt[0], rt[1], rt[2]);
                            if (body.rest[l] - geom::at(w, rt[0], rt[1], rt[2])).length() < thick * 2.0 {
                                continue;
                            }
                        }

                        let side = if d.dot(nrm) < 0.0 { -1.0 } else { 1.0 };
                        let friction = 0.5 * (fric[tb as usize] + fric[pbody as usize]);
                        out.push(PtTri { point: if is_kin { (k - n) as u32 } else { k as u32 }, kin: is_kin, tri: t, side, thick, friction });
                    }
                    out
                })
                .collect();
            pt_tri = found.into_iter().flatten().collect();
            s.stats.add("prep pairs: queries", tq);
            if std::env::var("DYN_DEBUG").is_ok() {
                eprintln!("pairs: {} points ({} collider corners), {} cloth triangles, cell {:.4}, {} pairs, {}", points, kin.len(), dyn_tris.len(), Self::cell_for(&tri_bounds, rmax), pt_tri.len(), hash.describe());
            }
        }
        s.stats.add("prep cloth pairs", t0);
        // Particles against particles (and cloth vertices against particles).
        let others: Vec<u32> = (0..n as u32).filter(|&i| s.pt.phase[i as usize] != Phase::Cloth && collides[s.pt.body[i as usize] as usize]).collect();
        if !others.is_empty() {
            let pts: Vec<u32> = (0..n as u32).filter(|&i| collides[s.pt.body[i as usize] as usize]).collect();
            let bounds: Vec<Aabb> = pts
                .iter()
                .map(|&i| {
                    let mut b = Aabb::EMPTY;
                    b.add(s.pt.x[i as usize]);
                    b.grow(s.pt.radius[i as usize])
                })
                .collect();
            let hash = SpatialHash::build(rmax * 2.0, &bounds);
            let found: Vec<Vec<PtPt>> = others
                .par_iter()
            .with_min_len(PAR_CHUNK / 2)
                .map_init(|| (Vec::new(), crate::hash::Seen::default()), |(buf, seen), &i| {
                    let mut out = Vec::new();
                    let iu = i as usize;
                    let xi = s.pt.x[iu];
                    let m = crate::util::clamp_len(s.pt.v[iu] * dt, vmax * dt).length();
                    let mut bb = Aabb::EMPTY;
                    bb.add(xi);
                    let reach_box = bb.grow(s.pt.radius[iu] + rmax + m * 2.0);
                    hash.query_marked(&reach_box, &bounds, seen, buf);
                    for &k in buf.iter() {
                        let j = pts[k as usize] as usize;
                        let cloth_j = s.pt.phase[j] == Phase::Cloth;
                        // Each pair once: particles in order; cloth vertices only from the particle's side.
                        if j == iu || (!cloth_j && j < iu) {
                            continue;
                        }
                        let (bi, bj) = (s.pt.body[iu], s.pt.body[j]);
                        if bi == bj && (s.pt.phase[iu] == Phase::Rigid || !s.bodies[bi as usize].settings.self_collide) {
                            continue;
                        }
                        let r = s.pt.radius[iu] + s.pt.radius[j];
                        if (xi - s.pt.x[j]).length() < r + m * 2.0 + rmax {
                            out.push(PtPt { a: i, b: j as u32, r, friction: 0.5 * (fric[bi as usize] + fric[bj as usize]) });
                        }
                    }
                    out
                })
                .collect();
            pt_pt = found.into_iter().flatten().collect();
        }
        FrameData { col_tris, col_motion, col_hash: Some(col_hash), cand_start, cand, dyn_tris, kin, pt_tri, pt_pt }
    }
}

impl Solver {
    /// A cloth vertex's input position at the end of the frame.
    pub(crate) fn anim_target_end(&self, i: usize) -> DVec3 {
        let b = &self.bodies[self.pt.body[i] as usize];
        b.anim.get(self.pt.local[i] as usize).copied().unwrap_or(self.pt.x[i])
    }
}

/// The sides of nearby colliders a point is on.
pub(crate) fn sides_at(s: &Solver, fd: &FrameData, i: usize, x: DVec3) -> Sides {
    let mut sides = Sides::default();
    let Some(hash) = &fd.col_hash else { return sides };
    let mut buf = Vec::new();
    let r = s.pt.radius[i] * 3.0 + s.bodies.iter().filter(|b| b.kind == BodyKind::Collider).fold(0.0f64, |m, b| m.max(b.settings.thickness * 3.0));
    let mut bb = Aabb::EMPTY;
    bb.add(x);
    hash.query(&bb.grow(r), &mut buf);
    let mut best: Vec<(u32, f64, f64)> = Vec::new();
    for &c in &buf {
        let (b, t) = fd.col_tris[c as usize];
        let Some([p0, p1, p2]) = s.tri_points(b as usize, t as usize) else { continue };
        let (w, _) = geom::closest_on_triangle(x, p0, p1, p2);
        let d = x - geom::at(w, p0, p1, p2);
        let n = geom::normal(p0, p1, p2);
        let (dist, sd) = (d.length(), d.dot(n));
        if dist > r {
            continue;
        }
        match best.iter_mut().find(|e| e.0 == b) {
            Some(e) if dist < e.1 => *e = (b, dist, sd),
            Some(_) => {}
            None => best.push((b, dist, sd)),
        }
    }
    for (b, _, sd) in best {
        if s.bodies[b as usize].outside.is_none() {
            sides.set(b, sd);
        }
    }
    sides
}

/// Particle and cloth contacts, Jacobi style: corrections are summed per particle in a fixed order and averaged.
pub(crate) fn solve_pairs(s: &mut Solver, _h: f64, a0: f64, a1: f64) {
    let fd = &s.frame_data;
    if fd.pt_tri.is_empty() && fd.pt_pt.is_empty() {
        return;
    }
    let p = &s.pt.p;
    let x = &s.pt.x;
    let im = &s.im;
    let kin_at = |k: u32, a: f64| {
        let (b, l) = fd.kin[k as usize];
        let body = &s.bodies[b as usize];
        body.anim_prev[l as usize].lerp(body.anim[l as usize], a)
    };
    type Fix = ([(u32, DVec3); 4], u8);
    let tri_fixes: Vec<Fix> = fd
        .pt_tri
        .par_iter()
            .with_min_len(PAR_CHUNK / 2)
        .map(|c| {
            let mut out: Fix = ([(0, DVec3::ZERO); 4], 0);
            let tri = fd.dyn_tris[c.tri as usize];
            let (pp, xp, wp) = if c.kin { (kin_at(c.point, a1), kin_at(c.point, a0), 0.0) } else { (p[c.point as usize], x[c.point as usize], im[c.point as usize]) };
            let tp = tri.map(|g| p[g as usize]);
            let tw = tri.map(|g| im[g as usize]);
            let (bw, feat) = geom::closest_on_triangle(pp, tp[0], tp[1], tp[2]);
            // Over the face only: beyond an edge the neighbouring triangle holds it, and the normal says nothing there.
            if feat != Feature::Face {
                return out;
            }
            let n = geom::normal(tp[0], tp[1], tp[2]) * c.side;
            if n == DVec3::ZERO {
                return out;
            }
            let q = geom::at(bw, tp[0], tp[1], tp[2]);
            let tx = tri.map(|g| x[g as usize]);
            let gap = (pp - q).dot(n);
            let target = zone_target(gap, (xp - geom::at(bw, tx[0], tx[1], tx[2])).dot(n), c.thick);
            // Far through on the wrong side, the side learnt at the search no longer says which way is out: leave it,
            // as a collider does. And no push moves more than twice the thickness in a step.
            if gap < -2.0 * c.thick {
                return out;
            }
            let cval = (gap - target).max(-2.0 * c.thick);
            if cval >= 0.0 {
                return out;
            }
            let wt = bw.x * bw.x * tw[0] + bw.y * bw.y * tw[1] + bw.z * bw.z * tw[2];
            let denom = wp + wt;
            if !(denom > 0.0) {
                return out;
            }
            let dl = -cval / denom;
            // Friction: undo sliding up to the friction times the push.
            let rel = (pp - xp) - (geom::at(bw, tp[0], tp[1], tp[2]) - geom::at(bw, tx[0], tx[1], tx[2]));
            let tang = rel - n * rel.dot(n);
            let tl = tang.length();
            let fr = if tl > 1e-12 { tang * (c.friction * (-cval) / tl).min(1.0) / denom } else { DVec3::ZERO };
            let mut push = |i: u32, d: DVec3| {
                if finite(d) && d != DVec3::ZERO {
                    out.0[out.1 as usize] = (i, d);
                    out.1 += 1;
                }
            };
            if !c.kin {
                push(c.point, n * (wp * dl) - fr * wp);
            }
            for k in 0..3 {
                let b = bw[k];
                push(tri[k], -n * (tw[k] * b * dl) + fr * (tw[k] * b));
            }
            out
        })
        .collect();
    let pair_fixes: Vec<Fix> = fd
        .pt_pt
        .par_iter()
            .with_min_len(PAR_CHUNK / 2)
        .map(|c| {
            let mut out: Fix = ([(0, DVec3::ZERO); 4], 0);
            let (a, b) = (c.a as usize, c.b as usize);
            let (wa, wb) = (im[a], im[b]);
            let d = p[a] - p[b];
            let l = d.length();
            let target = zone_target(l, (x[a] - x[b]).length(), c.r).min(l + c.r);
            if l >= target || !(wa + wb > 0.0) || l < 1e-12 {
                return out;
            }
            let n = d / l;
            let dl = (target - l) / (wa + wb);
            let rel = (p[a] - x[a]) - (p[b] - x[b]);
            let tang = rel - n * rel.dot(n);
            let tl = tang.length();
            let fr = if tl > 1e-12 { tang * (c.friction * (target - l) / tl).min(1.0) / (wa + wb) } else { DVec3::ZERO };
            let da = n * (wa * dl) - fr * wa;
            let db = -n * (wb * dl) + fr * wb;
            if finite(da) && finite(db) {
                out.0[0] = (c.a, da);
                out.0[1] = (c.b, db);
                out.1 = 2;
            }
            out
        })
        .collect();
    let n = s.pt.len();
    let mut sum = vec![DVec3::ZERO; n];
    let mut count = vec![0u32; n];
    let mut contacts = 0;
    for f in tri_fixes.iter().chain(&pair_fixes) {
        if f.1 > 0 {
            contacts += 1;
        }
        for &(i, d) in &f.0[..f.1 as usize] {
            sum[i as usize] += d;
            count[i as usize] += 1;
        }
    }
    s.stats.contacts += contacts;
    s.pt.p.par_iter_mut().with_min_len(PAR_CHUNK).enumerate().for_each(|(i, p)| {
        if count[i] > 0 {
            let to = *p + sum[i] / count[i] as f64;
            if finite(to) {
                *p = to;
            }
        }
    });
}

/// The nearest triangle of one collider to a point.
pub(crate) struct Hit {
    pub body: u32,
    pub tri: u32,
    pub cand: u32,
    pub w: DVec3,
    pub feat: Feature,
    pub q: DVec3,
    pub d: DVec3,
    pub dist: f64,
    pub n: DVec3,
    /// The nearest feature lies on the collider's open border.
    pub border: bool,
    /// The triangle's longest edge.
    pub size: f64,
}

/// For each collider among the candidates, its nearest triangle to `pos` at blend `a` through the frame.
pub(crate) fn closest_per_body(s: &Solver, fd: &FrameData, cands: &[u32], pos: DVec3, a: f64) -> Vec<Hit> {
    let mut hits: Vec<Hit> = Vec::new();
    for &c in cands {
        let (cb, t) = fd.col_tris[c as usize];
        let body = &s.bodies[cb as usize];
        let tri = body.topo.triangles[t as usize];
        let [p0, p1, p2] = tri.map(|k| body.anim_prev[k as usize].lerp(body.anim[k as usize], a));
        let n = geom::normal(p0, p1, p2);
        if n == DVec3::ZERO {
            continue;
        }
        let (w, feat) = geom::closest_on_triangle(pos, p0, p1, p2);
        let q = geom::at(w, p0, p1, p2);
        let d = pos - q;
        let dist = d.length();
        let better = |h: &Hit| dist < h.dist - 1e-12 || (dist <= h.dist + 1e-12 && feat == Feature::Face && h.feat != Feature::Face);
        let border = feat != Feature::Face && (0..3).filter(|&k| w[k] > 1e-9).all(|k| body.topo.border[tri[k] as usize]);
        let size = (p1 - p0).length().max((p2 - p1).length()).max((p0 - p2).length());
        let hit = Hit { body: cb, tri: t, cand: c, w, feat, q, d, dist, n, border, size };
        match hits.iter_mut().find(|h| h.body == cb) {
            Some(h) => {
                if better(h) {
                    *h = hit;
                }
            }
            None => hits.push(hit),
        }
    }
    hits
}

/// Collider contacts for every particle, with what is found wrong on the last substep.
pub(crate) fn solve_colliders(s: &mut Solver, h: f64, a0: f64, a1: f64, last: bool) {
    let fd = &s.frame_data;
    let ground = s.settings.ground.then_some((s.settings.ground_height, s.settings.ground_friction));
    if fd.cand.is_empty() && ground.is_none() {
        if last {
            for sig in &mut s.pt.sig {
                let nan = sig.nan;
                *sig = Signal { nan, ..Default::default() };
            }
        }
        return;
    }
    let tri_at = |b: u32, t: u32, a: f64| -> [DVec3; 3] {
        let body = &s.bodies[b as usize];
        body.topo.triangles[t as usize].map(|k| body.anim_prev[k as usize].lerp(body.anim[k as usize], a))
    };
    let res: Vec<(DVec3, Signal, Sides, usize)> = (0..s.pt.len())
        .into_par_iter()
            .with_min_len(PAR_CHUNK)
        .map(|i| {
            let mut pp = s.pt.p[i];
            let xs = s.pt.x[i];
            let w = s.im[i];
            let mut sides = s.pt.sides[i];
            let mut sig = s.pt.sig[i];
            if last {
                sig = Signal { nan: sig.nan, ..Default::default() };
            }
            let body = &s.bodies[s.pt.body[i] as usize];
            if !body.settings.collide || !body.settings.enabled {
                return (pp, sig, sides, 0);
            }
            let th_p = s.pt.radius[i];
            let mut contacts = 0;
            let mut normals: Vec<DVec3> = Vec::new();
            if let Some((gh, gf)) = ground {
                let target = zone_target(pp.y - gh, xs.y - gh, th_p);
                let d = pp.y - gh - target;
                if d < 0.0 && w > 0.0 {
                    pp.y -= d;
                    let rel = pp - xs;
                    let tang = DVec3::new(rel.x, 0.0, rel.z);
                    let tl = tang.length();
                    if tl > 1e-12 {
                        pp -= tang * (gf * -d / tl).min(1.0);
                    }
                    normals.push(DVec3::Y);
                    contacts += 1;
                }
            }
            let range = fd.cand_start.get(i).copied().unwrap_or(0) as usize..fd.cand_start.get(i + 1).copied().unwrap_or(0) as usize;
            let cands = &fd.cand[range];
            for hit in closest_per_body(s, fd, cands, pp, a1) {
                let cb = hit.body;
                let cst = &s.bodies[cb as usize].settings;
                let th = th_p + cst.thickness;
                let side = match s.bodies[cb as usize].outside.or_else(|| sides.get(cb)) {
                    Some(sd) => sd,
                    None => {
                        // First meeting: the side it was on at the start of the substep.
                        let [a0p, b0p, c0p] = tri_at(cb, hit.tri, a0);
                        let (w0, _) = geom::closest_on_triangle(xs, a0p, b0p, c0p);
                        let n0 = geom::normal(a0p, b0p, c0p);
                        let sd = if (xs - geom::at(w0, a0p, b0p, c0p)).dot(n0) < 0.0 { -1.0 } else { 1.0 };
                        sides.set(cb, sd);
                        sd
                    }
                };
                let s_raw = hit.d.dot(hit.n);
                // How deep a point may be and still be brought back unambiguously: past its own motion and the
                // collider's, and a quarter of the triangle across, which way out is no longer clear.
                let max_pen = 2.0 * th + fd.col_motion[hit.cand as usize] + (pp - xs).length() + hit.size * 0.25;
                let soft_k = if w > 0.0 { w / (w + cst.collide_compliance / (h * h)) } else { 0.0 };
                // Where the point was at the start of the substep, against the surface where it was then.
                let [a0p, b0p, c0p] = tri_at(cb, hit.tri, a0);
                let q0 = geom::at(hit.w, a0p, b0p, c0p);
                // Below the surface: straight back to it; in the soft zone, a gentle rise towards the thickness.
                let zone = |gap_now: f64, normal: DVec3| -> f64 {
                    let target = zone_target(gap_now, (xs - q0).dot(normal), th);
                    (-gap_now).max(0.0) + (target - gap_now.max(0.0)).max(0.0) * soft_k
                };
                let (nn, move_by) = if hit.feat == Feature::Face {
                    let sd = s_raw * side;
                    if sd >= th {
                        continue;
                    }
                    if sd < -max_pen {
                        if last {
                            sig.deep = true;
                        }
                        continue;
                    }
                    let nn = hit.n * side;
                    (nn, zone(sd, nn))
                } else if hit.dist < 1e-12 {
                    let nn = hit.n * side;
                    (nn, zone(0.0, nn))
                } else if s_raw * side >= 0.0 {
                    // Outside, nearest an edge or corner: away from it.
                    if hit.dist >= th {
                        if hit.border && hit.dist > th * 2.0 {
                            // Round an open border and well away: the side no longer matters, learn it again.
                            sides.set(cb, s_raw);
                        }
                        continue;
                    }
                    let nn = hit.d / hit.dist;
                    (nn, zone(hit.dist, nn))
                } else {
                    // Inside, nearest an edge or corner: out through it.
                    if hit.dist > max_pen {
                        if last {
                            sig.deep = true;
                        }
                        continue;
                    }
                    let nn = -hit.d / hit.dist;
                    (nn, zone(-hit.dist, nn))
                };
                contacts += 1;
                normals.push(nn);
                if w <= 0.0 || move_by <= 0.0 {
                    continue;
                }
                pp += nn * move_by;
                // Friction against the collider's own motion at that place.
                let dq = hit.q - q0;
                let rel = (pp - xs) - dq;
                let tang = rel - nn * rel.dot(nn);
                let tl = tang.length();
                let mu = 0.5 * (body.settings.friction + cst.friction);
                if tl > 1e-12 && !crate::util::off("friction") {
                    pp -= tang * (mu * move_by / tl).min(1.0);
                }
            }
            if last {
                if let Some(nrm) = normals.first() {
                    sig.normal = *nrm;
                }
                // What is left after the solve, and whether the animated place is itself inside something.
                let out = s.pt.rs[i].w < 1.0 || s.pt.rs[i].mode != crate::solver::Mode::Sim;
                // Contacts pressing well into their thickness zone: (outward normal) for each.
                let mut pressing: Vec<DVec3> = Vec::new();
                for (pos, is_anim) in [(pp, false), (s.anim_now[i], true)] {
                    if is_anim && !out {
                        continue;
                    }
                    for hit in closest_per_body(s, fd, cands, pos, a1) {
                        let th = th_p + s.bodies[hit.body as usize].settings.thickness;
                        let side = s.bodies[hit.body as usize].outside.or_else(|| sides.get(hit.body)).unwrap_or(1.0);
                        let s_raw = hit.d.dot(hit.n) * side;
                        let depth = if hit.feat == Feature::Face { -s_raw } else if s_raw < 0.0 { hit.dist } else { 0.0 };
                        if is_anim {
                            if depth > 0.25 * th {
                                sig.anim_bad = true;
                            }
                        } else {
                            if depth > 0.0 {
                                sig.pen = sig.pen.max(depth / th.max(1e-9));
                            }
                            // Within the inner half of the zone (or through): pressed by this one.
                            let gap = if hit.feat == Feature::Face { s_raw } else if s_raw < 0.0 { -hit.dist } else { hit.dist };
                            if gap < 0.5 * th && (hit.feat == Feature::Face || hit.dist > 0.0) {
                                let outward = if hit.feat == Feature::Face { hit.n * side } else { hit.d.normalize_or_zero() * if s_raw < 0.0 { -1.0 } else { 1.0 } };
                                pressing.push(outward);
                            }
                        }
                    }
                }
                if let Some((gh, _)) = ground {
                    if pp.y - gh < 0.5 * th_p {
                        pressing.push(DVec3::Y);
                    }
                }
                // Pressed from opposite sides at once: both cannot be met.
                'outer: for (k, a) in pressing.iter().enumerate() {
                    for b in &pressing[k + 1..] {
                        if a.dot(*b) < -0.3 {
                            sig.squeeze = true;
                            break 'outer;
                        }
                    }
                }
            } else if let Some(nrm) = normals.first() {
                sig.normal = *nrm;
            }
            if !finite(pp) {
                pp = s.pt.p[i];
            }
            (pp, sig, sides, contacts)
        })
        .collect();
    let mut total = 0;
    for (i, (p, sig, sides, c)) in res.into_iter().enumerate() {
        s.pt.p[i] = p;
        s.pt.sig[i] = sig;
        s.pt.sides[i] = sides;
        total += c;
    }
    s.stats.contacts += total;
}
