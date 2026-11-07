//! Tests: bevel.

mod common;

use std::cell::Cell;

use aztek_geo::fold_edges::*;
use aztek_geo::ops::*;
use aztek_geo::primitives::*;
use aztek_geo::symmetry::{SymmetryMap, SymmetryPlane};
use aztek_geo::triangulate::{triangulate, TriangulateOptions};
use aztek_geo::{validate, Bounds, Index, Mesh, INVALID};
use common::*;
use glam::{DVec2, DVec3};

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

fn all_edges(m: &Mesh) -> Vec<Index> {
    (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)).collect()
}

/// Segment pq crosses the triangle's interior, ignoring touches within tol of its border or the segment ends.
fn segment_hits_triangle(p: DVec3, q: DVec3, a: DVec3, b: DVec3, c: DVec3, tol: f64) -> bool {
    let d = q - p;
    let e1 = b - a;
    let e2 = c - a;
    let h = d.cross(e2);
    let det = e1.dot(h);
    if det.abs() < 1e-12 * d.dot(d) * e1.dot(e1) {
        return false;
    }
    let inv = 1.0 / det;
    let s = p - a;
    let u = s.dot(h) * inv;
    let qv = s.cross(e1);
    let v = d.dot(qv) * inv;
    let t = e2.dot(qv) * inv;
    u > tol && v > tol && u + v < 1.0 - tol && t > tol && t < 1.0 - tol
}

/// Pairs of triangles that pierce each other, skipping pairs that share an edge.
fn self_intersections(m: &Mesh) -> i32 {
    let tri = triangulate(m, &TriangulateOptions::default());
    let n = tri.triangle_count() as usize;
    let mut pts = vec![[0 as Index; 3]; n];
    let mut bx = vec![Bounds::default(); n];
    let mut size = 0.0f64;
    for t in 0..n {
        for k in 0..3 {
            pts[t][k] = m.corner_point(tri.corners[3 * t + k]);
            bx[t].extend(m.position(pts[t][k]));
        }
        size = size.max(bx[t].size().length());
    }
    let pad = 1e-6 * size;
    let mut hits = 0;
    for i in 0..n {
        for j in i + 1..n {
            if bx[i].min.cmpgt(bx[j].max + pad).any() || bx[j].min.cmpgt(bx[i].max + pad).any() {
                continue;
            }
            let shared: usize = pts[i].iter().map(|a| pts[j].iter().filter(|b| *b == a).count()).sum();
            if shared >= 2 {
                continue;
            }
            let test = |ti: usize, tj: usize| {
                let (a, b) = (&pts[ti], &pts[tj]);
                (0..3).any(|k| {
                    let p = a[k];
                    let q = a[(k + 1) % 3];
                    if b.contains(&p) || b.contains(&q) {
                        return false;
                    }
                    segment_hits_triangle(m.position(p), m.position(q), m.position(b[0]), m.position(b[1]), m.position(b[2]), 1e-7)
                })
            };
            if test(i, j) || test(j, i) {
                if hits < 5 {
                    eprintln!("triangles of faces {} and {} intersect", tri.faces[i], tri.faces[j]);
                }
                hits += 1;
            }
        }
    }
    hits
}

/// Largest turn between neighbouring face normals, degrees.
fn max_fold(m: &Mesh) -> f64 {
    let mut worst = 0.0f64;
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) && m.is_manifold_edge(e) {
            let c = m.edge_corner(e);
            let d = m.face_normal(m.corner_face(c)).dot(m.face_normal(m.corner_face(m.corner_radial(c))));
            worst = worst.max(d.clamp(-1.0, 1.0).acos().to_degrees());
        }
    }
    worst
}

fn small_faces(m: &Mesh, min_area: f64) -> Index {
    let mut n = 0;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) && m.face_area(f) < min_area {
            if n < 5 {
                eprintln!("face {f} of {} corners has area {}", m.face_size(f), m.face_area(f));
            }
            n += 1;
        }
    }
    n
}

