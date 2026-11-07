//! Tests: array.

use std::collections::BTreeMap;

use aztek_geo::array::*;
use aztek_geo::{Index, Mesh};
use glam::{DMat4, DVec3, IVec3};

fn near(a: DVec3, b: DVec3) -> bool {
    a.distance(b) < 1e-6
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < f32::EPSILON as f64 * 100.0 * (1.0 + a.abs().max(b.abs()))
}

fn closed_manifold(m: &Mesh) -> bool {
    (0..m.edge_capacity()).all(|e| !m.edge_valid(e) || m.edge_face_count(e) == 2)
}

fn consistent_winding(m: &Mesh) -> bool {
    let mut directed: BTreeMap<(Index, Index), i32> = BTreeMap::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let p = m.face_points(f);
        for i in 0..p.len() {
            let c = directed.entry((p[i], p[(i + 1) % p.len()])).or_insert(0);
            *c += 1;
            if *c > 1 {
                return false;
            }
        }
    }
    true
}

/// A closed wedge of a cylinder, radius r, height h, spanning `degrees` about +y from the +x axis (turning toward -z,
/// as a positive rotation about y does). Its two cut walls lie in the seam planes.
fn wedge(degrees: f64) -> Mesh {
    let (r, h) = (50.0, 20.0);
    let a = degrees.to_radians();
    let mut m = Mesh::new();
    let b0 = m.add_point(DVec3::new(0.0, 0.0, 0.0));
    let t0 = m.add_point(DVec3::new(0.0, h, 0.0));
    let b1 = m.add_point(DVec3::new(r, 0.0, 0.0));
    let t1 = m.add_point(DVec3::new(r, h, 0.0));
    let b2 = m.add_point(DVec3::new(r * a.cos(), 0.0, -r * a.sin()));
    let t2 = m.add_point(DVec3::new(r * a.cos(), h, -r * a.sin()));
    m.add_face(&[b0, b2, b1]);
    m.add_face(&[t0, t1, t2]);
    m.add_face(&[b1, b2, t2, t1]);
    m.add_face(&[b0, b1, t1, t0]);
    m.add_face(&[b0, t0, t2, b2]);
    m
}

fn translation(m: DMat4) -> DVec3 {
    m.w_axis.truncate()
}

#[test]
fn layout_maths_linear_radial_helical_grid_and_centre_frame() {
    let mut l = ArrayLayout { offset: DVec3::new(10.0, 0.0, 0.0), count: 4, ..Default::default() };
    assert_eq!(array_copy_count(&l), 4);
    assert!(near(translation(array_copy_transform(&l, 0)), DVec3::ZERO));
    assert!(near(translation(array_copy_transform(&l, 3)), DVec3::new(30.0, 0.0, 0.0)));

    // A rotation per copy curves the row: the second step heads off along the turned direction.
    l.rotate = DVec3::new(0.0, 90.0, 0.0);
    assert!(near(translation(array_copy_transform(&l, 1)), DVec3::new(10.0, 0.0, 0.0)));
    assert!(near(translation(array_copy_transform(&l, 2)), DVec3::new(10.0, 0.0, -10.0)));
    l.rotate = DVec3::ZERO;

    // Scale per copy compounds.
    l.scale = DVec3::splat(0.5);
    assert!(approx(array_copy_transform(&l, 2).x_axis.truncate().length(), 0.25));

    let mut r = ArrayLayout { mode: ArrayMode::Radial, count: 6, offset: DVec3::ZERO, ..Default::default() };
    assert!(approx(array_step_angle(&r), 60.0));
    assert!(array_closes_ring(&r));
    let p = DVec3::new(10.0, 0.0, 0.0);
    assert!(near(array_copy_transform(&r, 6).transform_point3(p), p));
    // A partial arc puts copies at both ends.
    r.angle = 90.0;
    r.count = 4;
    assert!(approx(array_step_angle(&r), 30.0));
    assert!(!array_closes_ring(&r));
    assert!(near(array_copy_transform(&r, 3).transform_point3(p), DVec3::new(0.0, 0.0, -10.0)));
    // Step mode: count copies the given angle apart; 8 x 45 closes the ring.
    r.step_angle = true;
    r.angle = 45.0;
    r.count = 8;
    assert!(approx(array_step_angle(&r), 45.0));
    assert!(array_closes_ring(&r));
    // A rise climbs the axis each copy and keeps the ring open.
    r.rise = 5.0;
    assert!(!array_closes_ring(&r));
    assert!(near(array_copy_transform(&r, 8).transform_point3(p), DVec3::new(10.0, 40.0, 0.0)));
    r.rise = 0.0;

    // Around another axis, about a moved centre.
    r.axis = 2;
    r.step_angle = false;
    r.angle = 360.0;
    r.count = 4;
    r.frame = DMat4::from_translation(DVec3::new(5.0, 0.0, 0.0));
    assert!(near(array_copy_transform(&r, 2).transform_point3(p), DVec3::ZERO));

    let gr = ArrayLayout {
        mode: ArrayMode::Grid,
        grid_count: IVec3::new(3, 2, 2),
        offset: DVec3::new(10.0, 20.0, 30.0),
        ..Default::default()
    };
    assert_eq!(array_copy_count(&gr), 12);
    assert_eq!(array_grid_cell(&gr, 7), IVec3::new(1, 0, 1));
    assert!(near(translation(array_copy_transform(&gr, 7)), DVec3::new(10.0, 0.0, 30.0)));
}

