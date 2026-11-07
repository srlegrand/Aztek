//! Script UI, hooks, panels and tool contexts, with no interface: the model answers and the operations pile up.

use aztek_host::Core;
use serde_json::{json, Value};

fn errors(c: &mut Core) -> Vec<String> {
    c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect()
}

fn ok(c: &mut Core, src: &str) -> Value {
    let r = c.mel_checked(src);
    assert!(r.is_ok(), "{src}: {r:?}");
    r.unwrap()
}

/// A press on a control, as the shell reports it.
fn press(c: &mut Core, path: &str, flag: &str, args: Value) {
    c.ui_event(&json!({"path": path, "fire": [flag], "args": args}));
}

/// A change in a control, as the shell reports it: its value and the callback flags.
fn user_set(c: &mut Core, path: &str, flag: &str, v: Value, fire: &str) {
    c.ui_event(&json!({"path": path, "set": {flag: v.clone()}, "fire": [fire], "args": [v]}));
}

fn bound_window(c: &mut Core) {
    ok(c, "polyCube -n box; window w; columnLayout col;");
}

#[test]
fn connect_control_shows_sets_and_follows_a_plug() {
    let mut c = Core::new();
    bound_window(&mut c);
    ok(&mut c, "setAttr box.tx 3;");
    let f = ok(&mut c, "floatSliderGrp -f 1 -min -10 -max 10 s;");
    let path = f.as_str().unwrap().to_string();
    ok(&mut c, "connectControl s box.tx;");
    assert_eq!(ok(&mut c, "floatSliderGrp -q -v s;"), json!(3.0));
    // The plug moves: at idle the control follows.
    ok(&mut c, "setAttr box.tx 5;");
    c.idle();
    assert_eq!(ok(&mut c, "floatSliderGrp -q -v s;"), json!(5.0));
    // The user drags: the plug follows quietly, and letting go leaves one undoable line.
    user_set(&mut c, &path, "value", json!(6.0), "dragCommand");
    assert_eq!(ok(&mut c, "getAttr box.tx;"), json!(6.0));
    user_set(&mut c, &path, "value", json!(7.0), "dragCommand");
    assert_eq!(ok(&mut c, "getAttr box.tx;"), json!(7.0));
    c.take_log();
    user_set(&mut c, &path, "value", json!(8.0), "changeCommand");
    assert_eq!(ok(&mut c, "getAttr box.tx;"), json!(8.0));
    let echoes: Vec<String> = c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 0).map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(echoes.iter().filter(|l| l.starts_with("setAttr")).count(), 1, "{echoes:?}");
    ok(&mut c, "undo;");
    assert_eq!(ok(&mut c, "getAttr box.tx;"), json!(5.0));
    c.idle();
    assert_eq!(ok(&mut c, "floatSliderGrp -q -v s;"), json!(5.0));
    // A script's edit of a bound control sets the plug.
    ok(&mut c, "floatSliderGrp -e -v 2 s;");
    assert_eq!(ok(&mut c, "getAttr box.tx;"), json!(2.0));
    assert!(c.mel_checked("connectControl s nothing.tx;").is_err());
    errors(&mut c);
}

#[test]
fn connect_control_on_fields_by_index_and_compound() {
    let mut c = Core::new();
    bound_window(&mut c);
    ok(&mut c, "setAttr box.t 1 2 3;");
    ok(&mut c, "floatFieldGrp -nf 3 g;");
    ok(&mut c, "connectControl g box.t;");
    assert_eq!(ok(&mut c, "floatFieldGrp -q -v g;"), json!([1.0, 2.0, 3.0]));
    ok(&mut c, "floatFieldGrp -nf 2 h;");
    ok(&mut c, "connectControl -index 2 h box.ry; connectControl -index 3 h box.rz; connectControl -index 1 h box.ry;");
    ok(&mut c, "setAttr box.rz 9;");
    c.idle();
    assert_eq!(ok(&mut c, "floatFieldGrp -q -v2 h;"), json!(9.0));
    assert_eq!(ok(&mut c, "floatFieldGrp -q -l h;"), json!("Rotate Y"));
    ok(&mut c, "checkBox cb;");
    ok(&mut c, "connectControl cb box.visibility;");
    assert_eq!(ok(&mut c, "checkBox -q -v cb;"), json!(true));
    let p = ok(&mut c, "checkBox -q -fpn cb;").as_str().unwrap().to_string();
    user_set(&mut c, &p, "value", json!(false), "changeCommand");
    assert_eq!(ok(&mut c, "getAttr box.visibility;"), json!(false));
    errors(&mut c);
}

#[test]
fn attr_controls_take_their_shape_from_the_attribute() {
    let mut c = Core::new();
    bound_window(&mut c);
    ok(&mut c, "addAttr -ln \"mood\" -at \"enum\" -en \"calm:angry=4:odd\" box;");
    ok(&mut c, "addAttr -ln \"tint\" -at \"float3\" -uac box; addAttr -ln \"tintR\" -at \"float\" -p tint box; addAttr -ln \"tintG\" -at \"float\" -p tint box; addAttr -ln \"tintB\" -at \"float\" -p tint box;");
    ok(&mut c, "addAttr -ln \"gain\" -at \"double\" -min 0 -max 4 box;");
    ok(&mut c, "addAttr -ln \"count\" -at \"long\" box;");
    ok(&mut c, "addAttr -ln \"note\" -dt \"string\" box; setAttr -type \"string\" box.note \"hi\";");

    assert_eq!(ok(&mut c, "attrFieldSliderGrp -at box.gain fs;"), json!("w|col|fs"));
    assert_eq!(ok(&mut c, "attrFieldSliderGrp -q -at fs;"), json!("box.gain"));
    assert_eq!(ok(&mut c, "attrFieldSliderGrp -q -ex fs;"), json!(true));
    assert_eq!(ok(&mut c, "attrFieldSliderGrp -q -max fs;"), json!(4.0));
    assert_eq!(ok(&mut c, "attrFieldSliderGrp -q -l fs;"), json!("Gain"));
    ok(&mut c, "setAttr box.gain 2.5;");
    c.idle();
    assert_eq!(ok(&mut c, "attrFieldSliderGrp -q -v fs;"), json!(2.5));
    user_set(&mut c, "w|col|fs", "value", json!(1.5), "changeCommand");
    assert_eq!(ok(&mut c, "getAttr box.gain;"), json!(1.5));
    ok(&mut c, "undo;");
    assert_eq!(ok(&mut c, "getAttr box.gain;"), json!(2.5));

    // Three fields for a compound, one to a child.
    ok(&mut c, "setAttr box.t 1 2 3;");
    ok(&mut c, "attrFieldGrp -at box.t af;");
    assert_eq!(ok(&mut c, "floatFieldGrp -q -nf af;"), json!(3));
    assert_eq!(ok(&mut c, "floatFieldGrp -q -v af;"), json!([1.0, 2.0, 3.0]));
    user_set(&mut c, "w|col|af", "value2", json!(7.0), "changeCommand");
    assert_eq!(ok(&mut c, "getAttr box.ty;"), json!(7.0));

    // attrControlGrp picks by type.
    ok(&mut c, "attrControlGrp -a box.visibility c1; attrControlGrp -a box.mood c2; attrControlGrp -a box.count c3; attrControlGrp -a box.t c4; attrControlGrp -a box.tint c5; attrControlGrp -a box.note c6; attrControlGrp -a box.gain c7;");
    let ty = |c: &mut Core, n: &str| aztek_host::scriptui::control_kind_of(&ok(c, &format!("control -q -fpn {n};")).as_str().unwrap().to_string());
    assert_eq!(ty(&mut c, "c1").as_deref(), Some("checkBoxGrp"));
    assert_eq!(ty(&mut c, "c2").as_deref(), Some("optionMenuGrp"));
    assert_eq!(ty(&mut c, "c3").as_deref(), Some("intSliderGrp"));
    assert_eq!(ty(&mut c, "c4").as_deref(), Some("floatFieldGrp"));
    assert_eq!(ty(&mut c, "c5").as_deref(), Some("colorSliderGrp"));
    assert_eq!(ty(&mut c, "c6").as_deref(), Some("textFieldGrp"));
    assert_eq!(ty(&mut c, "c7").as_deref(), Some("floatSliderGrp"));
    assert_eq!(ok(&mut c, "attrControlGrp -q -a c2;"), json!("box.mood"));
    assert_eq!(ok(&mut c, "textFieldGrp -q -tx c6;"), json!("hi"));

    // An enum with gaps: the menu's position is not the value.
    ok(&mut c, "setAttr box.mood 4;");
    c.idle();
    assert_eq!(ok(&mut c, "optionMenuGrp -q -v c2;"), json!("angry"));
    user_set(&mut c, "w|col|c2", "select", json!(3), "changeCommand");
    assert_eq!(ok(&mut c, "getAttr box.mood;"), json!(5));
    // Rebinding: same kind in place, another kind made again under the same name.
    ok(&mut c, "attrControlGrp -e -a box.tx c7;");
    assert_eq!(ok(&mut c, "attrControlGrp -q -a c7;"), json!("box.tx"));
    ok(&mut c, "attrControlGrp -e -a box.visibility c7;");
    assert_eq!(ty(&mut c, "c7").as_deref(), Some("checkBoxGrp"));

    // A colour.
    ok(&mut c, "setAttr box.tint 0.5 0.25 1;");
    ok(&mut c, "attrColorSliderGrp -at box.tint cs;");
    assert_eq!(ok(&mut c, "colorSliderGrp -q -rgb cs;"), json!([0.5, 0.25, 1.0]));
    user_set(&mut c, "w|col|cs", "rgbValue", json!([1.0, 0.0, 0.0]), "changeCommand");
    assert_eq!(ok(&mut c, "getAttr box.tintG;"), json!(0.0));

    // Named items over an enum's own.
    ok(&mut c, "attrEnumOptionMenuGrp -at box.mood -ei 0 \"Still\" -ei 4 \"Cross\" em;");
    ok(&mut c, "setAttr box.mood 4;");
    c.idle();
    assert_eq!(ok(&mut c, "optionMenuGrp -q -v em;"), json!("Cross"));
    ok(&mut c, "attrEnumOptionMenu -at box.mood eo;");
    assert_eq!(ok(&mut c, "optionMenu -q -ni eo;"), json!(3));
    assert!(c.mel_checked("attrFieldSliderGrp -at box.nothing bad;").is_err());
    assert!(c.mel_checked("attrControlGrp bad2;").is_err());
    errors(&mut c);
}

#[test]
fn bound_controls_say_how_the_plug_is_driven() {
    let mut c = Core::new();
    bound_window(&mut c);
    ok(&mut c, "attrFieldSliderGrp -at box.tx fs;");
    aztek_host::scriptui::take_ops();
    ok(&mut c, "setKeyframe box.tx;");
    c.idle();
    let ops = aztek_host::scriptui::take_ops();
    assert!(ops.iter().any(|o| o["props"]["plugState"] == "keyOnFrame"), "{ops:?}");
    errors(&mut c);
}

#[test]
fn more_controls_as_the_reference_has_them() {
    let mut c = Core::new();
    ok(&mut c, "window w; columnLayout col;");
    ok(&mut c, "floatSliderButtonGrp -l \"Size\" -f 1 -bl \"Reset\" -bc \"$gReset = 1;\" -min 0 -max 5 -v 2 fb;");
    assert_eq!(ok(&mut c, "floatSliderButtonGrp -q -v fb;"), json!(2.0));
    assert_eq!(ok(&mut c, "floatSliderButtonGrp -q -bl fb;"), json!("Reset"));
    ok(&mut c, "global int $gReset = 0;");
    press(&mut c, "w|col|fb", "buttonCommand", json!([]));
    assert_eq!(ok(&mut c, "global int $gReset; $x = $gReset;"), json!(1));

    ok(&mut c, "symbolButton -i \"a.png\" -c \"$gReset = 2;\" sb;");
    assert_eq!(ok(&mut c, "symbolButton -q -i sb;"), json!("a.png"));

    let t = ok(&mut c, "shelfTabLayout st;");
    assert_eq!(t, json!("w|col|st"));
    assert_eq!(ok(&mut c, "shelfTabLayout -q -ex st;"), json!(true));
    assert_eq!(ok(&mut c, "objectTypeUI st;"), json!("tabLayout"));
    ok(&mut c, "columnLayout one; setParent ..; columnLayout two; setParent ..;");
    assert_eq!(ok(&mut c, "shelfTabLayout -q -ca st;"), json!(["one", "two"]));
    ok(&mut c, "shelfTabLayout -e -sti 2 st;");
    assert_eq!(ok(&mut c, "shelfTabLayout -q -st st;"), json!("two"));
    ok(&mut c, "setParent ..;");

    ok(&mut c, "layerButton -n \"mine\" -s 1 -lv 0 -ls \"template\" lb;");
    assert_eq!(ok(&mut c, "layerButton -q -n lb;"), json!("mine"));
    assert_eq!(ok(&mut c, "layerButton -q -lv lb;"), json!(false));
    assert_eq!(ok(&mut c, "layerButton -q -ls lb;"), json!("template"));

    ok(&mut c, "timePort tp;");
    assert_eq!(ok(&mut c, "timePort -q -ex tp;"), json!(true));
    ok(&mut c, "playbackOptions -min 3 -max 40;");
    ok(&mut c, "rangeControl rc;");
    assert_eq!(ok(&mut c, "rangeControl -q -min rc;").as_f64(), Some(3.0));
    ok(&mut c, "rangeControl -e -min 5 -max 20 rc;");
    assert_eq!(ok(&mut c, "playbackOptions -q -min;").as_f64(), Some(5.0));
    assert_eq!(ok(&mut c, "playbackOptions -q -max;").as_f64(), Some(20.0));
    ok(&mut c, "timeRangeControl trc;");
    assert_eq!(ok(&mut c, "timeRangeControl -q -ex trc;"), json!(true));
    // The range is the process's, as the time slider is: other tests start from it.
    ok(&mut c, "playbackOptions -min 1 -max 120;");
    errors(&mut c);
}

