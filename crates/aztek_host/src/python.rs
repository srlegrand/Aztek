//! Embedded Python: aztek.cmds (and maya.cmds) call the commands; print goes to the script history.
//!
//! Python can be called back while the host is borrowed: a plugin command runs from inside `execute`, undoIt from inside
//! the undo stack. There the session is moved for the callback's length into a nested Core which the `_aztek` functions
//! then use (`nested`), so nothing aliases it. A plugin node's compute may run on a worker thread and may not reach the
//! host at all (`IN_COMPUTE`), and the functions here let go of the GIL before touching the host, so that a compute
//! waited on by the main thread can take it.

use std::cell::{Cell, RefCell};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread::ThreadId;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::graph::Graph;
use aztek_scene::node::NodeData;
use aztek_scene::plug::{NodeId, PlugType};
use aztek_scene::scripted;
use aztek_scene::Session;
use aztek_undo::FnAction;
use std::ffi::CString;
use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict};
use serde_json::{json, Map, Value};

use crate::core::{Core, LineKind};

thread_local! {
    /// This thread has run Python through the host, so its commands reach its own core.
    static ENTERED: Cell<bool> = const { Cell::new(false) };
    /// The core a callback's Python talks to; null when not inside one.
    static NESTED: Cell<*mut Core> = const { Cell::new(std::ptr::null_mut()) };
    static IN_COMPUTE: Cell<bool> = const { Cell::new(false) };
    /// What undoIt and redoIt run their commands with: the stack holds only the graph.
    static COMMANDS: RefCell<Vec<CommandDef<Session>>> = const { RefCell::new(Vec::new()) };
}

static MAIN: OnceLock<ThreadId> = OnceLock::new();
/// The Python object holding the plugin entry points; set when aztek.api.OpenAztek is imported.
static HOOKS: OnceLock<Py<PyAny>> = OnceLock::new();
/// Lines written where the history can't be reached: inside computes and nested calls.
static DEFERRED: Mutex<Vec<(String, LineKind)>> = Mutex::new(Vec::new());

const NO_HOST: &str = "(kFailure): Commands cannot run inside compute";

fn usable() -> bool {
    !IN_COMPUTE.with(Cell::get) && (ENTERED.with(Cell::get) || MAIN.get().is_some_and(|t| *t == std::thread::current().id()))
}

/// Whether the host lent its session to Python on this thread: a callback or command. What Python runs there may start
/// a nested Qt event loop, whose timers and paint calls reach the host again. Not inside a node's compute: the graph is
/// mid-evaluation there, and a compute that pumps events must not let the shell in.
pub(crate) fn in_host_callback() -> bool {
    !NESTED.with(Cell::get).is_null() && !IN_COMPUTE.with(Cell::get)
}

/// Runs `f` as a plug-in command or undo step does, with the session moved out into a nested core.
#[cfg(test)]
pub(crate) fn test_nested_session<R>(s: &mut Session, f: impl FnOnce() -> R) -> R {
    nested(s, f)
}

/// Runs `f` as inside a node's compute.
#[cfg(test)]
pub(crate) fn test_in_compute<R>(f: impl FnOnce() -> R) -> R {
    let was = IN_COMPUTE.with(|c| c.replace(true));
    let r = f();
    IN_COMPUTE.with(|c| c.set(was));
    r
}

/// Runs `f` as if from a Python callback the host lent `core` to, without starting the interpreter.
#[cfg(test)]
pub(crate) fn test_nested<R>(core: *mut Core, f: impl FnOnce() -> R) -> R {
    let saved = NESTED.with(|n| n.replace(core));
    let r = f();
    NESTED.with(|n| n.set(saved));
    r
}

fn with_core<R>(f: impl FnOnce(&mut Core) -> R) -> Result<R, String> {
    if !usable() {
        return Err(NO_HOST.to_string());
    }
    let p = NESTED.with(Cell::get);
    if p.is_null() {
        Ok(crate::with(|h| f(&mut h.core)))
    } else {
        // Set by `nested`, whose frame owns it and does nothing else until the callback returns.
        Ok(f(unsafe { &mut *p }))
    }
}

/// with_core without the GIL.
fn host<R: Send>(py: Python<'_>, f: impl FnOnce(&mut Core) -> R + Send) -> PyResult<R> {
    py.detach(|| with_core(f)).map_err(PyRuntimeError::new_err)
}

/// Runs `f` with the session moved into a nested Core: for calling Python from a command or an undo step.
fn nested<R>(s: &mut Session, f: impl FnOnce() -> R) -> R {
    let mut core = Core::nested(Session::hollow());
    std::mem::swap(s, &mut core.s);
    let prev = NESTED.with(|n| n.replace(&mut core));
    let r = f();
    NESTED.with(|n| n.set(prev));
    std::mem::swap(s, &mut core.s);
    if !core.log.is_empty() {
        let mut d = DEFERRED.lock().unwrap();
        d.extend(core.log.drain(..).map(|l| (l.text, l.kind)));
    }
    r
}

/// Lines written since the last call, for the history.
pub fn take_deferred() -> Vec<(String, LineKind)> {
    std::mem::take(&mut *DEFERRED.lock().unwrap())
}

fn hook_call<'py>(py: Python<'py>, method: &str, args: impl pyo3::call::PyCallArgs<'py>) -> Result<String, String> {
    let hooks = HOOKS.get().ok_or("the Python API is not loaded")?;
    hooks
        .bind(py)
        .call_method1(method, args)
        .and_then(|v| v.extract::<String>())
        .map_err(|e| e.to_string())
}

