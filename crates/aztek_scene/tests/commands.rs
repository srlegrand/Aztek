//! Tests: commands.

use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use aztek_scene::transform::{euler_to_matrix, RotateOrder, Transform};
use aztek_scene::transform_ops as xf;
use aztek_scene::{scene_file, Session};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn vec(j: &Value) -> DVec3 {
    DVec3::new(
        j[0].as_f64().unwrap(),
        j[1].as_f64().unwrap(),
        j[2].as_f64().unwrap(),
    )
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * (1.0 + a.abs().max(b.abs()))
}

fn world_t(s: &mut Session, n: &str) -> DVec3 {
    vec(&run(
        s,
        "xform",
        json!({"args": [n], "q": true, "ws": true, "t": true}),
    ))
}

fn world_m(s: &mut Session, n: &str) -> DMat4 {
    s.graph.evaluate();
    let id = s.graph.find(n).unwrap().unwrap();
    s.graph.node(id).unwrap().matrix(Transform::WORLD_MATRIX)
}

fn id(s: &Session, n: &str) -> u64 {
    s.graph.find(n).unwrap().unwrap()
}

#[test]
fn names_are_unique_and_undo_restores_them() {
    let mut s = Session::new();
    assert_eq!(run(&mut s, "spaceLocator", json!({"name": "loc"})), json!(["loc"]));
    assert_eq!(run(&mut s, "spaceLocator", json!({"name": "loc"})), json!(["loc1"]));
    assert_eq!(run(&mut s, "spaceLocator", json!({"name": "loc1"})), json!(["loc2"]));
    run(&mut s, "undo", json!({}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["loc2"]})), false);
    run(&mut s, "redo", json!({}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["loc2"]})), true);
}

#[test]
fn hierarchy_keeps_world_position_when_parenting() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "a"}));
    run(
        &mut s,
        "setAttr",
        json!({"args": ["a.translate", [10, 0, 0]]}),
    );
    run(&mut s, "spaceLocator", json!({"name": "b"}));
    run(
        &mut s,
        "setAttr",
        json!({"args": ["b.translate", [0, 5, 0]]}),
    );
    run(&mut s, "parent", json!({"args": ["b", "a"]}));
    let w = world_t(&mut s, "b");
    assert!(approx(w.x, 0.0) && approx(w.y, 5.0));
    assert!(approx(
        vec(&run(&mut s, "getAttr", json!({"args": ["b.translate"]}))).x,
        -10.0
    ));
    run(&mut s, "setAttr", json!({"args": ["a.tx", 20]}));
    assert!(approx(world_t(&mut s, "b").x, 10.0));
    run(&mut s, "undo", json!({}));
    assert!(approx(world_t(&mut s, "b").x, 0.0));
}

#[test]
fn connections_drive_values_and_refuse_cycles() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "src"}));
    run(&mut s, "spaceLocator", json!({"name": "dst"}));
    run(
        &mut s,
        "connectAttr",
        json!({"args": ["src.translate", "dst.rotate"]}),
    );
    run(
        &mut s,
        "setAttr",
        json!({"args": ["src.translate", [1, 2, 3]]}),
    );
    assert_eq!(
        vec(&run(&mut s, "getAttr", json!({"args": ["dst.rotate"]}))),
        DVec3::new(1.0, 2.0, 3.0)
    );
    assert!(s
        .run("setAttr", json!({"args": ["dst.rotate", [0, 0, 0]]}))
        .is_err());
    assert!(s
        .run("connectAttr", json!({"args": ["dst.rotate", "src.scale"]}))
        .is_err());
}

#[test]
fn undoing_a_connection_puts_back_the_value_the_plug_had_before_it() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "src"}));
    run(&mut s, "spaceLocator", json!({"name": "dst"}));
    run(
        &mut s,
        "setAttr",
        json!({"args": ["dst.translate", [4, 5, 6]]}),
    );
    run(
        &mut s,
        "setAttr",
        json!({"args": ["src.translate", [1, 2, 3]]}),
    );
    run(
        &mut s,
        "connectAttr",
        json!({"args": ["src.translate", "dst.translate"]}),
    );
    assert_eq!(
        vec(&run(&mut s, "getAttr", json!({"args": ["dst.translate"]}))),
        DVec3::new(1.0, 2.0, 3.0)
    );
    run(&mut s, "undo", json!({}));
    assert_eq!(
        vec(&run(&mut s, "getAttr", json!({"args": ["dst.translate"]}))),
        DVec3::new(4.0, 5.0, 6.0)
    );
    run(&mut s, "connectAttr", json!({"args": ["src.ty", "dst.tx"]}));
    assert_eq!(
        vec(&run(&mut s, "getAttr", json!({"args": ["dst.translate"]}))),
        DVec3::new(2.0, 5.0, 6.0)
    );
    run(&mut s, "undo", json!({}));
    assert_eq!(
        vec(&run(&mut s, "getAttr", json!({"args": ["dst.translate"]}))),
        DVec3::new(4.0, 5.0, 6.0)
    );
}

