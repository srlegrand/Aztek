//! Weighted tangents, tangent types, filters, key flags, motion paths, animation layers, characters and mute. The clock's
//! settings are process-wide, so what changes them is in anim_units.rs.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn at(s: &mut Session, plug: &str, frame: f64) -> f64 {
    run(s, "currentTime", json!({"args": [frame]}));
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap_or_else(|| panic!("{plug} is not a number"))
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} is not {b}");
}

fn key(s: &mut Session, plug: &str, t: f64, v: f64) {
    run(s, "setKeyframe", json!({"args": [plug], "time": t, "value": v}));
}

fn cube(s: &mut Session) {
    run(s, "polyCube", json!({}));
}

fn floats(v: &Value) -> Vec<f64> {
    v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect()
}

fn q(s: &mut Session, cmd: &str, plug: &str, flag: &str) -> Value {
    run(s, cmd, json!({"args": [plug], "query": true, "queried": [flag]}))
}

// ---------------------------------------------------------------------------------------------------------------------
// Tangents

/// Keys at frames 1 and 25, a second apart at 24 fps, flat tangents.
fn second_long(s: &mut Session) {
    cube(s);
    run(s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 1, "value": 0.0, "inTangentType": "flat", "outTangentType": "flat"}));
    run(s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 25, "value": 10.0, "inTangentType": "flat", "outTangentType": "flat"}));
}

#[test]
fn weighted_curve_matches_hermite_until_its_weights_change() {
    let mut s = Session::new();
    second_long(&mut s);
    // Hermite with flat tangents: 10 * (3s^2 - 2s^3) at s = 0.25.
    near(at(&mut s, "pCube1.tx", 7.0), 1.5625, 1e-9, "hermite");
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "weightedTangents": true}));
    assert_eq!(q(&mut s, "keyTangent", "pCube1.tx", "weightedTangents"), json!([true]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1_translateX.weightedTangents"]})), json!(true));
    near(at(&mut s, "pCube1.tx", 7.0), 1.5625, 1e-7, "bezier with thirds");
    near(at(&mut s, "pCube1.tx", 13.0), 5.0, 1e-7, "bezier middle");

    // A tangent three units long reaches the whole span: control x's 0, 1, 2/3, 1. At s = 0.5, x = 0.75 and y = 5.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 1, "outAngle": 0.0, "outWeight": 3.0}));
    assert_eq!(floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 1, "queried": ["outWeight"]}))), vec![3.0]);
    near(at(&mut s, "pCube1.tx", 19.0), 5.0, 1e-6, "long handle");
    // Hermite doesn't care about length.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "weightedTangents": false}));
    near(at(&mut s, "pCube1.tx", 13.0), 5.0, 1e-9, "back to hermite");
    near(at(&mut s, "pCube1.tx", 7.0), 1.5625, 1e-9, "back to hermite");
}

#[test]
fn weighted_handles_stay_inside_their_span_in_time() {
    let mut s = Session::new();
    second_long(&mut s);
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "weightedTangents": true}));
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "lock": false, "outAngle": 0.0, "inAngle": 0.0, "outWeight": 50.0, "inWeight": 50.0}));
    // Still a function of time: it only rises.
    let mut last = -1.0;
    for f in 1..=25 {
        let v = at(&mut s, "pCube1.tx", f as f64);
        assert!(v >= last - 1e-6, "falls at {f}: {v} after {last}");
        last = v;
    }
    near(last, 10.0, 1e-9, "end");
}

#[test]
fn angles_are_in_value_per_second_and_locks_join_them() {
    let mut s = Session::new();
    cube(&mut s);
    for (t, v) in [(1.0, 0.0), (11.0, 10.0), (21.0, 20.0)] {
        run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": t, "value": v, "inTangentType": "linear", "outTangentType": "linear"}));
    }
    // One unit a frame is 24 a second.
    let want = 24f64.atan().to_degrees();
    near(floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["inAngle"]})))[0], want, 1e-9, "ia");
    let (ix, iy) = (24f64.atan().cos(), 24f64.atan().sin());
    let xy = floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["ix", "iy"]})));
    near(xy[0], ix, 1e-9, "ix");
    near(xy[1], iy, 1e-9, "iy");

    // Locked by default: one angle moves both, and makes them fixed.
    assert_eq!(run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["lock", "weightLock"]})), json!([true, true]));
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "inAngle": 45.0}));
    let both = floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["inAngle", "outAngle"]})));
    near(both[0], 45.0, 1e-9, "ia");
    near(both[1], 45.0, 1e-9, "oa");
    assert_eq!(run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["outTangentType"]})), json!(["fixed"]));

    // Broken, they go their own ways. Relative edits add.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "lock": false}));
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "inAngle": 10.0}));
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "relative": true, "outAngle": 5.0}));
    let both = floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["inAngle", "outAngle"]})));
    near(both[0], 10.0, 1e-9, "broken ia");
    near(both[1], 50.0, 1e-9, "broken oa");

    // Unify gives them one angle.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "unify": true}));
    let both = floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["inAngle", "outAngle"]})));
    near(both[0], 30.0, 1e-9, "unified");
    near(both[1], 30.0, 1e-9, "unified");

    // x and y set the direction.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 1, "ox": 1.0, "oy": 1.0, "lock": false}));
    near(floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 1, "queried": ["outAngle"]})))[0], 45.0, 1e-9, "ox oy");

    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "weightLock": false}));
    assert_eq!(run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 1, "queried": ["weightLock"]})), json!([false]));
}

/// The angle of a slope per frame, as reported.
fn angle(per_frame: f64) -> f64 {
    (per_frame * 24.0).atan().to_degrees()
}

#[test]
fn each_tangent_type_has_its_slopes() {
    let mut s = Session::new();
    cube(&mut s);
    for (t, v) in [(1.0, 0.0), (11.0, 8.0), (21.0, 10.0), (31.0, 10.0), (41.0, 0.0)] {
        key(&mut s, "pCube1.tx", t, v);
    }
    let slopes = |s: &mut Session, ty: &str, t: f64| {
        run(s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "inTangentType": ty, "outTangentType": ty}));
        floats(&run(s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": t, "queried": ["inAngle", "outAngle"]})))
    };
    // At frame 11: the neighbours give 10 over 20, the segments 0.8 and 0.2.
    let v = slopes(&mut s, "spline", 11.0);
    near(v[0], angle(0.5), 1e-9, "spline in");
    near(v[1], angle(0.5), 1e-9, "spline out");
    let v = slopes(&mut s, "linear", 11.0);
    near(v[0], angle(0.8), 1e-9, "linear in");
    near(v[1], angle(0.2), 1e-9, "linear out");
    near(slopes(&mut s, "flat", 11.0)[0], 0.0, 1e-9, "flat");
    near(slopes(&mut s, "clamped", 11.0)[0], angle(0.5), 1e-9, "clamped away from equal neighbours");
    near(slopes(&mut s, "clamped", 21.0)[0], 0.0, 1e-9, "clamped next to an equal key");
    // Auto and plateau limit the slope so nothing overshoots, and are flat at the ends and at extremes.
    for ty in ["auto", "plateau"] {
        near(slopes(&mut s, ty, 11.0)[0], angle(0.5), 1e-9, ty);
        near(slopes(&mut s, ty, 21.0)[0], 0.0, 1e-9, ty);
        near(slopes(&mut s, ty, 1.0)[1], 0.0, 1e-9, ty);
        let mut top: f64 = 0.0;
        for f in 1..=41 {
            top = top.max(at(&mut s, "pCube1.tx", f as f64));
        }
        near(top, 10.0, 1e-9, "no overshoot");
    }
    // Fixed keeps what was there.
    slopes(&mut s, "linear", 11.0);
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "inTangentType": "fixed", "outTangentType": "fixed"}));
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 1, "relative": true, "valueChange": 5.0}));
    let v = floats(&run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 11, "queried": ["inAngle", "outAngle"]})));
    near(v[0], angle(0.8), 1e-9, "fixed in");
    near(v[1], angle(0.2), 1e-9, "fixed out");
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 1, "relative": true, "valueChange": -5.0}));
    // Step holds; step next jumps at once.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "outTangentType": "step"}));
    near(at(&mut s, "pCube1.tx", 10.0), 0.0, 1e-9, "step");
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "outTangentType": "stepnext"}));
    near(at(&mut s, "pCube1.tx", 2.0), 8.0, 1e-9, "step next");
}