fn answer(out: Result<String, String>) -> CmdResult {
    let v: Value = serde_json::from_str(&out.map_err(CommandError::new)?).map_err(|e| CommandError::new(e.to_string()))?;
    match v.get("error").and_then(Value::as_str) {
        Some(e) => Err(CommandError::new(e)),
        None => Ok(v.get("ok").cloned().unwrap_or(Value::Null)),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands from plugins

/// undoIt or redoIt of a plugin command. The stack hands over only the graph, so a session is put together around it.
fn replay(g: &mut Graph, inst: &Py<PyAny>, method: &str) {
    let mut s = Session::hollow();
    COMMANDS.with(|c| {
        for d in c.borrow().iter() {
            s.commands.register(d.clone());
        }
    });
    s.commands.set_previewing(true);
    std::mem::swap(g, &mut s.graph);
    nested(&mut s, || {
        Python::attach(|py| {
            if let Err(e) = hook_call(py, "command_replay", (inst.clone_ref(py), method)) {
                DEFERRED.lock().unwrap().push((format!("# Error: {e} #"), LineKind::Error));
            }
        })
    });
    std::mem::swap(g, &mut s.graph);
}

fn run_plugin_command(s: &mut Session, name: &str, a: &Args) -> CmdResult {
    let input = json!({"args": a.positional, "flags": a.flags}).to_string();
    let was_on = s.graph.undo_stack().enabled();
    let (out, undo) = nested(s, || {
        Python::attach(|py| -> (Result<String, String>, Option<Py<PyAny>>) {
            let Some(hooks) = HOOKS.get() else { return (Err("the Python API is not loaded".into()), None) };
            let hooks = hooks.bind(py);
            let made = hooks.call_method1("command_new", (name,)).and_then(|t| t.extract::<(Py<PyAny>, bool)>());
            let (inst, undoable) = match made {
                Ok(x) => x,
                Err(e) => return (Err(e.to_string()), None),
            };
            // An undoable command answers for its whole effect through undoIt: what it runs inside isn't also recorded.
            if undoable {
                let _ = py.detach(|| with_core(|c| c.s.graph.undo_stack_mut().set_enabled(false)));
            }
            let out = hooks.call_method1("command_do", (inst.clone_ref(py), input.as_str())).and_then(|v| v.extract::<String>()).map_err(|e| e.to_string());
            if undoable {
                let _ = py.detach(|| with_core(|c| c.s.graph.undo_stack_mut().set_enabled(was_on)));
            }
            (out, undoable.then_some(inst))
        })
    });
    let r = answer(out)?;
    if let Some(inst) = undo {
        let redo_inst = Python::attach(|py| inst.clone_ref(py));
        s.graph.undo_stack_mut().record(FnAction::new(move |g: &mut Graph| replay(g, &inst, "undoIt"), move |g: &mut Graph| replay(g, &redo_inst, "redoIt")));
    }
    Ok(r)
}

fn keep_commands(s: &Session) {
    COMMANDS.with(|c| *c.borrow_mut() = s.commands.all().cloned().collect());
}

#[pyfunction]
fn register_command(py: Python<'_>, name: &str, owner: &str, flags: &str) -> PyResult<()> {
    let flags: Vec<Value> = serde_json::from_str(flags).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    host(py, |c| {
        let n = name.to_string();
        let mut def = CommandDef::<Session>::new(name, "A command from a plug-in.", move |s, a| run_plugin_command(s, &n, a)).positional("arguments");
        def.owner = owner.to_string();
        def.scripted = true;
        for f in &flags {
            let text = |k: &str| f.get(k).and_then(Value::as_str).unwrap_or("");
            let ty = match text("type") {
                "bool" => ArgType::Bool,
                "int" => ArgType::Int,
                "float" => ArgType::Float,
                "string" => ArgType::String,
                "list" => ArgType::List,
                _ => ArgType::Any,
            };
            def = def.flag(FlagDef::new(text("name"), text("short"), ty, Value::Null, ""));
        }
        c.s.register(def);
        keep_commands(&c.s);
    })
}

/// A Python function deciding where asset paths resolve: pxr.Ar.SetResolveHook. It returns a file or None, and must not
/// touch stages: it runs while one is composing.
#[pyfunction]
fn set_usd_resolve_hook(func: Option<Py<PyAny>>) {
    match func {
        Some(f) => aztek_usd::extras::set_resolve_hook(Some(Arc::new(move |asset: &str| {
            Python::attach(|py| f.call1(py, (asset,)).ok().and_then(|r| r.extract::<Option<String>>(py).ok()).flatten())
        }))),
        None => aztek_usd::extras::set_resolve_hook(None),
    }
}

#[pyfunction]
fn deregister_command(py: Python<'_>, name: &str) -> PyResult<()> {
    host(py, |c| {
        c.s.commands.unregister(name);
        keep_commands(&c.s);
    })
}

/// kind: 0 a dependency node, 1 a deformer, 2 a locator, 3 a transform, 4 a constraint, 5 a shape, 6 an IK solver (the ABI's numbers).
#[pyfunction]
#[pyo3(signature = (name, owner, attrs, kind=0, type_id=0))]
fn register_node(py: Python<'_>, name: &str, owner: &str, attrs: &str, kind: i32, type_id: u32) -> PyResult<()> {
    let attrs: Vec<Value> = serde_json::from_str(attrs).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    let kind = match kind {
        1 => scripted::NativeKind::Deformer,
        2 => scripted::NativeKind::Locator,
        3 => scripted::NativeKind::Transform,
        4 => scripted::NativeKind::Constraint,
        5 => scripted::NativeKind::Shape,
        6 => scripted::NativeKind::IkSolver,
        _ => scripted::NativeKind::Depend,
    };
    py.detach(|| scripted::register_script_type(name, owner, &attrs, kind)).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    scripted::set_type_id(name, type_id);
    Ok(())
}

/// Whether openMayaPref -errlog is on.
#[pyfunction]
fn api_errlog() -> bool {
    crate::sys_info::errlog_on()
}

/// A failed API call, for openMayaPref -errlog: written to the error log when it is on.
#[pyfunction]
fn api_error(text: &str, stack: &str) {
    crate::sys_info::log_api_error(text, stack);
}

/// A plug-in's file translator (MPxFileTranslator): `file` hands its type's files to the plug-in's reader and writer.
#[pyfunction]
#[pyo3(signature = (name, owner, extension, read, write, default_options="", options_script=""))]
fn register_translator(name: &str, owner: &str, extension: &str, read: bool, write: bool, default_options: &str, options_script: &str) {
    static HOOKED: std::sync::Once = std::sync::Once::new();
    HOOKED.call_once(|| {
        aztek_scene::ma_file::set_translator_hook(Arc::new(|s: &mut Session, name: &str, mode: &str, path: &str, options: &str| {
            let (name, mode, path, options) = (name.to_string(), mode.to_string(), path.to_string(), options.to_string());
            answer(nested(s, || Python::attach(|py| hook_call(py, "translate", (name.as_str(), mode.as_str(), path.as_str(), options.as_str())))))
        }));
    });
    aztek_scene::ma_file::register_plugin_translator(aztek_scene::ma_file::PluginTranslator {
        name: name.to_string(),
        extension: extension.to_string(),
        read,
        write,
        owner: owner.to_string(),
        default_options: default_options.to_string(),
        options_script: options_script.to_string(),
    });
}

#[pyfunction]
fn deregister_translator(name: &str) {
    aztek_scene::ma_file::deregister_plugin_translator(name);
}

// ---------------------------------------------------------------------------------------------------------------------
// Plug-in data types (MPxData): a value is the Python object, written to and read from a file through its class.

struct PyData {
    obj: Py<PyAny>,
    kind: String,
}

impl std::fmt::Debug for PyData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} data", self.kind)
    }
}