#[test]
fn pivots_dont_move_the_object() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "p"}));
    run(
        &mut s,
        "setAttr",
        json!({"args": ["p.rotatePivot", [5, 0, 0]]}),
    );
    run(&mut s, "setAttr", json!({"args": ["p.rotate", [0, 0, 90]]}));
    let w = world_t(&mut s, "p");
    assert!(approx(w.x, 5.0) && approx(w.y, -5.0));
}

#[test]
fn parallel_evaluation_of_a_wide_graph() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "root"}));
    for i in 0..200 {
        let n = run(
            &mut s,
            "createNode",
            json!({"args": ["transform"], "name": "c", "parent": "root", "skipSelect": true}),
        );
        run(
            &mut s,
            "setAttr",
            json!({"args": [format!("{}.tx", n.as_str().unwrap()), i]}),
        );
    }
    run(&mut s, "setAttr", json!({"args": ["root.ty", 3]}));
    let w = world_t(&mut s, "c199");
    assert!(approx(w.x, 199.0) && approx(w.y, 3.0));
}

#[test]
fn naming_conventions_through_commands() {
    let mut s = Session::new();
    s.naming_prefs = json!({"enabled": true, "allowUnderscores": {"joint": false, "*": false}});
    s.load_naming_policy();
    assert_eq!(
        run(&mut s, "spaceLocator", json!({"name": "my_target"})),
        json!(["MyTarget"])
    );
    assert_eq!(run(&mut s, "spaceLocator", json!({})), json!(["Locator1"]));
    s.naming_prefs = json!({});
    s.load_naming_policy();
    assert_eq!(run(&mut s, "spaceLocator", json!({})), json!(["locator1"]));
    s.naming_prefs =
        json!({"enabled": true, "allowUnderscores": {"joint": false, "*": false}, "mode": "block"});
    s.load_naming_policy();
    assert!(s.run("spaceLocator", json!({"name": "bad_name"})).is_err());
    // check lists what fix would rename.
    s.naming_prefs = json!({});
    s.load_naming_policy();
    run(&mut s, "spaceLocator", json!({"name": "my_thing"}));
    let c = run(&mut s, "namingConvention", json!({"check": true}));
    assert!(c
        .as_array()
        .unwrap()
        .contains(&json!(["my_thing", "MyThing"])));
}

#[test]
fn scene_save_load_round_trip() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "A"}));
    run(
        &mut s,
        "setAttr",
        json!({"args": ["A.translate", [1, 2, 3]]}),
    );
    run(&mut s, "spaceLocator", json!({"name": "B"}));
    run(
        &mut s,
        "parent",
        json!({"args": ["B", "A"], "relative": true}),
    );
    run(&mut s, "setAttr", json!({"args": ["B.rotateOrder", "zxy"]}));
    run(
        &mut s,
        "connectAttr",
        json!({"args": ["A.translate", "B.scale"]}),
    );
    let doc = scene_file::serialize(&s.graph, false);

    run(&mut s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &doc).unwrap();
    assert_eq!(
        run(&mut s, "listRelatives", json!({"args": ["A"]})),
        json!(["AShape", "B"])
    );
    assert_eq!(
        vec(&run(&mut s, "getAttr", json!({"args": ["B.scale"]}))),
        DVec3::new(1.0, 2.0, 3.0)
    );
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["B.rotateOrder"]})),
        2
    );
    assert_eq!(run(&mut s, "undoInfo", json!({}))["undo"], "");
    assert_eq!(scene_file::serialize(&s.graph, false), doc);
}

