//! NURBS commands: flags, modes and return values from the reference's documentation, numbers from the geometry.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}
fn close(v: &Value, want: f64, tol: f64) {
    assert!((v.as_f64().unwrap_or_else(|| panic!("{v}")) - want).abs() < tol, "got {v}, wanted {want}");
}
fn close3(v: &Value, want: [f64; 3], tol: f64) {
    for i in 0..3 {
        assert!((v[i].as_f64().unwrap_or_else(|| panic!("{v}")) - want[i]).abs() < tol, "got {v}, wanted {want:?}");
    }
}
fn temp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("aztek_audit_nurbs_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name).to_string_lossy().to_string()
}
fn line(s: &mut Session, name: &str, a: [f64; 3], b: [f64; 3]) {
    run(s, "curve", json!({"name": name, "d": 1, "p": [a, b]}));
}
fn plane(s: &mut Session, name: &str) {
    run(s, "nurbsPlane", json!({"name": name, "axis": [0, 1, 0], "w": 10.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
}
fn cv_point(s: &mut Session, comp: &str) -> Value {
    run(s, "xform", json!({"args": [comp], "q": true, "ws": true, "t": true}))
}
fn mesh_area(s: &mut Session, surface: &str) -> f64 {
    // A polygon copy of what shows, trim included.
    s.graph.undo_stack_mut().set_enabled(false);
    let m = run(s, "nurbsToPoly", json!({"args": [surface], "ch": false, "name": "areaProbe"}));
    let name = m[0].as_str().unwrap().to_string();
    let a = run(s, "polyEvaluate", json!({"args": [name.clone()], "wa": true})).as_f64().unwrap();
    run(s, "delete", json!({"args": [name]}));
    s.graph.undo_stack_mut().set_enabled(true);
    a
}

#[test]
fn fillet_two_lines() {
    let mut s = Session::new();
    line(&mut s, "a", [0.0, 0.0, 0.0], [5.0, 0.0, 0.0]);
    line(&mut s, "b", [0.0, 0.0, 0.0], [0.0, 5.0, 0.0]);
    let r = run(&mut s, "filletCurve", json!({"args": ["a", "b"], "r": 1.0}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "filletCurve");
    let curve = r[0].as_str().unwrap().to_string();
    // A quarter circle of radius 1 about (1, 1): from (1, 0) to (0, 1).
    let (lo, hi) = (get(&mut s, &format!("{curve}.minValue")).as_f64().unwrap(), get(&mut s, &format!("{curve}.maxValue")).as_f64().unwrap());
    let p0 = run(&mut s, "pointOnCurve", json!({"args": [curve.clone()], "pr": lo}));
    let p1 = run(&mut s, "pointOnCurve", json!({"args": [curve.clone()], "pr": hi}));
    close3(&p0, [1.0, 0.0, 0.0], 1e-3);
    close3(&p1, [0.0, 1.0, 0.0], 1e-3);
    let mid = run(&mut s, "pointOnCurve", json!({"args": [curve.clone()], "pr": (lo + hi) * 0.5}));
    close(&json!((mid[0].as_f64().unwrap() - 1.0).hypot(mid[1].as_f64().unwrap() - 1.0)), 1.0, 1e-2);
    // History: a new radius rebuilds it.
    run(&mut s, "setAttr", json!({"args": [format!("{}.radius", r[1].as_str().unwrap()), 2.0]}));
    close3(&run(&mut s, "pointOnCurve", json!({"args": [curve.clone()], "pr": lo})), [2.0, 0.0, 0.0], 1e-3);
    run(&mut s, "undo", json!({}));
    close3(&run(&mut s, "pointOnCurve", json!({"args": [curve], "pr": lo})), [1.0, 0.0, 0.0], 1e-3);
    // Free form: from the end of one to the start of the other.
    line(&mut s, "c", [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]);
    line(&mut s, "d", [3.0, 1.0, 0.0], [4.0, 1.0, 0.0]);
    let f = run(&mut s, "filletCurve", json!({"args": ["c", "d"], "cir": false, "fb": 0.5, "n": "ff"}));
    close3(&run(&mut s, "pointOnCurve", json!({"args": [f[0].clone()], "pr": 0.0})), [1.0, 0.0, 0.0], 1e-6);
}

#[test]
fn curve_intersect_reports_both_params() {
    let mut s = Session::new();
    line(&mut s, "a", [0.0, 0.0, 0.0], [4.0, 0.0, 0.0]);
    line(&mut s, "b", [1.0, -2.0, 0.0], [1.0, 2.0, 0.0]);
    let t = run(&mut s, "curveIntersect", json!({"args": ["a", "b"]}));
    let n: Vec<f64> = t.as_str().unwrap().split_whitespace().map(|x| x.parse().unwrap()).collect();
    assert!((n[0] - 0.25).abs() < 1e-3 && (n[1] - 0.5).abs() < 1e-3, "{n:?}");
    // Apart in Z, they meet only when flattened along Z.
    line(&mut s, "c", [1.0, -2.0, 3.0], [1.0, 2.0, 3.0]);
    assert_eq!(run(&mut s, "curveIntersect", json!({"args": ["a", "c"]})), "");
    assert!(!run(&mut s, "curveIntersect", json!({"args": ["a", "c"], "ud": true, "d": [0.0, 0.0, 1.0]})).as_str().unwrap().is_empty());
}

#[test]
fn surfaces_intersect() {
    let mut s = Session::new();
    plane(&mut s, "a");
    run(&mut s, "nurbsPlane", json!({"name": "b", "axis": [1, 0, 0], "w": 4.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    let r = run(&mut s, "intersect", json!({"args": ["a", "b"]}));
    assert_eq!(r.as_array().unwrap().len(), 2, "{r}");
    let c = r[0].as_str().unwrap().to_string();
    close(&run(&mut s, "arclen", json!({"args": [c]})), 4.0, 1e-2);
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "intersectSurface");
    run(&mut s, "nurbsPlane", json!({"name": "far", "axis": [0, 1, 0], "p": [0.0, 50.0, 0.0]}));
    assert!(s.run("intersect", json!({"args": ["a", "far"]})).is_err());
}

#[test]
fn curve_on_surface_and_project() {
    let mut s = Session::new();
    plane(&mut s, "p");
    let c = run(&mut s, "curveOnSurface", json!({"args": ["p"], "d": 1, "uv": [[0.25, 0.25], [0.75, 0.75]]}));
    assert!(c.as_str().unwrap().contains("->"), "{c}");
    // On a plane 10 across with U and V 0 to 1: (0.25, 0.25) to (0.75, 0.75).
    close(&run(&mut s, "arclen", json!({"args": [c.clone()]})), 50f64.sqrt(), 1e-6);
    // It rides on the surface.
    run(&mut s, "setAttr", json!({"args": ["p.ty", 3.0]}));
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [c.as_str().unwrap().rsplit("->").next().unwrap()]}));
    close(&bb[1], 3.0, 1e-6);
    run(&mut s, "setAttr", json!({"args": ["p.ty", 0.0]}));
    // A curve above the plane, along Y.
    line(&mut s, "l", [-2.0, 5.0, 1.0], [2.0, 5.0, 1.0]);
    let r = run(&mut s, "projectCurve", json!({"args": ["l", "p"], "d": [0.0, 1.0, 0.0]}));
    assert!(r[0].as_str().unwrap().contains("->"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "projectCurve");
    let name = r[0].as_str().unwrap().rsplit("->").next().unwrap().to_string();
    close(&run(&mut s, "arclen", json!({"args": [name.clone()]})), 4.0, 1e-3);
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [name]}));
    close(&bb[1], 0.0, 1e-6);
    close(&bb[2], 1.0, 1e-3);
}

#[test]
fn trim_keeps_the_side_asked() {
    let mut s = Session::new();
    plane(&mut s, "p");
    close(&json!(mesh_area(&mut s, "p")), 100.0, 1e-3);
    // A square loop over the middle quarter of each side.
    let c = run(&mut s, "curveOnSurface", json!({"args": ["p"], "d": 1, "uv": [[0.25, 0.25], [0.75, 0.25], [0.75, 0.75], [0.25, 0.75], [0.25, 0.25]]}));
    let curve = c.as_str().unwrap().to_string();
    // The locator in the middle keeps the inside: 5 by 5.
    let r = run(&mut s, "trim", json!({"args": ["p", curve.clone()], "lu": 0.5, "lv": 0.5}));
    let id = s.graph.find("pShape").unwrap().unwrap();
    assert_eq!(aztek_scene::surfaces::world_surface(&mut s.graph, id).unwrap().trim.len(), 1);
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "trim");
    close(&json!(mesh_area(&mut s, "p")), 25.0, 0.2);
    // Saved with the surface, in both formats.
    for ext in ["ma", "aztek"] {
        let path = temp(&format!("trim.{ext}"));
        run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
        let mut t = Session::new();
        run(&mut t, "file", json!({"args": [path], "open": true}));
        close(&json!(mesh_area(&mut t, "p")), 25.0, 0.2);
    }
    run(&mut s, "undo", json!({}));
    close(&json!(mesh_area(&mut s, "p")), 100.0, 1e-3);
    // -sl 1 throws that region away: what's left is the frame.
    run(&mut s, "trim", json!({"args": ["p", curve.clone()], "lu": 0.5, "lv": 0.5, "sl": 1}));
    close(&json!(mesh_area(&mut s, "p")), 75.0, 0.2);
    let u = run(&mut s, "untrim", json!({"args": ["p"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [u[1].clone()]})), "untrim");
    close(&json!(mesh_area(&mut s, "p")), 100.0, 1e-3);
    // A locator outside the loop keeps the outside.
    run(&mut s, "trim", json!({"args": ["p", curve], "lu": 0.1, "lv": 0.1}));
    close(&json!(mesh_area(&mut s, "p")), 75.0, 0.2);
}

