//! The user folder and the plug-in list: an existing folder of the reference's used in place, copied into Aztek's
//! own, its modules, saved optionVars and userSetup read, and a plug-in compiled for the reference told apart.

use aztek_host::Core;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

fn write(p: &Path, text: &[u8]) {
    std::fs::create_dir_all(p.parent().unwrap()).unwrap();
    std::fs::write(p, text).unwrap();
}

fn slash(p: &Path) -> String {
    p.to_string_lossy().replace('\\', "/")
}

/// Every file under a folder with its bytes, to show a folder was left as it was.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut todo = vec![dir.to_path_buf()];
    while let Some(d) = todo.pop() {
        for e in std::fs::read_dir(&d).unwrap().flatten() {
            let p = e.path();
            if p.is_dir() {
                todo.push(p);
            } else {
                out.push((p.clone(), std::fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

// The paths and the choice are the process's, so everything runs in one test, in turn.
#[test]
fn an_existing_folder_is_linked_copied_and_its_plug_ins_told_apart() {
    let base = std::env::temp_dir().join(format!("aztek_user_folder_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&base);
    let (theirs, own, prefs) = (base.join("maya"), base.join("aztek"), base.join("prefs"));
    std::env::set_var("HOME", &base);
    std::env::set_var("AZTEK_PREFS", &prefs);
    std::env::set_var("AZTEK_APP_DIR", &own);
    for v in ["MAYA_APP_DIR", "AZTEK_SCRIPT_PATH", "AZTEK_PLUG_IN_PATH", "AZTEK_MODULE_PATH", "MAYA_SCRIPT_PATH", "MAYA_PLUG_IN_PATH", "MAYA_MODULE_PATH"] {
        std::env::remove_var(v);
    }

    write(&theirs.join("2024/prefs/userPrefs.mel"), b"optionVar -iv \"old\" 1;\n");
    write(
        &theirs.join("2025/prefs/userPrefs.mel"),
        b"// saved\noptionVar\n -iv \"fromThere\" 7\n -fv \"aFloat\" 2.5\n -sv \"aString\" \"two words; and more\"\n -ca \"aList\" -sva \"aList\" \"a\" -sva \"aList\" \"b\"\n -iv \"mine\" 1;\n",
    );
    write(&theirs.join("2025/prefs/shelves/shelf_Mine.mel"), b"global proc shelf_Mine () {}\n");
    write(&theirs.join("2025/scripts/userSetup.mel"), b"global int $ufSetupRan; $ufSetupRan = 5;\n");
    write(&theirs.join("scripts/ufShared.mel"), b"global proc int ufShared() { return 42; }\n");
    write(&theirs.join("2025/Maya.env"), format!("UF_TEST_VAR = hello\nMAYA_SCRIPT_PATH = {}\n", slash(&base.join("extra"))).as_bytes());
    write(&base.join("extra/ufExtra.mel"), b"global proc int ufExtra() { return 9; }\n");
    write(&theirs.join("2025/plug-ins/theirTool.so"), b"\x7fELF....initializePlugin\0uninitializePlugin\0libOpenMaya.so\0");
    write(&theirs.join("2025/plug-ins/notes.txt"), b"not a plug-in");
    write(&theirs.join("plug-ins/ufPy.py"), b"def initializePlugin(obj):\n    pass\n\ndef uninitializePlugin(obj):\n    pass\n");
    write(
        &theirs.join("modules/tool.mod"),
        b"+ MAYAVERSION:2024 ufTool 1.0 ../tool2024\n+ MAYAVERSION:2025 ufTool 2.0 ../tool\nUF_TOOL_ROOT := data\n+ PLATFORM:nowhere ufOther 1.0 ../other\n",
    );
    write(&theirs.join("tool/scripts/ufToolProc.mel"), b"global proc int ufToolProc() { return 3; }\n");
    write(&theirs.join("tool/plug-ins/toolPlug.py"), b"def initializePlugin(obj):\n    pass\n\ndef uninitializePlugin(obj):\n    pass\n");
    let before = snapshot(&theirs);

    let mut c = Core::new();
    // Nothing chosen: Aztek's own folder.
    let q = c.mel("aztekUserFolder -q;");
    assert_eq!(q["mode"], json!("aztek"));
    assert_eq!(q["detected"], json!(theirs.to_string_lossy()));
    assert_eq!(q["detectedVersions"], json!(["2025", "2024"]));
    assert_eq!(c.mel("internalVar -userAppDir;"), json!(format!("{}/", slash(&own))));
    // What the first start asks by: no choice made yet, and whether Aztek's own folder is there.
    assert_eq!(q["chosen"], json!(false), "{q}");
    assert_eq!(q["ownExists"], json!(own.is_dir()));

    // Linked: the newest version is taken, its folders are on the paths, its module is found.
    let q = c.mel(&format!("aztekUserFolder -mode \"link\" -folder \"{}\";", slash(&theirs)));
    assert_eq!(q["usedVersion"], json!("2025"), "{q}");
    assert_eq!(q["chosen"], json!(true), "a choice made is not asked again");
    assert_eq!(q["found"][0]["name"], json!("ufTool"), "{q}");
    assert_eq!(q["found"].as_array().unwrap().len(), 1, "a module for another platform is left out: {q}");
    assert_eq!(q["shelves"].as_array().unwrap().len(), 1);
    assert_eq!(c.mel("moduleInfo -lm;"), json!(["ufTool"]));
    assert_eq!(c.mel("moduleInfo -mn \"ufTool\" -v;"), json!("2.0"));
    assert_eq!(std::env::var("UF_TEST_VAR").unwrap(), "hello");
    assert!(std::env::var("UF_TOOL_ROOT").unwrap().replace('\\', "/").ends_with("tool/data"));
    assert_eq!(c.mel("internalVar -userScriptDir;"), json!(format!("{}/2025/scripts/", slash(&theirs))));
    assert_eq!(c.mel("ufShared();"), json!(42));
    assert_eq!(c.mel("ufToolProc();"), json!(3));
    assert_eq!(c.mel("ufExtra();"), json!(9));

    // Its plug-ins are listed; the compiled one is known for what it is and is not loaded.
    let rows = c.mel("aztekPlugins;");
    let row = |name: &str| rows.as_array().unwrap().iter().find(|r| r["name"] == name).cloned().unwrap_or(Value::Null);
    assert_eq!(row("theirTool")["kind"], json!("foreign"), "{rows}");
    assert!(row("theirTool")["note"].as_str().unwrap().contains("compiled for Maya"));
    assert_eq!(row("ufPy")["kind"], json!("python"), "{rows}");
    assert_eq!(row("toolPlug")["kind"], json!("python"), "{rows}");
    assert_eq!(row("notes"), Value::Null);
    let err = c.mel_checked("loadPlugin \"theirTool\";").unwrap_err();
    assert!(err.contains("compiled for Maya") && err.contains("recompiled for Aztek"), "{err}");
    assert_eq!(c.mel("pluginInfo -q -loaded \"theirTool\";"), json!(false));

    // Auto load is kept by file.
    c.mel("pluginInfo -e -autoload true \"ufPy\";");
    assert_eq!(c.mel("pluginInfo -q -autoload \"ufPy\";"), json!(true));
    let kept: Value = serde_json::from_str(&std::fs::read_to_string(prefs.join("pluginPrefs.json")).unwrap()).unwrap();
    assert!(kept["autoload"][0].as_str().unwrap().ends_with("ufPy.py"), "{kept}");
    c.mel("pluginInfo -e -autoload false \"ufPy\";");
    assert_eq!(c.mel("pluginInfo -q -autoload \"ufPy\";"), json!(false));

    // At start: the folder's optionVars are defaults under Aztek's own, and userSetup runs.
    write(&prefs.join("optionVars.json"), br#"{"mine":3}"#);
    let mut c = Core::new();
    c.use_prefs();
    assert_eq!(c.mel("optionVar -q fromThere;"), json!(7));
    assert_eq!(c.mel("optionVar -q aFloat;"), json!(2.5));
    assert_eq!(c.mel("optionVar -q aString;"), json!("two words; and more"));
    assert_eq!(c.mel("optionVar -q aList;"), json!(["a", "b"]));
    assert_eq!(c.mel("optionVar -q mine;"), json!(3), "what Aztek saved wins");
    assert_eq!(c.mel("optionVar -ex old;"), json!(false), "another version's are not read");
    let py = c.startup_scripts();
    assert!(py.is_empty());
    assert_eq!(c.mel("global int $ufSetupRan; $ufSetupRan;"), json!(5));
    c.mel("aztekUserFolder -userSetup false;");
    c.mel("global int $ufSetupRan; $ufSetupRan = 0;");
    c.startup_scripts();
    assert_eq!(c.mel("global int $ufSetupRan; $ufSetupRan;"), json!(0));
    aztek_host::ui_cmds::set_keeps_prefs(false);

    // A version can be named.
    let q = c.mel("aztekUserFolder -version \"2024\";");
    assert_eq!(q["usedVersion"], json!("2024"));
    assert_eq!(c.mel("moduleInfo -mn \"ufTool\" -v;"), json!("1.0"));

    // Linking wrote nothing there.
    assert_eq!(snapshot(&theirs), before, "a linked folder is only read");

    // Copied: Aztek's own folder gets the files, the choice goes back to it, and a second copy replaces nothing.
    write(&own.join("scripts/ufShared.mel"), b"global proc int ufShared() { return 1; }\n");
    let r = c.mel(&format!("aztekUserFolder -copyFrom \"{}\" -version \"2025\";", slash(&theirs)));
    assert_eq!(r["kept"], json!(1), "{r}");
    assert!(r["failed"].as_array().unwrap().is_empty(), "{r}");
    assert!(own.join("scripts/userSetup.mel").is_file());
    assert!(own.join("plug-ins/theirTool.so").is_file());
    assert!(own.join("modules/tool.mod").is_file());
    assert!(own.join("prefs/userPrefs.mel").is_file());
    assert!(own.join("Aztek.env").is_file());
    assert_eq!(std::fs::read(own.join("scripts/ufShared.mel")).unwrap(), b"global proc int ufShared() { return 1; }\n");
    let q = c.mel("aztekUserFolder -q;");
    assert_eq!(q["mode"], json!("aztek"));
    assert_eq!(q["root"], json!(own.to_string_lossy()));
    let again = c.mel(&format!("aztekUserFolder -copyFrom \"{}\" -version \"2025\";", slash(&theirs)));
    assert_eq!(again["copied"], json!(0), "{again}");
    assert_eq!(snapshot(&theirs), before, "a copied folder is only read");

    // The paths hold only the folder in force.
    let scripts = std::env::var("AZTEK_SCRIPT_PATH").unwrap_or_default();
    assert!(!scripts.contains(&*theirs.join("2025").to_string_lossy()), "{scripts}");

    assert!(c.mel_checked("aztekUserFolder -mode \"custom\" -folder \"/no/such/folder\";").is_err());
    let _ = std::fs::remove_dir_all(&base);
}
