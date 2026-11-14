// MFnNurbsCurve and MFnMesh over what a compute was given.
#include "internal.h"
#include <aztek/kit/geometry.h>
#include <algorithm>
#include <cmath>
#include <cstddef>

using namespace az;

namespace az {

// The host's table reaches as far as its size says: an older Aztek has no curves for plug-ins.
static bool hostHasCurves() {
    return host && host->size >= offsetof(AzHost, block_set_curve) + sizeof(host->block_set_curve);
}

CurveData::~CurveData() {
    if (handle && hostHasCurves()) host->curve_release(handle);
}

MObject curveObject(AzCurve* handle) {
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = MFn::kNurbsCurveData;
    d->curve = std::make_shared<CurveData>();
    d->curve->handle = handle;
    return MObject(d);
}

}

MFnNurbsCurveData::MFnNurbsCurveData() {}
MFnNurbsCurveData::MFnNurbsCurveData(const MObject& o, MStatus* st) { obj_ = o; if (st) *st = MS::kSuccess; }
MFn::Type MFnNurbsCurveData::type() const { return MFn::kNurbsCurveData; }

MFnNurbsCurve::MFnNurbsCurve() {}
MFnNurbsCurve::MFnNurbsCurve(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = (o.azData() && o.azData()->curve && o.azData()->curve->handle) ? MS::kSuccess : MS::kInvalidParameter;
}
MFn::Type MFnNurbsCurve::type() const { return MFn::kNurbsCurve; }

AzCurve* MFnNurbsCurve::azCurve(MStatus* st) const {
    AzCurve* c = (obj_.azData() && obj_.azData()->curve && hostHasCurves()) ? obj_.azData()->curve->handle : 0;
    if (st) *st = c ? MS::kSuccess : MS::kFailure;
    return c;
}

struct CurveInfo { int degree = 0, form = 0; size_t cvs = 0, knots = 0; double domain[2] = {0, 0}; };
static CurveInfo info(AzCurve* c) {
    CurveInfo i;
    if (c) host->curve_info(c, &i.degree, &i.form, &i.cvs, &i.knots, i.domain);
    return i;
}

