//! Tests: extrude.

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use aztek_geo::mesh::validate;
use aztek_geo::ops::{extrude_faces, inset_faces, ExtrudeOptions, InsetOptions};
use aztek_geo::primitives::{make_cube, CubeOptions};
use aztek_geo::{Index, Mesh, INVALID};
use aztek_ops::{execute, CallSource};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::last_op::{self, Change};
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

fn rerun(s: &mut Session, flags: Value) {
    last_op::rerun(s, &flags, CallSource::Script, false).unwrap();
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn mesh_of(s: &mut Session, name: &str) -> Mesh {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap();
    (**s.graph.node(aztek_scene::components::as_mesh(&s.graph, id).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap()).clone()
}

fn name(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

/// Faces lying on top of another face: the same corner positions in any order.
fn coincident_faces(m: &Mesh) -> i32 {
    let mut seen = BTreeSet::new();
    let mut dupes = 0;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let mut key = Vec::new();
        m.for_each_face_corner(f, |c| {
            let p = m.position(m.corner_point(c));
            key.push([(p.x * 1e4).round() as i64, (p.y * 1e4).round() as i64, (p.z * 1e4).round() as i64]);
        });
        key.sort();
        if !seen.insert(key) {
            dupes += 1;
        }
    }
    dupes
}

fn zero_area_faces(m: &Mesh) -> i32 {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_area(f) < 1e-9).count() as i32
}

fn face_facing(m: &Mesh, dir: DVec3) -> Index {
    let mut best = INVALID;
    let mut best_dot = -2.0;
    for f in 0..m.face_capacity() {
        if m.face_valid(f) && m.face_normal(f).dot(dir) > best_dot {
            best_dot = m.face_normal(f).dot(dir);
            best = f;
        }
    }
    best
}

fn size_sum(m: &Mesh) -> f64 {
    let s = m.bounds().size();
    s.x + s.y + s.z
}

#[test]
fn extrude_thickness_pushes_the_cap_out_offset_insets_it() {
    let mut cube = make_cube(&CubeOptions::default());
    let top = face_facing(&cube, DVec3::Y);
    let o = ExtrudeOptions { offset: 20.0, inset: 10.0, ..Default::default() };
    let r = extrude_faces(&mut cube, &[top], &o);
    assert_eq!(r.sides.len(), 4);
    assert_eq!(cube.num_faces(), 10);
    assert_eq!(cube.num_points(), 12);
    assert!(approx(cube.face_area(top), 80.0 * 80.0));
    assert!(approx(cube.bounds().max.y, 70.0));
    assert_eq!(coincident_faces(&cube), 0);
    assert_eq!(zero_area_faces(&cube), 0);
}

#[test]
fn inset_leaves_no_coincident_or_zero_area_faces() {
    let mut cube = make_cube(&CubeOptions::default());
    let top = face_facing(&cube, DVec3::Y);
    inset_faces(&mut cube, &[top], &InsetOptions { thickness: 10.0, ..Default::default() });
    assert_eq!(cube.num_faces(), 10);
    assert_eq!(coincident_faces(&cube), 0);
    assert_eq!(zero_area_faces(&cube), 0);

    // A region of four faces: one inner region plus its rim.
    let mut grid = make_cube(&CubeOptions { subdiv_x: 2, subdiv_y: 1, subdiv_z: 2, ..Default::default() });
    let tops: Vec<Index> = (0..grid.face_capacity()).filter(|&f| grid.face_valid(f) && grid.face_normal(f).y > 0.9).collect();
    assert_eq!(tops.len(), 4);
    let before = grid.num_faces();
    inset_faces(&mut grid, &tops, &InsetOptions { thickness: 5.0, ..Default::default() });
    assert_eq!(grid.num_faces(), before + 8);
    assert_eq!(coincident_faces(&grid), 0);
    assert_eq!(zero_area_faces(&grid), 0);
}

fn big_cube(s: &mut Session) -> String {
    name(&run(s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0}))[0])
}

