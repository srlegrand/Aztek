# A subset of Pixar's pxr module over Aztek's USD, and mayaUsd.ufe and mayaUsd.lib. A script that sticks to stages, prims,
# attributes, xform ops, layers and variants runs unchanged. What is here is listed in docs/USD.md; anything else raises
# AttributeError, as a missing name does.
import json
import sys
import types

import _aztek


def _op(name, *args):
    out = _aztek.call("usdPy", json.dumps([name] + [_plain(a) for a in args]), "{}")
    result = json.loads(out)
    if isinstance(result, dict) and "__error__" in result:
        raise RuntimeError(result["__error__"])
    return result


def _plain(v):
    if isinstance(v, Path):
        return v.pathString
    if isinstance(v, Matrix4d):
        return [x for row in v._rows for x in row]
    if isinstance(v, (list, tuple)):
        return [_plain(x) for x in v]
    if isinstance(v, TimeCode):
        return None if v.IsDefault() else v.GetValue()
    if isinstance(v, Prim):
        return v.GetPath().pathString
    return v


# ---------------------------------------------------------------------------------------------------------------------
# Gf and Vt


class _Vec(tuple):
    SIZE = 3

    def __new__(cls, *a):
        if len(a) == 1 and isinstance(a[0], (list, tuple)):
            a = tuple(a[0])
        if len(a) == 0:
            a = (0.0,) * cls.SIZE
        if len(a) == 1:
            a = (a[0],) * cls.SIZE
        return tuple.__new__(cls, [float(x) for x in a])

    def GetLength(self):
        return sum(x * x for x in self) ** 0.5

    def GetNormalized(self):
        n = self.GetLength() or 1.0
        return type(self)(*[x / n for x in self])

    def GetDot(self, o):
        return sum(a * b for a, b in zip(self, o))

    def __add__(self, o):
        return type(self)(*[a + b for a, b in zip(self, o)])

    def __sub__(self, o):
        return type(self)(*[a - b for a, b in zip(self, o)])

    def __mul__(self, k):
        return type(self)(*[a * k for a in self])

    def __neg__(self):
        return type(self)(*[-a for a in self])

    def __repr__(self):
        return "Gf.%s(%s)" % (type(self).__name__, ", ".join(repr(x) for x in self))


class Vec2f(_Vec):
    SIZE = 2


class Vec2d(_Vec):
    SIZE = 2


class Vec3f(_Vec):
    SIZE = 3


class Vec3d(_Vec):
    SIZE = 3


class Vec4f(_Vec):
    SIZE = 4


class Vec4d(_Vec):
    SIZE = 4


class Quatf:
    def __init__(self, w=1.0, x=0.0, y=0.0, z=0.0):
        if isinstance(x, (list, tuple)):
            x, y, z = x
        self.w, self.x, self.y, self.z = float(w), float(x), float(y), float(z)

    def GetReal(self):
        return self.w

    def GetImaginary(self):
        return Vec3f(self.x, self.y, self.z)

    def __iter__(self):
        return iter((self.w, self.x, self.y, self.z))


Quatd = Quatf


class Matrix4d:
    """Rows as USD writes them, with translation in the last row."""

    def __init__(self, *a):
        if len(a) == 0 or (len(a) == 1 and isinstance(a[0], (int, float))):
            k = float(a[0]) if a else 1.0
            self._rows = [[k if i == j else 0.0 for j in range(4)] for i in range(4)]
        elif len(a) == 16:
            self._rows = [[float(x) for x in a[i * 4:i * 4 + 4]] for i in range(4)]
        elif len(a) == 1 and len(a[0]) == 16:
            self._rows = [[float(x) for x in a[0][i * 4:i * 4 + 4]] for i in range(4)]
        else:
            self._rows = [[float(x) for x in r] for r in (a[0] if len(a) == 1 else a)]

    def SetTranslate(self, v):
        self.__init__()
        self._rows[3][:3] = [float(x) for x in v]
        return self

    def SetTranslateOnly(self, v):
        self._rows[3][:3] = [float(x) for x in v]
        return self

    def ExtractTranslation(self):
        return Vec3d(*self._rows[3][:3])

    def Transform(self, p):
        r = self._rows
        return Vec3d(*[p[0] * r[0][k] + p[1] * r[1][k] + p[2] * r[2][k] + r[3][k] for k in range(3)])

    def __mul__(self, o):
        return Matrix4d([[sum(self._rows[i][k] * o._rows[k][j] for k in range(4)) for j in range(4)] for i in range(4)])

    def __getitem__(self, i):
        return self._rows[i]

    def __eq__(self, o):
        return isinstance(o, Matrix4d) and self._rows == o._rows

    def __repr__(self):
        return "Gf.Matrix4d(%s)" % ", ".join(str(tuple(r)) for r in self._rows)


class _Array(list):
    pass


class Vec3fArray(_Array):
    pass


class Vec2fArray(_Array):
    pass


class IntArray(_Array):
    pass


class FloatArray(_Array):
    pass


class DoubleArray(_Array):
    pass


class TokenArray(_Array):
    pass


