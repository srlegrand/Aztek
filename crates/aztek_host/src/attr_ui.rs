//! Controls bound to plugs: connectControl and the attr* groups.
//!
//! A binding ties a flag of a control (its value, one of its fields, its colour) to a plug. Idle reads the plug and edits
//! the control when it changed, whatever changed it; what the user does comes back through `shell_changed` as setAttr:
//! silent while a slider drags, one echoed, undoable line when it lets go. The attr* commands pick the plain control the
//! attribute calls for and bind it, so everything else about them is that control's.

use std::cell::RefCell;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, CommandHost, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::core::Core;
use crate::scriptui::{add_column_flags, add_flags, control_full_path, control_kind, control_prop, controls_of, create_as, model_edit, take_dirty_tables, COMMON, FIELD_GRP, SLIDER_GRP};

type Cmd = CommandDef<Session>;

#[derive(Clone, Debug)]
struct Binding {
    path: String,
    slot: String,
    plug: String,
    /// An option menu's item values, when they aren't 0, 1, 2...
    values: Option<Vec<i64>>,
    /// What the control was last told, in the control's terms.
    last: Value,
    state: String,
}

#[derive(Clone, Debug)]
struct Preview {
    path: String,
    slot: String,
}

#[derive(Default)]
struct Bindings {
    all: Vec<Binding>,
    preview: Option<Preview>,
}

thread_local! {
    static BINDINGS: RefCell<Bindings> = RefCell::new(Bindings::default());
}

fn with<R>(f: impl FnOnce(&mut Bindings) -> R) -> R {
    BINDINGS.with(|b| f(&mut b.borrow_mut()))
}

fn number(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64)).or_else(|| v.as_str().and_then(|s| s.parse().ok())).unwrap_or(0.0)
}

fn flat(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        o => vec![o.clone()],
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        o => o.to_string(),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Between plugs and controls

fn read_plug(s: &mut Session, plug: &str) -> Option<Value> {
    s.run("getAttr", json!({ "args": [plug] })).ok().filter(|v| !v.is_null())
}

fn is_int_control(kind: &str) -> bool {
    matches!(kind, "intField" | "intSlider" | "intScrollBar" | "intSliderGrp" | "intFieldGrp" | "colorIndexSliderGrp")
}

fn is_switch_control(kind: &str) -> bool {
    matches!(kind, "checkBox" | "checkBoxGrp" | "symbolCheckBox" | "iconTextCheckBox")
}

/// A plug's value as the flag `slot` of a control of `kind` holds it.
fn to_control(kind: &str, slot: &str, raw: &Value, values: Option<&[i64]>) -> Value {
    match slot {
        "rgbValue" => Value::Array(flat(raw).iter().map(|v| json!(number(v))).collect()),
        "text" | "label" => json!(text(raw)),
        "select" => {
            let i = number(raw) as i64;
            match values {
                Some(list) => json!(list.iter().position(|x| *x == i).map_or(1, |p| p + 1)),
                None => json!(i + 1),
            }
        }
        _ if is_switch_control(kind) => json!(number(raw) != 0.0),
        _ if is_int_control(kind) => json!(number(raw).round() as i64),
        _ => json!(number(raw)),
    }
}

/// A control's value as what setAttr takes after the plug.
fn to_plug(slot: &str, v: &Value, values: Option<&[i64]>) -> (Vec<Value>, bool) {
    match slot {
        "rgbValue" => (flat(v).iter().map(|x| json!(number(x))).collect(), false),
        "text" => (vec![json!(text(v))], true),
        "select" => {
            let i = number(v) as i64 - 1;
            let out = match values {
                Some(list) => list.get(i.max(0) as usize).copied().unwrap_or(0),
                None => i,
            };
            (vec![json!(out)], false)
        }
        _ => (vec![json!(number(v))], false),
    }
}

fn plug_state(s: &mut Session, plug: &str) -> String {
    match s.graph.plug(plug) {
        Ok(p) => crate::query::channel_state(&s.graph, &p).to_string(),
        Err(_) => String::new(),
    }
}

fn set_call(plug: &str, values: Vec<Value>, is_text: bool) -> Value {
    let mut args = vec![json!(plug)];
    args.extend(values);
    if is_text {
        json!({"args": args, "type": "string"})
    } else {
        json!({ "args": args })
    }
}

fn close_enough(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Number(_), Value::Number(_)) => (number(a) - number(b)).abs() < 1e-9,
        (Value::Array(x), Value::Array(y)) => x.len() == y.len() && x.iter().zip(y).all(|(p, q)| close_enough(p, q)),
        _ => a == b,
    }
}

