//! Tests: uv.

use std::collections::BTreeSet;
use std::f64::consts::PI;

use aztek_geo::primitives::{
    make_cube, make_cylinder, make_plane, make_sphere, CapFill, CubeOptions, CylinderOptions, PlaneOptions, SphereOptions,
};
use aztek_geo::{names, Index, Mesh};
use aztek_uv::*;
use glam::{DMat3, DVec2, DVec3};

fn clear_seams(m: &mut Mesh) {
    m.edge_attrs_mut().get_or_add::<bool>(names::SEAM).fill(false);
}

fn overlaps(m: &Mesh) -> usize {
    let mut flags = Vec::new();
    overlapping_faces(m, "uv0", &mut flags, &[])
}

fn summary(m: &Mesh) -> DistortionSummary {
    summarize(m, &face_distortion(m, "uv0", &[]), &[])
}

fn inside_unit(m: &Mesh) -> bool {
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0");
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).all(|f| {
        m.face_corners(f).iter().all(|&c| {
            let p = uv[c as usize];
            p[0] >= -1e-6 && p[1] >= -1e-6 && p[0] <= 1.0 + 1e-6 && p[1] <= 1.0 + 1e-6
        })
    })
}

fn transform_points(m: &mut Mesh, r: DMat3, t: DVec3) {
    for p in m.positions_mut() {
        *p = r * *p + t;
    }
}

/// Faces whose uv winding is against their 3D winding.
fn flipped_faces(m: &Mesh) -> usize {
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0");
    (0..m.face_capacity())
        .filter(|&f| m.face_valid(f))
        .filter(|&f| {
            let mut a = 0.0;
            m.for_each_face_corner(f, |c| {
                let (p, q) = (uv[c as usize], uv[m.corner_next(c) as usize]);
                a += p[0] as f64 * q[1] as f64 - p[1] as f64 * q[0] as f64;
            });
            a <= 0.0
        })
        .count()
}

fn approx(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() <= eps * (1.0 + a.abs().max(b.abs()))
}

/// Many separate quads of assorted sizes and aspect ratios, scattered.
fn quad_soup(n: i32) -> Mesh {
    let mut pts = Vec::new();
    let (mut sizes, mut idx) = (Vec::new(), Vec::new());
    let mut seed: u32 = 12345;
    let mut rnd = || {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        (seed >> 8) as f64 / (1u32 << 24) as f64
    };
    for i in 0..n {
        let (w, h) = (2.0 + 30.0 * rnd(), 2.0 + 30.0 * rnd());
        let o = DVec3::new(rnd() * 500.0, rnd() * 500.0, i as f64 * 3.0);
        let b = pts.len() as Index;
        pts.extend([o, o + DVec3::new(w, 0.0, 0.0), o + DVec3::new(w, h, 0.0), o + DVec3::new(0.0, h, 0.0)]);
        sizes.push(4);
        idx.extend((0..4).map(|k| b + k));
    }
    let mut m = Mesh::from_polygons(&pts, &sizes, &idx);
    let mut uv = vec![[0f32; 2]; m.corner_capacity() as usize];
    for f in 0..m.face_capacity() {
        m.for_each_face_corner(f, |c| {
            let p = m.position(m.corner_point(c));
            uv[c as usize] = [p.x as f32, p.y as f32];
        });
    }
    uv_layer(&mut m, "uv0").copy_from_slice(&uv);
    m
}

#[test]
fn lscm_and_slim_flatten_a_planar_grid_exactly() {
    let m = make_plane(&PlaneOptions { subdiv_x: 6, subdiv_z: 4, ..Default::default() });
    let mut charts = build_charts(&m, &[], &ChartOptions::default());
    assert_eq!(charts.len(), 1);
    for p in charts[0].pinned.iter_mut() {
        *p = false;
    }
    assert!(unwrap_chart(&mut charts[0], &UnwrapOptions::default()).ok);
    let mut out = m.clone();
    write_charts(&mut out, &charts, "uv0");
    let d = summary(&out);
    assert!(d.mean_area < 1e-3 && d.mean_angle < 1e-3 && d.folded == 0, "{d:?}");
}

