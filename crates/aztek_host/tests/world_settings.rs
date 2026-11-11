//! The up axis and the working units, as the reference's Preferences > Settings has them: Z up through the views and
//! commands, the units through the grid and new scenes, and both kept as preferences.

use aztek_host::{take_ui_requests_for_tests as take_ui_requests, Core};
use serde_json::{json, Value};

fn prefs_dir(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("aztek_world_settings_{name}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_var("AZTEK_PREFS", &d);
    d
}

fn num(v: &Value) -> Vec<f64> {
    match v {
        Value::Array(a) => a.iter().flat_map(num).collect(),
        Value::Number(n) => vec![n.as_f64().unwrap()],
        _ => Vec::new(),
    }
}

fn close(got: &Value, want: &[f64]) {
    let g = num(got);
    assert_eq!(g.len(), want.len(), "got {got}, wanted {want:?}");
    for (a, b) in g.iter().zip(want) {
        assert!((a - b).abs() < 1e-3, "got {got}, wanted {want:?}");
    }
}

/// The last value a panelSet request gave a key, for the first model panel.
fn panel_value(requests: &Value, key: &str) -> Option<f64> {
    requests
        .as_array()?
        .iter()
        .filter_map(Value::as_str)
        .filter_map(|r| r.strip_prefix("panelSet|"))
        .filter_map(|j| serde_json::from_str::<Value>(j).ok())
        .filter(|v| v["panel"] == "modelPanel1" && v["key"] == key)
        .last()
        .and_then(|v| v["value"].as_f64())
}

#[test]
fn z_up_turns_the_views_and_their_commands() {
    let mut c = Core::new();
    c.mel("upAxis -ax z -rv;");
    assert_eq!(c.mel("upAxis -q -ax;"), json!("z"));
    // Looking down from above is looking down Z, with Y up the screen.
    c.mel("viewSet -top persp;");
    let eye = c.mel("xform -q -ws -t persp;");
    assert!(num(&eye)[2] > 1.0 && num(&eye)[0].abs() < 1e-6 && num(&eye)[1].abs() < 1e-6, "{eye}");
    close(&c.mel("xform -q -ws -ro persp;"), &[0.0, 0.0, 0.0]);
    c.mel("viewSet -front persp;");
    let eye = c.mel("xform -q -ws -t persp;");
    assert!(num(&eye)[1] < -1.0, "front looks along +Y from -Y: {eye}");
    close(&c.mel("xform -q -ws -ro persp;"), &[90.0, 0.0, 0.0]);
    c.mel("viewSet -side persp;");
    close(&c.mel("xform -q -ws -ro persp;"), &[90.0, 0.0, 90.0]);
    c.mel("viewSet -home persp;");
    close(&c.mel("xform -q -ws -t persp;"), &[28.0, -28.0, 21.0]);
    // Orbiting turns about world Z: the height stays.
    let before = num(&c.mel("xform -q -ws -t persp;"));
    c.mel("orbit -ha 30 persp;");
    let after = num(&c.mel("xform -q -ws -t persp;"));
    assert!((after[2] - before[2]).abs() < 1e-6 && (after[0] - before[0]).abs() > 1.0, "{before:?} {after:?}");
    // A new scene keeps Z up, with Z up cameras.
    c.new_scene();
    assert_eq!(c.mel("upAxis -q -ax;"), json!("z"));
    close(&c.mel("xform -q -ws -t top;"), &[0.0, 0.0, 1000.1]);
    // Primitives stand up Z.
    c.mel("polyCylinder -h 4 -n tall;");
    let b = num(&c.mel("exactWorldBoundingBox tall;"));
    assert!((b[5] - b[2] - 4.0).abs() < 1e-6, "{b:?}");
    // The preference follows the axis.
    assert_eq!(c.mel("optionVar -q upAxisDirection;"), json!("z"));
    c.mel("upAxis -ax y -rv;");
    assert_eq!(c.mel("optionVar -q upAxisDirection;"), json!("y"));
}

#[test]
fn the_view_camera_tumbles_about_world_up() {
    use aztek_host::camera::{CamKind, Camera};
    use glam::DVec2;
    aztek_scene::world::set_up_z(true);
    let mut cam = Camera::new(CamKind::Persp);
    let eye = cam.eye();
    assert!((eye - glam::DVec3::new(28.0, -28.0, 21.0)).length() < 1e-6, "{eye}");
    cam.tumble(DVec2::new(100.0, 0.0));
    assert!((cam.eye().z - 21.0).abs() < 1e-6, "a sideways tumble keeps the height: {}", cam.eye());
    let top = Camera::new(CamKind::Top);
    assert!(top.eye().z > 1.0, "the top view looks down Z: {}", top.eye());
    aztek_scene::world::set_up_z(false);
}

#[test]
fn the_grid_and_steps_are_in_the_linear_unit() {
    let mut c = Core::new();
    c.mel("grid -s 12 -sp 5 -d 5;");
    let r = take_ui_requests();
    assert_eq!(panel_value(&r, "gridSize"), Some(12.0));
    assert_eq!(panel_value(&r, "gridSpacing"), Some(1.0));
    c.mel("currentUnit -l m;");
    c.idle();
    let r = take_ui_requests();
    // 12 metres each way, a line a metre.
    assert_eq!(panel_value(&r, "gridSize"), Some(1200.0));
    assert_eq!(panel_value(&r, "gridSpacing"), Some(100.0));
    assert_eq!(c.mel("grid -q -s;"), json!(12.0));
    c.mel("currentUnit -l cm;");
}

// The frame rate is the process's, so what changes it runs in one test, in turn.
#[test]
fn time_units_new_scene_units_and_saved_preferences() {
    unit_changes_reach_jobs_and_drop_frame_is_its_own_unit();
    a_new_scene_starts_in_the_default_units();
    the_application_reads_and_keeps_its_preferences();
}

fn unit_changes_reach_jobs_and_drop_frame_is_its_own_unit() {
    let mut c = Core::new();
    c.mel("global int $lin = 0; global int $tim = 0; scriptJob -e linearUnitChanged \"$lin++\"; scriptJob -e timeUnitChanged \"$tim++\";");
    c.mel("currentUnit -l m;");
    c.mel("currentUnit -l m;");
    c.mel("currentUnit -t 29.97df;");
    c.idle();
    assert_eq!(c.mel("global int $lin; $lin;"), json!(1));
    assert_eq!(c.mel("global int $tim; $tim;"), json!(1));
    assert_eq!(c.mel("currentUnit -q -t;"), json!("29.97df"));
    c.mel("currentUnit -t 29.97fps;");
    assert_eq!(c.mel("currentUnit -q -t;"), json!("29.97fps"));
    c.mel("currentUnit -l cm -t film;");
}

fn a_new_scene_starts_in_the_default_units() {
    let mut c = Core::new();
    c.mel("newSceneUnits -l m -a rad -t pal;");
    c.mel("currentUnit -l in;");
    c.new_scene();
    assert_eq!(c.mel("currentUnit -q -l;"), json!("m"));
    assert_eq!(c.mel("currentUnit -q -a;"), json!("rad"));
    assert_eq!(c.mel("currentUnit -q -t;"), json!("pal"));
    c.mel("newSceneUnits -l cm -a deg -t film;");
    c.new_scene();
    assert_eq!(c.mel("currentUnit -q -l;"), json!("cm"));
    assert_eq!(c.mel("currentUnit -q -t;"), json!("film"));
}

fn the_application_reads_and_keeps_its_preferences() {
    let d = prefs_dir("startup");
    std::fs::write(d.join("optionVars.json"), r#"{"upAxisDirection":"z","workingUnitLinearDefault":"m","workingUnitTimeDefault":"ntsc","mine":3}"#).unwrap();
    let mut c = Core::new();
    c.use_prefs();
    assert_eq!(c.mel("upAxis -q -ax;"), json!("z"));
    assert_eq!(c.mel("currentUnit -q -l;"), json!("m"));
    assert_eq!(c.mel("currentUnit -q -t;"), json!("ntsc"));
    assert_eq!(c.mel("optionVar -q mine;"), json!(3));
    close(&c.mel("xform -q -ws -t persp;"), &[0.28, -0.28, 0.21]);
    c.mel("optionVar -iv mine 4; upAxis -ax y -rv;");
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(d.join("optionVars.json")).unwrap()).unwrap();
    assert_eq!(saved["mine"], json!(4));
    assert_eq!(saved["upAxisDirection"], json!("y"));
    aztek_scene::world::set_up_z(false);
    aztek_host::ui_cmds::set_keeps_prefs(false);
    c.mel("newSceneUnits -l cm -t film;");
    c.new_scene();
}
