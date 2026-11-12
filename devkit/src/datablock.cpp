// Aztek plug-in kit: data handles and blocks over the host's compute block, and mesh data.
#include "internal.h"

#include <algorithm>
#include <cstring>
#include <set>

using namespace az;

static const double kDeg = 180.0 / 3.14159265358979323846;

// ---------------------------------------------------------------------------------------------------------------------
// Mesh data

MeshData::MeshData() { memset(&arrays, 0, sizeof arrays); }
MeshData::~MeshData() {
    if (haveArrays && host) host->mesh_arrays_free(&arrays);
    if (handle && host) host->mesh_release(handle);
}
void MeshData::setHandle(AzMesh* h) {
    if (haveArrays && host) host->mesh_arrays_free(&arrays);
    haveArrays = false;
    memset(&arrays, 0, sizeof arrays);
    if (handle && host) host->mesh_release(handle);
    handle = h;
}
const AzMeshArrays& MeshData::get() {
    if (!haveArrays && handle && host) {
        host->mesh_arrays_of(handle, &arrays);
        haveArrays = true;
    }
    return arrays;
}
void MeshData::loadPoints() {
    if (pointsEdited) return;
    const AzMeshArrays& a = get();
    points.assign(a.points, a.points ? a.points + a.point_count * 3 : a.points);
}
AzMesh* MeshData::commit() {
    if (pointsEdited && handle && host) {
        AzMesh* h = host->mesh_with_points(handle, points.data(), points.size() / 3);
        std::vector<double> keep;
        keep.swap(points);
        setHandle(h);
        points.swap(keep);
        pointsEdited = false;
    }
    return handle;
}

static MObject meshObject(AzMesh* h) {
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = MFn::kMeshData;
    d->mesh = std::make_shared<MeshData>();
    d->mesh->setHandle(h);
    return MObject(d);
}

// ---------------------------------------------------------------------------------------------------------------------

BlockImpl& az::looseStorage() {
    static thread_local BlockImpl b;
    return b;
}

static BlockImpl& store(BlockImpl* b) { return b ? *b : looseStorage(); }

MDGContext MDGContext::fsNormal;
MDGContext::MDGContext() : normal_(true) {}
MDGContext::MDGContext(const MTime& t) : normal_(false), time_(t) {}
bool MDGContext::isNormal(MStatus* st) const { if (st) *st = MS::kSuccess; return normal_; }
bool MDGContext::isCurrent(MStatus* st) const { if (st) *st = MS::kSuccess; return true; }
MStatus MDGContext::getTime(MTime& t) const { t = time_; return MS::kSuccess; }
MTime MDGContext::getTime(MStatus* st) const { if (st) *st = MS::kSuccess; return time_; }

// ---------------------------------------------------------------------------------------------------------------------
// MDataHandle

MDataHandle::MDataHandle() : azBlock(0), azElement(-1) {}
MDataHandle::MDataHandle(const MDataHandle& o) : azBlock(o.azBlock), azSpec(o.azSpec), azElement(o.azElement) {}
MDataHandle& MDataHandle::operator=(const MDataHandle& o) {
    azBlock = o.azBlock;
    azSpec = o.azSpec;
    azElement = o.azElement;
    return *this;
}

static bool isWeights(const AttrSpec* s) { return s && s->builtin == "weights"; }

int MDataHandle::index(int* comp) const {
    if (comp) *comp = -1;
    if (!azBlock || !azSpec || !azBlock->block) return -1;
    return indexOf(azBlock->cls, azSpec.get(), comp);
}

// The element the host knows: deformer attributes that are arrays in the API are single plugs here.
static int hostElement(const MDataHandle& h) { return h.azSpec && !h.azSpec->builtin.empty() ? -1 : h.azElement; }

double MDataHandle::number(int component) const {
    if (isWeights(azSpec.get()) && azBlock && azBlock->block) {
        size_t n = 0;
        const double* w = host->block_weights(azBlock->block, &n);
        if (!w) return 1.0;
        return azElement >= 0 && (size_t)azElement < n ? w[azElement] : 0.0;
    }
    int c = -1;
    int i = index(&c);
    if (i < 0) return 0.0;
    double v = host->block_number(azBlock->block, i, hostElement(*this), component >= 0 ? component : c);
    return azSpec->unit == "angle" ? v / kDeg : v;
}

