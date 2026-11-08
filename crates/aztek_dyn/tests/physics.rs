//! Regression on real numbers: free fall and a pendulum against the analytic answers, a drape on a sphere, a rigid
//! stack, and the same answer on any number of threads.

mod common;

use aztek_dyn::{Anchor, BodySettings, Link, Solver, SolverSettings};
use common::*;
use glam::DVec3;

const DT: f64 = 1.0 / 24.0;

#[test]
fn free_fall_matches_the_formula() {
    let mut s = Solver::new(SolverSettings { substeps: 16, ..Default::default() });
    let b = s.add_particles("p", BodySettings { damping: 0.0, drag: 0.0, collide: false, ..Default::default() });
    s.reset();
    s.add_particles_now(b, &[(DVec3::ZERO, DVec3::ZERO)]);
    for _ in 0..24 {
        s.step(DT);
    }
    // Born at the start of the first frame, it falls for all 24.
    let t = 24.0 * DT;
    let y = s.positions(b)[0].y;
    let want = -0.5 * 9.8 * t * t;
    assert!((y - want).abs() < want.abs() * 0.01, "fell to {y}, formula {want}");
}

#[test]
fn pendulum_keeps_its_period() {
    let mut s = Solver::new(SolverSettings { substeps: 20, ..Default::default() });
    let (l, angle) = (1.0f64, 0.1f64);
    let b = s.add_particles("bob", BodySettings { damping: 0.0, drag: 0.0, collide: false, ..Default::default() });
    s.reset();
    s.add_particles_now(b, &[(DVec3::new(l * angle.sin(), -l * angle.cos(), 0.0), DVec3::ZERO)]);
    s.links.push(Link { name: "rod".into(), a: Anchor::Particle(b as u32, 0), b: Anchor::World(DVec3::ZERO), rest: l, compliance: 0.0, tear: 0.0, broken: false, enabled: true });
    s.step(DT);
    let mut crossings = Vec::new();
    let mut prev = s.positions(b)[0].x;
    for f in 0..200 {
        s.step(DT);
        let x = s.positions(b)[0].x;
        if prev > 0.0 && x <= 0.0 {
            crossings.push(f as f64 + prev / (prev - x));
        }
        prev = x;
    }
    assert!(crossings.len() >= 3, "{crossings:?}");
    let period = (crossings[crossings.len() - 1] - crossings[0]) / (crossings.len() - 1) as f64 * DT;
    let want = std::f64::consts::TAU * (l / 9.8).sqrt();
    assert!((period - want).abs() < want * 0.02, "period {period}, formula {want}");
    // Length held.
    assert!((s.positions(b)[0].length() - l).abs() < 1e-3);
}

fn drape(threads: usize) -> (Solver, usize) {
    let pool = rayon::ThreadPoolBuilder::new().num_threads(threads).build().unwrap();
    pool.install(|| {
        let mut s = Solver::new(SolverSettings::default());
        let (pts, faces) = grid(30, 2.0, 1.0);
        let c = s.add_cloth("cloth", pts, &faces, BodySettings { damping: 1.0, ..cloth_settings(0.02) });
        let (sp, sf) = sphere(0.5, DVec3::ZERO, 32, 16);
        s.add_collider("ball", sp, &sf, BodySettings { thickness: 0.0, ..Default::default() });
        s.reset();
        for _ in 0..96 {
            s.step(DT);
            assert!(all_finite(&s));
        }
        (s, c)
    })
}

