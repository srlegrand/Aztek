//! The host library the Qt shell links: a C interface over the session, script history, scene snapshots and the
//! Vulkan model panels. Strings are UTF-8; strings returned are freed with aztek_free. Main thread only.

// The C interface takes pointers from the Qt shell; its functions stay safe to name so the shell's header matches.
#![allow(clippy::not_unsafe_ptr_arg_deref)]

pub mod dyn_draw;
pub mod autosave;
pub mod archive;
pub mod axis_orient;
mod batch;
pub mod builtin_plugins;
pub mod camera;
mod core;
pub mod ctx_cmds;
pub mod feedback;
mod gpu;
mod harold_view;
pub mod mel;
pub mod node_manip;
pub mod option_boxes;
pub mod panel_cmds;
#[cfg(feature = "python")]
mod python;
mod native_plugin;
mod python_api;
pub mod query;
mod post;
mod render;
pub mod textures;
pub mod lighting;
pub mod script_jobs;
pub mod attr_ui;
pub mod script_objects;
pub mod host_general;
pub mod ee_listen;
pub mod command_port;
pub mod command_line;
pub mod completion;
pub mod scriptui;
pub mod tools;
pub mod tools_quad;
pub mod ui_cmds;
pub mod hud;
pub mod hotbox;
pub mod ramp_controls;
pub mod update;
pub mod mgear;
pub mod environment;
pub mod user_folder;
pub mod sys_host;
pub mod select_cmds;
pub mod sys_more;
pub mod sys_prefs;
pub mod profiler;
pub mod sys_info;
pub mod about;
pub mod channel_box;
pub mod cmd_fields;
pub mod more_controls;
pub mod script_tools;
pub mod attr_menus;
pub mod video;
pub mod text;
pub mod view;
pub mod usd_view;
pub mod viewport;
pub mod realtime;
pub mod view_cube;

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::num::NonZeroU32;
use std::ptr::NonNull;
use std::sync::Arc;

use glam::DVec2;
use raw_window_handle::{RawDisplayHandle, RawWindowHandle};
use serde_json::{json, Value};

pub use crate::core::Core;
pub use crate::query::outliner;

/// The requests waiting for the shell, for tests.
pub fn take_ui_requests_for_tests() -> Value {
    core::take_ui_requests()
}
use crate::gpu::{Gpu, Shared};
use crate::view::View;

pub(crate) struct Host {
    pub(crate) core: Core,
    /// Boxed, so a view made or dropped while another is in use doesn't move it.
    views: HashMap<i32, Box<View>>,
    next_view: i32,
    shared: Option<Arc<Shared>>,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    /// The host, for calls that arrive while a command is suspended in a shell hook.
    static LIVE: std::cell::Cell<*mut Host> = const { std::cell::Cell::new(std::ptr::null_mut()) };
    /// The views a call is working in. A call that reaches the host again from inside one (a dialog's or a Python
    /// callback's event loop, run from a heads-up display button, say) leaves them alone.
    static VIEWS_IN_USE: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
    /// Views the shell closed while a call was working in them: dropped when it returns.
    static VIEWS_CLOSED: RefCell<Vec<i32>> = const { RefCell::new(Vec::new()) };
}

/// Runs `f` on slot `id` of `slots` with `ctx`, unless there is no such slot or a call further out is working in it: a
/// call from a nested event loop must not take a second hold of a view the frames under it hold (#269). A slot closed
/// meanwhile goes to `drop_slot` once `f` returns.
fn use_slot<V, C, R>(slots: &mut HashMap<i32, Box<V>>, ctx: &mut C, id: i32, drop_slot: impl FnOnce(Box<V>), f: impl FnOnce(&mut C, &mut V) -> R) -> Option<R> {
    if VIEWS_IN_USE.with(|u| u.borrow().contains(&id)) {
        return None;
    }
    // The box stays put whatever the nested calls add to or take from the map; one closing this slot only marks it.
    let v: *mut V = &mut **slots.get_mut(&id)?;
    VIEWS_IN_USE.with(|u| u.borrow_mut().push(id));
    let r = f(ctx, unsafe { &mut *v });
    VIEWS_IN_USE.with(|u| u.borrow_mut().retain(|&x| x != id));
    let closed = VIEWS_CLOSED.with(|c| {
        let mut c = c.borrow_mut();
        let was = c.contains(&id);
        c.retain(|&x| x != id);
        was
    });
    if closed {
        if let Some(b) = slots.remove(&id) {
            drop_slot(b);
        }
    }
    Some(r)
}

/// Closes slot `id`, or marks it to close when the call working in it returns.
fn close_slot<V>(slots: &mut HashMap<i32, Box<V>>, id: i32, drop_slot: impl FnOnce(Box<V>)) {
    if VIEWS_IN_USE.with(|u| u.borrow().contains(&id)) {
        VIEWS_CLOSED.with(|c| c.borrow_mut().push(id));
    } else if let Some(b) = slots.remove(&id) {
        drop_slot(b);
    }
}

