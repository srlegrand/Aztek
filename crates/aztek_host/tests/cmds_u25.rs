//! Unit u25: hotBox, hudSliderButton's font sizes, iconTextButton, iconTextCheckBox, iconTextRadioButton,
//! iconTextRadioCollection, iconTextScrollList and iconTextStaticLabel.

use aztek_host::{take_ui_requests_for_tests as take_ui_requests, Core};
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

#[test]
fn hot_box_rows_and_styles() {
    let mut c = Core::new();
    // Every row shows to begin with.
    assert_eq!(ok(&mut c, "hotBox -q -showAllToggleMenus;"), json!(true));
    assert_eq!(ok(&mut c, "hotBox -q -displayStyle;"), json!("dh"));
    assert_eq!(ok(&mut c, "hotBox -q -dh;"), json!(true));
    ok(&mut c, "hotBox -polygonsToggleMenus 0;");
    assert_eq!(ok(&mut c, "hotBox -q -plt;"), json!(false));
    assert_eq!(ok(&mut c, "hotBox -q -a;"), json!(false));
    assert_eq!(ok(&mut c, "hotBox -q -mdt;"), json!(true));
    // One row alone.
    ok(&mut c, "hotBox -riggingOnlyMenus;");
    assert_eq!(ok(&mut c, "hotBox -q -rgo;"), json!(true));
    assert_eq!(ok(&mut c, "hotBox -q -ct;"), json!(false));
    assert_eq!(ok(&mut c, "hotBox -q -rgt;"), json!(true));
    ok(&mut c, "hotBox -ct 1 -pt 1;");
    assert_eq!(ok(&mut c, "hotBox -q -rgo;"), json!(false));
    assert_eq!(ok(&mut c, "hotBox -q -PaneToggleMenus;"), json!(true));
    // Menu sets by name, the reference's and Aztek's.
    ok(&mut c, "hotBox -menuSetOnly \"animationMenuSet\";");
    assert_eq!(ok(&mut c, "hotBox -q -ao;"), json!(true));
    ok(&mut c, "hotBox -menuSetToggle \"Rendering\" 1;");
    assert_eq!(ok(&mut c, "hotBox -q -rt;"), json!(true));
    assert_eq!(ok(&mut c, "hotBox -q -ao;"), json!(false));
    ok(&mut c, "hotBox -menuSetToggle \"FX\" 1;");
    assert_eq!(ok(&mut c, "hotBox -q -dt;"), json!(true));
    bad(&mut c, "hotBox -menuSetOnly \"nothingMenuSet\";");
    // All on, all off.
    ok(&mut c, "hotBox -showAllToggleMenus 0;");
    for f in ["ct", "pt", "mdt", "plt", "st", "rgt", "at", "dt", "clt", "rt", "lt", "cst"] {
        assert_eq!(ok(&mut c, &format!("hotBox -q -{f};")), json!(false), "{f}");
    }
    ok(&mut c, "hotBox -a 1;");
    assert_eq!(ok(&mut c, "hotBox -q -showAllToggleMenus;"), json!(true));
    for f in ["co", "po", "mdo", "plo", "so", "rgo", "ao", "do", "clo", "ro", "lo"] {
        assert_eq!(ok(&mut c, &format!("hotBox -q -{f};")), json!(false), "{f}");
    }
    // Display styles.
    ok(&mut c, "hotBox -displayZonesOnly;");
    assert_eq!(ok(&mut c, "hotBox -q -ds;"), json!("dzo"));
    assert_eq!(ok(&mut c, "hotBox -q -dzo;"), json!(true));
    ok(&mut c, "hotBox -dco;");
    assert_eq!(ok(&mut c, "hotBox -q -displayStyle;"), json!("dco"));
    ok(&mut c, "hotBox -displayHotbox;");
    assert_eq!(ok(&mut c, "hotBox -q -ds;"), json!("dh"));
    // Transparency in steps of 25, the right-click popups on or off.
    ok(&mut c, "hotBox -transparenyLevel 60;");
    assert_eq!(ok(&mut c, "hotBox -q -tr;"), json!(50));
    ok(&mut c, "hotBox -tr 140;");
    assert_eq!(ok(&mut c, "hotBox -q -tr;"), json!(100));
    ok(&mut c, "hotBox -rmb 0;");
    assert_eq!(ok(&mut c, "hotBox -q -rmbPopups;"), json!(false));
    // What the shell draws from.
    let st = ok(&mut c, "aztekHotBox;");
    assert_eq!(st["style"], json!("dh"));
    assert_eq!(st["transparency"], json!(100));
    assert_eq!(st["rows"]["rendering"], json!(true));
    // Settings alone don't pop it; -updateMenus asks the shell to build its menus again.
    take_ui_requests();
    ok(&mut c, "hotBox -um;");
    let r = take_ui_requests().to_string();
    assert!(r.contains("hotBox|update") && !r.contains("hotBox|show"), "{r}");
}

