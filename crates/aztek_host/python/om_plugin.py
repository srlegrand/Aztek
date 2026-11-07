# aztek.api.OpenMaya, plugins: MPxCommand, MSyntax, MArgList, MArgDatabase, MPxNode, MDataBlock, MFnPlugin, and the
# entry points the host calls: loadPlugin, unloadPlugin, pluginInfo, running commands, computing nodes.
import os as _os
import traceback as _traceback


class MTypeId:
    def __init__(self, a=0, b=None):
        if isinstance(a, MTypeId):
            self._id = a._id
        elif b is not None:
            self._id = (int(a) << 8) | int(b)
        else:
            self._id = int(a)

    def id(self):
        return self._id

    def __eq__(self, o):
        return isinstance(o, MTypeId) and self._id == o._id

    def __ne__(self, o):
        return not self == o

    def __hash__(self):
        return hash(self._id)

    def __repr__(self):
        return "aztek.api.OpenMaya.MTypeId(0x%x)" % self._id


def _type_id_of(type_name):
    e = _node_classes.get(type_name)
    return MTypeId(e["id"]) if e else MTypeId(0)


def _node_type_of_id(type_id):
    for k, e in _node_classes.items():
        if e["id"] == type_id:
            return k
    _fail(message="No node type has that id")


class MArgList:
    def __init__(self, items=None):
        self._a = list(items) if items is not None else []

    def __len__(self):
        return len(self._a)

    def length(self):
        return len(self._a)

    def _at(self, i):
        if not 0 <= i < len(self._a):
            raise IndexError("MArgList index out of range")
        return self._a[i]

    def addArg(self, v):
        self._a.append(v)
        return self

    def flagIndex(self, shortFlag, longFlag=None):
        names = {"-" + shortFlag.lstrip("-")}
        if longFlag:
            names.add("-" + longFlag.lstrip("-"))
        for i, v in enumerate(self._a):
            if isinstance(v, str) and v in names:
                return i
        return 4294967295

    def asBool(self, i):
        v = self._at(i)
        return v.lower() in ("true", "on", "yes", "1") if isinstance(v, str) else bool(v)

    def asInt(self, i):
        return int(self._at(i))

    def asDouble(self, i):
        return float(self._at(i))

    asFloat = asDouble

    def asString(self, i):
        return str(self._at(i))

    def asAngle(self, i):
        return MAngle(float(self._at(i)), MAngle.uiUnit())

    def asDistance(self, i):
        return MDistance(float(self._at(i)), MDistance.uiUnit())

    def asTime(self, i):
        return MTime(float(self._at(i)))

    def asVector(self, i, numElements=3):
        return MVector(*[float(self._at(i + k)) for k in range(numElements)])

    def asPoint(self, i, numElements=3):
        return MPoint(*[float(self._at(i + k)) for k in range(numElements)])

    def asMatrix(self, i):
        return MMatrix([float(self._at(i + k)) for k in range(16)])

    def asStringArray(self, i):
        v = self._at(i)
        return list(v) if isinstance(v, (list, tuple)) else [str(v)]

    def asIntArray(self, i):
        v = self._at(i)
        return MIntArray(v if isinstance(v, (list, tuple)) else [v])

    def asDoubleArray(self, i):
        v = self._at(i)
        return MDoubleArray(v if isinstance(v, (list, tuple)) else [v])