impl aztek_scene::plug::Data for PyData {
    fn kind(&self) -> &str {
        &self.kind
    }
    fn to_json(&self) -> Value {
        let text = Python::attach(|py| hook_call(py, "data_write", (self.obj.clone_ref(py),))).unwrap_or_default();
        json!({"kind": self.kind, "pluginData": true, "text": text})
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

/// Values handed across a compute: what node_values gave Python, what Python gives back. Keys are taken once.
static DATA_HANDOFF: Mutex<Vec<(u64, aztek_scene::plug::DataPtr)>> = Mutex::new(Vec::new());
static NEXT_HANDOFF: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn hand_off(d: aztek_scene::plug::DataPtr) -> u64 {
    let k = NEXT_HANDOFF.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    DATA_HANDOFF.lock().unwrap().push((k, d));
    k
}

fn take_handoff(k: u64) -> Option<aztek_scene::plug::DataPtr> {
    let mut v = DATA_HANDOFF.lock().unwrap();
    let at = v.iter().position(|(x, _)| *x == k)?;
    Some(v.swap_remove(at).1)
}

/// A data type's name, so values of it read back from a file.
#[pyfunction]
fn register_data(name: &str) {
    let kind = name.to_string();
    aztek_scene::plug::data::register_kind(
        name,
        Arc::new(move |j: &Value| {
            let text = j.get("text").and_then(Value::as_str).unwrap_or("").to_string();
            Python::attach(|py| -> Result<aztek_scene::plug::DataPtr, String> {
                let hooks = HOOKS.get().ok_or("the Python API is not loaded")?;
                let obj = hooks.bind(py).call_method1("data_read", (kind.as_str(), text.as_str())).map_err(|e| e.to_string())?;
                if obj.is_none() {
                    return Err(format!("{kind}: its readASCII refused the value"));
                }
                Ok(Arc::new(PyData { obj: obj.unbind(), kind: kind.clone() }))
            })
            .map_err(aztek_scene::error::GraphError::invalid)
        }),
    );
}

#[pyfunction]
fn deregister_data(name: &str) {
    aztek_scene::plug::data::remove(name);
}

/// The Python object of a value node_values handed over.
#[pyfunction]
fn data_get(py: Python<'_>, key: u64) -> Option<Py<PyAny>> {
    let v = DATA_HANDOFF.lock().unwrap().iter().find(|(k, _)| *k == key).map(|(_, d)| d.clone())?;
    v.as_any().downcast_ref::<PyData>().map(|d| d.obj.clone_ref(py))
}

/// A value a compute gives back, held until its outputs are written.
#[pyfunction]
fn data_put(obj: Py<PyAny>, kind: &str) -> u64 {
    hand_off(Arc::new(PyData { obj, kind: kind.to_string() }))
}

/// A Python callback of the messages (MSceneMessage and the others); what a check callback answered.
pub(crate) fn fire_message(s: &mut Session, id: i64, payload: &str) -> Option<Value> {
    if ensure().is_err() || HOOKS.get().is_none() {
        return None;
    }
    let out = nested(s, || Python::attach(|py| hook_call(py, "message", (id, payload))));
    match out {
        Ok(t) if !t.is_empty() => serde_json::from_str(&t).ok(),
        Ok(_) => None,
        Err(e) => {
            DEFERRED.lock().unwrap().push((format!("# Error: {e} #"), LineKind::Error));
            None
        }
    }
}

/// A Python IK solver's solve, from inside the solve node's compute: each joint's rotate.
pub(crate) fn ik_solve(ty: &str, input: &Value) -> Option<Vec<glam::DVec3>> {
    if HOOKS.get().is_none() {
        return None;
    }
    let was = IN_COMPUTE.with(|c| c.replace(true));
    let out = Python::attach(|py| hook_call(py, "ik_solve", (ty, input.to_string())));
    IN_COMPUTE.with(|c| c.set(was));
    let out = match out {
        Ok(t) => t,
        Err(e) => {
            DEFERRED.lock().unwrap().push((format!("# Error: {e} #"), LineKind::Error));
            return None;
        }
    };
    let v: Value = serde_json::from_str(&out).ok()?;
    Some(
        v.get("rotate")?
            .as_array()?
            .iter()
            .map(|r| {
                let n = |i: usize| r.get(i).and_then(Value::as_f64).unwrap_or(0.0);
                glam::DVec3::new(n(0), n(1), n(2))
            })
            .collect(),
    )
}

#[pyfunction]
fn deregister_node(name: &str) {
    scripted::deregister_type(name);
}

#[pyfunction]
fn set_hooks(hooks: Py<PyAny>) {
    let _ = HOOKS.set(hooks);
}

/// A plugin node's inputs as JSON: {attr: value}, an array attr as [[index, value], ...].
fn node_values(d: &NodeData, keys: &mut Vec<u64>) -> String {
    let mut o = Map::new();
    for (i, a) in d.ty().attrs.iter().enumerate() {
        if a.ty == PlugType::Data && a.category.starts_with(scripted::PLUGIN_DATA) {
            // A plug-in data value goes across by key.
            let mut one = |v: &aztek_scene::plug::PlugValue| match v {
                aztek_scene::plug::PlugValue::Data(Some(d)) => {
                    let k = hand_off(d.clone());
                    keys.push(k);
                    json!({"$data": k})
                }
                _ => Value::Null,
            };
            let v = if a.is_array() { Value::Array(d.elements(i).iter().map(|(k, v)| json!([k, one(v)])).collect()) } else { one(d.value(i)) };
            o.insert(a.name.clone(), v);
            continue;
        }
        if a.ty == PlugType::Mesh && !a.is_array() {
            // A mesh goes across by key; Python asks for its arrays when it reads it.
            if let aztek_scene::plug::PlugValue::Mesh(Some(m)) = d.value(i) {
                let k = mesh_hand_off(m.clone());
                keys.push(k);
                o.insert(a.name.clone(), json!({"$mesh": k}));
            } else {
                o.insert(a.name.clone(), Value::Null);
            }
            continue;
        }
        if matches!(a.ty, PlugType::Mesh | PlugType::Data) {
            continue;
        }
        let v = if a.is_array() {
            Value::Array(d.elements(i).iter().map(|(k, v)| json!([k, scripted::value_to_json(v)])).collect())
        } else {
            scripted::value_to_json(d.value(i))
        };
        o.insert(a.name.clone(), v);
    }
    Value::Object(o).to_string()
}

/// The mesh keys a compute's outputs name.
fn mesh_keys(v: &Value) -> Vec<u64> {
    match v {
        Value::Object(o) => o.iter().flat_map(|(k, x)| if k == "$mesh" { x.as_u64().into_iter().collect() } else { mesh_keys(x) }).collect(),
        Value::Array(a) => a.iter().flat_map(mesh_keys).collect(),
        _ => Vec::new(),
    }
}

fn write_outputs(d: &mut NodeData, out: &str) {
    let Ok(Value::Object(o)) = serde_json::from_str::<Value>(out) else { return };
    let nums = |v: Option<&Value>| v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<f64>>());
    scripted::apply_special_outputs(d, nums(o.get("$matrix")).as_deref(), nums(o.get("$bbox")).as_deref());
    // A plug-in data value given back by key.
    let value_of = |ty: PlugType, v: &Value| -> Option<aztek_scene::plug::PlugValue> {
        if let Some(k) = v.get("$data").and_then(Value::as_u64) {
            return Some(aztek_scene::plug::PlugValue::Data(take_handoff(k)));
        }
        if let Some(k) = v.get("$mesh").and_then(Value::as_u64) {
            return Some(aztek_scene::plug::PlugValue::Mesh(mesh_handed(k)));
        }
        scripted::value_from_json(ty, v)
    };
    for (k, v) in o {
        if k.starts_with('$') {
            continue;
        }
        let Some(i) = d.ty().attr_index(&k) else { continue };
        let a = &d.ty().attrs[i];
        let ty = a.ty;
        if a.is_array() {
            let els = v.as_array().map(|l| l.iter().filter_map(|e| Some((e.get(0)?.as_i64()? as i32, value_of(ty, e.get(1)?)?))).collect()).unwrap_or_default();
            d.set_output_elements(i, els);
        } else if let Some(x) = value_of(ty, &v) {
            d.set_output(i, x);
        }
    }
}

fn compute(type_name: &str, id: NodeId, name: &str, time: f64, d: &mut NodeData) {
    // Computes run on several threads at once, so only this compute's own keys are handed back.
    let mut keys = Vec::new();
    let mut input = node_values(d, &mut keys);
    // A deformer: its mesh's points go across, with the painted weights and the mesh's world matrix; with no mesh
    // in, nothing comes out.
    let deformer = scripted::is_plugin_deformer(type_name);
    let (inp, outp) = (d.ty().attr_index("inputPolymesh"), d.ty().attr_index("output"));
    let mesh_in = if deformer { inp.and_then(|i| d.mesh(i).cloned()) } else { None };
    if deformer {
        let Some(m) = &mesh_in else {
            if let Some(o) = outp {
                d.set_output(o, aztek_scene::plug::PlugValue::Mesh(None));
            }
            return;
        };
        let mut v: Value = serde_json::from_str(&input).unwrap_or_else(|_| json!({}));
        let pts: Vec<f64> = (0..m.point_capacity()).flat_map(|p| {
            let q = if m.point_valid(p) { m.position(p) } else { glam::DVec3::ZERO };
            [q.x, q.y, q.z]
        }).collect();
        v["$points"] = json!(pts);
        if let Some(w) = scripted::deformer_weights(d) {
            v["$weights"] = json!(w);
        }
        if let Some(gm) = d.ty().attr_index("geomMatrix") {
            v["$geomMatrix"] = json!(d.matrix(gm).to_cols_array());
        }
        input = v.to_string();
    }
    let was = IN_COMPUTE.with(|c| c.replace(true));
    let out = Python::attach(|py| hook_call(py, "compute", (type_name, id, name, time, input.as_str())));
    IN_COMPUTE.with(|c| c.set(was));
    // What was handed over for this compute goes back.
    DATA_HANDOFF.lock().unwrap().retain(|(k, _)| !keys.contains(k));
    match out {
        Ok(s) => {
            // Meshes made or handed in during the compute go once its outputs hold what they need.
            let made: Vec<u64> = serde_json::from_str::<Value>(&s).ok().map(|v| mesh_keys(&v)).unwrap_or_default();
            write_outputs(d, &s);
            MESH_HANDOFF.lock().unwrap().retain(|(k, _)| !keys.contains(k) && !made.contains(k));
            if let (true, Some(m), Some(o)) = (deformer, &mesh_in, outp) {
                // The points the deform gave back, on the mesh that came in; none back: the mesh as it came.
                let got = serde_json::from_str::<Value>(&s).ok().and_then(|v| v.get("$points").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect::<Vec<f64>>()));
                let out_mesh = match got {
                    Some(p) if p.len() == m.point_capacity() as usize * 3 => Arc::new(m.with_positions(p.chunks(3).map(|c| glam::DVec3::new(c[0], c[1], c[2])).collect())),
                    _ => m.clone(),
                };
                d.set_output(o, aztek_scene::plug::PlugValue::Mesh(Some(out_mesh)));
            }
        }
        Err(e) => DEFERRED.lock().unwrap().push((format!("# Error: {e} #"), LineKind::Error)),
    }
}