#[test]
fn parent_chain_follows_selection_order() {
    let mut s = Session::new();
    for n in ["L1", "L2", "L3"] {
        run(&mut s, "spaceLocator", json!({"name": n}));
    }
    run(&mut s, "select", json!({"args": ["L3", "L2", "L1"]}));
    run(&mut s, "parent", json!({"chain": true}));
    assert_eq!(
        run(
            &mut s,
            "listRelatives",
            json!({"args": ["L3"], "parent": true})
        ),
        json!(["L2"])
    );
    assert_eq!(
        run(
            &mut s,
            "listRelatives",
            json!({"args": ["L2"], "parent": true})
        ),
        json!(["L1"])
    );
    run(&mut s, "undo", json!({}));
    assert_eq!(
        run(
            &mut s,
            "listRelatives",
            json!({"args": ["L3"], "parent": true})
        ),
        json!([])
    );
}

#[test]
fn move_rotate_scale_commands() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "A"}));
    run(&mut s, "spaceLocator", json!({"name": "B"}));
    run(&mut s, "parent", json!({"args": ["B", "A"]}));
    run(&mut s, "move", json!({"args": ["A", "B"], "by": [0, 5, 0]}));
    // B is under A, so it only moves with its parent.
    let t = run(
        &mut s,
        "xform",
        json!({"args": ["B"], "query": true, "worldSpace": true, "translation": true}),
    );
    assert!(approx(t[1].as_f64().unwrap(), 5.0));
    run(&mut s, "rotate", json!({"args": ["A"], "by": [0, 90, 0]}));
    assert!(approx(
        run(&mut s, "getAttr", json!({"args": ["A.ry"]}))
            .as_f64()
            .unwrap(),
        90.0
    ));
    run(&mut s, "scale", json!({"args": ["A"], "by": [2, 2, 2]}));
    assert!(approx(
        run(&mut s, "getAttr", json!({"args": ["A.sx"]}))
            .as_f64()
            .unwrap(),
        2.0
    ));
    run(&mut s, "move", json!({"args": ["A"], "to": [10, 0, 0]}));
    let p = run(&mut s, "pivot", json!({"args": ["A"]}));
    assert!(approx(p[0].as_f64().unwrap(), 10.0));
}

fn mat_near(a: &DMat4, b: &DMat4) -> bool {
    a.abs_diff_eq(
        *b,
        1e-9 * (1.0 + a.to_cols_array().iter().fold(0.0f64, |m, x| m.max(x.abs()))),
    )
}

#[test]
fn pivot_edits_keep_the_object_in_place() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "P"}));
    run(
        &mut s,
        "xform",
        json!({"args": ["P"], "translation": [3, 4, 5], "rotation": [20, 30, 40], "scale": [1, 2, 3]}),
    );
    let before = world_m(&mut s, "P");
    run(
        &mut s,
        "setPivot",
        json!({"args": ["P"], "position": [10, -2, 7]}),
    );
    assert!(mat_near(&world_m(&mut s, "P"), &before));
    let piv = vec(&run(&mut s, "pivot", json!({"args": ["P"]})));
    assert!((piv - DVec3::new(10.0, -2.0, 7.0)).length() < 1e-9);

    // Rotating now turns about the new pivot, which stays put.
    run(&mut s, "rotate", json!({"args": ["P"], "by": [0, 45, 0]}));
    let piv = vec(&run(&mut s, "pivot", json!({"args": ["P"]})));
    assert!(approx(piv.x, 10.0) && approx(piv.z, 7.0));

    run(&mut s, "resetPivot", json!({"args": ["P"]}));
    let reset = world_m(&mut s, "P");
    run(&mut s, "undo", json!({}));
    assert!(mat_near(&world_m(&mut s, "P"), &reset));
}

fn near3(x: &DMat3, y: &DMat3) -> bool {
    (0..3).all(|c| (x.col(c) - y.col(c)).abs().max_element() <= 1e-6)
}

