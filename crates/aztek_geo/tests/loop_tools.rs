//! Tests: loop tools.

mod common;

use aztek_geo::conform::PointMoves;
use aztek_geo::loop_tools::*;
use aztek_geo::ops::{edge_loop_fractions, insert_edge_loop, region_ring_edges, split_rings, EdgeLoopOptions};
use aztek_geo::primitives::*;
use aztek_geo::symmetry::{SymmetryMap, SymmetryPlane};
use aztek_geo::{Index, Mesh, INVALID};
use common::*;
use glam::DVec3;

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

fn point_at(m: &Mesh, p: DVec3) -> Index {
    (0..m.point_capacity()).find(|&i| m.point_valid(i) && m.position(i).distance(p) < 1e-6).unwrap_or(INVALID)
}

/// The plane's edges along z at x, between z_from and z_to (grid lines every 25 cm on a 100 cm, 4 x 4 plane).
fn column_edges(m: &Mesh, x: f64, z_from: f64, z_to: f64) -> Vec<Index> {
    let mut out = Vec::new();
    let mut z = z_from;
    while z < z_to - 1e-9 {
        out.push(m.find_edge(point_at(m, v(x, 0.0, z)), point_at(m, v(x, 0.0, z + 25.0))));
        z += 25.0;
    }
    out
}

fn points_at_x(m: &Mesh, x: f64) -> usize {
    (0..m.point_capacity()).filter(|&i| m.point_valid(i) && (m.position(i).x - x).abs() < 1e-6).count()
}

fn mirrored(m: &Mesh) -> bool {
    SymmetryMap::build(m, &SymmetryPlane::default(), 1e-4).unmatched_count() == 0
}

fn plane(w: f64, sx: i32) -> Mesh {
    make_plane(&PlaneOptions { width: w, depth: w, subdiv_x: sx, subdiv_z: sx, ..Default::default() })
}

fn mix(a: DVec3, b: DVec3, t: f64) -> DVec3 {
    a * (1.0 - t) + b * t
}

#[test]
fn several_loops_follow_fraction_and_spacing() {
    let even = edge_loop_fractions(&EdgeLoopOptions { fraction: 0.5, count: 3, ..Default::default() });
    assert_eq!(even.len(), 3);
    assert!(approx(even[0], 0.25));
    assert!(approx(even[2], 0.75));

    let slid = edge_loop_fractions(&EdgeLoopOptions { fraction: 0.3, count: 2, ..Default::default() });
    assert_eq!(slid.len(), 2);
    assert!(approx(slid[0], 0.3 - 1.0 / 6.0));
    assert!(approx(slid[1], 0.3 + 1.0 / 6.0));

    let packed = edge_loop_fractions(&EdgeLoopOptions { fraction: 0.5, count: 2, spacing: 0.3 });
    assert!(approx(packed[1] - packed[0], 0.1));

    // Pushed against the end, the group slides back onto the edge instead of leaving it.
    let end = edge_loop_fractions(&EdgeLoopOptions { fraction: 0.0, count: 2, ..Default::default() });
    assert!(end[0] > 0.0);
    assert!(approx(end[1] - end[0], 1.0 / 3.0));

    let mut pl = plane(100.0, 1);
    let ring = pl.find_edge(point_at(&pl, v(-50.0, 0.0, 50.0)), point_at(&pl, v(-50.0, 0.0, -50.0)));
    assert_ne!(ring, INVALID);
    let from = pl.position(pl.edge_point(ring, 0));
    let to = pl.position(pl.edge_point(ring, 1));
    let loops = insert_edge_loop(&mut pl, ring, &EdgeLoopOptions { fraction: 0.25, count: 2, spacing: 0.5 });
    assert!(valid(&pl));
    assert_eq!(loops.len(), 2);
    assert_eq!(pl.num_faces(), 3);
    // 0.25 either side by half of a 1/6 gap, from the ring edge's first point.
    assert_ne!(point_at(&pl, mix(from, to, 0.25 - 1.0 / 12.0)), INVALID);
    assert_ne!(point_at(&pl, mix(from, to, 0.25 + 1.0 / 12.0)), INVALID);
}

