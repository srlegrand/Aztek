# aztek.OpenAztek (maya.OpenMaya): the reference's Python API 1.0, built on the 2.0 classes of maya.api.OpenMaya.
#
# 1.0 hands results back through the arguments (an MDagPath to fill, an MScriptUtil pointer for a number) where 2.0
# returns them. Each class here is its 2.0 class with the 1.0 calls on top; what a 2.0 call returns comes back as the
# 1.0 class, so `arr.length()` and the other 1.0 spellings work on everything a script gets.

import aztek.api.OpenAztek as _om2

_V1 = {}


def _to1(x):
    """A 2.0 value as its 1.0 class (in place); arrays have their items turned too."""
    c = _V1.get(type(x))
    if c is not None:
        try:
            x.__class__ = c
        except TypeError:
            return x
        if isinstance(x, list) and x and type(x[0]) in _V1:
            for v in x:
                _to1(v)
        return x
    if type(x) is tuple:
        return tuple(_to1(v) for v in x)
    if type(x) is list:
        return [_to1(v) for v in x]
    return x


_PLAIN = {int, float, bool, str, type(None)}


def _wrapped(f):
    def call(*a, **k):
        r = f(*a, **k)
        return r if type(r) in _PLAIN or type(r).__module__ == "aztek.OpenAztek" else _to1(r)

    call.__name__ = getattr(f, "__name__", "call")
    call.__doc__ = getattr(f, "__doc__", None)
    return call


_SKIP = {
    "__init__", "__new__", "__class__", "__getattribute__", "__setattr__", "__delattr__", "__init_subclass__",
    "__subclasshook__", "__slots__", "__dict__", "__weakref__", "__module__", "__doc__", "__repr__", "__str__",
    "__hash__", "__eq__", "__ne__", "__len__", "__bool__", "__contains__", "__reduce__", "__reduce_ex__",
    "__getstate__", "__setstate__", "__sizeof__", "__format__", "__dir__", "__lt__", "__le__", "__gt__", "__ge__",
    "__qualname__", "__firstlineno__", "__static_attributes__", "__abstractmethods__",
}


def _get_named(self, name):
    """1.0's getX() where 2.0 calls it x(): getTranslation, getRotatePivot and the rest."""
    if name.startswith("get") and len(name) > 3:
        f = getattr(type(self), name[3].lower() + name[4:], None)
        if callable(f):
            return _wrapped(f.__get__(self))
    raise AttributeError("'%s' object has no attribute '%s'" % (type(self).__name__, name))


def _v1(base, body=None, name=None):
    """The 1.0 class over a 2.0 one: the 1.0 calls in `body`, every other call handing back 1.0 values."""
    body = dict(body or {})
    ns = {"__slots__": (), "__module__": "aztek.OpenAztek"}
    done = set(body)
    for klass in base.__mro__:
        if klass in (object, list):
            continue
        for k, v in vars(klass).items():
            if k in done or k in _SKIP or (k.startswith("_") and not k.startswith("__")):
                continue
            done.add(k)
            if isinstance(v, staticmethod):
                ns[k] = staticmethod(_wrapped(v.__func__))
            elif isinstance(v, classmethod):
                ns[k] = classmethod(_wrapped(v.__func__))
            elif isinstance(v, property):
                # 1.0 has calls where 2.0 has properties: fn.name(), fn.setKeyable(True), fn.isKeyable().
                ns[k] = _wrapped(v.fget)
                cap = k[0].upper() + k[1:]
                if v.fset is not None:
                    setter = "set" + cap
                    if setter not in done and not hasattr(base, setter):
                        ns[setter] = (lambda fset: lambda self, value: fset(self, value))(v.fset)
                        done.add(setter)
                if not k.startswith("is"):
                    asker = "is" + cap
                    if asker not in done and not hasattr(base, asker):
                        ns[asker] = ns[k]
                        done.add(asker)
            elif callable(v) and not isinstance(v, type):
                ns[k] = _wrapped(v)
    for k in ("__iter__",):
        if k not in body and hasattr(base, k) and issubclass(base, list):
            ns[k] = lambda self: (_to1(v) for v in list.__iter__(self))
    ns.update(body)
    if "__getattr__" not in ns and not hasattr(base, "__getattr__"):
        ns["__getattr__"] = _get_named
    c = type(name or base.__name__, (base,), ns)
    _V1[base] = c
    return c


def _copy(dst, src):
    """Puts what `src` holds into the out argument `dst`, as 1.0 fills the argument it is given."""
    if isinstance(dst, list):
        dst[:] = list(src)
        return
    for klass in type(src).__mro__:
        for slot in getattr(klass, "__slots__", ()):
            if hasattr(src, slot):
                object.__setattr__(dst, slot, getattr(src, slot))
    d = getattr(src, "__dict__", None)
    if d is not None and hasattr(dst, "__dict__"):
        dst.__dict__.update(d)


# ---------------------------------------------------------------------------------------------------------------------
# Status, strings and pointers


class MStatus:
    kSuccess = 0
    kFailure = 1
    kInsufficientMemory = 2
    kInvalidParameter = 3
    kLicenseFailure = 4
    kUnknownParameter = 5
    kNotImplemented = 6
    kNotFound = 7
    kEndOfFile = 8

    def __init__(self, code=0):
        self._code = code.statusCode() if isinstance(code, MStatus) else int(code)

    def statusCode(self):
        return self._code

    def error(self):
        return self._code != 0

    def __bool__(self):
        return self._code == 0

    def __eq__(self, o):
        return self._code == (o.statusCode() if isinstance(o, MStatus) else o)

    def __hash__(self):
        return hash(self._code)

    def clear(self):
        self._code = 0

    def perror(self, msg=""):
        _om2.MGlobal.displayError(msg)

    def errorString(self):
        return "" if self._code == 0 else "(kFailure)"


kUnknownParameter = MStatus.kUnknownParameter


class MString(str):
    def asChar(self):
        return str(self)

    def asUTF8(self):
        return str(self)

    def length(self):
        return len(self)

    def numChars(self):
        return len(self)

    def isEmpty(self):
        return len(self) == 0


class _Ptr:
    """What an MScriptUtil hands out as int*, double* and the rest: a view on its values."""

    __slots__ = ("v", "cast")

    def __init__(self, v, cast):
        self.v = v
        self.cast = cast


def _put(ptr, *values):
    """Writes into a pointer argument; anything else (None, a plain number) is left alone."""
    if isinstance(ptr, _Ptr):
        while len(ptr.v) < len(values):
            ptr.v.append(0)
        for i, x in enumerate(values):
            ptr.v[i] = ptr.cast(x)


def _get(ptr, i=0):
    if isinstance(ptr, _Ptr):
        return ptr.v[i] if i < len(ptr.v) else 0
    return ptr


def _bool(v):
    return bool(v)


