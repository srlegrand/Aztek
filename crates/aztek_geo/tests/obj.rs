//! Tests: obj.

mod common;

use std::time::Instant;

use aztek_geo::io_common::{general, parse_double};
use aztek_geo::names;
use aztek_geo::obj::*;
use aztek_geo::primitives::*;
use common::*;
use rand_mt::Mt64;

fn read(text: &str) -> ObjReadResult {
    read_obj(text.as_bytes(), &ObjReadOptions::default())
}

#[test]
fn parse_basics() {
    let text = "# test
mtllib test.mtl
o first
v 0 0 0
v 1 0 0
v 1 1 0
v 0 1 0
v 2 0 0
vt 0 0
vt 1 0
vt 1 1
vt 0 1
vn 0 0 1
g top
usemtl red
s 1
f 1/1/1 2/2/1 3/3/1 4/4/1
usemtl blue
f -4/1/1 -1/2/1 -3/3/1
o second
v 5 5 5
v 6 5 5
v 6 6 5
f 6 7 8
f 6 7
";
    let r = read(text);
    assert!(r.ok());
    assert_eq!(r.objects.len(), 2);
    assert_eq!(r.mtllibs.len(), 1);
    let a = &r.objects[0].mesh;
    assert_eq!(r.objects[0].name, "first");
    assert_eq!(a.num_faces(), 2);
    assert_eq!(a.num_points(), 5);
    assert!(valid(a));
    assert_eq!(a.material_names().len(), 2);
    assert_eq!(a.material_names()[1], "blue");
    let mat = a.face_attrs().get::<i32>(names::MATERIAL);
    assert_eq!(mat[0], 0);
    assert_eq!(mat[1], 1);
    assert!(a.face_attrs().has("group"));
    assert_eq!(r.objects[0].group_names[0], "top");
    let uv = a.corner_attrs().get::<[f32; 2]>("uv0");
    assert_eq!(uv.len(), 7);
    assert_eq!(uv[2], [1.0, 1.0]);
    assert_eq!(a.face_size(1), 3);

    let b = &r.objects[1].mesh;
    assert_eq!(r.objects[1].name, "second");
    assert_eq!(b.num_faces(), 1);
    assert_eq!(b.num_points(), 3);
    assert_eq!(b.position(0).x, 5.0);
}

#[test]
fn round_trip_keeps_topology_uvs_hard_edges_and_materials() {
    let mut cyl = make_cylinder(&CylinderOptions {
        radius: 50.0,
        height: 100.0,
        subdiv_axis: 12,
        subdiv_height: 2,
        cap_fill: CapFill::Ngon,
        ..Default::default()
    });
    *cyl.material_names_mut() = vec!["side".into(), "cap".into()];
    let sizes: Vec<u32> = (0..cyl.face_capacity()).map(|f| cyl.face_size(f)).collect();
    let mat = cyl.face_attrs_mut().get_or_add::<i32>(names::MATERIAL);
    for (f, m) in mat.iter_mut().enumerate() {
        *m = if sizes[f] > 4 { 1 } else { 0 };
    }

    let text = write_obj(&cyl, &ObjWriteOptions::default());
    let r = read(&text);
    assert_eq!(r.objects.len(), 1);
    let m = &r.objects[0].mesh;
    assert!(valid(m));
    assert_eq!(m.num_points(), cyl.num_points());
    assert_eq!(m.num_faces(), cyl.num_faces());
    assert_eq!(m.num_edges(), cyl.num_edges());
    assert_eq!(hard_count(m), hard_count(&cyl));
    assert_eq!(m.material_names().len(), 2);
    for p in 0..m.point_capacity() {
        assert_eq!(m.position(p), cyl.position(p));
    }
    let uv_a = cyl.corner_attrs().get::<[f32; 2]>("uv0");
    let uv_b = m.corner_attrs().get::<[f32; 2]>("uv0");
    for c in 0..m.corner_capacity() as usize {
        assert_eq!(uv_a[c], uv_b[c]);
    }
    let mat_a = cyl.face_attrs().get::<i32>(names::MATERIAL);
    let mat_b = m.face_attrs().get::<i32>(names::MATERIAL);
    for f in 0..m.face_capacity() as usize {
        assert_eq!(m.material_names()[mat_b[f] as usize], cyl.material_names()[mat_a[f] as usize]);
    }

    // The text itself is stable.
    assert_eq!(write_obj(m, &ObjWriteOptions::default()), text);
}

