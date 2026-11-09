//! Skeletons, constraints and IK: numbers from the reference's documented behaviour.

use aztek_scene::Session;
use serde_json::{json, Value};

fn run(s: &mut Session, name: &str, call: Value) -> Value {
    s.run(name, call).unwrap_or_else(|e| panic!("{name}: {e}"))
}

fn vec3(v: &Value) -> [f64; 3] {
    let a = v.as_array().unwrap_or_else(|| panic!("not a vector: {v}"));
    [a[0].as_f64().unwrap(), a[1].as_f64().unwrap(), a[2].as_f64().unwrap()]
}

fn at(s: &mut Session, o: &str) -> [f64; 3] {
    vec3(&run(s, "xform", json!({"args": [o], "query": true, "worldSpace": true, "translation": true})))
}

fn attr(s: &mut Session, p: &str) -> Value {
    run(s, "getAttr", json!({"args": [p]}))
}

fn attr3(s: &mut Session, p: &str) -> [f64; 3] {
    vec3(&attr(s, p))
}

fn attrf(s: &mut Session, p: &str) -> f64 {
    attr(s, p).as_f64().unwrap_or_else(|| panic!("{p} is not a number"))
}

fn set(s: &mut Session, p: &str, v: Value) {
    run(s, "setAttr", json!({"args": [p, v]}));
}

#[track_caller]
fn near(a: [f64; 3], b: [f64; 3]) {
    assert!((0..3).all(|i| (a[i] - b[i]).abs() < 1e-4), "{a:?} is not {b:?}");
}

#[track_caller]
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-4, "{a} is not {b}");
}

fn q(s: &mut Session, cmd: &str, flag: &str, args: Value) -> Value {
    run(s, cmd, json!({"args": args, "query": true, "queried": [flag]}))
}

fn loc(s: &mut Session, name: &str, p: [f64; 3]) {
    run(s, "spaceLocator", json!({"name": name}));
    run(s, "move", json!({"args": [name], "to": p}));
}

/// Joints from the points, each under the last, named joint1, joint2...
fn chain_of(s: &mut Session, points: &[[f64; 3]]) {
    run(s, "select", json!({"clear": true}));
    for p in points {
        run(s, "joint", json!({"position": p}));
    }
}

/// A local axis of an object in world space.
fn axis(s: &mut Session, o: &str, k: usize) -> [f64; 3] {
    let m = run(s, "xform", json!({"args": [o], "query": true, "worldSpace": true, "matrix": true}));
    let m: Vec<f64> = m.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    let v = [m[k * 4], m[k * 4 + 1], m[k * 4 + 2]];
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

fn axis_length(s: &mut Session, o: &str, k: usize) -> f64 {
    let m = run(s, "xform", json!({"args": [o], "query": true, "worldSpace": true, "matrix": true}));
    let m: Vec<f64> = m.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    (m[k * 4] * m[k * 4] + m[k * 4 + 1] * m[k * 4 + 1] + m[k * 4 + 2] * m[k * 4 + 2]).sqrt()
}

fn dist(a: [f64; 3], b: [f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

fn roundtrip(s: &mut Session) -> Session {
    s.graph.evaluate();
    let text = serde_json::to_string(&aztek_scene::scene_file::serialize(&s.graph, false)).unwrap();
    let mut t = Session::new();
    let warnings = aztek_scene::scene_file::deserialize(&mut t.graph, &serde_json::from_str(&text).unwrap()).unwrap();
    assert!(warnings.is_empty(), "{warnings:?}");
    t
}

// ---------------------------------------------------------------------------------------------------------------------
// joint

#[test]
fn joint_flags_create_query_and_edit() {
    let mut s = Session::new();
    run(&mut s, "joint", json!({"position": [1, 2, 3], "name": "a", "radius": 2.5, "orientation": [0, 0, 90], "rotationOrder": "zyx", "degreeOfFreedom": "xz", "scaleCompensate": false}));
    near(vec3(&q(&mut s, "joint", "position", json!(["a"]))), [1.0, 2.0, 3.0]);
    assert_eq!(q(&mut s, "joint", "radius", json!(["a"])), json!([2.5]));
    near(vec3(&q(&mut s, "joint", "orientation", json!(["a"]))), [0.0, 0.0, 90.0]);
    assert_eq!(q(&mut s, "joint", "rotationOrder", json!(["a"])), json!("zyx"));
    assert_eq!(q(&mut s, "joint", "degreeOfFreedom", json!(["a"])), json!("xz"));
    assert_eq!(q(&mut s, "joint", "scaleCompensate", json!(["a"])), json!(false));
    assert_eq!(attr(&mut s, "a.jointTypeY"), json!(false));

    run(&mut s, "joint", json!({"args": ["a"], "edit": true, "degreeOfFreedom": "xyz", "angleX": 30, "angleY": 40, "angleZ": 50, "stiffnessY": 12, "limitX": [-10, 20], "limitSwitchX": [true, false], "segmentScaleCompensate": true}));
    close(q(&mut s, "joint", "angleY", json!(["a"])).as_f64().unwrap(), 40.0);
    near(attr3(&mut s, "a.rotate"), [30.0, 40.0, 50.0]);
    close(attrf(&mut s, "a.stiffnessY"), 12.0);
    assert_eq!(q(&mut s, "joint", "limitX", json!(["a"])), json!([-10.0, 20.0]));
    assert_eq!(q(&mut s, "joint", "limitSwitchX", json!(["a"])), json!([true, false]));
    close(attrf(&mut s, "a.minRotXLimit"), -10.0);
    assert_eq!(attr(&mut s, "a.minRotXLimitEnable"), json!(true));
    assert_eq!(attr(&mut s, "a.maxRotXLimitEnable"), json!(false));
    assert_eq!(attr(&mut s, "a.segmentScaleCompensate"), json!(true));

    assert_eq!(run(&mut s, "joint", json!({"exists": "a"})), json!(true));
    assert_eq!(run(&mut s, "joint", json!({"exists": "nothing"})), json!(false));
    assert_eq!(q(&mut s, "joint", "exists", json!(["nothing"])), json!(false));
}

#[test]
fn joint_labels_and_attributes_have_the_reference_names() {
    let mut s = Session::new();
    run(&mut s, "joint", json!({"name": "a"}));
    assert_eq!(attr(&mut s, "a.side"), json!(0));
    assert_eq!(attr(&mut s, "a.type"), json!(0));
    assert_eq!(attr(&mut s, "a.drawStyle"), json!(0));
    assert_eq!(attr(&mut s, "a.drawLabel"), json!(false));
    set(&mut s, "a.side", json!(1));
    set(&mut s, "a.type", json!(18));
    set(&mut s, "a.otherType", json!("wrist"));
    assert_eq!(attr(&mut s, "a.otp"), json!("wrist"));
    close(attrf(&mut s, "a.preferredAngleX"), 0.0);
    close(attrf(&mut s, "a.pax"), 0.0);
    near(attr3(&mut s, "a.is"), [1.0, 1.0, 1.0]);
}

#[test]
fn preferred_angles_set_and_assume() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    set(&mut s, "joint1.rotateZ", json!(25.0));
    set(&mut s, "joint2.rotateY", json!(10.0));
    run(&mut s, "joint", json!({"args": ["joint1"], "edit": true, "setPreferredAngles": true, "children": true}));
    near(attr3(&mut s, "joint1.preferredAngle"), [0.0, 0.0, 25.0]);
    near(attr3(&mut s, "joint2.pa"), [0.0, 10.0, 0.0]);
    set(&mut s, "joint1.rotateZ", json!(0.0));
    set(&mut s, "joint2.rotateY", json!(0.0));
    run(&mut s, "joint", json!({"args": ["joint1"], "edit": true, "assumePreferredAngles": true, "children": true}));
    near(attr3(&mut s, "joint1.rotate"), [0.0, 0.0, 25.0]);
    near(attr3(&mut s, "joint2.rotate"), [0.0, 10.0, 0.0]);
}

#[test]
fn zero_scale_orient_moves_nothing() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 1.0, 0.0]]);
    set(&mut s, "joint1.rotateAxis", json!([20.0, 0.0, 10.0]));
    set(&mut s, "joint1.rotate", json!([0.0, 30.0, 0.0]));
    let (x, tip) = (axis(&mut s, "joint1", 0), at(&mut s, "joint2"));
    run(&mut s, "joint", json!({"args": ["joint1"], "edit": true, "zeroScaleOrient": true}));
    near(attr3(&mut s, "joint1.rotateAxis"), [0.0, 0.0, 0.0]);
    near(axis(&mut s, "joint1", 0), x);
    near(at(&mut s, "joint2"), tip);
    assert_eq!(attr3(&mut s, "joint1.rotate"), [0.0, 30.0, 0.0]);
}

