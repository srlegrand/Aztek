//! Keyframes: setKeyframe, playback through currentTime, holds and autokey, keyframe queries and edits, cutKey.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn tx(s: &mut Session) -> f64 {
    run(s, "getAttr", json!({"args": ["pCube1.translateX"]})).as_f64().unwrap()
}

#[test]
fn keys_animate_and_interpolate() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    let n = run(&mut s, "setKeyframe", json!({"args": ["pCube1"]}));
    assert_eq!(n, json!(10));
    run(&mut s, "currentTime", json!({"args": [11]}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.translateX", 10.0]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "translateX"}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    assert!(tx(&mut s).abs() < 1e-9);
    run(&mut s, "currentTime", json!({"args": [6]}));
    assert!((tx(&mut s) - 5.0).abs() < 1e-6, "{}", tx(&mut s));
    run(&mut s, "currentTime", json!({"args": [20]}));
    assert!((tx(&mut s) - 10.0).abs() < 1e-9);
    let kc = run(&mut s, "keyframe", json!({"args": ["pCube1"], "attribute": "tx", "query": true, "queried": ["keyframeCount"]}));
    assert_eq!(kc, json!(2));
    let tc = run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["timeChange"]}));
    assert_eq!(tc, json!([1.0, 11.0]));
    assert_eq!(run(&mut s, "findKeyframe", json!({"args": ["pCube1"], "which": "next", "time": 1})), json!(11));
}

#[test]
fn unkeyed_changes_hold_until_time_moves() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "tx", "time": 1, "value": 0}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "tx", "time": 10, "value": 4}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [3, 0, 0]}));
    assert!((tx(&mut s) - 3.0).abs() < 1e-9);
    run(&mut s, "currentTime", json!({"args": [2]}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    assert!(tx(&mut s).abs() < 1e-9);
    run(&mut s, "autoKeyframe", json!({"state": true}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [2, 0, 0]}));
    run(&mut s, "currentTime", json!({"args": [2]}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    assert!((tx(&mut s) - 2.0).abs() < 1e-9);
    run(&mut s, "autoKeyframe", json!({"state": false}));
}

#[test]
fn cut_key_and_delete_clean_up_curves() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "time": 1}));
    let curves = run(&mut s, "ls", json!({"type": "animCurveTL"}));
    assert_eq!(curves.as_array().unwrap().len(), 3);
    run(&mut s, "cutKey", json!({"args": ["pCube1"], "attribute": "translate"}));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})).as_array().unwrap().len(), 0);
    run(&mut s, "delete", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTU"})).as_array().unwrap().len(), 0);
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTA"})).as_array().unwrap().len(), 0);
}

#[test]
fn tangents_and_infinity() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    for (t, v) in [(1, 0.0), (10, 10.0), (20, 0.0)] {
        run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "ty", "time": t, "value": v}));
    }
    // Auto tangents are flat at the peak: nothing overshoots.
    let mut max: f64 = 0.0;
    for f in 1..=20 {
        run(&mut s, "currentTime", json!({"args": [f]}));
        max = max.max(run(&mut s, "getAttr", json!({"args": ["pCube1.ty"]})).as_f64().unwrap());
    }
    assert!((max - 10.0).abs() < 1e-9);
    run(&mut s, "keyTangent", json!({"args": ["pCube1"], "attribute": "ty", "inTangentType": "linear", "outTangentType": "linear"}));
    run(&mut s, "currentTime", json!({"args": [15]}));
    assert!((run(&mut s, "getAttr", json!({"args": ["pCube1.ty"]})).as_f64().unwrap() - 5.0).abs() < 1e-9);
    run(&mut s, "setAttr", json!({"args": ["pCube1_translateY.postInfinity", 3]}));
    run(&mut s, "currentTime", json!({"args": [29]}));
    assert!((run(&mut s, "getAttr", json!({"args": ["pCube1.ty"]})).as_f64().unwrap() - 10.0).abs() < 1e-9);
}

