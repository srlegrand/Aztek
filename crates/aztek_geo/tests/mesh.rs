//! Tests: mesh.

use aztek_geo::{validate, Mesh, MeshRemap, INVALID};
use glam::DVec3;

fn is_valid(m: &Mesh, flags: u32) -> bool {
    let issues = m.validate(flags);
    for i in issues.iter().take(10) {
        eprintln!("{i}");
    }
    issues.is_empty()
}

fn boundary_edge_count(m: &Mesh) -> u32 {
    (0..m.edge_capacity())
        .filter(|&e| m.edge_valid(e) && m.is_boundary_edge(e))
        .count() as u32
}

fn quad_grid(nx: u32, ny: u32) -> Mesh {
    let mut m = Mesh::new();
    for j in 0..=ny {
        for i in 0..=nx {
            m.add_point(DVec3::new(i as f64, 0.0, -(j as f64)));
        }
    }
    for j in 0..ny {
        for i in 0..nx {
            let a = j * (nx + 1) + i;
            m.add_face(&[a, a + 1, a + 1 + nx + 1, a + nx + 1]);
        }
    }
    m
}

#[test]
fn build_and_adjacency() {
    let m = quad_grid(3, 2);
    assert_eq!(m.num_points(), 12);
    assert_eq!(m.num_faces(), 6);
    assert_eq!(m.num_edges(), 17);
    assert!(is_valid(&m, validate::DEFAULT));
    assert_eq!(boundary_edge_count(&m), 10);
    assert_eq!(m.point_valence(5), 4);
    assert!(!m.is_boundary_point(5));
    assert!(m.is_boundary_point(0));
    assert_eq!(m.point_faces(5).len(), 4);
    let e = m.find_edge(5, 6);
    assert_ne!(e, INVALID);
    assert_eq!(m.edge_face_count(e), 2);
    assert_eq!(m.find_edge(0, 5), INVALID);
    assert!((m.face_normal(0).y - 1.0).abs() < 1e-12);
}

#[test]
fn non_manifold_edge() {
    let mut m = Mesh::new();
    for i in 0..5 {
        m.add_point(DVec3::new(i as f64, (i * i) as f64, 0.0));
    }
    m.add_face(&[0, 1, 2]);
    m.add_face(&[1, 0, 3]);
    m.add_face(&[0, 1, 4]);
    assert_eq!(m.edge_face_count(m.find_edge(0, 1)), 3);
    assert!(is_valid(&m, validate::STRUCTURE));
    assert!(!m.validate(validate::MANIFOLD).is_empty());
}

#[test]
fn remove_compact_attributes_follow() {
    let mut m = quad_grid(3, 3);
    let pc = m.point_capacity();
    let w = m.point_attrs_mut().get_or_add::<f32>("weight");
    for p in 0..pc {
        w[p as usize] = p as f32;
    }
    let fc = m.face_capacity();
    let id = m.face_attrs_mut().get_or_add::<i32>("id");
    for f in 0..fc {
        id[f as usize] = f as i32;
    }
    m.remove_face(4);
    assert_eq!(m.num_faces(), 8);
    assert!(is_valid(&m, validate::DEFAULT));
    m.remove_point(0);
    assert_eq!(m.num_faces(), 7);
    m.remove_all_loose();
    assert!(is_valid(&m, validate::DEFAULT));
    let mut remap = MeshRemap::default();
    m.compact(Some(&mut remap));
    assert!(!m.has_deleted());
    assert!(is_valid(&m, validate::DEFAULT));
    assert_eq!(remap.points[0], INVALID);
    assert_eq!(
        m.point_attrs().get::<f32>("weight")[remap.points[5] as usize],
        5.0
    );
    assert_eq!(m.face_attrs().get::<i32>("id")[remap.faces[8] as usize], 8);
    // Contiguous corners after compact.
    for f in 0..m.face_capacity() {
        assert_eq!(m.corner_next(m.face_corner(f)), m.face_corner(f) + 1);
    }
}

