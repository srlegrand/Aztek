//! Tests: modstack.

use std::sync::Arc;

use aztek_scene::components::{self, ComponentMode};
use aztek_scene::modifiers::{self, ModifierNode};
use aztek_scene::node::types;
use aztek_scene::plug::{flags, AttrDef, PlugType, PlugValue};
use aztek_scene::{NodeType, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn faces(s: &mut Session, m: &Value) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn len(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

/// An extension-style modifier kind that always fails, registered once.
fn register_failing_kind() {
    if types::find("failTestModifier").is_some() {
        return;
    }
    let mesh = |name: &str, f: u32| AttrDef {
        name: name.into(),
        ty: PlugType::Mesh,
        default_value: PlugValue::Mesh(None),
        flags: f,
        ..Default::default()
    };
    let t = NodeType {
        name: "failTestModifier".into(),
        owner: "tests".into(),
        doc: "Always fails, to test how the stack reports a failing modifier.".into(),
        own_attrs: vec![
            mesh("inMesh", flags::INPUT | flags::HIDDEN),
            AttrDef {
                name: "enabled".into(),
                ty: PlugType::Bool,
                default_value: PlugValue::Bool(true),
                flags: flags::INPUT | flags::STORABLE,
                doc: "Turn the modifier off without removing it.".into(),
                ..Default::default()
            },
            mesh("outMesh", flags::OUTPUT | flags::HIDDEN),
        ],
        create: Some(Arc::new(|| Box::new(ModifierNode::new(Arc::new(|_, _| Err("no luck".to_string())), 2)))),
        ..Default::default()
    };
    let _ = types::register_type(t);
}

fn valid_faces(m: &aztek_geo::Mesh) -> usize {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).count()
}

#[test]
fn modifier_kinds_come_from_the_registry() {
    let mut s = Session::new();
    register_failing_kind();
    let kinds = run(&mut s, "modifierKinds", json!({}));
    let kinds = kinds.as_array().unwrap();
    for k in ["mirror", "subdivision", "bevel", "array", "triangulate", "weightedNormals", "solidify", "decimate", "failTest"] {
        assert!(kinds.contains(&json!(k)), "{k}");
    }
    assert!(!kinds.contains(&json!("skinCluster")));
}

#[test]
fn modifier_stack_reports_the_source_each_modifier_and_evaluation_errors() {
    let mut s = Session::new();
    register_failing_kind();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let st = run(&mut s, "modifierStack", json!({"args": [cube]}));
    assert_eq!(st["source"]["type"], "polyCube");
    assert_eq!(len(&st["modifiers"]), 0);

    let bad = run(&mut s, "addModifier", json!({"args": [cube], "kind": "failTest"}));
    // the failing modifier passes its input through
    assert_eq!(faces(&mut s, &cube), 6);
    let st = run(&mut s, "modifierStack", json!({"args": [cube]}));
    assert_eq!(len(&st["modifiers"]), 1);
    assert_eq!(st["modifiers"][0]["name"], bad);
    assert_eq!(st["modifiers"][0]["error"], "no luck");
    // the type has no exportEnabled attr
    assert_eq!(st["modifiers"][0]["exportEnabled"], true);

    run(&mut s, "setAttr", json!({"args": [format!("{}.enabled", bad.as_str().unwrap()), false]}));
    let st = run(&mut s, "modifierStack", json!({"args": [cube]}));
    assert_eq!(st["modifiers"][0]["error"], "");
}

#[test]
fn duplicate_modifier_copies_settings_and_inserts_after_the_original() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let arr = run(&mut s, "addModifier", json!({"args": [cube], "kind": "array", "params": {"count": 2}}));
    let tri = run(&mut s, "addModifier", json!({"args": [cube], "kind": "triangulate"}));
    let copy = run(&mut s, "duplicateModifier", json!({"args": [arr]}));
    assert_eq!(run(&mut s, "modifiers", json!({"args": [cube]})), json!([arr, copy, tri]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{}.count", copy.as_str().unwrap())]})), json!(2));
    assert_eq!(faces(&mut s, &cube), 6 * 2 * 2 * 2);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "modifiers", json!({"args": [cube]})), json!([arr, tri]));
}

#[test]
fn apply_all_modifiers_bakes_the_stack_over_an_editable_source() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "array", "params": {"count": 2}}));
    assert_eq!(faces(&mut s, &cube), 48);
    run(&mut s, "applyAllModifiers", json!({"args": [cube]}));
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [cube]}))), 0);
    assert_eq!(faces(&mut s, &cube), 48);
    let st = run(&mut s, "modifierStack", json!({"args": [cube]}));
    assert_eq!(st["source"]["type"], "meshSource");
    assert_eq!(len(&run(&mut s, "ls", json!({"type": "polyCube"}))), 0);

    // New modifiers stack on the baked base.
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "triangulate"}));
    assert_eq!(faces(&mut s, &cube), 96);
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [cube]}))), 2);
    assert_eq!(run(&mut s, "modifierStack", json!({"args": [cube]}))["source"]["type"], "polyCube");
}

#[test]
fn bake_modifier_stack_leaves_plain_geometry() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let sub = run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    run(&mut s, "bakeModifierStack", json!({"args": [cube]}));
    assert_eq!(faces(&mut s, &cube), 24);
    let st = run(&mut s, "modifierStack", json!({"args": [cube]}));
    assert!(st["source"].is_null());
    assert_eq!(len(&st["modifiers"]), 0);
    assert_eq!(len(&run(&mut s, "ls", json!({"type": "polyCube"}))), 0);
    assert_eq!(len(&run(&mut s, "ls", json!({"type": "subdivisionModifier"}))), 0);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "modifiers", json!({"args": [cube]})), json!([sub]));
    assert_eq!(faces(&mut s, &cube), 24);
}

