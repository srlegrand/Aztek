//! The file command from MEL, and Core's save and open, which the File menu calls.

use aztek_host::Core;
use serde_json::json;

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn faces(c: &mut Core, m: &str) -> i64 {
    c.s.run("meshInfo", json!({"args": [m]})).unwrap()["faces"].as_i64().unwrap()
}

#[test]
fn the_file_command_in_mel() {
    let dir = std::env::temp_dir().join(format!("aztek_mel_file_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.ma").to_str().unwrap().replace('\\', "/");
    let mut c = Core::new();
    assert_eq!(c.mel("file -q -sn;"), json!(""));
    c.mel("polyCube -w 2;");
    assert_eq!(c.mel(&format!("file -rename \"{path}\";")), json!(path));
    assert_eq!(c.mel("file -q -sn;"), json!(path));
    assert_eq!(c.mel("file -q -ex;"), json!(false));
    c.mel("file -save -type \"mayaAscii\";");
    assert_eq!(c.mel(&format!("file -q -ex \"{path}\";")), json!(true));
    assert_eq!(c.mel("file -q -type;"), json!(["mayaAscii"]));
    c.mel("file -f -new;");
    assert_eq!(c.s.graph.find("pCube1").unwrap(), None);
    c.mel(&format!("file -f -options \"v=0;\" -typ \"mayaAscii\" -o \"{path}\";"));
    assert_eq!(faces(&mut c, "pCube1"), 6);
    c.mel("select -r pCube1;");
    let sel = dir.join("sel.ma").to_str().unwrap().replace('\\', "/");
    c.mel(&format!("file -es -type \"mayaAscii\" \"{sel}\";"));
    c.mel(&format!("file -import \"{sel}\";"));
    assert_eq!(faces(&mut c, "pCube2"), 6);
    assert!(errors(&mut c).is_empty());

    let other = dir.join("other.ma").to_str().unwrap().replace('\\', "/");
    c.save(&other).unwrap();
    assert!(std::fs::read_to_string(&other).unwrap().starts_with("//Maya ASCII"));
    assert_eq!(c.s.scene_path.as_deref(), Some(other.as_str()));
    // A .mb name saves a binary scene, as the reference's mayaBinary.
    let binary = dir.join("binary.mb").to_str().unwrap().replace('\\', "/");
    c.save(&binary).unwrap();
    assert!(std::fs::read(&binary).unwrap().starts_with(b"FOR8"));
    c.open(&other).unwrap();
    assert_eq!(faces(&mut c, "pCube2"), 6);
    c.open(&binary).unwrap();
    assert_eq!(faces(&mut c, "pCube2"), 6);
    assert_eq!(c.s.scene_path.as_deref(), Some(binary.as_str()));
    assert_eq!(c.mel("file -q -type;"), json!(["mayaBinary"]));
    // Aztek's old JSON scene format is refused by name.
    let old = dir.join("old.aztek").to_str().unwrap().replace('\\', "/");
    std::fs::write(&old, "{\"aztek\": 1, \"nodes\": []}").unwrap();
    assert!(c.open(&old).unwrap_err().contains("no longer read"));

    // OBJ and USD are plug-ins, as the reference's objExport and mayaUsdPlugin, loaded at start.
    assert_eq!(c.mel("pluginInfo -q -loaded objExport;"), json!(true));
    assert_eq!(c.mel("pluginInfo -q -loaded mayaUsdPlugin;"), json!(true));
    let obj = dir.join("cube.obj").to_str().unwrap().replace('\\', "/");
    c.mel("select -r pCube2;");
    c.mel(&format!("file -es -type \"OBJexport\" \"{obj}\";"));
    assert!(std::fs::read_to_string(&obj).unwrap().contains("\nf "));
    c.mel(&format!("file -i -type \"OBJ\" \"{obj}\";"));
    assert!(errors(&mut c).is_empty());
    let usd = dir.join("cube.usda").to_str().unwrap().replace('\\', "/");
    c.mel(&format!("file -ea -type \"USD Export\" \"{usd}\";"));
    assert!(std::fs::read_to_string(&usd).unwrap().starts_with("#usda"));
    c.mel("unloadPlugin objExport;");
    c.mel("unloadPlugin mayaUsdPlugin;");
    assert!(c.s.commands.find("importObj").is_none());
    assert!(c.s.commands.find("mayaUSDImport").is_none());
    let _ = errors(&mut c);
    c.mel(&format!("file -i \"{obj}\";"));
    assert!(errors(&mut c).iter().any(|e| e.contains("objExport plug-in is not loaded")));
    c.mel(&format!("file -i \"{usd}\";"));
    assert!(errors(&mut c).iter().any(|e| e.contains("mayaUsdPlugin plug-in is not loaded")));
    c.mel("loadPlugin objExport;");
    c.mel("loadPlugin mayaUsdPlugin;");
    c.mel(&format!("file -i \"{obj}\";"));
    c.mel(&format!("file -i \"{usd}\";"));
    assert!(errors(&mut c).is_empty());

    // The playback range lives in a script node. It is process-wide, so it is tested here, in this file's only test.
    let range = dir.join("range.ma").to_str().unwrap().replace('\\', "/");
    std::fs::write(&range, "createNode script -n \"sceneConfigurationScriptNode\";\n\tsetAttr \".b\" -type \"string\" \"playbackOptions -min 5 -max 48 -ast 1 -aet 100 \";\n\tsetAttr \".st\" 6;\n").unwrap();
    c.open(&range).unwrap();
    assert_eq!(c.mel("playbackOptions -q -min;"), json!(5));
    assert_eq!(c.mel("playbackOptions -q -max;"), json!(48));
    assert_eq!(c.mel("playbackOptions -q -aet;"), json!(100));
    c.save(&range).unwrap();
    assert!(std::fs::read_to_string(&range).unwrap().contains("playbackOptions -min 5 -max 48 -ast 1 -aet 100 "));
    let _ = std::fs::remove_dir_all(dir);
}
