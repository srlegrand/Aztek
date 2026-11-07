//! Flexors, combination shapes and the smaller rigging commands.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn point(s: &mut Session, comp: &str) -> [f64; 3] {
    let v = run(s, "pointPosition", json!({"args": [comp]}));
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

fn get(s: &mut Session, plug: &str) -> f64 {
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap_or_else(|| panic!("{plug} is not a number"))
}

/// A cylinder along X, bound to three joints along X.
fn arm(s: &mut Session) {
    run(s, "polyCylinder", json!({"radius": 1.0, "height": 8.0, "subdivisionsHeight": 8, "subdivisionsAxis": 8, "axis": [1, 0, 0], "name": "limb"}));
    run(s, "select", json!({"clear": true}));
    run(s, "joint", json!({"position": [-4, 0, 0], "name": "j1"}));
    run(s, "joint", json!({"position": [0, 0, 0], "name": "j2"}));
    run(s, "joint", json!({"position": [4, 0, 0], "name": "j3"}));
    run(s, "skinCluster", json!({"args": ["j1", "limb"]}));
}

fn all_points(s: &mut Session) -> Vec<[f64; 3]> {
    let n = run(s, "polyEvaluate", json!({"args": ["limb"], "vertex": true})).as_i64().unwrap();
    (0..n).map(|i| point(s, &format!("limb.vtx[{i}]"))).collect()
}

#[test]
fn joint_flexor_bulges_with_the_bend_only() {
    let mut s = Session::new();
    arm(&mut s);
    let rest = all_points(&mut s);
    assert_eq!(run(&mut s, "flexor", json!({"query": true, "list": true})), json!(["jointLattice", "boneLattice", "sculpt"]));
    let made = run(&mut s, "flexor", json!({"args": ["j2"], "type": "jointLattice", "atJoints": true}));
    let node = made[0].as_str().unwrap().to_string();
    assert_eq!(node, "jointLattice1");
    // No bend, no bulge, whatever the creasing.
    run(&mut s, "jointLattice", json!({"args": [node], "edit": true, "creasing": 3.0, "rounding": 1.0}));
    let unbent = all_points(&mut s);
    for (a, b) in rest.iter().zip(&unbent) {
        for k in 0..3 {
            assert!((a[k] - b[k]).abs() < 1e-6, "a flexor moved a point at rest: {a:?} {b:?}");
        }
    }
    // Bent, the same bend with creasing differs from the bend without.
    run(&mut s, "setAttr", json!({"args": ["j2.rotateZ", 60.0]}));
    let creased = all_points(&mut s);
    assert!(get(&mut s, &format!("{node}.bendMagnitude")) > 0.3);
    run(&mut s, "jointLattice", json!({"args": [node], "edit": true, "creasing": 0.0, "rounding": 0.0}));
    let plain = all_points(&mut s);
    let moved = creased.iter().zip(&plain).map(|(a, b)| (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()).fold(0.0, f64::max);
    assert!(moved > 0.05, "creasing did nothing: {moved}");
    assert_eq!(run(&mut s, "jointLattice", json!({"args": [node], "query": true, "queried": ["joint"]})), json!("j2"));
}

#[test]
fn flexor_undo_and_refusals() {
    let mut s = Session::new();
    arm(&mut s);
    assert!(s.run("flexor", json!({"args": ["j1"], "type": "jointLattice"})).is_err(), "a root has no parent bone");
    assert!(s.run("flexor", json!({"args": ["j2"], "type": "nothing"})).is_err());
    run(&mut s, "flexor", json!({"args": ["j2"], "type": "jointLattice"}));
    assert!(s.undo());
    assert_eq!(run(&mut s, "objExists", json!({"args": ["jointLattice1"]})), json!(false));
}

#[test]
fn bone_flexor_and_copy() {
    let mut s = Session::new();
    arm(&mut s);
    let made = run(&mut s, "flexor", json!({"args": ["j1"], "type": "boneLattice", "atBones": true}));
    let node = made[0].as_str().unwrap().to_string();
    run(&mut s, "boneLattice", json!({"args": [node], "edit": true, "bicep": 2.0}));
    assert_eq!(run(&mut s, "boneLattice", json!({"args": [node], "query": true, "queried": ["bicep"]})), json!(2.0));
    let lat = run(&mut s, "listConnections", json!({"args": [format!("{node}.outputGeometry")]}));
    let ffd = lat[0].as_str().unwrap().to_string();
    let copy = run(&mut s, "copyFlexor", json!({"args": [ffd, "j2"]}));
    let copy = copy.as_str().unwrap().to_string();
    assert_eq!(run(&mut s, "boneLattice", json!({"args": [copy], "query": true, "queried": ["bicep"]})), json!(2.0));
}

#[test]
fn combination_shape_drives_its_target() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "base"}));
    for t in ["a", "b", "c"] {
        run(&mut s, "polyCube", json!({"name": t}));
    }
    run(&mut s, "blendShape", json!({"args": ["a", "b", "c", "base"], "name": "bs"}));
    let r = run(&mut s, "combinationShape", json!({"blendShape": "bs", "combinationTargetIndex": 2, "driverTargetIndex": [0, 1]}));
    assert_eq!(r, json!(2));
    assert_eq!(run(&mut s, "combinationShape", json!({"query": true, "blendShape": "bs", "combinationTargetIndex": 2, "exist": true})), json!(true));
    assert_eq!(run(&mut s, "combinationShape", json!({"query": true, "blendShape": "bs", "combinationTargetIndex": 2, "allDrivers": true})), json!([0, 1]));
    run(&mut s, "setAttr", json!({"args": ["bs.w[0]", 0.5]}));
    run(&mut s, "setAttr", json!({"args": ["bs.w[1]", 0.4]}));
    assert!((get(&mut s, "bs.w[2]") - 0.2).abs() < 1e-9, "multiplied");
    run(&mut s, "combinationShape", json!({"edit": true, "blendShape": "bs", "combinationTargetIndex": 2, "combineMethod": 1}));
    assert!((get(&mut s, "bs.w[2]") - 0.4).abs() < 1e-9, "lowest");
    run(&mut s, "combinationShape", json!({"edit": true, "blendShape": "bs", "combinationTargetIndex": 2, "removeDriver": true, "driverTargetIndex": 1}));
    assert_eq!(run(&mut s, "combinationShape", json!({"query": true, "blendShape": "bs", "combinationTargetIndex": 2, "allDrivers": true})), json!([0]));
    run(&mut s, "combinationShape", json!({"edit": true, "blendShape": "bs", "combinationTargetIndex": 2, "removeDriver": true, "allDrivers": true}));
    assert_eq!(run(&mut s, "combinationShape", json!({"query": true, "blendShape": "bs", "combinationTargetIndex": 2, "exist": true})), json!(false));
}

