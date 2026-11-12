//! The user folder: where scripts, plug-ins, modules and preferences a person keeps between versions are looked for.
//! Laid out as the reference lays out its own (`scripts`, `plug-ins`, `modules`, `prefs`, and the same again under a
//! version's folder), so an existing one can be used as it is.
//!
//! Three choices, kept in `userFolder.json` beside Aztek's other preferences:
//! - `aztek`: Aztek's own folder (`~/aztek`), the default. An existing folder can be copied into it.
//! - `link`: an existing folder of the reference's, used in place and never written to.
//! - `custom`: any other folder with the same layout.
//!
//! Applying a choice puts its folders on the script, plug-in and module paths (through `AZTEK_SCRIPT_PATH`,
//! `AZTEK_PLUG_IN_PATH`, `AZTEK_MODULE_PATH` and `PYTHONPATH`, which everything that searches reads), reads the
//! folder's environment file and its modules' `.mod` files, and offers its saved optionVars as defaults.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Value};

/// What was chosen.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    /// "aztek", "link" or "custom".
    pub mode: String,
    /// The folder for link and custom.
    pub folder: String,
    /// The version's subfolder to use ("2025"); empty takes the newest there is.
    pub version: String,
    /// Whether userSetup scripts run at start.
    pub user_setup: bool,
}

impl Default for Choice {
    fn default() -> Self {
        Choice { mode: "aztek".into(), folder: String::new(), version: String::new(), user_setup: true }
    }
}

/// A module a `.mod` file describes.
#[derive(Clone, Debug)]
pub struct Module {
    pub name: String,
    pub version: String,
    pub path: PathBuf,
    pub definition: PathBuf,
}

/// What is in force: the folders on each path, in order, and the modules found.
#[derive(Clone, Debug, Default)]
pub struct Applied {
    pub root: PathBuf,
    pub version: String,
    pub scripts: Vec<PathBuf>,
    pub plugins: Vec<PathBuf>,
    pub modules: Vec<PathBuf>,
    pub prefs: Option<PathBuf>,
    pub found: Vec<Module>,
    /// What each path variable was given, to take back when the choice changes.
    added: Vec<(String, PathBuf)>,
}

static APPLIED: Mutex<Option<Applied>> = Mutex::new(None);

fn home() -> PathBuf {
    PathBuf::from(std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).unwrap_or_default())
}

/// Where Aztek's own preference files are: `AZTEK_PREFS`, or `~/.aztek`.
pub fn prefs_base() -> PathBuf {
    std::env::var_os("AZTEK_PREFS").map(PathBuf::from).unwrap_or_else(|| home().join(".aztek"))
}

/// Aztek's own user folder: `AZTEK_APP_DIR`, or `~/aztek` (in Documents on Windows, where such folders live).
pub fn own_folder() -> PathBuf {
    if let Some(d) = std::env::var_os("AZTEK_APP_DIR").filter(|d| !d.is_empty()) {
        return PathBuf::from(d);
    }
    if cfg!(windows) {
        home().join("Documents").join("aztek")
    } else {
        home().join("aztek")
    }
}

/// Where the reference keeps its user folder on this system, if one is there: `MAYA_APP_DIR`, or its default place.
pub fn detect_reference_folder() -> Option<PathBuf> {
    let mut places = Vec::new();
    if let Some(d) = std::env::var_os("MAYA_APP_DIR").filter(|d| !d.is_empty()) {
        places.push(PathBuf::from(d));
    }
    if cfg!(windows) {
        places.push(home().join("Documents").join("maya"));
    } else if cfg!(target_os = "macos") {
        places.push(home().join("Library").join("Preferences").join("Autodesk").join("maya"));
    } else {
        places.push(home().join("maya"));
    }
    places.into_iter().find(|p| p.is_dir())
}