/// Makes `b` show its plug now.
fn show(s: &mut Session, b: &mut Binding) {
    let Some(raw) = read_plug(s, &b.plug) else { return };
    let Some(kind) = control_kind(&b.path) else { return };
    let v = to_control(&kind, &b.slot, &raw, b.values.as_deref());
    let state = plug_state(s, &b.plug);
    let mut flags = Map::new();
    if !close_enough(&v, &b.last) {
        flags.insert(b.slot.clone(), v.clone());
        b.last = v;
    }
    if state != b.state {
        flags.insert("plugState".to_string(), json!(state));
        b.state = state;
    }
    if !flags.is_empty() {
        let _ = model_edit(&b.path, &flags);
    }
}

fn bind(s: &mut Session, path: &str, slot: &str, plug: &str, values: Option<Vec<i64>>) {
    let mut b = Binding { path: path.to_string(), slot: slot.to_string(), plug: plug.to_string(), values, last: Value::Null, state: String::new() };
    show(s, &mut b);
    with(|x| {
        x.all.retain(|o| !(o.path == path && o.slot == slot));
        x.all.push(b);
    });
}

fn unbind(path: &str) {
    with(|x| x.all.retain(|o| o.path != path));
}

/// The plug behind a control's flag, for scripts that ask.
pub fn bound_plug(path: &str, slot: &str) -> Option<String> {
    let full = control_full_path(path)?;
    with(|x| x.all.iter().find(|o| o.path == full && o.slot == slot).map(|o| o.plug.clone()))
}

// ---------------------------------------------------------------------------------------------------------------------
// What moves them

/// Idle: plugs out to controls, tool buttons to the current tool, tables to their scripts.
pub(crate) fn idle(core: &mut Core) {
    let list = with(|x| x.all.clone());
    if !list.is_empty() {
        let previewing = with(|x| x.preview.clone());
        let mut kept = Vec::with_capacity(list.len());
        for mut b in list {
            // A deleted control goes with its bindings.
            if control_full_path(&b.path).is_none() {
                continue;
            }
            if previewing.as_ref().is_none_or(|p| p.path != b.path || p.slot != b.slot) {
                show(&mut core.s, &mut b);
            }
            kept.push(b);
        }
        with(|x| x.all = kept);
    }
    sync_tool_buttons();
    fill_tables(core);
    crate::ctx_cmds::idle(core);
}

fn sync_tool_buttons() {
    let cur = crate::ctx_cmds::current();
    for (path, props) in controls_of("toolButton") {
        let on = props.get("tool").map(flat).is_some_and(|t| t.iter().any(|x| text(x) == cur));
        if props.get("select").and_then(Value::as_bool).unwrap_or(false) != on {
            let _ = model_edit(&path, &Map::from_iter([("select".to_string(), json!(on))]));
        }
    }
}

fn mel_argument(a: &Value) -> String {
    match a {
        Value::String(t) => format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\"")),
        // Strings and numbers in braces, as MEL writes an array.
        Value::Array(l) => format!("{{{}}}", l.iter().map(mel_argument).collect::<Vec<_>>().join(",")),
        o => o.to_string(),
    }
}

/// A drag begins on a control: -dragCallback answers with the messages it carries. `{path, x, y, modifiers}`.
pub(crate) fn drag_start(core: &mut Core, arg: &str) -> Value {
    let ev: Value = serde_json::from_str(arg).unwrap_or(Value::Null);
    let path = ev["path"].as_str().unwrap_or("");
    let Some(cb) = control_prop(path, "dragCallback") else { return json!([]) };
    let v = call_for_value(core, &cb, &[json!(path), ev["x"].clone(), ev["y"].clone(), ev["modifiers"].clone()]).unwrap_or(Value::Null);
    match v {
        Value::Null => json!([]),
        o => Value::Array(flat(&o).iter().map(|x| json!(text(x))).collect()),
    }
}

pub fn drag_start_for_tests(core: &mut Core, arg: &str) -> Value {
    drag_start(core, arg)
}

/// A drop on a control: -dropCallback gets where it came from and its messages, as one undo step.
fn drop_on(core: &mut Core, path: &str, ev: &Value) {
    let Some(cb) = control_prop(path, "dropCallback") else { return };
    core.s.open_undo_chunk("drop");
    let r = call_for_value(core, &cb, &[ev["dropFrom"].clone(), json!(path), ev["messages"].clone(), ev["x"].clone(), ev["y"].clone(), ev["dragType"].clone()]);
    core.s.close_undo_chunk();
    if let Err(m) = r {
        core.push(format!("// Error: {m} //"), crate::core::LineKind::Error);
    }
    core.version += 1;
}

/// A script's own value: a string is the name of a MEL procedure, given `args` after it.
pub(crate) fn call_for_value(core: &mut Core, cb: &Value, args: &[Value]) -> Result<Value, String> {
    match cb {
        Value::String(src) => {
            let quoted: Vec<String> = args
                .iter()
                .map(mel_argument)
                .collect();
            crate::mel::call_for_value(core, &format!("{src} {};", quoted.join(" ")))
        }
        Value::Object(o) => {
            if let Some(id) = o.get("__pycb__").and_then(Value::as_i64) {
                crate::core::python_value(core, id, args)
            } else if let Some(src) = o.get("__pysrc__").and_then(Value::as_str) {
                let list: Vec<String> = args.iter().map(Value::to_string).collect();
                crate::core::python_from_mel(core, &format!("{src}({})", list.join(",")))
            } else {
                Ok(Value::Null)
            }
        }
        _ => Ok(Value::Null),
    }
}

