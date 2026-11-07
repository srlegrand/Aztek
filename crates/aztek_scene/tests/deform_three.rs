//! blendShape in full, the remaining deformers, painting attributes, skin extras, the original shape and deformer sets.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn point(s: &mut Session, comp: &str) -> [f64; 3] {
    let v = run(s, "pointPosition", json!({"args": [comp]}));
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
}

fn get(s: &mut Session, plug: &str) -> f64 {
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap_or_else(|| panic!("{plug} is not a number"))
}

fn set(s: &mut Session, plug: &str, v: f64) {
    run(s, "setAttr", json!({"args": [plug, v]}));
}

fn round_trip(s: &mut Session) {
    let doc = aztek_scene::scene_file::serialize(&s.graph, false);
    let text = serde_json::to_string(&doc).unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    s.graph.clear();
    aztek_scene::scene_file::deserialize(&mut s.graph, &doc).unwrap();
    s.graph.evaluate();
}

/// pCube1 at the origin and a copy, pCube2, with vtx[0] 1 further along x.
fn two_cubes(s: &mut Session) -> [f64; 3] {
    run(s, "polyCube", json!({}));
    run(s, "polyCube", json!({}));
    run(s, "move", json!({"args": ["pCube2.vtx[0]"], "by": [1.0, 0.0, 0.0]}));
    point(s, "pCube1.vtx[0]")
}

// ---------------------------------------------------------------------------------------------------------------------
// blendShape

#[test]
fn blend_weights_have_aliases_and_move_the_base() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    assert_eq!(run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]})), json!(["blendShape1"]));
    assert_eq!(get(&mut s, "blendShape1.pCube2"), 0.0);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), v0));
    set(&mut s, "blendShape1.pCube2", 0.5);
    assert_eq!(get(&mut s, "blendShape1.weight[0]"), 0.5);
    assert_eq!(get(&mut s, "blendShape1.w[0]"), 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.5, v0[1], v0[2]]));
    set(&mut s, "blendShape1.envelope", 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.25, v0[1], v0[2]]));
    assert_eq!(run(&mut s, "aliasAttr", json!({"args": ["blendShape1"], "query": true})), json!(["pCube2", "weight[0]"]));
    assert_eq!(run(&mut s, "listAttr", json!({"args": ["blendShape1.weight"], "multi": true})), json!(["pCube2"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "geometryFilter"})), json!(["blendShape1"]));
    // Scripts rename them too.
    run(&mut s, "aliasAttr", json!({"args": ["smile", "blendShape1.weight[0]"]}));
    assert_eq!(get(&mut s, "blendShape1.smile"), 0.5);
    run(&mut s, "aliasAttr", json!({"args": ["blendShape1.smile"], "remove": true}));
    assert!(s.run("getAttr", json!({"args": ["blendShape1.smile"]})).is_err());
}

#[test]
fn blend_queries_and_flags() {
    let mut s = Session::new();
    two_cubes(&mut s);
    run(&mut s, "polyCube", json!({}));
    let r = run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube3", "pCube1"], "name": "faces", "weight": [[0, 0.25], [1, 0.75]], "origin": "local"}));
    assert_eq!(r, json!(["faces"]));
    let q = |s: &mut Session, flag: &str| run(s, "blendShape", json!({"args": ["faces"], "query": true, "queried": [flag]}));
    assert_eq!(q(&mut s, "weight"), json!([0.25, 0.75]));
    assert_eq!(q(&mut s, "weightCount"), json!(2));
    assert_eq!(q(&mut s, "target"), json!(["pCubeShape2", "pCubeShape3"]));
    assert_eq!(q(&mut s, "geometry"), json!(["pCubeShape1"]));
    assert_eq!(q(&mut s, "origin"), json!("local"));
    assert_eq!(q(&mut s, "topologyCheck"), json!(true));
    run(&mut s, "blendShape", json!({"args": ["faces"], "edit": true, "weight": [1, 0.5]}));
    assert_eq!(q(&mut s, "weight"), json!([0.25, 0.5]));
    // The undo stack holds the edit.
    s.undo();
    assert_eq!(q(&mut s, "weight"), json!([0.25, 0.75]));
    // By the -t form, and removed by it.
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "blendShape", json!({"args": ["faces"], "edit": true, "target": ["pCube1", 5, "pCube4", 1.0]}));
    assert_eq!(q(&mut s, "weightCount"), json!(3));
    assert_eq!(get(&mut s, "faces.pCube4"), 0.0);
    assert_eq!(get(&mut s, "faces.weight[5]"), 0.0);
    run(&mut s, "blendShape", json!({"args": ["faces"], "edit": true, "remove": true, "target": ["pCube1", 5, "pCube4", 1.0]}));
    assert_eq!(q(&mut s, "weightCount"), json!(2));
    assert!(s.run("getAttr", json!({"args": ["faces.pCube4"]})).is_err());
    // -rtd by index clears the deltas and leaves the target.
    run(&mut s, "blendShape", json!({"args": ["faces"], "edit": true, "resetTargetDelta": [0, 0]}));
    assert_eq!(q(&mut s, "weightCount"), json!(2));
    assert_eq!(q(&mut s, "target"), json!(["pCubeShape3"]));
    // ... and by name removes it.
    run(&mut s, "blendShape", json!({"args": ["faces"], "edit": true, "resetTargetDelta": "pCube3"}));
    assert_eq!(q(&mut s, "weightCount"), json!(1));
}

#[test]
fn topology_check_refuses_a_different_count() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polySphere", json!({}));
    assert!(s.run("blendShape", json!({"args": ["pSphere1", "pCube1"]})).is_err());
    run(&mut s, "blendShape", json!({"args": ["pSphere1", "pCube1"], "topologyCheck": false}));
    assert_eq!(run(&mut s, "ls", json!({"type": "blendShape"})), json!(["blendShape1"]));
}

#[test]
fn in_betweens_pass_through_their_shape_at_their_weight() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "polyCube", json!({"name": "mid"}));
    // At weight 0.5 the point is 2 up rather than 0.5 along.
    run(&mut s, "move", json!({"args": ["mid.vtx[0]"], "by": [0.0, 2.0, 0.0]}));
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]}));
    run(&mut s, "blendShape", json!({"args": ["blendShape1"], "edit": true, "inBetween": true, "target": ["pCube1", 0, "mid", 0.5]}));
    set(&mut s, "blendShape1.pCube2", 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0], v0[1] + 2.0, v0[2]]));
    set(&mut s, "blendShape1.pCube2", 0.25);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0], v0[1] + 1.0, v0[2]]));
    set(&mut s, "blendShape1.pCube2", 0.75);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.5, v0[1] + 1.0, v0[2]]));
    set(&mut s, "blendShape1.pCube2", 1.0);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1], v0[2]]));
    // Past the ends it carries on the way it was going.
    set(&mut s, "blendShape1.pCube2", -0.25);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0], v0[1] - 1.0, v0[2]]));
    round_trip(&mut s);
    set(&mut s, "blendShape1.pCube2", 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0], v0[1] + 2.0, v0[2]]));
}

#[test]
fn relative_in_between_adds_to_the_full_target() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "polyCube", json!({"name": "mid"}));
    // Absolute shape at 0.5: 0.5 along and 2 up, so 2 up relative to where the target has got to.
    run(&mut s, "move", json!({"args": ["mid.vtx[0]"], "by": [0.5, 2.0, 0.0]}));
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]}));
    run(&mut s, "blendShape", json!({"args": ["blendShape1"], "edit": true, "inBetween": true, "inBetweenType": "relative", "target": ["pCube1", 0, "mid", 0.5]}));
    set(&mut s, "blendShape1.pCube2", 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.5, v0[1] + 2.0, v0[2]]), "{:?}", point(&mut s, "pCube1.vtx[0]"));
    set(&mut s, "blendShape1.pCube2", 1.0);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1], v0[2]]));
}

