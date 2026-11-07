//! Imports a USD file and prints each mesh's world points, UVs and material as JSON, at the frames given, to compare with
//! another tool's reading of the same file: `usd_dump file.usd 1 5`.

use aztek_scene::session::Session;
use aztek_scene::transform::Transform;
use serde_json::{json, Map, Value};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let frames: Vec<f64> = args[2..].iter().map(|x| x.parse().unwrap()).collect();
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    s.run("mayaUSDImport", json!({"file": args[1], "readAnimData": true, "primPath": "/", "upAxis": false, "unit": false})).expect("import");
    let mut out = Map::new();
    for &f in &frames {
        s.run("currentTime", json!({"args": [f]})).unwrap();
        s.graph.evaluate();
        for id in s.graph.nodes() {
            let Some(n) = s.graph.node(id) else { continue };
            if n.ty().name != "mesh" || n.bool(aztek_scene::transform::DagNode::INTERMEDIATE_OBJECT) {
                continue;
            }
            let parent = s.graph.parent(id);
            let Some(p) = s.graph.node(parent) else { continue };
            let name = s.graph.full_path(parent);
            let world = p.matrix(Transform::WORLD_MATRIX);
            let Some(m) = aztek_usd::util::evaluated_mesh(&mut s.graph, id) else { continue };
            let points: Vec<[f64; 3]> = m.positions().iter().map(|q| world.transform_point3(*q).to_array()).collect();
            let entry = out.entry(name).or_insert_with(|| json!({"frames": {}, "faces": m.num_faces()}));
            entry["frames"][format!("{f}")] = json!(points);
            if m.corner_attrs().has("uv0") {
                let uv = m.corner_attrs().get::<[f32; 2]>("uv0");
                let mut list = Vec::new();
                for fi in 0..m.num_faces() {
                    for c in m.face_corners(fi) {
                        list.push(uv[c as usize]);
                    }
                }
                entry["uv"] = json!(list);
            }
            let groups = aztek_scene::shading::membership_of(&s.graph, id);
            let names: Vec<Value> = groups.iter().filter_map(|(g, _)| s.graph.node(*g).map(|n| json!(n.name()))).collect();
            entry["groups"] = Value::Array(names);
        }
    }
    println!("{}", Value::Object(out));
}