fn fill_tables(core: &mut Core) {
    for (path, cb, rows, cols) in take_dirty_tables() {
        // The reference asks for the cells it draws; this asks for all of them, up to a sane number.
        let rows = rows.min(2000);
        let cols = cols.min(200);
        let mut cells = Vec::with_capacity(rows);
        for r in 1..=rows {
            let mut row = Vec::with_capacity(cols);
            for c in 1..=cols {
                let v = call_for_value(core, &cb, &[json!(r), json!(c)]).unwrap_or(Value::Null);
                row.push(text(&v));
            }
            cells.push(row);
        }
        crate::scriptui::table_set_cells(&path, cells);
    }
}

/// The user changed a cell: -cellChangedCmd may turn it down, and the cell shows what stands.
fn cell_edit(core: &mut Core, path: &str, edit: &Value) {
    let vals = flat(edit);
    let [r, c, v, ..] = vals.as_slice() else { return };
    let (r, c, v) = (number(r) as i64, number(c) as i64, text(v));
    let ok = match control_prop(path, "cellChangedCmd") {
        Some(cb) => {
            let answer = call_for_value(core, &cb, &[json!(r), json!(c), json!(v)]).unwrap_or(Value::Null);
            number(&answer) != 0.0 || answer.as_bool() == Some(true)
        }
        None => true,
    };
    let mut cells: Vec<Vec<String>> = control_prop(path, "cells").and_then(|x| serde_json::from_value(x).ok()).unwrap_or_default();
    if ok {
        if let Some(cell) = cells.get_mut((r - 1).max(0) as usize).and_then(|row| row.get_mut((c - 1).max(0) as usize)) {
            *cell = v.clone();
        }
    }
    // Sent either way: a refused edit has to go back in the shell's cell.
    crate::scriptui::table_set_cells(path, cells);
    if ok {
        if let Some(cb) = control_prop(path, "afterCellChangedCmd") {
            let _ = call_for_value(core, &cb, &[json!(r), json!(c), json!(v)]);
        }
    }
}

/// What the user did in a control: bound flags go to their plugs.
pub(crate) fn shell_changed(core: &mut Core, ev: &Value) {
    let path = ev["path"].as_str().unwrap_or("").to_string();
    if let Some(e) = ev.get("cellEdit") {
        cell_edit(core, &path, e);
    }
    if ev.get("dropFrom").is_some() {
        drop_on(core, &path, ev);
    }
    let fired: Vec<&str> = ev["fire"].as_array().map(|f| f.iter().filter_map(Value::as_str).collect()).unwrap_or_default();
    let dragging = fired.contains(&"dragCommand");
    let set = ev["set"].as_object().cloned().unwrap_or_default();
    let mut slots: Vec<String> = set.keys().cloned().collect();
    // A letting go that reports no value commits the drag's.
    let previewed = with(|x| x.preview.clone());
    if !dragging {
        if let Some(p) = previewed.as_ref().filter(|p| p.path == path && !slots.contains(&p.slot)) {
            slots.push(p.slot.clone());
        }
    }
    for slot in slots {
        let Some(b) = with(|x| x.all.iter().find(|o| o.path == path && o.slot == slot).cloned()) else { continue };
        let v = control_prop(&path, &slot).unwrap_or(Value::Null);
        let (values, is_text) = to_plug(&slot, &v, b.values.as_deref());
        let call = set_call(&b.plug, values, is_text);
        let was_preview = with(|x| {
            let same = x.preview.as_ref().is_some_and(|p| p.path == path && p.slot == slot);
            if same {
                x.preview = None;
            }
            same
        });
        if was_preview {
            core.retract_preview();
        }
        let ok = if dragging {
            let done = core.exec_silent("setAttr", call).is_ok();
            if done {
                with(|x| x.preview = Some(Preview { path: path.clone(), slot: slot.clone() }));
            }
            done
        } else if was_preview || !close_enough(&v, &b.last) {
            core.exec("setAttr", call).is_ok()
        } else {
            true
        };
        with(|x| {
            if let Some(o) = x.all.iter_mut().find(|o| o.path == path && o.slot == slot) {
                // Refused: the next idle puts back what the plug holds.
                o.last = if ok { v.clone() } else { Value::Null };
            }
        });
    }
}

