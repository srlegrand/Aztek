"""Step 4: builds Grox's rig on grox_geo.ma and saves out/grox_rig.ma. Run inside Aztek from the work folder:
GROX_WORK=$PWD GROX_TOOLS=<this folder> aztek --script rig.py"""
import sys, os, json, traceback, math
sys.path.insert(0, os.environ.get("GROX_TOOLS", os.getcwd()))
import maya.cmds as mc
import maya.mel as mel
import design as D

M = os.environ.get("GROX_WORK", os.getcwd()) + "/"
OUT = M + "out/"
LOG = []
def log(*a):
    LOG.append(" ".join(map(str, a)))
def step(name):
    def wrap(fn):
        try:
            fn(); log("ok", name)
        except Exception:
            log("FAIL", name, traceback.format_exc())
        return fn
    return wrap

SK = D.full_skeleton()
COL = {"L": 6, "R": 13, "C": 17}
def side(n): return n[0]
def JN(n): return n + "_JNT"

mc.file(M + "grox_geo.ma", o=True, f=True)
mc.currentUnit(l="cm", a="deg", t="film")
for g in ["grox_RIG", "GEO_GRP", "PROXY_GRP", "SKEL_GRP", "CTRL_GRP", "RIGNODES_GRP"]:
    mc.group(em=True, n=g)
mc.parent("GEO_GRP", "CTRL_GRP", "RIGNODES_GRP", "grox_RIG")
mc.parent("PROXY_GRP", "GEO_GRP")
mc.parent("grox_body_GEO", "GEO_GRP")
mc.setAttr("RIGNODES_GRP.visibility", 0)

# ---------------------------------------------------------------------------------------------- skeleton
@step("skeleton")
def _():
    made = set()
    def make(n):
        if n in made: return
        par, p = SK[n]
        if par: make(par)
        mc.select(JN(par) if par else "SKEL_GRP") if par else mc.select(cl=True)
        mc.joint(p=tuple(p), n=JN(n))
        if not par: mc.parent(JN(n), "SKEL_GRP")
        made.add(n)
    for n in SK: make(n)
    mc.joint(JN("C_root"), e=True, oj="xyz", sao="yup", ch=True, zso=True)
    for n in SK:
        if not mc.listRelatives(JN(n), c=True, type="joint"):
            mc.setAttr(JN(n) + ".jointOrient", 0, 0, 0)
    mc.setAttr("SKEL_GRP.visibility", 1)

def wpos(n): return mc.xform(n, q=True, ws=True, t=True)

def curve_shape(kind, size):
    s = size
    if kind == "circle":
        c = mc.circle(nr=(1, 0, 0), r=s, s=10, ch=False)[0]
    elif kind == "box":
        q = [(-1, -1, -1), (1, -1, -1), (1, 1, -1), (-1, 1, -1), (-1, -1, -1), (-1, -1, 1), (1, -1, 1), (1, -1, -1), (1, 1, -1), (1, 1, 1), (1, -1, 1), (1, 1, 1), (-1, 1, 1), (-1, -1, 1), (-1, 1, 1), (-1, 1, -1)]
        c = mc.curve(d=1, p=[(x * s, y * s, z * s) for x, y, z in q])
    elif kind == "diamond":
        q = [(0, 1, 0), (1, 0, 0), (0, -1, 0), (-1, 0, 0), (0, 1, 0), (0, 0, 1), (0, -1, 0), (0, 0, -1), (0, 1, 0)]
        c = mc.curve(d=1, p=[(x * s, y * s, z * s) for x, y, z in q])
    elif kind == "cross":
        q = [(0, 1, 0), (0, -1, 0), (0, 0, 0), (1, 0, 0), (-1, 0, 0), (0, 0, 0), (0, 0, 1), (0, 0, -1)]
        c = mc.curve(d=1, p=[(x * s, y * s, z * s) for x, y, z in q])
    elif kind == "arrows":
        q = [(-1, 0, -0.3), (-1, 0, 0.3), (-0.3, 0, 0.3), (-0.3, 0, 1), (-0.6, 0, 1), (0, 0, 1.6), (0.6, 0, 1), (0.3, 0, 1), (0.3, 0, 0.3), (1, 0, 0.3), (1, 0, -0.3), (0.3, 0, -0.3), (0.3, 0, -1), (-0.3, 0, -1), (-0.3, 0, -0.3), (-1, 0, -0.3)]
        c = mc.curve(d=1, p=[(x * s, y * s, z * s) for x, y, z in q])
    elif kind == "flat":
        c = mc.circle(nr=(0, 1, 0), r=s, s=12, ch=False)[0]
    return c

