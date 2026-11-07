//! Tests: duplicate.

use aztek_ops::{execute, CallSource};
use aztek_scene::duplicate;
use aztek_scene::last_op;
use aztek_scene::repeat::{copy_matrices, delta, step_matrix, Step};
use aztek_scene::transform::Transform;
use aztek_scene::Session;
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn first(v: Value) -> String {
    (if v.is_array() { v[0].clone() } else { v }).as_str().unwrap().to_string()
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6 * (1.0 + a.abs().max(b.abs()))
}

fn near(a: DVec3, b: DVec3) {
    assert!((a - b).length() < 1e-6, "{a:?} vs {b:?}");
}

fn get(s: &mut Session, plug: String) -> f64 {
    run(s, "getAttr", json!({"args": [plug]})).as_f64().unwrap()
}

fn world_pos(s: &mut Session, n: &Value) -> DVec3 {
    s.graph.evaluate();
    let id = s.graph.find(n.as_str().unwrap()).unwrap().unwrap();
    s.graph.node(id).unwrap().matrix(Transform::WORLD_MATRIX).w_axis.truncate()
}

fn kind(s: &mut Session, m: &str) -> String {
    run(s, "instanceInfo", json!({"args": [m]}))["kind"].as_str().unwrap().to_string()
}

fn count(s: &mut Session, ty: &str) -> usize {
    run(s, "ls", json!({"type": ty})).as_array().unwrap().len()
}

/// Full paths, which later copies with the same short names can't make ambiguous.
fn full(s: &Session, made: &Value) -> Vec<String> {
    made.as_array()
        .unwrap()
        .iter()
        .map(|m| s.graph.full_path(s.graph.find(m.as_str().unwrap()).unwrap().unwrap()))
        .collect()
}

#[test]
fn repeat_maths_steps_about_a_pivot_compose_into_runs() {
    let s = Step { rotate: DVec3::new(0.0, 90.0, 0.0), ..Default::default() };
    let step = step_matrix(&s, DVec3::ZERO, &DMat3::IDENTITY);
    let m = copy_matrices(&DMat4::from_translation(DVec3::new(100.0, 0.0, 0.0)), &step, 4);
    assert_eq!(m.len(), 4);
    near(m[0].w_axis.truncate(), DVec3::new(0.0, 0.0, -100.0));
    near(m[1].w_axis.truncate(), DVec3::new(-100.0, 0.0, 0.0));
    near(m[3].w_axis.truncate(), DVec3::new(100.0, 0.0, 0.0));

    // About a pivot: a rotation in place around (10, 0, 0) leaves that point where it is.
    let about = step_matrix(&s, DVec3::new(10.0, 0.0, 0.0), &DMat3::IDENTITY);
    near(about.transform_point3(DVec3::new(10.0, 0.0, 0.0)), DVec3::new(10.0, 0.0, 0.0));

    // In an object's frame the translate runs along its axes.
    let t = Step { translate: DVec3::new(5.0, 0.0, 0.0), ..Default::default() };
    let frame = DMat3::from_rotation_z(90f64.to_radians());
    near(step_matrix(&t, DVec3::ZERO, &frame).w_axis.truncate(), DVec3::new(0.0, 5.0, 0.0));

    let a = DMat4::from_translation(DVec3::new(1.0, 2.0, 3.0));
    let b = DMat4::from_translation(DVec3::new(4.0, 2.0, 3.0)) * DMat4::from_rotation_y(0.3);
    let d = delta(&a, &b);
    near((d * a).w_axis.truncate(), b.w_axis.truncate());
}

