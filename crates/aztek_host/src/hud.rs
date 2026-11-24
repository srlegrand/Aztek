//! headsUpDisplay: text blocks drawn over the views, in ten sections (five across the top, five across the bottom),
//! each section a column of blocks. A block shows a label and what its command returned the last time a trigger
//! (an event, a condition, an attribute or connection change, every refresh) ran it, or one of the presets.
//!
//! The records live with hudButton, hudSlider and hudSliderButton's in `ui_cmds`, so -remove, -exists, -lh and the
//! block checks see all of them. Triggers are script jobs no listing shows; what they ask for is run by the core at
//! idle (`idle`), which also watches the selection for -nodeChanges and removes blocks whose watched node went.
//! The views draw what `blocks` gives.

use std::collections::BTreeMap;

use aztek_ops::{fail, Args, CmdResult, CommandDef};
use aztek_scene::Session;
use serde_json::{json, Map, Value};

use crate::core::Core;
use crate::ui_cmds::with_ui;

type Cmd = CommandDef<Session>;

/// The layout's own state.
pub(crate) struct Layout {
    last_id: i64,
    /// -layoutVisibility: the whole layout on or off.
    pub(crate) visible: bool,
    /// -showGrid and -gridColor.
    pub(crate) grid: bool,
    pub(crate) grid_color: i64,
    /// -setOption: name to value.
    options: BTreeMap<String, String>,
}

impl Default for Layout {
    fn default() -> Self {
        Layout { last_id: 0, visible: true, grid: false, grid_color: 1, options: BTreeMap::from([("smpPolyCount".to_string(), "cage".to_string())]) }
    }
}

impl Layout {
    pub(crate) fn next_id(&mut self) -> i64 {
        self.last_id += 1;
        self.last_id
    }
}

pub const PRESETS: &[&str] = &["cameraNames", "polyVerts", "polyEdges", "polyFaces", "polyUVs", "polyTriangles", "materialLoadingCount", "textureLoadingCount", "frameRate", "viewAxis", "distanceFromCamera"];
pub const NODE_CHANGES: &[&str] = &["attributeChange", "connectionChange", "instanceChange"];

/// The flags that set what runs the command.
const TRIGGERS: &[&str] = &["event", "conditionChange", "conditionTrue", "conditionFalse", "attributeChange", "connectionChange", "attachToRefresh"];

/// The formatting flags, kept as given and read back by -q.
const KEPT: &[&str] = &[
    "section", "block", "blockSize", "blockAlignment", "label", "labelFontSize", "labelWidth", "dataAlignment", "dataFontSize", "dataWidth", "decimalPrecision", "padding", "allowOverlap",
    "visible", "preset", "command",
];

const FLAGS: &str = "allDescendants:ad:b allowOverlap:ao:a attachToRefresh:atr:b attributeChange:ac:a block:b:a blockAlignment:ba:a blockSize:bs:a command:c:a conditionChange:cc:a \
    conditionFalse:cf:a conditionTrue:ct:a connectionChange:con:a dataAlignment:da:a dataFontSize:dfs:a dataWidth:dw:a decimalPrecision:dp:a disregardIndex:di:b event:ev:a \
    exists:ex:b getOption:op:a gridColor:gco:a label:l:a labelFontSize:lfs:a labelWidth:lw:a lastOccupiedBlock:lob:a layoutVisibility:lv:a listConditions:lc:b listEvents:le:b \
    listHeadsUpDisplays:lh:b listNodeChanges:lnc:b listPresets:lp:b name:n:a nextFreeBlock:nfb:a nodeChanges:nc:a1 padding:p:a preset:pre:a refresh:r:b remove:rem:a \
    removeID:rid:a removePosition:rp:a2 resetNodeChanges:rnc:a1 scriptResult:sr:b section:s:a setOption:so:a2 showGrid:sg:b visible:vis:a query:q:b edit:e:b queried::a";

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

