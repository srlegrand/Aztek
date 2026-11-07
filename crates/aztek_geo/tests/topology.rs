//! Tests: topology.

mod common;
use common::*;

use aztek_geo::primitives::*;
use aztek_geo::topology::*;
use aztek_geo::{Index, Mesh, INVALID};
use glam::DVec3;

#[test]
fn append_and_copy() {
    let mut a = make_cube(&CubeOptions::default());
    let b = a.clone();
    let offset = a.append(&b);
    assert_eq!(offset, 8);
    assert_eq!(a.num_faces(), 12);
    assert!(valid(&a));
    assert_eq!(b.num_faces(), 6);
    assert!(approx(a.bounds().size().x, 100.0));
}

#[test]
fn loops_rings_boundaries_components() {
    let m = quad_grid(4, 3);
    // Interior horizontal edge between rows 1 and 2.
    let e = m.find_edge(6, 7);
    assert_eq!(edge_loop(&m, e).len(), 4);
    assert_eq!(edge_ring(&m, e).len(), 4);
    let loops = boundary_loops(&m);
    assert_eq!(loops.len(), 1);
    assert!(loops[0].closed);
    assert_eq!(loops[0].edges.len(), 14);

    let cube = make_cube(&CubeOptions::default());
    let ce = if cube.edge_corner(0) != INVALID { 0 } else { 1 };
    assert_eq!(edge_loop(&cube, ce).len(), 1); // valence-3 corners stop the loop
    assert_eq!(edge_ring(&cube, ce).len(), 4);

    let mut two = make_cube(&CubeOptions::default());
    two.append(&make_sphere(&SphereOptions::default()));
    let mut comp = Vec::new();
    assert_eq!(face_components(&two, &mut comp, &[]), 2);
}

fn loop_closed(m: &Mesh, lp: &[Index]) -> bool {
    if lp.len() < 3 {
        return false;
    }
    let (a, b) = (lp[0], *lp.last().unwrap());
    (0..2).any(|i| (0..2).any(|j| m.edge_point(a, i) == m.edge_point(b, j)))
}

fn horizontal(m: &Mesh, e: Index) -> bool {
    (m.position(m.edge_point(e, 0)).y - m.position(m.edge_point(e, 1)).y).abs() < 1e-6
}

fn on_side(m: &Mesh, e: Index, radius: f64) -> bool {
    (0..2).all(|i| {
        let q = m.position(m.edge_point(e, i));
        (q.x.hypot(q.z) - radius).abs() <= 1e-6
    })
}

fn valid_edges(m: &Mesh) -> impl Iterator<Item = Index> + '_ {
    (0..m.edge_capacity()).filter(|&e| m.edge_valid(e))
}

#[test]
fn edge_loops_run_through_valence_4_points_and_stop_at_poles() {
    // Cylinder with n-gon caps: rims and rings close, verticals stop at the rims.
    let m = make_cylinder(&CylinderOptions {
        subdiv_axis: 12,
        subdiv_height: 3,
        cap_fill: CapFill::Ngon,
        ..Default::default()
    });
    for e in valid_edges(&m) {
        let lp = edge_loop(&m, e);
        if horizontal(&m, e) {
            assert_eq!(lp.len(), 12);
            assert!(loop_closed(&m, &lp));
        } else {
            assert_eq!(lp.len(), 3);
        }
    }
    // Cylinder with fan caps.
    let m = make_cylinder(&CylinderOptions {
        subdiv_axis: 12,
        subdiv_height: 2,
        subdiv_caps: 1,
        ..Default::default()
    });
    for e in valid_edges(&m) {
        if horizontal(&m, e) && on_side(&m, e, 50.0) {
            assert_eq!(edge_loop(&m, e).len(), 12);
        }
    }
    // Torus: every loop closes.
    let m = make_torus(&TorusOptions {
        subdiv_axis: 12,
        subdiv_height: 8,
        ..Default::default()
    });
    for e in valid_edges(&m) {
        let lp = edge_loop(&m, e);
        assert!(lp.len() == 12 || lp.len() == 8);
        assert!(loop_closed(&m, &lp));
    }
    // Quad sphere: loops close or end on its valence-3 poles.
    let m = make_quad_sphere(&QuadSphereOptions {
        subdiv: 4,
        ..Default::default()
    });
    for e in valid_edges(&m) {
        let lp = edge_loop(&m, e);
        if loop_closed(&m, &lp) {
            assert_eq!(lp.len(), 16);
        } else {
            // Each open end sits on a pole.
            let mut end_a = m.edge_point(lp[0], 0);
            let mut end_b = m.edge_point(*lp.last().unwrap(), 0);
            if lp.len() > 1 {
                let (f1, b1) = (lp[1], lp[lp.len() - 2]);
                if m.edge_point(f1, 0) == end_a || m.edge_point(f1, 1) == end_a {
                    end_a = m.edge_point(lp[0], 1);
                }
                if m.edge_point(b1, 0) == end_b || m.edge_point(b1, 1) == end_b {
                    end_b = m.edge_point(*lp.last().unwrap(), 1);
                }
            }
            assert_eq!(m.point_valence(end_a), 3);
            assert_eq!(m.point_valence(end_b), 3);
        }
    }
    // UV sphere: meridians run pole to pole, parallels close.
    let m = make_sphere(&SphereOptions {
        subdiv_axis: 12,
        subdiv_height: 8,
        ..Default::default()
    });
    for e in valid_edges(&m) {
        let lp = edge_loop(&m, e);
        if horizontal(&m, e) {
            assert_eq!(lp.len(), 12);
            assert!(loop_closed(&m, &lp));
        } else {
            assert_eq!(lp.len(), 8);
        }
    }
    // Grid with mixed winding and its border.
    let mut m = quad_grid(4, 3);
    m.flip_face(1);
    m.flip_face(4);
    m.flip_face(6);
    assert_eq!(edge_loop(&m, m.find_edge(6, 7)).len(), 4);
    assert_eq!(edge_loop(&m, m.find_edge(1, 6)).len(), 3);
    let border = edge_loop(&m, m.find_edge(0, 1));
    assert_eq!(border.len(), 14);
    assert!(loop_closed(&m, &border));
    // Border runs on past a pole sitting on it: point 2 on the bottom border gets a second interior edge.
    let mut m = quad_grid(4, 2);
    let (mut c2, mut c6) = (INVALID, INVALID);
    m.for_each_face_corner(1, |c| {
        if m.corner_point(c) == 2 {
            c2 = c;
        }
        if m.corner_point(c) == 6 {
            c6 = c;
        }
    });
    m.split_face(1, c2, c6);
    assert_eq!(m.point_valence(2), 4);
    let border = edge_loop(&m, m.find_edge(0, 1));
    assert_eq!(border.len(), 12);
    assert!(loop_closed(&m, &border));
}

