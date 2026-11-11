//! Tests: meshes.

use aztek_geo::{Bounds, Mesh};
use aztek_ops::{execute, CallSource};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::last_op;
use aztek_scene::mesh_edit::transform;
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::transform::Transform;
use aztek_scene::Session;
use glam::DVec3;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn run_ui(s: &mut Session, name: &str, call: Value) -> Value {
    execute(s, name, &call, CallSource::Ui).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn approx_eps(a: f64, b: f64, eps: f64) -> bool {
    (a - b).abs() < eps * (1.0 + a.abs().max(b.abs()))
}

fn mesh_of(s: &mut Session, name: &str) -> Mesh {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap();
    (**s.graph.node(aztek_scene::components::as_mesh(&s.graph, id).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap()).clone()
}

fn bounds(s: &mut Session, name: &Value) -> Bounds {
    mesh_of(s, name.as_str().unwrap()).bounds()
}

fn up_axis_of(s: &Session, generator: &Value) -> i64 {
    let id = s.graph.find(generator.as_str().unwrap()).unwrap().unwrap();
    let n = s.graph.node(id).unwrap();
    n.int(n.ty().attr_index("upAxis").unwrap())
}

fn first_face_normal(m: &Mesh) -> DVec3 {
    (0..m.face_capacity()).find(|&f| m.face_valid(f)).map(|f| m.face_normal(f)).unwrap_or_default()
}

fn get(s: &mut Session, plug: &str) -> f64 {
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap()
}

#[test]
fn primitives_are_procedural_until_history_is_deleted() {
    let mut s = Session::new();
    run(&mut s, "newScene", json!({}));
    let r = run(&mut s, "polyCube", json!({"height": 100.0, "depth": 100.0, "width": 200, "subdivisionsX": 2}));
    let (mesh, gen) = (r[0].as_str().unwrap().to_string(), r[1].as_str().unwrap().to_string());
    assert_eq!(mesh, "pCube1");
    let info = run(&mut s, "meshInfo", json!({"args": [mesh]}));
    // 2 columns on four sides, 1 on the two ends
    assert_eq!(info["faces"], 10);
    assert!(approx(info["boundsMax"][0].as_f64().unwrap(), 100.0));
    run(&mut s, "setAttr", json!({"args": [format!("{gen}.subdivisionsY"), 3]}));
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [mesh]}))["faces"], 22);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [mesh]}))["faces"], 10);
    run(&mut s, "bakeModifierStack", json!({"args": [mesh]}));
    assert!(!run(&mut s, "objExists", json!({"args": [gen]})).as_bool().unwrap());
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [mesh]}))["faces"], 10);
    let baked = run(&mut s, "polySphere", json!({"radius": 50.0, "generator": false}));
    assert!(baked[1].is_null());
}

#[test]
fn set_mesh_points_moves_every_point_in_one_undo_step() {
    let mut s = Session::new();
    let mesh = run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2}))[0].clone();
    let before = run(&mut s, "meshData", json!({"args": [mesh]}))["points"].clone();
    let moved: Vec<Value> = before
        .as_array()
        .unwrap()
        .iter()
        .map(|p| json!([p[0].as_f64().unwrap() * 2.0, p[1].as_f64().unwrap() + 5.0, p[2]]))
        .collect();
    let moved = Value::Array(moved);
    assert_eq!(run(&mut s, "setMeshPoints", json!({"args": [mesh], "points": moved})), json!(before.as_array().unwrap().len()));
    assert_eq!(run(&mut s, "meshData", json!({"args": [mesh]}))["points"], moved);
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [mesh]}))["faces"], 6);
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "meshData", json!({"args": [mesh]}))["points"], before);
    assert!(s.run("setMeshPoints", json!({"args": [mesh], "points": [[0, 0, 0]]})).is_err());
}

#[test]
fn obj_export_and_import() {
    let mut s = Session::new();
    run(&mut s, "polyCylinder", json!({"radius": 50.0, "height": 100.0, "name": "Can", "position": [0, 50, 0]}));
    let dir = std::env::temp_dir().join("aztek_tests");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("can.obj").to_string_lossy().to_string();
    run(&mut s, "exportObj", json!({"args": [path, "Can"]}));
    run(&mut s, "newScene", json!({}));
    let names = run(&mut s, "importObj", json!({"args": [path]}));
    assert_eq!(names.as_array().unwrap().len(), 1);
    let info = run(&mut s, "meshInfo", json!({"args": [names[0]]}));
    assert!(approx_eps(info["boundsMin"][1].as_f64().unwrap(), 0.0, 1e-6));
}

