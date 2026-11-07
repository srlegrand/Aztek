//! USD from MEL: flags as scripts write them, the File command with translators' option strings, prims named in strings,
//! scriptJob events, cameras, stats, collections, resolver and translators.

use aztek_host::Core;
use serde_json::{json, Value};

fn mel(c: &mut Core, src: &str) -> Value {
    match c.mel_checked(src) {
        Ok(v) => v,
        Err(e) => panic!("{src}: {e}"),
    }
}

fn temp(name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("aztek_usd_mel_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name).to_string_lossy().replace('\\', "/")
}

const STAGE: &str = r#"#usda 1.0
def Xform "World"
{
    def Camera "shot"
    {
        float focalLength = 85
        float horizontalAperture = 36
        float verticalAperture = 24
        double3 xformOp:translate = (1, 2, 3)
        uniform token[] xformOpOrder = ["xformOp:translate"]
    }
    def Xform "set" (
        prepend apiSchemas = ["CollectionAPI:props"]
    )
    {
        uniform token collection:props:expansionRule = "expandPrims"
        rel collection:props:includes = </World/set/a>
        rel collection:props:excludes = </World/set/a/skip>
        def Xform "a"
        {
            def Cube "keep"
            {
            }
            def Cube "skip"
            {
            }
        }
    }
    def "custom" (
    )
    {
    }
    def SphereLight "lamp"
    {
        float inputs:radius = 1
        color3f inputs:color = (1, 0.5, 0)
    }
}
"#;

#[test]
fn import_export_flags_and_the_file_command() {
    let mut c = Core::new();
    mel(&mut c, "polyCube -n box; setKeyframe -t 1 -v 0 box.tx; setKeyframe -t 3 -v 2 box.tx;");
    let a = temp("flags.usda");
    mel(&mut c, &format!("mayaUSDExport -file \"{a}\" -frameRange 1 3 -exportUVs 0 -selection;"));
    let text = std::fs::read_to_string(&a).unwrap();
    assert!(text.contains("timeSamples"), "{text}");
    assert!(!text.contains("primvars:st"), "-exportUVs 0");
    // Short names.
    let b = temp("short.usda");
    mel(&mut c, &format!("mayaUSDExport -f \"{b}\" -fr 1 3 -mt 0 -sl;"));
    assert!(std::fs::read_to_string(&b).unwrap().contains("boxShape"), "-mt 0 keeps the shape's own prim");
    // Translator names and options, as File > Export writes them.
    let d = temp("byfile.usda");
    mel(&mut c, &format!("file -force -options \"exportUVs=0;animation=1;startTime=1;endTime=3;\" -type \"USD Export\" -es \"{d}\";"));
    let text = std::fs::read_to_string(&d).unwrap();
    assert!(text.contains("timeSamples") && !text.contains("primvars:st"), "{text}");
    mel(&mut c, "file -f -new;");
    mel(&mut c, &format!("file -i -type \"USD Import\" -options \"readAnimData=1\" \"{d}\";"));
    assert_eq!(mel(&mut c, "keyframe -q -kc box.tx;"), json!(3));
    mel(&mut c, "file -f -new;");
    let r = mel(&mut c, &format!("mayaUSDImport -file \"{d}\" -readAnimData 1 -pp \"/box\";"));
    assert_eq!(r, json!(["box"]));
    assert_eq!(mel(&mut c, "keyframe -q -kc box.tx;"), json!(3));
}

#[test]
fn prims_named_in_strings_cameras_stats_collections() {
    let mut c = Core::new();
    let f = temp("stage.usda");
    std::fs::write(&f, STAGE).unwrap();
    let shape = mel(&mut c, &format!("mayaUsd_createStageFromFile \"{f}\";"));
    let shape = shape.as_str().unwrap().to_string();
    let world = format!("{shape},/World");
    mel(&mut c, &format!("select -r \"{world}\";"));
    assert!(mel(&mut c, "ls -sl;")[0].as_str().unwrap().ends_with(",/World"));
    mel(&mut c, &format!("setAttr \"{shape},/World/set/a.visibility\" 0;"));
    assert_eq!(mel(&mut c, &format!("getAttr \"{shape},/World/set/a.visibility\";")), json!("invisible"));
    mel(&mut c, "undo;");

    // A camera following its prim.
    let cam = mel(&mut c, &format!("usdCamera \"{shape},/World/shot\";"));
    let cam = cam.as_str().unwrap().to_string();
    assert_eq!(mel(&mut c, &format!("getAttr {cam}.tx;")), json!(1.0));
    let lens = mel(&mut c, &format!("getAttr {cam}Shape.focalLength;"));
    assert!((lens.as_f64().unwrap() - 85.0).abs() < 1e-4, "{lens}");
    mel(&mut c, &format!("usdAttr -value 50 \"{shape},/World/shot.focalLength\";"));
    aztek_usd::extras::follow_cameras(&mut c.s.graph);
    let lens = mel(&mut c, &format!("getAttr {cam}Shape.focalLength;"));
    assert!((lens.as_f64().unwrap() - 50.0).abs() < 1e-4, "{lens}");

    // Numbers.
    let st = mel(&mut c, &format!("usdStats \"{shape}\";"));
    assert_eq!(st["byType"]["Camera"], json!(1));
    assert_eq!(st["byType"]["Cube"], json!(2));
    assert_eq!(st["layers"].as_array().unwrap().len(), 2);

    // Collections.
    let col = mel(&mut c, &format!("usdCollections \"{shape},/World/set\";"));
    assert_eq!(col[0]["name"], json!("props"));
    assert_eq!(col[0]["members"], json!(["/World/set/a", "/World/set/a/keep"]));
}

#[test]
fn script_jobs_hear_stage_events() {
    let mut c = Core::new();
    let f = temp("events.usda");
    std::fs::write(&f, STAGE).unwrap();
    let shape = mel(&mut c, &format!("mayaUsd_createStageFromFile \"{f}\";")).as_str().unwrap().to_string();
    mel(&mut c, "global int $usdHeard = 0; global proc usdHeardIt() { global int $usdHeard; $usdHeard += 1; } scriptJob -event \"usdStageChanged\" \"usdHeardIt\";");
    mel(&mut c, &format!("usdPrim -define \"Cube\" \"{shape},/World/new\";"));
    c.idle();
    let n = mel(&mut c, "global int $usdHeard; $usdHeard;");
    assert!(n.as_i64().unwrap() >= 1, "{n}");
}

#[test]
fn resolver_search_paths_and_map() {
    let mut c = Core::new();
    let dir = temp("assets");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(format!("{dir}/thing.usda"), "#usda 1.0\ndef Cube \"Thing\"\n{\n}\n").unwrap();
    std::fs::write(format!("{dir}/other.usda"), "#usda 1.0\ndef Cube \"Thing\"\n{\n}\n").unwrap();
    let f = temp("uses.usda");
    std::fs::write(&f, "#usda 1.0\ndef \"a\" (\n    references = @thing.usda@</Thing>\n)\n{\n}\ndef \"b\" (\n    references = @logical://thing.usda@</Thing>\n)\n{\n}\n").unwrap();
    mel(&mut c, &format!("usdResolver -addSearchPath \"{dir}\";"));
    mel(&mut c, &format!("usdResolver -map \"logical://thing.usda\" \"{dir}/other.usda\";"));
    let shape = mel(&mut c, &format!("mayaUsd_createStageFromFile \"{f}\";")).as_str().unwrap().to_string();
    let info = mel(&mut c, &format!("usdPrim -q \"{shape},/a\";"));
    assert_eq!(info["type"], json!("Cube"), "found on the search path");
    let info = mel(&mut c, &format!("usdPrim -q \"{shape},/b\";"));
    assert_eq!(info["type"], json!("Cube"), "found through the map");
    mel(&mut c, "usdResolver -clearSearchPaths;");
}
