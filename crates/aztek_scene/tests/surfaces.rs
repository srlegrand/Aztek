//! NURBS surfaces: primitives, CVs as components, history, edits, tessellation, saving.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn num(v: Value) -> f64 {
    v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))
}

fn attr(s: &mut Session, plug: &str) -> f64 {
    num(run(s, "getAttr", json!({"args": [plug]})))
}

fn point(s: &mut Session, name: &str, u: f64, v: f64) -> [f64; 3] {
    let p = run(s, "pointOnSurface", json!({"args": [name], "parameterU": u, "parameterV": v}));
    [p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap()]
}

fn near(a: [f64; 3], b: [f64; 3], tol: f64) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < tol)
}

fn cvs(s: &mut Session, name: &str) -> usize {
    run(s, "ls", json!({"args": [format!("{name}.cv[*][*]")], "flatten": true})).as_array().unwrap().len()
}

fn faces(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn y_sphere(s: &mut Session) -> Value {
    run(s, "sphere", json!({"axis": [0, 1, 0]}))
}

#[test]
fn sphere_defaults_as_the_reference() {
    let mut s = Session::new();
    assert_eq!(y_sphere(&mut s), json!(["nurbsSphere1", "makeNurbSphere1"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsSurface"})), json!(["nurbsSphereShape1"]));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["nurbsSphere1"]));
    assert_eq!(attr(&mut s, "nurbsSphereShape1.spansU"), 4.0);
    assert_eq!(attr(&mut s, "nurbsSphereShape1.spansV"), 8.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.degreeU"), 3.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.formU"), 0.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.formV"), 2.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.maxValueU"), 4.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.maxValueV"), 8.0);
    assert_eq!(cvs(&mut s, "nurbsSphere1"), 56);
    assert!(near(point(&mut s, "nurbsSphere1", 0.0, 0.0), [0.0, -1.0, 0.0], 1e-9));
    assert!(near(point(&mut s, "nurbsSphere1", 4.0, 3.0), [0.0, 1.0, 0.0], 1e-9));
    assert!(near(point(&mut s, "nurbsSphere1", 2.0, 0.0), [1.0, 0.0, 0.0], 0.01));
    // The bare command's axis is X, as the node's default.
    run(&mut s, "sphere", json!({"name": "x"}));
    assert!(near(point(&mut s, "x", 0.0, 0.0), [1.0, 0.0, 0.0], 1e-9) || near(point(&mut s, "x", 0.0, 0.0), [-1.0, 0.0, 0.0], 1e-9));
}

#[test]
fn history_edits_rebuild() {
    let mut s = Session::new();
    y_sphere(&mut s);
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.radius", 3.0]}));
    assert!(near(point(&mut s, "nurbsSphere1", 0.0, 0.0), [0.0, -3.0, 0.0], 1e-9));
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.sections", 12]}));
    assert_eq!(attr(&mut s, "nurbsSphere1.spansV"), 12.0);
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.spans", 6]}));
    assert_eq!(cvs(&mut s, "nurbsSphere1"), 9 * 12);
    run(&mut s, "delete", json!({"args": ["nurbsSphere1"], "constructionHistory": true}));
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbSphere"})), json!([]));
    assert!(near(point(&mut s, "nurbsSphere1", 0.0, 0.0), [0.0, -3.0, 0.0], 1e-9));
    // No history asked for.
    assert_eq!(run(&mut s, "sphere", json!({"constructionHistory": false})), json!(["nurbsSphere2"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbSphere"})), json!([]));
    // Deleting the object takes its make node.
    y_sphere(&mut s);
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbSphere"})).as_array().unwrap().len(), 1);
    run(&mut s, "delete", json!({"args": ["nurbsSphere3"]}));
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbSphere"})), json!([]));
}

#[test]
fn other_primitives() {
    let mut s = Session::new();
    let r = run(&mut s, "cylinder", json!({"axis": [0, 1, 0], "radius": 2.0, "heightRatio": 3.0}));
    assert_eq!(r, json!(["nurbsCylinder1", "makeNurbCylinder1"]));
    assert_eq!(cvs(&mut s, "nurbsCylinder1"), 4 * 8);
    assert!((point(&mut s, "nurbsCylinder1", 0.0, 0.0)[1] + 3.0).abs() < 1e-9);
    assert!((point(&mut s, "nurbsCylinder1", 1.0, 0.0)[1] - 3.0).abs() < 1e-9);
    assert!(near(point(&mut s, "nurbsCylinder1", 0.5, 0.0), [2.0, 0.0, 0.0], 1e-9));
    run(&mut s, "setAttr", json!({"args": ["makeNurbCylinder1.heightRatio", 1.0]}));
    assert!((point(&mut s, "nurbsCylinder1", 1.0, 0.0)[1] - 1.0).abs() < 1e-9);

    assert_eq!(run(&mut s, "cone", json!({"axis": [0, 1, 0]})), json!(["nurbsCone1", "makeNurbCone1"]));
    assert!(near(point(&mut s, "nurbsCone1", 1.0, 2.0), [0.0, 1.0, 0.0], 1e-9));
    assert!(near(point(&mut s, "nurbsCone1", 0.0, 0.0), [1.0, -1.0, 0.0], 1e-9));

    assert_eq!(run(&mut s, "torus", json!({"axis": [0, 1, 0]})), json!(["nurbsTorus1", "makeNurbTorus1"]));
    assert_eq!(attr(&mut s, "nurbsTorus1.formU"), 2.0);
    assert_eq!(attr(&mut s, "nurbsTorus1.formV"), 2.0);
    assert_eq!(cvs(&mut s, "nurbsTorus1"), 4 * 8);
    assert!(near(point(&mut s, "nurbsTorus1", 0.0, 0.0), [1.5, 0.0, 0.0], 0.01));

    let r = run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "width": 4.0, "lengthRatio": 0.5, "patchesU": 2, "patchesV": 3}));
    assert_eq!(r, json!(["nurbsPlane1", "makeNurbPlane1"]));
    assert_eq!(cvs(&mut s, "nurbsPlane1"), 5 * 6);
    assert!(near(point(&mut s, "nurbsPlane1", 0.0, 0.0), [-2.0, 0.0, 1.0], 1e-9));
    assert!(near(point(&mut s, "nurbsPlane1", 2.0, 3.0), [2.0, 0.0, -1.0], 1e-9));
    let n = run(&mut s, "pointOnSurface", json!({"args": ["nurbsPlane1"], "parameterU": 1.0, "parameterV": 1.0, "normalizedNormal": true}));
    assert_eq!(n, json!([0.0, 1.0, 0.0]));
    // The percentage flag covers the range.
    let p = run(&mut s, "pointOnSurface", json!({"args": ["nurbsPlane1"], "parameterU": 1.0, "parameterV": 1.0, "turnOnPercentage": true}));
    assert_eq!(p, json!([2.0, 0.0, -1.0]));

    assert_eq!(run(&mut s, "nurbsCube", json!({"axis": [0, 1, 0], "width": 2.0})), json!(["nurbsCube1", "makeNurbCube1"]));
    let shapes = run(&mut s, "ls", json!({"type": "nurbsSurface"}));
    assert_eq!(shapes.as_array().unwrap().iter().filter(|n| n.as_str().unwrap().contains("nurbsCubeShape")).count(), 6);
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["nurbsCube1"]})).as_array().unwrap().len(), 6);
    assert!((point(&mut s, "topnurbsCube1", 0.5, 0.5)[1] - 1.0).abs() < 1e-9);
    assert!((point(&mut s, "bottomnurbsCube1", 0.5, 0.5)[1] + 1.0).abs() < 1e-9);
    assert!((point(&mut s, "rightnurbsCube1", 0.5, 0.5)[0] - 1.0).abs() < 1e-9);
    for (face, out) in [("top", [0.0, 1.0, 0.0]), ("bottom", [0.0, -1.0, 0.0]), ("left", [-1.0, 0.0, 0.0]), ("right", [1.0, 0.0, 0.0]), ("front", [0.0, 0.0, 1.0]), ("back", [0.0, 0.0, -1.0])] {
        let n = run(&mut s, "pointOnSurface", json!({"args": [format!("{face}nurbsCube1")], "parameterU": 0.5, "parameterV": 0.5, "normalizedNormal": true}));
        let got = [n[0].as_f64().unwrap(), n[1].as_f64().unwrap(), n[2].as_f64().unwrap()];
        assert!(near(got, out, 1e-9), "{face} {got:?}");
    }
    run(&mut s, "setAttr", json!({"args": ["makeNurbCube1.width", 4.0]}));
    assert!((point(&mut s, "topnurbsCube1", 0.5, 0.5)[1] - 2.0).abs() < 1e-9);
    // With -po 1 the primitive is a mesh, its history live.
    let r = run(&mut s, "sphere", json!({"polygon": 1, "name": "polySph"}));
    assert_eq!(r[0], json!("polySph"));
    assert_eq!(faces(&mut s, "polySph"), 12 * 24);
}

