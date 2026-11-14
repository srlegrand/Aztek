// A command that adds message callbacks and gives back what they heard, one line each, for the tests:
// azMsg start, azMsg watch node, azMsg refuse 1, azMsg redirect file, azMsg log, azMsg stop.
#include <maya/MArgList.h>
#include <maya/MCallbackIdArray.h>
#include <maya/MDGMessage.h>
#include <maya/MDGModifier.h>
#include <maya/MDagMessage.h>
#include <maya/MDagPath.h>
#include <maya/MEventMessage.h>
#include <maya/MFileObject.h>
#include <maya/MFnDependencyNode.h>
#include <maya/MFnPlugin.h>
#include <maya/MNodeMessage.h>
#include <maya/MPlug.h>
#include <maya/MPxCommand.h>
#include <maya/MSceneMessage.h>
#include <maya/MSelectionList.h>
#include <maya/MStringArray.h>
#include <maya/MTime.h>
#include <sstream>
#include <string>
#include <vector>

namespace {
std::vector<std::string> lines;
MCallbackIdArray ids;
bool refuse = false;
std::string redirect;

void note(const std::string& s) { lines.push_back(s); }
std::string str(const MString& s) { return s.asChar(); }
std::string nodeText(MObject& n) {
    MFnDependencyNode fn(n);
    return str(fn.name()) + " " + str(fn.typeName());
}

void scene(void* cd) { note("scene " + std::to_string((long long)(size_t)cd)); }
void check(bool* ret, void*) {
    note("check new");
    *ret = !refuse;
}
void checkOpen(bool* ret, MFileObject& f, void*) {
    note("check open " + str(f.resolvedName()));
    if (!redirect.empty()) f.setRawFullName(MString(redirect.c_str()));
    *ret = true;
}
void plugins(const MStringArray& s, void*) {
    std::string t = "plugin";
    for (unsigned int i = 0; i < s.length(); ++i) t += i ? "|" + str(s[i]) : " " + str(s[i]);
    note(t);
}
void added(MObject& n, void*) { note("added " + nodeText(n)); }
void removed(MObject& n, void*) { note("removed " + nodeText(n)); }
void connected(MPlug& src, MPlug& dst, bool made, void*) { note("conn " + str(src.name()) + " " + str(dst.name()) + (made ? " made" : " broken")); }
void timed(MTime& t, void*) {
    std::ostringstream o;
    o << "time " << t.value();
    note(o.str());
}
void dag(MDagMessage::DagMessage m, MDagPath& child, MDagPath& parent, void*) {
    note("dag " + std::to_string((int)m) + " " + str(child.partialPathName()) + " " + (parent.length() ? str(parent.partialPathName()) : std::string("world")));
}
void event(void*) { note("event"); }

void attr(MNodeMessage::AttributeMessage m, MPlug& p, MPlug& o, void*) {
    note("attr " + std::to_string((int)m) + " " + str(p.name()) + (o.isNull() ? std::string() : " " + str(o.name())));
}
void attrAddRemove(MNodeMessage::AttributeMessage m, MPlug& p, void*) { note("attrAddRemove " + std::to_string((int)m) + " " + str(p.partialName())); }
void named(MObject& n, const MString& old, void*) { note("name " + str(MFnDependencyNode(n).name()) + " was " + str(old)); }
void aboutToDelete(MObject& n, MDGModifier&, void*) { note("aboutToDelete " + nodeText(n)); }
void preRemoval(MObject& n, void*) { note("preRemoval " + nodeText(n)); }
void dirty(MObject& n, void*) { note("dirty " + str(MFnDependencyNode(n).name())); }
void matrix(MObject& n, MDagMessage::MatrixModifiedFlags& f, void*) { note("matrix " + str(MFnDependencyNode(n).name()) + " " + std::to_string((int)f)); }
void selfRemoving(MObject& n, void*) {
    note("once " + str(MFnDependencyNode(n).name()));
    MMessage::removeCallback(MMessage::currentCallbackId());
}

void keep(MCallbackId id, MStatus st, const char* what) {
    if (st)
        ids.append(id);
    else
        note(std::string("failed ") + what);
}
}  // namespace

class MsgProbe : public MPxCommand {
public:
    static void* creator() { return new MsgProbe; }

