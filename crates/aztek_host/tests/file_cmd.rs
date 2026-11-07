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
    let path = dir.join("scene.ma").to_str().unwrap().to_string();
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
    let sel = dir.join("sel.ma").to_str().unwrap().to_string();
    c.mel(&format!("file -es -type \"mayaAscii\" \"{sel}\";"));
    c.mel(&format!("file -import \"{sel}\";"));
    assert_eq!(faces(&mut c, "pCube2"), 6);
    assert!(errors(&mut c).is_empty());

    let other = dir.join("other.ma").to_str().unwrap().to_string();
    c.save(&other).unwrap();
    assert!(std::fs::read_to_string(&other).unwrap().starts_with("//Maya ASCII"));
    assert_eq!(c.s.scene_path.as_deref(), Some(other.as_str()));
    let own = dir.join("own.aztek").to_str().unwrap().to_string();
    c.save(&own).unwrap();
    assert!(std::fs::read_to_string(&own).unwrap().trim_start().starts_with('{'));
    c.open(&other).unwrap();
    assert_eq!(faces(&mut c, "pCube2"), 6);
    c.open(&own).unwrap();
    assert_eq!(faces(&mut c, "pCube2"), 6);
    assert_eq!(c.s.scene_path.as_deref(), Some(own.as_str()));

    // The playback range lives in a script node. It is process-wide, so it is tested here, in this file's only test.
    let range = dir.join("range.ma").to_str().unwrap().to_string();
    std::fs::write(&range, "createNode script -n \"sceneConfigurationScriptNode\";\n\tsetAttr \".b\" -type \"string\" \"playbackOptions -min 5 -max 48 -ast 1 -aet 100 \";\n\tsetAttr \".st\" 6;\n").unwrap();
    c.open(&range).unwrap();
    assert_eq!(c.mel("playbackOptions -q -min;"), json!(5));
    assert_eq!(c.mel("playbackOptions -q -max;"), json!(48));
    assert_eq!(c.mel("playbackOptions -q -aet;"), json!(100));
    c.save(&range).unwrap();
    assert!(std::fs::read_to_string(&range).unwrap().contains("playbackOptions -min 5 -max 48 -ast 1 -aet 100 "));
    let _ = std::fs::remove_dir_all(dir);
}
