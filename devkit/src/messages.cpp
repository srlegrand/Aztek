// Aztek plug-in kit: messages. Each callback is one of the host's (callback_add), told what to hear with message.watch;
// the host calls it with what happened as JSON, and the trampoline below calls the plug-in's function with it.
#include "internal.h"
#include <aztek/kit/messages.h>
#include <atomic>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <map>
#include <mutex>
#include <random>

using namespace az;

// ---------------------------------------------------------------------------------------------------------------------
// MFileObject

MFileObject::MFileObject() {}
MFileObject::MFileObject(const MFileObject& o) : full_(o.full_), resolved_(o.resolved_) {}
MFileObject::~MFileObject() {}
MFileObject& MFileObject::operator=(const MFileObject& o) {
    full_ = o.full_;
    resolved_ = o.resolved_;
    return *this;
}

static std::string slashed(std::string s) {
    for (char& c : s)
        if (c == '\\') c = '/';
    return s;
}

// $VAR and ${VAR} from the environment, as the reference expands a path.
static std::string expanded(const std::string& in) {
    std::string out;
    for (size_t i = 0; i < in.size(); ++i) {
        if (in[i] != '$') {
            out += in[i];
            continue;
        }
        size_t start = i + 1, end;
        bool braced = start < in.size() && in[start] == '{';
        if (braced) {
            end = in.find('}', start);
            if (end == std::string::npos) {
                out += in.substr(i);
                break;
            }
            ++start;
        } else {
            end = start;
            while (end < in.size() && (isalnum((unsigned char)in[end]) || in[end] == '_')) ++end;
        }
        std::string name = in.substr(start, end - start);
        const char* v = name.empty() ? 0 : getenv(name.c_str());
        out += v ? v : "";
        i = braced ? end : end - 1;
    }
    return out;
}

static std::string dirOf(const std::string& p) {
    size_t at = p.rfind('/');
    return at == std::string::npos ? std::string() : p.substr(0, at + 1);
}
static std::string baseOf(const std::string& p) {
    size_t at = p.rfind('/');
    return at == std::string::npos ? p : p.substr(at + 1);
}

MStatus MFileObject::setRawName(const MString& name) {
    full_ = dirOf(full_) + slashed(name.asChar());
    resolved_.clear();
    return MS::kSuccess;
}
MStatus MFileObject::setRawPath(const MString& path) {
    std::string p = slashed(path.asChar());
    if (!p.empty() && p[p.size() - 1] != '/') p += '/';
    full_ = p + baseOf(full_);
    resolved_.clear();
    return MS::kSuccess;
}
MStatus MFileObject::setRawFullName(const MString& fullName) {
    full_ = slashed(fullName.asChar());
    resolved_.clear();
    return MS::kSuccess;
}
MStatus MFileObject::setRawURI(const MString& uri) {
    std::string u = uri.asChar();
    if (u.compare(0, 7, "file://") == 0) u = u.substr(7);
    return setRawFullName(MString(u.c_str()));
}
MStatus MFileObject::setFullName(const MString& fullName) { return setRawFullName(fullName); }
MStatus MFileObject::setName(const MString& name) { return setRawName(name); }
MStatus MFileObject::overrideResolvedFullName(const MString& fullName, bool) {
    resolved_ = slashed(fullName.asChar());
    return MS::kSuccess;
}
MString MFileObject::rawName() const { return MString(baseOf(full_).c_str()); }
MString MFileObject::rawPath() const { return MString(dirOf(full_).c_str()); }
MString MFileObject::rawFullName() const { return MString(full_.c_str()); }
MString MFileObject::rawURI() const { return MString(("file://" + expanded(full_)).c_str()); }
MString MFileObject::expandedPath() const { return MString(dirOf(expanded(full_)).c_str()); }
MString MFileObject::expandedFullName() const { return MString(expanded(full_).c_str()); }
MString MFileObject::resolvedFullName() const {
    if (!resolved_.empty()) return MString(resolved_.c_str());
    std::string e = expanded(full_);
    FILE* f = e.empty() ? 0 : fopen(e.c_str(), "rb");
    if (!f) return MString();
    fclose(f);
    return MString(e.c_str());
}
MString MFileObject::resolvedName() const { return MString(baseOf(resolvedFullName().asChar()).c_str()); }
MString MFileObject::resolvedPath() const { return MString(dirOf(resolvedFullName().asChar()).c_str()); }
MString MFileObject::name() const { return rawName(); }
MString MFileObject::path() const { return rawPath(); }
MString MFileObject::fullName() const { return expandedFullName(); }
bool MFileObject::exists() const { return resolvedFullName().length() > 0; }
bool MFileObject::isSet() const { return !full_.empty(); }