/// The value a flag of a record reads as when it was never given.
fn default_of(flag: &str) -> Value {
    match flag {
        "section" | "block" => json!(0),
        "blockSize" | "labelFontSize" | "dataFontSize" => json!("small"),
        "blockAlignment" | "dataAlignment" => json!("left"),
        "padding" => json!(15),
        "decimalPrecision" => json!(2),
        "labelWidth" | "dataWidth" => json!(0),
        "visible" => json!(true),
        "allowOverlap" | "attachToRefresh" => json!(false),
        "nodeChanges" => json!([]),
        _ => json!(""),
    }
}

/// The blocks in use in a section, by every kind of record.
fn occupied(huds: &BTreeMap<String, Value>, section: i64, except: Option<&str>) -> Vec<i64> {
    let mut b: Vec<i64> = huds
        .iter()
        .filter(|(n, _)| Some(n.as_str()) != except)
        .filter(|(_, h)| h.get("section").map(number).unwrap_or(0.0) as i64 == section)
        .map(|(_, h)| h.get("block").map(number).unwrap_or(0.0) as i64)
        .collect();
    b.sort();
    b.dedup();
    b
}

fn check_choice(flag: &str, v: &Value, ok: &[&str]) -> Result<(), aztek_ops::CommandError> {
    let t = text(v);
    if !ok.contains(&t.as_str()) {
        return Err(aztek_ops::CommandError::new(format!("headsUpDisplay: -{flag} is one of {}, not \"{t}\".", ok.join(", "))));
    }
    Ok(())
}

/// Checks the values of the formatting flags given.
fn check_flags(a: &Args) -> Result<(), aztek_ops::CommandError> {
    for (flag, ok) in [("blockSize", &["small", "medium", "large"][..]), ("blockAlignment", &["left", "center", "right"]), ("dataAlignment", &["left", "right"]), ("labelFontSize", &["small", "large"]), ("dataFontSize", &["small", "large"])] {
        if let Some(v) = a.get(flag) {
            check_choice(flag, v, ok)?;
        }
    }
    if let Some(v) = a.get("section") {
        let n = number(v) as i64;
        if !(0..=9).contains(&n) {
            fail!("headsUpDisplay: -section is 0 to 9, not {n}.");
        }
    }
    if let Some(v) = a.get("block") {
        if (number(v) as i64) < 0 {
            fail!("headsUpDisplay: -block is 0 or more.");
        }
    }
    if let Some(v) = a.get("decimalPrecision") {
        let n = number(v) as i64;
        if !(1..=8).contains(&n) {
            fail!("headsUpDisplay: -decimalPrecision is 1 to 8, not {n}.");
        }
    }
    if let Some(v) = a.get("gridColor") {
        let n = number(v) as i64;
        if !(1..=23).contains(&n) {
            fail!("headsUpDisplay: -gridColor is a colour index from 1 to 23, not {n}.");
        }
    }
    if let Some(v) = a.get("preset") {
        check_choice("preset", v, PRESETS)?;
    }
    for v in a.get("nodeChanges").map(flat).unwrap_or_default().iter().chain(a.get("resetNodeChanges").map(flat).unwrap_or_default().iter()) {
        check_choice("nodeChanges", v, NODE_CHANGES)?;
    }
    if let Some(e) = a.get("event") {
        let e = text(e);
        if !crate::script_jobs::EVENTS.contains(&e.as_str()) {
            fail!("headsUpDisplay: Invalid event name '{e}'.");
        }
    }
    for flag in ["conditionChange", "conditionTrue", "conditionFalse"] {
        if let Some(c) = a.get(flag) {
            let c = text(c);
            if !crate::script_jobs::CONDITIONS.contains(&c.as_str()) {
                fail!("headsUpDisplay: Invalid condition name '{c}'.");
            }
        }
    }
    Ok(())
}