class MScriptUtil:
    """Numbers passed by pointer, as 1.0 asks for them."""

    def __init__(self, *args):
        self._v = [0] * 16
        if len(args) == 1 and isinstance(args[0], (list, tuple)):
            self.createFromList(args[0], len(args[0]))
        elif args:
            self._v = list(args) + [0] * 16

    def createFromInt(self, *values):
        self._v = [int(x) for x in values] + [0] * 16

    def createFromDouble(self, *values):
        self._v = [float(x) for x in values] + [0.0] * 16

    def createFromList(self, values, count):
        self._v = list(values)[: int(count)] + [0] * 16

    def _ptr(self, cast):
        return _Ptr(self._v, cast)

    def asIntPtr(self):
        return self._ptr(int)

    def asUintPtr(self):
        return self._ptr(int)

    def asShortPtr(self):
        return self._ptr(int)

    def asUshortPtr(self):
        return self._ptr(int)

    def asCharPtr(self):
        return self._ptr(int)

    def asUcharPtr(self):
        return self._ptr(int)

    def asBoolPtr(self):
        return self._ptr(_bool)

    def asFloatPtr(self):
        return self._ptr(float)

    def asDoublePtr(self):
        return self._ptr(float)

    asInt2Ptr = asInt3Ptr = asInt4Ptr = asIntPtr
    asShort2Ptr = asShort3Ptr = asShort4Ptr = asShortPtr
    asFloat2Ptr = asFloat3Ptr = asFloat4Ptr = asFloatPtr
    asDouble2Ptr = asDouble3Ptr = asDouble4Ptr = asDoublePtr

    def asInt(self):
        return int(self._v[0])

    def asUint(self):
        return int(self._v[0])

    def asShort(self):
        return int(self._v[0])

    def asBool(self):
        return bool(self._v[0])

    def asFloat(self):
        return float(self._v[0])

    def asDouble(self):
        return float(self._v[0])

    @staticmethod
    def getInt(ptr):
        return int(_get(ptr))

    getUint = getShort = getUshort = getChar = getUchar = getInt

    @staticmethod
    def getFloat(ptr):
        return float(_get(ptr))

    getDouble = getFloat

    @staticmethod
    def getBool(ptr):
        return bool(_get(ptr))

    @staticmethod
    def setInt(ptr, v):
        _put(ptr, v)

    setUint = setShort = setUshort = setChar = setUchar = setBool = setFloat = setDouble = setInt

    @staticmethod
    def getIntArrayItem(ptr, i):
        return int(_get(ptr, i))

    getUintArrayItem = getShortArrayItem = getUshortArrayItem = getCharArrayItem = getUcharArrayItem = getIntArrayItem

    @staticmethod
    def getFloatArrayItem(ptr, i):
        return float(_get(ptr, i))

    getDoubleArrayItem = getFloatArrayItem

    @staticmethod
    def getBoolArrayItem(ptr, i):
        return bool(_get(ptr, i))

    @staticmethod
    def setIntArray(ptr, i, v):
        if isinstance(ptr, _Ptr):
            while len(ptr.v) <= i:
                ptr.v.append(0)
            ptr.v[i] = ptr.cast(v)

    setUintArray = setShortArray = setUshortArray = setCharArray = setUcharArray = setBoolArray = setIntArray
    setFloatArray = setDoubleArray = setIntArray

    @staticmethod
    def _rc(ptr, r, c, n):
        return _get(ptr, r * n + c)

    @staticmethod
    def getInt2ArrayItem(ptr, r, c):
        return int(_get(ptr, r * 2 + c))

    @staticmethod
    def getInt3ArrayItem(ptr, r, c):
        return int(_get(ptr, r * 3 + c))

    @staticmethod
    def getInt4ArrayItem(ptr, r, c):
        return int(_get(ptr, r * 4 + c))

    @staticmethod
    def getFloat2ArrayItem(ptr, r, c):
        return float(_get(ptr, r * 2 + c))

    @staticmethod
    def getFloat3ArrayItem(ptr, r, c):
        return float(_get(ptr, r * 3 + c))

    @staticmethod
    def getFloat4ArrayItem(ptr, r, c):
        return float(_get(ptr, r * 4 + c))

    getDouble2ArrayItem = getFloat2ArrayItem
    getDouble3ArrayItem = getFloat3ArrayItem
    getDouble4ArrayItem = getFloat4ArrayItem

    @staticmethod
    def _set_rc(ptr, r, c, n, v):
        MScriptUtil.setIntArray(ptr, r * n + c, v)

    @staticmethod
    def setInt2ArrayItem(ptr, r, c, v):
        MScriptUtil.setIntArray(ptr, r * 2 + c, v)

    @staticmethod
    def setInt3ArrayItem(ptr, r, c, v):
        MScriptUtil.setIntArray(ptr, r * 3 + c, v)

    @staticmethod
    def setInt4ArrayItem(ptr, r, c, v):
        MScriptUtil.setIntArray(ptr, r * 4 + c, v)

    setFloat2ArrayItem = setDouble2ArrayItem = setInt2ArrayItem
    setFloat3ArrayItem = setDouble3ArrayItem = setInt3ArrayItem
    setFloat4ArrayItem = setDouble4ArrayItem = setInt4ArrayItem

    @staticmethod
    def createMatrixFromList(values, matrix):
        _copy(matrix, _om2.MMatrix([float(x) for x in values][:16]))

    @staticmethod
    def createFloatMatrixFromList(values, matrix):
        _copy(matrix, _om2.MFloatMatrix([float(x) for x in values][:16]))

    @staticmethod
    def createIntArrayFromList(values, array):
        array[:] = [int(x) for x in values]

    @staticmethod
    def createFloatArrayFromList(values, array):
        array[:] = [float(x) for x in values]

    @staticmethod
    def createDoubleArrayFromList(values, array):
        array[:] = [float(x) for x in values]


def _fill_rotation(out, rot):
    """An MEulerRotation or MQuaternion out argument, or a double[3]/[4] pointer, takes `rot` in its own kind."""
    if isinstance(out, _om2.MQuaternion):
        _copy(out, rot if isinstance(rot, _om2.MQuaternion) else rot.asQuaternion())
    elif isinstance(out, _om2.MEulerRotation):
        _copy(out, rot if isinstance(rot, _om2.MEulerRotation) else rot.asEulerRotation())
    elif isinstance(out, _Ptr):
        e = rot if isinstance(rot, _om2.MEulerRotation) else rot.asEulerRotation()
        _put(out, e.x, e.y, e.z)


# ---------------------------------------------------------------------------------------------------------------------
# Values


MSpace = _om2.MSpace
MFn = _om2.MFn


def _array_body():
    def length(self):
        return len(self)

    def set(self, v, i):
        self[i] = v

    def get(self, ptr):
        _put(ptr, *self)

    return {"length": length, "set": set, "get": get, "clear": lambda self: list.clear(self)}


MIntArray = _v1(_om2.MIntArray, _array_body())
MFloatArray = _v1(_om2.MFloatArray, _array_body())
MDoubleArray = _v1(_om2.MDoubleArray, _array_body())
MStringArray = _v1(_om2.MStringArray, _array_body())
MUintArray = _v1(_om2.MIntArray, _array_body(), "MUintArray")
_V1[_om2.MIntArray] = MIntArray
MVector = _v1(_om2.MVector, {"get": lambda self, ptr: _put(ptr, self.x, self.y, self.z)})
MFloatVector = _v1(_om2.MFloatVector, {"get": lambda self, ptr: _put(ptr, self.x, self.y, self.z)})
MPoint = _v1(_om2.MPoint, {"get": lambda self, ptr: _put(ptr, self.x, self.y, self.z, self.w)})
MFloatPoint = _v1(_om2.MFloatPoint, {"get": lambda self, ptr: _put(ptr, self.x, self.y, self.z, self.w)})
MColor = _v1(_om2.MColor, {"get": lambda self, ptr: _put(ptr, self.r, self.g, self.b, self.a)})


def _matrix_get(self, ptr):
    _put(ptr, *[self.getElement(r, c) for r in range(4) for c in range(4)])


class _Row:
    """m[r][c] and MScriptUtil.getDoubleArrayItem(m[r], c), as 1.0 indexes a matrix by row."""

    def __init__(self, m, r):
        self.m, self.r = m, r

    def __getitem__(self, c):
        return self.m.getElement(self.r, c)

    def __setitem__(self, c, v):
        self.m.setElement(self.r, c, v)


def _matrix_body():
    def call(self, r, c):
        return self.getElement(r, c)

    def getitem(self, i):
        return _Row(self, i)

    return {"__call__": call, "__getitem__": getitem, "get": _matrix_get}


MMatrix = _v1(_om2.MMatrix, _matrix_body())
MFloatMatrix = _v1(_om2.MFloatMatrix, _matrix_body())
_row_get = MScriptUtil.getDoubleArrayItem


