"""A Python plug-in with one command, helloCube: makes a cube with a given size and says so.
Load it with: loadPlugin "<path>/hello_command_plugin.py"; then run helloCube -size 3;"""
import maya.api.OpenMaya as om
import maya.cmds as cmds


def maya_useNewAPI():
    pass


class HelloCube(om.MPxCommand):
    name = "helloCube"

    @staticmethod
    def syntax():
        s = om.MSyntax()
        s.addFlag("-s", "-size", om.MSyntax.kDouble)
        return s

    def doIt(self, args):
        db = om.MArgDatabase(self.syntax(), args)
        size = db.flagArgumentDouble("-size", 0) if db.isFlagSet("-size") else 1.0
        cube = cmds.polyCube(w=size, h=size, d=size, n="helloCube")[0]
        om.MGlobal.displayInfo("helloCube made %s, %g across." % (cube, size))
        self.setResult(cube)


def initializePlugin(obj):
    om.MFnPlugin(obj, "Aztek examples", "1.0").registerCommand(HelloCube.name, HelloCube, HelloCube.syntax)


def uninitializePlugin(obj):
    om.MFnPlugin(obj).deregisterCommand(HelloCube.name)
