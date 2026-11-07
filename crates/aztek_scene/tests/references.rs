//! File references against scenes the reference itself wrote (`tests/data/ref_files`): load state, edits, placeholders, and the
//! world matrices the reference gave for them. The scenes name a Windows path, so each is found by its name beside the scene.

use aztek_scene::Session;
use serde_json::{json, Value};
use std::path::PathBuf;

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn get(s: &mut Session, plug: &str) -> Vec<f64> {
    match run(s, "getAttr", json!({"args": [plug]})) {
        Value::Array(a) => a.iter().flat_map(|x| x.as_array().cloned().unwrap_or_else(|| vec![x.clone()])).map(|x| x.as_f64().unwrap()).collect(),
        Value::Bool(b) => vec![b as i32 as f64],
        v => vec![v.as_f64().unwrap_or_else(|| panic!("{plug}: {v}"))],
    }
}

fn exists(s: &mut Session, name: &str) -> bool {
    run(s, "objExists", json!({"args": [name]})) == json!(true)
}

/// A folder of its own with the reference scenes in it.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("aztek_refs_{}_{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let data = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/data/ref_files");
    for f in ["skeleton.ma", "refscene.ma", "refedits2.ma", "unloaded.ma"] {
        std::fs::copy(data.join(f), dir.join(f)).unwrap();
    }
    dir
}

fn open(s: &mut Session, path: &std::path::Path) {
    run(s, "file", json!({"args": [path.to_string_lossy()], "open": true, "force": true}));
}

fn text(p: &std::path::Path) -> String {
    p.to_string_lossy().to_string()
}

fn write(dir: &std::path::Path, name: &str, body: &str) -> String {
    let p = dir.join(name);
    std::fs::write(&p, format!("//Maya ASCII 2024 scene\nrequires maya \"2024\";\ncurrentUnit -l centimeter -a degree -t film;\n{body}")).unwrap();
    text(&p)
}

/// World matrices the reference reported with `xform -q -ws -m`, after the edits in refscene.ma.
const TRUTH: &[(&str, [f64; 16])] = &[
    ("rig:root", [-0.469846310393, 0.813797681349, -0.342020143326, 0.0, -0.882564119259, -0.44096961053, 0.163175911167, 0.0, -0.018028311236, 0.37852230637, 0.925416578398, 0.0, 1.0, 12.0, 3.0, 1.0]),
    ("rig:spine", [-0.701567896029, 1.181623607978, -0.601305360304, 0.0, 0.072344881655, -0.645169516403, -1.352228720743, 0.0, -1.323846178889, -0.661454415795, 0.24476386675, 0.0, -3.69846310393, 20.137976813494, -0.420201433257, 1.0]),
    ("rig:arm", [0.098244206935, 0.984363007584, -0.146210618983, 0.0, -0.114645600606, -0.134747971975, -0.984225264007, 0.0, -0.988536525408, 0.113456834736, 0.099614680577, 0.0, 6.17121907024, 31.913266590729, -7.141951535948, 1.0]),
    ("rig:hand", [0.098244206935, 0.984363007584, -0.146210618983, 0.0, -0.114645600606, -0.134747971975, -0.984225264007, 0.0, -0.988536525408, 0.113456834736, 0.099614680577, 0.0, 6.858928518784, 38.803807643819, -8.165425868827, 1.0]),
    ("rig:neck", [0.882564119259, 0.44096961053, -0.163175911167, 0.0, -0.449170615296, 0.688099415905, -0.569881524692, 0.0, -0.139019184828, 0.576250810343, 0.80536244625, 0.0, -10.853079887858, 32.188220483095, -6.552336964895, 1.0]),
    ("rigb:root", [-0.469846310393, 0.813797681349, -0.342020143326, 0.0, -0.882564119259, -0.44096961053, 0.163175911167, 0.0, -0.018028311236, 0.37852230637, 0.925416578398, 0.0, 1.0, 12.0, 3.0, 1.0]),
    ("rigb:hand", [0.098244206935, 0.984363007584, -0.146210618983, 0.0, -0.114645600606, -0.134747971975, -0.984225264007, 0.0, -0.988536525408, 0.113456834736, 0.099614680577, 0.0, 6.858928518784, 38.803807643819, -8.165425868827, 1.0]),
    ("rig:marker", [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, -4.0, 2.0, 3.0, 1.0]),
];

