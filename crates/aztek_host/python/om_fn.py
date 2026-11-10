# aztek.api.OpenMaya, function sets: nodes, transforms, meshes, curves, sets, attributes, data, components.
import __main__ as _main
import array as _array


class _CInt(int):
    """A number that can also be called: the reference has some of these as properties, scripts write them as methods too."""

    def __call__(self):
        return int(self)


class _CBool(int):
    def __call__(self):
        return bool(self)

    def __bool__(self):
        return int(self) != 0

    def __repr__(self):
        return repr(bool(int(self)))

    __str__ = __repr__


class _CStr(str):
    def __call__(self):
        return str(self)


class _CBox(MBoundingBox):
    def __call__(self):
        return self


# ---------------------------------------------------------------------------------------------------------------------
# Mesh data

# Point edits held back so that a loop of setPoint or setPosition makes one edit: {shape: {index: (x, y, z)}}.
_pending_points = {}
_mesh_cache = {}


def _flush_points():
    # A compute can't write: what's held waits for the thread that holds the host.
    if not _pending_points or getattr(_compute_local, "ctx", None) is not None:
        return
    work = dict(_pending_points)
    _pending_points.clear()
    for shape, moves in work.items():
        if not _api("node.exists", id=shape):
            continue
        p = _numbers("d", _az.mesh_arrays(shape)["points"])
        for i, v in moves.items():
            p[i * 3], p[i * 3 + 1], p[i * 3 + 2] = v
        _write_points(shape, p)


_main._before_call[:] = [_flush_points]


def _write_points(shape, flat):
    name = _api("node.info", id=shape)["path"]
    pts = [list(flat[i : i + 3]) for i in range(0, len(flat), 3)]
    _json_ok = _az.call("setMeshPoints", _json.dumps([name]), _json.dumps({"points": pts}))
    r = _json.loads(_json_ok)
    if isinstance(r, dict) and "__error__" in r:
        raise RuntimeError(r["__error__"])


def _numbers(code, raw):
    """An array the host sent as bytes (or, from older callers, a list)."""
    if isinstance(raw, (bytes, bytearray)):
        a = _array.array(code)
        a.frombytes(raw)
        return a
    return raw


class _MeshData:
    """A mesh's arrays at one scene version, with the adjacency worked out when first asked for."""

    def __init__(self, d):
        self.shape = d["shape"]
        self.points = _numbers("d", d["points"])
        self.counts = _numbers("q", d["counts"])
        self.connects = _numbers("q", d["connects"])
        self.corner_edges = _numbers("q", d["corner_edges"])
        self.edges = _numbers("q", d["edges"])
        self.corner_normals = _numbers("d", d["corner_normals"])
        self.point_normals = _numbers("d", d["point_normals"])
        self.has_uvs = d["has_uvs"]
        self.corner_maps = _numbers("q", d["corner_maps"])
        self.map_uvs = _numbers("d", d["map_uvs"])
        self.num_vertices = len(self.points) // 3
        self.num_edges = len(self.edges) // 2
        self.num_faces = len(self.counts)
        self.num_corners = len(self.connects)
        self.num_uvs = len(self.map_uvs) // 2 if self.has_uvs else 0
        self.offsets = [0] * (self.num_faces + 1)
        for i, c in enumerate(self.counts):
            self.offsets[i + 1] = self.offsets[i] + c
        self.point_at = {v: i for i, v in enumerate(d["point_ids"])} if d["point_ids"] else None
        self.edge_at = {v: i for i, v in enumerate(d["edge_ids"])} if d["edge_ids"] else None
        self.face_at = {v: i for i, v in enumerate(d["face_ids"])} if d["face_ids"] else None
        self._world = None
        self._vf = None
        self._ve = None
        self._ef = None
        self._nid = None

    def compact(self, fn, ids):
        at = {MFn.kMeshVertComponent: self.point_at, MFn.kMeshEdgeComponent: self.edge_at, MFn.kMeshPolygonComponent: self.face_at}.get(fn)
        return [at[i] for i in ids if i in at] if at else list(ids)

    def world(self):
        if self._world is None:
            self._world = MMatrix(_api("dag.matrices", id=self.shape)["world"])
        return self._world

    def point(self, i, space=MSpace.kObject):
        if not 0 <= i < self.num_vertices:
            _fail(message="Vertex index out of range")
        p = MPoint(self.points[i * 3], self.points[i * 3 + 1], self.points[i * 3 + 2])
        return p * self.world() if space == MSpace.kWorld else p

    def to_object(self, p, space):
        return MPoint(p) * self.world().inverse() if space == MSpace.kWorld else MPoint(p)

    def dir_out(self, x, y, z, space):
        v = MVector(x, y, z)
        if space == MSpace.kWorld:
            v = v.transformAsNormal(self.world()).normal()
        return v

    def face(self, f):
        if not 0 <= f < self.num_faces:
            _fail(message="Polygon index out of range")
        return self.offsets[f], self.offsets[f + 1]

    def face_vertices(self, f):
        a, b = self.face(f)
        return list(self.connects[a:b])

    def face_edges(self, f):
        a, b = self.face(f)
        return list(self.corner_edges[a:b])

    def edge_ends(self, e):
        if not 0 <= e < self.num_edges:
            _fail(message="Edge index out of range")
        return self.edges[e * 2], self.edges[e * 2 + 1]

    def vertex_faces(self):
        if self._vf is None:
            vf = [[] for _ in range(self.num_vertices)]
            for f in range(self.num_faces):
                for v in self.face_vertices(f):
                    vf[v].append(f)
            self._vf = vf
        return self._vf

    def vertex_edges(self):
        if self._ve is None:
            ve = [[] for _ in range(self.num_vertices)]
            for e in range(self.num_edges):
                ve[self.edges[e * 2]].append(e)
                ve[self.edges[e * 2 + 1]].append(e)
            self._ve = ve
        return self._ve

    def edge_faces(self):
        if self._ef is None:
            ef = [[] for _ in range(self.num_edges)]
            for f in range(self.num_faces):
                for e in self.face_edges(f):
                    ef[e].append(f)
            self._ef = ef
        return self._ef

    def corner(self, f, v):
        a, b = self.face(f)
        for c in range(a, b):
            if self.connects[c] == v:
                return c
        _fail(message="Vertex is not on that polygon")

    def corner_normal(self, c, space=MSpace.kObject):
        n = self.corner_normals
        return self.dir_out(n[c * 3], n[c * 3 + 1], n[c * 3 + 2], space)

    def vertex_normal(self, v, space=MSpace.kObject):
        if not 0 <= v < self.num_vertices:
            _fail(message="Vertex index out of range")
        n = self.point_normals
        return self.dir_out(n[v * 3], n[v * 3 + 1], n[v * 3 + 2], space)

    def face_normal(self, f, space=MSpace.kObject):
        nx = ny = nz = 0.0
        vs = self.face_vertices(f)
        p = self.points
        for i, a in enumerate(vs):
            b = vs[(i + 1) % len(vs)]
            ax, ay, az, bx, by, bz = p[a * 3], p[a * 3 + 1], p[a * 3 + 2], p[b * 3], p[b * 3 + 1], p[b * 3 + 2]
            nx += ay * bz - az * by
            ny += az * bx - ax * bz
            nz += ax * by - ay * bx
        return self.dir_out(*MVector(nx, ny, nz).normal(), space)

    def face_center(self, f, space=MSpace.kObject):
        vs = self.face_vertices(f)
        x = y = z = 0.0
        for v in vs:
            x += self.points[v * 3]
            y += self.points[v * 3 + 1]
            z += self.points[v * 3 + 2]
        c = MPoint(x / len(vs), y / len(vs), z / len(vs))
        return c * self.world() if space == MSpace.kWorld else c

    def face_area(self, f, space=MSpace.kObject):
        vs = [self.point(v, space) for v in self.face_vertices(f)]
        total = MVector()
        for i in range(1, len(vs) - 1):
            total = total + ((vs[i] - vs[0]) ^ (vs[i + 1] - vs[0]))
        return total.length() * 0.5

    def normal_ids(self):
        """Per corner its normal's id, and the normals: corners of a point sharing a normal share an id."""
        if self._nid is None:
            ids = [0] * self.num_corners
            normals = []
            seen = {}
            n = self.corner_normals
            for c in range(self.num_corners):
                key = (self.connects[c], round(n[c * 3], 5), round(n[c * 3 + 1], 5), round(n[c * 3 + 2], 5))
                i = seen.get(key)
                if i is None:
                    i = seen[key] = len(normals)
                    normals.append((n[c * 3], n[c * 3 + 1], n[c * 3 + 2]))
                ids[c] = i
            self._nid = (ids, normals)
        return self._nid

    def uv_of_corner(self, c):
        m = self.corner_maps[c]
        return self.map_uvs[m * 2], self.map_uvs[m * 2 + 1]


