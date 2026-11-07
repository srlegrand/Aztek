//! Animation layers: blend tables worked out from the node pages' formulas, solo and mute through the hierarchy, keying by
//! mode, merges, and the layout of a layered scene in .ma as the reference saves it.

use aztek_scene::anim_layer_nodes::{blend_rotation, blend_scalar, BlendKind};
use aztek_scene::transform::RotateOrder;
use aztek_scene::Session;
use glam::DVec3;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn near(a: f64, b: f64, tol: f64, what: &str) {
    assert!((a - b).abs() <= tol, "{what}: {a} is not {b}");
}

fn at(s: &mut Session, plug: &str, frame: f64) -> f64 {
    run(s, "currentTime", json!({"args": [frame]}));
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap()
}

fn cube(s: &mut Session) {
    run(s, "polyCube", json!({}));
    run(s, "select", json!({"args": ["pCube1"]}));
}

fn key_raw(s: &mut Session, plug: &str, layer: &str, t: f64, v: f64) {
    run(s, "setKeyframe", json!({"args": [plug], "animLayer": layer, "time": t, "value": v, "noResolve": true}));
}

fn layer_on(s: &mut Session, name: &str, plug: &str) {
    run(s, "animLayer", json!({"args": [name]}));
    run(s, "animLayer", json!({"args": [name], "edit": true, "attribute": [plug]}));
}

/// tx at 10 on the base (one key), L1 holding 4.
fn one_layer(s: &mut Session) {
    cube(s);
    run(s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 1, "value": 10.0}));
    layer_on(s, "L1", "pCube1.tx");
    key_raw(s, "pCube1.tx", "L1", 1.0, 4.0);
}

#[test]
fn scalar_blend_table() {
    // output = a * wa + b * wb
    for (a, b, wa, wb, want) in [(10.0, 4.0, 1.0, 1.0, 14.0), (10.0, 4.0, 1.0, 0.5, 12.0), (10.0, 4.0, 0.5, 0.5, 7.0), (10.0, 4.0, 0.0, 1.0, 4.0), (10.0, 4.0, 1.0, 0.0, 10.0)] {
        near(blend_scalar(BlendKind::Add, a, b, wa, wb, 0), want, 1e-12, "additive");
    }
    // Multiply: power(a, wa) * power(b, wb)
    near(blend_scalar(BlendKind::Scale, 2.0, 3.0, 1.0, 1.0, 1), 6.0, 1e-12, "multiply");
    near(blend_scalar(BlendKind::Scale, 2.0, 9.0, 1.0, 0.5, 1), 6.0, 1e-12, "multiply half");
    near(blend_scalar(BlendKind::Scale, 4.0, 9.0, 0.5, 0.5, 1), 6.0, 1e-12, "override multiply half");
    near(blend_scalar(BlendKind::Scale, -2.0, 3.0, 1.0, 1.0, 1), -6.0, 1e-12, "a negative scale keeps its sign");
    // Additive scale: a * wa + b * wb
    near(blend_scalar(BlendKind::Scale, 2.0, 3.0, 1.0, 1.0, 0), 5.0, 1e-12, "scale added");
    near(blend_scalar(BlendKind::Scale, 2.0, 3.0, 1.0, 0.5, 0), 3.5, 1e-12, "scale added half");
    // Boolean and enum: b unless the weight is 0, at any weight above it.
    near(blend_scalar(BlendKind::Switch, 0.0, 1.0, 1.0, 0.1, 0), 1.0, 1e-12, "a low weight still switches");
    near(blend_scalar(BlendKind::Switch, 0.0, 1.0, 1.0, 0.0, 0), 0.0, 1e-12, "no weight");
}

