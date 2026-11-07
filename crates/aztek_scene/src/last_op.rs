//! The last adjustable command run from the UI, kept so it can be re-run with other
//! flags. Each re-run is an undo step of its own ("Adjust Bevel: offset") that replaces the op's result, so undo walks
//! back through the settings before it undoes the op, and redo walks forward through them again.
//!
//! The session calls `sync` after every top-level command and every undo or
//! redo it runs, which sees the same steps in the same order.

use std::collections::BTreeMap;
use std::sync::Arc;

use aztek_ops::{execute, fail, ArgType, CallSource, CmdResult, CommandError, FlagDef};
use serde_json::{json, Map, Value};

use crate::components::{self, ComponentMode, Ids};
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

#[derive(Debug, Clone)]
pub struct Op {
    pub command: String,
    pub call: Value,
    pub mode_before: ComponentMode,
    pub mode_after: ComponentMode,
    /// Components selected in mode_after when the op, or the adjustment the stack is on, finished: what it made.
    pub made: BTreeMap<NodeId, Ids>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    /// A new op ran.
    Recorded,
    /// The op was re-run with new flags.
    Rerun,
    /// Undo or redo landed on one of the op's settings.
    Restored,
    /// Forgotten.
    Cleared,
    /// The op stopped being reachable(): another edit went on top, or undo went below it.
    Other,
}

pub type Listener = Arc<dyn Fn(Change) + Send + Sync>;

/// Local transform values of an object the op made, put back on its remake when the op is adjusted.
#[derive(Debug, Clone)]
struct Placement {
    path: String,
    values: Vec<(usize, PlugValue)>,
}

/// One setting of the op: the undo step holding its result and the call that made it.
#[derive(Debug, Clone)]
struct Entry {
    step: u64,
    call: Value,
    mode_after: ComponentMode,
    made: BTreeMap<NodeId, Ids>,
    /// Objects selected in object mode when it finished: what it created. Moving only these keeps the op adjustable.
    objects: Vec<NodeId>,
    /// Where those objects were moved to before this setting was made, kept through further adjustments.
    placements: Vec<Placement>,
}

#[derive(Default)]
pub struct State {
    op: Option<Op>,
    /// The op's original run first, then each adjustment still reachable by undo or redo.
    entries: Vec<Entry>,
    at: usize,
    dragging: bool,
    /// Undo went below the op or another step went on top, so landing back on an entry restores it.
    away: bool,
    was_reachable: bool,
    pending_mode: ComponentMode,
    pending_step: u64,
    rerunning: bool,
    limited: Map<String, Value>,
    /// Undo steps that only moved the objects of an entry, made on top of it, by the entry index they sit on.
    carried: BTreeMap<u64, usize>,
    carry_runs: i32,
    carry_clean: bool,
    last_state: u64,
    max_seen: u64,
    /// The newest step and state sync() last saw, so it only acts on changes.
    seen: (u64, u64, u64),
    listeners: BTreeMap<i32, Listener>,
    next_listener: i32,
}

fn notify(s: &Session, c: Change) {
    let copy: Vec<Listener> = s.last_op.listeners.values().cloned().collect();
    for f in copy {
        f(c);
    }
}

fn made_now(s: &Session) -> BTreeMap<NodeId, Ids> {
    let m = components::mode(&s.graph);
    if matches!(m, ComponentMode::Vertex | ComponentMode::Edge | ComponentMode::Face) {
        return components::selected_all(&s.graph, m).clone();
    }
    BTreeMap::new()
}

fn made_objects(s: &Session) -> Vec<NodeId> {
    if components::mode(&s.graph) != ComponentMode::Object {
        return Vec::new();
    }
    let g = &s.graph;
    g.selection().iter().copied().filter(|&id| g.node(id).is_some_and(|n| n.ty().is_a("transform"))).collect()
}