#[test]
fn a_cylinder_cut_once_unwraps_with_low_stretch() {
    let co = CylinderOptions { caps: false, subdiv_axis: 32, subdiv_height: 8, height: 200.0, ..Default::default() };
    let mut m = make_cylinder(&co);
    clear_seams(&mut m);
    // One straight seam along the side.
    let mut cut = 0;
    let mut seam = vec![false; m.edge_capacity() as usize];
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) {
            continue;
        }
        let (a, b) = (m.position(m.edge_point(e, 0)), m.position(m.edge_point(e, 1)));
        if (a.x - b.x).abs() < 1e-6 && (a.z - b.z).abs() < 1e-6 && a.x > 49.9 && a.z.abs() < 1e-6 {
            seam[e as usize] = true;
            cut += 1;
        }
    }
    m.edge_attrs_mut().get_mut::<bool>(names::SEAM).copy_from_slice(&seam);
    assert_eq!(cut, co.subdiv_height);
    let mut charts = build_charts(&m, &[], &ChartOptions::default());
    assert_eq!(charts.len(), 1);
    assert_eq!(charts[0].euler(), 1);
    for method in [UnwrapMethod::Slim, UnwrapMethod::Bff] {
        let mut ch = charts[0].clone();
        let r = unwrap_chart(&mut ch, &UnwrapOptions { method, ..Default::default() });
        assert!(r.ok);
        assert_eq!(folded_triangles(&ch), 0);
        assert!(chart_distortion(&ch) < 0.01, "{method:?} {}", chart_distortion(&ch));
        assert!(!chart_overlaps(&ch));
    }
    unwrap_charts(&mut charts, &UnwrapOptions::default());
    write_charts(&mut m, &charts, "uv0");
    let d = summary(&m);
    assert!(d.mean_area < 0.01 && d.mean_angle < 0.02 && d.folded == 0, "{d:?}");
    assert_eq!(overlaps(&m), 0);
}

#[test]
fn bff_shapes_the_boundary() {
    let m = make_plane(&PlaneOptions { subdiv_x: 8, subdiv_z: 8, ..Default::default() });
    let ch0 = build_charts(&m, &[], &ChartOptions::default()).remove(0);
    for boundary in [BffBoundary::MinimalArea, BffBoundary::Disk, BffBoundary::Polygon] {
        let mut ch = ch0.clone();
        assert!(bff(&mut ch, &BffOptions { boundary, ..Default::default() }).ok, "{boundary:?}");
        assert_eq!(folded_triangles(&ch), 0);
    }
    // A flat square stays a square: lengths kept, nothing to bend.
    let mut ch = ch0.clone();
    bff(&mut ch, &BffOptions::default());
    let ext = ch.uv_max() - ch.uv_min();
    assert!(approx(ext.x, 100.0, 1e-6) && approx(ext.y, 100.0, 1e-6), "{ext}");
    // Disk: the boundary on a circle.
    let mut ch = ch0.clone();
    bff(&mut ch, &BffOptions { boundary: BffBoundary::Disk, ..Default::default() });
    let lp = &ch.boundary_loops()[0];
    let c = lp.iter().fold(DVec2::ZERO, |a, &v| a + ch.uv[v as usize]) / lp.len() as f64;
    let radii: Vec<f64> = lp.iter().map(|&v| (ch.uv[v as usize] - c).length()).collect();
    let (lo, hi) = radii.iter().fold((f64::MAX, 0f64), |(l, h), &r| (l.min(r), h.max(r)));
    // Four rounds of settling the lengths leave it round to well under a percent.
    assert!(hi - lo < 1e-2 * hi, "{lo} {hi}");
}

