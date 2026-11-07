//! Tests: ops.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use aztek_geo::normals::{set_hard_edges_by_angle, update_corner_normals, CornerNormalOptions};
use aztek_geo::ops::*;
use aztek_geo::primitives::*;
use aztek_geo::symmetry::{axis_plane, SymmetryMap, SymmetryPlane};
use aztek_geo::topology::{edge_loop, faces_of_points};
use aztek_geo::triangulate::TriangulateOptions;
use aztek_geo::{names, validate, Index, Mesh, INVALID};
use common::*;
use glam::{DMat4, DVec3};

fn v(x: f64, y: f64, z: f64) -> DVec3 {
    DVec3::new(x, y, z)
}

/// The face of a cube-like mesh whose normal is closest to dir.
fn face_facing(m: &Mesh, dir: DVec3) -> Index {
    let mut best = INVALID;
    let mut best_dot = -2.0;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            let d = m.face_normal(f).dot(dir);
            if d > best_dot {
                best_dot = d;
                best = f;
            }
        }
    }
    best
}

fn edge_between(m: &Mesh, a: DVec3, b: DVec3) -> Index {
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) {
            let p = m.position(m.edge_point(e, 0));
            let q = m.position(m.edge_point(e, 1));
            if (p.distance(a) < 1e-6 && q.distance(b) < 1e-6) || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6) {
                return e;
            }
        }
    }
    INVALID
}

fn check_closed(m: &Mesh) {
    assert!(valid(m));
    assert_eq!(boundary_edge_count(m), 0);
    assert_eq!(euler(m), 2);
}

fn is_symmetric(m: &Mesh, plane: &SymmetryPlane) -> bool {
    let map = SymmetryMap::build(m, plane, 1e-4);
    if map.unmatched_count() > 0 {
        eprintln!("{} points without a mirror", map.unmatched_count());
    }
    map.unmatched_count() == 0
}

fn sym(m: &Mesh) -> bool {
    is_symmetric(m, &SymmetryPlane::default())
}

fn sum_hard(m: &Mesh) -> Index {
    let hard = m.edge_attrs().get::<bool>(names::HARD);
    (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && !hard.is_empty() && hard[e as usize]).count() as Index
}

fn cube() -> Mesh {
    make_cube(&CubeOptions::default())
}

fn cube_sub(w: f64, h: f64, d: f64, sx: i32, sy: i32, sz: i32) -> Mesh {
    make_cube(&CubeOptions { width: w, height: h, depth: d, subdiv_x: sx, subdiv_y: sy, subdiv_z: sz, ..Default::default() })
}

fn plane(w: f64, d: f64, sx: i32, sz: i32) -> Mesh {
    make_plane(&PlaneOptions { width: w, depth: d, subdiv_x: sx, subdiv_z: sz, ..Default::default() })
}

fn cylinder(radius: f64, height: f64, axis: i32, hs: i32, caps_subdiv: i32, caps: bool) -> Mesh {
    make_cylinder(&CylinderOptions {
        radius,
        height,
        subdiv_axis: axis,
        subdiv_height: hs,
        subdiv_caps: caps_subdiv,
        caps,
        ..Default::default()
    })
}

fn all_faces(m: &Mesh) -> Vec<Index> {
    (0..m.face_capacity()).collect()
}

#[test]
fn delete() {
    let mut c = cube();
    let top = face_facing(&c, v(0.0, 1.0, 0.0));
    delete_faces(&mut c, &[top], None);
    assert!(valid(&c));
    assert_eq!(c.num_faces(), 5);
    assert_eq!(boundary_edge_count(&c), 4);

    let mut c2 = cube();
    delete_edges(&mut c2, &[0], None);
    assert!(valid(&c2));
    assert_eq!(c2.num_faces(), 4);
    assert_eq!(c2.num_edges(), 11);

    let mut c3 = cube();
    delete_points(&mut c3, &[0], None);
    assert!(valid(&c3));
    assert_eq!(c3.num_faces(), 3);
    assert_eq!(c3.num_points(), 7);
}

#[test]
fn extrude() {
    let mut c = cube();
    let top = face_facing(&c, v(0.0, 1.0, 0.0));
    let mut eo = ExtrudeOptions { offset: 50.0, ..Default::default() };
    let r = extrude_faces(&mut c, &[top], &eo);
    check_closed(&c);
    assert_eq!(r.sides.len(), 4);
    assert_eq!(c.num_faces(), 10);
    assert!(approx(c.bounds().max.y, 100.0));
    assert!(approx(signed_volume(&c), 1.5e6));
    // Cube edges were hard, so are the new ones around the extrusion.
    assert_eq!(sum_hard(&c), 20);

    // Region of two neighbouring faces.
    let mut grid = cube_sub(100.0, 100.0, 100.0, 2, 1, 1);
    let both: Vec<Index> = (0..grid.face_capacity()).filter(|&f| grid.face_normal(f).y > 0.9).collect();
    assert_eq!(both.len(), 2);
    let r = extrude_faces(&mut grid, &both, &eo);
    check_closed(&grid);
    assert_eq!(r.sides.len(), 6);
    assert!(approx(signed_volume(&grid), 1.5e6));

    let mut ind = cube_sub(100.0, 100.0, 100.0, 2, 1, 1);
    eo.individual = true;
    let r = extrude_faces(&mut ind, &both, &eo);
    check_closed(&ind);
    assert_eq!(r.sides.len(), 8);
}

