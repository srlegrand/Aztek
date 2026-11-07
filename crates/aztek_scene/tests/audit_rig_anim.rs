//! Rigging and animation commands: flags, modes and return values from the reference's documentation.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}
fn close(v: &Value, want: f64) {
    assert!((v.as_f64().unwrap_or_else(|| panic!("{v}")) - want).abs() < 1e-5, "got {v}, wanted {want}");
}
fn close3(v: &Value, want: [f64; 3]) {
    for i in 0..3 {
        assert!((v[i].as_f64().unwrap() - want[i]).abs() < 1e-5, "got {v}, wanted {want:?}");
    }
}
fn temp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("aztek_audit_rig_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name).to_string_lossy().to_string()
}

fn chain(s: &mut Session) {
    run(s, "select", json!({"clear": true}));
    run(s, "joint", json!({"p": [0.0, -2.0, 0.0], "name": "j0"}));
    run(s, "joint", json!({"p": [0.0, 0.0, 0.0], "name": "j1"}));
    run(s, "joint", json!({"p": [0.0, 2.0, 0.0], "name": "j2"}));
    run(s, "polyCylinder", json!({"name": "c", "h": 4.0, "sy": 4}));
}

#[test]
fn rigid_bind_is_one_influence_per_point() {
    let mut s = Session::new();
    chain(&mut s);
    assert_eq!(run(&mut s, "bindSkin", json!({"args": ["j0", "c"]})), 1);
    let sk = run(&mut s, "findRelatedSkinCluster", json!({"args": ["c"]}));
    assert!(sk.as_str().is_some_and(|x| !x.is_empty()), "{sk}");
    let w = run(&mut s, "skinPercent", json!({"args": [sk, "c.vtx[0]"], "q": true, "v": true}));
    let vals: Vec<f64> = w.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert_eq!(vals.iter().filter(|x| **x > 1e-9).count(), 1, "{vals:?}");
    assert!(vals.iter().any(|x| (*x - 1.0).abs() < 1e-9));
    run(&mut s, "bindSkin", json!({"args": ["c"], "unbind": true}));
    assert_eq!(run(&mut s, "findRelatedSkinCluster", json!({"args": ["c"]})), "");
}

#[test]
fn geom_bind_runs_heat() {
    let mut s = Session::new();
    chain(&mut s);
    let sk = run(&mut s, "skinCluster", json!({"args": ["j0", "c"], "mi": 1}));
    let sk = sk[0].as_str().unwrap().to_string();
    run(&mut s, "geomBind", json!({"args": [sk.clone()], "bm": 3, "mi": 3}));
    let w = run(&mut s, "skinPercent", json!({"args": [sk, "c.vtx[0]"], "q": true, "v": true}));
    let total: f64 = w.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).sum();
    assert!((total - 1.0).abs() < 1e-6);
}

#[test]
fn pair_blend_on_translate() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "l"}));
    run(&mut s, "setAttr", json!({"args": ["l.tx", 2.0]}));
    let pb = run(&mut s, "pairBlend", json!({"nd": "l", "at": ["tx"]}));
    let pb = pb.as_str().unwrap().to_string();
    // Weight 1 is input 2, which reads 0; 0 is what was there.
    close(&get(&mut s, "l.tx"), 0.0);
    run(&mut s, "setAttr", json!({"args": [format!("{pb}.weight"), 0.0]}));
    close(&get(&mut s, "l.tx"), 2.0);
    let at = run(&mut s, "pairBlend", json!({"args": [pb], "q": true, "queried": ["attribute"]}));
    assert_eq!(at, json!(["l.translateX"]));
}