def ctrl(name, kind, size, at=None, parent="CTRL_GRP", color=None, orient=True, pos=None):
    c = curve_shape(kind, size)
    c = mc.rename(c, name + "_CTRL")
    sh = mc.listRelatives(c, s=True)[0]
    sh = mc.rename(sh, name + "_CTRLShape")
    mc.setAttr(sh + ".overrideEnabled", 1)
    mc.setAttr(sh + ".overrideColor", color if color is not None else COL.get(name[0], 17))
    off = mc.group(em=True, n=name + "_OFF")
    drv = mc.group(em=True, n=name + "_DRV", p=off)
    mc.parent(c, drv)
    if at:
        mc.matchTransform(off, at, pos=True, rot=orient)
    if pos is not None:
        mc.xform(off, ws=True, t=pos)
    mc.parent(off, parent)
    for a in ("tx", "ty", "tz", "rx", "ry", "rz", "sx", "sy", "sz"):
        mc.setAttr(c + "." + a, 0 if a[0] != "s" else 1)
    mc.setAttr(c + ".visibility", keyable=False, channelBox=False)
    return c

def lock(c, attrs):
    for a in attrs:
        mc.setAttr(c + "." + a, lock=True, keyable=False, channelBox=False)

# ---------------------------------------------------------------------------------------------- body controls
@step("global")
def _():
    g = ctrl("C_global", "arrows", 70, parent="CTRL_GRP", color=17, pos=(0, 0, 0))
    l = ctrl("C_local", "flat", 55, parent=g, color=18, pos=(0, 0, 0))
    mc.parent("SKEL_GRP", l); mc.parent("RIGNODES_GRP", l)
    mc.addAttr(g, ln="geometry", at="enum", en="full:proxy:none", k=False); mc.setAttr(g + ".geometry", cb=True)
    mc.addAttr(g, ln="skeleton", at="bool", dv=0); mc.setAttr(g + ".skeleton", cb=True)
    mc.addAttr(g, ln="geoLock", at="bool", dv=1); mc.setAttr(g + ".geoLock", cb=True)
    full = mc.createNode("condition", n="C_geoFull_CND"); mc.connectAttr(g + ".geometry", full + ".firstTerm")
    mc.setAttr(full + ".colorIfTrueR", 1); mc.setAttr(full + ".colorIfFalseR", 0); mc.connectAttr(full + ".outColorR", "grox_body_GEO.visibility")
    prox = mc.createNode("condition", n="C_geoProxy_CND"); mc.connectAttr(g + ".geometry", prox + ".firstTerm"); mc.setAttr(prox + ".secondTerm", 1)
    mc.setAttr(prox + ".colorIfTrueR", 1); mc.setAttr(prox + ".colorIfFalseR", 0); mc.connectAttr(prox + ".outColorR", "PROXY_GRP.visibility")
    mc.connectAttr(g + ".skeleton", "SKEL_GRP.visibility")
    mc.setAttr("GEO_GRP.overrideEnabled", 1); mc.setAttr("GEO_GRP.overrideDisplayType", 2)
    mc.connectAttr(g + ".geoLock", "GEO_GRP.overrideEnabled")
    mc.scaleConstraint(l, "GEO_GRP", mo=True) if False else None