#[test]
fn cvs_select_move_and_undo() {
    let mut s = Session::new();
    y_sphere(&mut s);
    let flat = run(&mut s, "ls", json!({"args": ["nurbsSphere1.cv[1][0:2]"], "flatten": true}));
    assert_eq!(flat, json!(["nurbsSphere1.cv[1][0]", "nurbsSphere1.cv[1][1]", "nurbsSphere1.cv[1][2]"]));
    run(&mut s, "select", json!({"args": ["nurbsSphere1.cv[3][2]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["nurbsSphere1.cv[3][2]"]));
    let q = |s: &mut Session| {
        let p = run(s, "xform", json!({"args": ["nurbsSphere1.cv[3][2]"], "query": true, "worldSpace": true, "translation": true}));
        [p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap()]
    };
    let before = q(&mut s);
    run(&mut s, "moveComponents", json!({"by": [0, 5, 0]}));
    let moved = q(&mut s);
    assert!((moved[1] - before[1] - 5.0).abs() < 1e-9, "{moved:?} {before:?}");
    assert!(s.undo());
    assert!(near(q(&mut s), before, 1e-9));
    assert!(s.redo());
    assert!(near(q(&mut s), moved, 1e-9));
    // Tweaks sit on top of history: a radius change keeps the offset.
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.radius", 2.0]}));
    let scaled = q(&mut s);
    assert!((scaled[1] - before[1] * 2.0 - 5.0).abs() < 1e-9, "{scaled:?}");
    // Ranges fold, and the seam's wrapped CVs follow the first lap.
    run(&mut s, "select", json!({"args": ["nurbsSphere1.cv[2:3][1:4]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["nurbsSphere1.cv[2:3][1:4]"]));
    let a = point(&mut s, "nurbsSphere1", 2.0, 8.0);
    let b = point(&mut s, "nurbsSphere1", 2.0, 0.0);
    assert!(near(a, b, 1e-9));
    run(&mut s, "xform", json!({"args": ["nurbsSphere1.cv[2][0]"], "relative": true, "translation": [0, 0, 1]}));
    let a = point(&mut s, "nurbsSphere1", 2.0, 8.0);
    let b = point(&mut s, "nurbsSphere1", 2.0, 0.0);
    assert!(near(a, b, 1e-9));
    assert!(s.run("select", json!({"args": ["nurbsSphere1.vtx[0]"]})).is_err());
    // Moving the transform moves the CVs in the world.
    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "name": "pl"}));
    run(&mut s, "move", json!({"args": ["pl"], "to": [10, 0, 0]}));
    let p = run(&mut s, "xform", json!({"args": ["pl.cv[0][0]"], "query": true, "worldSpace": true, "translation": true}));
    assert!((p[0].as_f64().unwrap() - 9.5).abs() < 1e-9, "{p}");
    run(&mut s, "xform", json!({"args": ["pl.cv[0][0]"], "worldSpace": true, "translation": [10.0, 1.0, 0.0]}));
    let p = run(&mut s, "xform", json!({"args": ["pl.cv[0][0]"], "query": true, "translation": true}));
    assert_eq!(p, json!([0.0, 1.0, 0.0]));
}

#[test]
fn select_type_switches_to_cvs() {
    let mut s = Session::new();
    y_sphere(&mut s);
    run(&mut s, "selectType", json!({"controlVertex": true}));
    assert_eq!(run(&mut s, "selectType", json!({"query": true, "controlVertex": true})), json!(true));
    assert_eq!(run(&mut s, "selectMode", json!({})), json!("vertex"));
}

fn lines(s: &mut Session) {
    run(s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [1, 0, 0]], "name": "c1"}));
    run(s, "curve", json!({"degree": 1, "point": [[0, 1, 0], [1, 1, 0]], "name": "c2"}));
    run(s, "curve", json!({"degree": 1, "point": [[0, 2, 0], [1, 2, 0]], "name": "c3"}));
}

#[test]
fn loft_shape_and_options() {
    let mut s = Session::new();
    lines(&mut s);
    assert_eq!(run(&mut s, "loft", json!({"args": ["c1", "c2", "c3"]})), json!(["loftedSurface1", "loft1"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsSurface"})), json!(["loftedSurfaceShape1"]));
    assert_eq!(attr(&mut s, "loftedSurface1.degreeU"), 3.0);
    assert_eq!(attr(&mut s, "loftedSurface1.degreeV"), 1.0);
    assert_eq!(attr(&mut s, "loftedSurface1.spansU"), 2.0);
    assert_eq!(cvs(&mut s, "loftedSurface1"), 5 * 2);
    // Through each curve, at its parameter.
    assert!(near(point(&mut s, "loftedSurface1", 0.0, 0.0), [0.0, 0.0, 0.0], 1e-9));
    assert!(near(point(&mut s, "loftedSurface1", 1.0, 1.0), [1.0, 1.0, 0.0], 1e-9));
    assert!(near(point(&mut s, "loftedSurface1", 2.0, 1.0), [1.0, 2.0, 0.0], 1e-9));
    // Live: it follows a curve.
    run(&mut s, "xform", json!({"args": ["c3.cv[1]"], "worldSpace": true, "translation": [1, 5, 0]}));
    assert!(near(point(&mut s, "loftedSurface1", 2.0, 1.0), [1.0, 5.0, 0.0], 1e-9));
    run(&mut s, "setAttr", json!({"args": ["loft1.sectionSpans", 2]}));
    assert_eq!(attr(&mut s, "loftedSurface1.spansU"), 4.0);

    // Two curves are linear across; a curve running backward is turned round.
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [1, 0, 0]], "name": "a"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[1, 0, 2], [0, 0, 2]], "name": "b"}));
    run(&mut s, "loft", json!({"args": ["a", "b"], "name": "two"}));
    assert_eq!(attr(&mut s, "two.degreeU"), 1.0);
    let far = point(&mut s, "two", 1.0, 0.0);
    assert!(near(far, [0.0, 0.0, 2.0], 1e-9), "{far:?}");
    run(&mut s, "loft", json!({"args": ["a", "b"], "autoReverse": false, "name": "crossed"}));
    assert!(near(point(&mut s, "crossed", 1.0, 0.0), [1.0, 0.0, 2.0], 1e-9));

    // Closed, across curves of unlike knots.
    run(&mut s, "circle", json!({"normal": [0, 1, 0], "name": "r0"}));
    run(&mut s, "circle", json!({"normal": [0, 1, 0], "center": [0, 1, 0], "sections": 8, "name": "r1"}));
    run(&mut s, "circle", json!({"normal": [0, 1, 0], "center": [0, 2, 0], "sections": 8, "name": "r2"}));
    run(&mut s, "loft", json!({"args": ["r0", "r1", "r2"], "close": true, "name": "ring"}));
    assert_eq!(attr(&mut s, "ring.formU"), 2.0);
    assert_eq!(attr(&mut s, "ring.formV"), 2.0);
    run(&mut s, "curve", json!({"degree": 3, "point": [[0, 0, 0], [1, 0, 1], [2, 0, 1], [3, 0, 0]], "name": "k0"}));
    run(&mut s, "curve", json!({"degree": 2, "point": [[0, 1, 0], [1, 1, 1], [2, 1, 1], [3, 1, 0]], "name": "k1"}));
    run(&mut s, "loft", json!({"args": ["k0", "k1"], "name": "mixed"}));
    assert_eq!(attr(&mut s, "mixed.degreeV"), 3.0);
    let (lo, hi) = (attr(&mut s, "mixed.minValueV"), attr(&mut s, "mixed.maxValueV"));
    assert!(near(point(&mut s, "mixed", 1.0, hi), [3.0, 1.0, 0.0], 1e-6));
    assert!(near(point(&mut s, "mixed", 0.0, lo), [0.0, 0.0, 0.0], 1e-6));
    // Polygons on request.
    run(&mut s, "loft", json!({"args": ["c1", "c2"], "polygon": 1, "name": "polyLoft"}));
    assert_eq!(faces(&mut s, "polyLoft"), 1);
}

