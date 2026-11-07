//! Rigging: joints and orientation, skinning, constraints, IK.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn pos(s: &mut Session, j: &str) -> [f64; 3] {
    let v = run(s, "xform", json!({"args": [j], "query": true, "worldSpace": true, "translation": true}));
    let a = v.as_array().unwrap();
    [a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()]
}

fn jpos(s: &mut Session, j: &str) -> [f64; 3] {
    let v = run(s, "joint", json!({"args": [j], "query": true}));
    let a = v.as_array().unwrap();
    [a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()]
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-4)
}

fn chain(s: &mut Session) {
    run(s, "select", json!({"clear": true}));
    run(s, "joint", json!({"position": [0, 0, 0]}));
    run(s, "joint", json!({"position": [2, 0, -0.5]}));
    run(s, "joint", json!({"position": [4, 0, 0]}));
    run(s, "joint", json!({"args": ["joint1"], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup", "children": true}));
}

#[test]
fn joints_keep_their_places_when_oriented() {
    let mut s = Session::new();
    chain(&mut s);
    assert!(near(jpos(&mut s, "joint2"), [2.0, 0.0, -0.5]), "{:?}", jpos(&mut s, "joint2"));
    assert!(near(jpos(&mut s, "joint3"), [4.0, 0.0, 0.0]));
    // joint1's X now points at joint2: rotating it about its own Y swings joint2 round.
    run(&mut s, "setAttr", json!({"args": ["joint1.rotateZ", 90.0]}));
    let p = jpos(&mut s, "joint2");
    assert!(p[1] > 2.0, "{p:?}");
}

#[test]
fn skin_follows_the_joints() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 4, "subdivisionsX": 4}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [2, 0, 0]}));
    chain(&mut s);
    run(&mut s, "skinCluster", json!({"args": ["joint1", "pCube1"]}));
    let before = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}))["boundsMax"][1].as_f64().unwrap();
    run(&mut s, "setAttr", json!({"args": ["joint1.rotateZ", 45.0]}));
    let after = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}))["boundsMax"][1].as_f64().unwrap();
    assert!(after > before + 0.5, "{before} {after}");
    let inf = run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "influence": true}));
    assert_eq!(inf.as_array().unwrap().len(), 3);
}

#[test]
fn constraints_drive_objects() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({}));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "move", json!({"args": ["locator1"], "to": [3, 1, 0]}));
    run(&mut s, "pointConstraint", json!({"args": ["locator1", "pCube1"]}));
    assert!(near(pos(&mut s, "pCube1"), [3.0, 1.0, 0.0]), "{:?}", pos(&mut s, "pCube1"));
    run(&mut s, "move", json!({"args": ["locator1"], "to": [-1, 2, 5]}));
    assert!(near(pos(&mut s, "pCube1"), [-1.0, 2.0, 5.0]));
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "move", json!({"args": ["pSphere1"], "to": [0, 0, 2]}));
    run(&mut s, "parentConstraint", json!({"args": ["locator1", "pSphere1"], "maintainOffset": true}));
    assert!(near(pos(&mut s, "pSphere1"), [0.0, 0.0, 2.0]));
    run(&mut s, "move", json!({"args": ["locator1"], "to": [0, 2, 5]}));
    assert!(near(pos(&mut s, "pSphere1"), [1.0, 0.0, -3.0 + 5.0]), "{:?}", pos(&mut s, "pSphere1"));
}

#[test]
fn ik_reaches_the_handle() {
    let mut s = Session::new();
    chain(&mut s);
    let h = run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    assert_eq!(h[0], "ikHandle1");
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3, 1, 0]}));
    let p = jpos(&mut s, "joint3");
    assert!(near(p, [3.0, 1.0, 0.0]), "{p:?}");
    // Bent toward the pole vector (-z, where joint2 started).
    assert!(jpos(&mut s, "joint2")[2] < 0.0);
    let base = jpos(&mut s, "joint1");
    assert!(near(base, [0.0, 0.0, 0.0]));
}

