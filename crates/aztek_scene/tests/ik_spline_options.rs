//! Spline IK's creation options, as the IK Spline Handle Tool hands them to ikHandle (#317): the curve's spans,
//! simplified or through every joint, parented under the chain's parent, snapped to the root, and the twist's spread.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn wpos(s: &mut Session, o: &str) -> [f64; 3] {
    let v = run(s, "xform", json!({"args": [o], "query": true, "worldSpace": true, "translation": true}));
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

/// Five joints along X under a group, so the chain's start has a parent.
fn chain_under_group(s: &mut Session) {
    run(s, "createNode", json!({"args": ["transform"], "name": "rig"}));
    run(s, "select", json!({"clear": true}));
    for x in 0..5 {
        run(s, "joint", json!({"position": [x as f64, 0, 0]}));
    }
    run(s, "parent", json!({"args": ["joint1", "rig"]}));
}

fn spline(s: &mut Session, extra: Value) -> Value {
    let mut call = json!({"startJoint": "joint1", "endEffector": "joint5", "solver": "ikSplineSolver"});
    for (k, v) in extra.as_object().unwrap() {
        call[k] = v.clone();
    }
    run(s, "ikHandle", call)
}

#[test]
fn a_made_curve_has_one_span_and_goes_under_the_chains_parent() {
    let mut s = Session::new();
    chain_under_group(&mut s);
    let r = spline(&mut s, json!({}));
    assert_eq!(r.as_array().unwrap().len(), 3, "{r}");
    // The reference's -numSpans default is 1: a cubic of four CVs.
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["curve1.spans"]})), json!(1));
    // -parentCurve is on by default.
    let p = run(&mut s, "listRelatives", json!({"args": ["curve1"], "parent": true}));
    assert_eq!(p, json!(["rig"]));
}

#[test]
fn parent_curve_off_leaves_the_curve_at_the_top() {
    let mut s = Session::new();
    chain_under_group(&mut s);
    spline(&mut s, json!({"parentCurve": false}));
    let p = run(&mut s, "listRelatives", json!({"args": ["curve1"], "parent": true}));
    assert!(p.as_array().is_none_or(|a| a.is_empty()), "{p}");
}

#[test]
fn simplify_curve_off_makes_a_curve_through_every_joint() {
    let mut s = Session::new();
    chain_under_group(&mut s);
    spline(&mut s, json!({"simplifyCurve": false, "numSpans": 1}));
    let spans = run(&mut s, "getAttr", json!({"args": ["curve1.spans"]})).as_i64().unwrap();
    assert!(spans > 1, "a curve through five joints has more than one span: {spans}");
}

#[test]
fn snap_curve_moves_a_given_curve_to_the_start_joint() {
    let mut s = Session::new();
    chain_under_group(&mut s);
    run(&mut s, "curve", json!({"degree": 3, "name": "path", "point": [[0, 3, 0], [1, 3, 0], [2, 3, 0], [3, 3, 0], [4, 3, 0]]}));
    spline(&mut s, json!({"curve": "path", "createCurve": false, "snapCurve": true}));
    // The curve came down to the chain, so the chain stays where it was.
    let t = wpos(&mut s, "path");
    assert!((t[1] + 3.0).abs() < 1e-6, "{t:?}");
    let p = wpos(&mut s, "joint1");
    assert!(p[1].abs() < 1e-6, "{p:?}");
}

#[test]
fn twist_type_and_root_twist_mode_are_on_the_handle() {
    let mut s = Session::new();
    chain_under_group(&mut s);
    spline(&mut s, json!({"twistType": "easeIn", "rootTwistMode": true}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["ikHandle1.twistType"]})), json!(1));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["ikHandle1.rootTwistMode"]})), json!(true));
    assert_eq!(run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "query": true, "queried": ["twistType"]})), json!("easeIn"));
    assert!(s.run("ikHandle", json!({"startJoint": "joint2", "endEffector": "joint4", "solver": "ikSplineSolver", "twistType": "wobbly"})).is_err());
}

/// The Y axis's turn about the chain at a joint, degrees, from its world matrix.
fn turn(s: &mut Session, j: &str) -> f64 {
    let m = run(s, "xform", json!({"args": [j], "query": true, "worldSpace": true, "matrix": true}));
    let y = [m[4].as_f64().unwrap(), m[5].as_f64().unwrap(), m[6].as_f64().unwrap()];
    y[2].atan2(y[1]).to_degrees()
}

#[test]
fn ease_in_gathers_the_twist_toward_the_end() {
    let mut s = Session::new();
    chain_under_group(&mut s);
    spline(&mut s, json!({}));
    run(&mut s, "setAttr", json!({"args": ["ikHandle1.twist", 90.0]}));
    let linear = turn(&mut s, "joint3").abs();
    run(&mut s, "setAttr", json!({"args": ["ikHandle1.twistType", 1]}));
    let ease_in = turn(&mut s, "joint3").abs();
    run(&mut s, "setAttr", json!({"args": ["ikHandle1.twistType", 2]}));
    let ease_out = turn(&mut s, "joint3").abs();
    // The third of four driven joints: two thirds of the twist, spread evenly.
    assert!((linear - 60.0).abs() < 1.0, "linear {linear}");
    assert!(ease_in < linear - 5.0, "ease in {ease_in} against {linear}");
    assert!(ease_out > linear + 5.0, "ease out {ease_out} against {linear}");
    // The start joint stays put unless Root twist mode is on.
    assert!(turn(&mut s, "joint1").abs() < 1e-6);
    run(&mut s, "setAttr", json!({"args": ["ikHandle1.twistType", 0]}));
    run(&mut s, "setAttr", json!({"args": ["ikHandle1.rootTwistMode", true]}));
    assert!(turn(&mut s, "joint1").abs() > 1.0);
}
