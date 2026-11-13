//! The Python API: aztek.api.OpenAztek, and maya.api.OpenMaya over it, run through the embedded interpreter. Python runs once per
//! process, so one test walks through the groups, each on a new scene.

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

/// Runs Python on a clean log; what it printed comes back. Any Python error fails the test with the error in view.
fn py(src: &str) -> Vec<String> {
    log();
    let c = CString::new(src).unwrap();
    aztek_python(c.as_ptr());
    let l = log();
    let errors: Vec<&String> = l.iter().filter(|(k, _)| *k == 2).map(|(_, t)| t).collect();
    assert!(errors.is_empty(), "Python failed:\n{}\nrunning:\n{src}", errors.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("\n"));
    l.into_iter().filter(|(k, _)| *k == 3).map(|(_, t)| t).collect()
}

/// Runs Python and checks its printed lines, in order.
fn expect(src: &str, want: &[&str]) {
    let got = py(src);
    assert_eq!(got, want, "running:\n{src}");
}

/// `expect` for scripts where a plug-in fails on purpose: its traceback goes to the history as error lines, and the script
/// catches the exception. Those lines must be there, so a failure that went unreported fails too.
fn expect_failing(src: &str, want: &[&str]) {
    log();
    let c = CString::new(src).unwrap();
    aztek_python(c.as_ptr());
    let l = log();
    assert!(l.iter().any(|(k, _)| *k == 2), "expected error lines, running:\n{src}");
    assert!(!l.iter().any(|(k, t)| *k == 2 && t.starts_with("# Error:")), "the script didn't catch it: {l:?}\nrunning:\n{src}");
    let got: Vec<String> = l.into_iter().filter(|(k, _)| *k == 3).map(|(_, t)| t).collect();
    assert_eq!(got, want, "running:\n{src}");
}

fn fresh() {
    py("import maya.cmds as cmds\nimport maya.api.OpenMaya as om\nimport math\ncmds.newScene()");
}

fn mel(src: &str) -> Value {
    let c = CString::new(src).unwrap();
    let p = aztek_mel(c.as_ptr());
    let s = unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned();
    aztek_free(p);
    serde_json::from_str(&s).unwrap()
}

fn values() {
    expect(
        r#"
v = om.MVector(1, 2, 3)
print(v + om.MVector(1, 1, 1), v - om.MVector(1, 1, 1), -v, v * 2, 2 * v, v / 2)
print(om.MVector(1, 0, 0) ^ om.MVector(0, 1, 0), om.MVector(1, 2, 3) * om.MVector(4, 5, 6))
print(om.MVector(3, 4, 0).length(), om.MVector(3, 4, 0).normal(), om.MVector(0, 0, 5).normalize())
print(om.MVector(1, 0, 0).rotateBy(om.MQuaternion(math.pi / 2, om.MVector(0, 0, 1))).isEquivalent(om.MVector(0, 1, 0), 1e-9))
print(om.MVector(1, 0, 0).rotateTo(om.MVector(0, 1, 0)).asMatrix().isEquivalent(om.MEulerRotation(0, 0, math.pi / 2).asMatrix(), 1e-9))
print(om.MPoint(3, 3, 3) - om.MPoint(1, 2, 3), type(om.MPoint(3, 3, 3) - om.MPoint(1, 2, 3)).__name__, om.MPoint(1, 1, 1) + om.MVector(1, 0, 0))
print(om.MPoint(1, 2, 3).distanceTo(om.MPoint(1, 2, 5)), om.MPoint(2, 4, 6, 2).cartesianize())
print(type(om.MFloatVector(1, 2, 3) + om.MFloatVector(1, 1, 1)).__name__, om.MFloatPoint(0.1, 0, 0).x == 0.1)
print(len(om.MVector()), list(om.MPoint(1, 2)), om.MColor(1, 0.5, 0).a, om.MColor(1, 0.5, 0) * 2)
"#,
        &[
            "(2, 3, 4) (0, 1, 2) (-1, -2, -3) (2, 4, 6) (2, 4, 6) (0.5, 1, 1.5)",
            "(0, 0, 1) 32.0",
            "5.0 (0.6, 0.8, 0) (0, 0, 1)",
            "True",
            "True",
            "(2, 1, 0) MVector (2, 1, 1, 1)",
            "2.0 (1, 2, 3, 1)",
            "MFloatVector False",
            "3 [1.0, 2.0, 0.0, 1.0] 1.0 (2, 1, 0, 2)",
        ],
    );
    expect(
        r#"
t = om.MMatrix([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 5, 6, 7, 1])
print(om.MPoint(1, 1, 1) * t, om.MVector(1, 1, 1) * t)
print(t[12], t.getElement(3, 1), len(t), t.transpose()[3], t.inverse()[12], t.det3x3(), t.det4x4())
print((t * t.inverse()).isEquivalent(om.MMatrix.kIdentity, 1e-12), om.MMatrix.kIdentity == om.MMatrix(), t != om.MMatrix())
r = om.MEulerRotation(0.1, 0.2, 0.3).asMatrix()
print((r.adjoint()[0] - r.inverse()[0] * r.det4x4()) < 1e-12, om.MMatrix([[1, 0, 0, 0], [0, 1, 0, 0], [0, 0, 1, 0], [1, 2, 3, 1]])[13])
m = om.MMatrix(); m.setElement(2, 3, 9.0); m[0] = 2; print(m[11], m[0], om.MFloatMatrix(m)[0])
# The reference's rotation: rx turns y toward z, row vectors.
print((om.MVector(0, 1, 0) * om.MEulerRotation(math.pi / 2, 0, 0).asMatrix()).isEquivalent(om.MVector(0, 0, 1), 1e-9))
ok = True
for o in range(6):
    e = om.MEulerRotation(0.3, -0.5, 0.7, o)
    ok = ok and e.isEquivalent(om.MEulerRotation.decompose(e.asMatrix(), o), 1e-9) and e.asQuaternion().asMatrix().isEquivalent(e.asMatrix(), 1e-9)
print(ok)
a, b = om.MQuaternion(0.3, om.MVector(1, 0, 0)), om.MQuaternion(0.4, om.MVector(0, 1, 0))
print((a * b).asMatrix().isEquivalent(a.asMatrix() * b.asMatrix(), 1e-9), om.MQuaternion.slerp(om.MQuaternion(), a, 0.5).isEquivalent(om.MQuaternion(0.15, om.MVector(1, 0, 0)), 1e-9))
print(a.asEulerRotation().isEquivalent(om.MEulerRotation(0.3, 0, 0), 1e-9), round(a.asAxisAngle()[1], 9), (a * a.inverse()).isEquivalent(om.MQuaternion.kIdentity, 1e-9))
print(om.MEulerRotation(0.1, 0.2, 0.3).reorder(om.MEulerRotation.kZYX).asMatrix().isEquivalent(om.MEulerRotation(0.1, 0.2, 0.3).asMatrix(), 1e-9))
"#,
        &[
            "(6, 7, 8, 1) (1, 1, 1)",
            "5.0 6.0 16 5.0 -5.0 1.0 1.0",
            "True True True",
            "True 2.0",
            "9.0 2.0 2.0",
            "True",
            "True",
            "True True",
            "True 0.3 True",
            "True",
        ],
    );
    expect(
        r#"
tm = om.MTransformationMatrix()
tm.setTranslation(om.MVector(1, 2, 3), om.MSpace.kTransform)
tm.setRotation(om.MEulerRotation(0, math.pi / 2, 0))
tm.setScale([2, 2, 2], om.MSpace.kTransform)
print(tm.translation(om.MSpace.kWorld), tm.scale(om.MSpace.kObject), tm.rotationOrder() == om.MTransformationMatrix.kXYZ)
m = tm.asMatrix()
print(m[12], m[13], m[14], round(m[2], 9), om.MPoint(1, 0, 0) * m == om.MPoint(1, 2, 3 - 2) or (om.MPoint(1, 0, 0) * m).isEquivalent(om.MPoint(1, 2, 1), 1e-9))
back = om.MTransformationMatrix(m)
print(back.translation(om.MSpace.kWorld), [round(x, 9) for x in back.scale(om.MSpace.kWorld)], back.rotation().isEquivalent(om.MEulerRotation(0, math.pi / 2, 0), 1e-9), back.asMatrix().isEquivalent(m, 1e-9))
print(om.MAngle(180, om.MAngle.kDegrees).asRadians() == math.pi, om.MDistance(1, om.MDistance.kMeters).asCentimeters(), om.MDistance(1, om.MDistance.kInches).asMillimeters())
print(om.MTime(48, om.MTime.kFilm).asUnits(om.MTime.kSeconds), (om.MTime(10) + om.MTime(5)).value, om.MTime(1, om.MTime.kSeconds) == om.MTime(24, om.MTime.k24FPS), om.MTime(5) < om.MTime(6))
print(om.MSpace.kWorld, om.MSpace.kObject, om.MSpace.kTransform)
p = om.MPlane(); p.setPlane(om.MVector(0, 1, 0), -2); print(p.directedDistance(om.MPoint(0, 5, 0)), p.normal(), p.distance())
b = om.MBoundingBox(om.MPoint(0, 0, 0), om.MPoint(1, 2, 3)); b.expand(om.MPoint(-1, 0, 0))
print(b.min, b.max, b.center, b.width, b.height, b.depth, b.contains(om.MPoint(0, 1, 1)), b.intersects(om.MBoundingBox(om.MPoint(5, 5, 5), om.MPoint(6, 6, 6))))
"#,
        &[
            "(1, 2, 3) [2.0, 2.0, 2.0] True",
            "1.0 2.0 3.0 -2.0 True",
            "(1, 2, 3) [2.0, 2.0, 2.0] True True",
            "True 100.0 25.4",
            "2.0 15.0 True True",
            "4 2 1",
            "3.0 (0, 1, 0) -2.0",
            "(-1, 0, 0, 1) (1, 2, 3, 1) (0, 1, 1.5, 1) 2.0 2.0 3.0 True False",
        ],
    );
    expect(
        r#"
a = om.MPointArray(); a.append(om.MPoint(1, 2, 3)); a.append((4, 5, 6))
print(len(a), a[1], a[-1].y, [p.x for p in a], type(a[0]).__name__)
a[0] = om.MVector(7, 8, 9); print(a[0]); a.remove(1); print(len(a)); a.clear(); print(len(a))
i = om.MIntArray([1, 2, 3]); i.append(4.0); i[0] = 9; print(list(i), i[1:3], len(om.MIntArray(3)), list(om.MDoubleArray(2, 1.5)))
f = om.MFloatArray([0.5]); s = om.MStringArray(["a", "b"]); s.append("c"); print(f[0], list(s), "b" in s)
print(len(om.MVectorArray(2)), type(om.MFloatVectorArray(1)[0]).__name__, type(om.MFloatPointArray(1)[0]).__name__, om.MMatrixArray(1)[0] == om.MMatrix())
i2 = om.MIntArray([1, 2]); i2.setLength(4); print(list(i2)); i2.insert(7, 0); print(list(i2))
"#,
        &[
            "2 (4, 5, 6, 1) 5.0 [1.0, 4.0] MPoint",
            "(7, 8, 9, 1)",
            "1",
            "0",
            "[9, 2, 3, 4] [2, 3] 3 [1.5, 1.5]",
            "0.5 ['a', 'b', 'c'] True",
            "2 MFloatVector MFloatPoint True",
            "[1, 2, 0, 0]",
            "[7, 1, 2, 0, 0]",
        ],
    );
}

