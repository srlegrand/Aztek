//! Tests: conform.

mod common;

use aztek_geo::conform::*;
use aztek_geo::primitives::*;
use aztek_geo::{Index, Mesh, INVALID};
use common::*;
use glam::{DMat3, DVec2, DVec3};

/// Plane point id at column i, row j (rows run toward -Z).
fn grid_id(sx: i32, i: i32, j: i32) -> Index {
    (j * (sx + 1) + i) as Index
}

fn spread_of(v: &[f64]) -> f64 {
    let lo = v.iter().cloned().fold(f64::MAX, f64::min);
    let hi = v.iter().cloned().fold(f64::MIN, f64::max);
    hi - lo
}

fn apply_moves(m: &mut Mesh, moves: &PointMoves) {
    for mv in moves {
        m.set_position(mv.point, mv.position);
    }
}

fn plane(w: f64, d: f64, sx: i32, sz: i32) -> Mesh {
    make_plane(&PlaneOptions { width: w, depth: d, subdiv_x: sx, subdiv_z: sz, ..Default::default() })
}

fn all_points(m: &Mesh) -> Vec<Index> {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p)).collect()
}

fn xz(p: DVec3) -> f64 {
    DVec2::new(p.x, p.z).length()
}

fn cube3() -> Mesh {
    make_cube(&CubeOptions { subdiv_x: 3, subdiv_y: 3, subdiv_z: 3, ..Default::default() })
}

#[test]
fn selections_become_chains_borders_and_interiors() {
    let pl = plane(100.0, 100.0, 4, 4);
    let all: Vec<Index> = (0..pl.face_capacity()).collect();
    let s = shape_of_faces(&pl, &all);
    assert_eq!(s.chains.len(), 1);
    assert!(s.chains[0].closed);
    assert_eq!(s.chains[0].points.len(), 16);
    assert_eq!(s.interior.len(), 9);

    // A row of points is a chain; the corners of one face are that face.
    let row: Vec<Index> = (0..=4).map(|i| grid_id(4, i, 0)).collect();
    let s = shape_of_points(&pl, &row);
    assert_eq!(s.chains.len(), 1);
    assert!(!s.chains[0].closed);
    assert_eq!(s.chains[0].points.len(), 5);
    let s = shape_of_points(&pl, &[grid_id(4, 0, 0), grid_id(4, 1, 0), grid_id(4, 1, 1), grid_id(4, 0, 1)]);
    assert_eq!(s.chains.len(), 1);
    assert!(s.chains[0].closed);
    assert!(s.interior.is_empty());
}

#[test]
fn circularize_face_region_keeps_border_on_one_circle_and_interior_inside() {
    let pl = plane(100.0, 60.0, 4, 4);
    let all: Vec<Index> = (0..pl.face_capacity()).collect();
    let s = shape_of_faces(&pl, &all);
    let mut used = Vec::new();
    let mut moves = circularize(&pl, &s, &CircleOptions::default(), Some(&mut used));
    assert_eq!(moves.len(), 16);
    follow_interior(&pl, &s.interior, &mut moves);
    assert_eq!(moves.len(), 25);
    let mut out = pl.clone();
    apply_moves(&mut out, &moves);

    let lp = &s.chains[0].points;
    let mut radii = Vec::new();
    let mut gaps = Vec::new();
    for i in 0..lp.len() {
        let p = out.position(lp[i]);
        assert!(p.y.abs() < 1e-9);
        radii.push(xz(p));
        gaps.push((out.position(lp[(i + 1) % lp.len()]) - p).length());
    }
    assert!(spread_of(&radii) < 1e-6);
    assert!(spread_of(&gaps) < 1e-6);
    assert_eq!(used.len(), 1);
    assert!(approx(used[0], radii[0]));
    // The rectangle's mean radius, give or take.
    assert!(radii[0] > 30.0 && radii[0] < 50.0);
    for &p in &s.interior {
        assert!(xz(out.position(p)) < radii[0]);
    }
    // The centre stays put by symmetry.
    assert!(out.position(grid_id(4, 2, 2)).length() < 1e-5);

    // A fixed radius and a polygon: four sides put every point on a square with its corners on the circle.
    for mv in circularize(&pl, &s, &CircleOptions { radius: 40.0, sides: 4, ..Default::default() }, None) {
        let r = xz(mv.position);
        assert!(r <= 40.0 + 1e-9);
        assert!(r >= 40.0 / 2f64.sqrt() - 1e-9);
    }
}