/// The record's watches: hidden script jobs that refresh it. Old ones end first.
fn rewatch(s: &mut Session, name: &str) -> Result<(), aztek_ops::CommandError> {
    let Some(h) = with_ui(|u| u.huds.get(name).cloned()) else { return Ok(()) };
    for id in h.get("__jobs").map(flat).unwrap_or_default() {
        crate::script_jobs::kill_hidden(number(&id) as i64);
    }
    let script = format!("headsUpDisplay -refresh \"{}\"", name.replace('\\', "\\\\").replace('"', "\\\""));
    let mut jobs = Vec::new();
    for (flag, job_flag) in [("event", "event"), ("conditionChange", "conditionChange"), ("conditionTrue", "conditionTrue"), ("conditionFalse", "conditionFalse"), ("attributeChange", "attributeChange"), ("connectionChange", "connectionChange")] {
        let Some(what) = h.get(flag).map(text).filter(|t| !t.is_empty()) else { continue };
        let mut call = Map::new();
        call.insert(job_flag.into(), json!([what, script]));
        if flag == "attributeChange" {
            if h.get("disregardIndex").is_some_and(truth) {
                call.insert("disregardIndex".into(), json!(true));
            }
            if h.get("allDescendants").is_some_and(truth) {
                call.insert("allChildren".into(), json!(true));
            }
        }
        let id = crate::script_jobs::start_hidden(s, Value::Object(call)).map_err(aztek_ops::CommandError::new)?;
        jobs.push(json!(id));
    }
    with_ui(|u| {
        if let Some(h) = u.huds.get_mut(name) {
            h["__jobs"] = Value::Array(jobs);
        }
    });
    Ok(())
}

fn drop_record(name: &str) -> Option<Value> {
    let h = with_ui(|u| u.huds.remove(name))?;
    for id in h.get("__jobs").map(flat).unwrap_or_default() {
        crate::script_jobs::kill_hidden(number(&id) as i64);
    }
    Some(h)
}

/// The flag a query asks about.
fn asked(a: &Args) -> Option<String> {
    a.get("queried").and_then(Value::as_array).and_then(|q| q.first()).and_then(Value::as_str).map(str::to_string).or_else(|| {
        a.flags.iter().find(|(k, v)| !matches!(k.as_str(), "query" | "queried" | "edit") && !v.is_null() && *v != &json!(false)).map(|(k, _)| k.clone())
    })
}

fn query(a: &Args, name: Option<String>) -> CmdResult {
    let Some(q) = asked(a) else { fail!("headsUpDisplay: name what to ask about.") };
    match q.as_str() {
        "layoutVisibility" => return Ok(json!(with_ui(|u| u.hud.visible))),
        "showGrid" => return Ok(json!(with_ui(|u| u.hud.grid))),
        "gridColor" if name.is_none() => return Ok(json!(with_ui(|u| u.hud.grid_color))),
        "listHeadsUpDisplays" => return Ok(json!(with_ui(|u| u.huds.keys().cloned().collect::<Vec<_>>()))),
        "listPresets" => return Ok(json!(PRESETS)),
        "listEvents" => return Ok(json!(crate::script_jobs::EVENTS)),
        "listConditions" => return Ok(json!(crate::script_jobs::CONDITIONS)),
        "listNodeChanges" => return Ok(json!(NODE_CHANGES)),
        "getOption" => {
            let Some(o) = a.get("getOption").map(text).filter(|t| !t.is_empty() && t != "true") else { fail!("headsUpDisplay: -getOption names the option.") };
            return with_ui(|u| match u.hud.options.get(&o) {
                Some(v) => Ok(json!(v)),
                None => fail!("headsUpDisplay: no option named {o}."),
            });
        }
        "exists" => return Ok(json!(name.is_some_and(|n| with_ui(|u| u.huds.contains_key(&n))))),
        _ => {}
    }
    let Some(n) = name else { fail!("headsUpDisplay: name the heads up display.") };
    with_ui(|u| {
        let Some(h) = u.huds.get(&n) else { fail!("headsUpDisplay: Object '{n}' not found.") };
        Ok(match q.as_str() {
            "scriptResult" => h.get("__result").cloned().unwrap_or(json!("")),
            "gridColor" => json!(u.hud.grid_color),
            "nodeChanges" => {
                // Each change watched, and whether it has happened since it was last reset.
                let seen = h.get("__changed").cloned().unwrap_or(json!({}));
                let mut out = Vec::new();
                for c in h.get("nodeChanges").map(flat).unwrap_or_default() {
                    let c = text(&c);
                    out.push(json!(c.clone()));
                    out.push(json!(if seen.get(&c).is_some_and(truth) { "1" } else { "0" }));
                }
                json!(out)
            }
            "attachToRefresh" => json!(h.get("attachToRefresh").is_some_and(truth)),
            "allowOverlap" | "visible" => json!(h.get(&q).map(truth).unwrap_or_else(|| truth(&default_of(&q)))),
            "section" | "block" | "padding" | "labelWidth" | "dataWidth" | "decimalPrecision" => json!(h.get(&q).map(|v| number(v) as i64).unwrap_or_else(|| number(&default_of(&q)) as i64)),
            "command" => h.get("command").map(|c| if c.is_string() { c.clone() } else { json!("") }).unwrap_or(json!("")),
            _ => h.get(&q).cloned().unwrap_or_else(|| default_of(&q)),
        })
    })
}

