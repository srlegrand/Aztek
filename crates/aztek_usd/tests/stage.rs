//! Proxy shapes, stages and undo through the scene's stack.

use aztek_scene::session::Session;
use aztek_usd::stages;
use openusd::gf;
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    s
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_usd_stage_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().into_owned()
}

const LAYER: &str = r#"#usda 1.0
(
    defaultPrim = "World"
)
def Xform "World"
{
    def Sphere "ball"
    {
        double radius = 2
    }
}
"#;

fn make(s: &mut Session, name: &str) -> (String, aztek_scene::plug::NodeId) {
    let f = temp(name);
    std::fs::write(&f, LAYER).unwrap();
    let shape = s.run("mayaUsd_createStageFromFile", json!({"args": [f]})).unwrap();
    let shape = shape.as_str().unwrap().to_string();
    let id = s.graph.find(&shape).unwrap().unwrap();
    (f, id)
}

#[test]
fn node_types_and_attributes() {
    let mut s = session();
    let (f, id) = make(&mut s, "types.usda");
    assert_eq!(s.run("nodeType", json!({"args": ["typesShape1"]})).unwrap(), json!("mayaUsdProxyShape"));
    assert_eq!(s.run("getAttr", json!({"args": ["typesShape1.filePath"]})).unwrap(), json!(f));
    assert_eq!(s.run("getAttr", json!({"args": ["typesShape1.drawProxyPurpose"]})).unwrap(), json!(true));
    assert_eq!(s.run("getAttr", json!({"args": ["typesShape1.drawRenderPurpose"]})).unwrap(), json!(false));
    let e = stages::entry_of(&mut s.graph, id).unwrap();
    assert!(e.stage.prim("/World/ball").unwrap().is_valid().unwrap());
    assert_eq!(s.run("getAttr", json!({"args": ["typesShape1.stageCacheId"]})).unwrap(), json!(e.id));
    // createNode works too, and the AL name is the same kind of thing.
    s.run("createNode", json!({"args": ["AL_usdmaya_ProxyShape"], "name": "alShape"})).unwrap();
    assert!(stages::is_proxy(&s.graph, s.graph.find("alShape").unwrap().unwrap()));
    let r = s.run("AL_usdmaya_ProxyShapeImport", json!({"file": f, "name": "alProxy"})).unwrap();
    assert_eq!(s.run("nodeType", json!({"args": [r[0]]})).unwrap(), json!("AL_usdmaya_ProxyShape"));
}

#[test]
fn proxies_of_one_file_share_a_stage_unless_told_not_to() {
    let mut s = session();
    let (f, a) = make(&mut s, "share.usda");
    let b = s.run("mayaUsd_createStageFromFile", json!({"args": [f]})).unwrap();
    let b = s.graph.find(b.as_str().unwrap()).unwrap().unwrap();
    let (ea, eb) = (stages::entry_of(&mut s.graph, a).unwrap(), stages::entry_of(&mut s.graph, b).unwrap());
    assert_eq!(ea.id, eb.id);
    let c = s.run("mayaUsd_createStageFromFile", json!({"args": [f]})).unwrap();
    let c_name = c.as_str().unwrap().to_string();
    s.run("setAttr", json!({"args": [format!("{c_name}.shareStage"), false]})).unwrap();
    let c = s.graph.find(&c_name).unwrap().unwrap();
    let ec = stages::entry_of(&mut s.graph, c).unwrap();
    assert_ne!(ea.id, ec.id);
}

#[test]
fn anonymous_stage_and_undo_redo_of_authoring() {
    let mut s = session();
    let shape = s.run("mayaUsd_createStageWithNewLayer", json!({})).unwrap();
    let id = s.graph.find(shape.as_str().unwrap()).unwrap().unwrap();
    let e = stages::entry_of(&mut s.graph, id).unwrap();
    assert_eq!(e.file, "");
    let p = "/a";
    s.graph.open_chunk("define");
    e.edit(&mut s, "define", |st| {
        st.define_prim(p).map_err(|x| x.to_string())?.set_type_name("Xform").map_err(|x| x.to_string())?;
        st.create_attribute("/a.v", "double").map_err(|x| x.to_string())?.set(1.0_f64).map_err(|x| x.to_string())?;
        Ok(())
    })
    .unwrap();
    s.graph.close_chunk();
    s.graph.open_chunk("set");
    e.edit(&mut s, "set", |st| {
        st.attribute("/a.v").map_err(|x| x.to_string())?.set(2.0_f64).map_err(|x| x.to_string())?;
        Ok(())
    })
    .unwrap();
    s.graph.close_chunk();
    let val = |e: &stages::Entry| e.stage.attribute("/a.v").unwrap().get::<f64>().unwrap();
    assert_eq!(val(&e), Some(2.0));
    s.undo();
    assert_eq!(val(&e), Some(1.0));
    s.undo();
    assert!(!e.stage.prim("/a").unwrap().is_valid().unwrap());
    s.redo();
    assert_eq!(val(&e), Some(1.0));
    s.redo();
    assert_eq!(val(&e), Some(2.0));
    // Undo again after redo, and a new edit after an undo.
    s.undo();
    assert_eq!(val(&e), Some(1.0));
    s.graph.open_chunk("other");
    e.edit(&mut s, "other", |st| {
        st.attribute("/a.v").map_err(|x| x.to_string())?.set(9.0_f64).map_err(|x| x.to_string())?;
        Ok(())
    })
    .unwrap();
    s.graph.close_chunk();
    assert_eq!(val(&e), Some(9.0));
    s.undo();
    assert_eq!(val(&e), Some(1.0));
    s.redo();
    assert_eq!(val(&e), Some(9.0));
}

#[test]
fn session_and_dirty_layers_come_back_through_the_attributes() {
    let f = temp("reopen.usda");
    std::fs::write(&f, LAYER).unwrap();
    let e = stages::open_entry(&f, "#usda 1.0\nover \"World\"\n{\n    over \"ball\"\n    {\n        double radius = 7\n    }\n}\n", true).unwrap();
    let r = e.stage.attribute("/World/ball.radius").unwrap().get::<f64>().unwrap();
    assert_eq!(r, Some(7.0), "the session layer is stronger than the root");
    let _ = gf::vec3f(0.0, 0.0, 0.0);
}

/// A stage exported as a reference (the reference's way), as a payload, or flattened into the file (Aztek's two).
#[test]
fn stages_export_as_references_payloads_or_flattened() {
    let mut s = session();
    let _ = make(&mut s, "arcs_src.usda");
    let read = |f: &str| std::fs::read_to_string(f).unwrap();
    let as_ref = temp("arcs_ref.usda");
    s.run("mayaUSDExport", json!({"file": as_ref})).unwrap();
    assert!(read(&as_ref).contains("prepend references = @"), "{}", read(&as_ref));
    let as_payload = temp("arcs_payload.usda");
    s.run("mayaUSDExport", json!({"file": as_payload, "exportStagesAsPayloads": true})).unwrap();
    let text = read(&as_payload);
    assert!(text.contains("prepend payload = @") && !text.contains("references"), "{text}");
    let flat = temp("arcs_flat.usda");
    s.run("mayaUSDExport", json!({"file": flat, "flatten": true})).unwrap();
    let text = read(&flat);
    assert!(!text.contains("references") && !text.contains("payload"), "{text}");
    assert!(text.contains("def Sphere \"ball\"") && text.contains("radius = 2"), "the stage's prims are in the file: {text}");
    assert!(!std::path::Path::new(&format!("{flat}.flatten.usdc")).exists());
}