fn selection_and_dag() {
    fresh();
    expect(
        r#"
cube = cmds.polyCube()[0]
sel = om.MSelectionList()
print(sel.isEmpty(), sel.length())
sel.add(cube); sel.add(cube + ".vtx[0:3]"); sel.add(cube + ".tx")
print(sel.length(), sel.getSelectionStrings())
print(sel.getDagPath(0).fullPathName(), sel.getDependNode(0).apiType() == om.MFn.kTransform)
path, comp = sel.getComponent(1)
print(path.fullPathName(), comp.apiType() == om.MFn.kMeshVertComponent, comp.hasFn(om.MFn.kComponent), om.MFnSingleIndexedComponent(comp).getElements())
print(sel.getPlug(2).name(), sel.hasItem(cube), sel.hasItem("|pCube1"))
sel.remove(2); print(sel.length())
other = om.MSelectionList(); other.add(cube); sel.merge(other); print(sel.length())
sel.toggle(sel.getDagPath(0), om.MObject.kNullObj); print(sel.getSelectionStrings())
sel.clear(); print(sel.isEmpty())
s = om.MSelectionList(); s.add("pC*"); print(s.getSelectionStrings())
for bad in ("noSuchThing", "pCube1.noAttr"):
    try:
        om.MSelectionList().add(bad)
    except RuntimeError as e:
        print(e)
try:
    om.MSelectionList().getDagPath(0)
except IndexError as e:
    print("IndexError")
"#,
        &[
            "True 0",
            "3 ['pCube1', 'pCube1.vtx[0:3]', 'pCube1.translateX']",
            "|pCube1 True",
            "|pCube1|pCubeShape1 True True [0, 1, 2, 3]",
            "pCube1.translateX True True",
            "2",
            "2",
            "['pCube1.vtx[0:3]']",
            "True",
            "['pCube1', 'pCubeShape1']",
            "(kInvalidParameter): Object does not exist",
            "(kInvalidParameter): Object does not exist",
            "IndexError",
        ],
    );
    expect(
        r#"
cmds.select("pCube1")
print(om.MGlobal.getActiveSelectionList().getSelectionStrings())
s = om.MSelectionList(); s.add("pCube1.f[1:2]")
om.MGlobal.setActiveSelectionList(s); print(cmds.ls(sl=True))
om.MGlobal.selectByName("pCube1"); print(cmds.ls(sl=True))
om.MGlobal.setActiveSelectionList(om.MSelectionList()); print(cmds.ls(sl=True))
om.MGlobal.displayInfo("info line")
om.MGlobal.executeCommand("spaceLocator -n fromMel")
print(cmds.objExists("fromMel"), om.MGlobal.executeCommandStringResult("objectType pCube1"), om.MGlobal.executePythonCommand("6 * 7"))
print(om.MGlobal.mayaState() == om.MGlobal.kInteractive, om.MGlobal.apiVersion() > 20000000)
"#,
        &["['pCube1']", "['pCube1.f[1:2]']", "['pCube1']", "[]", "info line", "True transform 42", "True True"],
    );
    let warned = {
        log();
        let c = CString::new("om.MGlobal.displayWarning('careful')\nom.MGlobal.displayError('wrong')").unwrap();
        aztek_python(c.as_ptr());
        log()
    };
    assert!(warned.iter().any(|(k, t)| *k == 4 && t.contains("careful")), "{warned:?}");
    assert!(warned.iter().any(|(k, t)| *k == 2 && t.contains("wrong")), "{warned:?}");
    expect(
        r#"
fresh = cmds.group(empty=True, name="grp")
cmds.parent("pCube1", "grp")
cmds.move(1, 2, 3, "grp")
s = om.MSelectionList(); s.add("pCube1")
p = s.getDagPath(0)
print(p.fullPathName(), p.partialPathName(), p.length(), p.isValid(), p.numberOfShapesDirectlyBelow(), p.apiType() == om.MFn.kTransform, p.hasFn(om.MFn.kDagNode))
print(p.inclusiveMatrix()[13], p.exclusiveMatrix()[12], om.MFnDependencyNode(p.node()).name(), om.MFnDependencyNode(p.transform()).name())
shape = om.MDagPath(p); shape.extendToShape()
print(shape.fullPathName(), shape.hasFn(om.MFn.kMesh), om.MFnDependencyNode(shape.transform()).name(), shape.inclusiveMatrix()[14], shape.childCount())
print(p.childCount(), om.MFnDependencyNode(p.child(0)).name(), p.getPath() == p)
p.pop(); print(p.fullPathName())
print(om.MDagPath().isValid(), om.MDagPath.getAPathTo(shape.node()).fullPathName())
# An MObject follows its node through a rename.
obj = s.getDependNode(0); h = om.MObjectHandle(obj)
cmds.rename("pCube1", "renamed"); print(om.MFnDependencyNode(obj).name(), h.isValid(), h.isAlive())
cmds.delete("renamed"); print(h.isValid())
print(om.MObject().isNull(), om.MObject.kNullObj.apiType() == om.MFn.kInvalid)
"#,
        &[
            "|grp|pCube1 pCube1 2 True 1 True True",
            "2.0 1.0 pCube1 pCube1",
            "|grp|pCube1|pCubeShape1 True pCube1 3.0 0",
            "1 pCubeShape1 True",
            "|grp",
            "False |grp|pCube1|pCubeShape1",
            "renamed True True",
            "False",
            "True True",
        ],
    );
    expect(
        r#"
cmds.newScene()
mod = om.MDagModifier()
a = mod.createNode("transform"); b = mod.createNode("transform", a)
mod.renameNode(a, "modA"); mod.renameNode(b, "modB")
mod.doIt()
print(om.MDagPath.getAPathTo(b).fullPathName())
mod.undoIt(); print(cmds.objExists("modA"), cmds.objExists("modB"))
mod.doIt(); print(cmds.objExists("modA"), om.MDagPath.getAPathTo(b).fullPathName())
c1, c2 = cmds.polyCube()[0], cmds.polyCube()[0]
s = om.MSelectionList(); s.add(c1 + ".tx"); s.add(c2 + ".ty")
dg = om.MDGModifier(); dg.connect(s.getPlug(0), s.getPlug(1)); dg.doIt()
print(s.getPlug(1).isConnected(), s.getPlug(1).source().name())
dg.undoIt(); print(s.getPlug(1).isConnected())
d2 = om.MDGModifier(); d2.connect(s.getPlug(0), s.getPlug(1)); d2.doIt()
d3 = om.MDGModifier(); d3.disconnect(s.getPlug(0), s.getPlug(1)); d3.doIt(); print(s.getPlug(1).isConnected())
d4 = om.MDGModifier(); n = d4.createNode("transform"); d4.doIt(); d5 = om.MDGModifier(); d5.deleteNode(n); name = om.MFnDependencyNode(n).name(); d5.doIt(); print(cmds.objExists(name))
"#,
        &["|modA|modB", "False False", "True |modA|modB", "True pCube1.translateX", "False", "False", "False"],
    );
}

