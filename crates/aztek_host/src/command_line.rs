//! The reference's commandLine control: one line to type a command into on the left, what it gave on the right. The
//! shell reports a line entered; it runs here in the control's language (-sourceType), goes into the control's history
//! (up to -numberOfHistoryLines), and its result or error shows in the output half.

use aztek_ops::{CommandDef, CommandError};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::core::{Core, LineKind};
use crate::scriptui::{control_full_path, control_kind, control_prop, model_edit};

pub(crate) const KIND: &str = "commandLine";
pub(crate) const RUN: &str = "aztekCommandLineRun";

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

fn mel_quote(t: &str) -> String {
    format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n"))
}

/// The shell's events for a commandLine: a line entered (`__cmdLineRun`, the line as the argument) becomes a call of
/// the run command.
pub(crate) fn shell_event(ev: &Value) -> Vec<(Value, Vec<Value>)> {
    let Some(path) = ev["path"].as_str().and_then(control_full_path) else { return Vec::new() };
    if control_kind(&path).as_deref() != Some(KIND) {
        return Vec::new();
    }
    let mut out = Vec::new();
    for flag in ev["fire"].as_array().into_iter().flatten().filter_map(Value::as_str) {
        if flag == "__cmdLineRun" {
            let line = ev["args"].as_array().and_then(|a| a.first()).map(text).unwrap_or_default();
            out.push((json!(format!("{RUN} {} {};", mel_quote(&path), mel_quote(&line))), Vec::new()));
        }
    }
    out
}

#[cfg(feature = "python")]
fn run_python(core: &mut Core, src: &str) -> Result<Value, String> {
    crate::python::eval_from_mel(core, src)
}

#[cfg(not(feature = "python"))]
fn run_python(_core: &mut Core, _src: &str) -> Result<Value, String> {
    Err("Python is not available in this build.".into())
}

fn plain(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Bool(b) => (*b as i32).to_string(),
        Value::Number(n) => match n.as_i64() {
            Some(i) => i.to_string(),
            None => crate::mel::fmt_float(n.as_f64().unwrap_or(0.0)),
        },
        Value::Array(a) => a.iter().map(plain).collect::<Vec<_>>().join(" "),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

/// Runs a line entered in a commandLine. Returns the control's full path.
pub(crate) fn run_line(core: &mut Core, name: &str, line: &str) -> Result<Value, String> {
    let Some(path) = control_full_path(name).filter(|p| control_kind(p).as_deref() == Some(KIND)) else { return Err(format!("{KIND}: Object '{name}' not found.")) };
    let python = control_prop(&path, "sourceType").is_some_and(|v| text(&v) == "python");
    let keep = control_prop(&path, "numberOfHistoryLines").map(|v| v.as_f64().unwrap_or(50.0) as usize).unwrap_or(50);
    // A one-line history is kept as the line itself.
    let mut hist: Vec<String> = match control_prop(&path, "__history") {
        Some(Value::Array(a)) => a.iter().map(text).collect(),
        Some(Value::String(s)) => vec![s],
        _ => Vec::new(),
    };
    if !line.trim().is_empty() && keep > 0 {
        hist.push(line.to_string());
        let n = hist.len();
        if n > keep {
            hist.drain(..n - keep);
        }
    }
    let mut output = String::new();
    if !line.trim().is_empty() {
        let before = core.log.len();
        if python {
            // Python has its result and error lines written here; MEL's interpreter writes its own.
            core.push(line.to_string(), LineKind::Echo);
            match run_python(core, line) {
                Ok(v) if !v.is_null() => core.push(format!("# Result: {} #", plain(&v)), LineKind::Result),
                Ok(_) => {}
                Err(e) => core.push(format!("# Error: {e} #"), LineKind::Error),
            }
        } else if let Err(e) = core.mel_checked(line) {
            if !core.log.get(before..).unwrap_or(&[]).iter().any(|l| l.kind == LineKind::Error) {
                core.push(format!("// Error: {e} //"), LineKind::Error);
            }
        }
        // What shows on the right: the last thing the line said, as the history has it.
        output = core.log.get(before..).unwrap_or(&[]).iter().rev().find(|l| l.kind != LineKind::Echo).map(|l| l.text.clone()).unwrap_or_default();
    }
    let mut m = Map::new();
    m.insert("__history".into(), json!(hist));
    m.insert("__output".into(), json!(output));
    m.insert("__input".into(), json!(""));
    model_edit(&path, &m)?;
    Ok(json!(path))
}

pub(crate) fn intercept(core: &mut Core, name: &str, call: &Value) -> Option<Result<Value, String>> {
    if name != RUN {
        return None;
    }
    let args: Vec<Value> = call.get("args").and_then(Value::as_array).cloned().unwrap_or_default();
    let Some(p) = args.first().map(text) else { return Some(Err(format!("{RUN}: give the command line and the line."))) };
    let line = args.get(1).map(text).unwrap_or_default();
    Some(run_line(core, &p, &line))
}

pub fn register(s: &mut Session) {
    s.register(
        CommandDef::new(RUN, "For a commandLine: runs a line entered in it, in its language, keeps it in its history and shows what it gave.", |_s, _a| {
            Err(CommandError::new(format!("{RUN}: needs the application's host.")))
        })
        .positional("commandLine line")
        .undoable(false),
    );
}