#[test]
fn match_transform() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "xform", json!({"args": ["b"], "t": [1.0, 2.0, 3.0], "ro": [0.0, 90.0, 0.0], "s": [2.0, 2.0, 2.0]}));
    run(&mut s, "matchTransform", json!({"args": ["a", "b"], "pos": true}));
    close3(&get(&mut s, "a.translate"), [1.0, 2.0, 3.0]);
    close3(&get(&mut s, "a.rotate"), [0.0, 0.0, 0.0]);
    run(&mut s, "matchTransform", json!({"args": ["a", "b"], "rot": true}));
    close3(&get(&mut s, "a.rotate"), [0.0, 90.0, 0.0]);
    close3(&get(&mut s, "a.scale"), [1.0, 1.0, 1.0]);
    run(&mut s, "matchTransform", json!({"args": ["a", "b"], "scl": true}));
    close3(&get(&mut s, "a.scale"), [2.0, 2.0, 2.0]);
    run(&mut s, "undo", json!({}));
    close3(&get(&mut s, "a.scale"), [1.0, 1.0, 1.0]);
}

#[test]
fn inherit_transform_preserving() {
    let mut s = Session::new();
    run(&mut s, "group", json!({"em": true, "name": "g"}));
    run(&mut s, "setAttr", json!({"args": ["g.tx", 5.0]}));
    run(&mut s, "spaceLocator", json!({"name": "l"}));
    run(&mut s, "setAttr", json!({"args": ["l.tx", 1.0]}));
    run(&mut s, "parent", json!({"args": ["l", "g"]}));
    close3(&get(&mut s, "l.translate"), [-4.0, 0.0, 0.0]);
    assert_eq!(run(&mut s, "inheritTransform", json!({"args": ["l"], "q": true})), json!([true]));
    run(&mut s, "inheritTransform", json!({"args": ["l"], "p": true}));
    assert_eq!(run(&mut s, "inheritTransform", json!({"args": ["l"], "q": true})), json!([false]));
    // Still at the world position it had.
    close3(&get(&mut s, "l.translate"), [1.0, 0.0, 0.0]);
}

#[test]
fn containers() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    assert_eq!(run(&mut s, "container", json!({"name": "box", "an": ["a", "b"]})), "box");
    assert_eq!(run(&mut s, "container", json!({"args": ["box"], "q": true, "queried": ["nodeList"]})), json!(["a", "b"]));
    assert_eq!(run(&mut s, "container", json!({"args": ["a"], "q": true, "queried": ["findContainer"]})), "box");
    run(&mut s, "container", json!({"args": ["box"], "e": true, "rn": ["a"]}));
    assert_eq!(run(&mut s, "container", json!({"args": ["box"], "q": true, "queried": ["nodeList"]})), json!(["b"]));
    run(&mut s, "rename", json!({"args": ["b", "c"]}));
    assert_eq!(run(&mut s, "container", json!({"args": ["box"], "q": true, "queried": ["nodeList"]})), json!(["c"]));
    let path = temp("box.ma");
    run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
    let mut t = Session::new();
    run(&mut t, "file", json!({"args": [path], "open": true}));
    assert_eq!(run(&mut t, "nodeType", json!({"args": ["box"]})), "container");
}

#[test]
fn controllers() {
    let mut s = Session::new();
    run(&mut s, "circle", json!({"name": "root"}));
    run(&mut s, "circle", json!({"name": "arm"}));
    run(&mut s, "controller", json!({"args": ["root"]}));
    run(&mut s, "controller", json!({"args": ["arm"], "p": "root"}));
    assert_eq!(run(&mut s, "controller", json!({"args": ["arm"], "q": true, "queried": ["parent"]})), "root");
    assert_eq!(run(&mut s, "controller", json!({"args": ["root"], "q": true, "queried": ["children"]})), json!(["arm"]));
    assert_eq!(run(&mut s, "controller", json!({"allControllers": true, "queried": ["allControllers"]})), json!(["root", "arm"]));
    assert_eq!(run(&mut s, "controller", json!({"args": ["arm"], "isController": true, "queried": ["isController"]})), true);
}