/// Every point lies behind every other face's plane: the solid is convex and all faces point outward.
fn convex_outward(m: &Mesh, tol: f64) -> bool {
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let n = m.face_normal(f);
        let c = m.face_centroid(f);
        let own = m.face_points(f);
        for p in 0..m.point_capacity() {
            if m.point_valid(p) && (m.position(p) - c).dot(n) > tol && !own.contains(&p) {
                eprintln!("point {p} is {} in front of face {f}", (m.position(p) - c).dot(n));
                return false;
            }
        }
    }
    true
}

fn uvs_finite(m: &Mesh) -> bool {
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0");
    (0..m.corner_capacity()).all(|c| !m.corner_valid(c) || (uv[c as usize][0].is_finite() && uv[c as usize][1].is_finite()))
}

/// The checks every bevel result must pass: valid, manifold, consistently wound, no tiny faces, no overlaps.
fn check_sound(m: &Mesh, offset: f64, closed: bool) {
    assert!(is_valid(m, validate::DEFAULT | validate::DEGENERATE));
    if closed {
        assert_eq!(boundary_edge_count(m), 0);
        assert!(signed_volume(m) > 0.0);
    }
    assert_eq!(small_faces(m, 1e-4 * offset * offset), 0);
    assert_eq!(self_intersections(m), 0);
    assert!(max_fold(m) < 135.0, "max fold {}", max_fold(m));
}

fn check_closed_sound(m: &Mesh, offset: f64) {
    check_sound(m, offset, true);
    assert_eq!(euler(m), 2);
}

/// Faces a corner of three bevelled edges turns into: Blender's quad grid, with a centre triangle for odd counts.
fn corner_faces(seg: i32) -> Index {
    let h = (seg / 2) as Index;
    if seg % 2 == 1 {
        3 * h * (h + 1) + 1
    } else {
        3 * h * h
    }
}

fn l_bracket() -> Mesh {
    let profile = [DVec2::new(0.0, 0.0), DVec2::new(10.0, 0.0), DVec2::new(10.0, 1.0), DVec2::new(1.0, 1.0), DVec2::new(1.0, 10.0), DVec2::new(0.0, 10.0)];
    let mut pts = Vec::new();
    for z in [0.0, 10.0] {
        for p in &profile {
            pts.push(DVec3::new(p.x, p.y, z));
        }
    }
    let mut sizes = vec![6];
    let mut idx: Vec<Index> = (0..6).rev().collect();
    sizes.push(6);
    idx.extend(6..12);
    for i in 0..6 {
        let j = (i + 1) % 6;
        sizes.push(4);
        idx.extend([i, j, 6 + j, 6 + i]);
    }
    Mesh::from_polygons(&pts, &sizes, &idx)
}

/// Ground grid 7 x 4 with two 2 x 2 blocks raised 1, one quad apart, as in the auto-bevel tests.
fn raised_blocks() -> Mesh {
    let (w, h) = (7, 4);
    let mut pts = Vec::new();
    for y in 0..=h {
        for x in 0..=w {
            pts.push(DVec3::new(x as f64, y as f64, 0.0));
        }
    }
    let mut sizes = Vec::new();
    let mut idx = Vec::new();
    for y in 0..h {
        for x in 0..w {
            sizes.push(4);
            for k in [y * (w + 1) + x, y * (w + 1) + x + 1, (y + 1) * (w + 1) + x + 1, (y + 1) * (w + 1) + x] {
                idx.push(k as Index);
            }
        }
    }
    let mut m = Mesh::from_polygons(&pts, &sizes, &idx);
    let mut blocks = Vec::new();
    for y in [1, 2] {
        for x in [1, 2, 4, 5] {
            blocks.push((y * w + x) as Index);
        }
    }
    extrude_faces(&mut m, &blocks, &ExtrudeOptions { offset: 1.0, ..Default::default() });
    m.compact(None);
    m
}