#[test]
fn poly_inset_and_poly_extrude_through_the_command_and_adjust_last_op_path() {
    let mut s = Session::new();
    let cube = big_cube(&mut s);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run_ui(&mut s, "polyInset", json!({"thickness": 5}));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 10);
    assert_eq!(coincident_faces(&mesh_of(&mut s, &cube)), 0);
    assert!(last_op::live(&s));
    rerun(&mut s, json!({"thickness": 10}));
    rerun(&mut s, json!({"thickness": 12}));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 10);
    assert_eq!(coincident_faces(&mesh_of(&mut s, &cube)), 0);

    run(&mut s, "newScene", json!({}));
    let cube = big_cube(&mut s);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run_ui(&mut s, "polyExtrude", json!({"thickness": 20, "offset": 10}));
    let m = mesh_of(&mut s, &cube);
    assert_eq!(m.num_faces(), 10);
    assert_eq!(coincident_faces(&m), 0);
    assert!(last_op::live(&s));
    rerun(&mut s, json!({"thickness": 30}));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 10);
    assert!(approx(size_sum(&mesh_of(&mut s, &cube)), 330.0));
    run(&mut s, "undo", json!({}));
    assert!(approx(size_sum(&mesh_of(&mut s, &cube)), 320.0));
    run(&mut s, "undo", json!({}));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 6);
}

#[test]
fn poly_extrude_duplicate_puts_the_extruded_faces_in_a_new_mesh() {
    let mut s = Session::new();
    let cube = name(&run(&mut s, "polyCube", json!({}))[0]);
    run(&mut s, "move", json!({"args": [cube], "to": [0, 0, 300]}));
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    let made = run_ui(&mut s, "polyExtrude", json!({"thickness": 20, "duplicate": true}));
    assert_eq!(made.as_array().unwrap().len(), 1);
    let copy = name(&made[0]);
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 6);
    assert_eq!(mesh_of(&mut s, &copy).num_faces(), 1);
    let copy_id = s.graph.find(&copy).unwrap().unwrap();
    assert!(approx(s.graph.node(copy_id).unwrap().vec3(Transform::TRANSLATE).z, 300.0));
    // The cap stays selected, so the handles and the last-op panel work on the copy.
    assert_eq!(components::mode(&s.graph), ComponentMode::Face);
    let sel = components::selected_all(&s.graph, ComponentMode::Face);
    assert_eq!(sel.len(), 1);
    assert_eq!(*sel.keys().next().unwrap(), aztek_scene::components::as_mesh(&s.graph, copy_id).unwrap());

    assert!(last_op::live(&s));
    rerun(&mut s, json!({"thickness": 30, "keepSides": true}));
    let sel = components::selected_all(&s.graph, ComponentMode::Face).clone();
    assert_eq!(sel.len(), 1);
    let again_path = s.graph.path(*sel.keys().next().unwrap());
    assert_eq!(mesh_of(&mut s, &again_path).num_faces(), 5);
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 6);
    // Back to the first setting, then past the extrude.
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    assert!(s.graph.find(&again_path).unwrap().is_none());
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 6);
}

#[test]
fn adjust_last_op_stays_reachable_through_selection_changes() {
    let mut s = Session::new();
    let cube = big_cube(&mut s);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run_ui(&mut s, "polyInset", json!({"thickness": 5}));
    let changes: Arc<Mutex<Vec<Change>>> = Arc::default();
    let c2 = changes.clone();
    let listener = last_op::add_listener(&mut s, Arc::new(move |c| c2.lock().unwrap().push(c)));
    let last = |c: &Arc<Mutex<Vec<Change>>>| *c.lock().unwrap().last().unwrap();

    run_ui(&mut s, "selectComponents", json!({"args": [cube], "faces": [1]}));
    run_ui(&mut s, "selectComponents", json!({"args": [cube], "faces": [2]}));
    assert!(!last_op::live(&s));
    assert!(last_op::reachable(&s));
    assert!(changes.lock().unwrap().is_empty());

    run(&mut s, "undo", json!({}));
    assert!(last_op::reachable(&s));
    run(&mut s, "undo", json!({}));
    assert!(last_op::live(&s));
    assert_eq!(changes.lock().unwrap().len(), 1);
    assert_eq!(last(&changes), Change::Restored);
    run(&mut s, "redo", json!({}));
    run(&mut s, "redo", json!({}));

    rerun(&mut s, json!({"thickness": 10}));
    assert!(last_op::live(&s));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 10);
    assert_eq!(s.graph.undo_stack().redo_count(), 0);
    run(&mut s, "undo", json!({}));
    assert!(last_op::live(&s));
    assert_eq!(last_op::current(&s).unwrap().call["thickness"], 5);
    run(&mut s, "undo", json!({}));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 6);
    assert!(!last_op::reachable(&s));
    assert_eq!(last(&changes), Change::Other);

    run(&mut s, "redo", json!({}));
    assert!(last_op::live(&s));
    assert_eq!(last(&changes), Change::Restored);
    run_ui(&mut s, "selectComponents", json!({"args": [cube], "faces": [3]}));
    run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0}));
    assert!(!last_op::reachable(&s));
    assert_eq!(last(&changes), Change::Other);
    run(&mut s, "undo", json!({}));
    assert!(last_op::reachable(&s));
    assert_eq!(last(&changes), Change::Restored);
    last_op::remove_listener(&mut s, listener);
}