pub(crate) fn plugin_command(s: &mut Session, method: &'static str, a: &Args) -> CmdResult {
    ensure().map_err(CommandError::new)?;
    let input = json!({"args": a.positional, "flags": a.flags}).to_string();
    answer(nested(s, || {
        Python::attach(|py| {
            py.import("aztek.api.OpenAztek").map_err(|e| e.to_string())?;
            hook_call(py, method, (input.as_str(),))
        })
    }))
}

/// loadPlugin, unloadPlugin, pluginInfo.
pub fn register_commands(s: &mut Session) {
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    s.register(
        CommandDef::<Session>::new("aztekPython", "Run Python from inside a command, as schema translators do.", |s, a| {
            ensure().map_err(CommandError::new)?;
            let code = CString::new(a.pos_str(0).unwrap_or("")).map_err(|e| CommandError::new(e.to_string()))?;
            let r: Result<(), String> = nested(s, || Python::attach(|py| py.run(&code, None, None).map_err(|e| e.to_string())));
            r.map_err(CommandError::new)?;
            Ok(Value::Null)
        })
        .positional("code"),
    );
    s.register(
        CommandDef::<Session>::new(
            "loadPlugin",
            "Load a Python plug-in: a .py file by path, or by name from the folders in AZTEK_PLUG_IN_PATH and MAYA_PLUG_IN_PATH. Returns its name in a list.",
            |s, a| plugin_command(s, "load_plugin", a),
        )
        .positional("plug-in file or name")
        .flag(b("quiet", "qt", "Don't complain when it can't be found."))
        .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("unloadPlugin", "Unload a plug-in: its uninitializePlugin runs and its commands and node types go.", |s, a| plugin_command(s, "unload_plugin", a))
            .positional("plug-in name")
            .flag(b("force", "f", "Accepted for scripts."))
            .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("pluginInfo", "Ask about plug-ins: pluginInfo -q -loaded name, -listPlugins, -version, -vendor, -path, -command, -dependNode.", |s, a| plugin_command(s, "plugin_info", a))
            .positional("plug-in name")
            .flag(b("query", "q", "Query."))
            .flag(b("edit", "e", "Accepted for scripts."))
            .flag(FlagDef::new("queried", "", ArgType::List, Value::Null, "What is asked."))
            .flag(b("loaded", "l", "Whether it is loaded."))
            .flag(b("registered", "r", "Whether it is known."))
            .flag(b("listPlugins", "ls", "The loaded plug-ins."))
            .flag(b("version", "v", "Its version."))
            .flag(b("vendor", "vd", "Its vendor."))
            .flag(b("path", "p", "Its file."))
            .flag(b("name", "n", "Its name."))
            .flag(b("command", "c", "The commands it registered."))
            .flag(b("dependNode", "dn", "The node types it registered."))
            .flag(b("apiVersion", "av", "The API version it asked for."))
            .undoable(false),
    );
}

