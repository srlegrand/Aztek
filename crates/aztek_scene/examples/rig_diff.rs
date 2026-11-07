//! Compares two .ma files with the same rig under the same pose: `rig_diff a.ma b.ma node.attr=value ...`. Prints the transforms
//! that end up in different places, with the ones whose parent agrees first, since the rest follow from those.

use aztek_scene::plug::{NodeId, NO_NODE};
use aztek_scene::transform::Transform;
use aztek_scene::{ma_file, Session};
use glam::DMat4;
use serde_json::json;
use std::collections::HashMap;

fn worlds(s: &mut Session) -> HashMap<String, (DMat4, String)> {
    s.graph.evaluate();
    let g = &s.graph;
    let ids: Vec<NodeId> = g.nodes();
    let mut out = HashMap::new();
    for id in ids {
        let Some(n) = g.node(id) else { continue };
        if !n.ty().is_a("transform") {
            continue;
        }
        let p = g.parent(id);
        let parent = if p == NO_NODE { String::new() } else { g.node(p).map_or(String::new(), |x| x.name().to_string()) };
        out.insert(n.name().to_string(), (n.data().matrix(Transform::WORLD_MATRIX), parent));
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (mut a, mut b) = (Session::new(), Session::new());
    ma_file::open_scene(&mut a, &args[1]).expect("a");
    ma_file::open_scene(&mut b, &args[2]).expect("b");
    for pose in &args[3..] {
        let (plug, v) = pose.split_once('=').expect("node.attr=value");
        let v: f64 = v.parse().expect("number");
        for s in [&mut a, &mut b] {
            s.run("setAttr", json!({"args": [plug, v]})).expect(plug);
        }
    }
    let (wa, wb) = (worlds(&mut a), worlds(&mut b));
    let off = |x: &DMat4, y: &DMat4| (0..4).map(|c| (x.col(c) - y.col(c)).abs().max_element()).fold(0.0, f64::max);
    let mut bad: Vec<(f64, String, String)> = Vec::new();
    for (name, (ma, parent)) in &wa {
        let Some((mb, _)) = wb.get(name) else {
            println!("only in the first: {name}");
            continue;
        };
        let d = off(ma, mb);
        if d > 0.01 {
            bad.push((d, name.clone(), parent.clone()));
        }
    }
    let names: std::collections::HashSet<String> = bad.iter().map(|x| x.1.clone()).collect();
    println!("{} of {} transforms differ", bad.len(), wa.len());
    bad.retain(|x| !names.contains(&x.2));
    bad.sort_by(|x, y| y.0.total_cmp(&x.0));
    for (d, name, _) in bad.iter().take(30) {
        println!("  {d:9.3}  {name}");
    }
}