@step("spine")
def _():
    cog = ctrl("C_cog", "circle", 48, at=JN("C_root"), parent="C_local_CTRL", orient=False)
    mc.setAttr("C_cog_OFF.rotate", 0, 0, 90)
    hips = ctrl("C_hips", "circle", 38, at=JN("C_root"), parent=cog)
    mc.parentConstraint(hips, JN("C_root"), mo=True, n="C_root_PCN")
    par = cog
    for n, s in (("C_spine01", 36), ("C_spine02", 38), ("C_spine03", 42), ("C_chest", 48)):
        c = ctrl(n, "circle", s, at=JN(n), parent=par)
        mc.parentConstraint(c, JN(n), mo=True, n=n + "_PCN"); par = c
    n1 = ctrl("C_neck01", "circle", 22, at=JN("C_neck01"), parent=par)
    mc.parentConstraint(n1, JN("C_neck01"), mo=True)
    n2 = ctrl("C_neck02", "circle", 20, at=JN("C_neck02"), parent=n1)
    mc.parentConstraint(n2, JN("C_neck02"), mo=True)
    # head: follows the neck or the world for its rotation
    head = ctrl("C_head", "box", 20, at=JN("C_head"), parent=n2, orient=False)
    mc.xform("C_head_OFF", ws=True, t=mc.xform(JN("C_head"), q=True, ws=True, t=True))
    mc.addAttr(head, ln="follow", at="enum", en="neck:chest:world", k=True)
    mc.addAttr(head, ln="blink", at="double", min=-1, max=1, dv=0, k=True)
    loc_neck = mc.group(em=True, n="C_headSpaceNeck_LOC", p=n2); mc.matchTransform(loc_neck, "C_head_OFF")
    loc_chest = mc.group(em=True, n="C_headSpaceChest_LOC", p="C_chest_CTRL"); mc.matchTransform(loc_chest, "C_head_OFF")
    loc_world = mc.group(em=True, n="C_headSpaceWorld_LOC", p="C_local_CTRL"); mc.matchTransform(loc_world, "C_head_OFF")
    oc = mc.orientConstraint(loc_neck, loc_chest, loc_world, "C_head_OFF", mo=True, n="C_headSpace_OCN")[0]
    wa = mc.orientConstraint(oc, q=True, wal=True)
    for i, w in enumerate(wa):
        cnd = mc.createNode("condition", n="C_headSpace%d_CND" % i)
        mc.connectAttr(head + ".follow", cnd + ".firstTerm"); mc.setAttr(cnd + ".secondTerm", i)
        mc.setAttr(cnd + ".colorIfTrueR", 1); mc.setAttr(cnd + ".colorIfFalseR", 0)
        mc.connectAttr(cnd + ".outColorR", oc + "." + w)
    mc.parentConstraint(head, JN("C_head"), mo=True, n="C_head_PCN")
    jaw = ctrl("C_jaw", "circle", 14, at=JN("C_jaw"), parent=head)
    mc.parentConstraint(jaw, JN("C_jaw"), mo=True)

@step("horns")
def _():
    for s in "LR":
        par = "C_head_CTRL"
        for i in range(1, 6):
            n = "%s_horn%02d" % (s, i)
            c = ctrl(n, "circle", 9 - i, at=JN(n), parent=par)
            mc.parentConstraint(c, JN(n), mo=True); par = c
        b = ctrl(s + "_brow", "diamond", 3, at=JN(s + "_brow"), parent="C_head_CTRL")
        mc.parentConstraint(b, JN(s + "_brow"), mo=True)

@step("eyes")
def _():
    look = ctrl("C_eyes", "box", 4, parent="C_head_CTRL", pos=(0, 228.5, 110), orient=False)
    mc.setAttr(look + "Shape.overrideColor" if False else "C_eyes_CTRLShape.overrideColor", 17)
    for s in "LR":
        e = JN(s + "_eye"); p = wpos(e)
        c = ctrl(s + "_eye", "circle", 3, parent=look, pos=(p[0], p[1], 110), orient=False)
        mc.aimConstraint(c, e, aim=(1, 0, 0), u=(0, 1, 0), wut="vector", wu=(0, 1, 0), mo=True, n=s + "_eye_ACN")
        geo = mc.polySphere(r=5.3, sx=24, sy=16, n=s + "_eye_GEO")[0]
        mc.rotate(90, 0, 0, geo)
        mc.xform(geo, ws=True, t=p); mc.parent(geo, "GEO_GRP"); mc.parentConstraint(e, geo, mo=True)
        # lids close at blink 1, open wide at -1
        for lid, sign in (("lidUpper", 1), ("lidLower", -1)):
            j = JN(s + "_" + lid)
            mc.setDrivenKeyframe(j + ".rotateZ", cd="C_head_CTRL.blink", dv=0, v=0)
            mc.setDrivenKeyframe(j + ".rotateZ", cd="C_head_CTRL.blink", dv=1, v=-38 * sign)
            mc.setDrivenKeyframe(j + ".rotateZ", cd="C_head_CTRL.blink", dv=-1, v=12 * sign)

@step("tail")
def _():
    par = "C_hips_CTRL"
    for i in range(1, 9):
        n = "C_tail%02d" % i
        c = ctrl(n, "circle", 16 - i * 1.5, at=JN(n), parent=par)
        mc.parentConstraint(c, JN(n), mo=True); par = c

