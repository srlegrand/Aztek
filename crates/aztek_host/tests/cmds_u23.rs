//! Unit u23: componentBox, confirmDialog -annotation, falloffCurve, falloffCurveAttr, fileBrowserDialog, floatSlider2,
//! fontDialog, framelessDialog. With no shell, dialogs give the answer a batch run would.

use aztek_host::Core;
use serde_json::json;

fn ok(c: &mut Core, src: &str) -> serde_json::Value {
    c.mel_checked(src).unwrap_or_else(|e| panic!("{src}: {e}"))
}

fn bad(c: &mut Core, src: &str) -> String {
    match c.mel_checked(src) {
        Ok(v) => panic!("{src} should fail, gave {v}"),
        Err(e) => e.to_string(),
    }
}

fn close(v: &serde_json::Value, want: &[f64]) {
    let got: Vec<f64> = v.as_array().unwrap_or_else(|| panic!("{v}")).iter().map(|x| x.as_f64().unwrap()).collect();
    assert_eq!(got.len(), want.len(), "{v}");
    for (a, b) in got.iter().zip(want) {
        assert!((a - b).abs() < 1e-6, "{v} vs {want:?}");
    }
}

#[test]
fn float_slider2_keeps_its_handles_in_order() {
    let mut c = Core::new();
    ok(&mut c, "window; columnLayout; floatField ff1; floatSlider2 -min 0 -max 360 -v1 30 -v2 90 -pc1 ff1 -pc2 ff2 s2; floatField ff2;");
    assert_eq!(ok(&mut c, "floatSlider2 -q -min s2;"), json!(0.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -max s2;"), json!(360.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -v1 s2;"), json!(30.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -v2 s2;"), json!(90.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -pol s2;"), json!(0));
    // Handle 1 drives its field; handle 2's field was made later and picks it up on the next change.
    assert_eq!(ok(&mut c, "floatField -q -v ff1;"), json!(30.0));
    ok(&mut c, "floatSlider2 -e -vs 10 200 -pol 1 s2;");
    assert_eq!(ok(&mut c, "floatSlider2 -q -v1 s2;"), json!(10.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -v2 s2;"), json!(200.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -pol s2;"), json!(1));
    assert_eq!(ok(&mut c, "floatField -q -v ff2;"), json!(200.0));
    // Handle 1 can't pass handle 2, and the range can't close.
    bad(&mut c, "floatSlider2 -e -v1 250 s2;");
    bad(&mut c, "floatSlider2 -e -v2 5 s2;");
    bad(&mut c, "floatSlider2 -e -vs 50 40 s2;");
    bad(&mut c, "floatSlider2 -e -max -1 s2;");
    bad(&mut c, "floatSlider2 -e -min 400 s2;");
    bad(&mut c, "floatSlider2 -e -pol 2 s2;");
    assert_eq!(ok(&mut c, "floatSlider2 -q -v1 s2;"), json!(10.0));
    // A smaller range pulls the handles in.
    ok(&mut c, "floatSlider2 -e -max 100 s2;");
    assert_eq!(ok(&mut c, "floatSlider2 -q -v2 s2;"), json!(100.0));
    // Defaults: 0 to 10, the handles at the ends.
    ok(&mut c, "floatSlider2 plain;");
    assert_eq!(ok(&mut c, "floatSlider2 -q -max plain;"), json!(10.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -v2 plain;"), json!(10.0));
    assert_eq!(ok(&mut c, "floatSlider2 -ex plain;"), json!(true));
    assert_eq!(ok(&mut c, "floatSlider2 -ex nothing;"), json!(false));
}

#[test]
fn float_slider2_commands_get_the_value() {
    let mut c = Core::new();
    ok(&mut c, "sphere -n ball; window; columnLayout; floatField f1; floatSlider2 -max 360 -cc1 \"setAttr makeNurbSphere1.startSweep\" -cc2 \"setAttr makeNurbSphere1.endSweep\" -pc1 f1 s2;");
    let path = ok(&mut c, "floatSlider2 -q -fpn s2;").as_str().unwrap().to_string();
    // The shell moves handle 1, then handle 2.
    c.ui_event(&json!({"path": path, "set": {"value1": 45.0}, "fire": ["__slider2Moved1"], "args": [45.0]}));
    c.ui_event(&json!({"path": path, "set": {"value2": 270.0}, "fire": ["__slider2Moved2"], "args": [270.0]}));
    assert_eq!(ok(&mut c, "getAttr makeNurbSphere1.startSweep;"), json!(45.0));
    assert_eq!(ok(&mut c, "getAttr makeNurbSphere1.endSweep;"), json!(270.0));
    assert_eq!(ok(&mut c, "floatField -q -v f1;"), json!(45.0));
    assert_eq!(ok(&mut c, "floatSlider2 -q -v1 s2;"), json!(45.0));
}

#[test]
fn falloff_curve_on_an_option_var() {
    let mut c = Core::new();
    ok(&mut c, "optionVar -rm u23Curve; optionVar -sva u23Curve \"0,1\" -sva u23Curve \"0.5,1\" -sva u23Curve \"0.5,0\" -sva u23Curve \"1,0\";");
    ok(&mut c, "window; columnLayout; falloffCurve -h 200 fCurve;");
    // Made with nothing to read: full at the start, nothing at the end.
    assert_eq!(ok(&mut c, "falloffCurve -q -asString fCurve;"), json!("0,1,1,0"));
    ok(&mut c, "falloffCurve -e -optionVar u23Curve fCurve;");
    assert_eq!(ok(&mut c, "falloffCurve -q -asString fCurve;"), json!("0,1,0.5,1,0.5,0,1,0"));
    assert_eq!(ok(&mut c, "falloffCurve -q -ov fCurve;"), json!("u23Curve"));
    // Keys: pick one, move it; the optionVar follows.
    ok(&mut c, "falloffCurve -e -ck 1 fCurve;");
    assert_eq!(ok(&mut c, "falloffCurve -q -ck fCurve;"), json!(1));
    ok(&mut c, "falloffCurve -e -ckv 0.25 0.75 fCurve;");
    close(&ok(&mut c, "falloffCurve -q -ckv fCurve;"), &[0.25, 0.75]);
    assert_eq!(ok(&mut c, "optionVar -q u23Curve;"), json!(["0,1", "0.25,0.75", "0.5,0", "1,0"]));
    // Moved past a neighbour, the key keeps being the current one.
    ok(&mut c, "falloffCurve -e -ckv 0.75 0.5 fCurve;");
    assert_eq!(ok(&mut c, "falloffCurve -q -ck fCurve;"), json!(2));
    assert_eq!(ok(&mut c, "falloffCurve -q -as fCurve;"), json!("0,1,0.5,0,0.75,0.5,1,0"));
    ok(&mut c, "falloffCurve -e -acv \"0.9,0.2\" fCurve;");
    assert_eq!(ok(&mut c, "falloffCurve -q -ck fCurve;"), json!(3));
    ok(&mut c, "falloffCurve -e -dcv 0 fCurve;");
    assert_eq!(ok(&mut c, "falloffCurve -q -as fCurve;"), json!("0.5,0,0.75,0.5,0.9,0.2,1,0"));
    assert_eq!(ok(&mut c, "optionVar -q u23Curve;"), json!(["0.5,0", "0.75,0.5", "0.9,0.2", "1,0"]));
    bad(&mut c, "falloffCurve -e -dcv 9 fCurve;");
    bad(&mut c, "falloffCurve -e -ck 9 fCurve;");
    // Snapping puts moved keys on the grid.
    ok(&mut c, "falloffCurve -e -stg true -ck 0 -ckv 0.33 0.47 fCurve;");
    close(&ok(&mut c, "falloffCurve -q -ckv fCurve;"), &[0.3, 0.5]);
    assert_eq!(ok(&mut c, "falloffCurve -q -stg fCurve;"), json!(true));
    // Settings read back.
    ok(&mut c, "falloffCurve -e -ro true -ccw true fCurve;");
    assert_eq!(ok(&mut c, "falloffCurve -q -ro fCurve;"), json!(true));
    assert_eq!(ok(&mut c, "falloffCurve -q -ccw fCurve;"), json!(true));
    // A script changing the optionVar changes the curve.
    ok(&mut c, "optionVar -sv u23Curve \"0,0\" -sva u23Curve \"1,1\";");
    assert_eq!(ok(&mut c, "falloffCurve -q -as fCurve;"), json!("0,0,1,1"));
    // The shell edits the points: the optionVar is written and -changeCommand runs.
    ok(&mut c, "global int $u23Changes = 0; falloffCurve -e -cc \"$u23Changes++\" fCurve;");
    let path = ok(&mut c, "falloffCurve -q -fpn fCurve;").as_str().unwrap().to_string();
    c.ui_event(&json!({"path": path, "set": {"asString": "0,0,0.5,0.8,1,1", "currentKey": 1}, "fire": ["__curveChanged"]}));
    assert_eq!(ok(&mut c, "optionVar -q u23Curve;"), json!(["0,0", "0.5,0.8", "1,1"]));
    assert_eq!(ok(&mut c, "global int $u23Changes; $u23Changes;"), json!(1));
    ok(&mut c, "optionVar -rm u23Curve;");
}

#[test]
fn falloff_curve_attr_edits_a_curve_attribute() {
    let mut c = Core::new();
    ok(&mut c, "createNode remapValue -n rv; setAttr rv.value[0].value_Position 0; setAttr rv.value[0].value_FloatValue 0; setAttr rv.value[1].value_Position 1; setAttr rv.value[1].value_FloatValue 1;");
    ok(&mut c, "window; columnLayout; floatSlider -min 0 -max 1 posSl; floatSlider -min 0 -max 1 valSl; falloffCurveAttr -h 150 -at \"rv.value\" -spc posSl -svc valSl crv;");
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -as crv;"), json!("0,0,1,1"));
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -ck crv;"), json!(0));
    ok(&mut c, "falloffCurveAttr -e -acv \"0.4,0.9\" crv;");
    assert_eq!(ok(&mut c, "getAttr -mi rv.value;"), json!([0, 1, 2]));
    assert_eq!(ok(&mut c, "getAttr rv.value[2].value_Position;"), json!(0.4));
    assert_eq!(ok(&mut c, "getAttr rv.value[2].value_FloatValue;"), json!(0.9));
    // The sliders show the current key, and moving one moves the key.
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -ck crv;"), json!(1));
    assert_eq!(ok(&mut c, "floatSlider -q -v posSl;"), json!(0.4));
    assert_eq!(ok(&mut c, "floatSlider -q -v valSl;"), json!(0.9));
    let sl = ok(&mut c, "floatSlider -q -fpn valSl;").as_str().unwrap().to_string();
    c.ui_event(&json!({"path": sl, "set": {"value": 0.6}, "fire": ["changeCommand"], "args": [0.6]}));
    close(&ok(&mut c, "falloffCurveAttr -q -ckv crv;"), &[0.4, 0.6]);
    assert_eq!(ok(&mut c, "getAttr rv.value[2].value_FloatValue;"), json!(0.6));
    // The attribute changed elsewhere: the control follows.
    ok(&mut c, "setAttr rv.value[1].value_FloatValue 0.5;");
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -as crv;"), json!("0,0,0.4,0.6,1,0.5"));
    ok(&mut c, "falloffCurveAttr -e -dcv 1 crv;");
    assert_eq!(ok(&mut c, "getAttr -mi rv.value;"), json!([0, 1]));
    ok(&mut c, "falloffCurveAttr -e -as \"0,1,0.5,0.5,1,0\" crv;");
    assert_eq!(ok(&mut c, "getAttr -mi rv.value;"), json!([0, 1, 2]));
    assert_eq!(ok(&mut c, "getAttr rv.value[2].value_Position;"), json!(1.0));
    // Integer switches, as the reference's flags take.
    ok(&mut c, "falloffCurveAttr -e -ro 1 -stg 1 crv;");
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -ro crv;"), json!(1));
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -stg crv;"), json!(1));
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -ccw crv;"), json!(0));
    // An element index in -attribute names the same multi.
    ok(&mut c, "falloffCurveAttr -at \"rv.value[1]\" crv2;");
    assert_eq!(ok(&mut c, "falloffCurveAttr -q -as crv2;"), json!("0,1,0.5,0.5,1,0"));
    bad(&mut c, "falloffCurveAttr -at \"rv.inputValue\" crv3;");
    bad(&mut c, "falloffCurveAttr crv4;");
    bad(&mut c, "falloffCurveAttr -e -at \"rv.value\" crv;");
}

#[test]
fn component_box_shows_and_moves_selected_components() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n box; select -r box.vtx[1] box.vtx[3];");
    ok(&mut c, "window; formLayout form; componentBox -pre 2 -lw 80 cbox; formLayout -e -af cbox \"top\" 0 -af cbox \"left\" 0 form;");
    assert_eq!(ok(&mut c, "componentBox -q -pre cbox;"), json!(2));
    assert_eq!(ok(&mut c, "componentBox -q -lw cbox;"), json!(80));
    ok(&mut c, "componentBox -e -mh 300 -mw 200 -rh 18 cbox;");
    assert_eq!(ok(&mut c, "componentBox -q -mh cbox;"), json!(300));
    assert_eq!(ok(&mut c, "componentBox -q -mw cbox;"), json!(200));
    bad(&mut c, "componentBox -e -pre 40 cbox;");
    // The shell selects cells, then one is edited: the vertex moves.
    let path = ok(&mut c, "componentBox -q -fpn cbox;").as_str().unwrap().to_string();
    c.ui_event(&json!({"path": path, "set": {"selectedAttr": ["vtx[3].yValue"]}}));
    assert_eq!(ok(&mut c, "componentBox -q -sla cbox;"), json!(["vtx[3].yValue"]));
    let was = ok(&mut c, "pointPosition -l box.vtx[3];");
    let (x, z) = (was[0].as_f64().unwrap(), was[2].as_f64().unwrap());
    c.ui_event(&json!({"path": path, "fire": ["__componentBoxEdit"], "args": ["vtx[3]", 1, 2.0]}));
    close(&ok(&mut c, "pointPosition -l box.vtx[3];"), &[x, 2.0, z]);
    // -execute runs a script per cell, or per selected cell.
    ok(&mut c, "global string $u23Cells[]; clear $u23Cells;");
    ok(&mut c, "componentBox -e -exe \"$u23Cells[size($u23Cells)] = \\\"#N #A\\\";\" true cbox;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $u23Cells[]; $u23Cells;"), json!(["box vtx[3].yValue"]));
    ok(&mut c, "clear $u23Cells; componentBox -e -exe \"$u23Cells[size($u23Cells)] = \\\"#A\\\";\" false cbox;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $u23Cells[]; size($u23Cells);"), json!(6));
    // Another selection: the rows follow, and cells no longer shown are no longer selected.
    ok(&mut c, "select -r box.vtx[0];");
    assert_eq!(ok(&mut c, "componentBox -q -sla cbox;"), json!([]));
}