#[test]
fn match_pivot_copies_position_orientation_or_both() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "A"}));
    run(&mut s, "spaceLocator", json!({"name": "S"}));
    run(
        &mut s,
        "xform",
        json!({"args": ["A"], "translation": [1, 2, 3], "rotation": [10, 0, 0], "scale": [1, 2, 1]}),
    );
    run(
        &mut s,
        "xform",
        json!({"args": ["S"], "translation": [-4, 0, 6], "rotation": [0, 50, 20]}),
    );
    run(
        &mut s,
        "setPivot",
        json!({"args": ["S"], "position": [9, 9, 9], "orient": [0, 0, 30]}),
    );
    let before = world_m(&mut s, "A");
    let (a, sid) = (id(&s, "A"), id(&s, "S"));
    let a_pivot = xf::world_pivot(&mut s.graph, a);
    let a_axes = xf::pivot_frame(&mut s.graph, a);
    let s_axes = xf::pivot_frame(&mut s.graph, sid);

    run(
        &mut s,
        "matchPivot",
        json!({"args": ["S", "A"], "orient": false}),
    );
    assert!((xf::world_pivot(&mut s.graph, a) - DVec3::splat(9.0)).length() < 1e-6);
    assert!(near3(&xf::pivot_frame(&mut s.graph, a), &a_axes));

    run(&mut s, "undo", json!({}));
    run(
        &mut s,
        "matchPivot",
        json!({"args": ["S", "A"], "position": false}),
    );
    assert!((xf::world_pivot(&mut s.graph, a) - a_pivot).length() < 1e-6);
    assert!(near3(&xf::pivot_frame(&mut s.graph, a), &s_axes));

    run(&mut s, "undo", json!({}));
    run(&mut s, "matchPivot", json!({"args": ["S", "A"]}));
    assert!((xf::world_pivot(&mut s.graph, a) - DVec3::splat(9.0)).length() < 1e-6);
    assert!(near3(&xf::pivot_frame(&mut s.graph, a), &s_axes));
    assert!(mat_near(&world_m(&mut s, "A"), &before));
}

#[test]
fn zero_pivot_rotation_lines_pivot_axes_up_with_the_world_or_the_object() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "A"}));
    run(
        &mut s,
        "xform",
        json!({"args": ["A"], "translation": [1, 2, 3], "rotation": [10, 40, 0]}),
    );
    run(
        &mut s,
        "setPivot",
        json!({"args": ["A"], "position": [5, 5, 5], "orient": [0, 0, 30]}),
    );
    let a = id(&s, "A");
    let before = world_m(&mut s, "A");
    run(&mut s, "zeroPivotRotation", json!({"args": ["A"]}));
    assert!(near3(&xf::pivot_frame(&mut s.graph, a), &DMat3::IDENTITY));
    assert!((xf::world_pivot(&mut s.graph, a) - DVec3::splat(5.0)).length() < 1e-6);
    assert!(mat_near(&world_m(&mut s, "A"), &before));
    run(
        &mut s,
        "zeroPivotRotation",
        json!({"args": ["A"], "space": "object"}),
    );
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["A.pivotOrient"]}))[2],
        0.0
    );
}

#[test]
fn closest_euler_avoids_flips() {
    let m = DMat3::from_mat4(euler_to_matrix(
        DVec3::new(170.0, 10.0, -175.0),
        RotateOrder::Xyz,
    ));
    let out = xf::closest_euler(&m, RotateOrder::Xyz, DVec3::new(530.0, 10.0, 185.0));
    assert!(approx(out.x, 530.0) && approx(out.z, 185.0));
    for o in 0..6 {
        let order = RotateOrder::from_index(o);
        let r = DMat3::from_mat4(euler_to_matrix(DVec3::new(25.0, 160.0, -70.0), order));
        let got = xf::closest_euler(&r, order, DVec3::new(100.0, 120.0, -200.0));
        let back = DMat3::from_mat4(euler_to_matrix(got, order));
        assert!((0..3).all(|c| (back.col(c) - r.col(c)).length() < 1e-9));
    }
}

#[test]
fn values_stay_inside_an_attrs_hard_limits() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "loc"}));
    run(
        &mut s,
        "addAttr",
        json!({"args": ["loc"], "longName": "count", "attributeType": "long", "minValue": 1, "maxValue": 32}),
    );
    run(
        &mut s,
        "addAttr",
        json!({"args": ["loc"], "longName": "amount", "attributeType": "double", "minValue": 0.0}),
    );
    run(&mut s, "setAttr", json!({"args": ["loc.count", -4]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["loc.count"]})), 1);
    run(&mut s, "setAttr", json!({"args": ["loc.count", 99]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["loc.count"]})), 32);
    run(&mut s, "setAttr", json!({"args": ["loc.amount", -0.5]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["loc.amount"]})), 0.0);
    run(&mut s, "setAttr", json!({"args": ["loc.amount", 7.5]}));
    assert_eq!(run(&mut s, "getAttr", json!({"args": ["loc.amount"]})), 7.5);
}

#[test]
fn connected_values_are_held_inside_the_destinations_limits() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "src"}));
    run(&mut s, "spaceLocator", json!({"name": "dst"}));
    run(
        &mut s,
        "addAttr",
        json!({"args": ["dst"], "longName": "count", "attributeType": "long", "minValue": 1, "maxValue": 32}),
    );
    run(
        &mut s,
        "connectAttr",
        json!({"args": ["src.tx", "dst.count"]}),
    );
    for (x, want) in [(-5.0, 1), (50.0, 32), (7.0, 7)] {
        run(&mut s, "setAttr", json!({"args": ["src.tx", x]}));
        assert_eq!(run(&mut s, "getAttr", json!({"args": ["dst.count"]})), want);
    }
}

