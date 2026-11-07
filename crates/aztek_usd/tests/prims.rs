//! Prims through the scene's commands: names, selection, transforms, variants, payloads, and undo of all of it.

use aztek_scene::session::Session;
use aztek_usd::stages;
use glam::DVec3;
use openusd::gf;
use serde_json::{json, Value};

const ROOT: &str = r#"#usda 1.0
(
    defaultPrim = "World"
    startTimeCode = 1
    endTimeCode = 5
)
def Xform "World"
{
    def Xform "a"
    {
        double3 xformOp:translate = (1, 0, 0)
        uniform token[] xformOpOrder = ["xformOp:translate"]
        def Sphere "ball"
        {
            double radius = 2
        }
    }
    def Xform "odd"
    {
        float xformOp:rotateY = 90
        uniform token[] xformOpOrder = ["xformOp:rotateY"]
    }
    def Xform "anim"
    {
        double3 xformOp:translate.timeSamples = {
            1: (0, 0, 0),
            5: (4, 0, 0),
        }
        uniform token[] xformOpOrder = ["xformOp:translate"]
    }
    def Xform "varied" (
        variants = {
            string shape = "one"
        }
        prepend variantSets = "shape"
    )
    {
        variantSet "shape" = {
            "one" {
                def Cube "inOne"
                {
                }
            }
            "two" {
                def Sphere "inTwo"
                {
                }
            }
        }
    }
    def Xform "heavy" (
        prepend payload = @./asset.usda@
    )
    {
    }
}
"#;

const ASSET: &str = r#"#usda 1.0
(
    defaultPrim = "Asset"
)
def Xform "Asset"
{
    def Cube "geo"
    {
    }
}
"#;