#[test]
fn extrude_copy_() {
    let c = cube();
    let top = face_facing(&c, v(0.0, 1.0, 0.0));
    let eo = ExtrudeOptions { offset: 50.0, ..Default::default() };
    let cap = extrude_copy(&c, &[top], &eo, false);
    assert_eq!(c.num_faces(), 6);
    assert_eq!(cap.mesh.num_faces(), 1);
    assert_eq!(cap.cap.len(), 1);
    assert!(approx(cap.mesh.face_centroid(cap.cap[0]).y, 100.0));
    assert!(approx(cap.mesh.face_normal(cap.cap[0]).y, 1.0));

    let walls = extrude_copy(&c, &[top], &eo, true);
    assert_eq!(walls.mesh.num_faces(), 5);
    assert_eq!(walls.cap.len(), 1);
    assert!(valid(&walls.mesh));
    // Open where the walls met the cube's top.
    assert_eq!(boundary_edge_count(&walls.mesh), 4);
    assert!(approx(walls.mesh.bounds().min.y, 50.0));
    assert!(approx(walls.mesh.bounds().max.y, 100.0));
}

#[test]
fn inset() {
    let mut c = cube();
    let top = face_facing(&c, v(0.0, 1.0, 0.0));
    let mut io = InsetOptions { thickness: 10.0, ..Default::default() };
    inset_faces(&mut c, &[top], &io);
    check_closed(&c);
    assert_eq!(c.num_faces(), 10);
    assert!(approx(c.face_area(top), 80.0 * 80.0));
    assert!(approx(signed_volume(&c), 1e6));

    let mut c2 = cube();
    io.depth = -10.0;
    let top = face_facing(&c2, v(0.0, 1.0, 0.0));
    inset_faces(&mut c2, &[top], &io);
    check_closed(&c2);
    // The rim slopes down to the inner face: a frustum comes out.
    assert!(approx(signed_volume(&c2), 1e6 - 10.0 / 3.0 * (10000.0 + 6400.0 + 8000.0)));
}

#[test]
fn inset_clamps_on_faces_narrower_than_twice_the_thickness() {
    // The +x side is 2.5 wide in z; split in two along y for the region case.
    for individual in [true, false] {
        let mut m = make_cube(&CubeOptions { width: 100.0, height: 100.0, depth: 2.5, subdiv_y: 2, ..Default::default() });
        let sel: Vec<Index> = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_normal(f).x > 0.5).collect();
        assert_eq!(sel.len(), 2);
        let area = m.face_area(sel[0]) + m.face_area(sel[1]);
        let r = inset_faces(&mut m, &sel, &InsetOptions { thickness: 7.0, individual, ..Default::default() });
        check_closed(&m);
        // The inner border closes on the medial line, so no inner face is left.
        assert!(r.faces.is_empty());
        assert_eq!(r.sides.len(), if individual { 8 } else { 6 });
        let mut worst_fold = 1.0f64;
        for e in 0..m.edge_capacity() {
            if m.edge_valid(e) && m.is_manifold_edge(e) {
                let c = m.edge_corner(e);
                worst_fold = worst_fold.min(m.face_normal(m.corner_face(c)).dot(m.face_normal(m.corner_face(m.corner_radial(c)))));
            }
        }
        assert!(worst_fold > -0.1);
        for f in 0..m.face_capacity() {
            if m.face_valid(f) {
                let longest = m.face_corners(f).iter().map(|&c| m.edge_length(m.corner_edge(c))).fold(0.0, f64::max);
                assert!(m.face_area(f) > 0.01 * longest);
            }
        }
        let mut rim_area = 0.0;
        for &f in r.sides.iter().chain(r.faces.iter()) {
            assert!(approx(m.face_normal(f).x, 1.0));
            rim_area += m.face_area(f);
        }
        // Inner faces and rim tile the original side exactly, with no overlap.
        assert!(approx(rim_area, area));
        assert!(approx(signed_volume(&m), 100.0 * 100.0 * 2.5));
    }
}

#[test]
fn inset_clamp_leaves_insets_with_room_unchanged() {
    for individual in [true, false] {
        let mut a = cube_sub(100.0, 100.0, 100.0, 3, 1, 3);
        let mut b = a.clone();
        let sel: Vec<Index> = (0..a.face_capacity()).filter(|&f| a.face_valid(f) && a.face_normal(f).y > 0.5).collect();
        inset_faces(&mut a, &sel, &InsetOptions { thickness: 10.0, depth: -5.0, individual, ..Default::default() });
        inset_faces(&mut b, &sel, &InsetOptions { thickness: 10.0, depth: -5.0, individual, clamp: false, ..Default::default() });
        assert_eq!(a.point_capacity(), b.point_capacity());
        for p in 0..a.point_capacity() {
            if a.point_valid(p) {
                assert!(a.position(p).distance(b.position(p)) < 1e-12);
            }
        }
    }
}

