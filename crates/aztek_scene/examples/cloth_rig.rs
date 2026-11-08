//! Cloth on a rig, timed. cargo run --release -p aztek_scene --example cloth_rig -- file.ma garment body control
//! [frames] [smooth]: makes the garment cloth (smoothed `smooth` times first), the body a collider, swings the control
//! and prints each frame's time and how many vertices were out of the simulation.
use aztek_scene::Session;
use serde_json::json;

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    if a.len() < 4 {
        eprintln!("usage: cloth_rig file.ma garment body control [frames] [smooth]");
        return;
    }
    let frames: i64 = a.get(4).and_then(|x| x.parse().ok()).unwrap_or(48);
    let smooth: i64 = a.get(5).and_then(|x| x.parse().ok()).unwrap_or(0);
    let mut s = Session::new();
    let t = std::time::Instant::now();
    let w = aztek_scene::ma_file::open_scene(&mut s, &a[0]).expect("open");
    eprintln!("opened in {:.1}s, {} warnings", t.elapsed().as_secs_f64(), w.len());
    let run = |s: &mut Session, n: &str, c: serde_json::Value| s.run(n, c).unwrap_or_else(|e| panic!("{n}: {e}"));
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": frames}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    if smooth > 0 {
        run(&mut s, "polySmooth", json!({"args": [a[1]], "divisions": smooth}));
    }
    let v = run(&mut s, "polyEvaluate", json!({"args": [a[1]], "vertex": true}));
    let bv = run(&mut s, "polyEvaluate", json!({"args": [a[2]], "vertex": true}));
    eprintln!("garment {} vertices, body {} vertices", v, bv);
    run(&mut s, "createCloth", json!({"args": [a[1]]}));
    run(&mut s, "collider", json!({"args": [a[2]]}));
    run(&mut s, "setAttr", json!({"args": ["cloth1.inputMeshAttract", 0.02]}));
    if let Some(k) = std::env::var("EVERY").ok().and_then(|v| v.parse::<i64>().ok()) {
        run(&mut s, "setAttr", json!({"args": ["solverShape1.contactEvery", k]}));
    }
    if let Some(k) = std::env::var("SUB").ok().and_then(|v| v.parse::<i64>().ok()) {
        run(&mut s, "setAttr", json!({"args": ["solverShape1.subSteps", k]}));
    }
    // Swing the control.
    for (f, r) in [(1, 0.0), (frames / 3, 40.0), (2 * frames / 3, -40.0), (frames, 0.0)] {
        run(&mut s, "setKeyframe", json!({"args": [a[3]], "attribute": "rotateY", "time": f, "value": r}));
    }
    let mut total = 0.0;
    for f in 1..=frames {
        let t = std::time::Instant::now();
        run(&mut s, "currentTime", json!({"args": [f]}));
        s.graph.evaluate();
        let ms = t.elapsed().as_secs_f64() * 1e3;
        total += ms;
        let info = run(&mut s, "dynInfo", json!({}));
        let solve = info.as_object().and_then(|o| o.values().next()).and_then(|x| x.get("solve")).cloned().unwrap_or_default();
        println!("{f:3} {ms:8.1} ms frame, solve {:.1} (prepare {:.1}, constraints {:.1}, collide {:.1}), out {}, contacts {}", solve["ms"].as_f64().unwrap_or(0.0), solve["msPrepare"].as_f64().unwrap_or(0.0), solve["msConstraints"].as_f64().unwrap_or(0.0), solve["msCollide"].as_f64().unwrap_or(0.0), info.as_object().and_then(|o| o.values().next()).map(|x| x["outOfSimulation"].clone()).unwrap_or_default(), solve["contacts"]);
    }
    let info = run(&mut s, "dynInfo", json!({}));
    println!("last frame by part: {}", info.as_object().and_then(|o| o.values().next()).map(|x| x["solve"]["detail"].to_string()).unwrap_or_default());
    println!("mean {:.1} ms a frame", total / frames as f64);
}