#[test]
fn rotation_blend_table() {
    let xyz = RotateOrder::Xyz;
    let v = |x, y, z| DVec3::new(x, y, z);
    // By component: the sum of Euler values, axes kept apart.
    let r = blend_rotation(v(30.0, 0.0, 0.0), v(0.0, 40.0, 0.0), 1.0, 0.5, 0, xyz);
    near(r.x, 30.0, 1e-9, "x");
    near(r.y, 20.0, 1e-9, "y");
    // By layer, one axis: the same.
    let r = blend_rotation(v(0.0, 10.0, 0.0), v(0.0, 40.0, 0.0), 1.0, 0.5, 1, xyz);
    near(r.y, 30.0, 1e-6, "by layer one axis");
    // By layer across axes differs from the sum: 90 about X then 90 about Y is no (90, 90, 0).
    let r = blend_rotation(v(90.0, 0.0, 0.0), v(0.0, 90.0, 0.0), 1.0, 1.0, 1, xyz);
    let sum = v(90.0, 90.0, 0.0);
    assert!((r - sum).length() > 1.0, "{r:?}");
    // Override by layer is a slerp, halfway along one axis here.
    let r = blend_rotation(v(0.0, 0.0, 20.0), v(0.0, 0.0, 60.0), 0.5, 0.5, 1, xyz);
    near(r.z, 40.0, 1e-6, "slerp");
    // Opaque: nothing of input A.
    let r = blend_rotation(v(0.0, 0.0, 20.0), v(0.0, 0.0, 60.0), 0.0, 1.0, 1, xyz);
    near(r.z, 60.0, 1e-6, "opaque");
}

#[test]
fn layer_modes_on_a_channel() {
    let mut s = Session::new();
    one_layer(&mut s);
    near(at(&mut s, "pCube1.tx", 1.0), 14.0, 1e-9, "additive");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "weight": 0.5}));
    near(at(&mut s, "pCube1.tx", 1.0), 12.0, 1e-9, "additive half");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "override": true}));
    near(at(&mut s, "pCube1.tx", 1.0), 7.0, 1e-9, "override with passthrough, half");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "passthrough": false}));
    near(at(&mut s, "pCube1.tx", 1.0), 2.0, 1e-9, "without passthrough nothing below shows");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "weight": 1.0}));
    near(at(&mut s, "pCube1.tx", 1.0), 4.0, 1e-9, "opaque");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "mute": true}));
    near(at(&mut s, "pCube1.tx", 1.0), 10.0, 1e-9, "a muted override layer lets the base through");
}

#[test]
fn weight_keys_play_and_key_resolution_reads_them() {
    let mut s = Session::new();
    one_layer(&mut s);
    run(&mut s, "setKeyframe", json!({"args": ["L1.weight"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["L1.weight"], "time": 11, "value": 1.0}));
    near(at(&mut s, "pCube1.tx", 1.0), 10.0, 1e-9, "no weight yet");
    near(at(&mut s, "pCube1.tx", 11.0), 14.0, 1e-9, "full weight");
    // A key at frame 11, asked for from frame 1, solves with frame 11's weight.
    run(&mut s, "currentTime", json!({"args": [1]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L1", "time": 11, "value": 30.0}));
    near(at(&mut s, "pCube1.tx", 11.0), 30.0, 1e-9, "solved at that frame's weight");
}

#[test]
fn solo_and_mute_through_the_hierarchy() {
    let mut s = Session::new();
    one_layer(&mut s);
    layer_on(&mut s, "L2", "pCube1.tx");
    key_raw(&mut s, "pCube1.tx", "L2", 1.0, 100.0);
    layer_on(&mut s, "Kid", "pCube1.tx");
    key_raw(&mut s, "pCube1.tx", "Kid", 1.0, 1000.0);
    run(&mut s, "animLayer", json!({"args": ["Kid"], "edit": true, "parent": "L2"}));
    near(at(&mut s, "pCube1.tx", 1.0), 1114.0, 1e-9, "all");
    // A parent's mute takes its children.
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "mute": true}));
    near(at(&mut s, "pCube1.tx", 1.0), 14.0, 1e-9, "parent muted");
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "mute": false}));
    // Solo is at its own level: L1 soloed mutes L2, and L2's child with it.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "solo": true}));
    near(at(&mut s, "pCube1.tx", 1.0), 14.0, 1e-9, "L1 soloed");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "solo": false}));
    // Two soloed layers both play.
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "solo": true}));
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "solo": true}));
    near(at(&mut s, "pCube1.tx", 1.0), 1114.0, 1e-9, "two soloed");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["BaseAnimation.childsoloed"]})), json!(true));
    // A soloed layer's sibling can't be keyed.
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "solo": false}));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["pCube1.tx"], "query": true, "queried": ["bestLayer"]})), json!("L1"));
}