#[test]
fn targets_are_live_until_deleted_then_baked() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]}));
    set(&mut s, "blendShape1.pCube2", 1.0);
    run(&mut s, "move", json!({"args": ["pCube2.vtx[0]"], "by": [2.0, 0.0, 0.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 3.0, v0[1], v0[2]]));
    // Local origin: moving the target as a whole does nothing.
    run(&mut s, "move", json!({"args": ["pCube2"], "by": [0.0, 5.0, 0.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 3.0, v0[1], v0[2]]));
    run(&mut s, "delete", json!({"args": ["pCube2"]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 3.0, v0[1], v0[2]]), "bakes as last seen");
    assert_eq!(run(&mut s, "blendShape", json!({"args": ["blendShape1"], "query": true, "queried": ["target"]})), json!([]));
    assert_eq!(get(&mut s, "blendShape1.pCube2"), 1.0);
    round_trip(&mut s);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 3.0, v0[1], v0[2]]));
}

#[test]
fn undoing_a_target_delete_brings_it_back_live() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]}));
    set(&mut s, "blendShape1.pCube2", 1.0);
    run(&mut s, "delete", json!({"args": ["pCube2"]}));
    s.undo();
    run(&mut s, "move", json!({"args": ["pCube2.vtx[0]"], "by": [1.0, 0.0, 0.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 2.0, v0[1], v0[2]]));
}

#[test]
fn world_origin_counts_the_targets_transform() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "move", json!({"args": ["pCube2"], "by": [0.0, 5.0, 0.0]}));
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"], "origin": "world", "weight": [0, 1.0]}));
    assert_eq!(run(&mut s, "blendShape", json!({"args": ["blendShape1"], "query": true, "queried": ["origin"]})), json!("world"));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1] + 5.0, v0[2]]));
}

#[test]
fn base_and_target_weights_scale_per_point() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"], "weight": [0, 1.0]}));
    set(&mut s, "blendShape1.inputTarget[0].baseWeights[0]", 0.5);
    assert_eq!(get(&mut s, "blendShape1.inputTarget[0].baseWeights[0]"), 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.5, v0[1], v0[2]]));
    set(&mut s, "blendShape1.inputTarget[0].inputTargetGroup[0].targetWeights[0]", 0.5);
    assert_eq!(get(&mut s, "blendShape1.inputTarget[0].inputTargetGroup[0].targetWeights[0]"), 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.25, v0[1], v0[2]]));
    round_trip(&mut s);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.25, v0[1], v0[2]]));
}

#[test]
fn weights_key_by_their_alias() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"]}));
    run(&mut s, "setKeyframe", json!({"args": ["blendShape1.pCube2"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["blendShape1.pCube2"], "time": 11, "value": 1.0}));
    assert_eq!(run(&mut s, "ls", json!({"type": "animCurve"})), json!(["blendShape1_pCube2"]));
    run(&mut s, "currentTime", json!({"args": [11]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1], v0[2]]));
}

#[test]
fn sculpt_target_writes_edits_of_the_base_into_the_target() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"], "weight": [0, 1.0]}));
    run(&mut s, "sculptTarget", json!({"args": ["blendShape1"], "edit": true, "target": 0}));
    assert_eq!(run(&mut s, "sculptTarget", json!({"args": ["blendShape1"], "query": true})), json!(0));
    run(&mut s, "move", json!({"args": ["pCube1.vtx[1]"], "by": [0.0, 3.0, 0.0]}));
    let v1 = point(&mut s, "pCube1.vtx[1]");
    run(&mut s, "sculptTarget", json!({"args": ["blendShape1"], "edit": true, "target": -1}));
    // The same as it was edited to, now by way of the target.
    assert!(near(point(&mut s, "pCube1.vtx[1]"), v1), "{:?} {:?}", point(&mut s, "pCube1.vtx[1]"), v1);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1], v0[2]]));
    set(&mut s, "blendShape1.pCube2", 0.0);
    assert!(near(point(&mut s, "pCube1.vtx[1]"), [v1[0], v1[1] - 3.0, v1[2]]));
    // Rebuilding makes a shape holding the target, live again.
    let made = run(&mut s, "sculptTarget", json!({"args": ["blendShape1"], "edit": true, "regenerate": true, "target": 0}));
    let name = made[0].as_str().unwrap().to_string();
    let t = point(&mut s, &format!("{name}.vtx[1]"));
    assert!((t[1] - v1[1]).abs() < 1e-6, "{t:?}");
}

#[test]
fn invert_shape_solves_through_the_deformers() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    // A cluster scaling and moving the mesh.
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.scale", 2.0, 2.0, 2.0]}));
    // The corrected look: the deformed mesh with one point lifted.
    run(&mut s, "polyCube", json!({"name": "fixed"}));
    run(&mut s, "setAttr", json!({"args": ["fixed.scale", 2.0, 2.0, 2.0]}));
    run(&mut s, "freezeTransforms", json!({"args": ["fixed"]}));
    run(&mut s, "move", json!({"args": ["fixed.vtx[0]"], "by": [0.0, 1.0, 0.0]}));
    let inv = run(&mut s, "invertShape", json!({"args": ["pCube1", "fixed"]}));
    let inv = inv.as_str().unwrap().to_string();
    // Scale 2 means a lift of 1 needs only 0.5 in front of the deformers.
    let rest = point(&mut s, &format!("{inv}.vtx[0]"));
    assert!(near(rest, [-0.5, 0.0, 0.5]), "{rest:?}");
    run(&mut s, "blendShape", json!({"args": [inv, "pCube1"], "frontOfChain": true, "weight": [0, 1.0]}));
    let got = point(&mut s, "pCube1.vtx[0]");
    let want = point(&mut s, "fixed.vtx[0]");
    assert!(near(got, want), "{got:?} {want:?}");
}

#[test]
fn front_of_chain_goes_before_the_skin() {
    let mut s = Session::new();
    two_cubes(&mut s);
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0.0, -1.0, 0.0], "name": "j1"}));
    run(&mut s, "skinCluster", json!({"args": ["j1", "pCube1"]}));
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"], "frontOfChain": true}));
    let h = run(&mut s, "listHistory", json!({"args": ["pCube1"], "pruneDagObjects": true}));
    let list: Vec<&str> = h.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    let (sk, bs) = (list.iter().position(|x| *x == "skinCluster1").unwrap(), list.iter().position(|x| *x == "blendShape1").unwrap());
    assert!(sk < bs, "{list:?}");
}

#[test]
fn parallel_adds_to_the_other_deformers() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.translateY", 2.0]}));
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"], "parallel": true, "weight": [0, 1.0]}));
    // Measured from the original, so the cluster's move isn't counted against the target.
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1] + 2.0, v0[2]]), "{:?}", point(&mut s, "pCube1.vtx[0]"));
}

#[test]
fn deformer_type_makes_blend_shapes_and_the_rest() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1"], "type": "jiggle"})), json!(["jiggle1"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1"], "type": "solidify"})), json!(["solidify1"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1"], "type": "textureDeformer"})), json!(["textureDeformer1"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1"], "type": "blendShape"})), json!(["blendShape1"]));
    run(&mut s, "curve", json!({"point": [[0.0, -1.0, 0.0], [0.0, 1.0, 0.0]], "degree": 1}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1", "curve1"], "type": "curveWarp"})), json!(["curveWarp1"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1", "curve1"], "type": "pointOnCurve"})), json!(["pointOnCurve1"]));
}

// ---------------------------------------------------------------------------------------------------------------------
// The other deformers