class MSyntax:
    kInvalidArgType = 0
    kNoArg = 1
    kBoolean = 2
    kLong = 3
    kDouble = 4
    kString = 5
    kUnsigned = 6
    kDistance = 7
    kAngle = 8
    kTime = 9
    kSelectionItem = 10
    kLastArgType = 11
    kInvalidObjectFormat = 0
    kNone = 1
    kStringObjects = 2
    kSelectionList = 3
    kLastObjectFormat = 4

    def __init__(self):
        self._flags = []
        self._args = []
        self._object_format = MSyntax.kNone
        self._min = 0
        self._max = 0
        self._selection_default = False
        self.enableQuery = False
        self.enableEdit = False

    def addFlag(self, shortName, longName, *argTypes):
        types = []
        for t in argTypes:
            types.extend(t if isinstance(t, (list, tuple)) else [t])
        self._flags.append({"short": shortName.lstrip("-"), "long": longName.lstrip("-"), "types": types, "multi": False})
        return self

    def _flag(self, name):
        n = name.lstrip("-")
        for f in self._flags:
            if n in (f["short"], f["long"]):
                return f
        return None

    def addArg(self, t):
        self._args.append(t)
        return self

    def makeFlagMultiUse(self, name):
        f = self._flag(name)
        if f:
            f["multi"] = True
        return self

    def makeFlagQueryWithFullArgs(self, name, optional):
        return self

    def setObjectType(self, format, minimum=0, maximum=4294967295):
        self._object_format = format
        self._min, self._max = minimum, maximum
        return self

    def setMinObjects(self, n):
        self._min = n
        return self

    def setMaxObjects(self, n):
        self._max = n
        return self

    def minObjects(self):
        return self._min

    def maxObjects(self):
        return self._max

    def useSelectionAsDefault(self, on):
        self._selection_default = on
        return self

    def host_flags(self):
        out = []
        kinds = {MSyntax.kBoolean: "bool", MSyntax.kLong: "int", MSyntax.kUnsigned: "int", MSyntax.kDouble: "float", MSyntax.kDistance: "float", MSyntax.kAngle: "float", MSyntax.kTime: "float", MSyntax.kString: "string", MSyntax.kSelectionItem: "string"}
        for f in self._flags:
            t = f["types"]
            if not t or t == [MSyntax.kNoArg]:
                ty = "bool"
            elif len(t) == 1 and not f["multi"]:
                ty = kinds.get(t[0], "any")
            else:
                ty = "any"
            out.append({"name": f["long"], "short": f["short"], "type": ty})
        if self.enableQuery:
            out.append({"name": "query", "short": "q", "type": "bool"})
            # The flags a query asks about arrive here, without values.
            out.append({"name": "queried", "short": "", "type": "list"})
        if self.enableEdit:
            out.append({"name": "edit", "short": "e", "type": "bool"})
        return out


class MArgParser:
    def __init__(self, syntax, args):
        self._syntax = syntax
        self._uses = {}
        self._rest = []
        self._query = False
        self._edit = False
        items = list(args._a)
        i = 0
        while i < len(items):
            v = items[i]
            f = syntax._flag(v) if isinstance(v, str) and v.startswith("-") and len(v) > 1 and not _is_number(v) else None
            if isinstance(v, str) and v in ("-q", "-query") and syntax.enableQuery:
                self._query = True
                i += 1
                continue
            if isinstance(v, str) and v in ("-e", "-edit") and syntax.enableEdit:
                self._edit = True
                i += 1
                continue
            if f is None:
                self._rest.append(v)
                i += 1
                continue
            n = len(f["types"]) if f["types"] != [MSyntax.kNoArg] else 0
            vals = items[i + 1 : i + 1 + n]
            if len(vals) < n and not self._query:
                _fail(message="Flag -%s needs %d values" % (f["long"], n))
            self._uses.setdefault(f["long"], []).append(vals)
            i += 1 + n

    def isFlagSet(self, name):
        f = self._syntax._flag(name)
        return bool(f) and f["long"] in self._uses

    def numberOfFlagUses(self, name):
        f = self._syntax._flag(name)
        return len(self._uses.get(f["long"], [])) if f else 0

    def numberOfFlagsUsed(self):
        return sum(len(v) for v in self._uses.values())

    @property
    def isQuery(self):
        return _CBool(self._query)

    @property
    def isEdit(self):
        return _CBool(self._edit)

    def _value(self, name, index):
        f = self._syntax._flag(name)
        uses = self._uses.get(f["long"]) if f else None
        if not uses:
            _fail(status="kFailure", message="Flag -%s was not given" % (name.lstrip("-"),))
        vals = uses[0]
        if not 0 <= index < len(vals):
            _fail(message="Flag -%s has no value %d" % (name.lstrip("-"), index))
        return vals[index]

    def flagArgumentBool(self, name, index):
        return MArgList([self._value(name, index)]).asBool(0)

    def flagArgumentInt(self, name, index):
        return int(self._value(name, index))

    def flagArgumentDouble(self, name, index):
        return float(self._value(name, index))

    flagArgumentFloat = flagArgumentDouble

    def flagArgumentString(self, name, index):
        return str(self._value(name, index))

    def flagArgumentAngle(self, name, index):
        return MAngle(float(self._value(name, index)), MAngle.uiUnit())

    def flagArgumentDistance(self, name, index):
        return MDistance(float(self._value(name, index)), MDistance.uiUnit())

    def flagArgumentTime(self, name, index):
        return MTime(float(self._value(name, index)))

    def getFlagArgumentList(self, name, occurrence):
        f = self._syntax._flag(name)
        uses = self._uses.get(f["long"], []) if f else []
        if not 0 <= occurrence < len(uses):
            _fail(message="Flag was not used that many times")
        return MArgList(uses[occurrence])

    def getFlagArgumentPosition(self, name, occurrence):
        return occurrence

    def commandArgumentBool(self, index):
        return MArgList(self._rest).asBool(index)

    def commandArgumentInt(self, index):
        return MArgList(self._rest).asInt(index)

    def commandArgumentDouble(self, index):
        return MArgList(self._rest).asDouble(index)

    commandArgumentFloat = commandArgumentDouble

    def commandArgumentString(self, index):
        return MArgList(self._rest).asString(index)

    def commandArgumentAngle(self, index):
        return MArgList(self._rest).asAngle(index)

    def commandArgumentDistance(self, index):
        return MArgList(self._rest).asDistance(index)

    def commandArgumentTime(self, index):
        return MArgList(self._rest).asTime(index)

    def getObjectStrings(self):
        out = [str(v) for v in self._rest]
        if not out and self._syntax._selection_default:
            out = _cmds.ls(sl=True) or []
        return out


