"""Poses a rig through a fixed set of control moves and writes every joint's world matrix and a sample of every skinned
mesh's points, so the same rig can be compared between the reference and Aztek.

Run it the same way in both, from the Script Editor or a batch session:

    import runpy; runpy.run_path("tools/rig_check/pose_dump.py", run_name="__main__")

with RIG_CHECK_SCENE set to the rig file and RIG_CHECK_OUT to the JSON to write (defaults: the open scene, and
pose_dump.json beside it). Compare two dumps with tools/rig_check/pose_compare.py.
"""

import json
import math
import os

import maya.cmds as cmds


def poses():
    """Each pose: a name and the attributes it sets, from the rest pose. mGear names; controls that are missing are
    skipped."""
    out = [("rest", {})]
    out.append(("world moved and turned", {"world_ctl.t": (30, 10, -20), "world_ctl.r": (20, 75, -10)}))
    out.append(("global moved and turned", {"global_C0_ctl.t": (-15, 0, 40), "global_C0_ctl.r": (-30, 140, 15)}))
    for side in "LR":
        for limb in ("arm", "leg"):
            blend = "%sUI_%s0_ctl.%s_blend" % (limb, side, limb)
            ik = "%s_%s0_ik_ctl" % (limb, side)
            upv = "%s_%s0_upv_ctl" % (limb, side)
            fk0 = "%s_%s0_fk0_ctl" % (limb, side)
            fk1 = "%s_%s0_fk1_ctl" % (limb, side)
            for i in range(12):
                a = 2 * math.pi * i / 12
                out.append(("%s %s ik circle %d" % (limb, side, i), {blend: 1, ik + ".t": (25 * math.cos(a) - 25, 25 * math.sin(a), 10)}))
            for ax in "xyz":
                for deg in (-135, -60, 60, 135, 179):
                    out.append(("%s %s ik r%s %d" % (limb, side, ax, deg), {blend: 1, "%s.r%s" % (ik, ax): deg}))
            out.append(("%s %s upv moved" % (limb, side), {blend: 1, upv + ".t": (20, -15, 30)}))
            for ax in "xyz":
                for deg in (-90, -45, 45, 90):
                    out.append(("%s %s fk0 r%s %d" % (limb, side, ax, deg), {blend: 0, "%s.r%s" % (fk0, ax): deg}))
            out.append(("%s %s fk1 bent" % (limb, side), {blend: 0, fk1 + ".rz": -95}))
    for c in ("body_C0_ctl", "spine_C0_ik1_ctl", "spine_C0_fk1_ctl", "neck_C0_ik_ctl", "neck_C0_head_ctl"):
        for ax in "xyz":
            for deg in (-60, 60):
                out.append(("%s r%s %d" % (c, ax, deg), {"%s.r%s" % (c, ax): deg}))
    return out


def settable(plug):
    node = plug.split(".")[0]
    if not cmds.objExists(node) or not cmds.attributeQuery(plug.split(".", 1)[1].split(".")[0], node=node, exists=True):
        return False
    try:
        return cmds.getAttr(plug, settable=True)
    except Exception:
        return False


def apply(edits):
    done = {}
    for plug, v in edits.items():
        if not settable(plug):
            continue
        done[plug] = cmds.getAttr(plug)
        if isinstance(v, tuple):
            cmds.setAttr(plug, *v)
        else:
            cmds.setAttr(plug, v)
    return done


def restore(done):
    for plug, v in done.items():
        if isinstance(v, list):
            cmds.setAttr(plug, *v[0])
        else:
            cmds.setAttr(plug, v)


def skinned_meshes():
    out = set()
    for sc in cmds.ls(type="skinCluster") or []:
        for m in cmds.listConnections(sc, s=False, d=True, shapes=True, type="mesh") or []:
            out.add(m)
    return sorted(out)


def main():
    scene = os.environ.get("RIG_CHECK_SCENE")
    if scene:
        cmds.file(scene, o=True, f=True)
    target = os.environ.get("RIG_CHECK_OUT") or os.path.join(os.path.dirname(cmds.file(q=True, sn=True) or "."), "pose_dump.json")
    joints = sorted(cmds.ls(type="joint", long=False) or [])
    meshes = skinned_meshes()
    # The same points in every pose: up to 200 a mesh, evenly spread.
    sample = {}
    for m in meshes:
        n = cmds.polyEvaluate(m, vertex=True) or 0
        step = max(1, n // 200)
        sample[m] = list(range(0, n, step))
    result = {"joints": joints, "poses": []}
    for name, edits in poses():
        done = apply(edits)
        pose = {"name": name, "set": sorted(done), "joints": {}, "points": {}}
        for j in joints:
            pose["joints"][j] = [round(x, 6) for x in cmds.getAttr(j + ".worldMatrix[0]")]
        for m, ids in sample.items():
            pts = []
            for i in ids:
                pts.append([round(x, 5) for x in cmds.pointPosition("%s.vtx[%d]" % (m, i), world=True)])
            pose["points"][m] = pts
        result["poses"].append(pose)
        restore(done)
    with open(target, "w") as f:
        json.dump(result, f)
    print("pose_dump: %d poses, %d joints, %d meshes -> %s" % (len(result["poses"]), len(joints), len(meshes), target))


if __name__ == "__main__":
    main()
