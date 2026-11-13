//! Tests: object sets.

use aztek_scene::components::{self, ComponentMode, Op};
use aztek_scene::{scene_file, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn made(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn members(s: &mut Session, set: &str) -> Value {
    run(s, "setMembers", json!({"args": [set]}))
}

#[test]
fn membership_renames_undo() {
    let mut s = Session::new();
    let a = made(run(&mut s, "polyCube", json!({})));
    let b = made(run(&mut s, "polyCube", json!({})));
    let c = made(run(&mut s, "polyCube", json!({})));
    run(&mut s, "select", json!({"args": [a, b]}));
    let set = made(run(&mut s, "createSet", json!({"name": "Props"})));
    assert_eq!(set, "Props");
    assert_eq!(members(&mut s, &set), json!([a, b]));
    assert_eq!(run(&mut s, "listSets", json!({})), json!(["Props"]));
    assert_eq!(run(&mut s, "listSets", json!({"quickSelect": true})), json!([]));

    run(&mut s, "addToSet", json!({"args": [set, c, a]}));
    assert_eq!(members(&mut s, &set), json!([a, b, c]));
    run(&mut s, "undo", json!({}));
    assert_eq!(members(&mut s, &set), json!([a, b]));

    run(&mut s, "removeFromSet", json!({"args": [set, a]}));
    assert_eq!(members(&mut s, &set), json!([b]));
    run(&mut s, "undo", json!({}));

    // An object can be in several sets, unlike a display layer.
    run(&mut s, "createSet", json!({"args": [b], "name": "Pick", "quickSelect": true}));
    assert_eq!(run(&mut s, "listSets", json!({"quickSelect": true})), json!(["Pick"]));
    assert_eq!(run(&mut s, "listSets", json!({"object": b})), json!(["Props", "Pick"]));

    run(&mut s, "rename", json!({"args": [a, "Crate"]}));
    run(&mut s, "rename", json!({"args": [set, "Stuff"]}));
    assert_eq!(members(&mut s, "Stuff"), json!(["Crate", b]));

    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "selectSetMembers", json!({"args": ["Stuff"]}));
    assert_eq!(s.graph.selection().len(), 2);

    // A set may hold a set, as the reference's do, but not itself.
    run(&mut s, "addToSet", json!({"args": ["Stuff", "Pick"]}));
    assert_eq!(members(&mut s, "Stuff"), json!(["Crate", b, "Pick"]));
    run(&mut s, "undo", json!({}));
    assert!(s.run("addToSet", json!({"args": ["Stuff", "Stuff"]})).is_err());
    assert!(s.run("addToSet", json!({"args": ["Stuff", "nothing"]})).is_err());

    run(&mut s, "deleteSet", json!({"args": ["Pick"]}));
    assert_eq!(run(&mut s, "listSets", json!({})), json!(["Stuff"]));
    run(&mut s, "undo", json!({}));
    assert_eq!(members(&mut s, "Pick"), json!([b]));
}

#[test]
fn component_members_and_quick_select() {
    let mut s = Session::new();
    let cube = made(run(&mut s, "polyCube", json!({})));
    let set = made(run(
        &mut s,
        "createSet",
        json!({"args": [format!("{cube}.vtx[0:2]"), format!("{cube}.vtx[5]"), format!("{cube}.f[1]")], "name": "Bits", "quickSelect": true}),
    ));
    assert_eq!(members(&mut s, &set), json!([format!("{cube}.vtx[0:2]"), format!("{cube}.vtx[5]"), format!("{cube}.f[1]")]));
    assert_eq!(run(&mut s, "setMembers", json!({"args": [set], "nodes": true})), json!([cube]));

    run(&mut s, "removeFromSet", json!({"args": [set, format!("{cube}.vtx[1]")]}));
    assert_eq!(
        members(&mut s, &set),
        json!([format!("{cube}.vtx[0]"), format!("{cube}.vtx[2]"), format!("{cube}.vtx[5]"), format!("{cube}.f[1]")])
    );
    run(&mut s, "addToSet", json!({"args": [set, format!("{cube}.vtx[1]")]}));

    // Selecting a component set selects the mesh and its components, in a component mode it has.
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "selectSetMembers", json!({"args": [set]}));
    let id = s.graph.find(&cube).unwrap().unwrap();
    assert_eq!(s.graph.selection(), &[id]);
    assert_eq!(components::mode(&s.graph), ComponentMode::Vertex);
    assert_eq!(components::selected(&s.graph, id, ComponentMode::Vertex), &[0, 1, 2, 5]);
    assert_eq!(components::selected(&s.graph, id, ComponentMode::Face), &[1]);

    // A set made in component mode takes the components, not the objects.
    components::select(&mut s.graph, id, ComponentMode::Vertex, &[3, 4], Op::Replace);
    let from_sel = made(run(&mut s, "createSet", json!({})));
    assert_eq!(members(&mut s, &from_sel), json!([format!("{cube}.vtx[3:4]")]));

    // A topology edit carries component members to their new ids.
    run(&mut s, "selectMode", json!({"args": ["face"]}));
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run(&mut s, "createSet", json!({"args": [format!("{cube}.f[4:5]")], "name": "Top"}));
    run(&mut s, "deleteComponents", json!({}));
    assert_eq!(members(&mut s, "Top"), json!([format!("{cube}.f[3:4]")]));
    run(&mut s, "undo", json!({}));
    assert_eq!(members(&mut s, "Top"), json!([format!("{cube}.f[4:5]")]));

    assert!(s.run("createSet", json!({"args": [format!("{cube}.uv[0]")]})).is_err());
    assert!(s.run("createSet", json!({"args": [format!("{cube}.vtx[3:1]")]})).is_err());
}

