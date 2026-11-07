//! The reference's core scene commands: flags, short names, modes and return values.

use aztek_scene::{scene_file, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}
fn get(s: &mut Session, plug: &str) -> Value {
    run(s, "getAttr", json!({"args": [plug]}))
}
fn set(s: &mut Session, plug: &str, v: Value) {
    run(s, "setAttr", json!({"args": [plug, v]}));
}
fn close3(v: &Value, want: [f64; 3]) {
    let a = v.as_array().unwrap_or_else(|| panic!("not a list: {v}"));
    for i in 0..3 {
        assert!((a[i].as_f64().unwrap() - want[i]).abs() < 1e-6, "got {v}, wanted {want:?}");
    }
}
fn close(v: &Value, want: f64) {
    assert!((v.as_f64().unwrap_or_else(|| panic!("{v}")) - want).abs() < 1e-6, "got {v}, wanted {want}");
}
fn tree() -> Session {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "group", json!({"args": ["b"], "name": "g"}));
    run(&mut s, "parent", json!({"args": ["g", "a"]}));
    s
}

#[test]
fn list_relatives() {
    let mut s = tree();
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["g"], "c": true})), json!(["b"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["b"], "p": true})), json!(["g"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["b"], "s": true})), json!(["bShape"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["b"], "p": true, "f": true})), json!(["|a|g"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["g"], "ad": true, "typ": "mesh"})), json!(["bShape"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["g"], "ad": true, "s": true, "ni": true})), json!(["bShape"]));
    // Deepest first.
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["a"], "ad": true, "typ": "transform"})), json!(["b", "g"]));
}

#[test]
fn list_connections() {
    let mut s = tree();
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "md"}));
    run(&mut s, "connectAttr", json!({"args": ["a.tx", "md.input1X"]}));
    run(&mut s, "connectAttr", json!({"args": ["md.outputX", "b.ty"]}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["md"], "s": true, "d": false})), json!(["a"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["md"], "d": true, "s": false, "p": true})), json!(["b.translateY"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["md.outputX"], "c": true, "p": true})), json!(["md.outputX", "b.translateY"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["md"], "t": "transform", "s": false})), json!(["b"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["md"], "et": "transform", "s": false})), json!(["b"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["md"], "t": "mesh"})), json!([]));
    // A shape reads as its transform unless -sh.
    run(&mut s, "createNode", json!({"args": ["closestPointOnMesh"], "name": "cpm"}));
    run(&mut s, "connectAttr", json!({"args": ["bShape.outMesh", "cpm.inMesh"]}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["cpm"], "s": true, "d": false})), json!(["b"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["cpm"], "s": true, "d": false, "sh": true})), json!(["bShape"]));
    // Through conversion nodes.
    run(&mut s, "createNode", json!({"args": ["unitConversion"], "name": "uc"}));
    run(&mut s, "createNode", json!({"args": ["addDoubleLinear"], "name": "adl"}));
    run(&mut s, "connectAttr", json!({"args": ["a.ty", "uc.input"]}));
    run(&mut s, "connectAttr", json!({"args": ["uc.output", "adl.input1"]}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["adl"], "s": true, "d": false})), json!(["uc"]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["adl"], "s": true, "d": false, "scn": true})), json!(["a"]));
}

#[test]
fn list_attr() {
    let mut s = tree();
    let k = run(&mut s, "listAttr", json!({"args": ["a"], "k": true}));
    let k: Vec<&str> = k.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect();
    assert!(k.contains(&"visibility") && k.contains(&"translateX") && k.contains(&"scaleZ"));
    assert!(!k.contains(&"translate"));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "speed", "at": "double", "k": true}));
    assert_eq!(run(&mut s, "listAttr", json!({"args": ["a"], "ud": true})), json!(["speed"]));
    run(&mut s, "setAttr", json!({"args": ["a.tx"], "l": true}));
    assert_eq!(run(&mut s, "listAttr", json!({"args": ["a"], "l": true})), json!(["translateX"]));
    let unlocked = run(&mut s, "listAttr", json!({"args": ["a"], "u": true, "k": true}));
    assert!(!unlocked.as_array().unwrap().contains(&json!("translateX")));
    assert_eq!(run(&mut s, "listAttr", json!({"args": ["a"], "st": "transl*Y"})), json!(["translateY"]));
    assert_eq!(run(&mut s, "listAttr", json!({"args": ["a"], "st": "translateY", "sn": true})), json!(["ty"]));
    run(&mut s, "createNode", json!({"args": ["plusMinusAverage"], "name": "pma"}));
    set(&mut s, "pma.input1D[2]", json!(1.0));
    let m = run(&mut s, "listAttr", json!({"args": ["pma"], "m": true}));
    assert!(m.as_array().unwrap().contains(&json!("input1D[2]")));
    let lf = run(&mut s, "listAttr", json!({"args": ["a"], "lf": true}));
    assert!(lf.as_array().unwrap().contains(&json!("translateX")) && !lf.as_array().unwrap().contains(&json!("translate")));
}

#[test]
fn get_attr_flags() {
    let mut s = tree();
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "typ": true})), "doubleLinear");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.rx"], "typ": true})), "doubleAngle");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.translate"], "typ": true})), "double3");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.matrix"], "typ": true})), "matrix");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.v"], "typ": true})), "bool");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.ro"], "typ": true})), "enum");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.ro"], "as": true})), "xyz");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "k": true})), true);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "cb": true})), false);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "se": true})), true);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.matrix"], "se": true})), false);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "l": true})), false);
    run(&mut s, "createNode", json!({"args": ["plusMinusAverage"], "name": "pma"}));
    set(&mut s, "pma.input1D[0]", json!(1.0));
    set(&mut s, "pma.input1D[5]", json!(2.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pma.input1D"], "s": true})), 2);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["pma.input1D"], "mi": true})), json!([0, 5]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "s": true})), 1);
    assert_eq!(get(&mut s, "pma.input1D"), json!([1.0, 2.0]));
    // At another time, leaving the current one alone.
    run(&mut s, "setKeyframe", json!({"args": ["a.ty"], "time": 1, "value": 0.0}));
    run(&mut s, "setKeyframe", json!({"args": ["a.ty"], "time": 11, "value": 10.0}));
    run(&mut s, "currentTime", json!({"args": [1]}));
    close(&run(&mut s, "getAttr", json!({"args": ["a.ty"], "t": 11})), 10.0);
    close(&get(&mut s, "a.ty"), 0.0);
    // Components.
    let v = get(&mut s, "a.vtx[0]");
    close3(&v, [-0.5, -0.5, 0.5]);
    // A plain compound and a whole element of a multi compound.
    run(&mut s, "createNode", json!({"args": ["remapValue"], "name": "rv"}));
    assert_eq!(get(&mut s, "rv.value[1]").as_array().unwrap().len(), 3);
}

#[test]
fn set_attr_flags_and_types() {
    let mut s = tree();
    run(&mut s, "setAttr", json!({"args": ["a.translate", 1, 2, 3], "typ": "double3"}));
    close3(&get(&mut s, "a.translate"), [1.0, 2.0, 3.0]);
    // Channels hide and show one by one.
    run(&mut s, "setAttr", json!({"args": ["a.tx"], "k": false}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "k": true})), false);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.ty"], "k": true})), true);
    run(&mut s, "setAttr", json!({"args": ["a.tx"], "cb": true}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "cb": true})), true);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.tx"], "cb": true})), false);
    // Lock with a value, then a refusal.
    run(&mut s, "setAttr", json!({"args": ["a.tz", 9.0], "l": true}));
    close(&get(&mut s, "a.tz"), 9.0);
    assert!(s.run("setAttr", json!({"args": ["a.tz", 1.0]})).is_err());
    run(&mut s, "setAttr", json!({"args": ["a.tz"], "l": false}));

    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "label", "dt": "string"}));
    run(&mut s, "setAttr", json!({"args": ["a.label", "hello"], "typ": "string"}));
    assert_eq!(get(&mut s, "a.label"), "hello");
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "tags", "dt": "stringArray"}));
    run(&mut s, "setAttr", json!({"args": ["a.tags", 2, "x", "y"], "typ": "stringArray"}));
    assert_eq!(get(&mut s, "a.tags"), json!(["x", "y"]));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "nums", "dt": "doubleArray"}));
    run(&mut s, "setAttr", json!({"args": ["a.nums", [1.0, 2.0, 3.0]], "typ": "doubleArray"}));
    assert_eq!(get(&mut s, "a.nums"), json!([1.0, 2.0, 3.0]));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "mat", "at": "matrix"}));
    let m: Vec<f64> = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 4.0, 5.0, 6.0, 1.0];
    run(&mut s, "setAttr", json!({"args": ["a.mat", m], "typ": "matrix"}));
    close(&get(&mut s, "a.mat")[13], 5.0);
    // Points by component.
    run(&mut s, "setAttr", json!({"args": ["a.vtx[0]", 7.0, 8.0, 9.0]}));
    close3(&get(&mut s, "a.vtx[0]"), [7.0, 8.0, 9.0]);
}

