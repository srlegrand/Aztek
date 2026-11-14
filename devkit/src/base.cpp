// Aztek plug-in kit: JSON, calls to the host, MStatus, MString, MTypeId, MObject, units.
#include "internal.h"
#include <algorithm>

#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <sstream>

namespace az {

const AzHost* host = 0;
void* pluginToken = 0;
MTime::Unit uiTimeUnit = MTime::k24FPS;

static std::string& lastErrorText() {
    static thread_local std::string e;
    return e;
}

std::string lastError() { return lastErrorText(); }

// ---------------------------------------------------------------------------------------------------------------------
// JSON

static const Json kNullJson;

double Json::num(double fallback) const {
    if (k_ == Number || k_ == Bool) return n_;
    if (k_ == String) {
        char* end = 0;
        double v = strtod(s_.c_str(), &end);
        if (end && end != s_.c_str()) return v;
    }
    return fallback;
}

bool Json::truth() const {
    switch (k_) {
    case Bool:
    case Number: return n_ != 0;
    case String: return !s_.empty() && s_ != "0" && s_ != "false";
    case Array: return !a_.empty();
    case Object: return !o_.empty();
    default: return false;
    }
}

const Json& Json::operator[](size_t i) const { return k_ == Array && i < a_.size() ? a_[i] : kNullJson; }

const Json& Json::operator[](const std::string& key) const {
    if (k_ != Object) return kNullJson;
    std::map<std::string, Json>::const_iterator it = o_.find(key);
    return it == o_.end() ? kNullJson : it->second;
}

std::string Json::text() const { return k_ == String ? s_ : dump(); }

static void dumpString(std::string& out, const std::string& s) {
    out += '"';
    for (size_t i = 0; i < s.size(); ++i) {
        unsigned char c = (unsigned char)s[i];
        switch (c) {
        case '"': out += "\\\""; break;
        case '\\': out += "\\\\"; break;
        case '\n': out += "\\n"; break;
        case '\r': out += "\\r"; break;
        case '\t': out += "\\t"; break;
        default:
            if (c < 0x20) {
                char buf[8];
                snprintf(buf, sizeof buf, "\\u%04x", c);
                out += buf;
            } else {
                out += (char)c;
            }
        }
    }
    out += '"';
}

static void dumpTo(std::string& out, const Json& j) {
    switch (j.kind()) {
    case Json::Null: out += "null"; break;
    case Json::Bool: out += j.truth() ? "true" : "false"; break;
    case Json::Number: {
        double v = j.num();
        if (!std::isfinite(v)) {
            out += "null";
        } else if (v == std::floor(v) && std::fabs(v) < 9e15) {
            char buf[32];
            snprintf(buf, sizeof buf, "%lld", (long long)v);
            out += buf;
        } else {
            char buf[40];
            snprintf(buf, sizeof buf, "%.17g", v);
            out += buf;
        }
        break;
    }
    case Json::String: dumpString(out, j.str()); break;
    case Json::Array: {
        out += '[';
        for (size_t i = 0; i < j.size(); ++i) {
            if (i) out += ',';
            dumpTo(out, j[i]);
        }
        out += ']';
        break;
    }
    case Json::Object: {
        out += '{';
        bool first = true;
        for (std::map<std::string, Json>::const_iterator it = j.fields().begin(); it != j.fields().end(); ++it) {
            if (!first) out += ',';
            first = false;
            dumpString(out, it->first);
            out += ':';
            dumpTo(out, it->second);
        }
        out += '}';
        break;
    }
    }
}

std::string Json::dump() const {
    std::string out;
    dumpTo(out, *this);
    return out;
}

namespace {
struct Parser {
    const char* p;
    const char* end;
    void ws() {
        while (p < end && (*p == ' ' || *p == '\n' || *p == '\r' || *p == '\t')) ++p;
    }
    static void utf8(std::string& out, unsigned cp) {
        if (cp < 0x80) {
            out += (char)cp;
        } else if (cp < 0x800) {
            out += (char)(0xC0 | (cp >> 6));
            out += (char)(0x80 | (cp & 0x3F));
        } else if (cp < 0x10000) {
            out += (char)(0xE0 | (cp >> 12));
            out += (char)(0x80 | ((cp >> 6) & 0x3F));
            out += (char)(0x80 | (cp & 0x3F));
        } else {
            out += (char)(0xF0 | (cp >> 18));
            out += (char)(0x80 | ((cp >> 12) & 0x3F));
            out += (char)(0x80 | ((cp >> 6) & 0x3F));
            out += (char)(0x80 | (cp & 0x3F));
        }
    }
    unsigned hex4() {
        unsigned v = 0;
        for (int i = 0; i < 4 && p < end; ++i, ++p) {
            char c = *p;
            v <<= 4;
            if (c >= '0' && c <= '9') v |= c - '0';
            else if (c >= 'a' && c <= 'f') v |= c - 'a' + 10;
            else if (c >= 'A' && c <= 'F') v |= c - 'A' + 10;
        }
        return v;
    }
    std::string str() {
        std::string out;
        ++p;
        while (p < end && *p != '"') {
            if (*p == '\\' && p + 1 < end) {
                ++p;
                char c = *p++;
                switch (c) {
                case 'n': out += '\n'; break;
                case 't': out += '\t'; break;
                case 'r': out += '\r'; break;
                case 'b': out += '\b'; break;
                case 'f': out += '\f'; break;
                case 'u': {
                    unsigned cp = hex4();
                    if (cp >= 0xD800 && cp < 0xDC00 && p + 1 < end && p[0] == '\\' && p[1] == 'u') {
                        p += 2;
                        unsigned lo = hex4();
                        cp = 0x10000 + ((cp - 0xD800) << 10) + (lo - 0xDC00);
                    }
                    utf8(out, cp);
                    break;
                }
                default: out += c;
                }
            } else {
                out += *p++;
            }
        }
        if (p < end) ++p;
        return out;
    }
    Json value() {
        ws();
        if (p >= end) return Json();
        char c = *p;
        if (c == '{') {
            ++p;
            Json o = Json::object();
            ws();
            if (p < end && *p == '}') { ++p; return o; }
            while (p < end) {
                ws();
                if (p >= end || *p != '"') break;
                std::string k = str();
                ws();
                if (p < end && *p == ':') ++p;
                o.set(k, value());
                ws();
                if (p < end && *p == ',') { ++p; continue; }
                if (p < end && *p == '}') { ++p; break; }
                break;
            }
            return o;
        }
        if (c == '[') {
            ++p;
            Json a = Json::array();
            ws();
            if (p < end && *p == ']') { ++p; return a; }
            while (p < end) {
                a.push(value());
                ws();
                if (p < end && *p == ',') { ++p; continue; }
                if (p < end && *p == ']') { ++p; break; }
                break;
            }
            return a;
        }
        if (c == '"') return Json(str());
        if (!strncmp(p, "true", 4)) { p += 4; return Json(true); }
        if (!strncmp(p, "false", 5)) { p += 5; return Json(false); }
        if (!strncmp(p, "null", 4)) { p += 4; return Json(); }
        char* stop = 0;
        double v = strtod(p, &stop);
        if (stop == p) { ++p; return Json(); }
        p = stop;
        return Json(v);
    }
};
}

Json Json::parse(const std::string& text) {
    Parser ps;
    ps.p = text.c_str();
    ps.end = ps.p + text.size();
    return ps.value();
}

// ---------------------------------------------------------------------------------------------------------------------
// Calls

std::string take(char* s) {
    if (!s) return std::string();
    std::string out(s);
    if (host) host->string_free(s);
    return out;
}

MStatus fail(MStatus::MStatusCode code, const std::string& message) {
    lastErrorText() = message;
    MStatus st(code);
    st.setErrorString(MString(message.c_str()));
    return st;
}

static Json answer(char* raw, MStatus* status) {
    Json j = Json::parse(take(raw));
    if (j.has("error")) {
        std::string e = j["error"].text();
        MStatus::MStatusCode code = MStatus::kFailure;
        if (e.find("kInvalidParameter") != std::string::npos) code = MStatus::kInvalidParameter;
        if (e.find("kNotImplemented") != std::string::npos) code = MStatus::kNotImplemented;
        MStatus st = fail(code, e);
        if (status) *status = st;
        return Json();
    }
    if (status) *status = MS::kSuccess;
    return j["ok"];
}

SceneStandIn& sceneStandIn() {
    static thread_local SceneStandIn s = 0;
    return s;
}

Json query(const char* op, const Json& args, MStatus* status) {
    if (SceneStandIn s = sceneStandIn()) {
        Json out;
        if (s(true, op, args, Json(), out, status)) return out;
    }
    if (!host) { if (status) *status = fail(MStatus::kFailure, "not loaded"); return Json(); }
    return answer(host->query(op, args.dump().c_str()), status);
}

Json command(const char* name, const Json& args, const Json& flags, MStatus* status) {
    if (!host) { if (status) *status = fail(MStatus::kFailure, "not loaded"); return Json(); }
    Json a = args.isArray() ? args : Json::array();
    if (!args.isArray() && !args.isNull()) a.push(args);
    if (SceneStandIn s = sceneStandIn()) {
        Json out;
        if (s(false, name, a, flags, out, status)) return out;
    }
    return answer(host->command(name, a.dump().c_str(), flags.isObject() ? flags.dump().c_str() : "{}"), status);
}

Json mel(const std::string& src, MStatus* status) {
    if (!host) { if (status) *status = fail(MStatus::kFailure, "not loaded"); return Json(); }
    return answer(host->mel(src.c_str()), status);
}

Json python(const std::string& src, MStatus* status) {
    if (!host) { if (status) *status = fail(MStatus::kFailure, "not loaded"); return Json(); }
    return answer(host->python(src.c_str()), status);
}

void log(const std::string& text, int kind) {
    if (host) host->log(text.c_str(), kind);
    else std::cerr << text << std::endl;
}

std::string quoted(const std::string& s) {
    std::string out = "\"";
    for (size_t i = 0; i < s.size(); ++i) {
        char c = s[i];
        if (c == '"' || c == '\\') out += '\\';
        if (c == '\n') { out += "\\n"; continue; }
        out += c;
    }
    return out + "\"";
}

// ---------------------------------------------------------------------------------------------------------------------
// Nodes as objects

MObject nodeObject(int64_t id) {
    if (id < 0) return MObject();
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = MFn::kDependencyNode;
    d->node = id;
    return MObject(d);
}

MObject attrObject(const std::shared_ptr<AttrSpec>& spec) {
    if (!spec) return MObject();
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = spec->fn;
    d->attr = spec;
    return MObject(d);
}

int64_t nodeOf(const MObject& o) {
    const std::shared_ptr<ObjectData>& d = o.azData();
    return d && d->node >= 0 && !d->attr && d->ids.empty() && !d->complete ? d->node : -1;
}

std::string nodePath(int64_t id) {
    if (id < 0) return std::string();
    Json a = Json::object();
    a.set("id", id);
    Json info = query("node.info", a);
    return info["path"].str();
}

int64_t nodeNamed(const std::string& name) {
    Json a = Json::object();
    a.set("name", name);
    MStatus st;
    Json r = query("node.find", a, &st);
    return r.isNumber() ? r.integer() : -1;
}

struct FnOfType {
    const char* type;
    MFn::Type fn;
};

static const FnOfType kFnOfType[] = {
    {"transform", MFn::kTransform},
    {"joint", MFn::kJoint},
    {"ikHandle", MFn::kIkHandle},
    {"ikEffector", MFn::kIkEffector},
    {"shape", MFn::kShape},
    {"mesh", MFn::kMesh},
    {"nurbsCurve", MFn::kNurbsCurve},
    {"nurbsSurface", MFn::kNurbsSurface},
    {"locator", MFn::kLocator},
    {"camera", MFn::kCamera},
    {"lattice", MFn::kLattice},
    {"geometryFilter", MFn::kGeometryFilt},
    {"skinCluster", MFn::kSkinClusterFilter},
    {"blendShape", MFn::kBlendShape},
    {"cluster", MFn::kCluster},
    {"ffd", MFn::kFFD},
    {"nonLinear", MFn::kNonLinear},
    {"wire", MFn::kWire},
    {"polyTweak", MFn::kPolyTweak},
    {"animCurve", MFn::kAnimCurve},
    {"animCurveTA", MFn::kAnimCurveTimeToAngular},
    {"animCurveTL", MFn::kAnimCurveTimeToDistance},
    {"animCurveTT", MFn::kAnimCurveTimeToTime},
    {"animCurveTU", MFn::kAnimCurveTimeToUnitless},
    {"animCurveUA", MFn::kAnimCurveUnitlessToAngular},
    {"animCurveUL", MFn::kAnimCurveUnitlessToDistance},
    {"animCurveUT", MFn::kAnimCurveUnitlessToTime},
    {"animCurveUU", MFn::kAnimCurveUnitlessToUnitless},
    {"aimConstraint", MFn::kAimConstraint},
    {"orientConstraint", MFn::kOrientConstraint},
    {"parentConstraint", MFn::kParentConstraint},
    {"pointConstraint", MFn::kPointConstraint},
    {"scaleConstraint", MFn::kScaleConstraint},
    {"objectSet", MFn::kSet},
    {"shadingEngine", MFn::kShadingEngine},
    {"lambert", MFn::kLambert},
    {"multiplyDivide", MFn::kMultiplyDivide},
    {"polyCube", MFn::kPolyCube},
    {"polySphere", MFn::kPolySphere},
    {"polyCylinder", MFn::kPolyCylinder},
    {"polyCone", MFn::kPolyCone},
    {"polyPlane", MFn::kPolyPlane},
    {"polyTorus", MFn::kPolyTorus},
    {"displayLayer", MFn::kDisplayLayer},
    {"time", MFn::kTime},
};

static void fnsOfNode(int64_t id, std::vector<MFn::Type>& out) {
    Json a = Json::object();
    a.set("id", id);
    Json info = query("node.info", a);
    if (!info.isObject()) return;
    const Json& chain = info["chain"];
    if (!info["owner"].str().empty()) {
        out.push_back(MFn::kPluginDependNode);
    }
    for (size_t i = 0; i < chain.size(); ++i) {
        const std::string& t = chain[i].str();
        for (size_t k = 0; k < sizeof kFnOfType / sizeof kFnOfType[0]; ++k) {
            if (t == kFnOfType[k].type) out.push_back(kFnOfType[k].fn);
        }
        if (t.compare(0, 4, "poly") == 0 && t != "polyTweak") out.push_back(MFn::kPolyCreator);
        if (t == "geometryFilter" && !info["owner"].str().empty()) out.insert(out.begin(), MFn::kPluginDeformerNode);
        // A plug-in's transform, constraint or shape is what its kind says, before what it is built on.
        if (i == 0 && !info["owner"].str().empty()) {
            std::string base = chain.size() > 1 ? chain[(size_t)1].str() : std::string();
            NodeClass* c = classNamed(t);
            int kind = c ? c->kind : -1;
            if (kind == AZ_NODE_CONSTRAINT) out.insert(out.begin(), MFn::kPluginConstraintNode);
            else if (kind == AZ_NODE_IK_SOLVER) out.insert(out.begin(), MFn::kPluginIkSolver);
            else if (base == "transform") out.insert(out.begin(), MFn::kPluginTransformNode);
            else if (base == "shape") out.insert(out.begin(), MFn::kPluginShape);
        }
        if (t.size() > 10 && t.compare(t.size() - 10, 10, "Constraint") == 0) out.push_back(MFn::kConstraint);
    }
    for (size_t i = 0; i < out.size(); ++i) {
        MFn::Type f = out[i];
        if (f == MFn::kMesh) { out.push_back(MFn::kSurface); out.push_back(MFn::kGeometric); }
        if (f == MFn::kNurbsCurve) { out.push_back(MFn::kCurve); out.push_back(MFn::kGeometric); }
        if (f == MFn::kNurbsSurface) { out.push_back(MFn::kSurface); out.push_back(MFn::kGeometric); }
        if (f >= MFn::kAnimCurveTimeToAngular && f <= MFn::kAnimCurveUnitlessToUnitless) out.push_back(MFn::kAnimCurve);
        if (f >= MFn::kPolyCube && f <= MFn::kPolyTorus) out.push_back(MFn::kPolyPrimitive);
        if (f == MFn::kSkinClusterFilter || f == MFn::kCluster || f == MFn::kFFD || f == MFn::kWire || f == MFn::kNonLinear) out.push_back(MFn::kWeightGeometryFilt);
        // Every weighted deformer is a geometry filter, a skinCluster too, though Aztek's isn't one underneath.
        if (f == MFn::kWeightGeometryFilt && std::find(out.begin(), out.end(), MFn::kGeometryFilt) == out.end()) out.push_back(MFn::kGeometryFilt);
    }
    if (info["dag"].truth()) { out.push_back(MFn::kDagNode); out.push_back(MFn::kEntity); }
    out.push_back(MFn::kDependencyNode);
    out.push_back(MFn::kNamedObject);
    out.push_back(MFn::kBase);
}

MFn::Type fnOfNode(int64_t id) {
    std::vector<MFn::Type> fns;
    fnsOfNode(id, fns);
    // The most derived: the first type of the chain found, a plug-in kind first.
    return fns.empty() ? MFn::kInvalid : fns[0];
}

bool nodeHasFn(int64_t id, MFn::Type t) {
    std::vector<MFn::Type> fns;
    fnsOfNode(id, fns);
    for (size_t i = 0; i < fns.size(); ++i)
        if (fns[i] == t) return true;
    return false;
}

#define AZ_FN_NAME(x) case MFn::x: return #x;
const char* fnName(MFn::Type t) {
    switch (t) {
        AZ_FN_NAME(kInvalid) AZ_FN_NAME(kBase) AZ_FN_NAME(kNamedObject) AZ_FN_NAME(kDependencyNode)
        AZ_FN_NAME(kDagNode) AZ_FN_NAME(kEntity) AZ_FN_NAME(kTransform) AZ_FN_NAME(kJoint) AZ_FN_NAME(kIkHandle)
        AZ_FN_NAME(kIkEffector) AZ_FN_NAME(kShape) AZ_FN_NAME(kGeometric) AZ_FN_NAME(kSurface) AZ_FN_NAME(kMesh)
        AZ_FN_NAME(kCurve) AZ_FN_NAME(kNurbsCurve) AZ_FN_NAME(kNurbsSurface) AZ_FN_NAME(kLocator) AZ_FN_NAME(kCamera)
        AZ_FN_NAME(kLight) AZ_FN_NAME(kLattice) AZ_FN_NAME(kGeometryFilt) AZ_FN_NAME(kWeightGeometryFilt)
        AZ_FN_NAME(kSkinClusterFilter) AZ_FN_NAME(kBlendShape) AZ_FN_NAME(kCluster) AZ_FN_NAME(kFFD)
        AZ_FN_NAME(kNonLinear) AZ_FN_NAME(kWire) AZ_FN_NAME(kTweak) AZ_FN_NAME(kPolyTweak) AZ_FN_NAME(kAnimCurve)
        AZ_FN_NAME(kAnimCurveTimeToAngular) AZ_FN_NAME(kAnimCurveTimeToDistance) AZ_FN_NAME(kAnimCurveTimeToTime)
        AZ_FN_NAME(kAnimCurveTimeToUnitless) AZ_FN_NAME(kAnimCurveUnitlessToAngular)
        AZ_FN_NAME(kAnimCurveUnitlessToDistance) AZ_FN_NAME(kAnimCurveUnitlessToTime)
        AZ_FN_NAME(kAnimCurveUnitlessToUnitless) AZ_FN_NAME(kConstraint) AZ_FN_NAME(kAimConstraint)
        AZ_FN_NAME(kOrientConstraint) AZ_FN_NAME(kParentConstraint) AZ_FN_NAME(kPointConstraint)
        AZ_FN_NAME(kScaleConstraint) AZ_FN_NAME(kSet) AZ_FN_NAME(kShadingEngine) AZ_FN_NAME(kLambert)
        AZ_FN_NAME(kMultiplyDivide) AZ_FN_NAME(kPolyCreator) AZ_FN_NAME(kPolyPrimitive) AZ_FN_NAME(kPolyCube)
        AZ_FN_NAME(kPolySphere) AZ_FN_NAME(kPolyCylinder) AZ_FN_NAME(kPolyCone) AZ_FN_NAME(kPolyPlane)
        AZ_FN_NAME(kPolyTorus) AZ_FN_NAME(kPluginDependNode) AZ_FN_NAME(kPluginDeformerNode)
        AZ_FN_NAME(kPluginLocatorNode) AZ_FN_NAME(kPluginTransformNode) AZ_FN_NAME(kPluginConstraintNode) AZ_FN_NAME(kPluginShape) AZ_FN_NAME(kPluginIkSolver) AZ_FN_NAME(kPluginData) AZ_FN_NAME(kDisplayLayer) AZ_FN_NAME(kTime) AZ_FN_NAME(kUnknown)
        AZ_FN_NAME(kAttribute) AZ_FN_NAME(kNumericAttribute) AZ_FN_NAME(kUnitAttribute) AZ_FN_NAME(kTypedAttribute)
        AZ_FN_NAME(kEnumAttribute) AZ_FN_NAME(kCompoundAttribute) AZ_FN_NAME(kMatrixAttribute)
        AZ_FN_NAME(kMessageAttribute) AZ_FN_NAME(kGenericAttribute) AZ_FN_NAME(kAttribute3Double)
        AZ_FN_NAME(kAttribute3Float) AZ_FN_NAME(kDoubleLinearAttribute) AZ_FN_NAME(kDoubleAngleAttribute)
        AZ_FN_NAME(kTimeAttribute) AZ_FN_NAME(kData) AZ_FN_NAME(kNumericData) AZ_FN_NAME(kMeshData)
        AZ_FN_NAME(kMatrixData) AZ_FN_NAME(kStringData) AZ_FN_NAME(kGeometryData) AZ_FN_NAME(kComponent)
        AZ_FN_NAME(kSingleIndexedComponent) AZ_FN_NAME(kDoubleIndexedComponent) AZ_FN_NAME(kMeshComponent)
        AZ_FN_NAME(kMeshVertComponent) AZ_FN_NAME(kMeshEdgeComponent) AZ_FN_NAME(kMeshPolygonComponent)
        AZ_FN_NAME(kMeshMapComponent) AZ_FN_NAME(kMeshVtxFaceComponent) AZ_FN_NAME(kCurveCVComponent)
        AZ_FN_NAME(kCurveEPComponent) AZ_FN_NAME(kPluginObject) AZ_FN_NAME(kWorld) AZ_FN_NAME(kLast)
    }
    return "kInvalid";
}

}  // namespace az

