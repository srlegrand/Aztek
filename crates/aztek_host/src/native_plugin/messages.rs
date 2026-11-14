//! Compiled plug-ins' messages: MSceneMessage, MDGMessage, MDagMessage, MNodeMessage. The kit registers a callback
//! (callback_add) and asks here for what it should hear (`message.watch`). The graph notes its changes while anything
//! watches (aztek_scene::messages); they are sent after each command, at any depth, still inside its undo step, and
//! after an undo or redo. File commands send their scene messages before and after they run; a check callback can stop
//! one, or pick another file.
//!
//! A message about a node that has gone (removed, undone) is sent once it has: the node's name, type and place are
//! answered for it while its callbacks run (`departed_info`). The delete command's own nodes get their about-to-delete
//! and pre-removal messages while they are still there.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};

use aztek_ops::{Args, CommandError};
use aztek_scene::messages::{self as notes, Departed, Event, NOTE_CONNECTIONS, NOTE_DAG, NOTE_HOOKS, NOTE_NAMES, NOTE_NODES, NOTE_PLUGS, NOTE_UUIDS};
use aztek_scene::node::types;
use aztek_scene::plug::{NodeId, PlugRef};
use aztek_scene::Session;
use serde_json::{json, Value};

use super::table::fire_callback;

// The reference's MSceneMessage::Message values.
pub(crate) const SCENE_UPDATE: i64 = 0;
const BEFORE_NEW: i64 = 1;
const AFTER_NEW: i64 = 2;
const BEFORE_IMPORT: i64 = 3;
const AFTER_IMPORT: i64 = 4;
const BEFORE_OPEN: i64 = 5;
const AFTER_OPEN: i64 = 6;
const BEFORE_FILE_READ: i64 = 7;
const AFTER_FILE_READ: i64 = 8;
const AFTER_SCENE_READ_AND_RECORD_EDITS: i64 = 9;
const BEFORE_EXPORT: i64 = 10;
const AFTER_EXPORT: i64 = 11;
const BEFORE_SAVE: i64 = 12;
const AFTER_SAVE: i64 = 13;
const BEFORE_REFERENCE: i64 = 14;
const AFTER_REFERENCE: i64 = 15;
const BEFORE_REMOVE_REFERENCE: i64 = 16;
const AFTER_REMOVE_REFERENCE: i64 = 17;
const BEFORE_IMPORT_REFERENCE: i64 = 18;
const AFTER_IMPORT_REFERENCE: i64 = 19;
const BEFORE_UNLOAD_REFERENCE: i64 = 22;
const AFTER_UNLOAD_REFERENCE: i64 = 23;
pub(crate) const MAYA_INITIALIZED: i64 = 29;
pub(crate) const MAYA_EXITING: i64 = 30;
const BEFORE_NEW_CHECK: i64 = 31;
const BEFORE_OPEN_CHECK: i64 = 32;
const BEFORE_SAVE_CHECK: i64 = 33;
const BEFORE_IMPORT_CHECK: i64 = 34;
const BEFORE_EXPORT_CHECK: i64 = 35;
const BEFORE_LOAD_REFERENCE: i64 = 36;
const AFTER_LOAD_REFERENCE: i64 = 37;
const BEFORE_LOAD_REFERENCE_CHECK: i64 = 38;
const BEFORE_CREATE_REFERENCE_CHECK: i64 = 39;
pub(crate) const BEFORE_PLUGIN_LOAD: i64 = 40;
pub(crate) const AFTER_PLUGIN_LOAD: i64 = 41;
pub(crate) const BEFORE_PLUGIN_UNLOAD: i64 = 42;
pub(crate) const AFTER_PLUGIN_UNLOAD: i64 = 43;
const BEFORE_CREATE_REFERENCE: i64 = 44;
const AFTER_CREATE_REFERENCE: i64 = 45;
const EXPORT_STARTED: i64 = 46;
const BEFORE_LOAD_REFERENCE_AND_RECORD_EDITS: i64 = 47;
const AFTER_LOAD_REFERENCE_AND_RECORD_EDITS: i64 = 48;
const BEFORE_CREATE_REFERENCE_AND_RECORD_EDITS: i64 = 49;
const AFTER_CREATE_REFERENCE_AND_RECORD_EDITS: i64 = 50;

// MDagMessage::DagMessage.
const PARENT_ADDED: i64 = 0;
const PARENT_REMOVED: i64 = 1;
const CHILD_ADDED: i64 = 2;
const CHILD_REMOVED: i64 = 3;
const CHILD_REORDERED: i64 = 4;

// MNodeMessage::AttributeMessage.
const CONNECTION_MADE: i64 = 0x01;
const CONNECTION_BROKEN: i64 = 0x02;
const ATTRIBUTE_SET: i64 = 0x08;
const ATTRIBUTE_LOCKED: i64 = 0x10;
const ATTRIBUTE_UNLOCKED: i64 = 0x20;
const ATTRIBUTE_ADDED: i64 = 0x40;
const ATTRIBUTE_REMOVED: i64 = 0x80;
const ATTRIBUTE_RENAMED: i64 = 0x100;
const ATTRIBUTE_KEYABLE: i64 = 0x200;
const ATTRIBUTE_UNKEYABLE: i64 = 0x400;
const INCOMING_DIRECTION: i64 = 0x800;
const ATTRIBUTE_ARRAY_REMOVED: i64 = 0x2000;

const CANCELLED: &str = "File operation cancelled by user supplied callback.";