#[test]
fn bevel() {
    let mut c = cube();
    let e = edge_between(&c, v(50.0, 50.0, 50.0), v(50.0, 50.0, -50.0));
    assert_ne!(e, INVALID);
    let mut bo = BevelOptions { offset: 10.0, ..Default::default() };
    let made = bevel_edges(&mut c, &[e], &bo);
    check_closed(&c);
    assert_eq!(made.len(), 1);
    assert_eq!(c.num_faces(), 7);
    assert!(approx(signed_volume(&c), 1e6 - 0.5 * 10.0 * 10.0 * 100.0));

    let mut all = cube();
    let edges: Vec<Index> = (0..all.edge_capacity()).collect();
    let made = bevel_edges(&mut all, &edges, &bo);
    check_closed(&all);
    assert_eq!(all.num_faces(), 26);
    assert_eq!(made.len(), 20);
    // At a 30 degree smoothing angle a chamfer is faceted everywhere.
    assert_eq!(sum_hard(&all), all.num_edges());

    let mut round = cube();
    bo.segments = 3;
    bevel_edges(&mut round, &edges, &bo);
    check_closed(&round);
    // Odd segment counts close each corner with three 1 x 2 quad grids around a centre triangle.
    assert_eq!(round.num_faces(), 6 + 12 * 3 + 8 * 7);
    assert!(signed_volume(&round) < 1e6);
    assert!(signed_volume(&round) > 1e6 - 12.0 * 0.5 * 10.0 * 10.0 * 80.0 - 8.0 * 1000.0);

    for seg in [2, 4] {
        let mut even = cube();
        bo.segments = seg;
        bevel_edges(&mut even, &edges, &bo);
        check_closed(&even);
        // Even counts make three quad grids per corner.
        assert_eq!(even.num_faces(), (6 + 12 * seg + 8 * 3 * (seg / 2) * (seg / 2)) as Index);
        for f in 0..even.face_capacity() {
            if even.face_valid(f) {
                assert_eq!(even.face_size(f), 4);
            }
        }
        // Every point of the round corner stays near the sphere of radius 10 around (40, 40, 40).
        for p in 0..even.point_capacity() {
            let q = even.position(p);
            if even.point_valid(p) && q.x > 40.0 && q.y > 40.0 && q.z > 40.0 {
                assert!(approx_eps(q.distance(DVec3::splat(40.0)), 10.0, 0.1));
            }
        }
    }

    let mut single = cube();
    bo.segments = 4;
    let e = edge_between(&single, v(50.0, 50.0, 50.0), v(50.0, 50.0, -50.0));
    let made = bevel_edges(&mut single, &[e], &bo);
    check_closed(&single);
    assert_eq!(made.len(), 4);
    assert_eq!(single.num_faces(), 6 + 4);
    // A round profile: every point of the strip lies on a cylinder of radius offset around x=y=40.
    for &f in &made {
        for p in single.face_points(f) {
            let q = single.position(p);
            assert!(approx((q.x - 40.0).hypot(q.y - 40.0), 10.0));
        }
    }
    // Soft between segments and against the sides (22.5 degrees), hard against the end caps.
    assert_eq!(sum_hard(&single), 11 + 2 * 4);

    // Two edges meeting at a corner.
    let mut two = cube();
    let pair = [
        edge_between(&two, v(50.0, 50.0, 50.0), v(50.0, 50.0, -50.0)),
        edge_between(&two, v(50.0, 50.0, 50.0), v(-50.0, 50.0, 50.0)),
    ];
    bo.segments = 2;
    bevel_edges(&mut two, &pair, &bo);
    check_closed(&two);

    // Open mesh, edge reaching the border.
    let mut pl = plane(100.0, 100.0, 2, 2);
    let mid = edge_between(&pl, v(0.0, 0.0, 50.0), v(0.0, 0.0, 0.0));
    assert_ne!(mid, INVALID);
    bo.segments = 1;
    bevel_edges(&mut pl, &[mid], &bo);
    assert!(valid(&pl));
}

#[test]
fn edge_loops() {
    let mut c = cube();
    let lp = insert_edge_loop(&mut c, 0, &EdgeLoopOptions::default());
    check_closed(&c);
    assert_eq!(lp.len(), 4);
    assert_eq!(c.num_faces(), 10);

    let mut c3 = cube();
    let mut lo = EdgeLoopOptions { count: 3, ..Default::default() };
    let lp = insert_edge_loop(&mut c3, 0, &lo);
    check_closed(&c3);
    assert_eq!(lp.len(), 12);
    assert_eq!(c3.num_faces(), 18);

    let mut pl = plane(100.0, 100.0, 3, 1);
    lo.count = 1;
    lo.fraction = 0.25;
    let e = edge_between(&pl, v(-50.0, 0.0, 50.0), v(-50.0, 0.0, -50.0));
    let lp = insert_edge_loop(&mut pl, e, &lo);
    assert!(valid(&pl));
    assert_eq!(lp.len(), 3);
    assert_eq!(pl.num_faces(), 6);
}

#[test]
fn merge_collapse_dissolve() {
    let mut a = plane(100.0, 100.0, 1, 1);
    let mut b = plane(100.0, 100.0, 1, 1);
    for p in b.positions_mut() {
        p.x += 100.0;
    }
    a.append(&b);
    assert_eq!(a.num_points(), 8);
    merge_by_distance(&mut a, &[], &MergeOptions { distance: 0.01, ..Default::default() });
    assert!(valid(&a));
    assert_eq!(a.num_points(), 6);
    assert_eq!(a.num_faces(), 2);
    assert_eq!(boundary_edge_count(&a), 6);

    let mut c = cube();
    let pts = collapse_edges(&mut c, &[0]);
    assert_eq!(pts.len(), 1);
    check_closed(&c);
    assert_eq!(c.num_points(), 7);

    let mut m2 = cube();
    assert_ne!(merge_to_center(&mut m2, &[0, 1]), INVALID);
    assert!(valid(&m2));
    let mut m3 = cube();
    assert_eq!(merge_to_target(&mut m3, &[1], 0), 0);
    assert!(valid(&m3));

    let mut grid = plane(100.0, 100.0, 2, 1);
    let e = edge_between(&grid, v(0.0, 0.0, 50.0), v(0.0, 0.0, -50.0));
    dissolve_edges(&mut grid, &[e], true);
    assert!(valid(&grid));
    assert_eq!(grid.num_faces(), 1);
    assert_eq!(grid.num_points(), 4);

    let mut g2 = plane(100.0, 100.0, 2, 2);
    let center = (0..g2.point_capacity()).filter(|&p| g2.position(p).length() < 1e-9).last().unwrap();
    let faces = dissolve_points(&mut g2, &[center]);
    assert!(valid(&g2));
    assert_eq!(faces.len(), 1);
    assert_eq!(g2.face_size(faces[0]), 8);
    assert!(approx(g2.face_normal(faces[0]).y, 1.0));
}

#[test]
fn bridge_() {
    let mut tube = cylinder(50.0, 300.0, 12, 3, 0, false);
    let middle: Vec<Index> = (0..tube.face_capacity()).filter(|&f| tube.face_centroid(f).y.abs() < 1.0).collect();
    assert_eq!(middle.len(), 12);
    delete_faces(&mut tube, &middle, None);
    assert_eq!(boundary_edge_count(&tube), 48);
    let inner: Vec<Index> = (0..tube.edge_capacity())
        .filter(|&e| tube.edge_valid(e) && tube.is_boundary_edge(e) && tube.position(tube.edge_point(e, 0)).y.abs() < 60.0)
        .collect();
    assert_eq!(inner.len(), 24);
    let made = bridge_edge_loops(&mut tube, &inner, &BridgeOptions { divisions: 1, ..Default::default() });
    assert_eq!(made.len(), 24);
    assert!(valid(&tube));
    assert_eq!(boundary_edge_count(&tube), 24);
}

