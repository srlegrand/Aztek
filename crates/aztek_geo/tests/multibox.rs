//! Tests: multibox.

mod common;

use aztek_geo::bounding::convex_hull_mesh;
use aztek_geo::multibox::*;
use aztek_geo::primitives::*;
use aztek_geo::{Index, Mesh};
use common::approx_eps;
use glam::{DMat4, DVec2, DVec3};

fn prism(poly: &[DVec2], height: f64) -> Mesh {
    let n = poly.len() as Index;
    let mut pts = Vec::new();
    for z in [0.0, height] {
        for p in poly {
            pts.push(DVec3::new(p.x, p.y, z));
        }
    }
    let mut sizes = vec![n];
    let mut idx: Vec<Index> = (0..n).rev().collect();
    sizes.push(n);
    idx.extend(n..2 * n);
    for i in 0..n {
        let j = (i + 1) % n;
        sizes.push(4);
        idx.extend_from_slice(&[i, j, n + j, n + i]);
    }
    Mesh::from_polygons(&pts, &sizes, &idx)
}

fn combine(parts: &[&Mesh]) -> Mesh {
    let mut pts = Vec::new();
    let mut sizes = Vec::new();
    let mut idx = Vec::new();
    for m in parts {
        let mut remap = vec![0 as Index; m.point_capacity() as usize];
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                remap[p as usize] = pts.len() as Index;
                pts.push(m.position(p));
            }
        }
        for f in 0..m.face_capacity() {
            if m.face_valid(f) {
                let fp = m.face_points(f);
                sizes.push(fp.len() as Index);
                idx.extend(fp.iter().map(|&p| remap[p as usize]));
            }
        }
    }
    Mesh::from_polygons(&pts, &sizes, &idx)
}

fn shift(m: &mut Mesh, d: DVec3) {
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            m.set_position(p, m.position(p) + d);
        }
    }
}

fn hull_volume(m: &Mesh) -> f64 {
    let mut sum = 0.0;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            let fp = m.face_points(f);
            for i in 1..fp.len() - 1 {
                sum += m.position(fp[0]).dot(m.position(fp[i]).cross(m.position(fp[i + 1])));
            }
        }
    }
    sum.abs() / 6.0
}

fn v2(x: f64, y: f64) -> DVec2 {
    DVec2::new(x, y)
}

fn l_shape() -> Mesh {
    prism(&[v2(0.0, 0.0), v2(100.0, 0.0), v2(100.0, 20.0), v2(20.0, 20.0), v2(20.0, 100.0), v2(0.0, 100.0)], 20.0)
}

fn inside_some(r: &MultiBoxResult, p: DVec3, tol: f64) -> bool {
    r.chunks
        .iter()
        .any(|c| (0..3).all(|i| (p - c.box_.center).dot(c.box_.axes.col(i)).abs() <= c.box_.size[i] * 0.5 + tol))
}

fn sphere() -> Mesh {
    make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 24, subdiv_height: 12, ..Default::default() })
}

#[test]
fn l_prism_is_two_full_boxes() {
    let r = multi_box_decompose(&l_shape(), &MultiBoxOptions::default());
    assert_eq!(r.chunks.len(), 2);
    for c in &r.chunks {
        assert!(c.fill > 0.95);
    }
    assert!(r.average_fill > 0.95);
    assert!(r.coverage > 0.99);
}

#[test]
fn cube_is_one_box() {
    let r = multi_box_decompose(
        &make_cube(&CubeOptions { width: 100.0, height: 40.0, depth: 60.0, ..Default::default() }),
        &MultiBoxOptions::default(),
    );
    assert_eq!(r.chunks.len(), 1);
    assert!(r.chunks[0].fill > 0.95);
    assert!(approx_eps(r.chunks[0].volume, 100.0 * 40.0 * 60.0, 0.02));
}

#[test]
fn rotated_l_is_still_two_boxes_when_oriented() {
    let mut m = l_shape();
    let xf = DMat4::from_axis_angle(DVec3::new(0.3, 1.0, 0.5).normalize(), 0.6);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            m.set_position(p, xf.transform_point3(m.position(p)));
        }
    }
    let r = multi_box_decompose(&m, &MultiBoxOptions::default());
    assert_eq!(r.chunks.len(), 2);
    for c in &r.chunks {
        assert!(c.fill > 0.85);
    }
}

#[test]
fn max_boxes_is_respected_and_every_vertex_is_covered() {
    let m = sphere();
    for limit in [1, 2, 5] {
        let r = multi_box_decompose(&m, &MultiBoxOptions { max_boxes: limit, min_fill: 0.95, ..Default::default() });
        assert!(r.chunks.len() as i32 <= limit);
        assert!(!r.chunks.is_empty());
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                assert!(inside_some(&r, m.position(p), 1e-6));
            }
        }
    }
    let l = l_shape();
    let r = multi_box_decompose(&l, &MultiBoxOptions { oriented: false, padding: 2.0, ..Default::default() });
    for p in 0..l.point_capacity() {
        if l.point_valid(p) {
            assert!(inside_some(&r, l.position(p), 1e-6));
        }
    }
}

