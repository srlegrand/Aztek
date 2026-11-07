//! The polygon commands under their reference names: topology counts, positions, names and returns, editing a node's
//! attribute afterwards, undo.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// (vertices, edges, faces).
fn counts(s: &mut Session, m: &str) -> (i64, i64, i64) {
    let i = run(s, "meshInfo", json!({"args": [m]}));
    (i["points"].as_i64().unwrap(), i["edges"].as_i64().unwrap(), i["faces"].as_i64().unwrap())
}

fn set(s: &mut Session, plug: &str, v: Value) {
    run(s, "setAttr", json!({"args": [plug, v]}));
}

fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}

fn cube(s: &mut Session) {
    run(s, "polyCube", json!({}));
}

/// Y of a vertex in world space.
fn vtx(s: &mut Session, c: &str) -> Vec<f64> {
    run(s, "xform", json!({"args": [c], "query": true, "worldSpace": true, "translation": true}))
        .as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

fn max_y(s: &mut Session, m: &str) -> f64 {
    run(s, "meshInfo", json!({"args": [m]}))["boundsMax"][1].as_f64().unwrap()
}

// ---------------------------------------------------------------------------------------------------------------------
// Primitives

#[test]
fn primitives_answer_to_reference_flags_and_attributes() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "polyCube", json!({"w": 2, "h": 3, "d": 4, "sx": 2, "sy": 3, "sz": 4})), json!(["pCube1", "polyCube1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 2 * (2 * 3 + 3 * 4 + 2 * 4));
    assert_eq!(get(&mut s, "polyCube1.subdivisionsWidth"), json!(2));
    assert_eq!(get(&mut s, "polyCube1.subdivisionsX"), json!(2));
    assert_eq!(get(&mut s, "polyCube1.sx"), json!(2));
    set(&mut s, "polyCube1.subdivisionsWidth", json!(1));
    set(&mut s, "polyCube1.sy", json!(1));
    set(&mut s, "polyCube1.sz", json!(1));
    assert_eq!(counts(&mut s, "pCube1"), (8, 12, 6));

    let r = run(&mut s, "polyPlane", json!({"w": 4, "h": 2, "sx": 3, "sy": 2}));
    assert_eq!(r, json!(["pPlane1", "polyPlane1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, 6);
    assert_eq!(get(&mut s, "polyPlane1.height"), json!(2.0));
    assert_eq!(get(&mut s, "polyPlane1.depth"), json!(2.0));
    // The other spelling still works.
    run(&mut s, "polyPlane", json!({"width": 2, "depth": 8, "subdivisionsX": 1, "subdivisionsZ": 2}));
    assert_eq!(counts(&mut s, "pPlane2").2, 2);
    assert_eq!(get(&mut s, "polyPlane2.height"), json!(8.0));

    run(&mut s, "polySphere", json!({"r": 2, "sx": 8, "sy": 6}));
    assert_eq!(counts(&mut s, "pSphere1").2, 48);
    assert_eq!(get(&mut s, "polySphere1.subdivisionsAxis"), json!(8));
    run(&mut s, "polyCylinder", json!({"r": 1, "h": 2, "sx": 8, "sy": 2, "sz": 0}));
    assert_eq!(get(&mut s, "polyCylinder1.subdivisionsHeight"), json!(2));
    run(&mut s, "polyCone", json!({"sx": 8, "sy": 1, "sz": 0}));
    run(&mut s, "polyTorus", json!({"r": 2, "sr": 0.5, "tw": 30, "sx": 8, "sy": 6}));
    assert_eq!(counts(&mut s, "pTorus1").2, 48);
    run(&mut s, "polyPipe", json!({"r": 1, "h": 2, "t": 0.25, "sa": 8, "sh": 1}));

    // -ch 0: just the transform, no node.
    assert_eq!(run(&mut s, "polyCube", json!({"ch": false, "n": "bare"})), json!(["bare"]));
    // -cuv 0: no UVs.
    let info = run(&mut s, "polyCube", json!({"cuv": 0, "n": "noUv"}));
    assert_eq!(info[0], json!("noUv"));

    // -ax: the height runs along the axis.
    run(&mut s, "polyCube", json!({"h": 5, "ax": [1, 0, 0], "n": "lying"}));
    let b = run(&mut s, "meshInfo", json!({"args": ["lying"]}));
    assert!((b["boundsMax"][0].as_f64().unwrap() - 2.5).abs() < 1e-9, "{b}");
}

#[test]
fn new_primitives_have_their_shapes() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "polyPrism", json!({"l": 2, "w": 1, "ns": 5, "sh": 1, "sc": 0}))[0], json!("pPrism1"));
    assert_eq!(counts(&mut s, "pPrism1"), (10, 15, 7));
    assert_eq!(run(&mut s, "polyPyramid", json!({"w": 1, "ns": 4}))[1], json!("polyPyramid1"));
    assert_eq!(counts(&mut s, "pPyramid1"), (5, 8, 5));
    set(&mut s, "polyPyramid1.numberOfSides", json!(3));
    assert_eq!(counts(&mut s, "pPyramid1"), (4, 6, 4));

    for (st, v, e, f) in [(0, 20, 30, 12), (1, 12, 30, 20), (2, 6, 12, 8), (3, 4, 6, 4)] {
        let r = run(&mut s, "polyPlatonicSolid", json!({"st": st, "r": 2}));
        let name = r[0].as_str().unwrap().to_string();
        assert!(name.starts_with("pSolid"), "{name}");
        assert_eq!(counts(&mut s, &name), (v, e, f), "solid {st}");
    }
    let ball = run(&mut s, "polyPrimitive", json!({"pt": 0}));
    assert_eq!(counts(&mut s, ball[0].as_str().unwrap()), (60, 90, 32));
    run(&mut s, "polySoccerBall", json!({}));
    assert_eq!(counts(&mut s, "pSoccerBall1").2, 32);

    run(&mut s, "polyHelix", json!({"c": 2, "h": 4, "w": 2, "r": 0.25, "sa": 6, "sco": 10, "sc": 0}));
    assert_eq!(counts(&mut s, "pHelix1").2, 6 * 20 + 2);
    run(&mut s, "polySuperShape", json!({"r": 1, "hd": 8, "vd": 4}));
    assert_eq!(counts(&mut s, "pSuperShape1").2, 8 * 2 + 8 * 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// Extrude and move

#[test]
fn extrude_facet_flags_nodes_and_edits() {
    let mut s = Session::new();
    cube(&mut s);
    let r = run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[1]"], "ltz": 2.0}));
    assert_eq!(r, json!(["polyExtrudeFace1"]));
    assert_eq!(counts(&mut s, "pCube1"), (12, 20, 10));
    assert!((get(&mut s, "polyExtrudeFace1.localTranslateZ").as_f64().unwrap() - 2.0).abs() < 1e-9);
    assert!((get(&mut s, "polyExtrudeFace1.ltz").as_f64().unwrap() - 2.0).abs() < 1e-9);
    let up = max_y(&mut s, "pCube1");
    assert!((up - 2.5).abs() < 1e-9, "{up}");
    set(&mut s, "polyExtrudeFace1.localTranslateZ", json!(1.0));
    assert!((max_y(&mut s, "pCube1") - 1.5).abs() < 1e-9);
    // Components named in the call leave the selection alone.
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pCube1"]));
    assert!(s.undo());
    assert_eq!(counts(&mut s, "pCube1"), (12, 20, 10));
    assert!(s.undo());
    assert_eq!(counts(&mut s, "pCube1"), (8, 12, 6));
    assert!(run(&mut s, "ls", json!({"type": "polyExtrudeFace"})).as_array().unwrap().is_empty());

    // -ch 0 records no node.
    let r = run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[1]"], "ltz": 1.0, "ch": false}));
    assert!(r.is_null() || r == json!([]), "{r}");
    assert!(run(&mut s, "ls", json!({"type": "polyExtrudeFace"})).as_array().unwrap().is_empty());
}

#[test]
fn extrude_facet_divisions_scale_keep_together_name() {
    let mut s = Session::new();
    cube(&mut s);
    // Three divisions: 4 sides each.
    run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[1]"], "ltz": 3.0, "d": 3, "n": "tall"}));
    assert_eq!(counts(&mut s, "pCube1").2, 6 + 4 * 3);
    assert_eq!(run(&mut s, "ls", json!({"type": "polyExtrudeFace"})), json!(["tall"]));
    assert!((max_y(&mut s, "pCube1") - 3.5).abs() < 1e-9);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    // Two faces apart with -kft 0 extrude alone: every shared edge doubles up.
    run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[0:1]"], "ltz": 1.0, "kft": false}));
    assert_eq!(counts(&mut s, "pCube1").2, 6 + 8);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[0:1]"], "ltz": 1.0}));
    assert_eq!(counts(&mut s, "pCube1").2, 6 + 6);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    // -ls shrinks the cap about its centre.
    run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[1]"], "ltz": 1.0, "ls": [0.5, 0.5, 1.0]}));
    let top = vtx(&mut s, "pCube1.vtx[8]");
    assert!((top[0].abs() - 0.25).abs() < 1e-9, "{top:?}");
    set(&mut s, "polyExtrudeFace1.localScaleX", json!(1.0));
    set(&mut s, "polyExtrudeFace1.localScaleY", json!(1.0));
    let top = vtx(&mut s, "pCube1.vtx[8]");
    assert!((top[0].abs() - 0.5).abs() < 1e-9, "{top:?}");

    // The old spelling reaches the same node.
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "select", json!({"args": ["pCube1.f[1]"]}));
    run(&mut s, "polyExtrude", json!({"thickness": 2.0}));
    assert!((get(&mut s, "polyExtrudeFace1.thickness").as_f64().unwrap() - 2.0).abs() < 1e-9);
    assert!((max_y(&mut s, "pCube1") - 2.5).abs() < 1e-9);
}

#[test]
fn extrude_facet_along_a_curve() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0.5, 0], [0, 4.5, 0]], "name": "path"}));
    run(&mut s, "polyExtrudeFacet", json!({"args": ["pCube1.f[1]"], "inc": "path", "d": 4}));
    assert_eq!(counts(&mut s, "pCube1").2, 6 + 4 * 4);
    assert!((max_y(&mut s, "pCube1") - 4.5).abs() < 1e-6, "{}", max_y(&mut s, "pCube1"));
}