class StringArray(_Array):
    pass


class Matrix4dArray(_Array):
    pass


_ELEM = {
    "point3f": Vec3f, "vector3f": Vec3f, "normal3f": Vec3f, "color3f": Vec3f, "float3": Vec3f,
    "point3d": Vec3d, "vector3d": Vec3d, "normal3d": Vec3d, "color3d": Vec3d, "double3": Vec3d,
    "float2": Vec2f, "texCoord2f": Vec2f, "double2": Vec2d, "float4": Vec4f, "color4f": Vec4f, "double4": Vec4d,
}
_ARRAYS = {"int": IntArray, "float": FloatArray, "double": DoubleArray, "token": TokenArray, "string": StringArray}


def _wrap(ty, v):
    if v is None:
        return None
    if ty.endswith("[]"):
        e = ty[:-2]
        if e in _ELEM:
            return Vec3fArray([_ELEM[e](*x) for x in v]) if e in ("point3f", "vector3f", "normal3f", "color3f", "float3") else _Array([_ELEM[e](*x) for x in v])
        if e == "matrix4d":
            return Matrix4dArray([Matrix4d(x) for x in v])
        return _ARRAYS.get(e, _Array)(v)
    if ty in _ELEM:
        return _ELEM[ty](*v)
    if ty == "matrix4d":
        return Matrix4d(v)
    if ty in ("quatf", "quatd"):
        return Quatf(*v)
    return v


# ---------------------------------------------------------------------------------------------------------------------
# Sdf and Tf


class Path:
    absoluteRootPath = None
    emptyPath = None

    def __init__(self, s=""):
        self.pathString = s.pathString if isinstance(s, Path) else str(s)

    @property
    def name(self):
        return self.pathString.rsplit(".", 1)[-1] if self.IsPropertyPath() else self.pathString.rsplit("/", 1)[-1]

    def GetString(self):
        return self.pathString

    def IsAbsolutePath(self):
        return self.pathString.startswith("/")

    def IsAbsoluteRootPath(self):
        return self.pathString == "/"

    def IsPropertyPath(self):
        return "." in self.pathString.rsplit("/", 1)[-1]

    def IsPrimPath(self):
        return bool(self.pathString) and not self.IsPropertyPath()

    def IsEmpty(self):
        return not self.pathString

    def GetPrimPath(self):
        return Path(self.pathString.rsplit(".", 1)[0]) if self.IsPropertyPath() else self

    def GetParentPath(self):
        if self.IsPropertyPath():
            return self.GetPrimPath()
        if self.pathString in ("", "/"):
            return Path("")
        return Path(self.pathString.rsplit("/", 1)[0] or "/")

    def AppendChild(self, n):
        return Path((self.pathString.rstrip("/")) + "/" + str(n))

    def AppendPath(self, p):
        return self.AppendChild(str(p).lstrip("/"))

    def AppendProperty(self, n):
        return Path(self.pathString + "." + str(n))

    def HasPrefix(self, o):
        o = str(o).rstrip("/")
        return self.pathString == o or self.pathString.startswith(o + "/") or o == ""

    def ReplaceName(self, n):
        return self.GetParentPath().AppendChild(n)

    def __str__(self):
        return self.pathString

    def __repr__(self):
        return "Sdf.Path(%r)" % self.pathString

    def __eq__(self, o):
        return str(self) == str(o)

    def __hash__(self):
        return hash(self.pathString)

    def __lt__(self, o):
        return self.pathString < str(o)

    def __bool__(self):
        return bool(self.pathString)


Path.absoluteRootPath = Path("/")
Path.emptyPath = Path("")


class _TypeName(str):
    @property
    def scalarType(self):
        return _TypeName(self[:-2]) if self.endswith("[]") else self

    @property
    def arrayType(self):
        return self if self.endswith("[]") else _TypeName(self + "[]")

    @property
    def type(self):
        return self

    @property
    def isArray(self):
        return self.endswith("[]")


class _ValueTypeNames:
    pass


for _n, _t in {
    "Bool": "bool", "Int": "int", "Int64": "int64", "Float": "float", "Double": "double", "String": "string", "Token": "token", "Asset": "asset",
    "Float2": "float2", "Float3": "float3", "Float4": "float4", "Double2": "double2", "Double3": "double3", "Double4": "double4",
    "Color3f": "color3f", "Color3d": "color3d", "Color4f": "color4f", "Point3f": "point3f", "Point3d": "point3d", "Vector3f": "vector3f",
    "Vector3d": "vector3d", "Normal3f": "normal3f", "Normal3d": "normal3d", "TexCoord2f": "texCoord2f", "Matrix4d": "matrix4d", "Quatf": "quatf", "Quatd": "quatd",
}.items():
    setattr(_ValueTypeNames, _n, _TypeName(_t))
    setattr(_ValueTypeNames, _n + "Array", _TypeName(_t + "[]"))


class AssetPath:
    def __init__(self, path="", resolved=""):
        self.path = path
        self.resolvedPath = resolved


