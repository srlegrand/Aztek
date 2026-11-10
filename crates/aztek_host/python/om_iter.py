# aztek.api.OpenMaya, iterators over selections, the DAG, the graph and mesh components.


class MItSelectionList:
    kUnknownItem = -1
    kDagSelectionItem = 0
    kAnimSelectionItem = 1
    kDNselectionItem = 2
    kPlugSelectionItem = 3

    def __init__(self, sel, filter=MFn.kInvalid):
        self._sel = sel
        self._filter = filter
        self.reset()

    def _keeps(self, i):
        if self._filter == MFn.kInvalid:
            return True
        it = self._sel._items[i]
        if it.comp is not None and it.comp.hasFn(self._filter):
            return True
        if _node_object(it.node).hasFn(self._filter):
            return True
        # A selected transform counts as the shape below it: filtering by kMesh finds a selected cube.
        if it.plug is None and it.comp is None:
            below = _shape_below(it.node)
            return below != it.node and _node_object(below).hasFn(self._filter)
        return False

    def _skip(self):
        while self._i < len(self._sel._items) and not self._keeps(self._i):
            self._i += 1

    def reset(self):
        self._i = 0
        self._skip()
        return self

    def setFilter(self, filter):
        self._filter = filter
        return self

    def isDone(self):
        return self._i >= len(self._sel._items)

    def next(self):
        self._i += 1
        self._skip()
        return self

    def itemType(self):
        it = self._sel._items[self._i]
        if it.plug is not None:
            return MItSelectionList.kPlugSelectionItem
        return MItSelectionList.kDagSelectionItem if _api("node.info", id=it.node)["dag"] else MItSelectionList.kDNselectionItem

    def getDagPath(self):
        return self._sel.getDagPath(self._i)

    def getDependNode(self):
        return self._sel.getDependNode(self._i)

    def getPlug(self):
        return self._sel.getPlug(self._i)

    def getComponent(self):
        return self._sel.getComponent(self._i)

    def hasComponents(self):
        return self._sel._items[self._i].comp is not None

    def getStrings(self):
        return self._sel.getSelectionStrings(self._i)


class MItDag:
    kInvalidType = 0
    kDepthFirst = 1
    kBreadthFirst = 2

    def __init__(self, traversalType=kDepthFirst, filterType=MFn.kInvalid):
        self._root = None
        self.reset(None, traversalType, filterType)

    def reset(self, root=None, traversalType=None, filterType=None):
        if traversalType is not None:
            self._trav = traversalType
        if filterType is not None:
            self._filter = filterType
        if root is not None:
            self._root = root._leaf() if isinstance(root, MDagPath) else _check_node(root)
        if self._root:
            self._stack = [(self._root, 0)]
        else:
            self._stack = [(r, 0) for r in _api("node.roots") if _api("node.info", id=r)["dag"]]
            if self._trav == MItDag.kDepthFirst:
                self._stack.reverse()
        self._cur = None
        self._pruned = False
        self._advance()
        return self

    def _advance(self):
        while self._stack:
            if self._trav == MItDag.kDepthFirst:
                id, depth = self._stack.pop()
            else:
                id, depth = self._stack.pop(0)
            info = _api("node.info", id=id)
            if not info:
                continue
            kids = [(k, depth + 1) for k in info["children"]]
            if self._trav == MItDag.kDepthFirst:
                self._stack.extend(reversed(kids))
            else:
                self._stack.extend(kids)
            self._kids = len(kids)
            if self._filter == MFn.kInvalid or _node_object(id).hasFn(self._filter):
                self._cur = (id, depth)
                return
        self._cur = None

    def isDone(self):
        return self._cur is None

    def next(self):
        self._advance()
        return self

    def prune(self):
        """Don't go below the current node."""
        if self._cur is None:
            return self
        kids = set(_api("node.info", id=self._cur[0])["children"])
        self._stack = [e for e in self._stack if e[0] not in kids]
        return self

    def currentItem(self):
        if self._cur is None:
            _fail(status="kFailure", message="Iterator is done")
        return _node_object(self._cur[0])

    def getPath(self):
        return MDagPath._of(self.currentItem()._id)

    def fullPathName(self):
        return _api("node.info", id=self.currentItem()._id)["full"]

    def partialPathName(self):
        return _api("node.info", id=self.currentItem()._id)["path"]

    def depth(self):
        return self._cur[1] if self._cur else 0

    def root(self):
        return _node_object(self._root) if self._root else _world_object()

    def instanceCount(self, total=False):
        return 1

    def isInstanced(self, indirect=True):
        return False

    def traversalType(self):
        return self._trav