#[test]
fn add_attr_query_and_edit() {
    let mut s = tree();
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "power", "sn": "pw", "nn": "Power Level", "at": "double", "dv": 2.0, "min": 0.0, "max": 10.0, "smn": 1.0, "smx": 5.0, "k": true}));
    close(&get(&mut s, "a.power"), 2.0);
    close(&get(&mut s, "a.pw"), 2.0);
    set(&mut s, "a.power", json!(50.0));
    close(&get(&mut s, "a.power"), 10.0);
    let q = |s: &mut Session, what: &str| run(s, "addAttr", json!({"args": ["a.power"], "query": true, "queried": [what]}));
    assert_eq!(q(&mut s, "attributeType"), "double");
    assert_eq!(q(&mut s, "longName"), "power");
    assert_eq!(q(&mut s, "shortName"), "pw");
    assert_eq!(q(&mut s, "niceName"), "Power Level");
    close(&q(&mut s, "defaultValue"), 2.0);
    close(&q(&mut s, "maxValue"), 10.0);
    close(&q(&mut s, "softMinValue"), 1.0);
    assert_eq!(q(&mut s, "keyable"), true);
    assert_eq!(run(&mut s, "addAttr", json!({"args": ["a"], "ln": "power", "query": true, "queried": ["exists"]})), true);
    assert_eq!(run(&mut s, "addAttr", json!({"args": ["a"], "ln": "none", "query": true, "queried": ["exists"]})), false);

    run(&mut s, "addAttr", json!({"args": ["a.power"], "edit": true, "max": 100.0, "nn": "Pow"}));
    set(&mut s, "a.power", json!(50.0));
    close(&get(&mut s, "a.power"), 50.0);
    assert_eq!(q(&mut s, "niceName"), "Pow");
    assert!(s.run("addAttr", json!({"args": ["a.tx"], "edit": true, "max": 1.0})).is_err());

    // Enums.
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "mode", "at": "enum", "en": "off:low:high"}));
    set(&mut s, "a.mode", json!("high"));
    assert_eq!(get(&mut s, "a.mode"), 2);
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.mode"], "as": true})), "high");

    // A double3 with named children, the reference's way.
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "tint", "at": "double3"}));
    for c in ["tintR", "tintG", "tintB"] {
        run(&mut s, "addAttr", json!({"args": ["a"], "ln": c, "at": "double", "p": "tint"}));
    }
    set(&mut s, "a.tintG", json!(0.5));
    close3(&get(&mut s, "a.tint"), [0.0, 0.5, 0.0]);

    // A plain compound.
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "group1", "at": "compound", "nc": 2}));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "kidA", "at": "double", "p": "group1"}));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "kidB", "at": "double", "p": "group1"}));
    set(&mut s, "a.group1.kidB", json!(3.0));
    close(&get(&mut s, "a.kidB"), 3.0);
    assert_eq!(get(&mut s, "a.group1"), json!([0.0, 3.0]));
    assert_eq!(run(&mut s, "attributeQuery", json!({"args": ["group1"], "n": "a", "lc": true})), json!(["kidA", "kidB"]));

    // Multi.
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "many", "at": "double", "m": true, "im": true}));
    set(&mut s, "a.many[3]", json!(1.5));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["a.many"], "mi": true})), json!([3]));

    // Rename and delete.
    run(&mut s, "renameAttr", json!({"args": ["a.power", "force"]}));
    close(&get(&mut s, "a.force"), 50.0);
    assert!(s.run("renameAttr", json!({"args": ["a.tx", "bad"]})).is_err());
    run(&mut s, "deleteAttr", json!({"args": ["a.force"]}));
    assert!(s.run("getAttr", json!({"args": ["a.force"]})).is_err());
    run(&mut s, "deleteAttr", json!({"args": ["a"], "at": "mode"}));
    assert!(s.run("getAttr", json!({"args": ["a.mode"]})).is_err());
}

#[test]
fn attribute_query() {
    let mut s = tree();
    let q = |s: &mut Session, attr: &str, flag: &str| run(s, "attributeQuery", json!({"args": [attr], "n": "a", flag: true}));
    assert_eq!(q(&mut s, "tx", "ex"), true);
    assert_eq!(q(&mut s, "nothing", "ex"), false);
    assert_eq!(q(&mut s, "tx", "k"), true);
    assert_eq!(q(&mut s, "tx", "ch"), false);
    assert_eq!(q(&mut s, "translate", "lc"), json!(["translateX", "translateY", "translateZ"]));
    assert_eq!(q(&mut s, "tx", "lp"), json!(["translate"]));
    assert_eq!(q(&mut s, "tx", "ln"), "translateX");
    assert_eq!(q(&mut s, "translateX", "sn"), "tx");
    assert_eq!(q(&mut s, "translateX", "nn"), "Translate X");
    assert_eq!(q(&mut s, "translate", "at"), "double3");
    assert_eq!(q(&mut s, "ro", "at"), "enum");
    assert_eq!(q(&mut s, "ro", "le"), json!(["xyz:yzx:zxy:xzy:yxz:zyx"]));
    assert_eq!(q(&mut s, "translate", "m"), false);
    assert_eq!(q(&mut s, "scaleX", "ld"), json!([1.0]));
    run(&mut s, "addAttr", json!({"args": ["a"], "ln": "w", "at": "double", "min": 0, "max": 2, "smn": 0.5, "smx": 1.5, "uac": false}));
    assert_eq!(q(&mut s, "w", "mne"), true);
    assert_eq!(q(&mut s, "w", "mn"), json!([0.0]));
    assert_eq!(q(&mut s, "w", "mx"), json!([2.0]));
    assert_eq!(q(&mut s, "w", "rg"), json!([0.0, 2.0]));
    assert_eq!(q(&mut s, "w", "smn"), json!([0.5]));
    assert_eq!(q(&mut s, "w", "smx"), json!([1.5]));
    assert_eq!(q(&mut s, "w", "sr"), json!([0.5, 1.5]));
    assert_eq!(run(&mut s, "attributeQuery", json!({"args": ["outColor"], "typ": "condition", "uac": true})), true);
    assert_eq!(run(&mut s, "attributeQuery", json!({"args": ["input1D"], "typ": "plusMinusAverage", "m": true})), true);
}

