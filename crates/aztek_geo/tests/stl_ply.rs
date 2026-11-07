//! Tests: stl ply.

mod common;

use std::collections::BTreeMap;

use aztek_geo::colors;
use aztek_geo::names;
use aztek_geo::ply::*;
use aztek_geo::primitives::*;
use aztek_geo::stl::*;
use aztek_geo::triangulate::{triangulate, TriangulateOptions};
use aztek_geo::{Mesh, INVALID};
use common::*;
use glam::DVec3;

fn cube() -> Vec<StlSolid> {
    vec![StlSolid {
        name: "box".into(),
        mesh: make_cube(&CubeOptions { width: 10.0, height: 20.0, depth: 30.0, ..Default::default() }),
    }]
}

fn stl(data: &[u8]) -> StlReadResult {
    read_stl(data, &StlReadOptions::default())
}

fn ply(data: &[u8]) -> PlyReadResult {
    read_ply(data, &PlyReadOptions::default())
}

#[test]
fn stl_binary_round_trip_welds_a_closed_mesh() {
    let data = write_stl(&cube(), &StlWriteOptions::default());
    assert_eq!(data.len(), 84 + 12 * 50);
    assert_ne!(&data[..5], b"solid");
    let r = stl(&data);
    assert!(r.ok());
    assert!(r.binary);
    assert_eq!(r.solids.len(), 1);
    let m = &r.solids[0].mesh;
    assert_eq!(m.num_points(), 8);
    assert_eq!(m.num_faces(), 12);
    assert_eq!(boundary_edge_count(m), 0);
    assert!(valid(m));
    assert!(approx(signed_volume(m), 6000.0));
    // The cube's edges, not the triangle diagonals.
    assert_eq!(hard_count(m), 12);
}

#[test]
fn stl_ascii_round_trip_keeps_solids_apart() {
    let mut solids = cube();
    solids.push(StlSolid {
        name: "second".into(),
        mesh: make_cube(&CubeOptions { width: 1.0, height: 1.0, depth: 1.0, ..Default::default() }),
    });
    let text = write_stl(&solids, &StlWriteOptions { binary: false, ..Default::default() });
    assert!(text.starts_with(b"solid box\n"));
    let r = stl(&text);
    assert!(r.ok());
    assert!(!r.binary);
    assert_eq!(r.solids.len(), 2);
    assert_eq!(r.solids[0].name, "box");
    assert_eq!(r.solids[1].name, "second");
    assert_eq!(r.solids[1].mesh.num_points(), 8);
    assert!(approx(signed_volume(&r.solids[1].mesh), 1.0));
}

#[test]
fn stl_binary_whose_header_starts_with_solid_still_reads_as_binary() {
    let mut data = write_stl(&cube(), &StlWriteOptions::default());
    data[..9].copy_from_slice(b"solid box");
    let r = stl(&data);
    assert!(r.ok());
    assert!(r.binary);
    assert_eq!(r.solids[0].mesh.num_faces(), 12);
}

#[test]
fn stl_no_weld_leaves_separate_triangles() {
    let r = read_stl(&write_stl(&cube(), &StlWriteOptions::default()), &StlReadOptions { weld: false, ..Default::default() });
    assert!(r.ok());
    assert_eq!(r.solids[0].mesh.num_points(), 36);
}

#[test]
fn stl_per_triangle_colors_round_trip() {
    let mut s = cube();
    colors::add(&mut s[0].mesh, "colorSet", [1.0, 0.0, 0.0, 1.0]);
    let r = stl(&write_stl(&s, &StlWriteOptions { colors: true, ..Default::default() }));
    assert!(r.ok());
    assert!(r.colors);
    let c = colors::values(&r.solids[0].mesh, "colorSet");
    assert!(!c.is_empty());
    assert!(approx(c[0][0] as f64, 1.0));
    assert!(approx(c[0][1] as f64, 0.0));
}

#[test]
fn stl_garbage_and_truncation_fail_cleanly() {
    assert!(!stl(b"hello").ok());
    let mut data = write_stl(&cube(), &StlWriteOptions::default());
    data.truncate(200);
    assert!(!stl(&data).ok());
}