#[test]
fn drape_on_a_sphere_settles() {
    let (s, c) = drape(4);
    let x = s.positions(c);
    // Nothing inside the ball.
    let least = x.iter().map(|p| p.length()).fold(f64::INFINITY, f64::min);
    assert!(least > 0.5 - 0.01, "a point at {least} from the centre");
    // The middle rests on top at the thickness.
    let top = x[15 * 31 + 15];
    assert!((top.y - 0.52).abs() < 0.02 && top.x.abs() < 0.1 && top.z.abs() < 0.1, "top at {top}");
    // Hanging, not stretched, nearly still, and nothing needed rescuing.
    assert!(worst_stretch(&s, c) < 1.15, "stretch {}", worst_stretch(&s, c));
    let corner = x[0];
    assert!(corner.y < 0.0, "corner did not fall: {corner}");
    // Nearly still: what a frame moves it, not the last substep's speed, which carries the solve's small corrections.
    let before = s.positions(c).to_vec();
    let mut s = s;
    s.step(DT);
    let moved = s.positions(c).iter().zip(&before).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
    assert!(moved < 0.02, "still moving {moved} m a frame");
    assert_eq!(s.out_count(c), 0, "vertices left the simulation in a plain drape");
}

#[test]
fn same_answer_on_any_number_of_threads() {
    let (a, c) = drape(1);
    let (b, _) = drape(8);
    let (pa, pb) = (a.positions(c), b.positions(c));
    for (p, q) in pa.iter().zip(pb) {
        assert_eq!(p.to_array().map(f64::to_bits), q.to_array().map(f64::to_bits));
    }
}

#[test]
fn rigid_stack_stays_standing() {
    let mut s = Solver::new(SolverSettings { ground: true, substeps: 10, ..Default::default() });
    let mut ids = Vec::new();
    for k in 0..3 {
        let (pts, faces) = cube(DVec3::new(0.0, 0.5 + k as f64 * 1.02, 0.0), DVec3::splat(0.5));
        let m = glam::DMat4::from_translation(DVec3::new(0.0, 0.5 + k as f64 * 1.02, 0.0));
        ids.push(s.add_rigid(&format!("box{k}"), pts, &faces, m, BodySettings { shape: aztek_dyn::RigidShape::Box, resolution: 4, friction: 0.6, ..Default::default() }));
    }
    s.reset();
    for _ in 0..120 {
        s.step(DT);
        assert!(all_finite(&s));
    }
    for (k, &b) in ids.iter().enumerate() {
        let m = s.rigid_matrix(b).unwrap();
        let at = m.w_axis.truncate();
        assert!(at.x.abs() < 0.15 && at.z.abs() < 0.15, "box {k} slid to {at}");
        assert!((at.y - (0.5 + k as f64)).abs() < 0.15, "box {k} at height {}", at.y);
        // Upright.
        assert!(m.y_axis.truncate().normalize().y > 0.98, "box {k} tipped");
    }
}

#[test]
fn fine_cloth_holds_its_length_over_a_ball() {
    // Ten thousand points of cotton over a ball: the fine edges alone carry the hanging weight only a few edges a
    // pass, and the cloth on the ball stretched by 7% on average before the long range limits.
    let mut s = Solver::new(SolverSettings::default());
    let n = 100;
    let (pts, faces) = grid(n, 2.0, 1.0);
    let c = s.add_cloth("cloth", pts, &faces, BodySettings { self_collide: false, ..cloth_settings(2.0 / n as f64 * 0.3) });
    let (sp, sf) = sphere(0.5, DVec3::ZERO, 32, 16);
    s.add_collider("ball", sp, &sf, BodySettings { thickness: 0.0, friction: 0.3, ..Default::default() });
    s.reset();
    for _ in 0..36 {
        s.step(DT);
    }
    let body = &s.bodies[c];
    let x = s.positions(c);
    let (mut sum, mut count) = (0.0, 0);
    for e in &body.topo.edges {
        let (a, b) = (x[e[0] as usize], x[e[1] as usize]);
        if a.length() < 0.55 || b.length() < 0.55 {
            sum += (a - b).length() / (body.rest[e[0] as usize] - body.rest[e[1] as usize]).length();
            count += 1;
        }
    }
    let mean = sum / count as f64;
    assert!(count > 1000, "only {count} edges on the ball");
    assert!(mean < 1.04, "edges on the ball stretched {mean} on average");
}