using namespace az;

// ---------------------------------------------------------------------------------------------------------------------
// MStatus

MStatus::MStatus() : code_(MS::kSuccess) {}
MStatus::MStatus(MS::MStatusCode code) : code_(code) {}
MStatus::MStatus(MStatusCode code) : code_((MS::MStatusCode)code) {}
MStatus::MStatus(const MStatus& o) : code_(o.code_), text_(o.text_) {}
MStatus::~MStatus() {}
MStatus& MStatus::operator=(const MStatus& o) {
    code_ = o.code_;
    text_ = o.text_;
    return *this;
}
bool MStatus::operator==(const MStatus& o) const { return code_ == o.code_; }
bool MStatus::operator==(MS::MStatusCode c) const { return code_ == c; }
bool MStatus::operator!=(const MStatus& o) const { return code_ != o.code_; }
bool MStatus::operator!=(MS::MStatusCode c) const { return code_ != c; }
MStatus::operator bool() const { return code_ == MS::kSuccess; }
bool MStatus::error() const { return code_ != MS::kSuccess; }
void MStatus::clear() {
    code_ = MS::kSuccess;
    text_.reset();
}
MS::MStatusCode MStatus::statusCode() const { return code_; }

static const char* codeText(MS::MStatusCode c) {
    switch (c) {
    case MS::kSuccess: return "(kSuccess)";
    case MS::kFailure: return "(kFailure): Unexpected Internal Failure";
    case MS::kInsufficientMemory: return "(kInsufficientMemory): Insufficient Memory";
    case MS::kInvalidParameter: return "(kInvalidParameter): Object is incompatible with this method";
    case MS::kLicenseFailure: return "(kLicenseFailure)";
    case MS::kUnknownParameter: return "(kUnknownParameter)";
    case MS::kNotImplemented: return "(kNotImplemented)";
    case MS::kNotFound: return "(kNotFound)";
    case MS::kEndOfFile: return "(kEndOfFile)";
    }
    return "(kFailure)";
}