/// The versions a folder holds, newest first: its subfolders named by a year, or a year and a suffix, that hold
/// `prefs`, `scripts`, `plug-ins` or `modules`.
pub fn versions(folder: &Path) -> Vec<String> {
    let Ok(rd) = std::fs::read_dir(folder) else { return Vec::new() };
    let mut out: Vec<String> = rd
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.len() >= 4 && n.chars().take(4).all(|c| c.is_ascii_digit()))
        .filter(|n| ["prefs", "scripts", "plug-ins", "modules"].iter().any(|s| folder.join(n).join(s).is_dir()))
        .collect();
    out.sort_by(|a, b| b.cmp(a));
    out
}

fn choice_file() -> PathBuf {
    prefs_base().join("userFolder.json")
}

pub fn load_choice() -> Choice {
    let Some(v) = std::fs::read_to_string(choice_file()).ok().and_then(|t| serde_json::from_str::<Value>(&t).ok()) else { return Choice::default() };
    let text = |k: &str| v[k].as_str().unwrap_or("").to_string();
    let mode = text("mode");
    Choice {
        mode: if ["aztek", "link", "custom"].contains(&mode.as_str()) { mode } else { "aztek".into() },
        folder: text("folder"),
        version: text("version"),
        user_setup: v["userSetup"].as_bool().unwrap_or(true),
    }
}

pub fn save_choice(c: &Choice) -> std::io::Result<()> {
    std::fs::create_dir_all(prefs_base())?;
    std::fs::write(choice_file(), json!({"mode": c.mode, "folder": c.folder, "version": c.version, "userSetup": c.user_setup}).to_string())
}

/// The folder a choice points at.
pub fn root_of(c: &Choice) -> PathBuf {
    match c.mode.as_str() {
        "link" | "custom" if !c.folder.is_empty() => PathBuf::from(&c.folder),
        _ => own_folder(),
    }
}

/// The folders a choice puts on each path. A folder goes on whether or not it exists yet, so a script dropped in
/// later is found; the lists hold only the layout, not what a module adds.
fn layout(c: &Choice) -> Applied {
    let root = root_of(c);
    let version = if c.version.is_empty() { versions(&root).into_iter().next().unwrap_or_default() } else { c.version.clone() };
    let mut a = Applied { root: root.clone(), version: version.clone(), ..Default::default() };
    let bases: Vec<PathBuf> = if version.is_empty() { vec![root.clone()] } else { vec![root.join(&version), root.clone()] };
    for b in &bases {
        a.scripts.push(b.join("scripts"));
        a.plugins.push(b.join("plug-ins"));
        a.modules.push(b.join("modules"));
    }
    let prefs = bases[0].join("prefs");
    a.scripts.push(prefs.join("scripts"));
    a.prefs = Some(prefs);
    a
}

fn path_list(var: &str) -> Vec<PathBuf> {
    std::env::var_os(var).map(|v| std::env::split_paths(&v).filter(|p| !p.as_os_str().is_empty()).collect()).unwrap_or_default()
}

fn set_path_list(var: &str, list: &[PathBuf]) {
    match std::env::join_paths(list) {
        Ok(v) if !list.is_empty() => std::env::set_var(var, v),
        _ => std::env::remove_var(var),
    }
}

fn add_to(a: &mut Applied, var: &str, dir: &Path) {
    let mut list = path_list(var);
    if list.iter().any(|p| p == dir) {
        return;
    }
    list.push(dir.to_path_buf());
    set_path_list(var, &list);
    a.added.push((var.to_string(), dir.to_path_buf()));
}

/// `$VAR`, `${VAR}` and `%VAR%` filled in from the environment; `~` at the start is the home folder.
fn expand(text: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = text.chars().collect();
    let mut i = 0;
    let var = |name: &str| std::env::var(name).unwrap_or_default();
    while i < chars.len() {
        match chars[i] {
            '~' if i == 0 && (chars.len() == 1 || chars[1] == '/' || chars[1] == '\\') => out.push_str(&home().to_string_lossy()),
            '$' if chars.get(i + 1) == Some(&'{') => {
                let end = chars[i..].iter().position(|&c| c == '}').map(|e| i + e).unwrap_or(chars.len());
                out.push_str(&var(&chars[i + 2..end].iter().collect::<String>()));
                i = end;
            }
            '$' => {
                let mut j = i + 1;
                while j < chars.len() && (chars[j].is_ascii_alphanumeric() || chars[j] == '_') {
                    j += 1;
                }
                if j == i + 1 {
                    out.push('$');
                } else {
                    out.push_str(&var(&chars[i + 1..j].iter().collect::<String>()));
                    i = j - 1;
                }
            }
            '%' => match chars[i + 1..].iter().position(|&c| c == '%') {
                Some(e) if e > 0 => {
                    out.push_str(&var(&chars[i + 1..i + 1 + e].iter().collect::<String>()));
                    i += e + 1;
                }
                _ => out.push('%'),
            },
            c => out.push(c),
        }
        i += 1;
    }
    out
}