fn random_shapes() -> Vec<Mesh> {
    let mut out = vec![
        make_cube(&CubeOptions { subdiv_x: 3, subdiv_y: 2, subdiv_z: 2, ..Default::default() }),
        make_cylinder(&CylinderOptions { subdiv_axis: 12, subdiv_height: 2, ..Default::default() }),
        make_cone(&ConeOptions { subdiv_axis: 10, ..Default::default() }),
        make_sphere(&SphereOptions { subdiv_axis: 10, subdiv_height: 8, ..Default::default() }),
        make_torus(&TorusOptions { subdiv_axis: 12, subdiv_height: 8, ..Default::default() }),
        make_ico_sphere(&IcoSphereOptions { subdivisions: 2, ..Default::default() }),
        l_bracket(),
        raised_blocks(),
        make_cylinder(&CylinderOptions { subdiv_axis: 6, subdiv_caps: 2, ..Default::default() }),
    ];
    let mut tri = make_cube(&CubeOptions { subdiv_x: 2, subdiv_y: 2, subdiv_z: 2, ..Default::default() });
    triangulate_faces(&mut tri, &[], &TriangulateOptions::default());
    let all: Vec<Index> = (0..tri.face_capacity()).filter(|&f| tri.face_valid(f)).collect();
    triangulate_faces(&mut tri, &all, &TriangulateOptions::default());
    tri.compact(None);
    out.push(tri);
    out
}

const CORNER: DVec3 = DVec3::new(50.0, 50.0, 50.0);

fn cube() -> Mesh {
    make_cube(&CubeOptions::default())
}

fn bevel(m: &mut Mesh, edges: &[Index], offset: f64, segments: i32) -> Vec<Index> {
    bevel_edges(m, edges, &BevelOptions { offset, segments, ..Default::default() })
}

#[test]
fn cube_every_edge_1_to_4_segments() {
    for seg in 1..=4 {
        let mut m = cube();
        let edges = all_edges(&m);
        let made = bevel(&mut m, &edges, 10.0, seg);
        check_closed_sound(&m, 10.0);
        assert_eq!(m.num_faces(), 6 + 12 * seg as Index + 8 * corner_faces(seg));
        assert_eq!(made.len(), 12 * seg as usize + 8 * corner_faces(seg) as usize);
        assert!(convex_outward(&m, 1e-6));
        assert!(uvs_finite(&m));
        if seg >= 2 {
            // Round profile: strips lie on cylinders and corners on spheres of radius offset.
            for p in 0..m.point_capacity() {
                if !m.point_valid(p) {
                    continue;
                }
                let q = m.position(p);
                let inner = q.clamp(DVec3::splat(-40.0), DVec3::splat(40.0));
                assert!(approx_eps(q.distance(inner), 10.0, 1e-9));
            }
            // All quads, plus one centre triangle per corner for odd counts.
            let tris = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_size(f) == 3).count();
            assert_eq!(tris, if seg % 2 == 1 { 8 } else { 0 });
        }
    }
}

#[test]
fn profiles_stay_consistent_across_the_corners() {
    for profile in [0.25, 0.7, 0.95] {
        for seg in [2, 3] {
            let mut m = cube();
            let edges = all_edges(&m);
            bevel_edges(&mut m, &edges, &BevelOptions { offset: 10.0, segments: seg, profile, ..Default::default() });
            check_closed_sound(&m, 10.0);
            // Near-square profiles bend corner quads sharply, so neighbours may sit a little above their mean plane.
            assert!(convex_outward(&m, if profile > 0.9 { 0.02 * 10.0 } else { 1e-6 }));
            if profile == 0.25 {
                // A straight profile chamfers each corner with a flat triangle: every corner point on x + y + z = 130.
                for p in 0..m.point_capacity() {
                    let q = m.position(p);
                    if m.point_valid(p) && q.x > 40.0 && q.y > 40.0 && q.z > 40.0 {
                        assert!(approx(q.x + q.y + q.z, 130.0));
                    }
                }
            }
        }
    }
}

#[test]
fn three_edges_at_one_corner() {
    for seg in 1..=4 {
        let mut m = cube();
        let edges = [
            edge_between(&m, CORNER, DVec3::new(50.0, 50.0, -50.0)),
            edge_between(&m, CORNER, DVec3::new(-50.0, 50.0, 50.0)),
            edge_between(&m, CORNER, DVec3::new(50.0, -50.0, 50.0)),
        ];
        bevel(&mut m, &edges, 10.0, seg);
        check_closed_sound(&m, 10.0);
        assert_eq!(m.num_faces(), 6 + 3 * seg as Index + corner_faces(seg));
        assert!(convex_outward(&m, 1e-6));
        // The far ends stop on the neighbouring cube corners, which stay where they were.
        for c in [DVec3::new(50.0, 50.0, -50.0), DVec3::new(-50.0, 50.0, 50.0), DVec3::new(50.0, -50.0, 50.0)] {
            let found = (0..m.point_capacity()).any(|p| m.point_valid(p) && m.position(p).distance(c) < 1e-9);
            assert!(!found);
        }
    }
}