// ---------------------------------------------------------------------------------------------------------------------
// _aztek

#[pyfunction]
fn call(py: Python<'_>, name: &str, args: &str, kwargs: &str) -> PyResult<String> {
    let args: Vec<Value> = serde_json::from_str(args).unwrap_or_default();
    let kwargs: Map<String, Value> = serde_json::from_str(kwargs).unwrap_or_default();
    let stored = STORED_UNITS.with(Cell::get);
    host(py, |c| {
        let run = |c: &mut Core| c.py_call(name, args, kwargs);
        let r = if stored { aztek_ops::units::stored(|| run(c)) } else { aztek_ops::units::as_script(|| run(c)) };
        match r {
            Ok(v) => v.to_string(),
            Err(e) => json!({ "__error__": e }).to_string(),
        }
    })
}

thread_local! {
    /// Set while the Python API calls a command: its values are stored ones.
    static STORED_UNITS: Cell<bool> = const { Cell::new(false) };
}

/// The API says it's calling, and gets back what it said before.
#[pyfunction]
fn stored_units(on: bool) -> bool {
    STORED_UNITS.with(|s| s.replace(on))
}

#[pyfunction]
fn mel(py: Python<'_>, src: &str) -> PyResult<String> {
    host(py, |c| match c.mel_checked(src) {
        Ok(v) => v.to_string(),
        Err(e) => json!({ "__error__": e }).to_string(),
    })
}

#[pyfunction]
fn write(py: Python<'_>, text: &str, kind: i32) -> PyResult<()> {
    let k = match kind {
        1 => LineKind::Result,
        2 => LineKind::Error,
        4 => LineKind::Warning,
        _ => LineKind::Info,
    };
    if !usable() {
        DEFERRED.lock().unwrap().push((text.to_string(), k));
        return Ok(());
    }
    host(py, |c| c.push(text, k))
}

#[pyfunction]
fn exists(py: Python<'_>, name: &str) -> PyResult<bool> {
    host(py, |c| c.s.commands.find(name).is_some() || crate::mel::alias(name).is_some())
}

#[pyfunction]
fn doc(py: Python<'_>, name: &str) -> PyResult<String> {
    host(py, |c| c.s.commands.find(name).map(|d| d.doc.clone()).unwrap_or_default())
}

#[pyfunction]
fn names(py: Python<'_>) -> PyResult<String> {
    host(py, |c| {
        let mut v: Vec<String> = c.s.commands.all().map(|d| d.name.clone()).collect();
        v.extend(crate::mel::ALIASES.iter().map(|a| a.the_reference.to_string()));
        json!(v).to_string()
    })
}

/// A query of python_api: op and a JSON object in, JSON out.
#[pyfunction]
fn api(py: Python<'_>, op: &str, args: &str) -> PyResult<String> {
    let args: Value = serde_json::from_str(args).unwrap_or(Value::Null);
    host(py, |c| crate::python_api::query(c, op, &args))?.map(|v| v.to_string()).map_err(PyRuntimeError::new_err)
}

/// Moves on whenever anything in the scene changes.
#[pyfunction]
fn scene_version(py: Python<'_>) -> PyResult<u64> {
    host(py, |c| c.s.graph.version())
}

#[pyfunction]
fn mesh_uv_set(py: Python<'_>, id: NodeId, set: &str) -> PyResult<(Vec<i64>, Vec<f64>)> {
    let set = set.to_string();
    host(py, move |c| crate::python_api::uv_set_arrays(c, id, &set))?.map_err(PyRuntimeError::new_err)
}

#[pyfunction]
fn mesh_arrays<'py>(py: Python<'py>, id: NodeId) -> PyResult<Bound<'py, PyDict>> {
    let m = host(py, |c| crate::python_api::mesh_arrays(c, id))?.map_err(PyRuntimeError::new_err)?;
    arrays_dict(py, &m)
}

