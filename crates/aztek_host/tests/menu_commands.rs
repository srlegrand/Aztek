//! What the menu items added or repaired run: each does something, with the flags the menu passes.

use aztek_host::Core;
use serde_json::{json, Value};

fn sel(c: &mut Core) -> Vec<String> {
    c.mel("ls -sl;").as_array().map(|a| a.iter().map(|v| v.as_str().unwrap().to_string()).collect()).unwrap_or_default()
}

#[test]
fn grow_and_shrink_along_loop() {
    let mut c = Core::new();
    c.mel("polyCube -sx 8 -sy 8 -sz 8;");
    // An edge in the middle of a side, where each end has four edges.
    c.mel("select pCube1.e[100];");
    c.exec("growSelection", json!({"alongLoop": true})).unwrap();
    let grown = c.s.run("filterExpand", json!({"selectionMask": 32, "expand": true})).unwrap();
    assert_eq!(grown.as_array().map(Vec::len), Some(3), "{grown}");
    c.exec("shrinkSelection", json!({"alongLoop": true})).unwrap();
    assert_eq!(sel(&mut c), ["pCube1.e[100]"]);
}

/// Edges at each point: a spin moves an edge's ends, so its id says nothing but this does.
fn valences(c: &mut Core) -> Vec<usize> {
    (0..8)
        .map(|i| {
            let v = c.mel(&format!("polyInfo -ve pCube1.vtx[{i}];"));
            v[0].as_str().unwrap().split(':').nth(1).unwrap().split_whitespace().filter(|w| w.parse::<i64>().is_ok()).count()
        })
        .collect()
}

#[test]
fn spin_edge_goes_both_ways() {
    let mut c = Core::new();
    c.mel("polyCube; select pCube1.e[0];");
    let start = valences(&mut c);
    c.exec("polySpinEdge", json!({"offset": 1})).unwrap();
    let forward = valences(&mut c);
    assert_ne!(start, forward);
    c.mel("undo;");
    assert_eq!(valences(&mut c), start);
    c.exec("polySpinEdge", json!({"offset": -1})).unwrap();
    let backward = valences(&mut c);
    assert_ne!(start, backward);
    assert_ne!(forward, backward);
}

#[test]
fn non_deformer_history_keeps_the_deformer() {
    let mut c = Core::new();
    c.mel("polyCube -n a; select a.f[1]; polyExtrudeFacet -ltz 1; select a; nonLinear -type bend;");
    c.mel("select a;");
    c.exec("bakePartialHistory", json!({"prePostDeformers": true})).unwrap();
    let h = c.mel("listHistory a;");
    let list: Vec<&str> = h.as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(list.iter().any(|n| n.starts_with("bend")), "{list:?}");
    assert!(!list.iter().any(|n| n.starts_with("polyExtrude") || n.starts_with("polyCube")), "{list:?}");
    assert_eq!(c.s.run("meshInfo", json!({"args": ["a"]})).unwrap()["faces"], 10, "the extrusion stayed");
    // No deformers: plain history goes.
    c.mel("polyCube -n b; select b.f[1]; polyExtrudeFacet;");
    c.mel("select b;");
    c.exec("bakePartialHistory", json!({})).unwrap();
    let h = c.mel("listHistory b;");
    assert!(!h.to_string().contains("polyExtrude"), "{h}");
}

#[test]
fn every_new_menu_command_exists() {
    let c = Core::new();
    for cmd in [
        "polyPyramid", "polyPrism", "polyHelix", "polySoccerBall", "annotate", "distanceDimension", "paramDimension", "camera", "meshReorder", "polyMoveFacet", "polyMoveEdge", "polyMoveVertex",
        "polyProjectCurve", "filletCurve", "curveIntersect", "polyContourProjection", "u3dAutoSeam", "polyMergeUV", "polyMapDel", "polyOptUvs", "polyStraightenUVBorder", "copyDeformerWeights",
        "deformerWeights", "scaleKey", "snapKey", "matchTransform", "matchPivot", "showHidden", "selectBorder", "bakePartialHistory",
    ] {
        assert!(c.s.commands.find(cmd).is_some(), "{cmd}");
    }
}

