//! Environment variables: what start-up sets, the environment file, and the variables that move where shelves,
//! icons, presets and projects are found. One test: the environment is the whole process's.

use aztek_host::Core;
use serde_json::{json, Value};
use std::path::Path;

fn write(p: &Path, text: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

#[test]
fn the_reference_s_variables_are_read() {
    let base = std::env::temp_dir().join(format!("aztek_environment_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (own, theirs, prefs) = (base.join("aztek"), base.join("theirs"), base.join("prefs"));
    std::env::set_var("HOME", &base);
    std::env::set_var("AZTEK_PREFS", &prefs);
    std::env::set_var("AZTEK_CONFIG_DIR", base.join("config"));
    std::env::set_var("AZTEK_APP_DIR", &own);
    std::env::set_var("MAYA_APP_DIR", &theirs);
    for v in ["AZTEK_ENV_DIR", "MAYA_ENV_DIR", "AZTEK_SCRIPT_PATH", "AZTEK_PLUG_IN_PATH", "AZTEK_MODULE_PATH", "MAYA_SCRIPT_PATH", "MAYA_PLUG_IN_PATH", "MAYA_MODULE_PATH", "MAYA_SHELF_PATH", "AZTEK_SHELF_PATH", "XBMLANGPATH", "MAYA_PROJECT", "AZTEK_PROJECT", "MAYA_PROJECTS_DIR", "MAYA_PRESET_PATH", "MAYA_SKIP_USERSETUP_PY", "MAYA_LOCATION", "AZTEK_LOCATION"] {
        std::env::remove_var(v);
    }
    std::fs::create_dir_all(theirs.join("2025/prefs")).unwrap();
    write(&own.join("scripts/userSetup.py"), b"pass\n");
    write(
        &own.join("Aztek.env"),
        format!("# a comment\nAZ_ENV_FROM_FILE = fromFile\nAZ_ENV_ALREADY = fromFile\nMAYA_APP_DIR = /nowhere\nMAYA_SHELF_PATH = $HOME/shelves\nXBMLANGPATH = {}/%B\n", slash(&base.join("icons"))).as_bytes(),
    );
    write(&base.join("elsewhere/Aztek.env"), b"AZ_ENV_ELSEWHERE = yes\n");
    write(&base.join("shelves/shelf_Studio.mel"), b"global proc shelf_Studio () {\n    shelfButton -label \"Hello\" -image1 \"studio.png\" -command \"print 1\" -sourceType \"mel\";\n}\n");
    write(&base.join("shelves/notes.txt"), b"not a shelf");
    write(&base.join("icons/studio.png"), b"png");
    std::env::set_var("AZ_ENV_ALREADY", "fromSystem");

    let mut c = Core::new();

    // Where the application is: set at start under both names, whatever MAYA_LOCATION was.
    let here = c.mel("getenv \"AZTEK_LOCATION\";");
    assert!(here.as_str().is_some_and(|h| !h.is_empty()), "{here}");
    assert_eq!(c.mel("getenv \"MAYA_LOCATION\";"), here);

    // The environment file: read from the user folder, a variable the system set is left, the user folder cannot be
    // moved from it, and inside the session MAYA_APP_DIR is the user folder in use.
    let q = c.mel("aztekUserFolder -mode \"aztek\";");
    assert_eq!(q["envFile"], json!(own.join("Aztek.env").to_string_lossy()), "{q}");
    assert_eq!(q["warnings"].as_array().map(Vec::len), Some(1), "{q}");
    assert!(q["warnings"][0].as_str().unwrap().contains("MAYA_APP_DIR"), "{q}");
    assert_eq!(c.mel("getenv \"AZ_ENV_FROM_FILE\";"), json!("fromFile"));
    assert_eq!(c.mel("getenv \"AZ_ENV_ALREADY\";"), json!("fromSystem"));
    assert_eq!(c.mel("getenv \"MAYA_APP_DIR\";"), json!(own.to_string_lossy()));
    // The reference's folder is still found where the system said it was.
    assert_eq!(q["detected"], json!(theirs.to_string_lossy()), "{q}");

    // Shelves on the shelf path, icons on the icon path with the /%B spelling.
    assert_eq!(c.mel("aztekEnvironment -shelfFiles;"), json!([base.join("shelves/shelf_Studio.mel").to_string_lossy()]));
    assert_eq!(c.mel("aztekEnvironment -icon \"studio.png\";"), json!(base.join("icons/studio.png").to_string_lossy()));
    assert_eq!(c.mel("aztekEnvironment -icon \"missing.png\";"), json!(""));
    assert_eq!(c.mel("xbmLangPathList;"), json!([base.join("icons").to_string_lossy()]));

    // AZTEK_ENV_DIR: the one folder the file is read from.
    std::env::set_var("AZTEK_ENV_DIR", base.join("elsewhere"));
    let q = c.mel("aztekUserFolder -mode \"aztek\";");
    assert_eq!(q["envFile"], json!(base.join("elsewhere/Aztek.env").to_string_lossy()), "{q}");
    assert_eq!(c.mel("getenv \"AZ_ENV_ELSEWHERE\";"), json!("yes"));
    assert_eq!(c.mel("aztekEnvironment -shelfFiles;"), json!([]), "what the other file put on the paths is taken back");
    std::env::remove_var("AZTEK_ENV_DIR");

    // The project a session starts in; Aztek's own spelling wins.
    std::env::set_var("MAYA_PROJECT", base.join("projA"));
    assert_eq!(c.mel("workspace -q -rd;"), json!(format!("{}/", base.join("projA").to_string_lossy())));
    std::env::set_var("AZTEK_PROJECT", base.join("projB"));
    assert_eq!(c.mel("workspace -q -rd;"), json!(format!("{}/", base.join("projB").to_string_lossy())));
    assert_eq!(c.mel("aztekEnvironment -value \"MAYA_PROJECT\";"), json!(base.join("projB").to_string_lossy()));
    std::env::set_var("MAYA_PROJECTS_DIR", base.join("projects"));
    assert_eq!(c.mel("internalVar -userWorkspaceDir;"), json!(format!("{}/", slash(&base.join("projects")))));

    // Presets on the preset path, beside the ones saved in Aztek's own folder.
    std::env::set_var("MAYA_PRESET_PATH", base.join("shared"));
    c.mel("shadingNode -asShader lambert -name \"envLambert\"; setAttr envLambert.diffuse 0.25;");
    c.mel("nodePreset -save envLambert \"studioGrey\";");
    let saved = base.join("config/presets/attrPresets/lambert/studioGrey.json");
    let shared = base.join("shared/attrPresets/lambert/studioGrey.json");
    std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
    std::fs::rename(&saved, &shared).unwrap();
    c.mel("setAttr envLambert.diffuse 0.9;");
    assert_eq!(c.mel("nodePreset -list envLambert;"), json!(["studioGrey"]));
    assert_eq!(c.mel("nodePreset -exists envLambert \"studioGrey\";"), json!(true));
    c.mel("nodePreset -load envLambert \"studioGrey\";");
    assert_eq!(c.mel("getAttr envLambert.diffuse;"), json!(0.25));

    // The Script Editor's lines, written to a file as well.
    let log = base.join("out/commands.log");
    std::fs::create_dir_all(log.parent().unwrap()).unwrap();
    let id = c.mel(&format!("cmdFileOutput -o \"{}\";", slash(&log)));
    assert!(id.as_i64().is_some_and(|i| i > 0), "{id}");
    assert_eq!(c.mel(&format!("cmdFileOutput -q -s {id};")), json!(0));
    c.mel("warning \"written to the file\";");
    assert_eq!(c.mel(&format!("cmdFileOutput -c {id};")), json!(0));
    c.mel("warning \"after it was closed\";");
    let text = std::fs::read_to_string(&log).unwrap();
    assert!(text.contains("written to the file") && !text.contains("after it was closed"), "{text}");
    assert_eq!(c.mel(&format!("cmdFileOutput -q -s {id};")), json!(-3));

    // userSetup.py files are left alone when asked.
    assert_eq!(c.startup_scripts().len(), 1);
    std::env::set_var("MAYA_SKIP_USERSETUP_PY", "1");
    assert_eq!(c.startup_scripts().len(), 0);

    // Every variable is listed, with whether it is read.
    let all = c.mel("aztekEnvironment -q;");
    let find = |name: &str| all.as_array().unwrap().iter().find(|v| v["name"] == json!(name)).cloned().unwrap_or(Value::Null);
    assert_eq!(find("MAYA_SCRIPT_PATH")["read"], json!("yes"));
    assert_eq!(find("MAYA_SCRIPT_PATH")["ownName"], json!("AZTEK_SCRIPT_PATH"));
    assert_eq!(find("OCIO")["read"], json!("no"));
    assert_eq!(find("MAYA_SKIP_USERSETUP_PY")["value"], json!("1"));

    let _ = std::fs::remove_dir_all(&base);
}

#[test]
fn the_document_is_the_list() {
    let doc = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/ENVIRONMENT.md");
    assert_eq!(std::fs::read_to_string(doc).unwrap_or_default(), aztek_core::env::markdown(), "run: cargo run -p aztek_host --example gen_environment");
}
