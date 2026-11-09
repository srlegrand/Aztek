//! Tests: graph.

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;

use aztek_scene::naming::{apply_case, CaseStyle, NamingMode, NamingPolicy, Underscores};
use aztek_scene::node::{types, EvalContext, NodeBehavior, NodeData, NodeType, PlaybackExtra};
use aztek_scene::plug::{flags, AttrDef, PlugRef, PlugType, PlugValue, NO_NODE};
use aztek_scene::transform::{euler_to_matrix, matrix_to_euler, RotateOrder, Transform};
use aztek_scene::{scene_file, Graph, GraphError, NodeId, SelectMode};
use glam::{DMat3, DMat4, DVec3};

fn new_scene() -> Graph {
    types::register_builtins();
    Graph::new()
}

fn ty(name: &str) -> Arc<NodeType> {
    types::find(name).unwrap()
}

fn loc(g: &mut Graph, name: &str) -> NodeId {
    g.chunk("spaceLocator", |g| {
        g.create_node(&ty("transform"), name, NO_NODE, true)
    })
    .unwrap()
}

fn make(g: &mut Graph, type_name: &str, name: &str, parent: Option<&str>) -> String {
    let p = parent
        .map(|p| g.find(p).unwrap().unwrap())
        .unwrap_or(NO_NODE);
    let id = g.create_node(&ty(type_name), name, p, true).unwrap();
    g.path(id)
}

fn id(g: &Graph, path: &str) -> NodeId {
    g.find(path).unwrap().unwrap()
}

fn plug(g: &Graph, path: &str) -> PlugRef {
    g.plug(path).unwrap()
}

fn set(g: &mut Graph, path: &str, v: impl Into<PlugValue>) -> Result<(), GraphError> {
    let p = g.plug(path)?;
    g.chunk("setAttr", |g| g.set_value(p, v.into()))
}

fn get(g: &mut Graph, path: &str) -> PlugValue {
    let p = plug(g, path);
    g.value(&p).unwrap()
}

fn world(g: &mut Graph, path: &str) -> DMat4 {
    g.evaluate();
    g.node(id(g, path)).unwrap().matrix(Transform::WORLD_MATRIX)
}

fn world_t(g: &mut Graph, path: &str) -> DVec3 {
    world(g, path).w_axis.truncate()
}

fn parent(g: &mut Graph, child: &str, parent: &str) {
    let (c, p) = (id(g, child), id(g, parent));
    g.chunk("parent", |g| g.set_parent(c, p, true, usize::MAX))
        .unwrap();
}

fn approx(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * (1.0 + a.abs().max(b.abs()))
}

#[test]
fn names_are_unique_and_undo_restores_them() {
    let mut g = new_scene();
    let a = loc(&mut g, "loc");
    let b = loc(&mut g, "loc");
    let c = loc(&mut g, "loc1");
    assert_eq!(g.node(a).unwrap().name(), "loc");
    assert_eq!(g.node(b).unwrap().name(), "loc1");
    assert_eq!(g.node(c).unwrap().name(), "loc2");
    g.undo();
    assert!(g.find("loc2").unwrap().is_none());
    g.redo();
    assert!(g.find("loc2").unwrap().is_some());
}

#[test]
fn hierarchy_keeps_world_position_when_parenting() {
    let mut g = new_scene();
    loc(&mut g, "a");
    set(&mut g, "a.translate", DVec3::new(10.0, 0.0, 0.0)).unwrap();
    loc(&mut g, "b");
    set(&mut g, "b.translate", DVec3::new(0.0, 5.0, 0.0)).unwrap();
    parent(&mut g, "b", "a");

    let w = world_t(&mut g, "b");
    assert!(approx(w.x, 0.0) && approx(w.y, 5.0));
    assert!(approx(get(&mut g, "b.translate").as_vec3().x, -10.0));

    set(&mut g, "a.tx", 20.0).unwrap();
    assert!(approx(world_t(&mut g, "b").x, 10.0));

    g.undo();
    assert!(approx(world_t(&mut g, "b").x, 0.0));
}