#[test]
fn hull_chunks_and_determinism() {
    let m = sphere();
    let o = MultiBoxOptions { shape: "hull".into(), ..Default::default() };
    let a = multi_box_decompose(&m, &o);
    let b = multi_box_decompose(&m, &o);
    assert_eq!(a.chunks.len(), b.chunks.len());
    let mut hulls = 0;
    for i in 0..a.chunks.len() {
        hulls += a.chunks[i].hull as usize;
        assert_eq!(a.chunks[i].hull, a.chunks[i].hull_mesh.num_faces() > 0);
        assert_eq!(a.chunks[i].box_.center, b.chunks[i].box_.center);
        assert_eq!(a.chunks[i].box_.size, b.chunks[i].box_.size);
        assert_eq!(a.chunks[i].fill, b.chunks[i].fill);
    }
    assert!(hulls > 0);
    assert_eq!(a.coverage, b.coverage);
}

#[test]
fn overlapping_closed_shells_and_open_meshes_are_filled() {
    let a = make_cube(&CubeOptions { width: 60.0, height: 60.0, depth: 60.0, ..Default::default() });
    let mut b = a.clone();
    shift(&mut b, DVec3::new(30.0, 0.0, 0.0));
    let both = combine(&[&a, &b]);
    let r = multi_box_decompose(&both, &MultiBoxOptions::default());
    assert_eq!(r.chunks.len(), 1);
    assert!(r.chunks[0].fill > 0.95);
}

#[test]
fn wedge_base_with_thin_bar_on_its_ridge_is_a_hull_and_a_box() {
    let base = prism(&[v2(-50.0, 0.0), v2(50.0, 0.0), v2(20.0, 30.0), v2(-20.0, 30.0)], 200.0);
    let mut bar = prism(&[v2(-4.0, 30.0), v2(4.0, 30.0), v2(4.0, 38.0), v2(-4.0, 38.0)], 180.0);
    shift(&mut bar, DVec3::new(0.0, 0.0, 10.0));
    let ship = combine(&[&base, &bar]);
    let r = multi_box_decompose(&ship, &MultiBoxOptions { max_boxes: 2, shape: "auto".into(), ..Default::default() });
    assert_eq!(r.chunks.len(), 2);
    let (mut wedge, mut thin) = (&r.chunks[0], &r.chunks[1]);
    if wedge.box_.size.x * wedge.box_.size.z < thin.box_.size.x * thin.box_.size.z {
        std::mem::swap(&mut wedge, &mut thin);
    }
    assert!(thin.fill > 0.9);
    assert!(thin.box_.size.y < 15.0);
    assert!(wedge.hull);
    assert!(approx_eps(hull_volume(&wedge.hull_mesh), 2100.0 * 200.0, 0.1));
    // The two shapes meet without one swallowing the other.
    let lo = wedge.box_.center.y - wedge.box_.size.y * 0.5;
    let hi = wedge.box_.center.y + wedge.box_.size.y * 0.5;
    assert!(thin.box_.center.y - thin.box_.size.y * 0.5 > hi - 6.0);
    assert!(lo < 3.0);
}

#[test]
fn tapered_flat_base_with_bar_thinner_than_a_voxel() {
    let corners = [
        DVec3::new(-60.0, 0.0, -150.0),
        DVec3::new(60.0, 0.0, -150.0),
        DVec3::new(30.0, 0.0, 150.0),
        DVec3::new(-30.0, 0.0, 150.0),
        DVec3::new(-40.0, 25.0, -130.0),
        DVec3::new(40.0, 25.0, -130.0),
        DVec3::new(15.0, 25.0, 130.0),
        DVec3::new(-15.0, 25.0, 130.0),
    ];
    let base = convex_hull_mesh(&corners, 32, 0.0);
    let mut bar = make_cube(&CubeOptions { width: 5.0, height: 5.0, depth: 260.0, ..Default::default() });
    shift(&mut bar, DVec3::new(0.0, 27.5, 0.0));
    let r = multi_box_decompose(&combine(&[&base, &bar]), &MultiBoxOptions { max_boxes: 2, shape: "auto".into(), ..Default::default() });
    assert_eq!(r.chunks.len(), 2);
    let (mut wedge, mut thin) = (&r.chunks[0], &r.chunks[1]);
    if wedge.box_.size.x * wedge.box_.size.y < thin.box_.size.x * thin.box_.size.y {
        std::mem::swap(&mut wedge, &mut thin);
    }
    assert!(thin.fill > 0.9);
    assert!(thin.box_.size.x < 15.0);
    assert!(thin.box_.size.y < 15.0);
    assert!(wedge.hull);
    assert!(approx_eps(hull_volume(&wedge.hull_mesh), hull_volume(&base), 0.1));
    assert!(thin.box_.center.y - thin.box_.size.y * 0.5 > 20.0);
}