class Layer:
    def __init__(self, stage, identifier):
        self._stage = stage
        self.identifier = identifier

    @staticmethod
    def FindOrOpen(path):
        s = _op("open", str(path), True)
        return Layer(s, _op("rootLayer", s))

    @staticmethod
    def CreateNew(path):
        s = _op("createNew", str(path))
        return Layer(s, _op("rootLayer", s))

    @staticmethod
    def CreateAnonymous(tag=""):
        s = _op("createInMemory")
        return Layer(s, _op("rootLayer", s))

    def _info(self):
        return _op("layerInfo", self._stage, self.identifier)

    @property
    def realPath(self):
        return self._info()["realPath"]

    @property
    def anonymous(self):
        return self._info()["anonymous"]

    @property
    def dirty(self):
        return self._info()["dirty"]

    @property
    def subLayerPaths(self):
        return self._info()["subLayers"]

    def InsertSubLayerPath(self, path, index=0):
        return _op("layerAddSub", self._stage, self.identifier, str(path), index)

    def ExportToString(self):
        return _op("layerText", self._stage, self.identifier)

    def ImportFromString(self, text):
        return _op("layerImportText", self._stage, self.identifier, text)

    def Export(self, path):
        return _op("layerExport", self._stage, self.identifier, str(path))

    def Save(self, force=False):
        return _op("layerSave", self._stage, self.identifier)


# ---------------------------------------------------------------------------------------------------------------------
# Usd


class TimeCode:
    def __init__(self, t=None):
        self._t = t._t if isinstance(t, TimeCode) else t

    @staticmethod
    def Default():
        return TimeCode(None)

    @staticmethod
    def EarliestTime():
        return TimeCode(-1.7976931348623157e308)

    def IsDefault(self):
        return self._t is None

    def GetValue(self):
        return self._t


def _time(t):
    if t is None:
        return None
    return t.GetValue() if isinstance(t, TimeCode) else float(t)


class Attribute:
    def __init__(self, stage, prim_path, name):
        self._s, self._p, self._n = stage, prim_path, name

    @property
    def _path(self):
        return self._p + "." + self._n

    def GetName(self):
        return self._n

    def GetBaseName(self):
        return self._n.rsplit(":", 1)[-1]

    def GetPath(self):
        return Path(self._path)

    def GetPrim(self):
        return Prim(self._s, self._p)

    def _info(self):
        return _op("attrInfo", self._s, self._path)

    def GetTypeName(self):
        return _TypeName(self._info()["type"])

    def IsValid(self):
        return self._info()["defined"]

    def __bool__(self):
        return self.IsValid()

    def IsAuthored(self):
        return self.HasAuthoredValue()

    def HasAuthoredValue(self):
        return self._info()["authored"]

    def HasValue(self):
        return self.Get() is not None

    def IsCustom(self):
        return self._info()["custom"]

    def GetTimeSamples(self):
        return self._info()["samples"]

    def GetNumTimeSamples(self):
        return len(self.GetTimeSamples())

    def ValueMightBeTimeVarying(self):
        return self.GetNumTimeSamples() > 1

    def Get(self, time=None):
        info = self._info()
        return _wrap(info["type"], _op("attrGet", self._s, self._path, _time(time)))

    def Set(self, value, time=None):
        return _op("attrSet", self._s, self._path, value, _time(time))

    def Clear(self):
        return _op("attrClear", self._s, self._path)

    def SetMetadata(self, key, value):
        return _op("attrMeta", self._s, self._path, key, value)

    def GetMetadata(self, key):
        if key == "interpolation":
            return self._info()["interpolation"]
        return None


class Relationship:
    def __init__(self, stage, prim_path, name):
        self._s, self._p, self._n = stage, prim_path, name

    def GetName(self):
        return self._n

    def GetTargets(self):
        return [Path(x) for x in _op("relTargets", self._s, self._p + "." + self._n)]

    def SetTargets(self, targets):
        return _op("relSet", self._s, self._p + "." + self._n, [str(t) for t in targets])

    def AddTarget(self, t):
        return self.SetTargets(self.GetTargets() + [t])


class VariantSet:
    def __init__(self, prim, name):
        self._prim, self._name = prim, name

    def _info(self):
        for v in _op("variants", self._prim._s, self._prim._p):
            if v["name"] == self._name:
                return v
        return {"name": self._name, "options": [], "selection": ""}

    def GetName(self):
        return self._name

    def GetVariantNames(self):
        return self._info()["options"]

    def GetVariantSelection(self):
        return self._info()["selection"]

    def SetVariantSelection(self, sel):
        _op("setVariant", self._prim._s, self._prim._p, self._name, sel)
        return True


class VariantSets:
    def __init__(self, prim):
        self._prim = prim

    def GetNames(self):
        return [v["name"] for v in _op("variants", self._prim._s, self._prim._p)]

    def GetVariantSet(self, name):
        return VariantSet(self._prim, name)

    def GetVariantSelection(self, name):
        return VariantSet(self._prim, name).GetVariantSelection()

    def SetSelection(self, name, sel):
        return VariantSet(self._prim, name).SetVariantSelection(sel)


