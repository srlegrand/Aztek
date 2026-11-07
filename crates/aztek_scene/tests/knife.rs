//! Tests: knife.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn faces(s: &mut Session, m: &Value) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn len(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

#[test]
fn loops_and_paths_are_one_undo_step_each() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let lp = run(&mut s, "polyKnife", json!({"mesh": cube, "loopEdge": 0, "fraction": 0.3}));
    assert_eq!(len(&lp), 4);
    assert_eq!(faces(&mut s, &cube), 10);
    assert_eq!(run(&mut s, "selectMode", json!({})), "edge");
    assert_eq!(len(&run(&mut s, "componentSelection", json!({}))[cube.as_str().unwrap()]["edges"]), 4);
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &cube), 6);

    let plane = run(&mut s, "polyPlane", json!({"subdivisionsX": 1, "subdivisionsZ": 1}))[0].clone();
    let cut = run(&mut s, "polyKnife", json!({"mesh": plane, "points": [{"vertex": 0}, {"vertex": 3}], "direction": [0, -1, 0]}));
    assert_eq!(len(&cut), 1);
    assert_eq!(faces(&mut s, &plane), 2);
    assert!(s.run("polyKnife", json!({"mesh": plane, "points": [{"vertex": 0}]})).is_err());
}

#[test]
fn symmetry_cuts_both_sides() {
    let mut s = Session::new();
    let plane = run(&mut s, "polyPlane", json!({"subdivisionsX": 4, "subdivisionsZ": 1}))[0].clone();
    run(&mut s, "setSymmetry", json!({"args": [plane], "axis": "x"}));
    // A loop across the outermost column on one side also lands on the other.
    run(&mut s, "polyKnife", json!({"mesh": plane, "loopEdge": 0, "fraction": 0.5}));
    assert_eq!(faces(&mut s, &plane), 6);
    assert_eq!(run(&mut s, "symmetryCheck", json!({"args": [plane]}))[plane.as_str().unwrap()]["unmatched"], 0);
}

#[test]
fn delete_edge_joins_its_two_faces_in_one_undo_step() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    run(&mut s, "polyKnife", json!({"mesh": cube, "deleteEdge": 0}));
    assert_eq!(faces(&mut s, &cube), 5);
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &cube), 6);
    assert!(s.run("polyKnife", json!({"mesh": cube, "deleteEdge": 999})).is_err());
}

#[test]
fn one_point_on_an_edge_inserts_a_vertex() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let points = run(&mut s, "meshInfo", json!({"args": [cube]}))["points"].as_i64().unwrap();
    let cut = run(&mut s, "polyKnife", json!({"mesh": cube, "points": [{"edge": 0, "t": 0.3}]}));
    assert_eq!(len(&cut), 1);
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [cube]}))["points"].as_i64().unwrap(), points + 1);
    assert_eq!(faces(&mut s, &cube), 6);
    assert_eq!(run(&mut s, "selectMode", json!({})), "vertex");
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [cube]}))["points"].as_i64().unwrap(), points);

    let plane = run(&mut s, "polyPlane", json!({"subdivisionsX": 4, "subdivisionsZ": 1}))[0].clone();
    run(&mut s, "setSymmetry", json!({"args": [plane], "axis": "x"}));
    assert_eq!(len(&run(&mut s, "polyKnife", json!({"mesh": plane, "points": [{"edge": 0, "t": 0.5}]}))), 2);
    assert_eq!(run(&mut s, "symmetryCheck", json!({"args": [plane]}))[plane.as_str().unwrap()]["unmatched"], 0);
}
