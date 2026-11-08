// Aztek plug-in kit: modifiers and the dependency node, DAG node and transform function sets.
#include "internal.h"

#include <cmath>

using namespace az;

namespace az {
std::vector<double> numbers(const Json& v);
Json args1(const std::string& a);
MPlug plugNamed(const std::string& full);
}

static const double kDeg = 180.0 / 3.14159265358979323846;

static void ok(MStatus* st) { if (st) *st = MS::kSuccess; }

static Json nodeInfo(int64_t id, MStatus* st = 0) {
    Json a = Json::object();
    a.set("id", id);
    MStatus s;
    Json info = query("node.info", a, &s);
    if (s && !info.isObject()) s = fail(MStatus::kInvalidParameter, "(kInvalidParameter): Object does not exist");
    if (st) *st = s;
    return info;
}

static int64_t idNamed(const Json& name) { return name.isArray() ? nodeNamed(name[(size_t)0].text()) : nodeNamed(name.text()); }

// ---------------------------------------------------------------------------------------------------------------------
// Modifiers: edits queued until doIt, undone in reverse by undoIt. Nodes are made at once, as the reference makes them, and go
// again on undoIt; the MObject given out follows the node when redone.

MDGModifier::MDGModifier() : done_(0) {}
MDGModifier::~MDGModifier() {}
void MDGModifier::azQueue(const std::function<MStatus()>& d, const std::function<MStatus()>& u) {
    Op op;
    op.do_ = d;
    op.undo_ = u;
    ops_.push_back(op);
}
MStatus MDGModifier::doIt() {
    MStatus result;
    for (; done_ < ops_.size(); ++done_) {
        MStatus st = ops_[done_].do_();
        if (!st) result = st;
    }
    return result;
}
MStatus MDGModifier::undoIt() {
    MStatus result;
    while (done_ > 0) {
        --done_;
        MStatus st = ops_[done_].undo_();
        if (!st) result = st;
    }
    return result;
}

MObject MDGModifier::azCreate(const std::string& type, int64_t parent, MStatus* st) {
    Json f = Json::object();
    f.set("skipSelect", true);
    if (parent >= 0) f.set("parent", nodePath(parent));
    MStatus s;
    Json made = command("createNode", args1(type), f, &s);
    if (st) *st = s;
    if (!s) return MObject();
    int64_t made_id = idNamed(made);
    // A shape made with no parent comes under a new transform, and that is what is given back, as the reference does.
    if (parent < 0) {
        Json info = nodeInfo(made_id);
        int64_t up = info["parent"].integer(0);
        bool shape = false;
        for (size_t i = 0; i < info["chain"].size(); ++i) shape = shape || info["chain"][i].str() == "shape";
        if (shape && up > 0) made_id = up;
    }
    MObject obj = nodeObject(made_id);
    std::shared_ptr<ObjectData> d = obj.azData();
    std::shared_ptr<bool> exists = std::make_shared<bool>(true);
    std::shared_ptr<std::string> name = std::make_shared<std::string>(nodeInfo(d->node)["name"].str());
    azQueue(
        [d, exists, type, f, name]() -> MStatus {
            if (*exists) return MS::kSuccess;
            MStatus s2;
            Json m = command("createNode", args1(type), f, &s2);
            if (!s2) return s2;
            int64_t id = idNamed(m);
            Json info = nodeInfo(id);
            bool shape = false;
            for (size_t i = 0; i < info["chain"].size(); ++i) shape = shape || info["chain"][i].str() == "shape";
            if (shape && !f.has("parent") && info["parent"].integer(0) > 0) id = info["parent"].integer(0);
            d->node = id;
            Json r = args1(nodePath(id));
            r.push(*name);
            command("rename", r, Json::object(), &s2);
            *exists = true;
            return s2;
        },
        [d, exists, name]() -> MStatus {
            if (!*exists) return MS::kSuccess;
            *name = nodeInfo(d->node)["name"].str();
            MStatus s2;
            command("delete", args1(nodePath(d->node)), Json::object(), &s2);
            *exists = false;
            return s2;
        });
    return obj;
}

static std::string typeOfId(const MTypeId& id) {
    NodeClass* c = classOfId(id);
    return c ? c->name : std::string();
}

MObject MDGModifier::createNode(const MString& type, MStatus* st) { return azCreate(type.asChar(), -1, st); }
MObject MDGModifier::createNode(const MTypeId& id, MStatus* st) {
    std::string t = typeOfId(id);
    if (t.empty()) {
        if (st) *st = fail(MStatus::kInvalidParameter, "createNode: no node type has that id");
        return MObject();
    }
    return azCreate(t, -1, st);
}

