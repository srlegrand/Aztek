//! A rig referenced twice with no UI: how long it takes, what edits appear with nothing changed, and whether a pose survives a save and reopen.
//! `cargo run --release -p aztek_scene --example ref_rig -- rig.ma control.attr value [out.ma]`

use aztek_scene::Session;
use serde_json::{json, Value};
use std::time::Instant;

fn edits(s: &mut Session, rfn: &str) -> Vec<String> {
    let v = s.run("referenceQuery", json!({"args": [rfn], "es": true})).unwrap_or(Value::Null);
    v.as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let (rig, plug, value) = (&a[0], &a[1], a[2].parse::<f64>().unwrap());
    let out = a.get(3).cloned().unwrap_or_else(|| std::env::temp_dir().join("ref_rig_out.ma").to_string_lossy().to_string());
    let mut s = Session::new();
    let t = Instant::now();
    s.run("file", json!({"args": [rig], "r": true, "ns": "a"})).unwrap();
    println!("first reference {:.2}s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    s.run("file", json!({"args": [rig], "r": true, "ns": "b"})).unwrap();
    println!("second reference {:.2}s", t.elapsed().as_secs_f64());
    let t = Instant::now();
    let e = edits(&mut s, "aRN");
    println!("edits with nothing changed: {} ({:.2}s)", e.len(), t.elapsed().as_secs_f64());
    for x in e.iter().take(12) {
        println!("  {}", &x[..x.len().min(130)]);
    }
    let by: std::collections::BTreeMap<String, usize> = e.iter().fold(Default::default(), |mut m, x| {
        let attr = x.split_whitespace().nth(1).and_then(|p| p.rsplit('.').next()).unwrap_or("").to_string();
        *m.entry(attr).or_insert(0) += 1;
        m
    });
    let mut by: Vec<_> = by.into_iter().collect();
    by.sort_by_key(|x| std::cmp::Reverse(x.1));
    println!("  by attribute: {:?}", &by[..by.len().min(12)]);
    let (pa, pb) = (format!("a:{plug}"), format!("b:{plug}"));
    s.run("setKeyframe", json!({"args": [pa], "t": 1.0, "v": 0.0})).unwrap();
    s.run("setKeyframe", json!({"args": [pa], "t": 10.0, "v": value})).unwrap();
    s.run("setAttr", json!({"args": [pb, value * 0.5]})).unwrap();
    s.run("currentTime", json!({"args": [10.0]})).unwrap();
    let t = Instant::now();
    s.run("file", json!({"args": [out], "rename": out})).unwrap();
    s.run("file", json!({"save": true})).unwrap();
    println!("save {:.2}s, {} bytes", t.elapsed().as_secs_f64(), std::fs::metadata(&out).unwrap().len());
    let worlds = |s: &mut Session| -> Vec<(String, Vec<f64>)> {
        let names: Vec<String> = s.run("ls", json!({"type": "joint"})).unwrap().as_array().unwrap().iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
        names.into_iter().map(|n| {
            let m = s.run("getAttr", json!({"args": [format!("{n}.worldMatrix")]})).unwrap();
            let v: Vec<f64> = m.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
            (n, v)
        }).collect()
    };
    let before = worlds(&mut s);
    let t = Instant::now();
    let mut r = Session::new();
    r.run("file", json!({"args": [out], "open": true, "force": true})).unwrap();
    println!("reopen {:.2}s", t.elapsed().as_secs_f64());
    r.run("currentTime", json!({"args": [10.0]})).unwrap();
    println!("after reopen, edits on aRN: {}, on bRN: {}", edits(&mut r, "aRN").len(), edits(&mut r, "bRN").len());
    let e = edits(&mut r, "aRN");
    for x in e.iter().take(14) {
        println!("  {}", &x[..x.len().min(170)]);
    }
    let after: std::collections::HashMap<String, Vec<f64>> = worlds(&mut r).into_iter().collect();
    let (mut worst, mut at, mut missing) = (0.0f64, String::new(), 0);
    for (n, m) in &before {
        match after.get(n) {
            Some(o) if o.len() == m.len() => {
                for (x, y) in m.iter().zip(o) {
                    if (x - y).abs() > worst {
                        worst = (x - y).abs();
                        at = n.clone();
                    }
                }
            }
            _ => missing += 1,
        }
    }
    println!("joints {}: worst difference {worst:.5} at {at}, {missing} missing", before.len());
}