const PLACEMENT_ATTRS: [usize; 10] = [
    Transform::TRANSLATE,
    Transform::ROTATE,
    Transform::SCALE,
    Transform::ROTATE_ORDER,
    Transform::ROTATE_PIVOT,
    Transform::SCALE_PIVOT,
    Transform::ROTATE_PIVOT_TRANSLATE,
    Transform::SCALE_PIVOT_TRANSLATE,
    Transform::PIVOT_ORIENT,
    Transform::ROTATE_AXIS,
];

fn args_list(call: &Value) -> Vec<Value> {
    match call.get("args") {
        Some(Value::Array(a)) => a.clone(),
        Some(v) => vec![v.clone()],
        None => Vec::new(),
    }
}

/// True when the command only moves, rotates, scales or re-pivots the current entry's objects.
fn moves_made_objects(s: &Session, command: &str, call: &Value) -> bool {
    let st = &s.last_op;
    let Some(entry) = st.entries.get(st.at) else { return false };
    if entry.objects.is_empty() {
        return false;
    }
    let g = &s.graph;
    let made = |id: NodeId| entry.objects.contains(&id);
    let list = args_list(call);
    if command == "setAttr" {
        let Some(path) = list.first().and_then(Value::as_str) else { return false };
        return match g.plug(path) {
            Ok(p) => made(p.node) && p.attr >= 0 && PLACEMENT_ATTRS.contains(&(p.attr as usize)),
            Err(_) => false,
        };
    }
    if !["move", "rotate", "scale", "setPivot", "resetPivot", "centerPivot"].contains(&command) {
        return false;
    }
    if list.is_empty() {
        return g.selection().iter().all(|&id| made(id));
    }
    list.iter().all(|a| a.as_str().and_then(|n| g.find(n).ok().flatten()).is_some_and(made))
}

fn carried_on(st: &State, step: u64) -> bool {
    st.carried.get(&step) == Some(&st.at)
}

fn capture_placements(s: &Session, objects: &[NodeId]) -> Vec<Placement> {
    let g = &s.graph;
    objects
        .iter()
        .filter_map(|&id| g.node(id).map(|n| (id, n)))
        .map(|(id, n)| Placement { path: g.path(id), values: PLACEMENT_ATTRS.iter().map(|&a| (a, n.value(a).clone())).collect() })
        .collect()
}

/// Puts the placements back on the remade objects: matched in order when the counts agree, else by path.
fn apply_placements(s: &mut Session, objects: &[NodeId], placements: &[Placement]) {
    let g = &mut s.graph;
    for (i, p) in placements.iter().enumerate() {
        let id = if placements.len() == objects.len() { Some(objects[i]) } else { g.find(&p.path).ok().flatten() };
        let Some(id) = id else { continue };
        if !g.node(id).is_some_and(|n| n.ty().is_a("transform")) {
            continue;
        }
        for (attr, value) in &p.values {
            let plug = PlugRef::new(id, *attr as i32);
            if g.node(id).unwrap().value(*attr) != value && g.source(&plug).is_none() {
                let _ = g.set_value(plug, value.clone());
            }
        }
    }
}

fn reset_carry(s: &mut Session) {
    let (state, newest) = (s.graph.undo_stack().state_id(), s.graph.undo_stack().newest_id());
    let st = &mut s.last_op;
    st.carry_runs = 0;
    st.carry_clean = true;
    st.last_state = state;
    st.max_seen = st.max_seen.max(newest);
}

fn move_to(s: &mut Session, index: usize) {
    let st = &mut s.last_op;
    st.at = index;
    let e = st.entries[index].clone();
    let op = st.op.as_mut().unwrap();
    op.call = e.call;
    op.mode_after = e.mode_after;
    op.made = e.made;
    components::set_mode(&mut s.graph, e.mode_after);
}

/// A top-level command is about to run.
pub(crate) fn command_started(s: &mut Session, name: &str, call: &Value) {
    if s.last_op.rerunning {
        return;
    }
    if s.last_op.op.is_some() {
        if moves_made_objects(s, name, call) {
            s.last_op.carry_runs += 1;
        } else {
            s.last_op.carry_clean = false;
        }
    }
    s.last_op.pending_mode = components::mode(&s.graph);
    s.last_op.pending_step = s.graph.undo_stack().newest_id();
    s.last_op.limited = Map::new();
}

