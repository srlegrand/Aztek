//! Layers: sublayers, edit targets, mute, lock, save, and a scene carrying unsaved work.

use aztek_scene::session::Session;
use aztek_usd::stages;
use serde_json::{json, Value};

const ROOT: &str = "#usda 1.0\ndef Xform \"World\"\n{\n    def Sphere \"ball\"\n    {\n        double radius = 2\n    }\n}\n";
const OVER: &str = "#usda 1.0\nover \"World\"\n{\n    over \"ball\"\n    {\n        double radius = 3\n    }\n}\n";

fn setup(tag: &str) -> (Session, String, std::path::PathBuf) {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    let dir = std::env::temp_dir().join(format!("aztek_usd_layers_{}_{tag}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("root.usda"), ROOT).unwrap();
    std::fs::write(dir.join("over.usda"), OVER).unwrap();
    let shape = s.run("mayaUsd_createStageFromFile", json!({"args": [dir.join("root.usda").to_string_lossy()]})).unwrap();
    (s, shape.as_str().unwrap().to_string(), dir)
}

fn radius(s: &mut Session, shape: &str) -> Value {
    s.run("getAttr", json!({"args": [format!("{shape},/World/ball.radius")]})).unwrap()
}

fn root_id(s: &mut Session, shape: &str) -> String {
    s.run("AL_usdmaya_LayerGetLayers", json!({"proxy": shape, "rootLayer": true})).unwrap().as_str().unwrap().to_string()
}

#[test]
fn sublayers_compose_and_undo() {
    let (mut s, shape, dir) = setup("sub");
    let root = root_id(&mut s, &shape);
    assert_eq!(radius(&mut s, &shape), json!(2.0));
    let over = dir.join("over.usda").to_string_lossy().into_owned();
    s.run("mayaUsdLayerEditor", json!({"args": [root], "insertSubPath": [0, over]})).unwrap();
    // The root is stronger than its sublayer, and the root says 2.
    assert_eq!(radius(&mut s, &shape), json!(2.0));
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[1]["role"], json!("root"));
    assert_eq!(t[1]["sublayers"][0]["name"], json!("over.usda"));
    assert_eq!(t[1]["editTarget"], json!(true));
    s.undo();
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[1]["sublayers"], json!([]));
    s.redo();
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[1]["sublayers"][0]["name"], json!("over.usda"));
}

#[test]
fn edit_target_routes_edits_and_mute_and_lock_refuse() {
    let (mut s, shape, _dir) = setup("target");
    let id = stages::proxy_named(&s.graph, &shape).unwrap();
    let e = stages::entry_of(&mut s.graph, id).unwrap();
    let session = e.session_identifier.clone();
    let root = root_id(&mut s, &shape);
    assert_eq!(s.run("mayaUsdEditTarget", json!({"args": [shape], "query": true, "editTarget": true})).unwrap(), json!(root));
    s.run("mayaUsdEditTarget", json!({"args": [shape], "edit": true, "editTarget": session})).unwrap();
    s.run("setAttr", json!({"args": [format!("{shape},/World/ball.radius"), 9.0]})).unwrap();
    assert_eq!(radius(&mut s, &shape), json!(9.0));
    // In the session layer, not the root.
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[0]["dirty"], json!(true));
    assert_eq!(t[1]["dirty"], json!(false));

    // Muting the session layer takes its opinion away and sends edits back to the root.
    s.run("mayaUsdLayerEditor", json!({"args": [session], "muteLayer": [true, shape]})).unwrap();
    assert_eq!(radius(&mut s, &shape), json!(2.0));
    assert_eq!(s.run("mayaUsdEditTarget", json!({"args": [shape], "query": true, "editTarget": true})).unwrap(), json!(root));
    s.undo();
    assert_eq!(radius(&mut s, &shape), json!(9.0));

    // A locked target refuses.
    s.run("mayaUsdLayerEditor", json!({"args": [root], "lockLayer": [1, shape]})).unwrap();
    assert!(s.run("mayaUsdEditTarget", json!({"args": [shape], "edit": true, "editTarget": root})).is_err());
    s.run("mayaUsdEditTarget", json!({"args": [shape], "edit": true, "editTarget": session})).unwrap();
    s.run("mayaUsdLayerEditor", json!({"args": [session], "lockLayer": [1, shape]})).unwrap();
    assert!(s.run("setAttr", json!({"args": [format!("{shape},/World/ball.radius"), 1.0]})).is_err());
}

