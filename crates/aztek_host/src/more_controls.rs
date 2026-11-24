//! Script UI controls with state past their flags, and a few dialogs.
//!
//! - floatSlider2: two handles on one range, handle 1 never above handle 2, each able to drive a floatField and run a
//!   command with its value added.
//! - falloffCurve and falloffCurveAttr: a curve of control points in the unit square, kept in an optionVar (one
//!   "position,value" string per point) or in a multi attribute of compounds (position, value).
//! - componentBox: the selected components of the last node on the active list, one row each with its local X, Y and
//!   Z; a cell edited moves the component.
//! - fileBrowserDialog (the older file browser, run as fileDialog2), fontDialog, framelessDialog.
//!
//! The model in `scriptui` holds every flag; this module checks and derives values before they go in, writes curves
//! back to where they live, and handles what the shell reports for these controls (`shell_changed`) and keeps them up
//! to date with the scene (`idle`).

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::core::Core;
use crate::scriptui::{call_hook, callback_value, command_for, control_full_path, control_kind, control_prop, control_props, controls_of, model_edit, run_soon};

type Cmd = CommandDef<Session>;

pub(crate) const SLIDER2: &str = "floatSlider2";
pub(crate) const CURVE: &str = "falloffCurve";
pub(crate) const CURVE_ATTR: &str = "falloffCurveAttr";
pub(crate) const COMPONENT_BOX: &str = "componentBox";

/// The flags of each control kind here, in the form `scriptui`'s kind table takes.
pub(crate) const SLIDER2_FLAGS: &str = "changeCommand1:cc1:a changeCommand2:cc2:a maximum:max:a minimum:min:a polarity:pol:a positionControl1:pc1:a positionControl2:pc2:a value1:v1:a value2:v2:a values:vs:a2";
pub(crate) const CURVE_FLAGS: &str = "addControlVertex:acv:a asString:as:a changeCommand:cc:a currentKey:ck:a currentKeyValue:ckv:a2 customCurveWidget:ccw:b deleteControlVertex:dcv:a optionVar:ov:a readOnly:ro:b snapToGrid:stg:b";
pub(crate) const CURVE_ATTR_FLAGS: &str = "addControlVertex:acv:a asString:as:a attribute:at:a changeCommand:cc:a currentKey:ck:a currentKeyValue:ckv:a2 customCurveWidget:ccw:a deleteControlVertex:dcv:a readOnly:ro:a selectedPositionControl:spc:a selectedValueControl:svc:a snapToGrid:stg:a";
pub(crate) const COMPONENT_BOX_FLAGS: &str = "execute:exe:a2 labelWidth:lw:a maxHeight:mh:a maxWidth:mw:a precision:pre:a rowHeight:rh:a selectedAttr:sla:b";

/// Fired by the shell: a handle of a floatSlider2 moved (the value as the argument).
const SLIDER2_MOVED: [&str; 2] = ["__slider2Moved1", "__slider2Moved2"];
/// Fired by the shell: a curve's points changed (asString and currentKey come in `set`).
const CURVE_CHANGED: &str = "__curveChanged";
/// Fired by the shell: a curve's control point picked (currentKey comes in `set`).
const CURVE_PICKED: &str = "__curveKeyPicked";
/// Fired by the shell: a componentBox cell edited, with (component, axis 0 to 2, value).
const BOX_EDIT: &str = "__componentBoxEdit";
/// The grid control points snap to with -snapToGrid.
const GRID: f64 = 0.1;

pub(crate) fn is_mine(cmd: &str) -> bool {
    matches!(cmd, SLIDER2 | CURVE | CURVE_ATTR | COMPONENT_BOX)
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        Value::Bool(b) => (*b as i32).to_string(),
        o => o.to_string(),
    }
}

fn number(v: &Value) -> f64 {
    v.as_f64().or_else(|| v.as_bool().map(|b| b as i32 as f64)).or_else(|| v.as_str().and_then(|s| s.trim().parse().ok())).unwrap_or(0.0)
}

fn flat(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        o => vec![o.clone()],
    }
}

fn truth(v: &Value) -> bool {
    number(v) != 0.0 || v.as_str().is_some_and(|s| matches!(s, "true" | "on" | "yes"))
}

fn fmt(f: f64) -> String {
    crate::mel::fmt_float(f)
}

fn mel_quote(t: &str) -> String {
    format!("\"{}\"", t.replace('\\', "\\\\").replace('"', "\\\""))
}

fn err(m: impl Into<String>) -> CommandError {
    CommandError::new(m.into())
}

/// The flag asked about in a query.
fn asked(a: &Args) -> Option<String> {
    a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| a.flags.keys().find(|k| !matches!(k.as_str(), "query" | "queried")).cloned())
}

/// The control a query or edit names, by full path.
fn named(cmd: &str, a: &Args) -> Result<String, CommandError> {
    let Some(n) = a.pos_str(0) else { return Err(err(format!("{cmd}: name a control."))) };
    control_full_path(n).filter(|p| control_kind(p).as_deref() == Some(cmd)).ok_or_else(|| err(format!("{cmd}: Object '{n}' not found.")))
}

fn edit1(path: &str, key: &str, v: Value) {
    let _ = model_edit(path, &Map::from_iter([(key.to_string(), v)]));
}