#[test]
fn connections_drive_values_and_refuse_cycles() {
    let mut g = new_scene();
    loc(&mut g, "src");
    loc(&mut g, "dst");
    g.connect(&plug(&g, "src.translate"), &plug(&g, "dst.rotate"), false)
        .unwrap();
    set(&mut g, "src.translate", DVec3::new(1.0, 2.0, 3.0)).unwrap();
    assert_eq!(
        get(&mut g, "dst.rotate").as_vec3(),
        DVec3::new(1.0, 2.0, 3.0)
    );
    assert!(set(&mut g, "dst.rotate", DVec3::ZERO).is_err());
    assert!(g
        .connect(&plug(&g, "dst.rotate"), &plug(&g, "src.scale"), false)
        .is_err());
}

#[test]
fn undoing_a_connection_puts_back_the_value_the_plug_had_before_it() {
    let mut g = new_scene();
    loc(&mut g, "src");
    loc(&mut g, "dst");
    set(&mut g, "dst.translate", DVec3::new(4.0, 5.0, 6.0)).unwrap();
    set(&mut g, "src.translate", DVec3::new(1.0, 2.0, 3.0)).unwrap();
    let (s, d) = (plug(&g, "src.translate"), plug(&g, "dst.translate"));
    g.chunk("connectAttr", |g| g.connect(&s, &d, false))
        .unwrap();
    assert_eq!(
        get(&mut g, "dst.translate").as_vec3(),
        DVec3::new(1.0, 2.0, 3.0)
    );
    g.undo();
    assert_eq!(
        get(&mut g, "dst.translate").as_vec3(),
        DVec3::new(4.0, 5.0, 6.0)
    );
    let (s, d) = (plug(&g, "src.ty"), plug(&g, "dst.tx"));
    g.chunk("connectAttr", |g| g.connect(&s, &d, false))
        .unwrap();
    assert_eq!(
        get(&mut g, "dst.translate").as_vec3(),
        DVec3::new(2.0, 5.0, 6.0)
    );
    g.undo();
    assert_eq!(
        get(&mut g, "dst.translate").as_vec3(),
        DVec3::new(4.0, 5.0, 6.0)
    );
}

#[test]
fn pivots_dont_move_the_object() {
    let mut g = new_scene();
    loc(&mut g, "p");
    set(&mut g, "p.rotatePivot", DVec3::new(5.0, 0.0, 0.0)).unwrap();
    set(&mut g, "p.rotate", DVec3::new(0.0, 0.0, 90_f64.to_radians())).unwrap();
    // Rotating 90 about Z around (5,0,0) swings the origin to (5,-5,0).
    let w = world_t(&mut g, "p");
    assert!(approx(w.x, 5.0) && approx(w.y, -5.0));
}

#[test]
fn euler_round_trip_for_every_rotate_order() {
    let r = DVec3::new(30_f64.to_radians(), (-40_f64).to_radians(), 70_f64.to_radians());
    for o in 0..6 {
        let order = RotateOrder::from_index(o);
        let m = euler_to_matrix(r, order);
        let back = matrix_to_euler(&DMat3::from_mat4(m), order);
        let m2 = euler_to_matrix(back, order);
        assert!(m.abs_diff_eq(m2, 1e-9), "order {o}");
        // Away from gimbal lock the angles themselves come back.
        assert!((back - r).abs().max_element() < 1e-9, "order {o}: {back}");
    }
}

#[test]
fn euler_at_gimbal_lock_still_round_trips() {
    for o in 0..6 {
        let order = RotateOrder::from_index(o);
        for mid in [90.0, -90.0] {
            let mut r = DVec3::new(25.0, 35.0, 45.0);
            let axes = [
                [0, 1, 2],
                [1, 2, 0],
                [2, 0, 1],
                [0, 2, 1],
                [1, 0, 2],
                [2, 1, 0],
            ][o as usize];
            r[axes[1]] = mid;
            let m = euler_to_matrix(r, order);
            let back = matrix_to_euler(&DMat3::from_mat4(m), order);
            assert!(
                m.abs_diff_eq(euler_to_matrix(back, order), 1e-9),
                "order {o} mid {mid}"
            );
        }
    }
}