int MFnNurbsCurve::numCVs(MStatus* st) const { return (int)info(azCurve(st)).cvs; }
int MFnNurbsCurve::degree(MStatus* st) const { return info(azCurve(st)).degree; }
int MFnNurbsCurve::numKnots(MStatus* st) const { return (int)info(azCurve(st)).knots; }
int MFnNurbsCurve::numSpans(MStatus* st) const { CurveInfo i = info(azCurve(st)); return (int)i.cvs - i.degree; }
MFnNurbsCurve::Form MFnNurbsCurve::form(MStatus* st) const {
    AzCurve* c = azCurve(st);
    if (!c) return kInvalid;
    int f = info(c).form;
    return f == 2 ? kPeriodic : f == 1 ? kClosed : kOpen;
}
MStatus MFnNurbsCurve::getKnotDomain(double& start, double& end) const {
    AzCurve* c = azCurve();
    if (!c) return MS::kFailure;
    CurveInfo i = info(c);
    start = i.domain[0];
    end = i.domain[1];
    return MS::kSuccess;
}
MStatus MFnNurbsCurve::getCVs(MPointArray& out, MSpace::Space) const {
    AzCurve* c = azCurve();
    out.clear();
    if (!c) return MS::kFailure;
    std::vector<double> xyz(info(c).cvs * 3);
    if (!xyz.empty()) host->curve_cvs(c, xyz.data());
    for (size_t k = 0; k + 2 < xyz.size(); k += 3) out.append(MPoint(xyz[k], xyz[k + 1], xyz[k + 2]));
    return MS::kSuccess;
}
MStatus MFnNurbsCurve::getCV(unsigned int index, MPoint& out, MSpace::Space space) const {
    MPointArray all;
    MStatus s = getCVs(all, space);
    if (!s || index >= all.length()) return MS::kInvalidParameter;
    out = all[index];
    return MS::kSuccess;
}
MStatus MFnNurbsCurve::getPointAtParam(double param, MPoint& out, MSpace::Space) const {
    AzCurve* c = azCurve();
    if (!c) return MS::kFailure;
    double p[3], t[3];
    host->curve_eval(c, param, p, t);
    out = MPoint(p[0], p[1], p[2]);
    return MS::kSuccess;
}
MVector MFnNurbsCurve::tangent(double param, MSpace::Space, MStatus* st) const {
    AzCurve* c = azCurve(st);
    if (!c) return MVector();
    double p[3], t[3];
    host->curve_eval(c, param, p, t);
    MVector v(t[0], t[1], t[2]);
    v.normalize();
    return v;
}
MVector MFnNurbsCurve::normal(double param, MSpace::Space space, MStatus* st) const {
    // The way the tangent turns, from two tangents a little apart.
    double a, b;
    if (!getKnotDomain(a, b)) { if (st) *st = MS::kFailure; return MVector(); }
    double h = (b - a) * 1.0e-4;
    MVector d = tangent(std::min(param + h, b), space) - tangent(std::max(param - h, a), space);
    MVector t = tangent(param, space, st);
    d = d - t * (d * t);
    d.normalize();
    return d;
}
double MFnNurbsCurve::length(double, MStatus* st) const { AzCurve* c = azCurve(st); return c ? host->curve_length(c) : 0.0; }
double MFnNurbsCurve::findParamFromLength(double partLength, MStatus* st) const { AzCurve* c = azCurve(st); return c ? host->curve_param_at_length(c, partLength) : 0.0; }
double MFnNurbsCurve::findParamFromLength(double partLength, double, MStatus* st) const { return findParamFromLength(partLength, st); }
double MFnNurbsCurve::findLengthFromParam(double param, MStatus* st) const { AzCurve* c = azCurve(st); return c ? host->curve_length_at_param(c, param) : 0.0; }
MPoint MFnNurbsCurve::closestPoint(const MPoint& to, double* param, double, MSpace::Space space, MStatus* st) const {
    AzCurve* c = azCurve(st);
    if (!c) return MPoint();
    double q[3] = {to.x, to.y, to.z};
    double u = host->curve_closest_param(c, q);
    if (param) *param = u;
    MPoint out;
    getPointAtParam(u, out, space);
    return out;
}
MStatus MFnNurbsCurve::getParamAtPoint(const MPoint& at, double& param, MSpace::Space space) const {
    MStatus s;
    closestPoint(at, &param, 1.0e-3, space, &s);
    return s;
}
MStatus MFnNurbsCurve::getParamAtPoint(const MPoint& at, double& param, double, MSpace::Space space) const { return getParamAtPoint(at, param, space); }

// ---------------------------------------------------------------------------------------------------------------------

MFnMesh::MFnMesh() {}
MFnMesh::MFnMesh(const MObject& o, MStatus* st) {
    obj_ = o;
    if (o.azData() && o.azData()->mesh) {
        if (st) *st = MS::kSuccess;
        return;
    }
    // A mesh shape in the scene.
    int64_t id = nodeOf(o);
    if (id >= 0 && nodeHasFn(id, MFn::kMesh)) {
        MStatus s;
        nodeMesh_ = meshOfNode(id, &s);
        node_ = id;
        path_ = MDagPath::azOf(id);
        if (st) *st = s;
        return;
    }
    if (st) *st = MS::kInvalidParameter;
}
MFnMesh::MFnMesh(const MDagPath& path, MStatus* st) {
    MDagPath shape = path;
    if (!nodeHasFn(shape.azNode(), MFn::kMesh)) shape.extendToShape();
    *this = MFnMesh(shape.node(), st);
}
bool MFnMesh::azWorld(MMatrix& out) const {
    if (node_ < 0) return false;
    out = MDagPath::azOf(node_).inclusiveMatrix();
    return true;
}
MFn::Type MFnMesh::type() const { return MFn::kMesh; }

std::shared_ptr<MeshData> MFnMesh::azMesh(MStatus* st) const {
    std::shared_ptr<MeshData> m = nodeMesh_ ? nodeMesh_ : obj_.azData() ? obj_.azData()->mesh : std::shared_ptr<MeshData>();
    if (st) *st = m ? MS::kSuccess : MS::kFailure;
    return m;
}