#[test]
fn extrude_edge_and_vertex() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 1, "sy": 1}));
    assert_eq!(run(&mut s, "polyExtrudeEdge", json!({"args": ["pPlane1.e[0]"], "off": 1.0})), json!(["polyExtrudeEdge1"]));
    assert_eq!(counts(&mut s, "pPlane1"), (6, 7, 2));

    cube(&mut s);
    assert_eq!(run(&mut s, "polyExtrudeVertex", json!({"args": ["pCube1.vtx[0]"], "w": 0.25, "l": 0.5})), json!(["polyExtrudeVertex1"]));
    // A corner of three faces: three new points, three triangles on top.
    assert_eq!(counts(&mut s, "pCube1"), (11, 18, 9));
    assert!((get(&mut s, "polyExtrudeVertex1.length").as_f64().unwrap() - 0.5).abs() < 1e-9);
    set(&mut s, "polyExtrudeVertex1.divisions", json!(2));
    // A ring up the sides: three more points, three quads more.
    assert_eq!(counts(&mut s, "pCube1"), (14, 24, 12));
}

#[test]
fn move_commands() {
    let mut s = Session::new();
    cube(&mut s);
    let before = vtx(&mut s, "pCube1.vtx[0]");
    assert_eq!(run(&mut s, "polyMoveVertex", json!({"args": ["pCube1.vtx[0]"], "t": [0, 1, 0]})), json!(["polyMoveVertex1"]));
    let after = vtx(&mut s, "pCube1.vtx[0]");
    assert!((after[1] - before[1] - 1.0).abs() < 1e-9);
    assert!((get(&mut s, "polyMoveVertex1.translateY").as_f64().unwrap() - 1.0).abs() < 1e-9);
    set(&mut s, "polyMoveVertex1.translateY", json!(0.0));
    assert!((vtx(&mut s, "pCube1.vtx[0]")[1] - before[1]).abs() < 1e-9);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyMoveFacet", json!({"args": ["pCube1.f[1]"], "ltz": 2.0})), json!(["polyMoveFace1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 6);
    assert!((max_y(&mut s, "pCube1") - 2.5).abs() < 1e-9);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    // Scaling a face about its centre, the pivot worked out for it.
    run(&mut s, "polyMoveFacet", json!({"args": ["pCube1.f[1]"], "s": [2, 1, 2]}));
    let b = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}));
    assert!((b["boundsMax"][0].as_f64().unwrap() - 1.0).abs() < 1e-9, "{b}");

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyMoveEdge", json!({"args": ["pCube1.e[0]"], "t": [0, 0, 1]})), json!(["polyMoveEdge1"]));
    assert_eq!(counts(&mut s, "pCube1"), (8, 12, 6));
    assert!((run(&mut s, "meshInfo", json!({"args": ["pCube1"]}))["boundsMax"][2].as_f64().unwrap() - 1.5).abs() < 1e-9);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyChipOff", json!({"args": ["pCube1.f[0]"], "ltz": 1.0})), json!(["polyChipOff1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 7);
    set(&mut s, "polyChipOff1.duplicate", json!(false));
    assert_eq!(counts(&mut s, "pCube1").2, 6);
    assert_eq!(counts(&mut s, "pCube1").0, 12);
}

// ---------------------------------------------------------------------------------------------------------------------
// Edit Mesh

#[test]
fn bevel3_flags() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polyBevel3", json!({"args": ["pCube1.e[0]"], "o": 0.25, "sg": 1})), json!(["polyBevel1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 7);
    assert!((get(&mut s, "polyBevel1.offset").as_f64().unwrap() - 0.25).abs() < 1e-9);
    set(&mut s, "polyBevel1.segments", json!(3));
    assert_eq!(counts(&mut s, "pCube1").2, 9);
    // The old command still names its node so.
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "select", json!({"args": ["pCube1.e[0]"]}));
    run(&mut s, "polyBevel", json!({"offset": 0.1}));
    assert_eq!(run(&mut s, "ls", json!({"type": "polyBevel3"})), json!(["polyBevel1"]));
}

#[test]
fn split_ring_and_detach() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polySplitRing", json!({"args": ["pCube1.e[0]"], "wt": 0.25})), json!(["polySplitRing1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 10);
    assert!((get(&mut s, "polySplitRing1.weight").as_f64().unwrap() - 0.25).abs() < 1e-9);
    // -stp 2 -div 3: three evenly.
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "polySplitRing", json!({"args": ["pCube1.e[0]"], "stp": 2, "div": 3}));
    assert_eq!(counts(&mut s, "pCube1").2, 6 + 4 * 3);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polySplitVertex", json!({"args": ["pCube1.vtx[0]"]})), json!(["polySplitVert1"]));
    assert_eq!(counts(&mut s, "pCube1").0, 10);
    assert_eq!(counts(&mut s, "pCube1").2, 6);

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"sx": 2, "sy": 1}));
    // The edge up the middle of two quads, border to border, parts them.
    let middle = (0..7).find(|i| {
        let p = run(&mut s, "polyListComponentConversion", json!({"args": [format!("pPlane1.e[{i}]")], "toVertex": true}));
        let v = run(&mut s, "xform", json!({"args": p, "query": true, "worldSpace": true, "translation": true}));
        let v: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        v.len() == 6 && v[0].abs() < 1e-9 && v[3].abs() < 1e-9
    });
    let e = format!("pPlane1.e[{}]", middle.unwrap());
    assert_eq!(run(&mut s, "polySplitEdge", json!({"args": [e]})), json!(["polySplitEdge1"]));
    assert_eq!(counts(&mut s, "pPlane1"), (8, 8, 2));
}

