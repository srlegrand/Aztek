//! The basics suite: everyday calls with the results the reference gives, from the parity review of 08/11/2026.
//!
//! Each test states the reference's result. Where no capture from the reference backs it yet, the line carries
//! `// unverified:` and the question is in tools/reference_capture/WANTED.md. Tests marked `#[ignore]` name the PLAN.md
//! item that fixes them; they fail today and lose the mark when the item lands.

use aztek_host::Core;
use serde_json::{json, Value};

fn core() -> Core {
    let mut c = Core::new();
    aztek_scene::cameras::make_defaults(&mut c.s.graph);
    c
}

/// Runs MEL that must not error and gives the last statement's value.
fn mel(c: &mut Core, src: &str) -> Value {
    let v = c.mel_checked(src);
    let bad: Vec<String> = c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap_or("").to_string()).collect();
    match v {
        Ok(v) if bad.is_empty() => v,
        Ok(_) => panic!("{src}: {bad:?}"),
        Err(e) => panic!("{src}: {e} {bad:?}"),
    }
}

/// Runs MEL that must error.
fn fails(c: &mut Core, src: &str) {
    let v = c.mel_checked(src);
    let bad = c.take_log().as_array().unwrap().iter().any(|l| l["kind"] == 2);
    assert!(v.is_err() || bad, "{src} should fail, gave {v:?}");
}

fn nums(v: &Value) -> Vec<f64> {
    match v {
        Value::Number(n) => vec![n.as_f64().unwrap()],
        Value::Array(a) => a.iter().flat_map(nums).collect(),
        Value::Bool(b) => vec![*b as i32 as f64],
        _ => panic!("{v} is not numbers"),
    }
}

#[track_caller]
fn near(v: Value, want: &[f64]) {
    let got = nums(&v);
    assert_eq!(got.len(), want.len(), "{v} against {want:?}");
    for (g, w) in got.iter().zip(want) {
        assert!((g - w).abs() < 1e-4, "{v} against {want:?}");
    }
}

// Transforms

#[test]
fn move_rotate_scale() {
    let mut c = core();
    assert_eq!(mel(&mut c, "polyCube;"), json!(["pCube1", "polyCube1"]));
    mel(&mut c, "move 1 2 3;");
    near(mel(&mut c, "getAttr pCube1.translate;"), &[1.0, 2.0, 3.0]);
    mel(&mut c, "move -r 1 0 0;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[2.0]);
    mel(&mut c, "rotate 0 90 0;");
    near(mel(&mut c, "getAttr pCube1.ry;"), &[90.0]);
    mel(&mut c, "rotate -r 0 10 0;");
    near(mel(&mut c, "getAttr pCube1.ry;"), &[100.0]);
    mel(&mut c, "scale 2 3 4;");
    near(mel(&mut c, "getAttr pCube1.scale;"), &[2.0, 3.0, 4.0]);
    near(mel(&mut c, "xform -q -ws -t pCube1;"), &[2.0, 2.0, 3.0]);
    mel(&mut c, "xform -ws -t 0 0 0 pCube1;");
    near(mel(&mut c, "getAttr pCube1.t;"), &[0.0, 0.0, 0.0]);
}

#[test]
fn world_space_under_a_parent() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube; parent pCube1 pCube2; move 0 5 0 pCube2;");
    near(mel(&mut c, "xform -q -ws -t pCube1;"), &[0.0, 5.0, 0.0]);
    near(mel(&mut c, "getAttr pCube1.t;"), &[0.0, 0.0, 0.0]);
    mel(&mut c, "move -ws 1 1 1 pCube1;");
    near(mel(&mut c, "getAttr pCube1.t;"), &[1.0, -4.0, 1.0]);
}

// Pivots

#[test]
fn centre_pivot() {
    let mut c = core();
    mel(&mut c, "polyCube; move -r 2 0 0 pCube1.vtx[*];");
    mel(&mut c, "xform -cp pCube1;");
    near(mel(&mut c, "xform -q -ws -rp pCube1;"), &[2.0, 0.0, 0.0]);
    near(mel(&mut c, "xform -q -ws -sp pCube1;"), &[2.0, 0.0, 0.0]);
}