// ---------------------------------------------------------------------------------------------------------------------
// MUuid

MUuid::MUuid() : valid_(false) { memset(b_, 0, 16); }
MUuid::MUuid(const MUuid& o) : valid_(o.valid_) { memcpy(b_, o.b_, 16); }
MUuid::MUuid(const MString& s, MStatus* st) : valid_(false) {
    memset(b_, 0, 16);
    std::string hex;
    for (const char* c = s.asChar(); *c; ++c)
        if (*c != '-') hex += *c;
    bool ok = hex.size() == 32;
    for (int i = 0; ok && i < 16; ++i) {
        unsigned int v = 0;
        ok = sscanf(hex.c_str() + 2 * i, "%2x", &v) == 1;
        b_[i] = (unsigned char)v;
    }
    valid_ = ok;
    if (st) *st = ok ? MS::kSuccess : MS::kInvalidParameter;
}
MUuid::MUuid(const unsigned char* bytes, MStatus* st) : valid_(bytes != 0) {
    if (bytes)
        memcpy(b_, bytes, 16);
    else
        memset(b_, 0, 16);
    if (st) *st = bytes ? MS::kSuccess : MS::kInvalidParameter;
}
MUuid& MUuid::operator=(const MUuid& o) {
    memcpy(b_, o.b_, 16);
    valid_ = o.valid_;
    return *this;
}
bool MUuid::operator==(const MUuid& o) const { return valid_ == o.valid_ && memcmp(b_, o.b_, 16) == 0; }
bool MUuid::operator!=(const MUuid& o) const { return !(*this == o); }
MStatus MUuid::copy(const MUuid& o) {
    *this = o;
    return MS::kSuccess;
}
MStatus MUuid::get(unsigned char* bytes) const {
    if (!bytes) return MS::kInvalidParameter;
    memcpy(bytes, b_, 16);
    return MS::kSuccess;
}
MString MUuid::asString() const {
    if (!valid_) return MString();
    char out[37];
    char* p = out;
    for (int i = 0; i < 16; ++i) {
        if (i == 4 || i == 6 || i == 8 || i == 10) *p++ = '-';
        snprintf(p, 3, "%02X", b_[i]);
        p += 2;
    }
    *p = 0;
    return MString(out);
}
void MUuid::generate() {
    static std::mt19937_64 rng(std::random_device{}());
    for (int i = 0; i < 16; ++i) b_[i] = (unsigned char)(rng() & 0xff);
    b_[6] = (unsigned char)((b_[6] & 0x0f) | 0x40);
    b_[8] = (unsigned char)((b_[8] & 0x3f) | 0x80);
    valid_ = true;
}
bool MUuid::valid() const { return valid_; }

// ---------------------------------------------------------------------------------------------------------------------
// Callbacks

