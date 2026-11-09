//! Grox (`examples/characters/grox.ma`): a horned brute with a production-style rig, built by `examples/tools/grox`.
//! The file opens without warnings, and the controls an animator reaches for first drive the skeleton and the skin.

use aztek_scene::ma_file::open_scene;
use aztek_scene::Session;
use serde_json::{json, Value};
use std::path::PathBuf;

fn path() -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/characters/grox.ma").to_string_lossy().to_string()
}

fn open() -> Session {
    let mut s = Session::new();
    let w = open_scene(&mut s, &path()).expect("grox_rig.ma opens");
    assert!(w.is_empty(), "warnings: {w:?}");
    s.graph.evaluate();
    s
}

fn run(s: &mut Session, cmd: &str, a: Value) -> Value {
    s.run(cmd, a.clone()).unwrap_or_else(|e| panic!("{cmd} {a}: {e}"))
}

fn nums(v: Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.into_iter().flat_map(|x| nums(x)).collect(),
        v => vec![v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))],
    }
}

fn set(s: &mut Session, plug: &str, v: f64) {
    run(s, "setAttr", json!({"args": [plug, v]}));
}

fn set_enum(s: &mut Session, plug: &str, v: i64) {
    run(s, "setAttr", json!({"args": [plug, v]}));
}

fn get(s: &mut Session, plug: &str) -> Vec<f64> {
    nums(run(s, "getAttr", json!({"args": [plug]})))
}

fn ws(s: &mut Session, node: &str) -> Vec<f64> {
    nums(run(s, "xform", json!({"args": [node], "query": true, "worldSpace": true, "translation": true})))
}

fn point(s: &mut Session, comp: &str) -> Vec<f64> {
    nums(run(s, "pointPosition", json!({"args": [comp]})))
}

fn move_by(s: &mut Session, node: &str, d: [f64; 3]) {
    run(s, "move", json!({"args": [d[0], d[1], d[2], node], "relative": true}));
}

fn dist(a: &[f64], b: &[f64]) -> f64 {
    a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum::<f64>().sqrt()
}

fn near(a: &[f64], b: &[f64], tol: f64, what: &str) {
    assert!(dist(a, b) < tol, "{what}: got {a:?}, want {b:?}");
}

// A vertex on the left palm, one on the left horn tip, one at the tail tip.
const PALM: &str = "grox_body_GEO.vtx[28407]";
const HORN_TIP: &str = "grox_body_GEO.vtx[24454]";
const TAIL_TIP: &str = "grox_body_GEO.vtx[14807]";

#[test]
fn opens_with_its_rig() {
    let mut s = open();
    let joints = run(&mut s, "ls", json!({"type": "joint"}));
    assert_eq!(joints.as_array().unwrap().len(), 134);
    let inf = run(&mut s, "skinCluster", json!({"args": ["grox_body_SKN"], "query": true, "influence": true}));
    assert_eq!(inf.as_array().unwrap().len(), 81);
    let ik = run(&mut s, "ls", json!({"type": "ikHandle"}));
    assert_eq!(ik.as_array().unwrap().len(), 8);
    near(&ws(&mut s, "L_wrist_JNT"), &[98.0, 104.0, 4.0], 1e-3, "left wrist at rest");
    near(&point(&mut s, PALM), &[100.372, 91.55, 3.671], 1e-2, "palm vertex at rest");
}

#[test]
fn ik_arm_reaches_and_stretches() {
    let mut s = open();
    set(&mut s, "L_armSettings_CTRL.ikFk", 1.0);
    move_by(&mut s, "L_armIK_CTRL", [-30.0, 60.0, 50.0]);
    near(&ws(&mut s, "L_wrist_JNT"), &[68.0, 164.0, 54.0], 1e-2, "wrist on the IK control");
    // the skin comes along: the palm vertex keeps close to the wrist
    let palm = point(&mut s, PALM);
    assert!(dist(&palm, &[68.0, 164.0, 54.0]) < 16.0, "palm vertex followed the hand: {palm:?}");
    // past the arm's reach the chain stretches to the control, and stops short with stretch off
    move_by(&mut s, "L_armIK_CTRL", [200.0, 0.0, 0.0]);
    near(&ws(&mut s, "L_wrist_JNT"), &[268.0, 164.0, 54.0], 1e-2, "stretched");
    set(&mut s, "L_armIK_CTRL.stretch", 0.0);
    let w = ws(&mut s, "L_wrist_JNT");
    let reach = dist(&w, &ws(&mut s, "L_shoulder_JNT"));
    assert!((reach - 107.98).abs() < 0.5, "unstretched reach {reach}, wrist {w:?}");
}

