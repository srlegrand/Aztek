//! Tests: radial symmetry.

mod common;

use aztek_geo::ops::*;
use aztek_geo::primitives::*;
use aztek_geo::symmetry::*;
use aztek_geo::{Index, Mesh, INVALID};
use common::*;
use glam::DVec3;
use rand_mt::Mt;

/// 24 columns: every count tested (3, 6, 8) divides it, and its vertex columns lie on every wedge and mirror plane.
fn sphere24() -> Mesh {
    make_sphere(&SphereOptions { subdiv_axis: 24, subdiv_height: 8, ..Default::default() })
}

const CASES: [(i32, bool); 4] = [(3, false), (6, false), (8, false), (6, true)];

fn group_of(c: (i32, bool)) -> RadialSymmetry {
    axis_radial(1, DVec3::ZERO, c.0, c.1, 0.0)
}

fn uniform(rng: &mut Mt, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * (rng.next_u32() as f64 / 4294967296.0)
}

/// Largest distance between a point's image under any element and that element applied to the point.
fn asymmetry(m: &Mesh, map: &SymmetryMap) -> f64 {
    let mut worst = 0.0f64;
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        for g in 0..map.order() {
            let q = map.image(g, p);
            if q != INVALID {
                worst = worst.max((m.position(q) - map.transform(g, m.position(p))).length());
            }
        }
    }
    worst
}

/// The face whose centre lies furthest along +x, so selections start off every mirror plane.
fn face_off_planes(m: &Mesh, r: &RadialSymmetry) -> Index {
    let mut best = INVALID;
    let mut best_score = -1.0;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) || m.face_size(f) != 4 {
            continue;
        }
        let mut c = DVec3::ZERO;
        for p in m.face_points(f) {
            c += m.position(p) * 0.25;
        }
        // Middle of the fundamental wedge, away from the poles.
        let a = r.angle(c);
        let mid = r.wedge_angle() * 0.5;
        let score = r.axis_distance(c) - 10.0 * (a - mid).abs();
        if score > best_score {
            best_score = score;
            best = f;
        }
    }
    best
}

#[test]
fn group_compose_inverse_and_apply_agree() {
    for c in CASES {
        let r = group_of(c);
        assert_eq!(r.order(), if c.1 { 2 } else { 1 } * c.0);
        let p = DVec3::new(13.0, -4.0, 7.5);
        for a in 0..r.order() {
            assert_eq!(r.compose(a, r.inverse(a)), 0);
            assert!((r.apply(r.inverse(a), r.apply(a, p)) - p).length() < 1e-9);
            for b in 0..r.order() {
                assert!((r.apply(r.compose(a, b), p) - r.apply(a, r.apply(b, p))).length() < 1e-9);
            }
        }
        // Each element's wedge holds its image of a point in the fundamental wedge.
        let inside = r.wedge_angle() * 0.3;
        let q = 20.0 * (inside.cos() * r.reference() + inside.sin() * r.axis().cross(r.reference()));
        assert_eq!(r.wedge_of(q), 0);
        for g in 0..r.order() {
            assert_eq!(r.wedge_of(r.apply(g, q)), g);
        }
    }
    // Quarter turns about a coordinate axis are exact.
    let four = axis_radial(1, DVec3::ZERO, 4, false, 0.0);
    assert_eq!(four.apply(1, DVec3::new(0.1, 0.3, 0.7)), DVec3::new(0.7, 0.3, -0.1));
}