fn plugs_and_function_sets() {
    fresh();
    expect(
        r#"
cube = cmds.polyCube()[0]
loc = cmds.spaceLocator(name="loc")[0]
s = om.MSelectionList(); s.add(cube); s.add(loc)
fn = om.MFnDependencyNode(s.getDependNode(0))
tx = fn.findPlug("translateX", False); r = fn.findPlug("rotate", True)
print(tx.name(), tx.partialName(), tx.partialName(useLongNames=True), tx.partialName(includeNodeName=True), tx.info)
tx.setDouble(2.5); print(tx.asDouble(), tx.asFloat(), tx.asInt(), cmds.getAttr(cube + ".tx"))
print(r.isCompound(), r.numChildren(), r.child(1).name(), r.child(1).parent().name(), r.child(1).isChild())
# Angles are radians here, degrees in getAttr.
r.child(1).setDouble(math.pi / 2); print(cmds.getAttr(cube + ".ry"), r.child(1).asMAngle().asDegrees())
fn.findPlug("tz", False).setMDistance(om.MDistance(1, om.MDistance.kMeters)); print(fn.findPlug("tz", False).asMDistance().asMeters())
print(fn.findPlug("visibility", False).asBool(), tx.isKeyable(), tx.isLocked(), tx.isArray(), tx.isNull(), om.MPlug().isNull())
tx.setLocked(True); print(tx.isLocked()); tx.setLocked(False)
cmds.addAttr(cube, longName="dyn", attributeType="double")
dyn = fn.findPlug("dyn", False); dyn.setKeyable(True); print(dyn.isKeyable())
src = om.MFnDependencyNode(s.getDependNode(1)).findPlug("translateY", False)
cmds.connectAttr(loc + ".ty", cube + ".sx")
sx = fn.findPlug("sx", False)
print(sx.isConnected(), sx.isDestination(), sx.source().name(), src.isSource(), [p.name() for p in src.destinations()], [p.name() for p in src.connectedTo(False, True)], [p.name() for p in sx.connectedTo(True, False)])
print(sorted(p.name() for p in om.MFnDependencyNode(s.getDependNode(1)).getConnections()))
print(om.MPlug(s.getDependNode(0), fn.attribute("tx")).name(), om.MFnDependencyNode(tx.node()).name(), tx.attribute().hasFn(om.MFn.kAttribute), tx == fn.findPlug("tx", False))
wm = fn.findPlug("worldMatrix", False)
print(wm.isArray(), wm.numElements(), wm.elementByLogicalIndex(0).name(), wm.elementByLogicalIndex(0).isElement(), wm.elementByLogicalIndex(0).logicalIndex(), list(wm.getExistingArrayAttributeIndices()), wm.elementByPhysicalIndex(0).name())
print(om.MFnMatrixData(wm.elementByLogicalIndex(0).asMObject()).matrix()[12])
cmds.addAttr(cube, longName="note", dataType="string"); fn.findPlug("note", False).setString("hi"); print(fn.findPlug("note", False).asString())
cmds.addAttr(cube, longName="level", attributeType="enum", enumName="low:high"); fn.findPlug("level", False).setInt(1); print(fn.findPlug("level", False).asInt(), fn.findPlug("level", False).asString())
cmds.addAttr(cube, longName="things", attributeType="double", multi=True)
cmds.setAttr(cube + ".things[0]", 1.0); cmds.setAttr(cube + ".things[3]", 3.0)
th = fn.findPlug("things", False)
print(th.isArray(), th.numElements(), list(th.getExistingArrayAttributeIndices()), th.elementByLogicalIndex(3).asDouble(), th.elementByPhysicalIndex(1).logicalIndex(), th.elementByLogicalIndex(3).array().name())
"#,
        &[
            "pCube1.translateX tx translateX pCube1.tx pCube1.translateX",
            "2.5 2.5 2 2.5",
            "True 3 pCube1.rotateY pCube1.rotate True",
            "90.0 90.0",
            "1.0",
            "True True False False False True",
            "True",
            "True",
            "True True loc.translateY True ['pCube1.scaleX'] ['pCube1.scaleX'] ['loc.translateY']",
            "['loc.translateY']",
            "pCube1.translateX pCube1 True True",
            "True 1 pCube1.worldMatrix[0] True 0 [0] pCube1.worldMatrix[0]",
            "2.5",
            "hi",
            "1 high",
            "True 2 [0, 3] 3.0 3 pCube1.things",
        ],
    );
    // Dependency and DAG function sets.
    expect(
        r#"
cmds.newScene()
fn = om.MFnDependencyNode()
obj = fn.create("transform", "made")
print(fn.name(), fn.typeName, fn.typeName(), fn.hasAttribute("tx"), fn.hasAttribute("nope"), fn.attributeCount() > 5, fn.attribute("tx").apiType() == om.MFn.kNumericAttribute)
print(fn.setName("again"), fn.name(), om.MFnDependencyNode(obj).object() == obj)
try:
    fn.findPlug("nope", False)
except RuntimeError as e:
    print("RuntimeError")
na = om.MFnNumericAttribute(); a = na.create("speed", "spd", om.MFnNumericData.kDouble, 2.5); na.keyable = True; na.setMin(0); na.setMax(10)
fn.addAttribute(a); print(cmds.getAttr("again.speed"), fn.findPlug("speed", False).isKeyable(), fn.findPlug("speed", False).isDynamic())
ta = om.MFnTypedAttribute(); fn.addAttribute(ta.create("label", "lb", om.MFnData.kString)); fn.findPlug("label", False).setString("x"); print(cmds.getAttr("again.label"))
ea = om.MFnEnumAttribute(); e = ea.create("mode", "md", 1); ea.addField("a", 0); ea.addField("b", 1); fn.addAttribute(e); print(cmds.getAttr("again.mode"), fn.findPlug("mode", False).asString())
pa = om.MFnNumericAttribute(); fn.addAttribute(pa.createPoint("spot", "sot")); print(fn.findPlug("spot", False).numChildren(), fn.findPlug("spotX", False).asDouble())
fn.removeAttribute(a); print(fn.hasAttribute("speed"))
dag = om.MFnDagNode()
g = dag.create("transform", "root")
kid = om.MFnDagNode().create("transform", "kid", g)
loc = om.MFnDagNode().create("locator")
print(om.MFnDependencyNode(loc).typeName, dag.childCount(), om.MFnDependencyNode(dag.child(0)).name(), om.MFnDagNode(kid).fullPathName(), om.MFnDagNode(kid).partialPathName(), om.MFnDependencyNode(om.MFnDagNode(kid).parent(0)).name(), dag.parentCount(), dag.parent(0).apiType() == om.MFn.kWorld)
dag.addChild(loc); print(om.MFnDagNode(loc).fullPathName(), om.MFnDagNode(kid).getPath().fullPathName(), bool(dag.isIntermediateObject), dag.isParentOf(kid))
c = cmds.polyCube(w=2, h=4, d=6)[0]; cmds.move(1, 0, 0, c)
s = om.MSelectionList(); s.add(c)
d = om.MFnDagNode(s.getDagPath(0)); b = d.boundingBox
print(b.min, b.max, d.transformationMatrix()[12], om.MFnDagNode(s.getDagPath(0).extendToShape()).boundingBox().width)
"#,
        &[
            "made transform transform True False True True",
            "again again True",
            "RuntimeError",
            "2.5 True True",
            "x",
            "1 b",
            "3 0.0",
            "False",
            "transform 1 kid |root|kid kid root 1 True",
            "|root|locator1 |root|kid False True",
            "(-1, -2, -3, 1) (1, 2, 3, 1) 1.0 2.0",
        ],
    );
    expect(
        r#"
cmds.newScene()
t = om.MFnTransform()
obj = t.create()
print(t.typeName, t.translation(om.MSpace.kTransform), t.scale(), t.rotation())
t.setTranslation(om.MVector(1, 2, 3), om.MSpace.kTransform); t.translateBy(om.MVector(1, 0, 0), om.MSpace.kTransform); print(t.translation(om.MSpace.kObject))
t.setRotation(om.MEulerRotation(0, 0, math.pi / 2)); print(t.rotation().isEquivalent(om.MEulerRotation(0, 0, math.pi / 2), 1e-9), t.rotation(asQuaternion=True).isEquivalent(om.MQuaternion(math.pi / 2, om.MVector(0, 0, 1)), 1e-9))
t.setRotation(om.MQuaternion(math.pi / 4, om.MVector(0, 1, 0)), om.MSpace.kTransform); print(round(cmds.getAttr(t.name() + ".ry"), 6))
t.rotateBy(om.MQuaternion(math.pi / 4, om.MVector(0, 1, 0)), om.MSpace.kTransform); print(round(cmds.getAttr(t.name() + ".ry"), 6))
t.setScale([2, 3, 4]); t.scaleBy([2, 1, 1]); print(t.scale())
t.setRotatePivot(om.MPoint(1, 0, 0), om.MSpace.kTransform, False); t.setScalePivot(om.MPoint(0, 1, 0), om.MSpace.kTransform, False); print(t.rotatePivot(om.MSpace.kTransform), t.scalePivot(om.MSpace.kTransform))
print(t.rotationOrder() == om.MTransformationMatrix.kXYZ)
s = om.MSelectionList(); s.add(t.name())
print(t.transformation().asMatrix().isEquivalent(s.getDagPath(0).inclusiveMatrix(), 1e-6))
tm = om.MTransformationMatrix(); tm.setTranslation(om.MVector(5, 0, 0), om.MSpace.kTransform); tm.setRotation(om.MEulerRotation(0.1, 0.2, 0.3)); tm.setScale([1, 2, 1], om.MSpace.kTransform)
u = om.MFnTransform(); u.create(); u.setTransformation(tm)
s2 = om.MSelectionList(); s2.add(u.name()); print(tm.asMatrix().isEquivalent(s2.getDagPath(0).inclusiveMatrix(), 1e-6))
# World space under a moved parent.
p = om.MFnTransform(); pobj = p.create(); p.setTranslation(om.MVector(10, 0, 0), om.MSpace.kTransform)
k = om.MFnTransform(); k.create(pobj); k.setTranslation(om.MVector(1, 1, 1), om.MSpace.kTransform)
print(k.translation(om.MSpace.kWorld), k.translation(om.MSpace.kTransform))
k.setTranslation(om.MVector(0, 0, 0), om.MSpace.kWorld); print(k.translation(om.MSpace.kTransform))
k.setRotation(om.MEulerRotation(0, 0, 0.5), om.MSpace.kWorld); print(k.rotation(om.MSpace.kWorld).isEquivalent(om.MEulerRotation(0, 0, 0.5), 1e-9))
try:
    om.MFnTransform(om.MObject())
except RuntimeError as e:
    print(e)
s3 = om.MSelectionList(); s3.add(cmds.polyCube()[0]); s3.add(cmds.polyCube()[1])
try:
    om.MFnTransform(s3.getDependNode(1))
except RuntimeError as e:
    print(e)
"#,
        &[
            "transform (0, 0, 0) [1.0, 1.0, 1.0] (0, 0, 0, kXYZ)",
            "(2, 2, 3)",
            "True True",
            "45.0",
            "90.0",
            "[4.0, 3.0, 4.0]",
            "(1, 0, 0, 1) (0, 1, 0, 1)",
            "True",
            "True",
            "True",
            "(11, 1, 1) (1, 1, 1)",
            "(-10, 0, 0)",
            "True",
            "(kInvalidParameter): Object is incompatible with this method",
            "(kInvalidParameter): Object is incompatible with this method",
        ],
    );
    // Sets.
    expect(
        r#"
cmds.newScene()
a, b = cmds.polyCube()[0], cmds.polyCube()[0]
s = om.MSelectionList(); s.add(a)
fn = om.MFnSet(); obj = fn.create(s)
print(obj.hasFn(om.MFn.kSet), fn.getMembers().getSelectionStrings())
s2 = om.MSelectionList(); s2.add(b); fn.addMember(s2.getDagPath(0)); print(fn.getMembers().length(), fn.isMember(s2.getDagPath(0)))
fn.removeMember(s2.getDagPath(0)); print(fn.getMembers().getSelectionStrings(), fn.isMember(s2.getDagPath(0)))
"#,
        &["True ['pCube1']", "2 True", "['pCube1'] False"],
    );
}