def _mesh_data(shape):
    v = _az.scene_version()
    e = _mesh_cache.get(shape)
    if e is not None and e[0] == v:
        return e[1]
    if _pending_points:
        _flush_points()
        v = _az.scene_version()
    m = _MeshData(_az.mesh_arrays(shape))
    _mesh_cache[shape] = (v, m)
    if len(_mesh_cache) > 16:
        for k in list(_mesh_cache):
            if k != shape:
                del _mesh_cache[k]
    return m


# ---------------------------------------------------------------------------------------------------------------------
# Base and dependency nodes


class MFnBase:
    def __init__(self, obj=None):
        self._obj = MObject()
        self._path = None
        if obj is not None:
            self.setObject(obj)

    def setObject(self, obj):
        if isinstance(obj, MDagPath):
            self._path = MDagPath(obj)
            obj = obj.node()
        if not isinstance(obj, MObject):
            _fail()
        self._check(obj)
        self._obj = MObject(obj)
        return self

    def _check(self, obj):
        pass

    def object(self):
        return MObject(self._obj)

    def type(self):
        return self._obj.apiType()

    def hasObj(self, obj):
        return True

    def _id(self):
        return _check_node(self._obj)

    def _nm(self):
        return _api("node.info", id=self._id())["path"]


class MFnDependencyNode(MFnBase):
    kLocalDynamicAttr = 0
    kNormalAttr = 1
    kExtensionAttr = 2
    kInvalidAttr = 3
    kTimerOff = 0
    kTimerOn = 1

    def _check(self, obj):
        if obj._kind != "node":
            _fail()

    def create(self, type_name, name=None):
        if isinstance(type_name, MTypeId):
            type_name = _node_type_of_id(type_name)
        kw = {"skipSelect": True}
        if name:
            kw["name"] = name
        self._obj = _node_object(_node_id_named(_cmds.createNode(type_name, **kw)))
        return self.object()

    def name(self):
        return _api("node.info", id=self._id())["name"]

    def absoluteName(self):
        return ":" + self.name()

    def setName(self, name, createNamespace=False):
        return _cmds.rename(self._nm(), name)

    @property
    def typeName(self):
        return _CStr(_api("node.info", id=self._id())["type"])

    @property
    def typeId(self):
        return _type_id_of(_api("node.info", id=self._id())["type"])

    @property
    def namespace(self):
        n = self.name()
        return n.rsplit(":", 1)[0] if ":" in n else ""

    @property
    def pluginName(self):
        return _api("node.info", id=self._id())["owner"]

    @property
    def isDefaultNode(self):
        return _CBool(False)

    @property
    def isFromReferencedFile(self):
        return _CBool(False)

    @property
    def isLocked(self):
        return _CBool(False)

    @property
    def isShared(self):
        return _CBool(False)

    def userNode(self):
        return _plugin_instance(self._id())

    def _attrs(self):
        return _api("node.attrs", id=self._id())

    def attributeCount(self):
        return len(self._attrs())

    def attribute(self, a):
        attrs = self._attrs()
        if isinstance(a, int):
            if not 0 <= a < len(attrs):
                raise IndexError("MFnDependencyNode attribute index out of range")
            return _attr_object(self._id(), attrs[a]["long"], attrs[a]["type"])
        for d in attrs:
            if a in (d["long"], d["short"]):
                return _attr_object(self._id(), d["long"], d["type"])
            if a in d["children"] or a in d["childShort"]:
                i = d["children"].index(a) if a in d["children"] else d["childShort"].index(a)
                return _attr_object(self._id(), d["children"][i], "float")
        _fail(message="Cannot find attribute named '%s'" % (a,))

    def hasAttribute(self, name):
        try:
            _api("plug.info", name=self._nm() + "." + name)
            return True
        except RuntimeError:
            return False

    def findPlug(self, attr, wantNetworked=False):
        name = _attr_name(attr)
        if name is None:
            _fail()
        p = MPlug._of(self._id(), name)
        p._info()
        return p

    def findAlias(self, alias):
        return self.attribute(alias)

    def addAttribute(self, attr):
        spec = attr._attr
        if not isinstance(spec, dict):
            _fail()
        _add_dynamic(self._nm(), spec)
        return self

    def removeAttribute(self, attr):
        n = _attr_name(attr)
        if n is None:
            _fail()
        spec = attr._attr if isinstance(attr._attr, dict) else None
        names = [k["name"] for k in spec.get("children", [])] if spec and spec["type"] != "vec3" else []
        for x in names + [n]:
            _cmds.deleteAttr(self._nm() + "." + x)
        return self

    def getConnections(self):
        id = self._id()
        out = MPlugArray()
        seen = set()
        name = self._nm()
        for s, d in _api("node.connections", id=id):
            for side in (s, d):
                if side.split(".", 1)[0] == name and side not in seen:
                    seen.add(side)
                    out.append(MPlug._named(side))
        return out

    def getAffectedAttributes(self, attr):
        return MObjectArray()


_ATTR_TYPE_FLAGS = {
    "float": {"attributeType": "double"},
    "int": {"attributeType": "long"},
    "bool": {"attributeType": "bool"},
    "enum": {"attributeType": "enum"},
    "vec3": {"attributeType": "double3"},
    "matrix": {"attributeType": "matrix"},
    "string": {"dataType": "string"},
}


def _add_dynamic(node, spec, parent=None):
    t = spec["type"]
    if t == "compound":
        _cmds.addAttr(node, longName=spec["name"], shortName=spec["short"] or spec["name"], attributeType="compound", multi=bool(spec.get("array")))
        for k in spec["children"]:
            _add_dynamic(node, k, spec["name"])
        return
    if t not in _ATTR_TYPE_FLAGS:
        _fail(status="kNotImplemented", message="Attributes of type %s can't be added to a node" % (t,))
    kw = dict(_ATTR_TYPE_FLAGS[t])
    kw["longName"] = spec["name"]
    if spec["short"]:
        kw["shortName"] = spec["short"]
    if parent:
        kw["parent"] = parent
    if spec.get("keyable"):
        kw["keyable"] = True
    if spec.get("hidden"):
        kw["hidden"] = True
    if spec.get("array"):
        kw["multi"] = True
    if spec.get("nice"):
        kw["niceName"] = spec["nice"]
    if spec.get("default") is not None and t in ("float", "int", "bool", "enum"):
        kw["defaultValue"] = spec["default"]
    for k, f in (("min", "minValue"), ("max", "maxValue"), ("softMin", "softMinValue"), ("softMax", "softMaxValue")):
        if spec.get(k) is not None:
            kw[f] = spec[k]
    if t == "enum":
        kw["enumName"] = ":".join(spec.get("enum") or ["none"])
    if t == "vec3" and spec.get("color"):
        kw["usedAsColor"] = True
    _cmds.addAttr(node, **kw)
    if t == "vec3":
        for i, axis in enumerate("XYZ"):
            kids = spec.get("children") or []
            long = kids[i]["name"] if kids else spec["name"] + axis
            short = kids[i]["short"] if kids else spec["short"] + axis.lower()
            if not _node_has_attr(node, long):
                _cmds.addAttr(node, longName=long, shortName=short, attributeType="double", parent=spec["name"])


def _node_has_attr(node, attr):
    try:
        _api("plug.info", name=node + "." + attr)
        return True
    except RuntimeError:
        return False


def _make_dag(type_name, name, parent):
    """A DAG node as the reference's API makes one: a shape with no parent gets a transform, and that is what comes back."""
    kw = {"skipSelect": True}
    if name:
        kw["name"] = name
    if parent:
        kw["parent"] = _api("node.info", id=parent)["path"]
    id = _node_id_named(_cmds.createNode(type_name, **kw))
    info = _api("node.info", id=id)
    if "shape" in info["chain"] and not parent:
        return info["parent"]
    return id


