//! Plug-ins that come with Aztek: features kept apart from the core, each registered when its plug-in loads and gone
//! when it unloads. They load at start unless turned off in the Plug-in Manager, and answer to loadPlugin, unloadPlugin
//! and pluginInfo by name like any other.

use std::cell::RefCell;

use aztek_scene::session::Session;
use serde_json::{json, Value};

pub struct Builtin {
    pub name: &'static str,
    pub doc: &'static str,
    /// The commands it registers, which unloading takes away.
    pub commands: &'static [&'static str],
    pub register: fn(&mut Session),
}

pub const VENDOR: &str = "Aztek";
/// The folder the Plug-in Manager lists them under.
pub const FOLDER: &str = "Aztek (built in)";

pub const BUILTINS: &[Builtin] = &[
    Builtin {
        name: "fbxExport",
        doc: "FBX export: File > Export with a .fbx name, exportFbx and FBXExport.",
        commands: aztek_scene::fbx_export::COMMANDS,
        register: |s| aztek_scene::fbx_export::register(&mut s.commands),
    },
    Builtin {
        name: "gltfExport",
        doc: "glTF 2.0 export: File > Export with a .glb or .gltf name, and exportGltf.",
        commands: aztek_scene::gltf_export::COMMANDS,
        register: |s| aztek_scene::gltf_export::register(&mut s.commands),
    },
];

thread_local! {
    static LOADED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

pub fn find(name: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.name == name)
}

pub fn is_loaded(name: &str) -> bool {
    LOADED.with(|l| l.borrow().contains(&name))
}

pub fn loaded() -> Vec<String> {
    LOADED.with(|l| l.borrow().iter().map(|n| n.to_string()).collect())
}

pub fn load(s: &mut Session, b: &'static Builtin) {
    if is_loaded(b.name) {
        return;
    }
    (b.register)(s);
    LOADED.with(|l| l.borrow_mut().push(b.name));
}

pub fn unload(s: &mut Session, b: &'static Builtin) {
    for c in b.commands {
        s.commands.unregister(c);
    }
    LOADED.with(|l| l.borrow_mut().retain(|n| *n != b.name));
}

fn off_file() -> std::path::PathBuf {
    crate::user_folder::prefs_base().join("builtinPlugins.json")
}

/// The ones turned off: every other loads at start.
fn off_list() -> Vec<String> {
    std::fs::read_to_string(off_file())
        .ok()
        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
        .and_then(|v| v["off"].as_array().map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()))
        .unwrap_or_default()
}

pub fn autoload(name: &str) -> bool {
    !off_list().iter().any(|n| n == name)
}

pub fn set_autoload(name: &str, on: bool) -> std::io::Result<()> {
    let mut off = off_list();
    off.retain(|n| n != name);
    if !on {
        off.push(name.to_string());
    }
    std::fs::create_dir_all(crate::user_folder::prefs_base())?;
    std::fs::write(off_file(), json!({"off": off}).to_string())
}

/// At start: every one that has not been turned off.
pub fn load_defaults(s: &mut Session) {
    let off = off_list();
    for b in BUILTINS {
        if !off.iter().any(|n| n == b.name) {
            load(s, b);
        }
    }
}

/// The Plug-in Manager's rows for them.
pub fn rows() -> Vec<Value> {
    let off = off_list();
    BUILTINS
        .iter()
        .map(|b| json!({"name": b.name, "file": b.name, "path": b.name, "folder": FOLDER, "kind": "aztek", "loaded": is_loaded(b.name), "autoload": !off.iter().any(|n| n == b.name), "note": b.doc}))
        .collect()
}
