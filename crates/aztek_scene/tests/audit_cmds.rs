//! System, query and reference commands: flags, modes and return values from the reference's documentation.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}
fn close(v: &Value, want: f64) {
    assert!((v.as_f64().unwrap_or_else(|| panic!("{v}")) - want).abs() < 1e-6, "got {v}, wanted {want}");
}
fn close3(v: &Value, want: [f64; 3]) {
    for i in 0..3 {
        assert!((v[i].as_f64().unwrap() - want[i]).abs() < 1e-5, "got {v}, wanted {want:?}");
    }
}
fn temp(name: &str) -> String {
    let d = std::env::temp_dir().join(format!("aztek_audit_{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d.join(name).to_string_lossy().to_string()
}

#[test]
fn up_axis() {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    assert_eq!(run(&mut s, "upAxis", json!({"q": true, "ax": true})), "y");
    run(&mut s, "upAxis", json!({"ax": "z"}));
    assert_eq!(run(&mut s, "upAxis", json!({"q": true, "ax": true})), "z");
    // The side camera stays on X; persp's Y height becomes Z.
    close3(&get(&mut s, "persp.translate"), [28.0, -28.0, 21.0]);
    run(&mut s, "upAxis", json!({"ax": "y"}));
    close3(&get(&mut s, "persp.translate"), [28.0, 21.0, 28.0]);
    assert!(s.run("upAxis", json!({"ax": "x"})).is_err());
}

#[test]
fn up_axis_saves() {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    run(&mut s, "upAxis", json!({"ax": "z"}));
    run(&mut s, "polyCube", json!({}));
    let path = temp("zup.ma");
    run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("upAxisScriptNode"));
    let mut t = Session::new();
    run(&mut t, "file", json!({"args": [path], "open": true, "force": true}));
    assert_eq!(run(&mut t, "upAxis", json!({"q": true, "ax": true})), "z");
}

#[test]
fn convert_unit() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "convertUnit", json!({"args": ["1"], "fromUnit": "inch", "toUnit": "cm"})), "2.54cm");
    assert_eq!(run(&mut s, "convertUnit", json!({"args": ["2m"], "toUnit": "cm"})), "200cm");
    assert_eq!(run(&mut s, "convertUnit", json!({"args": ["180"], "f": "deg", "t": "rad"})), "3.141593rad");
    assert!(s.run("convertUnit", json!({"args": ["1"], "f": "cm", "t": "deg"})).is_err());
}

#[test]
fn copy_attr() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "setAttr", json!({"args": ["a.tx", 3.0]}));
    run(&mut s, "setAttr", json!({"args": ["a.ty", 4.0]}));
    run(&mut s, "copyAttr", json!({"args": ["a", "b"], "v": true, "at": ["translate"]}));
    close(&get(&mut s, "b.tx"), 3.0);
    close(&get(&mut s, "b.ty"), 4.0);
    run(&mut s, "undo", json!({}));
    close(&get(&mut s, "b.tx"), 0.0);
    // In-connections move unless told to stay.
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "connectAttr", json!({"args": ["c.rx", "a.sx"]}));
    run(&mut s, "copyAttr", json!({"args": ["a", "b"], "ic": true, "at": ["scale"]}));
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["b.sx"], "sfd": true})), "c.rotateX");
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["a.sx"], "sfd": true})), "");
}

#[test]
fn cycle_check() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "cycleCheck", json!({"q": true})), false);
    run(&mut s, "cycleCheck", json!({"evaluation": true}));
    assert_eq!(run(&mut s, "cycleCheck", json!({"q": true})), true);
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "m"}));
    run(&mut s, "connectAttr", json!({"args": ["a.tx", "m.input1X"]}));
    assert_eq!(run(&mut s, "cycleCheck", json!({"all": true})), false);
    // tx -> md -> ty is no cycle: a plug's dependency, not the node's.
    run(&mut s, "connectAttr", json!({"args": ["m.outputX", "a.ty"]}));
    assert_eq!(run(&mut s, "cycleCheck", json!({"all": true})), false);
}