#[test]
fn duplicate_count_makes_rows_rings_and_growing_runs() {
    let mut s = Session::new();
    let p = first(run(&mut s, "polyCube", json!({"name": "Pillar"})));
    let made = run(&mut s, "duplicate", json!({"args": [p], "count": 3, "translate": [100, 0, 0]}));
    assert_eq!(made.as_array().unwrap().len(), 3);
    assert!(approx(get(&mut s, format!("{}.tx", made[0].as_str().unwrap())), 100.0));
    assert!(approx(get(&mut s, format!("{}.tx", made[2].as_str().unwrap())), 300.0));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})).as_array().unwrap().len(), 3);

    // A ring about the world origin; the rotate channels keep counting past 180.
    run(&mut s, "newScene", json!({}));
    let sp = first(run(&mut s, "polyCube", json!({"name": "Spoke", "position": [100, 0, 0]})));
    let ring = run(&mut s, "duplicate", json!({"args": [sp], "count": 3, "rotate": [0, 90, 0], "pivot": "world"}));
    assert_eq!(ring.as_array().unwrap().len(), 3);
    near(world_pos(&mut s, &ring[0]), DVec3::new(0.0, 0.0, -100.0));
    near(world_pos(&mut s, &ring[1]), DVec3::new(-100.0, 0.0, 0.0));
    near(world_pos(&mut s, &ring[2]), DVec3::new(0.0, 0.0, 100.0));
    assert!(approx(get(&mut s, format!("{}.ry", ring[2].as_str().unwrap())), 270.0));

    // A point pivot, and scale compounding per copy.
    let grow = run(&mut s, "duplicate", json!({"args": [sp], "count": 2, "scale": [2, 2, 2], "pivot": "point", "pivotPoint": [100, 0, 0]}));
    assert!(approx(get(&mut s, format!("{}.sx", grow[0].as_str().unwrap())), 2.0));
    assert!(approx(get(&mut s, format!("{}.sx", grow[1].as_str().unwrap())), 4.0));
    near(world_pos(&mut s, &grow[1]), DVec3::new(100.0, 0.0, 0.0));

    // Steps along the object's own axes.
    run(&mut s, "newScene", json!({}));
    let r = first(run(&mut s, "polyCube", json!({"name": "Rail"})));
    run(&mut s, "rotate", json!({"args": [r], "by": [0, 0, 90]}));
    let along = run(&mut s, "duplicate", json!({"args": [r], "translate": [10, 0, 0], "space": "object"}));
    near(world_pos(&mut s, &along[0]), DVec3::new(0.0, 10.0, 0.0));

    assert!(s.run("duplicate", json!({"args": [r], "mode": "clone"})).is_err());
    assert!(s.run("duplicate", json!({"args": [r], "pivot": "centre"})).is_err());
}

#[test]
fn duplicate_modes_and_group() {
    let mut s = Session::new();
    let c = first(run(&mut s, "polyCube", json!({"name": "Bolt"})));
    run(&mut s, "addModifier", json!({"args": [c], "kind": "subdivision"}));
    let made = run(&mut s, "duplicate", json!({"args": [c], "count": 2, "mode": "instance", "translate": [10, 0, 0], "group": true}));
    let inst = full(&s, &made);
    assert_eq!(inst.len(), 2);
    assert_eq!(kind(&mut s, &inst[0]), "instance");
    assert_eq!(kind(&mut s, &inst[1]), "instance");
    assert_eq!(inst[0], "|Group|Bolt1");
    near(world_pos(&mut s, &json!(inst[1])), DVec3::new(20.0, 0.0, 0.0));

    let made = run(&mut s, "duplicate", json!({"args": [c], "mode": "reference"}));
    let r = full(&s, &made);
    assert_eq!(kind(&mut s, &r[0]), "reference");

    // A plain copy of an instance is a mesh of its own with a live stack.
    let made = run(&mut s, "duplicate", json!({"args": [inst[0]]}));
    let copy = full(&s, &made);
    assert_eq!(kind(&mut s, &copy[0]), "none");
    assert_eq!(run(&mut s, "modifiers", json!({"args": [copy[0]]})).as_array().unwrap().len(), 1);

    // History shared within one call is copied once, so a source and its reference duplicated together stay related.
    let gens = count(&mut s, "polyCube");
    let made = run(&mut s, "duplicate", json!({"args": [c, r[0]]}));
    let both = full(&s, &made);
    assert_eq!(count(&mut s, "polyCube"), gens + 1);
    assert_eq!(kind(&mut s, &both[0]), "source");
    assert_eq!(kind(&mut s, &both[1]), "reference");
}

