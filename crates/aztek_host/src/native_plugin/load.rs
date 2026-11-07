//! loadPlugin, unloadPlugin and pluginInfo for compiled plug-ins, handing Python ones to the Python host.

use std::ffi::{c_char, c_void};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use serde_json::Map;

use super::table::TABLE;
use super::*;

#[cfg(windows)]
const EXTENSIONS: &[&str] = &["dll", "mll"];
#[cfg(not(windows))]
const EXTENSIONS: &[&str] = &["so"];

fn search_path() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for var in ["AZTEK_PLUG_IN_PATH", "MAYA_PLUG_IN_PATH"] {
        if let Some(v) = std::env::var_os(var) {
            out.extend(std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()));
        }
    }
    out
}

fn is_native(p: &Path) -> bool {
    p.extension().and_then(|e| e.to_str()).is_some_and(|e| ["so", "dll", "mll"].contains(&e.to_ascii_lowercase().as_str()))
}

/// A compiled plug-in's file: a path, or a name looked for on the plug-in path.
pub(crate) fn find_native(name: &str) -> Option<PathBuf> {
    let p = Path::new(name);
    if p.is_file() && is_native(p) {
        return Some(std::fs::canonicalize(p).unwrap_or(p.to_path_buf()));
    }
    if p.components().count() > 1 {
        return None;
    }
    for dir in search_path() {
        if is_native(p) && dir.join(name).is_file() {
            return Some(dir.join(name));
        }
        for ext in EXTENSIONS {
            let f = dir.join(format!("{name}.{ext}"));
            if f.is_file() {
                return Some(f);
            }
        }
    }
    None
}

/// Whether a Python plug-in by that name is there to load.
fn python_file(name: &str) -> bool {
    let p = Path::new(name);
    if p.extension().is_some_and(|e| e == "py") && p.is_file() {
        return true;
    }
    let file = if name.ends_with(".py") { name.to_string() } else { format!("{name}.py") };
    search_path().iter().any(|d| d.join(&file).is_file())
}

pub(crate) fn key_of(name: &str) -> String {
    let p = Path::new(name);
    let base = p.file_name().and_then(|b| b.to_str()).unwrap_or(name);
    for ext in ["so", "dll", "mll", "py"] {
        if let Some(stem) = base.strip_suffix(&format!(".{ext}")) {
            return stem.to_string();
        }
    }
    base.to_string()
}

fn open(path: &Path) -> Result<libloading::Library, String> {
    #[cfg(unix)]
    let r = unsafe { libloading::os::unix::Library::open(Some(path), libc::RTLD_NOW | libc::RTLD_LOCAL) }.map(libloading::Library::from);
    #[cfg(windows)]
    let r = unsafe { libloading::os::windows::Library::load_with_flags(path, libloading::os::windows::LOAD_WITH_ALTERED_SEARCH_PATH) }.map(libloading::Library::from);
    r.map_err(|e| {
        let text = e.to_string();
        let foreign = ["libOpenMaya", "OpenMaya.dll", "libFoundation", "Foundation.dll"].iter().any(|n| text.contains(n));
        if foreign {
            format!("{} was built against another application's libraries and can't load here. Rebuild it from source against Aztek's kit (see PLUGINS.md): {text}", path.display())
        } else {
            text
        }
    })
}

