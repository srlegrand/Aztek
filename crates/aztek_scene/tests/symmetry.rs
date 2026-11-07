//! Tests: symmetry.

use aztek_geo::ops::{delete_points, extrude_faces, ExtrudeOptions};
use aztek_geo::primitives::{make_cube, CubeOptions};
use aztek_geo::symmetry::{axis_plane, symmetry_layer_name, SymmetryMap, SymmetrySync};
use aztek_geo::topology::boundary_loops;
use aztek_geo::{Index, Mesh, INVALID};
use aztek_scene::mesh_nodes::{symmetry_state, MeshNode};
use aztek_scene::plug::{PlugRef, PlugValue};
use aztek_scene::transform::Transform;
use aztek_scene::{scene_file, Session};
use glam::DVec3;
use rand_mt::{Mt, Mt64};
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

fn name(v: &Value) -> String {
    v.as_str().unwrap().to_string()
}

fn same_bits(a: &Mesh, b: &Mesh) -> bool {
    a.point_capacity() == b.point_capacity()
        && a.positions().iter().zip(b.positions()).all(|(x, y)| x.to_array().map(f64::to_bits) == y.to_array().map(f64::to_bits))
}

/// libstdc++'s uniform_real_distribution over [lo, hi) from one 64-bit draw.
fn uniform64(rng: &mut Mt64, lo: f64, hi: f64) -> f64 {
    let r = rng.next_u64() as f64 / 18446744073709551616.0;
    lo + r * (hi - lo)
}

fn uniform32(rng: &mut Mt, lo: f64, hi: f64) -> f64 {
    // Two 32-bit draws make the 53-bit mantissa, as libstdc++'s generate_canonical does for mt19937.
    let a = rng.next_u32() as f64;
    let b = rng.next_u32() as f64;
    let r = (a + b * 4294967296.0) / 18446744073709551616.0;
    lo + r.min(1.0 - f64::EPSILON / 2.0) * (hi - lo)
}

fn pick(rng: &mut Mt, n: i64) -> i64 {
    (rng.next_u32() as u64 % n as u64) as i64
}

#[test]
fn persistent_symmetry_survives_drift_and_compaction() {
    let mut m = make_cube(&CubeOptions { subdiv_x: 4, subdiv_y: 3, ..Default::default() });
    let tol = 1e-3;
    let mut sync = SymmetrySync::default();
    let first = SymmetryMap::persistent_axis(&mut m, 0, tol, Some(&mut sync), 0.0);
    assert_eq!(sync.unmatched, 0);
    assert_eq!(sync.by_position, m.num_points());
    assert!(m.point_attrs().has(&symmetry_layer_name(0)));

    // Drift the negative side a thousand times past the matching tolerance.
    for p in 0..m.point_capacity() {
        if first.side(p) < 0 {
            m.set_position(p, m.position(p) + DVec3::new(0.7, -1.3, 2.1));
        }
    }
    let mut pairs = Vec::new();
    for p in 0..m.point_capacity() {
        if first.side(p) > 0 {
            pairs.push((m.position(p), m.position(first.mirror_point(p))));
        }
    }
    assert!(SymmetryMap::build(&m, &axis_plane(0, 0.0), tol).unmatched_count() > 0);

    // Renumber the points: delete the first one and compact. Its partner is left without one.
    delete_points(&mut m, &[0], None);
    m.compact(None);
    let mut sync = SymmetrySync::default();
    let again = SymmetryMap::persistent_axis(&mut m, 0, tol, Some(&mut sync), 0.0);
    assert_eq!(sync.by_position, 0);
    assert_eq!(sync.topological, 0);
    assert_eq!(sync.unmatched, 1);
    let mut found = 0;
    for p in 0..m.point_capacity() {
        if again.side(p) > 0 {
            for (a, b) in &pairs {
                if *a == m.position(p) {
                    assert_eq!(m.position(again.mirror_point(p)), *b);
                    found += 1;
                }
            }
        }
    }
    assert!(found > 0);

    again.symmetrize(&mut m, 1);
    let c = again.check(&m);
    // Only the deleted point's partner is off: its reflection falls in the hole.
    assert!(c.max_asymmetry > 0.0);
    assert_eq!(again.mirror_point(c.worst_point), INVALID);
    assert_eq!(c.unmatched, 1);
}