class _Arcs:
    def __init__(self, prim, op):
        self._prim, self._opname = prim, op

    def AddReference(self, asset, prim_path=""):
        _op(self._opname, self._prim._s, self._prim._p, str(asset), str(prim_path))
        return True

    AddPayload = AddReference


class Prim:
    def __init__(self, stage, path):
        self._s, self._p = stage, str(path)

    def _info(self):
        return _op("primInfo", self._s, self._p)

    def IsValid(self):
        return self._info() is not None

    def __bool__(self):
        return self.IsValid()

    def __eq__(self, o):
        return isinstance(o, Prim) and self._s == o._s and self._p == o._p

    def __hash__(self):
        return hash((self._s, self._p))

    def __repr__(self):
        return "Usd.Prim(<%s>)" % self._p

    def GetStage(self):
        return Stage(self._s)

    def GetPath(self):
        return Path(self._p)

    GetPrimPath = GetPath

    def GetName(self):
        return self._p.rsplit("/", 1)[-1]

    def GetParent(self):
        up = self._p.rsplit("/", 1)[0]
        return Prim(self._s, up or "/")

    def GetTypeName(self):
        return (self._info() or {}).get("type", "")

    def SetTypeName(self, t):
        _op("primSet", self._s, self._p, "typeName", str(t))

    def IsA(self, schema):
        names = [c.__name__ for c in getattr(schema, "__mro__", [schema])]
        own = self.GetTypeName()
        return own in names or own in getattr(schema, "_TYPES", ())

    def HasAPI(self, schema):
        name = schema if isinstance(schema, str) else schema.__name__
        return name in (self._info() or {}).get("apiSchemas", [])

    def ApplyAPI(self, schema):
        _op("primSet", self._s, self._p, "applyAPI", schema if isinstance(schema, str) else schema.__name__)

    def GetSpecifier(self):
        return (self._info() or {}).get("specifier", "")

    def IsActive(self):
        return (self._info() or {}).get("active", False)

    def SetActive(self, on):
        _op("primSet", self._s, self._p, "active", bool(on))

    def IsLoaded(self):
        return (self._info() or {}).get("loaded", False)

    def Load(self):
        _op("load", self._s, self._p)

    def Unload(self):
        _op("unload", self._s, self._p)

    def IsInstance(self):
        return (self._info() or {}).get("instance", False)

    def IsInstanceable(self):
        return (self._info() or {}).get("instanceable", False)

    def SetInstanceable(self, on):
        _op("primSet", self._s, self._p, "instanceable", bool(on))

    def GetKind(self):
        return (self._info() or {}).get("kind", "")

    def SetKind(self, k):
        _op("primSet", self._s, self._p, "kind", str(k))

    def GetChildren(self):
        return [Prim(self._s, self._child(n)) for n in (self._info() or {}).get("children", [])]

    GetAllChildren = GetChildren

    def GetChildrenNames(self):
        return (self._info() or {}).get("children", [])

    def _child(self, n):
        return self._p.rstrip("/") + "/" + n

    def GetChild(self, n):
        return Prim(self._s, self._child(n))

    def GetPropertyNames(self):
        return (self._info() or {}).get("properties", [])

    def GetAttributes(self):
        out = []
        for n in self.GetPropertyNames():
            a = Attribute(self._s, self._p, n)
            if a._info()["defined"]:
                out.append(a)
        return out

    GetAuthoredAttributes = GetAttributes

    def GetAttribute(self, name):
        return Attribute(self._s, self._p, str(name))

    def HasAttribute(self, name):
        return Attribute(self._s, self._p, str(name)).IsValid()

    def CreateAttribute(self, name, type_name, custom=True, variability=None):
        _op("attrCreate", self._s, self._p + "." + str(name), str(type_name), bool(custom), variability == "uniform")
        return Attribute(self._s, self._p, str(name))

    def GetRelationship(self, name):
        return Relationship(self._s, self._p, str(name))

    def CreateRelationship(self, name, custom=True):
        return Relationship(self._s, self._p, str(name))

    def GetVariantSets(self):
        return VariantSets(self)

    def GetVariantSet(self, name):
        return VariantSet(self, name)

    def GetReferences(self):
        return _Arcs(self, "addReference")

    def GetPayloads(self):
        return _Arcs(self, "addPayload")


class EditTarget:
    def __init__(self, layer):
        self._layer = layer

    def GetLayer(self):
        return self._layer