#[test]
fn orienting_clears_rotate_axis_and_keeps_the_joints_placed() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, -1.0], [4.0, 1.0, 0.0]]);
    set(&mut s, "joint1.rotateAxis", json!([0.0, 45.0, 0.0]));
    let (a, b) = (at(&mut s, "joint2"), at(&mut s, "joint3"));
    run(&mut s, "joint", json!({"args": ["joint1"], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup", "children": true}));
    near(attr3(&mut s, "joint1.rotateAxis"), [0.0, 0.0, 0.0]);
    near(at(&mut s, "joint2"), a);
    near(at(&mut s, "joint3"), b);
    // X aims at the child, Y as near world up as it can.
    let to = [a[0], a[1], a[2]];
    let len = dist(to, [0.0; 3]);
    near(axis(&mut s, "joint1", 0), [to[0] / len, to[1] / len, to[2] / len]);
    assert!(axis(&mut s, "joint1", 1)[1] > 0.5);
    // The end joint takes its parent's orientation.
    near(axis(&mut s, "joint3", 0), axis(&mut s, "joint2", 0));
    near(attr3(&mut s, "joint3.rotate"), [0.0, 0.0, 0.0]);
    // None zeroes jointOrient; the child stays.
    run(&mut s, "joint", json!({"args": ["joint3"], "edit": true, "orientJoint": "none"}));
    near(attr3(&mut s, "joint3.jointOrient"), [0.0, 0.0, 0.0]);
    near(at(&mut s, "joint3"), b);
    run(&mut s, "orientJoint", json!({"args": ["xyz", "joint1"], "secondaryAxisOrient": "zup", "zeroScaleOrient": true}));
    near(at(&mut s, "joint3"), b);
    // rotateAxis had turned the children before the first orient, so the aim is where the child is now.
    let y = axis(&mut s, "joint1", 1);
    let dot = y[0] * to[0] + y[1] * to[1] + y[2] * to[2];
    close(dot, 0.0);
    close(y[1], 0.0);
    assert!(y[2] > 0.0, "{y:?}");
}

// ---------------------------------------------------------------------------------------------------------------------
// Segment scale compensate

#[test]
fn segment_scale_compensate_keeps_the_child_scale() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    assert_eq!(attr(&mut s, "joint2.segmentScaleCompensate"), json!(true));
    let wired = run(&mut s, "listConnections", json!({"args": ["joint2.inverseScale"], "source": true, "plugs": true}));
    assert_eq!(wired, json!(["joint1.scale"]));
    set(&mut s, "joint1.scale", json!([2.0, 3.0, 4.0]));
    near(attr3(&mut s, "joint2.inverseScale"), [2.0, 3.0, 4.0]);
    // The child's place scales with the parent; its own axes don't.
    near(at(&mut s, "joint2"), [4.0, 0.0, 0.0]);
    close(axis_length(&mut s, "joint2", 0), 1.0);
    close(axis_length(&mut s, "joint2", 1), 1.0);
    close(axis_length(&mut s, "joint2", 2), 1.0);
    near(at(&mut s, "joint3"), [6.0, 0.0, 0.0]);
    // Off: the child inherits.
    set(&mut s, "joint2.segmentScaleCompensate", json!(false));
    close(axis_length(&mut s, "joint2", 0), 2.0);
    close(axis_length(&mut s, "joint2", 1), 3.0);
    near(at(&mut s, "joint3"), [8.0, 0.0, 0.0]);
    assert!(s.undo());
    close(axis_length(&mut s, "joint2", 0), 1.0);
}

#[test]
fn inverse_scale_follows_the_parent() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0, 5, 0], "name": "other"}));
    run(&mut s, "parent", json!({"args": ["joint2", "other"]}));
    let wired = run(&mut s, "listConnections", json!({"args": ["joint2.inverseScale"], "source": true, "plugs": true}));
    assert_eq!(wired, json!(["other.scale"]));
    assert!(s.undo());
    let wired = run(&mut s, "listConnections", json!({"args": ["joint2.inverseScale"], "source": true, "plugs": true}));
    assert_eq!(wired, json!(["joint1.scale"]));
    run(&mut s, "parent", json!({"args": ["joint2"], "world": true}));
    let wired = run(&mut s, "listConnections", json!({"args": ["joint2.inverseScale"], "source": true, "plugs": true}));
    assert_eq!(wired, json!([]));
    // A copied chain is wired to its own parents.
    run(&mut s, "select", json!({"clear": true}));
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    let copy = run(&mut s, "duplicate", json!({"args": ["joint3"]}))[0].as_str().unwrap().to_string();
    let child = run(&mut s, "listRelatives", json!({"args": [copy.clone()], "children": true, "path": true}));
    let child = child[0].as_str().unwrap().to_string();
    let wired = run(&mut s, "listConnections", json!({"args": [format!("{child}.inverseScale")], "source": true, "plugs": true}));
    assert_eq!(wired, json!([format!("{copy}.scale")]), "{child}");
}

#[test]
fn compensated_joint_keeps_its_place_when_moved_by_matrix() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    set(&mut s, "joint1.scale", json!([2.0, 2.0, 2.0]));
    run(&mut s, "move", json!({"args": ["joint2"], "to": [3, 1, 0]}));
    near(at(&mut s, "joint2"), [3.0, 1.0, 0.0]);
    run(&mut s, "xform", json!({"args": ["joint2"], "worldSpace": true, "rotation": [0, 0, 90]}));
    near(axis(&mut s, "joint2", 0), [0.0, 1.0, 0.0]);
}

// ---------------------------------------------------------------------------------------------------------------------
// mirrorJoint

fn left_arm(s: &mut Session) {
    run(s, "select", json!({"clear": true}));
    run(s, "joint", json!({"position": [2, 0, 0], "name": "L_arm"}));
    run(s, "joint", json!({"position": [4, 1, 0], "name": "L_elbow"}));
    run(s, "joint", json!({"position": [6, 0, 1], "name": "L_hand"}));
    run(s, "joint", json!({"args": ["L_arm"], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup", "children": true}));
}

#[test]
fn mirror_behavior_gives_opposite_motion() {
    let mut s = Session::new();
    left_arm(&mut s);
    let made = run(&mut s, "mirrorJoint", json!({"args": ["L_arm"], "mirrorYZ": true, "mirrorBehavior": true, "searchReplace": ["L_", "R_"]}));
    assert_eq!(made, json!(["R_arm", "R_elbow", "R_hand"]));
    near(at(&mut s, "R_arm"), [-2.0, 0.0, 0.0]);
    near(at(&mut s, "R_elbow"), [-4.0, 1.0, 0.0]);
    near(at(&mut s, "R_hand"), [-6.0, 0.0, 1.0]);
    // The mirrored axes are the source's reflected and reversed, so the child sits along negative X.
    assert!(attr3(&mut s, "R_elbow.translate")[0] < 0.0);
    let (x, mx) = (axis(&mut s, "L_arm", 0), axis(&mut s, "R_arm", 0));
    near(mx, [x[0], -x[1], -x[2]]);
    // Equal rotate values make a mirrored pose: rotation is opposite as seen in the world.
    for (attr_name, v) in [("rotateZ", 30.0), ("rotateY", -20.0), ("rotateX", 15.0)] {
        set(&mut s, &format!("L_arm.{attr_name}"), json!(v));
        set(&mut s, &format!("R_arm.{attr_name}"), json!(v));
        set(&mut s, &format!("L_elbow.{attr_name}"), json!(v));
        set(&mut s, &format!("R_elbow.{attr_name}"), json!(v));
    }
    let (l, r) = (at(&mut s, "L_hand"), at(&mut s, "R_hand"));
    near(r, [-l[0], l[1], l[2]]);
}

#[test]
fn mirror_orientation_keeps_the_world_axes() {
    let mut s = Session::new();
    left_arm(&mut s);
    let made = run(&mut s, "mirrorJoint", json!({"args": ["L_arm"], "mirrorYZ": true, "searchReplace": ["L_", "R_"]}));
    assert_eq!(made.as_array().unwrap().len(), 3);
    near(at(&mut s, "R_elbow"), [-4.0, 1.0, 0.0]);
    for (a, b) in [("L_arm", "R_arm"), ("L_elbow", "R_elbow"), ("L_hand", "R_hand")] {
        for k in 0..3 {
            near(axis(&mut s, a, k), axis(&mut s, b, k));
        }
    }
}

#[test]
fn mirror_other_planes_names_and_parent() {
    let mut s = Session::new();
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0, 0, 0], "name": "root"}));
    run(&mut s, "joint", json!({"position": [1, 2, 3], "name": "limb"}));
    run(&mut s, "joint", json!({"position": [2, 4, 3], "name": "tip"}));
    set(&mut s, "limb.side", json!(1));
    let made = run(&mut s, "mirrorJoint", json!({"args": ["limb"], "mirrorXY": true}));
    let names: Vec<String> = made.as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
    assert_eq!(names.len(), 2);
    assert_ne!(names[0], "limb");
    near(at(&mut s, &names[0]), [1.0, 2.0, -3.0]);
    near(at(&mut s, &names[1]), [2.0, 4.0, -3.0]);
    // A new hierarchy beside the old, under the same parent.
    let kids = run(&mut s, "listRelatives", json!({"args": ["root"], "children": true}));
    assert_eq!(kids.as_array().unwrap().len(), 2, "{kids}");
    assert_eq!(attr(&mut s, &format!("{}.side", names[0])), json!(2));
    let made = run(&mut s, "mirrorJoint", json!({"args": ["limb"], "mirrorXZ": true}));
    near(at(&mut s, made[0].as_str().unwrap()), [1.0, -2.0, 3.0]);
    assert!(s.run("mirrorJoint", json!({"args": ["limb"]})).is_err(), "a plane is required");
    let count = run(&mut s, "ls", json!({"type": "joint"})).as_array().unwrap().len();
    assert!(s.undo());
    assert_eq!(run(&mut s, "ls", json!({"type": "joint"})).as_array().unwrap().len(), count - 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// Editing skeletons

#[test]
fn insert_and_remove_joints() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 1.0, 0.0]]);
    let new = run(&mut s, "insertJoint", json!({"args": ["joint1"]}));
    assert_eq!(new, json!("joint4"));
    let parent = run(&mut s, "listRelatives", json!({"args": ["joint2"], "parent": true}));
    assert_eq!(parent, json!(["joint4"]));
    near(at(&mut s, "joint4"), [0.0, 0.0, 0.0]);
    near(at(&mut s, "joint2"), [2.0, 0.0, 0.0]);
    near(at(&mut s, "joint3"), [4.0, 1.0, 0.0]);
    assert!(s.undo());
    let parent = run(&mut s, "listRelatives", json!({"args": ["joint2"], "parent": true}));
    assert_eq!(parent, json!(["joint1"]));
    assert!(s.redo());
    run(&mut s, "joint", json!({"args": ["joint4"], "edit": true, "component": true, "position": [1, 0, 0]}));
    near(at(&mut s, "joint4"), [1.0, 0.0, 0.0]);
    near(at(&mut s, "joint2"), [2.0, 0.0, 0.0]);

    run(&mut s, "removeJoint", json!({"args": ["joint2"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["joint2"]})), json!(false));
    let parent = run(&mut s, "listRelatives", json!({"args": ["joint3"], "parent": true}));
    assert_eq!(parent, json!(["joint4"]));
    near(at(&mut s, "joint3"), [4.0, 1.0, 0.0]);
    assert!(s.undo());
    near(at(&mut s, "joint2"), [2.0, 0.0, 0.0]);
    near(at(&mut s, "joint3"), [4.0, 1.0, 0.0]);
}

#[test]
fn joints_with_skin_refuse_insert_and_remove() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({}));
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    run(&mut s, "skinCluster", json!({"args": ["joint1", "pCube1"]}));
    assert!(s.run("insertJoint", json!({"args": ["joint1"]})).is_err());
    assert!(s.run("removeJoint", json!({"args": ["joint2"]})).is_err());
}

#[test]
fn connect_and_disconnect_skeletons() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0]]);
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [5, 5, 0], "name": "a"}));
    run(&mut s, "joint", json!({"position": [6, 5, 0], "name": "b"}));
    // Parent mode: where it stands.
    run(&mut s, "connectJoint", json!({"args": ["a", "joint2"], "parentMode": true}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["a"], "parent": true})), json!(["joint2"]));
    near(at(&mut s, "a"), [5.0, 5.0, 0.0]);
    near(at(&mut s, "b"), [6.0, 5.0, 0.0]);
    assert!(s.undo());
    // Connect mode: the skeleton comes to the joint.
    run(&mut s, "connectJoint", json!({"args": ["a", "joint2"], "connectMode": true}));
    near(at(&mut s, "a"), [2.0, 0.0, 0.0]);
    near(at(&mut s, "b"), [3.0, 0.0, 0.0]);
    // Disconnect: a root again, and the old parent keeps its bone with a new end joint.
    let r = run(&mut s, "disconnectJoint", json!({"args": ["a"]}));
    assert_eq!(r[1], json!("a"));
    let leaf = r[0].as_str().unwrap().to_string();
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["a"], "parent": true})), json!([]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": [leaf.clone()], "parent": true})), json!(["joint2"]));
    near(at(&mut s, &leaf), [2.0, 0.0, 0.0]);
    near(at(&mut s, "a"), [2.0, 0.0, 0.0]);
    near(at(&mut s, "b"), [3.0, 0.0, 0.0]);
    assert!(s.run("disconnectJoint", json!({"args": ["joint1"]})).is_err());
}

