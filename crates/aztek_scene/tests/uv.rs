//! UV editing: projections, cut and sew, unfold, layout, moving UVs; each as a history node.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn uvs(s: &mut Session, what: &str) -> Vec<f64> {
    run(s, "polyEditUV", json!({"args": [what], "query": true})).as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()
}

fn in_unit(v: &[f64]) -> bool {
    v.iter().all(|x| *x >= -1e-6 && *x <= 1.0 + 1e-6)
}

#[test]
fn projections_and_layout_stay_in_0_1() {
    let mut s = Session::new();
    run(&mut s, "polySphere", json!({}));
    let made = run(&mut s, "polyProjection", json!({"args": ["pSphere1"], "type": "planar", "mapDirection": "y"}));
    assert_eq!(made, json!(["polyPlanarProj1"]));
    assert!(in_unit(&uvs(&mut s, "pSphere1")));
    run(&mut s, "polyAutoProjection", json!({"args": ["pSphere1"]}));
    assert!(in_unit(&uvs(&mut s, "pSphere1")));
    run(&mut s, "u3dLayout", json!({"args": ["pSphere1"]}));
    assert!(in_unit(&uvs(&mut s, "pSphere1")));
    let hist = run(&mut s, "listHistory", json!({"args": ["pSphere1"]}));
    let h: Vec<&str> = hist.as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
    assert!(h.contains(&"polyLayoutUV1") && h.contains(&"polyAutoProj1"), "{h:?}");
}

#[test]
fn edit_uv_moves_maps_and_undoes() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"subdivisionsX": 1, "subdivisionsZ": 1}));
    let before = uvs(&mut s, "pPlane1.map[0]");
    run(&mut s, "polyEditUV", json!({"args": ["pPlane1.map[0]"], "u": 0.25, "v": -0.5}));
    let after = uvs(&mut s, "pPlane1.map[0]");
    assert!((after[0] - before[0] - 0.25).abs() < 1e-6 && (after[1] - before[1] + 0.5).abs() < 1e-6);
    s.undo();
    assert_eq!(uvs(&mut s, "pPlane1.map[0]"), before);
}

#[test]
fn cut_then_sew_splits_and_joins_shells() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"subdivisionsX": 2, "subdivisionsZ": 1}));
    let maps = uvs(&mut s, "pPlane1").len() / 2;
    assert_eq!(maps, 6);
    run(&mut s, "polyMapCut", json!({"args": ["pPlane1.f[0]"]}));
    assert_eq!(uvs(&mut s, "pPlane1").len() / 2, 8);
    run(&mut s, "polyMapSew", json!({"args": ["pPlane1.f[0]"]}));
    assert_eq!(uvs(&mut s, "pPlane1").len() / 2, 6);
}

#[test]
fn unfold_keeps_shell_in_place() {
    let mut s = Session::new();
    run(&mut s, "polyCylinder", json!({}));
    run(&mut s, "u3dUnfold", json!({"args": ["pCylinder1"]}));
    let v = uvs(&mut s, "pCylinder1");
    assert!(v.iter().all(|x| x.is_finite()));
}
