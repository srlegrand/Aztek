//! Environment variables in a session: what start-up sets, the Script Editor's lines going to files and standard
//! output, and the commands that ask about the environment. The list of variables and the rule for their names are in
//! `aztek_core::env`.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Value};

/// The files `cmdFileOutput` has open, by descriptor.
static OUTPUTS: Mutex<Vec<(i64, PathBuf, File)>> = Mutex::new(Vec::new());
static NEXT: Mutex<i64> = Mutex::new(1);
/// The file `MAYA_CMD_FILE_OUTPUT` opened, so a second start-up in one process does not open it again.
static FROM_ENV: Mutex<Option<String>> = Mutex::new(None);

/// What start-up sets and opens. Safe to call again.
pub fn startup() {
    // Where the application is: scripts written for the reference build paths from MAYA_LOCATION.
    let here = std::env::var_os("AZTEK_LOCATION").filter(|v| !v.is_empty()).map(PathBuf::from).unwrap_or_else(aztek_core::paths::executable_dir);
    if !here.as_os_str().is_empty() {
        std::env::set_var("AZTEK_LOCATION", &here);
        std::env::set_var("MAYA_LOCATION", &here);
    }
    if let Some(file) = aztek_core::env::get("MAYA_CMD_FILE_OUTPUT") {
        let mut from_env = FROM_ENV.lock().unwrap();
        if from_env.as_deref() != Some(file.as_str()) {
            let _ = open(&file);
            *from_env = Some(file);
        }
    }
}

fn open(file: &str) -> Result<i64, String> {
    let f = File::create(file).map_err(|e| format!("{file}: {e}"))?;
    let mut next = NEXT.lock().unwrap();
    let id = *next;
    *next += 1;
    OUTPUTS.lock().unwrap().push((id, PathBuf::from(file), f));
    Ok(id)
}

/// A Script Editor line, on its way to the files `cmdFileOutput` has open and to standard output when
/// `MAYA_SCRIPT_EDITOR_STDOUT` is on.
pub fn line(text: &str) {
    if let Ok(mut outs) = OUTPUTS.try_lock() {
        for (_, _, f) in outs.iter_mut() {
            let _ = writeln!(f, "{text}");
            let _ = f.flush();
        }
    }
    // Batch runs already write their lines out.
    if !aztek_core::batch_mode() && aztek_core::env::on("MAYA_SCRIPT_EDITOR_STDOUT") {
        println!("{text}");
    }
}

fn cmd_file_output(_s: &mut Session, a: &Args) -> CmdResult {
    if let Some(file) = a.get("open").and_then(Value::as_str) {
        return Ok(json!(match open(file) {
            Ok(id) => id,
            Err(e) => {
                log::warn!("cmdFileOutput: {e}");
                -1
            }
        }));
    }
    if a.bool("closeAll") {
        OUTPUTS.lock().unwrap().clear();
        return Ok(json!(0));
    }
    if a.has("close") {
        let id = a.int("close", -1);
        let mut outs = OUTPUTS.lock().unwrap();
        let before = outs.len();
        outs.retain(|(i, _, _)| *i != id);
        return Ok(json!(if outs.len() < before { 0 } else { -1 }));
    }
    if a.has("status") {
        let id = a.int("status", -1);
        return Ok(json!(if OUTPUTS.lock().unwrap().iter().any(|(i, _, _)| *i == id) { 0 } else { -3 }));
    }
    Err(CommandError::new("cmdFileOutput: give -open file, -close descriptor, -closeAll or -status descriptor"))
}

fn environment_cmd(_s: &mut Session, a: &Args) -> CmdResult {
    use aztek_core::env;
    if let Some(name) = a.get("icon").and_then(Value::as_str) {
        return Ok(json!(env::find_icon(name).map(|p| p.to_string_lossy().into_owned()).unwrap_or_default()));
    }
    if let Some(name) = a.get("list").and_then(Value::as_str) {
        let dirs = if name == "XBMLANGPATH" { env::icon_dirs() } else { env::list(name) };
        return Ok(json!(dirs.iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>()));
    }
    if let Some(name) = a.get("value").and_then(Value::as_str) {
        return Ok(json!(env::get(name).unwrap_or_default()));
    }
    if a.bool("shelfFiles") {
        return Ok(json!(shelf_files()));
    }
    let status = |s: env::Status| match s {
        env::Status::Read => "yes",
        env::Status::Partly => "partly",
        env::Status::NotRead => "no",
    };
    Ok(json!(env::VARS
        .iter()
        .map(|x| json!({"name": x.name, "ownName": env::own_name(x.name), "group": x.group, "read": status(x.status), "note": x.note, "value": env::get(x.name)}))
        .collect::<Vec<_>>()))
}

/// The `shelf_*.mel` files in the folders of `MAYA_SHELF_PATH`, in the path's order and by name within a folder.
pub fn shelf_files() -> Vec<String> {
    let mut out = Vec::new();
    for dir in aztek_core::env::list("MAYA_SHELF_PATH") {
        let Ok(rd) = std::fs::read_dir(&dir) else { continue };
        let mut files: Vec<PathBuf> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "mel") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("shelf_")))
            .collect();
        files.sort();
        out.extend(files.into_iter().map(|p| p.to_string_lossy().into_owned()));
    }
    out
}

pub fn register_commands(s: &mut Session) {
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    let t = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::String, Value::Null, doc);
    let i = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Int, Value::Null, doc);
    s.register(
        CommandDef::<Session>::new(
            "cmdFileOutput",
            "Writes the Script Editor's lines to a file as well: -open file gives a descriptor, -close descriptor and -closeAll stop, -status descriptor is 0 while the file is open.",
            cmd_file_output,
        )
        .flag(t("open", "o", "Open this file and write every line to it from now on. The descriptor, or -1."))
        .flag(i("close", "c", "Close the file with this descriptor."))
        .flag(b("closeAll", "ca", "Close them all."))
        .flag(i("status", "s", "0 while the file with this descriptor is open."))
        .flag(b("query", "q", "Query."))
        .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new(
            "aztekEnvironment",
            "The environment variables Aztek knows of: -q lists each with whether it is read and its value in force; -value name gives one, Aztek's own spelling first; -list name gives a list of folders; -icon name finds an icon on XBMLANGPATH; -shelfFiles gives the shelf files on MAYA_SHELF_PATH.",
            environment_cmd,
        )
        .flag(b("query", "q", "Every variable: name, group, whether it is read, a note, and its value."))
        .flag(t("value", "v", "The value in force of this variable."))
        .flag(t("list", "l", "The folders of this variable, in order."))
        .flag(t("icon", "i", "The file of this icon, or an empty string."))
        .flag(b("shelfFiles", "sf", "The shelf_*.mel files on the shelf path."))
        .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("xbmLangPathList", "The folders icons are looked for in.", |_s: &mut Session, _a: &Args| {
            Ok(json!(aztek_core::env::icon_dirs().iter().map(|p| p.to_string_lossy().into_owned()).collect::<Vec<_>>()))
        })
        .undoable(false),
    );
}
