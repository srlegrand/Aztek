//! Traces the first frames of a ball pushed through pinned cloth.
#[path = "../tests/common/mod.rs"]
mod common;
use aztek_dyn::{BodySettings, Solver, SolverSettings};
use common::*;
use glam::DVec3;
fn main() {
    let mut s = Solver::new(SolverSettings::default());
    let n = 16;
    let (pts, faces) = grid(n, 2.0, 0.0);
    let mut pin = vec![0.0; (n + 1) * (n + 1)];
    for i in 0..=n {
        pin[i] = 1.0;
        pin[n * (n + 1) + i] = 1.0;
    }
    let c = s.add_cloth("cloth", pts.clone(), &faces, BodySettings { pin_map: Some(pin), self_collide: std::env::var("SELF").is_ok(), ..cloth_settings(0.02) });
    let (sp, sf) = sphere(0.4, DVec3::ZERO, 16, 8);
    let ball = s.add_collider("ball", sp.clone(), &sf, BodySettings { thickness: 0.01, ..Default::default() });
    s.reset();
    for f in 0..3 {
        let y = 1.5 - f as f64 * 0.5;
        let moved: Vec<DVec3> = sp.iter().map(|p| *p + DVec3::new(0.0, y, 0.0)).collect();
        s.set_input(ball, &moved);
        s.set_input(c, &pts);
        s.step(1.0 / 24.0);
        let x = s.positions(c);
        println!("frame {f}: stretch {:.3} v91 {:.3} v92 {:.3} sides91 {:?} out {}", worst_stretch(&s, c), x[91], x[92], s.pt.sides[91], s.out_count(c));
    }
}