void MDataHandle::setNumber(double v, int component) {
    int c = -1;
    int i = index(&c);
    if (i < 0) return;
    if (azSpec->unit == "angle") v *= kDeg;
    host->block_set_number(azBlock->block, i, hostElement(*this), component >= 0 ? component : c, v);
}

void MDataHandle::setVec(double a, double b, double c) {
    int comp = -1;
    int i = index(&comp);
    if (i < 0) return;
    double v[3] = {a, b, c};
    if (azSpec->unit == "angle") for (int k = 0; k < 3; ++k) v[k] *= kDeg;
    if (comp >= 0) {
        host->block_set_number(azBlock->block, i, hostElement(*this), comp, v[0]);
        return;
    }
    host->block_set_vec3(azBlock->block, i, hostElement(*this), v);
}

static void readVec(const MDataHandle& h, int i, double out[3]) {
    out[0] = out[1] = out[2] = 0;
    if (i < 0) return;
    host->block_vec3(h.azBlock->block, i, hostElement(h), out);
    if (h.azSpec->unit == "angle") for (int k = 0; k < 3; ++k) out[k] /= kDeg;
}

bool MDataHandle::isNumeric() const { return azSpec && (azSpec->type == "float" || azSpec->type == "int" || azSpec->type == "bool" || azSpec->type == "enum"); }
bool MDataHandle::isGeneric(bool& numeric, bool& null) const {
    numeric = isNumeric();
    null = !azSpec;
    return azSpec && azSpec->fn == MFn::kGenericAttribute;
}
MFnNumericData::Type MDataHandle::numericType() const { return azSpec ? (MFnNumericData::Type)azSpec->numeric : MFnNumericData::kInvalid; }
MFnData::Type MDataHandle::type() const {
    if (!azSpec) return MFnData::kInvalid;
    if (azSpec->type == "mesh") return MFnData::kMesh;
    if (azSpec->type == "matrix") return MFnData::kMatrix;
    if (azSpec->type == "string") return MFnData::kString;
    return MFnData::kNumeric;
}
MStatus MDataHandle::acceptedTypeIds(MUintArray& out) const { out.clear(); return MS::kSuccess; }

bool& MDataHandle::asBool() const { BlockImpl& s = store(azBlock); s.b.push_back(number() != 0); return s.b.back(); }
char& MDataHandle::asChar() const { BlockImpl& s = store(azBlock); s.c.push_back((char)number()); return s.c.back(); }
unsigned char& MDataHandle::asUChar() const { BlockImpl& s = store(azBlock); s.uc.push_back((unsigned char)number()); return s.uc.back(); }
short& MDataHandle::asShort() const { BlockImpl& s = store(azBlock); s.sh.push_back((short)number()); return s.sh.back(); }
int& MDataHandle::asInt() const { BlockImpl& s = store(azBlock); s.i.push_back((int)number()); return s.i.back(); }
int& MDataHandle::asLong() const { return asInt(); }
int64_t& MDataHandle::asInt64() const { BlockImpl& s = store(azBlock); s.i64.push_back((int64_t)number()); return s.i64.back(); }
float& MDataHandle::asFloat() const { BlockImpl& s = store(azBlock); s.f.push_back((float)number()); return s.f.back(); }
double& MDataHandle::asDouble() const { BlockImpl& s = store(azBlock); s.d.push_back(number()); return s.d.back(); }
double MDataHandle::asGenericDouble() const { return number(); }
MDistance MDataHandle::asDistance() const { return MDistance(number(), MDistance::kCentimeters); }
MAngle MDataHandle::asAngle() const { return MAngle(azSpec && azSpec->unit == "angle" ? number() : number() / kDeg, MAngle::kRadians); }
MTime MDataHandle::asTime() const { return MTime(number(), MTime::uiUnit()); }