/// After a script's -e, or a control made: a bound value given by a script is set on the plug too.
fn write_edits(s: &mut Session, name: &str) {
    let Some(path) = control_full_path(name) else { return };
    let list: Vec<Binding> = with(|x| x.all.iter().filter(|o| o.path == path).cloned().collect());
    for b in list {
        let Some(v) = control_prop(&path, &b.slot) else { continue };
        if close_enough(&v, &b.last) {
            continue;
        }
        let (values, is_text) = to_plug(&b.slot, &v, b.values.as_deref());
        if s.run("setAttr", set_call(&b.plug, values, is_text)).is_ok() {
            with(|x| {
                if let Some(o) = x.all.iter_mut().find(|o| o.path == path && o.slot == b.slot) {
                    o.last = v.clone();
                }
            });
        }
    }
}

/// Answers a query the model can't: a range control follows the playback range until a script gives it its own.
pub(crate) fn live_query(s: &mut Session, cmd: &str, a: &Args) -> Option<Value> {
    if !a.bool("query") {
        return None;
    }
    let asked = a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| a.flags.keys().find(|k| !matches!(k.as_str(), "query" | "queried")).cloned())?;
    if cmd == "animCurveEditor" {
        return match asked.as_str() {
            // The curves of the selection, which is what the editor shows.
            "curvesShown" => Some(s.run("keyframe", json!({"query": true, "name": true})).unwrap_or(json!([]))),
            "areCurvesSelected" => Some(json!(false)),
            _ => None,
        };
    }
    if !matches!(cmd, "rangeControl" | "timeRangeControl") {
        return None;
    }
    let name = a.pos_str(0)?;
    let flag = match asked.as_str() {
        "minRange" => "minTime",
        "maxRange" => "maxTime",
        _ => return None,
    };
    if control_prop(name, &asked).is_some() {
        return None;
    }
    s.run("playbackOptions", json!({"query": true, "queried": [flag]})).ok()
}

/// What a command does past the model.
pub(crate) fn after_command(s: &mut Session, cmd: &str, a: &Args, made: &CmdResult) {
    let name = a.pos_str(0).map(str::to_string).or_else(|| made.as_ref().ok().and_then(Value::as_str).map(str::to_string));
    if matches!(cmd, "rangeControl" | "timeRangeControl") && (a.has("minRange") || a.has("maxRange")) {
        let mut call = Map::new();
        if a.has("minRange") {
            call.insert("minTime".to_string(), json!(a.float("minRange", 1.0)));
        }
        if a.has("maxRange") {
            call.insert("maxTime".to_string(), json!(a.float("maxRange", 120.0)));
        }
        let _ = s.run("playbackOptions", Value::Object(call));
    }
    if a.bool("edit") && cmd == "workspaceControl" && (a.has("dockToMainWindow") || a.has("dockToControl") || a.has("dockToPanel") || a.has("tabToControl")) {
        if let Some(n) = &name {
            let _ = model_edit(n, &Map::from_iter([("floating".to_string(), json!(false))]));
        }
    }
    if cmd == "toolButton" && a.has("select") && a.bool("select") {
        if let Some(t) = name.as_deref().and_then(|n| control_prop(n, "tool")).map(|t| flat(&t)).and_then(|t| t.first().map(text)) {
            crate::scriptui::run_soon(json!(format!("setToolTo \"{t}\";")), Vec::new());
        }
    }
    if a.bool("edit") {
        if let Some(n) = name {
            write_edits(s, &n);
        }
    }
}

/// clipEditor: there is no Time Editor here. The nearest window is the Dope Sheet.
fn clip_editor(a: &Args) -> CmdResult {
    if a.bool("exists") {
        return Ok(json!(false));
    }
    if a.bool("query") {
        return Ok(Value::Null);
    }
    crate::script_jobs::warn_once("clipEditor", "clipEditor: clips are not available here; opening the Dope Sheet, the nearest window.");
    crate::core::push_ui_request("DopeSheetEditor".to_string());
    Ok(json!(a.pos_str(0).unwrap_or("clipEditor1")))
}

// ---------------------------------------------------------------------------------------------------------------------
// connectControl

fn split_plug(plug: &str) -> Option<(&str, &str)> {
    plug.split_once('.')
}

fn attr_children(s: &mut Session, plug: &str) -> Vec<String> {
    let Some((node, attr)) = split_plug(plug) else { return Vec::new() };
    let leaf = attr.rsplit('.').next().unwrap_or(attr).split('[').next().unwrap_or(attr);
    let v = s.run("attributeQuery", json!({"args": [leaf], "node": node, "listChildren": true})).unwrap_or(Value::Null);
    flat(&v).iter().filter_map(Value::as_str).map(|k| format!("{node}.{k}")).collect()
}

fn nice_name(s: &mut Session, plug: &str) -> String {
    let leaf = plug.rsplit('.').next().unwrap_or(plug).split('[').next().unwrap_or(plug).to_string();
    let Some((node, _)) = split_plug(plug) else { return leaf };
    match s.run("attributeQuery", json!({"args": [&leaf], "node": node, "niceName": true})) {
        Ok(Value::String(t)) if t != leaf => t,
        _ => nice_from_long(&leaf),
    }
}

