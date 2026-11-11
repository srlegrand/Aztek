//! The Python API 1.0: maya.OpenMaya, maya.OpenMayaAnim and maya.OpenMayaMPx, with results handed back through the
//! arguments (MScriptUtil pointers, arrays and paths to fill). Python runs once per process, so one test walks through.

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

fn fresh() {
    expect("import maya.cmds as cmds\nimport maya.OpenMaya as om\nimport maya.OpenMayaAnim as oma\ncmds.newScene()", &[]);
}

fn paths_and_selection() {
    fresh();
    expect(
        r#"
cmds.polySphere(n="ball")
cmds.move(1, 2, 3, "ball")
sel = om.MSelectionList()
om.MGlobal.getSelectionListByName("ball", sel)
dp = om.MDagPath()
sel.getDagPath(0, dp)
print(sel.length(), dp.fullPathName(), type(dp).__module__)
obj = om.MObject()
sel.getDependNode(0, obj)
print(obj.apiType() == om.MFn.kTransform, om.MFnDependencyNode(obj).name(), om.MFnDependencyNode(obj).typeName())
shape = om.MDagPath(dp)
shape.extendToShape()
print(shape.fullPathName(), shape.apiType() == om.MFn.kMesh, dp.fullPathName())
u = om.MScriptUtil(); u.createFromInt(0); n = u.asUintPtr()
dp.numberOfShapesDirectlyBelow(n)
print(om.MScriptUtil.getUint(n))
cmds.group("ball", n="grp")
cmds.rename("ball", "orb")
print(dp.fullPathName(), dp.partialPathName(), om.MObjectHandle(dp.node()).isAlive())
p2 = om.MDagPath()
om.MDagPath.getAPathTo(obj, p2)
print(p2.fullPathName(), p2 == dp)
cmds.select("orb", "grp")
act = om.MSelectionList()
om.MGlobal.getActiveSelectionList(act)
names = []
act.getSelectionStrings(names)
print(act.length(), names)
it = om.MItSelectionList(act)
while not it.isDone():
    q = om.MDagPath(); it.getDagPath(q); print(q.partialPathName()); it.next()
"#,
        &[
            "1 |ball maya.OpenMaya",
            "True ball transform",
            "|ball|ballShape True |ball",
            "1",
            "|grp|orb orb True",
            "|grp|orb True",
            "2 ['orb', 'grp']",
            "orb",
            "grp",
        ],
    );
}

fn transforms_and_values() {
    fresh();
    expect(
        r#"
cmds.polyCube(n="box")
cmds.setAttr("box.t", 1, 2, 3)
cmds.setAttr("box.rx", 90)
cmds.setAttr("box.s", 2, 2, 2)
sel = om.MSelectionList(); sel.add("box"); dp = om.MDagPath(); sel.getDagPath(0, dp)
fn = om.MFnTransform(dp)
print(fn.getTranslation(om.MSpace.kTransform).z, round(fn.rotation().x, 4))
s = om.MScriptUtil(); s.createFromList([0.0, 0.0, 0.0], 3); sp = s.asDoublePtr()
fn.getScale(sp)
print(om.MScriptUtil.getDoubleArrayItem(sp, 0), om.MScriptUtil.getDoubleArrayItem(sp, 2))
om.MScriptUtil.setDoubleArray(sp, 1, 5.0)
fn.setScale(sp)
print(cmds.getAttr("box.sy"))
r = om.MEulerRotation(); fn.getRotation(r); print(round(r.x, 4))
q = om.MQuaternion(); fn.getRotation(q); print(round(q.w, 4))
m = dp.inclusiveMatrix()
print(type(m).__name__, m(3, 0), m(3, 1), om.MScriptUtil.getDoubleArrayItem(m[3], 2))
tm = om.MTransformationMatrix(m)
print(tm.getTranslation(om.MSpace.kWorld).y, round(tm.eulerRotation().x, 4))
m2 = om.MMatrix()
om.MScriptUtil.createMatrixFromList([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 7, 8, 9, 1], m2)
print(m2(3, 2), (om.MPoint(1, 1, 1) * m2).x)
a = om.MIntArray()
om.MScriptUtil.createIntArrayFromList([4, 5, 6], a)
a.append(7)
print(a.length(), a[3], type(a).__name__)
v = om.MVector(1, 0, 0) ^ om.MVector(0, 1, 0)
print(v.z, type(v).__name__, isinstance(v, om.MVector))
st = om.MStatus()
print(bool(st), st.error())
om.MGlobal.setZAxisUp(True)
print(om.MGlobal.isZAxisUp(), om.MGlobal.upAxis(), cmds.upAxis(q=True, ax=True))
om.MGlobal.setYAxisUp(True)
print(om.MGlobal.isYAxisUp())
"#,
        &[
            "3.0 1.5708",
            "2.0 2.0",
            "5.0",
            "1.5708",
            "0.7071",
            "MMatrix 1.0 2.0 3.0",
            "2.0 1.5708",
            "9.0 8.0",
            "4 7 MIntArray",
            "1.0 MVector True",
            "True False",
            "True (0, 0, 1) z",
            "True",
        ],
    );
}

