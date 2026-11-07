//! condition, disable, grid, toggleAxis, commandEcho, help, hotkeyCheck, getProcArguments and the display scales.

use aztek_host::{take_ui_requests_for_tests as take_ui_requests, Core};
use serde_json::json;

fn prefs_dir(name: &str) {
    let d = std::env::temp_dir().join(format!("aztek_host_general_{name}_{}", std::process::id()));
    std::env::set_var("AZTEK_PREFS", d);
}

#[test]
fn conditions_hold_state_and_run_scripts() {
    prefs_dir("cond");
    let mut c = Core::new();
    c.mel("condition -st 1 manualOn;");
    assert_eq!(c.mel("condition -q -st manualOn;"), json!(true));
    assert_eq!(c.mel("isTrue manualOn;"), json!(true));
    c.mel("condition -e -st 0 manualOn;");
    assert_eq!(c.mel("isTrue manualOn;"), json!(false));
    // -i runs the script at once.
    c.mel("condition -s \"1+1\" -i scripted;");
    c.idle();
    assert_eq!(c.mel("isTrue scripted;"), json!(true));
    // A dependency: follows a built-in condition, checked at idle.
    c.mel("select -cl; condition -s \"isTrue SomethingSelected\" -d SomethingSelected follows;");
    assert_eq!(c.mel("isTrue follows;"), json!(false));
    c.mel("polyCube;");
    c.idle();
    assert_eq!(c.mel("isTrue follows;"), json!(true));
    c.mel("condition -delete follows;");
    assert_eq!(c.mel("isTrue follows;"), json!(false));
}

#[test]
fn disable_edits_a_control() {
    prefs_dir("disable");
    let mut c = Core::new();
    c.mel("window dw; columnLayout; button -l \"x\" bt;");
    assert_eq!(c.mel("control -q -en bt;"), json!(true));
    c.mel("disable bt;");
    assert_eq!(c.mel("control -q -en bt;"), json!(false));
    c.mel("disable -v 0 bt;");
    assert_eq!(c.mel("control -q -en bt;"), json!(true));
}

#[test]
fn grid_reaches_the_views() {
    prefs_dir("grid");
    let mut c = Core::new();
    assert_eq!(c.mel("grid -q -size;"), json!(12.0));
    assert_eq!(c.mel("grid -q -sp;"), json!(5.0));
    take_ui_requests();
    c.mel("grid -size 20 -spacing 10 -divisions 2;");
    let sent = take_ui_requests().to_string();
    assert!(sent.contains("gridSize") && sent.contains("gridSpacing"), "{sent}");
    assert!(sent.contains("modelPanel4"), "{sent}");
    assert!(sent.contains("\\\"value\\\":5.0"), "spacing over divisions: {sent}");
    assert_eq!(c.mel("grid -q -size;"), json!(20.0));
    c.mel("grid -tgl 0;");
    assert_eq!(c.mel("grid -q -tgl;"), json!(false));
    c.mel("grid -reset;");
    assert_eq!(c.mel("grid -q -size;"), json!(12.0));
    assert_eq!(c.mel("grid -q -tgl;"), json!(true));
}

#[test]
fn help_comes_from_the_registry() {
    prefs_dir("help");
    let mut c = Core::new();
    let text = c.mel("help -so \"flexor\";");
    let text = text.as_str().unwrap();
    assert!(text.starts_with("Synopsis: flexor"), "{text}");
    assert!(text.contains("-ab -atBones"), "{text}");
    let long = c.mel("help \"flexor\";");
    assert!(long.as_str().unwrap().contains("deformer at joints"));
    let list = c.mel("help -l \"^polyCub\";");
    assert!(list.as_array().unwrap().iter().any(|x| x == "polyCube"), "{list}");
    assert!(c.mel_checked("help -doc \"flexor\";").is_err(), "must not pretend");
    assert!(c.mel_checked("help \"noSuchCommand\";").is_err());
}

#[test]
fn proc_arguments_and_echo_and_hotkeys() {
    prefs_dir("misc");
    let mut c = Core::new();
    c.mel("proc foo(int $i, string $s[]) {}");
    assert_eq!(c.mel("getProcArguments \"foo\";"), json!(["int", "string[]"]));
    assert!(c.mel_checked("getProcArguments \"nope\";").is_err());
    assert_eq!(c.mel("commandEcho -q -st;"), json!(false));
    c.mel("commandEcho -st 1 -f \"polySphere\";");
    assert_eq!(c.mel("commandEcho -q -st;"), json!(true));
    c.mel("commandEcho -af \"ls\";");
    assert_eq!(c.mel("commandEcho -q -f;"), json!(["polySphere", "ls"]));
    c.mel("commandEcho -st 0 -f \"\";");
    c.mel("nameCommand -ann \"Do it\" -c \"polyCube\" myName;");
    c.mel("hotkey -k \"F9\" -ctl -n \"myName\";");
    assert_eq!(c.mel("hotkeyCheck -k \"F9\" -ctl;"), json!("Do it"));
    assert_eq!(c.mel("hotkeyCheck -k \"F9\";"), json!(""));
}