int MFnMesh::numVertices(MStatus* st) const { std::shared_ptr<MeshData> m = azMesh(st); return m ? (int)m->get().point_count : 0; }
int MFnMesh::numPolygons(MStatus* st) const { std::shared_ptr<MeshData> m = azMesh(st); return m ? (int)m->get().face_count : 0; }
int MFnMesh::numEdges(MStatus* st) const { std::shared_ptr<MeshData> m = azMesh(st); return m ? (int)m->get().edge_count : 0; }
int MFnMesh::numFaceVertices(MStatus* st) const { std::shared_ptr<MeshData> m = azMesh(st); return m ? (int)m->get().corner_count : 0; }
int MFnMesh::polygonVertexCount(int polygon, MStatus* st) const {
    std::shared_ptr<MeshData> m = azMesh(st);
    if (!m || polygon < 0 || (size_t)polygon >= m->get().face_count) return 0;
    return (int)m->get().counts[polygon];
}
MStatus MFnMesh::getVertices(MIntArray& counts, MIntArray& connects) const {
    std::shared_ptr<MeshData> m = azMesh();
    counts.clear();
    connects.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    for (size_t f = 0; f < a.face_count; ++f) counts.append((int)a.counts[f]);
    for (size_t c = 0; c < a.corner_count; ++c) connects.append((int)a.connects[c]);
    return MS::kSuccess;
}
MStatus MFnMesh::getPolygonVertices(int polygon, MIntArray& out) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (polygon < 0 || (size_t)polygon >= a.face_count) return MS::kInvalidParameter;
    size_t first = 0;
    for (int f = 0; f < polygon; ++f) first += (size_t)a.counts[f];
    for (int64_t k = 0; k < a.counts[polygon]; ++k) out.append((int)a.connects[first + k]);
    return MS::kSuccess;
}
static const double* pointsOf(const std::shared_ptr<MeshData>& m, size_t* count) {
    if (m->pointsEdited) { *count = m->points.size() / 3; return m->points.data(); }
    const AzMeshArrays& a = m->get();
    *count = a.point_count;
    return a.points;
}
// The matrix a space asks for: the shape's world matrix for world space, when the mesh is a shape's.
static bool spaceMatrix(const MFnMesh& fn, MSpace::Space space, MMatrix& m) {
    return (space == MSpace::kWorld || space == MSpace::kPostTransform) && fn.azWorld(m);
}
MStatus MFnMesh::getPoint(int vertex, MPoint& out, MSpace::Space space) const {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    size_t n;
    const double* p = pointsOf(m, &n);
    if (vertex < 0 || (size_t)vertex >= n) return MS::kInvalidParameter;
    out = MPoint(p[vertex * 3], p[vertex * 3 + 1], p[vertex * 3 + 2]);
    MMatrix w;
    if (spaceMatrix(*this, space, w)) out = out * w;
    return MS::kSuccess;
}
MStatus MFnMesh::getPoints(MPointArray& out, MSpace::Space space) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    size_t n;
    const double* p = pointsOf(m, &n);
    MMatrix w;
    bool world = spaceMatrix(*this, space, w);
    for (size_t k = 0; k < n; ++k) {
        MPoint q(p[k * 3], p[k * 3 + 1], p[k * 3 + 2]);
        out.append(world ? q * w : q);
    }
    return MS::kSuccess;
}
MStatus MFnMesh::getPoints(MFloatPointArray& out, MSpace::Space space) const {
    MPointArray pts;
    out.clear();
    MStatus st = getPoints(pts, space);
    for (unsigned int k = 0; k < pts.length(); ++k) out.append(MFloatPoint((float)pts[k].x, (float)pts[k].y, (float)pts[k].z));
    return st;
}
MStatus MFnMesh::setPoint(int vertex, const MPoint& p, MSpace::Space) {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    m->loadPoints();
    if (vertex < 0 || (size_t)vertex * 3 + 2 >= m->points.size()) return MS::kInvalidParameter;
    m->points[vertex * 3] = p.x;
    m->points[vertex * 3 + 1] = p.y;
    m->points[vertex * 3 + 2] = p.z;
    m->pointsEdited = true;
    return MS::kSuccess;
}
MStatus MFnMesh::setPoints(MPointArray& pts, MSpace::Space) {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    m->loadPoints();
    if ((size_t)pts.length() * 3 != m->points.size()) return MS::kInvalidParameter;
    for (unsigned int k = 0; k < pts.length(); ++k) {
        m->points[k * 3] = pts[k].x;
        m->points[k * 3 + 1] = pts[k].y;
        m->points[k * 3 + 2] = pts[k].z;
    }
    m->pointsEdited = true;
    return MS::kSuccess;
}
// A normal into world space: through the inverse transpose, so a scaled shape keeps its normals square to its faces.
static MVector worldNormal(const MVector& n, const MMatrix& w) {
    MMatrix it = w.inverse().transpose();
    MVector r(n.x * it(0, 0) + n.y * it(1, 0) + n.z * it(2, 0), n.x * it(0, 1) + n.y * it(1, 1) + n.z * it(2, 1), n.x * it(0, 2) + n.y * it(1, 2) + n.z * it(2, 2));
    return r.normal();
}
MStatus MFnMesh::getVertexNormal(int vertex, MVector& out, MSpace::Space space) const {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (vertex < 0 || (size_t)vertex >= a.point_count || !a.point_normals) return MS::kInvalidParameter;
    out = MVector(a.point_normals[vertex * 3], a.point_normals[vertex * 3 + 1], a.point_normals[vertex * 3 + 2]);
    MMatrix w;
    if (spaceMatrix(*this, space, w)) out = worldNormal(out, w);
    return MS::kSuccess;
}
MStatus MFnMesh::getVertexNormal(int vertex, bool, MVector& out, MSpace::Space space) const { return getVertexNormal(vertex, out, space); }
MStatus MFnMesh::getVertexNormals(bool, MFloatVectorArray& out, MSpace::Space space) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (!a.point_normals) return MS::kFailure;
    MMatrix w;
    bool world = spaceMatrix(*this, space, w);
    for (size_t k = 0; k < a.point_count; ++k) {
        MVector n(a.point_normals[k * 3], a.point_normals[k * 3 + 1], a.point_normals[k * 3 + 2]);
        if (world) n = worldNormal(n, w);
        out.append(MFloatVector((float)n.x, (float)n.y, (float)n.z));
    }
    return MS::kSuccess;
}
MStatus MFnMesh::getNormals(MFloatVectorArray& out, MSpace::Space space) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (!a.corner_normals) return MS::kFailure;
    MMatrix w;
    bool world = spaceMatrix(*this, space, w);
    for (size_t k = 0; k < a.corner_count; ++k) {
        MVector n(a.corner_normals[k * 3], a.corner_normals[k * 3 + 1], a.corner_normals[k * 3 + 2]);
        if (world) n = worldNormal(n, w);
        out.append(MFloatVector((float)n.x, (float)n.y, (float)n.z));
    }
    return MS::kSuccess;
}
MStatus MFnMesh::getTriangles(MIntArray& counts, MIntArray& verts) const {
    std::shared_ptr<MeshData> m = azMesh();
    counts.clear();
    verts.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    size_t c = 0;
    for (size_t f = 0; f < a.face_count; ++f) {
        int n = (int)a.counts[f];
        counts.append(n >= 3 ? n - 2 : 0);
        for (int t = 1; t + 1 < n; ++t) {
            verts.append((int)a.connects[c]);
            verts.append((int)a.connects[c + t]);
            verts.append((int)a.connects[c + t + 1]);
        }
        c += (size_t)n;
    }
    return MS::kSuccess;
}
MStatus MFnMesh::getPolygonTriangleVertices(int polygon, int triangle, int v[3]) const {
    MIntArray corners;
    MStatus st = getPolygonVertices(polygon, corners);
    if (!st) return st;
    if (triangle < 0 || triangle + 2 >= (int)corners.length()) return MS::kInvalidParameter;
    v[0] = corners[0];
    v[1] = corners[triangle + 1];
    v[2] = corners[triangle + 2];
    return MS::kSuccess;
}
MMeshIsectAccelParams MFnMesh::uniformGridParams(int, int, int) { return MMeshIsectAccelParams(); }
MMeshIsectAccelParams MFnMesh::autoUniformGridParams() { return MMeshIsectAccelParams(); }