#[test]
fn offset_full_loop_gets_a_loop_either_side() {
    let mut m = plane(100.0, 4);
    let edges = column_edges(&m, 0.0, -50.0, 50.0);
    let made = offset_edge_loops(&mut m, &edges, &OffsetLoopOptions::default(), false);
    assert!(valid(&m));
    assert_eq!(made.len(), 8);
    assert_eq!(m.num_faces(), 24);
    assert_eq!(points_at_x(&m, 6.25), 5);
    assert_eq!(points_at_x(&m, -6.25), 5);

    let mut one = plane(100.0, 4);
    let edges = column_edges(&one, 0.0, -50.0, 50.0);
    offset_edge_loops(
        &mut one,
        &edges,
        &OffsetLoopOptions { distance: 10.0, absolute: true, side: OffsetSide::Left, ..Default::default() },
        false,
    );
    assert!(valid(&one));
    assert_eq!(one.num_faces(), 20);
    assert_eq!(points_at_x(&one, 10.0) + points_at_x(&one, -10.0), 5);

    let mut removed = plane(100.0, 4);
    let edges = column_edges(&removed, 0.0, -50.0, 50.0);
    offset_edge_loops(&mut removed, &edges, &OffsetLoopOptions::default(), true);
    assert!(valid(&removed));
    assert_eq!(removed.num_faces(), 20);
    assert_eq!(points_at_x(&removed, 0.0), 0);
}

#[test]
fn offset_open_ends_converge_or_wrap() {
    let mut m = plane(100.0, 4);
    let edges = column_edges(&m, 0.0, -25.0, 25.0);
    offset_edge_loops(&mut m, &edges, &OffsetLoopOptions::default(), false);
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 20);
    assert_eq!(points_at_x(&m, 6.25), 1);

    let mut w = plane(100.0, 4);
    let edges = column_edges(&w, 0.0, -25.0, 25.0);
    let made = offset_edge_loops(&mut w, &edges, &OffsetLoopOptions { ends: OffsetEnds::Wrap, ..Default::default() }, false);
    assert!(valid(&w));
    assert_eq!(made.len(), 8);
    assert_eq!(w.num_faces(), 24);
    // The two loops meet on the edges carrying the chain on past its ends.
    assert_ne!(point_at(&w, v(0.0, 0.0, 31.25)), INVALID);
    assert_ne!(point_at(&w, v(0.0, 0.0, -31.25)), INVALID);
}

#[test]
fn offset_follows_symmetry_one_side_included() {
    for side in [OffsetSide::Both, OffsetSide::Left] {
        let mut m = plane(100.0, 4);
        let sym = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
        let edges = column_edges(&m, -25.0, -50.0, 50.0);
        offset_edge_loops(&mut m, &edges, &OffsetLoopOptions { side, symmetry: Some(&sym), ..Default::default() }, false);
        assert!(valid(&m));
        assert_eq!(m.num_faces(), if side == OffsetSide::Both { 32 } else { 24 });
        assert!(mirrored(&m));
    }
}

fn height_at(moves: &PointMoves, m: &Mesh, x: f64, z: f64) -> f64 {
    for mv in moves {
        if (m.position(mv.point).x - x).abs() < 1e-9 && (m.position(mv.point).z - z).abs() < 1e-9 {
            return mv.position.y;
        }
    }
    -1e9
}