#[test]
#[ignore = "PLAN 1.6: move on pivots takes -a"]
fn move_pivots_absolute() {
    let mut c = core();
    mel(&mut c, "polyCube; move 3 0 0;");
    mel(&mut c, "move -a 0 0 0 pCube1.scalePivot pCube1.rotatePivot;");
    near(mel(&mut c, "xform -q -ws -rp pCube1;"), &[0.0, 0.0, 0.0]);
    near(mel(&mut c, "xform -q -ws -sp pCube1;"), &[0.0, 0.0, 0.0]);
    mel(&mut c, "move -r 1 0 0 pCube1.rotatePivot;");
    near(mel(&mut c, "xform -q -ws -rp pCube1;"), &[1.0, 0.0, 0.0]);
    mel(&mut c, "xform -ws -rp 2 0 0 pCube1;");
    near(mel(&mut c, "xform -q -ws -rp pCube1;"), &[2.0, 0.0, 0.0]);
}

// Freeze and reset

#[test]
fn freeze_and_reset() {
    let mut c = core();
    mel(&mut c, "polyCube; move 1 2 3; rotate 0 45 0; scale 2 2 2;");
    let before = mel(&mut c, "xform -q -ws -t pCube1.vtx[0];");
    mel(&mut c, "makeIdentity -apply true -t 1 -r 1 -s 1 -n 0 pCube1;");
    near(mel(&mut c, "getAttr pCube1.t;"), &[0.0, 0.0, 0.0]);
    near(mel(&mut c, "getAttr pCube1.r;"), &[0.0, 0.0, 0.0]);
    near(mel(&mut c, "getAttr pCube1.s;"), &[1.0, 1.0, 1.0]);
    near(mel(&mut c, "xform -q -ws -t pCube1.vtx[0];"), &nums(&before));

    mel(&mut c, "polyCube; move 1 2 3; rotate 0 45 0;");
    mel(&mut c, "makeIdentity -apply false pCube2;");
    near(mel(&mut c, "getAttr pCube2.t;"), &[0.0, 0.0, 0.0]);
    near(mel(&mut c, "getAttr pCube2.r;"), &[0.0, 0.0, 0.0]);
    near(mel(&mut c, "xform -q -ws -t pCube2.vtx[0];"), &[-0.5, -0.5, 0.5]);
}

// Duplicate and instance

#[test]
fn duplicate_and_instance() {
    let mut c = core();
    mel(&mut c, "polyCube; move 1 0 0;");
    assert_eq!(mel(&mut c, "duplicate pCube1;"), json!(["pCube2"]));
    near(mel(&mut c, "getAttr pCube2.tx;"), &[1.0]);
    assert_eq!(mel(&mut c, "instance pCube1;"), json!(["pCube3"]));
    assert_eq!(mel(&mut c, "listRelatives -s pCube3;"), json!(["pCube3|pCubeShape1"])); // unverified: an instanced shape is named by its path
    // Duplicate with transform repeats the last duplicate's offset.
    mel(&mut c, "select -r pCube1; duplicate; move -r 1 0 0;");
    mel(&mut c, "duplicate -st;");
    near(mel(&mut c, "getAttr pCube5.tx;"), &[3.0]); // unverified: duplicate -st after a moved duplicate
}

#[test]
#[ignore = "PLAN 1.14: an instance is one shape under two parents"]
fn instances_share_one_shape() {
    let mut c = core();
    mel(&mut c, "polyCube; instance pCube1;");
    assert_eq!(mel(&mut c, "listRelatives -ap pCube1|pCubeShape1;"), json!(["pCube1", "pCube2"]));
    assert_eq!(mel(&mut c, "listRelatives -f -s pCube2;"), json!(["|pCube2|pCubeShape1"]));
    mel(&mut c, "setAttr pCube1|pCubeShape1.castsShadows 0;");
    near(mel(&mut c, "getAttr pCube2|pCubeShape1.castsShadows;"), &[0.0]);
}

#[test]
#[ignore = "PLAN 1.5: a duplicate's shape takes the next free shape name"]
fn duplicate_names_the_shape() {
    let mut c = core();
    mel(&mut c, "polyCube; duplicate pCube1;");
    assert_eq!(mel(&mut c, "listRelatives -s pCube2;"), json!(["pCubeShape2"]));
    assert_eq!(mel(&mut c, "listRelatives -ap pCubeShape1;"), json!(["pCube1"]));
}

// Group and parent

#[test]
#[ignore = "PLAN 1.2: group names are group1, group2"]
fn group_names() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube;");
    assert_eq!(mel(&mut c, "group pCube1 pCube2;"), json!("group1"));
    assert_eq!(mel(&mut c, "group -em;"), json!("group2"));
    assert_eq!(mel(&mut c, "group -n rig -em;"), json!("rig"));
    assert_eq!(mel(&mut c, "listRelatives -c group1;"), json!(["pCube1", "pCube2"]));
}