fn check_truth(s: &mut Session) {
    for (name, want) in TRUTH {
        let got = get(s, &format!("{name}.worldMatrix"));
        assert_eq!(got.len(), 16, "{name}: {got:?}");
        for (i, (a, b)) in got.iter().zip(want).enumerate() {
            assert!((a - b).abs() < 1e-6, "{name} [{i}]: {got:?} want {want:?}");
        }
    }
}

/// Every `file -r` line the reference writes says -dr 1: the load state is on the -rdi line, and these two are loaded.
#[test]
fn a_reference_scene_loads_its_references_and_matches_the_reference() {
    let dir = folder("truth");
    let mut s = Session::new();
    open(&mut s, &dir.join("refscene.ma"));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["rigRN"], "il": true})), json!(true));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["rigbRN"], "il": true})), json!(true));
    check_truth(&mut s);
    assert_eq!(get(&mut s, "rig:neck.visibility"), vec![0.0]);
    assert_eq!(get(&mut s, "rig:hand.grip"), vec![1.0], "an attribute the scene added to a referenced node");
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["rig:props"], "parent": true})), json!(["stage"]), "the parent edit");
}

#[test]
fn a_reference_saved_unloaded_stays_unloaded_with_its_edits() {
    let dir = folder("unloaded");
    let mut s = Session::new();
    open(&mut s, &dir.join("unloaded.ma"));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["rigRN"], "il": true})), json!(false));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["rigbRN"], "il": true})), json!(true));
    assert!(!exists(&mut s, "rig:root"));
    assert!(exists(&mut s, "rigb:root"));
    run(&mut s, "file", json!({"lr": "rigRN"}));
    assert_eq!(get(&mut s, "rig:spine.translateX"), vec![5.0]);
}

/// Connections and animation with a reference go through its placeholder list.
#[test]
fn placeholders_connect_both_ways_and_carry_animation() {
    let dir = folder("placeholders");
    let mut s = Session::new();
    open(&mut s, &dir.join("refedits2.ma"));
    let src = |s: &mut Session, p: &str| run(s, "listConnections", json!({"args": [p], "s": true, "d": false, "p": true}));
    assert_eq!(src(&mut s, "rig:spine.translateX"), json!(["driver.translateX"]));
    assert_eq!(src(&mut s, "driver.translateY"), json!(["rig:root.translateY"]));
    assert_eq!(src(&mut s, "rig:hand.rotateZ"), json!(["hand_rotateZ.output"]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["rig:neck.translateX"], "lock": true})), json!(true), "the lock edit");
}

/// Save and open again: the keys, values, parents and locks are all still there and in the reference's form.
#[test]
fn saving_writes_the_reference_reference_edits_that_read_back() {
    let dir = folder("resave");
    let mut s = Session::new();
    open(&mut s, &dir.join("refscene.ma"));
    run(&mut s, "setKeyframe", json!({"args": ["rig:hand.tx"], "t": 1.0, "v": 3.0}));
    run(&mut s, "setKeyframe", json!({"args": ["rig:hand.tx"], "t": 10.0, "v": 9.0}));
    run(&mut s, "setAttr", json!({"args": ["rig:arm.ry", 40.0]}));
    run(&mut s, "setAttr", json!({"args": ["rig:root.visibility"], "lock": true}));
    run(&mut s, "addAttr", json!({"args": ["rig:root"], "ln": "note", "at": "double", "dv": 2.0}));
    let out = dir.join("again.ma");
    run(&mut s, "file", json!({"args": [text(&out)], "rename": text(&out)}));
    run(&mut s, "file", json!({"save": true}));
    let saved = std::fs::read_to_string(&out).unwrap();
    assert!(saved.contains("-type \"dataReferenceEdits\""), "in the reference's form");
    assert!(saved.contains("placeHolderList[1]") && saved.contains("\"rigRN.phl[1]\""), "animation goes through a placeholder");
    assert!(saved.contains("0 \"|rig:props\"") || saved.contains("0 \"|stage|rig:props\""), "the parent edit: {}", &saved[..saved.len().min(3000)]);
    assert!(!saved.contains("createNode joint"), "The referenced contents stay out");
    assert!(!saved.contains("setAttr \".r\" -type \"double3\" 10 20 30"), "no plain lines for referenced nodes");

    let mut t = Session::new();
    open(&mut t, &out);
    assert_eq!(get(&mut t, "rig:arm.ry"), vec![40.0]);
    assert_eq!(run(&mut t, "keyframe", json!({"args": ["rig:hand.tx"], "q": true, "kc": true, "queried": ["keyframeCount"]})), json!(2), "the keys came back");
    run(&mut t, "currentTime", json!({"args": [10.0]}));
    assert_eq!(get(&mut t, "rig:hand.tx"), vec![9.0]);
    assert_eq!(run(&mut t, "getAttr", json!({"args": ["rig:root.visibility"], "lock": true})), json!(true));
    assert_eq!(get(&mut t, "rig:root.note"), vec![2.0]);
    assert_eq!(run(&mut t, "listRelatives", json!({"args": ["rig:props"], "parent": true})), json!(["stage"]));
    assert_eq!(get(&mut t, "rig:marker.translate"), vec![-4.0, 2.0, 3.0]);
}