#[test]
fn key_modes() {
    let mut s = Session::new();
    one_layer(&mut s);
    layer_on(&mut s, "L2", "pCube1.tx");
    let best = |s: &mut Session| run(s, "animLayer", json!({"args": ["pCube1.tx"], "query": true, "queried": ["bestLayer"]}));
    assert_eq!(best(&mut s), json!("L2"), "Hybrid with nothing selected: the last active layer");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "selected": true}));
    assert_eq!(best(&mut s), json!("L1"), "Hybrid: the selected layer");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "lock": true}));
    assert_eq!(best(&mut s), json!("L2"), "Hybrid: a locked selection falls to the last active");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "lock": false, "selected": false}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "preferred": true}));
    assert_eq!(best(&mut s), json!("L1"), "preferred comes first");
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "preferred": false}));
    run(&mut s, "animLayerOptions", json!({"keyMode": "selected"}));
    let n = run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 50, "value": 1.0}));
    assert_eq!(n, json!(0), "Selected mode with nothing selected keys nothing");
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "selected": true}));
    assert_eq!(run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 50, "value": 1.0})), json!(1));
    run(&mut s, "animLayerOptions", json!({"keyMode": "hybrid"}));
}

#[test]
fn identity_key_nullifies_the_layer() {
    let mut s = Session::new();
    one_layer(&mut s);
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L1", "time": 5, "identity": true}));
    near(at(&mut s, "pCube1.tx", 5.0), 10.0, 1e-9, "additive: back to the base");
    let mut s = Session::new();
    one_layer(&mut s);
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "override": true}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "animLayer": "L1", "time": 5, "identity": true}));
    near(at(&mut s, "pCube1.tx", 5.0), 10.0, 1e-9, "override: holds what is below");
}

#[test]
fn keying_a_rotate_by_layer_resolves_the_whole_rotation() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "setAttr", json!({"args": ["pCube1.rx", 90.0]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.rx", "pCube1.ry", "pCube1.rz"], "time": 1}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": false, "rotationAccumulationMode": "layer"}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "edit": true, "attribute": ["pCube1.rotate"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["pCube1_rotate_L1"]})), json!("animBlendNodeAdditiveRotation"));
    run(&mut s, "currentTime", json!({"args": [1]}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.ry", 30.0]}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.rx", "pCube1.ry", "pCube1.rz"], "animLayer": "L1", "time": 1}));
    near(at(&mut s, "pCube1.rx", 2.0), 90.0, 1e-6, "rx after a time change");
    near(at(&mut s, "pCube1.ry", 2.0), 30.0, 1e-6, "ry after a time change");
    assert!(run(&mut s, "ls", json!({"type": "animCurveTA"})).as_array().unwrap().iter().any(|x| x == "pCube1_rotate_L1_inputBY"));
}