#[test]
fn euler_operators() {
    let mut m = quad_grid(2, 1);
    let cc = m.corner_capacity();
    let pos: Vec<[f32; 2]> = (0..cc)
        .map(|c| {
            let p = m.position(m.corner_point(c));
            [p.x as f32, -p.z as f32]
        })
        .collect();
    m.corner_attrs_mut()
        .get_or_add::<[f32; 2]>("uv0")
        .copy_from_slice(&pos);

    let e = m.find_edge(1, 4);
    let mid = m.split_edge(e, 0.25);
    assert!(is_valid(&m, validate::DEFAULT));
    assert_eq!(m.face_size(0), 5);
    assert_eq!(m.face_size(1), 5);
    assert!((m.position(mid).z + 0.25).abs() < 1e-12);
    let uv = m.corner_attrs().get::<[f32; 2]>("uv0").to_vec();
    m.for_each_point_corner(mid, |c| assert!((uv[c as usize][1] - 0.25).abs() < 1e-6));

    let c0 = m.find_corner(0, 0);
    let cm = m.find_corner(0, mid);
    let nf = m.split_face(0, c0, cm);
    assert_ne!(nf, INVALID);
    assert!(is_valid(&m, validate::DEFAULT));
    assert_eq!(m.face_size(0) + m.face_size(nf), 7);

    let cut = m.last_split_edge();
    let joined = m.join_faces(cut);
    assert_ne!(joined, INVALID);
    assert!(is_valid(&m, validate::DEFAULT));
    assert_eq!(m.face_size(joined), 5);
    assert_eq!(m.num_faces(), 2);

    m.flip_face(1);
    assert!(!m.validate(validate::WINDING).is_empty());
    m.flip_face(1);
    assert!(is_valid(&m, validate::DEFAULT));
}

#[test]
fn bulk_builder_matches_face_by_face() {
    // A grid with a non-manifold fin, a bad face and a repeated point, through both builders.
    let (nx, ny) = (7u32, 5u32);
    let mut pts = Vec::new();
    for j in 0..=ny {
        for i in 0..=nx {
            pts.push(DVec3::new(i as f64, 0.0, j as f64));
        }
    }
    pts.push(DVec3::new(0.5, 1.0, 0.5));
    let fin = pts.len() as u32 - 1;
    let (mut sizes, mut idx) = (Vec::new(), Vec::new());
    for j in 0..ny {
        for i in 0..nx {
            let a = j * (nx + 1) + i;
            sizes.push(4);
            idx.extend([a, a + 1, a + 1 + nx + 1, a + nx + 1]);
        }
    }
    sizes.push(3);
    idx.extend([0, 1, fin]);
    sizes.push(3);
    idx.extend([1, 2, 1]);
    sizes.push(2);
    idx.extend([3, 4]);
    let a = Mesh::from_polygons(&pts, &sizes, &idx);
    let b = Mesh::from_polygons_bulk_for_tests(&pts, &sizes, &idx);
    assert_eq!(a.num_faces(), b.num_faces());
    assert_eq!(a.num_edges(), b.num_edges());
    assert_eq!(a.topology_dump(), b.topology_dump());
    assert!(is_valid(&b, validate::STRUCTURE));
}

#[test]
fn append_merges_layers_and_materials() {
    let mut a = quad_grid(1, 1);
    a.material_names_mut().push("red".into());
    a.face_attrs_mut().get_or_add::<i32>("material")[0] = 0;
    let mut b = quad_grid(1, 1);
    b.material_names_mut()
        .extend(["blue".to_string(), "red".to_string()]);
    b.face_attrs_mut().get_or_add::<i32>("material")[0] = 1;
    b.point_attrs_mut()
        .add("weight", aztek_geo::AttrType::Float)
        .set_default(2.0f32);
    let offset = a.append(&b);
    assert_eq!(offset, 4);
    assert_eq!(a.num_faces(), 2);
    assert_eq!(a.material_names(), &["red", "blue"]);
    assert_eq!(a.face_attrs().get::<i32>("material"), &[0, 0]);
    assert_eq!(a.point_attrs().get::<f32>("weight")[0], 2.0);
    assert!(is_valid(&a, validate::DEFAULT));
}
