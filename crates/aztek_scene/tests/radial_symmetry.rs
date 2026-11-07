//! Tests: radial symmetry.

use aztek_geo::mesh::validate;
use aztek_geo::symmetry::{radial_layer_name, SymmetryMap};
use aztek_geo::{Index, Mesh, INVALID};
use aztek_scene::mesh_nodes::{symmetry_spec, MeshNode, SYMMETRY_RADIAL};
use aztek_scene::{scene_file, Session};
use rand_mt::Mt;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-5 * (1.0 + a.abs().max(b.abs()))
}

fn mesh_of(s: &mut Session, name: &str) -> Mesh {
    s.graph.evaluate();
    let id = s.graph.find(name).unwrap().unwrap();
    (**s.graph.node(aztek_scene::components::as_mesh(&s.graph, id).unwrap()).unwrap().data().mesh(MeshNode::OUT_MESH).unwrap()).clone()
}

const CASES: [(i64, bool); 4] = [(3, false), (6, false), (8, false), (6, true)];

/// A 24-sided open tube: every tested count divides its columns.
fn tube(s: &mut Session) -> String {
    run(s, "polyCylinder", json!({"subdivisionsAxis": 24, "subdivisionsHeight": 3, "caps": false}))[0].as_str().unwrap().to_string()
}

fn set_radial(s: &mut Session, mesh: &str, (count, mirror): (i64, bool)) {
    run(s, "setSymmetry", json!({"args": [mesh], "axis": "radial", "count": count, "mirror": mirror, "radialAxis": "y"}));
}

fn pick(rng: &mut Mt, n: i64) -> i64 {
    (rng.next_u32() as u64 % n as u64) as i64
}

fn uniform(rng: &mut Mt, lo: f64, hi: f64) -> f64 {
    let (a, b) = (rng.next_u32() as f64, rng.next_u32() as f64);
    let r = ((a + b * 4294967296.0) / 18446744073709551616.0).min(1.0 - f64::EPSILON / 2.0);
    lo + r * (hi - lo)
}

fn check_of(s: &mut Session, mesh: &str) -> Value {
    run(s, "symmetryCheck", json!({"args": [mesh]}))[mesh].clone()
}

#[test]
fn random_edits_stay_exactly_symmetric() {
    for c in CASES {
        let mut s = Session::new();
        let mesh = tube(&mut s);
        set_radial(&mut s, &mesh, c);
        let mut rng = Mt::new(77);
        for _ in 0..120 {
            let info = run(&mut s, "meshInfo", json!({"args": [mesh]}));
            let (points, face_count) = (info["points"].as_i64().unwrap(), info["faces"].as_i64().unwrap());
            let op = pick(&mut rng, 12);
            if op == 0 && face_count < 600 {
                let f = pick(&mut rng, face_count);
                run(&mut s, "selectComponents", json!({"args": [mesh], "faces": [f]}));
                let t = 2.0 + uniform(&mut rng, -1.0, 1.0);
                run(&mut s, "polyExtrude", json!({"thickness": t}));
            } else if op == 1 && face_count < 600 {
                let f = pick(&mut rng, face_count);
                run(&mut s, "selectComponents", json!({"args": [mesh], "faces": [f]}));
                run(&mut s, "polyInset", json!({"thickness": 0.5}));
            } else {
                let n = 1 + pick(&mut rng, 3);
                let verts: Vec<i64> = (0..n).map(|_| pick(&mut rng, points)).collect();
                run(&mut s, "selectComponents", json!({"args": [mesh], "vertices": verts}));
                let mut d = || uniform(&mut rng, -1.0, 1.0);
                match op % 3 {
                    0 => run(&mut s, "moveComponents", json!({"by": [d() * 3.0, d() * 3.0, d() * 3.0]})),
                    1 => run(&mut s, "rotateComponents", json!({"by": [d() * 20.0, d() * 20.0, d() * 20.0]})),
                    _ => run(&mut s, "scaleComponents", json!({"by": [1.0 + d() * 0.1, 1.0 + d() * 0.1, 1.0 + d() * 0.1]})),
                };
            }
        }
        let check = check_of(&mut s, &mesh);
        assert_eq!(check["axis"], "radial", "{c:?}");
        assert_eq!(check["count"], c.0, "{c:?}");
        assert!(check["maxAsymmetry"].as_f64().unwrap() < 1e-9, "{c:?}");
        assert_eq!(check["unmatched"], 0, "{c:?}");
        assert_eq!(check["withoutStoredPartner"], 0, "{c:?}");
    }
}

