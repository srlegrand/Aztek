// Aztek plug-in kit: iterators over the scene and meshes, component function sets, closest points on a mesh, the
// thread pool and the deformer function sets.
#pragma once
#include "geometry.h"

// ---------------------------------------------------------------------------------------------------------------------
// Thread pool. Tasks run one after the other on the calling thread, in the order they were made: a compute may not
// reach the host from another thread, and a plug-in's tasks often do.

typedef void* MThreadRetVal;
typedef MThreadRetVal (*MThreadFunc)(void*);

class MThreadRootTask {
public:
    std::vector<std::pair<MThreadFunc, void*> > azTasks;
};

typedef void (*MThreadCallbackFunc)(void* data, MThreadRootTask* root);
typedef void (*MThreadLinearRegionFunc)(void* data);

class MThreadPool {
public:
    static MStatus init();
    static void release();
    static MStatus newParallelRegion(MThreadCallbackFunc func, void* data);
    static MStatus createTask(MThreadFunc func, void* data, MThreadRootTask* root);
    static MStatus executeAndJoin(MThreadRootTask* root);
    static MStatus executeAndRelease(MThreadRootTask* root);
    static MStatus newParallelRegionWithSpinLock(MThreadCallbackFunc func, void* data);
};

class MThreadUtils {
public:
    static int getNumThreads();
    static void setNumThreads(int n);
};

// ---------------------------------------------------------------------------------------------------------------------
// Components

class MFnComponent : public MFnBase {
public:
    MFnComponent();
    MFnComponent(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    int elementCount(MStatus* status = 0) const;
    MFn::Type componentType(MStatus* status = 0) const;
    bool isEmpty(MStatus* status = 0) const;
    bool isComplete(MStatus* status = 0) const;
    MStatus setComplete(bool complete);
    bool isEqual(const MObject& other, MStatus* status = 0) const;
};

class MFnSingleIndexedComponent : public MFnComponent {
public:
    MFnSingleIndexedComponent();
    MFnSingleIndexedComponent(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MObject create(MFn::Type compType, MStatus* status = 0);
    MStatus addElement(int element);
    MStatus addElements(MIntArray& elements);
    int element(int index, MStatus* status = 0) const;
    MStatus getElements(MIntArray& out) const;
    MStatus setCompleteData(int numElements);
    MStatus getCompleteData(int& numElements) const;
};

// ---------------------------------------------------------------------------------------------------------------------
// Closest points

class MPointOnMesh {
public:
    MPointOnMesh();
    const MFloatPoint& getPoint() const;
    const MFloatVector& getNormal() const;
    int faceIndex() const;
    int triangleIndex() const;
    void getBarycentricCoords(float& u, float& v) const;