#[test]
fn planar_fills_any_outline() {
    let mut s = Session::new();
    run(&mut s, "circle", json!({"name": "c", "nr": [0.0, 1.0, 0.0], "r": 2.0, "s": 16, "ch": false}));
    let r = run(&mut s, "planar", json!({"args": ["c"]}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": [r[0].clone()], "s": true, "typ": "nurbsSurface"})).as_array().unwrap().len(), 1);
    let area = mesh_area(&mut s, r[0].as_str().unwrap());
    close(&json!(area), std::f64::consts::PI * 4.0, 0.15);
    // The alias runs the same.
    let again = run(&mut s, "planarSrf", json!({"args": ["c"]}));
    close(&json!(mesh_area(&mut s, again[0].as_str().unwrap())), std::f64::consts::PI * 4.0, 0.15);
    // A rectangle stays an untrimmed patch.
    run(&mut s, "curve", json!({"name": "q", "d": 1, "p": [[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [2.0, 0.0, 2.0], [0.0, 0.0, 2.0], [0.0, 0.0, 0.0]]}));
    let q = run(&mut s, "planar", json!({"args": ["q"]}));
    close(&json!(mesh_area(&mut s, q[0].as_str().unwrap())), 4.0, 1e-6);
}

#[test]
fn extend_and_duplicate_surface() {
    let mut s = Session::new();
    plane(&mut s, "p");
    let r = run(&mut s, "extendSurface", json!({"args": ["p"], "d": 2.0, "ed": 0, "es": 1}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "extendSurface");
    close(&json!(mesh_area(&mut s, "p")), 120.0, 1e-3);
    run(&mut s, "undo", json!({}));
    close(&json!(mesh_area(&mut s, "p")), 100.0, 1e-3);
    run(&mut s, "extendSurface", json!({"args": ["p"], "d": 1.0, "ed": 2, "es": 2}));
    close(&json!(mesh_area(&mut s, "p")), 144.0, 1e-3);
    run(&mut s, "undo", json!({}));
    run(&mut s, "nurbsPlane", json!({"name": "b", "axis": [0, 1, 0], "w": 4.0, "lr": 1.0, "u": 4, "v": 4, "d": 1}));
    let d = run(&mut s, "duplicateSurface", json!({"args": ["b.sf[0][0]"], "ch": false}));
    close(&json!(mesh_area(&mut s, d[0].as_str().unwrap())), 1.0, 1e-3);
    let w = run(&mut s, "duplicateSurface", json!({"args": ["p"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [w[1].clone()]})), "subSurface");
    close(&json!(mesh_area(&mut s, w[0].as_str().unwrap())), 100.0, 1e-3);
}

#[test]
fn raw_surface_and_plane() {
    let mut s = Session::new();
    let r = run(&mut s, "surface", json!({"du": 1, "dv": 1, "ku": [0.0, 1.0], "kv": [0.0, 1.0], "p": [[0.0, 0.0, 0.0], [0.0, 0.0, 1.0], [1.0, 0.0, 0.0], [1.0, 0.0, 1.0]], "n": "raw"}));
    assert_eq!(r, "raw");
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["rawShape"]})), "nurbsSurface");
    close3(&cv_point(&mut s, "raw.cv[1][0]"), [1.0, 0.0, 0.0], 1e-9);
    close(&get(&mut s, "raw.degreeU"), 1.0, 1e-9);
    assert!(s.run("surface", json!({"du": 1, "dv": 1, "ku": [0.0, 1.0], "kv": [0.0, 1.0], "p": [[0.0, 0.0, 0.0]]})).is_err());
    let pl = run(&mut s, "plane", json!({"s": 2.0, "p": [0.0, 3.0, 0.0]}));
    close3(&get(&mut s, &format!("{}.translate", pl.as_str().unwrap())), [0.0, 3.0, 0.0], 1e-9);
}