#[test]
fn fk_arm_turns_and_leaves_the_ik_chain_alone() {
    let mut s = open();
    set(&mut s, "L_shoulderFK_CTRL.rz", -40.0);
    let w = ws(&mut s, "L_wrist_JNT");
    assert!(dist(&w, &[98.0, 104.0, 4.0]) > 40.0, "wrist moved with the FK shoulder: {w:?}");
    near(&ws(&mut s, "L_wrist_IK_JNT"), &[98.0, 104.0, 4.0], 1e-3, "IK chain stays");
}

#[test]
fn foot_rolls_on_heel_ball_and_toe() {
    let mut s = open();
    let ankle0 = ws(&mut s, "L_ankle_JNT");
    let toe0 = ws(&mut s, "L_toe_JNT");
    set(&mut s, "L_legIK_CTRL.roll", -20.0);
    assert!(ws(&mut s, "L_toe_JNT")[1] > toe0[1] + 10.0, "heel roll lifts the toes");
    set(&mut s, "L_legIK_CTRL.roll", 20.0);
    assert!(ws(&mut s, "L_ankle_JNT")[1] > ankle0[1] + 4.0, "ball roll lifts the ankle");
    near(&ws(&mut s, "L_toe_JNT"), &toe0, 1e-2, "toes stay planted on a ball roll");
    set(&mut s, "L_legIK_CTRL.roll", 60.0);
    let a = ws(&mut s, "L_ankle_JNT");
    assert!(a[1] > ankle0[1] + 15.0, "past the break the foot goes up on its toes: {a:?}");
}

#[test]
fn head_holds_its_world_rotation_when_asked() {
    let mut s = open();
    let before = get(&mut s, "C_head_JNT.worldMatrix");
    set_enum(&mut s, "C_head_CTRL.follow", 2);
    set(&mut s, "C_chest_CTRL.rx", 30.0);
    let after = get(&mut s, "C_head_JNT.worldMatrix");
    near(&after[..11], &before[..11], 1e-3, "world follow keeps the rotation");
    set_enum(&mut s, "C_head_CTRL.follow", 0);
    let neck = get(&mut s, "C_head_JNT.worldMatrix");
    assert!(dist(&neck[..11], &before[..11]) > 0.2, "neck follow turns with the chest");
}

#[test]
fn face_hands_and_horns() {
    let mut s = open();
    set(&mut s, "C_head_CTRL.blink", 1.0);
    assert!((get(&mut s, "L_lidUpper_JNT.rz")[0] + 38.0).abs() < 1e-6);
    assert!((get(&mut s, "L_lidLower_JNT.rz")[0] - 38.0).abs() < 1e-6);
    let jaw0 = ws(&mut s, "C_jawEnd_JNT");
    set(&mut s, "C_jaw_CTRL.rz", -20.0);
    assert!(ws(&mut s, "C_jawEnd_JNT")[1] < jaw0[1] - 5.0, "jaw opens down");
    set(&mut s, "L_hand_CTRL.curl", 10.0);
    assert!((get(&mut s, "L_clawA02_DRV.rz")[0] + 90.0).abs() < 1e-6, "curl drives the claws");
    let horn0 = point(&mut s, HORN_TIP);
    set(&mut s, "L_horn03_CTRL.rz", 20.0);
    assert!(dist(&point(&mut s, HORN_TIP), &horn0) > 2.0, "horn tip follows its controls");
}

#[test]
fn tail_and_global_scale_carry_the_skin() {
    let mut s = open();
    let tip0 = point(&mut s, TAIL_TIP);
    for i in 1..=8 {
        set(&mut s, &format!("C_tail{i:02}_CTRL.ry"), 10.0);
    }
    let tip = point(&mut s, TAIL_TIP);
    assert!(dist(&tip, &tip0) > 60.0, "tail tip swings: {tip:?}");
    let mut s = open();
    for a in ["sx", "sy", "sz"] {
        set(&mut s, &format!("C_global_CTRL.{a}"), 2.0);
    }
    near(&ws(&mut s, "L_wrist_JNT"), &[196.0, 208.0, 8.0], 1e-2, "wrist at twice the size");
    near(&point(&mut s, TAIL_TIP), &[tip0[0] * 2.0, tip0[1] * 2.0, tip0[2] * 2.0], 0.05, "skin at twice the size");
}

#[test]
fn proxy_switch() {
    let mut s = open();
    set_enum(&mut s, "C_global_CTRL.geometry", 1);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["grox_body_GEO.visibility"]})), json!(false));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["PROXY_GRP.visibility"]})), json!(true));
}