#[test]
fn map_by_position_orbits_axis_points_and_mirror_plane_points() {
    let m = sphere24();
    for c in CASES {
        let r = group_of(c);
        let map = SymmetryMap::build_radial(&m, &r, 1e-6);
        assert!(map.is_radial());
        assert_eq!(map.order(), r.order());
        assert_eq!(map.unmatched_count(), 0);
        assert!(asymmetry(&m, &map) < 1e-9);
        let mut on_axis = 0;
        let mut on_mirror = 0;
        for p in 0..m.point_capacity() {
            if !m.point_valid(p) {
                continue;
            }
            if r.axis_distance(m.position(p)) < 1e-9 {
                on_axis += 1;
                assert!(map.fixed(p));
                assert_eq!(map.wedge(p), -1);
                for g in 0..map.order() {
                    assert_eq!(map.image(g, p), p);
                }
            } else if map.fixed(p) {
                on_mirror += 1;
            } else {
                // Every member of an orbit is a different point and lies in its own slot's wedge.
                let mut orbit: Vec<Index> = (0..map.order()).map(|g| map.orbit_point(p, g)).collect();
                orbit.sort_unstable();
                assert!(orbit.windows(2).all(|w| w[0] != w[1]));
                assert_eq!(map.orbit_point(p, map.wedge(p)), p);
            }
        }
        assert_eq!(on_axis, 2);
        // Dihedral: the vertex columns on the 6 mirror planes are fixed by a reflection.
        assert_eq!(on_mirror > 0, c.1);
        let check = map.check(&m);
        assert_eq!(check.unmatched, 0);
        assert!(check.max_asymmetry < 1e-9);
    }
}

#[test]
fn a_count_that_does_not_fit_leaves_points_unmatched() {
    let m = sphere24();
    let map = SymmetryMap::build_radial(&m, &axis_radial(1, DVec3::ZERO, 5, false, 0.0), 1e-6);
    assert!(map.unmatched_count() > 0);
    assert_eq!(map.unmatched_points(&m).len() as Index, map.unmatched_count());
    // The poles still match: they sit on the axis whatever the count.
    assert_eq!(map.unmatched_count(), m.num_points() - 2);
}

#[test]
fn map_by_topology_survives_drift() {
    for c in CASES {
        let mut m = sphere24();
        let r = group_of(c);
        let exact = SymmetryMap::build_radial(&m, &r, 1e-6);
        let mut rng = Mt::new(7);
        for p in m.positions_mut() {
            *p += DVec3::new(uniform(&mut rng, -1.5, 1.5), uniform(&mut rng, -1.5, 1.5), uniform(&mut rng, -1.5, 1.5));
        }
        assert!(SymmetryMap::build_radial(&m, &r, 1e-3).unmatched_count() > 0);
        let topo = SymmetryMap::build_radial_topological(&m, 5, &r);
        assert!(!topo.is_empty());
        assert_eq!(topo.unmatched_count(), 0);
        for p in 0..m.point_capacity() {
            for g in 0..r.order() {
                assert_eq!(topo.image(g, p), exact.image(g, p));
            }
        }
    }
}

#[test]
fn persistent_layer_round_trip() {
    for c in CASES {
        let mut m = sphere24();
        let r = group_of(c);
        let mut sync = SymmetrySync::default();
        let first = SymmetryMap::persistent_radial(&mut m, &r, 1e-4, Some(&mut sync));
        assert_eq!(sync.unmatched, 0);
        assert_eq!(sync.by_position, m.num_points());
        assert!(m.point_attrs().has(&radial_layer_name(&r)));
        assert!(is_symmetry_layer(&radial_layer_name(&r)));

        // Far past any matching tolerance: the stored orbits still hold.
        let mut rng = Mt::new(11);
        for p in m.positions_mut() {
            *p += DVec3::new(uniform(&mut rng, -5.0, 5.0), uniform(&mut rng, -5.0, 5.0), uniform(&mut rng, -5.0, 5.0));
        }
        let again = SymmetryMap::persistent_radial(&mut m, &r, 1e-4, Some(&mut sync));
        assert_eq!(sync.stored, m.num_points());
        assert_eq!(sync.by_position + sync.topological, 0);
        for p in 0..m.point_capacity() {
            for g in 0..r.order() {
                assert_eq!(again.image(g, p), first.image(g, p));
            }
        }

        // Delete one whole orbit's faces and renumber: the rest keep their orbits.
        let f = face_off_planes(&m, &r);
        delete_faces(&mut m, &[f], Some(&again));
        m.compact(None);
        let after = SymmetryMap::persistent_radial(&mut m, &r, 1e-4, Some(&mut sync));
        assert_eq!(sync.unmatched, 0);
        assert_eq!(sync.by_position + sync.topological, 0);
        assert_eq!(m.num_faces(), sphere24().num_faces() - r.order() as Index);

        // Enforcing from a wedge makes every orbit exact again.
        after.symmetrize_wedge(&mut m, 0);
        assert!(asymmetry(&m, &after) < 1e-9);
        assert!(after.check(&m).max_asymmetry < 1e-9);
    }
}