#[test]
fn display_scales_reach_the_prefs_the_view_reads() {
    prefs_dir("scales");
    let mut c = Core::new();
    c.mel("jointDisplayScale 2.5;");
    assert_eq!(c.mel("jointDisplayScale -q;"), json!(2.5));
    assert_eq!(aztek_host::ui_cmds::pref_f64("jointScale", 1.0), 2.5);
    c.mel("ikHandleDisplayScale 3;");
    assert_eq!(c.mel("ikHandleDisplayScale -q;"), json!(3.0));
    assert_eq!(aztek_host::ui_cmds::pref_f64("ikHandleScale", 1.0), 3.0);
    assert!(c.mel_checked("jointDisplayScale 0;").is_err());
    c.mel("jointDisplayScale 1; ikHandleDisplayScale 1;");
    c.mel("toggleAxis -v 0 -o 1;");
    assert_eq!(c.mel("toggleAxis -q -v;"), json!(false));
    assert_eq!(c.mel("toggleAxis -q -o;"), json!(true));
    c.mel("toggleAxis -v 1;");
}

/// Scripts reach these through MEL's own argument handling, with short flags and query forms.
#[test]
fn scene_commands_from_mel() {
    prefs_dir("melforms");
    let mut c = Core::new();
    assert_eq!(c.mel("flexor -q -l;"), json!(["jointLattice", "boneLattice", "sculpt"]));
    c.mel("polyCylinder -r 1 -h 8 -sh 8 -sa 8 -ax 1 0 0 -n limb;");
    c.mel("select -cl; joint -p -4 0 0 -n j1; joint -p 0 0 0 -n j2; joint -p 4 0 0 -n j3;");
    c.mel("skinCluster j1 limb;");
    c.mel("select j2;");
    assert_eq!(c.mel("flexor -typ \"jointLattice\" -aj;"), json!(["jointLattice1"]));
    c.mel("jointLattice -e -cr 2.5 jointLattice1;");
    assert_eq!(c.mel("jointLattice -q -cr jointLattice1;"), json!(2.5));
    assert_eq!(c.mel("jointLattice -q -j jointLattice1;"), json!("j2"));
    let made = c.mel("flexor -typ \"sculpt\" -dc \"sculpt -mxd 4.0\" -aj j2;");
    assert_eq!(made, json!(["sculpt1"]));
    assert_eq!(c.mel("getAttr sculpt1.maximumDisplacement;"), json!(4.0));

    c.mel("xformConstraint -t \"surface\";");
    assert_eq!(c.mel("xformConstraint -q -t;"), json!("surface"));
    c.mel("timeCode -psh 1;");
    assert_eq!(c.mel("timeCode -q -psh;"), json!(1.0));
    assert_eq!(c.mel("ghosting -q -m;"), json!("preAndPost"));
    c.mel("ikSystemInfo -gsh 0;");
    assert_eq!(c.mel("ikSystemInfo -q -gsh;"), json!(false));
    c.mel("ikSystemInfo -gsh 1;");

    c.mel("polyCube -n bx;");
    c.mel("instanceable -a 0 bx;");
    assert_eq!(c.mel("instanceable -q bx;"), json!([false]));
    assert!(c.mel_checked("instance bx;").is_err());

    c.mel("curve -d 3 -p 0 0 0 -p 1 1 0 -p 2 1 0 -p 3 0 0 -n cv1;");
    c.mel("curve -d 3 -p 0 4 0 -p 1 5 0 -p 2 5 0 -p 3 4 0 -n cv2;");
    let r = c.mel("blend2 cv1 cv2;");
    assert_eq!(r.as_array().map(Vec::len), Some(2), "{r}");
    assert_eq!(c.mel("bezierInfo -afc 4;"), json!(1));

    c.mel("stringArrayIntersector si1;");
    c.mel("stringArrayIntersector -e -i {\"a\", \"b\"} si1;");
    c.mel("stringArrayIntersector -e -i {\"b\", \"c\"} si1;");
    assert_eq!(c.mel("stringArrayIntersector -q si1;"), json!(["b"]));
    c.mel("dataStructure -fmt \"raw\" -asString \"name=melSt:int32=ID\";");
    assert_eq!(c.mel("dataStructure -n \"melSt\" -q -lmn;"), json!(["ID"]));
}
