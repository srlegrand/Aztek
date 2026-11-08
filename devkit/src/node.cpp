// Aztek plug-in kit: the crash guard, node types and their compute, deformers, MItGeometry, MFnPlugin, load and unload.
#include "internal.h"

#include <cstring>
#include <exception>
#include <set>

#if defined(_WIN32)
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <dlfcn.h>
#include <setjmp.h>
#include <signal.h>
#endif

using namespace az;

// ---------------------------------------------------------------------------------------------------------------------
// Crash guard

#if defined(_WIN32)

static int sehFilter(unsigned long code, unsigned long* out) {
    // C++ exceptions go on to the try below.
    if (code == 0xE06D7363UL) return EXCEPTION_CONTINUE_SEARCH;
    *out = code;
    return EXCEPTION_EXECUTE_HANDLER;
}

static int sehCall(int (*fn)(void*), void* ctx, unsigned long* code) {
    __try {
        return fn(ctx);
    } __except (sehFilter(GetExceptionCode(), code)) {
        return AZ_CRASHED;
    }
}

int az::guarded(int (*fn)(void*), void* ctx, std::string& error) {
    unsigned long code = 0;
    try {
        int rc = sehCall(fn, ctx, &code);
        if (rc == AZ_CRASHED && code) {
            char buf[64];
            snprintf(buf, sizeof buf, "%s, code 0x%08lX", code == EXCEPTION_ACCESS_VIOLATION ? "access violation" : code == EXCEPTION_STACK_OVERFLOW ? "stack overflow" : "structured exception", code);
            error = buf;
        }
        return rc;
    } catch (const std::exception& e) {
        error = std::string("C++ exception: ") + e.what();
    } catch (...) {
        error = "an unknown C++ exception";
    }
    return AZ_FAILURE;
}

#else

static void jumpBack(void* buf, int sig) { siglongjmp(*(sigjmp_buf*)buf, sig); }

static const char* signalName(int sig) {
    switch (sig) {
    case SIGSEGV: return "segmentation fault, SIGSEGV";
    case SIGBUS: return "bus error, SIGBUS";
    case SIGFPE: return "arithmetic error, SIGFPE";
    case SIGILL: return "illegal instruction, SIGILL";
    case SIGABRT: return "aborted, SIGABRT";
    default: return "signal";
    }
}

static int callCatching(int (*fn)(void*), void* ctx, std::string& error) {
    try {
        return fn(ctx);
    } catch (const std::exception& e) {
        error = std::string("C++ exception: ") + e.what();
    } catch (...) {
        error = "an unknown C++ exception";
    }
    return AZ_FAILURE;
}

int az::guarded(int (*fn)(void*), void* ctx, std::string& error) {
    if (!host) return callCatching(fn, ctx, error);
    sigjmp_buf buf;
    void* prev = host->guard_enter(jumpBack, &buf);
    volatile int rc = AZ_FAILURE;
    int sig = sigsetjmp(buf, 1);
    if (sig == 0) {
        rc = callCatching(fn, ctx, error);
    } else {
        error = signalName(sig);
        rc = AZ_CRASHED;
    }
    host->guard_leave(prev);
    return rc;
}

#endif

// ---------------------------------------------------------------------------------------------------------------------
// Classes and attributes

std::vector<NodeClass*>& az::nodeClasses() {
    static std::vector<NodeClass*> v;
    return v;
}

NodeClass* az::classNamed(const std::string& name) {
    for (size_t i = 0; i < nodeClasses().size(); ++i)
        if (nodeClasses()[i]->name == name) return nodeClasses()[i];
    return 0;
}

NodeClass* az::classOfId(const MTypeId& id) {
    for (size_t i = 0; i < nodeClasses().size(); ++i)
        if (nodeClasses()[i]->id == id) return nodeClasses()[i];
    return 0;
}

NodeClass* az::classOfNode(int64_t node) {
    BlockImpl* b = currentBlock();
    if (b && b->node == node) return b->cls;
    Json a = Json::object();
    a.set("id", node);
    MStatus st;
    Json info = query("node.info", a, &st);
    return info.isObject() ? classNamed(info["type"].str()) : 0;
}

int az::indexOf(NodeClass* cls, const AttrSpec* spec, int* comp) {
    if (comp) *comp = -1;
    if (!cls || !spec) return -1;
    std::map<const AttrSpec*, int>::const_iterator it = cls->index.find(spec);
    if (it == cls->index.end()) return -1;
    std::map<const AttrSpec*, int>::const_iterator c = cls->component.find(spec);
    if (comp && c != cls->component.end()) *comp = c->second;
    return it->second;
}

BlockImpl*& az::currentBlock() {
    static thread_local BlockImpl* b = 0;
    return b;
}

static std::shared_ptr<AttrSpec> makeBuiltin(const char* name, const char* shortName, const char* type, const char* builtin, bool array) {
    std::shared_ptr<AttrSpec> s = std::make_shared<AttrSpec>();
    s->name = name;
    s->shortName = shortName;
    s->type = type;
    s->builtin = builtin;
    s->array = array;
    s->fn = std::string(type) == "compound" ? MFn::kCompoundAttribute : std::string(type) == "mesh" ? MFn::kTypedAttribute : MFn::kNumericAttribute;
    return s;
}