class MItDependencyNodes:
    def __init__(self, filter=MFn.kInvalid):
        self._filter = filter
        self.reset()

    def reset(self, filter=None):
        if filter is not None:
            self._filter = filter
        self._ids = _api("node.all")
        self._i = -1
        self.next()
        return self

    def next(self):
        self._i += 1
        while self._i < len(self._ids) and self._filter != MFn.kInvalid and not _node_object(self._ids[self._i]).hasFn(self._filter):
            self._i += 1
        return self

    def isDone(self):
        return self._i >= len(self._ids)

    def thisNode(self):
        if self.isDone():
            _fail(status="kFailure", message="Iterator is done")
        return _node_object(self._ids[self._i])


class MItDependencyGraph:
    kDownstream = 0
    kUpstream = 1
    kDepthFirst = 0
    kBreadthFirst = 1
    kNodeLevel = 0
    kPlugLevel = 1

    def __init__(self, root, filter=MFn.kInvalid, direction=kDownstream, traversal=kDepthFirst, level=kNodeLevel):
        if isinstance(root, MPlug):
            self._root_id = root._info()["node"]
            self._root_plug = MPlug(root)
        else:
            self._root_id = _check_node(root)
            self._root_plug = None
        self._filter, self._dir, self._trav, self._level = filter, direction, traversal, level
        self.reset()

    def reset(self):
        self._stack = [(self._root_id, self._root_plug)]
        self._seen = set()
        self._visited = []
        self._cur = None
        self._advance()
        return self

    def resetTo(self, root, filter=MFn.kInvalid, direction=kDownstream, traversal=kDepthFirst, level=kNodeLevel):
        self.__init__(root, filter, direction, traversal, level)
        return self

    def _neighbours(self, id):
        out = []
        for s, d in _api("node.connections", id=id):
            sid, _ = _split_plug(s)
            did, _ = _split_plug(d)
            if self._dir == MItDependencyGraph.kDownstream and sid == id and did != id:
                out.append((did, MPlug._named(d)))
            elif self._dir == MItDependencyGraph.kUpstream and did == id and sid != id:
                out.append((sid, MPlug._named(s)))
        return out

    def _advance(self):
        while self._stack:
            id, plug = self._stack.pop() if self._trav == MItDependencyGraph.kDepthFirst else self._stack.pop(0)
            if id in self._seen:
                continue
            self._seen.add(id)
            nexts = self._neighbours(id)
            self._stack.extend(reversed(nexts) if self._trav == MItDependencyGraph.kDepthFirst else nexts)
            if self._filter == MFn.kInvalid or _node_object(id).hasFn(self._filter):
                self._cur = (id, plug)
                self._visited.append((id, plug))
                return
        self._cur = None

    def isDone(self):
        return self._cur is None

    def next(self):
        self._advance()
        return self

    def prune(self):
        """Don't go beyond the current node."""
        if self._cur:
            away = {n for n, _ in self._neighbours(self._cur[0])}
            self._stack = [e for e in self._stack if e[0] not in away]
        return self

    def currentNode(self):
        if self._cur is None:
            _fail(status="kFailure", message="Iterator is done")
        return _node_object(self._cur[0])

    currentItem = currentNode

    def currentPlug(self):
        if self._cur is None or self._cur[1] is None:
            return MPlug()
        return MPlug(self._cur[1])

    thisPlug = currentPlug

    def rootNode(self):
        return _node_object(self._root_id)

    def rootPlug(self):
        return MPlug(self._root_plug) if self._root_plug else MPlug()

    def getNodesVisited(self):
        return MObjectArray([_node_object(i) for i, _ in self._visited])

    def getPlugsVisited(self):
        return MPlugArray([p for _, p in self._visited if p is not None])

    def currentFilter(self):
        return self._filter