#[test]
fn square_loop_already_on_its_circle_does_not_move() {
    let pl = plane(100.0, 100.0, 1, 1);
    let s = shape_of_faces(&pl, &[0]);
    for mv in circularize(&pl, &s, &CircleOptions::default(), None) {
        assert!((mv.position - pl.position(mv.point)).length() < 1e-9);
    }
}

#[test]
fn rectangle_sphere_cylinder_plane_and_line_fits() {
    // A circle of 16 points squared up: corners land on four of its points, every point on the square's outline.
    let disc = make_disc(&DiscOptions { radius: 50.0, subdiv_axis: 16, subdiv_rings: 0, ..Default::default() });
    let faces: Vec<Index> = (0..disc.face_capacity()).filter(|&f| disc.face_valid(f)).collect();
    let s = shape_of_faces(&disc, &faces);
    assert_eq!(s.chains.len(), 1);
    let moves = rectangularize(&disc, &s, &RectangleOptions { spacing: Spacing::Even, square: true, ..Default::default() });
    assert_eq!(moves.len(), 16);
    let c: DVec3 = moves.iter().map(|m| m.position / 16.0).sum();
    // The farthest point is a corner, so the square's sides run at 45 degrees to it; along those every point has one
    // coordinate at the half size.
    let corner = moves
        .iter()
        .max_by(|a, b| (a.position - c).length().total_cmp(&(b.position - c).length()))
        .unwrap()
        .position
        - c;
    let d0 = corner.normalize();
    let across = DVec3::Y.cross(d0);
    let e1 = (d0 + across).normalize();
    let e2 = (d0 - across).normalize();
    let mut reach = Vec::new();
    for mv in &moves {
        assert!(approx(mv.position.y, 0.0));
        let d = mv.position - c;
        reach.push(d.dot(e1).abs().max(d.dot(e2).abs()));
    }
    assert!(spread_of(&reach) < 1e-6);

    // Spherize: a subdivided cube's points all end at one distance from the centre.
    let cube = cube3();
    let pts = all_points(&cube);
    let radii: Vec<f64> = spherize(&cube, &pts, &SpherizeOptions::default(), None).iter().map(|m| m.position.length()).collect();
    assert_eq!(radii.len(), pts.len());
    assert!(spread_of(&radii) < 1e-6);

    // Cylinder: a dented cylinder's side comes back round about its own axis.
    let mut cyl = make_cylinder(&CylinderOptions {
        radius: 30.0,
        height: 100.0,
        subdiv_axis: 12,
        subdiv_height: 2,
        subdiv_caps: 0,
        caps: false,
        ..Default::default()
    });
    let side = all_points(&cyl);
    // Opposite points on one ring, so the fitted axis stays on Y.
    for p in [side[3], side[9]] {
        cyl.set_position(p, cyl.position(p) * DVec3::new(1.2, 1.0, 1.2));
    }
    let radii: Vec<f64> = cylinderize(&cyl, &side, -1, 0.0).iter().map(|m| xz(m.position)).collect();
    assert_eq!(radii.len(), side.len());
    assert!(spread_of(&radii) < 1e-6);

    // Plane and line.
    let mut pl = plane(100.0, 100.0, 4, 4);
    pl.set_position(grid_id(4, 2, 2), DVec3::new(0.0, 10.0, 0.0));
    let grid: Vec<Index> = (0..pl.point_capacity()).collect();
    let heights: Vec<f64> = flatten_to_plane(&pl, &grid).iter().map(|m| m.position.y).collect();
    assert!(spread_of(&heights) < 1e-9);
    let sums: Vec<f64> = align_to_line(&pl, &[grid_id(4, 0, 0), grid_id(4, 4, 4), grid_id(4, 2, 2)])
        .iter()
        .map(|m| m.position.x + m.position.z)
        .collect();
    assert!(spread_of(&sums) < 1e-6);
}

