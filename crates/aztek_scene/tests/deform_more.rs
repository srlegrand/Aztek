//! The rest of the Deform menu: lattice, softMod, deltaMush, tension, wrap, wire, and the general deformer commands.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

/// World position of a mesh's point.
fn point(s: &mut Session, comp: &str) -> [f64; 3] {
    let v = run(s, "pointPosition", json!({"args": [comp]}));
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    (0..3).all(|i| (a[i] - b[i]).abs() < 1e-6)
}

fn names(v: &Value) -> Vec<&str> {
    v.as_array().unwrap().iter().map(|x| x.as_str().unwrap()).collect()
}

fn bounds(s: &mut Session, m: &str) -> (Vec<f64>, Vec<f64>) {
    let i = run(s, "meshInfo", json!({"args": [m]}));
    let v = |k: &str| i[k].as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect::<Vec<f64>>();
    (v("worldBoundsMin"), v("worldBoundsMax"))
}

/// Saves and loads the scene in place.
fn round_trip(s: &mut Session) {
    let doc = aztek_scene::scene_file::serialize(&s.graph, false);
    let text = serde_json::to_string(&doc).unwrap();
    let doc: Value = serde_json::from_str(&text).unwrap();
    s.graph.clear();
    aztek_scene::scene_file::deserialize(&mut s.graph, &doc).unwrap();
    s.graph.evaluate();
}

// ---------------------------------------------------------------------------------------------------------------------
// Lattice

#[test]
fn lattice_makes_the_reference_nodes_and_changes_nothing_until_moved() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    let before = point(&mut s, "pCube1.vtx[0]");
    let r = run(&mut s, "lattice", json!({"args": ["pCube1"], "divisions": [2, 5, 2], "objectCentered": true}));
    assert_eq!(r, json!(["ffd1", "ffd1Lattice", "ffd1Base"]));
    for (node, ty) in [("ffd1", "ffd"), ("ffd1LatticeShape", "lattice"), ("ffd1BaseShape", "baseLattice"), ("ffd1Lattice", "transform")] {
        assert_eq!(run(&mut s, "nodeType", json!({"args": [node]})), json!(ty), "{node}");
    }
    assert!(near(point(&mut s, "pCube1.vtx[0]"), before));
    let h = run(&mut s, "listHistory", json!({"args": ["pCube1"]}));
    assert!(names(&h).contains(&"ffd1"), "{h}");
    assert_eq!(run(&mut s, "listHistory", json!({"args": ["pCube1"], "type": "ffd"})), json!(["ffd1"]));
    assert_eq!(run(&mut s, "listHistory", json!({"args": ["pCube1"], "type": "geometryFilter"})), json!(["ffd1"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "geometryFilter"})), json!(["ffd1"]));
    for (attr, value) in [("localInfluenceS", 2.0), ("lit", 2.0), ("liu", 2.0), ("outsideLattice", 0.0), ("envelope", 1.0)] {
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("ffd1.{attr}")]})).as_f64().unwrap(), value, "{attr}");
    }
    assert_eq!(run(&mut s, "lattice", json!({"args": ["ffd1"], "query": true, "queried": ["divisions"]})), json!([2, 5, 2]));
}

#[test]
fn lattice_points_are_components() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "divisions": [2, 3, 2]}));
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[1][2][0]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["ffd1Lattice.pt[1][2][0]"]));
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[0:1][*][*]"]}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["ffd1Lattice.pt[0:1][0:2][0:1]"]));
    let flat = run(&mut s, "ls", json!({"selection": true, "flatten": true}));
    assert_eq!(flat.as_array().unwrap().len(), 12);
    assert_eq!(flat[0], json!("ffd1Lattice.pt[0][0][0]"));
    assert_eq!(flat[1], json!("ffd1Lattice.pt[0][0][1]"));
    assert_eq!(flat[11], json!("ffd1Lattice.pt[1][2][1]"));
    let by_name = run(&mut s, "ls", json!({"args": ["ffd1Lattice.pt[1][*][0]"], "flatten": true}));
    assert_eq!(names(&by_name), ["ffd1Lattice.pt[1][0][0]", "ffd1Lattice.pt[1][1][0]", "ffd1Lattice.pt[1][2][0]"]);
    // Corner of the unit lattice, in world space.
    assert!(near(point(&mut s, "ffd1Lattice.pt[1][2][1]"), [0.5, 0.5, 0.5]));
    assert!(s.run("select", json!({"args": ["ffd1Lattice.pt[9][0][0]"]})).is_err());
}

#[test]
fn moving_lattice_points_moves_the_mesh_with_undo() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    // The top four corners of a 2 2 2 lattice carry the cube's top four points; the bottom stays.
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[*][1][*]"]}));
    run(&mut s, "moveComponents", json!({"by": [0, 2, 0]}));
    let (lo, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 2.5).abs() < 1e-6 && (lo[1] + 0.5).abs() < 1e-6, "{lo:?} {hi:?}");
    assert!(s.undo());
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 0.5).abs() < 1e-6, "{hi:?}");
    assert!(s.redo());
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 2.5).abs() < 1e-6, "{hi:?}");
    run(&mut s, "setAttr", json!({"args": ["ffd1.envelope", 0.0]}));
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 0.5).abs() < 1e-6, "{hi:?}");
    run(&mut s, "setAttr", json!({"args": ["ffd1.envelope", 0.5]}));
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 1.5).abs() < 1e-6, "{hi:?}");
    run(&mut s, "setAttr", json!({"args": ["ffd1.envelope", 1.0]}));
    run(&mut s, "lattice", json!({"args": ["ffd1"], "edit": true, "resetLattice": true}));
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 0.5).abs() < 1e-6, "{hi:?}");
}

#[test]
fn undoing_a_lattice_restores_the_chain() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    assert!(s.undo());
    assert!(!run(&mut s, "objExists", json!({"args": ["ffd1"]})).as_bool().unwrap());
    assert!(!run(&mut s, "objExists", json!({"args": ["ffd1Lattice"]})).as_bool().unwrap());
    let h = run(&mut s, "listHistory", json!({"args": ["pCube1"]}));
    assert_eq!(names(&h), ["pCubeShape1", "polyCube1"]);
    assert!(s.redo());
    assert!(names(&run(&mut s, "listHistory", json!({"args": ["pCube1"]}))).contains(&"ffd1"));
}

