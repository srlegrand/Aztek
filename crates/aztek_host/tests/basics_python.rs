//! The basics suite through `maya.cmds`, where Python's return types differ from MEL's. See basics.rs.
#![cfg(feature = "python")]

use std::ffi::{CStr, CString};
use std::sync::Mutex;

use aztek_host::*;
use serde_json::Value;

static ONE_AT_A_TIME: Mutex<()> = Mutex::new(());

/// Runs Python in a fresh scene and gives what it printed, failing on any error line.
fn run(src: &str) -> Vec<String> {
    aztek_init();
    let code = format!("import maya.cmds as cmds\ncmds.file(new=True, force=True)\n{src}");
    let c = CString::new(code).unwrap();
    take();
    aztek_python(c.as_ptr());
    let lines = take();
    let bad: Vec<&(i64, String)> = lines.iter().filter(|(k, _)| *k == 2).collect();
    assert!(bad.is_empty(), "{src}: {bad:?}");
    lines.into_iter().filter(|(k, _)| *k == 3).map(|(_, t)| t).collect()
}

fn take() -> Vec<(i64, String)> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap().to_string())).collect()
}

#[test]
fn creation_returns_lists_and_queries_return_values() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let out = run("print(cmds.polyCube())\ncmds.move(1, 2, 3)\nprint(cmds.getAttr('pCube1.translate'))\nprint(cmds.listRelatives('pCube1', p=True))\nprint(cmds.ls(sl=True))");
    assert_eq!(out, ["['pCube1', 'polyCube1']", "[(1.0, 2.0, 3.0)]", "None", "['pCube1']"]);
}

#[test]
#[ignore = "PLAN 1.11: polyEvaluate with several flags gives a dict"]
fn poly_evaluate_dict() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let out = run("cmds.polyCube()\nr = cmds.polyEvaluate('pCube1', v=True, f=True)\nprint(sorted(r.items()))\nprint(cmds.polyEvaluate('pCube1', e=True))");
    assert_eq!(out, ["[('face', 6), ('vertex', 8)]", "12"]);
}

#[test]
fn units_through_python() {
    let _one = ONE_AT_A_TIME.lock().unwrap_or_else(|e| e.into_inner());
    let out = run("cmds.polyCube()\ncmds.move(100, 0, 0)\ncmds.rotate(0, 90, 0)\ncmds.currentUnit(l='m', a='rad')\nprint(round(cmds.getAttr('pCube1.tx'), 4))\nprint(round(cmds.getAttr('pCube1.ry'), 4))\ncmds.currentUnit(l='cm', a='deg')");
    assert_eq!(out, ["1.0", "1.5708"]);
}