#[test]
fn match_transform_flags_run() {
    let mut c = Core::new();
    c.mel("polyCube -n a; polyCube -n b; move 3 0 0 b; select a b;");
    c.exec("matchTransform", json!({"position": true})).unwrap();
    let x = c.mel("getAttr a.translateX;").as_f64().unwrap();
    assert!((x - 3.0).abs() < 1e-9, "{x}");
}

#[test]
fn copy_and_paste_vertex_weights() {
    let mut c = Core::new();
    c.mel("select -cl; joint -p 0 0 0 -n j1; joint -p 0 2 0 -n j2; polyCube -n m -h 2 -sy 2; move 0 1 0 m; select j1 m; skinCluster -n sk;");
    let w = |c: &mut Core, v: &str| c.mel(&format!("skinPercent -t j2 -q sk m.vtx[{v}];")).as_f64().unwrap();
    let (low, high) = (w(&mut c, "0"), w(&mut c, "4"));
    assert!((low - high).abs() > 0.2, "{low} {high}");
    c.mel("select m.vtx[4];");
    c.exec("artAttrSkinWeightCopy", json!({})).unwrap();
    c.mel("select m.vtx[0];");
    c.exec("artAttrSkinWeightPaste", json!({})).unwrap();
    assert!((w(&mut c, "0") - high).abs() < 1e-6);
    c.mel("undo;");
    assert!((w(&mut c, "0") - low).abs() < 1e-6, "one undo");
}

#[test]
fn rivet_holds_a_locator_to_a_face() {
    let mut c = Core::new();
    c.mel("polyCube -n m; spaceLocator -n rivet1;");
    c.exec("pointOnPolyConstraint", json!({"args": ["m.f[1]", "rivet1"]})).unwrap();
    let y = c.mel("xform -q -ws -t rivet1;")[1].as_f64().unwrap();
    assert!((y - 0.5).abs() < 1e-3, "on the top face: {y}");
    c.mel("move -r 0 3 0 m;");
    let y = c.mel("xform -q -ws -t rivet1;")[1].as_f64().unwrap();
    assert!((y - 3.5).abs() < 1e-3, "follows: {y}");
}

#[test]
fn subdiv_proxy_commands_run() {
    let mut c = Core::new();
    c.mel("polyCube -n m; select m;");
    let made = c.exec("polyDuplicateAndConnect", json!({})).unwrap();
    let copy = made.as_array().map(|a| a[0].as_str().unwrap().to_string()).unwrap_or_else(|| made.as_str().unwrap().to_string());
    c.exec("polySmooth", json!({"args": [copy], "divisions": 2})).unwrap();
    assert!(c.s.run("meshInfo", json!({"args": [copy]})).unwrap()["faces"].as_i64().unwrap() > 6);
}

#[test]
fn uv_set_editor_flags_run() {
    let mut c = Core::new();
    c.mel("polyCube -n m; select m;");
    c.exec("polyUVSet", json!({"create": true, "uvSet": "uvSet1"})).unwrap();
    let all = c.s.run("polyUVSet", json!({"query": true, "queried": ["allUVSets"]})).unwrap();
    assert!(all.to_string().contains("uvSet1"), "{all}");
    c.exec("polyUVSet", json!({"delete": true, "uvSet": "uvSet1"})).unwrap();
}

#[test]
fn shelf_edits_bring_up_handles() {
    let mut c = Core::new();
    c.mel("polyCube; select pCube1.f[1];");
    let r = c.mel("polyExtrude;");
    assert!(aztek_host::node_manip::after_action(&mut c, &r), "{r}");
}
