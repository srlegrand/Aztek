import maya.cmds as cmds
import maya.mel as mel
import maya.api.OpenMaya as om
import math, traceback

def step(fn):
    try:
        fn(); print("ok  " + fn.__name__)
    except Exception as e:
        print("FAIL " + fn.__name__ + ": " + repr(e))
        print(traceback.format_exc().splitlines()[-2:])

def make_ctrl(name, parent=None, r=1.0, color=17):
    c = cmds.circle(name=name, nr=(1,0,0), r=r, ch=False)[0]
    shp = cmds.listRelatives(c, s=True, f=True)[0]
    cmds.setAttr(shp + '.overrideEnabled', 1)
    cmds.setAttr(shp + '.overrideColor', color)
    grp = cmds.group(c, n=name + '_grp')
    if parent:
        cmds.parent(grp, parent)
    return c, grp

def arm():
    cmds.select(cl=True)
    a = cmds.joint(p=(0,10,0), n='L_shoulder_jnt')
    b = cmds.joint(p=(4,10,-1), n='L_elbow_jnt')
    c = cmds.joint(p=(8,10,0), n='L_wrist_jnt')
    cmds.joint(a, e=True, oj='xyz', sao='yup', ch=True, zso=True)
    cmds.setAttr(c + '.jointOrient', 0, 0, 0, type='double3')
    assert cmds.listRelatives(a, ad=True, type='joint') == [c, b] or set(cmds.listRelatives(a, ad=True, type='joint')) == {b, c}
    assert cmds.objectType(a) == 'joint' and cmds.nodeType(a, i=True)[-1] == 'joint'
    h = cmds.ikHandle(sj=a, ee=c, sol='ikRPsolver', n='L_arm_ikh')[0]
    ct, g = make_ctrl('L_arm_ctrl', r=2)
    cmds.delete(cmds.pointConstraint(c, g))
    cmds.parent(h, ct)
    pv = cmds.spaceLocator(n='L_elbow_pv')[0]
    cmds.xform(pv, ws=True, t=cmds.xform(b, q=True, ws=True, t=True))
    cmds.move(0, 0, -5, pv, r=True, os=True)
    cmds.poleVectorConstraint(pv, h)
    cmds.addAttr(ct, ln='twist', at='double', k=True, dv=0)
    cmds.connectAttr(ct + '.twist', h + '.twist')
    cmds.setAttr(ct + '.twist', 15)
    assert cmds.getAttr(h + '.twist') == 15
    cmds.addAttr(ct, ln='stretch', at='double', min=0, max=1, dv=1, k=True)
    assert cmds.attributeQuery('stretch', node=ct, ex=True) and cmds.attributeQuery('stretch', node=ct, max=True) == [1.0]
    cmds.move(-1, 1, 0, ct, r=True)
    p = cmds.xform(c, q=True, ws=True, t=True)
    assert abs(p[0]-7) < .01 and abs(p[1]-11) < .01, p

def stretchy():
    d = cmds.createNode('distanceBetween', n='stretch_dist')
    a = cmds.spaceLocator(n='sa')[0]; b = cmds.spaceLocator(n='sb')[0]
    cmds.setAttr(b + '.tx', 5)
    cmds.connectAttr(a + 'Shape.worldPosition[0]', d + '.point1')
    cmds.connectAttr(b + 'Shape.worldPosition[0]', d + '.point2')
    assert abs(cmds.getAttr(d + '.distance') - 5) < 1e-6
    m = cmds.shadingNode('multiplyDivide', asUtility=True, n='ratio')
    cmds.setAttr(m + '.operation', 2)
    cmds.connectAttr(d + '.distance', m + '.input1X')
    cmds.setAttr(m + '.input2X', 2.0)
    assert cmds.getAttr(m + '.outputX') == 2.5
    co = cmds.createNode('condition')
    cmds.setAttr(co + '.operation', 2)
    cmds.connectAttr(m + '.outputX', co + '.firstTerm')
    cmds.setAttr(co + '.secondTerm', 1)
    cmds.connectAttr(m + '.outputX', co + '.colorIfTrueR')
    t = cmds.polyCube(n='scaled')[0]; cmds.connectAttr(co + '.outColorR', t + '.sx')
    assert cmds.getAttr(t + '.sx') == 2.5
    inf = cmds.listConnections(co + '.firstTerm', s=True, d=False, p=True)
    assert inf == ['ratio.outputX'], inf