#[test]
fn script_table_asks_its_scripts() {
    let mut c = Core::new();
    ok(&mut c, "global proc string cell(int $r, int $c) { return ($r + \"x\" + $c); }");
    ok(&mut c, "global string $gSeen = \"\"; global proc int change(int $r, int $c, string $v) { global string $gSeen; $gSeen = $v; return ($v != \"no\"); }");
    ok(&mut c, "window w; columnLayout col;");
    ok(&mut c, "scriptTable -rows 2 -columns 2 -label 1 \"A\" -label 2 \"B\" -cw 1 40 -gcc \"cell\" -ccc \"change\" t;");
    c.idle();
    assert_eq!(ok(&mut c, "scriptTable -q -rows t;"), json!(2));
    assert_eq!(ok(&mut c, "scriptTable -ci 2 1 -q -cv t;"), json!("2x1"));
    c.ui_event(&json!({"path": "w|col|t", "cellEdit": [1, 2, "yes"]}));
    assert_eq!(ok(&mut c, "scriptTable -ci 1 2 -q -cv t;"), json!("yes"));
    c.ui_event(&json!({"path": "w|col|t", "cellEdit": [1, 1, "no"]}));
    assert_eq!(ok(&mut c, "scriptTable -ci 1 1 -q -cv t;"), json!("1x1"));
    ok(&mut c, "scriptTable -e -ir 1 t;");
    assert_eq!(ok(&mut c, "scriptTable -q -rows t;"), json!(3));
    assert_eq!(ok(&mut c, "scriptTable -ci 2 1 -q -cv t;"), json!("1x1"));
    ok(&mut c, "scriptTable -e -dr 1 t;");
    assert_eq!(ok(&mut c, "scriptTable -ci 1 1 -q -cv t;"), json!("1x1"));
    c.ui_event(&json!({"path": "w|col|t", "set": {"selectedCells": [2, 1, 2, 2]}}));
    assert_eq!(ok(&mut c, "scriptTable -q -sr t;"), json!(2));
    assert_eq!(ok(&mut c, "scriptTable -q -scs t;"), json!([1, 2]));
    ok(&mut c, "scriptTable -e -clr t;");
    assert_eq!(ok(&mut c, "scriptTable -q -rows t;"), json!(0));
    errors(&mut c);
}

#[test]
fn tool_buttons_set_and_follow_the_tool() {
    let mut c = Core::new();
    ok(&mut c, "window w; columnLayout col; toolCollection tc;");
    ok(&mut c, "toolButton -t moveSuperContext mv; toolButton -t scaleSuperContext sc;");
    assert_eq!(ok(&mut c, "toolButton -q -t mv;"), json!("moveSuperContext"));
    assert_eq!(ok(&mut c, "toolButton -q -tc mv;"), json!(1));
    assert_eq!(ok(&mut c, "toolCollection -q -cia tc;"), json!(["mv", "sc"]));
    c.ui_event(&json!({"path": "w|col|mv", "set": {"select": true}}));
    assert_eq!(ok(&mut c, "currentCtx;"), json!("moveSuperContext"));
    assert_eq!(ok(&mut c, "toolCollection -q -sl tc;"), json!("mv"));
    ok(&mut c, "setToolTo scaleSuperContext;");
    c.idle();
    assert_eq!(ok(&mut c, "toolCollection -q -sl tc;"), json!("sc"));
    assert_eq!(ok(&mut c, "toolButton -q -sl mv;"), json!(false));
    ok(&mut c, "toolButton -e -sl 1 mv;");
    assert_eq!(ok(&mut c, "currentCtx;"), json!("moveSuperContext"));
    assert_eq!(ok(&mut c, "toolHasOptions moveSuperContext;"), json!(true));
    errors(&mut c);
}

#[test]
fn hud_controls_live_with_the_heads_up_display() {
    let mut c = Core::new();
    ok(&mut c, "hudButton -s 3 -b 1 -l \"Go\" -pc \"print 1;\" hb;");
    ok(&mut c, "hudSlider -s 3 -b 2 -l \"Amount\" -min 0 -max 10 -v 2 -t \"float\" hs;");
    ok(&mut c, "hudSliderButton -s 3 -b 3 -sl \"Size\" -bl \"Set\" -v 1 hsb;");
    assert_eq!(ok(&mut c, "hudButton -q -l hb;"), json!("Go"));
    assert_eq!(ok(&mut c, "hudSlider -q -max hs;"), json!(10));
    ok(&mut c, "hudSlider -e -v 4 hs;");
    assert_eq!(ok(&mut c, "hudSlider -q -v hs;"), json!(4));
    assert_eq!(ok(&mut c, "headsUpDisplay -ex hb;"), json!(true));
    assert_eq!(aztek_host::ui_cmds::hud_controls().iter().filter(|(n, _)| ["hb", "hs", "hsb"].contains(&n.as_str())).count(), 3);
    ok(&mut c, "headsUpDisplay -rem hb; headsUpDisplay -rem hs; headsUpDisplay -rem hsb;");
    assert_eq!(ok(&mut c, "hudButton -ex hb;"), json!(false));
    errors(&mut c);
}

#[test]
fn filters_connections_configurations_and_templates() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n boxA; polyCube -n boxB; spaceLocator -n loc;");
    assert_eq!(ok(&mut c, "itemFilter -byType \"locator\" lf;"), json!("lf"));
    assert_eq!(ok(&mut c, "itemFilter -ex lf;"), json!(true));
    ok(&mut c, "itemFilter -bn \"boxA*\" nf; itemFilter -un lf nf both; itemFilter -neg 1 -bn \"box*\" notbox;");
    let l = ok(&mut c, "lsThroughFilter lf;");
    assert!(l.as_array().unwrap().iter().any(|x| x.as_str().unwrap().ends_with("locShape")), "{l:?}");
    let l = ok(&mut c, "lsThroughFilter nf;");
    assert_eq!(l.as_array().unwrap().iter().filter(|x| x.as_str().unwrap().contains("boxB")).count(), 0);
    let l = ok(&mut c, "lsThroughFilter both;");
    assert!(l.as_array().unwrap().iter().any(|x| x.as_str().unwrap().contains("boxA")));
    assert!(l.as_array().unwrap().iter().any(|x| x.as_str().unwrap().ends_with("locShape")));
    assert!(ok(&mut c, "itemFilter -q -bn nf;").to_string().contains("boxA"));

    ok(&mut c, "selectionConnection -activeList act;");
    ok(&mut c, "select boxA;");
    assert_eq!(ok(&mut c, "selectionConnection -q -obj act;"), json!(["boxA"]));
    ok(&mut c, "selectionConnection -e -clr act;");
    assert_eq!(ok(&mut c, "ls -sl;"), json!([]));
    ok(&mut c, "selectionConnection own;");
    ok(&mut c, "selectionConnection -e -s boxA -s boxB own;");
    assert_eq!(ok(&mut c, "selectionConnection -q -obj own;"), json!(["boxA", "boxB"]));
    ok(&mut c, "selectionConnection -e -d boxA own;");
    assert_eq!(ok(&mut c, "selectionConnection -q -obj own;"), json!(["boxB"]));
    assert_eq!(ok(&mut c, "selectionConnection -q -ex own;").as_bool(), None);
    assert_eq!(ok(&mut c, "selectionConnection -ex own;"), json!(true));

    ok(&mut c, "panelConfiguration -l \"Two\" -sc 0 -cfs \"x\" -ap 1 \"Persp\" \"modelPanel\" \"a\" \"b\" cfg;");
    assert_eq!(ok(&mut c, "panelConfiguration -q -np cfg;"), json!(1));
    assert_eq!(ok(&mut c, "panelConfiguration -q -l cfg;"), json!("Two"));
    assert_eq!(ok(&mut c, "getPanel -cwl \"Two\";"), json!("cfg"));
    assert!(ok(&mut c, "getPanel -ac;").as_array().unwrap().contains(&json!("cfg")));

    ok(&mut c, "editorTemplate -bsl; editorTemplate -bl \"Mine\" -cl 0; editorTemplate -ac \"translate\"; editorTemplate -as; editorTemplate -el; editorTemplate -s \"caching\"; editorTemplate -aec; editorTemplate -esl;");
    let t = aztek_host::script_objects::take_template();
    let kinds: Vec<&str> = t.iter().map(|x| x["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["beginLayout", "control", "separator", "endLayout", "suppress", "extraControls"]);
    assert_eq!(t[1]["attr"], "translate");

    assert_eq!(ok(&mut c, "baseTemplate -ex \"x\";"), json!(false));
    ok(&mut c, "baseTemplate -l \"x\";");
    assert_eq!(ok(&mut c, "baseTemplate -ex \"x\";"), json!(true));
    errors(&mut c);
}

#[test]
fn editors_scripts_make() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n box; window w; columnLayout col;");
    assert_eq!(ok(&mut c, "outlinerEditor -showShapes 1 -showDagOnly 0 -mlc \"worldList\" oe;"), json!("w|col|oe"));
    assert_eq!(ok(&mut c, "outlinerEditor -q -showShapes oe;"), json!(true));
    assert_eq!(ok(&mut c, "outlinerEditor -q -dag oe;"), json!(false));
    assert_eq!(ok(&mut c, "outlinerEditor -q -mlc oe;"), json!("worldList"));
    ok(&mut c, "outlinerEditor -e -shp 0 oe;");
    assert_eq!(ok(&mut c, "outlinerEditor -q -shp oe;"), json!(false));
    assert_eq!(ok(&mut c, "outlinerEditor -ex oe;"), json!(true));
    assert_eq!(ok(&mut c, "control -q -ex oe;"), json!(true));
    // The application's own, under the reference's names.
    ok(&mut c, "outlinerEditor -e -showShapes 1 outlinerPanel1;");
    assert_eq!(ok(&mut c, "outlinerEditor -q -showShapes outlinerPanel1;"), json!(true));
    assert_eq!(ok(&mut c, "outlinerPanel -q -outlinerEditor outlinerPanel1;"), json!("outlinerPanel1"));

    // Panels with editors in them.
    assert_eq!(ok(&mut c, "outlinerPanel op;"), json!("op"));
    assert_eq!(ok(&mut c, "outlinerPanel -q -oe op;"), json!("op"));
    assert_eq!(ok(&mut c, "outlinerEditor -q -ex op;"), json!(true));
    assert!(ok(&mut c, "getPanel -typ \"outlinerPanel\";").as_array().unwrap().contains(&json!("op")));
    ok(&mut c, "blendShapePanel bp; posePanel pp;");
    assert_eq!(ok(&mut c, "blendShapePanel -q -be bp;"), json!("bp"));
    assert_eq!(ok(&mut c, "posePanel -q -pe pp;"), json!("pp"));

    assert_eq!(ok(&mut c, "nodeEditor -gv 1 -ntm \"short\" ne;"), json!("w|col|ne"));
    assert_eq!(ok(&mut c, "nodeEditor -q -ntm ne;"), json!("short"));
    ok(&mut c, "select box; nodeEditor -e -rfs ne;");
    assert_eq!(ok(&mut c, "dopeSheetEditor -stk 1 de;"), json!("w|col|de"));
    assert_eq!(ok(&mut c, "dopeSheetEditor -q -stk de;"), json!(true));
    ok(&mut c, "animCurveEditor -dk \"on\" ce;");
    assert_eq!(ok(&mut c, "animCurveEditor -q -dk ce;"), json!("on"));
    ok(&mut c, "setKeyframe box.tx;");
    assert_eq!(ok(&mut c, "animCurveEditor -q -cs ce;"), json!(["box_translateX"]));
    assert_eq!(ok(&mut c, "animCurveEditor -q -ex ce;"), json!(true));
    ok(&mut c, "blendShapeEditor -vs 1 be; poseEditor pe;");
    assert_eq!(ok(&mut c, "blendShapeEditor -q -vs be;"), json!(true));
    ok(&mut c, "componentEditor -pre 4 ce2; spreadSheetEditor -ko 0 se;");
    assert_eq!(ok(&mut c, "componentEditor -q -pre ce2;"), json!(4));
    assert_eq!(ok(&mut c, "spreadSheetEditor -q -ko se;"), json!(false));
    ok(&mut c, "keyframeOutliner -an \"box_translateX\" ko;");
    assert_eq!(ok(&mut c, "keyframeOutliner -q -an ko;"), json!("box_translateX"));

    // Made with no layout, put in one later.
    ok(&mut c, "setParent -top; deleteUI w;");
    assert_eq!(ok(&mut c, "outlinerEditor loose;"), json!("loose"));
    ok(&mut c, "window w2; columnLayout col2;");
    ok(&mut c, "outlinerEditor -e -p col2 loose;");
    assert_eq!(ok(&mut c, "control -q -fpn loose;"), json!("w2|col2|loose"));
    ok(&mut c, "outlinerEditor -e -up loose;");
    assert_eq!(ok(&mut c, "control -q -fpn loose;"), json!("loose"));

    // No clips here: a plain warning, and the Dope Sheet opens.
    c.take_log();
    aztek_host::take_ui_requests_for_tests();
    assert_eq!(ok(&mut c, "clipEditor;"), json!("clipEditor1"));
    assert!(aztek_host::take_ui_requests_for_tests().to_string().contains("DopeSheetEditor"));
    assert!(c.take_log().to_string().contains("clips are not available"));
    assert_eq!(ok(&mut c, "clipEditor -ex x;"), json!(false));
    errors(&mut c);
}

