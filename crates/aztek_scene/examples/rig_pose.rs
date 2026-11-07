//! Tries controls on a rig and says what moved: `rig_pose file.ma node.attr=value ...`. Each pose is set, measured against rest and
//! put back: how many joints moved, how far the final meshes moved, and how long the change took.

use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::plug::{NodeId, PlugValue};
use aztek_scene::transform::{DagNode, Transform};
use aztek_scene::{ma_file, Session};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::time::Instant;

fn joints(s: &Session) -> HashMap<NodeId, DMat4> {
    let g = &s.graph;
    g.nodes().into_iter().filter_map(|id| g.node(id).filter(|n| n.ty().is_a("joint")).map(|n| (id, n.data().matrix(Transform::WORLD_MATRIX)))).collect()
}

fn meshes(s: &Session) -> HashMap<NodeId, (String, Vec<DVec3>)> {
    let g = &s.graph;
    let mut out = HashMap::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().name != "mesh" || n.data().bool(DagNode::INTERMEDIATE_OBJECT) {
            continue;
        }
        if let PlugValue::Mesh(Some(m)) = n.data().value(MeshNode::OUT_MESH) {
            let w = n.data().matrix(DagNode::WORLD_MATRIX);
            out.insert(id, (n.name().to_string(), m.positions().iter().map(|p| w.transform_point3(*p)).collect()));
        }
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut s = Session::new();
    let t = Instant::now();
    let warnings = ma_file::open_scene(&mut s, &args[1]).expect("open");
    s.graph.evaluate();
    println!("open {:.2}s, {} warnings", t.elapsed().as_secs_f64(), warnings.len());
    let (rest_j, rest_m) = (joints(&s), meshes(&s));
    for pose in &args[2..] {
        // Several at once, left in place for the poses after: a,b=1 keeps a held.
        let mut parts: Vec<&str> = pose.split(',').collect();
        let last = parts.pop().unwrap();
        for held in parts {
            let (plug, v) = held.split_once('=').expect("node.attr=value");
            if let Err(e) = s.run("setAttr", json!({"args": [plug, v.parse::<f64>().expect("number")]})) {
                println!("{held}: {e}");
            }
        }
        let (plug, v) = last.split_once('=').expect("node.attr=value");
        let before: Value = match s.run("getAttr", json!({"args": [plug]})) {
            Ok(v) => v,
            Err(e) => {
                println!("{pose}: {e}");
                continue;
            }
        };
        let v: f64 = v.parse().expect("number");
        let t = Instant::now();
        if let Err(e) = s.run("setAttr", json!({"args": [plug, v]})) {
            println!("{pose}: {e}");
            continue;
        }
        s.graph.evaluate();
        let ms = t.elapsed().as_secs_f64() * 1000.0;
        let now_j = joints(&s);
        let moved = now_j.iter().filter(|(id, m)| rest_j.get(id).is_some_and(|r| (m.w_axis - r.w_axis).length() > 1e-3 || (m.x_axis - r.x_axis).length() > 1e-3)).count();
        let now_m = meshes(&s);
        let mut rows: Vec<(f64, String)> = Vec::new();
        for (id, (name, pts)) in &now_m {
            let Some((_, base)) = rest_m.get(id) else { continue };
            let far = pts.iter().zip(base).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
            if far > 1e-3 {
                rows.push((far, name.clone()));
            }
        }
        rows.sort_by(|a, b| b.0.total_cmp(&a.0));
        let list: Vec<String> = rows.iter().take(4).map(|(d, n)| format!("{n} {d:.2}")).collect();
        println!("{pose}: {ms:.0} ms, {moved} joints, meshes: {}", if list.is_empty() { "none".to_string() } else { list.join(", ") });
        let put_back = match before {
            Value::Array(a) => a.first().and_then(Value::as_f64).unwrap_or(0.0),
            x => x.as_f64().or(x.as_bool().map(|b| b as i32 as f64)).unwrap_or(0.0),
        };
        let _ = s.run("setAttr", json!({"args": [plug, put_back]}));
    }
    s.graph.evaluate();
    let now_m = meshes(&s);
    let off = now_m.iter().filter_map(|(id, (_, p))| rest_m.get(id).map(|(_, b)| p.iter().zip(b).map(|(a, x)| (*a - *x).length()).fold(0.0, f64::max))).fold(0.0, f64::max);
    println!("back at rest: meshes within {off:.4}");
}