fn remove_flags(a: &Args) -> CmdResult<Option<Value>> {
    if let Some(v) = a.get("remove").filter(|v| !v.is_null()) {
        let n = if v.is_boolean() { a.pos_str(0).map(str::to_string).unwrap_or_default() } else { text(v) };
        let Some(h) = drop_record(&n) else { fail!("headsUpDisplay: Object '{n}' not found.") };
        return Ok(Some(json!(h.get("id").map(|v| number(v) as i64).unwrap_or(0))));
    }
    if let Some(v) = a.get("removeID") {
        let id = number(v) as i64;
        let n = with_ui(|u| u.huds.iter().find(|(_, h)| h.get("id").is_some_and(|x| number(x) as i64 == id)).map(|(n, _)| n.clone()));
        let Some(n) = n else { fail!("headsUpDisplay: no heads up display has the ID {id}.") };
        drop_record(&n);
        return Ok(Some(json!(n)));
    }
    if let Some(v) = a.get("removePosition") {
        let p = flat(v);
        if p.len() < 2 {
            fail!("headsUpDisplay: -removePosition takes a section and a block.");
        }
        let (sec, blk) = (number(&p[0]) as i64, number(&p[1]) as i64);
        let n = with_ui(|u| {
            u.huds.iter().find(|(_, h)| h.get("section").map(number).unwrap_or(0.0) as i64 == sec && h.get("block").map(number).unwrap_or(0.0) as i64 == blk).map(|(n, _)| n.clone())
        });
        if let Some(n) = n {
            drop_record(&n);
        }
        return Ok(Some(json!([sec, blk])));
    }
    Ok(None)
}