#[test]
fn two_edges_meeting_at_a_corner_miter_cleanly() {
    for seg in 1..=4 {
        let mut m = cube();
        let edges = [edge_between(&m, CORNER, DVec3::new(50.0, 50.0, -50.0)), edge_between(&m, CORNER, DVec3::new(-50.0, 50.0, 50.0))];
        bevel(&mut m, &edges, 10.0, seg);
        check_closed_sound(&m, 10.0);
        // The two rounded strips meet along their miter: no patch between them.
        assert_eq!(m.num_faces(), 6 + 2 * seg as Index);
        assert!(convex_outward(&m, 1e-6));
        // Exact round fillets: every point of the strips sits on its cylinder.
        for p in 0..m.point_capacity() {
            let q = m.position(p);
            if !m.point_valid(p) || q.y <= 40.0 + 1e-9 {
                continue;
            }
            if q.x > 40.0 && q.z > 40.0 {
                let d = (q.x - 40.0).hypot(q.y - 40.0).min((q.y - 40.0).hypot(q.z - 40.0));
                assert!(approx(d, 10.0));
            }
        }
    }
}

#[test]
fn single_edge_ending_at_cube_corners() {
    for seg in 1..=4 {
        let mut m = cube();
        let e = edge_between(&m, CORNER, DVec3::new(50.0, 50.0, -50.0));
        let made = bevel(&mut m, &[e], 10.0, seg);
        check_closed_sound(&m, 10.0);
        assert_eq!(made.len(), seg as usize);
        assert_eq!(m.num_faces(), 6 + seg as Index);
        assert!(convex_outward(&m, 1e-6));
        // The end caps take the profile and stay flat.
        for f in 0..m.face_capacity() {
            if m.face_valid(f) && m.face_normal(f).z.abs() > 0.999 {
                for p in m.face_points(f) {
                    assert!(approx(m.position(p).z.abs(), 50.0));
                }
            }
        }
    }
}

#[test]
fn terminal_and_l_corners_on_valence_four_points() {
    // A 2x2x2 cube: edge midpoints have four edges, face centres four flat ones.
    let cube2 = || make_cube(&CubeOptions { subdiv_x: 2, subdiv_y: 2, subdiv_z: 2, ..Default::default() });
    for seg in 1..=4 {
        let off = 8.0;
        // One edge from a face centre to an edge midpoint, in the flat top face.
        let mut one = cube2();
        let e = edge_between(&one, DVec3::new(0.0, 50.0, 0.0), DVec3::new(50.0, 50.0, 0.0));
        bevel(&mut one, &[e], off, seg);
        check_closed_sound(&one, off);
        assert!(convex_outward(&one, 1e-6));

        // Two edges making an L in the flat top face.
        let mut ell = cube2();
        let l = [
            edge_between(&ell, DVec3::new(0.0, 50.0, 0.0), DVec3::new(50.0, 50.0, 0.0)),
            edge_between(&ell, DVec3::new(0.0, 50.0, 0.0), DVec3::new(0.0, 50.0, 50.0)),
        ];
        bevel(&mut ell, &l, off, seg);
        check_closed_sound(&ell, off);
        assert!(convex_outward(&ell, 1e-6));
        // Everything that was in the top face stays in it.
        for p in 0..ell.point_capacity() {
            if ell.point_valid(p) && ell.position(p).y > 49.0 {
                assert!(approx(ell.position(p).y, 50.0));
            }
        }

        // A whole cube edge, both halves, with a T: one top-face edge leaves its midpoint.
        let mut tee = cube2();
        let t = [
            edge_between(&tee, DVec3::new(50.0, 50.0, 50.0), DVec3::new(50.0, 50.0, 0.0)),
            edge_between(&tee, DVec3::new(50.0, 50.0, 0.0), DVec3::new(50.0, 50.0, -50.0)),
            edge_between(&tee, DVec3::new(50.0, 50.0, 0.0), DVec3::new(0.0, 50.0, 0.0)),
        ];
        bevel(&mut tee, &t, off, seg);
        check_closed_sound(&tee, off);
        // The long edge stays round through the junction: everything past the fillet's axis lies on its cylinder.
        for p in 0..tee.point_capacity() {
            let q = tee.position(p);
            assert!(q.abs().cmple(DVec3::splat(50.0 + 1e-9)).all());
            if tee.point_valid(p) && q.x > 42.0 + 1e-9 && q.y > 42.0 + 1e-9 {
                assert!(approx((q.x - 42.0).hypot(q.y - 42.0), 8.0));
            }
        }

        // Every edge: valence four points get four-sided corner grids.
        let mut all = cube2();
        let edges = all_edges(&all);
        bevel(&mut all, &edges, off, seg);
        check_closed_sound(&all, off);
        assert!(convex_outward(&all, 0.01 * off));
    }
}