/// `translateX` to `Translate X`, as the reference spells a long name for people.
fn nice_from_long(name: &str) -> String {
    let mut out = String::new();
    for (i, ch) in name.chars().enumerate() {
        if i == 0 {
            out.extend(ch.to_uppercase());
        } else {
            if ch.is_uppercase() {
                out.push(' ');
            }
            out.push(ch);
        }
    }
    out
}

/// The flag of a control that shows a plug: by -index for a group (1 being its label), else the first.
fn slot_for(kind: &str, index: Option<i64>, nth: usize) -> Option<String> {
    let field_grp = matches!(kind, "floatFieldGrp" | "intFieldGrp" | "checkBoxGrp");
    if let Some(i) = index {
        if i == 1 && kind.ends_with("Grp") {
            return Some("label".to_string());
        }
        if field_grp {
            return Some(format!("value{}", i - 1));
        }
    }
    Some(match kind {
        k if field_grp => {
            let _ = k;
            format!("value{}", nth + 1)
        }
        "colorSliderGrp" | "canvas" => "rgbValue".to_string(),
        "optionMenu" | "optionMenuGrp" | "radioButtonGrp" | "radioCollection" => "select".to_string(),
        "textField" | "textFieldGrp" | "textFieldButtonGrp" | "scrollField" => "text".to_string(),
        "text" => "label".to_string(),
        "floatField" | "intField" | "floatSlider" | "intSlider" | "floatScrollBar" | "intScrollBar" | "floatSliderGrp" | "intSliderGrp" | "floatSliderButtonGrp" | "checkBox" | "symbolCheckBox" | "iconTextCheckBox" | "colorIndexSliderGrp" => "value".to_string(),
        _ => return None,
    })
}