#[test]
fn from_polygons_builds_in_bulk_what_add_face_builds() {
    // A grid with mixed winding, a non-manifold fin, a duplicate face and faces from_polygons must reject.
    let n: u32 = 140;
    let mut points = Vec::new();
    for j in 0..=n {
        for i in 0..=n {
            points.push(DVec3::new(
                i as f64,
                ((i * 7 + j * 3) % 5) as f64,
                -(j as f64),
            ));
        }
    }
    let (mut sizes, mut indices) = (Vec::new(), Vec::new());
    let mut face = |pts: &[Index]| {
        sizes.push(pts.len() as Index);
        indices.extend_from_slice(pts);
    };
    for j in 0..n {
        for i in 0..n {
            let a = j * (n + 1) + i;
            let (b, c, d) = (a + 1, a + 1 + n + 1, a + n + 1);
            match (i + j) % 3 {
                0 => {
                    face(&[a, b, c]);
                    face(&[a, c, d]);
                }
                1 => face(&[d, c, b, a]),
                _ => face(&[a, b, c, d]),
            }
        }
    }
    let last = points.len() as Index;
    face(&[0, 1, 2]);
    face(&[0, 1, last - 1]);
    face(&[3, 4, 3]);
    face(&[5, 6]);
    face(&[7, 8, last + 10]);
    assert!(indices.len() >= 1 << 16);

    let mut serial = Mesh::new();
    for &p in &points {
        serial.add_point(p);
    }
    let mut at = 0;
    for &s in &sizes {
        serial.add_face(&indices[at..at + s as usize]);
        at += s as usize;
    }
    let bulk = Mesh::from_polygons(&points, &sizes, &indices);
    assert_eq!(bulk.num_faces(), serial.num_faces());
    assert_eq!(bulk.num_edges(), serial.num_edges());
    assert_eq!(bulk.num_corners(), serial.num_corners());
    assert_eq!(bulk.edge_capacity(), serial.edge_capacity());
    assert!(bulk.topology_dump() == serial.topology_dump());
}

#[test]
fn chains_regions_and_conversions() {
    let m = quad_grid(3, 3);
    // The row of three edges along the bottom, given out of order, chains into one open path.
    let row = [m.find_edge(2, 3), m.find_edge(0, 1), m.find_edge(1, 2)];
    let paths = chain_edges(&m, &row);
    assert_eq!(paths.len(), 1);
    assert!(!paths[0].closed);
    assert_eq!(paths[0].points.len(), 4);
    // A 2x2 block of faces has an 8-edge rim.
    let block = [0, 1, 3, 4];
    assert_eq!(region_boundary_edges(&m, &block).len(), 8);
    assert_eq!(points_of_faces(&m, &block).len(), 9);
    assert_eq!(edges_of_faces(&m, &block).len(), 12);
    assert_eq!(faces_of_points(&m, &[5]).len(), 4);
    assert_eq!(faces_of_edges(&m, &[m.find_edge(5, 6)]).len(), 2);
    assert_eq!(points_of_edges(&m, &row), vec![0, 1, 2, 3]);
}