#[test]
fn dialogs_answer_in_batch() {
    let mut c = Core::new();
    assert_eq!(ok(&mut c, "confirmDialog -t \"Delete\" -m \"Sure?\" -b \"Yes\" -b \"No\" -ann \"Delete it\" -ann \"Keep it\" -db \"No\" -dm \"More text\";"), json!("No"));
    assert_eq!(ok(&mut c, "framelessDialog -title \"Confirm\" -message \"File not found\" -path \"c:/path/of/file.txt\" -button \"Cancel\" -button \"OK\" -primary \"OK\";"), json!("OK"));
    assert_eq!(ok(&mut c, "framelessDialog -b \"A\" -b \"B\";"), json!("A"));
    assert_eq!(ok(&mut c, "framelessDialog;"), json!(""));
    // Nothing picked with no one to pick: no file command, the dialog's name back.
    ok(&mut c, "global int $u23Imported = 0; global proc int u23Import(string $f, string $t) { global int $u23Imported; $u23Imported++; return 1; }");
    let name = ok(&mut c, "fileBrowserDialog -m 0 -fc \"u23Import\" -ft \"image\" -an \"Import_Image\" -om \"Import\" -fl \"Pictures,*.png;*.jpg\" -wt \"Pick\" -tm \"tip\" -ds 2 -in \"x\";");
    assert!(name.as_str().unwrap().starts_with("fileBrowserDialog"), "{name}");
    assert_eq!(ok(&mut c, "global int $u23Imported; $u23Imported;"), json!(0));
    assert_eq!(ok(&mut c, "fontDialog;"), json!(""));
    let fonts = ok(&mut c, "fontDialog -FontList;");
    let fonts: Vec<String> = fonts.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert!(fonts.contains(&"DejaVu Sans".to_string()), "{fonts:?}");
    let scalable = ok(&mut c, "fontDialog -FontList -scalable;");
    assert!(scalable.as_array().unwrap().len() <= fonts.len());
}