#[test]
fn primitive_up_axis_turns_the_geometry_to_each_axis() {
    let mut s = Session::new();
    let cases = [
        ("x", DVec3::X),
        ("y", DVec3::Y),
        ("z", DVec3::Z),
        ("-x", DVec3::NEG_X),
        ("-y", DVec3::NEG_Y),
        ("-z", DVec3::NEG_Z),
    ];
    for (axis, dir) in cases {
        // Base at the origin, so the side it rises toward shows the sign.
        let r = run(&mut s, "polyCylinder", json!({"radius": 10.0, "height": 100.0, "baseAtOrigin": true, "upAxis": axis}));
        let b = bounds(&mut s, &r[0]);
        let size = b.size();
        assert!(approx(size.dot(dir.abs()), 100.0), "{axis}");
        assert!(approx_eps(size.x + size.y + size.z, 140.0, 0.01), "{axis}");
        let centre = (b.min + b.max) * 0.5;
        assert!(approx(centre.dot(dir), 50.0), "{axis}");

        // A plane faces along its up, so the turn keeps the winding.
        let plane = run(&mut s, "polyPlane", json!({"width": 100.0, "depth": 100.0, "upAxis": axis}));
        let m = mesh_of(&mut s, plane[0].as_str().unwrap());
        assert!(approx(first_face_normal(&m).dot(dir), 1.0), "{axis}");
    }
}

const PRIMITIVES: [&str; 11] = [
    "polyCube",
    "polySphere",
    "polyCylinder",
    "polyCapsule",
    "polyCone",
    "polyPlane",
    "polyTorus",
    "polyDisc",
    "polyPipe",
    "polyQuadSphere",
    "polyIcoSphere",
];

#[test]
fn every_primitive_has_up_axis_defaulting_to_the_scene_up_axis() {
    let mut s = Session::new();
    for ty in PRIMITIVES {
        let r = run(&mut s, ty, json!({}));
        assert_eq!(up_axis_of(&s, &r[1]), 1, "{ty}");
        let z = run(&mut s, ty, json!({"upAxis": "z"}));
        assert_eq!(up_axis_of(&s, &z[1]), 2, "{ty}");
        let (a, b) = (bounds(&mut s, &r[0]), bounds(&mut s, &z[0]));
        assert!(approx(a.size().y, b.size().z), "{ty}");
    }
    assert!(s.run("polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "upAxis": "w"})).is_err());

    aztek_scene::world::set_up_z(true);
    let r = run(&mut s, "polyCone", json!({"radius": 50.0, "height": 100.0}));
    aztek_scene::world::set_up_z(false);
    assert_eq!(up_axis_of(&s, &r[1]), 2);
}

#[test]
fn new_primitives_have_the_reference_default_sizes() {
    let mut s = Session::new();
    // The cube and plane are a unit across; the round ones a radius of 1, the tall ones 2 high.
    for ty in PRIMITIVES {
        let r = run(&mut s, ty, json!({}));
        let size = bounds(&mut s, &r[0]).size();
        let want = if matches!(&*ty, "polyCube" | "polyPlane") { 1.0 } else if matches!(&*ty, "polyTorus") { 3.0 } else { 2.0 };
        // Faceted spheres fall a little short.
        assert!(size.max_element() <= want + 1e-9, "{ty}: {size}");
        assert!(size.max_element() > want * 0.9, "{ty}: {size}");
    }
}

#[test]
fn a_primitive_created_from_the_ui_is_adjustable_up_axis_included() {
    let mut s = Session::new();
    let r = run_ui(&mut s, "polyTorus", json!({"radius": 50.0, "sectionRadius": 20.0, "position": [10, 0, 0]}));
    assert!(last_op::live(&s));
    let op = last_op::current(&s).unwrap().clone();
    assert_eq!(op.command, "polyTorus");
    assert_eq!(op.call["upAxis"], "y");
    for f in last_op::editable_flags(&s, "polyTorus", &[], &[]) {
        if f.name == "radius" {
            assert_eq!(last_op::flag_value(&op, &f), json!(50.0));
        }
    }

    last_op::rerun(&mut s, &json!({"upAxis": "z", "radius": 80.0}), CallSource::Script, false).unwrap();
    let gens = run(&mut s, "ls", json!({"type": "polyTorus"}));
    assert_eq!(gens.as_array().unwrap().len(), 1);
    assert_eq!(up_axis_of(&s, &gens[0]), 2);
    let b = bounds(&mut s, &r[0]);
    assert!(approx(b.size().x, 200.0));
    assert!(approx(b.size().z, 40.0));
    assert!(approx(get(&mut s, &format!("{}.translateX", r[0].as_str().unwrap())), 10.0));

    run(&mut s, "undo", json!({}));
    let gens = run(&mut s, "ls", json!({"type": "polyTorus"}));
    assert_eq!(gens.as_array().unwrap().len(), 1);
    assert_eq!(up_axis_of(&s, &gens[0]), 1);
    run(&mut s, "undo", json!({}));
    assert!(run(&mut s, "ls", json!({"type": "polyTorus"})).as_array().unwrap().is_empty());
}

#[test]
fn moving_a_created_primitive_keeps_it_adjustable_and_the_move_survives_adjusting() {
    let mut s = Session::new();
    let r = run_ui(&mut s, "polyQuadSphere", json!({"radius": 50.0}));
    let name = r[0].as_str().unwrap().to_string();
    assert!(last_op::live(&s));
    run_ui(&mut s, "move", json!({"args": [name], "by": [0, 30, 0]}));
    assert!(last_op::reachable(&s));
    run_ui(&mut s, "setAttr", json!({"args": [format!("{name}.translateX"), 5.0]}));
    assert!(last_op::reachable(&s));

    last_op::rerun(&mut s, &json!({"subdivisions": 5}), CallSource::Script, false).unwrap();
    assert!(approx(get(&mut s, &format!("{name}.translateY")), 30.0));
    assert!(approx(get(&mut s, &format!("{name}.translateX")), 5.0));
    // A second adjustment keeps the move too.
    last_op::rerun(&mut s, &json!({"subdivisions": 4}), CallSource::Script, false).unwrap();
    assert!(approx(get(&mut s, &format!("{name}.translateY")), 30.0));
    let gens = run(&mut s, "ls", json!({"type": "polyQuadSphere"}));
    assert_eq!(gens.as_array().unwrap().len(), 1);
    let sub = run(&mut s, "getAttr", json!({"args": [format!("{}.subdivisions", gens[0].as_str().unwrap())]}));
    assert_eq!(sub.as_i64(), Some(4));

    // Moving another object ends it.
    run_ui(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0}));
    run_ui(&mut s, "move", json!({"args": [name], "by": [0, 1, 0]}));
    assert_eq!(last_op::current(&s).unwrap().command, "polyCube");
    assert!(!last_op::reachable(&s));
}

