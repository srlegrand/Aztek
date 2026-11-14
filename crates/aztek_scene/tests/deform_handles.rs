//! Nonlinear deformers and deformer handles laid out as the reference has them: one nonLinear type with its kind's
//! settings added, a handle transform with the kind's shape under it (deformBend..., clusterHandle, implicitSphere),
//! the settings connected from the deformer into the shape.

use aztek_scene::ma_file::{read_text, write_text};
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn ty(s: &mut Session, n: &str) -> Value {
    run(s, "nodeType", json!({"args": [n]}))
}

#[test]
fn every_kind_is_a_nonlinear_with_its_handle_shape() {
    for (kind, shape, attr) in [("bend", "deformBend", "curvature"), ("twist", "deformTwist", "startAngle"), ("squash", "deformSquash", "factor"), ("flare", "deformFlare", "curve"), ("sine", "deformSine", "amplitude"), ("wave", "deformWave", "maxRadius")] {
        let mut s = Session::new();
        run(&mut s, "polyCylinder", json!({"height": 4, "subdivisionsHeight": 8}));
        let r = run(&mut s, "nonLinear", json!({"args": ["pCylinder1"], "type": kind}));
        assert_eq!(r, json!([format!("{kind}1"), format!("{kind}1Handle")]));
        assert_eq!(ty(&mut s, &format!("{kind}1")), json!("nonLinear"));
        assert_eq!(ty(&mut s, &format!("{kind}1Handle")), json!("transform"));
        let shapes = run(&mut s, "listRelatives", json!({"args": [format!("{kind}1Handle")], "shapes": true}));
        assert_eq!(shapes, json!([format!("{kind}1HandleShape")]));
        assert_eq!(ty(&mut s, &format!("{kind}1HandleShape")), json!(shape));
        // The setting is the deformer's, and drives the shape's.
        run(&mut s, "setAttr", json!({"args": [format!("{kind}1.{attr}"), 0.5]}));
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{kind}1HandleShape.{attr}")]})), json!(0.5), "{kind}");
        let user = run(&mut s, "listAttr", json!({"args": [format!("{kind}1")], "userDefined": true}));
        assert!(user.as_array().unwrap().iter().any(|a| a == attr), "{kind}: {user}");
    }
}

#[test]
fn cluster_and_sculpt_handles_have_the_reference_shapes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [3, 0, 0]}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    assert_eq!(ty(&mut s, "cluster1Handle"), json!("transform"));
    assert_eq!(ty(&mut s, "cluster1HandleShape"), json!("clusterHandle"));
    let o = run(&mut s, "getAttr", json!({"args": ["cluster1HandleShape.origin"]}));
    let x = o[0].as_f64().or_else(|| o[0][0].as_f64()).unwrap();
    assert!((x - 3.0).abs() < 1e-9, "{o}");
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "sculpt", json!({"args": ["pSphere1"]}));
    let shapes = run(&mut s, "listRelatives", json!({"args": ["sculptor1"], "shapes": true}));
    let shape = shapes[0].as_str().unwrap().to_string();
    assert_eq!(ty(&mut s, &shape), json!("implicitSphere"));
}

#[test]
fn a_bend_writes_and_reads_as_the_reference_has_it() {
    let mut s = Session::new();
    run(&mut s, "polyCylinder", json!({"height": 4, "subdivisionsHeight": 8}));
    run(&mut s, "nonLinear", json!({"args": ["pCylinder1"], "type": "bend", "curvature": 1.0}));
    let (text, _) = write_text(&mut s, "b.ma", None);
    for want in [
        "createNode nonLinear -n \"bend1\";",
        "createNode transform -n \"bend1Handle\";",
        "createNode deformBend -n \"bend1HandleShape\" -p \"bend1Handle\";",
        "connectAttr \"bend1HandleShape.dd\" \"bend1.dd\";",
        "connectAttr \"bend1.curvature\" \"bend1HandleShape.curvature\";",
    ] {
        assert!(text.lines().any(|l| l == want), "missing {want:?} in\n{text}");
    }
    assert!(text.lines().any(|l| l.contains("addAttr") && l.contains("-ln \"curvature\"")), "{text}");
    assert!(!text.contains("deformHandle"), "{text}");

    let mut b = Session::new();
    read_text(&mut b, &text, false);
    assert_eq!(ty(&mut b, "bend1"), json!("nonLinear"));
    assert_eq!(ty(&mut b, "bend1HandleShape"), json!("deformBend"));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["bend1.curvature"]})), json!(1.0));
    run(&mut b, "setAttr", json!({"args": ["bend1.curvature", 0.25]}));
    assert_eq!(run(&mut b, "getAttr", json!({"args": ["bend1HandleShape.curvature"]})), json!(0.25));
    let (again, _) = write_text(&mut b, "b.ma", None);
    assert_eq!(again.lines().filter(|l| l.starts_with("createNode deformBend")).count(), 1, "{again}");
}

#[test]
fn a_twist_from_the_reference_reads_its_angles_in_degrees() {
    let file = r#"//Maya ASCII 2024 scene
requires maya "2024";
currentUnit -l centimeter -a degree -t film;
createNode transform -n "pCylinder1";
createNode mesh -n "pCylinderShape1" -p "pCylinder1";
createNode polyCylinder -n "polyCylinder1";
	setAttr ".h" 4;
	setAttr ".sh" 8;
createNode nonLinear -n "twist1";
	addAttr -is true -ci true -k true -sn "sa" -ln "startAngle" -at "doubleAngle";
	addAttr -is true -ci true -k true -sn "ea" -ln "endAngle" -at "doubleAngle";
	addAttr -is true -ci true -k true -sn "lb" -ln "lowBound" -dv -1 -at "double";
	addAttr -is true -ci true -k true -sn "hb" -ln "highBound" -dv 1 -at "double";
	setAttr -k on ".ea" 90;
createNode transform -n "twist1Handle";
createNode deformTwist -n "twist1HandleShape" -p "twist1Handle";
	setAttr -k off ".v";
	setAttr ".ea" 90;
connectAttr "polyCylinder1.out" "pCylinderShape1.i";
connectAttr "twist1.ea" "twist1HandleShape.ea";
connectAttr "twist1.sa" "twist1HandleShape.sa";
connectAttr "twist1.lb" "twist1HandleShape.lb";
connectAttr "twist1.hb" "twist1HandleShape.hb";
connectAttr "twist1HandleShape.dd" "twist1.dd";
connectAttr "twist1Handle.wm" "twist1.ma";
"#;
    let mut s = Session::new();
    read_text(&mut s, file, false);
    let ea = run(&mut s, "getAttr", json!({"args": ["twist1HandleShape.endAngle"]})).as_f64().unwrap();
    assert!((ea - 90.0).abs() < 1e-6, "{ea}");
    assert_eq!(ty(&mut s, "twist1HandleShape"), json!("deformTwist"));
}