#[test]
fn hard_edges_from_normals_without_smoothing_groups() {
    let cube = make_cube(&CubeOptions::default());
    let wo = ObjWriteOptions { smoothing_groups: false, ..Default::default() };
    let r = read(&write_obj(&cube, &wo));
    assert_eq!(r.objects.len(), 1);
    assert_eq!(hard_count(&r.objects[0].mesh), 12);
}

fn uniform(rng: &mut Mt64, lo: f64, hi: f64) -> f64 {
    lo + (hi - lo) * ((rng.next_u64() >> 11) as f64 / (1u64 << 53) as f64)
}

/// printf "%.{n}e".
fn sci(v: f64, n: usize) -> String {
    let s = format!("{v:.n$e}");
    let (m, e) = s.split_once('e').unwrap();
    let e: i32 = e.parse().unwrap();
    format!("{m}e{}{:02}", if e < 0 { '-' } else { '+' }, e.abs())
}

#[test]
fn coordinates_read_exactly_as_from_chars_reads_them() {
    let mut rng = Mt64::new(7);
    let mut numbers: Vec<String> = [
        "0", "-0", "1.", ".5", "-.25", "1e5", "1E-7", "123456789012345", "1234567890123456", "0.000000000000000000001",
        "1e22", "1e23", "1e-22", "1e-23", "007.50", "-0.0",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect();
    for i in 0..3000 {
        let a = uniform(&mut rng, -30.0, 30.0);
        let b = uniform(&mut rng, -30.0, 30.0);
        let v = if i % 3 == 0 { -1.0 } else { 1.0 } * 10f64.powf(a / 5.0) * b;
        numbers.push(match i % 8 {
            0 => format!("{v:.6}"),
            1 => general(v, 9),
            2 => general(v, 17),
            3 => sci(v, 3),
            4 => general(v, 15),
            5 => general(v, 6),
            6 => format!("{v:.0}"),
            _ => format!("{v:.12}"),
        });
    }
    while numbers.len() % 3 != 0 {
        numbers.push("1".into());
    }
    let points = numbers.len() / 3;
    // Points no face uses are dropped, so every three points get a face.
    let mut all = String::new();
    for i in (0..numbers.len()).step_by(3) {
        all += &format!("v {} {} {}\n", numbers[i], numbers[i + 1], numbers[i + 2]);
    }
    let mut i = 0;
    while i + 2 < points {
        all += &format!("f {} {} {}\n", i + 1, i + 2, i + 3);
        i += 3;
    }
    let every = read(&all);
    assert_eq!(every.objects.len(), 1);
    let m = &every.objects[0].mesh;
    assert_eq!(m.point_capacity() as usize, points - points % 3);
    for p in 0..m.point_capacity() {
        for c in 0..3 {
            let s = &numbers[p as usize * 3 + c];
            let b = s.strip_prefix('+').unwrap_or(s);
            let want = parse_double(b.as_bytes()).map_or(0.0, |(v, _)| v);
            let want_std: f64 = b.parse().unwrap();
            assert_eq!(want.to_bits(), want_std.to_bits(), "{s}");
            assert_eq!(want.to_bits(), m.position(p)[c].to_bits(), "{s}");
        }
    }
}

#[test]
fn lines_without_faces_report_an_error() {
    let r = read("v 0 0 0\r\nv 1e-15 0 0\r\nv 0 1 0\r\nv 0 0 1\r\nl 1 2\r\nl 3 4\r\n");
    assert!(!r.ok());
    assert!(r.objects.is_empty());
    assert!(r.error.contains("2 line elements"));

    let mixed = read("v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nl 1 2\n");
    assert!(mixed.ok());
    assert_eq!(mixed.objects.len(), 1);
}

#[test]
#[ignore = "perf"]
fn perf_obj_1m_faces() {
    let m = make_plane(&PlaneOptions { width: 1000.0, depth: 1000.0, subdiv_x: 1000, subdiv_z: 1000, ..Default::default() });
    let t0 = Instant::now();
    let text = write_obj(&m, &ObjWriteOptions::default());
    let t_write = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let r = read(&text);
    let t_read = t0.elapsed().as_secs_f64();
    assert_eq!(r.objects.len(), 1);
    assert_eq!(r.objects[0].mesh.num_faces(), 1_000_000);
    println!("obj 1M quads ({} MB): write {t_write}s, read {t_read}s", text.len() >> 20);
}
