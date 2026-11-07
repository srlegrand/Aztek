//! Tests: modifiers.

use aztek_ops::{execute, CallSource};
use aztek_scene::last_op;
use aztek_scene::{scene_file, Session};
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn faces(s: &mut Session, m: &str) -> i64 {
    run(s, "meshInfo", json!({"args": [m]}))["faces"].as_i64().unwrap()
}

fn name(v: &Value) -> String {
    (if v.is_array() { &v[0] } else { v }).as_str().unwrap().to_string()
}

fn len(v: &Value) -> usize {
    v.as_array().map_or(0, Vec::len)
}

fn cube(s: &mut Session) -> String {
    name(&run(s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0})))
}

/// The cage face whose normal points along +axis (or -axis).
fn face_on_axis(s: &mut Session, m: &str, axis: usize, sign: f64) -> i64 {
    let d = run(s, "meshData", json!({"args": [m], "base": true}));
    for (f, face) in d["faces"].as_array().unwrap().iter().enumerate() {
        let (mut lo, mut hi, mut along) = (1e9f64, -1e9f64, 0.0);
        for p in face.as_array().unwrap() {
            let v = d["points"][p.as_u64().unwrap() as usize][axis].as_f64().unwrap();
            lo = lo.min(v);
            hi = hi.max(v);
            along += v;
        }
        if hi - lo < 1e-6 && along * sign > 0.0 {
            return f as i64;
        }
    }
    -1
}

/// The final mesh's extent along an axis.
fn extent(s: &mut Session, m: &str, axis: usize) -> (f64, f64) {
    let d = run(s, "meshData", json!({"args": [m]}));
    let mut lo = 1e9f64;
    let mut hi = -1e9f64;
    for p in d["points"].as_array().unwrap() {
        lo = lo.min(p[axis].as_f64().unwrap());
        hi = hi.max(p[axis].as_f64().unwrap());
    }
    (lo, hi)
}

#[test]
fn modifier_stack_is_live_and_edits_go_under_it() {
    let mut s = Session::new();
    let c = cube(&mut s);
    let sub = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "subdivision"})));
    assert_eq!(faces(&mut s, &c), 24);
    let mirror = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "array", "params": {"count": 2}})));
    assert_eq!(faces(&mut s, &c), 48);
    assert_eq!(run(&mut s, "modifiers", json!({"args": [c]})), json!([sub, mirror]));

    // Turning a modifier off passes the mesh through.
    run(&mut s, "setAttr", json!({"args": [format!("{sub}.enabled"), false]}));
    assert_eq!(faces(&mut s, &c), 12);
    run(&mut s, "setAttr", json!({"args": [format!("{sub}.enabled"), true]}));

    // Component edits land on the cage (6 faces), and the stack re-runs on top.
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 10}));
    assert_eq!(faces(&mut s, &c), 10 * 4 * 2);
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 2);

    run(&mut s, "moveModifier", json!({"args": [sub], "by": 1}));
    assert_eq!(run(&mut s, "modifiers", json!({"args": [c]})), json!([mirror, sub]));
    assert_eq!(faces(&mut s, &c), 80);

    run(&mut s, "applyModifier", json!({"args": [mirror]}));
    assert_eq!(run(&mut s, "modifiers", json!({"args": [c]})), json!([sub]));
    assert_eq!(faces(&mut s, &c), 80);
    run(&mut s, "removeModifier", json!({"args": [sub]}));
    assert_eq!(faces(&mut s, &c), 20);
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 0);

    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &c), 80);
}

#[test]
fn every_modifier_kind_evaluates() {
    let mut s = Session::new();
    for kind in ["mirror", "subdivision", "bevel", "array", "triangulate", "weightedNormals", "solidify", "decimate"] {
        let m = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "name": format!("M{kind}")})));
        run(&mut s, "addModifier", json!({"args": [m], "kind": kind}));
        assert!(faces(&mut s, &m) > 0, "{kind}");
    }
}

#[test]
fn extrude_modifier_extrudes_the_faces_selected_when_added_live_and_undoable() {
    let mut s = Session::new();
    let c = cube(&mut s);
    let top = face_on_axis(&mut s, &c, 1, 1.0);
    assert!(top >= 0);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [top]}));
    let m = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude", "params": {"thickness": 20.0}})));
    assert_eq!(faces(&mut s, &c), 10);
    assert_eq!(len(&run(&mut s, "meshData", json!({"args": [c], "base": true}))["faces"]), 6);
    assert!(approx(extent(&mut s, &c, 1).1, 70.0));

    run(&mut s, "setAttr", json!({"args": [format!("{m}.thickness"), 5.0]}));
    assert!(approx(extent(&mut s, &c, 1).1, 55.0));
    run(&mut s, "setAttr", json!({"args": [format!("{m}.offset"), 10.0]}));
    assert_eq!(faces(&mut s, &c), 10);
    run(&mut s, "undo", json!({}));
    run(&mut s, "undo", json!({}));
    assert!(approx(extent(&mut s, &c, 1).1, 70.0));
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &c), 6);
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 0);
    run(&mut s, "redo", json!({}));
    assert_eq!(faces(&mut s, &c), 10);
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.thickness")]})), json!(20.0));

    // Two neighbouring faces extrude as one region unless individual.
    run(&mut s, "newScene", json!({}));
    let c = cube(&mut s);
    let (a, b) = (face_on_axis(&mut s, &c, 1, 1.0), face_on_axis(&mut s, &c, 0, 1.0));
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [a, b]}));
    let m = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude"})));
    assert_eq!(faces(&mut s, &c), 6 + 6);
    run(&mut s, "setAttr", json!({"args": [format!("{m}.individual"), true]}));
    assert_eq!(faces(&mut s, &c), 6 + 8);

    // No selection extrudes every face.
    run(&mut s, "newScene", json!({}));
    let c = cube(&mut s);
    run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude"}));
    assert!(extent(&mut s, &c, 0).1 > 55.0);
}

