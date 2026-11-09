//! Compares Aztek with the reference, call by call, from `tools/reference_capture/calls.json`.
//!
//! Each call's expected result comes from a captured fixture in `tests/data/reference/calls/<area>.json` when there is
//! one, and otherwise from the hand-written `expect`. A call with neither is reported as skipped, never as a pass.
//! Mismatches still open are listed with their PLAN.md item in `tests/data/reference_open.txt`: a new mismatch fails,
//! and so does a listed call that now matches, so the list can only shrink.
#![cfg(feature = "python")]

use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{CStr, CString};
use std::path::PathBuf;

use aztek_host::*;
use serde_json::{json, Value};

const RESET: &str = "currentUnit -l cm -a deg -t film;";

fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_json(p: &PathBuf) -> Option<Value> {
    Some(serde_json::from_str(&std::fs::read_to_string(p).ok()?).unwrap_or_else(|e| panic!("{}: {e}", p.display())))
}

/// A number, however it's typed: MEL's booleans are 0 and 1.
fn number(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::Bool(b) => Some(*b as i32 as f64),
        _ => None,
    }
}

fn same(got: &Value, want: &Value) -> bool {
    if let (Some(a), Some(b)) = (number(got), number(want)) {
        return (a - b).abs() <= 1e-4 * b.abs().max(1.0);
    }
    match (got, want) {
        // An empty answer: MEL gives an empty array where Python gives None.
        (Value::Null, Value::Array(a)) | (Value::Array(a), Value::Null) => a.is_empty(),
        (Value::Array(a), Value::Array(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| same(x, y)),
        (Value::Object(a), Value::Object(b)) => a.len() == b.len() && a.iter().all(|(k, x)| b.get(k).is_some_and(|y| same(x, y))),
        _ => got == want,
    }
}

fn take_log() -> Vec<(i64, String)> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap().to_string())).collect()
}

fn run_mel(setup: &str, call: &str) -> Value {
    let mut c = Core::new();
    aztek_scene::cameras::make_defaults(&mut c.s.graph);
    let _ = c.mel_checked(RESET);
    let _ = c.mel_checked(setup);
    let _ = c.take_log();
    match c.mel_checked(call) {
        Ok(v) => v,
        Err(e) => json!({"error": e}),
    }
}

/// A Python call runs in the embedded interpreter, on the one global host.
fn run_python(setup: &str, call: &str) -> Value {
    aztek_init();
    const TAG: &str = "@@result:";
    let code = format!(
        "import json, maya.cmds as cmds, maya.mel as mel\ncmds.file(new=True, force=True)\nmel.eval({reset:?})\nmel.eval({setup:?})\n\
         try:\n    _r = {call}\nexcept Exception as _e:\n    _r = {{'error': str(_e)}}\nprint({TAG:?} + json.dumps(_r))\n",
        reset = RESET,
    );
    take_log();
    let c = CString::new(code).unwrap();
    aztek_python(c.as_ptr());
    for (_, line) in take_log() {
        if let Some(rest) = line.strip_prefix(TAG) {
            return serde_json::from_str(rest.trim()).unwrap();
        }
    }
    json!({"error": "the script stopped before its result"})
}

#[derive(Default)]
struct Report {
    captured: usize,
    hand_written: usize,
    skipped: Vec<String>,
    mismatched: BTreeMap<String, Vec<String>>,
    ids: BTreeSet<String>,
}

fn run_calls() -> Report {
    let list = read_json(&repo().join("tools/reference_capture/calls.json")).expect("calls.json");
    let fixtures = repo().join("tests/data/reference/calls");
    let mut report = Report::default();
    for c in list["calls"].as_array().unwrap() {
        let (id, area) = (c["id"].as_str().unwrap(), c["area"].as_str().unwrap());
        let captured = read_json(&fixtures.join(format!("{area}.json"))).and_then(|f| f.get(id).map(|e| e["result"].clone()));
        let (want, source) = match captured {
            Some(w) => (w, "captured"),
            None if c["expect"].is_null() => {
                report.skipped.push(id.to_string());
                continue;
            }
            None => (c["expect"].clone(), "hand-written"),
        };
        if source == "captured" {
            report.captured += 1;
        } else {
            report.hand_written += 1;
        }
        let (setup, call) = (c["setup"].as_str().unwrap_or(""), c["call"].as_str().unwrap());
        let got = if c["lang"] == "python" { run_python(setup, call) } else { run_mel(setup, call) };
        if !same(&got, &want) {
            report.ids.insert(id.to_string());
            report.mismatched.entry(area.to_string()).or_default().push(format!("{id}: {call} gave {got}, the reference {want} ({source})"));
        }
    }
    report
}

fn open_list() -> BTreeSet<String> {
    let text = std::fs::read_to_string(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/reference_open.txt")).unwrap();
    text.lines().filter_map(|l| l.split('#').next().map(str::trim)).filter(|l| !l.is_empty()).map(str::to_string).collect()
}

#[test]
fn calls_match_the_reference() {
    let report = run_calls();
    eprintln!(
        "reference fixtures: {} against captures, {} against hand-written readings, {} skipped with nothing to compare",
        report.captured,
        report.hand_written,
        report.skipped.len()
    );
    if report.captured == 0 {
        eprintln!("reference fixtures: none captured yet, see tools/reference_capture/WANTED.md");
    }
    if !report.skipped.is_empty() {
        eprintln!("skipped: {}", report.skipped.join(", "));
    }
    for (area, lines) in &report.mismatched {
        eprintln!("[{area}]");
        for l in lines {
            eprintln!("  {l}");
        }
    }
    let open = open_list();
    let new: Vec<&String> = report.ids.difference(&open).collect();
    let fixed: Vec<&String> = open.difference(&report.ids).collect();
    assert!(new.is_empty(), "new mismatches: {new:?}");
    assert!(fixed.is_empty(), "these match now, take them off tests/data/reference_open.txt: {fixed:?}");
}

#[test]
fn comparison_rules() {
    assert!(same(&json!(1), &json!(true)));
    assert!(same(&json!(1.00001), &json!(1.0)));
    assert!(!same(&json!(1.1), &json!(1.0)));
    assert!(same(&json!(null), &json!([])));
    assert!(!same(&json!(["a"]), &json!(null)));
    assert!(!same(&json!("a"), &json!(["a"])));
    assert!(!same(&json!({"error": "x"}), &json!(1)));
}