#[test]
fn symmetric_ops_pair_the_points_they_create_however_far_the_old_ones_drifted() {
    let mut m = make_cube(&CubeOptions::default());
    let map = SymmetryMap::persistent_axis(&mut m, 0, 1e-3, None, 0.0);
    for p in 0..m.point_capacity() {
        if map.side(p) < 0 {
            m.set_position(p, m.position(p) + DVec3::new(-0.5, 0.25, 0.0));
        }
    }
    let map = SymmetryMap::persistent_axis(&mut m, 0, 1e-3, None, 0.0);
    let side: Vec<Index> = (0..m.face_capacity()).filter(|&f| m.face_valid(f) && m.face_normal(f).x > 0.5).collect();
    assert_eq!(side.len(), 1);
    let before = m.point_capacity();
    let eo = ExtrudeOptions { offset: 10.0, symmetry: Some(&map), ..Default::default() };
    extrude_faces(&mut m, &side, &eo);
    assert_eq!(m.point_capacity(), before + 8);
    let layer = symmetry_layer_name(0);
    for p in before..m.point_capacity() {
        m.point_attrs_mut().get_mut::<i32>(&layer)[p as usize] = 0;
    }
    let mut sync = SymmetrySync::default();
    SymmetryMap::persistent_axis(&mut m, 0, 1e-3, Some(&mut sync), 0.0);
    assert_eq!(sync.topological, 8);
    assert_eq!(sync.unmatched, 0);
}

#[test]
fn scene_save_load_and_undo_keep_positions_bit_exact() {
    let mut s = Session::new();
    let cube = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 3}))[0]);
    let mut rng = Mt64::new(11);
    for v in 0..16 {
        run(&mut s, "selectComponents", json!({"args": [cube], "vertices": [v]}));
        let by = [uniform64(&mut rng, -1.0, 1.0), uniform64(&mut rng, -1.0, 1.0), uniform64(&mut rng, -1.0, 1.0)];
        run(&mut s, "moveComponents", json!({"by": by}));
    }
    let edited = mesh_of(&mut s, &cube);
    run(&mut s, "selectComponents", json!({"args": [cube], "vertices": [3]}));
    run(&mut s, "moveComponents", json!({"by": [1.0 / 3.0, 0, 0]}));
    run(&mut s, "undo", json!({}));
    assert!(same_bits(&edited, &mesh_of(&mut s, &cube)));

    for packed in [true, false] {
        let doc = scene_file::serialize(&s.graph, packed);
        let text = serde_json::to_string(&doc).unwrap();
        run(&mut s, "newScene", json!({}));
        scene_file::deserialize(&mut s.graph, &serde_json::from_str(&text).unwrap()).unwrap();
        assert!(same_bits(&edited, &mesh_of(&mut s, &cube)), "packed {packed}");
    }
}