#[test]
fn parallel_evaluation_of_a_wide_graph() {
    let mut g = new_scene();
    let root = loc(&mut g, "root");
    for i in 0..200 {
        let n = g.create_node(&ty("transform"), "c", root, true).unwrap();
        g.set_value(
            PlugRef::component(n, Transform::TRANSLATE as i32, 0),
            (i as f64).into(),
        )
        .unwrap();
    }
    set(&mut g, "root.ty", 3.0).unwrap();
    let w = world_t(&mut g, "c199");
    assert!(approx(w.x, 199.0) && approx(w.y, 3.0));
}

#[test]
fn naming_conventions() {
    let none = Underscores::default();
    assert_eq!(apply_case("myCube", CaseStyle::Pascal, &none), "MyCube");
    assert_eq!(apply_case("my_cube", CaseStyle::Pascal, &none), "MyCube");
    assert_eq!(apply_case("MyCube", CaseStyle::Snake, &none), "my_cube");
    assert_eq!(
        apply_case("LeftArmIK", CaseStyle::Snake, &none),
        "left_arm_ik"
    );
    assert_eq!(
        apply_case("HTTPServer", CaseStyle::Pascal, &none),
        "HttpServer"
    );
    let anywhere = Underscores {
        allow: true,
        suffixes_only: false,
        ..Default::default()
    };
    assert_eq!(
        apply_case("upper arm_l", CaseStyle::Pascal, &anywhere),
        "UpperArm_L"
    );
    assert_eq!(apply_case("tail_", CaseStyle::Pascal, &anywhere), "Tail_");
    assert_eq!(
        apply_case("left_arm_2", CaseStyle::Camel, &anywhere),
        "left_Arm_2"
    );
    let sides = Underscores {
        allow: true,
        suffixes: vec!["L".into(), "B".into(), "LF".into()],
        ..Default::default()
    };
    assert_eq!(
        apply_case("dog_leg_b", CaseStyle::Pascal, &sides),
        "DogLeg_B"
    );
    assert_eq!(
        apply_case("wheel_lf", CaseStyle::Pascal, &sides),
        "Wheel_LF"
    );
    assert_eq!(apply_case("arm_l_2", CaseStyle::Pascal, &sides), "Arm_L_2");
    assert_eq!(apply_case("tail_", CaseStyle::Pascal, &sides), "Tail_");
    assert_eq!(
        apply_case("upper_arm", CaseStyle::Camel, &sides),
        "upperArm"
    );
    assert_eq!(apply_case("l_arm", CaseStyle::Pascal, &sides), "LArm");
    let prefixed = Underscores {
        prefixes: true,
        ..sides.clone()
    };
    assert_eq!(apply_case("l_arm", CaseStyle::Pascal, &prefixed), "L_Arm");
    assert_eq!(
        apply_case("l_upper_arm_b", CaseStyle::Camel, &prefixed),
        "L_UpperArm_B"
    );
    assert_eq!(apply_case("l_b", CaseStyle::Pascal, &prefixed), "L_B");

    types::register_builtins();
    let locator = ty("transform");
    let kept = NamingPolicy::from_json(&serde_json::json!({"enabled": true}));
    assert_eq!(kept.conform(&locator, "ns:my_cube_12"), "ns:MyCube_12");
    assert_eq!(kept.conform(&locator, "dog_leg_b"), "DogLeg_B");
    assert_eq!(kept.conform(&locator, "UCX_arm_l"), "UCX_Arm_L");
    let mut p = NamingPolicy::from_json(
        &serde_json::json!({"enabled": true, "allowUnderscores": {"joint": false, "*": false}}),
    );
    assert_eq!(p.conform(&locator, "myCube"), "MyCube");
    assert_eq!(p.conform(&locator, "ns:my_cube12"), "ns:MyCube12");
    assert_eq!(p.conform(&locator, "UCX_my_cube"), "UCX_MyCube");

    let mut g = new_scene();
    g.set_naming_policy(p.clone());
    let a = loc(&mut g, "my_target");
    assert_eq!(g.node(a).unwrap().name(), "MyTarget");
    let b = loc(&mut g, "");
    assert_eq!(g.node(b).unwrap().name(), "Transform1");
    g.set_naming_policy(NamingPolicy::default());
    let c = loc(&mut g, "");
    assert_eq!(g.node(c).unwrap().name(), "transform1");
    p.mode = NamingMode::Block;
    g.set_naming_policy(p);
    assert!(g.create_node(&locator, "bad_name", NO_NODE, true).is_err());
}