namespace {

enum Kind {
    kBasic,
    kCheck,
    kCheckFile,
    kStrings,
    kReference,
    kCheckReference,
    kConnFail,
    kTwoStrings,
    kTime,
    kNode,
    kPlugs,
    kUuidCheck,
    kParentChild,
    kMsgParentChild,
    kMatrix,
    kAttr2,
    kAttr1,
    kNodePlug,
    kNodeString,
    kNodeUuid,
    kNodeModifier,
    kModifier,
    kKeyable,
    kEvent,
};

struct Reg {
    Kind kind;
    void* fn;
    void* client;
    MCallbackId id = 0;
    int job = -1;  // an event callback's script job
    int64_t node = -1;
};

std::mutex regLock;
std::map<MCallbackId, Reg*>& regs() {
    static std::map<MCallbackId, Reg*> m;
    return m;
}
thread_local MCallbackId current = 0;

MPlug plugNamed(const Json& v) {
    MPlug p;
    std::string name = v.str();
    if (name.empty()) return p;
    MSelectionList sl;
    if (sl.add(MString(name.c_str())) && sl.length() > 0) sl.getPlug(0, p);
    return p;
}

MDagPath pathOf(const Json& v) {
    int64_t id = v.integer(0);
    return id > 0 ? MDagPath::azOf(id) : MDagPath();
}

MObject objectOf(const Json& v) {
    int64_t id = v.integer(-1);
    return id > 0 ? nodeObject(id) : MObject();
}

MTime timeOf(const Json& v) {
    MTime t(v.num(), MTime::kSeconds);
    t.setUnit(MTime::uiUnit());
    return t;
}

void reply(bool ret, const MFileObject* file) {
    Json a = Json::object();
    a.set("retCode", Json(ret));
    if (file) a.set("file", std::string(file->rawFullName().asChar()));
    query("message.reply", a);
}

struct Call {
    Reg* r;
    const Json* p;
};

int run(void* ctx) {
    Call* c = (Call*)ctx;
    Reg* r = c->r;
    const Json& p = *c->p;
    void* cd = r->client;
    switch (r->kind) {
    case kBasic:
    case kEvent:
        ((MMessage::MBasicFunction)r->fn)(cd);
        break;
    case kCheck: {
        bool ret = true;
        ((MMessage::MCheckFunction)r->fn)(&ret, cd);
        reply(ret, 0);
        break;
    }
    case kCheckFile: {
        bool ret = true;
        MFileObject f;
        f.setRawFullName(MString(p["file"].str().c_str()));
        ((MMessage::MCheckFileFunction)r->fn)(&ret, f, cd);
        reply(ret, &f);
        break;
    }
    case kStrings: {
        MStringArray a;
        for (const Json& s : p["strings"].items()) a.append(MString(s.str().c_str()));
        ((MMessage::MStringArrayFunction)r->fn)(a, cd);
        break;
    }
    case kReference: {
        MFileObject f;
        f.setRawFullName(MString(p["file"].str().c_str()));
        MObject n = objectOf(p["node"]);
        ((MMessage::MObjectFileFunction)r->fn)(n, f, cd);
        break;
    }
    case kCheckReference: {
        bool ret = true;
        MFileObject f;
        f.setRawFullName(MString(p["file"].str().c_str()));
        MObject n = objectOf(p["node"]);
        ((MMessage::MCheckObjectFileFunction)r->fn)(&ret, n, f, cd);
        reply(ret, &f);
        break;
    }
    case kTwoStrings: {
        const Json& s = p["strings"];
        ((MMessage::MTwoStringFunction)r->fn)(MString(s[(size_t)0].str().c_str()), MString(s[(size_t)1].str().c_str()), cd);
        break;
    }
    case kTime: {
        MTime t = timeOf(p["time"]);
        ((MMessage::MTimeFunction)r->fn)(t, cd);
        break;
    }
    case kNode: {
        MObject n = objectOf(p["node"]);
        ((MMessage::MNodeFunction)r->fn)(n, cd);
        break;
    }
    case kPlugs: {
        MPlug src = plugNamed(p["src"]), dst = plugNamed(p["dst"]);
        ((MMessage::MPlugFunction)r->fn)(src, dst, p["made"].truth(), cd);
        break;
    }
    case kParentChild: {
        MDagPath child = pathOf(p["child"]), parent = pathOf(p["parent"]);
        ((MMessage::MParentChildFunction)r->fn)(child, parent, cd);
        break;
    }
    case kMsgParentChild: {
        MDagPath child = pathOf(p["child"]), parent = pathOf(p["parent"]);
        ((MDagMessage::MMessageParentChildFunction)r->fn)((MDagMessage::DagMessage)p["msg"].integer(), child, parent, cd);
        break;
    }
    case kMatrix: {
        MObject n = objectOf(p["node"]);
        MDagMessage::MatrixModifiedFlags flags = (MDagMessage::MatrixModifiedFlags)p["flags"].integer();
        ((MDagMessage::MWorldMatrixModifiedFunction)r->fn)(n, flags, cd);
        break;
    }
    case kAttr2: {
        MPlug plug = plugNamed(p["plug"]), other = plugNamed(p["other"]);
        ((MNodeMessage::MAttr2PlugFunction)r->fn)((MNodeMessage::AttributeMessage)p["msg"].integer(), plug, other, cd);
        break;
    }
    case kAttr1: {
        MPlug plug = plugNamed(p["plug"]);
        ((MNodeMessage::MAttrPlugFunction)r->fn)((MNodeMessage::AttributeMessage)p["msg"].integer(), plug, cd);
        break;
    }
    case kNodePlug: {
        MObject n = objectOf(p["node"]);
        MPlug plug = plugNamed(p["plug"]);
        ((MMessage::MNodePlugFunction)r->fn)(n, plug, cd);
        break;
    }
    case kNodeString: {
        MObject n = objectOf(p["node"]);
        ((MMessage::MNodeStringFunction)r->fn)(n, MString(p["old"].str().c_str()), cd);
        break;
    }
    case kNodeUuid: {
        MObject n = objectOf(p["node"]);
        MUuid u(MString(p["uuid"].str().c_str()));
        ((MMessage::MNodeUuidFunction)r->fn)(n, u, cd);
        break;
    }
    case kNodeModifier: {
        MObject n = objectOf(p["node"]);
        MDGModifier mod;
        ((MMessage::MNodeModifierFunction)r->fn)(n, mod, cd);
        mod.doIt();
        break;
    }
    case kModifier: {
        MDGModifier mod;
        ((MMessage::MModifierFunction)r->fn)(mod, cd);
        mod.doIt();
        break;
    }
    case kConnFail:
    case kUuidCheck:
    case kKeyable:
        break;
    }
    return AZ_OK;
}

void trampoline(void* user, const char* payload) {
    Reg* r = (Reg*)user;
    if (!r || !r->fn) return;
    Json p = Json::parse(payload ? payload : "{}");
    MCallbackId was = current;
    current = r->id;
    Call c = {r, &p};
    std::string e;
    if (guarded(run, &c, e) == AZ_CRASHED) log("callback " + std::to_string((long long)r->id) + " crashed: " + e, 2);
    current = was;
}

MCallbackId add(Kind kind, void* fn, void* client, Json watch, MStatus* st, int64_t node = -1) {
    if (!fn) {
        if (st) *st = MS::kInvalidParameter;
        return 0;
    }
    Reg* r = new Reg;
    r->kind = kind;
    r->fn = fn;
    r->client = client;
    r->node = node;
    int64_t id = host->callback_add(pluginToken, trampoline, r);
    if (id <= 0) {
        delete r;
        if (st) *st = fail(MStatus::kFailure, "(kFailure): the host took no callback");
        return 0;
    }
    r->id = (MCallbackId)id;
    if (kind != kEvent) {
        watch.set("callback", Json((int64_t)id));
        MStatus s;
        query("message.watch", watch, &s);
        if (!s) {
            host->callback_remove(id);
            delete r;
            if (st) *st = s;
            return 0;
        }
    }
    {
        std::lock_guard<std::mutex> g(regLock);
        regs()[r->id] = r;
    }
    if (st) *st = MS::kSuccess;
    return r->id;
}

Json watchOf(const char* kind, int64_t msg = -1, int64_t node = -1) {
    Json w = Json::object();
    w.set("kind", kind);
    w.set("msg", Json(msg));
    w.set("node", Json(node));
    return w;
}

int64_t nodeArg(const MObject& o) { return nodeOf(o); }
int64_t pathArg(const MDagPath& p) { return p.azNode() > 0 ? p.azNode() : -1; }

}  // namespace