#define AZ_VEC_AS(Method, Store, Type, Elem, N)                                                                        \
    Type& MDataHandle::Method() const {                                                                                \
        BlockImpl& s = store(azBlock);                                                                                 \
        double v[3];                                                                                                   \
        int comp = -1;                                                                                                 \
        readVec(*this, index(&comp), v);                                                                               \
        s.Store.push_back(BlockImpl::Elem());                                                                          \
        for (int k = 0; k < N; ++k) s.Store.back().v[k] = v[k];                                                        \
        return s.Store.back().v;                                                                                       \
    }
AZ_VEC_AS(asShort2, s2, short2, S2, 2)
AZ_VEC_AS(asInt2, i2, int2, I2, 2)
AZ_VEC_AS(asLong2, i2, int2, I2, 2)
AZ_VEC_AS(asFloat2, f2, float2, F2, 2)
AZ_VEC_AS(asDouble2, d2, double2, D2, 2)
AZ_VEC_AS(asShort3, s3, short3, S3, 3)
AZ_VEC_AS(asInt3, i3, int3, I3, 3)
AZ_VEC_AS(asLong3, i3, int3, I3, 3)
AZ_VEC_AS(asFloat3, f3, float3, F3, 3)
AZ_VEC_AS(asDouble3, d3, double3, D3, 3)

MVector& MDataHandle::asVector() const {
    BlockImpl& s = store(azBlock);
    double v[3];
    readVec(*this, index(0), v);
    s.vec.push_back(MVector(v));
    return s.vec.back();
}
MFloatVector& MDataHandle::asFloatVector() const {
    BlockImpl& s = store(azBlock);
    s.fvec.push_back(MFloatVector(asVector()));
    return s.fvec.back();
}
MMatrix& MDataHandle::asMatrix() const {
    BlockImpl& s = store(azBlock);
    s.mat.push_back(MMatrix());
    int i = index(0);
    if (i >= 0) {
        double m[16];
        host->block_matrix(azBlock->block, i, hostElement(*this), m);
        for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) s.mat.back().matrix[r][c] = m[r * 4 + c];
    }
    return s.mat.back();
}
MFloatMatrix& MDataHandle::asFloatMatrix() const {
    BlockImpl& s = store(azBlock);
    s.fmat.push_back(MFloatMatrix(asMatrix()));
    return s.fmat.back();
}
MString& MDataHandle::asString() const {
    BlockImpl& s = store(azBlock);
    int i = index(0);
    s.str.push_back(i >= 0 ? MString(host->block_string(azBlock->block, i, hostElement(*this))) : MString());
    return s.str.back();
}
MObject MDataHandle::asMesh() const {
    int i = index(0);
    if (i < 0) return MObject();
    std::pair<int, int> key(i, hostElement(*this));
    std::map<std::pair<int, int>, MObject>::iterator it = azBlock->meshes.find(key);
    if (it != azBlock->meshes.end()) return it->second;
    AzMesh* h = host->block_mesh(azBlock->block, i, key.second);
    if (!h) return MObject();
    MObject o = meshObject(h);
    azBlock->meshes[key] = o;
    return o;
}
MObject MDataHandle::asMeshTransformed() const { return asMesh(); }
MObject MDataHandle::asNurbsCurve() const {
    int i = index(0);
    if (i < 0 || !host || host->size < offsetof(AzHost, block_set_curve) + sizeof(host->block_set_curve)) return MObject();
    AzCurve* h = host->block_curve(azBlock->block, i, hostElement(*this));
    return h ? curveObject(h) : MObject();
}
MObject MDataHandle::asNurbsCurveTransformed() const { return asNurbsCurve(); }
MMatrix MDataHandle::geometryTransformMatrix() const { return MMatrix(); }

MObject MDataHandle::data() const {
    if (!azSpec) return MObject();
    if (azSpec->type == "mesh") return asMesh();
    if (azSpec->type == "curve") return asNurbsCurve();
    if (azSpec->type == "matrix") {
        MFnMatrixData f;
        return f.create(asMatrix());
    }
    if (azSpec->type == "string") {
        MFnStringData f;
        return f.create(asString());
    }
    MFnNumericData f;
    if (azSpec->type == "vec3") {
        MObject o = f.create(MFnNumericData::k3Double);
        double v[3];
        readVec(*this, index(0), v);
        f.setData(v[0], v[1], v[2]);
        return o;
    }
    MObject o = f.create(MFnNumericData::kDouble);
    o.azData()->nums.assign(1, number());
    return o;
}

