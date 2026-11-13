//! Tests: modeling.

use aztek_geo::primitives::{make_cube, CubeOptions};
use aztek_geo::{Index, Mesh};
use aztek_scene::components::ComponentMode;
use aztek_scene::mesh_edit::transform::component_normals;
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::transform_ops::{self as xf, Space};
use aztek_scene::Session;
use glam::DVec3;
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

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn approx_eps(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps * (1.0 + a.abs().max(b.abs()))
}

fn get_f(s: &mut Session, plug: String) -> f64 {
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap()
}

fn mesh_of(s: &mut Session, name: &str) -> Mesh {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap();
    (**s.graph.node(aztek_scene::components::as_mesh(&s.graph, id).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap()).clone()
}

fn world_points(s: &mut Session, m: &str) -> Vec<[f64; 3]> {
    let d = run(s, "meshData", json!({"args": [m], "worldSpace": true}));
    d["points"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| [p[0].as_f64().unwrap(), p[1].as_f64().unwrap(), p[2].as_f64().unwrap()])
        .collect()
}

fn same_points(before: &[[f64; 3]], after: &[[f64; 3]]) {
    assert_eq!(before.len(), after.len());
    for (b, a) in before.iter().zip(after) {
        for k in 0..3 {
            assert!(approx_eps(a[k], b[k], 1e-9), "{a:?} vs {b:?}");
        }
    }
}

fn cube100(s: &mut Session, extra: Value) -> String {
    let mut call = json!({"width": 100.0, "height": 100.0, "depth": 100.0});
    for (k, v) in extra.as_object().unwrap() {
        call[k] = v.clone();
    }
    first(run(s, "polyCube", call))
}

#[test]
fn component_selection_and_modeling_ops() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({}));
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    assert_eq!(run(&mut s, "selectMode", json!({})), "face");
    assert_eq!(run(&mut s, "componentSelection", json!({}))[&cube]["faces"], json!([0]));

    run(&mut s, "polyExtrude", json!({"thickness": 20}));
    assert_eq!(faces(&mut s, &cube), 10);
    // The edit is recorded as history after the generator.
    let history = run(&mut s, "listHistory", json!({"args": [cube]}));
    assert_eq!(history[1], "polyExtrudeFace1");
    assert_eq!(history[2], "polyCube1");
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &cube), 6);
    run(&mut s, "redo", json!({}));
    assert_eq!(faces(&mut s, &cube), 10);

    run(&mut s, "polyInset", json!({"thickness": 5}));
    assert_eq!(faces(&mut s, &cube), 14);

    run(&mut s, "convertSelection", json!({"args": ["edge"]}));
    let sel = run(&mut s, "componentSelection", json!({}));
    assert_eq!(sel[&cube]["edges"].as_array().unwrap().len(), 4); // the inset face's border
    run(&mut s, "polyBevel", json!({"offset": 1}));
    assert!(faces(&mut s, &cube) > 14);

    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0, 1]}));
    run(&mut s, "deleteComponents", json!({}));
    assert!(faces(&mut s, &cube) > 0);
    run(&mut s, "polyFillHole", json!({}));

    run(&mut s, "newScene", json!({}));
    let plane = first(run(&mut s, "polyPlane", json!({"width": 100.0, "depth": 100.0, "subdivisionsX": 4, "subdivisionsZ": 1})));
    run(&mut s, "setSymmetry", json!({"args": [plane], "axis": "x"}));
    run(&mut s, "selectComponents", json!({"args": [plane], "vertices": [0]}));
    run(&mut s, "moveComponents", json!({"by": [0, 10, 0]}));
    run(&mut s, "rotateComponents", json!({"by": [0, 0, 0]}));
    run(&mut s, "scaleComponents", json!({"by": [1, 1, 1]}));
    let after = run(&mut s, "meshInfo", json!({"args": [plane]}));
    assert!(approx(after["boundsMax"][1].as_f64().unwrap(), 10.0));

    run(&mut s, "newScene", json!({}));
    let a = cube100(&mut s, json!({"position": [-100, 0, 0]}));
    let b = cube100(&mut s, json!({"position": [100, 0, 0]}));
    let c = first(run(&mut s, "polyCombine", json!({"args": [a, b], "name": "Both"})));
    assert_eq!(faces(&mut s, &c), 12);
    let parts = run(&mut s, "polySeparate", json!({"args": [c]}));
    // Two transforms, then a polySeparate node for each.
    assert_eq!(parts.as_array().unwrap().len(), 4);
}