class _MeshIter:
    """Walks a mesh's components: all of them, or those of a component object."""

    _fn = MFn.kMeshVertComponent

    def __init__(self, obj, component=None):
        if isinstance(obj, MDagPath):
            path = MDagPath(obj)
            if path.hasFn(MFn.kTransform):
                try:
                    path.extendToShape()
                except RuntimeError:
                    _fail()
            self._shape = path._leaf()
        elif isinstance(obj, MObject):
            self._shape = _check_node(obj)
            self._shape = _shape_below(self._shape, "mesh")
        else:
            _fail()
        if not _node_object(self._shape).hasFn(MFn.kMesh):
            _fail()
        self._set_component(component)
        self._k = 0

    def _set_component(self, component):
        self._n = None
        if component is None or component.isNull():
            self._ids = None
            return
        if component._kind != "comp" or component._fn != self._fn:
            _fail()
        self._ids = _mesh_data(self._shape).compact(self._fn, component._comp)

    def _m(self):
        return _mesh_data(self._shape)

    def _total(self):
        raise NotImplementedError

    def _index(self):
        if self.isDone():
            _fail(status="kFailure", message="Iterator is done")
        return self._k if self._ids is None else self._ids[self._k]

    def index(self):
        return self._index()

    def isDone(self):
        # The count is taken once a walk starts: a loop of next() and isDone() should not ask the scene each step.
        n = self.__dict__.get("_n")
        if n is None:
            n = self._n = self._total() if self._ids is None else len(self._ids)
        return self._k >= n

    def next(self):
        self._k += 1
        return self

    def reset(self, *args):
        if args:
            obj = args[0]
            self.__init__(obj, args[1] if len(args) > 1 else None)
        self._k = 0
        self._n = None
        return self

    def count(self):
        return self._total()

    def setIndex(self, index):
        total = self._total()
        if not 0 <= index < total:
            _fail(message="Index out of range")
        prev = self._index() if not self.isDone() else -1
        self._ids = None
        self._n = None
        self._k = index
        return prev

    def currentItem(self):
        return _component_object(self._fn, [self._index()])


class MItMeshVertex(_MeshIter):
    _fn = MFn.kMeshVertComponent

    def _total(self):
        return self._m().num_vertices

    def position(self, space=MSpace.kObject):
        return self._m().point(self._index(), space)

    def setPosition(self, point, space=MSpace.kObject):
        m = self._m()
        _pending_write(m, self._index(), m.to_object(point, space))
        return self

    def translateBy(self, vector, space=MSpace.kObject):
        return self.setPosition(self.position(space) + vector, space)

    def updateSurface(self):
        _flush_points()
        return self

    def getNormal(self, *args):
        """getNormal(space) or getNormal(faceId, space)."""
        m = self._m()
        i = self._index()
        if args and isinstance(args[0], int) and len(args) == 2:
            return m.corner_normal(m.corner(args[0], i), args[1])
        return m.vertex_normal(i, args[0] if args else MSpace.kObject)

    def getNormals(self, space=MSpace.kObject):
        m = self._m()
        i = self._index()
        return MVectorArray([m.corner_normal(m.corner(f, i), space) for f in m.vertex_faces()[i]])

    def numConnectedEdges(self):
        return len(self._m().vertex_edges()[self._index()])

    def numConnectedFaces(self):
        return len(self._m().vertex_faces()[self._index()])

    def getConnectedVertices(self):
        m = self._m()
        i = self._index()
        out = MIntArray()
        for e in m.vertex_edges()[i]:
            a, b = m.edge_ends(e)
            list.append(out, b if a == i else a)
        return out

    def getConnectedEdges(self):
        return MIntArray(self._m().vertex_edges()[self._index()])

    def getConnectedFaces(self):
        return MIntArray(self._m().vertex_faces()[self._index()])

    connectedVertices = getConnectedVertices
    connectedEdges = getConnectedEdges
    connectedFaces = getConnectedFaces

    def connectedToEdge(self, edge):
        return edge in self._m().vertex_edges()[self._index()]

    def connectedToFace(self, face):
        return face in self._m().vertex_faces()[self._index()]

    def onBoundary(self):
        m = self._m()
        ef = m.edge_faces()
        return any(len(ef[e]) == 1 for e in m.vertex_edges()[self._index()])

    def getColor(self, *a):
        return MColor()

    def _corners(self):
        m = self._m()
        i = self._index()
        return [m.corner(f, i) for f in m.vertex_faces()[i]]

    def hasUVs(self, uvSet=""):
        return self._m().has_uvs

    def numUVs(self, uvSet=""):
        m = self._m()
        return len({m.corner_maps[c] for c in self._corners()}) if m.has_uvs else 0

    def getUV(self, *args):
        """getUV(uvSet) or getUV(faceId, uvSet)."""
        m = self._m()
        if not m.has_uvs:
            _fail(status="kFailure", message="Mesh has no UVs")
        i = self._index()
        if args and isinstance(args[0], int):
            return m.uv_of_corner(m.corner(args[0], i))
        return m.uv_of_corner(self._corners()[0])

    def getUVs(self, uvSet=""):
        m = self._m()
        us, vs, fs = MFloatArray(), MFloatArray(), MIntArray()
        if m.has_uvs:
            i = self._index()
            for f in m.vertex_faces()[i]:
                u, v = m.uv_of_corner(m.corner(f, i))
                us.append(u)
                vs.append(v)
                fs.append(f)
        return (us, vs, fs)