fn meshes() {
    fresh();
    expect(
        r#"
c = cmds.polyCube()[0]
s = om.MSelectionList(); s.add(c)
path = s.getDagPath(0)
m = om.MFnMesh(path)
print(m.numVertices, m.numEdges, m.numPolygons, m.numFaceVertices, m.numVertices(), m.numPolygons(), m.numNormals, m.numUVs(), m.getUVSetNames(), m.getCurrentUVSetName())
cmds.move(1, 0, 0, c)
print(m.getPoint(0), m.getPoint(0, om.MSpace.kWorld), m.getPoints(om.MSpace.kWorld)[0], len(m.getPoints()))
counts, conn = m.getVertices(); print(list(counts), len(conn), list(m.getPolygonVertices(0)), m.polygonVertexCount(0), m.getEdgeVertices(0))
print(m.getPolygonNormal(0), m.getFaceVertexNormal(0, list(m.getPolygonVertices(0))[0]), len(m.getNormals()))
print([round(x, 4) for x in m.getVertexNormal(0, True)], m.area(), m.area(om.MSpace.kWorld))
tc, tv = m.getTriangles(); print(list(tc), len(tv), m.getPolygonTriangleVertices(0, 1))
u, v = m.getUVs(); print(len(u), len(v), m.getPolygonUV(0, 0), m.getPolygonUVid(0, 0))
print(m.getPointAtUV(0, *m.getPolygonUV(0, 2)))
hit = m.closestIntersection(om.MFloatPoint(1, 0, 5), om.MFloatVector(0, 0, -1), om.MSpace.kWorld, 100, False)
print(hit[0], hit[1], hit[2], hit[3] >= 0)
print(m.closestIntersection(om.MFloatPoint(50, 0, 5), om.MFloatVector(0, 0, -1), om.MSpace.kWorld, 100, False)[2])
print(m.anyIntersection(om.MFloatPoint(1, 0, 5), om.MFloatVector(0, 0, -1), om.MSpace.kWorld, 100, False, accelParams=m.autoUniformGridParams())[2] >= 0)
allh = m.allIntersections(om.MFloatPoint(1, 0, 5), om.MFloatVector(0, 0, -1), om.MSpace.kWorld, 100, False, sortHits=True)
print(len(allh[0]), [round(x, 3) for x in allh[1]], allh[0][1])
print(m.getClosestPoint(om.MPoint(0, 5, 0)), m.getClosestPoint(om.MPoint(1, 5, 0), om.MSpace.kWorld)[0], m.getClosestNormal(om.MPoint(0, 5, 0))[0])
pts = m.getPoints()
for i in range(len(pts)):
    pts[i] = pts[i] * 2
m.setPoints(pts); print(m.getPoint(0), cmds.pointPosition(c + ".vtx[0]", l=True))
cmds.undo(); print(m.getPoint(0))
m.setPoint(0, om.MPoint(-3, -3, 3)); print(m.getPoint(0)); m.updateSurface(); print(cmds.pointPosition(c + ".vtx[0]", l=True))
cmds.undo(); print(m.getPoint(0))
m.setPoint(1, om.MPoint(5, 5, 5, 1), om.MSpace.kWorld); print(m.getPoint(1))
cmds.undo()
"#,
        &[
            "8 12 6 24 8 6 24 14 ['map1'] map1",
            "(-0.5, -0.5, 0.5, 1) (0.5, -0.5, 0.5, 1) (0.5, -0.5, 0.5, 1) 8",
            "[4, 4, 4, 4, 4, 4] 24 [0, 1, 2, 3] 4 (0, 1)",
            "(0, 0, 1) (0, 0, 1) 24",
            "[-0.5774, -0.5774, 0.5774] 6.0 6.0",
            "[2, 2, 2, 2, 2, 2] 36 (0, 2, 3)",
            "14 14 (0.375, 0.0) 0",
            "(0.5, 0.5, 0.5, 1)",
            "(1, 0, 0.5, 1) 4.5 0 True",
            "-1",
            "True",
            "2 [4.5, 5.5] (1, 0, -0.5, 1)",
            "(aztek.api.OpenAztek.MPoint(0, 0.5, 0, 1), 1) (1, 0.5, 0, 1) (0, 1, 0)",
            "(-1, -1, 1, 1) [-1.0, -1.0, 1.0]",
            "(-0.5, -0.5, 0.5, 1)",
            "(-3, -3, 3, 1)",
            "[-3.0, -3.0, 3.0]",
            "(-0.5, -0.5, 0.5, 1)",
            "(4, 5, 5, 1)",
        ],
    );
    expect(
        r#"
cmds.newScene()
fn = om.MFnMesh()
pts = [om.MPoint(0, 0, 0), om.MPoint(1, 0, 0), om.MPoint(1, 1, 0), om.MPoint(0, 1, 0)]
obj = fn.create(pts, [4], [0, 1, 2, 3])
print(obj.apiType() == om.MFn.kTransform, fn.object().hasFn(om.MFn.kMesh), fn.numVertices, fn.numPolygons, fn.numEdges, fn.numUVs())
u = om.MFloatArray([0, 1, 1, 0]); v = om.MFloatArray([0, 0, 1, 1])
fn.setUVs(u, v); fn.assignUVs([4], [0, 1, 2, 3])
print(fn.numUVs(), fn.getPolygonUV(0, 2), fn.getPointAtUV(0, 0.25, 0.75))
fn.setUVs([0.5, 1, 1, 0], [0, 0, 1, 1]); print(fn.getPolygonUV(0, 0)); print(list(fn.getAssignedUVs()[1]))
try:
    fn.getPointAtUV(0, 5, 5)
except RuntimeError as e:
    print("outside")
print(fn.getClosestPoint(om.MPoint(0.5, 0.5, 2))[0], fn.getPolygonNormal(0), fn.onBoundary(0), fn.isPolygonConvex(0))
"#,
        &[
            "True True 4 1 4 0",
            "4 (1.0, 1.0) (0.25, 0.75, 0, 1)",
            "(0.5, 0.0)",
            "[0, 1, 2, 3]",
            "outside",
            "(0.5, 0.5, 0, 1) (0, 0, 1) True True",
        ],
    );
    // Iterators over mesh components.
    expect(
        r#"
cmds.newScene()
c = cmds.polyCube()[0]
s = om.MSelectionList(); s.add(c); path = s.getDagPath(0)
it = om.MItMeshVertex(path)
print(it.count(), it.index(), it.position(), it.isDone(), it.numConnectedEdges(), it.numConnectedFaces(), it.onBoundary())
print(sorted(it.getConnectedVertices()), sorted(it.getConnectedEdges()), sorted(it.getConnectedFaces()), sorted(it.connectedVertices()) == sorted(it.getConnectedVertices()))
print([round(x, 4) for x in it.getNormal()], len(it.getNormals()), it.hasUVs(), it.numUVs(), it.getUV())
n = 0
while not it.isDone():
    n += 1; it.next()
print(n, it.isDone())
it.reset(); print(it.index()); print(it.setIndex(5), it.index())
# A component narrows it.
s2 = om.MSelectionList(); s2.add(c + ".vtx[2:3]"); p2, comp = s2.getComponent(0)
it2 = om.MItMeshVertex(p2, comp); seen = []
while not it2.isDone():
    seen.append(it2.index()); it2.next()
print(seen)
e = om.MItMeshEdge(path)
print(e.count(), e.index(), e.vertexId(0), e.vertexId(1), e.point(0), e.center(), e.length(), e.onBoundary(), e.numConnectedFaces(), sorted(e.getConnectedFaces()), e.numConnectedEdges(), len(e.getConnectedEdges()))
f = om.MItMeshPolygon(path)
print(f.count(), f.index(), f.polygonVertexCount(), list(f.getVertices()), f.vertexIndex(1), len(f.getEdges()), f.center(), f.getNormal(), f.getArea(), f.numTriangles(), len(f.getPoints()), f.point(0))
print(f.numConnectedFaces(), sorted(f.getConnectedFaces()), f.hasUVs(), f.getUV(0), len(f.getUVs()[0]), list(f.getTriangle(1)[1]), f.isConvex(), f.isPlanar(), f.onBoundary())
print(len(f.getConnectedVertices()), f.numConnectedEdges())
s3 = om.MSelectionList(); s3.add(c + ".f[2]"); p3, comp3 = s3.getComponent(0); f3 = om.MItMeshPolygon(p3, comp3); print(f3.index(), f3.next().isDone())
s4 = om.MSelectionList(); s4.add(c + ".e[1]"); p4, comp4 = s4.getComponent(0); print(om.MItMeshEdge(p4, comp4).index())
fv = om.MItMeshFaceVertex(path); cnt = 0; first = (fv.faceId(), fv.vertexId(), fv.faceVertexId(), fv.position(), fv.getNormal(), fv.hasUVs(), fv.getUV())
while not fv.isDone():
    cnt += 1; last = (fv.faceId(), fv.faceVertexId()); fv.next()
print(cnt, first, last)
g = om.MItGeometry(path); print(g.count(), g.index(), g.position(), len(g.allPositions()))
pts = g.allPositions(); pts[0] = om.MPoint(-1, -1, 1); g.setAllPositions(pts); print(om.MFnMesh(path).getPoint(0)); cmds.undo()
g.setPosition(om.MPoint(-2, -2, 2)); print(om.MFnMesh(path).getPoint(0))
cmds.undo(); print(om.MFnMesh(path).getPoint(0))
for bad in (lambda: om.MItMeshVertex(om.MDagPath()), lambda: om.MItMeshVertex(om.MObject())):
    try:
        bad()
    except RuntimeError as e:
        print("RuntimeError")
"#,
        &[
            "8 0 (-0.5, -0.5, 0.5, 1) False 3 3 False",
            "[1, 3, 7] [0, 3, 11] [0, 3, 5] True",
            "[-0.5774, -0.5774, 0.5774] 3 True 2 (0.375, 0.0)",
            "8 True",
            "0",
            "0 5",
            "[2, 3]",
            "12 0 0 1 (-0.5, -0.5, 0.5, 1) (0, -0.5, 0.5, 1) 1.0 False 2 [0, 3] 4 4",
            "6 0 4 [0, 1, 2, 3] 1 4 (0, 0, 0.5, 1) (0, 0, 1) 1.0 2 4 (-0.5, -0.5, 0.5, 1)",
            "4 [1, 3, 4, 5] True (0.375, 0.0) 4 [0, 2, 3] True True False",
            "4 4",
            "2 True",
            "1",
            "24 (0, 0, 0, aztek.api.OpenAztek.MPoint(-0.5, -0.5, 0.5, 1), aztek.api.OpenAztek.MVector(0, 0, 1), True, (0.375, 0.0)) (5, 3)",
            "8 0 (-0.5, -0.5, 0.5, 1) 8",
            "(-1, -1, 1, 1)",
            "(-2, -2, 2, 1)",
            "(-0.5, -0.5, 0.5, 1)",
            "RuntimeError",
            "RuntimeError",
        ],
    );
}

