//! Tests: bridge.

mod common;

use std::f64::consts::PI;

use aztek_geo::ops::*;
use aztek_geo::primitives::*;
use aztek_geo::{validate, Index, Mesh, INVALID};
use common::*;
use glam::DVec3;

/// A mesh with some faces deleted, the points that went with them, and the border edges left round the gap.
struct Gap {
    mesh: Mesh,
    removed: Vec<DVec3>,
    border: Vec<Index>,
    points_before: Index,
    faces_before: Index,
}

fn cut(mut m: Mesh, del: impl Fn(DVec3) -> bool, keep_border: Option<&dyn Fn(DVec3) -> bool>) -> Gap {
    let faces_before = m.num_faces();
    let faces: Vec<Index> = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && del(m.face_centroid(f))).collect();
    let before: Vec<DVec3> = (0..m.point_capacity()).map(|p| m.position(p)).collect();
    delete_faces(&mut m, &faces, None);
    let removed = (0..before.len()).filter(|&p| !m.point_valid(p as Index)).map(|p| before[p]).collect();
    let border = (0..m.edge_capacity())
        .filter(|&e| {
            if !m.edge_valid(e) || !m.is_boundary_edge(e) {
                return false;
            }
            let mid = (m.position(m.edge_point(e, 0)) + m.position(m.edge_point(e, 1))) * 0.5;
            keep_border.map_or(true, |k| k(mid))
        })
        .collect();
    Gap { points_before: m.point_capacity(), mesh: m, removed, border, faces_before }
}

/// Farthest any new point lies from the nearest point the cut removed.
fn worst_rebuild(g: &Gap) -> f64 {
    let mut worst = 0.0f64;
    for p in g.points_before..g.mesh.point_capacity() {
        if !g.mesh.point_valid(p) {
            continue;
        }
        let best = g.removed.iter().fold(1e30f64, |b, q| b.min(g.mesh.position(p).distance(*q)));
        worst = worst.max(best);
    }
    worst
}

fn auto_options() -> BridgeOptions {
    BridgeOptions { curve: BridgeCurve::Auto, divisions: -1, ..Default::default() }
}

fn open_cyl(radius: f64, height: f64, axis: i32, hs: i32) -> Mesh {
    make_cylinder(&CylinderOptions {
        radius,
        height,
        subdiv_axis: axis,
        subdiv_height: hs,
        subdiv_caps: 0,
        caps: false,
        ..Default::default()
    })
}

/// Two open tubes on the y axis facing each other across a gap.
fn two_tubes(sides_a: i32, sides_b: i32) -> Mesh {
    let gap = 100.0;
    let mut a = open_cyl(50.0, 100.0, sides_a, 1);
    let mut b = open_cyl(50.0, 100.0, sides_b, 1);
    for p in a.positions_mut() {
        p.y -= 50.0 + gap / 2.0;
    }
    for p in b.positions_mut() {
        p.y += 50.0 + gap / 2.0;
    }
    a.append(&b);
    a
}

fn inner_rims(m: &Mesh) -> Vec<Index> {
    (0..m.edge_capacity())
        .filter(|&e| m.edge_valid(e) && m.is_boundary_edge(e) && m.position(m.edge_point(e, 0)).y.abs() < 60.0)
        .collect()
}

/// Points of new faces at height y, as an average distance from the y axis.
fn radius_at(m: &Mesh, first_new_point: Index, y: f64) -> f64 {
    let mut sum = 0.0;
    let mut n = 0;
    for p in first_new_point..m.point_capacity() {
        if m.point_valid(p) && (m.position(p).y - y).abs() < 1e-6 {
            sum += m.position(p).x.hypot(m.position(p).z);
            n += 1;
        }
    }
    if n > 0 {
        sum / n as f64
    } else {
        -1.0
    }
}

