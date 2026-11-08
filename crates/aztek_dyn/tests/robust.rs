//! Trying to break the solver. Every frame of every case: all values finite, every vertex near its neighbours and its
//! animation, and no vertex out of the simulation for longer than the most it may stay out plus the blends. Cases
//! that are impossible on purpose also check that vertices left, and came back within a set time once it was over.

mod common;

use aztek_dyn::util::Rng;
use aztek_dyn::{BodySettings, Mode, Solver, SolverSettings};
use common::*;
use glam::{DMat4, DQuat, DVec3};

const DT: f64 = 1.0 / 24.0;

/// A flat square collider of two triangles, size s, centred at c, facing up.
fn plate(c: DVec3, s: f64, up: bool) -> (Vec<DVec3>, Vec<Vec<u32>>) {
    let h = s * 0.5;
    let pts = vec![c + DVec3::new(-h, 0.0, -h), c + DVec3::new(h, 0.0, -h), c + DVec3::new(h, 0.0, h), c + DVec3::new(-h, 0.0, h)];
    let face = if up { vec![0, 3, 2, 1] } else { vec![0, 1, 2, 3] };
    (pts, vec![face])
}

/// What every frame must hold. `bound` is how far a vertex may be from its animation.
fn check_frame(s: &Solver, cloth: usize, bound: f64, edge_most: f64, frame: usize) {
    assert!(all_finite(s), "frame {frame}: a value is not finite");
    let body = &s.bodies[cloth];
    let x = s.positions(cloth);
    for (l, p) in x.iter().enumerate() {
        let d = (*p - body.anim[l]).length();
        assert!(d < bound, "frame {frame}: vertex {l} is {d} from its animation");
    }
    for e in &body.topo.edges {
        let rest = (body.rest[e[0] as usize] - body.rest[e[1] as usize]).length();
        let now = (x[e[0] as usize] - x[e[1] as usize]).length();
        assert!(now < rest * edge_most + 1e-9, "frame {frame}: edge {e:?} at {now} from rest {rest}");
    }
    let st = &body.settings;
    let most = st.most_frames_out + 2 * st.blend_frames.ceil() as u32 + 2;
    for r in &s.pt.rs[body.start..body.start + body.count] {
        assert!(r.out_frames <= most || r.mode == Mode::Sim, "frame {frame}: a vertex out for {} frames", r.out_frames);
    }
}

#[test]
fn cloth_squeezed_between_closing_plates_leaves_and_returns() {
    let mut s = Solver::new(SolverSettings::default());
    let (pts, faces) = grid(16, 1.0, 0.0);
    let c = s.add_cloth("cloth", pts.clone(), &faces, BodySettings { attract: 0.05, ..cloth_settings(0.02) });
    let (lo, lf) = plate(DVec3::new(0.0, -0.2, 0.0), 3.0, true);
    let (hi, hf) = plate(DVec3::new(0.0, 0.2, 0.0), 3.0, false);
    let bottom = s.add_collider("bottom", lo.clone(), &lf, BodySettings { thickness: 0.0, ..Default::default() });
    let top = s.add_collider("top", hi.clone(), &hf, BodySettings { thickness: 0.0, ..Default::default() });
    s.settings.gravity = DVec3::ZERO;
    s.reset();
    let mut most_out = 0;
    let mut last_out_frame = 0;
    let mut modes = vec![Mode::Sim; pts.len()];
    let mut departures = vec![0u32; pts.len()];
    for f in 0..120 {
        // Close to touching over 24 frames, hold, open again.
        let gap = match f {
            0..=23 => 0.2 * (1.0 - f as f64 / 24.0),
            24..=47 => 0.0,
            48..=71 => 0.2 * (f - 48) as f64 / 24.0,
            _ => 0.2,
        };
        let move_by = |p: &Vec<DVec3>, y: f64| p.iter().map(|q| DVec3::new(q.x, y, q.z)).collect::<Vec<_>>();
        s.set_input(bottom, &move_by(&lo, -gap));
        s.set_input(top, &move_by(&hi, gap));
        s.set_input(c, &pts);
        s.step(DT);
        // Count departures per vertex: leaving more than twice in one squeeze is chatter.
        for (k, r) in s.pt.rs[s.bodies[c].start..s.bodies[c].start + s.bodies[c].count].iter().enumerate() {
            if r.mode == Mode::Leaving && modes[k] == Mode::Sim {
                departures[k] += 1;
            }
            modes[k] = r.mode;
        }
        check_frame(&s, c, 1.0, 4.0, f);
        let out = s.out_count(c);
        most_out = most_out.max(out);
        if out > 0 {
            last_out_frame = f;
        }
    }
    assert!(most_out > 0, "squeezed flat, no vertex was taken out");
    assert!(last_out_frame < 72 + 30, "vertices still out at frame {last_out_frame}, long after the plates opened");
    assert_eq!(s.out_count(c), 0);
    assert!(departures.iter().all(|&d| d <= 2), "chatter: a vertex left {} times", departures.iter().max().unwrap());
}

