//! Tests: component paths.

use aztek_geo::component_paths::*;
use aztek_geo::primitives::*;
use aztek_geo::topology::{chain_edges, edge_loop};
use aztek_geo::{Index, Mesh, INVALID};

fn point_at(m: &Mesh, x: f64, z: f64) -> Index {
    (0..m.point_capacity())
        .find(|&p| m.point_valid(p) && (m.position(p).x - x).abs() < 1e-6 && (m.position(p).z - z).abs() < 1e-6)
        .unwrap_or(INVALID)
}

fn face_at(m: &Mesh, x: f64, z: f64) -> Index {
    (0..m.face_capacity())
        .find(|&f| m.face_valid(f) && (m.face_centroid(f).x - x).abs() < 1e-6 && (m.face_centroid(f).z - z).abs() < 1e-6)
        .unwrap_or(INVALID)
}

fn edge_joining(m: &Mesh, x0: f64, z0: f64, x1: f64, z1: f64) -> Index {
    m.find_edge(point_at(m, x0, z0), point_at(m, x1, z1))
}

/// 4 x 4 quads, points on the integer grid from -2 to 2.
fn grid() -> Mesh {
    make_plane(&PlaneOptions { width: 4.0, depth: 4.0, subdiv_x: 4, subdiv_z: 4, ..Default::default() })
}

#[test]
fn edge_path_follows_shared_loop_ring_else_shortest() {
    let m = grid();
    let a = edge_joining(&m, -2.0, 0.0, -1.0, 0.0);
    let b = edge_joining(&m, 1.0, 0.0, 2.0, 0.0);
    assert_ne!(a, INVALID);
    assert_ne!(b, INVALID);
    let lp = edge_path_between(&m, a, b);
    assert_eq!(lp.len(), 4);
    assert_eq!(lp[0], a);
    assert_eq!(*lp.last().unwrap(), b);
    for &e in &lp {
        assert!(m.position(m.edge_point(e, 0)).z.abs() < 1e-9);
    }

    let r0 = edge_joining(&m, -1.0, -2.0, -1.0, -1.0);
    let r1 = edge_joining(&m, 2.0, -2.0, 2.0, -1.0);
    let ring = edge_path_between(&m, r0, r1);
    assert_eq!(ring.len(), 4);
    assert!(ring.contains(&edge_joining(&m, 0.0, -2.0, 0.0, -1.0)));
    assert!(ring.contains(&edge_joining(&m, 1.0, -2.0, 1.0, -1.0)));

    // Border edges share the boundary loop, so it is followed the shorter way round.
    let border = edge_path_between(&m, edge_joining(&m, -2.0, -2.0, -1.0, -2.0), edge_joining(&m, 1.0, 2.0, 2.0, 2.0));
    assert_eq!(border.len(), 9);
    for &e in &border {
        assert!(m.is_boundary_edge(e));
    }

    // Neither loop nor ring: the shortest connected chain.
    let c0 = edge_joining(&m, -1.0, -1.0, 0.0, -1.0);
    let c1 = edge_joining(&m, 0.0, 1.0, 1.0, 1.0);
    let path = edge_path_between(&m, c0, c1);
    assert!(path.contains(&c0));
    assert!(path.contains(&c1));
    assert_eq!(chain_edges(&m, &path).len(), 1);
    assert_eq!(path.len(), 4);
}

#[test]
fn edge_path_takes_shorter_way_round_closed_loop() {
    let m = make_cylinder(&CylinderOptions { radius: 1.0, height: 2.0, subdiv_axis: 8, subdiv_height: 2, ..Default::default() });
    let mid = (0..m.edge_capacity())
        .find(|&e| {
            m.edge_valid(e) && m.position(m.edge_point(e, 0)).y.abs() < 1e-6 && m.position(m.edge_point(e, 1)).y.abs() < 1e-6
        })
        .unwrap();
    let lp = edge_loop(&m, mid);
    assert_eq!(lp.len(), 8);
    let path = edge_path_between(&m, lp[0], lp[5]);
    assert_eq!(path.len(), 4);
    for e in [lp[0], lp[5], lp[6], lp[7]] {
        assert!(path.contains(&e));
    }
}

