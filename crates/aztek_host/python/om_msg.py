# aztek.api.OpenAztek, messages: MSceneMessage, MDGMessage, MDagMessage, MNodeMessage, MEventMessage and MMessage, with
# MFileObject, MUuid and MCallbackIdArray. Each callback is a watch the host keeps (message.pywatch); the host calls the
# `message` hook with what happened, and the function is called as the reference's Python API calls it.
import itertools as _itertools
import os as _os


# ---------------------------------------------------------------------------------------------------------------------
# MFileObject


def _slashed(p):
    return str(p).replace("\\", "/")


def _dir_of(p):
    i = p.rfind("/")
    return "" if i < 0 else p[: i + 1]


def _base_of(p):
    return p[p.rfind("/") + 1 :]


class MFileObject:
    """A file name as the reference holds one: as written (raw), with its variables expanded, and as found (resolved)."""

    kNone = 0
    kRelative = 1
    kInputFile = 2
    kDirMap = 4
    kExact = 5
    kStrict = 8

    def __init__(self, src=None):
        self._full = ""
        self._resolved = ""
        if isinstance(src, MFileObject):
            self._full, self._resolved = src._full, src._resolved

    def setRawName(self, name):
        self._full = _dir_of(self._full) + _slashed(name)
        self._resolved = ""
        return self

    def setRawPath(self, path):
        p = _slashed(path)
        if p and not p.endswith("/"):
            p += "/"
        self._full = p + _base_of(self._full)
        self._resolved = ""
        return self

    def setRawFullName(self, fullName):
        self._full = _slashed(fullName)
        self._resolved = ""
        return self

    def setRawURI(self, uri):
        u = str(uri)
        return self.setRawFullName(u[7:] if u.startswith("file://") else u)

    def overrideResolvedFullName(self, fullName, reresolveType=False):
        self._resolved = _slashed(fullName)
        return self

    def rawName(self):
        return _base_of(self._full)

    def rawPath(self):
        return _dir_of(self._full)

    def rawFullName(self):
        return self._full

    def rawURI(self):
        return "file://" + self.expandedFullName()

    def expandedFullName(self):
        return _os.path.expandvars(self._full)

    def expandedPath(self):
        return _dir_of(self.expandedFullName())

    def resolvedFullName(self):
        if self._resolved:
            return self._resolved
        e = self.expandedFullName()
        return e if e and _os.path.isfile(e) else ""

    def resolvedName(self):
        return _base_of(self.resolvedFullName())

    def resolvedPath(self):
        return _dir_of(self.resolvedFullName())

    def exists(self):
        return bool(self.resolvedFullName())

    def isSet(self):
        return bool(self._full)

    def copy(self, src):
        self._full, self._resolved = src._full, src._resolved
        return self

    def __repr__(self):
        return "aztek.api.OpenAztek.MFileObject(%r)" % (self._full,)


# ---------------------------------------------------------------------------------------------------------------------
# MUuid


class MUuid:
    def __init__(self, src=None):
        self._b = None
        if isinstance(src, MUuid):
            self._b = src._b
        elif isinstance(src, str):
            self._from_string(src)
        elif src is not None:
            b = bytes(bytearray(src))
            if len(b) != 16:
                _fail()
            self._b = b

    def _from_string(self, s):
        h = s.replace("-", "")
        try:
            b = bytes.fromhex(h)
        except ValueError:
            b = b""
        if len(b) != 16:
            _fail(message="Not a UUID: %s" % (s,))
        self._b = b

    def asString(self):
        if self._b is None:
            return ""
        h = self._b.hex().upper()
        return "%s-%s-%s-%s-%s" % (h[:8], h[8:12], h[12:16], h[16:20], h[20:])

    def generate(self):
        import uuid

        self._b = uuid.uuid4().bytes
        return self

    def valid(self):
        return self._b is not None

    def copy(self, src):
        self._b = src._b
        return self

    def __eq__(self, o):
        return isinstance(o, MUuid) and self._b == o._b

    def __ne__(self, o):
        return not self == o

    __hash__ = None

    def __str__(self):
        return self.asString()

    def __repr__(self):
        return "aztek.api.OpenAztek.MUuid(%r)" % (self.asString(),)