pub(crate) fn command(s: &mut Session, cmd: &str, a: &Args) -> CmdResult {
    if a.has("defineTemplate") || (a.bool("exists") && !a.bool("edit") && !a.bool("query")) {
        return command_for(cmd, a);
    }
    match cmd {
        SLIDER2 => slider2(a),
        CURVE | CURVE_ATTR => curve(s, cmd, a),
        _ => component_box(s, a),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// floatSlider2

fn slider2(a: &Args) -> CmdResult {
    if a.bool("query") {
        return command_for(SLIDER2, a);
    }
    let old = if a.bool("edit") { control_props(&named(SLIDER2, a)?).unwrap_or_default() } else { Map::new() };
    let get = |k: &str, d: f64| a.get(k).map(number).unwrap_or_else(|| old.get(k).map(number).unwrap_or(d));
    let min = get("minimum", 0.0);
    let max = get("maximum", 10.0);
    if max <= min {
        if a.has("maximum") {
            fail!("floatSlider2: -maximum {} is not above the minimum {}.", fmt(max), fmt(min));
        }
        fail!("floatSlider2: -minimum {} is not below the maximum {}.", fmt(min), fmt(max));
    }
    let pair = a.get("values").map(flat).filter(|v| v.len() >= 2);
    let (mut v1, mut v2) = (get("value1", min), get("value2", max));
    if let Some(p) = &pair {
        v1 = number(&p[0]);
        v2 = number(&p[1]);
        if v1 > v2 {
            fail!("floatSlider2: -values {} {}: the first value is above the second.", fmt(v1), fmt(v2));
        }
    } else if v1 > v2 {
        if a.has("value1") && !a.has("value2") {
            fail!("floatSlider2: -value1 {} is above handle 2's value {}.", fmt(v1), fmt(v2));
        }
        fail!("floatSlider2: -value2 {} is below handle 1's value {}.", fmt(v2), fmt(v1));
    }
    // The handles stay on the slider.
    let (v1, v2) = (v1.clamp(min, max), v2.clamp(min, max));
    if let Some(p) = a.get("polarity") {
        let p = number(p) as i64;
        if p != 0 && p != 1 {
            fail!("floatSlider2: -polarity is 0 or 1, not {p}.");
        }
    }
    let mut plain = a.clone();
    plain.flags.remove("values");
    for (k, v) in [("minimum", min), ("maximum", max), ("value1", v1), ("value2", v2)] {
        plain.flags.insert(k.to_string(), json!(v));
    }
    if let Some(p) = a.get("polarity") {
        plain.flags.insert("polarity".into(), json!(number(p) as i64));
    }
    let r = command_for(SLIDER2, &plain)?;
    let path = if a.bool("edit") { named(SLIDER2, a)? } else { r.as_str().unwrap_or("").to_string() };
    show_handles(&path);
    Ok(r)
}

/// The floatFields named by -positionControl1 and -positionControl2 show their handle's value.
fn show_handles(path: &str) {
    let Some(p) = control_props(path) else { return };
    for i in 1..=2 {
        let Some(field) = p.get(&format!("positionControl{i}")).map(text).filter(|t| !t.is_empty()) else { continue };
        let v = p.get(&format!("value{i}")).map(number).unwrap_or(0.0);
        if let Some(f) = control_full_path(&field) {
            edit1(&f, "value", json!(v));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// falloffCurve and falloffCurveAttr

type Points = Vec<(f64, f64)>;

/// "p,v,p,v...", or any list of such strings (an optionVar's), as points in order of position.
fn parse_points(v: &Value) -> Points {
    let nums: Vec<f64> = flat(v).iter().flat_map(|x| text(x).split(',').map(|t| t.trim().parse::<f64>().unwrap_or(0.0)).collect::<Vec<_>>()).collect();
    let mut pts: Points = nums.chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clamp(0.0, 1.0), c[1].clamp(0.0, 1.0))).collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts
}

fn points_string(p: &Points) -> String {
    p.iter().map(|(x, y)| format!("{},{}", fmt(*x), fmt(*y))).collect::<Vec<_>>().join(",")
}

fn snap(v: f64) -> f64 {
    ((v / GRID).round() * GRID).clamp(0.0, 1.0)
}

/// Sorts the points by position and gives where the point that was at `key` went.
fn sort_keeping(p: &mut Points, key: usize) -> usize {
    let tagged: Vec<(usize, (f64, f64))> = p.iter().copied().enumerate().collect();
    let mut t = tagged;
    t.sort_by(|a, b| a.1 .0.total_cmp(&b.1 .0));
    let at = t.iter().position(|(i, _)| *i == key).unwrap_or(0);
    *p = t.into_iter().map(|(_, q)| q).collect();
    at
}

/// A curve attribute's plug without an element index, its node, and the names of the position and value children.
struct CurvePlug {
    plug: String,
    pos: String,
    val: String,
}

fn curve_plug(s: &mut Session, at: &str) -> Result<CurvePlug, String> {
    let mut plug = at.trim().to_string();
    if plug.ends_with(']') {
        if let Some(i) = plug.rfind('[') {
            plug.truncate(i);
        }
    }
    let Some((node, attr)) = plug.split_once('.') else { return Err(format!("falloffCurveAttr: {at} is not an attribute.")) };
    let kids = s.run("attributeQuery", json!({"args": [attr], "node": node, "listChildren": true})).map_err(|e| e.to_string())?;
    let kids: Vec<String> = flat(&kids).iter().map(text).filter(|k| !k.is_empty()).collect();
    // The children are elements of a multi: the position child is an array attribute.
    let multi = kids.first().and_then(|k| s.graph.plug(&format!("{plug}[0].{k}")).ok()).is_some_and(|p| s.graph.attr(&p).is_array());
    if kids.len() < 2 || !multi {
        return Err(format!("falloffCurveAttr: {plug} is not a multi of compounds holding a position and a value."));
    }
    Ok(CurvePlug { plug, pos: kids[0].clone(), val: kids[1].clone() })
}

fn indices(s: &mut Session, plug: &str) -> Vec<i64> {
    let v = s.run("getAttr", json!({"args": [plug], "multiIndices": true})).unwrap_or(Value::Null);
    let mut i: Vec<i64> = flat(&v).iter().filter(|x| !x.is_null()).map(|x| number(x) as i64).collect();
    i.sort();
    i.dedup();
    i
}

/// The attribute's elements as (index, position, value), in index order.
fn read_elements(s: &mut Session, cp: &CurvePlug) -> Vec<(i64, f64, f64)> {
    let mut out = Vec::new();
    for i in indices(s, &cp.plug) {
        let g = |s: &mut Session, c: &str| s.run("getAttr", json!({"args": [format!("{}[{i}].{c}", cp.plug)]})).ok().map(|v| flat(&v).first().map(number).unwrap_or(0.0)).unwrap_or(0.0);
        let p = g(s, &cp.pos);
        let v = g(s, &cp.val);
        out.push((i, p, v));
    }
    out
}

fn read_attr(s: &mut Session, cp: &CurvePlug) -> Points {
    let mut pts: Points = read_elements(s, cp).into_iter().map(|(_, p, v)| (p, v)).collect();
    pts.sort_by(|a, b| a.0.total_cmp(&b.0));
    pts
}

/// Writes the points into the multi: points already there keep their elements, changed ones take the elements left,
/// new ones are added after the last, and elements no longer needed are removed.
fn write_attr(s: &mut Session, cp: &CurvePlug, pts: &Points) -> Result<(), String> {
    let had = read_elements(s, cp);
    let same = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let mut used = vec![false; had.len()];
    let mut todo: Vec<(f64, f64)> = Vec::new();
    for &(p, v) in pts {
        match had.iter().enumerate().position(|(n, e)| !used[n] && same(e.1, p) && same(e.2, v)) {
            Some(n) => used[n] = true,
            None => todo.push((p, v)),
        }
    }
    let mut free: Vec<i64> = had.iter().enumerate().filter(|(n, _)| !used[*n]).map(|(_, e)| e.0).collect();
    free.reverse();
    let mut next = had.last().map(|e| e.0 + 1).unwrap_or(0);
    for (p, v) in todo {
        let i = free.pop().unwrap_or_else(|| {
            next += 1;
            next - 1
        });
        for (c, x) in [(&cp.pos, p), (&cp.val, v)] {
            s.run("setAttr", json!({"args": [format!("{}[{i}].{c}", cp.plug), x]})).map_err(|e| e.to_string())?;
        }
    }
    for i in free {
        s.run("removeMultiInstance", json!({"args": [format!("{}[{i}]", cp.plug)], "break": true})).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn option_var_points(s: &Session, name: &str) -> Option<Points> {
    s.option_vars.get(name).map(parse_points)
}

fn write_option_var(s: &mut Session, name: &str, pts: &Points) {
    let list: Vec<Value> = pts.iter().map(|(x, y)| json!(format!("{},{}", fmt(*x), fmt(*y)))).collect();
    s.option_vars.insert(name.to_string(), Value::Array(list));
}

/// Where a curve control's points live now: its attribute or optionVar, when it has one.
fn stored_points(s: &mut Session, props: &Map<String, Value>) -> Option<Points> {
    if let Some(at) = props.get("attribute").map(text).filter(|t| !t.is_empty()) {
        let cp = curve_plug(s, &at).ok()?;
        return Some(read_attr(s, &cp));
    }
    let ov = props.get("optionVar").map(text).filter(|t| !t.is_empty())?;
    option_var_points(s, &ov)
}

fn write_back(s: &mut Session, props: &Map<String, Value>, pts: &Points) -> Result<(), String> {
    if let Some(at) = props.get("attribute").map(text).filter(|t| !t.is_empty()) {
        let cp = curve_plug(s, &at)?;
        return write_attr(s, &cp, pts);
    }
    if let Some(ov) = props.get("optionVar").map(text).filter(|t| !t.is_empty()) {
        write_option_var(s, &ov, pts);
    }
    Ok(())
}

/// The curve a control shows, read again from where it lives.
fn refresh_curve(s: &mut Session, path: &str) {
    let Some(props) = control_props(path) else { return };
    let Some(pts) = stored_points(s, &props) else { return };
    let now = points_string(&pts);
    if props.get("asString").map(text).as_deref() != Some(now.as_str()) {
        let key = props.get("currentKey").map(|v| number(v) as i64).unwrap_or(0);
        let mut e = Map::new();
        e.insert("asString".into(), json!(now));
        e.insert("currentKey".into(), json!(key.clamp(0, (pts.len() as i64 - 1).max(0))));
        let _ = model_edit(path, &e);
        show_selected_key(path);
    }
}

fn curve(s: &mut Session, cmd: &str, a: &Args) -> CmdResult {
    if a.bool("query") {
        let path = named(cmd, a)?;
        refresh_curve(s, &path);
        let props = control_props(&path).unwrap_or_default();
        let pts = parse_points(props.get("asString").unwrap_or(&Value::Null));
        let key = props.get("currentKey").map(|v| number(v) as usize).unwrap_or(0);
        return match asked(a).as_deref() {
            Some("asString") => Ok(json!(points_string(&pts))),
            Some("currentKeyValue") => Ok(match pts.get(key) {
                Some((x, y)) => json!([x, y]),
                None => json!([]),
            }),
            Some("currentKey") => Ok(json!(if pts.is_empty() { -1 } else { key as i64 })),
            Some("readOnly" | "snapToGrid" | "customCurveWidget") if cmd == CURVE_ATTR => {
                let k = asked(a).unwrap();
                Ok(json!(props.get(&k).is_some_and(truth) as i64))
            }
            _ => command_for(cmd, a),
        };
    }
    let edit = a.bool("edit");
    if edit && a.has("attribute") {
        fail!("{cmd}: -attribute is set when the control is made.");
    }
    // What the control holds now, and where it keeps it after this call.
    let (path, mut props) = if edit {
        let p = named(cmd, a)?;
        let props = control_props(&p).unwrap_or_default();
        (Some(p), props)
    } else {
        (None, Map::new())
    };
    for k in ["attribute", "optionVar"] {
        if let Some(v) = a.get(k) {
            props.insert(k.to_string(), v.clone());
        }
    }
    if !edit && cmd == CURVE_ATTR && !a.has("attribute") {
        fail!("falloffCurveAttr: give the curve attribute with -attribute.");
    }
    if let Some(at) = a.get("attribute").map(text) {
        curve_plug(s, &at).map_err(err)?;
    }
    // A new curve with nothing to read is full at the start and nothing at the end.
    let mut pts = match props.get("asString") {
        Some(v) => parse_points(v),
        None => vec![(0.0, 1.0), (1.0, 0.0)],
    };
    // A store named now gives the curve when it holds one, and is given the curve when it doesn't.
    if a.has("optionVar") || a.has("attribute") {
        if let Some(p) = stored_points(s, &props).filter(|p| !p.is_empty() || a.has("attribute")) {
            pts = p;
        }
    }
    if let Some(v) = a.get("asString") {
        pts = parse_points(v);
    }
    let snapping = a.get("snapToGrid").map(truth).unwrap_or_else(|| props.get("snapToGrid").is_some_and(truth));
    let mut key = a.get("currentKey").map(|v| number(v) as i64).unwrap_or_else(|| props.get("currentKey").map(|v| number(v) as i64).unwrap_or(0));
    if let Some(v) = a.get("addControlVertex") {
        let add = parse_points(v);
        let Some(&(x, y)) = add.first() else { fail!("{cmd}: -addControlVertex takes \"position,value\".") };
        pts.push((x, y));
        let last = pts.len() - 1;
        key = sort_keeping(&mut pts, last) as i64;
    }
    if let Some(v) = a.get("deleteControlVertex") {
        let i = number(v) as i64;
        if i < 0 || i as usize >= pts.len() {
            fail!("{cmd}: -deleteControlVertex {i}: the curve has {} control vertices.", pts.len());
        }
        pts.remove(i as usize);
        if key >= i && key > 0 {
            key -= 1;
        }
    }
    if a.has("currentKey") && (key < 0 || key as usize >= pts.len()) {
        fail!("{cmd}: -currentKey {key}: the curve has {} control vertices.", pts.len());
    }
    if let Some(v) = a.get("currentKeyValue") {
        let xy = flat(v);
        if xy.len() < 2 {
            fail!("{cmd}: -currentKeyValue takes a position and a value.");
        }
        let k = key.max(0) as usize;
        if k >= pts.len() {
            fail!("{cmd}: the curve has no control vertex {k}.");
        }
        let (mut x, mut y) = (number(&xy[0]).clamp(0.0, 1.0), number(&xy[1]).clamp(0.0, 1.0));
        if snapping {
            x = snap(x);
            y = snap(y);
        }
        pts[k] = (x, y);
        key = sort_keeping(&mut pts, k) as i64;
    }
    key = key.clamp(0, (pts.len() as i64 - 1).max(0));
    let mut plain = a.clone();
    for k in ["addControlVertex", "deleteControlVertex", "currentKeyValue"] {
        plain.flags.remove(k);
    }
    plain.flags.insert("asString".into(), json!(points_string(&pts)));
    plain.flags.insert("currentKey".into(), json!(key));
    let r = command_for(cmd, &plain)?;
    let path = path.unwrap_or_else(|| r.as_str().unwrap_or("").to_string());
    let props = control_props(&path).unwrap_or_default();
    write_back(s, &props, &pts).map_err(err)?;
    show_selected_key(&path);
    Ok(r)
}

/// falloffCurveAttr's -selectedPositionControl and -selectedValueControl sliders show the current key.
fn show_selected_key(path: &str) {
    let Some(p) = control_props(path) else { return };
    let pts = parse_points(p.get("asString").unwrap_or(&Value::Null));
    let key = p.get("currentKey").map(|v| number(v) as usize).unwrap_or(0);
    let Some(&(x, y)) = pts.get(key) else { return };
    for (flag, v) in [("selectedPositionControl", x), ("selectedValueControl", y)] {
        if let Some(f) = p.get(flag).map(text).filter(|t| !t.is_empty()).and_then(|n| control_full_path(&n)) {
            edit1(&f, "value", json!(v));
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// componentBox

const AXES: [&str; 3] = ["xValue", "yValue", "zValue"];

/// The rows a component box shows: the selected components of the last node on the active list, with their local
/// positions.
fn box_rows(s: &mut Session) -> Value {
    let sel = s.run("ls", json!({"selection": true, "flatten": true})).unwrap_or(Value::Null);
    let sel: Vec<String> = flat(&sel).iter().map(text).filter(|t| !t.is_empty()).collect();
    let Some(last) = sel.last() else { return json!({"node": "", "path": "", "rows": []}) };
    let node = last.split('.').next().unwrap_or("").to_string();
    let full = s.run("ls", json!({"args": [node.clone()], "long": true})).ok().and_then(|v| flat(&v).first().map(text)).unwrap_or_else(|| node.clone());
    let mut rows = Vec::new();
    for c in sel.iter().filter(|c| c.split('.').next() == Some(node.as_str()) && c.contains('[')) {
        let Some((_, comp)) = c.split_once('.') else { continue };
        let Ok(p) = s.run("pointPosition", json!({"args": [c], "local": true})) else { continue };
        let p: Vec<f64> = flat(&p).iter().map(number).collect();
        if p.len() < 3 {
            continue;
        }
        rows.push(json!({"name": comp, "values": [p[0], p[1], p[2]]}));
    }
    json!({"node": node, "path": full, "rows": rows})
}

fn refresh_box(s: &mut Session, path: &str) {
    let now = box_rows(s);
    if control_prop(path, "__rows").as_ref() != Some(&now) {
        // Selected cells that are gone are no longer selected.
        let names: Vec<String> = now["rows"].as_array().into_iter().flatten().filter_map(|r| r["name"].as_str()).flat_map(|n| AXES.iter().map(move |a| format!("{n}.{a}"))).collect();
        let sel: Vec<Value> = control_prop(path, "selectedAttr").map(|v| flat(&v)).unwrap_or_default().into_iter().filter(|v| names.contains(&text(v))).collect();
        let mut e = Map::new();
        e.insert("__rows".into(), now);
        e.insert("selectedAttr".into(), Value::Array(sel));
        let _ = model_edit(path, &e);
    }
}

fn component_box(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let path = named(COMPONENT_BOX, a)?;
        refresh_box(s, &path);
        return match asked(a).as_deref() {
            Some("selectedAttr") => Ok(Value::Array(control_prop(&path, "selectedAttr").map(|v| flat(&v)).unwrap_or_default().into_iter().filter(|v| !v.is_null()).collect())),
            Some("precision" | "labelWidth" | "maxHeight" | "maxWidth") => {
                let k = asked(a).unwrap();
                let d = if k == "precision" { 3 } else { 0 };
                Ok(json!(control_prop(&path, &k).map(|v| number(&v) as i64).unwrap_or(d)))
            }
            _ => command_for(COMPONENT_BOX, a),
        };
    }
    if let Some(p) = a.get("precision") {
        let p = number(p) as i64;
        if !(0..=15).contains(&p) {
            fail!("componentBox: -precision {p} is out of range (0 to 15).");
        }
    }
    let mut plain = a.clone();
    let exe = plain.flags.remove("execute");
    let r = command_for(COMPONENT_BOX, &plain)?;
    let path = if a.bool("edit") { named(COMPONENT_BOX, a)? } else { r.as_str().unwrap_or("").to_string() };
    refresh_box(s, &path);
    if let Some(e) = exe.filter(|v| !v.is_null()) {
        let e = flat(&e);
        let script = e.first().map(text).unwrap_or_default();
        let only_selected = e.get(1).is_some_and(truth);
        let rows = control_prop(&path, "__rows").unwrap_or(Value::Null);
        let selected: Vec<String> = control_prop(&path, "selectedAttr").map(|v| flat(&v).iter().map(text).collect()).unwrap_or_default();
        let node = text(&rows["node"]);
        let full = text(&rows["path"]);
        for r in rows["rows"].as_array().into_iter().flatten() {
            let comp = text(&r["name"]);
            for ax in AXES {
                let attr = format!("{comp}.{ax}");
                if only_selected && !selected.contains(&attr) {
                    continue;
                }
                let line = script.replace("#A", &attr).replace("#N", &node).replace("#P", &full);
                run_soon(callback_value(&json!(line)), Vec::new());
            }
        }
    }
    Ok(r)
}

// ---------------------------------------------------------------------------------------------------------------------
// What the shell reports, and keeping up with the scene

/// What the user did in one of these controls, or in a slider a curve control drives.
pub(crate) fn shell_changed(core: &mut Core, ev: &Value) {
    let Some(path) = ev["path"].as_str().and_then(control_full_path) else { return };
    let fired: Vec<String> = ev["fire"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    let args: Vec<Value> = ev["args"].as_array().cloned().unwrap_or_default();
    let kind = control_kind(&path).unwrap_or_default();
    match kind.as_str() {
        SLIDER2 => {
            show_handles(&path);
            for (i, flag) in SLIDER2_MOVED.iter().enumerate() {
                if !fired.iter().any(|f| f == flag) {
                    continue;
                }
                let v = control_prop(&path, &format!("value{}", i + 1)).map(|v| number(&v)).unwrap_or(0.0);
                // The handle's command has its new value added to it.
                if let Some(cb) = control_prop(&path, &format!("changeCommand{}", i + 1)) {
                    match cb {
                        Value::String(src) => core.run_callback(&json!(format!("{src} {}", fmt(v))), &[]),
                        o => core.run_callback(&o, &[json!(v)]),
                    }
                }
            }
        }
        CURVE | CURVE_ATTR if fired.iter().any(|f| f == CURVE_CHANGED) => {
            let props = control_props(&path).unwrap_or_default();
            let pts = parse_points(props.get("asString").unwrap_or(&Value::Null));
            if let Err(e) = write_back(&mut core.s, &props, &pts) {
                core.push(format!("// Error: {e} //"), crate::core::LineKind::Error);
            }
            show_selected_key(&path);
            if let Some(cb) = props.get("changeCommand") {
                core.run_callback(cb, &[]);
            }
        }
        CURVE | CURVE_ATTR if fired.iter().any(|f| f == CURVE_PICKED) => show_selected_key(&path),
        COMPONENT_BOX if fired.iter().any(|f| f == BOX_EDIT) => {
            let node = control_prop(&path, "__rows").map(|r| text(&r["node"])).unwrap_or_default();
            let comp = args.first().map(text).unwrap_or_default();
            let axis = args.get(1).map(|v| number(v) as usize).unwrap_or(0).min(2);
            let value = args.get(2).map(number).unwrap_or(0.0);
            let target = format!("{node}.{comp}");
            if let Ok(p) = core.s.run("pointPosition", json!({"args": [target.clone()], "local": true})) {
                let mut p: Vec<f64> = flat(&p).iter().map(number).collect();
                if p.len() >= 3 {
                    p[axis] = value;
                    let _ = core.exec("xform", json!({"args": [target], "objectSpace": true, "translation": [p[0], p[1], p[2]]}));
                }
            }
            refresh_box(&mut core.s, &path);
        }
        _ => {}
    }
    // A slider a curve control names: moving it moves the current key.
    if matches!(kind.as_str(), "floatSlider" | "floatSliderGrp") && ev["set"].get("value").is_some() {
        let v = control_prop(&path, "value").map(|v| number(&v)).unwrap_or(0.0);
        for (curve, props) in controls_of(CURVE_ATTR) {
            for (flag, axis) in [("selectedPositionControl", 0), ("selectedValueControl", 1)] {
                let named = props.get(flag).map(text).filter(|t| !t.is_empty()).and_then(|n| control_full_path(&n));
                if named.as_deref() != Some(path.as_str()) {
                    continue;
                }
                let mut pts = parse_points(props.get("asString").unwrap_or(&Value::Null));
                let k = props.get("currentKey").map(|v| number(v) as usize).unwrap_or(0);
                if k >= pts.len() {
                    continue;
                }
                if axis == 0 {
                    pts[k].0 = v.clamp(0.0, 1.0);
                } else {
                    pts[k].1 = v.clamp(0.0, 1.0);
                }
                let key = sort_keeping(&mut pts, k);
                let mut e = Map::new();
                e.insert("asString".into(), json!(points_string(&pts)));
                e.insert("currentKey".into(), json!(key));
                let _ = model_edit(&curve, &e);
                let _ = write_back(&mut core.s, &props, &pts);
                if let Some(cb) = props.get("changeCommand") {
                    core.run_callback(cb, &[]);
                }
            }
        }
    }
}

/// Idle: curves follow their attribute or optionVar, component boxes the selection and the points.
pub(crate) fn idle(core: &mut Core) {
    for kind in [CURVE, CURVE_ATTR] {
        for (path, _) in controls_of(kind) {
            refresh_curve(&mut core.s, &path);
        }
    }
    for (path, _) in controls_of(COMPONENT_BOX) {
        refresh_box(&mut core.s, &path);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Dialogs

fn f(name: &str, short: &str, ty: ArgType) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, "")
}

/// The older browser's file types, as fileDialog2 filters.
fn type_filter(t: &str) -> Option<&'static str> {
    Some(match t {
        "mayaAscii" => "Maya ASCII (*.ma)",
        "mayaBinary" => "Maya Binary (*.mb)",
        "mel" => "MEL (*.mel)",
        "OBJ" => "OBJ (*.obj)",
        "plug-in" => "Plug-ins (*.so *.dll *.bundle *.mll *.py)",
        "audio" => "Audio (*.wav *.aif *.aiff)",
        "move" => "Move (*.mov)",
        "EPS" => "EPS (*.eps)",
        "Illustrator" => "Illustrator (*.ai)",
        "image" => "Images (*.png *.jpg *.jpeg *.tif *.tiff *.exr *.hdr *.tga *.bmp *.iff *.dds *.psd)",
        _ => return None,
    })
}

/// -filterList entries, "Description,*.a;*.b", as fileDialog2 filters.
fn list_filter(entry: &str) -> String {
    match entry.split_once(',') {
        Some((d, pats)) => format!("{} ({})", d.trim(), pats.split(';').map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join(" ")),
        None => entry.trim().to_string(),
    }
}

thread_local! {
    static BROWSERS: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// fileBrowserDialog: what the older browser asks for, put to fileDialog2. The file picked goes to -fileCommand as
/// (file name, file type).
fn file_browser_dialog(a: &Args) -> CmdResult {
    let raw_mode = a.get("mode").map(|v| number(v) as i64).unwrap_or(0);
    let short_names = raw_mode >= 100;
    let mode = raw_mode % 100;
    let ftype = a.string("fileType", "");
    let mut filters: Vec<String> = a.get("filterList").map(flat).unwrap_or_default().iter().map(|v| list_filter(&text(v))).collect();
    if let Some(t) = type_filter(&ftype) {
        filters.insert(0, t.to_string());
    }
    if !filters.is_empty() {
        filters.push("All Files (*.*)".into());
    }
    let file_mode = match mode {
        1 | 2 => 0,
        4 => 3,
        _ if ftype == "directory" => 3,
        _ => 1,
    };
    let action = a.string("actionName", "").replace('_', " ");
    let ok = match a.get("includeName").map(text).filter(|t| !t.is_empty()) {
        Some(n) => {
            let n = if n.chars().count() > 32 { format!("...{}", n.chars().rev().take(29).collect::<Vec<_>>().into_iter().rev().collect::<String>()) } else { n };
            format!("{action} ({n})")
        }
        None => action,
    };
    let mut p = Map::new();
    p.insert("fileMode".into(), json!(file_mode));
    if !filters.is_empty() {
        p.insert("fileFilter".into(), json!(filters.join(";;")));
    }
    if let Some(t) = a.get("windowTitle") {
        p.insert("caption".into(), json!(text(t)));
    } else if !ok.is_empty() {
        p.insert("caption".into(), json!(ok.clone()));
    }
    if !ok.is_empty() {
        p.insert("okCaption".into(), json!(ok));
    }
    for k in ["dialogStyle", "tipMessage", "operationMode"] {
        if let Some(v) = a.get(k) {
            p.insert(k.into(), v.clone());
        }
    }
    let picked = call_hook("fileDialog2", &Value::Object(p));
    let file = picked.as_ref().map(flat).and_then(|l| l.first().map(text)).filter(|t| !t.is_empty());
    if let (Some(file), Some(cb)) = (file, a.get("fileCommand")) {
        let file = if short_names { file.rsplit(['/', '\\']).next().unwrap_or(&file).to_string() } else { file };
        let ftype = if ftype.is_empty() && file_mode == 3 { "directory".to_string() } else { ftype };
        match callback_value(cb) {
            Value::String(src) => run_soon(json!(format!("{src} {} {}", mel_quote(&file), mel_quote(&ftype))), Vec::new()),
            o => run_soon(o, vec![json!(file), json!(ftype)]),
        }
    }
    let n = BROWSERS.with(|b| {
        b.set(b.get() + 1);
        b.get()
    });
    Ok(json!(format!("fileBrowserDialog{n}")))
}

// fontDialog: the system's fonts, read from the font folders when there is no shell to ask.

fn font_dirs() -> Vec<std::path::PathBuf> {
    let mut d: Vec<std::path::PathBuf> = Vec::new();
    let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(std::path::PathBuf::from);
    if cfg!(windows) {
        if let Some(w) = std::env::var_os("WINDIR") {
            d.push(std::path::PathBuf::from(w).join("Fonts"));
        }
        if let Some(l) = std::env::var_os("LOCALAPPDATA") {
            d.push(std::path::PathBuf::from(l).join("Microsoft").join("Windows").join("Fonts"));
        }
    } else if cfg!(target_os = "macos") {
        d.extend(["/System/Library/Fonts", "/Library/Fonts"].map(std::path::PathBuf::from));
        if let Some(h) = &home {
            d.push(h.join("Library/Fonts"));
        }
    } else {
        d.extend(["/usr/share/fonts", "/usr/local/share/fonts", "/usr/share/X11/fonts"].map(std::path::PathBuf::from));
        if let Some(h) = &home {
            d.push(h.join(".fonts"));
            d.push(h.join(".local/share/fonts"));
        }
    }
    d
}

fn be16(b: &[u8], at: usize) -> Option<usize> {
    Some(u16::from_be_bytes([*b.get(at)?, *b.get(at + 1)?]) as usize)
}

fn be32(b: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes([*b.get(at)?, *b.get(at + 1)?, *b.get(at + 2)?, *b.get(at + 3)?]) as usize)
}

/// The family name in a TrueType or OpenType file (the first font of a collection).
pub fn font_family(b: &[u8]) -> Option<String> {
    let base = if b.get(0..4)? == b"ttcf" { be32(b, 12)? } else { 0 };
    let tables = be16(b, base + 4)?;
    let mut name = None;
    for i in 0..tables {
        let r = base + 12 + i * 16;
        if b.get(r..r + 4)? == b"name" {
            name = Some(be32(b, r + 8)?);
        }
    }
    let t = name?;
    let count = be16(b, t + 2)?;
    let strings = t + be16(b, t + 4)?;
    let mut best: Option<(u32, String)> = None;
    for i in 0..count {
        let r = t + 6 + i * 12;
        let (platform, lang, id, len, off) = (be16(b, r)?, be16(b, r + 4)?, be16(b, r + 6)?, be16(b, r + 8)?, be16(b, r + 10)?);
        if id != 1 && id != 16 {
            continue;
        }
        let raw = b.get(strings + off..strings + off + len)?;
        let s = match platform {
            0 | 3 => String::from_utf16_lossy(&raw.chunks(2).filter(|c| c.len() == 2).map(|c| u16::from_be_bytes([c[0], c[1]])).collect::<Vec<_>>()),
            1 => raw.iter().map(|&c| c as char).collect(),
            _ => continue,
        };
        if s.trim().is_empty() {
            continue;
        }
        // The typographic family first, then English, then Windows names.
        let rank = (id == 16) as u32 * 4 + (lang == 0x409 || (platform == 1 && lang == 0)) as u32 * 2 + (platform == 3) as u32;
        if best.as_ref().is_none_or(|(r, _)| rank > *r) {
            best = Some((rank, s.trim().to_string()));
        }
    }
    best.map(|(_, s)| s)
}

fn scan_fonts(dir: &std::path::Path, depth: u32, scalable_only: bool, out: &mut Vec<String>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        if p.is_dir() {
            if depth < 6 {
                scan_fonts(&p, depth + 1, scalable_only, out);
            }
            continue;
        }
        let name = p.file_name().map(|n| n.to_string_lossy().to_lowercase()).unwrap_or_default();
        let stem = || {
            let n = p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
            n.split('.').next().unwrap_or("").to_string()
        };
        if [".ttf", ".otf", ".ttc", ".otc"].iter().any(|x| name.ends_with(x)) {
            if let Some(fam) = std::fs::read(&p).ok().and_then(|b| font_family(&b)) {
                out.push(fam);
            }
        } else if name.ends_with(".pfb") || name.ends_with(".pfa") {
            out.push(stem());
        } else if !scalable_only && [".pcf", ".pcf.gz", ".bdf", ".fon"].iter().any(|x| name.ends_with(x)) {
            out.push(stem());
        }
    }
}

pub(crate) fn system_fonts(scalable_only: bool) -> Vec<String> {
    // The viewport's own font, shipped with the application.
    let mut out = vec!["DejaVu Sans".to_string()];
    for d in font_dirs() {
        scan_fonts(&d, 0, scalable_only, &mut out);
    }
    out.sort_by_key(|a| a.to_lowercase());
    out.dedup();
    out
}

fn font_dialog(a: &Args) -> CmdResult {
    let list = a.bool("FontList");
    let scalable = a.bool("scalable");
    let answer = call_hook("fontDialog", &json!({"fontList": list, "scalable": scalable}));
    if list {
        return Ok(match answer {
            Some(Value::Array(l)) => Value::Array(l),
            _ => json!(system_fonts(scalable)),
        });
    }
    // With no one to pick, nothing is picked.
    Ok(json!(answer.map(|v| text(&v)).unwrap_or_default()))
}

fn frameless_dialog(a: &Args) -> CmdResult {
    let buttons: Vec<String> = a.get("button").map(flat).unwrap_or_default().iter().map(text).collect();
    let primary: Vec<String> = a.get("primary").map(flat).unwrap_or_default().iter().map(text).collect();
    let mut p = Map::new();
    for k in ["title", "message", "path", "parent"] {
        if let Some(v) = a.get(k) {
            p.insert(k.into(), json!(text(v)));
        }
    }
    p.insert("button".into(), json!(buttons));
    p.insert("primary".into(), json!(primary));
    if let Some(v) = call_hook("framelessDialog", &Value::Object(p)) {
        return Ok(json!(text(&v)));
    }
    // With no one to press a button: the first primary one, as Enter would.
    Ok(json!(primary.iter().find(|b| buttons.contains(b)).or(buttons.first()).cloned().unwrap_or_default()))
}

/// grabColor: the shell waits for a mouse press and reports the pixel under the cursor as red, green, blue and alpha
/// from 0 to 1. With no shell to ask there is no screen: black, opaque.
fn grab_color(a: &Args) -> CmdResult {
    let got: Vec<f64> = call_hook("grabColor", &json!({})).map(|v| flat(&v).iter().map(number).collect()).unwrap_or_default();
    let px = |i: usize, d: f64| got.get(i).copied().unwrap_or(d).clamp(0.0, 1.0);
    let rgb = [px(0, 0.0), px(1, 0.0), px(2, 0.0)];
    let mut out: Vec<f64> = if a.bool("hsvValue") && !a.bool("rgbValue") { crate::scriptui::rgb_to_hsv(rgb).to_vec() } else { rgb.to_vec() };
    if a.bool("alpha") {
        out.push(px(3, 1.0));
    }
    Ok(json!(out))
}

pub(crate) fn register(s: &mut Session) {
    s.register(
        Cmd::new("grabColor", "Waits for a mouse press and returns the colour of the pixel under the cursor: red, green and blue (-rgb, the default) or hue, saturation and value (-hsv), with -a its alpha after.", |_s, a| grab_color(a))
            .undoable(true)
            .flag(f("alpha", "a", ArgType::Bool))
            .flag(f("hsvValue", "hsv", ArgType::Bool))
            .flag(f("rgbValue", "rgb", ArgType::Bool)),
    );
    s.register(
        Cmd::new(
            "fileBrowserDialog",
            "The older file browser, run as fileDialog2: -m 0 read, 1 or 2 write, 4 a folder (add 100 for short names), -ft \
             and -fl the filters, -an and -in the action's label, -wt the title. The file picked goes to -fileCommand as \
             (file name, file type). Returns the dialog's name.",
            |_s, a| file_browser_dialog(a),
        )
        .undoable(false)
        .flag(f("actionName", "an", ArgType::Any))
        .flag(f("dialogStyle", "ds", ArgType::Any))
        .flag(f("fileCommand", "fc", ArgType::Any))
        .flag(f("fileType", "ft", ArgType::Any))
        .flag(f("filterList", "fl", ArgType::Any).multi(1))
        .flag(f("includeName", "in", ArgType::Any))
        .flag(f("mode", "m", ArgType::Any))
        .flag(f("operationMode", "om", ArgType::Any))
        .flag(f("tipMessage", "tm", ArgType::Any))
        .flag(f("windowTitle", "wt", ArgType::Any)),
    );
    s.register(
        Cmd::new("fontDialog", "Pick a font: returns its name, or an empty string. -FontList returns every font with no dialog; -scalable keeps scalable fonts only.", |_s, a| font_dialog(a))
            .undoable(false)
            .flag(f("FontList", "fl", ArgType::Bool))
            .flag(f("scalable", "sc", ArgType::Bool)),
    );
    s.register(
        Cmd::new("framelessDialog", "A modal message with buttons and no frame: -t the title, -m the message, -pat a path under it, -b each button, -pr the primary ones. Returns the button pressed.", |_s, a| frameless_dialog(a))
            .undoable(false)
            .flag(f("button", "b", ArgType::Any).multi(1))
            .flag(f("message", "m", ArgType::Any))
            .flag(f("parent", "p", ArgType::Any))
            .flag(f("path", "pat", ArgType::Any))
            .flag(f("primary", "pr", ArgType::Any).multi(1))
            .flag(f("title", "t", ArgType::Any)),
    );
}
