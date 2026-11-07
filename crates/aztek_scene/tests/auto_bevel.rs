//! Tests: auto bevel.

use aztek_geo::{Index, Mesh, INVALID};
use aztek_scene::auto_bevel;
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::{scene_file, Session};
use glam::DVec3;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn faces(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn first(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn cube(s: &mut Session) -> String {
    first(run(s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0})))
}

fn edge_at(m: &Mesh, a: DVec3, b: DVec3) -> Index {
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) {
            continue;
        }
        let (p, q) = (m.position(m.edge_point(e, 0)), m.position(m.edge_point(e, 1)));
        if (p.distance(a) < 1e-6 && q.distance(b) < 1e-6) || (p.distance(b) < 1e-6 && q.distance(a) < 1e-6) {
            return e;
        }
    }
    INVALID
}

fn input_of(s: &mut Session, m: &str) -> Mesh {
    let id = s.graph.find(m).unwrap().unwrap();
    (*auto_bevel::chosen(&mut s.graph, id).unwrap().mesh.unwrap()).clone()
}

/// Edit Edges on a modifier, picking exactly the edge between a and b on its input.
fn pick_edge(s: &mut Session, cube: &str, m: &str, a: DVec3, b: DVec3) {
    run(s, "bevelEditEdges", json!({"args": [m]}));
    let e = edge_at(&input_of(s, m), a, b);
    assert_ne!(e, INVALID);
    run(s, "selectComponents", json!({"args": [cube], "edges": [e]}));
    run(s, "bevelApplyEdges", json!({"args": [m]}));
}

/// The bevelled edge of a one-edge modifier, as its endpoints.
fn picked_edge(s: &mut Session, m: &str) -> (DVec3, DVec3) {
    let info = run(s, "bevelEdgeInfo", json!({"args": [m]}));
    assert_eq!(info["count"], 1);
    assert_eq!(info["lost"], 0);
    let input = input_of(s, m);
    let e = info["edges"][0].as_u64().unwrap() as Index;
    (input.position(input.edge_point(e, 0)), input.position(input.edge_point(e, 1)))
}

fn same_edge(e: (DVec3, DVec3), a: DVec3, b: DVec3) -> bool {
    let near = |p: DVec3, q: DVec3| p.distance(q) < 1e-6;
    (near(e.0, a) && near(e.1, b)) || (near(e.0, b) && near(e.1, a))
}

#[test]
fn auto_bevel_modifier_bevels_folds_and_stays_live() {
    let mut s = Session::new();
    let c = cube(&mut s);
    let m = first(run(&mut s, "autoBevel", json!({"args": [c], "segments": 1, "offset": 5.0})));
    assert_eq!(run(&mut s, "bevelEdgeInfo", json!({"args": [m]}))["count"], 12);
    // A one-segment bevel of every cube edge: 6 faces, 12 strips, 8 corners.
    assert_eq!(faces(&mut s, &c), 26);
    run(&mut s, "setAttr", json!({"args": [format!("{m}.minAngle"), 100.0]}));
    assert_eq!(faces(&mut s, &c), 6);
    run(&mut s, "setAttr", json!({"args": [format!("{m}.mode"), "angle"]}));
    run(&mut s, "setAttr", json!({"args": [format!("{m}.minAngle"), 30.0]}));
    assert_eq!(faces(&mut s, &c), 26);
    run(&mut s, "setAttr", json!({"args": [format!("{m}.mode"), "weight"]}));
    assert_eq!(faces(&mut s, &c), 6);
}