#[test]
fn hot_box_pops_and_no_click_command() {
    let mut c = Core::new();
    ok(&mut c, "global string $gWhere; global proc showPos(int $x, int $y) { global string $gWhere; $gWhere = $x + \",\" + $y; }");
    take_ui_requests();
    assert_eq!(ok(&mut c, "hotBox -q -release;"), json!(true));
    ok(&mut c, "hotBox -noClickCommand \"showPos\" -noClickPosition -position 300 200;");
    let r = take_ui_requests().to_string();
    assert!(r.contains("hotBox|show"), "{r}");
    assert_eq!(ok(&mut c, "hotBox -q -release;"), json!(false));
    assert_eq!(ok(&mut c, "hotBox -q -pos;"), json!([300, 200]));
    // Released at once with nothing picked: the command runs with the place.
    ok(&mut c, "hotBox -release;");
    assert!(take_ui_requests().to_string().contains("hotBox|hide"));
    assert_eq!(ok(&mut c, "global string $gWhere; $gWhere;"), json!("300,200"));
    // Something picked in it: no command.
    ok(&mut c, "$gWhere = \"\";");
    ok(&mut c, "hotBox -ncc \"showPos\" -ncp;");
    ok(&mut c, "aztekHotBox -shown 10 20;");
    ok(&mut c, "aztekHotBox -used;");
    ok(&mut c, "hotBox -rl;");
    assert_eq!(ok(&mut c, "global string $gWhere; $gWhere;"), json!(""));
    // Too slow: no command.
    ok(&mut c, "hotBox -ncc \"showPos\" -ncp -ncd 0;");
    std::thread::sleep(std::time::Duration::from_millis(5));
    ok(&mut c, "hotBox -rl;");
    assert_eq!(ok(&mut c, "global string $gWhere; $gWhere;"), json!(""));
    // The pointer where the shell popped it; without -noClickPosition nothing is added.
    ok(&mut c, "global proc noArgs() { global string $gWhere; $gWhere = \"plain\"; }");
    ok(&mut c, "hotBox -ncc \"noArgs\";");
    ok(&mut c, "hotBox -rl;");
    assert_eq!(ok(&mut c, "global string $gWhere; $gWhere;"), json!("plain"));
    // -noKeyPress: popped by another device, still down until -release.
    ok(&mut c, "hotBox -noKeyPress;");
    assert_eq!(ok(&mut c, "hotBox -q -nkp;"), json!(true));
    assert!(take_ui_requests().to_string().contains("noKeyPress\\\":true"));
    ok(&mut c, "hotBox -release;");
    assert_eq!(ok(&mut c, "hotBox -q -nkp;"), json!(false));
    // A plain call pops it.
    ok(&mut c, "hotBox;");
    assert_eq!(ok(&mut c, "hotBox -q -rl;"), json!(false));
    ok(&mut c, "hotBox -rl;");
}