#[test]
fn evaluation_settings() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "evaluationManager", json!({"q": true, "mode": true, "queried": ["mode"]})), json!(["off"]));
    run(&mut s, "evaluationManager", json!({"mode": "parallel"}));
    assert_eq!(run(&mut s, "evaluationManager", json!({"q": true, "queried": ["mode"]})), json!(["parallel"]));
    run(&mut s, "threadCount", json!({"n": 3}));
    assert_eq!(run(&mut s, "threadCount", json!({"q": true, "queried": ["numberOfThreads"]})), 3);
    close(&run(&mut s, "dgtimer", json!({"q": true})), 0.0);
}

#[test]
fn display_colors_and_prefs() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "displayColor", json!({"args": ["lead", 4]})), Value::Null);
    assert_eq!(run(&mut s, "displayColor", json!({"args": ["lead"], "q": true})), 4);
    run(&mut s, "displayColor", json!({"args": ["lead", 6], "active": true}));
    assert_eq!(run(&mut s, "displayColor", json!({"args": ["lead"], "q": true, "active": true})), 6);
    assert_eq!(run(&mut s, "displayColor", json!({"args": ["lead"], "q": true})), 4);
    assert!(s.run("displayColor", json!({"args": ["noSuch", 1]})).is_err());
    run(&mut s, "displayRGBColor", json!({"args": ["background", 0.1, 0.2, 0.3]}));
    close3(&run(&mut s, "displayRGBColor", json!({"args": ["background"], "q": true})), [0.1, 0.2, 0.3]);
    close3(&run(&mut s, "colorIndex", json!({"args": [13], "q": true})), [1.0, 0.0, 0.0]);
    run(&mut s, "colorIndex", json!({"args": [13, 0.5, 0.5, 0.5]}));
    close3(&run(&mut s, "colorIndex", json!({"args": [13], "q": true})), [0.5, 0.5, 0.5]);
    assert_eq!(run(&mut s, "displayPref", json!({"q": true, "queried": ["materialLoadingMode"]})), "immediate");
    run(&mut s, "displayPref", json!({"wsa": "reduced"}));
    assert_eq!(run(&mut s, "displayPref", json!({"q": true, "queried": ["wireframeOnShadedActive"]})), "reduced");
    assert_eq!(run(&mut s, "selectPriority", json!({"q": true, "queried": ["controlVertex"]})), 5);
    run(&mut s, "selectPriority", json!({"cv": 7}));
    assert_eq!(run(&mut s, "selectPriority", json!({"q": true, "queried": ["controlVertex"]})), 7);
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "displaySurface", json!({"args": ["a"], "xr": true}));
    assert_eq!(run(&mut s, "displaySurface", json!({"args": ["a"], "q": true, "queried": ["xRay"]})), json!([true]));
    
}

#[test]
fn attribute_names() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    assert_eq!(run(&mut s, "attributeName", json!({"args": ["a.tx"], "n": true})), "Translate X");
    assert_eq!(run(&mut s, "attributeName", json!({"args": ["a.tx"], "l": true})), "translateX");
    assert_eq!(run(&mut s, "attributeName", json!({"args": ["a.translateX"], "s": true})), "tx");
    let all = run(&mut s, "attributeInfo", json!({"args": ["a"], "bool": true}));
    assert!(all.as_array().unwrap().iter().any(|x| x == "visibility"));
    assert!(!all.as_array().unwrap().iter().any(|x| x == "translate"));
    assert_eq!(run(&mut s, "encodeString", json!({"args": ["a\"b\\c\nd"]})), "a\\\"b\\\\c\\nd");
    assert_eq!(run(&mut s, "isTrue", json!({"args": ["SomethingSelected"]})), true);
    assert_eq!(run(&mut s, "isDirty", json!({"args": ["a.tx"]})).is_boolean(), true);
    assert_eq!(run(&mut s, "selectedNodes", json!({})), json!(["|a"]));
    let u = run(&mut s, "listNodeTypes", json!({"args": ["utility/general"]}));
    assert!(u.as_array().unwrap().iter().any(|x| x == "multiplyDivide"));
}