#[test]
fn inserting_a_key_keeps_the_curve() {
    for weighted in [false, true] {
        let mut s = Session::new();
        cube(&mut s);
        for (t, v) in [(1.0, 0.0), (11.0, 8.0), (21.0, -3.0), (31.0, 4.0)] {
            key(&mut s, "pCube1.tx", t, v);
        }
        if weighted {
            run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "weightedTangents": true}));
            run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "time": 11, "lock": false, "outAngle": 80.0, "outWeight": 4.0}));
        }
        let before: Vec<f64> = (0..=60).map(|i| at(&mut s, "pCube1.tx", 1.0 + i as f64 * 0.5)).collect();
        assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "insert": true, "time": 15.0})), json!(1));
        assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})), json!(5));
        for (i, want) in before.iter().enumerate() {
            let t = 1.0 + i as f64 * 0.5;
            near(at(&mut s, "pCube1.tx", t), *want, 1e-5, "after insert");
        }
        assert_eq!(run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "query": true, "time": 15, "queried": ["inTangentType"]})), json!(["fixed"]));
        // Nothing to insert into makes no key.
        assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["pCube1.ty"], "insert": true, "time": 15.0})), json!(0));
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Flags

#[test]
fn set_keyframe_flags() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "a"}));
    run(&mut s, "spaceLocator", json!({"name": "b"}));
    run(&mut s, "parent", json!({"args": ["b", "a"]}));
    // Several attributes, and the hierarchy below.
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["a"], "attribute": ["tx", "ty"], "time": 1, "hierarchy": "below"})), json!(4));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["b"], "attribute": "tx", "query": true, "queried": ["keyframeCount"]})), json!(1));
    // The shortest way round.
    key(&mut s, "a.ry", 1.0, 170.0);
    run(&mut s, "setKeyframe", json!({"args": ["a.ry"], "time": 2, "value": -170.0, "minimizeRotation": true}));
    near(floats(&run(&mut s, "keyframe", json!({"args": ["a.ry"], "query": true, "time": 2, "queried": ["valueChange"]})))[0], 190.0, 1e-9, "minimize");
    // Only keyable ones with -rk.
    run(&mut s, "setAttr", json!({"args": ["a.tz"], "keyable": false}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tz"], "keyable": true})), json!(false));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "keyable": true})), json!(true));
    let n = run(&mut s, "setKeyframe", json!({"args": ["a.tz", "a.tx"], "time": 3, "respectKeyable": true}));
    assert_eq!(n, json!(1));
    // S on the object passes over it too; a type's non-keyable channel can be made keyable.
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["a"], "time": 4})), json!(9));
    run(&mut s, "setAttr", json!({"args": ["a.tz"], "keyable": true}));
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["a"], "time": 5})), json!(10));
}

#[test]
fn keyframe_flags() {
    let mut s = Session::new();
    cube(&mut s);
    for t in [1.0, 5.0, 10.0, 15.0] {
        key(&mut s, "pCube1.tx", t, t);
    }
    let tc = |s: &mut Session, extra: Value| {
        let mut call = json!({"args": ["pCube1.tx"], "query": true, "queried": ["timeChange"]});
        for (k, v) in extra.as_object().unwrap() {
            call[k] = v.clone();
        }
        floats(&run(s, "keyframe", call))
    };
    // The upper bound.
    assert_eq!(tc(&mut s, json!({"time": "5:10"})), vec![5.0, 10.0]);
    assert_eq!(tc(&mut s, json!({"time": "5:10", "includeUpperBound": false})), vec![5.0]);
    assert_eq!(tc(&mut s, json!({"index": 1})), vec![5.0]);
    assert_eq!(tc(&mut s, json!({"index": "1:2"})), vec![5.0, 10.0]);
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "time": "5:10", "queried": ["indexValue"]})), json!([1, 2]));
    // Selected keys, and the last of them.
    assert_eq!(run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "time": "5:10"})), json!(2));
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"query": true, "selected": true, "queried": ["selected", "timeChange"]}))), vec![5.0, 10.0]);
    assert_eq!(run(&mut s, "keyframe", json!({"query": true, "selected": true, "queried": ["name"]})), json!(["pCube1_translateX"]));
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"query": true, "lastSelected": true, "queried": ["timeChange"]}))), vec![10.0]);
    // Edits on them, relative and absolute; selection follows the keys.
    run(&mut s, "keyframe", json!({"edit": true, "selected": true, "relative": true, "valueChange": 100.0}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "valueChange")), vec![1.0, 105.0, 110.0, 15.0]);
    run(&mut s, "keyframe", json!({"edit": true, "selected": true, "relative": true, "timeChange": 1.0}));
    assert_eq!(tc(&mut s, json!({})), vec![1.0, 6.0, 11.0, 15.0]);
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"query": true, "selected": true, "queried": ["timeChange"]}))), vec![6.0, 11.0]);
    // Tick colour.
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 1, "tickDrawSpecial": true}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "time": 1, "queried": ["tickDrawSpecial"]})), json!([true]));
    // Evaluate without moving time.
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "inTangentType": "linear", "outTangentType": "linear"}));
    near(floats(&run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "time": 13, "queried": ["eval"]})))[0], 110.0 + (15.0 - 110.0) * 0.5, 1e-9, "eval");
}

#[test]
fn keyframe_options_for_moving_past_keys() {
    let mut s = Session::new();
    let keys = |s: &mut Session| {
        if run(s, "objExists", json!({"args": ["pCube1"]})) == json!(true) {
            run(s, "delete", json!({"args": ["pCube1"]}));
        }
        cube(s);
        for t in [1.0, 5.0, 10.0] {
            key(s, "pCube1.tx", t, t);
        }
    };
    let times = |s: &mut Session| floats(&q(s, "keyframe", "pCube1.tx", "timeChange"));
    keys(&mut s);
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 5, "relative": true, "timeChange": 20.0, "option": "move"}));
    assert_eq!(times(&mut s), vec![1.0, 9.0, 10.0], "move stops short");
    keys(&mut s);
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 5, "relative": true, "timeChange": 20.0, "option": "over"}));
    assert_eq!(times(&mut s), vec![1.0, 10.0, 25.0], "over passes");
    keys(&mut s);
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 5, "relative": true, "timeChange": 3.0, "option": "insert"}));
    assert_eq!(times(&mut s), vec![1.0, 8.0, 13.0], "insert carries the rest");
    keys(&mut s);
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 5, "relative": true, "timeChange": 5.0, "option": "over"}));
    assert_eq!(times(&mut s), vec![1.0, 10.0], "landing on a key replaces it");
}

#[test]
fn cut_key_options() {
    let mut s = Session::new();
    cube(&mut s);
    for t in [1.0, 5.0, 10.0, 15.0, 20.0] {
        key(&mut s, "pCube1.tx", t, t * 2.0);
    }
    let times = |s: &mut Session| floats(&q(s, "keyframe", "pCube1.tx", "timeChange"));
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "time": 5}));
    assert_eq!(run(&mut s, "cutKey", json!({"selected": true, "clear": true})), json!(1));
    assert_eq!(times(&mut s), vec![1.0, 10.0, 15.0, 20.0]);
    run(&mut s, "cutKey", json!({"args": ["pCube1.tx"], "time": "10:15", "clear": true, "includeUpperBound": false}));
    assert_eq!(times(&mut s), vec![1.0, 15.0, 20.0]);

    // -o curve: the keys between go, and the ends of the range hold the curve there.
    let mut s = Session::new();
    cube(&mut s);
    for (t, v) in [(1.0, 0.0), (6.0, 5.0), (11.0, 10.0), (16.0, 15.0), (21.0, 20.0)] {
        run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": t, "value": v, "inTangentType": "linear", "outTangentType": "linear"}));
    }
    let before = at(&mut s, "pCube1.tx", 18.0);
    run(&mut s, "cutKey", json!({"args": ["pCube1.tx"], "time": "8:14", "option": "curve", "clear": true}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "timeChange")), vec![1.0, 6.0, 8.0, 14.0, 16.0, 21.0]);
    near(at(&mut s, "pCube1.tx", 18.0), before, 1e-9, "outside the range");
    near(at(&mut s, "pCube1.tx", 8.0), 7.0, 1e-9, "the range's start");

    // curveCollapse closes the gap.
    let mut s = Session::new();
    cube(&mut s);
    for t in [1.0, 5.0, 10.0, 15.0] {
        key(&mut s, "pCube1.tx", t, t);
    }
    run(&mut s, "cutKey", json!({"args": ["pCube1.tx"], "time": "5:10", "option": "curveCollapse", "clear": true}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "timeChange")), vec![1.0, 10.0]);
}