#[test]
fn sculpt_modes_push_points() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    let r = run(&mut s, "sculpt", json!({"args": ["pCube1"], "mode": "project", "maxDisplacement": 100.0}));
    assert_eq!(r, json!(["sculpt1", "sculptor1", "sculpt1StretchOrigin"]));
    assert_eq!(run(&mut s, "sculpt", json!({"args": ["sculpt1"], "query": true, "queried": ["mode"]})), json!("project"));
    assert_eq!(get(&mut s, "sculpt1.maximumDisplacement"), 100.0);
    // Sphere of radius 0.5 at the middle; corners project onto it.
    let radius = |p: [f64; 3]| (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
    let r0 = radius(point(&mut s, "pCube1.vtx[0]"));
    // The corner lies past the surface, by less than the dropoff, so it moves part way: more than none, not past the sphere.
    assert!(r0 < 0.75f64.sqrt() - 1e-3 && r0 > 0.5 - 1e-6, "{r0}");
    // A maximum displacement caps it.
    run(&mut s, "sculpt", json!({"args": ["sculpt1"], "edit": true, "maxDisplacement": 0.01}));
    let capped = radius(point(&mut s, "pCube1.vtx[0]"));
    assert!((0.75f64.sqrt() - capped).abs() <= 0.01 + 1e-9, "{capped}");
    // Stretch pushes out from the origin.
    run(&mut s, "sculpt", json!({"args": ["sculpt1"], "edit": true, "mode": "stretch", "maxDisplacement": 0.1, "dropoffDistance": 10.0}));
    assert!(radius(point(&mut s, "pCube1.vtx[0]")) > 0.75f64.sqrt());
    round_trip(&mut s);
    assert!(radius(point(&mut s, "pCube1.vtx[0]")) > 0.75f64.sqrt());
}

/// The y of a point over frames 1 to `n`, the cube keyed from 0 to 5 up over frames 1 to 5.
fn jiggle_run(s: &mut Session, n: i64) -> Vec<f64> {
    (1..=n)
        .map(|f| {
            run(s, "currentTime", json!({"args": [f]}));
            point(s, "pCube1.vtx[0]")[1]
        })
        .collect()
}

#[test]
fn jiggle_lags_overshoots_and_repeats() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.translateY"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1.translateY"], "time": 5, "value": 5.0}));
    assert_eq!(run(&mut s, "jiggle", json!({"args": ["pCube1"], "stiffness": 0.3, "damping": 0.3})), json!(["jiggle1"]));
    assert_eq!(get(&mut s, "jiggle1.stiffness"), 0.3);
    let a = jiggle_run(&mut s, 60);
    // Behind the mesh while it rises.
    assert!(a[3] < 4.0 - 0.5, "{:?}", &a[..6]);
    // Past it after it stops, and settled in the end.
    let top = a.iter().cloned().fold(f64::MIN, f64::max);
    assert!(top > 4.5, "{a:?}");
    assert!((a[59] - 4.5).abs() < 0.01, "{a:?}");
    // The same frames give the same answer, stepped again from the start.
    let b = jiggle_run(&mut s, 60);
    for (x, y) in a.iter().zip(&b) {
        assert!((x - y).abs() < 1e-9, "{a:?} {b:?}");
    }
    // Going back shows what was.
    run(&mut s, "currentTime", json!({"args": [4]}));
    assert!((point(&mut s, "pCube1.vtx[0]")[1] - a[3]).abs() < 1e-9);
    // Disabled it follows exactly.
    run(&mut s, "jiggle", json!({"args": ["jiggle1"], "edit": true, "enable": false}));
    run(&mut s, "currentTime", json!({"args": [5]}));
    assert!((point(&mut s, "pCube1.vtx[0]")[1] - 4.5).abs() < 1e-6);
}

#[test]
fn shrink_wrap_lands_on_the_target() {
    let mut s = Session::new();
    run(&mut s, "polySphere", json!({"radius": 2.0}));
    run(&mut s, "polyCube", json!({"width": 2.0, "height": 2.0, "depth": 2.0}));
    assert_eq!(run(&mut s, "shrinkWrap", json!({"args": ["pSphere1", "pCube1"], "projection": 2})), json!(["shrinkWrap1"]));
    let on_cube = |p: [f64; 3]| (p[0].abs().max(p[1].abs()).max(p[2].abs()) - 1.0).abs() < 1e-6;
    for v in [0, 37, 200] {
        let p = point(&mut s, &format!("pSphere1.vtx[{v}]"));
        assert!(on_cube(p), "{v}: {p:?}");
    }
    // An offset holds them off the surface.
    set(&mut s, "shrinkWrap1.offset", 0.5);
    let p = point(&mut s, "pSphere1.vtx[37]");
    assert!((p[0].abs().max(p[1].abs()).max(p[2].abs()) - 1.0).abs() > 0.05, "{p:?}");
    set(&mut s, "shrinkWrap1.offset", 0.0);
    // Off leaves the mesh alone.
    run(&mut s, "shrinkWrap", json!({"args": ["shrinkWrap1"], "edit": true, "projection": 0}));
    let p = point(&mut s, "pSphere1.vtx[37]");
    assert!(((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 2.0).abs() < 1e-6);
    // Closest point.
    run(&mut s, "shrinkWrap", json!({"args": ["shrinkWrap1"], "edit": true, "projection": 5}));
    assert!(on_cube(point(&mut s, "pSphere1.vtx[37]")));
    // Along the normals it casts outward and misses from out here: it stays, unless told to take the closest.
    run(&mut s, "shrinkWrap", json!({"args": ["shrinkWrap1"], "edit": true, "projection": 4}));
    let p = point(&mut s, "pSphere1.vtx[37]");
    assert!(((p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt() - 2.0).abs() < 1e-6, "{p:?}");
    run(&mut s, "shrinkWrap", json!({"args": ["shrinkWrap1"], "edit": true, "closestIfNoIntersection": true}));
    assert!(on_cube(point(&mut s, "pSphere1.vtx[37]")));
    // Reversed it hits from the outside.
    run(&mut s, "shrinkWrap", json!({"args": ["shrinkWrap1"], "edit": true, "closestIfNoIntersection": false, "reverse": true}));
    assert!(on_cube(point(&mut s, "pSphere1.vtx[37]")));
}

#[test]
fn morph_follows_its_target_with_scales() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    assert_eq!(run(&mut s, "morph", json!({"args": ["pCube1"], "morphTarget": "pCube2"})), json!(["morph1"]));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 1.0, v0[1], v0[2]]));
    set(&mut s, "morph1.envelope", 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.5, v0[1], v0[2]]));
    set(&mut s, "morph1.envelope", 1.0);
    set(&mut s, "morph1.normalScale", 0.0);
    set(&mut s, "morph1.tangentScale", 0.0);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), v0));
    assert!(s.run("morph", json!({"args": ["pCube1"]})).is_err());
}

#[test]
fn wrinkle_creases_within_its_reach() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"subdivisionsX": 20, "subdivisionsZ": 20, "width": 2.0, "depth": 2.0}));
    let r = run(&mut s, "wrinkle", json!({"args": ["pPlane1"], "wrinkleCount": 4, "wrinkleIntensity": 0.2, "dropoffDistance": 0.8, "randomness": 0.0, "style": "radial"}));
    assert_eq!(r, json!(["wrinkle1", "wrinkle1Handle"]));
    let ys: Vec<f64> = (0..441).map(|v| point(&mut s, &format!("pPlane1.vtx[{v}]"))[1]).collect();
    assert!(ys.iter().any(|y| y.abs() > 0.05), "some crease");
    // Corners are out of reach: flat.
    assert!(ys[0].abs() < 1e-9 && ys[440].abs() < 1e-9);
    assert!(ys.iter().all(|y| y.abs() <= 0.2 + 1e-9));
}

#[test]
fn proximity_wrap_follows_its_driver() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polyCube", json!({"name": "driver", "width": 1.5, "height": 1.5, "depth": 1.5}));
    assert_eq!(run(&mut s, "proximityWrap", json!({"args": ["pCube1"], "drivers": "driver"})), json!(["proximityWrap1"]));
    assert_eq!(run(&mut s, "proximityWrap", json!({"args": ["proximityWrap1"], "query": true, "queried": ["drivers"]})), json!(["driverShape"]));
    let before = point(&mut s, "pCube1.vtx[0]");
    assert!(near(before, [-0.5, -0.5, 0.5]));
    run(&mut s, "move", json!({"args": ["driver"], "by": [2.0, 0.0, 0.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [1.5, -0.5, 0.5]));
    set(&mut s, "proximityWrap1.envelope", 0.5);
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [0.5, -0.5, 0.5]));
    set(&mut s, "proximityWrap1.envelope", 1.0);
    // Snap goes onto the driver.
    run(&mut s, "proximityWrap", json!({"args": ["proximityWrap1"], "edit": true, "wrapMode": 2, "maxDrivers": 1}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [1.25, -0.75, 0.75]), "{:?}", point(&mut s, "pCube1.vtx[0]"));
}

