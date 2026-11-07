//! Characters: skeleton, definition, control rig, retargeting.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pos(s: &mut Session, j: &str) -> [f64; 3] {
    let v = run(s, "xform", json!({"args": [j], "query": true, "worldSpace": true, "translation": true}));
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

#[test]
fn skeleton_and_definition() {
    let mut s = Session::new();
    let j = run(&mut s, "hikCreateSkeleton", json!({"name": "Hero", "height": 170.0, "character": true}));
    assert!(j.as_array().unwrap().len() > 20);
    assert!((pos(&mut s, "Hero_Hips")[1] - 95.0).abs() < 1e-6);
    assert!((pos(&mut s, "Hero_LeftHand")[0] - 70.0).abs() < 1e-6);
    assert!((pos(&mut s, "Hero_RightHand")[0] + 70.0).abs() < 1e-6);
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Hero", 1]})), json!("Hero_Hips"));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Hero", "LeftArm"]})), json!("Hero_LeftArm"));
}

#[test]
fn names_map_to_slots() {
    use aztek_scene::hik::slot_for_name as f;
    for (name, want) in [
        ("mixamorig:LeftArm", "LeftArm"),
        ("Character1_RightFoot", "RightFoot"),
        ("L_UpperArm", "LeftArm"),
        ("r_thigh", "RightUpLeg"),
        ("pelvis", "Hips"),
        ("spine_02", "Spine2"),
        ("Right_Shoulder", "RightShoulder"),
    ] {
        assert_eq!(f(name).as_deref(), Some(want), "{name}");
    }
    assert_eq!(f("prop_sword"), None);
}

#[test]
fn definition_from_a_named_skeleton() {
    let mut s = Session::new();
    run(&mut s, "joint", json!({"position": [0, 10, 0], "name": "pelvis"}));
    run(&mut s, "joint", json!({"position": [0, 12, 0], "name": "spine_01"}));
    run(&mut s, "hikCreateCharacter", json!({"args": ["Bob", "pelvis"]}));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Bob", "Hips"]})), json!("pelvis"));
    run(&mut s, "joint", json!({"position": [0, 14, 0], "name": "oddName"}));
    run(&mut s, "setCharacterObject", json!({"args": ["oddName", "Bob", 15, 0]}));
    assert_eq!(run(&mut s, "hikGetSkNode", json!({"args": ["Bob", "Head"]})), json!("oddName"));
}

#[test]
fn control_rig_drives_the_skeleton() {
    let mut s = Session::new();
    run(&mut s, "hikCreateSkeleton", json!({"name": "Hero", "character": true}));
    let rest = pos(&mut s, "Hero_LeftForeArm");
    let made = run(&mut s, "hikCreateControlRig", json!({"args": ["Hero"]}));
    assert!(made.as_array().unwrap().iter().any(|x| x == "Hero_Ctrl_LeftHandEffector"));
    let near = |a: [f64; 3], b: [f64; 3], t: f64| (0..3).all(|i| (a[i] - b[i]).abs() < t);
    // Building it moves nothing.
    assert!(near(pos(&mut s, "Hero_LeftForeArm"), rest, 1e-3), "{:?} {rest:?}", pos(&mut s, "Hero_LeftForeArm"));
    // The hand effector pulls the hand.
    let h = pos(&mut s, "Hero_LeftHand");
    run(&mut s, "move", json!({"args": ["Hero_Ctrl_LeftHandEffector"], "to": [h[0] - 15.0, h[1] + 15.0, h[2]]}));
    let p = pos(&mut s, "Hero_LeftHand");
    assert!(near(p, [h[0] - 15.0, h[1] + 15.0, h[2]], 1e-2), "{p:?}");
    // The hips control moves the hips, and the feet stay.
    let foot = pos(&mut s, "Hero_LeftFoot");
    run(&mut s, "move", json!({"args": ["Hero_Ctrl_Hips"], "by": [0, -10, 0]}));
    assert!((pos(&mut s, "Hero_Hips")[1] - 85.0).abs() < 1e-2, "{:?}", pos(&mut s, "Hero_Hips"));
    assert!(near(pos(&mut s, "Hero_LeftFoot"), foot, 1e-2));
    run(&mut s, "hikDeleteControlRig", json!({"args": ["Hero"]}));
    assert_eq!(run(&mut s, "ls", json!({"args": ["Hero_Ctrl_*"]})), json!([]));
}

#[test]
fn retarget_and_bake() {
    let mut s = Session::new();
    run(&mut s, "hikCreateSkeleton", json!({"name": "Src", "character": true}));
    run(&mut s, "hikCreateSkeleton", json!({"name": "Dst", "character": true}));
    assert!(run(&mut s, "hikSetSource", json!({"args": ["Dst", "Src"]})).as_i64().unwrap() > 10);
    run(&mut s, "setKeyframe", json!({"args": ["Src_LeftArm.rz"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["Src_LeftArm.rz"], "time": 10, "value": 60.0}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    let (a, b) = (pos(&mut s, "Src_LeftHand"), pos(&mut s, "Dst_LeftHand"));
    assert!((a[1] - b[1]).abs() < 1e-3, "{a:?} {b:?}");
    assert!(b[1] > 150.0, "{b:?}");
    run(&mut s, "hikBake", json!({"args": ["Dst"], "time": "1:10"}));
    run(&mut s, "cutKey", json!({"args": ["Src_LeftArm.rz"], "clear": true}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    assert!(pos(&mut s, "Dst_LeftHand")[1] > 150.0, "{:?}", pos(&mut s, "Dst_LeftHand"));
}