/// The point of a reference: the file changes and every scene using it follows, keeping its own edits.
#[test]
fn a_changed_file_comes_through_on_reload_and_the_scene_keeps_its_edits() {
    let dir = folder("follow");
    let rig = write(&dir, "prop.ma", "createNode transform -n \"box\";\n\tsetAttr \".t\" -type \"double3\" 1 2 3 ;\ncreateNode transform -n \"lid\" -p \"box\";\n");
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [rig.clone()], "r": true, "ns": "a"}));
    run(&mut s, "setAttr", json!({"args": ["a:lid.tx", 7.0]}));
    write(&dir, "prop.ma", "createNode transform -n \"box\";\n\tsetAttr \".t\" -type \"double3\" 4 5 6 ;\ncreateNode transform -n \"lid\" -p \"box\";\ncreateNode transform -n \"added\" -p \"box\";\n");
    run(&mut s, "file", json!({"lr": "aRN"}));
    assert_eq!(get(&mut s, "a:box.translate"), vec![4.0, 5.0, 6.0], "from the new file");
    assert_eq!(get(&mut s, "a:lid.tx"), vec![7.0], "the edit stays");
    assert!(exists(&mut s, "a:added"));
    let scene = dir.join("shot.ma");
    run(&mut s, "file", json!({"args": [text(&scene)], "rename": text(&scene)}));
    run(&mut s, "file", json!({"save": true}));
    write(&dir, "prop.ma", "createNode transform -n \"box\";\n\tsetAttr \".t\" -type \"double3\" 8 8 8 ;\ncreateNode transform -n \"lid\" -p \"box\";\n");
    let mut t = Session::new();
    open(&mut t, &scene);
    assert_eq!(get(&mut t, "a:box.translate"), vec![8.0, 8.0, 8.0]);
    assert_eq!(get(&mut t, "a:lid.tx"), vec![7.0]);
}

