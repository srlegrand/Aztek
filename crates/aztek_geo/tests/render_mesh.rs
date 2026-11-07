//! Tests: render mesh.

use std::time::Instant;

use aztek_geo::normals::*;
use aztek_geo::primitives::*;
use aztek_geo::render_mesh::*;
use aztek_geo::triangulate::{triangulate, TriangulateOptions};
use aztek_geo::{colors, names};

#[test]
fn render_mesh_splits() {
    let mut cube = make_cube(&CubeOptions::default());
    let r = build_render_mesh(&cube, &RenderMeshOptions::default());
    assert_eq!(r.vertex_count(), 24);
    assert_eq!(r.triangle_count(), 12);
    assert_eq!(r.line_indices.len(), 24);
    assert_eq!(r.sections.len(), 1);
    assert!(r.indices.iter().all(|&i| i < r.vertex_count()));

    // Soft cube: uv seams still split, normals no longer do.
    set_soft_edges(&mut cube, &[]);
    let r = build_render_mesh(&cube, &RenderMeshOptions::default());
    assert_eq!(r.vertex_count(), 14);

    let sphere = make_sphere(&SphereOptions { radius: 50.0, subdiv_axis: 8, subdiv_height: 4, ..Default::default() });
    let ro = RenderMeshOptions { tangents: true, ..Default::default() };
    let r = build_render_mesh(&sphere, &ro);
    // Rings duplicate one point on the uv seam; each pole splits per triangle.
    assert_eq!(r.vertex_count(), 3 * 9 + 2 * 8);
    assert_eq!(r.tangents.len(), r.vertex_count() as usize);

    let mut two = make_cube(&CubeOptions::default());
    let mat = two.face_attrs_mut().get_or_add::<i32>(names::MATERIAL);
    mat[1] = 2;
    mat[3] = 2;
    let r = build_render_mesh(&two, &RenderMeshOptions::default());
    assert_eq!(r.sections.len(), 2);
    assert_eq!(r.sections[0].material, 0);
    assert_eq!(r.sections[0].index_count, 24);
    assert_eq!(r.sections[1].material, 2);
    assert_eq!(r.sections[1].first_index, 24);
    assert_eq!(two.face_attrs().get::<i32>(names::MATERIAL)[r.triangle_faces[8] as usize], 2);
}

#[test]
fn render_mesh_color_sets_split() {
    let mut cube = make_cube(&CubeOptions::default());
    set_soft_edges(&mut cube, &[]);
    let set = colors::unique_name(&cube, "");
    colors::add_white(&mut cube, &set);
    let r = build_render_mesh(&cube, &RenderMeshOptions::default());
    assert_eq!(r.colors.len(), r.vertex_count() as usize);
    assert_eq!(r.vertex_count(), 14);
    // One face red: its four corners split from their neighbours.
    let f = 0;
    let corners = cube.face_corners(f);
    for c in corners {
        colors::values_mut(&mut cube, &set)[c as usize] = [1.0, 0.0, 0.0, 1.0];
    }
    let r = build_render_mesh(&cube, &RenderMeshOptions::default());
    assert!(r.vertex_count() > 14);
    let ro = RenderMeshOptions { colors: false, ..Default::default() };
    assert!(build_render_mesh(&cube, &ro).colors.is_empty());
}

#[test]
#[ignore = "perf"]
fn perf_1m_faces() {
    let t0 = Instant::now();
    let mut m = make_plane(&PlaneOptions { width: 1000.0, depth: 1000.0, subdiv_x: 1000, subdiv_z: 1000, ..Default::default() });
    let t_build = t0.elapsed().as_secs_f64();
    assert_eq!(m.num_faces(), 1_000_000);
    for p in m.positions_mut() {
        p.y = 10.0 * (p.x * 0.05).sin() * (p.z * 0.05).cos();
    }
    let t0 = Instant::now();
    let _fnorm = face_normals(&m);
    let t_face = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let cn = corner_normals(&m, &CornerNormalOptions::default());
    let t_corner = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let tris = triangulate(&m, &TriangulateOptions::default());
    let t_tri = t0.elapsed().as_secs_f64();
    assert_eq!(tris.triangle_count(), 2_000_000);
    let t0 = Instant::now();
    let _tan = corner_tangents(&m, &tris, &cn, "uv0", true);
    let t_tan = t0.elapsed().as_secs_f64();
    let t0 = Instant::now();
    let r = build_render_mesh(&m, &RenderMeshOptions::default());
    let t_render = t0.elapsed().as_secs_f64();
    assert_eq!(r.vertex_count(), m.num_points());
    let t0 = Instant::now();
    let _copy = m.clone();
    let t_copy = t0.elapsed().as_secs_f64();
    println!(
        "1M quads: build {t_build}s, face normals {t_face}s, corner normals {t_corner}s, triangulate {t_tri}s, \
         mikktspace {t_tan}s, render build {t_render}s, copy {t_copy}s"
    );
}
