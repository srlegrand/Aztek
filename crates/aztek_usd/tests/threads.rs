//! Stages are per thread, but what they leave on disk is shared: one thread's session layer must not land on another's.

use serde_json::json;

#[test]
fn stages_opened_at_once_on_several_threads_all_read() {
    let dir = std::env::temp_dir().join(format!("aztek_usd_threads_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("root.usda");
    std::fs::write(&file, "#usda 1.0\ndef Xform \"World\" {}\n").unwrap();
    let handles: Vec<_> = (0..16)
        .map(|_| {
            let file = file.to_string_lossy().to_string();
            std::thread::spawn(move || {
                for _ in 0..8 {
                    let mut s = aztek_scene::Session::new();
                    aztek_scene::cameras::make_defaults(&mut s.graph);
                    aztek_usd::install(&mut s);
                    s.run("mayaUsd_createStageFromFile", json!({"args": [file]})).unwrap();
                }
            })
        })
        .collect();
    for h in handles {
        h.join().unwrap();
    }
    std::fs::remove_dir_all(dir).ok();
}