class MArgDatabase(MArgParser):
    def getObjectList(self):
        s = MSelectionList()
        for n in self.getObjectStrings():
            s.add(n)
        return s

    def getFlagArgument(self, name, index):
        return self._value(name, index)

    def selectionItem(self, name, index):
        s = MSelectionList()
        s.add(str(self._value(name, index)))
        return s

    def flagArgumentMSelectionList(self, name, index):
        return self.selectionItem(name, index)


def _is_number(s):
    try:
        float(s)
        return True
    except ValueError:
        return False


def _plain_result(v):
    if isinstance(v, (MAngle,)):
        return v.asDegrees()
    if isinstance(v, MDistance):
        return v.asCentimeters()
    if isinstance(v, MTime):
        return v.value
    if isinstance(v, (MVector, MPoint)):
        return [v[0], v[1], v[2]]
    if isinstance(v, _MatrixBase):
        return list(v)
    if isinstance(v, (list, tuple)):
        return [_plain_result(x) for x in v]
    return v


class MPxCommand:
    kDouble = 0
    kLong = 1
    kString = 2
    kNoArg = 3
    # What the running command has set; commands nest.
    _result = None
    _has_result = False

    def __init__(self):
        self._syntax_obj = None
        self.commandString = ""

    def doIt(self, args):
        pass

    def undoIt(self):
        pass

    def redoIt(self):
        pass

    def isUndoable(self):
        return False

    def hasSyntax(self):
        return self._syntax_obj is not None

    def syntax(self):
        return self._syntax_obj if self._syntax_obj is not None else MSyntax()

    @classmethod
    def setResult(cls, value):
        MPxCommand._result = _plain_result(value)
        MPxCommand._has_result = True

    @classmethod
    def appendToResult(cls, value):
        if not isinstance(MPxCommand._result, list):
            MPxCommand._result = [] if MPxCommand._result is None else [MPxCommand._result]
        MPxCommand._result.append(_plain_result(value))
        MPxCommand._has_result = True

    @classmethod
    def clearResult(cls):
        MPxCommand._result = None
        MPxCommand._has_result = False

    @classmethod
    def currentResult(cls):
        return MPxCommand._result

    @classmethod
    def isCurrentResultArray(cls):
        return isinstance(MPxCommand._result, list)

    @classmethod
    def currentResultType(cls):
        r = MPxCommand._result
        if isinstance(r, bool) or isinstance(r, int):
            return MPxCommand.kLong
        if isinstance(r, float):
            return MPxCommand.kDouble
        if isinstance(r, str):
            return MPxCommand.kString
        return MPxCommand.kNoArg

    @staticmethod
    def displayInfo(msg):
        MGlobal.displayInfo(msg)

    @staticmethod
    def displayWarning(msg, showLineNumber=False):
        MGlobal.displayWarning(msg)

    @staticmethod
    def displayError(msg, showLineNumber=False):
        MGlobal.displayError(msg)


# ---------------------------------------------------------------------------------------------------------------------
# Nodes


