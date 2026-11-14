# maya.OpenMayaMPx: the classes a 1.0 plug-in derives from, and the function set that registers them. Transforms,
# constraints and IK solvers are 1.0 only in the reference: its 2.0 API has no MPxTransform, MPxConstraint or
# MPxIkSolverNode.

import aztek.api.OpenAztek as _om2
import aztek.api.OpenAztekUI as _omui2
import aztek.OpenAztek as _om1
import aztek.OpenAztekAnim as _oma1

MPxCommand = _om2.MPxCommand
MPxNode = _om2.MPxNode
MPxData = _om2.MPxData
MPxSurfaceShape = _om2.MPxSurfaceShape
MPxLocatorNode = _omui2.MPxLocatorNode
MPxSurfaceShapeUI = _omui2.MPxSurfaceShapeUI


def asMPxPtr(obj):
    """1.0 creators hand their instance over through this; here the instance is the pointer."""
    return obj


# ---------------------------------------------------------------------------------------------------------------------
# Transforms


class MPxTransformationMatrix(_om1.MTransformationMatrix):
    """A transform's local matrix, built from its channels. A plug-in's own adds to asMatrix."""

    baseTransformationMatrixId = _om1.MTypeId(0x80000, 0)

    def __init__(self, *args):
        _om2.MTransformationMatrix.__init__(self, *args)

    @staticmethod
    def creator():
        return MPxTransformationMatrix()

    def typeId(self):
        return MPxTransformationMatrix.baseTransformationMatrixId

    def copyValues(self, other):
        self.__dict__.update(_om2.MTransformationMatrix(other).__dict__)

    def translateTo(self, vector, space=_om1.MSpace.kTransform):
        _om2.MTransformationMatrix.setTranslation(self, vector, space)

    def translateBy(self, vector, space=_om1.MSpace.kTransform):
        _om2.MTransformationMatrix.translateBy(self, vector, space)

    def rotateTo(self, rot, space=_om1.MSpace.kTransform):
        _om2.MTransformationMatrix.setRotation(self, rot)

    def scaleTo(self, v, space=_om1.MSpace.kTransform):
        _om2.MTransformationMatrix.setScale(self, list(v), space)

    def shearTo(self, v, space=_om1.MSpace.kTransform):
        _om2.MTransformationMatrix.setShear(self, list(v), space)

    def setRotateOrientation(self, q, space=_om1.MSpace.kTransform, balance=True):
        _om2.MTransformationMatrix.setRotationOrientation(self, q)

    def rotateOrientation(self, space=_om1.MSpace.kTransform):
        return _om1._to1(_om2.MTransformationMatrix.rotationOrientation(self))

    def decomposeMatrix(self, m):
        _om2.MTransformationMatrix.__init__(self, m)


def _block_value(block, name, default):
    v = block._values.get(name)
    return default if v is None else v


class MPxTransform(MPxNode):
    """A transform of a plug-in's own: its local matrix is what its transformation matrix's asMatrix gives."""

    def __init__(self, *args):
        MPxNode.__init__(self)
        self._xform = None

    def type(self):
        return MPxNode.kTransformNode

    def createTransformationMatrix(self):
        e = _om2._node_classes.get(getattr(self, "_type_name", None)) or {}
        make = e.get("matrix_creator")
        return make() if make is not None else MPxTransformationMatrix()

    def computeLocalTransformation(self, xform, block):
        """The channels into the transformation matrix, as the reference's transform has them."""
        T = _om2.MTransformationMatrix
        v = lambda n, d=(0.0, 0.0, 0.0): _block_value(block, n, list(d))
        T.setScalePivot(xform, _om2.MPoint(v("scalePivot")), _om2.MSpace.kTransform, False)
        T.setScalePivotTranslation(xform, _om2.MVector(v("scalePivotTranslate")), _om2.MSpace.kTransform)
        T.setRotatePivot(xform, _om2.MPoint(v("rotatePivot")), _om2.MSpace.kTransform, False)
        T.setRotatePivotTranslation(xform, _om2.MVector(v("rotatePivotTranslate")), _om2.MSpace.kTransform)
        T.setScale(xform, v("scale", (1.0, 1.0, 1.0)), _om2.MSpace.kTransform)
        T.setShear(xform, v("shear"), _om2.MSpace.kTransform)
        ra = v("rotateAxis")
        T.setRotationOrientation(xform, _om2.MEulerRotation(ra[0], ra[1], ra[2]).asQuaternion())
        r = v("rotate")
        T.setRotation(xform, _om2.MEulerRotation(r[0], r[1], r[2], int(_block_value(block, "rotateOrder", 0))))
        T.setTranslation(xform, _om2.MVector(v("translate")), _om2.MSpace.kTransform)
        return None

    def _az_local_matrix(self, block):
        xf = self.createTransformationMatrix()
        self.computeLocalTransformation(xf, block)
        self._xform = xf
        return xf.asMatrix()

    def transformationMatrix(self):
        return _om1._to1(self._xform.asMatrix()) if self._xform is not None else _om1.MMatrix()

    def transformationMatrixPtr(self):
        return self._xform

    def resetTransformation(self, matrix=None):
        pass

    def updateMatrixAttrs(self, *args):
        pass

    def isBounded(self):
        return False

    def boundingBox(self):
        return _om1.MBoundingBox()

    def treatAsTransform(self):
        return True