# ---------------------------------------------------------------------------------------------- limbs
def dup_chain(names, prefix):
    out = []
    for i, n in enumerate(names):
        j = mc.createNode("joint", n="%s_%s_JNT" % (n, prefix))
        mc.matchTransform(j, JN(n))
        if i: mc.parent(j, out[-1])
        else: mc.parent(j, mc.listRelatives(JN(n), p=True)[0])
        mc.makeIdentity(j, apply=True, r=True)
        out.append(j)
    return out

def blend_chain(names, fk, ik, switch):
    rev = mc.createNode("reverse", n=names[0] + "_ikFk_REV")
    mc.connectAttr(switch, rev + ".inputX")
    for n, f, k in zip(names, fk, ik):
        oc = mc.orientConstraint(f, k, JN(n), n=n + "_blend_OCN")[0]
        w = mc.orientConstraint(oc, q=True, wal=True)
        mc.connectAttr(rev + ".outputX", oc + "." + w[0]); mc.connectAttr(switch, oc + "." + w[1])
        # IK stretch reaches the bound joints through their scale
        sb = mc.createNode("blendTwoAttr", n=n + "_ikFkScale_BTA")
        mc.setAttr(sb + ".input[0]", 1); mc.connectAttr(k + ".scaleX", sb + ".input[1]")
        mc.connectAttr(switch, sb + ".attributesBlender"); mc.connectAttr(sb + ".output", JN(n) + ".scaleX")
    return rev

def stretch(chain, ctrl_node, start_parent, s, part):
    start = mc.group(em=True, n="%s_%sStretchStart_LOC" % (s, part), p=start_parent); mc.matchTransform(start, chain[0], pos=True)
    end = mc.group(em=True, n="%s_%sStretchEnd_LOC" % (s, part), p=ctrl_node)
    db = mc.createNode("distanceBetween", n="%s_%s_DIST" % (s, part))
    d1 = mc.createNode("decomposeMatrix", n="%s_%sStart_DCM" % (s, part)); d2 = mc.createNode("decomposeMatrix", n="%s_%sEnd_DCM" % (s, part))
    mc.connectAttr(start + ".worldMatrix[0]", d1 + ".inputMatrix"); mc.connectAttr(end + ".worldMatrix[0]", d2 + ".inputMatrix")
    mc.connectAttr(d1 + ".outputTranslate", db + ".point1"); mc.connectAttr(d2 + ".outputTranslate", db + ".point2")
    length = sum(abs(mc.getAttr(j + ".tx")) for j in chain[1:])
    # divide by the global scale too
    gs = mc.createNode("multiplyDivide", n="%s_%sGlobal_MD" % (s, part)); mc.setAttr(gs + ".operation", 2)
    mc.connectAttr(db + ".distance", gs + ".input1X"); mc.connectAttr("C_global_CTRL.scaleY", gs + ".input2X")
    ratio = mc.createNode("multiplyDivide", n="%s_%sRatio_MD" % (s, part)); mc.setAttr(ratio + ".operation", 2)
    mc.connectAttr(gs + ".outputX", ratio + ".input1X"); mc.setAttr(ratio + ".input2X", length)
    cnd = mc.createNode("condition", n="%s_%sStretch_CND" % (s, part)); mc.setAttr(cnd + ".operation", 2)
    mc.connectAttr(ratio + ".outputX", cnd + ".firstTerm"); mc.setAttr(cnd + ".secondTerm", 1)
    mc.connectAttr(ratio + ".outputX", cnd + ".colorIfTrueR"); mc.setAttr(cnd + ".colorIfFalseR", 1)
    on = mc.createNode("blendTwoAttr", n="%s_%sStretchOn_BTA" % (s, part))
    mc.setAttr(on + ".input[0]", 1); mc.connectAttr(cnd + ".outColorR", on + ".input[1]")
    mc.connectAttr(ctrl_node + ".stretch", on + ".attributesBlender")
    for j in chain[:-1]:
        mc.connectAttr(on + ".output", j + ".scaleX")

def settings(s, part, at, parent):
    c = ctrl("%s_%sSettings" % (s, part), "cross", 6, at=at, parent=parent, orient=False)
    p = wpos(at); mc.xform("%s_%sSettings_OFF" % (s, part), ws=True, t=(p[0] + (12 if s == "L" else -12), p[1], p[2] - 8))
    mc.parentConstraint(at, "%s_%sSettings_OFF" % (s, part), mo=True)
    lock(c, ["tx", "ty", "tz", "rx", "ry", "rz", "sx", "sy", "sz"])
    mc.addAttr(c, ln="ikFk", nn="IK / FK", at="double", min=0, max=1, dv=0, k=True)
    return c