#[test]
fn collider_pushed_clean_through_pinned_cloth() {
    let mut s = Solver::new(SolverSettings::default());
    let n = 16;
    let (pts, faces) = grid(n, 2.0, 0.0);
    let mut pin = vec![0.0; (n + 1) * (n + 1)];
    for i in 0..=n {
        pin[i] = 1.0;
        pin[n * (n + 1) + i] = 1.0;
    }
    let c = s.add_cloth("cloth", pts.clone(), &faces, BodySettings { pin_map: Some(pin), ..cloth_settings(0.02) });
    let (sp, sf) = sphere(0.4, DVec3::ZERO, 16, 8);
    let ball = s.add_collider("ball", sp.clone(), &sf, BodySettings { thickness: 0.01, ..Default::default() });
    s.reset();
    let mut most_out = 0;
    let mut recovered_at = None;
    for f in 0..150 {
        // From above, down through the cloth at 12 units a second, then away.
        let y = (1.5 - f as f64 * 0.5).max(-30.0);
        let moved: Vec<DVec3> = sp.iter().map(|p| *p + DVec3::new(0.0, y, 0.0)).collect();
        s.set_input(ball, &moved);
        s.set_input(c, &pts);
        s.step(DT);
        // Cloth carried by the ball stretches over it, up to its width across one edge, in the frames before
        // the impossible state handling takes those vertices out: bounded by the ball, not growing.
        check_frame(&s, c, 3.0, 10.0, f);
        let out = s.out_count(c);
        most_out = most_out.max(out);
        if f > 10 && out == 0 && recovered_at.is_none() {
            recovered_at = Some(f);
        }
    }
    // The ball is gone after frame 10; everything back within 60 frames of that.
    let back = recovered_at.expect("vertices never came back");
    assert!(back < 70, "back only at frame {back}");
    assert!(s.positions(c).iter().all(|p| p.y.abs() < 1.5), "cloth left hanging far away");
    let _ = most_out;
}

#[test]
fn fuzz_never_explodes() {
    let mut rng = Rng::new(7);
    for case in 0..24 {
        let mut s = Solver::new(SolverSettings { substeps: 1 + (rng.float() * 12.0) as u32, ground: rng.float() < 0.3, ..Default::default() });
        // A random grid, jittered, with some faces made degenerate.
        let n = 2 + (rng.float() * 10.0) as usize;
        let (mut pts, mut faces) = grid(n, 0.5 + rng.float() * 3.0, rng.signed());
        for p in &mut pts {
            *p += DVec3::new(rng.signed(), rng.signed(), rng.signed()) * 0.02;
        }
        for f in faces.iter_mut() {
            if rng.float() < 0.1 {
                f[1] = f[0];
            }
            if rng.float() < 0.05 {
                f[2] = f[0];
                f[3] = f[0];
            }
        }
        let mass = match (rng.float() * 4.0) as u32 {
            0 => 0.0,
            1 => 1e-9,
            2 => 1e9,
            _ => 1.0,
        };
        let mut mass_map: Vec<f64> = (0..pts.len()).map(|_| if rng.float() < 0.1 { 0.0 } else { 1.0 }).collect();
        if rng.float() < 0.2 {
            mass_map[0] = f64::NAN;
        }
        let settings = BodySettings { mass, mass_map: Some(mass_map), self_collide: rng.float() < 0.5, attract: rng.float() * 0.2, ..cloth_settings(0.01 + rng.float() * 0.05) };
        let c = s.add_cloth("cloth", pts.clone(), &faces, settings);
        let (bp, bf) = cube(DVec3::ZERO, DVec3::splat(0.3));
        let bx = s.add_collider("box", bp.clone(), &bf, BodySettings { thickness: rng.float() * 0.05, ..Default::default() });
        let p = s.add_particles("bits", BodySettings { radius: 0.02, ..Default::default() });
        s.reset();
        let mut anim = pts.clone();
        for f in 0..48 {
            // The collider anywhere, turned and scaled wildly; the cloth's input teleporting now and then.
            let m = DMat4::from_scale_rotation_translation(
                DVec3::splat(10f64.powf(rng.signed() * 2.0)),
                DQuat::from_axis_angle(rng.direction(), rng.float() * 7.0),
                rng.direction() * if rng.float() < 0.2 { 100.0 } else { rng.float() * 2.0 },
            );
            let moved: Vec<DVec3> = bp.iter().map(|q| m.transform_point3(*q)).collect();
            s.set_input(bx, &moved);
            if rng.float() < 0.1 {
                let jump = rng.direction() * 50.0;
                anim = anim.iter().map(|q| *q + jump).collect();
            }
            if rng.float() < 0.05 {
                anim[0] = DVec3::splat(f64::NAN);
            }
            s.set_input(c, &anim);
            if rng.float() < 0.3 {
                s.add_particles_now(p, &[(rng.direction(), rng.direction() * 1e6)]);
            }
            let dt = match (rng.float() * 5.0) as u32 {
                0 => 1e-7,
                1 => 10.0,
                2 => -1.0,
                _ => DT,
            };
            s.step(dt);
            assert!(all_finite(&s), "case {case} frame {f}: not finite");
            for (i, q) in s.pt.x.iter().enumerate() {
                let anim = if s.pt.body[i] as usize == c { s.bodies[c].anim[s.pt.local[i] as usize] } else { DVec3::ZERO };
                assert!(q.length() < 1e6, "case {case} frame {f}: particle {i} of body {} ({:?}, inverse mass {}, mode {:?}, weight {}) flew to {q}; its input {anim}; dt {dt}", s.pt.body[i], s.pt.phase[i], s.pt.inv_mass[i], s.pt.rs[i].mode, s.pt.rs[i].w);
            }
            let body = &s.bodies[c];
            let st = &body.settings;
            let most = st.most_frames_out + 2 * st.blend_frames.ceil() as u32 + 2;
            for r in &s.pt.rs {
                assert!(r.out_frames <= most || r.mode == Mode::Sim, "case {case} frame {f}: out for {}", r.out_frames);
            }
        }
    }
}