_TRANSFORM_ATTRS = (
    "translate", "translateX", "translateY", "translateZ", "rotate", "rotateX", "rotateY", "rotateZ", "rotateOrder",
    "scale", "scaleX", "scaleY", "scaleZ", "shear", "shearXY", "shearXZ", "shearYZ", "rotatePivot", "rotatePivotX",
    "rotatePivotY", "rotatePivotZ", "rotatePivotTranslate", "rotatePivotTranslateX", "rotatePivotTranslateY",
    "rotatePivotTranslateZ", "scalePivot", "scalePivotX", "scalePivotY", "scalePivotZ", "scalePivotTranslate",
    "scalePivotTranslateX", "scalePivotTranslateY", "scalePivotTranslateZ", "rotateAxis", "rotateAxisX",
    "rotateAxisY", "rotateAxisZ", "transMinusRotatePivot", "inheritsTransform", "displayHandle",
    "displayScalePivot", "displayRotatePivot", "displayLocalAxis", "selectHandle", "visibility", "matrix",
    "inverseMatrix", "worldMatrix", "worldInverseMatrix", "parentMatrix", "parentInverseMatrix", "xformMatrix",
    "rotationInterpolation", "dynamics", "showManipDefault", "specifiedManipLocation", "minTransLimit",
    "maxTransLimit", "minRotLimit", "maxRotLimit", "minScaleLimit", "maxScaleLimit", "boundingBox", "center",
    "instObjGroups", "intermediateObject", "isTemplated", "objectColor", "useObjectColor", "objectColorRGB",
)
for _n in _TRANSFORM_ATTRS:
    setattr(MPxTransform, _n, _om2._builtin_attr(_n))
MPxTransform.nodeBoundingBox = _om2._builtin_attr("boundingBox")
del _n


# ---------------------------------------------------------------------------------------------------------------------
# Constraints


class MPxConstraint(MPxNode):
    """A constraint of a plug-in's own: a transform under what it drives, its compute the plug-in's."""

    kObject = 0
    kObjectRotation = 1
    kWorld = 2

    def type(self):
        return MPxNode.kConstraintNode

    def weightAttribute(self):
        return _om1.MObject()

    def targetAttribute(self):
        return _om1.MObject()

    def passiveOutputAttribute(self):
        return _om1.MObject()

    def getOutputAttributes(self, attributeArray):
        pass

    def constraintRotateOrderAttribute(self):
        return _om1.MObject()


MPxConstraint.enableRestPosition = _om2._builtin_attr("enableRestPosition")
MPxConstraint.lockOutput = _om2._builtin_attr("lockOutput")


def _attr_text(attr):
    a = getattr(attr, "_attr", None)
    if isinstance(a, dict):
        return a["name"]
    return a or ""


