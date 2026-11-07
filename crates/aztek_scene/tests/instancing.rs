//! Tests: instancing.

use std::sync::Arc;

use aztek_scene::instancing;
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::plug::MeshPtr;
use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn first(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn faces(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn len(v: &Value) -> usize {
    v.as_array().unwrap().len()
}

fn kind(s: &mut Session, m: &str) -> String {
    run(s, "instanceInfo", json!({"args": [m]}))["kind"].as_str().unwrap().to_string()
}

/// The mesh shape m names.
fn id(s: &Session, m: &str) -> aztek_scene::plug::NodeId {
    let n = s.graph.find(m).unwrap().unwrap();
    aztek_scene::components::as_mesh(&s.graph, n).unwrap_or(n)
}

fn out_mesh(s: &mut Session, m: &str) -> MeshPtr {
    s.graph.evaluate();
    let n = id(s, m);
    s.graph.node(aztek_scene::components::as_mesh(&s.graph, n).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap().clone()
}

/// A point of the mesh as components are edited on it: the cage under the modifiers.
fn point_y(s: &mut Session, m: &str, p: usize) -> f64 {
    run(s, "meshData", json!({"args": [m], "base": true}))["points"][p][1].as_f64().unwrap()
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn an_instance_shares_its_sources_mesh_and_edits_on_any_member_reach_all() {
    let mut s = Session::new();
    let cube = first(run(&mut s, "polyCube", json!({"name": "Bolt"})));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    let made = run(&mut s, "instance", json!({"args": [cube]}));
    assert_eq!(len(&made), 1);
    let a = first(made);
    let b = first(run(&mut s, "instance", json!({"args": [a]})));

    assert_eq!(kind(&mut s, &cube), "source");
    assert_eq!(kind(&mut s, &a), "instance");
    assert_eq!(kind(&mut s, &b), "instance");
    // An instance of an instance hangs off the root.
    assert_eq!(instancing::instance_parent(&s.graph, id(&s, &b)), id(&s, &cube));
    assert_eq!(run(&mut s, "instanceInfo", json!({"args": [b]}))["source"], json!(cube));
    assert_eq!(len(&run(&mut s, "instanceInfo", json!({"args": [a]}))["members"]), 3);
    assert!(Arc::ptr_eq(&out_mesh(&mut s, &a), &out_mesh(&mut s, &cube)));
    assert!(Arc::ptr_eq(&out_mesh(&mut s, &b), &out_mesh(&mut s, &cube)));
    assert_eq!(faces(&mut s, &a), 24);

    // Edit points on an instance: the shared base changes, so every member moves.
    run(&mut s, "selectComponents", json!({"args": [b], "vertices": [0]}));
    let before = point_y(&mut s, &cube, 0);
    run(&mut s, "moveComponents", json!({"by": [0, 10, 0]}));
    assert!(approx(point_y(&mut s, &cube, 0), before + 10.0));
    assert!(approx(point_y(&mut s, &a, 0), before + 10.0));
    assert_eq!(kind(&mut s, &b), "instance");
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [cube]}))), 1);

    // Topology edits too.
    run(&mut s, "selectComponents", json!({"args": [a], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 5}));
    assert_eq!(faces(&mut s, &cube), faces(&mut s, &a));
    assert_eq!(faces(&mut s, &b), faces(&mut s, &cube));
    assert!(faces(&mut s, &cube) > 24);

    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &cube), 24);
    assert_eq!(faces(&mut s, &b), 24);

    // A modifier added on an instance goes on the shared stack.
    run(&mut s, "addModifier", json!({"args": [a], "kind": "triangulate"}));
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [cube]}))), 2);
    assert!(Arc::ptr_eq(&out_mesh(&mut s, &a), &out_mesh(&mut s, &cube)));

    // Selection helpers.
    assert_eq!(len(&run(&mut s, "selectInstances", json!({"args": [a]}))), 3);
    assert_eq!(run(&mut s, "selectInstanceSource", json!({"args": [b]})), json!([cube]));
}

