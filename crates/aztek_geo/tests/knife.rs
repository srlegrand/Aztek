//! Tests: knife.

mod common;

use aztek_geo::ops::*;
use aztek_geo::primitives::*;
use aztek_geo::symmetry::{axis_plane, SymmetryMap};
use aztek_geo::{Index, Mesh, INVALID};
use common::*;
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The edge whose midpoint is nearest p.
fn edge_at(m: &Mesh, p: DVec3) -> Index {
    let mut best = INVALID;
    let mut best_d = 1e30;
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) {
            let d = ((m.position(m.edge_point(e, 0)) + m.position(m.edge_point(e, 1))) * 0.5).distance(p);
            if d < best_d {
                best_d = d;
                best = e;
            }
        }
    }
    best
}

fn face_at(m: &Mesh, p: DVec3) -> Index {
    let mut best = INVALID;
    let mut best_d = 1e30;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) && m.face_centroid(f).distance(p) < best_d {
            best_d = m.face_centroid(f).distance(p);
            best = f;
        }
    }
    best
}

fn top_view<'a>() -> KnifeOptions<'a> {
    KnifeOptions { view_dir: v(0.0, -1.0, 0.0), ..Default::default() }
}

fn has_point_at(m: &Mesh, p: DVec3) -> bool {
    (0..m.point_capacity()).any(|i| m.point_valid(i) && m.position(i).distance(p) < 1e-6)
}

fn plane(w: f64, d: f64, sx: i32, sz: i32) -> Mesh {
    make_plane(&PlaneOptions { width: w, depth: d, subdiv_x: sx, subdiv_z: sz, ..Default::default() })
}

#[test]
fn one_face_edge_to_edge() {
    let mut m = plane(100.0, 100.0, 1, 1);
    let pts = [KnifePoint::edge(edge_at(&m, v(-50.0, 0.0, 0.0)), 0.5), KnifePoint::edge(edge_at(&m, v(50.0, 0.0, 0.0)), 0.5)];
    let edges = knife_cut(&mut m, &pts, &top_view());
    assert_eq!(edges.len(), 1);
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 2);
    assert_eq!(m.num_points(), 6);
    assert_eq!(boundary_edge_count(&m), 6);
    assert!(has_point_at(&m, v(-50.0, 0.0, 0.0)));
    assert!(has_point_at(&m, v(50.0, 0.0, 0.0)));
}

#[test]
fn across_several_faces_splits_every_crossed_edge() {
    let mut m = plane(100.0, 100.0, 3, 1);
    let pts = [KnifePoint::edge(edge_at(&m, v(-50.0, 0.0, 0.0)), 0.5), KnifePoint::edge(edge_at(&m, v(50.0, 0.0, 0.0)), 0.5)];
    let edges = knife_cut(&mut m, &pts, &top_view());
    assert_eq!(edges.len(), 3);
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 6);
    assert_eq!(m.num_points(), 12);
    // Every face is still a quad after a straight cut through quads.
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            assert_eq!(m.face_size(f), 4);
        }
    }
}

#[test]
fn vertex_to_vertex_and_a_diagonal_cut() {
    let mut m = plane(100.0, 100.0, 2, 2);
    let mut a = INVALID;
    let mut b = INVALID;
    for p in 0..m.point_capacity() {
        if m.position(p).distance(v(-50.0, 0.0, -50.0)) < 1e-6 {
            a = p;
        }
        if m.position(p).distance(v(50.0, 0.0, 50.0)) < 1e-6 {
            b = p;
        }
    }
    assert!(a != INVALID && b != INVALID);
    let edges = knife_cut(&mut m, &[KnifePoint::point(a), KnifePoint::point(b)], &top_view());
    // Through the centre vertex: two diagonals, no new points.
    assert_eq!(edges.len(), 2);
    assert!(valid(&m));
    assert_eq!(m.num_points(), 9);
    assert_eq!(m.num_faces(), 6);
}

#[test]
fn points_inside_a_face_become_a_polyline_through_it() {
    let mut m = plane(100.0, 100.0, 1, 1);
    let pts = [
        KnifePoint::edge(edge_at(&m, v(-50.0, 0.0, 0.0)), 0.5),
        KnifePoint::face(0, v(-10.0, 0.0, 20.0)),
        KnifePoint::face(0, v(10.0, 0.0, -20.0)),
        KnifePoint::edge(edge_at(&m, v(50.0, 0.0, 0.0)), 0.5),
    ];
    let edges = knife_cut(&mut m, &pts, &top_view());
    assert_eq!(edges.len(), 3);
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 2);
    assert_eq!(m.num_points(), 8);
    assert!(has_point_at(&m, v(-10.0, 0.0, 20.0)));
    assert!(has_point_at(&m, v(10.0, 0.0, -20.0)));
}

#[test]
fn a_path_that_starts_inside_a_face_cuts_only_from_the_first_border_it_reaches() {
    let mut m = plane(90.0, 30.0, 3, 1);
    let start = face_at(&m, v(-30.0, 0.0, 0.0));
    let pts = [KnifePoint::face(start, v(-30.0, 0.0, 0.0)), KnifePoint::edge(edge_at(&m, v(45.0, 0.0, 0.0)), 0.5)];
    knife_cut(&mut m, &pts, &top_view());
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 5);
    assert!(!has_point_at(&m, v(-30.0, 0.0, 0.0)));
}