class MDataHandle:
    def __init__(self, block, attr, value, element=None):
        self._block = block
        self._attr = attr
        self._v = value
        self._element = element

    def _unit(self):
        return self._block._units.get(self._attr)

    def _num(self):
        v = self._v
        if isinstance(v, (list, tuple)) or v is None:
            return 0.0
        return v

    def asBool(self):
        return bool(self._num())

    def asChar(self):
        return int(self._num())

    asShort = asChar
    asInt = asChar
    asLong = asChar
    asInt64 = asChar

    def asDouble(self):
        v = float(self._num())
        return _math.radians(v) if self._unit() == "angle" else v

    def asFloat(self):
        return _f32(self.asDouble())

    def asAngle(self):
        return MAngle(float(self._num()), MAngle.kDegrees)

    def asDistance(self):
        return MDistance(float(self._num()), MDistance.kCentimeters)

    def asTime(self):
        return MTime(float(self._num()))

    def asString(self):
        return "" if self._v is None else str(self._v)

    def _triple(self):
        return list(self._v) if isinstance(self._v, (list, tuple)) and len(self._v) == 3 else [0.0, 0.0, 0.0]

    def asVector(self):
        return MVector(self._triple())

    def asFloatVector(self):
        return MFloatVector(self._triple())

    def asDouble3(self):
        return tuple(self._triple())

    asFloat3 = asDouble3

    def asInt3(self):
        return tuple(int(x) for x in self._triple())

    asShort3 = asInt3

    def asMatrix(self):
        return MMatrix(self._v) if isinstance(self._v, (list, tuple)) and len(self._v) == 16 else MMatrix()

    def asFloatMatrix(self):
        return MFloatMatrix(self.asMatrix())

    def child(self, attr):
        name = _attr_name(attr)
        kids = self._block._children.get(self._attr)
        if isinstance(self._v, (list, tuple)) and kids and name in kids:
            return MDataHandle(self._block, name, self._v[kids.index(name)])
        return self._block.inputValue(attr)

    def _write(self, v):
        self._v = v
        self._block._write(self._attr, v, self._element)
        return self

    def setBool(self, v):
        return self._write(bool(v))

    def setChar(self, v):
        return self._write(int(v))

    setShort = setChar
    setInt = setChar
    setInt64 = setChar

    def setDouble(self, v):
        return self._write(_math.degrees(v) if self._unit() == "angle" else float(v))

    setFloat = setDouble

    def setMAngle(self, a):
        return self._write(a.asDegrees())

    def setMDistance(self, d):
        return self._write(d.asCentimeters())

    def setMTime(self, t):
        return self._write(t.value)

    def setString(self, v):
        return self._write(str(v))

    def set3Double(self, x, y, z):
        return self._write([float(x), float(y), float(z)])

    set3Float = set3Double

    def set3Int(self, x, y, z):
        return self._write([int(x), int(y), int(z)])

    set3Short = set3Int

    def setMVector(self, v):
        return self._write([v[0], v[1], v[2]])

    setMFloatVector = setMVector

    def setMMatrix(self, m):
        return self._write(list(m))

    setMFloatMatrix = setMMatrix
    setMatrix = setMMatrix

    def copy(self, other):
        return self._write(other._v)

    def setClean(self):
        return self

    def isNumeric(self):
        return isinstance(self._v, (int, float))


class MArrayDataHandle:
    def __init__(self, block, attr, elements):
        self._block = block
        self._attr = attr
        self._els = elements
        self._k = 0

    def __len__(self):
        return len(self._els)

    def elementCount(self):
        return len(self._els)

    def elementLogicalIndex(self):
        if not self._els:
            _fail(status="kFailure", message="Array has no elements")
        return self._els[self._k][0]

    def jumpToPhysicalElement(self, i):
        if not 0 <= i < len(self._els):
            _fail(message="Physical index out of range")
        self._k = i
        return self

    def jumpToLogicalElement(self, i):
        for k, e in enumerate(self._els):
            if e[0] == i:
                self._k = k
                return self
        _fail(message="No element at that index")

    def jumpToArrayElement(self, i):
        return self.jumpToPhysicalElement(i)

    def next(self):
        if self._k + 1 >= len(self._els):
            return False
        self._k += 1
        return True

    def isDone(self):
        return self._k >= len(self._els)

    def inputValue(self):
        if not self._els:
            _fail(status="kFailure", message="Array has no elements")
        e = self._els[self._k]
        return MDataHandle(self._block, self._attr, e[1], element=e[0])

    outputValue = inputValue

    def addElement(self, i):
        self._els.append([i, None])
        self._els.sort(key=lambda e: e[0])
        for k, e in enumerate(self._els):
            if e[0] == i:
                self._k = k
        return MDataHandle(self._block, self._attr, None, element=i)

    def setAllClean(self):
        return self

    setClean = setAllClean