#[test]
fn reroot_turns_the_skeleton_around() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 1.0, 0.0], [5.0, 3.0, 0.0]]);
    run(&mut s, "joint", json!({"args": ["joint1"], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup", "children": true}));
    let before: Vec<[f64; 3]> = ["joint1", "joint2", "joint3", "joint4"].iter().map(|j| at(&mut s, j)).collect();
    let x3 = axis(&mut s, "joint3", 0);
    run(&mut s, "reroot", json!({"args": ["joint3"]}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["joint3"], "parent": true})), json!([]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["joint2"], "parent": true})), json!(["joint3"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["joint1"], "parent": true})), json!(["joint2"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["joint4"], "parent": true})), json!(["joint3"]));
    for (j, b) in ["joint1", "joint2", "joint3", "joint4"].iter().zip(&before) {
        near(at(&mut s, j), *b);
    }
    near(axis(&mut s, "joint3", 0), x3);
    assert!(s.undo());
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["joint3"], "parent": true})), json!(["joint2"]));
    near(at(&mut s, "joint4"), before[3]);
}

// ---------------------------------------------------------------------------------------------------------------------
// Constraints

fn two_targets(s: &mut Session) {
    loc(s, "a", [2.0, 0.0, 0.0]);
    loc(s, "b", [0.0, 4.0, 0.0]);
    run(s, "polyCube", json!({}));
}

#[test]
fn parent_constraint_with_two_targets_sits_between() {
    let mut s = Session::new();
    two_targets(&mut s);
    set(&mut s, "a.rotateZ", json!(0.0));
    set(&mut s, "b.rotateZ", json!(60.0));
    let r = run(&mut s, "parentConstraint", json!({"args": ["a", "b", "pCube1"]}));
    assert_eq!(r, json!(["pCube1_parentConstraint1"]));
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    close(attr3(&mut s, "pCube1.rotate")[2], 30.0);
    // Weights of 3 and 1.
    run(&mut s, "parentConstraint", json!({"args": ["a", "pCube1"], "edit": true, "weight": 3.0}));
    near(at(&mut s, "pCube1"), [1.5, 1.0, 0.0]);
    close(run(&mut s, "parentConstraint", json!({"args": ["a", "pCube1"], "query": true, "queried": ["weight"]})).as_f64().unwrap(), 3.0);
    assert_eq!(q(&mut s, "parentConstraint", "weight", json!(["pCube1_parentConstraint1"])), json!([3.0, 1.0]));
    // The weight attrs are real: getAttr, setAttr, keyable.
    assert_eq!(q(&mut s, "parentConstraint", "weightAliasList", json!(["pCube1"])), json!(["aW0", "bW1"]));
    assert_eq!(q(&mut s, "parentConstraint", "targetList", json!(["pCube1_parentConstraint1"])), json!(["a", "b"]));
    close(attrf(&mut s, "pCube1_parentConstraint1.bW1"), 1.0);
    set(&mut s, "pCube1_parentConstraint1.aW0", json!(0.0));
    near(at(&mut s, "pCube1"), [0.0, 4.0, 0.0]);
    let keyable = run(&mut s, "listAttr", json!({"args": ["pCube1_parentConstraint1"], "keyable": true}));
    assert!(keyable.as_array().unwrap().iter().any(|x| x == "bW1"), "{keyable}");
}

#[test]
fn weights_are_per_target_at_creation_and_add_as_targets() {
    let mut s = Session::new();
    two_targets(&mut s);
    run(&mut s, "pointConstraint", json!({"args": ["a", "pCube1"], "weight": 1.0}));
    near(at(&mut s, "pCube1"), [2.0, 0.0, 0.0]);
    // A second call adds to the same constraint.
    let r = run(&mut s, "pointConstraint", json!({"args": ["b", "pCube1"], "weight": 3.0}));
    assert_eq!(r, json!(["pCube1_pointConstraint1"]));
    near(at(&mut s, "pCube1"), [0.5, 3.0, 0.0]);
    // Following moves.
    run(&mut s, "move", json!({"args": ["b"], "to": [0, 8, 0]}));
    near(at(&mut s, "pCube1"), [0.5, 6.0, 0.0]);
}

#[test]
fn weights_below_one_blend_toward_the_rest_position() {
    let mut s = Session::new();
    loc(&mut s, "a", [4.0, 0.0, 0.0]);
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [0, 2, 0]}));
    run(&mut s, "pointConstraint", json!({"args": ["a", "pCube1"]}));
    near(attr3(&mut s, "pCube1_pointConstraint1.restTranslate"), [0.0, 2.0, 0.0]);
    assert_eq!(attr(&mut s, "pCube1_pointConstraint1.enableRestPosition"), json!(true));
    set(&mut s, "pCube1_pointConstraint1.aW0", json!(0.0));
    near(at(&mut s, "pCube1"), [0.0, 2.0, 0.0]);
    set(&mut s, "pCube1_pointConstraint1.aW0", json!(0.5));
    near(at(&mut s, "pCube1"), [2.0, 1.0, 0.0]);
    // Rest off: a lone target counts in full whatever its weight, and zero leaves the object alone.
    set(&mut s, "pCube1_pointConstraint1.enableRestPosition", json!(false));
    near(at(&mut s, "pCube1"), [4.0, 0.0, 0.0]);
    set(&mut s, "pCube1_pointConstraint1.aW0", json!(0.0));
    near(at(&mut s, "pCube1"), [4.0, 0.0, 0.0]);
}

#[test]
fn node_state_holds_the_outputs() {
    let mut s = Session::new();
    loc(&mut s, "a", [4.0, 0.0, 0.0]);
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "pointConstraint", json!({"args": ["a", "pCube1"]}));
    near(at(&mut s, "pCube1"), [4.0, 0.0, 0.0]);
    set(&mut s, "pCube1_pointConstraint1.nodeState", json!(2));
    run(&mut s, "move", json!({"args": ["a"], "to": [9, 0, 0]}));
    near(at(&mut s, "pCube1"), [4.0, 0.0, 0.0]);
    set(&mut s, "pCube1_pointConstraint1.nodeState", json!(0));
    near(at(&mut s, "pCube1"), [9.0, 0.0, 0.0]);
}

#[test]
fn skip_flags_leave_axes_alone() {
    let mut s = Session::new();
    loc(&mut s, "a", [1.0, 2.0, 3.0]);
    set(&mut s, "a.rotate", json!([10.0, 20.0, 30.0]));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "pointConstraint", json!({"args": ["a", "pCube1"], "skip": "x"}));
    near(at(&mut s, "pCube1"), [0.0, 2.0, 3.0]);
    // A skipped axis stays free.
    set(&mut s, "pCube1.tx", json!(7.0));
    near(at(&mut s, "pCube1"), [7.0, 2.0, 3.0]);

    run(&mut s, "polySphere", json!({}));
    run(&mut s, "parentConstraint", json!({"args": ["a", "pSphere1"], "skipTranslate": ["x", "y"], "skipRotate": "z"}));
    near(at(&mut s, "pSphere1"), [0.0, 0.0, 3.0]);
    close(attr3(&mut s, "pSphere1.rotate")[2], 0.0);
    assert!(attr3(&mut s, "pSphere1.rotate")[1].abs() > 1.0);

    run(&mut s, "polyCone", json!({}));
    run(&mut s, "scaleConstraint", json!({"args": ["a", "pCone1"], "skip": ["y"]}));
    set(&mut s, "a.scale", json!([2.0, 3.0, 4.0]));
    near(attr3(&mut s, "pCone1.scale"), [2.0, 1.0, 4.0]);

    run(&mut s, "polyTorus", json!({}));
    assert!(s.run("pointConstraint", json!({"args": ["a", "pTorus1"], "skip": ["x", "y", "z"]})).is_err());
}

#[test]
fn maintain_offset_and_offset_flags() {
    let mut s = Session::new();
    loc(&mut s, "a", [1.0, 1.0, 1.0]);
    set(&mut s, "a.rotateY", json!(90.0));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [3, 1, 1]}));
    set(&mut s, "pCube1.rotateX", json!(20.0));

    // Parent: per target, in the target's frame.
    run(&mut s, "parentConstraint", json!({"args": ["a", "pCube1"], "maintainOffset": true}));
    near(at(&mut s, "pCube1"), [3.0, 1.0, 1.0]);
    close(attr3(&mut s, "pCube1.rotate")[0], 20.0);
    near(attr3(&mut s, "pCube1_parentConstraint1.target[0].targetOffsetTranslate"), [0.0, 0.0, 2.0]);
    set(&mut s, "a.rotateY", json!(0.0));
    near(at(&mut s, "pCube1"), [1.0, 1.0, 3.0]);
    run(&mut s, "delete", json!({"args": ["pCube1_parentConstraint1"]}));

    // Point: one offset.
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "move", json!({"args": ["pSphere1"], "to": [5, 5, 5]}));
    run(&mut s, "pointConstraint", json!({"args": ["a", "pSphere1"], "maintainOffset": true}));
    near(vec3(&q(&mut s, "pointConstraint", "offset", json!(["pSphere1"]))), [4.0, 4.0, 4.0]);
    near(at(&mut s, "pSphere1"), [5.0, 5.0, 5.0]);
    run(&mut s, "pointConstraint", json!({"args": ["pSphere1"], "edit": true, "offset": [0, 1, 0]}));
    near(at(&mut s, "pSphere1"), [1.0, 2.0, 1.0]);

    // Orient.
    run(&mut s, "polyCone", json!({}));
    set(&mut s, "pCone1.rotate", json!([0.0, 0.0, 40.0]));
    run(&mut s, "orientConstraint", json!({"args": ["a", "pCone1"], "maintainOffset": true}));
    close(attr3(&mut s, "pCone1.rotate")[2], 40.0);
    set(&mut s, "a.rotateZ", json!(10.0));
    close(attr3(&mut s, "pCone1.rotate")[2], 50.0);

    // Scale.
    run(&mut s, "polyTorus", json!({}));
    set(&mut s, "pTorus1.scale", json!([2.0, 2.0, 2.0]));
    run(&mut s, "scaleConstraint", json!({"args": ["a", "pTorus1"], "maintainOffset": true}));
    near(attr3(&mut s, "pTorus1.scale"), [2.0, 2.0, 2.0]);
    set(&mut s, "a.scaleX", json!(3.0));
    close(attr3(&mut s, "pTorus1.scale")[0], 6.0);
    run(&mut s, "scaleConstraint", json!({"args": ["pTorus1"], "edit": true, "offset": [1, 1, 1]}));
    close(attr3(&mut s, "pTorus1.scale")[0], 3.0);
}

