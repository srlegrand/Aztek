//! Tests: booleans.

use std::collections::BTreeMap;

use aztek_geo::mesh::validate;
use aztek_geo::primitives::{make_cube, make_cylinder, make_sphere, CubeOptions, CylinderOptions, SphereOptions};
use aztek_geo::{names, Index, Mesh};
use aztek_scene::mesh_boolean::{mesh_boolean, BooleanOperand, BooleanOptions, BooleanResult, BOOLEAN_SEAM_ATTR};
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::Session;
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

fn side_counts(m: &Mesh) -> BTreeMap<Index, usize> {
    let mut sizes = BTreeMap::new();
    for f in 0..m.face_capacity() {
        if m.face_valid(f) {
            *sizes.entry(m.face_size(f)).or_insert(0) += 1;
        }
    }
    sizes
}

fn largest_face(m: &Mesh) -> Index {
    *side_counts(m).keys().last().unwrap()
}

/// N-gons that don't lie flat: those would shade badly, flat ones are fine.
fn bent_ngons(m: &Mesh) -> usize {
    (0..m.face_capacity())
        .filter(|&f| m.face_valid(f) && m.face_size(f) > 4)
        .filter(|&f| {
            let (n, c) = (m.face_normal(f), m.face_centroid(f));
            m.face_points(f).iter().any(|&p| (m.position(p) - c).dot(n).abs() > 1e-3)
        })
        .count()
}

/// Smallest corner angle in degrees: slivers show up here.
fn thinnest_corner(m: &Mesh) -> f64 {
    let mut worst = 180f64;
    for c in 0..m.corner_capacity() {
        if !m.corner_valid(c) {
            continue;
        }
        let p = m.position(m.corner_point(c));
        let a = m.position(m.corner_point(m.corner_prev(c))) - p;
        let b = m.position(m.corner_next_point(c)) - p;
        worst = worst.min(a.normalize().dot(b.normalize()).clamp(-1.0, 1.0).acos().to_degrees());
    }
    worst
}

fn cut(base: &Mesh, cutter: &Mesh, xf: DMat4, opts: BooleanOptions) -> BooleanResult {
    let ops = [BooleanOperand { mesh: base, to_world: DMat4::IDENTITY }, BooleanOperand { mesh: cutter, to_world: xf }];
    mesh_boolean(&ops, &DMat4::IDENTITY, &opts)
}

fn cube100(sub: i32) -> Mesh {
    make_cube(&CubeOptions { width: 100.0, height: 100.0, depth: 100.0, subdiv_x: sub, subdiv_y: sub, subdiv_z: sub, ..Default::default() })
}

fn drill() -> Mesh {
    make_cylinder(&CylinderOptions { radius: 20.0, height: 300.0, ..Default::default() })
}

#[test]
fn seam_cleanup_leaves_only_flat_ngons_along_the_cut() {
    let (plate, drill) = (cube100(1), drill());
    let raw = cut(&plate, &drill, DMat4::IDENTITY, BooleanOptions { cleanup: false, split_ngons: false, ..Default::default() });
    assert!(raw.ok(), "{}", raw.error);
    assert!(largest_face(&raw.mesh) > 4);

    let r = cut(&plate, &drill, DMat4::IDENTITY, BooleanOptions::default());
    assert!(r.ok());
    assert!(r.mesh.validate(validate::MANIFOLD).is_empty());
    assert!(r.seam_edges > 0);
    assert_eq!(bent_ngons(&r.mesh), 0);
    // Each cap is cut by spokes from its corners into four flat pieces, not fanned into slivers.
    assert_eq!(side_counts(&r.mesh).get(&3).copied().unwrap_or(0), 0, "{:?}", side_counts(&r.mesh));
    assert!(largest_face(&r.mesh) < largest_face(&raw.mesh) / 2);
}

#[test]
fn a_subdivided_block_keeps_its_untouched_quads_and_splits_the_rest() {
    let r = cut(&cube100(4), &drill(), DMat4::from_translation(DVec3::new(7.0, 0.0, 3.0)), BooleanOptions::default());
    assert!(r.ok());
    let sizes = side_counts(&r.mesh);
    assert!(r.mesh.validate(validate::MANIFOLD).is_empty());
    assert_eq!(bent_ngons(&r.mesh), 0);
    assert!(thinnest_corner(&r.mesh) > 9.0, "{}", thinnest_corner(&r.mesh));
    // The block's side faces never meet the drill and come back as their original quads.
    let (q, t) = (sizes.get(&4).copied().unwrap_or(0), sizes.get(&3).copied().unwrap_or(0));
    assert!(q >= 4 * 16, "{sizes:?}");
    assert!(t < q);
}