def _element_plug(attr, index):
    """An attribute's plug for target `index`: its own element, or its element under the array compound it sits in."""
    a = getattr(attr, "_attr", None)
    if isinstance(a, dict):
        parent = a.get("parent")
        if parent:
            spec = next((s for e in _om2._node_classes.values() for s in e["attrs"] if s["name"] == parent), None)
            if spec is not None and spec.get("array"):
                return "%s[%d].%s" % (parent, index, a["name"])
        if a.get("array"):
            return "%s[%d]" % (a["name"], index)
    return _attr_text(attr)


def _null(o):
    return o is None or (hasattr(o, "isNull") and o.isNull())


class MPxConstraintCommand(MPxCommand):
    """The command that makes a plug-in's constraint, as the reference's constraint commands do: the constraint under
    the driven object, its output onto the object, each target connected with its weight at 1."""

    kTransform = 0
    kGeometryShape = 1
    kLast = 2

    def __init__(self):
        MPxCommand.__init__(self)
        self._constraint = None

    def appendSyntax(self):
        pass

    def parseArgs(self, argList):
        pass

    def constraintTypeId(self):
        return _om1.MTypeId()

    def constraintInstancedAttribute(self):
        return _om1.MObject()

    def constraintOutputAttribute(self):
        return _om1.MObject()

    def constraintTargetInstancedAttribute(self):
        return _om1.MObject()

    def constraintTargetAttribute(self):
        return _om1.MObject()

    def constraintTargetWeightAttribute(self):
        return _om1.MObject()

    def objectAttribute(self):
        return _om1.MObject()

    def targetType(self):
        return MPxConstraintCommand.kTransform

    def createdConstraint(self, constraint):
        pass

    def setRestPosition(self, obj):
        pass

    def supportsOffset(self):
        return False

    def hasVectorFlags(self):
        return False

    def hasUpVectorFlags(self):
        return False

    def handleNewTargets(self, obj):
        pass

    def connectObjectAndConstraint(self, modifier):
        pass

    def syntax(self):
        return self._syntax_obj

    def isUndoable(self):
        # Not undoable on its own: the commands it runs are, each in the same undo step.
        return False

    def _node(self):
        return _om2._api("node.info", id=self._constraint)["full"]

    def connectTargetAttribute(self, target, index, targetAttr, constraintAttr, targetIsArray=False):
        src = "%s.%s%s" % (target.fullPathName(), _attr_text(targetAttr), "[0]" if targetIsArray else "")
        _om2._cmds.connectAttr(src, self._node() + "." + _element_plug(constraintAttr, index), force=True)

    def connectObjectAttribute(self, child, objectAttr, constraintAttr, objectIsArray=False):
        obj = "%s.%s%s" % (child.fullPathName(), _attr_text(objectAttr), "[0]" if objectIsArray else "")
        _om2._cmds.connectAttr(obj, self._node() + "." + _attr_text(constraintAttr), force=True)

    def connectTargetMatrixAttribute(self, target, index, constraintAttr):
        self.connectTargetAttribute(target, index, MPxTransform.worldMatrix, constraintAttr, False)

    def connectTarget(self, target, index):
        a, b = self.constraintTargetInstancedAttribute(), self.constraintTargetAttribute()
        if _null(a) or _null(b):
            return
        self.connectTargetAttribute(target, index, a, b, _attr_text(a) in ("worldMatrix", "worldMesh"))

    def doIt(self, args):
        names = _om2.MArgParser(self._syntax_obj or _om2.MSyntax(), args).getObjectStrings()
        if not names:
            names = _om2._cmds.ls(selection=True, long=True) or []
        if len(names) < 2:
            raise RuntimeError("(kInvalidParameter): name the targets, then the object to constrain")
        self.parseArgs(args)
        obj = names[-1]
        targets = names[:-1]
        type_name = _om2._node_type_of_id(_om2.MTypeId(self.constraintTypeId()))
        short = obj.split("|")[-1]
        made = _om2._cmds.createNode(type_name, parent=obj, name="%s_%s1" % (short, type_name))
        made = made[0] if isinstance(made, list) else made
        self._constraint = _om2._node_id_named(made)
        node = self._node()
        obj_path = _om1.MDagPath(_om2.MSelectionList().add(obj).getDagPath(0))
        inst = self.constraintInstancedAttribute()
        if not _null(inst):
            self.connectObjectAttribute(obj_path, MPxTransform.parentInverseMatrix, inst, False)
        out, obj_attr = self.constraintOutputAttribute(), self.objectAttribute()
        if not _null(out) and not _null(obj_attr):
            _om2._cmds.connectAttr(node + "." + _attr_text(out), obj_path.fullPathName() + "." + _attr_text(obj_attr), force=True)
        weight = self.constraintTargetWeightAttribute()
        for i, t in enumerate(targets):
            try:
                tp = _om1.MDagPath(_om2.MSelectionList().add(t).getDagPath(0))
            except Exception:
                continue
            self.connectTarget(tp, i)
            if not _null(weight):
                _om2._cmds.setAttr(node + "." + _element_plug(weight, i), 1.0)
        mod = _om1.MDGModifier()
        self.connectObjectAndConstraint(mod)
        mod.doIt()
        cobj = _om2._node_object(self._constraint)
        self.handleNewTargets(_om1._to1(cobj))
        self.createdConstraint(_om2._node_instances.get(self._constraint))
        MPxCommand.setResult(made)

    def undoIt(self):
        pass

    def redoIt(self):
        pass