class MDataBlock:
    def __init__(self, values, units, children, time):
        self._values = values
        self._units = units
        self._children = children
        self._out = {}
        self._time = time
        self._clean = set()

    def _name(self, a):
        if isinstance(a, MPlug):
            return a._leaf_attr()
        n = _attr_name(a)
        if n is None:
            _fail()
        return n

    def _handle(self, a):
        n = self._name(a)
        if n in self._values:
            return MDataHandle(self, n, self._values[n])
        for parent, kids in self._children.items():
            if n in kids:
                v = self._values.get(parent)
                return MDataHandle(self, parent, v).child(n) if isinstance(v, (list, tuple)) else MDataHandle(self, n, 0.0)
        _fail(message="Attribute is not on this node")

    def inputValue(self, a):
        return self._handle(a)

    outputValue = inputValue

    def inputArrayValue(self, a):
        n = self._name(a)
        v = self._values.get(n)
        if not isinstance(v, list):
            _fail(message="Attribute is not an array")
        return MArrayDataHandle(self, n, [list(e) for e in v])

    outputArrayValue = inputArrayValue

    def setClean(self, a):
        self._clean.add(self._name(a))
        return self

    def isClean(self, a):
        return self._name(a) in self._clean

    def context(self):
        return MDGContext()

    def _write(self, attr, v, element):
        if element is not None:
            self._out.setdefault(attr, {})[element] = v
        else:
            self._values[attr] = v
            self._out[attr] = v


class MDGContext:
    kNormal = None

    def __init__(self, time=None):
        self._time = time

    def isNormal(self):
        return self._time is None

    def getTime(self):
        return self._time if self._time is not None else MTime()


class MPxNode:
    kDependNode = 0
    kLocatorNode = 1
    kDeformerNode = 2
    kManipContainer = 3
    kSurfaceShape = 4
    kFieldNode = 5
    kEmitterNode = 6
    kSkinCluster = 7
    kGeometryFilter = 8
    kBlendShape = 9
    kHardwareShader = 10
    kHwShaderNode = 11
    kTransformNode = 12
    kManipulatorNode = 13
    kClientDeviceNode = 14
    kThreadedDeviceNode = 15
    kAssembly = 16
    kConstraintNode = 17
    kCameraSetNode = 18
    kImagePlaneNode = 19
    kLast = 20

    _registering = None

    def __init__(self):
        self._node_id = 0

    def compute(self, plug, dataBlock):
        return None

    def postConstructor(self):
        pass

    def thisMObject(self):
        return _node_object(self._node_id)

    def typeName(self):
        return self._type_name

    def typeId(self):
        return _type_id_of(self._type_name)

    def name(self):
        return _api("node.info", id=self._node_id)["name"]

    def isAbstractClass(self):
        return False

    def existWithoutInConnections(self, *a):
        return True

    def existWithoutOutConnections(self, *a):
        return True

    @classmethod
    def addAttribute(cls, attr):
        reg = MPxNode._registering
        if reg is None:
            _fail(status="kFailure", message="addAttribute belongs in the node's initialize function")
        if not isinstance(attr._attr, dict):
            _fail()
        reg["attrs"].append(attr._attr)

    @classmethod
    def attributeAffects(cls, whenChanges, isAffected):
        reg = MPxNode._registering
        if reg is None:
            _fail(status="kFailure", message="attributeAffects belongs in the node's initialize function")
        reg["affects"].append((_attr_name(whenChanges), _attr_name(isAffected)))

    @classmethod
    def inheritAttributesFrom(cls, name):
        _fail(status="kNotImplemented", message="Plug-in node types can't inherit attributes")