fn keyed(s: &mut Session) {
    run(s, "polyCube", json!({}));
    for (t, v) in [(1, 0.0), (11, 10.0), (21, 0.0)] {
        run(s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": t, "value": v}));
    }
}

fn times(s: &mut Session, p: &str) -> Vec<f64> {
    run(s, "keyframe", json!({"args": [p], "query": true, "queried": ["timeChange"]})).as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect()
}

#[test]
fn copy_and_paste_keys() {
    let mut s = Session::new();
    keyed(&mut s);
    assert_eq!(run(&mut s, "copyKey", json!({"args": ["pCube1.tx"], "time": "1:11"})), json!(2));
    run(&mut s, "polyCube", json!({}));
    // Pasted at the current time.
    run(&mut s, "currentTime", json!({"args": [5]}));
    assert_eq!(run(&mut s, "pasteKey", json!({"args": ["pCube2.tx"]})), json!(2));
    assert_eq!(times(&mut s, "pCube2.tx"), vec![5.0, 15.0]);
    // Merge overlays; insert moves later keys along.
    run(&mut s, "pasteKey", json!({"args": ["pCube2.tx"], "time": 30, "option": "merge"}));
    assert_eq!(times(&mut s, "pCube2.tx"), vec![5.0, 15.0, 30.0, 40.0]);
    run(&mut s, "pasteKey", json!({"args": ["pCube2.tx"], "time": 15, "option": "insert"}));
    assert_eq!(times(&mut s, "pCube2.tx"), vec![5.0, 15.0, 25.0, 26.0, 41.0, 51.0]);
    // Replace clears the range it covers.
    run(&mut s, "pasteKey", json!({"args": ["pCube2.tx"], "time": 5, "option": "replaceCompletely", "valueOffset": 1.0}));
    assert_eq!(times(&mut s, "pCube2.tx"), vec![5.0, 15.0]);
    let v = run(&mut s, "keyframe", json!({"args": ["pCube2.tx"], "query": true, "queried": ["valueChange"]}));
    assert_eq!(v, json!([1.0, 11.0]));
    // cutKey fills the clipboard; -cl doesn't.
    run(&mut s, "cutKey", json!({"args": ["pCube1.tx"], "time": "21"}));
    assert_eq!(times(&mut s, "pCube1.tx"), vec![1.0, 11.0]);
    run(&mut s, "pasteKey", json!({"args": ["pCube1.tx"], "time": 40, "option": "merge"}));
    assert_eq!(times(&mut s, "pCube1.tx"), vec![1.0, 11.0, 40.0]);
}

#[test]
fn breakdowns_keep_their_place_between_keys() {
    let mut s = Session::new();
    keyed(&mut s);
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 6, "value": 5.0, "breakdown": true}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["breakdown"]})), json!([6.0]));
    // Moving the key after it stretches the breakdown along: halfway stays halfway.
    run(&mut s, "cutKey", json!({"args": ["pCube1.tx"], "time": 21, "clear": true}));
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "timeChange": 21.0}));
    let t = times(&mut s, "pCube1.tx");
    assert_eq!(t, vec![1.0, 11.0, 21.0]);
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "breakdown": false}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["breakdown"]})), json!([]));
}

#[test]
fn scale_snap_and_infinity() {
    let mut s = Session::new();
    keyed(&mut s);
    run(&mut s, "scaleKey", json!({"args": ["pCube1.tx"], "timeScale": 2.0, "timePivot": 1.0, "valueScale": 0.5}));
    assert_eq!(times(&mut s, "pCube1.tx"), vec![1.0, 21.0, 41.0]);
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "time": "21", "queried": ["valueChange"]})), json!([5.0]));
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 21, "timeChange": 21.4}));
    run(&mut s, "snapKey", json!({"args": ["pCube1.tx"]}));
    assert_eq!(times(&mut s, "pCube1.tx"), vec![1.0, 21.0, 41.0]);
    run(&mut s, "setInfinity", json!({"args": ["pCube1.tx"], "postInfinite": "cycle"}));
    assert_eq!(run(&mut s, "setInfinity", json!({"args": ["pCube1.tx"], "query": true, "queried": ["postInfinite"]})), json!(["cycle"]));
    run(&mut s, "currentTime", json!({"args": [61]}));
    assert!((tx(&mut s) - 5.0).abs() < 1e-6, "{}", tx(&mut s));
}