static std::atomic<long long> nextOwnId(1LL << 40);
MCallbackId MMessage::azNewId() { return (MCallbackId)nextOwnId++; }

MStatus MMessage::removeCallback(MCallbackId id) {
    Reg* r = 0;
    {
        std::lock_guard<std::mutex> g(regLock);
        std::map<MCallbackId, Reg*>::iterator it = regs().find(id);
        if (it == regs().end()) return (long long)id >= (1LL << 40) ? MStatus(MS::kSuccess) : fail(MStatus::kInvalidParameter, "(kInvalidParameter): no such callback");
        r = it->second;
        regs().erase(it);
    }
    if (r->job >= 0) az::mel("scriptJob -force -kill " + std::to_string(r->job) + ";");
    host->callback_remove((int64_t)id);
    // A callback may remove itself while it runs: what it was given stays, unused.
    r->fn = 0;
    return MS::kSuccess;
}

MStatus MMessage::removeCallbacks(MCallbackIdArray& ids) {
    MStatus out;
    for (unsigned int i = 0; i < ids.length(); ++i) {
        MStatus s = removeCallback(ids[i]);
        if (!s) out = s;
    }
    ids.clear();
    return out;
}

MStatus MMessage::removeCallbacks(MIntArray& ids) {
    MStatus out;
    for (unsigned int i = 0; i < ids.length(); ++i) {
        MStatus s = removeCallback((MCallbackId)ids[i]);
        if (!s) out = s;
    }
    ids.clear();
    return out;
}

