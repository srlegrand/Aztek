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
/// The Python object holding the plugin entry points; set when aztek.api.OpenMaya is imported.
static HOOKS: OnceLock<Py<PyAny>> = OnceLock::new();
/// Lines written where the history can't be reached: inside computes and nested calls.
static DEFERRED: Mutex<Vec<(String, LineKind)>> = Mutex::new(Vec::new());

const NO_HOST: &str = "(kFailure): Commands cannot run inside compute";

fn usable() -> bool {
    !IN_COMPUTE.with(Cell::get) && (ENTERED.with(Cell::get) || MAIN.get().is_some_and(|t| *t == std::thread::current().id()))
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

#[pyfunction]
fn register_node(py: Python<'_>, name: &str, owner: &str, attrs: &str) -> PyResult<()> {
    let attrs: Vec<Value> = serde_json::from_str(attrs).map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
    py.detach(|| scripted::register_type(name, owner, &attrs)).map_err(|e| PyRuntimeError::new_err(e.to_string()))
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
fn node_values(d: &NodeData) -> String {
    let mut o = Map::new();
    for (i, a) in d.ty().attrs.iter().enumerate() {
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

fn write_outputs(d: &mut NodeData, out: &str) {
    let Ok(Value::Object(o)) = serde_json::from_str::<Value>(out) else { return };
    for (k, v) in o {
        let Some(i) = d.ty().attr_index(&k) else { continue };
        let a = &d.ty().attrs[i];
        let ty = a.ty;
        if a.is_array() {
            let els = v.as_array().map(|l| l.iter().filter_map(|e| Some((e.get(0)?.as_i64()? as i32, scripted::value_from_json(ty, e.get(1)?)?))).collect()).unwrap_or_default();
            d.set_output_elements(i, els);
        } else if let Some(x) = scripted::value_from_json(ty, &v) {
            d.set_output(i, x);
        }
    }
}

fn compute(type_name: &str, id: NodeId, name: &str, time: f64, d: &mut NodeData) {
    let input = node_values(d);
    let was = IN_COMPUTE.with(|c| c.replace(true));
    let out = Python::attach(|py| hook_call(py, "compute", (type_name, id, name, time, input.as_str())));
    IN_COMPUTE.with(|c| c.set(was));
    match out {
        Ok(s) => write_outputs(d, &s),
        Err(e) => DEFERRED.lock().unwrap().push((format!("# Error: {e} #"), LineKind::Error)),
    }
}

pub(crate) fn plugin_command(s: &mut Session, method: &'static str, a: &Args) -> CmdResult {
    ensure().map_err(CommandError::new)?;
    let input = json!({"args": a.positional, "flags": a.flags}).to_string();
    answer(nested(s, || {
        Python::attach(|py| {
            py.import("aztek.api.OpenMaya").map_err(|e| e.to_string())?;
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
fn mesh_arrays<'py>(py: Python<'py>, id: NodeId) -> PyResult<Bound<'py, PyDict>> {
    let m = host(py, |c| crate::python_api::mesh_arrays(c, id))?.map_err(PyRuntimeError::new_err)?;
    // The numbers go across as raw bytes, which Python takes into array.array in one copy: a list of Python floats
    // for every number of a big mesh takes seconds to build.
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
    d.set_item("point_ids", m.point_ids)?;
    d.set_item("edge_ids", m.edge_ids)?;
    d.set_item("face_ids", m.face_ids)?;
    Ok(d)
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
        "om_anim" => include_str!("../python/om_anim.py"),
        "om1" => include_str!("../python/om1.py"),
        "om1_anim" => include_str!("../python/om1_anim.py"),
        "om1_mpx" => include_str!("../python/om1_mpx.py"),
        "pxr_shim" => include_str!("../python/pxr_shim.py"),
        _ => return None,
    })
}

#[pymodule]
fn _aztek(m: &Bound<'_, PyModule>) -> PyResult<()> {
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
    m.add_function(wrap_pyfunction!(source, m)?)?;
    m.add_function(wrap_pyfunction!(set_usd_resolve_hook, m)?)?;
    m.add_function(wrap_pyfunction!(set_hooks, m)?)?;
    m.add_function(wrap_pyfunction!(register_command, m)?)?;
    m.add_function(wrap_pyfunction!(deregister_command, m)?)?;
    m.add_function(wrap_pyfunction!(register_node, m)?)?;
    m.add_function(wrap_pyfunction!(deregister_node, m)?)?;
    Ok(())
}

/// The interpreter starts once per process; its outcome stays for later callers.
static STARTED: OnceLock<Result<(), String>> = OnceLock::new();

fn ensure() -> Result<(), String> {
    ENTERED.with(|e| e.set(true));
    STARTED
        .get_or_init(|| {
            let _ = MAIN.set(std::thread::current().id());
            pyo3::append_to_inittab!(_aztek);
            Python::initialize();
            let r = Python::attach(|py| -> PyResult<()> {
                let code = std::ffi::CString::new(include_str!("../python/bootstrap.py")).unwrap();
                py.run(&code, None, None)
            })
            .map_err(|e| format!("Python start-up: {e}"));
            scripted::set_compute_hook(Arc::new(compute));
            r
        })
        .clone()
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
        with_core(|core| core.push(format!("# Error: {e} #"), LineKind::Error));
        return;
    }
    let r = Python::attach(|py| -> PyResult<()> {
        let main = py.import("__main__")?;
        main.getattr("_aztek_run")?.call1((src,))?;
        Ok(())
    });
    with_core(|core| {
        if let Err(e) = r {
            core.push(format!("# Error: {e} #"), LineKind::Error);
        }
        core.version += 1
    });
}