/// A 6 x 5 sheet of 100 cm quads with a 4 x 1 slot cut at each z in slot_rows.
fn slotted_sheet(slot_rows: &[f64]) -> Mesh {
    let mut sheet = plane(600.0, 500.0, 6, 5);
    let mut cut = Vec::new();
    for f in 0..sheet.face_capacity() {
        for &z in slot_rows {
            if sheet.face_valid(f) && sheet.face_centroid(f).x.abs() < 200.0 && (sheet.face_centroid(f).z - z).abs() < 1.0 {
                cut.push(f);
            }
        }
    }
    delete_faces(&mut sheet, &cut, None);
    sheet
}

/// Border edges inside the sheet's outer rim, without those passing through skip.
fn slot_edges(m: &Mesh, skip: Option<DVec3>) -> Vec<Index> {
    (0..m.edge_capacity())
        .filter(|&e| {
            if !m.edge_valid(e) || !m.is_boundary_edge(e) {
                return false;
            }
            let mid = (m.position(m.edge_point(e, 0)) + m.position(m.edge_point(e, 1))) * 0.5;
            mid.x.abs() < 299.0 && mid.z.abs() < 249.0 && !skip.is_some_and(|s| mid.distance(s) < 1.0)
        })
        .collect()
}

#[test]
fn bridge_works_out_the_sides_of_a_slot() {
    // The whole border round the slot: its long sides bridge, the ends that join them are left.
    let mut sheet = slotted_sheet(&[-100.0]);
    assert_eq!(slot_edges(&sheet, None).len(), 10);
    assert_eq!(plan_bridges(&sheet, &slot_edges(&sheet, None)).len(), 1);
    let edges = slot_edges(&sheet, None);
    assert_eq!(bridge_edges(&mut sheet, &edges, &BridgeOptions::default()).len(), 4);
    assert!(valid(&sheet));
    assert_eq!(boundary_edge_count(&sheet), 22);

    // A U: one end left unselected.
    let mut u = slotted_sheet(&[-100.0]);
    let three = slot_edges(&u, Some(v(200.0, 0.0, -100.0)));
    assert_eq!(three.len(), 9);
    assert_eq!(bridge_edges(&mut u, &three, &BridgeOptions::default()).len(), 4);
    assert!(valid(&u));
    assert_eq!(boundary_edge_count(&u), 22);
}

#[test]
fn bridge_pairs_separate_selections() {
    // Two slots in one sheet each bridge across, not into each other.
    let mut sheet = slotted_sheet(&[-100.0, 100.0]);
    let edges = slot_edges(&sheet, None);
    assert_eq!(edges.len(), 20);
    assert_eq!(bridge_edges(&mut sheet, &edges, &BridgeOptions::default()).len(), 8);
    assert!(valid(&sheet));
    assert_eq!(boundary_edge_count(&sheet), 22);

    // Two gaps in a tube: each pair of facing loops bridges with its nearest partner.
    let mut tube = cylinder(50.0, 500.0, 12, 5, 0, false);
    let gaps: Vec<Index> =
        (0..tube.face_capacity()).filter(|&f| (tube.face_centroid(f).y.abs() - 100.0).abs() < 1.0).collect();
    assert_eq!(gaps.len(), 24);
    delete_faces(&mut tube, &gaps, None);
    let loops: Vec<Index> = (0..tube.edge_capacity())
        .filter(|&e| tube.edge_valid(e) && tube.is_boundary_edge(e) && tube.position(tube.edge_point(e, 0)).y.abs() < 240.0)
        .collect();
    assert_eq!(loops.len(), 48);
    assert_eq!(plan_bridges(&tube, &loops).len(), 2);
    assert_eq!(bridge_edges(&mut tube, &loops, &BridgeOptions::default()).len(), 24);
    assert!(valid(&tube));
    assert_eq!(boundary_edge_count(&tube), 24);
}

#[test]
fn poke_spin_fill_flip_conform_triangulate_quadrangulate() {
    let mut c = cube();
    let top = face_facing(&c, v(0.0, 1.0, 0.0));
    let made = poke_faces(&mut c, &[top]);
    assert_eq!(made.len(), 4);
    check_closed(&c);

    let mut grid = plane(100.0, 100.0, 2, 2);
    let mid = edge_between(&grid, v(0.0, 0.0, 0.0), v(0.0, 0.0, 50.0));
    let spun = spin_edges(&mut grid, &[mid]);
    assert_eq!(spun.len(), 1);
    assert!(valid(&grid));
    assert_eq!(grid.num_faces(), 4);

    let mut open = cube();
    delete_faces(&mut open, &[2], None);
    let made = fill_holes(&mut open, &[]);
    assert_eq!(made.len(), 1);
    check_closed(&open);
    assert!(approx(signed_volume(&open), 1e6));

    let mut sphere = make_sphere(&SphereOptions::default());
    flip_normals(&mut sphere, &[3, 50, 51, 52, 200]);
    assert!(!sphere.validate(validate::WINDING).is_empty());
    conform_normals(&mut sphere, &[]);
    check_closed(&sphere);
    assert!(signed_volume(&sphere) > 0.0);
    let all = all_faces(&sphere);
    flip_normals(&mut sphere, &all);
    conform_normals(&mut sphere, &[]);
    assert!(signed_volume(&sphere) > 0.0);

    let mut tri = cube();
    let all = all_faces(&tri);
    let made = triangulate_faces(&mut tri, &all, &TriangulateOptions::default());
    assert_eq!(made.len(), 12);
    check_closed(&tri);
    assert_eq!(sum_hard(&tri), 12);
    let made = quadrangulate_faces(&mut tri, &made, &QuadrangulateOptions::default());
    assert_eq!(made.len(), 6);
    check_closed(&tri);
    tri.compact(None);
    assert!(uvs_dont_overlap(&tri));
}