class Stage:
    LoadAll = "all"
    LoadNone = "none"

    def __init__(self, stage_id):
        self._id = stage_id

    def __eq__(self, o):
        return isinstance(o, Stage) and self._id == o._id

    def __hash__(self):
        return hash(self._id)

    @staticmethod
    def Open(path, load="all"):
        p = path.identifier if isinstance(path, Layer) else str(path)
        return Stage(_op("open", p, load != "none"))

    @staticmethod
    def CreateNew(path):
        return Stage(_op("createNew", str(path)))

    @staticmethod
    def CreateInMemory(tag=""):
        return Stage(_op("createInMemory"))

    def GetRootLayer(self):
        return Layer(self._id, _op("rootLayer", self._id))

    def GetSessionLayer(self):
        return Layer(self._id, _op("sessionLayer", self._id))

    def GetLayerStack(self, includeSessionLayers=True):
        return [Layer(self._id, x) for x in _op("layerStack", self._id)]

    def GetEditTarget(self):
        return EditTarget(Layer(self._id, _op("editTarget", self._id)))

    def SetEditTarget(self, target):
        layer = target.GetLayer() if isinstance(target, EditTarget) else target
        _op("setEditTarget", self._id, layer.identifier)

    def GetEditTargetForLocalLayer(self, layer):
        return EditTarget(layer)

    def Save(self):
        _op("save", self._id)

    def Export(self, path):
        return _op("export", self._id, str(path))

    def ExportToString(self):
        return _op("exportToString", self._id)

    def Flatten(self):
        return _op("exportToString", self._id)

    def GetPseudoRoot(self):
        return Prim(self._id, "/")

    def GetPrimAtPath(self, path):
        return Prim(self._id, str(path))

    def DefinePrim(self, path, type_name=""):
        _op("define", self._id, str(path), str(type_name))
        return Prim(self._id, str(path))

    def OverridePrim(self, path):
        _op("override", self._id, str(path), "")
        return Prim(self._id, str(path))

    def RemovePrim(self, path):
        return _op("removePrim", self._id, str(path))

    def Traverse(self, *a):
        return iter([Prim(self._id, p) for p in _op("traverse", self._id, False)])

    def TraverseAll(self):
        return iter([Prim(self._id, p) for p in _op("traverse", self._id, True)])

    def GetDefaultPrim(self):
        n = _op("defaultPrim", self._id)
        return Prim(self._id, "/" + n) if n else Prim(self._id, "")

    def SetDefaultPrim(self, prim):
        _op("setDefaultPrim", self._id, prim.GetName())

    def HasDefaultPrim(self):
        return bool(_op("defaultPrim", self._id))

    def GetStartTimeCode(self):
        return _op("timeRange", self._id)[0]

    def GetEndTimeCode(self):
        return _op("timeRange", self._id)[1]

    def GetTimeCodesPerSecond(self):
        return _op("timeRange", self._id)[2]

    def SetStartTimeCode(self, t):
        _op("setTimeRange", self._id, t, None)

    def SetEndTimeCode(self, t):
        _op("setTimeRange", self._id, None, t)

    def GetMetadata(self, key):
        return _op("metadataGet", self._id, key)

    def SetMetadata(self, key, value):
        return _op("metadataSet", self._id, key, value)


# ---------------------------------------------------------------------------------------------------------------------
# UsdGeom


class _Schema:
    _TYPE = ""
    _TYPES = ()
    _ATTRS = {}

    def __init__(self, prim=None):
        self._prim = prim if isinstance(prim, Prim) else None

    @classmethod
    def Define(cls, stage, path):
        return cls(stage.DefinePrim(path, cls._TYPE))

    @classmethod
    def Get(cls, stage, path):
        return cls(stage.GetPrimAtPath(path))

    def GetPrim(self):
        return self._prim

    def GetPath(self):
        return self._prim.GetPath()

    def __bool__(self):
        return self._prim is not None and self._prim.IsValid()

    def __getattr__(self, name):
        # GetPointsAttr and CreatePointsAttr, from the schema's table.
        for prefix in ("Create", "Get"):
            if name.startswith(prefix) and name.endswith("Attr"):
                key = name[len(prefix):-4]
                for cls in type(self).__mro__:
                    spec = cls.__dict__.get("_ATTRS", {}).get(key)
                    if spec:
                        return self._attr_call(prefix, spec)
        raise AttributeError(name)

    def _attr_call(self, prefix, spec):
        attr_name, ty = spec[0], spec[1]
        uniform = len(spec) > 2

        def run(value=None, write_sparsely=False):
            a = self._prim.GetAttribute(attr_name)
            if prefix == "Create":
                a = self._prim.CreateAttribute(attr_name, ty, False, "uniform" if uniform else None)
                if value is not None:
                    a.Set(value)
            return a

        return run


class Imageable(_Schema):
    _ATTRS = {"Visibility": ("visibility", "token"), "Purpose": ("purpose", "token", "uniform")}

    def MakeVisible(self, time=None):
        self.CreateVisibilityAttr().Set("inherited", time)

    def MakeInvisible(self, time=None):
        self.CreateVisibilityAttr().Set("invisible", time)

    def ComputeVisibility(self, time=None):
        p = self._prim
        while p.GetPath().pathString not in ("", "/"):
            if p.GetAttribute("visibility").Get(time) == "invisible":
                return "invisible"
            p = p.GetParent()
        return "inherited"


class XformOp:
    def __init__(self, attr, inverse=False):
        self._attr, self._inverse = attr, inverse

    def GetAttr(self):
        return self._attr

    def GetName(self):
        return self._attr.GetName()

    def GetOpName(self):
        return ("!invert!" if self._inverse else "") + self._attr.GetName()

    def GetOpType(self):
        return self._attr.GetName().split(":")[1]

    def Get(self, time=None):
        return self._attr.Get(time)

    def Set(self, value, time=None):
        return self._attr.Set(value, time)