#[test]
fn revolve_extrude_planar() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"degree": 1, "point": [[1, 0, 0], [1, 2, 0]], "name": "edge"}));
    assert_eq!(run(&mut s, "revolve", json!({"args": ["edge"], "sections": 12})), json!(["revolvedSurface1", "revolve1"]));
    assert_eq!(attr(&mut s, "revolvedSurface1.formV"), 2.0);
    assert_eq!(attr(&mut s, "revolvedSurface1.spansV"), 12.0);
    assert_eq!(cvs(&mut s, "revolvedSurface1"), 2 * 12);
    assert!(near(point(&mut s, "revolvedSurface1", 1.0, 0.0), [1.0, 2.0, 0.0], 0.001));
    run(&mut s, "setAttr", json!({"args": ["revolve1.sections", 6]}));
    assert_eq!(attr(&mut s, "revolvedSurface1.spansV"), 6.0);
    run(&mut s, "revolve", json!({"args": ["edge"], "startSweep": 0.0, "endSweep": 180.0, "sections": 4, "name": "half"}));
    assert_eq!(attr(&mut s, "half.formV"), 0.0);
    assert!(near(point(&mut s, "half", 0.0, 4.0), [-1.0, 0.0, 0.0], 1e-9));
    assert!(near(point(&mut s, "half", 0.0, 2.0), [0.0, 0.0, 1.0], 1e-9));

    run(&mut s, "circle", json!({"normal": [0, 1, 0], "radius": 0.5, "name": "profile"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [0, 4, 0]], "name": "path"}));
    assert_eq!(run(&mut s, "extrude", json!({"args": ["profile", "path"]})), json!(["extrudedSurface1", "extrude1"]));
    assert_eq!(attr(&mut s, "extrudedSurface1.formU"), 2.0);
    assert_eq!(attr(&mut s, "extrudedSurface1.degreeV"), 1.0);
    assert!((point(&mut s, "extrudedSurface1", 0.0, 1.0)[1] - 4.0).abs() < 1e-9);
    run(&mut s, "xform", json!({"args": ["path.cv[1]"], "worldSpace": true, "translation": [0, 6, 0]}));
    assert!((point(&mut s, "extrudedSurface1", 0.0, 1.0)[1] - 6.0).abs() < 1e-9);
    // Scale at the end of the path.
    run(&mut s, "extrude", json!({"args": ["profile", "path"], "scale": 2.0, "name": "flared"}));
    let (a, b) = (point(&mut s, "flared", 0.0, 0.0), point(&mut s, "flared", 0.0, 1.0));
    assert!((b[0] - a[0] * 2.0).abs() < 1e-6, "{a:?} {b:?}");
    // No path: a direction and a length.
    run(&mut s, "extrude", json!({"args": ["profile"], "extrudeType": 0, "direction": [1, 0, 0], "length": 3.0, "name": "straight"}));
    let (a, b) = (point(&mut s, "straight", 0.0, 0.0), point(&mut s, "straight", 0.0, 1.0));
    assert!((b[0] - a[0] - 3.0).abs() < 1e-9);
    // A curved path turns a tube: the profile stays square to it.
    run(&mut s, "curve", json!({"degree": 3, "point": [[0, 0, 0], [0, 2, 0], [0, 4, 2], [0, 4, 4]], "name": "bend"}));
    run(&mut s, "extrude", json!({"args": ["profile", "bend"], "name": "tube", "extrudeType": 2, "fixedPath": true}));
    let (a, b) = (point(&mut s, "tube", 0.0, 1.0), point(&mut s, "tube", 2.0, 1.0));
    assert!((a[2] - 4.0).abs() < 1e-3 && (b[2] - 4.0).abs() < 1e-3, "{a:?} {b:?}");

    // planar: a plain patch for a rectangle, a trimmed plane otherwise, polygons with -po 1.
    run(&mut s, "nurbsSquare", json!({"name": "sq"}));
    assert_eq!(run(&mut s, "planar", json!({"args": ["sq"]})), json!(["planarTrimmedSurface1", "planarTrim1"]));
    assert_eq!(attr(&mut s, "planarTrimmedSurface1.degreeU"), 1.0);
    assert_eq!(cvs(&mut s, "planarTrimmedSurface1"), 4);
    run(&mut s, "planar", json!({"args": ["profile"], "name": "disc"}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["disc"], "s": true, "typ": "nurbsSurface"})), json!(["discShape"]));
    run(&mut s, "planar", json!({"args": ["sq"], "polygon": 1, "name": "polyQuad"}));
    assert_eq!(faces(&mut s, "polyQuad"), 1);
}