#[test]
fn edit_edges_stores_overrides_that_survive_upstream_changes() {
    let mut s = Session::new();
    let c = cube(&mut s);
    let (a0, a1) = (DVec3::new(-50.0, 50.0, 50.0), DVec3::new(50.0, 50.0, 50.0));
    let (b0, b1) = (DVec3::new(-50.0, -50.0, -50.0), DVec3::new(50.0, -50.0, -50.0));
    let one = first(run(&mut s, "autoBevel", json!({"args": [c], "mode": "manual", "segments": 1, "offset": 5.0})));
    let two = first(run(&mut s, "autoBevel", json!({"args": [c], "mode": "manual", "segments": 2, "offset": 5.0})));
    pick_edge(&mut s, &c, &one, a0, a1);
    pick_edge(&mut s, &c, &two, b0, b1);
    assert!(same_edge(picked_edge(&mut s, &one), a0, a1));
    assert!(same_edge(picked_edge(&mut s, &two), b0, b1));

    // Re-bevelling upstream renumbers the second modifier's input, but it keeps its edge.
    for segments in [3, 1, 4] {
        run(&mut s, "setAttr", json!({"args": [format!("{one}.segments"), segments]}));
        assert!(same_edge(picked_edge(&mut s, &two), b0, b1));
    }
    run(&mut s, "setAttr", json!({"args": [format!("{one}.offset"), 12.0]}));
    assert!(same_edge(picked_edge(&mut s, &two), b0, b1));

    // A direct edit under the stack compacts the base; the first modifier follows its edge to the new ids.
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [2]}));
    run(&mut s, "polyExtrude", json!({"thickness": 10}));
    let e = picked_edge(&mut s, &one);
    assert!(((e.0 - e.1).length() - 100.0).abs() < 1e-6);

    // Undo walks back one step at a time.
    run(&mut s, "undo", json!({}));
    assert!(same_edge(picked_edge(&mut s, &one), a0, a1));
}

#[test]
fn edit_edges_shows_the_input_cancels_undoes_and_saves() {
    let mut s = Session::new();
    let c = cube(&mut s);
    let m = first(run(&mut s, "autoBevel", json!({"args": [c], "segments": 1})));
    run(&mut s, "addModifier", json!({"args": [c], "kind": "subdivision"}));
    let full = faces(&mut s, &c);

    run(&mut s, "bevelEditEdges", json!({"args": [m]}));
    // This modifier and the subdivision after it pass through for display.
    assert_eq!(faces(&mut s, &c), 6);
    let id = s.graph.find(&c).unwrap().unwrap();
    assert_eq!(components::selected(&s.graph, id, ComponentMode::Edge).len(), 12);
    assert!(s.run("polyExtrude", json!({"thickness": 1})).is_err());

    let text = serde_json::to_string(&scene_file::serialize(&s.graph, false)).unwrap();
    run(&mut s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
    assert_eq!(run(&mut s, "bevelEdgeInfo", json!({"args": [m]}))["editing"], true);
    assert_eq!(faces(&mut s, &c), 6);

    run(&mut s, "bevelCancelEdges", json!({"args": [m]}));
    assert_eq!(faces(&mut s, &c), full);
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &c), 6);
    run(&mut s, "redo", json!({}));

    // Deselect two edges and apply: plain auto becomes auto with overrides.
    run(&mut s, "bevelEditEdges", json!({"args": [m]}));
    run(&mut s, "selectComponents", json!({"args": [c], "edges": [0, 1], "deselect": true}));
    let r = run(&mut s, "bevelApplyEdges", json!({"args": [m]}));
    assert_eq!(r["removed"], 2);
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.mode")]})), json!(1));
    assert_eq!(run(&mut s, "bevelEdgeInfo", json!({"args": [m]}))["count"], 10);
    assert_eq!(run(&mut s, "bevelEdgeInfo", json!({"args": [m]}))["editing"], false);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "bevelEdgeInfo", json!({"args": [m]}))["editing"], true);
    run(&mut s, "bevelCancelEdges", json!({"args": [m]}));
}

#[test]
fn select_fold_edges_and_measure() {
    let mut s = Session::new();
    let c = first(run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 2})));
    // The four edges along x are split in two; the loop around the middle is flat.
    assert_eq!(run(&mut s, "selectFoldEdges", json!({"args": [c]})), 16);
    let m = first(run(&mut s, "autoBevel", json!({"args": [c]})));
    run(&mut s, "selectComponents", json!({"args": [c], "edges": [0]}));
    let thin = run(&mut s, "bevelMeasureThin", json!({"args": [m]})).as_f64().unwrap();
    assert!(thin > 0.0);
    assert!((run(&mut s, "getAttr", json!({"args": [format!("{m}.thin")]})).as_f64().unwrap() - thin).abs() < 1e-9);
}