#[test]
fn constraint_under_a_transformed_parent() {
    let mut s = Session::new();
    loc(&mut s, "a", [1.0, 2.0, 3.0]);
    set(&mut s, "a.rotateZ", json!(45.0));
    run(&mut s, "group", json!({"empty": true, "name": "grp"}));
    run(&mut s, "xform", json!({"args": ["grp"], "translation": [5, 0, 0], "rotation": [0, 90, 0], "scale": [2, 2, 2]}));
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "parent", json!({"args": ["pCube1", "grp"]}));
    run(&mut s, "parentConstraint", json!({"args": ["a", "pCube1"]}));
    near(at(&mut s, "pCube1"), [1.0, 2.0, 3.0]);
    near(axis(&mut s, "pCube1", 0), axis(&mut s, "a", 0));
    // Moving the parent changes nothing in the world.
    run(&mut s, "move", json!({"args": ["grp"], "to": [-3, 1, 1]}));
    near(at(&mut s, "pCube1"), [1.0, 2.0, 3.0]);
    // Reparented: the constraint follows the new parent.
    run(&mut s, "parent", json!({"args": ["pCube1"], "world": true}));
    near(at(&mut s, "pCube1"), [1.0, 2.0, 3.0]);
}

#[test]
fn constraints_on_joints_respect_joint_orient() {
    let mut s = Session::new();
    loc(&mut s, "a", [0.0, 0.0, 0.0]);
    set(&mut s, "a.rotate", json!([30.0, 40.0, 50.0]));
    run(&mut s, "select", json!({"clear": true}));
    run(&mut s, "joint", json!({"position": [0, 0, 0], "orientation": [10, 0, 80], "name": "j"}));
    run(&mut s, "orientConstraint", json!({"args": ["a", "j"]}));
    for k in 0..3 {
        near(axis(&mut s, "j", k), axis(&mut s, "a", k));
    }
    // With a rotate order too.
    run(&mut s, "joint", json!({"position": [1, 0, 0], "orientation": [0, 30, 0], "rotationOrder": "zxy", "name": "k"}));
    run(&mut s, "orientConstraint", json!({"args": ["a", "k"]}));
    for k in 0..3 {
        near(axis(&mut s, "k", k), axis(&mut s, "a", k));
    }
    assert_eq!(attr(&mut s, "k_orientConstraint1.constraintRotateOrder"), json!(2));
    // Changing the order keeps the pose.
    set(&mut s, "k.rotateOrder", json!(5));
    near(axis(&mut s, "k", 0), axis(&mut s, "a", 0));
}

#[test]
fn aim_and_up_vectors_work() {
    let mut s = Session::new();
    loc(&mut s, "t", [5.0, 0.0, 0.0]);
    run(&mut s, "polyCube", json!({}));
    let r = run(&mut s, "aimConstraint", json!({"args": ["t", "pCube1"], "aimVector": [0, 1, 0], "upVector": [1, 0, 0], "worldUpType": "vector", "worldUpVector": [0, 0, 1]}));
    assert_eq!(r, json!(["pCube1_aimConstraint1"]));
    // Y at the target, X as near world Z as it can be.
    near(axis(&mut s, "pCube1", 1), [1.0, 0.0, 0.0]);
    near(axis(&mut s, "pCube1", 0), [0.0, 0.0, 1.0]);
    near(vec3(&q(&mut s, "aimConstraint", "aimVector", json!(["pCube1"]))), [0.0, 1.0, 0.0]);
    near(vec3(&q(&mut s, "aimConstraint", "upVector", json!(["pCube1"]))), [1.0, 0.0, 0.0]);
    assert_eq!(q(&mut s, "aimConstraint", "worldUpType", json!(["pCube1"])), json!("vector"));
    // Follows the target and the object.
    run(&mut s, "move", json!({"args": ["t"], "to": [0, 5, 0]}));
    near(axis(&mut s, "pCube1", 1), [0.0, 1.0, 0.0]);
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [0, 0, 5]}));
    let dir = (1.0_f64 / 2.0_f64).sqrt();
    near(axis(&mut s, "pCube1", 1), [0.0, dir, -dir]);
}

#[test]
fn aim_defaults_and_world_up_types() {
    let mut s = Session::new();
    loc(&mut s, "t", [0.0, 0.0, 5.0]);
    loc(&mut s, "up", [5.0, 0.0, 0.0]);
    run(&mut s, "polyCube", json!({}));
    run(&mut s, "aimConstraint", json!({"args": ["t", "pCube1"]}));
    // Local X at the target, Y up.
    near(axis(&mut s, "pCube1", 0), [0.0, 0.0, 1.0]);
    near(axis(&mut s, "pCube1", 1), [0.0, 1.0, 0.0]);
    // Object up: Y toward the object.
    run(&mut s, "aimConstraint", json!({"args": ["pCube1"], "edit": true, "worldUpType": "object", "worldUpObject": "up"}));
    assert_eq!(q(&mut s, "aimConstraint", "worldUpObject", json!(["pCube1"])), json!("up"));
    near(axis(&mut s, "pCube1", 1), [1.0, 0.0, 0.0]);
    run(&mut s, "move", json!({"args": ["up"], "to": [-5, 0, 0]}));
    near(axis(&mut s, "pCube1", 1), [-1.0, 0.0, 0.0]);
    // Object rotation up: the object's axes carry worldUpVector.
    run(&mut s, "aimConstraint", json!({"args": ["pCube1"], "edit": true, "worldUpType": "objectrotation", "worldUpVector": [0, 1, 0]}));
    near(axis(&mut s, "pCube1", 1), [0.0, 1.0, 0.0]);
    set(&mut s, "up.rotateZ", json!(90.0));
    near(axis(&mut s, "pCube1", 1), [-1.0, 0.0, 0.0]);
    // Scene: world Y whatever worldUpVector says.
    run(&mut s, "aimConstraint", json!({"args": ["pCube1"], "edit": true, "worldUpType": "scene", "worldUpVector": [1, 0, 0]}));
    near(axis(&mut s, "pCube1", 1), [0.0, 1.0, 0.0]);
    // None: the shortest turn from the rest pose.
    run(&mut s, "polySphere", json!({}));
    set(&mut s, "pSphere1.rotateX", json!(30.0));
    run(&mut s, "aimConstraint", json!({"args": ["t", "pSphere1"], "worldUpType": "none"}));
    near(axis(&mut s, "pSphere1", 0), [0.0, 0.0, 1.0]);
    assert!(s.run("aimConstraint", json!({"args": ["pSphere1"], "edit": true, "worldUpType": "sideways"})).is_err());
}

#[test]
fn aim_with_an_animated_position_and_maintain_offset() {
    let mut s = Session::new();
    loc(&mut s, "t", [5.0, 0.0, 0.0]);
    run(&mut s, "polyCube", json!({}));
    set(&mut s, "pCube1.rotateX", json!(25.0));
    run(&mut s, "aimConstraint", json!({"args": ["t", "pCube1"], "maintainOffset": true}));
    close(attr3(&mut s, "pCube1.rotate")[0], 25.0);
    close(attr3(&mut s, "pCube1.rotate")[1], 0.0);
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "tz", "time": 1, "value": 0}));
    run(&mut s, "setKeyframe", json!({"args": ["pCube1"], "attribute": "tz", "time": 10, "value": 5}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    let x = axis(&mut s, "pCube1", 0);
    let dir = (1.0_f64 / 2.0_f64).sqrt();
    near(x, [dir, 0.0, -dir]);
}

#[test]
fn interp_type_is_kept() {
    let mut s = Session::new();
    two_targets(&mut s);
    set(&mut s, "a.rotateY", json!(0.0));
    set(&mut s, "b.rotateY", json!(120.0));
    run(&mut s, "orientConstraint", json!({"args": ["a", "b", "pCube1"], "interpType": "shortest"}));
    assert_eq!(attr(&mut s, "pCube1_orientConstraint1.interpType"), json!(2));
    close(attr3(&mut s, "pCube1.rotate")[1], 60.0);
    // The long way round.
    set(&mut s, "pCube1_orientConstraint1.interpType", json!(3));
    near(axis(&mut s, "pCube1", 0), [-0.5, 0.0, 0.75_f64.sqrt()]);
    assert_eq!(q(&mut s, "orientConstraint", "interpType", json!(["pCube1"])), json!(3));
}

#[test]
fn removing_targets_and_deleting_constraints() {
    let mut s = Session::new();
    two_targets(&mut s);
    run(&mut s, "parentConstraint", json!({"args": ["a", "b", "pCube1"]}));
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    run(&mut s, "parentConstraint", json!({"args": ["a", "pCube1"], "edit": true, "remove": true}));
    near(at(&mut s, "pCube1"), [0.0, 4.0, 0.0]);
    assert_eq!(q(&mut s, "parentConstraint", "targetList", json!(["pCube1"])), json!(["b"]));
    // Indices stay as they were.
    assert_eq!(q(&mut s, "parentConstraint", "weightAliasList", json!(["pCube1"])), json!(["bW1"]));
    assert!(s.run("getAttr", json!({"args": ["pCube1_parentConstraint1.aW0"]})).is_err());
    assert!(s.undo());
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    close(attrf(&mut s, "pCube1_parentConstraint1.aW0"), 1.0);
    // The last target takes the constraint with it; the object holds its pose, free of connections.
    run(&mut s, "parentConstraint", json!({"args": ["a", "b", "pCube1"], "edit": true, "remove": true}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["pCube1_parentConstraint1"]})), json!(false));
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["pCube1.translate"], "source": true})), json!([]));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [9, 9, 9]}));
    near(at(&mut s, "pCube1"), [9.0, 9.0, 9.0]);
    assert!(s.undo());
    assert!(s.undo());
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    assert_eq!(q(&mut s, "parentConstraint", "targetList", json!(["pCube1"])), json!(["a", "b"]));
    // Delete holds the pose too.
    run(&mut s, "move", json!({"args": ["a"], "to": [4, 0, 0]}));
    near(at(&mut s, "pCube1"), [2.0, 2.0, 0.0]);
    run(&mut s, "delete", json!({"args": ["pCube1_parentConstraint1"]}));
    near(at(&mut s, "pCube1"), [2.0, 2.0, 0.0]);
    run(&mut s, "move", json!({"args": ["a"], "to": [40, 0, 0]}));
    near(at(&mut s, "pCube1"), [2.0, 2.0, 0.0]);
    assert!(s.undo());
    assert!(s.undo());
    near(at(&mut s, "pCube1"), [2.0, 2.0, 0.0]);
    run(&mut s, "move", json!({"args": ["b"], "to": [0, 0, 0]}));
    near(at(&mut s, "pCube1"), [2.0, 0.0, 0.0]);
}