#[test]
fn deletes_and_collapses() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polyDelEdge", json!({"args": ["pCube1.e[0]"]})), json!(["polyDelEdge1"]));
    assert_eq!(counts(&mut s, "pCube1"), (8, 11, 5));
    set(&mut s, "polyDelEdge1.cleanVertices", json!(true));
    assert_eq!(counts(&mut s, "pCube1"), (6, 9, 5));
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyDelFacet", json!({"args": ["pCube1.f[0]"]})), json!(["polyDelFacet1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 5);
    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"sx": 2, "sy": 2}));
    // The middle vertex goes and its four faces become one.
    let mid = (0..9).find(|i| {
        let c = format!("pPlane1.vtx[{i}]");
        let v = vtx(&mut s, &c);
        v[0].abs() < 1e-9 && v[2].abs() < 1e-9
    });
    assert_eq!(run(&mut s, "polyDelVertex", json!({"args": [format!("pPlane1.vtx[{}]", mid.unwrap())]})), json!(["polyDelVertex1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, 1);
    // The old delete still removes the faces.
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "select", json!({"args": ["pCube1.e[0]"]}));
    run(&mut s, "deleteComponents", json!({}));
    assert_eq!(counts(&mut s, "pCube1").2, 4);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyCollapseEdge", json!({"args": ["pCube1.e[0]"]})), json!(["polyCollapseEdge1"]));
    assert_eq!(counts(&mut s, "pCube1").0, 7);
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyCollapseFacet", json!({"args": ["pCube1.f[0]"]})), json!(["polyCollapseF1"]));
    // The face goes to a point: its four neighbours become triangles.
    assert_eq!(counts(&mut s, "pCube1"), (5, 8, 5));
}

#[test]
fn merges() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polyMergeVertex", json!({"args": ["pCube1.vtx[0:1]"], "d": 5.0})), json!(["polyMergeVert1"]));
    assert_eq!(counts(&mut s, "pCube1").0, 7);
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    // Far apart, -am welds a pair anyway, and d 0 alone doesn't.
    run(&mut s, "polyMergeVertex", json!({"args": ["pCube1.vtx[0:1]"], "d": 0.0}));
    assert_eq!(counts(&mut s, "pCube1").0, 8);
    run(&mut s, "polyMergeVertex", json!({"args": ["pCube1.vtx[0:1]"], "am": true}));
    assert_eq!(counts(&mut s, "pCube1").0, 7);

    // Two planes, combined: two border edges merge.
    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"sx": 1, "sy": 1, "n": "a"}));
    run(&mut s, "polyPlane", json!({"sx": 1, "sy": 1, "n": "b"}));
    run(&mut s, "move", json!({"args": ["b"], "to": [1, 0, 0]}));
    let u = run(&mut s, "polyUnite", json!({"args": ["a", "b"], "ch": false}));
    let name = u[0].as_str().unwrap().to_string();
    let pick = |s: &mut Session, x: f64| -> String {
        let n = counts(s, &name).1;
        for i in 0..n {
            let c = format!("{name}.e[{i}]");
            let p = run(s, "polyListComponentConversion", json!({"args": [c.clone()], "toVertex": true}));
            let v = run(s, "xform", json!({"args": p, "query": true, "worldSpace": true, "translation": true}));
            let v: Vec<f64> = v.as_array().unwrap().iter().map(|q| q.as_f64().unwrap()).collect();
            if v.len() == 6 && (v[0] - x).abs() < 1e-9 && (v[3] - x).abs() < 1e-9 {
                return c;
            }
        }
        panic!("no edge at {x}");
    };
    let left = pick(&mut s, 0.5);
    let right = {
        let n = counts(&mut s, &name).1;
        (0..n).map(|i| format!("{name}.e[{i}]")).find(|c| {
            let p = run(&mut s, "polyListComponentConversion", json!({"args": [c.clone()], "toVertex": true}));
            let v = run(&mut s, "xform", json!({"args": p, "query": true, "worldSpace": true, "translation": true}));
            let v: Vec<f64> = v.as_array().unwrap().iter().map(|q| q.as_f64().unwrap()).collect();
            v.len() == 6 && (v[0] - 0.5).abs() < 1e-9 && (v[3] - 0.5).abs() < 1e-9 && *c != left
        })
    };
    let right = right.expect("the other plane's edge");
    assert_eq!(run(&mut s, "polyMergeEdge", json!({"args": [left, right]})), json!(["polyMergeEdge1"]));
    assert_eq!(counts(&mut s, &name), (6, 7, 2));
}

#[test]
fn merge_facet_joins_two_shells() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"n": "a"}));
    run(&mut s, "polyCube", json!({"n": "b"}));
    run(&mut s, "move", json!({"args": ["b"], "to": [2, 0, 0]}));
    let u = run(&mut s, "polyUnite", json!({"args": ["a", "b"], "ch": false}));
    let n = u[0].as_str().unwrap().to_string();
    assert_eq!(counts(&mut s, &n), (16, 24, 12));
    // a's +x face and b's -x face.
    let facing = |s: &mut Session, x: f64| -> i64 {
        (0..12)
            .find(|f| {
                let p = run(s, "polyListComponentConversion", json!({"args": [format!("{n}.f[{f}]")], "toVertex": true}));
                let v = run(s, "xform", json!({"args": p, "query": true, "worldSpace": true, "translation": true}));
                let v: Vec<f64> = v.as_array().unwrap().iter().map(|q| q.as_f64().unwrap()).collect();
                v.chunks(3).all(|c| (c[0] - x).abs() < 1e-9)
            })
            .unwrap() as i64
    };
    let (f1, f2) = (facing(&mut s, 0.5), facing(&mut s, 1.5));
    let r = run(&mut s, "polyMergeFacet", json!({"args": [n.clone()], "ff": f1, "sf": f2, "mm": 1}));
    assert_eq!(r, json!(["polyMergeFace1"]));
    assert_eq!(counts(&mut s, &n), (12, 20, 10));
}

#[test]
fn subdivide_poke_wedge_chamfer_circularize_connect() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polySubdivideFacet", json!({"args": ["pCube1.f[0]"], "dv": 1})), json!(["polySubdFace1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 9);
    set(&mut s, "polySubdFace1.divisions", json!(2));
    assert_eq!(counts(&mut s, "pCube1").2, 5 + 16);
    set(&mut s, "polySubdFace1.mode", json!(1));
    set(&mut s, "polySubdFace1.divisions", json!(1));
    assert_eq!(counts(&mut s, "pCube1").2, 5 + 4);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polySubdivideEdge", json!({"args": ["pCube1.e[0]"], "dv": 2})), json!(["polySubdEdge1"]));
    assert_eq!(counts(&mut s, "pCube1"), (10, 14, 6));

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyPoke", json!({"args": ["pCube1.f[1]"], "ltz": 1.0})), json!(["polyPoke1"]));
    assert_eq!(counts(&mut s, "pCube1"), (9, 16, 9));
    assert!((max_y(&mut s, "pCube1") - 1.5).abs() < 1e-9, "{}", max_y(&mut s, "pCube1"));

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyChamferVtx", json!({"args": ["pCube1.vtx[0]"], "w": 0.25})), json!(["polyChamfer1"]));
    assert_eq!(counts(&mut s, "pCube1"), (10, 15, 7));

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"w": 4, "h": 2, "sx": 1, "sy": 1}));
    assert_eq!(run(&mut s, "polyCircularize", json!({"args": ["pPlane1.f[0]"], "ro": 0.5})), json!(["polyCircularize1"]));
    let b = run(&mut s, "meshInfo", json!({"args": ["pPlane1"]}));
    let r = 5f64.sqrt() + 0.5;
    assert!(b["boundsMax"][0].as_f64().unwrap() <= r + 1e-6, "{b}");
    let v = vtx(&mut s, "pPlane1.vtx[0]");
    assert!(((v[0] * v[0] + v[2] * v[2]).sqrt() - r).abs() < 1e-6, "{v:?}");

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    // Two opposite edges of a face: a new edge between their midpoints.
    let face_edges = run(&mut s, "polyInfo", json!({"args": ["pCube1.f[0]"], "fe": true}));
    let nums: Vec<&str> = face_edges[0].as_str().unwrap().split_whitespace().collect();
    let (a, b) = (nums[2], nums[4]);
    let r = run(&mut s, "polyConnectComponents", json!({"args": [format!("pCube1.e[{a}]"), format!("pCube1.e[{b}]")]}));
    assert_eq!(r, json!(["polyConnectComponents1"]));
    assert_eq!(counts(&mut s, "pCube1"), (10, 15, 7));
}