#[test]
fn object_center_and_compare() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a", "w": 2.0}));
    run(&mut s, "setAttr", json!({"args": ["a.tx", 5.0]}));
    close3(&run(&mut s, "objectCenter", json!({"args": ["a"]})), [5.0, 0.0, 0.0]);
    close(&run(&mut s, "objectCenter", json!({"args": ["a"], "x": true})), 5.0);
    close3(&run(&mut s, "objectCenter", json!({"args": ["a"], "l": true})), [0.0, 0.0, 0.0]);
    run(&mut s, "polyCube", json!({"name": "b"}));
    assert_eq!(run(&mut s, "transformCompare", json!({"args": ["a", "b"]})), 1);
    run(&mut s, "setAttr", json!({"args": ["b.tx", 5.0]}));
    assert_eq!(run(&mut s, "transformCompare", json!({"args": ["a", "b"]})), 0);
}

#[test]
fn closest_point_uv() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"name": "p", "w": 2.0, "h": 2.0, "sx": 1, "sy": 1}));
    let n = run(&mut s, "closestPointOnMesh", json!({"args": ["p"], "ip": [0.5, 3.0, 0.5]}));
    let n = n.as_str().unwrap().to_string();
    close3(&get(&mut s, &format!("{n}.position")), [0.5, 0.0, 0.5]);
    // A plane's UVs run 0 to 1 across it.
    close(&get(&mut s, &format!("{n}.parameterU")), 0.75);
    let v = get(&mut s, &format!("{n}.parameterV")).as_f64().unwrap();
    assert!((v - 0.25).abs() < 1e-6 || (v - 0.75).abs() < 1e-6, "{v}");
}

#[test]
fn locked_nodes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    assert_eq!(run(&mut s, "lockNode", json!({"args": ["a"], "q": true, "queried": ["lock"]})), json!([false]));
    run(&mut s, "lockNode", json!({"args": ["a"], "l": true}));
    assert_eq!(run(&mut s, "lockNode", json!({"args": ["a"], "q": true})), json!([true]));
    assert!(s.run("delete", json!({"args": ["a"]})).is_err());
    assert!(s.run("rename", json!({"args": ["a", "b"]})).is_err());
    // Saved, in both formats.
    for ext in ["ma", "aztek"] {
        let path = temp(&format!("locked.{ext}"));
        run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
        let mut t = Session::new();
        run(&mut t, "file", json!({"args": [path], "open": true}));
        assert_eq!(run(&mut t, "lockNode", json!({"args": ["a"], "q": true})), json!([true]), "{ext}");
    }
    run(&mut s, "undo", json!({}));
    run(&mut s, "lockNode", json!({"args": ["a"], "l": false}));
    run(&mut s, "rename", json!({"args": ["a", "b"]}));
}

fn cube_file(ext: &str) -> String {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "spaceLocator", json!({"name": "loc"}));
    // Tests run in parallel: each its own file, or one reads what another is rewriting.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = temp(&format!("ref_source_{n}.{ext}"));
    run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
    path
}