#[test]
fn types_and_existence() {
    let mut s = tree();
    assert_eq!(run(&mut s, "objExists", json!({"args": ["a"]})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["a.tx"]})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["a.nope"]})), false);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["nope"]})), false);
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["a"]})), "transform");
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["aShape"], "i": true})), json!(["dagNode", "shape", "mesh"]));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["a"], "api": true})), "kTransform");
    let d = run(&mut s, "nodeType", json!({"args": ["transform"], "d": true, "itn": true}));
    assert!(d.as_array().unwrap().contains(&json!("joint")));
    assert_eq!(run(&mut s, "objectType", json!({"args": ["a"]})), "transform");
    assert_eq!(run(&mut s, "objectType", json!({"args": ["a"], "isType": "transform"})), true);
    assert_eq!(run(&mut s, "objectType", json!({"args": ["aShape"], "isType": "shape"})), false);
    assert_eq!(run(&mut s, "objectType", json!({"args": ["aShape"], "isa": "shape"})), true);
}

#[test]
fn rename_alias_and_connections() {
    let mut s = tree();
    assert_eq!(run(&mut s, "rename", json!({"args": ["a", "top"]})), "top");
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["top"], "s": true})), json!(["topShape"]));
    assert_eq!(run(&mut s, "rename", json!({"args": ["top", "keep"], "is": true})), "keep");
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["keep"], "s": true})), json!(["topShape"]));

    run(&mut s, "aliasAttr", json!({"args": ["slide", "keep.tx"]}));
    set(&mut s, "keep.slide", json!(4.0));
    close(&get(&mut s, "keep.tx"), 4.0);
    assert_eq!(run(&mut s, "aliasAttr", json!({"args": ["keep"], "query": true})), json!(["slide", "tx"]));
    let doc = scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    scene_file::deserialize(&mut t.graph, &doc).unwrap();
    close(&get(&mut t, "keep.slide"), 4.0);
    run(&mut s, "aliasAttr", json!({"args": ["keep.slide"], "rm": true}));
    assert!(s.run("getAttr", json!({"args": ["keep.slide"]})).is_err());

    // connectionInfo and isConnected.
    run(&mut s, "connectAttr", json!({"args": ["keep.tx", "b.tx"]}));
    assert_eq!(run(&mut s, "isConnected", json!({"args": ["keep.tx", "b.tx"]})), true);
    assert_eq!(run(&mut s, "isConnected", json!({"args": ["b.tx", "keep.tx"]})), false);
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["b.tx"], "sfd": true})), "keep.translateX");
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["keep.tx"], "dfs": true})), json!(["b.translateX"]));
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["b.tx"], "ges": true})), "keep.translateX");
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["b.tx"], "id": true})), true);
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["b.tx"], "is": true})), false);
    assert_eq!(run(&mut s, "connectionInfo", json!({"args": ["keep.tx"], "is": true})), true);
    // A connection to replace needs force; a wrong pair won't disconnect.
    run(&mut s, "createNode", json!({"args": ["floatConstant"], "name": "fc"}));
    assert!(s.run("connectAttr", json!({"args": ["fc.outFloat", "b.tx"]})).is_err());
    assert!(s.run("disconnectAttr", json!({"args": ["fc.outFloat", "b.tx"]})).is_err());
    run(&mut s, "connectAttr", json!({"args": ["fc.outFloat", "b.tx"], "f": true}));
    assert_eq!(run(&mut s, "isConnected", json!({"args": ["keep.tx", "b.tx"]})), false);
    run(&mut s, "disconnectAttr", json!({"args": ["fc.outFloat", "b.tx"]}));
    assert_eq!(run(&mut s, "isConnected", json!({"args": ["fc.outFloat", "b.tx"]})), false);
    // -l locks the destination.
    run(&mut s, "connectAttr", json!({"args": ["fc.outFloat", "b.ty"], "l": true}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["b.ty"], "l": true})), true);
}

#[test]
fn create_node_and_hierarchy() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "createNode", json!({"args": ["transform"], "n": "t1"})), "t1");
    assert_eq!(run(&mut s, "createNode", json!({"args": ["transform"], "n": "kid", "p": "t1"})), "kid");
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["kid"], "p": true})), json!(["t1"]));
    assert_eq!(run(&mut s, "createNode", json!({"args": ["transform"], "n": "t1", "s": true})), "t1");
    assert_eq!(run(&mut s, "ls", json!({"type": "transform"})).as_array().unwrap().len(), 2);
    run(&mut s, "select", json!({"cl": true}));
    run(&mut s, "createNode", json!({"args": ["transform"], "n": "quiet", "ss": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!([]));

    // Groups: under -p, or the world.
    let mut s = tree();
    run(&mut s, "group", json!({"args": ["b"], "n": "inner", "p": "g"}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["inner"], "p": true})), json!(["g"]));
    run(&mut s, "group", json!({"args": ["inner"], "n": "outer", "w": true}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["outer"], "p": true})), json!([]));
    run(&mut s, "ungroup", json!({"args": ["outer"], "w": true}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["inner"], "p": true})), json!([]));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["outer"]})), false);

    // Parent: world placement kept; -r keeps local; -s moves shapes.
    let mut s = tree();
    set(&mut s, "a.tx", json!(10.0));
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "parent", json!({"args": ["c", "a"]}));
    close(&run(&mut s, "xform", json!({"args": ["c"], "q": true, "ws": true, "t": true}))[0], 0.0);
    close(&get(&mut s, "c.tx"), -10.0);
    run(&mut s, "parent", json!({"args": ["c"], "w": true}));
    close(&get(&mut s, "c.tx"), 0.0);
    run(&mut s, "parent", json!({"args": ["c", "a"], "r": true}));
    close(&get(&mut s, "c.tx"), 0.0);
    close(&run(&mut s, "xform", json!({"args": ["c"], "q": true, "ws": true, "t": true}))[0], 10.0);
    run(&mut s, "createNode", json!({"args": ["transform"], "n": "ctl"}));
    run(&mut s, "parent", json!({"args": ["cShape", "ctl"], "s": true, "r": true}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["ctl"], "s": true})), json!(["cShape"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["c"], "s": true})), json!([]));
}