MString MStatus::errorString() const {
    if (text_ && !text_->empty()) return MString(text_->c_str());
    return MString(codeText(code_));
}

void MStatus::setErrorString(const MString& t) { text_ = std::make_shared<std::string>(t.asChar()); }

void MStatus::perror(const char* t) const {
    std::string s = t && *t ? std::string(t) + ": " : std::string();
    az::log("// Error: " + s + errorString().asChar() + " //", 2);
}
void MStatus::perror(const MString& t) const { perror(t.asChar()); }

std::ostream& operator<<(std::ostream& os, const MStatus& s) { return os << s.errorString().asChar(); }

void azReportStatus(const MStatus& s, const char* expr, const char* file, int line) {
    std::ostringstream o;
    const char* base = strrchr(file, '/');
    if (!base) base = strrchr(file, '\\');
    o << "// Error: " << s.errorString().asChar() << " in " << expr << ", " << (base ? base + 1 : file) << " line " << line << " //";
    az::log(o.str(), 2);
}

// ---------------------------------------------------------------------------------------------------------------------
// MString

MString::MString() {}
MString::MString(const char* t) : s_(t ? t : "") {}
MString::MString(const char* t, int n) : s_(t ? std::string(t, n < 0 ? 0 : n) : std::string()) {}
MString::MString(const wchar_t* t) {
    if (!t) return;
    for (; *t; ++t) {
        unsigned cp = (unsigned)*t;
        if (cp < 0x80) s_ += (char)cp;
        else if (cp < 0x800) { s_ += (char)(0xC0 | (cp >> 6)); s_ += (char)(0x80 | (cp & 0x3F)); }
        else { s_ += (char)(0xE0 | (cp >> 12)); s_ += (char)(0x80 | ((cp >> 6) & 0x3F)); s_ += (char)(0x80 | (cp & 0x3F)); }
    }
}
MString::MString(const MString& o) : s_(o.s_) {}
MString::MString(const std::string& t) : s_(t) {}
MString::~MString() {}
MString& MString::operator=(const MString& o) { s_ = o.s_; return *this; }
MString& MString::operator=(const char* t) { s_ = t ? t : ""; return *this; }
MString& MString::operator=(double v) { return set(v); }
MString& MString::operator+=(const MString& o) { s_ += o.s_; return *this; }
MString& MString::operator+=(const char* t) { if (t) s_ += t; return *this; }