fn fan_cylinder(sides: i32) -> Mesh {
    make_cylinder(&CylinderOptions { subdiv_axis: sides, cap_fill: CapFill::Fan, ..Default::default() })
}

fn poles(m: &Mesh, valence: usize) -> Vec<Index> {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p) && faces_of_points(m, &[p]).len() == valence).collect()
}

fn count_sides(m: &Mesh, sides: Index) -> Index {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_size(f) == sides).count() as Index
}

#[test]
fn grid_fill_even_fans_become_a_ring_and_a_grid_of_quads() {
    let mut m = fan_cylinder(40);
    let volume = signed_volume(&m);
    let centres = poles(&m, 40);
    assert_eq!(centres.len(), 2);
    let faces = faces_of_points(&m, &centres);
    let made = grid_fill_faces(&mut m, &faces, &GridFillOptions::default(), None);
    assert_eq!(made.len(), 2 * (40 + 100));
    check_closed(&m);
    assert!(approx(signed_volume(&m), volume));
    assert_eq!(count_sides(&m, 4), m.num_faces());
    m.compact(None);
    assert!(uvs_dont_overlap(&m));
}

#[test]
fn grid_fill_odd_fans_keep_one_triangle() {
    let mut m = fan_cylinder(21);
    let faces = faces_of_points(&m, &poles(&m, 21));
    let made = grid_fill_faces(&mut m, &faces, &GridFillOptions::default(), None);
    assert_eq!(made.len(), 2 * (22 + 30));
    check_closed(&m);
    assert_eq!(count_sides(&m, 3), 2);
}

#[test]
fn grid_fill_holes_and_a_grid_right_to_the_border() {
    let mut m = fan_cylinder(16);
    let volume = signed_volume(&m);
    let faces = faces_of_points(&m, &poles(&m, 16));
    delete_faces(&mut m, &faces, None);
    let made = grid_fill_holes(&mut m, &[], &GridFillOptions { grid_size: 1.0, rotate: 3 }, None);
    assert_eq!(made.len(), 2 * 16);
    check_closed(&m);
    assert!(approx(signed_volume(&m), volume));
    assert_eq!(count_sides(&m, 4), m.num_faces());
}

#[test]
fn grid_fill_rectangular_holes_put_the_grids_corners_on_theirs() {
    for holes in [false, true] {
        let mut m = make_plane(&PlaneOptions { width: 120.0, depth: 120.0, subdiv_x: 12, subdiv_z: 12, ..Default::default() });
        let hole: Vec<Index> = (0..m.face_capacity())
            .filter(|&f| {
                let c: DVec3 = m.face_points(f).iter().map(|&p| m.position(p) / 4.0).sum();
                c.x.abs() > 10.0 && c.x.abs() < 30.0 && c.z > -20.0 && c.z < 10.0
            })
            .collect();
        assert_eq!(hole.len(), 12);
        let o = GridFillOptions { grid_size: 1.0, ..Default::default() };
        let made = if !holes {
            grid_fill_faces(&mut m, &hole, &o, None)
        } else {
            delete_faces(&mut m, &hole, None);
            let inner: Vec<Index> = (0..m.edge_capacity())
                .filter(|&e| {
                    m.edge_valid(e)
                        && m.position(m.edge_point(e, 0)).x.abs() < 50.0
                        && m.position(m.edge_point(e, 0)).z.abs() < 50.0
                })
                .collect();
            grid_fill_holes(&mut m, &inner, &o, None)
        };
        assert_eq!(made.len(), 12);
        for &f in &made {
            let p: Vec<DVec3> = m.face_points(f).iter().map(|&q| m.position(q)).collect();
            assert_eq!(p.len(), 4);
            assert!(approx((p[2] - p[0]).cross(p[3] - p[1]).length() / 2.0, 100.0));
        }
    }
}

#[test]
fn grid_fill_patches_that_are_not_discs_are_left_alone() {
    let mut m = fan_cylinder(12);
    let sides: Vec<Index> = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_size(f) == 4).collect();
    assert!(grid_fill_faces(&mut m, &sides, &GridFillOptions::default(), None).is_empty());
    check_closed(&m);
}

fn sub(m: &Mesh, levels: i32) -> Mesh {
    subdivide(m, &SubdivideOptions { levels, ..Default::default() }, None)
}

#[test]
fn subdivide_() {
    let mut c = cube();
    let s = sub(&c, 1);
    check_closed(&s);
    assert_eq!(s.num_faces(), 24);
    assert_eq!(s.num_points(), 26);
    assert!(signed_volume(&s) < 0.9e6);
    assert!(uvs_dont_overlap(&s));
    let s2 = sub(&c, 2);
    check_closed(&s2);
    assert_eq!(s2.num_faces(), 96);

    c.edge_attrs_mut().get_or_add::<f32>(names::CREASE).fill(2.0);
    let sharp = sub(&c, 2);
    assert!(approx(sharp.bounds().size().x, 100.0));
    assert!(approx(signed_volume(&sharp), 1e6));

    let pl = make_plane(&PlaneOptions::default());
    let sp = sub(&pl, 1);
    assert!(valid(&sp));
    assert!(approx(sp.bounds().size().x, 100.0));
}

