//! The host library the Qt shell links: a C interface over the session, script history, scene snapshots and the
//! Vulkan model panels. Strings are UTF-8; strings returned are freed with aztek_free. Main thread only.

pub mod dyn_draw;
pub mod axis_orient;
mod batch;
pub mod camera;
mod core;
pub mod ctx_cmds;
pub mod feedback;
use aztek_geo::stroke_font as font;
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
mod render;
pub mod textures;
pub mod lighting;
pub mod script_jobs;
pub mod attr_ui;
pub mod script_objects;
pub mod host_general;
pub mod scriptui;
pub mod tools;
pub mod ui_cmds;
pub mod view;
pub mod usd_view;
pub mod viewport;

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
    views: HashMap<i32, View>,
    next_view: i32,
    shared: Option<Arc<Shared>>,
}

thread_local! {
    static HOST: RefCell<Option<Host>> = const { RefCell::new(None) };
    /// The host, for calls that arrive while a command is suspended in a shell hook.
    static LIVE: std::cell::Cell<*mut Host> = const { std::cell::Cell::new(std::ptr::null_mut()) };
}

/// The host. A dialog a command opens runs the shell's event loop on top of that command, whose frames hold the host: the
/// shell's calls from that loop take it from `LIVE`. Those frames are suspended in the hook call and touch nothing until
/// it returns. Any other second borrow is a bug, and panics.
pub(crate) fn with<R>(f: impl FnOnce(&mut Host) -> R) -> R {
    HOST.with(|h| match h.try_borrow_mut() {
        Ok(mut b) => {
            if b.is_none() {
                crate::core::install_logger();
                *b = Some(Host { core: Core::new(), views: HashMap::new(), next_view: 1, shared: None });
            }
            let host = b.as_mut().unwrap();
            LIVE.with(|l| l.set(host as *mut Host));
            f(host)
        }
        Err(_) => {
            assert!(scriptui::in_hook(), "the host was entered twice");
            let host = unsafe { &mut *LIVE.with(std::cell::Cell::get) };
            lend_session(host, f)
        }
    })
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

#[no_mangle]
pub extern "C" fn aztek_init() {
    with(|_| ());
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
            "channelBox" => aztek_usd::panels::channel_box(&mut c.s).unwrap_or_else(|| query::channel_box(c)),
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
            "uiShowManipAttr" => {
                c.node_manip.active_attr = if arg.is_empty() { None } else { Some(arg.clone()) };
                c.version += 1;
                Value::Null
            }
            "uiToolChanged" => {
                ctx_cmds::shell_tool_changed(&arg);
                Value::Null
            }
            "melEcho" => {
                let p: Value = serde_json::from_str(&arg).unwrap_or(json!({}));
                json!(mel::echo(p["name"].as_str().unwrap_or(""), &p["call"]))
            }
            "editors" => query::editors(c, &arg),
            "uvData" => query::uv_data(c),
            "shading" => query::shading(c),
            "uvTexture" => query::uv_texture(c),
            "texturePath" => query::texture_path(c, &arg),
            "attributeEditor" => aztek_usd::panels::attribute_editor(&mut c.s, &arg).unwrap_or_else(|| query::attribute_editor(c, &arg)),
            "layers" => query::layers(c),
            "references" => aztek_scene::references::ui_list(&mut c.s),
            "state" => query::state(c),
            "commands" => query::commands(c),
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
                h.views.insert(id, View::new(gpu, cam, dpr));
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
    with(|h| {
        if let Some(mut v) = h.views.remove(&id) {
            v.vp.release(&mut v.gpu);
        }
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_resize(id: i32, width: i32, height: i32) {
    with(|h| {
        if let Some(v) = h.views.get_mut(&id) {
            v.resize(width.max(1) as u32, height.max(1) as u32);
        }
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_render(id: i32) {
    with(|h| {
        let Host { core, views, .. } = h;
        if let Some(v) = views.get_mut(&id) {
            v.render(core);
        }
    })
}

/// Mouse events in physical pixels. kind 0 press, 1 move, 2 release; button 1 left, 2 middle, 3 right; mods bits
/// 1 shift, 2 ctrl, 4 alt; buttons the held set (bit 1 left, 2 middle, 4 right). Returns 1 to redraw.
#[no_mangle]
pub extern "C" fn aztek_view_mouse(id: i32, kind: i32, button: i32, x: f64, y: f64, mods: i32, buttons: i32) -> i32 {
    with(|h| {
        let Host { core, views, .. } = h;
        match views.get_mut(&id) {
            Some(v) => v.mouse(core, kind, button, DVec2::new(x, y), mods, buttons) as i32,
            None => 0,
        }
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_wheel(id: i32, delta: f64) {
    with(|h| {
        if let Some(v) = h.views.get_mut(&id) {
            v.wheel(&mut h.core, delta);
        }
    })
}

/// View settings: "tool" ("select", "move", "rotate", "scale"), "wireframe", "wireOnShaded", "grid", "camera",
/// "frame" ("selection" or "all"), "home", "dpr". value is JSON.
#[no_mangle]
pub extern "C" fn aztek_view_set(id: i32, key: *const c_char, value: *const c_char) {
    let key = cstr(key);
    let v: Value = serde_json::from_str(&cstr(value)).unwrap_or(Value::Null);
    with(|h| {
        let Host { core, views, .. } = h;
        if id == 0 {
            for v2 in views.values_mut() {
                v2.set(core, &key, &v);
            }
        } else if let Some(view) = views.get_mut(&id) {
            view.set(core, &key, &v);
        }
    })
}

#[no_mangle]
pub extern "C" fn aztek_view_get(id: i32, key: *const c_char) -> *mut c_char {
    let key = cstr(key);
    with(|h| out_json(h.views.get(&id).map(|v| v.get(&key)).unwrap_or(Value::Null)))
}

/// Renders a view and writes it as a binary PPM (for tests and bug reports). Returns 1 on success.
#[no_mangle]
pub extern "C" fn aztek_view_capture(id: i32, path: *const c_char) -> i32 {
    let path = cstr(path);
    with(|h| {
        let Host { core, views, .. } = h;
        let Some(v) = views.get_mut(&id) else { return 0 };
        v.want_capture = true;
        v.render(core);
        let Some((w, hgt, rgb)) = v.capture.take() else { return 0 };
        let mut data = format!("P6\n{w} {hgt}\n255\n").into_bytes();
        data.extend_from_slice(&rgb);
        std::fs::write(&path, data).is_ok() as i32
    })
}

/// The object under a view pixel: its transform's path, or empty.
#[no_mangle]
pub extern "C" fn aztek_view_hit(id: i32, x: f64, y: f64) -> *mut c_char {
    with(|h| {
        let Host { core, views, .. } = h;
        let Some(v) = views.get_mut(&id) else { return out(String::new()) };
        let items = viewport::gather(&mut core.s.graph);
        v.vp.rect = [0.0, 0.0, v.gpu.extent.width as f64, v.gpu.extent.height as f64];
        let px = DVec2::new(x, y);
        if let Some(id) = v.vp.pick_rig(&core.s.graph, px) {
            return out(core.s.graph.path(id));
        }
        let hit = v.vp.raycast(&items, px).map(|(i, _, _)| core.s.graph.path(items[i].xf)).unwrap_or_default();
        out(hit)
    })
}
