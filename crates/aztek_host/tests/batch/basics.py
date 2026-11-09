# The basics through `aztek --batch`, end to end: the same results as tests/basics_python.rs. Grows with each PLAN item.
import maya.cmds as cmds

cmds.file(new=True, force=True)
assert cmds.polyCube() == ["pCube1", "polyCube1"]
cmds.move(1, 2, 3)
assert cmds.getAttr("pCube1.translate") == [(1.0, 2.0, 3.0)]
assert cmds.listRelatives("pCube1", p=True) is None
assert cmds.ls(sl=True) == ["pCube1"]

# Real units.
cmds.file(new=True, force=True)
cmds.polyCube()
cmds.move(100, 0, 0)
cmds.rotate(0, 90, 0)
cmds.currentUnit(linear="m", angle="rad")
assert abs(cmds.getAttr("pCube1.tx") - 1.0) < 1e-9
assert abs(cmds.getAttr("pCube1.ry") - 1.5707963267948966) < 1e-9
cmds.currentUnit(linear="cm", angle="deg")

# Groups.
cmds.file(new=True, force=True)
cmds.polyCube(); cmds.polyCube()
assert cmds.group("pCube1", "pCube2") == "group1"
assert cmds.group(em=True) == "group2"
assert cmds.xform("group1", q=True, ws=True, rp=True) == [0.0, 0.0, 0.0]

# Names.
cmds.file(new=True, force=True)
cmds.polyCube(); cmds.polyCube(); cmds.polyCube()
assert cmds.rename("pCube1", "thing#") == "thing1"
assert cmds.rename("pCube2", "thing#") == "thing2"
assert cmds.rename("pCube3", "part###") == "part001"
cmds.polyCube(n="arm_01")
assert cmds.duplicate("arm_01") == ["arm_02"]

# Pivots, selection, hotkeys.
cmds.file(new=True, force=True)
cmds.polyCube()
cmds.move(3, 0, 0)
cmds.move(0, 0, 0, "pCube1.scalePivot", "pCube1.rotatePivot", absolute=True)
assert cmds.xform("pCube1", q=True, ws=True, rp=True) == [0.0, 0.0, 0.0]
cmds.polyCube(); cmds.polyCube()
cmds.select("pCube1")
cmds.InvertSelection()
assert cmds.ls(sl=True) == ["pCube2", "pCube3"]
cmds.nameCommand("makeCubeNameCommand", ann="Make a cube", c="polyCube")
cmds.hotkey(k="F12", n="makeCubeNameCommand")
assert cmds.hotkey(k="F12", q=True, n=True) == "makeCubeNameCommand"