#[test]
fn hud_slider_button_font_sizes() {
    let mut c = Core::new();
    ok(&mut c, "hudSliderButton -s 1 -b 2 -sl \"Size\" -bl \"Set\" -sfs \"large\" hsb;");
    assert_eq!(ok(&mut c, "hudSliderButton -q -sliderLabelFontSize hsb;"), json!("large"));
    assert_eq!(ok(&mut c, "hudSliderButton -q -bfs hsb;"), json!("small"));
    ok(&mut c, "hudSliderButton -e -buttonLabelFontSize \"large\" hsb;");
    assert_eq!(ok(&mut c, "hudSliderButton -q -bfs hsb;"), json!("large"));
    bad(&mut c, "hudSliderButton -e -bfs \"huge\" hsb;");
    assert_eq!(ok(&mut c, "hudSliderButton -q -bfs hsb;"), json!("large"));
}

#[test]
fn icon_text_button_flips_and_callbacks() {
    let mut c = Core::new();
    ok(&mut c, "global string $gGot; global proc gotIt(string $s) { global string $gGot; $gGot = $s; }");
    ok(&mut c, "window w; columnLayout col; iconTextButton -i \"cube.png\" -fx 1 -hnd \"gotIt\" -lec \"gotIt\" b;");
    assert_eq!(ok(&mut c, "iconTextButton -q -flipX b;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextButton -q -flipY b;"), json!(false));
    ok(&mut c, "iconTextButton -e -fy 1 -fx 0 b;");
    assert_eq!(ok(&mut c, "iconTextButton -q -fy b;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextButton -q -fx b;"), json!(false));
    // A node dropped on it, and its label renamed: the name goes to the script.
    c.ui_event(&json!({"path": "w|col|b", "fire": ["handleNodeDropCallback"], "args": ["pCube1"]}));
    assert_eq!(ok(&mut c, "global string $gGot; $gGot;"), json!("pCube1"));
    c.ui_event(&json!({"path": "w|col|b", "set": {"label": "New"}, "fire": ["labelEditingCallback"], "args": ["New"]}));
    assert_eq!(ok(&mut c, "global string $gGot; $gGot;"), json!("New"));
    assert_eq!(ok(&mut c, "iconTextButton -q -l b;"), json!("New"));
}

#[test]
fn icon_text_check_box_and_radio_button_looks() {
    let mut c = Core::new();
    ok(&mut c, "window; columnLayout;");
    ok(&mut c, "iconTextCheckBox -st \"iconAndTextVertical\" -i1 \"a.png\" -i2 \"b.png\" -i3 \"c.png\" -di \"d.png\" -hi \"h.png\" -si \"s.png\" -shi \"sh.png\" -iol \"ov\" -olc 1 0 0 -olb 0 0 1 0.5 -mw 4 -mh 3 -lo 2 -rot 1.5 -ua 0 -fx 1 -ver \"2027\" -l \"Check\" cb;");
    for (f, want) in [
        ("i2", json!("b.png")),
        ("i3", json!("c.png")),
        ("di", json!("d.png")),
        ("hi", json!("h.png")),
        ("si", json!("s.png")),
        ("shi", json!("sh.png")),
        ("iol", json!("ov")),
        ("olc", json!([1.0, 0.0, 0.0])),
        ("olb", json!([0.0, 0.0, 1.0, 0.5])),
        ("mw", json!(4)),
        ("mh", json!(3)),
        ("lo", json!(2)),
        ("rot", json!(1.5)),
        ("ua", json!(false)),
        ("fx", json!(true)),
        ("fy", json!(false)),
        ("ver", json!("2027")),
    ] {
        assert_eq!(ok(&mut c, &format!("iconTextCheckBox -q -{f} cb;")), want, "{f}");
    }
    ok(&mut c, "iconTextCheckBox -e -mw 6 -iol \"x\" -v 1 cb;");
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -marginWidth cb;"), json!(6));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -v cb;"), json!(true));
    // The defaults the reference documents.
    ok(&mut c, "iconTextCheckBox plain;");
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -mw plain;"), json!(1));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -mh plain;"), json!(1));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -lo plain;"), json!(0));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -olc plain;"), json!([0.8, 0.8, 0.8]));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -olb plain;"), json!([0.0, 0.0, 0.0, 0.5]));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -ua plain;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -al plain;"), json!("center"));
    // The radio button takes the same.
    ok(&mut c, "iconTextRadioCollection rc; iconTextRadioButton -i1 \"a.png\" -fy 1 -si \"s.png\" -mh 5 -olc 0 1 0 rb;");
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -fy rb;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -selectionImage rb;"), json!("s.png"));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -marginHeight rb;"), json!(5));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -overlayLabelColor rb;"), json!([0.0, 1.0, 0.0]));
    ok(&mut c, "iconTextRadioButton -e -rot 0.5 -di \"off.png\" rb;");
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -rotation rb;"), json!(0.5));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -disabledImage rb;"), json!("off.png"));
}