#[test]
#[ignore = "PLAN 1.3: group goes under the shared parent, pivot at the origin"]
fn group_under_the_shared_parent() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube; group -em -n top; parent pCube1 pCube2 top; move 5 0 0 top;");
    let g = mel(&mut c, "group pCube1 pCube2;");
    assert_eq!(mel(&mut c, &format!("listRelatives -p {};", g.as_str().unwrap())), json!(["top"]));
    near(mel(&mut c, &format!("xform -q -ws -rp {};", g.as_str().unwrap())), &[0.0, 0.0, 0.0]);
    near(mel(&mut c, "xform -q -ws -t pCube1;"), &[5.0, 0.0, 0.0]);
}

#[test]
fn parent_and_unparent() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube; move 0 2 0 pCube2;");
    assert_eq!(mel(&mut c, "parent pCube1 pCube2;"), json!(["pCube1"]));
    assert_eq!(mel(&mut c, "listRelatives -p pCube1;"), json!(["pCube2"]));
    near(mel(&mut c, "getAttr pCube1.ty;"), &[-2.0]);
    assert_eq!(mel(&mut c, "parent -w pCube1;"), json!(["pCube1"]));
    assert_eq!(mel(&mut c, "listRelatives -p pCube1;"), json!([])); // MEL gives an empty array; Python gives None
    near(mel(&mut c, "getAttr pCube1.ty;"), &[0.0]);
}

// Names

#[test]
fn rename_plain() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    assert_eq!(mel(&mut c, "rename pCube1 box;"), json!("box"));
    assert_eq!(mel(&mut c, "listRelatives -s box;"), json!(["boxShape"])); // unverified: rename renames the shape
    mel(&mut c, "polyCube;");
    assert_eq!(mel(&mut c, "rename pCube1 box;"), json!("box1"));
}

#[test]
#[ignore = "PLAN 1.4: # in rename is the next free number"]
fn rename_hashes() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube; polyCube;");
    assert_eq!(mel(&mut c, "rename pCube1 \"thing#\";"), json!("thing1"));
    assert_eq!(mel(&mut c, "rename pCube2 \"thing#\";"), json!("thing2"));
    assert_eq!(mel(&mut c, "rename pCube3 \"part###\";"), json!("part001"));
}

#[test]
#[ignore = "PLAN 1.5: duplicates keep the number's padding"]
fn duplicate_keeps_padding() {
    let mut c = core();
    mel(&mut c, "polyCube -n arm_01;");
    assert_eq!(mel(&mut c, "duplicate arm_01;"), json!(["arm_02"])); // unverified: padding kept
    mel(&mut c, "polyCube -n ctl_L_009;");
    assert_eq!(mel(&mut c, "duplicate ctl_L_009;"), json!(["ctl_L_010"])); // unverified: padding kept past a carry
}

// Undo