#[test]
fn solidify_keeps_the_rest_shape() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1.vtx[0:3]"], "name": "pull"}));
    run(&mut s, "solidify", json!({"args": ["pCube1"], "scaleEnvelope": 0.0}));
    let edge = |s: &mut Session, a: &str, b: &str| {
        let (p, q) = (point(s, a), point(s, b));
        ((p[0] - q[0]).powi(2) + (p[1] - q[1]).powi(2) + (p[2] - q[2]).powi(2)).sqrt()
    };
    let diag = edge(&mut s, "pCube1.vtx[0]", "pCube1.vtx[5]");
    let side = edge(&mut s, "pCube1.vtx[0]", "pCube1.vtx[1]");
    run(&mut s, "setAttr", json!({"args": ["pullHandle.translateZ", 2.0]}));
    // The cluster stretches the mesh and solidify takes it back: every edge keeps its length.
    assert!((edge(&mut s, "pCube1.vtx[0]", "pCube1.vtx[5]") - diag).abs() < 1e-6);
    assert!((edge(&mut s, "pCube1.vtx[0]", "pCube1.vtx[1]") - side).abs() < 1e-6);
    // At zero envelope the stretch is there.
    set(&mut s, "solidify1.envelope", 0.0);
    assert!((edge(&mut s, "pCube1.vtx[0]", "pCube1.vtx[5]") - diag).abs() > 0.1);
}

#[test]
fn point_on_curve_moves_with_its_place() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "curve", json!({"point": [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]], "degree": 1}));
    run(&mut s, "pointOnCurveDeformer", json!({"args": ["pCube1", "curve1"], "parameter": 2.0}));
    let v0 = point(&mut s, "pCube1.vtx[0]");
    assert!(near(v0, [-0.5, -0.5, 0.5]));
    run(&mut s, "move", json!({"args": ["curve1.cv[2]"], "by": [0.0, 3.0, 0.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [-0.5, 2.5, 0.5]), "{:?}", point(&mut s, "pCube1.vtx[0]"));
    run(&mut s, "pointOnCurveDeformer", json!({"args": ["pointOnCurve1"], "edit": true, "parameter": 0.0}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), v0));
}

#[test]
fn texture_deformer_displaces_by_its_texture() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"subdivisionsX": 4, "subdivisionsZ": 4}));
    run(&mut s, "textureDeformer", json!({"args": ["pPlane1"], "noiseAmplitude": 0.0, "offset": 0.5}));
    assert!((point(&mut s, "pPlane1.vtx[0]")[1] - 0.5).abs() < 1e-6, "offset alone, along the normal");
    set(&mut s, "textureDeformer1.offset", 0.0);
    set(&mut s, "textureDeformer1.texture", 0.25);
    set(&mut s, "textureDeformer1.strength", 2.0);
    assert!((point(&mut s, "pPlane1.vtx[0]")[1] - 0.5).abs() < 1e-6, "the connected value by the strength");
    set(&mut s, "textureDeformer1.texture", 0.0);
    set(&mut s, "textureDeformer1.noiseAmplitude", 1.0);
    let ys: Vec<f64> = (0..25).map(|v| point(&mut s, &format!("pPlane1.vtx[{v}]"))[1]).collect();
    assert!(ys.iter().any(|y| (y - ys[0]).abs() > 1e-3), "the procedural varies");
    run(&mut s, "textureDeformer", json!({"args": ["textureDeformer1"], "edit": true, "direction": 1, "vectorStrength": [0.0, 0.0, 0.0], "vectorOffset": [1.0, 0.0, 0.0]}));
    assert!(near(point(&mut s, "pPlane1.vtx[0]"), [-0.5 + 1.0, 0.0, 0.5]));
}

#[test]
fn curve_warp_lays_the_mesh_along_the_curve() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"height": 4.0}));
    run(&mut s, "curve", json!({"point": [[0.0, -2.0, 0.0], [0.0, 2.0, 0.0]], "degree": 1}));
    run(&mut s, "curveWarp", json!({"args": ["pCube1", "curve1"]}));
    // A straight curve along the mesh changes nothing.
    let names: Vec<String> = (0..8).map(|v| format!("pCube1.vtx[{v}]")).collect();
    let now: Vec<[f64; 3]> = names.iter().map(|n| point(&mut s, n)).collect();
    for p in &now {
        assert!((p[0].abs() - 0.5).abs() < 1e-6 && (p[1].abs() - 2.0).abs() < 1e-6 && (p[2].abs() - 0.5).abs() < 1e-6, "{p:?}");
    }
    let top = (0..8).find(|&v| now[v][1] > 0.0).unwrap();
    // Half the length.
    set(&mut s, "curveWarp1.lengthScale", 0.5);
    assert!(point(&mut s, &names[top])[1].abs() < 1e-6);
    set(&mut s, "curveWarp1.lengthScale", 1.0);
    // Turn the curve to lie along X: the mesh goes with it.
    run(&mut s, "move", json!({"args": ["curve1.cv[1]"], "by": [4.0, -4.0, 0.0]}));
    let p = point(&mut s, &names[top]);
    assert!((p[0] - 4.0).abs() < 0.51 && (p[1] + 2.0).abs() < 0.51, "{p:?}");
    let bottom = (0..8).find(|&v| now[v][1] < 0.0).unwrap();
    let q = point(&mut s, &names[bottom]);
    assert!(q[0].abs() < 0.51 && (q[1] + 2.0).abs() < 0.51, "{q:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// Painting attributes

fn paint_ctx(s: &mut Session, call: Value) {
    let mut c = call.as_object().unwrap().clone();
    c.insert("edit".into(), json!(true));
    run(s, "artAttrCtx", Value::Object(c));
}

#[test]
fn paints_a_cluster_weights_and_they_take_effect() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "artAttrCtx", json!({"query": true, "queried": ["paintableAttrs"]})), json!(["cluster.cluster1.weights"]));
    paint_ctx(&mut s, json!({"attrSelected": "cluster.cluster1.weights", "value": 0.25, "radius": 0.1, "selectedattroper": "absolute", "stampProfile": "solid"}));
    assert_eq!(run(&mut s, "artAttrCtx", json!({"query": true, "queried": ["attrSelected"]})), json!("cluster.cluster1.weights"));
    assert_eq!(run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [-0.5, -0.5, 0.5]})), json!(1));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[0]"), 0.25);
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[1]"), 1.0);
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.translateY", 1.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [-0.5, -0.25, 0.5]));
    // One undo takes the stamp off.
    s.undo();
    s.undo();
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[0]"), 1.0);
    // Flood.
    paint_ctx(&mut s, json!({"value": 0.5, "clear": true}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[7]"), 0.5);
    paint_ctx(&mut s, json!({"selectedattroper": "additive", "value": 0.1, "clear": true}));
    assert!((get(&mut s, "cluster1.weightList[0].weights[7]") - 0.6).abs() < 1e-9);
    paint_ctx(&mut s, json!({"selectedattroper": "scale", "value": 0.5, "clear": true}));
    assert!((get(&mut s, "cluster1.weightList[0].weights[7]") - 0.3).abs() < 1e-9);
    round_trip(&mut s);
    assert!((get(&mut s, "cluster1.weightList[0].weights[7]") - 0.3).abs() < 1e-9);
}

#[test]
fn smooth_paint_evens_out_and_ctrl_inverts() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "softMod", json!({"args": ["pCube1"]}));
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    paint_ctx(&mut s, json!({"attrSelected": "softMod.softMod1.weights", "value": 0.0, "radius": 0.1, "stampProfile": "solid"}));
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [-0.5, -0.5, 0.5]}));
    assert_eq!(get(&mut s, "softMod1.weightList[0].weights[0]"), 0.0);
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [-0.5, -0.5, 0.5], "smooth": true}));
    assert_eq!(get(&mut s, "softMod1.weightList[0].weights[0]"), 1.0);
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [-0.5, -0.5, 0.5], "invert": true}));
    assert_eq!(get(&mut s, "softMod1.weightList[0].weights[0]"), 1.0);
}

