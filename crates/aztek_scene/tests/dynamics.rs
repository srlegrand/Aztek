//! Dynamics in the scene: cloth on a collider through time, scrubbing and starting again, particles from an emitter
//! under gravity, a rigid body on the ground, undo, and the commands' answers.

use aztek_scene::{PlugRef, Session};
use glam::DVec3;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn frame(s: &mut Session, f: f64) {
    run(s, "currentTime", json!({"args": [f]}));
    s.graph.evaluate();
}

/// A mesh's points in world space, through its transform.
fn points(s: &mut Session, name: &str) -> Vec<DVec3> {
    let id = s.graph.find(name).unwrap().unwrap();
    let shape = aztek_scene::components::as_mesh(&s.graph, id).unwrap();
    let out = s.graph.node(shape).unwrap().ty().attr_index("outMesh").unwrap();
    let m = s.graph.value(&PlugRef::new(shape, out as i32)).unwrap();
    let mesh = m.as_mesh().unwrap().clone();
    let xf = aztek_scene::dag::transform_of(&s.graph, shape);
    let w = aztek_scene::rig::world(&mut s.graph, xf);
    mesh.positions().iter().map(|p| w.transform_point3(*p)).collect()
}

fn lowest(p: &[DVec3]) -> f64 {
    p.iter().map(|p| p.y).fold(f64::INFINITY, f64::min)
}

fn drape_scene() -> Session {
    let mut s = Session::new();
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 60}));
    run(&mut s, "polySphere", json!({"name": "ball", "radius": 0.5, "sx": 24, "sy": 12}));
    run(&mut s, "polyPlane", json!({"name": "sheet", "width": 2, "height": 2, "sx": 16, "sy": 16}));
    run(&mut s, "move", json!({"args": [0, 1, 0, "sheet"]}));
    frame(&mut s, 1.0);
    let cloth = run(&mut s, "createCloth", json!({"args": ["sheet"]}));
    assert!(cloth[0].as_str().unwrap().starts_with("cloth"), "{cloth}");
    let col = run(&mut s, "collider", json!({"args": ["ball"]}));
    assert!(col[0].as_str().unwrap().starts_with("collider"), "{col}");
    s
}

#[test]
fn cloth_falls_onto_a_collider_and_rests_on_it() {
    let mut s = drape_scene();
    frame(&mut s, 1.0);
    let start = points(&mut s, "sheet");
    assert!((lowest(&start) - 1.0).abs() < 1e-9, "at the start frame the cloth is its input");
    for f in 2..=48 {
        frame(&mut s, f as f64);
    }
    let end = points(&mut s, "sheet");
    assert!(end.iter().all(|p| p.is_finite()));
    assert!(lowest(&end) < 0.0, "the corners hang below the ball: {}", lowest(&end));
    let inside = end.iter().map(|p| p.length()).fold(f64::INFINITY, f64::min);
    assert!(inside > 0.45, "a point went into the ball, {inside} from its centre");
    let top = end.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max);
    assert!(top > 0.45 && top < 0.7, "the middle rests on top: {top}");
    let oos = run(&mut s, "dynInfo", json!({"outOfSimulation": true}));
    assert_eq!(oos, json!(0), "nothing out of the simulation in a plain drape");
}

#[test]
fn scrubbing_back_shows_solved_frames_and_the_start_frame_starts_again() {
    let mut s = drape_scene();
    for f in 1..=20 {
        frame(&mut s, f as f64);
    }
    let at20 = points(&mut s, "sheet");
    for f in 21..=30 {
        frame(&mut s, f as f64);
    }
    // Back to a solved frame: it is shown as it was, nothing solved backwards.
    frame(&mut s, 20.0);
    assert_eq!(points(&mut s, "sheet"), at20);
    // Back to the start: the input again, and solving again from there gives the same frames.
    frame(&mut s, 1.0);
    assert!((lowest(&points(&mut s, "sheet")) - 1.0).abs() < 1e-9);
    for f in 2..=20 {
        frame(&mut s, f as f64);
    }
    let again = points(&mut s, "sheet");
    for (a, b) in again.iter().zip(&at20) {
        assert!((*a - *b).length() < 1e-9, "the same solve gave {a} then {b}");
    }
}