    // The kit's own.
    MFloatPoint azPoint;
    MFloatVector azNormal;
    int azFace;
    int azTriangle;
    float azU, azV;
};

// Closest points on a mesh, every triangle tried. The point asked about is in world space, taken through the inverse
// of the matrix given to create(); the point found is in the mesh's own space.
class MMeshIntersector {
public:
    MMeshIntersector();
    ~MMeshIntersector();
    MStatus create(MObject& meshObject, const MMatrix& matrix = MMatrix::identity);
    bool isCreated();
    MStatus getClosestPoint(const MPoint& point, MPointOnMesh& out, double maxDistance = 1.0e+300);

private:
    bool created_;
    MMatrix inverse_;
    std::vector<double> points_;
    std::vector<int> tris_;      // three points per triangle
    std::vector<int> triFace_;   // the face each comes from
    std::vector<int> triIndex_;  // and its index in that face
};

// ---------------------------------------------------------------------------------------------------------------------
// Iterators

class MItSelectionList {
public:
    enum selItemType { kUnknownItem = -1, kDagSelectionItem, kAnimSelectionItem, kDNselectionItem, kPlugSelectionItem };
    MItSelectionList(const MSelectionList& list, MFn::Type filter = MFn::kInvalid, MStatus* status = 0);
    ~MItSelectionList();
    bool isDone(MStatus* status = 0);
    MStatus next();
    MStatus reset();
    MStatus getDependNode(MObject& out);
    MStatus getDagPath(MDagPath& out, MObject& component);
    MStatus getDagPath(MDagPath& out);
    MStatus getPlug(MPlug& out) const;
    MStatus getStrings(MStringArray& out) const;
    bool hasComponents(MStatus* status = 0) const;
    selItemType itemType(MStatus* status = 0);
    MStatus setFilter(MFn::Type filter);

private:
    void azSkip();
    MSelectionList list_;
    MFn::Type filter_;
    unsigned int at_;
};

class MItMeshVertex {
public:
    MItMeshVertex(const MDagPath& path, const MObject& component = MObject::kNullObj, MStatus* status = 0);
    MItMeshVertex(const MObject& mesh, MStatus* status = 0);
    ~MItMeshVertex();
    bool isDone(MStatus* status = 0);
    MStatus next();
    MStatus reset();
    int count(MStatus* status = 0);
    int index(MStatus* status = 0);
    MStatus setIndex(int index, int& prevIndex);
    MPoint position(MSpace::Space space = MSpace::kObject, MStatus* status = 0);
    MStatus setPosition(const MPoint& point, MSpace::Space space = MSpace::kObject);
    MStatus getNormal(MVector& out, MSpace::Space space = MSpace::kObject);
    MStatus getConnectedFaces(MIntArray& out);
    MStatus getConnectedEdges(MIntArray& out);
    MStatus getConnectedVertices(MIntArray& out);
    MStatus numConnectedFaces(int& out) const;
    MStatus numConnectedEdges(int& out) const;
    MObject currentItem(MStatus* status = 0);
    bool onBoundary(MStatus* status = 0);

private:
    void azBuild();
    MFnMesh mesh_;
    std::vector<int> ids_;
    size_t at_;
    std::vector<std::vector<int> > faces_;
    std::vector<std::vector<int> > edges_;
    std::vector<std::vector<int> > neighbours_;
};

class MItDependencyGraph {
public:
    enum Direction { kDownstream, kUpstream };
    enum Traversal { kDepthFirst, kBreadthFirst };
    enum Level { kNodeLevel, kPlugLevel };
    MItDependencyGraph(MObject& root, MFn::Type filter = MFn::kInvalid, Direction direction = kDownstream, Traversal traversal = kDepthFirst, Level level = kNodeLevel, MStatus* status = 0);
    MItDependencyGraph(MPlug& root, MFn::Type filter = MFn::kInvalid, Direction direction = kDownstream, Traversal traversal = kDepthFirst, Level level = kNodeLevel, MStatus* status = 0);
    ~MItDependencyGraph();
    bool isDone(MStatus* status = 0) const;
    MStatus next();
    MStatus reset();
    MStatus prune();
    MObject currentItem(MStatus* status = 0);
    MObject thisNode(MStatus* status = 0);
    MObject rootNode(MStatus* status = 0);
    Direction currentDirection(MStatus* status = 0) const;
    MStatus setCurrentFilter(MFn::Type filter);

private:
    void azWalk();
    int64_t root_;
    MFn::Type filter_;
    Direction direction_;
    Traversal traversal_;
    std::vector<int64_t> found_;
    size_t at_;
};

// ---------------------------------------------------------------------------------------------------------------------
// Deformers

class MFnGeometryFilter : public MFnDependencyNode {
public:
    MFnGeometryFilter();
    MFnGeometryFilter(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MStatus getInputGeometry(MObjectArray& out) const;
    MStatus getOutputGeometry(MObjectArray& out) const;
    MObject inputShapeAtIndex(unsigned int index, MStatus* status = 0) const;
    MObject outputShapeAtIndex(unsigned int index, MStatus* status = 0) const;
    unsigned int indexForOutputShape(const MObject& shape, MStatus* status = 0) const;
    MStatus getPathAtIndex(unsigned int index, MDagPath& out) const;
    float envelope(MStatus* status = 0) const;
    MStatus setEnvelope(float envelope);
    MObject deformerSet(MStatus* status = 0) const;
    unsigned int numOutputConnections(MStatus* status = 0) const;
    unsigned int indexForGroupId(unsigned int groupId, MStatus* status = 0) const;
    unsigned int groupIdAtIndex(unsigned int index, MStatus* status = 0) const;

private:
    void azGeometry(std::vector<std::string>& names, std::vector<int>& indices) const;
};

class MFnWeightGeometryFilter : public MFnGeometryFilter {
public:
    MFnWeightGeometryFilter();
    MFnWeightGeometryFilter(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MStatus getWeights(unsigned int index, const MObject& components, MFloatArray& weights) const;
    MStatus setWeight(const MDagPath& path, unsigned int index, const MObject& components, float weight);
    MStatus setWeight(const MDagPath& path, unsigned int index, const MObject& components, MFloatArray& weights);
};