fn command(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).map(str::to_string);
    if a.bool("query") {
        return query(a, name);
    }
    if let Some(r) = remove_flags(a)? {
        return Ok(r);
    }
    // Lists and layout-wide settings, alone.
    for (flag, list) in [("listHeadsUpDisplays", None), ("listPresets", Some(PRESETS)), ("listEvents", Some(crate::script_jobs::EVENTS)), ("listConditions", Some(crate::script_jobs::CONDITIONS)), ("listNodeChanges", Some(NODE_CHANGES))] {
        if a.bool(flag) {
            return Ok(match list {
                Some(l) => json!(l),
                None => json!(with_ui(|u| u.huds.keys().cloned().collect::<Vec<_>>())),
            });
        }
    }
    if a.bool("exists") {
        return Ok(json!(name.is_some_and(|n| with_ui(|u| u.huds.contains_key(&n) || u.hud_on.contains(&n)))));
    }
    if let Some(v) = a.get("nextFreeBlock") {
        let sec = number(v) as i64;
        let used = with_ui(|u| occupied(&u.huds, sec, None));
        return Ok(json!((0..).find(|b| !used.contains(b)).unwrap_or(0)));
    }
    if let Some(v) = a.get("lastOccupiedBlock") {
        let sec = number(v) as i64;
        return Ok(json!(with_ui(|u| occupied(&u.huds, sec, None).last().copied().unwrap_or(-1))));
    }
    if a.bool("refresh") {
        let Some(n) = name else { fail!("headsUpDisplay: -refresh names the heads up display.") };
        return with_ui(|u| {
            let Some(h) = u.huds.get_mut(&n) else { fail!("headsUpDisplay: Object '{n}' not found.") };
            h["__dirty"] = json!(true);
            Ok(Value::Null)
        });
    }
    check_flags(a)?;
    let mut layout_only = true;
    if let Some(v) = a.get("layoutVisibility") {
        with_ui(|u| u.hud.visible = truth(v));
    }
    if a.bool("showGrid") {
        with_ui(|u| u.hud.grid = !u.hud.grid);
    }
    if let Some(v) = a.get("gridColor") {
        with_ui(|u| u.hud.grid_color = number(v) as i64);
    }
    if let Some(v) = a.get("setOption") {
        let p = flat(v);
        if p.len() < 2 {
            fail!("headsUpDisplay: -setOption takes an option and its value.");
        }
        let (o, val) = (text(&p[0]), text(&p[1]));
        if o != "smpPolyCount" {
            fail!("headsUpDisplay: no option named {o}.");
        }
        if !matches!(val.as_str(), "cage" | "smp") {
            fail!("headsUpDisplay: smpPolyCount is \"cage\" or \"smp\", not \"{val}\".");
        }
        with_ui(|u| u.hud.options.insert(o, val));
    }
    for k in a.flags.keys() {
        if !matches!(k.as_str(), "layoutVisibility" | "showGrid" | "gridColor" | "setOption" | "edit" | "queried") && !a.flags[k].is_null() {
            layout_only = false;
        }
    }
    let Some(n) = name else {
        if layout_only {
            return Ok(Value::Null);
        }
        fail!("headsUpDisplay: name the heads up display.");
    };
    // The built-in displays (polyCount, cameraNames...) only show or hide.
    let builtin = with_ui(|u| !u.huds.contains_key(&n)) && a.has("visible") && !a.has("section") && !a.has("block");
    if builtin {
        let on = a.get("visible").is_some_and(truth);
        crate::ui_cmds::set_hud(&n, on);
        return Ok(Value::Null);
    }
    let exists = with_ui(|u| u.huds.contains_key(&n));
    let has_command = a.get("command").is_some_and(|c| !c.is_null() && c != &json!(""));
    let has_trigger = TRIGGERS.iter().any(|t| a.get(t).is_some_and(|v| !v.is_null() && v != &json!(false) && v != &json!("")));
    if a.has("preset") && (has_command || has_trigger) {
        fail!("headsUpDisplay: -preset can't be used with -command or a trigger.");
    }
    // A record named again without a place is edited, as an edit would.
    let edit = a.bool("edit") || (exists && !a.has("section") && !a.has("block"));
    if !edit {
        if exists {
            fail!("headsUpDisplay: Object's name '{n}' is not unique.");
        }
        if !a.has("section") || !a.has("block") {
            fail!("headsUpDisplay: -section and -block are needed to make a heads up display.");
        }
    } else if !exists {
        fail!("headsUpDisplay: Object '{n}' not found.");
    }
    // The place it goes must be free.
    let (sec, blk) = with_ui(|u| {
        let h = u.huds.get(&n);
        let g = |k: &str| a.get(k).map(number).or_else(|| h.and_then(|h| h.get(k)).map(number)).unwrap_or(0.0) as i64;
        (g("section"), g("block"))
    });
    if (a.has("section") || a.has("block")) && with_ui(|u| occupied(&u.huds, sec, Some(&n)).contains(&blk)) {
        fail!("headsUpDisplay: Section {sec}, block {blk} is already occupied.");
    }
    if let Some(new) = a.get("name").map(text).filter(|t| !t.is_empty() && t != &n) {
        if with_ui(|u| u.huds.contains_key(&new)) {
            fail!("headsUpDisplay: Object's name '{new}' is not unique.");
        }
    }
    let id = with_ui(|u| {
        let id = if edit { u.huds.get(&n).and_then(|h| h.get("id")).map(|v| number(v) as i64).unwrap_or(0) } else { u.hud.next_id() };
        let h = u.huds.entry(n.clone()).or_insert_with(|| json!({ "id": id }));
        // A preset replaces a command and its triggers, and the other way round.
        if a.has("preset") {
            for t in TRIGGERS.iter().chain(["command"].iter()) {
                if let Some(o) = h.as_object_mut() {
                    o.remove(*t);
                }
            }
        }
        if has_command || has_trigger {
            if let Some(o) = h.as_object_mut() {
                o.remove("preset");
            }
        }
        for k in KEPT.iter().chain(TRIGGERS.iter()).chain(["allDescendants", "disregardIndex"].iter()) {
            if let Some(v) = a.get(k) {
                let v = match *k {
                    "command" => crate::scriptui::callback_value(v),
                    "section" | "block" | "padding" | "labelWidth" | "dataWidth" | "decimalPrecision" => json!(number(v) as i64),
                    "visible" | "allowOverlap" | "attachToRefresh" | "allDescendants" | "disregardIndex" => json!(truth(v)),
                    _ => json!(text(v)),
                };
                h[*k] = v;
            }
        }
        if let Some(v) = a.get("nodeChanges") {
            let mut have: Vec<Value> = h.get("nodeChanges").map(flat).unwrap_or_default();
            for c in flat(v) {
                if !have.contains(&c) {
                    have.push(c);
                }
            }
            h["nodeChanges"] = Value::Array(have);
        }
        if let Some(v) = a.get("resetNodeChanges") {
            for c in flat(v) {
                if let Some(m) = h.get_mut("__changed").and_then(Value::as_object_mut) {
                    m.insert(text(&c), json!(false));
                }
            }
        }
        // Run once now, so it has something to show.
        if has_command || (!edit && h.get("command").is_some()) {
            h["__dirty"] = json!(true);
        }
        id
    });
    let mut current = n.clone();
    if let Some(new) = a.get("name").map(text).filter(|t| !t.is_empty() && t != &n) {
        with_ui(|u| {
            if let Some(h) = u.huds.remove(&n) {
                u.huds.insert(new.clone(), h);
            }
        });
        current = new;
    }
    if has_trigger || a.has("preset") || a.has("name") || a.has("disregardIndex") || a.has("allDescendants") {
        rewatch(s, &current)?;
    }
    if edit {
        return Ok(Value::Null);
    }
    Ok(json!(id))
}