static std::string numText(double v) {
    char buf[40];
    snprintf(buf, sizeof buf, "%g", v);
    return buf;
}

MString& MString::operator+=(double v) { s_ += numText(v); return *this; }
MString& MString::operator+=(float v) { s_ += numText(v); return *this; }
MString& MString::operator+=(int v) { s_ += std::to_string(v); return *this; }
MString& MString::operator+=(unsigned int v) { s_ += std::to_string(v); return *this; }
MString MString::operator+(const MString& o) const { MString r(*this); r += o; return r; }
MString MString::operator+(const char* o) const { MString r(*this); r += o; return r; }
MString MString::operator+(double v) const { MString r(*this); r += v; return r; }
MString MString::operator+(int v) const { MString r(*this); r += v; return r; }
MString MString::operator+(unsigned int v) const { MString r(*this); r += v; return r; }
bool MString::operator==(const MString& o) const { return s_ == o.s_; }
bool MString::operator==(const char* o) const { return s_ == (o ? o : ""); }
bool MString::operator!=(const MString& o) const { return s_ != o.s_; }
bool MString::operator!=(const char* o) const { return !(*this == o); }
bool MString::operator<(const MString& o) const { return s_ < o.s_; }
bool MString::operator>(const MString& o) const { return s_ > o.s_; }
MString& MString::set(const char* t) { s_ = t ? t : ""; return *this; }
MString& MString::set(const char* t, int n) { s_ = t ? std::string(t, n < 0 ? 0 : n) : std::string(); return *this; }
MString& MString::set(double v) { s_ = numText(v); return *this; }
MString& MString::set(double v, int precision) {
    char buf[64];
    snprintf(buf, sizeof buf, "%.*f", precision, v);
    s_ = buf;
    return *this;
}
const char* MString::asChar() const { return s_.c_str(); }
const char* MString::asChar(int& n) const { n = (int)s_.size(); return s_.c_str(); }
const char* MString::asUTF8() const { return s_.c_str(); }
const char* MString::asUTF8(int& n) const { n = (int)s_.size(); return s_.c_str(); }