#[test]
fn whole_mesh_commands_take_groups() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({"position": [-100, 0, 0]})));
    let b = first(run(&mut s, "polyCube", json!({"position": [100, 0, 0]})));
    let c = first(run(&mut s, "polyCube", json!({"position": [0, 100, 0]})));
    let inner = first(run(&mut s, "group", json!({"args": [b, c]})));
    let outer = first(run(&mut s, "group", json!({"args": [a, inner]})));
    let combined = first(run(&mut s, "polyCombine", json!({"args": [outer]})));
    assert_eq!(faces(&mut s, &combined), 18);

    run(&mut s, "newScene", json!({}));
    let a = first(run(&mut s, "polyCube", json!({"position": [-100, 0, 0]})));
    let b = first(run(&mut s, "polyCube", json!({"position": [100, 0, 0]})));
    let grp = first(run(&mut s, "group", json!({"args": [a, b]})));
    let empty = first(run(&mut s, "group", json!({"empty": true})));
    // A hierarchy selection: the group, its meshes again, and a group with no meshes.
    let mut selection = run(&mut s, "listRelatives", json!({"args": [grp]})).as_array().unwrap().clone();
    selection.push(json!(grp));
    selection.push(json!(empty));
    run(&mut s, "select", json!({"args": selection}));
    let combined = first(run(&mut s, "polyCombine", json!({})));
    assert_eq!(faces(&mut s, &combined), 12);

    assert!(s.run("polySubdivide", json!({"args": [empty]})).is_err());
}

#[test]
fn duplicate_freeze_center_pivot() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({"name": "Box", "position": [10, 20, 30]}));
    run(&mut s, "addModifier", json!({"args": [cube], "kind": "subdivision"}));
    let dup = run(&mut s, "duplicate", json!({"args": [cube]}));
    assert_eq!(dup.as_array().unwrap().len(), 1);
    let copy = first(dup);
    assert_ne!(copy, cube);
    assert_eq!(faces(&mut s, &copy), faces(&mut s, &cube));
    // The copy has its own live stack: changing the original's generator leaves the copy alone.
    assert_eq!(run(&mut s, "modifiers", json!({"args": [copy]})).as_array().unwrap().len(), 1);
    run(&mut s, "setAttr", json!({"args": ["polyCube1.subdivisionsX", 3]}));
    assert_ne!(faces(&mut s, &cube), faces(&mut s, &copy));

    run(&mut s, "rotate", json!({"args": [copy], "by": [0, 45, 0]}));
    run(&mut s, "scale", json!({"args": [copy], "by": [2, 1, 1]}));
    let before = world_points(&mut s, &copy);
    run(&mut s, "freezeTransforms", json!({"args": [copy]}));
    assert!(approx(get_f(&mut s, format!("{copy}.ry")), 0.0));
    assert!(approx(get_f(&mut s, format!("{copy}.sx")), 1.0));
    let after = world_points(&mut s, &copy);
    same_points(&before, &after);

    run(&mut s, "centerPivot", json!({"args": [copy]}));
    run(&mut s, "undo", json!({}));
}

#[test]
fn freeze_and_center_pivot_on_a_group_reach_its_children() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({"name": "A", "position": [10, 0, 0]})));
    let b = first(run(&mut s, "polyCube", json!({"name": "B", "position": [30, 0, 0]})));
    let grp = first(run(&mut s, "group", json!({"args": [a, b]})));
    run(&mut s, "move", json!({"args": [grp], "by": [0, 5, 0]}));
    run(&mut s, "rotate", json!({"args": [a], "by": [0, 30, 0]}));

    run(&mut s, "centerPivot", json!({"args": [grp]}));
    let piv = run(&mut s, "pivot", json!({"args": [grp]}));
    assert!(approx_eps(piv[0].as_f64().unwrap(), 20.0, 0.05));
    assert!(approx(piv[1].as_f64().unwrap(), 5.0));

    let before = world_points(&mut s, &a);
    run(&mut s, "freezeTransforms", json!({"args": [grp]}));
    for n in [&grp, &a, &b] {
        assert!(approx(get_f(&mut s, format!("{n}.ty")), 0.0));
        assert!(approx(get_f(&mut s, format!("{n}.tx")), 0.0));
    }
    assert!(approx(get_f(&mut s, format!("{a}.ry")), 0.0));
    let after = world_points(&mut s, &a);
    same_points(&before, &after);

    run(&mut s, "undo", json!({}));
    run(&mut s, "freezeTransforms", json!({"args": [grp], "hierarchy": false}));
    assert!(approx(get_f(&mut s, format!("{grp}.ty")), 0.0));
    assert!(approx(get_f(&mut s, format!("{a}.ty")), 5.0));
}