// ---------------------------------------------------------------------------------------------------------------------
// Idle: running what is due, watching the selection

/// A hash of what -nodeChanges watches on the selected nodes.
fn node_state(s: &mut Session, change: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    let sel = s.graph.selection().to_vec();
    match change {
        "attributeChange" => {
            for id in sel {
                let Some(n) = s.graph.node(id) else { continue };
                let count = n.ty().attrs.len();
                for i in 0..count {
                    let p = aztek_scene::plug::PlugRef::new(id, i as i32);
                    let def = s.graph.attr(&p);
                    if def.is_array() || matches!(def.ty, aztek_scene::plug::PlugType::Mesh | aztek_scene::plug::PlugType::Data) {
                        continue;
                    }
                    if let Ok(v) = s.graph.value(&p) {
                        format!("{v:?}").hash(&mut h);
                    }
                }
            }
        }
        "connectionChange" => {
            for (a, b) in s.graph.connections() {
                if sel.contains(&a.node) || sel.contains(&b.node) {
                    format!("{a:?}{b:?}").hash(&mut h);
                }
            }
        }
        _ => {
            for id in sel {
                let path = s.graph.path(id);
                s.run("listRelatives", json!({"args": [path], "allParents": true})).map(|v| v.to_string()).unwrap_or_default().hash(&mut h);
            }
        }
    }
    h.finish()
}