#[test]
fn icon_text_controls_from_a_run_time_command() {
    let mut c = Core::new();
    ok(&mut c, "runTimeCommand -ann \"Makes a cube\" -l \"Cube\" -i \"cube.png\" -ver \"2027\" -c \"polyCube\" MakeCube;");
    ok(&mut c, "window; columnLayout;");
    ok(&mut c, "iconTextStaticLabel -rtc \"MakeCube\" s;");
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -l s;"), json!("Cube"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -ann s;"), json!("Makes a cube"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -i s;"), json!("cube.png"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -ver s;"), json!("2027"));
    // What the call gives wins.
    ok(&mut c, "iconTextCheckBox -rtc \"MakeCube\" -l \"Mine\" cb;");
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -l cb;"), json!("Mine"));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -ann cb;"), json!("Makes a cube"));
    assert_eq!(ok(&mut c, "iconTextCheckBox -q -cc cb;"), json!("polyCube"));
    bad(&mut c, "iconTextStaticLabel -rtc \"NoSuchCommand\" s2;");
    assert_eq!(ok(&mut c, "iconTextStaticLabel -ex s2;"), json!(false));
}

#[test]
fn icon_text_static_label() {
    let mut c = Core::new();
    ok(&mut c, "window w; columnLayout col;");
    let p = ok(&mut c, "iconTextStaticLabel -st \"iconAndTextHorizontal\" -i1 \"cone.png\" -l \"cone\" -fn \"boldLabelFont\" -al \"left\" -w 120 -h 30 -ann \"A cone\" lab;");
    assert_eq!(p, json!("w|col|lab"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -st lab;"), json!("iconAndTextHorizontal"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -i1 lab;"), json!("cone.png"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -l lab;"), json!("cone"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -fn lab;"), json!("boldLabelFont"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -al lab;"), json!("left"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -w lab;"), json!(120));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -ann lab;"), json!("A cone"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -fpn lab;"), json!("w|col|lab"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -en lab;"), json!(true));
    ok(&mut c, "iconTextStaticLabel -e -l \"sphere\" -i \"sphere.png\" -fx 1 -iol \"SP\" -olc 1 1 0 -olb 0 0 0 1 -rot 3.14 -mw 2 -en 0 lab;");
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -label lab;"), json!("sphere"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -image lab;"), json!("sphere.png"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -flipX lab;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -imageOverlayLabel lab;"), json!("SP"));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -overlayLabelBackColor lab;"), json!([0.0, 0.0, 0.0, 1.0]));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -rotation lab;"), json!(3.14));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -enable lab;"), json!(false));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -ex lab;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextStaticLabel -ex nope;"), json!(false));
    // Templates.
    ok(&mut c, "uiTemplate tpl; iconTextStaticLabel -dt tpl -st \"iconOnly\"; setUITemplate -pushTemplate tpl; iconTextStaticLabel lab2; setUITemplate -popTemplate;");
    assert_eq!(ok(&mut c, "iconTextStaticLabel -q -st lab2;"), json!("iconOnly"));
    ok(&mut c, "deleteUI lab;");
    assert_eq!(ok(&mut c, "iconTextStaticLabel -ex lab;"), json!(false));
}

