//! Metadata, nodeCast, nodePreset, files, the intersector and the other general commands.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn scratch(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("aztek_general_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

#[test]
fn expanded_selection_walks_connections() {
    let mut s = Session::new();
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "t1"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "t2"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "t3"}));
    run(&mut s, "connectAttr", json!({"args": ["t1.tx", "t2.tx"]}));
    run(&mut s, "connectAttr", json!({"args": ["t2.ty", "t3.ty"]}));
    run(&mut s, "select", json!({"args": ["t1"]}));
    assert_eq!(run(&mut s, "expandedSelection", json!({"depth": 1, "expansionType": "DG"})), json!(["t1", "t2"]));
    assert_eq!(run(&mut s, "expandedSelection", json!({"depth": 2})), json!(["t1", "t2", "t3"]));
    assert_eq!(run(&mut s, "expandedSelection", json!({"depth": 0})), json!(["t1"]));
    assert!(s.run("expandedSelection", json!({"expansionType": "XX"})).is_err());
}

#[test]
fn metadata_as_the_reference_shows_it() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "dataStructure", json!({"format": "raw", "asString": "name=idStructure:int32=ID"})), json!("idStructure"));
    run(&mut s, "dataStructure", json!({"format": "raw", "asString": "name=NameAndID:string=name:int32=ID"}));
    assert_eq!(run(&mut s, "dataStructure", json!({"name": "NameAndID", "query": true, "listMemberNames": true, "dataType": true})), json!(["name", "string", "ID", "int32"]));
    assert_eq!(run(&mut s, "dataStructure", json!({"name": "NameAndID", "query": true, "asString": true})), json!("name=NameAndID:string=name:int32=ID"));
    run(&mut s, "polyPlane", json!({"name": "smcPlane", "constructionHistory": false}));
    run(&mut s, "select", json!({"args": ["smcPlaneShape"]}));
    run(&mut s, "addMetadata", json!({"structure": "idStructure", "streamName": "idStream", "channelName": "vertex"}));
    run(&mut s, "select", json!({"args": ["smcPlaneShape.vtx[8:10]"]}));
    assert_eq!(run(&mut s, "editMetadata", json!({"streamName": "idStream", "memberName": "ID", "value": 7})), json!(true));
    run(&mut s, "select", json!({"args": ["smcPlaneShape"]}));
    assert_eq!(run(&mut s, "getMetadata", json!({"streamName": "idStream", "memberName": "ID", "channelName": "vertex"})), json!([7, 7, 7]));
    run(&mut s, "editMetadata", json!({"streamName": "idStream", "memberName": "ID", "channelName": "vertex", "value": 8, "index": "14"}));
    assert_eq!(run(&mut s, "getMetadata", json!({"streamName": "idStream", "memberName": "ID", "channelName": "vertex", "index": "14"})), json!([8]));
    run(&mut s, "select", json!({"args": ["smcPlaneShape.vtx[9]"]}));
    run(&mut s, "editMetadata", json!({"streamName": "idStream", "remove": true}));
    assert_eq!(run(&mut s, "getMetadata", json!({"args": ["smcPlaneShape"], "streamName": "idStream", "memberName": "ID"})), json!([7, 7, 8]));
    assert_eq!(run(&mut s, "getMetadata", json!({"args": ["smcPlaneShape"], "listStreamNames": true})), json!(["idStream"]));
    assert_eq!(run(&mut s, "getMetadata", json!({"args": ["smcPlaneShape"], "listChannelNames": true})), json!(["vertex"]));
    assert_eq!(run(&mut s, "getMetadata", json!({"args": ["smcPlaneShape"], "streamName": "idStream", "listMemberNames": true})), json!(["ID"]));
    assert_eq!(run(&mut s, "getMetadata", json!({"args": ["smcPlaneShape"], "streamName": "idStream", "memberName": "ID", "dataType": true})), json!("int32"));
    assert!(s.run("editMetadata", json!({"args": ["smcPlaneShape"], "streamName": "idStream", "memberName": "nope", "value": 1, "index": "1"})).is_err());
    // On the scene.
    run(&mut s, "addMetadata", json!({"structure": "idStructure", "streamName": "sc", "channelName": "general", "scene": true}));
    run(&mut s, "editMetadata", json!({"streamName": "sc", "memberName": "ID", "value": 3, "index": "0", "scene": true}));
    assert_eq!(run(&mut s, "getMetadata", json!({"streamName": "sc", "memberName": "ID", "scene": true})), json!([3]));
}

#[test]
fn node_cast_moves_the_target_onto_the_source() {
    let mut s = Session::new();
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "driver"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "old"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "fresh"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "kid", "parent": "old"}));
    run(&mut s, "connectAttr", json!({"args": ["driver.tx", "old.ty"]}));
    run(&mut s, "setAttr", json!({"args": ["old.sx", 3.0]}));
    assert_eq!(run(&mut s, "nodeCast", json!({"args": ["fresh", "old"]})), json!(0));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["fresh.ty"]})), json!(["driver"]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["fresh.sx"]})), json!(3.0));
    let kids = run(&mut s, "listRelatives", json!({"args": ["fresh"], "children": true}));
    assert_eq!(kids, json!(["kid"]));
    assert!(s.undo());
    let kids = run(&mut s, "listRelatives", json!({"args": ["old"], "children": true}));
    assert_eq!(kids, json!(["kid"]));
    run(&mut s, "nodeCast", json!({"args": ["fresh", "old"], "swapNames": true}));
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["old.ty"]})), json!(["driver"]));
    // Not a kind of the target: refused, and nothing changes.
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "md"}));
    assert!(s.run("nodeCast", json!({"args": ["md", "old"]})).is_err());
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["old.ty"]})), json!(["driver"]));
}