std::shared_ptr<AttrSpec> az::builtinAttr(const char* name) {
    static std::map<std::string, std::shared_ptr<AttrSpec> > all;
    if (all.empty()) {
        all["inputGeometry"] = makeBuiltin("inputGeometry", "ig", "mesh", "inputPolymesh", false);
        all["groupId"] = makeBuiltin("groupId", "gi", "int", "groupId", false);
        std::shared_ptr<AttrSpec> input = makeBuiltin("input", "ip", "compound", "input", true);
        input->children.push_back(all["inputGeometry"]);
        input->children.push_back(all["groupId"]);
        all["inputGeometry"]->parent = input.get();
        all["groupId"]->parent = input.get();
        all["input"] = input;
        all["outputGeometry"] = makeBuiltin("outputGeometry", "og", "mesh", "output", true);
        all["originalGeometry"] = makeBuiltin("originalGeometry", "orggeom", "mesh", "originalGeometry", true);
        all["envelope"] = makeBuiltin("envelope", "en", "float", "envelope", false);
        all["weights"] = makeBuiltin("weights", "w", "float", "weights", true);
        std::shared_ptr<AttrSpec> wl = makeBuiltin("weightList", "wl", "compound", "weightList", true);
        wl->children.push_back(all["weights"]);
        all["weights"]->parent = wl.get();
        all["weightList"] = wl;
        all["message"] = makeBuiltin("message", "msg", "message", "message", false);
        all["isHistoricallyInteresting"] = makeBuiltin("isHistoricallyInteresting", "ihi", "int", "isHistoricallyInteresting", false);
        all["caching"] = makeBuiltin("caching", "cch", "bool", "caching", false);
        all["nodeState"] = makeBuiltin("nodeState", "nds", "enum", "nodeState", false);
        all["frozen"] = makeBuiltin("frozen", "fzn", "bool", "frozen", false);
    }
    std::map<std::string, std::shared_ptr<AttrSpec> >::iterator it = all.find(name);
    return it == all.end() ? std::shared_ptr<AttrSpec>() : it->second;
}

MObject MPxNode::message = attrObject(builtinAttr("message"));
MObject MPxNode::isHistoricallyInteresting = attrObject(builtinAttr("isHistoricallyInteresting"));
MObject MPxNode::caching = attrObject(builtinAttr("caching"));
MObject MPxNode::state = attrObject(builtinAttr("nodeState"));
MObject MPxNode::frozen = attrObject(builtinAttr("frozen"));
MObject MPxGeometryFilter::input = attrObject(builtinAttr("input"));
MObject MPxGeometryFilter::inputGeom = attrObject(builtinAttr("inputGeometry"));
MObject MPxGeometryFilter::groupId = attrObject(builtinAttr("groupId"));
MObject MPxGeometryFilter::outputGeom = attrObject(builtinAttr("outputGeometry"));
MObject MPxGeometryFilter::originalGeometry = attrObject(builtinAttr("originalGeometry"));
MObject MPxGeometryFilter::envelope = attrObject(builtinAttr("envelope"));
MObject MPxDeformerNode::weightList = attrObject(builtinAttr("weightList"));
MObject MPxDeformerNode::weights = attrObject(builtinAttr("weights"));

// The class whose initialize function is running.
static NodeClass* registering = 0;

// ---------------------------------------------------------------------------------------------------------------------
// MPxNode

MPxNode::MPxNode() : azNodeId_(-1), azClass_(0) {}
MPxNode::~MPxNode() {}
void MPxNode::postConstructor() {}
MStatus MPxNode::compute(const MPlug&, MDataBlock&) { return MS::kUnknownParameter; }
MPxNode::SchedulingType MPxNode::schedulingType() const { return kDefaultScheduling; }
bool MPxNode::isAbstractClass() const { return false; }
bool MPxNode::isPassiveOutput(const MPlug&) const { return false; }
MStatus MPxNode::setDependentsDirty(const MPlug&, MPlugArray&) { return MS::kSuccess; }
MStatus MPxNode::connectionMade(const MPlug&, const MPlug&, bool) { return MS::kUnknownParameter; }
MStatus MPxNode::connectionBroken(const MPlug&, const MPlug&, bool) { return MS::kUnknownParameter; }
MPxNode::Type MPxNode::type() const { return kDependNode; }
MObject MPxNode::thisMObject() const { return nodeObject(azNodeId_); }
MString MPxNode::name(MStatus* st) const {
    MStatus s;
    MString n = MFnDependencyNode(thisMObject()).name(&s);
    if (st) *st = s;
    return n;
}
MTypeId MPxNode::typeId() const { return azClass_ ? azClass_->id : MTypeId(); }
MString MPxNode::typeName() const { return azClass_ ? MString(azClass_->name) : MString(); }
MStatus MPxNode::setExistWithoutInConnections(bool) { return MS::kSuccess; }
bool MPxNode::existWithoutInConnections(MStatus* st) const { if (st) *st = MS::kSuccess; return true; }
MStatus MPxNode::setExistWithoutOutConnections(bool) { return MS::kSuccess; }
bool MPxNode::existWithoutOutConnections(MStatus* st) const { if (st) *st = MS::kSuccess; return true; }

