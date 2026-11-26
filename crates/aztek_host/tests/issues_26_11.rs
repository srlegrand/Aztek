//! Issues of 26/11/2026.

use aztek_host::Core;
use serde_json::{json, Value};

fn mel(c: &mut Core, src: &str) -> Value {
    let v = c.mel(src);
    let errors: Vec<String> = c.take_log().as_array().unwrap().iter().filter(|l| l["kind"] == 2).map(|l| l["text"].as_str().unwrap().to_string()).collect();
    assert!(errors.is_empty(), "{src}: {errors:?}");
    v
}

fn chain(c: &mut Core) {
    mel(c, "select -cl; joint -p 0 0 0; joint -p 1 0 0; joint -p 2 0 0; joint -p 3 0 0; joint -p 4 0 0; select -cl;");
}

#[test]
fn the_ik_tools_are_contexts_with_the_references_settings() {
    // #317: Create IK Handle and Create IK Spline Handle are tools, with their own settings.
    let mut c = Core::new();
    mel(&mut c, "setToolTo ikSplineHandleContext;");
    assert_eq!(mel(&mut c, "currentCtx;"), json!("ikSplineHandleContext"));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -ccv ikSplineHandleContext;"), json!(true));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -roc ikSplineHandleContext;"), json!(true));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -pcv ikSplineHandleContext;"), json!(true));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -scv ikSplineHandleContext;"), json!(true));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -cra ikSplineHandleContext;"), json!(false));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -rtm ikSplineHandleContext;"), json!(false));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -ns ikSplineHandleContext;"), json!(1));
    assert_eq!(mel(&mut c, "ikSplineHandleCtx -q -tws ikSplineHandleContext;"), json!("linear"));
    mel(&mut c, "setToolTo ikHandleContext;");
    assert_eq!(mel(&mut c, "currentCtx;"), json!("ikHandleContext"));
    assert_eq!(mel(&mut c, "ikHandleCtx -q -stH ikHandleContext;"), json!("ikRPsolver"));
}

#[test]
fn the_spline_tool_makes_its_handle_with_its_settings() {
    // #317: the start and end joints clicked make a spline handle with what the tool settings say.
    let mut c = Core::new();
    chain(&mut c);
    mel(&mut c, "ikSplineHandleCtx -e -ns 3 -tws easeOut -rtm 1 ikSplineHandleContext;");
    let call = aztek_host::tools::ik_handle_call("joint1", "joint5", None, true);
    assert_eq!(call["solver"], json!("ikSplineSolver"));
    assert_eq!(call["numSpans"], json!(3));
    let r = c.exec("ikHandle", call).unwrap();
    assert_eq!(r.as_array().unwrap().len(), 3, "{r}");
    assert_eq!(mel(&mut c, "getAttr curve1.spans;"), json!(3));
    assert_eq!(mel(&mut c, "getAttr ikHandle1.twistType;"), json!(2));
    assert_eq!(mel(&mut c, "getAttr ikHandle1.rootTwistMode;"), json!(true));
}

#[test]
fn the_spline_tool_takes_a_clicked_curve_when_it_makes_none() {
    // #317: Auto create curve off, the third click names the curve.
    let mut c = Core::new();
    chain(&mut c);
    mel(&mut c, "curve -d 3 -n path -p 0 1 0 -p 1 1 0 -p 2 1 0 -p 3 1 0 -p 4 1 0;");
    mel(&mut c, "ikSplineHandleCtx -e -ccv 0 ikSplineHandleContext;");
    let call = aztek_host::tools::ik_handle_call("joint1", "joint5", Some("path"), true);
    assert_eq!(call["createCurve"], json!(false));
    c.exec("ikHandle", call).unwrap();
    assert_eq!(mel(&mut c, "ikHandle -q -c ikHandle1;"), json!("path"));
    // Snap curve to root is on in the tool: the curve came down to the start joint.
    let t = mel(&mut c, "getAttr path.ty;");
    assert!((t.as_f64().unwrap() + 1.0).abs() < 1e-6, "{t}");
}

#[test]
fn the_ik_handle_tool_makes_its_handle_with_its_solver() {
    // #317: the IK Handle Tool's current solver.
    let mut c = Core::new();
    chain(&mut c);
    mel(&mut c, "ikHandleCtx -e -stH ikSCsolver ikHandleContext;");
    let call = aztek_host::tools::ik_handle_call("joint1", "joint3", None, false);
    c.exec("ikHandle", call).unwrap();
    assert_eq!(mel(&mut c, "ikHandle -q -sol ikHandle1;"), json!("ikSCsolver"));
}