/// A mesh's arrays as Python takes them. The numbers go across as raw bytes, which Python takes into array.array in
/// one copy: a list of Python floats for every number of a big mesh takes seconds to build.
fn arrays_dict<'py>(py: Python<'py>, m: &crate::python_api::MeshArrays) -> PyResult<Bound<'py, PyDict>> {
    let d = PyDict::new(py);
    let f = |v: &Vec<f64>| PyBytes::new(py, bytemuck::cast_slice(v));
    let i = |v: &Vec<i64>| PyBytes::new(py, bytemuck::cast_slice(v));
    d.set_item("shape", m.shape)?;
    d.set_item("points", f(&m.points))?;
    d.set_item("counts", i(&m.counts))?;
    d.set_item("connects", i(&m.connects))?;
    d.set_item("corner_edges", i(&m.corner_edges))?;
    d.set_item("edges", i(&m.edges))?;
    d.set_item("corner_normals", f(&m.corner_normals))?;
    d.set_item("point_normals", f(&m.point_normals))?;
    d.set_item("has_uvs", m.has_uvs)?;
    d.set_item("corner_maps", i(&m.corner_maps))?;
    d.set_item("map_uvs", f(&m.map_uvs))?;
    d.set_item("point_ids", m.point_ids.clone())?;
    d.set_item("edge_ids", m.edge_ids.clone())?;
    d.set_item("face_ids", m.face_ids.clone())?;
    Ok(d)
}

// Meshes handed across a compute (mesh attributes in and out), by key, as plug-in data values are.
static MESH_HANDOFF: Mutex<Vec<(u64, Arc<aztek_geo::Mesh>)>> = Mutex::new(Vec::new());

fn mesh_hand_off(m: Arc<aztek_geo::Mesh>) -> u64 {
    let k = NEXT_HANDOFF.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    MESH_HANDOFF.lock().unwrap().push((k, m));
    k
}

fn mesh_handed(k: u64) -> Option<Arc<aztek_geo::Mesh>> {
    MESH_HANDOFF.lock().unwrap().iter().find(|(x, _)| *x == k).map(|(_, m)| m.clone())
}

/// A mesh a compute was handed, or made: its arrays.
#[pyfunction]
fn mesh_handoff_arrays<'py>(py: Python<'py>, key: u64) -> PyResult<Bound<'py, PyDict>> {
    let m = mesh_handed(key).ok_or_else(|| PyRuntimeError::new_err("(kFailure): The mesh data is gone"))?;
    arrays_dict(py, &crate::python_api::arrays_of(&m))
}

/// A mesh made from arrays in Python (MFnMesh.create on MFnMeshData's object): its key, and its arrays. uv_ids, when
/// given, picks each corner's UV from us and vs.
#[pyfunction]
#[pyo3(signature = (points, counts, connects, us=None, vs=None, uv_ids=None))]
fn mesh_from_arrays<'py>(py: Python<'py>, points: Vec<f64>, counts: Vec<i64>, connects: Vec<i64>, us: Option<Vec<f64>>, vs: Option<Vec<f64>>, uv_ids: Option<Vec<i64>>) -> PyResult<(u64, Bound<'py, PyDict>)> {
    let pts: Vec<glam::DVec3> = points.chunks(3).filter(|c| c.len() == 3).map(|c| glam::DVec3::new(c[0], c[1], c[2])).collect();
    let sizes: Vec<aztek_geo::Index> = counts.iter().map(|&c| c as aztek_geo::Index).collect();
    let idx: Vec<aztek_geo::Index> = connects.iter().map(|&c| c as aztek_geo::Index).collect();
    if sizes.iter().map(|&c| c as usize).sum::<usize>() != idx.len() || idx.iter().any(|&i| i as usize >= pts.len()) {
        return Err(PyRuntimeError::new_err("(kInvalidParameter): polygonCounts and polygonConnects don't agree with the points"));
    }
    let mut m = aztek_geo::Mesh::from_polygons(&pts, &sizes, &idx);
    if let (Some(us), Some(vs)) = (us, vs) {
        // Corners in face order, as from_polygons made them.
        let corners: Vec<aztek_geo::Index> = (0..m.face_capacity()).filter(|&f| m.face_valid(f)).flat_map(|f| m.face_corners(f)).collect();
        let layer = m.corner_attrs_mut().get_or_add::<[f32; 2]>(aztek_scene::uv_ops::UV);
        for (k, &c) in corners.iter().enumerate() {
            let at = uv_ids.as_ref().map_or(k, |ids| ids.get(k).copied().unwrap_or(0) as usize);
            if let (Some(&u), Some(&v)) = (us.get(at), vs.get(at)) {
                layer[c as usize] = [u as f32, v as f32];
            }
        }
    }
    let m = Arc::new(m);
    let d = arrays_dict(py, &crate::python_api::arrays_of(&m))?;
    Ok((mesh_hand_off(m), d))
}

/// The source of an embedded Python file; the parts of a package are run in one module, in order.
#[pyfunction]
fn source(name: &str) -> Option<&'static str> {
    Some(match name {
        "om_values" => include_str!("../python/om_values.py"),
        "om_core" => include_str!("../python/om_core.py"),
        "om_fn" => include_str!("../python/om_fn.py"),
        "om_iter" => include_str!("../python/om_iter.py"),
        "om_plugin" => include_str!("../python/om_plugin.py"),
        "om_msg" => include_str!("../python/om_msg.py"),
        "om_surface" => include_str!("../python/om_surface.py"),
        "om_thin" => include_str!("../python/om_thin.py"),
        "om2_ui" => include_str!("../python/om2_ui.py"),
        "om2_render" => include_str!("../python/om2_render.py"),
        "om_anim" => include_str!("../python/om_anim.py"),
        "om1" => include_str!("../python/om1.py"),
        "om1_anim" => include_str!("../python/om1_anim.py"),
        "om1_mpx" => include_str!("../python/om1_mpx.py"),
        "pxr_shim" => include_str!("../python/pxr_shim.py"),
        "omui" => include_str!("../python/omui.py"),
        "maya_mixin" => include_str!("../python/maya_mixin.py"),
        "maya_misc" => include_str!("../python/maya_misc.py"),
        _ => return None,
    })
}

/// The address of the application's main window (a QMainWindow), or 0 when there is none, as in batch.
#[pyfunction]
fn main_window() -> usize {
    MAIN_WINDOW.load(std::sync::atomic::Ordering::Relaxed)
}

/// Set by the shell once its main window exists.
pub static MAIN_WINDOW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