def _array_item(ptr, i):
    if isinstance(ptr, _Row):
        return ptr[i]
    return _row_get(ptr, i)


MScriptUtil.getDoubleArrayItem = staticmethod(_array_item)
MScriptUtil.getFloatArrayItem = staticmethod(_array_item)

MPointArray = _v1(_om2.MPointArray, dict(_array_body(), _item=staticmethod(lambda x: MPoint(x)), _blank=staticmethod(MPoint)))
MFloatPointArray = _v1(_om2.MFloatPointArray, dict(_array_body(), _item=staticmethod(lambda x: MFloatPoint(x)), _blank=staticmethod(MFloatPoint)))
MVectorArray = _v1(_om2.MVectorArray, dict(_array_body(), _item=staticmethod(lambda x: MVector(x)), _blank=staticmethod(MVector)))
MFloatVectorArray = _v1(_om2.MFloatVectorArray, dict(_array_body(), _item=staticmethod(lambda x: MFloatVector(x)), _blank=staticmethod(MFloatVector)))
MMatrixArray = _v1(_om2.MMatrixArray, dict(_array_body(), _item=staticmethod(lambda x: MMatrix(x)), _blank=staticmethod(MMatrix)))
MColorArray = _v1(_om2.MColorArray, dict(_array_body(), _item=staticmethod(lambda x: MColor(x)), _blank=staticmethod(MColor)))
MAngle = _v1(_om2.MAngle)
MDistance = _v1(_om2.MDistance)
MTime = _v1(_om2.MTime)
MTimeArray = _v1(_om2.MTimeArray, dict(_array_body(), _item=staticmethod(lambda x: MTime(x)), _blank=staticmethod(MTime)))
def _bb_corner(which):
    def call(self):
        p = getattr(self, which)
        return MPoint(p) if p is not None else MPoint()

    return call


def _bb_center(self):
    a, b = _bb_corner("_min")(self), _bb_corner("_max")(self)
    return MPoint((a.x + b.x) / 2, (a.y + b.y) / 2, (a.z + b.z) / 2)


MBoundingBox = _v1(
    _om2.MBoundingBox,
    {
        "min": _bb_corner("_min"),
        "max": _bb_corner("_max"),
        "center": _bb_center,
        "width": lambda self: _bb_corner("_max")(self).x - _bb_corner("_min")(self).x,
        "height": lambda self: _bb_corner("_max")(self).y - _bb_corner("_min")(self).y,
        "depth": lambda self: _bb_corner("_max")(self).z - _bb_corner("_min")(self).z,
        "__repr__": lambda self: "aztek.OpenAztek.MBoundingBox(%s, %s)" % (self.min(), self.max()),
    },
)
MPlane = _v1(_om2.MPlane)
MTypeId = _v1(_om2.MTypeId)
MEulerRotation = _v1(_om2.MEulerRotation)
MQuaternion = _v1(_om2.MQuaternion, {"get": lambda self, ptr: _put(ptr, self.x, self.y, self.z, self.w)})


def _tm_get_rotation(self, out, order=None, space=None):
    _fill_rotation(out, _om2.MTransformationMatrix.rotation(self, False))
    if isinstance(order, _Ptr):
        _put(order, self.rotationOrder())


def _tm_set_rotation(self, rot, order=None, space=None):
    if isinstance(rot, _Ptr):
        rot = _om2.MEulerRotation(rot.v[0], rot.v[1], rot.v[2], int(order) if order is not None else 0)
    _om2.MTransformationMatrix.setRotation(self, rot)


def _tm_get_scale(self, ptr, space=MSpace.kTransform):
    _put(ptr, *_om2.MTransformationMatrix.scale(self, space))


def _tm_set_scale(self, ptr, space=MSpace.kTransform):
    v = ptr.v[:3] if isinstance(ptr, _Ptr) else list(ptr)
    _om2.MTransformationMatrix.setScale(self, v, space)


MTransformationMatrix = _v1(
    _om2.MTransformationMatrix,
    {
        "getTranslation": lambda self, space=MSpace.kTransform: _to1(_om2.MTransformationMatrix.translation(self, space)),
        "rotation": lambda self: _to1(_om2.MTransformationMatrix.rotation(self, True)),
        "eulerRotation": lambda self: _to1(_om2.MTransformationMatrix.rotation(self, False)),
        "getRotation": _tm_get_rotation,
        "setRotation": _tm_set_rotation,
        "rotateTo": lambda self, rot, space=None: _om2.MTransformationMatrix.setRotation(self, rot),
        "getRotationQuaternion": lambda self, x, y, z, w, space=None: (
            lambda q: (_put(x, q.x), _put(y, q.y), _put(z, q.z), _put(w, q.w))
        )(_om2.MTransformationMatrix.rotation(self, True)),
        "setRotationQuaternion": lambda self, x, y, z, w, space=None: _om2.MTransformationMatrix.setRotation(
            self, _om2.MQuaternion(x, y, z, w)
        ),
        "getScale": _tm_get_scale,
        "setScale": _tm_set_scale,
        "addScale": lambda self, ptr, space=MSpace.kTransform: _om2.MTransformationMatrix.scaleBy(self, ptr.v[:3] if isinstance(ptr, _Ptr) else list(ptr), space),
        "getShear": lambda self, ptr, space=MSpace.kTransform: _put(ptr, *_om2.MTransformationMatrix.shear(self, space)),
        "setShear": lambda self, ptr, space=MSpace.kTransform: _om2.MTransformationMatrix.setShear(self, ptr.v[:3] if isinstance(ptr, _Ptr) else list(ptr), space),
        "addTranslation": lambda self, v, space=MSpace.kTransform: _om2.MTransformationMatrix.translateBy(self, v, space),
    },
)

# ---------------------------------------------------------------------------------------------------------------------
# Objects, paths and plugs

MObject = _v1(_om2.MObject, {"__repr__": lambda self: "<aztek.OpenAztek.MObject %s>" % (_om2.MObject.apiTypeStr.__get__(self),)})
MObject.kNullObj = MObject()
MObjectHandle = _v1(_om2.MObjectHandle)


def _arr_fill(dst, src):
    if dst is not None:
        dst[:] = list(src)
        _to1(dst)


def _dp_get_a_path_to(obj, path=None):
    p = _om2.MDagPath.getAPathTo(obj)
    if path is None:
        return _to1(p)
    _copy(path, p)


def _dp_get_all_paths_to(obj, paths=None):
    ps = _om2.MDagPath.getAllPathsTo(obj)
    if paths is None:
        return _to1(ps)
    _arr_fill(paths, ps)


def _dp_shapes_below(self, ptr=None):
    n = _om2.MDagPath.numberOfShapesDirectlyBelow(self)
    if ptr is None:
        return n
    _put(ptr, n)


def _dp_get_path(self, path=None):
    if path is None:
        return _to1(_om2.MDagPath(self))
    _copy(path, self)


MDagPath = _v1(
    _om2.MDagPath,
    {
        "getAPathTo": staticmethod(_dp_get_a_path_to),
        "getAllPathsTo": staticmethod(_dp_get_all_paths_to),
        "numberOfShapesDirectlyBelow": _dp_shapes_below,
        "getPath": _dp_get_path,
    },
)
MDagPathArray = _v1(_om2.MDagPathArray, dict(_array_body(), _item=staticmethod(lambda x: MDagPath(x)), _blank=staticmethod(MDagPath)))
MObjectArray = _v1(_om2.MObjectArray, dict(_array_body(), _item=staticmethod(lambda x: MObject(x)), _blank=staticmethod(MObject)))