#[test]
fn skeleton_embed_follows_the_long_axis() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 1.0, "height": 10.0, "depth": 1.0, "subdivisionsHeight": 10, "name": "body"}));
    let q = run(&mut s, "skeletonEmbed", json!({"args": ["body"], "query": true}));
    let q = q.as_array().unwrap();
    assert_eq!(q.len(), 5);
    assert!((q[0][1].as_f64().unwrap() + 5.0).abs() < 1e-6 && (q[4][1].as_f64().unwrap() - 5.0).abs() < 1e-6);
    assert!(q[2][0].as_f64().unwrap().abs() < 1e-6);
    let merged = run(&mut s, "skeletonEmbed", json!({"args": ["body"], "query": true, "mergedMesh": true}));
    assert!(merged["faces"].as_array().unwrap().len() > 6);
    let made = run(&mut s, "skeletonEmbed", json!({"args": ["body"], "jointCount": 3}));
    assert_eq!(made.as_array().unwrap().len(), 3);
    assert_eq!(run(&mut s, "objectType", json!({"args": ["joint1"]})), json!("joint"));
}

#[test]
fn characterize_and_hik_helpers() {
    let mut s = Session::new();
    run(&mut s, "joint", json!({"position": [0, 10, 0], "name": "Hips"}));
    run(&mut s, "joint", json!({"position": [0, 12, 0], "name": "Spine"}));
    run(&mut s, "joint", json!({"position": [0, 14, 0], "name": "lump"}));
    let c = run(&mut s, "characterize", json!({"args": ["Hips"], "name": "Bob", "sourceSkeleton": "lump Head"}));
    assert_eq!(c, json!("Bob"));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Bob", "Head"]})), json!("lump"));
    assert_eq!(run(&mut s, "characterize", json!({"query": true, "queried": ["posture"]})), json!("biped"));
    assert!(s.run("characterize", json!({"args": ["Hips"], "addFloorContactPlane": true})).is_err(), "must not pretend");
    let deps = run(&mut s, "hikGetDependNodes", json!({"args": ["Bob"]}));
    assert!(deps.as_array().unwrap().iter().any(|x| x == "lump"));
    assert_eq!(run(&mut s, "hikGlobals", json!({"releaseAllPinning": true})), json!(true));
    assert_eq!(run(&mut s, "hikGlobals", json!({"query": true, "releaseAllPinning": true})), json!(true));
}