fn connect_control(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let (Some(control), true) = (names.first(), names.len() >= 2) else { fail!("connectControl: give a control and an attribute.") };
    let Some(path) = control_full_path(control) else { fail!("connectControl: Object '{control}' not found.") };
    let kind = control_kind(&path).unwrap_or_default();
    let index = a.has("index").then(|| a.int("index", 1));
    let mut plugs: Vec<String> = names[1..].to_vec();
    // One compound on a run of fields: its children, one to a field.
    if index.is_none() && plugs.len() == 1 && matches!(kind.as_str(), "floatFieldGrp" | "intFieldGrp" | "checkBoxGrp") {
        let kids = attr_children(s, &plugs[0]);
        if !kids.is_empty() {
            plugs = kids;
        }
    }
    for (n, plug) in plugs.iter().enumerate() {
        if read_plug(s, plug).is_none() {
            fail!("connectControl: No object matches name: {plug}");
        }
        let Some(slot) = slot_for(&kind, index, n) else { fail!("connectControl: a {kind} cannot show an attribute.") };
        if slot == "label" {
            let label = nice_name(s, plug);
            model_edit(&path, &Map::from_iter([("label".to_string(), json!(label))])).map_err(CommandError::new)?;
            continue;
        }
        bind(s, &path, &slot, plug, None);
    }
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// The attr* controls

struct Plan {
    kind: &'static str,
    flags: Map<String, Value>,
    binds: Vec<(String, String)>,
    /// (value, label) of an option menu's items.
    items: Vec<(i64, String)>,
}

fn enum_items(s: &mut Session, node: &str, leaf: &str) -> Vec<(i64, String)> {
    let v = s.run("attributeQuery", json!({"args": [leaf], "node": node, "listEnum": true})).unwrap_or(Value::Null);
    let mut out = Vec::new();
    let mut next = 0;
    for part in flat(&v).iter().map(text).collect::<Vec<_>>().join(":").split(':') {
        if part.is_empty() {
            continue;
        }
        let (label, val) = match part.split_once('=') {
            Some((l, n)) => (l, n.parse().unwrap_or(next)),
            None => (part, next),
        };
        out.push((val, label.to_string()));
        next = val + 1;
    }
    out
}

fn plan_for(s: &mut Session, cmd: &str, plug: &str) -> Result<Plan, String> {
    let Some((node, attr)) = split_plug(plug) else { return Err(format!("{cmd}: '{plug}' is not an attribute.")) };
    let leaf = attr.rsplit('.').next().unwrap_or(attr).split('[').next().unwrap_or(attr).to_string();
    let Some(value) = read_plug(s, plug) else { return Err(format!("{cmd}: No object matches name: {plug}")) };
    let ask = |s: &mut Session, flag: &str| s.run("attributeQuery", json!({"args": [&leaf], "node": node, flag: true})).ok();
    let at = ask(s, "attributeType").map(|v| text(&v)).unwrap_or_default();
    let vals = flat(&value);
    let is_color = vals.len() == 3 && ask(s, "usedAsColor").is_some_and(|v| v == json!(true));
    let is_enum = at == "enum";
    let is_bool = at == "bool" || value.is_boolean();
    let is_text = value.is_string() && !is_enum;
    let is_int = matches!(at.as_str(), "long" | "short" | "byte" | "int" | "int32" | "char");
    let mut kids = attr_children(s, plug);
    if kids.len() != vals.len() {
        kids.clear();
    }

    let mut flags = Map::new();
    let mut binds = Vec::new();
    let mut items = Vec::new();
    // The range: the slider's from the soft limits, else the hard ones, else about what the value is.
    let range = |s: &mut Session, soft: &str, hard: &str| -> Option<f64> {
        for fl in [soft, hard] {
            if let Some(v) = ask(s, fl) {
                if let Some(x) = flat(&v).first().and_then(Value::as_f64) {
                    return Some(x);
                }
            }
        }
        None
    };
    let lo = range(s, "softMin", "minimum");
    let hi = range(s, "softMax", "maximum");
    let v0 = vals.first().map(number).unwrap_or(0.0);
    let kind: &'static str = match cmd {
        "attrColorSliderGrp" => {
            if vals.len() != 3 {
                return Err(format!("{cmd}: {plug} is not a colour."));
            }
            "colorSliderGrp"
        }
        "attrEnumOptionMenuGrp" => "optionMenuGrp",
        "attrEnumOptionMenu" => "optionMenu",
        "attrFieldGrp" => {
            if is_int && vals.len() == 1 {
                "intFieldGrp"
            } else {
                "floatFieldGrp"
            }
        }
        "attrNavigationControlGrp" => "textFieldGrp",
        // attrControlGrp and attrFieldSliderGrp: what the attribute's type asks for.
        _ if is_color && cmd == "attrControlGrp" => "colorSliderGrp",
        _ if is_enum && cmd == "attrControlGrp" => "optionMenuGrp",
        _ if is_bool && cmd == "attrControlGrp" => "checkBoxGrp",
        _ if is_text => "textFieldGrp",
        _ if vals.len() > 1 => "floatFieldGrp",
        _ if is_int => "intSliderGrp",
        _ => "floatSliderGrp",
    };
    match kind {
        "colorSliderGrp" => binds.push(("rgbValue".to_string(), plug.to_string())),
        "optionMenuGrp" | "optionMenu" => {
            if is_enum {
                items = enum_items(s, node, &leaf);
            }
            binds.push(("select".to_string(), plug.to_string()));
        }
        "floatFieldGrp" | "intFieldGrp" => {
            flags.insert("numberOfFields".to_string(), json!(vals.len().clamp(1, 4)));
            if kids.is_empty() {
                binds.push(("value1".to_string(), plug.to_string()));
            } else {
                for (i, k) in kids.iter().enumerate().take(4) {
                    binds.push((format!("value{}", i + 1), k.clone()));
                }
            }
        }
        "checkBoxGrp" => {
            flags.insert("numberOfCheckBoxes".to_string(), json!(1));
            binds.push(("value1".to_string(), plug.to_string()));
        }
        "textFieldGrp" => {
            if cmd == "attrNavigationControlGrp" {
                flags.insert("editable".to_string(), json!(false));
                return Ok(Plan { kind, flags, binds, items });
            }
            binds.push(("text".to_string(), plug.to_string()));
        }
        _ => {
            flags.insert("field".to_string(), json!(true));
            let (l, h) = if v0 >= 0.0 { (0.0, (v0 * 2.0).max(1.0)) } else { (v0 * 2.0, 1.0_f64.max(-v0 * 2.0)) };
            flags.insert("minValue".to_string(), json!(lo.unwrap_or(l)));
            flags.insert("maxValue".to_string(), json!(hi.unwrap_or(h)));
            // The field goes past the slider unless the attribute's hard limits say not.
            let hard_lo = ask(s, "minimum").and_then(|v| flat(&v).first().and_then(Value::as_f64));
            let hard_hi = ask(s, "maximum").and_then(|v| flat(&v).first().and_then(Value::as_f64));
            flags.insert("fieldMinValue".to_string(), json!(hard_lo.unwrap_or(-1e12)));
            flags.insert("fieldMaxValue".to_string(), json!(hard_hi.unwrap_or(1e12)));
            binds.push(("value".to_string(), plug.to_string()));
        }
    }
    Ok(Plan { kind, flags, binds, items })
}

fn enumerated(a: &Args) -> Vec<(i64, String)> {
    let vals = a.get("enumeratedItem").map(flat).unwrap_or_default();
    vals.chunks(2).filter_map(|p| if let [v, l] = p { Some((number(v) as i64, text(l))) } else { None }).collect()
}

fn install(s: &mut Session, cmd: &str, path: &str, plan: &Plan, items: Vec<(i64, String)>) {
    let values = (!items.is_empty()).then(|| items.iter().map(|x| x.0).collect::<Vec<i64>>());
    for (_, label) in &items {
        let flags = Map::from_iter([("parent".to_string(), json!(path)), ("label".to_string(), json!(label))]);
        let _ = create_as("menuItem", "menuItem", &Args { positional: Vec::new(), flags });
    }
    for (slot, plug) in &plan.binds {
        bind(s, path, slot, plug, values.clone());
    }
    let _ = cmd;
}