class MCallbackIdArray(MIntArray):
    pass


# ---------------------------------------------------------------------------------------------------------------------
# Callbacks

# id -> {"kind", "fn", "client", "node", "job", "v1"}
_msg_regs = {}
_msg_current = [0]
_event_ids = _itertools.count(1 << 41)
# Set by the 1.0 module: its classes for what callbacks are given, and its pointers for check callbacks' answers.
_MSG_TO1 = [None]
_MSG_PTR = [None]
_MSG_V1 = [False]


def _msg_add(kind, fn, client, watch, node=-1):
    if not callable(fn):
        _fail(message="The callback is not callable")
    w = dict(watch)
    w.setdefault("msg", -1)
    w.setdefault("node", -1)
    id = _api("message.pywatch", **w)
    _msg_regs[id] = {"kind": kind, "fn": fn, "client": client, "node": node, "job": None, "v1": _MSG_V1[0]}
    return id


def _msg_node(obj):
    if obj is None:
        return -1
    if isinstance(obj, MDagPath):
        obj = obj.node()
    return _check_node(obj)


def _msg_object(id):
    return _node_object(id) if isinstance(id, int) and id > 0 else MObject()


def _msg_path(id):
    return MDagPath._of(id) if isinstance(id, int) and id > 0 else MDagPath()


def _msg_plug(name):
    if not name:
        return MPlug()
    try:
        return MSelectionList().add(name).getPlug(0)
    except Exception:
        return MPlug()


def _msg_time(seconds):
    t = MTime(MTime(float(seconds), MTime.kSeconds).asUnits(MTime.uiUnit()), MTime.uiUnit())
    return t


def _msg_file(p):
    f = MFileObject()
    f.setRawFullName(p.get("file") or "")
    return f


def _msg_call(r, p):
    """Calls one callback with what the host sent; a check callback's answer as the host takes it."""
    kind, fn, cd = r["kind"], r["fn"], r["client"]
    v1 = r["v1"]
    to1 = _MSG_TO1[0] if v1 and _MSG_TO1[0] else (lambda x: x)
    if kind in ("basic", "event"):
        fn(cd)
    elif kind in ("check", "checkFile", "checkReference"):
        extra = []
        f = None
        if kind == "checkReference":
            extra.append(to1(_msg_object(p.get("node"))))
        if kind != "check":
            f = _msg_file(p)
            extra.append(to1(f))
        if v1:
            # 1.0: the answer is set through a pointer given first.
            ptr = _MSG_PTR[0]()
            fn(ptr, *(extra + [cd]))
            ok = bool(ptr.v[0]) if ptr.v else True
        else:
            got = fn(*(extra + [cd]))
            ok = True if got is None else bool(got)
        out = {"retCode": ok}
        if f is not None:
            out["file"] = f.rawFullName()
        return out
    elif kind == "strings":
        fn(to1(MStringArray(p.get("strings") or [])) if v1 else list(p.get("strings") or []), cd)
    elif kind == "reference":
        fn(to1(_msg_object(p.get("node"))), to1(_msg_file(p)), cd)
    elif kind == "twoStrings":
        s = p.get("strings") or ["", ""]
        fn(s[0], s[1], cd)
    elif kind == "time":
        fn(to1(_msg_time(p.get("time", 0.0))), cd)
    elif kind == "node":
        fn(to1(_msg_object(p.get("node"))), cd)
    elif kind == "plugs":
        fn(to1(_msg_plug(p.get("src"))), to1(_msg_plug(p.get("dst"))), bool(p.get("made")), cd)
    elif kind == "parentChild":
        fn(to1(_msg_path(p.get("child"))), to1(_msg_path(p.get("parent"))), cd)
    elif kind == "msgParentChild":
        fn(p.get("msg", -1), to1(_msg_path(p.get("child"))), to1(_msg_path(p.get("parent"))), cd)
    elif kind == "matrix":
        fn(to1(_msg_object(p.get("node"))), p.get("flags", 0), cd)
    elif kind == "attr2":
        fn(p.get("msg", 0), to1(_msg_plug(p.get("plug"))), to1(_msg_plug(p.get("other"))), cd)
    elif kind == "attr1":
        fn(p.get("msg", 0), to1(_msg_plug(p.get("plug"))), cd)
    elif kind == "nodePlug":
        fn(to1(_msg_object(p.get("node"))), to1(_msg_plug(p.get("plug"))), cd)
    elif kind == "nodeString":
        fn(to1(_msg_object(p.get("node"))), p.get("old", ""), cd)
    elif kind == "nodeUuid":
        fn(to1(_msg_object(p.get("node"))), MUuid(p.get("uuid") or None) if p.get("uuid") else MUuid(), cd)
    elif kind == "nodeModifier":
        mod = MDGModifier()
        fn(to1(_msg_object(p.get("node"))), to1(mod), cd)
        mod.doIt()
    elif kind == "modifier":
        mod = MDGModifier()
        fn(to1(mod), cd)
        mod.doIt()
    return None