#[test]
fn placing_a_primitive_turns_the_scene_up_to_the_surface_normal() {
    let mut s = Session::new();
    let r = run(
        &mut s,
        "polyCylinder",
        json!({"height": 100.0, "radius": 10.0, "upAxis": "x", "position": [0, 0, 0], "normal": [0, 0, 1]}),
    );
    s.graph.evaluate();
    let id = s.graph.find(r[0].as_str().unwrap()).unwrap().unwrap();
    let w = s.graph.node(id).unwrap().matrix(Transform::WORLD_MATRIX);
    // The cylinder's axis is x in object space; standing its y on +z leaves the axis along world x.
    let axis = w.transform_vector3(DVec3::X);
    assert!(approx(axis.x.abs(), 1.0));
}

fn query_cube(s: &mut Session) -> Value {
    run(s, "newScene", json!({}));
    run(s, "polyCube", json!({"width": 20, "height": 10, "depth": 30, "subdivisionsX": 2, "position": [0, 5, 0]}))[0].clone()
}

#[test]
fn find_components_filters_by_normal_side_box_extreme_and_direction() {
    let mut s = Session::new();
    let m = query_cube(&mut s);
    let top = run(&mut s, "findComponents", json!({"args": [m], "kind": "face", "normal": [0, 1, 0]}));
    assert_eq!(top.as_array().unwrap().len(), 2);
    let top_plus = run(&mut s, "findComponents", json!({"args": [m], "kind": "face", "normal": [0, 1, 0], "side": "+x"}));
    assert_eq!(top_plus.as_array().unwrap().len(), 1);
    let info = run(&mut s, "componentInfo", json!({"args": [m], "kind": "face", "ids": top_plus}));
    assert_eq!(info.as_array().unwrap().len(), 1);
    assert!(approx(info[0]["center"][0].as_f64().unwrap(), 5.0));
    assert!(approx(info[0]["center"][1].as_f64().unwrap(), 10.0));
    assert!(approx(info[0]["normal"][1].as_f64().unwrap(), 1.0));
    assert_eq!(info[0]["neighbors"].as_array().unwrap().len(), 4);

    // Open box sides: everything above y=9 is the top two faces.
    let above = run(&mut s, "findComponents", json!({"args": [m], "kind": "face", "box": [[null, 9, null], [null, null, null]]}));
    assert_eq!(above, top);

    let front = run(&mut s, "findComponents", json!({"args": [m], "kind": "face", "extreme": "+z", "count": 2}));
    for f in run(&mut s, "componentInfo", json!({"args": [m], "kind": "face", "ids": front})).as_array().unwrap() {
        assert!(approx(f["normal"][2].as_f64().unwrap(), 1.0));
    }

    let centre = run(&mut s, "findComponents", json!({"args": [m], "kind": "vertex", "side": "center"}));
    assert_eq!(centre.as_array().unwrap().len(), 4);
    let vertical = run(&mut s, "findComponents", json!({"args": [m], "kind": "edge", "direction": [0, 1, 0]}));
    assert_eq!(vertical.as_array().unwrap().len(), 6);
    assert!(s.run("findComponents", json!({"args": [m], "kind": "face", "direction": [0, 1, 0]})).is_err());
}