/// The reference takes a label from the attribute when none is given, except where the command leaves it to the script.
fn default_label(s: &mut Session, cmd: &str, a: &Args, plug: &str, flags: &mut Map<String, Value>) {
    if !a.has("label") && cmd != "attrEnumOptionMenu" {
        flags.insert("label".to_string(), json!(nice_name(s, plug)));
    }
}

fn attr_command(s: &mut Session, cmd: &'static str, a: &Args) -> CmdResult {
    if a.bool("query") || (a.bool("exists") && !a.bool("edit")) {
        let plain = crate::scriptui::command_for(cmd, a);
        return plain;
    }
    if a.bool("edit") {
        return attr_edit(s, cmd, a);
    }
    if a.has("defineTemplate") {
        return create_as(cmd, cmd, a);
    }
    let attribute = a.get("attribute").map(text).filter(|t| !t.is_empty());
    let (kind, plan) = match &attribute {
        Some(plug) => {
            let p = plan_for(s, cmd, plug).map_err(CommandError::new)?;
            (p.kind, Some(p))
        }
        None => (
            match cmd {
                "attrColorSliderGrp" => "colorSliderGrp",
                "attrEnumOptionMenuGrp" => "optionMenuGrp",
                "attrEnumOptionMenu" => "optionMenu",
                "attrFieldGrp" => "floatFieldGrp",
                "attrNavigationControlGrp" => "textFieldGrp",
                "attrFieldSliderGrp" => "floatSliderGrp",
                _ => fail!("{cmd}: give -attribute."),
            },
            None,
        ),
    };
    let mut flags = a.flags.clone();
    if let Some(p) = &plan {
        for (k, v) in &p.flags {
            flags.entry(k.clone()).or_insert(v.clone());
        }
    }
    if let Some(plug) = &attribute {
        default_label(s, cmd, a, plug, &mut flags);
    }
    let made = create_as(kind, cmd, &Args { positional: a.positional.clone(), flags })?;
    let path = made.as_str().unwrap_or("").to_string();
    if let Some(p) = &plan {
        let items = if a.has("enumeratedItem") { enumerated(a) } else { p.items.clone() };
        install(s, cmd, &path, p, items);
    } else if a.has("enumeratedItem") {
        for (_, label) in enumerated(a) {
            let f = Map::from_iter([("parent".to_string(), json!(path)), ("label".to_string(), json!(label))]);
            let _ = create_as("menuItem", "menuItem", &Args { positional: Vec::new(), flags: f });
        }
    }
    Ok(made)
}

/// -e on an attr control: a new attribute rebinds it, and another type of attribute makes it the other control, at the
/// end of its layout. The rest is an edit of the plain control.
fn attr_edit(s: &mut Session, cmd: &'static str, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("{cmd}: name a control.") };
    let Some(path) = control_full_path(&name) else { fail!("Object '{name}' not found.") };
    let mut path = path;
    if let Some(plug) = a.get("attribute").map(text).filter(|t| !t.is_empty()) {
        let plan = plan_for(s, cmd, &plug).map_err(CommandError::new)?;
        let items = if a.has("enumeratedItem") { enumerated(a) } else { plan.items.clone() };
        let was = control_kind(&path).unwrap_or_default();
        unbind(&path);
        if was == plan.kind {
            let mut flags = plan.flags.clone();
            flags.retain(|k, _| !a.flags.contains_key(k));
            flags.insert("attribute".to_string(), json!(plug));
            if control_prop(&path, "__autoLabel").is_some() {
                flags.insert("label".to_string(), json!(nice_name(s, &plug)));
            }
            // An option menu's items are the attribute's.
            if was.starts_with("optionMenu") {
                flags.insert("deleteAllItems".to_string(), json!(true));
            }
            model_edit(&path, &flags).map_err(CommandError::new)?;
            install(s, cmd, &path, &plan, items);
        } else {
            let mut flags: Map<String, Value> = crate::scriptui::control_props(&path).unwrap_or_default();
            flags.retain(|k, _| !k.starts_with("__") && !k.starts_with("value") && !matches!(k.as_str(), "rgbValue" | "select" | "text" | "plugState" | "numberOfFields" | "numberOfCheckBoxes" | "field" | "minValue" | "maxValue" | "fieldMinValue" | "fieldMaxValue" | "name"));
            for (k, v) in &plan.flags {
                flags.insert(k.clone(), v.clone());
            }
            flags.insert("attribute".to_string(), json!(plug));
            if let Some((parent, _)) = path.rsplit_once('|') {
                flags.insert("parent".to_string(), json!(parent));
            }
            crate::scriptui::delete_control(&path);
            let short = path.rsplit('|').next().unwrap_or(&path).to_string();
            let made = create_as(plan.kind, cmd, &Args { positional: vec![json!(short)], flags })?;
            path = made.as_str().unwrap_or("").to_string();
            install(s, cmd, &path, &plan, items);
        }
    }
    let mut rest = a.flags.clone();
    rest.remove("attribute");
    rest.remove("edit");
    rest.remove("enumeratedItem");
    rest.retain(|_, v| !v.is_null());
    if !rest.is_empty() {
        model_edit(&path, &rest).map_err(CommandError::new)?;
        write_edits(s, &path);
    }
    Ok(json!(path))
}

