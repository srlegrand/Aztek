//! Unit u24: grabColor, gradientControl, gradientControlNoAttr, headsUpDisplay, helpLine; and absolute component moves,
//! attributeQuery on multi compounds, getAttr and setAttr on CVs and single axes of points.

use aztek_host::Core;
use serde_json::{json, Value};

/// The heads up display's records are the application's, not a core's: tests here that idle take turns.
static TURN: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn turn() -> std::sync::MutexGuard<'static, ()> {
    TURN.lock().unwrap_or_else(|e| e.into_inner())
}

fn ok(c: &mut Core, src: &str) -> Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap()).collect()).unwrap_or_else(|| vec![v.as_f64().unwrap()])
}

fn close(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-6)
}

#[test]
fn grab_color_returns_rgb_hsv_and_alpha() {
    let _t = turn();
    let mut c = Core::new();
    // No shell to press in: the pixel reads as opaque black.
    assert_eq!(nums(&ok(&mut c, "grabColor;")).len(), 3);
    assert_eq!(nums(&ok(&mut c, "grabColor -rgbValue -a;")).len(), 4);
    assert_eq!(nums(&ok(&mut c, "grabColor -a;"))[3], 1.0);
    assert_eq!(nums(&ok(&mut c, "grabColor -hsv;")).len(), 3);
}