/// A file that references another: nested namespaces, reference nodes, parents, and edits at each level.
#[test]
fn nested_references_stay_nested() {
    let dir = folder("nested");
    write(&dir, "bot.ma", "createNode transform -n \"sphere\";\n");
    write(
        &dir,
        "mid.ma",
        "file -rdi 1 -ns \"bot\" -rfn \"botRN\" \"bot.ma\";\nfile -r -ns \"bot\" -dr 1 -rfn \"botRN\" \"bot.ma\";\ncreateNode transform -n \"cone\";\n\
         createNode reference -n \"botRN\";\n\tsetAttr \".ed\" -type \"dataReferenceEdits\" \n\t\t\"botRN\"\n\t\t\"botRN\" 1\n\t\t2 \"|bot:sphere\" \"translate\" \" -type \\\"double3\\\" 1 1 1\";\nlockNode -l 1 ;\n",
    );
    let mid = text(&dir.join("mid.ma"));
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [mid.clone()], "r": true, "ns": "mid"}));
    assert!(exists(&mut s, "mid:cone"));
    assert!(exists(&mut s, "mid:bot:sphere"));
    assert_eq!(get(&mut s, "mid:bot:sphere.translate"), vec![1.0, 1.0, 1.0], "the edit mid.ma keeps on bot.ma");
    let q = |s: &mut Session, name: &str, flag: &str| run(s, "referenceQuery", json!({"args": [name], flag: true}));
    assert_eq!(q(&mut s, "mid:cone", "rfn"), json!("midRN"));
    assert_eq!(q(&mut s, "mid:bot:sphere", "rfn"), json!("mid:botRN"));
    assert_eq!(q(&mut s, "mid:bot:sphere", "ns"), json!(":mid:bot"));
    assert_eq!(q(&mut s, "mid:bot:sphere", "pns"), json!("mid"));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["mid:botRN"], "rfn": true, "p": true})), json!("midRN"));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["mid:botRN"], "rfn": true, "tr": true})), json!("midRN"));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["midRN"], "rfn": true, "ch": true})), json!(["mid:botRN"]));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["midRN"], "f": true, "ch": true, "shn": true})), json!(["bot.ma"]));

    // An edit from this scene on the inner node, saved under the top reference.
    run(&mut s, "setAttr", json!({"args": ["mid:bot:sphere.ty", 6.0]}));
    let scene = dir.join("top.ma");
    run(&mut s, "file", json!({"args": [text(&scene)], "rename": text(&scene)}));
    run(&mut s, "file", json!({"save": true}));
    let saved = std::fs::read_to_string(&scene).unwrap();
    assert!(saved.contains("\"mid:botRN\" 1"), "{saved}");
    assert!(saved.contains("file -rdi 2 -ns \"bot\" -rfn \"mid:botRN\""), "{saved}");
    assert!(!saved.contains("createNode reference -n \"mid:botRN\""), "only the top reference has a node in the file");
    let mut t = Session::new();
    open(&mut t, &scene);
    assert_eq!(get(&mut t, "mid:bot:sphere.translate"), vec![1.0, 6.0, 1.0]);

    // Unloading the inner one alone keeps the outer.
    run(&mut t, "file", json!({"ur": "mid:botRN"}));
    assert!(!exists(&mut t, "mid:bot:sphere") && exists(&mut t, "mid:cone"));
    run(&mut t, "file", json!({"lr": "mid:botRN"}));
    assert_eq!(get(&mut t, "mid:bot:sphere.translate"), vec![1.0, 6.0, 1.0]);
}

/// The scene's own nodes tied to a reference: parented under, constrained to, keyed.
#[test]
fn the_scene_can_use_referenced_nodes_and_keep_it_through_a_save_and_an_unload() {
    let dir = folder("use");
    let rig = write(&dir, "rig.ma", "createNode transform -n \"ctrl\";\ncreateNode transform -n \"tip\" -p \"ctrl\";\n");
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [rig], "r": true, "ns": "c"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "hat", "parent": "c:tip"}));
    run(&mut s, "createNode", json!({"args": ["transform"], "name": "prop"}));
    run(&mut s, "parentConstraint", json!({"args": ["c:tip", "prop"]}));
    run(&mut s, "setKeyframe", json!({"args": ["c:ctrl.tx"], "t": 1.0, "v": 1.0}));
    run(&mut s, "setKeyframe", json!({"args": ["c:ctrl.tx"], "t": 5.0, "v": 5.0}));
    run(&mut s, "currentTime", json!({"args": [5.0]}));
    assert_eq!(get(&mut s, "prop.tx"), vec![5.0]);

    run(&mut s, "file", json!({"ur": "cRN"}));
    assert!(exists(&mut s, "prop"));
    assert!(exists(&mut s, "ctrl_translateX"), "the keys wait in the scene while it's unloaded");
    let scene = dir.join("shot.ma");
    run(&mut s, "file", json!({"args": [text(&scene)], "rename": text(&scene)}));
    run(&mut s, "file", json!({"save": true}));
    let mut t = Session::new();
    open(&mut t, &scene);
    assert_eq!(run(&mut t, "referenceQuery", json!({"args": ["cRN"], "il": true})), json!(false));
    run(&mut t, "file", json!({"lr": "cRN"}));
    run(&mut t, "currentTime", json!({"args": [5.0]}));
    assert_eq!(get(&mut t, "c:ctrl.tx"), vec![5.0]);
    assert_eq!(get(&mut t, "prop.tx"), vec![5.0]);
    let _ = exists(&mut t, "hat");
}