#[test]
fn paints_blend_shape_target_and_base_weights() {
    let mut s = Session::new();
    let v0 = two_cubes(&mut s);
    run(&mut s, "blendShape", json!({"args": ["pCube2", "pCube1"], "weight": [0, 1.0]}));
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "artAttrCtx", json!({"query": true, "queried": ["paintableAttrs"]})), json!(["blendShape.blendShape1.baseWeights", "blendShape.blendShape1.targetWeights[0]"]));
    paint_ctx(&mut s, json!({"attrSelected": "blendShape.blendShape1.targetWeights[0]", "value": 0.5, "radius": 0.1, "stampProfile": "solid"}));
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [v0[0] + 1.0, v0[1], v0[2]]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.5, v0[1], v0[2]]));
    paint_ctx(&mut s, json!({"attrSelected": "blendShape.blendShape1.baseWeights", "value": 0.5}));
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [v0[0] + 0.5, v0[1], v0[2]]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [v0[0] + 0.25, v0[1], v0[2]]));
    paint_ctx(&mut s, json!({"attrSelected": "blendShape.blendShape1.paintTargetWeights", "value": 0.0}));
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [v0[0] + 0.25, v0[1], v0[2]]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), v0));
}

#[test]
fn every_weighted_deformer_offers_its_weights() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    run(&mut s, "deltaMush", json!({"args": ["pCube1"]}));
    run(&mut s, "jiggle", json!({"args": ["pCube1"]}));
    run(&mut s, "curve", json!({"point": [[0.0, -1.0, 0.0], [0.0, 1.0, 0.0]], "degree": 1}));
    run(&mut s, "wire", json!({"args": ["pCube1"], "wire": "curve1"}));
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    let list = run(&mut s, "artAttrCtx", json!({"query": true, "queried": ["paintableAttrs"]}));
    for want in ["ffd.ffd1.weights", "deltaMush.deltaMush1.weights", "jiggle.jiggle1.weights", "wire.wire1.weights"] {
        assert!(list.as_array().unwrap().iter().any(|x| x == want), "{want} in {list}");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Skin

#[test]
fn heat_bind_stays_inside_the_mesh_across_a_gap() {
    // Two cubes side by side joined by nothing near the top: a mesh of two boxes with a narrow gap. A bone inside each, the
    // left bone far nearer to the right box's inner wall than the right bone is.
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "left", "width": 1.0, "height": 1.0, "depth": 1.0, "subdivisionsX": 4, "subdivisionsY": 4, "subdivisionsZ": 4}));
    run(&mut s, "setAttr", json!({"args": ["left.translateX", -0.6]}));
    run(&mut s, "polyCube", json!({"name": "right", "width": 1.0, "height": 1.0, "depth": 1.0, "subdivisionsX": 4, "subdivisionsY": 4, "subdivisionsZ": 4}));
    run(&mut s, "setAttr", json!({"args": ["right.translateX", 0.6]}));
    let u = run(&mut s, "polyCombine", json!({"args": ["left", "right"], "name": "both"}));
    let mesh = u.as_array().map_or_else(|| u.as_str().unwrap().to_string(), |v| v[0].as_str().unwrap().to_string());
    run(&mut s, "select", json!({"clear": true}));
    // Inside the left box, against its inner wall. Inside the right box, against its outer wall.
    run(&mut s, "joint", json!({"position": [-0.2, -0.3, 0.0], "name": "a1"}));
    run(&mut s, "joint", json!({"position": [-0.2, 0.3, 0.0], "name": "a2"}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0.95, -0.3, 0.0], "name": "b1"}));
    run(&mut s, "joint", json!({"position": [0.95, 0.3, 0.0], "name": "b2"}));
    let bones = ["a1", "b1"];
    // Find a point on the right box's inner wall, at x = 0.1, in the middle.
    let info = run(&mut s, "polyEvaluate", json!({"args": [mesh.clone()], "vertex": true}));
    let count = info.as_i64().unwrap_or(0);
    let mut inner = None;
    for v in 0..count {
        let p = point(&mut s, &format!("{mesh}.vtx[{v}]"));
        if (p[0] - 0.1).abs() < 1e-6 && p[1].abs() < 1e-6 && p[2].abs() < 1e-6 {
            inner = Some(v);
        }
    }
    let inner = inner.expect("a point mid-wall");
    let weight_of = |s: &mut Session, sk: &str, j: &str| -> f64 {
        run(s, "skinPercent", json!({"args": [sk, format!("{mesh}.vtx[{inner}]")], "query": true, "transform": j})).as_f64().unwrap()
    };
    let close = run(&mut s, "skinCluster", json!({"args": [bones[0], "a2", bones[1], "b2", mesh.clone()], "toSelectedBones": true, "name": "close", "bindMethod": 0, "dropoffRate": 4.0}));
    assert_eq!(close, json!(["close"]));
    let by_distance = weight_of(&mut s, "close", "b1") + weight_of(&mut s, "close", "b2");
    assert!(by_distance < 0.5, "closest distance gives it to the wrong box: {by_distance}");
    run(&mut s, "skinCluster", json!({"args": [mesh.clone()], "edit": true, "unbind": true}));
    let heat = run(&mut s, "skinCluster", json!({"args": [bones[0], "a2", bones[1], "b2", mesh.clone()], "toSelectedBones": true, "name": "hot", "bindMethod": 2, "heatmapFalloff": 1.0}));
    assert_eq!(heat, json!(["hot"]));
    assert_eq!(run(&mut s, "skinCluster", json!({"args": ["hot"], "query": true, "queried": ["bindMethod"]})), json!(2));
    let by_heat = weight_of(&mut s, "hot", "b1") + weight_of(&mut s, "hot", "b2");
    assert!(by_heat > 0.99, "heat stays on its own surface: {by_heat}");
    // Each point sums to 1.
    let vals = run(&mut s, "skinPercent", json!({"args": ["hot", format!("{mesh}.vtx[0]")], "query": true, "value": true}));
    let sum: f64 = vals.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).sum();
    assert!((sum - 1.0).abs() < 1e-6, "{sum}");
}

#[test]
fn heat_spreads_smoothly_on_one_surface_and_falloff_tightens_it() {
    let bind = |falloff: f64| -> Vec<f64> {
        let mut s = Session::new();
        run(&mut s, "polyCylinder", json!({"height": 4.0, "subdivisionsHeight": 8, "subdivisionsAxis": 8}));
        run(&mut s, "select", json!({"clear": true}));
        run(&mut s, "joint", json!({"position": [0.0, -2.0, 0.0], "name": "j1"}));
        run(&mut s, "joint", json!({"position": [0.0, 0.0, 0.0], "name": "j2"}));
        run(&mut s, "joint", json!({"position": [0.0, 2.0, 0.0], "name": "j3"}));
        run(&mut s, "skinCluster", json!({"args": ["j1", "pCylinder1"], "bindMethod": 2, "heatmapFalloff": falloff, "maximumInfluences": 3}));
        (0..72)
            .map(|v| {
                let vals = run(&mut s, "skinPercent", json!({"args": ["skinCluster1", format!("pCylinder1.vtx[{v}]")], "query": true, "transform": "j1"}));
                vals.as_f64().unwrap()
            })
            .collect()
    };
    let wide = bind(1.0);
    let tight = bind(0.0);
    // Graded along the height, not a step.
    assert!(wide.iter().any(|w| *w > 0.05 && *w < 0.95), "{wide:?}");
    // Tighter weights are nearer 0 or 1.
    let spread = |w: &[f64]| w.iter().map(|x| x.min(1.0 - x)).sum::<f64>();
    assert!(spread(&tight) < spread(&wide));
}

fn skinned(s: &mut Session) {
    run(s, "polyCube", json!({"height": 2.0, "subdivisionsY": 2}));
    run(s, "select", json!({"clear": true}));
    run(s, "joint", json!({"position": [0.0, -1.0, 0.0], "name": "j1"}));
    run(s, "joint", json!({"position": [0.0, 0.0, 0.0], "name": "j2"}));
    run(s, "joint", json!({"position": [0.0, 1.0, 0.0], "name": "j3"}));
    run(s, "skinCluster", json!({"args": ["j1", "pCube1"]}));
}

