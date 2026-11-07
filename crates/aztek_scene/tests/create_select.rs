//! Tests: create select.

use aztek_geo::names;
use aztek_ops::{execute, CallSource};
use aztek_scene::plug::PlugValue;
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn first(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn face_mode(s: &mut Session, mesh: &str) {
    run(s, "selectComponents", json!({"args": [mesh], "faces": [0]}));
    assert_eq!(run(s, "selectMode", json!({})), "face");
}

fn mode(s: &mut Session) -> Value {
    run(s, "selectMode", json!({}))
}

fn selection(s: &mut Session) -> Value {
    run(s, "ls", json!({"selection": true}))
}

#[test]
fn convert_selection_in_object_mode_takes_every_component_of_the_selected_meshes() {
    let mut s = Session::new();
    let cube = first(run(&mut s, "polyCube", json!({})));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "select", json!({"args": [cube]}));

    run(&mut s, "convertSelection", json!({"args": ["edge"]}));
    assert_eq!(mode(&mut s), "edge");
    assert_eq!(run(&mut s, "componentSelection", json!({}))[&cube]["edges"].as_array().unwrap().len(), 12);

    run(&mut s, "undo", json!({}));
    assert_eq!(mode(&mut s), "object");
    assert!(run(&mut s, "componentSelection", json!({})).get(&cube).is_none());
    run(&mut s, "redo", json!({}));
    assert_eq!(mode(&mut s), "edge");
    assert_eq!(run(&mut s, "componentSelection", json!({}))[&cube]["edges"].as_array().unwrap().len(), 12);

    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "convertSelection", json!({"args": ["vertex"]}));
    assert_eq!(mode(&mut s), "vertex");
    assert_eq!(run(&mut s, "componentSelection", json!({}))[&cube]["vertices"].as_array().unwrap().len(), 8);

    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "select", json!({"clear": true}));
    assert!(s.run("convertSelection", json!({"args": ["face"]})).is_err());
    assert_eq!(mode(&mut s), "object");
}

#[test]
fn creating_a_node_lands_in_object_mode_with_the_new_node_selected() {
    let mut s = Session::new();
    let cube = first(run(&mut s, "polyCube", json!({})));

    face_mode(&mut s, &cube);
    let loc = first(run(&mut s, "spaceLocator", json!({})));
    assert_eq!(mode(&mut s), "object");
    assert_eq!(selection(&mut s), json!([loc]));

    face_mode(&mut s, &cube);
    let group = first(run(&mut s, "group", json!({"empty": true})));
    assert_eq!(mode(&mut s), "object");
    assert_eq!(selection(&mut s), json!([group]));

    face_mode(&mut s, &cube);
    let sphere = first(run(&mut s, "polySphere", json!({})));
    assert_eq!(mode(&mut s), "object");
    assert_eq!(selection(&mut s), json!([sphere]));

    face_mode(&mut s, &cube);
    let node = first(run(&mut s, "createNode", json!({"args": ["transform"]})));
    assert_eq!(mode(&mut s), "object");
    assert_eq!(selection(&mut s), json!([node]));

    if s.commands.find("joint").is_some() {
        face_mode(&mut s, &cube);
        let joint = first(run(&mut s, "joint", json!({})));
        assert_eq!(mode(&mut s), "object");
        assert_eq!(selection(&mut s), json!([joint]));
    }

    // Edits that make no new selected node keep the component mode.
    face_mode(&mut s, &cube);
    run(&mut s, "polyExtrude", json!({"thickness": 1}));
    assert_eq!(mode(&mut s), "face");
    face_mode(&mut s, &cube);
    run(&mut s, "createNode", json!({"args": ["transform"], "skipSelect": true}));
    assert_eq!(mode(&mut s), "face");
}

#[test]
fn picking_another_object_from_the_ui_leaves_component_mode() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({})));
    let b = first(run(&mut s, "polySphere", json!({})));
    run(&mut s, "select", json!({"args": [a]}));
    let pick = |s: &mut Session, call: Value| {
        execute(s, "select", &call, CallSource::Ui).unwrap();
    };

    face_mode(&mut s, &a);
    pick(&mut s, json!({"args": [b]}));
    assert_eq!(mode(&mut s), "object");
    assert_eq!(selection(&mut s), json!([b]));

    // Reselecting what is already selected, or deselecting, keeps the mode.
    run(&mut s, "select", json!({"args": [a, b]}));
    face_mode(&mut s, &a);
    pick(&mut s, json!({"args": [a]}));
    assert_eq!(mode(&mut s), "face");
    pick(&mut s, json!({"args": [a], "deselect": true}));
    assert_eq!(mode(&mut s), "face");

    // Scripts select without changing the mode.
    run(&mut s, "select", json!({"args": [a]}));
    face_mode(&mut s, &a);
    run(&mut s, "select", json!({"args": [b]}));
    assert_eq!(mode(&mut s), "face");
}

