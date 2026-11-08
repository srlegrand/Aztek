//! Impossible states, per vertex. After each frame every simulated vertex is checked for what the solve could not
//! fix: left through a collider, pushed through deeper than it can be brought back, squeezed between opposite
//! contacts, stretched past the limit, swinging back and forth faster than its thickness each frame, or a value that
//! stopped being finite. Each adds to a trouble score that decays by 0.6 a frame; at 2 the vertex leaves.
//!
//! Leaving, its simulation weight ramps from 1 to 0 over `blend_frames`, smoothed at both ends, and its position blends
//! towards the fallback: its animated input, or the average of its neighbours (Auto takes the input where it is near and
//! not itself inside something). Out, it is checked each frame at the fallback place; after `return_frames` frames in a
//! possible state in a row it ramps back the same way. It comes back after `most_frames_out` frames whatever happens,
//! learning its sides from where it is, so none stays out for good. While out, constraints treat it as kinematic.

use glam::DVec3;

use crate::body::{BodyKind, Fallback, Phase};
use crate::solver::{Mode, Signal, Solver};
use crate::util::smoothstep;

/// Moves vertices that are not fully simulated towards their fallback, by their weight this substep.
pub(crate) fn blend(s: &mut Solver, a1: f64, h: f64) {
    s.blend_ws.clear();
    s.blend_ws.resize(s.pt.len(), 1.0);
    for i in 0..s.pt.len() {
        let r = s.pt.rs[i];
        if r.w >= 1.0 && r.w_prev >= 1.0 {
            continue;
        }
        let ws = smoothstep(r.w_prev + (r.w - r.w_prev) * a1);
        s.blend_ws[i] = ws;
        // The fallback moves no faster than anything simulated may.
        let x = s.pt.x[i];
        let fb = x + crate::util::clamp_len(fallback_target(s, i) - x, s.settings.max_velocity * h);
        let p = fb + (s.pt.p[i] - fb) * ws;
        if crate::util::finite(p) {
            s.pt.p[i] = p;
        }
    }
}

pub(crate) fn fallback_target(s: &Solver, i: usize) -> DVec3 {
    let body = &s.bodies[s.pt.body[i] as usize];
    match s.fallback_of(i) {
        Fallback::Animation if body.kind == BodyKind::Cloth => s.anim_now[i],
        _ if body.kind == BodyKind::Cloth => {
            let l = s.pt.local[i] as usize;
            let around = &body.topo.neighbours[l];
            if around.is_empty() {
                return s.anim_now[i];
            }
            // The plain average of where the neighbours are: next to simulated ones it relaxes to a smooth patch
            // between them, and with none it can only gather in, never drift.
            let mut sum = DVec3::ZERO;
            for &k in around {
                sum += s.pt.p[body.start + k as usize];
            }
            sum / around.len() as f64
        }
        _ => s.pt.x0[i],
    }
}

/// The trouble a vertex shows this frame.
fn score(s: &Solver, i: usize, dt: f64) -> f64 {
    let sig: &Signal = &s.pt.sig[i];
    let mut t = 0.0;
    if sig.nan {
        t += 10.0;
    }
    if sig.deep {
        t += 2.0;
    }
    if sig.pen > 0.5 {
        t += 1.0;
    }
    if sig.squeeze {
        t += 1.0;
    }
    let body = &s.bodies[s.pt.body[i] as usize];
    if body.kind == BodyKind::Cloth {
        let l = s.pt.local[i] as usize;
        let rest = if body.settings.rest_from_input { &body.anim } else { &body.rest };
        let mut worst = 0.0f64;
        for &k in &body.topo.neighbours[l] {
            let r = (rest[l] - rest[k as usize]).length();
            if r > 1e-9 {
                worst = worst.max((s.pt.x[i] - s.pt.x[body.start + k as usize]).length() / r);
            }
        }
        if worst > body.settings.stretch_limit {
            t += 1.0;
        }
    }
    let (v, vp) = (s.pt.v[i], s.pt.v_prev_frame.get(i).copied().unwrap_or(DVec3::ZERO));
    let (lv, lp) = (v.length(), vp.length());
    if lv * dt > s.pt.radius[i].max(1e-9) * 2.0 && lp * dt > s.pt.radius[i].max(1e-9) * 2.0 && v.dot(vp) < -0.5 * lv * lp {
        t += 0.75;
    }
    t
}

pub(crate) fn end_frame(s: &mut Solver, dt: f64) {
    let n = s.pt.len();
    let scores: Vec<f64> = (0..n).map(|i| score(s, i, dt)).collect();
    let mut relearn = Vec::new();
    let frame = s.frame;
    for i in 0..n {
        let body = &s.bodies[s.pt.body[i] as usize];
        let st = &body.settings;
        let on = st.resolve && matches!(s.pt.phase[i], Phase::Cloth | Phase::Particle) && s.pt.inv_mass[i] > 0.0;
        let step = 1.0 / st.blend_frames.max(1.0);
        let r = &mut s.pt.rs[i];
        r.w_prev = r.w;
        if !on {
            *r = Default::default();
            continue;
        }
        let sc = scores[i];
        r.trouble = r.trouble * 0.6 + sc;
        let possible = sc == 0.0;
        match r.mode {
            Mode::Sim => {
                if r.trouble >= 2.0 {
                    r.mode = Mode::Leaving;
                    r.out_frames = 0;
                    r.calm = 0;
                    r.left_at = frame;
                    s.stats.left += 1;
                }
            }
            Mode::Leaving | Mode::Out => {
                r.out_frames += 1;
                r.calm = if possible { r.calm + 1 } else { 0 };
                if r.mode == Mode::Leaving {
                    r.w = (r.w - step).max(0.0);
                    if r.w <= 0.0 {
                        r.mode = Mode::Out;
                    }
                }
                if r.calm >= st.return_frames || r.out_frames >= st.most_frames_out {
                    r.mode = Mode::Returning;
                    r.trouble = 0.0;
                    relearn.push(i);
                }
            }
            Mode::Returning => {
                r.out_frames += 1;
                r.w = (r.w + step).min(1.0);
                if r.trouble >= 2.0 && r.out_frames < st.most_frames_out {
                    r.mode = Mode::Leaving;
                    r.calm = 0;
                } else if r.w >= 1.0 {
                    r.mode = Mode::Sim;
                    r.trouble = 0.0;
                    s.stats.returned += 1;
                }
            }
        }
        s.stats.longest_out = s.stats.longest_out.max(r.out_frames * (r.mode != Mode::Sim) as u32);
    }
    if !relearn.is_empty() {
        let fd = crate::contacts::FrameData::colliders_only(s);
        for i in relearn {
            let x = s.pt.x[i];
            s.pt.sides[i] = crate::contacts::sides_at(s, &fd, i, x);
        }
    }
    for sig in &mut s.pt.sig {
        sig.nan = false;
    }
    s.stats.out = s.pt.rs.iter().filter(|r| r.w < 1.0 || r.mode != Mode::Sim).count();
}