#[test]
fn wedge_and_flip_edge_and_duplicate_edge() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 1, "sy": 1}));
    let info = run(&mut s, "polyInfo", json!({"args": ["pPlane1.f[0]"], "fe": true}));
    let hinge: i64 = info[0].as_str().unwrap().split_whitespace().nth(2).unwrap().parse().unwrap();
    let r = run(&mut s, "polyWedgeFace", json!({"args": ["pPlane1.f[0]"], "wa": 90.0, "d": 3, "ed": hinge}));
    assert_eq!(r, json!(["polyWedgeFace1"]));
    let (v, _, f) = counts(&mut s, "pPlane1");
    // The hinge's two points stay; each division adds the far two.
    assert_eq!(v, 4 + 2 * 3);
    assert!(f >= 4, "{f}");

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"sx": 1, "sy": 1}));
    run(&mut s, "polyTriangulate", json!({"args": ["pPlane1"]}));
    let r = (0..5).find_map(|i| {
        let c = format!("pPlane1.e[{i}]");
        let p = run(&mut s, "polyListComponentConversion", json!({"args": [c.clone()], "toFace": true}));
        let n = run(&mut s, "ls", json!({"args": p, "flatten": true})).as_array().unwrap().len();
        (n == 2).then_some(c)
    });
    assert_eq!(run(&mut s, "polyFlipEdge", json!({"args": [r.unwrap()]})), json!(["polyFlipEdge1"]));
    assert_eq!(counts(&mut s, "pPlane1"), (4, 5, 2));

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"sx": 3, "sy": 1}));
    let mid = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "er": 1, "ns": true}));
    assert!(!mid.as_array().unwrap().is_empty());
    let before = counts(&mut s, "pPlane1").2;
    let inner = (0..10).find(|i| {
        let c = format!("pPlane1.e[{i}]");
        let p = run(&mut s, "polyListComponentConversion", json!({"args": [c], "toFace": true}));
        run(&mut s, "ls", json!({"args": p, "flatten": true})).as_array().unwrap().len() == 2
    });
    let r = run(&mut s, "polyDuplicateEdge", json!({"args": [format!("pPlane1.e[{}]", inner.unwrap())], "of": 0.5}));
    assert_eq!(r, json!(["polyDuplicateEdge1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, before + 2);
}

#[test]
fn bridge_edge() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 3, "sy": 1, "w": 3, "h": 1}));
    run(&mut s, "polyDelFacet", json!({"args": ["pPlane1.f[1]"]}));
    assert_eq!(counts(&mut s, "pPlane1").2, 2);
    // The two edges facing each other across the gap.
    let n = counts(&mut s, "pPlane1").1;
    let mut facing = Vec::new();
    for i in 0..n {
        let c = format!("pPlane1.e[{i}]");
        let p = run(&mut s, "polyListComponentConversion", json!({"args": [c.clone()], "toVertex": true}));
        let v = run(&mut s, "xform", json!({"args": p, "query": true, "worldSpace": true, "translation": true}));
        let v: Vec<f64> = v.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
        if v.len() == 6 && (v[0] - v[3]).abs() < 1e-9 && v[0].abs() < 1.0 {
            facing.push(c);
        }
    }
    assert_eq!(facing.len(), 2);
    let r = run(&mut s, "polyBridgeEdge", json!({"args": facing.clone(), "dv": 2, "ctp": 0}));
    assert_eq!(r, json!(["polyBridgeEdge1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, 2 + 3);
    set(&mut s, "polyBridgeEdge1.divisions", json!(0));
    assert_eq!(counts(&mut s, "pPlane1").2, 2 + 1);
    assert_eq!(get(&mut s, "polyBridgeEdge1.divisions"), json!(0));
    // The selection isn't touched by naming them.
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pPlane1"]));
}

#[test]
fn split_append_create_facet() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 1, "sy": 1}));
    let info = run(&mut s, "polyInfo", json!({"args": ["pPlane1.f[0]"], "fe": true}));
    let e: Vec<i64> = info[0].as_str().unwrap().split_whitespace().skip(2).map(|x| x.parse().unwrap()).collect();
    let r = run(&mut s, "polySplit", json!({"args": ["pPlane1"], "ep": [[e[0], 0.5], [e[2], 0.5]]}));
    assert_eq!(r, json!(["polySplit1"]));
    assert_eq!(counts(&mut s, "pPlane1"), (6, 7, 2));
    assert!(s.undo());
    assert_eq!(counts(&mut s, "pPlane1").2, 1);

    // The knife keeps history as polySplit too.
    let cut = run(&mut s, "polyKnife", json!({"mesh": "pPlane1", "points": [{"edge": e[0], "t": 0.5}, {"edge": e[2], "t": 0.5}]}));
    assert!(!cut.as_array().unwrap().is_empty());
    assert_eq!(run(&mut s, "ls", json!({"type": "polySplit"})), json!(["polySplit1"]));
    assert_eq!(get(&mut s, "polyPlane1.width"), json!(1.0));
    assert_eq!(counts(&mut s, "pPlane1").2, 2);
    // Editing the generator under the cut carries through it.
    set(&mut s, "polyPlane1.width", json!(4.0));
    assert!((run(&mut s, "meshInfo", json!({"args": ["pPlane1"]}))["boundsMax"][0].as_f64().unwrap() - 2.0).abs() < 1e-9);
    assert_eq!(counts(&mut s, "pPlane1").2, 2);

    let r = run(&mut s, "polyCreateFacet", json!({"p": [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]], "n": "tile"}));
    assert_eq!(r, json!(["tile", "polyCreateFace1"]));
    assert_eq!(counts(&mut s, "tile"), (4, 4, 1));
    assert_eq!(run(&mut s, "polyCreateFacet", json!({"p": [[0, 0, 0], [1, 0, 0], [0, 0, 1]], "ch": false})).as_array().unwrap().len(), 1);

    // Append along a border edge, through two new points.
    let edges = run(&mut s, "polyInfo", json!({"args": ["tile.f[0]"], "fe": true}));
    let first: i64 = edges[0].as_str().unwrap().split_whitespace().nth(2).unwrap().parse().unwrap();
    let r = run(&mut s, "polyAppend", json!({"args": ["tile"], "ed": [first], "p": [[1, 1, 0], [0, 1, 0]]}));
    assert_eq!(r, json!(["polyAppend1"]));
    assert_eq!(counts(&mut s, "tile"), (6, 7, 2));
}

#[test]
fn projected_curve_splits_a_mesh() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"w": 4, "h": 4, "sx": 1, "sy": 1}));
    run(&mut s, "curve", json!({"degree": 1, "point": [[-2, 1, 0], [2, 1, 0]], "name": "line"}));
    let r = run(&mut s, "polyProjectCurve", json!({"args": ["line", "pPlane1"], "d": [0, -1, 0]}));
    let on_mesh = r[0].as_str().unwrap().to_string();
    let i = run(&mut s, "xform", json!({"args": [format!("{on_mesh}.cv[0]")], "query": true, "worldSpace": true, "translation": true}));
    assert!(i[1].as_f64().unwrap().abs() < 1e-9, "{i}");
    let cut = run(&mut s, "polySplit", json!({"args": ["pPlane1"], "pc": on_mesh}));
    assert_eq!(cut, json!(["polySplit1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// Mesh menu

#[test]
fn unite_and_separate_keep_history() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"n": "a"}));
    run(&mut s, "polyCube", json!({"n": "b"}));
    run(&mut s, "move", json!({"args": ["b"], "to": [3, 0, 0]}));
    assert_eq!(run(&mut s, "polyUnite", json!({"args": ["a", "b"], "n": "both"})), json!(["both", "polyUnite1"]));
    assert_eq!(counts(&mut s, "both"), (16, 24, 12));
    assert!((run(&mut s, "meshInfo", json!({"args": ["both"]}))["boundsMax"][0].as_f64().unwrap() - 3.5).abs() < 1e-9);
    // Moving an original moves the result, while the originals hide.
    run(&mut s, "move", json!({"args": ["b"], "to": [5, 0, 0]}));
    assert!((run(&mut s, "meshInfo", json!({"args": ["both"]}))["boundsMax"][0].as_f64().unwrap() - 5.5).abs() < 1e-9);
    // Resizing an input through its own history reaches the result too.
    set(&mut s, "polyCube2.subdivisionsWidth", json!(2));
    assert_eq!(counts(&mut s, "both").2, 12 + 4 + 2 - 2);
    assert!(s.undo());

    // Undo takes the whole combine back.
    let mut t = Session::new();
    run(&mut t, "polyCube", json!({"n": "x"}));
    run(&mut t, "polyCube", json!({"n": "y"}));
    run(&mut t, "polyUnite", json!({"args": ["x", "y"], "n": "xy"}));
    assert!(t.undo());
    assert!(t.graph.find("xy").unwrap().is_none());
    assert_eq!(counts(&mut t, "x"), (8, 12, 6));
    assert_eq!(run(&mut t, "ls", json!({"type": "polyUnite"})), json!([]));

    let r = run(&mut s, "polySeparate", json!({"args": ["both"]}));
    assert_eq!(r, json!(["polySurface1", "polySurface2", "polySeparate1", "polySeparate2"]));
    assert_eq!(counts(&mut s, "polySurface1"), (8, 12, 6));
    assert_eq!(counts(&mut s, "polySurface2"), (8, 12, 6));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["polySurface1"], "parent": true})), json!(["both"]));

    let mut p = Session::new();
    run(&mut p, "polyCube", json!({"n": "only"}));
    assert!(p.run("polySeparate", json!({"args": ["only"]})).is_err());
}

