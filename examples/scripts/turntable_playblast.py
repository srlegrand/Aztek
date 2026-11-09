"""Puts the selection on a turntable: a camera circling it over 100 frames, then a playblast to images beside the scene."""
import os
from aztek import cmds

sel = cmds.ls(sl=True) or cmds.ls(type="mesh") and [cmds.listRelatives(m, p=True)[0] for m in cmds.ls(type="mesh")]
bb = cmds.exactWorldBoundingBox(sel)
cx, cy, cz = (bb[0] + bb[3]) / 2, (bb[1] + bb[4]) / 2, (bb[2] + bb[5]) / 2
size = max(bb[3] - bb[0], bb[4] - bb[1], bb[5] - bb[2])
pivot = cmds.group(em=True, n="turntable")
cmds.move(cx, cy, cz, pivot)
cam = cmds.camera(n="turntableCam", focalLength=50)[0]
cmds.parent(cam, pivot, r=True)
cmds.move(0, size * 0.3, size * 2.2, cam, os=True, r=True)
cmds.aimConstraint(pivot, cam, aim=(0, 0, -1), u=(0, 1, 0))
cmds.setKeyframe(pivot, at="ry", t=1, v=0)
cmds.setKeyframe(pivot, at="ry", t=101, v=360)
cmds.keyTangent(pivot, at="ry", itt="linear", ott="linear")
cmds.playbackOptions(min=1, max=100)
cmds.lookThru(cam)
scene = cmds.file(q=True, sn=True) or os.path.join(os.path.expanduser("~"), "untitled.ma")
out = os.path.splitext(scene)[0] + "_turntable"
cmds.playblast(st=1, et=100, f=out, fmt="image", c="png", v=False, fo=True, p=100, w=960, h=540)
print("Turntable images: %s.####.png" % out)