fn scene_iterators() {
    fresh();
    expect(
        r#"
a = cmds.polyCube()[0]; g = cmds.group(a, name="grp"); j = cmds.joint(name="j1"); cmds.select(clear=True); l = cmds.spaceLocator(name="loc")[0]
sel = om.MSelectionList(); sel.add(a); sel.add(a + ".vtx[1]"); sel.add("polyCube1"); sel.add(a + ".tx")
it = om.MItSelectionList(sel)
out = []
while not it.isDone():
    out.append((it.itemType(), it.hasComponents(), it.getStrings()))
    it.next()
print(out)
it.reset(); print(it.getDagPath().fullPathName(), om.MFnDependencyNode(it.getDependNode()).name(), it.getComponent()[1].isNull())
f = om.MItSelectionList(sel, om.MFn.kTransform); n = 0
while not f.isDone():
    n += 1; f.next()
print(n)
v = om.MItSelectionList(sel, om.MFn.kMeshVertComponent); print(v.getStrings(), v.getComponent()[1].hasFn(om.MFn.kMeshVertComponent))
cams = ("persp", "perspShape", "top", "topShape", "front", "frontShape", "side", "sideShape")
names = []; d = om.MItDag(om.MItDag.kDepthFirst)
while not d.isDone():
    names.append((d.partialPathName(), d.depth())); d.next()
print([x for x in names if x[0] not in cams])
m = om.MItDag(om.MItDag.kDepthFirst, om.MFn.kMesh); print(m.fullPathName(), m.getPath().fullPathName(), m.currentItem().hasFn(om.MFn.kMesh), m.next().isDone())
b = om.MItDag(om.MItDag.kBreadthFirst); order = []
while not b.isDone():
    order.append(b.partialPathName()); b.next()
print([x for x in order if x not in cams])
p = om.MItDag(); out = []
while not p.isDone():
    out.append(p.partialPathName())
    if p.partialPathName() == "grp":
        p.prune()
    p.next()
print([x for x in out if x not in cams])
root = om.MSelectionList(); root.add("grp"); r = om.MItDag(); r.reset(root.getDependNode(0)); out = []
while not r.isDone():
    out.append(r.partialPathName()); r.next()
print(out)
dn = om.MItDependencyNodes(om.MFn.kJoint); print(om.MFnDependencyNode(dn.thisNode()).name(), dn.next().isDone())
total = 0; dn = om.MItDependencyNodes()
while not dn.isDone():
    total += 1; dn.next()
print(total >= 7)
root = om.MSelectionList(); root.add("polyCube1")
g1 = om.MItDependencyGraph(root.getDependNode(0)); out = []
while not g1.isDone():
    out.append(om.MFnDependencyNode(g1.currentNode()).name()); g1.next()
print(out)
sh = om.MSelectionList(); sh.add("pCubeShape1")
g2 = om.MItDependencyGraph(sh.getDependNode(0), om.MFn.kPolyCube, om.MItDependencyGraph.kUpstream); print(om.MFnDependencyNode(g2.currentNode()).name(), g2.next().isDone())
"#,
        &[
            "[(0, False, ['pCube1']), (0, True, ['pCube1.vtx[1]']), (2, False, ['polyCube1']), (3, False, ['pCube1.translateX'])]",
            "|grp|pCube1 pCube1 True",
            "2",
            "['pCube1.vtx[1]'] True",
            "[('grp', 0), ('pCube1', 1), ('pCubeShape1', 2), ('j1', 0), ('loc', 0), ('locShape', 1)]",
            "|grp|pCube1|pCubeShape1 |grp|pCube1|pCubeShape1 True True",
            "['grp', 'j1', 'loc', 'pCube1', 'locShape', 'pCubeShape1']",
            "['grp', 'j1', 'loc', 'locShape']",
            "['grp', 'pCube1', 'pCubeShape1']",
            "j1 True",
            "True",
            "['polyCube1', 'pCubeShape1', 'initialShadingGroup', 'initialMaterialInfo']",
            "polyCube1 True",
        ],
    );
}

fn curves() {
    fresh();
    expect(
        r#"
cv = cmds.curve(p=[(0, 0, 0), (1, 0, 0), (2, 0, 0), (3, 0, 0)], d=3)
s = om.MSelectionList(); s.add(cv); path = s.getDagPath(0)
c = om.MFnNurbsCurve(path)
print(c.numCVs, c.degree, c.numSpans, c.form == om.MFnNurbsCurve.kOpen, c.numCVs(), list(c.knots()), c.knotDomain)
print(c.length(), c.getPointAtParam(0.5), c.tangent(0.5).normal(), c.cvPosition(2), len(c.cvPositions()))
print(round(c.findParamFromLength(1.5), 3), c.closestPoint(om.MPoint(1.5, 1, 0))[0].isEquivalent(om.MPoint(1.5, 0, 0), 1e-3), c.isParamOnCurve(0.5), c.isParamOnCurve(2))
cmds.move(0, 5, 0, cv)
print(c.cvPosition(0, om.MSpace.kWorld), c.getPointAtParam(0, om.MSpace.kWorld), c.cvPositions(om.MSpace.kWorld)[3])
c.setCVPosition(1, om.MPoint(1, 1, 0)); print(c.cvPosition(1), cmds.arclen(cv) > 3.0)
cmds.undo(); print(c.cvPosition(1))
pts = c.cvPositions(); pts[3] = om.MPoint(9, 0, 0); c.setCVPositions(pts); print(c.cvPosition(3))
fn = om.MFnNurbsCurve()
o = fn.create([om.MPoint(0, 0, 0), om.MPoint(0, 1, 0), om.MPoint(0, 2, 0)], [0, 1, 2], 1, om.MFnNurbsCurve.kOpen, False, False)
print(o.apiType() == om.MFn.kTransform, fn.object().hasFn(om.MFn.kNurbsCurve), fn.numCVs, fn.degree, fn.numSpans, fn.length())
circ = cmds.circle()[0]; s2 = om.MSelectionList(); s2.add(circ); k = om.MFnNurbsCurve(s2.getDagPath(0)); print(k.form == om.MFnNurbsCurve.kPeriodic, k.numSpans, k.degree, k.numCVs)
g = om.MItGeometry(s2.getDagPath(0)); print(g.count() == k.numCVs, g.position().distanceTo(k.cvPosition(0)))
comp = om.MSelectionList(); comp.add(cv + ".cv[1:2]"); print(comp.getSelectionStrings(), comp.getComponent(0)[1].apiType() == om.MFn.kCurveCVComponent)
"#,
        &[
            "4 3 1 True 4 [0.0, 0.0, 0.0, 1.0, 1.0, 1.0] (0.0, 1.0)",
            "3.0 (1.5, 0, 0, 1) (1, 0, 0) (2, 0, 0, 1) 4",
            "0.5 True True False",
            "(0, 5, 0, 1) (0, 5, 0, 1) (3, 5, 0, 1)",
            "(1, 1, 0, 1) True",
            "(1, 0, 0, 1)",
            "(9, 0, 0, 1)",
            "True True 3 1 2 2.0",
            "True 8 3 11",
            "True 0.0",
            "['curve1.cv[1:2]'] True",
        ],
    );
}