def _flatten_attrs(attrs):
    """The attrs the host holds: three numeric children make a vec3; other compounds hold their children flat."""
    out = []
    for s in attrs:
        if s["type"] == "compound":
            kids = s["children"]
            if len(kids) == 3 and all(k["type"] == "float" for k in kids) and not s.get("array"):
                d = dict(s)
                d["type"] = "vec3"
                d["default"] = [k.get("default") or 0.0 for k in kids]
                out.append(d)
            elif s.get("array"):
                _fail(status="kNotImplemented", message="Array compounds aren't available on plug-in nodes: %s" % (s["name"],))
            else:
                out.extend(_flatten_attrs(kids))
        elif not s.get("parent") or s not in out:
            out.append(s)
    # Children of a compound were also added on their own, as the reference's initializers do.
    names = set()
    unique = []
    inside = set()
    for s in attrs:
        if s["type"] == "compound" and len(s["children"]) == 3 and not s.get("array"):
            inside.update(k["name"] for k in s["children"])
    for s in out:
        if s["name"] in names or (s["name"] in inside and s["type"] != "vec3"):
            continue
        names.add(s["name"])
        unique.append(s)
    return unique


# type name -> {"cls", "creator", "id", "owner", "affects", "units", "children", "outputs"}
_node_classes = {}
_node_instances = {}
_command_classes = {}
# plugin name -> {"path", "vendor", "version", "api", "module", "commands", "nodes", "object"}
_plugins = {}
_loading = []


def _plugin_instance(node_id):
    inst = _node_instances.get(node_id)
    if inst is not None:
        return inst
    info = _api("node.info", id=node_id)
    if not info or info["type"] not in _node_classes:
        _fail()
    return _instance(info["type"], node_id)


def _instance(type_name, node_id):
    inst = _node_instances.get(node_id)
    if inst is None:
        e = _node_classes[type_name]
        inst = e["creator"]()
        inst._node_id = node_id
        inst._type_name = type_name
        _node_instances[node_id] = inst
        inst.postConstructor()
    return inst


class MFnPlugin(MFnBase):
    def __init__(self, obj=None, vendor="Unknown", version="Unknown", apiVersion="Any"):
        self._obj = obj if isinstance(obj, MObject) else MObject()
        if self._obj._kind != "plugin":
            _fail()
        p = self._obj._data
        self._p = p
        if vendor != "Unknown":
            p["vendor"] = vendor
        if version != "Unknown":
            p["version"] = version
        if apiVersion != "Any":
            p["api"] = apiVersion

    def vendor(self):
        return self._p["vendor"]

    def version(self):
        return self._p["version"]

    def apiVersion(self):
        return self._p["api"]

    def name(self):
        return self._p["name"]

    def setName(self, name, allowRename=True):
        return self

    def loadPath(self):
        return _os.path.dirname(self._p["path"])

    def registerUI(self, *a):
        return self

    def registerCommand(self, name, creator, syntaxCreator=None):
        if _az.exists(name):
            _fail(status="kFailure", message="A command named %s already exists" % (name,))
        syntax = syntaxCreator() if syntaxCreator is not None else None
        _az.register_command(name, self._p["name"], _json.dumps(syntax.host_flags() if syntax else []))
        _command_classes[name] = {"creator": creator, "syntax": syntax, "owner": self._p["name"]}
        if syntax is None:
            _main._plain_arg_commands.add(name)
        self._p["commands"].append(name)
        _forget_cached_command(name)
        return self

    def deregisterCommand(self, name):
        if name not in _command_classes:
            _fail(status="kFailure", message="No plug-in command named %s" % (name,))
        _az.deregister_command(name)
        del _command_classes[name]
        _main._plain_arg_commands.discard(name)
        if name in self._p["commands"]:
            self._p["commands"].remove(name)
        _forget_cached_command(name)
        return self

    def registerNode(self, name, typeId, creator, initialize=None, nodeType=MPxNode.kDependNode, classification=None):
        if nodeType != MPxNode.kDependNode:
            _fail(status="kNotImplemented", message="Only plain dependency nodes can come from plug-ins")
        reg = {"attrs": [], "affects": []}
        was = MPxNode._registering
        MPxNode._registering = reg
        try:
            if initialize is not None:
                initialize()
        finally:
            MPxNode._registering = was
        attrs = _flatten_attrs(reg["attrs"])
        # What gets computed: what something affects, and whatever can't be written from outside.
        affected = {b for _, b in reg["affects"]}
        outputs = []
        children = {}
        units = {}
        for s in attrs:
            if s["name"] in affected and "input" not in s:
                s["input"] = False
            if not s.get("input", True):
                outputs.append(s["name"])
            if s.get("unit"):
                units[s["name"]] = s["unit"]
                if s["unit"] == "angle":
                    _PLUGIN_ANGLES.add((name, s["name"]))
            if s["type"] == "vec3":
                kids = s.get("children") or [{"name": s["name"] + a, "short": s["short"] + a.lower()} for a in "XYZ"]
                children[s["name"]] = [k["name"] for k in kids]
        _az.register_node(name, self._p["name"], _json.dumps(attrs))
        _node_classes[name] = {"creator": creator, "id": MTypeId(typeId), "owner": self._p["name"], "outputs": outputs, "units": units, "children": children, "attrs": attrs}
        self._p["nodes"].append(name)
        return self

    def deregisterNode(self, typeId):
        tid = MTypeId(typeId)
        for name, e in list(_node_classes.items()):
            if e["id"] == tid:
                _az.deregister_node(name)
                del _node_classes[name]
                if name in self._p["nodes"]:
                    self._p["nodes"].remove(name)
                return self
        _fail(status="kFailure", message="No plug-in node has that id")


