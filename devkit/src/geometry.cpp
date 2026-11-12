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
    if (st) *st = (o.azData() && o.azData()->mesh) ? MS::kSuccess : MS::kInvalidParameter;
}
MFn::Type MFnMesh::type() const { return MFn::kMesh; }

std::shared_ptr<MeshData> MFnMesh::azMesh(MStatus* st) const {
    std::shared_ptr<MeshData> m = obj_.azData() ? obj_.azData()->mesh : std::shared_ptr<MeshData>();
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
MStatus MFnMesh::getPoint(int vertex, MPoint& out, MSpace::Space) const {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    size_t n;
    const double* p = pointsOf(m, &n);
    if (vertex < 0 || (size_t)vertex >= n) return MS::kInvalidParameter;
    out = MPoint(p[vertex * 3], p[vertex * 3 + 1], p[vertex * 3 + 2]);
    return MS::kSuccess;
}
MStatus MFnMesh::getPoints(MPointArray& out, MSpace::Space) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    size_t n;
    const double* p = pointsOf(m, &n);
    for (size_t k = 0; k < n; ++k) out.append(MPoint(p[k * 3], p[k * 3 + 1], p[k * 3 + 2]));
    return MS::kSuccess;
}
MStatus MFnMesh::getPoints(MFloatPointArray& out, MSpace::Space) const {
    std::shared_ptr<MeshData> m = azMesh();
    out.clear();
    if (!m) return MS::kFailure;
    size_t n;
    const double* p = pointsOf(m, &n);
    for (size_t k = 0; k < n; ++k) out.append(MFloatPoint((float)p[k * 3], (float)p[k * 3 + 1], (float)p[k * 3 + 2]));
    return MS::kSuccess;
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
MStatus MFnMesh::getVertexNormal(int vertex, MVector& out, MSpace::Space) const {
    std::shared_ptr<MeshData> m = azMesh();
    if (!m) return MS::kFailure;
    const AzMeshArrays& a = m->get();
    if (vertex < 0 || (size_t)vertex >= a.point_count || !a.point_normals) return MS::kInvalidParameter;
    out = MVector(a.point_normals[vertex * 3], a.point_normals[vertex * 3 + 1], a.point_normals[vertex * 3 + 2]);
    return MS::kSuccess;
}
MStatus MFnMesh::getVertexNormal(int vertex, bool, MVector& out, MSpace::Space space) const { return getVertexNormal(vertex, out, space); }
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
