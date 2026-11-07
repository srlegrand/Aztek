//! Tests: reduce.

mod corpus;

use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

use aztek_geo::normals::{corner_normals, CornerNormalOptions};
use aztek_geo::primitives::{make_sphere, SphereOptions};
use aztek_geo::reduce::{reduce_mesh, ReduceOptions};
use aztek_geo::triangulate::TriangulateOptions;
use aztek_geo::ops::triangulate_faces;
use aztek_geo::{Index, Mesh};
use corpus::*;

fn key(p: glam::DVec3) -> [u64; 3] {
    [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()]
}

/// Corners whose uv isn't one the source had at that point: interpolated or swapped uvs make textures swim.
fn foreign_uvs(src: &Mesh, out: &Mesh) -> Index {
    let a = src.corner_attrs().get::<[f32; 2]>("uv0");
    let b = out.corner_attrs().get::<[f32; 2]>("uv0");
    if a.is_empty() {
        return 0;
    }
    if b.is_empty() {
        return 1;
    }
    let mut at: BTreeMap<[u64; 3], Vec<[f32; 2]>> = BTreeMap::new();
    for f in all_faces(src) {
        src.for_each_face_corner(f, |c| at.entry(key(src.position(src.corner_point(c)))).or_default().push(a[c as usize]));
    }
    let mut bad = 0;
    for f in all_faces(out) {
        out.for_each_face_corner(f, |c| {
            let list = at.get(&key(out.position(out.corner_point(c))));
            bad += !list.is_some_and(|l| l.contains(&b[c as usize])) as Index;
        });
    }
    bad
}

/// Faces whose uv triangle winds against the majority, i.e. mirrored or folded in uv space.
fn uv_flips(m: &Mesh) -> Index {
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0");
    if uv.is_empty() {
        return 0;
    }
    let (mut pos, mut neg) = (0, 0);
    let g = |c: Index| glam::DVec2::new(uv[c as usize][0] as f64, uv[c as usize][1] as f64);
    for f in all_faces(m) {
        let mut area = 0.0;
        let c0 = m.face_corner(f);
        let mut c = m.corner_next(c0);
        while m.corner_next(c) != c0 {
            let (p, q, r) = (g(c0), g(c), g(m.corner_next(c)));
            area += (q.x - p.x) * (r.y - p.y) - (q.y - p.y) * (r.x - p.x);
            c = m.corner_next(c);
        }
        pos += (area > 1e-12) as Index;
        neg += (area < -1e-12) as Index;
    }
    pos.min(neg)
}

fn border_points(m: &Mesh) -> BTreeSet<[u64; 3]> {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p) && m.is_boundary_point(p)).map(|p| key(m.position(p))).collect()
}

