//! Scripts written the way people write them for the reference, end to end: an arm rig, a stretch network, skinning, animation,
//! modelling, scene queries and MEL. Each part prints "ok" or "FAIL".
#![cfg(feature = "python")]

use std::ffi::{CStr, CString};

use aztek_host::*;
use serde_json::Value;

#[test]
fn ref_style_script() {
    let src = CString::new(include_str!("scripts/ref_style.py")).unwrap();
    aztek_python(src.as_ptr());
    let p = aztek_take_log();
    let text = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let log: Value = serde_json::from_str(&text).unwrap();
    let lines: Vec<&str> = log.as_array().unwrap().iter().filter_map(|l| l["text"].as_str()).collect();
    let results: Vec<&&str> = lines.iter().filter(|l| l.starts_with("ok  ") || l.starts_with("FAIL ")).collect();
    assert_eq!(results.len(), 7, "{lines:#?}");
    let failed: Vec<&&&str> = results.iter().filter(|l| l.starts_with("FAIL")).collect();
    assert!(failed.is_empty(), "{failed:#?}");
}