#[test]
fn set_driven_keys() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "door"}));
    run(&mut s, "spaceLocator", json!({"name": "handle"}));
    run(&mut s, "setAttr", json!({"args": ["handle.tx", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["door.ry", 0.0]}));
    run(&mut s, "setDrivenKeyframe", json!({"args": ["door.ry"], "currentDriver": "handle.tx"}));
    run(&mut s, "setDrivenKeyframe", json!({"args": ["door.ry"], "currentDriver": "handle.tx", "driverValue": 10.0, "value": 90.0}));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveUA"})).as_array().unwrap().len(), 1);
    run(&mut s, "setAttr", json!({"args": ["handle.tx", 10.0]}));
    let ry = |s: &mut Session| run(s, "getAttr", json!({"args": ["door.ry"]})).as_f64().unwrap();
    assert!((ry(&mut s) - 90.0).abs() < 1e-6, "{}", ry(&mut s));
    run(&mut s, "setAttr", json!({"args": ["handle.tx", 5.0]}));
    assert!((ry(&mut s) - 45.0).abs() < 8.0, "{}", ry(&mut s));
    assert_eq!(run(&mut s, "setDrivenKeyframe", json!({"args": ["door.ry"], "query": true, "driver": true})), json!(["handle.translateX"]));
    assert_eq!(run(&mut s, "setDrivenKeyframe", json!({"args": ["handle.tx"], "query": true, "driven": true})), json!(["door.rotateY"]));
    // Time doesn't move it, and a time key doesn't show on the slider.
    run(&mut s, "currentTime", json!({"args": [50]}));
    assert!((ry(&mut s) - 45.0).abs() < 8.0);
    assert_eq!(run(&mut s, "keyTimes", json!({"args": ["door"]})), json!([]));
    // A second driver adds to it through a blendWeighted.
    run(&mut s, "spaceLocator", json!({"name": "second"}));
    run(&mut s, "setDrivenKeyframe", json!({"args": ["door.ry"], "currentDriver": "second.tx", "driverValue": 0.0, "value": 0.0}));
    run(&mut s, "setDrivenKeyframe", json!({"args": ["door.ry"], "currentDriver": "second.tx", "driverValue": 1.0, "value": 10.0}));
    assert_eq!(run(&mut s, "ls", json!({"type": "blendWeighted"})).as_array().unwrap().len(), 1);
    run(&mut s, "setAttr", json!({"args": ["handle.tx", 10.0]}));
    run(&mut s, "setAttr", json!({"args": ["second.tx", 1.0]}));
    assert!((ry(&mut s) - 100.0).abs() < 1e-6, "{}", ry(&mut s));
    assert_eq!(run(&mut s, "setDrivenKeyframe", json!({"args": ["door.ry"], "query": true, "driver": true})).as_array().unwrap().len(), 2);
}

#[test]
fn bake_results_keys_what_drives_a_channel() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "leader"}));
    run(&mut s, "spaceLocator", json!({"name": "follower"}));
    run(&mut s, "setKeyframe", json!({"args": ["leader.tx"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["leader.tx"], "time": 11, "value": 10.0}));
    run(&mut s, "pointConstraint", json!({"args": ["leader", "follower"]}));
    assert_eq!(run(&mut s, "bakeResults", json!({"args": ["follower"], "attribute": "translateX", "time": "1:11", "sampleBy": 2})), json!(1));
    assert_eq!(times(&mut s, "follower.tx").len(), 6);
    run(&mut s, "currentTime", json!({"args": [11]}));
    assert!((run(&mut s, "getAttr", json!({"args": ["follower.tx"]})).as_f64().unwrap() - 10.0).abs() < 1e-6);
}
