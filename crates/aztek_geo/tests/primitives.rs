//! Tests: primitives.

mod common;
use common::*;

use aztek_geo::primitives::*;
use aztek_geo::{names, validate, Index, Mesh};

fn check_closed(m: &Mesh, expected_euler: i64) {
    assert!(valid(m));
    assert_eq!(boundary_edge_count(m), 0);
    assert_eq!(euler(m), expected_euler);
    assert!(signed_volume(m) > 0.0);
    assert!(uvs_dont_overlap(m));
}

/// An open dome: valid, outward, one flat boundary loop at height y, every boundary point on exactly two border edges.
fn check_dome(m: &Mesh, y: f64) {
    assert!(is_valid(m, validate::DEFAULT | validate::DEGENERATE));
    assert!(uvs_dont_overlap(m));
    assert!(boundary_edge_count(m) > 0);
    let mut border = vec![0; m.point_capacity() as usize];
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) && m.is_boundary_edge(e) {
            let c = m.edge_corner(e);
            border[m.corner_point(c) as usize] += 1;
            border[m.corner_next_point(c) as usize] += 1;
        }
    }
    for p in 0..m.point_capacity() {
        if m.point_valid(p) && border[p as usize] != 0 {
            assert_eq!(border[p as usize], 2);
            // Approx(y).epsilon(1e-9).scale(100)
            assert!(
                (m.position(p).y - y).abs() < 1e-9 * (100.0 + m.position(p).y.abs().max(y.abs()))
            );
        }
    }
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            assert!(m.face_normal(f).dot(m.face_centroid(f)) > 0.0);
        }
    }
    assert!(approx_scale(m.bounds().min.y, y, 100.0));
}

fn all_quads(m: &Mesh) -> bool {
    (0..m.face_capacity()).all(|f| !m.face_valid(f) || m.face_size(f) == 4)
}

fn sides(m: &Mesh, n: Index) -> i32 {
    (0..m.face_capacity())
        .filter(|&f| m.face_valid(f) && m.face_size(f) == n)
        .count() as i32
}

#[test]
fn cube() {
    let m = make_cube(&CubeOptions::default());
    assert_eq!(m.num_points(), 8);
    assert_eq!(m.num_faces(), 6);
    check_closed(&m, 2);
    assert!(approx(signed_volume(&m), 1e6));
    assert!(m.edge_attrs().get::<bool>(names::HARD).iter().all(|&h| h));

    let s = make_cube(&CubeOptions {
        width: 200.0,
        height: 100.0,
        depth: 50.0,
        subdiv_x: 3,
        subdiv_y: 2,
        subdiv_z: 4,
        base_at_origin: true,
    });
    check_closed(&s, 2);
    assert_eq!(s.num_faces(), 2 * (3 * 2 + 2 * 4 + 3 * 4));
    assert!(approx(s.bounds().min.y, 0.0));
    assert!(approx(s.bounds().size().x, 200.0));
    assert_eq!(
        s.edge_attrs()
            .get::<bool>(names::HARD)
            .iter()
            .filter(|&&h| h)
            .count(),
        4 * (3 + 2 + 4)
    );
}

#[test]
fn plane_and_disc() {
    let p = make_plane(&PlaneOptions {
        width: 100.0,
        depth: 50.0,
        subdiv_x: 4,
        subdiv_z: 2,
    });
    assert!(valid(&p));
    assert_eq!(p.num_faces(), 8);
    assert!(approx(p.face_normal(0).y, 1.0));
    assert!(uvs_dont_overlap(&p));
    assert_eq!(boundary_edge_count(&p), 12);

    let d = make_disc(&DiscOptions::default());
    assert!(valid(&d));
    assert!(approx(d.face_normal(0).y, 1.0));
    assert!(uvs_dont_overlap(&d));
    let n = make_disc(&DiscOptions {
        radius: 50.0,
        subdiv_axis: 12,
        subdiv_rings: 0,
        ..Default::default()
    });
    assert!(valid(&n));
    assert_eq!(n.num_faces(), 1);
    assert_eq!(n.face_size(0), 12);
    assert!(approx(n.face_normal(0).y, 1.0));
}

