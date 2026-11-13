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

fn nums(v: &serde_json::Value) -> Vec<f64> {
    match v {
        serde_json::Value::Array(a) => a.iter().flat_map(nums).collect(),
        v => vec![v.as_f64().unwrap_or_else(|| panic!("not a number: {v}"))],
    }
}

fn round_trip(c: &mut Core, tag: &str) {
    let path = std::env::temp_dir().join(format!("aztek-mgear-{tag}-{}.ma", std::process::id())).display().to_string().replace('\\', "/");
    c.mel(&format!("file -rename \"{path}\"; file -save -type \"mayaAscii\"; file -f -o \"{path}\";"));
    let _ = std::fs::remove_file(&path);
}

/// What building an mGear biped asked of Aztek, and what its rig needed to come back the same from a file.
#[test]
fn what_an_mgear_build_asks_for() {
    let mut c = Core::new();

    // Message attributes, and the controller tag of an object.
    c.mel("createNode transform -n rig; addAttr -ln rigGroups -at message -m rig;");
    assert_eq!(c.mel("attributeQuery -n rig -ex rigGroups;"), json!(true));
    c.mel("createNode transform -n ctl; controller ctl;");
    assert_eq!(c.mel("controller -q ctl;"), json!(["ctl_tag"]));
    assert_eq!(c.mel("controller -q rig;"), serde_json::Value::Null);

    // Sets hold sets; selecting the outer one selects what both hold; the file keeps them.
    c.mel("createNode transform -n a; createNode transform -n b; sets -n inner a; sets -n outer b inner;");
    assert_eq!(c.mel("sets -q outer;"), json!(["b", "inner"]));
    c.mel("select -r outer;");
    let mut sel: Vec<String> = c.mel("ls -sl;").as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
    sel.sort();
    assert_eq!(sel, vec!["a", "b"]);
    assert!(c.mel_checked("sets -add inner outer;").is_err(), "a set can't hold itself");

    // The future of a node through every connection.
    c.mel("createNode multiplyDivide -n md; connectAttr rig.tx md.input1X; connectAttr md.outputX a.tx;");
    let fut = c.mel("listHistory -ac -f rig;");
    assert!(fut.as_array().unwrap().iter().any(|v| v == "md") && fut.as_array().unwrap().iter().any(|v| v == "a"), "{fut}");

    // A pose of the objects named only, transforms among them.
    c.mel("setAttr ctl.tx 3; dagPose -save -selection -n ctlPose ctl;");
    c.mel("setAttr ctl.tx 0; dagPose -restore ctlPose;");
    assert_eq!(c.mel("getAttr ctl.tx;"), json!(3.0));

    // An orient constraint's axes taken apart and put back whole, as mGear's oriCns does.
    c.mel("createNode transform -n src; setAttr src.r 10 20 30; createNode transform -n dst; orientConstraint src dst;");
    c.mel("disconnectAttr dst_orientConstraint1.constraintRotateX dst.rotateX;");
    let ry = c.mel("listConnections -s 1 -d 0 -p 1 dst.rotateY;");
    assert_eq!(ry, json!(["dst_orientConstraint1.constraintRotateY"]));
    c.mel("connectAttr -f dst_orientConstraint1.constraintRotate dst.rotate;");
    assert!(nums(&c.mel("getAttr dst.r;")).iter().zip([10.0, 20.0, 30.0]).all(|(a, b)| (a - b).abs() < 1e-6));

    // A set on a channel a constraint drives is taken, and the constraint's value stays.
    c.mel("createNode transform -n held; scaleConstraint src held; setAttr held.sx 0.001;");
    assert!((c.mel("getAttr held.sx;").as_f64().unwrap() - 1.0).abs() < 1e-9);

    // Parenting keeps the world matrix: with a shear under a parent scaled unevenly, and around a driven channel.
    c.mel("createNode transform -n flat; setAttr flat.s 0.001 1 1; setAttr flat.r 10 20 30;");
    c.mel("createNode transform -n kid; setAttr kid.r 40 -25 70; setAttr kid.t 1 2 3;");
    let before = nums(&c.mel("xform -q -ws -m kid;"));
    c.mel("parent kid flat;");
    let after = nums(&c.mel("xform -q -ws -m kid;"));
    assert!(before.iter().zip(&after).all(|(a, b)| (a - b).abs() < 1e-6), "{before:?}\n{after:?}");
    c.mel("createNode transform -n driven; createNode transform -n drvP; setAttr drvP.tx 4; connectAttr md.output driven.translate;");
    assert!(c.mel_checked("parent driven drvP;").is_ok(), "a driven translate is left as it is");

    // A shape added under a second transform moves there; the first keeps a copy.
    c.mel("circle -n buf; createNode transform -n ctl2; parent -add -shape bufShape ctl2;");
    assert_eq!(c.mel("listRelatives -s ctl2;"), json!(["bufShape"]));
    assert_eq!(c.mel("listRelatives -s buf;"), json!(["bufShapeOrig"]));

    // Through a file: the sets, a constraint's rest in degrees, a joint with no inverseScale link.
    c.mel("createNode transform -n tgt2; createNode transform -n con; setAttr con.rx -15.9; parentConstraint -mo -st x -st y -st z tgt2 con; setAttr con_parentConstraint1.tgt2W0 0;");
    c.mel("joint -n j1; joint -n j2; disconnectAttr j1.scale j2.inverseScale;");
    round_trip(&mut c, "build");
    assert_eq!(c.mel("sets -q outer;"), json!(["b", "inner"]));
    assert!((c.mel("getAttr con.rx;").as_f64().unwrap() + 15.9).abs() < 1e-6, "{}", c.mel("getAttr con.rx;"));
    assert_eq!(c.mel("listConnections -s 1 -d 0 j2.inverseScale;"), json!([]));
}

/// MPlug's states: properties in API 2.0, calls in API 1.0, as the reference has them.
#[test]
fn plug_states_as_both_apis_have_them() {
    let mut c = Core::new();
    c.mel("createNode transform -n n;");
    let out = c.mel(
        "python(\"import maya.api.OpenMaya as om2, maya.OpenMaya as om1\\n\
         s = om2.MSelectionList(); s.add('n.tx'); p = s.getPlug(0)\\n\
         s1 = om1.MSelectionList(); s1.add('n.tx'); p1 = om1.MPlug(); s1.getPlug(0, p1)\\n\
         r = [bool(p.isChild), p.isChild == True, bool(p.isArray), p.isKeyable, p1.isChild(), p1.isArray(), repr(p.isChild)]\");\
         python(\"r\");",
    );
    // MEL takes a Python bool as 1 or 0.
    assert_eq!(out, json!(["1", "1", "0", "1", "1", "0", "True"]));
}
