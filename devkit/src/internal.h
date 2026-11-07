// The kit's insides: the host's table, JSON, and what an MObject holds. Not for plug-ins.
#ifndef AZTEK_KIT_INTERNAL_H
#define AZTEK_KIT_INTERNAL_H

#include <aztek/abi.h>
#include <maya/MTypes.h>
#include <maya/MObject.h>
#include <maya/MString.h>
#include <maya/MStatus.h>
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
    std::vector<double> nums;  // numeric and matrix data
    int numericType = 0;
    std::string str;              // string data; a component's node path
    std::vector<int> ids;         // component indices
    std::vector<int> ids2;        // second indices of a double-indexed component
    bool complete = false;        // a component standing for all
    void* plugin = 0;
};

MObject nodeObject(int64_t id);
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

}

#endif
