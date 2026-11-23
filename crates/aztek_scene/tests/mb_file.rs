//! Binary scene files (.mb) read through the .ma reader.

use aztek_scene::{ma_file, mb_file, Session};
use serde_json::{json, Value};
use std::process::Command;

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
    let mut t = Session::new();
    run(&mut t, "file", json!({"args": [data("gt-tools/dummy_mesh.mb")], "open": true, "force": true, "type": "mayaBinary"}));
    // A .mb name writes a binary file; -type mayaBinary gives a bare name the extension.
    run(&mut s, "file", json!({"args": [temp("out.mb")], "save": true}));
    assert!(mb_file::is_binary(&std::fs::read(temp("out.mb")).unwrap()));
    assert_eq!(run(&mut s, "file", json!({"query": true, "type": true})), json!(["mayaBinary"]));
    run(&mut s, "file", json!({"args": [temp("bare")], "save": true, "type": "mayaBinary"}));
    assert!(mb_file::is_binary(&std::fs::read(temp("bare.mb")).unwrap()));
    // Only the reference's two scene types.
    assert!(s.run("file", json!({"args": [temp("x")], "save": true, "type": "aztek"})).is_err());
}

/// The scene as .ma text, without the lines that name the file or the application.
fn scene_text(s: &mut Session, name: &str) -> String {
    let (text, _) = ma_file::write_text(s, name, None);
    text.lines().filter(|l| !l.starts_with("//") && !l.starts_with("fileInfo")).collect::<Vec<_>>().join("\n")
}

/// Lines that say the same: numbers equal to a binary file's single precision, where it keeps points that way.
fn same_line(a: &str, b: &str) -> bool {
    let (ta, tb): (Vec<&str>, Vec<&str>) = (a.split_whitespace().collect(), b.split_whitespace().collect());
    ta.len() == tb.len()
        && ta.iter().zip(&tb).all(|(x, y)| {
            let (x, y) = (x.trim_end_matches(';'), y.trim_end_matches(';'));
            match (x.parse::<f64>(), y.parse::<f64>()) {
                (Ok(p), Ok(q)) => (p - q).abs() <= 1e-6 * p.abs().max(q.abs()).max(1.0),
                _ => x == y,
            }
        })
}

fn first_difference(a: &str, b: &str) -> String {
    let (la, lb): (Vec<&str>, Vec<&str>) = (a.lines().collect(), b.lines().collect());
    for i in 0..la.len().max(lb.len()) {
        if !same_line(la.get(i).unwrap_or(&""), lb.get(i).unwrap_or(&"")) {
            return format!("line {i}:\n  ma: {:?}\n  mb: {:?}", la.get(i), lb.get(i));
        }
    }
    String::new()
}

fn saved_both_ways(s: &mut Session, name: &str) -> (String, String) {
    let (ma, mb) = (temp(&format!("{name}.ma")), temp(&format!("{name}.mb")));
    ma_file::save_scene(s, &ma, ma_file::Format::Ma).unwrap();
    let w = ma_file::save_scene(s, &mb, ma_file::Format::Mb).unwrap();
    assert!(w.is_empty(), "{w:?}");
    assert!(mb_file::is_binary(&std::fs::read(&mb).unwrap()));
    let mut from_ma = Session::new();
    ma_file::open_scene(&mut from_ma, &ma).unwrap();
    let mut from_mb = Session::new();
    let w = ma_file::open_scene(&mut from_mb, &mb).unwrap();
    assert!(w.is_empty(), "{w:?}");
    (scene_text(&mut from_ma, "x.ma"), scene_text(&mut from_mb, "x.ma"))
}