MStatus MDataHandle::setData(const MObject& d) { return set(d); }

MDataHandle MDataHandle::child(const MObject& attr) {
    MDataHandle h(*this);
    const std::shared_ptr<ObjectData>& d = attr.azData();
    h.azSpec = d ? d->attr : std::shared_ptr<AttrSpec>();
    return h;
}
MDataHandle MDataHandle::child(const MPlug& plug) {
    MDataHandle h(*this);
    h.azSpec = plug.azSpec();
    return h;
}

void MDataHandle::set(bool v) { setNumber(v ? 1 : 0); }
void MDataHandle::set(char v) { setNumber(v); }
void MDataHandle::set(short v) { setNumber(v); }
void MDataHandle::set(int v) { setNumber(v); }
void MDataHandle::set(int64_t v) { setNumber((double)v); }
void MDataHandle::set(float v) { setNumber(v); }
void MDataHandle::set(double v) { setNumber(v); }
void MDataHandle::set(const MMatrix& m) {
    int i = index(0);
    if (i < 0) return;
    double v[16];
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) v[r * 4 + c] = m[r][c];
    host->block_set_matrix(azBlock->block, i, hostElement(*this), v);
}
void MDataHandle::set(const MFloatMatrix& m) { set(MMatrix(m)); }
void MDataHandle::set(const MVector& v) { setVec(v.x, v.y, v.z); }
void MDataHandle::set(const MFloatVector& v) { setVec(v.x, v.y, v.z); }
void MDataHandle::set(const MDistance& v) { setNumber(v.asCentimeters()); }
void MDataHandle::set(const MAngle& v) { setNumber(azSpec && azSpec->unit == "angle" ? v.asRadians() : v.asDegrees()); }
void MDataHandle::set(const MTime& v) { setNumber(v.as(MTime::uiUnit())); }
void MDataHandle::set(short a, short b) { setVec(a, b, 0); }
void MDataHandle::set(int a, int b) { setVec(a, b, 0); }
void MDataHandle::set(float a, float b) { setVec(a, b, 0); }
void MDataHandle::set(double a, double b) { setVec(a, b, 0); }
void MDataHandle::set(short a, short b, short c) { setVec(a, b, c); }
void MDataHandle::set(int a, int b, int c) { setVec(a, b, c); }
void MDataHandle::set(float a, float b, float c) { setVec(a, b, c); }
void MDataHandle::set(double a, double b, double c) { setVec(a, b, c); }
void MDataHandle::set(const MString& v) {
    int i = index(0);
    if (i >= 0) host->block_set_string(azBlock->block, i, hostElement(*this), v.asChar());
}
MStatus MDataHandle::set(const MObject& data) {
    const std::shared_ptr<ObjectData>& d = data.azData();
    int i = index(0);
    if (i < 0) return MS::kInvalidParameter;
    if (!d) {
        if (azSpec->type == "mesh") host->block_set_mesh(azBlock->block, i, hostElement(*this), 0);
        return MS::kSuccess;
    }
    switch (d->type) {
    case MFn::kMeshData: {
        AzMesh* h = d->mesh ? d->mesh->commit() : 0;
        host->block_set_mesh(azBlock->block, i, hostElement(*this), h);
        azBlock->meshes[std::make_pair(i, hostElement(*this))] = data;
        return MS::kSuccess;
    }
    case MFn::kMatrixData: {
        MFnMatrixData f(data);
        set(f.matrix());
        return MS::kSuccess;
    }
    case MFn::kStringData: set(MString(d->str)); return MS::kSuccess;
    case MFn::kNumericData:
        if (d->nums.size() >= 2) setVec(d->nums[0], d->nums[1], d->nums.size() > 2 ? d->nums[2] : 0.0);
        else if (!d->nums.empty()) setNumber(d->nums[0]);
        return MS::kSuccess;
    default: return MS::kInvalidParameter;
    }
}
void MDataHandle::setBool(bool v) { set(v); }
void MDataHandle::setChar(char v) { set(v); }
void MDataHandle::setShort(short v) { set(v); }
void MDataHandle::setInt(int v) { set(v); }
void MDataHandle::setInt64(int64_t v) { set(v); }
void MDataHandle::setFloat(float v) { set(v); }
void MDataHandle::setDouble(double v) { set(v); }
void MDataHandle::setMMatrix(const MMatrix& v) { set(v); }
void MDataHandle::setMFloatMatrix(const MFloatMatrix& v) { set(v); }
void MDataHandle::setMVector(const MVector& v) { set(v); }
void MDataHandle::setMFloatVector(const MFloatVector& v) { set(v); }
void MDataHandle::setMDistance(const MDistance& v) { set(v); }
void MDataHandle::setMAngle(const MAngle& v) { set(v); }
void MDataHandle::setMTime(const MTime& v) { set(v); }
void MDataHandle::set2Short(short a, short b) { set(a, b); }
void MDataHandle::set2Int(int a, int b) { set(a, b); }
void MDataHandle::set2Float(float a, float b) { set(a, b); }
void MDataHandle::set2Double(double a, double b) { set(a, b); }
void MDataHandle::set3Short(short a, short b, short c) { set(a, b, c); }
void MDataHandle::set3Int(int a, int b, int c) { set(a, b, c); }
void MDataHandle::set3Float(float a, float b, float c) { set(a, b, c); }
void MDataHandle::set3Double(double a, double b, double c) { set(a, b, c); }
void MDataHandle::setString(const MString& v) { set(v); }