def _constraint_syntax(creator):
    """The flags every constraint command has, then the plug-in's own (appendSyntax)."""
    syn = _om2.MSyntax()
    syn.addFlag("-w", "-weight", _om2.MSyntax.kDouble)
    syn.addFlag("-rm", "-remove")
    syn.addFlag("-n", "-name", _om2.MSyntax.kString)
    syn.addFlag("-tl", "-targetList")
    syn.addFlag("-wal", "-weightAliasList")
    syn.addFlag("-lck", "-lock", _om2.MSyntax.kBoolean)
    syn.setObjectType(_om2.MSyntax.kStringObjects, 0)
    syn.enableQuery = True
    syn.enableEdit = True
    probe = creator()
    if isinstance(probe, MPxConstraintCommand):
        probe._syntax_obj = syn
        probe.appendSyntax()
    return syn


# ---------------------------------------------------------------------------------------------------------------------
# IK solvers


MIkHandleGroup = _oma1.MIkHandleGroup


class MPxIkSolverNode(MPxNode):
    """An IK solver of a plug-in's own. While doSolve runs, its handle, effector and joints answer from the solve:
    their channels, matrices and positions as the solve leaves them; the rotations it sets are the answer. Nothing
    else in the scene can be reached then."""

    def __init__(self):
        MPxNode.__init__(self)
        self._group = None
        self._flags = {"rotatePlane": False, "singleChainOnly": False, "positionOnly": False, "supportJointLimits": False, "uniqueSolution": True}
        self._tolerance = 1e-5
        self._iterations = 100

    def type(self):
        return MPxNode.kIkSolverNode

    def preSolve(self):
        pass

    def doSolve(self):
        pass

    def postSolve(self, status=None):
        pass

    def solverTypeName(self):
        return getattr(self, "_type_name", "")

    def handleGroup(self):
        return self._group

    def setHandleGroup(self, group):
        self._group = group

    def rotatePlane(self):
        return self._flags["rotatePlane"]

    def setRotatePlane(self, on):
        self._flags["rotatePlane"] = bool(on)

    def singleChainOnly(self):
        return self._flags["singleChainOnly"]

    def setSingleChainOnly(self, on):
        self._flags["singleChainOnly"] = bool(on)

    def positionOnly(self):
        return self._flags["positionOnly"]

    def setPositionOnly(self, on):
        self._flags["positionOnly"] = bool(on)

    def supportJointLimits(self):
        return self._flags["supportJointLimits"]

    def setSupportJointLimits(self, on):
        self._flags["supportJointLimits"] = bool(on)

    def uniqueSolution(self):
        return self._flags["uniqueSolution"]

    def setUniqueSolution(self, on):
        self._flags["uniqueSolution"] = bool(on)

    def isSingleChainOnly(self):
        return self.singleChainOnly()

    def isPositionOnly(self):
        return self.positionOnly()

    def hasJointLimitSupport(self):
        return self.supportJointLimits()

    def hasUniqueSolution(self):
        return self.uniqueSolution()

    def groupHandlesByTopology(self):
        return True

    def funcValueTolerance(self):
        return self._tolerance

    def setFuncValueTolerance(self, t):
        self._tolerance = float(t)

    def maxIterations(self):
        return self._iterations

    def setMaxIterations(self, n):
        self._iterations = int(n)

    def toWorldSpace(self):
        return _om1.MMatrix()

    def toSolverSpace(self):
        return _om1.MMatrix()