/// A top-level command finished.
pub(crate) fn command_finished(s: &mut Session, name: &str, call: &Value, source: CallSource, ok: bool) {
    if s.last_op.rerunning {
        return;
    }
    sync(s);
    let undo = s.graph.undo_stack();
    let recorded = ok
        && source == CallSource::Ui
        && s.commands.find(name).is_some_and(|d| d.adjustable && d.undoable && undo.undo_label() == d.name)
        && undo.newest_id() != s.last_op.pending_step;
    if !recorded {
        return;
    }
    let newest = undo.newest_id();
    let mut c = if call.is_object() { call.clone() } else { json!({}) };
    for (k, v) in &s.last_op.limited {
        c[k] = v.clone();
    }
    let mode = components::mode(&s.graph);
    let made = made_now(s);
    let objects = made_objects(s);
    let st = &mut s.last_op;
    st.op = Some(Op { command: name.into(), call: c.clone(), mode_before: st.pending_mode, mode_after: mode, made: made.clone() });
    st.entries = vec![Entry { step: newest, call: c, mode_after: mode, made, objects, placements: Vec::new() }];
    st.at = 0;
    st.dragging = false;
    st.away = false;
    st.was_reachable = true;
    st.carried.clear();
    reset_carry(s);
    notify(s, Change::Recorded);
}

/// Called after anything that may have changed the undo stack.
pub fn sync(s: &mut Session) {
    let undo = s.graph.undo_stack();
    let now = (undo.newest_id(), undo.state_id(), undo.serial());
    if now == s.last_op.seen {
        return;
    }
    s.last_op.seen = now;
    if s.last_op.rerunning || s.last_op.op.is_none() || s.commands.previewing() {
        return;
    }
    let (newest, edit) = (now.0, now.1);
    let st = &mut s.last_op;
    // A step that only moved what the op made, pushed onto the op or onto another such step, keeps the op adjustable.
    let pushed = newest > st.max_seen;
    if pushed
        && edit == newest
        && st.carry_clean
        && st.carry_runs > 0
        && st.at < st.entries.len()
        && (st.last_state == st.entries[st.at].step || carried_on(st, st.last_state))
    {
        let at = st.at;
        st.carried.insert(newest, at);
    }
    reset_carry(s);
    if let Some(i) = s.last_op.entries.iter().position(|e| e.step == newest) {
        s.last_op.was_reachable = true;
        let away = std::mem::take(&mut s.last_op.away);
        if i != s.last_op.at || away {
            s.last_op.dragging = false;
            move_to(s, i);
            notify(s, Change::Restored);
        }
        return;
    }
    s.last_op.away = true;
    // Selection steps on top of an entry keep it adjustable; a jump in the history can land on another entry.
    if let Some(i) = s.last_op.entries.iter().position(|e| e.step == edit) {
        if i != s.last_op.at {
            move_to(s, i);
        }
    }
    let now = reachable(s);
    let was = std::mem::replace(&mut s.last_op.was_reachable, now);
    if was != now {
        notify(s, if now { Change::Restored } else { Change::Other });
    }
}

/// The last recorded op. It may be stale; see live().
pub fn current(s: &Session) -> Option<&Op> {
    s.last_op.op.as_ref()
}

/// True while the op, or an adjustment of it, is the newest undo step, so it can be re-run.
pub fn live(s: &Session) -> bool {
    let st = &s.last_op;
    st.op.is_some() && !st.rerunning && st.at < st.entries.len() && s.graph.undo_stack().newest_id() == st.entries[st.at].step
}

/// True while the op, or an adjustment of it, is the newest undo step other than selection changes. rerun() and
/// make_non_destructive() accept this: they first undo the selection steps above the op and drop them, since
/// component ids they name may not survive a re-run.
pub fn reachable(s: &Session) -> bool {
    let st = &s.last_op;
    // A selection op (selectByAngle) is itself a selection step, so only live() keeps it.
    let state = s.graph.undo_stack().state_id();
    live(s)
        || (st.op.is_some()
            && !st.rerunning
            && st.at < st.entries.len()
            && (state == st.entries[st.at].step || carried_on(st, state)))
}