MStatus MDataHandle::copy(const MDataHandle& src) {
    if (!azSpec || !src.azSpec) return MS::kInvalidParameter;
    const std::string& t = azSpec->type;
    if (t == "mesh") return set(src.asMesh());
    if (t == "matrix") { set(src.asMatrix()); return MS::kSuccess; }
    if (t == "string") { set(src.asString()); return MS::kSuccess; }
    if (t == "vec3") { MVector v = src.asVector(); setVec(v.x, v.y, v.z); return MS::kSuccess; }
    setNumber(src.number());
    return MS::kSuccess;
}
MStatus MDataHandle::copyWritable(const MDataHandle& src) { return copy(src); }
void MDataHandle::setClean() {}

// ---------------------------------------------------------------------------------------------------------------------
// Arrays

// The elements an array attribute has: an array compound's are those any of its children have.
static std::vector<int> elementIds(BlockImpl* b, const std::shared_ptr<AttrSpec>& spec) {
    std::vector<int> out;
    if (!b || !b->block || !spec) return out;
    if (spec->builtin == "input" || spec->builtin == "output" || spec->builtin == "weightList") {
        out.push_back(0);
        return out;
    }
    if (spec->type == "compound") {
        std::set<int> all;
        for (size_t k = 0; k < spec->children.size(); ++k) {
            std::vector<int> ids = elementIds(b, spec->children[k]);
            all.insert(ids.begin(), ids.end());
        }
        out.assign(all.begin(), all.end());
        return out;
    }
    int i = indexOf(b->cls, spec.get(), 0);
    if (i < 0) return out;
    int n = host->block_element_count(b->block, i);
    for (int k = 0; k < n; ++k) out.push_back(host->block_element_at(b->block, i, k));
    return out;
}