#[test]
fn ply_round_trip_keeps_polygons_uvs_normals_and_colors() {
    let mut m = make_cube(&CubeOptions { width: 10.0, height: 10.0, depth: 10.0, ..Default::default() });
    colors::add(&mut m, "colorSet", [0.25, 0.5, 1.0, 1.0]);
    for binary in [true, false] {
        let r = ply(&write_ply(&m, &PlyWriteOptions { binary, ..Default::default() }));
        assert!(r.ok(), "{}", r.error);
        assert!(!r.point_cloud);
        assert_eq!(r.mesh.num_faces(), 6);
        assert_eq!(r.mesh.face_size(0), 4);
        assert!(r.mesh.corner_attrs().has(names::NORMAL));
        assert!(r.mesh.corner_attrs().has(&names::uv(0)));
        let c = colors::values(&r.mesh, "colorSet");
        assert!(!c.is_empty());
        assert!(approx(c[0][2] as f64, 1.0));
        assert!(approx_eps(c[0][1] as f64, 0.5, 0.01));
        assert!(approx(signed_volume(&r.mesh), 1000.0));
    }
}

#[test]
fn ply_without_split_attributes_the_mesh_stays_welded() {
    let m = make_cube(&CubeOptions { width: 10.0, height: 10.0, depth: 10.0, ..Default::default() });
    let r = ply(&write_ply(&m, &PlyWriteOptions { normals: false, uvs: false, colors: false, ..Default::default() }));
    assert!(r.ok());
    assert_eq!(r.mesh.num_points(), 8);
    assert_eq!(boundary_edge_count(&r.mesh), 0);
    assert!(valid(&r.mesh));
}

#[test]
fn ply_ascii_point_cloud_with_extra_properties() {
    let text = "ply\r\nformat ascii 1.0\r\ncomment made by hand\r\nelement vertex 3\r\nproperty float x\r\nproperty float y\r\n\
                property float z\r\nproperty uchar red\r\nproperty uchar green\r\nproperty uchar blue\r\nproperty float quality\r\n\
                end_header\r\n0 0 0 255 0 0 0.5\r\n1 0 0 0 255 0 1\r\n0 1 0 0 0 255 2\r\n";
    let r = ply(text.as_bytes());
    assert!(r.ok(), "{}", r.error);
    assert!(r.point_cloud);
    assert_eq!(r.mesh.num_points(), 3);
    let c = r.mesh.point_attrs().get::<[f32; 4]>(names::COLOR);
    assert_eq!(c.len(), 3);
    assert!(approx(c[1][1] as f64, 1.0));
    let q = r.mesh.point_attrs().get::<f32>("quality");
    assert_eq!(q.len(), 3);
    assert!(approx(q[2] as f64, 2.0));

    let back = ply(&write_ply(&r.mesh, &PlyWriteOptions::default()));
    assert!(back.ok());
    assert!(back.point_cloud);
    assert_eq!(back.mesh.num_points(), 3);
    assert!(approx(back.mesh.point_attrs().get::<[f32; 4]>(names::COLOR)[1][1] as f64, 1.0));
}