fn weights(s: &mut Session, p: i64, j: &str) -> f64 {
    run(s, "skinPercent", json!({"args": ["skinCluster1", format!("pCube1.vtx[{p}]")], "query": true, "transform": j})).as_f64().unwrap()
}

#[test]
fn deformer_weights_round_trip_through_xml_and_json() {
    let dir = std::env::temp_dir().join(format!("aztek_deform_three_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.to_string_lossy().to_string();
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["j1", 0.25], ["j2", 0.75]], "zeroRemainingInfluences": true}));
    for file in ["w.xml", "w.json"] {
        let r = run(&mut s, "deformerWeights", json!({"args": [file], "export": true, "deformer": "skinCluster1", "path": path}));
        assert!(r.as_str().unwrap().ends_with(file));
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        if file == "w.xml" {
            assert!(text.starts_with("<?xml version=\"1.0\"?>\n<deformerWeight>"), "{text}");
            assert!(text.contains("<shape name=\"pCubeShape1\" group=\"0\" stride=\"3\" size=\"12\" max=\"11\">"), "{text}");
            assert!(text.contains("<weights deformer=\"skinCluster1\" source=\"j1\" shape=\"pCubeShape1\" layer=\"0\""), "{text}");
            assert!(text.contains("<point index=\"0\" value=\"0.250\"/>"), "{text}");
        } else {
            let v: Value = serde_json::from_str(&text).unwrap();
            assert_eq!(v["deformerWeight"]["weights"][0]["source"], json!("j1"));
        }
        // Wreck the weights, then read them back.
        run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["j3", 1.0]], "zeroRemainingInfluences": true}));
        assert_eq!(weights(&mut s, 0, "j3"), 1.0);
        run(&mut s, "deformerWeights", json!({"args": [file], "import": true, "deformer": "skinCluster1", "path": path, "method": "index"}));
        assert!((weights(&mut s, 0, "j1") - 0.25).abs() < 1e-3);
        assert!((weights(&mut s, 0, "j2") - 0.75).abs() < 1e-3);
        assert_eq!(weights(&mut s, 0, "j3"), 0.0);
        // The import undoes as one.
        s.undo();
        assert_eq!(weights(&mut s, 0, "j3"), 1.0);
        run(&mut s, "deformerWeights", json!({"args": [file], "import": true, "deformer": "skinCluster1", "path": path, "method": "nearest"}));
        assert!((weights(&mut s, 0, "j1") - 0.25).abs() < 1e-3);
    }
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn deformer_weights_methods_on_other_deformers() {
    let dir = std::env::temp_dir().join(format!("aztek_deform_three_b_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.to_string_lossy().to_string();
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[3]"], "value": 0.5}));
    run(&mut s, "deformerWeights", json!({"args": ["c.xml"], "export": true, "deformer": "cluster1", "path": path, "weightTolerance": 0.6}));
    // What the tolerance left out takes the default, with index.
    run(&mut s, "deformerWeights", json!({"args": ["c.xml"], "import": true, "deformer": "cluster1", "path": path, "defaultValue": 0.1}));
    assert!((get(&mut s, "cluster1.weightList[0].weights[3]") - 0.1).abs() < 1e-9);
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[2]"), 1.0);
    // With over it stays what it is.
    run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[3]"], "value": 0.7}));
    run(&mut s, "deformerWeights", json!({"args": ["c.xml"], "import": true, "deformer": "cluster1", "path": path, "method": "over"}));
    assert!((get(&mut s, "cluster1.weightList[0].weights[3]") - 0.7).abs() < 1e-9);
    // Barycentric and bilinear share among the nearest points; on a stored point they take it exactly.
    run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[3]"], "value": 0.2}));
    run(&mut s, "deformerWeights", json!({"args": ["c.xml"], "import": true, "deformer": "cluster1", "path": path, "method": "barycentric"}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[2]"), 1.0);
    run(&mut s, "deformerWeights", json!({"args": ["c.xml"], "import": true, "deformer": "cluster1", "path": path, "method": "bilinear"}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[2]"), 1.0);
    assert!(s.run("deformerWeights", json!({"args": ["c.xml"], "import": true, "deformer": "cluster1", "path": path, "method": "guess"})).is_err());
    std::fs::remove_dir_all(dir).ok();
}

#[test]
fn weight_blended_skinning_goes_between_the_two() {
    let posed = |method: i64, blend: Option<f64>| -> [f64; 3] {
        let mut s = Session::new();
        skinned(&mut s);
        run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[3]"], "transformValue": [["j2", 0.5], ["j3", 0.5]], "zeroRemainingInfluences": true}));
        run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "skinMethod": method}));
        if let Some(b) = blend {
            run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[3]"], "dqBlendWeight": b}));
        }
        run(&mut s, "setAttr", json!({"args": ["j2.rotateZ", 120.0]}));
        point(&mut s, "pCube1.vtx[3]")
    };
    let (linear, dual) = (posed(0, None), posed(1, None));
    assert!(!near(linear, dual));
    assert!(near(posed(2, Some(0.0)), linear));
    assert!(near(posed(2, Some(1.0)), dual));
    let half = posed(2, Some(0.5));
    assert!(near(half, [(linear[0] + dual[0]) * 0.5, (linear[1] + dual[1]) * 0.5, (linear[2] + dual[2]) * 0.5]));
}

#[test]
fn blend_weights_read_and_write_as_a_plug() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[1]"], "dqBlendWeight": 0.25}));
    assert_eq!(get(&mut s, "skinCluster1.blendWeights[1]"), 0.25);
    assert_eq!(get(&mut s, "skinCluster1.bw[0]"), 0.0);
    set(&mut s, "skinCluster1.blendWeights[2]", 0.5);
    assert_eq!(get(&mut s, "skinCluster1.blendWeights[2]"), 0.5);
    round_trip(&mut s);
    let bw = aztek_scene::skin::skin_named(&s.graph, "skinCluster1").unwrap();
    let data = s.graph.node(bw).unwrap().data();
    let i = data.ty().attr_index("blendWeights").unwrap();
    let PlugValueData(v) = PlugValueData::of(data, i);
    assert_eq!(v[1], 0.25);
}

struct PlugValueData(Vec<f64>);
impl PlugValueData {
    fn of(d: &aztek_scene::NodeData, i: usize) -> PlugValueData {
        match d.value(i) {
            aztek_scene::PlugValue::Data(Some(x)) => PlugValueData(x.as_any().downcast_ref::<aztek_scene::deform::Floats>().unwrap().0.clone()),
            _ => PlugValueData(Vec::new()),
        }
    }
}

#[test]
fn hammer_takes_the_weights_around_it() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"subdivisionsX": 2, "subdivisionsZ": 2}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [-1.0, 0.0, 0.0], "name": "j1"}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [1.0, 0.0, 0.0], "name": "j2"}));
    run(&mut s, "skinCluster", json!({"args": ["j1", "j2", "pPlane1"], "toSelectedBones": true}));
    for v in 0..9 {
        run(&mut s, "skinPercent", json!({"args": ["skinCluster1", format!("pPlane1.vtx[{v}]")], "transformValue": [["j1", 1.0]], "zeroRemainingInfluences": true}));
    }
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pPlane1.vtx[4]"], "transformValue": [["j2", 1.0]], "zeroRemainingInfluences": true}));
    run(&mut s, "weightHammerVerts", json!({"args": ["pPlane1.vtx[4]"]}));
    let w = run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pPlane1.vtx[4]"], "query": true, "transform": "j1"}));
    assert!((w.as_f64().unwrap() - 1.0).abs() < 1e-6, "{w}");
}

#[test]
fn transfer_moves_weight_between_influences() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["j1", 0.8], ["j2", 0.2]], "zeroRemainingInfluences": true}));
    run(&mut s, "transferBetweenInfluences", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "source": "j1", "destination": "j3", "value": 0.5}));
    assert!((weights(&mut s, 0, "j1") - 0.4).abs() < 1e-9);
    assert!((weights(&mut s, 0, "j2") - 0.2).abs() < 1e-9);
    assert!((weights(&mut s, 0, "j3") - 0.4).abs() < 1e-9);
    s.undo();
    assert_eq!(weights(&mut s, 0, "j3"), 0.0);
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "influence": "j3", "lockWeights": true}));
    assert!(s.run("transferBetweenInfluences", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "source": "j1", "destination": "j3"})).is_err());
}