#[test]
fn edit_edge_flow_bends_flat_loop_to_neighbours_curve() {
    // A parabola across x, with the x = 0 column pressed flat between its neighbours.
    let mut m = plane(100.0, 4);
    for p in 0..m.point_capacity() {
        let mut q = m.position(p);
        q.y = if q.x.abs() < 1e-9 { 0.0 } else { 10.0 - q.x * q.x / 62.5 };
        m.set_position(p, q);
    }
    let lp = column_edges(&m, 0.0, -50.0, 50.0);
    let up = edit_edge_flow(&m, &lp, &[], &EdgeFlowOptions::default());
    assert_eq!(up.len(), 5);
    assert!(height_at(&up, &m, 0.0, 0.0) > 3.0);
    for mv in &up {
        assert!(approx(mv.position.x, m.position(mv.point).x));
        assert!(approx(mv.position.z, m.position(mv.point).z));
    }

    assert!(approx(height_at(&edit_edge_flow(&m, &lp, &[], &EdgeFlowOptions { amount: 0.0, ..Default::default() }), &m, 0.0, 0.0), 0.0));
    assert!(height_at(&edit_edge_flow(&m, &lp, &[], &EdgeFlowOptions { amount: -1.0, ..Default::default() }), &m, 0.0, 0.0) < -3.0);
    assert!(height_at(&edit_edge_flow(&m, &lp, &[], &EdgeFlowOptions { edge_flow: false, ..Default::default() }), &m, 0.0, 0.0) > 1.0);

    // A loose vertex fits both directions: across x it rises, along z it is already flat.
    let centre = point_at(&m, v(0.0, 0.0, 0.0));
    let loose = edit_edge_flow(&m, &[], &[centre], &EdgeFlowOptions::default());
    assert_eq!(loose.len(), 1);
    assert!(loose[0].position.y > 1.0);
    assert!(loose[0].position.y < height_at(&up, &m, 0.0, 0.0));
}

#[test]
fn edit_edge_flow_selections_of_points() {
    let m = plane(100.0, 4);
    let column: Vec<Index> = (0..5).map(|k| point_at(&m, v(0.0, 0.0, -50.0 + 25.0 * k as f64))).collect();
    let (edges, loose) = flow_selection_of_points(&m, &column);
    assert_eq!(edges.len(), 4);
    assert!(loose.is_empty());

    let mut patch = Vec::new();
    for x in [-25.0, 0.0, 25.0] {
        for z in [-25.0, 0.0, 25.0] {
            patch.push(point_at(&m, v(x, 0.0, z)));
        }
    }
    let (_, loose) = flow_selection_of_points(&m, &patch);
    assert_eq!(loose.len(), 9);
}

#[test]
fn edit_edge_flow_moves_neighbouring_loops_together() {
    // Three flat columns side by side in a parabola across x: each curves from the unselected columns past them.
    let mut m = plane(200.0, 8);
    for p in 0..m.point_capacity() {
        let mut q = m.position(p);
        q.y = if q.x.abs() < 30.0 { 0.0 } else { 10.0 - q.x * q.x / 250.0 };
        m.set_position(p, q);
    }
    let mut loops = Vec::new();
    for x in [-25.0, 0.0, 25.0] {
        loops.extend(column_edges(&m, x, -100.0, 100.0));
    }
    let moves = edit_edge_flow(&m, &loops, &[], &EdgeFlowOptions::default());
    let h = |x: f64| height_at(&moves, &m, x, 0.0);
    assert!(h(0.0) > 8.0);
    assert!(h(25.0) > 5.0);
    assert!(h(25.0) < h(0.0));
    assert!(approx(h(-25.0), h(25.0)));
}

#[test]
fn split_rings_subdivides_one_way() {
    let mut m = plane(100.0, 4);
    let column = column_edges(&m, 0.0, -50.0, 50.0);
    let made = split_rings(&mut m, &column, 1, None);
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 32);
    assert_eq!(made.len(), 16);
    assert_ne!(point_at(&m, v(0.0, 0.0, -37.5)), INVALID);
    assert_eq!(point_at(&m, v(-37.5, 0.0, 0.0)), INVALID);

    // Two edges of one ring cut it once.
    let mut once = plane(100.0, 4);
    let pair = [column_edges(&once, 0.0, -50.0, -25.0)[0], column_edges(&once, 25.0, -50.0, -25.0)[0]];
    split_rings(&mut once, &pair, 2, None);
    assert!(valid(&once));
    assert_eq!(once.num_faces(), 24);

    // Faces: a strip along x gets lines across it, through whole rings.
    let mut strip = plane(100.0, 4);
    let row: Vec<Index> =
        (0..strip.face_capacity()).filter(|&f| strip.face_valid(f) && (strip.face_centroid(f).z - 12.5).abs() < 1e-6).collect();
    assert_eq!(row.len(), 4);
    let rings = region_ring_edges(&strip, &row, true);
    split_rings(&mut strip, &rings, 1, None);
    assert!(valid(&strip));
    assert_eq!(strip.num_faces(), 32);
    assert_eq!(points_at_x(&strip, 12.5), 5);
    let mut along = plane(100.0, 4);
    let rings = region_ring_edges(&along, &row, false);
    split_rings(&mut along, &rings, 1, None);
    assert_eq!(along.num_faces(), 20);
    assert_ne!(point_at(&along, v(0.0, 0.0, 12.5)), INVALID);
}