/// A view and the core, as use_slot gives them.
fn use_view<R>(h: &mut Host, id: i32, f: impl FnOnce(&mut Core, &mut View) -> R) -> Option<R> {
    let Host { core, views, .. } = h;
    use_slot(views, core, id, release_view, f)
}

fn release_view(mut v: Box<View>) {
    v.vp.release(&mut v.gpu);
}

/// The host. A dialog a command opens runs the shell's event loop on top of that command, whose frames hold the host: the
/// shell's calls from that loop take it from `LIVE`. Those frames are suspended in the hook call and touch nothing until
/// it returns. A Python callback can do the same, a tool calling `QApplication.processEvents()` in its window setup; its
/// frames are suspended in the Python call. Any other second borrow is a bug, and panics.
pub(crate) fn with<R>(f: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|h| match h.try_borrow_mut() {
        Ok(mut b) => {
            if b.is_none() {
                crate::core::install_logger();
                crate::sys_host::install_crash_handler();
                *b = Some(Host { core: Core::new(), views: HashMap::new(), next_view: 1, shared: None });
            }
            let host = b.as_mut().unwrap();
            LIVE.with(|l| l.set(host as *mut Host));
            f(host)
        }
        Err(_) => {
            assert!(scriptui::in_hook() || in_host_callback(), "the host was entered twice");
            let host = unsafe { &mut *LIVE.with(std::cell::Cell::get) };
            lend_session(host, f)
        }
    })
}

/// Whether this thread is inside Python the host lent its session to. Qt may run a nested event loop there, and the
/// shell's timers and paint calls then reach the host again.
fn in_host_callback() -> bool {
    #[cfg(feature = "python")]
    {
        python::in_host_callback()
    }
    #[cfg(not(feature = "python"))]
    {
        false
    }
}

#[cfg(feature = "python")]
fn lend_session<R>(host: &mut Host, f: impl FnOnce(&mut Host) -> R) -> R {
    let hp: *mut Host = host;
    // The core is reached again through `hp` inside, so the closure gets the whole host with the session in place.
    python::with_lent_session(&mut host.core, |_| f(unsafe { &mut *hp }))
}

#[cfg(not(feature = "python"))]
fn lend_session<R>(host: &mut Host, f: impl FnOnce(&mut Host) -> R) -> R {
    f(host)
}

fn cstr(p: *const c_char) -> String {
    if p.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
}

fn out(s: String) -> *mut c_char {
    CString::new(s.replace('\0', " ")).unwrap().into_raw()
}

fn out_json(v: Value) -> *mut c_char {
    out(v.to_string())
}

#[no_mangle]
pub extern "C" fn aztek_free(p: *mut c_char) {
    if !p.is_null() {
        unsafe { drop(CString::from_raw(p)) };
    }
}

/// An image file decoded for the interface, whichever format: RGBA, top row first. Null when it can't be read. The
/// caller gives it back to `aztek_image_free` with the same size.
#[no_mangle]
pub extern "C" fn aztek_image_load(path: *const c_char, width: *mut i32, height: *mut i32) -> *mut u8 {
    let Ok(d) = textures::decode(&cstr(path)) else { return std::ptr::null_mut() };
    if width.is_null() || height.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *width = d.width as i32;
        *height = d.height as i32;
    }
    Box::into_raw(d.rgba.into_boxed_slice()) as *mut u8
}

#[no_mangle]
pub extern "C" fn aztek_image_free(data: *mut u8, width: i32, height: i32) {
    if !data.is_null() {
        let len = width as usize * height as usize * 4;
        unsafe { drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(data, len))) };
    }
}

/// The shell's main window (a QMainWindow), for Python tools that parent their own Qt windows to it.
#[no_mangle]
pub extern "C" fn aztek_set_main_window(window: *mut std::ffi::c_void) {
    #[cfg(feature = "python")]
    python::MAIN_WINDOW.store(window as usize, std::sync::atomic::Ordering::Relaxed);
    #[cfg(not(feature = "python"))]
    let _ = window;
}

#[no_mangle]
pub extern "C" fn aztek_init() {
    with(|_| ());
}

/// `aztekpy` and `aztek --python`: Python's command line with Aztek's modules, no window. `argv` is the whole command
/// line; everything after `--python` (or after the program, run as aztekpy) goes to Python.
#[no_mangle]
pub extern "C" fn aztek_python_main(argc: i32, argv: *const *const std::ffi::c_char) -> i32 {
    let args: Vec<String> = (0..argc.max(0) as usize).map(|i| cstr(unsafe { *argv.add(i) })).collect();
    let rest: Vec<String> = match args.iter().position(|a| a == "--python" || a == "-python") {
        Some(i) => args[i + 1..].to_vec(),
        None => args.iter().skip(1).cloned().collect(),
    };
    #[cfg(feature = "python")]
    {
        aztek_core::set_batch_mode(true);
        aztek_init();
        // The launcher beside the program, so a tool's subprocess of sys.executable runs Python, not the window.
        let exe = std::env::current_exe().ok();
        let launcher = exe.as_ref().and_then(|e| e.parent()).map(|d| d.join(if cfg!(windows) { "aztekpy.exe" } else { "aztekpy" })).filter(|p| p.exists());
        let exe = launcher.or(exe).map(|p| p.to_string_lossy().to_string()).unwrap_or_default();
        let rc = python::python_main(&exe, &rest);
        aztek_exiting();
        rc
    }
    #[cfg(not(feature = "python"))]
    {
        let _ = rest;
        eprintln!("aztekpy: this build has no Python");
        1
    }
}