#[test]
fn select_key_modes() {
    let mut s = Session::new();
    cube(&mut s);
    for t in [1.0, 2.5, 5.0] {
        key(&mut s, "pCube1.tx", t, t);
    }
    let count = |s: &mut Session| run(s, "selectKey", json!({"query": true, "queried": ["keyframeCount"]}));
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "keyframe": true, "time": 1}));
    assert_eq!(count(&mut s), json!(1));
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "addTo": true, "time": 5}));
    assert_eq!(count(&mut s), json!(2));
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "remove": true, "time": 1}));
    assert_eq!(count(&mut s), json!(1));
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "toggle": true, "time": "1:5"}));
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"query": true, "selected": true, "queried": ["timeChange"]}))), vec![1.0, 2.5]);
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "replace": true, "unsnappedKeys": 1.0}));
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"query": true, "selected": true, "queried": ["timeChange"]}))), vec![2.5]);
    run(&mut s, "selectKey", json!({"clear": true}));
    assert_eq!(count(&mut s), json!(0));
}

// ---------------------------------------------------------------------------------------------------------------------
// Rotation interpolation and filters

#[test]
fn slerp_interpolates_at_constant_speed() {
    let mut s = Session::new();
    cube(&mut s);
    for ax in ["rx", "ry", "rz"] {
        key(&mut s, &format!("pCube1.{ax}"), 1.0, 0.0);
    }
    key(&mut s, "pCube1.ry", 11.0, 90.0);
    // Flat Euler tangents ease; 90 * (3s^2 - 2s^3) at a quarter.
    near(at(&mut s, "pCube1.ry", 3.5), 14.0625, 1e-9, "euler");
    assert_eq!(run(&mut s, "rotationInterpolation", json!({"args": ["pCube1"], "convert": "quaternionSlerp"})), json!(3));
    // Keys are kept together on all axes.
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.rx"], "query": true, "queried": ["keyframeCount"]})), json!(2));
    assert_eq!(run(&mut s, "rotationInterpolation", json!({"args": ["pCube1_rotateY"], "query": true})), json!(["quaternionSlerp"]));
    near(at(&mut s, "pCube1.ry", 3.5), 22.5, 1e-6, "slerp");
    near(at(&mut s, "pCube1.rx", 3.5), 0.0, 1e-6, "slerp other axis");
    near(at(&mut s, "pCube1.ry", 11.0), 90.0, 1e-6, "slerp end");
    // Squad passes through the keys too.
    run(&mut s, "rotationInterpolation", json!({"args": ["pCube1"], "convert": "quaternionSquad"}));
    near(at(&mut s, "pCube1.ry", 1.0), 0.0, 1e-6, "squad start");
    near(at(&mut s, "pCube1.ry", 11.0), 90.0, 1e-6, "squad end");
    // Editing one curve reaches the others.
    run(&mut s, "rotationInterpolation", json!({"args": ["pCube1"], "convert": "quaternionSlerp"}));
    run(&mut s, "keyframe", json!({"args": ["pCube1.ry"], "edit": true, "time": 11, "valueChange": 45.0}));
    near(at(&mut s, "pCube1.ry", 6.0), 22.5, 1e-6, "after an edit");
    // Back to independent curves.
    run(&mut s, "rotationInterpolation", json!({"args": ["pCube1"], "convert": "none"}));
    near(at(&mut s, "pCube1.ry", 3.5), 7.03125, 1e-6, "none");
}

#[test]
fn euler_filter_removes_turns_and_flips() {
    let mut s = Session::new();
    cube(&mut s);
    for (t, x, y, z) in [(1.0, 0.0, 0.0, 170.0), (2.0, 0.0, 0.0, -170.0), (3.0, 0.0, 0.0, -150.0)] {
        key(&mut s, "pCube1.rx", t, x);
        key(&mut s, "pCube1.ry", t, y);
        key(&mut s, "pCube1.rz", t, z);
    }
    assert_eq!(run(&mut s, "filterCurve", json!({"args": ["pCube1"]})), json!(3));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.rz", "valueChange")), vec![170.0, 190.0, 210.0]);

    // The other Euler solution of the same rotation: x + 180, 180 - y, z + 180.
    let mut s = Session::new();
    cube(&mut s);
    for (t, x, y, z) in [(1.0, 10.0, 20.0, 30.0), (2.0, 192.0, 158.0, 212.0), (3.0, 14.0, 24.0, 34.0)] {
        key(&mut s, "pCube1.rx", t, x);
        key(&mut s, "pCube1.ry", t, y);
        key(&mut s, "pCube1.rz", t, z);
    }
    run(&mut s, "filterCurve", json!({"args": ["pCube1_rotateX", "pCube1_rotateY", "pCube1_rotateZ"], "filter": "euler"}));
    for (ax, want) in [("rx", [10.0, 12.0, 14.0]), ("ry", [20.0, 22.0, 24.0]), ("rz", [30.0, 32.0, 34.0])] {
        let got = floats(&q(&mut s, "keyframe", &format!("pCube1.{ax}"), "valueChange"));
        for i in 0..3 {
            near(got[i], want[i], 1e-9, ax);
        }
    }
}

fn spike(s: &mut Session) {
    cube(s);
    for f in 1..=25 {
        key(s, "pCube1.tx", f as f64, if f == 13 { 10.0 } else { 0.0 });
    }
}

#[test]
fn filters_smooth_resample_and_reduce() {
    // Low pass and gaussian both lower the spike and share it with its neighbours.
    for (filter, extra) in [("butterworth", json!({"cutoffFrequency": 3.0})), ("gaussian", json!({"width": 1.0}))] {
        let mut s = Session::new();
        spike(&mut s);
        let mut call = json!({"args": ["pCube1.tx"], "filter": filter});
        for (k, v) in extra.as_object().unwrap() {
            call[k] = v.clone();
        }
        assert_eq!(run(&mut s, "filterCurve", call), json!(1));
        let v = floats(&q(&mut s, "keyframe", "pCube1.tx", "valueChange"));
        assert_eq!(v.len(), 25);
        assert!(v[12] < 6.0 && v[12] > 1.0, "{filter}: {}", v[12]);
        assert!(v[11] > 0.5, "{filter}: {}", v[11]);
        near(v[11], v[13], 1e-3, "keeps symmetry: no shift in time");
    }
    // A gaussian of width 1 keeps a weight of 1 / sum at the middle: the kernel exp(-i^2 / 2) over -3..3.
    let mut s = Session::new();
    spike(&mut s);
    run(&mut s, "filterCurve", json!({"args": ["pCube1.tx"], "filter": "gaussian", "width": 1.0}));
    let sum: f64 = (-3..=3).map(|i: i32| (-0.5 * (i * i) as f64).exp()).sum();
    near(floats(&q(&mut s, "keyframe", "pCube1.tx", "valueChange"))[12], 10.0 / sum, 1e-9, "gaussian middle");

    // Resample every 4 frames: a sixth of a second.
    let mut s = Session::new();
    spike(&mut s);
    run(&mut s, "filterCurve", json!({"args": ["pCube1.tx"], "filter": "resample", "period": 1.0 / 6.0}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "timeChange")), vec![1.0, 5.0, 9.0, 13.0, 17.0, 21.0, 25.0]);

    // The peak remover takes the spike and nothing else.
    let mut s = Session::new();
    spike(&mut s);
    run(&mut s, "filterCurve", json!({"args": ["pCube1.tx"], "filter": "peak_remove", "tolerance": 1.0}));
    let t = floats(&q(&mut s, "keyframe", "pCube1.tx", "timeChange"));
    assert_eq!(t.len(), 24);
    assert!(!t.contains(&13.0));
    assert!(floats(&q(&mut s, "keyframe", "pCube1.tx", "valueChange")).iter().all(|v| *v == 0.0));

    // Key sync: both curves get all the times, and keep their shape.
    let mut s = Session::new();
    cube(&mut s);
    key(&mut s, "pCube1.tx", 1.0, 0.0);
    key(&mut s, "pCube1.tx", 11.0, 10.0);
    key(&mut s, "pCube1.ty", 1.0, 0.0);
    key(&mut s, "pCube1.ty", 6.0, 3.0);
    let before = at(&mut s, "pCube1.tx", 6.0);
    run(&mut s, "filterCurve", json!({"args": ["pCube1_translateX", "pCube1_translateY"], "filter": "key_sync"}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "timeChange")), vec![1.0, 6.0, 11.0]);
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.ty", "timeChange")), vec![1.0, 6.0, 11.0]);
    near(at(&mut s, "pCube1.tx", 6.0), before, 1e-9, "key sync");
}

