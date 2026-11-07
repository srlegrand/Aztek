//! Tests: bvh.

mod common;

use std::sync::atomic::{AtomicI32, Ordering};
use std::time::Instant;

use aztek_geo::bvh::*;
use aztek_geo::primitives::*;
use aztek_geo::triangulate::Triangulation;
use aztek_geo::{Index, Mesh};
use common::*;
use glam::DVec3;
use rand_mt::Mt;
use rayon::prelude::*;

fn uniform(rng: &mut Mt) -> f64 {
    -1.0 + 2.0 * (rng.next_u32() as f64 / 4294967296.0)
}

fn brute_ray(m: &Mesh, tris: &Triangulation, o: DVec3, d: DVec3) -> Option<f64> {
    let mut best = f64::MAX;
    let mut any = false;
    for t in 0..tris.triangle_count() as usize {
        let p = |k: usize| m.position(m.corner_point(tris.corners[t * 3 + k]));
        let (a, b, c) = (p(0), p(1), p(2));
        let (e1, e2) = (b - a, c - a);
        let pv = d.cross(e2);
        let det = e1.dot(pv);
        if det == 0.0 {
            continue;
        }
        let s = o - a;
        let u = s.dot(pv) / det;
        let q = s.cross(e1);
        let v = d.dot(q) / det;
        let tt = e2.dot(q) / det;
        if u >= 0.0 && v >= 0.0 && u + v <= 1.0 && tt >= 0.0 && tt < best {
            best = tt;
            any = true;
        }
    }
    any.then_some(best)
}

#[test]
fn raycast_and_closest_point_match_brute_force() {
    let mut m = make_torus(&TorusOptions { radius: 50.0, section_radius: 20.0, subdiv_axis: 24, subdiv_height: 16, ..Default::default() });
    m.append(&make_cube(&CubeOptions { width: 30.0, height: 30.0, depth: 30.0, ..Default::default() }));
    let bvh = MeshBvh::new(&m);
    assert_eq!(bvh.triangle_count(), 24 * 16 * 2 + 12);
    assert!(approx_eps(bvh.bounds().size().x, 140.0, 1e-4));

    let mut rng = Mt::new(7);
    let mut hits = 0;
    for _ in 0..300 {
        let o = DVec3::new(uniform(&mut rng) * 120.0, uniform(&mut rng) * 120.0, uniform(&mut rng) * 120.0);
        let d = DVec3::new(uniform(&mut rng), uniform(&mut rng), uniform(&mut rng)).normalize();
        let expect = brute_ray(&m, bvh.triangles(), o, d);
        let got = bvh.raycast(o, d);
        assert_eq!(got.is_some(), expect.is_some());
        if let (Some(hit), Some(brute)) = (got, expect) {
            assert!(approx(hit.t, brute));
            hits += 1;
            assert!(hit.point.distance(o + d * brute) < 1e-6);
            assert!(approx(hit.bary.x + hit.bary.y + hit.bary.z, 1.0));
        }
        assert_eq!(bvh.occluded(o, d, 1e9), expect.is_some());
    }
    assert!(hits > 20);

    for _ in 0..100 {
        let p = DVec3::new(uniform(&mut rng) * 100.0, uniform(&mut rng) * 100.0, uniform(&mut rng) * 100.0);
        let c = bvh.closest_point(p).expect("closest");
        // No vertex may be closer than the reported surface distance.
        for v in 0..m.point_capacity() {
            assert!(m.position(v).distance(p) >= c.distance - 1e-9);
        }
        assert!(approx(c.point.distance(p), c.distance));
    }

    // Uv under a ray through the cube's top.
    let mut cube = make_cube(&CubeOptions::default());
    let mut cb = MeshBvh::new(&cube);
    let hit = cb.raycast(DVec3::new(0.0, 100.0, 0.0), DVec3::new(0.0, -1.0, 0.0)).expect("hit");
    assert!(approx(hit.t, 50.0));
    assert!(approx(cube.face_normal(hit.face).y, 1.0));
    assert!(hit.front_facing);
    let uv = cb.interpolate(cube.corner_attrs().get::<[f32; 2]>("uv0"), hit.triangle, hit.bary);
    assert!(approx(uv[0] as f64, 0.5));
    assert!(approx(uv[1] as f64, 0.375));

    // Refit after moving points.
    for p in cube.positions_mut() {
        p.y += 10.0;
    }
    cb.refit(&cube);
    let hit = cb.raycast(DVec3::new(0.0, 100.0, 0.0), DVec3::new(0.0, -1.0, 0.0)).expect("hit");
    assert!(approx(hit.t, 40.0));
    assert_eq!(cb.overlapping(&cube.bounds()).len(), 12);
}

#[test]
fn refit_points_matches_full_refit() {
    let mut m = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 32, subdiv_height: 16, ..Default::default() });
    let mut a = MeshBvh::new(&m);
    let mut b = a.clone();
    let moved: Vec<Index> = (0..m.point_capacity()).step_by(7).collect();
    for &p in &moved {
        let q = m.position(p) * 1.3;
        m.set_position(p, q);
    }
    a.refit(&m);
    b.refit_points(&m, &moved);
    assert_eq!(a.export_flat().nodes, b.export_flat().nodes);
    let flat = a.export_flat();
    assert_eq!(flat.corners.len(), a.triangle_count() as usize * 3);
    let first_tri = flat.corners[0][3].to_bits();
    assert!(first_tri < a.triangle_count());
}

#[test]
#[ignore = "perf"]
fn perf_bvh_1m_faces() {
    let mut m = make_plane(&PlaneOptions { width: 1000.0, depth: 1000.0, subdiv_x: 1000, subdiv_z: 1000, ..Default::default() });
    for p in m.positions_mut() {
        p.y = 10.0 * (p.x * 0.05).sin() * (p.z * 0.05).cos();
    }
    let t0 = Instant::now();
    let bvh = MeshBvh::new(&m);
    let t_build = t0.elapsed().as_secs_f64();
    let rays = 1_000_000;
    let hits = AtomicI32::new(0);
    let t0 = Instant::now();
    (0..rays).into_par_iter().for_each(|i| {
        let x = (i % 1000) as f64 - 499.5;
        let z = (i / 1000) as f64 - 499.5;
        if bvh.raycast(DVec3::new(x, 100.0, z), DVec3::new(0.1, -1.0, 0.05)).is_some() {
            hits.fetch_add(1, Ordering::Relaxed);
        }
    });
    let t_rays = t0.elapsed().as_secs_f64();
    assert!(hits.load(Ordering::Relaxed) > rays * 9 / 10);
    let t0 = Instant::now();
    (0..rays).into_par_iter().for_each(|i| {
        bvh.closest_point(DVec3::new((i % 1000) as f64 - 499.5, 30.0, (i / 1000) as f64 - 499.5));
    });
    let t_closest = t0.elapsed().as_secs_f64();
    println!("bvh 2M tris: build {t_build}s (incl. triangulation), 1M rays {t_rays}s, 1M closest {t_closest}s");
}