#[test]
fn rig_animation_and_uvs_survive_save_and_load() {
    use aztek_scene::scene_file;
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 4, "subdivisionsX": 4}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [2, 0, 0]}));
    chain(&mut s);
    run(&mut s, "skinCluster", json!({"args": ["joint1", "pCube1"]}));
    run(&mut s, "polyAutoProjection", json!({"args": ["pCube1"]}));
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    run(&mut s, "setKeyframe", json!({"args": ["ikHandle1"], "attribute": "ty", "time": 1, "value": 0}));
    run(&mut s, "setKeyframe", json!({"args": ["ikHandle1"], "attribute": "ty", "time": 10, "value": 2}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    let before = run(&mut s, "meshData", json!({"args": ["pCube1"]}));
    let text = serde_json::to_string(&scene_file::serialize(&s.graph, false)).unwrap();
    run(&mut s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
    run(&mut s, "currentTime", json!({"args": [10]}));
    let after = run(&mut s, "meshData", json!({"args": ["pCube1"]}));
    let pts = |v: &Value| v["points"].as_array().unwrap().iter().flat_map(|p| p.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<_>>()).collect::<Vec<f64>>();
    let (a, b) = (pts(&before), pts(&after));
    assert_eq!(a.len(), b.len());
    assert!(a.iter().zip(&b).all(|(x, y)| (x - y).abs() < 1e-6));
}

fn skinned(s: &mut Session) {
    run(s, "polyCube", json!({"width": 4, "subdivisionsX": 8}));
    run(s, "move", json!({"args": ["pCube1"], "to": [2, 0, 0]}));
    chain(s);
    run(s, "skinCluster", json!({"args": ["joint1", "pCube1"]}));
}

fn weight(s: &mut Session, j: &str, c: &str) -> f64 {
    run(s, "skinPercent", json!({"args": ["skinCluster1", c], "query": true, "transform": j})).as_f64().unwrap()
}

#[test]
fn skin_percent_sets_queries_and_normalizes() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["joint1", 1.0]]}));
    assert!((weight(&mut s, "joint1", "pCube1.vtx[0]") - 1.0).abs() < 1e-9);
    assert!(weight(&mut s, "joint2", "pCube1.vtx[0]").abs() < 1e-9);
    // Setting one influence makes the others up to 1.
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": ["joint2", 0.25]}));
    let (a, b) = (weight(&mut s, "joint1", "pCube1.vtx[0]"), weight(&mut s, "joint2", "pCube1.vtx[0]"));
    assert!((b - 0.25).abs() < 1e-9 && (a - 0.75).abs() < 1e-9, "{a} {b}");
    // Relative.
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": ["joint2", 0.25], "relative": true}));
    assert!((weight(&mut s, "joint2", "pCube1.vtx[0]") - 0.5).abs() < 1e-9);
    let vals = run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "query": true, "value": true}));
    let sum: f64 = vals.as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).sum();
    assert!((sum - 1.0).abs() < 1e-9);
    assert_eq!(vals.as_array().unwrap().len(), 3);
    // Prune drops the small ones and normalizes.
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["joint1", 0.05], ["joint2", 0.95]]}));
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "pruneWeights": 0.1}));
    assert!((weight(&mut s, "joint2", "pCube1.vtx[0]") - 1.0).abs() < 1e-9);
    // Move weight.
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformMoveWeights": ["joint2", "joint3"]}));
    assert!((weight(&mut s, "joint3", "pCube1.vtx[0]") - 1.0).abs() < 1e-9);
}

