//! Pull and push: Edit As Scene Data, Merge to USD, discard, duplicate, MayaReference prims and driving transforms.

use aztek_scene::session::Session;
use glam::DVec3;
use serde_json::{json, Value};

const ROOT: &str = r#"#usda 1.0
(
    defaultPrim = "World"
)
def Xform "World"
{
    double3 xformOp:translate = (10, 0, 0)
    uniform token[] xformOpOrder = ["xformOp:translate"]
    def Mesh "tri"
    {
        double3 xformOp:translate = (0, 5, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
        int[] faceVertexCounts = [3]
        int[] faceVertexIndices = [0, 1, 2]
        point3f[] points = [(0,0,0),(1,0,0),(0,1,0)]
    }
}
"#;

fn setup(tag: &str) -> (Session, String, std::path::PathBuf) {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    let dir = std::env::temp_dir().join(format!("aztek_usd_translate_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("root.usda");
    std::fs::write(&f, ROOT).unwrap();
    let shape = s.run("mayaUsd_createStageFromFile", json!({"args": [f.to_string_lossy()]})).unwrap();
    (s, shape.as_str().unwrap().to_string(), dir)
}

fn v3(v: &Value) -> DVec3 {
    let a = v.as_array().unwrap();
    DVec3::new(a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap())
}

fn world(s: &mut Session, n: &str) -> DVec3 {
    v3(&s.run("xform", json!({"args": [n], "query": true, "worldSpace": true, "translation": true})).unwrap())
}

#[test]
fn pull_hides_the_prim_and_keeps_its_place_then_merge_writes_back() {
    let (mut s, shape, _dir) = setup("pull");
    let tri = format!("{shape},/World/tri");
    let top = s.run("mayaUsdEditAsMaya", json!({"args": [tri]})).unwrap();
    let top = top.as_str().unwrap().to_string();
    assert_eq!(world(&mut s, &top), DVec3::new(10.0, 5.0, 0.0), "the node stands where the prim did");
    let ex = s.run("getAttr", json!({"args": [format!("{shape}.excludePrimPaths")]})).unwrap();
    assert_eq!(ex, json!("/World/tri"));

    // Edit it as a node, then merge.
    s.run("setAttr", json!({"args": [format!("{top}.ty"), 7.0]})).unwrap();
    s.run("move", json!({"args": [format!("{top}.vtx[0]")], "by": [0.0, 0.0, 2.0]})).ok();
    let back = s.run("mayaUsdMergeToUsd", json!({"args": [top]})).unwrap();
    assert!(back.as_str().unwrap().ends_with(",/World/tri"));
    assert!(s.graph.find(&top).unwrap().is_none(), "the nodes are gone");
    let ex = s.run("getAttr", json!({"args": [format!("{shape}.excludePrimPaths")]})).unwrap();
    assert_eq!(ex, json!(""));
    let t = v3(&s.run("getAttr", json!({"args": [format!("{tri}.xformOp:translate")]})).unwrap());
    assert_eq!(t, DVec3::new(0.0, 7.0, 0.0));
    assert_eq!(s.run("getAttr", json!({"args": [format!("{tri}.xformOpOrder")]})).unwrap(), json!(["xformOp:translate"]));
    let info = s.run("usdPrim", json!({"args": [tri], "query": true})).unwrap();
    assert_eq!(info["type"], json!("Mesh"));

    // Merge undoes as one with its nodes.
    s.undo();
    assert!(s.graph.find(&top).unwrap().is_some());
    let t = v3(&s.run("getAttr", json!({"args": [format!("{tri}.xformOp:translate")]})).unwrap());
    assert_eq!(t, DVec3::new(0.0, 5.0, 0.0));
    assert_eq!(s.run("getAttr", json!({"args": [format!("{shape}.excludePrimPaths")]})).unwrap(), json!("/World/tri"));
    s.redo();
    assert!(s.graph.find(&top).unwrap().is_none());
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{tri}.xformOp:translate")]})).unwrap()), DVec3::new(0.0, 7.0, 0.0));
}

#[test]
fn discard_restores_and_undo_of_pull() {
    let (mut s, shape, _dir) = setup("discard");
    let tri = format!("{shape},/World/tri");
    let top = s.run("mayaUsdEditAsMaya", json!({"args": [tri]})).unwrap().as_str().unwrap().to_string();
    s.run("setAttr", json!({"args": [format!("{top}.ty"), 99.0]})).unwrap();
    s.run("mayaUsdDiscardEdits", json!({"args": [top]})).unwrap();
    assert!(s.graph.find(&top).unwrap().is_none());
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{tri}.xformOp:translate")]})).unwrap()), DVec3::new(0.0, 5.0, 0.0));
    assert_eq!(s.run("getAttr", json!({"args": [format!("{shape}.excludePrimPaths")]})).unwrap(), json!(""));
}

#[test]
fn duplicate_both_ways() {
    let (mut s, shape, _dir) = setup("dup");
    let tri = format!("{shape},/World/tri");
    let copy = s.run("mayaUsdDuplicate", json!({"args": [tri, ""]})).unwrap().as_str().unwrap().to_string();
    assert_eq!(world(&mut s, &copy), DVec3::new(10.0, 5.0, 0.0));
    assert_eq!(s.run("getAttr", json!({"args": [format!("{shape}.excludePrimPaths")]})).unwrap(), json!(""), "a copy hides nothing");
    assert_eq!(s.run("polyEvaluate", json!({"args": [copy], "face": true})).unwrap(), json!(1));

    s.run("polyCube", json!({"name": "box"})).unwrap();
    let made = s.run("mayaUsdDuplicate", json!({"args": ["box", format!("{shape},/World")]})).unwrap();
    assert!(made.as_str().unwrap().ends_with(",/World/box"));
    let info = s.run("usdPrim", json!({"args": [made], "query": true})).unwrap();
    assert_eq!(info["type"], json!("Mesh"));
    assert!(s.graph.find("box").unwrap().is_some(), "the node stays");
    s.undo();
    assert_eq!(s.run("usdPrim", json!({"args": [made], "query": true, "exists": true})).unwrap(), json!(false));
}

#[test]
fn al_translate_prim() {
    let (mut s, shape, _dir) = setup("al");
    let r = s.run("AL_usdmaya_TranslatePrim", json!({"proxy": shape, "importPaths": "/World/tri"})).unwrap();
    let top = r[0].as_str().unwrap().to_string();
    assert!(s.graph.find(&top).unwrap().is_some());
    s.run("AL_usdmaya_TranslatePrim", json!({"proxy": shape, "teardownPaths": "/World/tri"})).unwrap();
    assert!(s.graph.find(&top).unwrap().is_none());
}

#[test]
fn the_reference_reference_prim_loads_and_caches() {
    let (mut s, shape, dir) = setup("ref");
    // A scene file to reference.
    let mut other = Session::new();
    aztek_scene::cameras::make_defaults(&mut other.graph);
    other.run("polyCube", json!({"name": "body"})).unwrap();
    let ma = dir.join("rig.ma").to_string_lossy().into_owned();
    other.run("file", json!({"args": [ma], "exportAll": true, "type": "mayaAscii"})).unwrap();

    let p = format!("{shape},/World/rig");
    s.run("usdPrim", json!({"args": [p], "define": "MayaReference"})).unwrap();
    s.run("usdAttr", json!({"args": [format!("{p}.mayaReference")], "value": ma, "type": "asset"})).unwrap();
    s.run("usdAttr", json!({"args": [format!("{p}.mayaNamespace")], "value": "rig", "type": "string"})).unwrap();
    let group = s.run("mayaUsdLoadMayaReference", json!({"args": [p]})).unwrap().as_str().unwrap().to_string();
    assert_eq!(world(&mut s, &group), DVec3::new(10.0, 0.0, 0.0));
    assert!(s.graph.find("rig:body").unwrap().is_some());
    assert_eq!(s.run("getAttr", json!({"args": [format!("{shape}.excludePrimPaths")]})).unwrap(), json!("/World/rig"));

    let cache = dir.join("cache.usda").to_string_lossy().into_owned();
    s.run("mayaUsdCacheMayaReference", json!({"args": [group], "file": cache})).unwrap();
    assert!(std::fs::read_to_string(&cache).unwrap().contains("Mesh"));
    let info = s.run("usdPrim", json!({"args": [p], "query": true})).unwrap();
    assert_eq!(info["references"][0]["asset"], json!(cache));
}

#[test]
fn transform_drives_a_prim_through_the_session_layer() {
    let (mut s, shape, _dir) = setup("drive");
    let tri = format!("{shape},/World/tri");
    s.run("spaceLocator", json!({"name": "ctrl"})).unwrap();
    s.run("setAttr", json!({"args": ["ctrl.tx", 13.0]})).unwrap();
    s.run("usdDriveFromNode", json!({"args": [tri], "driver": "ctrl"})).unwrap();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{tri}.xformOp:translate")]})).unwrap()), DVec3::new(3.0, 0.0, 0.0), "world 13 under a parent at 10");
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[0]["dirty"], json!(true), "in the session layer");
    assert_eq!(t[1]["dirty"], json!(false), "not the root");
}