#[test]
fn scene_save_load_round_trip() {
    let mut g = new_scene();
    loc(&mut g, "A");
    set(&mut g, "A.translate", DVec3::new(1.0, 2.0, 3.0)).unwrap();
    loc(&mut g, "B");
    let (a, b) = (id(&g, "A"), id(&g, "B"));
    g.set_parent(b, a, false, usize::MAX).unwrap();
    set(&mut g, "B.rotateOrder", 2i64).unwrap();
    g.connect(&plug(&g, "A.translate"), &plug(&g, "B.scale"), false)
        .unwrap();
    let doc = scene_file::serialize(&g, false);

    let mut h = new_scene();
    let warnings = scene_file::deserialize(&mut h, &doc).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    let a = id(&h, "A");
    assert_eq!(
        h.children(a)
            .iter()
            .map(|&c| h.node(c).unwrap().name().to_string())
            .collect::<Vec<_>>(),
        ["B"]
    );
    assert_eq!(get(&mut h, "B.scale").as_vec3(), DVec3::new(1.0, 2.0, 3.0));
    assert_eq!(get(&mut h, "B.rotateOrder").as_int(), 2);
    assert!(h.undo_stack().undo_label().is_empty() && !h.undo_stack().can_undo());
    assert_eq!(scene_file::serialize(&h, false), doc);
}

#[test]
fn evaluate_upstream_computes_only_what_the_given_nodes_depend_on() {
    let mut g = new_scene();
    loc(&mut g, "parent");
    loc(&mut g, "child");
    parent(&mut g, "child", "parent");
    loc(&mut g, "other");
    g.evaluate();

    set(&mut g, "parent.translate", DVec3::new(0.0, 5.0, 0.0)).unwrap();
    set(&mut g, "other.translate", DVec3::new(0.0, 7.0, 0.0)).unwrap();
    let other = id(&g, "other");
    let before = g.node(other).unwrap().evaluated_pass();
    g.evaluate_upstream(&[id(&g, "child")]);
    let child = g.node(id(&g, "child")).unwrap();
    assert!(approx(child.matrix(Transform::WORLD_MATRIX).w_axis.y, 5.0));
    assert_eq!(g.node(other).unwrap().evaluated_pass(), before);

    // The rest computes on the next full evaluation.
    g.evaluate();
    assert!(g.node(other).unwrap().evaluated_pass() > before);
    assert!(approx(
        g.node(other)
            .unwrap()
            .matrix(Transform::WORLD_MATRIX)
            .w_axis
            .y,
        7.0
    ));
}

fn user(name: &str, ty: PlugType, min: Option<f64>, max: Option<f64>) -> AttrDef {
    let default_value = aztek_scene::plug::default_for(ty);
    AttrDef {
        name: name.into(),
        ty,
        default_value,
        min_value: min,
        max_value: max,
        ..Default::default()
    }
}

