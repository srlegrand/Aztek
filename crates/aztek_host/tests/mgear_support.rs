//! What rigging tools written for the reference need from Aztek, found installing mGear: setAttr writing a curve,
//! the attributes every node has, dirmap, the quaternion nodes, worldUpType by number and the plug-in names tools
//! load.

use aztek_host::Core;
use serde_json::json;

#[test]
fn what_rigging_tools_ask_for() {
    let mut c = Core::new();
    // A curve from setAttr -type nurbsCurve, as tools write a control's shape: degree 1, 3 CVs, open, not rational.
    c.mel("createNode transform -n ctl; createNode nurbsCurve -n ctlShape -p ctl;");
    c.mel("setAttr ctlShape.create -type nurbsCurve 1 2 0 no 3 3 0 1 2 3 0 0 0 1 0 0 2 1 0;");
    assert_eq!(c.mel("getAttr ctlShape.spans;"), json!(2));
    assert_eq!(c.mel("pointPosition ctlShape.cv[2];"), json!([2.0, 1.0, 0.0]));
    // From Python the knots come as a list and the CVs as points.
    c.mel("python(\"import maya.cmds as cmds; cmds.setAttr('ctlShape.create', 1, 1, 0, False, 3, [0, 1], 2, 2, (0, 0, 0), (0, 4, 0), type='nurbsCurve')\");");
    assert_eq!(c.mel("pointPosition ctlShape.cv[1];"), json!([0.0, 4.0, 0.0]));
    c.mel("setAttr ctlShape.alwaysDrawOnTop 1;");

    // Attributes every node has.
    for a in ["caching", "frozen", "isHistoricallyInteresting", "nodeState", "binMembership", "message"] {
        assert_eq!(c.mel(&format!("attributeQuery -n ctl -ex {a};")), json!(true), "{a}");
    }
    c.mel("setAttr ctl.ihi 0;");
    assert_eq!(c.mel("getAttr ctl.isHistoricallyInteresting;"), json!(0));

    // dirmap.
    c.mel("dirmap -en 1; dirmap -m \"D:/proj\" \"/mnt/proj\";");
    assert_eq!(c.mel("dirmap -cd \"D:/proj/tex/a.png\";"), json!("/mnt/proj/tex/a.png"));
    assert_eq!(c.mel("dirmap -cd \"E:/other.png\";"), json!("E:/other.png"));
    assert_eq!(c.mel("getApplicationVersionAsFloat;"), json!(2025.0));
    assert!(c.mel("allNodeTypes;").as_array().unwrap().iter().any(|t| t == "quatSlerp"));

    // The quaternion nodes the reference keeps in quatNodes, and the name loads.
    c.mel("loadPlugin quatNodes; loadPlugin matrixNodes;");
    c.mel("createNode quatProd -n qp; setAttr qp.input1Quat 0 0 0.7071068 0.7071068; setAttr qp.input2Quat 0 0 0.7071068 0.7071068;");
    let z = c.mel("getAttr qp.outputQuatZ;").as_f64().unwrap();
    assert!((z - 1.0).abs() < 1e-6, "90 then 90 about Z is 180: {z}");
    c.mel("createNode quatSlerp -n qs; setAttr qs.input2Quat 0 0 1 0; setAttr qs.inputT 0.5;");
    let z = c.mel("getAttr qs.outputQuatZ;").as_f64().unwrap();
    assert!((z - 0.7071068).abs() < 1e-5, "halfway to 180 is 90: {z}");

    // worldUpType by its number.
    c.mel("createNode transform -n tgt; createNode transform -n drv; aimConstraint -wut 2 -wuo tgt tgt drv;");
    assert_eq!(c.mel("getAttr drv_aimConstraint1.worldUpType;"), json!(2));
}