#[test]
fn ply_big_endian_int_indices_texcoord_lists_face_colors_and_skipped_elements() {
    let be32 = |s: &mut Vec<u8>, v: u32| s.extend_from_slice(&v.to_be_bytes());
    let bef = |s: &mut Vec<u8>, f: f32| s.extend_from_slice(&f.to_bits().to_be_bytes());
    let mut s = b"ply\nformat binary_big_endian 1.0\ncomment TextureFile tex.png\nelement vertex 4\nproperty float x\n\
property float y\nproperty float z\nelement face 2\nproperty list uchar uint vertex_indices\n\
property list uchar float texcoord\nproperty uchar red\nproperty uchar green\nproperty uchar blue\n\
element material 1\nproperty int id\nend_header\n"
        .to_vec();
    let pts = [[0.0f32, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    for p in &pts {
        for &v in p {
            bef(&mut s, v);
        }
    }
    let faces = [[0u32, 1, 2], [0, 2, 3]];
    for (f, face) in faces.iter().enumerate() {
        s.push(3);
        for &i in face {
            be32(&mut s, i);
        }
        s.push(6);
        for &i in face {
            bef(&mut s, pts[i as usize][0]);
            bef(&mut s, pts[i as usize][1]);
        }
        s.push(if f > 0 { 0 } else { 255 });
        s.push(0);
        s.push(if f > 0 { 255 } else { 0 });
    }
    be32(&mut s, 7);
    let r = ply(&s);
    assert!(r.ok(), "{}", r.error);
    assert_eq!(r.mesh.num_faces(), 2);
    assert_eq!(r.mesh.num_points(), 4);
    assert_eq!(r.texture_files, vec!["tex.png"]);
    assert!(r.skipped.iter().any(|x| x == "element material"));
    let uv = r.mesh.corner_attrs().get::<[f32; 2]>(&names::uv(0));
    assert!(!uv.is_empty());
    for c in 0..r.mesh.corner_capacity() {
        let p = r.mesh.position(r.mesh.corner_point(c));
        assert!(approx(uv[c as usize][0] as f64, p.x));
        assert!(approx(uv[c as usize][1] as f64, p.y));
    }
    let col = colors::values(&r.mesh, "colorSet");
    assert!(!col.is_empty());
    assert!(approx(col[r.mesh.face_corner(0) as usize][0] as f64, 1.0));
    assert!(approx(col[r.mesh.face_corner(1) as usize][2] as f64, 1.0));
}

#[test]
fn ply_bad_files_fail_cleanly() {
    assert!(!ply(b"not a ply").ok());
    assert!(!ply(b"ply\nformat ascii 1.0\nelement vertex 2\nproperty float x\nproperty float y\nproperty float z\nend_header\n0 0 0\n").ok());
    assert!(!ply(b"ply\nformat ascii 1.0\nelement vertex 1\nproperty float x\nend_header\n0\n").ok());
}

/// Big enough that every reader and mesh build step takes its parallel path.
fn big_sphere() -> Mesh {
    make_sphere(&SphereOptions { radius: 10.0, subdiv_axis: 150, subdiv_height: 150, ..Default::default() })
}

/// Each face's corner positions in order, the same for any point numbering.
fn face_shapes(m: &Mesh) -> Vec<Vec<DVec3>> {
    (0..m.face_capacity())
        .filter(|&f| m.face_valid(f))
        .map(|f| m.face_points(f).iter().map(|&p| m.position(p)).collect())
        .collect()
}

/// Positions in order of first use by the faces, as a reader adding points one at a time would number them.
fn first_use(faces: &[Vec<DVec3>]) -> Vec<DVec3> {
    let mut out = Vec::new();
    let mut seen = BTreeMap::new();
    for f in faces {
        for p in f {
            let key = [p.x.to_bits(), p.y.to_bits(), p.z.to_bits()];
            if seen.insert(key, out.len()).is_none() {
                out.push(*p);
            }
        }
    }
    out
}

fn positions(m: &Mesh) -> Vec<DVec3> {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| m.position(p)).collect()
}

/// Positions rounded to float, as STL and PLY store them.
fn as_float(mut m: Mesh) -> Mesh {
    for p in m.positions_mut() {
        *p = p.as_vec3().as_dvec3();
    }
    m
}

#[test]
fn stl_big_binary_and_ascii_files_weld_in_first_use_order() {
    let sphere = as_float(big_sphere());
    let solids = vec![StlSolid { name: "ball".into(), mesh: sphere.clone() }];
    for binary in [true, false] {
        let r = stl(&write_stl(&solids, &StlWriteOptions { binary, ..Default::default() }));
        assert!(r.ok());
        assert_eq!(r.solids.len(), 1);
        let m = &r.solids[0].mesh;
        assert!(valid(m));
        assert_eq!(boundary_edge_count(m), 0);
        assert_eq!(m.num_points(), sphere.num_points());
        let faces = face_shapes(m);
        assert!(positions(m) == first_use(&faces));
        assert_eq!(faces.len() as u32, triangulate(&sphere, &TriangulateOptions::default()).triangle_count());
    }
}

#[test]
fn stl_big_binary_drops_degenerate_triangles_and_the_points_only_they_used() {
    let mut s = vec![0u8; 80];
    let n: u32 = 30000;
    s.extend_from_slice(&n.to_le_bytes());
    let mut tri = |a: [f32; 3], b: [f32; 3], c: [f32; 3]| {
        let v = [0.0, 0.0, 1.0, a[0], a[1], a[2], b[0], b[1], b[2], c[0], c[1], c[2]];
        for x in v {
            s.extend_from_slice(&f32::to_le_bytes(x));
        }
        s.extend_from_slice(&[0, 0]);
    };
    for i in 0..n {
        let x = i as f32;
        if i % 1000 == 7 {
            // Two corners at one position.
            tri([x, 0.0, 0.0], [x, 5.0, 0.0], [x, 0.0, 0.0]);
        } else {
            tri([x, 0.0, 0.0], [x + 1.0, 0.0, 0.0], [x, 1.0, 0.0]);
        }
    }
    let r = stl(&s);
    assert!(r.ok());
    let m = &r.solids[0].mesh;
    assert_eq!(m.num_faces(), n - 30);
    assert!(valid(m));
    for p in 0..m.point_capacity() {
        assert_ne!(m.point_edge(p), INVALID);
    }
    assert!(positions(m) == first_use(&face_shapes(m)));
}

#[test]
fn stl_ascii_with_everything_on_one_line_still_reads() {
    let r = stl(b"solid t facet normal 0 0 1 outer loop vertex 0 0 0 vertex 1 0 0 vertex 0 1 0 endloop endfacet endsolid t");
    assert!(r.ok());
    assert_eq!(r.solids.len(), 1);
    assert_eq!(
        r.solids[0].name,
        "t facet normal 0 0 1 outer loop vertex 0 0 0 vertex 1 0 0 vertex 0 1 0 endloop endfacet endsolid t"
    );
}

#[test]
fn ply_big_binary_and_ascii_files_read_the_same_mesh() {
    let sphere = as_float(big_sphere());
    let want = face_shapes(&sphere);
    for binary in [true, false] {
        let r = ply(&write_ply(
            &sphere,
            &PlyWriteOptions { binary, normals: false, uvs: false, colors: false, ..Default::default() },
        ));
        assert!(r.ok());
        assert!(valid(&r.mesh));
        assert_eq!(r.mesh.num_points(), sphere.num_points());
        assert!(face_shapes(&r.mesh) == want);
    }
}

#[test]
fn ply_big_ascii_with_mixed_polygons_bad_faces_and_face_properties() {
    let faces = 20000usize;
    let mut s = format!(
        "ply\nformat ascii 1.0\nelement vertex {}\nproperty float x\nproperty float y\nproperty float z\nelement face {}\n\
         property list uchar int vertex_indices\nproperty float id\nend_header\n",
        faces * 4,
        faces
    );
    for i in 0..faces {
        s += &format!("{i} 0 0\n{i} 1 0\n{i} 1 1\n{i} 0 1\n");
    }
    for i in 0..faces {
        let b = i * 4;
        if i == 123 {
            s += &format!("3 {} {} {} {}\n", b, b + 1, faces * 4 + 5, i); // out of range
        } else if i == 9876 {
            s += &format!("3 {} {} {} {}\n", b, b + 1, b, i); // repeats a point
        } else if i % 2 == 1 {
            s += &format!("3 {} {} {} {}\n", b, b + 1, b + 2, i);
        } else {
            s += &format!("4 {} {} {} {} {}\n", b, b + 1, b + 2, b + 3, i);
        }
    }
    let r = ply(s.as_bytes());
    assert!(r.ok());
    assert_eq!(r.mesh.num_faces() as usize, faces - 2);
    assert!(r.skipped.iter().any(|x| x == "2 bad faces"));
    let id = r.mesh.face_attrs().get::<f32>("id");
    assert_eq!(id.len(), faces - 2);
    let mut expect = 0usize;
    for f in 0..r.mesh.face_capacity() {
        if expect == 123 || expect == 9876 {
            expect += 1;
        }
        assert_eq!(id[f as usize], expect as f32);
        assert_eq!(r.mesh.face_size(f), if expect % 2 == 1 { 3 } else { 4 });
        assert_eq!(r.mesh.position(r.mesh.corner_point(r.mesh.face_corner(f))).x, expect as f64);
        expect += 1;
    }
    // Points no kept face uses go: the triangles' fourth, and all of the bad faces'.
    assert_eq!(r.mesh.num_points() as usize, (faces / 2 - 1) * 4 + (faces / 2 - 1) * 3);
}

#[test]
fn ply_ascii_records_broken_across_lines() {
    let text = "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\n\
                property list uchar int vertex_indices\nend_header\n0 0\n0 1 0 0 0\n1 0 3\n0 1 2\n";
    let r = ply(text.as_bytes());
    assert!(r.ok(), "{}", r.error);
    assert_eq!(r.mesh.num_faces(), 1);
    assert_eq!(r.mesh.position(1), DVec3::new(1.0, 0.0, 0.0));
}

#[test]
fn ply_big_binary_point_cloud() {
    let mut cloud = Mesh::new();
    for i in 0..100000 {
        cloud.add_point(DVec3::new(i as f64, (i % 7) as f64, 0.5));
    }
    let r = ply(&write_ply(&cloud, &PlyWriteOptions::default()));
    assert!(r.ok());
    assert!(r.point_cloud);
    assert!(positions(&r.mesh) == positions(&cloud));
}
