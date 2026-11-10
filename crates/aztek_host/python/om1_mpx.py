# maya.OpenMayaMPx: the classes a 1.0 plug-in derives from, and the function set that registers them.

import maya.api.OpenMaya as _om2
import maya.OpenMaya as _om1

MPxCommand = _om2.MPxCommand
MPxNode = _om2.MPxNode


def asMPxPtr(obj):
    """1.0 creators hand their instance over through this; here the instance is the pointer."""
    return obj


class MFnPlugin(_om2.MFnPlugin):
    """MFnPlugin(mobject, vendor, version, requiredApiVersion)."""

    def registerCommand(self, name, creator, syntaxCreator=None):
        _om2.MFnPlugin.registerCommand(self, name, creator, syntaxCreator)

    def deregisterCommand(self, name):
        _om2.MFnPlugin.deregisterCommand(self, name)

    def registerNode(self, name, typeId, creator, initialize=None, nodeType=MPxNode.kDependNode, classification=None):
        _om2.MFnPlugin.registerNode(self, name, typeId, creator, initialize, nodeType, classification)

    def deregisterNode(self, typeId):
        _om2.MFnPlugin.deregisterNode(self, typeId)


__all__ = ["MPxCommand", "MPxNode", "MFnPlugin", "asMPxPtr"]
