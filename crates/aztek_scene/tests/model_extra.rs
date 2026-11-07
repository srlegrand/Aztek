//! Bezier curves, explicit surface UVs, blend2, roundConstantRadius and polyInstallAction.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn cv(s: &mut Session, comp: &str) -> [f64; 3] {
    let v = run(s, "pointPosition", json!({"args": [comp]}));
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    (0..3).map(|k| (a[k] - b[k]).powi(2)).sum::<f64>().sqrt()
}

fn bezier_curve(s: &mut Session) {
    run(s, "curve", json!({"point": [[0, 0, 0], [1, 1, 0], [2, 1, 0], [3, 0, 0], [4, -2, 0], [6, -1, 0], [7, 0, 0]], "bezier": true, "name": "bz"}));
}

#[test]
fn a_curve_becomes_bezier_and_keeps_its_shape() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"point": [[0, 0, 0], [1, 2, 0], [3, 2, 0], [4, 0, 0], [6, 1, 0], [7, 0, 0]], "name": "crv"}));
    let before = run(&mut s, "pointOnCurve", json!({"args": ["crv"], "parameter": 1.5}));
    let r = run(&mut s, "nurbsCurveToBezier", json!({"args": ["crv"]}));
    assert_eq!(r.as_array().unwrap().len(), 2, "[object, node]");
    assert_eq!(run(&mut s, "objectType", json!({"args": [r[1]]})), json!("nurbsCurveToBezier"));
    let after = run(&mut s, "pointOnCurve", json!({"args": ["crv"], "parameter": 1.5}));
    for k in 0..3 {
        assert!((before[k].as_f64().unwrap() - after[k].as_f64().unwrap()).abs() < 1e-6);
    }
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["crv.degree"]})), json!(3));
    let n = run(&mut s, "getAttr", json!({"args": ["crv.spans"]})).as_i64().unwrap();
    assert_eq!(n, 7, "cvs less the degree: ten CVs, three Bezier segments");
    let back = run(&mut s, "bezierCurveToNurbs", json!({"args": ["crv"]}));
    assert_eq!(back.as_array().unwrap().len(), 2);
    run(&mut s, "curve", json!({"point": [[0, 0, 0], [1, 2, 0], [3, 2, 0], [4, 0, 0], [6, 1, 0]], "name": "plain"}));
    assert!(s.run("bezierCurveToNurbs", json!({"args": ["plain"]})).is_err(), "not Bezier");
}

#[test]
fn bezier_info_answers() {
    let mut s = Session::new();
    bezier_curve(&mut s);
    assert_eq!(run(&mut s, "bezierInfo", json!({"anchorFromCV": 4})), json!(1));
    assert_eq!(run(&mut s, "bezierInfo", json!({"anchorFromCV": 3})), json!(1));
    assert_eq!(run(&mut s, "bezierInfo", json!({"anchorFromCV": 2})), json!(1));
    assert_eq!(run(&mut s, "bezierInfo", json!({"cvFromAnchor": 2})), json!(6));
    run(&mut s, "select", json!({"args": ["bz.cv[3]"]}));
    assert_eq!(run(&mut s, "bezierInfo", json!({"isAnchorSelected": true})), json!(1));
    assert_eq!(run(&mut s, "bezierInfo", json!({"isTangentSelected": true})), json!(0));
    assert_eq!(run(&mut s, "bezierInfo", json!({"onlyAnchorsSelected": true})), json!(1));
    run(&mut s, "select", json!({"args": ["bz.cv[4]"]}));
    assert_eq!(run(&mut s, "bezierInfo", json!({"onlyTangentsSelected": true})), json!(1));
}