class Xformable(Imageable):
    _ATTRS = {"XformOpOrder": ("xformOpOrder", "token[]", "uniform")}

    def _add(self, kind, ty, suffix=""):
        name = "xformOp:" + kind + (":" + suffix if suffix else "")
        a = self._prim.CreateAttribute(name, ty, False)
        order = self._prim.GetAttribute("xformOpOrder").Get() or []
        self._prim.CreateAttribute("xformOpOrder", "token[]", False, "uniform").Set(list(order) + [name])
        return XformOp(a)

    def AddTranslateOp(self, precision=None, opSuffix=""):
        return self._add("translate", "double3", opSuffix)

    def AddScaleOp(self, precision=None, opSuffix=""):
        return self._add("scale", "float3", opSuffix)

    def AddOrientOp(self, precision=None, opSuffix=""):
        return self._add("orient", "quatf", opSuffix)

    def AddTransformOp(self, precision=None, opSuffix=""):
        return self._add("transform", "matrix4d", opSuffix)

    def GetOrderedXformOps(self):
        out = []
        for n in self._prim.GetAttribute("xformOpOrder").Get() or []:
            inv = n.startswith("!invert!")
            out.append(XformOp(self._prim.GetAttribute(n[8:] if inv else n), inv))
        return out

    def SetXformOpOrder(self, ops):
        self._prim.CreateAttribute("xformOpOrder", "token[]", False, "uniform").Set([o.GetOpName() for o in ops])

    def ClearXformOpOrder(self):
        self._prim.CreateAttribute("xformOpOrder", "token[]", False, "uniform").Set([])

    def GetLocalTransformation(self, time=None):
        import aztek.cmds as _c  # noqa
        m = Matrix4d()
        for op in self.GetOrderedXformOps():
            m = _op_matrix(op, time) * m
        return m


for _axes in ("XYZ", "XZY", "YXZ", "YZX", "ZXY", "ZYX"):
    setattr(Xformable, "AddRotate%sOp" % _axes, (lambda ax: lambda self, precision=None, opSuffix="": self._add("rotate" + ax, "float3", opSuffix))(_axes))
for _axis in "XYZ":
    setattr(Xformable, "AddRotate%sOp" % _axis, (lambda ax: lambda self, precision=None, opSuffix="": self._add("rotate" + ax, "float", opSuffix))(_axis))


def _op_matrix(op, time):
    import math
    v = op.Get(time)
    kind = op.GetOpType()
    m = Matrix4d()
    if kind == "translate":
        m.SetTranslate(v)
    elif kind == "scale":
        for i in range(3):
            m._rows[i][i] = float(v[i])
    elif kind == "transform":
        m = v
    elif kind.startswith("rotate"):
        axes = kind[6:]
        vals = v if isinstance(v, (tuple, list)) else [v]
        for ax, deg in zip(axes, vals):
            c, s = math.cos(math.radians(deg)), math.sin(math.radians(deg))
            r = Matrix4d()
            i, j = {"X": (1, 2), "Y": (2, 0), "Z": (0, 1)}[ax]
            r._rows[i][i], r._rows[i][j], r._rows[j][i], r._rows[j][j] = c, s, -s, c
            m = r * m
    if op._inverse and kind == "translate":
        m.SetTranslate((-v[0], -v[1], -v[2]))
    return m


class Xform(Xformable):
    _TYPE = "Xform"
    _TYPES = ("Xform",)


class Scope(Imageable):
    _TYPE = "Scope"
    _TYPES = ("Scope",)


class Gprim(Xformable):
    _ATTRS = {"DisplayColor": ("primvars:displayColor", "color3f[]"), "DisplayOpacity": ("primvars:displayOpacity", "float[]"), "DoubleSided": ("doubleSided", "bool", "uniform"), "Orientation": ("orientation", "token", "uniform"), "Extent": ("extent", "float3[]")}


class Mesh(Gprim):
    _TYPE = "Mesh"
    _TYPES = ("Mesh",)
    _ATTRS = {
        "Points": ("points", "point3f[]"),
        "Normals": ("normals", "normal3f[]"),
        "FaceVertexCounts": ("faceVertexCounts", "int[]"),
        "FaceVertexIndices": ("faceVertexIndices", "int[]"),
        "SubdivisionScheme": ("subdivisionScheme", "token", "uniform"),
        "HoleIndices": ("holeIndices", "int[]"),
        "CreaseIndices": ("creaseIndices", "int[]"),
        "CreaseLengths": ("creaseLengths", "int[]"),
        "CreaseSharpnesses": ("creaseSharpnesses", "float[]"),
    }


def _shape(name, attrs):
    return type(name, (Gprim,), {"_TYPE": name, "_TYPES": (name,), "_ATTRS": attrs})