#[test]
fn boundary_square_birail() {
    let mut s = Session::new();
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [2, 0, 0]], "name": "a"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[2, 0, 0], [2, 0, 2]], "name": "b"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[2, 0, 2], [0, 0, 2]], "name": "c"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 2], [0, 0, 0]], "name": "e"}));
    assert_eq!(run(&mut s, "boundary", json!({"args": ["a", "b", "c", "e"]})), json!(["boundarySurface1", "boundary1"]));
    let (ulo, uhi) = (attr(&mut s, "boundarySurface1.minValueU"), attr(&mut s, "boundarySurface1.maxValueU"));
    assert_eq!((ulo, uhi), (0.0, 1.0));
    assert!(near(point(&mut s, "boundarySurface1", 0.5, 0.5), [1.0, 0.0, 1.0], 1e-9));
    assert!(near(point(&mut s, "boundarySurface1", 1.0, 1.0), [2.0, 0.0, 2.0], 1e-9));
    assert_eq!(run(&mut s, "square", json!({"args": ["a", "b", "c", "e"]})), json!(["squareSurface1", "squareSrf1"]));
    run(&mut s, "boundary", json!({"args": ["a", "b", "e"], "name": "tri"}));
    assert!(near(point(&mut s, "tri", 0.0, 0.0), [0.0, 0.0, 0.0], 1e-9));
    assert!(s.run("boundary", json!({"args": ["a", "b"]})).is_err());

    // A profile between two rails, widening as they part.
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [1, 0, 0]], "name": "profile"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [0, 0, 5]], "name": "r0"}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[1, 0, 0], [3, 0, 5]], "name": "r1"}));
    let r = run(&mut s, "singleProfileBirailSurface", json!({"args": ["profile", "r0", "r1"]}));
    assert_eq!(r, json!(["birailSurface1", "singleProfileBirailSurface1"]));
    assert!(near(point(&mut s, "birailSurface1", 1.0, 1.0), [3.0, 0.0, 5.0], 1e-9));
    assert!(near(point(&mut s, "birailSurface1", 0.5, 1.0), [1.5, 0.0, 5.0], 1e-9));
}