#[test]
fn harden_and_arcs() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"name": "c", "d": 3, "p": [[0.0, 0.0, 0.0], [1.0, 2.0, 0.0], [2.0, 0.0, 0.0], [3.0, 2.0, 0.0], [4.0, 0.0, 0.0], [5.0, 2.0, 0.0]]}));
    let before = get(&mut s, "cShape.spans");
    let r = run(&mut s, "hardenPointCurve", json!({"args": ["c.cv[2]"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "hardenPointCurve");
    // The CV count stays; the curve now runs through it.
    assert_eq!(get(&mut s, "cShape.degree"), 3);
    let _ = before;
    let (mut best, lo, hi) = (f64::MAX, get(&mut s, "c.minValue").as_f64().unwrap(), get(&mut s, "c.maxValue").as_f64().unwrap());
    for k in 0..=200 {
        let p = run(&mut s, "pointOnCurve", json!({"args": ["c"], "pr": lo + (hi - lo) * k as f64 / 200.0}));
        best = best.min((p[0].as_f64().unwrap() - 2.0).hypot(p[1].as_f64().unwrap()));
    }
    assert!(best < 0.03, "{best}");
    let a = run(&mut s, "threePointArc", json!({"p1": [1.0, 0.0, 0.0], "p2": [0.0, 1.0, 0.0], "p3": [-1.0, 0.0, 0.0]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [a[1].clone()]})), "makeThreePointCircularArc");
    close(&run(&mut s, "arclen", json!({"args": [a[0].clone()]})), std::f64::consts::PI, 2e-2);
    close(&run(&mut s, "threePointArc", json!({"args": [a[1].clone()], "q": true, "queried": ["radius"]})), 1.0, 1e-9);
    close3(&run(&mut s, "threePointArc", json!({"args": [a[1].clone()], "q": true, "queried": ["center"]})), [0.0, 0.0, 0.0], 1e-9);
    let b = run(&mut s, "twoPointArc", json!({"p1": [-1.0, 0.0, 0.0], "p2": [1.0, 0.0, 0.0], "r": 1.0, "dv": [0.0, 0.0, 1.0]}));
    close(&run(&mut s, "arclen", json!({"args": [b[0].clone()]})), std::f64::consts::PI, 2e-2);
}

#[test]
fn point_curve_constraint_pulls_the_curve() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"name": "c", "d": 3, "p": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [4.0, 0.0, 0.0]]}));
    let r = run(&mut s, "pointCurveConstraint", json!({"args": ["c.u[1]"], "p": [1.5, 2.0, 0.0]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "pointCurveConstraint");
    close3(&run(&mut s, "pointOnCurve", json!({"args": ["c"], "pr": 1.0})), [1.5, 2.0, 0.0], 1e-6);
}

#[test]
fn curve_edit_and_curve_points() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"name": "c", "d": 3, "p": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0], [3.0, 0.0, 0.0], [4.0, 0.0, 0.0], [5.0, 0.0, 0.0]]}));
    // Three spans, four edit points.
    assert_eq!(run(&mut s, "ls", json!({"args": ["c.ep[*]"], "flatten": true})).as_array().unwrap().len(), 4);
    run(&mut s, "select", json!({"args": ["c.ep[1]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["c.ep[1]"]));
    run(&mut s, "select", json!({"args": ["c.u[0.5]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["c.u[0.5]"]));
    let at = run(&mut s, "pointPosition", json!({"args": ["c.u[1]"]}));
    close3(&at, run(&mut s, "pointOnCurve", json!({"args": ["c"], "pr": 1.0})).as_array().map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap()]).unwrap(), 1e-9);
    let ep = cv_point(&mut s, "c.ep[1]");
    close3(&ep, [ep[0].as_f64().unwrap(), 0.0, 0.0], 1e-9);
    // Moving an edit point puts the curve through the new place.
    run(&mut s, "xform", json!({"args": ["c.ep[1]"], "ws": true, "t": [ep[0], 3.0, 0.0]}));
    close(&cv_point(&mut s, "c.ep[1]")[1], 3.0, 1e-6);
    run(&mut s, "undo", json!({}));
    close(&cv_point(&mut s, "c.ep[1]")[1], 0.0, 1e-6);
    // Through move too, by the selection.
    run(&mut s, "select", json!({"args": ["c.ep[2]"]}));
    run(&mut s, "moveComponents", json!({"by": [0.0, 1.0, 0.0]}));
    close(&cv_point(&mut s, "c.ep[2]")[1], 1.0, 1e-3);
}

#[test]
fn surface_components() {
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "p", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 2, "v": 2, "d": 3}));
    assert_eq!(run(&mut s, "ls", json!({"args": ["p.ep[*][*]"], "flatten": true})).as_array().unwrap().len(), 9);
    assert_eq!(run(&mut s, "ls", json!({"args": ["p.sf[*][*]"], "flatten": true})).as_array().unwrap().len(), 4);
    run(&mut s, "select", json!({"args": ["p.sf[1][0]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["p.sf[1][0]"]));
    run(&mut s, "select", json!({"args": ["p.u[0.5]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["p.u[0.5]"]));
    run(&mut s, "select", json!({"args": ["p.v[1]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["p.v[1]"]));
    let e = cv_point(&mut s, "p.ep[1][1]");
    close3(&e, [0.0, 0.0, 0.0], 1e-9);
    run(&mut s, "xform", json!({"args": ["p.ep[1][1]"], "ws": true, "t": [0.0, 2.0, 0.0]}));
    close(&cv_point(&mut s, "p.ep[1][1]")[1], 2.0, 1e-5);
}

#[test]
fn stitch() {
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "a", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "nurbsPlane", json!({"name": "b", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 3.0]}));
    // A gap of 1 in X: right edge of a at 1, left edge of b at 2.
    let r = run(&mut s, "stitchSurface", json!({"args": ["a", "b"], "b": 0.5}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "stitchSrf");
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": ["a"]}));
    close(&bb[3], 1.5, 1e-6);
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": ["b"]}));
    close(&bb[0], 1.5, 1e-6);
    run(&mut s, "undo", json!({}));
    close(&run(&mut s, "exactWorldBoundingBox", json!({"args": ["a"]}))[3], 1.0, 1e-6);
    // By points.
    run(&mut s, "stitchSurfacePoints", json!({"args": ["a.cv[1][0]", "b.cv[0][0]"]}));
    let (x, y) = (cv_point(&mut s, "a.cv[1][0]"), cv_point(&mut s, "b.cv[0][0]"));
    close3(&x, [y[0].as_f64().unwrap(), y[1].as_f64().unwrap(), y[2].as_f64().unwrap()], 1e-6);
    close(&x[0], 1.5, 1e-6);
    // Global: edges within reach meet.
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "a", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "nurbsPlane", json!({"name": "b", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 2.2]}));
    assert!(run(&mut s, "globalStitch", json!({"args": ["a", "b"], "ms": 0.1})).as_array().unwrap().is_empty());
    run(&mut s, "globalStitch", json!({"args": ["a", "b"], "ms": 0.5}));
    close(&run(&mut s, "exactWorldBoundingBox", json!({"args": ["a"]}))[3], 1.1, 1e-6);
}

#[test]
fn single_surface_and_smooth_tangent() {
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "a", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "nurbsPlane", json!({"name": "b", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 2.0]}));
    let j = run(&mut s, "makeSingleSurface", json!({"args": ["a", "b"]}));
    close(&json!(mesh_area(&mut s, j[0].as_str().unwrap())), 8.0, 1e-3);
    // Kink at the middle of a three-span row, then smooth it.
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "p", "axis": [0, 1, 0], "w": 4.0, "lr": 1.0, "u": 4, "v": 1, "d": 1}));
    run(&mut s, "move", json!({"args": ["p.cv[1][0:1]", 0.0, 1.0, 0.0], "r": true}));
    let before = cv_point(&mut s, "p.cv[1][0]");
    let r = run(&mut s, "smoothTangentSurface", json!({"args": ["p"], "p": 2.0, "d": 1}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "smoothTangentSrf");
    let after = cv_point(&mut s, "p.cv[1][0]");
    assert!((after[1].as_f64().unwrap() - before[1].as_f64().unwrap()).abs() > 1e-3, "{after}");
}

#[test]
fn nurbs_select() {
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "p", "axis": [0, 1, 0], "u": 2, "v": 2, "d": 1}));
    run(&mut s, "nurbsSelect", json!({"args": ["p"], "bs": true}));
    let border = run(&mut s, "ls", json!({"selection": true, "flatten": true}));
    assert_eq!(border.as_array().unwrap().len(), 8);
    run(&mut s, "nurbsSelect", json!({"args": ["p"], "gs": 1}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true, "flatten": true})).as_array().unwrap().len(), 9);
    run(&mut s, "nurbsSelect", json!({"args": ["p"], "lb": true}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true, "flatten": true})).as_array().unwrap().len(), 3);
}

