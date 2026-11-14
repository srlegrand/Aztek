# aztek.api.OpenAztek, the scene side: MFn, MObject, MObjectHandle, MDagPath, MPlug, MSelectionList, MGlobal, modifiers.
# An MObject holds a node's id, not its name, so it survives renames and reparenting.
import json as _json
import re as _re
import sys as _sys
import threading as _threading

import _aztek as _az

_script_cmds = _sys.modules["aztek.cmds"]


class _StoredCmds:
    """aztek.cmds in stored units, centimetres and radians, as the API's values are in the reference."""

    def __getattr__(self, name):
        # A compute or a solve answers what it can of its own node itself; the host can't be reached then, not even
        # to look the command up.
        ctx = getattr(_compute_local, "ctx", None)
        if ctx is not None and "command" in ctx:
            return lambda *args, **kwargs: ctx["command"](name, args, kwargs)
        fn = getattr(_script_cmds, name)

        def call(*args, **kwargs):
            was = _az.stored_units(True)
            try:
                return fn(*args, **kwargs)
            finally:
                _az.stored_units(was)

        return call

    # Plug-in commands come and go on the module itself.
    def __setattr__(self, name, value):
        setattr(_script_cmds, name, value)

    def __delattr__(self, name):
        delattr(_script_cmds, name)

    @property
    def __dict__(self):
        return _script_cmds.__dict__

    def __dir__(self):
        return dir(_script_cmds)


_cmds = _StoredCmds()


# Set per thread while a node computes: {"id", "name", "type", "attrs"}. The host can't be reached then, and a compute may
# run on a worker thread alongside others.
_compute_local = _threading.local()


def _api(op, **kw):
    ctx = getattr(_compute_local, "ctx", None)
    if ctx is not None:
        return _compute_answer(ctx, op, kw)
    return _json.loads(_az.api(op, _json.dumps(kw)))


def _compute_answer(ctx, op, kw):
    """What a compute can ask about its own node: its name, type and attributes."""
    if "query" in ctx:
        return ctx["query"](op, kw)
    if op == "node.exists":
        return kw["id"] == ctx["id"]
    if op == "node.info" and kw["id"] == ctx["id"]:
        return {
            "id": ctx["id"], "name": ctx["name"], "type": ctx["type"], "chain": [ctx["type"]], "dag": False, "owner": ctx["owner"],
            "path": ctx["name"], "full": ctx["name"], "parent": 0, "children": [], "intermediate": False,
        }
    if op == "node.find" and kw["name"] == ctx["name"]:
        return ctx["id"]
    if op == "plug.info" and kw["name"].split(".", 1)[0] == ctx["name"]:
        return _compute_plug_info(ctx, kw["name"].split(".", 1)[1])
    raise RuntimeError("(kFailure): Commands cannot run inside compute")


def _compute_plug_info(ctx, attr):
    element = -1
    if attr.endswith("]"):
        attr, _, i = attr[:-1].rpartition("[")
        element = int(i)
    for s in ctx["attrs"]:
        kids = s.get("children") or []
        if s["type"] == "vec3" and not kids:
            kids = [{"name": s["name"] + a, "short": s["short"] + a.lower()} for a in "XYZ"]
        names = [k["name"] for k in kids]
        shorts = [k["short"] for k in kids]
        component = -1
        if attr in names:
            component = names.index(attr)
        elif attr in shorts:
            component = shorts.index(attr)
        elif attr not in (s["name"], s["short"]):
            continue
        return {
            "long": s["name"], "short": s["short"], "type": s["type"], "array": bool(s.get("array")), "input": s.get("input", True),
            "output": not s.get("input", True), "keyable": bool(s.get("keyable")), "storable": s.get("storable", s.get("input", True)),
            "hidden": bool(s.get("hidden")), "channelBox": False, "user": False, "color": bool(s.get("color")), "children": names,
            "childShort": shorts, "enum": s.get("enum") or [], "min": s.get("min"), "max": s.get("max"), "softMin": s.get("softMin"),
            "softMax": s.get("softMax"), "nice": s.get("nice", ""), "default": s.get("default"), "node": ctx["id"], "component": component,
            "element": element, "name": ctx["name"] + "." + attr, "locked": False,
        }
    raise RuntimeError("(kInvalidParameter): The node has no attribute '%s'" % (attr,))


class MFn:
    kInvalid = 0
    kBase = 1
    kNamedObject = 2
    kModel = 3
    kDependencyNode = 4
    kAttribute = 5
    kAnimCurve = 7
    kAnimCurveTimeToAngular = 8
    kAnimCurveTimeToDistance = 9
    kAnimCurveTimeToTime = 10
    kAnimCurveTimeToUnitless = 11
    kAnimCurveUnitlessToAngular = 12
    kAnimCurveUnitlessToDistance = 13
    kAnimCurveUnitlessToTime = 14
    kAnimCurveUnitlessToUnitless = 15
    kAimConstraint = 111
    kOrientConstraint = 239
    kParentConstraint = 242
    kPointConstraint = 240
    kScaleConstraint = 244
    kConstraint = 917
    kBlendShape = 336
    kCluster = 251
    kComponent = 524
    kCurve = 266
    kNurbsCurve = 267
    kCurveCVComponent = 533
    kCurveEPComponent = 534
    kCurveKnotComponent = 535
    kCurveParamComponent = 536
    kDagNode = 107
    kEntity = 108
    kGeometric = 265
    kGeometryFilt = 338
    kIkHandle = 120
    kJoint = 121
    kLocator = 281
    kMesh = 296
    kMeshComponent = 548
    kMeshEdgeComponent = 549
    kMeshPolygonComponent = 550
    kMeshFrEdgeComponent = 551
    kMeshVertComponent = 553
    kMeshVtxFaceComponent = 554
    kMeshMapComponent = 555
    kPluginDependNode = 456
    # Not checked against the reference's numbers: its 2027 documentation doesn't give them.
    kPluginLocatorNode = 457
    kPluginTransformNode = 2001
    kPluginConstraintNode = 2002
    kPluginShape = 2003
    kPluginIkSolver = 2004
    kPluginData = 2005
    kIkEffector = 119
    kNurbsSurface = 294
    kPluginManipContainer = 689
    kPluginNode = 456
    kSet = 464
    kShape = 248
    kSingleIndexedComponent = 538
    kDoubleIndexedComponent = 539
    kTripleIndexedComponent = 540
    kSkinClusterFilter = 682
    kSurface = 293
    kTransform = 110
    kWorld = 247
    kNumericAttribute = 530
    kUnitAttribute = 531
    kTypedAttribute = 532
    kEnumAttribute = 537
    kCompoundAttribute = 541
    kMessageAttribute = 542
    kMatrixAttribute = 543
    kGenericAttribute = 544
    kLightDataAttribute = 545
    kNumericData = 587
    kMatrixData = 588
    kStringData = 589
    kMeshData = 590
    kPolyCube = 433
    kPolySphere = 439
    kPolyCylinder = 435
    kPolyCone = 434
    kPolyPlane = 438
    kPolyTorus = 442
    kPolyTweak = 397
    kPolyPrimitive = 432
    kCamera = 250
    kDisplayLayer = 725
    kTime = 516
    kMultiplyDivide = 301
    kNonLinear = 615
    kTweak = 335
    kLast = 2000