fn requests() -> Vec<String> {
    aztek_host::take_ui_requests_for_tests().as_array().unwrap().iter().filter_map(|r| r.as_str().map(str::to_string)).collect()
}

#[test]
fn contexts_reach_the_tools_that_serve_them() {
    let mut c = Core::new();
    requests();
    for (cmd, want) in [
        ("orbitCtx", "setTool|camTumble"),
        ("boxDollyCtx", "setTool|camBoxDolly"),
        ("walkCtx", "setTool|camWalk"),
        ("modelCurrentTimeCtx", "setTool|modelTime"),
        ("curveAddPtCtx", "setTool|curveAddPt"),
        ("curveMoveEPCtx", "setTool|move"),
        ("curveBezierCtx", "setTool|curveBezier"),
        ("twoPointArcCtx", "setTool|arcTwo"),
        ("threePointArcCtx", "setTool|arcThree"),
        ("polySelectCtx", "setTool|select"),
        ("polySuperCtx", "setTool|select"),
        ("polyCreaseCtx", "setTool|crease"),
        ("polyMergeEdgeCtx", "setTool|mergeEdge"),
        ("polyMergeFacetCtx", "setTool|mergeFacet"),
        ("createPolyCubeCtx", "setTool|createCube"),
        ("createPolyPlaneCtx", "setTool|createPlane"),
        ("snapTogetherCtx", "setTool|snapTogether"),
        ("showManipCtx", "ShowManipulators"),
        ("setEditCtx", "setTool|editMembership"),
        ("artSetPaintCtx", "setTool|paintSelect"),
        ("artUserPaintCtx", "setTool|paintSelect"),
        ("artAttrPaintVertexCtx", "setTool|paintSelect"),
        ("artPuttyCtx", "setTool|sculpt"),
        ("graphDollyCtx", "keyTool|dolly"),
        ("graphTrackCtx", "keyTool|track"),
        ("graphSelectContext", "keyTool|select"),
        ("directKeyCtx", "keyTool|move"),
        ("currentTimeCtx", "keyTool|time"),
        ("curveEditorCtx", "keyTool|move"),
        ("setKeyCtx", "keyTool|set"),
        ("keyframeRegionMoveKeyCtx", "keyTool|move"),
        ("keyframeRegionScaleKeyCtx", "keyTool|scale"),
        ("keyframeRegionSelectKeyCtx", "keyTool|select"),
        ("keyframeRegionSetKeyCtx", "keyTool|set"),
        ("keyframeRegionInsertKeyCtx", "keyTool|insert"),
        ("keyframeRegionDirectKeyCtx", "keyTool|move"),
        ("keyframeRegionDollyCtx", "keyTool|dolly"),
        ("keyframeRegionTrackCtx", "keyTool|track"),
        ("keyframeRegionCurrentTimeCtx", "keyTool|time"),
        ("texRotateContext", "uvTool|rotate"),
        ("texScaleContext", "uvTool|scale"),
        ("texSelectContext", "uvTool|select"),
        ("texMoveUVShellContext", "uvTool|shell"),
        ("texCutContext", "uvTool|cut"),
        ("texSmoothContext", "uvTool|smooth"),
        ("texTweakUVContext", "uvTool|tweak"),
        ("texSelectShortestPathCtx", "uvTool|path"),
        ("textureLassoContext", "uvTool|lasso"),
        ("texWinToolCtx", "uvTool|window"),
    ] {
        let n = format!("{cmd}Made");
        ok(&mut c, &format!("{cmd} {n}; setToolTo {n};"));
        assert_eq!(ok(&mut c, "currentCtx;"), json!(n));
        let r = requests();
        assert!(r.iter().any(|x| x == want), "{cmd}: {r:?}");
    }
    // A curve can take a key from the tools by the curve's own name.
    ok(&mut c, "polyCube -n keyed; setKeyframe keyed.tx;");
    let plugs = ok(&mut c, "listConnections -p 1 -d 1 -s 0 keyed_translateX;");
    assert_eq!(plugs, json!(["keyed.translateX"]));
    ok(&mut c, "setKeyframe -t 10 -i keyed.translateX;");
    assert_eq!(ok(&mut c, "keyframe -q -kc keyed_translateX;"), json!(2));
    // The reference's own names for the UV tools work from the start.
    ok(&mut c, "setToolTo texMoveContext;");
    assert!(requests().iter().any(|x| x == "uvTool|move"));
    // Those with nothing behind them say so, once.
    c.take_log();
    ok(&mut c, "texSmudgeUVContext sm; setToolTo sm; texLatticeDeformContext lt; setToolTo lt; setToolTo sm;");
    let log = c.take_log().to_string();
    assert_eq!(log.matches("texSmudgeUVContext: this tool").count(), 1, "{log}");
    assert_eq!(log.matches("texLatticeDeformContext: this tool").count(), 1, "{log}");
    assert_eq!(ok(&mut c, "toolHasOptions sm;"), json!(true));
    errors(&mut c);
}

#[test]
fn view_commands_for_cameras_and_manipulators() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n box;");
    let before = ok(&mut c, "getAttr persp.tx;");
    ok(&mut c, "tumble -aa 30 -ea 10 persp;");
    assert_ne!(ok(&mut c, "getAttr persp.tx;"), before);
    let before = ok(&mut c, "getAttr persp.tx;");
    ok(&mut c, "panZoom -r 2 persp;");
    assert_ne!(ok(&mut c, "getAttr persp.tx;"), before);

    assert_eq!(ok(&mut c, "manipPivot -q -v;"), json!(false));
    ok(&mut c, "select box; manipPivot -p 1 2 3;");
    assert_eq!(ok(&mut c, "manipPivot -q -p;"), json!([1.0, 2.0, 3.0]));
    assert_eq!(ok(&mut c, "manipPivot -q -v;"), json!(true));
    c.idle();
    ok(&mut c, "select -cl;");
    c.idle();
    assert_eq!(ok(&mut c, "manipPivot -q -v;"), json!(false));
    ok(&mut c, "manipPivot -p 1 0 0; manipPivot -r;");
    assert_eq!(ok(&mut c, "manipPivot -q -v;"), json!(false));

    requests();
    ok(&mut c, "manipOptions -s 2;");
    assert_eq!(ok(&mut c, "manipOptions -q -s;"), json!(2));
    assert!(requests().iter().any(|x| x.contains("manipSize")));
    assert_eq!(ok(&mut c, "setXformManip -q -ws;"), json!(true));
    ok(&mut c, "setXformManip -ws 0;");
    assert_eq!(ok(&mut c, "setXformManip -q -ws;"), json!(false));
    ok(&mut c, "viewManip -v 0;");
    assert_eq!(ok(&mut c, "viewManip -q -v;"), json!(false));
    ok(&mut c, "viewManip -v 1;");
    requests();
    ok(&mut c, "pixelMove 3 4;");
    assert!(requests().iter().any(|x| x.starts_with("pixelMove|")));
    ok(&mut c, "softSelectOptionsCtx so; setToolTo so; softSelectOptionsCtx -e -btd 1 so;");
    assert!(requests().iter().any(|x| x.contains("brushAdjust")));
    errors(&mut c);
}

#[test]
fn script_contexts_run_on_what_is_picked() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n a; polyCube -n b; select -cl;");
    ok(&mut c, "global string $gStarted = \"\"; global string $gPicked = \"\"; global string $gDone = \"\";");
    ok(&mut c, "scriptCtx -ts \"global string $gStarted; $gStarted = \\\"yes\\\";\" -tf \"global string $gDone; $gDone = \\\"yes\\\";\" -ssc 2 -sac 1 -euc 0 -fcs \"global string $gPicked; $gPicked = stringArrayToString($Selection1, \\\",\\\");\" pick;");
    ok(&mut c, "setToolTo pick;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $gStarted; $x = $gStarted;"), json!("yes"));
    ok(&mut c, "select a;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $gPicked; $x = $gPicked;"), json!(""));
    ok(&mut c, "select -add b;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $gPicked; $x = $gPicked;"), json!("a,b"));
    assert_eq!(ok(&mut c, "ls -sl;"), json!([]));
    // Leaving runs the finish script.
    ok(&mut c, "setToolTo selectSuperContext;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $gDone; $x = $gDone;"), json!("yes"));
    // Enter finishes one that does not complete by itself.
    ok(&mut c, "scriptCtx -ssc 1 -fcs \"global string $gPicked; $gPicked = \\\"enter\\\";\" pick2; setToolTo pick2; select a;");
    c.idle();
    assert_ne!(ok(&mut c, "global string $gPicked; $x = $gPicked;"), json!("enter"));
    ok(&mut c, "ctxCompletion;");
    c.idle();
    assert_eq!(ok(&mut c, "global string $gPicked; $x = $gPicked;"), json!("enter"));
    errors(&mut c);
}

#[test]
fn paint_contexts_act_on_what_the_brush_takes() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n a; sets -em -n mine; select -cl;");
    ok(&mut c, "artSetPaintCtx -stm mine ps; setToolTo ps;");
    c.idle();
    ok(&mut c, "select a.vtx[0];");
    c.idle();
    assert!(ok(&mut c, "sets -q mine;").to_string().contains("vtx[0]"));
    ok(&mut c, "select -cl; setToolTo selectSuperContext;");
    c.idle();
    ok(&mut c, "global int $gSeen = -1; global proc setVal(int $s, int $v, float $x) { global int $gSeen; $gSeen = $v; }");
    ok(&mut c, "artUserPaintCtx -svc \"setVal\" up; setToolTo up;");
    c.idle();
    ok(&mut c, "select a.vtx[3];");
    c.idle();
    assert_eq!(ok(&mut c, "global int $gSeen; $x = $gSeen;"), json!(3));
    ok(&mut c, "select -cl; artPuttyCtx -r 2.5 -mtm \"smooth\" pt; setToolTo pt;");
    errors(&mut c);
}

#[test]
fn attribute_editor_templates_run_by_type() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n box;");
    assert_eq!(aztek_host::script_objects::ae_template(&mut c, "transform box"), json!([]));
    ok(&mut c, "global proc AEtransformTemplate(string $n) { editorTemplate -bsl; editorTemplate -bl \"Mine\"; editorTemplate -ac \"translateX\"; editorTemplate -el; editorTemplate -s \"caching\"; editorTemplate -esl; }");
    let t = aztek_host::script_objects::ae_template(&mut c, "transform box");
    assert_eq!(t[0]["title"], "Mine");
    assert_eq!(t[1]["attr"], "translateX");
    assert_eq!(t[3]["kind"], "suppress");
    errors(&mut c);
}

