//! NURBS curves: creation, CVs as components, history, info nodes, meshes from curves.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn num(v: Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn line(s: &mut Session) -> Value {
    run(s, "curve", json!({"degree": 3, "point": [[0, 0, 0], [1, 0, 0], [2, 1, 0], [3, 1, 0], [4, 0, 0]]}))
}

#[test]
fn curve_makes_transform_and_shape() {
    let mut s = Session::new();
    assert_eq!(line(&mut s), json!("curve1"));
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsCurve"})), json!(["curveShape1"]));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["curve1.degree"]}))), 3.0);
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["curveShape1.spans"]}))), 2.0);
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["curveShape1.maxValue"]}))), 2.0);
    // Flat points, as MEL gives them; too few CVs for the degree lowers it.
    run(&mut s, "curve", json!({"degree": 3, "point": [0, 0, 0, 1, 0, 0], "name": "seg"}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["seg.degree"]}))), 1.0);
    // Knots are checked.
    assert!(s.run("curve", json!({"degree": 1, "point": [0, 0, 0, 1, 0, 0], "knot": [0, 1, 2]})).is_err());
    run(&mut s, "curve", json!({"degree": 1, "point": [0, 0, 0, 1, 0, 0], "knot": [0, 5], "name": "k"}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["k.maxValue"]}))), 5.0);
}

#[test]
fn cvs_are_components() {
    let mut s = Session::new();
    line(&mut s);
    assert_eq!(run(&mut s, "ls", json!({"args": ["curve1.cv[*]"], "flatten": true})).as_array().unwrap().len(), 5);
    assert_eq!(run(&mut s, "xform", json!({"args": ["curve1.cv[2]"], "query": true, "worldSpace": true, "translation": true})), json!([2.0, 1.0, 0.0]));
    run(&mut s, "select", json!({"args": ["curve1.cv[4]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["curve1.cv[4]"]));
    run(&mut s, "moveComponents", json!({"by": [0, 5, 0]}));
    assert_eq!(run(&mut s, "xform", json!({"args": ["curve1.cv[4]"], "query": true, "worldSpace": true, "translation": true})), json!([4.0, 5.0, 0.0]));
    assert!(s.undo());
    assert_eq!(run(&mut s, "xform", json!({"args": ["curve1.cv[4]"], "query": true, "worldSpace": true, "translation": true})), json!([4.0, 0.0, 0.0]));
    // A moved transform moves them in the world.
    run(&mut s, "move", json!({"args": ["curve1"], "to": [10, 0, 0]}));
    assert_eq!(run(&mut s, "xform", json!({"args": ["curve1.cv[0]"], "query": true, "worldSpace": true, "translation": true})), json!([10.0, 0.0, 0.0]));
    run(&mut s, "xform", json!({"args": ["curve1.cv[0]"], "worldSpace": true, "translation": [10.0, 1.0, 0.0]}));
    assert_eq!(run(&mut s, "xform", json!({"args": ["curve1.cv[0]"], "query": true, "translation": true})), json!([0.0, 1.0, 0.0]));
}

#[test]
fn circle_has_history() {
    let mut s = Session::new();
    let r = run(&mut s, "circle", json!({"normal": [0, 1, 0], "radius": 2.0}));
    assert_eq!(r, json!(["nurbsCircle1", "makeNurbCircle1"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsCurve"})), json!(["nurbsCircleShape1"]));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["nurbsCircle1.form"]}))), 2.0);
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["nurbsCircle1.spans"]}))), 8.0);
    assert_eq!(run(&mut s, "ls", json!({"args": ["nurbsCircle1.cv[*]"], "flatten": true})).as_array().unwrap().len(), 8);
    let len = num(run(&mut s, "arclen", json!({"args": ["nurbsCircle1"]})));
    assert!((len - std::f64::consts::TAU * 2.0).abs() < 0.2, "{len}");
    // Changing the radius on the node rebuilds it.
    run(&mut s, "setAttr", json!({"args": ["makeNurbCircle1.radius", 1.0]}));
    let len = num(run(&mut s, "arclen", json!({"args": ["nurbsCircle1"]})));
    assert!((len - std::f64::consts::TAU).abs() < 0.1, "{len}");
    // CVs move on top of history, as tweaks.
    run(&mut s, "xform", json!({"args": ["nurbsCircle1.cv[0]"], "relative": true, "translation": [0, 3, 0]}));
    let y = run(&mut s, "xform", json!({"args": ["nurbsCircle1.cv[0]"], "query": true, "translation": true}))[1].as_f64().unwrap();
    assert!((y - 3.0).abs() < 1e-9, "{y}");
    run(&mut s, "setAttr", json!({"args": ["makeNurbCircle1.radius", 2.0]}));
    let y = run(&mut s, "xform", json!({"args": ["nurbsCircle1.cv[0]"], "query": true, "translation": true}))[1].as_f64().unwrap();
    assert!((y - 3.0).abs() < 1e-9, "{y}");
    // Without history.
    let r = run(&mut s, "circle", json!({"constructionHistory": false}));
    assert_eq!(r, json!(["nurbsCircle2"]));
    // With its history baked, a CV moves the curve itself.
    run(&mut s, "xform", json!({"args": ["nurbsCircle2.cv[0]"], "relative": true, "translation": [0, 0, 3]}));
    let z = run(&mut s, "xform", json!({"args": ["nurbsCircle2.cv[0]"], "query": true, "translation": true}))[2].as_f64().unwrap();
    assert!((z - 3.0).abs() < 1e-9, "{z}");
}

#[test]
fn edits_keep_history_or_not() {
    let mut s = Session::new();
    line(&mut s);
    assert_eq!(run(&mut s, "rebuildCurve", json!({"args": ["curve1"], "spans": 6, "degree": 3})), json!(["curve1", "rebuildCurve1"]));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["curve1.spans"]}))), 6.0);
    run(&mut s, "setAttr", json!({"args": ["rebuildCurve1.spans", 3]}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["curve1.spans"]}))), 3.0);
    let end = |s: &mut Session| run(s, "pointOnCurve", json!({"args": ["curve1"], "parameter": 1.0, "turnOnPercentage": true}));
    let e = end(&mut s);
    assert!((e[0].as_f64().unwrap() - 4.0).abs() < 1e-6, "{e}");
    run(&mut s, "reverseCurve", json!({"args": ["curve1"]}));
    let e = end(&mut s);
    assert!(e[0].as_f64().unwrap().abs() < 1e-6, "{e}");
    assert_eq!(run(&mut s, "ls", json!({"type": "reverseCurve"})), json!(["reverseCurve1"]));
    // No history: nothing left behind.
    run(&mut s, "smoothCurve", json!({"args": ["curve1"], "constructionHistory": false}));
    assert_eq!(run(&mut s, "ls", json!({"type": "smoothCurve"})), json!([]));
    // A copy.
    let r = run(&mut s, "offsetCurve", json!({"args": ["curve1"], "distance": 1.0, "normal": [0, 0, 1]}));
    assert_eq!(r[0], json!("curve2"));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [1, 0, 0]], "name": "a"}));
    run(&mut s, "curve", json!({"args": ["a"], "append": true, "point": [[2, 0, 0]]}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["a.spans"]}))), 2.0);
    assert_eq!(run(&mut s, "ls", json!({"args": ["a.cv[*]"], "flatten": true})).as_array().unwrap().len(), 3);
    // The pieces, then the detachCurve node that makes them.
    let pieces = run(&mut s, "detachCurve", json!({"args": ["a"], "parameter": [1.0], "replaceOriginal": true}));
    assert_eq!(pieces.as_array().unwrap().len(), 3, "{pieces}");
    assert!((num(run(&mut s, "arclen", json!({"args": ["a"]}))) - 1.0).abs() < 1e-6);
    assert!((num(run(&mut s, "arclen", json!({"args": [pieces[1]]}))) - 1.0).abs() < 1e-6);
    let joined = run(&mut s, "attachCurve", json!({"args": ["a", pieces[1]]}));
    assert!((num(run(&mut s, "arclen", json!({"args": [joined]}))) - 2.0).abs() < 0.05);
}