fn load_native(s: &mut Session, key: &str, path: &Path) -> Result<(), String> {
    let lib = open(path)?;
    let abi = unsafe { lib.get::<unsafe extern "C" fn() -> i32>(b"aztek_plugin_abi\0") }
        .map_err(|_| format!("{} was not built with Aztek's kit: it has no aztek_plugin_abi. A plug-in built for another application can't load; rebuild it from source with aztek_add_plugin (see PLUGINS.md).", path.display()))?;
    let version = unsafe { abi() };
    if version < 1 || version as u32 > ABI_VERSION {
        return Err(format!("{} was built with a newer kit (ABI {version}) than this Aztek has ({ABI_VERSION}).", path.display()));
    }
    let load = unsafe { lib.get::<unsafe extern "C" fn(*const c_void, *mut c_void, *mut *mut c_char) -> i32>(b"aztek_plugin_load\0") }.map_err(|e| e.to_string())?;
    let load = *load;
    let unload = unsafe { lib.get::<unsafe extern "C" fn(*mut *mut c_char) -> i32>(b"aztek_plugin_unload\0") }.ok().map(|f| *f);
    let lib = Arc::new(Lib(lib));
    let state = Box::new(PluginState {
        name: key.to_string(),
        path: path.to_string_lossy().into_owned(),
        vendor: "Unknown".into(),
        version: "Unknown".into(),
        api: "Any".into(),
        lib,
        commands: Vec::new(),
        nodes: Vec::new(),
        callbacks: Vec::new(),
        unload,
    });
    let ptr = PLUGINS.with(|p| {
        let mut v = p.borrow_mut();
        v.push(state);
        &mut **v.last_mut().unwrap() as *mut PluginState
    });
    let mut err: *mut c_char = std::ptr::null_mut();
    let rc = nested(s, || unsafe { load(&TABLE as *const _ as *const c_void, ptr as *mut c_void, &mut err) });
    let err = take(err);
    if rc != AZ_OK {
        take_back(s, ptr);
        PLUGINS.with(|p| p.borrow_mut().retain(|x| x.name != key));
        return Err(match rc {
            AZ_CRASHED => format!("initializePlugin of {key} crashed ({err})"),
            _ if err.is_empty() => format!("initializePlugin of {key} failed"),
            _ => format!("initializePlugin of {key} failed: {err}"),
        });
    }
    Ok(())
}

/// What a plug-in still has registered goes when it does.
fn take_back(s: &mut Session, p: *mut PluginState) {
    let p = unsafe { &mut *p };
    for name in std::mem::take(&mut p.commands) {
        s.commands.unregister(&name);
    }
    keep_commands(s);
    for name in std::mem::take(&mut p.nodes) {
        aztek_scene::scripted::deregister_type(&name);
    }
    for id in std::mem::take(&mut p.callbacks) {
        CALLBACKS.with(|c| c.borrow_mut().remove(&id));
    }
}

fn unload_native(s: &mut Session, key: &str, force: bool) -> Result<(), String> {
    let p = plugin_named(key).ok_or_else(|| format!("Plug-in \"{key}\" is not loaded."))?;
    let types: Vec<String> = unsafe { (*p).nodes.clone() };
    let in_use = s.graph.nodes().into_iter().filter(|&id| s.graph.node(id).is_some_and(|n| types.contains(&n.ty().name))).count();
    if in_use > 0 && !force {
        return Err(format!("Plug-in \"{key}\" is still in use: the scene has {in_use} of its nodes. Delete them first, or unload with -force."));
    }
    let mut err: *mut c_char = std::ptr::null_mut();
    let rc = match unsafe { (*p).unload } {
        Some(f) => nested(s, || unsafe { f(&mut err) }),
        None => AZ_OK,
    };
    let err = take(err);
    take_back(s, p);
    // Nodes, undo steps and commands still holding the library keep it mapped until they go.
    PLUGINS.with(|v| v.borrow_mut().retain(|x| x.name != key));
    match rc {
        AZ_OK => Ok(()),
        AZ_CRASHED => Err(format!("uninitializePlugin of {key} crashed ({err})")),
        _ => Err(format!("uninitializePlugin of {key} failed: {err}")),
    }
}

#[cfg(feature = "python")]
fn python_call(s: &mut Session, method: &'static str, name: Option<&str>, flags: &Map<String, Value>) -> CmdResult {
    let a = Args { positional: name.map(|n| vec![json!(n)]).unwrap_or_default(), flags: flags.clone() };
    crate::python::plugin_command(s, method, &a)
}

#[cfg(feature = "python")]
fn python_started() -> bool {
    crate::python::started()
}

#[cfg(not(feature = "python"))]
fn python_call(_s: &mut Session, _method: &'static str, name: Option<&str>, _flags: &Map<String, Value>) -> CmdResult {
    Err(CommandError::new(format!("Plug-in \"{}\": this build has no Python.", name.unwrap_or(""))))
}

#[cfg(not(feature = "python"))]
fn python_started() -> bool {
    false
}