#[test]
fn deleting_targets_and_the_driven_object() {
    let mut s = Session::new();
    two_targets(&mut s);
    run(&mut s, "pointConstraint", json!({"args": ["a", "b", "pCube1"]}));
    run(&mut s, "delete", json!({"args": ["a"]}));
    near(at(&mut s, "pCube1"), [0.0, 4.0, 0.0]);
    assert_eq!(q(&mut s, "pointConstraint", "targetList", json!(["pCube1"])), json!(["b"]));
    assert!(s.undo());
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    run(&mut s, "delete", json!({"args": ["pCube1"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["pCube1_pointConstraint1"]})), json!(false));
    assert!(s.undo());
    near(at(&mut s, "pCube1"), [1.0, 2.0, 0.0]);
    assert_eq!(run(&mut s, "objExists", json!({"args": ["pCube1_pointConstraint1"]})), json!(true));
}

#[test]
fn constraint_names_and_lookup() {
    let mut s = Session::new();
    two_targets(&mut s);
    assert_eq!(run(&mut s, "pointConstraint", json!({"args": ["a", "pCube1"], "name": "mine"})), json!(["mine"]));
    assert_eq!(q(&mut s, "pointConstraint", "name", json!(["pCube1"])), json!("mine"));
    let c = run(&mut s, "listConnections", json!({"args": ["pCube1.translate"], "source": true}));
    assert_eq!(c, json!(["mine"]));
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["mine"]})), json!("pointConstraint"));
    let t = run(&mut s, "listConnections", json!({"args": ["mine"], "source": true}));
    assert!(t.as_array().unwrap().iter().any(|x| x == "a"), "{t}");
    assert!(s.run("orientConstraint", json!({"args": ["pCube1"], "query": true, "queried": ["targetList"]})).is_err());
    assert!(s.run("pointConstraint", json!({"args": ["pCube1"]})).is_err());
    // Selection form: targets, then the driven object.
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "select", json!({"args": ["a", "pSphere1"]}));
    assert_eq!(run(&mut s, "scaleConstraint", json!({})), json!(["pSphere1_scaleConstraint1"]));
}

#[test]
fn constraints_save_and_load() {
    let mut s = Session::new();
    two_targets(&mut s);
    run(&mut s, "parentConstraint", json!({"args": ["a", "b", "pCube1"], "maintainOffset": true, "skipRotate": "x"}));
    run(&mut s, "parentConstraint", json!({"args": ["b", "pCube1"], "edit": true, "weight": 3.0}));
    run(&mut s, "polySphere", json!({}));
    run(&mut s, "aimConstraint", json!({"args": ["a", "pSphere1"], "aimVector": [0, 0, 1], "upVector": [0, 1, 0], "worldUpType": "object", "worldUpObject": "b"}));
    run(&mut s, "move", json!({"args": ["a"], "to": [3, 3, 3]}));
    let (p, x) = (at(&mut s, "pCube1"), axis(&mut s, "pSphere1", 2));
    let mut t = roundtrip(&mut s);
    near(at(&mut t, "pCube1"), p);
    near(axis(&mut t, "pSphere1", 2), x);
    assert_eq!(q(&mut t, "parentConstraint", "weightAliasList", json!(["pCube1"])), json!(["aW0", "bW1"]));
    close(attrf(&mut t, "pCube1_parentConstraint1.bW1"), 3.0);
    assert_eq!(q(&mut t, "aimConstraint", "worldUpObject", json!(["pSphere1"])), json!("b"));
    // Still alive.
    run(&mut t, "move", json!({"args": ["a"], "to": [4, 3, 3]}));
    assert!(dist(at(&mut t, "pCube1"), p) > 0.1);
}

#[test]
fn geometry_constraint_stays_on_the_surface() {
    let mut s = Session::new();
    run(&mut s, "polyCube", json!({"width": 2, "height": 2, "depth": 2}));
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [10, 0, 0]}));
    loc(&mut s, "p", [10.2, 5.0, 0.1]);
    let r = run(&mut s, "geometryConstraint", json!({"args": ["pCube1", "p"]}));
    assert_eq!(r, json!(["p_geometryConstraint1"]));
    near(at(&mut s, "p"), [10.2, 1.0, 0.1]);
    // Moving the object slides it on the surface.
    run(&mut s, "move", json!({"args": ["p"], "to": [20, 0.3, 0.2]}));
    near(at(&mut s, "p"), [11.0, 0.3, 0.2]);
    // The surface moves, the object follows.
    run(&mut s, "move", json!({"args": ["pCube1"], "to": [12, 0, 0]}));
    near(at(&mut s, "p"), [13.0, 0.3, 0.2]);
    // Stacked on a point constraint: the point target says where, the surface holds it.
    loc(&mut s, "q", [12.0, 9.0, 0.0]);
    run(&mut s, "spaceLocator", json!({"name": "ball"}));
    run(&mut s, "pointConstraint", json!({"args": ["q", "ball"]}));
    run(&mut s, "geometryConstraint", json!({"args": ["pCube1", "ball"]}));
    near(at(&mut s, "ball"), [12.0, 1.0, 0.0]);
    run(&mut s, "move", json!({"args": ["q"], "to": [12.5, -9, 0]}));
    near(at(&mut s, "ball"), [12.5, -1.0, 0.0]);
    run(&mut s, "polyCone", json!({}));
    assert!(s.run("geometryConstraint", json!({"args": ["q", "pCone1"]})).is_err(), "no geometry");
}

#[test]
fn normal_constraint_aims_along_the_surface() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"width": 10, "depth": 10}));
    loc(&mut s, "p", [1.0, 3.0, 1.0]);
    let r = run(&mut s, "normalConstraint", json!({"args": ["pPlane1", "p"], "aimVector": [0, 0, 1], "upVector": [0, 1, 0], "worldUpType": "vector", "worldUpVector": [1, 0, 0]}));
    assert_eq!(r, json!(["p_normalConstraint1"]));
    near(axis(&mut s, "p", 2), [0.0, 1.0, 0.0]);
    near(axis(&mut s, "p", 1), [1.0, 0.0, 0.0]);
    set(&mut s, "pPlane1.rotateX", json!(90.0));
    near(axis(&mut s, "p", 2), [0.0, 0.0, 1.0]);
}

#[test]
fn point_on_poly_follows_the_vertex() {
    let mut s = Session::new();
    run(&mut s, "polyPlane", json!({"width": 2, "depth": 2, "subdivisionsX": 2, "subdivisionsZ": 2}));
    let corner = "pPlane1.vtx[8]";
    let wanted = vec3(&run(&mut s, "xform", json!({"args": [corner], "query": true, "worldSpace": true, "translation": true})));
    loc(&mut s, "p", [7.0, 7.0, 7.0]);
    let r = run(&mut s, "pointOnPolyConstraint", json!({"args": [corner, "p"]}));
    assert_eq!(r, json!(["p_pointOnPolyConstraint1"]));
    near(at(&mut s, "p"), wanted);
    near(axis(&mut s, "p", 1), [0.0, 1.0, 0.0]);
    let u = attrf(&mut s, "p_pointOnPolyConstraint1.pPlane1U0");
    assert!((0.0..=1.0).contains(&u));
    close(attrf(&mut s, "p_pointOnPolyConstraint1.pPlane1W0"), 1.0);
    run(&mut s, "move", json!({"args": ["pPlane1"], "to": [0, 3, 0]}));
    near(at(&mut s, "p"), [wanted[0], wanted[1] + 3.0, wanted[2]]);
    // The UV attrs move it across the surface.
    set(&mut s, "p_pointOnPolyConstraint1.pPlane1U0", json!(0.5));
    set(&mut s, "p_pointOnPolyConstraint1.pPlane1V0", json!(0.5));
    near(at(&mut s, "p"), [0.0, 3.0, 0.0]);
}

// ---------------------------------------------------------------------------------------------------------------------
// IK

fn arm(s: &mut Session) {
    chain_of(s, &[[0.0, 0.0, 0.0], [2.0, 0.0, -0.5], [4.0, 0.0, 0.0]]);
    run(s, "joint", json!({"args": ["joint1"], "edit": true, "orientJoint": "xyz", "secondaryAxisOrient": "yup", "children": true}));
}

#[test]
fn handle_creation_makes_a_real_effector_and_keeps_the_pose() {
    let mut s = Session::new();
    arm(&mut s);
    let (b, c) = (at(&mut s, "joint2"), at(&mut s, "joint3"));
    let r = run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    assert_eq!(r, json!(["ikHandle1", "effector1"]));
    near(at(&mut s, "joint2"), b);
    near(at(&mut s, "joint3"), c);
    assert_eq!(run(&mut s, "nodeType", json!({"args": ["effector1"]})), json!("ikEffector"));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["effector1"], "parent": true})), json!(["joint2"]));
    near(at(&mut s, "effector1"), c);
    near(at(&mut s, "ikHandle1"), c);
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["ikHandle1"], "parent": true})), json!([]));
    // The solver nodes the reference keeps.
    assert_eq!(run(&mut s, "ls", json!({"type": "ikRPsolver"})), json!(["ikRPsolver"]));
    assert_eq!(run(&mut s, "ls", json!({"type": "ikSystem"})), json!(["ikSystem"]));
    assert_eq!(attr(&mut s, "ikHandle1.ikBlend"), json!(1.0));
    // Queries.
    assert_eq!(q(&mut s, "ikHandle", "startJoint", json!(["ikHandle1"])), json!("joint1"));
    assert_eq!(q(&mut s, "ikHandle", "endEffector", json!(["ikHandle1"])), json!("effector1"));
    assert_eq!(q(&mut s, "ikHandle", "jointList", json!(["ikHandle1"])), json!(["joint1", "joint2"]));
    assert_eq!(q(&mut s, "ikHandle", "solver", json!(["ikHandle1"])), json!("ikRPsolver"));
    assert_eq!(q(&mut s, "ikHandle", "exists", json!(["ikHandle1"])), json!(true));
    assert_eq!(run(&mut s, "ikHandle", json!({"exists": "nothing"})), json!(false));
    // The end joint keeps its own rotate.
    set(&mut s, "joint3.rotateZ", json!(33.0));
    close(attr3(&mut s, "joint3.rotate")[2], 33.0);
    near(at(&mut s, "joint3"), c);
    let (pv, x) = (attr3(&mut s, "ikHandle1.poleVector"), attr(&mut s, "ikHandle1.poleVectorX"));
    close(x.as_f64().unwrap(), pv[0]);
}