#[test]
fn l_bracket_folds_concave_edges_and_reflex_corners() {
    for seg in 1..=4 {
        // The outer and inner fold.
        let mut m = l_bracket();
        let folds = [
            edge_between(&m, DVec3::new(0.0, 0.0, 0.0), DVec3::new(0.0, 0.0, 10.0)),
            edge_between(&m, DVec3::new(1.0, 1.0, 0.0), DVec3::new(1.0, 1.0, 10.0)),
        ];
        let before = signed_volume(&m);
        bevel(&mut m, &folds, 0.3, seg);
        check_closed_sound(&m, 0.3);
        assert_eq!(m.num_faces(), 8 + 2 * seg as Index);
        // The outer fold loses what the inner gains when both are round.
        if seg >= 3 {
            assert!(approx_eps(signed_volume(&m), before, 1e-3));
        }
        // Caps stay flat.
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                let z = m.position(p).z;
                assert!(z.abs() < 1e-9 || (z - 10.0).abs() < 1e-9);
            }
        }

        // Every edge: concave folds, reflex cap corners and three-edge corners mixing both.
        let mut all = l_bracket();
        let edges = all_edges(&all);
        bevel(&mut all, &edges, 0.3, seg);
        check_closed_sound(&all, 0.3);
    }
}

#[test]
fn auto_bevel_fold_corners_on_raised_blocks() {
    for seg in 1..=3 {
        let mut m = raised_blocks();
        let mut o = FoldEdgeOptions { complete_corners: true, ..Default::default() };
        let folds = fold_edges(&m, &o);
        assert_eq!(folds.len(), 42);
        bevel(&mut m, &folds, 0.2, seg);
        check_sound(&m, 0.2, false);
        // The open border is untouched.
        assert_eq!(boundary_edge_count(&m), 22);

        let mut plain = raised_blocks();
        o.complete_corners = false;
        let folds = fold_edges(&plain, &o);
        bevel(&mut plain, &folds, 0.2, seg);
        check_sound(&plain, 0.2, false);
    }
}

#[test]
fn cylinder_rim() {
    let cyl = || make_cylinder(&CylinderOptions { radius: 50.0, height: 100.0, subdiv_axis: 20, cap_fill: CapFill::Ngon, ..Default::default() });
    for seg in 1..=4 {
        let mut m = cyl();
        let b = m.bounds();
        let rim: Vec<Index> = all_edges(&m)
            .into_iter()
            .filter(|&e| {
                let p = m.position(m.edge_point(e, 0));
                let q = m.position(m.edge_point(e, 1));
                (p.y - b.max.y).abs() < 1e-9 && (q.y - b.max.y).abs() < 1e-9
            })
            .collect();
        assert_eq!(rim.len(), 20);
        let before = m.num_faces();
        bevel(&mut m, &rim, 5.0, seg);
        check_closed_sound(&m, 5.0);
        assert_eq!(m.num_faces(), before + 20 * seg as Index);
        assert!(convex_outward(&m, 1e-6));

        // Every edge: n-gon caps meet the sides at corners that are not square.
        let mut all = cyl();
        let edges = all_edges(&all);
        bevel(&mut all, &edges, 5.0, seg);
        check_closed_sound(&all, 5.0);
        assert!(convex_outward(&all, 0.02 * 5.0));
    }
}