#[test]
fn heads_up_display_blocks_triggers_and_layout() {
    let _t = turn();
    let mut c = Core::new();
    ok(&mut c, "global proc float[] u24Pos() { return {1.5, 2.25, 3.0}; }");
    ok(&mut c, "polyCube -n u24Box; select -cl;");
    let id = ok(&mut c, "headsUpDisplay -s 6 -b 0 -bs \"medium\" -l \"Position\" -lfs \"large\" -c \"u24Pos\" -ev \"SelectionChanged\" -nc \"attributeChange\" u24Hud;");
    assert!(id.as_i64().unwrap() > 0);
    // Only one HUD a block.
    assert!(c.mel_checked("headsUpDisplay -s 6 -b 0 -l \"x\" u24Other;").is_err());
    assert!(c.mel_checked("headsUpDisplay -s 6 -b 1 -l \"x\" u24Hud;").is_err(), "names are unique");
    assert_eq!(ok(&mut c, "headsUpDisplay -nfb 6;"), json!(1));
    assert_eq!(ok(&mut c, "headsUpDisplay -lob 6;"), json!(0));
    assert_eq!(ok(&mut c, "headsUpDisplay -lob 8;"), json!(-1));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -section u24Hud;"), json!(6));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -blockSize u24Hud;"), json!("medium"));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -label u24Hud;"), json!("Position"));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -padding u24Hud;"), json!(15));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -blockAlignment u24Hud;"), json!("left"));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -event u24Hud;"), json!("SelectionChanged"));
    assert_eq!(ok(&mut c, "headsUpDisplay -ex u24Hud;"), json!(true));
    assert!(ok(&mut c, "headsUpDisplay -lh;").as_array().unwrap().contains(&json!("u24Hud")));
    // The command ran once at creation; a selection change runs it again.
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -sr u24Hud;"), json!([1.5, 2.25, 3.0]));
    ok(&mut c, "global proc float[] u24Pos() { return {7.0}; }");
    ok(&mut c, "select -r u24Box;");
    c.idle();
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -sr u24Hud;"), json!([7.0]));
    // The display's watch is no script job anyone sees.
    let jobs = ok(&mut c, "scriptJob -lj;");
    assert!(!jobs.to_string().contains("u24Hud"), "{jobs}");
    // -refresh runs it now.
    ok(&mut c, "global proc float[] u24Pos() { return {8.0}; }");
    ok(&mut c, "headsUpDisplay -r u24Hud;");
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -sr u24Hud;"), json!([8.0]));
    // -nodeChanges: a change on the selected node, the selection the same, runs it and is recorded until reset.
    c.idle();
    c.idle();
    ok(&mut c, "global proc float[] u24Pos() { return {9.0}; }");
    ok(&mut c, "setAttr u24Box.tx 4;");
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -sr u24Hud;"), json!([9.0]));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -nc u24Hud;"), json!(["attributeChange", "1"]));
    ok(&mut c, "headsUpDisplay -e -rnc \"attributeChange\" u24Hud;");
    assert_eq!(ok(&mut c, "headsUpDisplay -q -nc u24Hud;"), json!(["attributeChange", "0"]));
    // Edits: formatting, the place, the name.
    ok(&mut c, "headsUpDisplay -e -ba \"right\" -dp 3 -dw 50 -lw 80 -da \"right\" -dfs \"large\" -p 4 u24Hud;");
    assert_eq!(ok(&mut c, "headsUpDisplay -q -ba u24Hud;"), json!("right"));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -dp u24Hud;"), json!(3));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -dw u24Hud;"), json!(50));
    assert!(c.mel_checked("headsUpDisplay -e -dp 9 u24Hud;").is_err());
    assert!(c.mel_checked("headsUpDisplay -e -bs \"huge\" u24Hud;").is_err());
    ok(&mut c, "headsUpDisplay -e -b 2 u24Hud;");
    assert_eq!(ok(&mut c, "headsUpDisplay -nfb 6;"), json!(0));
    ok(&mut c, "headsUpDisplay -e -n u24Renamed u24Hud;");
    assert_eq!(ok(&mut c, "headsUpDisplay -ex u24Hud;"), json!(false));
    assert_eq!(ok(&mut c, "headsUpDisplay -q -label u24Renamed;"), json!("Position"));
    // Presets, and the flags they can't go with.
    assert!(ok(&mut c, "headsUpDisplay -lp;").as_array().unwrap().contains(&json!("polyVerts")));
    assert!(c.mel_checked("headsUpDisplay -s 7 -b 0 -pre \"cameraNames\" -c \"u24Pos\" u24Bad;").is_err());
    assert!(c.mel_checked("headsUpDisplay -s 7 -b 0 -pre \"nonsense\" u24Bad;").is_err());
    let cam = ok(&mut c, "headsUpDisplay -s 7 -b 0 -ba \"center\" -dw 50 -pre \"cameraNames\" u24Cam;").as_i64().unwrap();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -preset u24Cam;"), json!("cameraNames"));
    // Lists of what triggers there are.
    assert!(ok(&mut c, "headsUpDisplay -le;").as_array().unwrap().contains(&json!("SelectionChanged")));
    assert!(ok(&mut c, "headsUpDisplay -lc;").as_array().unwrap().contains(&json!("SomethingSelected")));
    assert_eq!(ok(&mut c, "headsUpDisplay -lnc;"), json!(["attributeChange", "connectionChange", "instanceChange"]));
    assert!(c.mel_checked("headsUpDisplay -s 7 -b 1 -c \"u24Pos\" -ev \"NoSuchEvent\" u24Bad;").is_err());
    // An attribute trigger: the attribute changing runs it; the node going takes the display with it.
    ok(&mut c, "global proc float u24Tx() { return `getAttr u24Box.tx`; }");
    ok(&mut c, "headsUpDisplay -s 8 -b 0 -c \"u24Tx\" -ac \"u24Box.tx\" u24Attr;");
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -sr u24Attr;"), json!(4.0));
    ok(&mut c, "setAttr u24Box.tx 6;");
    c.idle();
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -q -sr u24Attr;"), json!(6.0));
    ok(&mut c, "delete u24Box;");
    c.idle();
    c.idle();
    assert_eq!(ok(&mut c, "headsUpDisplay -ex u24Attr;"), json!(false));
    // -attachToRefresh runs it every time the views draw: the blocks the views get.
    ok(&mut c, "headsUpDisplay -s 8 -b 1 -l \"Refresh\" -c \"u24Pos\" -atr u24Refresh;");
    assert_eq!(ok(&mut c, "headsUpDisplay -q -atr u24Refresh;"), json!(true));
    let blocks = aztek_host::hud::blocks(&mut c);
    let b = blocks.iter().find(|b| b.name == "u24Refresh").unwrap();
    assert_eq!((b.section, b.block, b.label.as_str()), (8, 1, "Refresh"));
    assert_eq!(b.data, vec!["9.00".to_string()]);
    let r = blocks.iter().find(|b| b.name == "u24Renamed").unwrap();
    assert_eq!((r.height, r.align.as_str(), r.data_right, r.data_large), (35.0, "right", true, true));
    // Options, the grid and the layout as a whole.
    assert_eq!(ok(&mut c, "headsUpDisplay -q -getOption \"smpPolyCount\";"), json!("cage"));
    ok(&mut c, "headsUpDisplay -e -setOption \"smpPolyCount\" \"smp\";");
    assert_eq!(ok(&mut c, "headsUpDisplay -q -getOption \"smpPolyCount\";"), json!("smp"));
    ok(&mut c, "headsUpDisplay -e -setOption \"smpPolyCount\" \"cage\";");
    let grid = ok(&mut c, "headsUpDisplay -q -sg;");
    ok(&mut c, "headsUpDisplay -sg;");
    assert_ne!(ok(&mut c, "headsUpDisplay -q -sg;"), grid);
    ok(&mut c, "headsUpDisplay -sg -gco 13;");
    assert_eq!(ok(&mut c, "headsUpDisplay -q -gco;"), json!(13));
    ok(&mut c, "headsUpDisplay -lv 0;");
    assert_eq!(ok(&mut c, "headsUpDisplay -q -lv;"), json!(false));
    assert!(!aztek_host::hud::layout_visible());
    ok(&mut c, "headsUpDisplay -lv 1;");
    // Hidden displays aren't drawn.
    ok(&mut c, "headsUpDisplay -e -vis 0 u24Refresh;");
    assert!(!aztek_host::hud::blocks(&mut c).iter().any(|b| b.name == "u24Refresh"));
    // Removing: by name (gives the ID), by ID (gives the name) and by place.
    let rid = ok(&mut c, "headsUpDisplay -q -section u24Renamed;");
    assert_eq!(rid, json!(6));
    assert_eq!(ok(&mut c, "headsUpDisplay -rem u24Renamed;"), id);
    assert_eq!(ok(&mut c, &format!("headsUpDisplay -rid {cam};")), json!("u24Cam"));
    assert_eq!(ok(&mut c, "headsUpDisplay -rp 8 1;"), json!([8, 1]));
    assert_eq!(ok(&mut c, "headsUpDisplay -ex u24Refresh;"), json!(false));
    // Built-in displays still only show and hide.
    ok(&mut c, "headsUpDisplay -vis 1 polyCount;");
    assert!(aztek_host::ui_cmds::hud_on("polyCount"));
    ok(&mut c, "headsUpDisplay -vis 0 polyCount;");
    assert!(!aztek_host::ui_cmds::hud_on("polyCount"));
}