#[test]
fn deleted_members_drop_out_and_come_back_on_undo() {
    let mut s = Session::new();
    let a = made(run(&mut s, "polyCube", json!({})));
    let b = made(run(&mut s, "polyCube", json!({})));
    let set = made(run(&mut s, "createSet", json!({"args": [a, format!("{b}.f[0]")]})));
    run(&mut s, "delete", json!({"args": [a]}));
    assert_eq!(members(&mut s, &set), json!([format!("{b}.f[0]")]));
    run(&mut s, "undo", json!({}));
    assert_eq!(members(&mut s, &set), json!([a, format!("{b}.f[0]")]));
    run(&mut s, "delete", json!({"args": [b]}));
    assert_eq!(members(&mut s, &set), json!([a]));

    // A new node with a deleted member's name doesn't join.
    let again = made(run(&mut s, "polyCube", json!({"name": b})));
    assert_eq!(again, b);
    assert_eq!(members(&mut s, &set), json!([a]));
}

#[test]
fn object_sets_save_and_open() {
    let mut s = Session::new();
    let a = made(run(&mut s, "polyCube", json!({})));
    let b = made(run(&mut s, "polyCube", json!({})));
    run(&mut s, "createSet", json!({"args": [a, format!("{b}.e[2:4]")], "name": "Export"}));
    run(&mut s, "createSet", json!({"args": [format!("{b}.vtx[7]")], "name": "Quick", "quickSelect": true}));
    run(&mut s, "createSet", json!({"empty": true, "name": "Empty"}));
    let before = run(&mut s, "listSets", json!({"info": true}));

    for packed in [true, false] {
        let text = serde_json::to_string(&scene_file::serialize(&s.graph, packed)).unwrap();
        run(&mut s, "newScene", json!({}));
        assert_eq!(run(&mut s, "listSets", json!({})), json!([]));
        scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(run(&mut s, "listSets", json!({"info": true})), before);
        // Bound by id after loading: renames keep the membership.
        run(&mut s, "rename", json!({"args": [a, "Renamed"]}));
        assert_eq!(members(&mut s, "Export")[0], "Renamed");
        run(&mut s, "rename", json!({"args": ["Renamed", a]}));
    }
}