#[test]
fn compounds_and_multi_compounds() {
    let mut s = Session::new();
    run(&mut s, "spaceLocator", json!({"name": "n"}));
    run(
        &mut s,
        "addAttr",
        json!({"args": ["n"], "longName": "targets", "attributeType": "compound", "multi": true}),
    );
    run(
        &mut s,
        "addAttr",
        json!({"args": ["n"], "longName": "weight", "attributeType": "double", "parent": "targets"}),
    );
    run(
        &mut s,
        "setAttr",
        json!({"args": ["n.targets[2].weight", 0.5]}),
    );
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["n.targets[2].weight"]})),
        0.5
    );
    run(
        &mut s,
        "removeMultiInstance",
        json!({"args": ["n.targets[2]"]}),
    );
    assert_eq!(
        run(
            &mut s,
            "getAttr",
            json!({"args": ["n.targets.weight"], "multiIndices": true})
        ),
        json!([])
    );
    run(&mut s, "deleteAttr", json!({"args": ["n.targets"]}));
    assert_eq!(
        run(
            &mut s,
            "listAttr",
            json!({"args": ["n"], "userDefined": true})
        ),
        json!([])
    );
}

#[test]
fn pick_walk_walks_on_from_a_deleted_object() {
    let mut s = Session::new();
    for n in ["a", "b", "c"] {
        run(&mut s, "spaceLocator", json!({"name": n}));
    }
    run(&mut s, "select", json!({"args": ["b"]}));
    run(&mut s, "delete", json!({}));
    run(&mut s, "pickWalk", json!({"direction": "right"}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["c"]));
    run(&mut s, "pickWalk", json!({"direction": "right"}));
    assert_eq!(run(&mut s, "ls", json!({"selection": true})), json!(["a"]));
}

// Node paths

fn make(s: &mut Session, ty: &str, name: &str, parent: Option<&str>) -> String {
    let mut call = json!({"args": [ty], "name": name, "skipSelect": true});
    if let Some(p) = parent {
        call["parent"] = json!(p);
    }
    run(s, "createNode", call).as_str().unwrap().to_string()
}

fn two_arms() -> Session {
    let mut s = Session::new();
    make(&mut s, "transform", "Body", None);
    make(&mut s, "transform", "L", Some("Body"));
    make(&mut s, "transform", "R", Some("Body"));
    make(&mut s, "transform", "Arm", Some("L"));
    make(&mut s, "transform", "Arm", Some("R"));
    s
}

#[test]
fn names_only_need_to_be_unique_among_siblings() {
    let mut s = Session::new();
    make(&mut s, "transform", "L", None);
    make(&mut s, "transform", "R", None);
    assert_eq!(make(&mut s, "transform", "Arm", Some("L")), "Arm");
    assert_eq!(make(&mut s, "transform", "Arm", Some("R")), "R|Arm");
    assert_eq!(
        run(&mut s, "ls", json!({"args": ["Arm"]})),
        json!(["L|Arm", "R|Arm"])
    );
    assert_eq!(make(&mut s, "transform", "Arm", Some("L")), "Arm1");
    let a = make(&mut s, "transform", "", Some("L"));
    let b = make(&mut s, "transform", "", Some("R"));
    assert_ne!(a, b);
}

#[test]
fn an_ambiguous_name_is_an_error_that_lists_the_paths() {
    let mut s = two_arms();
    let err = s
        .run("getAttr", json!({"args": ["Arm.tx"]}))
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("ambiguous") && err.contains("|Body|L|Arm") && err.contains("|Body|R|Arm")
    );
    assert!(s.run("select", json!({"args": ["Arm"]})).is_err());
    assert_eq!(run(&mut s, "objExists", json!({"args": ["Arm"]})), true);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["Foot"]})), false);
    assert_eq!(
        run(&mut s, "objExists", json!({"args": ["R|L|Arm"]})),
        false
    );
}

#[test]
fn partial_and_full_paths_resolve() {
    let mut s = two_arms();
    run(&mut s, "setAttr", json!({"args": ["L|Arm.tx", 3]}));
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["|Body|L|Arm.tx"]})),
        3.0
    );
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["Body|R|Arm.tx"]})),
        0.0
    );
    assert_eq!(run(&mut s, "objExists", json!({"args": ["|L|Arm"]})), false);
}