#[test]
fn extrude_modifier_keeps_its_faces_through_edits_under_it() {
    let mut s = Session::new();
    let c = cube(&mut s);
    // The highest id, so deleting face 0 shifts it when the cage compacts.
    let (mut axis, mut sign) = (usize::MAX, 0.0);
    for a in 0..3 {
        for sg in [1.0, -1.0] {
            if face_on_axis(&mut s, &c, a, sg) == 5 {
                axis = a;
                sign = sg;
            }
        }
    }
    assert_ne!(axis, usize::MAX);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [5]}));
    let m = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude", "params": {"thickness": 30.0}})));
    let reach = |s: &mut Session| {
        let (lo, hi) = extent(s, &c, axis);
        if sign > 0.0 {
            hi
        } else {
            -lo
        }
    };
    assert!(approx(reach(&mut s), 80.0));

    run(&mut s, "selectComponents", json!({"args": [c], "faces": [0]}));
    run(&mut s, "deleteComponents", json!({}));
    assert_eq!(faces(&mut s, &c), 5 + 4);
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.lostComponents")]})), json!(0));
    assert!(approx(reach(&mut s), 80.0));
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &c), 10);
    assert!(approx(reach(&mut s), 80.0));

    // A modifier below that changes the topology: the faces are found again where they were.
    run(&mut s, "newScene", json!({}));
    let c = cube(&mut s);
    run(&mut s, "addModifier", json!({"args": [c], "kind": "array", "params": {"count": 2}}));
    let top = face_on_axis(&mut s, &c, 1, 1.0);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [top]}));
    let m = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude"})));
    assert_eq!(faces(&mut s, &c), 12 + 4);
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.lostComponents")]})), json!(0));
    // A face that no longer exists is dropped rather than guessed.
    run(&mut s, "polySubdivide", json!({"args": [c]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.lostComponents")]})), json!(1));
}

#[test]
fn extrude_modifier_grows_edges_and_follows_symmetry() {
    std::env::set_var("AZTEK_CONFIG_DIR", std::env::temp_dir().join("aztek_test_config"));
    let mut s = Session::new();
    let plane = name(&run(&mut s, "polyPlane", json!({"width": 100.0, "depth": 100.0, "subdivisionsX": 1, "subdivisionsZ": 1})));
    run(&mut s, "selectComponents", json!({"args": [plane], "edges": [0]}));
    run(&mut s, "addModifier", json!({"args": [plane], "kind": "extrude", "params": {"thickness": 0.0, "offset": 25.0}}));
    assert_eq!(faces(&mut s, &plane), 2);
    let (lo, hi) = extent(&mut s, &plane, 0);
    let (zlo, zhi) = extent(&mut s, &plane, 2);
    assert!(approx((hi - lo) + (zhi - zlo), 225.0));

    run(&mut s, "newScene", json!({}));
    let c = cube(&mut s);
    run(&mut s, "toggleSymmetry", json!({"args": [c], "axis": "x"}));
    let f = face_on_axis(&mut s, &c, 0, 1.0);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [f]}));
    run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude"}));
    assert_eq!(faces(&mut s, &c), 6 + 8);
    assert!(approx(extent(&mut s, &c, 0).0, -60.0));
}