#[test]
fn a_thousand_random_symmetric_edits_stay_exactly_symmetric() {
    let mut s = Session::new();
    let cube = name(
        &run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 4, "subdivisionsY": 2}))[0],
    );
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    let mut rng = Mt::new(1234);
    for _ in 0..1000 {
        let info = run(&mut s, "meshInfo", json!({"args": [cube]}));
        let points = info["points"].as_i64().unwrap();
        let face_count = info["faces"].as_i64().unwrap();
        let op = pick(&mut rng, 20);
        if op == 0 && face_count < 400 {
            let f = pick(&mut rng, face_count);
            run(&mut s, "selectComponents", json!({"args": [cube], "faces": [f]}));
            let t = 2.0 + uniform32(&mut rng, -1.0, 1.0);
            run(&mut s, "polyExtrude", json!({"thickness": t}));
        } else if op == 1 && face_count < 400 {
            let f = pick(&mut rng, face_count);
            run(&mut s, "selectComponents", json!({"args": [cube], "faces": [f]}));
            run(&mut s, "polyInset", json!({"thickness": 1.0}));
        } else {
            let n = 1 + pick(&mut rng, 4);
            let verts: Vec<i64> = (0..n).map(|_| pick(&mut rng, points)).collect();
            run(&mut s, "selectComponents", json!({"args": [cube], "vertices": verts}));
            let mut d = || uniform32(&mut rng, -1.0, 1.0);
            match op % 3 {
                0 => run(&mut s, "moveComponents", json!({"by": [d() * 3.0, d() * 3.0, d() * 3.0]})),
                1 => run(&mut s, "rotateComponents", json!({"by": [d() * 20.0, d() * 20.0, d() * 20.0]})),
                _ => run(&mut s, "scaleComponents", json!({"by": [1.0 + d() * 0.1, 1.0 + d() * 0.1, 1.0 + d() * 0.1]})),
            };
        }
    }
    let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
    assert_eq!(c["maxAsymmetry"].as_f64().unwrap(), 0.0);
    assert_eq!(c["unmatched"], 0);
    assert_eq!(c["withoutStoredPartner"], 0);
}

#[test]
fn drift_with_symmetry_off_keeps_the_pairs_symmetrize_repairs_it() {
    let mut s = Session::new();
    let cube = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 2}))[0]);
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    run(&mut s, "selectComponents", json!({"args": [cube], "vertices": [0]}));
    // stores the pairs
    run(&mut s, "moveComponents", json!({"by": [0, 1, 0]}));
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "off"}));
    for v in 0..6 {
        run(&mut s, "selectComponents", json!({"args": [cube], "vertices": [v]}));
        run(&mut s, "moveComponents", json!({"by": [0.5 + v as f64, -0.25, 0.75]}));
    }
    // A topology edit with symmetry off renumbers points; the pairs ride along.
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0]}));
    run(&mut s, "polyExtrude", json!({"thickness": 3}));
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));

    let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
    assert!(c["maxAsymmetry"].as_f64().unwrap() > 0.1);
    run(&mut s, "symmetrize", json!({"args": [cube], "side": "negative"}));
    let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
    // The extrusion made with symmetry off has no mirror; symmetryRebuild makes one.
    assert!(approx(c["maxAsymmetry"].as_f64().unwrap(), 3.0));
    assert_eq!(c["unmatched"], 4);
    run(&mut s, "symmetryRebuild", json!({"args": [cube]}));
    let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
    assert_eq!(c["maxAsymmetry"].as_f64().unwrap(), 0.0);
    assert_eq!(c["unmatched"], 0);
}

#[test]
fn poly_bevel_with_symmetry_on_keeps_the_pairs_at_corners() {
    for segments in 1..=3 {
        let mut s = Session::new();
        let cube = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 2}))[0]);
        run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
        let before = run(&mut s, "meshInfo", json!({"args": [cube]}))["faces"].clone();
        // Every face's border in turn: corners of three bevelled edges, terminal edges and edges across the plane.
        for f in [0, 3] {
            run(&mut s, "selectComponents", json!({"args": [cube], "faces": [f]}));
            run(&mut s, "polyBevel", json!({"offset": 5.0, "segments": segments}));
            let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
            assert!(c["maxAsymmetry"].as_f64().unwrap() < 1e-9, "{segments} {f}");
            assert_eq!(c["unmatched"], 0, "{segments} {f}");
            run(&mut s, "undo", json!({}));
            assert_eq!(run(&mut s, "meshInfo", json!({"args": [cube]}))["faces"], before);
        }
    }
}

