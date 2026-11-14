// Aztek plug-in kit: iterators, component function sets, closest points, the thread pool and deformer function sets.
#include "internal.h"
#include <aztek/kit/iterators.h>
#include <algorithm>
#include <cmath>
#include <set>

using namespace az;

// ---------------------------------------------------------------------------------------------------------------------
// Thread pool

MStatus MThreadPool::init() { return MS::kSuccess; }
void MThreadPool::release() {}
MStatus MThreadPool::newParallelRegion(MThreadCallbackFunc func, void* data) {
    MThreadRootTask root;
    func(data, &root);
    return executeAndJoin(&root);
}
MStatus MThreadPool::newParallelRegionWithSpinLock(MThreadCallbackFunc func, void* data) { return newParallelRegion(func, data); }
MStatus MThreadPool::createTask(MThreadFunc func, void* data, MThreadRootTask* root) {
    if (!root || !func) return MS::kInvalidParameter;
    root->azTasks.push_back(std::make_pair(func, data));
    return MS::kSuccess;
}
MStatus MThreadPool::executeAndJoin(MThreadRootTask* root) {
    if (!root) return MS::kInvalidParameter;
    std::vector<std::pair<MThreadFunc, void*> > tasks;
    tasks.swap(root->azTasks);
    for (size_t i = 0; i < tasks.size(); ++i) tasks[i].first(tasks[i].second);
    return MS::kSuccess;
}
MStatus MThreadPool::executeAndRelease(MThreadRootTask* root) { return executeAndJoin(root); }
int MThreadUtils::getNumThreads() { return 1; }
void MThreadUtils::setNumThreads(int) {}

// ---------------------------------------------------------------------------------------------------------------------
// Components

MFnComponent::MFnComponent() {}
MFnComponent::MFnComponent(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = o.azData() && !o.azData()->str.empty() ? MS::kSuccess : MS::kInvalidParameter;
}
MFn::Type MFnComponent::type() const { return MFn::kComponent; }
int MFnComponent::elementCount(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return obj_.azData() ? (int)obj_.azData()->ids.size() : 0;
}
MFn::Type MFnComponent::componentType(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return obj_.azData() ? obj_.azData()->type : MFn::kInvalid;
}
bool MFnComponent::isEmpty(MStatus* st) const { return elementCount(st) == 0 && !isComplete(); }
bool MFnComponent::isComplete(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return obj_.azData() && obj_.azData()->complete;
}
MStatus MFnComponent::setComplete(bool on) {
    if (!obj_.azData()) return MS::kInvalidParameter;
    obj_.azData()->complete = on;
    return MS::kSuccess;
}
bool MFnComponent::isEqual(const MObject& other, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    const std::shared_ptr<ObjectData>& a = obj_.azData();
    const std::shared_ptr<ObjectData>& b = other.azData();
    if (!a || !b) return a == b;
    std::set<int> x(a->ids.begin(), a->ids.end()), y(b->ids.begin(), b->ids.end());
    return a->type == b->type && a->complete == b->complete && x == y;
}

MFnSingleIndexedComponent::MFnSingleIndexedComponent() {}
MFnSingleIndexedComponent::MFnSingleIndexedComponent(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = o.azData() ? MS::kSuccess : MS::kInvalidParameter;
}
MFn::Type MFnSingleIndexedComponent::type() const { return MFn::kSingleIndexedComponent; }
MObject MFnSingleIndexedComponent::create(MFn::Type t, MStatus* st) {
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = t;
    obj_ = MObject(d);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MStatus MFnSingleIndexedComponent::addElement(int e) {
    if (!obj_.azData()) return MS::kInvalidParameter;
    obj_.azData()->ids.push_back(e);
    return MS::kSuccess;
}
MStatus MFnSingleIndexedComponent::addElements(MIntArray& es) {
    if (!obj_.azData()) return MS::kInvalidParameter;
    for (unsigned int i = 0; i < es.length(); ++i) obj_.azData()->ids.push_back(es[i]);
    return MS::kSuccess;
}
int MFnSingleIndexedComponent::element(int i, MStatus* st) const {
    const std::shared_ptr<ObjectData>& d = obj_.azData();
    if (!d || i < 0 || (size_t)i >= d->ids.size()) {
        if (st) *st = MS::kInvalidParameter;
        return -1;
    }
    if (st) *st = MS::kSuccess;
    return d->ids[i];
}
MStatus MFnSingleIndexedComponent::getElements(MIntArray& out) const {
    out.clear();
    const std::shared_ptr<ObjectData>& d = obj_.azData();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->ids.size(); ++i) out.append(d->ids[i]);
    return MS::kSuccess;
}
MStatus MFnSingleIndexedComponent::setCompleteData(int n) {
    const std::shared_ptr<ObjectData>& d = obj_.azData();
    if (!d) return MS::kInvalidParameter;
    d->complete = true;
    d->ids.clear();
    for (int i = 0; i < n; ++i) d->ids.push_back(i);
    return MS::kSuccess;
}
MStatus MFnSingleIndexedComponent::getCompleteData(int& n) const {
    const std::shared_ptr<ObjectData>& d = obj_.azData();
    n = d && d->complete ? (int)d->ids.size() : 0;
    return d ? MS::kSuccess : MS::kInvalidParameter;
}

// ---------------------------------------------------------------------------------------------------------------------
// Closest points

MPointOnMesh::MPointOnMesh() : azFace(-1), azTriangle(-1), azU(0), azV(0) {}
const MFloatPoint& MPointOnMesh::getPoint() const { return azPoint; }
const MFloatVector& MPointOnMesh::getNormal() const { return azNormal; }
int MPointOnMesh::faceIndex() const { return azFace; }
int MPointOnMesh::triangleIndex() const { return azTriangle; }
void MPointOnMesh::getBarycentricCoords(float& u, float& v) const {
    u = azU;
    v = azV;
}

MMeshIntersector::MMeshIntersector() : created_(false) {}
MMeshIntersector::~MMeshIntersector() {}
bool MMeshIntersector::isCreated() { return created_; }

MStatus MMeshIntersector::create(MObject& mesh, const MMatrix& matrix) {
    created_ = false;
    MStatus st;
    MFnMesh fn(mesh, &st);
    if (!st) return st;
    MPointArray pts;
    fn.getPoints(pts);
    points_.clear();
    for (unsigned int i = 0; i < pts.length(); ++i) {
        points_.push_back(pts[i].x);
        points_.push_back(pts[i].y);
        points_.push_back(pts[i].z);
    }
    MIntArray counts, verts;
    fn.getTriangles(counts, verts);
    tris_.assign(verts.length(), 0);
    for (unsigned int i = 0; i < verts.length(); ++i) tris_[i] = verts[i];
    triFace_.clear();
    triIndex_.clear();
    for (unsigned int f = 0; f < counts.length(); ++f)
        for (int t = 0; t < counts[f]; ++t) {
            triFace_.push_back((int)f);
            triIndex_.push_back(t);
        }
    inverse_ = matrix.inverse();
    created_ = true;
    return MS::kSuccess;
}