#[test]
fn keeps_the_bases_hard_edges_and_marks_the_seam() {
    let mut block = cube100(1);
    block.edge_attrs_mut().get_or_add::<bool>(names::HARD).fill(true);
    let ball = make_sphere(&SphereOptions { radius: 40.0, ..Default::default() });
    let r = cut(&block, &ball, DMat4::from_translation(DVec3::new(50.0, 50.0, 0.0)), BooleanOptions::default());
    assert!(r.ok());
    assert!(r.mesh.validate(validate::MANIFOLD).is_empty());
    assert_eq!(bent_ngons(&r.mesh), 0);
    let seam = r.mesh.edge_attrs().get::<bool>(BOOLEAN_SEAM_ATTR);
    assert!(!seam.is_empty());
    assert_eq!(seam.iter().filter(|&&s| s).count() as Index, r.seam_edges);
    // The cube's far corner edges are untouched and stay hard.
    let hard = r.mesh.edge_attrs().get::<bool>(names::HARD);
    let hard_far = (0..r.mesh.edge_capacity())
        .filter(|&e| {
            let (a, b) = (r.mesh.position(r.mesh.edge_point(e, 0)), r.mesh.position(r.mesh.edge_point(e, 1)));
            a.x < -49.0 && b.x < -49.0 && hard[e as usize]
        })
        .count();
    assert_eq!(hard_far, 4);
}

#[test]
fn fillet_rounds_the_seam_into_a_closed_strip() {
    let (plate, drill) = (cube100(1), drill());
    let sharp = cut(&plate, &drill, DMat4::IDENTITY, BooleanOptions::default());
    let round = cut(&plate, &drill, DMat4::IDENTITY, BooleanOptions { fillet_width: 3.0, fillet_segments: 3, ..Default::default() });
    assert!(round.ok());
    assert!(round.mesh.validate(validate::MANIFOLD).is_empty());
    assert!(round.mesh.num_faces() > sharp.mesh.num_faces() + round.seam_edges);
}

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn first(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn mesh_of(s: &mut Session, name: &str) -> Mesh {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap();
    (**s.graph.node(aztek_scene::components::as_mesh(&s.graph, id).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap()).clone()
}

#[test]
fn poly_boolean_cleans_up_and_adds_weighted_normals() {
    let mut s = Session::new();
    let bx = first(run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "name": "Block", "generator": false})));
    let drill = first(run(&mut s, "polyCylinder", json!({"name": "Drill", "generator": false, "radius": 20, "height": 300})));
    run(&mut s, "polyBoolean", json!({"args": [bx, drill]}));
    let m = mesh_of(&mut s, &bx);
    assert!(m.validate(validate::MANIFOLD).is_empty());
    assert_eq!(bent_ngons(&m), 0);
    let stack = run(&mut s, "modifiers", json!({"args": [bx]}));
    assert!(stack.to_string().contains("weightedNormals"), "{stack}");
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "objExists", json!({"args": [drill]})), true);
}

#[test]
fn live_boolean_follows_the_cutter() {
    let mut s = Session::new();
    let bx = first(run(
        &mut s,
        "polyCube",
        json!({"width": 100.0, "height": 100.0, "depth": 100.0, "name": "Block", "generator": false, "subdivisionsX": 6, "subdivisionsY": 6, "subdivisionsZ": 6}),
    ));
    let drill = first(run(&mut s, "polyCylinder", json!({"name": "Drill", "generator": false, "radius": 15, "height": 300})));
    run(&mut s, "polyBoolean", json!({"args": [bx, drill], "live": true}));
    assert_eq!(run(&mut s, "objExists", json!({"args": [drill]})), true);
    let stack = run(&mut s, "modifiers", json!({"args": [bx]}));
    assert!(stack.to_string().contains("booleanModifier"), "{stack}");
    let before = mesh_of(&mut s, &bx);
    assert!(before.validate(validate::MANIFOLD).is_empty());
    let cut_faces = before.num_faces();
    assert!(cut_faces > 6 * 36);

    // Moving the cutter off the block leaves it whole; moving it back cuts again.
    run(&mut s, "move", json!({"args": [drill], "by": [500, 0, 0]}));
    assert_eq!(mesh_of(&mut s, &bx).num_faces(), 6 * 36);
    run(&mut s, "move", json!({"args": [drill], "by": [-500, 0, 0]}));
    assert_eq!(mesh_of(&mut s, &bx).num_faces(), cut_faces);
}
