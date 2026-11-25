//! Help > Install mGear: mGear built for Aztek, from the releases of https://github.com/srlegrand/mgear (one zip a
//! platform and plug-in ABI, made by its `aztek/package.py`), unpacked as a module into the user folder's `modules`
//! folder: `aztek-mGear.mod` and `mGear-aztek/` (scripts, icons, and the plug-ins built with Aztek's kit).
//!
//! `aztekMgear -release` finds the release for this Aztek, `-installFile zip` unpacks one, `-install` does both, and
//! `-query` says what is installed. AZTEK_MGEAR_RELEASES points the release list somewhere else, for tests.

use std::path::{Path, PathBuf};

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, FlagDef};
use aztek_scene::Session;
use serde_json::{json, Value};

const RELEASES: &str = "https://api.github.com/repos/srlegrand/mgear/releases";
/// The module's folder and definition, as the release's zip has them.
pub const MODULE_DIR: &str = "mGear-aztek";
pub const MOD_FILE: &str = "aztek-mGear.mod";

/// The platform part of a release asset's name.
pub fn platform() -> &'static str {
    if cfg!(windows) {
        "windows-x64"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "linux-x64"
    }
}

/// How a release's zip for this Aztek ends: its plug-in ABI and platform.
pub fn asset_suffix() -> String {
    format!("-aztek-abi{}-{}.zip", crate::native_plugin::ABI_VERSION, platform())
}

/// The newest release with a zip for this Aztek: {tag, version, asset: {name, url, size, digest}}, or {error}. Blocks.
pub fn release() -> Value {
    let url = std::env::var("AZTEK_MGEAR_RELEASES").unwrap_or_else(|_| RELEASES.to_string());
    let text = crate::update::get_json(&url);
    let v: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if let Some(e) = v.get("error").and_then(Value::as_str) {
        return json!({ "error": format!("Couldn't get mGear's releases: {e}") });
    }
    pick(&v)
}

/// Of GitHub's list of releases, the newest with a zip for this Aztek.
pub fn pick(v: &Value) -> Value {
    let suffix = asset_suffix();
    // GitHub lists the newest first.
    for rel in v.as_array().into_iter().flatten() {
        if rel["draft"].as_bool() == Some(true) {
            continue;
        }
        for a in rel["assets"].as_array().into_iter().flatten() {
            let name = a["name"].as_str().unwrap_or("");
            if let Some(rest) = name.strip_suffix(&suffix) {
                let version = rest.strip_prefix("mgear-").unwrap_or(rest);
                return json!({
                    "tag": rel["tag_name"],
                    "version": version,
                    "asset": {"name": name, "url": a["browser_download_url"], "size": a["size"], "digest": a["digest"]},
                });
            }
        }
    }
    json!({ "error": format!("There is no mGear release for this Aztek yet (plug-in ABI {}, {}).", crate::native_plugin::ABI_VERSION, platform()) })
}

/// Where mGear goes: the modules folder of the user folder chosen at the first start.
pub fn modules_dir() -> PathBuf {
    crate::user_folder::applied().root.join("modules")
}

/// What is installed: {version, path, definition}, or null.
pub fn installed() -> Value {
    let def = modules_dir().join(MOD_FILE);
    match crate::user_folder::modules().into_iter().find(|m| m.definition == def) {
        Some(m) => json!({"version": m.version, "path": m.path.to_string_lossy(), "definition": def.to_string_lossy()}),
        None => Value::Null,
    }
}

/// Unpacks a release's zip into the modules folder, over what an earlier install left, and puts it in force.
pub fn install(zip_path: &Path) -> Result<Value, String> {
    let file = std::fs::File::open(zip_path).map_err(|e| format!("Couldn't open {}: {e}", zip_path.display()))?;
    let mut z = zip::ZipArchive::new(file).map_err(|e| format!("{} isn't a zip: {e}", zip_path.display()))?;
    // Only the module's own two names: nothing of the zip lands anywhere else in the folder.
    let mut entries = Vec::new();
    let mut has_def = false;
    for i in 0..z.len() {
        let f = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(rel) = f.enclosed_name() else { return Err(format!("{} holds a path outside its folder: {}", zip_path.display(), f.name().unwrap_or_default())) };
        let first = rel.components().next().map(|c| c.as_os_str().to_string_lossy().into_owned()).unwrap_or_default();
        if first != MODULE_DIR && first != MOD_FILE {
            return Err(format!("{} isn't an mGear for Aztek release: it holds {}", zip_path.display(), f.name().unwrap_or_default()));
        }
        has_def |= first == MOD_FILE;
        entries.push((i, rel, f.is_dir()));
    }
    if !has_def {
        return Err(format!("{} has no {MOD_FILE}", zip_path.display()));
    }
    let dir = modules_dir();
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't make {}: {e}", dir.display()))?;
    let old = dir.join(MODULE_DIR);
    if old.exists() {
        std::fs::remove_dir_all(&old).map_err(|e| format!("Couldn't remove the mGear installed before ({}): {e}", old.display()))?;
    }
    for (i, rel, is_dir) in entries {
        let out = dir.join(&rel);
        if is_dir {
            std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
            continue;
        }
        if let Some(p) = out.parent() {
            std::fs::create_dir_all(p).map_err(|e| e.to_string())?;
        }
        let mut f = z.by_index(i).map_err(|e| e.to_string())?;
        let mut w = std::fs::File::create(&out).map_err(|e| format!("Couldn't write {}: {e}", out.display()))?;
        std::io::copy(&mut f, &mut w).map_err(|e| format!("Couldn't write {}: {e}", out.display()))?;
        #[cfg(unix)]
        if let Some(mode) = f.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o777));
        }
    }
    crate::user_folder::apply(&crate::user_folder::load_choice());
    let found = installed();
    if found.is_null() {
        return Err(format!("mGear was unpacked into {} but its module wasn't found there", dir.display()));
    }
    let scripts = Path::new(found["path"].as_str().unwrap_or("")).join("scripts");
    Ok(json!({"version": found["version"], "path": found["path"], "definition": found["definition"], "scripts": scripts.to_string_lossy()}))
}