// The point of triangle abc nearest p, with its weights on a, b and c (Ericson, Real-Time Collision Detection 5.1.5).
static void closestOnTriangle(const double p[3], const double a[3], const double b[3], const double c[3], double out[3], double w[3]) {
    double ab[3], ac[3], ap[3];
    for (int k = 0; k < 3; ++k) {
        ab[k] = b[k] - a[k];
        ac[k] = c[k] - a[k];
        ap[k] = p[k] - a[k];
    }
    double d1 = ab[0] * ap[0] + ab[1] * ap[1] + ab[2] * ap[2];
    double d2 = ac[0] * ap[0] + ac[1] * ap[1] + ac[2] * ap[2];
    double bw = 0, cw = 0;
    if (d1 <= 0 && d2 <= 0) {
    } else {
        double bp[3], cp[3];
        for (int k = 0; k < 3; ++k) {
            bp[k] = p[k] - b[k];
            cp[k] = p[k] - c[k];
        }
        double d3 = ab[0] * bp[0] + ab[1] * bp[1] + ab[2] * bp[2];
        double d4 = ac[0] * bp[0] + ac[1] * bp[1] + ac[2] * bp[2];
        double d5 = ab[0] * cp[0] + ab[1] * cp[1] + ab[2] * cp[2];
        double d6 = ac[0] * cp[0] + ac[1] * cp[1] + ac[2] * cp[2];
        double vc = d1 * d4 - d3 * d2, vb = d5 * d2 - d1 * d6, va = d3 * d6 - d5 * d4;
        if (d3 >= 0 && d4 <= d3) {
            bw = 1;
        } else if (vc <= 0 && d1 >= 0 && d3 <= 0) {
            bw = d1 / (d1 - d3);
        } else if (d6 >= 0 && d5 <= d6) {
            cw = 1;
        } else if (vb <= 0 && d2 >= 0 && d6 <= 0) {
            cw = d2 / (d2 - d6);
        } else if (va <= 0 && (d4 - d3) >= 0 && (d5 - d6) >= 0) {
            cw = (d4 - d3) / ((d4 - d3) + (d5 - d6));
            bw = 1 - cw;
        } else {
            double denom = 1.0 / (va + vb + vc);
            bw = vb * denom;
            cw = vc * denom;
        }
    }
    w[0] = 1 - bw - cw;
    w[1] = bw;
    w[2] = cw;
    for (int k = 0; k < 3; ++k) out[k] = a[k] + ab[k] * bw + ac[k] * cw;
}

MStatus MMeshIntersector::getClosestPoint(const MPoint& point, MPointOnMesh& out, double maxDistance) {
    if (!created_) return MS::kFailure;
    MPoint local = point * inverse_;
    const double p[3] = {local.x, local.y, local.z};
    double best = maxDistance * maxDistance;
    int found = -1;
    double bestPoint[3] = {0, 0, 0}, bestW[3] = {0, 0, 0};
    for (size_t t = 0; t * 3 + 2 < tris_.size(); ++t) {
        const double* a = &points_[tris_[t * 3] * 3];
        const double* b = &points_[tris_[t * 3 + 1] * 3];
        const double* c = &points_[tris_[t * 3 + 2] * 3];
        double q[3], w[3];
        closestOnTriangle(p, a, b, c, q, w);
        double d = (q[0] - p[0]) * (q[0] - p[0]) + (q[1] - p[1]) * (q[1] - p[1]) + (q[2] - p[2]) * (q[2] - p[2]);
        if (d < best) {
            best = d;
            found = (int)t;
            for (int k = 0; k < 3; ++k) {
                bestPoint[k] = q[k];
                bestW[k] = w[k];
            }
        }
    }
    if (found < 0) return MS::kFailure;
    const double* a = &points_[tris_[found * 3] * 3];
    const double* b = &points_[tris_[found * 3 + 1] * 3];
    const double* c = &points_[tris_[found * 3 + 2] * 3];
    MVector n = (MVector(b[0] - a[0], b[1] - a[1], b[2] - a[2]) ^ MVector(c[0] - a[0], c[1] - a[1], c[2] - a[2])).normal();
    out.azPoint = MFloatPoint((float)bestPoint[0], (float)bestPoint[1], (float)bestPoint[2]);
    out.azNormal = MFloatVector((float)n.x, (float)n.y, (float)n.z);
    out.azFace = triFace_[found];
    out.azTriangle = triIndex_[found];
    // The reference's barycentric coordinates weigh the triangle's first two corners.
    out.azU = (float)bestW[0];
    out.azV = (float)bestW[1];
    return MS::kSuccess;
}