fn animation() {
    fresh();
    expect(
        r#"
import maya.api.OpenMayaAnim as oma
import aztek.api.OpenMayaAnim as same
print(oma is same)
print(oma.MAnimControl.currentTime().value, oma.MAnimControl.maxTime().value)
oma.MAnimControl.setCurrentTime(om.MTime(12)); print(cmds.currentTime(q=True), oma.MAnimControl.currentTime().value)
oma.MAnimControl.setMinTime(om.MTime(5)); oma.MAnimControl.setMaxTime(om.MTime(50)); print(oma.MAnimControl.minTime().value, oma.MAnimControl.maxTime().value)
oma.MAnimControl.setCurrentTime(om.MTime(1))
c = cmds.polyCube()[0]
cmds.setKeyframe(c, at="ty", t=1, v=0); cmds.setKeyframe(c, at="ty", t=11, v=10)
s = om.MSelectionList(); s.add(c + ".ty"); plug = s.getPlug(0)
curve = oma.MFnAnimCurve(plug)
print(curve.numKeys, curve.numKeys(), curve.input(1).value, curve.value(1), curve.animCurveType == oma.MFnAnimCurve.kAnimCurveTL, curve.object().hasFn(om.MFn.kAnimCurve))
print(curve.findClosest(om.MTime(9)), curve.find(om.MTime(11)), curve.find(om.MTime(3)))
curve.addKey(om.MTime(21), 20.0); print(curve.numKeys, curve.value(2), cmds.keyframe(c, at="ty", q=True, kc=True))
curve.setValue(2, 30.0); print(curve.value(2)); curve.remove(2); print(curve.numKeys)
print(curve.evaluate(om.MTime(1)), curve.evaluate(om.MTime(11)))
s2 = om.MSelectionList(); s2.add(c + ".tz"); nu = oma.MFnAnimCurve(); nu.create(s2.getPlug(0)); nu.addKeys([om.MTime(1), om.MTime(2)], [3.0, 4.0]); print(nu.numKeys, cmds.keyframe(c, at="tz", q=True, vc=True))
s3 = om.MSelectionList(); s3.add(c + ".ry"); rc = oma.MFnAnimCurve(); rc.create(s3.getPlug(0)); rc.addKey(om.MTime(1), math.pi); print(cmds.keyframe(c, at="ry", q=True, vc=True), rc.value(0) == math.pi)
try:
    oma.MFnAnimCurve(s.getDependNode(0))
except RuntimeError as e:
    print(e)
"#,
        &[
            "True",
            "1.0 120.0",
            "12 12.0",
            "5.0 50.0",
            "2 2 11.0 10.0 True True",
            "1 1 None",
            "3 20.0 3",
            "30.0",
            "2",
            "0.0 10.0",
            "2 [3.0, 4.0]",
            "[180.0] True",
            "(kInvalidParameter): Object is incompatible with this method",
        ],
    );
    expect(
        r#"
cmds.newScene()
import maya.api.OpenMayaAnim as oma
mesh = cmds.polyCube(h=4, sy=2)[0]
j1 = cmds.joint(p=(0, -2, 0)); j2 = cmds.joint(p=(0, 2, 0))
sk = cmds.skinCluster(j1, mesh)[0] if False else cmds.skinCluster(j1, j2, mesh)[0]
s = om.MSelectionList(); s.add(sk); s.add(mesh); s.add(j2)
fn = oma.MFnSkinCluster(s.getDependNode(0))
infl = fn.influenceObjects(); print([p.partialPathName() for p in infl], fn.indexForInfluenceObject(s.getDagPath(2)))
shape = s.getDagPath(1).extendToShape()
c = om.MFnSingleIndexedComponent(); comp = c.create(om.MFn.kMeshVertComponent); c.addElements([0, 1])
w, n = fn.getWeights(shape, comp); print(n, len(w), round(sum(w[0:2]), 6))
all_w, n2 = fn.getWeights(shape, om.MObject.kNullObj); print(n2, len(all_w) == om.MFnMesh(shape).numVertices * 2)
one = fn.getWeights(shape, comp, 1); print(len(one))
old = fn.setWeights(shape, comp, om.MIntArray([0, 1]), om.MDoubleArray([0.25, 0.75, 0.5, 0.5]), True, True)
print(len(old), [round(x, 6) for x in fn.getWeights(shape, comp)[0]], cmds.skinPercent(sk, mesh + ".vtx[0]", q=True, t=j2))
sel, wts = fn.getPointsAffectedByInfluence(s.getDagPath(2)); print(sel.length(), len(wts) > 1, list(wts)[0] > 0)
print(fn.getOutputGeometry()[0].hasFn(om.MFn.kMesh), len(fn.getBlendWeights(shape, comp)))
try:
    oma.MFnSkinCluster(s.getDependNode(1))
except RuntimeError as e:
    print(e)
"#,
        &[
            "['joint1', 'joint2'] 1",
            "2 4 1.0",
            "2 True",
            "2",
            "4 [0.25, 0.75, 0.5, 0.5] 0.75",
            "1 True True",
            "True 2",
            "(kInvalidParameter): Object is incompatible with this method",
        ],
    );
}

const PLUGIN: &str = r#"
import maya.api.OpenMaya as om
import maya.cmds as cmds

def maya_useNewAPI():
    pass

class MakeLoc(om.MPxCommand):
    name = "makeLoc"
    def __init__(self):
        om.MPxCommand.__init__(self)
        self.made = None
    @staticmethod
    def creator():
        return MakeLoc()
    @staticmethod
    def syntax_():
        s = om.MSyntax()
        s.addFlag("-h", "-height", om.MSyntax.kDouble)
        s.addFlag("-n", "-label", om.MSyntax.kString)
        s.addFlag("-v", "-verbose")
        s.addFlag("-p", "-pos", om.MSyntax.kDouble, om.MSyntax.kDouble, om.MSyntax.kDouble)
        s.setObjectType(om.MSyntax.kStringObjects)
        return s
    def doIt(self, args):
        db = om.MArgDatabase(self.syntax(), args)
        self.y = db.flagArgumentDouble("-height", 0) if db.isFlagSet("-h") else 1.0
        self.label = db.flagArgumentString("-label", 0) if db.isFlagSet("-label") else "madeLoc"
        self.verbose = db.isFlagSet("verbose")
        self.pos = [db.flagArgumentDouble("-p", i) for i in range(3)] if db.isFlagSet("-p") else None
        self.objs = db.getObjectStrings()
        self.redoIt()
    def redoIt(self):
        self.made = cmds.spaceLocator(name=self.label)[0]
        cmds.setAttr(self.made + ".ty", self.y)
        if self.pos:
            cmds.setAttr(self.made + ".tx", self.pos[0])
        if self.verbose:
            om.MGlobal.displayInfo("made " + self.made)
        self.setResult(self.made)
    def undoIt(self):
        cmds.delete(self.made)
    def isUndoable(self):
        return True

class Asks(om.MPxCommand):
    @staticmethod
    def creator():
        return Asks()
    @staticmethod
    def syntax_():
        s = om.MSyntax()
        s.addFlag("-s", "-size", om.MSyntax.kDouble)
        s.enableQuery = True
        s.enableEdit = True
        return s
    def doIt(self, args):
        db = om.MArgDatabase(self.syntax(), args)
        self.setResult("query" if db.isQuery else "edit" if db.isEdit else "create")

class Plain(om.MPxCommand):
    @staticmethod
    def creator():
        return Plain()
    def doIt(self, args):
        for i in range(args.length()):
            self.appendToResult(args.asString(i))
        self.appendToResult(len(args))

class Boom(om.MPxCommand):
    @staticmethod
    def creator():
        return Boom()
    def doIt(self, args):
        raise RuntimeError("boom went the command")

class Inner(om.MPxCommand):
    """Not undoable: what it runs stays on the stack on its own."""
    @staticmethod
    def creator():
        return Inner()
    def doIt(self, args):
        cmds.polyCube(name="innerCube")
        self.setResult(True)

class ModCmd(om.MPxCommand):
    @staticmethod
    def creator():
        return ModCmd()
    def doIt(self, args):
        self.mod = om.MDagModifier()
        self.node = self.mod.createNode("transform")
        self.mod.renameNode(self.node, "modded")
        self.mod.doIt()
        self.setResult(1.5)
    def undoIt(self):
        self.mod.undoIt()
    def redoIt(self):
        self.mod.doIt()
    def isUndoable(self):
        return True