MStatus MDGModifier::deleteNode(const MObject& node, bool) {
    std::shared_ptr<ObjectData> d = node.azData();
    if (!d || d->node < 0) return MS::kInvalidParameter;
    Json info = nodeInfo(d->node);
    std::string type = info["type"].str(), name = info["name"].str();
    int64_t parent = info["parent"].integer(-1);
    std::string parentPath = info["dag"].truth() && parent > 0 ? nodePath(parent) : std::string();
    azQueue(
        [d]() -> MStatus {
            MStatus s;
            command("delete", args1(nodePath(d->node)), Json::object(), &s);
            return s;
        },
        [d, type, name, parentPath]() -> MStatus {
            Json f = Json::object();
            f.set("name", name).set("skipSelect", true);
            if (!parentPath.empty()) f.set("parent", parentPath);
            MStatus s;
            Json m = command("createNode", args1(type), f, &s);
            if (s) d->node = idNamed(m);
            return s;
        });
    return MS::kSuccess;
}

MStatus MDGModifier::renameNode(const MObject& node, const MString& name) {
    std::shared_ptr<ObjectData> d = node.azData();
    if (!d || d->node < 0) return MS::kInvalidParameter;
    std::string to = name.asChar();
    std::shared_ptr<std::string> old = std::make_shared<std::string>();
    azQueue(
        [d, to, old]() -> MStatus {
            *old = nodeInfo(d->node)["name"].str();
            MStatus s;
            Json a = args1(nodePath(d->node));
            a.push(to);
            command("rename", a, Json::object(), &s);
            return s;
        },
        [d, old]() -> MStatus {
            MStatus s;
            Json a = args1(nodePath(d->node));
            a.push(*old);
            command("rename", a, Json::object(), &s);
            return s;
        });
    return MS::kSuccess;
}

static MStatus connectNow(const MPlug& a, const MPlug& b, bool on) {
    Json args = args1(a.azPath());
    args.push(b.azPath());
    MStatus s;
    command(on ? "connectAttr" : "disconnectAttr", args, Json::object(), &s);
    return s;
}

MStatus MDGModifier::connect(const MPlug& src, const MPlug& dst) {
    if (src.isNull() || dst.isNull()) return MS::kInvalidParameter;
    azQueue([src, dst]() { return connectNow(src, dst, true); }, [src, dst]() { return connectNow(src, dst, false); });
    return MS::kSuccess;
}
MStatus MDGModifier::connect(const MObject& sn, const MObject& sa, const MObject& dn, const MObject& da) { return connect(MPlug(sn, sa), MPlug(dn, da)); }
MStatus MDGModifier::disconnect(const MPlug& src, const MPlug& dst) {
    if (src.isNull() || dst.isNull()) return MS::kInvalidParameter;
    azQueue([src, dst]() { return connectNow(src, dst, false); }, [src, dst]() { return connectNow(src, dst, true); });
    return MS::kSuccess;
}
MStatus MDGModifier::disconnect(const MObject& sn, const MObject& sa, const MObject& dn, const MObject& da) { return disconnect(MPlug(sn, sa), MPlug(dn, da)); }

MStatus MDGModifier::addAttribute(const MObject& node, const MObject& attr) {
    MObject n = node, a = attr;
    azQueue([n, a]() { return MFnDependencyNode(n).addAttribute(a); }, [n, a]() { return MFnDependencyNode(n).removeAttribute(a); });
    return MS::kSuccess;
}
MStatus MDGModifier::removeAttribute(const MObject& node, const MObject& attr) {
    MObject n = node, a = attr;
    azQueue([n, a]() { return MFnDependencyNode(n).removeAttribute(a); }, [n, a]() { return MFnDependencyNode(n).addAttribute(a); });
    return MS::kSuccess;
}

// The plug's value now, put back on undo.
static MStatus queueValue(MDGModifier* m, std::vector<MDGModifier::Op>& ops, const MPlug& p, const std::function<MStatus(MPlug&)>& set) {
    (void)m;
    MPlug plug = p;
    MObject before = plug.asMObject();
    MDGModifier::Op op;
    op.do_ = [plug, set]() mutable { return set(plug); };
    op.undo_ = [plug, before]() mutable { return plug.setValue(before); };
    ops.push_back(op);
    return MS::kSuccess;
}