#[test]
fn radial_array_welds_closed_wedge_into_one_closed_manifold_ring() {
    for n in [3, 6, 8] {
        let mut o = ArrayOptions::default();
        o.layout.mode = ArrayMode::Radial;
        o.layout.count = n;
        o.layout.angle = 360.0;
        let piece = wedge(360.0 / n as f64);

        let loose = array_mesh(&piece, &o);
        assert_eq!(loose.num_faces(), 5 * n as Index);
        assert_eq!(loose.num_points(), 6 * n as Index);

        o.weld = true;
        let ring = array_mesh(&piece, &o);
        // The cut walls go; the centre line welds to one point top and bottom.
        assert_eq!(ring.num_faces(), 3 * n as Index, "n={n}");
        assert_eq!(ring.num_points(), 2 + 2 * n as Index);
        assert!(closed_manifold(&ring));
        assert!(consistent_winding(&ring));
        assert_eq!(ring.num_points() as i64 - ring.num_edges() as i64 + ring.num_faces() as i64, 2);
    }

    // A partial arc leaves its ends open unless merge_ends asks; an open arc of a closed wedge keeps its end walls.
    let mut o = ArrayOptions::default();
    o.layout.mode = ArrayMode::Radial;
    o.layout.count = 4;
    o.layout.step_angle = true;
    o.layout.angle = 45.0;
    o.weld = true;
    let arc = array_mesh(&wedge(45.0), &o);
    assert_eq!(arc.num_faces(), 4 * 3 + 2);
    assert!(closed_manifold(&arc));
}

#[test]
fn linear_array_welds_open_strips_at_seams_within_tolerance_only() {
    let mut strip = Mesh::new();
    let a = strip.add_point(DVec3::new(0.0, 0.0, 0.0));
    let b = strip.add_point(DVec3::new(10.0, 0.0, 0.0));
    let c = strip.add_point(DVec3::new(10.0, 0.0, 10.0));
    let d = strip.add_point(DVec3::new(0.0, 0.0, 10.0));
    strip.add_face(&[a, d, c, b]);
    let mut o = ArrayOptions { weld: true, tolerance: 0.01, ..Default::default() };
    o.layout.count = 5;
    o.layout.offset = DVec3::new(10.004, 0.0, 0.0);
    let joined = array_mesh(&strip, &o);
    assert_eq!(joined.num_faces(), 5);
    assert_eq!(joined.num_points(), 12);
    o.tolerance = 0.001;
    assert_eq!(array_mesh(&strip, &o).num_points(), 20);

    // Ends meet only with merge_ends: four strips turned about their shared edges make a square tube.
    let mut ring = ArrayOptions { weld: true, ..Default::default() };
    ring.layout.count = 4;
    ring.layout.offset = DVec3::new(10.0, 0.0, 0.0);
    ring.layout.rotate = DVec3::new(0.0, 0.0, 90.0);
    assert_eq!(array_mesh(&strip, &ring).num_points(), 4 * 4 - 3 * 2);
    ring.merge_ends = true;
    assert_eq!(array_mesh(&strip, &ring).num_points(), 4 * 4 - 4 * 2);
}