static std::wstring wide(const std::string& s) {
    std::wstring out;
    for (size_t i = 0; i < s.size();) {
        unsigned char c = (unsigned char)s[i];
        unsigned cp = c;
        int n = 0;
        if (c >= 0xF0) { cp = c & 0x07; n = 3; }
        else if (c >= 0xE0) { cp = c & 0x0F; n = 2; }
        else if (c >= 0xC0) { cp = c & 0x1F; n = 1; }
        ++i;
        for (int k = 0; k < n && i < s.size(); ++k, ++i) cp = (cp << 6) | ((unsigned char)s[i] & 0x3F);
        out += (wchar_t)cp;
    }
    return out;
}

const wchar_t* MString::asWChar() const { w_ = wide(s_); return w_.c_str(); }
unsigned int MString::length() const { return (unsigned int)s_.size(); }
unsigned int MString::numChars() const { return (unsigned int)wide(s_).size(); }
void MString::clear() { s_.clear(); }
int MString::index(char c) const { size_t p = s_.find(c); return p == std::string::npos ? -1 : (int)p; }
int MString::rindex(char c) const { size_t p = s_.rfind(c); return p == std::string::npos ? -1 : (int)p; }
int MString::indexW(const MString& s) const { size_t p = s_.find(s.s_); return p == std::string::npos ? -1 : (int)p; }
int MString::indexW(char c) const { return index(c); }
int MString::rindexW(const MString& s) const { size_t p = s_.rfind(s.s_); return p == std::string::npos ? -1 : (int)p; }
int MString::rindexW(char c) const { return rindex(c); }

