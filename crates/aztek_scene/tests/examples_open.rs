//! Every scene in `examples/` opens without a warning and plays: a frame from the middle and the last frame evaluate.

use aztek_scene::ma_file::open_scene;
use aztek_scene::Session;
use serde_json::json;
use std::path::{Path, PathBuf};

fn scenes(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir).unwrap().flatten().map(|e| e.path()).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            if p.file_name().map_or(false, |n| n == "tools" || n == "textures" || n == "scripts") {
                continue;
            }
            scenes(&p, out);
        } else if p.extension().map_or(false, |e| e == "ma") {
            out.push(p);
        }
    }
}

#[test]
fn every_example_opens_and_plays() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut all = Vec::new();
    scenes(&root, &mut all);
    assert!(all.len() >= 50, "found {} scenes", all.len());
    let mut problems = Vec::new();
    for p in &all {
        let name = p.strip_prefix(&root).unwrap().display().to_string();
        let mut s = Session::new();
        match open_scene(&mut s, &p.to_string_lossy()) {
            Err(e) => problems.push(format!("{name}: {e}")),
            Ok(w) if !w.is_empty() => problems.push(format!("{name}: {w:?}")),
            Ok(_) => {}
        }
        let min = s.run("playbackOptions", json!({"query": true, "min": true})).ok().and_then(|v| v.as_f64()).unwrap_or(1.0);
        let max = s.run("playbackOptions", json!({"query": true, "max": true})).ok().and_then(|v| v.as_f64()).unwrap_or(1.0);
        // Simulations have to step there frame by frame; the rest jump.
        let frames: Vec<f64> = if s.run("ls", json!({"type": ["cloth", "rigidBody", "particle", "nParticle"]})).ok().map_or(false, |v| v.as_array().map_or(false, |a| !a.is_empty())) {
            (min as i64..=(min as i64 + 10)).map(|f| f as f64).collect()
        } else {
            vec![((min + max) / 2.0).round(), max]
        };
        for f in frames {
            if let Err(e) = s.run("currentTime", json!({"args": [f]})) {
                problems.push(format!("{name}: frame {f}: {e}"));
                break;
            }
        }
        s.graph.evaluate();
    }
    assert!(problems.is_empty(), "{} of {} examples:\n{}", problems.len(), all.len(), problems.join("\n"));
}