#[test]
fn handle_flags_query_and_edit() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3", "name": "hand", "priority": 3, "weight": 0.5, "positionWeight": 0.25, "sticky": "sticky"}));
    assert_eq!(q(&mut s, "ikHandle", "name", json!(["hand"])), json!("hand"));
    assert_eq!(q(&mut s, "ikHandle", "priority", json!(["hand"])), json!(3));
    close(q(&mut s, "ikHandle", "weight", json!(["hand"])).as_f64().unwrap(), 0.5);
    close(q(&mut s, "ikHandle", "positionWeight", json!(["hand"])).as_f64().unwrap(), 0.25);
    assert_eq!(q(&mut s, "ikHandle", "sticky", json!(["hand"])), json!("sticky"));
    run(&mut s, "ikHandle", json!({"args": ["hand"], "edit": true, "priority": 7, "twist": 10.0, "roll": 5.0, "snapHandleFlagToggle": false, "sticky": "off"}));
    assert_eq!(attr(&mut s, "hand.priority"), json!(7));
    close(attrf(&mut s, "hand.twist"), 10.0);
    close(attrf(&mut s, "hand.roll"), 5.0);
    assert_eq!(q(&mut s, "ikHandle", "snapHandleFlagToggle", json!(["hand"])), json!(false));
    assert_eq!(attr(&mut s, "hand.stickiness"), json!(0));
    close(attrf(&mut s, "hand.offset"), 0.0);
    run(&mut s, "ikHandle", json!({"args": ["hand"], "edit": true, "autoPriority": true}));
    assert_eq!(attr(&mut s, "hand.priority"), json!(1));
    assert!(s.run("ikHandle", json!({"args": ["hand"], "edit": true, "solver": "ikSplineSolver"})).is_err());
    assert!(s.run("ikHandle", json!({"startJoint": "joint3", "endEffector": "joint1"})).is_err());
}

#[test]
fn rp_reaches_with_the_bend_toward_the_pole() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 2, 1]}));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    near(at(&mut s, "joint1"), [0.0, 0.0, 0.0]);
    let lengths = |s: &mut Session| (dist(at(s, "joint1"), at(s, "joint2")), dist(at(s, "joint2"), at(s, "joint3")));
    let (l1, l2) = lengths(&mut s);
    close(l1, 4.25_f64.sqrt());
    close(l2, 4.25_f64.sqrt());
    // The pole says which way the middle goes.
    for pole in [[0.0, 0.0, 1.0], [0.0, 0.0, -1.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
        set(&mut s, "ikHandle1.poleVector", json!(pole));
        near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
        let b = at(&mut s, "joint2");
        let line = [2.0 / 3.0, 2.0 / 3.0, 1.0 / 3.0];
        let along = b[0] * line[0] + b[1] * line[1] + b[2] * line[2];
        let off = [b[0] - along * line[0], b[1] - along * line[1], b[2] - along * line[2]];
        let pole_along = pole[0] * line[0] + pole[1] * line[1] + pole[2] * line[2];
        let pp = [pole[0] - pole_along * line[0], pole[1] - pole_along * line[1], pole[2] - pole_along * line[2]];
        let dot = off[0] * pp[0] + off[1] * pp[1] + off[2] * pp[2];
        let (no, np) = (dist(off, [0.0; 3]), dist(pp, [0.0; 3]));
        close(dot / (no * np), 1.0);
    }
}

#[test]
fn twist_turns_the_plane_about_the_handle_line() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3, 0, 0]}));
    set(&mut s, "ikHandle1.poleVector", json!([0.0, 0.0, -1.0]));
    let b0 = at(&mut s, "joint2");
    assert!(b0[2] < 0.0 && b0[1].abs() < 1e-6, "{b0:?}");
    // A positive turn about +X takes -Z toward +Y.
    set(&mut s, "ikHandle1.twist", json!(90.0));
    let b = at(&mut s, "joint2");
    near(b, [b0[0], -b0[2], 0.0]);
    near(at(&mut s, "joint3"), [3.0, 0.0, 0.0]);
    set(&mut s, "ikHandle1.twist", json!(0.0));
    set(&mut s, "ikHandle1.roll", json!(180.0));
    near(at(&mut s, "joint2"), [b0[0], 0.0, -b0[2]]);
}

#[test]
fn out_of_reach_goes_straight_at_the_target() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 0.0, 0.0], [3.0, 1.0, 0.0], [4.0, 0.0, 0.0]]);
    for solver in ["ikRPsolver", "ikSCsolver"] {
        run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint5", "solver": solver, "name": "h"}));
        run(&mut s, "move", json!({"args": ["h"], "to": [100, 0, 0]}));
        let length = 4.0 * 2.0_f64.sqrt();
        near(at(&mut s, "joint5"), [length, 0.0, 0.0]);
        near(at(&mut s, "joint3"), [length / 2.0, 0.0, 0.0]);
        run(&mut s, "move", json!({"args": ["h"], "to": [0, -100, 100]}));
        let d = length / 2.0_f64.sqrt();
        near(at(&mut s, "joint5"), [0.0, -d, d]);
        run(&mut s, "delete", json!({"args": ["h"]}));
        run(&mut s, "setAttr", json!({"args": ["joint1.rotate", [0, 0, 0]]}));
        run(&mut s, "setAttr", json!({"args": ["joint2.rotate", [0, 0, 0]]}));
        run(&mut s, "setAttr", json!({"args": ["joint3.rotate", [0, 0, 0]]}));
        run(&mut s, "setAttr", json!({"args": ["joint4.rotate", [0, 0, 0]]}));
        for j in ["joint1", "joint2", "joint3", "joint4"] {
            set(&mut s, &format!("{j}.preferredAngle"), json!([0.0, 0.0, 0.0]));
        }
    }
    // A target on the start joint doesn't blow up.
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint5", "name": "h"}));
    run(&mut s, "move", json!({"args": ["h"], "to": [0, 0, 0]}));
    let p = at(&mut s, "joint5");
    assert!(p.iter().all(|x| x.is_finite()), "{p:?}");
}

#[test]
fn chains_longer_than_eight_joints_solve() {
    let mut s = Session::new();
    let points: Vec<[f64; 3]> = (0..13).map(|i| [i as f64, ((i % 2) as f64) * 0.25, 0.0]).collect();
    chain_of(&mut s, &points);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint13"}));
    assert_eq!(q(&mut s, "ikHandle", "jointList", json!(["ikHandle1"])).as_array().unwrap().len(), 12);
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [6, 5, 1]}));
    near(at(&mut s, "joint13"), [6.0, 5.0, 1.0]);
    near(at(&mut s, "joint1"), [0.0, 0.0, 0.0]);
    // Bones keep their lengths.
    let l = (1.0_f64 + 0.0625).sqrt();
    for i in 1..12 {
        close(dist(at(&mut s, &format!("joint{i}")), at(&mut s, &format!("joint{}", i + 1))), l);
    }
}

#[test]
fn multi_joint_rp_stays_in_the_plane_and_keeps_the_preferred_shape() {
    let mut s = Session::new();
    // A curled chain, bent in the XY plane.
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [1.0, 1.0, 0.0], [2.0, 1.5, 0.0], [3.0, 1.0, 0.0], [4.0, 0.0, 0.0]]);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint5"}));
    // The pole starts toward the bend, and the pose doesn't jump.
    near(at(&mut s, "joint3"), [2.0, 1.5, 0.0]);
    near(attr3(&mut s, "ikHandle1.poleVector"), [0.0, 1.0, 0.0]);
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3, 0, 0]}));
    near(at(&mut s, "joint5"), [3.0, 0.0, 0.0]);
    for j in ["joint2", "joint3", "joint4"] {
        let p = at(&mut s, j);
        close(p[2], 0.0);
        assert!(p[1] > 0.0, "{j} {p:?}");
    }
    // Symmetric shape stays symmetric.
    let (a, c) = (at(&mut s, "joint2"), at(&mut s, "joint4"));
    close(a[1], c[1]);
    close(a[0], 3.0 - c[0]);
    // Rotating the pole turns all of it.
    set(&mut s, "ikHandle1.poleVector", json!([0.0, 0.0, 1.0]));
    near(at(&mut s, "joint5"), [3.0, 0.0, 0.0]);
    for j in ["joint2", "joint3", "joint4"] {
        let p = at(&mut s, j);
        close(p[1], 0.0);
        assert!(p[2] > 0.0, "{j} {p:?}");
    }
}

#[test]
fn solve_is_deterministic_whatever_the_path() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [1.0, 0.5, 0.0], [2.0, 0.5, 0.0], [3.0, 0.0, 0.0]]);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint4"}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [1, 1, 1]}));
    let a = [at(&mut s, "joint2"), at(&mut s, "joint3"), attr3(&mut s, "joint2.rotate")];
    for p in [[-2.0, 1.0, 0.0], [0.5, 2.0, -1.0], [9.0, 9.0, 9.0], [0.0, 0.1, 0.0]] {
        run(&mut s, "move", json!({"args": ["ikHandle1"], "to": p}));
        at(&mut s, "joint4");
    }
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [1, 1, 1]}));
    near(at(&mut s, "joint2"), a[0]);
    near(at(&mut s, "joint3"), a[1]);
    near(attr3(&mut s, "joint2.rotate"), a[2]);
}

#[test]
fn preferred_angle_is_the_rest_pose() {
    let mut s = Session::new();
    // Straight chain: which way it bends comes from the preferred angle.
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    set(&mut s, "joint2.preferredAngleY", json!(-40.0));
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    assert_eq!(attr3(&mut s, "joint2.preferredAngle"), [0.0, -40.0, 0.0]);
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3, 0, 0]}));
    set(&mut s, "ikHandle1.poleVector", json!([0.0, 0.0, 1.0]));
    near(at(&mut s, "joint3"), [3.0, 0.0, 0.0]);
    let z = at(&mut s, "joint2")[2];
    assert!(z > 0.5, "{z}");
    // No preferred angle and a bent pose: the pose is the rest pose, and nothing moves.
    let mut t = Session::new();
    chain_of(&mut t, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    set(&mut t, "joint2.rotateZ", json!(50.0));
    let (b, c) = (at(&mut t, "joint2"), at(&mut t, "joint3"));
    run(&mut t, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    near(attr3(&mut t, "joint2.preferredAngle"), [0.0, 0.0, 50.0]);
    near(at(&mut t, "joint2"), b);
    near(at(&mut t, "joint3"), c);
    near(attr3(&mut t, "joint2.rotate"), [0.0, 0.0, 50.0]);
}

#[test]
fn sc_ignores_the_pole_and_follows_the_handle() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3", "solver": "ikSCsolver"}));
    assert_eq!(q(&mut s, "ikHandle", "solver", json!(["ikHandle1"])), json!("ikSCsolver"));
    assert_eq!(run(&mut s, "ls", json!({"type": "ikSCsolver"})), json!(["ikSCsolver"]));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 2, 1]}));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    let b = at(&mut s, "joint2");
    set(&mut s, "ikHandle1.poleVector", json!([1.0, 0.0, 0.0]));
    near(at(&mut s, "joint2"), b);
    set(&mut s, "ikHandle1.twist", json!(77.0));
    near(at(&mut s, "joint2"), b);
    // Turning the handle turns the chain about the line to it.
    set(&mut s, "ikHandle1.rotate", json!([0.0, 30.0, 0.0]));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    assert!(dist(at(&mut s, "joint2"), b) > 0.01);
    // Changing solver changes behaviour at once.
    set(&mut s, "ikHandle1.rotate", json!([0.0, 0.0, 0.0]));
    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "solver": "ikRPsolver"}));
    assert_eq!(q(&mut s, "ikHandle", "solver", json!(["ikHandle1"])), json!("ikRPsolver"));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
}

