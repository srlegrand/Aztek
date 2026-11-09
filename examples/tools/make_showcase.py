"""Writes the showcase scenes in examples/<area>/: one small scene per feature, saved by Aztek itself.
Run from the repository root: AZTEK_EXAMPLES=$PWD/examples ./build/aztek --script examples/tools/make_showcase.py
Each scene is built in its own try, and the ones that failed are listed in examples/tools/make_showcase.log (not kept)."""
import math
import os
import random
import traceback

import maya.cmds as mc
import maya.mel as mel

ROOT = os.environ.get("AZTEK_EXAMPLES", os.path.join(os.getcwd(), "examples"))
LOG = []
ONLY = [s for s in os.environ.get("AZTEK_SHOWCASE_ONLY", "").split(",") if s]


def scene(area, name):
    def wrap(fn):
        if ONLY and name not in ONLY:
            return fn
        try:
            mc.file(new=True, force=True)
            mc.currentUnit(l="cm", a="deg", t="film")
            fn()
            mc.select(cl=True)
            mc.currentTime(mc.playbackOptions(q=True, min=True))
            folder = os.path.join(ROOT, area)
            os.makedirs(folder, exist_ok=True)
            mc.file(rename=os.path.join(folder, name + ".ma"))
            mc.file(save=True, type="mayaAscii", force=True)
            LOG.append("ok   %s/%s" % (area, name))
        except Exception:
            LOG.append("FAIL %s/%s\n%s" % (area, name, traceback.format_exc()))
        return fn
    return wrap


# ---------------------------------------------------------------------------------------------------------- helpers
def look(eye, at, fl=35):
    mc.viewPlace("persp", eye=eye, la=at)
    mc.setAttr("perspShape.focalLength", fl)


def mat(name, color, objs=None, kind="blinn", **attrs):
    s = mc.shadingNode(kind, asShader=True, n=name)
    attr = "baseColor" if kind == "standardSurface" else "color"
    mc.setAttr(s + "." + attr, *color, type="double3")
    for a, v in attrs.items():
        if isinstance(v, (tuple, list)):
            mc.setAttr(s + "." + a, *v, type="double3")
        else:
            mc.setAttr(s + "." + a, v)
    sg = mc.sets(renderable=True, noSurfaceShader=True, empty=True, n=name + "SG")
    mc.connectAttr(s + ".outColor", sg + ".surfaceShader")
    if objs:
        mc.sets(objs, e=True, forceElement=sg)
    return s


def assign(shader, objs):
    sg = mc.listConnections(shader + ".outColor", type="shadingEngine")[0]
    mc.sets(objs, e=True, forceElement=sg)


def checker(name, objs, repeat=4):
    s = mc.shadingNode("lambert", asShader=True, n=name)
    f = mc.shadingNode("file", asTexture=True, n=name + "File")
    p = mc.shadingNode("place2dTexture", asUtility=True, n=name + "Place")
    mc.connectAttr(p + ".outUV", f + ".uvCoord")
    mc.connectAttr(p + ".outUvFilterSize", f + ".uvFilterSize")
    mc.setAttr(p + ".repeatU", repeat)
    mc.setAttr(p + ".repeatV", repeat)
    mc.setAttr(f + ".fileTextureName", "../textures/checker.png", type="string")
    mc.connectAttr(f + ".outColor", s + ".color")
    sg = mc.sets(renderable=True, noSurfaceShader=True, empty=True, n=name + "SG")
    mc.connectAttr(s + ".outColor", sg + ".surfaceShader")
    mc.sets(objs, e=True, forceElement=sg)
    return s


def floor(size=40, name="floor", color=(0.45, 0.45, 0.45)):
    f = mc.polyPlane(n=name, w=size, h=size, sx=1, sy=1)[0]
    mat(name + "Mat", color, f, "lambert")
    return f


def label(text, pos, height=1.0):
    """A plain text label as a curve-free annotation: an annotation locator at pos."""
    loc = mc.spaceLocator(n="label_" + "".join(c for c in text if c.isalnum()))[0]
    mc.move(pos[0], pos[1], pos[2], loc)
    a = mc.annotate(loc, tx=text, p=(pos[0], pos[1] + height, pos[2]))
    return loc


def keys(node, attr, pairs, tangent=None):
    for t, v in pairs:
        mc.setKeyframe(node, at=attr, t=t, v=v)
    if tangent:
        mc.keyTangent(node, at=attr, itt=tangent, ott=tangent)



def face_centre(f):
    ps = [mc.pointPosition(v) for v in mc.ls(mc.polyListComponentConversion(f, tv=True), fl=True)]
    return [sum(p[k] for p in ps) / len(ps) for k in range(3)]


def faces_where(obj, test):
    """Faces whose centre c and normal n pass test(c, n): picks faces by where they are, not by index."""
    out = []
    for i in range(mc.polyEvaluate(obj, f=True)):
        f = "%s.f[%d]" % (obj, i)
        vs = [mc.pointPosition(v) for v in mc.ls(mc.polyListComponentConversion(f, tv=True), fl=True)]
        c = [sum(p[k] for p in vs) / len(vs) for k in range(3)]
        a, b = vs[0], vs[1]
        nrm = [0.0, 0.0, 0.0]
        for j in range(len(vs)):
            p, q = vs[j], vs[(j + 1) % len(vs)]
            nrm[0] += (p[1] - q[1]) * (p[2] + q[2])
            nrm[1] += (p[2] - q[2]) * (p[0] + q[0])
            nrm[2] += (p[0] - q[0]) * (p[1] + q[1])
        L = math.sqrt(sum(x * x for x in nrm)) or 1.0
        if test(c, [x / L for x in nrm]):
            out.append(f)
    return out


def lights_basic():
    k = mc.directionalLight(n="keyLight", rotation=(-45, 35, 0), intensity=1.0)
    f = mc.ambientLight(n="fillLight", intensity=0.25)
    return k, f