MArrayDataHandle::MArrayDataHandle() : azBlock(0), azAt(0) {}
MArrayDataHandle::MArrayDataHandle(const MDataHandle& in, MStatus* st) : azBlock(in.azBlock), azSpec(in.azSpec), azAt(0) {
    if (isWeights(azSpec.get()) && azBlock && azBlock->block) {
        size_t n = 0;
        if (host->block_weights(azBlock->block, &n))
            for (size_t i = 0; i < n; ++i) azIds.push_back((int)i);
    } else {
        azIds = elementIds(azBlock, azSpec);
    }
    if (st) *st = azSpec ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MArrayDataHandle::MArrayDataHandle(const MArrayDataHandle& o) : azBlock(o.azBlock), azSpec(o.azSpec), azIds(o.azIds), azAt(o.azAt) {}
MArrayDataHandle& MArrayDataHandle::operator=(const MArrayDataHandle& o) {
    azBlock = o.azBlock;
    azSpec = o.azSpec;
    azIds = o.azIds;
    azAt = o.azAt;
    return *this;
}
MDataHandle MArrayDataHandle::inputValue(MStatus* st) {
    MDataHandle h;
    h.azBlock = azBlock;
    h.azSpec = azSpec;
    h.azElement = azAt < azIds.size() ? azIds[azAt] : -1;
    if (st) *st = azAt < azIds.size() ? MStatus(MS::kSuccess) : MStatus(MS::kFailure);
    return h;
}
MDataHandle MArrayDataHandle::outputValue(MStatus* st) { return inputValue(st); }
MArrayDataHandle MArrayDataHandle::inputArrayValue(MStatus* st) { return MArrayDataHandle(inputValue(st)); }
MArrayDataHandle MArrayDataHandle::outputArrayValue(MStatus* st) { return inputArrayValue(st); }
MStatus MArrayDataHandle::next() {
    ++azAt;
    return azAt < azIds.size() ? MStatus(MS::kSuccess) : MStatus(MS::kFailure);
}
unsigned int MArrayDataHandle::elementCount(MStatus* st) { if (st) *st = MS::kSuccess; return (unsigned int)azIds.size(); }
unsigned int MArrayDataHandle::elementIndex(MStatus* st) {
    if (azAt >= azIds.size()) {
        if (st) *st = MS::kFailure;
        return 0;
    }
    if (st) *st = MS::kSuccess;
    return (unsigned int)azIds[azAt];
}
MStatus MArrayDataHandle::jumpToElement(unsigned int index) {
    for (size_t k = 0; k < azIds.size(); ++k)
        if (azIds[k] == (int)index) {
            azAt = k;
            return MS::kSuccess;
        }
    // Not there yet: reading gives the default, writing makes it.
    azIds.push_back((int)index);
    std::sort(azIds.begin(), azIds.end());
    return jumpToElement(index);
}
MStatus MArrayDataHandle::jumpToLogicalElement(unsigned int index) { return jumpToElement(index); }
MStatus MArrayDataHandle::jumpToArrayElement(unsigned int pos) {
    if (pos >= azIds.size()) return MS::kInvalidParameter;
    azAt = pos;
    return MS::kSuccess;
}
MStatus MArrayDataHandle::jumpToPhysicalElement(unsigned int pos) { return jumpToArrayElement(pos); }
MStatus MArrayDataHandle::setClean() { return MS::kSuccess; }
MStatus MArrayDataHandle::setAllClean() { return MS::kSuccess; }
MArrayDataBuilder MArrayDataHandle::builder(MStatus* st) {
    MArrayDataBuilder b;
    b.azBlock = azBlock;
    b.azSpec = azSpec;
    b.azAdded = azIds;
    if (st) *st = MS::kSuccess;
    return b;
}
MStatus MArrayDataHandle::set(const MArrayDataBuilder& b) {
    azIds = b.azAdded;
    return MS::kSuccess;
}

MArrayDataBuilder::MArrayDataBuilder() : azBlock(0) {}
MArrayDataBuilder::MArrayDataBuilder(const MObject& attr, unsigned int, MStatus* st) : azBlock(currentBlock()) {
    azSpec = attr.azData() ? attr.azData()->attr : std::shared_ptr<AttrSpec>();
    if (st) *st = azSpec ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MArrayDataBuilder::MArrayDataBuilder(MDataBlock* block, const MObject& attr, unsigned int, MStatus* st) : azBlock(block ? block->azBlock() : 0) {
    azSpec = attr.azData() ? attr.azData()->attr : std::shared_ptr<AttrSpec>();
    if (st) *st = azSpec ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MArrayDataBuilder::MArrayDataBuilder(const MArrayDataBuilder& o) : azBlock(o.azBlock), azSpec(o.azSpec), azAdded(o.azAdded) {}
MArrayDataBuilder& MArrayDataBuilder::operator=(const MArrayDataBuilder& o) {
    azBlock = o.azBlock;
    azSpec = o.azSpec;
    azAdded = o.azAdded;
    return *this;
}
MDataHandle MArrayDataBuilder::addElement(unsigned int index, MStatus* st) {
    if (std::find(azAdded.begin(), azAdded.end(), (int)index) == azAdded.end()) {
        azAdded.push_back((int)index);
        std::sort(azAdded.begin(), azAdded.end());
    }
    MDataHandle h;
    h.azBlock = azBlock;
    h.azSpec = azSpec;
    h.azElement = (int)index;
    if (st) *st = MS::kSuccess;
    return h;
}
MDataHandle MArrayDataBuilder::addLast(MStatus* st) { return addElement(azAdded.empty() ? 0 : (unsigned int)azAdded.back() + 1, st); }
MStatus MArrayDataBuilder::removeElement(unsigned int index) {
    std::vector<int>::iterator it = std::find(azAdded.begin(), azAdded.end(), (int)index);
    if (it == azAdded.end()) return MS::kInvalidParameter;
    azAdded.erase(it);
    return MS::kSuccess;
}
unsigned int MArrayDataBuilder::elementCount(MStatus* st) const { if (st) *st = MS::kSuccess; return (unsigned int)azAdded.size(); }
MStatus MArrayDataBuilder::growArray(unsigned int) { return MS::kSuccess; }
MStatus MArrayDataBuilder::setGrowSize(unsigned int) { return MS::kSuccess; }

// ---------------------------------------------------------------------------------------------------------------------
// MDataBlock

static MDataHandle handleOf(BlockImpl* b, const std::shared_ptr<AttrSpec>& spec, int element, MStatus* st) {
    MDataHandle h;
    h.azBlock = b;
    h.azSpec = spec;
    h.azElement = element;
    bool ok = spec && (isWeights(spec.get()) || spec->type == "compound" || spec->builtin == "input" || indexOf(b ? b->cls : 0, spec.get(), 0) >= 0);
    if (st) *st = ok ? MStatus(MS::kSuccess) : fail(MStatus::kInvalidParameter, std::string("(kInvalidParameter): the attribute ") + (spec ? spec->name : std::string("?")) + " is not on this node");
    return h;
}

static std::shared_ptr<AttrSpec> specOf(const MObject& o) { return o.azData() ? o.azData()->attr : std::shared_ptr<AttrSpec>(); }

MDataHandle MDataBlock::inputValue(const MPlug& p, MStatus* st) { return handleOf(b_, p.azSpec(), p.azElement(), st); }
MDataHandle MDataBlock::inputValue(const MObject& a, MStatus* st) { return handleOf(b_, specOf(a), -1, st); }
MDataHandle MDataBlock::outputValue(const MPlug& p, MStatus* st) { return handleOf(b_, p.azSpec(), p.azElement(), st); }
MDataHandle MDataBlock::outputValue(const MObject& a, MStatus* st) { return handleOf(b_, specOf(a), -1, st); }
MArrayDataHandle MDataBlock::inputArrayValue(const MPlug& p, MStatus* st) { return MArrayDataHandle(handleOf(b_, p.azSpec(), -1, st)); }
MArrayDataHandle MDataBlock::inputArrayValue(const MObject& a, MStatus* st) { return MArrayDataHandle(handleOf(b_, specOf(a), -1, st)); }
MArrayDataHandle MDataBlock::outputArrayValue(const MPlug& p, MStatus* st) { return MArrayDataHandle(handleOf(b_, p.azSpec(), -1, st)); }
MArrayDataHandle MDataBlock::outputArrayValue(const MObject& a, MStatus* st) { return MArrayDataHandle(handleOf(b_, specOf(a), -1, st)); }
MStatus MDataBlock::setClean(const MPlug&) { return MS::kSuccess; }
MStatus MDataBlock::setClean(const MObject&) { return MS::kSuccess; }
bool MDataBlock::isClean(const MPlug&, MStatus* st) { if (st) *st = MS::kSuccess; return false; }
bool MDataBlock::isClean(const MObject&, MStatus* st) { if (st) *st = MS::kSuccess; return false; }
MDGContext MDataBlock::context(MStatus* st) {
    if (st) *st = MS::kSuccess;
    if (b_ && b_->block) {
        double secs = host->block_time(b_->block);
        MTime t(secs, MTime::kSeconds);
        MDGContext c;
        (void)t;
        return c;
    }
    return MDGContext();
}