def _plug_connected_to(self, *args):
    if args and isinstance(args[0], list):
        arr, as_dst, as_src = args[0], args[1], args[2]
        got = _om2.MPlug.connectedTo(self, as_dst, as_src)
        _arr_fill(arr, got)
        return len(got) > 0
    return _to1(_om2.MPlug.connectedTo(self, *args))


def _plug_indices(self, arr=None):
    got = _om2.MPlug.getExistingArrayAttributeIndices(self)
    if arr is None:
        return _to1(got)
    _arr_fill(arr, got)
    return len(got)


def _plug_get_value(self, out):
    """1.0's getValue(int&/double&/...) through an MScriptUtil pointer, or an MObject/MString to fill."""
    if isinstance(out, _Ptr):
        _put(out, out.cast(self.asDouble() if out.cast is float else self.asInt() if out.cast is int else self.asBool()))
    elif isinstance(out, _om2.MObject):
        _copy(out, self.asMObject())
    elif isinstance(out, _om2.MAngle):
        _copy(out, self.asMAngle())
    elif isinstance(out, _om2.MDistance):
        _copy(out, self.asMDistance())
    elif isinstance(out, _om2.MTime):
        _copy(out, self.asMTime())


def _plug_set_value(self, v):
    if isinstance(v, bool):
        self.setBool(v)
    elif isinstance(v, int):
        self.setInt(v)
    elif isinstance(v, float):
        self.setDouble(v)
    elif isinstance(v, str):
        self.setString(v)
    elif isinstance(v, _om2.MObject):
        self.setMObject(v)
    elif isinstance(v, _om2.MAngle):
        self.setMAngle(v)
    elif isinstance(v, _om2.MDistance):
        self.setMDistance(v)
    elif isinstance(v, _om2.MTime):
        self.setMTime(v)


MPlug = _v1(
    _om2.MPlug,
    {
        "connectedTo": _plug_connected_to,
        "getExistingArrayAttributeIndices": _plug_indices,
        "getValue": _plug_get_value,
        "setValue": _plug_set_value,
    },
)
MPlugArray = _v1(_om2.MPlugArray, dict(_array_body(), _item=staticmethod(lambda x: MPlug(x)), _blank=staticmethod(MPlug)))


def _sl_get_dag_path(self, i, path=None, comp=None):
    if path is None:
        return _to1(_om2.MSelectionList.getDagPath(self, i))
    _copy(path, _om2.MSelectionList.getDagPath(self, i))
    if comp is not None:
        p, c = _om2.MSelectionList.getComponent(self, i)
        _copy(comp, c)


def _sl_get_depend_node(self, i, obj=None):
    o = _om2.MSelectionList.getDependNode(self, i)
    if obj is None:
        return _to1(o)
    _copy(obj, o)


def _sl_get_plug(self, i, plug=None):
    p = _om2.MSelectionList.getPlug(self, i)
    if plug is None:
        return _to1(p)
    _copy(plug, p)


def _sl_strings(self, *args):
    if args and isinstance(args[-1], list):
        out = args[-1]
        got = _om2.MSelectionList.getSelectionStrings(self, *args[:-1])
        _arr_fill(out, got)
        return None
    return _to1(_om2.MSelectionList.getSelectionStrings(self, *args))


def _sl_add(self, item, *rest):
    # 1.0 add(dagPath, component, merge) or add(name or object, merge).
    if rest and isinstance(rest[0], _om2.MObject) and isinstance(item, _om2.MDagPath):
        if rest[0].isNull():
            return _om2.MSelectionList.add(self, item, *rest[1:])
        return _om2.MSelectionList.add(self, (item, rest[0]), *rest[1:])
    return _om2.MSelectionList.add(self, item, *rest)


MSelectionList = _v1(
    _om2.MSelectionList,
    {
        "getDagPath": _sl_get_dag_path,
        "getDependNode": _sl_get_depend_node,
        "getPlug": _sl_get_plug,
        "getSelectionStrings": _sl_strings,
        "add": _sl_add,
    },
)


def _g_active(sel=None, ordered=False):
    got = _om2.MGlobal.getActiveSelectionList(ordered)
    if sel is None:
        return _to1(got)
    _om2.MSelectionList.clear(sel)
    _om2.MSelectionList.merge(sel, got)


def _g_by_name(name, sel=None):
    got = _om2.MGlobal.getSelectionListByName(name)
    if sel is None:
        return _to1(got)
    _om2.MSelectionList.merge(sel, got)


class MCommandResult:
    kInvalid = 0
    kInt = 1
    kIntArray = 2
    kDouble = 3
    kDoubleArray = 4
    kString = 5
    kStringArray = 6
    kVector = 7
    kVectorArray = 8
    kMatrix = 9
    kMatrixArray = 10

    def __init__(self):
        self._v = None

    def resultType(self):
        v = self._v
        if v is None:
            return self.kInvalid
        if isinstance(v, bool) or isinstance(v, int):
            return self.kInt
        if isinstance(v, float):
            return self.kDouble
        if isinstance(v, str):
            return self.kString
        if isinstance(v, list):
            if all(isinstance(x, str) for x in v):
                return self.kStringArray
            if all(isinstance(x, int) for x in v):
                return self.kIntArray
            return self.kDoubleArray
        return self.kInvalid

    def stringResult(self):
        return MString(self._v if isinstance(self._v, str) else str(self._v))

    def getResult(self, out):
        v = self._v
        if isinstance(out, list):
            out[:] = v if isinstance(v, list) else [v]
        else:
            _put(out, v)


def _g_execute(command, *args):
    """executeCommand(cmd[, result][, display[, undoable]]): a result argument (int or double pointer, array or
    MCommandResult) takes what the command gave back."""
    out = None
    if args and not isinstance(args[0], bool):
        out, args = args[0], args[1:]
    if out is None:
        return _om2.MGlobal.executeCommand(command, *args)
    r = _om2.MGlobal._mel(command)
    if isinstance(out, MCommandResult):
        out._v = r
    elif isinstance(out, list):
        out[:] = r if isinstance(r, list) else ([] if r is None else [r])
    elif isinstance(out, _Ptr):
        _put(out, r[0] if isinstance(r, list) else r)
    return None


def _g_execute_python(command, *args):
    out = None
    if args and not isinstance(args[0], bool):
        out, args = args[0], args[1:]
    r = _om2.MGlobal.executePythonCommandStringResult(command, *args) if out is not None else _om2.MGlobal.executePythonCommand(command, *args)
    if isinstance(out, list):
        out[:] = [r]
    elif isinstance(out, _Ptr):
        _put(out, r)
    return None if out is not None else r


MGlobal = _v1(
    _om2.MGlobal,
    {
        "getActiveSelectionList": staticmethod(_g_active),
        "getSelectionListByName": staticmethod(_g_by_name),
        "executeCommand": staticmethod(_g_execute),
        "executePythonCommand": staticmethod(_g_execute_python),
        "getLiveList": staticmethod(lambda sel: None),
        "getHiliteList": staticmethod(lambda sel: _om2.MSelectionList.merge(sel, _om2.MGlobal.getSelectionListByName("*")) if False else None),
    },
)

MDGModifier = _v1(_om2.MDGModifier)
MDagModifier = _v1(_om2.MDagModifier)

# ---------------------------------------------------------------------------------------------------------------------
# Function sets


def _status_of(f):
    """A 2.0 setter that returns its function set; 1.0 returns nothing (an MStatus that raises on failure)."""

    def call(self, *a, **k):
        f(self, *a, **k)

    return call


MFnBase = _v1(_om2.MFnBase, {"setObject": _status_of(_om2.MFnBase.setObject)})


def _dn_connections(self, arr=None):
    got = _om2.MFnDependencyNode.getConnections(self)
    if arr is None:
        return _to1(got)
    _arr_fill(arr, got)


def _dn_attribute(self, a):
    return _to1(_om2.MFnDependencyNode.attribute(self, a))