#[test]
fn set_pivot_turns_the_pivots_axes_about_a_world_axis_and_leaves_the_object() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({}));
    run(&mut s, "rotate", json!({"args": [cube], "by": [0, 20, 0]}));
    let id = s.graph.find(&cube).unwrap().unwrap();
    let start = xf::pivot_frame(&mut s.graph, id);
    let before = world_points(&mut s, &cube);
    let object = xf::frame(&mut s.graph, id, Space::Object);

    // Twice 30 degrees about world Z, as two drags on a ring would.
    for _ in 0..2 {
        run(&mut s, "setPivot", json!({"args": [cube], "turnAxis": [0, 0, 1], "turnAngle": 30.0}));
    }
    let want = glam::DMat3::from_axis_angle(DVec3::Z, 60f64.to_radians()) * start;
    let axes = xf::pivot_frame(&mut s.graph, id);
    for c in 0..3 {
        assert!((axes.col(c) - want.col(c)).length() < 1e-6);
        assert!((xf::frame(&mut s.graph, id, Space::Object).col(c) - object.col(c)).length() < 1e-9);
    }
    same_points(&before, &world_points(&mut s, &cube));

    assert!(s.undo());
    assert!(s.undo());
    let axes = xf::pivot_frame(&mut s.graph, id);
    for c in 0..3 {
        assert!((axes.col(c) - start.col(c)).length() < 1e-6);
    }
}

#[test]
fn bake_pivot_turns_the_object_onto_its_pivots_axes_without_moving_it() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({}));
    run(&mut s, "rotate", json!({"args": [cube], "by": [0, 20, 0]}));
    run(&mut s, "setPivot", json!({"args": [cube], "position": [10, 0, 0], "orient": [0, 0, 30]}));
    let id = s.graph.find(&cube).unwrap().unwrap();
    let axes = xf::pivot_frame(&mut s.graph, id);
    let before = world_points(&mut s, &cube);

    run(&mut s, "bakePivot", json!({"args": [cube]}));
    let object = xf::frame(&mut s.graph, id, Space::Object);
    for c in 0..3 {
        assert!((object.col(c) - axes.col(c)).length() < 1e-6);
    }
    assert!((xf::world_pivot(&mut s.graph, id) - DVec3::new(10.0, 0.0, 0.0)).length() < 1e-6);
    assert!(approx(run(&mut s, "getAttr", json!({"args": [format!("{cube}.pivotOrient")]}))[2].as_f64().unwrap(), 0.0));
    let after = world_points(&mut s, &cube);
    same_points(&before, &after);
}

#[test]
fn bake_pivot_makes_the_pivot_the_objects_origin() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({}));
    let child = first(run(&mut s, "polyCube", json!({"name": "bakeChild"})));
    run(&mut s, "parent", json!({"args": [child.clone(), cube.clone()]}));
    run(&mut s, "move", json!({"args": [cube], "by": [5, 0, 0]}));
    run(&mut s, "rotate", json!({"args": [cube], "by": [0, 40, 0]}));
    run(&mut s, "movePivot", json!({"args": [cube], "by": [3, 4, 0]}));
    let id = s.graph.find(&cube).unwrap().unwrap();
    let pivot = xf::world_pivot(&mut s.graph, id);
    assert!((pivot - DVec3::new(8.0, 4.0, 0.0)).length() < 1e-6, "{pivot}");
    let (before, child_before) = (world_points(&mut s, &cube), world_points(&mut s, &child));

    // Orientation alone has nothing to do here.
    run(&mut s, "bakePivot", json!({"args": [cube], "orientation": true}));
    assert!(!approx(run(&mut s, "getAttr", json!({"args": [format!("{cube}.rotatePivot")]}))[1].as_f64().unwrap(), 0.0));

    run(&mut s, "bakePivot", json!({"args": [cube]}));
    let t = run(&mut s, "getAttr", json!({"args": [format!("{cube}.translate")]}));
    assert!(approx(t[0].as_f64().unwrap(), 8.0) && approx(t[1].as_f64().unwrap(), 4.0) && approx(t[2].as_f64().unwrap(), 0.0), "{t}");
    for attr in ["rotatePivot", "scalePivot", "rotatePivotTranslate", "scalePivotTranslate"] {
        let v = run(&mut s, "getAttr", json!({"args": [format!("{cube}.{attr}")]}));
        assert!((0..3).all(|i| approx(v[i].as_f64().unwrap(), 0.0)), "{attr} {v}");
    }
    assert!((xf::world_pivot(&mut s.graph, id) - pivot).length() < 1e-6);
    same_points(&before, &world_points(&mut s, &cube));
    same_points(&child_before, &world_points(&mut s, &child));
    run(&mut s, "undo", json!({}));
    same_points(&before, &world_points(&mut s, &cube));
}