MCallbackId MMessage::currentCallbackId(MStatus* st) {
    if (st) *st = current ? MS::kSuccess : MS::kFailure;
    return current;
}

MStatus MMessage::nodeCallbacks(MObject& node, MCallbackIdArray& ids) {
    Json a = Json::object();
    a.set("id", Json(nodeOf(node)));
    MStatus st;
    Json r = query("message.nodeCallbacks", a, &st);
    if (!st) return st;
    std::lock_guard<std::mutex> g(regLock);
    for (const Json& v : r.items())
        if (regs().count((MCallbackId)v.integer())) ids.append((MCallbackId)v.integer());
    return MS::kSuccess;
}

MStatus MMessage::nodeCallbacks(MObject& node, MIntArray& ids) {
    MCallbackIdArray all;
    MStatus st = nodeCallbacks(node, all);
    for (unsigned int i = 0; i < all.length(); ++i) ids.append((int)all[i]);
    return st;
}

static bool callableScript = false;
void MMessage::setRegisteringCallableScript() { callableScript = true; }
bool MMessage::registeringCallableScript() { return callableScript; }
void MMessage::stopRegisteringCallableScript() { callableScript = false; }
static MPluginCallableInfo* callableInfo = 0;
void MMessage::setCallableInfo(MPluginCallableInfo* info) { callableInfo = info; }
MPluginCallableInfo* MMessage::getCallableInfo() { return callableInfo; }
MPluginCallableInfo* MMessage::stealCallableInfo() {
    MPluginCallableInfo* i = callableInfo;
    callableInfo = 0;
    return i;
}

// MSceneMessage