def skin():
    m = cmds.polyCylinder(n='body', h=10, r=1, sy=10, ax=(0,1,0), ch=False)[0]
    cmds.select(cl=True)
    j = [cmds.joint(p=(0,y,0), n="s_%d" % (y+5)) for y in (-5,0,5)]
    sc = cmds.skinCluster(j[0], m, tsb=False, mi=3, n='bodySkin')[0]
    assert mel.eval('findRelatedSkinCluster("%s")' % m) == sc
    infs = cmds.skinCluster(sc, q=True, inf=True)
    assert len(infs) == 3, infs
    for i in range(cmds.polyEvaluate(m, v=True)):
        v = '%s.vtx[%d]' % (m, i)
        w = cmds.skinPercent(sc, v, q=True, v=True)
        assert abs(sum(w) - 1) < 1e-6
    cmds.skinPercent(sc, m + '.vtx[0:3]', tv=[(j[0], 0.5), (j[1], 0.5)])
    cmds.setAttr(j[1] + '.rz', 30)
    bb = cmds.exactWorldBoundingBox(m)
    assert bb[3] > 1.5, bb
    cmds.setAttr(j[1] + '.rz', 0)
    his = cmds.listHistory(m, type='skinCluster')
    assert his == [sc]

def anim():
    c = cmds.polyCube(n='hopper')[0]
    for t, v in ((1,0),(12,5),(24,0)):
        cmds.setKeyframe(c, at='translateY', t=t, v=v, itt='auto', ott='auto')
    assert cmds.keyframe(c, at='ty', q=True, kc=True) == 3
    assert cmds.keyframe(c, at='ty', q=True, tc=True) == [1.0, 12.0, 24.0]
    assert abs(cmds.getAttr(c + '.ty', t=12) - 5) < 1e-6
    cmds.keyTangent(c, at='ty', t=(12,12), itt='linear', ott='linear')
    cmds.setInfinity(c, at='ty', poi='cycle')
    cmds.currentTime(30)
    assert cmds.currentTime(q=True) == 30
    curves = cmds.listConnections(c + '.ty', type='animCurve')
    assert len(curves) == 1, curves
    cmds.playbackOptions(min=1, max=48)
    assert cmds.playbackOptions(q=True, max=True) == 48
    cmds.setDrivenKeyframe(c + '.sx', cd=c + '.tx', dv=0, v=1)
    cmds.setDrivenKeyframe(c + '.sx', cd=c + '.tx', dv=5, v=3)
    cmds.setAttr(c + '.tx', 5)
    assert abs(cmds.getAttr(c + '.sx') - 3) < 1e-6
    cmds.expression(s='hopper.tz = frame * 2;', n='ex1')
    cmds.currentTime(10)
    assert cmds.getAttr('hopper.tz') == 20
    cmds.currentTime(1)