fn meshes() {
    fresh();
    expect(
        r#"
cmds.polyCube(n="box", sx=2, sy=2, sz=2)
cmds.move(0, 5, 0, "box")
sel = om.MSelectionList(); sel.add("box"); dp = om.MDagPath(); sel.getDagPath(0, dp)
mesh = om.MFnMesh(dp)
print(mesh.numVertices(), mesh.numPolygons(), mesh.numEdges())
pts = om.MPointArray()
mesh.getPoints(pts, om.MSpace.kWorld)
print(pts.length(), type(pts[0]).__name__, pts[0].y)
counts = om.MIntArray(); ids = om.MIntArray()
mesh.getVertices(counts, ids)
print(counts.length(), ids.length())
fv = om.MIntArray(); mesh.getPolygonVertices(0, fv); print(fv.length())
e = om.MScriptUtil(); e.createFromList([0, 0], 2); ep = e.asInt2Ptr()
mesh.getEdgeVertices(0, ep)
print(om.MScriptUtil.getInt2ArrayItem(ep, 0, 0) != om.MScriptUtil.getInt2ArrayItem(ep, 0, 1))
n = om.MVector(); mesh.getPolygonNormal(0, n, om.MSpace.kWorld); print(round(n.length(), 6))
cp = om.MPoint()
fu = om.MScriptUtil(); fu.createFromInt(-1); fp = fu.asIntPtr()
mesh.getClosestPoint(om.MPoint(0, 20, 0), cp, om.MSpace.kWorld, fp)
print(cp.y, om.MScriptUtil.getInt(fp) >= 0)
hit = om.MFloatPoint()
pu = om.MScriptUtil(); pu.createFromDouble(0.0); prm = pu.asFloatPtr()
hu = om.MScriptUtil(); hu.createFromInt(-1); hf = hu.asIntPtr()
ok = mesh.closestIntersection(om.MFloatPoint(0.1, 20, 0.1), om.MFloatVector(0, -1, 0), None, None, False, om.MSpace.kWorld, 100, False, None, hit, prm, hf, None, None, None)
print(ok, hit.y, om.MScriptUtil.getFloat(prm))
miss = mesh.closestIntersection(om.MFloatPoint(9, 20, 9), om.MFloatVector(0, -1, 0), None, None, False, om.MSpace.kWorld, 100, False, None, hit, prm, hf, None, None, None)
print(miss)
us = om.MFloatArray(); vs = om.MFloatArray(); mesh.getUVs(us, vs); print(us.length() == vs.length() and us.length() > 0)
sets = []; mesh.getUVSetNames(sets); print(sets)
for i in range(pts.length()):
    pts.set(om.MPoint(pts[i].x, pts[i].y + 1, pts[i].z), i)
mesh.setPoints(pts, om.MSpace.kWorld)
print(cmds.pointPosition("box.vtx[0]", w=True)[1])
"#,
        &[
            "26 24 48",
            "26 MPoint 4.5",
            "24 96",
            "4",
            "True",
            "1.0",
            "5.5 True",
            "True 5.5 14.5",
            "False",
            "True",
            "['map1']",
            "5.5",
        ],
    );
}