const PATH_VARS: &[&str] = &["MAYA_SCRIPT_PATH", "MAYA_PLUG_IN_PATH", "MAYA_MODULE_PATH", "AZTEK_SCRIPT_PATH", "AZTEK_PLUG_IN_PATH", "AZTEK_MODULE_PATH", "PYTHONPATH", "XBMLANGPATH", "PATH", "MAYA_SHELF_PATH", "MAYA_PRESET_PATH"];

/// The folder's environment file: `NAME = value` lines, `#` and `//` comments. A variable the process already has
/// is left as it is, except the path lists, which the file's folders are added to.
fn read_env_file(a: &mut Applied, file: &Path) {
    let Ok(text) = std::fs::read_to_string(file) else { return };
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        let Some((name, value)) = line.split_once('=') else { continue };
        let (name, value) = (name.trim(), expand(value.trim().trim_matches('"')));
        if name.is_empty() || !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
            continue;
        }
        if PATH_VARS.contains(&name) {
            for p in std::env::split_paths(&value).filter(|p| !p.as_os_str().is_empty()) {
                add_to(a, name, &p);
            }
        } else if std::env::var_os(name).is_none() {
            std::env::set_var(name, value);
        }
    }
}

fn platform_matches(cond: &str) -> bool {
    let want = cond.to_ascii_lowercase();
    if cfg!(windows) {
        want.starts_with("win")
    } else if cfg!(target_os = "macos") {
        want.starts_with("mac") || want.starts_with("osx") || want.starts_with("darwin")
    } else {
        want.starts_with("linux")
    }
}

