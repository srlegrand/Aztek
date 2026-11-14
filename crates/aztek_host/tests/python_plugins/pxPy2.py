# A 2.0 plug-in: a data type and two nodes passing it, a shape with its own bounds, a locator, and a draw override
# registered through the thin maya.api.OpenMayaRender.
import maya.api.OpenMaya as om
import maya.api.OpenMayaUI as omui
import maya.api.OpenMayaRender as omr


def maya_useNewAPI():
    pass


class Points(om.MPxData):
    id = om.MTypeId(0x7f201)

    def __init__(self):
        om.MPxData.__init__(self)
        self.values = []

    def copy(self, other):
        self.values = list(other.values)

    def typeId(self):
        return Points.id

    def name(self):
        return "pyPoints"

    def writeASCII(self):
        return " ".join([str(len(self.values))] + [repr(v) for v in self.values])

    def readASCII(self, args, last):
        n = args.asInt(0)
        self.values = [args.asDouble(i + 1) for i in range(n)]
        return n


class MakePoints(om.MPxNode):
    id = om.MTypeId(0x7f202)

    @staticmethod
    def init():
        n = om.MFnNumericAttribute()
        MakePoints.count = n.create("count", "cnt", om.MFnNumericData.kInt, 0)
        t = om.MFnTypedAttribute()
        MakePoints.points = t.create("points", "pts", Points.id)
        t.writable = False
        om.MPxNode.addAttribute(MakePoints.count)
        om.MPxNode.addAttribute(MakePoints.points)
        om.MPxNode.attributeAffects(MakePoints.count, MakePoints.points)

    def compute(self, plug, data):
        if plug != MakePoints.points:
            return None
        c = data.inputValue(MakePoints.count).asInt()
        fn = om.MFnPluginData()
        fn.create(Points.id)
        p = fn.data()
        p.values = [i * 0.5 for i in range(c)]
        data.outputValue(MakePoints.points).setMPxData(p)


class SumPoints(om.MPxNode):
    id = om.MTypeId(0x7f203)

    @staticmethod
    def init():
        t = om.MFnTypedAttribute()
        SumPoints.points = t.create("points", "pts", Points.id)
        n = om.MFnNumericAttribute()
        SumPoints.sum = n.create("sum", "sum", om.MFnNumericData.kDouble, 0)
        n.writable = False
        om.MPxNode.addAttribute(SumPoints.points)
        om.MPxNode.addAttribute(SumPoints.sum)
        om.MPxNode.attributeAffects(SumPoints.points, SumPoints.sum)

    def compute(self, plug, data):
        if plug != SumPoints.sum:
            return None
        p = data.inputValue(SumPoints.points).asPluginData()
        data.outputValue(SumPoints.sum).setDouble(sum(p.values) if p else 0.0)


class BoxShape(om.MPxSurfaceShape):
    id = om.MTypeId(0x7f206)

    @staticmethod
    def init():
        n = om.MFnNumericAttribute()
        BoxShape.size = n.create("size", "sz", om.MFnNumericData.kDouble, 1)
        om.MPxNode.addAttribute(BoxShape.size)

    def isBounded(self):
        return True

    def boundingBox(self):
        s = om.MPlug(self.thisMObject(), BoxShape.size).asDouble() * 0.5
        return om.MBoundingBox(om.MPoint(-s, -s, -s), om.MPoint(s, s, s))


class Marker(omui.MPxLocatorNode):
    id = om.MTypeId(0x7f209)

    @staticmethod
    def init():
        pass


class Draw(omr.MPxDrawOverride):
    def __init__(self, obj):
        omr.MPxDrawOverride.__init__(self, obj, None)


def initializePlugin(obj):
    p = om.MFnPlugin(obj, "Aztek tests", "1.0", "Any")
    p.registerData("pyPoints", Points.id, Points)
    p.registerNode("pyMakePoints", MakePoints.id, MakePoints, MakePoints.init)
    p.registerNode("pySumPoints", SumPoints.id, SumPoints, SumPoints.init)
    p.registerShape("pyBoxShape", BoxShape.id, BoxShape, BoxShape.init, omui.MPxSurfaceShapeUI)
    p.registerNode("pyMarker", Marker.id, Marker, Marker.init, om.MPxNode.kLocatorNode, "drawdb/geometry/pyMarker")
    omr.MDrawRegistry.registerDrawOverrideCreator("drawdb/geometry/pyMarker", "pyMarker", Draw)


def uninitializePlugin(obj):
    p = om.MFnPlugin(obj)
    omr.MDrawRegistry.deregisterDrawOverrideCreator("drawdb/geometry/pyMarker", "pyMarker")
    p.deregisterNode(Marker.id)
    p.deregisterNode(BoxShape.id)
    p.deregisterNode(SumPoints.id)
    p.deregisterNode(MakePoints.id)
    p.deregisterData(Points.id)