#[test]
fn edits_keep_history() {
    let mut s = Session::new();
    y_sphere(&mut s);
    let r = run(&mut s, "rebuildSurface", json!({"args": ["nurbsSphere1"], "spansU": 6, "spansV": 12}));
    assert_eq!(r, json!(["nurbsSphere1", "rebuildSurface1"]));
    assert_eq!(attr(&mut s, "nurbsSphere1.spansU"), 6.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.spansV"), 12.0);
    assert_eq!(attr(&mut s, "nurbsSphere1.formV"), 2.0);
    run(&mut s, "setAttr", json!({"args": ["rebuildSurface1.spansU", 5]}));
    assert_eq!(attr(&mut s, "nurbsSphere1.spansU"), 5.0);
    // History above it still drives.
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.radius", 2.0]}));
    let p = point(&mut s, "nurbsSphere1", 0.0, 0.0);
    assert!((p[1] + 2.0).abs() < 1e-6, "{p:?}");
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbSphere"})).as_array().unwrap().len(), 1);

    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "name": "pl"}));
    let start = point(&mut s, "pl", 0.0, 0.0);
    assert_eq!(run(&mut s, "reverseSurface", json!({"args": ["pl"], "direction": 0})), json!(["pl", "reverseSurface1"]));
    assert!(near(point(&mut s, "pl", 1.0, 0.0), start, 1e-9));
    run(&mut s, "setAttr", json!({"args": ["reverseSurface1.direction", 1]}));
    assert!(near(point(&mut s, "pl", 0.0, 1.0), start, 1e-9));

    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "name": "kn"}));
    let before = point(&mut s, "kn", 0.3, 0.6);
    let r = run(&mut s, "insertKnotSurface", json!({"args": ["kn"], "parameter": [0.5], "direction": 1}));
    assert_eq!(r, json!(["kn", "insertKnotSurface1"]));
    assert_eq!(cvs(&mut s, "kn"), 5 * 4);
    assert!(near(point(&mut s, "kn", 0.3, 0.6), before, 1e-9));
    run(&mut s, "insertKnotSurface", json!({"args": ["kn"], "parameter": [0.5], "direction": 0, "constructionHistory": false}));
    assert_eq!(cvs(&mut s, "kn"), 5 * 5);
    assert_eq!(run(&mut s, "ls", json!({"type": "insertKnotSurface"})), json!([]));

    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "width": 2.0, "name": "cut"}));
    let r = run(&mut s, "detachSurface", json!({"args": ["cut"], "parameter": [0.5], "direction": 1}));
    assert_eq!(r.as_array().unwrap().len(), 3, "{r}");
    assert_eq!(r[0], json!("cut"));
    assert!(near(point(&mut s, "cut", 0.5, 0.0), [0.0, 0.0, 1.0], 1e-9));
    let second = r[1].as_str().unwrap().to_string();
    assert!(near(point(&mut s, &second, 1.0, 0.0), [1.0, 0.0, 1.0], 1e-9));
    assert_eq!(attr(&mut s, &format!("{second}.minValueU")), 0.5);
    let joined = run(&mut s, "attachSurface", json!({"args": ["cut", second]}));
    assert_eq!(joined, json!(["cut", "attachSurface1"]));
    assert_eq!(attr(&mut s, "cut.maxValueU"), 1.0);
    assert!(near(point(&mut s, "cut", 1.0, 0.0), [1.0, 0.0, 1.0], 1e-9));

    run(&mut s, "sphere", json!({"axis": [0, 1, 0], "name": "off"}));
    assert_eq!(run(&mut s, "offsetSurface", json!({"args": ["off"], "distance": 1.0})), json!(["off", "offsetSurface1"]));
    let p = point(&mut s, "off", 2.0, 0.0);
    assert!((p[0] - 2.0).abs() < 0.02, "{p:?}");

    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "patchesU": 4, "name": "shut"}));
    assert_eq!(run(&mut s, "closeSurface", json!({"args": ["shut"], "direction": 0})), json!(["shut", "closeSurface1"]));
    assert_eq!(attr(&mut s, "shut.formU"), 2.0);

    // Without -rpo, a new surface.
    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "name": "orig"}));
    let r = run(&mut s, "reverseSurface", json!({"args": ["orig"], "replaceOriginal": false}));
    assert_ne!(r[0], json!("orig"));
    assert!(near(point(&mut s, "orig", 0.0, 0.0), [-0.5, 0.0, 0.5], 1e-9));
    assert!(near(point(&mut s, r[0].as_str().unwrap(), 1.0, 0.0), [-0.5, 0.0, 0.5], 1e-9));
    assert!(s.run("rebuildSurface", json!({"args": ["makeNurbPlane1"]})).is_err());
}

