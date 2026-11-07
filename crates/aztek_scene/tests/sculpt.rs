//! Sculpting brushes.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn plane(s: &mut Session) {
    run(s, "polyPlane", json!({"width": 10, "depth": 10, "subdivisionsX": 20, "subdivisionsZ": 20}));
    run(s, "sculptMeshCacheCtx", json!({"edit": true, "size": 3.0, "strength": 1.0}));
}

fn top(s: &mut Session) -> f64 {
    run(s, "meshInfo", json!({"args": ["pPlane1"]}))["worldBoundsMax"][1].as_f64().unwrap()
}

fn bottom(s: &mut Session) -> f64 {
    run(s, "meshInfo", json!({"args": ["pPlane1"]}))["worldBoundsMin"][1].as_f64().unwrap()
}

#[test]
fn sculpt_pushes_and_inverts_and_undoes_in_one_step() {
    let mut s = Session::new();
    plane(&mut s);
    assert!(run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0], "normal": [0, 1, 0]})).as_i64().unwrap() > 0);
    assert!(top(&mut s) > 0.5);
    assert!(s.undo());
    assert!(top(&mut s).abs() < 1e-9);
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0], "normal": [0, 1, 0], "invert": true}));
    assert!(bottom(&mut s) < -0.5);
}

#[test]
fn brushes_do_their_own_thing() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0], "normal": [0, 1, 0]}));
    let h = top(&mut s);
    // Smooth lowers the peak; flatten lowers it more toward the mean plane.
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "sculptTool": "smooth"}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, h, 0]}));
    let h2 = top(&mut s);
    assert!(h2 < h, "{h} {h2}");
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "sculptTool": "flatten"}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, h2, 0]}));
    assert!(top(&mut s) < h2);
    // Grab follows the drag.
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "sculptTool": "grab"}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [3, 0, 3], "drag": [0, 2, 0]}));
    assert!(top(&mut s) > 1.5);
    // Shift smooths, whichever brush.
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "sculptTool": "sculpt"}));
    let before = top(&mut s);
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [3, 2, 3], "smooth": true}));
    assert!(top(&mut s) < before);
}

#[test]
fn frozen_points_stay() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "sculptTool": "freeze", "strength": 1.0}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0]}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0]}));
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "sculptTool": "sculpt"}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0], "normal": [0, 1, 0]}));
    let frozen = top(&mut s);
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "clearFreeze": true}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [0, 0, 0], "normal": [0, 1, 0]}));
    assert!(top(&mut s) > frozen + 0.3, "{frozen} {}", top(&mut s));
}

#[test]
fn symmetry_mirrors_the_dab() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "sculptMeshCacheCtx", json!({"edit": true, "symmetry": true, "size": 1.0}));
    run(&mut s, "sculptStamp", json!({"args": ["pPlane1"], "position": [3, 0, 0], "normal": [0, 1, 0]}));
    let i = run(&mut s, "ls", json!({"args": ["pPlane1.vtx[*]"], "flatten": true}));
    let (mut plus, mut minus) = (0.0_f64, 0.0_f64);
    for v in i.as_array().unwrap() {
        let p = run(&mut s, "xform", json!({"args": [v], "query": true, "worldSpace": true, "translation": true}));
        let (x, y) = (p[0].as_f64().unwrap(), p[1].as_f64().unwrap());
        if x > 0.0 {
            plus = plus.max(y);
        } else {
            minus = minus.max(y);
        }
    }
    assert!(plus > 0.05 && (plus - minus).abs() < 1e-9, "{plus} {minus}");
}
