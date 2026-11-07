//! The animation commands as typed in MEL, and what they leave surviving a save and load.

use aztek_host::Core;
use serde_json::{json, Value};

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

/// Runs lines, none of which may fail, and gives the last result.
fn mel(c: &mut Core, src: &str) -> Value {
    let v = c.mel(src);
    let e = errors(c);
    assert!(e.is_empty(), "{src}: {e:?}");
    v
}

fn number(v: &Value) -> f64 {
    match v {
        Value::Array(a) => a[0].as_f64().unwrap(),
        o => o.as_f64().unwrap_or_else(|| panic!("not a number: {o}")),
    }
}

#[test]
fn tangents_and_key_flags() {
    let mut c = Core::new();
    mel(&mut c, "polyCube;");
    mel(&mut c, "setKeyframe -t 1 -v 0 -itt flat -ott flat -at tx pCube1;");
    mel(&mut c, "setKeyframe -t 25 -v 10 -itt flat -ott flat -at tx pCube1;");
    mel(&mut c, "keyTangent -e -wt 1 pCube1.tx;");
    assert_eq!(mel(&mut c, "keyTangent -q -wt pCube1.tx;"), json!([true]));
    mel(&mut c, "keyTangent -e -t 1 -l 0 -wl 0 -oa 0 -ow 3 pCube1.tx;");
    assert_eq!(number(&mel(&mut c, "keyTangent -q -t 1 -ow pCube1.tx;")), 3.0);
    assert_eq!(mel(&mut c, "keyTangent -q -t 1 -l pCube1.tx;"), json!([false]));
    assert_eq!(mel(&mut c, "keyTangent -q -t 1 -wl pCube1.tx;"), json!([false]));
    mel(&mut c, "keyTangent -e -r -t 1 -oa 10 pCube1.tx;");
    assert!((number(&mel(&mut c, "keyTangent -q -t 1 -oa pCube1.tx;")) - 10.0).abs() < 1e-9);
    mel(&mut c, "keyTangent -e -t 25 -unify pCube1.tx;");
    mel(&mut c, "keyframe -e -t 1 -r -vc 2 pCube1.tx;");
    assert_eq!(mel(&mut c, "keyframe -q -vc pCube1.tx;"), json!([2.0, 10.0]));
    assert_eq!(mel(&mut c, "keyframe -q -kc -t \"1:25\" pCube1.tx;"), json!(2));
    mel(&mut c, "setKeyframe -i -t 13 pCube1.tx;");
    mel(&mut c, "selectKey -add -k -t 13 pCube1.tx;");
    assert_eq!(mel(&mut c, "keyframe -q -sl -tc;"), json!([13.0]));
    mel(&mut c, "selectKey -cl;");
    mel(&mut c, "cutKey -cl -t 13 pCube1.tx;");
    mel(&mut c, "filterCurve -f euler pCube1;");
    mel(&mut c, "simplify -vtl 0.01 pCube1.tx;");
    mel(&mut c, "mute pCube1.tx;");
    assert_eq!(mel(&mut c, "mute -q pCube1.tx;"), json!(true));
    mel(&mut c, "mute -d -f pCube1.tx;");
    mel(&mut c, "setAttr -k off pCube1.tz;");
    assert_eq!(mel(&mut c, "getAttr -k pCube1.tz;"), json!(false));
}

#[test]
fn motion_path_lines() {
    let mut c = Core::new();
    c.s.run("curve", json!({"degree": 1, "point": [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], "name": "path"})).unwrap();
    mel(&mut c, "polyCube;");
    let mp = mel(&mut c, "pathAnimation -c path -f 1 -fm 1 -fa x -ua y -wut vector -wu 0 1 0 -iu -if -b 1 -bs 1 -bl 45 -stu 1 -etu 120 pCube1;");
    assert_eq!(mp, json!("motionPath1"));
    assert_eq!(mel(&mut c, "pathAnimation -q -c motionPath1;"), json!("path"));
    assert_eq!(mel(&mut c, "pathAnimation -q -fa motionPath1;"), json!("x"));
    assert_eq!(mel(&mut c, "pathAnimation -q -etu pCube1;"), json!(120));
    assert_eq!(mel(&mut c, "getAttr motionPath1.bankLimit;"), json!(45.0));
    let snap = mel(&mut c, "snapshot -i 40 -st 1 -et 120 -ch 1 pCube1;");
    assert_eq!(snap, json!(["snapshot1Group", "snapshot1"]));
    assert_eq!(mel(&mut c, "motionTrail -st 1 -et 3 pCube1;"), json!(["motionTrail1Handle", "motionTrail1"]));
    mel(&mut c, "motionTrail -e -ev motionTrail1;");
}