class Doubler(om.MPxNode):
    kId = om.MTypeId(0x00127A01)
    aInput = None
    aFactor = None
    aOutput = None
    aVec = None
    aVecOut = None
    aAngle = None
    aAngleOut = None
    aName = None
    aNameOut = None
    aMat = None
    aMatOut = None
    aMode = None
    aList = None
    aSum = None
    aFlag = None
    def __init__(self):
        om.MPxNode.__init__(self)
    @staticmethod
    def creator():
        return Doubler()
    @staticmethod
    def initialize():
        n = om.MFnNumericAttribute()
        Doubler.aInput = n.create("input", "in", om.MFnNumericData.kDouble, 1.0)
        n.keyable = True; n.storable = True
        Doubler.aFactor = n.create("factor", "fa", om.MFnNumericData.kFloat, 2.0)
        n.setMin(0.0); n.setMax(10.0)
        Doubler.aOutput = n.create("output", "out", om.MFnNumericData.kDouble, 0.0)
        n.writable = False; n.storable = False
        Doubler.aVec = n.createPoint("vec", "ve")
        Doubler.aVecOut = n.create("vecOut", "vo", om.MFnNumericData.k3Double)
        n.writable = False
        Doubler.aFlag = n.create("flag", "fl", om.MFnNumericData.kBoolean, False)
        n.array = False
        u = om.MFnUnitAttribute()
        Doubler.aAngle = u.create("spin", "sp", om.MFnUnitAttribute.kAngle, om.MAngle(90, om.MAngle.kDegrees))
        Doubler.aAngleOut = u.create("spinOut", "so", om.MFnUnitAttribute.kAngle, 0.0)
        u.writable = False
        t = om.MFnTypedAttribute()
        Doubler.aName = t.create("label", "lb", om.MFnData.kString)
        Doubler.aNameOut = t.create("labelOut", "lo", om.MFnData.kString)
        t.writable = False
        m = om.MFnMatrixAttribute()
        Doubler.aMat = m.create("inMatrix", "im")
        Doubler.aMatOut = m.create("outMatrix", "om")
        m.writable = False
        e = om.MFnEnumAttribute()
        Doubler.aMode = e.create("mode", "mo", 1)
        e.addField("add", 0); e.addField("mul", 1)
        n2 = om.MFnNumericAttribute()
        Doubler.aList = n2.create("items", "it", om.MFnNumericData.kDouble, 0.0)
        n2.array = True; n2.keyable = True
        Doubler.aSum = n2.create("total", "to", om.MFnNumericData.kDouble, 0.0)
        n2.array = False; n2.keyable = False; n2.writable = False
        msg = om.MFnMessageAttribute()
        Doubler.aMsg = msg.create("link", "lk")
        for a in (Doubler.aInput, Doubler.aFactor, Doubler.aOutput, Doubler.aVec, Doubler.aVecOut, Doubler.aFlag, Doubler.aAngle, Doubler.aAngleOut,
                  Doubler.aName, Doubler.aNameOut, Doubler.aMat, Doubler.aMatOut, Doubler.aMode, Doubler.aList, Doubler.aSum, Doubler.aMsg):
            om.MPxNode.addAttribute(a)
        om.MPxNode.attributeAffects(Doubler.aInput, Doubler.aOutput)
        om.MPxNode.attributeAffects(Doubler.aFactor, Doubler.aOutput)
        om.MPxNode.attributeAffects(Doubler.aMode, Doubler.aOutput)
        om.MPxNode.attributeAffects(Doubler.aVec, Doubler.aVecOut)
        om.MPxNode.attributeAffects(Doubler.aAngle, Doubler.aAngleOut)
        om.MPxNode.attributeAffects(Doubler.aName, Doubler.aNameOut)
        om.MPxNode.attributeAffects(Doubler.aMat, Doubler.aMatOut)
        om.MPxNode.attributeAffects(Doubler.aList, Doubler.aSum)
    def compute(self, plug, block):
        if plug == Doubler.aOutput or plug.attribute() == Doubler.aOutput:
            i = block.inputValue(Doubler.aInput).asDouble()
            f = block.inputValue(Doubler.aFactor).asFloat()
            mode = block.inputValue(Doubler.aMode).asShort()
            out = block.outputValue(Doubler.aOutput)
            out.setDouble(i * f if mode == 1 else i + f)
            out.setClean()
            block.setClean(plug)
        elif plug.attribute() == Doubler.aVecOut:
            v = block.inputValue(Doubler.aVec).asVector()
            block.outputValue(Doubler.aVecOut).setMVector(v * 2)
            block.setClean(plug)
        elif plug.attribute() == Doubler.aAngleOut:
            a = block.inputValue(Doubler.aAngle)
            block.outputValue(Doubler.aAngleOut).setMAngle(om.MAngle(a.asAngle().asRadians() * 2))
        elif plug.attribute() == Doubler.aNameOut:
            block.outputValue(Doubler.aNameOut).setString(block.inputValue(Doubler.aName).asString().upper())
        elif plug.attribute() == Doubler.aMatOut:
            block.outputValue(Doubler.aMatOut).setMMatrix(block.inputValue(Doubler.aMat).asMatrix().inverse())
        elif plug.attribute() == Doubler.aSum:
            arr = block.inputArrayValue(Doubler.aList); total = 0.0
            for k in range(arr.elementCount()):
                arr.jumpToPhysicalElement(k)
                total += arr.inputValue().asDouble()
            block.outputValue(Doubler.aSum).setDouble(total)

class Chatty(om.MPxNode):
    kId = om.MTypeId(0x00127A02)
    aIn = None
    aOut = None
    @staticmethod
    def creator():
        return Chatty()
    @staticmethod
    def initialize():
        n = om.MFnNumericAttribute()
        Chatty.aIn = n.create("input", "in", om.MFnNumericData.kDouble, 0.0)
        Chatty.aOut = n.create("output", "out", om.MFnNumericData.kDouble, 0.0)
        n.writable = False
        om.MPxNode.addAttribute(Chatty.aIn)
        om.MPxNode.addAttribute(Chatty.aOut)
        om.MPxNode.attributeAffects(Chatty.aIn, Chatty.aOut)
    def compute(self, plug, block):
        v = block.inputValue(Chatty.aIn).asDouble()
        print("computing %s %g" % (om.MFnDependencyNode(self.thisMObject()).name(), v))
        try:
            cmds.getAttr("persp.tx")
        except RuntimeError as e:
            print("refused: " + str(e))
        block.outputValue(Chatty.aOut).setDouble(v + 1)

def initializePlugin(obj):
    p = om.MFnPlugin(obj, "Testers", "1.2", "Any")
    p.registerCommand("makeLoc", MakeLoc.creator, MakeLoc.syntax_)
    p.registerCommand("plainCmd", Plain.creator)
    p.registerCommand("asksCmd", Asks.creator, Asks.syntax_)
    p.registerCommand("boomCmd", Boom.creator)
    p.registerCommand("innerCmd", Inner.creator)
    p.registerCommand("modCmd", ModCmd.creator)
    p.registerNode("doubler", Doubler.kId, Doubler.creator, Doubler.initialize)
    p.registerNode("chatty", Chatty.kId, Chatty.creator, Chatty.initialize)

def uninitializePlugin(obj):
    p = om.MFnPlugin(obj)
    p.deregisterCommand("makeLoc")
    p.deregisterCommand("plainCmd")
    p.deregisterCommand("boomCmd")
    p.deregisterNode(Doubler.kId)
"#;