#[test]
fn jumping_ahead_solves_one_frame() {
    let mut s = drape_scene();
    frame(&mut s, 1.0);
    frame(&mut s, 2.0);
    let two = points(&mut s, "sheet");
    frame(&mut s, 40.0);
    let jumped = points(&mut s, "sheet");
    // One frame further, not forty: it has barely moved.
    let moved = jumped.iter().zip(&two).map(|(a, b)| (*a - *b).length()).fold(0.0, f64::max);
    assert!(moved < 0.05, "a jump solved {moved} worth of motion");
}

#[test]
fn undo_takes_the_cloth_away() {
    let mut s = drape_scene();
    assert!(s.undo());
    assert!(s.undo());
    for f in 1..=10 {
        frame(&mut s, f as f64);
    }
    assert!((lowest(&points(&mut s, "sheet")) - 1.0).abs() < 1e-9, "without cloth the sheet stays put");
}

#[test]
fn emitted_particles_fall_under_gravity() {
    let mut s = Session::new();
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 60}));
    let made = run(&mut s, "emitter", json!({"type": "omni", "rate": 48, "speed": 1, "name": "spout"}));
    let particles = made[1].as_str().unwrap().to_string();
    let solvers = s.graph.nodes().into_iter().filter(|&id| s.graph.node(id).unwrap().ty().name == "solver").count();
    assert_eq!(solvers, 1);
    let g = run(&mut s, "gravity", json!({"args": [particles.clone()], "magnitude": 9.8}));
    assert!(g[0].as_str().unwrap().starts_with("gravityField"), "{g}");
    for f in 1..=24 {
        frame(&mut s, f as f64);
    }
    let count = run(&mut s, "particle", json!({"args": [particles.clone()], "query": true, "count": true}));
    assert!((40..=50).contains(&count.as_i64().unwrap()), "48 a second for a second: {count}");
    let mean = run(&mut s, "getParticleAttr", json!({"args": [particles.clone()], "attribute": "position"}));
    assert!(mean[1].as_f64().unwrap() < -0.5, "the particles fell: {mean}");
    let all = run(&mut s, "getParticleAttr", json!({"args": [particles], "attribute": "velocity", "array": true}));
    assert_eq!(all.as_array().unwrap().len() as i64, count.as_i64().unwrap() * 3);
}

#[test]
fn rigid_body_lands_on_the_ground_plane() {
    let mut s = Session::new();
    run(&mut s, "playbackOptions", json!({"minTime": 1, "maxTime": 120}));
    run(&mut s, "polyCube", json!({"name": "box"}));
    run(&mut s, "move", json!({"args": [0, 3, 0, "box"]}));
    let r = run(&mut s, "rigidBody", json!({"args": ["box"], "active": true, "standInObject": 1}));
    let solver = s.graph.nodes().into_iter().find(|&id| s.graph.node(id).unwrap().ty().name == "solver").unwrap();
    let path = s.graph.path(solver);
    run(&mut s, "setAttr", json!({"args": [format!("{path}.usePlane"), true]}));
    for f in 1..=96 {
        frame(&mut s, f as f64);
    }
    let ty = run(&mut s, "getAttr", json!({"args": ["box.translateY"]})).as_f64().unwrap();
    assert!((ty - 0.5).abs() < 0.1, "resting on the ground: {ty} ({r})");
    let rx = run(&mut s, "getAttr", json!({"args": ["box.rotateX"]})).as_f64().unwrap();
    assert!(rx.abs() < 5.0, "it stays upright: {rx}");
}

#[test]
fn presets_and_old_command_names() {
    let mut s = drape_scene();
    run(&mut s, "clothPreset", json!({"args": ["sheet"], "preset": "leather"}));
    let b = run(&mut s, "getAttr", json!({"args": ["cloth1.bendResistance"]}));
    assert_eq!(b, json!(2.0));
    run(&mut s, "polyPlane", json!({"name": "flag"}));
    let made = run(&mut s, "nCloth", json!({"args": ["flag"]}));
    assert!(made[0].as_str().unwrap().starts_with("cloth"));
    let made = run(&mut s, "nParticle", json!({"position": [[0, 0, 0], [1, 0, 0]]}));
    let count = run(&mut s, "particle", json!({"args": [made[1].clone()], "query": true, "count": true}));
    assert_eq!(count, json!(2));
}