#[test]
fn round() {
    let sphere = make_sphere(&SphereOptions::default());
    check_closed(&sphere, 2);
    assert_eq!(sphere.num_points(), 20 * 19 + 2);
    assert_eq!(sphere.num_faces(), 400);
    assert!(approx(sphere.bounds().size().y, 100.0));

    let cyl = make_cylinder(&CylinderOptions::default());
    check_closed(&cyl, 2);
    assert_eq!(cyl.num_faces(), 20 + 2 * (5 * 5 + 20));
    let cyl2 = make_cylinder(&CylinderOptions {
        radius: 50.0,
        height: 100.0,
        subdiv_axis: 16,
        subdiv_height: 3,
        subdiv_caps: 2,
        caps: true,
        base_at_origin: true,
        ..Default::default()
    });
    check_closed(&cyl2, 2);
    assert!(approx(cyl2.bounds().min.y, 0.0));

    check_closed(&make_cone(&ConeOptions::default()), 2);
    check_closed(
        &make_cone(&ConeOptions {
            radius: 50.0,
            height: 100.0,
            subdiv_axis: 12,
            subdiv_height: 2,
            subdiv_cap: 2,
            ..Default::default()
        }),
        2,
    );

    let torus = make_torus(&TorusOptions::default());
    check_closed(&torus, 0);
    assert_eq!(torus.num_faces(), 400);

    check_closed(&make_pipe(&PipeOptions::default()), 0);

    let qs = make_quad_sphere(&QuadSphereOptions::default());
    check_closed(&qs, 2);
    assert_eq!(qs.num_faces(), 6 * 9);
    for p in 0..qs.point_capacity() {
        assert!(approx(qs.position(p).length(), 50.0));
    }
}

#[test]
fn overlap_check_catches_overlap() {
    let mut p = make_plane(&PlaneOptions {
        width: 100.0,
        depth: 100.0,
        subdiv_x: 2,
        subdiv_z: 1,
    });
    let uv = p.corner_attrs_mut().get_mut::<[f32; 2]>("uv0");
    for (c, v) in uv.iter_mut().enumerate() {
        *v = [
            if c % 4 == 1 || c % 4 == 2 { 1.0 } else { 0.0 },
            if c % 4 >= 2 { 1.0 } else { 0.0 },
        ];
    }
    assert!(!uvs_dont_overlap(&p));
}

#[test]
fn quad_sphere_fraction() {
    let mut o = QuadSphereOptions {
        subdiv: 4,
        fraction: 0.5,
        ..Default::default()
    };
    let half = make_quad_sphere(&o);
    check_dome(&half, 0.0);
    assert!(all_quads(&half));
    assert_eq!(half.num_faces(), 3 * 16);
    // At one half the cut follows the full sphere's equator, so the points are the full sphere's upper ones.
    o.fraction = 1.0;
    let full = make_quad_sphere(&o);
    let upper = (0..full.point_capacity())
        .filter(|&p| full.position(p).y > -1e-9)
        .count() as Index;
    assert_eq!(half.num_points(), upper);

    for n in [1, 3, 8] {
        for f in [0.05, 0.2, 0.5, 0.7, 0.85, 0.97] {
            let m = make_quad_sphere(&QuadSphereOptions {
                subdiv: n,
                fraction: f,
                ..Default::default()
            });
            check_dome(&m, 50.0 * (1.0 - 2.0 * f));
            assert!(all_quads(&m), "n {n} f {f}");
            for p in 0..m.point_capacity() {
                assert!(approx(m.position(p).length(), 50.0));
            }
        }
    }
}