fn ramp(s: &mut Session) {
    cube(s);
    for f in 0..=20 {
        // A straight line, with one key off it by a little.
        let bump = if f == 10 { 0.001 } else { 0.0 };
        run(s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": f as f64, "value": f as f64 + bump, "inTangentType": "linear", "outTangentType": "linear"}));
    }
}

#[test]
fn simplify_and_key_reducer_remove_what_makes_no_difference() {
    let mut s = Session::new();
    ramp(&mut s);
    let before: Vec<f64> = (0..=40).map(|i| at(&mut s, "pCube1.tx", i as f64 * 0.5)).collect();
    run(&mut s, "simplify", json!({"args": ["pCube1.tx"], "valueTolerance": 0.01, "timeTolerance": 0.05}));
    let left = run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})).as_i64().unwrap();
    assert_eq!(left, 2, "a line needs two keys");
    for (i, want) in before.iter().enumerate() {
        near(at(&mut s, "pCube1.tx", i as f64 * 0.5), *want, 0.01, "within the tolerance");
    }
    // A tighter tolerance keeps the bump.
    let mut s = Session::new();
    ramp(&mut s);
    run(&mut s, "simplify", json!({"args": ["pCube1.tx"], "valueTolerance": 0.0001}));
    let left = run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})).as_i64().unwrap();
    assert!(left > 2 && left < 21, "{left}");
    // A time range leaves the rest alone.
    let mut s = Session::new();
    ramp(&mut s);
    run(&mut s, "simplify", json!({"args": ["pCube1.tx"], "time": "5:8", "valueTolerance": 0.01}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})), json!(17));
    // Key reducer: a percentage of the range.
    let mut s = Session::new();
    ramp(&mut s);
    run(&mut s, "filterCurve", json!({"args": ["pCube1.tx"], "filter": "keyReducer", "precision": 1.0}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})), json!(2));
}

#[test]
fn buffer_curves_swap() {
    let mut s = Session::new();
    cube(&mut s);
    key(&mut s, "pCube1.tx", 1.0, 1.0);
    key(&mut s, "pCube1.tx", 2.0, 2.0);
    assert_eq!(run(&mut s, "bufferCurve", json!({"args": ["pCube1.tx"], "query": true, "exists": true})), json!(false));
    run(&mut s, "bufferCurve", json!({"args": ["pCube1.tx"]}));
    assert_eq!(run(&mut s, "bufferCurve", json!({"args": ["pCube1.tx"], "query": true, "exists": true})), json!(true));
    run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "edit": true, "time": 2, "valueChange": 9.0}));
    run(&mut s, "bufferCurve", json!({"args": ["pCube1.tx"], "swap": true}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "valueChange")), vec![1.0, 2.0]);
    run(&mut s, "bufferCurve", json!({"args": ["pCube1.tx"], "swap": true}));
    assert_eq!(floats(&q(&mut s, "keyframe", "pCube1.tx", "valueChange")), vec![1.0, 9.0]);
    // The stats fields.
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"], "time": 2}));
    assert_eq!(run(&mut s, "keyframeStats", json!({"query": true, "queried": ["timeAnnotation"]})), json!("2"));
    assert_eq!(run(&mut s, "keyframeStats", json!({"query": true, "queried": ["valueAnnotation"]})), json!("9"));
    run(&mut s, "selectKey", json!({"args": ["pCube1.tx"]}));
    assert_eq!(run(&mut s, "keyframeStats", json!({"query": true, "queried": ["timeAnnotation"]})), json!(""));
}

// ---------------------------------------------------------------------------------------------------------------------
// Mute

#[test]
fn mute_holds_a_channel_and_keys_carry_on_underneath() {
    let mut s = Session::new();
    cube(&mut s);
    key(&mut s, "pCube1.tx", 1.0, 0.0);
    key(&mut s, "pCube1.tx", 11.0, 10.0);
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "inTangentType": "linear", "outTangentType": "linear"}));
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "before");
    assert_eq!(q(&mut s, "mute", "pCube1.tx", "disable"), json!(false));
    assert_eq!(run(&mut s, "mute", json!({"args": ["pCube1.tx"]})), json!(["mute_pCube1_translateX"]));
    assert_eq!(run(&mut s, "mute", json!({"args": ["pCube1.tx"], "query": true})), json!(true));
    near(at(&mut s, "pCube1.tx", 11.0), 5.0, 1e-9, "muted");
    // The curve is still found, and a key goes on it.
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})), json!(2));
    key(&mut s, "pCube1.tx", 21.0, 20.0);
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["keyframeCount"]})), json!(3));
    // Off, the node stays; forced off, it goes and the curve drives again.
    run(&mut s, "mute", json!({"args": ["pCube1.tx"], "disable": true}));
    assert_eq!(run(&mut s, "mute", json!({"args": ["pCube1.tx"], "query": true})), json!(false));
    near(at(&mut s, "pCube1.tx", 11.0), 10.0, 1e-9, "unmuted");
    assert_eq!(run(&mut s, "ls", json!({"type": "mute"})).as_array().unwrap().len(), 1);
    run(&mut s, "mute", json!({"args": ["pCube1.tx"], "disable": true, "force": true}));
    assert_eq!(run(&mut s, "ls", json!({"type": "mute"})).as_array().unwrap().len(), 0);
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "driven again");
    // A channel with no keys can be muted too, and keyed afterwards.
    run(&mut s, "setAttr", json!({"args": ["pCube1.ty", 2.0]}));
    run(&mut s, "muteChannel", json!({"args": ["pCube1"], "attribute": "ty"}));
    near(at(&mut s, "pCube1.ty", 3.0), 2.0, 1e-9, "muted static");
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["pCube1.ty"], "time": 1, "value": 7.0})), json!(1));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})).as_array().unwrap().len(), 2);
    near(at(&mut s, "pCube1.ty", 3.0), 2.0, 1e-9, "still muted");
}

// ---------------------------------------------------------------------------------------------------------------------
// Motion paths

fn line(s: &mut Session, to: [f64; 3]) {
    run(s, "curve", json!({"degree": 1, "point": [[0.0, 0.0, 0.0], to], "name": "path"}));
}

fn linear_u(s: &mut Session, mp: &str) {
    run(s, "keyTangent", json!({"args": [format!("{mp}.uValue")], "edit": true, "inTangentType": "linear", "outTangentType": "linear"}));
}