#[test]
fn poly_inset_on_every_face_of_a_symmetric_cube_adds_rims_without_duplicates() {
    for symmetric in [false, true] {
        for individual in [false, true] {
            let mut s = Session::new();
            let cube = name(
                &run(
                    &mut s,
                    "polyCube",
                    json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 2, "subdivisionsY": 2, "subdivisionsZ": 2}),
                )[0],
            );
            if symmetric {
                run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
            }
            let count = mesh_of(&mut s, &cube).num_faces();
            for f in 0..count {
                run(&mut s, "selectComponents", json!({"args": [cube], "faces": [f, (f + 1) % count]}));
                run(&mut s, "polyInset", json!({"thickness": 5, "individual": individual}));
                let m = mesh_of(&mut s, &cube);
                assert_eq!(coincident_faces(&m), 0, "{f} {symmetric} {individual}");
                assert_eq!(zero_area_faces(&m), 0, "{f} {symmetric} {individual}");
                run(&mut s, "undo", json!({}));
            }
        }
    }
}

#[test]
fn poly_bevel_on_faces_bevels_the_border_of_the_selection_not_the_edges_inside_it() {
    let mut s = Session::new();
    let cyl = name(&run(&mut s, "polyCylinder", json!({"radius": 50.0, "height": 100.0, "subdivisionsAxis": 16, "subdivisionsCaps": 3}))[0]);
    let before = mesh_of(&mut s, &cyl);
    let top: Vec<Index> = (0..before.face_capacity()).filter(|&f| before.face_valid(f) && before.face_normal(f).y > 0.99).collect();
    assert!(top.len() > 16);
    run(&mut s, "selectMode", json!({"args": ["face"]}));
    run(&mut s, "selectComponents", json!({"args": [cyl], "faces": top}));
    run(&mut s, "polyBevel", json!({"offset": 3.0}));
    let m = mesh_of(&mut s, &cyl);
    assert!(m.validate(validate::DEFAULT | validate::DEGENERATE).is_empty());
    // One ring of chamfer faces round the rim; the cap's own faces keep their count.
    assert_eq!(m.num_faces(), before.num_faces() + 16);
    let cap_faces = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_normal(f).y > 0.99).count();
    assert_eq!(cap_faces, top.len());
}

#[test]
fn poly_bevel_offset_adjusts_through_adjust_last_op_and_keeps_edge_mode() {
    let mut s = Session::new();
    let cube = big_cube(&mut s);
    run(&mut s, "selectMode", json!({"args": ["edge"]}));
    run(&mut s, "selectComponents", json!({"args": [cube], "edges": [0]}));
    run_ui(&mut s, "polyBevel", json!({"offset": 2}));
    let spread = |s: &mut Session| {
        let m = mesh_of(s, &cube);
        let mut lo: f64 = 1e9;
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                for i in 0..3 {
                    let v = m.position(p)[i].abs();
                    if v > 1e-6 && v < 49.9 {
                        lo = lo.min(v);
                    }
                }
            }
        }
        lo
    };
    assert!(approx(spread(&mut s), 48.0));
    assert_eq!(components::mode(&s.graph), ComponentMode::Edge);
    assert!(last_op::live(&s));
    rerun(&mut s, json!({"offset": 10}));
    assert!(approx(spread(&mut s), 40.0));
    // Past what the cube allows, the recorded offset stops at the clamp: the faces beside the edge close entirely.
    rerun(&mut s, json!({"offset": 500}));
    let limit = last_op::current(&s).unwrap().call["offset"].as_f64().unwrap();
    assert!(approx(limit, 100.0));
    let m = mesh_of(&mut s, &cube);
    assert!(m.validate(validate::DEFAULT | validate::DEGENERATE).is_empty());
    assert_eq!(m.num_faces(), 5);
    run(&mut s, "undo", json!({}));
    assert!(approx(spread(&mut s), 40.0));
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    assert_eq!(components::mode(&s.graph), ComponentMode::Edge);
    let id = s.graph.find(&cube).unwrap().unwrap();
    assert_eq!(components::selected(&s.graph, id, ComponentMode::Edge), &[0]);
}