#[test]
fn anchors_take_state_and_presets() {
    let mut s = Session::new();
    bezier_curve(&mut s);
    run(&mut s, "select", json!({"args": ["bz.cv[3]"]}));
    assert_eq!(run(&mut s, "bezierAnchorState", json!({"smooth": true, "even": true})), json!(1));
    let (a, i, o) = (cv(&mut s, "bz.cv[3]"), cv(&mut s, "bz.cv[2]"), cv(&mut s, "bz.cv[4]"));
    assert!((dist(a, i) - dist(a, o)).abs() < 1e-6, "even");
    let dir = |x: [f64; 3]| { let l = dist(x, a); [(x[0] - a[0]) / l, (x[1] - a[1]) / l, (x[2] - a[2]) / l] };
    let (di, d_o) = (dir(i), dir(o));
    assert!((0..3).all(|k| (di[k] + d_o[k]).abs() < 1e-6), "smooth: {di:?} {d_o:?}");
    assert_eq!(a, [3.0, 0.0, 0.0], "the anchor stays");
    // Corner draws the tangents in.
    assert_eq!(run(&mut s, "bezierAnchorPreset", json!({"preset": 2})), json!(1));
    assert!(dist(cv(&mut s, "bz.cv[2]"), a) < 1e-9 && dist(cv(&mut s, "bz.cv[4]"), a) < 1e-9);
    // Bezier Corner: a third of the way to each neighbour.
    run(&mut s, "bezierAnchorPreset", json!({"preset": 1}));
    let next = cv(&mut s, "bz.cv[6]");
    let t = cv(&mut s, "bz.cv[4]");
    assert!(dist(t, [a[0] + (next[0] - a[0]) / 3.0, 0.0, 0.0]) < 1e-9);
    assert!(s.run("bezierAnchorPreset", json!({"preset": 9})).is_err());
    assert!(s.undo());
    run(&mut s, "select", json!({"clear": true}));
    assert!(s.run("bezierAnchorState", json!({"smooth": true})).is_err(), "nothing selected");
}

fn plane(s: &mut Session, name: &str, pivot: [f64; 3], axis: [f64; 3]) {
    run(s, "nurbsPlane", json!({"name": name, "pivot": pivot, "axis": axis, "width": 2.0, "lengthRatio": 1.0, "patchesU": 2, "patchesV": 2, "constructionHistory": false}));
}

#[test]
fn explicit_surface_uvs() {
    let mut s = Session::new();
    plane(&mut s, "pl", [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    assert_eq!(run(&mut s, "nurbsUVSet", json!({"args": ["pl"], "query": true, "useExplicit": true})), json!(false));
    assert!(s.run("nurbsUVSet", json!({"args": ["pl"], "useExplicit": true})).is_err(), "no set yet");
    assert!(s.run("nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "uValue": 0.5})).is_err(), "no set yet");
    run(&mut s, "nurbsUVSet", json!({"args": ["pl"], "create": true}));
    run(&mut s, "nurbsUVSet", json!({"args": ["pl"], "useExplicit": true}));
    assert_eq!(run(&mut s, "nurbsUVSet", json!({"args": ["pl"], "query": true, "useExplicit": true})), json!(true));
    assert_eq!(run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "query": true})), json!([0.0, 0.0]));
    run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "uValue": 0.25, "vValue": 0.5}));
    assert_eq!(run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "query": true})), json!([0.25, 0.5]));
    run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "relative": false, "uValue": 1.0, "vValue": 0.0}));
    run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "angle": 90.0}));
    let q = run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "query": true}));
    assert!(q[0].as_f64().unwrap().abs() < 1e-9 && (q[1].as_f64().unwrap() - 1.0).abs() < 1e-9, "{q}");
    run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "scaleV": 2.0}));
    assert_eq!(run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "query": true, "queried": ["vValue"]})), json!([2.0]));
    assert!(s.undo());
    run(&mut s, "nurbsCopyUVSet", json!({"args": ["pl"]}));
    assert_eq!(run(&mut s, "nurbsEditUV", json!({"args": ["pl.cv[0][0]"], "query": true})), json!([0.0, 0.0]));
}

#[test]
fn blend2_spans_two_curves_and_two_edges() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"point": [[0, 0, 0], [1, 0, 0], [2, 0, 0], [3, 0, 0]], "name": "c1"}));
    run(&mut s, "curve", json!({"point": [[0, 4, 0], [1, 4, 0], [2, 4, 0], [3, 4, 0]], "name": "c2"}));
    let r = run(&mut s, "blend2", json!({"args": ["c1", "c2"]}));
    assert_eq!(r.as_array().unwrap().len(), 2, "[surface, node]");
    assert_eq!(run(&mut s, "objectType", json!({"args": [r[1]]})), json!("ffBlendSrf"));
    assert_eq!(run(&mut s, "objectType", json!({"args": [r[0], ], "isAType": "transform"})), json!(true));
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [r[0]]}));
    assert!((bb[1].as_f64().unwrap()).abs() < 1e-6 && (bb[4].as_f64().unwrap() - 4.0).abs() < 1e-6, "{bb}");
    assert!((bb[3].as_f64().unwrap() - 3.0).abs() < 1e-6, "{bb}");
    // A range on the rails narrows it.
    let r2 = run(&mut s, "blend2", json!({"args": ["c1", "c2"], "leftEnd": 0.5, "rightEnd": 0.5, "constructionHistory": false}));
    assert_eq!(r2.as_array().unwrap().len(), 1);
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [r2[0]]}));
    assert!((bb[3].as_f64().unwrap() - 1.5).abs() < 1e-6, "{bb}");
    // Edges.
    plane(&mut s, "pa", [0.0, 0.0, 0.0], [0.0, 1.0, 0.0]);
    plane(&mut s, "pb", [5.0, 3.0, 0.0], [0.0, 1.0, 0.0]);
    let r3 = run(&mut s, "blend2", json!({"args": ["pa.u[1]", "pb.u[0]"]}));
    assert_eq!(r3.as_array().unwrap().len(), 2);
    assert!(s.run("blend2", json!({"args": ["c1", "c2"], "crvsInFirstRail": 2})).is_err());
    assert!(s.run("blend2", json!({"args": ["c1"]})).is_err());
}