# Node types the API knows by name, with what else each answers to.
_FN_OF_TYPE = {
    "dagNode": MFn.kDagNode,
    "shape": MFn.kShape,
    "transform": MFn.kTransform,
    "joint": MFn.kJoint,
    "mesh": MFn.kMesh,
    "nurbsCurve": MFn.kNurbsCurve,
    "locator": MFn.kLocator,
    "camera": MFn.kCamera,
    "ikHandle": MFn.kIkHandle,
    "ikEffector": MFn.kIkEffector,
    "nurbsSurface": MFn.kNurbsSurface,
    "skinCluster": MFn.kSkinClusterFilter,
    "blendShape": MFn.kBlendShape,
    "cluster": MFn.kCluster,
    "nonLinear": MFn.kNonLinear,
    "objectSet": MFn.kSet,
    "displayLayer": MFn.kDisplayLayer,
    "time": MFn.kTime,
    "polyCube": MFn.kPolyCube,
    "polySphere": MFn.kPolySphere,
    "polyCylinder": MFn.kPolyCylinder,
    "polyCone": MFn.kPolyCone,
    "polyPlane": MFn.kPolyPlane,
    "polyTorus": MFn.kPolyTorus,
    "polyTweak": MFn.kPolyTweak,
    "animCurveTL": MFn.kAnimCurveTimeToDistance,
    "animCurveTA": MFn.kAnimCurveTimeToAngular,
    "animCurveTT": MFn.kAnimCurveTimeToTime,
    "animCurveTU": MFn.kAnimCurveTimeToUnitless,
    "animCurveUL": MFn.kAnimCurveUnitlessToDistance,
    "animCurveUA": MFn.kAnimCurveUnitlessToAngular,
    "animCurveUT": MFn.kAnimCurveUnitlessToTime,
    "animCurveUU": MFn.kAnimCurveUnitlessToUnitless,
    "aimConstraint": MFn.kAimConstraint,
    "orientConstraint": MFn.kOrientConstraint,
    "parentConstraint": MFn.kParentConstraint,
    "pointConstraint": MFn.kPointConstraint,
    "scaleConstraint": MFn.kScaleConstraint,
    "multiplyDivide": MFn.kMultiplyDivide,
}
_FN_BASES = {
    MFn.kMesh: [MFn.kSurface, MFn.kGeometric, MFn.kShape, MFn.kDagNode, MFn.kEntity],
    MFn.kNurbsCurve: [MFn.kCurve, MFn.kGeometric, MFn.kShape, MFn.kDagNode, MFn.kEntity],
    MFn.kLocator: [MFn.kShape, MFn.kDagNode, MFn.kEntity],
    MFn.kCamera: [MFn.kShape, MFn.kDagNode, MFn.kEntity],
    MFn.kShape: [MFn.kDagNode, MFn.kEntity],
    MFn.kJoint: [MFn.kTransform, MFn.kDagNode, MFn.kEntity],
    MFn.kIkHandle: [MFn.kTransform, MFn.kDagNode, MFn.kEntity],
    MFn.kIkEffector: [MFn.kTransform, MFn.kDagNode, MFn.kEntity],
    MFn.kNurbsSurface: [MFn.kSurface, MFn.kGeometric, MFn.kShape, MFn.kDagNode, MFn.kEntity],
    MFn.kTransform: [MFn.kDagNode, MFn.kEntity],
    MFn.kDagNode: [MFn.kEntity],
    MFn.kSkinClusterFilter: [MFn.kGeometryFilt],
    MFn.kBlendShape: [MFn.kGeometryFilt],
    MFn.kCluster: [MFn.kGeometryFilt],
    MFn.kAnimCurveTimeToDistance: [MFn.kAnimCurve],
    MFn.kAnimCurveTimeToAngular: [MFn.kAnimCurve],
    MFn.kAnimCurveTimeToTime: [MFn.kAnimCurve],
    MFn.kAnimCurveTimeToUnitless: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToDistance: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToAngular: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToTime: [MFn.kAnimCurve],
    MFn.kAnimCurveUnitlessToUnitless: [MFn.kAnimCurve],
    MFn.kAimConstraint: [MFn.kConstraint],
    MFn.kOrientConstraint: [MFn.kConstraint],
    MFn.kParentConstraint: [MFn.kConstraint],
    MFn.kPointConstraint: [MFn.kConstraint],
    MFn.kScaleConstraint: [MFn.kConstraint],
    MFn.kPolyCube: [MFn.kPolyPrimitive],
    MFn.kPolySphere: [MFn.kPolyPrimitive],
    MFn.kPolyCylinder: [MFn.kPolyPrimitive],
    MFn.kPolyCone: [MFn.kPolyPrimitive],
    MFn.kPolyPlane: [MFn.kPolyPrimitive],
    MFn.kPolyTorus: [MFn.kPolyPrimitive],
    MFn.kMeshVertComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshEdgeComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshPolygonComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshMapComponent: [MFn.kMeshComponent, MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kMeshVtxFaceComponent: [MFn.kMeshComponent, MFn.kDoubleIndexedComponent, MFn.kComponent],
    MFn.kCurveCVComponent: [MFn.kSingleIndexedComponent, MFn.kComponent],
    MFn.kCurveEPComponent: [MFn.kSingleIndexedComponent, MFn.kComponent],
}
_FN_NAMES = {v: k for k, v in reversed(list(MFn.__dict__.items())) if k.startswith("k") and isinstance(v, int)}

# Components by the name they go by in strings.
_COMP_TYPES = {
    "vtx": MFn.kMeshVertComponent,
    "e": MFn.kMeshEdgeComponent,
    "f": MFn.kMeshPolygonComponent,
    "map": MFn.kMeshMapComponent,
    "vtxFace": MFn.kMeshVtxFaceComponent,
    "cv": MFn.kCurveCVComponent,
    "ep": MFn.kCurveEPComponent,
}
_COMP_NAMES = {v: k for k, v in _COMP_TYPES.items()}
_COMP_RE = _re.compile(r"^(.+)\.(vtxFace|vtx|map|cv|ep|e|f)\[([^\]]*)\](?:\[([^\]]*)\])?$")

# Rotation attributes, by every spelling.
_ANGLE_ATTRS = {
    "rotate", "rotateX", "rotateY", "rotateZ", "r", "rx", "ry", "rz",
    "jointOrient", "jointOrientX", "jointOrientY", "jointOrientZ", "jo", "jox", "joy", "joz",
    "rotateAxis", "rotateAxisX", "rotateAxisY", "rotateAxisZ", "ra", "rax", "ray", "raz",
}
# One-element arrays in the reference that are single plugs here.
_ONE_ELEMENT = {"worldMatrix", "wm", "parentMatrix", "pm", "worldInverseMatrix", "wim", "parentInverseMatrix", "pim", "worldSpace", "ws"}


def _fn_chain(info):
    fns = []
    for t in info["chain"]:
        f = _FN_OF_TYPE.get(t)
        if f is not None and f not in fns:
            fns.append(f)
            for b in _FN_BASES.get(f, []):
                if b not in fns:
                    fns.append(b)
    if info.get("owner"):
        fns.insert(0, MFn.kPluginDependNode)
        # A plug-in's node is what its kind says, before what it is built on.
        chain = info["chain"]
        kind = _node_classes.get(chain[0], {}).get("kind") if chain else None
        base = chain[1] if len(chain) > 1 else ""
        if kind == 4:
            fns.insert(0, MFn.kPluginConstraintNode)
        elif kind == 6:
            fns.insert(0, MFn.kPluginIkSolver)
        elif base == "transform":
            fns.insert(0, MFn.kPluginTransformNode)
        elif base == "shape":
            fns.insert(0, MFn.kPluginShape)
        elif base == "locator":
            fns.insert(0, MFn.kPluginLocatorNode)
    return fns


class MObject:
    """A handle on a node, an attribute, a component or a data value."""

    __slots__ = ("_kind", "_id", "_attr", "_comp", "_data", "_fn")

    def __init__(self, src=None):
        self._kind = None
        self._id = 0
        self._attr = None
        self._comp = None
        self._data = None
        self._fn = None
        if isinstance(src, MObject):
            self._kind, self._id, self._attr, self._comp, self._data, self._fn = src._kind, src._id, src._attr, src._comp, src._data, src._fn

    def isNull(self):
        return self._kind is None

    def _fns(self):
        k = self._kind
        if k == "node":
            info = _api("node.info", id=self._id)
            return _fn_chain(info) if info else []
        if k == "world":
            return [MFn.kWorld]
        if k == "attr":
            return [self._fn, MFn.kAttribute]
        if k == "comp":
            return [self._fn] + _FN_BASES.get(self._fn, [MFn.kComponent])
        if k in ("data", "plugin"):
            return [self._fn]
        return []

    def apiType(self):
        if self._kind is None:
            return MFn.kInvalid
        fns = self._fns()
        return fns[0] if fns else MFn.kDependencyNode

    @property
    def apiTypeStr(self):
        return _FN_NAMES.get(self.apiType(), "kInvalid")

    def hasFn(self, fn):
        if fn in (MFn.kBase, MFn.kNamedObject):
            return self._kind is not None
        if fn == MFn.kDependencyNode:
            return self._kind == "node"
        return fn in self._fns()

    def __eq__(self, o):
        if not isinstance(o, MObject) or self._kind != o._kind:
            return False
        k = self._kind
        if k == "node":
            return self._id == o._id
        if k == "attr":
            return _attr_name(self) == _attr_name(o)
        if k == "comp":
            return self._fn == o._fn and self._comp == o._comp
        if k == "data":
            return self._data is o._data
        return True

    def __ne__(self, o):
        return not self == o

    def __hash__(self):
        return hash((self._kind, self._id, _attr_name(self) if self._kind == "attr" else None))

    def __repr__(self):
        return "<aztek.api.OpenAztek.MObject %s>" % (self.apiTypeStr,)


MObject.kNullObj = MObject()


def _node_object(id):
    o = MObject()
    o._kind, o._id = "node", id
    return o


def _world_object():
    o = MObject()
    o._kind = "world"
    return o


def _check_node(obj):
    if not isinstance(obj, MObject) or obj._kind != "node" or not _api("node.exists", id=obj._id):
        _fail()
    return obj._id


def _node_id_named(name):
    try:
        id = _api("node.find", name=name)
    except RuntimeError as e:
        _fail(message=str(e))
    return id


def _shape_below(id, type_name=None):
    """The first shape of a transform (intermediates last); the node itself when it is a shape."""
    info = _api("node.info", id=id)
    if not info or "transform" not in info["chain"]:
        return id
    later = None
    for c in info["children"]:
        k = _api("node.info", id=c)
        if "transform" in k["chain"] or not k["dag"] or (type_name and type_name not in k["chain"]):
            continue
        if not k["intermediate"]:
            return c
        later = later or c
    return later or id


class MObjectHandle:
    def __init__(self, obj=None):
        self._o = MObject(obj) if obj is not None else MObject()

    def assign(self, obj):
        self._o = MObject(obj)
        return self

    def object(self):
        return self._o

    def isAlive(self):
        return self._o._kind is not None and (self._o._kind != "node" or _api("node.exists", id=self._o._id))

    isValid = isAlive

    def hashCode(self):
        return hash(self._o) & 0xFFFFFFFF

    def __eq__(self, o):
        return isinstance(o, MObjectHandle) and self._o == o._o

    def __ne__(self, o):
        return not self == o

    __hash__ = None


class MDagPath:
    kInvalid = 0

    def __init__(self, src=None):
        self._nodes = list(src._nodes) if isinstance(src, MDagPath) else []

    @staticmethod
    def _of(id):
        p = MDagPath()
        chain = []
        while id:
            chain.append(id)
            id = _api("node.info", id=id)["parent"]
        p._nodes = chain[::-1]
        return p

    @staticmethod
    def getAPathTo(obj):
        id = _check_node(obj)
        if not _api("node.info", id=id)["dag"]:
            _fail()
        return MDagPath._of(id)

    @staticmethod
    def getAllPathsTo(obj):
        return MDagPathArray([MDagPath.getAPathTo(obj)])

    def _leaf(self):
        if not self.isValid():
            _fail(message="Object does not exist")
        return self._nodes[-1]

    def isValid(self):
        return bool(self._nodes) and all(_api("node.exists", id=i) for i in self._nodes[-1:])

    def fullPathName(self):
        return _api("node.info", id=self._leaf())["full"]

    def partialPathName(self):
        return _api("node.info", id=self._leaf())["path"]

    def node(self):
        return _node_object(self._leaf())

    def transform(self):
        for id in reversed(self._nodes):
            if "transform" in _api("node.info", id=id)["chain"]:
                return _node_object(id)
        _fail(message="Object does not exist")

    def apiType(self):
        return self.node().apiType()

    def hasFn(self, fn):
        return self.node().hasFn(fn)

    def length(self):
        return len(self._nodes)

    def isInstanced(self):
        return False

    def instanceNumber(self):
        return 0

    def getPath(self):
        return MDagPath(self)

    def set(self, other):
        self._nodes = list(other._nodes)
        return self

    def _matrices(self):
        return _api("dag.matrices", id=self._leaf())

    def inclusiveMatrix(self):
        return MMatrix(self._matrices()["world"])

    def exclusiveMatrix(self):
        return MMatrix(self._matrices()["parent"])

    def inclusiveMatrixInverse(self):
        return self.inclusiveMatrix().inverse()

    def exclusiveMatrixInverse(self):
        return self.exclusiveMatrix().inverse()

    def _shapes(self):
        out = []
        for c in _api("node.info", id=self._leaf())["children"]:
            k = _api("node.info", id=c)
            if "shape" in k["chain"]:
                out.append(c)
        return out

    def numberOfShapesDirectlyBelow(self):
        return len(self._shapes())

    def extendToShape(self, index=0):
        if "shape" in _api("node.info", id=self._leaf())["chain"]:
            return self
        shapes = self._shapes()
        if not shapes or index >= len(shapes):
            _fail(status="kFailure", message="Object has no shape below it")
        self._nodes.append(shapes[index])
        return self

    extendToShapeDirectlyBelow = extendToShape

    def childCount(self):
        return len(_api("node.info", id=self._leaf())["children"])

    def child(self, i):
        kids = _api("node.info", id=self._leaf())["children"]
        if i >= len(kids):
            raise IndexError("MDagPath child index out of range")
        return _node_object(kids[i])

    def push(self, obj):
        self._nodes.append(_check_node(obj))
        return self

    def pop(self, num=1):
        if num > len(self._nodes):
            _fail()
        del self._nodes[len(self._nodes) - num :]
        return self

    def isVisible(self):
        for id in self._nodes:
            n = _api("node.info", id=id)["path"]
            if not _cmds.getAttr(n + ".visibility"):
                return False
        return True

    def __eq__(self, o):
        # By where the paths lead now: a path follows its node through renames and reparenting.
        if not isinstance(o, MDagPath):
            return False
        if self._nodes == o._nodes:
            return True
        try:
            return self.fullPathName() == o.fullPathName()
        except RuntimeError:
            return False

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __str__(self):
        return self.partialPathName() if self._nodes else ""

    def __repr__(self):
        return "aztek.api.OpenAztek.MDagPath(%s)" % (self.fullPathName() if self._nodes else "")


class MDagPathArray(_Array):
    _item = staticmethod(lambda x: MDagPath(x))
    _blank = staticmethod(MDagPath)


class MObjectArray(_Array):
    _item = staticmethod(lambda x: MObject(x))
    _blank = staticmethod(MObject)


def _attr_fn(type_name, long=""):
    if long in _ANGLE_ATTRS:
        return MFn.kUnitAttribute
    return {
        "enum": MFn.kEnumAttribute,
        "matrix": MFn.kMatrixAttribute,
        "string": MFn.kTypedAttribute,
        "mesh": MFn.kTypedAttribute,
        "data": MFn.kTypedAttribute,
        "vec3": MFn.kNumericAttribute,
    }.get(type_name, MFn.kNumericAttribute)


def _attr_object(node, long, type_name):
    o = MObject()
    o._kind = "attr"
    o._attr = long
    o._fn = _attr_fn(type_name, long)
    # The node it came from, so MFnAttribute can read the attribute's flags.
    o._data = node
    return o


def _split_plug(name):
    """(node id, attr path) of node.attr."""
    node, _, attr = name.partition(".")
    id = _node_id_named(node)
    if not id:
        _fail(message="Object does not exist")
    return id, attr


class MPlug:
    kAll = 0
    kNonDefault = 1
    kChanged = 2
    kLastAttrSelector = 3
    kFreeToChange = 0
    kNotFreeToChange = 1
    kChildrenNotFreeToChange = 2

    def __init__(self, a=None, b=None):
        self._node = 0
        self._attr = None
        if isinstance(a, MPlug):
            self._node, self._attr = a._node, a._attr
        elif a is not None:
            self._node = _check_node(a)
            self._attr = _attr_name(b)
            if self._attr is None:
                _fail()
            self._info()

    @staticmethod
    def _of(id, attr):
        p = MPlug()
        p._node, p._attr = id, attr
        return p

    @staticmethod
    def _named(name):
        return MPlug._of(*_split_plug(name))

    def _nodename(self):
        info = _api("node.info", id=self._node) if self._node else None
        if not info:
            _fail(message="Plug is not valid")
        return info["path"]

    def _host_attr(self):
        return self._attr

    def _hostname(self):
        return self._nodename() + "." + self._host_attr()

    def _info(self):
        try:
            d = _api("plug.info", name=self._hostname())
        except RuntimeError:
            _fail(message="Cannot find plug or attribute named '%s'" % (self._attr,))
        c = d["component"]
        if c >= 0:
            d["parent"] = d["long"]
            d["long"], d["short"] = d["children"][c], d["childShort"][c]
            d["children"], d["childShort"] = [], []
            d["type"] = "float"
        return d

    def _leaf_attr(self):
        return self._attr.rsplit("[", 1)[0] if self._attr.endswith("]") else self._attr

    def _is_one_element(self):
        return self._leaf_attr() in _ONE_ELEMENT

    def isNull(self):
        return not self._node or self._attr is None

    # Names

    def name(self):
        if self.isNull():
            return ""
        i = self._info()
        attr = self._attr if "." in self._attr or self._attr.endswith("]") else i["long"]
        return _api("node.info", id=i["node"])["path"] + "." + attr

    @property
    def info(self):
        return self.name()

    def partialName(self, includeNodeName=False, includeNonMandatoryIndices=False, includeInstancedIndices=False, useAlias=False, useFullAttributePath=False, useLongNames=False):
        i = self._info()
        base = i["long"] if useLongNames else (i["short"] or i["long"])
        if self._attr.endswith("]"):
            base += "[" + self._attr.rsplit("[", 1)[1]
        if includeNodeName:
            return _api("node.info", id=i["node"])["name"] + "." + base
        return base

    def node(self):
        return _node_object(self._info()["node"])

    def attribute(self):
        i = self._info()
        return _attr_object(i["node"], i["long"], i["type"])

    # Shape

    def isArray(self):
        if self._attr.endswith("]"):
            return False
        return self._info()["array"] or self._is_one_element()

    def isElement(self):
        return self._attr.endswith("]")

    def isCompound(self):
        return bool(self._info()["children"])

    def isChild(self):
        return "parent" in self._info()

    def numChildren(self):
        return len(self._info()["children"])

    def child(self, index):
        kids = self._info()["children"]
        if isinstance(index, MObject):
            n = _attr_name(index)
            for k, s in zip(kids, self._info()["childShort"]):
                if n in (k, s):
                    return MPlug._of(self._node, k)
            _fail(message="Attribute is not a child of this plug")
        if not 0 <= index < len(kids):
            raise IndexError("MPlug child index out of range")
        return MPlug._of(self._node, kids[index])

    def parent(self):
        p = self._info().get("parent")
        if not p:
            _fail(message="Plug is not a child")
        return MPlug._of(self._node, p)

    def array(self):
        if not self._attr.endswith("]"):
            _fail(message="Plug is not an array element")
        return MPlug._of(self._node, self._leaf_attr())

    def elementByLogicalIndex(self, index):
        if not self.isArray():
            _fail(message="Plug is not an array")
        return MPlug._of(self._node, "%s[%d]" % (self._attr, index))

    def _indices(self):
        if self._is_one_element():
            return [0]
        try:
            return _api("plug.elements", name=self._hostname())
        except RuntimeError:
            return []

    def elementByPhysicalIndex(self, index):
        ids = self._indices()
        if not 0 <= index < len(ids):
            raise IndexError("MPlug physical index out of range")
        return self.elementByLogicalIndex(ids[index])

    def numElements(self):
        if not self.isArray():
            _fail(message="Plug is not an array")
        return len(self._indices())

    evaluateNumElements = numElements

    def getExistingArrayAttributeIndices(self):
        return MIntArray(self._indices())

    def logicalIndex(self):
        if not self.isElement():
            _fail(message="Plug is not an array element")
        return int(self._attr.rsplit("[", 1)[1][:-1])

    # State

    def isConnected(self):
        c = _api("plug.connections", name=self._hostname())
        return bool(c["source"] or c["dests"])

    def isDestination(self):
        return bool(_api("plug.connections", name=self._hostname())["source"])

    def isSource(self):
        return bool(_api("plug.connections", name=self._hostname())["dests"])

    def isKeyable(self):
        return self._info()["keyable"]

    def isChannelBox(self):
        return self._info()["channelBox"]

    def isLocked(self):
        return self._info()["locked"]

    def isDynamic(self):
        return self._info()["user"]

    def isNetworked(self):
        return self.isConnected()

    def isFromReferencedFile(self):
        return False

    def isDefaultValue(self):
        v = self._value()
        return v == self._info()["default"]

    def setKeyable(self, on):
        _api("plug.setFlag", name=self._hostname(), flag="keyable", on=bool(on))
        return self

    def setChannelBox(self, on):
        _api("plug.setFlag", name=self._hostname(), flag="channelBox", on=bool(on))
        return self

    def setLocked(self, on):
        _cmds.setAttr(self._hostname(), lock=bool(on))
        return self

    # Connections

    def source(self):
        c = _api("plug.connections", name=self._hostname())
        return MPlug._named(c["source"]) if c["source"] else MPlug()

    sourceWithConversion = source

    def destinations(self):
        c = _api("plug.connections", name=self._hostname())
        return MPlugArray([MPlug._named(d) for d in c["dests"]])

    destinationsWithConversions = destinations

    def connectedTo(self, asDst, asSrc):
        c = _api("plug.connections", name=self._hostname())
        out = MPlugArray()
        if asDst and c["source"]:
            out.append(MPlug._named(c["source"]))
        if asSrc:
            for d in c["dests"]:
                out.append(MPlug._named(d))
        return out

    # Values

    def _value(self):
        v = _cmds.getAttr(self._hostname())
        if isinstance(v, list) and len(v) == 1 and isinstance(v[0], tuple):
            return list(v[0])
        return v

    def _is_angle(self):
        i = self._info()
        return i["long"] in _ANGLE_ATTRS or (self._node, i["long"]) in _PLUGIN_ANGLES or _plugin_attr_is_angle(self._node, i["long"])

    def _number(self):
        v = self._value()
        if isinstance(v, (list, str)) or v is None:
            _fail(message="Plug is not numeric")
        return v

    def asDouble(self, *a):
        v = float(self._number())
        return v

    def asFloat(self, *a):
        return _f32(self.asDouble())

    def asInt(self, *a):
        return int(self.asDouble())

    asShort = asInt
    asChar = asInt

    def asBool(self, *a):
        return bool(self._number())

    def asString(self, *a):
        v = self._value()
        if isinstance(v, str):
            return v
        if isinstance(v, (int, float)) and not isinstance(v, bool) and self._info()["type"] == "enum":
            names = self._info()["enum"]
            return names[int(v)] if 0 <= int(v) < len(names) else ""
        return "" if v is None else str(v)

    def asMAngle(self, *a):
        return MAngle(self.asDouble(), MAngle.kRadians)

    def asMDistance(self, *a):
        return MDistance(float(self._number()), MDistance.kCentimeters)

    def asMTime(self, *a):
        return MTime(float(self._number()))

    def asMObject(self, *a):
        i = self._info()
        v = self._value()
        o = MObject()
        o._kind = "data"
        o._data = v
        if i["type"] == "matrix":
            o._fn = MFn.kMatrixData
            o._data = MMatrix(v)
        elif i["type"] == "string":
            o._fn = MFn.kStringData
        elif i["type"] == "vec3":
            o._fn = MFn.kNumericData
        elif i["type"] in ("mesh", "data"):
            o._fn = MFn.kMeshData if i["type"] == "mesh" else MFn.kNumericData
        else:
            o._fn = MFn.kNumericData
        return o

    def _set(self, v):
        _cmds.setAttr(self._hostname(), v)
        return self

    def setDouble(self, v):
        return self._set(float(v))

    setFloat = setDouble

    def setInt(self, v):
        return self._set(int(v))

    setShort = setInt
    setChar = setInt

    def setBool(self, v):
        return self._set(bool(v))

    def setString(self, v):
        return self._set(str(v))

    def setMAngle(self, a):
        return self._set(a.asRadians())

    def setMDistance(self, d):
        return self._set(d.asCentimeters())

    def setMTime(self, t):
        return self._set(t.asUnits(MTime.uiUnit()))

    def setMObject(self, o):
        if o._kind != "data":
            _fail()
        d = o._data
        if isinstance(d, MMatrix):
            d = list(d)
        return self._set(d)

    def __eq__(self, o):
        if isinstance(o, MObject):
            # plug == attribute, as compute methods write it.
            n = _attr_name(o)
            if n is None or self.isNull():
                return False
            i = self._info()
            return n in (i["long"], i["short"])
        if not isinstance(o, MPlug) or self._node != o._node:
            return False
        if self._attr == o._attr:
            return True
        # tx and translateX are one plug.
        if self.isNull() or o.isNull():
            return False
        a, b = self._info(), o._info()
        return a["long"] == b["long"] and a["element"] == b["element"]

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __str__(self):
        return self.name()

    def __repr__(self):
        return "aztek.api.OpenAztek.MPlug(%s)" % (self.name() if not self.isNull() else "")



class _Flag(int):
    """A plug state as API 2.0 gives it, a property (`plug.isChild`), that can still be called as API 1.0's method
    is (`plug.isChild()`)."""

    def __new__(cls, on):
        return int.__new__(cls, bool(on))

    def __call__(self):
        return bool(self)

    def __repr__(self):
        return repr(bool(self))

    __str__ = __repr__


def _flag_property(fn):
    return property(lambda self: _Flag(fn(self)), doc=fn.__doc__)


# The reference's API 2.0 has these as properties of MPlug, API 1.0 as methods.
for _n in ("isNull", "isArray", "isElement", "isCompound", "isChild", "isConnected", "isDestination", "isSource",
           "isKeyable", "isChannelBox", "isLocked", "isDynamic", "isNetworked", "isFromReferencedFile"):
    setattr(MPlug, _n, _flag_property(getattr(MPlug, _n)))
MPlug.isProcedural = property(lambda self: _Flag(False))
MPlug.isIgnoredWhenRendering = property(lambda self: _Flag(False))
MPlug.isCaching = property(lambda self: _Flag(False))
del _n

class MPlugArray(_Array):
    _item = staticmethod(lambda x: MPlug(x))
    _blank = staticmethod(MPlug)


# Plugin attributes in degrees, filled by om_plugin: (type name, attr).
_PLUGIN_ANGLES = set()


def _plugin_attr_is_angle(node, long):
    if not _PLUGIN_ANGLES:
        return False
    info = _api("node.info", id=node)
    return bool(info) and (info["type"], long) in _PLUGIN_ANGLES


def _attr_name(a):
    if isinstance(a, str):
        return a
    if isinstance(a, MObject) and a._kind == "attr":
        return a._attr if isinstance(a._attr, str) else a._attr["name"]
    return None


def _parse_ids(text):
    """Indices from 0:3,7 ; None for *."""
    out = []
    for part in text.split(","):
        part = part.strip()
        if part == "*":
            return None
        if ":" in part:
            a, b = part.split(":")
            out.extend(range(int(a), int(b) + 1))
        else:
            out.append(int(part))
    return out


def _compress_ids(ids):
    ids = sorted(set(ids))
    parts = []
    i = 0
    while i < len(ids):
        j = i
        while j + 1 < len(ids) and ids[j + 1] == ids[j] + 1:
            j += 1
        parts.append((ids[i], ids[j]))
        i = j + 1
    return parts


def _component_object(fn, ids):
    o = MObject()
    o._kind = "comp"
    o._fn = fn
    o._comp = list(ids)
    return o


def _component_count(shape, fn):
    if fn in (MFn.kCurveCVComponent, MFn.kCurveEPComponent):
        d = _api("curve.data", id=shape)
        return d["spans"] + (0 if d["form"] == 2 else d["degree"]) if fn == MFn.kCurveCVComponent else d["spans"] + 1
    m = _mesh_data(shape)
    return {
        MFn.kMeshVertComponent: m.num_vertices,
        MFn.kMeshEdgeComponent: m.num_edges,
        MFn.kMeshPolygonComponent: m.num_faces,
        MFn.kMeshMapComponent: m.num_uvs,
    }.get(fn, 0)


class _Item:
    """An entry of a selection list: a node, a plug or a node's components."""

    __slots__ = ("node", "plug", "comp")

    def __init__(self, node=0, plug=None, comp=None):
        self.node = node
        self.plug = plug
        self.comp = comp

    def same(self, o):
        if self.node != o.node:
            return False
        if self.plug is not None or o.plug is not None:
            return self.plug == o.plug
        if (self.comp is None) != (o.comp is None):
            return False
        return self.comp is None or (self.comp._fn == o.comp._fn and sorted(self.comp._comp) == sorted(o.comp._comp))

    def string(self):
        info = _api("node.info", id=self.node)
        if self.plug is not None:
            return self.plug.name()
        if self.comp is None:
            return info["path"]
        owner = info["path"]
        if info["parent"] and "shape" in info["chain"]:
            owner = _api("node.info", id=info["parent"])["path"]
        word = _COMP_NAMES[self.comp._fn]
        if self.comp._fn == MFn.kMeshVtxFaceComponent:
            return ["%s.%s[%d][%d]" % (owner, word, v, f) for v, f in self.comp._comp]
        return [
            "%s.%s[%s]" % (owner, word, str(a) if a == b else "%d:%d" % (a, b))
            for a, b in _compress_ids(self.comp._comp)
        ]


def _parse_item(text):
    """The items a string names: a component, a plug, or nodes (patterns allowed)."""
    m = _COMP_RE.match(text)
    if m:
        id = _node_id_named(m.group(1))
        if not id:
            _fail(message="Object does not exist")
        fn = _COMP_TYPES[m.group(2)]
        shape = _shape_below(id, "nurbsCurve" if m.group(2) in ("cv", "ep") else "mesh")
        if m.group(4) is not None:
            vs, fs = _parse_ids(m.group(3)), _parse_ids(m.group(4))
            return [_Item(shape, comp=_component_object(fn, [(v, f) for v in vs or [] for f in fs or []]))]
        ids = _parse_ids(m.group(3))
        if ids is None:
            ids = list(range(_component_count(shape, fn)))
        return [_Item(shape, comp=_component_object(fn, ids))]
    if "." in text:
        node, _, attr = text.partition(".")
        id = _node_id_named(node)
        if id:
            try:
                plug = MPlug._of(id, attr)
                plug._info()
                return [_Item(id, plug=plug)]
            except RuntimeError:
                pass
        _fail(message="Object does not exist")
    if "*" in text or "?" in text:
        found = _cmds.ls(text)
        if not found:
            _fail(message="Object does not exist")
        return [_Item(_node_id_named(n)) for n in found]
    id = None
    try:
        id = _api("node.find", name=text)
    except RuntimeError as e:
        _fail(message=str(e))
    if not id:
        _fail(message="Object does not exist")
    return [_Item(id)]


class MSelectionList:
    kMergeNormal = 0
    kXORWithList = 1
    kRemoveFromList = 2

    def __init__(self, src=None):
        self._items = list(src._items) if isinstance(src, MSelectionList) else []

    def _at(self, i):
        if not 0 <= i < len(self._items):
            raise IndexError("MSelectionList index out of range")
        return self._items[i]

    def _add_item(self, item, merge):
        for e in self._items:
            if e.same(item):
                return
            if merge and e.comp is not None and item.comp is not None and e.node == item.node and e.comp._fn == item.comp._fn:
                e.comp._comp = sorted(set(e.comp._comp) | set(item.comp._comp))
                return
        self._items.append(item)

    def _items_of(self, item):
        if isinstance(item, str):
            return _parse_item(item)
        if isinstance(item, MObject):
            return [_Item(_check_node(item))]
        if isinstance(item, MDagPath):
            return [_Item(item._leaf())]
        if isinstance(item, MPlug):
            return [_Item(item._node, plug=MPlug(item))]
        if isinstance(item, (tuple, list)) and len(item) == 2:
            path, comp = item
            leaf = path._leaf() if isinstance(path, MDagPath) else _check_node(path)
            if comp is None or comp.isNull():
                return [_Item(leaf)]
            return [_Item(_shape_below(leaf), comp=_component_object(comp._fn, comp._comp))]
        _fail()

    def add(self, item, mergeWithExisting=True):
        for i in self._items_of(item):
            self._add_item(i, mergeWithExisting)
        return self

    def length(self):
        return len(self._items)

    def __len__(self):
        return len(self._items)

    def isEmpty(self):
        return not self._items

    def clear(self):
        self._items = []
        return self

    def merge(self, *args):
        """merge(list, strategy) or merge(dagPath, component, strategy)."""
        strategy = MSelectionList.kMergeNormal
        if isinstance(args[0], MSelectionList):
            items = list(args[0]._items)
            if len(args) > 1:
                strategy = args[1]
        else:
            items = self._items_of((args[0], args[1] if len(args) > 1 else None))
            if len(args) > 2:
                strategy = args[2]
        for it in items:
            found = [e for e in self._items if e.same(it)]
            if strategy == MSelectionList.kRemoveFromList or (strategy == MSelectionList.kXORWithList and found):
                self._items = [e for e in self._items if not e.same(it)]
            elif not found:
                self._add_item(_Item(it.node, it.plug, it.comp), True)
        return self

    def remove(self, index):
        self._at(index)
        del self._items[index]
        return self

    def replace(self, index, item):
        self._at(index)
        self._items[index] = self._items_of(item)[0]
        return self

    def toggle(self, path, component=None):
        return self.merge(path, component, MSelectionList.kXORWithList)

    def hasItem(self, item):
        its = self._items_of(item)
        return all(any(e.same(i) for e in self._items) for i in its)

    def hasItemPartly(self, path, component):
        its = self._items_of((path, component))
        for i in its:
            for e in self._items:
                if e.node == i.node and e.comp is not None and e.comp._fn == i.comp._fn and set(e.comp._comp) & set(i.comp._comp):
                    return True
        return False

    def getDagPath(self, index):
        it = self._at(index)
        if not _api("node.info", id=it.node)["dag"]:
            _fail()
        return MDagPath._of(it.node)

    def getDependNode(self, index):
        return _node_object(self._at(index).node)

    def getPlug(self, index):
        it = self._at(index)
        if it.plug is None:
            _fail()
        return MPlug(it.plug)

    def getComponent(self, index):
        it = self._at(index)
        if not _api("node.info", id=it.node)["dag"]:
            _fail()
        path = MDagPath._of(it.node)
        return (path, _component_object(it.comp._fn, it.comp._comp) if it.comp is not None else MObject())

    def getSelectionStrings(self, index=None):
        items = self._items if index is None else [self._at(index)]
        out = []
        for it in items:
            s = it.string()
            out.extend(s if isinstance(s, list) else [s])
        return out

    def copy(self, src):
        self._items = list(src._items)
        return self


class MGlobal:
    kReplaceList = 0
    kXORWithList = 1
    kAddToList = 2
    kRemoveFromList = 3
    kAddToHeadOfList = 4
    kInteractive = 0
    kBatch = 1
    kLibraryApp = 2
    kBaseUIMode = 3
    kSelectObjectMode = 0
    kSelectComponentMode = 1

    @staticmethod
    def mayaState():
        return MGlobal.kInteractive

    @staticmethod
    def mayaVersion():
        return "2024"

    @staticmethod
    def apiVersion():
        return 20240000

    @staticmethod
    def getActiveSelectionList(orderedSelectionIfAvailable=False):
        out = MSelectionList()
        for s in _cmds.ls(sl=True) or []:
            out.add(s)
        return out

    @staticmethod
    def setActiveSelectionList(sel, listAdjustment=0):
        names = sel.getSelectionStrings()
        MGlobal._select(names, listAdjustment)

    @staticmethod
    def _select(names, mode):
        if mode == MGlobal.kReplaceList:
            # Replacing with objects leaves component mode, or the components stay selected as well.
            if not any(_COMP_RE.match(n) for n in names):
                _cmds.selectMode("object")
            if names:
                _cmds.select(names)
            else:
                _cmds.select(clear=True)
        elif names:
            if mode == MGlobal.kXORWithList:
                _cmds.select(names, toggle=True)
            elif mode == MGlobal.kRemoveFromList:
                _cmds.select(names, deselect=True)
            else:
                _cmds.select(names, add=True)

    @staticmethod
    def selectByName(pattern, listAdjustment=0):
        found = _cmds.ls(pattern)
        if not found:
            _fail(message="Object does not exist")
        MGlobal._select(found, listAdjustment)

    @staticmethod
    def select(obj, comp=None, listAdjustment=0):
        s = MSelectionList()
        if isinstance(obj, MSelectionList):
            s = obj
        elif comp is not None:
            s.add((obj, comp))
        else:
            s.add(obj)
        MGlobal._select(s.getSelectionStrings(), listAdjustment)

    @staticmethod
    def clearSelectionList():
        _cmds.select(clear=True)

    @staticmethod
    def getSelectionListByName(pattern):
        s = MSelectionList()
        s.add(pattern)
        return s

    @staticmethod
    def displayInfo(msg):
        _az.write(str(msg), 3)

    @staticmethod
    def displayWarning(msg):
        _az.write("# Warning: %s #" % (msg,), 4)

    @staticmethod
    def displayError(msg):
        _az.write("# Error: %s #" % (msg,), 2)

    @staticmethod
    def _mel(command):
        v = _json.loads(_az.mel(command))
        if isinstance(v, dict) and "__error__" in v:
            raise RuntimeError(v["__error__"])
        return v

    @staticmethod
    def executeCommand(command, displayEnabled=False, undoEnabled=False):
        MGlobal._mel(command)

    @staticmethod
    def executeCommandStringResult(command, displayEnabled=False, undoEnabled=False):
        v = MGlobal._mel(command)
        if isinstance(v, list):
            return [str(x) for x in v]
        return "" if v is None else str(v)

    @staticmethod
    def executeCommandOnIdle(command, displayEnabled=False):
        _az.mel(command)

    @staticmethod
    def executePythonCommand(command, displayEnabled=False, undoEnabled=False):
        import __main__

        g = __main__.__dict__
        try:
            code = compile(command, "<executePythonCommand>", "eval")
        except SyntaxError:
            exec(compile(command, "<executePythonCommand>", "exec"), g)
            return None
        return eval(code, g)

    @staticmethod
    def executePythonCommandStringResult(command, displayEnabled=False, undoEnabled=False):
        v = MGlobal.executePythonCommand(command)
        return "" if v is None else str(v)

    @staticmethod
    def executePythonCommandOnIdle(command, displayEnabled=False):
        MGlobal.executePythonCommand(command)

    @staticmethod
    def isUndoing():
        return False

    @staticmethod
    def isRedoing():
        return False

    @staticmethod
    def isYAxisUp():
        return _cmds.upAxis(q=True, axis=True) == "y"

    @staticmethod
    def isZAxisUp():
        return _cmds.upAxis(q=True, axis=True) == "z"

    @staticmethod
    def upAxis():
        return MVector(0, 0, 1) if MGlobal.isZAxisUp() else MVector(0, 1, 0)

    @staticmethod
    def setYAxisUp(rotateView=False):
        _cmds.upAxis(axis="y", rotateView=bool(rotateView))

    @staticmethod
    def setZAxisUp(rotateView=False):
        _cmds.upAxis(axis="z", rotateView=bool(rotateView))

    @staticmethod
    def getFunctionSetList(obj):
        return [_FN_NAMES.get(f, "kBase") for f in obj._fns()] + ["kBase"]


class MDGModifier:
    """Edits queued until doIt, undone in reverse by undoIt. Nodes are created at once, as the reference does, and go again on undoIt."""

    def __init__(self):
        self._ops = []
        self._done = 0

    def _queue(self, do, undo):
        self._ops.append((do, undo))

    def doIt(self):
        for do, _ in self._ops[self._done :]:
            do()
        self._done = len(self._ops)
        return self

    def undoIt(self):
        for _, undo in reversed(self._ops[: self._done]):
            undo()
        self._done = 0
        return self

    def _create(self, type_name, parent=None):
        if isinstance(type_name, MTypeId):
            type_name = _node_type_of_id(type_name)
        kw = {"skipSelect": True}
        if parent is not None:
            kw["parent"] = _api("node.info", id=parent)["path"]
        made = _cmds.createNode(type_name, **kw)
        obj = _node_object(_node_id_named(made))
        state = {"exists": True}

        def again():
            if not state["exists"]:
                obj._id = _node_id_named(_cmds.createNode(type_name, **kw))
                state["exists"] = True

        def gone():
            if state["exists"]:
                _cmds.delete(_api("node.info", id=obj._id)["path"])
                state["exists"] = False

        self._queue(again, gone)
        return obj

    def createNode(self, type_name):
        return self._create(type_name)

    def _plugs(self, args):
        if len(args) == 2:
            return args[0], args[1]
        if len(args) == 4:
            return MPlug(args[0], args[1]), MPlug(args[2], args[3])
        _fail()

    def connect(self, *args):
        src, dst = self._plugs(args)
        s, d = src._hostname(), dst._hostname()
        self._queue(lambda: _cmds.connectAttr(s, d), lambda: _cmds.disconnectAttr(s, d))
        return self

    def disconnect(self, *args):
        src, dst = self._plugs(args)
        s, d = src._hostname(), dst._hostname()
        self._queue(lambda: _cmds.disconnectAttr(s, d), lambda: _cmds.connectAttr(s, d))
        return self

    def deleteNode(self, node, includeParents=False):
        id = _check_node(node)
        name = _api("node.info", id=id)["path"]
        info = _api("node.info", id=id)
        kind, parent = info["type"], info["parent"]

        def undo():
            kw = {"skipSelect": True, "name": info["name"]}
            if parent:
                kw["parent"] = _api("node.info", id=parent)["path"]
            node._id = _node_id_named(_cmds.createNode(kind, **kw))

        self._queue(lambda: _cmds.delete(_api("node.info", id=node._id)["path"] if node._id else name), undo)
        return self

    def renameNode(self, node, newName):
        id = _check_node(node)
        old = _api("node.info", id=id)["name"]
        self._queue(lambda: _cmds.rename(_api("node.info", id=node._id)["path"], newName), lambda: _cmds.rename(_api("node.info", id=node._id)["path"], old))
        return self

    def newPlugValue(self, plug, data):
        return self._plug_value(plug, data._data)

    def _plug_value(self, plug, v):
        p = MPlug(plug)
        old = p._value()
        self._queue(lambda: p._set(v), lambda: p._set(old))
        return self

    def newPlugValueBool(self, plug, v):
        return self._plug_value(plug, bool(v))

    def newPlugValueInt(self, plug, v):
        return self._plug_value(plug, int(v))

    newPlugValueShort = newPlugValueInt
    newPlugValueChar = newPlugValueInt

    def newPlugValueDouble(self, plug, v):
        return self._plug_value(plug, float(v))

    newPlugValueFloat = newPlugValueDouble

    def newPlugValueString(self, plug, v):
        return self._plug_value(plug, str(v))

    def newPlugValueMAngle(self, plug, a):
        return self._plug_value(plug, a.asRadians())

    def newPlugValueMDistance(self, plug, d):
        return self._plug_value(plug, d.asCentimeters())

    def newPlugValueMTime(self, plug, t):
        return self._plug_value(plug, t.asUnits(MTime.uiUnit()))

    def addAttribute(self, node, attr):
        def add():
            MFnDependencyNode(node).addAttribute(attr)

        def rem():
            MFnDependencyNode(node).removeAttribute(attr)

        self._queue(add, rem)
        return self

    def removeAttribute(self, node, attr):
        def rem():
            MFnDependencyNode(node).removeAttribute(attr)

        self._queue(rem, lambda: MFnDependencyNode(node).addAttribute(attr))
        return self

    def commandToExecute(self, command):
        self._queue(lambda: _az.mel(command), lambda: None)
        return self

    def pythonCommandToExecute(self, command):
        self._queue(lambda: MGlobal.executePythonCommand(command), lambda: None)
        return self


class MDagModifier(MDGModifier):
    def createNode(self, type_name, parent=None):
        if isinstance(type_name, MTypeId):
            type_name = _node_type_of_id(type_name)
        parent_id = None
        if parent is not None and not parent.isNull():
            parent_id = None if parent._kind == "world" else _check_node(parent)
        return self._create(type_name, parent_id)

    def reparentNode(self, node, newParent=None):
        id = _check_node(node)
        was = _api("node.info", id=id)["parent"]

        def to(target):
            path = _api("node.info", id=node._id)["path"]
            if target:
                _cmds.parent(path, _api("node.info", id=target)["path"])
            else:
                _cmds.parent(path, world=True)

        target = None if newParent is None or newParent.isNull() or newParent._kind == "world" else _check_node(newParent)
        self._queue(lambda: to(target), lambda: to(was))
        return self