Cube = _shape("Cube", {"Size": ("size", "double")})
Sphere = _shape("Sphere", {"Radius": ("radius", "double")})
Cylinder = _shape("Cylinder", {"Radius": ("radius", "double"), "Height": ("height", "double"), "Axis": ("axis", "token", "uniform")})
Cone = _shape("Cone", {"Radius": ("radius", "double"), "Height": ("height", "double"), "Axis": ("axis", "token", "uniform")})
Capsule = _shape("Capsule", {"Radius": ("radius", "double"), "Height": ("height", "double"), "Axis": ("axis", "token", "uniform")})


class Camera(Xformable):
    _TYPE = "Camera"
    _TYPES = ("Camera",)
    _ATTRS = {
        "FocalLength": ("focalLength", "float"), "HorizontalAperture": ("horizontalAperture", "float"), "VerticalAperture": ("verticalAperture", "float"),
        "ClippingRange": ("clippingRange", "float2"), "FocusDistance": ("focusDistance", "float"), "FStop": ("fStop", "float"), "Projection": ("projection", "token"),
    }


class Primvar:
    def __init__(self, attr):
        self._attr = attr

    def GetAttr(self):
        return self._attr

    def GetPrimvarName(self):
        return self._attr.GetName().split(":", 1)[1]

    def GetInterpolation(self):
        return self._attr.GetMetadata("interpolation") or "constant"

    def SetInterpolation(self, i):
        self._attr.SetMetadata("interpolation", i)

    def Get(self, time=None):
        return self._attr.Get(time)

    def Set(self, v, time=None):
        return self._attr.Set(v, time)

    def SetIndices(self, ix, time=None):
        p = self._attr.GetPrim()
        p.CreateAttribute(self._attr.GetName() + ":indices", "int[]", False).Set(list(ix), time)

    def GetIndices(self, time=None):
        return self._attr.GetPrim().GetAttribute(self._attr.GetName() + ":indices").Get(time)

    def HasValue(self):
        return self._attr.HasValue()


class PrimvarsAPI(_Schema):
    def __init__(self, prim):
        super().__init__(prim.GetPrim() if hasattr(prim, "GetPrim") else prim)

    def CreatePrimvar(self, name, ty, interpolation=None, elementSize=-1):
        a = self._prim.CreateAttribute("primvars:" + name, str(ty), False)
        pv = Primvar(a)
        if interpolation:
            pv.SetInterpolation(interpolation)
        if elementSize > 0:
            a.SetMetadata("elementSize", elementSize)
        return pv

    def GetPrimvar(self, name):
        return Primvar(self._prim.GetAttribute("primvars:" + name))

    def GetPrimvars(self):
        return [Primvar(self._prim.GetAttribute(n)) for n in self._prim.GetPropertyNames() if n.startswith("primvars:") and not n.endswith(":indices")]


class Tokens:
    y = "Y"
    z = "Z"
    x = "X"
    faceVarying = "faceVarying"
    vertex = "vertex"
    uniform = "uniform"
    varying = "varying"
    constant = "constant"
    inherited = "inherited"
    invisible = "invisible"
    default_ = "default"
    render = "render"
    proxy = "proxy"
    guide = "guide"
    catmullClark = "catmullClark"
    none = "none"
    loop = "loop"
    bilinear = "bilinear"
    rightHanded = "rightHanded"
    leftHanded = "leftHanded"
    perspective = "perspective"
    orthographic = "orthographic"


def GetStageUpAxis(stage):
    return stage.GetMetadata("upAxis") or "Y"


def SetStageUpAxis(stage, axis):
    stage.SetMetadata("upAxis", axis)
    return True


def GetStageMetersPerUnit(stage):
    return stage.GetMetadata("metersPerUnit") or 0.01


def SetStageMetersPerUnit(stage, v):
    stage.SetMetadata("metersPerUnit", float(v))
    return True


class XformCache:
    def __init__(self, time=None):
        self._t = time

    def SetTime(self, t):
        self._t = t

    def GetLocalTransformation(self, prim):
        return Xformable(prim).GetLocalTransformation(self._t), False

    def GetLocalToWorldTransform(self, prim):
        m = Matrix4d()
        p = prim
        while p.GetPath().pathString not in ("", "/"):
            m = m * Xformable(p).GetLocalTransformation(self._t)
            p = p.GetParent()
        return m


# ---------------------------------------------------------------------------------------------------------------------
# UsdShade, narrowly: what binding a material takes.


class Material(_Schema):
    _TYPE = "Material"
    _TYPES = ("Material",)


class Shader(_Schema):
    _TYPE = "Shader"
    _TYPES = ("Shader",)

    def CreateIdAttr(self, value=None):
        a = self._prim.CreateAttribute("info:id", "token", False, "uniform")
        if value is not None:
            a.Set(value)
        return a


class MaterialBindingAPI(_Schema):
    @classmethod
    def Apply(cls, prim):
        prim.ApplyAPI("MaterialBindingAPI")
        return cls(prim)

    def Bind(self, material):
        p = material.GetPath() if hasattr(material, "GetPath") else material
        _op("bindMaterial", self._prim._s, self._prim._p, str(p), "")
        return True