/// The application's start-up: reads the saved preferences (the up axis, the units a new scene starts in, the
/// optionVars) and keeps them from here on. Call first, before anything else.
#[no_mangle]
pub extern "C" fn aztek_use_prefs() {
    with(|h| h.core.use_prefs());
}

#[no_mangle]
pub extern "C" fn aztek_preferences_dir() -> *mut c_char {
    #[cfg(target_os = "linux")]
    aztek_core::paths::linux_config_dir();
    out(aztek_core::paths::to_utf8(&user_folder::prefs_base()))
}

/// Once the main window is up: loads the plug-ins marked to load at start and runs the user folder's userSetup
/// scripts, which may add menus and shelves.
#[no_mangle]
pub extern "C" fn aztek_startup_scripts() {
    let python = with(|h| h.core.startup_scripts());
    #[cfg(feature = "python")]
    for file in python {
        let path = file.to_string_lossy().replace('\\', "/");
        python::run(&format!("import __main__\nexec(compile(open({path:?}, encoding='utf-8').read(), {path:?}, 'exec'), __main__.__dict__)"));
    }
    #[cfg(not(feature = "python"))]
    let _ = python;
    with(|h| crate::native_plugin::messages::scene(&mut h.core.s, crate::native_plugin::messages::MAYA_INITIALIZED, json!({})));
}

/// The application is about to quit: compiled plug-ins hear it.
#[no_mangle]
pub extern "C" fn aztek_exiting() {
    with(|h| crate::native_plugin::messages::scene(&mut h.core.s, crate::native_plugin::messages::MAYA_EXITING, json!({})));
}

pub use batch::{aztek_batch, run as batch_run};

/// Runs MEL-style source; returns the last result as JSON.
#[no_mangle]
pub extern "C" fn aztek_mel(src: *const c_char) -> *mut c_char {
    let src = cstr(src);
    with(|h| out_json(h.core.mel(&src)))
}

/// Runs a command with a JSON call, echoed to the history. Returns {"ok": result} or {"error": message}.
#[no_mangle]
pub extern "C" fn aztek_exec(name: *const c_char, call: *const c_char) -> *mut c_char {
    let name = cstr(name);
    let call: Value = serde_json::from_str(&cstr(call)).unwrap_or(json!({}));
    with(|h| {
        out_json(match h.core.exec(&name, call) {
            Ok(v) => {
                node_manip::after_action(&mut h.core, &v);
                json!({ "ok": v })
            }
            Err(e) => json!({ "error": e }),
        })
    })
}

/// The feedback report's link and text from its JSON. See feedback.rs.
#[no_mangle]
pub extern "C" fn aztek_feedback_issue(call: *const c_char) -> *mut c_char {
    let call: Value = serde_json::from_str(&cstr(call)).unwrap_or(json!({}));
    with(|h| {
        let gpu = h.shared.as_ref().map(|s| s.device_name.clone());
        out_json(feedback::issue_from_json(&call, gpu))
    })
}

/// The latest release on GitHub, as update::latest. Blocks: call it off the interface thread.
#[no_mangle]
pub extern "C" fn aztek_update_latest() -> *mut c_char {
    out_json(update::latest())
}

/// Downloads url to path, checking the digest when there is one. Null when it worked, else the reason. Blocks.
#[no_mangle]
pub extern "C" fn aztek_update_download(url: *const c_char, path: *const c_char, digest: *const c_char) -> *mut c_char {
    match update::download(&cstr(url), std::path::Path::new(&cstr(path)), &cstr(digest)) {
        None => std::ptr::null_mut(),
        Some(e) => out(e),
    }
}

/// Bytes downloaded so far and the total (0 when not known yet).
#[no_mangle]
pub extern "C" fn aztek_update_progress(done: *mut u64, total: *mut u64) {
    let (d, t) = update::progress();
    unsafe {
        if !done.is_null() {
            *done = d;
        }
        if !total.is_null() {
            *total = t;
        }
    }
}

/// A JSON document from a URL, as update::get_json: its text, or {error}. Blocks: call it off the interface thread.
#[no_mangle]
pub extern "C" fn aztek_fetch_json(url: *const c_char) -> *mut c_char {
    out(update::get_json(&cstr(url)))
}