#[test]
fn gradient_control_no_attr_keeps_its_ramp() {
    let _t = turn();
    let mut c = Core::new();
    ok(&mut c, "optionVar -sv u24Falloff \"1,0,1,0,1,1\";");
    let path = ok(&mut c, "window u24W; columnLayout; gradientControlNoAttr -h 90 -ov u24Falloff u24Ramp;");
    let path = path.as_str().unwrap().to_string();
    assert!(path.ends_with("u24Ramp"));
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -as u24Ramp;"), json!("1,0,1,0,1,1"));
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -ov u24Ramp;"), json!("u24Falloff"));
    assert!(close(&nums(&ok(&mut c, "gradientControlNoAttr -q -vap 0.25 u24Ramp;")), &[0.75]));
    // Entries added: a curve takes the colour's average.
    ok(&mut c, "gradientControlNoAttr -e -ae 0.5 0.3 0.6 0.9 2 u24Ramp;");
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -ck u24Ramp;"), json!(1));
    assert!(close(&nums(&ok(&mut c, "gradientControlNoAttr -q -cvv u24Ramp;")), &[0.6]));
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -civ u24Ramp;"), json!(2));
    assert_eq!(ok(&mut c, "optionVar -q u24Falloff;"), json!("1,0,1,0.6,0.5,2,0,1,1"));
    ok(&mut c, "gradientControlNoAttr -e -ck 0 -cvv 0.8 -civ 0 u24Ramp;");
    assert!(close(&nums(&ok(&mut c, "gradientControlNoAttr -q -vap 0.25 u24Ramp;")), &[0.8]));
    assert!(c.mel_checked("gradientControlNoAttr -e -ck 7 u24Ramp;").is_err());
    assert!(c.mel_checked("gradientControlNoAttr -e -clv 1 0 0 u24Ramp;").is_err(), "a curve has no colours");
    // The optionVar changed elsewhere: the control follows.
    ok(&mut c, "optionVar -sv u24Falloff \"0,0,1,1,1,1\";");
    c.idle();
    assert!(close(&nums(&ok(&mut c, "gradientControlNoAttr -q -vap 0.5 u24Ramp;")), &[0.5]));
    // A colour ramp.
    ok(&mut c, "gradientControlNoAttr -rac 1 u24Colors;");
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -rac u24Colors;"), json!(true));
    ok(&mut c, "gradientControlNoAttr -e -as \"1,0,0,0,1,0,0,1,1,1\" u24Colors;");
    assert!(close(&nums(&ok(&mut c, "gradientControlNoAttr -q -vap 0.5 u24Colors;")), &[0.5, 0.0, 0.5]));
    ok(&mut c, "gradientControlNoAttr -e -ck 1 -clv 0 1 0 u24Colors;");
    assert!(close(&nums(&ok(&mut c, "gradientControlNoAttr -q -clv u24Colors;")), &[0.0, 1.0, 0.0]));
    // Settings read back.
    ok(&mut c, "gradientControlNoAttr -e -ro 1 -snc 1 -dki 1 -hlm \"hover\" u24Colors;");
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -ro u24Colors;"), json!(true));
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -snc u24Colors;"), json!(true));
    assert_eq!(ok(&mut c, "gradientControlNoAttr -q -hlm u24Colors;"), json!("hover"));
    assert!(c.mel_checked("gradientControlNoAttr -e -hlm \"blink\" u24Colors;").is_err());
    assert_eq!(ok(&mut c, "gradientControlNoAttr -ex u24Colors;"), json!(true));
    // What the user does in the shell: the ramp is written back and the change command runs.
    ok(&mut c, "global int $u24Changed = 0; gradientControlNoAttr -e -cc \"$u24Changed++\" u24Ramp;");
    c.ui_event(&json!({"path": path, "set": {"asString": "0.2,0,1,0.9,1,1", "currentKey": 1}, "fire": ["__rampChanged"], "args": []}));
    assert_eq!(ok(&mut c, "optionVar -q u24Falloff;"), json!("0.2,0,1,0.9,1,1"));
    assert_eq!(ok(&mut c, "global int $u24Changed; $u24Changed;"), json!(1));
}