#[test]
fn edits_can_be_listed_removed_and_retargeted() {
    let dir = folder("edits");
    let rig = write(&dir, "rig.ma", "createNode transform -n \"a\";\ncreateNode transform -n \"b\";\n");
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [rig.clone()], "r": true, "ns": "x"}));
    run(&mut s, "setAttr", json!({"args": ["x:a.tx", 2.0]}));
    run(&mut s, "setAttr", json!({"args": ["x:b.ty", 3.0]}));
    let strings = run(&mut s, "referenceQuery", json!({"args": ["xRN"], "es": true}));
    assert_eq!(strings, json!(["setAttr |x:a.translate -type \"double3\" 2 0 0", "setAttr |x:b.translate -type \"double3\" 0 3 0"]));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["x:a"], "es": true})), json!(["setAttr |x:a.translate -type \"double3\" 2 0 0"]));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["xRN"], "en": true, "ea": true})), json!(["|x:a.translate", "|x:b.translate"]));
    assert!(s.run("referenceEdit", json!({"args": ["xRN"], "r": true})).is_err(), "a reference has to be unloaded first");
    run(&mut s, "file", json!({"ur": "xRN"}));
    run(&mut s, "referenceEdit", json!({"args": ["x:a"], "r": true, "ec": "setAttr"}));
    run(&mut s, "file", json!({"lr": "xRN"}));
    assert_eq!(get(&mut s, "x:a.tx"), vec![0.0]);
    assert_eq!(get(&mut s, "x:b.ty"), vec![3.0]);

    // A renamed node: its edit fails and stays, and can be pointed at the new name.
    run(&mut s, "file", json!({"ur": "xRN"}));
    write(&dir, "rig.ma", "createNode transform -n \"a\";\ncreateNode transform -n \"renamed\";\n");
    run(&mut s, "file", json!({"lr": "xRN"}));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["xRN"], "es": true, "fld": true, "scs": false})), json!(["setAttr |x:b.translate -type \"double3\" 0 3 0"]));
    run(&mut s, "referenceEdit", json!({"args": ["xRN"], "cet": ["x:b", "x:renamed"]}));
    run(&mut s, "referenceEdit", json!({"args": ["xRN"], "afe": true}));
    assert_eq!(get(&mut s, "x:renamed.ty"), vec![3.0]);
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["xRN"], "es": true, "fld": true, "scs": false})), json!([]));

    run(&mut s, "file", json!({"ur": "xRN"}));
    run(&mut s, "file", json!({"cr": "xRN", "ec": "setAttr"}));
    run(&mut s, "file", json!({"lr": "xRN"}));
    assert_eq!(get(&mut s, "x:renamed.ty"), vec![0.0], "file -cr cleans them");
}

#[test]
fn replacing_importing_and_the_load_flags() {
    let dir = folder("replace");
    let one = write(&dir, "one.ma", "createNode transform -n \"box\";\n");
    let two = write(&dir, "two.ma", "createNode transform -n \"box\";\ncreateNode transform -n \"more\";\n");
    let mut s = Session::new();
    assert_eq!(run(&mut s, "file", json!({"args": [one.clone()], "r": true, "ns": "p", "rnn": true})), json!(["p:box"]));
    run(&mut s, "setAttr", json!({"args": ["p:box.tx", 2.0]}));
    run(&mut s, "file", json!({"args": [two.clone()], "lr": "pRN"}));
    assert!(exists(&mut s, "p:more"));
    assert_eq!(get(&mut s, "p:box.tx"), vec![2.0], "edits follow a replaced file");
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["pRN"], "f": true})), json!(two));

    run(&mut s, "file", json!({"args": [two.clone()], "ir": true}));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["p:box"], "inr": true})), json!(false), "imported, it's the scene's");
    assert!(!exists(&mut s, "pRN"));
    run(&mut s, "delete", json!({"args": ["p:box"]}));

    run(&mut s, "file", json!({"args": [one.clone()], "r": true, "gr": true, "gn": "holder", "ns": "g"}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["g:box"], "parent": true})), json!(["holder"]));
    run(&mut s, "file", json!({"args": [one.clone()], "r": true, "ns": "u", "dr": true}));
    assert_eq!(run(&mut s, "file", json!({"args": [one.clone() + "{1}"], "q": true, "dr": true, "queried": ["deferReference"]})), json!(true));
    run(&mut s, "file", json!({"args": [one.clone()], "r": true, "ns": "g", "mnc": true}));
    assert_eq!(run(&mut s, "file", json!({"q": true, "r": true, "shn": true, "queried": ["reference", "shortName"]})), json!(["one.ma", "one.ma", "one.ma"]));
    run(&mut s, "file", json!({"args": [one.clone()], "r": true, "ns": "L", "lck": true}));
    assert!(s.run("setAttr", json!({"args": ["L:box.tx", 2.0]})).is_err(), "a locked reference");
    run(&mut s, "file", json!({"args": [one.clone()], "r": true, "rpr": "pre"}));
    assert!(exists(&mut s, "pre_box"));
    assert_eq!(run(&mut s, "ls", json!({"rn": true, "type": "transform"})).as_array().unwrap().len(), 4);
    assert_eq!(run(&mut s, "lockNode", json!({"args": ["gRN"], "q": true, "lock": true, "queried": ["lock"]})), json!([true]));
}