#[test]
fn menu_items_say_what_they_are() {
    let mut c = Core::new();
    ok(&mut c, "window w -mb 1; menu m; menuItem -l \"Do\" one; menuItem -ob 1 box; menuItem -cb 1 chk;");
    assert_eq!(ok(&mut c, "menuItem -q -iob box;"), json!(true));
    assert_eq!(ok(&mut c, "menuItem -q -iob one;"), json!(false));
    assert_eq!(ok(&mut c, "menuItem -q -icb chk;"), json!(true));
    errors(&mut c);
}

#[test]
fn panes_keep_their_sizes() {
    let mut c = Core::new();
    ok(&mut c, "window w; paneLayout -cn \"vertical2\" -ps 1 30 100 pl;");
    assert_eq!(ok(&mut c, "paneLayout -q -ps pl;"), json!([30, 100]));
    aztek_host::scriptui::take_ops();
    ok(&mut c, "paneLayout -e -ps 2 70 100 pl;");
    assert_eq!(ok(&mut c, "paneLayout -q -ps pl;"), json!([30, 100, 70, 100]));
    let ops = aztek_host::scriptui::take_ops();
    assert_eq!(ops[0]["props"]["paneSizes"]["1"], json!([30, 100]), "{ops:?}");
    errors(&mut c);
}

#[test]
fn workspace_controls_say_where_they_are() {
    let mut c = Core::new();
    ok(&mut c, "workspaceControl -l \"Free\" free;");
    assert_eq!(ok(&mut c, "workspaceControl -q -fl free;"), json!(true));
    ok(&mut c, "workspaceControl -dtm \"right\" 1 fixed;");
    assert_eq!(ok(&mut c, "workspaceControl -q -fl fixed;"), json!(false));
    ok(&mut c, "workspaceControl -e -dtc \"Outliner\" \"bottom\" free;");
    assert_eq!(ok(&mut c, "workspaceControl -q -fl free;"), json!(false));
    ok(&mut c, "window dw; columnLayout; text; dockControl -a \"left\" -con dw -l \"Dock\" dc;");
    assert_eq!(ok(&mut c, "dockControl -q -a dc;"), json!("left"));
    assert_eq!(ok(&mut c, "dockControl -q -fl dc;"), json!(false));
    errors(&mut c);
}

#[test]
fn drags_carry_messages_to_the_drop() {
    let mut c = Core::new();
    ok(&mut c, "global string $gGot = \"\";");
    ok(&mut c, "global proc string[] dragIt(string $ctl, int $x, int $y, int $mods) { return {\"one\", \"two\"}; }");
    ok(&mut c, "global proc dropIt(string $from, string $to, string $msgs[], int $x, int $y, int $type) { global string $gGot; $gGot = ($from + \">\" + $to + \":\" + $msgs[1] + \":\" + $x); }");
    ok(&mut c, "window w; columnLayout col; button -dgc \"dragIt\" a; button -dpc \"dropIt\" b;");
    let ops = aztek_host::scriptui::take_ops();
    assert!(ops.iter().any(|o| o["props"]["hasDrag"] == true), "{ops:?}");
    assert!(ops.iter().any(|o| o["props"]["hasDrop"] == true), "{ops:?}");
    let msgs = aztek_host::attr_ui::drag_start_for_tests(&mut c, &json!({"path": "w|col|a", "x": 3, "y": 4, "modifiers": 0}).to_string());
    assert_eq!(msgs, json!(["one", "two"]));
    c.ui_event(&json!({"path": "w|col|b", "dropFrom": "w|col|a", "messages": msgs, "x": 7, "y": 8, "dragType": 1}));
    assert_eq!(ok(&mut c, "global string $gGot; $x = $gGot;"), json!("w|col|a>w|col|b:two:7"));
    errors(&mut c);
}

#[test]
fn movies_go_through_ffmpeg() {
    use aztek_host::panel_cmds::{find_program, movie_arguments, movie_file_name};
    assert!(find_program("surely-no-program-has-this-name").is_none());
    assert_eq!(movie_file_name("/a.b/shot", "qt"), "/a.b/shot.mov");
    assert_eq!(movie_file_name("/a/shot", "avi"), "/a/shot.avi");
    assert_eq!(movie_file_name("/a/shot.mp4", "movie"), "/a/shot.mp4");
    let args = movie_arguments("/t/frame.%06d.png", 24.0, "/a/shot.mov");
    assert_eq!(args.first().map(String::as_str), Some("-y"));
    assert_eq!(args.last().map(String::as_str), Some("/a/shot.mov"));
    assert!(args.iter().any(|x| x == "24"));
    let mut c = Core::new();
    assert!(ok(&mut c, "playblast -q -fmt;").to_string().contains("movie"));
    errors(&mut c);
}

#[test]
fn names_and_paths_as_the_reference_returns_them() {
    let mut c = Core::new();
    assert_eq!(ok(&mut c, "window;"), json!("window1"));
    assert_eq!(ok(&mut c, "columnLayout;"), json!("window1|columnLayout1"));
    assert_eq!(ok(&mut c, "button -l \"Press\";"), json!("window1|columnLayout1|button1"));
    assert_eq!(ok(&mut c, "button named;"), json!("window1|columnLayout1|named"));
    assert_eq!(ok(&mut c, "setParent -q;"), json!("window1|columnLayout1"));
    assert_eq!(ok(&mut c, "setParent ..;"), json!("window1"));
    assert_eq!(ok(&mut c, "rowLayout -nc 2;"), json!("window1|rowLayout1"));
    assert_eq!(ok(&mut c, "text;"), json!("window1|rowLayout1|text1"));
    assert_eq!(ok(&mut c, "objectTypeUI window1;"), json!("floatingWindow"));
    assert_eq!(ok(&mut c, "window -q -vis window1;"), json!(false));
    ok(&mut c, "showWindow window1;");
    assert_eq!(ok(&mut c, "window -q -vis window1;"), json!(true));
    assert_eq!(ok(&mut c, "objectTypeUI \"window1|rowLayout1|text1\";"), json!("staticText"));
    // Two with one name in one parent is an error; in another parent it is fine.
    assert!(c.mel_checked("text text1;").is_err());
    ok(&mut c, "setParent ..; columnLayout other; text text1;");
    assert_eq!(ok(&mut c, "control -q -fpn text1;"), json!("window1|other|text1"));
    assert_eq!(ok(&mut c, "layout -q -ca window1;"), json!(["columnLayout1", "rowLayout1", "other"]));
    assert_eq!(ok(&mut c, "control -q -p \"window1|other\";"), json!("window1"));
    // No window: no controls.
    ok(&mut c, "deleteUI -window window1;");
    assert_eq!(ok(&mut c, "window -q -ex window1;"), json!(false));
    assert!(c.mel_checked("button;").is_err());
    assert!(c.mel_checked("deleteUI nothingHere;").is_err());
    errors(&mut c);
}

#[test]
fn create_edit_query_on_the_controls() {
    let mut c = Core::new();
    ok(&mut c, "window -t \"My Tool\" -wh 300 200 -s 0 -mnb 0 -mxb 0 -rtf 1 tool;");
    assert_eq!(ok(&mut c, "window -q -t tool;"), json!("My Tool"));
    assert_eq!(ok(&mut c, "window -q -wh tool;"), json!([300, 200]));
    assert_eq!(ok(&mut c, "window -q -s tool;"), json!(false));
    assert_eq!(ok(&mut c, "window -q -rtf tool;"), json!(true));
    ok(&mut c, "window -e -t \"Renamed\" -w 500 tool;");
    assert_eq!(ok(&mut c, "window -q -t tool;"), json!("Renamed"));
    assert_eq!(ok(&mut c, "window -q -w tool;"), json!(500));
    ok(&mut c, "columnLayout -adj true -rs 4 -cat \"both\" 5 col;");
    assert_eq!(ok(&mut c, "columnLayout -q -adj col;"), json!(true));
    assert_eq!(ok(&mut c, "columnLayout -q -rs col;"), json!(4));

    ok(&mut c, "button -l \"Go\" -w 80 -h 30 -bgc 0.5 0.2 0.1 -ann \"Tip\" -en 0 b;");
    assert_eq!(ok(&mut c, "button -q -l b;"), json!("Go"));
    assert_eq!(ok(&mut c, "button -q -w b;"), json!(80));
    assert_eq!(ok(&mut c, "button -q -bgc b;"), json!([0.5, 0.2, 0.1]));
    assert_eq!(ok(&mut c, "button -q -ann b;"), json!("Tip"));
    assert_eq!(ok(&mut c, "button -q -en b;"), json!(false));
    ok(&mut c, "button -e -en 1 -l \"Stop\" b;");
    assert_eq!(ok(&mut c, "button -q -en b;"), json!(true));
    assert_eq!(ok(&mut c, "button -q -l b;"), json!("Stop"));
    // A label defaults to the name.
    ok(&mut c, "button unlabelled;");
    assert_eq!(ok(&mut c, "button -q -l unlabelled;"), json!("unlabelled"));

    ok(&mut c, "textField -tx \"hello\" -pht \"type\" f;");
    assert_eq!(ok(&mut c, "textField -q -tx f;"), json!("hello"));
    assert_eq!(ok(&mut c, "textField -q -pht f;"), json!("type"));
    ok(&mut c, "textField -e -tx \"bye\" f;");
    assert_eq!(ok(&mut c, "textField -q -tx f;"), json!("bye"));
    ok(&mut c, "textFieldGrp -l \"Name\" -tx \"x\" -cw2 80 100 -ad2 2 tg;");
    assert_eq!(ok(&mut c, "textFieldGrp -q -tx tg;"), json!("x"));
    assert_eq!(ok(&mut c, "textFieldGrp -q -l tg;"), json!("Name"));
    ok(&mut c, "textFieldButtonGrp -l \"Path\" -bl \"Browse\" -tx \"/a\" tb;");
    assert_eq!(ok(&mut c, "textFieldButtonGrp -q -bl tb;"), json!("Browse"));
    ok(&mut c, "scrollField -tx \"long\" -ww 1 -ed 0 sf;");
    assert_eq!(ok(&mut c, "scrollField -q -ed sf;"), json!(false));

    ok(&mut c, "intField -v 3 -min 0 -max 9 i;");
    assert_eq!(ok(&mut c, "intField -q -v i;"), json!(3));
    ok(&mut c, "floatField -v 2 -pre 2 ff;");
    assert_eq!(ok(&mut c, "floatField -q -v ff;"), json!(2.0));
    ok(&mut c, "intFieldGrp -nf 3 -l \"Count\" -v1 1 -v2 2 -v3 3 ig;");
    assert_eq!(ok(&mut c, "intFieldGrp -q -v2 ig;"), json!(2));
    assert_eq!(ok(&mut c, "intFieldGrp -q -v ig;"), json!([1, 2, 3]));
    ok(&mut c, "floatFieldGrp -nf 2 -v 1.5 2.5 0 0 fg;");
    assert_eq!(ok(&mut c, "floatFieldGrp -q -v1 fg;"), json!(1.5));
    assert_eq!(ok(&mut c, "floatFieldGrp -q -v fg;"), json!([1.5, 2.5]));

    ok(&mut c, "intSlider -min 0 -max 10 -v 4 is;");
    assert_eq!(ok(&mut c, "intSlider -q -v is;"), json!(4));
    assert_eq!(ok(&mut c, "intSlider -q -max is;"), json!(10));
    ok(&mut c, "floatSlider -min 0 -max 2 -v 0.5 fs;");
    assert_eq!(ok(&mut c, "floatSlider -q -v fs;"), json!(0.5));
    ok(&mut c, "floatSliderGrp -l \"Weight\" -f 1 -min 0 -max 1 -v 0.25 -pre 3 -cw3 80 50 100 fsg;");
    assert_eq!(ok(&mut c, "floatSliderGrp -q -v fsg;"), json!(0.25));
    assert_eq!(ok(&mut c, "floatSliderGrp -q -f fsg;"), json!(true));
    ok(&mut c, "intSliderGrp -l \"Spans\" -f 1 -min 1 -max 8 -v 2 isg;");
    assert_eq!(ok(&mut c, "intSliderGrp -q -v isg;"), json!(2));

    ok(&mut c, "checkBox -l \"On\" -v 1 cb;");
    assert_eq!(ok(&mut c, "checkBox -q -v cb;"), json!(true));
    ok(&mut c, "checkBoxGrp -ncb 3 -l \"Axes\" -la3 \"X\" \"Y\" \"Z\" -va3 1 0 1 cbg;");
    assert_eq!(ok(&mut c, "checkBoxGrp -q -l2 cbg;"), json!("Y"));
    assert_eq!(ok(&mut c, "checkBoxGrp -q -v3 cbg;"), json!(true));
    assert_eq!(ok(&mut c, "checkBoxGrp -q -v2 cbg;"), json!(false));

    ok(&mut c, "text -l \"Note\" -al \"left\" -fn \"boldLabelFont\" t;");
    assert_eq!(ok(&mut c, "text -q -l t;"), json!("Note"));
    assert_eq!(ok(&mut c, "text -q -al t;"), json!("left"));
    ok(&mut c, "colorSliderGrp -l \"Colour\" -rgb 1 0 0 csg;");
    assert_eq!(ok(&mut c, "colorSliderGrp -q -rgb csg;"), json!([1, 0, 0]));
    ok(&mut c, "colorIndexSliderGrp -l \"Index\" -min 1 -max 31 -v 4 cig;");
    assert_eq!(ok(&mut c, "colorIndexSliderGrp -q -v cig;"), json!(4));
    ok(&mut c, "progressBar -max 10 -pr 2 pb;");
    ok(&mut c, "progressBar -e -step 3 pb;");
    assert_eq!(ok(&mut c, "progressBar -q -pr pb;"), json!(5));
    ok(&mut c, "separator -st \"in\" -h 4 sep; image -i \"x.png\" im; canvas -rgb 1 1 0 cv; iconTextButton -st \"textOnly\" -l \"I\" itb; symbolButton -i \"s.png\" sb;");
    assert_eq!(ok(&mut c, "image -q -i im;"), json!("x.png"));
    assert_eq!(ok(&mut c, "iconTextButton -q -st itb;"), json!("textOnly"));
    errors(&mut c);
}