#[test]
fn values_stay_inside_an_attrs_hard_limits() {
    let mut g = new_scene();
    let l = loc(&mut g, "loc");
    g.add_attr(l, user("count", PlugType::Int, Some(1.0), Some(32.0)))
        .unwrap();
    g.add_attr(l, user("amount", PlugType::Float, Some(0.0), None))
        .unwrap();
    set(&mut g, "loc.count", -4i64).unwrap();
    assert_eq!(get(&mut g, "loc.count").as_int(), 1);
    set(&mut g, "loc.count", 99i64).unwrap();
    assert_eq!(get(&mut g, "loc.count").as_int(), 32);
    set(&mut g, "loc.amount", -0.5).unwrap();
    assert_eq!(get(&mut g, "loc.amount").as_float(), 0.0);
    set(&mut g, "loc.amount", 7.5).unwrap();
    assert_eq!(get(&mut g, "loc.amount").as_float(), 7.5);
}

#[test]
fn connected_values_are_held_inside_the_destinations_limits() {
    let mut g = new_scene();
    loc(&mut g, "src");
    let d = loc(&mut g, "dst");
    g.add_attr(d, user("count", PlugType::Int, Some(1.0), Some(32.0)))
        .unwrap();
    g.connect(&plug(&g, "src.tx"), &plug(&g, "dst.count"), false)
        .unwrap();
    for (x, want) in [(-5.0, 1), (50.0, 32), (7.0, 7)] {
        set(&mut g, "src.tx", x).unwrap();
        assert_eq!(get(&mut g, "dst.count").as_int(), want);
    }
}