#[test]
fn a_reference_shares_the_base_but_keeps_a_modifier_stack_of_its_own() {
    let mut s = Session::new();
    let panel = first(run(&mut s, "polyCube", json!({"name": "Panel"})));
    run(&mut s, "addModifier", json!({"args": [panel], "kind": "bevel", "params": {"segments": 1}}));
    let r = first(run(&mut s, "instance", json!({"args": [panel], "reference": true})));
    assert_eq!(kind(&mut s, &panel), "source");
    assert_eq!(kind(&mut s, &r), "reference");
    assert_eq!(run(&mut s, "instanceInfo", json!({"args": [r]}))["source"], json!(panel));
    // It starts with copies of the source's modifiers, its own.
    let mods = run(&mut s, "modifiers", json!({"args": [r]}));
    assert_eq!(len(&mods), 1);
    assert_ne!(mods[0], run(&mut s, "modifiers", json!({"args": [panel]}))[0]);
    assert_eq!(faces(&mut s, &r), faces(&mut s, &panel));

    // Its own stack changes without touching the source...
    run(&mut s, "setAttr", json!({"args": [format!("{}.segments", mods[0].as_str().unwrap()), 3]}));
    assert!(faces(&mut s, &r) > faces(&mut s, &panel));
    // ...but a base edit through either reaches both.
    let (ref_faces, panel_faces) = (faces(&mut s, &r), faces(&mut s, &panel));
    run(&mut s, "selectComponents", json!({"args": [r], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 5}));
    assert!(faces(&mut s, &r) > ref_faces);
    assert!(faces(&mut s, &panel) > panel_faces);
    assert_eq!(kind(&mut s, &r), "reference");

    // A reference of a generator-made mesh shares the generator until an edit freezes it for both.
    run(&mut s, "newScene", json!({}));
    let bx = first(run(&mut s, "polyCube", json!({"name": "Box"})));
    let bx2 = first(run(&mut s, "duplicate", json!({"args": [bx], "mode": "reference"})));
    assert_eq!(kind(&mut s, &bx2), "reference");
    run(&mut s, "selectComponents", json!({"args": [bx2], "vertices": [1]}));
    run(&mut s, "moveComponents", json!({"by": [0, 3, 0]}));
    assert!(approx(point_y(&mut s, &bx, 1), point_y(&mut s, &bx2, 1)));
    assert_eq!(kind(&mut s, &bx2), "reference");
}

#[test]
fn make_unique_breaks_sharing_for_instances_references_and_sources() {
    let mut s = Session::new();
    let cube = first(run(&mut s, "polyCube", json!({"name": "Src"})));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    let a = first(run(&mut s, "instance", json!({"args": [cube]})));
    let b = first(run(&mut s, "instance", json!({"args": [cube]})));

    assert_eq!(run(&mut s, "makeUnique", json!({"args": [a]})), json!([a]));
    assert_eq!(kind(&mut s, &a), "none");
    // It keeps a live copy of the source's stack.
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [a]}))), 1);
    assert_eq!(faces(&mut s, &a), faces(&mut s, &cube));
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 5}));
    assert_ne!(faces(&mut s, &a), faces(&mut s, &cube));
    assert_eq!(faces(&mut s, &b), faces(&mut s, &cube));
    for _ in 0..3 {
        // extrude, the face selection, makeUnique
        run(&mut s, "undo", json!({}));
    }
    assert_eq!(kind(&mut s, &a), "instance");

    // The source going unique hands its data to its instance.
    run(&mut s, "makeUnique", json!({"args": [cube]}));
    assert_eq!(kind(&mut s, &cube), "none");
    assert_eq!(kind(&mut s, &a), "source");
    assert_eq!(kind(&mut s, &b), "instance");
    assert_eq!(run(&mut s, "instanceInfo", json!({"args": [b]}))["source"], json!(a));
    assert_eq!(faces(&mut s, &cube), faces(&mut s, &a));

    // A reference takes its own base.
    run(&mut s, "newScene", json!({}));
    let p = first(run(&mut s, "polyCube", json!({"name": "P"})));
    let r = first(run(&mut s, "instance", json!({"args": [p], "reference": true})));
    run(&mut s, "makeUnique", json!({"args": [r]}));
    assert_eq!(kind(&mut s, &r), "none");
    assert_eq!(kind(&mut s, &p), "none");
    run(&mut s, "selectComponents", json!({"args": [p], "vertices": [0]}));
    run(&mut s, "moveComponents", json!({"by": [0, 5, 0]}));
    assert!(!approx(point_y(&mut s, &p, 0), point_y(&mut s, &r, 0)));
}