/// Reads one `.mod` file: each `+ [conditions] name version path` line starts a module, and the lines under it add
/// to the environment (`VAR = value`, `VAR := folder in the module`, `VAR += value`, `VAR +:= folder`) or move one of
/// its folders (`scripts: folder`, `plug-ins: folder`). A module's `scripts` and `plug-ins` folders go on the paths.
/// A block for another platform is skipped; of several blocks for versions of the reference, the one for the chosen
/// version is taken, or the first when none is chosen or none matches.
fn read_mod_file(a: &mut Applied, file: &Path, version: &str) {
    let Ok(text) = std::fs::read_to_string(file) else { return };
    let dir = file.parent().unwrap_or(Path::new(".")).to_path_buf();
    struct Block {
        name: String,
        version: String,
        path: PathBuf,
        for_version: Option<String>,
        lines: Vec<String>,
    }
    let mut blocks: Vec<Block> = Vec::new();
    let mut skipping = false;
    for raw in text.lines() {
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
            continue;
        }
        if line.starts_with('+') || line.starts_with('-') {
            skipping = line.starts_with('-');
            if skipping {
                continue;
            }
            let mut parts: Vec<&str> = line[1..].split_whitespace().collect();
            let mut for_version = None;
            let mut wrong_platform = false;
            while let Some(first) = parts.first() {
                let Some((key, value)) = first.split_once(':') else { break };
                match key.to_ascii_uppercase().as_str() {
                    "MAYAVERSION" => for_version = Some(value.to_string()),
                    "PLATFORM" => wrong_platform |= !platform_matches(value),
                    "LOCALE" => {}
                    _ => break,
                }
                parts.remove(0);
            }
            if wrong_platform || parts.len() < 3 {
                skipping = true;
                continue;
            }
            let given = expand(&parts[2..].join(" "));
            let path = if Path::new(&given).is_absolute() { PathBuf::from(given) } else { dir.join(given) };
            blocks.push(Block { name: parts[0].to_string(), version: parts[1].to_string(), path, for_version, lines: Vec::new() });
        } else if !skipping {
            if let Some(b) = blocks.last_mut() {
                b.lines.push(line.to_string());
            }
        }
    }
    let mut names: Vec<String> = Vec::new();
    for b in &blocks {
        if !names.contains(&b.name) {
            names.push(b.name.clone());
        }
    }
    for name in names {
        if a.found.iter().any(|m| m.name == name) {
            continue;
        }
        let of: Vec<&Block> = blocks.iter().filter(|b| b.name == name).collect();
        let pick = of.iter().find(|b| b.for_version.as_deref() == Some(version) && !version.is_empty()).or_else(|| of.iter().find(|b| b.for_version.is_none())).or(of.first());
        let Some(b) = pick else { continue };
        let root = aztek_core::paths::normalize(&b.path);
        let (mut scripts, mut plugins) = (root.join("scripts"), root.join("plug-ins"));
        for line in &b.lines {
            if let Some((key, value)) = line.split_once(':').filter(|(k, v)| !k.contains('=') && !k.contains('+') && !v.starts_with('=')) {
                let key = key.trim().trim_start_matches("[r]").trim();
                let dir = root.join(expand(value.trim()));
                match key {
                    "scripts" => scripts = dir,
                    "plug-ins" => plugins = dir,
                    _ => {}
                }
                continue;
            }
            let (name_part, value, relative, append) = if let Some((n, v)) = line.split_once("+:=") {
                (n, v, true, true)
            } else if let Some((n, v)) = line.split_once(":=") {
                (n, v, true, false)
            } else if let Some((n, v)) = line.split_once("+=") {
                (n, v, false, true)
            } else if let Some((n, v)) = line.split_once('=') {
                (n, v, false, false)
            } else {
                continue;
            };
            let var = name_part.trim();
            if var.is_empty() || !var.chars().all(|c| c.is_ascii_alphanumeric() || c == '_') {
                continue;
            }
            let value = expand(value.trim());
            let value = if relative { root.join(&value).to_string_lossy().into_owned() } else { value };
            if append || PATH_VARS.contains(&var) {
                for p in std::env::split_paths(&value).filter(|p| !p.as_os_str().is_empty()) {
                    add_to(a, var, &p);
                }
            } else {
                std::env::set_var(var, value);
            }
        }
        if scripts.is_dir() {
            add_to(a, "AZTEK_SCRIPT_PATH", &scripts);
            add_to(a, "PYTHONPATH", &scripts);
        }
        if plugins.is_dir() {
            add_to(a, "AZTEK_PLUG_IN_PATH", &plugins);
        }
        let icons = root.join("icons");
        if icons.is_dir() {
            add_to(a, "XBMLANGPATH", &icons);
        }
        a.found.push(Module { name, version: b.version.clone(), path: root, definition: file.to_path_buf() });
    }
}

/// Takes back what the last choice put on the paths.
fn withdraw() {
    let Some(old) = APPLIED.lock().unwrap().take() else { return };
    for (var, dir) in old.added {
        let list: Vec<PathBuf> = path_list(&var).into_iter().filter(|p| *p != dir).collect();
        set_path_list(&var, &list);
    }
}

/// Puts a choice in force: its folders on the paths, its environment file read, its modules found.
pub fn apply(c: &Choice) -> Applied {
    withdraw();
    let mut a = layout(c);
    let bases: Vec<PathBuf> = if a.version.is_empty() { vec![a.root.clone()] } else { vec![a.root.join(&a.version), a.root.clone()] };
    for b in &bases {
        for name in ["Aztek.env", "Maya.env"] {
            read_env_file(&mut a, &b.join(name));
        }
    }
    for d in a.scripts.clone() {
        add_to(&mut a, "AZTEK_SCRIPT_PATH", &d);
        add_to(&mut a, "PYTHONPATH", &d);
    }
    for d in a.plugins.clone() {
        add_to(&mut a, "AZTEK_PLUG_IN_PATH", &d);
    }
    for d in a.modules.clone() {
        add_to(&mut a, "AZTEK_MODULE_PATH", &d);
    }
    let mut module_dirs = path_list("AZTEK_MODULE_PATH");
    module_dirs.extend(path_list("MAYA_MODULE_PATH"));
    let version = a.version.clone();
    for d in module_dirs {
        let Ok(rd) = std::fs::read_dir(&d) else { continue };
        let mut files: Vec<PathBuf> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "mod")).collect();
        files.sort();
        for f in files {
            read_mod_file(&mut a, &f, &version);
        }
    }
    *APPLIED.lock().unwrap() = Some(a.clone());
    a
}

