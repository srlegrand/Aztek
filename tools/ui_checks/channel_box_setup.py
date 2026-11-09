# Two cubes selected, and whenever a value changes or an undo runs, the values the check reads written to a file. Not on
# idle: a job that fires then moves the core's version, the panels rebuild and a field being typed in loses the keyboard.
import json
import os

import maya.cmds as cmds

OUT = os.environ["CHECK_STATE"]
cmds.polyCube()
cmds.polyCube()
cmds.select("pCube1", "pCube2")


def write_state():
    state = {
        c: [cmds.getAttr("%s.s%s" % (c, a)) for a in "xyz"] + [cmds.getAttr("%s.t%s" % (c, a)) for a in "xyz"] for c in ("pCube1", "pCube2")
    }
    with open(OUT + ".tmp", "w") as f:
        json.dump(state, f)
    os.replace(OUT + ".tmp", OUT)


write_state()
for c in ("pCube1", "pCube2"):
    for a in ("sx", "sy", "sz", "tx", "ty", "tz"):
        cmds.scriptJob(attributeChange=["%s.%s" % (c, a), write_state])
cmds.scriptJob(event=["Undo", write_state])