#[test]
fn edits_run_in_a_component_mode_stay_in_it() {
    let mut s = Session::new();
    let cube = big_cube(&mut s);
    run(&mut s, "selectMode", json!({"args": ["face"]}));
    run(&mut s, "selectComponentsWhere", json!({"args": [cube], "kind": "face", "extreme": "+z"}));
    run(&mut s, "selectComponentsWhere", json!({"args": [cube], "kind": "face", "extreme": "-z", "add": true}));
    run(&mut s, "deleteComponents", json!({}));
    run(&mut s, "selectComponentsWhere", json!({"args": [cube], "kind": "edge", "border": true}));
    assert_eq!(components::mode(&s.graph), ComponentMode::Edge);
    run(&mut s, "polyBridge", json!({}));
    assert_eq!(mesh_of(&mut s, &cube).num_faces(), 8);
    assert_eq!(components::mode(&s.graph), ComponentMode::Edge);
    run(&mut s, "selectComponents", json!({"args": [cube], "edges": [0]}));
    run(&mut s, "polyCollapse", json!({}));
    assert_eq!(components::mode(&s.graph), ComponentMode::Edge);
}

#[test]
fn poly_target_weld_vertex_into_target_halfway_edges_and_across_symmetry() {
    let has_point = |m: &Mesh, p: DVec3| (0..m.point_capacity()).any(|i| m.point_valid(i) && m.position(i).distance(p) < 1e-6);
    let mut s = Session::new();
    let cube = big_cube(&mut s);
    let before = mesh_of(&mut s, &cube);
    let (a, b) = (before.edge_point(0, 0), before.edge_point(0, 1));
    run(&mut s, "polyTargetWeld", json!({"mesh": cube, "source": a, "target": b}));
    assert_eq!(mesh_of(&mut s, &cube).num_points(), 7);
    assert!(has_point(&mesh_of(&mut s, &cube), before.position(b)));
    assert!(!has_point(&mesh_of(&mut s, &cube), before.position(a)));
    run(&mut s, "undo", json!({}));
    assert_eq!(mesh_of(&mut s, &cube).num_points(), 8);

    run(&mut s, "polyTargetWeld", json!({"mesh": cube, "source": a, "target": b, "center": true}));
    assert!(has_point(&mesh_of(&mut s, &cube), (before.position(a) + before.position(b)) * 0.5));
    run(&mut s, "undo", json!({}));

    let mut apart = INVALID;
    for e in 1..before.edge_capacity() {
        if apart != INVALID {
            break;
        }
        let (p, q) = (before.edge_point(e, 0), before.edge_point(e, 1));
        if before.edge_valid(e) && p != a && p != b && q != a && q != b {
            apart = e;
        }
    }
    assert_ne!(apart, INVALID);
    run(&mut s, "polyTargetWeld", json!({"mesh": cube, "source": 0, "target": apart, "edge": true}));
    assert_eq!(mesh_of(&mut s, &cube).num_points(), 6);
    run(&mut s, "undo", json!({}));

    // With X symmetry, welding a +X vertex to its neighbour welds the -X pair too; welding across the plane meets on
    // it.
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    let (mut px, mut nx, mut py) = (INVALID, INVALID, INVALID);
    for i in 0..before.point_capacity() {
        let p = before.position(i);
        if p.x > 0.0 && p.y > 0.0 && p.z > 0.0 {
            px = i;
        }
        if p.x < 0.0 && p.y > 0.0 && p.z > 0.0 {
            nx = i;
        }
        if p.x > 0.0 && p.y < 0.0 && p.z > 0.0 {
            py = i;
        }
    }
    run(&mut s, "polyTargetWeld", json!({"mesh": cube, "source": px, "target": py}));
    assert_eq!(mesh_of(&mut s, &cube).num_points(), 6);
    run(&mut s, "undo", json!({}));
    run(&mut s, "polyTargetWeld", json!({"mesh": cube, "source": px, "target": nx}));
    assert_eq!(mesh_of(&mut s, &cube).num_points(), 7);
    let p = before.position(px);
    assert!(has_point(&mesh_of(&mut s, &cube), DVec3::new(0.0, p.y, p.z)));
}

fn rounded(m: &Mesh) -> BTreeSet<[i64; 3]> {
    (0..m.point_capacity())
        .filter(|&p| m.point_valid(p))
        .map(|p| {
            let q = m.position(p);
            [(q.x * 1e3).round() as i64, (q.y * 1e3).round() as i64, (q.z * 1e3).round() as i64]
        })
        .collect()
}

