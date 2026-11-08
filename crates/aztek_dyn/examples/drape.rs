//! Times a drape: cargo run --release -p aztek_dyn --example drape -- [grid] [frames]
#[path = "../tests/common/mod.rs"]
mod common;

use aztek_dyn::{BodySettings, Solver, SolverSettings};
use common::*;
use glam::DVec3;

fn main() {
    let args: Vec<usize> = std::env::args().skip(1).filter_map(|a| a.parse().ok()).collect();
    let n = args.first().copied().unwrap_or(30);
    let frames = args.get(1).copied().unwrap_or(72);
    let env = |k: &str, d: u32| std::env::var(k).ok().and_then(|v| v.parse().ok()).unwrap_or(d);
    let mut s = Solver::new(SolverSettings { substeps: env("SUB", SolverSettings::default().substeps), iterations: env("ITER", SolverSettings::default().iterations), ..Default::default() });
    let (pts, faces) = grid(n, 2.0, 1.0);
    // Thickness well under half an edge, as a garment would have.
    let mut cs = cloth_settings((2.0 / n as f64 * 0.3).min(0.02));
    cs.self_collide = std::env::var("SELF").map_or(true, |v| v != "0");
    if let Some(c) = std::env::var("COMP").ok().and_then(|v| v.parse().ok()) {
        cs.compression_stiffness = c;
    }
    if std::env::var("NORESOLVE").is_ok() {
        cs.resolve = false;
    }
    if std::env::var("PIN").is_ok() {
        // Hung from two corners.
        let mut pin = vec![0.0; (n + 1) * (n + 1)];
        pin[0] = 1.0;
        pin[n] = 1.0;
        cs.pin_map = Some(pin);
    }
    if let Some(f) = std::env::var("FRIC").ok().and_then(|v| v.parse().ok()) {
        cs.friction = f;
    }
    if let Some(f) = std::env::var("SOFT").ok().and_then(|v| v.parse().ok()) {
        cs.collide_compliance = f;
    }
    let c = s.add_cloth("cloth", pts, &faces, cs);
    let (sp, sf) = sphere(0.5, DVec3::ZERO, 32, 16);
    if std::env::var("BALL").map_or(true, |v| v != "0") {
    s.add_collider("ball", sp, &sf, BodySettings { thickness: 0.0, friction: std::env::var("FRIC").ok().and_then(|v| v.parse().ok()).unwrap_or(0.3), ..Default::default() });
    }
    s.reset();
    let mut prev: Vec<DVec3> = Vec::new();
    for f in 0..frames {
        let t = std::time::Instant::now();
        s.step(1.0 / 24.0);
        let st = &s.stats;
        let least = s.positions(c).iter().map(|p| p.length()).fold(f64::INFINITY, f64::min);
        if std::env::var("SIG").is_ok() {
            for i in 0..s.pt.len() {
                if s.pt.rs[i].mode != aztek_dyn::Mode::Sim && s.pt.rs[i].out_frames == 0 {
                    println!("  leaves {i} {:?} x {:?} v {:?} vprev {:?}", s.pt.sig[i], s.pt.x[i], s.pt.v[i], s.pt.v_prev_frame[i]);
                }
            }
        }
        if std::env::var("QUIET").is_ok() && f + 1 < frames {
            continue;
        }
        if std::env::var("DETAIL").is_ok() {
            let d: Vec<String> = s.stats.detail.iter().map(|(k, v)| format!("{k} {v:.1}")).collect();
            println!("  {}", d.join(", "));
        }
        let worst = common::worst_stretch(&s, c);
        if std::env::var("SPEED").is_ok() {
            let v = s.velocities(c);
            let top = v.iter().map(|v| v.length()).fold(0.0, f64::max);
            let at = v.iter().position(|q| q.length() == top).unwrap_or(0);
            let w = n + 1;
            println!("  fastest point {at} (row {}, column {}) at {:.3} |p| {:.3}", at / w, at % w, s.positions(c)[at], s.positions(c)[at].length());
            let fast = v.iter().filter(|v| v.length() > 0.2).count();
            let lowest = s.positions(c).iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
            let x = s.positions(c).to_vec();
            let moved = if prev.len() == x.len() { x.iter().zip(&prev).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max) } else { 0.0 };
            prev = x;
            println!("  most moved in the frame {:.4} m", moved);
            println!("  top speed {top:.2}, {fast} over 0.2 m/s, lowest y {lowest:.3}, clamped {}, fixed {}", s.stats.clamped, s.stats.fixed);
        }
        if std::env::var("WHERE").is_ok() {
            let body = &s.bodies[c];
            let x = s.positions(c);
            let mut ratios: Vec<(f64, usize)> = body.topo.edges.iter().enumerate().map(|(k, e)| ((x[e[0] as usize] - x[e[1] as usize]).length() / (body.rest[e[0] as usize] - body.rest[e[1] as usize]).length(), k)).collect();
            ratios.sort_by(|a, b| b.0.total_cmp(&a.0));
            let over = ratios.iter().filter(|r| r.0 > 1.1).count();
            let mean = ratios.iter().map(|r| r.0).sum::<f64>() / ratios.len() as f64;
            let e = body.topo.edges[ratios[0].1];
            let (a, b) = (x[e[0] as usize], x[e[1] as usize]);
            println!("  mean {mean:.3}, {over} edges over 1.1; worst edge {} {} at {a:.3} |a| {:.3} and {b:.3} |b| {:.3}, sim weights {:.2} {:.2}", e[0], e[1], a.length(), b.length(), s.pt.rs[e[0] as usize].w, s.pt.rs[e[1] as usize].w);
        }
        print!("stretch {worst:.3} ");
        println!("{f:3} {:7.2} ms  prep {:6.2} cons {:6.2} coll {:6.2} other {:6.2}  pairs {} contacts {} out {} inside {:.3}", t.elapsed().as_secs_f64() * 1e3, st.ms_prepare, st.ms_constraints, st.ms_collide, st.ms_other, st.pairs, st.contacts, st.out, least);
    }
}
