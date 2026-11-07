//! Prints curves and plug values from a .ma file for checking a rig by hand:
//! `rig_probe file.ma name.attr ...`; a curve shape's name alone prints its CVs.

use aztek_scene::{curves, ma_file, Session};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut s = Session::new();
    ma_file::open_scene(&mut s, &args[1]).expect("open");
    s.graph.evaluate();
    for a in &args[2..] {
        if let Some(path) = a.strip_prefix("save=") {
            match ma_file::write_file(&mut s, path, ma_file::Format::of_path(path), None) {
                Ok(w) => w.iter().for_each(|x| println!("save warning: {x}")),
                Err(e) => println!("save failed: {e}"),
            }
            continue;
        }
        // speed=node.attr: how long a change to it takes to reach everything downstream.
        if let Some(plug) = a.strip_prefix("speed=") {
            let t = std::time::Instant::now();
            for k in 0..30 {
                s.run("setAttr", serde_json::json!({"args": [plug, k as f64 * 0.2]})).expect(plug);
                s.graph.evaluate();
            }
            println!("{plug}: {:.1} ms per change", t.elapsed().as_secs_f64() * 1000.0 / 30.0);
            continue;
        }
        // expressions: says which expression nodes don't parse.
        if a == "expressions" {
            for id in s.graph.nodes() {
                let Some(n) = s.graph.node(id) else { continue };
                if n.ty().name != "expression" {
                    continue;
                }
                let Some(at) = n.ty().attr_index("expression") else { continue };
                if let aztek_scene::plug::PlugValue::String(src) = n.value(at) {
                    if let Err(e) = aztek_scene::expression::parse(src) {
                        println!("{}: {e}", n.name());
                    }
                }
            }
            continue;
        }
        // set=node.attr=value: poses it for what is asked after.
        if let Some(pose) = a.strip_prefix("set=") {
            let (plug, v) = pose.split_once('=').expect("set=node.attr=value");
            let v: f64 = v.parse().expect("number");
            println!("{pose}: {:?}", s.run("setAttr", serde_json::json!({"args": [plug, v]})).map(|_| ()));
            s.graph.evaluate();
            continue;
        }
        if a == "touch" {
            // Dirties everything and evaluates again: a result that moves was not settled.
            for id in s.graph.nodes() {
                s.graph.touch(id);
            }
            s.graph.evaluate();
            println!("-- touched");
            continue;
        }
        if a.contains('.') {
            let v = s.run("getAttr", serde_json::json!({"args": [a]}));
            println!("{a} = {v:?}");
        } else if let Ok(Some(id)) = s.graph.find(a) {
            println!("{a}: {:?}", curves::world_curve(&mut s.graph, id).map(|c| (c.degree, c.cvs.iter().map(|p| (*p * 100.0).round() / 100.0).collect::<Vec<_>>())));
        }
    }
}
