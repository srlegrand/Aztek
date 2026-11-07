//! Finds what an ordinary change leaves stale: `rig_stale file.ma [pattern] [value]` poses each keyable control matching
//! the pattern and compares what an evaluation gave with what a full re-evaluation of everything gives. Any difference is
//! a mesh or joint that would snap when something later re-evaluates it.

use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::plug::{NodeId, PlugValue};
use aztek_scene::transform::{DagNode, Transform};
use aztek_scene::{ma_file, Session};
use glam::{DMat4, DVec3};
use serde_json::json;
use std::collections::HashMap;
use std::time::Instant;

type Snap = (HashMap<NodeId, DMat4>, HashMap<NodeId, Vec<DVec3>>);

fn snap(s: &Session) -> Snap {
    let g = &s.graph;
    let (mut j, mut m) = (HashMap::new(), HashMap::new());
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().is_a("joint") {
            j.insert(id, n.data().matrix(Transform::WORLD_MATRIX));
        } else if n.ty().name == "mesh" && !n.data().bool(DagNode::INTERMEDIATE_OBJECT) {
            if let PlugValue::Mesh(Some(x)) = n.data().value(MeshNode::OUT_MESH) {
                m.insert(id, x.positions().to_vec());
            }
        }
    }
    (j, m)
}

fn touch_all(s: &mut Session) {
    for id in s.graph.nodes() {
        s.graph.touch(id);
    }
    s.graph.evaluate();
}

/// The largest joint and point differences and whose they are.
fn differ(s: &Session, a: &Snap, b: &Snap) -> (f64, String, f64, String) {
    let name = |id: &NodeId| s.graph.node(*id).map_or(String::new(), |n| n.name().to_string());
    let (mut jw, mut jn, mut mw, mut mn) = (0.0f64, String::new(), 0.0f64, String::new());
    for (id, x) in &a.0 {
        if let Some(y) = b.0.get(id) {
            let d = (x.w_axis - y.w_axis).length();
            if d > jw {
                jw = d;
                jn = name(id);
            }
        }
    }
    for (id, x) in &a.1 {
        if let Some(y) = b.1.get(id) {
            for (p, q) in x.iter().zip(y) {
                let d = (*p - *q).length();
                if d > mw {
                    mw = d;
                    mn = name(id);
                }
            }
        }
    }
    (jw, jn, mw, mn)
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let pattern = args.get(2).map(String::as_str).unwrap_or("");
    let value: f64 = args.get(3).and_then(|v| v.parse().ok()).unwrap_or(20.0);
    let mut s = Session::new();
    ma_file::open_scene(&mut s, &args[1]).expect("open");
    s.graph.evaluate();
    let rest = snap(&s);
    touch_all(&mut s);
    let again = snap(&s);
    let d = differ(&s, &rest, &again);
    println!("after opening, full re-evaluation moves: joint {:.4} {}, point {:.4} {}", d.0, d.1, d.2, d.3);
    let names: Vec<String> = s.run("ls", json!({"type": "transform"})).unwrap().as_array().unwrap().iter().filter_map(|x| x.as_str().map(str::to_string)).filter(|n| n.contains(pattern)).collect();
    let (mut tried, mut stale, mut slowest, mut total) = (0, 0, 0.0f64, 0.0f64);
    for n in names {
        for attr in ["rx", "ty"] {
            let plug = format!("{n}.{attr}");
            let Some(was) = s.run("getAttr", json!({"args": [plug]})).ok().and_then(|v| v.as_f64()) else { continue };
            if s.run("getAttr", json!({"args": [plug], "lock": true})).ok().and_then(|v| v.as_bool()).unwrap_or(true) {
                continue;
            }
            // A driven channel only holds what's set until its source next changes.
            let Ok(p) = s.graph.plug(&plug) else { continue };
            if s.graph.source(&p).is_some() {
                continue;
            }
            if s.run("setAttr", json!({"args": [plug, was + value]})).is_err() {
                continue;
            }
            let t = Instant::now();
            s.graph.evaluate();
            let e = t.elapsed().as_secs_f64();
            let inc = snap(&s);
            let moved = differ(&s, &again, &inc);
            if moved.0 > 1e-6 || moved.2 > 1e-6 {
                tried += 1;
                slowest = slowest.max(e);
                total += e;
            }
            touch_all(&mut s);
            let full = snap(&s);
            let d = differ(&s, &inc, &full);
            if d.0 > 1e-4 || d.2 > 1e-4 {
                stale += 1;
                println!("STALE {plug}: moved point {:.3}; re-evaluating moves joint {:.4} {}, point {:.4} {}", moved.2, d.0, d.1, d.2, d.3);
            }
            let _ = s.run("setAttr", json!({"args": [plug, was]}));
            s.graph.evaluate();
        }
    }
    println!("{tried} changes that moved something, {stale} stale; per change average {:.1} ms, slowest {:.1} ms", total * 1000.0 / tried.max(1) as f64, slowest * 1000.0);
}