#[pymodule]
fn _aztek(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(main_window, m)?)?;
    m.add_function(wrap_pyfunction!(call, m)?)?;
    m.add_function(wrap_pyfunction!(stored_units, m)?)?;
    m.add_function(wrap_pyfunction!(mel, m)?)?;
    m.add_function(wrap_pyfunction!(write, m)?)?;
    m.add_function(wrap_pyfunction!(exists, m)?)?;
    m.add_function(wrap_pyfunction!(doc, m)?)?;
    m.add_function(wrap_pyfunction!(names, m)?)?;
    m.add_function(wrap_pyfunction!(api, m)?)?;
    m.add_function(wrap_pyfunction!(scene_version, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_arrays, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_handoff_arrays, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_from_arrays, m)?)?;
    m.add_function(wrap_pyfunction!(mesh_uv_set, m)?)?;
    m.add_function(wrap_pyfunction!(source, m)?)?;
    m.add_function(wrap_pyfunction!(set_usd_resolve_hook, m)?)?;
    m.add_function(wrap_pyfunction!(set_hooks, m)?)?;
    m.add_function(wrap_pyfunction!(register_command, m)?)?;
    m.add_function(wrap_pyfunction!(deregister_command, m)?)?;
    m.add_function(wrap_pyfunction!(register_node, m)?)?;
    m.add_function(wrap_pyfunction!(register_translator, m)?)?;
    m.add_function(wrap_pyfunction!(api_error, m)?)?;
    m.add_function(wrap_pyfunction!(api_errlog, m)?)?;
    m.add_function(wrap_pyfunction!(deregister_translator, m)?)?;
    m.add_function(wrap_pyfunction!(register_data, m)?)?;
    m.add_function(wrap_pyfunction!(deregister_data, m)?)?;
    m.add_function(wrap_pyfunction!(data_get, m)?)?;
    m.add_function(wrap_pyfunction!(data_put, m)?)?;
    m.add_function(wrap_pyfunction!(deregister_node, m)?)?;
    m.add_function(wrap_pyfunction!(standalone_initialize, m)?)?;
    Ok(())
}

/// maya.standalone.initialize(): the user folder's paths, modules, plug-ins to load and userSetup scripts, as a
/// window's start-up has them. Once a process; in a window they are there already.
#[pyfunction]
fn standalone_initialize(py: Python<'_>) {
    static DONE: std::sync::Once = std::sync::Once::new();
    if !aztek_core::batch_mode() {
        return;
    }
    py.detach(|| {
        DONE.call_once(|| {
            crate::user_folder::apply(&crate::user_folder::load_choice());
            crate::aztek_startup_scripts();
        })
    });
}

/// The interpreter starts once per process; its outcome stays for later callers.
static STARTED: OnceLock<Result<(), String>> = OnceLock::new();

fn ensure() -> Result<(), String> {
    ENTERED.with(|e| e.set(true));
    STARTED
        .get_or_init(|| {
            let _ = MAIN.set(std::thread::current().id());
            pyo3::append_to_inittab!(_aztek);
            let home = bundled_home();
            if let Some(h) = &home {
                // Set before Python starts, on the thread that starts it; taken out again just after.
                std::env::set_var("PYTHONHOME", h);
            }
            if home.is_some() || cfg!(target_os = "windows") {
                // The interpreter's own packages only: a user site one would shadow the bundled PySide6 and the rest.
                std::env::set_var("PYTHONNOUSERSITE", "1");
            }
            Python::initialize();
            if home.is_some() {
                // Read once, at start-up: kept, it would point the system's own Python, run from a script, at this one.
                std::env::remove_var("PYTHONHOME");
                let _ = Python::attach(|py| py.run(c"import os; os.environ.pop('PYTHONHOME', None)", None, None));
            }
            let r = Python::attach(|py| -> PyResult<()> {
                let code = std::ffi::CString::new(include_str!("../python/bootstrap.py")).unwrap();
                py.run(&code, None, None)?;
                // Aztek's own packages, where aztekpy's pip puts them: on the path of every session, ahead of the
                // system's, so what was installed for Aztek is what Aztek imports.
                let dir = packages_folder();
                py.import("__main__")?.getattr("_aztek_add_packages")?.call1((dir.to_string_lossy().to_string(),))?;
                Ok(())
            })
            .map_err(|e| format!("Python start-up: {e}"));
            scripted::set_compute_hook(Arc::new(compute));
            r
        })
        .clone()
}

/// The home of a bundled Python (tools/python/fetch-python.sh): the folder above the libpython this process loaded on
/// Linux, or Contents/Resources/python in the app on macOS, when its standard library is there. Such a Python was built
/// for another folder and doesn't find its own; a system's Python does, and isn't given one. None as well when
/// PYTHONHOME is already set.
fn bundled_home() -> Option<std::path::PathBuf> {
    if std::env::var_os("PYTHONHOME").is_some() {
        return None;
    }
    #[cfg(target_os = "linux")]
    {
        let maps = std::fs::read_to_string("/proc/self/maps").ok()?;
        let lib = maps.lines().filter_map(|l| l.find('/').map(|i| &l[i..])).find(|p| p.contains("/libpython3.13.so"))?;
        let dir = std::path::Path::new(lib).parent()?;
        let home = dir.parent()?;
        let own = dir.file_name()? == "lib" && !["/usr", "/usr/local"].iter().any(|s| home == std::path::Path::new(s));
        (own && home.join("lib/python3.13/os.py").is_file()).then(|| home.to_path_buf())
    }
    #[cfg(target_os = "macos")]
    {
        // Aztek.app/Contents/MacOS/aztek, and the standard library build-macos.sh puts in Contents/Resources/python.
        let exe = std::env::current_exe().ok()?;
        let home = exe.parent()?.parent()?.join("Resources/python");
        (home.join("lib/python3.13/os.py").is_file()).then(|| home)
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        None
    }
}

/// Aztek's own folder of Python packages, for this Python's version: what `aztekpy -m pip install` puts there is on
/// every Aztek session's path and no other Python's. `AZTEK_PYTHON_PACKAGES` names another.
pub fn packages_folder() -> std::path::PathBuf {
    if let Some(d) = std::env::var_os("AZTEK_PYTHON_PACKAGES").filter(|d| !d.is_empty()) {
        return d.into();
    }
    let v = Python::attach(|py| py.version_info().major.to_string() + "." + &py.version_info().minor.to_string());
    crate::user_folder::own_folder().join("python").join(v).join("site-packages")
}