/// What one callback hears.
#[derive(Debug, Clone)]
struct Watch {
    kind: String,
    /// The message: a scene message, a DAG message (-1 for every one), a time callback's place in the order.
    msg: i64,
    /// The node it's on; -1 for every node.
    node: i64,
    /// nodeAdded and nodeRemoved: the type, with the types derived from it.
    ty: String,
    /// Python's (the 2.0 and 1.0 APIs): called through its hook, not a compiled plug-in's table.
    python: bool,
}

/// Python's callback ids: apart from compiled plug-ins'.
static NEXT_PYTHON: std::sync::atomic::AtomicI64 = std::sync::atomic::AtomicI64::new(1 << 40);

#[derive(Default)]
struct State {
    dispatching: bool,
    last_time: Option<f64>,
    /// Each matrix callback's node, and its ancestors for a world matrix one, with their last values.
    matrices: HashMap<i64, Vec<(NodeId, Vec<f64>)>>,
    departed: HashMap<NodeId, Departed>,
    /// Nodes the delete command told of before it deleted them.
    told: HashSet<NodeId>,
    reply: Option<Value>,
    /// The file commands running, each with whether it sends its messages: one inside another of its kind (a file -new
    /// running newScene) doesn't.
    running: Vec<(FileOp, bool)>,
}

thread_local! {
    static WATCHES: RefCell<BTreeMap<i64, Watch>> = const { RefCell::new(BTreeMap::new()) };
    static STATE: RefCell<State> = RefCell::new(State::default());
}

pub(crate) fn install() {
    notes::set_hooks(notes::Hooks { before, after, flush: dispatch });
}

fn mask_of(w: &Watch) -> u32 {
    match w.kind.as_str() {
        "nodeAdded" | "nodeRemoved" | "aboutToDelete" | "preRemoval" | "destroyed" => NOTE_NODES,
        "dag" => NOTE_NODES | NOTE_DAG,
        "attributeChanged" | "dirty" | "dirtyPlug" => NOTE_NODES | NOTE_PLUGS | NOTE_CONNECTIONS,
        "attributeAddedOrRemoved" => NOTE_NODES | NOTE_PLUGS,
        "connection" | "preConnection" => NOTE_NODES | NOTE_CONNECTIONS,
        "nameChanged" => NOTE_NAMES,
        "uuidChanged" => NOTE_UUIDS,
        _ => 0,
    }
}

fn recount() {
    let mask = WATCHES.with(|w| {
        let w = w.borrow();
        if w.is_empty() {
            0
        } else {
            w.values().fold(NOTE_HOOKS, |m, x| m | mask_of(x))
        }
    });
    notes::set_watching(mask);
}

fn int(a: &Value, k: &str, d: i64) -> i64 {
    a.get(k).and_then(Value::as_i64).unwrap_or(d)
}

/// `message.watch`: {callback, kind, msg, node, type}.
pub(crate) fn watch(s: &mut Session, a: &Value) -> Result<Value, String> {
    let id = int(a, "callback", 0);
    if id <= 0 {
        return Err("(kInvalidParameter): no callback".into());
    }
    let w = Watch {
        kind: a.get("kind").and_then(Value::as_str).unwrap_or("").to_string(),
        msg: int(a, "msg", -1),
        node: int(a, "node", -1),
        ty: a.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
        python: a.get("python").and_then(Value::as_bool).unwrap_or(false),
    };
    match w.kind.as_str() {
        "scene" | "time" | "nodeAdded" | "nodeRemoved" | "connection" | "preConnection" | "dag" | "attributeChanged"
        | "attributeAddedOrRemoved" | "dirty" | "dirtyPlug" | "nameChanged" | "uuidChanged" | "aboutToDelete"
        | "preRemoval" | "destroyed" | "connectionFailed" | "namespaceRenamed" | "uuidCheck" | "keyableOverride" => {}
        "matrix" | "worldMatrix" => {
            let chain = matrix_chain(s, w.node.max(0) as NodeId, w.kind == "worldMatrix");
            STATE.with(|st| st.borrow_mut().matrices.insert(id, chain));
        }
        k => return Err(format!("(kInvalidParameter): no message {k}")),
    }
    if w.node >= 0 && s.graph.node(w.node as NodeId).is_none() {
        return Err("(kInvalidParameter): Object does not exist".into());
    }
    if w.kind == "time" {
        let t = s.graph.time();
        STATE.with(|st| st.borrow_mut().last_time = Some(t));
    }
    WATCHES.with(|x| x.borrow_mut().insert(id, w));
    recount();
    Ok(json!(id))
}

/// `message.pywatch`: a Python callback, its id made here.
pub(crate) fn watch_python(s: &mut Session, a: &Value) -> Result<Value, String> {
    let mut a = a.clone();
    a["callback"] = json!(NEXT_PYTHON.fetch_add(1, std::sync::atomic::Ordering::Relaxed));
    a["python"] = json!(true);
    watch(s, &a)
}

/// A callback removed, by the plug-in or with it.
pub(crate) fn unwatch(id: i64) {
    let had = WATCHES.with(|w| w.borrow_mut().remove(&id).is_some());
    STATE.with(|st| st.borrow_mut().matrices.remove(&id));
    if had {
        recount();
    }
}

/// MMessage::nodeCallbacks.
pub(crate) fn node_callbacks(node: NodeId) -> Value {
    WATCHES.with(|w| json!(w.borrow().iter().filter(|(_, x)| x.node == node as i64).map(|(id, _)| *id).collect::<Vec<_>>()))
}