#[test]
fn toggle_symmetry_turns_a_mixed_selection_on_then_all_off_undoably() {
    std::env::set_var("AZTEK_CONFIG_DIR", std::env::temp_dir().join("aztek_test_config"));
    let mut s = Session::new();
    let a = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0}))[0]);
    let b = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0}))[0]);
    let group = name(&run(&mut s, "createNode", json!({"args": ["transform"]})));
    let axis_of = |s: &Session, n: &str| s.graph.node(aztek_scene::components::as_mesh(&s.graph, s.graph.find(n).unwrap().unwrap()).unwrap()).unwrap().int(MeshNode::SYMMETRY);

    run(&mut s, "setSymmetry", json!({"args": [a], "axis": "y"}));
    run(&mut s, "select", json!({"args": [a, b, group]}));
    let st = symmetry_state(&s.graph, s.graph.selection());
    assert_eq!(st.meshes, 2);
    assert_eq!(st.on, 1);
    assert_eq!(st.axis, 1);

    // Partial: everything goes on, on the axis already in use.
    assert_eq!(run(&mut s, "toggleSymmetry", json!({})), "y");
    assert_eq!(axis_of(&s, &a), 2);
    assert_eq!(axis_of(&s, &b), 2);
    assert_eq!(run(&mut s, "toggleSymmetry", json!({})), "off");
    assert_eq!(axis_of(&s, &a), 0);
    assert_eq!(axis_of(&s, &b), 0);
    run(&mut s, "undo", json!({}));
    assert_eq!(axis_of(&s, &b), 2);

    // An explicit axis different from the current one switches instead of turning off.
    assert_eq!(run(&mut s, "toggleSymmetry", json!({"axis": "z"})), "z");
    assert_eq!(symmetry_state(&s.graph, s.graph.selection()).axis, 2);
    assert_eq!(run(&mut s, "toggleSymmetry", json!({"axis": "z"})), "off");

    run(&mut s, "setSymmetry", json!({"args": [a], "axis": "x"}));
    run(&mut s, "setSymmetry", json!({"args": [b], "axis": "z"}));
    let st = symmetry_state(&s.graph, s.graph.selection());
    assert_eq!(st.on, 2);
    assert_eq!(st.axis, -1);

    run(&mut s, "select", json!({"args": [group]}));
    assert!(s.run("toggleSymmetry", json!({})).is_err());
}

/// The first face whose centre satisfies pred.
fn find_face(m: &Mesh, pred: impl Fn(DVec3) -> bool) -> i64 {
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let mut c = DVec3::ZERO;
        let k = m.face_size(f) as f64;
        m.for_each_face_corner(f, |x| c += m.position(m.corner_point(x)) / k);
        if pred(c) {
            return f as i64;
        }
    }
    -1
}

/// Every point has a point at its exact reflection across x = 0.
fn exactly_mirrored(m: &Mesh) -> bool {
    let map = SymmetryMap::build(m, &axis_plane(0, 0.0), 1e-12);
    if map.unmatched_count() != 0 {
        return false;
    }
    (0..m.point_capacity()).all(|p| {
        !m.point_valid(p)
            || if map.on_plane(p) {
                m.position(p).x == 0.0
            } else {
                m.position(map.mirror_point(p)) == m.position(p) * DVec3::new(-1.0, 1.0, 1.0)
            }
    })
}

#[test]
fn symmetry_check_counts_unmatched_points_as_asymmetry() {
    let mut s = Session::new();
    let cube = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0}))[0]);
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    // stores the cube's pairs
    run(&mut s, "symmetryCheck", json!({"args": [cube]}));
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "off"}));
    let f = find_face(&mesh_of(&mut s, &cube), |c| c.x > 40.0);
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [f]}));
    run(&mut s, "polyExtrude", json!({"thickness": 30}));
    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
    assert_eq!(c["unmatched"], 4);
    assert!(approx(c["maxAsymmetry"].as_f64().unwrap(), 30.0));
}