#[test]
fn a_component_move_is_turned_and_mirrored_into_every_wedge() {
    for c in CASES {
        let mut s = Session::new();
        let mesh = tube(&mut s);
        set_radial(&mut s, &mesh, c);
        let before = mesh_of(&mut s, &mesh);
        // A point off every mirror plane: the middle of a column pair isn't a vertex, so take one and move it sideways.
        run(&mut s, "selectComponents", json!({"args": [mesh], "vertices": [30]}));
        run(&mut s, "moveComponents", json!({"by": [1.0, 2.0, 0.5]}));
        let mut after = mesh_of(&mut s, &mesh);
        let spec = symmetry_spec(&s.graph, s.graph.find(&mesh).unwrap().unwrap()).unwrap();
        let radial = spec.radial.unwrap();
        let map = SymmetryMap::persistent_radial(&mut after, &radial, 1e-4, None);
        let delta = after.position(30) - before.position(30);
        assert!(delta.length() > 0.5);
        for g in 1..map.order() {
            let q = map.image(g, 30);
            assert_ne!(q, INVALID);
            if q == 30 {
                continue;
            }
            assert!((after.position(q) - map.transform(g, after.position(30))).length() < 1e-9, "{c:?}");
        }
        let moved = (0..after.point_capacity()).filter(|&p| (after.position(p) - before.position(p)).length() > 1e-9).count();
        assert_eq!(moved, map.expand_points(&[30]).len(), "{c:?}");
    }
}

#[test]
fn symmetrize_radial_rebuilds_one_wedge_round_the_axis_and_turns_radial_symmetry_on() {
    for c in CASES {
        let mut s = Session::new();
        let sphere = run(&mut s, "polySphere", json!({"subdivisionsAxis": 24, "subdivisionsHeight": 8}))[0].as_str().unwrap().to_string();
        // Spoil the mesh with symmetry off, then rebuild it from wedge 0.
        for v in (0..40).step_by(7) {
            run(&mut s, "selectComponents", json!({"args": [sphere], "vertices": [v]}));
            run(&mut s, "moveComponents", json!({"by": [0.0, 1.5, 0.0]}));
        }
        run(&mut s, "select", json!({"args": [sphere]}));
        run(&mut s, "selectMode", json!({"args": ["object"]}));
        let out = run(&mut s, "symmetrizeRadial", json!({"args": [sphere], "count": c.0, "mirror": c.1, "radialAxis": "y", "wedge": 0}));
        assert!(out[&sphere].as_i64().unwrap() > 0);
        let n = s.graph.node(aztek_scene::components::as_mesh(&s.graph, s.graph.find(&sphere).unwrap().unwrap()).unwrap()).unwrap();
        assert_eq!(n.int(MeshNode::SYMMETRY), SYMMETRY_RADIAL);
        assert_eq!(n.int(MeshNode::RADIAL_COUNT), c.0);
        assert!(mesh_of(&mut s, &sphere).validate(validate::DEFAULT).is_empty(), "{c:?}");
        let check = check_of(&mut s, &sphere);
        assert!(check["maxAsymmetry"].as_f64().unwrap() < 1e-9, "{c:?}");
        assert_eq!(check["unmatched"], 0, "{c:?}");
        assert_eq!(check["stored"], true, "{c:?}");
    }
}

#[test]
fn mismatch_unmatched_points_and_the_count_that_fits() {
    let mut s = Session::new();
    let mesh = tube(&mut s);
    run(&mut s, "setSymmetry", json!({"args": [mesh], "axis": "radial", "count": 5}));
    let check = check_of(&mut s, &mesh);
    assert!(check["unmatched"].as_i64().unwrap() > 0);
    assert_eq!(check["unmatchedPoints"].as_array().unwrap().len() as i64, check["unmatched"].as_i64().unwrap());
    assert_eq!(check["bestCount"], 24);
    run(&mut s, "setSymmetry", json!({"args": [mesh], "axis": "radial", "count": 8}));
    let check = check_of(&mut s, &mesh);
    assert_eq!(check["unmatched"], 0);
    assert!(check["unmatchedPoints"].as_array().unwrap().is_empty());
}