/// What is in force, applying the saved choice if nothing has been yet.
pub fn applied() -> Applied {
    if let Some(a) = APPLIED.lock().unwrap().clone() {
        return a;
    }
    apply(&load_choice())
}

/// The modules found.
pub fn modules() -> Vec<Module> {
    applied().found
}

/// The optionVars a folder's `prefs/userPrefs.mel` sets: its `optionVar` statements read as data, not run.
pub fn saved_option_vars(prefs: &Path) -> BTreeMap<String, Value> {
    let mut out = BTreeMap::new();
    let Ok(text) = std::fs::read_to_string(prefs.join("userPrefs.mel")) else { return out };
    // Words and quoted strings; a statement ends at a semicolon outside quotes.
    let mut tokens: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut chars = text.chars().peekable();
    let flush = |cur: &mut String, tokens: &mut Vec<String>| {
        if !cur.is_empty() {
            tokens.push(std::mem::take(cur));
        }
    };
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                flush(&mut cur, &mut tokens);
                let mut s = String::from("\"");
                while let Some(d) = chars.next() {
                    match d {
                        '\\' => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(e) => s.push(e),
                            None => {}
                        },
                        '"' => break,
                        d => s.push(d),
                    }
                }
                tokens.push(s);
            }
            '/' if chars.peek() == Some(&'/') => {
                flush(&mut cur, &mut tokens);
                for d in chars.by_ref() {
                    if d == '\n' {
                        break;
                    }
                }
            }
            ';' => {
                flush(&mut cur, &mut tokens);
                tokens.push(";".into());
            }
            c if c.is_whitespace() => flush(&mut cur, &mut tokens),
            c => cur.push(c),
        }
    }
    flush(&mut cur, &mut tokens);
    let text_of = |t: &str| t.strip_prefix('"').unwrap_or(t).to_string();
    let mut i = 0;
    while i < tokens.len() {
        if tokens[i] != "optionVar" {
            i += 1;
            continue;
        }
        i += 1;
        while i < tokens.len() && tokens[i] != ";" {
            let flag = tokens[i].clone();
            let (name, value) = (tokens.get(i + 1).map(|t| text_of(t)), tokens.get(i + 2).cloned());
            match (flag.as_str(), name, value) {
                ("-iv" | "-intValue", Some(n), Some(v)) => {
                    out.insert(n, json!(v.parse::<i64>().unwrap_or(0)));
                    i += 3;
                }
                ("-fv" | "-floatValue", Some(n), Some(v)) => {
                    out.insert(n, json!(v.parse::<f64>().unwrap_or(0.0)));
                    i += 3;
                }
                ("-sv" | "-stringValue", Some(n), Some(v)) => {
                    out.insert(n, json!(text_of(&v)));
                    i += 3;
                }
                ("-iva" | "-intValueAppend", Some(n), Some(v)) => {
                    append(&mut out, n, json!(v.parse::<i64>().unwrap_or(0)));
                    i += 3;
                }
                ("-fva" | "-floatValueAppend", Some(n), Some(v)) => {
                    append(&mut out, n, json!(v.parse::<f64>().unwrap_or(0.0)));
                    i += 3;
                }
                ("-sva" | "-stringValueAppend", Some(n), Some(v)) => {
                    append(&mut out, n, json!(text_of(&v)));
                    i += 3;
                }
                ("-ca" | "-clearArray", Some(n), _) => {
                    out.insert(n, json!([]));
                    i += 2;
                }
                _ => i += 1,
            }
        }
    }
    out
}