#[test]
fn delete_flags() {
    let mut s = tree();
    run(&mut s, "setKeyframe", json!({"args": ["a.tx"], "time": 1, "value": 1.0}));
    run(&mut s, "setKeyframe", json!({"args": ["a.tx"], "time": 5, "value": 1.0}));
    run(&mut s, "setKeyframe", json!({"args": ["a.ty"], "time": 1, "value": 1.0}));
    run(&mut s, "setKeyframe", json!({"args": ["a.ty"], "time": 5, "value": 2.0}));
    // Static curves go, moving ones stay.
    run(&mut s, "delete", json!({"args": ["a"], "sc": true}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["a.tx"]})), json!([]));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["a.ty"], "t": "animCurve"})).as_array().unwrap().len(), 1);
    run(&mut s, "delete", json!({"args": ["a"], "c": true, "at": "ty"}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["a.ty"]})), json!([]));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["a"]})), true);

    run(&mut s, "expression", json!({"string": "a.tz = 1;"}));
    run(&mut s, "delete", json!({"args": ["a"], "e": true}));
    assert_eq!(run(&mut s, "ls", json!({"type": "expression"})), json!([]));

    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "pointConstraint", json!({"args": ["c", "a"]}));
    assert!(!run(&mut s, "listConnections", json!({"args": ["a.t"], "s": true, "d": false})).as_array().unwrap().is_empty());
    run(&mut s, "delete", json!({"args": ["a"], "cn": true}));
    assert_eq!(run(&mut s, "ls", json!({"type": "pointConstraint"})), json!([]));

    run(&mut s, "delete", json!({"args": ["b"], "s": true}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["b"]})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["bShape"]})), false);
}

#[test]
fn duplicate_flags() {
    let mut s = tree();
    // Children come back too, unless -rr.
    let full = run(&mut s, "duplicate", json!({"args": ["g"]}));
    assert_eq!(full.as_array().unwrap().len(), 2);
    let roots = run(&mut s, "duplicate", json!({"args": ["g"], "rr": true}));
    assert_eq!(roots.as_array().unwrap().len(), 1);
    let only = run(&mut s, "duplicate", json!({"args": ["g"], "rr": true, "po": true, "n": "bare"}));
    assert_eq!(only, json!(["bare"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["bare"]})), json!([]));
    let mut s = tree();
    let t = run(&mut s, "duplicate", json!({"args": ["b"], "rr": true, "to": true, "n": "noshape"}));
    assert_eq!(t, json!(["noshape"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["noshape"], "s": true})), json!([]));

    // -ic shares the inputs; -un copies the node feeding them.
    run(&mut s, "createNode", json!({"args": ["floatConstant"], "n": "fc"}));
    set(&mut s, "fc.inFloat", json!(3.0));
    run(&mut s, "connectAttr", json!({"args": ["fc.outFloat", "b.tx"]}));
    run(&mut s, "duplicate", json!({"args": ["b"], "ic": true, "n": "withIn"}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["withIn.tx"], "s": true, "d": false})), json!(["fc"]));
    run(&mut s, "duplicate", json!({"args": ["b"], "un": true, "n": "withCopy"}));
    let src = run(&mut s, "listConnections", json!({"args": ["withCopy.tx"], "s": true, "d": false}));
    assert_ne!(src, json!(["fc"]));
    close(&get(&mut s, "withCopy.tx"), 3.0);
}