#[test]
fn sphere_and_ico_sphere_fraction() {
    let mut s = SphereOptions {
        fraction: 0.5,
        ..Default::default()
    };
    let hemi = make_sphere(&s);
    check_dome(&hemi, 0.0);
    assert_eq!(hemi.num_faces(), 20 * 10);
    s.base_at_origin = true;
    s.fraction = 0.25;
    let cap = make_sphere(&s);
    check_dome(&cap, 0.0);
    assert!(approx(cap.bounds().max.y, 25.0));

    for level in [1, 2, 3, 4] {
        for f in [0.1, 0.3, 0.5, 0.77] {
            let m = make_ico_sphere(&IcoSphereOptions {
                subdivisions: level,
                fraction: f,
                ..Default::default()
            });
            check_dome(&m, 50.0 * (1.0 - 2.0 * f));
            for p in 0..m.point_capacity() {
                assert!(approx(m.position(p).length(), 50.0), "level {level} f {f}");
            }
        }
    }
    let based = make_ico_sphere(&IcoSphereOptions {
        fraction: 0.5,
        base_at_origin: true,
        ..Default::default()
    });
    assert!(approx_scale(based.bounds().min.y, 0.0, 100.0));
}

#[test]
fn cap_fills() {
    for axis in [3, 4, 5, 6, 7, 12, 20, 21, 32] {
        for rings in [0, 1, 3] {
            let mut o = CylinderOptions {
                subdiv_axis: axis,
                subdiv_height: 2,
                subdiv_caps: rings,
                ..Default::default()
            };
            let quads = make_cylinder(&o);
            check_closed(&quads, 2);
            assert!(is_valid(&quads, validate::DEFAULT | validate::DEGENERATE));
            let border = axis - axis % 2;
            let (a, c) = (border / 4, border / 2 - border / 4);
            let extra = 2 * (rings.max(1) - 1) * axis;
            if axis >= 4 {
                assert_eq!(
                    quads.num_faces() as i32,
                    2 * axis + 2 * (a * c + border + axis % 2) + extra,
                    "axis {axis} rings {rings}"
                );
                assert_eq!(sides(&quads, 3), 2 * (axis % 2));
                assert_eq!(sides(&quads, 4), quads.num_faces() as i32 - 2 * (axis % 2));
            } else {
                assert_eq!(quads.num_faces() as i32, 2 * axis + 2 + extra);
            }
            o.cap_fill = CapFill::Fan;
            let fan = make_cylinder(&o);
            check_closed(&fan, 2);
            assert_eq!(fan.num_faces() as i32, 2 * axis + 2 * axis + extra);
            o.cap_fill = CapFill::Ngon;
            let ngon = make_cylinder(&o);
            check_closed(&ngon, 2);
            assert_eq!(ngon.num_faces() as i32, 2 * axis + 2 + extra);
            o.cap_fill = CapFill::None;
            let open = make_cylinder(&o);
            assert!(valid(&open));
            assert_eq!(open.num_faces() as i32, 2 * axis);
            assert_eq!(boundary_edge_count(&open) as i32, 2 * axis);

            let mut k = ConeOptions {
                subdiv_axis: axis,
                subdiv_cap: rings,
                ..Default::default()
            };
            let cone = make_cone(&k);
            check_closed(&cone, 2);
            if axis >= 4 {
                assert_eq!(sides(&cone, 3), axis + axis % 2);
            }
            k.cap_fill = CapFill::None;
            let open_cone = make_cone(&k);
            assert!(valid(&open_cone));
            assert_eq!(boundary_edge_count(&open_cone) as i32, axis);
        }
    }
    // Quad caps are flat, inside the rim, and their uvs stay in their island.
    let m = make_cylinder(&CylinderOptions {
        radius: 30.0,
        height: 80.0,
        subdiv_axis: 24,
        ..Default::default()
    });
    let uv = m.corner_attrs().get::<[f32; 2]>(&names::uv(0));
    for f in 0..m.face_capacity() {
        if m.face_valid(f) && m.face_normal(f).y.abs() > 0.5 {
            m.for_each_face_corner(f, |c| {
                let p = m.position(m.corner_point(c));
                assert!((p.y.abs() - 40.0).abs() < 1e-9);
                assert!(p.x.hypot(p.z) <= 30.0 + 1e-9);
                let u = uv[c as usize];
                assert!(u[1] < 0.5 && u[0] >= 0.0 && u[0] <= 1.0);
            });
        }
    }
    // Wedges pair their fan into quads.
    let wedge = make_cylinder(&CylinderOptions {
        subdiv_axis: 9,
        fraction: 0.5,
        ..Default::default()
    });
    check_closed(&wedge, 2);
    assert_eq!(sides(&wedge, 3), 2);
}