/// Points without a partner mirrored across x = 0.
fn unmirrored(m: &Mesh) -> usize {
    let at = rounded(m);
    at.iter().filter(|k| !at.contains(&[-k[0], k[1], k[2]])).count()
}

/// Points without a partner mirrored across y = 0.
fn unmirrored_y(m: &Mesh) -> usize {
    let at = rounded(m);
    at.iter().filter(|k| !at.contains(&[k[0], -k[1], k[2]])).count()
}

#[test]
fn poly_grid_fill_with_symmetry_fills_both_sides_alike() {
    for size in [0.4, 1.0] {
        for shape in ["side", "top", "fan"] {
            let mut s = Session::new();
            let cube = name(
                &run(
                    &mut s,
                    "polyCube",
                    json!({"width": 120.0, "height": 120.0, "depth": 120.0, "subdivisionsX": 6, "subdivisionsY": 6, "subdivisionsZ": 6}),
                )[0],
            );
            run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
            let m = mesh_of(&mut s, &cube);
            let mut faces = Vec::new();
            let mut side = INVALID;
            for f in 0..m.face_capacity() {
                if !m.face_valid(f) {
                    continue;
                }
                let mut c = DVec3::ZERO;
                m.for_each_face_corner(f, |fc| c += m.position(m.corner_point(fc)) / 4.0);
                if shape == "top" && c.y > 59.0 && c.x > 10.0 && c.x < 50.0 && c.z > -30.0 && c.z < 30.0 {
                    faces.push(f);
                }
                if shape == "side" && c.x > 59.0 && c.y > -20.0 && c.y < 40.0 && c.z > -40.0 && c.z < 20.0 {
                    faces.push(f);
                }
                if c.x > 59.0 && (c.y - 10.0).abs() < 1.0 && (c.z - 10.0).abs() < 1.0 {
                    side = f;
                }
            }
            if shape == "fan" {
                assert_ne!(side, INVALID);
                run(&mut s, "selectMode", json!({"args": ["face"]}));
                run(&mut s, "selectComponents", json!({"args": [cube], "faces": [side]}));
                run(&mut s, "polyPoke", json!({}));
                assert_eq!(unmirrored(&mesh_of(&mut s, &cube)), 0);
                let poked = mesh_of(&mut s, &cube);
                let centre = (0..poked.point_capacity())
                    .find(|&p| poked.point_valid(p) && poked.position(p).distance(DVec3::new(60.0, 10.0, 10.0)) < 1e-6)
                    .unwrap();
                run(&mut s, "selectMode", json!({"args": ["vertex"]}));
                run(&mut s, "selectComponents", json!({"args": [cube], "vertices": [centre]}));
            } else {
                assert!(!faces.is_empty());
                run(&mut s, "selectMode", json!({"args": ["face"]}));
                run(&mut s, "selectComponents", json!({"args": [cube], "faces": faces}));
            }
            run(&mut s, "polyGridFill", json!({"gridSize": size}));
            let m = mesh_of(&mut s, &cube);
            assert_eq!(unmirrored(&m), 0, "{size} {shape}");
            assert_eq!(zero_area_faces(&m), 0, "{size} {shape}");
        }
    }
}

#[test]
fn poly_grid_fill_with_symmetry_fills_round_mirrored_fans_alike() {
    for sides in [12, 16, 20, 13] {
        for size in [0.4, 1.0] {
            let mut s = Session::new();
            let cyl = name(&run(&mut s, "polyCylinder", json!({"radius": 50.0, "height": 100.0, "subdivisionsAxis": sides, "capType": "fan"}))[0]);
            run(&mut s, "setSymmetry", json!({"args": [cyl], "axis": "y"}));
            let m = mesh_of(&mut s, &cyl);
            let top = (0..m.point_capacity()).find(|&p| m.point_valid(p) && m.position(p).distance(DVec3::new(0.0, 50.0, 0.0)) < 1e-6).unwrap();
            run(&mut s, "selectMode", json!({"args": ["vertex"]}));
            run(&mut s, "selectComponents", json!({"args": [cyl], "vertices": [top]}));
            run(&mut s, "polyGridFill", json!({"gridSize": size}));
            let m = mesh_of(&mut s, &cyl);
            assert_eq!(unmirrored_y(&m), 0, "{sides} {size}");
            assert_eq!(zero_area_faces(&m), 0, "{sides} {size}");
            assert_eq!(coincident_faces(&m), 0, "{sides} {size}");
        }
    }
}