#[test]
fn undo_and_redo() {
    let mut c = core();
    mel(&mut c, "polyCube; move 1 0 0;");
    mel(&mut c, "undo;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[0.0]);
    mel(&mut c, "redo;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[1.0]);
    mel(&mut c, "undoInfo -openChunk; move 2 0 0; move 3 0 0; undoInfo -closeChunk;");
    mel(&mut c, "undo;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[1.0]);
    mel(&mut c, "undo; undo;");
    assert_eq!(mel(&mut c, "objExists pCube1;"), json!(false));
    assert_eq!(mel(&mut c, "undoInfo -q -st;"), json!(true));
}

// Units

#[test]
fn linear_units_convert() {
    let mut c = core();
    mel(&mut c, "polyCube; move 100 0 0;");
    mel(&mut c, "currentUnit -l m;");
    assert_eq!(mel(&mut c, "currentUnit -q -l;"), json!("m"));
    near(mel(&mut c, "getAttr pCube1.tx;"), &[1.0]);
    mel(&mut c, "setAttr pCube1.ty 2;");
    near(mel(&mut c, "xform -q -ws -t pCube1;"), &[1.0, 2.0, 0.0]);
    mel(&mut c, "move -r 0.5 0 0;");
    mel(&mut c, "currentUnit -l mm;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[1500.0]);
    mel(&mut c, "currentUnit -l cm;");
    near(mel(&mut c, "getAttr pCube1.t;"), &[150.0, 200.0, 0.0]);
    mel(&mut c, "currentUnit -l in;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[150.0 / 2.54]);
}

#[test]
fn angular_units_convert() {
    let mut c = core();
    mel(&mut c, "polyCube; rotate 0 90 0;");
    mel(&mut c, "currentUnit -a rad;");
    near(mel(&mut c, "getAttr pCube1.ry;"), &[std::f64::consts::FRAC_PI_2]);
    mel(&mut c, "setAttr pCube1.rx 1.0;");
    mel(&mut c, "currentUnit -a deg;");
    near(mel(&mut c, "getAttr pCube1.rx;"), &[1.0f64.to_degrees()]);
}

#[test]
fn time_units_convert() {
    let mut c = core();
    mel(&mut c, "currentUnit -t film; currentTime 24;");
    mel(&mut c, "currentUnit -t pal;");
    near(mel(&mut c, "currentTime -q;"), &[25.0]); // unverified: current time keeps its seconds across a unit change
    mel(&mut c, "currentUnit -t ntsc;");
    near(mel(&mut c, "currentTime -q;"), &[30.0]);
}

// Keys

#[test]
fn keys_and_tangents() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    mel(&mut c, "setKeyframe -t 1 -v 0 -at tx pCube1; setKeyframe -t 10 -v 10 -at tx pCube1;");
    assert_eq!(mel(&mut c, "keyframe -q -kc pCube1.tx;"), json!(2));
    near(mel(&mut c, "keyframe -q -tc pCube1.tx;"), &[1.0, 10.0]);
    near(mel(&mut c, "keyframe -q -vc pCube1.tx;"), &[0.0, 10.0]);
    assert_eq!(mel(&mut c, "keyTangent -q -itt -t 1 pCube1.tx;"), json!(["auto"]));
    assert_eq!(mel(&mut c, "keyTangent -q -ott -t 1 pCube1.tx;"), json!(["auto"]));
    near(mel(&mut c, "getAttr -t 1 pCube1.tx;"), &[0.0]);
    near(mel(&mut c, "getAttr -t 10 pCube1.tx;"), &[10.0]);
    assert_eq!(mel(&mut c, "listConnections pCube1.tx;"), json!(["pCube1_translateX"]));
    mel(&mut c, "cutKey -t 10 pCube1.tx;");
    assert_eq!(mel(&mut c, "keyframe -q -kc pCube1.tx;"), json!(1));
}

// Files

#[test]
fn save_and_open() {
    let dir = std::env::temp_dir().join(format!("aztek_basics_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scene.ma").to_string_lossy().replace('\\', "/");
    let mut c = core();
    mel(&mut c, "polyCube; move 1 2 3; setKeyframe -t 1 -v 0 -at ry pCube1; setKeyframe -t 5 -v 45 -at ry pCube1;");
    mel(&mut c, &format!("file -rename \"{path}\"; file -s -type \"mayaAscii\";"));
    mel(&mut c, "file -new -f;");
    assert_eq!(mel(&mut c, "objExists pCube1;"), json!(false));
    mel(&mut c, &format!("file -o -f \"{path}\";"));
    near(mel(&mut c, "getAttr pCube1.t;"), &[1.0, 2.0, 3.0]);
    near(mel(&mut c, "getAttr -t 5 pCube1.ry;"), &[45.0]);
    assert_eq!(mel(&mut c, "file -q -sn;"), json!(path));
    std::fs::remove_dir_all(dir).ok();
}

// Selection

#[test]
fn select_add_toggle_deselect() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube; polyCube;");
    mel(&mut c, "select -r pCube1;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCube1"]));
    mel(&mut c, "select -add pCube2;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCube1", "pCube2"]));
    mel(&mut c, "select -tgl pCube1 pCube3;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCube2", "pCube3"]));
    mel(&mut c, "select -d pCube2;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCube3"]));
    mel(&mut c, "select -cl;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!([]));
}

#[test]
#[ignore = "PLAN 1.7: InvertSelection inverts"]
fn invert_selection() {
    let mut c = core();
    mel(&mut c, "polyCube; polyCube; polyCube; select -r pCube1;");
    mel(&mut c, "InvertSelection;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCube2", "pCube3"]));
    mel(&mut c, "select -r pCube1.f[0:1]; selectMode -co; selectType -pf 1;");
    mel(&mut c, "InvertSelection;");
    assert_eq!(mel(&mut c, "ls -sl;"), json!(["pCube1.f[2:5]"])); // unverified: component invert stays on the object
}

// Attributes

