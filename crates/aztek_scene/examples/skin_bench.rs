//! Where the time goes when a rig deforms: `skin_bench file.ma node.attr [frames]`. Drags the channel through a range, times a
//! whole evaluation, a mesh copy, and building what the viewport draws.

use aztek_geo::render_mesh::{build_render_mesh, RenderMeshOptions};
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::plug::PlugValue;
use aztek_scene::transform::DagNode;
use aztek_scene::{ma_file, Session};
use serde_json::json;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let frames: usize = a.get(3).and_then(|x| x.parse().ok()).unwrap_or(40);
    let mut s = Session::new();
    ma_file::open_scene(&mut s, &a[1]).expect("open");
    s.graph.evaluate();
    let meshes: Vec<_> = s.graph.nodes().into_iter().filter(|&id| s.graph.node(id).is_some_and(|n| n.ty().name == "mesh" && !n.data().bool(DagNode::INTERMEDIATE_OBJECT))).collect();
    let (mut points, mut corners, mut stored) = (0, 0, 0);
    let (mut clone, mut build) = (0.0, 0.0);
    for &id in &meshes {
        if let PlugValue::Mesh(Some(m)) = s.graph.node(id).unwrap().data().value(MeshNode::OUT_MESH) {
            points += m.point_capacity() as usize;
            corners += m.corner_capacity() as usize;
            if !m.corner_attrs().get::<[f32; 3]>(aztek_geo::names::NORMAL).is_empty() {
                stored += 1;
            }
            let t = Instant::now();
            let c = (**m).clone();
            clone += t.elapsed().as_secs_f64();
            std::hint::black_box(&c);
            let t = Instant::now();
            let r = build_render_mesh(m, &RenderMeshOptions { max_uv_sets: 1, ..Default::default() });
            build += t.elapsed().as_secs_f64();
            std::hint::black_box(&r);
        }
    }
    println!("{} meshes, {points} points, {corners} corners, {stored} with stored normals", meshes.len());
    println!("one pass over every mesh: copy {:.1} ms, build for drawing {:.1} ms", clone * 1000.0, build * 1000.0);
    let was = s.run("getAttr", json!({"args": [a[2]]})).unwrap().as_f64().unwrap();
    let mut times = Vec::new();
    for f in 0..frames {
        s.run("setAttr", json!({"args": [a[2], was + (f as f64 * 0.3).sin() * 30.0]})).unwrap();
        let t = Instant::now();
        s.graph.evaluate();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(|x, y| x.partial_cmp(y).unwrap());
    println!("evaluate per change: median {:.1} ms, worst {:.1} ms", times[times.len() / 2], times[times.len() - 1]);
}