class MItMeshEdge(_MeshIter):
    _fn = MFn.kMeshEdgeComponent

    def _total(self):
        return self._m().num_edges

    def vertexId(self, which):
        if which not in (0, 1):
            _fail()
        return self._m().edge_ends(self._index())[which]

    def point(self, which, space=MSpace.kObject):
        return self._m().point(self.vertexId(which), space)

    def setPoint(self, point, which, space=MSpace.kObject):
        m = self._m()
        _pending_write(m, self.vertexId(which), m.to_object(point, space))
        return self

    def updateSurface(self):
        _flush_points()
        return self

    def center(self, space=MSpace.kObject):
        a, b = self.point(0, space), self.point(1, space)
        return MPoint((a.x + b.x) / 2, (a.y + b.y) / 2, (a.z + b.z) / 2)

    def length(self, space=MSpace.kObject):
        return self.point(0, space).distanceTo(self.point(1, space))

    def _edge_neighbours(self):
        m = self._m()
        i = self._index()
        a, b = m.edge_ends(i)
        ve = m.vertex_edges()
        return [e for e in ve[a] + ve[b] if e != i]

    def numConnectedEdges(self):
        return len(self._edge_neighbours())

    def getConnectedEdges(self):
        return MIntArray(self._edge_neighbours())

    connectedEdges = getConnectedEdges

    def numConnectedFaces(self):
        return len(self._m().edge_faces()[self._index()])

    def getConnectedFaces(self):
        return MIntArray(self._m().edge_faces()[self._index()])

    connectedFaces = getConnectedFaces

    def connectedToEdge(self, edge):
        return edge in self._edge_neighbours()

    def connectedToFace(self, face):
        return face in self._m().edge_faces()[self._index()]

    def onBoundary(self):
        return len(self._m().edge_faces()[self._index()]) == 1