#[test]
fn mesh_check_passes_a_closed_cube() {
    let mut s = Session::new();
    let m = query_cube(&mut s);
    let ok = run(&mut s, "meshCheck", json!({"args": [m]}));
    assert_eq!(ok["ok"], true);
    assert_eq!(ok["shells"], 1);
    assert!(ok["issues"].as_array().unwrap().is_empty());
}

#[test]
fn select_components_where_selects_and_feeds_mesh_edits_mesh_info_reports_health() {
    let mut s = Session::new();
    let m = query_cube(&mut s);
    run(&mut s, "setSymmetry", json!({"args": [m], "axis": "x"}));
    let sel = run(&mut s, "selectComponentsWhere", json!({"args": [m], "kind": "face", "normal": [0, 0, 1], "side": "+x"}));
    assert_eq!(sel.as_array().unwrap().len(), 1);
    assert_eq!(run(&mut s, "selectMode", json!({})), "face");
    run(&mut s, "polyExtrude", json!({"thickness": 5}));
    let info = run(&mut s, "meshInfo", json!({"args": [m]}));
    // Symmetry mirrors the face; the pair touches the seam, so it extrudes as one region: 10 faces + 6 sides.
    assert_eq!(info["faces"], 16);
    assert_eq!(info["closed"], true);
    assert_eq!(info["shells"], 1);
    assert_eq!(info["quads"], 16);
    assert_eq!(info["symmetry"], "x");
    assert!(approx(info["worldBoundsMax"][2].as_f64().unwrap(), 20.0));

    run(&mut s, "selectComponentsWhere", json!({"args": [m], "kind": "face", "normal": [0, 0, 1], "deselect": true}));
    assert!(run(&mut s, "componentSelection", json!({})).as_object().is_none_or(|o| o.is_empty()));
}

#[test]
fn mesh_check_finds_floating_parts_and_shell_selects_one() {
    let mut s = Session::new();
    let m = query_cube(&mut s);
    let ok = run(&mut s, "meshCheck", json!({"args": [m]}));
    assert_eq!(ok["ok"], true);
    assert_eq!(ok["shells"], 1);
    assert!(ok["issues"].as_array().unwrap().is_empty());

    let ball = run(&mut s, "polyCube", json!({"width": 4, "height": 4, "depth": 4, "position": [0, 20, 0]}))[0].clone();
    let both = run(&mut s, "polyCombine", json!({"args": [m, ball], "name": "Both"}));
    let bad = run(&mut s, "meshCheck", json!({"args": [both]}));
    assert_eq!(bad["ok"], false);
    assert_eq!(bad["shells"], 2);
    assert_eq!(bad["shellList"].as_array().unwrap().len(), 2);
    assert_eq!(bad["shellList"][0]["faces"], 10);
    assert_eq!(bad["shellList"][1]["faces"], 6);
    assert!(approx(bad["shellList"][1]["box"][0][1].as_f64().unwrap(), 18.0));
    assert_eq!(run(&mut s, "meshCheck", json!({"args": [both], "expectShells": 2}))["ok"], true);

    let small = run(&mut s, "findComponents", json!({"args": [both], "kind": "face", "shell": 1}));
    assert_eq!(small.as_array().unwrap().len(), 6);
    let brief = run(&mut s, "componentInfo", json!({"args": [both], "kind": "face", "ids": small, "fields": ["id", "center"]}));
    assert_eq!(brief.as_array().unwrap().len(), 6);
    assert_eq!(brief[0].as_object().unwrap().len(), 2);
    assert!(brief[0]["center"][1].as_f64().unwrap() > 17.0);
}