#[test]
fn node_presets_round_trip() {
    let dir = scratch("presets");
    std::env::set_var("AZTEK_CONFIG_DIR", &dir);
    let mut s = Session::new();
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "a"}));
    run(&mut s, "createNode", json!({"args": ["multiplyDivide"], "name": "b"}));
    run(&mut s, "setAttr", json!({"args": ["a.operation", 2]}));
    assert_eq!(run(&mut s, "nodePreset", json!({"isValidName": "good_1"})), json!(true));
    assert_eq!(run(&mut s, "nodePreset", json!({"isValidName": "bad name"})), json!(false));
    run(&mut s, "nodePreset", json!({"save": ["a", "half"]}));
    assert_eq!(run(&mut s, "nodePreset", json!({"exists": ["b", "half"]})), json!(true));
    assert!(run(&mut s, "nodePreset", json!({"list": "b"})).as_array().unwrap().iter().any(|x| x == "half"));
    run(&mut s, "nodePreset", json!({"load": ["b", "half"]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["b.operation"]})), json!(2));
    run(&mut s, "nodePreset", json!({"delete": ["b", "half"]}));
    assert_eq!(run(&mut s, "nodePreset", json!({"exists": ["b", "half"]})), json!(false));
}

#[test]
fn files() {
    let dir = scratch("files");
    let d = dir.to_string_lossy().to_string();
    let mut s = Session::new();
    assert_eq!(run(&mut s, "sysFile", json!({"args": [format!("{d}/x/y")], "makeDir": true})), json!(true));
    std::fs::write(dir.join("one.txt"), "1").unwrap();
    assert_eq!(run(&mut s, "sysFile", json!({"args": [format!("{d}/one.txt")], "copy": format!("{d}/two.txt")})), json!(true));
    assert_eq!(run(&mut s, "sysFile", json!({"args": [format!("{d}/two.txt")], "rename": format!("{d}/three.mel")})), json!(true));
    assert_eq!(run(&mut s, "getFileList", json!({"folder": d, "filespec": "*.txt"})), json!(["one.txt"]));
    assert_eq!(run(&mut s, "getFileList", json!({"folder": d})), json!(["one.txt", "three.mel", "x"]));
    assert_eq!(run(&mut s, "sysFile", json!({"args": [format!("{d}/one.txt")], "delete": true})), json!(true));
    assert_eq!(run(&mut s, "sysFile", json!({"args": [d.clone()], "removeEmptyDir": true})), json!(false), "not empty");
    assert_eq!(run(&mut s, "sysFile", json!({"args": [format!("{d}/x/y")], "removeEmptyDir": true})), json!(true));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn string_array_intersector() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "stringArrayIntersector", json!({"args": ["si"]})), json!("si"));
    assert_eq!(run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "exists": true})), json!(true));
    run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "edit": true, "intersect": ["a", "b", "c", "c"]}));
    assert_eq!(run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "query": true})), json!(["a", "b", "c"]));
    run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "edit": true, "intersect": ["c", "b", "z"]}));
    assert_eq!(run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "query": true})), json!(["b", "c"]));
    run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "edit": true, "reset": true}));
    assert_eq!(run(&mut s, "stringArrayIntersector", json!({"args": ["si"], "query": true})), json!([]));
    run(&mut s, "stringArrayIntersector", json!({"args": ["dup"], "allowDuplicates": true, "intersect": ["a", "a", "b"]}));
    run(&mut s, "stringArrayIntersector", json!({"args": ["dup"], "edit": true, "intersect": ["a", "a"]}));
    assert_eq!(run(&mut s, "stringArrayIntersector", json!({"args": ["dup"], "query": true})), json!(["a", "a"]));
}

#[test]
fn held_settings_read_back() {
    let mut s = Session::new();
    run(&mut s, "displayAffected", json!({"args": [true]}));
    assert_eq!(run(&mut s, "displayAffected", json!({"query": true})), json!(true));
    run(&mut s, "setAttrMapping", json!({"device": "spaceball", "offset": 2.0}));
    assert_eq!(run(&mut s, "setAttrMapping", json!({"query": true, "queried": ["offset"]})), json!(2.0));
    run(&mut s, "makePaintable", json!({"args": ["cluster", "weights"], "attrType": "multiFloat", "uiName": "W"}));
    assert_eq!(run(&mut s, "makePaintable", json!({"query": true})), json!(["cluster", "weights", "multiFloat", "W"]));
    run(&mut s, "makePaintable", json!({"args": ["cluster", "weights"], "remove": true}));
    assert_eq!(run(&mut s, "makePaintable", json!({"query": true})), json!([]));
    run(&mut s, "polyCube", json!({"name": "c"}));
    run(&mut s, "uvLink", json!({"uvSet": "cShape.uvSet[0].uvSetName", "texture": "file1"}));
    assert_eq!(run(&mut s, "uvLink", json!({"query": true, "uvSet": "cShape.uvSet[0].uvSetName"})), json!(["file1"]));
    assert_eq!(run(&mut s, "uvLink", json!({"query": true, "texture": "file1", "queryObject": "cShape"})), json!(["cShape.uvSet[0].uvSetName"]));
    assert_eq!(run(&mut s, "uvLink", json!({"isValid": true, "uvSet": "cShape.uvSet[0].uvSetName"})), json!(true));
    assert_eq!(run(&mut s, "uvLink", json!({"isValid": true, "texture": "file1"})), json!(false));
    run(&mut s, "uvLink", json!({"b": true, "uvSet": "cShape.uvSet[0].uvSetName", "texture": "file1"}));
    assert_eq!(run(&mut s, "uvLink", json!({"query": true, "texture": "file1"})), json!([]));
}