# ======================================================================================================= modelling
@scene("modelling", "poly_primitives")
def _():
    made = []
    for i, (cmd, kw) in enumerate([
        (mc.polyCube, dict(w=2, h=2, d=2, sx=2, sy=2, sz=2)),
        (mc.polySphere, dict(r=1.1, sx=20, sy=14)),
        (mc.polyCylinder, dict(r=1, h=2, sx=20, sc=1)),
        (mc.polyCone, dict(r=1, h=2, sx=20)),
        (mc.polyTorus, dict(r=1, sr=0.35, sx=24, sy=12)),
        (mc.polyPlane, dict(w=2, h=2, sx=4, sy=4)),
        (mc.polyPyramid, dict(w=2)),
        (mc.polyPrism, dict(w=1.2, l=2)),
        (mc.polyPipe, dict(r=1, h=2, t=0.3, sa=20)),
        (mc.polyHelix, dict(c=3, h=2.4, w=2, r=0.25)),
        (mc.polyPlatonicSolid, dict(r=1.1, st=3)),
        (getattr(mc, "polyDisc", None), dict()),
    ]):
        if cmd is None:
            continue
        try:
            o = cmd(**kw)[0]
        except Exception as e:
            LOG.append("  poly_primitives: %s %s" % (cmd.__name__ if hasattr(cmd, "__name__") else cmd, e))
            continue
        mc.move((i % 4) * 3.2 - 4.8, 1.2, (i // 4) * 3.2 - 3.2, o)
        made.append(o)
    m = mat("primitiveMat", (0.85, 0.5, 0.2), made)
    floor(18)
    lights_basic()
    look((10, 9, 14), (0, 0.8, 0))


@scene("modelling", "extrude_bevel_smooth")
def _():
    # A castle tower from one cylinder: the top inset and sunk, battlements extruded from every other rim face, windows
    # inset, the rim bevelled. Every step is a history node in the channel box's inputs.
    t = mc.polyCylinder(n="tower", r=2, h=5, sx=16, sy=5, sc=0)[0]
    mc.move(0, 2.5, 0, t)
    cap = faces_where(t, lambda c, n: c[1] > 4.99 and abs(n[1]) > 0.9)
    mc.polyExtrudeFacet(cap, off=0.45)
    mc.polyExtrudeFacet(cap, ltz=-0.8)
    rim = faces_where(t, lambda c, n: c[1] > 4.99 and n[1] > 0.9 and math.hypot(c[0], c[2]) > 1.5)
    rim.sort(key=lambda f: math.atan2(*face_centre(f)[::2]))
    mc.polyExtrudeFacet(rim[::2], ltz=0.6)
    windows = faces_where(t, lambda c, n: 2.5 < c[1] < 3.5 and abs(n[1]) < 0.2)
    mc.polyExtrudeFacet(windows[::2], ltz=-0.15, ls=(0.55, 0.7, 1))
    door = faces_where(t, lambda c, n: c[1] < 1 and n[2] > 0.9)
    if door:
        mc.polyExtrudeFacet(door[0], ltz=-0.3, ls=(0.7, 0.8, 1))
    mat("stoneMat", (0.62, 0.6, 0.55), t)
    floor(20)
    lights_basic()
    look((8, 7, 10), (0, 2.5, 0))

@scene("modelling", "booleans")
def _():
    out = []
    for i, (op, nm) in enumerate([(1, "union"), (2, "difference"), (3, "intersection")]):
        a = mc.polyCube(n=nm + "A", w=2, h=2, d=2)[0]
        s = mc.polySphere(n=nm + "B", r=1.3, sx=24, sy=16)[0]
        mc.move(0.6, 0.6, 0.6, s)
        r = mc.polyCBoolOp(a, s, op=op, n=nm)[0]
        mc.move((i - 1) * 4, 1.2, 0, r)
        out.append(r)
    mat("boolMat", (0.75, 0.75, 0.7), out)
    floor(20)
    lights_basic()
    look((0, 6, 12), (0, 1, 0))


@scene("modelling", "loops_bridge_merge")
def _():
    # Edge loops inserted into a tube; two boxes bridged across a gap; the corners of a plane welded.
    a = mc.polyCylinder(n="tube", r=1, h=3, sx=12, sy=1)[0]
    mc.polySplitRing(a + ".e[12:23]", wt=0.25)
    mc.polySplitRing(a + ".e[12:23]", wt=0.75)
    mc.move(-4, 1.5, 0, a)
    lo = mc.polyCube(n="lower", w=1.5, h=1, d=1.5)[0]
    up = mc.polyCube(n="upper", w=1.5, h=1, d=1.5)[0]
    mc.move(0, 0.5, 0, lo)
    mc.move(0.6, 3.5, 0, up)
    u = mc.polyUnite(lo, up, n="bridged")[0]
    mc.delete(faces_where(u, lambda c, n: abs(c[1] - 1) < 0.01 or abs(c[1] - 3) < 0.01))
    border = mc.ls(mc.polyListComponentConversion(u, te=True, bo=True), fl=True)
    mc.polyBridgeEdge(border, divisions=4)
    w = mc.polyPlane(n="welded", w=2, h=2, sx=4, sy=4)[0]
    mc.move(4, 0.01, 0, w)
    mc.polyMergeVertex(w + ".vtx[0]", w + ".vtx[1]", w + ".vtx[5]", w + ".vtx[6]", d=5)
    mat("loopMat", (0.55, 0.78, 0.4), [a, u, w])
    floor(20)
    lights_basic()
    look((7, 7, 11), (0, 1.5, 0))

@scene("modelling", "mirror_and_symmetry")
def _():
    # Half a spaceship (a wing and half a cockpit), mirrored live across X with polyMirrorFace and merged at the seam.
    h = mc.polyCube(n="ship", w=1.5, h=0.8, d=5, sx=1, sy=1, sz=3)[0]
    mc.move(0.75, 1.2, 0, h)
    side = faces_where(h, lambda c, n: n[0] > 0.9 and abs(c[2]) < 0.9)
    mc.polyExtrudeFacet(side, ltz=2.6, ls=(0.25, 0.9, 1), lty=0.3)
    top = faces_where(h, lambda c, n: n[1] > 0.9 and c[2] > 0.5 and c[0] < 1.6)
    if top:
        mc.polyExtrudeFacet(top[0], ltz=0.5, ls=(0.7, 0.7, 1))
    back = faces_where(h, lambda c, n: n[2] < -0.9 and c[0] < 1.6)
    if back:
        mc.polyExtrudeFacet(back[0], ltz=0.4, ls=(0.6, 0.5, 1))
    mc.polyMirrorFace(h, ws=True, d=1, mm=1, p=(0, 0, 0))
    mc.symmetricModelling(e=True, symmetry=True, about="world", axis="x")
    mat("shipMat", (0.8, 0.82, 0.88), h, "blinn")
    floor(16)
    lights_basic()
    look((6, 6, 8), (0, 1.2, 0))

@scene("modelling", "smooth_preview_creases")
def _():
    # The same cage three times, smooth mesh preview on: no creases, the top edges creased at 2, creased to the limit.
    out = []
    for i, c in enumerate([0, 2, 10]):
        o = mc.polyCube(n="cage%d" % i, w=2, h=2, d=2)[0]
        top = mc.ls(mc.polyListComponentConversion(faces_where(o, lambda cc, n: n[1] > 0.9), te=True), fl=True)
        if c:
            mc.polyCrease(top, v=c)
        mc.move((i - 1) * 3.5, 1, 0, o)
        shp = mc.listRelatives(o, s=True)[0]
        for a, v in (("displaySmoothMesh", 2), ("smoothPreview", 1)):
            if mc.attributeQuery(a, n=shp, ex=True):
                mc.setAttr(shp + "." + a, v)
        out.append(o)
    mat("cageMat", (0.9, 0.85, 0.75), out)
    floor(16)
    lights_basic()
    look((0, 5, 11), (0, 1, 0))

@scene("modelling", "duplicates_and_instances")
def _():
    src = mc.polyCone(n="tree", r=0.6, h=2, sx=8)[0]
    mc.move(0, 1, 0, src)
    mat("treeMat", (0.2, 0.55, 0.25), src)
    random.seed(4)
    for i in range(24):
        n = mc.instance(src)[0] if i % 2 else mc.duplicate(src)[0]
        mc.move(random.uniform(-8, 8), 1, random.uniform(-8, 8), n, a=True)
        s = random.uniform(0.6, 1.6)
        mc.scale(s, s, s, n)
    # Duplicate with transform: each copy steps up and turns a little.
    st = mc.polyCube(n="step", w=2, h=0.3, d=0.8)[0]
    mc.move(10, 0.15, 0, st)
    for i in range(12):
        mc.duplicate(st)
        mc.move(0, 0.3, 0, r=True)
        mc.rotate(0, 15, 0, r=True, os=True)
    floor(30)
    lights_basic()
    look((18, 12, 20), (3, 1, 0))


@scene("modelling", "nurbs_curves")
def _():
    cv = mc.curve(n="cvCurve", d=3, p=[(-6, 0, 0), (-5, 0, 2), (-3, 0, -1), (-2, 0, 2)])
    ep = mc.curve(n="epCurve", d=3, ep=[(-1, 0, 0), (0, 0, 2), (1, 0, 0), (2, 0, 2)])
    c = mc.circle(n="circle", r=1.2, nr=(0, 1, 0))[0]
    mc.move(5, 0, 1, c)
    arc = mc.circle(n="arc", r=1.2, nr=(0, 1, 0), sw=200)[0]
    mc.move(5, 0, -3, arc)
    try:
        mc.offsetCurve(cv, d=0.4, n="cvOffset")
    except Exception as e:
        LOG.append("  nurbs_curves: offset %s" % e)
    try:
        mc.rebuildCurve(ep, s=12, d=3, rpo=False, n="epRebuilt")
    except Exception as e:
        LOG.append("  nurbs_curves: rebuild %s" % e)
    a = mc.curve(n="attachA", d=1, p=[(-6, 0, -4), (-4, 0, -4)])
    b = mc.curve(n="attachB", d=1, p=[(-4, 0, -4), (-3, 0, -2)])
    try:
        mc.attachCurve(a, b, n="attached", rpo=False)
    except Exception as e:
        LOG.append("  nurbs_curves: attach %s" % e)
    look((0, 14, 9), (0, 0, 0))


@scene("modelling", "nurbs_surfaces")
def _():
    # A vase revolved from a profile, a lofted ribbon, a tube extruded along a path, a trimmed plane.
    prof = mc.curve(n="vaseProfile", d=3, p=[(0.2, 0, 0), (1.4, 0.3, 0), (1.6, 1.5, 0), (0.7, 2.6, 0), (0.9, 3.4, 0)])
    vase = mc.revolve(prof, ax=(0, 1, 0), n="vase")[0]
    mc.move(-5, 0, 0, vase)
    ribs = []
    for i in range(4):
        c = mc.circle(n="ribbon%d" % i, r=0.8 + 0.3 * math.sin(i), nr=(0, 1, 0))[0]
        mc.move(0, i * 1.0, 0, c)
        ribs.append(c)
    loft = mc.loft(*ribs, n="lofted")[0]
    path = mc.curve(n="tubePath", d=3, p=[(3, 0, 0), (4, 1, 1), (5, 2, -1), (6, 3, 0)])
    prof2 = mc.circle(n="tubeProfile", r=0.25, nr=(1, 0, 0))[0]
    mc.move(3, 0, 0, prof2)
    try:
        mc.extrude(prof2, path, et=2, fpt=True, n="tube")
    except Exception as e:
        LOG.append("  nurbs_surfaces: extrude %s" % e)
    sq = mc.nurbsSquare(n="trimOutline", c=(0, 0, 4), nr=(0, 1, 0), sl1=3)
    try:
        edges = mc.listRelatives(sq[0], c=True) if sq else []
        mc.planarSrf(*edges, n="trimmedPlane")
    except Exception as e:
        LOG.append("  nurbs_surfaces: planarSrf %s" % e)
    try:
        poly = mc.nurbsToPoly(loft, f=2, pt=1, n="loftAsPolygons")[0]
        mc.move(0, 0, -4, poly)
    except Exception as e:
        LOG.append("  nurbs_surfaces: nurbsToPoly %s" % e)
    mat("vaseMat", (0.2, 0.45, 0.75), vase)
    lights_basic()
    look((4, 8, 16), (0, 1.5, 0))


# ============================================================================================================== uvs
@scene("uvs", "uv_projections")
def _():
    objs = []
    for i, (kind, mk) in enumerate([("planar", lambda: mc.polyCube(n="planarBox", w=2, h=2, d=2)[0]),
                                    ("cylindrical", lambda: mc.polyCylinder(n="cylinderCan", r=1, h=2.4, sx=24)[0]),
                                    ("spherical", lambda: mc.polySphere(n="sphereBall", r=1.2, sx=24, sy=16)[0]),
                                    ("automatic", lambda: mc.polyTorus(n="autoTorus", r=1.1, sr=0.4)[0])]):
        o = mk()
        mc.move((i - 1.5) * 3.2, 1.3, 0, o)
        if kind == "planar":
            mc.polyPlanarProjection(o + ".f[*]", md="z")
        elif kind == "cylindrical":
            mc.polyCylindricalProjection(o + ".f[*]")
        elif kind == "spherical":
            mc.polySphericalProjection(o + ".f[*]")
        else:
            mc.polyAutoProjection(o + ".f[*]")
        objs.append(o)
    checker("checkerMat", objs, 4)
    floor(16)
    lights_basic()
    look((0, 5, 12), (0, 1.2, 0))


@scene("uvs", "unfold_and_layout")
def _():
    t = mc.polyTorus(n="ring", r=2, sr=0.7, sx=32, sy=16)[0]
    mc.move(0, 0.7, 0, t)
    mc.polyAutoProjection(t + ".f[*]")
    mc.u3dUnfold(t)
    mc.u3dLayout(t, res=1024)
    k = mc.polyCube(n="crate", w=2, h=2, d=2)[0]
    mc.move(4, 1, 0, k)
    mc.polyAutoProjection(k + ".f[*]")
    mc.u3dLayout(k, res=1024)
    checker("checkerMat", [t, k], 6)
    floor(16)
    lights_basic()
    look((2, 6, 10), (1, 0.8, 0))


# =========================================================================================================== rigging
@scene("rigging", "constraints")
def _():
    # One animated driver; each object follows it through a different constraint.
    drv = mc.spaceLocator(n="driver")[0]
    keys(drv, "tx", [(1, -4), (60, 4), (120, -4)])
    keys(drv, "ty", [(1, 2), (30, 4), (60, 2), (90, 4), (120, 2)])
    keys(drv, "rz", [(1, 0), (120, 360)])
    mc.playbackOptions(min=1, max=120)
    out = []
    for i, (kind, fn) in enumerate([("point", mc.pointConstraint), ("orient", mc.orientConstraint), ("parent", mc.parentConstraint),
                                    ("aim", mc.aimConstraint), ("scale", mc.scaleConstraint)]):
        o = mc.polyCube(n=kind + "Follower", w=0.8, h=0.8, d=0.8)[0]
        mc.move((i - 2) * 2.5, 0.4, -4, o)
        if kind == "parent":
            fn(drv, o, mo=True)
        elif kind == "aim":
            fn(drv, o, aim=(0, 1, 0), u=(0, 0, 1))
        elif kind == "scale":
            keys(drv, "sx", [(1, 1), (60, 2), (120, 1)])
            fn(drv, o)
        else:
            fn(drv, o, mo=kind != "point")
        out.append(o)
    # A ball rolling on a surface: geometry and normal constraints.
    g = mc.polyPlane(n="hill", w=10, h=4, sx=20, sy=4)[0]
    for v in range(mc.polyEvaluate(g, v=True)):
        p = mc.pointPosition("%s.vtx[%d]" % (g, v))
        mc.move(0, math.sin(p[0] * 0.8) * 0.6, 0, "%s.vtx[%d]" % (g, v), r=True)
    mc.move(0, 0, 4, g)
    rider = mc.polyCone(n="rider", r=0.3, h=0.8)[0]
    mc.pointConstraint(drv, rider, sk="y")
    mc.geometryConstraint(g, rider)
    mc.normalConstraint(g, rider, aim=(0, 1, 0), u=(1, 0, 0))
    mat("followerMat", (0.9, 0.6, 0.2), out + [rider])
    mat("hillMat", (0.3, 0.6, 0.3), g, "lambert")
    lights_basic()
    look((0, 9, 14), (0, 1, 0))


@scene("rigging", "ik_leg_reverse_foot")
def _():
    mc.select(cl=True)
    hip = mc.joint(n="hip", p=(0, 9, 0))
    knee = mc.joint(n="knee", p=(0, 5, 0.6))
    ankle = mc.joint(n="ankle", p=(0, 1, 0))
    ball = mc.joint(n="ball", p=(0, 0.2, 1.4))
    toe = mc.joint(n="toe", p=(0, 0.2, 2.6))
    mc.joint(hip, e=True, oj="xyz", sao="zup", ch=True, zso=True)
    leg = mc.ikHandle(sj=hip, ee=ankle, sol="ikRPsolver", n="legIK")[0]
    bik = mc.ikHandle(sj=ankle, ee=ball, sol="ikSCsolver", n="ballIK")[0]
    tik = mc.ikHandle(sj=ball, ee=toe, sol="ikSCsolver", n="toeIK")[0]
    ctrl = mc.circle(n="footCtrl", nr=(0, 1, 0), r=1.6)[0]
    mc.move(0, 0, 1, ctrl)
    mc.makeIdentity(ctrl, apply=True, t=True)
    mc.addAttr(ctrl, ln="roll", at="double", min=-40, max=90, k=True)
    heel = mc.group(em=True, n="heelPivot", p=ctrl)
    mc.move(0, 0, -0.4, heel, a=True)
    tip = mc.group(em=True, n="toePivot", p=heel)
    mc.move(0, 0, 2.6, tip, a=True)
    bp = mc.group(em=True, n="ballPivot", p=tip)
    mc.move(0, 0.2, 1.4, bp, a=True)
    mc.parent(leg, bik, bp)
    mc.parent(tik, tip)
    cl = mc.createNode("clamp", n="rollClamp")
    mc.connectAttr(ctrl + ".roll", cl + ".inputR"); mc.setAttr(cl + ".minR", -40)
    mc.connectAttr(ctrl + ".roll", cl + ".inputG"); mc.setAttr(cl + ".maxG", 35)
    mc.connectAttr(ctrl + ".roll", cl + ".inputB"); mc.setAttr(cl + ".minB", 35); mc.setAttr(cl + ".maxB", 90)
    pm = mc.createNode("plusMinusAverage", n="toeRollMinus"); mc.setAttr(pm + ".operation", 2)
    mc.connectAttr(cl + ".outputB", pm + ".input1D[0]"); mc.setAttr(pm + ".input1D[1]", 35)
    mc.connectAttr(cl + ".outputR", heel + ".rotateX")
    mc.connectAttr(cl + ".outputG", bp + ".rotateX")
    mc.connectAttr(pm + ".output1D", tip + ".rotateX")
    pv = mc.spaceLocator(n="kneePV")[0]
    mc.move(0, 5, 6, pv)
    mc.poleVectorConstraint(pv, leg)
    keys(ctrl, "roll", [(1, 0), (15, -30), (30, 0), (45, 35), (60, 80), (75, 0)])
    keys(ctrl, "tz", [(1, 0), (75, 6)])
    mc.playbackOptions(min=1, max=75)
    floor(20)
    look((14, 7, 8), (0, 4, 2))


@scene("rigging", "spline_ik_tail")
def _():
    # The curve's points follow locators (translate into controlPoints); the spline IK lays the joints along it.
    mc.select(cl=True)
    js = [mc.joint(n="tail%02d" % i, p=(0, 0.5, -i * 1.0)) for i in range(10)]
    pts = [(0, 0.5, -i * 1.8) for i in range(6)]
    crv = mc.curve(n="tailCurve", d=3, p=pts)
    shp = mc.listRelatives(crv, s=True)[0]
    for i, p in enumerate(pts):
        loc = mc.spaceLocator(n="tailPoint%d" % i)[0]
        mc.move(*p, loc)
        mc.connectAttr(loc + ".translate", "%s.controlPoints[%d]" % (shp, i))
        if i >= 2:
            keys(loc, "tx", [(1, 0), (30, i * 0.7), (60, -i * 0.7), (90, 0)], "spline")
            keys(loc, "ty", [(1, 0.5), (45, 0.5 + i * 0.3), (90, 0.5)], "spline")
    mc.ikHandle(sj=js[0], ee=js[-1], sol="ikSplineSolver", c=crv, ccv=False, pcv=False, n="tailIK")
    skin = mc.polyCylinder(n="tailMesh", r=0.4, h=9, sx=12, sy=27, ax=(0, 0, 1))[0]
    mc.move(0, 0.5, -4.5, skin)
    mc.skinCluster(js, skin, tsb=True, mi=3)
    mat("tailMat", (0.55, 0.3, 0.7), skin)
    mc.playbackOptions(min=1, max=90)
    floor(20)
    lights_basic()
    look((10, 8, 6), (0, 0.5, -4.5))


@scene("rigging", "nonlinear_deformers")
def _():
    out = []
    for i, kind in enumerate(["bend", "twist", "flare", "sine", "squash", "wave"]):
        o = mc.polyCylinder(n=kind + "Column", r=0.5, h=4, sx=16, sy=24)[0] if kind != "wave" else mc.polyPlane(n="wavePlane", w=4, h=4, sx=30, sy=30)[0]
        mc.move((i % 3 - 1) * 3.5, 2 if kind != "wave" else 0.5, (i // 3) * 4 - 2, o)
        d, handle = mc.nonLinear(o, type=kind, n=kind + "Deformer")
        attr = {"bend": "curvature", "twist": "endAngle", "flare": "endFlareX", "sine": "amplitude", "squash": "factor", "wave": "amplitude"}[kind]
        lo, hi = {"bend": (0, 120), "twist": (0, 270), "flare": (1, 2), "sine": (0, 0.3), "squash": (0, -0.5), "wave": (0, 0.2)}[kind]
        keys(d, attr, [(1, lo), (40, hi), (80, lo)], "spline")
        if kind in ("sine", "wave"):
            keys(d, "offset", [(1, 0), (80, 6)], "linear")
        out.append(o)
    mat("deformMat", (0.85, 0.45, 0.25), out)
    mc.playbackOptions(min=1, max=80)
    floor(20)
    lights_basic()
    look((0, 10, 16), (0, 1.5, 0))


@scene("rigging", "lattice_wire_wrap_cluster")
def _():
    s = mc.polySphere(n="latticeBall", r=1.5, sx=24, sy=16)[0]
    mc.move(-5, 1.5, 0, s)
    ffd, lat, base = mc.lattice(s, dv=(3, 3, 3), oc=True, n="ballLattice")
    mc.move(0, 0.8, 0, lat + ".pt[1][2][1]", r=True)
    mc.move(0.6, 0, 0, lat + ".pt[2][1][1]", r=True)
    p = mc.polyPlane(n="wirePlane", w=4, h=4, sx=30, sy=30)[0]
    mc.move(0, 0.01, 0, p)
    w = mc.curve(n="wireCurve", d=3, p=[(-2, 0, 0), (-0.7, 0, 0.3), (0.7, 0, -0.3), (2, 0, 0)])
    mc.wire(p, w=w, n="planeWire", dds=(0, 1.2))
    keys(w, "ty", [(1, 0), (40, 1.2), (80, 0)], "spline")
    c = mc.polyCube(n="clusterBox", w=2, h=2, d=2, sx=4, sy=4, sz=4)[0]
    mc.move(5, 1, 0, c)
    cl, ch = mc.cluster(c + ".vtx[0:24]", n="boxCluster")
    keys(ch, "ty", [(1, 0), (40, -0.6), (80, 0)], "spline")
    hi = mc.polySphere(n="wrapped", r=1, sx=40, sy=30)[0]
    lo = mc.polyCube(n="wrapDriver", w=1.6, h=1.6, d=1.6)[0]
    for o in (hi, lo):
        mc.move(0, 1.2, 4, o)
    mc.select(hi, lo)
    try:
        mel.eval("CreateWrap")
    except Exception:
        try:
            mc.deformer(hi, type="wrap", n="sphereWrap")
        except Exception as e:
            LOG.append("  lattice_wire_wrap_cluster: wrap %s" % e)
    keys(lo, "sx", [(1, 1), (40, 1.6), (80, 1)], "spline")
    mc.setAttr(lo + ".overrideEnabled", 1)
    mc.setAttr(lo + ".overrideShading", 0)
    sm = mc.polyPlane(n="softModPlane", w=4, h=4, sx=20, sy=20)[0]
    mc.move(0, 0, -5, sm)
    try:
        smd, smh = mc.softMod(sm + ".vtx[220]", fas=True, n="planeSoftMod")
        mc.setAttr(smd + ".falloffRadius", 1.5)
        keys(smh, "ty", [(1, 0), (40, 1.2), (80, 0)], "spline")
    except Exception as e:
        LOG.append("  lattice_wire_wrap_cluster: softMod %s" % e)
    mat("defMat", (0.35, 0.65, 0.85), [s, p, c, hi, sm])
    mc.playbackOptions(min=1, max=80)
    lights_basic()
    look((10, 11, 15), (0, 1, 0))


@scene("rigging", "skinning_linear_vs_dual_quaternion")
def _():
    # Two arms twisted the same amount: linear skinning collapses at the wrist, dual quaternion keeps the volume.
    out = []
    for i, method in enumerate([0, 1]):
        mc.select(cl=True)
        j0 = mc.joint(n="shoulder%d" % i, p=(-3, 2, (i - 0.5) * 4))
        j1 = mc.joint(n="elbow%d" % i, p=(0, 2, (i - 0.5) * 4))
        j2 = mc.joint(n="wrist%d" % i, p=(3, 2, (i - 0.5) * 4))
        mc.joint(j0, e=True, oj="xyz", sao="yup", ch=True, zso=True)
        m = mc.polyCylinder(n=("linearArm", "dualQuatArm")[i], r=0.6, h=6, sx=16, sy=30, ax=(1, 0, 0))[0]
        mc.move(0, 2, (i - 0.5) * 4, m)
        sc = mc.skinCluster(j0, j1, j2, m, tsb=True, mi=2, n=("linearSkin", "dualQuatSkin")[i])[0]
        mc.setAttr(sc + ".skinningMethod", method)
        keys(j1, "rx", [(1, 0), (40, 150), (80, 0)], "spline")
        keys(j1, "rz", [(1, 0), (40, 60), (80, 0)], "spline")
        out.append(m)
    mat("armMat", (0.85, 0.65, 0.5), out)
    mc.playbackOptions(min=1, max=80)
    floor(20)
    lights_basic()
    look((2, 8, 13), (0, 2, 0))


@scene("rigging", "blend_shapes")
def _():
    base = mc.polySphere(n="blob", r=1.5, sx=32, sy=24)[0]
    mc.move(0, 1.6, 0, base)
    targets = []
    for i, (nm, fn) in enumerate([("squash", lambda t: mc.scale(1.4, 0.6, 1.4, t)),
                                   ("stretch", lambda t: mc.scale(0.7, 1.6, 0.7, t)),
                                   ("lean", lambda t: mc.polyMoveVertex(t + ".vtx[*]", tx=0.0) or mc.nonLinear(t, type="bend", curvature=60))]):
        t = mc.duplicate(base, n=nm)[0]
        mc.move((i - 1) * 4, 1.6, -5, t)
        fn(t)
        mc.delete(t, ch=True)
        targets.append(t)
    bs = mc.blendShape(targets, base, n="blobShapes")[0]
    # An in-between: halfway to stretch, the blob first pinches.
    ib = mc.duplicate(base, n="stretchHalf")[0]
    mc.scale(0.6, 1.2, 0.6, ib)
    mc.delete(ib, ch=True)
    try:
        mc.blendShape(bs, e=True, ib=True, t=(base, 1, ib, 0.5))
    except Exception as e:
        LOG.append("  blend_shapes: inbetween %s" % e)
    mc.move(4, 1.6, -5, ib)
    keys(bs, "squash", [(1, 0), (20, 1), (40, 0)], "spline")
    keys(bs, "stretch", [(30, 0), (50, 1), (70, 0)], "spline")
    keys(bs, "lean", [(60, 0), (80, 1), (100, 0)], "spline")
    mat("blobMat", (0.4, 0.75, 0.55), [base] + targets + [ib])
    mc.playbackOptions(min=1, max=100)
    floor(20)
    lights_basic()
    look((0, 6, 12), (0, 1.5, -2))


@scene("rigging", "set_driven_keys")
def _():
    # A drawer and a lid driven by one "open" attribute on a control.
    cab = mc.polyCube(n="cabinet", w=3, h=2, d=2)[0]
    mc.move(0, 1, 0, cab)
    drawer = mc.polyCube(n="drawer", w=2.6, h=0.7, d=1.9)[0]
    mc.move(0, 1.4, 0.06, drawer)
    lid = mc.polyCube(n="lid", w=3, h=0.15, d=2)[0]
    mc.move(0, 2.08, 0, lid)
    mc.xform(lid, ws=True, piv=(0, 2.08, -1))
    ctrl = mc.circle(n="cabinetCtrl", nr=(0, 1, 0), r=2.2)[0]
    mc.addAttr(ctrl, ln="open", at="double", min=0, max=10, k=True)
    for dv, tz, rx in ((0, 0.06, 0), (5, 1.5, 0), (10, 1.5, -100)):
        mc.setAttr(ctrl + ".open", dv)
        mc.setAttr(drawer + ".tz", tz)
        mc.setAttr(lid + ".rx", rx)
        mc.setDrivenKeyframe(drawer + ".tz", cd=ctrl + ".open")
        mc.setDrivenKeyframe(lid + ".rx", cd=ctrl + ".open")
    mc.setAttr(ctrl + ".open", 0)
    keys(ctrl, "open", [(1, 0), (40, 10), (70, 10), (100, 0)], "spline")
    mat("woodMat", (0.55, 0.35, 0.18), [cab, drawer, lid])
    mc.playbackOptions(min=1, max=100)
    floor(12)
    lights_basic()
    look((6, 5, 8), (0, 1.2, 0))


@scene("rigging", "delta_mush_and_jiggle")
def _():
    mc.select(cl=True)
    j0 = mc.joint(n="baseJ", p=(0, 0, 0))
    j1 = mc.joint(n="midJ", p=(0, 2, 0))
    j2 = mc.joint(n="topJ", p=(0, 4, 0))
    m = mc.polyCylinder(n="mushed", r=0.7, h=4, sx=16, sy=24)[0]
    mc.move(0, 2, 0, m)
    mc.skinCluster(j0, j1, j2, m, tsb=True, mi=1)  # one influence: hard creases, smoothed by delta mush
    mc.deltaMush(m, n="smoothOut", si=10)
    keys(j1, "rz", [(1, 0), (30, 70), (60, -70), (90, 0)], "spline")
    b = mc.polySphere(n="jelly", r=1, sx=24, sy=16)[0]
    mc.move(4, 1, 0, b)
    keys(b, "tx", [(1, 4), (20, 7), (40, 4)], "linear")
    try:
        mc.deformer(b, type="jiggle", n="jellyJiggle")
    except Exception as e:
        LOG.append("  delta_mush_and_jiggle: jiggle %s" % e)
    mat("mushMat", (0.85, 0.35, 0.45), [m, b])
    mc.playbackOptions(min=1, max=90)
    floor(20)
    lights_basic()
    look((4, 6, 13), (2, 2, 0))


# ========================================================================================================= animation
@scene("animation", "bouncing_ball")
def _():
    b = mc.polySphere(n="ball", r=1, sx=24, sy=16)[0]
    mc.move(0, 1, 0, b)
    mc.xform(b, piv=(0, 0, 0), os=True)
    t = 1
    h = 8.0
    x = -10
    while h > 0.3:
        dur = int(6 + h * 1.6)
        keys(b, "ty", [(t, 1)])
        keys(b, "sy", [(t, 0.75), (t + 2, 1.1)])
        keys(b, "sx", [(t, 1.2), (t + 2, 0.92)])
        keys(b, "ty", [(t + dur // 2, 1 + h)])
        keys(b, "sy", [(t + dur // 2, 1.0)])
        keys(b, "sx", [(t + dur // 2, 1.0)])
        x += h * 0.7
        t += dur
        h *= 0.6
    keys(b, "ty", [(t, 1)])
    keys(b, "tx", [(1, -10), (t, x)], "linear")
    mc.keyTangent(b, at="ty", itt="spline", ott="spline")
    mc.playbackOptions(min=1, max=t + 10)
    mat("ballMat", (0.9, 0.2, 0.15), b)
    floor(40)
    lights_basic()
    look((0, 6, 26), (0, 3, 0))


@scene("animation", "motion_path")
def _():
    path = mc.curve(n="flightPath", d=3, p=[(-8, 1, 0), (-5, 4, 6), (0, 2, 8), (5, 5, 4), (8, 1, -2), (2, 3, -8), (-6, 2, -5), (-8, 1, 0)])
    plane = mc.polyCone(n="dart", r=0.5, h=2, sx=3)[0]
    mc.rotate(90, 0, 0, plane)
    mc.makeIdentity(plane, apply=True, r=True)
    mc.pathAnimation(plane, c=path, follow=True, fa="z", ua="y", bank=True, stu=1, etu=150, n="dartPath")
    mc.playbackOptions(min=1, max=150)
    mat("dartMat", (0.95, 0.75, 0.1), plane)
    floor(30)
    lights_basic()
    look((0, 18, 20), (0, 2, 0))


@scene("animation", "animation_layers")
def _():
    c = mc.polyCube(n="hopper", w=1.5, h=1.5, d=1.5)[0]
    keys(c, "tx", [(1, -6), (100, 6)], "linear")
    keys(c, "ty", [(1, 0.75)])
    mc.select(c)
    layer = mc.animLayer("hops", addSelectedObjects=True)
    for t in range(1, 101, 12):
        mc.setKeyframe(c, at="ty", t=t, v=0.0, animLayer=layer)
        mc.setKeyframe(c, at="ty", t=t + 6, v=2.0, animLayer=layer)
    layer2 = mc.animLayer("spin", addSelectedObjects=True)
    mc.setKeyframe(c, at="ry", t=1, v=0, animLayer=layer2)
    mc.setKeyframe(c, at="ry", t=100, v=720, animLayer=layer2)
    mc.setAttr(layer2 + ".weight", 0.5)
    mc.playbackOptions(min=1, max=100)
    mat("hopperMat", (0.3, 0.6, 0.9), c)
    floor(20)
    lights_basic()
    look((0, 6, 16), (0, 1.5, 0))


@scene("animation", "expressions")
def _():
    out = []
    for i in range(12):
        c = mc.polyCube(n="key%02d" % i, w=0.7, h=0.7, d=0.7)[0]
        mc.move((i - 5.5) * 1.0, 0.35, 0, c)
        mc.expression(n="wave%02d" % i, s="%s.ty = 0.35 + abs(sin(time * 3 + %d * 0.5)) * 2;\n%s.ry = time * 90;" % (c, i, c))
        out.append(c)
    s = mc.polySphere(n="orbiter", r=0.4)[0]
    mc.expression(n="orbit", s="orbiter.tx = cos(frame * 0.05) * 6;\norbiter.tz = sin(frame * 0.05) * 6;\norbiter.ty = 3 + noise(frame * 0.05);")
    mat("exprMat", (0.9, 0.85, 0.2), out + [s])
    mc.playbackOptions(min=1, max=200)
    floor(20)
    lights_basic()
    look((0, 9, 15), (0, 1.5, 0))


@scene("animation", "pendulums")
def _():
    # Five pendulums of different lengths, keyed as cycles with infinity set to cycle.
    out = []
    for i in range(5):
        piv = mc.group(em=True, n="pivot%d" % i)
        mc.move((i - 2) * 1.6, 8, 0, piv)
        L = 3 + i * 0.8
        bob = mc.polySphere(n="bob%d" % i, r=0.4)[0]
        rod = mc.polyCylinder(n="rod%d" % i, r=0.04, h=L)[0]
        mc.move(0, -L / 2, 0, rod)
        mc.move(0, -L, 0, bob)
        mc.parent(bob, rod, piv, r=True)
        period = int(24 * 2 * math.pi * math.sqrt(L / 980.0 * 100) / 2)
        keys(piv, "rz", [(1, 30), (1 + period, -30), (1 + 2 * period, 30)], "spline")
        mc.setInfinity(piv, at="rz", pri="cycle", poi="cycle")
        out.append(bob)
    mat("bobMat", (0.8, 0.8, 0.85), out)
    mc.playbackOptions(min=1, max=240)
    lights_basic()
    look((0, 5, 18), (0, 4, 0))


# ========================================================================================================== dynamics
@scene("dynamics", "cloth_flag")
def _():
    # A 6 by 4 metre flag nailed along a pole, in a 5 m/s breeze.
    pole = mc.polyCylinder(n="pole", r=8, h=1000, sx=12)[0]
    mc.move(0, 500, 0, pole)
    flag = mc.polyPlane(n="flag", w=600, h=400, sx=36, sy=24, ax=(0, 0, 1))[0]
    mc.move(308, 780, 0, flag)
    mc.createCloth(flag, n="flagCloth", preset="cotton")
    edge = [v for v in mc.ls(flag + ".vtx[*]", fl=True) if mc.pointPosition(v)[0] < 9]
    mc.dynamicConstraint(*edge, type="nail")
    sol = mc.ls(type="solver")[0]
    mc.setAttr(sol + ".windSpeed", 5)
    mc.setAttr("flagCloth.stretchResistance", 80)
    mc.setAttr("flagCloth.bendResistance", 0.5)
    mc.setAttr("flagCloth.drag", 0.08)
    mc.setAttr("flagCloth.tangentialDrag", 0.01)
    mc.setAttr(sol + ".windDirection", 1, 0, 0.35)
    mat("flagMat", (0.85, 0.12, 0.12), flag, "lambert")
    mat("poleMat", (0.8, 0.8, 0.8), pole)
    mc.playbackOptions(min=1, max=200)
    floor(3000)
    lights_basic()
    look((1100, 700, 1900), (300, 600, 0))

@scene("dynamics", "cloth_on_moving_collider")
def _():
    # A 3 metre silk sheet dropped on a ball that rises under it and rolls away.
    sheet = mc.polyPlane(n="tablecloth", w=300, h=300, sx=40, sy=40)[0]
    mc.move(0, 260, 0, sheet)
    mc.createCloth(sheet, n="tableclothCloth", preset="silk")
    ball = mc.polySphere(n="pusher", r=60, sx=24, sy=16)[0]
    keys(ball, "ty", [(1, 60), (40, 140), (140, 140)], "spline")
    keys(ball, "tx", [(60, 0), (140, 180)], "spline")
    mc.collider(ball, n="pusherCollider")
    fl = floor(2000)
    mc.collider(fl, n="floorCollider")
    mat("silkMat", (0.6, 0.2, 0.7), sheet, "lambert")
    mat("pusherMat", (0.9, 0.8, 0.2), ball)
    mc.playbackOptions(min=1, max=160)
    lights_basic()
    look((450, 380, 550), (40, 100, 0))

@scene("dynamics", "particles_bouncing")
def _():
    # Particles poured on a tilted board, bouncing off it and the floor.
    ramp = mc.polyPlane(n="ramp", w=800, h=400)[0]
    mc.rotate(0, 0, -20, ramp)
    mc.move(0, 300, 0, ramp)
    mc.collider(ramp)
    fl = floor(3000)
    mc.collider(fl)
    e = mc.emitter(type="omni", r=200, spd=150, n="fountain")
    em, parts = e[0], e[1]
    mc.move(-250, 800, 0, em)
    for a, v in (("radius", 9), ("bounce", 0.6), ("friction", 0.1), ("lifespanMode", 1), ("lifespan", 5), ("particleRenderType", 4)):
        mc.setAttr(parts + "." + a, v)
    mat("rampMat", (0.3, 0.5, 0.8), ramp, "lambert")
    mc.playbackOptions(min=1, max=150)
    lights_basic()
    look((150, 450, 900), (0, 250, 0))

@scene("dynamics", "particles_vortex")
def _():
    # Particles lifted by a uniform field and spun by a vortex into a column.
    e = mc.emitter(type="omni", r=400, spd=60, n="swirlSource")
    parts = e[1]
    mc.select(parts)
    mc.vortex(m=400, n="swirl")
    mc.select(parts)
    mc.uniform(m=1400, dx=0, dy=1, dz=0, n="lift")
    for a, v in (("radius", 7), ("lifespanMode", 1), ("lifespan", 4), ("particleRenderType", 4)):
        mc.setAttr(parts + "." + a, v)
    mc.playbackOptions(min=1, max=200)
    floor(3000)
    lights_basic()
    look((0, 350, 800), (0, 300, 0))

@scene("dynamics", "rigid_tower")
def _():
    # A tower of blocks and a heavy ball sent into it.
    fl = floor(4000)
    mc.rigidBody(fl, passive=True)
    random.seed(2)
    for row in range(8):
        for k in range(3):
            b = mc.polyCube(n="block_%d_%d" % (row, k), w=150 if row % 2 == 0 else 48, h=45, d=48 if row % 2 == 0 else 150)[0]
            if row % 2 == 0:
                mc.move(0, 22.5 + row * 45, (k - 1) * 51, b)
            else:
                mc.move((k - 1) * 51, 22.5 + row * 45, 0, b)
            mc.rigidBody(b, active=True, m=40, b=0.1, sf=0.6, df=0.5)
            mat("blockMat%d%d" % (row, k), (0.75 + random.uniform(-0.1, 0.1), 0.4, 0.25), b, "lambert")
    ball = mc.polySphere(n="wreckingBall", r=50)[0]
    mc.move(-700, 120, 0, ball)
    mc.rigidBody(ball, active=True, m=800, iv=(1600, 0, 0))
    mat("ballMat", (0.15, 0.15, 0.18), ball)
    mc.playbackOptions(min=1, max=150)
    lights_basic()
    look((800, 500, 900), (-50, 150, 0))

# ========================================================================================================= rendering
def shot(eye, at, fl=40, w=960, h=540):
    cam = mc.camera(n="shotCam", focalLength=fl)[0]
    mc.viewPlace(cam, eye=eye, la=at)
    mc.setAttr(cam + "Shape.renderable", 1)
    mc.setAttr("perspShape.renderable", 0)
    mc.setAttr("defaultResolution.width", w)
    mc.setAttr("defaultResolution.height", h)
    mc.setAttr("defaultArnoldRenderOptions.AASamples", 4)
    mc.setAttr("defaultRenderGlobals.imageFilePrefix", "<Scene>", type="string")
    return cam


def sky(intensity=1.0):
    t = mc.createNode("transform", n="sky")
    s = mc.createNode("aiSkyDomeLight", n="skyShape", p=t)
    f = mc.shadingNode("file", asTexture=True, n="skyImage")
    mc.setAttr(f + ".fileTextureName", "../textures/sky.hdr", type="string")
    mc.setAttr(f + ".colorSpace", "scene-linear Rec.709-sRGB", type="string")
    mc.connectAttr(f + ".outColor", s + ".color")
    mc.setAttr(s + ".intensity", intensity)
    return s


@scene("rendering", "standard_surface_lobes")
def _():
    # One sphere per lobe of the standard surface.
    looks = [
        ("subsurface", dict(baseColor=(0.9, 0.6, 0.5), subsurface=1.0, subsurfaceColor=(0.95, 0.5, 0.4), subsurfaceScale=0.3)),
        ("coat", dict(baseColor=(0.6, 0.05, 0.05), coat=1.0, coatRoughness=0.02, specularRoughness=0.5)),
        ("sheen", dict(baseColor=(0.15, 0.2, 0.5), sheen=1.0, sheenColor=(0.8, 0.85, 1.0), sheenRoughness=0.4, specular=0.2)),
        ("thinFilm", dict(baseColor=(0.05, 0.05, 0.05), metalness=0.3, thinFilmThickness=420, specularRoughness=0.1)),
        ("brushedMetal", dict(baseColor=(0.9, 0.9, 0.92), metalness=1.0, specularRoughness=0.35, specularAnisotropy=0.8)),
        ("glow", dict(baseColor=(0.1, 0.1, 0.1), emission=2.0, emissionColor=(1.0, 0.5, 0.1))),
    ]
    for i, (nm, attrs) in enumerate(looks):
        s = mc.polySphere(n=nm + "Ball", r=1, sx=48, sy=32)[0]
        mc.move((i - 2.5) * 2.4, 1, 0, s)
        m = mc.shadingNode("standardSurface", asShader=True, n=nm + "Mat")
        for a, v in attrs.items():
            if isinstance(v, tuple):
                mc.setAttr(m + "." + a, *v, type="double3")
            else:
                mc.setAttr(m + "." + a, v)
        sg = mc.sets(renderable=True, noSurfaceShader=True, empty=True, n=nm + "SG")
        mc.connectAttr(m + ".outColor", sg + ".surfaceShader")
        mc.sets(s, e=True, forceElement=sg)
    fl = floor(60, color=(0.4, 0.4, 0.38))
    sky(1.0)
    shot((0, 3.5, 14), (0, 1, 0), 35)
    look((0, 3.5, 14), (0, 1, 0))


@scene("rendering", "depth_of_field")
def _():
    out = []
    for i in range(9):
        c = mc.polyCube(n="post%d" % i, w=0.6, h=2, d=0.6)[0]
        mc.move(-2 + i * 0.5, 1, 4 - i * 2.2, c)
        out.append(c)
    mat("postMat", (0.85, 0.75, 0.3), out, "standardSurface", specularRoughness=0.4)
    floor(60, color=(0.3, 0.32, 0.35))
    sky(0.8)
    cam = shot((2.5, 1.6, 9), (-0.5, 1, -2), 85)
    mc.setAttr(cam + "Shape.depthOfField", 1)
    mc.setAttr(cam + "Shape.focusDistance", 11.5)
    mc.setAttr(cam + "Shape.fStop", 1.4)
    look((6, 4, 12), (0, 1, -4))


@scene("rendering", "motion_blur")
def _():
    fan = mc.polyCube(n="blade", w=6, h=0.1, d=0.6)[0]
    mc.move(0, 2, 0, fan)
    keys(fan, "ry", [(1, 0), (24, 720)], "linear")
    mc.setInfinity(fan, at="ry", poi="linear")
    hub = mc.polyCylinder(n="hub", r=0.4, h=2, sx=16)[0]
    mc.move(0, 1, 0, hub)
    ball = mc.polySphere(n="flyer", r=0.5)[0]
    keys(ball, "tx", [(1, -6), (24, 6)], "linear")
    mc.move(0, 1, 3, ball)
    mat("bladeMat", (0.9, 0.9, 0.92), [fan, hub], "standardSurface", metalness=1.0, specularRoughness=0.2)
    mat("flyerMat", (0.9, 0.2, 0.1), ball, "standardSurface")
    floor(40, color=(0.35, 0.35, 0.35))
    sky(1.0)
    shot((0, 5, 12), (0, 1.5, 0), 35)
    for a, v in (("motionBlur", 1),):
        try:
            mc.setAttr("defaultRenderGlobals." + a, v)
        except Exception as e:
            LOG.append("  motion_blur: %s %s" % (a, e))
    for a, v in (("motion_blur_enable", 1),):
        try:
            mc.setAttr("defaultArnoldRenderOptions." + a, v)
        except Exception as e:
            LOG.append("  motion_blur: %s %s" % (a, e))
    mc.playbackOptions(min=1, max=24)
    mc.currentTime(12)
    look((0, 5, 12), (0, 1.5, 0))


@scene("rendering", "displacement_and_subdivision")
def _():
    p = mc.polyPlane(n="terrain", w=10, h=10, sx=20, sy=20)[0]
    shp = mc.listRelatives(p, s=True)[0]
    for a, v in (("aiSubdivType", 1), ("aiSubdivIterations", 3), ("aiDispHeight", 1.2)):
        try:
            mc.setAttr(shp + "." + a, v)
        except Exception as e:
            LOG.append("  displacement: %s %s" % (a, e))
    m = mat("terrainMat", (0.45, 0.5, 0.35), p, "standardSurface", specularRoughness=0.8)
    n = mc.shadingNode("noise", asTexture=True, n="terrainNoise")
    mc.setAttr(n + ".frequencyRatio", 2.2)
    mc.setAttr(n + ".depthMax", 6)
    d = mc.shadingNode("displacementShader", asShader=True, n="terrainDisplace")
    mc.connectAttr(n + ".outAlpha", d + ".displacement")
    sg = mc.listConnections(m + ".outColor", type="shadingEngine")[0]
    mc.connectAttr(d + ".displacement", sg + ".displacementShader")
    s = mc.polySphere(n="smoothed", r=1, sx=8, sy=6)[0]
    mc.move(0, 2.5, 0, s)
    for a, v in (("aiSubdivType", 1), ("aiSubdivIterations", 3)):
        try:
            mc.setAttr(mc.listRelatives(s, s=True)[0] + "." + a, v)
        except Exception as e:
            LOG.append("  displacement: %s %s" % (a, e))
    mat("smoothMat", (0.85, 0.85, 0.9), s, "standardSurface", metalness=1.0, specularRoughness=0.15)
    sky(1.0)
    shot((7, 6, 9), (0, 0.5, 0), 35)
    look((7, 6, 9), (0, 0.5, 0))


@scene("rendering", "light_types")
def _():
    floor(30, color=(0.6, 0.6, 0.6))
    wall = mc.polyPlane(n="backWall", w=30, h=12, ax=(0, 0, 1))[0]
    mc.move(0, 6, -5, wall)
    mat("wallMat", (0.6, 0.6, 0.6), wall, "lambert")
    out = []
    for i in range(5):
        s = mc.polySphere(n="target%d" % i, r=0.8)[0]
        mc.move((i - 2) * 4, 0.8, 0, s)
        out.append(s)
    mat("targetMat", (0.85, 0.85, 0.85), out, "standardSurface", specularRoughness=0.3)
    sp = mc.spotLight(n="spot", coneAngle=35, penumbra=8, intensity=1.2, rgb=(1, 0.8, 0.5))
    mc.move(-8, 6, 3, mc.listRelatives(sp, p=True)[0])
    mc.rotate(-45, -45, 0, mc.listRelatives(sp, p=True)[0])
    pt = mc.pointLight(n="bulb", intensity=0.8, rgb=(0.4, 0.6, 1.0))
    mc.move(4, 1.8, 2, mc.listRelatives(pt, p=True)[0])
    ar = mc.shadingNode("areaLight", asLight=True, n="softbox")
    mc.move(0, 4, 3, ar)
    mc.rotate(-60, 0, 0, ar)
    mc.setAttr(ar + ".intensity", 0.8)
    mc.directionalLight(n="sun", rotation=(-40, 50, 0), intensity=0.25, rgb=(1, 0.95, 0.85))
    mc.ambientLight(n="ambient", intensity=0.04)
    shot((0, 6, 16), (0, 1.5, 0), 35)
    look((0, 6, 16), (0, 1.5, 0))

# ========================================================================================================= pipeline
@scene("pipeline", "display_layers_and_overrides")
def _():
    groups = {}
    for name, col, state in (("props", 13, 0), ("set", 6, 2), ("guides", 17, 1)):
        objs = []
        for i in range(4):
            o = mc.polyCube(n="%s%d" % (name, i), w=1, h=1 + i * 0.4, d=1)[0]
            mc.move((i - 1.5) * 1.6, 0.5, {"props": 0, "set": -3, "guides": 3}[name], o)
            objs.append(o)
        mc.select(objs)
        layer = mc.createDisplayLayer(n=name + "Layer", e=False)
        mc.setAttr(layer + ".color", col)
        mc.setAttr(layer + ".displayType", state)  # 0 normal, 1 template, 2 reference
        groups[name] = objs
    ctrl = mc.circle(n="coloredCtrl", nr=(0, 1, 0), r=1.2)[0]
    mc.move(5, 0, 0, ctrl)
    sh = mc.listRelatives(ctrl, s=True)[0]
    mc.setAttr(sh + ".overrideEnabled", 1)
    mc.setAttr(sh + ".overrideRGBColors", 1)
    mc.setAttr(sh + ".overrideColorRGB", 1, 0.4, 0.8)
    floor(16)
    lights_basic()
    look((6, 8, 12), (0, 0.5, 0))


@scene("pipeline", "namespaces_and_sets")
def _():
    for ns in ("chairA", "chairB"):
        mc.namespace(add=ns)
        mc.namespace(set=ns)
        seat = mc.polyCube(n="seat", w=2, h=0.3, d=2)[0]
        back = mc.polyCube(n="back", w=2, h=2, d=0.3)[0]
        mc.move(0, 1, 0, seat)
        mc.move(0, 2, -0.85, back)
        legs = []
        for k, (x, z) in enumerate(((-0.8, -0.8), (0.8, -0.8), (-0.8, 0.8), (0.8, 0.8))):
            l = mc.polyCube(n="leg%d" % k, w=0.2, h=1, d=0.2)[0]
            mc.move(x, 0.5, z, l)
            legs.append(l)
        g = mc.group(seat, back, *legs, n="chair")
        mc.namespace(set=":")
        mc.move(-2 if ns == "chairA" else 2, 0, 0, g)
    mc.sets(mc.ls("*:leg*", type="transform"), n="allLegs")
    mc.sets(mc.ls("*:seat", "*:back"), n="seatsAndBacks")
    mat("chairMat", (0.6, 0.4, 0.25), mc.ls("*:seat", "*:back", "*:leg*", type="transform"))
    floor(12)
    lights_basic()
    look((6, 6, 9), (0, 1, 0))


@scene("pipeline", "usd_round_trip")
def _():
    # A small set exported to USD and opened again as a stage in the scene, next to the original.
    out = []
    for i in range(5):
        o = mc.polyCylinder(n="barrel%d" % i, r=0.6, h=1.4, sx=16)[0]
        mc.move((i - 2) * 1.6, 0.7, -3, o)
        out.append(o)
    mat("barrelMat", (0.5, 0.3, 0.15), out)
    g = mc.group(out, n="barrels")
    usd = os.path.join(ROOT, "pipeline", "barrels.usda")
    os.makedirs(os.path.dirname(usd), exist_ok=True)
    mc.select(g)
    mel.eval('mayaUSDExport -file "%s" -selection' % usd.replace("\\", "/"))
    mel.eval('mayaUsd_createStageFromFile "%s"' % usd.replace("\\", "/"))
    stages = mc.ls(type="mayaUsdProxyShape")
    for st in stages:
        mc.setAttr(st + ".filePath", "barrels.usda", type="string")  # beside the scene, wherever it is
    if stages:
        mc.move(0, 0, 3, mc.listRelatives(stages[0], p=True)[0])
    floor(14)
    lights_basic()
    look((6, 6, 10), (0, 0.7, 0))


# ===================================================================================================== characters
@scene("characters", "keyed_robot")
def _():
    # A blocky robot: a hierarchy of FK controls, a walk cycle on infinite repeat.
    def box(n, w, h, d, p, parent=None):
        o = mc.polyCube(n=n, w=w, h=h, d=d)[0]
        mc.move(*p, o)
        if parent:
            mc.parent(o, parent)
        return o
    body = box("torso", 2, 2.4, 1.2, (0, 4.6, 0))
    head = box("head", 1.2, 1.0, 1.0, (0, 6.4, 0), body)
    mc.xform(head, ws=True, piv=(0, 5.8, 0))
    parts = [body, head]
    for side, x in (("L", 1.3), ("R", -1.3)):
        arm = box(side + "_arm", 0.5, 2.2, 0.5, (x, 4.6, 0), body)
        mc.xform(arm, ws=True, piv=(x, 5.7, 0))
        leg = box(side + "_leg", 0.7, 3, 0.7, (x * 0.4, 1.8, 0), body)
        mc.xform(leg, ws=True, piv=(x * 0.4, 3.3, 0))
        ph = 0 if side == "L" else 12
        keys(arm, "rx", [(1 + ph, 30), (13 + ph, -30), (25 + ph, 30)], "spline")
        keys(leg, "rx", [(1 + ph, -25), (13 + ph, 25), (25 + ph, -25)], "spline")
        for a in ("rx",):
            mc.setInfinity(arm, at=a, pri="cycle", poi="cycle")
            mc.setInfinity(leg, at=a, pri="cycle", poi="cycle")
        parts += [arm, leg]
    keys(body, "ty", [(1, 4.6), (7, 4.8), (13, 4.6), (19, 4.8), (25, 4.6)], "spline")
    mc.setInfinity(body, at="ty", pri="cycle", poi="cycle")
    keys(body, "tz", [(1, 0), (25, 3)], "linear")
    mc.setInfinity(body, at="tz", pri="cycleRelative", poi="cycleRelative")
    mat("robotMat", (0.6, 0.65, 0.75), parts, "blinn")
    mc.playbackOptions(min=1, max=120)
    floor(40)
    lights_basic()
    look((14, 7, 14), (0, 3.5, 6))


open(os.path.join(ROOT, "tools", "make_showcase.log"), "w").write("\n".join(LOG) + "\n")
os._exit(0)
