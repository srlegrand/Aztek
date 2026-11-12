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
        name: "fbx",
        doc: "FBX import and export: File > Import and File > Export with a .fbx name, importFbx, exportFbx, FBXImport and FBXExport.",
        commands: &["exportFbx", "FBXExport", "importFbx", "FBXImport"],
        register: |s| {
            aztek_scene::fbx_export::register(&mut s.commands);
            aztek_scene::fbx_import::register(&mut s.commands);
        },
    },
    Builtin {
        name: "gltf",
        doc: "glTF 2.0 import and export: File > Import and File > Export with a .glb or .gltf name, importGltf and exportGltf.",
        commands: &["exportGltf", "importGltf"],
        register: |s| {
            aztek_scene::gltf_export::register(&mut s.commands);
            aztek_scene::gltf_import::register(&mut s.commands);
        },
    },
];

/// The names the two went by when they only exported.
const OLD_NAMES: &[(&str, &str)] = &[("fbxExport", "fbx"), ("gltfExport", "gltf")];

thread_local! {
    static LOADED: RefCell<Vec<&'static str>> = const { RefCell::new(Vec::new()) };
}

pub fn find(name: &str) -> Option<&'static Builtin> {
    let name = OLD_NAMES.iter().find(|o| o.0 == name).map_or(name, |o| o.1);
    BUILTINS.iter().find(|b| b.name == name)
}

pub fn is_loaded(name: &str) -> bool {
    let Some(b) = find(name) else { return false };
    LOADED.with(|l| l.borrow().contains(&b.name))
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
    let name = find(name).map_or(name, |b| b.name);
    !off_list().iter().any(|n| find(n).map_or(n.as_str(), |b| b.name) == name)
}

pub fn set_autoload(name: &str, on: bool) -> std::io::Result<()> {
    let name = find(name).map_or(name, |b| b.name);
    let mut off = off_list();
    off.retain(|n| find(n).map_or(n.as_str(), |b| b.name) != name);
    if !on {
        off.push(name.to_string());
    }
    std::fs::create_dir_all(crate::user_folder::prefs_base())?;
    std::fs::write(off_file(), json!({"off": off}).to_string())
}

/// At start: every one that has not been turned off.
pub fn load_defaults(s: &mut Session) {
    for b in BUILTINS {
        if autoload(b.name) {
            load(s, b);
        }
    }
}

/// The Plug-in Manager's rows for them.
pub fn rows() -> Vec<Value> {
    BUILTINS
        .iter()
        .map(|b| json!({"name": b.name, "file": b.name, "path": b.name, "folder": FOLDER, "kind": "aztek", "loaded": is_loaded(b.name), "autoload": autoload(b.name), "note": b.doc}))
        .collect()
}