#[test]
fn add_lock_delete_attributes() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    mel(&mut c, "addAttr -ln speed -at double -dv 2 -k 1 pCube1;");
    near(mel(&mut c, "getAttr pCube1.speed;"), &[2.0]);
    assert_eq!(mel(&mut c, "getAttr -k pCube1.speed;"), json!(true));
    mel(&mut c, "addAttr -ln mode -at enum -en \"off:on:auto\" pCube1;");
    assert_eq!(mel(&mut c, "getAttr -as pCube1.mode;"), json!("off"));
    mel(&mut c, "setAttr pCube1.mode 2;");
    assert_eq!(mel(&mut c, "getAttr -as pCube1.mode;"), json!("auto"));
    mel(&mut c, "setAttr -l true pCube1.tx;");
    assert_eq!(mel(&mut c, "getAttr -l pCube1.tx;"), json!(true));
    fails(&mut c, "setAttr pCube1.tx 5;");
    near(mel(&mut c, "getAttr pCube1.tx;"), &[0.0]);
    mel(&mut c, "deleteAttr pCube1.speed;");
    assert_eq!(mel(&mut c, "attributeQuery -n pCube1 -ex speed;"), json!(false));
}

// Hotkeys

#[test]
#[ignore = "PLAN 1.8: hotkey -q reads the key from -k"]
fn hotkey_query() {
    let mut c = core();
    mel(&mut c, "polyCube; select -r pCube1;");
    mel(&mut c, "nameCommand -ann \"Make a cube\" -c \"polyCube\" makeCubeNameCommand;");
    mel(&mut c, "hotkey -k \"F12\" -n \"makeCubeNameCommand\";");
    assert_eq!(mel(&mut c, "hotkey -q -k \"F12\" -n;"), json!("makeCubeNameCommand"));
}

// polyEvaluate

#[test]
#[ignore = "PLAN 1.9: polyEvaluate -e is edges, not edit"]
fn poly_evaluate_counts() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    assert_eq!(mel(&mut c, "polyEvaluate -v pCube1;"), json!(8));
    assert_eq!(mel(&mut c, "polyEvaluate -f pCube1;"), json!(6));
    assert_eq!(mel(&mut c, "polyEvaluate -e pCube1;"), json!(12));
    assert_eq!(mel(&mut c, "polyEvaluate -t pCube1;"), json!(12));
    assert_eq!(mel(&mut c, "polyEvaluate -uv pCube1;"), json!(14));
}

#[test]
#[ignore = "PLAN 1.11: polyEvaluate -uvShell"]
fn poly_evaluate_uv_shells() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    assert_eq!(mel(&mut c, "polyEvaluate -uvShell pCube1;"), json!(1));
}

// Tests that pinned a divergence

#[test]
fn bevel_node_name() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    assert_eq!(mel(&mut c, "polyBevel pCube1.e[0];"), json!(["polyBevel1"]));
}

#[test]
#[ignore = "PLAN 1.10: a cycle through two nodes connects"]
fn cycles_connect() {
    let mut c = core();
    mel(&mut c, "polyCube -n src; polyCube -n dst;");
    mel(&mut c, "connectAttr src.t dst.r;");
    mel(&mut c, "connectAttr dst.r src.s;"); // unverified: the reference connects and warns about the cycle at evaluation
    assert_eq!(mel(&mut c, "isConnected dst.r src.s;"), json!(true));
}

// Smooth preview

#[test]
#[ignore = "PLAN 1.12: smooth preview is displaySmoothMesh"]
fn smooth_preview_attributes() {
    let mut c = core();
    mel(&mut c, "polyCube;");
    near(mel(&mut c, "getAttr pCubeShape1.displaySmoothMesh;"), &[0.0]);
    near(mel(&mut c, "getAttr pCubeShape1.smoothLevel;"), &[2.0]);
    mel(&mut c, "displaySmoothness -po 3 pCube1;");
    near(mel(&mut c, "getAttr pCubeShape1.displaySmoothMesh;"), &[2.0]);
    near(mel(&mut c, "displaySmoothness -q -po pCube1;"), &[3.0]); // unverified: query answers per object as an array
    mel(&mut c, "setAttr pCubeShape1.displaySmoothMesh 0;");
    near(mel(&mut c, "displaySmoothness -q -po pCube1;"), &[1.0]);
}

// Shading

#[test]
#[ignore = "PLAN 1.13: shadingNode -isColorManaged"]
fn shading_node_colour_managed() {
    let mut c = core();
    assert_eq!(mel(&mut c, "shadingNode -asTexture -isColorManaged file;"), json!("file1"));
    mel(&mut c, "shadingNode -asShader lambert -n m1; shadingNode -asTexture checker -n ch; connectAttr ch.outColor m1.color;");
    assert_eq!(mel(&mut c, "hyperShade -listUpstreamShaderNodes m1;"), json!(["ch"])); // unverified: return shape
}