#[test]
fn lattice_transform_against_its_base_deforms() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "name": "boxy"}));
    assert!(run(&mut s, "objExists", json!({"args": ["boxyLattice"]})).as_bool().unwrap());
    run(&mut s, "setAttr", json!({"args": ["boxyLattice.tx", 3.0]}));
    let (lo, hi) = bounds(&mut s, "pCube1");
    assert!((lo[0] - 2.5).abs() < 1e-6 && (hi[0] - 3.5).abs() < 1e-6, "{lo:?} {hi:?}");
    // Moving the base the same way puts it back.
    run(&mut s, "setAttr", json!({"args": ["boxyBase.tx", 3.0]}));
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[0] + 0.5).abs() < 1e-6, "{lo:?}");
}

#[test]
fn outside_lattice_modes() {
    let mut s = Session::new();
    // A plane 4 across with points every 1, in a lattice 2 across: the middle is inside, the rim outside.
    run(&mut s, "polyPlane", json!({"width": 4, "depth": 4, "subdivisionsX": 4, "subdivisionsZ": 4}));
    run(&mut s, "lattice", json!({"args": ["pPlane1"], "objectCentered": false, "scale": [2, 2, 2]}));
    run(&mut s, "setAttr", json!({"args": ["ffd1Lattice.ty", 1.0]}));
    // A point inside moves with the lattice; a point well outside doesn't.
    let inside = "pPlane1.vtx[12]";
    let outside = "pPlane1.vtx[0]";
    assert!(near(point(&mut s, inside), [0.0, 1.0, 0.0]));
    assert!(point(&mut s, outside)[1].abs() < 1e-6);
    run(&mut s, "lattice", json!({"args": ["ffd1"], "edit": true, "outsideLattice": 1}));
    assert!((point(&mut s, outside)[1] - 1.0).abs() < 1e-6);
    run(&mut s, "lattice", json!({"args": ["ffd1"], "edit": true, "outsideLattice": 2, "outsideFalloffDist": 0.5}));
    // vtx 10 sits 1 cm past the box, half a lattice width: on the edge of a falloff of 0.5, half way along one of 1.
    assert!(point(&mut s, outside)[1].abs() < 1e-6);
    let past = "pPlane1.vtx[10]";
    assert!(point(&mut s, past)[1].abs() < 1e-6, "{:?}", point(&mut s, past));
    run(&mut s, "lattice", json!({"args": ["ffd1"], "edit": true, "outsideFalloffDist": 1.0}));
    let y = point(&mut s, past)[1];
    assert!((y - 0.5).abs() < 1e-6, "{y}");
}

#[test]
fn lattice_local_influence_keeps_straight_lattices_straight_and_reaches_further() {
    let mut s = Session::new();
    run(&mut s, "polyCylinder", json!({"height": 4, "subdivisionsHeight": 8}));
    let (lo0, hi0) = bounds(&mut s, "pCylinder1");
    run(&mut s, "lattice", json!({"args": ["pCylinder1"], "divisions": [2, 5, 2], "ldivisions": [2, 4, 2]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["ffd1.lit"]})).as_i64().unwrap(), 4);
    let (lo, hi) = bounds(&mut s, "pCylinder1");
    assert!((hi[1] - hi0[1]).abs() < 1e-6 && (lo[0] - lo0[0]).abs() < 1e-6, "{lo:?} {hi:?}");
    // Full local influence: the top control points pull everything some way along.
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[*][4][*]"]}));
    run(&mut s, "moveComponents", json!({"by": [1, 0, 0]}));
    let mid = "pCylinder1.vtx[80]";
    let moved_wide = point(&mut s, mid)[0];
    run(&mut s, "setAttr", json!({"args": ["ffd1.lit", 2]}));
    let moved_narrow = point(&mut s, mid)[0];
    assert!(moved_wide > moved_narrow, "{moved_wide} {moved_narrow}");
}

#[test]
fn lattice_divisions_change_keeps_the_shape() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[*][1][*]"]}));
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    let (_, hi) = bounds(&mut s, "pCube1");
    run(&mut s, "setAttr", json!({"args": ["ffd1LatticeShape.tDivisions", 4]}));
    let (_, hi2) = bounds(&mut s, "pCube1");
    assert!((hi[1] - hi2[1]).abs() < 1e-6, "{hi:?} {hi2:?}");
    run(&mut s, "lattice", json!({"args": ["ffd1"], "edit": true, "divisions": [3, 3, 3]}));
    assert_eq!(run(&mut s, "lattice", json!({"args": ["ffd1Lattice"], "query": true, "queried": ["divisions"]})), json!([3, 3, 3]));
    let flat = run(&mut s, "ls", json!({"args": ["ffd1Lattice.pt[*][*][*]"], "flatten": true}));
    assert_eq!(flat.as_array().unwrap().len(), 27);
}

#[test]
fn lattice_freeze_mapping_and_remap() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "freezeMapping": true}));
    assert_eq!(run(&mut s, "lattice", json!({"args": ["ffd1"], "query": true, "queried": ["freezeMapping"]})), json!(true));
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[*][1][*]"]}));
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 1.5).abs() < 1e-6, "{hi:?}");
    // The cluster carries the points well clear of the lattice; frozen, they keep the top's offset.
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.tx", 5.0]}));
    let (lo, hi) = bounds(&mut s, "pCube1");
    assert!((lo[0] - 4.5).abs() < 1e-6 && (hi[1] - 1.5).abs() < 1e-6, "{lo:?} {hi:?}");
    // Mapped again out there, every point is outside the lattice and only the cluster moves it.
    run(&mut s, "lattice", json!({"args": ["ffd1"], "edit": true, "remap": true}));
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 0.5).abs() < 1e-6, "{hi:?}");
}

#[test]
fn lattice_survives_save_and_load() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "divisions": [2, 3, 2]}));
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[*][2][*]"]}));
    run(&mut s, "moveComponents", json!({"by": [0, 2, 0]}));
    let (_, hi) = bounds(&mut s, "pCube1");
    round_trip(&mut s);
    let (_, hi2) = bounds(&mut s, "pCube1");
    assert!((hi[1] - hi2[1]).abs() < 1e-6 && (hi2[1] - 2.5).abs() < 1e-6, "{hi:?} {hi2:?}");
    assert!(near(point(&mut s, "ffd1Lattice.pt[0][2][0]"), [-0.5, 2.5, -0.5]));
    run(&mut s, "setAttr", json!({"args": ["ffd1.envelope", 0.0]}));
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[1] - 0.5).abs() < 1e-6);
}