/// Called by a running command whose flag hit a limit (a bevel offset clamped by the mesh): the op records the value
/// actually used, so adjusting it cannot run past the limit.
pub fn limit_flag(s: &mut Session, name: &str, value: Value) {
    if s.commands.previewing() {
        return;
    }
    s.last_op.limited.insert(name.into(), value);
}

pub fn clear(s: &mut Session) {
    if s.last_op.op.is_none() {
        return;
    }
    s.last_op.op = None;
    s.last_op.entries.clear();
    s.last_op.carried.clear();
    s.last_op.was_reachable = false;
    notify(s, Change::Cleared);
}

/// Undoes the selection steps above the op's current entry, so it is the newest step again. False if it is not
/// reachable.
fn undo_selection_on_top(s: &mut Session) -> bool {
    if !reachable(s) {
        return false;
    }
    s.last_op.rerunning = true;
    let step = s.last_op.entries[s.last_op.at].step;
    while s.graph.undo_stack().newest_id() != step && s.graph.undo() {}
    s.last_op.rerunning = false;
    live(s)
}

fn not_reachable() -> CommandError {
    CommandError::new("nothing to adjust: the last operation is no longer the latest undo step")
}

/// Runs the op again with `flags` merged into its call, as a new undo step that replaces its result. Returns the
/// command's result, or Null when the flags change nothing. `drag` marks one tick of a continuous edit (a handle or
/// slider drag): drag re-runs replace one another's step until end_drag(), so the whole drag undoes at once. On
/// failure the previous result is put back and the error returned.
pub fn rerun(s: &mut Session, flags: &Value, source: CallSource, drag: bool) -> CmdResult {
    sync(s);
    if !reachable(s) {
        return Err(not_reachable());
    }
    let op = s.last_op.op.clone().unwrap();
    let mut call = op.call.clone();
    if let Some(f) = flags.as_object() {
        for (k, v) in f {
            call[k] = v.clone();
        }
    }
    if call == op.call {
        return Ok(Value::Null);
    }
    // Moves made since are undone with the selection steps and re-applied to the remade objects.
    let at = s.last_op.at;
    let placements = if carried_on(&s.last_op, s.graph.undo_stack().state_id()) {
        let objects = s.last_op.entries[at].objects.clone();
        capture_placements(s, &objects)
    } else {
        s.last_op.entries[at].placements.clone()
    };
    let redo_before = s.graph.undo_stack().redo_count();
    if !undo_selection_on_top(s) {
        return Err(not_reachable());
    }
    let selection_undone = s.graph.undo_stack().redo_count() - redo_before;
    let st = &s.last_op;
    // A drag keeps replacing the step it made, never the op's own run.
    let replace = drag && st.dragging && st.at > 0 && st.at + 1 == st.entries.len();

    // Named after the flags that differ from the settings the step replaces.
    let from = if replace { st.entries[st.at - 1].call.clone() } else { op.call.clone() };
    let changed: Vec<&str> = flags
        .as_object()
        .map(|f| f.iter().filter(|(k, v)| *k != "args" && from.get(k.as_str()) != Some(v)).map(|(k, _)| k.as_str()).collect())
        .unwrap_or_default();
    let label = format!(
        "Adjust {}{}",
        display_name(&op.command),
        if changed.is_empty() { String::new() } else { format!(": {}", changed.join(", ")) }
    );
    let redo_selection = |s: &mut Session| {
        for _ in 0..selection_undone {
            s.graph.redo();
        }
    };
    s.last_op.rerunning = true;
    s.last_op.limited = Map::new();
    let opened = if replace { s.graph.reopen_newest(label) } else { s.graph.open_amend(label) };
    if !opened {
        redo_selection(s);
        s.last_op.rerunning = false;
        fail!("nothing to adjust: the undo stack is busy");
    }
    components::set_mode(&mut s.graph, op.mode_before);
    let result = match execute(s, &op.command, &call, source) {
        Ok(r) => r,
        Err(e) => {
            s.graph.cancel_chunk();
            redo_selection(s);
            s.last_op.rerunning = false;
            // A replaced drag step is gone with the cancelled chunk.
            if replace {
                s.last_op.entries.pop();
                s.last_op.dragging = false;
            }
            let i = s.last_op.at.min(s.last_op.entries.len() - 1);
            move_to(s, i);
            notify(s, Change::Rerun);
            return Err(e);
        }
    };
    let objects = made_objects(s);
    apply_placements(s, &objects, &placements);
    s.graph.close_chunk();
    s.last_op.rerunning = false;
    for (k, v) in &s.last_op.limited {
        call[k] = v.clone();
    }
    let entry = Entry {
        step: s.graph.undo_stack().newest_id(),
        call,
        mode_after: components::mode(&s.graph),
        made: made_now(s),
        objects,
        placements,
    };
    let st = &mut s.last_op;
    if replace {
        *st.entries.last_mut().unwrap() = entry;
    } else {
        st.entries.truncate(st.at + 1);
        st.entries.push(entry);
    }
    st.dragging = drag;
    let last = st.entries.len() - 1;
    move_to(s, last);
    reset_carry(s);
    s.last_op.seen = (0, 0, 0);
    sync_seen(s);
    notify(s, Change::Rerun);
    Ok(result)
}

