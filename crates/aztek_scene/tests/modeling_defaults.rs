//! Tests: modeling defaults.

use aztek_geo::names;
use aztek_scene::isolation::{inherit_into, Set};
use aztek_scene::mesh_nodes::MeshNode;
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

fn points(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["points"].as_i64().unwrap()
}

fn unmatched(s: &mut Session, m: &str) -> i64 {
    run(s, "symmetryCheck", json!({"args": [m]}))[m]["unmatched"].as_i64().unwrap()
}

fn plane(s: &mut Session, width: f64, sx: i64, sz: i64) -> String {
    first(run(s, "polyPlane", json!({"width": width, "depth": 100.0, "subdivisionsX": sx, "subdivisionsZ": sz})))
}

fn cube(s: &mut Session) -> String {
    first(run(s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0})))
}

fn hard_edges(s: &mut Session, m: &str) -> usize {
    s.graph.evaluate();
    let id = s.graph.find(m).unwrap().unwrap();
    let mesh = s.graph.node(aztek_scene::components::as_mesh(&s.graph, id).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap().clone();
    let hard = mesh.edge_attrs().get::<bool>(names::HARD);
    (0..hard.len() as u32).filter(|&e| mesh.edge_valid(e) && hard[e as usize]).count()
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn poly_insert_loop_follows_symmetry() {
    let mut s = Session::new();
    let p = plane(&mut s, 100.0, 4, 1);
    run(&mut s, "setSymmetry", json!({"args": [p], "axis": "x"}));
    run(&mut s, "selectComponents", json!({"args": [p], "edges": [0]}));
    run(&mut s, "polyInsertLoop", json!({}));
    assert_eq!(faces(&mut s, &p), 6);
    assert_eq!(unmatched(&mut s, &p), 0);
    run(&mut s, "undo", json!({}));
    run(&mut s, "selectComponents", json!({"args": [p], "edges": [0]}));
    run(&mut s, "polyInsertLoop", json!({"count": 2}));
    assert_eq!(faces(&mut s, &p), 8);
}

#[test]
fn poly_insert_loop_spreads_several_loops_under_symmetry() {
    let mut s = Session::new();
    let p = plane(&mut s, 100.0, 4, 1);
    run(&mut s, "setSymmetry", json!({"args": [p], "axis": "x"}));
    run(&mut s, "selectComponents", json!({"args": [p], "edges": [0]}));
    run(&mut s, "polyInsertLoop", json!({"count": 2, "fraction": 0.3, "spacing": 0.5}));
    assert_eq!(faces(&mut s, &p), 8);
    assert_eq!(unmatched(&mut s, &p), 0);
}

#[test]
fn offset_loop_and_edit_edge_flow_are_undoable_edits() {
    let mut s = Session::new();
    let p = plane(&mut s, 100.0, 4, 4);
    run(&mut s, "selectComponents", json!({"args": [p], "edges": [0]}));
    run(&mut s, "selectLoop", json!({}));
    run(&mut s, "polyOffsetLoop", json!({}));
    // The border loop has faces on one side only: each of the 12 border faces is cut once.
    assert_eq!(faces(&mut s, &p), 28);
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &p), 16);

    let before = run(&mut s, "meshData", json!({"args": [p], "base": true}));
    let centre = before["points"]
        .as_array()
        .unwrap()
        .iter()
        .position(|q| q[0].as_f64().unwrap().abs() < 1e-6 && q[2].as_f64().unwrap().abs() < 1e-6)
        .expect("centre point");
    run(&mut s, "selectComponents", json!({"args": [p], "vertices": [centre]}));
    run(&mut s, "moveComponents", json!({"by": [0.0, 10.0, 0.0]}));
    run(&mut s, "polyEditEdgeFlow", json!({"adjustEdgeFlow": 0.0}));
    let after = run(&mut s, "meshData", json!({"args": [p], "base": true}));
    assert!(approx(after["points"][centre][1].as_f64().unwrap(), 0.0));
    run(&mut s, "undo", json!({}));
    let undone = run(&mut s, "meshData", json!({"args": [p], "base": true}));
    assert!(approx(undone["points"][centre][1].as_f64().unwrap(), 10.0));
}

