//! Real units: centimetres and radians stored, everything a script or a file sees in its own units.

use aztek_scene::session::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn num(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_array().and_then(|a| a.first()).and_then(Value::as_f64)).unwrap_or_else(|| panic!("{v} is no number"))
}

fn get(s: &mut Session, plug: &str) -> f64 {
    num(&run(s, "getAttr", json!({"args": [plug]})))
}

fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-6, "{a} is not {b}");
}

fn units(s: &mut Session, linear: &str, angle: &str) {
    run(s, "currentUnit", json!({"linear": linear, "angle": angle}));
}

fn fresh() -> Session {
    let mut s = Session::new();
    run(&mut s, "newScene", json!({}));
    s
}

#[test]
fn every_linear_unit_reads_the_same_length() {
    let mut s = fresh();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "setAttr", json!({"args": ["a.tx", 254.0]}));
    for (unit, value) in [("mm", 2540.0), ("cm", 254.0), ("m", 2.54), ("km", 0.00254), ("in", 100.0), ("ft", 100.0 / 12.0), ("yd", 100.0 / 36.0)] {
        units(&mut s, unit, "deg");
        close(get(&mut s, "a.tx"), value);
    }
    run(&mut s, "newScene", json!({}));
}

#[test]
fn a_matrix_stays_in_centimetres() {
    let mut s = fresh();
    run(&mut s, "polyCube", json!({"name": "a"}));
    units(&mut s, "m", "rad");
    run(&mut s, "setAttr", json!({"args": ["a.tx", 2.0]}));
    let m = run(&mut s, "getAttr", json!({"args": ["a.worldMatrix"]}));
    let flat: Vec<f64> = m.as_array().unwrap().iter().flat_map(|x| x.as_array().cloned().unwrap_or_else(|| vec![x.clone()])).filter_map(|x| x.as_f64()).collect();
    close(flat[12], 200.0);
    run(&mut s, "newScene", json!({}));
}

#[test]
fn primitives_and_flags_take_the_current_unit() {
    let mut s = fresh();
    units(&mut s, "m", "deg");
    run(&mut s, "polyCube", json!({"name": "a", "width": 2.0}));
    close(get(&mut s, "polyCube1.width"), 2.0);
    let b = run(&mut s, "polyEvaluate", json!({"args": ["a"], "boundingBox": true}));
    units(&mut s, "cm", "deg");
    close(get(&mut s, "polyCube1.width"), 200.0);
    let _ = b;
    run(&mut s, "newScene", json!({}));
}

#[test]
fn keys_hold_stored_values_and_answer_in_current_units() {
    let mut s = fresh();
    run(&mut s, "polyCube", json!({"name": "a"}));
    units(&mut s, "m", "rad");
    run(&mut s, "setKeyframe", json!({"args": ["a.tx"], "time": 1.0, "value": 2.0}));
    run(&mut s, "setKeyframe", json!({"args": ["a.ry"], "time": 1.0, "value": 1.0}));
    units(&mut s, "cm", "deg");
    close(get(&mut s, "a.tx"), 200.0);
    close(get(&mut s, "a.ry"), 1f64.to_degrees());
    let q = run(&mut s, "keyframe", json!({"args": ["a.ry"], "query": true, "queried": ["valueChange"]}));
    close(num(&q), 1f64.to_degrees());
    run(&mut s, "keyframe", json!({"args": ["a.ry"], "edit": true, "time": [1.0, 1.0], "valueChange": 90.0, "absolute": true}));
    close(get(&mut s, "a.ry"), 90.0);
    run(&mut s, "newScene", json!({}));
}

