//! Tests: bounding.

mod common;

use aztek_geo::bounding::*;
use aztek_geo::primitives::*;
use aztek_geo::Mesh;
use common::*;
use glam::{DMat4, DVec3};

fn points_of(m: &Mesh, xf: DMat4) -> Vec<DVec3> {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| xf.transform_point3(m.position(p))).collect()
}

fn worst_outside_box(pts: &[DVec3], b: &OrientedBox) -> f64 {
    let mut worst = -1e300f64;
    for &p in pts {
        for i in 0..3 {
            worst = worst.max((p - b.center).dot(b.axes.col(i)).abs() - b.size[i] * 0.5);
        }
    }
    worst
}

fn tilted() -> DMat4 {
    DMat4::from_translation(DVec3::new(30.0, -12.0, 5.0))
        * DMat4::from_axis_angle(DVec3::new(1.0, 2.0, 0.5).normalize(), 0.7)
        * DMat4::from_axis_angle(DVec3::Z, 0.3)
}

fn cube(w: f64, h: f64, d: f64, sx: i32, sy: i32, sz: i32) -> Mesh {
    make_cube(&CubeOptions { width: w, height: h, depth: d, subdiv_x: sx, subdiv_y: sy, subdiv_z: sz, ..Default::default() })
}

#[test]
fn rotated_slab_recovers_its_exact_size() {
    let pts = points_of(&cube(240.0, 20.0, 90.0, 4, 2, 3), tilted());
    let b = minimum_volume_box(&pts).unwrap();
    let mut sorted = [b.size.x, b.size.y, b.size.z];
    sorted.sort_by(f64::total_cmp);
    assert!(approx_eps(sorted[0], 20.0, 1e-6));
    assert!(approx_eps(sorted[1], 90.0, 1e-6));
    assert!(approx_eps(sorted[2], 240.0, 1e-6));
    assert!(worst_outside_box(&pts, &b) < 1e-6);
    assert!(approx(b.axes.determinant(), 1.0));
}

#[test]
fn near_aligned_cloud_keeps_near_identity_axes() {
    let pts = points_of(&cube(100.0, 50.0, 30.0, 1, 1, 1), DMat4::from_axis_angle(DVec3::Y, 0.05));
    let b = minimum_volume_box(&pts).unwrap();
    for i in 0..3 {
        assert!(b.axes.col(i)[i] > 0.99);
    }
    assert!(approx_eps(b.size.x, 100.0, 1e-6));
    assert!(approx_eps(b.size.y, 50.0, 1e-6));
}

#[test]
fn sphere_cylinder_and_capsule_contain_every_point() {
    let sphere = points_of(
        &make_ico_sphere(&IcoSphereOptions { radius: 40.0, subdivisions: 3, ..Default::default() }),
        DMat4::from_translation(DVec3::new(5.0, 6.0, 7.0)),
    );
    let s = minimum_sphere(&sphere);
    assert!(approx_eps(s.radius, 40.0, 1e-6));
    assert!((s.center - DVec3::new(5.0, 6.0, 7.0)).length() < 1e-6);

    let xf = tilted();
    let rod = points_of(
        &make_cylinder(&CylinderOptions { radius: 10.0, height: 200.0, subdiv_axis: 24, ..Default::default() }),
        xf,
    );
    let frame = minimum_volume_box(&rod).unwrap();
    let c = fit_cylinder(&rod, &frame.axes, -1);
    assert!(approx_eps(c.radius, 10.0, 1e-3));
    assert!(approx_eps(c.height, 200.0, 1e-6));
    assert!(approx_eps(c.axes.y_axis.dot(xf.y_axis.truncate()).abs(), 1.0, 1e-6));

    let pill = points_of(
        &make_capsule(&CapsuleOptions { radius: 20.0, height: 160.0, subdiv_axis: 24, subdiv_height: 1, subdiv_caps: 8, ..Default::default() }),
        xf,
    );
    let p = fit_capsule(&pill, &minimum_volume_box(&pill).unwrap().axes, -1);
    assert!(approx_eps(p.radius, 20.0, 1e-3));
    assert!(approx_eps(p.height, 160.0, 1e-3));
    let spine = 0.5 * (p.height - 2.0 * p.radius);
    for &q in &pill {
        let t = (q - p.center).dot(p.axes.y_axis).clamp(-spine, spine);
        assert!((q - (p.center + p.axes.y_axis * t)).length() <= p.radius + 1e-6);
    }
}

#[test]
fn hull_mesh_is_closed_outward_and_capped() {
    let pts = points_of(
        &make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 32, subdiv_height: 16, ..Default::default() }),
        DMat4::IDENTITY,
    );
    let hull = convex_hull_mesh(&pts, 32, 0.0);
    assert!(valid(&hull));
    assert_eq!(boundary_edge_count(&hull), 0);
    assert!(signed_volume(&hull) > 0.0);
    assert!(hull.num_points() <= 32);

    let b = convex_hull_mesh(&points_of(&cube(10.0, 10.0, 10.0, 3, 3, 3), DMat4::IDENTITY), 32, 1.0);
    assert_eq!(b.num_points(), 8);
    assert!(approx(b.bounds().size().x, 12.0));
}

#[test]
fn primitives_capsule() {
    let m = make_capsule(&CapsuleOptions { radius: 25.0, height: 150.0, subdiv_axis: 16, subdiv_height: 2, subdiv_caps: 5, ..Default::default() });
    assert!(valid(&m));
    assert_eq!(boundary_edge_count(&m), 0);
    assert_eq!(euler(&m), 2);
    assert!(signed_volume(&m) > 0.0);
    assert!(approx(m.bounds().size().y, 150.0));
    assert!(approx(m.bounds().size().x, 50.0));
    let ball = make_capsule(&CapsuleOptions { radius: 25.0, height: 10.0, ..Default::default() });
    assert_eq!(boundary_edge_count(&ball), 0);
    assert!(approx(ball.bounds().size().y, 50.0));
}