#[test]
fn reference_in_and_out() {
    for ext in ["ma", "aztek"] {
        let path = cube_file(ext);
        let mut s = Session::new();
        assert_eq!(run(&mut s, "file", json!({"args": [path.clone()], "r": true, "ns": "x"})), json!(path));
        assert_eq!(run(&mut s, "objExists", json!({"args": ["x:box"]})), true);
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["x:box"], "inr": true})), true);
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["x:box"], "f": true})), json!(path));
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["x:box"], "ns": true})), ":x");
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["x:box"], "rfn": true})), "xRN");
        let nodes = run(&mut s, "referenceQuery", json!({"args": ["xRN"], "n": true}));
        assert!(nodes.as_array().unwrap().iter().any(|x| x == "x:box"), "{nodes}");
        assert_eq!(run(&mut s, "file", json!({"q": true, "r": true, "queried": ["reference"]})), json!([path.clone()]));
        // Referenced nodes can't go by themselves.
        assert!(s.run("delete", json!({"args": ["x:box"]})).is_err());
        assert!(s.run("rename", json!({"args": ["x:box", "y"]})).is_err());
        run(&mut s, "polyCube", json!({"name": "own"}));
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["own"], "inr": true})), false);

        // Unload takes it away and keeps the edit; load puts both back.
        run(&mut s, "setAttr", json!({"args": ["x:box.tx", 7.0]}));
        run(&mut s, "file", json!({"ur": "xRN"}));
        assert_eq!(run(&mut s, "objExists", json!({"args": ["x:box"]})), false);
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["xRN"], "il": true})), false);
        run(&mut s, "file", json!({"lr": "xRN"}));
        close(&get(&mut s, "x:box.tx"), 7.0);
        assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["xRN"], "il": true})), true);

        run(&mut s, "file", json!({"args": [path.clone()], "rr": true}));
        assert_eq!(run(&mut s, "objExists", json!({"args": ["x:box"]})), false);
        assert_eq!(run(&mut s, "objExists", json!({"args": ["xRN"]})), false);
        assert_eq!(run(&mut s, "file", json!({"q": true, "r": true, "queried": ["reference"]})), json!([]));
    }
}

#[test]
fn second_reference_gets_a_copy_number() {
    let path = cube_file("ma");
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [path.clone()], "r": true, "ns": "a"}));
    let second = run(&mut s, "file", json!({"args": [path.clone()], "r": true, "ns": "a"}));
    assert_eq!(second, json!(format!("{path}{{1}}")));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["a1:box"]})), true);
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["a1:box"], "f": true})), second);
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["a1:box"], "f": true, "wcn": true})), json!(path));
}

#[test]
fn saved_as_the_reference_with_its_edits() {
    for (src, dst) in [("ma", "ma"), ("aztek", "aztek"), ("ma", "aztek")] {
        let path = cube_file(src);
        let mut s = Session::new();
        run(&mut s, "file", json!({"args": [path.clone()], "r": true, "ns": "x"}));
        run(&mut s, "polyCube", json!({"name": "own"}));
        run(&mut s, "setAttr", json!({"args": ["x:box.tx", 2.5]}));
        run(&mut s, "connectAttr", json!({"args": ["x:box.ry", "own.ty"]}));
        let scene = temp(&format!("main_{src}_{dst}.{dst}"));
        run(&mut s, "file", json!({"args": [scene.clone()], "save": true}));
        let text = std::fs::read_to_string(&scene).unwrap();
        assert!(!text.contains("createNode transform -n \"x:box\""), "the contents stay out");
        assert!(text.contains(&path));
        let mut t = Session::new();
        run(&mut t, "file", json!({"args": [scene], "open": true}));
        assert_eq!(run(&mut t, "referenceQuery", json!({"args": ["x:box"], "inr": true})), true, "{src} {dst}");
        close(&get(&mut t, "x:box.tx"), 2.5);
        run(&mut t, "setAttr", json!({"args": ["x:box.ry", 11.0]}));
        close(&get(&mut t, "own.ty"), 11.0);
        assert_eq!(run(&mut t, "objExists", json!({"args": ["own"]})), true);
    }
}

#[test]
fn instances_survive_ma() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    let made = run(&mut s, "instance", json!({"args": ["a"], "name": "b"}));
    assert_eq!(made, json!(["b"]));
    run(&mut s, "setAttr", json!({"args": ["b.tx", 4.0]}));
    let path = temp("instance.ma");
    run(&mut s, "file", json!({"args": [path.clone()], "save": true}));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("parent -s -nc -r -add"), "{text}");
    let mut t = Session::new();
    run(&mut t, "file", json!({"args": [path], "open": true}));
    assert_eq!(run(&mut t, "instanceInfo", json!({"args": ["b"]}))["kind"], "instance");
    close(&get(&mut t, "b.tx"), 4.0);
    assert_eq!(run(&mut t, "polyEvaluate", json!({"args": ["b"], "v": true})), 8);
}