#[test]
fn point_and_face_paths_between() {
    let m = grid();
    let line = point_path_between(&m, point_at(&m, -2.0, 0.0), point_at(&m, 2.0, 0.0));
    assert_eq!(line.len(), 5);
    for &p in &line {
        assert!(m.position(p).z.abs() < 1e-9);
    }
    let diag = point_path_between(&m, point_at(&m, -2.0, -2.0), point_at(&m, 2.0, 2.0));
    assert_eq!(diag.len(), 9);
    assert_eq!(diag[0], point_at(&m, -2.0, -2.0));
    assert_eq!(*diag.last().unwrap(), point_at(&m, 2.0, 2.0));

    let row = face_path_between(&m, face_at(&m, -1.5, 0.5), face_at(&m, 1.5, 0.5));
    assert_eq!(row.len(), 4);
    for &f in &row {
        assert!((m.face_centroid(f).z - 0.5).abs() < 1e-9);
    }
    let across = face_path_between(&m, face_at(&m, -1.5, -1.5), face_at(&m, 1.5, 1.5));
    assert_eq!(across.len(), 7);
    assert_eq!(across[0], face_at(&m, -1.5, -1.5));
    assert_eq!(*across.last().unwrap(), face_at(&m, 1.5, 1.5));
    assert_eq!(point_path_between(&m, point_at(&m, 0.0, 0.0), point_at(&m, 0.0, 0.0)).len(), 1);
}

#[test]
fn face_loop_crosses_the_edges_ring() {
    let m = grid();
    let lp = face_loop(&m, edge_joining(&m, 0.0, -1.0, 0.0, 0.0));
    assert_eq!(lp.len(), 4);
    for &f in &lp {
        assert!((m.face_centroid(f).z + 0.5).abs() < 1e-9);
    }
}

#[test]
fn contained_faces_flood_fill_inside_closed_borders() {
    let cyl = make_cylinder(&CylinderOptions {
        radius: 1.0,
        height: 2.0,
        subdiv_axis: 8,
        subdiv_height: 2,
        cap_fill: CapFill::Ngon,
        ..Default::default()
    });
    let top_ring: Vec<Index> = (0..cyl.edge_capacity())
        .filter(|&e| {
            cyl.edge_valid(e) && cyl.position(cyl.edge_point(e, 0)).y > 0.999 && cyl.position(cyl.edge_point(e, 1)).y > 0.999
        })
        .collect();
    assert_eq!(top_ring.len(), 8);
    let cap = contained_faces(&cyl, &top_ring, true);
    assert_eq!(cap.len(), 1);
    assert_eq!(cyl.face_size(cap[0]), 8);
    assert_eq!(contained_faces(&cyl, &top_ring, false).len() as Index, cyl.num_faces() - 1);

    // The mesh border is a wall: a square around the centre four quads of a grid.
    let m = grid();
    let mut square = Vec::new();
    for k in -1..1 {
        let k = k as f64;
        square.push(edge_joining(&m, k, -1.0, k + 1.0, -1.0));
        square.push(edge_joining(&m, k, 1.0, k + 1.0, 1.0));
        square.push(edge_joining(&m, -1.0, k, -1.0, k + 1.0));
        square.push(edge_joining(&m, 1.0, k, 1.0, k + 1.0));
    }
    let inside = contained_faces(&m, &square, true);
    assert_eq!(inside.len(), 4);
    for &f in &inside {
        assert!(m.face_centroid(f).x.abs() < 1.0);
    }
    assert!(contained_faces(&m, &[edge_joining(&m, -1.0, -1.0, 0.0, -1.0)], true).is_empty());
}