MStatus MPxNode::addAttribute(const MObject& attr) {
    if (!registering) return fail(MStatus::kFailure, "addAttribute belongs in the node's initialize function");
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (!d || !d->attr) return fail(MStatus::kInvalidParameter, "addAttribute: not an attribute");
    for (size_t i = 0; i < registering->attrs.size(); ++i)
        if (registering->attrs[i] == d->attr) return MS::kSuccess;
    registering->attrs.push_back(d->attr);
    return MS::kSuccess;
}

MStatus MPxNode::attributeAffects(const MObject& a, const MObject& b) {
    if (!registering) return fail(MStatus::kFailure, "attributeAffects belongs in the node's initialize function");
    const std::shared_ptr<ObjectData>& da = a.azData();
    const std::shared_ptr<ObjectData>& db = b.azData();
    if (!da || !da->attr || !db || !db->attr) return fail(MStatus::kInvalidParameter, "attributeAffects: not an attribute");
    registering->affects.push_back(std::make_pair(da->attr.get(), db->attr.get()));
    return MS::kSuccess;
}

MStatus MPxNode::inheritAttributesFrom(const MString& parent) {
    return fail(MStatus::kNotImplemented, std::string("inheritAttributesFrom ") + parent.asChar() + " isn't available");
}

// ---------------------------------------------------------------------------------------------------------------------
// Deformers

MPxGeometryFilter::MPxGeometryFilter() {}
MPxGeometryFilter::~MPxGeometryFilter() {}
MPxNode::Type MPxGeometryFilter::type() const { return kGeometryFilter; }
MStatus MPxGeometryFilter::deform(MDataBlock&, MItGeometry&, const MMatrix&, unsigned int) { return MS::kSuccess; }
MObject& MPxGeometryFilter::accessoryAttribute() const {
    static MObject none;
    return none;
}
MStatus MPxGeometryFilter::accessoryNodeSetup(MDagModifier&) { return MS::kSuccess; }
MObject MPxGeometryFilter::getInputGeom(unsigned int, MStatus* st) {
    BlockImpl* b = currentBlock();
    if (!b) {
        if (st) *st = fail(MStatus::kFailure, "getInputGeom belongs in compute");
        return MObject();
    }
    MDataBlock data(b);
    return data.inputValue(inputGeom, st).asMesh();
}

static MMatrix geomMatrix(BlockImpl* b) {
    MMatrix m;
    int i = host->attr_index(b->cls->name.c_str(), "geomMatrix");
    if (i >= 0) {
        double v[16];
        host->block_matrix(b->block, i, -1, v);
        for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) m.matrix[r][c] = v[r * 4 + c];
    }
    return m;
}

MStatus MPxGeometryFilter::compute(const MPlug& plug, MDataBlock& data) {
    if (plug.azSpec() != builtinAttr("outputGeometry")) return MS::kUnknownParameter;
    BlockImpl* b = data.azBlock();
    MObject in = data.inputValue(inputGeom).asMesh();
    MDataHandle out = data.outputValue(outputGeom);
    if (in.isNull()) return MS::kSuccess;
    // The input goes out unchanged until deform has run, so a failure leaves the mesh as it was.
    out.set(in);
    std::shared_ptr<MeshData> mesh = std::make_shared<MeshData>();
    mesh->setHandle(host->mesh_retain(in.azData()->mesh->handle));
    MMatrix world = geomMatrix(b);
    MItGeometry iter(mesh, world);
    MStatus st = deform(data, iter, world, 0);
    if (!st) return st;
    std::shared_ptr<ObjectData> od = std::make_shared<ObjectData>();
    od->type = MFn::kMeshData;
    od->mesh = mesh;
    out.set(MObject(od));
    return MS::kSuccess;
}

MPxDeformerNode::MPxDeformerNode() {}
MPxDeformerNode::~MPxDeformerNode() {}
MPxNode::Type MPxDeformerNode::type() const { return kDeformerNode; }
float MPxDeformerNode::weightValue(MDataBlock& block, unsigned int, unsigned int index) {
    BlockImpl* b = block.azBlock();
    if (!b || !b->block) return 1.0f;
    size_t n = 0;
    const double* w = host->block_weights(b->block, &n);
    if (!w) return 1.0f;
    return index < n ? (float)w[index] : 0.0f;
}

// ---------------------------------------------------------------------------------------------------------------------
// MItGeometry