#[test]
fn split_rings_follows_symmetry() {
    let mut m = plane(100.0, 4);
    let sym = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
    let left = m.find_edge(point_at(&m, v(-50.0, 0.0, 0.0)), point_at(&m, v(-25.0, 0.0, 0.0)));
    let right = m.find_edge(point_at(&m, v(25.0, 0.0, 0.0)), point_at(&m, v(50.0, 0.0, 0.0)));
    split_rings(&mut m, &[left, right], 1, Some(&sym));
    assert!(valid(&m));
    assert_eq!(m.num_faces(), 24);
    assert_eq!(points_at_x(&m, -37.5), 5);
    assert_eq!(points_at_x(&m, 37.5), 5);
    assert!(mirrored(&m));
}

#[test]
fn edit_edge_flow_centres_loop_on_band_curving_in_its_plane() {
    // A flat 4 x 4 plane bent into an annulus sector: x is the radius from 200, z the angle at 15 degrees per 25 cm.
    let annulus = |x: f64, z: f64| {
        let r = 200.0 + x;
        let a = (z / 25.0 * 15.0).to_radians();
        v(r * a.cos(), 0.0, r * a.sin())
    };
    let mut m = plane(100.0, 4);
    let uv: Vec<DVec3> = m.positions().to_vec();
    for p in 0..m.point_capacity() {
        m.set_position(p, annulus(uv[p as usize].x, uv[p as usize].z));
    }
    // The z = 0 row pressed onto the chords toward one neighbour: off the arc and off centre.
    for p in 0..m.point_capacity() {
        let q = uv[p as usize];
        if q.z.abs() < 1e-9 {
            m.set_position(p, mix(annulus(q.x, -25.0), annulus(q.x, 25.0), 0.3));
        }
    }
    let row: Vec<Index> = (0..m.edge_capacity())
        .filter(|&e| {
            m.edge_valid(e) && uv[m.edge_point(e, 0) as usize].z.abs() < 1e-9 && uv[m.edge_point(e, 1) as usize].z.abs() < 1e-9
        })
        .collect();
    assert_eq!(row.len(), 4);
    for mv in edit_edge_flow(&m, &row, &[], &EdgeFlowOptions::default()) {
        assert!(mv.position.z.atan2(mv.position.x).abs() < 1e-9);
        let want = 200.0 + uv[mv.point as usize].x;
        assert!(approx_eps(mv.position.length(), want, 1e-9));
    }
}

#[test]
fn edit_edge_flow_spreads_split_rings_as_subdivision_would() {
    // An open 12-sided cylinder, every column split twice: the new points land evenly on the circle.
    let mut m = make_cylinder(&CylinderOptions { radius: 100.0, height: 100.0, subdiv_axis: 12, caps: false, ..Default::default() });
    let bottom: Vec<Index> = (0..m.edge_capacity())
        .filter(|&e| m.edge_valid(e) && m.position(m.edge_point(e, 0)).y < -49.0 && m.position(m.edge_point(e, 1)).y < -49.0)
        .collect();
    assert_eq!(bottom.len(), 12);
    let made = split_rings(&mut m, &bottom, 2, None);
    assert!(valid(&m));
    let moves = edit_edge_flow(&m, &made, &[], &EdgeFlowOptions::default());
    assert_eq!(moves.len(), 2 * 2 * 12);
    for mv in &moves {
        assert!(approx_eps(mv.position.x.hypot(mv.position.z), 100.0, 1e-9));
        m.set_position(mv.point, mv.position);
    }
    // Every edge round the circle is the same length.
    let want = 2.0 * 100.0 * 5f64.to_radians().sin();
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) && (m.position(m.edge_point(e, 0)).y - m.position(m.edge_point(e, 1)).y).abs() < 1e-9 {
            assert!(approx_eps(m.edge_length(e), want, 1e-9));
        }
    }
}
