//! No dead rows: every option box row is a flag its command takes, and its default is one of its own choices.

use aztek_host::option_boxes::{rows, COMMANDS};
use aztek_host::Core;
use serde_json::{json, Value};

#[test]
fn every_row_is_a_flag_of_its_command() {
    let c = Core::new();
    let mut bad = Vec::new();
    for cmd in COMMANDS {
        let Some(def) = c.s.commands.find(cmd) else {
            bad.push(format!("{cmd}: no such command"));
            continue;
        };
        let table = rows(cmd);
        assert!(!table.is_empty(), "{cmd} has no rows");
        for r in table {
            let flag = r["flag"].as_str().unwrap();
            if !def.flags.iter().any(|f| f.name == flag) {
                bad.push(format!("{cmd}: {flag}"));
            }
            if let Some(ch) = r.get("choices").and_then(Value::as_array) {
                assert!(ch.iter().any(|p| p[1] == r["default"]), "{cmd}.{flag}: default is not a choice");
            }
        }
    }
    assert!(bad.is_empty(), "rows with no flag: {bad:#?}");
}

fn call_of(cmd: &str, args: &[&str]) -> Value {
    let mut call = json!({"args": args});
    for r in rows(cmd) {
        if r["kind"] == "axes" || r["default"] == json!("") {
            continue;
        }
        call[r["flag"].as_str().unwrap()] = r["default"].clone();
    }
    call
}

/// What the dialog sends with nothing changed runs.
#[test]
fn defaults_run() {
    let mut c = Core::new();
    c.mel("polyCube -n a; polyCube -n b; select a.f[1];");
    c.exec("polyExtrude", call_of("polyExtrude", &[])).unwrap();
    c.mel("select a.e[0];");
    c.exec("polyBevel", call_of("polyBevel", &[])).unwrap();
    for cmd in ["parentConstraint", "pointConstraint", "orientConstraint", "scaleConstraint", "aimConstraint"] {
        c.mel("select a b;");
        c.exec(cmd, call_of(cmd, &[])).unwrap_or_else(|e| panic!("{cmd}: {e}"));
    }
    c.mel("select -cl; joint -p 0 0 0 -n j1; joint -p 0 2 0 -n j2; select j1 b;");
    c.exec("skinCluster", call_of("skinCluster", &[])).unwrap();
    c.mel("polyCube -n d; select d;");
    c.exec("polyMirrorFace", call_of("polyMirrorFace", &[])).unwrap();
    c.mel("polyCube -n e; select e;");
    c.exec("lattice", call_of("lattice", &[])).unwrap();
}

#[test]
fn extrude_options_reach_the_node() {
    let mut c = Core::new();
    c.mel("polyCube; select pCube1.f[1];");
    let mut call = call_of("polyExtrude", &[]);
    call["divisions"] = json!(3);
    call["thickness"] = json!(0.5);
    c.exec("polyExtrude", call).unwrap();
    assert_eq!(c.mel("getAttr polyExtrudeFace1.divisions;"), json!(3));
    assert_eq!(c.mel("getAttr polyExtrudeFace1.thickness;"), json!(0.5));
}

#[test]
fn bevel_defaults_to_a_fraction_and_takes_a_width() {
    let mut c = Core::new();
    c.mel("polyCube; select pCube1.e[0];");
    let call = call_of("polyBevel", &[]);
    let r = c.exec("polyBevel", call).unwrap();
    let node = r[0].as_str().unwrap().to_string();
    assert_eq!(c.mel(&format!("getAttr {node}.fraction;")), json!(0.5));
    assert_eq!(c.mel(&format!("getAttr {node}.offsetAsFraction;")), json!(true));
    c.mel(&format!("setAttr {node}.fraction 0.25;"));
    assert_eq!(c.mel(&format!("getAttr {node}.fraction;")), json!(0.25));
    // Bare command: the reference's default again.
    c.mel("polyCube -n b; select b.e[0];");
    let r = c.exec("polyBevel", json!({})).unwrap();
    assert_eq!(c.mel(&format!("getAttr {}.offsetAsFraction;", r[0].as_str().unwrap())), json!(true));
}