#[test]
fn deleting_a_source_promotes_its_first_instance_and_undo_puts_it_back() {
    let mut s = Session::new();
    let cube = first(run(&mut s, "polyCube", json!({"name": "Src"})));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    let a = first(run(&mut s, "instance", json!({"args": [cube]})));
    let b = first(run(&mut s, "instance", json!({"args": [cube]})));
    let n = faces(&mut s, &cube);

    run(&mut s, "delete", json!({"args": [cube]}));
    assert_eq!(faces(&mut s, &a), n);
    assert_eq!(faces(&mut s, &b), n);
    assert_eq!(kind(&mut s, &a), "source");
    assert_eq!(kind(&mut s, &b), "instance");
    assert_eq!(run(&mut s, "instanceInfo", json!({"args": [b]}))["source"], json!(a));
    // The promoted instance owns the stack now.
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [a]}))), 1);

    run(&mut s, "undo", json!({}));
    assert_eq!(kind(&mut s, &cube), "source");
    assert_eq!(kind(&mut s, &a), "instance");
    assert_eq!(run(&mut s, "instanceInfo", json!({"args": [b]}))["source"], json!(cube));

    // Deleting a group holding the source and one instance promotes the instance outside it.
    run(&mut s, "group", json!({"args": [cube, a], "name": "Grp"}));
    run(&mut s, "delete", json!({"args": ["Grp"]}));
    assert_eq!(faces(&mut s, &b), n);
    assert_eq!(kind(&mut s, &b), "none");

    // A source with a stored base hands that over too.
    run(&mut s, "newScene", json!({}));
    let plain = first(run(&mut s, "polyCube", json!({"name": "Plain"})));
    run(&mut s, "bakeModifierStack", json!({"args": [plain]}));
    let c = first(run(&mut s, "instance", json!({"args": [plain]})));
    run(&mut s, "delete", json!({"args": [plain]}));
    assert_eq!(faces(&mut s, &c), 6);
    assert_eq!(kind(&mut s, &c), "none");
}

#[test]
fn convert_to_instance_replaces_identical_copies_and_leaves_different_meshes_alone() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({"name": "Kit"})));
    run(&mut s, "bakeModifierStack", json!({"args": [a]}));
    let b = first(run(&mut s, "duplicate", json!({"args": [a]})));
    let c = first(run(&mut s, "duplicate", json!({"args": [a]})));
    run(&mut s, "move", json!({"args": [b], "by": [100, 0, 0]}));
    let other = first(run(&mut s, "polySphere", json!({"name": "Ball"})));
    run(&mut s, "selectComponents", json!({"args": [c], "vertices": [0]}));
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    run(&mut s, "select", json!({"args": [a]}));

    let converted = run(&mut s, "convertToInstance", json!({"args": [a]}));
    assert_eq!(converted, json!([b]));
    assert_eq!(kind(&mut s, &b), "instance");
    assert_eq!(kind(&mut s, &c), "none");
    assert_eq!(kind(&mut s, &other), "none");
    // The instance keeps its own transform.
    assert!(approx(run(&mut s, "getAttr", json!({"args": [format!("{b}.tx")]})).as_f64().unwrap(), 100.0));

    // force converts a different mesh when named.
    assert_eq!(run(&mut s, "convertToInstance", json!({"args": [c, a], "force": true})), json!([c]));
    assert_eq!(kind(&mut s, &c), "instance");
    run(&mut s, "undo", json!({}));
    assert_eq!(kind(&mut s, &c), "none");
}

#[test]
fn freeze_transforms_on_an_instance_makes_it_unique_instead_of_moving_every_member() {
    let mut s = Session::new();
    let cube = first(run(&mut s, "polyCube", json!({"name": "Src"})));
    let a = first(run(&mut s, "instance", json!({"args": [cube]})));
    run(&mut s, "move", json!({"args": [a], "by": [0, 100, 0]}));
    let y = point_y(&mut s, &cube, 0);
    run(&mut s, "freezeTransforms", json!({"args": [a]}));
    assert!(approx(point_y(&mut s, &cube, 0), y));
    assert!(approx(point_y(&mut s, &a, 0), y + 100.0));
    assert_eq!(kind(&mut s, &a), "none");
}