#[test]
fn distance_dimension() {
    let mut s = Session::new();
    let sh = run(&mut s, "distanceDimension", json!({"sp": [0.0, 0.0, 0.0], "ep": [3.0, 4.0, 0.0]}));
    assert_eq!(sh, "distanceDimensionShape1");
    close(&get(&mut s, "distanceDimensionShape1.distance"), 5.0);
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["distanceDimensionShape1"]})), "distanceDimShape");
    assert_eq!(run(&mut s, "objExists", json!({"args": ["locator2"]})), true);
    // The locators drive it.
    run(&mut s, "setAttr", json!({"args": ["locator2.tx", 0.0]}));
    close(&get(&mut s, "distanceDimensionShape1.distance"), 4.0);
    // An existing locator is reused.
    run(&mut s, "distanceDimension", json!({"sp": [0.0, 0.0, 0.0], "ep": [1.0, 0.0, 0.0]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["locator4"]})), false);
    let path = temp("dd.ma");
    run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
    let mut t = Session::new();
    run(&mut t, "file", json!({"args": [path], "open": true}));
    close(&get(&mut t, "distanceDimensionShape1.distance"), 4.0);
    run(&mut t, "setAttr", json!({"args": ["locator2.ty", 0.0]}));
    close(&get(&mut t, "distanceDimensionShape1.distance"), 0.0);
}

#[test]
fn annotate_and_param_dimension() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "l"}));
    let sh = run(&mut s, "annotate", json!({"args": ["l"], "tx": "hello", "p": [1.0, 2.0, 3.0]}));
    let sh = sh.as_str().unwrap().to_string();
    assert_eq!(get(&mut s, &format!("{sh}.text")), "hello");
    close3(&get(&mut s, "annotation1.translate"), [1.0, 2.0, 3.0]);
    run(&mut s, "curve", json!({"name": "cv", "d": 1, "p": [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]]}));
    let d = run(&mut s, "paramDimension", json!({"args": ["cv.u[0.5]"]}));
    close(&get(&mut s, &format!("{}.uParamValue", d.as_str().unwrap())), 0.5);
    close3(&get(&mut s, "paramDimension1.translate"), [5.0, 0.0, 0.0]);
    let l = run(&mut s, "dropoffLocator", json!({"args": [1.0, 50.0, "cv.u[0.5]"]}));
    assert_eq!(l, json!(["dropoffLocator1"]));
}

#[test]
fn deformers_found_and_weights_copied() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "a", "sx": 1, "sy": 1}));
    run(&mut s, "polyPlane", json!({"name": "b", "sx": 1, "sy": 1}));
    let c1 = run(&mut s, "cluster", json!({"args": ["a"]}));
    let c2 = run(&mut s, "cluster", json!({"args": ["b"]}));
    let (c1, c2) = (c1[0].as_str().unwrap().to_string(), c2[0].as_str().unwrap().to_string());
    assert_eq!(run(&mut s, "findDeformers", json!({"args": ["a"]})), json!([c1.clone()]));
    run(&mut s, "percent", json!({"args": [c1.clone(), "a.vtx[0]"], "v": 0.25}));
    run(&mut s, "copyDeformerWeights", json!({"sd": c1, "dd": c2.clone(), "ss": "a", "ds": "b"}));
    close(&run(&mut s, "percent", json!({"args": [c2.clone(), "b.vtx[0]"], "q": true}))[0], 0.25);
    close(&run(&mut s, "percent", json!({"args": [c2, "b.vtx[1]"], "q": true}))[0], 1.0);
}

fn keyed_cube(s: &mut Session) {
    run(s, "polyCube", json!({"name": "a"}));
    run(s, "setKeyframe", json!({"args": ["a.tx"], "t": 1, "v": 0.0, "itt": "linear", "ott": "linear"}));
    run(s, "setKeyframe", json!({"args": ["a.tx"], "t": 11, "v": 10.0, "itt": "linear", "ott": "linear"}));
}

