# The basics through `aztek --batch`, end to end: the same results as tests/basics_python.rs. Grows with each PLAN item.
import maya.cmds as cmds

cmds.file(new=True, force=True)
assert cmds.polyCube() == ["pCube1", "polyCube1"]
cmds.move(1, 2, 3)
assert cmds.getAttr("pCube1.translate") == [(1.0, 2.0, 3.0)]
assert cmds.listRelatives("pCube1", p=True) is None
assert cmds.ls(sl=True) == ["pCube1"]