#[test]
fn gradient_control_edits_a_multi_attribute() {
    let _t = turn();
    let mut c = Core::new();
    ok(&mut c, "createNode remapValue -n u24Remap;");
    ok(&mut c, "setAttr u24Remap.value[0].value_Position 0; setAttr u24Remap.value[0].value_FloatValue 0; setAttr u24Remap.value[0].value_Interp 1;");
    ok(&mut c, "setAttr u24Remap.value[1].value_Position 1; setAttr u24Remap.value[1].value_FloatValue 1; setAttr u24Remap.value[1].value_Interp 1;");
    ok(&mut c, "window u24W2; columnLayout; floatSliderGrp u24Pos; floatSliderGrp u24Val; gradientControl -at u24Remap.value -spc u24Pos -scc u24Val u24Grad;");
    assert_eq!(ok(&mut c, "gradientControl -q -nc u24Grad;"), json!(2));
    assert_eq!(ok(&mut c, "gradientControl -q -at u24Grad;"), json!("u24Remap.value"));
    // The attribute changed elsewhere: the control follows.
    ok(&mut c, "setAttr u24Remap.value[2].value_Position 0.5; setAttr u24Remap.value[2].value_FloatValue 0.2; setAttr u24Remap.value[2].value_Interp 0;");
    c.idle();
    assert_eq!(ok(&mut c, "gradientControl -q -nc u24Grad;"), json!(3));
    // The shell's edit is written to the attribute.
    let path = ok(&mut c, "gradientControl -q -fpn u24Grad;").as_str().unwrap().to_string();
    c.ui_event(&json!({"path": path, "set": {"asString": "0,0,1,0.4,0.5,0,1,1,1", "currentKey": 1}, "fire": ["__rampChanged"], "args": []}));
    let v = ok(&mut c, "getAttr u24Remap.value[2].value_FloatValue;");
    assert!(close(&nums(&v), &[0.4]), "{v}");
    // The position slider shows the current entry and moves it.
    assert!(close(&nums(&ok(&mut c, "floatSliderGrp -q -v u24Pos;")), &[0.5]));
    assert!(close(&nums(&ok(&mut c, "floatSliderGrp -q -v u24Val;")), &[0.4]));
    // A colour ramp from a colour multi.
    ok(&mut c, "gradientControl -at u24Remap.color u24GradColor;");
    let n = ok(&mut c, "getAttr -size u24Remap.color;");
    assert_eq!(ok(&mut c, "gradientControl -q -nc u24GradColor;"), n);
    // Settings.
    ok(&mut c, "gradientControl -e -as 1 -sp 1 -snc 1 -vl 1 -ror 1 -ro 1 -dki 1 u24Grad;");
    for f in ["as", "sp", "snc", "vl", "ro", "dki"] {
        assert_eq!(ok(&mut c, &format!("gradientControl -q -{f} u24Grad;")), json!(true), "{f}");
    }
    assert_eq!(ok(&mut c, "gradientControl -q -ror u24Grad;"), json!(1));
    // Not a ramp attribute.
    assert!(c.mel_checked("gradientControl -at u24Remap.inputValue u24Bad;").is_err());
    ok(&mut c, "gradientControl -e -cla u24Grad;");
    assert_eq!(ok(&mut c, "gradientControl -q -at u24Grad;"), json!(""));
}