#[test]
fn auto_rebuilds_deleted_strip_of_cylinder() {
    let mut g = cut(open_cyl(50.0, 400.0, 16, 8), |c| c.y.abs() < 50.0, Some(&|mid: DVec3| mid.y.abs() < 60.0));
    assert_eq!(g.border.len(), 32);
    let border = g.border.clone();
    let r = bridge(&mut g.mesh, &border, &auto_options());
    assert_eq!(r.rows, 2);
    assert_eq!(r.faces.len(), 32);
    assert!(valid(&g.mesh));
    assert_eq!(boundary_edge_count(&g.mesh), 32);
    assert!(worst_rebuild(&g) < 1e-6);
}

#[test]
fn auto_rebuilds_deleted_band_of_sphere() {
    let mut g = cut(
        make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 16, subdiv_height: 16, ..Default::default() }),
        |c| c.y.abs() < 12.0,
        None,
    );
    assert_eq!(g.border.len(), 32);
    let border = g.border.clone();
    let r = bridge(&mut g.mesh, &border, &auto_options());
    assert_eq!(r.rows, 2);
    assert!(valid(&g.mesh));
    assert_eq!(boundary_edge_count(&g.mesh), 0);
    assert!(worst_rebuild(&g) < 1e-3);
}

#[test]
fn auto_rebuilds_deleted_strips_of_torus_both_ways() {
    let torus = || make_torus(&TorusOptions { radius: 50.0, section_radius: 20.0, subdiv_axis: 24, subdiv_height: 12, ..Default::default() });
    // Across the tube: a wedge of the ring.
    let mut ring = cut(torus(), |c| c.z.atan2(c.x).abs() < 20f64.to_radians(), None);
    assert_eq!(ring.border.len(), 24);
    let border = ring.border.clone();
    let r = bridge(&mut ring.mesh, &border, &auto_options());
    assert_eq!(r.rows, 2);
    assert!(valid(&ring.mesh));
    assert_eq!(boundary_edge_count(&ring.mesh), 0);
    assert!(worst_rebuild(&ring) < 1e-3);

    // Along the tube: a band round the top of it.
    let mut band = cut(torus(), |c| c.y > 15.0, None);
    assert_eq!(band.border.len(), 48);
    let border = band.border.clone();
    let r = bridge(&mut band.mesh, &border, &auto_options());
    assert_eq!(r.pairs_bridged, 1);
    assert!(valid(&band.mesh));
    assert_eq!(boundary_edge_count(&band.mesh), 0);
    assert!(worst_rebuild(&band) < 1e-3);
}

#[test]
fn open_ends_weld_to_the_border_between_the_sides() {
    // A 4 x 2 hole in a sheet; bridging only its long sides lands the rows on the short sides' middle points.
    let hole = || {
        cut(
            make_plane(&PlaneOptions { width: 600.0, depth: 500.0, subdiv_x: 6, subdiv_z: 5, ..Default::default() }),
            |c| c.x.abs() < 200.0 && c.z > -50.0 && c.z < 150.0,
            Some(&|mid: DVec3| mid.x.abs() < 199.0 && ((mid.z + 50.0).abs() < 1.0 || (mid.z - 150.0).abs() < 1.0)),
        )
    };
    let mut g = hole();
    assert_eq!(g.border.len(), 8);
    let border = g.border.clone();
    let r = bridge(&mut g.mesh, &border, &auto_options());
    assert_eq!(r.rows, 2);
    assert_eq!(r.welded, 2);
    assert_eq!(r.split, 0);
    assert_eq!(r.unwelded, 0);
    assert!(valid(&g.mesh));
    assert_eq!(g.mesh.num_faces(), g.faces_before);
    assert_eq!(boundary_edge_count(&g.mesh), 22);
    assert!(worst_rebuild(&g) < 1e-6);

    // Three rows against one border point each end: two rows split the end borders, the point stays as a corner.
    let mut h = hole();
    let border = h.border.clone();
    let r = bridge(&mut h.mesh, &border, &BridgeOptions { divisions: 2, ..auto_options() });
    assert_eq!(r.rows, 3);
    assert_eq!(r.split + r.welded, 4);
    assert_eq!(r.unwelded + r.welded, 2);
    assert!(valid(&h.mesh));
    assert_eq!(boundary_edge_count(&h.mesh), 22);
}