#[test]
fn path_animation_moves_along_the_curve() {
    let mut s = Session::new();
    line(&mut s, [10.0, 0.0, 0.0]);
    cube(&mut s);
    let mp = run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "path", "fractionMode": true, "startTimeU": 1.0, "endTimeU": 11.0}));
    assert_eq!(mp, json!("motionPath1"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["motionPath1"]})), json!("motionPath"));
    // uValue's curve, under the reference's name.
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!(["motionPath1_uValue"]));
    near(at(&mut s, "pCube1.tx", 1.0), 0.0, 1e-9, "start");
    near(at(&mut s, "pCube1.tx", 11.0), 10.0, 1e-9, "end");
    linear_u(&mut s, "motionPath1");
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "middle");
    near(at(&mut s, "pCube1.ty", 6.0), 0.0, 1e-9, "on the line");
    assert_eq!(q(&mut s, "pathAnimation", "motionPath1", "curve"), json!("path"));
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "startTimeU"), json!(1));
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "endTimeU"), json!(11));
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "fractionMode"), json!(true));
    // Directly connected, with no addDoubleLinear.
    assert_eq!(run(&mut s, "ls", json!({"type": "addDoubleLinear"})), json!([]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.translate"], "source": true, "destination": false})), json!(["motionPath1"]));
    // The curve moving takes the object with it.
    run(&mut s, "setAttr", json!({"args": ["path.ty", 3.0]}));
    near(at(&mut s, "pCube1.ty", 6.0), 3.0, 1e-9, "curve moved");
    // Going second node gets its own name.
    run(&mut s, "spaceLocator", json!({"name": "other"}));
    assert_eq!(run(&mut s, "pathAnimation", json!({"args": ["other"], "curve": "path"})), json!("motionPath2"));
}

#[test]
fn parametric_and_fraction_modes_differ_on_uneven_curves() {
    let mut s = Session::new();
    // Three points on a line, the middle one near the start: as a degree 1 curve its spans are equal in u, not length.
    run(&mut s, "curve", json!({"degree": 1, "point": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [10.0, 0.0, 0.0]], "name": "path"}));
    cube(&mut s);
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "path"}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.uValue", 1.5]}));
    near(run(&mut s, "getAttr", json!({"args": ["pCube1.tx"]})).as_f64().unwrap(), 5.5, 1e-9, "parametric");
    run(&mut s, "setAttr", json!({"args": ["motionPath1.fractionMode", true]}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.uValue", 0.5]}));
    near(run(&mut s, "getAttr", json!({"args": ["pCube1.tx"]})).as_f64().unwrap(), 5.0, 1e-2, "by length");
}

fn euler(s: &mut Session) -> [f64; 3] {
    let f = |s: &mut Session, a: &str| run(s, "getAttr", json!({"args": [format!("pCube1.{a}")]})).as_f64().unwrap();
    [f(s, "rx"), f(s, "ry"), f(s, "rz")]
}

fn close(a: [f64; 3], b: [f64; 3], what: &str) {
    for i in 0..3 {
        near(a[i], b[i], 1e-6, what);
    }
}

#[test]
fn follow_turns_the_object_along_the_path() {
    let mut s = Session::new();
    line(&mut s, [0.0, 0.0, 10.0]);
    cube(&mut s);
    run(&mut s, "setAttr", json!({"args": ["pCube1.rx", 7.0]}));
    // Without follow rotation is left alone.
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "path"}));
    close(euler(&mut s), [7.0, 0.0, 0.0], "no follow");
    // X down the path, which runs along +Z: a turn of -90 about Y.
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "follow": true}));
    close(euler(&mut s), [0.0, -90.0, 0.0], "follow");
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "followAxis"), json!("x"));
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "upAxis"), json!("y"));
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "inverseFront": true}));
    close(euler(&mut s), [0.0, 90.0, 0.0], "inverse front");
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "inverseFront": false, "inverseUp": true}));
    // X along +Z, Y down, so Z = X cross Y = +X.
    let m = run(&mut s, "getAttr", json!({"args": ["motionPath1.orientMatrix"]}));
    let m = floats(&json!(m.as_array().unwrap().iter().flat_map(|r| r.as_array().cloned().unwrap_or_else(|| vec![r.clone()])).collect::<Vec<_>>()));
    for (i, want) in [0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0].iter().enumerate() {
        near(m[i], *want, 1e-9, "orient matrix");
    }
    // Front Z, up Y: no turn at all.
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "inverseUp": false, "followAxis": "z", "upAxis": "y"}));
    close(euler(&mut s), [0.0, 0.0, 0.0], "front z");
    assert!(s.run("pathAnimation", json!({"args": ["pCube1"], "edit": true, "followAxis": "y"})).is_err(), "front and up the same");
}

#[test]
fn world_up_types_and_twists() {
    let mut s = Session::new();
    line(&mut s, [10.0, 0.0, 0.0]);
    cube(&mut s);
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "path", "follow": true, "followAxis": "x", "upAxis": "y", "worldUpType": "vector", "worldUpVector": [0.0, 1.0, 0.0]}));
    close(euler(&mut s), [0.0, 0.0, 0.0], "up is Y");
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "worldUpType"), json!("vector"));
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "worldUpVector"), json!([0.0, 1.0, 0.0]));
    // Up along Z turns it a quarter about the path.
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "worldUpVector": [0.0, 0.0, 1.0]}));
    close(euler(&mut s), [90.0, 0.0, 0.0], "up is Z");
    // Up towards an object: at Y -5, so up points down.
    run(&mut s, "spaceLocator", json!({"name": "above"}));
    run(&mut s, "setAttr", json!({"args": ["above.ty", -5.0]}));
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "worldUpObject": "above", "worldUpType": "object"}));
    close(euler(&mut s), [180.0, 0.0, 0.0], "up towards the object");
    assert_eq!(q(&mut s, "pathAnimation", "pCube1", "worldUpObject"), json!("above"));
    // Up as an object's rotation of the vector.
    run(&mut s, "setAttr", json!({"args": ["above.rx", 90.0]}));
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "worldUpType": "objectrotation", "worldUpVector": [0.0, 1.0, 0.0]}));
    close(euler(&mut s), [90.0, 0.0, 0.0], "object rotation");
    // Twist about the front axis.
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "worldUpType": "scene"}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.frontTwist", 30.0]}));
    close(euler(&mut s), [30.0, 0.0, 0.0], "front twist");
    run(&mut s, "setAttr", json!({"args": ["motionPath1.frontTwist", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.upTwist", 30.0]}));
    close(euler(&mut s), [0.0, 30.0, 0.0], "up twist");
    run(&mut s, "setAttr", json!({"args": ["motionPath1.upTwist", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.sideTwist", 30.0]}));
    close(euler(&mut s), [0.0, 0.0, 30.0], "side twist");
}

#[test]
fn banking_leans_into_a_circle_up_to_the_limit() {
    let mut s = Session::new();
    run(&mut s, "circle", json!({"name": "ring", "normal": [0.0, 1.0, 0.0], "radius": 5.0, "sections": 16}));
    cube(&mut s);
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "ring", "follow": true, "fractionMode": true, "worldUpType": "vector"}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.uValue", 0.3]}));
    let p = |s: &mut Session| {
        let f = |s: &mut Session, a: &str| run(s, "getAttr", json!({"args": [format!("pCube1.{a}")]})).as_f64().unwrap();
        [f(s, "tx"), f(s, "ty"), f(s, "tz")]
    };
    let at_u = p(&mut s);
    near((at_u[0] * at_u[0] + at_u[2] * at_u[2]).sqrt(), 5.0, 0.02, "on the circle");
    near(at_u[1], 0.0, 1e-9, "in its plane");
    // The object's up axis, column Y of the orient matrix.
    let up = |s: &mut Session| {
        let m = run(s, "getAttr", json!({"args": ["motionPath1.orientMatrix"]}));
        let v: Vec<f64> = m.as_array().unwrap().iter().flat_map(|r| r.as_array().cloned().unwrap_or_else(|| vec![r.clone()])).map(|x| x.as_f64().unwrap()).collect();
        [v[4], v[5], v[6]]
    };
    near(up(&mut s)[1], 1.0, 1e-6, "flat without bank");
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "bank": true, "bankScale": 5.0}));
    let u = up(&mut s);
    // A radius of 5 turns 1/5 rad a unit: 11.46 degrees, times five.
    let lean = (11.459155902616466f64 * 5.0).to_radians();
    near(u[1], lean.cos(), 2e-3, "leans by the curvature");
    // Towards the centre.
    let dot = u[0] * -at_u[0] + u[2] * -at_u[2];
    assert!(dot > 0.0, "leans out of the turn: {dot}");
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "edit": true, "bankThreshold": 10.0}));
    near(up(&mut s)[1], 10f64.to_radians().cos(), 1e-9, "the limit");
}

#[test]
fn a_parented_object_stays_on_the_path() {
    let mut s = Session::new();
    line(&mut s, [10.0, 0.0, 0.0]);
    run(&mut s, "spaceLocator", json!({"name": "holder"}));
    run(&mut s, "setAttr", json!({"args": ["holder.tx", 100.0]}));
    cube(&mut s);
    run(&mut s, "parent", json!({"args": ["pCube1", "holder"]}));
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "path", "fractionMode": true}));
    run(&mut s, "setAttr", json!({"args": ["motionPath1.uValue", 0.5]}));
    near(run(&mut s, "getAttr", json!({"args": ["pCube1.tx"]})).as_f64().unwrap(), -95.0, 1e-9, "in its parent's space");
}

#[test]
fn deleting_the_object_takes_its_motion_path() {
    let mut s = Session::new();
    line(&mut s, [10.0, 0.0, 0.0]);
    cube(&mut s);
    run(&mut s, "pathAnimation", json!({"args": ["pCube1"], "curve": "path", "startTimeU": 1.0, "endTimeU": 11.0}));
    run(&mut s, "delete", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "ls", json!({"type": "motionPath"})), json!([]));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!([]));
}