#[test]
fn nurbs_to_poly_counts() {
    let mut s = Session::new();
    y_sphere(&mut s);
    let r = run(&mut s, "nurbsToPoly", json!({"args": ["nurbsSphere1"], "format": 2, "polygonType": 1, "uType": 3, "uNumber": 3, "vType": 3, "vNumber": 3}));
    assert_eq!(r, json!(["nurbsToPoly1", "nurbsTessellate1"]));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["nurbsToPoly1"]));
    assert_eq!(faces(&mut s, "nurbsToPoly1"), 12 * 24);
    let i = run(&mut s, "meshInfo", json!({"args": ["nurbsToPoly1"]}));
    assert_eq!(i["points"].as_i64().unwrap_or(i["vertices"].as_i64().unwrap_or(0)), 11 * 24 + 2, "{i}");
    // The node's settings are live, and so is the surface.
    run(&mut s, "setAttr", json!({"args": ["nurbsTessellate1.uNumber", 1]}));
    assert_eq!(faces(&mut s, "nurbsToPoly1"), 4 * 24);
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.sections", 4]}));
    assert_eq!(faces(&mut s, "nurbsToPoly1"), 4 * 12);
    // Triangles double the count where the cells are quads; the poles are triangles already.
    run(&mut s, "nurbsPlane", json!({"name": "pl", "patchesU": 2, "patchesV": 2}));
    run(&mut s, "nurbsToPoly", json!({"args": ["pl"], "format": 2, "polygonType": 0, "uNumber": 1, "vNumber": 1, "name": "tris"}));
    assert_eq!(faces(&mut s, "tris"), 8);
    // By count, by CV, and with the standard fit.
    run(&mut s, "nurbsToPoly", json!({"args": ["pl"], "format": 0, "polygonType": 1, "polygonCount": 100, "name": "count"}));
    assert_eq!(faces(&mut s, "count"), 100);
    run(&mut s, "nurbsToPoly", json!({"args": ["pl"], "format": 3, "polygonType": 1, "name": "bycv"}));
    assert_eq!(faces(&mut s, "bycv"), 16);
    run(&mut s, "sphere", json!({"name": "fit"}));
    run(&mut s, "nurbsToPoly", json!({"args": ["fit"], "polygonType": 1, "fractionalTolerance": 0.01, "name": "fine"}));
    run(&mut s, "nurbsToPoly", json!({"args": ["fit"], "polygonType": 1, "fractionalTolerance": 0.2, "name": "coarse"}));
    assert!(faces(&mut s, "fine") > faces(&mut s, "coarse"));
    // A group's surfaces all convert; without history nothing's left.
    run(&mut s, "nurbsCube", json!({"name": "box"}));
    let r = run(&mut s, "nurbsToPoly", json!({"args": ["box"], "format": 2, "polygonType": 1, "constructionHistory": false}));
    assert_eq!(r.as_array().unwrap().len(), 6);
    assert!(s.run("nurbsToPoly", json!({"args": ["tris"]})).is_err());
}