#[test]
fn layouts_keep_their_flags() {
    let mut c = Core::new();
    ok(&mut c, "window w; formLayout -nd 200 form;");
    assert_eq!(ok(&mut c, "formLayout -q -nd form;"), json!(200));
    ok(&mut c, "button a; button b;");
    ok(&mut c, "formLayout -e -af a \"top\" 5 -af a \"left\" 5 -ac b \"top\" 2 a -ap b \"left\" 3 50 -an b \"bottom\" form;");
    let af = ok(&mut c, "formLayout -q -af form;");
    assert_eq!(af.as_array().unwrap().len(), 2);
    assert_eq!(af[0]["offset"], 5);
    // A second attachment of one edge replaces the first.
    ok(&mut c, "formLayout -e -af a \"top\" 9 form;");
    let af = ok(&mut c, "formLayout -q -af form;");
    assert_eq!(af.as_array().unwrap().len(), 2);
    assert!(c.mel_checked("formLayout -e -af nobody \"top\" 1 form;").is_err());
    assert!(c.mel_checked("formLayout -e -af a \"middle\" 1 form;").is_err());
    ok(&mut c, "setParent ..;");

    ok(&mut c, "rowColumnLayout -nc 2 -cw 1 100 -cw 2 200 -cal 1 \"right\" rc;");
    assert_eq!(ok(&mut c, "rowColumnLayout -q -nc rc;"), json!(2));
    assert_eq!(ok(&mut c, "rowColumnLayout -q -cw rc;"), json!([1, 100, 2, 200]));
    ok(&mut c, "setParent ..;");
    ok(&mut c, "gridLayout -nr 2 -nc 3 -cwh 40 40 g; setParent ..; flowLayout -wr 1 fl; setParent ..;");
    assert_eq!(ok(&mut c, "gridLayout -q -nc g;"), json!(3));
    ok(&mut c, "frameLayout -l \"Options\" -cll 1 -cl 1 -mh 3 -mw 2 fr; setParent ..;");
    assert_eq!(ok(&mut c, "frameLayout -q -cl fr;"), json!(true));
    assert_eq!(ok(&mut c, "frameLayout -q -l fr;"), json!("Options"));
    ok(&mut c, "frameLayout -e -cl 0 fr;");
    assert_eq!(ok(&mut c, "frameLayout -q -cl fr;"), json!(false));
    ok(&mut c, "scrollLayout -cr 1 sc; setParent ..;");
    ok(&mut c, "tabLayout -imw 2 tabs; columnLayout one; setParent ..; columnLayout two; setParent ..;");
    ok(&mut c, "tabLayout -e -tl one \"First\" -tl two \"Second\" -sti 2 tabs;");
    assert_eq!(ok(&mut c, "tabLayout -q -tl tabs;"), json!(["First", "Second"]));
    assert_eq!(ok(&mut c, "tabLayout -q -sti tabs;"), json!(2));
    assert_eq!(ok(&mut c, "tabLayout -q -st tabs;"), json!("two"));
    ok(&mut c, "tabLayout -e -st \"one\" tabs;");
    assert_eq!(ok(&mut c, "tabLayout -q -sti tabs;"), json!(1));
    ok(&mut c, "setParent ..;");
    ok(&mut c, "paneLayout -cn \"vertical2\" pn; setParent ..; shelfLayout sh; setParent ..; menuBarLayout mb; setParent ..;");
    assert_eq!(ok(&mut c, "paneLayout -q -cn pn;"), json!("vertical2"));
    assert_eq!(ok(&mut c, "layout -q -nch w;"), json!(10));
    errors(&mut c);
}

#[test]
fn menus_lists_and_radios() {
    let mut c = Core::new();
    ok(&mut c, "window -mb 1 w; columnLayout;");
    ok(&mut c, "menu -l \"File\" fileMenu;");
    assert_eq!(ok(&mut c, "menuItem -l \"Open\" -c \"polyCube -n fromMenu\" open;"), json!("w|fileMenu|open"));
    ok(&mut c, "menuItem -d 1; menuItem -l \"Check\" -cb 1 chk; menuItem -l \"More\" -sm 1 more; menuItem -l \"Deep\" deep; setParent -m ..; menuItem -l \"After\" after;");
    assert_eq!(ok(&mut c, "menuItem -q -cb chk;"), json!(true));
    assert_eq!(ok(&mut c, "menu -q -ni fileMenu;"), json!(5));
    assert_eq!(ok(&mut c, "control -q -fpn deep;"), json!("w|fileMenu|more|deep"));
    assert_eq!(ok(&mut c, "control -q -fpn after;"), json!("w|fileMenu|after"));
    press(&mut c, "w|fileMenu|open", "command", json!([false]));
    assert_eq!(ok(&mut c, "objExists fromMenu;"), json!(true));
    // Popup and menus on the application's own window.
    ok(&mut c, "popupMenu -b 3 pop; menuItem -l \"X\" -p pop;");
    assert_eq!(ok(&mut c, "popupMenu -q -b pop;"), json!(3));
    ok(&mut c, "global string $gMainWindow; menu -p $gMainWindow -l \"My Tools\" toolsMenu;");
    assert_eq!(ok(&mut c, "control -q -fpn toolsMenu;"), json!("MayaWindow|toolsMenu"));
    assert_eq!(ok(&mut c, "objectTypeUI MayaWindow;"), json!("floatingWindow"));
    assert!(c.mel_checked("deleteUI MayaWindow;").is_err());
    ok(&mut c, "deleteUI -menu toolsMenu;");

    ok(&mut c, "optionMenu -l \"Axis\" om; menuItem -l \"X\"; menuItem -l \"Y\"; menuItem -l \"Z\";");
    assert_eq!(ok(&mut c, "optionMenu -q -v om;"), json!("X"));
    assert_eq!(ok(&mut c, "optionMenu -q -ni om;"), json!(3));
    ok(&mut c, "optionMenu -e -v \"Z\" om;");
    assert_eq!(ok(&mut c, "optionMenu -q -sl om;"), json!(3));
    ok(&mut c, "optionMenu -e -sl 2 om;");
    assert_eq!(ok(&mut c, "optionMenu -q -v om;"), json!("Y"));
    assert!(c.mel_checked("optionMenu -e -v \"W\" om;").is_err());
    ok(&mut c, "optionMenuGrp -l \"Mode\" omg; menuItem -l \"A\"; menuItem -l \"B\" -p \"omg|OptionMenu\";");
    assert_eq!(ok(&mut c, "optionMenuGrp -q -ni omg;"), json!(2));

    ok(&mut c, "textScrollList -nr 4 -ams 1 -a \"one\" -a \"two\" -a \"three\" -sii 2 l;");
    assert_eq!(ok(&mut c, "textScrollList -q -ni l;"), json!(3));
    assert_eq!(ok(&mut c, "textScrollList -q -si l;"), json!(["two"]));
    assert_eq!(ok(&mut c, "textScrollList -q -sii l;"), json!([2]));
    ok(&mut c, "textScrollList -e -si \"three\" l;");
    assert_eq!(ok(&mut c, "textScrollList -q -si l;"), json!(["two", "three"]));
    ok(&mut c, "textScrollList -e -ri \"two\" l;");
    assert_eq!(ok(&mut c, "textScrollList -q -ai l;"), json!(["one", "three"]));
    assert_eq!(ok(&mut c, "textScrollList -q -sii l;"), json!([2]));
    ok(&mut c, "textScrollList -e -da -ra l;");
    assert_eq!(ok(&mut c, "textScrollList -q -ni l;"), json!(0));
    // A click, as the shell reports it.
    ok(&mut c, "textScrollList -e -a \"p\" -a \"q\" l;");
    c.ui_event(&json!({"path": "w|columnLayout1|l", "set": {"selected": [1]}}));
    assert_eq!(ok(&mut c, "textScrollList -q -si l;"), json!(["q"]));

    ok(&mut c, "radioCollection rc; radioButton -l \"A\" ra; radioButton -l \"B\" -sl rb;");
    assert_eq!(ok(&mut c, "radioCollection -q -sl rc;"), json!("rb"));
    ok(&mut c, "radioButton -e -sl ra;");
    assert_eq!(ok(&mut c, "radioCollection -q -sl rc;"), json!("ra"));
    assert_eq!(ok(&mut c, "radioButton -q -sl rb;"), json!(false));
    ok(&mut c, "radioCollection -e -sl \"rb\" rc;");
    assert_eq!(ok(&mut c, "radioButton -q -sl ra;"), json!(false));
    assert_eq!(ok(&mut c, "radioCollection -q -nci rc;"), json!(2));
    ok(&mut c, "radioButtonGrp -nrb 3 -l \"Pick\" -la3 \"a\" \"b\" \"c\" -sl 2 rg;");
    assert_eq!(ok(&mut c, "radioButtonGrp -q -sl rg;"), json!(2));
    errors(&mut c);
}

#[test]
fn callbacks_run_mel_with_the_value() {
    let mut c = Core::new();
    ok(&mut c, "global int $hit = 0; global string $got = \"\"; global proc noteIt(string $v) { global string $got; $got = $v; }");
    ok(&mut c, "window w; columnLayout; button -c \"polyCube -n pressed\" b;");
    press(&mut c, "w|columnLayout1|b", "command", json!([false]));
    assert_eq!(ok(&mut c, "objExists pressed;"), json!(true));
    // One press, one undo step.
    ok(&mut c, "button -e -c \"polyCube -n p1; polyCube -n p2\" b;");
    press(&mut c, "w|columnLayout1|b", "command", json!([false]));
    assert_eq!(ok(&mut c, "objExists p2;"), json!(true));
    ok(&mut c, "undo;");
    assert_eq!(ok(&mut c, "objExists p1;"), json!(false));
    ok(&mut c, "checkBox -cc \"noteIt \\\"#1\\\"\" cb;");
    c.ui_event(&json!({"path": "w|columnLayout1|cb", "set": {"value": true}, "fire": ["changeCommand"], "args": [true]}));
    assert_eq!(ok(&mut c, "checkBox -q -v cb;"), json!(true));
    let got = c.mel("global string $got; $got;");
    assert_eq!(got, json!("1"));
    ok(&mut c, "floatSlider -cc \"noteIt \\\"%1\\\"\" fs;");
    c.ui_event(&json!({"path": "w|columnLayout1|fs", "set": {"value": 0.5}, "fire": ["changeCommand"], "args": [0.5]}));
    assert_eq!(c.mel("global string $got; $got;"), json!("0.5"));
    assert_eq!(ok(&mut c, "floatSlider -q -v fs;"), json!(0.5));
    // An error in a callback lands in the history and leaves the core working.
    ok(&mut c, "button -e -c \"nosuchproc\" b;");
    errors(&mut c);
    press(&mut c, "w|columnLayout1|b", "command", json!([false]));
    assert!(errors(&mut c).iter().any(|e| e.contains("nosuchproc")));
    // A closed window goes, and its close command runs.
    ok(&mut c, "window -cc \"polyCube -n closed\" w2;");
    c.ui_event(&json!({"path": "w2", "closed": true}));
    assert_eq!(ok(&mut c, "window -q -ex w2;"), json!(false));
    assert_eq!(ok(&mut c, "objExists closed;"), json!(true));
}