#[test]
fn select_flags() {
    let mut s = tree();
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "select", json!({"args": ["a"], "r": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["a"]));
    run(&mut s, "select", json!({"args": ["c"], "add": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["a", "c"]));
    run(&mut s, "select", json!({"args": ["b"], "af": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["b", "a", "c"]));
    run(&mut s, "select", json!({"args": ["a"], "d": true}));
    assert_eq!(run(&mut s, "selected", json!({})), json!(["b", "c"]));
    run(&mut s, "select", json!({"args": ["c"], "tgl": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["b"]));
    run(&mut s, "select", json!({"cl": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!([]));
    run(&mut s, "select", json!({"args": ["g"], "hi": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["g", "b", "bShape"]));
    run(&mut s, "select", json!({"ado": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["a", "c"]));
    run(&mut s, "select", json!({"args": ["*Shape"]}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})).as_array().unwrap().len(), 3);
    run(&mut s, "setAttr", json!({"args": ["c.v", false]}));
    run(&mut s, "select", json!({"ado": true, "vis": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["a"]));

    // A set stands for its members unless -ne.
    let set = run(&mut s, "sets", json!({"args": ["a", "c"], "n": "mine"}));
    assert_eq!(set, "mine");
    run(&mut s, "select", json!({"args": ["mine"]}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["a", "c"]));
    run(&mut s, "select", json!({"args": ["mine"], "ne": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["mine"]));
}

#[test]
fn xform_query_and_set() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "p"}));
    run(&mut s, "polyCube", json!({"name": "a", "width": 2, "height": 4, "depth": 6}));
    run(&mut s, "parent", json!({"args": ["a", "p"]}));
    set(&mut s, "p.translate", json!([10.0, 0.0, 0.0]));
    set(&mut s, "a.translate", json!([1.0, 2.0, 3.0]));
    let q = |s: &mut Session, flag: &str, ws: bool| run(s, "xform", json!({"args": ["a"], "q": true, "ws": ws, flag: true}));
    close3(&q(&mut s, "t", false), [1.0, 2.0, 3.0]);
    close3(&q(&mut s, "t", true), [11.0, 2.0, 3.0]);
    run(&mut s, "xform", json!({"args": ["a"], "ro": [0.0, 90.0, 0.0]}));
    close3(&q(&mut s, "ro", false), [0.0, 90.0, 0.0]);
    run(&mut s, "xform", json!({"args": ["a"], "ro": [0.0, 10.0, 0.0], "r": true}));
    close3(&q(&mut s, "ro", false), [0.0, 100.0, 0.0]);
    run(&mut s, "xform", json!({"args": ["a"], "ro": [0.0, 0.0, 0.0]}));
    run(&mut s, "xform", json!({"args": ["a"], "s": [2.0, 2.0, 2.0]}));
    run(&mut s, "xform", json!({"args": ["a"], "s": [2.0, 1.0, 1.0], "r": true}));
    close3(&q(&mut s, "s", false), [4.0, 2.0, 2.0]);
    run(&mut s, "xform", json!({"args": ["a"], "s": [1.0, 1.0, 1.0]}));
    run(&mut s, "xform", json!({"args": ["a"], "sh": [0.5, 0.0, 0.0]}));
    close3(&q(&mut s, "sh", false), [0.5, 0.0, 0.0]);
    run(&mut s, "xform", json!({"args": ["a"], "sh": [0.0, 0.0, 0.0]}));
    assert_eq!(run(&mut s, "xform", json!({"args": ["a"], "q": true, "roo": true})), "xyz");
    run(&mut s, "xform", json!({"args": ["a"], "ra": [10.0, 0.0, 0.0]}));
    close3(&q(&mut s, "ra", false), [10.0, 0.0, 0.0]);
    run(&mut s, "xform", json!({"args": ["a"], "ra": [0.0, 0.0, 0.0]}));
    let bb = q(&mut s, "bb", true);
    assert_eq!(bb.as_array().unwrap().len(), 6);
    close(&bb[0], 10.0);
    close(&bb[3], 12.0);
    close(&bb[4], 4.0);
    let local = q(&mut s, "bb", false);
    close(&local[0], -1.0);

    // Pivots: moving one leaves the object where it is, with the compensation in rotatePivotTranslate.
    set(&mut s, "a.ry", json!(90.0));
    let before = get(&mut s, "a.worldMatrix");
    run(&mut s, "xform", json!({"args": ["a"], "rp": [1.0, 0.0, 0.0]}));
    close3(&q(&mut s, "rp", false), [1.0, 0.0, 0.0]);
    let after = get(&mut s, "a.worldMatrix");
    for i in 0..16 {
        close(&after[i], before[i].as_f64().unwrap());
    }
    assert_ne!(q(&mut s, "rt", false), json!([0.0, 0.0, 0.0]));
    run(&mut s, "xform", json!({"args": ["a"], "ws": true, "piv": [0.0, 0.0, 0.0]}));
    close3(&q(&mut s, "rp", true), [0.0, 0.0, 0.0]);
    let piv = q(&mut s, "piv", true);
    assert_eq!(piv.as_array().unwrap().len(), 6);
    let after = get(&mut s, "a.worldMatrix");
    for i in 0..16 {
        close(&after[i], before[i].as_f64().unwrap());
    }

    // World rotation under a turned parent.
    set(&mut s, "p.ry", json!(30.0));
    run(&mut s, "xform", json!({"args": ["a"], "ws": true, "ro": [0.0, 45.0, 0.0]}));
    close3(&q(&mut s, "ro", false), [0.0, 15.0, 0.0]);
    close3(&q(&mut s, "ro", true), [0.0, 45.0, 0.0]);

    // Matrices.
    let m: Vec<f64> = vec![1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 7.0, 8.0, 9.0, 1.0];
    run(&mut s, "xform", json!({"args": ["p"], "ro": [0.0, 0.0, 0.0], "t": [0.0, 0.0, 0.0]}));
    run(&mut s, "xform", json!({"args": ["a"], "m": m}));
    close3(&json!(q(&mut s, "m", false).as_array().unwrap()[12..15].to_vec()), [7.0, 8.0, 9.0]);

    // Rotate order preserving the orientation.
    run(&mut s, "xform", json!({"args": ["a"], "ro": [30.0, 40.0, 50.0]}));
    let before = get(&mut s, "a.matrix");
    run(&mut s, "xform", json!({"args": ["a"], "roo": "zyx", "p": true}));
    assert_eq!(run(&mut s, "xform", json!({"args": ["a"], "q": true, "roo": true})), "zyx");
    let after = get(&mut s, "a.matrix");
    for i in 0..16 {
        close(&after[i], before[i].as_f64().unwrap());
    }

    // Centre pivots.
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "move", json!({"args": ["c.vtx[0:7]"], "by": [5.0, 0.0, 0.0]}));
    run(&mut s, "xform", json!({"args": ["c"], "cp": true}));
    close3(&run(&mut s, "xform", json!({"args": ["c"], "q": true, "ws": true, "rp": true})), [5.0, 0.0, 0.0]);
    run(&mut s, "xform", json!({"args": ["c"], "ztp": true}));
    close3(&run(&mut s, "xform", json!({"args": ["c"], "q": true, "rp": true})), [0.0, 0.0, 0.0]);
}

#[test]
fn move_rotate_scale_forms() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "p"}));
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "parent", json!({"args": ["a", "p"]}));
    set(&mut s, "p.tx", json!(100.0));
    // Absolute, by to=, is world; relative by by=.
    run(&mut s, "move", json!({"args": ["a"], "to": [1.0, 2.0, 3.0]}));
    close3(&get(&mut s, "a.translate"), [-99.0, 2.0, 3.0]);
    run(&mut s, "move", json!({"args": ["a"], "by": [1.0, 0.0, 0.0]}));
    close3(&get(&mut s, "a.translate"), [-98.0, 2.0, 3.0]);
    // to= with relative on, what r=True gives through the Python and MEL layers.
    run(&mut s, "move", json!({"args": ["a"], "to": [1.0, 0.0, 0.0], "r": true}));
    close3(&get(&mut s, "a.translate"), [-97.0, 2.0, 3.0]);
    // Local absolute.
    run(&mut s, "move", json!({"args": ["a"], "to": [1.0, 2.0, 3.0], "ls": true}));
    close3(&get(&mut s, "a.translate"), [1.0, 2.0, 3.0]);
    // Named axes with values left positional: move 5 a -x, and two axes.
    run(&mut s, "move", json!({"args": [50.0, "a"], "x": true, "ls": true}));
    close3(&get(&mut s, "a.translate"), [50.0, 2.0, 3.0]);
    run(&mut s, "move", json!({"args": [7.0, 8.0, "a"], "xy": true, "ls": true}));
    close3(&get(&mut s, "a.translate"), [7.0, 8.0, 3.0]);
    run(&mut s, "move", json!({"args": [1.0, "a"], "y": true, "r": true}));
    close3(&get(&mut s, "a.translate"), [7.0, 9.0, 3.0]);
    run(&mut s, "move", json!({"args": ["a"], "to": [1.0, 1.0, 1.0], "xyz": true, "ls": true}));
    close3(&get(&mut s, "a.translate"), [1.0, 1.0, 1.0]);
    // The older axis form still sets one world axis.
    run(&mut s, "move", json!({"args": ["a"], "x": 0.0}));
    close(&run(&mut s, "xform", json!({"args": ["a"], "q": true, "ws": true, "t": true}))[0], 0.0);
    // Object space, along the object's own axes.
    set(&mut s, "a.translate", json!([0.0, 0.0, 0.0]));
    set(&mut s, "a.ry", json!(90.0));
    run(&mut s, "move", json!({"args": ["a"], "to": [1.0, 0.0, 0.0], "r": true, "os": true}));
    close3(&get(&mut s, "a.translate"), [0.0, 0.0, -1.0]);
    set(&mut s, "a.ry", json!(0.0));
    // -pcp leaves children where they are.
    set(&mut s, "a.translate", json!([0.0, 0.0, 0.0]));
    run(&mut s, "move", json!({"args": ["p"], "by": [5.0, 0.0, 0.0], "pcp": true}));
    close(&run(&mut s, "xform", json!({"args": ["a"], "q": true, "ws": true, "t": true}))[0], 100.0);

    // Rotate.
    run(&mut s, "rotate", json!({"args": ["a"], "to": [10.0, 20.0, 30.0]}));
    close3(&get(&mut s, "a.rotate"), [10.0, 20.0, 30.0]);
    run(&mut s, "rotate", json!({"args": [40.0, "a"], "x": true}));
    close3(&get(&mut s, "a.rotate"), [40.0, 20.0, 30.0]);
    run(&mut s, "rotate", json!({"args": ["a"], "to": [0.0, 0.0, 0.0]}));
    run(&mut s, "rotate", json!({"args": ["a"], "to": [0.0, 90.0, 0.0], "r": true, "os": true}));
    close3(&get(&mut s, "a.rotate"), [0.0, 90.0, 0.0]);
    set(&mut s, "a.rotate", json!([0.0, 0.0, 0.0]));
    // About a point.
    run(&mut s, "polyCube", json!({"name": "c"}));
    set(&mut s, "c.translate", json!([1.0, 0.0, 0.0]));
    run(&mut s, "rotate", json!({"args": ["c"], "to": [0.0, 90.0, 0.0], "r": true, "p": [0.0, 0.0, 0.0]}));
    close3(&get(&mut s, "c.translate"), [0.0, 0.0, -1.0]);

    // Scale.
    run(&mut s, "scale", json!({"args": ["c"], "to": [2.0, 3.0, 4.0]}));
    close3(&get(&mut s, "c.scale"), [2.0, 3.0, 4.0]);
    run(&mut s, "scale", json!({"args": ["c"], "to": [2.0, 1.0, 1.0], "r": true}));
    close3(&get(&mut s, "c.scale"), [4.0, 3.0, 4.0]);
    run(&mut s, "scale", json!({"args": [5.0, "c"], "y": true}));
    close3(&get(&mut s, "c.scale"), [4.0, 5.0, 4.0]);
    set(&mut s, "c.translate", json!([1.0, 0.0, 0.0]));
    set(&mut s, "c.scale", json!([1.0, 1.0, 1.0]));
    run(&mut s, "scale", json!({"args": ["c"], "by": [2.0, 2.0, 2.0], "p": [0.0, 0.0, 0.0]}));
    close3(&get(&mut s, "c.translate"), [2.0, 0.0, 0.0]);
}

