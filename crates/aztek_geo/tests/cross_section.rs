//! Tests: cross section.

mod common;

use aztek_geo::cross_section::*;
use aztek_geo::primitives::*;
use aztek_geo::{Index, Mesh, INVALID};
use common::*;
use glam::{DMat4, DVec2, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn check_near(a: DVec3, b: DVec3, eps: f64) {
    assert!((a - b).length() < eps, "got {a:?} want {b:?}");
}

fn center(m: &Mesh, p: DVec3, dir: DVec3, world: DMat4) -> Option<SectionPick> {
    section_center(&[SectionMesh::new(m, world)], p, dir)
}

fn cyl(radius: f64, height: f64, axis: i32, hs: i32, caps: bool) -> Mesh {
    make_cylinder(&CylinderOptions { radius, height, subdiv_axis: axis, subdiv_height: hs, caps, ..Default::default() })
}

#[test]
fn cylinder_centres_an_off_axis_point() {
    let c = cyl(10.0, 100.0, 16, 4, true);
    let b = c.bounds();
    assert!(approx(b.max.y - b.min.y, 100.0));

    let pick = center(&c, v(3.0, 12.0, -4.0), DVec3::Y, DMat4::IDENTITY).unwrap();
    assert!(pick.inside);
    check_near(pick.center, v(0.0, 12.0, 0.0), 1e-6);

    // Tilted direction: the oblique cut is an ellipse, still centred on the axis.
    let pick = center(&c, v(2.0, 5.0, 1.0), v(0.3, 1.0, 0.0).normalize(), DMat4::IDENTITY).unwrap();
    assert!(pick.inside);
    assert!(DVec2::new(pick.center.x, pick.center.z).length() < 1e-6);

    // Placed in the world: the mesh's transform applies.
    let w = DMat4::from_translation(v(50.0, 0.0, 0.0));
    let pick = center(&c, v(52.0, 0.0, 3.0), DVec3::Y, w).unwrap();
    check_near(pick.center, v(50.0, 0.0, 0.0), 1e-6);
}

#[test]
fn cut_along_the_axis_goes_through_ngon_caps() {
    let c = make_cylinder(&CylinderOptions { radius: 10.0, height: 40.0, subdiv_axis: 12, ..Default::default() });
    // Plane x = 2 cuts the side and both 12-sided caps into a rectangle centred at y = 0, z = 0.
    let pick = center(&c, v(2.0, 7.0, 1.0), DVec3::X, DMat4::IDENTITY).unwrap();
    assert!(pick.inside);
    check_near(pick.center, v(2.0, 0.0, 0.0), 1e-6);
    assert!(pick.area > 40.0 * 15.0);
}

#[test]
fn bent_tube_torus_cut_picks_the_side_holding_the_point() {
    let torus = make_torus(&TorusOptions { radius: 30.0, section_radius: 5.0, subdiv_axis: 32, subdiv_height: 16, ..Default::default() });
    let b = torus.bounds();
    assert!(approx_eps(b.max.y - b.min.y, 10.0, 0.01));
    // The ring lies in XZ; at (30, 0, 0) the tube runs along Z, and the plane z = 0 cuts two loops at x = +-30.
    let pick = center(&torus, v(31.5, 2.0, 0.0), DVec3::Z, DMat4::IDENTITY).unwrap();
    assert!(pick.inside);
    assert!(approx_eps(pick.center.x, 30.0, 0.02));
    assert!(pick.center.y.abs() < 1e-6);

    let pick = center(&torus, v(-28.0, -1.0, 0.0), DVec3::Z, DMat4::IDENTITY).unwrap();
    assert!(approx_eps(pick.center.x, -30.0, 0.02));
    // Outside both tubes: the nearest loop wins.
    let pick = center(&torus, v(-40.0, 0.0, 0.0), DVec3::Z, DMat4::IDENTITY).unwrap();
    assert!(!pick.inside);
    assert!(pick.center.x < -25.0);
}

#[test]
fn two_limbs_across_one_mesh_or_two() {
    let mut left = cyl(6.0, 80.0, 12, 3, true);
    let mut right = left.clone();
    for p in 0..left.point_capacity() {
        left.set_position(p, left.position(p) + v(-15.0, 0.0, 0.0));
        right.set_position(p, right.position(p) + v(15.0, 0.0, 0.0));
    }
    let mut legs = left.clone();
    legs.append(&right);

    let pick = center(&legs, v(13.0, -10.0, 2.0), DVec3::Y, DMat4::IDENTITY).unwrap();
    assert!(pick.inside);
    check_near(pick.center, v(15.0, -10.0, 0.0), 1e-6);
    let pick = center(&legs, v(-17.0, 20.0, -1.0), DVec3::Y, DMat4::IDENTITY).unwrap();
    check_near(pick.center, v(-15.0, 20.0, 0.0), 1e-6);

    let both = [SectionMesh::new(&left, DMat4::IDENTITY), SectionMesh::new(&right, DMat4::IDENTITY)];
    let pick = section_center(&both, v(16.0, 0.0, 0.0), DVec3::Y).unwrap();
    check_near(pick.center, v(15.0, 0.0, 0.0), 1e-6);
}

#[test]
fn open_and_non_manifold_meshes_still_give_closed_loops() {
    // A cylinder with a column of side faces removed: the cut is an open arc that gets closed across the gap.
    let mut c = cyl(10.0, 20.0, 16, 1, false);
    if let Some(f) = (0..c.face_capacity()).find(|&f| c.face_valid(f) && c.face_centroid(f).x > 9.0) {
        c.remove_face(f);
    }
    let loops = cross_sections(&[SectionMesh::new(&c, DMat4::IDENTITY)], DVec3::ZERO, DVec3::Y);
    assert_eq!(loops.len(), 1);
    assert!(loops[0].bridged);
    let pick = center(&c, v(1.0, 0.0, 1.0), DVec3::Y, DMat4::IDENTITY).unwrap();
    assert!(pick.inside);
    assert!(pick.center.length() < 1.0);

    // A fin face sharing an edge with the tube (three faces on one edge) must not break the loop.
    let mut fin = cyl(10.0, 20.0, 8, 1, false);
    let e = (0..fin.edge_capacity())
        .find(|&i| {
            fin.edge_valid(i) && (fin.position(fin.edge_point(i, 0)).y - fin.position(fin.edge_point(i, 1)).y).abs() > 1.0
        })
        .unwrap_or(INVALID);
    assert_ne!(e, INVALID);
    let a: Index = fin.edge_point(e, 0);
    let b: Index = fin.edge_point(e, 1);
    let out = v(fin.position(a).x, 0.0, fin.position(a).z).normalize() * 10.0;
    let c2 = fin.add_point(fin.position(b) + out);
    let d = fin.add_point(fin.position(a) + out);
    assert_ne!(fin.add_face(&[a, b, c2, d]), INVALID);
    let fin_pick = center(&fin, v(0.5, 0.0, -0.5), DVec3::Y, DMat4::IDENTITY).unwrap();
    assert!(fin_pick.inside);
    assert!(fin_pick.center.length() < 2.0);

    // Nothing to cut.
    assert!(center(&c, v(0.0, 500.0, 0.0), DVec3::Y, DMat4::IDENTITY).is_none());
}
