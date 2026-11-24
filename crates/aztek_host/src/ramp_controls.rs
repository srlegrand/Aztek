//! gradientControl and gradientControlNoAttr: a ramp of entries (a position from 0 to 1, a colour or a value, and an
//! interpolation: 0 none, 1 linear, 2 smooth, 3 spline) shown as a colour bar or as a curve.
//!
//! gradientControl edits a multi attribute whose elements are compounds of a position, a colour or a value, and an
//! interpolation (remapValue.value, remapValue.color, ramp.colorEntryList): a colour child makes it a colour ramp, a
//! float child a curve. gradientControlNoAttr keeps its ramp in an optionVar, as a string of entries ("value,position,
//! interpolation" for a curve, "r,g,b,position,interpolation" for colours), or in the control alone.
//!
//! The model in `scriptui` holds the flags; `asString` there is the ramp as the shell draws it (the same string
//! format). This module reads and writes where the ramp lives, works out values, and handles what the shell reports.

use aztek_ops::{fail, Args, CmdResult, CommandError};
use aztek_scene::plug::PlugType;
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::core::Core;
use crate::scriptui::{callback_value, command_for, control_full_path, control_kind, control_props, controls_of, model_edit};

pub(crate) const ATTR: &str = "gradientControl";
pub(crate) const NO_ATTR: &str = "gradientControlNoAttr";

pub(crate) const ATTR_FLAGS: &str = "adaptiveScaling:as:b attribute:at:a clearAttribute:cla:b displayKeyInfo:dki:b highlightMode:hlm:a numberOfControls:nc:b readOnly:ro:b \
    refreshOnRelease:ror:a selectedColorControl:scc:a selectedInterpControl:sic:a selectedPositionControl:spc:a staticNumberOfControls:snc:b staticPositions:sp:b \
    upperLimitControl:ulc:a verticalLayout:vl:b";
pub(crate) const NO_ATTR_FLAGS: &str = "addEntry:ae:a5 asString:as:a changeCommand:cc:a currentKey:ck:a currentKeyChanged:ckc:a currentKeyColorValue:clv:a3 \
    currentKeyCurveValue:cvv:a currentKeyInterpValue:civ:a displayKeyInfo:dki:b dragCommand:dc:a highlightMode:hlm:a optionVar:ov:a rampAsColor:rac:b readOnly:ro:b \
    staticNumberOfControls:snc:b valueAtPoint:vap:a";

/// Fired by the shell: the entries changed (asString and currentKey come in `set`); `__rampDrag` while dragging.
const CHANGED: &str = "__rampChanged";
const DRAG: &str = "__rampDrag";
/// Fired by the shell: an entry was picked (currentKey comes in `set`).
const PICKED: &str = "__rampPicked";