/// Something missing doesn't stop a scene opening: it stays in the list, unloaded, and keeps its edits for later.
#[test]
fn a_missing_file_stays_listed_and_keeps_its_edits() {
    let dir = folder("missing");
    let scene = write(&dir, "shot.ma", "file -rdi 1 -ns \"r\" -rfn \"rRN\" \"gone.ma\";\nfile -r -ns \"r\" -dr 1 -rfn \"rRN\" \"gone.ma\";\ncreateNode transform -n \"k\";\ncreateNode reference -n \"rRN\";\n\tsetAttr \".ed\" -type \"dataReferenceEdits\" \n\t\t\"rRN\"\n\t\t\"rRN\" 1\n\t\t2 \"|r:box\" \"translateX\" \" 4\";\nlockNode -l 1 ;\n");
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [scene], "open": true, "force": true}));
    assert_eq!(run(&mut s, "referenceQuery", json!({"args": ["rRN"], "il": true})), json!(false));
    assert_eq!(run(&mut s, "file", json!({"q": true, "r": true, "queried": ["reference"]})).as_array().unwrap().len(), 1);
    write(&dir, "gone.ma", "createNode transform -n \"box\";\n");
    run(&mut s, "file", json!({"lr": "rRN"}));
    assert_eq!(get(&mut s, "r:box.tx"), vec![4.0]);
}

/// Reading a file adds to the shared shading group; it used to replace what earlier files put there, leaving those meshes
/// with no material at all.
#[test]
fn referenced_meshes_stay_in_the_shared_shading_group() {
    let dir = folder("shared_group");
    let body = "createNode transform -n \"box\";\ncreateNode mesh -n \"boxShape\" -p \"box\";\ncreateNode polyCube -n \"polyCube1\";\nconnectAttr \"polyCube1.out\" \"boxShape.i\";\nconnectAttr \"boxShape.iog\" \":initialShadingGroup.dsm\" -na;\n";
    let rig = write(&dir, "prop.ma", body);
    let mut s = Session::new();
    run(&mut s, "file", json!({"args": [rig.clone()], "r": true, "ns": "a"}));
    run(&mut s, "file", json!({"args": [rig], "r": true, "ns": "b"}));
    run(&mut s, "polyCube", json!({"name": "own"}));
    let held = run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})).to_string();
    for name in ["a:boxShape", "b:boxShape", "ownShape"] {
        assert!(held.contains(name), "{name} in {held}");
    }
    let imported = write(&dir, "more.ma", "createNode transform -n \"extra\";\ncreateNode mesh -n \"extraShape\" -p \"extra\";\ncreateNode polyCube -n \"polyCube2\";\nconnectAttr \"polyCube2.out\" \"extraShape.i\";\nconnectAttr \"extraShape.iog\" \":initialShadingGroup.dsm\" -na;\n");
    run(&mut s, "file", json!({"args": [imported], "import": true}));
    let held = run(&mut s, "sets", json!({"args": ["initialShadingGroup"], "q": true})).to_string();
    assert!(held.contains("ownShape") && held.contains("extraShape") && held.contains("a:boxShape"), "{held}");
}