#[test]
fn snapshot_leaves_copies_along_the_animation() {
    let mut s = Session::new();
    cube(&mut s);
    key(&mut s, "pCube1.tx", 1.0, 0.0);
    key(&mut s, "pCube1.tx", 11.0, 10.0);
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "inTangentType": "linear", "outTangentType": "linear"}));
    run(&mut s, "currentTime", json!({"args": [3]}));
    let r = run(&mut s, "snapshot", json!({"args": ["pCube1"], "startTime": 1.0, "endTime": 11.0, "increment": 5.0, "constructionHistory": true}));
    assert_eq!(r, json!(["snapshot1Group", "snapshot1"]));
    assert_eq!(run(&mut s, "currentTime", json!({"query": true})), json!(3), "comes back to the frame it was on");
    let kids = run(&mut s, "listRelatives", json!({"args": ["snapshot1Group"]}));
    let kids: Vec<String> = kids.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
    assert_eq!(kids.len(), 3);
    for (k, want) in kids.iter().zip([0.0, 5.0, 10.0]) {
        near(run(&mut s, "getAttr", json!({"args": [format!("snapshot1Group|{k}.tx")]})).as_f64().unwrap(), want, 1e-9, "copy");
    }
    // Static copies: time doesn't move them.
    run(&mut s, "currentTime", json!({"args": [11]}));
    near(run(&mut s, "getAttr", json!({"args": [format!("snapshot1Group|{}.tx", kids[0])]})).as_f64().unwrap(), 0.0, 1e-9, "static");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["snapshot1.increment"]})), json!(5.0));
}

#[test]
fn motion_trails_keep_the_points() {
    let mut s = Session::new();
    cube(&mut s);
    key(&mut s, "pCube1.tx", 1.0, 0.0);
    key(&mut s, "pCube1.tx", 5.0, 4.0);
    run(&mut s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "inTangentType": "linear", "outTangentType": "linear"}));
    let r = run(&mut s, "snapshot", json!({"args": ["pCube1"], "motionTrail": true, "startTime": 1.0, "endTime": 5.0, "increment": 1.0}));
    assert_eq!(r, json!(["motionTrail1Handle", "motionTrail1"]));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["motionTrail1Handle|motionTrail1HandleShape"]})), json!("motionTrailShape"));
    let points = q(&mut s, "motionTrail", "motionTrail1", "points");
    let xs: Vec<f64> = points.as_array().unwrap().iter().map(|p| p[0].as_f64().unwrap()).collect();
    assert_eq!(xs, vec![0.0, 1.0, 2.0, 3.0, 4.0]);
    assert_eq!(q(&mut s, "motionTrail", "motionTrail1Handle", "keyframeTimes"), json!([1.0, 5.0]));
    // Editing the animation updates it, and what is drawn.
    key(&mut s, "pCube1.tx", 3.0, 100.0);
    let points = q(&mut s, "motionTrail", "motionTrail1", "points");
    assert_eq!(points[2][0], json!(100.0));
    let shape = s.graph.find("motionTrail1HandleShape").unwrap().unwrap();
    let segs = aztek_scene::motion_path::trail_lines(s.graph.node(shape).unwrap());
    // Four between five points, three lines at each of three keys.
    assert_eq!(segs.len(), 4 + 9);
    assert_eq!(segs[1].1.x, 100.0);
    // The command's own form.
    let r = run(&mut s, "motionTrail", json!({"args": ["pCube1"], "startTime": 1.0, "endTime": 2.0}));
    assert_eq!(r, json!(["motionTrail2Handle", "motionTrail2"]));
}

// ---------------------------------------------------------------------------------------------------------------------
// Animation layers

/// tx rises 0 to 10 over frames 1 to 11, on the base.
fn layered(s: &mut Session) {
    cube(s);
    key(s, "pCube1.tx", 1.0, 0.0);
    key(s, "pCube1.tx", 11.0, 10.0);
    run(s, "keyTangent", json!({"args": ["pCube1.tx"], "edit": true, "inTangentType": "linear", "outTangentType": "linear"}));
    run(s, "animLayer", json!({"args": ["L1"]}));
    run(s, "animLayer", json!({"args": ["L1"], "edit": true, "attribute": ["pCube1.tx"]}));
    run(s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L1", "time": 1, "value": 3.0, "noResolve": true}));
}

#[test]
fn layers_build_the_reference_nodes() {
    let mut s = Session::new();
    layered(&mut s);
    assert_eq!(run(&mut s, "animLayer", json!({"query": true, "queried": ["root"]})), json!("BaseAnimation"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["L1"]})), json!("animLayer"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["pCube1_translateX_L1"]})), json!("animBlendNodeAdditiveDL"));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!(["pCube1_translateX", "pCube1_translateX_L1_inputB"]));
    assert_eq!(q(&mut s, "animLayer", "L1", "parent"), json!("BaseAnimation"));
    assert_eq!(q(&mut s, "animLayer", "BaseAnimation", "children"), json!(["L1"]));
    assert_eq!(q(&mut s, "animLayer", "L1", "attribute"), json!(["pCube1.translateX"]));
    assert_eq!(q(&mut s, "animLayer", "L1", "animCurves"), json!(["pCube1_translateX_L1_inputB"]));
    assert_eq!(q(&mut s, "animLayer", "L1", "baseAnimCurves"), json!(["pCube1_translateX"]));
    assert_eq!(q(&mut s, "animLayer", "L1", "blendNodes"), json!(["pCube1_translateX_L1"]));
    assert_eq!(q(&mut s, "animLayer", "pCube1.tx", "affectedLayers"), json!(["BaseAnimation", "L1"]));
    assert_eq!(q(&mut s, "animLayer", "L1", "exists"), json!(true));
    assert_eq!(run(&mut s, "animLayer", json!({"query": true, "queried": ["layeredHierarchy"]})), json!(["BaseAnimation", "L1"]));
    // A name left out gets the reference's.
    assert_eq!(run(&mut s, "animLayer", json!({})), json!("AnimLayer1"));
    assert_eq!(run(&mut s, "animLayer", json!({})), json!("AnimLayer2"));
}

#[test]
fn additive_and_override_results() {
    let mut s = Session::new();
    layered(&mut s);
    near(at(&mut s, "pCube1.tx", 6.0), 8.0, 1e-9, "base plus layer");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "weight": 0.5}));
    near(at(&mut s, "pCube1.tx", 6.0), 6.5, 1e-9, "half weight");
    assert_eq!(q(&mut s, "animLayer", "L1", "weight"), json!(0.5));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "mute": true}));
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "muted");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "mute": false, "weight": 1.0, "override": true}));
    near(at(&mut s, "pCube1.tx", 6.0), 3.0, 1e-9, "override");
    assert_eq!(q(&mut s, "animLayer", "L1", "override"), json!(true));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "weight": 0.5}));
    near(at(&mut s, "pCube1.tx", 6.0), 4.0, 1e-9, "half override");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "mute": true}));
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "muted override lets the base through");
}

#[test]
fn keys_go_to_the_chosen_layer_and_resolve() {
    let mut s = Session::new();
    layered(&mut s);
    // At frame 6 the base is 5. Asking for 20 through L1 leaves 15 on its curve.
    run(&mut s, "currentTime", json!({"args": [6]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L1", "value": 20.0}));
    near(at(&mut s, "pCube1.tx", 6.0), 20.0, 1e-9, "the channel comes to the value");
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"args": ["pCube1_translateX_L1_inputB"], "query": true, "time": 6, "queried": ["valueChange"]}))), vec![15.0]);
    // -al on a layer the channel isn't in is ignored: the key goes where it would have gone, in Hybrid mode the last
    // active layer holding the channel.
    run(&mut s, "animLayer", json!({"args": ["Other"]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "Other", "time": 21.0, "value": 50.0, "noResolve": true}));
    assert_eq!(floats(&run(&mut s, "keyframe", json!({"args": ["pCube1_translateX_L1_inputB"], "query": true, "time": 21, "queried": ["valueChange"]}))), vec![50.0]);
    assert_eq!(q(&mut s, "animLayer", "pCube1.tx", "bestLayer"), json!("L1"));
    // Selecting the base bar sends keys there.
    run(&mut s, "animLayer", json!({"args": ["BaseAnimation"], "edit": true, "selected": true}));
    assert_eq!(q(&mut s, "animLayer", "pCube1.tx", "bestLayer"), json!("BaseAnimation"));
    run(&mut s, "animLayer", json!({"args": ["BaseAnimation"], "edit": true, "selected": false}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "selected": true}));
    assert_eq!(q(&mut s, "animLayer", "pCube1.tx", "bestLayer"), json!("L1"));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1.tx"], "query": true, "queried": ["name"]})), json!(["pCube1_translateX_L1_inputB"]));
    // A locked layer takes no keys.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "lock": true}));
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L1", "time": 30.0, "value": 1.0})), json!(0));
    // A set on the layered channel holds until the time moves, as on any animated channel.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "lock": false, "selected": false}));
    run(&mut s, "currentTime", json!({"args": [6]}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 77.0]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.tx"]})), json!(77.0));
    near(at(&mut s, "pCube1.tx", 7.0), 6.0 + (20.0 - 5.0) * 0.8 + 3.0 * 0.2 + 0.0, 8.0, "released");
}