MFnDependencyNode = _v1(
    _om2.MFnDependencyNode,
    {"getConnections": _dn_connections, "attribute": _dn_attribute, "setObject": _status_of(_om2.MFnDependencyNode.setObject) if hasattr(_om2.MFnDependencyNode, "setObject") else _status_of(_om2.MFnBase.setObject)},
)


def _dag_get_path(self, path=None):
    p = _om2.MFnDagNode.getPath(self)
    if path is None:
        return _to1(p)
    _copy(path, p)


def _dag_all_paths(self, arr=None):
    got = _om2.MFnDagNode.getAllPaths(self)
    if arr is None:
        return _to1(got)
    _arr_fill(arr, got)


MFnDagNode = _v1(
    _om2.MFnDagNode,
    {"getPath": _dag_get_path, "getAllPaths": _dag_all_paths, "setObject": _status_of(_om2.MFnDagNode.setObject)},
)


def _tf_get_rotation(self, out, space=MSpace.kTransform, *rest):
    if isinstance(out, _om2.MQuaternion):
        _copy(out, _om2.MFnTransform.rotation(self, space, True))
    elif isinstance(out, _om2.MEulerRotation):
        _copy(out, _om2.MFnTransform.rotation(self, space, False))
    elif isinstance(out, _Ptr):
        e = _om2.MFnTransform.rotation(self, MSpace.kTransform, False)
        _put(out, e.x, e.y, e.z)
        if isinstance(space, _Ptr):
            _put(space, e.order)


def _tf_set_rotation(self, rot, *rest):
    if isinstance(rot, _Ptr):
        order = int(rest[0]) if rest else 0
        _om2.MFnTransform.setRotation(self, _om2.MEulerRotation(rot.v[0], rot.v[1], rot.v[2], order), MSpace.kTransform)
    else:
        _om2.MFnTransform.setRotation(self, rot, *rest)


def _tf_get_scale(self, ptr):
    _put(ptr, *_om2.MFnTransform.scale(self))


def _tf_set_scale(self, ptr):
    _om2.MFnTransform.setScale(self, ptr.v[:3] if isinstance(ptr, _Ptr) else list(ptr))


def _tf_scale_by(self, ptr):
    _om2.MFnTransform.scaleBy(self, ptr.v[:3] if isinstance(ptr, _Ptr) else list(ptr))


def _tf_get_shear(self, ptr):
    _put(ptr, *_om2.MFnTransform.shear(self))


def _tf_get_quat(self, x, y, z, w, space=MSpace.kTransform):
    q = _om2.MFnTransform.rotation(self, space, True)
    _put(x, q.x)
    _put(y, q.y)
    _put(z, q.z)
    _put(w, q.w)


MFnTransform = _v1(
    _om2.MFnTransform,
    {
        "getRotation": _tf_get_rotation,
        "setRotation": _tf_set_rotation,
        "rotation": lambda self, space=MSpace.kTransform: _to1(_om2.MFnTransform.rotation(self, space, False)),
        "getScale": _tf_get_scale,
        "setScale": _tf_set_scale,
        "scaleBy": _tf_scale_by,
        "getShear": _tf_get_shear,
        "getRotationQuaternion": _tf_get_quat,
        "setRotationQuaternion": lambda self, x, y, z, w, space=MSpace.kTransform: _om2.MFnTransform.setRotation(self, _om2.MQuaternion(x, y, z, w), space),
        "setObject": _status_of(_om2.MFnTransform.setObject) if "setObject" in vars(_om2.MFnTransform) else _status_of(_om2.MFnDagNode.setObject),
    },
)


def _out(dst, value):
    """Fills an out argument (an array, a point, a vector, an MObject) or a pointer with `value`."""
    if dst is None:
        return _to1(value)
    if isinstance(dst, _Ptr):
        _put(dst, *(value if isinstance(value, (list, tuple)) else [value]))
    elif isinstance(dst, list):
        _arr_fill(dst, value)
    else:
        _copy(dst, value)
    return None


def _space_of(*args, default=MSpace.kObject):
    for a in args:
        if isinstance(a, int) and not isinstance(a, bool):
            return a
    return default