MItGeometry::MItGeometry(const std::shared_ptr<MeshData>& mesh, const MMatrix& world) : mesh_(mesh), node_(-1), world_(world), at_(0) {
    mesh_->loadPoints();
    size_t n = mesh_->points.size() / 3;
    const double* w = 0;
    size_t wn = 0;
    BlockImpl* b = currentBlock();
    if (b && b->block) w = host->block_weights(b->block, &wn);
    const AzMeshArrays& a = mesh_->get();
    for (size_t i = 0; i < n; ++i) {
        size_t id = a.point_id_count ? (size_t)a.point_ids[i] : i;
        // Points outside the deformer's set have weight 0 here: they aren't visited, as the reference leaves them out.
        if (w && (id >= wn || w[id] == 0.0)) continue;
        order_.push_back((int)i);
    }
}

static std::shared_ptr<MeshData> meshOfNode(int64_t node, MStatus* st) {
    std::shared_ptr<MeshData> m = std::make_shared<MeshData>();
    char* err = 0;
    AzMesh* h = host ? host->mesh_of_node(node, &err) : 0;
    std::string e = take(err);
    if (!h) {
        if (st) *st = fail(MStatus::kInvalidParameter, e.empty() ? std::string("(kInvalidParameter): not a mesh") : e);
        return m;
    }
    m->setHandle(h);
    if (st) *st = MS::kSuccess;
    return m;
}

MItGeometry::MItGeometry(const MDagPath& path, MStatus* st) : node_(path.azNode()), at_(0) {
    mesh_ = meshOfNode(node_, st);
    world_ = path.inclusiveMatrix();
    mesh_->loadPoints();
    for (size_t i = 0; i < mesh_->points.size() / 3; ++i) order_.push_back((int)i);
}
MItGeometry::MItGeometry(const MObject& o, MStatus* st) : node_(nodeOf(o)), at_(0) {
    mesh_ = meshOfNode(node_, st);
    world_ = MDagPath::azOf(node_).inclusiveMatrix();
    mesh_->loadPoints();
    for (size_t i = 0; i < mesh_->points.size() / 3; ++i) order_.push_back((int)i);
}
MItGeometry::MItGeometry(MDataHandle& h, unsigned int, bool, MStatus* st) : node_(-1), at_(0) {
    MObject m = h.asMesh();
    mesh_ = m.azData() && m.azData()->mesh ? m.azData()->mesh : std::make_shared<MeshData>();
    mesh_->loadPoints();
    for (size_t i = 0; i < mesh_->points.size() / 3; ++i) order_.push_back((int)i);
    if (st) *st = m.isNull() ? MStatus(MS::kInvalidParameter) : MStatus(MS::kSuccess);
}
MItGeometry::MItGeometry(MDataHandle& h, bool ro, MStatus* st) : node_(-1), at_(0) {
    MItGeometry it(h, 0, ro, st);
    mesh_ = it.mesh_;
    order_ = it.order_;
}