#[test]
fn ik_blend_and_the_global_switch() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    let fk_end = at(&mut s, "joint3");
    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "snapHandleFlagToggle": false}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 2, 1]}));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);

    set(&mut s, "ikHandle1.ikBlend", json!(0.0));
    near(at(&mut s, "joint3"), fk_end);
    // FK works while it's off: rotates set now are the joints' own.
    set(&mut s, "joint1.rotateZ", json!(90.0));
    assert!(at(&mut s, "joint3")[1] > 3.0);
    set(&mut s, "joint1.rotateZ", json!(0.0));
    set(&mut s, "ikHandle1.ikBlend", json!(0.5));
    let half = at(&mut s, "joint3");
    assert!(dist(half, fk_end) > 0.1 && dist(half, [2.0, 2.0, 1.0]) > 0.1, "{half:?}");
    set(&mut s, "ikHandle1.ikBlend", json!(1.0));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);

    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "disableHandles": true}));
    close(attrf(&mut s, "ikHandle1.ikBlend"), 0.0);
    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "enableHandles": true}));
    close(attrf(&mut s, "ikHandle1.ikBlend"), 1.0);

    assert_eq!(q(&mut s, "ikSystem", "solve", json!([])), json!(true));
    run(&mut s, "ikSystem", json!({"edit": true, "solve": false}));
    assert_eq!(q(&mut s, "ikSystem", "solve", json!([])), json!(false));
    near(at(&mut s, "joint3"), fk_end);
    run(&mut s, "ikSystem", json!({"edit": true, "solve": true}));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    set(&mut s, "ikHandle1.solverEnable", json!(false));
    near(at(&mut s, "joint3"), fk_end);
    set(&mut s, "ikHandle1.solverEnable", json!(true));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
}

#[test]
fn the_handle_snaps_to_the_end_while_ik_is_off() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    let start_end = at(&mut s, "joint3");
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 2, 1]}));
    set(&mut s, "ikHandle1.ikBlend", json!(0.0));
    s.graph.evaluate();
    near(at(&mut s, "ikHandle1"), start_end);
    set(&mut s, "joint1.rotateZ", json!(90.0));
    s.graph.evaluate();
    near(at(&mut s, "ikHandle1"), at(&mut s, "joint3"));
    // Back on: the chain is where FK left it.
    let end = at(&mut s, "joint3");
    let b = at(&mut s, "joint2");
    set(&mut s, "ikHandle1.ikBlend", json!(1.0));
    near(at(&mut s, "joint3"), end);
    let _ = b;
    // Snap to the effector on request.
    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "snapHandleFlagToggle": false}));
    run(&mut s, "ikSystem", json!({"edit": true, "snap": false}));
    assert_eq!(q(&mut s, "ikSystem", "snap", json!([])), json!(false));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [1, 1, 0]}));
    set(&mut s, "ikHandle1.ikBlend", json!(0.0));
    s.graph.evaluate();
    near(at(&mut s, "ikHandle1"), [1.0, 1.0, 0.0]);
    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "snapHandleToEffector": true}));
    near(at(&mut s, "ikHandle1"), at(&mut s, "joint3"));
}

#[test]
fn joint_limits_hold_under_ik() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, 0.0], [4.0, 0.0, 0.0]]);
    set(&mut s, "joint2.preferredAngleZ", json!(20.0));
    run(&mut s, "joint", json!({"args": ["joint2"], "edit": true, "limitZ": [0, 30], "limitSwitchZ": [true, true]}));
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    // Close in: the free solve wants a much sharper elbow.
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [1, 1, 0]}));
    let rz = attr3(&mut s, "joint2.rotate")[2];
    assert!(rz <= 30.0 + 1e-6 && rz >= -1e-6, "{rz}");
    // A pose inside the limit is reached exactly.
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3.9, 0.3, 0]}));
    near(at(&mut s, "joint3"), [3.9, 0.3, 0.0]);
    let rz = attr3(&mut s, "joint2.rotate")[2];
    assert!((0.0..=30.0).contains(&rz), "{rz}");
    // An axis the joint may not turn about stays at its preferred angle.
    run(&mut s, "joint", json!({"args": ["joint1"], "edit": true, "degreeOfFreedom": "z"}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 1, 2]}));
    let r = attr3(&mut s, "joint1.rotate");
    close(r[0], 0.0);
    close(r[1], 0.0);
}

#[test]
fn handles_follow_animation_a_moved_parent_and_edited_joints() {
    let mut s = Session::new();
    run(&mut s, "group", json!({"empty": true, "name": "rig"}));
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [2.0, 0.0, -0.5], [4.0, 0.0, 0.0]]);
    run(&mut s, "parent", json!({"args": ["joint1", "rig"]}));
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    // Keys on the handle keep driving.
    run(&mut s, "setKeyframe", json!({"args": ["ikHandle1"], "attribute": "ty", "time": 1, "value": 0}));
    run(&mut s, "setKeyframe", json!({"args": ["ikHandle1"], "attribute": "ty", "time": 10, "value": 0.5}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    near(at(&mut s, "joint3"), [4.0, 0.5, 0.0]);
    run(&mut s, "currentTime", json!({"args": [1]}));
    near(at(&mut s, "joint3"), [4.0, 0.0, 0.0]);
    // A moved parent carries the chain's start; the end stays at the handle.
    run(&mut s, "move", json!({"args": ["rig"], "to": [0, 0, 1]}));
    near(at(&mut s, "joint1"), [0.0, 0.0, 1.0]);
    near(at(&mut s, "joint3"), [4.0, 0.0, 0.0]);
    run(&mut s, "move", json!({"args": ["rig"], "to": [0, 0, 0]}));
    // A longer bone is picked up at once.
    set(&mut s, "joint3.translateX", json!(5.0));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [100, 0, 0]}));
    let reach = at(&mut s, "joint3")[0];
    assert!(reach > 7.0, "{reach}");
    assert!(s.undo());
    assert!(s.undo());
    // A preferred angle change is too; a joint moved by animation as well.
    set(&mut s, "joint3.translateX", json!(2.0));
    run(&mut s, "setKeyframe", json!({"args": ["joint2"], "attribute": "tx", "time": 1, "value": 2}));
    run(&mut s, "setKeyframe", json!({"args": ["joint2"], "attribute": "tx", "time": 10, "value": 4}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [100, 0, 0]}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    assert!(at(&mut s, "joint3")[0] > 5.5, "{:?}", at(&mut s, "joint3"));
}

#[test]
fn undo_redo_and_delete_of_handles() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 2, 1]}));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    assert!(s.undo());
    near(at(&mut s, "joint3"), [4.0, 0.0, 0.0]);
    assert!(s.redo());
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    let middle = at(&mut s, "joint2");
    // Deleting the handle takes its helpers; the joints hold their pose and are free.
    run(&mut s, "delete", json!({"args": ["ikHandle1"]}));
    for gone in ["ikHandle1", "effector1", "ikSolve1"] {
        assert_eq!(run(&mut s, "objExists", json!({"args": [gone]})), json!(false), "{gone}");
    }
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    near(at(&mut s, "joint2"), middle);
    assert_eq!(run(&mut s, "listConnections", json!({"args": ["joint1.rotate"], "source": true})), json!([]));
    set(&mut s, "joint1.rotate", json!([0.0, 0.0, 0.0]));
    assert!(s.undo());
    assert!(s.undo());
    assert_eq!(run(&mut s, "objExists", json!({"args": ["effector1"]})), json!(true));
    near(at(&mut s, "joint3"), [2.0, 2.0, 1.0]);
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3, 1, 0]}));
    near(at(&mut s, "joint3"), [3.0, 1.0, 0.0]);
    // So does deleting the effector, or a joint of the chain.
    run(&mut s, "delete", json!({"args": ["effector1"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["ikHandle1"]})), json!(false));
    assert!(s.undo());
    assert_eq!(run(&mut s, "objExists", json!({"args": ["ikHandle1"]})), json!(true));
    run(&mut s, "delete", json!({"args": ["joint3"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["ikHandle1"]})), json!(false));
    assert!(s.undo());
    near(at(&mut s, "joint3"), [3.0, 1.0, 0.0]);
}

#[test]
fn handles_save_and_load() {
    let mut s = Session::new();
    chain_of(&mut s, &(0..11).map(|i| [i as f64, ((i % 2) as f64) * 0.25, 0.0]).collect::<Vec<_>>());
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint11", "name": "h"}));
    run(&mut s, "setKeyframe", json!({"args": ["h"], "attribute": "ty", "time": 1, "value": 0}));
    run(&mut s, "setKeyframe", json!({"args": ["h"], "attribute": "ty", "time": 10, "value": 2}));
    run(&mut s, "currentTime", json!({"args": [10]}));
    set(&mut s, "h.twist", json!(25.0));
    let (mid, tip) = (at(&mut s, "joint6"), at(&mut s, "joint11"));
    let mut t = roundtrip(&mut s);
    run(&mut t, "currentTime", json!({"args": [10]}));
    near(at(&mut t, "joint6"), mid);
    near(at(&mut t, "joint11"), tip);
    assert_eq!(q(&mut t, "ikHandle", "endEffector", json!(["h"])), json!("effector1"));
    assert_eq!(q(&mut t, "ikHandle", "jointList", json!(["h"])).as_array().unwrap().len(), 10);
    close(attrf(&mut t, "h.twist"), 25.0);
    run(&mut t, "currentTime", json!({"args": [1]}));
    near(at(&mut t, "joint11"), [10.0, 0.0, 0.0]);
    set(&mut t, "h.ikBlend", json!(0.0));
    near(at(&mut t, "joint11"), [10.0, 0.0, 0.0]);
    run(&mut t, "delete", json!({"args": ["h"]}));
    assert_eq!(run(&mut t, "objExists", json!({"args": ["effector1"]})), json!(false));
}