#define SCENE(kind) add(kind, (void*)func, clientData, watchOf("scene", (int64_t)msg), status)
MCallbackId MSceneMessage::addCallback(Message msg, MMessage::MBasicFunction func, void* clientData, MStatus* status) { return SCENE(kBasic); }
MCallbackId MSceneMessage::addCheckCallback(Message msg, MMessage::MCheckFunction func, void* clientData, MStatus* status) { return SCENE(kCheck); }
MCallbackId MSceneMessage::addCheckFileCallback(Message msg, MMessage::MCheckFileFunction func, void* clientData, MStatus* status) { return SCENE(kCheckFile); }
MCallbackId MSceneMessage::addStringArrayCallback(Message msg, MMessage::MStringArrayFunction func, void* clientData, MStatus* status) { return SCENE(kStrings); }
MCallbackId MSceneMessage::addReferenceCallback(Message msg, MMessage::MObjectFileFunction func, void* clientData, MStatus* status) { return SCENE(kReference); }
MCallbackId MSceneMessage::addCheckReferenceCallback(Message msg, MMessage::MCheckObjectFileFunction func, void* clientData, MStatus* status) { return SCENE(kCheckReference); }
MCallbackId MSceneMessage::addCallback(Message msg, void (*func)(bool*, void*), void* clientData, MStatus* status) { return SCENE(kCheck); }
MCallbackId MSceneMessage::addCallback(Message msg, void (*func)(bool*, MFileObject&, void*), void* clientData, MStatus* status) { return SCENE(kCheckFile); }
MCallbackId MSceneMessage::addCallback(Message msg, void (*func)(const MStringArray&, void*), void* clientData, MStatus* status) { return SCENE(kStrings); }
#undef SCENE
MCallbackId MSceneMessage::addConnectionFailedCallback(MMessage::MConnFailFunction func, void* clientData, MStatus* status) { return add(kConnFail, (void*)func, clientData, watchOf("connectionFailed"), status); }
MCallbackId MSceneMessage::addNamespaceRenamedCallback(MMessage::MTwoStringFunction func, void* clientData, MStatus* status) { return add(kTwoStrings, (void*)func, clientData, watchOf("namespaceRenamed"), status); }

// MDGMessage

MCallbackId MDGMessage::addTimeChangeCallback(MMessage::MTimeFunction func, void* clientData, MStatus* status) { return add(kTime, (void*)func, clientData, watchOf("time", 0), status); }
MCallbackId MDGMessage::addDelayedTimeChangeCallback(MMessage::MTimeFunction func, void* clientData, MStatus* status) { return add(kTime, (void*)func, clientData, watchOf("time", 1), status); }
MCallbackId MDGMessage::addDelayedTimeChangeRunupCallback(MMessage::MTimeFunction func, void* clientData, MStatus* status) { return add(kTime, (void*)func, clientData, watchOf("time", 2), status); }
MCallbackId MDGMessage::addForceUpdateCallback(MMessage::MTimeFunction func, void* clientData, MStatus* status) { return add(kTime, (void*)func, clientData, watchOf("time", 3), status); }
MCallbackId MDGMessage::addNodeAddedCallback(MMessage::MNodeFunction func, const MString& nodeType, void* clientData, MStatus* status) {
    Json w = watchOf("nodeAdded");
    w.set("type", nodeType);
    return add(kNode, (void*)func, clientData, w, status);
}
MCallbackId MDGMessage::addNodeRemovedCallback(MMessage::MNodeFunction func, const MString& nodeType, void* clientData, MStatus* status) {
    Json w = watchOf("nodeRemoved");
    w.set("type", nodeType);
    return add(kNode, (void*)func, clientData, w, status);
}
MCallbackId MDGMessage::addConnectionCallback(MMessage::MPlugFunction func, void* clientData, MStatus* status) { return add(kPlugs, (void*)func, clientData, watchOf("connection"), status); }
MCallbackId MDGMessage::addPreConnectionCallback(MMessage::MPlugFunction func, void* clientData, MStatus* status) { return add(kPlugs, (void*)func, clientData, watchOf("preConnection"), status); }
MCallbackId MDGMessage::addNodeChangeUuidCheckCallback(MMessage::MCheckNodeUuidFunction func, void* clientData, MStatus* status) { return add(kUuidCheck, (void*)func, clientData, watchOf("uuidCheck"), status); }

// MDagMessage