#[test]
fn merge_options() {
    // Automatic mode: an override among the merged makes an override result, and the result stays.
    let mut s = Session::new();
    one_layer(&mut s);
    layer_on(&mut s, "L2", "pCube1.tx");
    key_raw(&mut s, "pCube1.tx", "L2", 1.0, 2.0);
    key_raw(&mut s, "pCube1.tx", "L2", 5.0, 6.0);
    run(&mut s, "animLayer", json!({"args": ["L2"], "edit": true, "override": true, "weight": 0.5}));
    let want = at(&mut s, "pCube1.tx", 3.0);
    assert_eq!(run(&mut s, "animLayerMerge", json!({"args": ["L1", "L2"]})), json!("L1"));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["override"]})), json!(true));
    near(at(&mut s, "pCube1.tx", 3.0), want, 1e-2, "kept");
    assert_eq!(run(&mut s, "ls", json!({"type": "animLayer"})), json!(["BaseAnimation", "L1"]));

    // To the top, keeping the emptied layer, forced additive.
    let mut s = Session::new();
    one_layer(&mut s);
    layer_on(&mut s, "L2", "pCube1.tx");
    key_raw(&mut s, "pCube1.tx", "L2", 1.0, 2.0);
    let want = at(&mut s, "pCube1.tx", 1.0);
    assert_eq!(run(&mut s, "animLayerMerge", json!({"args": ["L1", "L2"], "mergeTo": "top", "deleteBaked": false, "resultLayerMode": "additive"})), json!("L2"));
    near(at(&mut s, "pCube1.tx", 1.0), want, 1e-6, "kept");
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["attribute"]})), json!([]));
    assert_eq!(run(&mut s, "ls", json!({"type": "animLayer"})), json!(["BaseAnimation", "L1", "L2"]));

    // Sampled keys every -sb frames; smart keeps only the sources' key times on straight lines.
    let keys = |s: &mut Session, c: &str| run(s, "keyframe", json!({"args": [c], "query": true, "queried": ["keyframeCount"]})).as_i64().unwrap();
    for (smart, want) in [(false, 11), (true, 2)] {
        let mut s = Session::new();
        one_layer(&mut s);
        layer_on(&mut s, "L2", "pCube1.tx");
        key_raw(&mut s, "pCube1.tx", "L2", 1.0, 0.0);
        key_raw(&mut s, "pCube1.tx", "L2", 11.0, 0.0);
        run(&mut s, "animLayerMerge", json!({"args": ["L1", "L2"], "smart": smart}));
        assert_eq!(keys(&mut s, "pCube1_translateX_L1_inputB"), want, "smart {smart}");
    }
}

#[test]
fn menu_commands() {
    let mut s = Session::new();
    one_layer(&mut s);
    // Copy with and without animation; copyAnimation and extractAnimation by name.
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["C1"], "copy": "L1"})), json!("C1"));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["C1"], "query": true, "queried": ["animCurves"]})), json!(["pCube1_translateX_C1_inputB"]));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["C2"], "copyNoAnimation": "L1"})), json!("C2"));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["C2"], "query": true, "queried": ["animCurves"]})), json!([]));
    run(&mut s, "animLayer", json!({"args": ["C2"], "edit": true, "extractAnimation": "L1"}));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["animCurves"]})), json!([]));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["C2"], "query": true, "queried": ["animCurves"]})), json!(["pCube1_translateX_C2_inputB"]));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["pCube1.tx", "L1"], "query": true, "layeredPlug": "pCube1.tx"})), json!("pCube1_translateX_L1.inputB"));
    // Extract Selected Objects.
    let mut s = Session::new();
    one_layer(&mut s);
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "layerEditorExtractObjects", json!({"args": ["L1"]})), json!(["L1_extract"]));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["attribute"]})), json!([]));
    near(at(&mut s, "pCube1.tx", 1.0), 14.0, 1e-9, "same result");
    // Select Objects, Delete Empty.
    run(&mut s, "select", json!({"clear": true}));
    assert_eq!(run(&mut s, "layerEditorSelectObjects", json!({"args": ["L1_extract"]})), json!(["pCube1"]));
    assert_eq!(run(&mut s, "layerEditorDeleteEmpty", json!({})), json!(1));
    // Extract non-layered animation keeps the result.
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 1, "value": 3.0}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx"], "time": 11, "value": 13.0}));
    let want = at(&mut s, "pCube1.tx", 6.0);
    run(&mut s, "layerEditorExtractNonLayered", json!({}));
    near(at(&mut s, "pCube1.tx", 6.0), want, 1e-9, "extracted");
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurveTL"})), json!(["pCube1_translateX_AnimLayer1_inputB"]));
}

