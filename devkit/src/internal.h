// The kit's insides: the host's table, JSON, and what an MObject holds. Not for plug-ins.
#ifndef AZTEK_KIT_INTERNAL_H
#define AZTEK_KIT_INTERNAL_H

#include <aztek/abi.h>
#include <maya/MTypes.h>
#include <maya/MObject.h>
#include <maya/MString.h>
#include <maya/MStatus.h>
#include <aztek/kit/all.h>
#include <deque>
#include <mutex>
#include <map>
#include <string>
#include <vector>

namespace az {

extern const AzHost* host;
extern void* pluginToken;

// ---------------------------------------------------------------------------------------------------------------------
// JSON

class Json {
public:
    enum Kind { Null, Bool, Number, String, Array, Object };
    Json() : k_(Null), n_(0) {}
    Json(bool b) : k_(Bool), n_(b ? 1 : 0) {}
    Json(int v) : k_(Number), n_(v) {}
    Json(unsigned int v) : k_(Number), n_(v) {}
    Json(int64_t v) : k_(Number), n_((double)v) {}
    Json(double v) : k_(Number), n_(v) {}
    Json(const char* s) : k_(String), n_(0), s_(s ? s : "") {}
    Json(const std::string& s) : k_(String), n_(0), s_(s) {}
    Json(const MString& s) : k_(String), n_(0), s_(s.asChar()) {}
    static Json array() { Json j; j.k_ = Array; return j; }
    static Json object() { Json j; j.k_ = Object; return j; }
    static Json parse(const std::string& text);