#[test]
fn make_identity_and_locators() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    set(&mut s, "a.translate", json!([1.0, 2.0, 3.0]));
    set(&mut s, "a.sx", json!(2.0));
    // Only the channels named.
    run(&mut s, "makeIdentity", json!({"args": ["a"], "a": true, "t": true}));
    close3(&get(&mut s, "a.translate"), [0.0, 0.0, 0.0]);
    close(&get(&mut s, "a.sx"), 2.0);
    close3(&run(&mut s, "pointPosition", json!({"args": ["a.vtx[0]"]})), [0.0, 1.5, 3.5]);
    run(&mut s, "makeIdentity", json!({"args": ["a"], "a": true}));
    close(&get(&mut s, "a.sx"), 1.0);
    // Without -a: back to identity, and the object moves.
    set(&mut s, "a.tx", json!(5.0));
    run(&mut s, "makeIdentity", json!({"args": ["a"]}));
    close(&get(&mut s, "a.tx"), 0.0);
    close3(&run(&mut s, "pointPosition", json!({"args": ["a.vtx[0]"]})), [0.0, 1.5, 3.5]);

    assert_eq!(run(&mut s, "spaceLocator", json!({"n": "l1", "p": [1.0, 2.0, 3.0]})), json!(["l1"]));
    close3(&get(&mut s, "lShape1.localPosition"), [1.0, 2.0, 3.0]);
    run(&mut s, "spaceLocator", json!({"n": "l2", "a": true, "p": [1.0, 2.0, 3.0]}));
    close3(&get(&mut s, "l2.rotatePivot"), [1.0, 2.0, 3.0]);
    close3(&get(&mut s, "l2.scalePivot"), [1.0, 2.0, 3.0]);
}

#[test]
fn sets_command() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "polyCube", json!({"name": "c"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["a", "b"], "n": "s1"})), "s1");
    assert_eq!(run(&mut s, "sets", json!({"args": ["s1"], "q": true})), json!(["a", "b"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s1"], "q": true, "queried": ["size"]})), 2);
    assert_eq!(run(&mut s, "sets", json!({"em": true, "n": "s2"})), "s2");
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "q": true})), json!([]));
    run(&mut s, "sets", json!({"args": ["c"], "add": "s2"}));
    run(&mut s, "sets", json!({"args": ["a"], "fe": "s2"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "q": true})), json!(["c", "a"]));
    run(&mut s, "sets", json!({"args": ["c"], "rm": "s2"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "q": true})), json!(["a"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["a"], "im": "s2"})), true);
    assert_eq!(run(&mut s, "sets", json!({"args": ["b"], "im": "s2"})), false);
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "un": "s1"})), json!(["a", "b"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "int": "s1"})), json!(["a"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "sub": "s1"})), json!(["b"]));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s2"], "ii": "s1"})), true);
    run(&mut s, "sets", json!({"cl": "s1"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s1"], "q": true})), json!([]));
    // Components.
    run(&mut s, "sets", json!({"args": ["a.vtx[0:2]"], "n": "s3"}));
    assert_eq!(run(&mut s, "sets", json!({"args": ["s3"], "q": true})), json!(["a.vtx[0:2]"]));
}

#[test]
fn undo_info_and_friends() {
    let mut s = Session::new();
    let q = |s: &mut Session, what: &str| run(s, "undoInfo", json!({"query": true, "queried": [what]}));
    assert_eq!(q(&mut s, "state"), true);
    run(&mut s, "polyCube", json!({}));
    assert_eq!(q(&mut s, "undoName"), "polyCube");
    run(&mut s, "undo", json!({}));
    assert_eq!(q(&mut s, "redoName"), "polyCube");
    run(&mut s, "redo", json!({}));
    run(&mut s, "undoInfo", json!({"ock": true, "cn": "both"}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.tx", 1.0]}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.ty", 1.0]}));
    run(&mut s, "undoInfo", json!({"cck": true}));
    assert_eq!(q(&mut s, "undoName"), "both");
    run(&mut s, "undo", json!({}));
    close(&get(&mut s, "pCube1.tx"), 0.0);
    close(&get(&mut s, "pCube1.ty"), 0.0);
    run(&mut s, "undoInfo", json!({"swf": false}));
    assert_eq!(q(&mut s, "state"), false);
    run(&mut s, "undoInfo", json!({"swf": true}));
    assert_eq!(q(&mut s, "state"), true);
    run(&mut s, "flushUndo", json!({}));
    assert_eq!(q(&mut s, "undoQueueEmpty"), true);

    assert!(run(&mut s, "about", json!({"v": true})).is_string());
    assert!(run(&mut s, "about", json!({"api": true})).is_number());
    assert!(run(&mut s, "about", json!({"os": true})).is_string());
    assert_eq!(run(&mut s, "about", json!({"linux": true})), cfg!(target_os = "linux"));
    assert_eq!(run(&mut s, "about", json!({"win": true})), cfg!(windows));
    assert_eq!(run(&mut s, "about", json!({"b": true})), false);

    run(&mut s, "optionVar", json!({"iv": ["count", 5]}));
    run(&mut s, "optionVar", json!({"fv": ["rate", 1.5]}));
    run(&mut s, "optionVar", json!({"sv": ["word", "hi"]}));
    assert_eq!(run(&mut s, "optionVar", json!({"args": ["count"], "q": true})), 5);
    assert_eq!(run(&mut s, "optionVar", json!({"args": ["rate"], "q": true})), 1.5);
    assert_eq!(run(&mut s, "optionVar", json!({"args": ["word"], "q": true})), "hi");
    assert_eq!(run(&mut s, "optionVar", json!({"ex": "count"})), true);
    run(&mut s, "optionVar", json!({"rm": "count"}));
    assert_eq!(run(&mut s, "optionVar", json!({"ex": "count"})), false);
    assert_eq!(run(&mut s, "optionVar", json!({"args": ["count"], "q": true})), 0);
    // The MEL form: name in the flag, value left positional.
    run(&mut s, "optionVar", json!({"args": [9], "iv": "fromMel"}));
    assert_eq!(run(&mut s, "optionVar", json!({"args": ["fromMel"], "q": true})), 9);

    assert_eq!(run(&mut s, "currentUnit", json!({"q": true, "queried": ["linear"]})), "cm");
    assert_eq!(run(&mut s, "currentUnit", json!({"q": true, "queried": ["angle"]})), "deg");
    assert_eq!(run(&mut s, "currentUnit", json!({"q": true, "queried": ["angle"], "f": true})), "degree");
    assert!(run(&mut s, "workspace", json!({"q": true, "queried": ["rootDirectory"]})).as_str().unwrap().ends_with('/'));
    assert_eq!(run(&mut s, "getModifiers", json!({})), 0);
    run(&mut s, "refresh", json!({"f": true}));
    run(&mut s, "dgdirty", json!({"a": true}));
    run(&mut s, "dgeval", json!({"args": ["pCube1.tx"]}));
    assert_eq!(run(&mut s, "getClassification", json!({"args": ["condition"], "sat": "utility"})), true);
    assert_eq!(run(&mut s, "getClassification", json!({"args": ["transform"]})), json!([]));
    run(&mut s, "putenv", json!({"args": ["AZTEK_TEST_VALUE", "yes"]}));
    assert_eq!(run(&mut s, "getenv", json!({"args": ["AZTEK_TEST_VALUE"]})), "yes");
}