fn mgear_cmd(_s: &mut Session, a: &Args) -> CmdResult {
    let err = |m: String| CommandError::new(format!("aztekMgear: {m}"));
    if a.bool("query") {
        return Ok(json!({"modules": modules_dir().to_string_lossy(), "installed": installed()}));
    }
    if a.bool("release") {
        return Ok(release());
    }
    if let Some(p) = a.get("installFile").and_then(Value::as_str) {
        return install(Path::new(p)).map_err(err);
    }
    if a.bool("install") {
        let rel = release();
        if let Some(e) = rel.get("error").and_then(Value::as_str) {
            return Err(err(e.to_string()));
        }
        let asset = &rel["asset"];
        let file = std::env::temp_dir().join("aztek-mgear").join(asset["name"].as_str().unwrap_or("mgear.zip"));
        if let Some(e) = crate::update::download(asset["url"].as_str().unwrap_or(""), &file, asset["digest"].as_str().unwrap_or("")) {
            return Err(err(e));
        }
        let r = install(&file).map_err(err);
        let _ = std::fs::remove_file(&file);
        return r;
    }
    Err(err("give -query, -release, -install or -installFile".into()))
}

pub fn register_commands(s: &mut Session) {
    let b = |name: &str, short: &str, doc: &str| FlagDef::new(name, short, ArgType::Bool, Value::Null, doc);
    s.register(
        CommandDef::<Session>::new(
            "aztekMgear",
            "mGear built for Aztek (github.com/srlegrand/mgear), installed as a module in the user folder's modules folder. \
             -release finds the release for this Aztek, -install downloads and installs it, -installFile installs a \
             release's zip, -query says what is installed. Aztek's own command.",
            mgear_cmd,
        )
        .flag(b("query", "q", "Where mGear goes and what is installed there: {modules, installed: {version, path, definition} or null}."))
        .flag(b("release", "rel", "The newest release for this Aztek: {tag, version, asset}, or {error}."))
        .flag(b("install", "i", "Download the newest release for this Aztek and install it."))
        .flag(FlagDef::new("installFile", "if", ArgType::String, Value::Null, "Install a release's zip from disk.")),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_newest_release_with_a_zip_for_this_aztek_is_picked() {
        let s = asset_suffix();
        let other = if platform() == "linux-x64" { "-aztek-abi3-windows-x64.zip" } else { "-aztek-abi3-linux-x64.zip" };
        let list = json!([
            {"tag_name": "v9.9.9-aztek-abi999", "draft": false, "assets": [{"name": "mgear-9.9.9-aztek-abi999-linux-x64.zip", "browser_download_url": "u0"}]},
            {"tag_name": "draft", "draft": true, "assets": [{"name": format!("mgear-5.5.0{s}"), "browser_download_url": "u1"}]},
            {"tag_name": "v5.4.8-aztek", "draft": false, "assets": [
                {"name": format!("mgear-5.4.8{other}"), "browser_download_url": "u2"},
                {"name": format!("mgear-5.4.8{s}"), "browser_download_url": "u3", "size": 10, "digest": "sha256:ab"}]},
            {"tag_name": "older", "draft": false, "assets": [{"name": format!("mgear-5.4.7{s}"), "browser_download_url": "u4"}]},
        ]);
        let r = pick(&list);
        assert_eq!(r["version"], json!("5.4.8"));
        assert_eq!(r["asset"]["url"], json!("u3"));
        assert_eq!(r["asset"]["digest"], json!("sha256:ab"));
        assert!(pick(&json!([])).get("error").is_some());
    }
}