class MFnDagNode(MFnDependencyNode):
    kNextPos = 0xFF

    def setObject(self, obj):
        if isinstance(obj, MDagPath):
            self._path = MDagPath(obj)
            self._obj = obj.node()
            return self
        return MFnDependencyNode.setObject(self, obj)

    def _check(self, obj):
        if obj._kind != "node" or not _api("node.info", id=obj._id) or not _api("node.info", id=obj._id)["dag"]:
            _fail()

    def create(self, type_name, name=None, parent=None):
        if isinstance(type_name, MTypeId):
            type_name = _node_type_of_id(type_name)
        p = None if parent is None or parent.isNull() or parent._kind == "world" else _check_node(parent)
        self._obj = _node_object(_make_dag(type_name, name, p))
        self._path = None
        return self.object()

    def _info(self):
        return _api("node.info", id=self._id())

    def getPath(self):
        return MDagPath(self._path) if self._path else MDagPath._of(self._id())

    dagPath = getPath

    def fullPathName(self):
        return self._info()["full"]

    def partialPathName(self):
        return self._info()["path"]

    def parentCount(self):
        return 1

    def parent(self, index=0):
        if index != 0:
            raise IndexError("MFnDagNode parent index out of range")
        p = self._info()["parent"]
        return _node_object(p) if p else _world_object()

    def childCount(self):
        return len(self._info()["children"])

    def child(self, index):
        kids = self._info()["children"]
        if not 0 <= index < len(kids):
            raise IndexError("MFnDagNode child index out of range")
        return _node_object(kids[index])

    def hasChild(self, obj):
        return obj._id in self._info()["children"]

    def hasParent(self, obj):
        return obj._id == self._info()["parent"]

    def isChildOf(self, obj):
        return self.hasParent(obj)

    def isParentOf(self, obj):
        return self.hasChild(obj)

    def addChild(self, obj, index=kNextPos, keepExistingParents=False):
        _cmds.parent(_api("node.info", id=_check_node(obj))["path"], self._nm())
        return self

    def removeChild(self, obj):
        _cmds.parent(_api("node.info", id=_check_node(obj))["path"], world=True)
        return self

    def duplicate(self, instance=False, instanceLeaf=False):
        return _node_object(_node_id_named(_cmds.duplicate(self._nm())[0]))

    def instanceCount(self, indirect=False):
        return 1

    def getAllPaths(self):
        return MDagPathArray([self.getPath()])

    def dagRoot(self):
        return _node_object(self.getPath()._nodes[0])

    def transformationMatrix(self):
        return MMatrix(_api("dag.matrices", id=self._id())["local"])

    @property
    def boundingBox(self):
        b = _api("dag.bounds", id=self._id())
        return _CBox(MPoint(b[0], b[1], b[2]), MPoint(b[3], b[4], b[5])) if b else _CBox()

    @property
    def isIntermediateObject(self):
        return _CBool(self._info()["intermediate"])

    @isIntermediateObject.setter
    def isIntermediateObject(self, on):
        _cmds.setAttr(self._nm() + ".intermediateObject", bool(on))

    @property
    def inModel(self):
        return _CBool(True)

    @property
    def inUnderWorld(self):
        return _CBool(False)

    @property
    def isInstanceable(self):
        return _CBool(True)

    @property
    def objectColor(self):
        return 0


# ---------------------------------------------------------------------------------------------------------------------
# Transforms


def _vec_attr(name):
    v = _cmds.getAttr(name)
    if isinstance(v, list) and len(v) == 1:
        return list(v[0])
    return list(v)


def _set_vec_attr(name, v):
    _cmds.setAttr(name, float(v[0]), float(v[1]), float(v[2]))


class MFnTransform(MFnDagNode):
    kScaleMinLimit = 0

    def _check(self, obj):
        if not obj.hasFn(MFn.kTransform):
            _fail()

    def create(self, parent=None):
        p = None if parent is None or parent.isNull() or parent._kind == "world" else _check_node(parent)
        self._obj = _node_object(_make_dag("transform", None, p))
        self._path = None
        return self.object()

    def _attr(self, a):
        return self._nm() + "." + a

    def _has(self, a):
        return _node_has_attr(self._nm(), a)

    def _parent_matrix(self):
        return MMatrix(_api("dag.matrices", id=self._id())["parent"])

    def _world_matrix(self):
        return MMatrix(_api("dag.matrices", id=self._id())["world"])

    def _orient(self):
        """A joint's orient, between its rotation and its parent."""
        if not self._has("jointOrient"):
            return MQuaternion()
        v = _vec_attr(self._attr("jointOrient"))
        return MEulerRotation(v[0], v[1], v[2]).asQuaternion()

    # Translation

    def translation(self, space=MSpace.kTransform):
        t = _vec_attr(self._attr("translate"))
        if space == MSpace.kWorld:
            return MVector(MPoint(t) * self._parent_matrix())
        return MVector(t)

    def setTranslation(self, vector, space=MSpace.kTransform):
        v = vector
        if space == MSpace.kWorld:
            v = MPoint(vector[0], vector[1], vector[2]) * self._parent_matrix().inverse()
        _set_vec_attr(self._attr("translate"), v)
        return self

    def translateBy(self, vector, space=MSpace.kTransform):
        if space == MSpace.kWorld:
            t = self.translation(MSpace.kWorld) + MVector(vector[0], vector[1], vector[2])
            return self.setTranslation(t, MSpace.kWorld)
        v = MVector(vector[0], vector[1], vector[2])
        if space in (MSpace.kObject, MSpace.kPreTransform):
            v = v * self.rotation(asQuaternion=True).asMatrix()
        return self.setTranslation(MVector(_vec_attr(self._attr("translate"))) + v)

    # Rotation

    def rotationOrder(self):
        return MTransformationMatrix.kXYZ + int(_cmds.getAttr(self._attr("rotateOrder")))

    def setRotationOrder(self, order, reorder=True):
        e = self._euler()
        new = order - 1
        if reorder:
            e = e.reorder(new)
        _cmds.setAttr(self._attr("rotateOrder"), new)
        self._set_euler(e)
        return self

    def _euler(self):
        v = _vec_attr(self._attr("rotate"))
        return MEulerRotation(v[0], v[1], v[2], int(_cmds.getAttr(self._attr("rotateOrder"))))

    def _set_euler(self, e):
        _set_vec_attr(self._attr("rotate"), (e.x, e.y, e.z))

    def rotation(self, space=MSpace.kTransform, asQuaternion=False):
        if space == MSpace.kWorld:
            m = _strip_scale(self._world_matrix())
            order = int(_cmds.getAttr(self._attr("rotateOrder")))
            e = MEulerRotation.decompose(m, order)
            return e.asQuaternion() if asQuaternion else e
        e = self._euler()
        return e.asQuaternion() if asQuaternion else e

    def setRotation(self, rot, space=MSpace.kTransform):
        order = int(_cmds.getAttr(self._attr("rotateOrder")))
        q = rot.asQuaternion() if isinstance(rot, MEulerRotation) else rot
        if not isinstance(q, MQuaternion):
            _fail()
        if space == MSpace.kWorld:
            local = q.asMatrix() * _strip_scale(self._parent_matrix()).inverse()
            q = MEulerRotation.decompose(local, order).asQuaternion()
            q = q * self._orient().inverse()
        if isinstance(rot, MEulerRotation) and rot.order == order and space != MSpace.kWorld:
            self._set_euler(rot)
        else:
            self._set_euler(MEulerRotation.decompose(q.asMatrix(), order))
        return self

    def rotateBy(self, rot, space=MSpace.kTransform):
        q = rot.asQuaternion() if isinstance(rot, MEulerRotation) else rot
        cur = self._euler().asQuaternion()
        out = q * cur if space in (MSpace.kObject, MSpace.kPreTransform) else cur * q
        return self.setRotation(out)

    def rotateOrientation(self, space=MSpace.kTransform):
        if not self._has("rotateAxis"):
            return MQuaternion()
        v = _vec_attr(self._attr("rotateAxis"))
        return MEulerRotation(v[0], v[1], v[2]).asQuaternion()

    def setRotateOrientation(self, q, space=MSpace.kTransform, balance=True):
        e = MEulerRotation.decompose(q.asMatrix(), 0)
        _set_vec_attr(self._attr("rotateAxis"), (e.x, e.y, e.z))
        return self

    # Scale

    def scale(self):
        return _vec_attr(self._attr("scale"))

    def setScale(self, seq):
        _set_vec_attr(self._attr("scale"), seq)
        return self

    def scaleBy(self, seq):
        s = self.scale()
        return self.setScale([s[0] * seq[0], s[1] * seq[1], s[2] * seq[2]])

    def shear(self):
        return [0.0, 0.0, 0.0]

    # Pivots

    def _pivot(self, attr, space):
        p = MPoint(_vec_attr(self._attr(attr)))
        return p * self._world_matrix() if space == MSpace.kWorld else p

    def _set_pivot(self, attr, point, space):
        p = MPoint(point) * self._world_matrix().inverse() if space == MSpace.kWorld else MPoint(point)
        _set_vec_attr(self._attr(attr), (p.x, p.y, p.z))
        return self

    def rotatePivot(self, space=MSpace.kTransform):
        return self._pivot("rotatePivot", space)

    def setRotatePivot(self, point, space=MSpace.kTransform, balance=True):
        return self._set_pivot("rotatePivot", point, space)

    def scalePivot(self, space=MSpace.kTransform):
        return self._pivot("scalePivot", space)

    def setScalePivot(self, point, space=MSpace.kTransform, balance=True):
        return self._set_pivot("scalePivot", point, space)

    def rotatePivotTranslation(self, space=MSpace.kTransform):
        return MVector(_vec_attr(self._attr("rotatePivotTranslate")))

    def setRotatePivotTranslation(self, v, space=MSpace.kTransform):
        _set_vec_attr(self._attr("rotatePivotTranslate"), v)
        return self

    def scalePivotTranslation(self, space=MSpace.kTransform):
        return MVector(_vec_attr(self._attr("scalePivotTranslate")))

    def setScalePivotTranslation(self, v, space=MSpace.kTransform):
        _set_vec_attr(self._attr("scalePivotTranslate"), v)
        return self

    # The whole

    def transformation(self):
        tm = MTransformationMatrix()
        tm._order = int(_cmds.getAttr(self._attr("rotateOrder")))
        tm._t = MVector(_vec_attr(self._attr("translate")))
        e = self._euler()
        orient = self._orient()
        if orient == MQuaternion():
            tm.setRotation(e)
        else:
            tm.setRotation(e.asQuaternion() * orient)
        tm._s = self.scale()
        tm._rp = MVector(_vec_attr(self._attr("rotatePivot")))
        tm._sp = MVector(_vec_attr(self._attr("scalePivot")))
        tm._rpt = self.rotatePivotTranslation()
        tm._spt = self.scalePivotTranslation()
        tm._ro = self.rotateOrientation()
        return tm

    def setTransformation(self, tm):
        _set_vec_attr(self._attr("translate"), tm._t)
        self.setScale(tm._s)
        order = int(_cmds.getAttr(self._attr("rotateOrder")))
        q = tm._q * self._orient().inverse()
        self._set_euler(MEulerRotation.decompose(q.asMatrix(), order))
        _set_vec_attr(self._attr("rotatePivot"), tm._rp)
        _set_vec_attr(self._attr("scalePivot"), tm._sp)
        _set_vec_attr(self._attr("rotatePivotTranslate"), tm._rpt)
        _set_vec_attr(self._attr("scalePivotTranslate"), tm._spt)
        return self

    def resetFromRestPosition(self):
        return self