#[test]
fn deleting_the_lattice_or_the_ffd_takes_the_lot_and_restores_the_mesh() {
    for doomed in ["ffd1", "ffd1Lattice", "ffd1Base"] {
        let mut s = Session::new();
        run(&mut s, "polyCube", json!({}));
        run(&mut s, "lattice", json!({"args": ["pCube1"]}));
        run(&mut s, "setAttr", json!({"args": ["ffd1Lattice.tx", 3.0]}));
        run(&mut s, "delete", json!({"args": [doomed]}));
        for gone in ["ffd1", "ffd1Lattice", "ffd1Base"] {
            assert!(!run(&mut s, "objExists", json!({"args": [gone]})).as_bool().unwrap(), "{doomed}: {gone}");
        }
        let (lo, _) = bounds(&mut s, "pCube1");
        assert!((lo[0] + 0.5).abs() < 1e-6, "{doomed}: {lo:?}");
        assert_eq!(names(&run(&mut s, "listHistory", json!({"args": ["pCube1"]}))), ["pCubeShape1", "polyCube1"]);
        assert!(s.undo());
        let (lo, _) = bounds(&mut s, "pCube1");
        assert!((lo[0] - 2.5).abs() < 1e-6, "{doomed} undo: {lo:?}");
    }
}

#[test]
fn delete_history_bakes_the_lattice_and_removes_it() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    run(&mut s, "setAttr", json!({"args": ["ffd1Lattice.tx", 3.0]}));
    run(&mut s, "delete", json!({"args": ["pCube1"], "constructionHistory": true}));
    assert!(!run(&mut s, "objExists", json!({"args": ["ffd1Lattice"]})).as_bool().unwrap());
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[0] - 2.5).abs() < 1e-6, "{lo:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// The general commands

#[test]
fn percent_sets_and_queries_weights() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    let (rest0, rest7) = (point(&mut s, "pCube1.vtx[0]"), point(&mut s, "pCube1.vtx[7]"));
    run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[0:3]"], "value": 0.5}));
    assert_eq!(run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[0]", "pCube1.vtx[7]"], "query": true, "queried": ["value"]})), json!([0.5, 1.0]));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.ty", 2.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), [rest0[0], rest0[1] + 1.0, rest0[2]]));
    assert!(near(point(&mut s, "pCube1.vtx[7]"), [rest7[0], rest7[1] + 2.0, rest7[2]]));
    run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[0]"], "value": 0.5, "multiply": true}));
    assert_eq!(run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[0]"], "query": true, "queried": ["value"]})), json!([0.25]));
    assert!(s.undo());
    assert_eq!(run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[0]"], "query": true, "queried": ["value"]})), json!([0.5]));
    round_trip(&mut s);
    assert_eq!(run(&mut s, "percent", json!({"args": ["cluster1", "pCube1.vtx[0]"], "query": true, "queried": ["value"]})), json!([0.5]));
}

#[test]
fn percent_weighs_a_lattice() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    run(&mut s, "percent", json!({"args": ["ffd1", "pCube1.vtx[0]"], "value": 0.0}));
    let v0 = point(&mut s, "pCube1.vtx[0]");
    run(&mut s, "setAttr", json!({"args": ["ffd1Lattice.tx", 3.0]}));
    assert!(near(point(&mut s, "pCube1.vtx[0]"), v0));
    let v1 = point(&mut s, "pCube1.vtx[1]");
    assert!((v1[0] - 3.5).abs() < 1e-6, "{v1:?}");
}

#[test]
fn deformer_command_makes_queries_adds_and_removes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polySphere", json!({"radius": 0.4}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["pCube1"], "type": "ffd", "name": "box"})), json!(["box"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["box"], "query": true, "geometry": true})), json!(["pCubeShape1"]));
    run(&mut s, "deformer", json!({"args": ["box"], "edit": true, "geometry": "pSphere1"}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["box"], "query": true, "geometry": true})), json!(["pCubeShape1", "pSphereShape1"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["box"], "query": true, "geometryIndices": true})), json!([0, 1]));
    // Both meshes answer to the one lattice and the one envelope.
    run(&mut s, "setAttr", json!({"args": ["boxLattice.tx", 3.0]}));
    let moved = bounds(&mut s, "pSphere1");
    run(&mut s, "setAttr", json!({"args": ["box.envelope", 0.0]}));
    let rest = bounds(&mut s, "pSphere1");
    assert!((moved.0[0] - rest.0[0] - 3.0).abs() < 1e-6, "{moved:?} {rest:?}");
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[0] + 0.5).abs() < 1e-6, "{lo:?}");
    run(&mut s, "setAttr", json!({"args": ["box.envelope", 1.0]}));
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[0] - 2.5).abs() < 1e-6, "{lo:?}");
    run(&mut s, "deformer", json!({"args": ["box"], "edit": true, "remove": true, "geometry": "pCube1"}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["box"], "query": true, "geometry": true})), json!(["pSphereShape1"]));
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[0] + 0.5).abs() < 1e-6, "{lo:?}");
    assert!(s.undo());
    let (lo, _) = bounds(&mut s, "pCube1");
    assert!((lo[0] - 2.5).abs() < 1e-6, "{lo:?}");
    assert!(s.run("deformer", json!({"args": ["pCube1"], "type": "noSuchThing"})).is_err());
}

#[test]
fn lattice_covers_several_meshes_as_one() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "setAttr", json!({"args": ["pCube2.tx", 2.0]}));
    let r = run(&mut s, "lattice", json!({"args": ["pCube1", "pCube2"]}));
    assert_eq!(r, json!(["ffd1", "ffd1Lattice", "ffd1Base"]));
    assert_eq!(run(&mut s, "lattice", json!({"args": ["ffd1"], "query": true, "geometry": true})), json!(["pCubeShape1", "pCubeShape2"]));
    run(&mut s, "setAttr", json!({"args": ["ffd1Lattice.ty", 1.0]}));
    assert!((bounds(&mut s, "pCube1").1[1] - 1.5).abs() < 1e-6);
    assert!((bounds(&mut s, "pCube2").1[1] - 1.5).abs() < 1e-6);
}

#[test]
fn before_and_front_of_chain_place_the_deformer() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"], "name": "first"}));
    run(&mut s, "cluster", json!({"args": ["pCube1"], "name": "second"}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "before": true, "name": "mid"}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "frontOfChain": true, "name": "top"}));
    run(&mut s, "lattice", json!({"args": ["pCube1"], "after": true, "name": "end"}));
    let h = run(&mut s, "listHistory", json!({"args": ["pCube1"], "type": "geometryFilter"}));
    assert_eq!(names(&h), ["end", "second", "mid", "first", "top"]);
}