#[test]
fn icon_text_radio_collection() {
    let mut c = Core::new();
    ok(&mut c, "global string $gLog; $gLog = \"\";");
    ok(&mut c, "window w; columnLayout col;");
    let coll = ok(&mut c, "iconTextRadioCollection itrc;");
    assert_eq!(coll, json!("w|col|itrc"));
    ok(&mut c, "iconTextRadioButton -st \"textOnly\" -l \"Sphere\" -onc \"$gLog += \\\"on1 \\\"\" -ofc \"$gLog += \\\"off1 \\\"\" one;");
    ok(&mut c, "iconTextRadioButton -st \"textOnly\" -l \"Cube\" -cc \"$gLog += (\\\"cc2=\\\" + #1 + \\\" \\\")\" two;");
    ok(&mut c, "iconTextRadioButton -st \"textOnly\" -l \"Cone\" three;");
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -nci itrc;"), json!(3));
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -cia itrc;").as_array().unwrap().len(), 3);
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -sl itrc;"), json!("NONE"));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -cl one;"), json!("w|col|itrc"));
    // Picking runs the buttons' commands: the one turned off first.
    ok(&mut c, "iconTextRadioCollection -e -sl one itrc;");
    assert_eq!(ok(&mut c, "global string $gLog; $gLog;"), json!("on1 "));
    ok(&mut c, "iconTextRadioCollection -e -sl two itrc;");
    assert_eq!(ok(&mut c, "global string $gLog; $gLog;"), json!("on1 off1 cc2=1 "));
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -select itrc;"), json!("two"));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -sl one;"), json!(false));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -sl two;"), json!(true));
    // -disableCommands: picked without them.
    ok(&mut c, "$gLog = \"\"; iconTextRadioCollection -e -dcm -sl one itrc;");
    assert_eq!(ok(&mut c, "global string $gLog; $gLog;"), json!(""));
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -sl itrc;"), json!("one"));
    bad(&mut c, "iconTextRadioCollection -e -sl nobody itrc;");
    // A button made selected turns the others off.
    ok(&mut c, "iconTextRadioButton -sl 1 -l \"Torus\" four;");
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -sl itrc;"), json!("four"));
    assert_eq!(ok(&mut c, "iconTextRadioButton -q -sl one;"), json!(false));
    // A click in the shell does the same.
    c.ui_event(&json!({"path": "w|col|three", "set": {"select": true}, "fire": ["changeCommand", "onCommand"], "args": [true]}));
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -sl itrc;"), json!("three"));
    // Global: no parent, outlives the window.
    ok(&mut c, "iconTextRadioCollection -gl 1 shared;");
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -gl shared;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextRadioCollection -q -gl itrc;"), json!(false));
    ok(&mut c, "deleteUI w;");
    assert_eq!(ok(&mut c, "iconTextRadioCollection -ex itrc;"), json!(false));
    assert_eq!(ok(&mut c, "iconTextRadioCollection -ex shared;"), json!(true));
    ok(&mut c, "deleteUI -collection shared;");
    assert_eq!(ok(&mut c, "iconTextRadioCollection -ex shared;"), json!(false));
}