#[test]
fn display_and_order_commands() {
    let mut s = tree();
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "polyCube", json!({"name": "d"}));
    run(&mut s, "hide", json!({"args": ["c"]}));
    assert_eq!(get(&mut s, "c.v"), false);
    assert_eq!(run(&mut s, "hide", json!({"args": ["c"], "tv": true})), true);
    run(&mut s, "showHidden", json!({"args": ["c"]}));
    assert_eq!(get(&mut s, "c.v"), true);
    run(&mut s, "hide", json!({"args": ["b"]}));
    run(&mut s, "showHidden", json!({"args": ["g"], "b": true}));
    assert_eq!(get(&mut s, "b.v"), true);
    run(&mut s, "hide", json!({"args": ["c", "d"]}));
    run(&mut s, "showHidden", json!({"a": true}));
    assert_eq!(get(&mut s, "d.v"), true);

    assert_eq!(run(&mut s, "toggle", json!({"args": ["c"], "la": true})), true);
    assert_eq!(get(&mut s, "c.displayLocalAxis"), true);
    assert_eq!(run(&mut s, "toggle", json!({"args": ["c"], "la": true})), false);
    run(&mut s, "toggle", json!({"args": ["c"], "rp": true, "st": true}));
    assert_eq!(get(&mut s, "c.displayRotatePivot"), true);
    assert_eq!(run(&mut s, "toggle", json!({"args": ["c"], "rp": true, "q": true})), true);
    run(&mut s, "displayCull", json!({"args": ["c"], "bfc": true}));
    assert_eq!(run(&mut s, "displayCull", json!({"args": ["c"], "q": true})), true);

    // Order among siblings.
    run(&mut s, "group", json!({"args": ["c", "d"], "n": "row"}));
    run(&mut s, "polyCube", json!({"name": "e"}));
    run(&mut s, "parent", json!({"args": ["e", "row"]}));
    let kids = |s: &mut Session| run(s, "listRelatives", json!({"args": ["row"]}));
    run(&mut s, "reorder", json!({"args": ["d"], "f": true}));
    assert_eq!(kids(&mut s), json!(["d", "c", "e"]));
    run(&mut s, "reorder", json!({"args": ["d"], "r": 1}));
    assert_eq!(kids(&mut s), json!(["c", "d", "e"]));
    run(&mut s, "reorder", json!({"args": ["d"], "b": true}));
    assert_eq!(kids(&mut s), json!(["c", "e", "d"]));

    // Modes.
    run(&mut s, "selectMode", json!({"co": true}));
    assert_eq!(run(&mut s, "selectMode", json!({"q": true, "queried": ["component"]})), true);
    run(&mut s, "selectType", json!({"pe": true}));
    assert_eq!(run(&mut s, "selectType", json!({"q": true, "pe": true})), true);
    run(&mut s, "selectMode", json!({"o": true}));
    assert_eq!(run(&mut s, "selectMode", json!({"q": true, "queried": ["object"]})), true);
    run(&mut s, "snapMode", json!({"gr": true}));
    assert_eq!(run(&mut s, "snapMode", json!({"q": true, "queried": ["grid"]})), true);
    assert_eq!(run(&mut s, "snapMode", json!({"q": true, "queried": ["point"]})), false);
    run(&mut s, "select", json!({"cl": true}));
    run(&mut s, "hilite", json!({"args": ["c"]}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["c"]));
    run(&mut s, "hilite", json!({"args": ["c"], "u": true}));
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!([]));
    // Pick walk with its short flag, returning the selection.
    run(&mut s, "select", json!({"args": ["b"]}));
    assert_eq!(run(&mut s, "pickWalk", json!({"d": "up"})), json!(["g"]));
}