#[test]
fn freeze_without_hierarchy_leaves_childrens_values() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({"name": "A"})));
    let grp = first(run(&mut s, "group", json!({"args": [a]})));
    run(&mut s, "move", json!({"args": [grp], "by": [0, 5, 0]}));
    run(&mut s, "freezeTransforms", json!({"args": [grp], "hierarchy": false}));
    assert!(approx(get_f(&mut s, format!("{grp}.ty")), 0.0));
    assert!(approx(get_f(&mut s, format!("{a}.ty")), 5.0));
}

#[test]
fn component_normals_are_area_weighted_and_selection_aware() {
    let cube = make_cube(&CubeOptions::default());
    let all: Vec<Index> = (0..cube.face_capacity()).collect();
    let n = component_normals(&cube, ComponentMode::Face, &all);
    for p in 0..cube.point_capacity() {
        let corner = cube.position(p).signum() / 3f64.sqrt();
        assert!((n[p as usize] - corner).length() < 1e-9);
    }
    // One selected face: its corners take only that face's normal.
    let one = component_normals(&cube, ComponentMode::Face, &[0]);
    let face = cube.face_normal(0);
    for p in cube.face_points(0) {
        assert!((one[p as usize] - face).length() < 1e-9);
    }
}

#[test]
fn move_components_normal_offsets_each_point_along_its_own_normal() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({}));
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0, 1, 2, 3, 4, 5]}));
    run(&mut s, "moveComponents", json!({"normal": 10}));
    let info = run(&mut s, "meshInfo", json!({"args": [cube]}));
    // Every corner leaves along its diagonal, so all six faces move out by the same amount.
    let out = 50.0 + 10.0 / 3f64.sqrt();
    for i in 0..3 {
        assert!(approx(info["boundsMax"][i].as_f64().unwrap(), out));
        assert!(approx(info["boundsMin"][i].as_f64().unwrap(), -out));
    }
    run(&mut s, "undo", json!({}));
    assert!(approx(run(&mut s, "meshInfo", json!({"args": [cube]}))["boundsMax"][0].as_f64().unwrap(), 50.0));

    // A single face moves out by the full distance and leaves the rest alone.
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    let before = mesh_of(&mut s, &cube);
    let normal = before.face_normal(0);
    run(&mut s, "moveComponents", json!({"normal": 7}));
    let after = mesh_of(&mut s, &cube);
    let face_pts = before.face_points(0);
    for p in 0..before.point_capacity() {
        let expect = before.position(p) + if face_pts.contains(&p) { normal * 7.0 } else { DVec3::ZERO };
        assert!((after.position(p) - expect).length() < 1e-9);
    }
    assert!(s.run("moveComponents", json!({})).is_err());
    assert!(s.run("moveComponents", json!({"by": [1, 0, 0], "normal": 1})).is_err());
}

#[test]
fn move_components_normal_keeps_symmetry() {
    let mut s = Session::new();
    let cube = cube100(&mut s, json!({"subdivisionsX": 4, "subdivisionsY": 2}));
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    let m = mesh_of(&mut s, &cube);
    // The +X half of the top, so the mirror side follows from symmetry alone.
    let pts: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.position(p).y > 49.0 && m.position(p).x > 1.0).collect();
    run(&mut s, "selectComponents", json!({"args": [cube], "vertices": pts}));
    run(&mut s, "moveComponents", json!({"normal": 6}));
    let after = mesh_of(&mut s, &cube);
    let mut moved = 0;
    for p in 0..after.point_capacity() {
        let at = after.position(p);
        moved += ((at - m.position(p)).length() > 1e-6) as usize;
        let mirrored = DVec3::new(-at.x, at.y, at.z);
        let best = (0..after.point_capacity()).map(|q| (after.position(q) - mirrored).length()).fold(f64::MAX, f64::min);
        assert!(best < 1e-9);
    }
    assert_eq!(moved, 2 * pts.len());
}