#[test]
fn modifiers_turned_off_for_export_are_skipped_by_exporters_only() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let sub = run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "array", "params": {"count": 2}}));
    let sub = sub.as_str().unwrap().to_string();
    let mesh = s.graph.find(cube.as_str().unwrap()).unwrap().unwrap();
    assert_eq!(valid_faces(&modifiers::export_mesh(&mut s.graph, mesh).unwrap()), 48);

    run(&mut s, "setAttr", json!({"args": [format!("{sub}.exportEnabled"), false]}));
    assert_eq!(faces(&mut s, &cube), 48);
    assert_eq!(valid_faces(&modifiers::export_mesh(&mut s.graph, mesh).unwrap()), 12);

    // Off in the viewport but on for export: the export runs it.
    run(&mut s, "setAttr", json!({"args": [format!("{sub}.exportEnabled"), true]}));
    run(&mut s, "setAttr", json!({"args": [format!("{sub}.enabled"), false]}));
    assert_eq!(faces(&mut s, &cube), 12);
    assert_eq!(valid_faces(&modifiers::export_mesh(&mut s.graph, mesh).unwrap()), 48);
}

#[test]
fn mesh_of_and_stack_source_walk_the_chain() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    let a = run(&mut s, "addModifier", json!({"args": [cube], "kind": "mirror"}));
    let b = run(&mut s, "addModifier", json!({"args": [cube], "kind": "bevel"}));
    let g = &s.graph;
    let id = |v: &Value| g.find(v.as_str().unwrap()).unwrap().unwrap();
    let mesh = components::as_mesh(g, id(&cube)).unwrap();
    assert_eq!(modifiers::mesh_of(g, id(&a)), mesh);
    assert_eq!(modifiers::mesh_of(g, id(&b)), mesh);
    let src = modifiers::stack_source(g, mesh);
    assert_ne!(src, aztek_scene::NO_NODE);
    assert_eq!(g.node(src).unwrap().ty().name, "polyCube");
    assert_eq!(modifiers::mesh_of(g, src), mesh);
}

#[test]
fn bake_modifier_stack_bakes_generator_and_every_modifier() {
    let mut s = Session::new();
    let cube = run(&mut s, "polyCube", json!({}))[0].clone();
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "array", "params": {"count": 2}}));
    let locator = run(&mut s, "spaceLocator", json!({}));
    run(&mut s, "bakeModifierStack", json!({"args": [cube, locator]}));
    assert_eq!(faces(&mut s, &cube), 48);
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [cube]}))), 0);
    assert_eq!(len(&run(&mut s, "ls", json!({"type": "polyCube"}))), 0);
    assert_eq!(len(&run(&mut s, "ls", json!({"type": "subdivisionModifier"}))), 0);
    run(&mut s, "undo", json!({}));
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [cube]}))), 2);
}

#[test]
fn loop_path_and_contained_face_commands_select_as_one_undo_step() {
    let mut s = Session::new();
    let plane = run(&mut s, "polyPlane", json!({"width": 4, "depth": 4, "subdivisionsX": 4, "subdivisionsZ": 4}))[0].clone();
    let id = components::as_mesh(&s.graph, s.graph.find(plane.as_str().unwrap()).unwrap().unwrap()).unwrap();
    run(&mut s, "selectMode", json!({"args": ["edge"]}));
    let edges = |s: &Session, id, k| components::selected(&s.graph, id, k).len();

    let lp = run(&mut s, "selectLoopThrough", json!({"args": [plane], "edge": 0}));
    assert!(len(&lp) >= 4);
    assert_eq!(edges(&s, id, ComponentMode::Edge), len(&lp));
    run(&mut s, "undo", json!({}));
    assert_eq!(edges(&s, id, ComponentMode::Edge), 0);

    let path = run(&mut s, "selectPathBetween", json!({"args": [plane], "kind": "edge", "from": lp[0], "to": lp[2]}));
    assert_eq!(len(&path), 3);
    assert_eq!(edges(&s, id, ComponentMode::Edge), 3);
    run(&mut s, "undo", json!({}));
    assert_eq!(edges(&s, id, ComponentMode::Edge), 0);

    let fs = run(&mut s, "selectLoopThrough", json!({"args": [plane], "edge": 0, "faces": true}));
    assert_eq!(components::mode(&s.graph), ComponentMode::Face);
    assert_eq!(edges(&s, id, ComponentMode::Face), len(&fs));
    assert!(s.run("selectPathBetween", json!({"args": [plane], "kind": "face", "from": 0})).is_err());

    run(&mut s, "newScene", json!({}));
    let cyl = run(
        &mut s,
        "polyCylinder",
        json!({"radius": 1, "height": 2, "subdivisionsAxis": 8, "subdivisionsHeight": 2, "capType": "ngon"}),
    )[0]
        .clone();
    let cid = s.graph.find(cyl.as_str().unwrap()).unwrap().unwrap();
    let top = run(&mut s, "findComponents", json!({"args": [cyl], "kind": "edge", "extreme": "+y", "count": 8}));
    assert_eq!(len(&top), 8);
    run(&mut s, "selectComponents", json!({"args": [cyl], "edges": top}));
    let out = run(&mut s, "selectContainedFaces", json!({}));
    assert_eq!(len(&out[cyl.as_str().unwrap()]), 1);
    assert_eq!(components::mode(&s.graph), ComponentMode::Face);
    run(&mut s, "undo", json!({}));
    assert_eq!(edges(&s, cid, ComponentMode::Face), 0);
    assert_eq!(edges(&s, cid, ComponentMode::Edge), 8);
}