#[test]
fn reorder_swaps_and_the_chain_still_evaluates() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    run(&mut s, "lattice", json!({"args": ["pCube1"]}));
    run(&mut s, "cluster", json!({"args": ["pCube1"], "name": "last"}));
    let order = |s: &mut Session| run(s, "listHistory", json!({"args": ["pCube1"], "type": "geometryFilter"}));
    assert_eq!(names(&order(&mut s)), ["last", "ffd1", "cluster1"]);
    // The lattice stretches X by moving a side; the cluster then carries the points. Order changes the answer.
    run(&mut s, "select", json!({"args": ["ffd1Lattice.pt[1][*][*]"]}));
    run(&mut s, "moveComponents", json!({"by": [1, 0, 0]}));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.tx", 5.0]}));
    let (lo, hi) = bounds(&mut s, "pCube1");
    // Cluster first: the points leave the lattice, which only holds inside points.
    assert!((lo[0] - 4.5).abs() < 1e-6 && (hi[0] - 5.5).abs() < 1e-6, "{lo:?} {hi:?}");
    run(&mut s, "reorderDeformers", json!({"args": ["cluster1", "ffd1", "pCube1"]}));
    assert_eq!(names(&order(&mut s)), ["last", "cluster1", "ffd1"]);
    let (lo, hi) = bounds(&mut s, "pCube1");
    // Lattice first: stretched, then carried.
    assert!((lo[0] - 4.5).abs() < 1e-6 && (hi[0] - 6.5).abs() < 1e-6, "{lo:?} {hi:?}");
    // Across one between them, keeping it in place.
    run(&mut s, "reorderDeformers", json!({"args": ["last", "ffd1"]}));
    assert_eq!(names(&order(&mut s)), ["ffd1", "cluster1", "last"]);
    assert!(s.undo());
    assert_eq!(names(&order(&mut s)), ["last", "cluster1", "ffd1"]);
    round_trip(&mut s);
    assert_eq!(names(&order(&mut s)), ["last", "cluster1", "ffd1"]);
    let (_, hi) = bounds(&mut s, "pCube1");
    assert!((hi[0] - 6.5).abs() < 1e-6, "{hi:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// softMod

/// A plane 10 across with points every 1, flat at y = 0. Its middle point is vtx[60].
fn plane(s: &mut Session) {
    run(s, "polyPlane", json!({"width": 10, "depth": 10, "subdivisionsX": 10, "subdivisionsZ": 10}));
}

#[test]
fn soft_mod_moves_points_by_falloff_from_the_selection() {
    let mut s = Session::new();
    plane(&mut s);
    assert!(near(point(&mut s, "pPlane1.vtx[60]"), [0.0, 0.0, 0.0]));
    let r = run(&mut s, "softMod", json!({"args": ["pPlane1.vtx[60]"], "falloffRadius": 2.0}));
    assert_eq!(r, json!(["softMod1", "softMod1Handle"]));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["softMod1Handle"]})), json!("transform"));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["softMod1HandleShape"]})), json!("softModHandle"));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["softMod1.falloffRadius"]})), json!(2.0));
    // The handle starts at zero, its pivot on the point.
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["softMod1Handle.ty"]})), json!(0.0));
    assert!(near(point(&mut s, "pPlane1.vtx[60]"), [0.0, 0.0, 0.0]));
    run(&mut s, "setAttr", json!({"args": ["softMod1Handle.ty", 1.0]}));
    let y = |s: &mut Session, v: &str| point(s, v)[1];
    assert!((y(&mut s, "pPlane1.vtx[60]") - 1.0).abs() < 1e-6);
    // One point along is half way out: the smooth curve is at a half there.
    assert!((y(&mut s, "pPlane1.vtx[61]") - 0.5).abs() < 1e-6, "{}", y(&mut s, "pPlane1.vtx[61]"));
    assert!(y(&mut s, "pPlane1.vtx[62]").abs() < 1e-6);
    assert!(y(&mut s, "pPlane1.vtx[0]").abs() < 1e-6);
    run(&mut s, "setAttr", json!({"args": ["softMod1.falloffRadius", 4.0]}));
    let t: f64 = 0.25;
    let smooth = 1.0 - t * t * (3.0 - 2.0 * t);
    assert!((y(&mut s, "pPlane1.vtx[61]") - smooth).abs() < 1e-6);
    run(&mut s, "setAttr", json!({"args": ["softMod1.envelope", 0.0]}));
    assert!(y(&mut s, "pPlane1.vtx[60]").abs() < 1e-9);
    run(&mut s, "setAttr", json!({"args": ["softMod1.envelope", 1.0]}));
    // The curve is the reference's ramp: a linear one reads straight.
    run(&mut s, "setAttr", json!({"args": ["softMod1.falloffCurve[0].falloffCurve_Interp", 1]}));
    assert!((y(&mut s, "pPlane1.vtx[61]") - 0.75).abs() < 1e-6);
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": ["pPlane1"], "type": "softMod"}))), ["softMod1"]);
    round_trip(&mut s);
    assert!((y(&mut s, "pPlane1.vtx[61]") - 0.75).abs() < 1e-6);
    assert!((y(&mut s, "pPlane1.vtx[60]") - 1.0).abs() < 1e-6);
}

#[test]
fn soft_mod_from_the_object_centre_or_a_given_one() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "softMod", json!({"args": ["pPlane1"], "falloffRadius": 2.0}));
    run(&mut s, "setAttr", json!({"args": ["softMod1Handle.ty", 1.0]}));
    assert!((point(&mut s, "pPlane1.vtx[60]")[1] - 1.0).abs() < 1e-6);
    assert!(point(&mut s, "pPlane1.vtx[0]")[1].abs() < 1e-6);
    assert!(s.undo());
    assert!(s.undo());
    let mut t = Session::new();
    plane(&mut t);
    run(&mut t, "softMod", json!({"args": ["pPlane1"], "falloffRadius": 2.0, "falloffCenter": [3, 0, 0], "name": "dent"}));
    run(&mut t, "setAttr", json!({"args": ["dentHandle.ty", 1.0]}));
    assert!((point(&mut t, "pPlane1.vtx[63]")[1] - 1.0).abs() < 1e-6);
    assert!(point(&mut t, "pPlane1.vtx[60]")[1].abs() < 1e-6);
    assert_eq!(run(&mut t, "softMod", json!({"args": ["dent"], "query": true, "queried": ["falloffRadius"]})), json!(2.0));
    run(&mut t, "softMod", json!({"args": ["dentHandle"], "edit": true, "falloffRadius": 1.0}));
    assert_eq!(run(&mut t, "getAttr", json!({"args": ["dent.fr"]})), json!(1.0));
}

