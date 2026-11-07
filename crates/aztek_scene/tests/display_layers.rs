//! Tests: display layers.

use aztek_scene::display_layers::{self as dl, DisplayType};
use aztek_scene::plug::NodeId;
use aztek_scene::transform::effectively_visible;
use aztek_scene::transform::Transform;
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn made(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn id(s: &Session, n: &str) -> NodeId {
    s.graph.find(n).unwrap().unwrap()
}

fn layer_name(s: &Session, node: &str) -> String {
    dl::layer_of(&s.graph, id(s, node)).map(|l| s.graph.node(l).unwrap().name().to_string()).unwrap_or_default()
}

fn visible(s: &mut Session, n: &str) -> bool {
    s.graph.evaluate();
    effectively_visible(&s.graph, id(s, n))
}

#[test]
fn membership_one_layer_per_object_undo() {
    let mut s = Session::new();
    let a = made(run(&mut s, "polyCube", json!({})));
    let b = made(run(&mut s, "polyCube", json!({})));
    let c = made(run(&mut s, "polyCube", json!({})));
    let l1 = made(run(&mut s, "createDisplayLayer", json!({"args": [a, b], "name": "Props"})));
    assert_eq!(l1, "Props");
    assert_eq!(layer_name(&s, &a), "Props");
    assert!(layer_name(&s, &c).is_empty());
    assert_eq!(run(&mut s, "displayLayer", json!({"args": [l1]}))["members"], json!([a, b]));

    // Joining another layer leaves the first.
    let l2 = made(run(&mut s, "createDisplayLayer", json!({"args": [b]})));
    assert_eq!(layer_name(&s, &b), l2);
    assert_eq!(run(&mut s, "displayLayer", json!({"args": [l1]}))["members"], json!([a]));
    run(&mut s, "undo", json!({}));
    assert!(s.graph.find(&l2).unwrap().is_none());
    assert_eq!(layer_name(&s, &b), "Props");

    run(&mut s, "editDisplayLayerMembers", json!({"args": [l1, c]}));
    assert_eq!(layer_name(&s, &c), "Props");
    run(&mut s, "editDisplayLayerMembers", json!({"args": ["defaultLayer", c]}));
    assert!(layer_name(&s, &c).is_empty());
    run(&mut s, "removeFromDisplayLayer", json!({"args": [a]}));
    assert!(layer_name(&s, &a).is_empty());
    run(&mut s, "undo", json!({}));
    assert_eq!(layer_name(&s, &a), "Props");

    // Renames on either side keep the membership.
    run(&mut s, "rename", json!({"args": [a, "Crate"]}));
    run(&mut s, "displayLayer", json!({"args": [l1], "name": "Set"}));
    assert_eq!(layer_name(&s, "Crate"), "Set");

    run(&mut s, "selectDisplayLayerMembers", json!({"args": ["Set"]}));
    assert_eq!(s.graph.selection().len(), 2);

    run(&mut s, "deleteDisplayLayer", json!({"args": ["Set"]}));
    assert!(layer_name(&s, "Crate").is_empty());
    assert!(s.graph.find("Crate").unwrap().is_some());
    run(&mut s, "undo", json!({}));
    assert_eq!(layer_name(&s, "Crate"), "Set");

    let copy = made(run(&mut s, "duplicate", json!({"args": ["Crate"]})));
    assert_eq!(layer_name(&s, &copy), "Set");
}

#[test]
fn visibility_and_hide_on_playback() {
    let mut s = Session::new();
    let cube = made(run(&mut s, "polyCube", json!({})));
    let grp = made(run(&mut s, "group", json!({"args": [cube]})));
    let layer = made(run(&mut s, "createDisplayLayer", json!({"args": [grp]})));
    assert!(visible(&mut s, &cube));

    run(&mut s, "displayLayer", json!({"args": [layer], "visibility": false}));
    assert!(!visible(&mut s, &grp));
    assert!(!visible(&mut s, &cube));
    assert!(s.graph.node(id(&s, &cube)).unwrap().bool(Transform::VISIBILITY));
    run(&mut s, "undo", json!({}));
    assert!(visible(&mut s, &cube));

    run(&mut s, "displayLayer", json!({"args": [layer], "hideOnPlayback": true}));
    assert!(visible(&mut s, &cube));
    dl::set_play_direction(1);
    let hidden = !visible(&mut s, &cube);
    dl::set_play_direction(0);
    assert!(hidden);
    assert!(visible(&mut s, &cube));
}

#[test]
fn template_and_reference_are_drawn_but_not_pickable_colour() {
    let mut s = Session::new();
    let cube = made(run(&mut s, "polyCube", json!({})));
    let grp = made(run(&mut s, "group", json!({"args": [cube]})));
    let other = made(run(&mut s, "polyCube", json!({})));
    let layer = made(run(&mut s, "createDisplayLayer", json!({"args": [grp]})));
    let c = id(&s, &cube);
    assert!(dl::pickable(&s.graph, c));
    assert!(dl::display(&s.graph, c).color.is_none());

    run(&mut s, "displayLayer", json!({"args": [layer], "displayType": "template"}));
    assert_eq!(dl::display(&s.graph, c).ty, DisplayType::Template);
    assert!(!dl::pickable(&s.graph, c));
    assert!(visible(&mut s, &cube));
    assert!(dl::pickable(&s.graph, id(&s, &other)));

    run(&mut s, "displayLayer", json!({"args": [layer], "displayType": "reference", "rgbColor": [1.0, 0.0, 0.0]}));
    assert!(!dl::pickable(&s.graph, c));
    assert_eq!(dl::display(&s.graph, c).color.unwrap()[0], 1.0);

    // A child in a layer of its own follows that one, not its parent's.
    run(&mut s, "createDisplayLayer", json!({"args": [cube]}));
    assert!(dl::pickable(&s.graph, c));
    assert!(dl::display(&s.graph, c).color.is_none());

    run(&mut s, "displayLayer", json!({"args": [layer], "color": 0}));
    assert!(run(&mut s, "displayLayer", json!({"args": [layer]}))["color"].is_null());
    assert!(s.run("displayLayer", json!({"args": [layer], "displayType": "ghost"})).is_err());
    assert!(s.run("displayLayer", json!({"args": [cube]})).is_err());
}

#[test]
fn order() {
    let mut s = Session::new();
    for n in ["A", "B", "C"] {
        run(&mut s, "createDisplayLayer", json!({"empty": true, "name": n}));
    }
    let names = |s: &mut Session| -> Vec<String> {
        run(s, "listDisplayLayers", json!({})).as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(names(&mut s), ["A", "B", "C"]);
    run(&mut s, "reorderDisplayLayer", json!({"args": ["C"], "index": 0}));
    assert_eq!(names(&mut s), ["C", "A", "B"]);
    run(&mut s, "undo", json!({}));
    assert_eq!(names(&mut s), ["A", "B", "C"]);
    run(&mut s, "reorderDisplayLayer", json!({"args": ["A"], "index": 9}));
    assert_eq!(names(&mut s), ["B", "C", "A"]);
}