#[test]
fn command_results_re_resolve_to_the_same_node() {
    let mut s = two_arms();
    for p in run(&mut s, "ls", json!({})).as_array().unwrap() {
        assert_eq!(s.graph.find_all(p.as_str().unwrap()).len(), 1);
    }
    run(&mut s, "select", json!({"args": ["L|Arm", "R|Arm"]}));
    let sel = run(&mut s, "ls", json!({"selection": true}));
    assert_eq!(
        s.graph.full_path(id(&s, sel[0].as_str().unwrap())),
        "|Body|L|Arm"
    );
    assert_eq!(
        s.graph.full_path(id(&s, sel[1].as_str().unwrap())),
        "|Body|R|Arm"
    );
    assert_eq!(
        run(&mut s, "rename", json!({"args": ["R|Arm", "Arm"]})),
        "R|Arm"
    );
    assert_eq!(
        run(&mut s, "rename", json!({"args": ["R|Arm", "Wing"]})),
        "Wing"
    );
}

#[test]
fn group_and_parent_leave_other_nodes_names_alone() {
    let mut s = two_arms();
    assert_eq!(
        run(&mut s, "group", json!({"args": ["L|Arm"], "name": "G", "world": true})),
        "G"
    );
    assert!(s.graph.find("G|Arm").unwrap().is_some());
    run(&mut s, "parent", json!({"args": ["R|Arm"], "world": true}));
    assert_eq!(s.graph.node(id(&s, "|Arm")).unwrap().name(), "Arm");
    // Parenting into a real clash renames, and undo puts the name back.
    run(&mut s, "parent", json!({"args": ["|Arm", "G"]}));
    assert!(s.graph.find("|G|Arm1").unwrap().is_some());
    run(&mut s, "undo", json!({}));
    assert!(s.graph.find("|Arm").unwrap().is_some());
    assert!(s.graph.find("|G|Arm1").unwrap().is_none());
}

#[test]
fn scene_documents_keep_duplicate_short_names_apart() {
    let mut s = two_arms();
    run(&mut s, "spaceLocator", json!({"name": "Arm"}));
    run(&mut s, "setAttr", json!({"args": ["L|Arm.tx", 4]}));
    run(
        &mut s,
        "connectAttr",
        json!({"args": ["L|Arm.tx", "R|Arm.ty"]}),
    );
    run(&mut s, "select", json!({"args": ["R|Arm"]}));
    let doc = scene_file::serialize(&s.graph, false);
    run(&mut s, "newScene", json!({}));
    scene_file::deserialize(&mut s.graph, &doc).unwrap();
    assert_eq!(s.graph.find_all("Arm").len(), 3);
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["|Body|L|Arm.tx"]})),
        4.0
    );
    assert_eq!(
        run(&mut s, "getAttr", json!({"args": ["|Body|R|Arm.ty"]})),
        4.0
    );
    assert_eq!(s.graph.node(id(&s, "|Arm|ArmShape")).unwrap().ty().name, "locator");
    assert_eq!(s.graph.full_path(s.graph.selection()[0]), "|Body|R|Arm");
}

#[test]
fn the_unique_names_pref_restores_scene_wide_renaming() {
    let mut s = Session::new();
    run(&mut s, "namingConvention", json!({"uniqueNames": true}));
    make(&mut s, "transform", "L", None);
    make(&mut s, "transform", "R", None);
    assert_eq!(make(&mut s, "transform", "Arm", Some("L")), "Arm");
    assert_eq!(make(&mut s, "transform", "Arm", Some("R")), "Arm1");
    assert_eq!(
        run(&mut s, "rename", json!({"args": ["Arm1", "Arm"]})),
        "Arm1"
    );
}

#[test]
fn undo_info_history_and_jump() {
    let mut s = Session::new();
    for n in ["a", "b", "c"] {
        run(&mut s, "spaceLocator", json!({"name": n}));
    }
    let info = run(&mut s, "undoInfo", json!({"history": true}));
    assert_eq!(info["position"], 3);
    run(&mut s, "undoJump", json!({"position": 1}));
    assert_eq!(run(&mut s, "ls", json!({})), json!(["a", "aShape"]));
    run(&mut s, "undoJump", json!({"position": 3}));
    assert_eq!(run(&mut s, "ls", json!({})).as_array().unwrap().len(), 6);
}