#[test]
fn sliver_between_two_curves_meeting_at_both_ends_leaves_no_loose_points() {
    // An upper arc of 7 points and a lower arc of 5, sharing their end points, with a row of quads beyond each.
    let mut pts: Vec<DVec3> = Vec::new();
    let mut sizes = Vec::new();
    let mut idx: Vec<Index> = Vec::new();
    let upper = |t: f64| DVec3::new(-300.0 + 600.0 * t, 0.0, 40.0 * (PI * t).sin());
    let lower = |t: f64| DVec3::new(-300.0 + 600.0 * t, 0.0, -30.0 * (PI * t).sin());
    let mut u = Vec::new();
    let mut l = Vec::new();
    let mut uo = Vec::new();
    let mut lo = Vec::new();
    for i in 0..=6 {
        u.push(pts.len() as Index);
        pts.push(upper(i as f64 / 6.0));
    }
    l.push(u[0]);
    for i in 1..4 {
        l.push(pts.len() as Index);
        pts.push(lower(i as f64 / 4.0));
    }
    l.push(*u.last().unwrap());
    for &p in &u {
        uo.push(pts.len() as Index);
        pts.push(pts[p as usize] + DVec3::new(0.0, 0.0, 100.0));
    }
    for &p in &l {
        lo.push(pts.len() as Index);
        pts.push(pts[p as usize] - DVec3::new(0.0, 0.0, 100.0));
    }
    for i in 0..u.len() - 1 {
        sizes.push(4);
        idx.extend_from_slice(&[u[i], uo[i], uo[i + 1], u[i + 1]]);
    }
    for i in 0..l.len() - 1 {
        sizes.push(4);
        idx.extend_from_slice(&[l[i], l[i + 1], lo[i + 1], lo[i]]);
    }
    let m = Mesh::from_polygons(&pts, &sizes, &idx);
    assert!(valid(&m));
    let sliver: Vec<Index> = (0..m.edge_capacity())
        .filter(|&e| {
            m.edge_valid(e)
                && m.is_boundary_edge(e)
                && m.position(m.edge_point(e, 0)).z.abs() < 50.0
                && m.position(m.edge_point(e, 1)).z.abs() < 50.0
        })
        .collect();
    assert_eq!(sliver.len(), 10);

    for curve in [BridgeCurve::Linear, BridgeCurve::Blend, BridgeCurve::Auto] {
        let mut b = m.clone();
        let r = bridge(&mut b, &sliver, &BridgeOptions { curve, divisions: 3, ..Default::default() });
        assert_eq!(r.pairs_bridged, 1, "{curve:?}");
        assert!(is_valid(&b, validate::DEFAULT | validate::LOOSE), "{curve:?}");
        // Only the outer rim stays open: no hole at the tips, no point left hanging there.
        assert_eq!(boundary_edge_count(&b), 6 + 4 + 4, "{curve:?}");
        for p in m.point_capacity()..b.point_capacity() {
            if b.point_valid(p) {
                assert!(b.position(p).distance(b.position(u[0])) > 1e-6);
                assert!(b.position(p).distance(b.position(*u.last().unwrap())) > 1e-6);
            }
        }
    }
}