fn iterators() {
    fresh();
    expect(
        r#"
cmds.polyPlane(n="pl", sx=2, sy=2)
sel = om.MSelectionList(); sel.add("pl"); dp = om.MDagPath(); sel.getDagPath(0, dp)
it = om.MItMeshVertex(dp)
total = 0
while not it.isDone():
    conn = om.MIntArray()
    it.getConnectedVertices(conn)
    total += conn.length()
    it.next()
print(it.count(), total)
pu = om.MScriptUtil(); pu.createFromInt(0); prev = pu.asIntPtr()
it.reset(); it.setIndex(4, prev)
c = om.MScriptUtil(); c.createFromInt(0); cp = c.asIntPtr()
it.numConnectedEdges(cp)
print(om.MScriptUtil.getInt(prev), it.index(), om.MScriptUtil.getInt(cp), it.position().x)
pit = om.MItMeshPolygon(dp)
a = om.MScriptUtil(); a.createFromDouble(0.0); ap = a.asDoublePtr()
area = 0.0
while not pit.isDone():
    pit.getArea(ap); area += om.MScriptUtil.getDouble(ap)
    vs = om.MIntArray(); pit.getVertices(vs)
    pit.next()
print(round(area, 6), vs.length())
eit = om.MItMeshEdge(dp)
l = om.MScriptUtil(); l.createFromDouble(0.0); lp = l.asDoublePtr()
eit.getLength(lp)
print(eit.index(), eit.index(0), eit.index(1), om.MScriptUtil.getDouble(lp))
dit = om.MItDag(om.MItDag.kDepthFirst, om.MFn.kMesh)
found = []
while not dit.isDone():
    p = om.MDagPath(); dit.getPath(p); found.append(p.partialPathName()); dit.next()
print(found)
"#,
        &["9 24", "0 4 4 0.0", "1.0 4", "0 0 1 0.5", "['plShape']"],
    );
}

fn plugs_and_commands() {
    fresh();
    expect(
        r#"
cmds.polyCube(n="box")
loc = cmds.spaceLocator(n="drv")[0]
sel = om.MSelectionList(); sel.add("box"); o = om.MObject(); sel.getDependNode(0, o)
fn = om.MFnDependencyNode(o)
plug = fn.findPlug("translateX")
arr = om.MPlugArray()
print(plug.connectedTo(arr, True, False), arr.length())
cmds.connectAttr("drv.tx", "box.tx")
print(plug.connectedTo(arr, True, False), arr.length(), arr[0].name())
conns = om.MPlugArray(); fn.getConnections(conns); print(conns.length() >= 1)
cmds.setAttr("drv.tx", 3)
print(plug.asDouble())
attr = om.MFnAttribute(plug.attribute())
print(attr.name(), attr.isKeyable(), attr.isStorable())
r = om.MCommandResult()
om.MGlobal.executeCommand("ls -type transform box drv", r)
sa = om.MStringArray(); r.getResult(sa)
print(sa.length(), sorted(sa), r.resultType() == om.MCommandResult.kStringArray)
"#,
        &[
            "False 0",
            "True 1 drv.translateX",
            "True",
            "3.0",
            "translateX True True",
            "2 ['box', 'drv'] True",
        ],
    );
}

fn curves_and_skins() {
    fresh();
    expect(
        r#"
c = cmds.curve(n="crv", d=1, p=[(0, 0, 0), (1, 0, 0), (2, 0, 0)])
sel = om.MSelectionList(); sel.add(c); cp = om.MDagPath(); sel.getDagPath(0, cp)
fc = om.MFnNurbsCurve(cp)
cvs = om.MPointArray(); fc.getCVs(cvs, om.MSpace.kWorld)
pt = om.MPoint(); fc.getPointAtParam(1.5, pt, om.MSpace.kWorld)
pu = om.MScriptUtil(); pu.createFromDouble(0.0); pp = pu.asDoublePtr()
q = fc.closestPoint(om.MPoint(1.25, 1, 0), pp, 0.001, om.MSpace.kWorld)
print(cvs.length(), pt.x, round(q.x, 4), round(om.MScriptUtil.getDouble(pp), 4), fc.numCVs(), fc.degree())
cmds.polyCube(n="box", sy=2)
j1 = cmds.joint(p=(0, -1, 0), n="j1")
j2 = cmds.joint(p=(0, 1, 0), n="j2")
sk = cmds.skinCluster("j1", "box")[0]
sel = om.MSelectionList(); sel.add(sk); sel.add("box")
so = om.MObject(); sel.getDependNode(0, so)
bp = om.MDagPath(); sel.getDagPath(1, bp)
skin = oma.MFnSkinCluster(so)
infs = om.MDagPathArray()
print(skin.influenceObjects(infs), [infs[i].partialPathName() for i in range(infs.length())])
w = om.MDoubleArray()
u = om.MScriptUtil(); u.createFromInt(0); np = u.asUintPtr()
skin.getWeights(bp, om.MObject(), w, np)
print(w.length(), om.MScriptUtil.getUint(np), round(sum(w), 6))
w0 = om.MDoubleArray(); skin.getWeights(bp, om.MObject(), 0, w0)
print(w0.length())
"#,
        &["3 1.5 1.25 1.25 3 1", "2 ['j1', 'j2']", "24 2 12.0", "12"],
    );
}