#[test]
fn duplicate_is_adjustable_count_and_step_re_run_live() {
    let mut s = Session::new();
    let c = first(run(&mut s, "polyCube", json!({"name": "Post"})));
    run(&mut s, "select", json!({"args": [c]}));
    execute(&mut s, "duplicate", &json!({}), CallSource::Ui).unwrap();
    assert!(last_op::live(&s));
    assert_eq!(last_op::current(&s).unwrap().command, "duplicate");
    // The option override's adjust list; the option box itself is not implemented.
    let flags: Vec<String> = last_op::editable_flags(&s, "duplicate", &[], &["translate", "rotate", "scale", "pivotPoint"])
        .into_iter()
        .map(|f| f.name)
        .collect();
    for want in ["count", "translate", "rotate", "scale", "mode", "pivot", "group"] {
        assert!(flags.iter().any(|f| f == want), "{want}");
    }
    assert!(!flags.iter().any(|f| f == "name"));

    last_op::rerun(&mut s, &json!({"count": 4, "translate": [0, 0, 50]}), CallSource::Script, false).unwrap();
    assert_eq!(count(&mut s, "mesh"), 5);
    let sel = run(&mut s, "ls", json!({"selection": true}));
    assert_eq!(sel.as_array().unwrap().len(), 4);
    assert!(approx(get(&mut s, format!("{}.tz", sel[3].as_str().unwrap())), 200.0));
    last_op::rerun(&mut s, &json!({"count": 2}), CallSource::Script, false).unwrap();
    assert_eq!(count(&mut s, "mesh"), 3);
    run(&mut s, "undo", json!({}));
    assert_eq!(count(&mut s, "mesh"), 5);
}

#[test]
fn duplicate_with_transform_repeats_the_last_pairs_live_delta() {
    let mut s = Session::new();
    let c = first(run(&mut s, "polyCube", json!({"name": "Fence"})));
    assert!(s.run("duplicateWithTransform", json!({})).is_err());
    let a = first(run(&mut s, "duplicate", json!({"args": [c]})));
    run(&mut s, "move", json!({"args": [a], "by": [120, 0, 0]}));
    run(&mut s, "rotate", json!({"args": [a], "by": [0, 10, 0]}));
    // The step is the copy's whole change of world matrix, turn included, so a turned step bends the run into an arc.
    let step = DMat4::from_translation(DVec3::new(120.0, 0.0, 0.0)) * DMat4::from_rotation_y(10f64.to_radians());
    // Selection changes don't lose the record.
    run(&mut s, "select", json!({"args": [c]}));
    let next = run(&mut s, "duplicateWithTransform", json!({}));
    assert_eq!(next.as_array().unwrap().len(), 1);
    near(world_pos(&mut s, &next[0]), (step * step).w_axis.truncate());
    assert!(approx(get(&mut s, format!("{}.ry", next[0].as_str().unwrap())), 20.0));
    let more = run(&mut s, "duplicateWithTransform", json!({"count": 2}));
    assert_eq!(more.as_array().unwrap().len(), 2);
    near(world_pos(&mut s, &more[1]), (step * step * step * step).w_axis.truncate());
    assert!(approx(get(&mut s, format!("{}.ry", more[1].as_str().unwrap())), 40.0));

    // Editing the newest copy changes the step the next repeat uses.
    run(&mut s, "newScene", json!({}));
    let c = first(run(&mut s, "polyCube", json!({"name": "Rail"})));
    let a = first(run(&mut s, "duplicate", json!({"args": [c]})));
    run(&mut s, "move", json!({"args": [a], "by": [100, 0, 0]}));
    let third = run(&mut s, "duplicateWithTransform", json!({}));
    near(world_pos(&mut s, &third[0]), DVec3::new(200.0, 0.0, 0.0));
    run(&mut s, "move", json!({"args": [third[0]], "by": [0, 10, 0]}));
    let bent = run(&mut s, "duplicateWithTransform", json!({}));
    near(world_pos(&mut s, &bent[0]), DVec3::new(300.0, 20.0, 0.0));

    // The mode of the last duplicate carries over.
    let i = first(run(&mut s, "duplicate", json!({"args": [c], "mode": "instance"})));
    run(&mut s, "move", json!({"args": [i], "by": [0, 0, 30]}));
    let inst = run(&mut s, "duplicateWithTransform", json!({}));
    assert_eq!(kind(&mut s, inst[0].as_str().unwrap()), "instance");

    // Deleting either object of the pair forgets it.
    run(&mut s, "delete", json!({"args": [inst[0]]}));
    assert!(duplicate::last_pairs(&s.graph).is_empty());
    assert!(s.run("duplicateWithTransform", json!({})).is_err());
}
