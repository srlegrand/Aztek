//! Step 7: MEL's math and string procedures, alias, and the commands rig scripts call.

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
fn step7() {
    aztek_init();
    expect("import maya.cmds as cmds\nimport maya.mel as mel\ncmds.newScene()", &[]);
    expect(
        r###"
m = mel.eval
r = lambda x: [round(v, 4) + 0.0 for v in x] if isinstance(x, (list, tuple)) else round(x, 6)
print(m("constrainValue 3.0 9.0 1.5 8.0"), m("constrainValue 3.0 9.0 1.5 0.0"), m("constrainValue 3.0 9.0 1.5 12.0"))
print(m("float $a[] = {1,2,3}; float $b[] = {4,5,6}; crossProduct($a, $b, 0, 0)"), m("float $a[] = {1,2,3}; float $b[] = {4,5,6}; dotProduct($a, $b, 0)"))
print(m("equivalent(0.0, 0.0005)"), m("equivalent(0.0, 0.005)"), m("equivalentTol(0.0, 0.0005, 0.001)"), m("floatEq(5.0, 5.001)"), m("floatEq(5.0, 5.1)"))
print(r(m("erf 1.0")), r(m("gamma 2.5")), r(m("besselj1 5.5")))
print(r(m("hsv_to_rgb <<1,.5,.6>>")), r(m("rgb_to_hsv <<.6,.6,.6>>")), m("maxint"), m("minint"))
print(m("float $v[] = {3,0,4}; normalize($v); $v"), m("float $z[] = {0,0,0}; normalize($z)"))
print(m("float $p[] = {1,2,3}; float $mx[] = {1,0,0,0,0,2,0,0,0,0,3,0,0,0,0,0}; pointMatrixMult($p, $mx)"))
print(m('interToUI("graphEditor")'), m('interToUI("AndWoozles")'), m('nodeAttrJoin("a", "b")'), m('nodeAttrJoin("a", ".b")'))
print(m('sortCaseInsensitive({"bcd", "Def", "abC"})'), m('stringArrayDuplicates({"d", "b", "c", "c", "a", "a", "b"})'), m('stringArrayIntersection({"item2", "item1", "item3"}, {"item5", "item3", "item4"})'))
print(m('float $l[] = {1.0, 2.0, 3.0}; floatArrayRemoveAtIndex(1, $l); $l'), m('int $li[] = {1, 2}; intArrayRemoveAtIndex(5, $li)'))
print(m('string $o[] = {"a", "b"}; stringArrayAddPrefix($o, "***"); $o'), m('stringArrayRemovePrefix({"##x", "z##", "##"}, "##")'))
print(m('float $s[] = {1.0, 2.0}; float $d[] = {3.0, 4.0}; copyArray($s, $d, 1); $d'), m('containsMultibyte("Test String")'))
a = m('rand "s1" 1'); b = m('rand "s2" 1'); m('seed "s1" 7'); c = m('rand "s1" 1'); m('seed "s1" 7'); d = m('rand "s1" 1')
print(c == d, len(m('randstate "s1"')), m('delrandstr "s*"'), m('delrandstr "s*"'))
print(m('whatIs "alias"'), m('whatIs "crossProduct"'), m('whatIs "listTransforms"'))
"###,
        &[
            "7.5 3.0 9.0",
            "[-3.0, 6.0, -3.0] 32.0",
            "1 0 1 1 0",
            "0.842701 0.284683 -0.341438",
            "[0.6, 0.3, 0.3] [0.0, 0.0, 0.6] 2147483647 -2147483648",
            "[0.6, 0.0, 0.8] -1",
            "[1.0, 4.0, 9.0]",
            "Graph Editor And Woozles a.b a.b",
            "['abC', 'bcd', 'Def'] ['a', 'b', 'c'] ['item3']",
            "[1.0, 3.0] 0",
            "['***a', '***b'] ['x', 'z##', '']",
            "[1.0, 4.0] 0",
            "True 3 2 0",
            "Command Command Command",
        ],
    );
    // alias: words before the caller's, nested, and statements.
    expect(
        r#"
mel.eval("alias mkc polyCube -n;")
mel.eval("mkc aliasCube;")
mel.eval("alias mkc2 mkc aliasCube2;")
mel.eval("mkc2;")
mel.eval("alias clearAll { select -all; delete; };")
print(cmds.objExists("aliasCube"), cmds.objExists("aliasCube2"))
mel.eval("clearAll;")
print(cmds.ls(type="mesh"))
"#,
        &["True True", "[]"],
    );
    expect(
        r#"
cmds.file(new=True, force=True)
g = cmds.group(empty=True, n="grp")
c = cmds.polyCube(n="box")[0]
cmds.parent(c, g)
print(mel.eval('isParentOf("grp", "box")'), mel.eval('isParentOf("box", "box")'), mel.eval('isParentOf("", "box")'))
print(mel.eval('lsType("mesh")'), mel.eval('listTransforms("-type mesh")'))
print(hasattr(cmds, "isParentOf"), hasattr(cmds, "getChain"), hasattr(cmds, "affects"), hasattr(cmds, "fileInfo"))
cl = cmds.cluster(c)[0]
b = cmds.nonLinear(c, type="bend")[0]
print(mel.eval('findRelatedDeformer("box")') == [b, cl] or mel.eval('findRelatedDeformer("box")'), mel.eval('getChain("boxShape")') == mel.eval('findRelatedDeformer("box")'))
print(len(mel.eval("getAllChains")))
print(cmds.affects("translateX", "box"), "matrix" in cmds.affects("translateX", "box", by=True), cmds.affects("output", type="addDoubleLinear"))
"#,
        &[
            "1 0 0",
            "['boxShape', '<done>'] ['box']",
            "False False True True",
            "True True",
            "1",
            "[] True ['input1', 'input2']",
        ],
    );
    // Bind pose, constraint rest, IK to FK.
    expect(
        r#"
cmds.file(new=True, force=True)
cmds.select(clear=True)
j1 = cmds.joint(p=(0, 0, 0)); j2 = cmds.joint(p=(0, 2, 0)); j3 = cmds.joint(p=(0, 4, 0))
box = cmds.polyCube()[0]
cmds.skinCluster(j1, box)
cmds.setAttr(j1 + ".rz", 45)
cmds.select(j1)
mel.eval("gotoBindPose")
print(round(cmds.getAttr(j1 + ".rz"), 4))
loc = cmds.spaceLocator()[0]
tgt = cmds.spaceLocator()[0]
cmds.setAttr(tgt + ".t", 5, 0, 0)
pc = cmds.pointConstraint(tgt, loc)[0]
cmds.select(loc)
mel.eval("setConstraintRestPosition")
print([round(x, 3) for x in cmds.getAttr(pc + ".restTranslate")[0]])
h = cmds.ikHandle(sj=j1, ee=j3)[0]
cmds.setAttr(h + ".ikBlend", 0)
cmds.setAttr(j1 + ".rz", 90)
cmds.select(h)
mel.eval("moveIKtoFK")
print([round(x, 3) for x in cmds.xform(h, q=True, ws=True, t=True)], [round(x, 3) for x in cmds.xform(j3, q=True, ws=True, t=True)])
"#,
        &["0.0", "[5.0, 0.0, 0.0]", "[-4.0, 0.0, 0.0] [-4.0, 0.0, 0.0]"],
    );
    expect(
        r#"
print(cmds.sortStringArray(["file10", "file9", "file1"]), cmds.sortStringArray(["file10", "file9", "file1"], ia=True), cmds.sortStringArray(["file10", "file9", "file1"], ord="alphabetic"))
print(cmds.sortStringArray(["b", "A", "c"], ic=True, rev=True), cmds.sortCaseInsensitive(["bcd", "Def", "abC"]), mel.eval('sortStringArray {"x2", "x10"}'))
mx = cmds.matrixUtil(t=(1, 2, 3), r=(0, 0, 90), s=(2, 2, 2))
print([round(v, 3) + 0.0 for v in mx])
print([round(v, 3) + 0.0 for v in cmds.matrixUtil(mx, q=True, t=True)], [round(v, 3) + 0.0 for v in cmds.matrixUtil(mx, q=True, r=True)], [round(v, 3) + 0.0 for v in cmds.matrixUtil(mx, q=True, s=True)])
print([round(v, 3) + 0.0 for v in cmds.matrixUtil(mx, t=(1, 1, 1), rt=True)][12:15], [round(v, 3) + 0.0 for v in cmds.matrixUtil(cmds.matrixUtil(t=(4, 0, 0)), iv=True)][12:15])
"#,
        &[
            "['file1', 'file9', 'file10'] [2, 1, 0] ['file1', 'file10', 'file9']",
            "['c', 'b', 'A'] ['abC', 'bcd', 'Def'] ['x2', 'x10']",
            "[0.0, 2.0, 0.0, 0.0, -2.0, 0.0, 0.0, 0.0, 0.0, 0.0, 2.0, 0.0, 1.0, 2.0, 3.0, 1.0]",
            "[1.0, 2.0, 3.0] [0.0, 0.0, 90.0] [2.0, 2.0, 2.0]",
            "[2.0, 3.0, 4.0] [-4.0, 0.0, 0.0]",
        ],
    );
    let path = std::env::temp_dir().join(format!("aztek_step7_{}.ma", std::process::id()));
    let p = path.to_str().unwrap().replace('\\', "/");
    expect(
        &format!(
            r#"
cmds.file(new=True, force=True)
cmds.fileInfo("rig", "v3")
cmds.fileInfo("owner", "simon")
cmds.fileInfo("rig", "v4")
print(cmds.fileInfo(q=True), cmds.fileInfo("rig", q=True))
cmds.fileInfo(rm="owner")
cmds.requires("maya", "2027")
t = cmds.createNode("timeFunction")
cmds.setAttr(t + ".input", 2.5)
s = cmds.createNode("sampler")
cmds.connectAttr(t + ".output", s + ".function_Raw")
cmds.setAttr(s + ".minimum", 0); cmds.setAttr(s + ".maximum", 1); cmds.setAttr(s + ".step", 0.25)
print(cmds.getAttr(s + ".value", size=True) if False else [cmds.getAttr("%s.value[%d]" % (s, k)) for k in range(5)])
cmds.file(rename="{p}"); cmds.file(save=True, type="mayaAscii", force=True)
cmds.file(new=True, force=True)
print(cmds.fileInfo(q=True))
cmds.file("{p}", open=True, force=True)
print(cmds.fileInfo("rig", q=True), cmds.fileInfo("owner", q=True))
"#
        ),
        &["['rig', 'v4', 'owner', 'simon'] ['v4']", "[2.5, 2.5, 2.5, 2.5, 2.5]", "[]", "['v4'] []"],
    );
    let _ = std::fs::remove_file(path);
}