#[test]
fn with_symmetry_scaling_a_pair_clear_of_the_plane_keeps_each_part_in_place() {
    let mut s = Session::new();
    let m = run(&mut s, "polyCube", json!({"width": 20, "height": 10, "depth": 30, "subdivisionsX": 4}))[0].clone();
    run(&mut s, "setSymmetry", json!({"args": [m], "axis": "x"}));
    run(&mut s, "selectComponentsWhere", json!({"args": [m], "kind": "face", "normal": [0, -1, 0], "side": "+x", "extreme": "+x"}));
    run(&mut s, "polyExtrude", json!({"thickness": 5}));
    run(&mut s, "scaleComponents", json!({"by": [0.5, 1, 0.5]}));
    let caps = run(&mut s, "componentInfo", json!({"args": [m], "kind": "face", "fields": ["center", "area"]}));
    assert_eq!(caps.as_array().unwrap().len(), 2);
    for c in caps.as_array().unwrap() {
        assert!(approx(c["center"][0].as_f64().unwrap().abs(), 7.5));
        assert!(approx(c["area"].as_f64().unwrap(), 5.0 * 30.0 * 0.25));
    }
}

#[test]
fn with_symmetry_the_side_clicked_holds_the_manipulator() {
    let mut s = Session::new();
    let m = run(&mut s, "polyCube", json!({"width": 20, "height": 10, "depth": 30, "subdivisionsX": 4}))[0].clone();
    run(&mut s, "setSymmetry", json!({"args": [m], "axis": "x"}));
    for side in ["-x", "+x"] {
        let face = run(&mut s, "findComponents", json!({"args": [m], "kind": "face", "normal": [0, -1, 0], "side": side, "extreme": side}));
        assert_eq!(face.as_array().unwrap().len(), 1);
        run(&mut s, "selectComponents", json!({"args": [m], "faces": face, "symmetric": true}));
        assert_eq!(run(&mut s, "componentInfo", json!({"args": [m], "kind": "face", "fields": ["id"]})).as_array().unwrap().len(), 2);
        let x = transform::symmetric_selection_center(&mut s.graph).x;
        assert!(approx(x, if side.starts_with('-') { -7.5 } else { 7.5 }));
        let (node, ids) = components::selected_all(&s.graph, ComponentMode::Face).iter().next().map(|(k, v)| (*k, v.clone())).unwrap();
        let want: Vec<u32> = face.as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u32).collect();
        assert_eq!(transform::driving_side(&mut s.graph, node, ComponentMode::Face, &ids, None), want);
    }
}

#[test]
fn stl_and_ply_export_and_import() {
    let dir = std::env::temp_dir().join("aztek_tests");
    std::fs::create_dir_all(&dir).unwrap();
    for ext in ["stl", "ply"] {
        let mut s = Session::new();
        run(&mut s, "polyCylinder", json!({"radius": 50.0, "height": 100.0, "name": "Can", "position": [0, 50, 0]}));
        let path = dir.join(format!("can.{ext}")).to_string_lossy().to_string();
        let up = if ext == "stl" { "Stl" } else { "Ply" };
        run(&mut s, &format!("export{up}"), json!({"args": [path, "Can"]}));
        run(&mut s, "newScene", json!({}));
        let names = run(&mut s, &format!("import{up}"), json!({"args": [path]}));
        assert_eq!(names.as_array().unwrap().len(), 1);
        let info = run(&mut s, "meshInfo", json!({"args": [names[0]]}));
        assert!(approx_eps(info["boundsMin"][1].as_f64().unwrap(), 0.0, 1e-4), "{ext}");
        assert!(approx_eps(info["boundsMax"][1].as_f64().unwrap(), 100.0, 1e-4), "{ext}");
    }
}

#[test]
fn stl_units_and_up_axis() {
    let path = std::env::temp_dir().join("aztek_tests").join("box.stl").to_string_lossy().to_string();
    std::fs::create_dir_all(std::env::temp_dir().join("aztek_tests")).unwrap();
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"name": "Box", "width": 10, "height": 20, "depth": 30, "position": [0, 10, 0]}));
    run(&mut s, "exportStl", json!({"args": [path, "Box"]}));
    run(&mut s, "newScene", json!({}));
    // Read as centimetres and Y up: the file's millimetres come in ten times larger, its Z up lying along Y.
    let names = run(&mut s, "importStl", json!({"args": [path], "unit": "cm", "upAxis": "y"}));
    let info = run(&mut s, "meshInfo", json!({"args": [names[0]]}));
    assert!(approx(info["boundsMax"][2].as_f64().unwrap() - info["boundsMin"][2].as_f64().unwrap(), 200.0));
    assert!(approx(info["boundsMax"][1].as_f64().unwrap() - info["boundsMin"][1].as_f64().unwrap(), 300.0));
}