class MItMeshPolygon(_MeshIter):
    _fn = MFn.kMeshPolygonComponent

    def _total(self):
        return self._m().num_faces

    def polygonVertexCount(self):
        return self._m().counts[self._index()]

    def getVertices(self):
        return MIntArray(self._m().face_vertices(self._index()))

    def vertexIndex(self, which):
        vs = self._m().face_vertices(self._index())
        if not 0 <= which < len(vs):
            _fail(message="Index out of range")
        return vs[which]

    def getEdges(self):
        return MIntArray(self._m().face_edges(self._index()))

    def edgeCount(self):
        return len(self._m().face_edges(self._index()))

    def point(self, which, space=MSpace.kObject):
        return self._m().point(self.vertexIndex(which), space)

    def setPoint(self, point, which, space=MSpace.kObject):
        m = self._m()
        _pending_write(m, self.vertexIndex(which), m.to_object(point, space))
        return self

    def getPoints(self, space=MSpace.kObject):
        m = self._m()
        return MPointArray([m.point(v, space) for v in m.face_vertices(self._index())])

    def setPoints(self, points, space=MSpace.kObject):
        m = self._m()
        for v, p in zip(m.face_vertices(self._index()), points):
            _pending_write(m, v, m.to_object(p, space))
        return self

    def updateSurface(self):
        _flush_points()
        return self

    def center(self, space=MSpace.kObject):
        return self._m().face_center(self._index(), space)

    def getNormal(self, *args):
        """getNormal(space) for the face, or getNormal(vertexIndex, space) at a corner."""
        m = self._m()
        f = self._index()
        if args and isinstance(args[0], int) and len(args) == 2:
            return m.corner_normal(m.offsets[f] + args[0], args[1])
        return m.face_normal(f, args[0] if args else MSpace.kObject)

    def getNormals(self, space=MSpace.kObject):
        m = self._m()
        a, b = m.face(self._index())
        return MVectorArray([m.corner_normal(c, space) for c in range(a, b)])

    def normalIndex(self, which):
        m = self._m()
        return m.normal_ids()[0][m.offsets[self._index()] + which]

    def getArea(self, space=MSpace.kObject):
        return self._m().face_area(self._index(), space)

    def numTriangles(self):
        return self.polygonVertexCount() - 2

    def getTriangle(self, which, space=MSpace.kObject):
        m = self._m()
        vs = m.face_vertices(self._index())
        if not 0 <= which < len(vs) - 2:
            _fail(message="Triangle index out of range")
        ids = [vs[0], vs[which + 1], vs[which + 2]]
        return (MPointArray([m.point(v, space) for v in ids]), MIntArray(ids))

    def getTriangles(self, space=MSpace.kObject):
        m = self._m()
        vs = m.face_vertices(self._index())
        pts, ids = MPointArray(), MIntArray()
        for t in range(1, len(vs) - 1):
            for v in (vs[0], vs[t], vs[t + 1]):
                pts.append(m.point(v, space))
                ids.append(v)
        return (pts, ids)

    def _face_neighbours(self):
        m = self._m()
        f = self._index()
        ef = m.edge_faces()
        out = []
        for e in m.face_edges(f):
            for g in ef[e]:
                if g != f and g not in out:
                    out.append(g)
        return out

    def numConnectedFaces(self):
        return len(self._face_neighbours())

    def getConnectedFaces(self):
        return MIntArray(self._face_neighbours())

    connectedFaces = getConnectedFaces

    def _edge_neighbours(self):
        m = self._m()
        mine = set(m.face_edges(self._index()))
        ve = m.vertex_edges()
        out = []
        for v in m.face_vertices(self._index()):
            for e in ve[v]:
                if e not in mine and e not in out:
                    out.append(e)
        return out

    def numConnectedEdges(self):
        return len(self._edge_neighbours())

    def getConnectedEdges(self):
        return MIntArray(self._edge_neighbours())

    connectedEdges = getConnectedEdges

    def getConnectedVertices(self):
        m = self._m()
        mine = set(m.face_vertices(self._index()))
        out = []
        for e in self._edge_neighbours():
            a, b = m.edge_ends(e)
            for v in (a, b):
                if v not in mine and v not in out:
                    out.append(v)
        return MIntArray(out)

    connectedVertices = getConnectedVertices

    def hasUVs(self, uvSet=""):
        return self._m().has_uvs

    def numUVs(self, uvSet=""):
        return self.polygonVertexCount() if self._m().has_uvs else 0

    def getUV(self, which, uvSet=""):
        m = self._m()
        if not m.has_uvs:
            _fail(status="kFailure", message="Mesh has no UVs")
        return m.uv_of_corner(m.offsets[self._index()] + which)

    def getUVIndex(self, which, uvSet=""):
        m = self._m()
        return m.corner_maps[m.offsets[self._index()] + which]

    def getUVs(self, uvSet=""):
        m = self._m()
        a, b = m.face(self._index())
        pairs = [m.uv_of_corner(c) for c in range(a, b)] if m.has_uvs else []
        return (MFloatArray([p[0] for p in pairs]), MFloatArray([p[1] for p in pairs]))

    def isConvex(self):
        return MFnMesh(_node_object(self._shape)).isPolygonConvex(self._index())

    def isPlanar(self):
        m = self._m()
        f = self._index()
        n = m.face_normal(f)
        vs = [m.point(v) for v in m.face_vertices(f)]
        return all(abs((p - vs[0]) * n) < 1e-6 for p in vs)

    def onBoundary(self):
        m = self._m()
        ef = m.edge_faces()
        return any(len(ef[e]) == 1 for e in m.face_edges(self._index()))

    def zeroArea(self):
        return self.getArea() < 1e-12