#[test]
fn isoparm_curves_and_info_nodes() {
    let mut s = Session::new();
    y_sphere(&mut s);
    let r = run(&mut s, "duplicateCurve", json!({"args": ["nurbsSphere1.u[2]"]}));
    assert_eq!(r, json!(["curve1", "curveFromSurfaceIso1"]));
    assert_eq!(attr(&mut s, "curve1.form"), 2.0);
    let len = num(run(&mut s, "arclen", json!({"args": ["curve1"]})));
    assert!((len - std::f64::consts::TAU).abs() < 0.05, "{len}");
    // It follows the surface.
    run(&mut s, "setAttr", json!({"args": ["makeNurbSphere1.radius", 2.0]}));
    let len = num(run(&mut s, "arclen", json!({"args": ["curve1"]})));
    assert!((len - std::f64::consts::TAU * 2.0).abs() < 0.1, "{len}");
    run(&mut s, "duplicateCurve", json!({"args": ["nurbsSphere1.v[0]"], "constructionHistory": false, "name": "half"}));
    assert_eq!(attr(&mut s, "half.form"), 0.0);
    let len = num(run(&mut s, "arclen", json!({"args": ["half"]})));
    assert!((len - std::f64::consts::TAU).abs() < 0.1, "{len}");
    run(&mut s, "duplicateCurve", json!({"args": ["nurbsSphere1.u[1.5]"], "name": "frac"}));
    assert_eq!(attr(&mut s, "frac.form"), 2.0);
    assert!(s.run("duplicateCurve", json!({"args": ["nurbsSphere1"]})).is_err());

    run(&mut s, "nurbsPlane", json!({"axis": [0, 1, 0], "width": 2.0, "name": "pl"}));
    run(&mut s, "createNode", json!({"args": ["pointOnSurfaceInfo"], "name": "posi"}));
    run(&mut s, "connectAttr", json!({"args": ["plShape.worldSpace[0]", "posi.inputSurface"]}));
    run(&mut s, "setAttr", json!({"args": ["posi.parameterU", 0.5]}));
    run(&mut s, "setAttr", json!({"args": ["posi.parameterV", 0.5]}));
    let close = |v: Value, w: [f64; 3]| (0..3).all(|i| (v[i].as_f64().unwrap() - w[i]).abs() < 1e-9);
    assert!(close(run(&mut s, "getAttr", json!({"args": ["posi.position"]})), [0.0, 0.0, 0.0]));
    assert!(close(run(&mut s, "getAttr", json!({"args": ["posi.normalizedNormal"]})), [0.0, 1.0, 0.0]));
    run(&mut s, "move", json!({"args": ["pl"], "to": [0, 3, 0]}));
    assert!(close(run(&mut s, "getAttr", json!({"args": ["posi.position"]})), [0.0, 3.0, 0.0]));
    run(&mut s, "createNode", json!({"args": ["closestPointOnSurface"], "name": "cpos"}));
    run(&mut s, "connectAttr", json!({"args": ["plShape.worldSpace[0]", "cpos.inputSurface"]}));
    run(&mut s, "setAttr", json!({"args": ["cpos.inPosition", 0.5, 9.0, 0.0]}));
    let p = run(&mut s, "getAttr", json!({"args": ["cpos.position"]}));
    assert!((p[0].as_f64().unwrap() - 0.5).abs() < 1e-6 && (p[1].as_f64().unwrap() - 3.0).abs() < 1e-6, "{p}");
    assert!((attr(&mut s, "cpos.parameterU") - 0.75).abs() < 1e-6);
    run(&mut s, "createNode", json!({"args": ["surfaceInfo"], "name": "info"}));
    run(&mut s, "connectAttr", json!({"args": ["plShape.worldSpace[0]", "info.inputSurface"]}));
    assert_eq!(attr(&mut s, "info.degreeU"), 3.0);
    assert_eq!(attr(&mut s, "info.spansV"), 1.0);
}