#[test]
fn stacked_layers_and_solo() {
    let mut s = Session::new();
    layered(&mut s);
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": false}));
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "attribute": ["pCube1.tx"]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L2", "time": 1, "value": 100.0, "noResolve": true}));
    near(at(&mut s, "pCube1.tx", 6.0), 108.0, 1e-9, "both");
    // L2 sits on L1: the channel's source is L2's blend.
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"], "source": true, "destination": false})), json!(["pCube1_translateX_L2"]));
    // Solo plays only the soloed layer over the base.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "solo": true}));
    near(at(&mut s, "pCube1.tx", 6.0), 8.0, 1e-9, "solo L1");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "solo": false}));
    near(at(&mut s, "pCube1.tx", 6.0), 108.0, 1e-9, "solo off");
    // An override above: replaces what is under it, the base and L1 both.
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "override": true}));
    near(at(&mut s, "pCube1.tx", 6.0), 100.0, 1e-9, "override on top");
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "override": false}));
    // Moving L2 under L1 keeps the sum and swaps the chain.
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "moveLayerBefore": "L1"}));
    assert_eq!(run(&mut s, "animLayer", json!({"query": true, "queried": ["layeredHierarchy"]})), json!(["BaseAnimation", "L2", "L1"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"], "source": true, "destination": false})), json!(["pCube1_translateX_L1"]));
    near(at(&mut s, "pCube1.tx", 6.0), 108.0, 1e-9, "moved");
    // A child layer's weight is under its parent's.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "parent": "L2", "weight": 1.0}));
    assert_eq!(q(&mut s, "animLayer", "L2", "children"), json!(["L1"]));
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "weight": 0.5}));
    near(at(&mut s, "pCube1.tx", 6.0), 5.0 + 50.0 + 1.5, 1e-9, "parent weight");
}

#[test]
fn rotate_scale_and_boolean_channels() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    key(&mut s, "pCube1.ry", 1.0, 10.0);
    run(&mut s, "setAttr", json!({"args": ["pCube1.sx", 2.0]}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": false, "addSelectedObjects": true}));
    // Whole rotate, per-axis scale, as the reference's blend types go.
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["pCube1_rotate_L1"]})), json!("animBlendNodeAdditiveRotation"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["pCube1_scaleX_L1"]})), json!("animBlendNodeAdditiveScale"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["pCube1_visibility_L1"]})), json!("animBlendNodeBoolean"));
    // Nothing moves until there are keys on it.
    near(at(&mut s, "pCube1.ry", 1.0), 10.0, 1e-9, "base rotate");
    near(at(&mut s, "pCube1.sx", 1.0), 2.0, 1e-9, "base scale");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.visibility"]})), json!(true));
    // Rotation adds by axis, scale multiplies.
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.ry"], "animLayer": "L1", "time": 1, "value": 30.0, "noResolve": true}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.sx"], "animLayer": "L1", "time": 1, "value": 3.0, "noResolve": true}));
    near(at(&mut s, "pCube1.ry", 1.0), 40.0, 1e-9, "rotate adds");
    near(at(&mut s, "pCube1.sx", 1.0), 6.0, 1e-9, "scale multiplies");
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTA"})).as_array().unwrap().len(), 2);
    // Half weight scales by the square root of the layer.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "weight": 0.5}));
    near(at(&mut s, "pCube1.sx", 1.0), 2.0 * 3f64.sqrt(), 1e-9, "half scale");
    near(at(&mut s, "pCube1.ry", 1.0), 25.0, 1e-9, "half rotate");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "weight": 1.0, "scaleAccumulationMode": "additive"}));
    near(at(&mut s, "pCube1.sx", 1.0), 5.0, 1e-9, "scale adds: a * wa + b * wb, the node page's formula");
    // By layer, rotations compose: the same axis still adds.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "rotationAccumulationMode": "layer"}));
    near(at(&mut s, "pCube1.ry", 1.0), 40.0, 1e-6, "composed");
    // Whole-rotate moves hold through the blend, and key resolved.
    run(&mut s, "currentTime", json!({"args": [1]}));
    run(&mut s, "rotate", json!({"args": ["pCube1"], "to": [0.0, 90.0, 0.0]}));
    near(run(&mut s, "getAttr", json!({"args": ["pCube1.ry"]})).as_f64().unwrap(), 90.0, 1e-9, "held");
    // Excluding kinds of channel.
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "addSelectedObjects": true, "excludeBoolean": true, "excludeRotate": true, "excludeScale": true}));
    assert_eq!(q(&mut s, "animLayer", "L1", "attribute"), json!(["pCube1.translateX", "pCube1.translateY", "pCube1.translateZ"]));
}

#[test]
fn deleting_and_merging_layers() {
    let mut s = Session::new();
    layered(&mut s);
    // Deleting leaves the base curve driving the channel; the last layer takes BaseAnimation with it.
    run(&mut s, "delete", json!({"args": ["L1"]}));
    assert_eq!(run(&mut s, "ls", json!({"type": "animBlendNodeAdditiveDL"})), json!([]));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!(["pCube1_translateX"]));
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "base alone");
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.tx"], "source": true, "destination": false})), json!(["pCube1_translateX"]));

    // Merging keeps the result on the base curve.
    let mut s = Session::new();
    layered(&mut s);
    key(&mut s, "pCube1.tx", 6.0, 8.0);
    let before: Vec<f64> = (1..=11).map(|f| at(&mut s, "pCube1.tx", f as f64)).collect();
    assert_eq!(run(&mut s, "mergeAnimLayers", json!({"args": ["BaseAnimation", "L1"]})), json!("BaseAnimation"));
    assert_eq!(run(&mut s, "ls", json!({"type": "animBlendNodeAdditiveDL"})), json!([]));
    assert_eq!(run(&mut s, "ls", json!({"type": "animLayer"})), json!([]));
    for (i, want) in before.iter().enumerate() {
        near(at(&mut s, "pCube1.tx", (i + 1) as f64), *want, 1e-3, "merged");
    }
    // Into a layer rather than the base: the base stays, and the sum does too.
    let mut s = Session::new();
    layered(&mut s);
    run(&mut s, "animLayer", json!({"args": ["L2"]}));
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "attribute": ["pCube1.tx"]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L2", "time": 1, "value": 100.0, "noResolve": true}));
    let before = at(&mut s, "pCube1.tx", 6.0);
    assert_eq!(run(&mut s, "mergeAnimLayers", json!({"args": ["L1", "L2"]})), json!("L1"));
    near(at(&mut s, "pCube1.tx", 6.0), before, 1e-3, "merged into a layer");
    near(floats(&run(&mut s, "keyframe", json!({"args": ["pCube1_translateX_L1_inputB"], "query": true, "time": 6, "queried": ["eval"]})))[0], 103.0, 1e-3, "on L1");
    assert_eq!(run(&mut s, "ls", json!({"type": "animLayer"})), json!(["BaseAnimation", "L1"]));

    // Deleting the object takes blends and curves.
    let mut s = Session::new();
    layered(&mut s);
    run(&mut s, "delete", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "ls", json!({"type": "animBlendNodeAdditiveDL"})), json!([]));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!([]));
}

// ---------------------------------------------------------------------------------------------------------------------
// Characters