#[test]
fn a_mesh_can_be_an_influence_by_its_distance() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "polySphere", json!({"radius": 0.3, "name": "blob"}));
    run(&mut s, "setAttr", json!({"args": ["blob.translateY", 1.0]}));
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "addInfluence": "blob", "useGeometry": true, "dropoffRate": 2.0}));
    let list = run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "influence": true}));
    assert!(list.as_array().unwrap().iter().any(|x| x == "blob"), "{list}");
    // Top points nearer the blob take more of it than the bottom ones.
    let (bottom, top) = (weights(&mut s, 0, "blob"), weights(&mut s, 4, "blob"));
    assert!(top > bottom, "{top} {bottom}");
    let vals = run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "query": true, "value": true}));
    let sum: f64 = vals.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).sum();
    assert!((sum - 1.0).abs() < 1e-6);
    // Moving the blob moves points by the weight.
    let before = point(&mut s, "pCube1.vtx[4]");
    run(&mut s, "setAttr", json!({"args": ["blob.translateX", 1.0]}));
    let after = point(&mut s, "pCube1.vtx[4]");
    assert!((after[0] - before[0] - top).abs() < 1e-6);
}

#[test]
fn substitute_geometry_takes_over_the_skin() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "polyCube", json!({"height": 2.0, "subdivisionsY": 4, "name": "fine"}));
    let r = run(&mut s, "substituteGeometry", json!({"args": ["pCube1", "fine"]}));
    assert_eq!(run(&mut s, "findRelatedSkinCluster", json!({"args": ["fine"]})), r[0]);
    assert!(s.graph.find("pCube1").unwrap().is_none(), "the old mesh goes");
    let before = point(&mut s, "fine.vtx[0]");
    run(&mut s, "setAttr", json!({"args": ["j1.translateX", 1.0]}));
    let after = point(&mut s, "fine.vtx[0]");
    assert!(after[0] > before[0] + 0.5, "{before:?} {after:?}");
}

#[test]
fn move_joints_mode_and_remove_unused() {
    let mut s = Session::new();
    skinned(&mut s);
    let before = point(&mut s, "pCube1.vtx[4]");
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "moveJointsMode": true}));
    assert_eq!(run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "queried": ["moveJointsMode"]})), json!(true));
    run(&mut s, "setAttr", json!({"args": ["j1.translateX", 0.3]}));
    assert!(near(point(&mut s, "pCube1.vtx[4]"), before));
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "moveJointsMode": false}));
    assert!(near(point(&mut s, "pCube1.vtx[4]"), before), "where they are is the new bind");
    run(&mut s, "setAttr", json!({"args": ["j1.translateX", 1.3]}));
    assert!(!near(point(&mut s, "pCube1.vtx[4]"), before), "and it deforms again");
    run(&mut s, "setAttr", json!({"args": ["j1.translateX", 0.3]}));
    // Clear j3 from every point; it goes.
    for v in 0..12 {
        run(&mut s, "skinPercent", json!({"args": ["skinCluster1", format!("pCube1.vtx[{v}]")], "transformValue": [["j1", 1.0]], "zeroRemainingInfluences": true}));
    }
    let gone = run(&mut s, "removeUnusedInfluences", json!({"args": ["pCube1"]}));
    assert!(gone.as_array().unwrap().iter().any(|x| x == "j3"), "{gone}");
    let list = run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "query": true, "influence": true}));
    assert_eq!(list, json!(["j1"]));
}

#[test]
fn normalize_post_keeps_what_was_set_and_normalizes_when_deforming() {
    let mut s = Session::new();
    skinned(&mut s);
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "normalizeWeights": 2}));
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[0]"], "transformValue": [["j1", 1.0], ["j2", 1.0]], "zeroRemainingInfluences": true}));
    assert_eq!(weights(&mut s, 0, "j1"), 1.0);
    assert_eq!(weights(&mut s, 0, "j2"), 1.0);
    let before = point(&mut s, "pCube1.vtx[0]");
    run(&mut s, "setAttr", json!({"args": ["j1.translateX", 2.0]}));
    // j2 and j3 are children of j1, so all follow it: only a point on j1 alone would differ. Half and half of the same move.
    assert!((point(&mut s, "pCube1.vtx[0]")[0] - before[0] - 2.0).abs() < 1e-6);
    run(&mut s, "setAttr", json!({"args": ["j1.translateX", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["j2.translateX", 2.0]}));
    // Normalized: j2 at 1 of 2 carries half of its move, not all.
    assert!((point(&mut s, "pCube1.vtx[0]")[0] - before[0] - 1.0).abs() < 1e-6, "{:?} {before:?}", point(&mut s, "pCube1.vtx[0]"));
    // Back to interactive and edits share out again.
    run(&mut s, "skinCluster", json!({"args": ["skinCluster1"], "edit": true, "normalizeWeights": 1}));
    run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[1]"], "transformValue": [["j1", 0.5], ["j2", 0.5]], "zeroRemainingInfluences": true}));
    let vals = run(&mut s, "skinPercent", json!({"args": ["skinCluster1", "pCube1.vtx[1]"], "query": true, "value": true}));
    let sum: f64 = vals.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).sum();
    assert!((sum - 1.0).abs() < 1e-6);
}

#[test]
fn copies_weights_by_uv() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"subdivisionsX": 2, "subdivisionsZ": 2, "name": "from"}));
    run(&mut s, "polyPlane", json!({"subdivisionsX": 2, "subdivisionsZ": 2, "name": "to"}));
    // The destination far away in space, but the same in UV.
    run(&mut s, "setAttr", json!({"args": ["to.translateX", 50.0]}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [-1.0, 0.0, 0.0], "name": "j1"}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [1.0, 0.0, 0.0], "name": "j2"}));
    let a = run(&mut s, "skinCluster", json!({"args": ["j1", "j2", "from"], "toSelectedBones": true, "name": "skinA"}));
    let b = run(&mut s, "skinCluster", json!({"args": ["j1", "j2", "to"], "toSelectedBones": true, "name": "skinB"}));
    assert_eq!((a, b), (json!(["skinA"]), json!(["skinB"])));
    run(&mut s, "skinPercent", json!({"args": ["skinA", "from.vtx[2]"], "transformValue": [["j1", 0.3], ["j2", 0.7]], "zeroRemainingInfluences": true}));
    run(&mut s, "copySkinWeights", json!({"sourceSkin": "skinA", "destinationSkin": "skinB", "noMirror": true, "uvSpace": ["map1", "map1"], "influenceAssociation": ["oneToOne"]}));
    let w = run(&mut s, "skinPercent", json!({"args": ["skinB", "to.vtx[2]"], "query": true, "transform": "j2"})).as_f64().unwrap();
    assert!((w - 0.7).abs() < 1e-5, "{w}");
}

// ---------------------------------------------------------------------------------------------------------------------
// The original shape, and deformer sets