#[test]
fn written_as_binary_it_reads_as_the_ascii_file_does() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "setKeyframe", json!({"args": ["box"], "attribute": "translateX", "time": 1, "value": 0}));
    run(&mut s, "setKeyframe", json!({"args": ["box"], "attribute": "translateX", "time": 10, "value": 5}));
    run(&mut s, "setKeyframe", json!({"args": ["box"], "attribute": "rotateY", "time": 5, "value": 45}));
    run(&mut s, "addAttr", json!({"args": ["box"], "longName": "blend", "attributeType": "double", "minValue": 0, "maxValue": 1, "defaultValue": 0.5, "keyable": true}));
    run(&mut s, "addAttr", json!({"args": ["box"], "longName": "note", "dataType": "string"}));
    run(&mut s, "setAttr", json!({"args": ["box.note", "hello"], "type": "string"}));
    run(&mut s, "addAttr", json!({"args": ["box"], "longName": "mode", "attributeType": "enum", "enumName": "a:b:c"}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0, 0, 0], "name": "j1"}));
    run(&mut s, "joint", json!({"position": [0, 2, 0], "name": "j2"}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "polySphere", json!({"name": "ball"}));
    run(&mut s, "skinCluster", json!({"args": ["j1", "j2", "ball"]}));
    run(&mut s, "circle", json!({"name": "ring"}));
    run(&mut s, "parentConstraint", json!({"args": ["box", "ring"]}));
    run(&mut s, "setAttr", json!({"args": ["ring.visibility", false]}));
    run(&mut s, "setAttr", json!({"args": ["box.translateY"], "lock": true}));
    run(&mut s, "currentTime", json!({"args": [7]}));
    let (ma, mb) = saved_both_ways(&mut s, "rich");
    let d = first_difference(&ma, &mb);
    assert!(d.is_empty(), "{d}");
}

#[test]
fn a_reference_binary_file_written_again_reads_the_same() {
    let mut s = Session::new();
    ma_file::open_scene(&mut s, &data("gt-tools/dummy_mesh.mb")).unwrap();
    let (ma, mb) = saved_both_ways(&mut s, "dummy_again");
    let d = first_difference(&ma, &mb);
    assert!(d.is_empty(), "{d}");
}

/// Run with `AZTEK_MAYAPY` set to Maya's mayapy, then pass `-- --ignored` to this test.
#[test]
#[ignore = "requires Maya 2026 or 2027 mayapy; set AZTEK_MAYAPY to enable"]
fn maya_reads_uvs_from_aztek_binary() {
    let mayapy = std::env::var_os("AZTEK_MAYAPY").expect("set AZTEK_MAYAPY to Maya mayapy");
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "uvProbe", "subdivisionsX": 2, "subdivisionsZ": 1}));
    run(&mut s, "polyCube", json!({"name": "cubeProbe"}));
    let path = temp("maya_uv_probe.mb");
    let warnings = ma_file::save_scene(&mut s, &path, ma_file::Format::Mb).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");

    let script = format!("{}/../../tools/reference_capture/verify_mb_uv.py", env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(mayapy).arg(script).arg(&path).output().expect("run Maya mayapy");
    assert!(output.status.success(), "Maya failed:\n{}", String::from_utf8_lossy(&output.stderr));
    let result: Value = serde_json::from_slice(&output.stdout).expect("Maya JSON output");
    assert!(matches!(result["version"].as_str(), Some("2026" | "2027")), "{result}");
    eprintln!("Maya runtime: {}", result);
    let data = &result["meshes"]["uvProbeShape"]["sets"]["map1"];
    assert_eq!(data["uvCount"], 6);
    assert_eq!(data["faceCounts"], json!([4, 4]));
    assert_eq!(data["faceIds"], json!([0, 1, 4, 3, 1, 2, 5, 4]));
    assert_eq!(data["uvs"], json!([0.0, 0.0, 0.5, 0.0, 1.0, 0.0, 0.0, 1.0, 0.5, 1.0, 1.0, 1.0]));
    let cube = &result["meshes"]["cubeProbeShape"]["sets"]["map1"];
    assert_eq!(cube["uvCount"], 14, "{result}");
    assert_eq!(cube["faceCounts"], json!([4, 4, 4, 4, 4, 4]));
    assert_eq!(cube["faceIds"], json!([0, 1, 3, 2, 2, 3, 5, 4, 4, 5, 7, 6, 6, 7, 9, 8, 1, 10, 11, 3, 12, 0, 2, 13]));
    let expected = [
        0.33, 0.0, 0.6633333, 0.0, 0.33, 0.25, 0.6633333, 0.25, 0.33, 0.5, 0.6633333, 0.5,
        0.33, 0.75, 0.6633333, 0.75, 0.33, 1.0, 0.6633333, 1.0, 1.0, 0.0, 1.0, 0.25, 0.0,
        0.0, 0.0, 0.25,
    ];
    let actual = cube["uvs"].as_array().expect("cube UV coordinates");
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert!((actual.as_f64().unwrap() - expected).abs() < 1e-6, "{result}");
    }
}

#[test]
fn not_a_scene() {
    assert!(mb_file::convert(b"FOR4\0\0\0\x04ABCD").is_err());
    let path = temp("bad.mb");
    std::fs::write(&path, "createNode transform;").unwrap();
    let mut s = Session::new();
    assert!(ma_file::open_scene(&mut s, &path).is_err());
}