MStatus MString::split(char c, MStringArray& out) const {
    out.clear();
    std::string cur;
    for (size_t i = 0; i < s_.size(); ++i) {
        if (s_[i] == c) {
            if (!cur.empty()) out.append(MString(cur));
            cur.clear();
        } else {
            cur += s_[i];
        }
    }
    if (!cur.empty()) out.append(MString(cur));
    return MS::kSuccess;
}

MString MString::substring(int a, int b) const {
    if (a < 0) a = 0;
    if (b >= (int)s_.size()) b = (int)s_.size() - 1;
    if (b < a) return MString();
    return MString(s_.substr(a, b - a + 1));
}
MString MString::substringW(int a, int b) const { return substring(a, b); }

MString& MString::toUpperCase() {
    for (size_t i = 0; i < s_.size(); ++i) if (s_[i] >= 'a' && s_[i] <= 'z') s_[i] -= 32;
    return *this;
}
MString& MString::toLowerCase() {
    for (size_t i = 0; i < s_.size(); ++i) if (s_[i] >= 'A' && s_[i] <= 'Z') s_[i] += 32;
    return *this;
}
MString MString::toUpperCase() const { MString r(*this); return r.toUpperCase(); }
MString MString::toLowerCase() const { MString r(*this); return r.toLowerCase(); }

static bool wholeNumber(const std::string& s) {
    if (s.empty()) return false;
    char* end = 0;
    strtol(s.c_str(), &end, 10);
    return end && *end == 0;
}
static bool wholeReal(const std::string& s) {
    if (s.empty()) return false;
    char* end = 0;
    strtod(s.c_str(), &end);
    return end && *end == 0;
}

bool MString::isShort() const { return wholeNumber(s_); }
short MString::asShort() const { return (short)atoi(s_.c_str()); }
bool MString::isInt() const { return wholeNumber(s_); }
int MString::asInt() const { return atoi(s_.c_str()); }
bool MString::isUnsigned() const { return wholeNumber(s_) && s_[0] != '-'; }
unsigned int MString::asUnsigned() const { return (unsigned int)strtoul(s_.c_str(), 0, 10); }
bool MString::isFloat() const { return wholeReal(s_); }
float MString::asFloat() const { return (float)atof(s_.c_str()); }
bool MString::isDouble() const { return wholeReal(s_); }
double MString::asDouble() const { return atof(s_.c_str()); }

MString MString::expandEnvironmentVariablesAndTilde(MStatus* st) const {
    std::string out;
    for (size_t i = 0; i < s_.size(); ++i) {
        if (s_[i] == '$') {
            size_t j = i + 1;
            bool brace = j < s_.size() && s_[j] == '{';
            if (brace) ++j;
            size_t k = j;
            while (k < s_.size() && (isalnum((unsigned char)s_[k]) || s_[k] == '_')) ++k;
            const char* v = getenv(s_.substr(j, k - j).c_str());
            if (v) out += v;
            if (brace && k < s_.size() && s_[k] == '}') ++k;
            i = k - 1;
        } else if (s_[i] == '~' && i == 0) {
            const char* h = getenv("HOME");
            if (!h) h = getenv("USERPROFILE");
            out += h ? h : "~";
        } else {
            out += s_[i];
        }
    }
    if (st) *st = MS::kSuccess;
    return MString(out);
}

MString MString::expandFilePath(MStatus* st) const { return expandEnvironmentVariablesAndTilde(st); }

static MString& replaceIn(std::string& s, const std::string& f, const std::string& w, bool all, MString& self) {
    if (f.empty()) return self;
    size_t p = 0;
    while ((p = s.find(f, p)) != std::string::npos) {
        s.replace(p, f.size(), w);
        p += w.size();
        if (!all) break;
    }
    return self;
}

MString& MString::substitute(const MString& f, const MString& w) { return replaceIn(s_, f.s_, w.s_, true, *this); }
MString& MString::substituteFirst(const MString& f, const MString& w) { return replaceIn(s_, f.s_, w.s_, false, *this); }
MString& MString::substituteW(const MString& f, const MString& w) { return substitute(f, w); }
MString& MString::substituteFirstW(const MString& f, const MString& w) { return substituteFirst(f, w); }
bool MString::isEmpty() const { return s_.empty(); }

MStatus MString::format(const MString& fmt, const MString& a1, const MString& a2, const MString& a3, const MString& a4,
                        const MString& a5, const MString& a6, const MString& a7, const MString& a8, const MString& a9,
                        const MString& a10) {
    const MString* args[10] = {&a1, &a2, &a3, &a4, &a5, &a6, &a7, &a8, &a9, &a10};
    std::string out;
    const std::string& f = fmt.s_;
    for (size_t i = 0; i < f.size(); ++i) {
        // ^1s stands for the first argument; ^^ for a caret.
        if (f[i] == '^' && i + 1 < f.size()) {
            if (f[i + 1] == '^') { out += '^'; ++i; continue; }
            size_t j = i + 1;
            int n = 0;
            while (j < f.size() && isdigit((unsigned char)f[j])) n = n * 10 + (f[j++] - '0');
            if (n >= 1 && n <= 10 && j < f.size() && f[j] == 's') {
                out += args[n - 1]->s_;
                i = j;
                continue;
            }
        }
        out += f[i];
    }
    s_ = out;
    return MS::kSuccess;
}

MString operator+(const char* a, const MString& b) { return MString(a) + b; }
std::ostream& operator<<(std::ostream& os, const MString& s) { return os << s.asChar(); }