fn append(out: &mut BTreeMap<String, Value>, name: String, v: Value) {
    match out.get_mut(&name) {
        Some(Value::Array(a)) => a.push(v),
        _ => {
            out.insert(name, json!([v]));
        }
    }
}

/// What copying a folder did.
#[derive(Default, Debug)]
pub struct Copied {
    pub files: usize,
    pub kept: usize,
    pub failed: Vec<String>,
}

fn copy_tree(from: &Path, to: &Path, r: &mut Copied) {
    let Ok(rd) = std::fs::read_dir(from) else { return };
    if std::fs::create_dir_all(to).is_err() {
        r.failed.push(to.to_string_lossy().into_owned());
        return;
    }
    for e in rd.flatten() {
        let (src, dst) = (e.path(), to.join(e.file_name()));
        // Followed, so a linked folder is copied as what it holds.
        if src.is_dir() {
            copy_tree(&src, &dst, r);
        } else if dst.exists() {
            r.kept += 1;
        } else if std::fs::copy(&src, &dst).is_ok() {
            r.files += 1;
        } else {
            r.failed.push(src.to_string_lossy().into_owned());
        }
    }
}

/// Copies an existing folder into Aztek's own: the chosen version's and the shared `scripts`, `plug-ins` and
/// `modules` into one of each, the version's `prefs`, and its environment file as `Aztek.env`. Nothing already in
/// Aztek's folder is replaced, and the folder copied from is only read.
pub fn copy_into_own(from: &Path, version: &str) -> Copied {
    let mut r = Copied::default();
    let to = own_folder();
    let version = if version.is_empty() { versions(from).into_iter().next().unwrap_or_default() } else { version.to_string() };
    let bases: Vec<PathBuf> = if version.is_empty() { vec![from.to_path_buf()] } else { vec![from.join(&version), from.to_path_buf()] };
    for b in &bases {
        for sub in ["scripts", "plug-ins", "modules"] {
            copy_tree(&b.join(sub), &to.join(sub), &mut r);
        }
    }
    copy_tree(&bases[0].join("prefs"), &to.join("prefs"), &mut r);
    let env = bases[0].join("Maya.env");
    if env.is_file() && !to.join("Aztek.env").exists() {
        match std::fs::create_dir_all(&to).and_then(|_| std::fs::copy(&env, to.join("Aztek.env"))) {
            Ok(_) => r.files += 1,
            Err(_) => r.failed.push(env.to_string_lossy().into_owned()),
        }
    }
    r
}

fn slashed(p: &Path) -> String {
    let mut s = p.to_string_lossy().replace('\\', "/");
    if !s.ends_with('/') {
        s.push('/');
    }
    s
}

fn describe(c: &Choice, a: &Applied) -> Value {
    let list = |v: &[PathBuf]| v.iter().map(|p| json!({"path": p.to_string_lossy(), "exists": p.is_dir()})).collect::<Vec<_>>();
    let detected = detect_reference_folder();
    let shelves = a.prefs.as_ref().map(|p| p.join("shelves")).filter(|p| p.is_dir());
    let shelf_files: Vec<String> = shelves
        .as_ref()
        .and_then(|d| std::fs::read_dir(d).ok())
        .map(|rd| {
            let mut v: Vec<String> = rd.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|x| x == "mel") && p.file_name().is_some_and(|n| n.to_string_lossy().starts_with("shelf_"))).map(|p| p.to_string_lossy().into_owned()).collect();
            v.sort();
            v
        })
        .unwrap_or_default();
    json!({
        "mode": c.mode,
        "folder": c.folder,
        "version": c.version,
        "userSetup": c.user_setup,
        "ownFolder": own_folder().to_string_lossy(),
        // Whether Aztek's own folder is there yet, and whether a choice has been made: the first start asks.
        "ownExists": own_folder().is_dir(),
        "chosen": choice_file().is_file(),
        "detected": detected.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "detectedVersions": detected.as_ref().map(|p| versions(p)).unwrap_or_default(),
        "root": a.root.to_string_lossy(),
        "usedVersion": a.version,
        "versions": versions(&a.root),
        "scripts": list(&a.scripts),
        "plugins": list(&a.plugins),
        "modules": list(&a.modules),
        "prefs": a.prefs.as_ref().map(|p| p.to_string_lossy().into_owned()),
        "shelves": shelf_files,
        "found": a.found.iter().map(|m| json!({"name": m.name, "version": m.version, "path": m.path.to_string_lossy(), "definition": m.definition.to_string_lossy()})).collect::<Vec<_>>(),
    })
}

