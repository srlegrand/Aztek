//! The joint tool with the reference's settings: orientation, degrees of freedom, an IK handle at the end of the chain,
//! and a point to place the joint even looking above the horizon.

use aztek_host::camera::CamKind;
use aztek_host::view::{finish_joint_chain, place_joint};
use aztek_host::viewport::{snap_curves, Viewport};
use aztek_host::Core;
use glam::{DVec2, DVec3};
use serde_json::{json, Value};

fn mel(c: &mut Core, src: &str) -> Value {
    c.mel(src)
}

fn axis(c: &mut Core, j: &str, k: usize) -> [f64; 3] {
    let m: Vec<f64> = serde_json::from_value(mel(c, &format!("xform -q -ws -m {j};"))).unwrap();
    let v = [m[k * 4], m[k * 4 + 1], m[k * 4 + 2]];
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    [v[0] / l, v[1] / l, v[2] / l]
}

fn near(a: [f64; 3], b: [f64; 3]) {
    assert!((0..3).all(|k| (a[k] - b[k]).abs() < 1e-6), "{a:?} != {b:?}");
}

fn chain(c: &mut Core, pts: &[[f64; 3]]) {
    mel(c, "select -cl;");
    for p in pts {
        place_joint(c, DVec3::from_array(*p));
    }
}

#[test]
fn the_tool_orients_as_its_settings_say_and_ends_line_up_with_their_parent() {
    let mut c = Core::new();
    // Primary Y, secondary Z toward world -X.
    mel(&mut c, "jointCtx -e -ajo yzx -sao xdown jointContext;");
    chain(&mut c, &[[0.0, 0.0, 0.0], [0.0, 2.0, 1.0], [0.0, 4.0, 0.0]]);
    let b = [0.0, 2.0, 1.0];
    let l = 5f64.sqrt();
    near(axis(&mut c, "joint1", 1), [b[0] / l, b[1] / l, b[2] / l]);
    assert!(axis(&mut c, "joint1", 2)[0] < -0.99);
    // The end joint takes its parent's orientation.
    assert_eq!(mel(&mut c, "getAttr joint3.jointOrient;"), json!([0.0, 0.0, 0.0]));
    near(axis(&mut c, "joint3", 0), axis(&mut c, "joint2", 0));
    // Orient Joint to World: every joint lined up with the world.
    mel(&mut c, "jointCtx -e -ajo none jointContext;");
    chain(&mut c, &[[5.0, 0.0, 0.0], [6.0, 3.0, 0.0], [7.0, 4.0, 1.0]]);
    for j in ["joint4", "joint5", "joint6"] {
        near(axis(&mut c, j, 0), [1.0, 0.0, 0.0]);
        near(axis(&mut c, j, 1), [0.0, 1.0, 0.0]);
    }
}

#[test]
fn degrees_of_freedom_and_an_ik_handle_down_the_chain() {
    let mut c = Core::new();
    mel(&mut c, "jointCtx -e -dJ \"xz\" -ikh true jointContext;");
    chain(&mut c, &[[0.0, 0.0, 0.0], [2.0, 0.0, -1.0], [4.0, 0.0, 0.0]]);
    assert_eq!(mel(&mut c, "getAttr joint2.jointTypeY;"), json!(false));
    assert_eq!(mel(&mut c, "getAttr joint2.jointTypeX;"), json!(true));
    finish_joint_chain(&mut c);
    assert_eq!(mel(&mut c, "ls -type ikHandle;"), json!(["ikHandle1"]));
    assert_eq!(mel(&mut c, "ikHandle -q -sj ikHandle1;"), json!("joint1"));
    assert_eq!(mel(&mut c, "ikHandle -q -ee ikHandle1;"), json!("effector1"));
    // Without the setting, Enter clears the selection for the next chain.
    mel(&mut c, "jointCtx -e -ikh false jointContext;");
    chain(&mut c, &[[0.0, 0.0, 5.0], [2.0, 0.0, 5.0]]);
    finish_joint_chain(&mut c);
    assert_eq!(mel(&mut c, "ls -sl;"), json!([]));
    assert_eq!(mel(&mut c, "ls -type ikHandle;"), json!(["ikHandle1"]));
}

#[test]
fn looking_above_the_horizon_the_joint_goes_in_front_of_the_camera() {
    let mut vp = Viewport::new(CamKind::Persp);
    vp.rect = [0.0, 0.0, 800.0, 800.0];
    vp.cam.coi = DVec3::new(0.0, 5.0, 0.0);
    vp.cam.yaw = 0.0;
    vp.cam.pitch = 0.0;
    vp.cam.distance = 20.0;
    // Near the top of the view, the ray rises and never meets the ground in front.
    let p = vp.place_point(DVec2::new(400.0, 50.0));
    let eye = vp.cam.eye();
    let to_coi = (vp.cam.coi - eye).normalize();
    assert!((p - eye).dot(to_coi) > 1.0, "{p:?} is behind the camera at {eye:?}");
    assert!(p.y > 5.0);
}

#[test]
fn with_c_held_the_joint_goes_on_the_curve_under_the_cursor() {
    // #273: the joint tool snaps to a curve as it is placed, not only once a joint is moved.
    let mut c = Core::new();
    mel(&mut c, "curve -d 1 -p -5 3 0 -p 5 3 0;");
    mel(&mut c, "select -cl;");
    // The view draws, and snaps to, the evaluated curve.
    c.s.graph.evaluate();
    let mut vp = Viewport::new(CamKind::Persp);
    vp.rect = [0.0, 0.0, 800.0, 800.0];
    vp.cam.coi = DVec3::ZERO;
    vp.cam.yaw = 0.3;
    vp.cam.pitch = -0.6;
    vp.cam.distance = 30.0;
    let curves = snap_curves(&c.s.graph);
    let on = DVec3::new(2.0, 3.0, 0.0);
    let px = vp.screen().project(on).unwrap() + DVec2::new(0.0, 5.0);
    // Without a snap the click lands on the ground, below the curve.
    assert!(vp.place_joint_point(&curves, px).y.abs() < 1e-6);
    vp.snap = 3;
    let at = vp.place_joint_point(&curves, px);
    assert!((at.y - 3.0).abs() < 1e-6 && at.z.abs() < 1e-6, "{at:?} is off the curve");
    assert!((at.x - 2.0).abs() < 0.5, "{at:?} is not near the cursor");
    place_joint(&mut c, at);
    let t: Vec<f64> = serde_json::from_value(mel(&mut c, "xform -q -ws -t joint1;")).unwrap();
    assert!((t[1] - 3.0).abs() < 1e-6, "{t:?}");
    // Far from any curve the joint goes where it would without the snap.
    let far = DVec2::new(5.0, 795.0);
    assert_eq!(vp.place_joint_point(&curves, far), vp.place_point(far));
}