def _hook_message(self, id, payload):
    r = _msg_regs.get(id)
    if r is None or r["fn"] is None:
        return ""
    was = _msg_current[0]
    _msg_current[0] = id
    try:
        out = _msg_call(r, _json.loads(payload or "{}"))
        return _json.dumps(out) if out is not None else ""
    except Exception as e:
        _show_traceback(e)
        _az.write("# Error: %s: %s #" % (type(e).__name__, e), 2)
        return ""
    finally:
        _msg_current[0] = was
        _sys.stdout.flush()


_Hooks.message = _hook_message


class MMessage:
    kDefaultAction = 0
    kDoNotDoAction = 1
    kDoAction = 2

    @staticmethod
    def removeCallback(id):
        r = _msg_regs.pop(int(id), None)
        if r is None:
            _fail(message="No such callback")
        if r["job"] is not None:
            try:
                _cmds.scriptJob(kill=r["job"], force=True)
            except Exception:
                pass
        else:
            _api("message.unwatch", callback=int(id))
        # A callback may remove itself while it runs.
        r["fn"] = None

    @staticmethod
    def removeCallbacks(ids):
        for id in list(ids):
            MMessage.removeCallback(id)

    @staticmethod
    def currentCallbackId():
        return _msg_current[0]

    @staticmethod
    def nodeCallbacks(node):
        n = _msg_node(node)
        mine = [i for i in _api("message.nodeCallbacks", id=n) if i in _msg_regs]
        return MCallbackIdArray(mine)


def _scene_add(kind):
    def add(msg, function, clientData=None):
        return _msg_add(kind, function, clientData, {"kind": "scene", "msg": int(msg)})

    return staticmethod(add)