#[test]
fn deforming_makes_the_intermediate_original() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    assert!(s.graph.find("pCubeShape1Orig").unwrap().is_none());
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    assert_eq!(get_bool(&mut s, "pCubeShape1Orig.intermediateObject"), true);
    assert_eq!(get_bool(&mut s, "pCubeShape1.intermediateObject"), false);
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["pCubeShape1Orig"]})), json!("mesh"));
    // One only, however many deformers.
    run(&mut s, "softMod", json!({"args": ["pCube1"]}));
    let io = run(&mut s, "ls", json!({"intermediateObjects": true}));
    assert_eq!(io, json!(["pCubeShape1Orig"]));
    assert_eq!(run(&mut s, "ls", json!({"args": ["pCubeShape1Orig"], "noIntermediate": true})), json!([]));
    // The chain runs from what made the mesh, through the original.
    let h = run(&mut s, "listHistory", json!({"args": ["pCube1"]}));
    let list: Vec<&str> = h.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert_eq!(list.first(), Some(&"pCubeShape1"));
    let (orig, cube) = (list.iter().position(|x| *x == "pCubeShape1Orig").unwrap(), list.iter().position(|x| *x == "polyCube1").unwrap());
    assert!(orig < cube, "{list:?}");
    assert!(list.iter().position(|x| *x == "cluster1").unwrap() < orig, "{list:?}");
    // Pruned: only what isn't in the DAG.
    let pruned = run(&mut s, "listHistory", json!({"args": ["pCube1"], "pruneDagObjects": true}));
    assert!(pruned.as_array().unwrap().iter().all(|x| x != "pCubeShape1" && x != "pCubeShape1Orig"), "{pruned}");
    // The visible shape stays the shape.
    assert_eq!(run(&mut s, "deformableShape", json!({"args": ["pCube1"], "originalGeometry": true})), json!(["pCubeShape1Orig.worldMesh"]));
    assert_eq!(run(&mut s, "deformableShape", json!({"args": ["pCube1"], "localShapeInAttr": true})), json!(["inMesh"]));
    // Undo takes it away with the deformers.
    s.undo();
    s.undo();
    assert!(s.graph.find("pCubeShape1Orig").unwrap().is_none());
    // The original feeds what comes after a save.
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    round_trip(&mut s);
    assert_eq!(get_bool(&mut s, "pCubeShape1Orig.intermediateObject"), true);
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.translateY", 1.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [-0.5, 0.5, 0.5]));
    // Editing the original's generator still comes through.
    run(&mut s, "setAttr", json!({"args": ["polyCube1.width", 3.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [-1.5, 0.5, 0.5]), "{:?}", point(&mut s, "pCube1.vtx[0]"));
}

fn get_bool(s: &mut Session, plug: &str) -> bool {
    run(s, "getAttr", json!({"args": [plug]})).as_bool().unwrap()
}

#[test]
fn the_original_doesnt_get_drawn_or_copied_as_a_shape() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    let (m, _) = (aztek_scene::components::as_mesh(&s.graph, s.graph.find("pCube1").unwrap().unwrap()).unwrap(), 0);
    assert_eq!(s.graph.path(m), "pCubeShape1");
    // delete -ch bakes and takes the original.
    run(&mut s, "delete", json!({"args": ["pCube1"], "constructionHistory": true}));
    assert!(s.graph.find("pCubeShape1Orig").unwrap().is_none());
    assert!(s.graph.find("cluster1").unwrap().is_none());
    assert!(s.graph.find("cluster1Set").unwrap().is_none());
}

#[test]
fn a_skin_has_an_original_too() {
    let mut s = Session::new();
    skinned(&mut s);
    assert_eq!(get_bool(&mut s, "pCubeShape1Orig.intermediateObject"), true);
    assert_eq!(run(&mut s, "listHistory", json!({"args": ["pCube1"], "type": "skinCluster"})), json!(["skinCluster1"]));
}

#[test]
fn deformer_sets_are_real_membership() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    let rest: Vec<[f64; 3]> = (0..8).map(|v| point(&mut s, &format!("pCube1.vtx[{v}]"))).collect();
    let up = |v: usize| [rest[v][0], rest[v][1] + 1.0, rest[v][2]];
    run(&mut s, "cluster", json!({"args": ["pCube1.vtx[0:3]"]}));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["cluster1Set"]})), json!("objectSet"));
    assert_eq!(run(&mut s, "sets", json!({"args": ["cluster1Set"], "query": true})), json!(["pCube1.vtx[0:3]"]));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.translateY", 1.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), up(0)));
    assert!(near(point(&mut s, "pCube1.vtx[4]"), rest[4]), "not in the set, doesn't move");
    assert!(near(point(&mut s, "pCube1.vtx[6]"), rest[6]), "not in the set, doesn't move");
    // Take one out.
    run(&mut s, "sets", json!({"args": ["pCube1.vtx[0]"], "remove": "cluster1Set"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["cluster1Set"], "query": true})), json!(["pCube1.vtx[1:3]"]));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), rest[0]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["pCube1.vtx[0]"], "isMember": "cluster1Set"})), json!(false));
    assert_eq!(run(&mut s, "sets", json!({"args": ["pCube1.vtx[1]"], "isMember": "cluster1Set"})), json!(true));
    // Undo puts it back and its weight.
    s.undo();
    assert!(near(point(&mut s, "pCube1.vtx[0]"), up(0)));
    // Put another in: it moves.
    run(&mut s, "sets", json!({"args": ["pCube1.vtx[6]"], "addElement": "cluster1Set"}));
    assert!(near(point(&mut s, "pCube1.vtx[6]"), up(6)));
    // percent and the brush can't give weight outside the set.
    run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[7]"], "value": 1.0}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[7]"), 0.0);
    run(&mut s, "select", json!({"args": ["pCube1"]}));
    run(&mut s, "artAttrCtx", json!({"edit": true, "attrSelected": "cluster.cluster1.weights", "value": 1.0, "radius": 5.0, "stampProfile": "solid"}));
    run(&mut s, "attrPaintStamp", json!({"args": ["pCube1"], "position": [0.0, 0.0, 0.0]}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[7]"), 0.0);
    // deformer -e -g points in and -rm out.
    run(&mut s, "deformer", json!({"args": ["cluster1"], "edit": true, "geometry": "pCube1.vtx[7]"}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[7]"), 1.0);
    run(&mut s, "deformer", json!({"args": ["cluster1"], "edit": true, "remove": true, "geometry": "pCube1.vtx[7]"}));
    assert_eq!(get(&mut s, "cluster1.weightList[0].weights[7]"), 0.0);
    // A set survives saving, still doing its job.
    round_trip(&mut s);
    run(&mut s, "sets", json!({"args": ["pCube1.vtx[1]"], "remove": "cluster1Set"}));
    assert!(near(point(&mut s, "pCube1.vtx[1]"), rest[1]));
    // It goes with its deformer.
    run(&mut s, "delete", json!({"args": ["cluster1"]}));
    assert!(s.graph.find("cluster1Set").unwrap().is_none());
    s.undo();
    assert!(s.graph.find("cluster1Set").unwrap().is_some());
}

#[test]
fn membership_works_for_the_other_deformers_and_new_geometry() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polyCube", json!({}));
    let rest: Vec<[f64; 3]> = (0..8).map(|v| point(&mut s, &format!("pCube1.vtx[{v}]"))).collect();
    let moving = (0..8).find(|&v| rest[v][1] > 0.0).unwrap();
    let id = format!("pCube1.vtx[{moving}]");
    run(&mut s, "nonLinear", json!({"args": ["pCube1"], "type": "twist", "endAngle": 90.0}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["twist1Set"], "query": true})), json!(["pCube1.vtx[0:7]"]));
    let moved = point(&mut s, &id);
    assert!(!near(moved, rest[moving]));
    run(&mut s, "sets", json!({"args": [id.clone()], "remove": "twist1Set"}));
    assert!(near(point(&mut s, &id), rest[moving]));
    // Adding a point of another mesh adds the mesh to the deformer, and only that point.
    run(&mut s, "sets", json!({"args": ["pCube2.vtx[3]"], "addElement": "twist1Set"}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["twist1"], "query": true, "queried": ["geometry"]})), json!(["pCubeShape1", "pCubeShape2"]));
    let held = run(&mut s, "sets", json!({"args": ["twist1Set"], "query": true}));
    assert!(held.as_array().unwrap().iter().any(|x| x == "pCube2.vtx[3]"), "{held}");
    assert_eq!(held.as_array().unwrap().len(), 3, "{held}");
    assert!(near(point(&mut s, "pCube2.vtx[1]"), rest[1]));
    // One deformer on two meshes has one set.
    assert_eq!(run(&mut s, "ls", json!({"type": "objectSet"})), json!(["twist1Set"]));
    // A plain set is left alone by all this.
    assert_eq!(run(&mut s, "sets", json!({"args": ["pCube1"], "name": "plain"})), json!("plain"));
    assert_eq!(run(&mut s, "sets", json!({"args": ["plain"], "query": true})), json!(["pCube1"]));
}