#[test]
fn round_constant_radius_makes_a_fillet() {
    let mut s = Session::new();
    // Two planes meeting along the Z axis: one along +X, one along +Y.
    let line = |s: &mut Session, name: &str, p: [f64; 3], q: [f64; 3]| {
        run(s, "curve", json!({"degree": 1, "point": [p, q], "name": name}));
    };
    line(&mut s, "l0", [0.0, 0.0, -1.0], [0.0, 0.0, 1.0]);
    line(&mut s, "l1", [2.0, 0.0, -1.0], [2.0, 0.0, 1.0]);
    line(&mut s, "l2", [0.0, 2.0, -1.0], [0.0, 2.0, 1.0]);
    run(&mut s, "loft", json!({"args": ["l0", "l1"], "name": "pa", "constructionHistory": false, "degree": 1}));
    run(&mut s, "loft", json!({"args": ["l0", "l2"], "name": "pb", "constructionHistory": false, "degree": 1}));
    let mut good = false;
    for ea in ["pa.u[0]", "pa.u[1]", "pa.v[0]", "pa.v[1]"] {
        for eb in ["pb.u[0]", "pb.u[1]", "pb.v[0]", "pb.v[1]"] {
            let Ok(r) = s.run("roundConstantRadius", json!({"args": [ea, eb], "radiuss": 0.5})) else { continue };
            let Ok(bb) = s.run("exactWorldBoundingBox", json!({"args": [r[0]]})) else { continue };

            let n = |k: usize| bb[k].as_f64().unwrap_or(f64::NAN);
            if n(0).abs() < 3e-3 && n(1).abs() < 3e-3 && (n(3) - 0.5).abs() < 3e-3 && (n(4) - 0.5).abs() < 3e-3 {
                assert_eq!(r.as_array().unwrap().len(), 2, "[fillet, node]");
                assert_eq!(run(&mut s, "objectType", json!({"args": [r[1]]})), json!("roundConstantRadius"));
                good = true;
            }
        }
    }
    assert!(good, "no pair of edges made a fillet of radius 0.5");
    assert!(s.run("roundConstantRadius", json!({"args": ["pa.u[0]"]})).is_err());
    assert!(s.run("roundConstantRadius", json!({"args": ["pa.u[0]", "pb.u[0]"], "append": true})).is_err(), "must not pretend");
}

#[test]
fn poly_install_action() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "select", json!({"args": ["box"]}));
    run(&mut s, "polyInstallAction", json!({"args": ["polyCloseBorder"], "installConstraint": true}));
    assert_eq!(run(&mut s, "polyInstallAction", json!({"query": true, "commandName": true})), json!("polyCloseBorder"));
    assert_eq!(run(&mut s, "polyInstallAction", json!({"query": true, "installConstraint": true})), json!(1));
    assert_eq!(run(&mut s, "polySelectConstraint", json!({"query": true, "queried": ["where"]})), json!(1));
    run(&mut s, "polyInstallAction", json!({}));
    assert_eq!(run(&mut s, "polyInstallAction", json!({"query": true, "installConstraint": true})), json!(0));
    run(&mut s, "polyInstallAction", json!({"args": ["polyExtrudeFacet"], "convertSelection": true}));
    let sel = run(&mut s, "ls", json!({"selection": true}));
    assert!(sel[0].as_str().unwrap().contains(".f["), "{sel}");
    assert!(s.run("polyInstallAction", json!({"args": ["polyNothing"]})).is_err());
}