class MSceneMessage(MMessage):
    kSceneUpdate = 0
    kBeforeNew = 1
    kAfterNew = 2
    kBeforeImport = 3
    kAfterImport = 4
    kBeforeOpen = 5
    kAfterOpen = 6
    kBeforeFileRead = 7
    kAfterFileRead = 8
    kAfterSceneReadAndRecordEdits = 9
    kBeforeExport = 10
    kAfterExport = 11
    kBeforeSave = 12
    kAfterSave = 13
    kBeforeReference = 14
    kAfterReference = 15
    kBeforeRemoveReference = 16
    kAfterRemoveReference = 17
    kBeforeImportReference = 18
    kAfterImportReference = 19
    kBeforeExportReference = 20
    kAfterExportReference = 21
    kBeforeUnloadReference = 22
    kAfterUnloadReference = 23
    kBeforeSoftwareRender = 24
    kAfterSoftwareRender = 25
    kBeforeSoftwareFrameRender = 26
    kAfterSoftwareFrameRender = 27
    kSoftwareRenderInterrupted = 28
    kMayaInitialized = 29
    kMayaExiting = 30
    kBeforeNewCheck = 31
    kBeforeOpenCheck = 32
    kBeforeSaveCheck = 33
    kBeforeImportCheck = 34
    kBeforeExportCheck = 35
    kBeforeLoadReference = 36
    kAfterLoadReference = 37
    kBeforeLoadReferenceCheck = 38
    kBeforeReferenceCheck = 39
    kBeforeCreateReferenceCheck = 39
    kBeforePluginLoad = 40
    kAfterPluginLoad = 41
    kBeforePluginUnload = 42
    kAfterPluginUnload = 43
    kBeforeCreateReference = 44
    kAfterCreateReference = 45
    kExportStarted = 46
    kBeforeLoadReferenceAndRecordEdits = 47
    kAfterLoadReferenceAndRecordEdits = 48
    kBeforeCreateReferenceAndRecordEdits = 49
    kAfterCreateReferenceAndRecordEdits = 50
    kLast = 51

    addCallback = _scene_add("basic")
    addCheckCallback = _scene_add("check")
    addCheckFileCallback = _scene_add("checkFile")
    addStringArrayCallback = _scene_add("strings")
    addReferenceCallback = _scene_add("reference")
    addCheckReferenceCallback = _scene_add("checkReference")

    @staticmethod
    def addConnectionFailedCallback(function, clientData=None):
        return _msg_add("connFail", function, clientData, {"kind": "connectionFailed"})

    @staticmethod
    def addNamespaceRenamedCallback(function, clientData=None):
        return _msg_add("twoStrings", function, clientData, {"kind": "namespaceRenamed"})


class MDGMessage(MMessage):
    @staticmethod
    def addTimeChangeCallback(function, clientData=None):
        return _msg_add("time", function, clientData, {"kind": "time", "msg": 0})

    @staticmethod
    def addDelayedTimeChangeCallback(function, clientData=None):
        return _msg_add("time", function, clientData, {"kind": "time", "msg": 1})

    @staticmethod
    def addDelayedTimeChangeRunupCallback(function, clientData=None):
        return _msg_add("time", function, clientData, {"kind": "time", "msg": 2})

    @staticmethod
    def addForceUpdateCallback(function, clientData=None):
        return _msg_add("time", function, clientData, {"kind": "time", "msg": 3})

    @staticmethod
    def addNodeAddedCallback(function, nodeType="dependNode", clientData=None):
        return _msg_add("node", function, clientData, {"kind": "nodeAdded", "type": nodeType})

    @staticmethod
    def addNodeRemovedCallback(function, nodeType="dependNode", clientData=None):
        return _msg_add("node", function, clientData, {"kind": "nodeRemoved", "type": nodeType})

    @staticmethod
    def addConnectionCallback(function, clientData=None):
        return _msg_add("plugs", function, clientData, {"kind": "connection"})

    @staticmethod
    def addPreConnectionCallback(function, clientData=None):
        return _msg_add("plugs", function, clientData, {"kind": "preConnection"})

    @staticmethod
    def addNodeChangeUuidCheckCallback(function, clientData=None):
        return _msg_add("uuidCheck", function, clientData, {"kind": "uuidCheck"})