#[test]
fn help_line_is_a_control() {
    let _t = turn();
    let mut c = Core::new();
    let p = ok(&mut c, "window u24W3; frameLayout -lv 0; helpLine -h 20 -ann \"tips\" u24Help;");
    assert!(p.as_str().unwrap().ends_with("u24Help"));
    assert_eq!(ok(&mut c, "helpLine -ex u24Help;"), json!(true));
    assert_eq!(ok(&mut c, "helpLine -q -h u24Help;"), json!(20));
    assert_eq!(ok(&mut c, "helpLine -q -ann u24Help;"), json!("tips"));
    ok(&mut c, "helpLine -e -vis 0 u24Help;");
    assert_eq!(ok(&mut c, "helpLine -q -vis u24Help;"), json!(false));
    assert!(ok(&mut c, "helpLine -q -fpn u24Help;").as_str().unwrap().contains("|"));
}

#[test]
fn absolute_move_puts_components_there() {
    let _t = turn();
    let mut c = Core::new();
    ok(&mut c, "polyCube; move -r 5 5 5 pCube1;");
    ok(&mut c, "move -a -os 1 2 3 pCube1.vtx[3];");
    assert!(close(&nums(&ok(&mut c, "xform -q -os -t pCube1.vtx[3];")), &[1.0, 2.0, 3.0]));
    ok(&mut c, "move -a 1 1 1 pCube1.vtx[0:1];");
    assert!(close(&nums(&ok(&mut c, "xform -q -ws -t pCube1.vtx[0:1];")), &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0]));
    ok(&mut c, "move -a -x 0 pCube1.vtx[4];");
    assert!(close(&nums(&ok(&mut c, "xform -q -ws -t pCube1.vtx[4];")), &[0.0, 5.5, 4.5]));
    ok(&mut c, "undo;");
    assert!(close(&nums(&ok(&mut c, "xform -q -ws -t pCube1.vtx[4];")), &[5.5, 5.5, 4.5]));
    // Relative still adds.
    ok(&mut c, "move -r 1 0 0 pCube1.vtx[4];");
    assert!(close(&nums(&ok(&mut c, "xform -q -ws -t pCube1.vtx[4];")), &[6.5, 5.5, 4.5]));
    // Curve CVs too.
    ok(&mut c, "curve -d 1 -p 0 0 0 -p 1 0 0 -n u24Crv;");
    ok(&mut c, "move -a 2 3 4 u24Crv.cv[1];");
    assert!(close(&nums(&ok(&mut c, "xform -q -ws -t u24Crv.cv[1];")), &[2.0, 3.0, 4.0]));
}