fn plugins() {
    let dir = std::env::temp_dir().join(format!("aztek_om1_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("helloV1.py"),
        r#"
import maya.OpenMaya as OpenMaya
import maya.OpenMayaMPx as OpenMayaMPx

kId = OpenMaya.MTypeId(0x8700A)


class HelloCmd(OpenMayaMPx.MPxCommand):
    def __init__(self):
        OpenMayaMPx.MPxCommand.__init__(self)

    def doIt(self, args):
        n = args.asInt(0) if args.length() else 1
        self.setResult("hello x%d" % n)


def cmdCreator():
    return OpenMayaMPx.asMPxPtr(HelloCmd())


class AddOne(OpenMayaMPx.MPxNode):
    aIn = OpenMaya.MObject()
    aOut = OpenMaya.MObject()

    def __init__(self):
        OpenMayaMPx.MPxNode.__init__(self)

    def compute(self, plug, data):
        if plug == AddOne.aOut:
            v = data.inputValue(AddOne.aIn).asFloat()
            data.outputValue(AddOne.aOut).setFloat(v + 1.0)
            data.setClean(plug)
        else:
            return OpenMaya.kUnknownParameter


def nodeCreator():
    return OpenMayaMPx.asMPxPtr(AddOne())


def nodeInit():
    n = OpenMaya.MFnNumericAttribute()
    AddOne.aIn = n.create("input", "in", OpenMaya.MFnNumericData.kFloat, 0.0)
    n.setKeyable(True)
    AddOne.aOut = n.create("output", "out", OpenMaya.MFnNumericData.kFloat, 0.0)
    n.setWritable(False)
    n.setStorable(False)
    AddOne.addAttribute(AddOne.aIn)
    AddOne.addAttribute(AddOne.aOut)
    AddOne.attributeAffects(AddOne.aIn, AddOne.aOut)


def initializePlugin(mobject):
    p = OpenMayaMPx.MFnPlugin(mobject, "Tester", "1.0", "Any")
    p.registerCommand("helloV1", cmdCreator)
    p.registerNode("addOneV1", kId, nodeCreator, nodeInit)


def uninitializePlugin(mobject):
    p = OpenMayaMPx.MFnPlugin(mobject)
    p.deregisterCommand("helloV1")
    p.deregisterNode(kId)
"#,
    )
    .unwrap();
    let dirs = dir.to_str().unwrap().replace('\\', "/");
    fresh();
    expect(
        &format!(
            r#"
import os
os.environ["AZTEK_PLUG_IN_PATH"] = "{dirs}"
print(cmds.loadPlugin("helloV1"), cmds.pluginInfo("helloV1", q=True, vendor=True))
print(cmds.helloV1(3), cmds.helloV1())
n = cmds.createNode("addOneV1")
cmds.setAttr(n + ".input", 4.5)
print(cmds.getAttr(n + ".output"))
cmds.delete(n)
cmds.unloadPlugin("helloV1")
print(cmds.pluginInfo("helloV1", q=True, loaded=True))
"#
        ),
        &["['helloV1'] Tester", "hello x3 hello x1", "5.5", "False"],
    );
}

#[test]
fn python_api_v1() {
    aztek_init();
    expect("import maya.OpenMaya as a, maya.api.OpenMaya as b\nprint(issubclass(a.MDagPath, b.MDagPath), a.MDagPath is b.MDagPath)", &["True False"]);
    paths_and_selection();
    transforms_and_values();
    meshes();
    iterators();
    plugs_and_commands();
    curves_and_skins();
    plugins();
}