#[test]
fn soft_mod_surface_falloff_runs_along_the_mesh() {
    let mut s = Session::new();
    // A cube's top and bottom faces are 1 apart through space, but further along the surface.
    run(&mut s, "polyCube", json!({"subdivisionsX": 4, "subdivisionsY": 4, "subdivisionsZ": 4}));
    let (top, bottom) = {
        let n = run(&mut s, "polyEvaluate", json!({"args": ["pCube1"], "vertex": true})).as_i64().unwrap();
        let (mut t, mut b) = (0, 0);
        for i in 0..n {
            let p = point(&mut s, &format!("pCube1.vtx[{i}]"));
            if near(p, [0.0, 0.5, 0.0]) {
                t = i;
            }
            if near(p, [0.0, -0.5, 0.0]) {
                b = i;
            }
        }
        (t, b)
    };
    run(&mut s, "softMod", json!({"args": [format!("pCube1.vtx[{top}]")], "falloffRadius": 1.2}));
    run(&mut s, "setAttr", json!({"args": ["softMod1Handle.ty", 1.0]}));
    let bottom = format!("pCube1.vtx[{bottom}]");
    let through = point(&mut s, &bottom)[1] + 0.5;
    assert!(through > 0.0, "volume reaches the far face");
    run(&mut s, "setAttr", json!({"args": ["softMod1.falloffMode", 1]}));
    assert!((point(&mut s, &bottom)[1] + 0.5).abs() < 1e-6, "surface doesn't");
}

// ---------------------------------------------------------------------------------------------------------------------
// deltaMush and tension

#[test]
fn delta_mush_keeps_the_rest_pose_and_rigid_moves() {
    let mut s = Session::new();
    run(&mut s, "polySphere", json!({}));
    let v = "pSphere1.vtx[50]";
    let rest = point(&mut s, v);
    run(&mut s, "cluster", json!({"args": ["pSphere1"]}));
    assert_eq!(run(&mut s, "deltaMush", json!({"args": ["pSphere1"]})), json!(["deltaMush1"]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["deltaMush1.si"]})), json!(10));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["deltaMush1.ss"]})), json!(0.5));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["deltaMush1.en"]})), json!(1.0));
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": ["pSphere1"], "type": "geometryFilter"}))), ["deltaMush1", "cluster1"]);
    assert!(near(point(&mut s, v), rest), "at rest nothing moves");
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.tx", 2.0]}));
    assert!(near(point(&mut s, v), [rest[0] + 2.0, rest[1], rest[2]]));
    // A turn of the whole mesh turns the detail too.
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.tx", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.rz", 90.0]}));
    let turned = point(&mut s, v);
    assert!(near(turned, [-rest[1], rest[0], rest[2]]), "{turned:?} {rest:?}");
}

#[test]
fn delta_mush_smooths_a_spike_and_survives_save() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "cluster", json!({"args": ["pPlane1.vtx[60]"]}));
    run(&mut s, "deltaMush", json!({"args": ["pPlane1"], "smoothingIterations": 5, "smoothingStep": 0.5, "pinBorderVertices": false, "name": "mush"}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["mush.si"]})), json!(5));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["mush.pbv"]})), json!(false));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.ty", 2.0]}));
    let y = point(&mut s, "pPlane1.vtx[60]")[1];
    assert!(y > 0.0 && y < 1.0, "the spike is smoothed away: {y}");
    let ring = point(&mut s, "pPlane1.vtx[61]")[1];
    assert!(ring > 0.0, "and shared with its neighbours: {ring}");
    round_trip(&mut s);
    assert!((point(&mut s, "pPlane1.vtx[60]")[1] - y).abs() < 1e-9);
    run(&mut s, "setAttr", json!({"args": ["mush.envelope", 0.0]}));
    assert!((point(&mut s, "pPlane1.vtx[60]")[1] - 2.0).abs() < 1e-9, "envelope 0 passes the input on");
    run(&mut s, "setAttr", json!({"args": ["mush.envelope", 1.0]}));
    // No iterations: nothing to smooth, nothing changes.
    run(&mut s, "setAttr", json!({"args": ["mush.si", 0]}));
    assert!((point(&mut s, "pPlane1.vtx[60]")[1] - 2.0).abs() < 1e-9);
    assert!(s.undo());
    assert!((point(&mut s, "pPlane1.vtx[60]")[1] - y).abs() < 1e-9);
}

#[test]
fn delta_mush_constraints_hold_points_back() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "cluster", json!({"args": ["pPlane1.vtx[60]"]}));
    run(&mut s, "deltaMush", json!({"args": ["pPlane1"], "smoothingIterations": 5, "pinBorderVertices": false}));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.ty", 2.0]}));
    let free = point(&mut s, "pPlane1.vtx[61]")[1];
    assert!(free > 0.0);
    // The neighbour wants to rise along its normal.
    run(&mut s, "setAttr", json!({"args": ["deltaMush1.outwardConstraint", 1.0]}));
    let held = point(&mut s, "pPlane1.vtx[61]")[1];
    assert!(held < free, "{held} {free}");
}

#[test]
fn tension_pulls_edges_back_to_rest_length() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"subdivisionsY": 4, "height": 4}));
    // Pull the top 2 away, leaving the rest: the stretched edges at the join want their lengths back.
    let top: Vec<String> = (0..20).filter_map(|i| {
        let c = format!("pCube1.vtx[{i}]");
        (point(&mut s, &c)[1] > 1.9).then_some(c)
    }).collect();
    assert_eq!(top.len(), 4);
    run(&mut s, "cluster", json!({"args": top}));
    assert_eq!(run(&mut s, "tension", json!({"args": ["pCube1"], "pinBorderVertices": false})), json!(["tension1"]));
    let (_, hi0) = bounds(&mut s, "pCube1");
    assert!((hi0[1] - 2.0).abs() < 1e-6, "at rest nothing moves: {hi0:?}");
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["tension1.smoothingIterations"]})), json!(10));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.ty", 2.0]}));
    let (_, relaxed) = bounds(&mut s, "pCube1");
    assert!(relaxed[1] < 4.0 - 0.1, "{relaxed:?}");
    // A full stretch constraint leaves stretched edges alone.
    run(&mut s, "setAttr", json!({"args": ["tension1.stretchConstraint", 1.0]}));
    let (_, held) = bounds(&mut s, "pCube1");
    assert!((held[1] - 4.0).abs() < 1e-6, "{held:?}");
    run(&mut s, "setAttr", json!({"args": ["tension1.stretchConstraint", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["tension1.envelope", 0.0]}));
    let (_, off) = bounds(&mut s, "pCube1");
    assert!((off[1] - 4.0).abs() < 1e-6, "{off:?}");
    run(&mut s, "setAttr", json!({"args": ["tension1.envelope", 1.0]}));
    round_trip(&mut s);
    let (_, back) = bounds(&mut s, "pCube1");
    assert!((back[1] - relaxed[1]).abs() < 1e-6);
}

// ---------------------------------------------------------------------------------------------------------------------
// wrap