#[test]
fn projections() {
    let mut m = make_cube(&CubeOptions::default());
    let mut o = ProjectOptions { ty: Projection::Box, ..Default::default() };
    assert!(project(&mut m, &[], "uv0", &o).is_some());
    assert_eq!(island_faces(&m, &[], &ChartOptions::default()).len(), 6);
    assert!(inside_unit(&m));
    let d = summary(&m);
    assert!(d.mean_area < 1e-4 && d.folded == 0);

    let mut c = make_cylinder(&CylinderOptions { caps: false, ..Default::default() });
    o.ty = Projection::Cylindrical;
    o.axis = ProjectAxis::Y;
    assert!(project(&mut c, &[], "uv0", &o).is_some());
    let dc = summary(&c);
    assert!(dc.folded == 0 && dc.mean_angle < 0.05, "{dc:?}");

    o.ty = Projection::Planar;
    o.axis = ProjectAxis::BestFit;
    let mut p = make_plane(&PlaneOptions::default());
    assert!(project(&mut p, &[], "uv0", &o).is_some());
    assert!(summary(&p).mean_angle < 1e-4);

    o.ty = Projection::Spherical;
    let mut s = make_sphere(&SphereOptions::default());
    assert!(project(&mut s, &[], "uv0", &o).is_some());
    assert_eq!(summary(&s).folded, 0);
}

#[test]
fn cylinder_fit_finds_a_tilted_axis_and_seams_along_an_edge_column() {
    let rot = euler_frame(DVec3::new(31.0, -17.0, 52.0));
    let mv = DVec3::new(120.0, -40.0, 15.0);
    for caps in [true, false] {
        let co = CylinderOptions { caps, subdiv_axis: 24, subdiv_height: 3, ..Default::default() };
        let mut m = make_cylinder(&co);
        transform_points(&mut m, rot, mv);
        let o = ProjectOptions { ty: Projection::Cylindrical, ..Default::default() };
        let fr = fit_projection(&m, &[], &o);
        let angle = fr.axes.z_axis.dot(rot * DVec3::Y).abs().min(1.0).acos().to_degrees();
        assert!(angle < 1.0, "caps {caps}: {angle}");
        assert!(approx(fr.radius, 50.0, 0.02), "{}", fr.radius);
        assert!(approx(fr.height, 100.0, 1e-6));
        assert!((fr.center - mv).length() < 1e-6);

        if caps {
            continue;
        }
        clear_seams(&mut m);
        assert!(project(&mut m, &[], "uv0", &o).is_some());
        assert_eq!(flipped_faces(&m), 0);
        assert_eq!(overlaps(&m), 0);
        // The seam runs along one column of edges: no face is cut across the wrap.
        let seams = m.edge_attrs().get::<bool>(names::SEAM).iter().filter(|&&s| s).count();
        assert_eq!(seams as i32, co.subdiv_height);
        assert!(summary(&m).mean_angle < 0.05);
    }
}

#[test]
fn cylinder_seam_goes_to_the_side_facing_away_from_the_view() {
    let mut m = make_cylinder(&CylinderOptions { caps: false, subdiv_axis: 16, ..Default::default() });
    // The primitive's own seam would win over the view.
    clear_seams(&mut m);
    let mut o = ProjectOptions { ty: Projection::Cylindrical, ..Default::default() };
    let seam_dir = |fr: &ProjectFrame| fr.seam_angle.cos() * fr.axes.x_axis + fr.seam_angle.sin() * fr.axes.y_axis;
    for view in [DVec3::new(0.0, 0.0, -1.0), DVec3::new(1.0, 0.0, 0.0), DVec3::new(-0.7, -0.3, 0.7)] {
        o.view_dir = Some(view);
        let fr = fit_projection(&m, &[], &o);
        let z = fr.axes.z_axis;
        let away = (view - view.dot(z) * z).normalize();
        // Within half a segment of straight behind, on a vertex column.
        assert!(seam_dir(&fr).dot(away) > (PI / 16.0).cos() - 1e-9, "{view}");
    }

    // An existing seam column is preferred over the far side.
    o.view_dir = Some(DVec3::new(0.0, 0.0, -1.0));
    let behind = seam_dir(&fit_projection(&m, &[], &o));
    let mut seam = vec![false; m.edge_capacity() as usize];
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) {
            let (a, b) = (m.position(m.edge_point(e, 0)), m.position(m.edge_point(e, 1)));
            seam[e as usize] = (a.y - b.y).abs() > 1.0 && DVec3::new(a.x, 0.0, a.z).normalize().dot(behind) < -0.99;
        }
    }
    assert_eq!(seam.iter().filter(|&&s| s).count(), 1);
    m.edge_attrs_mut().get_or_add::<bool>(names::SEAM).copy_from_slice(&seam);
    let fr = fit_projection(&m, &[], &o);
    assert!(seam_dir(&fr).dot(behind) < -0.99);
}