#[test]
fn booleans_as_nodes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"n": "a"}));
    run(&mut s, "polyCube", json!({"n": "b"}));
    run(&mut s, "move", json!({"args": ["b"], "to": [0.5, 0, 0]}));
    let r = run(&mut s, "polyCBoolOp", json!({"args": ["a", "b"], "op": 1, "n": "u"}));
    assert_eq!(r, json!(["u", "polyCBoolOp1"]));
    let m = run(&mut s, "meshInfo", json!({"args": ["u"]}));
    assert!((m["boundsMax"][0].as_f64().unwrap() - 1.0).abs() < 1e-6, "{m}");
    // Switching the operation re-runs it.
    set(&mut s, "polyCBoolOp1.operation", json!(3));
    let m = run(&mut s, "meshInfo", json!({"args": ["u"]}));
    assert!((m["boundsMax"][0].as_f64().unwrap() - 0.5).abs() < 1e-6, "{m}");
    assert!((m["boundsMin"][0].as_f64().unwrap() - 0.0).abs() < 1e-6, "{m}");
}

#[test]
fn smooth_reduce_remesh_retopo() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polySmooth", json!({"args": ["pCube1"], "dv": 1})), json!(["polySmoothFace1"]));
    assert_eq!(counts(&mut s, "pCube1"), (26, 48, 24));
    set(&mut s, "polySmoothFace1.divisions", json!(2));
    assert_eq!(counts(&mut s, "pCube1").2, 96);
    assert_eq!(get(&mut s, "polySmoothFace1.dv"), json!(2));
    set(&mut s, "polySmoothFace1.method", json!(1));
    // Linear keeps the corners where they were.
    assert!((run(&mut s, "meshInfo", json!({"args": ["pCube1"]}))["boundsMax"][0].as_f64().unwrap() - 0.5).abs() < 1e-9);

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polySphere", json!({"sx": 24, "sy": 16}));
    let before = counts(&mut s, "pSphere1").2;
    assert_eq!(run(&mut s, "polyReduce", json!({"args": ["pSphere1"], "p": 50.0, "ver": 1, "trm": 0})), json!(["polyReduce1"]));
    let half = counts(&mut s, "pSphere1").2;
    assert!(half < before, "{half} {before}");
    set(&mut s, "polyReduce1.percentage", json!(90.0));
    let less = counts(&mut s, "pSphere1").2;
    assert!(less < half, "{less} {half}");
    set(&mut s, "polyReduce1.termination", json!(2));
    set(&mut s, "polyReduce1.triangleCount", json!(100));
    let tris = counts(&mut s, "pSphere1").2;
    assert!(tris <= 110, "{tris}");
    // The ratio form goes on as it did.
    run(&mut s, "newScene", json!({}));
    run(&mut s, "polySphere", json!({"sx": 24, "sy": 16}));
    run(&mut s, "polyReduce", json!({"args": ["pSphere1"], "ratio": 0.5}));
    assert!(counts(&mut s, "pSphere1").2 < 384);

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"w": 4, "h": 4, "sx": 1, "sy": 1}));
    assert_eq!(run(&mut s, "polyRemesh", json!({"args": ["pPlane1"], "mel": 0.5, "urv": false})), json!(["polyRemesh1"]));
    let (_, _, f) = counts(&mut s, "pPlane1");
    assert!(f >= 32, "{f}");
    let m = run(&mut s, "meshInfo", json!({"args": ["pPlane1"]}));
    // Still the same square, and flat.
    assert!((m["boundsMax"][0].as_f64().unwrap() - 2.0).abs() < 1e-6, "{m}");
    assert!(m["boundsMax"][1].as_f64().unwrap().abs() < 1e-6, "{m}");

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polySphere", json!({"sx": 24, "sy": 16}));
    assert_eq!(run(&mut s, "polyRetopo", json!({"args": ["pSphere1"], "targetFaceCount": 200})), json!(["polyRetopo1"]));
    let (_, _, f) = counts(&mut s, "pSphere1");
    assert!((60..=800).contains(&f), "{f}");
    let m = run(&mut s, "meshInfo", json!({"args": ["pSphere1"]}));
    assert!((m["boundsMax"][1].as_f64().unwrap() - 1.0).abs() < 0.1, "{m}");
}

#[test]
fn triangulate_quad_mirror_normal() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 2, "sy": 2}));
    assert_eq!(run(&mut s, "polyTriangulate", json!({"args": ["pPlane1"]})), json!(["polyTriangulate1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, 8);
    assert_eq!(run(&mut s, "polyQuad", json!({"args": ["pPlane1"], "a": 30.0})), json!(["polyQuad1"]));
    assert_eq!(counts(&mut s, "pPlane1").2, 4);
    assert_eq!(get(&mut s, "polyQuad1.angle"), json!(30.0));

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    let r = run(&mut s, "polyMirrorFace", json!({"args": ["pCube1"], "a": 0, "ma": 0, "ad": 1, "mm": 0}));
    assert_eq!(r, json!(["polyMirror1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 12);
    let m = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}));
    assert!((m["boundsMax"][0].as_f64().unwrap() - 1.5).abs() < 1e-9, "{m}");
    set(&mut s, "polyMirror1.axis", json!(1));
    let m = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}));
    assert!((m["boundsMax"][1].as_f64().unwrap() - 1.5).abs() < 1e-9, "{m}");

    // World plane through the middle with cutMesh: half rebuilt from the other half and welded.
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "polyMirror", json!({"args": ["pCube1"], "axis": 0, "mirrorAxis": 2, "cutMesh": true, "axisDirection": 1}));
    let m = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}));
    assert!((m["boundsMax"][0].as_f64().unwrap() - 0.5).abs() < 1e-9, "{m}");
    // The old form of the same command.
    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    run(&mut s, "polyMirror", json!({"args": ["pCube1"], "axis": "x", "direction": "both", "weld": false}));
    assert_eq!(counts(&mut s, "pCube1").2, 12);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    let before = run(&mut s, "polyInfo", json!({"args": ["pCube1.f[1]"], "fn": true}));
    assert_eq!(run(&mut s, "polyNormal", json!({"args": ["pCube1.f[1]"], "nm": 0})), json!(["polyNormal1"]));
    let after = run(&mut s, "polyInfo", json!({"args": ["pCube1.f[1]"], "fn": true}));
    assert_ne!(before, after);
    assert!(after[0].as_str().unwrap().starts_with("FACE_NORMAL      1:"), "{after}");
    assert!(after[0].as_str().unwrap().contains("-1.000000"), "{after}");
    // Conform turns it back.
    run(&mut s, "polyNormal", json!({"args": ["pCube1"], "nm": 2}));
    let m = run(&mut s, "polyInfo", json!({"args": ["pCube1.f[1]"], "fn": true}));
    assert!(m[0].as_str().unwrap().contains(" 1.000000"), "{m}");
}