bool MFnMesh::closestIntersection(const MFloatPoint& src, const MFloatVector& dirIn, const MIntArray*, const MIntArray*, bool, MSpace::Space, float maxParam, bool both, MMeshIsectAccelParams*, MFloatPoint& hitPoint, float* hitRayParam, int* hitFace, int* hitTriangle, float* hitBary1, float* hitBary2, float tolerance, MStatus* st) {
    std::shared_ptr<MeshData> m = azMesh(st);
    if (!m) return false;
    size_t n;
    const double* p = pointsOf(m, &n);
    const AzMeshArrays& a = m->get();
    double len = std::sqrt((double)dirIn.x * dirIn.x + (double)dirIn.y * dirIn.y + (double)dirIn.z * dirIn.z);
    if (len < 1.0e-12) return false;
    const double o[3] = {src.x, src.y, src.z};
    const double d[3] = {dirIn.x / len, dirIn.y / len, dirIn.z / len};
    double best = 1.0e300;
    int bestFace = -1, bestTri = -1;
    double bestU = 0, bestV = 0;
    size_t first = 0;
    for (size_t f = 0; f < a.face_count; ++f) {
        size_t count = (size_t)a.counts[f];
        // A fan from the face's first corner.
        for (size_t k = 1; k + 1 < count; ++k) {
            const int64_t ia = a.connects[first], ib = a.connects[first + k], ic = a.connects[first + k + 1];
            if ((size_t)ia >= n || (size_t)ib >= n || (size_t)ic >= n) continue;
            const double* A = p + ia * 3;
            const double* B = p + ib * 3;
            const double* C = p + ic * 3;
            const double e1[3] = {B[0] - A[0], B[1] - A[1], B[2] - A[2]};
            const double e2[3] = {C[0] - A[0], C[1] - A[1], C[2] - A[2]};
            const double pv[3] = {d[1] * e2[2] - d[2] * e2[1], d[2] * e2[0] - d[0] * e2[2], d[0] * e2[1] - d[1] * e2[0]};
            const double det = e1[0] * pv[0] + e1[1] * pv[1] + e1[2] * pv[2];
            if (std::fabs(det) < 1.0e-14) continue;
            const double inv = 1.0 / det;
            const double tv[3] = {o[0] - A[0], o[1] - A[1], o[2] - A[2]};
            const double u = (tv[0] * pv[0] + tv[1] * pv[1] + tv[2] * pv[2]) * inv;
            if (u < -tolerance || u > 1.0 + tolerance) continue;
            const double qv[3] = {tv[1] * e1[2] - tv[2] * e1[1], tv[2] * e1[0] - tv[0] * e1[2], tv[0] * e1[1] - tv[1] * e1[0]};
            const double v = (d[0] * qv[0] + d[1] * qv[1] + d[2] * qv[2]) * inv;
            if (v < -tolerance || u + v > 1.0 + tolerance) continue;
            const double t = (e2[0] * qv[0] + e2[1] * qv[1] + e2[2] * qv[2]) * inv;
            if (!both && t < 0.0) continue;
            if (std::fabs(t) > maxParam) continue;
            if (std::fabs(t) < std::fabs(best)) {
                best = t;
                bestFace = (int)f;
                bestTri = (int)(k - 1);
                bestU = u;
                bestV = v;
            }
        }
        first += count;
    }
    if (bestFace < 0) return false;
    hitPoint = MFloatPoint((float)(o[0] + d[0] * best), (float)(o[1] + d[1] * best), (float)(o[2] + d[2] * best));
    if (hitRayParam) *hitRayParam = (float)best;
    if (hitFace) *hitFace = bestFace;
    if (hitTriangle) *hitTriangle = bestTri;
    if (hitBary1) *hitBary1 = (float)(1.0 - bestU - bestV);
    if (hitBary2) *hitBary2 = (float)bestU;
    return true;
}