#[test]
fn dialogs_answer_in_a_batch_run() {
    let mut c = Core::new();
    assert_eq!(ok(&mut c, "confirmDialog -t \"Sure\" -m \"Really?\" -b \"Yes\" -b \"No\" -db \"No\" -cb \"No\" -ds \"No\";"), json!("No"));
    assert_eq!(ok(&mut c, "confirmDialog -m \"x\" -b \"Fine\";"), json!("Fine"));
    assert_eq!(ok(&mut c, "promptDialog -t \"Name\" -tx \"abc\" -b \"OK\" -db \"OK\";"), json!("OK"));
    assert_eq!(ok(&mut c, "promptDialog -q -tx;"), json!("abc"));
    assert_eq!(ok(&mut c, "fileDialog2 -fm 1;"), Value::Null);
    assert_eq!(ok(&mut c, "fileDialog;"), json!(""));
    ok(&mut c, "progressWindow -t \"Work\" -pr 0 -st \"Going\" -ii 1;");
    assert_eq!(ok(&mut c, "progressWindow -q -ic;"), json!(false));
    ok(&mut c, "progressWindow -e -pr 40;");
    assert_eq!(ok(&mut c, "progressWindow -q -pr;"), json!(40));
    ok(&mut c, "progressWindow -e -s 5;");
    assert_eq!(ok(&mut c, "progressWindow -q -pr;"), json!(45));
    ok(&mut c, "progressWindow -ep;");
    assert_eq!(ok(&mut c, "progressWindow -q -pr;"), json!(0));
    aztek_host::take_ui_requests_for_tests();
    ok(&mut c, "headsUpMessage \"Done\" -t 2;");
    ok(&mut c, "inViewMessage -amg \"Hello\" -pos \"midCenter\" -fade;");
    let reqs = aztek_host::take_ui_requests_for_tests();
    assert_eq!(reqs.as_array().unwrap().iter().filter(|r| r.as_str().is_some_and(|t| t.starts_with("inViewMessage|"))).count(), 2);
}

#[test]
fn templates_and_small_commands() {
    let mut c = Core::new();
    ok(&mut c, "uiTemplate tpl; button -dt tpl -h 44;");
    assert_eq!(ok(&mut c, "uiTemplate -ex tpl;"), json!(true));
    ok(&mut c, "window w; columnLayout; setUITemplate -pushTemplate tpl;");
    ok(&mut c, "button b; setUITemplate -popTemplate; button c;");
    assert_eq!(ok(&mut c, "button -q -h b;"), json!(44));
    assert_eq!(ok(&mut c, "button -q -h c;"), json!(22));
    // The language of a callback's string can be said.
    ok(&mut c, "iconTextButton -stp \"python\" -c \"import maya.cmds; maya.cmds.polyCube(n='stpCube')\" stp;");
    #[cfg(feature = "python")]
    {
        c.ui_event(&json!({"path": "w|columnLayout1|stp", "fire": ["command"], "args": [false]}));
        assert_eq!(ok(&mut c, "objExists stpCube;"), json!(true));
    }
    assert_eq!(ok(&mut c, "workspaceControlState -ex anyName;"), json!(false));
    ok(&mut c, "button -ut tpl d;");
    assert_eq!(ok(&mut c, "button -q -h d;"), json!(44));
    assert_eq!(ok(&mut c, "waitCursor -st 1;"), json!(true));
    assert_eq!(ok(&mut c, "waitCursor -q -st;"), json!(true));
    ok(&mut c, "waitCursor -st 0;");
    assert_eq!(ok(&mut c, "windowPref -ex w;"), json!(false));
    ok(&mut c, "global proc buildDock() { columnLayout inDock; button dockButton; }");
    assert_eq!(ok(&mut c, "workspaceControl -l \"Dock\" -uiScript \"buildDock\" dock;"), json!("dock"));
    assert_eq!(ok(&mut c, "control -q -fpn dockButton;"), json!("dock|inDock|dockButton"));
    assert_eq!(ok(&mut c, "workspaceControl -q -l dock;"), json!("Dock"));
}