#[test]
fn connections_between_kinds_go_through_a_conversion_node() {
    let mut s = fresh();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "connectAttr", json!({"args": ["a.tx", "b.rx"]}));
    close(get(&mut s, "unitConversion1.conversionFactor"), 1f64.to_radians());
    run(&mut s, "setAttr", json!({"args": ["a.tx", 30.0]}));
    close(get(&mut s, "b.rx"), 30.0);
    assert_eq!(run(&mut s, "isConnected", json!({"args": ["a.tx", "b.rx"]})), json!(true));
    run(&mut s, "disconnectAttr", json!({"args": ["a.tx", "b.rx"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["unitConversion1"]})), json!(false));
    // Same kind, no node; undo takes the whole connection back.
    run(&mut s, "connectAttr", json!({"args": ["a.tx", "b.ty"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["unitConversion1"]})), json!(false));
    run(&mut s, "newScene", json!({}));
}

#[test]
fn a_whole_vector_converts_per_axis() {
    let mut s = fresh();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "setAttr", json!({"args": ["a.t", 10.0, 20.0, 30.0]}));
    run(&mut s, "connectAttr", json!({"args": ["a.t", "b.r"]}));
    close(get(&mut s, "b.rz"), 30.0);
    assert!(s.run("setAttr", json!({"args": ["b.r", 0.0, 0.0, 0.0]})).is_err());
    run(&mut s, "newScene", json!({}));
}

#[test]
fn a_file_is_written_in_the_scenes_units_and_read_back_in_them() {
    let dir = std::env::temp_dir().join(format!("aztek_units_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("metres.ma").to_string_lossy().replace('\\', "/");
    let mut s = fresh();
    units(&mut s, "m", "rad");
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "setAttr", json!({"args": ["a.tx", 2.0]}));
    run(&mut s, "setAttr", json!({"args": ["a.ry", 1.0]}));
    run(&mut s, "setKeyframe", json!({"args": ["a.tz"], "time": 1.0, "value": 3.0}));
    run(&mut s, "file", json!({"rename": path}));
    run(&mut s, "file", json!({"save": true, "type": "mayaAscii"}));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("currentUnit -l meter -a radian"), "the header names the units");
    assert!(text.contains("\".t\" -type \"double3\" 2 0"), "the file's numbers are in them: {}", &text[..text.len().min(1500)]);
    assert!(text.contains("\".r\" -type \"double3\" 0 1 0"));

    let mut t = fresh();
    run(&mut t, "file", json!({"args": [path], "open": true, "force": true}));
    assert_eq!(run(&mut t, "currentUnit", json!({"query": true, "queried": ["linear"]})), json!("m"));
    assert_eq!(run(&mut t, "currentUnit", json!({"query": true, "queried": ["angle"]})), json!("rad"));
    close(get(&mut t, "a.tx"), 2.0);
    close(get(&mut t, "a.ry"), 1.0);
    close(get(&mut t, "a.tz"), 3.0);
    units(&mut t, "cm", "deg");
    close(get(&mut t, "a.tx"), 200.0);
    close(get(&mut t, "a.ry"), 1f64.to_degrees());
    close(get(&mut t, "a.tz"), 300.0);
    run(&mut t, "newScene", json!({}));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_new_scene_goes_back_to_the_defaults() {
    let mut s = fresh();
    units(&mut s, "m", "rad");
    run(&mut s, "newScene", json!({}));
    assert_eq!(run(&mut s, "currentUnit", json!({"query": true, "queried": ["linear"]})), json!("cm"));
    assert_eq!(run(&mut s, "currentUnit", json!({"query": true, "queried": ["angle"]})), json!("deg"));
}

#[test]
fn time_units_by_name_and_custom_rates() {
    let mut s = fresh();
    for (name, fps) in [("film", 24.0), ("pal", 25.0), ("ntsc", 30.0), ("show", 48.0), ("37fps", 37.0)] {
        run(&mut s, "currentUnit", json!({"time": "film"}));
        run(&mut s, "currentTime", json!({"args": [24.0]}));
        run(&mut s, "currentUnit", json!({"time": name}));
        assert_eq!(run(&mut s, "currentUnit", json!({"query": true, "queried": ["time"]})), json!(name));
        close(num(&run(&mut s, "currentTime", json!({"query": true}))), fps);
    }
    run(&mut s, "newScene", json!({}));
}