@step("arms")
def _():
    for s in "LR":
        names = [s + "_shoulder", s + "_elbow", s + "_wrist"]
        clav = ctrl(s + "_clavicle", "circle", 12, at=JN(s + "_clavicle"), parent="C_chest_CTRL")
        mc.parentConstraint(clav, JN(s + "_clavicle"), mo=True)
        fk = dup_chain(names, "FK"); ik = dup_chain(names, "IK")
        sett = settings(s, "arm", JN(s + "_wrist"), "CTRL_GRP")
        rev = blend_chain(names, fk, ik, sett + ".ikFk")
        # FK controls
        par = clav; fkc = []
        for n, j in zip(names, fk):
            c = ctrl(n + "FK", "circle", 13 if "wrist" not in n else 10, at=j, parent=par)
            mc.parentConstraint(c, j, mo=True); par = c; fkc.append(c)
        mc.connectAttr(rev + ".outputX", s + "_shoulderFK_OFF.visibility")
        # IK
        h = mc.ikHandle(sj=ik[0], ee=ik[2], sol="ikRPsolver", n=s + "_arm_IKH")[0]
        ikc = ctrl(s + "_armIK", "box", 8, at=ik[2], parent="C_local_CTRL", orient=False)
        mc.addAttr(ikc, ln="stretch", at="double", min=0, max=1, dv=1, k=True)
        mc.addAttr(ikc, ln="follow", at="enum", en="world:chest:cog", k=True)
        mc.parent(h, ikc); mc.setAttr(h + ".visibility", 0)
        mc.orientConstraint(ikc, ik[2], mo=True)
        e = wpos(ik[1]); pv = ctrl(s + "_armPV", "diamond", 4, parent="C_local_CTRL", pos=(e[0], e[1], e[2] - 45), orient=False)
        mc.poleVectorConstraint(pv, h, n=s + "_arm_PVC")
        for c in (ikc, pv):
            mc.connectAttr(sett + ".ikFk", mc.listRelatives(c, p=True)[0] + ".visibility")
        # IK space
        spaces = ["C_local_CTRL", "C_chest_CTRL", "C_cog_CTRL"]
        locs = []
        for k, sp in enumerate(spaces):
            l = mc.group(em=True, n="%s_armIKSpace%d_LOC" % (s, k), p=sp); mc.matchTransform(l, s + "_armIK_OFF"); locs.append(l)
        pc = mc.parentConstraint(*(locs + [s + "_armIK_OFF"]), mo=True, n=s + "_armIKSpace_PCN")[0]
        for k, w in enumerate(mc.parentConstraint(pc, q=True, wal=True)):
            cnd = mc.createNode("condition", n="%s_armIKSpace%d_CND" % (s, k))
            mc.connectAttr(ikc + ".follow", cnd + ".firstTerm"); mc.setAttr(cnd + ".secondTerm", k)
            mc.setAttr(cnd + ".colorIfTrueR", 1); mc.setAttr(cnd + ".colorIfFalseR", 0); mc.connectAttr(cnd + ".outColorR", pc + "." + w)
        stretch(ik, ikc, JN(s + "_clavicle"), s, "arm")
        # hand
        hand = ctrl(s + "_hand", "flat", 9, at=JN(s + "_palm"), parent="CTRL_GRP")
        mc.parentConstraint(JN(s + "_wrist"), s + "_hand_OFF", mo=True)
        mc.parentConstraint(hand, JN(s + "_palm"), mo=True)
        mc.addAttr(hand, ln="curl", at="double", min=-2, max=10, dv=0, k=True)
        mc.addAttr(hand, ln="spread", at="double", min=-10, max=10, dv=0, k=True)
        for k, f in enumerate(["clawA", "clawB", "clawC"]):
            par = hand
            for i in (1, 2, 3):
                n = "%s_%s%02d" % (s, f, i)
                c = ctrl(n, "circle", 4, at=JN(n), parent=par)
                mc.parentConstraint(c, JN(n), mo=True)
                md = mc.createNode("multiplyDivide", n=n + "_curl_MD")
                mc.connectAttr(hand + ".curl", md + ".input1X"); mc.setAttr(md + ".input2X", -9)
                mc.connectAttr(md + ".outputX", n + "_DRV.rotateZ")
                if i == 1:
                    sp = mc.createNode("multiplyDivide", n=n + "_spread_MD")
                    mc.connectAttr(hand + ".spread", sp + ".input1X"); mc.setAttr(sp + ".input2X", (k - 1) * 3)
                    mc.connectAttr(sp + ".outputX", n + "_DRV.rotateY")
                par = c
        par = hand
        for i in (1, 2):
            n = "%s_thumb%02d" % (s, i)
            c = ctrl(n, "circle", 4, at=JN(n), parent=par); mc.parentConstraint(c, JN(n), mo=True); par = c