# ---------------------------------------------------------------------------------------------------------------------
# Modules


def _module(name, **items):
    m = types.ModuleType(name)
    m.__dict__.update(items)
    sys.modules[name] = m
    return m


class ErrorException(RuntimeError):
    pass


_gf = _module("pxr.Gf", Vec2f=Vec2f, Vec2d=Vec2d, Vec3f=Vec3f, Vec3d=Vec3d, Vec4f=Vec4f, Vec4d=Vec4d, Quatf=Quatf, Quatd=Quatd, Matrix4d=Matrix4d)
_vt = _module("pxr.Vt", Vec3fArray=Vec3fArray, Vec2fArray=Vec2fArray, IntArray=IntArray, FloatArray=FloatArray, DoubleArray=DoubleArray, TokenArray=TokenArray, StringArray=StringArray, Matrix4dArray=Matrix4dArray)
_tf = _module("pxr.Tf", ErrorException=ErrorException, MakeValidIdentifier=lambda s: "".join(c if c.isalnum() or c == "_" else "_" for c in s))
_sdf = _module("pxr.Sdf", Path=Path, Layer=Layer, AssetPath=AssetPath, ValueTypeNames=_ValueTypeNames, VariabilityUniform="uniform", VariabilityVarying="varying")
_usd = _module("pxr.Usd", Stage=Stage, Prim=Prim, Attribute=Attribute, Relationship=Relationship, TimeCode=TimeCode, EditTarget=EditTarget, VariantSets=VariantSets, VariantSet=VariantSet)
_geom = _module(
    "pxr.UsdGeom",
    Imageable=Imageable, Xformable=Xformable, Xform=Xform, Scope=Scope, Gprim=Gprim, Mesh=Mesh, Cube=Cube, Sphere=Sphere, Cylinder=Cylinder, Cone=Cone, Capsule=Capsule, Camera=Camera,
    XformOp=XformOp, Primvar=Primvar, PrimvarsAPI=PrimvarsAPI, Tokens=Tokens, GetStageUpAxis=GetStageUpAxis, SetStageUpAxis=SetStageUpAxis,
    GetStageMetersPerUnit=GetStageMetersPerUnit, SetStageMetersPerUnit=SetStageMetersPerUnit, XformCache=XformCache,
)
_shade = _module("pxr.UsdShade", Material=Material, Shader=Shader, MaterialBindingAPI=MaterialBindingAPI)
_ar = _module("pxr.Ar", SetResolveHook=_aztek.set_usd_resolve_hook)
_pxr = _module("pxr", Usd=_usd, UsdGeom=_geom, Sdf=_sdf, Gf=_gf, Vt=_vt, Tf=_tf, UsdShade=_shade, Ar=_ar)
_pxr.__path__ = []


class TranslatorBase:
    """A schema translator, as the Animal Logic USD plug-in has them: subclass, name the prim type, and read prims into nodes."""

    primType = ""

    def importObject(self, prim, parent):
        """Make nodes under the transform named `parent` for a prim."""
        raise NotImplementedError

    @classmethod
    def register(cls):
        _translators[cls.primType] = cls
        code = "import _aztek_pxr_shim; _aztek_pxr_shim._run_translator('%s', '{file}', '{prim}', '{parent}')" % cls.primType
        _aztek.call("usdRegisterTranslator", "[]", json.dumps({"type": cls.primType, "command": code}))

    @classmethod
    def unregister(cls):
        _translators.pop(cls.primType, None)
        _aztek.call("usdRegisterTranslator", "[]", json.dumps({"type": cls.primType, "remove": True}))


_translators = {}


def _run_translator(type_name, file, prim, parent):
    stage = Stage.Open(file)
    _translators[type_name]().importObject(stage.GetPrimAtPath(prim), parent)


_al = _module("AL")
_al.__path__ = []
_alu = _module("AL.usdmaya", TranslatorBase=TranslatorBase)
_al.usdmaya = _alu
_aztek_usd = _module("aztek.usd", TranslatorBase=TranslatorBase)


def _get_stage(proxy):
    """mayaUsd.ufe.getStage: the stage a proxy shows, from its name or a |proxy,/prim path."""
    name = str(proxy).split(",/")[0]
    return Stage(_op("ofProxy", name))


def _get_prim(path):
    """The prim of a |proxy,/prim path."""
    name, _, prim = str(path).partition(",/")
    return Prim(_op("ofProxy", name), "/" + prim)


_ufe = _module("mayaUsd.ufe", getStage=_get_stage, ufePathToPrim=_get_prim, usdPathFromUfePath=lambda p: Path("/" + str(p).partition(",/")[2]))
_lib = _module("mayaUsd.lib", GetPrim=_get_prim, mayaUsdLibVersion=lambda: 1)
_mu = _module("mayaUsd", ufe=_ufe, lib=_lib)
_mu.__path__ = []
del _gf, _vt, _tf, _sdf, _usd, _geom, _shade, _pxr, _ufe, _lib, _mu, _ar, _alu, _al