MStatus MFnMesh::getClosestPoint(const MPoint& to, MPoint& out, MSpace::Space space, int* polygon) const {
    MObject o = node_ >= 0 ? nodeObject(node_) : obj_;
    MMatrix w;
    bool world = (space == MSpace::kWorld) && azWorld(w);
    MMeshIntersector mi;
    MStatus st = mi.create(o, world ? w : MMatrix::identity);
    if (!st) return st;
    MPointOnMesh on;
    st = mi.getClosestPoint(to, on);
    if (!st) return st;
    out = MPoint(on.getPoint());
    if (world) out = out * w;
    if (polygon) *polygon = on.faceIndex();
    return MS::kSuccess;
}
MStatus MFnMesh::getClosestNormal(const MPoint& to, MVector& out, MSpace::Space space, int* polygon) const {
    MObject o = node_ >= 0 ? nodeObject(node_) : obj_;
    MMatrix w;
    bool world = (space == MSpace::kWorld) && azWorld(w);
    MMeshIntersector mi;
    MStatus st = mi.create(o, world ? w : MMatrix::identity);
    if (!st) return st;
    MPointOnMesh on;
    st = mi.getClosestPoint(to, on);
    if (!st) return st;
    out = MVector(on.getNormal());
    if (world) out = (out * w).normal();
    if (polygon) *polygon = on.faceIndex();
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// Selection lists

MItSelectionList::MItSelectionList(const MSelectionList& list, MFn::Type filter, MStatus* st) : list_(list), filter_(filter), at_(0) {
    azSkip();
    if (st) *st = MS::kSuccess;
}
MItSelectionList::~MItSelectionList() {}
void MItSelectionList::azSkip() {
    if (filter_ == MFn::kInvalid) return;
    while (at_ < list_.length()) {
        MObject o;
        if (list_.getDependNode(at_, o) && o.hasFn(filter_)) return;
        ++at_;
    }
}
bool MItSelectionList::isDone(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return at_ >= list_.length();
}
MStatus MItSelectionList::next() {
    ++at_;
    azSkip();
    return MS::kSuccess;
}
MStatus MItSelectionList::reset() {
    at_ = 0;
    azSkip();
    return MS::kSuccess;
}
MStatus MItSelectionList::getDependNode(MObject& out) { return list_.getDependNode(at_, out); }
MStatus MItSelectionList::getDagPath(MDagPath& out, MObject& comp) { return list_.getDagPath(at_, out, comp); }
MStatus MItSelectionList::getDagPath(MDagPath& out) { return list_.getDagPath(at_, out); }
MStatus MItSelectionList::getPlug(MPlug& out) const { return list_.getPlug(at_, out); }
MStatus MItSelectionList::getStrings(MStringArray& out) const { return list_.getSelectionStrings(at_, out); }
bool MItSelectionList::hasComponents(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return at_ < list_.azComponents().size() && !list_.azComponents()[at_].isNull();
}
MItSelectionList::selItemType MItSelectionList::itemType(MStatus* st) {
    if (st) *st = MS::kSuccess;
    if (at_ >= list_.length()) return kUnknownItem;
    if (list_.azItems()[at_].find('.') != std::string::npos && !hasComponents()) return kPlugSelectionItem;
    MObject o;
    if (!list_.getDependNode(at_, o)) return kUnknownItem;
    return o.hasFn(MFn::kDagNode) ? kDagSelectionItem : kDNselectionItem;
}
MStatus MItSelectionList::setFilter(MFn::Type filter) {
    filter_ = filter;
    return reset();
}

// ---------------------------------------------------------------------------------------------------------------------
// Mesh vertices

MItMeshVertex::MItMeshVertex(const MDagPath& path, const MObject& comp, MStatus* st) : mesh_(path, st), at_(0) {
    azBuild();
    const std::shared_ptr<ObjectData>& d = comp.azData();
    if (d && !d->complete && !d->ids.empty()) {
        ids_.clear();
        for (size_t i = 0; i < d->ids.size(); ++i)
            if (d->ids[i] >= 0 && (size_t)d->ids[i] < faces_.size()) ids_.push_back(d->ids[i]);
    }
}
MItMeshVertex::MItMeshVertex(const MObject& mesh, MStatus* st) : mesh_(mesh, st), at_(0) { azBuild(); }
MItMeshVertex::~MItMeshVertex() {}

void MItMeshVertex::azBuild() {
    MIntArray counts, connects;
    mesh_.getVertices(counts, connects);
    int n = mesh_.numVertices();
    faces_.assign(n, std::vector<int>());
    neighbours_.assign(n, std::vector<int>());
    edges_.assign(n, std::vector<int>());
    std::vector<std::set<int> > nb(n);
    unsigned int c = 0;
    for (unsigned int f = 0; f < counts.length(); ++f) {
        int k = counts[f];
        for (int i = 0; i < k; ++i) {
            int v = connects[c + i], next = connects[c + (i + 1) % k], prev = connects[c + (i + k - 1) % k];
            if (v < 0 || v >= n) continue;
            if (faces_[v].empty() || faces_[v].back() != (int)f) faces_[v].push_back((int)f);
            nb[v].insert(next);
            nb[v].insert(prev);
        }
        c += (unsigned int)k;
    }
    for (int v = 0; v < n; ++v) neighbours_[v].assign(nb[v].begin(), nb[v].end());
    // The mesh's own edge ids at each point.
    for (int e = 0; e < mesh_.numEdges(); ++e) {
        int2 ends;
        if (!mesh_.getEdgeVertices(e, ends)) continue;
        for (int k = 0; k < 2; ++k)
            if (ends[k] >= 0 && ends[k] < n) edges_[ends[k]].push_back(e);
    }
    ids_.clear();
    for (int v = 0; v < n; ++v) ids_.push_back(v);
}
bool MItMeshVertex::isDone(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return at_ >= ids_.size();
}
MStatus MItMeshVertex::next() {
    ++at_;
    return MS::kSuccess;
}
MStatus MItMeshVertex::reset() {
    at_ = 0;
    return MS::kSuccess;
}
int MItMeshVertex::count(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return (int)faces_.size();
}
int MItMeshVertex::index(MStatus* st) {
    if (st) *st = at_ < ids_.size() ? MS::kSuccess : MS::kFailure;
    return at_ < ids_.size() ? ids_[at_] : -1;
}
MStatus MItMeshVertex::setIndex(int i, int& prev) {
    prev = index();
    if (i < 0 || (size_t)i >= faces_.size()) return MS::kInvalidParameter;
    std::vector<int>::iterator it = std::find(ids_.begin(), ids_.end(), i);
    if (it == ids_.end()) {
        ids_.push_back(i);
        at_ = ids_.size() - 1;
    } else {
        at_ = (size_t)(it - ids_.begin());
    }
    return MS::kSuccess;
}
MPoint MItMeshVertex::position(MSpace::Space space, MStatus* st) {
    MPoint p;
    MStatus s = mesh_.getPoint(index(), p, space);
    if (st) *st = s;
    return p;
}
MStatus MItMeshVertex::setPosition(const MPoint& p, MSpace::Space space) { return mesh_.setPoint(index(), p, space); }
MStatus MItMeshVertex::getNormal(MVector& out, MSpace::Space space) { return mesh_.getVertexNormal(index(), out, space); }
MStatus MItMeshVertex::getConnectedFaces(MIntArray& out) {
    out.clear();
    int v = index();
    if (v < 0) return MS::kFailure;
    for (size_t i = 0; i < faces_[v].size(); ++i) out.append(faces_[v][i]);
    return MS::kSuccess;
}
MStatus MItMeshVertex::getConnectedVertices(MIntArray& out) {
    out.clear();
    int v = index();
    if (v < 0) return MS::kFailure;
    for (size_t i = 0; i < neighbours_[v].size(); ++i) out.append(neighbours_[v][i]);
    return MS::kSuccess;
}
MStatus MItMeshVertex::getConnectedEdges(MIntArray& out) {
    out.clear();
    int v = index();
    if (v < 0) return MS::kFailure;
    for (size_t i = 0; i < edges_[v].size(); ++i) out.append(edges_[v][i]);
    return MS::kSuccess;
}
MStatus MItMeshVertex::numConnectedFaces(int& out) const {
    out = at_ < ids_.size() ? (int)faces_[ids_[at_]].size() : 0;
    return MS::kSuccess;
}
MStatus MItMeshVertex::numConnectedEdges(int& out) const {
    out = at_ < ids_.size() ? (int)neighbours_[ids_[at_]].size() : 0;
    return MS::kSuccess;
}
MObject MItMeshVertex::currentItem(MStatus* st) {
    MFnSingleIndexedComponent fn;
    MObject o = fn.create(MFn::kMeshVertComponent, st);
    fn.addElement(index());
    return o;
}
bool MItMeshVertex::onBoundary(MStatus* st) {
    if (st) *st = MS::kSuccess;
    int v = index();
    return v >= 0 && neighbours_[v].size() > faces_[v].size();
}

// ---------------------------------------------------------------------------------------------------------------------
// Dependency graph

MItDependencyGraph::MItDependencyGraph(MObject& root, MFn::Type filter, Direction dir, Traversal trav, Level, MStatus* st)
    : root_(nodeOf(root)), filter_(filter), direction_(dir), traversal_(trav), at_(0) {
    if (st) *st = root_ >= 0 ? MS::kSuccess : MS::kInvalidParameter;
    azWalk();
}
MItDependencyGraph::MItDependencyGraph(MPlug& root, MFn::Type filter, Direction dir, Traversal trav, Level, MStatus* st)
    : root_(root.azNode()), filter_(filter), direction_(dir), traversal_(trav), at_(0) {
    if (st) *st = root_ >= 0 ? MS::kSuccess : MS::kInvalidParameter;
    azWalk();
}
MItDependencyGraph::~MItDependencyGraph() {}

// The nodes a node is connected to in a direction: upstream its sources, downstream its destinations.
static std::vector<int64_t> neighboursOf(int64_t id, bool upstream) {
    std::vector<int64_t> out;
    Json a = Json::object();
    a.set("id", id);
    Json pairs = query("node.connections", a);
    for (size_t i = 0; i < pairs.size(); ++i) {
        const std::string& src = pairs[i][(size_t)0].str();
        const std::string& dst = pairs[i][(size_t)1].str();
        const std::string& mine = upstream ? dst : src;
        const std::string& other = upstream ? src : dst;
        int64_t m = nodeNamed(mine.substr(0, mine.find('.')));
        if (m != id) continue;
        int64_t o = nodeNamed(other.substr(0, other.find('.')));
        if (o >= 0 && std::find(out.begin(), out.end(), o) == out.end()) out.push_back(o);
    }
    return out;
}

void MItDependencyGraph::azWalk() {
    found_.clear();
    at_ = 0;
    if (root_ < 0) return;
    bool up = direction_ == kUpstream;
    std::set<int64_t> seen;
    std::vector<int64_t> order;
    if (traversal_ == kDepthFirst) {
        std::vector<int64_t> stack(1, root_);
        while (!stack.empty()) {
            int64_t n = stack.back();
            stack.pop_back();
            if (!seen.insert(n).second) continue;
            order.push_back(n);
            std::vector<int64_t> next = neighboursOf(n, up);
            for (size_t i = next.size(); i-- > 0;) stack.push_back(next[i]);
        }
    } else {
        std::vector<int64_t> queue(1, root_);
        seen.insert(root_);
        for (size_t q = 0; q < queue.size(); ++q) {
            order.push_back(queue[q]);
            std::vector<int64_t> next = neighboursOf(queue[q], up);
            for (size_t i = 0; i < next.size(); ++i)
                if (seen.insert(next[i]).second) queue.push_back(next[i]);
        }
    }
    for (size_t i = 0; i < order.size(); ++i)
        if (filter_ == MFn::kInvalid || nodeHasFn(order[i], filter_)) found_.push_back(order[i]);
}
bool MItDependencyGraph::isDone(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return at_ >= found_.size();
}
MStatus MItDependencyGraph::next() {
    ++at_;
    return MS::kSuccess;
}
MStatus MItDependencyGraph::reset() {
    at_ = 0;
    return MS::kSuccess;
}
MStatus MItDependencyGraph::prune() { return MS::kSuccess; }
MObject MItDependencyGraph::currentItem(MStatus* st) {
    if (at_ >= found_.size()) {
        if (st) *st = MS::kFailure;
        return MObject();
    }
    if (st) *st = MS::kSuccess;
    return nodeObject(found_[at_]);
}
MObject MItDependencyGraph::thisNode(MStatus* st) { return currentItem(st); }
MObject MItDependencyGraph::rootNode(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return nodeObject(root_);
}
MItDependencyGraph::Direction MItDependencyGraph::currentDirection(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return direction_;
}
MStatus MItDependencyGraph::setCurrentFilter(MFn::Type filter) {
    filter_ = filter;
    azWalk();
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// Deformers

MFnGeometryFilter::MFnGeometryFilter() {}
MFnGeometryFilter::MFnGeometryFilter(const MObject& o, MStatus* st) : MFnDependencyNode(o, st) {
    if (st && *st && !o.hasFn(MFn::kGeometryFilt)) *st = MS::kInvalidParameter;
}
MFn::Type MFnGeometryFilter::type() const { return MFn::kGeometryFilt; }

// The shapes the deformer changes and their indices, as `deformer -q` gives them.
void MFnGeometryFilter::azGeometry(std::vector<std::string>& names, std::vector<int>& indices) const {
    std::string me = nodePath(nodeOf(obj_));
    Json q = Json::object();
    q.set("query", true);
    q.set("geometry", true);
    Json g = command("deformer", args1(me), q);
    Json qi = Json::object();
    qi.set("query", true);
    qi.set("geometryIndices", true);
    std::vector<double> idx = numbers(command("deformer", args1(me), qi));
    for (size_t i = 0; i < g.size(); ++i) {
        names.push_back(g[i].str());
        indices.push_back(i < idx.size() ? (int)idx[i] : (int)i);
    }
}
MStatus MFnGeometryFilter::getOutputGeometry(MObjectArray& out) const {
    out.clear();
    std::vector<std::string> names;
    std::vector<int> indices;
    azGeometry(names, indices);
    for (size_t i = 0; i < names.size(); ++i) {
        int64_t id = nodeNamed(names[i]);
        if (id >= 0) out.append(nodeObject(id));
    }
    return MS::kSuccess;
}
MStatus MFnGeometryFilter::getInputGeometry(MObjectArray& out) const { return getOutputGeometry(out); }
MObject MFnGeometryFilter::outputShapeAtIndex(unsigned int index, MStatus* st) const {
    std::vector<std::string> names;
    std::vector<int> indices;
    azGeometry(names, indices);
    for (size_t i = 0; i < names.size(); ++i)
        if (indices[i] == (int)index) {
            if (st) *st = MS::kSuccess;
            return nodeObject(nodeNamed(names[i]));
        }
    if (st) *st = MS::kInvalidParameter;
    return MObject();
}
MObject MFnGeometryFilter::inputShapeAtIndex(unsigned int index, MStatus* st) const { return outputShapeAtIndex(index, st); }
unsigned int MFnGeometryFilter::indexForOutputShape(const MObject& shape, MStatus* st) const {
    std::vector<std::string> names;
    std::vector<int> indices;
    azGeometry(names, indices);
    int64_t want = nodeOf(shape);
    for (size_t i = 0; i < names.size(); ++i)
        if (nodeNamed(names[i]) == want) {
            if (st) *st = MS::kSuccess;
            return (unsigned int)indices[i];
        }
    if (st) *st = MS::kInvalidParameter;
    return 0;
}
MStatus MFnGeometryFilter::getPathAtIndex(unsigned int index, MDagPath& out) const {
    MStatus st;
    MObject o = outputShapeAtIndex(index, &st);
    if (!st) return st;
    return MDagPath::getAPathTo(o, out);
}
float MFnGeometryFilter::envelope(MStatus* st) const {
    std::vector<double> v = numbers(command("getAttr", args1(nodePath(nodeOf(obj_)) + ".envelope"), Json::object(), st));
    return v.empty() ? 1.0f : (float)v[0];
}
MStatus MFnGeometryFilter::setEnvelope(float e) {
    Json a = args1(nodePath(nodeOf(obj_)) + ".envelope");
    a.push((double)e);
    MStatus st;
    command("setAttr", a, Json::object(), &st);
    return st;
}
MObject MFnGeometryFilter::deformerSet(MStatus* st) const {
    if (st) *st = MS::kFailure;
    return MObject();
}
unsigned int MFnGeometryFilter::numOutputConnections(MStatus* st) const {
    std::vector<std::string> names;
    std::vector<int> indices;
    azGeometry(names, indices);
    if (st) *st = MS::kSuccess;
    return (unsigned int)names.size();
}
unsigned int MFnGeometryFilter::indexForGroupId(unsigned int groupId, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return groupId;
}
unsigned int MFnGeometryFilter::groupIdAtIndex(unsigned int index, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return index;
}

MFnWeightGeometryFilter::MFnWeightGeometryFilter() {}
MFnWeightGeometryFilter::MFnWeightGeometryFilter(const MObject& o, MStatus* st) : MFnGeometryFilter(o, st) {}
MFn::Type MFnWeightGeometryFilter::type() const { return MFn::kWeightGeometryFilt; }

static std::string weightPlug(const MObject& node, unsigned int index, int point) {
    return nodePath(nodeOf(node)) + ".weightList[" + std::to_string(index) + "].weights[" + std::to_string(point) + "]";
}
MStatus MFnWeightGeometryFilter::getWeights(unsigned int index, const MObject& comps, MFloatArray& out) const {
    out.clear();
    const std::shared_ptr<ObjectData>& d = comps.azData();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->ids.size(); ++i) {
        MStatus st;
        std::vector<double> v = numbers(command("getAttr", args1(weightPlug(obj_, index, d->ids[i])), Json::object(), &st));
        out.append(v.empty() ? 1.0f : (float)v[0]);
    }
    return MS::kSuccess;
}
MStatus MFnWeightGeometryFilter::setWeight(const MDagPath&, unsigned int index, const MObject& comps, float w) {
    const std::shared_ptr<ObjectData>& d = comps.azData();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->ids.size(); ++i) {
        Json a = args1(weightPlug(obj_, index, d->ids[i]));
        a.push((double)w);
        MStatus st;
        command("setAttr", a, Json::object(), &st);
        if (!st) return st;
    }
    return MS::kSuccess;
}
MStatus MFnWeightGeometryFilter::setWeight(const MDagPath&, unsigned int index, const MObject& comps, MFloatArray& ws) {
    const std::shared_ptr<ObjectData>& d = comps.azData();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->ids.size() && i < ws.length(); ++i) {
        Json a = args1(weightPlug(obj_, index, d->ids[i]));
        a.push((double)ws[i]);
        MStatus st;
        command("setAttr", a, Json::object(), &st);
        if (!st) return st;
    }
    return MS::kSuccess;
}