#[test]
fn close_border_hole_edge_flow_soft_edge() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "polyDelFacet", json!({"args": ["pCube1.f[0]"]}));
    assert_eq!(run(&mut s, "polyCloseBorder", json!({"args": ["pCube1"]})), json!(["polyCloseBorder1"]));
    assert_eq!(counts(&mut s, "pCube1").2, 6);

    run(&mut s, "newScene", json!({}));
    cube(&mut s);
    assert_eq!(run(&mut s, "polyHole", json!({"args": ["pCube1.f[0]"], "ah": true})), json!(["polyHole1"]));
    assert_eq!(run(&mut s, "polyHole", json!({"args": ["pCube1.f[0]"], "query": true})), json!(true));
    assert_eq!(run(&mut s, "polyHole", json!({"args": ["pCube1.f[1]"], "query": true})), json!(false));
    set(&mut s, "polyHole1.assignHole", json!(false));
    assert_eq!(run(&mut s, "polyHole", json!({"args": ["pCube1.f[0]"], "query": true})), json!(false));

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polySphere", json!({"sx": 8, "sy": 8}));
    let sel = run(&mut s, "polySelect", json!({"args": ["pSphere1"], "el": 3, "ns": true, "ass": true}));
    run(&mut s, "polyEditEdgeFlow", json!({"args": sel, "aef": 1.0}));
    assert_eq!(run(&mut s, "polySoftEdge", json!({"args": ["pSphere1"], "a": 30.0})), json!(["polySoftEdge1"]));
    assert_eq!(get(&mut s, "polySoftEdge1.angle"), json!(30.0));
}

#[test]
fn transfer_attributes() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"w": 2, "h": 2, "sx": 2, "sy": 2, "n": "from"}));
    run(&mut s, "polyPlane", json!({"w": 2, "h": 2, "sx": 2, "sy": 2, "n": "to"}));
    run(&mut s, "move", json!({"args": ["to"], "to": [0, 1, 0]}));
    run(&mut s, "polyEditUV", json!({"args": ["from.map[*]"], "u": 0.25, "v": 0.0}));
    let r = run(&mut s, "transferAttributes", json!({"args": ["from", "to"], "uvs": 2, "pos": 1, "spa": 0, "sm": 3}));
    assert_eq!(r, json!(["transferAttributes1"]));
    // Each point lands on the source, in the target's own space, once through world.
    let v = vtx(&mut s, "to.vtx[0]");
    assert!(v[1].abs() < 1e-6, "{v:?}");
    assert_eq!(counts(&mut s, "to").2, 4);
    // Live: moving the source moves it again.
    run(&mut s, "move", json!({"args": ["from"], "to": [0, -1, 0]}));
    let v = vtx(&mut s, "to.vtx[0]");
    assert!((v[1] + 1.0).abs() < 1e-6, "{v:?}");
    assert!(s.undo());
    // Component space: by vertex id, without a matrix.
    run(&mut s, "polyPlane", json!({"w": 6, "h": 6, "sx": 2, "sy": 2, "n": "wide"}));
    run(&mut s, "transferAttributes", json!({"args": ["from", "wide"], "pos": 1, "spa": 4}));
    let m = run(&mut s, "meshInfo", json!({"args": ["wide"]}));
    assert!((m["boundsMax"][0].as_f64().unwrap() - 1.0).abs() < 1e-6, "{m}");
    // UV space: positions read where the same UV lies.
    run(&mut s, "polyPlane", json!({"w": 2, "h": 2, "sx": 2, "sy": 2, "n": "src2"}));
    run(&mut s, "polyPlane", json!({"w": 8, "h": 8, "sx": 2, "sy": 2, "n": "dst2"}));
    run(&mut s, "transferAttributes", json!({"args": ["src2", "dst2"], "pos": 1, "spa": 3}));
    let m = run(&mut s, "meshInfo", json!({"args": ["dst2"]}));
    assert!((m["boundsMax"][0].as_f64().unwrap() - 1.0).abs() < 1e-6, "{m}");
}

#[test]
fn cleanup_finds_and_fixes() {
    let mut s = Session::new();
    run(&mut s, "createMesh", json!({
        "points": [[0, 0, 0], [1, 0, 0], [2, 0, 0], [2, 0, 1], [1, 0, 1], [0, 0, 1], [0, 0, 2], [1, 0, 2]],
        "faces": [[0, 5, 4, 3, 2, 1], [5, 6, 7, 4]],
        "name": "ngon",
    }));
    let found = run(&mut s, "polyCleanup", json!({"args": ["ngon"], "selectOnly": true, "nsided": true}));
    assert_eq!(found, json!(["ngon.f[0]"]));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["ngon.f[0]"]));
    run(&mut s, "polyCleanup", json!({"args": ["ngon"], "nsided": true}));
    assert_eq!(counts(&mut s, "ngon").2, 5);
    // The positional form: fix four-sided faces too.
    let list: Vec<String> = ["0", "1", "1", "1", "0", "0", "0", "0", "0", "1e-05", "0", "1e-05", "0", "1e-05", "0", "0", "0", "0"].iter().map(|x| x.to_string()).collect();
    let found = run(&mut s, "polyCleanupArgList", json!({"args": [4, list]}));
    assert!(found.as_array().is_some(), "{found}");
    // Lamina: two faces on the same points, found.
    run(&mut s, "createMesh", json!({"points": [[0, 0, 0], [1, 0, 0], [0, 0, 1]], "faces": [[0, 1, 2], [2, 1, 0]], "name": "twin"}));
    let lam = run(&mut s, "polyInfo", json!({"args": ["twin"], "lf": true}));
    assert_eq!(lam, json!(["twin.f[1]"]));
    assert_eq!(run(&mut s, "polyCleanup", json!({"args": ["twin"], "selectOnly": true, "lamina": true})), json!(["twin.f[1]"]));
    run(&mut s, "polyCleanup", json!({"args": ["twin"], "lamina": true}));
    assert_eq!(counts(&mut s, "twin").2, 1);
}

// ---------------------------------------------------------------------------------------------------------------------
// Queries

#[test]
fn poly_info_strings() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"sx": 1}));
    let f = run(&mut s, "polyInfo", json!({"args": ["pCube1.f[0]"], "fv": true}));
    let line = f[0].as_str().unwrap();
    assert!(line.starts_with("FACE      0:"), "{line:?}");
    assert!(line.ends_with(" \n"), "{line:?}");
    assert_eq!(line.split_whitespace().count(), 6);
    let e = run(&mut s, "polyInfo", json!({"args": ["pCube1.e[0]"], "ev": true}));
    assert!(e[0].as_str().unwrap().starts_with("EDGE      0:"), "{e}");
    assert!(e[0].as_str().unwrap().contains("Hard"), "{e}");
    let v = run(&mut s, "polyInfo", json!({"args": ["pCube1.vtx[0]"], "vf": true}));
    assert!(v[0].as_str().unwrap().starts_with("VERTEX      0:"), "{v}");
    assert_eq!(v[0].as_str().unwrap().split_whitespace().count(), 5);
    assert_eq!(run(&mut s, "polyInfo", json!({"args": ["pCube1"], "fe": true})).as_array().unwrap().len(), 6);
    assert_eq!(run(&mut s, "polyInfo", json!({"args": ["pCube1"], "nme": true})), json!([]));
    assert_eq!(run(&mut s, "polyInfo", json!({"args": ["pCube1"], "ve": true})).as_array().unwrap().len(), 8);
}