#[test]
fn ma_layout_and_round_trip() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.tx", "pCube1.ry", "pCube1.sx", "pCube1.v"], "time": 1}));
    run(&mut s, "animLayer", json!({"args": ["AnimLayer1"], "addSelectedObjects": true, "weight": 0.5}));
    key_raw(&mut s, "pCube1.tx", "AnimLayer1", 1.0, 4.0);
    key_raw(&mut s, "pCube1.ry", "AnimLayer1", 1.0, 30.0);
    key_raw(&mut s, "pCube1.sx", "AnimLayer1", 1.0, 4.0);
    let want = [at(&mut s, "pCube1.tx", 1.0), at(&mut s, "pCube1.ry", 1.0), at(&mut s, "pCube1.sx", 1.0)];
    let (text, warnings) = aztek_scene::ma_file::write_text(&mut s, "t.ma", None);
    assert!(warnings.iter().all(|w| !w.contains("anim")), "{warnings:?}");
    for line in [
        "createNode animLayer -n \"BaseAnimation\";",
        "setAttr \".ovrd\" yes;",
        "setAttr \".wgth\" 0.5;",
        "connectAttr \"AnimLayer1.play\" \"BaseAnimation.cdly[0]\";",
        "connectAttr \"AnimLayer1.sl\" \"BaseAnimation.chsl[0]\";",
        "connectAttr \"BaseAnimation.csol\" \"AnimLayer1.sslo\";",
        "connectAttr \"BaseAnimation.fgwt\" \"AnimLayer1.pwth\";",
        "connectAttr \"BaseAnimation.omte\" \"AnimLayer1.pmte\";",
        "connectAttr \"AnimLayer1.fgwt\" \"pCube1_translateX_AnimLayer1.wb\";",
        "connectAttr \"AnimLayer1.bgwt\" \"pCube1_translateX_AnimLayer1.wa\";",
        "connectAttr \"AnimLayer1.oram\" \"pCube1_rotate_AnimLayer1.acm\";",
        "connectAttr \"AnimLayer1.sam\" \"pCube1_scaleX_AnimLayer1.acm\";",
        "connectAttr \"pCube1.ro\" \"pCube1_rotate_AnimLayer1.ro\";",
        "connectAttr \"pCube1_rotate_AnimLayer1.oy\" \"pCube1.ry\";",
        "connectAttr \"pCube1_rotate_AnimLayer1_inputBY.o\" \"pCube1_rotate_AnimLayer1.iby\";",
        "connectAttr \"pCube1_rotateY.o\" \"pCube1_rotate_AnimLayer1.iay\";",
        "connectAttr \"pCube1_translateX_AnimLayer1_inputB.o\" \"pCube1_translateX_AnimLayer1.ib\";",
        "connectAttr \"pCube1_translateX_AnimLayer1.msg\" \"AnimLayer1.bnds",
        "connectAttr \"pCube1.tx\" \"AnimLayer1.dsm\" -na;",
    ] {
        assert!(text.contains(line), "missing: {line}");
    }
    let mut b = Session::new();
    let warnings = aztek_scene::ma_file::read_text(&mut b, &text, false);
    assert!(warnings.iter().all(|w| !w.contains("anim") && !w.contains("Anim")), "{warnings:?}");
    assert_eq!(run(&mut b, "animLayer", json!({"query": true, "queried": ["layeredHierarchy"]})), json!(["BaseAnimation", "AnimLayer1"]));
    near(at(&mut b, "pCube1.tx", 1.0), want[0], 1e-9, "tx");
    near(at(&mut b, "pCube1.ry", 1.0), want[1], 1e-9, "ry");
    near(at(&mut b, "pCube1.sx", 1.0), want[2], 1e-9, "sx");
    assert_eq!(run(&mut b, "animLayer", json!({"args": ["AnimLayer1"], "query": true, "queried": ["animCurves"]})).as_array().unwrap().len(), 3);
    // Written again, it says the same.
    let (again, _) = aztek_scene::ma_file::write_text(&mut b, "t.ma", None);
    let conns = |t: &str| t.lines().filter(|l| l.starts_with("connectAttr") && (l.contains("AnimLayer1") || l.contains("BaseAnimation"))).map(str::to_string).collect::<std::collections::BTreeSet<_>>();
    assert_eq!(conns(&text), conns(&again));
}