#[test]
fn icon_text_scroll_list() {
    let mut c = Core::new();
    ok(&mut c, "global string $gLog; $gLog = \"\";");
    ok(&mut c, "window w; paneLayout pl;");
    let p = ok(&mut c, "iconTextScrollList -ams 1 -a \"one\" -a \"two\" -a \"three\" -a \"four\" -si \"two\" -sc \"$gLog += \\\"sel \\\"\" -dcc \"$gLog += \\\"dbl \\\"\" -cc \"$gLog += \\\"cc \\\"\" -h 54 -w 80 list;");
    assert_eq!(p, json!("w|pl|list"));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -si list;"), json!(["two"]));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -sii list;"), json!([2]));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -ams list;"), json!(true));
    // Several picked in multi-select mode.
    ok(&mut c, "iconTextScrollList -e -sii 4 -si \"one\" list;");
    assert_eq!(ok(&mut c, "iconTextScrollList -q -sii list;"), json!([2, 4, 1]));
    ok(&mut c, "iconTextScrollList -e -da list;");
    assert_eq!(ok(&mut c, "iconTextScrollList -q -si list;"), json!([]));
    // Single select: one at a time.
    ok(&mut c, "iconTextScrollList -e -ams 0 -sii 1 -sii 3 list;");
    assert_eq!(ok(&mut c, "iconTextScrollList -q -sii list;"), json!([3]));
    // Rows: 54 pixels high shows three of the four.
    assert_eq!(ok(&mut c, "iconTextScrollList -q -nr list;"), json!(3));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -nic list;"), json!(4));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -itemAt 5 20 list;"), json!("two"));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -ia 5 40 list;"), json!("three"));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -ia 500 20 list;"), json!(""));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -visualRectAt 5 20 list;"), json!([0, 18, 80, 18]));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -vra 5 70 list;"), json!([]));
    // Colours of item text, editing in place.
    ok(&mut c, "iconTextScrollList -e -itc 2 1 0 0 -itc 3 0 1 0 list;");
    bad(&mut c, "iconTextScrollList -e -itc 9 1 0 0 list;");
    ok(&mut c, "iconTextScrollList -e -ed 1 -ei 2 -dfv 0 list;");
    bad(&mut c, "iconTextScrollList -e -ei 7 list;");
    c.ui_event(&json!({"path": "w|pl|list", "set": {"__editedItem": [1, "TWO"]}, "fire": ["changeCommand"]}));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -ia 5 20 list;"), json!("TWO"));
    // The user's picks and double clicks run the scripts.
    c.ui_event(&json!({"path": "w|pl|list", "set": {"selected": [0]}, "fire": ["selectCommand"]}));
    c.ui_event(&json!({"path": "w|pl|list", "fire": ["doubleClickCommand"]}));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -si list;"), json!(["one"]));
    assert_eq!(ok(&mut c, "global string $gLog; $gLog;"), json!("cc sel dbl "));
    // Everything gone.
    ok(&mut c, "iconTextScrollList -e -ra list;");
    assert_eq!(ok(&mut c, "iconTextScrollList -q -nic list;"), json!(0));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -ia 5 5 list;"), json!(""));
    // The common flags.
    ok(&mut c, "iconTextScrollList -e -ann \"Things\" -bgc 0.2 0.2 0.2 -en 0 list;");
    assert_eq!(ok(&mut c, "iconTextScrollList -q -ann list;"), json!("Things"));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -en list;"), json!(false));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -vis list;"), json!(true));
    assert_eq!(ok(&mut c, "iconTextScrollList -q -npm list;"), json!(0));
    assert_eq!(ok(&mut c, "iconTextScrollList -ex list;"), json!(true));
    // No height: every item counts as shown.
    ok(&mut c, "iconTextScrollList -a \"a\" -a \"b\" other;");
    assert_eq!(ok(&mut c, "iconTextScrollList -q -nr other;"), json!(2));
}

