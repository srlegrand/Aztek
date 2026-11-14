# aztek.api.OpenAztek, NURBS surfaces: MFnNurbsSurface, reading and evaluating a surface.


class MFnNurbsSurface(MFnDagNode):
    kInvalid = 0
    kOpen = 1
    kClosed = 2
    kPeriodic = 3
    kLast = 4
    kInvalidBoundary = 0
    kOuter = 1
    kInner = 2
    kSegment = 3
    kClosedSegment = 4
    kPointTolerance = 1e-3

    def setObject(self, obj):
        if isinstance(obj, MDagPath):
            path = MDagPath(obj)
            if path.hasFn(MFn.kTransform):
                try:
                    path.extendToShape()
                except RuntimeError:
                    _fail()
            if not path.hasFn(MFn.kNurbsSurface):
                _fail()
            self._path = path
            self._obj = path.node()
            return self
        if not isinstance(obj, MObject) or not obj.hasFn(MFn.kNurbsSurface):
            _fail()
        self._obj = MObject(obj)
        self._path = None
        return self

    def _d(self):
        return _api("surface.data", id=self._id())

    def _eval(self, what, u=0.0, v=0.0, **more):
        return _api("surface.eval", id=self._id(), what=what, u=float(u), v=float(v), **more)

    def _world(self, space):
        if space != MSpace.kWorld:
            return None
        return MMatrix(_api("dag.matrices", id=self._id())["world"])

    # Properties

    @property
    def numCVsInU(self):
        return self._d()["numU"]

    @property
    def numCVsInV(self):
        return self._d()["numV"]

    @property
    def numSpansInU(self):
        return self._d()["spansU"]

    @property
    def numSpansInV(self):
        return self._d()["spansV"]

    @property
    def degreeInU(self):
        return self._d()["degreeU"]

    @property
    def degreeInV(self):
        return self._d()["degreeV"]

    @property
    def formInU(self):
        return self._d()["formU"] + 1

    @property
    def formInV(self):
        return self._d()["formV"] + 1

    @property
    def numKnotsInU(self):
        return len(self._d()["knotsU"])

    @property
    def numKnotsInV(self):
        return len(self._d()["knotsV"])

    @property
    def knotDomainInU(self):
        d = self._d()["domain"]
        return (d[0], d[1])

    @property
    def knotDomainInV(self):
        d = self._d()["domain"]
        return (d[2], d[3])

    @property
    def isTrimmedSurface(self):
        return bool(self._d()["trimmed"])

    @property
    def numPatchesInU(self):
        return self._d()["spansU"]

    @property
    def numPatchesInV(self):
        return self._d()["spansV"]

    @property
    def numPatches(self):
        d = self._d()
        return d["spansU"] * d["spansV"]

    # Knots

    def knotsInU(self):
        return MDoubleArray(self._d()["knotsU"])

    def knotsInV(self):
        return MDoubleArray(self._d()["knotsV"])

    def knotInU(self, index):
        return self._d()["knotsU"][index]

    def knotInV(self, index):
        return self._d()["knotsV"][index]

    # CVs

    def cvPositions(self, space=MSpace.kObject):
        c = self._d()["cvs"]
        w = self._world(space)
        out = MPointArray()
        for i in range(0, len(c) - 2, 3):
            p = MPoint(c[i], c[i + 1], c[i + 2])
            out.append(p * w if w is not None else p)
        return out

    def cvPosition(self, uIndex, vIndex, space=MSpace.kObject):
        d = self._d()
        if not (0 <= uIndex < d["numU"] and 0 <= vIndex < d["numV"]):
            _fail(message="CV index out of range")
        k = (uIndex * d["numV"] + vIndex) * 3
        c = d["cvs"]
        p = MPoint(c[k], c[k + 1], c[k + 2])
        w = self._world(space)
        return p * w if w is not None else p

    # Evaluation

    def getPointAtParam(self, uParam, vParam, space=MSpace.kObject):
        p = MPoint(self._eval("point", uParam, vParam))
        w = self._world(space)
        return p * w if w is not None else p

    def normal(self, uParam, vParam, space=MSpace.kObject):
        n = MVector(self._eval("normal", uParam, vParam))
        w = self._world(space)
        return (n * w.inverse().transpose()).normal() if w is not None else n

    def getDerivativesAtParam(self, uParam, vParam, space=MSpace.kObject, secondOrder=False):
        d = self._eval("derivatives", uParam, vParam)
        p, du, dv = MPoint(d["point"]), MVector(d["du"]), MVector(d["dv"])
        w = self._world(space)
        if w is not None:
            p, du, dv = p * w, du * w, dv * w
        if secondOrder:
            # Second derivatives aren't given here.
            return (p, du, dv, MVector(), MVector(), MVector())
        return (p, du, dv)

    def tangents(self, uParam, vParam, space=MSpace.kObject):
        _, du, dv = self.getDerivativesAtParam(uParam, vParam, space)
        return (du.normal(), dv.normal())

    def closestPoint(self, testPoint, uStart=None, vStart=None, ignoreTrimBoundaries=False, tolerance=kPointTolerance, space=MSpace.kObject):
        w = self._world(space)
        local = MPoint(testPoint) * w.inverse() if w is not None else MPoint(testPoint)
        r = self._eval("closest", point=[local.x, local.y, local.z])
        p = MPoint(r["point"])
        return (p * w if w is not None else p, r["u"], r["v"])

    def getParamAtPoint(self, point, ignoreTrimBoundaries=False, tolerance=kPointTolerance, space=MSpace.kObject):
        p, u, v = self.closestPoint(point, None, None, ignoreTrimBoundaries, tolerance, space)
        if p.distanceTo(MPoint(point)) > max(tolerance, 1e-6):
            _fail(status="kFailure", message="The point is not on the surface")
        return (u, v)

    def isPointOnSurface(self, point, tolerance=kPointTolerance, space=MSpace.kObject):
        p, _, _ = self.closestPoint(point, None, None, False, tolerance, space)
        return p.distanceTo(MPoint(point)) <= tolerance

    def distanceToPoint(self, point, space=MSpace.kObject):
        p, _, _ = self.closestPoint(point, None, None, False, self.kPointTolerance, space)
        return p.distanceTo(MPoint(point))

    def isParamOnSurface(self, uParam, vParam):
        d = self._d()["domain"]
        return d[0] <= uParam <= d[1] and d[2] <= vParam <= d[3]

    def area(self, space=MSpace.kObject, tolerance=kPointTolerance):
        """In the surface's own space, from a tessellation: close, not exact."""
        return float(self._eval("area"))
