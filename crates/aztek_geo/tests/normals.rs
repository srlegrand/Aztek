//! Tests: normals.

mod common;
use common::*;

use std::f64::consts::TAU;

use aztek_geo::normals::*;
use aztek_geo::primitives::*;
use aztek_geo::triangulate::{triangulate, TriangulateOptions, Triangulation};
use aztek_geo::{names, Index, Mesh};
use glam::{DVec2, DVec3, Vec3};

fn polygon(pts: &[(f64, f64)]) -> Mesh {
    let mut m = Mesh::new();
    let ids: Vec<Index> = pts
        .iter()
        .map(|&(x, y)| m.add_point(DVec3::new(x, 0.0, -y)))
        .collect();
    m.add_face(&ids);
    m
}

fn tri_area(m: &Mesh, t: &Triangulation, i: usize, n: DVec3) -> f64 {
    let p = |k: usize| m.position(m.corner_point(t.corners[i * 3 + k]));
    (p(1) - p(0)).cross(p(2) - p(0)).dot(n) * 0.5
}

fn tri(m: &Mesh) -> Triangulation {
    triangulate(m, &TriangulateOptions::default())
}

#[test]
fn triangulate_convex_quads_concave() {
    assert_eq!(
        tri(&make_cube(&CubeOptions::default())).triangle_count(),
        12
    );

    // L shape, concave at (1,1).
    let l = polygon(&[
        (0.0, 0.0),
        (2.0, 0.0),
        (2.0, 1.0),
        (1.0, 1.0),
        (1.0, 2.0),
        (0.0, 2.0),
    ]);
    let t = tri(&l);
    assert_eq!(t.triangle_count(), 4);
    let mut total = 0.0;
    for i in 0..4 {
        let a = tri_area(&l, &t, i, DVec3::Y);
        assert!(a > 0.0);
        total += a;
    }
    assert!(approx(total, 3.0));

    // Star with 5 reflex points.
    let mut s = Mesh::new();
    let ids: Vec<Index> = (0..10)
        .map(|i| {
            let r = if i % 2 == 1 { 0.4 } else { 1.0 };
            let a = TAU * i as f64 / 10.0;
            s.add_point(DVec3::new(r * a.cos(), r * a.sin(), 0.0))
        })
        .collect();
    s.add_face(&ids);
    let ts = tri(&s);
    assert_eq!(ts.triangle_count(), 8);
    let mut total = 0.0;
    for i in 0..8 {
        let a = tri_area(&s, &ts, i, DVec3::Z);
        assert!(a > 0.0);
        total += a;
    }
    assert!(approx(total, s.face_area(0)));
    assert_eq!(tri(&s).corners, ts.corners);

    // A concave quad must split through its reflex corner.
    let dart = polygon(&[(0.0, 0.0), (1.0, 0.3), (2.0, 0.0), (1.0, 2.0)]);
    let td = tri(&dart);
    for i in 0..2 {
        assert!(tri_area(&dart, &td, i, DVec3::Y) > 0.0);
    }
}

#[test]
fn hard_and_smooth() {
    let mut cube = make_cube(&CubeOptions::default());
    let n = corner_normals(&cube, &CornerNormalOptions::default());
    for c in 0..cube.corner_capacity() {
        let fnorm = cube.face_normal(cube.corner_face(c));
        assert!(approx(n[c as usize].as_dvec3().dot(fnorm), 1.0));
    }
    set_soft_edges(&mut cube, &[]);
    let n = corner_normals(&cube, &CornerNormalOptions::default());
    for c in 0..cube.corner_capacity() {
        let expect = cube.position(cube.corner_point(c)).normalize();
        assert!(approx(n[c as usize].as_dvec3().dot(expect), 1.0));
    }
    let sphere = make_sphere(&SphereOptions {
        radius: 50.0,
        subdiv_axis: 32,
        subdiv_height: 16,
        ..Default::default()
    });
    let n = corner_normals(&sphere, &CornerNormalOptions::default());
    for c in 0..sphere.corner_capacity() {
        let expect = sphere.position(sphere.corner_point(c)).normalize();
        assert!(n[c as usize].as_dvec3().dot(expect) > 0.995);
    }
    let mut cyl = make_cylinder(&CylinderOptions {
        radius: 50.0,
        height: 100.0,
        subdiv_axis: 24,
        subdiv_height: 2,
        ..Default::default()
    });
    set_soft_edges(&mut cyl, &[]);
    set_hard_edges_by_angle(&mut cyl, 30.0, &[]);
    assert_eq!(
        cyl.edge_attrs()
            .get::<bool>(names::HARD)
            .iter()
            .filter(|&&h| h)
            .count(),
        48
    );
}