#[test]
fn symmetry_rebuild_mirrors_the_kept_side_of_an_asymmetric_mesh() {
    // An extruded +X side face: cut at the plane, 9 faces kept, mirrored to 18. Keeping -X leaves a split cube.
    // With a centre loop the extruded face is a top quad on +X whose inner wall lies in the plane and goes, so 8 faces
    // are kept and the block is mirrored into one twice as wide.
    for (subdiv, side, faces, points) in [(1, "auto", 18, 20), (1, "negative", 10, 12), (2, "auto", 16, 18), (2, "positive", 16, 18), (2, "negative", 10, 12)] {
        let mut s = Session::new();
        let cube = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": subdiv}))[0]);
        let before = mesh_of(&mut s, &cube);
        let face = if subdiv == 1 { find_face(&before, |c| c.x > 40.0) } else { find_face(&before, |c| c.x > 0.0 && c.y > 40.0) };
        assert!(face >= 0);
        run(&mut s, "selectComponents", json!({"args": [cube], "faces": [face]}));
        run(&mut s, "polyExtrude", json!({"thickness": 30}));
        let extruded = run(&mut s, "meshInfo", json!({"args": [cube]}))["faces"].clone();

        let out = run(&mut s, "symmetryRebuild", json!({"args": [cube], "side": side}));
        assert_eq!(out[&cube], 0, "{subdiv} {side}");
        let m = mesh_of(&mut s, &cube);
        assert_eq!(m.num_faces(), faces, "{subdiv} {side}");
        assert_eq!(m.num_points(), points, "{subdiv} {side}");
        assert!(boundary_loops(&m).is_empty());
        assert!(exactly_mirrored(&m));

        run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
        let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
        assert_eq!(c["maxAsymmetry"].as_f64().unwrap(), 0.0);
        assert_eq!(c["unmatched"], 0);
        assert_eq!(c["withoutStoredPartner"], 0);

        run(&mut s, "undo", json!({}));
        run(&mut s, "undo", json!({}));
        assert_eq!(run(&mut s, "meshInfo", json!({"args": [cube]}))["faces"], extruded);
    }
}

#[test]
fn symmetry_rebuild_and_symmetry_check_use_the_plane_through_the_pivot() {
    let mut s = Session::new();
    let cube = name(&run(&mut s, "polyCube", json!({"width": 100.0, "height": 100.0, "depth": 100.0, "subdivisionsX": 2}))[0]);
    // The geometry sits 30 cm along +X of the origin with the pivot at its centre, as on a part modelled in place.
    run(&mut s, "selectComponents", json!({"args": [cube], "faces": [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]}));
    run(&mut s, "moveComponents", json!({"by": [30.0, 0.0, 0.0]}));
    let id = s.graph.find(&cube).unwrap().unwrap();
    s.graph.set_value(PlugRef::new(id, Transform::ROTATE_PIVOT as i32), PlugValue::Vec3(DVec3::new(30.0, 0.0, 0.0))).unwrap();
    run(&mut s, "select", json!({"args": [cube]}));
    run(&mut s, "selectMode", json!({"args": ["object"]}));

    let out = run(&mut s, "symmetryRebuild", json!({"args": [cube]}));
    assert_eq!(out[&cube], 0);
    let m = mesh_of(&mut s, &cube);
    assert_eq!(m.num_faces(), 10);
    assert!(approx(m.bounds().min.x, -20.0));
    assert!(approx(m.bounds().max.x, 80.0));

    run(&mut s, "setSymmetry", json!({"args": [cube], "axis": "x"}));
    let c = run(&mut s, "symmetryCheck", json!({"args": [cube]}))[&cube].clone();
    assert_eq!(c["unmatched"], 0);
    assert!(c["maxAsymmetry"].as_f64().unwrap() < 1e-9);
}