fn user_folder_cmd(_s: &mut Session, a: &Args) -> CmdResult {
    if let Some(from) = a.get("copyFrom").and_then(Value::as_str) {
        let from = PathBuf::from(expand(from));
        if !from.is_dir() {
            return Err(CommandError::new(format!("aztekUserFolder: {} is not a folder", from.display())));
        }
        if aztek_core::paths::normalize(&from) == aztek_core::paths::normalize(&own_folder()) {
            return Err(CommandError::new("aztekUserFolder: that is Aztek's own folder"));
        }
        let r = copy_into_own(&from, &a.string("version", ""));
        let mut c = load_choice();
        c.mode = "aztek".into();
        c.folder.clear();
        c.version.clear();
        save_choice(&c).map_err(|e| CommandError::new(format!("aztekUserFolder: {e}")))?;
        apply(&c);
        return Ok(json!({"copied": r.files, "kept": r.kept, "failed": r.failed, "to": own_folder().to_string_lossy()}));
    }
    if let Some(folder) = a.get("listVersions").and_then(Value::as_str) {
        return Ok(json!(versions(Path::new(&expand(folder)))));
    }
    let mut c = load_choice();
    if a.bool("query") || (a.get("mode").is_none() && a.get("folder").is_none() && a.get("version").is_none() && a.get("userSetup").is_none()) {
        return Ok(describe(&c, &applied()));
    }
    if let Some(m) = a.get("mode").and_then(Value::as_str) {
        if !["aztek", "link", "custom"].contains(&m) {
            return Err(CommandError::new("aztekUserFolder: -mode is aztek, link or custom"));
        }
        c.mode = m.to_string();
    }
    if let Some(f) = a.get("folder").and_then(Value::as_str) {
        c.folder = expand(f);
    }
    if let Some(v) = a.get("version").and_then(Value::as_str) {
        c.version = v.to_string();
    }
    if let Some(v) = a.get("userSetup") {
        c.user_setup = v.as_bool().unwrap_or_else(|| v.as_f64().is_some_and(|x| x != 0.0));
    }
    if c.mode != "aztek" && !Path::new(&c.folder).is_dir() {
        return Err(CommandError::new(format!("aztekUserFolder: {} is not a folder", if c.folder.is_empty() { "(none given)" } else { &c.folder })));
    }
    if c.mode == "aztek" {
        c.folder.clear();
        c.version.clear();
    }
    save_choice(&c).map_err(|e| CommandError::new(format!("aztekUserFolder: {e}")))?;
    let applied = apply(&c);
    Ok(describe(&c, &applied))
}

fn internal_var(_s: &mut Session, a: &Args) -> CmdResult {
    let ap = applied();
    let base = if ap.version.is_empty() { ap.root.clone() } else { ap.root.join(&ap.version) };
    let prefs = ap.prefs.clone().unwrap_or_else(|| base.join("prefs"));
    let on = |k: &str| a.bool(k);
    let dir = if on("userAppDir") {
        ap.root.clone()
    } else if on("userScriptDir") {
        base.join("scripts")
    } else if on("userPrefDir") {
        prefs
    } else if on("userShelfDir") {
        prefs.join("shelves")
    } else if on("userMarkingMenuDir") {
        prefs.join("markingMenus")
    } else if on("userBitmapsDir") {
        prefs.join("icons")
    } else if on("userHotkeyDir") {
        prefs.join("hotkeys")
    } else if on("userPresetsDir") {
        base.join("presets")
    } else if on("userWorkspaceDir") {
        ap.root.join("projects")
    } else if on("userTmpDir") {
        std::env::temp_dir()
    } else if on("mayaInstallDir") {
        aztek_core::paths::executable_dir()
    } else {
        return Err(CommandError::new("internalVar: ask for one of -userAppDir, -userScriptDir, -userPrefDir, -userShelfDir, -userMarkingMenuDir, -userBitmapsDir, -userHotkeyDir, -userPresetsDir, -userWorkspaceDir, -userTmpDir, -mayaInstallDir"));
    };
    Ok(json!(slashed(&dir)))
}