// ---------------------------------------------------------------------------------------------------------------------
// Mesh faces

namespace {
// The ids a component names, those in range, or none when it names none (all, then).
std::vector<int> picked(const MObject& comp, size_t count) {
    std::vector<int> out;
    const std::shared_ptr<ObjectData>& d = comp.azData();
    if (!d || d->complete || d->ids.empty()) return out;
    for (size_t i = 0; i < d->ids.size(); ++i)
        if (d->ids[i] >= 0 && (size_t)d->ids[i] < count) out.push_back(d->ids[i]);
    return out;
}
std::vector<int> all(size_t n) {
    std::vector<int> out(n);
    for (size_t i = 0; i < n; ++i) out[i] = (int)i;
    return out;
}
MObject oneComponent(MFn::Type t, int id, MStatus* st) {
    MFnSingleIndexedComponent fn;
    MObject o = fn.create(t, st);
    fn.addElement(id);
    return o;
}
void appendUnique(MIntArray& out, int v) {
    for (unsigned int i = 0; i < out.length(); ++i)
        if (out[i] == v) return;
    out.append(v);
}
}  // namespace

MItMeshPolygon::MItMeshPolygon(const MDagPath& path, const MObject& comp, MStatus* st) : mesh_(path, st), at_(0) {
    azBuild();
    azPick(comp);
}
MItMeshPolygon::MItMeshPolygon(const MObject& mesh, MStatus* st) : mesh_(mesh, st), at_(0) {
    azBuild();
    azPick(MObject());
}
MItMeshPolygon::~MItMeshPolygon() {}