@step("legs")
def _():
    for s in "LR":
        names = [s + "_hip", s + "_knee", s + "_ankle", s + "_ball", s + "_toe"]
        fk = dup_chain(names, "FK"); ik = dup_chain(names, "IK")
        sett = settings(s, "leg", JN(s + "_ankle"), "CTRL_GRP")
        rev = blend_chain(names[:4], fk[:4], ik[:4], sett + ".ikFk")
        par = "C_hips_CTRL"
        for n, j in zip(names[:4], fk[:4]):
            c = ctrl(n + "FK", "circle", 15 if "hip" in n else 11, at=j, parent=par)
            mc.parentConstraint(c, j, mo=True); par = c
        mc.connectAttr(rev + ".outputX", s + "_hipFK_OFF.visibility")
        a, b, t = wpos(ik[2]), wpos(ik[3]), wpos(ik[4])
        ikc = ctrl(s + "_legIK", "box", 10, parent="C_local_CTRL", pos=a, orient=False)
        for at_, kw in (("stretch", dict(min=0, max=1, dv=1)), ("roll", dict(min=-90, max=90, dv=0)), ("rollBreak", dict(min=0, max=90, dv=30)),
                        ("toeTap", dict(min=-90, max=90, dv=0)), ("heelTwist", dict(min=-90, max=90, dv=0)), ("toeTwist", dict(min=-90, max=90, dv=0)),
                        ("bank", dict(min=-90, max=90, dv=0))):
            mc.addAttr(ikc, ln=at_, at="double", k=True, **kw)
        heel = mc.group(em=True, n=s + "_heel_PIV", p=ikc); mc.xform(heel, ws=True, t=(a[0], 0, a[2] - 12))
        bin_ = mc.group(em=True, n=s + "_bankIn_PIV", p=heel); mc.xform(bin_, ws=True, t=(b[0] - (12 if s == "L" else -12), 0, b[2]))
        bout = mc.group(em=True, n=s + "_bankOut_PIV", p=bin_); mc.xform(bout, ws=True, t=(b[0] + (12 if s == "L" else -12), 0, b[2]))
        tip = mc.group(em=True, n=s + "_toeTip_PIV", p=bout); mc.xform(tip, ws=True, t=(t[0], 0, t[2] + 4))
        ball = mc.group(em=True, n=s + "_ball_PIV", p=tip); mc.xform(ball, ws=True, t=b)
        tap = mc.group(em=True, n=s + "_toeTap_PIV", p=tip); mc.xform(tap, ws=True, t=b)
        h = mc.ikHandle(sj=ik[0], ee=ik[2], sol="ikRPsolver", n=s + "_leg_IKH")[0]
        hb = mc.ikHandle(sj=ik[2], ee=ik[3], sol="ikSCsolver", n=s + "_ball_IKH")[0]
        ht = mc.ikHandle(sj=ik[3], ee=ik[4], sol="ikSCsolver", n=s + "_toe_IKH")[0]
        mc.parent(h, ball); mc.parent(hb, ball); mc.parent(ht, tap)
        for x in (h, hb, ht): mc.setAttr(x + ".visibility", 0)
        # roll: back on the heel, then the ball up to the break angle, then the toe tip
        cl = mc.createNode("clamp", n=s + "_roll_CLP")
        mc.connectAttr(ikc + ".roll", cl + ".inputR"); mc.setAttr(cl + ".minR", -90); mc.setAttr(cl + ".maxR", 0)
        mc.connectAttr(ikc + ".roll", cl + ".inputG"); mc.setAttr(cl + ".minG", 0); mc.connectAttr(ikc + ".rollBreak", cl + ".maxG")
        mc.connectAttr(ikc + ".roll", cl + ".inputB"); mc.connectAttr(ikc + ".rollBreak", cl + ".minB"); mc.setAttr(cl + ".maxB", 180)
        pm = mc.createNode("plusMinusAverage", n=s + "_toeRoll_PMA"); mc.setAttr(pm + ".operation", 2)
        mc.connectAttr(cl + ".outputB", pm + ".input1D[0]"); mc.connectAttr(ikc + ".rollBreak", pm + ".input1D[1]")
        mc.connectAttr(cl + ".outputR", heel + ".rotateX"); mc.connectAttr(cl + ".outputG", ball + ".rotateX")
        mc.connectAttr(pm + ".output1D", tip + ".rotateX")
        mc.connectAttr(ikc + ".toeTap", tap + ".rotateX")
        mc.connectAttr(ikc + ".heelTwist", heel + ".rotateY"); mc.connectAttr(ikc + ".toeTwist", tip + ".rotateY")
        bi = mc.createNode("clamp", n=s + "_bank_CLP"); sign = 1 if s == "L" else -1
        mul = mc.createNode("multDoubleLinear", n=s + "_bank_MDL"); mc.connectAttr(ikc + ".bank", mul + ".input1"); mc.setAttr(mul + ".input2", sign)
        mc.connectAttr(mul + ".output", bi + ".inputR"); mc.setAttr(bi + ".minR", -90); mc.setAttr(bi + ".maxR", 0)
        mc.connectAttr(mul + ".output", bi + ".inputG"); mc.setAttr(bi + ".maxG", 90)
        mc.connectAttr(bi + ".outputR", bin_ + ".rotateZ"); mc.connectAttr(bi + ".outputG", bout + ".rotateZ")
        k = wpos(ik[1]); pv = ctrl(s + "_legPV", "diamond", 5, parent="C_local_CTRL", pos=(k[0], k[1], k[2] + 50), orient=False)
        mc.poleVectorConstraint(pv, h, n=s + "_leg_PVC")
        for c in (ikc, pv):
            mc.connectAttr(sett + ".ikFk", mc.listRelatives(c, p=True)[0] + ".visibility")
        stretch(ik[:3], ikc, JN("C_root"), s, "leg")
        mc.setAttr(sett + ".ikFk", 1)
        # toes
        for f in "ABC":
            n = "%s_toe%s01" % (s, f)
            c = ctrl(n, "circle", 4, at=JN(n), parent=s + "_toe_OFF" if mc.objExists(s + "_toe_OFF") else "CTRL_GRP")
            mc.parentConstraint(JN(s + "_ball"), n + "_OFF", mo=True)
            mc.parentConstraint(c, JN(n), mo=True)