const ATTR_COMMON: &str = "attribute:at:a label:l:a changeCommand:cc:a extraButton:eb:b extraButtonCommand:ebc:a extraButtonIcon:ebi:a extraLabel:el:a forceAddMapButton:fmb:b hideMapButton:hmb:b adjustableColumn:adj:a columnWidth:cw:a2 columnAlign:cal:a2 columnAttach:cat:a3 rowAttach:rat:a3 vertical:vr:b precision:pre:a";

const ATTR_KINDS: &[(&str, &str)] = &[
    ("attrFieldSliderGrp", "minValue:min:a maxValue:max:a fieldMinValue:fmn:a fieldMaxValue:fmx:a sliderMinValue:smn:a sliderMaxValue:smx:a step:s:a fieldStep:fs:a sliderStep:ss:a value:v:a"),
    ("attrFieldGrp", "minValue:min:a maxValue:max:a step:s:a numberOfFields:nf:a"),
    ("attrControlGrp", "attribute:a:a enable:en:b"),
    ("attrColorSliderGrp", "rgbValue:rgb:a hsvValue:hsv:a showButton:sb:b attrNavDecision:nd:a2 hideButton:hb:b"),
    ("attrEnumOptionMenuGrp", "enumeratedItem:ei:a2 value:v:a select:sl:a"),
    ("attrEnumOptionMenu", "enumeratedItem:ei:a2 value:v:a select:sl:a"),
    ("attrNavigationControlGrp", "attrNavDecision:nd:a2 connectToExisting:ce:a createNew:cn:a defaultTraversal:dtv:a delete:d:a disconnect:dn:a ignoreNotSupported:ins:b noIgnorableMenu:nim:b relatedNodes:ren:a text:tx:a unignore:u:a"),
];

pub(crate) fn register(s: &mut Session) {
    for (name, spec) in ATTR_KINDS {
        let cmd: &'static str = name;
        let mut c = Cmd::new(cmd, &format!("{cmd}: a control bound to the attribute given by -attribute; it shows the plug, sets it undoably, and follows it."), move |s, a| attr_command(s, cmd, a)).positional("[name]").undoable(false);
        c = add_flags(c, spec);
        c = add_flags(c, ATTR_COMMON);
        // Queries go to the plain control underneath, whichever flags it has.
        c = add_flags(c, SLIDER_GRP);
        c = add_flags(c, FIELD_GRP);
        c = add_column_flags(c);
        c = add_flags(c, COMMON);
        s.register(c);
    }
    s.register(add_flags(
        Cmd::new("clipEditor", "Clips are not available here: warns, and opens the Dope Sheet.", |_s, a| clip_editor(a)).positional("[name]").undoable(false),
        "allTrackHeights:th:a autoFit:af:a autoFitTime:aft:a clipDropCmd:cd:a clipStyle:cs:a deleteCmd:dlc:a deselectAll:da:b displayActiveKeyTangents:dat:a displayActiveKeys:dak:a displayInfinities:di:a displayKeys:dk:a displayTangents:dtn:a displayValues:dv:a frameAll:fa:b frameRange:fr:a highlightConnection:hlc:a highlightedBlend:hb:a highlightedClip:hc:a initialized:in:b listAllCharacters:lac:b listCurrentCharacters:lc:b lockMainConnection:lck:b lookAt:la:a mainListConnection:mlc:a manageSequencer:ms:b menuContext:mc:b parent:p:a selectBlend:sb:a selectClip:sc:a selectionConnection:slc:a snapTime:st:a snapValue:sv:a stateString:sts:b useTemplate:ut:a defineTemplate:dt:a exists:ex:b query:q:b edit:e:b queried::a",
    ));
    s.register(
        CommandDef::new("connectControl", "connectControl [-index n] control attribute...: binds a control to plugs. The control shows the plug, sets it undoably, and follows it. On a group, -index 1 is its label, 2 its first field.", connect_control)
            .positional("control attribute...")
            .undoable(false)
            .flag(FlagDef::new("index", "in", ArgType::Int, Value::Null, "Which part of a group."))
            .flag(FlagDef::new("fileName", "fi", ArgType::Bool, Value::Null, "A file name attribute."))
            .flag(FlagDef::new("preventContextualMenu", "pcm", ArgType::Bool, Value::Null, "Accepted for existing scripts."))
            .flag(FlagDef::new("preventOverride", "po", ArgType::Bool, Value::Null, "Accepted for existing scripts.")),
    );
}