#[test]
fn save_reload_and_flatten() {
    let (mut s, shape, dir) = setup("save");
    let root = root_id(&mut s, &shape);
    s.run("setAttr", json!({"args": [format!("{shape},/World/ball.radius"), 5.0]})).unwrap();
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[1]["dirty"], json!(true));
    s.run("usdLayer", json!({"args": [root], "reload": true})).unwrap();
    assert_eq!(radius(&mut s, &shape), json!(2.0));
    s.run("setAttr", json!({"args": [format!("{shape},/World/ball.radius"), 6.0]})).unwrap();
    s.run("usdLayer", json!({"args": [root], "save": true})).unwrap();
    assert!(std::fs::read_to_string(dir.join("root.usda")).unwrap().contains("radius = 6"));
    let t = s.run("usdLayer", json!({"proxy": shape, "query": true})).unwrap();
    assert_eq!(t[1]["dirty"], json!(false));
    let flat = dir.join("flat.usda").to_string_lossy().into_owned();
    s.run("usdLayer", json!({"proxy": shape, "flattenTo": flat})).unwrap();
    assert!(std::fs::read_to_string(&flat).unwrap().contains("ball"));
    let text = s.run("AL_usdmaya_LayerSave", json!({"proxy": shape, "string": true})).unwrap();
    assert!(text.as_str().unwrap().contains("radius = 6"));
}

#[test]
fn scene_carries_session_and_unsaved_layers_and_leaves_clean_ones_referenced() {
    let (mut s, shape, dir) = setup("scene");
    let id = stages::proxy_named(&s.graph, &shape).unwrap();
    let e = stages::entry_of(&mut s.graph, id).unwrap();
    let session = e.session_identifier.clone();
    let ma = dir.join("scene.ma").to_string_lossy().into_owned();

    // Nothing touched: the file holds the path and no layer text.
    s.run("file", json!({"args": [ma], "exportAll": true, "type": "mayaAscii"})).unwrap();
    let text = std::fs::read_to_string(&ma).unwrap();
    assert!(text.contains("createNode mayaUsdProxyShape"), "{text}");
    assert!(text.contains("root.usda"));
    assert!(!text.contains("radius"), "a clean layer stays a reference");

    // A session opinion and an unsaved root edit.
    s.run("mayaUsdEditTarget", json!({"args": [shape], "edit": true, "editTarget": session})).unwrap();
    s.run("usdPrim", json!({"args": [format!("{shape},/World/ball")], "purpose": "guide"})).unwrap();
    let root = root_id(&mut s, &shape);
    s.run("mayaUsdEditTarget", json!({"args": [shape], "edit": true, "editTarget": root})).unwrap();
    s.run("setAttr", json!({"args": [format!("{shape},/World/ball.radius"), 8.0]})).unwrap();
    s.run("file", json!({"args": [ma], "rename": ma})).ok();
    s.run("file", json!({"args": [ma], "exportAll": true, "type": "mayaAscii"})).unwrap();

    let mut b = Session::new();
    aztek_scene::cameras::make_defaults(&mut b.graph);
    aztek_usd::install(&mut b);
    stages::forget_all();
    b.run("file", json!({"args": [ma], "open": true, "force": true})).unwrap();
    let list = b.run("ls", json!({"type": "mayaUsdProxyShape"})).unwrap();
    let sh = list[0].as_str().unwrap().to_string();
    assert_eq!(radius(&mut b, &sh), json!(8.0), "the unsaved edit came back");
    let info = b.run("getAttr", json!({"args": [format!("{sh},/World/ball.purpose")]})).unwrap();
    assert_eq!(info, json!("guide"), "the session layer came back");
    // The file on disk still says what it said.
    assert!(std::fs::read_to_string(dir.join("root.usda")).unwrap().contains("radius = 2"));
}

#[test]
fn anonymous_root_lives_in_the_scene_until_saved() {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    let dir = std::env::temp_dir().join(format!("aztek_usd_layers_{}_anon", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let shape = s.run("mayaUsd_createStageWithNewLayer", json!({})).unwrap().as_str().unwrap().to_string();
    s.run("usdPrim", json!({"args": [format!("{shape},/made")], "define": "Cube"})).unwrap();
    let root = root_id(&mut s, &shape);
    assert!(s.run("usdLayer", json!({"args": [root], "save": true})).is_err(), "an anonymous layer needs a name");
    let ma = dir.join("anon.ma").to_string_lossy().into_owned();
    s.run("file", json!({"args": [ma], "exportAll": true, "type": "mayaAscii"})).unwrap();

    let mut b = Session::new();
    aztek_scene::cameras::make_defaults(&mut b.graph);
    aztek_usd::install(&mut b);
    stages::forget_all();
    b.run("file", json!({"args": [ma], "open": true, "force": true})).unwrap();
    let sh = b.run("ls", json!({"type": "mayaUsdProxyShape"})).unwrap()[0].as_str().unwrap().to_string();
    let e = b.run("usdPrim", json!({"args": [format!("{sh},/made")], "query": true, "exists": true})).unwrap();
    assert_eq!(e, json!(true));

    let f = dir.join("named.usda").to_string_lossy().into_owned();
    let r2 = root_id(&mut b, &sh);
    b.run("usdLayer", json!({"args": [r2], "saveAs": f, "proxy": sh})).unwrap();
    assert_eq!(b.run("getAttr", json!({"args": [format!("{sh}.filePath")]})).unwrap(), json!(f));
    assert!(std::fs::read_to_string(&f).unwrap().contains("made"));
    let still = b.run("usdPrim", json!({"args": [format!("{sh},/made")], "query": true, "exists": true})).unwrap();
    assert_eq!(still, json!(true));
}