fn at_frame(s: &mut Session, f: f64, plug: &str) -> f64 {
    run(s, "currentTime", json!({"args": [f]}));
    get(s, plug).as_f64().unwrap()
}

#[test]
fn time_warp_moves_a_key() {
    let mut s = Session::new();
    keyed_cube(&mut s);
    let tw = run(&mut s, "timeWarp", json!({"args": ["a"], "f": [1.0, 11.0]}));
    let tw = tw.as_str().unwrap().to_string();
    assert_eq!(run(&mut s, "nodeType", json!({"args": [tw.clone()]})), "timeWarp");
    assert_eq!(run(&mut s, "timeWarp", json!({"args": [tw.clone()], "q": true, "queried": ["frame"]})), json!([1.0, 11.0]));
    assert!((at_frame(&mut s, 6.0, "a.tx") - 5.0).abs() < 1e-6);
    // The key at 11 now plays at 6, so frame 6 reads its value.
    run(&mut s, "timeWarp", json!({"args": [tw.clone()], "e": true, "mf": [1, 6.0]}));
    assert!((at_frame(&mut s, 6.0, "a.tx") - 10.0).abs() < 1e-6);
    assert!((at_frame(&mut s, 3.5, "a.tx") - 5.0).abs() < 1e-6);
    run(&mut s, "setAttr", json!({"args": [format!("{tw}.apply"), false]}));
    assert!((at_frame(&mut s, 6.0, "a.tx") - 5.0).abs() < 1e-6);
}

#[test]
fn set_key_path_builds_a_path() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "l"}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    let first = run(&mut s, "setKeyPath", json!({"args": ["l"]}));
    let (curve, mp) = (first[0].as_str().unwrap().to_string(), first[1].as_str().unwrap().to_string());
    assert_eq!(run(&mut s, "nodeType", json!({"args": [mp]})), "motionPath");
    run(&mut s, "currentTime", json!({"args": [10]}));
    // The path holds the object; move it with the path's own curve point by keying a new place.
    run(&mut s, "disconnectAttr", json!({"args": ["motionPath1.allCoordinates", "l.translate"]}));
    run(&mut s, "setAttr", json!({"args": ["l.translate", 4.0, 0.0, 0.0]}));
    run(&mut s, "connectAttr", json!({"args": ["motionPath1.allCoordinates", "l.translate"]}));
    let _ = curve;
    assert_eq!(run(&mut s, "objExists", json!({"args": ["motionPath1"]})), true);
}

#[test]
fn aliases_and_settings() {
    let mut s = Session::new();
    keyed_cube(&mut s);
    assert_eq!(run(&mut s, "selectKeyframe", json!({"args": ["a.tx"], "t": "1:1"})), 1);
    run(&mut s, "tickDrawSpecial", json!({"args": ["a.tx"], "t": "11:11"}));
    assert_eq!(run(&mut s, "keyframe", json!({"args": ["a.tx"], "q": true, "t": "11:11", "tds": true, "queried": ["tickDrawSpecial"]})), json!([true]));
    assert_eq!(run(&mut s, "animDisplay", json!({"q": true, "queried": ["modelUpdate"]})), "interactive");
    run(&mut s, "animDisplay", json!({"upd": "none"}));
    assert_eq!(run(&mut s, "animDisplay", json!({"q": true, "queried": ["modelUpdate"]})), "none");
    run(&mut s, "recordAttr", json!({"args": ["a"], "at": ["tx"]}));
    assert_eq!(run(&mut s, "recordAttr", json!({"args": ["a"], "q": true})), json!(["a.tx"]));
    run(&mut s, "recordAttr", json!({"args": ["a"], "at": ["tx"], "d": true}));
    assert_eq!(run(&mut s, "recordAttr", json!({"args": ["a"], "q": true})), json!([]));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "pointConstraint", json!({"args": ["a", "b"]}));
    run(&mut s, "bakeSimulation", json!({"args": ["b.tx"], "t": "1:11"}));
    run(&mut s, "delete", json!({"args": ["b_pointConstraint1"]}));
    assert!((at_frame(&mut s, 6.0, "b.tx") - 5.0).abs() < 1e-6);
}