/// By hand, in the words of a scene saved by the reference: a boolean blend's saved output, a rotation by axes.
const REF_TEXT: &str = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "ball";
createNode animLayer -n "BaseAnimation";
	setAttr ".ovrd" yes;
createNode animLayer -n "AnimLayer1";
	setAttr -s 3 ".dsm";
	setAttr -s 2 ".bnds";
	setAttr ".pref" yes;
	setAttr ".slct" yes;
createNode animBlendNodeBoolean -n "ball_visibility_AnimLayer1";
	setAttr ".ia" yes;
	setAttr ".o" yes;
createNode animBlendNodeAdditiveRotation -n "ball_rotate_AnimLayer1";
	setAttr ".ia" -type "double3" 10 0 0 ;
	setAttr ".o" -type "double3" 10 0 0 ;
createNode animCurveTA -n "ball_rotate_AnimLayer1_inputBX";
	setAttr ".tan" 18;
	setAttr ".wgt" no;
	setAttr ".ktv[0]"  1 25;
createNode animCurveTU -n "ball_visibility_AnimLayer1_inputB";
	setAttr ".tan" 9;
	setAttr ".wgt" no;
	setAttr ".ktv[0]"  1 0;
connectAttr "ball_visibility_AnimLayer1.o" "ball.v";
connectAttr "ball_rotate_AnimLayer1.ox" "ball.rx";
connectAttr "ball_rotate_AnimLayer1.oy" "ball.ry";
connectAttr "ball_rotate_AnimLayer1.oz" "ball.rz";
connectAttr "AnimLayer1.sl" "BaseAnimation.chsl[0]";
connectAttr "AnimLayer1.play" "BaseAnimation.cdly[0]";
connectAttr "BaseAnimation.csol" "AnimLayer1.sslo";
connectAttr "BaseAnimation.fgwt" "AnimLayer1.pwth";
connectAttr "BaseAnimation.omte" "AnimLayer1.pmte";
connectAttr "ball.v" "AnimLayer1.dsm" -na;
connectAttr "ball.rx" "AnimLayer1.dsm" -na;
connectAttr "ball_visibility_AnimLayer1.msg" "AnimLayer1.bnds[0]";
connectAttr "ball_rotate_AnimLayer1.msg" "AnimLayer1.bnds[1]";
connectAttr "AnimLayer1.bgwt" "ball_visibility_AnimLayer1.wa";
connectAttr "AnimLayer1.fgwt" "ball_visibility_AnimLayer1.wb";
connectAttr "ball_visibility_AnimLayer1_inputB.o" "ball_visibility_AnimLayer1.ib";
connectAttr "ball.ro" "ball_rotate_AnimLayer1.ro";
connectAttr "AnimLayer1.oram" "ball_rotate_AnimLayer1.acm";
connectAttr "AnimLayer1.bgwt" "ball_rotate_AnimLayer1.wa";
connectAttr "AnimLayer1.fgwt" "ball_rotate_AnimLayer1.wb";
connectAttr "ball_rotate_AnimLayer1_inputBX.o" "ball_rotate_AnimLayer1.ibx";
"#;