#[test]
fn poly_select_loops_rings_and_how() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 3, "sy": 3}));
    let (_, edges, _) = counts(&mut s, "pPlane1");
    assert_eq!(edges, 24);
    let inner = (0..edges)
        .find(|i| {
            let c = format!("pPlane1.e[{i}]");
            let p = run(&mut s, "polyListComponentConversion", json!({"args": [c], "toFace": true}));
            run(&mut s, "ls", json!({"args": p, "flatten": true})).as_array().unwrap().len() == 2
        })
        .unwrap();
    let l = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "el": inner}));
    assert_eq!(l.as_array().unwrap().len(), 3);
    // It selected them.
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 32})).as_array().unwrap().len(), 3);
    let r = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "er": inner, "ns": true}));
    assert_eq!(r.as_array().unwrap().len(), 4);
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 32})).as_array().unwrap().len(), 3);
    // -ass: as strings.
    let st = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "el": inner, "ns": true, "ass": true}));
    assert!(st[0].as_str().unwrap().starts_with("pPlane1.e["), "{st}");
    // -add keeps what was there.
    run(&mut s, "polySelect", json!({"args": ["pPlane1"], "er": inner, "add": true}));
    assert!(run(&mut s, "filterExpand", json!({"sm": 32})).as_array().unwrap().len() > 3);
    // A border edge.
    let border = (0..edges)
        .find(|i| {
            let c = format!("pPlane1.e[{i}]");
            let p = run(&mut s, "polyListComponentConversion", json!({"args": [c], "toFace": true}));
            run(&mut s, "ls", json!({"args": p, "flatten": true})).as_array().unwrap().len() == 1
        })
        .unwrap();
    let b = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "eb": border, "ns": true}));
    assert_eq!(b.as_array().unwrap().len(), 12);
    let sh = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "ets": 0, "ns": true}));
    assert_eq!(sh.as_array().unwrap().len(), 9);
    // The stretch of a border between two of its edges.
    let list: Vec<i64> = b.as_array().unwrap().iter().map(|v| v.as_i64().unwrap()).collect();
    let part = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "ebp": [list[0], list[2]], "ns": true}));
    assert_eq!(part.as_array().unwrap().len(), 3);
    let every = run(&mut s, "polySelect", json!({"args": ["pPlane1"], "bpt": [list[0], list[2]], "ns": true}));
    assert_eq!(every.as_array().unwrap().len(), 6);
}

#[test]
fn select_constraint_state_and_results() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"sx": 3, "sy": 3}));
    assert_eq!(run(&mut s, "polySelectConstraint", json!({"query": true, "queried": ["mode"]})), json!(0));
    // Mode 3, edges on the border: all twelve, selected.
    run(&mut s, "select", json!({"args": ["pPlane1"]}));
    run(&mut s, "polySelectConstraint", json!({"m": 3, "t": 0x8000, "w": 1}));
    assert_eq!(run(&mut s, "polySelectConstraint", json!({"query": true, "queried": ["mode"]})), json!(3));
    assert_eq!(run(&mut s, "polySelectConstraint", json!({"query": true, "queried": ["where"]})), json!(1));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 32})).as_array().unwrap().len(), 12);
    // Inside instead, returned without selecting.
    let r = run(&mut s, "polySelectConstraint", json!({"w": 2, "rs": true}));
    assert_eq!(run(&mut s, "ls", json!({"args": r, "flatten": true})).as_array().unwrap().len(), 12);
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 32})).as_array().unwrap().len(), 12);
    let state = run(&mut s, "polySelectConstraint", json!({"query": true, "queried": ["stateString"]}));
    assert!(state.as_str().unwrap().contains("-where 2"), "{state}");
    // -dis clears them.
    run(&mut s, "polySelectConstraint", json!({"dis": true}));
    assert_eq!(run(&mut s, "polySelectConstraint", json!({"query": true, "queried": ["mode"]})), json!(0));

    // Faces by size.
    run(&mut s, "polyTriangulate", json!({"args": ["pPlane1.f[0]"]}));
    run(&mut s, "select", json!({"args": ["pPlane1"]}));
    run(&mut s, "polySelectConstraint", json!({"m": 3, "t": 0x0008, "sz": 1}));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 34})).as_array().unwrap().len(), 2);
    run(&mut s, "polySelectConstraint", json!({"sz": 2}));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 34})).as_array().unwrap().len(), 8);
    run(&mut s, "polySelectConstraint", json!({"dis": true}));

    // Propagate grows the selection.
    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"sx": 5, "sy": 5}));
    run(&mut s, "select", json!({"args": ["pPlane1.f[12]"]}));
    run(&mut s, "polySelectConstraint", json!({"pp": 1, "t": 0x0008}));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 34})).as_array().unwrap().len(), 9);
    run(&mut s, "polySelectConstraint", json!({"pp": 2, "t": 0x0008}));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 34})).as_array().unwrap().len(), 1);
    run(&mut s, "polySelectConstraint", json!({"dis": true}));
}

#[test]
fn component_conversion_and_filter_expand() {
    let mut s = Session::new();
    cube(&mut s);
    let ls = |s: &mut Session, r: Value| run(s, "ls", json!({"args": r, "flatten": true})).as_array().unwrap().len();
    // Touching by default, inside with -in.
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.vtx[0]"], "fv": true, "te": true}));
    assert_eq!(ls(&mut s, r), 3);
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.vtx[0]"], "fv": true, "te": true, "in": true}));
    assert_eq!(r, json!([]));
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.vtx[0]"], "tf": true}));
    assert_eq!(ls(&mut s, r), 3);
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.f[0]"], "ff": true, "tv": true}));
    assert_eq!(ls(&mut s, r), 4);
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.f[0]"], "te": true}));
    assert_eq!(ls(&mut s, r), 4);
    // A from flag leaves other kinds out.
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.f[0]"], "fv": true, "te": true}));
    assert_eq!(r, json!([]));
    // A mesh means all of it.
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1"], "tv": true}));
    assert_eq!(r, json!(["pCube1.vtx[0:7]"]));
    // UVs and vertex faces.
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.f[0]"], "tuv": true}));
    assert!(r[0].as_str().unwrap().starts_with("pCube1.map["), "{r}");
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.f[0]"], "tvf": true}));
    assert_eq!(r.as_array().unwrap().len(), 4);
    assert!(r[0].as_str().unwrap().contains("vtxFace["), "{r}");
    let back = run(&mut s, "polyListComponentConversion", json!({"args": [r[0].clone()], "tv": true}));
    assert_eq!(ls(&mut s, back), 1);
    // -bo: the rim of a patch.
    run(&mut s, "polyPlane", json!({"sx": 3, "sy": 3}));
    let r = run(&mut s, "polyListComponentConversion", json!({"args": ["pPlane1.f[0:8]"], "te": true, "bo": true}));
    assert_eq!(ls(&mut s, r), 12);

    run(&mut s, "select", json!({"args": ["pCube1.f[0:2]"]}));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 34})).as_array().unwrap().len(), 3);
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 34, "ex": false})), json!(["pCube1.f[0:2]"]));
    assert_eq!(run(&mut s, "filterExpand", json!({"sm": 31})), Value::Null);
    assert_eq!(run(&mut s, "filterExpand", json!({"args": ["pCube1"], "sm": 12})), json!(["pCube1"]));
    assert_eq!(run(&mut s, "filterExpand", json!({"args": ["pCube1"], "sm": 12, "fp": true})), json!(["|pCube1"]));
    assert_eq!(run(&mut s, "filterExpand", json!({"args": ["pCube1.map[0:1]"], "sm": 35})).as_array().unwrap().len(), 2);
    assert_eq!(run(&mut s, "filterExpand", json!({"args": ["pCube1.vtxFace[0][0:1]"], "sm": 70})).as_array().unwrap().len(), 2);
    run(&mut s, "curve", json!({"degree": 1, "point": [[0, 0, 0], [1, 0, 0]], "name": "c"}));
    assert_eq!(run(&mut s, "filterExpand", json!({"args": ["c"], "sm": 9})), json!(["c"]));
    assert_eq!(run(&mut s, "filterExpand", json!({"args": ["c.cv[0:1]"], "sm": 28})).as_array().unwrap().len(), 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// Normals, colours, sets, options, UVs

#[test]
fn normals_per_vertex() {
    let mut s = Session::new();
    cube(&mut s);
    let r = run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1.vtx[0]"], "xyz": [0, 1, 0]}));
    assert_eq!(r, json!(["polyNormalPerVertex1"]));
    let q = run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["normalXYZ"]}));
    assert_eq!(q.as_array().unwrap().len(), 9);
    assert!((q[1].as_f64().unwrap() - 1.0).abs() < 1e-6, "{q}");
    let one = run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["normalY"]}));
    assert_eq!(one.as_array().unwrap().len(), 3);
    assert_eq!(run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["freezeNormal"]})), json!(true));
    let qf = run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1.vtxFace[0][0]"], "query": true, "queried": ["normalXYZ"]}));
    assert_eq!(qf.as_array().unwrap().len(), 3);
    assert_eq!(run(&mut s, "polySetToFaceNormal", json!({"args": ["pCube1.f[1]"]})), json!(["polySetToFaceNormal1"]));
    assert_eq!(run(&mut s, "polyAverageNormal", json!({"args": ["pCube1.vtx[1]"]})), json!(["polyAverageNormal1"]));
    run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1"], "ufn": true}));
    assert_eq!(run(&mut s, "polyNormalPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["freezeNormal"]})), json!(false));
}

#[test]
fn colours_and_colour_sets() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "query": true, "queried": ["allColorSets"]})), Value::Null);
    run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "create": true, "colorSet": "tint"}));
    run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "create": true, "cs": "other"}));
    assert_eq!(run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "query": true, "queried": ["allColorSets"]})), json!(["tint", "other"]));
    run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "currentColorSet": true, "cs": "other"}));
    assert_eq!(run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "query": true, "queried": ["currentColorSet"]})), json!(["other"]));
    run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "rename": true, "cs": "other", "nc": "main"}));
    run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "copy": true, "cs": "tint", "nc": "copied"}));
    run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "delete": true, "cs": "tint"}));
    assert_eq!(run(&mut s, "polyColorSet", json!({"args": ["pCube1"], "query": true, "queried": ["allColorSets"]})), json!(["main", "copied"]));

    let r = run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtx[0]"], "rgb": [1, 0, 0], "a": 0.5}));
    assert_eq!(r, json!(["polyColorPerVertex1"]));
    let q = run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["colorRGB"]}));
    assert_eq!(q, json!([1.0, 0.0, 0.0]));
    let q = run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["alpha"]}));
    assert_eq!(q, json!([0.5]));
    let q = run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtx[1]"], "query": true, "queried": ["colorR"]}));
    assert_eq!(q, json!([1.0]));
    // One vertex of one face.
    run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtxFace[1][0]"], "r": 0.25}));
    let q = run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtxFace[1][0]"], "query": true, "queried": ["colorR"]}));
    assert_eq!(q, json!([0.25]));
    run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtx[0]"], "r": 0.25, "rel": true, "cla": true}));
    let q = run(&mut s, "polyColorPerVertex", json!({"args": ["pCube1.vtx[0]"], "query": true, "queried": ["colorR"]}));
    assert_eq!(q, json!([1.0]));
}