# ---------------------------------------------------------------------------------------------- proxy, skin, look
@step("proxy")
def _():
    pairs = [("C_root", "C_spine02", 30), ("C_spine02", "C_chest", 34), ("C_chest", "C_neck01", 30), ("C_neck01", "C_head", 15), ("C_head", "C_headEnd", 18),
             ("C_jaw", "C_jawEnd", 12)]
    for s in "LR":
        pairs += [(s + "_shoulder", s + "_elbow", 12), (s + "_elbow", s + "_wrist", 10), (s + "_wrist", s + "_palm", 8),
                  (s + "_hip", s + "_knee", 14), (s + "_knee", s + "_ankle", 10), (s + "_ankle", s + "_ball", 7), (s + "_ball", s + "_toe", 6),
                  (s + "_horn01", s + "_horn03", 6), (s + "_horn03", s + "_hornEnd", 3)]
    for i in range(1, 8):
        pairs.append(("C_tail%02d" % i, "C_tail%02d" % (i + 1), 12 - i * 1.3))
    for a, b, r in pairs:
        pa, pb = wpos(JN(a)), wpos(JN(b))
        L = math.dist(pa, pb)
        c = mc.polyCylinder(r=r * 0.8, h=L, sx=8, sy=1, ax=(1, 0, 0), n="%s_PROXY" % a.replace("_", "_", 1))[0]
        mc.matchTransform(c, JN(a), pos=True, rot=True)
        mc.move(L / 2, 0, 0, c, r=True, os=True, wd=True)
        mc.parent(c, "PROXY_GRP"); mc.parentConstraint(JN(a), c, mo=True)
        sh = mc.listRelatives(c, s=True)[0]; mc.setAttr(sh + ".overrideEnabled", 1); mc.setAttr(sh + ".overrideColor", 3)

BIND = [JN(n) for n in SK if not n.endswith("End") and not n.endswith("_toe") and "lid" not in n] + [JN(s + "_lid" + u) for s in "LR" for u in ("Upper", "Lower")]

