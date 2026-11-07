//! What the interactive modelling tools run.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn info(s: &mut Session, m: &str) -> Value {
    run(s, "meshInfo", json!({"args": [m]}))
}

#[test]
fn slide_edge_moves_along_the_neighbours() {
    let mut s = Session::new();
    // Three quads in a row along X; the middle edges slide.
    run(&mut s, "polyPlane", json!({"width": 3, "depth": 1, "subdivisionsX": 3, "subdivisionsZ": 1}));
    run(&mut s, "selectMode", json!({"args": ["edge"]}));
    let all = run(&mut s, "ls", json!({"args": ["pPlane1.e[*]"], "flatten": true}));
    // Find the edge at x = -0.5 crossing Z.
    let mut pick = None;
    for e in all.as_array().unwrap() {
        let c = e.as_str().unwrap();
        run(&mut s, "select", json!({"args": [c]}));
        let vs = run(&mut s, "polyListComponentConversion", json!({"args": [c], "toVertex": true}));
        let v = run(&mut s, "xform", json!({"args": vs, "query": true, "worldSpace": true, "translation": true}));
        let p: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        if p.len() == 6 && (p[0] + 0.5).abs() < 1e-6 && (p[3] + 0.5).abs() < 1e-6 {
            pick = Some(c.to_string());
        }
    }
    let e = pick.expect("the edge at x=-0.5");
    run(&mut s, "select", json!({"args": [e.clone()]}));
    run(&mut s, "polySlideEdge", json!({"value": 0.5}));
    let vs = run(&mut s, "polyListComponentConversion", json!({"args": [e.clone()], "toVertex": true}));
    let v = run(&mut s, "xform", json!({"args": vs.clone(), "query": true, "worldSpace": true, "translation": true}));
    let x = v[0].as_f64().unwrap();
    assert!((x.abs() - 0.5).abs() > 0.2, "{v}");
    // One history node, editable.
    assert_eq!(run(&mut s, "ls", json!({"type": "polySlideEdge"})), json!(["polySlideEdge1"]));
    run(&mut s, "setAttr", json!({"args": ["polySlideEdge1.value", 0.0]}));
    let v = run(&mut s, "xform", json!({"args": vs, "query": true, "worldSpace": true, "translation": true}));
    assert!((v[0].as_f64().unwrap() + 0.5).abs() < 1e-6, "{v}");
}

#[test]
fn append_vertex_builds_faces() {
    let mut s = Session::new();
    let r = run(&mut s, "polyAppendVertex", json!({"point": [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]}));
    assert_eq!(r[0], json!("polySurface1"));
    assert_eq!(info(&mut s, "polySurface1")["faces"], json!(1));
    assert_eq!(info(&mut s, "polySurface1")["points"], json!(4));
    // A second face shares an edge and winds to match.
    let r = run(&mut s, "polyAppendVertex", json!({"mesh": "polySurface1", "items": [{"vertex": 1}, {"vertex": 2}, {"position": [2, 0, 1]}, {"position": [2, 0, 0]}]}));
    assert_eq!(r[1], json!("f[1]"));
    let i = info(&mut s, "polySurface1");
    assert_eq!(i["faces"], json!(2));
    assert_eq!(i["points"], json!(6));
    assert_eq!(i["edges"], json!(7));
    assert!(s.undo());
    assert_eq!(info(&mut s, "polySurface1")["faces"], json!(1));
}

#[test]
fn crease_and_live() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polyCrease", json!({"args": ["pCube1.e[0:1]"], "value": 2.0}));
    assert_eq!(run(&mut s, "polyCrease", json!({"args": ["pCube1.e[0]"], "query": true, "queried": ["value"]})), json!([2.0]));
    run(&mut s, "makeLive", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "makeLive", json!({"query": true})), json!(["pCubeShape1"]));
    run(&mut s, "makeLive", json!({"none": true}));
    assert_eq!(run(&mut s, "makeLive", json!({"query": true})), json!([]));
}