#[test]
fn disc_cap_types_and_a_capped_cone_tip() {
    for axis in [8, 9, 20] {
        let quads = make_disc(&DiscOptions {
            subdiv_axis: axis,
            cap_fill: CapFill::Quads,
            ..Default::default()
        });
        assert!(valid(&quads));
        assert!(uvs_dont_overlap(&quads));
        assert_eq!(sides(&quads, 3), axis % 2);
        assert_eq!(boundary_edge_count(&quads) as i32, axis);
        let fan = make_disc(&DiscOptions {
            subdiv_axis: axis,
            cap_fill: CapFill::Fan,
            ..Default::default()
        });
        assert_eq!(sides(&fan, 3), axis);
        let ngon = make_disc(&DiscOptions {
            subdiv_axis: axis,
            cap_fill: CapFill::Ngon,
            ..Default::default()
        });
        assert_eq!(ngon.num_faces(), 1);
        for d in [&quads, &fan, &ngon] {
            for f in 0..d.face_capacity() {
                if d.face_valid(f) {
                    assert!(approx(d.face_normal(f).y, 1.0));
                }
            }
        }
        // The capped tip leaves no triangles with an even side count, and stays a thin point.
        let tip = make_cone(&ConeOptions {
            subdiv_axis: axis,
            cap_tip: true,
            tip_radius: 0.5,
            ..Default::default()
        });
        check_closed(&tip, 2);
        assert_eq!(sides(&tip, 3), 2 * (axis % 2));
        assert!(approx(tip.bounds().max.y, 50.0));
    }
}

#[test]
fn partial_sweeps() {
    for f in [0.25, 0.5, 0.8] {
        let mut c = CylinderOptions {
            fraction: f,
            ..Default::default()
        };
        let cyl = make_cylinder(&c);
        check_closed(&cyl, 2);
        assert!(signed_volume(&cyl) < signed_volume(&make_cylinder(&CylinderOptions::default())));
        c.subdiv_caps = 2;
        c.subdiv_height = 3;
        check_closed(&make_cylinder(&c), 2);
        c.cut_caps = false;
        let open = make_cylinder(&c);
        assert!(valid(&open));
        assert!(boundary_edge_count(&open) > 0);
        assert!(uvs_dont_overlap(&open));

        check_closed(
            &make_cone(&ConeOptions {
                fraction: f,
                ..Default::default()
            }),
            2,
        );
        check_closed(
            &make_pipe(&PipeOptions {
                fraction: f,
                subdiv_height: 2,
                ..Default::default()
            }),
            2,
        );

        let mut t = TorusOptions {
            fraction: f,
            ..Default::default()
        };
        check_closed(&make_torus(&t), 2);
        t.cut_caps = false;
        let tube = make_torus(&t);
        assert!(valid(&tube));
        assert_eq!(boundary_edge_count(&tube), 2 * 20);

        for rings in [0, 1, 3] {
            let disc = make_disc(&DiscOptions {
                fraction: f,
                subdiv_rings: rings,
                ..Default::default()
            });
            assert!(valid(&disc));
            assert!(uvs_dont_overlap(&disc));
            for fc in 0..disc.face_capacity() {
                assert!(approx(disc.face_normal(fc).y, 1.0));
            }
        }
    }
    // Half a cylinder is cut through its axis, so the cut faces lie in the XY plane.
    let h = make_cylinder(&CylinderOptions {
        fraction: 0.5,
        ..Default::default()
    });
    assert!(approx_scale(h.bounds().max.z, 0.0, 100.0));
    assert!(approx(h.bounds().min.z, -50.0));
}