def model():
    a = cmds.polyCube(w=2, h=2, d=2, sx=2, sy=2, sz=2, n='box')
    assert a == ['box', 'polyCube1'], a
    assert cmds.polyEvaluate('box', f=True) == 24
    assert cmds.polyCube('polyCube1', q=True, w=True) == 2
    cmds.polyCube('polyCube1', e=True, w=3)
    assert cmds.getAttr('polyCube1.width') == 3
    r = cmds.polyExtrudeFacet('box.f[0:3]', ltz=0.5, kft=True)
    assert cmds.nodeType(r[0]) == 'polyExtrudeFace', r
    assert cmds.getAttr(r[0] + '.localTranslateZ') == 0.5
    cmds.polyBevel3('box.e[0]', o=0.1, sg=2)
    cmds.polySmooth('box', dv=1)
    faces = cmds.polyEvaluate('box', f=True)
    assert faces > 24
    cmds.select('box.f[0]')
    v = cmds.polyListComponentConversion(tv=True)
    flat = cmds.ls(v, fl=True)
    assert len(flat) == 4, flat
    cmds.move(0, 1, 0, flat, r=True)
    cmds.select('box')
    cmds.polyNormal('box', nm=0)
    cmds.delete('box', ch=True)
    assert cmds.listHistory('box') == ['boxShape'] or len(cmds.listHistory('box')) <= 2
    cmds.makeIdentity('box', a=True, t=True, r=True, s=True)
    d = cmds.duplicate('box', n='box2', rr=True)
    assert d == ['box2'], d
    u = cmds.polyUnite('box', 'box2', n='both', ch=False)
    assert u[0] == 'both', u
    cmds.polyProjection('both.f[*]', type='Planar', md='y')
    cmds.polyAutoProjection('both.f[*]', lm=0, pb=0, ibd=1, cm=0, l=2, sc=1, o=1, p=6, ps=0.2, ws=0)
    print(cmds.polyEvaluate('both', uv=True), cmds.polyInfo('both.f[0]', fv=True)[0].strip())
    cmds.polyMergeVertex('both', d=0.001)
    cmds.polySoftEdge('both', a=30)

def scene():
    g = cmds.group(em=True, n='root')
    k = cmds.group(em=True, n='kid', p=g)
    assert cmds.listRelatives(g, c=True) == [k]
    assert cmds.ls(k, l=True) == ['|root|kid']
    assert cmds.ls('roo*') == ['root']
    cmds.rename(k, 'kid2')
    assert cmds.objExists('kid2') and not cmds.objExists('kid')
    cmds.setAttr('kid2.tx', 2); cmds.setAttr('kid2.tx', lock=True)
    assert cmds.getAttr('kid2.tx', lock=True)
    cmds.setAttr('kid2.tx', lock=False)
    assert cmds.getAttr('kid2.t') == [(2.0, 0.0, 0.0)]
    assert 'translateX' in cmds.listAttr('kid2', k=True)
    s = cmds.sets('root', n='mySet')
    assert cmds.sets(s, q=True) == ['root']
    cmds.select('root', hi=True)
    assert len(cmds.ls(sl=True)) == 2
    wm = cmds.getAttr('kid2.worldMatrix[0]')
    assert len(wm) == 16 and wm[12] == 2
    sel = om.MSelectionList(); sel.add('kid2')
    assert om.MFnTransform(sel.getDagPath(0)).translation(om.MSpace.kWorld).x == 2
    cmds.namespace(add='ns1')
    cmds.spaceLocator(n='ns1:loc')
    assert cmds.objExists('ns1:loc')
    cmds.undoInfo(openChunk=True); cmds.polyCube(n='tmp'); cmds.polyCube(n='tmp2'); cmds.undoInfo(closeChunk=True)
    cmds.undo()
    assert not cmds.objExists('tmp')
    cmds.select(cl=True)
    grp = cmds.group(cmds.spaceLocator(n='inner')[0], n='inG')
    cmds.parent(grp, 'root')
    g2 = cmds.group('inner', n='inG2')
    assert cmds.listRelatives(g2, p=True) == ['inG'], cmds.listRelatives(g2, p=True)
    print(cmds.about(v=True), cmds.optionVar(q='nope'), cmds.currentUnit(q=True, l=True))

def melstuff():
    r = mel.eval('''
    global proc string[] ctrlsOf(string $p) { string $o[] = `ls ($p + "*")`; return $o; }
    polySphere -n "mx1"; polySphere -n "mx2";
    string $a[] = ctrlsOf("mx");
    int $n = size($a);
    string $res = "";
    for ($x in $a) { if (`objectType $x` == "transform") { setAttr ($x + ".tx") 3; $res += $x + ";"; } }
    $res;
    ''')
    assert 'mx1' in r, r
    assert mel.eval('getAttr mx2.tx') == 3
    mel.eval('select -r mx1; xform -cp; string $s[] = `ls -sl`; print $s;')

for f in (arm, stretchy, skin, anim, model, scene, melstuff):
    cmds.file(new=True, f=True)
    step(f)