#[test]
fn wrap_follows_its_wrapper() {
    let mut s = Session::new();
    run(&mut s, "polySphere", json!({"radius": 1.0, "name": "inner"}));
    run(&mut s, "polyCube", json!({"width": 3, "height": 3, "depth": 3, "subdivisionsX": 3, "subdivisionsY": 3, "subdivisionsZ": 3, "name": "cage"}));
    let v = "inner.vtx[50]";
    let rest = point(&mut s, v);
    assert_eq!(run(&mut s, "wrap", json!({"args": ["inner"], "influence": "cage"})), json!(["wrap1"]));
    assert_eq!(run(&mut s, "wrap", json!({"args": ["wrap1"], "query": true, "queried": ["influence"]})), json!(["cageShape"]));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["wrap1.maxDistance"]})), json!(1.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["wrap1.exclusiveBind"]})), json!(false));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["wrap1.autoWeightThreshold"]})), json!(true));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["wrap1.falloffMode"]})), json!(0));
    assert!(near(point(&mut s, v), rest), "binding moves nothing");
    // Moving the wrapper's transform carries the mesh.
    run(&mut s, "setAttr", json!({"args": ["cage.ty", 2.0]}));
    assert!(near(point(&mut s, v), [rest[0], rest[1] + 2.0, rest[2]]));
    run(&mut s, "setAttr", json!({"args": ["wrap1.envelope", 0.0]}));
    assert!(near(point(&mut s, v), rest));
    run(&mut s, "setAttr", json!({"args": ["wrap1.envelope", 1.0]}));
    run(&mut s, "setAttr", json!({"args": ["cage.ty", 0.0]}));
    // Deforming the wrapper's points moves what is near them.
    let (lo0, hi0) = bounds(&mut s, "inner");
    let top: Vec<String> = (0..56).filter_map(|i| {
        let c = format!("cage.vtx[{i}]");
        (point(&mut s, &c)[1] > 1.4).then_some(c)
    }).collect();
    run(&mut s, "select", json!({"args": top}));
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    let (lo, hi) = bounds(&mut s, "inner");
    assert!(hi[1] > hi0[1] + 0.3, "{hi:?} {hi0:?}");
    assert!(lo[1] < hi[1] && lo[1] < lo0[1] + 0.5, "{lo:?} {lo0:?}");
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": ["inner"], "type": "wrap"}))), ["wrap1"]);
    let before = point(&mut s, v);
    round_trip(&mut s);
    assert!(near(point(&mut s, v), before), "saved and loaded");
    assert!(s.run("wrap", json!({"args": ["wrap1"], "edit": true, "influence": "noSuch"})).is_err());
}

#[test]
fn wrap_binds_from_the_scripted_form_and_undoes() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "inner"}));
    run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2, "name": "cage"}));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["inner"], "type": "wrap"})), json!(["wrap1"]));
    // No influence yet: nothing happens.
    let (lo0, _) = bounds(&mut s, "inner");
    run(&mut s, "setAttr", json!({"args": ["cage.tx", 1.0]}));
    assert!((bounds(&mut s, "inner").0[0] - lo0[0]).abs() < 1e-9);
    run(&mut s, "setAttr", json!({"args": ["cage.tx", 0.0]}));
    run(&mut s, "wrap", json!({"args": ["wrap1"], "edit": true, "influence": "cage", "exclusiveBind": true}));
    run(&mut s, "setAttr", json!({"args": ["cage.tx", 1.0]}));
    assert!((bounds(&mut s, "inner").0[0] - lo0[0] - 1.0).abs() < 1e-6);
    assert!(s.undo());
    assert!((bounds(&mut s, "inner").0[0] - lo0[0]).abs() < 1e-6);
    // Rebinding where the wrapper now stands takes that as the rest.
    run(&mut s, "setAttr", json!({"args": ["cage.tx", 5.0]}));
    run(&mut s, "wrap", json!({"args": ["wrap1"], "edit": true, "rebind": true}));
    assert!((bounds(&mut s, "inner").0[0] - lo0[0]).abs() < 1e-6, "rest again");
    run(&mut s, "wrap", json!({"args": ["wrap1"], "edit": true, "removeInfluence": "cage"}));
    run(&mut s, "setAttr", json!({"args": ["cage.tx", 9.0]}));
    assert!((bounds(&mut s, "inner").0[0] - lo0[0]).abs() < 1e-6, "no influence left");
}

#[test]
fn wrap_takes_the_selection_with_the_wrapper_last() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "inner"}));
    run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2, "name": "cage"}));
    run(&mut s, "select", json!({"args": ["inner", "cage"]}));
    assert_eq!(run(&mut s, "wrap", json!({})), json!(["wrap1"]));
    assert_eq!(run(&mut s, "wrap", json!({"args": ["wrap1"], "query": true, "queried": ["influence"]})), json!(["cageShape"]));
    assert_eq!(run(&mut s, "deformer", json!({"args": ["wrap1"], "query": true, "geometry": true})), json!(["innerShape"]));
}

#[test]
fn percent_weighs_soft_mod_mush_and_wrap() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "softMod", json!({"args": ["pPlane1.vtx[60]"], "falloffRadius": 2.0}));
    run(&mut s, "percent", json!({"args": ["softMod1", "pPlane1.vtx[60]"], "value": 0.5}));
    run(&mut s, "setAttr", json!({"args": ["softMod1Handle.ty", 1.0]}));
    assert!((point(&mut s, "pPlane1.vtx[60]")[1] - 0.5).abs() < 1e-6);
    assert_eq!(run(&mut s, "percent", json!({"args": ["softMod1", "pPlane1.vtx[60]"], "query": true, "queried": ["value"]})), json!([0.5]));
}

// ---------------------------------------------------------------------------------------------------------------------
// wire

/// A plane 8 across with points every 1, and a stand-in for a curve: a sliver of a plane along X at z = 0. Its points run along
/// one long side, then jump back and run along the other, a hair apart, so it reads as one line. A wire takes anything with points.
fn wire_scene(s: &mut Session) {
    run(s, "polyPlane", json!({"width": 8, "depth": 8, "subdivisionsX": 8, "subdivisionsZ": 8, "name": "sheet"}));
    run(s, "polyPlane", json!({"width": 8, "depth": 0.0001, "subdivisionsX": 8, "subdivisionsZ": 1, "name": "line"}));
}

/// The sheet's point at x, z.
fn sheet_point(x: i32, z: i32) -> String {
    format!("sheet.vtx[{}]", (4 - z) * 9 + (x + 4))
}