def _strip_scale(m):
    def norm(i):
        v = MVector(m[i], m[i + 1], m[i + 2]).normal()
        return [v.x, v.y, v.z, 0.0]

    return MMatrix(norm(0) + norm(4) + norm(8) + [0.0, 0.0, 0.0, 1.0])


# ---------------------------------------------------------------------------------------------------------------------
# Meshes


class MMeshIsectAccelParams:
    pass


class MFnMesh(MFnDagNode):
    kIntersectTolerance = 1e-6
    kPointTolerance = 1e-10
    kAlphaColor = 1
    kRGBColor = 3
    kRGBAColor = 4

    def __init__(self, obj=None):
        self._uv_pending = None
        MFnDagNode.__init__(self, obj)

    def setObject(self, obj):
        if isinstance(obj, MDagPath):
            path = MDagPath(obj)
            if path.hasFn(MFn.kTransform):
                try:
                    path.extendToShape()
                except RuntimeError:
                    _fail()
            if not path.hasFn(MFn.kMesh):
                _fail()
            self._path = path
            self._obj = path.node()
            return self
        if not isinstance(obj, MObject) or not obj.hasFn(MFn.kMesh):
            _fail()
        self._obj = MObject(obj)
        self._path = None
        return self

    def _m(self):
        return _mesh_data(self._id())

    @staticmethod
    def autoUniformGridParams():
        return MMeshIsectAccelParams()

    @staticmethod
    def uniformGridParams(xDivs, yDivs, zDivs):
        return MMeshIsectAccelParams()

    @staticmethod
    def globalIntersectionTolerance():
        return 1e-6

    # Making one

    def create(self, vertices, polygonCounts, polygonConnects, uValues=None, vValues=None, parent=None):
        pts = [[p[0], p[1], p[2]] for p in vertices]
        faces = []
        at = 0
        for c in polygonCounts:
            faces.append([int(i) for i in polygonConnects[at : at + c]])
            at += c
        if at != len(polygonConnects):
            _fail(message="polygonCounts and polygonConnects don't agree")
        kw = {"points": pts, "faces": faces}
        if uValues is not None and vValues is not None and len(uValues) == at and len(vValues) == at:
            kw["uvs"] = [[uValues[i], vValues[i]] for i in range(at)]
        made = _cmds.createMesh(**kw)
        made = made[0] if isinstance(made, list) else made
        id = _node_id_named(made)
        info = _api("node.info", id=id)
        transform = info["parent"] if "shape" in info["chain"] else id
        shape = _shape_below(transform, "mesh")
        if uValues is not None and vValues is not None and "uvs" not in kw:
            self._uv_pending = (list(uValues), list(vValues))
        self._obj = _node_object(shape)
        self._path = None
        if parent is not None and not parent.isNull() and parent._kind == "node":
            _cmds.parent(_api("node.info", id=transform)["path"], _api("node.info", id=parent._id)["path"])
            return _node_object(shape)
        return _node_object(transform)

    # Counts: properties in the reference, callable here as well

    @property
    def numVertices(self):
        return _CInt(self._m().num_vertices)

    @property
    def numEdges(self):
        return _CInt(self._m().num_edges)

    @property
    def numPolygons(self):
        return _CInt(self._m().num_faces)

    @property
    def numFaceVertices(self):
        return _CInt(self._m().num_corners)

    @property
    def numNormals(self):
        return _CInt(len(self._m().normal_ids()[1]))

    def numUVs(self, uvSet=""):
        return self._m().num_uvs

    def numUVSets(self):
        return 1

    @property
    def numColorSets(self):
        return _CInt(0)

    def numColors(self, colorSet=""):
        return 0

    # Points

    def getPoints(self, space=MSpace.kObject):
        out = MPointArray()
        list.extend(out, self._point_list(space, MPoint))
        return out

    def _point_list(self, space, cls):
        """The points as `cls` (MPoint or a subclass), built without going through MPoint's constructor: this is the
        call scripts make on whole meshes."""
        m = self._m()
        p = m.points
        n = m.num_vertices * 3
        new = object.__new__
        out = [None] * m.num_vertices
        k = 0
        if space == MSpace.kWorld:
            a = m.world()._m
            for i in range(0, n, 3):
                x, y, z = p[i], p[i + 1], p[i + 2]
                o = new(cls)
                w = x * a[3] + y * a[7] + z * a[11] + a[15]
                o.x = x * a[0] + y * a[4] + z * a[8] + a[12]
                o.y = x * a[1] + y * a[5] + z * a[9] + a[13]
                o.z = x * a[2] + y * a[6] + z * a[10] + a[14]
                o.w = w
                out[k] = o
                k += 1
        else:
            for i in range(0, n, 3):
                o = new(cls)
                o.x, o.y, o.z, o.w = float(p[i]), float(p[i + 1]), float(p[i + 2]), 1.0
                out[k] = o
                k += 1
        return out

    def getFloatPoints(self, space=MSpace.kObject):
        return MFloatPointArray(self.getPoints(space))

    def setPoints(self, points, space=MSpace.kObject):
        m = self._m()
        if len(points) != m.num_vertices:
            _fail(message="Mesh has %d vertices, got %d points" % (m.num_vertices, len(points)))
        flat = [0.0] * (3 * len(points))
        k = 0
        if space == MSpace.kWorld:
            a = m.world().inverse()._m
            for p in points:
                if isinstance(p, MPoint):
                    x, y, z, w = p.x, p.y, p.z, p.w
                else:
                    x, y, z = p[0], p[1], p[2]
                    w = p[3] if len(p) > 3 else 1.0
                qw = x * a[3] + y * a[7] + z * a[11] + w * a[15]
                flat[k] = (x * a[0] + y * a[4] + z * a[8] + w * a[12]) / qw
                flat[k + 1] = (x * a[1] + y * a[5] + z * a[9] + w * a[13]) / qw
                flat[k + 2] = (x * a[2] + y * a[6] + z * a[10] + w * a[14]) / qw
                k += 3
        else:
            for p in points:
                if isinstance(p, MPoint):
                    flat[k], flat[k + 1], flat[k + 2] = p.x, p.y, p.z
                else:
                    flat[k], flat[k + 1], flat[k + 2] = p[0], p[1], p[2]
                k += 3
        _write_points(m.shape, flat)
        return self

    def getPoint(self, index, space=MSpace.kObject):
        return self._m().point(index, space)

    def setPoint(self, index, point, space=MSpace.kObject):
        m = self._m()
        if not 0 <= index < m.num_vertices:
            _fail(message="Vertex index out of range")
        q = m.to_object(point, space)
        _pending_write(m, index, q)
        return self

    def updateSurface(self):
        _flush_points()
        return self

    def syncObject(self):
        _flush_points()
        return self

    # Topology

    def getVertices(self):
        m = self._m()
        return (MIntArray(m.counts), MIntArray(m.connects))

    def getPolygonVertices(self, polygonId):
        return MIntArray(self._m().face_vertices(polygonId))

    def getEdgeVertices(self, edgeId):
        return self._m().edge_ends(edgeId)

    def polygonVertexCount(self, polygonId):
        m = self._m()
        m.face(polygonId)
        return m.counts[polygonId]

    def getTriangles(self):
        m = self._m()
        counts = MIntArray()
        verts = MIntArray()
        for f in range(m.num_faces):
            vs = m.face_vertices(f)
            list.append(counts, len(vs) - 2)
            for t in range(1, len(vs) - 1):
                list.extend(verts, (vs[0], vs[t], vs[t + 1]))
        return (counts, verts)

    def getPolygonTriangleVertices(self, polygonId, triangleId):
        vs = self._m().face_vertices(polygonId)
        if not 0 <= triangleId < len(vs) - 2:
            _fail(message="Triangle index out of range")
        return (vs[0], vs[triangleId + 1], vs[triangleId + 2])

    def isPolygonConvex(self, polygonId):
        m = self._m()
        n = m.face_normal(polygonId)
        vs = [m.point(v) for v in m.face_vertices(polygonId)]
        for i in range(len(vs)):
            a, b, c = vs[i], vs[(i + 1) % len(vs)], vs[(i + 2) % len(vs)]
            if ((b - a) ^ (c - b)) * n < -1e-9:
                return False
        return True

    def onBoundary(self, polygonId):
        m = self._m()
        ef = m.edge_faces()
        return any(len(ef[e]) == 1 for e in m.face_edges(polygonId))

    # Normals

    def getNormals(self, space=MSpace.kObject):
        m = self._m()
        out = MFloatVectorArray()
        for n in m.normal_ids()[1]:
            list.append(out, MFloatVector(m.dir_out(n[0], n[1], n[2], space)))
        return out

    def getNormalIds(self):
        m = self._m()
        return (MIntArray(m.counts), MIntArray(m.normal_ids()[0]))

    def getVertexNormal(self, vertexId, angleWeighted=False, space=MSpace.kObject):
        return MVector(self._m().vertex_normal(vertexId, space))

    def getVertexNormals(self, angleWeighted=False, space=MSpace.kObject):
        m = self._m()
        return MFloatVectorArray([m.vertex_normal(v, space) for v in range(m.num_vertices)])

    def getFaceVertexNormal(self, faceId, vertexId, space=MSpace.kObject):
        m = self._m()
        return MVector(m.corner_normal(m.corner(faceId, vertexId), space))

    def getFaceVertexNormals(self, faceId, space=MSpace.kObject):
        m = self._m()
        a, b = m.face(faceId)
        return MFloatVectorArray([m.corner_normal(c, space) for c in range(a, b)])

    def getFaceNormalIds(self, faceId):
        m = self._m()
        a, b = m.face(faceId)
        return MIntArray(m.normal_ids()[0][a:b])

    def getPolygonNormal(self, polygonId, space=MSpace.kObject):
        return self._m().face_normal(polygonId, space)

    # UVs

    def getCurrentUVSetName(self, instance=-1):
        return "map1"

    def setCurrentUVSetName(self, name, modifier=None, currentSelection=None):
        if name != "map1":
            _fail(message="Only the set map1 exists")
        return self

    def getUVSetNames(self):
        return ["map1"]

    def getUVs(self, uvSet=""):
        m = self._m()
        if not m.has_uvs:
            return (MFloatArray(), MFloatArray())
        return (MFloatArray(m.map_uvs[0::2]), MFloatArray(m.map_uvs[1::2]))

    def setUVs(self, uValues, vValues, uvSet=""):
        if len(uValues) != len(vValues):
            _fail(message="u and v arrays differ in length")
        m = self._m()
        if m.num_uvs and len(uValues) == m.num_uvs:
            us = [float(uValues[i]) for i in m.corner_maps]
            vs = [float(vValues[i]) for i in m.corner_maps]
            _cmds.setMeshUVs(self._nm(), u=us, v=vs)
        else:
            # New UVs: they take effect when assignUVs says which corner has which.
            self._uv_pending = (list(uValues), list(vValues))
        return self

    def assignUVs(self, uvCounts, uvIds, uvSet=""):
        if self._uv_pending is None:
            _fail(message="Call setUVs first")
        us, vs = self._uv_pending
        _cmds.setMeshUVs(self._nm(), u=[float(us[i]) for i in uvIds], v=[float(vs[i]) for i in uvIds])
        self._uv_pending = None
        return self

    def getAssignedUVs(self, uvSet=""):
        m = self._m()
        return (MIntArray(m.counts), MIntArray(m.corner_maps))

    def getPolygonUV(self, polygonId, vertexIndex, uvSet=""):
        m = self._m()
        a, b = m.face(polygonId)
        if not 0 <= vertexIndex < b - a or not m.has_uvs:
            _fail(message="Polygon has no such UV")
        return m.uv_of_corner(a + vertexIndex)

    def getPolygonUVid(self, polygonId, vertexIndex, uvSet=""):
        m = self._m()
        a, b = m.face(polygonId)
        if not 0 <= vertexIndex < b - a or not m.has_uvs:
            _fail(message="Polygon has no such UV")
        return m.corner_maps[a + vertexIndex]

    def getPointAtUV(self, faceId, u, v, space=MSpace.kObject, uvSet="", tolerance=0.0):
        m = self._m()
        if not m.has_uvs:
            _fail(status="kFailure", message="Mesh has no UVs")
        a, b = m.face(faceId)
        n = b - a
        for t in range(1, n - 1):
            c0, c1, c2 = a, a + t, a + t + 1
            u0, v0 = m.uv_of_corner(c0)
            u1, v1 = m.uv_of_corner(c1)
            u2, v2 = m.uv_of_corner(c2)
            d = (u1 - u0) * (v2 - v0) - (u2 - u0) * (v1 - v0)
            if d == 0.0:
                continue
            s1 = ((u - u0) * (v2 - v0) - (u2 - u0) * (v - v0)) / d
            s2 = ((u1 - u0) * (v - v0) - (u - u0) * (v1 - v0)) / d
            eps = max(tolerance, 1e-6)
            if s1 >= -eps and s2 >= -eps and s1 + s2 <= 1.0 + eps:
                p0, p1, p2 = m.point(m.connects[c0]), m.point(m.connects[c1]), m.point(m.connects[c2])
                out = p0 + (p1 - p0) * s1 + (p2 - p0) * s2
                return out * m.world() if space == MSpace.kWorld else out
        _fail(status="kFailure", message="The face does not contain that UV")

    # Measuring and rays

    def area(self, space=MSpace.kObject):
        m = self._m()
        return sum(m.face_area(f, space) for f in range(m.num_faces))

    def _ray(self, source, direction, space):
        m = self._m()
        o, d = MPoint(source), MVector(direction[0], direction[1], direction[2])
        if space == MSpace.kWorld:
            inv = m.world().inverse()
            o = o * inv
            d = d * inv
        return m, o, d

    def _hits(self, source, direction, space, maxParam, testBothDirections, faceIds):
        m, o, d = self._ray(source, direction, space)
        hits = _api("mesh.hits", id=m.shape, origin=[o.x, o.y, o.z], dir=[d.x, d.y, d.z], max=float(maxParam), both=bool(testBothDirections))
        if faceIds is not None:
            keep = set(faceIds)
            hits = [h for h in hits if int(h[1]) in keep]
        return m, o, d, hits

    def _hit_point(self, m, o, d, h, space):
        p = MPoint(o.x + d.x * h[0], o.y + d.y * h[0], o.z + d.z * h[0])
        return MFloatPoint(p * m.world() if space == MSpace.kWorld else p)

    def closestIntersection(self, raySource, rayDirection, space, maxParam, testBothDirections, faceIds=None, triIds=None, idsSorted=False, accelParams=None, tolerance=1e-6, **kw):
        m, o, d, hits = self._hits(raySource, rayDirection, space, maxParam, testBothDirections, faceIds)
        if not hits:
            return (MFloatPoint(), 0.0, -1, -1, 0.0, 0.0)
        h = hits[0]
        return (self._hit_point(m, o, d, h, space), h[0], int(h[1]), int(h[2]), h[3], h[4])

    anyIntersection = closestIntersection

    def allIntersections(self, raySource, rayDirection, space, maxParam, testBothDirections, faceIds=None, triIds=None, idsSorted=False, accelParams=None, sortHits=False, tolerance=1e-6, **kw):
        m, o, d, hits = self._hits(raySource, rayDirection, space, maxParam, testBothDirections, faceIds)
        return (
            MFloatPointArray([self._hit_point(m, o, d, h, space) for h in hits]),
            MFloatArray([h[0] for h in hits]),
            MIntArray([int(h[1]) for h in hits]),
            MIntArray([int(h[2]) for h in hits]),
            MFloatArray([h[3] for h in hits]),
            MFloatArray([h[4] for h in hits]),
        )

    def _closest(self, point, space):
        m = self._m()
        q = m.to_object(point, space)
        r = _api("mesh.closest", id=m.shape, point=[q.x, q.y, q.z])
        if r is None:
            _fail(status="kFailure", message="Mesh has no faces")
        return m, MPoint(r[0], r[1], r[2]), r[3]

    def getClosestPoint(self, point, space=MSpace.kObject, accelParams=None):
        m, p, f = self._closest(point, space)
        return (p * m.world() if space == MSpace.kWorld else p, f)

    def getClosestNormal(self, point, space=MSpace.kObject, accelParams=None):
        m, _, f = self._closest(point, space)
        return (m.face_normal(f, space), f)

    def getClosestPointAndNormal(self, point, space=MSpace.kObject, accelParams=None):
        m, p, f = self._closest(point, space)
        return (p * m.world() if space == MSpace.kWorld else p, m.face_normal(f, space), f)


