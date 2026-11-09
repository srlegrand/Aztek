//! How long one step of dragging a control takes to evaluate: `cargo run --release -p aztek_scene --example drag_bench -- rig.ma control.attr [steps]`.

use aztek_scene::{ma_file, Session};
use serde_json::json;
use std::time::Instant;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (path, plug) = (&a[0], &a[1]);
    let steps: usize = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(30);
    let mut s = Session::new();
    ma_file::open_scene(&mut s, path).expect("open");
    s.graph.evaluate();
    let start: f64 = s.run("getAttr", json!({"args": [plug]})).ok().and_then(|v| v.as_f64()).unwrap_or(0.0);
    let mut times = Vec::new();
    let mut set_times = Vec::new();
    for i in 0..steps {
        let t = Instant::now();
        let _ = s.run("setAttr", json!({"args": [plug, start + i as f64 * 0.5]}));
        let set = t.elapsed().as_secs_f64() * 1000.0;
        s.graph.evaluate();
        times.push(t.elapsed().as_secs_f64() * 1000.0);
        set_times.push(set);
    }
    times.sort_by(f64::total_cmp);
    set_times.sort_by(f64::total_cmp);
    println!("{plug}: median {:.1} ms (the set alone {:.1} ms), max {:.1} ms over {steps} steps", times[steps / 2], set_times[steps / 2], times[steps - 1]);
}