#[test]
fn attribute_query_on_multi_compounds() {
    let _t = turn();
    let mut c = Core::new();
    ok(&mut c, "createNode remapValue;");
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -multi value;"), json!(true));
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -multi color;"), json!(true));
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -multi inputValue;"), json!(false));
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -indexMatters value;"), json!(true));
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -keyable value;"), json!(false));
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -readable -writable value;"), json!(true));
    assert_eq!(ok(&mut c, "attributeQuery -n remapValue1 -typeExact value;"), json!("compound"));
}

#[test]
fn points_as_plugs() {
    let _t = turn();
    let mut c = Core::new();
    ok(&mut c, "polyCube;");
    assert_eq!(ok(&mut c, "getAttr pCube1.vtx[3].xv;"), json!(-0.5));
    assert_eq!(ok(&mut c, "getAttr pCube1.vtx[3].yValue;"), json!(0.5));
    ok(&mut c, "setAttr pCube1.vtx[3].xv 2;");
    assert!(close(&nums(&ok(&mut c, "getAttr pCube1.vtx[3];")), &[2.0, 0.5, 0.5]));
    ok(&mut c, "setAttr pCube1.vtx[3].zValue -1;");
    assert!(close(&nums(&ok(&mut c, "getAttr pCube1.vtx[3];")), &[2.0, 0.5, -1.0]));
    ok(&mut c, "curve -d 3 -p 0 0 0 -p 1 0 0 -p 2 1 0 -p 3 0 0;");
    assert!(close(&nums(&ok(&mut c, "getAttr curve1.cv[1];")), &[1.0, 0.0, 0.0]));
    ok(&mut c, "setAttr curve1.cv[1] 5 6 7;");
    assert!(close(&nums(&ok(&mut c, "getAttr curveShape1.cv[1];")), &[5.0, 6.0, 7.0]));
    ok(&mut c, "setAttr curve1.cv[1].yValue 9;");
    assert_eq!(ok(&mut c, "getAttr curve1.cv[1].yv;"), json!(9.0));
    assert!(close(&nums(&ok(&mut c, "getAttr curve1.cv[0:1].xValue;")), &[0.0, 5.0]));
    ok(&mut c, "setAttr curve1.cv[0:1] 1 1 1 2 2 2;");
    assert!(close(&nums(&ok(&mut c, "getAttr curve1.cv[0:1];")), &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0]));
    ok(&mut c, "sphere -n u24Sph;");
    assert_eq!(nums(&ok(&mut c, "getAttr u24Sph.cv[1][2];")).len(), 3);
}

