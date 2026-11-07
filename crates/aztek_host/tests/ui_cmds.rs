//! The application commands: option vars, masks, runtime commands, and the requests they hand the shell.

use aztek_host::Core;
use serde_json::json;

fn prefs_dir(name: &str) {
    let d = std::env::temp_dir().join(format!("aztek_ui_cmds_{name}_{}", std::process::id()));
    std::env::set_var("AZTEK_PREFS", d);
}

#[test]
fn option_var_sets_reads_and_removes() {
    prefs_dir("optionvar");
    let mut c = Core::new();
    c.mel("optionVar -iv \"uiTestInt\" 3;");
    assert_eq!(c.mel("optionVar -q \"uiTestInt\";"), json!(3));
    assert_eq!(c.mel("optionVar -exists \"uiTestInt\";"), json!(true));
    c.mel("optionVar -sv \"uiTestStr\" \"hello\";");
    assert_eq!(c.mel("optionVar -q \"uiTestStr\";"), json!("hello"));
    c.mel("optionVar -remove \"uiTestInt\";");
    assert_eq!(c.mel("optionVar -exists \"uiTestInt\";"), json!(false));
}

#[test]
fn run_time_command_runs_by_name_through_the_shell() {
    let mut c = Core::new();
    c.mel("runTimeCommand -annotation \"Makes a cube\" -category \"User\" -command \"polyCube;\" uiTestCube;");
    assert_eq!(c.mel("runTimeCommand -exists uiTestCube;"), json!(true));
    aztek_host::take_ui_requests_for_tests();
    c.mel("uiTestCube;");
    let reqs = aztek_host::take_ui_requests_for_tests();
    assert!(reqs.as_array().unwrap().iter().any(|r| r == "mel|polyCube;"), "{reqs}");
}

#[test]
fn hotkey_hands_the_binding_to_the_shell() {
    let mut c = Core::new();
    aztek_host::take_ui_requests_for_tests();
    c.mel("nameCommand -annotation \"cube\" -command \"polyCube\" uiTestName;");
    c.mel("hotkey -k \"j\" -ctl -n \"uiTestName\";");
    let reqs = aztek_host::take_ui_requests_for_tests();
    let hk = reqs.as_array().unwrap().iter().filter_map(|r| r.as_str()).find(|r| r.starts_with("hotkey|")).expect("a hotkey request");
    let body: serde_json::Value = serde_json::from_str(&hk["hotkey|".len()..]).unwrap();
    assert_eq!(body["key"], "j");
    assert_eq!(body["ctrl"], true);
    assert_eq!(body["name"], "uiTestName");
}

#[test]
fn select_type_masks_read_back() {
    let mut c = Core::new();
    c.mel("selectType -joint 0;");
    assert_eq!(c.mel("selectType -q -joint 1;"), json!(false));
    c.mel("selectType -joint 1;");
    assert_eq!(c.mel("selectType -q -joint 1;"), json!(true));
    // The component kinds still work through the same command.
    c.mel("polyCube; selectType -polymeshFace 1;");
    assert_eq!(c.mel("selectMode;"), json!("face"));
    assert_eq!(c.mel("selectType -q -polymeshEdge 1;"), json!(false));
}

#[test]
fn clean_up_scene_removes_empty_transforms_only() {
    let mut c = Core::new();
    c.mel("polyCube; group -em -n emptyA; group -em -n emptyB; parent emptyB emptyA;");
    let gone = c.mel("cleanUpScene;");
    assert_eq!(gone.as_array().unwrap().len(), 2, "{gone}");
    assert_eq!(c.mel("objExists pCube1;"), json!(true));
    assert_eq!(c.mel("objExists emptyA;"), json!(false));
}