std::ostream& operator<<(std::ostream& os, const MIntArray& a) {
    os << "[";
    for (unsigned i = 0; i < a.length(); ++i) os << (i ? ", " : "") << a[i];
    return os << "]";
}
std::ostream& operator<<(std::ostream& os, const MDoubleArray& a) {
    os << "[";
    for (unsigned i = 0; i < a.length(); ++i) os << (i ? ", " : "") << a[i];
    return os << "]";
}

// ---------------------------------------------------------------------------------------------------------------------
// MTypeId

MTypeId::MTypeId() : id_(0) {}
MTypeId::MTypeId(unsigned int id) : id_(id) {}
MTypeId::MTypeId(unsigned int prefix, unsigned int id) : id_((prefix << 8) | (id & 0xFF)) {}
MTypeId::MTypeId(const MTypeId& o) : id_(o.id_) {}
MTypeId& MTypeId::operator=(const MTypeId& o) { id_ = o.id_; return *this; }
bool MTypeId::operator==(const MTypeId& o) const { return id_ == o.id_; }
bool MTypeId::operator!=(const MTypeId& o) const { return id_ != o.id_; }
unsigned int MTypeId::id(MStatus* st) const { if (st) *st = MS::kSuccess; return id_; }

// ---------------------------------------------------------------------------------------------------------------------
// MObject

const MObject MObject::kNullObj;

MObject::MObject() {}
MObject::MObject(const MObject& o) : d_(o.d_) {}
MObject::MObject(const std::shared_ptr<ObjectData>& d) : d_(d) {}
MObject::~MObject() {}
MObject& MObject::operator=(const MObject& o) { d_ = o.d_; return *this; }
bool MObject::isNull() const { return !d_ || d_->type == MFn::kInvalid; }

MFn::Type MObject::apiType() const {
    if (!d_) return MFn::kInvalid;
    if (d_->attr) return d_->attr->fn;
    if (nodeOf(*this) >= 0) return fnOfNode(d_->node);
    return d_->type;
}

bool MObject::hasFn(MFn::Type t) const {
    if (!d_) return t == MFn::kInvalid;
    if (t == MFn::kBase) return true;
    if (d_->attr) return t == MFn::kAttribute || t == d_->attr->fn;
    if (nodeOf(*this) >= 0) return nodeHasFn(d_->node, t);
    MFn::Type mine = d_->type;
    if (t == mine) return true;
    switch (t) {
    case MFn::kData: return mine == MFn::kMeshData || mine == MFn::kNumericData || mine == MFn::kMatrixData || mine == MFn::kStringData;
    case MFn::kGeometryData: return mine == MFn::kMeshData;
    case MFn::kComponent: return mine >= MFn::kSingleIndexedComponent && mine <= MFn::kCurveEPComponent;
    case MFn::kMeshComponent: return mine >= MFn::kMeshVertComponent && mine <= MFn::kMeshVtxFaceComponent;
    case MFn::kSingleIndexedComponent:
        return (mine >= MFn::kMeshVertComponent && mine <= MFn::kMeshMapComponent) || mine == MFn::kCurveCVComponent || mine == MFn::kCurveEPComponent;
    case MFn::kDoubleIndexedComponent: return mine == MFn::kMeshVtxFaceComponent;
    default: return false;
    }
}

const char* MObject::apiTypeStr() const { return fnName(apiType()); }

bool MObject::operator==(const MObject& o) const {
    if (d_ == o.d_) return true;
    if (isNull() && o.isNull()) return true;
    if (!d_ || !o.d_) return false;
    if (d_->attr || o.d_->attr) return d_->attr == o.d_->attr;
    if (nodeOf(*this) >= 0 || nodeOf(o) >= 0) return nodeOf(*this) == nodeOf(o);
    return false;
}
bool MObject::operator!=(const MObject& o) const { return !(*this == o); }

MObjectHandle::MObjectHandle() {}
MObjectHandle::MObjectHandle(const MObject& o) : o_(o) {}
MObject MObjectHandle::object() const { return isAlive() ? o_ : MObject(); }
const MObject& MObjectHandle::objectRef() const { return o_; }
bool MObjectHandle::isValid() const { return isAlive(); }
bool MObjectHandle::isAlive() const {
    int64_t n = nodeOf(o_);
    if (n < 0) return !o_.isNull();
    Json a = Json::object();
    a.set("id", n);
    return query("node.exists", a).truth();
}
unsigned int MObjectHandle::hashCode() const {
    int64_t n = nodeOf(o_);
    if (n >= 0) return (unsigned int)n;
    return (unsigned int)(std::uintptr_t)o_.azData().get();
}
bool MObjectHandle::operator==(const MObjectHandle& o) const { return o_ == o.o_; }
bool MObjectHandle::operator==(const MObject& o) const { return o_ == o; }
bool MObjectHandle::operator!=(const MObjectHandle& o) const { return !(o_ == o.o_); }

// ---------------------------------------------------------------------------------------------------------------------
// Units

double MTime::framesPerSecond(Unit u) {
    switch (u) {
    case kHours: return 1.0 / 3600.0;
    case kMinutes: return 1.0 / 60.0;
    case kSeconds: return 1.0;
    case kMilliseconds: return 1000.0;
    case k15FPS: return 15.0;
    case k24FPS: return 24.0;
    case k25FPS: return 25.0;
    case k30FPS: return 30.0;
    case k48FPS: return 48.0;
    case k50FPS: return 50.0;
    case k60FPS: return 60.0;
    case k120FPS: return 120.0;
    default: return 24.0;
    }
}