#[test]
fn poly_extrude_made_non_destructive_becomes_an_extrude_modifier() {
    let mut s = Session::new();
    let c = cube(&mut s);
    let front = face_on_axis(&mut s, &c, 2, 1.0);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [front]}));
    execute(&mut s, "polyExtrude", &json!({"thickness": 15.0, "offset": 5.0}), CallSource::Ui).unwrap();
    assert!(last_op::live(&s));
    assert_eq!(faces(&mut s, &c), 10);
    last_op::make_non_destructive(&mut s).unwrap();
    let stack = run(&mut s, "modifiers", json!({"args": [c]}));
    assert_eq!(len(&stack), 1);
    let m = name(&stack[0]);
    assert_eq!(run(&mut s, "nodeType", json!({"args": [m]})), "extrudeModifier");
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.thickness")]})), json!(15.0));
    assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{m}.offset")]})), json!(5.0));
    assert_eq!(faces(&mut s, &c), 10);
    assert_eq!(len(&run(&mut s, "meshData", json!({"args": [c], "base": true}))["faces"]), 6);
    assert!(approx(extent(&mut s, &c, 2).1, 65.0));
    run(&mut s, "undo", json!({}));
    assert_eq!(faces(&mut s, &c), 6);
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 0);
    run(&mut s, "redo", json!({}));
    assert_eq!(faces(&mut s, &c), 10);

    // In object mode the op extruded every face, and so does the modifier.
    run(&mut s, "newScene", json!({}));
    let c = cube(&mut s);
    run(&mut s, "select", json!({"args": [c]}));
    execute(&mut s, "polyExtrude", &json!({"thickness": 10.0}), CallSource::Ui).unwrap();
    let destructive = run(&mut s, "meshData", json!({"args": [c]}));
    last_op::make_non_destructive(&mut s).unwrap();
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 1);
    assert_eq!(run(&mut s, "meshData", json!({"args": [c]})), destructive);
}

fn round_trip(s: &mut Session, packed: bool) {
    let text = serde_json::to_string(&scene_file::serialize(&s.graph, packed)).unwrap();
    run(s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
}

#[test]
fn extrude_modifier_saves_and_loads() {
    let mut s = Session::new();
    let c = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "name": "Extruded"})));
    run(&mut s, "addModifier", json!({"args": [c], "kind": "array", "params": {"count": 2}}));
    let top = face_on_axis(&mut s, &c, 1, 1.0);
    run(&mut s, "selectComponents", json!({"args": [c], "faces": [top]}));
    let face_mod = name(&run(&mut s, "addModifier", json!({"args": [c], "kind": "extrude", "params": {"thickness": 12.5}})));
    let plane = name(&run(&mut s, "polyPlane", json!({"width": 100.0, "depth": 100.0, "name": "Strip"})));
    run(&mut s, "selectComponents", json!({"args": [plane], "edges": [0]}));
    run(&mut s, "addModifier", json!({"args": [plane], "kind": "extrude", "params": {"thickness": 0.0, "offset": 10.0}}));
    let cube_data = run(&mut s, "meshData", json!({"args": [c]}));
    let plane_data = run(&mut s, "meshData", json!({"args": [plane]}));
    for packed in [true, false] {
        round_trip(&mut s, packed);
        assert_eq!(run(&mut s, "meshData", json!({"args": [c]})), cube_data);
        assert_eq!(run(&mut s, "meshData", json!({"args": [plane]})), plane_data);
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{face_mod}.thickness")]})), json!(12.5));
        assert_eq!(run(&mut s, "getAttr", json!({"args": [format!("{face_mod}.lostComponents")]})), json!(0));
        run(&mut s, "setAttr", json!({"args": [format!("{face_mod}.thickness"), 2.5]}));
        assert!(approx(extent(&mut s, &c, 1).1, 52.5));
        run(&mut s, "undo", json!({}));
        assert_eq!(run(&mut s, "meshData", json!({"args": [c]})), cube_data);
    }
}

#[test]
fn modifier_stacks_save_and_load() {
    let mut s = Session::new();
    let c = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "name": "Stacked"})));
    run(&mut s, "addModifier", json!({"args": [c], "kind": "bevel", "params": {"segments": 3}}));
    run(&mut s, "addModifier", json!({"args": [c], "kind": "mirror"}));
    let before = faces(&mut s, &c);
    for packed in [true, false] {
        round_trip(&mut s, packed);
        assert_eq!(faces(&mut s, &c), before);
        assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 2);
    }
}

#[test]
fn triangulating_a_mesh_with_modifiers_triangulates_the_result() {
    let mut s = Session::new();
    let c = cube(&mut s);
    run(&mut s, "addModifier", json!({"args": [c], "kind": "bevel", "params": {"segments": 2}}));
    let before = run(&mut s, "meshInfo", json!({"args": [c]}));
    run(&mut s, "select", json!({"args": [c]}));
    run(&mut s, "polyTriangulate", json!({}));
    let after = run(&mut s, "meshInfo", json!({"args": [c]}));
    assert_eq!(after["points"], before["points"]);
    assert_eq!(after["faces"], before["triangles"]);
    assert_eq!(after["triangles"], before["triangles"]);
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 2);
    // Again is a no-op rather than a second modifier.
    run(&mut s, "selectMode", json!({"args": ["object"]}));
    run(&mut s, "select", json!({"args": [c]}));
    run(&mut s, "polyTriangulate", json!({}));
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 2);
    run(&mut s, "undo", json!({}));
    assert_eq!(len(&run(&mut s, "modifiers", json!({"args": [c]}))), 1);
    assert_eq!(run(&mut s, "meshInfo", json!({"args": [c]}))["faces"], before["faces"]);
}