#[test]
fn radial_orbits_survive_save_load_and_drift_with_symmetry_off() {
    let mut s = Session::new();
    let mesh = tube(&mut s);
    set_radial(&mut s, &mesh, (6, true));
    run(&mut s, "selectComponents", json!({"args": [mesh], "vertices": [3]}));
    // stores the orbits
    run(&mut s, "moveComponents", json!({"by": [0.0, 1.0, 0.0]}));
    let spec = symmetry_spec(&s.graph, s.graph.find(&mesh).unwrap().unwrap()).unwrap();
    let layer = radial_layer_name(spec.radial.as_ref().unwrap());
    assert!(mesh_of(&mut s, &mesh).point_attrs().has(&layer));

    let text = serde_json::to_string(&scene_file::serialize(&s.graph, true)).unwrap();
    run(&mut s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
    assert!(mesh_of(&mut s, &mesh).point_attrs().has(&layer));

    run(&mut s, "setSymmetry", json!({"args": [mesh], "axis": "off"}));
    for v in 0..10 {
        run(&mut s, "selectComponents", json!({"args": [mesh], "vertices": [v * 5]}));
        run(&mut s, "moveComponents", json!({"by": [2.0, -1.0, 3.0]}));
    }
    set_radial(&mut s, &mesh, (6, true));
    let check = check_of(&mut s, &mesh);
    assert_eq!(check["withoutStoredPartner"], 0);
    assert!(check["maxAsymmetry"].as_f64().unwrap() > 1.0);
    run(&mut s, "symmetrize", json!({"args": [mesh], "wedge": 0}));
    assert!(check_of(&mut s, &mesh)["maxAsymmetry"].as_f64().unwrap() < 1e-9);
}

#[test]
fn set_symmetry_plane_with_radial_symmetry_puts_the_axis_through_the_selections_centre() {
    let mut s = Session::new();
    let mesh = tube(&mut s);
    let all: Vec<Index> = (0..96).collect();
    run(&mut s, "selectComponents", json!({"args": [mesh], "vertices": all}));
    run(&mut s, "moveComponents", json!({"by": [10.0, 0.0, 0.0]}));
    run(&mut s, "setSymmetry", json!({"args": [mesh], "axis": "radial", "count": 24}));
    assert!(check_of(&mut s, &mesh)["unmatched"].as_i64().unwrap() > 0);
    // The first 24 points are the bottom ring.
    let ring: Vec<Index> = (0..24).collect();
    run(&mut s, "selectComponents", json!({"args": [mesh], "vertices": ring}));
    let out = run(&mut s, "setSymmetryPlane", json!({}));
    assert_eq!(out[&mesh][0], "radial");
    let n = s.graph.node(aztek_scene::components::as_mesh(&s.graph, s.graph.find(&mesh).unwrap().unwrap()).unwrap()).unwrap();
    assert_eq!(n.int(MeshNode::SYMMETRY_PLANE), 1);
    let centre = n.vec3(MeshNode::RADIAL_CENTER);
    assert!(approx(centre.x, 10.0));
    assert!(centre.z.abs() < 1e-9);
    let check = check_of(&mut s, &mesh);
    assert_eq!(check["unmatched"], 0);
    assert!(check["maxAsymmetry"].as_f64().unwrap() < 1e-6);
}

#[test]
fn toggle_symmetry_turns_radial_symmetry_on_and_off() {
    std::env::set_var("AZTEK_CONFIG_DIR", std::env::temp_dir().join("aztek_test_config"));
    let mut s = Session::new();
    let mesh = tube(&mut s);
    assert_eq!(run(&mut s, "toggleSymmetry", json!({"args": [mesh], "axis": "radial"})), "radial");
    assert_eq!(s.graph.node(aztek_scene::components::as_mesh(&s.graph, s.graph.find(&mesh).unwrap().unwrap()).unwrap()).unwrap().int(MeshNode::SYMMETRY), SYMMETRY_RADIAL);
    assert_eq!(run(&mut s, "toggleSymmetry", json!({"args": [mesh]})), "off");
    assert_eq!(run(&mut s, "toggleSymmetry", json!({"args": [mesh]})), "radial");
}