pub(crate) fn is_mine(cmd: &str) -> bool {
    matches!(cmd, ATTR | NO_ATTR)
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

fn truth(v: &Value) -> bool {
    match v {
        Value::Bool(b) => *b,
        Value::String(s) => matches!(s.as_str(), "true" | "on" | "yes" | "1"),
        o => number(o) != 0.0,
    }
}

fn flat(v: &Value) -> Vec<Value> {
    match v {
        Value::Array(a) => a.iter().flat_map(flat).collect(),
        o => vec![o.clone()],
    }
}

fn err(m: impl Into<String>) -> CommandError {
    CommandError::new(m.into())
}

fn fmt(f: f64) -> String {
    crate::mel::fmt_float(f)
}

// ---------------------------------------------------------------------------------------------------------------------
// The ramp

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub pos: f64,
    /// A curve's value is the first; a colour ramp uses all three.
    pub value: [f64; 3],
    pub interp: i64,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ramp {
    pub color: bool,
    pub entries: Vec<Entry>,
}

impl Ramp {
    /// "v,p,i,..." or "r,g,b,p,i,...".
    pub fn parse(s: &str, color: bool) -> Ramp {
        let n: Vec<f64> = s.split(',').filter(|t| !t.trim().is_empty()).map(|t| t.trim().parse().unwrap_or(0.0)).collect();
        let step = if color { 5 } else { 3 };
        let entries = n
            .chunks(step)
            .filter(|c| c.len() == step)
            .map(|c| if color { Entry { pos: c[3], value: [c[0], c[1], c[2]], interp: c[4] as i64 } } else { Entry { pos: c[1], value: [c[0]; 3], interp: c[2] as i64 } })
            .collect();
        Ramp { color, entries }
    }

    pub fn to_text(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        for e in &self.entries {
            if self.color {
                out.extend([fmt(e.value[0]), fmt(e.value[1]), fmt(e.value[2])]);
            } else {
                out.push(fmt(e.value[0]));
            }
            out.extend([fmt(e.pos), e.interp.to_string()]);
        }
        out.join(",")
    }

    fn sorted(&self) -> Vec<Entry> {
        let mut e = self.entries.clone();
        e.sort_by(|a, b| a.pos.total_cmp(&b.pos));
        e
    }

    /// The ramp's value at a position: the interpolation of the entry on the left takes it to the next entry.
    pub fn at(&self, t: f64) -> [f64; 3] {
        let e = self.sorted();
        let (Some(first), Some(last)) = (e.first(), e.last()) else { return [0.0; 3] };
        if t <= first.pos {
            return first.value;
        }
        if t >= last.pos {
            return last.value;
        }
        let k = e.iter().rposition(|x| x.pos <= t).unwrap_or(0);
        let (a, b) = (e[k], e[(k + 1).min(e.len() - 1)]);
        let span = (b.pos - a.pos).max(1e-12);
        let u = ((t - a.pos) / span).clamp(0.0, 1.0);
        let mut out = [0.0; 3];
        for c in 0..3 {
            let (p0, p1) = (a.value[c], b.value[c]);
            out[c] = match a.interp {
                0 => p0,
                2 => p0 + (p1 - p0) * u * u * (3.0 - 2.0 * u),
                3 => {
                    // Catmull-Rom through the neighbours, the ends held.
                    let pm = if k > 0 { e[k - 1].value[c] } else { p0 };
                    let pn = if k + 2 < e.len() { e[k + 2].value[c] } else { p1 };
                    let (u2, u3) = (u * u, u * u * u);
                    0.5 * (2.0 * p0 +(-pm + p1) * u + (2.0 * pm - 5.0 * p0 + 4.0 * p1 - pn) * u2 + (-pm + 3.0 * p0 - 3.0 * p1 + pn) * u3)
                }
                _ => p0 + (p1 - p0) * u,
            };
        }
        out
    }

    /// Adds an entry and gives its index once sorted.
    fn add(&mut self, e: Entry) -> usize {
        self.entries.push(e);
        self.resort(self.entries.len() - 1)
    }

    /// Sorts by position and gives where the entry that was at `keep` went.
    fn resort(&mut self, keep: usize) -> usize {
        let mut t: Vec<(usize, Entry)> = self.entries.iter().copied().enumerate().collect();
        t.sort_by(|a, b| a.1.pos.total_cmp(&b.1.pos));
        let at = t.iter().position(|(i, _)| *i == keep).unwrap_or(0);
        self.entries = t.into_iter().map(|(_, e)| e).collect();
        at
    }
}

fn default_ramp(color: bool) -> Ramp {
    if color {
        Ramp { color, entries: vec![Entry { pos: 0.0, value: [0.0; 3], interp: 1 }, Entry { pos: 1.0, value: [1.0; 3], interp: 1 }] }
    } else {
        Ramp { color, entries: vec![Entry { pos: 0.0, value: [0.0; 3], interp: 1 }, Entry { pos: 1.0, value: [1.0; 3], interp: 1 }] }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Where gradientControl's ramp lives: a multi of compounds

struct RampPlug {
    plug: String,
    pos: String,
    val: String,
    interp: Option<String>,
    color: bool,
}

fn ramp_plug(s: &mut Session, at: &str) -> Result<RampPlug, String> {
    let mut plug = at.trim().to_string();
    if plug.ends_with(']') {
        if let Some(i) = plug.rfind('[') {
            plug.truncate(i);
        }
    }
    let bad = || format!("gradientControl: {plug} is not a multi of compounds holding a position, a colour or value and an interpolation.");
    let Some((node, attr)) = plug.split_once('.') else { return Err(bad()) };
    let kids = s.run("attributeQuery", json!({"args": [attr], "node": node, "listChildren": true})).map_err(|e| e.to_string())?;
    let kids: Vec<String> = flat(&kids).iter().map(text).filter(|k| !k.is_empty()).collect();
    let (mut pos, mut val, mut interp, mut color) = (None, None, None, false);
    for k in &kids {
        let Ok(p) = s.graph.plug(&format!("{plug}[0].{k}")) else { continue };
        let d = s.graph.attr(&p);
        if !d.is_array() {
            return Err(bad());
        }
        let lower = k.to_lowercase();
        match d.ty {
            PlugType::Enum if interp.is_none() => interp = Some(k.clone()),
            PlugType::Vec3 if val.is_none() => {
                val = Some(k.clone());
                color = true;
            }
            PlugType::Float if pos.is_none() && lower.contains("position") => pos = Some(k.clone()),
            PlugType::Float if val.is_none() => val = Some(k.clone()),
            PlugType::Int if interp.is_none() && lower.contains("interp") => interp = Some(k.clone()),
            _ => {}
        }
    }
    match (pos, val) {
        (Some(pos), Some(val)) => Ok(RampPlug { plug, pos, val, interp, color }),
        _ => Err(bad()),
    }
}

fn indices(s: &mut Session, plug: &str) -> Vec<i64> {
    let v = s.run("getAttr", json!({"args": [plug], "multiIndices": true})).unwrap_or(Value::Null);
    let mut i: Vec<i64> = flat(&v).iter().filter(|x| !x.is_null()).map(|x| number(x) as i64).collect();
    i.sort();
    i.dedup();
    i
}

fn read_elements(s: &mut Session, rp: &RampPlug) -> Vec<(i64, Entry)> {
    let mut out = Vec::new();
    for i in indices(s, &rp.plug) {
        let g = |s: &mut Session, c: &str| -> Vec<f64> { s.run("getAttr", json!({"args": [format!("{}[{i}].{c}", rp.plug)]})).ok().map(|v| flat(&v).iter().map(number).collect()).unwrap_or_default() };
        let p = g(s, &rp.pos).first().copied().unwrap_or(0.0);
        let v = g(s, &rp.val);
        let value = if rp.color { [v.first().copied().unwrap_or(0.0), v.get(1).copied().unwrap_or(0.0), v.get(2).copied().unwrap_or(0.0)] } else { [v.first().copied().unwrap_or(0.0); 3] };
        let interp = match &rp.interp {
            Some(c) => g(s, c).first().copied().unwrap_or(1.0) as i64,
            None => 1,
        };
        out.push((i, Entry { pos: p, value, interp }));
    }
    out
}

fn read_attr(s: &mut Session, rp: &RampPlug) -> Ramp {
    let mut r = Ramp { color: rp.color, entries: read_elements(s, rp).into_iter().map(|(_, e)| e).collect() };
    r.entries.sort_by(|a, b| a.pos.total_cmp(&b.pos));
    r
}

/// Writes the entries into the multi: entries already there keep their elements, changed ones take the elements left,
/// new ones go after the last, and elements no longer needed are removed.
fn write_attr(s: &mut Session, rp: &RampPlug, r: &Ramp) -> Result<(), String> {
    let had = read_elements(s, rp);
    let same = |a: &Entry, b: &Entry| (a.pos - b.pos).abs() < 1e-9 && (0..3).all(|c| (a.value[c] - b.value[c]).abs() < 1e-9) && (rp.interp.is_none() || a.interp == b.interp);
    let mut used = vec![false; had.len()];
    let mut todo: Vec<Entry> = Vec::new();
    for e in &r.entries {
        match had.iter().enumerate().position(|(n, h)| !used[n] && same(&h.1, e)) {
            Some(n) => used[n] = true,
            None => todo.push(*e),
        }
    }
    let mut free: Vec<i64> = had.iter().enumerate().filter(|(n, _)| !used[*n]).map(|(_, e)| e.0).collect();
    free.reverse();
    let mut next = had.iter().map(|e| e.0).max().map(|m| m + 1).unwrap_or(0);
    for e in todo {
        let i = free.pop().unwrap_or_else(|| {
            next += 1;
            next - 1
        });
        let at = |c: &str| format!("{}[{i}].{c}", rp.plug);
        s.run("setAttr", json!({"args": [at(&rp.pos), e.pos]})).map_err(|e| e.to_string())?;
        if rp.color {
            s.run("setAttr", json!({"args": [at(&rp.val), e.value[0], e.value[1], e.value[2]], "type": "double3"})).map_err(|e| e.to_string())?;
        } else {
            s.run("setAttr", json!({"args": [at(&rp.val), e.value[0]]})).map_err(|e| e.to_string())?;
        }
        if let Some(c) = &rp.interp {
            s.run("setAttr", json!({"args": [at(c), e.interp]})).map_err(|e| e.to_string())?;
        }
    }
    for i in free {
        s.run("removeMultiInstance", json!({"args": [format!("{}[{i}]", rp.plug)], "break": true})).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Reading and writing a control's ramp

fn is_color(cmd: &str, props: &Map<String, Value>) -> bool {
    if cmd == NO_ATTR {
        props.get("rampAsColor").is_some_and(truth)
    } else {
        props.get("__color").is_some_and(truth)
    }
}

/// The ramp where it lives now: the attribute, the optionVar, or the control.
fn stored(s: &mut Session, cmd: &str, props: &Map<String, Value>) -> Option<Ramp> {
    if let Some(at) = props.get("attribute").map(text).filter(|t| !t.is_empty()) {
        let rp = ramp_plug(s, &at).ok()?;
        return Some(read_attr(s, &rp));
    }
    let color = is_color(cmd, props);
    if let Some(ov) = props.get("optionVar").map(text).filter(|t| !t.is_empty()) {
        if let Some(v) = s.option_vars.get(&ov) {
            let t = flat(v).iter().map(text).collect::<Vec<_>>().join(",");
            return Some(Ramp::parse(&t, color));
        }
    }
    None
}

fn write_back(s: &mut Session, props: &Map<String, Value>, r: &Ramp) -> Result<(), String> {
    if let Some(at) = props.get("attribute").map(text).filter(|t| !t.is_empty()) {
        let rp = ramp_plug(s, &at)?;
        return write_attr(s, &rp, r);
    }
    if let Some(ov) = props.get("optionVar").map(text).filter(|t| !t.is_empty()) {
        s.option_vars.insert(ov, json!(r.to_text()));
    }
    Ok(())
}

fn named(cmd: &str, a: &Args) -> Result<String, CommandError> {
    let Some(n) = a.pos_str(0) else { return Err(err(format!("{cmd}: name a control."))) };
    control_full_path(n).filter(|p| control_kind(p).as_deref() == Some(cmd)).ok_or_else(|| err(format!("{cmd}: Object '{n}' not found.")))
}

fn shown(cmd: &str, props: &Map<String, Value>) -> Ramp {
    Ramp::parse(&props.get("asString").map(text).unwrap_or_default(), is_color(cmd, props))
}

/// The ramp a control shows, read again from where it lives.
fn refresh(s: &mut Session, cmd: &str, path: &str) {
    let Some(props) = control_props(path) else { return };
    let Some(r) = stored(s, cmd, &props) else { return };
    let now = r.to_text();
    if props.get("asString").map(text).as_deref() != Some(now.as_str()) {
        let key = props.get("currentKey").map(|v| number(v) as i64).unwrap_or(0);
        let mut e = Map::new();
        e.insert("asString".into(), json!(now));
        e.insert("currentKey".into(), json!(key.clamp(0, (r.entries.len() as i64 - 1).max(0))));
        let _ = model_edit(path, &e);
        show_selected(path);
    }
}

/// The controls -selectedPositionControl, -selectedColorControl and -selectedInterpControl name show the current entry;
/// -upperLimitControl the top of the scale.
fn show_selected(path: &str) {
    let Some(p) = control_props(path) else { return };
    let cmd = control_kind(path).unwrap_or_default();
    let r = shown(&cmd, &p);
    let key = p.get("currentKey").map(|v| number(v) as usize).unwrap_or(0);
    let edit = |flag: &str, k: &str, v: Value| {
        if let Some(f) = p.get(flag).map(text).filter(|t| !t.is_empty()).and_then(|n| control_full_path(&n)) {
            let _ = model_edit(&f, &Map::from_iter([(k.to_string(), v)]));
        }
    };
    if let Some(e) = r.entries.get(key) {
        edit("selectedPositionControl", "value", json!(e.pos));
        if r.color {
            edit("selectedColorControl", "rgbValue", json!(e.value));
        } else {
            edit("selectedColorControl", "value", json!(e.value[0]));
        }
        // An optionMenu's items count from 1.
        let interp_ctl = p.get("selectedInterpControl").map(text).filter(|t| !t.is_empty()).and_then(|n| control_full_path(&n));
        if let Some(f) = interp_ctl {
            let k = if control_kind(&f).is_some_and(|k| k.starts_with("optionMenu") || k.starts_with("attrEnumOptionMenu")) { "select" } else { "value" };
            let v = if k == "select" { json!(e.interp + 1) } else { json!(e.interp) };
            let _ = model_edit(&f, &Map::from_iter([(k.to_string(), v)]));
        }
    }
    if p.get("adaptiveScaling").is_some_and(truth) {
        let top = r.entries.iter().flat_map(|e| e.value).fold(1.0f64, f64::max);
        edit("upperLimitControl", "text", json!(fmt(top.ceil())));
    }
}

pub(crate) fn command(s: &mut Session, cmd: &str, a: &Args) -> CmdResult {
    if a.has("defineTemplate") || (a.bool("exists") && !a.bool("edit") && !a.bool("query")) {
        return command_for(cmd, a);
    }
    if a.bool("query") {
        return query(s, cmd, a);
    }
    let edit = a.bool("edit");
    let (path, mut props) = if edit {
        let p = named(cmd, a)?;
        let props = control_props(&p).unwrap_or_default();
        (Some(p), props)
    } else {
        (None, Map::new())
    };
    if a.bool("clearAttribute") {
        if !edit {
            fail!("{cmd}: -clearAttribute is for -edit.");
        }
        props.remove("attribute");
    }
    for k in ["attribute", "optionVar", "rampAsColor"] {
        if let Some(v) = a.get(k) {
            props.insert(k.to_string(), v.clone());
        }
    }
    let mut color = cmd == NO_ATTR && props.get("rampAsColor").is_some_and(truth);
    if let Some(at) = a.get("attribute").map(text) {
        let rp = ramp_plug(s, &at).map_err(err)?;
        color = rp.color;
    } else if cmd == ATTR {
        color = props.get("__color").is_some_and(truth) && props.contains_key("attribute");
    }
    props.insert("__color".into(), json!(color));
    if let Some(v) = a.get("highlightMode") {
        let t = text(v);
        if !matches!(t.as_str(), "off" | "hover" | "always") {
            fail!("{cmd}: -highlightMode is off, hover or always, not \"{t}\".");
        }
    }
    let mut r = match props.get("asString") {
        Some(v) if !a.has("rampAsColor") => Ramp::parse(&text(v), color),
        _ => default_ramp(color),
    };
    // A store named now gives the ramp when it holds one, and is given the ramp when it doesn't.
    if a.has("optionVar") || a.has("attribute") {
        if let Some(st) = stored(s, cmd, &props) {
            if !st.entries.is_empty() || a.has("attribute") {
                r = st;
            }
        }
    }
    r.color = color;
    if let Some(v) = a.get("asString") {
        r = Ramp::parse(&text(v), color);
    }
    let mut key = a.get("currentKey").map(|v| number(v) as i64).unwrap_or_else(|| props.get("currentKey").map(|v| number(v) as i64).unwrap_or(0));
    if let Some(v) = a.get("addEntry") {
        let n: Vec<f64> = flat(v).iter().map(number).collect();
        if n.len() < 5 {
            fail!("{cmd}: -addEntry takes a position, a red, green and blue, and an interpolation.");
        }
        let interp = n[4] as i64;
        if !(0..=3).contains(&interp) {
            fail!("{cmd}: -addEntry's interpolation is 0 to 3, not {interp}.");
        }
        let value = if color { [n[1], n[2], n[3]] } else { [(n[1] + n[2] + n[3]) / 3.0; 3] };
        key = r.add(Entry { pos: n[0].clamp(0.0, 1.0), value, interp }) as i64;
    }
    if a.has("currentKey") && (key < 0 || key as usize >= r.entries.len()) {
        fail!("{cmd}: -currentKey {key}: the ramp has {} entries.", r.entries.len());
    }
    let k = key.clamp(0, (r.entries.len() as i64 - 1).max(0)) as usize;
    if let Some(v) = a.get("currentKeyColorValue") {
        let c: Vec<f64> = flat(v).iter().map(number).collect();
        if c.len() < 3 {
            fail!("{cmd}: -currentKeyColorValue takes a red, a green and a blue.");
        }
        if !color {
            fail!("{cmd}: -currentKeyColorValue is for colour ramps.");
        }
        if let Some(e) = r.entries.get_mut(k) {
            e.value = [c[0], c[1], c[2]];
        }
    }
    if let Some(v) = a.get("currentKeyCurveValue") {
        if color {
            fail!("{cmd}: -currentKeyCurveValue is for curve ramps.");
        }
        if let Some(e) = r.entries.get_mut(k) {
            e.value = [number(v); 3];
        }
    }
    if let Some(v) = a.get("currentKeyInterpValue") {
        let i = number(v) as i64;
        if !(0..=3).contains(&i) {
            fail!("{cmd}: -currentKeyInterpValue is 0 to 3, not {i}.");
        }
        if let Some(e) = r.entries.get_mut(k) {
            e.interp = i;
        }
    }
    let mut plain = a.clone();
    for f in ["addEntry", "currentKeyColorValue", "currentKeyCurveValue", "currentKeyInterpValue", "clearAttribute"] {
        plain.flags.remove(f);
    }
    if a.bool("clearAttribute") {
        plain.flags.insert("attribute".into(), json!(""));
    }
    if let Some(v) = a.get("currentKeyChanged") {
        plain.flags.insert("currentKeyChanged".into(), callback_value(v));
    }
    plain.flags.insert("asString".into(), json!(r.to_text()));
    plain.flags.insert("currentKey".into(), json!(k as i64));
    plain.flags.insert("__color".into(), json!(color));
    let before_key = props.get("currentKey").map(|v| number(v) as i64);
    let res = command_for(cmd, &plain)?;
    let path = path.unwrap_or_else(|| res.as_str().unwrap_or("").to_string());
    let props = control_props(&path).unwrap_or_default();
    // Clearing the attribute leaves the control with nothing to write to.
    if !a.bool("clearAttribute") {
        write_back(s, &props, &r).map_err(err)?;
    }
    show_selected(&path);
    if edit && before_key.is_some_and(|b| b != k as i64) {
        if let Some(cb) = props.get("currentKeyChanged").cloned() {
            crate::scriptui::run_soon(cb, Vec::new());
        }
    }
    Ok(res)
}

fn query(s: &mut Session, cmd: &str, a: &Args) -> CmdResult {
    let path = named(cmd, a)?;
    refresh(s, cmd, &path);
    let props = control_props(&path).unwrap_or_default();
    let r = shown(cmd, &props);
    let key = props.get("currentKey").map(|v| number(v) as usize).unwrap_or(0);
    let asked = a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| {
        a.flags.iter().find(|(k, v)| !matches!(k.as_str(), "query" | "queried") && !v.is_null() && *v != &json!(false)).map(|(k, _)| k.clone())
    });
    let b = |k: &str| json!(props.get(k).is_some_and(truth));
    match asked.as_deref() {
        Some("asString") => Ok(json!(r.to_text())),
        Some("numberOfControls") => Ok(json!(r.entries.len())),
        Some("currentKey") => Ok(json!(if r.entries.is_empty() { -1 } else { key as i64 })),
        Some("currentKeyColorValue") => Ok(match r.entries.get(key) {
            Some(e) if r.color => json!(e.value),
            Some(_) => fail!("{cmd}: -currentKeyColorValue is for colour ramps."),
            None => json!([]),
        }),
        Some("currentKeyCurveValue") => Ok(match r.entries.get(key) {
            Some(e) if !r.color => json!(e.value[0]),
            Some(_) => fail!("{cmd}: -currentKeyCurveValue is for curve ramps."),
            None => json!(0.0),
        }),
        Some("currentKeyInterpValue") => Ok(json!(r.entries.get(key).map(|e| e.interp).unwrap_or(0))),
        Some("valueAtPoint") => {
            let Some(t) = a.get("valueAtPoint").filter(|v| !v.is_boolean()).map(number) else { fail!("{cmd}: -valueAtPoint takes the position to read.") };
            let v = r.at(t);
            Ok(if r.color { json!(v) } else { json!(v[0]) })
        }
        Some("rampAsColor") => Ok(json!(r.color)),
        Some(k @ ("adaptiveScaling" | "displayKeyInfo" | "readOnly" | "staticNumberOfControls" | "staticPositions" | "verticalLayout")) => Ok(b(k)),
        Some("refreshOnRelease") => Ok(json!(props.get("refreshOnRelease").map(|v| number(v) as i64).unwrap_or(0))),
        Some("highlightMode") => Ok(json!(props.get("highlightMode").map(text).unwrap_or_else(|| "off".into()))),
        Some(k @ ("optionVar" | "upperLimitControl" | "attribute" | "selectedColorControl" | "selectedInterpControl" | "selectedPositionControl")) => Ok(json!(props.get(k).map(text).unwrap_or_default())),
        _ => command_for(cmd, a),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// What the shell reports, and keeping up with the scene

pub(crate) fn shell_changed(core: &mut Core, ev: &Value) {
    let Some(path) = ev["path"].as_str().and_then(control_full_path) else { return };
    let fired: Vec<String> = ev["fire"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    let kind = control_kind(&path).unwrap_or_default();
    if is_mine(&kind) {
        let props = control_props(&path).unwrap_or_default();
        let r = shown(&kind, &props);
        let changed = fired.iter().any(|f| f == CHANGED);
        let drag = fired.iter().any(|f| f == DRAG);
        if changed || drag {
            if let Err(e) = write_back(&mut core.s, &props, &r) {
                core.push(format!("// Error: {e} //"), crate::core::LineKind::Error);
            }
            show_selected(&path);
            let cb = if drag { props.get("dragCommand") } else { props.get("changeCommand") };
            if let Some(cb) = cb.cloned() {
                core.run_callback(&cb, &[]);
            }
        }
        if fired.iter().any(|f| f == PICKED) || ev["set"].get("currentKey").is_some() {
            show_selected(&path);
            if fired.iter().any(|f| f == PICKED) {
                if let Some(cb) = props.get("currentKeyChanged").cloned() {
                    core.run_callback(&cb, &[]);
                }
            }
        }
        return;
    }
    // A control a ramp names to edit its current entry: changing it changes the entry.
    let set = &ev["set"];
    let (pos, val, interp) = (set.get("value").map(number), set.get("rgbValue").map(|v| flat(v).iter().map(number).collect::<Vec<f64>>()), set.get("select").map(number));
    if pos.is_none() && val.is_none() && interp.is_none() {
        return;
    }
    for (ramp, props) in controls_of(ATTR) {
        let points = |flag: &str| props.get(flag).map(text).filter(|t| !t.is_empty()).and_then(|n| control_full_path(&n)).as_deref() == Some(path.as_str());
        let mut r = shown(ATTR, &props);
        let k = props.get("currentKey").map(|v| number(v) as usize).unwrap_or(0);
        if k >= r.entries.len() {
            continue;
        }
        let mut key = k;
        let mut hit = false;
        if points("selectedPositionControl") {
            if let Some(p) = pos {
                r.entries[k].pos = p.clamp(0.0, 1.0);
                key = r.resort(k);
                hit = true;
            }
        }
        if points("selectedColorControl") {
            if let Some(c) = &val {
                if c.len() >= 3 {
                    r.entries[k].value = [c[0], c[1], c[2]];
                    hit = true;
                }
            } else if let Some(v) = pos {
                r.entries[k].value = [v; 3];
                hit = true;
            }
        }
        if points("selectedInterpControl") {
            if let Some(i) = interp.map(|i| i as i64 - 1).or(pos.map(|v| v as i64)) {
                r.entries[k].interp = i.clamp(0, 3);
                hit = true;
            }
        }
        if !hit {
            continue;
        }
        let mut e = Map::new();
        e.insert("asString".into(), json!(r.to_text()));
        e.insert("currentKey".into(), json!(key));
        let _ = model_edit(&ramp, &e);
        let _ = write_back(&mut core.s, &props, &r);
        show_selected(&ramp);
    }
}

/// Idle: ramps follow their attribute or optionVar.
pub(crate) fn idle(core: &mut Core) {
    for kind in [ATTR, NO_ATTR] {
        for (path, _) in controls_of(kind) {
            refresh(&mut core.s, kind, &path);
        }
    }
}

/// The value of a ramp control at a position, for tests.
pub fn value_at(path: &str, t: f64) -> Option<Vec<f64>> {
    let kind = control_kind(path)?;
    let props = control_props(path)?;
    let r = shown(&kind, &props);
    let v = r.at(t);
    Some(if r.color { v.to_vec() } else { vec![v[0]] })
}