fn is_selection_trigger(h: &Value) -> bool {
    h.get("event").map(text).as_deref() == Some("SelectionChanged") || ["conditionChange", "conditionTrue", "conditionFalse"].iter().any(|k| h.get(*k).map(text).as_deref() == Some("SomethingSelected"))
}

/// A command's answer as the strings a block shows.
fn shown(v: &Value, precision: usize) -> Vec<String> {
    match v {
        Value::Array(a) => a.iter().flat_map(|x| shown(x, precision)).collect(),
        Value::Null => Vec::new(),
        Value::Number(n) if n.is_f64() => vec![format!("{:.*}", precision, n.as_f64().unwrap_or(0.0))],
        o => vec![text(o)],
    }
}

/// Runs a block's command and keeps what it returned.
pub(crate) fn evaluate(core: &mut Core, name: &str) {
    let Some(cb) = with_ui(|u| u.huds.get(name).and_then(|h| h.get("command")).cloned()) else { return };
    let lines = core.log.len();
    let v = match &cb {
        Value::String(src) if !src.trim().is_empty() => core.mel_checked(src).unwrap_or(Value::Null),
        Value::Object(o) => match o.get("__pycb__").and_then(Value::as_i64) {
            Some(id) => crate::core::python_value(core, id, &[]).unwrap_or(Value::Null),
            None => {
                core.run_callback(&cb, &[]);
                Value::Null
            }
        },
        _ => Value::Null,
    };
    core.log.truncate(lines);
    with_ui(|u| {
        if let Some(h) = u.huds.get_mut(name) {
            h["__result"] = v;
            h["__dirty"] = json!(false);
        }
    });
}

/// The core's idle step: blocks whose watched node went are removed, -nodeChanges are checked, and the commands due run.
pub(crate) fn idle(core: &mut Core) {
    let all: Vec<(String, Value)> = with_ui(|u| u.huds.iter().filter(|(_, h)| h.get("kind").is_none()).map(|(n, h)| (n.clone(), h.clone())).collect());
    for (n, h) in &all {
        for k in ["attributeChange", "connectionChange"] {
            if let Some(p) = h.get(k).map(text).filter(|t| !t.is_empty()) {
                if core.s.graph.plug(&p).is_err() {
                    drop_record(n);
                }
            }
        }
        let watched: Vec<String> = h.get("nodeChanges").map(flat).unwrap_or_default().iter().map(text).collect();
        if watched.is_empty() || !is_selection_trigger(h) {
            continue;
        }
        let mut fired = false;
        let mut states = h.get("__states").cloned().unwrap_or(json!({}));
        let mut changed = h.get("__changed").cloned().unwrap_or(json!({}));
        let sel: Vec<String> = core.s.graph.selection().iter().map(|&i| core.s.graph.path(i)).collect();
        for c in &watched {
            let now = format!("{:?}{}", sel, node_state(&mut core.s, c));
            let was = states.get(c).map(text);
            // A change of selection is the trigger's own; a change on the same selection is the node change.
            let same_sel = h.get("__sel").map(text).as_deref() == Some(format!("{sel:?}").as_str());
            if was.as_deref().is_some_and(|w| w != now) && same_sel {
                changed[c] = json!(true);
                fired = true;
            }
            states[c] = json!(now);
        }
        with_ui(|u| {
            if let Some(r) = u.huds.get_mut(n) {
                r["__states"] = states;
                r["__changed"] = changed;
                r["__sel"] = json!(format!("{sel:?}"));
                if fired {
                    r["__dirty"] = json!(true);
                }
            }
        });
    }
    run_due(core);
}