@step("skin")
def _():
    sc = mc.skinCluster(BIND, "grox_body_GEO", tsb=True, mi=4, n="grox_body_SKN")[0]
    log("influences", len(mc.skinCluster(sc, q=True, inf=True)))

@step("look")
def _():
    rel = "textures/"
    def tex(name, path, raw):
        f = mc.shadingNode("file", asTexture=True, n=name)
        p2 = mc.shadingNode("place2dTexture", asUtility=True, n=name + "_P2D")
        for a in ("coverage", "translateFrame", "rotateFrame", "mirrorU", "mirrorV", "stagger", "wrapU", "wrapV", "repeatUV", "offset", "rotateUV", "noiseUV", "vertexUvOne", "vertexUvTwo", "vertexUvThree", "vertexCameraOne"):
            try: mc.connectAttr(p2 + "." + a, f + "." + a)
            except Exception: pass
        mc.connectAttr(p2 + ".outUV", f + ".uvCoord"); mc.connectAttr(p2 + ".outUvFilterSize", f + ".uvFilterSize")
        mc.setAttr(f + ".fileTextureName", rel + path, type="string")
        mc.setAttr(f + ".colorSpace", "Raw" if raw else "sRGB", type="string")
        if raw: mc.setAttr(f + ".ignoreColorSpaceFileRules", 1)
        return f
    m = mc.shadingNode("standardSurface", asShader=True, n="grox_skin_MTL")
    sg = mc.sets(renderable=True, noSurfaceShader=True, empty=True, n="grox_skin_SG"); mc.connectAttr(m + ".outColor", sg + ".surfaceShader")
    bc = tex("grox_baseColor_FILE", "grox_body_baseColor.png", False)
    mc.connectAttr(bc + ".outColor", m + ".baseColor")
    orm = tex("grox_ORM_FILE", "grox_body_ORM.png", True)
    mc.connectAttr(orm + ".outColorG", m + ".specularRoughness")
    mc.setAttr(m + ".specular", 0.5)
    nm = tex("grox_normal_FILE", "grox_body_normal.png", True)
    b = mc.shadingNode("bump2d", asUtility=True, n="grox_normal_BMP")
    mc.setAttr(b + ".bumpInterp", 1)
    mc.connectAttr(nm + ".outAlpha", b + ".bumpValue")
    try: mc.connectAttr(nm + ".outColor", b + ".normalMap") if mc.attributeQuery("normalMap", n=b, ex=True) else None
    except Exception as e: log("normalMap", e)
    mc.connectAttr(b + ".outNormal", m + ".normalCamera")
    mask = tex("grox_mask_FILE", "grox_body_mask.png", True)  # belly, markings, horn: for look development
    mc.sets("grox_body_GEO", e=True, forceElement=sg)
    e = mc.shadingNode("standardSurface", asShader=True, n="grox_eye_MTL")
    mc.setAttr(e + ".baseColor", 0.9, 0.55, 0.05, type="double3"); mc.setAttr(e + ".emission", 0.6)
    mc.setAttr(e + ".emissionColor", 1.0, 0.6, 0.1, type="double3"); mc.setAttr(e + ".specularRoughness", 0.15)
    esg = mc.sets(renderable=True, noSurfaceShader=True, empty=True, n="grox_eye_SG"); mc.connectAttr(e + ".outColor", esg + ".surfaceShader")
    mc.sets("L_eye_GEO", "R_eye_GEO", e=True, forceElement=esg)
    pm = mc.shadingNode("lambert", asShader=True, n="grox_proxy_MTL"); mc.setAttr(pm + ".color", 0.55, 0.25, 0.15, type="double3")
    psg = mc.sets(renderable=True, noSurfaceShader=True, empty=True, n="grox_proxy_SG"); mc.connectAttr(pm + ".outColor", psg + ".surfaceShader")
    mc.sets(mc.listRelatives("PROXY_GRP", c=True), e=True, forceElement=psg)

@step("tidy")
def _():
    mc.select(cl=True)
    mc.setAttr("C_global_CTRL.skeleton", 0)
    mc.currentTime(1)

os.makedirs(OUT, exist_ok=True)
mc.file(rename=OUT + "grox_rig.ma"); mc.file(save=True, type="mayaAscii", f=True)
open(M + "rig.log", "w").write("\n".join(LOG))
os._exit(0)