#[test]
fn conform_to_mesh_lands_a_ring_one_to_one_on_a_cylinders_rim() {
    // The plane's border has 8 points; the open cylinder's rims have 8 too.
    let mut pl = plane(60.0, 60.0, 2, 2);
    for p in 0..pl.point_capacity() {
        pl.set_position(p, pl.position(p) + DVec3::new(2.0, 49.0, 3.0));
    }
    let cyl = make_cylinder(&CylinderOptions {
        radius: 25.0,
        height: 100.0,
        subdiv_axis: 8,
        subdiv_height: 1,
        subdiv_caps: 0,
        caps: false,
        ..Default::default()
    });
    let s = shape_of_faces(&pl, &[0, 1, 2, 3]);
    let mut moves = conform_to_mesh(&pl, &s, &cyl, &ConformOptions::default());
    assert_eq!(moves.len(), 8);
    let mut used = Vec::new();
    for mv in &moves {
        let mut best = 1e9;
        let mut at = INVALID;
        for p in 0..cyl.point_capacity() {
            let d = (cyl.position(p) - mv.position).length();
            if d < best {
                best = d;
                at = p;
            }
        }
        assert!(best < 1e-9);
        assert!(approx(cyl.position(at).y, 50.0));
        used.push(at);
    }
    used.sort();
    let n = used.len();
    used.dedup();
    assert_eq!(used.len(), n);
    follow_interior(&pl, &s.interior, &mut moves);
    assert_eq!(moves.len(), 9);

    // Surface mode: every point onto the sphere.
    let sphere = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 32, subdiv_height: 32, ..Default::default() });
    let onto = conform_to_mesh(&pl, &s, &sphere, &ConformOptions { mode: ConformMode::Surface, ..Default::default() });
    assert_eq!(onto.len(), 9);
    for mv in &onto {
        assert!(mv.position.length() <= 50.0 + 1e-6);
        assert!(mv.position.length() > 49.0);
    }
}

fn spherize_into(m: &mut Mesh, pts: &[Index], o: &SpherizeOptions, fitted: Option<&mut SpherizeResult>) {
    let moves = spherize(m, pts, o, fitted);
    apply_moves(m, &moves);
}

#[test]
fn spherize_lands_on_sphere_at_amount_1_and_stays_at_0() {
    let original = cube3();
    let pts = all_points(&original);

    let mut cube = original.clone();
    let mut fitted = SpherizeResult::default();
    spherize_into(&mut cube, &pts, &SpherizeOptions { radius: 37.0, center: Some(DVec3::ZERO), ..Default::default() }, Some(&mut fitted));
    assert!(approx(fitted.radius, 37.0));
    for &p in &pts {
        assert!(approx_eps(cube.position(p).length(), 37.0, 1e-9));
    }

    let mut cube = original.clone();
    spherize_into(&mut cube, &pts, &SpherizeOptions { amount: 0.0, ..Default::default() }, None);
    for &p in &pts {
        assert_eq!(cube.position(p), original.position(p));
    }

    let mut cube = original.clone();
    spherize_into(&mut cube, &pts, &SpherizeOptions::default(), None);
    let radii: Vec<f64> = pts.iter().map(|&p| cube.position(p).length()).collect();
    assert!(spread_of(&radii) < 1e-9);

    let mut cube = original.clone();
    spherize_into(
        &mut cube,
        &pts,
        &SpherizeOptions { amount: 0.5, radius: 100.0, center: Some(DVec3::ZERO), ..Default::default() },
        None,
    );
    let corner = pts[0];
    let a = original.position(corner);
    assert!((cube.position(corner) - (a + a.normalize() * 100.0) * 0.5).length() < 1e-9 * 100.0);
}