def _forget_cached_command(name):
    if name in _cmds.__dict__:
        del _cmds.__dict__[name]


def _plugin_search_path():
    out = []
    for var in ("AZTEK_PLUG_IN_PATH", "MAYA_PLUG_IN_PATH"):
        out.extend(p for p in _os.environ.get(var, "").split(_os.pathsep) if p)
    return out


def _find_plugin_file(name):
    if _os.path.isfile(name):
        return _os.path.abspath(name)
    file = name if name.endswith(".py") else name + ".py"
    for d in _plugin_search_path():
        p = _os.path.join(d, file)
        if _os.path.isfile(p):
            return p
    return None


def _plugin_key(name):
    base = _os.path.basename(name)
    return base[:-3] if base.endswith(".py") else base


def _error_answer(e):
    return _json.dumps({"error": str(e) or type(e).__name__})


def _show_traceback(e):
    lines = _traceback.format_exception(type(e), e, e.__traceback__)
    for l in "".join(lines[1:-1]).rstrip().splitlines():
        _az.write("# " + l, 2)


class _Hooks:
    """What the host calls."""

    def load_plugin(self, input):
        a = _json.loads(input)
        try:
            names = []
            for n in a["args"]:
                names.append(_load_one(n, a["flags"].get("quiet")))
            return _json.dumps({"ok": [n for n in names if n]})
        except Exception as e:
            _show_traceback(e)
            return _error_answer(e)

    def unload_plugin(self, input):
        a = _json.loads(input)
        try:
            for n in a["args"]:
                _unload_one(n)
            return _json.dumps({"ok": None})
        except Exception as e:
            _show_traceback(e)
            return _error_answer(e)

    def plugin_info(self, input):
        a = _json.loads(input)
        try:
            return _json.dumps({"ok": _plugin_info(a["args"], a["flags"])})
        except Exception as e:
            return _error_answer(e)

    def command_new(self, name):
        e = _command_classes.get(name)
        if e is None:
            raise RuntimeError("(kFailure): The plug-in command %s is gone" % (name,))
        inst = e["creator"]()
        inst._syntax_obj = e["syntax"]
        return (inst, bool(inst.isUndoable()))

    def command_do(self, inst, input):
        a = _json.loads(input)
        items = []
        syntax = inst._syntax_obj
        if syntax is not None:
            for k, v in a["flags"].items():
                if k == "queried":
                    continue
                if k == "query":
                    items.append("-q")
                    continue
                if k == "edit":
                    items.append("-e")
                    continue
                f = syntax._flag(k)
                if f is None:
                    continue
                items.append("-" + f["long"])
                if f["types"] and f["types"] != [MSyntax.kNoArg]:
                    items.extend(v if isinstance(v, list) else [v])
                elif v is False:
                    items.pop()
        for v in a["args"]:
            items.extend(v if isinstance(v, list) else [v])
        # Last, so nothing after them reads as their values.
        for k in a["flags"].get("queried") or []:
            items.append("-" + k)
        saved = (MPxCommand._result, MPxCommand._has_result)
        MPxCommand._result, MPxCommand._has_result = None, False
        try:
            try:
                inst.doIt(MArgList(items))
                _flush_points()
            except Exception as e:
                _show_traceback(e)
                return _error_answer(e)
            return _json.dumps({"ok": MPxCommand._result})
        finally:
            MPxCommand._result, MPxCommand._has_result = saved
            _sys.stdout.flush()

    def command_replay(self, inst, method):
        try:
            getattr(inst, method)()
            _flush_points()
        except Exception as e:
            _show_traceback(e)
            _az.write("# Error: %s #" % (e,), 2)
        finally:
            _sys.stdout.flush()
        return ""

    def compute(self, type_name, node_id, name, time, values):
        e = _node_classes.get(type_name)
        if e is None or not e["outputs"]:
            return "{}"
        _compute_local.ctx = {"id": node_id, "name": name, "type": type_name, "owner": e["owner"], "attrs": e["attrs"]}
        try:
            inst = _instance(type_name, node_id)
            block = MDataBlock(_json.loads(values), e["units"], e["children"], time)
            for out in e["outputs"]:
                inst.compute(MPlug._of(node_id, out), block)
            return _json.dumps(_host_outputs(block._out))
        except Exception as ex:
            _show_traceback(ex)
            _az.write("# Error: %s: %s #" % (type(ex).__name__, ex), 2)
            return "{}"
        finally:
            _compute_local.ctx = None
            _sys.stdout.flush()