/// The newest mGear release for this Aztek, as mgear::release. Blocks: call it off the interface thread.
#[no_mangle]
pub extern "C" fn aztek_mgear_release() -> *mut c_char {
    out_json(mgear::release())
}

#[no_mangle]
pub extern "C" fn aztek_update_cancel() {
    update::cancel();
}

/// A zipped copy of the scene at this path, leaving the scene as it was. Returns {"ok": bytes} or {"error": message}.
#[no_mangle]
pub extern "C" fn aztek_feedback_scene_zip(path: *const c_char) -> *mut c_char {
    let path = cstr(path);
    with(|h| {
        out_json(match feedback::scene_zip(&mut h.core.s, std::path::Path::new(&path)) {
            Ok(bytes) => json!({ "ok": bytes }),
            Err(e) => json!({ "error": e }),
        })
    })
}

/// aztek_exec without the echo or result lines: scrubbing, playback and other UI chatter the reference doesn't print.
#[no_mangle]
pub extern "C" fn aztek_exec_quiet(name: *const c_char, call: *const c_char) -> *mut c_char {
    let name = cstr(name);
    let call: Value = serde_json::from_str(&cstr(call)).unwrap_or(json!({}));
    with(|h| {
        // Echo All Commands: the quiet ones show too.
        if ui_cmds::pref_bool("echoAllCommands", false) && name != "uiPref" {
            h.core.push(mel::echo(&name, &call), core::LineKind::Echo);
        }
        out_json(match h.core.exec_silent(&name, call) {
            Ok(v) => json!({ "ok": v }),
            Err(e) => {
                h.core.push(format!("// Error: {e} //"), core::LineKind::Error);
                json!({ "error": e })
            }
        })
    })
}

/// A read-only command (a query): no history lines, and the panels aren't told anything changed.
#[no_mangle]
pub extern "C" fn aztek_ask(name: *const c_char, call: *const c_char) -> *mut c_char {
    let name = cstr(name);
    let call: Value = serde_json::from_str(&cstr(call)).unwrap_or(json!({}));
    with(|h| {
        let v = h.core.version;
        let r = h.core.exec_silent(&name, call);
        h.core.version = v;
        out_json(match r {
            Ok(v) => json!({ "ok": v }),
            Err(e) => json!({ "error": e }),
        })
    })
}

/// New history lines since the last call: [{"text", "kind"}], kind 0 echo, 1 result, 2 error, 3 info, 4 warning.
#[no_mangle]
pub extern "C" fn aztek_take_log() -> *mut c_char {
    with(|h| out_json(h.core.take_log()))
}

/// Adds a line to the history (UI-side messages).
#[no_mangle]
pub extern "C" fn aztek_log(text: *const c_char, kind: i32) {
    let text = cstr(text);
    with(|h| {
        let k = match kind {
            0 => core::LineKind::Echo,
            1 => core::LineKind::Result,
            2 => core::LineKind::Error,
            4 => core::LineKind::Warning,
            _ => core::LineKind::Info,
        };
        h.core.push(text, k)
    });
}

#[no_mangle]
pub extern "C" fn aztek_version() -> u64 {
    with(|h| h.core.version)
}