fn setup(tag: &str) -> (Session, String) {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    let dir = std::env::temp_dir().join(format!("aztek_usd_prims_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let f = dir.join("root.usda");
    std::fs::write(&f, ROOT).unwrap();
    std::fs::write(dir.join("asset.usda"), ASSET).unwrap();
    let shape = s.run("mayaUsd_createStageFromFile", json!({"args": [f.to_string_lossy()]})).unwrap();
    (s, shape.as_str().unwrap().to_string())
}

fn v3(v: &Value) -> DVec3 {
    let a = v.as_array().unwrap();
    DVec3::new(a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap())
}

#[test]
fn selection_holds_prims_beside_nodes_and_undoes() {
    let (mut s, shape) = setup("sel");
    s.run("polyCube", json!({"name": "box"})).unwrap();
    let ball = format!("{shape},/World/a/ball");
    s.run("select", json!({"args": [ball]})).unwrap();
    let full = s.run("ls", json!({"selection": true})).unwrap();
    assert_eq!(full.as_array().unwrap().len(), 1);
    assert!(full[0].as_str().unwrap().ends_with(",/World/a/ball"), "{full}");
    s.run("select", json!({"args": ["box"], "add": true})).unwrap();
    assert_eq!(s.run("ls", json!({"selection": true})).unwrap().as_array().unwrap().len(), 2);
    s.run("select", json!({"args": ["box"]})).unwrap();
    assert_eq!(s.run("ls", json!({"selection": true})).unwrap(), json!(["box"]));
    s.undo();
    assert_eq!(s.run("ls", json!({"selection": true})).unwrap().as_array().unwrap().len(), 2);
    s.run("select", json!({"clear": true})).unwrap();
    assert_eq!(s.run("ls", json!({"selection": true})).unwrap(), json!([]));
}

#[test]
fn move_authors_on_the_edit_target_and_undoes() {
    let (mut s, shape) = setup("move");
    let name = format!("{shape},/World/a");
    s.run("move", json!({"args": [name], "by": [1.0, 2.0, 0.0]})).unwrap();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{name}.xformOp:translate")]})).unwrap()), DVec3::new(2.0, 2.0, 0.0));
    s.undo();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{name}.xformOp:translate")]})).unwrap()), DVec3::new(1.0, 0.0, 0.0));
    s.redo();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{name}.xformOp:translate")]})).unwrap()), DVec3::new(2.0, 2.0, 0.0));
    // A child under a moved proxy moves in world space: the parent's transform is taken out.
    let id = stages::proxy_named(&s.graph, &shape).unwrap();
    let tr = s.graph.parent(id);
    let trn = s.graph.path(tr);
    s.run("setAttr", json!({"args": [format!("{trn}.tx"), 100.0]})).unwrap();
    let ball = format!("{shape},/World/a/ball");
    s.run("move", json!({"args": [ball], "by": [0.0, 0.0, 3.0]})).unwrap();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{ball}.xformOp:translate")]})).unwrap()), DVec3::new(0.0, 0.0, 3.0));
}

#[test]
fn rotate_and_scale_keep_channels_and_other_stacks_become_a_matrix() {
    let (mut s, shape) = setup("rot");
    let a = format!("{shape},/World/a");
    s.run("rotate", json!({"args": [a], "by": [0.0, 90.0, 0.0]})).unwrap();
    let r = v3(&s.run("getAttr", json!({"args": [format!("{a}.xformOp:rotateXYZ")]})).unwrap());
    assert!((r.y - 90.0).abs() < 1e-3, "{r}");
    let order = s.run("getAttr", json!({"args": [format!("{a}.xformOpOrder")]})).unwrap();
    assert_eq!(order, json!(["xformOp:translate", "xformOp:rotateXYZ"]));
    s.run("scale", json!({"args": [a], "by": [2.0, 2.0, 2.0]})).unwrap();
    let c = v3(&s.run("getAttr", json!({"args": [format!("{a}.xformOp:scale")]})).unwrap());
    assert!((c - DVec3::splat(2.0)).length() < 1e-4, "{c}");
    let odd = format!("{shape},/World/odd");
    s.run("move", json!({"args": [odd], "by": [5.0, 0.0, 0.0]})).unwrap();
    assert_eq!(s.run("getAttr", json!({"args": [format!("{odd}.xformOpOrder")]})).unwrap(), json!(["xformOp:transform"]));
}

#[test]
fn sampled_attributes_take_a_sample_at_the_current_time() {
    let (mut s, shape) = setup("anim");
    let a = format!("{shape},/World/anim");
    s.run("currentTime", json!({"args": [3.0]})).unwrap();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{a}.xformOp:translate")]})).unwrap()), DVec3::new(2.0, 0.0, 0.0));
    s.run("move", json!({"args": [a], "by": [0.0, 1.0, 0.0]})).unwrap();
    let times = s.run("usdAttr", json!({"args": [format!("{a}.xformOp:translate")], "query": true, "timeSamples": true})).unwrap();
    assert_eq!(times, json!([1.0, 3.0, 5.0]));
    s.undo();
    let times = s.run("usdAttr", json!({"args": [format!("{a}.xformOp:translate")], "query": true, "timeSamples": true})).unwrap();
    assert_eq!(times, json!([1.0, 5.0]));
}

#[test]
fn variants_payloads_and_activation() {
    let (mut s, shape) = setup("var");
    let v = format!("{shape},/World/varied");
    let info = s.run("usdPrim", json!({"args": [v], "query": true})).unwrap();
    assert_eq!(info["variantSets"][0]["name"], json!("shape"));
    assert_eq!(info["variantSets"][0]["selection"], json!("one"));
    assert_eq!(info["variantSets"][0]["options"], json!(["one", "two"]));
    let kids = |s: &mut Session, p: &str| s.run("usdPrim", json!({"args": [p], "query": true, "children": true})).unwrap();
    assert_eq!(kids(&mut s, &v)[0]["name"], json!("inOne"));
    s.run("usdPrim", json!({"args": [v], "setVariant": ["shape", "two"]})).unwrap();
    assert_eq!(kids(&mut s, &v)[0]["name"], json!("inTwo"));
    s.undo();
    assert_eq!(kids(&mut s, &v)[0]["name"], json!("inOne"));

    let h = format!("{shape},/World/heavy");
    assert_eq!(s.run("usdPrim", json!({"args": [h], "query": true})).unwrap()["loaded"], json!(true));
    assert!(s.run("usdPrim", json!({"args": [h], "query": true})).unwrap()["hasPayload"].as_bool().unwrap());
    s.run("usdPrim", json!({"args": [h], "unload": true})).unwrap();
    assert_eq!(s.run("usdPrim", json!({"args": [h], "query": true})).unwrap()["loaded"], json!(false));
    s.undo();
    assert_eq!(s.run("usdPrim", json!({"args": [h], "query": true})).unwrap()["loaded"], json!(true));

    let b = format!("{shape},/World/a/ball");
    s.run("usdPrim", json!({"args": [b], "active": false})).unwrap();
    assert_eq!(s.run("usdPrim", json!({"args": [b], "query": true})).unwrap()["active"], json!(false));
    s.undo();
    assert_eq!(s.run("usdPrim", json!({"args": [b], "query": true})).unwrap()["active"], json!(true));
}

#[test]
fn add_rename_reparent_delete_and_references() {
    let (mut s, shape) = setup("ns");
    let n = format!("{shape},/World/made");
    assert_eq!(s.run("usdPrim", json!({"args": [n], "define": "Cube"})).unwrap().as_str().unwrap().ends_with(",/World/made"), true);
    let renamed = s.run("rename", json!({"args": [n, "called"]})).unwrap();
    assert!(renamed.as_str().unwrap().ends_with(",/World/called"), "{renamed}");
    let exists = |s: &mut Session, p: &str| s.run("usdPrim", json!({"args": [format!("{shape},{p}")], "query": true, "exists": true})).unwrap();
    assert_eq!(exists(&mut s, "/World/called"), json!(true));
    assert_eq!(exists(&mut s, "/World/made"), json!(false));
    s.undo();
    assert_eq!(exists(&mut s, "/World/made"), json!(true));
    s.redo();
    let moved = s.run("usdPrim", json!({"args": [format!("{shape},/World/called")], "reparent": "/World/a"})).unwrap();
    assert!(moved.as_str().unwrap().ends_with(",/World/a/called"));
    s.run("delete", json!({"args": [format!("{shape},/World/a/called")]})).unwrap();
    assert_eq!(exists(&mut s, "/World/a/called"), json!(false));
    s.undo();
    assert_eq!(exists(&mut s, "/World/a/called"), json!(true));

    let r = format!("{shape},/World/refd");
    s.run("usdPrim", json!({"args": [r], "define": "Xform"})).unwrap();
    s.run("usdPrim", json!({"args": [r], "addReference": "./asset.usda"})).unwrap();
    assert_eq!(exists(&mut s, "/World/refd/geo"), json!(true));
    let info = s.run("usdPrim", json!({"args": [r], "query": true})).unwrap();
    assert_eq!(info["references"][0]["asset"], json!("./asset.usda"));
    s.undo();
    assert_eq!(exists(&mut s, "/World/refd/geo"), json!(false));
}

#[test]
fn attributes_author_with_their_types() {
    let (mut s, shape) = setup("attr");
    let name = format!("{shape},/World/a/ball.radius");
    assert_eq!(s.run("getAttr", json!({"args": [name]})).unwrap(), json!(2.0));
    s.run("setAttr", json!({"args": [name, 5.0]})).unwrap();
    assert_eq!(s.run("getAttr", json!({"args": [name]})).unwrap(), json!(5.0));
    s.undo();
    assert_eq!(s.run("getAttr", json!({"args": [name]})).unwrap(), json!(2.0));
    let c = format!("{shape},/World/a/ball.primvars:displayColor");
    assert!(s.run("usdAttr", json!({"args": [c], "value": [1.0, 0.0, 0.0]})).is_err(), "array types aren't set from three numbers");
    let n = format!("{shape},/World/a/ball.note");
    s.run("usdAttr", json!({"args": [n], "value": "hello", "type": "string"})).unwrap();
    assert_eq!(s.run("getAttr", json!({"args": [n]})).unwrap(), json!("hello"));
    let _ = gf::vec3f(0.0, 0.0, 0.0);
}

#[test]
fn panels_read_and_write_prims_like_nodes() {
    let (mut s, shape) = setup("panels");
    let a = format!("{shape},/World/a");
    s.run("select", json!({"args": [a]})).unwrap();
    let cb = aztek_usd::panels::channel_box(&mut s).unwrap();
    assert_eq!(cb["name"], json!("a"));
    assert_eq!(cb["channels"][0]["attr"], json!("translateX"));
    assert_eq!(cb["channels"][0]["value"], json!(1.0));
    // The Channel Box sets a channel by its plug.
    s.run("setAttr", json!({"args": [cb["channels"][0]["plug"], 4.0]})).unwrap();
    assert_eq!(v3(&s.run("getAttr", json!({"args": [format!("{a}.xformOp:translate")]})).unwrap()), DVec3::new(4.0, 0.0, 0.0));
    s.undo();
    s.run("setAttr", json!({"args": [format!("{a}.rotateY"), 400.0]})).unwrap();
    assert_eq!(s.run("getAttr", json!({"args": [format!("{a}.rotateY")]})).unwrap().as_f64().map(|x| (x - 400.0).abs() < 1e-3), Some(true), "angles past a turn stay");

    // The variant set is an enum in the Attribute Editor.
    let v = format!("{shape},/World/varied");
    let ae = aztek_usd::panels::attribute_editor(&mut s, &v).unwrap();
    let attrs = ae["tabs"][0]["attrs"].as_array().unwrap();
    let field = attrs.iter().find(|x| x["attr"] == "variant:shape").unwrap();
    assert_eq!(field["enum"], json!(["one", "two"]));
    s.run("setAttr", json!({"args": [field["plug"], 1]})).unwrap();
    let kids = s.run("usdPrim", json!({"args": [v], "query": true, "children": true})).unwrap();
    assert_eq!(kids[0]["name"], json!("inTwo"));

    // Payload state and activation through plugs.
    let h = format!("{shape},/World/heavy");
    s.run("setAttr", json!({"args": [format!("{h}.loaded"), false]})).unwrap();
    assert_eq!(s.run("getAttr", json!({"args": [format!("{h}.loaded")]})).unwrap(), json!(false));

    // Nothing selected: the panels fall through to nodes.
    s.run("select", json!({"clear": true})).unwrap();
    assert!(aztek_usd::panels::channel_box(&mut s).is_none());
}

#[test]
fn set_keyframe_authors_samples() {
    let (mut s, shape) = setup("key");
    let a = format!("{shape},/World/a");
    s.run("setKeyframe", json!({"args": [a], "time": 2.0})).unwrap();
    s.run("setKeyframe", json!({"args": [format!("{a}.xformOp:translate")], "time": 6.0})).unwrap();
    let times = s.run("usdAttr", json!({"args": [format!("{a}.xformOp:translate")], "query": true, "timeSamples": true})).unwrap();
    assert_eq!(times, json!([2.0, 6.0]));
    s.undo();
    let times = s.run("usdAttr", json!({"args": [format!("{a}.xformOp:translate")], "query": true, "timeSamples": true})).unwrap();
    assert_eq!(times, json!([2.0]));
}

#[test]
fn parent_moves_prims_and_loadable_prims_list() {
    let (mut s, shape) = setup("parent");
    let r = s.run("parent", json!({"args": [format!("{shape},/World/odd"), format!("{shape},/World/a")]})).unwrap();
    assert!(r[0].as_str().unwrap().ends_with(",/World/a/odd"), "{r}");
    s.undo();
    assert_eq!(s.run("usdPrim", json!({"args": [format!("{shape},/World/odd")], "query": true, "exists": true})).unwrap(), json!(true));
    let all = s.run("AL_usdmaya_ProxyShapeFindLoadable", json!({"args": [shape]})).unwrap();
    assert_eq!(all, json!(["/World/heavy"]));
    s.run("usdPrim", json!({"args": [format!("{shape},/World/heavy")], "unload": true})).unwrap();
    assert_eq!(s.run("AL_usdmaya_ProxyShapeFindLoadable", json!({"args": [shape], "loaded": true})).unwrap(), json!([]));
    assert_eq!(s.run("AL_usdmaya_ProxyShapeFindLoadable", json!({"args": [shape], "unloaded": true})).unwrap(), json!(["/World/heavy"]));
}