_VEC_ATTRS = {
    "translate": "t", "rotate": "r", "scale": "s", "jointOrient": "jo", "rotateAxis": "ra", "preferredAngle": "pa",
    "poleVector": "pv",
}


class _IkNode(object):
    def __init__(self, j, type_name):
        self.id = j.get("id", -1)
        self.parent = j.get("parent", 0)
        self.path = j.get("path", "")
        self.full = j.get("full", "")
        self.name = j.get("name", "")
        self.type = type_name
        self.t = [0.0, 0.0, 0.0]
        self.jo = [0.0, 0.0, 0.0]
        self.ra = [0.0, 0.0, 0.0]
        self.s = [1.0, 1.0, 1.0]
        self.is_ = [1.0, 1.0, 1.0]
        self.r = [0.0, 0.0, 0.0]
        self.pa = [0.0, 0.0, 0.0]
        self.ro = 0


class _IkScene(object):
    """The handle, its effector and its chain as the host sent them, answering the queries and commands the API's
    classes make of them while a Python solver solves. Values are in stored units (radians), as the API's commands
    are."""

    def __init__(self, a):
        self.nodes = {}
        self.chain = []
        nodes = a.get("nodes") or {}
        for nj, j in zip(nodes.get("joints") or [], a.get("joints") or []):
            n = _IkNode(nj, "joint")
            n.t, n.jo, n.ra = list(j.get("t", n.t)), list(j.get("jo", n.jo)), list(j.get("ra", n.ra))
            n.s, n.is_, n.pa = list(j.get("s", n.s)), list(j.get("is", n.is_)), list(j.get("pa", n.pa))
            n.r = list(n.pa)
            n.ro = int(j.get("ro", 0))
            self.chain.append(n.id)
            self.nodes[n.id] = n
        end = _IkNode(nodes.get("end") or {}, "joint")
        end.t = list(a.get("end") or [0.0, 0.0, 0.0])
        self.nodes[end.id] = end
        self.effector = None
        if isinstance(nodes.get("effector"), dict):
            eff = _IkNode(nodes["effector"], "ikEffector")
            eff.t = list(end.t)
            self.nodes[eff.id] = eff
            self.effector = eff.id
        h = _IkNode(nodes.get("handle") or {}, "ikHandle")
        self.nodes[h.id] = h
        self.handle = h.id
        self.parent_world = _om2.MMatrix(a["parentMatrix"]) if a.get("parentMatrix") else _om2.MMatrix()
        self.handle_world = _om2.MMatrix(a["handleMatrix"]) if a.get("handleMatrix") else _om2.MMatrix()
        self.pole = list(a.get("poleVector") or [0.0, 0.0, 0.0])
        self.twist = float(a.get("twist") or 0.0)
        self.roll = float(a.get("roll") or 0.0)

    def named(self, name):
        for n in self.nodes.values():
            if name in (n.path, n.full, n.name, "|" + n.path):
                return n
        return None

    @staticmethod
    def _t(v):
        return _om2.MMatrix([1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, v[0], v[1], v[2], 1])

    @staticmethod
    def _s(v):
        return _om2.MMatrix([v[0], 0, 0, 0, 0, v[1], 0, 0, 0, 0, v[2], 0, 0, 0, 0, 1])

    def local(self, n):
        if n.type == "ikHandle":
            return self.handle_world * self.parent_of(n).inverse()
        if n.type != "joint":
            return self._t(n.t)
        E = _om2.MEulerRotation
        return (self._s(n.s) * E(n.ra[0], n.ra[1], n.ra[2]).asMatrix() * E(n.r[0], n.r[1], n.r[2], n.ro).asMatrix()
                * E(n.jo[0], n.jo[1], n.jo[2]).asMatrix() * self._s(n.is_) * self._t(n.t))

    def parent_of(self, n):
        p = self.nodes.get(n.parent)
        if p is None:
            return _om2.MMatrix() if n.type == "ikHandle" else self.parent_world
        return self.world(p)

    def world(self, n):
        if n.type == "ikHandle":
            return self.handle_world
        return self.local(n) * self.parent_of(n)

    def _info(self, n):
        return {
            "id": n.id, "name": n.name, "type": n.type, "chain": [n.type, "transform", "dagNode"], "dag": True, "owner": "",
            "path": n.path, "full": n.full, "parent": n.parent, "children": [], "intermediate": False,
        }

    def _value(self, n, attr):
        base, comp = attr, -1
        for l, sh in _VEC_ATTRS.items():
            if attr in (l, sh):
                base = l
            for c, ax in enumerate("XYZ"):
                if attr in (l + ax, sh + ax.lower()):
                    base, comp = l, c
        if base == "rotateOrder" or base == "ro":
            return n.ro
        if n.type == "ikHandle" and attr in ("twist", "twi"):
            return self.twist
        if n.type == "ikHandle" and attr in ("roll", "rol"):
            return self.roll
        v = {
            "translate": n.t, "rotate": n.r, "scale": n.s, "jointOrient": n.jo, "rotateAxis": n.ra, "preferredAngle": n.pa,
            "poleVector": self.pole if n.type == "ikHandle" else None,
        }.get(base)
        if v is None:
            # Pivots and what else the chain doesn't send: at rest.
            if attr.endswith(("Pivot", "PivotTranslate", "rp", "sp", "rpt", "spt")):
                v = [0.0, 0.0, 0.0]
            else:
                raise RuntimeError("(kInvalidParameter): only the handle's and chain's channels answer while a solver solves")
        return v[comp] if comp >= 0 else [tuple(v)]

    def query(self, op, kw):
        if op in ("node.info", "dag.matrices"):
            n = self.nodes.get(kw.get("id"))
            if n is None:
                raise RuntimeError("(kInvalidParameter): only the handle and its chain answer while a solver solves")
            if op == "node.info":
                return self._info(n)
            flat = lambda m: [m[i] for i in range(16)]
            return {"world": flat(self.world(n)), "parent": flat(self.parent_of(n)), "local": flat(self.local(n))}
        if op == "node.find":
            n = self.named(kw.get("name", ""))
            if n is None:
                raise RuntimeError("(kInvalidParameter): No object matches name: %s" % (kw.get("name"),))
            return n.id
        if op == "node.exists":
            return kw.get("id") in self.nodes
        if op == "plug.info":
            node, _, attr = kw.get("name", "").partition(".")
            n = self.named(node)
            if n is None:
                raise RuntimeError("(kInvalidParameter): no such plug while a solver solves")
            v = self._value(n, attr)
            return {
                "long": attr, "short": attr, "type": "vec3" if isinstance(v, list) else "float", "node": n.id, "component": -1,
                "element": -1, "name": kw["name"], "locked": False, "children": [], "childShort": [], "array": False,
                "input": True, "output": False, "keyable": True, "storable": True, "hidden": False, "channelBox": False,
                "user": False, "color": False, "enum": [], "nice": "", "default": None,
            }
        raise RuntimeError("(kFailure): %s can't run while a solver solves" % (op,))

    def command(self, name, args, kwargs):
        if name == "getAttr":
            node, _, attr = str(args[0]).partition(".")
            n = self.named(node)
            if n is None:
                raise RuntimeError("(kInvalidParameter): only the handle's and chain's channels answer while a solver solves")
            return self._value(n, attr)
        if name == "setAttr":
            node, _, attr = str(args[0]).partition(".")
            n = self.named(node)
            v = [float(x) for x in args[1:]]
            comp = {"rotateX": 0, "rx": 0, "rotateY": 1, "ry": 1, "rotateZ": 2, "rz": 2}.get(attr, -1)
            if n is None or n.type != "joint" or (attr not in ("rotate", "r") and comp < 0) or not v:
                raise RuntimeError("(kInvalidParameter): a solver sets only its joints' rotations")
            if comp >= 0:
                n.r[comp] = v[0]
            else:
                n.r = list(v[:3])
            return None
        if name == "ls" and args and not kwargs.get("selection", kwargs.get("sl")):
            n = self.named(str(args[0]))
            return [n.full] if n else []
        if name == "xform" and (kwargs.get("query") or kwargs.get("q")):
            n = self.named(str(args[0]))
            if n is None:
                raise RuntimeError("(kInvalidParameter): a solver can only ask where its handle and joints are")
            world = kwargs.get("worldSpace", kwargs.get("ws"))
            m = self.world(n) if world else self.local(n)
            return [m[12], m[13], m[14]]
        if name == "ikHandle" and (kwargs.get("query") or kwargs.get("q")):
            if kwargs.get("startJoint", kwargs.get("sj")) and self.chain:
                return self.nodes[self.chain[0]].full
            if kwargs.get("endEffector", kwargs.get("ee")) and self.effector is not None:
                return self.nodes[self.effector].full
        raise RuntimeError("(kFailure): %s can't run while a solver solves" % (name,))