/// Scene snapshots: "outliner" (arg "shapes" to include shapes), "channelBox", "attributeEditor" (arg: node path or
/// empty for the lead selection), "layers", "state", "commands".
#[no_mangle]
pub extern "C" fn aztek_query(what: *const c_char, arg: *const c_char) -> *mut c_char {
    let (what, arg) = (cstr(what), cstr(arg));
    with(|h| {
        let c = &mut h.core;
        c.s.graph.evaluate();
        out_json(match what.as_str() {
            "outliner" => query::outliner(c, &arg),
            "channelBox" => {
                let v = aztek_usd::panels::channel_box(&mut c.s).unwrap_or_else(|| query::channel_box(c));
                channel_box::decorate(&mut c.s, &arg, v)
            }
            // The channels picked in a channel box, from the shell: {"attrs": [...], "plugs": [...]}.
            "uiChannelPicks" => {
                channel_box::shell_picks(&arg);
                Value::Null
            }
            "animCurves" => query::anim_curves(c, &arg),
            "nodeGraph" => query::node_graph(c, &arg),
            "uiRequests" => core::take_ui_requests(),
            "registerUiNames" => {
                let list: Value = serde_json::from_str(&arg).unwrap_or(json!([]));
                ui_cmds::register_ui_names(&mut c.s, &list);
                Value::Null
            }
            "runtimeCommands" => ui_cmds::runtime_commands(),
            "uiDrag" => attr_ui::drag_start(c, &arg),
            "uiDropRect" => attr_ui::drop_rect(c, &arg),
            "aeTemplate" => script_objects::ae_template(c, &arg),
            "panelSync" => {
                panel_cmds::shell_sync(&serde_json::from_str(&arg).unwrap_or(json!({})));
                Value::Null
            }
            "uiCustomAxis" => axis_orient::from_selection(&mut c.s, &arg).map_or(Value::Null, |e| json!(e)),
            "optionBox" => json!(option_boxes::rows(&arg)),
            "uiAfterAction" => {
                let v: Value = serde_json::from_str(&arg).unwrap_or(Value::Null);
                node_manip::after_action(c, &v[0]);
                Value::Null
            }
            "nodeManip" => node_manip::fields_json(c),
            "uiShowManip" => {
                node_manip::show_node(c, &arg);
                Value::Null
            }
            // The channel box's picked channels: "parent" while translate channels are picked (#136).
            "uiWorkspace" => {
                crate::user_folder::set_current_workspace(&arg);
                Value::Null
            }
            // No version bump: that rebuilds the Channel Box, which asks this on every restyle, so a field being typed
            // in lost the keyboard and a drag down the names lost its label. The shell redraws the views on a change.
            "uiChannelSpace" => json!(crate::viewport::set_channel_space(!arg.is_empty())),
            "uiShowManipAttr" => {
                c.node_manip.active_attr = if arg.is_empty() { None } else { Some(arg.clone()) };
                c.version += 1;
                Value::Null
            }
            "uiToolChanged" => {
                ctx_cmds::shell_tool_changed(&arg);
                c.multi_cut = (arg == "multiCut").then(|| {
                    let g = &mut c.s.graph;
                    let targets = aztek_scene::components::hilited_meshes(g);
                    if !targets.is_empty() {
                        aztek_scene::components::set_mode_exact(g, aztek_scene::components::ComponentMode::Edge);
                        c.version += 1;
                    }
                    (g.scene_serial(), targets)
                });
                Value::Null
            }
            "melEcho" => {
                let p: Value = serde_json::from_str(&arg).unwrap_or(json!({}));
                json!(mel::echo(p["name"].as_str().unwrap_or(""), &p["call"]))
            }
            "editors" => query::editors(c, &arg),
            "uvData" => query::uv_data(c),
            "bookmarks" => aztek_scene::bookmarks::all(&c.s.graph),
            "shading" => query::shading(c),
            "uvTexture" => query::uv_texture(c),
            "texturePath" => query::texture_path(c, &arg),
            "attributeEditor" => aztek_usd::panels::attribute_editor(&mut c.s, &arg).unwrap_or_else(|| query::attribute_editor(c, &arg)),
            "layers" => query::layers(c),
            "references" => aztek_scene::references::ui_list(&mut c.s),
            "state" => query::state(c),
            "commands" => query::commands(c),
            "completionCore" => completion::core_index(c),
            "liveProcedures" => completion::live_procedures(c),
            "flagDefaults" => c
                .s
                .commands
                .find(&arg)
                .map(|d| Value::Object(d.flags.iter().map(|f| (f.name.clone(), f.default_value.clone())).collect()))
                .unwrap_or(Value::Null),
            _ => Value::Null,
        })
    })
}

/// The Script Editor's completion index: commands with their flags and help, MEL procedures and built-ins, and the
/// Python modules with their module-level names. The Python half is absent when this build has no Python. Building it
/// starts Python the first time; call it with the host free, as a query handler does.
#[no_mangle]
pub extern "C" fn aztek_completion_index() -> *mut c_char {
    let mut v = with(|h| completion::core_index(&h.core));
    #[cfg(feature = "python")]
    match python::completion_modules() {
        Ok(text) => match serde_json::from_str::<Value>(&text) {
            Ok(p) => v["python"] = p,
            Err(e) => v["pythonError"] = json!(e.to_string()),
        },
        Err(e) => v["pythonError"] = json!(e),
    }
    out_json(v)
}

/// One Python class for the Script Editor's completion: constructor, docstring and members, as JSON. Null when the
/// class is not in the module or this build has no Python.
#[no_mangle]
pub extern "C" fn aztek_completion_class(module: *const c_char, class: *const c_char) -> *mut c_char {
    #[cfg(feature = "python")]
    match python::completion_class(&cstr(module), &cstr(class)) {
        Ok(text) => out(text),
        Err(_) => std::ptr::null_mut(),
    }
    #[cfg(not(feature = "python"))]
    {
        let _ = (module, class);
        std::ptr::null_mut()
    }
}

/// The completion index without Python, for tests and for a build with no interpreter.
pub fn completion_index(core: &Core) -> Value {
    completion::core_index(core)
}

/// Script UI: the operations waiting for the shell, as a JSON array, or null when there are none.
#[no_mangle]
pub extern "C" fn aztek_ui_take_ops() -> *mut c_char {
    let ops = scriptui::take_ops();
    if ops.is_empty() {
        return std::ptr::null_mut();
    }
    out_json(Value::Array(ops))
}

/// Script UI: what the user did. See `scriptui::shell_event`.
#[no_mangle]
pub extern "C" fn aztek_ui_event(ev: *const c_char) {
    let ev: Value = serde_json::from_str(&cstr(ev)).unwrap_or(Value::Null);
    with(|h| h.core.ui_event(&ev));
}