#[test]
fn keying_group_is_a_set() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    assert_eq!(run(&mut s, "keyingGroup", json!({"args": ["a"], "n": "kg"})), "kg");
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["kg"]})), "keyingGroup");
    assert_eq!(run(&mut s, "ls", json!({"type": "objectSet"})).as_array().unwrap().iter().any(|x| x == "kg"), true);
    assert_eq!(run(&mut s, "sets", json!({"args": ["kg"], "q": true})), json!(["a"]));
}

fn character_with_clips(s: &mut Session) {
    run(s, "polyCube", json!({"name": "a"}));
    run(s, "character", json!({"args": ["a.tx"], "name": "ch"}));
    run(s, "setKeyframe", json!({"args": ["a.tx"], "t": 1, "v": 0.0}));
    run(s, "setKeyframe", json!({"args": ["a.tx"], "t": 5, "v": 4.0}));
    run(s, "setKeyframe", json!({"args": ["a.tx"], "t": 11, "v": 100.0}));
    run(s, "setKeyframe", json!({"args": ["a.tx"], "t": 15, "v": 104.0}));
    run(s, "clip", json!({"args": ["ch"], "name": "one", "st": 1.0, "et": 5.0}));
    run(s, "clip", json!({"args": ["ch"], "name": "two", "st": 11.0, "et": 15.0}));
}

#[test]
fn bake_clip_into_one() {
    let mut s = Session::new();
    character_with_clips(&mut s);
    let before = at_frame(&mut s, 3.0, "a.tx");
    let name = run(&mut s, "bakeClip", json!({"args": ["ch"], "name": "both"}));
    assert_eq!(name, "both");
    assert_eq!(run(&mut s, "clip", json!({"args": ["ch"], "q": true, "queried": ["allClips"]})), json!(["both"]));
    assert!((at_frame(&mut s, 3.0, "a.tx") - before).abs() < 1e-4);
}

#[test]
fn clip_matching_shifts_the_second() {
    let mut s = Session::new();
    character_with_clips(&mut s);
    let before = at_frame(&mut s, 15.0, "a.tx");
    assert!((before - 104.0).abs() < 1e-4, "before {before}");
    run(&mut s, "clipMatching", json!({"args": ["ch"], "cs": ["one", 5.0], "cd": ["two", 11.0], "mt": 1}));
    // The second clip starts from where the first ended: 4.
    let v = at_frame(&mut s, 11.0, "a.tx");
    assert!((v - 4.0).abs() < 1e-4, "{v}");
    let v = at_frame(&mut s, 15.0, "a.tx");
    assert!((v - 8.0).abs() < 1e-4, "{v}");
}

#[test]
fn character_map_pairs_by_name() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "character", json!({"args": ["a.tx", "a.ty"], "name": "c1"}));
    run(&mut s, "character", json!({"args": ["b.tx", "b.ry"], "name": "c2"}));
    assert_eq!(run(&mut s, "characterMap", json!({"args": ["c1", "c2"], "pm": true})), json!(["a.translateX b.translateX"]));
    let m = run(&mut s, "characterMap", json!({"args": ["c1", "c2"]}));
    assert_eq!(run(&mut s, "characterMap", json!({"args": [m[0].clone()], "q": true})), json!(["a.translateX b.translateX"]));
    run(&mut s, "characterMap", json!({"args": [m[0].clone()], "e": true, "ma": ["a.translateY", "b.rotateY"]}));
    assert_eq!(run(&mut s, "characterMap", json!({"args": [m[0].clone()], "q": true})).as_array().unwrap().len(), 2);
}