def _host_outputs(out):
    res = {}
    for k, v in out.items():
        res[k] = sorted([[i, x] for i, x in v.items()]) if isinstance(v, dict) else v
    return res


def _load_one(name, quiet):
    key = _plugin_key(name)
    if key in _plugins:
        return key
    path = _find_plugin_file(name)
    if path is None:
        if quiet:
            return None
        raise RuntimeError('Plug-in "%s" was not found on the plug-in path.' % (name,))
    import importlib.util

    spec = importlib.util.spec_from_file_location("_aztek_plugin_" + key, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    if not hasattr(module, "maya_useNewAPI") and not hasattr(module, "aztek_useNewAPI"):
        raise RuntimeError('Plug-in "%s" does not define maya_useNewAPI(): only API 2.0 plug-ins can load.' % (key,))
    if not hasattr(module, "initializePlugin"):
        raise RuntimeError('Plug-in "%s" has no initializePlugin.' % (key,))
    p = {"name": key, "path": path, "vendor": "Unknown", "version": "Unknown", "api": "Any", "module": module, "commands": [], "nodes": []}
    obj = MObject()
    obj._kind, obj._fn, obj._data = "plugin", MFn.kInvalid, p
    p["object"] = obj
    try:
        module.initializePlugin(obj)
    except Exception:
        _take_back(p)
        raise
    _plugins[key] = p
    return key


def _take_back(p):
    """What a plug-in still has registered goes when it does."""
    for name in list(p["commands"]):
        if name in _command_classes:
            _az.deregister_command(name)
            del _command_classes[name]
            _main._plain_arg_commands.discard(name)
            _forget_cached_command(name)
    for name in list(p["nodes"]):
        if name in _node_classes:
            _az.deregister_node(name)
            del _node_classes[name]
    p["commands"], p["nodes"] = [], []


def _unload_one(name):
    key = _plugin_key(name)
    p = _plugins.get(key)
    if p is None:
        raise RuntimeError('Plug-in "%s" is not loaded.' % (name,))
    if hasattr(p["module"], "uninitializePlugin"):
        p["module"].uninitializePlugin(p["object"])
    _take_back(p)
    del _plugins[key]


def _plugin_info(args, flags):
    if flags.get("listPlugins"):
        return sorted(_plugins)
    name = args[0] if args else None
    if name is None:
        _fail(message="pluginInfo: name a plug-in")
    p = _plugins.get(_plugin_key(name))
    if flags.get("loaded"):
        return p is not None
    if flags.get("registered"):
        return p is not None or _find_plugin_file(name) is not None
    if p is None:
        raise RuntimeError('Plug-in "%s" is not loaded.' % (name,))
    for flag, key in (("version", "version"), ("vendor", "vendor"), ("path", "path"), ("name", "name"), ("apiVersion", "api")):
        if flags.get(flag):
            return p[key]
    if flags.get("command"):
        return list(p["commands"])
    if flags.get("dependNode"):
        return list(p["nodes"])
    _fail(message="pluginInfo: nothing asked")


_az.set_hooks(_Hooks())