#define DAG(m, nodeId, kind) add(kind, (void*)func, clientData, watchOf("dag", (int64_t)(m), nodeId), status, nodeId)
MCallbackId MDagMessage::addParentAddedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kParentAdded, -1, kParentChild); }
MCallbackId MDagMessage::addParentAddedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kParentAdded, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addParentRemovedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kParentRemoved, -1, kParentChild); }
MCallbackId MDagMessage::addParentRemovedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kParentRemoved, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addChildAddedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildAdded, -1, kParentChild); }
MCallbackId MDagMessage::addChildAddedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildAdded, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addChildRemovedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildRemoved, -1, kParentChild); }
MCallbackId MDagMessage::addChildRemovedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildRemoved, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addChildReorderedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildReordered, -1, kParentChild); }
MCallbackId MDagMessage::addChildReorderedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildReordered, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addDagCallback(DagMessage msgType, MDagMessage::MMessageParentChildFunction func, void* clientData, MStatus* status) { return DAG(msgType, -1, kMsgParentChild); }
MCallbackId MDagMessage::addDagDagPathCallback(MDagPath& node, DagMessage msgType, MDagMessage::MMessageParentChildFunction func, void* clientData, MStatus* status) { return DAG(msgType, pathArg(node), kMsgParentChild); }
MCallbackId MDagMessage::addAllDagChangesCallback(MDagMessage::MMessageParentChildFunction func, void* clientData, MStatus* status) { return DAG(-1, -1, kMsgParentChild); }
MCallbackId MDagMessage::addAllDagChangesDagPathCallback(MDagPath& node, MDagMessage::MMessageParentChildFunction func, void* clientData, MStatus* status) { return DAG(-1, pathArg(node), kMsgParentChild); }
MCallbackId MDagMessage::addInstanceAddedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kInstanceAdded, -1, kParentChild); }
MCallbackId MDagMessage::addInstanceAddedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kInstanceAdded, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addInstanceRemovedCallback(MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kInstanceRemoved, -1, kParentChild); }
MCallbackId MDagMessage::addInstanceRemovedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kInstanceRemoved, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addParentAddedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kParentAdded, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addParentRemovedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kParentRemoved, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addChildAddedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildAdded, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addChildRemovedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildRemoved, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addChildReorderedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kChildReordered, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addInstanceAddedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kInstanceAdded, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addInstanceRemovedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(kInstanceRemoved, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addDagCallback(DagMessage msgType, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(msgType, -1, kParentChild); }
MCallbackId MDagMessage::addDagCallback(MDagPath& node, DagMessage msgType, MMessage::MParentChildFunction func, void* clientData, MStatus* status) { return DAG(msgType, pathArg(node), kParentChild); }
MCallbackId MDagMessage::addDagCallback(MDagPath& node, DagMessage msgType, MDagMessage::MMessageParentChildFunction func, void* clientData, MStatus* status) { return DAG(msgType, pathArg(node), kMsgParentChild); }
MCallbackId MDagMessage::addAllDagChangesCallback(MDagPath& node, MDagMessage::MMessageParentChildFunction func, void* clientData, MStatus* status) { return DAG(-1, pathArg(node), kMsgParentChild); }
#undef DAG
MCallbackId MDagMessage::addMatrixModifiedCallback(MDagPath& affectedNode, MDagMessage::MWorldMatrixModifiedFunction func, void* clientData, MStatus* status) {
    return add(kMatrix, (void*)func, clientData, watchOf("matrix", -1, pathArg(affectedNode)), status, pathArg(affectedNode));
}
MCallbackId MDagMessage::addWorldMatrixModifiedCallback(MDagPath& affectedNode, MDagMessage::MWorldMatrixModifiedFunction func, void* clientData, MStatus* status) {
    return add(kMatrix, (void*)func, clientData, watchOf("worldMatrix", -1, pathArg(affectedNode)), status, pathArg(affectedNode));
}

// MNodeMessage

#define NODE(what, kind) add(kind, (void*)func, clientData, watchOf(what, -1, nodeArg(node)), status, nodeArg(node))
MCallbackId MNodeMessage::addAttributeChangedCallback(MObject& node, MNodeMessage::MAttr2PlugFunction func, void* clientData, MStatus* status) { return NODE("attributeChanged", kAttr2); }
MCallbackId MNodeMessage::addAttributeAddedOrRemovedCallback(MObject& node, MNodeMessage::MAttrPlugFunction func, void* clientData, MStatus* status) { return NODE("attributeAddedOrRemoved", kAttr1); }
MCallbackId MNodeMessage::addNodeDirtyCallback(MObject& node, MMessage::MNodeFunction func, void* clientData, MStatus* status) { return NODE("dirty", kNode); }
MCallbackId MNodeMessage::addNodeDirtyPlugCallback(MObject& node, MMessage::MNodePlugFunction func, void* clientData, MStatus* status) { return NODE("dirtyPlug", kNodePlug); }
MCallbackId MNodeMessage::addNameChangedCallback(MObject& node, MMessage::MNodeStringFunction func, void* clientData, MStatus* status) { return NODE("nameChanged", kNodeString); }
MCallbackId MNodeMessage::addUuidChangedCallback(MObject& node, MMessage::MNodeUuidFunction func, void* clientData, MStatus* status) { return NODE("uuidChanged", kNodeUuid); }
MCallbackId MNodeMessage::addNodeAboutToDeleteCallback(MObject& node, MMessage::MNodeModifierFunction func, void* clientData, MStatus* status) { return NODE("aboutToDelete", kNodeModifier); }
MCallbackId MNodeMessage::addNodePreRemovalCallback(MObject& node, MMessage::MNodeFunction func, void* clientData, MStatus* status) { return NODE("preRemoval", kNode); }
MCallbackId MNodeMessage::addNodeDestroyedCallback(MObject& node, MMessage::MBasicFunction func, void* clientData, MStatus* status) { return NODE("destroyed", kBasic); }
MCallbackId MNodeMessage::addNodeDirtyCallback(MObject& node, MMessage::MBasicFunction func, void* clientData, MStatus* status) { return NODE("dirty", kBasic); }
MCallbackId MNodeMessage::addNodeDirtyCallback(MObject& node, MMessage::MNodePlugFunction func, void* clientData, MStatus* status) { return NODE("dirtyPlug", kNodePlug); }
MCallbackId MNodeMessage::addNameChangedCallback(MObject& node, MMessage::MNodeFunction func, void* clientData, MStatus* status) { return NODE("nameChanged", kNode); }
MCallbackId MNodeMessage::addNodeAboutToDeleteCallback(MObject& node, MMessage::MModifierFunction func, void* clientData, MStatus* status) { return NODE("aboutToDelete", kModifier); }
#undef NODE
MCallbackId MNodeMessage::addKeyableChangeOverride(MPlug& plug, MNodeMessage::MKeyableFunction func, void* clientData, MStatus* status) {
    MObject node = plug.node();
    return add(kKeyable, (void*)func, clientData, watchOf("keyableOverride", -1, nodeArg(node)), status, nodeArg(node));
}

// MEventMessage: a script job on the event that calls the callback back.

MCallbackId MEventMessage::addEventCallback(const MString& event, MMessage::MBasicFunction func, void* clientData, MStatus* status) {
    MStatus st;
    MCallbackId id = add(kEvent, (void*)func, clientData, Json::object(), &st);
    if (!st) {
        if (status) *status = st;
        return 0;
    }
    // MEL, so the job's script is MEL too.
    Json job = az::mel("scriptJob -event " + quoted(event.asChar()) + " " + quoted("aztekPluginCallback " + std::to_string((long long)id)) + ";", &st);
    if (!st) {
        removeCallback(id);
        if (status) *status = st;
        return 0;
    }
    {
        std::lock_guard<std::mutex> g(regLock);
        std::map<MCallbackId, Reg*>::iterator it = regs().find(id);
        if (it != regs().end()) it->second->job = (int)job.integer(-1);
    }
    if (status) *status = MS::kSuccess;
    return id;
}

void MEventMessage::getEventNames(MStringArray& names) {
    Json flags = Json::object();
    flags.set("listEvents", Json(true));
    Json r = command("scriptJob", Json::array(), flags);
    for (const Json& v : r.items()) names.append(MString(v.str().c_str()));
}