_ik_solvers = {}


def _hook_ik_solve(self, type_name, input):
    """A Python solver's solve: its preSolve, doSolve and postSolve on the chain the host sent; the rotates back."""
    e = _om2._node_classes.get(type_name)
    if e is None:
        return ""
    import json

    sc = _IkScene(json.loads(input))
    inst = _ik_solvers.get(type_name)
    if inst is None:
        inst = e["creator"]()
        inst._node_id = 0
        inst._type_name = type_name
        _ik_solvers[type_name] = inst
    inst.setHandleGroup(MIkHandleGroup([sc.handle]))
    _om2._compute_local.ctx = {"query": sc.query, "command": sc.command}
    try:
        inst.preSolve()
        inst.doSolve()
        inst.postSolve()
    except Exception as ex:
        _om2._show_traceback(ex)
        _om2._az.write("# Error: %s: %s #" % (type(ex).__name__, ex), 2)
        return ""
    finally:
        _om2._compute_local.ctx = None
        inst.setHandleGroup(None)
    return json.dumps({"rotate": [sc.nodes[j].r for j in sc.chain]})


_om2._Hooks.ik_solve = _hook_ik_solve


# ---------------------------------------------------------------------------------------------------------------------
# Registration


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

    def registerTransform(self, name, typeId, creator, initialize, matrixCreator=None, matrixId=None, classification=None):
        """A transform and the transformation matrix its createTransformationMatrix makes."""
        kind = _om2._HOST_KINDS[MPxNode.kTransformNode]
        extra = {"matrix_creator": matrixCreator, "matrix_id": _om2.MTypeId(matrixId) if matrixId is not None else None}
        _om2.MFnPlugin._register(self, name, typeId, creator, initialize, kind, extra)

    def matrixTypeIdFromXformId(self, xformId):
        tid = _om2.MTypeId(xformId)
        for e in _om2._node_classes.values():
            if e["id"] == tid and e.get("matrix_id") is not None:
                return _om1.MTypeId(e["matrix_id"])
        return _om1.MTypeId(MPxTransformationMatrix.baseTransformationMatrixId)

    def registerShape(self, name, typeId, creator, initialize, uiCreator=None, classification=None):
        _om2.MFnPlugin.registerShape(self, name, typeId, creator, initialize, uiCreator, classification)

    def registerData(self, name, typeId, creator, dataType=0):
        _om2.MFnPlugin.registerData(self, name, typeId, creator, dataType)

    def deregisterData(self, typeId):
        _om2.MFnPlugin.deregisterData(self, typeId)

    def registerConstraintCommand(self, name, creator):
        """The constraint's command: the flags every constraint command has, then the plug-in's own."""
        syn = _constraint_syntax(creator)
        _om2.MFnPlugin.registerCommand(self, name, creator, lambda: syn)

    def deregisterConstraintCommand(self, name):
        _om2.MFnPlugin.deregisterCommand(self, name)

    def registerIkSolver(self, *args):
        raise RuntimeError("(kFailure): registerIkSolver is obsolete in the reference: register the solver as a node, with MPxNode.kIkSolverNode")

    def deregisterIkSolver(self, *args):
        raise RuntimeError("(kFailure): deregisterIkSolver is obsolete in the reference: deregister the solver node")