fn sync_seen(s: &mut Session) {
    let u = s.graph.undo_stack();
    s.last_op.seen = (u.newest_id(), u.state_id(), u.serial());
}

/// Ends a drag of re-runs. Without `keep` the drag's step is undone and dropped, as when the drag is cancelled.
pub fn end_drag(s: &mut Session, keep: bool) {
    if !std::mem::take(&mut s.last_op.dragging) || keep || !live(s) || s.last_op.at == 0 {
        return;
    }
    s.last_op.rerunning = true;
    s.graph.undo();
    s.graph.undo_stack_mut().flush_future();
    s.last_op.rerunning = false;
    s.last_op.entries.pop();
    let last = s.last_op.entries.len() - 1;
    move_to(s, last);
    sync_seen(s);
    notify(s, Change::Rerun);
}

/// Flags of the op's command the UI can edit: Bool, Int, Float, String with choices, and the Vec3 and free-text String
/// flags `adjust` lists (the command's option override). `hidden` are left out.
pub fn editable_flags(s: &Session, command: &str, hidden: &[&str], adjust: &[&str]) -> Vec<FlagDef> {
    let Some(def) = s.commands.find(command) else { return Vec::new() };
    def.flags
        .iter()
        .filter(|f| !hidden.contains(&f.name.as_str()))
        .filter(|f| match f.ty {
            ArgType::Bool | ArgType::Int | ArgType::Float => true,
            ArgType::String => !f.choices.is_empty() || adjust.contains(&f.name.as_str()),
            ArgType::Vec3 => adjust.contains(&f.name.as_str()),
            _ => false,
        })
        .cloned()
        .collect()
}

/// The value the op ran with for a flag: its call's value, else the flag default.
pub fn flag_value(op: &Op, flag: &FlagDef) -> Value {
    if let Some(v) = op.call.get(&flag.name).filter(|v| !v.is_null()) {
        return v.clone();
    }
    if !flag.short_name.is_empty() {
        if let Some(v) = op.call.get(&flag.short_name) {
            return v.clone();
        }
    }
    flag.default_value.clone()
}

/// "polyInsertLoop" reads as "Insert Loop".
pub fn display_name(command: &str) -> String {
    let s = if command.starts_with("poly") && command.len() > 4 { &command[4..] } else { command };
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            out.push(' ');
        }
        out.push(if i == 0 { c.to_ascii_uppercase() } else { c });
    }
    out
}

/// Modifier kind (as for addModifier) that does the op non-destructively, or empty.
pub fn modifier_equivalent(command: &str) -> &'static str {
    match command {
        "polyBevel" => "bevel",
        "polyExtrude" => "extrude",
        "polyMirror" => "mirror",
        "polySubdivide" => "subdivision",
        "polyTriangulate" => "triangulate",
        "polySplitRings" => "splitRings",
        _ => "",
    }
}