#[test]
fn corpus_reduce() {
    let mut failures = Vec::new();
    for c in all() {
        for (quads, ratio) in [(false, 0.5), (true, 0.5), (false, 0.15)] {
            let r = reduce_mesh(&c.mesh, &ReduceOptions { ratio, quads, ..Default::default() });
            let m = &r.mesh;
            let mut p = introduced(&c.mesh, m);
            let before = triangle_count(&c.mesh);
            let after = triangle_count(m);
            if after > before {
                p.push(format!("more triangles {before} -> {after}"));
            }
            if !quads
                && boundary_edges(&c.mesh) == 0
                && before >= 200
                && (after as f64 - ratio * before as f64).abs() > 0.05 * before as f64
            {
                p.push(format!("missed target: {before} -> {after}"));
            }
            // A thin wall cut down to a handful of triangles crosses itself, which reads as flipped.
            let f = if after >= 100 { flipped_faces(m, &c.mesh, -0.2) } else { 0 };
            if f > 0 {
                p.push(format!("{f} flipped faces"));
            }
            // Halving a coarse mesh has to cut corners; a dense one must keep its shape.
            let h = hausdorff(&c.mesh, m);
            if h > if before >= 500 { if ratio < 0.5 { 0.04 } else { 0.015 } } else { 0.3 } {
                p.push(format!("hausdorff {h}"));
            }
            if border_points(m) != border_points(&c.mesh) {
                p.push("border points changed".into());
            }
            if (boundary_length(m) - boundary_length(&c.mesh)).abs() > 1e-9 * (1.0 + boundary_length(&c.mesh)) {
                p.push("border length changed".into());
            }
            let u = foreign_uvs(&c.mesh, m);
            if u > 0 {
                p.push(format!("{u} corners with uvs the source didn't have"));
            }
            let mut triangulated = c.mesh.clone();
            let faces = all_faces(&triangulated);
            triangulate_faces(&mut triangulated, &faces, &TriangulateOptions::default());
            if !quads && uv_flips(m) > uv_flips(&triangulated) {
                p.push(format!("uv flips {} -> {}", uv_flips(&triangulated), uv_flips(m)));
            }
            if !p.is_empty() {
                failures.push(format!("{} quads={quads} ratio={ratio}: {}", c.name, join(&p)));
            }
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn reduce_hits_counts_keeps_hard_edges_and_symmetry() {
    let mut sphere = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 64, subdiv_height: 64, ..Default::default() });
    sphere.compact(None);
    for target in [2000, 500, 100] {
        let got = triangle_count(&reduce_mesh(&sphere, &ReduceOptions { target_triangles: target, ..Default::default() }).mesh);
        assert!((got as f64 - target as f64).abs() <= 0.05 * target as f64 + 2.0, "{target} -> {got}");
    }

    // Faces of a hard-edged cube stay flat-shaded: every corner normal matches its face.
    let cube = hard_cube();
    let reduced = reduce_mesh(&cube, &ReduceOptions { ratio: 0.3, ..Default::default() }).mesh;
    let normals = corner_normals(&reduced, &CornerNormalOptions::default());
    let mut soft = 0;
    for f in all_faces(&reduced) {
        reduced.for_each_face_corner(f, |c| soft += (normals[c as usize].as_dvec3().dot(reduced.face_normal(f)) < 0.999) as i32);
    }
    assert_eq!(soft, 0);
    assert!(triangle_count(&reduced) < triangle_count(&cube));

    // Symmetric: one half reduced and mirrored, so every point has an exact mirror image.
    let sym_src = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 40, subdiv_height: 30, ..Default::default() });
    let sr = reduce_mesh(&sym_src, &ReduceOptions { ratio: 0.3, symmetry_axis: 0, ..Default::default() });
    assert!(!sr.symmetry_ignored);
    let pts: BTreeSet<[u64; 3]> = (0..sr.mesh.point_capacity()).map(|p| key(sr.mesh.position(p))).collect();
    let unmatched = pts
        .iter()
        .filter(|q| {
            let x = f64::from_bits(q[0]);
            let mx = if x == 0.0 { 0.0 } else { -x };
            !pts.contains(&[mx.to_bits(), q[1], q[2]])
        })
        .count();
    assert_eq!(unmatched, 0);
    assert_eq!(boundary_edges(&sr.mesh), 0);
    assert!(introduced(&sym_src, &sr.mesh).is_empty());
}

#[test]
fn reduce_dense_mesh_timing() {
    let mut dense = noisy_sphere(360, 0.002, 2);
    dense.compact(None);
    let start = Instant::now();
    let m = reduce_mesh(&dense, &ReduceOptions { ratio: 0.1, ..Default::default() }).mesh;
    println!("{} -> {} triangles in {} s", triangle_count(&dense), triangle_count(&m), start.elapsed().as_secs_f64());
    assert!(introduced(&dense, &m).is_empty());
    assert_eq!(flipped_faces(&m, &dense, -0.2), 0);
    assert!(hausdorff(&dense, &m) < 0.01);
}
