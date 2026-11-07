//! `file` with USD: the scene crate's file command hands USD types and extensions to `aztekUsdFile`, which turns the
//! translator's `-options` string into the import or export command's flags.

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::session::Session;
use serde_json::{json, Map, Value as J};

/// "a=1;b=two;c=[x,y]" as flags. Numbers stay numbers; 0 and 1 stand for off and on where the flag is a switch, which the
/// commands read either way.
pub fn parse_options(text: &str) -> Map<String, J> {
    let mut out = Map::new();
    for part in text.split(';') {
        let Some((k, v)) = part.split_once('=') else { continue };
        let (k, v) = (k.trim(), v.trim());
        if k.is_empty() || v.is_empty() {
            continue;
        }
        let val = if let Some(inner) = v.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
            J::Array(inner.split(',').map(|x| json!(x.trim())).filter(|x| x != &json!("")).collect())
        } else if let Ok(i) = v.parse::<i64>() {
            json!(i)
        } else if let Ok(f) = v.parse::<f64>() {
            json!(f)
        } else {
            json!(v)
        };
        out.insert(k.to_string(), val);
    }
    out
}

/// The flags a command knows, from what the options gave. The translator's UI writes names the command doesn't take.
fn keep_known(s: &Session, cmd: &str, mut flags: Map<String, J>) -> Map<String, J> {
    let known: Vec<String> = s.commands.find(cmd).map(|d| d.flags.iter().map(|f| f.name.clone()).collect()).unwrap_or_default();
    flags.retain(|k, _| known.iter().any(|n| n == k));
    flags
}

fn run_file(s: &mut Session, a: &Args) -> CmdResult {
    let path = a.string("file", "");
    if path.is_empty() {
        fail!("file: USD needs a path");
    }
    let mut opts = parse_options(&a.string("options", ""));
    if a.bool("import") || a.bool("open") {
        if a.bool("open") {
            s.run("file", json!({"new": true, "force": true}))?;
        }
        let mut flags = keep_known(s, "mayaUSDImport", std::mem::take(&mut opts));
        flags.insert("file".into(), json!(path));
        s.run("mayaUSDImport", J::Object(flags))?;
        return Ok(json!(path));
    }
    // The export UI's animation switch and range.
    let animated = opts.get("animation").and_then(J::as_i64) == Some(1);
    let range = (opts.get("startTime").and_then(J::as_f64), opts.get("endTime").and_then(J::as_f64));
    let mut flags = keep_known(s, "mayaUSDExport", opts);
    if animated {
        if let (Some(lo), Some(hi)) = range {
            flags.insert("frameRange".into(), json!([lo, hi]));
        }
    }
    flags.insert("file".into(), json!(path));
    if a.bool("exportSelected") {
        flags.insert("selection".into(), json!(true));
    }
    s.run("mayaUSDExport", J::Object(flags))?;
    Ok(json!(path))
}

pub fn register(c: &mut Commands<Session>) {
    let f = |n: &str, ty: ArgType| FlagDef::new(n, "", ty, J::Null, "See file.");
    c.register(
        CommandDef::new("aztekUsdFile", "What file runs for a USD type or extension.", run_file)
            .undoable(false)
            .flag(f("file", ArgType::String))
            .flag(f("options", ArgType::String))
            .flag(f("import", ArgType::Bool))
            .flag(f("open", ArgType::Bool))
            .flag(f("exportAll", ArgType::Bool))
            .flag(f("exportSelected", ArgType::Bool)),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options() {
        let o = parse_options("exportUVs=1;shadingMode=useRegistry;convertMaterialsTo=[UsdPreviewSurface];frameStride=0.5;empty=;");
        assert_eq!(o["exportUVs"], json!(1));
        assert_eq!(o["shadingMode"], json!("useRegistry"));
        assert_eq!(o["convertMaterialsTo"], json!(["UsdPreviewSurface"]));
        assert_eq!(o["frameStride"], json!(0.5));
        assert!(!o.contains_key("empty"));
    }
}