#[test]
fn font_family_names_come_from_the_name_table() {
    let ttf = std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/DejaVuSans.ttf")).unwrap();
    assert_eq!(aztek_host::more_controls::font_family(&ttf).as_deref(), Some("DejaVu Sans"));
}

#[test]
fn every_flag_of_the_reference_is_there() {
    let c = Core::new();
    for (cmd, flags) in [
        ("componentBox", "ann annotation backgroundColor bgc defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exe execute exists fpn fullPathName h height highlightColor hlc io isObscured labelWidth lw m manage maxHeight maxWidth mh mw nbg noBackground npm numberOfPopupMenus p parent pma po popupMenuArray pre precision preventOverride q query rh rowHeight sbm selectedAttr sla statusBarMessage useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("confirmDialog", "ann annotation"),
        ("falloffCurve", "acv addControlVertex ann annotation asString backgroundColor bgc cc ccw changeCommand ck ckv currentKey currentKeyValue customCurveWidget dcv defineTemplate deleteControlVertex dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor hlc io isObscured m manage nbg noBackground npm numberOfPopupMenus optionVar ov p parent pma po popupMenuArray preventOverride q query readOnly ro sbm snapToGrid statusBarMessage stg useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("falloffCurveAttr", "acv addControlVertex ann annotation asString at attribute backgroundColor bgc cc ccw changeCommand ck ckv currentKey currentKeyValue customCurveWidget dcv defineTemplate deleteControlVertex dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor hlc io isObscured m manage nbg noBackground npm numberOfPopupMenus p parent pma po popupMenuArray preventOverride q query readOnly ro sbm selectedPositionControl selectedValueControl snapToGrid spc statusBarMessage stg svc useTemplate ut vcc vis visible visibleChangeCommand w width"),
        ("fileBrowserDialog", "actionName an dialogStyle ds fc fileCommand fileType filterList fl ft includeName m mode om operationMode tipMessage tm windowTitle wt"),
        ("floatSlider2", "ann annotation backgroundColor bgc cc1 cc2 changeCommand1 changeCommand2 defineTemplate dgc docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor hlc io isObscured m manage max maximum min minimum nbg noBackground npm numberOfPopupMenus p parent pc1 pc2 pma po pol polarity popupMenuArray positionControl1 positionControl2 preventOverride q query sbm statusBarMessage useTemplate ut v1 v2 value1 value2 values vcc vis visible visibleChangeCommand vs w width"),
        ("fontDialog", "FontList fl sc scalable"),
        ("framelessDialog", "b button m message p parent pat path pr primary t title"),
    ] {
        let def = c.s.commands.find(cmd).unwrap_or_else(|| panic!("{cmd} missing"));
        for f in flags.split_whitespace() {
            assert!(def.flags.iter().any(|d| d.name == f || d.short_name == f), "{cmd} -{f} missing");
        }
    }
}