#[test]
fn subdivide_map() {
    let c = cube();
    let mut map = SubdivideMap::default();
    let s = subdivide(&c, &SubdivideOptions { levels: 2, ..Default::default() }, Some(&mut map));
    assert_eq!(map.points.len(), c.point_capacity() as usize);
    assert_eq!(map.edges.len(), s.edge_capacity() as usize);
    assert_eq!(map.faces.len(), s.face_capacity() as usize);
    // Each cage edge becomes a chain of 4 pieces from one end's image to the other's.
    let mut ends: Vec<BTreeMap<Index, i32>> = vec![BTreeMap::new(); c.edge_capacity() as usize];
    for e in 0..s.edge_capacity() {
        if s.edge_valid(e) && map.edges[e as usize] != INVALID {
            for k in 0..2 {
                *ends[map.edges[e as usize] as usize].entry(s.edge_point(e, k)).or_insert(0) += 1;
            }
        }
    }
    for e in 0..c.edge_capacity() {
        if c.edge_valid(e) {
            // 4 pieces through 5 points; the two used once are the images of the edge's ends.
            assert_eq!(ends[e as usize].len(), 5);
            let tips: BTreeSet<Index> = ends[e as usize].iter().filter(|(_, &u)| u == 1).map(|(&p, _)| p).collect();
            let want: BTreeSet<Index> =
                [map.points[c.edge_point(e, 0) as usize], map.points[c.edge_point(e, 1) as usize]].into_iter().collect();
            assert_eq!(tips, want);
        }
    }
    let mut parts = vec![0; c.face_capacity() as usize];
    for f in 0..s.face_capacity() {
        if s.face_valid(f) {
            let cf = map.faces[f as usize];
            assert!(cf < c.face_capacity());
            parts[cf as usize] += 1;
            // Every piece lies on its cage face's side of the cube.
            assert!(s.face_centroid(f).dot(c.face_normal(cf)) > 0.0);
        }
    }
    for f in 0..c.face_capacity() {
        if c.face_valid(f) {
            assert_eq!(parts[f as usize], 16);
            assert!(s.position(map.face_points[f as usize]).dot(c.face_normal(f)) > 0.0);
        }
    }
    // Cage points land on the limit-ish corners, pulled in from the cage's.
    for p in 0..c.point_capacity() {
        if c.point_valid(p) {
            assert!(s.position(map.points[p as usize]).length() < c.position(p).length());
        }
    }
    let plain = sub(&c, 2);
    assert_eq!(plain.num_faces(), s.num_faces());
}

#[test]
fn subdivide_hard_edges() {
    let mut c = cube();
    set_hard_edges_by_angle(&mut c, 30.0, &[]);
    update_corner_normals(&mut c, &CornerNormalOptions::default());
    assert!(c.edge_attrs().has(names::HARD));
    let soft = sub(&c, 1);
    assert!(!soft.edge_attrs().has(names::HARD));
    assert!(!soft.corner_attrs().has(names::NORMAL));
    let hard = subdivide(&c, &SubdivideOptions { levels: 1, keep_hard_edges: true, ..Default::default() }, None);
    assert!(hard.edge_attrs().has(names::HARD));
}

#[test]
fn subdivide_stencils_match_subdivide() {
    let mut c = cube_sub(100.0, 100.0, 100.0, 2, 1, 1);
    c.edge_attrs_mut().get_or_add::<f32>(names::CREASE)[0] = 0.5;
    let opts = SubdivideOptions { levels: 2, ..Default::default() };
    let mut result = Mesh::new();
    let stencils = subdivide_stencils(&c, &opts, Some(&mut result));
    let mut pos = c.positions().to_vec();
    for s in &stencils {
        pos = s.apply(&pos);
    }
    assert_eq!(pos.len(), result.point_capacity() as usize);
    for p in 0..result.point_capacity() {
        assert!(pos[p as usize].distance(result.position(p)) < 1e-9);
    }
}

#[test]
fn mirror_separate_combine() {
    let mut sphere = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 16, subdiv_height: 8, ..Default::default() });
    let full = sphere.num_points();
    let left: Vec<Index> = (0..sphere.face_capacity()).filter(|&f| sphere.face_centroid(f).x < 0.0).collect();
    delete_faces(&mut sphere, &left, None);
    mirror_mesh(&mut sphere, &MirrorOptions::default());
    sphere.compact(None);
    check_closed(&sphere);
    assert_eq!(sphere.num_points(), full);
    assert!(signed_volume(&sphere) > 0.0);
    assert!(sym(&sphere));

    let mut two = cube();
    let other = make_torus(&TorusOptions::default());
    two.append(&other);
    let parts = separate(&two);
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].num_faces(), 6);
    assert_eq!(parts[1].num_faces(), other.num_faces());
    assert!(valid(&parts[1]));
    let back = combine(&[&parts[0], &parts[1]]);
    assert_eq!(back.num_faces(), two.num_faces());
}

fn shifted_cube() -> Mesh {
    let mut m = cube();
    for p in m.positions_mut() {
        p.x += 20.0;
    }
    m
}

#[test]
fn mirror_directions_cut_at_the_plane() {
    let run = |dir: MirrorDirection| {
        let mut m = shifted_cube();
        mirror_mesh(&mut m, &MirrorOptions { direction: dir, ..Default::default() });
        m.compact(None);
        m
    };

    let pos = run(MirrorDirection::Positive);
    check_closed(&pos);
    assert_eq!(pos.num_points(), 12);
    assert_eq!(pos.num_faces(), 10);
    assert!(sym(&pos));
    assert!(approx(pos.bounds().min.x, -70.0));
    assert!(approx(pos.bounds().max.x, 70.0));
    assert!(approx(signed_volume(&pos), 1.4e6));

    let neg = run(MirrorDirection::Negative);
    check_closed(&neg);
    assert_eq!(neg.num_points(), 12);
    assert_eq!(neg.num_faces(), 10);
    assert!(sym(&neg));
    assert!(approx(neg.bounds().max.x, 30.0));
    assert!(approx(signed_volume(&neg), 6e5));

    let both = run(MirrorDirection::Both);
    assert_eq!(both.num_points(), 16);
    assert_eq!(both.num_faces(), 12);
    assert!(approx(both.bounds().min.x, -70.0));

    // Other axes, and a plane given in another space: world x = 0 is object x = -20 here.
    let mut y = shifted_cube();
    mirror_mesh(&mut y, &MirrorOptions { plane: axis_plane(1, 0.0), direction: MirrorDirection::Negative, ..Default::default() });
    y.compact(None);
    check_closed(&y);
    assert_eq!(y.num_faces(), 10);
    assert!(approx(y.bounds().max.y, 50.0));
    let mut w = cube();
    mirror_mesh(
        &mut w,
        &MirrorOptions {
            space: DMat4::from_translation(v(20.0, 0.0, 0.0)),
            direction: MirrorDirection::Positive,
            ..Default::default()
        },
    );
    w.compact(None);
    check_closed(&w);
    assert!(approx(w.bounds().min.x, -90.0));
    assert!(approx(w.bounds().max.x, 50.0));
    assert!(is_symmetric(&w, &SymmetryPlane { normal: DVec3::X, offset: -20.0 }));
}