fn module_info(_s: &mut Session, a: &Args) -> CmdResult {
    let mods = modules();
    if a.bool("listModules") {
        return Ok(json!(mods.iter().map(|m| m.name.clone()).collect::<Vec<_>>()));
    }
    let name = a.string("moduleName", "");
    let Some(m) = mods.iter().find(|m| m.name == name) else { return Err(CommandError::new(format!("moduleInfo: no module named {name}"))) };
    Ok(if a.bool("version") {
        json!(m.version)
    } else if a.bool("definition") {
        json!(m.definition.to_string_lossy())
    } else {
        json!(m.path.to_string_lossy())
    })
}

/// Where a module lives, for getModulePath.
pub fn module_path(name: &str) -> Option<String> {
    modules().into_iter().find(|m| m.name == name).map(|m| m.path.to_string_lossy().into_owned())
}

pub fn register_commands(s: &mut Session) {
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    let t = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::String, Value::Null, doc);
    s.register(
        CommandDef::<Session>::new(
            "aztekUserFolder",
            "Where scripts, plug-ins, modules and preferences kept between versions are looked for: -mode aztek (Aztek's own folder), link (an existing folder of the reference's, used in place and never written to) or custom, with -folder and -version; -copyFrom folder copies one into Aztek's own. -q describes what is in force.",
            user_folder_cmd,
        )
        .flag(b("query", "q", "Describe the choice, the folders on each path and the modules found."))
        .flag(t("mode", "m", "aztek, link or custom."))
        .flag(t("folder", "f", "The folder for link and custom."))
        .flag(t("version", "v", "The version's subfolder to use; empty takes the newest."))
        .flag(b("userSetup", "us", "Whether userSetup scripts run at start."))
        .flag(t("copyFrom", "cf", "Copy this folder into Aztek's own and use that."))
        .flag(t("listVersions", "lv", "The versions a folder holds, newest first."))
        .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("internalVar", "The user folder's places, each ending in a slash: -userAppDir, -userScriptDir, -userPrefDir, -userShelfDir and the rest.", internal_var)
            .flag(b("userAppDir", "uad", "The user folder."))
            .flag(b("userScriptDir", "usd", "Its scripts."))
            .flag(b("userPrefDir", "upd", "Its preferences."))
            .flag(b("userShelfDir", "ush", "Its shelves."))
            .flag(b("userMarkingMenuDir", "umm", "Its marking menus."))
            .flag(b("userBitmapsDir", "ubd", "Its icons."))
            .flag(b("userHotkeyDir", "uhk", "Its hotkeys."))
            .flag(b("userPresetsDir", "ups", "Its presets."))
            .flag(b("userWorkspaceDir", "uwd", "Its projects."))
            .flag(b("userTmpDir", "utd", "The temporary folder."))
            .flag(b("mayaInstallDir", "mid", "Where the application is installed."))
            .undoable(false),
    );
    s.register(
        CommandDef::<Session>::new("moduleInfo", "Ask about the modules found: -listModules, or -moduleName with -path, -version or -definition.", module_info)
            .flag(b("listModules", "lm", "Their names."))
            .flag(t("moduleName", "mn", "The module asked about."))
            .flag(b("path", "p", "Where it is."))
            .flag(b("version", "v", "Its version."))
            .flag(b("definition", "d", "Its .mod file."))
            .undoable(false),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_variables() {
        std::env::set_var("AZ_UF_TEST", "there");
        assert_eq!(expand("$AZ_UF_TEST/a"), "there/a");
        assert_eq!(expand("${AZ_UF_TEST}b"), "thereb");
        assert_eq!(expand("%AZ_UF_TEST%\\c"), "there\\c");
        assert_eq!(expand("100% $"), "100% $");
    }
}