#[test]
fn plane_fit_on_a_rotated_plane_gives_undistorted_uvs_along_its_sides() {
    let mut m = make_plane(&PlaneOptions { width: 200.0, depth: 100.0, subdiv_x: 4, subdiv_z: 2 });
    transform_points(
        &mut m,
        euler_frame(DVec3::new(20.0, 35.0, -50.0)) * euler_frame(DVec3::new(0.0, 27.0, 0.0)),
        DVec3::new(5.0, 6.0, 7.0),
    );
    assert!(project(&mut m, &[], "uv0", &ProjectOptions::default()).is_some());
    let d = summary(&m);
    assert!(d.mean_angle < 1e-4 && d.mean_area < 1e-4);
    assert_eq!(flipped_faces(&m), 0);
    // Aligned to the minimum-area rectangle, so the 2:1 plane fills 1 x 0.5 exactly.
    let (mut lo, mut hi) = ([1e9f32; 2], [-1e9f32; 2]);
    for p in m.corner_attrs().get::<[f32; 2]>("uv0") {
        lo = [lo[0].min(p[0]), lo[1].min(p[1])];
        hi = [hi[0].max(p[0]), hi[1].max(p[1])];
    }
    let ext = [(hi[0] - lo[0]) as f64, (hi[1] - lo[1]) as f64];
    assert!(approx(ext[0].max(ext[1]), 1.0, 1e-5));
    assert!(approx(ext[0].min(ext[1]), 0.5, 1e-5));
    assert!((lo[0] as f64).abs() < 1e-5 && (lo[1] as f64).abs() < 1e-5);
}

#[test]
fn projecting_through_a_fitted_frames_explicit_values_reproduces_the_fit() {
    let mut m = make_cylinder(&CylinderOptions { cap_fill: CapFill::Ngon, ..Default::default() });
    transform_points(&mut m, euler_frame(DVec3::new(10.0, 20.0, 30.0)), DVec3::new(1.0, 2.0, 3.0));
    for ty in [Projection::Planar, Projection::Box, Projection::Cylindrical, Projection::Spherical] {
        let o = ProjectOptions { ty, ..Default::default() };
        let mut a = m.clone();
        let fr = project(&mut a, &[], "uv0", &o).unwrap();
        let e = ProjectOptions {
            center: Some(fr.center),
            rotation: Some(frame_euler(&fr.axes)),
            width: Some(fr.width),
            height: Some(fr.height),
            depth: Some(fr.depth),
            radius: Some(fr.radius),
            seam_angle: Some(fr.seam_angle.to_degrees()),
            ..o.clone()
        };
        let mut b = m.clone();
        assert!(project(&mut b, &[], "uv0", &e).is_some());
        let (ua, ub) = (a.corner_attrs().get::<[f32; 2]>("uv0"), b.corner_attrs().get::<[f32; 2]>("uv0"));
        // A corner of a face around the axis (a cap) exactly opposite the seam wraps to either side depending on the
        // last bit of the frame's rotation; a known tie. A whole wrap apart counts as the same.
        let wrap = match ty {
            Projection::Cylindrical => (2.0 * PI * fr.radius / (2.0 * PI * fr.radius).max(fr.height)) as f32,
            Projection::Spherical => 1.0,
            _ => f32::INFINITY,
        };
        let worst = ua
            .iter()
            .zip(ub)
            .map(|(p, q)| {
                let du = (p[0] - q[0]).abs();
                let du = du.min((du - wrap).abs());
                (du.powi(2) + (p[1] - q[1]).powi(2)).sqrt()
            })
            .fold(0f32, f32::max);
        assert!(worst < 1e-4, "{ty:?}: {worst}");
    }
}