#[test]
fn loops_of_different_lengths_join_with_quads() {
    for (a, b, tris) in [(12, 16, 0), (12, 18, 0), (12, 13, 1), (8, 16, 0)] {
        let mut m = two_tubes(a, b);
        let rims = inner_rims(&m);
        let r = bridge(&mut m, &rims, &BridgeOptions { divisions: 3, ..Default::default() });
        assert_eq!(r.pairs_bridged, 1, "{a} {b}");
        assert!(valid(&m));
        assert_eq!(boundary_edge_count(&m), (a + b) as Index);
        assert_eq!(r.triangles, tris, "{a} {b}");
        let other = r.faces.iter().filter(|&&f| m.face_size(f) != 4).count() as i32;
        assert_eq!(other, tris);
    }
    // No rows to spread over: triangles make up the difference.
    let mut m = two_tubes(12, 16);
    let rims = inner_rims(&m);
    let r = bridge(&mut m, &rims, &BridgeOptions { transition: BridgeTransition::Triangles, ..Default::default() });
    assert!(valid(&m));
    assert_eq!(r.triangles, 4);
}

#[test]
fn shape_options() {
    let base = two_tubes(16, 16).point_capacity();
    let run = |o: &BridgeOptions| {
        let mut m = two_tubes(16, 16);
        let rims = inner_rims(&m);
        bridge(&mut m, &rims, o);
        assert!(valid(&m));
        m
    };
    let o = BridgeOptions { divisions: 1, ..Default::default() };
    assert!(approx_eps(radius_at(&run(&o), base, 0.0), 50.0, 1e-6));
    assert!(approx_eps(radius_at(&run(&BridgeOptions { taper: 0.5, ..o.clone() }), base, 0.0), 25.0, 1e-6));
    assert!(approx_eps(radius_at(&run(&BridgeOptions { profile_factor: 0.5, ..o.clone() }), base, 0.0), 75.0, 1e-6));

    // Twist turns the middle row half way: 90 degrees at the target is 45 at the middle.
    let straight = run(&o);
    let turned = run(&BridgeOptions { twist: 90.0, ..o.clone() });
    let a = straight.position(base);
    let b = turned.position(base);
    let angle = DVec3::new(a.x, 0.0, a.z).normalize().dot(DVec3::new(b.x, 0.0, b.z).normalize()).acos().to_degrees();
    assert!(approx_eps(angle, 45.0, 1e-6));

    // Offset steps the pairing one point round: every column leans by one side's step.
    let shifted = run(&BridgeOptions { offset: 1, ..o.clone() });
    let even = run(&o);
    let lean = |m: &Mesh| (0..m.face_capacity()).filter(|&f| m.face_valid(f)).fold(0.0f64, |a, f| a.max(m.face_normal(f).y.abs()));
    assert!(lean(&shifted) > lean(&even) + 0.01);

    // Blend on coaxial tubes of one radius stays on the cylinder.
    assert!(approx_eps(radius_at(&run(&BridgeOptions { curve: BridgeCurve::Blend, ..o.clone() }), base, 0.0), 50.0, 1e-6));

    // Path: the middle row follows an arc's middle, bowed out along x.
    let mut path = BridgeOptions { curve: BridgeCurve::Path, ..o.clone() };
    for i in 0..=16 {
        let t = i as f64 / 16.0;
        path.path.push(DVec3::new(200.0 * (PI * t).sin(), -50.0 + 100.0 * t, 0.0));
    }
    let bowed = run(&path);
    let new_points: Vec<DVec3> = (base..bowed.point_capacity()).filter(|&p| bowed.point_valid(p)).map(|p| bowed.position(p)).collect();
    let centre = new_points.iter().fold(DVec3::ZERO, |a, &p| a + p) / new_points.len() as f64;
    assert!(centre.x > 50.0);
}