/// Script UI: how commands reach the shell for what must answer at once (dialogs, playblast).
#[no_mangle]
pub extern "C" fn aztek_ui_set_hook(hook: Option<scriptui::UiHook>) {
    scriptui::set_hook(hook);
}

/// The hook's answer, as JSON. No answer means the shell didn't handle it.
#[no_mangle]
pub extern "C" fn aztek_ui_answer(json: *const c_char) {
    scriptui::set_hook_answer(cstr(json));
}

/// The Cancel button of a progress window. Needs no host: a command is running under it.
#[no_mangle]
pub extern "C" fn aztek_ui_progress_cancel() {
    scriptui::progress_cancelled();
}

/// Whether a command is suspended in the hook, with the shell's event loop on top of it.
#[no_mangle]
pub extern "C" fn aztek_ui_in_hook() -> i32 {
    scriptui::in_hook() as i32
}

/// One idle step: script jobs and deferred scripts.
#[no_mangle]
pub extern "C" fn aztek_idle() {
    with(|h| h.core.idle());
}

/// File operations: "new", "open" path, "save" path. Returns an error message or empty.
#[no_mangle]
pub extern "C" fn aztek_file(op: *const c_char, path: *const c_char) -> *mut c_char {
    let (op, path) = (cstr(op), cstr(path));
    with(|h| {
        let r = match op.as_str() {
            "new" => {
                h.core.new_scene();
                Ok(())
            }
            "open" => h.core.open(&path),
            "save" => h.core.save(&path),
            _ => Err(format!("unknown file operation {op}")),
        };
        match r {
            Ok(()) => out(String::new()),
            Err(e) => {
                h.core.push(format!("// Error: {e} //"), core::LineKind::Error);
                out(e)
            }
        }
    })
}

/// Runs Python source in the embedded interpreter (aztek.cmds, maya.cmds). Returns 0 when this build has no Python.
#[no_mangle]
pub extern "C" fn aztek_python(src: *const c_char) -> i32 {
    let src = cstr(src);
    #[cfg(feature = "python")]
    {
        with(|h| h.core.push(src.clone(), core::LineKind::Echo));
        python::run(&src);
        1
    }
    #[cfg(not(feature = "python"))]
    {
        let _ = src;
        with(|h| h.core.push("# Error: this build has no Python #", core::LineKind::Error));
        0
    }
}

#[no_mangle]
pub extern "C" fn aztek_undo() {
    with(|h| {
        let _ = h.core.exec("undo", json!({}));
    })
}

/// Takes back a preview step (a drag's last change) without a history line.
#[no_mangle]
pub extern "C" fn aztek_retract() {
    with(|h| h.core.retract_preview())
}