#[test]
fn the_shell_gets_operations_in_order() {
    let mut c = Core::new();
    aztek_host::scriptui::take_ops();
    ok(&mut c, "window w; columnLayout; button -l \"A\" -c \"x\" b; showWindow w; button -e -l \"B\" b; deleteUI b;");
    let ops = aztek_host::scriptui::take_ops();
    let kinds: Vec<&str> = ops.iter().map(|o| o["op"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["create", "create", "create", "show", "edit", "delete"]);
    assert_eq!(ops[2]["kind"], "button");
    assert_eq!(ops[2]["parent"], "w|columnLayout1");
    assert_eq!(ops[2]["props"]["label"], "A");
    // Scripts stay in the core.
    assert!(ops[2]["props"].get("command").is_none());
    assert_eq!(ops[4]["props"]["label"], "B");
}

#[test]
fn script_jobs_fire_at_idle_and_die_when_killed() {
    let mut c = Core::new();
    ok(&mut c, "global int $n = 0; global proc bump() { global int $n; $n += 1; } polyCube -n a; polyCube -n b;");
    let count = |c: &mut Core| c.mel("global int $n; $n;").as_i64().unwrap();
    let id = ok(&mut c, "scriptJob -e \"SelectionChanged\" \"bump\";").as_i64().unwrap();
    c.idle();
    assert_eq!(count(&mut c), 0);
    ok(&mut c, "select a;");
    assert_eq!(count(&mut c), 0, "jobs wait for idle");
    c.idle();
    assert_eq!(count(&mut c), 1);
    c.idle();
    assert_eq!(count(&mut c), 1);
    assert_eq!(ok(&mut c, &format!("scriptJob -ex {id};")), json!(true));
    assert!(ok(&mut c, "scriptJob -lj;").as_array().unwrap().iter().any(|l| l.as_str().unwrap().contains("SelectionChanged")));
    ok(&mut c, &format!("scriptJob -k {id};"));
    assert_eq!(ok(&mut c, &format!("scriptJob -ex {id};")), json!(false));
    ok(&mut c, "select b;");
    c.idle();
    assert_eq!(count(&mut c), 1);
    assert!(c.mel_checked(&format!("scriptJob -k {id};")).is_err());
    assert!(c.mel_checked("scriptJob -e \"NotAnEvent\" \"bump\";").is_err());

    // Run once.
    ok(&mut c, "scriptJob -ro 1 -e \"timeChanged\" \"bump\";");
    ok(&mut c, "currentTime 57;");
    c.idle();
    assert_eq!(count(&mut c), 2);
    ok(&mut c, "currentTime 58;");
    c.idle();
    assert_eq!(count(&mut c), 2);

    // Undo and redo.
    ok(&mut c, "scriptJob -e \"Undo\" \"bump\"; polyCube -n forUndo;");
    ok(&mut c, "undo;");
    c.idle();
    assert_eq!(count(&mut c), 3);

    // Attribute change; the job dies with the node.
    let ac = ok(&mut c, "scriptJob -ac \"a.tx\" \"bump\";").as_i64().unwrap();
    c.idle();
    assert_eq!(count(&mut c), 3);
    ok(&mut c, "setAttr a.tx 3;");
    c.idle();
    assert_eq!(count(&mut c), 4);
    // Node deleted.
    ok(&mut c, "scriptJob -nd b \"bump\"; delete b;");
    c.idle();
    assert_eq!(count(&mut c), 5);
    ok(&mut c, "delete a;");
    c.idle();
    assert_eq!(ok(&mut c, &format!("scriptJob -ex {ac};")), json!(false));

    // Condition.
    ok(&mut c, "select -cl; scriptJob -ct \"SomethingSelected\" \"bump\"; polyCube -n d; select -cl;");
    c.idle();
    let before = count(&mut c);
    ok(&mut c, "select d;");
    c.idle();
    assert_eq!(count(&mut c), before + 1);

    // Kill with the scene, and with a control; protected ones need -f.
    let kws = ok(&mut c, "scriptJob -kws -e \"timeChanged\" \"bump\";").as_i64().unwrap();
    let keep = ok(&mut c, "scriptJob -pro -e \"timeChanged\" \"bump\";").as_i64().unwrap();
    ok(&mut c, "window w; columnLayout col;");
    let tied = ok(&mut c, "scriptJob -p col -e \"timeChanged\" \"bump\";").as_i64().unwrap();
    ok(&mut c, "deleteUI w;");
    c.idle();
    assert_eq!(ok(&mut c, &format!("scriptJob -ex {tied};")), json!(false));
    let seen_new = ok(&mut c, "global int $opened = 0; scriptJob -e \"NewSceneOpened\" \"global int $opened; $opened += 1;\";").as_i64().unwrap();
    ok(&mut c, "file -new -f;");
    c.idle();
    assert_eq!(ok(&mut c, &format!("scriptJob -ex {kws};")), json!(false));
    assert_eq!(ok(&mut c, &format!("scriptJob -ex {seen_new};")), json!(true));
    assert_eq!(c.mel("global int $opened; $opened;"), json!(1));
    assert!(c.mel_checked(&format!("scriptJob -k {keep};")).is_err());
    ok(&mut c, &format!("scriptJob -k {keep} -f;"));
    ok(&mut c, "scriptJob -ka;");
    assert_eq!(ok(&mut c, "scriptJob -lj;"), json!([]));
    errors(&mut c);
}

#[test]
fn a_job_at_idle_leaves_undo_alone() {
    let mut c = Core::new();
    ok(&mut c, "polyCube -n u; scriptJob -e \"SelectionChanged\" \"\"; scriptJob -ct \"SomethingSelected\" \"\";");
    c.idle();
    c.idle();
    ok(&mut c, "undo;");
    assert_eq!(ok(&mut c, "objExists u;"), json!(false));
    c.idle();
    ok(&mut c, "redo;");
    assert_eq!(ok(&mut c, "objExists u;"), json!(true));
    ok(&mut c, "scriptJob -ka;");
}

#[test]
fn script_nodes_run_and_save_with_the_scene() {
    let dir = std::env::temp_dir().join(format!("aztek_script_ui_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["ma", "aztek"] {
        let path = dir.join(format!("scene.{ext}")).display().to_string();
        let mut c = Core::new();
        ok(&mut c, "scriptNode -st 1 -bs \"polyCube -n onOpen\" -as \"polyCube -n onClose\" -n myScript;");
        assert_eq!(ok(&mut c, "scriptNode -q -bs myScript;"), json!("polyCube -n onOpen"));
        assert_eq!(ok(&mut c, "scriptNode -q -st myScript;"), json!(1));
        assert_eq!(ok(&mut c, "scriptNode -q -stp myScript;"), json!("mel"));
        assert_eq!(ok(&mut c, "objExists onOpen;"), json!(false));
        ok(&mut c, "scriptNode -eb myScript;");
        assert_eq!(ok(&mut c, "objExists onOpen;"), json!(true), "-eb runs it");
        ok(&mut c, "delete onOpen;");
        c.save(&path).unwrap();
        let mut d = Core::new();
        d.open(&path).unwrap();
        assert_eq!(ok(&mut d, "objExists onOpen;"), json!(true), "{ext}: run on open");
        assert_eq!(ok(&mut d, "objExists myScript;"), json!(true));
        assert_eq!(ok(&mut d, "scriptNode -q -as myScript;"), json!("polyCube -n onClose"));
        // Closing the scene runs the after script, in the scene it is leaving, which goes with it.
        ok(&mut d, "scriptNode -e -as \"global int $closed; $closed = 1;\" myScript;");
        ok(&mut d, "file -new -f;");
        assert_eq!(d.mel("global int $closed; $closed;"), json!(1));
        assert_eq!(ok(&mut d, "objExists myScript;"), json!(false));
        errors(&mut d);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn timer_stack_trace_and_last_error() {
    let mut c = Core::new();
    ok(&mut c, "timer -s -n t1;");
    let lap = ok(&mut c, "timer -lap -n t1;").as_f64().unwrap();
    assert!(lap >= 0.0);
    let end = ok(&mut c, "timer -e -n t1;").as_f64().unwrap();
    assert!(end >= 0.0);
    assert!(c.mel_checked("timer -e -n t1;").is_err());
    assert_eq!(ok(&mut c, "stackTrace -q -st;"), json!(false));
    ok(&mut c, "stackTrace -st 1;");
    assert_eq!(ok(&mut c, "stackTrace -q -st;"), json!(true));
    ok(&mut c, "stackTrace -st 0;");
    let _ = c.mel_checked("error \"boom\";");
    assert_eq!(ok(&mut c, "getLastError;"), json!("boom"));
    errors(&mut c);
}

#[test]
fn panels_and_editors() {
    let mut c = Core::new();
    assert_eq!(ok(&mut c, "getPanel -wf;"), json!("modelPanel4"));
    assert_eq!(ok(&mut c, "getPanel -typ \"modelPanel\";"), json!(["modelPanel1", "modelPanel2", "modelPanel3", "modelPanel4"]));
    assert_eq!(ok(&mut c, "getPanel -to modelPanel1;"), json!("modelPanel"));
    assert_eq!(ok(&mut c, "getPanel -wl \"Top View\";"), json!("modelPanel1"));
    assert_eq!(ok(&mut c, "getPanel -vis;"), json!(["modelPanel4"]));
    assert_eq!(ok(&mut c, "getPanel -con persp;"), json!("modelPanel4"));
    assert!(ok(&mut c, "getPanel -all;").as_array().unwrap().len() >= 4);
    assert_eq!(ok(&mut c, "modelPanel -q -cam modelPanel4;"), json!("persp"));
    assert_eq!(ok(&mut c, "modelPanel -q -cam modelPanel1;"), json!("top"));
    assert_eq!(ok(&mut c, "modelPanel -q -ex modelPanel1;"), json!(true));
    assert_eq!(ok(&mut c, "modelPanel -q -ex notThere;"), json!(false));
    aztek_host::take_ui_requests_for_tests();
    ok(&mut c, "modelPanel -e -cam front modelPanel4;");
    assert_eq!(ok(&mut c, "modelEditor -q -cam modelPanel4;"), json!("front"));
    ok(&mut c, "modelEditor -e -cam persp -da \"wireframe\" -wos 1 -grid 0 -joints 0 -nurbsCurves 0 -polymeshes 1 -locators 0 -deformers 0 -ikHandles 0 -cameras 0 -manipulators 0 -dtx 1 modelPanel4;");
    assert_eq!(ok(&mut c, "modelEditor -q -da modelPanel4;"), json!("wireframe"));
    assert_eq!(ok(&mut c, "modelEditor -q -wos modelPanel4;"), json!(true));
    assert_eq!(ok(&mut c, "modelEditor -q -grid modelPanel4;"), json!(false));
    assert_eq!(ok(&mut c, "modelEditor -q -joints modelPanel4;"), json!(false));
    assert_eq!(ok(&mut c, "modelEditor -q -polymeshes modelPanel4;"), json!(true));
    assert_eq!(ok(&mut c, "modelEditor -q -dtx modelPanel4;"), json!(true));
    let reqs = aztek_host::take_ui_requests_for_tests();
    let has = |needle: &str| reqs.as_array().unwrap().iter().any(|r| r.as_str().is_some_and(|t| t.contains(needle)));
    assert!(has("\"key\":\"wireframe\""), "{reqs}");
    assert!(has("\"key\":\"grid\""), "{reqs}");
    assert!(has("\"joints\":false"), "{reqs}");
    ok(&mut c, "modelEditor -e -alo 0 -polymeshes 1 modelPanel1;");
    assert_eq!(ok(&mut c, "modelEditor -q -nurbsCurves modelPanel1;"), json!(false));
    assert_eq!(ok(&mut c, "modelEditor -q -polymeshes modelPanel1;"), json!(true));
    // A panel's camera and what the shell reports.
    ok(&mut c, "lookThru side;");
    assert_eq!(ok(&mut c, "lookThru -q;"), json!("side"));
    assert_eq!(ok(&mut c, "modelPanel -q -cam modelPanel4;"), json!("side"));
    ok(&mut c, "lookThru top modelPanel3;");
    assert_eq!(ok(&mut c, "modelPanel -q -cam modelPanel3;"), json!("top"));
    aztek_host::panel_cmds::shell_sync(&json!({"focus": "modelPanel2", "underPointer": "modelPanel3", "panels": [{"name": "modelPanel2", "camera": "persp", "visible": true, "wireframe": false, "disp": {"joints": false, "flat": true}}]}));
    assert_eq!(ok(&mut c, "getPanel -wf;"), json!("modelPanel2"));
    assert_eq!(ok(&mut c, "getPanel -up;"), json!("modelPanel3"));
    assert_eq!(ok(&mut c, "modelPanel -q -cam modelPanel2;"), json!("persp"));
    assert_eq!(ok(&mut c, "modelEditor -q -da modelPanel2;"), json!("flatShaded"));
    assert_eq!(ok(&mut c, "modelEditor -q -joints modelPanel2;"), json!(false));

    // A panel in a script's layout, and a scripted type.
    ok(&mut c, "window w; columnLayout cl;");
    assert_eq!(ok(&mut c, "modelPanel -cam side myView;"), json!("myView"));
    assert_eq!(ok(&mut c, "control -q -fpn myView;"), json!("w|cl|myView"));
    assert_eq!(ok(&mut c, "modelPanel -q -cam myView;"), json!("side"));
    ok(&mut c, "global proc myAdd(string $p) { setParent $p; button -l \"in panel\" pb; } scriptedPanelType -acb \"myAdd\" myType;");
    ok(&mut c, "scriptedPanel -typ \"myType\" -l \"Mine\" -p w mine;");
    assert_eq!(ok(&mut c, "control -q -fpn pb;"), json!("w|mine|pb"));
    assert_eq!(ok(&mut c, "scriptedPanel -q -typ mine;"), json!("myType"));
    assert_eq!(ok(&mut c, "getPanel -sty \"myType\";"), json!(["mine"]));
    errors(&mut c);
    aztek_host::panel_cmds::shell_sync(&json!({"focus": "modelPanel4", "underPointer": "modelPanel4"}));
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

fn tx(c: &mut Core, cam: &str) -> [f64; 3] {
    let v = ok(c, &format!("getAttr {cam}.translate;"));
    let v = v.as_array().unwrap();
    let n = |x: &Value| x.as_f64().unwrap();
    [n(&v[0]), n(&v[1]), n(&v[2])]
}

#[test]
fn camera_moves_work_on_the_camera_node() {
    let mut c = Core::new();
    assert_eq!(ok(&mut c, "listCameras;"), json!(["front", "persp", "side", "top"]).as_array().map(|_| ok(&mut Core::new(), "listCameras;")).unwrap());
    let persp = ok(&mut c, "listCameras -p;");
    assert_eq!(persp, json!(["persp"]));
    let ortho = ok(&mut c, "listCameras -o;");
    assert_eq!(ortho.as_array().unwrap().len(), 3);

    ok(&mut c, "camera -n shot; rename shot shot;");
    let cam = ok(&mut c, "ls -type camera;");
    assert!(cam.as_array().unwrap().iter().any(|x| x.as_str().unwrap().starts_with("shot")));
    ok(&mut c, "setAttr shot.tz 10; camera -e -coi 10 shot;");
    // Dolly in, along -Z here: the eye comes closer, the centre stays.
    ok(&mut c, "dolly -d 4 shot;");
    assert!(near(tx(&mut c, "shot")[2], 6.0));
    assert!(near(ok(&mut c, "camera -q -coi shot;").as_f64().unwrap(), 6.0));
    ok(&mut c, "dolly -abs -d 10 shot;");
    assert!(near(tx(&mut c, "shot")[2], 10.0));
    ok(&mut c, "track -r 2 -u 1 shot;");
    let t = tx(&mut c, "shot");
    assert!(near(t[0], 2.0) && near(t[1], 1.0));
    ok(&mut c, "track -l 2 -d 1 shot;");
    ok(&mut c, "roll -d 15 shot;");
    assert!(near(ok(&mut c, "getAttr shot.rz;").as_f64().unwrap(), 15.0));
    ok(&mut c, "roll -abs -d 0 shot;");
    // Orbit a quarter turn about the centre at the origin: the eye swings round to +X or -X, distance kept.
    ok(&mut c, "orbit -ra 90 0 shot;");
    let t = tx(&mut c, "shot");
    assert!(near(t[2].abs(), 0.0) && near(t[0].abs(), 10.0), "{t:?}");
    // viewPlace sets all of it.
    ok(&mut c, "viewPlace -eye 0 0 20 -la 0 0 0 shot;");
    let t = tx(&mut c, "shot");
    assert!(near(t[2], 20.0));
    assert!(near(ok(&mut c, "camera -q -coi shot;").as_f64().unwrap(), 20.0));
    let r = ok(&mut c, "getAttr shot.rotate;");
    assert!(near(r[0].as_f64().unwrap(), 0.0) && near(r[1].as_f64().unwrap(), 0.0), "{r}");
    // viewLookAt turns it where it stands.
    ok(&mut c, "viewLookAt -pos 20 0 20 shot;");
    let t2 = tx(&mut c, "shot");
    assert!(near(t2[2], 20.0));
    assert!(near(ok(&mut c, "getAttr shot.ry;").as_f64().unwrap(), -90.0));
    // viewFit: a unit cube fills the view, near the origin.
    ok(&mut c, "viewPlace -eye 0 0 200 -la 0 0 0 shot; polyCube -n fitMe; viewFit -all shot;");
    let far = tx(&mut c, "shot")[2];
    assert!(far > 0.5 && far < 20.0, "{far}");
    let before = far;
    ok(&mut c, "viewFit -all -f 0.5 shot;");
    let t = tx(&mut c, "shot")[2];
    assert!(near(t, before * 2.0), "{t} {before}");
    // viewSet puts it on an axis at the same distance from its centre.
    ok(&mut c, "viewSet -top shot;");
    let t = tx(&mut c, "shot");
    assert!(t[1] > 0.5 && near(t[0], 0.0) && near(t[2], 0.0), "{t:?}");
    assert!(near(ok(&mut c, "getAttr shot.rx;").as_f64().unwrap(), -90.0));
    ok(&mut c, "viewSet -front shot;");
    assert!(tx(&mut c, "shot")[2] > 0.5);
    ok(&mut c, "viewSet -side shot;");
    assert!(tx(&mut c, "shot")[0] > 0.5);
    ok(&mut c, "viewSet -home persp;");
    assert!(near(tx(&mut c, "persp")[0], 28.0));
    // Head on to selected faces: looking down their normal, at them.
    ok(&mut c, "select fitMe.f[1]; viewHeadOn shot; select -cl;");
    let t = tx(&mut c, "shot");
    assert!(near(t[0], 0.0), "{t:?}");
    assert!(near(ok(&mut c, "viewFit -an 0 -all shot; getAttr shot.rx;").as_f64().unwrap().abs() % 90.0, 0.0));
    // Clip planes.
    ok(&mut c, "viewClipPlane -ncp 0.5 -fcp 500 shot;");
    assert!(near(ok(&mut c, "viewClipPlane -q -ncp shot;").as_f64().unwrap(), 0.5));
    assert!(near(ok(&mut c, "camera -q -fcp shot;").as_f64().unwrap(), 500.0));
    // Default: the active panel's camera. Script moves undo.
    ok(&mut c, "dolly -d 1;");
    ok(&mut c, "undo;");
    assert!(near(tx(&mut c, "persp")[0], 28.0));
    assert!(c.mel_checked("dolly -d 1 fitMe;").is_err());
    // Playblast with no view writes nothing and says so, and gives the pattern.
    let dir = std::env::temp_dir().join(format!("aztek_blast_{}", std::process::id()));
    let base = dir.join("shot").display().to_string();
    assert_eq!(ok(&mut c, &format!("playblast -f \"{base}\" -fmt \"image\" -st 1 -et 3 -wh 64 64 -v 0 -fp 4 -p 100 -fo -cc 1;")), json!(format!("{base}.####.png")));
    errors(&mut c);
}

#[test]
fn tool_contexts() {
    let mut c = Core::new();
    aztek_host::take_ui_requests_for_tests();
    assert_eq!(ok(&mut c, "currentCtx;"), json!("selectSuperContext"));
    ok(&mut c, "setToolTo moveSuperContext;");
    assert_eq!(ok(&mut c, "currentCtx;"), json!("moveSuperContext"));
    ok(&mut c, "setToolTo RotateSuperContext;");
    ok(&mut c, "setToolTo scaleSuperContext;");
    ok(&mut c, "setToolTo selectSuperContext;");
    ok(&mut c, "setToolTo artAttrSkinContext;");
    let reqs = aztek_host::take_ui_requests_for_tests();
    let list: Vec<&str> = reqs.as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert_eq!(list, ["setTool|move", "setTool|rotate", "setTool|scale", "setTool|select", "setTool|paintSkin"]);
    // Made by a Ctx command, by name or numbered.
    assert_eq!(ok(&mut c, "jointCtx myJoint;"), json!("myJoint"));
    ok(&mut c, "setToolTo myJoint;");
    assert_eq!(ok(&mut c, "currentCtx;"), json!("myJoint"));
    assert_eq!(aztek_host::take_ui_requests_for_tests(), json!(["setTool|joint"]));
    assert_eq!(ok(&mut c, "jointCtx -q -ex myJoint;"), json!(true));
    assert!(c.mel_checked("jointCtx myJoint;").is_err());
    ok(&mut c, "jointCtx -e -sao \"yup\" myJoint;");
    assert_eq!(ok(&mut c, "jointCtx -q -sao myJoint;"), json!("yup"));
    ok(&mut c, "resetTool myJoint;");
    assert_eq!(ok(&mut c, "jointCtx -q -sao myJoint;"), Value::Null);
    let auto = ok(&mut c, "curveCVCtx -d 2;");
    assert_eq!(auto, json!("curveCVCtx1"));
    assert_eq!(ok(&mut c, "contextInfo -c curveCVCtx1;"), json!("curveCVCtx"));
    assert_eq!(ok(&mut c, "contextInfo -ex nothing;"), json!(false));
    // Every context command makes one that setToolTo takes.
    aztek_host::take_ui_requests_for_tests();
    for k in ["ikHandleCtx", "ikSplineHandleCtx", "curveEPCtx", "curveSketchCtx", "insertJointCtx", "polyCutCtx", "polySplitCtx", "polyCreateFacetCtx", "targetWeldCtx", "polySelectEditCtx", "polyShortestPathCtx", "artAttrCtx", "artSelectCtx", "skinBindCtx", "softModContext", "setKeyCtx", "moveKeyCtx", "selectKeyCtx", "insertKeyCtx", "scaleKeyCtx"] {
        let name = format!("{k}Mine");
        assert_eq!(ok(&mut c, &format!("{k} {name};")), json!(name));
        ok(&mut c, &format!("setToolTo {name};"));
        assert_eq!(ok(&mut c, "currentCtx;"), json!(name));
    }
    let reqs = aztek_host::take_ui_requests_for_tests();
    let has = |t: &str| reqs.as_array().unwrap().iter().any(|r| r == t);
    assert!(has("setTool|multiCut") && has("setTool|createPolygon") && has("setTool|targetWeld") && has("setTool|shortestPath") && has("setTool|curveEp") && has("setTool|insertJoint") && has("setTool|paintSelect"), "{reqs}");
    // No tool yet: one warning per kind.
    let warnings: Vec<String> = c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 4).map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert_eq!(warnings.iter().filter(|w| w.contains("ikHandleCtx")).count(), 1, "{warnings:?}");
    ok(&mut c, "setToolTo ikHandleCtxMine;");
    let warnings: Vec<String> = c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 4).map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(c.mel_checked("setToolTo nothingLikeIt;").is_err());
    // The manipulator contexts: mode reaches the views.
    aztek_host::take_ui_requests_for_tests();
    assert_eq!(ok(&mut c, "manipMoveContext -q -m Move;"), json!(2));
    ok(&mut c, "manipMoveContext -e -m 0 Move;");
    assert_eq!(ok(&mut c, "manipMoveContext -q -m Move;"), json!(0));
    ok(&mut c, "manipMoveContext -e -snap 1 -snapValue 0.5 -ah 1 Move;");
    assert_eq!(ok(&mut c, "manipMoveContext -q -ah Move;"), json!(1));
    ok(&mut c, "manipRotateContext -e -m 1 Rotate; manipScaleContext -e -m 2 Scale;");
    assert_eq!(ok(&mut c, "manipRotateContext -q -m Rotate;"), json!(1));
    let reqs = aztek_host::take_ui_requests_for_tests();
    let has = |t: &str| reqs.as_array().unwrap().iter().any(|r| r.as_str().is_some_and(|x| x.contains(t)));
    assert!(has("axisOrientation") && has("stepMove"), "{reqs}");
    ok(&mut c, "polyCube -n pm; move 1 2 3 pm;");
    assert_eq!(ok(&mut c, "manipMoveContext -q -p Move;"), json!([1.0, 2.0, 3.0]));
    ok(&mut c, "ctxEditMode;");
    assert_eq!(ok(&mut c, "ctxEditMode -q;"), json!(true));
    ok(&mut c, "ctxEditMode;");
    // The shell moving the tool moves the context.
    aztek_host::ctx_cmds::shell_tool_changed("move");
    assert_eq!(ok(&mut c, "currentCtx;"), json!("moveSuperContext"));
    ok(&mut c, "manipMoveContext -e -m 2 Move;");
}

#[cfg(feature = "python")]
mod python {
    use super::*;

    fn py(c: &mut Core, src: &str) {
        // MEL's python() runs against this core.
        let quoted = src.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
        let r = c.mel_checked(&format!("python(\"{quoted}\");"));
        assert!(r.is_ok(), "{src}: {r:?}");
    }

    /// A Python expression's value, whole: through JSON, since MEL has no booleans or mixed arrays.
    fn pv(c: &mut Core, src: &str) -> Value {
        let quoted = src.replace('\\', "\\\\").replace('"', "\\\"");
        let text = c.mel_checked(&format!("python(\"__import__('json').dumps({quoted})\");")).unwrap();
        serde_json::from_str(text.as_str().unwrap()).unwrap()
    }

    fn lines(c: &mut Core) -> Vec<String> {
        c.take_log().as_array().unwrap().iter().map(|l| l["text"].as_str().unwrap().to_string()).collect()
    }

    #[test]
    fn python_builds_and_calls_back() {
        let mut c = Core::new();
        py(&mut c, "import maya.cmds as cmds\nimport functools\nseen = []\nwin = cmds.window(title='T', widthHeight=(200, 100))\ncol = cmds.columnLayout(adjustableColumn=True)\nf = cmds.textField(tx='start')");
        assert_eq!(pv(&mut c, "win"), json!("window1"));
        assert_eq!(pv(&mut c, "col"), json!("window1|columnLayout1"));
        assert_eq!(pv(&mut c, "cmds.textField(f, q=True, tx=True)"), json!("start"), "right after creation");
        assert_eq!(pv(&mut c, "cmds.window(win, q=True, wh=True)"), json!([200, 100]));

        py(&mut c, "def on_press(*a):\n    seen.append(('press', a))\nb = cmds.button(l='Go', c=on_press)");
        press(&mut c, "window1|columnLayout1|button1", "command", json!([false]));
        assert_eq!(pv(&mut c, "seen[-1]"), json!(["press", [false]]));
        py(&mut c, "ck = cmds.checkBox(l='C', cc=lambda v: seen.append(('check', v)))");
        c.ui_event(&json!({"path": "window1|columnLayout1|checkBox1", "set": {"value": true}, "fire": ["changeCommand"], "args": [true]}));
        assert_eq!(pv(&mut c, "seen[-1]"), json!(["check", true]));
        py(&mut c, "s = cmds.floatSlider(cc=functools.partial(lambda tag, v: seen.append((tag, v)), 'slide'))");
        c.ui_event(&json!({"path": "window1|columnLayout1|floatSlider1", "set": {"value": 0.75}, "fire": ["changeCommand"], "args": [0.75]}));
        assert_eq!(pv(&mut c, "seen[-1]"), json!(["slide", 0.75]));

        // A string from Python is Python. Querying gives the callable back.
        py(&mut c, "b2 = cmds.button(c='cmds.polyCube(n=\"fromString\")')");
        press(&mut c, "window1|columnLayout1|button2", "command", json!([false]));
        assert_eq!(c.mel("objExists fromString;"), json!(true));
        assert_eq!(pv(&mut c, "cmds.button(b, q=True, c=True) is on_press"), json!(true));
        assert_eq!(pv(&mut c, "cmds.button(b2, q=True, c=True)"), json!("cmds.polyCube(n=\"fromString\")"));

        // An exception is reported, as a callback's.
        py(&mut c, "def bad(*a):\n    raise ValueError('nope')\nb3 = cmds.button(c=bad)");
        lines(&mut c);
        press(&mut c, "window1|columnLayout1|button3", "command", json!([false]));
        assert!(lines(&mut c).iter().any(|l| l.contains("ValueError") && l.contains("nope")));

        // A control made by a callback: the name comes back, its parent is the current one.
        py(&mut c, "def add(*a):\n    seen.append(cmds.text(l='made'))\nb4 = cmds.button(c=add)");
        press(&mut c, "window1|columnLayout1|button4", "command", json!([false]));
        assert_eq!(pv(&mut c, "seen[-1]"), json!("window1|columnLayout1|text1"));

        // Forms and tuples, as scripts write them.
        py(&mut c, "form = cmds.formLayout(numberOfDivisions=100)\na = cmds.button()\nd = cmds.button()\ncmds.formLayout(form, e=True, attachForm=[(a, 'top', 5), (a, 'left', 5)], attachControl=[(d, 'top', 2, a)], attachPosition=[(d, 'left', 3, 50)], attachNone=(d, 'bottom'))\ncmds.setParent('..')");
        assert_eq!(pv(&mut c, "len(cmds.formLayout(form, q=True, af=True))"), json!(2));
        // Empty list queries are None from Python, as in the reference.
        py(&mut c, "l = cmds.textScrollList(a=['x', 'y'], ams=True)");
        assert_eq!(pv(&mut c, "cmds.textScrollList(l, q=True, si=True) is None"), json!(true));
        py(&mut c, "cmds.textScrollList(l, e=True, si='y')");
        assert_eq!(pv(&mut c, "cmds.textScrollList(l, q=True, si=True)"), json!(["y"]));
        assert_eq!(pv(&mut c, "cmds.window(win, exists=True)"), json!(true));
        py(&mut c, "cmds.deleteUI(win, window=True)");
        assert_eq!(pv(&mut c, "cmds.window(win, exists=True)"), json!(false));
    }

    #[test]
    fn jobs_and_deferred_from_python() {
        let mut c = Core::new();
        py(&mut c, "import maya.cmds as cmds\nimport maya.utils\njob_log = []\nj = cmds.scriptJob(e=['SelectionChanged', lambda: job_log.append('sel')])\ncmds.polyCube(n='jc')\ncmds.select('jc')");
        c.idle();
        assert_eq!(pv(&mut c, "job_log"), json!(["sel"]));
        py(&mut c, "cmds.scriptJob(kill=j)\ncmds.select(cl=True)");
        c.idle();
        assert_eq!(pv(&mut c, "job_log"), json!(["sel"]));
        assert_eq!(pv(&mut c, "cmds.scriptJob(exists=j)"), json!(false));
        // A string from Python is Python.
        py(&mut c, "j2 = cmds.scriptJob(runOnce=True, e=['timeChanged', 'job_log.append(\"time\")'])\ncmds.currentTime(9)");
        c.idle();
        assert_eq!(pv(&mut c, "job_log[-1]"), json!("time"));
        // Deferred: after the script; a function, a string, executeDeferred with its arguments, in order.
        py(&mut c, "cmds.evalDeferred(lambda: job_log.append('d1'))\ncmds.evalDeferred('job_log.append(\"d2\")')\nmaya.utils.executeDeferred(job_log.append, 'd3')");
        assert_eq!(pv(&mut c, "job_log[-1]"), json!("time"));
        c.idle();
        assert_eq!(pv(&mut c, "job_log[-3:]"), json!(["d1", "d2", "d3"]));
        // processIdleEvents flushes from inside a script.
        py(&mut c, "cmds.evalDeferred(lambda: job_log.append('d4'))\nmaya.utils.processIdleEvents()\nhit = job_log[-1]");
        assert_eq!(pv(&mut c, "hit"), json!("d4"));
        assert_eq!(pv(&mut c, "maya.utils.executeInMainThreadWithResult(lambda a: a + 1, 2)"), json!(3));
        py(&mut c, "cmds.scriptJob(ka=True)");
    }

    #[test]
    fn functions_cmds_lacks_otherwise() {
        let mut c = Core::new();
        py(&mut c, "import maya.cmds as cmds\ncmds.warning('careful')");
        assert!(lines(&mut c).iter().any(|l| l.contains("Warning: careful")));
        assert!(c.mel_checked("python(\"cmds.error('boom')\");").is_err());
        assert_eq!(pv(&mut c, "cmds.whatIs('polyCube')"), json!("Command"));
        py(&mut c, "cmds.timer(s=True)\nt = cmds.timer(e=True)");
        assert_eq!(pv(&mut c, "t >= 0"), json!(true));
        py(&mut c, "n = cmds.scriptNode(st=2, bs='import maya.cmds as cmds; cmds.polyCube(n=\"pyNode\")', stp='python', n='pn')\ncmds.scriptNode(n, eb=True)");
        assert_eq!(c.mel("objExists pyNode;"), json!(true));
        assert_eq!(pv(&mut c, "cmds.scriptNode('pn', q=True, stp=True)"), json!("python"));
        // Panels and contexts read the same from Python.
        assert_eq!(pv(&mut c, "cmds.getPanel(withFocus=True)"), json!("modelPanel4"));
        assert_eq!(pv(&mut c, "cmds.modelEditor('modelPanel4', q=True, joints=True)"), json!(true));
        py(&mut c, "cmds.manipMoveContext('Move', e=True, mode=0)\nmode = cmds.manipMoveContext('Move', q=True, mode=True)");
        assert_eq!(pv(&mut c, "mode"), json!(0));
        py(&mut c, "cmds.setToolTo('moveSuperContext')");
        assert_eq!(pv(&mut c, "cmds.currentCtx()"), json!("moveSuperContext"));
    }
}