fn plugins() {
    fresh();
    let dir = std::env::temp_dir().join(format!("aztek_om_test_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("testPlugin.py"), PLUGIN).unwrap();
    std::fs::write(dir.join("broken.py"), "def maya_useNewAPI():\n    pass\ndef initializePlugin(obj):\n    raise RuntimeError('cannot start')\n").unwrap();
    let dirs = dir.to_str().unwrap().replace('\\', "/");
    expect_failing(
        &format!(
            r#"
import os
print(cmds.pluginInfo("testPlugin", q=True, loaded=True), cmds.pluginInfo(q=True, listPlugins=True))
try:
    cmds.loadPlugin("testPlugin")
except RuntimeError as e:
    print(e)
os.environ["AZTEK_PLUG_IN_PATH"] = "{dirs}"
print(cmds.loadPlugin("testPlugin"))
print(cmds.pluginInfo("testPlugin", q=True, loaded=True), cmds.pluginInfo(q=True, listPlugins=True), cmds.pluginInfo("testPlugin", q=True, vendor=True), cmds.pluginInfo("testPlugin", q=True, version=True), cmds.pluginInfo("testPlugin", q=True, path=True).endswith("testPlugin.py"))
print(sorted(cmds.pluginInfo("testPlugin", q=True, command=True)), sorted(cmds.pluginInfo("testPlugin", q=True, dependNode=True)))
print(cmds.loadPlugin("testPlugin"), "makeLoc" in dir(cmds))
for bad in ("broken",):
    try:
        cmds.loadPlugin(bad)
    except RuntimeError as e:
        print(e)
print(cmds.pluginInfo("broken", q=True, loaded=True), cmds.loadPlugin("noSuch", quiet=True))
"#
        ),
        &[
            // The plug-ins that come with Aztek are loaded from the start.
            "False ['fbx', 'gltf', 'matrixNodes', 'quatNodes']",
            "Plug-in \"testPlugin\" was not found on the plug-in path.",
            "['testPlugin']",
            "True ['fbx', 'gltf', 'matrixNodes', 'quatNodes', 'testPlugin'] Testers 1.2 True",
            "['asksCmd', 'boomCmd', 'innerCmd', 'makeLoc', 'modCmd', 'plainCmd'] ['chatty', 'doubler']",
            "['testPlugin'] True",
            "cannot start",
            "False []",
        ],
    );
    log();
    // Commands, from Python and MEL, with undo.
    expect_failing(
        r#"
r = cmds.makeLoc(height=4.0, label="first", verbose=True)
print(r, cmds.getAttr("first.ty"))
cmds.undo(); print(cmds.objExists("first"))
cmds.redo(); print(cmds.objExists("first"), cmds.getAttr("first.ty"))
cmds.undo(); print(cmds.objExists("first"))
print(cmds.makeLoc(h=2.0, n="short", pos=[7.0, 0.0, 0.0]), cmds.getAttr("short.tx"), cmds.getAttr("short.ty"))
print(cmds.plainCmd("a", "b", foo=3, bar=True))
print(cmds.plainCmd())
try:
    cmds.boomCmd()
except RuntimeError as e:
    print(e)
try:
    cmds.makeLoc(nope=1)
except RuntimeError as e:
    print("bad flag")
print(cmds.modCmd(), cmds.objExists("modded"))
cmds.undo(); print(cmds.objExists("modded"))
cmds.redo(); print(cmds.objExists("modded"))
cmds.undo()
print(cmds.innerCmd(), cmds.objExists("innerCube"))
cmds.undo(); print(cmds.objExists("innerCube"))
"#,
        &[
            "made first",
            "first 4.0",
            "False",
            "made first",
            "True 4.0",
            "False",
            "short 7.0 2.0",
            "['a', 'b', '-foo', '3', '-bar', 'True', 6]",
            "[0]",
            "boom went the command",
            "bad flag",
            "1.5 True",
            "False",
            "True",
            "True True",
            "False",
        ],
    );
    log();
    assert_eq!(mel("makeLoc -h 3 -n fromMel"), Value::String("fromMel".into()));
    let l = log();
    assert!(!l.iter().any(|(k, _)| *k == 2), "{l:?}");
    expect("print(cmds.getAttr('fromMel.ty'))", &["3.0"]);
    mel("undo");
    expect("print(cmds.objExists('fromMel'))", &["False"]);
    assert_eq!(mel("plainCmd one two"), serde_json::json!(["one", "two", 2]));
    assert_eq!(mel("pluginInfo -q -loaded testPlugin"), Value::Bool(true));
    assert_eq!(mel("pluginInfo -q -loaded elsewhere"), Value::Bool(false));
    assert_eq!(mel("asksCmd"), Value::String("create".into()));
    assert_eq!(mel("asksCmd -s 2"), Value::String("create".into()));
    assert_eq!(mel("asksCmd -q -s"), Value::String("query".into()));
    expect("print(cmds.asksCmd(q=True, size=True), cmds.asksCmd(e=True, size=3.0), cmds.asksCmd(size=1.0))", &["query edit create"]);
    log();
    mel("boomCmd");
    let l = log();
    assert!(l.iter().any(|(k, t)| *k == 2 && t.contains("boom went the command")), "{l:?}");
    // Nodes.
    expect(
        r#"
n = cmds.createNode("doubler", name="dbl")
print(cmds.nodeType(n), cmds.getAttr(n + ".output"))
cmds.setAttr(n + ".input", 5.0); print(cmds.getAttr(n + ".output"))
cmds.setAttr(n + ".factor", 3.0); print(cmds.getAttr(n + ".output"))
cmds.setAttr(n + ".mode", 0); print(cmds.getAttr(n + ".output")); cmds.setAttr(n + ".mode", 1)
cmds.setAttr(n + ".factor", 50.0); print(cmds.getAttr(n + ".factor")); cmds.setAttr(n + ".factor", 2.0)
cmds.setAttr(n + ".vec", 1, 2, 3); print(cmds.getAttr(n + ".vecOut"))
print(cmds.getAttr(n + ".spin"), cmds.getAttr(n + ".spinOut"))
cmds.setAttr(n + ".label", "abc"); print(cmds.getAttr(n + ".labelOut"))
cmds.setAttr(n + ".items[0]", 1.0); cmds.setAttr(n + ".items[4]", 2.5); print(cmds.getAttr(n + ".total"))
c = cmds.polyCube()[0]; d = cmds.polyCube()[0]
cmds.connectAttr(c + ".tx", n + ".input"); cmds.connectAttr(n + ".output", d + ".ty")
cmds.setAttr(c + ".tx", 4.0); print(cmds.getAttr(d + ".ty"))
s = om.MSelectionList(); s.add(n)
obj = s.getDependNode(0)
fn = om.MFnDependencyNode(obj)
print(obj.apiType() == om.MFn.kPluginDependNode, obj.hasFn(om.MFn.kDependencyNode), fn.typeName, fn.typeId == om.MTypeId(0x00127A01), fn.typeId.id() == 0x127A01, fn.pluginName, type(fn.userNode()).__name__, fn.userNode().thisMObject() == obj, fn.userNode().name())
print(fn.findPlug("output", False).asDouble(), fn.findPlug("spin", False).asDouble() == math.pi / 2, fn.findPlug("spin", False).asMAngle().asDegrees(), fn.findPlug("items", False).numElements())
fn.findPlug("spin", False).setMAngle(om.MAngle(45, om.MAngle.kDegrees)); print(cmds.getAttr(n + ".spinOut"))
made = om.MFnDependencyNode(); o2 = made.create("doubler"); print(made.typeName, made.name()); print(om.MDGModifier().createNode(om.MTypeId(0x00127A01)).hasFn(om.MFn.kPluginDependNode))
cmds.undo(); cmds.undo()
print(cmds.objExists(n))
cmds.delete(n); print(cmds.objExists(n))
"#,
        &[
            "doubler 2.0",
            "10.0",
            "15.0",
            "8.0",
            "10.0",
            "[(2.0, 4.0, 6.0)]",
            "90.0 180.0",
            "ABC",
            "3.5",
            "8.0",
            "True True doubler True True testPlugin Doubler True dbl",
            "8.0 True 90.0 2",
            "90.0",
            "doubler doubler1",
            "True",
            "True",
            "False",
        ],
    );
    // A compute that prints and tries a command: the print arrives, the command is refused rather than deadlocking.
    expect(
        r#"
ch = cmds.createNode("chatty", name="ch1")
cmds.setAttr(ch + ".input", 2.0)
print(cmds.getAttr(ch + ".output"))
"#,
        &[
            "computing ch1 2",
            "refused: (kFailure): Commands cannot run inside compute",
            "3.0",
        ],
    );
    // Enough dirty nodes at once that the graph computes them on worker threads.
    let got = py(
        r#"
names = [cmds.createNode("doubler") for _ in range(40)]
for i, n in enumerate(names):
    cmds.setAttr(n + ".input", float(i))
print(sum(cmds.getAttr(n + ".output") for n in names))
"#,
    );
    assert_eq!(got, ["1560.0"]);
    // Unloading takes back everything, deregistered or not, and the plugin can come back.
    expect_failing(
        r#"
cmds.delete(cmds.ls(type="doubler"))
print(cmds.unloadPlugin("testPlugin"), cmds.pluginInfo("testPlugin", q=True, loaded=True), "makeLoc" in dir(cmds), "modCmd" in dir(cmds))
try:
    cmds.makeLoc()
except AttributeError:
    print("no command")
try:
    cmds.createNode("doubler")
except RuntimeError as e:
    print(e)
try:
    cmds.unloadPlugin("testPlugin")
except RuntimeError as e:
    print(e)
print(cmds.loadPlugin("testPlugin"), cmds.getAttr(cmds.createNode("doubler") + ".output"))
"#,
        &[
            "None False False False",
            "no command",
            "unknown node type 'doubler'",
            "Plug-in \"testPlugin\" is not loaded.",
            "['testPlugin'] 2.0",
        ],
    );
    std::fs::remove_dir_all(&dir).ok();
}

/// What a real script does, importing from maya.api as such scripts do.
fn real_world() {
    fresh();
    let got = py(
        r#"
from maya.api import OpenMaya
import maya.cmds as cmds

def inflate(amount):
    sel = OpenMaya.MGlobal.getActiveSelectionList()
    it = OpenMaya.MItSelectionList(sel, OpenMaya.MFn.kMesh)
    while not it.isDone():
        path = it.getDagPath()
        mesh = OpenMaya.MFnMesh(path)
        verts = OpenMaya.MItMeshVertex(path)
        points = mesh.getPoints(OpenMaya.MSpace.kObject)
        while not verts.isDone():
            normal = verts.getNormal(OpenMaya.MSpace.kObject)
            points[verts.index()] += OpenMaya.MVector(normal) * amount
            verts.next()
        mesh.setPoints(points, OpenMaya.MSpace.kObject)
        it.next()

sphere = cmds.polySphere(r=2, sx=12, sy=8)[0]
cmds.move(3, 0, 0, sphere)
cmds.select(sphere)
before = cmds.polyEvaluate(sphere, v=True)
inflate(0.5)
sel = OpenMaya.MSelectionList(); sel.add(sphere)
mesh = OpenMaya.MFnMesh(sel.getDagPath(0))
radius = [OpenMaya.MVector(p).length() for p in mesh.getPoints()]
print(before == mesh.numVertices, round(min(radius), 2), round(max(radius), 2))
world = mesh.getPoints(OpenMaya.MSpace.kWorld)[0]
print(world.isEquivalent(mesh.getPoint(0) + OpenMaya.MVector(3, 0, 0), 1e-9))
cmds.undo()
print(round(max(OpenMaya.MVector(p).length() for p in mesh.getPoints()), 4))

# The same with one edit per vertex through the iterator: still a single undo.
verts = OpenMaya.MItMeshVertex(sel.getDagPath(0))
while not verts.isDone():
    verts.setPosition(verts.position() + verts.getNormal() * 0.5)
    verts.next()
verts.updateSurface()
print(round(max(OpenMaya.MVector(p).length() for p in mesh.getPoints()), 4))
cmds.undo()
print(round(max(OpenMaya.MVector(p).length() for p in mesh.getPoints()), 4))

# A joint's world matrix.
cmds.select(clear=True)
root = cmds.joint(p=(0, 0, 0), name="root")
tip = cmds.joint(p=(0, 5, 0), name="tip")
cmds.setAttr(root + ".rz", -90)
sel = OpenMaya.MSelectionList(); sel.add(tip)
path = sel.getDagPath(0)
m = path.inclusiveMatrix()
print(path.hasFn(OpenMaya.MFn.kJoint), OpenMaya.MTransformationMatrix(m).translation(OpenMaya.MSpace.kWorld).isEquivalent(OpenMaya.MVector(5, 0, 0), 1e-9))
pm = OpenMaya.MFnDependencyNode(path.node()).findPlug("worldMatrix", False).elementByLogicalIndex(0)
print(OpenMaya.MFnMatrixData(pm.asMObject()).matrix().isEquivalent(m, 1e-9), (OpenMaya.MPoint(0, 0, 0) * m).isEquivalent(OpenMaya.MPoint(5, 0, 0), 1e-9))
print(OpenMaya.MFnTransform(path).translation(OpenMaya.MSpace.kWorld).isEquivalent(OpenMaya.MVector(5, 0, 0), 1e-9), OpenMaya.MFnTransform(path).transformation().asMatrix().isEquivalent(path.inclusiveMatrix() * path.exclusiveMatrix().inverse(), 1e-9))
"#,
    );
    assert_eq!(got, ["True 2.5 2.5", "True", "2.0", "2.5", "2.0", "True True", "True True", "True True"]);
}

#[test]
fn python_api() {
    aztek_init();
    py("import maya.api.OpenMaya as om\nimport aztek.api.OpenMaya as az\nprint(om is az, az.MVector is om.MVector)");
    expect("import maya.api.OpenMaya as a, aztek.api.OpenMaya as b\nprint(a is b)", &["True"]);
    // OpenAztek is the API's own name; the reference's names, and Aztek's earlier ones, are the same modules.
    expect(
        "import aztek.api.OpenAztek as a, maya.api.OpenMaya as b, aztek.api.OpenAztekAnim as c, maya.api.OpenMayaAnim as d\n\
         import aztek.OpenAztek as e, maya.OpenMaya as f, aztek.OpenAztekUI as g, maya.OpenMayaUI as h, aztek.OpenAztekMPx as i, maya.OpenMayaMPx as j\n\
         print(a is b, c is d, e is f, g is h, i is j, a.__name__, repr(b.MPoint(1, 2, 3)))",
        &["True True True True True aztek.api.OpenAztek aztek.api.OpenAztek.MPoint(1, 2, 3, 1)"],
    );
    fresh();
    values();
    selection_and_dag();
    plugs_and_function_sets();
    meshes();
    scene_iterators();
    curves();
    animation();
    plugins();
    real_world();
}