#[test]
fn close_and_info_nodes() {
    let mut s = Session::new();
    line(&mut s);
    run(&mut s, "closeCurve", json!({"args": ["curve1"]}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["curve1.form"]}))), 2.0);
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [3, 0, 0]], "name": "seg"}));
    run(&mut s, "createNode", json!({"args": ["curveInfo"], "name": "info"}));
    run(&mut s, "connectAttr", json!({"args": ["segShape.worldSpace[0]", "info.inputCurve"]}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["info.arcLength"]}))), 3.0);
    run(&mut s, "scale", json!({"args": ["seg"], "to": [2, 1, 1]}));
    assert_eq!(num(run(&mut s, "getAttr", json!({"args": ["info.arcLength"]}))), 6.0);
    run(&mut s, "createNode", json!({"args": ["pointOnCurveInfo"], "name": "poci"}));
    run(&mut s, "connectAttr", json!({"args": ["segShape.worldSpace[0]", "poci.inputCurve"]}));
    run(&mut s, "setAttr", json!({"args": ["poci.turnOnPercentage", 1]}));
    run(&mut s, "setAttr", json!({"args": ["poci.parameter", 0.5]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["poci.position"]})), json!([3.0, 0.0, 0.0]));
    run(&mut s, "createNode", json!({"args": ["nearestPointOnCurve"], "name": "near"}));
    run(&mut s, "connectAttr", json!({"args": ["segShape.worldSpace[0]", "near.inputCurve"]}));
    run(&mut s, "setAttr", json!({"args": ["near.inPosition", 1.5, 4.0, 0.0]}));
    let p = run(&mut s, "getAttr", json!({"args": ["near.position"]}));
    assert!((p[0].as_f64().unwrap() - 1.5).abs() < 1e-4 && p[1].as_f64().unwrap().abs() < 1e-6, "{p}");
}