#[test]
fn skin_cluster_edit_and_query() {
    let mut s = Session::new();
    skinned(&mut s);
    assert_eq!(run(&mut s, "skinCluster", json!({"args": ["pCube1"], "query": true, "maximumInfluences": true})), json!(5));
    assert_eq!(run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "geometry": true})), json!(["pCubeShape1"]));
    assert_eq!(run(&mut s, "findRelatedSkinCluster", json!({"args": ["pCube1"]})), json!("skinCluster1"));
    // Locked weights hold while the rest changes.
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["joint1", 0.5], ["joint2", 0.5]]}));
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "influence": "joint1", "lockWeights": true}));
    assert_eq!(run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "influence": "joint1", "lockWeights": true})), json!(true));
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": ["joint2", 0.1]}));
    assert!((weight(&mut s, "joint1", "pCube1.vtx[0]") - 0.5).abs() < 1e-9);
    assert!((weight(&mut s, "joint2", "pCube1.vtx[0]") - 0.1).abs() < 1e-9);
    assert!((weight(&mut s, "joint3", "pCube1.vtx[0]") - 0.4).abs() < 1e-9);
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "influence": "joint1", "lockWeights": false}));
    // Add and remove influences.
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [1, 3, 0], "name": "extra"}));
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "addInfluence": "extra", "weight": 0.0}));
    let inf = run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "influence": true}));
    assert_eq!(inf.as_array().unwrap().len(), 4, "{inf}");
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": ["extra", 1.0]}));
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "removeInfluence": "extra"}));
    let inf = run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "influence": true}));
    assert_eq!(inf.as_array().unwrap().len(), 3);
    // Smoothing evens out a spike.
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1"], "transformValue": [["joint1", 1.0]]}));
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[4]"], "transformValue": [["joint3", 1.0]]}));
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "smoothWeights": 0.5, "smoothWeightsMaxIterations": 1}));
    let v = weight(&mut s, "joint3", "pCube1.vtx[4]");
    assert!(v > 0.2 && v < 0.9, "{v}");
    assert!(weight(&mut s, "joint3", "pCube1.vtx[5]") > 0.0);
}

#[test]
fn dual_quaternion_keeps_volume_at_a_bend() {
    let mut s = Session::new();
    let mut heights = Vec::new();
    for method in [0, 1] {
        let mut s2 = Session::new();
        run(&mut s2, "polyCube", json!({"width": 4, "subdivisionsX": 8}));
        run(&mut s2, "move", json!({"args": ["pCube1"], "to": [2, 0, 0]}));
        run(&mut s2, "select", json!({"clear": true}));
        run(&mut s2, "joint", json!({"position": [0, 0, 0]}));
        run(&mut s2, "joint", json!({"position": [2, 0, 0]}));
        run(&mut s2, "joint", json!({"position": [4, 0, 0]}));
        run(&mut s2, "skinCluster", json!({"args": ["joint1", "pCube1"], "skinMethod": method, "maximumInfluences": 2, "dropoffRate": 1.0}));
        assert_eq!(run(&mut s2, "skinCluster", json!({"args": ["pCube1"], "query": true, "skinMethod": true})), json!(method));
        run(&mut s2, "setAttr", json!({"args": ["joint2.rotateX", 180.0]}));
        let i = run(&mut s2, "meshInfo", json!({"args": ["pCube1"]}));
        let h = i["boundsMax"][1].as_f64().unwrap() - i["boundsMin"][1].as_f64().unwrap();
        heights.push(h);
    }
    let _ = &mut s;
    // Linear blending pinches a half turn; dual quaternions don't.
    assert!(heights[1] >= heights[0] - 1e-6, "{heights:?}");
}

#[test]
fn mirror_weights_copy_across_the_plane() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 4, "subdivisionsX": 4}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0, 0, 0], "name": "root"}));
    run(&mut s, "joint", json!({"position": [2, 0, 0], "name": "L_arm"}));
    run(&mut s, "select", json!({"args": ["root"]}));
    run(&mut s, "joint", json!({"position": [-2, 0, 0], "name": "R_arm"}));
    run(&mut s, "skinCluster", json!({"args": ["root", "pCube1"]}));
    // All of the positive side to the left arm.
    let info = run(&mut s, "meshInfo", json!({"args": ["pCube1"]}));
    let n = info["points"].as_i64().unwrap_or(info["vertices"].as_i64().unwrap_or(0));
    assert!(n > 0, "{info}");
    let mut plus = Vec::new();
    let mut minus = Vec::new();
    for p in 0..n {
        let c = format!("pCube1.vtx[{p}]");
        let v = run(&mut s, "xform", json!({"args": [c.clone()], "query": true, "worldSpace": true, "translation": true}));
        let x = v[0].as_f64().unwrap();
        if x > 0.1 {
            plus.push(c.clone());
        } else if x < -0.1 {
            minus.push(c.clone());
        }
    }
    run(&mut s, "skinPercent", json!({"args": [json!("skinCluster1"), plus], "transformValue": [["L_arm", 1.0]]}));
    run(&mut s, "skinPercent", json!({"args": [json!("skinCluster1"), minus.clone()], "transformValue": [["root", 1.0]]}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "copySkinWeights", json!({"sourceSkin": "skinCluster1", "destinationSkin": "skinCluster1", "mirrorMode": "YZ", "influenceAssociation": ["name", "closestJoint"]}));
    for c in &minus {
        assert!((weight(&mut s, "R_arm", c) - 1.0).abs() < 1e-3, "{c}: {}", weight(&mut s, "R_arm", c));
    }
}