void MItMeshPolygon::azBuild() {
    MIntArray counts, connects, ce;
    mesh_.getVertices(counts, connects);
    mesh_.azCornerEdges(ce);
    counts_.assign(counts.length(), 0);
    start_.assign(counts.length(), 0);
    connects_.assign(connects.length(), 0);
    cornerEdges_.assign(ce.length(), -1);
    for (unsigned int i = 0; i < connects.length(); ++i) connects_[i] = connects[i];
    for (unsigned int i = 0; i < ce.length(); ++i) cornerEdges_[i] = ce[i];
    edgeFaces_.assign(mesh_.numEdges(), std::vector<int>());
    pointFaces_.assign(mesh_.numVertices(), std::vector<int>());
    int c = 0;
    for (unsigned int f = 0; f < counts.length(); ++f) {
        counts_[f] = counts[f];
        start_[f] = c;
        for (int k = 0; k < counts[f]; ++k) {
            int e = (size_t)(c + k) < cornerEdges_.size() ? cornerEdges_[c + k] : -1;
            if (e >= 0 && (size_t)e < edgeFaces_.size() && (edgeFaces_[e].empty() || edgeFaces_[e].back() != (int)f)) edgeFaces_[e].push_back((int)f);
            int v = connects_[c + k];
            if (v >= 0 && (size_t)v < pointFaces_.size() && (pointFaces_[v].empty() || pointFaces_[v].back() != (int)f)) pointFaces_[v].push_back((int)f);
        }
        c += counts[f];
    }
}
void MItMeshPolygon::azPick(const MObject& comp) {
    ids_ = picked(comp, counts_.size());
    if (ids_.empty()) ids_ = all(counts_.size());
    at_ = 0;
}
bool MItMeshPolygon::isDone(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return at_ >= ids_.size();
}
MStatus MItMeshPolygon::next() {
    ++at_;
    return MS::kSuccess;
}
MStatus MItMeshPolygon::reset() {
    at_ = 0;
    return MS::kSuccess;
}
MStatus MItMeshPolygon::reset(const MDagPath& path, const MObject& comp) {
    MStatus st;
    mesh_ = MFnMesh(path, &st);
    azBuild();
    azPick(comp);
    return st;
}
unsigned int MItMeshPolygon::count(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return (unsigned int)counts_.size();
}
unsigned int MItMeshPolygon::index(MStatus* st) {
    if (st) *st = at_ < ids_.size() ? MS::kSuccess : MS::kFailure;
    return at_ < ids_.size() ? (unsigned int)ids_[at_] : 0;
}
MStatus MItMeshPolygon::setIndex(int i, int& prev) {
    prev = at_ < ids_.size() ? ids_[at_] : -1;
    if (i < 0 || (size_t)i >= counts_.size()) return MS::kInvalidParameter;
    std::vector<int>::iterator it = std::find(ids_.begin(), ids_.end(), i);
    if (it == ids_.end()) {
        ids_.push_back(i);
        at_ = ids_.size() - 1;
    } else {
        at_ = (size_t)(it - ids_.begin());
    }
    return MS::kSuccess;
}
unsigned int MItMeshPolygon::polygonVertexCount(MStatus* st) {
    unsigned int f = index(st);
    return f < counts_.size() ? (unsigned int)counts_[f] : 0;
}
int MItMeshPolygon::vertexIndex(int k, MStatus* st) {
    unsigned int f = index(st);
    if (f >= counts_.size() || k < 0 || k >= counts_[f]) {
        if (st) *st = MS::kInvalidParameter;
        return -1;
    }
    return connects_[start_[f] + k];
}
MStatus MItMeshPolygon::getVertices(MIntArray& out) {
    out.clear();
    MStatus st;
    unsigned int f = index(&st);
    if (!st) return st;
    for (int k = 0; k < counts_[f]; ++k) out.append(connects_[start_[f] + k]);
    return MS::kSuccess;
}
MStatus MItMeshPolygon::getEdges(MIntArray& out) {
    out.clear();
    MStatus st;
    unsigned int f = index(&st);
    if (!st) return st;
    for (int k = 0; k < counts_[f]; ++k) out.append(cornerEdges_[start_[f] + k]);
    return MS::kSuccess;
}
MStatus MItMeshPolygon::getConnectedFaces(MIntArray& out) {
    // Faces sharing an edge with this one.
    out.clear();
    MIntArray es;
    MStatus st = getEdges(es);
    if (!st) return st;
    int me = (int)index();
    for (unsigned int i = 0; i < es.length(); ++i) {
        if (es[i] < 0 || (size_t)es[i] >= edgeFaces_.size()) continue;
        for (size_t j = 0; j < edgeFaces_[es[i]].size(); ++j)
            if (edgeFaces_[es[i]][j] != me) appendUnique(out, edgeFaces_[es[i]][j]);
    }
    return MS::kSuccess;
}
MStatus MItMeshPolygon::getConnectedEdges(MIntArray& out) {
    // Edges at this face's corners that aren't its own.
    out.clear();
    MIntArray vs, own;
    getVertices(vs);
    getEdges(own);
    for (int e = 0; e < (int)edgeFaces_.size(); ++e) {
        int2 ends;
        if (!mesh_.getEdgeVertices(e, ends)) continue;
        bool touches = false, mine = false;
        for (unsigned int i = 0; i < vs.length(); ++i) touches = touches || vs[i] == ends[0] || vs[i] == ends[1];
        for (unsigned int i = 0; i < own.length(); ++i) mine = mine || own[i] == e;
        if (touches && !mine) out.append(e);
    }
    return MS::kSuccess;
}
MStatus MItMeshPolygon::getConnectedVertices(MIntArray& out) {
    // Points of the faces at this face's corners, that aren't this face's.
    out.clear();
    MIntArray vs;
    MStatus st = getVertices(vs);
    if (!st) return st;
    int me = (int)index();
    for (unsigned int i = 0; i < vs.length(); ++i) {
        const std::vector<int>& fs = pointFaces_[vs[i]];
        for (size_t j = 0; j < fs.size(); ++j) {
            if (fs[j] == me) continue;
            for (int k = 0; k < counts_[fs[j]]; ++k) {
                int v = connects_[start_[fs[j]] + k];
                bool own = false;
                for (unsigned int m = 0; m < vs.length(); ++m) own = own || vs[m] == v;
                if (!own) appendUnique(out, v);
            }
        }
    }
    return MS::kSuccess;
}
MStatus MItMeshPolygon::numConnectedFaces(int& out) const {
    MItMeshPolygon* self = const_cast<MItMeshPolygon*>(this);
    MIntArray a;
    MStatus st = self->getConnectedFaces(a);
    out = (int)a.length();
    return st;
}
MStatus MItMeshPolygon::numConnectedEdges(int& out) const {
    MItMeshPolygon* self = const_cast<MItMeshPolygon*>(this);
    MIntArray a;
    MStatus st = self->getConnectedEdges(a);
    out = (int)a.length();
    return st;
}
MPoint MItMeshPolygon::point(int k, MSpace::Space space, MStatus* st) {
    MPoint p;
    int v = vertexIndex(k, st);
    if (v >= 0) mesh_.getPoint(v, p, space);
    return p;
}
MStatus MItMeshPolygon::getPoints(MPointArray& out, MSpace::Space space, MStatus*) {
    out.clear();
    MIntArray vs;
    MStatus st = getVertices(vs);
    for (unsigned int i = 0; i < vs.length(); ++i) {
        MPoint p;
        mesh_.getPoint(vs[i], p, space);
        out.append(p);
    }
    return st;
}
MStatus MItMeshPolygon::setPoint(const MPoint& p, unsigned int k, MSpace::Space space) {
    MStatus st;
    int v = vertexIndex((int)k, &st);
    if (!st) return st;
    return mesh_.setPoint(v, p, space);
}
MPoint MItMeshPolygon::center(MSpace::Space space, MStatus* st) {
    MPointArray pts;
    MStatus s = getPoints(pts, space);
    if (st) *st = s;
    MPoint c(0, 0, 0);
    for (unsigned int i = 0; i < pts.length(); ++i) {
        c.x += pts[i].x;
        c.y += pts[i].y;
        c.z += pts[i].z;
    }
    if (pts.length()) {
        c.x /= pts.length();
        c.y /= pts.length();
        c.z /= pts.length();
    }
    return c;
}
MStatus MItMeshPolygon::getNormal(MVector& out, MSpace::Space space) {
    MStatus st;
    unsigned int f = index(&st);
    if (!st) return st;
    return mesh_.getPolygonNormal((int)f, out, space);
}
MStatus MItMeshPolygon::getNormal(unsigned int k, MVector& out, MSpace::Space space) {
    MStatus st;
    unsigned int f = index(&st);
    if (!st) return st;
    if (k >= (unsigned int)counts_[f]) return MS::kInvalidParameter;
    MFloatVectorArray ns;
    st = mesh_.getNormals(ns, space);
    if (!st || (size_t)(start_[f] + k) >= ns.length()) return mesh_.getPolygonNormal((int)f, out, space);
    const MFloatVector& n = ns[start_[f] + k];
    out = MVector(n.x, n.y, n.z);
    return MS::kSuccess;
}
MStatus MItMeshPolygon::getArea(double& out, MSpace::Space space) {
    MPointArray pts;
    MStatus st = getPoints(pts, space);
    out = 0.0;
    for (unsigned int i = 1; i + 1 < pts.length(); ++i) out += ((pts[i] - pts[0]) ^ (pts[i + 1] - pts[0])).length() * 0.5;
    return st;
}
bool MItMeshPolygon::hasValidTriangulation(MStatus* st) { return polygonVertexCount(st) >= 3; }
MStatus MItMeshPolygon::numTriangles(int& out) {
    MStatus st;
    out = (int)polygonVertexCount(&st) - 2;
    if (out < 0) out = 0;
    return st;
}
// Each face as a fan from its first corner, as MFnMesh::getTriangles gives it.
MStatus MItMeshPolygon::getTriangle(int t, MPointArray& points, MIntArray& vertices, MSpace::Space space) {
    points.clear();
    vertices.clear();
    int n = 0;
    numTriangles(n);
    if (t < 0 || t >= n) return MS::kInvalidParameter;
    int corners[3] = {0, t + 1, t + 2};
    for (int k = 0; k < 3; ++k) {
        vertices.append(vertexIndex(corners[k]));
        points.append(point(corners[k], space));
    }
    return MS::kSuccess;
}
MStatus MItMeshPolygon::getTriangles(MPointArray& points, MIntArray& vertices, MSpace::Space space) {
    points.clear();
    vertices.clear();
    int n = 0;
    MStatus st = numTriangles(n);
    for (int t = 0; t < n; ++t) {
        MPointArray p;
        MIntArray v;
        getTriangle(t, p, v, space);
        for (unsigned int k = 0; k < 3; ++k) {
            points.append(p[k]);
            vertices.append(v[k]);
        }
    }
    return st;
}
bool MItMeshPolygon::hasUVs(MStatus* st) const { return mesh_.numUVs(st) > 0; }
MStatus MItMeshPolygon::getUVIndex(int k, int& id, const MString* set) { return mesh_.getPolygonUVid((int)index(), k, id, set); }
MStatus MItMeshPolygon::getUV(int k, float2& uv, const MString* set) { return mesh_.getPolygonUV((int)index(), k, uv[0], uv[1], set); }
MStatus MItMeshPolygon::getUVs(MFloatArray& u, MFloatArray& v, const MString* set) {
    u.clear();
    v.clear();
    unsigned int n = polygonVertexCount();
    for (unsigned int k = 0; k < n; ++k) {
        float a = 0, b = 0;
        MStatus st = mesh_.getPolygonUV((int)index(), (int)k, a, b, set);
        if (!st) return st;
        u.append(a);
        v.append(b);
    }
    return MS::kSuccess;
}
bool MItMeshPolygon::onBoundary(MStatus* st) {
    MIntArray es;
    if (st) *st = getEdges(es);
    for (unsigned int i = 0; i < es.length(); ++i)
        if (es[i] >= 0 && (size_t)es[i] < edgeFaces_.size() && edgeFaces_[es[i]].size() < 2) return true;
    return false;
}
bool MItMeshPolygon::isConvex(MStatus* st) {
    MPointArray pts;
    if (st) *st = getPoints(pts);
    MVector n;
    getNormal(n);
    unsigned int m = pts.length();
    for (unsigned int i = 0; i < m; ++i) {
        MVector a = pts[(i + 1) % m] - pts[i], b = pts[(i + 2) % m] - pts[(i + 1) % m];
        if ((a ^ b) * n < -1e-12) return false;
    }
    return true;
}
bool MItMeshPolygon::zeroArea(MStatus* st) {
    double a = 0;
    MStatus s = getArea(a);
    if (st) *st = s;
    return a < 1e-12;
}
MObject MItMeshPolygon::currentItem(MStatus* st) { return oneComponent(MFn::kMeshPolygonComponent, (int)index(), st); }

