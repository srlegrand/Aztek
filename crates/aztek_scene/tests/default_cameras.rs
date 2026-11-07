//! The four cameras every scene starts with, against what a new scene saved by the reference holds.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn scene() -> Session {
    let mut s = Session::new();
    aztek_scene::cameras::make_defaults(&mut s.graph);
    s
}

fn numbers(v: Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.into_iter().flat_map(numbers).collect(),
        Value::Bool(b) => vec![b as i32 as f64],
        v => vec![v.as_f64().unwrap_or_else(|| panic!("{v}"))],
    }
}

#[test]
fn names_shapes_and_values_are_those_of_a_new_scene() {
    let mut s = scene();
    assert_eq!(run(&mut s, "ls", json!({"type": "camera"})), json!(["perspShape", "topShape", "frontShape", "sideShape"]));
    for (cam, t, r, ortho, coi, rnd, home) in [
        ("persp", [28.0, 21.0, 28.0], [-27.938352729602379, 45.0, 0.0], 0.0, 44.82186966202994, 1.0, "viewSet -p %camera"),
        ("top", [0.0, 1000.1, 0.0], [-90.0, 0.0, 0.0], 1.0, 1000.1, 0.0, "viewSet -t %camera"),
        ("front", [0.0, 0.0, 1000.1], [0.0, 0.0, 0.0], 1.0, 1000.1, 0.0, "viewSet -f %camera"),
        ("side", [1000.1, 0.0, 0.0], [0.0, 90.0, 0.0], 1.0, 1000.1, 0.0, "viewSet -s %camera"),
    ] {
        let mut num = |p: String| numbers(run(&mut s, "getAttr", json!({"args": [p]})));
        assert_eq!(num(format!("{cam}.t")), t, "{cam}");
        assert_eq!(num(format!("{cam}.r")), r, "{cam}");
        assert_eq!(num(format!("{cam}.v")), [0.0], "{cam} is hidden");
        let sh = format!("{cam}Shape");
        assert_eq!(num(format!("{sh}.o")), [ortho], "{sh}.o");
        assert_eq!(num(format!("{sh}.coi")), [coi], "{sh}.coi");
        assert_eq!(num(format!("{sh}.rnd")), [rnd], "{sh}.rnd");
        assert_eq!(num(format!("{sh}.ow")), [30.0], "{sh}.ow");
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{sh}.imn")]})), json!(cam));
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{sh}.den")]})), json!(format!("{cam}_depth")));
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{sh}.man")]})), json!(format!("{cam}_mask")));
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{sh}.hc")]})), json!(home));
    }
}

#[test]
fn startup_cameras_stay_until_a_script_says_otherwise() {
    let mut s = scene();
    assert_eq!(run(&mut s, "camera", json!({"args": ["persp"], "q": true, "sc": true, "queried": ["startupCamera"]})), json!(true));
    let e = s.run("delete", json!({"args": ["persp"]})).unwrap_err().to_string();
    assert!(e.contains("startup"), "{e}");
    assert!(s.run("delete", json!({"args": ["topShape"]})).is_err());
    run(&mut s, "camera", json!({"args": ["persp"], "e": true, "sc": false}));
    run(&mut s, "delete", json!({"args": ["persp"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["persp"]})), json!(false));
    run(&mut s, "newScene", json!({}));
    aztek_scene::cameras::make_defaults(&mut s.graph);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["persp"]})), json!(true), "a new scene has it again");
}

#[test]
fn a_saved_scene_keeps_what_the_camera_was_set_to() {
    let mut s = scene();
    run(&mut s, "setAttr", json!({"args": ["perspShape.fl", 85.0]}));
    run(&mut s, "setAttr", json!({"args": ["perspShape.dfg", true]}));
    run(&mut s, "setAttr", json!({"args": ["persp.tx", 3.0]}));
    let (text, w) = aztek_scene::ma_file::write_text(&mut s, "a.ma", None);
    assert!(w.is_empty(), "{w:?}");
    assert!(text.contains("setAttr \".hc\" -type \"string\" \"viewSet -t %camera\""), "{text}");
    assert!(text.contains("setAttr \".rnd\" no"), "{text}");
    let mut t = scene();
    aztek_scene::ma_file::read_text(&mut t, &text, false);
    assert_eq!(numbers(run(&mut t, "getAttr", json!({"args": ["perspShape.fl"]}))), [85.0]);
    assert_eq!(numbers(run(&mut t, "getAttr", json!({"args": ["perspShape.dfg"]}))), [1.0]);
    assert_eq!(numbers(run(&mut t, "getAttr", json!({"args": ["persp.tx"]}))), [3.0]);
    assert_eq!(run(&mut t, "ls", json!({"type": "camera"})).as_array().unwrap().len(), 4, "still only the four");
}

/// What decides whether closing or opening asks about saving.
#[test]
fn modified_follows_edits_saves_and_undo() {
    let dir = std::env::temp_dir().join(format!("aztek_modified_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("a.ma").to_string_lossy().to_string();
    let modified = |s: &mut Session| run(s, "file", json!({"q": true, "mf": true, "queried": ["modified"]}));
    let mut s = scene();
    assert_eq!(modified(&mut s), json!(false), "a new scene");
    run(&mut s, "polyCube", json!({"name": "a"}));
    assert_eq!(modified(&mut s), json!(true));
    assert!(s.run("file", json!({"new": true})).is_err(), "a script has to say -force");
    assert!(s.run("file", json!({"args": [path.clone()], "open": true})).is_err());
    assert_eq!(run(&mut s, "objExists", json!({"args": ["a"]})), json!(true), "and the scene is untouched");
    run(&mut s, "file", json!({"args": [path.clone()], "rename": path.clone()}));
    run(&mut s, "file", json!({"save": true}));
    assert_eq!(modified(&mut s), json!(false), "saved");
    run(&mut s, "setAttr", json!({"args": ["a.tx", 2.0]}));
    assert_eq!(modified(&mut s), json!(true));
    run(&mut s, "undo", json!({}));
    assert_eq!(modified(&mut s), json!(false), "back at what's saved");
    run(&mut s, "setAttr", json!({"args": ["a.tx", 3.0]}));
    run(&mut s, "file", json!({"args": [path.clone()], "open": true, "force": true}));
    assert_eq!(modified(&mut s), json!(false), "just opened");
    run(&mut s, "polyCube", json!({"name": "b"}));
    run(&mut s, "file", json!({"new": true, "force": true}));
    assert_eq!(modified(&mut s), json!(false), "new");
    std::fs::remove_dir_all(dir).ok();
}