    MStatus doIt(const MArgList& args) override {
        clearResult();
        MString what = args.asString(0);
        MStatus st;
        if (what == "start") {
            // Every scene message but the checks and the plug-in ones logs its number.
            for (int m = MSceneMessage::kSceneUpdate; m < MSceneMessage::kLast; ++m) {
                if ((m >= MSceneMessage::kBeforeNewCheck && m <= MSceneMessage::kBeforeExportCheck) || m == MSceneMessage::kBeforeLoadReferenceCheck || m == MSceneMessage::kBeforeCreateReferenceCheck) continue;
                if (m >= MSceneMessage::kBeforePluginLoad && m <= MSceneMessage::kAfterPluginUnload) continue;
                keep(MSceneMessage::addCallback((MSceneMessage::Message)m, scene, (void*)(size_t)m, &st), st, "scene");
            }
            keep(MSceneMessage::addCheckCallback(MSceneMessage::kBeforeNewCheck, check, 0, &st), st, "newCheck");
            keep(MSceneMessage::addCheckFileCallback(MSceneMessage::kBeforeOpenCheck, checkOpen, 0, &st), st, "openCheck");
            keep(MSceneMessage::addStringArrayCallback(MSceneMessage::kAfterPluginLoad, plugins, 0, &st), st, "plugins");
            keep(MDGMessage::addNodeAddedCallback(added, "transform", 0, &st), st, "added");
            keep(MDGMessage::addNodeRemovedCallback(removed, kDefaultNodeType, 0, &st), st, "removed");
            keep(MDGMessage::addConnectionCallback(connected, 0, &st), st, "connection");
            keep(MDGMessage::addTimeChangeCallback(timed, 0, &st), st, "time");
            keep(MDagMessage::addAllDagChangesCallback(dag, 0, &st), st, "dag");
            keep(MEventMessage::addEventCallback("SelectionChanged", event, 0, &st), st, "event");
            return MS::kSuccess;
        }
        if (what == "watch") {
            MSelectionList sl;
            sl.add(args.asString(1));
            MObject n;
            sl.getDependNode(0, n);
            keep(MNodeMessage::addAttributeChangedCallback(n, attr, 0, &st), st, "attr");
            keep(MNodeMessage::addAttributeAddedOrRemovedCallback(n, attrAddRemove, 0, &st), st, "attrAddRemove");
            keep(MNodeMessage::addNameChangedCallback(n, named, 0, &st), st, "name");
            keep(MNodeMessage::addNodeAboutToDeleteCallback(n, aboutToDelete, 0, &st), st, "aboutToDelete");
            keep(MNodeMessage::addNodePreRemovalCallback(n, preRemoval, 0, &st), st, "preRemoval");
            keep(MNodeMessage::addNodeDirtyCallback(n, dirty, 0, &st), st, "dirty");
            MDagPath p;
            if (sl.getDagPath(0, p)) keep(MDagMessage::addMatrixModifiedCallback(p, matrix, 0, &st), st, "matrix");
            return MS::kSuccess;
        }
        if (what == "once") {
            keep(MDGMessage::addNodeAddedCallback(selfRemoving, kDefaultNodeType, 0, &st), st, "once");
            return MS::kSuccess;
        }
        if (what == "refuse") {
            refuse = args.asInt(1) != 0;
            return MS::kSuccess;
        }
        if (what == "redirect") {
            redirect = args.length() > 1 ? args.asString(1).asChar() : "";
            return MS::kSuccess;
        }
        if (what == "count") {
            setResult((int)ids.length());
            return MS::kSuccess;
        }
        if (what == "nodeCallbacks") {
            MSelectionList sl;
            sl.add(args.asString(1));
            MObject n;
            sl.getDependNode(0, n);
            MCallbackIdArray mine;
            MMessage::nodeCallbacks(n, mine);
            setResult((int)mine.length());
            return MS::kSuccess;
        }
        if (what == "log") {
            for (const std::string& l : lines) appendToResult(l.c_str());
            lines.clear();
            return MS::kSuccess;
        }
        if (what == "stop") {
            MMessage::removeCallbacks(ids);
            return MS::kSuccess;
        }
        return MS::kInvalidParameter;
    }
};

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek tests", "1.0", "Any");
    return plugin.registerCommand("azMsg", MsgProbe::creator);
}

MStatus uninitializePlugin(MObject obj) {
    MMessage::removeCallbacks(ids);
    MFnPlugin plugin(obj);
    return plugin.deregisterCommand("azMsg");
}