#[test]
fn reads_a_scene_in_the_references_words() {
    let mut s = Session::new();
    let warnings = aztek_scene::ma_file::read_text(&mut s, REF_TEXT, false);
    assert!(warnings.iter().all(|w| !w.contains("AnimLayer") && !w.contains("animLayer") && !w.contains("BaseAnimation")), "{warnings:?}");
    near(at(&mut s, "ball.rx", 1.0), 35.0, 1e-9, "stored inputA plus the layer's curve");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["ball.v"]})), json!(false));
    run(&mut s, "animLayer", json!({"args": ["AnimLayer1"], "edit": true, "mute": true}));
    run(&mut s, "currentTime", json!({"args": [2]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["ball.v"]})), json!(true), "muted, the boolean falls to input A");
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["AnimLayer1"], "query": true, "queried": ["attribute"]})).as_array().unwrap().len(), 4);
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["AnimLayer1"], "query": true, "queried": ["preferred"]})), json!(true));
}

#[test]
fn undo_takes_a_layer_back() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.ry"], "time": 1, "value": 5.0}));
    run(&mut s, "animLayer", json!({"args": ["L1"], "addSelectedObjects": true}));
    assert!(run(&mut s, "ls", json!({"type": "animBlendNodeAdditiveRotation"})).as_array().unwrap().len() == 1);
    s.graph.undo();
    assert_eq!(run(&mut s, "ls", json!({"type": "animBlendNodeAdditiveRotation"})), json!([]));
    near(at(&mut s, "pCube1.ry", 1.0), 5.0, 1e-9, "back on the base curve");
}

#[test]
fn bake_with_layers() {
    let curve = |s: &mut Session, name: &str, t: f64| run(s, "keyframe", json!({"args": [name], "query": true, "time": t, "queried": ["valueChange"]}))[0].as_f64().unwrap();
    // By default the result goes on the base and the layers are muted.
    let mut s = Session::new();
    one_layer(&mut s);
    let want = at(&mut s, "pCube1.tx", 1.0);
    run(&mut s, "bakeResults", json!({"args": ["pCube1.tx"], "time": "1:3"}));
    near(at(&mut s, "pCube1.tx", 2.0), want, 1e-9, "same result");
    near(curve(&mut s, "pCube1_translateX", 2.0), 14.0, 1e-9, "on the base");
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["mute"]})), json!(true));
    // -ral takes the channels out instead.
    let mut s = Session::new();
    one_layer(&mut s);
    run(&mut s, "bakeResults", json!({"args": ["pCube1.tx"], "time": "1:3", "removeBakedAttributeFromLayer": true}));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["attribute"]})), json!([]));
    near(at(&mut s, "pCube1.tx", 2.0), 14.0, 1e-9, "same result");
    // -dl: onto a layer, solved against what is under it.
    let mut s = Session::new();
    one_layer(&mut s);
    layer_on(&mut s, "L2", "pCube1.tx");
    key_raw(&mut s, "pCube1.tx", "L2", 1.0, 1.0);
    run(&mut s, "bakeResults", json!({"args": ["pCube1.tx"], "time": "1:3", "destinationLayer": "L2"}));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["mute"]})), json!(true));
    near(curve(&mut s, "pCube1_translateX_L2_inputB", 2.0), 5.0, 1e-9, "15 over the base's 10");
    near(at(&mut s, "pCube1.tx", 2.0), 15.0, 1e-9, "same result");
    // -bol: a new override layer on top, the others left alone.
    let mut s = Session::new();
    one_layer(&mut s);
    run(&mut s, "bakeResults", json!({"args": ["pCube1.tx"], "time": "1:3", "bakeOnOverrideLayer": true}));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["BakeResults"], "query": true, "queried": ["override"]})), json!(true));
    assert_eq!(run(&mut s, "animLayer", json!({"args": ["L1"], "query": true, "queried": ["mute"]})), json!(false));
    near(at(&mut s, "pCube1.tx", 2.0), 14.0, 1e-9, "same result");
}