// ---------------------------------------------------------------------------------------------------------------------
// Mesh edges

MItMeshEdge::MItMeshEdge(const MDagPath& path, const MObject& comp, MStatus* st) : mesh_(path, st), at_(0) {
    azBuild();
    azPick(comp);
}
MItMeshEdge::MItMeshEdge(const MObject& mesh, MStatus* st) : mesh_(mesh, st), at_(0) {
    azBuild();
    azPick(MObject());
}
MItMeshEdge::~MItMeshEdge() {}

void MItMeshEdge::azBuild() {
    int n = mesh_.numEdges();
    ends_.assign((size_t)n * 2, -1);
    faces_.assign(n, std::vector<int>());
    pointEdges_.assign(mesh_.numVertices(), std::vector<int>());
    for (int e = 0; e < n; ++e) {
        int2 v;
        if (!mesh_.getEdgeVertices(e, v)) continue;
        ends_[e * 2] = v[0];
        ends_[e * 2 + 1] = v[1];
        for (int k = 0; k < 2; ++k)
            if (v[k] >= 0 && (size_t)v[k] < pointEdges_.size()) pointEdges_[v[k]].push_back(e);
    }
    MIntArray counts, connects, ce;
    mesh_.getVertices(counts, connects);
    mesh_.azCornerEdges(ce);
    unsigned int c = 0;
    for (unsigned int f = 0; f < counts.length(); ++f) {
        for (int k = 0; k < counts[f]; ++k) {
            int e = c + k < ce.length() ? ce[c + k] : -1;
            if (e >= 0 && e < n && (faces_[e].empty() || faces_[e].back() != (int)f)) faces_[e].push_back((int)f);
        }
        c += (unsigned int)counts[f];
    }
}
void MItMeshEdge::azPick(const MObject& comp) {
    ids_ = picked(comp, faces_.size());
    if (ids_.empty()) ids_ = all(faces_.size());
    at_ = 0;
}
bool MItMeshEdge::isDone(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return at_ >= ids_.size();
}
MStatus MItMeshEdge::next() {
    ++at_;
    return MS::kSuccess;
}
MStatus MItMeshEdge::reset() {
    at_ = 0;
    return MS::kSuccess;
}
MStatus MItMeshEdge::reset(const MDagPath& path, const MObject& comp) {
    MStatus st;
    mesh_ = MFnMesh(path, &st);
    azBuild();
    azPick(comp);
    return st;
}
int MItMeshEdge::count(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return (int)faces_.size();
}
int MItMeshEdge::index(MStatus* st) {
    if (st) *st = at_ < ids_.size() ? MS::kSuccess : MS::kFailure;
    return at_ < ids_.size() ? ids_[at_] : -1;
}
int MItMeshEdge::index(int end, MStatus* st) {
    int e = index(st);
    if (e < 0 || end < 0 || end > 1) {
        if (st) *st = MS::kInvalidParameter;
        return -1;
    }
    return ends_[e * 2 + end];
}
MStatus MItMeshEdge::setIndex(int i, int& prev) {
    prev = index();
    if (i < 0 || (size_t)i >= faces_.size()) return MS::kInvalidParameter;
    std::vector<int>::iterator it = std::find(ids_.begin(), ids_.end(), i);
    if (it == ids_.end()) {
        ids_.push_back(i);
        at_ = ids_.size() - 1;
    } else {
        at_ = (size_t)(it - ids_.begin());
    }
    return MS::kSuccess;
}
MPoint MItMeshEdge::point(int end, MSpace::Space space, MStatus* st) {
    MPoint p;
    int v = index(end, st);
    if (v >= 0) mesh_.getPoint(v, p, space);
    return p;
}
MPoint MItMeshEdge::center(MSpace::Space space, MStatus* st) {
    MPoint a = point(0, space, st), b = point(1, space, st);
    return MPoint((a.x + b.x) * 0.5, (a.y + b.y) * 0.5, (a.z + b.z) * 0.5);
}
MStatus MItMeshEdge::getLength(double& out, MSpace::Space space) {
    MStatus st;
    out = point(0, space, &st).distanceTo(point(1, space, &st));
    return st;
}
MStatus MItMeshEdge::getConnectedFaces(MIntArray& out, MStatus* st) {
    out.clear();
    int e = index(st);
    if (e < 0) return MS::kFailure;
    for (size_t i = 0; i < faces_[e].size(); ++i) out.append(faces_[e][i]);
    return MS::kSuccess;
}
MStatus MItMeshEdge::getConnectedEdges(MIntArray& out, MStatus* st) {
    // Edges sharing an end with this one.
    out.clear();
    int e = index(st);
    if (e < 0) return MS::kFailure;
    for (int k = 0; k < 2; ++k) {
        int v = ends_[e * 2 + k];
        if (v < 0 || (size_t)v >= pointEdges_.size()) continue;
        for (size_t i = 0; i < pointEdges_[v].size(); ++i)
            if (pointEdges_[v][i] != e) appendUnique(out, pointEdges_[v][i]);
    }
    return MS::kSuccess;
}
int MItMeshEdge::numConnectedFaces(MStatus* st) {
    int e = index(st);
    return e < 0 ? 0 : (int)faces_[e].size();
}
int MItMeshEdge::numConnectedEdges(MStatus* st) {
    MIntArray a;
    getConnectedEdges(a, st);
    return (int)a.length();
}
MStatus MItMeshEdge::numConnectedFaces(int& out) const {
    out = at_ < ids_.size() ? (int)faces_[ids_[at_]].size() : 0;
    return MS::kSuccess;
}
MStatus MItMeshEdge::numConnectedEdges(int& out) const {
    out = const_cast<MItMeshEdge*>(this)->numConnectedEdges();
    return MS::kSuccess;
}
bool MItMeshEdge::connectedToFace(int face, MStatus* st) {
    int e = index(st);
    return e >= 0 && std::find(faces_[e].begin(), faces_[e].end(), face) != faces_[e].end();
}
bool MItMeshEdge::connectedToEdge(int edge, MStatus* st) {
    MIntArray a;
    getConnectedEdges(a, st);
    for (unsigned int i = 0; i < a.length(); ++i)
        if (a[i] == edge) return true;
    return false;
}
bool MItMeshEdge::onBoundary(MStatus* st) {
    int e = index(st);
    return e >= 0 && faces_[e].size() < 2;
}
MObject MItMeshEdge::currentItem(MStatus* st) { return oneComponent(MFn::kMeshEdgeComponent, index(), st); }