#[test]
fn persistent_new_points_from_an_op_are_matched_by_topology() {
    for c in CASES {
        let mut m = sphere24();
        let r = group_of(c);
        let map = SymmetryMap::persistent_radial(&mut m, &r, 1e-4, None);
        // Drift so position matching cannot help, then extrude: the new points must still find their orbits.
        for p in m.positions_mut() {
            *p *= 1.0 + 0.01 * p.y;
        }
        let first_new = m.point_capacity();
        let f = face_off_planes(&m, &r);
        extrude_faces(&mut m, &[f], &ExtrudeOptions { offset: 4.0, symmetry: Some(&map), ..Default::default() });
        clear_symmetry_keys(&mut m, first_new);
        let mut sync = SymmetrySync::default();
        SymmetryMap::persistent_radial(&mut m, &r, 1e-4, Some(&mut sync));
        assert_eq!(sync.unmatched, 0);
        assert_eq!(sync.topological, m.point_capacity() - first_new);
    }
}

#[test]
fn geo_ops_follow_radial_symmetry() {
    for c in CASES {
        let r = group_of(c);
        let check = |op: &str, m: &Mesh| {
            assert!(valid(m), "{op}");
            let after = SymmetryMap::build_radial(m, &r, 1e-6);
            assert_eq!(after.unmatched_count(), 0, "{op} {c:?}");
        };
        {
            let mut m = sphere24();
            let map = SymmetryMap::build_radial(&m, &r, 1e-6);
            let faces = m.num_faces();
            let f = face_off_planes(&m, &r);
            let res = extrude_faces(&mut m, &[f], &ExtrudeOptions { offset: 5.0, individual: true, symmetry: Some(&map), ..Default::default() });
            assert_eq!(res.faces.len(), r.order() as usize);
            assert_eq!(m.num_faces(), faces + 4 * r.order() as Index);
            check("extrude", &m);
        }
        {
            let mut m = sphere24();
            let map = SymmetryMap::build_radial(&m, &r, 1e-6);
            let f = face_off_planes(&m, &r);
            inset_faces(&mut m, &[f], &InsetOptions { thickness: 1.0, symmetry: Some(&map), ..Default::default() });
            check("inset", &m);
        }
        {
            let mut m = sphere24();
            let map = SymmetryMap::build_radial(&m, &r, 1e-6);
            let f = face_off_planes(&m, &r);
            let e = m.corner_edge(m.face_corner(f));
            bevel_edges(&mut m, &[e], &BevelOptions { offset: 1.0, symmetry: Some(&map), ..Default::default() });
            check("bevel", &m);
        }
        {
            let mut m = sphere24();
            let map = SymmetryMap::build_radial(&m, &r, 1e-6);
            let faces = m.num_faces();
            let f = face_off_planes(&m, &r);
            delete_faces(&mut m, &[f], Some(&map));
            assert_eq!(m.num_faces(), faces - r.order() as Index);
            m.compact(None);
            check("delete", &m);
        }
        {
            let mut m = sphere24();
            let map = SymmetryMap::build_radial(&m, &r, 1e-6);
            let f = face_off_planes(&m, &r);
            delete_faces(&mut m, &[f], Some(&map));
            m.compact(None);
            let holes = SymmetryMap::build_radial(&m, &r, 1e-6);
            let border: Vec<Index> = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.is_boundary_edge(e)).take(1).collect();
            extrude_edges(&mut m, &border, &EdgeExtrudeOptions { offset: 2.0, symmetry: Some(&holes), ..Default::default() });
            check("edge extrude", &m);
        }
        {
            let mut m = sphere24();
            let map = SymmetryMap::build_radial(&m, &r, 1e-6);
            let f = face_off_planes(&m, &r);
            let e = m.corner_edge(m.face_corner(f));
            let plan = knife_loop_plan(&m, e, 0.3, Some(&map));
            assert!(!plan.segments.is_empty());
            knife_apply(&mut m, &plan);
            check("knife loop", &m);
        }
    }
}