#[test]
fn freeze_keeps_the_pivots_world_position_and_oriented_axes() {
    let mut s = Session::new();
    let a = first(run(&mut s, "polyCube", json!({"name": "A", "position": [10, 0, 0]})));
    run(&mut s, "rotate", json!({"args": [a], "by": [0, 30, 0]}));
    run(&mut s, "scale", json!({"args": [a], "by": [2, 1, 1]}));
    run(&mut s, "setPivot", json!({"args": [a], "position": [13, 4, 0], "orient": [0, 0, 20]}));
    let id = s.graph.find(&a).unwrap().unwrap();
    let at = xf::world_pivot(&mut s.graph, id);
    let axes = xf::pivot_frame(&mut s.graph, id);
    run(&mut s, "freezeTransforms", json!({"args": [a]}));
    assert!((xf::world_pivot(&mut s.graph, id) - at).length() < 1e-6);
    let now = xf::pivot_frame(&mut s.graph, id);
    for c in 0..3 {
        assert!((now.col(c).normalize() - axes.col(c).normalize()).length() < 1e-6);
    }
}

#[test]
fn unfrozen_transforms_lists_non_identity_objects_and_filters_rotations_by_step() {
    let mut s = Session::new();
    let frozen = first(run(&mut s, "polyCube", json!({"name": "Frozen"})));
    let moved = first(run(&mut s, "polyCube", json!({"name": "Moved", "position": [10, 0, 0]})));
    let quarter = first(run(&mut s, "polyCube", json!({"name": "Quarter"})));
    let tilted = first(run(&mut s, "polyCube", json!({"name": "Tilted"})));
    let scaled = first(run(&mut s, "polyCube", json!({"name": "Scaled"})));
    run(&mut s, "rotate", json!({"args": [quarter], "by": [0, 90, 0]}));
    run(&mut s, "rotate", json!({"args": [tilted], "by": [0, 37.5, 0]}));
    run(&mut s, "scale", json!({"args": [scaled], "by": [2, 2, 2]}));

    let names = |rows: Value| {
        let mut out: Vec<String> = rows.as_array().unwrap().iter().map(|r| r["name"].as_str().unwrap().to_string()).collect();
        out.sort();
        out
    };
    let sorted = |v: &[&String]| {
        let mut out: Vec<String> = v.iter().map(|s| s.to_string()).collect();
        out.sort();
        out
    };
    let all = json!({"args": [frozen, moved, quarter, tilted, scaled]});
    assert_eq!(names(run(&mut s, "unfrozenTransforms", all.clone())), sorted(&[&moved, &quarter, &scaled, &tilted]));

    let mut off_step = all.clone();
    off_step["translate"] = json!(false);
    off_step["scale"] = json!(false);
    off_step["rotateStep"] = json!(45.0);
    assert_eq!(names(run(&mut s, "unfrozenTransforms", off_step.clone())), vec![tilted.clone()]);
    off_step["invert"] = json!(true);
    assert_eq!(names(run(&mut s, "unfrozenTransforms", off_step)), vec![quarter.clone()]);

    let rows = run(&mut s, "unfrozenTransforms", json!({"args": [tilted]}));
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["unfrozen"], json!(["rotate"]));
    assert!(approx(rows[0]["rotate"][1].as_f64().unwrap(), 37.5));

    assert_eq!(names(run(&mut s, "unfrozenTransforms", json!({"scene": true}))), sorted(&[&moved, &quarter, &scaled, &tilted]));

    run(&mut s, "freezeTransforms", json!({"args": [moved, quarter, tilted, scaled]}));
    assert!(run(&mut s, "unfrozenTransforms", json!({"scene": true})).as_array().unwrap().is_empty());
}

/// Whichever way an edit is asked for, it's one more node on what was there, as in the reference. These used to bake and wipe it.
#[test]
fn edits_add_to_the_history_and_never_wipe_it() {
    for (cmd, comps, node) in [
        ("polySubdivide", "", "polySmoothFace1"),
        ("polyMirror", "", "polyMirror1"),
        ("polyReduce", "", "polyReduce1"),
        ("polySmooth", "", "polySmoothFace1"),
    ] {
        let mut s = Session::new();
        s.run("polyCube", json!({"name": "box", "sx": 2, "sy": 2, "sz": 2})).unwrap();
        let target = if comps.is_empty() { "box".to_string() } else { format!("box.{comps}") };
        s.run("select", json!({"args": [target]})).unwrap();
        s.run(cmd, json!({})).unwrap_or_else(|e| panic!("{cmd}: {e}"));
        let h = s.run("listHistory", json!({"args": ["box"]})).unwrap().to_string();
        assert!(h.contains("polyCube1"), "{cmd} kept what was there: {h}");
        assert!(h.contains(node), "{cmd} added {node}: {h}");
        s.run("undo", json!({})).unwrap();
        assert!(!s.run("listHistory", json!({"args": ["box"]})).unwrap().to_string().contains(node), "{cmd} undoes");
    }
}