#[test]
fn character_membership() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "spaceLocator", json!({"name": "ctrl"}));
    let c = run(&mut s, "character", json!({"args": ["pCube1.tx", "pCube1.ry"], "name": "hero"}));
    assert_eq!(c, json!("hero"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["hero"]})), json!("character"));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true})), json!(["pCube1.translateX", "pCube1.rotateY"]));
    run(&mut s, "character", json!({"args": ["ctrl.tz"], "addElement": "hero"}));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true, "queried": ["nodesOnly"]})), json!(["pCube1", "ctrl"]));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true, "queried": ["attributes"]})), json!(["translateX", "rotateY", "translateZ"]));
    assert_eq!(run(&mut s, "character", json!({"args": ["ctrl.tz"], "isMember": "hero"})), json!(true));
    assert_eq!(run(&mut s, "character", json!({"args": ["ctrl.tx"], "isMember": "hero"})), json!(false));
    run(&mut s, "character", json!({"args": ["pCube1.ry"], "remove": "hero"}));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true})), json!(["pCube1.translateX", "ctrl.translateZ"]));
    // Members follow renames, and drop out when deleted.
    run(&mut s, "rename", json!({"args": ["ctrl", "boss"]}));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true})), json!(["pCube1.translateX", "boss.translateZ"]));
    run(&mut s, "delete", json!({"args": ["boss"]}));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true})), json!(["pCube1.translateX"]));
    // A whole object brings its keyable channels, less those excluded.
    run(&mut s, "character", json!({"args": ["pCube1"], "name": "body", "excludeVisibility": true, "excludeScale": true}));
    assert_eq!(run(&mut s, "character", json!({"args": ["body"], "query": true})).as_array().unwrap().len(), 6);
    // Force moves a channel from another character.
    run(&mut s, "character", json!({"args": ["pCube1.tx"], "forceElement": "hero"}));
    assert_eq!(run(&mut s, "character", json!({"args": ["pCube1.tx"], "isMember": "body"})), json!(false));
    run(&mut s, "character", json!({"clear": "hero"}));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true})), json!([]));
    // Keying a character keys its members; the current character takes the place of a selection.
    run(&mut s, "select", json!({"clear": true}));
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["body"], "time": 1})), json!(5));
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["body"], "attribute": "ty", "time": 2})), json!(1));
    run(&mut s, "currentCharacters", json!({"args": ["body"]}));
    assert_eq!(run(&mut s, "currentCharacters", json!({"query": true})), json!(["body"]));
    assert_eq!(run(&mut s, "setKeyframe", json!({"time": 3})), json!(5));
    run(&mut s, "currentCharacters", json!({"clear": true}));
    assert_eq!(run(&mut s, "currentCharacters", json!({"query": true})), json!([]));
}

#[test]
fn clips_play_keys_on_a_schedule() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "character", json!({"args": ["pCube1.tx", "pCube1.ty"], "name": "hero"}));
    for (t, v) in [(1.0, 0.0), (11.0, 10.0)] {
        run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": t, "value": v, "inTangentType": "linear", "outTangentType": "linear"}));
    }
    run(&mut s, "setAttr", json!({"args": ["pCube1.ty", 4.0]}));
    let r = run(&mut s, "clip", json!({"args": ["hero"], "name": "walk", "scheduleClip": true, "startTime": 1.0, "endTime": 11.0}));
    assert_eq!(r, json!(["walkSource", "walk"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "clipScheduler"})), json!(["heroScheduler"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "clipLibrary"})), json!(["heroClips"]));
    assert_eq!(run(&mut s, "character", json!({"args": ["hero"], "query": true, "queried": ["scheduler"]})), json!("heroScheduler"));
    // The keys have moved into the clip; it plays the same.
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!([]));
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "plays");
    near(at(&mut s, "pCube1.ty", 6.0), 4.0, 1e-9, "a channel with no keys holds");
    near(at(&mut s, "pCube1.tx", 30.0), 10.0, 1e-9, "holds after");
    assert_eq!(run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "query": true})), json!(["walk,0,1,1,11,1,0,1,0"]));
    assert_eq!(run(&mut s, "clip", json!({"args": ["hero"], "query": true, "queried": ["allClips"]})), json!(["walk"]));
    assert_eq!(run(&mut s, "clip", json!({"args": ["hero"], "query": true, "queried": ["allSourceClips"]})), json!(["walkSource"]));
    // Moving, scaling, cycling.
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "name": "walk", "start": 11.0}));
    near(at(&mut s, "pCube1.tx", 16.0), 5.0, 1e-9, "moved");
    near(at(&mut s, "pCube1.tx", 5.0), 0.0, 1e-9, "before it");
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "clipIndex": 0, "scale": 2.0}));
    near(at(&mut s, "pCube1.tx", 21.0), 5.0, 1e-9, "twice as long");
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "name": "walk", "scale": 1.0, "cycle": 2.0}));
    near(at(&mut s, "pCube1.tx", 26.0), 5.0, 1e-9, "second cycle");
    assert_eq!(run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "query": true, "name": "walk", "queried": ["cycle"]})), json!(2.0));
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "name": "walk", "cycle": 1.0, "weight": 0.5}));
    near(at(&mut s, "pCube1.tx", 21.0), 5.0, 1e-9, "half weight of the end");
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "name": "walk", "weight": 1.0}));
    // A second instance later on, a higher track wins where they overlap.
    let name = run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "instance": "walkSource", "name": "run", "start": 16.0, "track": 1}));
    assert_eq!(name, json!("run"));
    near(at(&mut s, "pCube1.tx", 18.0), 2.0, 1e-9, "top track");
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "name": "run", "enable": false}));
    near(at(&mut s, "pCube1.tx", 18.0), 7.0, 1e-9, "disabled");
    run(&mut s, "clipSchedule", json!({"args": ["heroScheduler"], "edit": true, "name": "run", "remove": true}));
    assert_eq!(run(&mut s, "clipSchedule", json!({"args": ["hero"], "query": true})).as_array().unwrap().len(), 1);
}

#[test]
fn clips_with_a_range_leave_other_keys_in_play() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "character", json!({"args": ["pCube1.tx"], "name": "hero"}));
    for (t, v) in [(1.0, 0.0), (11.0, 10.0), (21.0, 5.0)] {
        key(&mut s, "pCube1.tx", t, v);
    }
    run(&mut s, "clip", json!({"args": ["hero"], "name": "a", "startTime": 1.0, "endTime": 11.0}));
    // The key at 21 stays on its curve.
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["pCube1_translateX"], "query": true, "queried": ["timeChange"]})), json!([21.0]));
    // A source made without scheduling.
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "ty", "time": 1, "value": 1.0}));
    let r = run(&mut s, "clip", json!({"args": ["hero"], "name": "b", "scheduleClip": false, "startTime": 40.0, "endTime": 50.0}));
    assert_eq!(r, json!(["bSource"]));
    assert_eq!(run(&mut s, "clip", json!({"args": ["hero"], "query": true, "queried": ["allClips"]})), json!(["a"]));
}

#[test]
fn poses_keep_and_restore_values() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "character", json!({"args": ["pCube1.tx", "pCube1.ry"], "name": "hero"}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 3.0]}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.ry", 40.0]}));
    assert_eq!(run(&mut s, "pose", json!({"args": ["hero"], "name": "stand"})), json!("stand"));
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 9.0]}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.ry", -1.0]}));
    assert_eq!(run(&mut s, "pose", json!({"args": ["hero"], "query": true, "queried": ["allPoses"]})), json!(["stand"]));
    run(&mut s, "pose", json!({"args": ["hero"], "apply": true, "name": "stand"}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.tx"]})), json!(3.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.ry"]})), json!(40.0));
    assert!(s.run("pose", json!({"args": ["hero"], "apply": true, "name": "nope"})).is_err());
}

#[test]
fn ghosting_is_kept_on_dag_nodes() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.ghosting"]})), json!(false));
    run(&mut s, "GhostSelected", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.ghosting"]})), json!(true));
    run(&mut s, "UnghostSelected", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.ghosting"]})), json!(false));
    run(&mut s, "GhostSelected", json!({"args": ["pCube1"]}));
    run(&mut s, "UnghostAll", json!({}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pCube1.ghosting"]})), json!(false));
}

#[test]
fn layered_keys_save_and_undo() {
    let mut s = Session::new();
    layered(&mut s);
    near(at(&mut s, "pCube1.tx", 6.0), 8.0, 1e-9, "before");
    s.undo();
    near(at(&mut s, "pCube1.tx", 6.0), 5.0, 1e-9, "the layer's key undone");
    s.redo();
    near(at(&mut s, "pCube1.tx", 6.0), 8.0, 1e-9, "redone");
}