#[test]
fn bounds_align_and_measures() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a", "width": 2, "height": 2, "depth": 2}));
    run(&mut s, "polyCube", json!({"name": "b", "width": 2, "height": 2, "depth": 2}));
    set(&mut s, "b.translate", json!([10.0, 4.0, 0.0]));
    let bb = run(&mut s, "exactWorldBoundingBox", json!({"args": ["a", "b"]}));
    assert_eq!(bb, json!([-1.0, -1.0, -1.0, 11.0, 5.0, 1.0]));
    run(&mut s, "hide", json!({"args": ["b"]}));
    assert_eq!(run(&mut s, "exactWorldBoundingBox", json!({"args": ["a", "b"], "ii": true})), json!([-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]));
    run(&mut s, "showHidden", json!({"args": ["b"]}));

    run(&mut s, "align", json!({"args": ["a", "b"], "y": "Max"}));
    close(&get(&mut s, "a.ty"), 4.0);
    close(&get(&mut s, "b.ty"), 4.0);
    run(&mut s, "align", json!({"args": ["a", "b"], "x": "Mid", "atl": true}));
    close(&get(&mut s, "a.tx"), 10.0);

    // polyEvaluate.
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["a"], "v": true})), 8);
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["a"], "e": true})), 12);
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["a"], "f": true})), 6);
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["a"], "t": true})), 12);
    close(&run(&mut s, "polyEvaluate", json!({"args": ["a"], "a": true})), 24.0);
    set(&mut s, "a.sx", json!(2.0));
    close(&run(&mut s, "polyEvaluate", json!({"args": ["a"], "wa": true})), 40.0);
    close(&run(&mut s, "polyEvaluate", json!({"args": ["a"], "a": true})), 24.0);
    assert!(run(&mut s, "polyEvaluate", json!({"args": ["a"], "fmt": true})).as_str().unwrap().contains("vertex=8"));
    run(&mut s, "select", json!({"args": ["a.vtx[0]", "a.vtx[1]"]}));
    assert_eq!(run(&mut s, "polyEvaluate", json!({"vc": true})), 2);
    let bc = run(&mut s, "polyEvaluate", json!({"bc": true}));
    assert_eq!(bc.as_array().unwrap().len(), 3);
    run(&mut s, "selectMode", json!({"args": ["object"]}));

    // The angleBetween command: axis and angle.
    let ab = run(&mut s, "angleBetween", json!({"v1": [1, 0, 0], "v2": [0, 1, 0]}));
    close3(&ab, [0.0, 0.0, 1.0]);
    close(&ab[3], 90.0);
    close3(&run(&mut s, "angleBetween", json!({"v1": [1, 0, 0], "v2": [0, 1, 0], "er": true})), [0.0, 0.0, 90.0]);

    // Mass renaming.
    assert_eq!(run(&mut s, "searchReplaceNames", json!({"args": ["bSh", "bigSh", "all"]})), 1);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["bigShape"]})), true);
    let names = run(&mut s, "prefixHierarchy", json!({"args": ["a", "L_"]}));
    assert_eq!(names, json!(["L_a", "L_aShape"]));
}

#[test]
fn namespaces() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "namespace", json!({"add": "rig"})), "rig");
    assert_eq!(run(&mut s, "namespace", json!({"ex": "rig"})), true);
    assert_eq!(run(&mut s, "namespace", json!({"ex": "other"})), false);
    assert!(s.run("namespace", json!({"add": "rig"})).is_err());
    assert_eq!(run(&mut s, "namespaceInfo", json!({"cur": true})), ":");
    run(&mut s, "namespace", json!({"set": "rig"}));
    assert_eq!(run(&mut s, "namespaceInfo", json!({"cur": true})), "rig");
    assert_eq!(run(&mut s, "namespace", json!({"q": true})), ":rig");
    // New nodes land in it, shapes too, and names resolve in full.
    run(&mut s, "polyCube", json!({"name": "box"}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["rig:box"]})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["rig:boxShape"]})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["box"]})), false);
    close(&get(&mut s, "rig:box.tx"), 0.0);
    assert_eq!(run(&mut s, "ls", json!({"args": ["rig:*"], "type": "transform"})), json!(["rig:box"]));
    assert_eq!(run(&mut s, "createNode", json!({"args": ["transform"]})), "rig:transform1");
    run(&mut s, "namespace", json!({"add": "arm"}));
    run(&mut s, "namespace", json!({"set": ":"}));
    assert_eq!(run(&mut s, "namespace", json!({"ex": "rig:arm"})), true);
    assert_eq!(run(&mut s, "createNode", json!({"args": ["transform"], "n": "plain"})), "plain");

    assert_eq!(run(&mut s, "namespaceInfo", json!({"lon": true})), json!(["rig"]));
    assert_eq!(run(&mut s, "namespaceInfo", json!({"args": ["rig"], "lon": true})), json!(["rig:arm"]));
    assert_eq!(run(&mut s, "namespaceInfo", json!({"args": ["rig"], "lod": true})), json!(["rig:polyCube1", "rig:box", "rig:boxShape", "rig:transform1"]));
    let polyc = run(&mut s, "namespaceInfo", json!({"args": ["rig"], "ls": true}));
    assert!(polyc.as_array().unwrap().contains(&json!("rig:arm")));
    assert_eq!(run(&mut s, "namespaceInfo", json!({"args": ["rig:arm"], "bn": true})), "arm");
    assert_eq!(run(&mut s, "namespaceInfo", json!({"args": ["rig:arm"], "p": true})), "rig");

    // Saved with the scene.
    let doc = scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    scene_file::deserialize(&mut t.graph, &doc).unwrap();
    assert_eq!(run(&mut t, "namespace", json!({"ex": "rig:arm"})), true);
    close(&get(&mut t, "rig:box.tx"), 0.0);

    // Removal needs an empty namespace, or a merge, or -dnc.
    assert!(s.run("namespace", json!({"rm": "rig"})).is_err());
    run(&mut s, "namespace", json!({"rm": "rig:arm"}));
    assert_eq!(run(&mut s, "namespace", json!({"ex": "rig:arm"})), false);
    run(&mut s, "namespace", json!({"add": "merge"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "n": "merge:m"}));
    run(&mut s, "namespace", json!({"rm": "merge", "mnr": true}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["m"]})), true);
    assert_eq!(run(&mut s, "namespace", json!({"ex": "merge"})), false);
    run(&mut s, "namespace", json!({"rm": "rig", "dnc": true}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["rig:box"]})), false);
    assert_eq!(run(&mut s, "namespace", json!({"ex": "rig"})), false);
    // Undo brings it back, in step with the nodes.
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "namespace", json!({"ex": "rig"})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["rig:box"]})), true);
}

#[test]
fn uuids() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "polyCube", json!({"name": "b"}));
    let ua = run(&mut s, "ls", json!({"args": ["a"], "uuid": true}));
    let ub = run(&mut s, "ls", json!({"args": ["b"], "uid": true}));
    let id = ua[0].as_str().unwrap().to_string();
    assert_eq!(id.len(), 36);
    assert_ne!(ua, ub);
    assert_eq!(run(&mut s, "ls", json!({"args": [id.clone()]})), json!(["a"]));
    // A name change leaves it; a reload keeps it.
    run(&mut s, "rename", json!({"args": ["a", "c"]}));
    assert_eq!(run(&mut s, "ls", json!({"args": [id.clone()]})), json!(["c"]));
    let doc = scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    scene_file::deserialize(&mut t.graph, &doc).unwrap();
    assert_eq!(run(&mut t, "ls", json!({"args": [id.clone()]})), json!(["c"]));
    // A duplicate gets its own.
    let copy = run(&mut s, "duplicate", json!({"args": ["c"], "rr": true}));
    assert_ne!(run(&mut s, "ls", json!({"args": [copy[0].clone()], "uuid": true}))[0], json!(id));
}

#[test]
fn channels_hidden_one_by_one_survive_a_save() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "a"}));
    run(&mut s, "setAttr", json!({"args": ["a.tx"], "k": false}));
    let doc = scene_file::serialize(&s.graph, false);
    let mut t = Session::new();
    scene_file::deserialize(&mut t.graph, &doc).unwrap();
    assert_eq!(run(&mut t, "getAttr", json!({"args": ["a.tx"], "k": true})), false);
    assert_eq!(run(&mut t, "getAttr", json!({"args": ["a.ty"], "k": true})), true);
}
