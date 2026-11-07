//! Deformers: nonlinear, cluster, blend shape.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn bounds(s: &mut Session, m: &str) -> (Vec<f64>, Vec<f64>) {
    let i = run(s, "meshInfo", json!({"args": [m]}));
    let v = |k: &str| i[k].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<f64>>();
    (v("worldBoundsMin"), v("worldBoundsMax"))
}

#[test]
fn bend_and_twist_deform_through_their_handles() {
    let mut s = Session::new();
    run(&mut s, "polyCylinder", json!({"height": 4, "subdivisionsHeight": 16}));
    let (lo0, hi0) = bounds(&mut s, "pCylinder1");
    let r = run(&mut s, "nonLinear", json!({"args": ["pCylinder1"], "type": "bend", "curvature": 1.0}));
    assert_eq!(r, json!(["bend1", "bend1Handle"]));
    let (lo, hi) = bounds(&mut s, "pCylinder1");
    assert!(hi[0] - lo[0] > hi0[0] - lo0[0] + 0.3, "{lo:?} {hi:?}");
    run(&mut s, "setAttr", json!({"args": ["bend1.curvature", 0.0]}));
    let (lo, hi) = bounds(&mut s, "pCylinder1");
    assert!((hi[1] - hi0[1]).abs() < 1e-6 && (lo[0] - lo0[0]).abs() < 1e-6);
    let h = run(&mut s, "listHistory", json!({"args": ["pCylinder1"]}));
    assert!(h.as_array().unwrap().iter().any(|x| x == "bend1"), "{h}");
}

#[test]
fn cluster_moves_its_points() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "move", json!({"args": ["cluster1Handle"], "to": [0, 2, 0]}));
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[1] - 1.5).abs() < 1e-6, "{lo:?}");
}

#[test]
fn blend_shape_blends() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "scale", json!({"args": ["pCube2"], "to": [1, 1, 1]}));
    run(&mut s, "select", json!({"args": ["pCube2.vtx[0:7]"]}));
    run(&mut s, "scaleComponents", json!({"by": [2, 2, 2]}));
    run(&mut s, "select", json!({"clear": true}));
    let r = run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]}));
    assert_eq!(r, json!(["blendShape1"]));
    run(&mut s, "setAttr", json!({"args": ["blendShape1.pCube2", 0.5]}));
    let (lo, hi) = bounds(&mut s, "pCube1");
    assert!((hi[0] - lo[0] - 1.5).abs() < 1e-6, "{lo:?} {hi:?}");
}