MItGeometry::~MItGeometry() {
    // A scene mesh's points go back to it.
    if (node_ >= 0 && mesh_ && mesh_->pointsEdited) {
        Json pts = Json::array();
        for (size_t i = 0; i + 2 < mesh_->points.size(); i += 3) {
            Json p = Json::array();
            p.push(mesh_->points[i]).push(mesh_->points[i + 1]).push(mesh_->points[i + 2]);
            pts.push(p);
        }
        Json f = Json::object();
        f.set("points", pts);
        Json a = Json::array();
        a.push(nodePath(node_));
        command("setMeshPoints", a, f);
    }
}
bool MItGeometry::isDone(MStatus* st) { if (st) *st = MS::kSuccess; return at_ >= (int)order_.size(); }
MStatus MItGeometry::next() { ++at_; return MS::kSuccess; }
MStatus MItGeometry::reset() { at_ = 0; return MS::kSuccess; }
MPoint MItGeometry::position(MSpace::Space space, MStatus* st) const {
    if (at_ >= (int)order_.size()) {
        if (st) *st = MS::kFailure;
        return MPoint();
    }
    const double* p = &mesh_->points[order_[at_] * 3];
    MPoint q(p[0], p[1], p[2]);
    if (st) *st = MS::kSuccess;
    return space == MSpace::kWorld ? q * world_ : q;
}
MStatus MItGeometry::setPosition(const MPoint& p0, MSpace::Space space) {
    if (at_ >= (int)order_.size()) return MS::kFailure;
    MPoint p = space == MSpace::kWorld ? p0 * world_.inverse() : p0;
    double* q = &mesh_->points[order_[at_] * 3];
    q[0] = p.x;
    q[1] = p.y;
    q[2] = p.z;
    mesh_->pointsEdited = true;
    return MS::kSuccess;
}
MVector MItGeometry::normal(MSpace::Space space, MStatus* st) const {
    const AzMeshArrays& a = mesh_->get();
    if (at_ >= (int)order_.size() || !a.point_normals) {
        if (st) *st = MS::kFailure;
        return MVector();
    }
    const double* n = &a.point_normals[order_[at_] * 3];
    MVector v(n[0], n[1], n[2]);
    if (st) *st = MS::kSuccess;
    return space == MSpace::kWorld ? v.transformAsNormal(world_) : v;
}
MObject MItGeometry::currentItem(MStatus* st) {
    MObject c;
    component(c);
    if (st) *st = MS::kSuccess;
    return c;
}
MStatus MItGeometry::component(MObject& out) {
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = MFn::kMeshVertComponent;
    if (at_ < (int)order_.size()) d->ids.push_back(order_[at_]);
    out = MObject(d);
    return MS::kSuccess;
}
int MItGeometry::index(MStatus* st) const { if (st) *st = MS::kSuccess; return at_ < (int)order_.size() ? order_[at_] : -1; }
int MItGeometry::count(MStatus* st) const { if (st) *st = MS::kSuccess; return (int)order_.size(); }
int MItGeometry::exactCount(MStatus* st) const { return count(st); }
float MItGeometry::weight(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    BlockImpl* b = currentBlock();
    if (!b || !b->block || at_ >= (int)order_.size()) return 1.0f;
    size_t n = 0;
    const double* w = host->block_weights(b->block, &n);
    size_t i = (size_t)order_[at_];
    return !w ? 1.0f : i < n ? (float)w[i] : 0.0f;
}
MStatus MItGeometry::allPositions(MPointArray& out, MSpace::Space space) const {
    out.clear();
    for (size_t k = 0; k < order_.size(); ++k) {
        const double* p = &mesh_->points[order_[k] * 3];
        MPoint q(p[0], p[1], p[2]);
        out.append(space == MSpace::kWorld ? q * world_ : q);
    }
    return MS::kSuccess;
}
MStatus MItGeometry::setAllPositions(const MPointArray& pts, MSpace::Space space) {
    if (pts.length() != order_.size()) return MS::kInvalidParameter;
    MMatrix inv = world_.inverse();
    for (size_t k = 0; k < order_.size(); ++k) {
        MPoint p = space == MSpace::kWorld ? pts[(unsigned int)k] * inv : pts[(unsigned int)k];
        double* q = &mesh_->points[order_[k] * 3];
        q[0] = p.x;
        q[1] = p.y;
        q[2] = p.z;
    }
    mesh_->pointsEdited = true;
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// Node types as the host runs them

struct NodeCall {
    NodeClass* cls;
    MPxNode* node;
    int64_t id;
    const MPlug* plug;
    MDataBlock* data;
    MStatus status;
    int which;  // 0 create, 1 compute, 2 destroy
};

static int runNode(void* p) {
    NodeCall* c = (NodeCall*)p;
    switch (c->which) {
    case 0: {
        MPxNode* n = (MPxNode*)c->cls->creator();
        if (!n) return AZ_FAILURE;
        n->azNodeId_ = c->id;
        n->azClass_ = c->cls;
        {
            std::lock_guard<std::mutex> g(c->cls->lock);
            c->cls->instances[c->id] = n;
        }
        c->node = n;
        n->postConstructor();
        return AZ_OK;
    }
    case 1:
        c->status = c->node->compute(*c->plug, *c->data);
        if (c->status == MS::kUnknownParameter) return AZ_UNKNOWN_PARAMETER;
        return c->status ? AZ_OK : AZ_FAILURE;
    default: {
        {
            std::lock_guard<std::mutex> g(c->cls->lock);
            c->cls->instances.erase(c->node->azNodeId_);
        }
        delete c->node;
        return AZ_OK;
    }
    }
}

static char* hostString(const std::string& s) { return host ? host->string_new(s.c_str()) : 0; }

static void* nodeCreate(void* ctx, int64_t id, char** error) {
    NodeCall c = {(NodeClass*)ctx, 0, id, 0, 0, MStatus(), 0};
    std::string e;
    int rc = guarded(runNode, &c, e);
    if (rc != AZ_OK) {
        if (error) *error = hostString(e.empty() ? std::string("the creator made nothing") : e);
        return 0;
    }
    return c.node;
}

static void nodeDestroy(void* ctx, void* inst) {
    NodeCall c = {(NodeClass*)ctx, (MPxNode*)inst, 0, 0, 0, MStatus(), 2};
    std::string e;
    guarded(runNode, &c, e);
}

static int nodeScheduling(void*, void* inst) {
    switch (((MPxNode*)inst)->schedulingType()) {
    case MPxNode::kParallel: return AZ_SCHED_PARALLEL;
    case MPxNode::kSerial: return AZ_SCHED_SERIAL;
    case MPxNode::kGloballySerial: return AZ_SCHED_GLOBALLY_SERIAL;
    default: return AZ_SCHED_UNTRUSTED;
    }
}

static int nodeCompute(void* ctx, void* inst, int attr, int element, AzBlock* block, char** error) {
    NodeClass* cls = (NodeClass*)ctx;
    std::map<int, std::shared_ptr<AttrSpec> >::iterator it = cls->byIndex.find(attr);
    if (it == cls->byIndex.end()) return AZ_UNKNOWN_PARAMETER;
    BlockImpl b;
    b.block = block;
    b.cls = cls;
    b.node = host->block_node(block);
    MDataBlock data(&b);
    const std::shared_ptr<AttrSpec>& spec = it->second;
    bool inArrayCompound = spec->parent && spec->parent->array && spec->builtin.empty();
    MPlug plug = MPlug::azMake(b.node, spec->name, spec, inArrayCompound ? -1 : element, inArrayCompound ? element : -1);
    BlockImpl* was = currentBlock();
    currentBlock() = &b;
    NodeCall c = {cls, (MPxNode*)inst, b.node, &plug, &data, MStatus(), 1};
    std::string e;
    int rc = guarded(runNode, &c, e);
    currentBlock() = was;
    if (rc == AZ_FAILURE && e.empty()) e = c.status.errorString().asChar();
    if (rc != AZ_OK && rc != AZ_UNKNOWN_PARAMETER && error) *error = hostString(e);
    return rc;
}

// The host's attributes for a class: three numbers make a vec3; other compounds give their children; an array compound's
// children are arrays named parent.child.
static void hostAttrs(NodeClass* cls, const std::shared_ptr<AttrSpec>& s, const AttrSpec* arrayParent, const std::set<const AttrSpec*>& affected,
                      Json& out, std::vector<std::pair<std::string, std::shared_ptr<AttrSpec> > >& names) {
    if (s->type == "compound") {
        if (s->array && arrayParent) {
            log("// Warning: " + cls->name + "." + s->name + ": an array compound inside an array compound isn't available //", 4);
            return;
        }
        for (size_t i = 0; i < s->children.size(); ++i) hostAttrs(cls, s->children[i], s->array ? s.get() : arrayParent, affected, out, names);
        return;
    }
    Json j = Json::object();
    std::string name = arrayParent ? arrayParent->name + "." + s->name : s->name;
    std::string shortName = arrayParent ? arrayParent->shortName + "." + s->shortName : s->shortName;
    j.set("name", name);
    j.set("short", shortName);
    std::string type = s->type == "message" ? "bool" : s->type;
    j.set("type", type);
    bool output = affected.count(s.get()) || !s->writable || (s->parent && affected.count(s->parent));
    j.set("input", !output);
    j.set("storable", s->storable && !output);
    j.set("keyable", s->keyable);
    j.set("hidden", s->hidden);
    j.set("array", s->array || arrayParent != 0);
    j.set("color", s->color);
    if (!s->nice.empty()) j.set("nice", s->nice);
    if (s->hasMin) j.set("min", s->min);
    if (s->hasMax) j.set("max", s->max);
    if (s->hasSoftMin) j.set("softMin", s->softMin);
    if (s->hasSoftMax) j.set("softMax", s->softMax);
    if (type == "string") {
        j.set("default", s->defString);
    } else if (type == "vec3" || type == "matrix") {
        Json d = Json::array();
        for (size_t i = 0; i < s->def.size(); ++i) d.push(s->def[i]);
        while (type == "vec3" && d.size() < 3) d.push(0.0);
        j.set("default", d);
    } else if (!s->def.empty()) {
        j.set("default", s->def[0]);
    }
    if (type == "vec3") {
        Json kids = Json::array();
        static const char* axes[3] = {"X", "Y", "Z"};
        int n = s->components < 3 ? s->components : 3;
        for (int i = 0; i < n; ++i) {
            Json k = Json::object();
            if (i < (int)s->children.size()) {
                k.set("name", s->children[i]->name);
                k.set("short", s->children[i]->shortName);
            } else {
                k.set("name", s->name + axes[i]);
                k.set("short", s->shortName + std::string(1, (char)('x' + i)));
            }
            kids.push(k);
        }
        j.set("children", kids);
    }
    if (type == "enum") {
        int top = 0;
        for (size_t i = 0; i < s->fields.size(); ++i) if (s->fields[i].second > top) top = s->fields[i].second;
        std::vector<std::string> byValue(top + 1);
        for (size_t i = 0; i < s->fields.size(); ++i) if (s->fields[i].second >= 0) byValue[s->fields[i].second] = s->fields[i].first;
        Json e = Json::array();
        for (size_t i = 0; i < byValue.size(); ++i) e.push(byValue[i]);
        j.set("enum", e);
    }
    out.push(j);
    names.push_back(std::make_pair(name, s));
}

MStatus MFnPlugin::registerNode(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init, MPxNode::Type type, const MString*) {
    if (!host || !creator) return fail(MStatus::kInvalidParameter, "registerNode: no creator");
    int kind = AZ_NODE_DEPEND;
    if (type == MPxNode::kDeformerNode || type == MPxNode::kGeometryFilter) {
        kind = AZ_NODE_DEFORMER;
    } else if (type != MPxNode::kDependNode) {
        return fail(MStatus::kNotImplemented, std::string("registerNode ") + name.asChar() + ": only dependency nodes and deformers can come from plug-ins yet");
    }
    if (classOfId(id)) return fail(MStatus::kFailure, std::string("registerNode ") + name.asChar() + ": a node type with that id is already registered");
    NodeClass* c = new NodeClass();
    c->name = name.asChar();
    c->id = id;
    c->creator = creator;
    c->kind = kind;
    NodeClass* was = registering;
    registering = c;
    MStatus st;
    if (init) {
        std::string e;
        struct Init {
            MInitializeFunction fn;
            MStatus st;
            static int run(void* p) {
                Init* i = (Init*)p;
                i->st = i->fn();
                return i->st ? AZ_OK : AZ_FAILURE;
            }
        } ini = {init, MStatus()};
        int rc = guarded(Init::run, &ini, e);
        st = ini.st;
        if (rc == AZ_CRASHED) st = fail(MStatus::kFailure, "the initialize function crashed (" + e + ")");
        else if (rc != AZ_OK && !e.empty()) st = fail(MStatus::kFailure, e);
    }
    registering = was;
    if (!st) {
        delete c;
        return st;
    }
    // What gets computed: whatever is affected. A child added on its own as well as through its parent counts once.
    std::set<const AttrSpec*> affected;
    for (size_t i = 0; i < c->affects.size(); ++i) affected.insert(c->affects[i].second);
    std::set<const AttrSpec*> inside;
    for (size_t i = 0; i < c->attrs.size(); ++i)
        for (size_t k = 0; k < c->attrs[i]->children.size(); ++k) inside.insert(c->attrs[i]->children[k].get());
    Json attrs = Json::array();
    std::vector<std::pair<std::string, std::shared_ptr<AttrSpec> > > names;
    for (size_t i = 0; i < c->attrs.size(); ++i) {
        const std::shared_ptr<AttrSpec>& s = c->attrs[i];
        if (inside.count(s.get()) || !s->builtin.empty()) continue;
        hostAttrs(c, s, 0, affected, attrs, names);
    }
    Json outputs = Json::array();
    for (size_t i = 0; i < names.size(); ++i) {
        const AttrSpec* s = names[i].second.get();
        if (affected.count(s) || !s->writable || (s->parent && affected.count(s->parent))) outputs.push(names[i].first);
    }
    c->cls.ctx = c;
    c->cls.create = nodeCreate;
    c->cls.destroy = nodeDestroy;
    c->cls.scheduling = nodeScheduling;
    c->cls.compute = nodeCompute;
    std::string err = take(host->register_node(pluginToken, c->name.c_str(), id.id(), kind, attrs.dump().c_str(), outputs.dump().c_str(), &c->cls));
    if (!err.empty()) {
        delete c;
        return fail(MStatus::kFailure, err);
    }
    for (size_t i = 0; i < names.size(); ++i) {
        int idx = host->attr_index(c->name.c_str(), names[i].first.c_str());
        if (idx < 0) continue;
        const std::shared_ptr<AttrSpec>& s = names[i].second;
        c->index[s.get()] = idx;
        c->byIndex[idx] = s;
        for (size_t k = 0; k < s->children.size() && s->type == "vec3"; ++k) {
            c->index[s->children[k].get()] = idx;
            c->component[s->children[k].get()] = (int)k;
        }
    }
    if (kind == AZ_NODE_DEFORMER) {
        const char* builtins[] = {"inputGeometry", "outputGeometry", "envelope"};
        for (int k = 0; k < 3; ++k) {
            std::shared_ptr<AttrSpec> s = builtinAttr(builtins[k]);
            int idx = host->attr_index(c->name.c_str(), s->builtin.c_str());
            if (idx < 0) continue;
            c->index[s.get()] = idx;
            c->byIndex[idx] = s;
        }
    }
    nodeClasses().push_back(c);
    return MS::kSuccess;
}

MStatus MFnPlugin::deregisterNode(const MTypeId& id) {
    NodeClass* c = classOfId(id);
    if (!c || !host) return fail(MStatus::kFailure, "deregisterNode: no node type has that id");
    std::string err = take(host->deregister_node(pluginToken, c->name.c_str()));
    if (!err.empty()) return fail(MStatus::kFailure, err);
    // Nodes of the type can outlive this, so the class stays until the plug-in goes.
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// MFnPlugin

static std::string& pluginPath() {
    static std::string p;
    return p;
}
static std::string& pluginName() {
    static std::string n;
    return n;
}
static std::string pluginVendor = "Unknown", pluginVersion = "Unknown", pluginApi = "Any";
static std::vector<std::string> deleteProcs;

MFnPlugin::MFnPlugin() {}
MFnPlugin::MFnPlugin(MObject& obj, const char* vendor, const char* version, const char* api, MStatus* st) {
    obj_ = obj;
    pluginVendor = vendor ? vendor : "Unknown";
    pluginVersion = version ? version : "Unknown";
    pluginApi = api ? api : "Any";
    if (host) take(host->plugin_info(pluginToken, pluginVendor.c_str(), pluginVersion.c_str(), pluginApi.c_str()));
    if (st) *st = MS::kSuccess;
}
MFnPlugin::~MFnPlugin() {}
MFn::Type MFnPlugin::type() const { return MFn::kPluginObject; }
MString MFnPlugin::vendor(MStatus* st) const { if (st) *st = MS::kSuccess; return MString(pluginVendor); }
MString MFnPlugin::version(MStatus* st) const { if (st) *st = MS::kSuccess; return MString(pluginVersion); }
MString MFnPlugin::apiVersion(MStatus* st) const { if (st) *st = MS::kSuccess; return MString(pluginApi); }
MString MFnPlugin::name(MStatus* st) const { if (st) *st = MS::kSuccess; return MString(pluginName()); }
MString MFnPlugin::loadPath(MStatus* st) const {
    if (st) *st = MS::kSuccess;
    const std::string& p = pluginPath();
    size_t k = p.find_last_of("/\\");
    return MString(k == std::string::npos ? std::string(".") : p.substr(0, k));
}
MStatus MFnPlugin::setName(const MString&, bool) { return MS::kSuccess; }
MStatus MFnPlugin::setVersion(const MString& v) {
    pluginVersion = v.asChar();
    if (host) take(host->plugin_info(pluginToken, "", pluginVersion.c_str(), ""));
    return MS::kSuccess;
}
MStatus MFnPlugin::registerUI(const MString& create, const MString& remove, const MString&, const MString&) {
    if (remove.length()) deleteProcs.push_back(remove.asChar());
    if (!create.length()) return MS::kSuccess;
    MStatus st;
    mel(create.asChar(), &st);
    if (!st) log("// Warning: " + pluginName() + ": its UI procedure " + create.asChar() + " failed: " + lastError() + " //", 4);
    return MS::kSuccess;
}
MStatus MFnPlugin::registerUIStrings(void (*proc)(), const MString&) {
    if (proc) proc();
    return MS::kSuccess;
}
MObject MFnPlugin::findPlugin(const MString& name) {
    if (name == MString(pluginName())) {
        std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
        d->type = MFn::kPluginObject;
        d->plugin = pluginToken;
        return MObject(d);
    }
    return MObject();
}
bool MFnPlugin::isNodeRegistered(const MString& type) {
    if (classNamed(type.asChar())) return true;
    MStatus st;
    Json r = mel(std::string("nodeType -isTypeName ") + quoted(type.asChar()), &st);
    return st && r.truth();
}

// ---------------------------------------------------------------------------------------------------------------------
// Load and unload, called from the entry points every plug-in has (entry.cpp).

static std::string ownPath() {
#if defined(_WIN32)
    HMODULE m = 0;
    GetModuleHandleExA(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT, (LPCSTR)&ownPath, &m);
    char buf[4096];
    DWORD n = GetModuleFileNameA(m, buf, sizeof buf);
    return std::string(buf, n);
#else
    Dl_info info;
    if (dladdr((void*)&ownPath, &info) && info.dli_fname) return info.dli_fname;
    return std::string();
#endif
}

namespace az {

typedef MStatus (*EntryFn)(MObject);

struct EntryCall {
    EntryFn fn;
    MObject obj;
    MStatus st;
    static int run(void* p) {
        EntryCall* c = (EntryCall*)p;
        c->st = c->fn(c->obj);
        return c->st ? AZ_OK : AZ_FAILURE;
    }
};

static MObject pluginObject() {
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = MFn::kPluginObject;
    d->plugin = pluginToken;
    return MObject(d);
}

int load(const AzHost* h, void* plugin, char** error, EntryFn init) {
    if (!h || h->abi_version < 1) return AZ_FAILURE;
    host = h;
    pluginToken = plugin;
    pluginPath() = ownPath();
    std::string base = pluginPath();
    size_t k = base.find_last_of("/\\");
    if (k != std::string::npos) base = base.substr(k + 1);
    size_t dot = base.rfind('.');
    pluginName() = dot == std::string::npos ? base : base.substr(0, dot);
    MStatus st;
    Json unit = command("currentUnit", Json::array(), Json::object().set("query", true).set("time", true), &st);
    static const char* names[] = {"", "hour", "min", "sec", "millisec", "game", "film", "pal", "ntsc", "show", "palf", "ntscf", "120fps"};
    for (int i = 1; i <= 12; ++i)
        if (unit.text() == names[i]) uiTimeUnit = (MTime::Unit)i;
    EntryCall c = {init, pluginObject(), MStatus()};
    std::string e;
    int rc = guarded(EntryCall::run, &c, e);
    if (rc != AZ_OK && error) {
        if (e.empty()) e = c.st.errorString().asChar();
        *error = host->string_new(e.c_str());
    }
    return rc;
}

int unload(char** error, EntryFn uninit) {
    for (size_t i = 0; i < deleteProcs.size(); ++i) mel(deleteProcs[i]);
    deleteProcs.clear();
    EntryCall c = {uninit, pluginObject(), MStatus()};
    std::string e;
    int rc = guarded(EntryCall::run, &c, e);
    if (rc != AZ_OK && error) {
        if (e.empty()) e = c.st.errorString().asChar();
        *error = host->string_new(e.c_str());
    }
    return rc;
}

}  // namespace az