#[test]
fn display_smoothness_sets_the_shape() {
    let mut s = Session::new();
    y_sphere(&mut s);
    assert_eq!(attr(&mut s, "nurbsSphereShape1.curvePrecision"), 4.0);
    assert_eq!(attr(&mut s, "nurbsSphereShape1.curvePrecisionShaded"), 1.0);
    run(&mut s, "displaySmoothness", json!({"divisionsU": 3, "divisionsV": 3, "pointsWire": 16, "pointsShaded": 4}));
    assert_eq!(attr(&mut s, "nurbsSphereShape1.divisionsU"), 3.0);
    assert_eq!(attr(&mut s, "nurbsSphereShape1.curvePrecision"), 16.0);
    assert_eq!(attr(&mut s, "nurbsSphereShape1.curvePrecisionShaded"), 4.0);
}

#[test]
fn surfaces_save_load_and_duplicate() {
    let mut s = Session::new();
    y_sphere(&mut s);
    run(&mut s, "xform", json!({"args": ["nurbsSphere1.cv[3][2]"], "relative": true, "translation": [0, 2, 0]}));
    run(&mut s, "sphere", json!({"axis": [0, 1, 0], "radius": 2.0, "constructionHistory": false, "name": "plain"}));
    run(&mut s, "xform", json!({"args": ["plain.cv[3][2]"], "relative": true, "translation": [0, 2, 0]}));
    run(&mut s, "nurbsCube", json!({"name": "box", "width": 3.0}));
    s.graph.evaluate();
    let before = point(&mut s, "nurbsSphere1", 3.0, 1.0);
    let plain = point(&mut s, "plain", 3.0, 1.0);
    let doc = aztek_scene::scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    aztek_scene::scene_file::deserialize(&mut t.graph, &doc).unwrap();
    assert_eq!(run(&mut t, "ls", json!({"type": "makeNurbSphere"})), json!(["makeNurbSphere1"]));
    assert!(near(point(&mut t, "nurbsSphere1", 3.0, 1.0), before, 1e-9));
    let loaded = point(&mut t, "plain", 3.0, 1.0);
    assert!(near(loaded, plain, 1e-9), "{loaded:?} {plain:?}");
    assert_eq!(attr(&mut t, "nurbsSphere1.spansU"), 4.0);
    assert_eq!(cvs(&mut t, "nurbsSphere1"), 56);
    // History still works, tweak and all.
    run(&mut t, "setAttr", json!({"args": ["makeNurbSphere1.radius", 2.0]}));
    let p = run(&mut t, "xform", json!({"args": ["nurbsSphere1.cv[3][2]"], "query": true, "translation": true}));
    let h = run(&mut t, "xform", json!({"args": ["nurbsSphere1.cv[3][3]"], "query": true, "translation": true}));
    assert!((p[1].as_f64().unwrap() - h[1].as_f64().unwrap() - 2.0).abs() < 1e-9, "{p} {h}");
    assert_eq!(run(&mut t, "ls", json!({"type": "nurbsSurface"})).as_array().unwrap().len(), 8);

    let d = run(&mut s, "duplicate", json!({"args": ["nurbsSphere1"]}));
    let name = d.as_array().and_then(|a| a.first().cloned()).unwrap_or(d.clone());
    let name = name.as_str().unwrap().to_string();
    assert!(near(point(&mut s, &name, 3.0, 1.0), before, 1e-9));
    assert_eq!(cvs(&mut s, &name), 56);
    assert!(aztek_scene::surfaces::as_surface(&s.graph, s.graph.find(&name).unwrap().unwrap()).is_some());
}

#[test]
fn creating_undoes_in_one_step() {
    let mut s = Session::new();
    y_sphere(&mut s);
    assert!(s.undo());
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsSurface"})), json!([]));
    assert_eq!(run(&mut s, "ls", json!({"type": "makeNurbSphere"})), json!([]));
    assert!(s.redo());
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsSurface"})), json!(["nurbsSphereShape1"]));
    run(&mut s, "nurbsCube", json!({}));
    assert!(s.undo());
    assert_eq!(run(&mut s, "ls", json!({"type": "nurbsSurface"})).as_array().unwrap().len(), 1);
}