#[test]
fn every_flag_of_the_unit_is_there() {
    let _t = turn();
    let c = Core::new();
    for (cmd, flags) in [
        ("grabColor", "a alpha hsv hsvValue rgb rgbValue"),
        ("gradientControl", "adaptiveScaling ann annotation at attribute backgroundColor bgc cla clearAttribute defineTemplate dgc displayKeyInfo dki docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor highlightMode hlc hlm io isObscured m manage nbg nc noBackground npm numberOfControls numberOfPopupMenus p parent pma po popupMenuArray preventOverride q query readOnly refreshOnRelease ro ror sbm scc selectedColorControl selectedInterpControl selectedPositionControl sic snc sp spc staticNumberOfControls staticPositions statusBarMessage ulc upperLimitControl useTemplate ut vcc verticalLayout vis visible visibleChangeCommand vl w width"),
        ("gradientControlNoAttr", "addEntry ae ann annotation asString backgroundColor bgc cc changeCommand civ ck ckc clv currentKey currentKeyChanged currentKeyColorValue currentKeyCurveValue currentKeyInterpValue cvv dc defineTemplate dgc displayKeyInfo dki docTag dpc dragCallback dragCommand dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor highlightMode hlc hlm io isObscured m manage nbg noBackground npm numberOfPopupMenus optionVar ov p parent pma po popupMenuArray preventOverride q query rac rampAsColor readOnly ro sbm snc staticNumberOfControls statusBarMessage useTemplate ut valueAtPoint vap vcc vis visible visibleChangeCommand w width"),
        ("headsUpDisplay", "ac ad allDescendants allowOverlap ao atr attachToRefresh attributeChange ba blockAlignment blockSize bs cc cf con conditionChange conditionFalse conditionTrue connectionChange ct da dataAlignment dataFontSize dataWidth decimalPrecision dfs di disregardIndex dp dw e edit ev event gco getOption gridColor labelFontSize labelWidth lastOccupiedBlock layoutVisibility lc le lfs listConditions listEvents listNodeChanges listPresets lnc lob lp lv lw n name nc nextFreeBlock nfb nodeChanges op p padding pre preset q query r refresh removeID removePosition resetNodeChanges rid rnc rp scriptResult setOption sg showGrid so sr"),
        ("helpLine", "ann annotation backgroundColor bgc defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor hlc io isObscured m manage nbg noBackground npm numberOfPopupMenus p parent pma po popupMenuArray preventOverride q query sbm statusBarMessage useTemplate ut vcc vis visible visibleChangeCommand w width"),
    ] {
        let def = c.s.commands.find(cmd).unwrap_or_else(|| panic!("no {cmd}"));
        for f in flags.split_whitespace() {
            assert!(def.flags.iter().any(|d| d.name == f || d.short_name == f) || matches!(f, "q" | "query" | "e" | "edit"), "{cmd} -{f}");
        }
    }
}

#[test]
fn ramp_strings_and_values() {
    let r = aztek_host::ramp_controls::Ramp::parse("0,0,1,1,1,1", false);
    assert_eq!(r.entries.len(), 2);
    assert!((r.at(0.25)[0] - 0.25).abs() < 1e-9);
    assert_eq!(r.to_text(), "0,0,1,1,1,1");
    let none = aztek_host::ramp_controls::Ramp::parse("0,0,0,1,1,0", false);
    assert_eq!(none.at(0.7)[0], 0.0);
    let smooth = aztek_host::ramp_controls::Ramp::parse("0,0,2,1,1,2", false);
    assert!((smooth.at(0.5)[0] - 0.5).abs() < 1e-9);
    assert!(smooth.at(0.25)[0] < 0.25);
    let c = aztek_host::ramp_controls::Ramp::parse("1,0,0,0,1,0,0,1,1,1", true);
    let v = c.at(0.5);
    assert!((v[0] - 0.5).abs() < 1e-9 && (v[2] - 0.5).abs() < 1e-9);
    let spline = aztek_host::ramp_controls::Ramp::parse("0,0,3,1,0.5,3,0,1,3", false);
    assert!((spline.at(0.5)[0] - 1.0).abs() < 1e-9);
    assert!(spline.at(0.25)[0] > 0.5);
}
