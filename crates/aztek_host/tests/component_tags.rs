//! Component tags, tag expressions on deformers, and the falloff nodes.

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
fn component_tags() {
    aztek_init();
    expect("import maya.cmds as cmds\ncmds.newScene()", &[]);
    expect(
        r#"
r = lambda xs: [round(x, 3) + 0.0 for x in xs]
cube = cmds.polyCube(n="c")[0]
print(cmds.componentTag(cube + ".f[0:1]", cr=True, ntn="front"), cmds.componentTag(cube + ".vtx[0:1]", cr=True, ntn="bottomEdge"))
print(cmds.geometryAttrInfo(cube + ".outMesh", cnm=True))
print(cmds.geometryAttrInfo(cube + ".outMesh", cex="front", cmp=True), cmds.geometryAttrInfo(cube + ".outMesh", cex="front", ccy=True))
print(cmds.geometryAttrInfo(cube + ".outMesh", cex="front", ctv=True, cmp=True))
print(cmds.geometryAttrInfo(cube + ".outMesh", cex="!bottomEdge", pi=True), cmds.geometryAttrInfo(cube + ".outMesh", cex="front ^ bottomEdge", pi=True))
print(cmds.geometryAttrInfo(cube + ".outMesh", cex="*-(front+bottomEdge)", pc=True), cmds.geometryAttrInfo(cube + ".outMesh", cex="f*", pc=True), cmds.geometryAttrInfo(cube + ".outMesh", cex="v(front)", ccy=True))
cmds.componentTag(cube + ".vtx[7]", tn="bottomEdge", m="add")
print(cmds.geometryAttrInfo(cube + ".outMesh", cex="bottomEdge", pi=True))
cmds.componentTag(cube, rn=True, tn="bottomEdge", ntn="low")
cmds.componentTag(cube, d=True, tn="fr*")
print(cmds.geometryAttrInfo(cube + ".outMesh", cnm=True))
"#,
        &[
            "front bottomEdge",
            "['bottomEdge', 'front']",
            "['f[0:1]'] f",
            "['vtx[0:5]']",
            "[2, 3, 4, 5, 6, 7] [0, 1]",
            "2 6 v",
            "[0, 1, 7]",
            "['low']",
        ],
    );
    expect(
        r#"
cmds.file(new=True, force=True)
cube = cmds.polyCube(n="c")[0]
cmds.componentTag(cube + ".vtx[0:3]", cr=True, ntn="lower")
cl, h = cmds.cluster(cube)
cmds.setAttr(cl + ".componentTagExpression", "lower", type="string")
cmds.setAttr(h + ".ty", 1)
ys = [round(cmds.pointPosition(cube + ".vtx[%d]" % i, w=True)[1], 3) for i in range(8)]
print(ys)
u = cmds.createNode("uniformFalloff")
cmds.setAttr(u + ".uniformWeight", 0.5)
cmds.connectAttr(u + ".outputWeightFunction", cl + ".weightFunction")
print([round(cmds.pointPosition(cube + ".vtx[%d]" % i, w=True)[1], 3) for i in range(8)])
cmds.setAttr(cl + ".componentTagExpression", "*", type="string")
p = cmds.createNode("primitiveFalloff")
cmds.setAttr(p + ".t", 0.5, 0.5, 0.5)
cmds.setAttr(p + ".end", 0.5)
cmds.connectAttr(p + ".outputWeightFunction", cl + ".weightFunction", force=True)
print([round(cmds.pointPosition(cube + ".vtx[%d]" % i, w=True)[1], 3) for i in range(8)])
cmds.setAttr(p + ".end", 2)
print([round(cmds.pointPosition(cube + ".vtx[%d]" % i, w=True)[1], 3) for i in range(8)])
"#,
        &[
            "[0.5, 0.5, 1.5, 1.5, 0.5, 0.5, -0.5, -0.5]",
            "[0.0, 0.0, 1.0, 1.0, 0.5, 0.5, -0.5, -0.5]",
            "[-0.5, -0.5, 1.5, 0.5, 0.5, 0.5, -0.5, -0.5]",
            "[-0.207, 0.0, 1.5, 1.0, 1.0, 0.793, -0.207, -0.366]",
        ],
    );
    expect(
        r#"
cmds.file(new=True, force=True)
cube = cmds.polyCube(n="c")[0]
cmds.componentTag(cube + ".vtx[0:3]", cr=True, ntn="lower")
cl, h = cmds.cluster(cube)
cmds.setAttr(h + ".ty", 1)
ys = lambda: [round(cmds.pointPosition(cube + ".vtx[%d]" % i, w=True)[1] - y0[i], 3) for i in range(8)]
y0 = [0.5 * (1 if i in (2, 3, 4, 5) else -1) for i in range(8)]
b = cmds.createNode("blendFalloff")
for k, (w, mode) in enumerate([(0.5, 1), (0.2, 3)]):
    u = cmds.createNode("uniformFalloff")
    cmds.setAttr(u + ".uniformWeight", w)
    cmds.connectAttr(u + ".outputWeightFunction", "%s.target[%d].weightFunction" % (b, k))
    cmds.setAttr("%s.target[%d].mode" % (b, k), mode)
cmds.connectAttr(b + ".outputWeightFunction", cl + ".weightFunction")
print(ys())
sf = cmds.createNode("subsetFalloff")
cmds.setAttr(sf + ".useFalloffTags", 1)
cmds.setAttr(sf + ".falloffTags", "lower", type="string")
cmds.setAttr(sf + ".withinBoundary", 0)
cmds.setAttr(sf + ".end", 2)
cmds.connectAttr(sf + ".outputWeightFunction", cl + ".weightFunction", force=True)
print(ys())
t = cmds.createNode("componentTagBase")
cmds.connectAttr(cmds.listRelatives(cube, s=True)[0] + ".outMesh", t + ".inputGeometry")
cmds.setAttr(t + ".componentTags[0].componentTagName", "one", type="string")
cmds.setAttr(t + ".componentTags[0].componentTagContents", "vtx[5]", type="string")
print(cmds.geometryAttrInfo(t + ".outputGeometry", cnm=True), cmds.geometryAttrInfo(t + ".outputGeometry", cex="one", pi=True))
m = cmds.createNode("componentMatch")
other = cmds.polyCube(n="d")[0]
e = cmds.polyCube(n="e")[0]
cmds.componentTag(e + ".vtx[0:3]", cr=True, ntn="lower")
cmds.connectAttr(cmds.listRelatives(e, s=True)[0] + ".outMesh", m + ".inputGeometry")
cmds.connectAttr(cmds.listRelatives(other, s=True)[0] + ".outMesh", m + ".targetGeometry")
cmds.setAttr(m + ".componentTagExpression", "lower", type="string")
print([cmds.getAttr("%s.componentLookup[%d]" % (m, k)) for k in range(4)])
"#,
        &["[0.7, 0.7, 0.7, 0.7, 0.7, 0.7, 0.7, 0.7]", "[1.0, 1.0, 1.0, 1.0, 0.5, 0.5, 0.5, 0.5]", "['lower', 'one'] [5]", "[0, 1, 2, 3]"],
    );
    let path = std::env::temp_dir().join(format!("aztek_tags_{}.ma", std::process::id()));
    let p = path.to_str().unwrap().replace('\\', "/");
    expect(
        &format!(
            r#"
cmds.file(new=True, force=True)
cube = cmds.polyCube(n="c")[0]
cmds.componentTag(cube + ".vtx[0:3]", cr=True, ntn="lower")
cmds.componentTag(cube + ".f[1]", cr=True, ntn="top")
cl, h = cmds.cluster(cube)
cmds.setAttr(cl + ".componentTagExpression", "lower", type="string")
u = cmds.createNode("uniformFalloff", n="uf")
cmds.setAttr(u + ".uniformWeight", 0.5)
cmds.connectAttr(u + ".outputWeightFunction", cl + ".weightFunction")
cmds.setAttr(h + ".ty", 2)
before = [round(cmds.pointPosition(cube + ".vtx[%d]" % i, w=True)[1], 3) for i in range(8)]
cmds.file(rename="{p}"); cmds.file(save=True, type="mayaAscii", force=True)
text = open("{p}").read()
print('setAttr ".gtag[0].gtagnm" -type "string" "lower";' in text, 'setAttr ".gtag[0].gtagcmp" -type "componentList" 1 "vtx[0:3]";' in text, 'setAttr ".gtag[1].gtagcmp" -type "componentList" 1 "f[1]";' in text, 'setAttr ".ip[0].gtg" -type "string" "lower";' in text, '.wfl[0]"' in text)
cmds.file("{p}", open=True, force=True)
after = [round(cmds.pointPosition("c.vtx[%d]" % i, w=True)[1], 3) for i in range(8)]
print(before == after, after, cmds.geometryAttrInfo("cShape.outMesh", cnm=True))
"#
        ),
        &["True True True True True", "True [0.5, 0.5, 1.5, 1.5, 0.5, 0.5, -0.5, -0.5] ['lower', 'top']"],
    );
    let _ = std::fs::remove_file(path);
}