#[no_mangle]
pub extern "C" fn aztek_redo() {
    with(|h| {
        let _ = h.core.exec("redo", json!({}));
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// Views

/// Platforms for aztek_view_create.
pub const PLATFORM_XCB: i32 = 1;
pub const PLATFORM_XLIB: i32 = 2;
pub const PLATFORM_WIN32: i32 = 3;
pub const PLATFORM_WAYLAND: i32 = 4;
pub const PLATFORM_APPKIT: i32 = 5;

fn handles(platform: i32, display: *mut c_void, window: u64, extra: *mut c_void) -> Option<(RawDisplayHandle, RawWindowHandle)> {
    use raw_window_handle::*;
    Some(match platform {
        PLATFORM_XCB => (
            RawDisplayHandle::Xcb(XcbDisplayHandle::new(NonNull::new(display), 0)),
            RawWindowHandle::Xcb(XcbWindowHandle::new(NonZeroU32::new(window as u32)?)),
        ),
        PLATFORM_XLIB => (
            RawDisplayHandle::Xlib(XlibDisplayHandle::new(NonNull::new(display), 0)),
            RawWindowHandle::Xlib(XlibWindowHandle::new(window as std::ffi::c_ulong)),
        ),
        PLATFORM_WIN32 => {
            let mut w = Win32WindowHandle::new(std::num::NonZeroIsize::new(window as isize)?);
            w.hinstance = std::num::NonZeroIsize::new(extra as isize);
            (RawDisplayHandle::Windows(WindowsDisplayHandle::new()), RawWindowHandle::Win32(w))
        }
        PLATFORM_WAYLAND => (
            RawDisplayHandle::Wayland(WaylandDisplayHandle::new(NonNull::new(display)?)),
            RawWindowHandle::Wayland(WaylandWindowHandle::new(NonNull::new(extra)?)),
        ),
        // window is the NSView.
        PLATFORM_APPKIT => (
            RawDisplayHandle::AppKit(AppKitDisplayHandle::new()),
            RawWindowHandle::AppKit(AppKitWindowHandle::new(NonNull::new(window as usize as *mut c_void)?)),
        ),
        _ => return None,
    })
}

/// Makes a model panel on a native window. camera: "persp", "top", "front" or "side". Returns its id, or 0 with the
/// reason in the history.
#[no_mangle]
pub extern "C" fn aztek_view_create(
    platform: i32,
    display: *mut c_void,
    window: u64,
    extra: *mut c_void,
    width: i32,
    height: i32,
    dpr: f64,
    camera: *const c_char,
) -> i32 {
    let cam = camera::CamKind::from_name(&cstr(camera)).unwrap_or(camera::CamKind::Persp);
    with(|h| {
        let Some((dh, wh)) = handles(platform, display, window, extra) else {
            h.core.push("// Error: viewport: unsupported window system //", core::LineKind::Error);
            return 0;
        };
        if h.shared.is_none() {
            match unsafe { Shared::new(dh, wh) } {
                Ok(s) => {
                    h.core.push(format!("// Vulkan device: {} //", s.device_name), core::LineKind::Info);
                    h.shared = Some(s);
                }
                Err(e) => {
                    h.core.push(format!("// Error: Vulkan: {e} //"), core::LineKind::Error);
                    return 0;
                }
            }
        }
        let shared = h.shared.clone().unwrap();
        match unsafe { Gpu::new(shared, dh, wh, width.max(1) as u32, height.max(1) as u32) } {
            Ok(gpu) => {
                let id = h.next_view;
                h.next_view += 1;
                h.views.insert(id, Box::new(View::new(gpu, cam, dpr)));
                id
            }
            Err(e) => {
                h.core.push(format!("// Error: viewport: {e} //"), core::LineKind::Error);
                0
            }
        }
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_destroy(id: i32) {
    with(|h| close_slot(&mut h.views, id, release_view))
}

#[no_mangle]
pub extern "C" fn aztek_view_resize(id: i32, width: i32, height: i32) {
    with(|h| {
        use_view(h, id, |_, v| v.resize(width.max(1) as u32, height.max(1) as u32));
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_render(id: i32) {
    with(|h| {
        use_view(h, id, |core, v| v.render(core));
    })
}

/// Mouse events in physical pixels. kind 0 press, 1 move, 2 release; button 1 left, 2 middle, 3 right; mods bits
/// 1 shift, 2 ctrl, 4 alt; buttons the held set (bit 1 left, 2 middle, 4 right). Returns 1 to redraw.
#[no_mangle]
pub extern "C" fn aztek_view_mouse(id: i32, kind: i32, button: i32, x: f64, y: f64, mods: i32, buttons: i32) -> i32 {
    // The viewport works in what's stored; what it echoes goes back to the current units.
    aztek_ops::units::stored(|| {
    with(|h| {
        use_view(h, id, |core, v| {
            // Where the pointer is, for autoPlace -um.
            let size = v.vp.size();
            let rays = crate::host_general::ViewRays { centre: v.vp.cam.ray(size * 0.5, size), pointer: v.vp.cam.ray(DVec2::new(x, y), size) };
            crate::host_general::note_pointer(&v.cam_node, rays);
            v.mouse(core, kind, button, DVec2::new(x, y), crate::host_general::forced_modifiers(mods), buttons) as i32
        })
        .unwrap_or(0)
    })
    })
}

/// Whether a point, in device pixels, is on the view's ViewCube: a right click there opens the cube's menu, not the
/// marking menu.
#[no_mangle]
pub extern "C" fn aztek_view_on_cube(id: i32, x: f64, y: f64) -> i32 {
    with(|h| use_view(h, id, |_, v| v.on_cube(DVec2::new(x, y))).unwrap_or(false) as i32)
}

#[no_mangle]
pub extern "C" fn aztek_view_wheel(id: i32, delta: f64) {
    // The viewport works in what's stored; what it echoes goes back to the current units.
    aztek_ops::units::stored(|| {
    with(|h| {
        use_view(h, id, |core, v| v.wheel(core, delta));
    })
    })
}

/// View settings: "tool" ("select", "move", "rotate", "scale"), "wireframe", "wireOnShaded", "grid", "camera",
/// "frame" ("selection" or "all"), "home", "dpr". value is JSON.
#[no_mangle]
pub extern "C" fn aztek_view_set(id: i32, key: *const c_char, value: *const c_char) {
    // The viewport works in what's stored; what it echoes goes back to the current units.
    aztek_ops::units::stored(|| {
    let key = cstr(key);
    let v: Value = serde_json::from_str(&cstr(value)).unwrap_or(Value::Null);
    with(|h| {
        let ids: Vec<i32> = if id == 0 { h.views.keys().copied().collect() } else { vec![id] };
        for i in ids {
            use_view(h, i, |core, view| view.set(core, &key, &v));
        }
    })
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_get(id: i32, key: *const c_char) -> *mut c_char {
    let key = cstr(key);
    with(|h| out_json(use_view(h, id, |_, v| v.get(&key)).unwrap_or(Value::Null)))
}

/// Renders a view and writes it as a binary PPM (for tests and bug reports). Returns 1 on success.
#[no_mangle]
pub extern "C" fn aztek_view_capture(id: i32, path: *const c_char) -> i32 {
    let path = cstr(path);
    with(|h| {
        use_view(h, id, |core, v| {
            v.want_capture = true;
            v.render(core);
            let Some((w, hgt, rgb)) = v.capture.take() else { return 0 };
            let mut data = format!("P6\n{w} {hgt}\n255\n").into_bytes();
            data.extend_from_slice(&rgb);
            std::fs::write(&path, data).is_ok() as i32
        })
        .unwrap_or(0)
    })
}

/// The object under a view pixel: its transform's path, or empty.
#[no_mangle]
pub extern "C" fn aztek_view_hit(id: i32, x: f64, y: f64) -> *mut c_char {
    with(|h| {
        let hit = use_view(h, id, |core, v| {
            let items = viewport::gather(&mut core.s.graph);
            v.vp.rect = [0.0, 0.0, v.gpu.extent.width as f64, v.gpu.extent.height as f64];
            let px = DVec2::new(x, y);
            if let Some(id) = v.vp.pick_rig(&core.s.graph, px) {
                return core.s.graph.path(id);
            }
            v.vp.raycast(&items, px).map(|(i, _, _)| core.s.graph.path(items[i].xf)).unwrap_or_default()
        });
        out(hit.unwrap_or_default())
    })
}

#[cfg(all(test, feature = "python"))]
mod tests {
    use super::*;

    /// A tool's Python callback can run a nested Qt event loop (`QApplication.processEvents()`, which the Red Kite
    /// shelves do while building their tabs). The shell's poller and paint calls reach the host from that loop, and must
    /// find it, as they do from a dialog's hook.
    #[test]
    fn the_host_is_lent_to_a_nested_event_loop_in_python() {
        with(|h| {
            let core_ptr: *mut core::Core = &mut h.core;
            let version = h.core.version;
            python::test_nested(core_ptr, || {
                with(|h| h.core.version += 1);
            });
            assert!(h.core.version > version);
        });
    }

    /// From a plug-in command or an undo step the session has moved out into a nested core: a call from a nested event
    /// loop there finds the scene, and what it does stays when the command returns.
    #[test]
    fn a_nested_event_loop_in_a_plugin_command_reaches_the_scene() {
        with(|h| {
            h.core.mel("createNode transform -n beforeNested;");
            let s: *mut aztek_scene::Session = &mut h.core.s;
            python::test_nested_session(unsafe { &mut *s }, || {
                with(|h| {
                    assert!(h.core.s.graph.find("beforeNested").ok().flatten().is_some(), "the lent session is the scene");
                    h.core.mel("createNode transform -n fromNested;");
                });
            });
            assert!(h.core.s.graph.find("fromNested").ok().flatten().is_some(), "what the nested call made stays");
        });
    }

    /// Inside a node's compute the graph is mid-evaluation: the shell isn't let in even with a callback's core set.
    #[test]
    fn a_compute_does_not_lend_the_host() {
        let mut c = core::Core::new();
        python::test_nested(&mut c, || {
            assert!(in_host_callback());
            python::test_in_compute(|| assert!(!in_host_callback()));
        });
    }

    /// A call from a nested event loop leaves alone a view a call under it is working in, and a view closed from there
    /// goes once that call returns; another view made meanwhile doesn't move the one in use.
    #[test]
    fn a_view_in_use_is_left_alone_by_nested_calls() {
        let mut slots: HashMap<i32, Box<u32>> = HashMap::from([(1, Box::new(10)), (2, Box::new(20))]);
        let map: *mut HashMap<i32, Box<u32>> = &mut slots;
        let dropped = RefCell::new(Vec::new());
        let r = use_slot(unsafe { &mut *map }, &mut (), 1, |b| dropped.borrow_mut().push(*b), |_, v| {
            let before: *const u32 = v;
            // The nested call: view 1 is skipped, view 2 is used, and new views go in.
            let inner = unsafe { &mut *map };
            assert_eq!(use_slot(inner, &mut (), 1, |_| {}, |_, _| ()), None);
            assert_eq!(use_slot(inner, &mut (), 2, |_| {}, |_, w| *w), Some(20));
            for k in 3..200 {
                inner.insert(k, Box::new(k as u32));
            }
            close_slot(inner, 1, |_| panic!("dropped under the call using it"));
            assert!(std::ptr::eq(before, v as *const u32), "the view in use moved");
            assert!(dropped.borrow().is_empty());
            *v += 1;
            *v
        });
        assert_eq!(r, Some(11));
        // Closed while in use: dropped once the call returned, with what that call left in it.
        assert_eq!(*dropped.borrow(), vec![11]);
        assert!(!slots.contains_key(&1));
        // Not in use: closed at once.
        close_slot(&mut slots, 2, |b| dropped.borrow_mut().push(*b));
        assert_eq!(*dropped.borrow(), vec![11, 20]);
    }
}