#[test]
fn text_curves_builds_letters() {
    let mut s = Session::new();
    let r = run(&mut s, "textCurves", json!({"t": "Hi"}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "makeTextCurves");
    let g = r[0].as_str().unwrap().to_string();
    let kids = run(&mut s, "listRelatives", json!({"args": [g.clone()], "c": true}));
    assert_eq!(kids.as_array().unwrap().len(), 2, "{kids}");
    let curves = run(&mut s, "listRelatives", json!({"args": [g], "ad": true, "typ": "nurbsCurve"}));
    assert!(curves.as_array().unwrap().len() >= 3, "{curves}");
    assert!(s.run("textCurves", json!({})).is_err());
}

#[test]
fn bevels() {
    let mut s = Session::new();
    run(&mut s, "circle", json!({"name": "c", "nr": [0.0, 0.0, 1.0], "r": 2.0, "ch": false, "s": 16}));
    let r = run(&mut s, "bevel", json!({"args": ["c"], "w": 0.5, "d": 0.5, "ed": 1.0}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "bevel");
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [r[0].clone()]}));
    // The circle pushed out by the width: 2.5 across at the wall.
    assert!(bb[3].as_f64().unwrap() > 2.3 && bb[3].as_f64().unwrap() < 2.7, "{bb}");
    let p = run(&mut s, "bevel", json!({"args": ["c"], "po": 1, "ch": false, "name": "polyBevel"}));
    assert!(run(&mut s, "polyEvaluate", json!({"args": [p[0].clone()], "f": true})).as_i64().unwrap() > 0);
    // bevelPlus caps a closed curve; the default output is polygons.
    let q = run(&mut s, "bevelPlus", json!({"args": ["c"], "w": 0.1, "d": 0.1, "ed": 1.0}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [q[1].clone()]})), "bevelPlus");
    let faces = run(&mut s, "polyEvaluate", json!({"args": [q[0].clone()], "f": true})).as_i64().unwrap();
    let no_cap = run(&mut s, "bevelPlus", json!({"args": ["c"], "cap": 1, "name": "nc"}));
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": [no_cap[0].clone()], "f": true})).as_i64().unwrap() + 2, faces);
    let n = run(&mut s, "bevelPlus", json!({"args": ["c"], "po": 0}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": [n[0].clone()], "c": true})).as_array().unwrap().len(), 3);
}

#[test]
fn surface_fillets() {
    let mut s = Session::new();
    // A floor and a wall meeting at a corner.
    run(&mut s, "nurbsPlane", json!({"name": "floor", "axis": [0, 1, 0], "w": 8.0, "lr": 1.0, "u": 2, "v": 2, "d": 1}));
    run(&mut s, "nurbsPlane", json!({"name": "wall", "axis": [1, 0, 0], "w": 8.0, "lr": 1.0, "u": 2, "v": 2, "d": 1}));
    let (nf, nw) = (run(&mut s, "pointOnSurface", json!({"args": ["floor"], "u": 0.5, "v": 0.5, "nn": true})), run(&mut s, "pointOnSurface", json!({"args": ["wall"], "u": 0.5, "v": 0.5, "nn": true})));
    let _ = (nf, nw);
    let r = run(&mut s, "circularFillet", json!({"args": ["floor", "wall"], "r": 1.0}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "rbfSrf");
    // A section's arc leaves each surface at the radius from the corner.
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [r[0].clone()]}));
    close(&json!(bb[3].as_f64().unwrap().abs().max(bb[0].as_f64().unwrap().abs())), 1.0, 0.05);
    // A blend between two edges with their surfaces' tangents.
    let mut s = Session::new();
    run(&mut s, "nurbsPlane", json!({"name": "a", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "nurbsPlane", json!({"name": "b", "axis": [0, 1, 0], "w": 2.0, "lr": 1.0, "u": 1, "v": 1, "d": 1}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 6.0]}));
    let u1 = get(&mut s, "a.maxValueU").as_f64().unwrap();
    let f = run(&mut s, "freeFormFillet", json!({"args": [format!("a.u[{u1}]"), "b.u[0]"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [f[1].clone()]})), "ffFilletSrf");
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": [f[0].clone()]}));
    close(&bb[0], 1.0, 1e-3);
    close(&bb[3], 5.0, 1e-3);
}

#[test]
fn project_tangent_follows() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"name": "c", "d": 2, "p": [[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0]]}));
    line(&mut s, "t", [2.0, -1.0, 0.0], [2.0, 1.0, 0.0]);
    let r = run(&mut s, "projectTangent", json!({"args": ["c", "t"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": [r[1].clone()]})), "projectTangent");
    // The next-to-last CV now lies on the tangent line through the end, so the curve's end runs along it.
    let p = cv_point(&mut s, "c.cv[1]");
    close(&p[0], 2.0, 1e-6);
}

#[test]
fn nurbs_boolean_gives_a_mesh() {
    let mut s = Session::new();
    run(&mut s, "sphere", json!({"name": "a", "r": 1.0, "ch": false}));
    run(&mut s, "sphere", json!({"name": "b", "r": 1.0, "ch": false}));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 1.0]}));
    let r = run(&mut s, "nurbsBoolean", json!({"args": ["a", "b"], "op": 0}));
    let name = r.get(0).and_then(Value::as_str).or(r.as_str()).unwrap().to_string();
    assert!(run(&mut s, "polyEvaluate", json!({"args": [name], "f": true})).as_i64().unwrap() > 0);
}
