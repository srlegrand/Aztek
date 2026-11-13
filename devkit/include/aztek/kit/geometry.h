// Geometry function sets: MFnMesh over a mesh a compute was given, MFnNurbsCurve over a curve it was given.
#pragma once
#include "plugin.h"

struct AzCurve;

namespace az {
// A curve a compute holds: the host's handle, released with the last object that points at it.
struct CurveData {
    AzCurve* handle = 0;
    ~CurveData();
};
MObject curveObject(AzCurve* handle);
}

class MFnNurbsCurveData : public MFnGeometryData {
public:
    MFnNurbsCurveData();
    MFnNurbsCurveData(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
};

class MFnNurbsCurve : public MFnBase {
public:
    enum Form { kInvalid = 0, kOpen, kClosed, kPeriodic, kLast };
    MFnNurbsCurve();
    MFnNurbsCurve(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    static const char* className() { return "MFnNurbsCurve"; }
    int numCVs(MStatus* status = 0) const;
    int degree(MStatus* status = 0) const;
    int numSpans(MStatus* status = 0) const;
    int numKnots(MStatus* status = 0) const;
    Form form(MStatus* status = 0) const;
    MStatus getKnotDomain(double& start, double& end) const;
    MStatus getCVs(MPointArray& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getCV(unsigned int index, MPoint& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getPointAtParam(double param, MPoint& out, MSpace::Space space = MSpace::kObject) const;
    MVector tangent(double param, MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MVector normal(double param, MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    double length(double tolerance = 1.0e-3, MStatus* status = 0) const;
    double findParamFromLength(double partLength, MStatus* status = 0) const;
    double findParamFromLength(double partLength, double tolerance, MStatus* status = 0) const;
    double findLengthFromParam(double param, MStatus* status = 0) const;
    MPoint closestPoint(const MPoint& to, double* param = 0, double tolerance = 1.0e-3, MSpace::Space space = MSpace::kObject, MStatus* status = 0) const;
    MStatus getParamAtPoint(const MPoint& at, double& param, MSpace::Space space = MSpace::kObject) const;
    MStatus getParamAtPoint(const MPoint& at, double& param, double tolerance, MSpace::Space space = MSpace::kObject) const;

private:
    AzCurve* azCurve(MStatus* status = 0) const;
};

// What the reference takes to speed up ray casts; Aztek's are not tuned by it.
class MMeshIsectAccelParams {
public:
    MMeshIsectAccelParams() {}
};

// Over mesh data, or over a mesh shape in the scene by its node or path; world space is the shape's when it has one.
class MFnMesh : public MFnDagNode {
public:
    MFnMesh();
    MFnMesh(const MObject& o, MStatus* status = 0);
    MFnMesh(const MDagPath& path, MStatus* status = 0);
    virtual MFn::Type type() const;
    static const char* className() { return "MFnMesh"; }
    int numVertices(MStatus* status = 0) const;
    int numPolygons(MStatus* status = 0) const;
    int numEdges(MStatus* status = 0) const;
    int numFaceVertices(MStatus* status = 0) const;
    int polygonVertexCount(int polygon, MStatus* status = 0) const;
    MStatus getPolygonVertices(int polygon, MIntArray& out) const;
    MStatus getVertices(MIntArray& counts, MIntArray& connects) const;
    MStatus getPoint(int vertex, MPoint& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getPoints(MPointArray& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getPoints(MFloatPointArray& out, MSpace::Space space = MSpace::kObject) const;
    MStatus setPoint(int vertex, const MPoint& p, MSpace::Space space = MSpace::kObject);
    MStatus setPoints(MPointArray& pts, MSpace::Space space = MSpace::kObject);
    MStatus getVertexNormal(int vertex, MVector& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getVertexNormal(int vertex, bool angleWeighted, MVector& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getVertexNormals(bool angleWeighted, MFloatVectorArray& out, MSpace::Space space = MSpace::kObject) const;
    MStatus getNormals(MFloatVectorArray& out, MSpace::Space space = MSpace::kObject) const;
    // Each face as a fan from its first corner: n - 2 triangles for n corners.
    MStatus getTriangles(MIntArray& triangleCounts, MIntArray& triangleVertices) const;
    MStatus getPolygonTriangleVertices(int polygon, int triangle, int vertices[3]) const;
    MStatus getClosestPoint(const MPoint& to, MPoint& out, MSpace::Space space = MSpace::kObject, int* closestPolygon = 0) const;
    MStatus getClosestNormal(const MPoint& to, MVector& out, MSpace::Space space = MSpace::kObject, int* closestPolygon = 0) const;
    static MMeshIsectAccelParams uniformGridParams(int xDiv, int yDiv, int zDiv);
    static MMeshIsectAccelParams autoUniformGridParams();
    // The nearest place a ray meets the mesh. The ids, sorting and acceleration arguments are taken and not used: every
    // face is tried.
    bool closestIntersection(const MFloatPoint& raySource, const MFloatVector& rayDirection, const MIntArray* faceIds, const MIntArray* triIds, bool idsSorted, MSpace::Space space, float maxParam, bool testBothDirections, MMeshIsectAccelParams* accelParams, MFloatPoint& hitPoint, float* hitRayParam, int* hitFace, int* hitTriangle, float* hitBary1, float* hitBary2, float tolerance = 1.0e-6f, MStatus* status = 0);

    // The kit's own: the matrix world space adds, when the mesh is a shape's.
    bool azWorld(MMatrix& out) const;

private:
    std::shared_ptr<az::MeshData> azMesh(MStatus* status = 0) const;
    std::shared_ptr<az::MeshData> nodeMesh_;
    int64_t node_ = -1;
};