#[test]
fn enforce_wedge_carries_one_wedges_edit_onto_every_slot() {
    for c in CASES {
        let mut m = sphere24();
        let r = group_of(c);
        let map = SymmetryMap::build_radial(&m, &r, 1e-6);
        let f = face_off_planes(&m, &r);
        let pts = m.face_points(f);
        let w = map.majority_wedge(&pts);
        for &p in &pts {
            if map.drives(p, w) {
                let v = m.position(p) * 1.3 + DVec3::new(0.0, 2.0, 0.0);
                m.set_position(p, v);
            }
        }
        let all = map.expand_points(&pts);
        assert!(all.len() >= pts.len() * c.0 as usize);
        map.enforce_wedge(&mut m, &all, w);
        assert!(asymmetry(&m, &map) < 1e-9);
        // The poles stay exactly on the axis.
        for p in 0..m.point_capacity() {
            if map.fixed(p) && r.axis_distance(m.position(p)) < 1e-6 {
                assert_eq!(r.axis_distance(m.position(p)), 0.0);
            }
        }
    }
}

#[test]
fn symmetrize_radial_topology_rebuilds_the_kept_wedge_exactly() {
    for c in CASES {
        let r = group_of(c);
        let mut m = sphere24();
        let points = m.num_points();
        let faces = m.num_faces();
        // Spoil every wedge but one: the rebuild must come out of the kept wedge alone.
        for p in 0..m.point_capacity() {
            if r.wedge_of(m.position(p)) != 0 && r.axis_distance(m.position(p)) > 1.0 {
                let v = m.position(p) + DVec3::new(0.0, 3.0, 0.0);
                m.set_position(p, v);
            }
        }
        let map = symmetrize_radial_topology(&mut m, &r, 0, 1e-4);
        assert!(valid(&m));
        assert_eq!(boundary_edge_count(&m), 0);
        assert_eq!(m.num_points(), points);
        assert_eq!(m.num_faces(), faces);
        assert_eq!(map.unmatched_count(), 0);
        assert!(asymmetry(&m, &map) < 1e-9);
        assert!(m.point_attrs().has(&radial_layer_name(&r)));
    }
    // A count that cuts across the columns makes new seam points, still exactly symmetric and closed.
    let mut m = sphere24();
    let five = axis_radial(1, DVec3::ZERO, 5, false, 0.0);
    let map = symmetrize_radial_topology(&mut m, &five, 0, 1e-4);
    assert!(valid(&m));
    assert_eq!(boundary_edge_count(&m), 0);
    assert_eq!(map.unmatched_count(), 0);
    assert!(asymmetry(&m, &map) < 1e-9);
}

#[test]
fn mirror_maps_answer_the_generic_calls_as_the_order_2_case() {
    let m = make_cube(&CubeOptions { subdiv_x: 3, ..Default::default() });
    let map = SymmetryMap::build(&m, &axis_plane(0, 0.0), 1e-6);
    assert!(!map.is_radial());
    assert_eq!(map.order(), 2);
    for p in 0..m.point_capacity() {
        assert_eq!(map.image(0, p), p);
        assert_eq!(map.image(1, p), map.mirror_point(p));
        assert_eq!(map.fixed(p), map.on_plane(p));
        if map.side(p) > 0 {
            assert_eq!(map.wedge(p), 0);
        }
        if map.side(p) < 0 {
            assert_eq!(map.wedge(p), 1);
        }
        assert_eq!(map.orbit_point(p, 0), if map.side(p) < 0 { map.mirror_point(p) } else { p });
    }
    assert!(asymmetry(&m, &map) < 1e-12);
}
