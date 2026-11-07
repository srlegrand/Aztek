//! blendTwoAttr, ghosting and timeCode.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn tx(s: &mut Session) -> f64 {
    run(s, "getAttr", json!({"args": ["box.tx"]})).as_f64().unwrap()
}

#[test]
fn blend_two_attr_blends_over_a_range() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "src0"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "src1"}));
    run(&mut s, "setAttr", json!({"args": ["src0.tx", 10.0]}));
    run(&mut s, "setAttr", json!({"args": ["src1.tx", 20.0]}));
    let made = run(&mut s, "blendTwoAttr", json!({"args": ["box"], "attribute": "tx", "attribute0": "src0.tx", "attribute1": "src1.tx", "time": "1:11"}));
    let node = made[0].as_str().unwrap().to_string();
    assert_eq!(run(&mut s, "objectType", json!({"args": [node]})), json!("blendTwoAttr"));
    run(&mut s, "currentTime", json!({"args": [1]}));
    assert!((tx(&mut s) - 10.0).abs() < 1e-6);
    run(&mut s, "currentTime", json!({"args": [11]}));
    assert!((tx(&mut s) - 20.0).abs() < 1e-6);
    run(&mut s, "currentTime", json!({"args": [6]}));
    assert!((tx(&mut s) - 15.0).abs() < 1e-6, "{}", tx(&mut s));
    assert_eq!(run(&mut s, "blendTwoAttr", json!({"args": [node], "query": true, "queried": ["attribute0"]})), json!("src0.translateX"));
}

#[test]
fn blend_two_attr_keeps_the_old_driver_and_takes_a_blender() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "old"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "ctl"}));
    run(&mut s, "connectAttr", json!({"args": ["old.ty", "box.tx"]}));
    run(&mut s, "setAttr", json!({"args": ["old.ty", 4.0]}));
    let made = run(&mut s, "blendTwoAttr", json!({"args": ["box.tx"], "blender": "ctl.sx", "name": "mix"}));
    assert_eq!(made, json!(["mix"]));
    assert_eq!(run(&mut s, "blendTwoAttr", json!({"args": ["mix"], "query": true, "queried": ["attribute0"]})), json!("old.translateY"));
    assert_eq!(run(&mut s, "blendTwoAttr", json!({"args": ["mix"], "query": true, "queried": ["blender"]})), json!("ctl.scaleX"));
    run(&mut s, "setAttr", json!({"args": ["ctl.sx", 0.0]}));
    assert!((tx(&mut s) - 4.0).abs() < 1e-6);
    run(&mut s, "blendTwoAttr", json!({"args": ["mix"], "edit": true, "driver": 1}));
    assert_eq!(run(&mut s, "blendTwoAttr", json!({"args": ["mix"], "query": true, "queried": ["driver"]})), json!(1));
}

#[test]
fn ghosting_sets_attributes_and_answers_frames() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "select", json!({"args": ["box"]}));
    assert_eq!(run(&mut s, "ghosting", json!({"query": true, "queried": ["mode"]})), json!("preAndPost"));
    run(&mut s, "ghosting", json!({"preFrames": 2, "postFrames": 1, "ghostsStep": 2}));
    assert_eq!(run(&mut s, "ghosting", json!({"action": "ghost"})), json!("box"));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["box.ghosting"]})), json!(true));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["box.ghostPreSteps"]})), json!(2));
    assert_eq!(run(&mut s, "ghosting", json!({"query": true, "ghostedObjects": true})), json!(["box"]));
    run(&mut s, "currentTime", json!({"args": [10]}));
    assert_eq!(run(&mut s, "ghosting", json!({"query": true, "frames": true})), json!([6.0, 8.0, 12.0]));
    run(&mut s, "ghosting", json!({"mode": "custom"}));
    run(&mut s, "ghosting", json!({"customFrames": [1, 2, 50]}));
    assert_eq!(run(&mut s, "ghosting", json!({"query": true, "frames": true})), json!([1.0, 2.0, 50.0]));
    run(&mut s, "ghosting", json!({"customFrames": -9999999}));
    assert_eq!(run(&mut s, "ghosting", json!({"query": true, "queried": ["customFrames"]})), json!([]));
    run(&mut s, "ghosting", json!({"action": "unghostAll"}));
    assert_eq!(run(&mut s, "ghosting", json!({"query": true, "ghostedObjects": true})), json!([]));
    assert!(s.run("ghosting", json!({"action": "haunt"})).is_err());
}

#[test]
fn time_code_reads_back() {
    let mut s = Session::new();
    run(&mut s, "timeCode", json!({"productionStartHour": 1.0, "productionStartMinute": 33.0, "productionStartSecond": 52.0, "productionStartFrame": 23.0}));
    assert_eq!(run(&mut s, "timeCode", json!({"query": true, "queried": ["productionStartHour"]})), json!(1.0));
    assert_eq!(run(&mut s, "timeCode", json!({"query": true, "queried": ["productionStartMinute"]})), json!(33.0));
    assert_eq!(run(&mut s, "timeCode", json!({"query": true, "queried": ["mayaStartFrame"]})), json!(0.0));
}