class MDagMessage(MMessage):
    kInvalidMsg = -1
    kParentAdded = 0
    kParentRemoved = 1
    kChildAdded = 2
    kChildRemoved = 3
    kChildReordered = 4
    kInstanceAdded = 5
    kInstanceRemoved = 6
    kLast = 7

    kScaleX = 1 << 0
    kScaleY = 1 << 1
    kScaleZ = 1 << 2
    kShearXY = 1 << 3
    kShearXZ = 1 << 4
    kShearYZ = 1 << 5
    kRotateX = 1 << 6
    kRotateY = 1 << 7
    kRotateZ = 1 << 8
    kTranslateX = 1 << 9
    kTranslateY = 1 << 10
    kTranslateZ = 1 << 11
    kScalePivotX = 1 << 12
    kScalePivotY = 1 << 13
    kScalePivotZ = 1 << 14
    kRotatePivotX = 1 << 15
    kRotatePivotY = 1 << 16
    kRotatePivotZ = 1 << 17
    kScaleTransX = 1 << 18
    kScaleTransY = 1 << 19
    kScaleTransZ = 1 << 20
    kRotateTransX = 1 << 21
    kRotateTransY = 1 << 22
    kRotateTransZ = 1 << 23
    kRotateOrientX = 1 << 24
    kRotateOrientY = 1 << 25
    kRotateOrientZ = 1 << 26
    kRotateOrder = 1 << 27
    kAll = (1 << 28) - 1
    kScale = kScaleX | kScaleY | kScaleZ
    kShear = kShearXY | kShearXZ | kShearYZ
    kRotation = kRotateX | kRotateY | kRotateZ
    kTranslation = kTranslateX | kTranslateY | kTranslateZ
    kScalePivot = kScalePivotX | kScalePivotY | kScalePivotZ
    kRotatePivot = kRotatePivotX | kRotatePivotY | kRotatePivotZ
    kScalePivotTrans = kScaleTransX | kScaleTransY | kScaleTransZ
    kRotatePivotTrans = kRotateTransX | kRotateTransY | kRotateTransZ
    kRotateOrient = kRotateOrientX | kRotateOrientY | kRotateOrientZ

    @staticmethod
    def _add(msg, node, kind, function, clientData):
        n = _msg_node(node) if node is not None else -1
        return _msg_add(kind, function, clientData, {"kind": "dag", "msg": msg, "node": n}, n)

    @staticmethod
    def addParentAddedCallback(function, clientData=None):
        return MDagMessage._add(0, None, "parentChild", function, clientData)

    @staticmethod
    def addParentAddedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(0, node, "parentChild", function, clientData)

    @staticmethod
    def addParentRemovedCallback(function, clientData=None):
        return MDagMessage._add(1, None, "parentChild", function, clientData)

    @staticmethod
    def addParentRemovedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(1, node, "parentChild", function, clientData)

    @staticmethod
    def addChildAddedCallback(function, clientData=None):
        return MDagMessage._add(2, None, "parentChild", function, clientData)

    @staticmethod
    def addChildAddedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(2, node, "parentChild", function, clientData)

    @staticmethod
    def addChildRemovedCallback(function, clientData=None):
        return MDagMessage._add(3, None, "parentChild", function, clientData)

    @staticmethod
    def addChildRemovedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(3, node, "parentChild", function, clientData)

    @staticmethod
    def addChildReorderedCallback(function, clientData=None):
        return MDagMessage._add(4, None, "parentChild", function, clientData)

    @staticmethod
    def addChildReorderedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(4, node, "parentChild", function, clientData)

    @staticmethod
    def addInstanceAddedCallback(function, clientData=None):
        return MDagMessage._add(5, None, "parentChild", function, clientData)

    @staticmethod
    def addInstanceAddedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(5, node, "parentChild", function, clientData)

    @staticmethod
    def addInstanceRemovedCallback(function, clientData=None):
        return MDagMessage._add(6, None, "parentChild", function, clientData)

    @staticmethod
    def addInstanceRemovedDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(6, node, "parentChild", function, clientData)

    @staticmethod
    def addDagCallback(msgType, function, clientData=None):
        return MDagMessage._add(int(msgType), None, "msgParentChild", function, clientData)

    @staticmethod
    def addDagDagPathCallback(node, msgType, function, clientData=None):
        return MDagMessage._add(int(msgType), node, "msgParentChild", function, clientData)

    @staticmethod
    def addAllDagChangesCallback(function, clientData=None):
        return MDagMessage._add(-1, None, "msgParentChild", function, clientData)

    @staticmethod
    def addAllDagChangesDagPathCallback(node, function, clientData=None):
        return MDagMessage._add(-1, node, "msgParentChild", function, clientData)

    @staticmethod
    def addMatrixModifiedCallback(node, function, clientData=None):
        n = _msg_node(node)
        return _msg_add("matrix", function, clientData, {"kind": "matrix", "node": n}, n)

    @staticmethod
    def addWorldMatrixModifiedCallback(node, function, clientData=None):
        n = _msg_node(node)
        return _msg_add("matrix", function, clientData, {"kind": "worldMatrix", "node": n}, n)