/// `aztekpy`, the reference's mayapy: Python's own command line (a script and its arguments, -c, -m, -V, or the
/// interactive prompt with none), run by Aztek's Python with Aztek's modules ready, no window. `args` are what follow
/// the program's name. The exit status is the script's.
pub fn python_main(exe: &str, args: &[String]) -> i32 {
    if let Err(e) = ensure() {
        eprintln!("aztekpy: {e}");
        return 1;
    }
    let dir = packages_folder();
    Python::attach(|py| -> PyResult<i32> {
        let main = py.import("__main__")?;
        main.getattr("_aztek_pymain")?.call1((args.to_vec(), exe, dir.to_string_lossy().to_string()))?.extract()
    })
    .unwrap_or_else(|e| {
        eprintln!("aztekpy: {e}");
        1
    })
}

/// Whether the interpreter has been started: no Python plug-in can be loaded before that.
pub(crate) fn started() -> bool {
    STARTED.get().is_some()
}

/// MEL's python("..."): a lone expression's value comes back as JSON, statements give null. An exception is the error.
pub(crate) fn eval_from_mel(core: &mut Core, src: &str) -> Result<Value, String> {
    let saved = NESTED.with(|c| c.replace(core as *mut Core));
    let r = ensure().and_then(|()| {
        Python::attach(|py| -> PyResult<String> {
            let main = py.import("__main__")?;
            main.getattr("_aztek_eval")?.call1((src,))?.extract()
        })
        .map_err(|e| e.to_string())
    });
    NESTED.with(|c| c.set(saved));
    serde_json::from_str(&r?).map_err(|e| e.to_string())
}

/// Calls a function of __main__ with `core` as the commands' core, the way MEL's python() does.
fn call_main(core: &mut Core, func: &str, args: impl for<'py> pyo3::call::PyCallArgs<'py>) -> Result<(), String> {
    let saved = NESTED.with(|c| c.replace(core as *mut Core));
    let r = ensure().and_then(|()| {
        Python::attach(|py| -> PyResult<()> {
            py.import("__main__")?.getattr(func)?.call1(args)?;
            Ok(())
        })
        .map_err(|e| e.to_string())
    });
    NESTED.with(|c| c.set(saved));
    r
}

/// Runs a Python callable kept for a control or job, with what the reference would pass it.
pub(crate) fn fire_callback(core: &mut Core, id: i64, args: &[Value]) -> Result<(), String> {
    call_main(core, "_aztek_fire", (id, Value::Array(args.to_vec()).to_string()))
}

/// As `fire_callback`, with what the callable returns.
pub(crate) fn fire_value(core: &mut Core, id: i64, args: &[Value]) -> Result<Value, String> {
    let saved = NESTED.with(|c| c.replace(core as *mut Core));
    let text = Value::Array(args.to_vec()).to_string();
    let r = ensure().and_then(|()| {
        Python::attach(|py| -> PyResult<String> { py.import("__main__")?.getattr("_aztek_fire_value")?.call1((id, text))?.extract() }).map_err(|e| e.to_string())
    });
    NESTED.with(|c| c.set(saved));
    serde_json::from_str(&r?).map_err(|e| e.to_string())
}

/// A kept callable given keyword arguments (an object of them), with what it returns.
pub(crate) fn fire_value_kw(core: &mut Core, id: i64, kwargs: &Value) -> Result<Value, String> {
    let saved = NESTED.with(|c| c.replace(core as *mut Core));
    let text = kwargs.to_string();
    let r = ensure().and_then(|()| {
        Python::attach(|py| -> PyResult<String> { py.import("__main__")?.getattr("_aztek_fire_value_kw")?.call1((id, text))?.extract() }).map_err(|e| e.to_string())
    });
    NESTED.with(|c| c.set(saved));
    serde_json::from_str(&r?).map_err(|e| e.to_string())
}

/// Runs Python source given as a callback. Exceptions are reported in the history, as for a callable.
pub(crate) fn fire_source(core: &mut Core, src: &str) -> Result<(), String> {
    call_main(core, "_aztek_fire_source", (src,))
}

/// While a dialog's event loop runs on top of a suspended command, a nested core may hold the session. Lends it to `h`
/// for the call. The nested core's frame is suspended under this one and does nothing until the hook returns.
pub(crate) fn with_lent_session<R>(h: &mut Core, f: impl FnOnce(&mut Core) -> R) -> R {
    let p = NESTED.with(Cell::get);
    if p.is_null() || std::ptr::eq(p, h) {
        return f(h);
    }
    std::mem::swap(&mut h.s, unsafe { &mut (*p).s });
    let r = f(h);
    std::mem::swap(&mut h.s, unsafe { &mut (*p).s });
    r
}

/// Runs Python source in __main__. A lone expression's value is printed as the reference does: # Result: ... #
/// Must not be called from inside a host borrow: the commands it runs borrow the host themselves.
/// As `run`, for `aztek --batch`: the script's exit status. An exception nothing caught is 1 and `sys.exit(n)` is n.
pub fn run_status(src: &str) -> i32 {
    if let Err(e) = ensure() {
        let _ = with_core(|core| core.push(format!("# Error: {e} #"), LineKind::Error));
        return 1;
    }
    let r = Python::attach(|py| -> PyResult<i32> {
        let main = py.import("__main__")?;
        match main.getattr("_aztek_run")?.call1((src,)) {
            Ok(_) => Ok(main.getattr("_aztek_run_failed")?.is_truthy()? as i32),
            Err(e) if e.is_instance_of::<pyo3::exceptions::PySystemExit>(py) => {
                let code = e.value(py).getattr("code")?;
                if code.is_none() {
                    return Ok(0);
                }
                match code.extract::<i32>() {
                    Ok(n) => Ok(n),
                    Err(_) => {
                        eprintln!("{code}");
                        Ok(1)
                    }
                }
            }
            Err(e) => Err(e),
        }
    });
    let status = match r {
        Ok(n) => n,
        Err(e) => {
            let _ = with_core(|core| core.push(format!("# Error: {e} #"), LineKind::Error));
            1
        }
    };
    let _ = with_core(|core| core.version += 1);
    status
}

pub fn run(src: &str) {
    if let Err(e) = ensure() {
        let _ = with_core(|core| core.push(format!("# Error: {e} #"), LineKind::Error));
        return;
    }
    let r = Python::attach(|py| -> PyResult<()> {
        let main = py.import("__main__")?;
        main.getattr("_aztek_run")?.call1((src,))?;
        Ok(())
    });
    let _ = with_core(|core| {
        if let Err(e) = r {
            core.push(format!("# Error: {e} #"), LineKind::Error);
        }
        core.version += 1
    });
}