/// What a check callback answered: its return code, the file it picked.
pub(crate) fn reply(a: &Value) {
    STATE.with(|st| st.borrow_mut().reply = Some(a.clone()));
}

/// `node.info` for a node that has gone, while its messages are sent.
pub(crate) fn departed_info(id: NodeId) -> Option<Value> {
    STATE.with(|st| {
        st.borrow().departed.get(&id).map(|d| {
            json!({
                "id": d.id, "name": d.name, "type": d.ty, "chain": d.chain, "dag": d.dag, "owner": d.owner,
                "path": d.path, "full": d.full, "parent": d.parent, "children": [], "intermediate": false, "departed": true,
            })
        })
    })
}

fn watches_where(f: impl Fn(&Watch) -> bool) -> Vec<i64> {
    WATCHES.with(|w| w.borrow().iter().filter(|(_, x)| f(x)).map(|(id, _)| *id).collect())
}

fn any_watch(f: impl Fn(&Watch) -> bool) -> bool {
    WATCHES.with(|w| w.borrow().values().any(f))
}

/// One callback, a compiled plug-in's or Python's; what a check callback answered.
fn call_one(s: &mut Session, id: i64, text: &str) -> Option<Value> {
    let python = WATCHES.with(|w| w.borrow().get(&id).map(|x| x.python))?;
    if python {
        #[cfg(feature = "python")]
        return crate::python::fire_message(s, id, text);
        #[cfg(not(feature = "python"))]
        return None;
    }
    STATE.with(|st| st.borrow_mut().reply = None);
    if let Err(e) = fire_callback(s, id, text) {
        super::defer(format!("// Warning: {e} //"), crate::core::LineKind::Warning);
    }
    STATE.with(|st| st.borrow_mut().reply.take())
}

