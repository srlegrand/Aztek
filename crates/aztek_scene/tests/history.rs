//! Construction history: poly edits recorded as nodes after the generator, live until deleted.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn faces(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn max_y(s: &mut Session, m: &str) -> f64 {
    run(s, "meshInfo", json!({"args": [m]}))["boundsMax"][1].as_f64().unwrap()
}

fn names(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect()
}

fn top_face(s: &mut Session, cube: &str) -> i64 {
    let data = run(s, "meshData", json!({"args": [cube]}));
    let pts = data["points"].as_array().unwrap().clone();
    let fs = data["faces"].as_array().unwrap().clone();
    (0..fs.len())
        .find(|&f| fs[f].as_array().unwrap().iter().all(|p| pts[p.as_u64().unwrap() as usize][1].as_f64().unwrap() > 0.0))
        .unwrap() as i64
}

#[test]
fn an_edit_records_a_node_whose_settings_stay_live() {
    let mut s = Session::new();
    let r = run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2}));
    let cube = r[0].as_str().unwrap().to_string();
    let top = top_face(&mut s, &cube);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [top]}));
    run(&mut s, "polyExtrude", json!({"thickness": 1.0}));
    assert_eq!(faces(&mut s, &cube), 10);
    assert!((max_y(&mut s, &cube) - 2.0).abs() < 1e-9);
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1", "polyExtrudeFace1", "polyCube1"]);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["polyExtrudeFace1.inputComponents"]})), json!(format!("f[{top}]")));

    // The node's settings re-run the edit.
    run(&mut s, "setAttr", json!({"args": ["polyExtrudeFace1.thickness", 3.0]}));
    assert!((max_y(&mut s, &cube) - 4.0).abs() < 1e-9);
    // So does the generator's: the extrude follows the taller cube.
    run(&mut s, "setAttr", json!({"args": ["polyCube1.height", 4.0]}));
    assert!((max_y(&mut s, &cube) - 5.0).abs() < 1e-9);

    // Undo takes the node away with the edit.
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &cube), 6);
    assert!(run(&mut s, "ls", json!({"type": "polyExtrudeFace"})).as_array().unwrap().is_empty());
}

#[test]
fn delete_history_keeps_the_result_and_drops_the_nodes() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].as_str().unwrap().to_string();
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0, 1]}));
    run(&mut s, "polyInset", json!({"thickness": 0.1}));
    let inset = faces(&mut s, &cube);
    assert!(inset > 6);
    run(&mut s, "delete", json!({"args": [cube], "constructionHistory": true}));
    assert_eq!(faces(&mut s, &cube), inset);
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1"]);
    assert!(run(&mut s, "ls", json!({"type": "polyInset"})).as_array().unwrap().is_empty());
    assert!(run(&mut s, "ls", json!({"type": "polyCube"})).as_array().unwrap().is_empty());
    run(&mut s, "undo", json!({}));
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1", "polyInset1", "polyCube1"]);
}

#[test]
fn deleting_the_object_takes_its_history() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].as_str().unwrap().to_string();
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 0.5}));
    run(&mut s, "delete", json!({"args": [cube]}));
    assert!(run(&mut s, "ls", json!({"type": "polyExtrudeFace"})).as_array().unwrap().is_empty());
    assert!(run(&mut s, "ls", json!({"type": "polyCube"})).as_array().unwrap().is_empty());
}

#[test]
fn a_mesh_without_history_gets_an_intermediate_copy_and_history_off_records_nothing() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({"constructionHistory": false}))[0].as_str().unwrap().to_string();
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 0.5}));
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1", "polyExtrudeFace1", "polySurfaceShape1"]);
    assert_eq!(faces(&mut s, &cube), 10);

    run(&mut s, "constructionHistory", json!({"toggle": true}));
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run(&mut s, "polyInset", json!({"thickness": 0.1}));
    // History off: the edit bakes what came before it.
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1"]);
    assert_eq!(faces(&mut s, &cube), 14);
}

#[test]
fn moving_points_on_a_mesh_with_history_adds_a_tweak() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2}))[0].as_str().unwrap().to_string();
    let top = top_face(&mut s, &cube);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [top]}));
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    assert!((max_y(&mut s, &cube) - 2.0).abs() < 1e-9);
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1", "polyTweak1", "polyCube1"]);
    // The generator stays live under the tweak.
    run(&mut s, "setAttr", json!({"args": ["polyCube1.height", 4.0]}));
    assert!((max_y(&mut s, &cube) - 3.0).abs() < 1e-9);
    // A second move lands in the same tweak.
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    assert!((max_y(&mut s, &cube) - 4.0).abs() < 1e-9);
    assert_eq!(run(&mut s, "ls", json!({"type": "polyTweak"})).as_array().unwrap().len(), 1);
}

#[test]
fn history_survives_save_and_load() {
    use aztek_scene::scene_file;
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2}))[0].as_str().unwrap().to_string();
    let top = top_face(&mut s, &cube);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [top]}));
    run(&mut s, "polyExtrude", json!({"thickness": 1.0}));
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    let before = run(&mut s, "meshData", json!({"args": [cube]}));
    let text = serde_json::to_string(&scene_file::serialize(&s.graph, false)).unwrap();
    run(&mut s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(run(&mut s, "meshData", json!({"args": [cube]})), before);
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": [cube]}))), ["pCubeShape1", "polyTweak1", "polyExtrudeFace1", "polyCube1"]);
    run(&mut s, "setAttr", json!({"args": ["polyExtrudeFace1.thickness", 2.0]}));
    assert!((max_y(&mut s, &cube) - 4.0).abs() < 1e-9);
}