#[test]
fn closed_path_around_a_cube_and_edge_loop_plan() {
    let mut cube = make_cube(&CubeOptions::default());
    let top = edge_at(&cube, v(0.0, 50.0, 50.0));
    let plan = knife_loop_plan(&cube, top, 0.5, None);
    let lp = knife_apply(&mut cube, &plan);
    assert_eq!(lp.len(), 4);
    assert!(valid(&cube));
    assert_eq!(boundary_edge_count(&cube), 0);
    assert_eq!(euler(&cube), 2);
    assert_eq!(cube.num_faces(), 10);

    let mut c2 = make_cube(&CubeOptions::default());
    let pts = [
        KnifePoint::edge(edge_at(&c2, v(0.0, 50.0, 50.0)), 0.5),
        KnifePoint::edge(edge_at(&c2, v(0.0, 50.0, -50.0)), 0.5),
        KnifePoint::edge(edge_at(&c2, v(0.0, -50.0, -50.0)), 0.5),
        KnifePoint::edge(edge_at(&c2, v(0.0, -50.0, 50.0)), 0.5),
    ];
    let ring = knife_cut(&mut c2, &pts, &KnifeOptions { closed: true, ..Default::default() });
    assert_eq!(ring.len(), 4);
    assert!(valid(&c2));
    assert_eq!(euler(&c2), 2);
    assert_eq!(c2.num_faces(), 10);
    assert_eq!(c2.num_points(), 12);
}

#[test]
fn symmetry_cuts_the_mirror_side_too_and_keeps_on_plane_cuts_single() {
    let mut m = plane(100.0, 100.0, 4, 1);
    let sym = SymmetryMap::build(&m, &axis_plane(0, 0.0), 1e-3);
    assert_eq!(sym.unmatched_count(), 0);
    let mut o = top_view();
    o.symmetry = Some(&sym);
    let side = [KnifePoint::edge(edge_at(&m, v(37.5, 0.0, 50.0)), 0.3), KnifePoint::edge(edge_at(&m, v(37.5, 0.0, -50.0)), 0.3)];
    knife_cut(&mut m, &side, &o);
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 6);
    assert_eq!(m.num_points(), 14);
    assert_eq!(SymmetryMap::build(&m, &axis_plane(0, 0.0), 1e-3).unmatched_count(), 0);

    let mut c = plane(100.0, 100.0, 4, 1);
    let csym = SymmetryMap::build(&c, &axis_plane(0, 0.0), 1e-3);
    let mut o = top_view();
    o.symmetry = Some(&csym);
    let across = [KnifePoint::edge(edge_at(&c, v(-50.0, 0.0, 0.0)), 0.5), KnifePoint::edge(edge_at(&c, v(50.0, 0.0, 0.0)), 0.5)];
    knife_cut(&mut c, &across, &o);
    assert!(valid(&c));
    assert_eq!(c.num_faces(), 8);
    assert_eq!(c.num_points(), 15);
}

#[test]
fn through_cuts_hidden_faces_too() {
    let stacked = || {
        let mut m = plane(100.0, 100.0, 1, 1);
        let mut lower = plane(100.0, 100.0, 1, 1);
        for p in lower.positions_mut() {
            p.y -= 10.0;
        }
        m.append(&lower);
        m
    };
    let mut m = stacked();
    let pts = [KnifePoint::edge(edge_at(&m, v(-50.0, 0.0, 0.0)), 0.5), KnifePoint::edge(edge_at(&m, v(50.0, 0.0, 0.0)), 0.5)];
    knife_cut(&mut m, &pts, &top_view());
    assert_eq!(m.num_faces(), 3);

    let mut t = stacked();
    let mut o = top_view();
    o.through = true;
    knife_cut(&mut t, &pts, &o);
    assert!(valid(&t));
    assert_eq!(t.num_faces(), 4);
    assert!(has_point_at(&t, v(-50.0, -10.0, 0.0)));

    // Perspective from above gives the same cut on the top plane.
    let mut p = stacked();
    let persp = KnifeOptions { eye: Some(v(0.0, 500.0, 0.0)), ..Default::default() };
    knife_cut(&mut p, &pts, &persp);
    assert!(valid(&p));
    assert_eq!(p.num_faces(), 3);
}

#[test]
fn attributes_follow_and_nothing_happens_without_a_path() {
    let mut m = plane(100.0, 100.0, 1, 1);
    let none = knife_plan(&m, &[KnifePoint::face(0, DVec3::ZERO)], &top_view());
    assert!(none.segments.is_empty());
    assert!(knife_apply(&mut m, &none).is_empty());
    assert_eq!(m.num_faces(), 1);

    let mut u = plane(100.0, 100.0, 1, 1);
    assert!(u.corner_attrs().has("uv0"));
    let pts = [
        KnifePoint::edge(edge_at(&u, v(-50.0, 0.0, 0.0)), 0.5),
        KnifePoint::face(0, DVec3::ZERO),
        KnifePoint::edge(edge_at(&u, v(50.0, 0.0, 0.0)), 0.5),
    ];
    knife_cut(&mut u, &pts, &top_view());
    assert!(valid(&u));
    assert!(uvs_dont_overlap(&u));
}

#[test]
fn perspective_walk_over_a_curved_surface_stays_on_the_near_side() {
    let mut m = make_sphere(&SphereOptions::default());
    let before = m.num_faces();
    let fa = face_at(&m, v(-35.0, 5.0, 35.0));
    let fb = face_at(&m, v(35.0, 5.0, 35.0));
    let pts = [KnifePoint::face(fa, m.face_centroid(fa)), KnifePoint::face(fb, m.face_centroid(fb))];
    let o = KnifeOptions { eye: Some(v(0.0, 0.0, 500.0)), ..Default::default() };
    let plan = knife_plan(&m, &pts, &o);
    assert!(plan.failed.is_empty());
    for s in &plan.segments {
        assert!(m.face_centroid(s.face).z > 0.0);
    }
    knife_apply(&mut m, &plan);
    assert!(valid(&m));
    assert_eq!(euler(&m), 2);
    assert!(m.num_faces() > before);
}
