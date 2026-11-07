//! Scripting commands: polyEvaluate, listConnections, objectType, polyListComponentConversion, filterExpand,
//! polyCreateFacet, polyAverageVertex.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn scripting_commands() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["pCube1"], "vertex": true})), json!(8));
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["pCube1"], "face": true})), json!(6));
    assert_eq!(run(&mut s, "polyEvaluate", json!({"args": ["pCube1.f[0:2]"], "face": true})), json!(3));
    assert_eq!(run(&mut s, "objectType", json!({"args": ["pCubeShape1"]})), json!("mesh"));
    let c = run(&mut s, "listConnections", json!({"args": ["pCubeShape1"], "source": true, "destination": false}));
    assert_eq!(c, json!(["polyCube1"]));
    let v = run(&mut s, "polyListComponentConversion", json!({"args": ["pCube1.f[0]"], "toVertex": true}));
    assert_eq!(v.as_array().unwrap().len(), 1);
    run(&mut s, "select", json!({"args": ["pCube1.vtx[0:3]"]}));
    let fe = run(&mut s, "filterExpand", json!({"selectionMask": 31}));
    assert_eq!(fe.as_array().unwrap().len(), 4);
    assert_eq!(run(&mut s, "filterExpand", json!({"selectionMask": 34})), Value::Null);
    let r = run(&mut s, "polyCreateFacet", json!({"point": [[0, 0, 0], [1, 0, 0], [0, 0, 1]]}));
    assert!(r[0].is_string());
    run(&mut s, "polyAverageVertex", json!({"args": ["pCube1.vtx[0]"], "iterations": 3}));
}

#[test]
fn ls_lists_as_the_reference_does() {
    let mut s = Session::new();
    s.run("polyCube", json!({})).unwrap();
    s.run("polyCube", json!({})).unwrap();
    s.run("group", json!({"args": ["pCube2"], "name": "grp"})).unwrap();
    assert_eq!(s.run("ls", json!({"args": ["pCube*"], "transforms": true})).unwrap(), json!(["pCube1", "pCube2"]));
    assert_eq!(s.run("ls", json!({"args": ["pCube*"], "shapes": true})).unwrap(), json!(["pCubeShape1", "pCubeShape2"]));
    assert_eq!(s.run("ls", json!({"args": ["pCube2"], "long": true})).unwrap(), json!(["|grp|pCube2"]));
    assert_eq!(s.run("ls", json!({"args": ["grp"], "dagObjects": true, "shapes": true})).unwrap(), json!(["pCubeShape2"]));
    assert_eq!(s.run("ls", json!({"type": "polyCube"})).unwrap(), json!(["polyCube1", "polyCube2"]));
    assert_eq!(s.run("ls", json!({"args": ["pCube1.vtx[*]"], "flatten": true})).unwrap().as_array().unwrap().len(), 8);
    assert_eq!(s.run("ls", json!({"args": ["pCube1.vtx[*]"]})).unwrap(), json!(["pCube1.vtx[0:7]"]));
    assert_eq!(s.run("ls", json!({"args": ["pCube1.f[1:2]"], "flatten": true})).unwrap(), json!(["pCube1.f[1]", "pCube1.f[2]"]));
    assert_eq!(s.run("ls", json!({"assemblies": true})).unwrap(), json!(["pCube1", "grp"]));
    assert_eq!(s.run("ls", json!({"args": ["pCube1"], "showType": true})).unwrap(), json!(["pCube1", "transform"]));
    s.run("select", json!({"args": ["pCube1", "grp"]})).unwrap();
    assert_eq!(s.run("ls", json!({"selection": true, "head": 1})).unwrap(), json!(["pCube1"]));
}