#[test]
fn ik_settings_and_effector() {
    let mut s = Session::new();
    run(&mut s, "joint", json!({"position": [0, 0, 0], "name": "a"}));
    run(&mut s, "joint", json!({"position": [1, 0, 0], "name": "b"}));
    run(&mut s, "joint", json!({"position": [2, 0, 0], "name": "c"}));
    run(&mut s, "ikHandle", json!({"startJoint": "a", "endEffector": "c", "name": "h"}));
    assert_eq!(run(&mut s, "ikSystemInfo", json!({"query": true, "globalSnapHandle": true})), json!(true));
    run(&mut s, "ikSystemInfo", json!({"globalSnapHandle": false}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["ikSystem.globalSnap"]})), json!(false));
    assert_eq!(run(&mut s, "effector", json!({"args": ["effector1"], "query": true, "hide": true})), json!(false));
    run(&mut s, "effector", json!({"args": ["effector1"], "edit": true, "hide": true, "name": "eff"}));
    assert_eq!(run(&mut s, "effector", json!({"args": ["eff"], "query": true, "hide": true})), json!(true));
    run(&mut s, "ikfkDisplayMethod", json!({"display": "fk"}));
    assert_eq!(run(&mut s, "ikfkDisplayMethod", json!({"query": true, "queried": ["display"]})), json!("fk"));
}

#[test]
fn instanceable_stops_instance() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    assert_eq!(run(&mut s, "instanceable", json!({"args": ["box"], "query": true})), json!([true]));
    run(&mut s, "instanceable", json!({"args": ["box"], "allow": false, "shape": true}));
    let e = s.run("instance", json!({"args": ["box"]})).unwrap_err().to_string();
    assert!(e.contains("cannot be instanced"), "{e}");
    run(&mut s, "instanceable", json!({"args": ["box"], "allow": true, "shape": true}));
    run(&mut s, "instance", json!({"args": ["box"]}));
}

#[test]
fn stored_settings_read_back() {
    let mut s = Session::new();
    run(&mut s, "xformConstraint", json!({"type": "edge"}));
    assert_eq!(run(&mut s, "xformConstraint", json!({"query": true, "queried": ["type"]})), json!("edge"));
    assert!(s.run("xformConstraint", json!({"type": "sideways"})).is_err());
    assert_eq!(run(&mut s, "xformConstraint", json!({"query": true, "live": true})), json!(false));
    run(&mut s, "weightsColor", json!({"falseColor": true, "deformer": "cluster1"}));
    assert_eq!(run(&mut s, "weightsColor", json!({"query": true, "queried": ["deformer"]})), json!("cluster1"));
    run(&mut s, "relationship", json!({"args": ["ignore", "lightLinker1", "pCube1", "pointLight1"]}));
    assert_eq!(run(&mut s, "relationship", json!({"args": ["ignore", "lightLinker1"], "query": true})), json!(["pCube1", "pointLight1"]));
    run(&mut s, "relationship", json!({"args": ["ignore", "lightLinker1", "pCube1", "pointLight1"], "b": true}));
    assert_eq!(run(&mut s, "relationship", json!({"args": ["ignore", "lightLinker1"], "query": true})), json!([]));
}

#[test]
fn keying_target_weights_animates_them() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "base"}));
    run(&mut s, "polyCube", json!({"name": "tgt"}));
    run(&mut s, "move", json!({"args": [0, 1, 0, "tgt.vtx[0]"], "relative": true}));
    run(&mut s, "blendShape", json!({"args": ["tgt", "base"], "name": "bs"}));
    run(&mut s, "setAttr", json!({"args": ["bs.w[0]", 1.0]}));
    let y0 = point(&mut s, "base.vtx[0]")[1];
    run(&mut s, "currentTime", json!({"args": [1]}));
    let n = run(&mut s, "setKeyframeBlendshapeTargetWts", json!({"args": ["base"]}));
    assert_eq!(n, json!(8));
    let key = "bs.targetWeightKeys[0]";
    run(&mut s, "setKeyframe", json!({"args": [key], "time": 10, "value": 0.0}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["bs.envelope"], "query": true, "keyframeCount": true})), json!(0), "the envelope stays unkeyed");
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["bs.targetWeightKeys[0]"], "query": true, "keyframeCount": true})), json!(2));
    run(&mut s, "currentTime", json!({"args": [10]}));
    let y1 = point(&mut s, "base.vtx[0]")[1];
    assert!(y0 - y1 > 0.5, "the keyed weight should take the delta off: {y0} {y1}");
}

#[test]
fn set_keyframe_keys_an_array_element_not_the_node() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "base"}));
    run(&mut s, "polyCube", json!({"name": "tgt"}));
    run(&mut s, "blendShape", json!({"args": ["tgt", "base"], "name": "bs"}));
    run(&mut s, "setKeyframe", json!({"args": ["bs.w[0]"], "time": 3, "value": 0.5}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["bs.w[0]"], "query": true, "keyframeCount": true})), json!(1));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["bs.envelope"], "query": true, "keyframeCount": true})), json!(0));
    // A component still keys its transform.
    let n = run(&mut s, "setKeyframe", json!({"args": ["base.vtx[0]"], "time": 1})).as_i64().unwrap();
    assert!(n >= 9, "the transform's channels: {n}");
}

#[test]
fn create_curve_warp_runs_curve_warp() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "curve", json!({"point": [[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]], "name": "crv"}));
    let r = run(&mut s, "createCurveWarp", json!({"args": ["box", "crv"]}));
    assert_eq!(r, json!(["curveWarp1"]));
}
