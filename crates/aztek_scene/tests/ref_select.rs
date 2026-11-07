//! Reference-style component strings in select and ls.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

#[test]
fn select_and_ls_take_component_strings() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "select", json!({"args": ["pCube1.f[0:2]"]}));
    assert_eq!(run(&mut s, "selectMode", json!({})), "face");
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pCube1.f[0:2]"]));
    assert_eq!(run(&mut s, "ls", json!({"selection": true, "flatten": true})), json!(["pCube1.f[0]", "pCube1.f[1]", "pCube1.f[2]"]));

    run(&mut s, "select", json!({"args": ["pCube1.f[4]"], "add": true}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pCube1.f[0:2]", "pCube1.f[4]"]));
    run(&mut s, "select", json!({"args": ["pCube1.f[1]"], "deselect": true}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pCube1.f[0]", "pCube1.f[2]", "pCube1.f[4]"]));
    run(&mut s, "select", json!({"args": ["pCube1.f[0]", "pCube1.f[3]"], "toggle": true}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["pCube1.f[2:4]"]));

    // A component op works on what select picked.
    run(&mut s, "polyExtrude", json!({"thickness": 0.2}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["polyExtrudeFace1.inputComponents"]})), "f[2:4]");

    run(&mut s, "select", json!({"args": ["pCube1.vtx[0]", "pCube1.vtx[7]"]}));
    assert_eq!(run(&mut s, "selectMode", json!({})), "vertex");
    assert_eq!(run(&mut s, "ls", json!({"sl": true})), json!(["pCube1.vtx[0]", "pCube1.vtx[7]"]));
}