#[test]
fn every_listed_flag_is_there() {
    let c = Core::new();
    let want: &[(&str, &str)] = &[
        ("hotBox", "PaneOnlyMenus PaneToggleMenus a animationOnlyMenus animationToggleMenus ao at clo clothOnlyMenus clothToggleMenus clt co commonOnlyMenus commonToggleMenus cst ct customMenuSetsToggleMenus dco dh displayCenterOnly displayHotbox displayStyle displayZonesOnly do ds dt dynamicsOnlyMenus dynamicsToggleMenus dzo liveOnlyMenus liveToggleMenus lo lt mdo mdt menuSetOnly menuSetToggle modelingOnlyMenus modelingToggleMenus mso mst ncc ncd ncp nkp noClickCommand noClickDelay noClickPosition noKeyPress plo plt po polygonsOnlyMenus polygonsToggleMenus pos position pt q query release renderingOnlyMenus renderingToggleMenus rgo rgt riggingOnlyMenus riggingToggleMenus rl rmb rmbPopups ro rt showAllToggleMenus so st surfacesOnlyMenus surfacesToggleMenus tr transparenyLevel um updateMenus"),
        ("hudSliderButton", "bfs sfs"),
        ("iconTextButton", "flipX flipY fx fy handleNodeDropCallback hnd labelEditingCallback lec"),
        ("iconTextCheckBox", "di disabledImage flipX flipY fx fy hi highlightImage i2 i3 image2 image3 imageOverlayLabel iol labelOffset lo marginHeight marginWidth mh mw olb olc overlayLabelBackColor overlayLabelColor rot rotation selectionHighlightImage selectionImage shi si ua useAlpha ver version"),
        ("iconTextRadioButton", "di disabledImage flipX flipY fx fy hi highlightImage i2 i3 image2 image3 imageOverlayLabel iol labelOffset lo marginHeight marginWidth mh mw olb olc overlayLabelBackColor overlayLabelColor rot rotation selectionHighlightImage selectionImage shi si ua useAlpha ver version"),
        ("iconTextRadioCollection", "cia collectionItemArray dcm defineTemplate disableCommands dt e edit ex exists gl nci numberOfCollectionItems p parent q query select sl useTemplate ut"),
        ("iconTextScrollList", "a allowMultiSelection ams ann annotation append backgroundColor bgc cc changeCommand da dcc defineTemplate deselectAll dfv dgc docTag doubleClickCommand dpc dragCallback dragFeedbackVisible drc dropCallback dropRectCallback dt dtg e ebg ed edit editIndexed editable ei ekf en enable enableBackground enableKeyboardFocus ex exists fpn fullPathName h height highlightColor hlc ia io isObscured itc itemAt itemTextColor m manage nbg nic noBackground npm nr numberOfIcons numberOfPopupMenus numberOfRows p parent pma po popupMenuArray preventOverride q query ra removeAll sbm sc selectCommand selectIndexedItem selectItem si sii statusBarMessage useTemplate ut vcc vis visible visibleChangeCommand visualRectAt vra w width"),
        ("iconTextStaticLabel", "al align ann annotation backgroundColor bgc defineTemplate dgc di disabledImage docTag dpc dragCallback dropCallback dt dtg e ebg edit ekf en enable enableBackground enableKeyboardFocus ex exists flipX flipY fn font fpn fullPathName fx fy h height highlightColor hlc i i1 i2 i3 image image1 image2 image3 imageOverlayLabel io iol isObscured l label labelOffset lo m manage marginHeight marginWidth mh mw nbg noBackground npm numberOfPopupMenus olb olc overlayLabelBackColor overlayLabelColor p parent pma po popupMenuArray preventOverride q query rot rotation sbm st statusBarMessage style ua useAlpha useTemplate ut vcc ver version vis visible visibleChangeCommand w width"),
    ];
    for (cmd, flags) in want {
        let def = c.s.commands.find(cmd).unwrap_or_else(|| panic!("no {cmd}"));
        for fl in flags.split_whitespace() {
            assert!(def.flags.iter().any(|f| f.name == fl || f.short_name == fl), "{cmd} -{fl}");
        }
    }
}