#[test]
fn layer_character_and_clip_lines() {
    let mut c = Core::new();
    mel(&mut c, "polyCube;");
    mel(&mut c, "select -r pCube1;");
    assert_eq!(mel(&mut c, "animLayer -aso -ebl -ert -esc L1;"), json!("L1"));
    mel(&mut c, "animLayer -e -w 0.5 -o 0 -m 0 -l 0 -s 0 L1;");
    assert_eq!(mel(&mut c, "animLayer -q -w L1;"), json!(0.5));
    assert_eq!(mel(&mut c, "animLayer -q -p L1;"), json!("BaseAnimation"));
    assert_eq!(mel(&mut c, "animLayer -q -r;"), json!("BaseAnimation"));
    assert_eq!(mel(&mut c, "animLayer -q -afl pCube1.tx;"), json!(["BaseAnimation", "L1"]));
    mel(&mut c, "setKeyframe -al L1 -t 1 -v 4 -nr pCube1.tx;");
    assert_eq!(mel(&mut c, "animLayer -q -anc L1;"), json!(["pCube1_translateX_L1_inputB"]));
    assert_eq!(mel(&mut c, "getAttr pCube1.tx;"), json!(2.0));

    mel(&mut c, "polySphere;");
    mel(&mut c, "character -n hero pSphere1.tx pSphere1.ty;");
    assert_eq!(mel(&mut c, "character -q -no hero;"), json!(["pSphere1"]));
    mel(&mut c, "setKeyframe -t 1 -v 0 pSphere1.tx;");
    mel(&mut c, "setKeyframe -t 10 -v 9 pSphere1.tx;");
    assert_eq!(mel(&mut c, "clip -n walk -sc 1 -st 1 -et 10 hero;"), json!(["walkSource", "walk"]));
    assert_eq!(mel(&mut c, "clipSchedule -q heroScheduler;"), json!(["walk,0,1,1,10,1,0,1,0"]));
    assert_eq!(mel(&mut c, "pose -n stand hero;"), json!("stand"));
    assert_eq!(mel(&mut c, "pose -a -n stand hero;"), json!("stand"));
}

#[test]
fn layers_paths_and_weights_survive_a_save() {
    let mut c = Core::new();
    c.s.run("curve", json!({"degree": 1, "point": [[0.0, 0.0, 0.0], [10.0, 0.0, 0.0]], "name": "path"})).unwrap();
    mel(&mut c, "polyCube;");
    mel(&mut c, "pathAnimation -c path -fm 1 pCube1;");
    mel(&mut c, "setAttr motionPath1.uValue 0.5;");
    // Weighted keys with their own weights, on the sphere.
    mel(&mut c, "polySphere;");
    mel(&mut c, "setKeyframe -t 1 -v 0 -itt flat -ott flat pSphere1.tx;");
    mel(&mut c, "setKeyframe -t 25 -v 10 -itt flat -ott flat pSphere1.tx;");
    mel(&mut c, "keyTangent -e -wt 1 pSphere1.tx;");
    mel(&mut c, "keyTangent -e -t 1 -l 0 -oa 0 -ow 3 pSphere1.tx;");
    // A layer over a second channel.
    mel(&mut c, "setKeyframe -t 1 -v 1 pSphere1.ty;");
    mel(&mut c, "animLayer L1;");
    mel(&mut c, "animLayer -e -at pSphere1.ty L1;");
    mel(&mut c, "setKeyframe -al L1 -t 1 -v 2 -nr pSphere1.ty;");
    // A character's clip.
    mel(&mut c, "polyTorus;");
    mel(&mut c, "character -n hero pTorus1.tx;");
    mel(&mut c, "setKeyframe -t 1 -v 0 pTorus1.tx;");
    mel(&mut c, "setKeyframe -t 11 -v 10 pTorus1.tx;");
    mel(&mut c, "clip -n walk hero;");
    mel(&mut c, "currentTime 1;");
    assert_eq!(number(&mel(&mut c, "getAttr pSphere1.ty;")), 3.0);

    let dir = std::env::temp_dir().join(format!("aztek_anim_mel_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.json");
    let path = path.to_str().unwrap();
    c.save(path).unwrap();
    c.open(path).unwrap();
    errors(&mut c);
    std::fs::remove_dir_all(&dir).unwrap();

    assert_eq!(number(&mel(&mut c, "getAttr pCube1.tx;")), 5.0);
    assert_eq!(mel(&mut c, "keyTangent -q -wt pSphere1.tx;"), json!([true]));
    assert_eq!(number(&mel(&mut c, "keyTangent -q -t 1 -ow pSphere1.tx;")), 3.0);
    mel(&mut c, "currentTime 19;");
    assert!((number(&mel(&mut c, "getAttr pSphere1.tx;")) - 5.0).abs() < 1e-6);
    mel(&mut c, "currentTime 1;");
    assert_eq!(number(&mel(&mut c, "getAttr pSphere1.ty;")), 3.0);
    assert_eq!(mel(&mut c, "animLayer -q -at L1;"), json!(["pSphere1.translateY"]));
    assert_eq!(mel(&mut c, "character -q hero;"), json!(["pTorus1.translateX"]));
    assert_eq!(mel(&mut c, "clipSchedule -q hero;"), json!(["walk,0,1,1,11,1,0,1,0"]));
    mel(&mut c, "currentTime 11;");
    assert_eq!(number(&mel(&mut c, "getAttr pTorus1.tx;")), 10.0);
}