// ---------------------------------------------------------------------------------------------------------------------
// MFnMesh: edges, face normals and UVs

MStatus MFnMesh::getEdgeVertices(int edge, int2& out) const {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (edge < 0 || (size_t)edge >= a.edge_count) return MS::kInvalidParameter;
    out[0] = (int)a.edges[edge * 2];
    out[1] = (int)a.edges[edge * 2 + 1];
    return MS::kSuccess;
}
MStatus MFnMesh::azCornerEdges(MIntArray& out) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    for (size_t c = 0; c < a.corner_count; ++c) out.append(a.corner_edges ? (int)a.corner_edges[c] : -1);
    return MS::kSuccess;
}
// Newell's normal: right for any face, flat or not.
MStatus MFnMesh::getPolygonNormal(int polygon, MVector& out, MSpace::Space space) const {
    MIntArray vs;
    MStatus st = getPolygonVertices(polygon, vs);
    if (!st) return st;
    if (vs.length() < 3) return MS::kFailure;
    MVector n(0, 0, 0);
    for (unsigned int i = 0; i < vs.length(); ++i) {
        MPoint p, q;
        getPoint(vs[i], p, space);
        getPoint(vs[(i + 1) % vs.length()], q, space);
        n.x += (p.y - q.y) * (p.z + q.z);
        n.y += (p.z - q.z) * (p.x + q.x);
        n.z += (p.x - q.x) * (p.y + q.y);
    }
    out = n.normal();
    return MS::kSuccess;
}
int MFnMesh::numUVs(MStatus* st) const {
    std::shared_ptr<MeshData> m = azMesh(st);
    return m && m->get().has_uvs ? (int)m->get().map_count : 0;
}
int MFnMesh::numUVs(const MString&, MStatus* st) const { return numUVs(st); }
MStatus MFnMesh::getUVs(MFloatArray& u, MFloatArray& v, const MString*) const {
    std::shared_ptr<MeshData> m = azMesh();
    u.clear();
    v.clear();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (!a.has_uvs) return MS::kSuccess;
    for (size_t k = 0; k < a.map_count; ++k) {
        u.append((float)a.map_uvs[k * 2]);
        v.append((float)a.map_uvs[k * 2 + 1]);
    }
    return MS::kSuccess;
}
MStatus MFnMesh::getPolygonUVid(int polygon, int corner, int& uvId, const MString*) const {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (!a.has_uvs || polygon < 0 || (size_t)polygon >= a.face_count) return MS::kFailure;
    size_t start = 0;
    for (int f = 0; f < polygon; ++f) start += (size_t)a.counts[f];
    if (corner < 0 || corner >= (int)a.counts[polygon]) return MS::kInvalidParameter;
    uvId = (int)a.corner_maps[start + corner];
    return uvId >= 0 ? MS::kSuccess : MS::kFailure;
}
MStatus MFnMesh::getPolygonUV(int polygon, int corner, float& u, float& v, const MString* set) const {
    int id = -1;
    MStatus st = getPolygonUVid(polygon, corner, id, set);
    if (!st) return st;
    const AzMeshArrays& a = azMesh()->get();
    u = (float)a.map_uvs[id * 2];
    v = (float)a.map_uvs[id * 2 + 1];
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// MFnNurbsSurface: a shape in the scene, through the host's surface queries

namespace {
Json surfaceData(int64_t shape, MStatus* st) {
    Json a = Json::object();
    a.set("id", shape);
    return query("surface.data", a, st);
}
Json surfaceEval(int64_t shape, const char* what, double u, double v, MStatus* st) {
    Json a = Json::object();
    a.set("id", shape);
    a.set("what", what);
    a.set("u", u);
    a.set("v", v);
    return query("surface.eval", a, st);
}
MPoint pointOf(const Json& j) { return MPoint(j[(size_t)0].num(), j[(size_t)1].num(), j[(size_t)2].num()); }
MVector vectorOf(const Json& j) { return MVector(j[(size_t)0].num(), j[(size_t)1].num(), j[(size_t)2].num()); }
MFnNurbsSurface::Form formOf(int64_t f) { return f == 2 ? MFnNurbsSurface::kPeriodic : f == 1 ? MFnNurbsSurface::kClosed : MFnNurbsSurface::kOpen; }
}  // namespace

MFnNurbsSurface::MFnNurbsSurface() {}
MFnNurbsSurface::MFnNurbsSurface(const MObject& o, MStatus* st) : MFnDagNode(o, st) {
    if (st && *st && !o.hasFn(MFn::kNurbsSurface)) *st = MS::kInvalidParameter;
}
MFnNurbsSurface::MFnNurbsSurface(const MDagPath& path, MStatus* st) {
    MDagPath shape = path;
    if (!shape.hasFn(MFn::kNurbsSurface)) shape.extendToShape();
    *this = MFnNurbsSurface(shape.node(), st);
}
MFn::Type MFnNurbsSurface::type() const { return MFn::kNurbsSurface; }

int64_t MFnNurbsSurface::azShape(MStatus* st) const {
    int64_t id = nodeOf(obj_);
    if (st) *st = id >= 0 ? MS::kSuccess : MS::kInvalidParameter;
    return id;
}

namespace {
// The matrix world space adds.
bool surfaceWorld(int64_t shape, MSpace::Space space, MMatrix& w) {
    if (space != MSpace::kWorld) return false;
    w = MDagPath::azOf(shape).inclusiveMatrix();
    return true;
}
}  // namespace

int MFnNurbsSurface::numCVsInU(MStatus* st) const { return (int)surfaceData(azShape(st), st)["numU"].integer(); }
int MFnNurbsSurface::numCVsInV(MStatus* st) const { return (int)surfaceData(azShape(st), st)["numV"].integer(); }
int MFnNurbsSurface::numSpansInU(MStatus* st) const { return (int)surfaceData(azShape(st), st)["spansU"].integer(); }
int MFnNurbsSurface::numSpansInV(MStatus* st) const { return (int)surfaceData(azShape(st), st)["spansV"].integer(); }
int MFnNurbsSurface::degreeU(MStatus* st) const { return (int)surfaceData(azShape(st), st)["degreeU"].integer(); }
int MFnNurbsSurface::degreeV(MStatus* st) const { return (int)surfaceData(azShape(st), st)["degreeV"].integer(); }
MFnNurbsSurface::Form MFnNurbsSurface::formInU(MStatus* st) const { return formOf(surfaceData(azShape(st), st)["formU"].integer()); }
MFnNurbsSurface::Form MFnNurbsSurface::formInV(MStatus* st) const { return formOf(surfaceData(azShape(st), st)["formV"].integer()); }
int MFnNurbsSurface::numKnotsInU(MStatus* st) const { return (int)surfaceData(azShape(st), st)["knotsU"].size(); }
int MFnNurbsSurface::numKnotsInV(MStatus* st) const { return (int)surfaceData(azShape(st), st)["knotsV"].size(); }
MStatus MFnNurbsSurface::getKnotsInU(MDoubleArray& out) const {
    MStatus st;
    std::vector<double> k = numbers(surfaceData(azShape(&st), &st)["knotsU"]);
    out.clear();
    for (size_t i = 0; i < k.size(); ++i) out.append(k[i]);
    return st;
}
MStatus MFnNurbsSurface::getKnotsInV(MDoubleArray& out) const {
    MStatus st;
    std::vector<double> k = numbers(surfaceData(azShape(&st), &st)["knotsV"]);
    out.clear();
    for (size_t i = 0; i < k.size(); ++i) out.append(k[i]);
    return st;
}
MStatus MFnNurbsSurface::getKnotDomain(double& u0, double& u1, double& v0, double& v1) const {
    MStatus st;
    std::vector<double> d = numbers(surfaceData(azShape(&st), &st)["domain"]);
    if (!st || d.size() < 4) return st ? MStatus(MS::kFailure) : st;
    u0 = d[0];
    u1 = d[1];
    v0 = d[2];
    v1 = d[3];
    return MS::kSuccess;
}
bool MFnNurbsSurface::isTrimmedSurface(MStatus* st) const { return surfaceData(azShape(st), st)["trimmed"].truth(); }
MStatus MFnNurbsSurface::getCVs(MPointArray& out, MSpace::Space space) const {
    MStatus st;
    int64_t shape = azShape(&st);
    std::vector<double> c = numbers(surfaceData(shape, &st)["cvs"]);
    out.clear();
    if (!st) return st;
    MMatrix w;
    bool world = surfaceWorld(shape, space, w);
    for (size_t i = 0; i + 2 < c.size(); i += 3) {
        MPoint p(c[i], c[i + 1], c[i + 2]);
        out.append(world ? p * w : p);
    }
    return MS::kSuccess;
}
MStatus MFnNurbsSurface::getCV(unsigned int u, unsigned int v, MPoint& out, MSpace::Space space) const {
    MStatus st;
    int64_t shape = azShape(&st);
    Json d = surfaceData(shape, &st);
    if (!st) return st;
    unsigned int nu = (unsigned int)d["numU"].integer(), nv = (unsigned int)d["numV"].integer();
    if (u >= nu || v >= nv) return MS::kInvalidParameter;
    const Json& c = d["cvs"];
    size_t k = ((size_t)u * nv + v) * 3;
    out = MPoint(c[k].num(), c[k + 1].num(), c[k + 2].num());
    MMatrix w;
    if (surfaceWorld(shape, space, w)) out = out * w;
    return MS::kSuccess;
}
MStatus MFnNurbsSurface::getPointAtParam(double u, double v, MPoint& out, MSpace::Space space) const {
    MStatus st;
    int64_t shape = azShape(&st);
    Json p = surfaceEval(shape, "point", u, v, &st);
    if (!st) return st;
    out = pointOf(p);
    MMatrix w;
    if (surfaceWorld(shape, space, w)) out = out * w;
    return MS::kSuccess;
}
MVector MFnNurbsSurface::normal(double u, double v, MSpace::Space space, MStatus* st) const {
    int64_t shape = azShape(st);
    MVector n = vectorOf(surfaceEval(shape, "normal", u, v, st));
    MMatrix w;
    if (surfaceWorld(shape, space, w)) n = (n * w.inverse().transpose()).normal();
    return n;
}
MStatus MFnNurbsSurface::getDerivativesAtParm(double u, double v, MPoint& point, MVector& du, MVector& dv, MSpace::Space space, MVector* duu, MVector* dvv, MVector* duv) const {
    MStatus st;
    int64_t shape = azShape(&st);
    Json d = surfaceEval(shape, "derivatives", u, v, &st);
    if (!st) return st;
    point = pointOf(d["point"]);
    du = vectorOf(d["du"]);
    dv = vectorOf(d["dv"]);
    MMatrix w;
    if (surfaceWorld(shape, space, w)) {
        point = point * w;
        du = du * w;
        dv = dv * w;
    }
    // Second derivatives aren't given here.
    if (duu) *duu = MVector(0, 0, 0);
    if (dvv) *dvv = MVector(0, 0, 0);
    if (duv) *duv = MVector(0, 0, 0);
    return MS::kSuccess;
}
MStatus MFnNurbsSurface::getTangents(double u, double v, MVector& tu, MVector& tv, MSpace::Space space) const {
    MPoint p;
    MStatus st = getDerivativesAtParm(u, v, p, tu, tv, space);
    if (!st) return st;
    tu = tu.normal();
    tv = tv.normal();
    return MS::kSuccess;
}
MPoint MFnNurbsSurface::closestPoint(const MPoint& to, double* u, double* v, bool, double, MSpace::Space space, MStatus* st) const {
    int64_t shape = azShape(st);
    MMatrix w;
    bool world = surfaceWorld(shape, space, w);
    MPoint local = world ? to * w.inverse() : to;
    Json a = Json::object();
    a.set("id", shape);
    a.set("what", "closest");
    Json p = Json::array();
    p.push(local.x);
    p.push(local.y);
    p.push(local.z);
    a.set("point", p);
    Json r = query("surface.eval", a, st);
    if (u) *u = r["u"].num();
    if (v) *v = r["v"].num();
    MPoint out = pointOf(r["point"]);
    return world ? out * w : out;
}
MStatus MFnNurbsSurface::getParamAtPoint(const MPoint& at, double& u, double& v, MSpace::Space space) {
    return getParamAtPoint(at, u, v, false, space);
}
MStatus MFnNurbsSurface::getParamAtPoint(const MPoint& at, double& u, double& v, bool ignoreTrim, MSpace::Space space, double tolerance) {
    MStatus st;
    MPoint p = closestPoint(at, &u, &v, ignoreTrim, tolerance, space, &st);
    if (!st) return st;
    return p.distanceTo(at) <= std::max(tolerance, 1.0e-6) ? MStatus(MS::kSuccess) : MStatus(MS::kFailure);
}
bool MFnNurbsSurface::isPointOnSurface(const MPoint& at, double tolerance, MSpace::Space space, MStatus* st) {
    double u, v;
    MPoint p = closestPoint(at, &u, &v, false, tolerance, space, st);
    return p.distanceTo(at) <= tolerance;
}
// In the surface's own space, from a tessellation: close, not exact.
double MFnNurbsSurface::area(MSpace::Space, double, MStatus* st) const {
    return surfaceEval(azShape(st), "area", 0, 0, st).num();
}