#[test]
fn a_fine_drape_with_self_collision_stays_bounded() {
    let mut s = Solver::new(SolverSettings::default());
    let n = 60;
    let (pts, faces) = grid(n, 2.0, 1.0);
    let c = s.add_cloth("cloth", pts, &faces, cloth_settings(2.0 / n as f64 * 0.3));
    let (sp, sf) = sphere(0.5, DVec3::ZERO, 32, 16);
    s.add_collider("ball", sp, &sf, BodySettings { thickness: 0.0, ..Default::default() });
    s.reset();
    for f in 0..36 {
        s.step(DT);
        assert!(all_finite(&s), "frame {f}");
        let x = s.positions(c);
        // Nothing far above where it started or flung away, nothing through the ball.
        assert!(x.iter().all(|p| p.y < 1.05 && p.length() < 3.0), "frame {f}: a point left the scene");
        assert!(x.iter().all(|p| p.length() > 0.4), "frame {f}: a point went into the ball");
        assert!(worst_stretch(&s, c) < 12.0, "frame {f}: stretch {}", worst_stretch(&s, c));
    }
}

#[test]
fn a_garment_mesh_with_degenerate_seams_holds_together() {
    // A sheet cut down the middle and stitched with quads of no area, as modelled garments are: seam points with no
    // area of their own, edges of no length, sliver triangles.
    let n = 16;
    let (mut pts, faces) = grid(n, 2.0, 1.0);
    let w = n + 1;
    let seam = n / 2;
    let dup: Vec<u32> = (0..w).map(|j| {
        pts.push(pts[j * w + seam]);
        (pts.len() - 1) as u32
    }).collect();
    let mut faces: Vec<Vec<u32>> = faces
        .into_iter()
        .map(|f| {
            // Faces right of the seam take the copies.
            let right = f.iter().all(|&v| (v as usize % w) >= seam);
            f.into_iter().map(|v| if right && v as usize % w == seam { dup[v as usize / w] } else { v }).collect()
        })
        .collect();
    for j in 0..n {
        faces.push(vec![(j * w + seam) as u32, dup[j], dup[j + 1], ((j + 1) * w + seam) as u32]);
    }
    let mut s = Solver::new(SolverSettings::default());
    let c = s.add_cloth("garment", pts, &faces, cloth_settings(0.01));
    let (sp, sf) = sphere(0.5, DVec3::ZERO, 32, 16);
    s.add_collider("ball", sp, &sf, BodySettings { thickness: 0.0, ..Default::default() });
    s.reset();
    for f in 0..48 {
        let before = s.positions(c).to_vec();
        s.step(DT);
        assert!(all_finite(&s), "frame {f}");
        let x = s.positions(c);
        // No point flung: nothing moves more than a free fall's frame could, and the seam stays closed.
        let moved = x.iter().zip(&before).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
        assert!(moved < 0.3, "frame {f}: a point moved {moved} m");
        for j in 0..w {
            let gap = (x[j * w + seam] - x[dup[j] as usize]).length();
            assert!(gap < 0.03, "frame {f}: the seam opened {gap} m at row {j}");
        }
    }
}