#[test]
fn pocket_with_concave_three_edge_corners() {
    for seg in 1..=4 {
        let mut m = cube();
        let top = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_normal(f).y > 0.999).last().unwrap();
        inset_faces(&mut m, &[top], &InsetOptions { thickness: 20.0, depth: -30.0, ..Default::default() });
        m.compact(None);
        assert!(valid(&m));
        let before = signed_volume(&m);
        let edges = all_edges(&m);
        bevel(&mut m, &edges, 5.0, seg);
        check_closed_sound(&m, 5.0);
        // Rounding the pocket's inside corners adds material, rounding the outside takes it away.
        assert!(signed_volume(&m) < before);
    }
}

#[test]
fn clamp_keeps_neighbours_from_crossing() {
    for seg in [1, 3] {
        let mut m = make_cube(&CubeOptions { width: 100.0, height: 20.0, depth: 100.0, ..Default::default() });
        let edges = all_edges(&m);
        bevel(&mut m, &edges, 40.0, seg);
        check_closed_sound(&m, 1.0);
        assert!(convex_outward(&m, 1e-6));
        assert!(approx(m.bounds().size().y, 20.0));

        let mut l = l_bracket();
        let edges = all_edges(&l);
        bevel(&mut l, &edges, 5.0, seg);
        check_closed_sound(&l, 0.05);
    }
}

#[test]
fn edges_between_faces_folded_flat_onto_each_other() {
    // An unclamped inset of a 4 wide face by 7 crosses its rim over itself: the short rim faces become bowties lying
    // back to back with the long ones.
    let mut m = make_cube(&CubeOptions { width: 100.0, height: 100.0, depth: 4.0, ..Default::default() });
    let side = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_normal(f).x > 0.5).last().unwrap();
    let inset = inset_faces(&mut m, &[side], &InsetOptions { thickness: 7.0, individual: true, clamp: false, ..Default::default() });
    extrude_faces(&mut m, &inset.faces, &ExtrudeOptions { offset: -2.5, ..Default::default() });
    assert!(max_fold(&m) > 179.9);

    let edges = all_edges(&m);
    bevel_edges(&mut m, &edges, &BevelOptions { offset: 1.2, ..Default::default() });
    assert!(valid(&m));
    assert_eq!(boundary_edge_count(&m), 0);
    // Heights under 0.01 over the longest edge, as meshCheck reports zero-area faces.
    let mut slivers = 0;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            let longest = m.face_corners(f).iter().map(|&c| m.edge_length(m.corner_edge(c))).fold(0.0, f64::max);
            if m.face_area(f) <= 0.01 * longest {
                eprintln!("face {f} of {} corners has area {}", m.face_size(f), m.face_area(f));
                slivers += 1;
            }
        }
    }
    assert_eq!(slivers, 0);
}