#[test]
fn copy_weights_to_another_mesh() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1"], "transformValue": [["joint3", 1.0]]}));
    run(&mut s, "polyCube", json!({"width": 4, "subdivisionsX": 8}));
    run(&mut s, "move", json!({"args": ["pCube2"], "to": [2, 0, 0]}));
    run(&mut s, "skinCluster", json!({"args": ["joint1", "pCube2"], "name": "other"}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "copySkinWeights", json!({"sourceSkin": "skinCluster1", "destinationSkin": "other", "noMirror": true}));
    let v = run(&mut s, "skinPercent", json!({"args": ["other", "pCube2.vtx[0]"], "query": true, "transform": "joint3"})).as_f64().unwrap();
    assert!((v - 1.0).abs() < 1e-6, "{v}");
}

#[test]
fn the_brush_paints_and_falls_off_and_undoes() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1"], "transformValue": [["joint1", 1.0]]}));
    run(&mut s, "artAttrSkinPaintCtx", json!({"edit": true, "influence": "joint3", "radius": 1.0, "value": 1.0, "opacity": 1.0, "selectedattroper": "absolute"}));
    assert_eq!(run(&mut s, "artAttrSkinPaintCtx", json!({"query": true, "queried": ["radius"]})), json!(1.0));
    let far = "pCube1.vtx[0]"; // x = 0
    let n = run(&mut s, "skinPaintStamp", json!({"args": ["pCube1"], "position": [4, 0.5, 0.5]})).as_i64().unwrap();
    assert!(n > 0);
    assert!(weight(&mut s, "joint3", far).abs() < 1e-9, "far points are out of reach");
    // The corner under the brush is all joint3's now.
    let v = run(&mut s, "ls", json!({"args": ["pCube1.vtx[*]"], "flatten": true}));
    let mut best = 0.0_f64;
    for c in v.as_array().unwrap() {
        best = best.max(weight(&mut s, "joint3", c.as_str().unwrap()));
    }
    assert!(best > 0.9, "{best}");
    assert!(s.undo());
    let mut after = 0.0_f64;
    for c in v.as_array().unwrap() {
        after = after.max(weight(&mut s, "joint3", c.as_str().unwrap()));
    }
    assert!(after < 1e-9, "{after}");
    // Flood.
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    run(&mut s, "artAttrSkinPaintCtx", json!({"edit": true, "influence": "joint2", "clear": true}));
    assert!((weight(&mut s, "joint2", "pCube1.vtx[0]") - 1.0).abs() < 1e-9);
}

#[test]
fn bind_pose_comes_back() {
    let mut s = Session::new();
    skinned(&mut s);
    let before = jpos(&mut s, "joint3");
    run(&mut s, "setAttr", json!({"args": ["joint1.rotateZ", 40.0]}));
    assert!(!near(jpos(&mut s, "joint3"), before));
    assert_eq!(run(&mut s, "dagPose", json!({"args": ["joint1"], "query": true, "bindPose": true})), json!(["bindPose1"]));
    run(&mut s, "dagPose", json!({"args": ["joint1"], "restore": true, "global": true, "bindPose": true}));
    assert!(near(jpos(&mut s, "joint3"), before));
}

#[test]
fn skin_saves_and_loads() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["joint3", 0.75]]}));
    s.graph.evaluate();
    let doc = aztek_scene::scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    aztek_scene::scene_file::deserialize(&mut t.graph, &doc).unwrap();
    let v = run(&mut t, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "query": true, "transform": "joint3"})).as_f64().unwrap();
    assert!((v - 0.75).abs() < 1e-9);
}