#[test]
fn spherize_fit_recovers_known_sphere_and_radius_modes() {
    let centre = DVec3::new(3.0, -2.0, 5.0);
    let radius = 7.0;
    let mut m = Mesh::new();
    let mut pts = Vec::new();
    for i in 0..40 {
        let z = -0.95 + 1.9 * i as f64 / 39.0;
        let phi = i as f64 * 2.399963;
        let ring = (1.0 - z * z).sqrt();
        pts.push(m.add_point(centre + radius * DVec3::new(ring * phi.cos(), z, ring * phi.sin())));
    }
    let mut fitted = SpherizeResult::default();
    spherize(&m, &pts, &SpherizeOptions { radius_mode: SphereRadius::Fit, ..Default::default() }, Some(&mut fitted));
    assert!((fitted.center - centre).length() < 1e-9);
    assert!(approx_eps(fitted.radius, radius, 1e-9));

    spherize(&m, &pts, &SpherizeOptions { fit: SphereFit::Center, radius_mode: SphereRadius::Max, ..Default::default() }, Some(&mut fitted));
    let max = fitted;
    spherize(&m, &pts, &SpherizeOptions { fit: SphereFit::Center, radius_mode: SphereRadius::Min, ..Default::default() }, Some(&mut fitted));
    assert!(fitted.radius <= max.radius);

    spherize(&m, &pts, &SpherizeOptions { radius_mode: SphereRadius::Explicit, radius: 12.5, ..Default::default() }, Some(&mut fitted));
    assert!(approx(fitted.radius, 12.5));

    // Four coplanar points have no sphere to fit, so they take the centroid.
    let mut flat = Mesh::new();
    let quad: Vec<Index> = [DVec3::new(1.0, 0.0, 1.0), DVec3::new(-1.0, 0.0, 1.0), DVec3::new(-1.0, 0.0, -1.0), DVec3::new(1.0, 0.0, -1.0)]
        .iter()
        .map(|&q| flat.add_point(q))
        .collect();
    spherize(&flat, &quad, &SpherizeOptions::default(), Some(&mut fitted));
    assert!(fitted.center.length() < 1e-9);
    assert!(approx(fitted.radius, 2f64.sqrt()));
}

#[test]
fn spherize_ellipsoid_scale_and_normals() {
    let original = cube3();
    let pts = all_points(&original);
    let scale = DVec3::new(2.0, 1.0, 0.5);

    let mut cube = original.clone();
    spherize_into(&mut cube, &pts, &SpherizeOptions { radius: 20.0, center: Some(DVec3::ZERO), scale, ..Default::default() }, None);
    for &p in &pts {
        assert!(approx_eps((cube.position(p) / scale).length(), 20.0, 1e-9));
    }

    // A turned frame: the long axis follows it.
    let mut cube = original.clone();
    let turned = DMat3::from_cols(DVec3::Y, DVec3::new(-1.0, 0.0, 0.0), DVec3::Z);
    spherize_into(
        &mut cube,
        &pts,
        &SpherizeOptions { radius: 20.0, center: Some(DVec3::ZERO), scale, frame: turned, ..Default::default() },
        None,
    );
    for &p in &pts {
        let q = cube.position(p);
        assert!(approx_eps((q.y / 2.0).powi(2) + q.x.powi(2) + (q.z / 0.5).powi(2), 400.0, 1e-9));
    }

    let mut cube = original.clone();
    spherize_into(
        &mut cube,
        &pts,
        &SpherizeOptions { radius: 80.0, center: Some(DVec3::ZERO), along_normals: true, ..Default::default() },
        None,
    );
    for &p in &pts {
        assert!(approx_eps(cube.position(p).length(), 80.0, 1e-9));
        let n: DVec3 = original.point_faces(p).iter().map(|&f| original.face_area_vector(f)).sum();
        let moved = cube.position(p) - original.position(p);
        assert!(moved.cross(n.normalize()).length() < 1e-9);
    }
}