#[test]
fn poly_duplicate_faces_copies_into_objects_of_their_own() {
    // As the reference's Duplicate on faces: polyChipOff with duplicate on, then polySeparate.
    let mut s = Session::new();
    let c = cube(&mut s);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [0]}));
    let made = run(&mut s, "polyDuplicateFaces", json!({}));
    let made: Vec<String> = made.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    assert_eq!(made.len(), 2, "{made:?}");
    let mut counts: Vec<i64> = made.iter().map(|m| faces(&mut s, m)).collect();
    counts.sort();
    assert_eq!(counts, [1, 6]);
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &c), 6);
}

#[test]
fn component_edits_in_object_mode_take_the_whole_mesh() {
    let mut s = Session::new();
    let p = first(run(&mut s, "polyPlane", json!({"width": 100.0, "depth": 100.0, "subdivisionsX": 1, "subdivisionsZ": 1})));
    let before = faces(&mut s, &p);
    run(&mut s, "select", json!({"args": [p]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    // The source case predates keepBack, which (on by default) would also close the lone face's back.
    run(&mut s, "polyExtrude", json!({"thickness": 1.0, "keepBack": false}));
    assert_eq!(faces(&mut s, &p), before + 4);
    assert_eq!(run(&mut s, "selectMode", json!({})), "face");
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &p), before);
    assert_eq!(run(&mut s, "selectMode", json!({})), "object");

    let c = cube(&mut s);
    run(&mut s, "select", json!({"args": [c]}));
    run(&mut s, "polySoftEdge", json!({"angle": 0.0}));
    assert_eq!(hard_edges(&mut s, &c), 12);
    assert_eq!(run(&mut s, "selectMode", json!({})), "object");
    run(&mut s, "polySoftEdge", json!({"angle": 180.0}));
    assert_eq!(hard_edges(&mut s, &c), 0);
    run(&mut s, "polySoftEdge", json!({"angle": 30.0}));
    assert_eq!(hard_edges(&mut s, &c), 12);
}

#[test]
fn poly_mirror_flip_keeps_nothing_of_the_original() {
    let mut s = Session::new();
    let c = cube(&mut s);
    run(&mut s, "setAttr", json!({"args": [format!("{c}.translate"), [20.0, 0.0, 0.0]]}));
    run(&mut s, "polyMirror", json!({"args": [c], "space": "world", "flip": true}));
    assert_eq!(faces(&mut s, &c), 6);
    let data = run(&mut s, "meshData", json!({"args": [c]}));
    let xs: Vec<f64> = data["points"].as_array().unwrap().iter().map(|p| p[0].as_f64().unwrap()).collect();
    assert!(approx(xs.iter().copied().fold(f64::MAX, f64::min), -90.0));
    assert!(approx(xs.iter().copied().fold(f64::MIN, f64::max), 10.0));
}

#[test]
fn show_last_hidden_shows_what_the_last_hide_hid() {
    let mut s = Session::new();
    let a = cube(&mut s);
    let b = cube(&mut s);
    run(&mut s, "hide", json!({"args": [a]}));
    run(&mut s, "hide", json!({"args": [b]}));
    let shown = run(&mut s, "showLastHidden", json!({}));
    assert_eq!(shown.as_array().unwrap().len(), 1);
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{b}.visibility")]})), true);
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{a}.visibility")]})), false);
}

#[test]
fn isolation_sets_take_in_objects_made_from_isolated_ones() {
    let mut set = Set::default();
    assert!(!inherit_into(&mut set, &[7], &[1]));
    set.nodes = [1, 2].into_iter().collect();
    assert!(!inherit_into(&mut set, &[7], &[3]));
    assert!(inherit_into(&mut set, &[7, 8], &[3, 2]));
    assert!(set.nodes.contains(&7));
    assert!(set.nodes.contains(&8));
}

#[test]
fn merging_exactly_two_vertices_ignores_the_distance() {
    let mut s = Session::new();
    let p = plane(&mut s, 100.0, 1, 1);
    run(&mut s, "selectComponents", json!({"args": [p], "vertices": [0, 1]}));
    run(&mut s, "polyMerge", json!({}));
    assert_eq!(points(&mut s, &p), 3);
    run(&mut s, "undo", json!({}));
    run(&mut s, "selectComponents", json!({"args": [p], "vertices": [0, 1, 2]}));
    run(&mut s, "polyMerge", json!({}));
    assert_eq!(points(&mut s, &p), 4);
}

#[test]
fn symmetric_selection_picks_the_mirror_images() {
    let mut s = Session::new();
    let p = plane(&mut s, 100.0, 4, 1);
    let selected_faces = |s: &mut Session| {
        let sel = run(s, "componentSelection", json!({}));
        sel.as_object()
            .and_then(|o| o.values().next())
            .and_then(|v| v.get("faces"))
            .and_then(Value::as_array)
            .map_or(0, Vec::len)
    };
    run(&mut s, "selectComponents", json!({"args": [p], "faces": [0], "symmetric": true}));
    assert_eq!(selected_faces(&mut s), 1);
    run(&mut s, "setSymmetry", json!({"args": [p], "axis": "x"}));
    run(&mut s, "selectComponents", json!({"args": [p], "faces": [0], "symmetric": true}));
    assert_eq!(selected_faces(&mut s), 2);
    run(&mut s, "selectComponents", json!({"args": [p], "faces": [1]}));
    run(&mut s, "symmetrizeSelection", json!({}));
    assert_eq!(selected_faces(&mut s), 2);
    run(&mut s, "polySplitRings", json!({"count": 1}));
    assert_eq!(unmatched(&mut s, &p), 0);
}

#[test]
fn poly_split_rings_in_object_mode_splits_the_whole_mesh() {
    let mut s = Session::new();
    let p = plane(&mut s, 400.0, 4, 1);
    run(&mut s, "select", json!({"args": [p]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    // Along the strip: one loop down its length. Across: one per column.
    run(&mut s, "polySplitRings", json!({}));
    assert_eq!(faces(&mut s, &p), 8);
    assert_eq!(points(&mut s, &p), 15);
    run(&mut s, "undo", json!({}));
    run(&mut s, "select", json!({"args": [p]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "polySplitRings", json!({"direction": "across"}));
    assert_eq!(points(&mut s, &p), 18);
}

#[test]
fn split_rings_modifier_splits_live_on_the_selection_or_the_whole_mesh() {
    let mut s = Session::new();
    let p = plane(&mut s, 400.0, 4, 1);
    run(&mut s, "select", json!({"args": [p]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    let whole = first(run(&mut s, "addModifier", json!({"args": [p], "kind": "splitRings"})));
    assert_eq!(faces(&mut s, &p), 8);
    run(&mut s, "setAttr", json!({"args": [format!("{whole}.count"), 3]}));
    assert_eq!(faces(&mut s, &p), 16);
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));

    run(&mut s, "selectComponents", json!({"args": [p], "edges": [0]}));
    run(&mut s, "addModifier", json!({"args": [p], "kind": "splitRings"}));
    assert_eq!(faces(&mut s, &p), 5);
}

#[test]
fn split_rings_modifier_selection_can_be_edited_applied_and_cancelled() {
    let mut s = Session::new();
    let p = plane(&mut s, 400.0, 4, 1);
    run(&mut s, "select", json!({"args": [p]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    let m = first(run(&mut s, "addModifier", json!({"args": [p], "kind": "splitRings"})));
    assert_eq!(faces(&mut s, &p), 8);

    run(&mut s, "splitRingsEditSelection", json!({"args": [m]}));
    assert_eq!(faces(&mut s, &p), 4);
    assert_eq!(run(&mut s, "selectMode", json!({})), "face");
    assert_eq!(run(&mut s, "componentSelection", json!({}))[&p]["faces"].as_array().unwrap().len(), 4);
    run(&mut s, "selectComponents", json!({"args": [p], "edges": [0]}));
    run(&mut s, "splitRingsApplySelection", json!({"args": [p]}));
    assert_eq!(faces(&mut s, &p), 5);
    assert_eq!(run(&mut s, "selectMode", json!({})), "object");

    run(&mut s, "splitRingsEditSelection", json!({"args": [p]}));
    assert_eq!(run(&mut s, "componentSelection", json!({}))[&p]["edges"], json!([0]));
    run(&mut s, "selectComponents", json!({"args": [p], "edges": [1, 2]}));
    run(&mut s, "splitRingsCancelSelection", json!({"args": [p]}));
    assert_eq!(faces(&mut s, &p), 5);

    run(&mut s, "splitRingsEditSelection", json!({"args": [p]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    assert!(s.run("splitRingsApplySelection", json!({"args": [p]})).is_err());
    run(&mut s, "splitRingsCancelSelection", json!({"args": [p]}));
    assert_eq!(faces(&mut s, &p), 5);
}