#[test]
fn plan_matches_corner_normals_and_holds_while_only_points_move() {
    let check = |m: &Mesh, plan: &mut CornerNormalPlan, opts: &CornerNormalOptions| {
        let expect = corner_normals(m, opts);
        let mut rows = vec![Vec3::ZERO; plan.rows()];
        plan.compute(m, &mut rows);
        let mut row = 0;
        for f in 0..m.face_capacity() {
            if m.face_valid(f) {
                m.for_each_face_corner(f, |c| {
                    assert!((rows[row] - expect[c as usize]).length() < 1e-5);
                    row += 1;
                });
            }
        }
        assert_eq!(row, plan.rows());
    };
    let mut cyl = make_cylinder(&CylinderOptions {
        radius: 50.0,
        height: 100.0,
        subdiv_axis: 24,
        subdiv_height: 2,
        ..Default::default()
    });
    set_soft_edges(&mut cyl, &[]);
    set_hard_edges_by_angle(&mut cyl, 30.0, &[]);
    let mut sphere = make_sphere(&SphereOptions {
        radius: 50.0,
        subdiv_axis: 32,
        subdiv_height: 16,
        ..Default::default()
    });
    sphere.remove_face(5);
    for w in [
        NormalWeight::Uniform,
        NormalWeight::Area,
        NormalWeight::Angle,
        NormalWeight::AreaAngle,
    ] {
        for m in [&cyl, &sphere] {
            let opts = CornerNormalOptions {
                weight: w,
                ..Default::default()
            };
            let mut plan = CornerNormalPlan::new(m, &opts);
            check(m, &mut plan, &opts);
            let mut moved = m.clone();
            assert_eq!(moved.topology_id(), m.topology_id());
            for p in 0..moved.point_capacity() {
                let q = moved.position(p) * DVec3::new(1.0, 1.5, 0.5)
                    + DVec3::new(0.0, 0.01 * (p % 7) as f64, 0.0);
                moved.set_position(p, q);
            }
            assert!(plan.fits(&moved));
            check(&moved, &mut plan, &opts);
        }
    }
    let d = CornerNormalOptions::default();
    let mut changed = cyl.clone();
    assert!(CornerNormalPlan::new(&cyl, &d).fits(&changed));
    set_soft_edges(&mut changed, &[]);
    assert!(!CornerNormalPlan::new(&cyl, &d).fits(&changed));
    let mut changed = cyl.clone();
    changed.remove_face(0);
    assert_ne!(changed.topology_id(), cyl.topology_id());
    assert!(!CornerNormalPlan::new(&cyl, &d).fits(&changed));
}

#[test]
fn mikktspace_tangents() {
    let mut plane = make_plane(&PlaneOptions::default());
    assert!(update_tangents(&mut plane, &TangentOptions::default()));
    for t in plane.corner_attrs().get::<[f32; 4]>(names::TANGENT) {
        assert!(approx(t[0] as f64, 1.0) && approx(t[3] as f64, 1.0));
    }
    let mut sphere = make_sphere(&SphereOptions::default());
    assert!(update_tangents(&mut sphere, &TangentOptions::default()));
    let cn = corner_normals(&sphere, &CornerNormalOptions::default());
    for (c, t) in sphere
        .corner_attrs()
        .get::<[f32; 4]>(names::TANGENT)
        .iter()
        .enumerate()
    {
        assert!(Vec3::new(t[0], t[1], t[2]).dot(cn[c]).abs() < 1e-3);
    }
}

#[test]
fn parallel_tangents_match_one_call() {
    let one = make_sphere(&SphereOptions {
        radius: 50.0,
        subdiv_axis: 24,
        subdiv_height: 16,
        ..Default::default()
    });
    let mut many = Mesh::new();
    for i in 0..200 {
        let mut s = one.clone();
        for p in s.positions_mut() {
            p.x += 150.0 * i as f64;
        }
        many.append(&s);
    }
    let tm = tri(&many);
    assert!(tm.triangle_count() > 1 << 17);
    let n = corner_normals(&many, &CornerNormalOptions::default());
    assert_eq!(
        corner_tangents(&many, &tm, &n, "uv0", true),
        corner_tangents(&many, &tm, &n, "uv0", false)
    );
}

#[test]
fn parallel_tangents_on_one_connected_mesh_match_one_call() {
    let mut m = make_plane(&PlaneOptions {
        width: 100.0,
        depth: 100.0,
        subdiv_x: 300,
        subdiv_z: 300,
    });
    // A second plane edge to edge, its own points on the same positions, so MikkTSpace welds across the seam.
    let mut side = make_plane(&PlaneOptions {
        width: 100.0,
        depth: 100.0,
        subdiv_x: 300,
        subdiv_z: 300,
    });
    for p in side.positions_mut() {
        p.x += 100.0;
    }
    m.append(&side);
    for p in m.positions_mut() {
        p.y = 3.0 * (p.x * 0.2).sin() * (p.z * 0.3).cos();
    }
    for (c, uv) in m
        .corner_attrs_mut()
        .get_mut::<[f32; 2]>("uv0")
        .iter_mut()
        .enumerate()
    {
        // Mirrored corners: mixed bitangent signs.
        uv[1] *= if c % 7 == 0 { -1.0 } else { 1.0 };
    }
    let tris = tri(&m);
    assert!(tris.triangle_count() > 1 << 17);
    let n = corner_normals(&m, &CornerNormalOptions::default());
    let par = corner_tangents(&m, &tris, &n, "uv0", true);
    let one = corner_tangents(&m, &tris, &n, "uv0", false);
    assert_eq!(par.len(), one.len());
    assert_eq!(par.iter().zip(&one).filter(|(a, b)| a != b).count(), 0);
}

#[test]
fn ear_clip_and_weighted_normals() {
    // A square in 2D clips into two triangles of the right winding.
    let mut out = Vec::new();
    aztek_geo::triangulate::ear_clip(&[DVec2::ZERO, DVec2::X, DVec2::ONE, DVec2::Y], &mut out);
    assert_eq!(out.len(), 6);
    // On a soft cube every face at a point weighs the same, so each corner takes the point's diagonal.
    let mut cube = make_cube(&CubeOptions::default());
    set_soft_edges(&mut cube, &[]);
    let n = weighted_normals(&cube, &WeightedNormalOptions::default());
    for c in 0..cube.corner_capacity() {
        let expect = cube.position(cube.corner_point(c)).normalize();
        assert!(approx(n[c as usize].as_dvec3().dot(expect), 1.0));
    }
}