class MItMeshFaceVertex(_MeshIter):
    _fn = MFn.kMeshVtxFaceComponent

    def _set_component(self, component):
        self._ids = None
        self._n = None
        if component is not None and not component.isNull():
            m = _mesh_data(self._shape)
            if component._kind == "comp" and component._fn == self._fn:
                self._ids = [m.corner(f, v) for v, f in component._comp]
            elif component._kind == "comp" and component._fn == MFn.kMeshPolygonComponent:
                self._ids = [c for f in m.compact(component._fn, component._comp) for c in range(*m.face(f))]
            else:
                _fail()

    def _total(self):
        return self._m().num_corners

    def _corner_face(self):
        """(corner, face) now; found by walking, as corners are few per step."""
        import bisect

        m = self._m()
        c = self._index()
        return c, bisect.bisect_right(m.offsets, c) - 1

    def vertexId(self):
        return self._m().connects[self._index()]

    def faceId(self):
        return self._corner_face()[1]

    def faceVertexId(self):
        c, f = self._corner_face()
        return c - self._m().offsets[f]

    faceVertex = faceVertexId

    def position(self, space=MSpace.kObject):
        return self._m().point(self.vertexId(), space)

    def getNormal(self, space=MSpace.kObject):
        return self._m().corner_normal(self._index(), space)

    def normalId(self):
        return self._m().normal_ids()[0][self._index()]

    def hasUVs(self, uvSet=""):
        return self._m().has_uvs

    def getUV(self, uvSet=""):
        m = self._m()
        if not m.has_uvs:
            _fail(status="kFailure", message="Mesh has no UVs")
        return m.uv_of_corner(self._index())

    def getUVIndex(self, uvSet=""):
        return self._m().corner_maps[self._index()]

    def hasColor(self):
        return False

    def getColor(self, *a):
        return MColor()


class MItGeometry:
    """A mesh's vertices or a curve's CVs as plain points."""

    def __init__(self, obj, component=None):
        if isinstance(obj, MDagPath):
            path = MDagPath(obj)
            if path.hasFn(MFn.kTransform):
                try:
                    path.extendToShape()
                except RuntimeError:
                    _fail()
            self._shape = path._leaf()
        else:
            self._shape = _shape_below(_check_node(obj))
        o = _node_object(self._shape)
        self._mesh = o.hasFn(MFn.kMesh)
        if not self._mesh and not o.hasFn(MFn.kNurbsCurve):
            _fail()
        self._ids = None
        if component is not None and not component.isNull():
            self._ids = list(component._comp)
        self._k = 0

    def _total(self):
        if self._mesh:
            return _mesh_data(self._shape).num_vertices
        return len(_api("curve.data", id=self._shape)["cvs"]) // 3

    def count(self):
        return self._total() if self._ids is None else len(self._ids)

    exactCount = count

    def index(self):
        return self._k if self._ids is None else self._ids[self._k]

    def isDone(self):
        return self._k >= self.count()

    def next(self):
        self._k += 1
        return self

    def reset(self):
        self._k = 0
        return self

    def currentItem(self):
        fn = MFn.kMeshVertComponent if self._mesh else MFn.kCurveCVComponent
        return _component_object(fn, [self.index()])

    def position(self, space=MSpace.kObject):
        if self._mesh:
            return _mesh_data(self._shape).point(self.index(), space)
        return MFnNurbsCurve(_node_object(self._shape)).cvPosition(self.index(), space)

    def setPosition(self, point, space=MSpace.kObject):
        if self._mesh:
            m = _mesh_data(self._shape)
            _pending_write(m, self.index(), m.to_object(point, space))
        else:
            MFnNurbsCurve(_node_object(self._shape)).setCVPosition(self.index(), point, space)
        return self

    def normal(self, space=MSpace.kObject):
        if not self._mesh:
            return MVector()
        return _mesh_data(self._shape).vertex_normal(self.index(), space)

    def weight(self):
        return _component_object(MFn.kMeshVertComponent, [self.index()])

    def allPositions(self, space=MSpace.kObject):
        if self._mesh:
            return MFnMesh(_node_object(self._shape)).getPoints(space)
        return MFnNurbsCurve(_node_object(self._shape)).cvPositions(space)

    def setAllPositions(self, points, space=MSpace.kObject):
        if self._mesh:
            MFnMesh(_node_object(self._shape)).setPoints(points, space)
        else:
            MFnNurbsCurve(_node_object(self._shape)).setCVPositions(points, space)
        return self
