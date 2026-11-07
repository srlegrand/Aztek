//! mayaUSDExport: scenes built with commands, written, then read back through the openusd stage.

use aztek_scene::session::Session;
use openusd::{gf, usd};
use serde_json::json;

fn session() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    aztek_usd::install(&mut s);
    s
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_usd_export_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().into_owned()
}

fn open(path: &str) -> usd::Stage {
    usd::Stage::builder().schema_registry(openusd_schemas::schema_registry()).open(path).unwrap()
}

#[test]
fn cube_merges_transform_and_shape() {
    let mut s = session();
    s.run("polyCube", json!({"name": "box"})).unwrap();
    s.run("move", json!({"args": [1.0, 2.0, 3.0, "box"]})).unwrap();
    let f = temp("cube.usda");
    s.run("mayaUSDExport", json!({"file": f})).unwrap();
    let st = open(&f);
    let p = st.prim("/box").unwrap();
    assert_eq!(p.type_name().unwrap().as_deref(), Some("Mesh"));
    assert_eq!(st.default_prim().as_deref(), Some("box"));
    let pts = p.attribute("points").get::<Vec<gf::Vec3f>>().unwrap().unwrap();
    assert_eq!(pts.len(), 8);
    let counts = p.attribute("faceVertexCounts").get::<Vec<i32>>().unwrap().unwrap();
    assert_eq!(counts, vec![4; 6]);
    let t = p.attribute("xformOp:translate").get::<gf::Vec3d>().unwrap().unwrap();
    assert_eq!((t.x, t.y, t.z), (1.0, 2.0, 3.0));
    let scheme = p.attribute("subdivisionScheme").get::<openusd::tf::Token>().unwrap().unwrap();
    assert_eq!(scheme.as_str(), "catmullClark");
    assert!(st.stage_metadata("metersPerUnit").unwrap().is_some());
    // The default cameras stay out unless asked for.
    assert!(!st.prim("/persp").unwrap().is_valid().unwrap());
}

#[test]
fn animation_becomes_time_samples() {
    let mut s = session();
    s.run("polyCube", json!({"name": "box"})).unwrap();
    s.run("setKeyframe", json!({"args": ["box.tx"], "time": 1.0, "value": 0.0})).unwrap();
    s.run("setKeyframe", json!({"args": ["box.tx"], "time": 5.0, "value": 4.0})).unwrap();
    let f = temp("anim.usda");
    s.run("mayaUSDExport", json!({"file": f, "frameRange": [1.0, 5.0]})).unwrap();
    let st = open(&f);
    let a = st.attribute("/box.xformOp:translate").unwrap();
    assert_eq!(a.time_sample_times().unwrap(), vec![1.0, 2.0, 3.0, 4.0, 5.0]);
    assert_eq!(st.start_time_code(), 1.0);
    assert_eq!(st.end_time_code(), 5.0);
    // What doesn't move is static.
    assert_eq!(st.attribute("/box.points").unwrap().num_time_samples().unwrap(), 0);
}

#[test]
fn formats_by_extension() {
    let mut s = session();
    s.run("polyCube", json!({"name": "box"})).unwrap();
    for name in ["a.usda", "a.usdc", "a.usd", "a.usdz"] {
        let f = temp(name);
        s.run("mayaUSDExport", json!({"file": f})).unwrap();
        let st = open(&f);
        assert!(st.prim("/box").unwrap().is_valid().unwrap(), "{name}");
    }
    let text = std::fs::read(temp("a.usda")).unwrap();
    assert!(text.starts_with(b"#usda"));
    assert!(std::fs::read(temp("a.usdc")).unwrap().starts_with(b"PXR-USDC"));
    assert!(std::fs::read(temp("a.usd")).unwrap().starts_with(b"PXR-USDC"));
    let f = temp("b.usd");
    s.run("mayaUSDExport", json!({"file": f, "defaultUSDFormat": "usda"})).unwrap();
    assert!(std::fs::read(&f).unwrap().starts_with(b"#usda"));
}

#[test]
fn deformed_points_become_samples_without_skeletons() {
    let mut s = session();
    s.run("polyPlane", json!({"name": "m", "subdivisionsX": 1, "subdivisionsY": 2, "width": 1, "height": 4})).unwrap();
    s.run("select", json!({"clear": true})).unwrap();
    s.run("joint", json!({"name": "a", "position": [0.0, 0.0, 0.0]})).unwrap();
    s.run("skinCluster", json!({"args": ["a", "m"]})).unwrap();
    s.run("setKeyframe", json!({"args": ["a.ty"], "time": 1.0, "value": 0.0})).unwrap();
    s.run("setKeyframe", json!({"args": ["a.ty"], "time": 3.0, "value": 2.0})).unwrap();
    let f = temp("deformed.usda");
    s.run("mayaUSDExport", json!({"file": f, "frameRange": [1.0, 3.0]})).unwrap();
    let st = open(&f);
    let a = st.attribute("/m.points").unwrap();
    assert_eq!(a.num_time_samples().unwrap(), 3);
    let at = |t: f64| a.get_at::<Vec<gf::Vec3f>>(usd::TimeCode::new(t)).unwrap().unwrap()[0];
    assert!((at(3.0).y - at(1.0).y - 2.0).abs() < 1e-5, "points must carry the deformation");
}
