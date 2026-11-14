//! The reference's newer rigging nodes (rig_nodes.rs): parentMatrix, uvPin, proximityPin, remapColor and remapHsv.

use std::ffi::{CStr, CString};

use aztek_host::*;
use serde_json::Value;

fn log() -> Vec<(i64, String)> {
    let p = aztek_take_log();
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    let v: Value = serde_json::from_str(&s).unwrap();
    v.as_array().unwrap().iter().map(|l| (l["kind"].as_i64().unwrap(), l["text"].as_str().unwrap().to_string())).collect()
}

fn expect(src: &str, want: &[&str]) {
    log();
    let c = CString::new(src).unwrap();
    aztek_python(c.as_ptr());
    let l = log();
    let errors: Vec<&String> = l.iter().filter(|(k, _)| *k == 2).map(|(_, t)| t).collect();
    assert!(errors.is_empty(), "Python failed:\n{}\nrunning:\n{src}", errors.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    let got: Vec<String> = l.into_iter().filter(|(k, _)| *k == 3).map(|(_, t)| t).collect();
    assert_eq!(got, want, "running:\n{src}");
}

#[test]
fn rig_nodes() {
    aztek_init();
    expect("import maya.cmds as cmds\ncmds.newScene()", &[]);
    expect(
        r#"
def mx(t):
    return [1,0,0,0, 0,1,0,0, 0,0,1,0, t[0],t[1],t[2],1]
r = lambda xs: [round(x, 3) + 0.0 for x in xs]
p = cmds.createNode("parentMatrix")
cmds.setAttr(p + ".target[0].targetMatrix", mx((10, 0, 0)), type="matrix")
print(r(cmds.getAttr(p + ".outputMatrix")[12:15]))
cmds.setAttr(p + ".target[1].targetMatrix", mx((0, 4, 0)), type="matrix")
print(r(cmds.getAttr(p + ".outputMatrix")[12:15]))
cmds.setAttr(p + ".target[1].weight", 0)
cmds.setAttr(p + ".envelope", 0.5)
print(r(cmds.getAttr(p + ".outputMatrix")[12:15]))
cmds.setAttr(p + ".inputMatrix", mx((0, 0, 2)), type="matrix")
cmds.setAttr(p + ".enable", 0)
print(r(cmds.getAttr(p + ".outputMatrix")[12:15]))
print(cmds.getAttr(p + ".tgt[0].wgt"), cmds.getClassification("parentMatrix"))
"#,
        &["[10.0, 0.0, 0.0]", "[5.0, 2.0, 0.0]", "[5.0, 0.0, 0.0]", "[0.0, 0.0, 2.0]", "1.0 ['animation']"],
    );
    expect(
        r#"
c = cmds.createNode("remapColor")
print(r(cmds.getAttr(c + ".outColor")[0]), cmds.getAttr(c + ".red", size=True), cmds.getAttr(c + ".red[1].red_FloatValue"))
print(cmds.getAttr(c + ".r", multiIndices=True))
cmds.setAttr(c + ".color", 0.25, 0.5, 1.0)
cmds.setAttr(c + ".red[1].red_FloatValue", 0.5)
cmds.setAttr(c + ".outputMax", 2)
print(r(cmds.getAttr(c + ".outColor")[0]))
h = cmds.createNode("remapHsv")
cmds.setAttr(h + ".color", 1, 0.5, 0)
print(r(cmds.getAttr(h + ".outColor")[0]))
cmds.setAttr(h + ".saturation[1].saturation_FloatValue", 0)
print(r(cmds.getAttr(h + ".outColor")[0]), cmds.getClassification("remapHsv"))
"#,
        &["[0.5, 0.5, 0.5] 2 1.0", "[0, 1]", "[0.25, 1.0, 2.0]", "[1.0, 0.5, 0.0]", "[1.0, 1.0, 1.0] ['utility/color:drawdb/shader/operation/remapHsv']"],
    );
    expect(
        r#"
t, _ = cmds.polyPlane(w=2, h=2, sx=1, sy=1)
shape = cmds.listRelatives(t, shapes=True)[0]
u = cmds.createNode("uvPin")
cmds.connectAttr(shape + ".worldMesh[0]", u + ".deformedGeometry")
cmds.setAttr(u + ".coordinate[0]", 0.5, 0.5)
cmds.setAttr(u + ".coordinate[1]", 1, 1)
m = cmds.getAttr(u + ".outputMatrix[0]")
print(r(m[0:3]), r(m[4:7]), r(m[8:11]), r(m[12:15]))
print(r(cmds.getAttr(u + ".outputTranslate[1]")[0]))
cmds.setAttr(t + ".ty", 3)
print(r(cmds.getAttr(u + ".outputTranslate[0]")[0]))
cmds.setAttr(u + ".normalAxis", 1)
cmds.setAttr(u + ".tangentAxis", 0)
m = cmds.getAttr(u + ".outputMatrix[0]")
print(r(m[0:3]), r(m[4:7]), r(m[8:11]))
"#,
        &["[0.0, 1.0, 0.0] [0.0, 0.0, 1.0] [1.0, 0.0, 0.0] [0.0, 0.0, 0.0]", "[1.0, 0.0, -1.0]", "[0.0, 3.0, 0.0]", "[1.0, 0.0, 0.0] [0.0, 1.0, 0.0] [0.0, 0.0, 1.0]"],
    );
    expect(
        r#"
a, _ = cmds.polyPlane(w=2, h=2, sx=1, sy=1)
b, _ = cmds.polyPlane(w=2, h=2, sx=1, sy=1)
cmds.setAttr(b + ".ty", 3)
cmds.setAttr(b + ".sx", 2)
x = cmds.createNode("proximityPin")
cmds.connectAttr(cmds.listRelatives(a, shapes=True)[0] + ".worldMesh[0]", x + ".originalGeometry")
cmds.connectAttr(cmds.listRelatives(b, shapes=True)[0] + ".worldMesh[0]", x + ".deformedGeometry")
cmds.setAttr(x + ".inputMatrix[0]", mx((0.5, 1, 0.25)), type="matrix")
print(r(cmds.getAttr(x + ".outputMatrix[0]")[12:15]))
cmds.setAttr(x + ".offsetTranslation", 0)
print(r(cmds.getAttr(x + ".outputMatrix[0]")[12:15]))
cmds.setAttr(x + ".envelope", 0)
print(r(cmds.getAttr(x + ".outputMatrix[0]")[12:15]))
"#,
        &["[1.0, 4.0, 0.25]", "[1.0, 3.0, 0.25]", "[0.5, 1.0, 0.25]"],
    );
    let path = std::env::temp_dir().join(format!("aztek_rig_{}.ma", std::process::id()));
    let p = path.to_str().unwrap().replace('\\', "/");
    expect(
        &format!(
            r#"
cmds.file(new=True, force=True)
cmds.createNode("parentMatrix", n="pm")
cmds.setAttr("pm.target[0].targetMatrix", mx((1, 2, 3)), type="matrix")
cmds.setAttr("pm.target[0].weight", 0.5)
cmds.createNode("remapColor", n="rc")
cmds.setAttr("rc.green[1].green_FloatValue", 0.25)
cmds.file(rename="{p}"); cmds.file(save=True, type="mayaAscii", force=True)
lines = [l.strip() for l in open("{p}")]
print([l for l in lines if l.startswith('setAttr ".tgt') or l.startswith('setAttr ".g[')])
cmds.file("{p}", open=True, force=True)
print(r(cmds.getAttr("pm.outputMatrix")[12:15]), cmds.getAttr("pm.tgt[0].wgt"), cmds.getAttr("rc.g", size=True), cmds.getAttr("rc.g[1].gfv"))
"#
        ),
        &[
            r#"['setAttr ".tgt[0].tmat" -type "matrix" 1 0 0 0 0 1 0 0 0 0 1 0 1 2 3 1;', 'setAttr ".tgt[0].wgt" 0.5;', 'setAttr ".g[0].gp" 0;', 'setAttr ".g[1].gp" 1;', 'setAttr ".g[0].gfv" 0;', 'setAttr ".g[1].gfv" 0.25;', 'setAttr ".g[0].gi" 1;', 'setAttr ".g[1].gi" 1;']"#,
            "[1.0, 2.0, 3.0] 0.5 2 0.25",
        ],
    );
    let _ = std::fs::remove_file(path);
    joint_symmetry();
}

/// The joint tool's symmetry: `joint -sym` and the symmetryConstraint. Called from rig_nodes: the host is one per process.
fn joint_symmetry() {
    expect("import maya.cmds as cmds\ncmds.newScene()", &[]);
    expect(
        r#"
r = lambda xs: [round(x, 3) + 0.0 for x in xs]
cmds.select(clear=True)
a = cmds.joint(p=(0, 10, 0))
b = cmds.joint(p=(2, 10, 0), sym=True, sa="x")
c = cmds.joint(p=(4, 9, 1), sym=True, sa="x")
print(a, b, c, sorted(cmds.ls(type="joint")), cmds.ls(type="symmetryConstraint"))
twin_b = cmds.listConnections(cmds.ls(type="symmetryConstraint")[0] + ".constraintTranslate", d=True, s=False)
print(twin_b, cmds.listRelatives(twin_b[0], parent=True))
twin_c = cmds.listConnections(cmds.ls(type="symmetryConstraint")[1] + ".ct", d=True, s=False)[0]
print(cmds.listRelatives(twin_c, parent=True) == twin_b, r(cmds.xform(twin_c, q=True, ws=True, t=True)))
cmds.setAttr(b + ".rz", 30)
print(r(cmds.xform(c, q=True, ws=True, t=True)), r(cmds.xform(twin_c, q=True, ws=True, t=True)))
print(r(cmds.getAttr(twin_b[0] + ".r")[0]), r(cmds.getAttr(twin_b[0] + ".jo")[0]))
cmds.setAttr(b + ".ty", 1)
print(r(cmds.xform(twin_b[0], q=True, ws=True, t=True)), r(cmds.getAttr(cmds.ls(type="symmetryConstraint")[0] + ".cmwp")[0]))
cmds.delete(b, constraints=True)
print(cmds.ls(type="symmetryConstraint"), r(cmds.xform(twin_b[0], q=True, ws=True, t=True)))
cmds.select(clear=True)
m = cmds.joint(p=(0, 5, 0), sym=True)
print(cmds.ls(type="symmetryConstraint"))
"#,
        &[
            "joint1 joint2 joint4 ['joint1', 'joint2', 'joint3', 'joint4', 'joint5'] ['joint3_symmetryConstraint1', 'joint5_symmetryConstraint1']",
            "['joint3'] ['joint1']",
            "True [-4.0, 9.0, 1.0]",
            "[4.232, 10.134, 1.0] [-4.232, 10.134, 1.0]",
            "[0.0, 0.0, 30.0] [180.0, 0.0, 0.0]",
            "[-2.0, 11.0, 0.0] [0.0, 11.0, 0.0]",
            "['joint5_symmetryConstraint1'] [-2.0, 11.0, 0.0]",
            "['joint5_symmetryConstraint1']",
        ],
    );
    let path = std::env::temp_dir().join(format!("aztek_sym_{}.ma", std::process::id()));
    let p = path.to_str().unwrap().replace('\\', "/");
    expect(
        &format!(
            r#"
cmds.file(new=True, force=True)
cmds.select(clear=True)
a = cmds.joint(p=(1, 0, 0), sym=True, sa="x")
b = cmds.joint(p=(3, 0, 0), sym=True, sa="x")
cmds.file(rename="{p}"); cmds.file(save=True, type="mayaAscii", force=True)
text = open("{p}").read()
print(text.count("createNode symmetryConstraint"), 'connectAttr "joint3.wm" ' in text or 'connectAttr "joint3.worldMatrix' in text)
cmds.file("{p}", open=True, force=True)
cmds.setAttr("joint1.ry", 90)
print(r(cmds.xform("joint4", q=True, ws=True, t=True)), cmds.ls(type="symmetryConstraint"))
# As the reference writes one: under the driven joint.
src = open("{p}").read().replace('createNode symmetryConstraint -n "joint4_symmetryConstraint1";', 'createNode symmetryConstraint -n "joint4_symmetryConstraint1" -p "joint4";')
open("{p}", "w").write(src)
cmds.file("{p}", open=True, force=True)
cmds.setAttr("joint1.ry", 90)
print(r(cmds.xform("joint4", q=True, ws=True, t=True)))
"#
        ),
        &["2 True", "[-1.0, 0.0, -2.0] ['joint2_symmetryConstraint1', 'joint4_symmetryConstraint1']", "[-1.0, 0.0, -2.0]"],
    );
    let _ = std::fs::remove_file(path);
}
