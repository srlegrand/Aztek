// Aztek plug-in kit: attribute and data function sets.
#include "internal.h"

#include <cmath>

using namespace az;

// ---------------------------------------------------------------------------------------------------------------------
// MFnBase

MFnBase::MFnBase() {}
MFnBase::~MFnBase() {}
MFn::Type MFnBase::type() const { return MFn::kBase; }
bool MFnBase::hasObj(MFn::Type t) const { return obj_.hasFn(t); }
bool MFnBase::hasObj(const MObject& o) const { return !o.isNull(); }
MObject MFnBase::object(MStatus* st) const {
    if (st) *st = obj_.isNull() ? MStatus(MS::kFailure) : MStatus(MS::kSuccess);
    return obj_;
}
MStatus MFnBase::setObject(const MObject& o) {
    obj_ = o;
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// Data

static MObject newData(MFn::Type t) {
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = t;
    return MObject(d);
}

static ObjectData* dataOf(const MObject& o, MFn::Type t) {
    const std::shared_ptr<ObjectData>& d = o.azData();
    return d && d->type == t ? d.get() : 0;
}

MFnNumericData::MFnNumericData() {}
MFnNumericData::MFnNumericData(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = dataOf(o, MFn::kNumericData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MFn::Type MFnNumericData::type() const { return MFn::kNumericData; }
MObject MFnNumericData::create(Type t, MStatus* st) {
    obj_ = newData(MFn::kNumericData);
    ObjectData* d = obj_.azData().get();
    d->numericType = t;
    d->nums.assign(t == k4Double ? 4 : (t == k2Short || t == k2Long || t == k2Float || t == k2Double) ? 2 : (t == k3Short || t == k3Long || t == k3Float || t == k3Double) ? 3 : 1, 0.0);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MFnNumericData::Type MFnNumericData::numericType(MStatus* st) {
    ObjectData* d = dataOf(obj_, MFn::kNumericData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (Type)d->numericType : kInvalid;
}

template <typename T>
static MStatus getN(const MObject& o, T* out[], int n) {
    ObjectData* d = dataOf(o, MFn::kNumericData);
    if (!d) return MS::kInvalidParameter;
    for (int i = 0; i < n; ++i) *out[i] = (T)(i < (int)d->nums.size() ? d->nums[i] : 0.0);
    return MS::kSuccess;
}
static MStatus setN(const MObject& o, const double* v, int n) {
    ObjectData* d = dataOf(o, MFn::kNumericData);
    if (!d) return MS::kInvalidParameter;
    d->nums.assign(v, v + n);
    return MS::kSuccess;
}

MStatus MFnNumericData::getData(int& a, int& b) { int* o[] = {&a, &b}; return getN(obj_, o, 2); }
MStatus MFnNumericData::getData(int& a, int& b, int& c) { int* o[] = {&a, &b, &c}; return getN(obj_, o, 3); }
MStatus MFnNumericData::getData(float& a, float& b) { float* o[] = {&a, &b}; return getN(obj_, o, 2); }
MStatus MFnNumericData::getData(float& a, float& b, float& c) { float* o[] = {&a, &b, &c}; return getN(obj_, o, 3); }
MStatus MFnNumericData::getData(double& a, double& b) { double* o[] = {&a, &b}; return getN(obj_, o, 2); }
MStatus MFnNumericData::getData(double& a, double& b, double& c) { double* o[] = {&a, &b, &c}; return getN(obj_, o, 3); }
MStatus MFnNumericData::getData(double& a, double& b, double& c, double& e) { double* o[] = {&a, &b, &c, &e}; return getN(obj_, o, 4); }
MStatus MFnNumericData::setData(int a, int b) { double v[] = {(double)a, (double)b}; return setN(obj_, v, 2); }
MStatus MFnNumericData::setData(int a, int b, int c) { double v[] = {(double)a, (double)b, (double)c}; return setN(obj_, v, 3); }
MStatus MFnNumericData::setData(float a, float b) { double v[] = {a, b}; return setN(obj_, v, 2); }
MStatus MFnNumericData::setData(float a, float b, float c) { double v[] = {a, b, c}; return setN(obj_, v, 3); }
MStatus MFnNumericData::setData(double a, double b) { double v[] = {a, b}; return setN(obj_, v, 2); }
MStatus MFnNumericData::setData(double a, double b, double c) { double v[] = {a, b, c}; return setN(obj_, v, 3); }
MStatus MFnNumericData::setData(double a, double b, double c, double e) { double v[] = {a, b, c, e}; return setN(obj_, v, 4); }
MStatus MFnNumericData::getData2Short(short& a, short& b) { short* o[] = {&a, &b}; return getN(obj_, o, 2); }
MStatus MFnNumericData::getData2Int(int& a, int& b) { return getData(a, b); }
MStatus MFnNumericData::getData2Float(float& a, float& b) { return getData(a, b); }
MStatus MFnNumericData::getData2Double(double& a, double& b) { return getData(a, b); }
MStatus MFnNumericData::getData3Short(short& a, short& b, short& c) { short* o[] = {&a, &b, &c}; return getN(obj_, o, 3); }
MStatus MFnNumericData::getData3Int(int& a, int& b, int& c) { return getData(a, b, c); }
MStatus MFnNumericData::getData3Float(float& a, float& b, float& c) { return getData(a, b, c); }
MStatus MFnNumericData::getData3Double(double& a, double& b, double& c) { return getData(a, b, c); }
MStatus MFnNumericData::getData4Double(double& a, double& b, double& c, double& d) { return getData(a, b, c, d); }
MStatus MFnNumericData::setData2Short(short a, short b) { double v[] = {(double)a, (double)b}; return setN(obj_, v, 2); }
MStatus MFnNumericData::setData2Int(int a, int b) { return setData(a, b); }
MStatus MFnNumericData::setData2Float(float a, float b) { return setData(a, b); }
MStatus MFnNumericData::setData2Double(double a, double b) { return setData(a, b); }
MStatus MFnNumericData::setData3Short(short a, short b, short c) { double v[] = {(double)a, (double)b, (double)c}; return setN(obj_, v, 3); }
MStatus MFnNumericData::setData3Int(int a, int b, int c) { return setData(a, b, c); }
MStatus MFnNumericData::setData3Float(float a, float b, float c) { return setData(a, b, c); }
MStatus MFnNumericData::setData3Double(double a, double b, double c) { return setData(a, b, c); }
MStatus MFnNumericData::setData4Double(double a, double b, double c, double d) { return setData(a, b, c, d); }

MFnMatrixData::MFnMatrixData() {}
MFnMatrixData::MFnMatrixData(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = dataOf(o, MFn::kMatrixData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MFn::Type MFnMatrixData::type() const { return MFn::kMatrixData; }
MObject MFnMatrixData::create(const MMatrix& m, MStatus* st) {
    obj_ = newData(MFn::kMatrixData);
    set(m);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnMatrixData::create(MStatus* st) { return create(MMatrix(), st); }
const MMatrix& MFnMatrixData::matrix(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kMatrixData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    cache_.setToIdentity();
    if (d && d->nums.size() == 16)
        for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) cache_.matrix[r][c] = d->nums[r * 4 + c];
    return cache_;
}
MStatus MFnMatrixData::set(const MMatrix& m) {
    ObjectData* d = dataOf(obj_, MFn::kMatrixData);
    if (!d) return MS::kInvalidParameter;
    d->nums.resize(16);
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) d->nums[r * 4 + c] = m[r][c];
    return MS::kSuccess;
}
bool MFnMatrixData::isTransformation(MStatus* st) const { if (st) *st = MS::kSuccess; return false; }
MTransformationMatrix MFnMatrixData::transformation(MStatus* st) const { return MTransformationMatrix(matrix(st)); }

MFnStringData::MFnStringData() {}
MFnStringData::MFnStringData(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = dataOf(o, MFn::kStringData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MFn::Type MFnStringData::type() const { return MFn::kStringData; }
MObject MFnStringData::create(const MString& s, MStatus* st) {
    obj_ = newData(MFn::kStringData);
    obj_.azData()->str = s.asChar();
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnStringData::create(MStatus* st) { return create(MString(), st); }
MString MFnStringData::string(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kStringData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? MString(d->str) : MString();
}
MStatus MFnStringData::set(const MString& s) {
    ObjectData* d = dataOf(obj_, MFn::kStringData);
    if (!d) return MS::kInvalidParameter;
    d->str = s.asChar();
    return MS::kSuccess;
}

// Array data: numbers in nums (a point's four, a vector's three), strings in strs.

#define AZ_ARRAY_DATA_COMMON(Fn, Kind)                                                                                   \
    Fn::Fn() {}                                                                                                          \
    Fn::Fn(const MObject& o, MStatus* st) {                                                                              \
        obj_ = o;                                                                                                        \
        if (st) *st = dataOf(o, Kind) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);                          \
    }                                                                                                                    \
    MFn::Type Fn::type() const { return Kind; }

AZ_ARRAY_DATA_COMMON(MFnDoubleArrayData, MFn::kDoubleArrayData)
MObject MFnDoubleArrayData::create(const MDoubleArray& v, MStatus* st) {
    obj_ = newData(MFn::kDoubleArrayData);
    set(v);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnDoubleArrayData::create(MStatus* st) { return create(MDoubleArray(), st); }
MDoubleArray MFnDoubleArrayData::array(MStatus* st) {
    MDoubleArray out;
    copyTo(out);
    if (st) *st = dataOf(obj_, MFn::kDoubleArrayData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return out;
}
unsigned int MFnDoubleArrayData::length(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kDoubleArrayData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (unsigned int)d->nums.size() : 0;
}
double MFnDoubleArrayData::operator[](unsigned int i) const {
    ObjectData* d = dataOf(obj_, MFn::kDoubleArrayData);
    return d && i < d->nums.size() ? d->nums[i] : 0.0;
}
MStatus MFnDoubleArrayData::set(const MDoubleArray& v) {
    ObjectData* d = dataOf(obj_, MFn::kDoubleArrayData);
    if (!d) return MS::kInvalidParameter;
    d->nums.clear();
    for (unsigned int i = 0; i < v.length(); ++i) d->nums.push_back(v[i]);
    return MS::kSuccess;
}
MStatus MFnDoubleArrayData::copyTo(MDoubleArray& out) const {
    ObjectData* d = dataOf(obj_, MFn::kDoubleArrayData);
    out.clear();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->nums.size(); ++i) out.append(d->nums[i]);
    return MS::kSuccess;
}

AZ_ARRAY_DATA_COMMON(MFnFloatArrayData, MFn::kFloatArrayData)
MObject MFnFloatArrayData::create(const MFloatArray& v, MStatus* st) {
    obj_ = newData(MFn::kFloatArrayData);
    set(v);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnFloatArrayData::create(MStatus* st) { return create(MFloatArray(), st); }
MFloatArray MFnFloatArrayData::array(MStatus* st) {
    MFloatArray out;
    copyTo(out);
    if (st) *st = dataOf(obj_, MFn::kFloatArrayData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return out;
}
unsigned int MFnFloatArrayData::length(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kFloatArrayData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (unsigned int)d->nums.size() : 0;
}
MStatus MFnFloatArrayData::set(const MFloatArray& v) {
    ObjectData* d = dataOf(obj_, MFn::kFloatArrayData);
    if (!d) return MS::kInvalidParameter;
    d->nums.clear();
    for (unsigned int i = 0; i < v.length(); ++i) d->nums.push_back(v[i]);
    return MS::kSuccess;
}
MStatus MFnFloatArrayData::copyTo(MFloatArray& out) const {
    ObjectData* d = dataOf(obj_, MFn::kFloatArrayData);
    out.clear();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->nums.size(); ++i) out.append((float)d->nums[i]);
    return MS::kSuccess;
}

AZ_ARRAY_DATA_COMMON(MFnIntArrayData, MFn::kIntArrayData)
MObject MFnIntArrayData::create(const MIntArray& v, MStatus* st) {
    obj_ = newData(MFn::kIntArrayData);
    set(v);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnIntArrayData::create(MStatus* st) { return create(MIntArray(), st); }
MIntArray MFnIntArrayData::array(MStatus* st) {
    MIntArray out;
    copyTo(out);
    if (st) *st = dataOf(obj_, MFn::kIntArrayData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return out;
}
unsigned int MFnIntArrayData::length(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kIntArrayData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (unsigned int)d->nums.size() : 0;
}
int MFnIntArrayData::operator[](unsigned int i) const {
    ObjectData* d = dataOf(obj_, MFn::kIntArrayData);
    return d && i < d->nums.size() ? (int)d->nums[i] : 0;
}
MStatus MFnIntArrayData::set(const MIntArray& v) {
    ObjectData* d = dataOf(obj_, MFn::kIntArrayData);
    if (!d) return MS::kInvalidParameter;
    d->nums.clear();
    for (unsigned int i = 0; i < v.length(); ++i) d->nums.push_back(v[i]);
    return MS::kSuccess;
}
MStatus MFnIntArrayData::copyTo(MIntArray& out) const {
    ObjectData* d = dataOf(obj_, MFn::kIntArrayData);
    out.clear();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->nums.size(); ++i) out.append((int)d->nums[i]);
    return MS::kSuccess;
}

AZ_ARRAY_DATA_COMMON(MFnStringArrayData, MFn::kStringArrayData)
MObject MFnStringArrayData::create(const MStringArray& v, MStatus* st) {
    obj_ = newData(MFn::kStringArrayData);
    set(v);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnStringArrayData::create(MStatus* st) { return create(MStringArray(), st); }
MStringArray MFnStringArrayData::array(MStatus* st) {
    MStringArray out;
    copyTo(out);
    if (st) *st = dataOf(obj_, MFn::kStringArrayData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return out;
}
unsigned int MFnStringArrayData::length(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kStringArrayData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (unsigned int)d->strs.size() : 0;
}
MStatus MFnStringArrayData::set(const MStringArray& v) {
    ObjectData* d = dataOf(obj_, MFn::kStringArrayData);
    if (!d) return MS::kInvalidParameter;
    d->strs.clear();
    for (unsigned int i = 0; i < v.length(); ++i) d->strs.push_back(v[i].asChar());
    return MS::kSuccess;
}
MStatus MFnStringArrayData::copyTo(MStringArray& out) const {
    ObjectData* d = dataOf(obj_, MFn::kStringArrayData);
    out.clear();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i < d->strs.size(); ++i) out.append(MString(d->strs[i]));
    return MS::kSuccess;
}

AZ_ARRAY_DATA_COMMON(MFnPointArrayData, MFn::kPointArrayData)
MObject MFnPointArrayData::create(const MPointArray& v, MStatus* st) {
    obj_ = newData(MFn::kPointArrayData);
    set(v);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnPointArrayData::create(MStatus* st) { return create(MPointArray(), st); }
MPointArray MFnPointArrayData::array(MStatus* st) {
    MPointArray out;
    copyTo(out);
    if (st) *st = dataOf(obj_, MFn::kPointArrayData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return out;
}
unsigned int MFnPointArrayData::length(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kPointArrayData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (unsigned int)(d->nums.size() / 4) : 0;
}
MStatus MFnPointArrayData::set(const MPointArray& v) {
    ObjectData* d = dataOf(obj_, MFn::kPointArrayData);
    if (!d) return MS::kInvalidParameter;
    d->nums.clear();
    for (unsigned int i = 0; i < v.length(); ++i) {
        d->nums.push_back(v[i].x);
        d->nums.push_back(v[i].y);
        d->nums.push_back(v[i].z);
        d->nums.push_back(v[i].w);
    }
    return MS::kSuccess;
}
MStatus MFnPointArrayData::copyTo(MPointArray& out) const {
    ObjectData* d = dataOf(obj_, MFn::kPointArrayData);
    out.clear();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i + 3 < d->nums.size(); i += 4) out.append(MPoint(d->nums[i], d->nums[i + 1], d->nums[i + 2], d->nums[i + 3]));
    return MS::kSuccess;
}

AZ_ARRAY_DATA_COMMON(MFnVectorArrayData, MFn::kVectorArrayData)
MObject MFnVectorArrayData::create(const MVectorArray& v, MStatus* st) {
    obj_ = newData(MFn::kVectorArrayData);
    set(v);
    if (st) *st = MS::kSuccess;
    return obj_;
}
MObject MFnVectorArrayData::create(MStatus* st) { return create(MVectorArray(), st); }
MVectorArray MFnVectorArrayData::array(MStatus* st) {
    MVectorArray out;
    copyTo(out);
    if (st) *st = dataOf(obj_, MFn::kVectorArrayData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return out;
}
unsigned int MFnVectorArrayData::length(MStatus* st) const {
    ObjectData* d = dataOf(obj_, MFn::kVectorArrayData);
    if (st) *st = d ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return d ? (unsigned int)(d->nums.size() / 3) : 0;
}
MStatus MFnVectorArrayData::set(const MVectorArray& v) {
    ObjectData* d = dataOf(obj_, MFn::kVectorArrayData);
    if (!d) return MS::kInvalidParameter;
    d->nums.clear();
    for (unsigned int i = 0; i < v.length(); ++i) {
        d->nums.push_back(v[i].x);
        d->nums.push_back(v[i].y);
        d->nums.push_back(v[i].z);
    }
    return MS::kSuccess;
}
MStatus MFnVectorArrayData::copyTo(MVectorArray& out) const {
    ObjectData* d = dataOf(obj_, MFn::kVectorArrayData);
    out.clear();
    if (!d) return MS::kInvalidParameter;
    for (size_t i = 0; i + 2 < d->nums.size(); i += 3) out.append(MVector(d->nums[i], d->nums[i + 1], d->nums[i + 2]));
    return MS::kSuccess;
}

MFnGeometryData::MFnGeometryData() {}
MFnGeometryData::MFnGeometryData(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = o.hasFn(MFn::kGeometryData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MMatrix MFnGeometryData::matrix(MStatus* st) const { if (st) *st = MS::kSuccess; return MMatrix(); }
MStatus MFnGeometryData::setMatrix(const MMatrix&) { return MS::kSuccess; }
bool MFnGeometryData::isIdentity(MStatus* st) const { if (st) *st = MS::kSuccess; return true; }

MFnMeshData::MFnMeshData() {}
MFnMeshData::MFnMeshData(const MObject& o, MStatus* st) : MFnGeometryData(o, st) {}
MFn::Type MFnMeshData::type() const { return MFn::kMeshData; }
MObject MFnMeshData::create(MStatus* st) {
    obj_ = newData(MFn::kMeshData);
    obj_.azData()->mesh = std::make_shared<MeshData>();
    if (st) *st = MS::kSuccess;
    return obj_;
}

// ---------------------------------------------------------------------------------------------------------------------
// Attributes

MFnAttribute::MFnAttribute() {}
MFnAttribute::MFnAttribute(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = o.azData() && o.azData()->attr ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MFn::Type MFnAttribute::type() const { return MFn::kAttribute; }

AttrSpec* MFnAttribute::azSpec(MStatus* st) const {
    AttrSpec* s = obj_.azData() ? obj_.azData()->attr.get() : 0;
    if (st) *st = s ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return s;
}

MObject MFnAttribute::azNew(MFn::Type fn, const MString& longName, const MString& shortName, const char* type, MStatus* st) {
    std::shared_ptr<AttrSpec> s = std::make_shared<AttrSpec>();
    s->fn = fn;
    s->name = longName.asChar();
    s->shortName = shortName.length() ? shortName.asChar() : longName.asChar();
    s->type = type;
    obj_ = attrObject(s);
    if (st) *st = s->name.empty() ? MStatus(MS::kInvalidParameter) : MStatus(MS::kSuccess);
    return obj_;
}

#define AZ_GET(method, field)                                                                                          \
    bool MFnAttribute::method(MStatus* st) const {                                                                     \
        AttrSpec* s = azSpec(st);                                                                                      \
        return s ? s->field : false;                                                                                   \
    }
#define AZ_SET(method, field)                                                                                          \
    MStatus MFnAttribute::method(bool on) {                                                                            \
        MStatus st;                                                                                                    \
        AttrSpec* s = azSpec(&st);                                                                                     \
        if (s) s->field = on;                                                                                          \
        return st;                                                                                                     \
    }
AZ_GET(isReadable, readable)
AZ_GET(isWritable, writable)
AZ_GET(isConnectable, connectable)
AZ_GET(isStorable, storable)
AZ_GET(isCached, cached)
AZ_GET(isArray, array)
AZ_GET(isKeyable, keyable)
AZ_GET(isHidden, hidden)
AZ_GET(isChannelBoxFlagSet, channelBox)
AZ_GET(isUsedAsColor, color)
AZ_GET(usesArrayDataBuilder, builder)
AZ_SET(setReadable, readable)
AZ_SET(setWritable, writable)
AZ_SET(setConnectable, connectable)
AZ_SET(setStorable, storable)
AZ_SET(setCached, cached)
AZ_SET(setArray, array)
AZ_SET(setKeyable, keyable)
AZ_SET(setHidden, hidden)
AZ_SET(setChannelBox, channelBox)
AZ_SET(setUsedAsColor, color)
AZ_SET(setUsesArrayDataBuilder, builder)

bool MFnAttribute::isDynamic(MStatus* st) const { if (st) *st = MS::kSuccess; return false; }
MString MFnAttribute::name(MStatus* st) const { AttrSpec* s = azSpec(st); return s ? MString(s->name) : MString(); }
MString MFnAttribute::shortName(MStatus* st) const { AttrSpec* s = azSpec(st); return s ? MString(s->shortName) : MString(); }
MString MFnAttribute::niceNameOverride(MStatus* st) const { AttrSpec* s = azSpec(st); return s ? MString(s->nice) : MString(); }
MObject MFnAttribute::parent(MStatus* st) const {
    AttrSpec* s = azSpec(st);
    if (!s || !s->parent) return MObject();
    // The parent holds its children; find the shared pointer to it through a child's grandparent or the classes.
    for (size_t k = 0; k < nodeClasses().size(); ++k) {
        NodeClass* c = nodeClasses()[k];
        for (size_t i = 0; i < c->attrs.size(); ++i)
            if (c->attrs[i].get() == s->parent) return attrObject(c->attrs[i]);
    }
    return MObject();
}
MStatus MFnAttribute::setUsedAsFilename(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setIndexMatters(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setDisconnectBehavior(DisconnectBehavior) { return MS::kSuccess; }
MStatus MFnAttribute::setAffectsAppearance(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setAffectsWorldSpace(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setWorldSpace(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setInternal(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setRenderSource(bool) { return MS::kSuccess; }
MStatus MFnAttribute::setReadOnly(bool on) { return setWritable(!on); }
MStatus MFnAttribute::setNiceNameOverride(const MString& n) {
    MStatus st;
    AttrSpec* s = azSpec(&st);
    if (s) s->nice = n.asChar();
    return st;
}
MStatus MFnAttribute::addToCategory(const MString&) { return MS::kSuccess; }
MStatus MFnAttribute::setMin(double v) { MStatus st; AttrSpec* s = azSpec(&st); if (s) { s->hasMin = true; s->min = v; } return st; }
MStatus MFnAttribute::setMax(double v) { MStatus st; AttrSpec* s = azSpec(&st); if (s) { s->hasMax = true; s->max = v; } return st; }
MStatus MFnAttribute::setSoftMin(double v) { MStatus st; AttrSpec* s = azSpec(&st); if (s) { s->hasSoftMin = true; s->softMin = v; } return st; }
MStatus MFnAttribute::setSoftMax(double v) { MStatus st; AttrSpec* s = azSpec(&st); if (s) { s->hasSoftMax = true; s->softMax = v; } return st; }

// Numeric

MFnNumericAttribute::MFnNumericAttribute() {}
MFnNumericAttribute::MFnNumericAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnNumericAttribute::type() const { return MFn::kNumericAttribute; }

static int componentsOf(MFnNumericData::Type t) {
    switch (t) {
    case MFnNumericData::k2Short:
    case MFnNumericData::k2Long:
    case MFnNumericData::k2Float:
    case MFnNumericData::k2Double: return 2;
    case MFnNumericData::k3Short:
    case MFnNumericData::k3Long:
    case MFnNumericData::k3Float:
    case MFnNumericData::k3Double: return 3;
    case MFnNumericData::k4Double: return 4;
    default: return 1;
    }
}

static const char* hostTypeOf(MFnNumericData::Type t) {
    switch (t) {
    case MFnNumericData::kBoolean: return "bool";
    case MFnNumericData::kByte:
    case MFnNumericData::kChar:
    case MFnNumericData::kShort:
    case MFnNumericData::kLong:
    case MFnNumericData::kInt64:
    case MFnNumericData::kAddr: return "int";
    case MFnNumericData::kFloat:
    case MFnNumericData::kDouble: return "float";
    default: return "vec3";
    }
}

MObject MFnNumericAttribute::create(const MString& ln, const MString& sn, MFnNumericData::Type t, double def, MStatus* st) {
    if (t == MFnNumericData::kInvalid || t == MFnNumericData::kLast || t == MFnNumericData::k4Double) {
        if (st) *st = fail(MStatus::kNotImplemented, std::string("numeric attribute ") + ln.asChar() + ": that type isn't available");
        obj_ = MObject();
        return obj_;
    }
    azNew(MFn::kNumericAttribute, ln, sn, hostTypeOf(t), st);
    AttrSpec* s = azSpec();
    s->numeric = t;
    s->components = componentsOf(t);
    s->def.assign(s->components, def);
    if (t == MFnNumericData::kAddr) s->hidden = true, s->storable = false;
    return obj_;
}

MObject MFnNumericAttribute::create(const MString& ln, const MString& sn, const MObject& c1, const MObject& c2, const MObject& c3, MStatus* st) {
    const MObject* kids[3] = {&c1, &c2, &c3};
    azNew(MFn::kNumericAttribute, ln, sn, "vec3", st);
    std::shared_ptr<AttrSpec> self = obj_.azData()->attr;
    self->components = c3.isNull() ? 2 : 3;
    self->numeric = self->components == 2 ? MFnNumericData::k2Double : MFnNumericData::k3Double;
    self->def.clear();
    for (int i = 0; i < self->components; ++i) {
        const std::shared_ptr<ObjectData>& d = kids[i]->azData();
        if (!d || !d->attr) {
            if (st) *st = fail(MStatus::kInvalidParameter, std::string(ln.asChar()) + ": a child is not an attribute");
            return obj_;
        }
        d->attr->parent = self.get();
        self->children.push_back(d->attr);
        self->def.push_back(d->attr->def.empty() ? 0.0 : d->attr->def[0]);
        if (!d->attr->unit.empty()) self->unit = d->attr->unit;
    }
    return obj_;
}

MObject MFnNumericAttribute::createPoint(const MString& ln, const MString& sn, MStatus* st) {
    create(ln, sn, MFnNumericData::k3Float, 0.0, st);
    azSpec()->point = true;
    return obj_;
}
MObject MFnNumericAttribute::child(unsigned int i, MStatus* st) const {
    std::shared_ptr<AttrSpec> self = obj_.azData() ? obj_.azData()->attr : std::shared_ptr<AttrSpec>();
    if (!self || self->components < 2 || (int)i >= self->components) {
        if (st) *st = MS::kInvalidParameter;
        return MObject();
    }
    if (self->children.empty()) {
        // Named as the reference names them: X Y Z for a point, R G B for a colour, 0 1 2 otherwise.
        const char* point[3] = {"X", "Y", "Z"};
        const char* color[3] = {"R", "G", "B"};
        const char* plain[3] = {"0", "1", "2"};
        const char** suffix = self->color ? color : self->point ? point : plain;
        for (int k = 0; k < self->components; ++k) {
            std::shared_ptr<AttrSpec> c = std::make_shared<AttrSpec>();
            c->fn = MFn::kNumericAttribute;
            c->name = self->name + suffix[k];
            c->shortName = self->shortName + suffix[k];
            c->type = "float";
            c->numeric = self->numeric == MFnNumericData::k3Float || self->numeric == MFnNumericData::k2Float ? MFnNumericData::kFloat : MFnNumericData::kDouble;
            c->def.assign(1, (size_t)k < self->def.size() ? self->def[k] : 0.0);
            c->parent = self.get();
            self->children.push_back(c);
        }
    }
    if (st) *st = MS::kSuccess;
    return attrObject(self->children[i]);
}
MObject MFnNumericAttribute::createColor(const MString& ln, const MString& sn, MStatus* st) {
    create(ln, sn, MFnNumericData::k3Float, 0.0, st);
    azSpec()->color = true;
    return obj_;
}
MObject MFnNumericAttribute::createAddr(const MString& ln, const MString& sn, void*, MStatus* st) { return create(ln, sn, MFnNumericData::kAddr, 0.0, st); }
MFnNumericData::Type MFnNumericAttribute::unitType(MStatus* st) const { AttrSpec* s = azSpec(st); return s ? (MFnNumericData::Type)s->numeric : MFnNumericData::kInvalid; }
MFnNumericData::Type MFnNumericAttribute::numericType(MStatus* st) const { return unitType(st); }

static MStatus setDef(AttrSpec* s, double a, double b, double c, int n) {
    if (!s) return MS::kInvalidParameter;
    double v[3] = {a, b, c};
    s->def.assign(v, v + (n > 3 ? 3 : n));
    if (n == 1 && s->components > 1) s->def.assign(s->components, a);
    return MS::kSuccess;
}
MStatus MFnNumericAttribute::setDefault(bool v) { return setDef(azSpec(), v ? 1 : 0, 0, 0, 1); }
MStatus MFnNumericAttribute::setDefault(int v) { return setDef(azSpec(), v, 0, 0, 1); }
MStatus MFnNumericAttribute::setDefault(float v) { return setDef(azSpec(), v, 0, 0, 1); }
MStatus MFnNumericAttribute::setDefault(double v) { return setDef(azSpec(), v, 0, 0, 1); }
MStatus MFnNumericAttribute::setDefault(int a, int b) { return setDef(azSpec(), a, b, 0, 2); }
MStatus MFnNumericAttribute::setDefault(int a, int b, int c) { return setDef(azSpec(), a, b, c, 3); }
MStatus MFnNumericAttribute::setDefault(float a, float b) { return setDef(azSpec(), a, b, 0, 2); }
MStatus MFnNumericAttribute::setDefault(float a, float b, float c) { return setDef(azSpec(), a, b, c, 3); }
MStatus MFnNumericAttribute::setDefault(double a, double b) { return setDef(azSpec(), a, b, 0, 2); }
MStatus MFnNumericAttribute::setDefault(double a, double b, double c) { return setDef(azSpec(), a, b, c, 3); }
MStatus MFnNumericAttribute::getDefault(bool& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; v = s->def[0] != 0; return MS::kSuccess; }
MStatus MFnNumericAttribute::getDefault(int& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; v = (int)s->def[0]; return MS::kSuccess; }
MStatus MFnNumericAttribute::getDefault(float& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; v = (float)s->def[0]; return MS::kSuccess; }
MStatus MFnNumericAttribute::getDefault(double& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; v = s->def[0]; return MS::kSuccess; }
MStatus MFnNumericAttribute::getDefault(double& a, double& b, double& c) {
    AttrSpec* s = azSpec();
    if (!s) return MS::kInvalidParameter;
    a = s->def.size() > 0 ? s->def[0] : 0;
    b = s->def.size() > 1 ? s->def[1] : 0;
    c = s->def.size() > 2 ? s->def[2] : 0;
    return MS::kSuccess;
}
MStatus MFnNumericAttribute::setMin(int v) { return MFnAttribute::setMin(v); }
MStatus MFnNumericAttribute::setMin(float v) { return MFnAttribute::setMin(v); }
MStatus MFnNumericAttribute::setMin(double v) { return MFnAttribute::setMin(v); }
MStatus MFnNumericAttribute::setMin(double a, double, double) { return MFnAttribute::setMin(a); }
MStatus MFnNumericAttribute::setMax(int v) { return MFnAttribute::setMax(v); }
MStatus MFnNumericAttribute::setMax(float v) { return MFnAttribute::setMax(v); }
MStatus MFnNumericAttribute::setMax(double v) { return MFnAttribute::setMax(v); }
MStatus MFnNumericAttribute::setMax(double a, double, double) { return MFnAttribute::setMax(a); }
MStatus MFnNumericAttribute::setSoftMin(double v) { return MFnAttribute::setSoftMin(v); }
MStatus MFnNumericAttribute::setSoftMax(double v) { return MFnAttribute::setSoftMax(v); }
bool MFnNumericAttribute::hasMin(MStatus* st) const { AttrSpec* s = azSpec(st); return s && s->hasMin; }
bool MFnNumericAttribute::hasMax(MStatus* st) const { AttrSpec* s = azSpec(st); return s && s->hasMax; }
bool MFnNumericAttribute::hasSoftMin(MStatus* st) const { AttrSpec* s = azSpec(st); return s && s->hasSoftMin; }
bool MFnNumericAttribute::hasSoftMax(MStatus* st) const { AttrSpec* s = azSpec(st); return s && s->hasSoftMax; }
MStatus MFnNumericAttribute::getMin(double& v) const { AttrSpec* s = azSpec(); if (!s || !s->hasMin) return MS::kFailure; v = s->min; return MS::kSuccess; }
MStatus MFnNumericAttribute::getMax(double& v) const { AttrSpec* s = azSpec(); if (!s || !s->hasMax) return MS::kFailure; v = s->max; return MS::kSuccess; }
MStatus MFnNumericAttribute::getSoftMin(double& v) const { AttrSpec* s = azSpec(); if (!s || !s->hasSoftMin) return MS::kFailure; v = s->softMin; return MS::kSuccess; }
MStatus MFnNumericAttribute::getSoftMax(double& v) const { AttrSpec* s = azSpec(); if (!s || !s->hasSoftMax) return MS::kFailure; v = s->softMax; return MS::kSuccess; }

// Units. The host holds angles in radians, as the reference does inside, distances in centimetres and times in frames
// of the scene's rate. A data block reads and writes them as they are held.

MFnUnitAttribute::MFnUnitAttribute() {}
MFnUnitAttribute::MFnUnitAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnUnitAttribute::type() const { return MFn::kUnitAttribute; }
MObject MFnUnitAttribute::create(const MString& ln, const MString& sn, Type t, double def, MStatus* st) {
    azNew(t == kAngle ? MFn::kDoubleAngleAttribute : t == kDistance ? MFn::kDoubleLinearAttribute : MFn::kTimeAttribute, ln, sn, "float", st);
    AttrSpec* s = azSpec();
    s->unit = t == kAngle ? "angle" : t == kDistance ? "distance" : "time";
    s->numeric = MFnNumericData::kDouble;
    s->def.assign(1, def);
    return obj_;
}
MObject MFnUnitAttribute::create(const MString& ln, const MString& sn, const MAngle& d, MStatus* st) { return create(ln, sn, kAngle, d.asRadians(), st); }
MObject MFnUnitAttribute::create(const MString& ln, const MString& sn, const MDistance& d, MStatus* st) { return create(ln, sn, kDistance, d.asCentimeters(), st); }
MObject MFnUnitAttribute::create(const MString& ln, const MString& sn, const MTime& d, MStatus* st) { return create(ln, sn, kTime, d.as(MTime::uiUnit()), st); }
MFnUnitAttribute::Type MFnUnitAttribute::unitType(MStatus* st) const {
    AttrSpec* s = azSpec(st);
    if (!s) return kInvalid;
    return s->unit == "angle" ? kAngle : s->unit == "distance" ? kDistance : s->unit == "time" ? kTime : kInvalid;
}
MStatus MFnUnitAttribute::setDefault(double v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; s->def.assign(1, v); return MS::kSuccess; }
MStatus MFnUnitAttribute::setDefault(const MAngle& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; s->def.assign(1, v.asRadians()); return MS::kSuccess; }
MStatus MFnUnitAttribute::setDefault(const MDistance& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; s->def.assign(1, v.asCentimeters()); return MS::kSuccess; }
MStatus MFnUnitAttribute::setDefault(const MTime& v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; s->def.assign(1, v.as(MTime::uiUnit())); return MS::kSuccess; }
static double inHost(AttrSpec*, double v) { return v; }
MStatus MFnUnitAttribute::setMin(double v) { return MFnAttribute::setMin(inHost(azSpec(), v)); }
MStatus MFnUnitAttribute::setMax(double v) { return MFnAttribute::setMax(inHost(azSpec(), v)); }
MStatus MFnUnitAttribute::setSoftMin(double v) { return MFnAttribute::setSoftMin(inHost(azSpec(), v)); }
MStatus MFnUnitAttribute::setSoftMax(double v) { return MFnAttribute::setSoftMax(inHost(azSpec(), v)); }
bool MFnUnitAttribute::hasMin(MStatus* st) const { AttrSpec* s = azSpec(st); return s && s->hasMin; }
bool MFnUnitAttribute::hasMax(MStatus* st) const { AttrSpec* s = azSpec(st); return s && s->hasMax; }

// Typed

MFnTypedAttribute::MFnTypedAttribute() {}
MFnTypedAttribute::MFnTypedAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnTypedAttribute::type() const { return MFn::kTypedAttribute; }
MObject MFnTypedAttribute::create(const MString& ln, const MString& sn, MFnData::Type t, const MObject& def, MStatus* st) {
    const char* host = t == MFnData::kString ? "string" : t == MFnData::kMatrix ? "matrix" : t == MFnData::kMesh ? "mesh" : t == MFnData::kNurbsCurve ? "curve"
                     : t == MFnData::kDoubleArray ? "doubleArray" : t == MFnData::kFloatArray ? "floatArray" : t == MFnData::kIntArray ? "intArray"
                     : t == MFnData::kStringArray ? "stringArray" : t == MFnData::kPointArray ? "pointArray" : t == MFnData::kVectorArray ? "vectorArray" : "float";
    azNew(MFn::kTypedAttribute, ln, sn, host, st);
    AttrSpec* s = azSpec();
    s->dataType = t;
    if (std::string(host) == "float") {
        // Kept so the plug-in loads; it holds nothing.
        s->hidden = true;
        s->storable = false;
        az::log(std::string("// Warning: typed attribute ") + ln.asChar() + ": data of that kind isn't available yet; it holds nothing //", 4);
    }
    if (std::string(host) != "float") s->def.clear();
    if (t == MFnData::kString && def.azData() && def.azData()->type == MFn::kStringData) s->defString = def.azData()->str;
    if (std::string(host) == "mesh" || std::string(host) == "curve") s->storable = false;
    return obj_;
}
MObject MFnTypedAttribute::create(const MString& ln, const MString& sn, MFnData::Type t, MStatus* st) { return create(ln, sn, t, MObject::kNullObj, st); }
MObject MFnTypedAttribute::create(const MString& ln, const MString& sn, const MTypeId& id, const MObject& def, MStatus* st) {
    // A plug-in data type registered before: its values flow through the attribute.
    PluginDataType* t = pluginDataTypeOfId(id);
    if (!t) return create(ln, sn, MFnData::kPlugin, def, st);
    azNew(MFn::kTypedAttribute, ln, sn, "data", st);
    AttrSpec* s = azSpec();
    s->dataType = t->type == MPxData::kGeometryData ? MFnData::kPluginGeometry : MFnData::kPlugin;
    s->pluginType = t->name;
    s->def.clear();
    return obj_;
}
MFnData::Type MFnTypedAttribute::attrType(MStatus* st) const { AttrSpec* s = azSpec(st); return s ? (MFnData::Type)s->dataType : MFnData::kInvalid; }
MStatus MFnTypedAttribute::setDefault(const MObject& data) {
    AttrSpec* s = azSpec();
    if (!s) return MS::kInvalidParameter;
    if (data.azData() && data.azData()->type == MFn::kStringData) s->defString = data.azData()->str;
    return MS::kSuccess;
}
MStatus MFnTypedAttribute::getDefault(MObject& data) {
    AttrSpec* s = azSpec();
    if (!s) return MS::kInvalidParameter;
    if (s->type == "string") {
        MFnStringData f;
        data = f.create(MString(s->defString));
    } else {
        data = MObject();
    }
    return MS::kSuccess;
}

// Enum

MFnEnumAttribute::MFnEnumAttribute() {}
MFnEnumAttribute::MFnEnumAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnEnumAttribute::type() const { return MFn::kEnumAttribute; }
MObject MFnEnumAttribute::create(const MString& ln, const MString& sn, short def, MStatus* st) {
    azNew(MFn::kEnumAttribute, ln, sn, "enum", st);
    azSpec()->def.assign(1, def);
    return obj_;
}
MStatus MFnEnumAttribute::addField(const MString& name, short value) {
    AttrSpec* s = azSpec();
    if (!s) return MS::kInvalidParameter;
    for (size_t i = 0; i < s->fields.size(); ++i)
        if (s->fields[i].first == name.asChar() || s->fields[i].second == value) return MS::kInvalidParameter;
    s->fields.push_back(std::make_pair(std::string(name.asChar()), (int)value));
    return MS::kSuccess;
}
MString MFnEnumAttribute::fieldName(short value, MStatus* st) const {
    AttrSpec* s = azSpec(st);
    if (s) for (size_t i = 0; i < s->fields.size(); ++i) if (s->fields[i].second == value) return MString(s->fields[i].first);
    if (st) *st = MS::kInvalidParameter;
    return MString();
}
short MFnEnumAttribute::fieldIndex(const MString& name, MStatus* st) const {
    AttrSpec* s = azSpec(st);
    if (s) for (size_t i = 0; i < s->fields.size(); ++i) if (s->fields[i].first == name.asChar()) return (short)s->fields[i].second;
    if (st) *st = MS::kInvalidParameter;
    return 0;
}
MStatus MFnEnumAttribute::getMin(short& v) const {
    AttrSpec* s = azSpec();
    if (!s || s->fields.empty()) return MS::kFailure;
    v = (short)s->fields[0].second;
    for (size_t i = 0; i < s->fields.size(); ++i) if (s->fields[i].second < v) v = (short)s->fields[i].second;
    return MS::kSuccess;
}
MStatus MFnEnumAttribute::getMax(short& v) const {
    AttrSpec* s = azSpec();
    if (!s || s->fields.empty()) return MS::kFailure;
    v = (short)s->fields[0].second;
    for (size_t i = 0; i < s->fields.size(); ++i) if (s->fields[i].second > v) v = (short)s->fields[i].second;
    return MS::kSuccess;
}
MStatus MFnEnumAttribute::setDefault(short v) { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; s->def.assign(1, v); return MS::kSuccess; }
MStatus MFnEnumAttribute::setDefault(const MString& name) {
    MStatus st;
    short v = fieldIndex(name, &st);
    return st ? setDefault(v) : st;
}
MStatus MFnEnumAttribute::getDefault(short& v) const { AttrSpec* s = azSpec(); if (!s) return MS::kInvalidParameter; v = (short)s->def[0]; return MS::kSuccess; }
MString MFnEnumAttribute::defaultValue(MStatus* st) const { short v = 0; getDefault(v); return fieldName(v, st); }

// Compound

MFnCompoundAttribute::MFnCompoundAttribute() {}
MFnCompoundAttribute::MFnCompoundAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnCompoundAttribute::type() const { return MFn::kCompoundAttribute; }
MObject MFnCompoundAttribute::create(const MString& ln, const MString& sn, MStatus* st) {
    azNew(MFn::kCompoundAttribute, ln, sn, "compound", st);
    azSpec()->def.clear();
    return obj_;
}
MStatus MFnCompoundAttribute::addChild(const MObject& child) {
    AttrSpec* s = azSpec();
    const std::shared_ptr<ObjectData>& d = child.azData();
    if (!s || !d || !d->attr) return MS::kInvalidParameter;
    d->attr->parent = s;
    s->children.push_back(d->attr);
    return MS::kSuccess;
}
MStatus MFnCompoundAttribute::removeChild(const MObject& child) {
    AttrSpec* s = azSpec();
    const std::shared_ptr<ObjectData>& d = child.azData();
    if (!s || !d || !d->attr) return MS::kInvalidParameter;
    for (size_t i = 0; i < s->children.size(); ++i)
        if (s->children[i] == d->attr) {
            s->children.erase(s->children.begin() + i);
            d->attr->parent = 0;
            return MS::kSuccess;
        }
    return MS::kFailure;
}
unsigned int MFnCompoundAttribute::numChildren(MStatus* st) const { AttrSpec* s = azSpec(st); return s ? (unsigned int)s->children.size() : 0; }
MObject MFnCompoundAttribute::child(unsigned int i, MStatus* st) const {
    AttrSpec* s = azSpec(st);
    if (!s || i >= s->children.size()) {
        if (st) *st = MS::kInvalidParameter;
        return MObject();
    }
    return attrObject(s->children[i]);
}

// Matrix, message, generic

MFnMatrixAttribute::MFnMatrixAttribute() {}
MFnMatrixAttribute::MFnMatrixAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnMatrixAttribute::type() const { return MFn::kMatrixAttribute; }
MObject MFnMatrixAttribute::create(const MString& ln, const MString& sn, Type, MStatus* st) {
    azNew(MFn::kMatrixAttribute, ln, sn, "matrix", st);
    MMatrix id;
    setDefault(id);
    return obj_;
}
MStatus MFnMatrixAttribute::setDefault(const MMatrix& m) {
    AttrSpec* s = azSpec();
    if (!s) return MS::kInvalidParameter;
    s->def.resize(16);
    for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) s->def[r * 4 + c] = m[r][c];
    return MS::kSuccess;
}
MStatus MFnMatrixAttribute::setDefault(const MFloatMatrix& m) { return setDefault(MMatrix(m)); }

MFnMessageAttribute::MFnMessageAttribute() {}
MFnMessageAttribute::MFnMessageAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnMessageAttribute::type() const { return MFn::kMessageAttribute; }
MObject MFnMessageAttribute::create(const MString& ln, const MString& sn, MStatus* st) {
    azNew(MFn::kMessageAttribute, ln, sn, "message", st);
    AttrSpec* s = azSpec();
    s->storable = false;
    s->hidden = true;
    return obj_;
}

MFnGenericAttribute::MFnGenericAttribute() {}
MFnGenericAttribute::MFnGenericAttribute(const MObject& o, MStatus* st) : MFnAttribute(o, st) {}
MFn::Type MFnGenericAttribute::type() const { return MFn::kGenericAttribute; }
MObject MFnGenericAttribute::create(const MString& ln, const MString& sn, MStatus* st) { return azNew(MFn::kGenericAttribute, ln, sn, "float", st); }
MStatus MFnGenericAttribute::addNumericDataAccept(MFnNumericData::Type) { return MS::kSuccess; }
MStatus MFnGenericAttribute::addDataAccept(MFnData::Type t) {
    AttrSpec* s = azSpec();
    if (s && t == MFnData::kMesh) s->type = "mesh";
    if (s && t == MFnData::kString && s->type == "float") s->type = "string";
    if (s && t == MFnData::kMatrix && s->type == "float") s->type = "matrix";
    return MS::kSuccess;
}
MStatus MFnGenericAttribute::addAccept(MFnNumericData::Type t) { return addNumericDataAccept(t); }
MStatus MFnGenericAttribute::addAccept(MFnData::Type t) { return addDataAccept(t); }