#[test]
fn pole_vector_constraint_drives_the_pole() {
    let mut s = Session::new();
    arm(&mut s);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3"}));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [3, 0, 0]}));
    loc(&mut s, "pv", [1.0, 5.0, 0.0]);
    let r = run(&mut s, "poleVectorConstraint", json!({"args": ["pv", "ikHandle1"]}));
    assert_eq!(r, json!(["ikHandle1_poleVectorConstraint1"]));
    // The vector from the start joint to the locator.
    near(attr3(&mut s, "ikHandle1.poleVector"), [1.0, 5.0, 0.0]);
    assert!(at(&mut s, "joint2")[1] > 0.1);
    run(&mut s, "move", json!({"args": ["pv"], "to": [1, 0, 5]}));
    near(attr3(&mut s, "ikHandle1.poleVector"), [1.0, 0.0, 5.0]);
    assert!(at(&mut s, "joint2")[2] > 0.1);
    near(at(&mut s, "joint3"), [3.0, 0.0, 0.0]);
    // The start joint's translate counts.
    set(&mut s, "joint1.translateX", json!(1.0));
    s.graph.evaluate();
    near(attr3(&mut s, "ikHandle1.poleVector"), [0.0, 0.0, 5.0]);
    set(&mut s, "joint1.translateX", json!(0.0));
    // Two targets blend, the weight forms work.
    loc(&mut s, "pv2", [1.0, 0.0, -5.0]);
    run(&mut s, "poleVectorConstraint", json!({"args": ["pv2", "ikHandle1"]}));
    near(attr3(&mut s, "ikHandle1.poleVector"), [1.0, 0.0, 0.0]);
    run(&mut s, "poleVectorConstraint", json!({"args": ["pv2", "ikHandle1"], "edit": true, "weight": 0.0}));
    near(attr3(&mut s, "ikHandle1.poleVector"), [1.0, 0.0, 5.0]);
    assert_eq!(q(&mut s, "poleVectorConstraint", "targetList", json!(["ikHandle1"])), json!(["pv", "pv2"]));
    // Delete: the handle keeps the vector it had.
    run(&mut s, "delete", json!({"args": ["ikHandle1_poleVectorConstraint1"]}));
    near(attr3(&mut s, "ikHandle1.poleVector"), [1.0, 0.0, 5.0]);
    set(&mut s, "ikHandle1.poleVector", json!([0.0, 1.0, 0.0]));
    assert!(s.run("poleVectorConstraint", json!({"args": ["pv", "joint1"]})).is_err());
}

#[test]
fn skeleton_edits_carry_the_handles() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0], [1.0, 0.5, 0.0], [2.0, 0.5, 0.0], [3.0, 0.0, 0.0]]);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint4"}));
    // A new joint joins the chain.
    let new = run(&mut s, "insertJoint", json!({"args": ["joint2"]}));
    let list = q(&mut s, "ikHandle", "jointList", json!(["ikHandle1"]));
    assert_eq!(list, json!(["joint1", "joint2", new.clone(), "joint3"]));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [2, 1, 0]}));
    near(at(&mut s, "joint4"), [2.0, 1.0, 0.0]);
    // A removed one leaves it.
    run(&mut s, "removeJoint", json!({"args": [new.as_str().unwrap()]}));
    assert_eq!(q(&mut s, "ikHandle", "jointList", json!(["ikHandle1"])), json!(["joint1", "joint2", "joint3"]));
    near(at(&mut s, "joint4"), [2.0, 1.0, 0.0]);
    // The effector moves with the end joint's parent.
    run(&mut s, "removeJoint", json!({"args": ["joint3"]}));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["effector1"], "parent": true})), json!(["joint2"]));
    near(at(&mut s, "joint4"), [2.0, 1.0, 0.0]);
    // Breaking the chain deletes the handle.
    run(&mut s, "disconnectJoint", json!({"args": ["joint2"]}));
    assert_eq!(run(&mut s, "objExists", json!({"args": ["ikHandle1"]})), json!(false));
    assert!(s.undo());
    assert_eq!(run(&mut s, "objExists", json!({"args": ["ikHandle1"]})), json!(true));
    // A new end.
    run(&mut s, "ikHandle", json!({"args": ["ikHandle1"], "edit": true, "endEffector": "joint2"}));
    assert_eq!(q(&mut s, "ikHandle", "jointList", json!(["ikHandle1"])), json!(["joint1"]));
    assert_eq!(run(&mut s, "listRelatives", json!({"args": ["effector1"], "parent": true})), json!(["joint1"]));
    run(&mut s, "move", json!({"args": ["ikHandle1"], "to": [0, 9, 0]}));
    let p = at(&mut s, "joint2");
    close(p[0], 0.0);
    assert!(p[1] > 0.5);
}

#[test]
fn solver_nodes_and_custom_names() {
    let mut s = Session::new();
    arm(&mut s);
    assert_eq!(run(&mut s, "ikSolver", json!({"solverType": "ikSCsolver", "name": "mySolver", "epsilon": 0.01, "maxIterations": 10})), json!("mySolver"));
    assert_eq!(q(&mut s, "ikSolver", "solverType", json!(["mySolver"])), json!("ikSCsolver"));
    close(q(&mut s, "ikSolver", "epsilon", json!(["mySolver"])).as_f64().unwrap(), 0.01);
    run(&mut s, "ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3", "solver": "mySolver"}));
    assert_eq!(q(&mut s, "ikHandle", "solver", json!(["ikHandle1"])), json!("mySolver"));
    let b = at(&mut s, "joint2");
    set(&mut s, "ikHandle1.twist", json!(40.0));
    near(at(&mut s, "joint2"), b);
    let solvers = run(&mut s, "ikSystem", json!({"query": true, "queried": ["listSolvers"]}));
    assert_eq!(solvers, json!(["mySolver"]));
    assert!(s.run("ikHandle", json!({"startJoint": "joint1", "endEffector": "joint3", "solver": "nope"})).is_err());
    assert!(s.run("ikSolver", json!({"solverType": "nope"})).is_err());
}

#[test]
fn rotate_axis_shows_in_xform() {
    let mut s = Session::new();
    chain_of(&mut s, &[[0.0, 0.0, 0.0]]);
    set(&mut s, "joint1.rotateAxis", json!([1.0, 2.0, 3.0]));
    let v = run(&mut s, "xform", json!({"args": ["joint1"], "query": true, "rotateAxis": true}));
    near(vec3(&v), [1.0, 2.0, 3.0]);
}

fn wpos(s: &mut Session, j: &str) -> [f64; 3] {
    let v = s.run("xform", json!({"args": [j], "query": true, "worldSpace": true, "translation": true})).unwrap();
    [v[0].as_f64().unwrap(), v[1].as_f64().unwrap(), v[2].as_f64().unwrap()]
}

fn spline_chain(s: &mut Session) {
    s.run("select", json!({"clear": true})).unwrap();
    for x in 0..5 {
        s.run("joint", json!({"position": [x as f64, 0, 0]})).unwrap();
    }
}

#[test]
fn spline_ik_lays_the_chain_along_the_curve() {
    let mut s = Session::new();
    spline_chain(&mut s);
    s.run("curve", json!({"degree": 3, "name": "path", "point": [[0, 0, 0], [1, 1, 0], [2, 2, 0], [3, 2, 0], [4, 1, 0], [5, 0, 0]]})).unwrap();
    let r = s.run("ikHandle", json!({"startJoint": "joint1", "endEffector": "joint5", "solver": "ikSplineSolver", "curve": "path", "createCurve": false})).unwrap();
    assert_eq!(r, json!(["ikHandle1", "effector1", "path"]));
    s.graph.evaluate();
    // Every joint sits on the curve, a bone's length from the one before.
    let mut last = wpos(&mut s, "joint1");
    assert!(last.iter().all(|v| v.abs() < 1e-6), "{last:?}");
    for j in ["joint2", "joint3", "joint4", "joint5"] {
        let p = wpos(&mut s, j);
        let d = ((p[0] - last[0]).powi(2) + (p[1] - last[1]).powi(2) + (p[2] - last[2]).powi(2)).sqrt();
        assert!((d - 1.0).abs() < 1e-4, "{j} {d}");
        let near = s.run("createNode", json!({"args": ["nearestPointOnCurve"]})).unwrap();
        let near = near.as_str().unwrap().to_string();
        s.run("connectAttr", json!({"args": ["pathShape.worldSpace[0]", format!("{near}.inputCurve")]})).unwrap();
        s.run("setAttr", json!({"args": [format!("{near}.inPosition"), p[0], p[1], p[2]]})).unwrap();
        let q = s.run("getAttr", json!({"args": [format!("{near}.position")]})).unwrap();
        let off: f64 = (0..3).map(|k| (q[k].as_f64().unwrap() - p[k]).powi(2)).sum::<f64>().sqrt();
        assert!(off < 1e-3, "{j} is {off} off the curve");
        last = p;
    }
    assert!(wpos(&mut s, "joint3")[1] > 0.8, "{:?}", wpos(&mut s, "joint3"));
    // Moving a CV moves the chain.
    let before = wpos(&mut s, "joint5");
    s.run("xform", json!({"args": ["path.cv[3]"], "relative": true, "translation": [0, 0, 2]})).unwrap();
    let after = wpos(&mut s, "joint5");
    assert!((after[2] - before[2]).abs() > 0.05, "{before:?} {after:?}");
    assert!(s.undo());
    let back = wpos(&mut s, "joint5");
    assert!((back[2] - before[2]).abs() < 1e-6);
}

#[test]
fn spline_ik_makes_its_own_curve_and_twists() {
    let mut s = Session::new();
    spline_chain(&mut s);
    let r = s.run("ikHandle", json!({"startJoint": "joint1", "endEffector": "joint5", "solver": "ikSplineSolver", "numSpans": 2})).unwrap();
    assert_eq!(r[2], json!("curve1"));
    assert_eq!(s.run("getAttr", json!({"args": ["curve1.spans"]})).unwrap(), json!(2));
    s.graph.evaluate();
    // A straight curve through a straight chain leaves it where it was.
    let p = wpos(&mut s, "joint4");
    assert!((p[0] - 3.0).abs() < 1e-3 && p[1].abs() < 1e-3, "{p:?}");
    // Twist runs up the chain: the end joint's Y axis turns, the first's doesn't.
    s.run("setAttr", json!({"args": ["ikHandle1.twist", 90.0]})).unwrap();
    let axis = |s: &mut Session, j: &str| s.run("xform", json!({"args": [j], "query": true, "worldSpace": true, "matrix": true})).unwrap();
    let m1 = axis(&mut s, "joint1");
    let m4 = axis(&mut s, "joint4");
    assert!((m1[5].as_f64().unwrap() - 1.0).abs() < 1e-6, "{m1}");
    assert!(m4[5].as_f64().unwrap().abs() < 1e-6, "{m4}");
}

#[test]
fn tangent_constraint_aims_along_the_curve() {
    let mut s = Session::new();
    s.run("curve", json!({"degree": 1, "name": "rail", "point": [[0, 0, 0], [0, 10, 0]]})).unwrap();
    s.run("spaceLocator", json!({"name": "rider"})).unwrap();
    s.run("move", json!({"args": ["rider"], "to": [1, 5, 0]})).unwrap();
    let r = s.run("tangentConstraint", json!({"args": ["rail", "rider"], "aimVector": [1, 0, 0], "upVector": [0, 0, 1]})).unwrap();
    assert_eq!(r, json!(["rider_tangentConstraint1"]));
    s.graph.evaluate();
    let m = s.run("xform", json!({"args": ["rider"], "query": true, "worldSpace": true, "matrix": true})).unwrap();
    // Local X lies along the rail, which runs up Y.
    assert!((m[1].as_f64().unwrap().abs() - 1.0).abs() < 1e-6, "{m}");
    s.run("delete", json!({"args": ["rider_tangentConstraint1"]})).unwrap();
}
