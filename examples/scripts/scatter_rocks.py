"""Scatters rocks on a plane: random sizes and turns, each a lumpy sphere, all sharing one material."""
import random
from aztek import cmds

random.seed(7)
ground = cmds.polyPlane(n="ground", w=2000, h=2000)[0]
rocks = []
for i in range(60):
    r = cmds.polySphere(n="rock%02d" % i, r=1, sx=10, sy=8)[0]
    for v in range(cmds.polyEvaluate(r, v=True)):
        cmds.move(random.uniform(-0.15, 0.15), random.uniform(-0.15, 0.15), random.uniform(-0.15, 0.15), "%s.vtx[%d]" % (r, v), r=True)
    s = random.uniform(10, 60)
    cmds.scale(s * random.uniform(0.8, 1.4), s * random.uniform(0.5, 0.9), s, r)
    cmds.rotate(random.uniform(-15, 15), random.uniform(0, 360), random.uniform(-15, 15), r)
    cmds.move(random.uniform(-900, 900), s * 0.4, random.uniform(-900, 900), r)
    rocks.append(r)
m = cmds.shadingNode("lambert", asShader=True, n="rockMat")
cmds.setAttr(m + ".color", 0.42, 0.4, 0.37, type="double3")
cmds.select(rocks)
cmds.hyperShade(assign=m)
cmds.group(rocks, n="rocks")
print("Scattered %d rocks." % len(rocks))