#[test]
fn merge_smoothing_angle_faces() {
    let mut m = two_tubes(12, 12);
    let before = m.num_points();
    let rims = inner_rims(&m);
    bridge(&mut m, &rims, &BridgeOptions { merge: true, merge_factor: 0.5, ..Default::default() });
    assert!(valid(&m));
    assert_eq!(m.num_points(), before - 12);
    assert_eq!(boundary_edge_count(&m), 24);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) && m.position(p).y.abs() < 60.0 {
            assert!(m.position(p).y.abs() < 1e-9);
        }
    }

    // Hard where the strip folds sharper than the angle: a flat gap between two tubes has none.
    let mut h = two_tubes(16, 16);
    let rims = inner_rims(&h);
    bridge(&mut h, &rims, &BridgeOptions { smoothing_angle: 30.0, ..Default::default() });
    assert_eq!(hard_count(&h), 0);
    let mut h2 = two_tubes(16, 16);
    let rims = inner_rims(&h2);
    bridge(&mut h2, &rims, &BridgeOptions { smoothing_angle: 0.0, taper: 0.3, divisions: 1, ..Default::default() });
    assert!(hard_count(&h2) > 0);

    // Two faces through a block: deleted and joined by a tunnel.
    let mut cube = make_cube(&CubeOptions { width: 300.0, height: 300.0, depth: 300.0, subdiv_x: 3, subdiv_y: 3, subdiv_z: 3, ..Default::default() });
    let ends: Vec<Index> = (0..cube.face_capacity())
        .filter(|&f| {
            let c = cube.face_centroid(f);
            cube.face_valid(f) && c.x.abs() < 1.0 && c.z.abs() < 1.0 && (c.y.abs() - 150.0).abs() < 1.0
        })
        .collect();
    assert_eq!(ends.len(), 2);
    let faces_before = cube.num_faces();
    let r = bridge_faces(&mut cube, &ends, &BridgeOptions::default());
    assert_eq!(r.faces.len(), 4);
    assert!(valid(&cube));
    assert_eq!(boundary_edge_count(&cube), 0);
    assert_eq!(cube.num_faces(), faces_before + 2);
    assert!(approx(signed_volume(&cube), 300.0 * 300.0 * 300.0 - 100.0 * 100.0 * 300.0));
}

#[test]
fn chain_runs_one_bridge_through_several_wire_rings() {
    let mut m = Mesh::new();
    for ring in 0..3 {
        let ids: Vec<Index> = (0..8)
            .map(|i| {
                let a = 2.0 * PI * i as f64 / 8.0;
                m.add_point(DVec3::new(50.0 * a.cos(), 100.0 * ring as f64, 50.0 * a.sin()))
            })
            .collect();
        for i in 0..8 {
            m.add_edge(ids[i], ids[(i + 1) % 8]);
        }
    }
    let all: Vec<Index> = (0..m.edge_capacity()).collect();
    let r = bridge(&mut m, &all, &BridgeOptions { connect: BridgeConnect::Chain, ..Default::default() });
    assert_eq!(r.pairs_bridged, 2);
    assert_eq!(r.faces.len(), 16);
    assert!(valid(&m));
    assert_eq!(boundary_edge_count(&m), 16);
}

#[test]
fn explicit_start_points_pair_as_asked() {
    let mut m = two_tubes(12, 12);
    let rims = inner_rims(&m);
    // The lowest-id point on each inner rim.
    let (mut a, mut b) = (INVALID, INVALID);
    for &e in &rims {
        for i in 0..2 {
            let p = m.edge_point(e, i);
            let slot = if m.position(p).y < 0.0 { &mut a } else { &mut b };
            *slot = if *slot == INVALID { p } else { (*slot).min(p) };
        }
    }
    // Point b's neighbour three steps round, so the pairing has to turn.
    let mut target = b;
    for _ in 0..3 {
        let mut next = INVALID;
        m.for_each_point_edge(target, |e| {
            let o = m.edge_other_point(e, target);
            if next == INVALID && m.is_boundary_edge(e) && o > target && (m.position(o).y - m.position(b).y).abs() < 1e-6 {
                next = o;
            }
        });
        if next != INVALID {
            target = next;
        }
    }
    assert_ne!(target, b);
    bridge(&mut m, &rims, &BridgeOptions { pairs: vec![(a, target)], ..Default::default() });
    assert!(valid(&m));
    assert_ne!(m.find_edge(a, target), INVALID);
}