    Kind kind() const { return k_; }
    bool isNull() const { return k_ == Null; }
    bool isNumber() const { return k_ == Number || k_ == Bool; }
    bool isString() const { return k_ == String; }
    bool isArray() const { return k_ == Array; }
    bool isObject() const { return k_ == Object; }
    double num(double fallback = 0.0) const;
    int64_t integer(int64_t fallback = 0) const { return isNumber() ? (int64_t)num() : fallback; }
    bool truth() const;
    const std::string& str() const { return s_; }
    std::string text() const;  // a string's text, or anything else as JSON
    size_t size() const { return k_ == Array ? a_.size() : k_ == Object ? o_.size() : 0; }
    const Json& operator[](size_t i) const;
    const Json& operator[](const std::string& key) const;
    const Json& operator[](const char* key) const { return (*this)[std::string(key)]; }
    bool has(const std::string& key) const { return k_ == Object && o_.count(key) != 0; }
    Json& push(const Json& v) { k_ = Array; a_.push_back(v); return *this; }
    Json& set(const std::string& key, const Json& v) { k_ = Object; o_[key] = v; return *this; }
    const std::vector<Json>& items() const { return a_; }
    const std::map<std::string, Json>& fields() const { return o_; }
    std::string dump() const;

private:
    Kind k_;
    double n_;
    std::string s_;
    std::vector<Json> a_;
    std::map<std::string, Json> o_;
};

// ---------------------------------------------------------------------------------------------------------------------
// Calls to the host. Each fills `status` and returns the answer's "ok", or null with the error kept for messages.

Json query(const char* op, const Json& args, MStatus* status = 0);
Json command(const char* name, const Json& args, const Json& flags = Json::object(), MStatus* status = 0);
Json mel(const std::string& src, MStatus* status = 0);
Json python(const std::string& src, MStatus* status = 0);
void log(const std::string& text, int kind);
std::string take(char* s);  // a host string, freed
MStatus fail(MStatus::MStatusCode code, const std::string& message);
std::string lastError();
// A MEL string literal.
std::string quoted(const std::string& s);

// ---------------------------------------------------------------------------------------------------------------------
// What an MObject points at.

struct MeshData;
struct CurveData;
struct NodeClass;

// An attribute as the plug-in made it.
struct AttrSpec {
    MFn::Type fn = MFn::kAttribute;
    std::string name, shortName, nice;
    // float, int, bool, enum, vec3, matrix, string, mesh, message, compound, data
    std::string type = "float";
    int numeric = 0;     // MFnNumericData::Type
    int dataType = 0;    // MFnData::Type
    std::string unit;    // angle, distance, time
    int components = 1;  // 2 or 3 for numeric compounds
    std::vector<double> def = std::vector<double>(1, 0.0);
    std::string defString;
    bool readable = true, writable = true, storable = true, keyable = false, hidden = false, array = false;
    bool connectable = true, channelBox = false, color = false, builder = false, cached = true;
    bool point = false;  // made by createPoint: its numbers are named X, Y and Z
    bool hasMin = false, hasMax = false, hasSoftMin = false, hasSoftMax = false;
    double min = 0, max = 0, softMin = 0, softMax = 0;
    std::vector<std::pair<std::string, int> > fields;
    std::vector<std::shared_ptr<AttrSpec> > children;
    AttrSpec* parent = 0;
    // A static attribute every deformer has: the host's name for it.
    std::string builtin;
};

struct ObjectData {
    MFn::Type type = MFn::kInvalid;
    int64_t node = -1;  // a scene node
    std::shared_ptr<AttrSpec> attr;
    std::shared_ptr<MeshData> mesh;
    std::shared_ptr<CurveData> curve;
    std::vector<double> nums;  // numeric and matrix data
    int numericType = 0;
    std::string str;              // string data; a component's node path
    std::vector<std::string> strs;  // string array data
    std::vector<int> ids;         // component indices
    std::vector<int> ids2;        // second indices of a double-indexed component
    bool complete = false;        // a component standing for all
    void* plugin = 0;
};

MObject nodeObject(int64_t id);
std::shared_ptr<MeshData> meshOfNode(int64_t node, MStatus* status);  // a mesh shape's output mesh
MObject attrObject(const std::shared_ptr<AttrSpec>& spec);
int64_t nodeOf(const MObject& o);  // -1 when not a node
std::string nodePath(int64_t id);  // "" when gone
int64_t nodeNamed(const std::string& name);
MFn::Type fnOfNode(int64_t id);
bool nodeHasFn(int64_t id, MFn::Type t);
const char* fnName(MFn::Type t);

// The mesh a mesh data object or compute holds: a host handle, with its arrays read when first asked for.
struct MeshData {
    AzMesh* handle = 0;
    bool haveArrays = false;
    AzMeshArrays arrays;
    std::vector<double> points;  // edited points, written back as a new handle
    bool pointsEdited = false;
    MeshData();
    ~MeshData();
    void setHandle(AzMesh* h);
    const AzMeshArrays& get();
    void loadPoints();
    AzMesh* commit();  // the handle with the edited points
};


extern MTime::Unit uiTimeUnit;

// A node type the plug-in registered.
struct NodeClass {
    std::string name;
    MTypeId id;
    MCreatorFunction creator = 0;
    int kind = 0;  // AZ_NODE_DEPEND, AZ_NODE_DEFORMER or AZ_NODE_LOCATOR
    std::string classification;  // as registered: a draw override is found by it
    std::vector<std::shared_ptr<AttrSpec> > attrs;  // as added
    std::vector<std::pair<AttrSpec*, AttrSpec*> > affects;
    // Each attribute's index on the host's type, and the component of a vec3 child.
    std::map<const AttrSpec*, int> index;
    std::map<const AttrSpec*, int> component;
    std::map<int, std::shared_ptr<AttrSpec> > byIndex;
    AzNodeClass cls;
    std::map<int64_t, MPxNode*> instances;
    std::mutex lock;  // instances
};

std::vector<NodeClass*>& nodeClasses();
NodeClass* classOfNode(int64_t node);   // null when not one of this plug-in's
NodeClass* classNamed(const std::string& name);
NodeClass* classOfId(const MTypeId& id);
// The host's index of an attribute on a type, with the component of a vec3 child; -1 when it isn't there.
int indexOf(NodeClass* cls, const AttrSpec* spec, int* component);
// The deformer attributes every deformer has.
std::shared_ptr<AttrSpec> builtinAttr(const char* name);
// Arrays inside arrays. How many array levels lead down to an attribute, its own included; the array compounds above it,
// outermost first; and the indices at those levels packed into the one element the host keeps, as the host packs them
// (aztek_scene::plug::pack): two levels 7 and 24 bits, three 7, 12 and 12, four 7, 8, 8 and 8.
int arrayDepth(const AttrSpec* s);
// Every number in a value, flattened; a one-argument list.
std::vector<double> numbers(const Json& v);
Json args1(const std::string& a);
// Draw overrides by draw classification: the registrant id and the creator.
std::map<std::string, std::pair<std::string, MHWRender::DrawOverrideCreator> >& drawOverrides();
std::vector<const AttrSpec*> arrayAncestors(const AttrSpec* s);
int packPath(const std::vector<int>& idx);
std::vector<int> unpackPath(int element, int levels);

// One compute: the host's block and storage for what handles give out by reference.
struct BlockImpl {
    AzBlock* block = 0;
    NodeClass* cls = 0;
    int64_t node = -1;
    std::deque<bool> b;
    std::deque<char> c;
    std::deque<unsigned char> uc;
    std::deque<short> sh;
    std::deque<int> i;
    std::deque<int64_t> i64;
    std::deque<float> f;
    std::deque<double> d;
    struct S2 { short v[2]; };
    struct S3 { short v[3]; };
    struct I2 { int v[2]; };
    struct I3 { int v[3]; };
    struct F2 { float v[2]; };
    struct F3 { float v[3]; };
    struct D2 { double v[2]; };
    struct D3 { double v[3]; };
    std::deque<S2> s2;
    std::deque<S3> s3;
    std::deque<I2> i2;
    std::deque<I3> i3;
    std::deque<F2> f2;
    std::deque<F3> f3;
    std::deque<D2> d2;
    std::deque<D3> d3;
    std::deque<MVector> vec;
    std::deque<MFloatVector> fvec;
    std::deque<MMatrix> mat;
    std::deque<MFloatMatrix> fmat;
    std::deque<MString> str;
    // Meshes handed out, by attribute and element, so edits through MFnMesh reach what gets written.
    std::map<std::pair<int, int>, MObject> meshes;
};

// The compute running on this thread, so plugs of its node read from its block.
BlockImpl*& currentBlock();
BlockImpl& looseStorage();

// The current command's result.
Json& commandResult();

// A crash-guarded call into plug-in code: AZ_OK, AZ_FAILURE (with the message in error) or AZ_CRASHED.
int guarded(int (*fn)(void*), void* ctx, std::string& error);

}

#endif