// ---------------------------------------------------------------------------------------------------------------------
// DAG

MItDag::MItDag(TraversalType t, MFn::Type filter, MStatus* st) : traversal_(t == kInvalidType ? kDepthFirst : t), filter_(filter), root_(-1), current_(-1), depth_(0) {
    azStart();
    if (st) *st = MS::kSuccess;
}
MItDag::~MItDag() {}

// A node's children, in the host's order, after what is queued (breadth first) or before it (depth first).
void MItDag::azExpand(int64_t node, unsigned int depth) {
    std::vector<int64_t> kids;
    if (node < 0) {
        Json r = query("node.roots", Json::object());
        for (size_t i = 0; i < r.size(); ++i) kids.push_back(r[i].integer());
    } else {
        MDagPath p = MDagPath::azOf(node);
        unsigned int n = p.childCount();
        for (unsigned int i = 0; i < n; ++i) kids.push_back(nodeOf(p.child(i)));
    }
    if (traversal_ == kBreadthFirst) {
        for (size_t i = 0; i < kids.size(); ++i) queue_.push_back(std::make_pair(kids[i], depth));
    } else {
        for (size_t i = kids.size(); i-- > 0;) queue_.push_front(std::make_pair(kids[i], depth));
    }
}
// From a root, or from the scene's top nodes, each at depth 1.
void MItDag::azStart() {
    queue_.clear();
    current_ = -1;
    if (root_ >= 0) {
        queue_.push_back(std::make_pair(root_, 0u));
    } else {
        azExpand(-1, 1);
    }
    next();
}
MStatus MItDag::next() {
    while (!queue_.empty()) {
        std::pair<int64_t, unsigned int> it = queue_.front();
        queue_.pop_front();
        if (it.first < 0) continue;
        azExpand(it.first, it.second + 1);
        if (filter_ != MFn::kInvalid && !nodeHasFn(it.first, filter_)) continue;
        current_ = it.first;
        depth_ = it.second;
        return MS::kSuccess;
    }
    current_ = -1;
    return MS::kSuccess;
}
// The current node's children don't come: they were queued when it came, so take them out again.
MStatus MItDag::prune() {
    if (current_ < 0) return MS::kFailure;
    MDagPath p = MDagPath::azOf(current_);
    unsigned int n = p.childCount();
    std::set<int64_t> kids;
    for (unsigned int i = 0; i < n; ++i) kids.insert(nodeOf(p.child(i)));
    std::deque<std::pair<int64_t, unsigned int> > kept;
    for (size_t i = 0; i < queue_.size(); ++i)
        if (!(kids.count(queue_[i].first) && queue_[i].second == depth_ + 1)) kept.push_back(queue_[i]);
    queue_.swap(kept);
    return MS::kSuccess;
}
MStatus MItDag::reset() {
    azStart();
    return MS::kSuccess;
}
MStatus MItDag::reset(const MObject& root, TraversalType t, MFn::Type filter) {
    root_ = nodeOf(root);
    traversal_ = t == kInvalidType ? kDepthFirst : t;
    filter_ = filter;
    azStart();
    return root_ >= 0 || root.isNull() ? MS::kSuccess : MS::kInvalidParameter;
}
MStatus MItDag::reset(const MDagPath& root, TraversalType t, MFn::Type filter) { return reset(root.node(), t, filter); }
bool MItDag::isDone(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return current_ < 0;
}
MObject MItDag::currentItem(MStatus* st) const {
    if (st) *st = current_ >= 0 ? MS::kSuccess : MS::kFailure;
    return current_ >= 0 ? nodeObject(current_) : MObject();
}
MObject MItDag::item(MStatus* st) const { return currentItem(st); }
MStatus MItDag::getPath(MDagPath& out) const {
    if (current_ < 0) return MS::kFailure;
    out = MDagPath::azOf(current_);
    return MS::kSuccess;
}
MString MItDag::fullPathName(MStatus* st) const { return MDagPath::azOf(current_).fullPathName(st); }
MString MItDag::partialPathName(MStatus* st) const { return MDagPath::azOf(current_).partialPathName(st); }
unsigned int MItDag::depth(MStatus* st) const {
    if (st) *st = current_ >= 0 ? MS::kSuccess : MS::kFailure;
    return depth_;
}
MObject MItDag::root(MStatus* st) {
    if (st) *st = MS::kSuccess;
    return root_ >= 0 ? nodeObject(root_) : MObject();
}
bool MItDag::isInstanced(bool indirect, MStatus* st) const { return MDagPath::azOf(current_).isInstanced(st) && indirect; }
MItDag::TraversalType MItDag::getType(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return traversal_;
}
MStatus MItDag::setTraversalType(TraversalType t) {
    if (t == kInvalidType) return MS::kInvalidParameter;
    traversal_ = t;
    return reset();
}