fn meshes_of(s: &Session, op: &Op) -> Vec<String> {
    let g = &s.graph;
    let mesh_path = |id: NodeId| components::as_mesh(g, id).map(|m| components::display_path(g, m));
    if op.call.get("args").is_some() {
        return args_list(&op.call).iter().filter_map(|a| a.as_str().map(str::to_string)).collect();
    }
    if op.mode_before != ComponentMode::Object && !components::selected_all(g, op.mode_before).is_empty() {
        return components::selected_all(g, op.mode_before).keys().filter_map(|&id| mesh_path(id)).collect();
    }
    g.selection().iter().filter_map(|&id| mesh_path(id)).collect()
}

/// Undoes the op and adds its modifier equivalent, with matching parameters, to the meshes it worked on.
pub fn make_non_destructive(s: &mut Session) -> CmdResult<()> {
    sync(s);
    if !reachable(s) {
        fail!("makeNonDestructive: the last operation is no longer the latest undo step");
    }
    let op = s.last_op.op.clone().unwrap();
    let kind = modifier_equivalent(&op.command);
    if kind.is_empty() {
        fail!("makeNonDestructive: {} has no modifier equivalent", op.command);
    }
    undo_selection_on_top(s);
    let def = s.commands.find(&op.command).cloned().unwrap();
    let value = |flag: &str| def.flags.iter().find(|f| f.name == flag).map(|f| flag_value(&op, f)).unwrap_or(Value::Null);
    let params = match kind {
        "bevel" => json!({"offset": value("offset"), "segments": value("segments"), "profile": value("profile")}),
        "mirror" => {
            let axis = value("axis").as_str().unwrap_or("x").to_string();
            let direction = value("direction").as_str().unwrap_or("positive").to_string();
            let a = axis.bytes().next().map(|c| (c as i32 - b'x' as i32).clamp(0, 2)).unwrap_or(0);
            json!({"axis": a, "direction": match direction.as_str() { "negative" => 1, "both" => 2, _ => 0 }, "weld": value("weld")})
        }
        "subdivision" => json!({"levels": value("levels")}),
        "splitRings" => json!({"count": value("count"), "direction": if value("direction") == "across" { 1 } else { 0 }, "edgeFlow": value("edgeFlow")}),
        // The modifier takes the selected faces or edges as addModifier creates it, once undo has put the selection
        // back.
        "extrude" => {
            if value("duplicate") == json!(true) {
                fail!("makeNonDestructive: a duplicating extrude has no modifier equivalent");
            }
            json!({"thickness": value("thickness"), "offset": value("offset"), "individual": value("individual"), "keepBack": value("keepBack")})
        }
        _ => json!({}),
    };

    // Back past every adjustment and the op's own run.
    s.last_op.rerunning = true;
    for _ in 0..=s.last_op.at {
        s.graph.undo();
    }
    s.last_op.rerunning = false;
    components::set_mode(&mut s.graph, op.mode_before);
    let meshes = meshes_of(s, &op);
    s.last_op.op = None;
    s.last_op.entries.clear();
    notify(s, Change::Cleared);
    s.graph.open_chunk("Make Non-Destructive");
    let mut result = Ok(());
    for mesh in meshes {
        let mut call = json!({"args": [mesh], "kind": kind});
        if params.as_object().is_some_and(|p| !p.is_empty()) {
            call["params"] = params.clone();
        }
        if let Err(e) = execute(s, "addModifier", &call, CallSource::Ui) {
            result = Err(e);
            break;
        }
    }
    s.graph.close_chunk();
    sync_seen(s);
    result
}

pub fn add_listener(s: &mut Session, f: Listener) -> i32 {
    let st = &mut s.last_op;
    st.next_listener += 1;
    st.listeners.insert(st.next_listener, f);
    st.next_listener
}

pub fn remove_listener(s: &mut Session, id: i32) {
    s.last_op.listeners.remove(&id);
}