fn faces(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

#[test]
fn meshes_from_curves() {
    let mut s = Session::new();
    run(&mut s, "circle", json!({"normal": [0, 1, 0], "radius": 0.5, "name": "profile"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [0, 4, 0]], "name": "path"}));
    let r = run(&mut s, "extrude", json!({"args": ["profile", "path"], "polygon": 1}));
    assert_eq!(r, json!(["extrudedSurface1", "extrude1"]));
    assert!(faces(&mut s, "extrudedSurface1") > 0);
    let i = run(&mut s, "meshInfo", json!({"args": ["extrudedSurface1"]}));
    assert!((i["worldBoundsMax"][1].as_f64().unwrap() - 4.0).abs() < 1e-6, "{i}");
    // It follows the path: history stays live.
    run(&mut s, "move", json!({"args": ["path.cv[1]"], "by": [0, 2, 0]}));
    run(&mut s, "xform", json!({"args": ["path.cv[1]"], "worldSpace": true, "translation": [0, 6, 0]}));
    let i = run(&mut s, "meshInfo", json!({"args": ["extrudedSurface1"]}));
    assert!((i["worldBoundsMax"][1].as_f64().unwrap() - 6.0).abs() < 1e-6, "{i}");

    run(&mut s, "curve", json!({"degree": 1, "point": [[1, 0, 0], [1, 2, 0]], "name": "edge"}));
    let r = run(&mut s, "revolve", json!({"args": ["edge"], "sections": 12, "polygon": 1}));
    assert_eq!(r, json!(["revolvedSurface1", "revolve1"]));
    assert_eq!(faces(&mut s, "revolvedSurface1"), 12);
    run(&mut s, "setAttr", json!({"args": ["revolve1.sections", 6]}));
    assert_eq!(faces(&mut s, "revolvedSurface1"), 6);

    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [1, 0, 0]], "name": "c1"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 1, 0], [1, 1, 0]], "name": "c2"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 2, 0], [1, 2, 0]], "name": "c3"}));
    run(&mut s, "loft", json!({"args": ["c1", "c2", "c3"], "polygon": 1}));
    assert_eq!(faces(&mut s, "loftedSurface1"), 2);

    run(&mut s, "nurbsSquare", json!({"name": "sq"}));
    run(&mut s, "planar", json!({"args": ["sq"], "polygon": 1}));
    assert_eq!(faces(&mut s, "planarTrimmedSurface1"), 1);

    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [0, 2, 0]], "name": "tube"}));
    let r = run(&mut s, "sweepMeshFromCurve", json!({"args": ["tube"], "scaleProfileX": 0.2, "profilePolySides": 6}));
    assert_eq!(r, json!(["sweep1", "sweepMeshCreator1"]));
    assert_eq!(faces(&mut s, "sweep1"), 6);
}

#[test]
fn curves_save_load_and_duplicate() {
    let mut s = Session::new();
    run(&mut s, "circle", json!({"normal": [0, 1, 0], "radius": 2.0, "name": "ctrl"}));
    line(&mut s);
    run(&mut s, "setAttr", json!({"args": ["curveShape1.overrideEnabled", 1]}));
    run(&mut s, "setAttr", json!({"args": ["curveShape1.overrideColor", 13]}));
    s.graph.evaluate();
    let doc = aztek_scene::scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    aztek_scene::scene_file::deserialize(&mut t.graph, &doc).unwrap();
    assert_eq!(run(&mut t, "ls", json!({"type": "nurbsCurve"})).as_array().unwrap().len(), 2);
    assert!((num(run(&mut t, "arclen", json!({"args": ["ctrl"]}))) - std::f64::consts::TAU * 2.0).abs() < 0.2);
    assert_eq!(num(run(&mut t, "getAttr", json!({"args": ["curve1.spans"]}))), 2.0);
    assert_eq!(num(run(&mut t, "getAttr", json!({"args": ["curveShape1.overrideColor"]}))), 13.0);
    let d = run(&mut s, "duplicate", json!({"args": ["curve1"]}));
    let name = d.as_array().and_then(|a| a.first().cloned()).unwrap_or(d.clone());
    assert_eq!(num(run(&mut s, "arclen", json!({"args": [name]}))) > 0.0, true);
    run(&mut s, "delete", json!({"args": ["ctrl"], "constructionHistory": true}));
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbCircle"})), json!([]));
    assert!((num(run(&mut s, "arclen", json!({"args": ["ctrl"]}))) - std::f64::consts::TAU * 2.0).abs() < 0.2);
}
