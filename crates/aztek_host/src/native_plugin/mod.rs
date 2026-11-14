//! Compiled plug-ins: shared libraries built against the kit in `devkit/`. The kit's C++ classes run inside the plug-in
//! and reach the host only through the table of C functions here (`devkit/include/aztek/abi.h`, mirrored by hand below).
//! A plug-in links nothing of Aztek's, so the same library loads on any build of the host with the same or a newer ABI.
//!
//! Like Python plug-ins, a plug-in is called back while the host is borrowed: its doIt runs inside `execute`, its undoIt
//! inside the undo stack. The session is moved into a nested Core for the call (`nested`), which the table's functions
//! then use. A node's compute may run on a worker thread: there only the data block and mesh handles are open
//! (`IN_COMPUTE`), and the node type's scheduling decides which lock it holds.

mod load;
pub(crate) mod messages;
mod nodes;
pub(crate) use nodes::run_post_create;
mod table;

use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::{Arc, Mutex};

use aztek_ops::CommandDef;
use aztek_scene::Session;
use serde_json::{json, Value};

use crate::core::{Core, LineKind};

pub use load::{load_autoload, register_commands};

pub const ABI_VERSION: u32 = 2;

pub const AZ_OK: i32 = 0;
pub const AZ_UNKNOWN_PARAMETER: i32 = 1;
pub const AZ_FAILURE: i32 = 2;
pub const AZ_CRASHED: i32 = 3;

thread_local! {
    /// The core a plug-in's calls talk to; null when no plug-in code is running on this thread from the host.
    static NESTED: Cell<*mut Core> = const { Cell::new(std::ptr::null_mut()) };
    static IN_COMPUTE: Cell<bool> = const { Cell::new(false) };
    /// What undoIt and redoIt run their commands with: the stack holds only the graph.
    static COMMANDS: RefCell<Vec<CommandDef<Session>>> = const { RefCell::new(Vec::new()) };
    /// Loaded plug-ins, boxed so the pointer each was given stays put.
    static PLUGINS: RefCell<Vec<Box<PluginState>>> = const { RefCell::new(Vec::new()) };
    static CALLBACKS: RefCell<BTreeMap<i64, Callback>> = const { RefCell::new(BTreeMap::new()) };
    static NEXT_CALLBACK: Cell<i64> = const { Cell::new(1) };
}

/// Lines written where the history can't be reached.
static DEFERRED: Mutex<Vec<(String, LineKind)>> = Mutex::new(Vec::new());

const NO_HOST: &str = "(kFailure): The scene can't be reached from here: inside a compute only the data block can";

/// The library stays mapped while anything of it lives: a node, an undo step, a registered command.
pub(crate) struct Lib(#[allow(dead_code)] libloading::Library);

pub(crate) struct PluginState {
    pub name: String,
    pub path: String,
    pub vendor: String,
    pub version: String,
    pub api: String,
    /// The ABI the plug-in was built with.
    pub abi: u32,
    pub lib: Arc<Lib>,
    pub commands: Vec<String>,
    pub nodes: Vec<String>,
    pub callbacks: Vec<i64>,
    pub unload: Option<unsafe extern "C" fn(*mut *mut c_char) -> i32>,
}

struct Callback {
    func: unsafe extern "C" fn(*mut c_void, *const c_char),
    user: *mut c_void,
    _lib: Arc<Lib>,
}

pub(crate) fn defer(text: impl Into<String>, kind: LineKind) {
    DEFERRED.lock().unwrap_or_else(|e| e.into_inner()).push((text.into(), kind));
}

/// Lines written since the last call, for the history.
pub fn take_deferred() -> Vec<(String, LineKind)> {
    std::mem::take(&mut *DEFERRED.lock().unwrap_or_else(|e| e.into_inner()))
}

fn with_core<R>(f: impl FnOnce(&mut Core) -> R) -> Result<R, String> {
    let p = NESTED.with(Cell::get);
    if p.is_null() || IN_COMPUTE.with(Cell::get) {
        return Err(NO_HOST.to_string());
    }
    // Set by `nested`, whose frame owns it and does nothing else until the plug-in returns.
    Ok(f(unsafe { &mut *p }))
}

/// Runs `f` with the session moved into a nested Core: for calling a plug-in from a command or an undo step.
pub(crate) fn nested<R>(s: &mut Session, f: impl FnOnce() -> R) -> R {
    let mut core = Core::nested(Session::hollow());
    std::mem::swap(s, &mut core.s);
    let prev = NESTED.with(|n| n.replace(&mut core));
    let r = f();
    NESTED.with(|n| n.set(prev));
    std::mem::swap(s, &mut core.s);
    if !core.log.is_empty() {
        let mut d = DEFERRED.lock().unwrap_or_else(|e| e.into_inner());
        d.extend(core.log.drain(..).map(|l| (l.text, l.kind)));
    }
    r
}

fn keep_commands(s: &Session) {
    COMMANDS.with(|c| *c.borrow_mut() = s.commands.all().cloned().collect());
}

/// A string from the plug-in; empty for null.
pub(crate) fn text<'a>(p: *const c_char) -> &'a str {
    if p.is_null() {
        return "";
    }
    unsafe { CStr::from_ptr(p) }.to_str().unwrap_or("")
}

/// A string for the plug-in, which frees it with string_free.
pub(crate) fn give(s: &str) -> *mut c_char {
    CString::new(s.replace('\0', " ")).unwrap().into_raw()
}

/// A string the plug-in made with string_new, taken back.
pub(crate) fn take(p: *mut c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CString::from_raw(p) }.into_string().unwrap_or_default()
}

fn answer(r: Result<Value, String>) -> *mut c_char {
    give(&match r {
        Ok(v) => json!({ "ok": v }).to_string(),
        Err(e) => json!({ "error": e }).to_string(),
    })
}

fn plugin_named(name: &str) -> Option<*mut PluginState> {
    PLUGINS.with(|p| p.borrow_mut().iter_mut().find(|x| x.name == name).map(|b| &mut **b as *mut PluginState))
}

/// The loaded compiled plug-ins' names.
pub(crate) fn loaded_names() -> Vec<String> {
    PLUGINS.with(|p| p.borrow().iter().map(|x| x.name.clone()).collect())
}