#[test]
fn wire_moves_points_by_distance_from_the_curve() {
    let mut s = Session::new();
    wire_scene(&mut s);
    assert!(near(point(&mut s, &sheet_point(1, 2)), [1.0, 0.0, 2.0]));
    let r = run(&mut s, "wire", json!({"args": ["sheet"], "wire": "line", "dropoffDistance": [0, 2.0], "groupWithBase": false, "envelope": 1.0, "crossingEffect": 0.0, "localInfluence": 0.0, "name": "wire1"}));
    assert_eq!(r, json!(["wire1"]));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["wire1"]})), json!("wire"));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["wire1.dropoffDistance[0]"]})), json!(2.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["wire1.rotation"]})), json!(1.0));
    assert_eq!(run(&mut s, "wire", json!({"args": ["wire1"], "query": true, "queried": ["wire"]})), json!(["line"]));
    assert!(near(point(&mut s, &sheet_point(1, 1)), [1.0, 0.0, 1.0]), "nothing moves until the wire does");
    run(&mut s, "setAttr", json!({"args": ["line.ty", 1.0]}));
    let y = |s: &mut Session, x: i32, z: i32| point(s, &sheet_point(x, z))[1];
    assert!((y(&mut s, 0, 0) - 1.0).abs() < 1e-3, "on the wire: {}", y(&mut s, 0, 0));
    assert!((y(&mut s, 0, 1) - 0.5).abs() < 1e-3, "half way out: {}", y(&mut s, 0, 1));
    assert!(y(&mut s, 0, 3).abs() < 1e-9, "past the dropoff");
    assert_eq!(names(&run(&mut s, "listHistory", json!({"args": ["sheet"], "type": "wire"}))), ["wire1"]);
    run(&mut s, "setAttr", json!({"args": ["wire1.envelope", 0.0]}));
    assert!(y(&mut s, 0, 0).abs() < 1e-9);
    run(&mut s, "setAttr", json!({"args": ["wire1.envelope", 1.0]}));
    run(&mut s, "setAttr", json!({"args": ["wire1.dropoffDistance[0]", 4.0]}));
    assert!(y(&mut s, 0, 3) > 0.0);
    run(&mut s, "wire", json!({"args": ["wire1"], "edit": true, "dropoffDistance": [0, 2.0]}));
    assert!(y(&mut s, 0, 3).abs() < 1e-9);
    assert!(s.undo());
    assert!(y(&mut s, 0, 3) > 0.0);
}

#[test]
fn wire_follows_the_drivers_own_points_with_undo_and_save() {
    let mut s = Session::new();
    wire_scene(&mut s);
    run(&mut s, "wire", json!({"args": ["sheet"], "wire": "line", "dropoffDistance": [0, 2.0]}));
    let y = |s: &mut Session, x: i32, z: i32| point(s, &sheet_point(x, z))[1];
    // Editing the driver's points, not its transform: only the right end of the sheet comes up.
    let right: Vec<String> = (0..18).filter_map(|i| {
        let c = format!("line.vtx[{i}]");
        (point(&mut s, &c)[0] > 3.5).then_some(c)
    }).collect();
    assert_eq!(right.len(), 2);
    run(&mut s, "select", json!({"args": right}));
    run(&mut s, "moveComponents", json!({"by": [0, 2, 0]}));
    let far = y(&mut s, 4, 0);
    assert!((far - 2.0).abs() < 1e-3, "{far}");
    assert!(y(&mut s, -4, 0).abs() < 1e-9);
    assert!(s.undo());
    assert!(y(&mut s, 4, 0).abs() < 1e-9, "the wire goes back with its driver");
    assert!(s.redo());
    assert!((y(&mut s, 4, 0) - 2.0).abs() < 1e-3);
    round_trip(&mut s);
    assert!((y(&mut s, 4, 0) - 2.0).abs() < 1e-3, "saved and loaded");
    assert!(y(&mut s, -4, 0).abs() < 1e-9);
}

#[test]
fn wire_turns_points_with_it_as_rotation_says() {
    let mut s = Session::new();
    wire_scene(&mut s);
    run(&mut s, "wire", json!({"args": ["sheet"], "wire": "line", "dropoffDistance": [0, 4.0], "rotation": 0.0}));
    let at = sheet_point(2, 1);
    // A quarter turn about Y carries the wire's place at x = 2 to (0, 0, -2), and the point's offset of 1 along Z to 1 along X.
    // The point is 1 out with a reach of 4, so the falloff is smooth(0.75) = 0.84375.
    run(&mut s, "setAttr", json!({"args": ["line.ry", 90.0]}));
    let fall = 0.84375;
    let close = |a: [f64; 3], b: [f64; 3]| (0..3).all(|i| (a[i] - b[i]).abs() < 1e-3);
    // Without rotation the offset stays along Z: the place wanted is (0, 0, -1).
    let stiff = point(&mut s, &at);
    assert!(close(stiff, [2.0 - 2.0 * fall, 0.0, 1.0 - 2.0 * fall]), "{stiff:?}");
    // With it the offset turns too: the place wanted is (1, 0, -2).
    run(&mut s, "setAttr", json!({"args": ["wire1.rotation", 1.0]}));
    let turning = point(&mut s, &at);
    assert!(close(turning, [2.0 - fall, 0.0, 1.0 - 3.0 * fall]), "{turning:?}");
}