/// Outputs the time it computed at and counts its computes.
struct TimeCounter(Arc<AtomicI32>);
impl NodeBehavior for TimeCounter {
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        self.0.fetch_add(1, Ordering::SeqCst);
        d.set_output(0, ctx.time);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn out_attr() -> AttrDef {
    AttrDef {
        name: "out".into(),
        short_name: "o".into(),
        flags: flags::OUTPUT,
        ..Default::default()
    }
}

#[test]
fn cached_playback_restores_kept_times_and_drops_them_on_an_edit() {
    let computes = Arc::new(AtomicI32::new(0));
    let c2 = computes.clone();
    let counter_ty = types::find("testTimeCounter").unwrap_or_else(|| {
        types::register_type(NodeType {
            name: "testTimeCounter".into(),
            time_dependent: true,
            own_attrs: vec![out_attr()],
            create: Some(Arc::new(move || Box::new(TimeCounter(c2.clone())))),
            ..Default::default()
        })
        .unwrap()
    });
    let mut g = new_scene();
    g.create_node(&counter_ty, "counter", NO_NODE, true)
        .unwrap();
    let l = loc(&mut g, "loc");
    g.connect(&plug(&g, "counter.out"), &plug(&g, "loc.tx"), false)
        .unwrap();
    let x = |g: &Graph| g.node(l).unwrap().matrix(Transform::WORLD_MATRIX).w_axis.x;

    g.set_cached_playback(true);
    for t in [1.0, 2.0] {
        g.set_time(t);
        g.evaluate();
    }
    assert_eq!(g.playback_cache_times(), vec![1.0, 2.0]);

    let before = computes.load(Ordering::SeqCst);
    g.set_time(1.0);
    g.evaluate();
    assert!(approx(x(&g), 1.0));
    g.set_time(2.0);
    g.evaluate();
    assert!(approx(x(&g), 2.0));
    assert_eq!(computes.load(Ordering::SeqCst), before);

    set(&mut g, "loc.ty", 3.0).unwrap();
    assert!(g.playback_cache_times().is_empty());
    g.set_time(1.0);
    g.evaluate();
    assert_eq!(computes.load(Ordering::SeqCst), before + 1);
    assert!(approx(x(&g), 1.0));
}

/// Outputs -1 until told its data has arrived, as a simulation's output does before its state reaches the frame.
struct LateData(Arc<AtomicBool>);
impl NodeBehavior for LateData {
    fn compute(&mut self, ctx: &EvalContext, d: &mut NodeData) {
        d.set_output(
            0,
            if self.0.load(Ordering::SeqCst) {
                ctx.time
            } else {
                -1.0
            },
        );
    }
    fn playback_extra(&self, _: &EvalContext, _: &NodeData) -> PlaybackExtra {
        PlaybackExtra {
            keep: self.0.load(Ordering::SeqCst),
            ..Default::default()
        }
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[test]
fn cached_playback_never_keeps_a_placeholder_result() {
    let ready = Arc::new(AtomicBool::new(false));
    let r2 = ready.clone();
    let late_ty = types::register_type(NodeType {
        name: "testLateData".into(),
        time_dependent: true,
        own_attrs: vec![out_attr()],
        create: Some(Arc::new(move || Box::new(LateData(r2.clone())))),
        ..Default::default()
    })
    .unwrap();
    let mut g = new_scene();
    let late = g.create_node(&late_ty, "late", NO_NODE, true).unwrap();
    let f = loc(&mut g, "follower");
    g.connect(&plug(&g, "late.out"), &plug(&g, "follower.tx"), false)
        .unwrap();
    let x = |g: &Graph| g.node(f).unwrap().matrix(Transform::WORLD_MATRIX).w_axis.x;
    g.set_cached_playback(true);

    g.set_time(3.0);
    g.evaluate();
    assert!(approx(g.node(late).unwrap().float(0), -1.0));
    assert!(g.playback_cache_times().is_empty());

    ready.store(true, Ordering::SeqCst);
    g.touch(late);
    g.evaluate();
    assert!(approx(g.node(late).unwrap().float(0), 3.0));
    assert!(approx(x(&g), 3.0));
    assert_eq!(g.playback_cache_times(), vec![3.0]);

    g.set_time(4.0);
    g.evaluate();
    g.set_time(3.0);
    g.evaluate();
    assert!(approx(g.node(late).unwrap().float(0), 3.0));
    assert!(approx(x(&g), 3.0));
}

// Node paths

/// Two "Arm"s, under |Body|L and |Body|R.
fn two_arms() -> Graph {
    let mut g = new_scene();
    make(&mut g, "transform", "Body", None);
    make(&mut g, "transform", "L", Some("Body"));
    make(&mut g, "transform", "R", Some("Body"));
    make(&mut g, "transform", "Arm", Some("L"));
    make(&mut g, "transform", "Arm", Some("R"));
    g
}

#[test]
fn names_only_need_to_be_unique_among_siblings() {
    let mut g = new_scene();
    make(&mut g, "transform", "L", None);
    make(&mut g, "transform", "R", None);
    assert_eq!(make(&mut g, "transform", "Arm", Some("L")), "Arm");
    assert_eq!(make(&mut g, "transform", "Arm", Some("R")), "R|Arm");
    // A real sibling clash still gets a suffix.
    assert_eq!(make(&mut g, "transform", "Arm", Some("L")), "Arm1");
    // Default names stay unique across the scene.
    let a = make(&mut g, "transform", "", Some("L"));
    let b = make(&mut g, "transform", "", Some("R"));
    assert_ne!(a, b);
}

#[test]
fn an_ambiguous_name_is_an_error_that_lists_the_paths() {
    let g = two_arms();
    let err = g.plug("Arm.tx").unwrap_err().to_string();
    assert!(err.contains("ambiguous"));
    assert!(err.contains("|Body|L|Arm"));
    assert!(err.contains("|Body|R|Arm"));
    assert!(g.find("Foot").unwrap().is_none());
    assert!(g.find("R|L|Arm").unwrap().is_none());
}

#[test]
fn partial_and_full_paths_resolve() {
    let mut g = two_arms();
    set(&mut g, "L|Arm.tx", 3.0).unwrap();
    assert!(approx(get(&mut g, "|Body|L|Arm.tx").as_float(), 3.0));
    assert!(approx(get(&mut g, "Body|R|Arm.tx").as_float(), 0.0));
    // Absolute must start at a root.
    assert!(g.find("|L|Arm").unwrap().is_none());

    // The shortest unique suffix grows as needed.
    make(&mut g, "transform", "Other", None);
    make(&mut g, "transform", "L", Some("Other"));
    make(&mut g, "transform", "Arm", Some("|Other|L"));
    assert_eq!(g.path(id(&g, "|Body|L|Arm")), "Body|L|Arm");
    assert_eq!(g.path(id(&g, "|Body|R|Arm")), "R|Arm");
    assert_eq!(g.full_path(id(&g, "|Other|L|Arm")), "|Other|L|Arm");
    assert_eq!(g.node(id(&g, "Other|L|Arm")).unwrap().name(), "Arm");
    assert!(matches!(g.find("L|Arm"), Err(GraphError::Ambiguous(_))));
    assert_eq!(g.find_all("L|Arm").len(), 2);
}

#[test]
fn results_re_resolve_to_the_same_node() {
    let mut g = two_arms();
    for n in g.nodes() {
        assert_eq!(g.find_all(&g.path(n)).len(), 1);
    }
    let sel = [id(&g, "L|Arm"), id(&g, "R|Arm")];
    g.select(&sel, SelectMode::Replace);
    assert_eq!(g.full_path(g.selection()[0]), "|Body|L|Arm");
    assert_eq!(g.full_path(g.selection()[1]), "|Body|R|Arm");
    assert_eq!(g.plug_name(&plug(&g, "|Body|R|Arm.tx")), "R|Arm.translateX");
    let r = id(&g, "R|Arm");
    assert_eq!(g.rename(r, "Arm").unwrap(), "Arm");
    assert_eq!(g.rename(r, "Wing").unwrap(), "Wing");
}

// Locks, user attrs, arrays, deletion: graph behaviour the command-level tests cover indirectly.

#[test]
fn locks_reject_edits_and_undo() {
    let mut g = new_scene();
    loc(&mut g, "a");
    let tx = plug(&g, "a.tx");
    g.set_locked(&tx, true).unwrap();
    assert!(set(&mut g, "a.tx", 2.0).is_err());
    // A whole Vec3 set fails only when the locked component would change.
    assert!(set(&mut g, "a.translate", DVec3::new(0.0, 4.0, 0.0)).is_ok());
    assert!(set(&mut g, "a.translate", DVec3::new(1.0, 4.0, 0.0)).is_err());
    assert_eq!(
        g.keep_locked(&plug(&g, "a.translate"), DVec3::new(9.0, 9.0, 9.0).into())
            .as_vec3(),
        DVec3::new(0.0, 9.0, 9.0)
    );
    assert!(g.any_locked(&plug(&g, "a.translate")) && !g.is_locked(&plug(&g, "a.translate")));
    g.undo(); // the translate set
    g.undo(); // the lock
    assert!(set(&mut g, "a.tx", 2.0).is_ok());
}

#[test]
fn user_attrs_add_edit_move_delete_with_undo() {
    let mut g = new_scene();
    let a = loc(&mut g, "a");
    loc(&mut g, "b");
    let base = g.node(a).unwrap().ty().attrs.len();
    let i = g
        .chunk("addAttr", |g| {
            g.add_attr(a, user("weight", PlugType::Float, None, None))
        })
        .unwrap();
    let j = g
        .chunk("addAttr", |g| {
            g.add_attr(a, user("tint", PlugType::Vec3, None, None))
        })
        .unwrap();
    assert_eq!((i, j), (base, base + 1));
    assert!(g.plug("a.tintY").is_ok());
    // The registered type is untouched.
    assert_eq!(ty("transform").attrs.len(), base);
    set(&mut g, "a.weight", 2.5).unwrap();
    g.connect(&plug(&g, "b.tx"), &plug(&g, "a.weight"), false)
        .unwrap();

    // Moving keeps the connection on the attr.
    g.move_attr(a, i, j).unwrap();
    assert_eq!(g.source(&plug(&g, "a.weight")), Some(plug(&g, "b.tx")));
    assert_eq!(plug(&g, "a.weight").attr as usize, j);
    g.undo();
    assert_eq!(plug(&g, "a.weight").attr as usize, i);

    // Retyping drops the connection and converts the stored value (nothing was pulled through, so it is still 2.5).
    let mut def = g.attr(&plug(&g, "a.weight")).clone();
    def.ty = PlugType::String;
    def.default_value = PlugValue::String(String::new());
    g.edit_attr(&plug(&g, "a.weight"), def).unwrap();
    assert!(g.source(&plug(&g, "a.weight")).is_none());
    assert_eq!(get(&mut g, "a.weight").as_str(), "2.5");

    g.delete_attr(&plug(&g, "a.weight")).unwrap();
    assert!(g.plug("a.weight").is_err());
    g.undo();
    assert_eq!(get(&mut g, "a.weight").as_str(), "2.5");
    assert!(g.delete_attr(&plug(&g, "a.translate")).is_err());
}

#[test]
fn array_elements_set_connect_and_remove() {
    let mut g = new_scene();
    let a = loc(&mut g, "a");
    loc(&mut g, "b");
    let mut def = user("inputs", PlugType::Float, None, None);
    def.flags |= flags::ARRAY;
    g.add_attr(a, def).unwrap();
    let arr = plug(&g, "a.inputs");
    g.set_elements(&arr, &[1.0.into(), 2.0.into()]).unwrap();
    assert_eq!(get(&mut g, "a.inputs[1]").as_float(), 2.0);
    // Connecting to the whole array appends an element.
    let dst = g.connect(&plug(&g, "b.ty"), &arr, false).unwrap();
    assert_eq!(dst.element, 2);
    set(&mut g, "b.ty", 7.0).unwrap();
    assert_eq!(get(&mut g, "a.inputs[2]").as_float(), 7.0);
    g.remove_element(&dst).unwrap();
    assert_eq!(g.elements(&arr).len(), 2);
    g.undo();
    assert_eq!(g.elements(&arr).len(), 3);
}

#[test]
fn deleting_a_parent_deletes_children_and_undo_restores_everything() {
    let mut g = new_scene();
    loc(&mut g, "p");
    loc(&mut g, "c");
    loc(&mut g, "other");
    parent(&mut g, "c", "p");
    g.connect(&plug(&g, "other.tx"), &plug(&g, "c.ty"), false)
        .unwrap();
    set(&mut g, "other.tx", 4.0).unwrap();
    let p = id(&g, "p");
    g.select(&[p], SelectMode::Replace);
    g.chunk("delete", |g| g.delete_node(p)).unwrap();
    assert!(g.find("c").unwrap().is_none());
    assert!(g.selection().is_empty());
    g.undo();
    let c = id(&g, "c");
    assert_eq!(g.parent(c), id(&g, "p"));
    assert_eq!(g.source(&plug(&g, "c.ty")), Some(plug(&g, "other.tx")));
    assert_eq!(g.selection(), [id(&g, "p")]);
    assert!(approx(world_t(&mut g, "c").y, 4.0));
    g.redo();
    assert!(g.find("p").unwrap().is_none());
}

#[test]
fn listeners_hear_changes_and_batches_collapse_to_reset() {
    use std::sync::Mutex;
    let mut g = new_scene();
    let heard = Arc::new(Mutex::new(Vec::new()));
    let h = heard.clone();
    g.add_listener(Arc::new(move |_, c, _| h.lock().unwrap().push(c)), false);
    loc(&mut g, "a");
    assert_eq!(
        heard.lock().unwrap().as_slice(),
        [aztek_scene::Change::Created]
    );
    heard.lock().unwrap().clear();
    g.batch(|g| {
        loc(g, "b");
        set(g, "b.tx", 1.0).unwrap();
    });
    assert_eq!(
        heard.lock().unwrap().as_slice(),
        [aztek_scene::Change::Reset]
    );
}