MTime::MTime() : seconds_(0), unit_(uiUnit()) {}
MTime::MTime(double v, Unit u) : unit_(u == kInvalid ? uiUnit() : u) { seconds_ = v / framesPerSecond(unit_); }
MTime::MTime(const MTime& o) : seconds_(o.seconds_), unit_(o.unit_) {}
MTime& MTime::operator=(const MTime& o) { seconds_ = o.seconds_; unit_ = o.unit_; return *this; }
MTime::Unit MTime::unit() const { return unit_; }
double MTime::value() const { return seconds_ * framesPerSecond(unit_); }
MStatus MTime::setUnit(Unit u) { if (u == kInvalid || u >= kLast) return MS::kInvalidParameter; unit_ = u; return MS::kSuccess; }
MStatus MTime::setValue(double v) { seconds_ = v / framesPerSecond(unit_); return MS::kSuccess; }
double MTime::as(Unit u) const { return seconds_ * framesPerSecond(u); }
double MTime::asUnits(Unit u) const { return as(u); }
MTime::Unit MTime::uiUnit() { return az::uiTimeUnit; }
MStatus MTime::setUIUnit(Unit u) {
    static const char* names[] = {"", "hour", "min", "sec", "millisec", "game", "film", "pal", "ntsc", "show", "palf", "ntscf", "120fps"};
    if (u <= kInvalid || u > k120FPS) return MS::kInvalidParameter;
    MStatus st;
    Json f = Json::object();
    f.set("time", names[u]);
    command("currentUnit", Json::array(), f, &st);
    if (st) az::uiTimeUnit = u;
    return st;
}
MTime MTime::operator+(const MTime& o) const { MTime r(*this); r.seconds_ += o.seconds_; return r; }
MTime MTime::operator-(const MTime& o) const { MTime r(*this); r.seconds_ -= o.seconds_; return r; }
MTime MTime::operator*(double d) const { MTime r(*this); r.seconds_ *= d; return r; }
MTime MTime::operator/(double d) const { MTime r(*this); r.seconds_ /= d; return r; }
MTime& MTime::operator+=(const MTime& o) { seconds_ += o.seconds_; return *this; }
MTime& MTime::operator-=(const MTime& o) { seconds_ -= o.seconds_; return *this; }
bool MTime::operator==(const MTime& o) const { return std::fabs(seconds_ - o.seconds_) < 1e-9; }
bool MTime::operator!=(const MTime& o) const { return !(*this == o); }
bool MTime::operator<(const MTime& o) const { return seconds_ < o.seconds_ && !(*this == o); }
bool MTime::operator>(const MTime& o) const { return seconds_ > o.seconds_ && !(*this == o); }
bool MTime::operator<=(const MTime& o) const { return !(*this > o); }
bool MTime::operator>=(const MTime& o) const { return !(*this < o); }

static const double kPi = 3.14159265358979323846;

static double radiansPer(MAngle::Unit u) {
    switch (u) {
    case MAngle::kDegrees: return kPi / 180.0;
    case MAngle::kAngMinutes: return kPi / 10800.0;
    case MAngle::kAngSeconds: return kPi / 648000.0;
    default: return 1.0;
    }
}

MAngle::MAngle() : radians_(0), unit_(kRadians) {}
MAngle::MAngle(double v, Unit u) : unit_(u == kInvalid ? kRadians : u) { radians_ = v * radiansPer(unit_); }
MAngle::MAngle(const MAngle& o) : radians_(o.radians_), unit_(o.unit_) {}
MAngle& MAngle::operator=(const MAngle& o) { radians_ = o.radians_; unit_ = o.unit_; return *this; }
MAngle::Unit MAngle::unit() const { return unit_; }
double MAngle::value() const { return radians_ / radiansPer(unit_); }
MStatus MAngle::setUnit(Unit u) { if (u == kInvalid || u >= kLast) return MS::kInvalidParameter; unit_ = u; return MS::kSuccess; }
MStatus MAngle::setValue(double v) { radians_ = v * radiansPer(unit_); return MS::kSuccess; }
double MAngle::as(Unit u) const { return radians_ / radiansPer(u); }
double MAngle::asUnits(Unit u) const { return as(u); }
double MAngle::asRadians() const { return radians_; }
double MAngle::asDegrees() const { return as(kDegrees); }
double MAngle::asAngMinutes() const { return as(kAngMinutes); }
double MAngle::asAngSeconds() const { return as(kAngSeconds); }
MAngle::Unit MAngle::uiUnit() { return kDegrees; }
MStatus MAngle::setUIUnit(Unit u) { return u == kDegrees ? MStatus(MS::kSuccess) : MStatus(MS::kNotImplemented); }
MAngle::Unit MAngle::internalUnit() { return kRadians; }

static double cmPer(MDistance::Unit u) {
    switch (u) {
    case MDistance::kInches: return 2.54;
    case MDistance::kFeet: return 30.48;
    case MDistance::kYards: return 91.44;
    case MDistance::kMiles: return 160934.4;
    case MDistance::kMillimeters: return 0.1;
    case MDistance::kKilometers: return 100000.0;
    case MDistance::kMeters: return 100.0;
    default: return 1.0;
    }
}

MDistance::MDistance() : cm_(0), unit_(kCentimeters) {}
MDistance::MDistance(double v, Unit u) : unit_(u == kInvalid ? kCentimeters : u) { cm_ = v * cmPer(unit_); }
MDistance::MDistance(const MDistance& o) : cm_(o.cm_), unit_(o.unit_) {}
MDistance& MDistance::operator=(const MDistance& o) { cm_ = o.cm_; unit_ = o.unit_; return *this; }
MDistance::Unit MDistance::unit() const { return unit_; }
double MDistance::value() const { return cm_ / cmPer(unit_); }
MStatus MDistance::setUnit(Unit u) { if (u == kInvalid || u >= kLast) return MS::kInvalidParameter; unit_ = u; return MS::kSuccess; }
MStatus MDistance::setValue(double v) { cm_ = v * cmPer(unit_); return MS::kSuccess; }
double MDistance::as(Unit u) const { return cm_ / cmPer(u); }
double MDistance::asUnits(Unit u) const { return as(u); }
double MDistance::asCentimeters() const { return cm_; }
double MDistance::asMeters() const { return as(kMeters); }
double MDistance::asMillimeters() const { return as(kMillimeters); }
double MDistance::asInches() const { return as(kInches); }
double MDistance::asFeet() const { return as(kFeet); }
MDistance::Unit MDistance::uiUnit() { return kCentimeters; }
MStatus MDistance::setUIUnit(Unit u) { return u == kCentimeters ? MStatus(MS::kSuccess) : MStatus(MS::kNotImplemented); }
MDistance::Unit MDistance::internalUnit() { return kCentimeters; }