fn send(s: &mut Session, ids: &[i64], payload: &Value) {
    let text = payload.to_string();
    // One callback may remove another: call_one skips those gone.
    for &id in ids {
        call_one(s, id, &text);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Scene messages

/// Sends a scene message. A check message's callbacks may stop what's coming (false) or name another file.
pub(crate) fn scene(s: &mut Session, msg: i64, payload: Value) -> (bool, Option<String>) {
    let ids = watches_where(|w| w.kind == "scene" && w.msg == msg);
    if ids.is_empty() {
        return (true, None);
    }
    let mut payload = payload;
    let mut file = None;
    for id in ids {
        if let Some(r) = call_one(s, id, &payload.to_string()) {
            if let Some(f) = r.get("file").and_then(Value::as_str).filter(|f| !f.is_empty()) {
                if payload.get("file").and_then(Value::as_str) != Some(f) {
                    file = Some(f.to_string());
                    payload["file"] = json!(f);
                }
            }
            if r.get("retCode").and_then(Value::as_bool) == Some(false) {
                return (false, file);
            }
        }
    }
    (true, file)
}

fn plain(s: &mut Session, msgs: &[i64], payload: &Value) {
    for &m in msgs {
        scene(s, m, payload.clone());
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum FileOp {
    New,
    Open,
    Save,
    Import,
    Export,
    Reference { deferred: bool },
    LoadReference,
    UnloadReference,
    RemoveReference,
    ImportReference,
}

fn on(a: &Args, k: &str) -> bool {
    a.flags.get(k).is_some_and(|v| v.as_bool().unwrap_or_else(|| !v.is_null() && v.as_i64() != Some(0)))
}

fn given(a: &Args, k: &str) -> bool {
    a.flags.get(k).is_some_and(|v| !v.is_null() && v.as_bool() != Some(false))
}

fn file_op(name: &str, a: &Args) -> Option<FileOp> {
    if name == "newScene" {
        return Some(FileOp::New);
    }
    if !matches!(name, "file" | "reference") || on(a, "query") {
        return None;
    }
    if given(a, "unloadReference") || given(a, "flushReference") {
        return Some(FileOp::UnloadReference);
    }
    if given(a, "loadReference") || given(a, "loadReferencePreview") {
        return Some(FileOp::LoadReference);
    }
    if on(a, "importReference") {
        return Some(FileOp::ImportReference);
    }
    if on(a, "removeReference") {
        return Some(FileOp::RemoveReference);
    }
    if name == "reference" {
        return (!on(a, "edit")).then_some(FileOp::Reference { deferred: given(a, "deferReference") });
    }
    if on(a, "new") {
        Some(FileOp::New)
    } else if on(a, "open") {
        Some(FileOp::Open)
    } else if on(a, "save") {
        Some(FileOp::Save)
    } else if on(a, "import") {
        Some(FileOp::Import)
    } else if on(a, "exportAll") || on(a, "exportSelected") {
        Some(FileOp::Export)
    } else if on(a, "reference") {
        Some(FileOp::Reference { deferred: given(a, "deferReference") })
    } else {
        None
    }
}

fn file_arg(s: &Session, name: &str, a: &Args, op: FileOp) -> String {
    let pos = a.positional.first().and_then(Value::as_str).map(str::to_string);
    match op {
        FileOp::Save => a.flags.get("rename").and_then(Value::as_str).map(str::to_string).or(pos).or_else(|| s.scene_path.clone()).unwrap_or_default(),
        _ if name == "reference" => a.flags.get("filename").and_then(Value::as_str).map(str::to_string).or(pos).unwrap_or_default(),
        _ => pos.unwrap_or_default(),
    }
}

/// The reference a load, unload, remove or import names: its node's id and its file.
fn reference_named(s: &Session, a: &Args, op: FileOp) -> (i64, String) {
    let flag = match op {
        FileOp::UnloadReference => ["unloadReference", "flushReference"].into_iter().find(|f| given(a, f)),
        FileOp::LoadReference => ["loadReference", "loadReferencePreview"].into_iter().find(|f| given(a, f)),
        _ => None,
    };
    let name = flag
        .and_then(|f| a.flags.get(f).and_then(Value::as_str).filter(|n| !n.is_empty()).map(str::to_string))
        .or_else(|| a.flags.get("referenceNode").and_then(Value::as_str).map(str::to_string))
        .or_else(|| a.positional.first().and_then(Value::as_str).map(str::to_string))
        .unwrap_or_default();
    match aztek_scene::references::reference_of(s, &name) {
        Some((rfn, path)) => (node_named(s, &rfn), path),
        None => (-1, name),
    }
}

fn node_named(s: &Session, name: &str) -> i64 {
    s.graph.find(name).ok().flatten().map_or(-1, |n| n as i64)
}

fn check(s: &mut Session, msg: i64, payload: Value) -> Result<Option<String>, CommandError> {
    match scene(s, msg, payload) {
        (true, f) => Ok(f),
        (false, _) => Err(CommandError::new(CANCELLED)),
    }
}

fn before(s: &mut Session, name: &str, a: &mut Args) -> Result<(), CommandError> {
    if name == "delete" {
        tell_doomed(s, a);
        return Ok(());
    }
    let Some(op) = file_op(name, a) else { return Ok(()) };
    // An open clears the scene on its way: that's no new scene of its own.
    let inner = STATE.with(|st| st.borrow().running.iter().any(|(o, _)| *o == op || (op == FileOp::New && *o == FileOp::Open)));
    if inner {
        STATE.with(|st| st.borrow_mut().running.push((op, false)));
        return Ok(());
    }
    before_file(s, name, a, op)?;
    STATE.with(|st| st.borrow_mut().running.push((op, true)));
    Ok(())
}

fn before_file(s: &mut Session, name: &str, a: &mut Args, op: FileOp) -> Result<(), CommandError> {
    let file = file_arg(s, name, a, op);
    let set_file = |a: &mut Args, f: Option<String>| {
        if let Some(f) = f {
            match a.positional.first_mut() {
                Some(p) => *p = json!(f),
                None => a.positional.push(json!(f)),
            }
            if name == "reference" && a.flags.contains_key("filename") {
                a.flags.insert("filename".into(), json!(f));
            }
        }
    };
    let fp = json!({ "file": file });
    match op {
        FileOp::New => {
            check(s, BEFORE_NEW_CHECK, json!({}))?;
            plain(s, &[BEFORE_NEW], &json!({}));
        }
        FileOp::Open => {
            let f = check(s, BEFORE_OPEN_CHECK, fp)?;
            set_file(a, f.clone());
            let fp = json!({ "file": f.unwrap_or(file) });
            plain(s, &[BEFORE_OPEN, BEFORE_FILE_READ], &fp);
        }
        FileOp::Save => {
            check(s, BEFORE_SAVE_CHECK, fp.clone())?;
            plain(s, &[BEFORE_SAVE], &fp);
        }
        FileOp::Import => {
            let f = check(s, BEFORE_IMPORT_CHECK, fp)?;
            set_file(a, f.clone());
            let fp = json!({ "file": f.unwrap_or(file) });
            plain(s, &[BEFORE_IMPORT, BEFORE_FILE_READ], &fp);
        }
        FileOp::Export => {
            let f = check(s, BEFORE_EXPORT_CHECK, fp)?;
            set_file(a, f.clone());
            let fp = json!({ "file": f.unwrap_or(file) });
            plain(s, &[BEFORE_EXPORT, EXPORT_STARTED], &fp);
        }
        FileOp::Reference { deferred } => {
            let f = check(s, BEFORE_CREATE_REFERENCE_CHECK, json!({ "file": file, "node": -1 }))?;
            set_file(a, f.clone());
            let file = f.unwrap_or(file);
            let fp = json!({ "file": file, "node": -1 });
            plain(s, &[BEFORE_REFERENCE, BEFORE_CREATE_REFERENCE, BEFORE_CREATE_REFERENCE_AND_RECORD_EDITS], &fp);
            if !deferred {
                check(s, BEFORE_LOAD_REFERENCE_CHECK, fp.clone())?;
                plain(s, &[BEFORE_LOAD_REFERENCE, BEFORE_LOAD_REFERENCE_AND_RECORD_EDITS, BEFORE_FILE_READ], &fp);
            }
        }
        FileOp::LoadReference => {
            let (node, path) = reference_named(s, a, op);
            let fp = json!({ "file": path, "node": node });
            check(s, BEFORE_LOAD_REFERENCE_CHECK, fp.clone())?;
            plain(s, &[BEFORE_LOAD_REFERENCE, BEFORE_LOAD_REFERENCE_AND_RECORD_EDITS, BEFORE_FILE_READ], &fp);
        }
        FileOp::UnloadReference => {
            let (node, path) = reference_named(s, a, op);
            plain(s, &[BEFORE_UNLOAD_REFERENCE], &json!({ "file": path, "node": node }));
        }
        FileOp::RemoveReference => {
            let (node, path) = reference_named(s, a, op);
            plain(s, &[BEFORE_REMOVE_REFERENCE], &json!({ "file": path, "node": node }));
        }
        FileOp::ImportReference => {
            let (node, path) = reference_named(s, a, op);
            plain(s, &[BEFORE_IMPORT_REFERENCE], &json!({ "file": path, "node": node }));
        }
    }
    Ok(())
}

fn after(s: &mut Session, name: &str, a: &Args, ok: bool) {
    // What the command did to the scene first: a file read's nodes arrive before its after messages.
    dispatch(s);
    if name == "namespace" && ok {
        if let Some(Value::Array(pair)) = a.flags.get("rename") {
            let (old, new) = (pair.first().and_then(Value::as_str).unwrap_or(""), pair.get(1).and_then(Value::as_str).unwrap_or(""));
            let ids = watches_where(|w| w.kind == "namespaceRenamed");
            send(s, &ids, &json!({ "strings": [new, old] }));
        }
        return;
    }
    let Some(op) = file_op(name, a) else { return };
    let sends = STATE.with(|st| {
        let mut st = st.borrow_mut();
        match st.running.iter().rposition(|(o, _)| *o == op) {
            Some(at) => st.running.remove(at).1,
            None => true,
        }
    });
    if sends && ok {
        after_file(s, name, a, op);
    }
}

fn after_file(s: &mut Session, name: &str, a: &Args, op: FileOp) {
    let file = file_arg(s, name, a, op);
    let fp = json!({ "file": file });
    match op {
        FileOp::New => plain(s, &[AFTER_NEW, SCENE_UPDATE], &json!({})),
        FileOp::Open => plain(s, &[AFTER_FILE_READ, AFTER_OPEN, AFTER_SCENE_READ_AND_RECORD_EDITS, SCENE_UPDATE], &fp),
        FileOp::Save => plain(s, &[AFTER_SAVE], &fp),
        FileOp::Import => plain(s, &[AFTER_FILE_READ, AFTER_IMPORT, AFTER_SCENE_READ_AND_RECORD_EDITS], &fp),
        FileOp::Export => plain(s, &[AFTER_EXPORT], &fp),
        FileOp::Reference { deferred } => {
            let (node, path) = aztek_scene::references::newest_reference(s).map(|(rfn, path)| (node_named(s, &rfn), path)).unwrap_or((-1, file));
            let fp = json!({ "file": path, "node": node });
            if deferred {
                plain(s, &[AFTER_REFERENCE, AFTER_CREATE_REFERENCE, AFTER_CREATE_REFERENCE_AND_RECORD_EDITS, SCENE_UPDATE], &fp);
            } else {
                plain(s, &[AFTER_FILE_READ, AFTER_REFERENCE, AFTER_CREATE_REFERENCE, AFTER_CREATE_REFERENCE_AND_RECORD_EDITS, AFTER_LOAD_REFERENCE, AFTER_LOAD_REFERENCE_AND_RECORD_EDITS, AFTER_SCENE_READ_AND_RECORD_EDITS, SCENE_UPDATE], &fp);
            }
        }
        FileOp::LoadReference => {
            let (node, path) = reference_named(s, a, op);
            plain(s, &[AFTER_FILE_READ, AFTER_LOAD_REFERENCE, AFTER_LOAD_REFERENCE_AND_RECORD_EDITS, AFTER_SCENE_READ_AND_RECORD_EDITS, SCENE_UPDATE], &json!({ "file": path, "node": node }));
        }
        FileOp::UnloadReference => {
            let (node, path) = reference_named(s, a, op);
            plain(s, &[AFTER_UNLOAD_REFERENCE, SCENE_UPDATE], &json!({ "file": path, "node": node }));
        }
        FileOp::RemoveReference => plain(s, &[AFTER_REMOVE_REFERENCE, SCENE_UPDATE], &json!({ "file": file, "node": -1 })),
        FileOp::ImportReference => plain(s, &[AFTER_IMPORT_REFERENCE, SCENE_UPDATE], &json!({ "file": file, "node": -1 })),
    }
}

/// The scene messages of a scene opened, saved or made from the interface rather than by a command.
pub(crate) fn before_ui(s: &mut Session, what: &str, file: &str) -> Result<(), String> {
    if !notes::watching() {
        return Ok(());
    }
    let mut a = Args::default();
    a.positional.push(json!(file));
    a.flags.insert(what.into(), json!(true));
    before(s, "file", &mut a).map_err(|e| e.to_string())
}

pub(crate) fn after_ui(s: &mut Session, what: &str, file: &str) {
    if !notes::watching() {
        return;
    }
    let mut a = Args::default();
    a.positional.push(json!(file));
    a.flags.insert(what.into(), json!(true));
    after(s, "file", &a, true);
}

/// The delete command's nodes, and what's below them: about to go, while they're still there.
fn tell_doomed(s: &mut Session, a: &Args) {
    let wanted = |w: &Watch| matches!(w.kind.as_str(), "aboutToDelete" | "preRemoval");
    if !any_watch(wanted) || a.flags.iter().any(|(k, v)| k != "queried" && !v.is_null() && v.as_bool() != Some(false)) {
        return;
    }
    let names: Vec<String> = if a.positional.is_empty() {
        s.graph.selection().iter().map(|&id| s.graph.path(id)).collect()
    } else {
        a.positional.iter().filter_map(Value::as_str).map(str::to_string).collect()
    };
    let mut doomed = Vec::new();
    for n in names {
        if let Ok(Some(id)) = s.graph.find(&n) {
            for d in std::iter::once(id).chain(s.graph.descendants(id)) {
                if !doomed.contains(&d) {
                    doomed.push(d);
                }
            }
        }
    }
    for id in doomed.into_iter().rev() {
        for kind in ["aboutToDelete", "preRemoval"] {
            let ids = watches_where(|w| w.kind == kind && w.node == id as i64);
            if !ids.is_empty() {
                STATE.with(|st| st.borrow_mut().told.insert(id));
                send(s, &ids, &json!({ "node": id }));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Graph messages

fn plug_text(s: &Session, departed: &HashMap<NodeId, Departed>, p: &PlugRef) -> String {
    if s.graph.node(p.node).is_some() {
        return s.graph.plug_name(p);
    }
    departed
        .get(&p.node)
        .and_then(|d| types::find(&d.ty).and_then(|t| t.attrs.get(p.attr.max(0) as usize).map(|a| format!("{}.{}", d.name, a.name))))
        .unwrap_or_default()
}

fn chain_of(s: &Session, departed: &HashMap<NodeId, Departed>, id: NodeId) -> Vec<String> {
    if let Some(n) = s.graph.node(id) {
        let mut out = Vec::new();
        let mut t = Some(n.ty());
        while let Some(ty) = t {
            out.push(ty.name.clone());
            t = ty.base.as_deref();
        }
        return out;
    }
    departed.get(&id).map(|d| d.chain.clone()).unwrap_or_default()
}

fn type_matches(filter: &str, chain: &[String]) -> bool {
    filter.is_empty() || filter == "dependNode" || chain.iter().any(|c| c == filter)
}

fn is_dag(s: &Session, departed: &HashMap<NodeId, Departed>, id: NodeId) -> bool {
    s.graph.node(id).map(|n| n.ty().dag).or_else(|| departed.get(&id).map(|d| d.dag)).unwrap_or(false)
}

/// DAG messages: `child` gained or lost `parent`. Every-node callbacks hear each; a node's own hear what concerns it.
fn dag_calls(out: &mut Vec<(Vec<i64>, Value)>, child: NodeId, parent: NodeId, msgs: &[i64]) {
    for &m in msgs {
        let about = if m == CHILD_ADDED || m == CHILD_REMOVED || m == CHILD_REORDERED { parent } else { child };
        let ids = watches_where(|w| w.kind == "dag" && (w.msg == m || w.msg == -1) && (w.node == -1 || w.node == about as i64));
        if !ids.is_empty() {
            out.push((ids, json!({ "msg": m, "child": child, "parent": parent })));
        }
    }
}

fn attr_calls(out: &mut Vec<(Vec<i64>, Value)>, node: NodeId, msg: i64, plug: &str, other: &str) {
    let ids = watches_where(|w| w.kind == "attributeChanged" && w.node == node as i64);
    if !ids.is_empty() {
        out.push((ids, json!({ "msg": msg, "plug": plug, "other": other })));
    }
}

/// Sends what the graph noted, the time if it moved, and matrices that changed. Again while callbacks change more.
pub(crate) fn dispatch(s: &mut Session) {
    if !notes::watching() || aztek_scene::graph::in_compute(|_| ()).is_some() {
        return;
    }
    if STATE.with(|st| std::mem::replace(&mut st.borrow_mut().dispatching, true)) {
        return;
    }
    for _ in 0..64 {
        let events = notes::take(s.graph.serial());
        let timed = time_calls(s);
        let matrices = matrix_calls(s);
        if events.is_empty() && timed.is_empty() && matrices.is_empty() {
            break;
        }
        for e in &events {
            if let Event::Removed(d) = e {
                STATE.with(|st| st.borrow_mut().departed.insert(d.id, d.clone()));
            }
        }
        let departed = STATE.with(|st| st.borrow().departed.clone());
        let mut calls = calls_for(s, &events, &departed);
        calls.extend(timed);
        calls.extend(matrices);
        for (ids, payload) in calls {
            send(s, &ids, &payload);
        }
        // Node callbacks on nodes gone with the scene go too.
        let gone: Vec<NodeId> = events.iter().filter_map(|e| if let Event::Removed(d) = e { d.cleared.then_some(d.id) } else { None }).collect();
        if !gone.is_empty() {
            let set: HashSet<NodeId> = gone.into_iter().collect();
            for id in watches_where(|w| w.node >= 0 && set.contains(&(w.node as NodeId))) {
                unwatch(id);
            }
        }
        STATE.with(|st| {
            let mut st = st.borrow_mut();
            st.departed.clear();
            st.told.clear();
        });
    }
    STATE.with(|st| st.borrow_mut().dispatching = false);
}

fn calls_for(s: &mut Session, events: &[Event], departed: &HashMap<NodeId, Departed>) -> Vec<(Vec<i64>, Value)> {
    let mut out: Vec<(Vec<i64>, Value)> = Vec::new();
    let mut dirty_from: Vec<(NodeId, String)> = Vec::new();
    for e in events {
        match e {
            Event::Added(id) => {
                let chain = chain_of(s, departed, *id);
                let ids = watches_where(|w| w.kind == "nodeAdded" && type_matches(&w.ty, &chain));
                if !ids.is_empty() {
                    out.push((ids, json!({ "node": id })));
                }
                if is_dag(s, departed, *id) {
                    let parent = s.graph.node(*id).map(|_| s.graph.parent(*id)).unwrap_or(0);
                    dag_calls(&mut out, *id, parent, &[PARENT_ADDED, CHILD_ADDED]);
                }
            }
            Event::Removed(d) => {
                let told = STATE.with(|st| st.borrow().told.contains(&d.id));
                if !told {
                    for kind in ["aboutToDelete", "preRemoval"] {
                        let ids = watches_where(|w| w.kind == kind && w.node == d.id as i64);
                        if !ids.is_empty() {
                            out.push((ids, json!({ "node": d.id })));
                        }
                    }
                }
                if d.dag {
                    dag_calls(&mut out, d.id, d.parent, &[PARENT_REMOVED, CHILD_REMOVED]);
                }
                let ids = watches_where(|w| w.kind == "nodeRemoved" && type_matches(&w.ty, &d.chain));
                if !ids.is_empty() {
                    out.push((ids, json!({ "node": d.id })));
                }
                if d.cleared {
                    let ids = watches_where(|w| w.kind == "destroyed" && w.node == d.id as i64);
                    if !ids.is_empty() {
                        out.push((ids, json!({})));
                    }
                }
            }
            Event::Renamed { id, old } => {
                let ids = watches_where(|w| w.kind == "nameChanged" && (w.node == *id as i64 || w.node == -1));
                if !ids.is_empty() {
                    out.push((ids, json!({ "node": id, "old": old })));
                }
            }
            Event::Reparented { id, old_parent, new_parent } => {
                if old_parent == new_parent {
                    dag_calls(&mut out, *id, *new_parent, &[CHILD_REORDERED]);
                } else {
                    dag_calls(&mut out, *id, *old_parent, &[PARENT_REMOVED, CHILD_REMOVED]);
                    dag_calls(&mut out, *id, *new_parent, &[PARENT_ADDED, CHILD_ADDED]);
                }
            }
            Event::Set(p) => {
                let name = plug_text(s, departed, p);
                attr_calls(&mut out, p.node, ATTRIBUTE_SET | INCOMING_DIRECTION, &name, "");
                dirty_from.push((p.node, name));
            }
            Event::ElementRemoved(p) => {
                let name = plug_text(s, departed, p);
                attr_calls(&mut out, p.node, ATTRIBUTE_ARRAY_REMOVED, &name, "");
                dirty_from.push((p.node, name));
            }
            Event::Connected { src, dst, made } => {
                let (sn, dn) = (plug_text(s, departed, src), plug_text(s, departed, dst));
                let ids = watches_where(|w| w.kind == "preConnection");
                if !ids.is_empty() {
                    out.push((ids, json!({ "src": sn, "dst": dn, "made": made })));
                }
                let ids = watches_where(|w| w.kind == "connection");
                if !ids.is_empty() {
                    out.push((ids, json!({ "src": sn, "dst": dn, "made": made })));
                }
                let m = if *made { CONNECTION_MADE } else { CONNECTION_BROKEN };
                attr_calls(&mut out, src.node, m, &sn, &dn);
                attr_calls(&mut out, dst.node, m | INCOMING_DIRECTION, &dn, &sn);
                dirty_from.push((dst.node, dn));
            }
            Event::Locked { plug, locked } => {
                let name = plug_text(s, departed, plug);
                attr_calls(&mut out, plug.node, if *locked { ATTRIBUTE_LOCKED } else { ATTRIBUTE_UNLOCKED }, &name, "");
            }
            Event::Keyable { plug, keyable } => {
                let name = plug_text(s, departed, plug);
                attr_calls(&mut out, plug.node, if *keyable { ATTRIBUTE_KEYABLE } else { ATTRIBUTE_UNKEYABLE }, &name, "");
            }
            Event::AttrAdded { node, attr, .. } => {
                let name = plug_text(s, departed, &PlugRef::new(*node, *attr as i32));
                attr_calls(&mut out, *node, ATTRIBUTE_ADDED, &name, "");
                let ids = watches_where(|w| w.kind == "attributeAddedOrRemoved" && w.node == *node as i64);
                if !ids.is_empty() {
                    out.push((ids, json!({ "msg": ATTRIBUTE_ADDED, "plug": name })));
                }
            }
            Event::AttrRemoved { node, name, .. } => {
                let node_name = s.graph.node(*node).map(|n| n.name().to_string()).or_else(|| departed.get(node).map(|d| d.name.clone())).unwrap_or_default();
                let full = format!("{node_name}.{name}");
                attr_calls(&mut out, *node, ATTRIBUTE_REMOVED, &full, "");
                let ids = watches_where(|w| w.kind == "attributeAddedOrRemoved" && w.node == *node as i64);
                if !ids.is_empty() {
                    out.push((ids, json!({ "msg": ATTRIBUTE_REMOVED, "plug": full })));
                }
            }
            Event::AttrRenamed { node, attr, .. } => {
                let name = plug_text(s, departed, &PlugRef::new(*node, *attr as i32));
                attr_calls(&mut out, *node, ATTRIBUTE_RENAMED, &name, "");
            }
            Event::Uuid(id) => {
                let ids = watches_where(|w| w.kind == "uuidChanged" && w.node == *id as i64);
                if !ids.is_empty() {
                    let uuid = s.graph.node(*id).map(|n| n.uuid().to_string()).unwrap_or_default();
                    out.push((ids, json!({ "node": id, "uuid": uuid })));
                }
            }
        }
    }
    if !dirty_from.is_empty() && any_watch(|w| w.kind == "dirty" || w.kind == "dirtyPlug") {
        dirty_calls(s, &mut out, dirty_from);
    }
    out
}

/// A node made dirty and everything downstream of it: node dirty and dirty plug messages, once per node and plug.
fn dirty_calls(s: &Session, out: &mut Vec<(Vec<i64>, Value)>, start: Vec<(NodeId, String)>) {
    let mut down: HashMap<NodeId, Vec<PlugRef>> = HashMap::new();
    for (dst, src) in s.graph.connections() {
        down.entry(src.node).or_default().push(dst);
    }
    let mut seen_nodes: HashSet<NodeId> = HashSet::new();
    let mut seen_plugs: HashSet<String> = HashSet::new();
    let mut queue: std::collections::VecDeque<(NodeId, String)> = start.into();
    while let Some((node, plug)) = queue.pop_front() {
        if seen_plugs.insert(plug.clone()) {
            let ids = watches_where(|w| w.kind == "dirtyPlug" && w.node == node as i64);
            if !ids.is_empty() {
                out.push((ids, json!({ "node": node, "plug": plug })));
            }
        }
        if !seen_nodes.insert(node) {
            continue;
        }
        let ids = watches_where(|w| w.kind == "dirty" && w.node == node as i64);
        if !ids.is_empty() {
            out.push((ids, json!({ "node": node })));
        }
        for d in down.get(&node).into_iter().flatten() {
            if s.graph.node(d.node).is_some() {
                queue.push_back((d.node, s.graph.plug_name(d)));
            }
        }
    }
}

/// The time callbacks, in the reference's order, when the time moved.
fn time_calls(s: &Session) -> Vec<(Vec<i64>, Value)> {
    if !any_watch(|w| w.kind == "time") {
        return Vec::new();
    }
    let now = s.graph.time();
    let moved = STATE.with(|st| {
        let mut st = st.borrow_mut();
        let moved = st.last_time.is_some_and(|t| (t - now).abs() > 1e-12);
        st.last_time = Some(now);
        moved
    });
    if !moved {
        return Vec::new();
    }
    // timeChange, forceUpdate, then the delayed ones and the run-up.
    [0, 3, 1, 2]
        .iter()
        .filter_map(|&m| {
            let ids = watches_where(|w| w.kind == "time" && w.msg == m);
            (!ids.is_empty()).then(|| (ids, json!({ "time": now })))
        })
        .collect()
}

/// The parts of a transform's local matrix, with MDagMessage::MatrixModifiedFlags' bit for each.
const MATRIX_PARTS: &[(&str, u32)] = &[
    ("scaleX", 0), ("scaleY", 1), ("scaleZ", 2),
    ("shearXY", 3), ("shearXZ", 4), ("shearYZ", 5),
    ("rotateX", 6), ("rotateY", 7), ("rotateZ", 8),
    ("translateX", 9), ("translateY", 10), ("translateZ", 11),
    ("scalePivotX", 12), ("scalePivotY", 13), ("scalePivotZ", 14),
    ("rotatePivotX", 15), ("rotatePivotY", 16), ("rotatePivotZ", 17),
    ("scalePivotTranslateX", 18), ("scalePivotTranslateY", 19), ("scalePivotTranslateZ", 20),
    ("rotatePivotTranslateX", 21), ("rotatePivotTranslateY", 22), ("rotatePivotTranslateZ", 23),
    ("rotateAxisX", 24), ("rotateAxisY", 25), ("rotateAxisZ", 26),
    ("rotateOrder", 27),
];

fn matrix_values(s: &mut Session, id: NodeId) -> Vec<f64> {
    let Some(path) = s.graph.node(id).map(|_| s.graph.full_path(id)) else { return Vec::new() };
    MATRIX_PARTS
        .iter()
        .map(|(a, _)| {
            let g = &mut s.graph;
            g.plug(&format!("{path}.{a}")).ok().and_then(|p| g.value(&p).ok()).and_then(|v| v.scalar()).unwrap_or(0.0)
        })
        .collect()
}

fn matrix_chain(s: &mut Session, id: NodeId, world: bool) -> Vec<(NodeId, Vec<f64>)> {
    let mut out = Vec::new();
    let mut at = id;
    while at > 0 && s.graph.node(at).is_some() {
        out.push((at, matrix_values(s, at)));
        if !world {
            break;
        }
        at = s.graph.parent(at);
    }
    out
}

fn matrix_calls(s: &mut Session) -> Vec<(Vec<i64>, Value)> {
    let watched: Vec<(i64, NodeId, bool)> = WATCHES.with(|w| w.borrow().iter().filter(|(_, x)| (x.kind == "matrix" || x.kind == "worldMatrix") && x.node > 0).map(|(id, x)| (*id, x.node as NodeId, x.kind == "worldMatrix")).collect());
    let mut out = Vec::new();
    for (id, node, world) in watched {
        if s.graph.node(node).is_none() {
            continue;
        }
        let now = matrix_chain(s, node, world);
        let was = STATE.with(|st| st.borrow_mut().matrices.insert(id, now.clone())).unwrap_or_default();
        let mut flags: u32 = 0;
        for (i, (n, vals)) in now.iter().enumerate() {
            match was.get(i).filter(|(m, _)| m == n) {
                Some((_, old)) => {
                    for (k, (_, bit)) in MATRIX_PARTS.iter().enumerate() {
                        if (old.get(k).copied().unwrap_or(0.0) - vals[k]).abs() > 1e-12 {
                            flags |= 1 << bit;
                        }
                    }
                }
                // A new ancestor: everything above may have moved.
                None => flags |= (1 << 28) - 1,
            }
        }
        if now.len() != was.len() {
            flags |= (1 << 28) - 1;
        }
        if flags != 0 {
            out.push((vec![id], json!({ "node": node, "flags": flags })));
        }
    }
    out
}