#[test]
fn wires_cross_blending_or_adding() {
    let mut s = Session::new();
    wire_scene(&mut s);
    run(&mut s, "polyPlane", json!({"width": 8, "depth": 0.0001, "subdivisionsX": 8, "subdivisionsZ": 1, "name": "other"}));
    run(&mut s, "wire", json!({"args": ["sheet"], "wire": ["line", "other"], "dropoffDistance": [0, 2.0, 1, 2.0]}));
    assert_eq!(run(&mut s, "wire", json!({"args": ["wire1"], "query": true, "queried": ["wire"]})), json!(["line", "other"]));
    assert_eq!(run(&mut s, "wire", json!({"args": ["wire1"], "query": true, "queried": ["dropoffDistance"]})), json!([2.0, 2.0]));
    run(&mut s, "setAttr", json!({"args": ["line.ty", 1.0]}));
    run(&mut s, "setAttr", json!({"args": ["other.ty", 1.0]}));
    let y = |s: &mut Session| point(s, &sheet_point(0, 0))[1];
    // Blended, two wires moving 1 move the point 1. Added, 2.
    assert!((y(&mut s) - 1.0).abs() < 1e-3, "{}", y(&mut s));
    run(&mut s, "setAttr", json!({"args": ["wire1.crossingEffect", 1.0]}));
    assert!((y(&mut s) - 2.0).abs() < 1e-3, "{}", y(&mut s));
    // With no local influence a still wire has no say; with full, it holds half.
    run(&mut s, "setAttr", json!({"args": ["wire1.crossingEffect", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["other.ty", 0.0]}));
    assert!((y(&mut s) - 1.0).abs() < 1e-3, "{}", y(&mut s));
    run(&mut s, "setAttr", json!({"args": ["wire1.localInfluence", 1.0]}));
    assert!((y(&mut s) - 0.5).abs() < 1e-3, "{}", y(&mut s));
}

#[test]
fn percent_weighs_a_wire_and_the_selection_form_works() {
    let mut s = Session::new();
    wire_scene(&mut s);
    run(&mut s, "select", json!({"args": ["sheet", "line"]}));
    assert_eq!(run(&mut s, "wire", json!({})), json!(["wire1"]));
    assert_eq!(run(&mut s, "wire", json!({"args": ["wire1"], "query": true, "queried": ["wire"]})), json!(["line"]));
    let c = sheet_point(0, 0);
    run(&mut s, "percent", json!({"args": ["wire1", c.clone()], "value": 0.5}));
    run(&mut s, "setAttr", json!({"args": ["line.ty", 1.0]}));
    assert!((point(&mut s, &c)[1] - 0.5).abs() < 1e-3);
    // A driver deleted leaves the mesh as it came.
    run(&mut s, "delete", json!({"args": ["wire1"]}));
    assert!(point(&mut s, &c)[1].abs() < 1e-9);
    assert!(run(&mut s, "objExists", json!({"args": ["line"]})).as_bool().unwrap(), "the curve is the user's: it stays");
}

#[test]
fn cluster_handle_stays_at_zero_with_its_pivot_on_the_points() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "setAttr", json!({"args": ["pCube1.ty", 5.0]}));
    run(&mut s, "cluster", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["cluster1Handle.ty"]})), json!(0.0));
    let (lo0, _) = bounds(&mut s, "pCube1");
    assert!((lo0[1] - 4.5).abs() < 1e-6, "{lo0:?}");
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.ty", 2.0]}));
    assert!((bounds(&mut s, "pCube1").0[1] - 6.5).abs() < 1e-6);
    // Scaling happens about the pivot, the points' middle.
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.ty", 0.0]}));
    run(&mut s, "setAttr", json!({"args": ["cluster1Handle.sy", 2.0]}));
    let (lo, hi) = bounds(&mut s, "pCube1");
    assert!((lo[1] - 4.0).abs() < 1e-6 && (hi[1] - 6.0).abs() < 1e-6, "{lo:?} {hi:?}");
}

#[test]
fn point_position_of_a_missing_point_is_an_error() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    assert!(s.run("pointPosition", json!({"args": ["pCube1.vtx[99]"]})).is_err());
}

#[test]
fn further_meshes_answer_to_one_deformer() {
    let mut s = Session::new();
    plane(&mut s);
    run(&mut s, "polyPlane", json!({"width": 10, "depth": 10, "subdivisionsX": 10, "subdivisionsZ": 10, "name": "other"}));
    // Picked on the first plane; the second has no picked points, and falls off around the centre.
    run(&mut s, "softMod", json!({"args": ["pPlane1.vtx[0]"], "falloffRadius": 2.0}));
    run(&mut s, "deformer", json!({"args": ["softMod1"], "edit": true, "geometry": "other"}));
    run(&mut s, "setAttr", json!({"args": ["softMod1Handle.ty", 1.0]}));
    assert!((point(&mut s, "pPlane1.vtx[0]")[1] - 1.0).abs() < 1e-6);
    assert!(point(&mut s, "other.vtx[60]")[1].abs() < 1e-6, "the far corner is the centre here, not the middle");
    assert!((point(&mut s, "other.vtx[0]")[1] - 1.0).abs() < 1e-6);
    // One envelope, one radius.
    run(&mut s, "setAttr", json!({"args": ["softMod1.envelope", 0.0]}));
    assert!(point(&mut s, "other.vtx[0]")[1].abs() < 1e-9);
    run(&mut s, "setAttr", json!({"args": ["softMod1.envelope", 1.0]}));
    assert_eq!(run(&mut s, "percent", json!({"args": ["softMod1", "other.vtx[0]"], "query": true, "queried": ["value"]})), json!([1.0]));
    // A further mesh's wrap, mush and wire bind to that mesh.
    let mut t = Session::new();
    run(&mut t, "polyCube", json!({"name": "a"}));
    run(&mut t, "polyCube", json!({"name": "b"}));
    run(&mut t, "setAttr", json!({"args": ["b.tx", 4.0]}));
    run(&mut t, "cluster", json!({"args": ["a"], "name": "ca"}));
    run(&mut t, "cluster", json!({"args": ["b"], "name": "cb"}));
    run(&mut t, "deltaMush", json!({"args": ["a", "b"]}));
    assert_eq!(run(&mut t, "deformer", json!({"args": ["deltaMush1"], "query": true, "geometry": true})), json!(["aShape", "bShape"]));
    let (lo, _) = bounds(&mut t, "b");
    assert!((lo[0] - 3.5).abs() < 1e-6, "{lo:?}");
    run(&mut t, "setAttr", json!({"args": ["cbHandle.ty", 1.0]}));
    let (lo, _) = bounds(&mut t, "b");
    assert!((lo[1] - 0.5).abs() < 1e-6 && (lo[0] - 3.5).abs() < 1e-6, "{lo:?}");
}

#[test]
fn deleting_one_of_two_wires_leaves_the_other_working() {
    let mut s = Session::new();
    wire_scene(&mut s);
    run(&mut s, "polyPlane", json!({"width": 8, "depth": 0.0001, "subdivisionsX": 8, "subdivisionsZ": 1, "name": "other"}));
    run(&mut s, "setAttr", json!({"args": ["other.tz", 3.0]}));
    run(&mut s, "wire", json!({"args": ["sheet"], "wire": ["line", "other"], "dropoffDistance": [0, 1.0, 1, 1.0]}));
    run(&mut s, "delete", json!({"args": ["line"]}));
    run(&mut s, "setAttr", json!({"args": ["other.ty", 1.0]}));
    assert!((point(&mut s, &sheet_point(0, 3))[1] - 1.0).abs() < 1e-3, "{:?}", point(&mut s, &sheet_point(0, 3)));
    assert!(point(&mut s, &sheet_point(0, 0))[1].abs() < 1e-9);
    assert!(s.undo());
    assert!(s.undo());
    run(&mut s, "setAttr", json!({"args": ["line.ty", 2.0]}));
    assert!((point(&mut s, &sheet_point(0, 0))[1] - 2.0).abs() < 1e-3, "the deleted wire came back: {:?}", point(&mut s, &sheet_point(0, 0)));
}