def _pending_write(m, index, q):
    # Readers see it at once in the snapshot; normals follow when it goes out, as in the reference after updateSurface.
    _pending_points.setdefault(m.shape, {})[index] = (q.x, q.y, q.z)
    m.points[index * 3], m.points[index * 3 + 1], m.points[index * 3 + 2] = q.x, q.y, q.z


# ---------------------------------------------------------------------------------------------------------------------
# Curves


class MFnNurbsCurve(MFnDagNode):
    kInvalid = 0
    kOpen = 1
    kClosed = 2
    kPeriodic = 3
    kLast = 4
    kPointTolerance = 1e-3
    kFindParamTolerance = 1e-6

    def setObject(self, obj):
        if isinstance(obj, MDagPath):
            path = MDagPath(obj)
            if path.hasFn(MFn.kTransform):
                try:
                    path.extendToShape()
                except RuntimeError:
                    _fail()
            if not path.hasFn(MFn.kNurbsCurve):
                _fail()
            self._path = path
            self._obj = path.node()
            return self
        if not isinstance(obj, MObject) or not obj.hasFn(MFn.kNurbsCurve):
            _fail()
        self._obj = MObject(obj)
        self._path = None
        return self

    def _d(self):
        return _api("curve.data", id=self._id())

    def _world(self):
        return MMatrix(_api("dag.matrices", id=self._id())["world"])

    def create(self, cvs, knots, degree, form, is2D=False, rational=False, parent=None):
        pts = [[p[0], p[1], p[2]] for p in cvs]
        kw = {"point": pts, "degree": int(degree)}
        if form == MFnNurbsCurve.kPeriodic:
            kw["periodic"] = True
        if knots:
            kw["knot"] = list(knots)
        made = _cmds.curve(**kw)
        made = made[0] if isinstance(made, list) else made
        transform = _node_id_named(made)
        self._obj = _node_object(_shape_below(transform, "nurbsCurve"))
        self._path = None
        if parent is not None and not parent.isNull() and parent._kind == "node":
            _cmds.parent(_api("node.info", id=transform)["path"], _api("node.info", id=parent._id)["path"])
            return MObject(self._obj)
        return _node_object(transform)

    @property
    def numCVs(self):
        return _CInt(len(self._d()["cvs"]) // 3)

    @property
    def numSpans(self):
        return _CInt(self._d()["spans"])

    @property
    def degree(self):
        return _CInt(self._d()["degree"])

    @property
    def form(self):
        return _CInt(self._d()["form"] + 1)

    @property
    def numKnots(self):
        return _CInt(len(self._d()["knots"]))

    @property
    def knotDomain(self):
        return tuple(self._d()["domain"])

    def knots(self):
        return MDoubleArray(self._d()["knots"])

    def knot(self, index):
        return self._d()["knots"][index]

    def cvPosition(self, index, space=MSpace.kObject):
        c = self._d()["cvs"]
        if not 0 <= index < len(c) // 3:
            raise IndexError("MFnNurbsCurve CV index out of range")
        p = MPoint(c[index * 3], c[index * 3 + 1], c[index * 3 + 2])
        return p * self._world() if space == MSpace.kWorld else p

    def cvPositions(self, space=MSpace.kObject):
        c = self._d()["cvs"]
        w = self._world() if space == MSpace.kWorld else None
        out = MPointArray()
        for i in range(len(c) // 3):
            p = MPoint(c[i * 3], c[i * 3 + 1], c[i * 3 + 2])
            list.append(out, p * w if w else p)
        return out

    def setCVPositions(self, points, space=MSpace.kObject):
        inv = self._world().inverse() if space == MSpace.kWorld else None
        pts = []
        for p in points:
            q = MPoint(p) * inv if inv else MPoint(p)
            pts.append([q.x, q.y, q.z])
        _cmds.setCurvePoints(self._nm(), points=pts)
        return self

    def setCVPosition(self, index, point, space=MSpace.kObject):
        pts = self.cvPositions(space)
        pts[index] = point
        return self.setCVPositions(pts, space)

    def updateCurve(self):
        return self

    def length(self, tolerance=1e-3):
        return self._d()["length"]

    def findParamFromLength(self, partLength, tolerance=1e-3):
        return _api("curve.eval", id=self._id(), what="paramAtLength", length=float(partLength))

    def findLengthFromParam(self, param, tolerance=1e-3):
        lo, hi = self._d()["domain"]
        steps = 200
        last = self.getPointAtParam(lo)
        total = 0.0
        for i in range(1, steps + 1):
            p = self.getPointAtParam(lo + (param - lo) * i / steps)
            total += p.distanceTo(last)
            last = p
        return total

    def getPointAtParam(self, param, space=MSpace.kObject):
        r = _api("curve.eval", id=self._id(), what="point", param=float(param))
        p = MPoint(r)
        return p * self._world() if space == MSpace.kWorld else p

    def tangent(self, param, space=MSpace.kObject):
        r = _api("curve.eval", id=self._id(), what="tangent", param=float(param))
        v = MVector(r)
        return v * self._world() if space == MSpace.kWorld else v

    def closestPoint(self, point, paramGuess=False, tolerance=1e-3, space=MSpace.kObject):
        q = MPoint(point) * self._world().inverse() if space == MSpace.kWorld else MPoint(point)
        r = _api("curve.eval", id=self._id(), what="closest", point=[q.x, q.y, q.z])
        p = MPoint(r["point"])
        return (p * self._world() if space == MSpace.kWorld else p, r["param"])

    def getParamAtPoint(self, point, tolerance=1e-3, space=MSpace.kObject):
        p, t = self.closestPoint(point, False, tolerance, space)
        if p.distanceTo(MPoint(point)) > max(tolerance, 1e-3):
            _fail(status="kFailure", message="Point is not on the curve")
        return t

    def isPointOnCurve(self, point, tolerance=1e-3, space=MSpace.kObject):
        p, _ = self.closestPoint(point, False, tolerance, space)
        return p.distanceTo(MPoint(point)) <= tolerance

    def isParamOnCurve(self, param):
        lo, hi = self._d()["domain"]
        return lo <= param <= hi


# ---------------------------------------------------------------------------------------------------------------------
# Sets


class MFnSet(MFnDependencyNode):
    kNone = 0
    kVerticesOnly = 1
    kEdgesOnly = 2
    kFacetsOnly = 3
    kEditPointsOnly = 4
    kRenderableOnly = 5

    def create(self, members, restriction=kNone):
        names = members.getSelectionStrings() if isinstance(members, MSelectionList) else []
        made = _cmds.createSet(names, empty=True) if not names else _cmds.createSet(names)
        made = made[0] if isinstance(made, list) else made
        self._obj = _node_object(_node_id_named(made))
        return self.object()

    def getMembers(self, flatten=False):
        out = MSelectionList()
        for n in _cmds.setMembers(self._nm()) or []:
            out.add(n)
        return out

    def addMember(self, *args):
        s = MSelectionList()
        s.add(args[0] if len(args) == 1 else (args[0], args[1]))
        _cmds.addToSet(self._nm(), s.getSelectionStrings())
        return self

    def addMembers(self, sel):
        _cmds.addToSet(self._nm(), sel.getSelectionStrings())
        return self

    def removeMember(self, *args):
        s = MSelectionList()
        s.add(args[0] if len(args) == 1 else (args[0], args[1]))
        _cmds.removeFromSet(self._nm(), s.getSelectionStrings())
        return self

    def removeMembers(self, sel):
        _cmds.removeFromSet(self._nm(), sel.getSelectionStrings())
        return self

    def clear(self):
        names = _cmds.setMembers(self._nm()) or []
        if names:
            _cmds.removeFromSet(self._nm(), names)
        return self

    def isMember(self, *args):
        s = MSelectionList()
        s.add(args[0] if len(args) == 1 else (args[0], args[1]))
        return self.getMembers().hasItem(args[0] if len(args) == 1 else (args[0], args[1]))

    def restriction(self):
        return MFnSet.kNone

    @property
    def annotation(self):
        return ""


# ---------------------------------------------------------------------------------------------------------------------
# Attributes


def _attr_prop(key, default):
    def get(self):
        return self._spec().get(key, default)

    def put(self, v):
        self._spec()[key] = v

    return property(get, put)


class MFnAttribute(MFnBase):
    kNothingChanged = 0
    kDelete = 0
    kReset = 1
    kNothing = 2

    def _check(self, obj):
        if obj._kind != "attr":
            _fail()

    def _spec(self):
        s = self._obj._attr
        if s is None:
            _fail()
        if isinstance(s, str):
            spec = {"name": s, "short": "", "type": "float"}
            node = self._obj._data
            if isinstance(node, int) and not isinstance(node, bool):
                try:
                    i = _api("plug.info", name="%s.%s" % (_api("node.info", id=node)["path"], s))
                except RuntimeError:
                    i = None
                if i is not None:
                    spec.update(
                        short=i["short"], type=i["type"], array=i["array"], input=i["input"] or not i["output"],
                        keyable=i["keyable"], storable=i["storable"], hidden=i["hidden"], channelBox=i["channelBox"],
                        color=i["color"], nice=i["nice"] or "", dynamic=i["user"],
                    )
            s = spec
        return s

    @property
    def name(self):
        return self._spec()["name"]

    @property
    def shortName(self):
        return self._spec().get("short", "")

    keyable = _attr_prop("keyable", False)
    hidden = _attr_prop("hidden", False)
    array = _attr_prop("array", False)
    readable = _attr_prop("readable", True)
    connectable = _attr_prop("connectable", True)
    cached = _attr_prop("cached", True)
    channelBox = _attr_prop("channelBox", False)
    usedAsColor = _attr_prop("color", False)
    usedAsFilename = _attr_prop("filename", False)
    indexMatters = _attr_prop("indexMatters", True)
    usesArrayDataBuilder = _attr_prop("arrayBuilder", False)
    disconnectBehavior = _attr_prop("disconnectBehavior", 2)
    affectsWorldSpace = _attr_prop("worldSpace", False)
    affectsAppearance = _attr_prop("appearance", False)
    renderSource = _attr_prop("renderSource", False)
    worldSpace = _attr_prop("worldSpace", False)
    indeterminant = _attr_prop("indeterminant", False)
    dynamic = _attr_prop("dynamic", False)
    niceName = _attr_prop("nice", "")

    @property
    def storable(self):
        s = self._spec()
        return s.get("storable", s.get("input", True))

    @storable.setter
    def storable(self, v):
        self._spec()["storable"] = bool(v)

    @property
    def writable(self):
        return self._spec().get("input", True)

    @writable.setter
    def writable(self, v):
        self._spec()["input"] = bool(v)

    def setNiceNameOverride(self, name):
        self._spec()["nice"] = name
        return self

    def isFreeToChange(self):
        return True


def _new_attr(fn, long, short, type_name, default=None, **more):
    spec = {"name": long, "short": short, "type": type_name, "default": default}
    spec.update(more)
    o = MObject()
    o._kind = "attr"
    o._attr = spec
    o._fn = fn
    return o


class MFnNumericData(MFnBase):
    kInvalid = 0
    kBoolean = 1
    kByte = 2
    kChar = 3
    kShort = 4
    k2Short = 5
    k3Short = 6
    kLong = 7
    kInt = 7
    k2Long = 8
    k2Int = 8
    k3Long = 9
    k3Int = 9
    kInt64 = 10
    kFloat = 11
    k2Float = 12
    k3Float = 13
    kDouble = 14
    k2Double = 15
    k3Double = 16
    k4Double = 17
    kAddr = 18
    kLast = 19

    def create(self, kind=kDouble):
        o = MObject()
        o._kind = "data"
        o._fn = MFn.kNumericData
        o._data = 0.0
        self._obj = o
        return MObject(o)

    def getData(self):
        d = self._obj._data
        return list(d) if isinstance(d, (list, tuple)) else [d]

    def setData(self, v):
        self._obj._data = list(v) if isinstance(v, (list, tuple)) else v
        return self


_NUMERIC_TYPE = {
    MFnNumericData.kBoolean: "bool",
    MFnNumericData.kByte: "int",
    MFnNumericData.kChar: "int",
    MFnNumericData.kShort: "int",
    MFnNumericData.kLong: "int",
    MFnNumericData.kInt64: "int",
    MFnNumericData.kFloat: "float",
    MFnNumericData.kDouble: "float",
    MFnNumericData.k3Double: "vec3",
    MFnNumericData.k3Float: "vec3",
    MFnNumericData.k3Short: "vec3",
    MFnNumericData.k3Long: "vec3",
}


class MFnNumericAttribute(MFnAttribute):
    def create(self, longName, shortName, dataType, defaultValue=0):
        t = _NUMERIC_TYPE.get(dataType)
        if t is None:
            _fail(status="kNotImplemented", message="Numeric attributes of that type aren't available")
        d = defaultValue
        if t == "vec3":
            d = list(d) if isinstance(d, (list, tuple)) else [float(d)] * 3
        elif t == "bool":
            d = bool(d)
        elif t == "int":
            d = int(d)
        else:
            d = float(d)
        self._obj = _new_attr(MFn.kNumericAttribute, longName, shortName, t, d, numeric=dataType)
        return self.object()

    def createPoint(self, longName, shortName):
        self._obj = _new_attr(MFn.kNumericAttribute, longName, shortName, "vec3", [0.0, 0.0, 0.0], numeric=MFnNumericData.k3Float)
        return self.object()

    def createColor(self, longName, shortName):
        self._obj = _new_attr(MFn.kNumericAttribute, longName, shortName, "vec3", [0.0, 0.0, 0.0], numeric=MFnNumericData.k3Float, color=True)
        return self.object()

    def numericType(self):
        return self._spec().get("numeric", MFnNumericData.kDouble)

    @property
    def default(self):
        return self._spec().get("default")

    @default.setter
    def default(self, v):
        self._spec()["default"] = list(v) if isinstance(v, (list, tuple)) else v

    def setMin(self, *v):
        self._spec()["min"] = float(v[0])
        return self

    def setMax(self, *v):
        self._spec()["max"] = float(v[0])
        return self

    def setSoftMin(self, *v):
        self._spec()["softMin"] = float(v[0])
        return self

    def setSoftMax(self, *v):
        self._spec()["softMax"] = float(v[0])
        return self

    def hasMin(self):
        return "min" in self._spec()

    def hasMax(self):
        return "max" in self._spec()

    def getMin(self):
        return self._spec().get("min")

    def getMax(self):
        return self._spec().get("max")


class MFnUnitAttribute(MFnNumericAttribute):
    kInvalid = 0
    kAngle = 1
    kDistance = 2
    kTime = 3
    kLast = 4

    def create(self, longName, shortName, unitType, defaultValue=0.0):
        d = defaultValue
        if isinstance(d, MAngle):
            d = d.asRadians()
        elif isinstance(d, MDistance):
            d = d.asCentimeters()
        elif isinstance(d, MTime):
            d = d.asUnits(MTime.uiUnit())
        unit = {1: "angle", 2: "distance", 3: "time"}.get(unitType)
        self._obj = _new_attr(MFn.kUnitAttribute, longName, shortName, "float", float(d), unit=unit)
        return self.object()

    def unitType(self):
        return {"angle": 1, "distance": 2, "time": 3}[self._spec()["unit"]]


class MFnData:
    kInvalid = 0
    kNumeric = 1
    kPlugin = 2
    kPluginGeometry = 3
    kString = 4
    kMatrix = 5
    kStringArray = 6
    kDoubleArray = 7
    kFloatArray = 8
    kIntArray = 9
    kPointArray = 10
    kVectorArray = 11
    kComponentList = 13
    kMesh = 14
    kLattice = 15
    kNurbsCurve = 16
    kNurbsSurface = 17
    kSphere = 18
    kDynArrayAttrs = 19
    kAny = 23
    kLast = 24


class MFnTypedAttribute(MFnAttribute):
    def create(self, longName, shortName, dataType, defaultValue=None):
        t = {MFnData.kString: "string", MFnData.kMatrix: "matrix", MFnData.kMesh: "mesh"}.get(dataType)
        if t is None:
            _fail(status="kNotImplemented", message="Typed attributes of that data type aren't available")
        d = None
        if isinstance(defaultValue, MObject) and defaultValue._kind == "data":
            d = defaultValue._data
            d = list(d) if isinstance(d, MMatrix) else d
        self._obj = _new_attr(MFn.kTypedAttribute, longName, shortName, t, d, data=dataType)
        return self.object()

    def attrType(self):
        return self._spec().get("data", MFnData.kInvalid)


class MFnEnumAttribute(MFnAttribute):
    def create(self, longName, shortName, defaultValue=0):
        self._obj = _new_attr(MFn.kEnumAttribute, longName, shortName, "enum", int(defaultValue), enum=[])
        return self.object()

    def addField(self, name, value):
        names = self._spec()["enum"]
        while len(names) <= value:
            names.append("")
        names[value] = name
        return self

    def fieldName(self, value):
        return self._spec()["enum"][value]

    def fieldValue(self, name):
        return self._spec()["enum"].index(name)

    def getMin(self):
        return 0

    def getMax(self):
        return len(self._spec()["enum"]) - 1

    def setDefaultByName(self, name):
        self._spec()["default"] = self.fieldValue(name)
        return self


class MFnCompoundAttribute(MFnAttribute):
    def create(self, longName, shortName):
        self._obj = _new_attr(MFn.kCompoundAttribute, longName, shortName, "compound", None, children=[])
        return self.object()

    def addChild(self, child):
        s = child._attr
        if not isinstance(s, dict):
            _fail()
        s["parent"] = self._spec()["name"]
        self._spec()["children"].append(s)
        return self

    def removeChild(self, child):
        self._spec()["children"].remove(child._attr)
        return self

    def numChildren(self):
        return len(self._spec()["children"])

    def child(self, index):
        o = MObject()
        o._kind = "attr"
        o._attr = self._spec()["children"][index]
        o._fn = MFn.kNumericAttribute
        return o

    def getAddAttrCmds(self, useLongName=False):
        return []


class MFnMatrixAttribute(MFnAttribute):
    kFloat = 1
    kDouble = 2

    def create(self, longName, shortName, type=2):
        self._obj = _new_attr(MFn.kMatrixAttribute, longName, shortName, "matrix", None)
        return self.object()

    @property
    def default(self):
        d = self._spec().get("default")
        return MMatrix(d) if d else MMatrix()

    @default.setter
    def default(self, m):
        self._spec()["default"] = list(m)


class MFnMessageAttribute(MFnAttribute):
    def create(self, longName, shortName):
        self._obj = _new_attr(MFn.kMessageAttribute, longName, shortName, "bool", False, hidden=True, storable=False)
        return self.object()


class MFnGenericAttribute(MFnAttribute):
    def create(self, longName, shortName):
        self._obj = _new_attr(MFn.kGenericAttribute, longName, shortName, "float", 0.0)
        return self.object()

    def addNumericType(self, *t):
        return self

    def addUnitType(self, *t):
        return self

    def addDataType(self, *t):
        return self

    def addTypeId(self, *t):
        return self


# ---------------------------------------------------------------------------------------------------------------------
# Data


class MFnMatrixData(MFnBase):
    def _check(self, obj):
        if obj._kind != "data" or obj._fn != MFn.kMatrixData:
            _fail()

    def create(self, m=None):
        o = MObject()
        o._kind, o._fn, o._data = "data", MFn.kMatrixData, MMatrix(m) if m is not None else MMatrix()
        self._obj = o
        return MObject(o)

    def matrix(self):
        self._check(self._obj)
        return MMatrix(self._obj._data)

    def transformation(self):
        return MTransformationMatrix(self.matrix())

    def set(self, m):
        self._check(self._obj)
        self._obj._data = MMatrix(m._m if isinstance(m, _MatrixBase) else m.asMatrix())
        return self


class MFnStringData(MFnBase):
    def create(self, s=""):
        o = MObject()
        o._kind, o._fn, o._data = "data", MFn.kStringData, str(s)
        self._obj = o
        return MObject(o)

    def string(self):
        return self._obj._data

    def set(self, s):
        self._obj._data = str(s)
        return self


# ---------------------------------------------------------------------------------------------------------------------
# Components


class MFnComponent(MFnBase):
    def _check(self, obj):
        if obj._kind != "comp":
            _fail()

    @property
    def elementCount(self):
        return _CInt(len(self._obj._comp))

    @property
    def componentType(self):
        return self._obj._fn

    @property
    def hasWeights(self):
        return False

    @property
    def isComplete(self):
        return False

    @property
    def isEmpty(self):
        return not self._obj._comp

    def isEqual(self, o):
        return sorted(self._obj._comp) == sorted(o._comp)


class MFnSingleIndexedComponent(MFnComponent):
    def create(self, fn):
        self._obj = _component_object(fn, [])
        return self._obj

    def addElement(self, i):
        self._obj._comp.append(int(i))
        return self

    def addElements(self, seq):
        self._obj._comp.extend(int(i) for i in seq)
        return self

    def element(self, i):
        return self._obj._comp[i]

    def getElements(self):
        return MIntArray(self._obj._comp)

    def setCompleteData(self, n):
        self._obj._comp = list(range(n))
        return self

    def getCompleteData(self):
        return len(self._obj._comp)


class MFnDoubleIndexedComponent(MFnComponent):
    def create(self, fn):
        self._obj = _component_object(fn, [])
        return self._obj

    def addElement(self, a, b):
        self._obj._comp.append((int(a), int(b)))
        return self

    def addElements(self, a, b):
        self._obj._comp.extend((int(x), int(y)) for x, y in zip(a, b))
        return self

    def getElement(self, i):
        return self._obj._comp[i]

    def getElements(self):
        return (MIntArray([x for x, _ in self._obj._comp]), MIntArray([y for _, y in self._obj._comp]))