#[test]
fn mirror_cut_splits_crossing_faces_and_interpolates_uvs() {
    let shifted_plane = || {
        let mut q = make_plane(&PlaneOptions::default());
        for p in q.positions_mut() {
            p.x += 10.0;
        }
        q
    };
    let mut quad = shifted_plane();
    let mut o = MirrorOptions::default();
    mirror_mesh(&mut quad, &o);
    quad.compact(None);
    assert!(valid(&quad));
    assert_eq!(quad.num_faces(), 2);
    assert_eq!(quad.num_points(), 8);
    o.direction = MirrorDirection::Positive;
    let mut quad = shifted_plane();
    mirror_mesh(&mut quad, &o);
    quad.compact(None);
    assert!(valid(&quad));
    assert_eq!(quad.num_faces(), 2);
    assert_eq!(quad.num_points(), 6);
    for f in 0..quad.face_capacity() {
        assert_eq!(quad.face_size(f), 4);
    }
    let uv = quad.corner_attrs().get::<[f32; 2]>(&names::uv(0));
    assert!(!uv.is_empty());
    // The cut runs 40% of the way across the 100 wide plane.
    for c in 0..quad.corner_capacity() {
        if quad.corner_valid(c) && quad.position(quad.corner_point(c)).x.abs() < 1e-9 {
            let u = uv[c as usize][0];
            assert!((u - 0.4).abs().min((u - 0.6).abs()) < 1e-5);
        }
    }

    // A C shape whose two prongs reach across the plane: one face crossing it four times.
    let c_shape = || {
        let mut m = Mesh::new();
        let xy = [[-5.0, -10.0], [10.0, -10.0], [10.0, -6.0], [-2.0, -6.0], [-2.0, 6.0], [10.0, 6.0], [10.0, 10.0], [-5.0, 10.0]];
        let pts: Vec<Index> = xy.iter().map(|p| m.add_point(v(p[0], p[1], 0.0))).collect();
        m.add_face(&pts);
        m
    };
    let area = |m: &Mesh| (0..m.face_capacity()).filter(|&f| m.face_valid(f)).map(|f| m.face_area(f)).sum::<f64>();
    let mut prongs = c_shape();
    mirror_mesh(&mut prongs, &o);
    prongs.compact(None);
    assert!(valid(&prongs));
    assert_eq!(prongs.num_faces(), 4);
    assert_eq!(prongs.num_points(), 12);
    assert!(approx(area(&prongs), 160.0));
    assert!(sym(&prongs));

    let mut back = c_shape();
    o.direction = MirrorDirection::Negative;
    mirror_mesh(&mut back, &o);
    back.compact(None);
    assert!(valid(&back));
    assert_eq!(back.num_faces(), 2);
    assert!(approx(area(&back), 152.0));
    assert!(sym(&back));
}

#[test]
fn symmetry_maps() {
    let sphere = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 16, subdiv_height: 8, ..Default::default() });
    let map = SymmetryMap::build(&sphere, &SymmetryPlane::default(), 1e-3);
    assert_eq!(map.unmatched_count(), 0);
    let on_plane = (0..sphere.point_capacity()).filter(|&p| map.on_plane(p)).count();
    assert_eq!(on_plane, 2 * 7 + 2);

    // Topological match from an edge on the plane.
    let seed = (0..sphere.edge_capacity())
        .find(|&e| map.on_plane(sphere.edge_point(e, 0)) && map.on_plane(sphere.edge_point(e, 1)))
        .expect("seed");
    let topo = SymmetryMap::build_topological(&sphere, seed, &SymmetryPlane::default());
    assert!(!topo.is_empty());
    for p in 0..sphere.point_capacity() {
        assert_eq!(topo.mirror_point(p), map.mirror_point(p));
    }

    let f = 5;
    let mf = map.mirror_face(&sphere, f);
    assert_ne!(mf, INVALID);
    assert!(approx(sphere.face_centroid(mf).x, -sphere.face_centroid(f).x));
}