#[test]
fn create_mesh_puts_each_face_corners_uv_on_that_corner() {
    let mut s = Session::new();
    let made = run(
        &mut s,
        "createMesh",
        json!({
            "points": [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0], [2, 0, 0]],
            "faces": [[0, 1, 2, 3], [1, 4, 2]],
            "uvs": [[0, 0], [1, 0], [1, 1], [0, 1], [0.5, 0], [1, 0.5], [0.5, 0.5]],
            "name": "UvQuad",
        }),
    );
    let p = s.graph.plug(&format!("{}Shape.inMesh", first(made))).unwrap();
    let PlugValue::Mesh(Some(m)) = s.graph.stored_value(&p).clone() else { panic!("no mesh") };
    let uv = m.corner_attrs().get::<[f32; 2]>(&names::uv(0));
    assert!(uv.len() >= 7);
    let faces = [vec![0u32, 1, 2, 3], vec![1, 4, 2]];
    let want = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [0.5, 0.0], [1.0, 0.5], [0.5, 0.5]];
    let mut at = 0;
    for f in 0..m.num_faces() {
        let mut c = m.face_corner(f);
        for &point in &faces[f as usize] {
            assert_eq!(m.corner_point(c), point);
            assert_eq!(uv[c as usize], want[at]);
            at += 1;
            c = m.corner_next(c);
        }
    }

    assert!(s
        .run("createMesh", json!({"points": [[0, 0, 0], [1, 0, 0], [1, 1, 0]], "faces": [[0, 1, 2]], "uvs": [[0, 0]]}))
        .is_err());
}

#[test]
fn smooth_preview_on_a_group_reaches_every_mesh_below_it() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({})));
    let b = first(run(&mut s, "polyCube", json!({})));
    let inner = first(run(&mut s, "group", json!({"args": [b]})));
    let outer = first(run(&mut s, "group", json!({"args": [a, inner]})));
    run(&mut s, "smoothPreview", json!({"args": [outer], "level": 2}));
    let meshes = run(&mut s, "ls", json!({"type": "mesh"}));
    assert_eq!(meshes.as_array().unwrap().len(), 2);
    for mesh in meshes.as_array().unwrap() {
        let v = run(&mut s, "getAttr", json!({"args": [format!("{}.smoothPreview", mesh.as_str().unwrap())]}));
        assert_eq!(v.as_i64().unwrap(), 2);
    }
}

#[test]
fn pick_walk_after_a_delete_walks_on_from_the_deleted_objects_place() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({"name": "a"})));
    let b = first(run(&mut s, "polyCube", json!({"name": "b"})));
    let c = first(run(&mut s, "polyCube", json!({"name": "c"})));
    let grp = first(run(&mut s, "group", json!({"args": [a, b, c]})));
    let kids = run(&mut s, "listRelatives", json!({"args": [grp]}));
    assert_eq!(kids.as_array().unwrap().len(), 3);

    run(&mut s, "select", json!({"args": [kids[1]]}));
    run(&mut s, "delete", json!({}));
    run(&mut s, "pickWalk", json!({"direction": "right"}));
    assert_eq!(selection(&mut s), json!([kids[2]]));
    run(&mut s, "delete", json!({}));
    run(&mut s, "pickWalk", json!({"direction": "right"}));
    assert_eq!(selection(&mut s), json!([kids[0]]));

    for _ in 0..4 {
        run(&mut s, "undo", json!({}));
    }
    assert_eq!(run(&mut s, "listRelatives", json!({"args": [grp]})).as_array().unwrap().len(), 3);
    run(&mut s, "select", json!({"args": [kids[1]]}));
    run(&mut s, "delete", json!({}));
    run(&mut s, "pickWalk", json!({"direction": "left"}));
    assert_eq!(selection(&mut s), json!([kids[0]]));
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));

    // Once the delete is undone, the walk goes from the selection as always.
    run(&mut s, "select", json!({"args": [kids[1]]}));
    run(&mut s, "delete", json!({}));
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": [grp]})).as_array().unwrap().len(), 3);
    let after_undo = selection(&mut s);
    run(&mut s, "pickWalk", json!({"direction": "right"}));
    let expect = if after_undo.as_array().unwrap().is_empty() { json!([]) } else { json!([kids[2]]) };
    assert_eq!(selection(&mut s), expect);

    // Selecting anything after the delete drops the remembered place.
    run(&mut s, "select", json!({"args": [kids[1]]}));
    run(&mut s, "delete", json!({}));
    run(&mut s, "select", json!({"args": [kids[0]]}));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "pickWalk", json!({"direction": "right"}));
    assert!(selection(&mut s).as_array().unwrap().is_empty());
}