fn load_plugin(s: &mut Session, a: &Args) -> CmdResult {
    let quiet = a.bool("quiet");
    let mut out = Vec::new();
    for v in &a.positional {
        let Some(name) = v.as_str() else { continue };
        let key = key_of(name);
        if plugin_named(&key).is_some() {
            out.push(json!(key));
            continue;
        }
        if let Some(path) = find_native(name) {
            load_native(s, &key, &path).map_err(CommandError::new)?;
            out.push(json!(key));
            continue;
        }
        if python_file(name) {
            let mut f = Map::new();
            f.insert("quiet".into(), json!(quiet));
            if let Value::Array(l) = python_call(s, "load_plugin", Some(name), &f)? {
                out.extend(l);
            }
            continue;
        }
        if !quiet {
            return Err(CommandError::new(format!("Plug-in \"{name}\" was not found on the plug-in path.")));
        }
    }
    Ok(Value::Array(out))
}

fn unload_plugin(s: &mut Session, a: &Args) -> CmdResult {
    let force = a.bool("force");
    for v in &a.positional {
        let Some(name) = v.as_str() else { continue };
        let key = key_of(name);
        if plugin_named(&key).is_some() {
            unload_native(s, &key, force).map_err(CommandError::new)?;
        } else if python_started() {
            python_call(s, "unload_plugin", Some(name), &a.flags)?;
        } else {
            return Err(CommandError::new(format!("Plug-in \"{name}\" is not loaded.")));
        }
    }
    Ok(Value::Null)
}

fn plugin_info(s: &mut Session, a: &Args) -> CmdResult {
    let on = |k: &str| a.bool(k);
    if on("listPlugins") {
        let mut names = loaded_names();
        if python_started() {
            if let Value::Array(l) = python_call(s, "plugin_info", None, &a.flags)? {
                names.extend(l.into_iter().filter_map(|x| x.as_str().map(str::to_string)));
            }
        }
        names.sort();
        return Ok(json!(names));
    }
    let Some(name) = a.positional.first().and_then(Value::as_str) else { return Err(CommandError::new("pluginInfo: name a plug-in")) };
    let key = key_of(name);
    let Some(p) = plugin_named(&key) else {
        if python_started() {
            return python_call(s, "plugin_info", Some(name), &a.flags);
        }
        if on("loaded") {
            return Ok(json!(false));
        }
        if on("registered") {
            return Ok(json!(find_native(name).is_some() || python_file(name)));
        }
        return Err(CommandError::new(format!("Plug-in \"{name}\" is not loaded.")));
    };
    let p = unsafe { &*p };
    Ok(if on("loaded") || on("registered") {
        json!(true)
    } else if on("version") {
        json!(p.version)
    } else if on("vendor") {
        json!(p.vendor)
    } else if on("path") {
        json!(p.path)
    } else if on("name") {
        json!(p.name)
    } else if on("apiVersion") {
        json!(p.api)
    } else if on("command") {
        json!(p.commands)
    } else if on("dependNode") {
        json!(p.nodes)
    } else {
        return Err(CommandError::new("pluginInfo: nothing asked"));
    })
}

/// loadPlugin, unloadPlugin and pluginInfo for every kind of plug-in, and the command callbacks arrive by.
pub fn register_commands(s: &mut Session) {
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    s.register(
        CommandDef::<Session>::new(
            "loadPlugin",
            "Load a plug-in: a compiled one (.so, .dll) built with Aztek's kit or a Python one (.py), by path or by name from the folders in AZTEK_PLUG_IN_PATH and MAYA_PLUG_IN_PATH. Returns its name in a list.",
            load_plugin,
        )
        .positional("plug-in file or name")
        .flag(b("quiet", "qt", "Don't complain when it can't be found."))
        .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("unloadPlugin", "Unload a plug-in: its uninitializePlugin runs and its commands and node types go. A compiled one in use by the scene needs -force.", unload_plugin)
            .positional("plug-in name")
            .flag(b("force", "f", "Unload even with its nodes in the scene."))
            .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("pluginInfo", "Ask about plug-ins: pluginInfo -q -loaded name, -listPlugins, -version, -vendor, -path, -command, -dependNode.", plugin_info)
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
    s.register(
        CommandDef::<Session>::new("aztekPluginCallback", "Runs a compiled plug-in's message callback by its id; scriptJobs the kit makes call it.", |s, a| {
            let id = a.positional.first().and_then(Value::as_i64).unwrap_or(0);
            let payload = a.positional.get(1).map(|v| v.as_str().map(str::to_string).unwrap_or(v.to_string())).unwrap_or_default();
            super::table::fire_callback(s, id, &payload).map_err(CommandError::new)?;
            Ok(Value::Null)
        })
        .positional("callback id, then what it is told")
        .undoable(false),
    );
}