#[test]
fn symmetry_ops_stay_symmetric() {
    // Even subdivisions put a column of points on the plane.
    let mut m = cube_sub(100.0, 100.0, 100.0, 4, 4, 4);
    let map = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
    assert_eq!(map.unmatched_count(), 0);
    let sel: Vec<Index> = (0..m.face_capacity())
        .filter(|&f| {
            let c = m.face_centroid(f);
            m.face_normal(f).y > 0.9 && c.x > 0.0 && c.x < 25.0 && c.z > 0.0 && c.z < 25.0
        })
        .collect();
    assert_eq!(sel.len(), 1);
    let r = extrude_faces(&mut m, &sel, &ExtrudeOptions { offset: 20.0, symmetry: Some(&map), ..Default::default() });
    assert_eq!(r.faces.len(), 2);
    check_closed(&m);
    assert!(sym(&m));

    // Bevel an edge touching the plane.
    m.compact(None);
    let map = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
    assert_eq!(map.unmatched_count(), 0);
    let e = (0..m.edge_capacity())
        .find(|&i| {
            let a = m.position(m.edge_point(i, 0));
            let b = m.position(m.edge_point(i, 1));
            (a.y + 50.0).abs() < 1e-6
                && (b.y + 50.0).abs() < 1e-6
                && (a.z - b.z).abs() < 1e-6
                && a.x.min(b.x) > -1e-6
                && a.x.min(b.x) < 1e-6
                && a.z.abs() < 30.0
                && a.z > 0.0
        })
        .expect("edge");
    bevel_edges(&mut m, &[e], &BevelOptions { offset: 5.0, segments: 2, symmetry: Some(&map), ..Default::default() });
    check_closed(&m);
    assert!(sym(&m));

    // Inset across the plane.
    m.compact(None);
    let map = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
    let sel: Vec<Index> = (0..m.face_capacity())
        .filter(|&f| {
            let c = m.face_centroid(f);
            m.face_normal(f).z > 0.9 && c.x > 0.0 && c.x < 25.0 && c.y > 0.0 && c.y < 25.0
        })
        .collect();
    assert_eq!(sel.len(), 1);
    inset_faces(&mut m, &sel, &InsetOptions { thickness: 3.0, depth: -5.0, symmetry: Some(&map), ..Default::default() });
    check_closed(&m);
    assert!(sym(&m));
}

#[test]
fn more_coverage() {
    // Closed loop around a cylinder's side.
    let cyl = cylinder(50.0, 100.0, 16, 2, 0, true);
    let ring = (0..cyl.edge_capacity())
        .find(|&e| cyl.position(cyl.edge_point(e, 0)).y.abs() < 1e-9 && cyl.position(cyl.edge_point(e, 1)).y.abs() < 1e-9)
        .expect("ring");
    assert_eq!(edge_loop(&cyl, ring).len(), 16);

    // Extruding a whole open plane walls in its border and keeps the plane facing back at the base.
    let mut pl = plane(100.0, 100.0, 2, 2);
    let all = all_faces(&pl);
    let mut open = pl.clone();
    let mut eo = ExtrudeOptions { offset: 10.0, ..Default::default() };
    let r = extrude_faces(&mut pl, &all, &eo);
    assert!(valid(&pl));
    assert_eq!(r.sides.len(), 8);
    assert_eq!(r.back.len(), 4);
    assert_eq!(boundary_edge_count(&pl), 0);
    assert_eq!(pl.num_points(), 18);
    assert!(approx(pl.bounds().max.y, 10.0));
    assert!(approx(pl.bounds().min.y, 0.0));
    for &f in &r.back {
        assert!(approx(pl.face_normal(f).y, -1.0));
    }

    eo.keep_back = false;
    let r = extrude_faces(&mut open, &all, &eo);
    assert!(valid(&open));
    assert!(r.back.is_empty());
    assert_eq!(boundary_edge_count(&open), 8);

    // Part of an open plane meets the faces around it, so it has no back.
    let mut part = plane(100.0, 100.0, 2, 2);
    eo.keep_back = true;
    let r = extrude_faces(&mut part, &[0], &eo);
    assert!(valid(&part));
    assert!(r.back.is_empty());

    // Individual inset keeps faces apart.
    let mut grid = plane(100.0, 100.0, 2, 1);
    let r = inset_faces(&mut grid, &[0, 1], &InsetOptions { thickness: 5.0, individual: true, ..Default::default() });
    assert!(valid(&grid));
    assert_eq!(r.sides.len(), 8);
    assert!(approx(grid.face_area(0), 40.0 * 90.0));

    // Merge with symmetry welds both sides.
    let mut a = plane(100.0, 100.0, 2, 2);
    let mut b = a.clone();
    // A gap under the merge distance; coincident points would make mirror matches ambiguous.
    for p in b.positions_mut() {
        p.z -= 100.005;
    }
    a.append(&b);
    let map = SymmetryMap::build(&a, &SymmetryPlane::default(), 1e-3);
    assert_eq!(map.unmatched_count(), 0);
    let seam: Vec<Index> =
        (0..a.point_capacity()).filter(|&p| (a.position(p).z + 50.0).abs() < 0.01 && a.position(p).x > 1.0).collect();
    assert_eq!(seam.len(), 2);
    assert_eq!(map.expand_points(&seam).len(), 4);
    merge_by_distance(&mut a, &seam, &MergeOptions { distance: 0.01, symmetry: Some(&map), ..Default::default() });
    assert!(valid(&a));
    assert_eq!(a.num_points(), 18 - 2);
}

#[test]
#[ignore = "perf"]
fn perf_ops_on_large_meshes() {
    let mut m = plane(1000.0, 1000.0, 1000, 1000);
    let some: Vec<Index> = (0..m.face_capacity()).step_by(100).collect();
    let t0 = Instant::now();
    extrude_faces(&mut m, &some, &ExtrudeOptions { offset: 1.0, individual: true, ..Default::default() });
    let t_extrude = t0.elapsed().as_secs_f64();
    assert_eq!(m.num_faces(), 1_000_000 + 4 * 10000);

    let q = plane(1000.0, 1000.0, 500, 500);
    let t0 = Instant::now();
    let s = sub(&q, 1);
    let t_sub = t0.elapsed().as_secs_f64();
    assert_eq!(s.num_faces(), 1_000_000);

    let mut b = plane(1000.0, 1000.0, 300, 300);
    let edges: Vec<Index> = (0..b.edge_capacity()).step_by(7).filter(|&e| !b.is_boundary_edge(e)).collect();
    let t0 = Instant::now();
    bevel_edges(&mut b, &edges, &BevelOptions { offset: 0.5, ..Default::default() });
    let t_bevel = t0.elapsed().as_secs_f64();
    assert!(valid(&b));
    println!(
        "10k individual extrudes on 1M quads {t_extrude}s, subdivide 250k -> 1M quads {t_sub}s, bevel {} edges {t_bevel}s",
        edges.len()
    );
}