#[test]
fn clamp_closes_faces_that_run_out_of_room() {
    // Shortest edge over the mesh size, so slivers show however small they are.
    let shortest = |m: &Mesh| {
        let s = all_edges(m).iter().map(|&e| m.edge_length(e)).fold(1e300, f64::min);
        s / m.bounds().size().length()
    };
    for seg in [1, 3] {
        // The 20 high sides close to a line where the top and bottom bevels meet.
        let mut m = make_cube(&CubeOptions { width: 100.0, height: 20.0, depth: 100.0, ..Default::default() });
        let applied = Cell::new(0.0);
        let edges = all_edges(&m);
        bevel_edges(&mut m, &edges, &BevelOptions { offset: 40.0, segments: seg, applied_offset: Some(&applied), ..Default::default() });
        assert!(approx(applied.get(), 10.0));
        check_closed_sound(&m, 10.0);
        assert!(shortest(&m) > 1e-3);
        assert!(convex_outward(&m, 1e-6));

        // A narrow side inset to its medial line leaves triangles too small for the offset: they close to points.
        let mut w = make_cube(&CubeOptions { width: 100.0, height: 100.0, depth: 2.5, ..Default::default() });
        let sel: Vec<Index> = (0..w.face_capacity()).filter(|&f| w.face_valid(f) && w.face_normal(f).x.abs() > 0.5).collect();
        inset_faces(&mut w, &sel, &InsetOptions { thickness: 7.0, individual: true, ..Default::default() });
        let edges = all_edges(&w);
        bevel(&mut w, &edges, 1.2, seg);
        check_closed_sound(&w, 0.1);
        assert!(shortest(&w) > 1e-4);
    }

    // With room to spare the clamp changes nothing.
    let mut a = cube();
    let mut b = cube();
    let ea = all_edges(&a);
    let eb = all_edges(&b);
    bevel(&mut a, &ea, 10.0, 2);
    bevel_edges(&mut b, &eb, &BevelOptions { offset: 10.0, segments: 2, clamp: false, ..Default::default() });
    assert_eq!(a.point_capacity(), b.point_capacity());
    for p in 0..a.point_capacity() {
        if a.point_valid(p) {
            assert!(a.position(p).distance(b.position(p)) < 1e-12);
        }
    }
}

#[test]
fn symmetry() {
    for seg in 1..=3 {
        let mut m = make_cube(&CubeOptions { subdiv_x: 2, ..Default::default() });
        let map = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
        assert_eq!(map.unmatched_count(), 0);
        // Three edges at one +x corner and the top edge crossing the plane.
        let edges = [
            edge_between(&m, CORNER, DVec3::new(50.0, 50.0, -50.0)),
            edge_between(&m, CORNER, DVec3::new(0.0, 50.0, 50.0)),
            edge_between(&m, CORNER, DVec3::new(50.0, -50.0, 50.0)),
        ];
        bevel_edges(&mut m, &edges, &BevelOptions { offset: 10.0, segments: seg, symmetry: Some(&map), ..Default::default() });
        check_closed_sound(&m, 10.0);
        assert!(convex_outward(&m, 1e-6));
        let after = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-6);
        assert_eq!(after.unmatched_count(), 0);
    }
}

#[test]
fn random_edge_sets_on_assorted_meshes_stay_sound() {
    let shapes = random_shapes();
    let mut seed: u32 = 12345;
    let mut next = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        seed >> 8
    };
    for (shape, base) in shapes.iter().enumerate() {
        let scale = base.bounds().size().length();
        for trial in 0..40 {
            let mut m = base.clone();
            let edges: Vec<Index> = all_edges(&m).into_iter().filter(|_| next() % 3 == 0).collect();
            let seg = 1 + (next() % 4) as i32;
            let offset = scale * if next() % 2 == 1 { 0.02 } else { 0.06 };
            let profile = if next() % 2 == 1 { 0.5 } else { 0.3 };
            bevel_edges(&mut m, &edges, &BevelOptions { offset, segments: seg, profile, ..Default::default() });
            let ctx = format!("shape {shape} trial {trial} seg {seg}");
            assert!(is_valid(&m, validate::DEFAULT | validate::DEGENERATE), "{ctx}");
            assert_eq!(euler(&m), euler(base), "{ctx}");
            assert_eq!(self_intersections(&m), 0, "{ctx}");
            assert!(max_fold(&m) < 150.0, "{ctx}");
        }
    }
}

#[test]
fn symmetry_every_face_border_of_a_split_cube() {
    // Borders meeting the plane make T junctions on it, whose corner grids must come out mirror images.
    for seg in 1..=3 {
        for f0 in 0..10 {
            let mut m = make_cube(&CubeOptions { subdiv_x: 2, ..Default::default() });
            let map = SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-3);
            let edges: Vec<Index> = m.face_corners(f0).iter().map(|&c| m.corner_edge(c)).collect();
            bevel_edges(&mut m, &edges, &BevelOptions { offset: 5.0, segments: seg, symmetry: Some(&map), ..Default::default() });
            check_closed_sound(&m, 5.0);
            assert_eq!(SymmetryMap::build(&m, &SymmetryPlane::default(), 1e-6).unmatched_count(), 0, "seg {seg} face {f0}");
        }
    }
}