/// Runs the commands of the blocks marked for it.
pub(crate) fn run_due(core: &mut Core) {
    let due: Vec<String> = with_ui(|u| u.huds.iter().filter(|(_, h)| h.get("__dirty").is_some_and(truth)).map(|(n, _)| n.clone()).collect());
    for n in due {
        evaluate(core, &n);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// What the views draw

/// One block as the views lay it out.
pub struct Block {
    pub name: String,
    pub section: i64,
    pub block: i64,
    /// Height in pixels: 20, 35 or 50.
    pub height: f64,
    pub align: String,
    pub padding: f64,
    pub label: String,
    pub label_large: bool,
    pub label_width: f64,
    pub data: Vec<String>,
    pub data_large: bool,
    pub data_width: f64,
    pub data_right: bool,
    pub allow_overlap: bool,
    /// The preset it shows, when it is one: the views work its values out.
    pub preset: String,
    pub precision: usize,
}

pub fn layout_visible() -> bool {
    with_ui(|u| u.hud.visible)
}

/// The grid lines and their colour index, when on.
pub fn grid() -> Option<i64> {
    with_ui(|u| u.hud.grid.then_some(u.hud.grid_color))
}

/// The blocks to draw: the visible headsUpDisplay records. Blocks with -attachToRefresh run their command first.
pub fn blocks(core: &mut Core) -> Vec<Block> {
    let refresh: Vec<String> = with_ui(|u| u.huds.iter().filter(|(_, h)| h.get("kind").is_none() && h.get("attachToRefresh").is_some_and(truth)).map(|(n, _)| n.clone()).collect());
    for n in refresh {
        evaluate(core, &n);
    }
    with_ui(|u| {
        u.huds
            .iter()
            .filter(|(_, h)| h.get("kind").is_none() && h.get("visible").map(truth).unwrap_or(true))
            .map(|(n, h)| {
                let g = |k: &str| h.get(k).cloned().unwrap_or_else(|| default_of(k));
                let precision = number(&g("decimalPrecision")) as usize;
                Block {
                    name: n.clone(),
                    section: number(&g("section")) as i64,
                    block: number(&g("block")) as i64,
                    height: match text(&g("blockSize")).as_str() {
                        "medium" => 35.0,
                        "large" => 50.0,
                        _ => 20.0,
                    },
                    align: text(&g("blockAlignment")),
                    padding: number(&g("padding")),
                    label: text(&g("label")),
                    label_large: text(&g("labelFontSize")) == "large",
                    label_width: number(&g("labelWidth")),
                    data: shown(h.get("__result").unwrap_or(&Value::Null), precision),
                    data_large: text(&g("dataFontSize")) == "large",
                    data_width: number(&g("dataWidth")),
                    data_right: text(&g("dataAlignment")) == "right",
                    allow_overlap: truth(&g("allowOverlap")),
                    preset: text(&g("preset")),
                    precision,
                }
            })
            .collect()
    })
}

pub(crate) fn register(s: &mut Session) {
    let c = Cmd::new(
        "headsUpDisplay",
        "Text blocks over the views: -s section (0 to 9) -b block, -l label, -c command with a trigger (-ev, -cc, -ct, -cf, -ac, -con, -atr) or -pre preset, and their formatting. \
         Returns the new block's ID; -rem, -rid and -rp remove one, -ex tests, -lh lists, -nfb and -lob find blocks, -lv shows or hides the lot.",
        command,
    )
    .positional("name")
    .undoable(false);
    s.register(crate::scriptui::add_flags(c, FLAGS));
}
