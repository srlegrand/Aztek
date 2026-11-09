//! Binary scene files (.mb) read through the .ma reader.

use aztek_scene::{ma_file, mb_file, Session};
use serde_json::{json, Value};

fn data(name: &str) -> String {
    format!("{}/tests/data/mb/{name}", env!("CARGO_MANIFEST_DIR"))
}

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_mb_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_str().unwrap().to_string()
}

fn point_count(s: &mut Session, shape: &str) -> i64 {
    run(s, "polyEvaluate", json!({"args": [shape], "vertex": true})).as_i64().unwrap_or(-1)
}

#[test]
fn a_mesh_scene_opens() {
    let mut s = Session::new();
    let w = ma_file::open_scene(&mut s, &data("gt-tools/dummy_mesh.mb")).unwrap();
    assert!(!w.iter().any(|x| x.contains("skipped")), "{w:?}");
    for name in ["Dummy_Mesh", "Dummy_MeshShape", "M_Dummy", "ramp_red_blue"] {
        assert!(s.graph.find(name).unwrap().is_some(), "{name}");
    }
    // The geometry is kept on the shape under its history.
    assert_eq!(point_count(&mut s, "Dummy_MeshShape"), 2240);
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["M_Dummy"]})), json!("standardSurface"));
    assert_eq!(run(&mut s, "file", json!({"query": true, "sceneName": true})), json!(data("gt-tools/dummy_mesh.mb")));
}

#[test]
fn values_on_nodes_the_file_did_not_make() {
    let mut s = Session::new();
    ma_file::open_scene(&mut s, &data("mayascenekit/file_owner_delete.mb")).unwrap();
    for name in ["deleteTex", "keepTex"] {
        assert_eq!(run(&mut s, "nodeType", json!({"args": [name]})), json!("file"));
    }
    let persp = run(&mut s, "getAttr", json!({"args": ["persp.translate"]}));
    assert_eq!(persp, json!([28.0, 21.0, 28.0]));
    // Rotations are kept in radians and shown in degrees.
    let r = run(&mut s, "getAttr", json!({"args": ["top.rotateX"]})).as_f64().unwrap();
    assert!((r + 90.0).abs() < 1e-9, "{r}");
}

#[test]
fn saved_again_as_ma_it_reads_the_same() {
    let mut s = Session::new();
    ma_file::open_scene(&mut s, &data("gt-tools/dummy_mesh.mb")).unwrap();
    let out = temp("dummy.ma");
    ma_file::save_scene(&mut s, &out, ma_file::Format::Ma).unwrap();
    let mut again = Session::new();
    ma_file::open_scene(&mut again, &out).unwrap();
    assert_eq!(point_count(&mut again, "Dummy_MeshShape"), 2240);
}

#[test]
fn import_and_type_flags() {
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [data("gt-tools/dummy_mesh.mb")], "import": true}));
    assert!(s.graph.find("Dummy_Mesh").unwrap().is_some());
    // Binary files are read, not written; the type is fine on a read.
    let mut t = Session::new();
    run(&mut t, "file", json!({"args": [data("gt-tools/dummy_mesh.mb")], "open": true, "force": true, "type": "mayaBinary"}));
    assert!(s.run("file", json!({"args": [temp("out.mb")], "save": true})).is_err());
    assert!(s.run("file", json!({"args": [temp("out")], "save": true, "type": "mayaBinary"})).is_err());
    assert!(!std::path::Path::new(&temp("out.mb")).exists());
}

#[test]
fn not_a_scene() {
    assert!(mb_file::convert(b"FOR4\0\0\0\x04ABCD").is_err());
    let path = temp("bad.mb");
    std::fs::write(&path, "createNode transform;").unwrap();
    let mut s = Session::new();
    assert!(ma_file::open_scene(&mut s, &path).is_err());
}
