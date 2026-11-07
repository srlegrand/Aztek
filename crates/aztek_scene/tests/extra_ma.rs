//! The commands' nodes and added attributes through .ma files.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn round_trip(a: &mut Session, name: &str) -> Session {
    let path = std::env::temp_dir().join(format!("aztek_extra_{name}_{}.ma", std::process::id()));
    let path = path.to_string_lossy().to_string();
    run(a, "file", json!({"args": [path], "exportAll": true, "type": "mayaAscii"}));
    let mut b = Session::new();
    run(&mut b, "file", json!({"args": [path], "open": true, "force": true}));
    let _ = std::fs::remove_file(&path);
    b
}

#[test]
fn a_joint_flexor_survives_and_still_bulges() {
    let mut a = Session::new();
    run(&mut a, "polyCylinder", json!({"radius": 1.0, "height": 8.0, "subdivisionsHeight": 8, "subdivisionsAxis": 8, "axis": [1, 0, 0], "name": "limb"}));
    run(&mut a, "select", json!({"clear": true}));
    run(&mut a, "joint", json!({"position": [-4, 0, 0], "name": "j1"}));
    run(&mut a, "joint", json!({"position": [0, 0, 0], "name": "j2"}));
    run(&mut a, "joint", json!({"position": [4, 0, 0], "name": "j3"}));
    run(&mut a, "skinCluster", json!({"args": ["j1", "limb"]}));
    run(&mut a, "flexor", json!({"args": ["j2"], "type": "jointLattice"}));
    run(&mut a, "jointLattice", json!({"args": ["jointLattice1"], "edit": true, "creasing": 3.0}));
    let mut b = round_trip(&mut a, "flexor");
    assert_eq!(run(&mut b, "objectType", json!({"args": ["jointLattice1"]})), json!("jointLattice"));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["jointLattice1.creasing"]})), json!(3.0));
    let links = run(&mut b, "listConnections", json!({"args": ["jointLattice1.outputGeometry"]}));
    assert_eq!(links, json!(["ffd1"]), "still feeds the ffd");
    let rest = run(&mut b, "getAttr", json!({"args": ["jointLattice1.bendMagnitude"]})).as_f64().unwrap();
    assert!(rest < 1e-6, "the rest pose came through: {rest}");
    run(&mut b, "setAttr", json!({"args": ["j2.rotateZ", 60.0]}));
    let bend = run(&mut b, "getAttr", json!({"args": ["jointLattice1.bendMagnitude"]})).as_f64().unwrap();
    assert!(bend > 0.3, "the joints still drive it: {bend}");
}

#[test]
fn combination_shape_survives() {
    let mut a = Session::new();
    for t in ["base", "x", "y", "z"] {
        run(&mut a, "polyCube", json!({"name": t}));
    }
    run(&mut a, "blendShape", json!({"args": ["x", "y", "z", "base"], "name": "bs"}));
    run(&mut a, "combinationShape", json!({"blendShape": "bs", "combinationTargetIndex": 2, "driverTargetIndex": [0, 1], "combineMethod": 1}));
    let mut b = round_trip(&mut a, "combo");
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["combinationShape1.combinationMethod"]})), json!(1));
    run(&mut b, "setAttr", json!({"args": ["bs.w[0]", 0.5]}));
    run(&mut b, "setAttr", json!({"args": ["bs.w[1]", 0.25]}));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["bs.w[2]"]})), json!(0.25));
}

#[test]
fn added_attributes_survive() {
    let mut a = Session::new();
    run(&mut a, "polyCube", json!({"name": "box"}));
    run(&mut a, "instanceable", json!({"args": ["box"], "allow": false}));
    run(&mut a, "dataStructure", json!({"format": "raw", "asString": "name=st:int32=ID"}));
    run(&mut a, "addMetadata", json!({"args": ["boxShape"], "structure": "st", "streamName": "s1", "channelName": "vertex"}));
    run(&mut a, "editMetadata", json!({"args": ["boxShape"], "streamName": "s1", "memberName": "ID", "value": 4, "index": "2"}));
    run(&mut a, "curve", json!({"point": [[0, 0, 0], [1, 2, 0], [3, 2, 0], [4, 0, 0], [6, 1, 0], [7, 0, 0]], "name": "crv"}));
    run(&mut a, "nurbsCurveToBezier", json!({"args": ["crv"], "constructionHistory": false}));
    run(&mut a, "nurbsPlane", json!({"name": "pl", "constructionHistory": false}));
    run(&mut a, "nurbsUVSet", json!({"args": ["pl"], "create": true}));
    run(&mut a, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "relative": false, "uValue": 0.25, "vValue": 0.5}));
    let mut b = round_trip(&mut a, "added");
    assert_eq!(run(&mut b, "instanceable", json!({"args": ["box"], "query": true})), json!([false]));
    run(&mut b, "dataStructure", json!({"format": "raw", "asString": "name=st:int32=ID"}));
    assert_eq!(run(&mut b, "getMetadata", json!({"args": ["boxShape"], "streamName": "s1", "memberName": "ID"})), json!([4]));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["crv.spans"]})), json!(7), "the tripled knots are in the file");
    run(&mut b, "bezierCurveToNurbs", json!({"args": ["crv"]}));
    assert_eq!(run(&mut b, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "query": true})), json!([0.25, 0.5]));
}

#[test]
fn keyed_target_weights_survive() {
    let mut a = Session::new();
    run(&mut a, "polyCube", json!({"name": "base"}));
    run(&mut a, "polyCube", json!({"name": "tgt"}));
    run(&mut a, "move", json!({"args": [0, 1, 0, "tgt.vtx[0]"], "relative": true}));
    run(&mut a, "blendShape", json!({"args": ["tgt", "base"], "name": "bs"}));
    run(&mut a, "setAttr", json!({"args": ["bs.w[0]", 1.0]}));
    run(&mut a, "currentTime", json!({"args": [1]}));
    run(&mut a, "setKeyframeBlendshapeTargetWts", json!({"args": ["base"]}));
    run(&mut a, "setKeyframe", json!({"args": ["bs.targetWeightKeys[0]"], "time": 10, "value": 0.0}));
    assert_eq!(run(&mut a, "keyframe", json!({"args": ["bs.envelope"], "query": true, "keyframeCount": true})), json!(0), "the envelope stays unkeyed");
    assert_eq!(run(&mut a, "keyframe", json!({"args": ["bs.targetWeightKeys[0]"], "query": true, "keyframeCount": true})), json!(2));
    let mut b = round_trip(&mut a, "targetkeys");
    assert_eq!(run(&mut b, "keyframe", json!({"args": ["bs.envelope"], "query": true, "keyframeCount": true})), json!(0), "the envelope stays unkeyed");
    assert_eq!(run(&mut b, "keyframe", json!({"args": ["bs.targetWeightKeys[0]"], "query": true, "keyframeCount": true})), json!(2));
    run(&mut b, "currentTime", json!({"args": [10]}));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["bs.targetWeightKeys[0]"]})), json!(0.0));
}
