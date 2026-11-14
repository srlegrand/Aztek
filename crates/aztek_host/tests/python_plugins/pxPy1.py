# A 1.0 plug-in of each kind the reference has only in 1.0: a transform with its own matrix (registerTransform), a
# constraint and its command, and an IK solver that aims the chain.
import math
import maya.OpenMaya as om
import maya.OpenMayaAnim as oma
import maya.OpenMayaMPx as mpx


class RockMatrix(mpx.MPxTransformationMatrix):
    id = om.MTypeId(0x7f304)

    def __init__(self):
        mpx.MPxTransformationMatrix.__init__(self)
        self.rock = 0.0

    def asMatrix(self, *a):
        return om.MEulerRotation(self.rock, 0, 0).asMatrix() * mpx.MPxTransformationMatrix.asMatrix(self)


class Rock(mpx.MPxTransform):
    id = om.MTypeId(0x7f305)
    rockX = om.MObject()

    def __init__(self):
        mpx.MPxTransform.__init__(self)

    def computeLocalTransformation(self, xform, block):
        xform.rock = block.inputValue(Rock.rockX).asDouble()
        return mpx.MPxTransform.computeLocalTransformation(self, xform, block)


def rockInit():
    n = om.MFnNumericAttribute()
    Rock.rockX = n.create("rockX", "rx2", om.MFnNumericData.kDouble, 0)
    n.setKeyable(True)
    Rock.addAttribute(Rock.rockX)
    Rock.attributeAffects(Rock.rockX, mpx.MPxTransform.matrix)


class Avg(mpx.MPxConstraint):
    id = om.MTypeId(0x7f307)

    def __init__(self):
        mpx.MPxConstraint.__init__(self)

    def compute(self, plug, data):
        a = data.inputArrayValue(Avg.targetTranslate)
        w = data.inputArrayValue(Avg.targetWeight)
        ws = {}
        for i in range(w.elementCount()):
            w.jumpToPhysicalElement(i)
            ws[w.elementLogicalIndex()] = w.inputValue().asDouble()
        s = om.MVector()
        tot = 0.0
        for i in range(a.elementCount()):
            a.jumpToPhysicalElement(i)
            k = ws.get(a.elementLogicalIndex(), 1.0)
            s += a.inputValue().asVector() * k
            tot += k
        data.outputValue(Avg.constraintTranslate).setMVector(s / tot if tot else om.MVector())

    def weightAttribute(self):
        return Avg.targetWeight

    def targetAttribute(self):
        return Avg.targetTranslate


def avgInit():
    n = om.MFnNumericAttribute()
    Avg.targetTranslate = n.create("targetTranslate", "tt", om.MFnNumericData.k3Double)
    n.setArray(True)
    Avg.targetWeight = n.create("targetWeight", "tw", om.MFnNumericData.kDouble, 1)
    n.setArray(True)
    Avg.constraintTranslate = n.create("constraintTranslate", "ct", om.MFnNumericData.k3Double)
    n.setWritable(False)
    for x in (Avg.targetTranslate, Avg.targetWeight, Avg.constraintTranslate):
        Avg.addAttribute(x)
    Avg.attributeAffects(Avg.targetTranslate, Avg.constraintTranslate)
    Avg.attributeAffects(Avg.targetWeight, Avg.constraintTranslate)


class AvgCmd(mpx.MPxConstraintCommand):
    def __init__(self):
        mpx.MPxConstraintCommand.__init__(self)

    def constraintTypeId(self):
        return Avg.id

    def constraintOutputAttribute(self):
        return Avg.constraintTranslate

    def constraintTargetInstancedAttribute(self):
        return mpx.MPxTransform.translate

    def constraintTargetAttribute(self):
        return Avg.targetTranslate

    def constraintTargetWeightAttribute(self):
        return Avg.targetWeight

    def objectAttribute(self):
        return mpx.MPxTransform.translate


class Aim(mpx.MPxIkSolverNode):
    id = om.MTypeId(0x7f308)

    def __init__(self):
        mpx.MPxIkSolverNode.__init__(self)

    def solverTypeName(self):
        return "pyAimSolver"

    def doSolve(self):
        g = self.handleGroup()
        h = g.handle(0)
        hp = om.MDagPath()
        om.MDagPath.getAPathTo(h, hp)
        handle = oma.MFnIkHandle(hp)
        start = om.MDagPath()
        eff = om.MDagPath()
        handle.getStartJoint(start)
        handle.getEffector(eff)
        joint = oma.MFnIkJoint(start)
        goal = handle.rotatePivot(om.MSpace.kWorld)
        root = joint.rotatePivot(om.MSpace.kWorld)
        end = oma.MFnIkEffector(eff).rotatePivot(om.MSpace.kWorld)
        q = om.MQuaternion(end - root, goal - root)
        joint.rotateBy(q, om.MSpace.kWorld)


def initializePlugin(obj):
    p = mpx.MFnPlugin(obj, "Aztek tests", "1.0", "Any")
    p.registerTransform("pyRockTransform", Rock.id, lambda: mpx.asMPxPtr(Rock()), rockInit, lambda: mpx.asMPxPtr(RockMatrix()), RockMatrix.id)
    p.registerNode("pyAvgConstraint", Avg.id, lambda: mpx.asMPxPtr(Avg()), avgInit, mpx.MPxNode.kConstraintNode)
    p.registerConstraintCommand("pyAvgConstraint", lambda: mpx.asMPxPtr(AvgCmd()))
    p.registerNode("pyAimSolver", Aim.id, lambda: mpx.asMPxPtr(Aim()), lambda: None, mpx.MPxNode.kIkSolverNode)


def uninitializePlugin(obj):
    p = mpx.MFnPlugin(obj)
    p.deregisterNode(Aim.id)
    p.deregisterConstraintCommand("pyAvgConstraint")
    p.deregisterNode(Avg.id)
    p.deregisterNode(Rock.id)