def _node_add(what, kind):
    def add(node, function, clientData=None):
        n = _msg_node(node)
        return _msg_add(kind, function, clientData, {"kind": what, "node": n}, n)

    return staticmethod(add)


class MNodeMessage(MMessage):
    kConnectionMade = 0x01
    kConnectionBroken = 0x02
    kAttributeEval = 0x04
    kAttributeSet = 0x08
    kAttributeLocked = 0x10
    kAttributeUnlocked = 0x20
    kAttributeAdded = 0x40
    kAttributeRemoved = 0x80
    kAttributeRenamed = 0x100
    kAttributeKeyable = 0x200
    kAttributeUnkeyable = 0x400
    kIncomingDirection = 0x800
    kAttributeArrayAdded = 0x1000
    kAttributeArrayRemoved = 0x2000
    kOtherPlugSet = 0x4000
    kLast = 0x8000

    kKeyChangeInvalid = 0
    kMakeKeyable = 1
    kMakeUnkeyable = 2
    kKeyChangeLast = 3

    addAttributeChangedCallback = _node_add("attributeChanged", "attr2")
    addAttributeAddedOrRemovedCallback = _node_add("attributeAddedOrRemoved", "attr1")
    addNodeDirtyCallback = _node_add("dirty", "node")
    addNodeDirtyPlugCallback = _node_add("dirtyPlug", "nodePlug")
    addNameChangedCallback = _node_add("nameChanged", "nodeString")
    addUuidChangedCallback = _node_add("uuidChanged", "nodeUuid")
    addNodeAboutToDeleteCallback = _node_add("aboutToDelete", "nodeModifier")
    addNodePreRemovalCallback = _node_add("preRemoval", "node")
    addNodeDestroyedCallback = _node_add("destroyed", "basic")

    @staticmethod
    def addKeyableChangeOverride(plug, function, clientData=None):
        n = _check_node(plug.node())
        return _msg_add("keyable", function, clientData, {"kind": "keyableOverride", "node": n}, n)


class MEventMessage(MMessage):
    @staticmethod
    def addEventCallback(eventName, function, clientData=None):
        if not callable(function):
            _fail(message="The callback is not callable")
        id = next(_event_ids)
        r = {"kind": "event", "fn": function, "client": clientData, "node": -1, "job": None, "v1": _MSG_V1[0]}

        def fire():
            if r["fn"] is None:
                return
            was = _msg_current[0]
            _msg_current[0] = id
            try:
                r["fn"](clientData)
            finally:
                _msg_current[0] = was

        r["job"] = _cmds.scriptJob(event=[eventName, fire])
        _msg_regs[id] = r
        return id

    @staticmethod
    def getEventNames():
        return list(_cmds.scriptJob(listEvents=True) or [])


def _msg_v1(cls, name):
    """The 1.0 class: the same calls, its callbacks given 1.0 values and check answers through a pointer."""

    def wrap(f):
        def call(*a, **k):
            _MSG_V1[0] = True
            try:
                return f(*a, **k)
            finally:
                _MSG_V1[0] = False

        return staticmethod(call)

    ns = {}
    for k in dir(cls):
        if k.startswith("add"):
            ns[k] = wrap(getattr(cls, k))
    return type(name, (cls,), ns)