MStatus MDGModifier::newPlugValue(const MPlug& p, const MObject& d) { MObject v = d; return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueBool(const MPlug& p, bool v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueChar(const MPlug& p, char v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setChar(v); }); }
MStatus MDGModifier::newPlugValueShort(const MPlug& p, short v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueInt(const MPlug& p, int v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueInt64(const MPlug& p, int64_t v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setInt64(v); }); }
MStatus MDGModifier::newPlugValueFloat(const MPlug& p, float v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueDouble(const MPlug& p, double v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueString(const MPlug& p, const MString& v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueMAngle(const MPlug& p, const MAngle& v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueMDistance(const MPlug& p, const MDistance& v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::newPlugValueMTime(const MPlug& p, const MTime& v) { return queueValue(this, ops_, p, [v](MPlug& x) { return x.setValue(v); }); }
MStatus MDGModifier::commandToExecute(const MString& c) {
    std::string src = c.asChar();
    azQueue(
        [src]() -> MStatus {
            MStatus s;
            mel(src, &s);
            return s;
        },
        []() -> MStatus { return MS::kSuccess; });
    return MS::kSuccess;
}
MStatus MDGModifier::pythonCommandToExecute(const MString& c) {
    std::string src = c.asChar();
    azQueue(
        [src]() -> MStatus {
            MStatus s;
            python(src, &s);
            return s;
        },
        []() -> MStatus { return MS::kSuccess; });
    return MS::kSuccess;
}
MStatus MDGModifier::setNodeLockState(const MObject& node, bool locked) {
    MObject n = node;
    azQueue([n, locked]() { return MFnDependencyNode(n).setLocked(locked); }, [n, locked]() { return MFnDependencyNode(n).setLocked(!locked); });
    return MS::kSuccess;
}

MDagModifier::MDagModifier() {}
MDagModifier::~MDagModifier() {}
MObject MDagModifier::createNode(const MString& type, const MObject& parent, MStatus* st) {
    int64_t p = nodeOf(parent);
    return azCreate(type.asChar(), p, st);
}
MObject MDagModifier::createNode(const MTypeId& id, const MObject& parent, MStatus* st) {
    std::string t = typeOfId(id);
    if (t.empty()) {
        if (st) *st = fail(MStatus::kInvalidParameter, "createNode: no node type has that id");
        return MObject();
    }
    return azCreate(t, nodeOf(parent), st);
}
static MStatus parentTo(int64_t node, int64_t parent) {
    MStatus s;
    Json a = args1(nodePath(node));
    Json f = Json::object();
    if (parent > 0) a.push(nodePath(parent));
    else f.set("world", true);
    command("parent", a, f, &s);
    return s;
}
MStatus MDagModifier::reparentNode(const MObject& node, const MObject& newParent) {
    std::shared_ptr<ObjectData> d = node.azData();
    if (!d || d->node < 0) return MS::kInvalidParameter;
    int64_t to = nodeOf(newParent);
    std::shared_ptr<int64_t> was = std::make_shared<int64_t>(0);
    azQueue(
        [d, to, was]() {
            *was = nodeInfo(d->node)["parent"].integer(0);
            return parentTo(d->node, to);
        },
        [d, was]() { return parentTo(d->node, *was); });
    return MS::kSuccess;
}

// ---------------------------------------------------------------------------------------------------------------------
// MFnDependencyNode

MFnDependencyNode::MFnDependencyNode() {}
MFnDependencyNode::MFnDependencyNode(const MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = nodeOf(o) >= 0 ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MFnDependencyNode::~MFnDependencyNode() {}
MFn::Type MFnDependencyNode::type() const { return MFn::kDependencyNode; }
int64_t MFnDependencyNode::azId(MStatus* st) const {
    int64_t id = nodeOf(obj_);
    if (st) *st = id >= 0 ? MStatus(MS::kSuccess) : fail(MStatus::kInvalidParameter, "(kInvalidParameter): Object is incompatible with this method");
    return id;
}
MObject MFnDependencyNode::create(const MString& type, MStatus* st) { return create(type, MString(), st); }
MObject MFnDependencyNode::create(const MString& type, const MString& name, MStatus* st) {
    Json f = Json::object();
    f.set("skipSelect", true);
    if (name.length()) f.set("name", name.asChar());
    MStatus s;
    Json made = command("createNode", args1(type.asChar()), f, &s);
    if (st) *st = s;
    obj_ = s ? nodeObject(idNamed(made)) : MObject();
    return obj_;
}
MObject MFnDependencyNode::create(const MTypeId& id, MStatus* st) { return create(id, MString(), st); }
MObject MFnDependencyNode::create(const MTypeId& id, const MString& name, MStatus* st) {
    std::string t = typeOfId(id);
    if (t.empty()) {
        if (st) *st = fail(MStatus::kInvalidParameter, "create: no node type has that id");
        return MObject();
    }
    return create(MString(t), name, st);
}
MString MFnDependencyNode::name(MStatus* st) const {
    int64_t id = azId(st);
    if (id < 0) return MString();
    MStatus s;
    Json info = nodeInfo(id, &s);
    if (!s) {
        // Inside a compute the scene can't be reached: the type and id stand in.
        NodeClass* c = classOfNode(id);
        if (st) *st = s;
        return MString((c ? c->name : std::string("node")) + "#" + std::to_string(id));
    }
    return MString(info["name"].str());
}
MString MFnDependencyNode::absoluteName(MStatus* st) const { return MString(":") + name(st); }
MString MFnDependencyNode::setName(const MString& n, bool, MStatus* st) {
    int64_t id = azId(st);
    if (id < 0) return MString();
    Json a = args1(nodePath(id));
    a.push(n.asChar());
    MStatus s;
    Json r = command("rename", a, Json::object(), &s);
    if (st) *st = s;
    return MString(r.text());
}
MString MFnDependencyNode::typeName(MStatus* st) const {
    int64_t id = azId(st);
    return id < 0 ? MString() : MString(nodeInfo(id, st)["type"].str());
}
MTypeId MFnDependencyNode::typeId(MStatus* st) const {
    NodeClass* c = classNamed(typeName(st).asChar());
    return c ? c->id : MTypeId(0);
}
MString MFnDependencyNode::pluginName(MStatus* st) const {
    int64_t id = azId(st);
    return id < 0 ? MString() : MString(nodeInfo(id, st)["owner"].str());
}
MString MFnDependencyNode::uuidString() const {
    int64_t id = nodeOf(obj_);
    if (id < 0) return MString();
    Json r = command("ls", args1(nodePath(id)), Json::object().set("uuid", true));
    return MString(r.isArray() && r.size() ? r[(size_t)0].text() : std::string());
}
MPlug MFnDependencyNode::findPlug(const MString& n, bool, MStatus* st) const { return findPlug(n, st); }
MPlug MFnDependencyNode::findPlug(const MString& n, MStatus* st) const {
    int64_t id = azId(st);
    if (id < 0) return MPlug();
    std::string full = nodePath(id) + "." + n.asChar();
    Json a = Json::object();
    a.set("name", full);
    MStatus s;
    Json info = query("plug.info", a, &s);
    if (!s) {
        if (st) *st = fail(MStatus::kInvalidParameter, std::string("(kInvalidParameter): Cannot find plug or attribute named ") + n.asChar());
        return MPlug();
    }
    MPlug p = plugNamed(full);
    // A short name reaches the long one, as the plug names itself.
    if (!p.azSpec() && info["component"].integer(-1) < 0 && std::string(n.asChar()).find('[') == std::string::npos && info["long"].isString())
        p = MPlug::azMake(id, info["long"].str(), std::shared_ptr<AttrSpec>(), -1, -1);
    ok(st);
    return p;
}
MPlug MFnDependencyNode::findPlug(const MObject& attr, bool, MStatus* st) const { return findPlug(attr, st); }
MPlug MFnDependencyNode::findPlug(const MObject& attr, MStatus* st) const {
    int64_t id = azId(st);
    if (id < 0) return MPlug();
    return MPlug(obj_, attr);
}
unsigned int MFnDependencyNode::attributeCount(MStatus* st) const {
    int64_t id = azId(st);
    Json a = Json::object();
    a.set("id", id);
    return id < 0 ? 0 : (unsigned int)query("node.attrs", a, st).size();
}
MObject MFnDependencyNode::attribute(unsigned int i, MStatus* st) const {
    int64_t id = azId(st);
    if (id < 0) return MObject();
    Json a = Json::object();
    a.set("id", id);
    Json attrs = query("node.attrs", a, st);
    if (i >= attrs.size()) {
        if (st) *st = MS::kInvalidParameter;
        return MObject();
    }
    return findPlug(MString(attrs[i]["long"].str()), st).attribute();
}
MObject MFnDependencyNode::attribute(const MString& n, MStatus* st) const { return findPlug(n, st).attribute(); }
bool MFnDependencyNode::hasAttribute(const MString& n, MStatus* st) const {
    MStatus s;
    findPlug(n, &s);
    ok(st);
    return s;
}

// A dynamic attribute through addAttr.
MStatus MFnDependencyNode::addAttribute(const MObject& attr) {
    MStatus st;
    int64_t id = azId(&st);
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (id < 0 || !d || !d->attr) return MS::kInvalidParameter;
    const AttrSpec& s = *d->attr;
    Json f = Json::object();
    f.set("longName", s.name).set("shortName", s.shortName);
    if (s.type == "string") f.set("dataType", "string");
    else if (s.type == "mesh") f.set("dataType", "mesh");
    else if (s.type == "matrix") f.set("attributeType", "matrix");
    else if (s.type == "bool") f.set("attributeType", "bool");
    else if (s.type == "int") f.set("attributeType", "long");
    else if (s.type == "message") f.set("attributeType", "message");
    else if (s.type == "enum") {
        std::string fields;
        for (size_t i = 0; i < s.fields.size(); ++i) fields += (i ? ":" : "") + s.fields[i].first + "=" + std::to_string(s.fields[i].second);
        f.set("attributeType", "enum").set("enumName", fields);
    } else if (s.type == "vec3") f.set("attributeType", s.color ? "float3" : "double3");
    else if (s.type == "compound") f.set("attributeType", "compound").set("numberOfChildren", (int)s.children.size());
    else f.set("attributeType", s.unit == "angle" ? "doubleAngle" : s.unit == "distance" ? "doubleLinear" : s.unit == "time" ? "time" : "double");
    if (s.color) f.set("usedAsColor", true);
    if (s.keyable) f.set("keyable", true);
    if (s.array) f.set("multi", true);
    if (s.hasMin) f.set("minValue", s.min);
    if (s.hasMax) f.set("maxValue", s.max);
    if (s.hasSoftMin) f.set("softMinValue", s.softMin);
    if (s.hasSoftMax) f.set("softMaxValue", s.softMax);
    if ((s.type == "float" || s.type == "int" || s.type == "bool" || s.type == "enum") && !s.def.empty()) f.set("defaultValue", s.def[0]);
    if (s.parent) f.set("parent", s.parent->name);
    command("addAttr", args1(nodePath(id)), f, &st);
    if (!st) return st;
    if (s.type == "vec3" || s.type == "compound") {
        static const char* axes[3] = {"X", "Y", "Z"};
        for (int i = 0; i < (s.type == "vec3" ? 3 : (int)s.children.size()); ++i) {
            if (s.type == "compound") {
                st = addAttribute(attrObject(s.children[i]));
                continue;
            }
            Json c = Json::object();
            std::string cn = i < (int)s.children.size() ? s.children[i]->name : s.name + axes[i];
            std::string cs = i < (int)s.children.size() ? s.children[i]->shortName : s.shortName + std::string(1, (char)('x' + i));
            c.set("longName", cn).set("shortName", cs).set("attributeType", s.color ? "float" : "double").set("parent", s.name);
            if (s.keyable) c.set("keyable", true);
            command("addAttr", args1(nodePath(id)), c, &st);
        }
    }
    return st;
}
MStatus MFnDependencyNode::removeAttribute(const MObject& attr) {
    MStatus st;
    int64_t id = azId(&st);
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (id < 0 || !d || !d->attr) return MS::kInvalidParameter;
    command("deleteAttr", args1(nodePath(id) + "." + d->attr->name), Json::object(), &st);
    return st;
}
MFnDependencyNode::MAttrClass MFnDependencyNode::attributeClass(const MObject& attr, MStatus* st) const {
    ok(st);
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (!d || !d->attr) return kInvalidAttr;
    return findPlug(attr).isDynamic() ? kLocalDynamicAttr : kNormalAttr;
}
MStatus MFnDependencyNode::getConnections(MPlugArray& out) const {
    MStatus st;
    int64_t id = azId(&st);
    out.clear();
    if (id < 0) return st;
    Json a = Json::object();
    a.set("id", id);
    Json pairs = query("node.connections", a, &st);
    std::string me = nodePath(id) + ".";
    for (size_t i = 0; i < pairs.size(); ++i) {
        for (int k = 0; k < 2; ++k) {
            std::string p = pairs[i][(size_t)k].str();
            MPlug plug = plugNamed(p);
            if (plug.azNode() != id) continue;
            bool have = false;
            for (unsigned int j = 0; j < out.length(); ++j) have = have || out[j] == plug;
            if (!have) out.append(plug);
        }
    }
    return st;
}
bool MFnDependencyNode::isFromReferencedFile(MStatus* st) const {
    int64_t id = azId(st);
    if (id < 0) return false;
    return command("referenceQuery", args1(nodePath(id)), Json::object().set("isNodeReferenced", true)).truth();
}
bool MFnDependencyNode::isShared(MStatus* st) const { ok(st); return false; }
bool MFnDependencyNode::isDefaultNode(MStatus* st) const { ok(st); return false; }
bool MFnDependencyNode::isLocked(MStatus* st) const {
    int64_t id = azId(st);
    if (id < 0) return false;
    Json r = command("lockNode", args1(nodePath(id)), Json::object().set("query", true).set("lock", true));
    std::vector<double> n = numbers(r);
    return !n.empty() ? n[0] != 0 : r.truth();
}
MStatus MFnDependencyNode::setLocked(bool on) {
    MStatus st;
    int64_t id = azId(&st);
    if (id < 0) return st;
    command("lockNode", args1(nodePath(id)), Json::object().set("lock", on), &st);
    return st;
}
bool MFnDependencyNode::canBeWritten(MStatus* st) const { ok(st); return true; }
MPxNode* MFnDependencyNode::userNode(MStatus* st) const {
    int64_t id = azId(st);
    NodeClass* c = id >= 0 ? classOfNode(id) : 0;
    if (!c) {
        if (st) *st = MS::kInvalidParameter;
        return 0;
    }
    std::lock_guard<std::mutex> g(c->lock);
    std::map<int64_t, MPxNode*>::iterator it = c->instances.find(id);
    return it == c->instances.end() ? 0 : it->second;
}
MString MFnDependencyNode::classification(const MString&) { return MString(); }

// ---------------------------------------------------------------------------------------------------------------------
// MFnDagNode

MFnDagNode::MFnDagNode() {}
MFnDagNode::MFnDagNode(const MObject& o, MStatus* st) { MStatus s = setObject(o); if (st) *st = s; }
MFnDagNode::MFnDagNode(const MDagPath& p, MStatus* st) { MStatus s = setObject(p); if (st) *st = s; }
MFnDagNode::~MFnDagNode() {}
MFn::Type MFnDagNode::type() const { return MFn::kDagNode; }
MStatus MFnDagNode::setObject(const MDagPath& p) {
    path_ = p;
    obj_ = p.node();
    return p.azNode() >= 0 ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MStatus MFnDagNode::setObject(const MObject& o) {
    obj_ = o;
    MStatus st = MDagPath::getAPathTo(o, path_);
    return st;
}
MObject MFnDagNode::create(const MString& type, const MObject& parent, MStatus* st) { return create(type, MString(), parent, st); }
MObject MFnDagNode::create(const MString& type, const MString& name, const MObject& parent, MStatus* st) {
    Json f = Json::object();
    f.set("skipSelect", true);
    if (name.length()) f.set("name", name.asChar());
    int64_t p = nodeOf(parent);
    if (p > 0) f.set("parent", nodePath(p));
    MStatus s;
    Json made = command("createNode", args1(type.asChar()), f, &s);
    if (st) *st = s;
    if (!s) return MObject();
    int64_t id = idNamed(made);
    // A shape made on its own comes with a transform: that is what create returns.
    Json info = nodeInfo(id);
    int64_t up = info["parent"].integer(0);
    setObject(nodeObject(id));
    return nodeObject(p <= 0 && up > 0 ? up : id);
}
unsigned int MFnDagNode::parentCount(MStatus* st) const { ok(st); return path_.azNode() >= 0 ? 1 : 0; }
MObject MFnDagNode::parent(unsigned int i, MStatus* st) const {
    if (i > 0 || path_.azNode() < 0) {
        if (st) *st = MS::kInvalidParameter;
        return MObject();
    }
    ok(st);
    int64_t p = nodeInfo(path_.azNode())["parent"].integer(0);
    if (p > 0) return nodeObject(p);
    std::shared_ptr<ObjectData> w = std::make_shared<ObjectData>();
    w->type = MFn::kWorld;
    return MObject(w);
}
unsigned int MFnDagNode::childCount(MStatus* st) const { return path_.childCount(st); }
MObject MFnDagNode::child(unsigned int i, MStatus* st) const { return path_.child(i, st); }
MStatus MFnDagNode::addChild(MObject& c, unsigned int, bool) { return parentTo(nodeOf(c), path_.azNode()); }
MStatus MFnDagNode::removeChild(MObject& c) { return parentTo(nodeOf(c), 0); }
bool MFnDagNode::hasParent(const MObject& n, MStatus* st) const { ok(st); return nodeInfo(path_.azNode())["parent"].integer(0) == nodeOf(n); }
bool MFnDagNode::hasChild(const MObject& n, MStatus* st) const {
    ok(st);
    Json kids = nodeInfo(path_.azNode())["children"];
    for (size_t i = 0; i < kids.size(); ++i)
        if (kids[i].integer() == nodeOf(n)) return true;
    return false;
}
bool MFnDagNode::isChildOf(const MObject& n, MStatus* st) const { return hasParent(n, st); }
bool MFnDagNode::isParentOf(const MObject& n, MStatus* st) const { return hasChild(n, st); }
MDagPath MFnDagNode::dagPath(MStatus* st) const { ok(st); return path_; }
MStatus MFnDagNode::getPath(MDagPath& p) const { p = path_; return path_.azNode() >= 0 ? MStatus(MS::kSuccess) : MStatus(MS::kFailure); }
MStatus MFnDagNode::getAllPaths(MDagPathArray& paths) const {
    paths.clear();
    paths.append(path_);
    return MS::kSuccess;
}
MString MFnDagNode::fullPathName(MStatus* st) const { return path_.fullPathName(st); }
MString MFnDagNode::partialPathName(MStatus* st) const { return path_.partialPathName(st); }
MDagPath MFnDagNode::dagRoot(MStatus* st) {
    MDagPath p = path_;
    while (nodeInfo(p.azNode())["parent"].integer(0) > 0) p.pop();
    ok(st);
    return p;
}
bool MFnDagNode::isIntermediateObject(MStatus* st) const { return nodeInfo(path_.azNode(), st)["intermediate"].truth(); }
MStatus MFnDagNode::setIntermediateObject(bool on) {
    MStatus st;
    Json a = args1(nodePath(path_.azNode()) + ".intermediateObject");
    a.push(on);
    command("setAttr", a, Json::object(), &st);
    return st;
}
bool MFnDagNode::inUnderWorld(MStatus* st) const { ok(st); return false; }
bool MFnDagNode::isInstanced(bool, MStatus* st) const { ok(st); return false; }
unsigned int MFnDagNode::instanceCount(bool, MStatus* st) const { ok(st); return 1; }
MBoundingBox MFnDagNode::boundingBox(MStatus* st) const {
    Json a = Json::object();
    a.set("id", path_.azNode());
    std::vector<double> b = numbers(query("dag.bounds", a, st));
    if (b.size() != 6) return MBoundingBox();
    return MBoundingBox(MPoint(b[0], b[1], b[2]), MPoint(b[3], b[4], b[5]));
}
MMatrix MFnDagNode::transformationMatrix(MStatus* st) const {
    Json a = Json::object();
    a.set("id", path_.azNode());
    std::vector<double> n = numbers(query("dag.matrices", a, st)["local"]);
    MMatrix m;
    if (n.size() == 16)
        for (int r = 0; r < 4; ++r) for (int c = 0; c < 4; ++c) m.matrix[r][c] = n[r * 4 + c];
    return m;
}
MObject MFnDagNode::duplicate(bool instance, bool, MStatus* st) const {
    Json r = command(instance ? "instance" : "duplicate", args1(nodePath(path_.azNode())), Json::object(), st);
    return nodeObject(idNamed(r));
}
MColor MFnDagNode::objectColorRGB(MStatus* st) const {
    std::vector<double> c = numbers(command("getAttr", args1(nodePath(path_.azNode()) + ".overrideColorRGB"), Json::object(), st));
    return c.size() >= 3 ? MColor((float)c[0], (float)c[1], (float)c[2]) : MColor();
}

// ---------------------------------------------------------------------------------------------------------------------
// MFnTransform, over xform and the transform's channels.

MFnTransform::MFnTransform() {}
MFnTransform::MFnTransform(const MObject& o, MStatus* st) : MFnDagNode(o, st) {}
MFnTransform::MFnTransform(const MDagPath& p, MStatus* st) : MFnDagNode(p, st) {}
MFnTransform::~MFnTransform() {}
MFn::Type MFnTransform::type() const { return MFn::kTransform; }
MObject MFnTransform::create(const MObject& parent, MStatus* st) { return MFnDagNode::create("transform", parent, st); }

static std::string pathOf(const MDagPath& p) { return nodePath(p.azNode()); }

static MVector xformVec(const MDagPath& p, const char* flag, MSpace::Space space, MStatus* st) {
    Json f = Json::object();
    f.set("query", true).set(flag, true);
    if (space == MSpace::kWorld) f.set("worldSpace", true);
    std::vector<double> v = numbers(command("xform", args1(pathOf(p)), f, st));
    return v.size() >= 3 ? MVector(v[0], v[1], v[2]) : MVector();
}
static MStatus xformSet(const MDagPath& p, const char* flag, const MVector& v, MSpace::Space space, bool relative = false) {
    Json f = Json::object();
    Json val = Json::array();
    val.push(v.x).push(v.y).push(v.z);
    f.set(flag, val);
    if (space == MSpace::kWorld) f.set("worldSpace", true);
    if (relative) f.set("relative", true);
    MStatus st;
    command("xform", args1(pathOf(p)), f, &st);
    return st;
}
static MVector channel(const MDagPath& p, const char* attr, MStatus* st = 0) {
    std::vector<double> v = numbers(command("getAttr", args1(pathOf(p) + "." + attr), Json::object(), st));
    return v.size() >= 3 ? MVector(v[0], v[1], v[2]) : MVector();
}
static MStatus setChannel(const MDagPath& p, const char* attr, const MVector& v) {
    Json a = args1(pathOf(p) + "." + attr);
    a.push(v.x).push(v.y).push(v.z);
    MStatus st;
    command("setAttr", a, Json::object(), &st);
    return st;
}

MVector MFnTransform::getTranslation(MSpace::Space s, MStatus* st) const { return xformVec(path_, "translation", s, st); }
MVector MFnTransform::translation(MSpace::Space s, MStatus* st) const { return getTranslation(s, st); }
MStatus MFnTransform::setTranslation(const MVector& v, MSpace::Space s) { return xformSet(path_, "translation", v, s); }
MStatus MFnTransform::translateBy(const MVector& v, MSpace::Space s) { return xformSet(path_, "translation", v, s, true); }
MTransformationMatrix::RotationOrder MFnTransform::rotationOrder(MStatus* st) const {
    std::vector<double> v = numbers(command("getAttr", args1(pathOf(path_) + ".rotateOrder"), Json::object(), st));
    return (MTransformationMatrix::RotationOrder)((v.empty() ? 0 : (int)v[0]) + 1);
}
MStatus MFnTransform::getRotation(MEulerRotation& e) const {
    MStatus st;
    MVector r = channel(path_, "rotate", &st);
    e = MEulerRotation(r.x / kDeg, r.y / kDeg, r.z / kDeg, (MEulerRotation::RotationOrder)(rotationOrder() - 1));
    return st;
}
MStatus MFnTransform::getRotation(MQuaternion& q, MSpace::Space space) const {
    MEulerRotation e;
    MStatus st = getRotation(e);
    q = e.asQuaternion();
    if (space == MSpace::kWorld) {
        MQuaternion parent;
        parent = path_.exclusiveMatrix();
        q = q * parent;
    }
    return st;
}
MStatus MFnTransform::getRotation(double r[3], MTransformationMatrix::RotationOrder& order) const {
    MEulerRotation e;
    MStatus st = getRotation(e);
    r[0] = e.x;
    r[1] = e.y;
    r[2] = e.z;
    order = (MTransformationMatrix::RotationOrder)(e.order + 1);
    return st;
}
MStatus MFnTransform::setRotation(const MEulerRotation& e0) {
    MEulerRotation e = e0.reorder((MEulerRotation::RotationOrder)(rotationOrder() - 1));
    return setChannel(path_, "rotate", MVector(e.x * kDeg, e.y * kDeg, e.z * kDeg));
}
MStatus MFnTransform::setRotation(const MQuaternion& q0, MSpace::Space space) {
    MQuaternion q = q0;
    if (space == MSpace::kWorld) {
        MQuaternion parent;
        parent = path_.exclusiveMatrix();
        q = q * parent.inverse();
    }
    return setRotation(MEulerRotation::decompose(q.asMatrix(), (MEulerRotation::RotationOrder)(rotationOrder() - 1)));
}
MStatus MFnTransform::setRotation(const double r[3], MTransformationMatrix::RotationOrder order) {
    return setRotation(MEulerRotation(r[0], r[1], r[2], (MEulerRotation::RotationOrder)(order - 1)));
}
MStatus MFnTransform::rotateBy(const MQuaternion& q, MSpace::Space space) {
    MQuaternion now;
    getRotation(now, MSpace::kTransform);
    return setRotation(space == MSpace::kWorld ? now * q : q * now);
}
MStatus MFnTransform::rotateBy(const MEulerRotation& e, MSpace::Space space) { return rotateBy(e.asQuaternion(), space); }
MStatus MFnTransform::getRotationQuaternion(double& x, double& y, double& z, double& w, MSpace::Space space) const {
    MQuaternion q;
    MStatus st = getRotation(q, space);
    x = q.x;
    y = q.y;
    z = q.z;
    w = q.w;
    return st;
}
MStatus MFnTransform::setRotationQuaternion(double x, double y, double z, double w, MSpace::Space space) { return setRotation(MQuaternion(x, y, z, w), space); }
MStatus MFnTransform::setRotationOrder(MTransformationMatrix::RotationOrder order, bool reorder) {
    MEulerRotation e;
    getRotation(e);
    Json a = args1(pathOf(path_) + ".rotateOrder");
    a.push((int)order - 1);
    MStatus st;
    command("setAttr", a, Json::object(), &st);
    if (st && reorder) st = setRotation(e);
    return st;
}
MStatus MFnTransform::getScale(double s[3]) const {
    MStatus st;
    MVector v = channel(path_, "scale", &st);
    s[0] = v.x;
    s[1] = v.y;
    s[2] = v.z;
    return st;
}
MStatus MFnTransform::setScale(const double s[3]) { return setChannel(path_, "scale", MVector(s[0], s[1], s[2])); }
MStatus MFnTransform::scaleBy(const double s[3]) {
    double now[3];
    getScale(now);
    return setChannel(path_, "scale", MVector(now[0] * s[0], now[1] * s[1], now[2] * s[2]));
}
MStatus MFnTransform::getShear(double s[3]) const {
    MStatus st;
    MVector v = channel(path_, "shear", &st);
    s[0] = v.x;
    s[1] = v.y;
    s[2] = v.z;
    return st;
}
MStatus MFnTransform::setShear(const double s[3]) { return setChannel(path_, "shear", MVector(s[0], s[1], s[2])); }
MPoint MFnTransform::rotatePivot(MSpace::Space s, MStatus* st) const { return MPoint(xformVec(path_, "rotatePivot", s, st)); }
MStatus MFnTransform::setRotatePivot(const MPoint& p, MSpace::Space s, bool) { return xformSet(path_, "rotatePivot", MVector(p), s); }
MPoint MFnTransform::scalePivot(MSpace::Space s, MStatus* st) const { return MPoint(xformVec(path_, "scalePivot", s, st)); }
MStatus MFnTransform::setScalePivot(const MPoint& p, MSpace::Space s, bool) { return xformSet(path_, "scalePivot", MVector(p), s); }
MQuaternion MFnTransform::rotateOrientation(MSpace::Space, MStatus* st) const {
    MVector r = channel(path_, "rotateAxis", st);
    return MEulerRotation(r.x / kDeg, r.y / kDeg, r.z / kDeg).asQuaternion();
}
MStatus MFnTransform::setRotateOrientation(const MQuaternion& q, MSpace::Space, bool) {
    MEulerRotation e = q.asEulerRotation();
    return setChannel(path_, "rotateAxis", MVector(e.x * kDeg, e.y * kDeg, e.z * kDeg));
}
MTransformationMatrix MFnTransform::transformation(MStatus* st) const {
    MTransformationMatrix t;
    t.setTranslation(getTranslation(MSpace::kTransform, st), MSpace::kTransform);
    MEulerRotation e;
    getRotation(e);
    t.rotateTo(e);
    double s[3];
    getScale(s);
    t.setScale(s, MSpace::kTransform);
    double sh[3];
    getShear(sh);
    t.setShear(sh, MSpace::kTransform);
    return t;
}
MStatus MFnTransform::set(const MTransformationMatrix& t) {
    MStatus st = setTranslation(t.getTranslation(MSpace::kTransform), MSpace::kTransform);
    if (st) st = setRotation(t.eulerRotation());
    double s[3], sh[3];
    t.getScale(s, MSpace::kTransform);
    t.getShear(sh, MSpace::kTransform);
    if (st) st = setScale(s);
    if (st) st = setShear(sh);
    return st;
}
MStatus MFnTransform::resetFromRestPosition() { return fail(MStatus::kNotImplemented, "resetFromRestPosition isn't available"); }