def _m_get_points(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    given = arr is not None
    if not given:
        arr = MPointArray()
    pts = _om2.MFnMesh._point_list(self, space, MFloatPoint if isinstance(arr, _om2.MFloatPointArray) else MPoint)
    list.clear(arr)
    list.extend(arr, pts)
    return None if given else arr


def _m_get_float_points(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MFnMesh.getFloatPoints(self, space))


def _m_get_point(self, i, pt=None, space=MSpace.kObject):
    if isinstance(pt, int) and not isinstance(pt, bool):
        pt, space = None, pt
    return _out(pt, _om2.MFnMesh.getPoint(self, i, space))


def _m_get_vertices(self, counts=None, ids=None):
    c, v = _om2.MFnMesh.getVertices(self)
    if counts is None:
        return _to1((c, v))
    _arr_fill(counts, c)
    _arr_fill(ids, v)


def _m_poly_verts(self, i, ids=None):
    return _out(ids, _om2.MFnMesh.getPolygonVertices(self, i))


def _m_edge_verts(self, e, ptr=None):
    v = _om2.MFnMesh.getEdgeVertices(self, e)
    if ptr is None:
        return tuple(v)
    _put(ptr, v[0], v[1])


def _m_triangles(self, counts=None, verts=None):
    c, v = _om2.MFnMesh.getTriangles(self)
    if counts is None:
        return _to1((c, v))
    _arr_fill(counts, c)
    _arr_fill(verts, v)


def _m_normals(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MFnMesh.getNormals(self, space))


def _m_vertex_normal(self, vid, *args):
    # 1.0: (vertexId, normal, space) or (vertexId, angleWeighted, normal, space).
    weighted, out, space = False, None, MSpace.kObject
    rest = list(args)
    if rest and isinstance(rest[0], bool):
        weighted = rest.pop(0)
    if rest and isinstance(rest[0], _om2.MVector) or (rest and isinstance(rest[0], _om2.MFloatVector)):
        out = rest.pop(0)
    if rest:
        space = rest.pop(0)
    return _out(out, _om2.MFnMesh.getVertexNormal(self, vid, weighted, space))


def _m_vertex_normals(self, weighted, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MFnMesh.getVertexNormals(self, weighted, space))


def _m_face_vertex_normal(self, face, vert, out=None, space=MSpace.kObject):
    if isinstance(out, int) and not isinstance(out, bool):
        out, space = None, out
    return _out(out, _om2.MFnMesh.getFaceVertexNormal(self, face, vert, space))


def _m_face_vertex_normals(self, face, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MFnMesh.getFaceVertexNormals(self, face, space))


def _m_poly_normal(self, i, out=None, space=MSpace.kObject):
    if isinstance(out, int) and not isinstance(out, bool):
        out, space = None, out
    return _out(out, _om2.MFnMesh.getPolygonNormal(self, i, space))


def _m_uvs(self, *args):
    # 1.0: (uArray, vArray, uvSet=None).
    if len(args) >= 2 and isinstance(args[0], list):
        u, v = _om2.MFnMesh.getUVs(self, args[2] if len(args) > 2 and args[2] else "")
        _arr_fill(args[0], u)
        _arr_fill(args[1], v)
        return None
    return _to1(_om2.MFnMesh.getUVs(self, *args))


def _m_assigned_uvs(self, *args):
    if len(args) >= 2 and isinstance(args[0], list):
        c, i = _om2.MFnMesh.getAssignedUVs(self, args[2] if len(args) > 2 and args[2] else "")
        _arr_fill(args[0], c)
        _arr_fill(args[1], i)
        return None
    return _to1(_om2.MFnMesh.getAssignedUVs(self, *args))


def _m_uv_set_names(self, arr=None):
    return _out(arr, _om2.MFnMesh.getUVSetNames(self))


def _m_current_uv_set(self, *args):
    name = _om2.MFnMesh.getCurrentUVSetName(self)
    if args and isinstance(args[0], list):
        args[0][:] = [name]
        return None
    return MString(name)


def _m_polygon_uv(self, face, vert, u=None, v=None, uv_set=""):
    uv = _om2.MFnMesh.getPolygonUV(self, face, vert, uv_set or "")
    if u is None:
        return uv
    _put(u, uv[0])
    _put(v, uv[1])


def _m_polygon_uv_id(self, face, vert, ptr=None, uv_set=""):
    i = _om2.MFnMesh.getPolygonUVid(self, face, vert, uv_set or "")
    if ptr is None:
        return i
    _put(ptr, i)


def _m_closest_point(self, point, out=None, space=MSpace.kObject, face_ptr=None, accel=None):
    if isinstance(out, int) and not isinstance(out, bool):
        out, space, face_ptr = None, out, space if isinstance(space, _Ptr) else None
    p, f = _om2.MFnMesh.getClosestPoint(self, point, space)
    if out is None:
        return _to1((p, f))
    _copy(out, p)
    _put(face_ptr, f)


def _m_closest_point_and_normal(self, point, out_p=None, out_n=None, space=MSpace.kObject, face_ptr=None, accel=None):
    p, n, f = _om2.MFnMesh.getClosestPointAndNormal(self, point, space)
    if out_p is None:
        return _to1((p, n, f))
    _copy(out_p, p)
    _copy(out_n, n)
    _put(face_ptr, f)


def _m_closest_normal(self, point, out=None, space=MSpace.kObject, face_ptr=None, accel=None):
    n, f = _om2.MFnMesh.getClosestNormal(self, point, space)
    if out is None:
        return _to1((n, f))
    _copy(out, n)
    _put(face_ptr, f)


def _ids_list(x):
    return None if x is None or (isinstance(x, list) and not x) and False else x


def _v1_ray_args(args):
    """The 1.0 ray calls start with faceIds (None or an array) where 2.0 starts with the space."""
    return len(args) >= 9 and not (isinstance(args[0], int) and not isinstance(args[0], bool))


def _ray_ids(x):
    return list(x) if x else None


def _m_closest_intersection(self, source, direction, *args, **kw):
    # 1.0: (raySource, rayDirection, faceIds, triIds, idsSorted, space, maxParam, testBothDirections, accelParams,
    #       hitPoint, hitRayParam*, hitFace*, hitTriangle*, hitBary1*, hitBary2*, tolerance) -> bool.
    if not _v1_ray_args(args):
        return _to1(_om2.MFnMesh.closestIntersection(self, source, direction, *args, **kw))
    face_ids, tri_ids, sorted_, space, max_param, both, accel = args[:7]
    outs = list(args[7:13])
    tol = args[13] if len(args) > 13 else kw.get("tolerance", 1e-6)
    hp, param, face, tri, b1, b2 = _om2.MFnMesh.closestIntersection(
        self, source, direction, space, max_param, both, _ray_ids(face_ids), _ray_ids(tri_ids), bool(sorted_), accel, tol
    )
    hit = face >= 0
    if hit:
        if outs and outs[0] is not None:
            _copy(outs[0], hp)
        for ptr, v in zip(outs[1:6], (param, face, tri, b1, b2)):
            _put(ptr, v)
    return hit


def _m_all_intersections(self, source, direction, *args, **kw):
    # 1.0: (raySource, rayDirection, faceIds, triIds, idsSorted, space, maxParam, testBothDirections, accelParams,
    #       sortHits, hitPoints, hitRayParams, hitFaces, hitTriangles, hitBary1s, hitBary2s, tolerance) -> bool.
    if not _v1_ray_args(args):
        return _to1(_om2.MFnMesh.allIntersections(self, source, direction, *args, **kw))
    face_ids, tri_ids, sorted_, space, max_param, both, accel, sort_hits = args[:8]
    outs = list(args[8:14])
    tol = args[14] if len(args) > 14 else kw.get("tolerance", 1e-6)
    got = _om2.MFnMesh.allIntersections(
        self, source, direction, space, max_param, both, _ray_ids(face_ids), _ray_ids(tri_ids), bool(sorted_), accel, bool(sort_hits), tol
    )
    for dst, src in zip(outs, got):
        if isinstance(dst, list):
            _arr_fill(dst, src)
    return len(got[0]) > 0


MFnMesh = _v1(
    _om2.MFnMesh,
    {
        "getPoints": _m_get_points,
        "getFloatPoints": _m_get_float_points,
        "getPoint": _m_get_point,
        "getVertices": _m_get_vertices,
        "getPolygonVertices": _m_poly_verts,
        "getEdgeVertices": _m_edge_verts,
        "getTriangles": _m_triangles,
        "getNormals": _m_normals,
        "getVertexNormal": _m_vertex_normal,
        "getVertexNormals": _m_vertex_normals,
        "getFaceVertexNormal": _m_face_vertex_normal,
        "getFaceVertexNormals": _m_face_vertex_normals,
        "getPolygonNormal": _m_poly_normal,
        "getUVs": _m_uvs,
        "getAssignedUVs": _m_assigned_uvs,
        "getUVSetNames": _m_uv_set_names,
        "getCurrentUVSetName": _m_current_uv_set,
        "getPolygonUV": _m_polygon_uv,
        "getPolygonUVid": _m_polygon_uv_id,
        "getClosestPoint": _m_closest_point,
        "getClosestPointAndNormal": _m_closest_point_and_normal,
        "getClosestNormal": _m_closest_normal,
        "closestIntersection": _m_closest_intersection,
        "anyIntersection": _m_closest_intersection,
        "allIntersections": _m_all_intersections,
        "setObject": _status_of(_om2.MFnMesh.setObject),
    },
)
MMeshIsectAccelParams = _v1(_om2.MMeshIsectAccelParams)


def _c_get_cvs(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MFnNurbsCurve.cvPositions(self, space))


def _c_get_cv(self, i, pt=None, space=MSpace.kObject):
    if isinstance(pt, int) and not isinstance(pt, bool):
        pt, space = None, pt
    return _out(pt, _om2.MFnNurbsCurve.cvPosition(self, i, space))


def _c_point_at_param(self, param, pt=None, space=MSpace.kObject):
    if isinstance(pt, int) and not isinstance(pt, bool):
        pt, space = None, pt
    return _out(pt, _om2.MFnNurbsCurve.getPointAtParam(self, param, space))


def _c_param_at_point(self, point, ptr=None, *args):
    if isinstance(ptr, _Ptr):
        tol = next((a for a in args if isinstance(a, float)), 0.001)
        space = next((a for a in args if isinstance(a, int) and not isinstance(a, bool)), MSpace.kObject)
        _put(ptr, _om2.MFnNurbsCurve.getParamAtPoint(self, point, tol, space))
        return None
    rest = ([ptr] if ptr is not None else []) + list(args)
    return _om2.MFnNurbsCurve.getParamAtPoint(self, point, *rest)


def _c_closest_point(self, point, *args):
    # 1.0: closestPoint(point, double* param=None, tolerance, space) -> MPoint.
    ptr = args[0] if args and (isinstance(args[0], _Ptr) or args[0] is None) else None
    rest = args[1:] if ptr is not None or (args and args[0] is None) else args
    tol = next((a for a in rest if isinstance(a, float)), 0.001)
    space = next((a for a in rest if isinstance(a, int) and not isinstance(a, bool)), MSpace.kObject)
    p, t = _om2.MFnNurbsCurve.closestPoint(self, point, False, tol, space)
    _put(ptr, t)
    return _to1(p) if (ptr is not None or args) else _to1((p, t))


def _c_knots(self, arr=None):
    return _out(arr, _om2.MFnNurbsCurve.knots(self))


MFnNurbsCurve = _v1(
    _om2.MFnNurbsCurve,
    {
        "getCVs": _c_get_cvs,
        "setCVs": lambda self, pts, space=MSpace.kObject: _om2.MFnNurbsCurve.setCVPositions(self, pts, space),
        "getCV": _c_get_cv,
        "setCV": lambda self, i, pt, space=MSpace.kObject: _om2.MFnNurbsCurve.setCVPosition(self, i, pt, space),
        "getPointAtParam": _c_point_at_param,
        "getParamAtPoint": _c_param_at_point,
        "closestPoint": _c_closest_point,
        "getKnots": _c_knots,
        "getKnotDomain": lambda self, a, b: (lambda d: (_put(a, d[0]), _put(b, d[1])))(_om2.MFnNurbsCurve.knotDomain(self)),
        "setObject": _status_of(_om2.MFnNurbsCurve.setObject),
    },
)


def _set_members(self, sel, flatten=False):
    if isinstance(sel, list) or isinstance(sel, _om2.MSelectionList):
        got = _om2.MFnSet.getMembers(self, flatten)
        _om2.MSelectionList.clear(sel)
        _om2.MSelectionList.merge(sel, got)
        return None
    return _to1(_om2.MFnSet.getMembers(self, sel))


MFnSet = _v1(_om2.MFnSet, {"getMembers": _set_members})
MFnComponent = _v1(_om2.MFnComponent)


def _sic_elements(self, arr=None):
    return _out(arr, _om2.MFnSingleIndexedComponent.getElements(self))


def _dic_elements(self, a=None, b=None):
    got = _om2.MFnDoubleIndexedComponent.getElements(self)
    if a is None:
        return _to1(got)
    _arr_fill(a, [x[0] for x in got])
    _arr_fill(b, [x[1] for x in got])


def _dic_element(self, i, a=None, b=None):
    got = _om2.MFnDoubleIndexedComponent.getElement(self, i)
    if a is None:
        return got
    _put(a, got[0])
    _put(b, got[1])


MFnSingleIndexedComponent = _v1(_om2.MFnSingleIndexedComponent, {"getElements": _sic_elements})
MFnDoubleIndexedComponent = _v1(_om2.MFnDoubleIndexedComponent, {"getElements": _dic_elements, "getElement": _dic_element})
MFnAttribute = _v1(_om2.MFnAttribute)
MFnNumericAttribute = _v1(_om2.MFnNumericAttribute)
MFnUnitAttribute = _v1(_om2.MFnUnitAttribute)
MFnTypedAttribute = _v1(_om2.MFnTypedAttribute)
MFnEnumAttribute = _v1(_om2.MFnEnumAttribute)
MFnCompoundAttribute = _v1(_om2.MFnCompoundAttribute)
MFnMatrixAttribute = _v1(_om2.MFnMatrixAttribute)
MFnMessageAttribute = _v1(_om2.MFnMessageAttribute)
MFnGenericAttribute = _v1(_om2.MFnGenericAttribute)
MFnData = _v1(_om2.MFnData)


def _nd_get_data(self, *ptrs):
    got = _om2.MFnNumericData.getData(self)
    if not ptrs:
        return got
    vals = got if isinstance(got, (list, tuple)) else [got]
    for p, v in zip(ptrs, vals):
        _put(p, v)


MFnNumericData = _v1(_om2.MFnNumericData, {"getData": _nd_get_data})
MFnMatrixData = _v1(_om2.MFnMatrixData)
MFnStringData = _v1(_om2.MFnStringData)

# ---------------------------------------------------------------------------------------------------------------------
# Iterators


def _it_set_index(base):
    def set_index(self, i, prev=None):
        p = base.setIndex(self, i)
        if prev is None:
            return p
        _put(prev, p)

    return set_index


def _it_ints(base, name):
    f = getattr(base, name)

    def call(self, arr=None):
        return _out(arr, f(self))

    return call


def _it_count(base, name):
    f = getattr(base, name)

    def call(self, ptr=None):
        n = f(self)
        if ptr is None:
            return n
        _put(ptr, n)

    return call


def _iv_normal(self, out=None, *args):
    if out is None or isinstance(out, int) and not isinstance(out, bool):
        return _to1(_om2.MItMeshVertex.getNormal(self, *([out] if out is not None else []) + list(args)))
    space = _space_of(*args)
    return _out(out, _om2.MItMeshVertex.getNormal(self, space))


def _iv_normals(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MItMeshVertex.getNormals(self, space))


def _iv_uv(self, *args):
    # 1.0: getUV(float2 ptr, uvSet=None).
    if args and isinstance(args[0], _Ptr):
        uv = _om2.MItMeshVertex.getUV(self, *args[1:])
        _put(args[0], uv[0], uv[1])
        return None
    return _om2.MItMeshVertex.getUV(self, *args)


def _iv_uvs(self, *args):
    if len(args) >= 2 and isinstance(args[0], list):
        got = _om2.MItMeshVertex.getUVs(self, args[3] if len(args) > 3 and args[3] else "")
        for dst, src in zip(args[:3], got):
            _arr_fill(dst, src)
        return None
    return _to1(_om2.MItMeshVertex.getUVs(self, *args))


MItMeshVertex = _v1(
    _om2.MItMeshVertex,
    {
        "setIndex": _it_set_index(_om2.MItMeshVertex),
        "getConnectedVertices": _it_ints(_om2.MItMeshVertex, "getConnectedVertices"),
        "getConnectedFaces": _it_ints(_om2.MItMeshVertex, "getConnectedFaces"),
        "getConnectedEdges": _it_ints(_om2.MItMeshVertex, "getConnectedEdges"),
        "numConnectedEdges": _it_count(_om2.MItMeshVertex, "numConnectedEdges"),
        "numConnectedFaces": _it_count(_om2.MItMeshVertex, "numConnectedFaces"),
        "numUVs": _it_count(_om2.MItMeshVertex, "numUVs"),
        "getNormal": _iv_normal,
        "getNormals": _iv_normals,
        "getUV": _iv_uv,
        "getUVs": _iv_uvs,
    },
)


def _ip_normal(self, out=None, *args):
    if out is None or isinstance(out, int) and not isinstance(out, bool):
        return _to1(_om2.MItMeshPolygon.getNormal(self, *([out] if out is not None else []) + list(args)))
    return _out(out, _om2.MItMeshPolygon.getNormal(self, _space_of(*args)))


def _ip_points(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MItMeshPolygon.getPoints(self, space))


def _ip_normals(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MItMeshPolygon.getNormals(self, space))


def _ip_area(self, ptr=None, space=MSpace.kObject):
    if isinstance(ptr, int) and not isinstance(ptr, bool):
        ptr, space = None, ptr
    a = _om2.MItMeshPolygon.getArea(self, space)
    if ptr is None:
        return a
    _put(ptr, a)


def _ip_triangles(self, pts=None, ids=None, space=MSpace.kObject):
    if isinstance(pts, int) and not isinstance(pts, bool):
        pts, space = None, pts
    p, v = _om2.MItMeshPolygon.getTriangles(self, space)
    if pts is None:
        return _to1((p, v))
    _arr_fill(pts, p)
    _arr_fill(ids, v)


def _ip_uv(self, which, *args):
    if args and isinstance(args[0], _Ptr):
        uv = _om2.MItMeshPolygon.getUV(self, which, *args[1:])
        _put(args[0], uv[0], uv[1])
        return None
    return _om2.MItMeshPolygon.getUV(self, which, *args)


def _ip_uvs(self, *args):
    if len(args) >= 2 and isinstance(args[0], list):
        u, v = _om2.MItMeshPolygon.getUVs(self, args[2] if len(args) > 2 and args[2] else "")
        _arr_fill(args[0], u)
        _arr_fill(args[1], v)
        return None
    return _to1(_om2.MItMeshPolygon.getUVs(self, *args))


def _ip_uv_index(self, which, *args):
    if args and isinstance(args[0], _Ptr):
        _put(args[0], _om2.MItMeshPolygon.getUVIndex(self, which, *args[1:]))
        return None
    return _om2.MItMeshPolygon.getUVIndex(self, which, *args)


MItMeshPolygon = _v1(
    _om2.MItMeshPolygon,
    {
        "setIndex": _it_set_index(_om2.MItMeshPolygon),
        "getVertices": _it_ints(_om2.MItMeshPolygon, "getVertices"),
        "getEdges": _it_ints(_om2.MItMeshPolygon, "getEdges"),
        "getConnectedFaces": _it_ints(_om2.MItMeshPolygon, "getConnectedFaces"),
        "getConnectedEdges": _it_ints(_om2.MItMeshPolygon, "getConnectedEdges"),
        "getConnectedVertices": _it_ints(_om2.MItMeshPolygon, "getConnectedVertices"),
        "numTriangles": _it_count(_om2.MItMeshPolygon, "numTriangles"),
        "numConnectedFaces": _it_count(_om2.MItMeshPolygon, "numConnectedFaces"),
        "numConnectedEdges": _it_count(_om2.MItMeshPolygon, "numConnectedEdges"),
        "numUVs": _it_count(_om2.MItMeshPolygon, "numUVs"),
        "getNormal": _ip_normal,
        "getNormals": _ip_normals,
        "getPoints": _ip_points,
        "getArea": _ip_area,
        "getTriangles": _ip_triangles,
        "getUV": _ip_uv,
        "getUVs": _ip_uvs,
        "getUVIndex": _ip_uv_index,
    },
)


def _ie_index(self, which=None):
    # 1.0: index() is the edge's own; index(0) and index(1) its vertices.
    if which is None:
        return _om2.MItMeshEdge.index(self)
    return _om2.MItMeshEdge.vertexId(self, which)


def _ie_length(self, ptr=None, space=MSpace.kObject):
    if isinstance(ptr, int) and not isinstance(ptr, bool):
        ptr, space = None, ptr
    n = _om2.MItMeshEdge.length(self, space)
    if ptr is None:
        return n
    _put(ptr, n)


MItMeshEdge = _v1(
    _om2.MItMeshEdge,
    {
        "index": _ie_index,
        "setIndex": _it_set_index(_om2.MItMeshEdge),
        "getConnectedFaces": _it_ints(_om2.MItMeshEdge, "getConnectedFaces"),
        "getConnectedEdges": _it_ints(_om2.MItMeshEdge, "getConnectedEdges"),
        "numConnectedFaces": _it_count(_om2.MItMeshEdge, "numConnectedFaces"),
        "numConnectedEdges": _it_count(_om2.MItMeshEdge, "numConnectedEdges"),
        "getLength": _ie_length,
    },
)


def _ifv_normal(self, out=None, space=MSpace.kObject):
    if isinstance(out, int) and not isinstance(out, bool):
        out, space = None, out
    return _out(out, _om2.MItMeshFaceVertex.getNormal(self, space))


def _ifv_uv(self, *args):
    if args and isinstance(args[0], _Ptr):
        uv = _om2.MItMeshFaceVertex.getUV(self, *args[1:])
        _put(args[0], uv[0], uv[1])
        return None
    return _om2.MItMeshFaceVertex.getUV(self, *args)


MItMeshFaceVertex = _v1(_om2.MItMeshFaceVertex, {"getNormal": _ifv_normal, "getUV": _ifv_uv})


def _ig_all(self, arr=None, space=MSpace.kObject):
    if isinstance(arr, int) and not isinstance(arr, bool):
        arr, space = None, arr
    return _out(arr, _om2.MItGeometry.allPositions(self, space))


MItGeometry = _v1(_om2.MItGeometry, {"allPositions": _ig_all})


def _isl_dag_path(self, path=None, comp=None):
    p = _om2.MItSelectionList.getDagPath(self)
    if path is None:
        return _to1(p)
    _copy(path, p)
    if comp is not None:
        _p, c = _om2.MItSelectionList.getComponent(self)
        _copy(comp, c)


def _isl_depend_node(self, obj=None):
    return _out(obj, _om2.MItSelectionList.getDependNode(self))


def _isl_plug(self, plug=None):
    return _out(plug, _om2.MItSelectionList.getPlug(self))


def _isl_strings(self, arr=None):
    return _out(arr, _om2.MItSelectionList.getStrings(self))


MItSelectionList = _v1(
    _om2.MItSelectionList,
    {"getDagPath": _isl_dag_path, "getDependNode": _isl_depend_node, "getPlug": _isl_plug, "getStrings": _isl_strings},
)


def _idag_path(self, path=None):
    return _out(path, _om2.MItDag.getPath(self))


def _idag_all_paths(self, arr):
    _arr_fill(arr, [_om2.MItDag.getPath(self)])


MItDag = _v1(_om2.MItDag, {"getPath": _idag_path, "getAllPaths": _idag_all_paths, "item": lambda self: _to1(_om2.MItDag.currentItem(self))})
MItDependencyNodes = _v1(_om2.MItDependencyNodes, {"item": lambda self: _to1(_om2.MItDependencyNodes.thisNode(self))})


def _idg_nodes(self, arr=None):
    return _out(arr, _om2.MItDependencyGraph.getNodesVisited(self))


def _idg_plugs(self, arr=None):
    return _out(arr, _om2.MItDependencyGraph.getPlugsVisited(self))


MItDependencyGraph = _v1(_om2.MItDependencyGraph, {"getNodesVisited": _idg_nodes, "getPlugsVisited": _idg_plugs})

# ---------------------------------------------------------------------------------------------------------------------
# Commands and plug-in data (the classes MPx classes in OpenMayaMPx work with)

MArgList = _v1(_om2.MArgList)
MSyntax = _v1(_om2.MSyntax)
MArgParser = _v1(_om2.MArgParser)


def _adb_objects(self, sel=None):
    got = _om2.MArgDatabase.getObjectList(self)
    if sel is None:
        return _to1(got)
    _om2.MSelectionList.clear(sel)
    _om2.MSelectionList.merge(sel, got)


def _adb_flag_sel(self, name, index, sel=None):
    got = _om2.MArgDatabase.flagArgumentMSelectionList(self, name, index)
    if sel is None:
        return _to1(got)
    _om2.MSelectionList.clear(sel)
    _om2.MSelectionList.merge(sel, got)


def _adb_object_strings(self, arr=None):
    return _out(arr, _om2.MArgDatabase.getObjectStrings(self))


def _adb_flag_argument(self, name, index, out=None):
    v = _om2.MArgDatabase.getFlagArgument(self, name, index)
    if out is None:
        return v
    if isinstance(out, list):
        out[:] = [v]
    else:
        _put(out, v)


MArgDatabase = _v1(
    _om2.MArgDatabase,
    {"getObjects": _adb_objects, "getObjectList": _adb_objects, "flagArgumentMSelectionList": _adb_flag_sel, "getObjectStrings": _adb_object_strings, "getFlagArgument": _adb_flag_argument},
)
MDataHandle = _v1(_om2.MDataHandle)
MArrayDataHandle = _v1(_om2.MArrayDataHandle)
MDataBlock = _v1(_om2.MDataBlock)
MDGContext = _v1(_om2.MDGContext)
MFnPlugin = _v1(_om2.MFnPlugin)

# The reference's 1.0 module also holds these names.
MStatus.kSuccess = 0
MFn.kInvalid = getattr(MFn, "kInvalid", 0)


def _all_names():
    return [n for n in globals() if n.startswith("M") or n == "kUnknownParameter"]


__all__ = _all_names()