#[test]
fn box_fit_follows_the_objects_principal_axes() {
    let mut m = make_cube(&CubeOptions { width: 200.0, height: 50.0, depth: 100.0, ..Default::default() });
    let rot = euler_frame(DVec3::new(0.0, 0.0, 25.0));
    transform_points(&mut m, rot, DVec3::ZERO);
    let o = ProjectOptions { ty: Projection::Box, ..Default::default() };
    let fr = fit_projection(&m, &[], &o);
    for i in 0..3 {
        assert!(fr.axes.col(i).dot(rot.col(i)).abs() > 0.9999);
    }
    assert!(approx(fr.width, 200.0, 1e-9) && approx(fr.height, 50.0, 1e-9) && approx(fr.depth, 100.0, 1e-9));
    assert!(project(&mut m, &[], "uv0", &o).is_some());
    assert_eq!(flipped_faces(&m), 0);
    assert!(summary(&m).mean_angle < 1e-4);
}

#[test]
fn straighten_turns_a_bent_quad_strip_into_a_straight_grid() {
    let co = CylinderOptions { caps: false, subdiv_axis: 16, subdiv_height: 3, ..Default::default() };
    let m = make_cylinder(&co);
    let mut charts = build_charts(&m, &[], &ChartOptions::default());
    assert!(!charts.is_empty());
    let ch = &mut charts[0];
    for p in ch.uv.iter_mut() {
        p.x += (p.y * 7.0).sin() * 0.01;
    }
    assert_eq!(straighten_chart(ch), StraightenResult::Grid);
    let key = |x: f64| (x * 1e6).round() as i64;
    let us: BTreeSet<i64> = ch.uv.iter().map(|p| key(p.x)).collect();
    let vs: BTreeSet<i64> = ch.uv.iter().map(|p| key(p.y)).collect();
    assert_eq!(us.len(), co.subdiv_axis as usize + 1);
    assert_eq!(vs.len(), co.subdiv_height as usize + 1);
}

#[test]
fn orient_aligns_an_island_to_the_axes() {
    let mut m = make_plane(&PlaneOptions::default());
    for p in uv_layer(&mut m, "uv0").iter_mut() {
        let a = 0.5f32;
        *p = [p[0] * 2.0 * a.cos() - p[1] * a.sin(), p[0] * 2.0 * a.sin() + p[1] * a.cos()];
    }
    let co = ChartOptions { source: IslandSource::Uvs, ..Default::default() };
    let mut charts = build_charts(&m, &[], &co);
    orient_chart(&mut charts[0], OrientMode::BoundingBox);
    let ext = charts[0].uv_max() - charts[0].uv_min();
    assert!(approx(ext.x, 2.0, 1e-4) && approx(ext.y, 1.0, 1e-4), "{ext}");
}

#[test]
fn texel_density_measure_and_set() {
    let mut m = make_plane(&PlaneOptions::default()); // 100 x 100 cm, uv 0-1
    assert!(approx(texel_density(&m, "uv0", &[], 1024.0, &DMat3::IDENTITY), 10.24, 1e-6));
    set_texel_density(&mut m, "uv0", &[], 1024.0, 5.12, &DMat3::IDENTITY);
    assert!(approx(texel_density(&m, "uv0", &[], 1024.0, &DMat3::IDENTITY), 5.12, 1e-6));
}

#[test]
fn overlap_detection_finds_stacked_faces() {
    let mut m = quad_soup(4);
    // Same unit square for every face.
    let sq = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let mut uv = vec![[0.5f32; 2]; m.corner_capacity() as usize];
    for f in 0..m.face_capacity() {
        for (k, c) in m.face_corners(f).into_iter().enumerate() {
            uv[c as usize] = sq[k];
        }
    }
    uv_layer(&mut m, "uv0").copy_from_slice(&uv);
    assert_eq!(overlaps(&m), 4);
}
