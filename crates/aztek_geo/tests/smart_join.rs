//! Tests: smart join.

use std::f64::consts::PI;

use aztek_geo::primitives::*;
use aztek_geo::smart_join::*;
use aztek_geo::{validate, Index, Mesh};
use glam::DVec3;

/// Lays a Y-up cylinder along +X starting at x0 (its bottom cap ends up at x0).
fn arm(segments: i32, radius: f64, length: f64, x0: f64, caps: bool) -> Mesh {
    let mut m = make_cylinder(&CylinderOptions {
        radius,
        height: length,
        subdiv_axis: segments,
        caps,
        base_at_origin: true,
        cap_fill: CapFill::Ngon,
        ..Default::default()
    });
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            let q = m.position(p);
            m.set_position(p, DVec3::new(q.y + x0, -q.x, q.z));
        }
    }
    m
}

fn boundary_edges(m: &Mesh) -> usize {
    (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.is_boundary_edge(e)).count()
}

fn faces_of_size(m: &Mesh, pred: impl Fn(Index) -> bool) -> usize {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f) && pred(m.face_size(f))).count()
}

fn body8() -> Mesh {
    make_cylinder(&CylinderOptions { radius: 50.0, height: 200.0, subdiv_axis: 8, cap_fill: CapFill::Ngon, ..Default::default() })
}

fn clean(m: &Mesh) -> bool {
    m.validate(validate::DEFAULT).is_empty()
}

#[test]
fn perpendicular_8_segment_cylinders() {
    let body = body8();
    let limb = arm(8, 20.0, 100.0, 30.0, true); // pokes about 16 cm into the body
    let r = smart_join(&limb, &body, &SmartJoinOptions::default());
    assert!(r.ok, "{}", r.summary);
    assert!(r.confidence >= 90, "{}", r.summary);
    assert_eq!(r.limb_loop_size, 8);
    assert_eq!(r.hole_size, 8);
    assert!(r.trimmed);
    assert_eq!(r.triangles, 0);
    assert_eq!(r.ngons, 0);
    assert!(clean(&r.mesh));
    assert_eq!(boundary_edges(&r.mesh), 0);
    // Only the three caps are not quads.
    assert_eq!(faces_of_size(&r.mesh, |s| s != 4), 3);
    assert!(r.preserved_points as f64 / r.original_points as f64 >= 0.7);
    // Nothing of the limb is left inside the body.
    for p in 0..r.mesh.point_capacity() {
        if r.mesh.point_valid(p) {
            let q = r.mesh.position(p);
            assert!(q.x.hypot(q.z) > 50.0 * (PI / 8.0).cos() - 1e-3);
        }
    }
}

#[test]
fn open_ended_limb_near_the_body_uses_its_border() {
    let body = body8();
    let limb = arm(8, 20.0, 100.0, 60.0, false);
    let r = smart_join(&limb, &body, &SmartJoinOptions { segments: 3, ..Default::default() });
    assert!(r.ok, "{}", r.summary);
    assert!(!r.trimmed);
    assert_eq!(r.segments, 3);
    assert!(r.confidence >= 85, "{}", r.summary);
    assert_eq!(r.triangles, 0);
    assert!(clean(&r.mesh));
    // The far end stays open; everything else is closed.
    assert_eq!(boundary_edges(&r.mesh), 8);
}

#[test]
fn mismatched_and_grazing_joins_rate_lower() {
    let body = body8();
    let good = smart_join(&arm(8, 20.0, 100.0, 30.0, true), &body, &SmartJoinOptions::default());
    let odd = smart_join(&arm(7, 20.0, 100.0, 30.0, true), &body, &SmartJoinOptions::default());
    assert!(odd.ok, "{}", odd.summary);
    assert!(odd.confidence < good.confidence);
    assert!(odd.triangles > 0);
    assert!(clean(&odd.mesh));

    // A limb tilted 60 degrees off the body normal.
    let mut tilted = arm(8, 20.0, 100.0, 0.0, false);
    let (s, c) = 60f64.to_radians().sin_cos();
    for p in 0..tilted.point_capacity() {
        if tilted.point_valid(p) {
            let q = tilted.position(p);
            tilted.set_position(p, DVec3::new(q.x * c - q.y * s, q.x * s + q.y * c, q.z) + DVec3::new(70.0, 0.0, 0.0));
        }
    }
    let graze = smart_join(&tilted, &body, &SmartJoinOptions::default());
    println!("{}", graze.summary);
    assert!(graze.ok);
    assert!(graze.confidence < good.confidence);
    assert!(clean(&graze.mesh));
}

#[test]
fn parameters() {
    let body = body8();
    let limb = arm(8, 20.0, 100.0, 30.0, true);
    for twist in [0, 1, 3] {
        for scale in [0.8, 1.0, 1.2] {
            let r = smart_join(
                &limb,
                &body,
                &SmartJoinOptions { segments: 4, tension: 1.5, twist, hole_scale: scale, relax: 2, ..Default::default() },
            );
            assert!(r.ok, "{}", r.summary);
            assert!(clean(&r.mesh), "{}", r.summary);
            assert_eq!(r.triangles, 0);
        }
    }
    let kept = smart_join(&limb, &body, &SmartJoinOptions { delete_buried: false, ..Default::default() });
    assert!(kept.ok);
    assert!(clean(&kept.mesh), "{:?}", kept.mesh.validate(validate::DEFAULT));
    // The detached buried part keeps its own open border.
    assert_eq!(boundary_edges(&kept.mesh), 8);
    let moved = smart_join(&limb, &body, &SmartJoinOptions { offset_v: 10.0, ..Default::default() });
    assert!(moved.ok);
    assert!(clean(&moved.mesh));
}