#[test]
fn uv_sets_and_uv_commands() {
    let mut s = Session::new();
    cube(&mut s);
    assert_eq!(run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "query": true, "queried": ["allUVSets"]})), json!(["map1"]));
    assert_eq!(run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "query": true, "queried": ["currentUVSet"]})), json!(["map1"]));
    run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "create": true, "uvSet": "second"}));
    assert_eq!(run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "query": true, "queried": ["allUVSets"]})), json!(["map1", "second"]));
    run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "currentUVSet": true, "uvSet": "second"}));
    assert_eq!(run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "query": true, "queried": ["currentUVSet"]})), json!(["second"]));
    run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "currentUVSet": true, "uvSet": "map1"}));
    run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "rename": true, "uvSet": "second", "nuv": "renamed"}));
    run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "copy": true, "uvSet": "map1", "nuv": "copied"}));
    run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "delete": true, "uvSet": "renamed"}));
    assert_eq!(run(&mut s, "polyUVSet", json!({"args": ["pCube1"], "query": true, "queried": ["allUVSets"]})), json!(["map1", "copied"]));

    let uv = |s: &mut Session| run(s, "polyEditUV", json!({"args": ["pCube1.map[0]"], "query": true}));
    let before = uv(&mut s);
    let r = run(&mut s, "polyMoveUV", json!({"args": ["pCube1.map[0]"], "tu": 0.25, "tv": 0.0}));
    assert_eq!(r, json!(["polyMoveUV1"]));
    let after = uv(&mut s);
    assert!((after[0].as_f64().unwrap() - before[0].as_f64().unwrap() - 0.25).abs() < 1e-6, "{before} {after}");
    run(&mut s, "polyCopyUV", json!({"args": ["pCube1"], "uvi": "copied"}));
    let copied = uv(&mut s);
    assert!((copied[0].as_f64().unwrap() - before[0].as_f64().unwrap()).abs() < 1e-6);
    assert_eq!(run(&mut s, "ls", json!({"type": "polyCopyUV"})), json!(["polyCopyUV1"]));

    run(&mut s, "polyMoveUV", json!({"args": ["pCube1"], "tu": 3.0, "tv": 0.0}));
    run(&mut s, "polyLayoutUV", json!({"args": ["pCube1"], "ps": 1.0}));
    assert_eq!(run(&mut s, "ls", json!({"type": "polyLayoutUV"})), json!(["polyLayoutUV1"]));
    // A face's UVs of its own, then welded back to their neighbours'.
    run(&mut s, "polyForceUV", json!({"args": ["pCube1.f[0]"], "uni": true}));
    run(&mut s, "polyMergeUV", json!({"args": ["pCube1"], "d": 2.0}));
    assert_eq!(run(&mut s, "ls", json!({"type": "polyMergeUV"})), json!(["polyMergeUV1"]));
    run(&mut s, "polyPinUV", json!({"args": ["pCube1.map[0]"], "v": 1.0}));
    run(&mut s, "polyForceUV", json!({"args": ["pCube1.f[0]"], "uni": true}));
    run(&mut s, "polyContourProjection", json!({"args": ["pCube1.f[0]"]}));
    run(&mut s, "polyFlipUV", json!({"args": ["pCube1.f[0]"], "ft": 0}));

    // Copy and paste a face's UVs.
    run(&mut s, "polyForceUV", json!({"args": ["pCube1.f[0]"], "uni": true}));
    run(&mut s, "polyClipboard", json!({"args": ["pCube1.f[0]"], "cp": true, "uv": true}));
    run(&mut s, "polyClipboard", json!({"args": ["pCube1.f[1]"], "ps": true, "uv": true}));
    run(&mut s, "polyPasteFace", json!({"args": ["pCube1.f[2]"]}));
    run(&mut s, "polyClipboard", json!({"cl": true}));

    // Transfer between same topology.
    run(&mut s, "polyCube", json!({"n": "b"}));
    run(&mut s, "polyMoveUV", json!({"args": ["b"], "tu": 0.5, "tv": 0.0}));
    run(&mut s, "polyCube", json!({"n": "c", "w": 2}));
    assert_eq!(run(&mut s, "polyTransfer", json!({"args": ["c"], "ao": "b", "uv": true, "v": true})), json!(["polyTransfer1"]));
    assert!((run(&mut s, "meshInfo", json!({"args": ["c"]}))["boundsMax"][0].as_f64().unwrap() - 0.5).abs() < 1e-6);
}

#[test]
fn options_table_and_reorder_and_symmetrize() {
    let mut s = Session::new();
    cube(&mut s);
    run(&mut s, "polyOptions", json!({"bc": true, "sizeNormal": 2.5}));
    assert_eq!(run(&mut s, "polyOptions", json!({"query": true, "queried": ["backCulling"]})), json!([true]));
    assert_eq!(run(&mut s, "polyOptions", json!({"query": true, "queried": ["sizeNormal"]})), json!([2.5]));
    assert_eq!(run(&mut s, "polyOptions", json!({"query": true, "queried": ["displayCreaseEdge"]})), json!([false]));

    let v3 = vtx(&mut s, "pCube1.vtx[3]");
    run(&mut s, "meshReorder", json!({"args": ["pCube1.vtx[3]", "pCube1.vtx[2]", "pCube1.vtx[0]"]}));
    assert_eq!(vtx(&mut s, "pCube1.vtx[0]"), v3);
    assert_eq!(counts(&mut s, "pCube1"), (8, 12, 6));

    run(&mut s, "newScene", json!({}));
    run(&mut s, "polyPlane", json!({"w": 2, "h": 2, "sx": 2, "sy": 2}));
    run(&mut s, "polySymmetrize", json!({"args": ["pPlane1"]}));
    assert_eq!(counts(&mut s, "pPlane1").2, 4);
}
