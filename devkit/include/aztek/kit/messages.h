// Aztek plug-in kit: scene messages. A callback added here is called when the host sends its message.
//
// When they come: graph messages (nodes added and removed, connections, attributes set, names, parents) after the command
// that made the change, still inside its undo step, rather than in the middle of it; a node removed is told of once it
// has gone, its name and type still answered while its callbacks run. A node the delete command removes hears its
// about-to-delete and pre-removal messages before. Scene messages come before and after the file command. Time and
// matrix messages come when a command or the idle loop finds the time or the matrix changed. Event messages
// (MEventMessage) come from the idle loop, as script jobs do.
#pragma once
#include "scene.h"

#ifndef kDefaultNodeType
#define kDefaultNodeType "dependNode"
#endif

class MFileObject {
public:
    enum MFileResolveMethod { kNone = 0, kRelative = 1, kInputFile = 2, kDirMap = 4, kExact = kDirMap | kRelative, kStrict = 8 };
    MFileObject();
    MFileObject(const MFileObject& other);
    virtual ~MFileObject();
    MFileObject& operator=(const MFileObject& other);
    MStatus setRawName(const MString& name);
    MStatus setRawPath(const MString& path);
    MStatus setRawFullName(const MString& fullName);
    MStatus setRawURI(const MString& uri);
    MStatus setFullName(const MString& fullName);
    MStatus setName(const MString& name);
    MStatus overrideResolvedFullName(const MString& fullName, bool reResolve = false);
    MString rawName() const;
    MString rawPath() const;
    MString rawFullName() const;
    MString rawURI() const;
    MString expandedPath() const;
    MString expandedFullName() const;
    MString resolvedName() const;
    MString resolvedPath() const;
    MString resolvedFullName() const;
    MString name() const;
    MString path() const;
    MString fullName() const;
    bool exists() const;
    bool isSet() const;

private:
    std::string full_;
    std::string resolved_;
};

class MUuid {
public:
    MUuid();
    MUuid(const MUuid& other);
    explicit MUuid(const MString& s, MStatus* status = 0);
    explicit MUuid(const unsigned char* bytes, MStatus* status = 0);
    MUuid& operator=(const MUuid& other);
    bool operator==(const MUuid& other) const;
    bool operator!=(const MUuid& other) const;
    MStatus copy(const MUuid& other);
    MStatus get(unsigned char* bytes) const;
    MString asString() const;
    void generate();
    bool valid() const;

private:
    unsigned char b_[16];
    bool valid_;
};

class MDGModifier;
class MDagPath;

class MMessage {
public:
    enum Action { kDefaultAction, kDoNotDoAction, kDoAction };
    typedef void (*MBasicFunction)(void* clientData);
    typedef void (*MElapsedTimeFunction)(float elapsedTime, float lastTime, void* clientData);
    typedef void (*MCheckFunction)(bool* retCode, void* clientData);
    typedef void (*MCheckFileFunction)(bool* retCode, MFileObject& file, void* clientData);
    typedef void (*MCheckPlugFunction)(bool* retCode, MPlug& plug, void* clientData);
    typedef void (*MComponentFunction)(MUintArray componentIds[], unsigned int count, void* clientData);
    typedef void (*MNodeFunction)(MObject& node, void* clientData);
    typedef void (*MStringFunction)(const MString& str, void* clientData);
    typedef void (*MTwoStringFunction)(const MString& str1, const MString& str2, void* clientData);
    typedef void (*MThreeStringFunction)(const MString& str1, const MString& str2, const MString& str3, void* clientData);
    typedef void (*MStringIntBoolIntFunction)(const MString& str, unsigned int index, bool flag, unsigned int type, void* clientData);
    typedef void (*MStringIndexFunction)(const MString&, unsigned int index, void* clientData);
    typedef void (*MNodeStringBoolFunction)(MObject& node, const MString&, bool, void* clientData);
    typedef void (*MStateFunction)(bool state, void* clientData);
    typedef void (*MTimeFunction)(MTime& time, void* clientData);
    typedef void (*MPlugFunction)(MPlug& srcPlug, MPlug& destPlug, bool made, void* clientData);
    typedef void (*MNodePlugFunction)(MObject& node, MPlug& plug, void* clientData);
    typedef void (*MNodeStringFunction)(MObject& node, const MString& str, void* clientData);
    typedef void (*MParentChildFunction)(MDagPath& child, MDagPath& parent, void* clientData);
    typedef void (*MModifierFunction)(MDGModifier& modifier, void* clientData);
    typedef void (*MStringArrayFunction)(const MStringArray& strs, void* clientData);
    typedef void (*MNodeModifierFunction)(MObject& node, MDGModifier& modifier, void* clientData);
    typedef void (*MObjArray)(MObjectArray& objects, void* clientData);
    typedef void (*MNodeObjArray)(MObject& node, MObjectArray& objects, void* clientData);
    typedef void (*MStringNode)(const MString& str, MObject& node, void* clientData);
    typedef void (*MCameraLayerFunction)(MObject& cameraSetNode, unsigned int multiIndex, bool added, void* clientData);
    typedef void (*MCameraLayerCameraFunction)(MObject& cameraSetNode, unsigned int multiIndex, MObject& oldCamera, MObject& newCamera, void* clientData);
    typedef void (*MConnFailFunction)(MPlug& srcPlug, MPlug& destPlug, const MString& srcPlugName, const MString& dstPlugName, void* clientData);
    typedef void (*MPlugsDGModFunction)(MPlugArray& plugs, MDGModifier& modifier, void* clientData);
    typedef void (*MNodeUuidFunction)(MObject& node, const MUuid& uuid, void* clientData);
    typedef Action (*MCheckNodeUuidFunction)(bool doAction, MObject& node, MUuid& uuid, void* clientData);
    typedef void (*MObjectFileFunction)(const MObject& object, const MFileObject& file, void* clientData);
    typedef void (*MCheckObjectFileFunction)(bool* retCode, const MObject& referenceNode, MFileObject& file, void* clientData);

    static MStatus removeCallback(MCallbackId id);
    static MStatus removeCallbacks(MCallbackIdArray& ids);
    static MStatus removeCallbacks(MIntArray& ids);
    static MCallbackId currentCallbackId(MStatus* status = 0);
    static MStatus nodeCallbacks(MObject& node, MCallbackIdArray& ids);
    static MStatus nodeCallbacks(MObject& node, MIntArray& ids);
    static void setRegisteringCallableScript();
    static bool registeringCallableScript();
    static void stopRegisteringCallableScript();
    static const char* className() { return "MMessage"; }
    // The kit's own: an id for a callback the host never calls.
    static MCallbackId azNewId();
};

class MSceneMessage : public MMessage {
public:
    enum Message {
        kSceneUpdate,
        kBeforeNew,
        kAfterNew,
        kBeforeImport,
        kAfterImport,
        kBeforeOpen,
        kAfterOpen,
        kBeforeFileRead,
        kAfterFileRead,
        kAfterSceneReadAndRecordEdits,
        kBeforeExport,
        kAfterExport,
        kBeforeSave,
        kAfterSave,
        kBeforeReference,
        kAfterReference,
        kBeforeRemoveReference,
        kAfterRemoveReference,
        kBeforeImportReference,
        kAfterImportReference,
        kBeforeExportReference,
        kAfterExportReference,
        kBeforeUnloadReference,
        kAfterUnloadReference,
        kBeforeSoftwareRender,
        kAfterSoftwareRender,
        kBeforeSoftwareFrameRender,
        kAfterSoftwareFrameRender,
        kSoftwareRenderInterrupted,
        kMayaInitialized,
        kMayaExiting,
        kBeforeNewCheck,
        kBeforeOpenCheck,
        kBeforeSaveCheck,
        kBeforeImportCheck,
        kBeforeExportCheck,
        kBeforeLoadReference,
        kAfterLoadReference,
        kBeforeLoadReferenceCheck,
        kBeforeReferenceCheck,
        kBeforeCreateReferenceCheck = kBeforeReferenceCheck,
        kBeforePluginLoad,
        kAfterPluginLoad,
        kBeforePluginUnload,
        kAfterPluginUnload,
        kBeforeCreateReference,
        kAfterCreateReference,
        kExportStarted,
        kBeforeLoadReferenceAndRecordEdits,
        kAfterLoadReferenceAndRecordEdits,
        kBeforeCreateReferenceAndRecordEdits,
        kAfterCreateReferenceAndRecordEdits,
        kLast
    };
    static MCallbackId addCallback(Message msg, MMessage::MBasicFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addCheckCallback(Message msg, MMessage::MCheckFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addCheckFileCallback(Message msg, MMessage::MCheckFileFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addStringArrayCallback(Message msg, MMessage::MStringArrayFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addReferenceCallback(Message msg, MMessage::MObjectFileFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addCheckReferenceCallback(Message msg, MMessage::MCheckObjectFileFunction func, void* clientData = 0, MStatus* status = 0);
    // Connections refused while a file is read. None are refused here, so it is never called.
    static MCallbackId addConnectionFailedCallback(MMessage::MConnFailFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNamespaceRenamedCallback(MMessage::MTwoStringFunction func, void* clientData = 0, MStatus* status = 0);
    // Obsolete forms.
    static MCallbackId addCallback(Message msg, void (*func)(bool* retCode, void* clientData), void* clientData = 0, MStatus* status = 0);
    static MCallbackId addCallback(Message msg, void (*func)(bool* retCode, MFileObject& file, void* clientData), void* clientData = 0, MStatus* status = 0);
    static MCallbackId addCallback(Message msg, void (*func)(const MStringArray&, void*), void* clientData, MStatus* status);
    static const char* className() { return "MSceneMessage"; }
};

class MDGMessage : public MMessage {
public:
    static MCallbackId addTimeChangeCallback(MMessage::MTimeFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDelayedTimeChangeCallback(MMessage::MTimeFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDelayedTimeChangeRunupCallback(MMessage::MTimeFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addForceUpdateCallback(MMessage::MTimeFunction func, void* clientData = 0, MStatus* status = 0);
    // Nodes of the type, or of a type derived from it; every node for "dependNode".
    static MCallbackId addNodeAddedCallback(MMessage::MNodeFunction func, const MString& nodeType = kDefaultNodeType, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNodeRemovedCallback(MMessage::MNodeFunction func, const MString& nodeType = kDefaultNodeType, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addConnectionCallback(MMessage::MPlugFunction func, void* clientData = 0, MStatus* status = 0);
    // Sent just before the connection callbacks, once the connection is made or broken.
    static MCallbackId addPreConnectionCallback(MMessage::MPlugFunction func, void* clientData = 0, MStatus* status = 0);
    // UUIDs never change on their own here, so it is never called.
    static MCallbackId addNodeChangeUuidCheckCallback(MMessage::MCheckNodeUuidFunction func, void* clientData = 0, MStatus* status = 0);
    static const char* className() { return "MDGMessage"; }
};

class MDagMessage : public MMessage {
public:
    enum DagMessage { kInvalidMsg = -1, kParentAdded, kParentRemoved, kChildAdded, kChildRemoved, kChildReordered, kInstanceAdded, kInstanceRemoved, kLast };
    enum MatrixModifiedFlags {
        kScaleX = 1 << 0,
        kScaleY = 1 << 1,
        kScaleZ = 1 << 2,
        kShearXY = 1 << 3,
        kShearXZ = 1 << 4,
        kShearYZ = 1 << 5,
        kRotateX = 1 << 6,
        kRotateY = 1 << 7,
        kRotateZ = 1 << 8,
        kTranslateX = 1 << 9,
        kTranslateY = 1 << 10,
        kTranslateZ = 1 << 11,
        kScalePivotX = 1 << 12,
        kScalePivotY = 1 << 13,
        kScalePivotZ = 1 << 14,
        kRotatePivotX = 1 << 15,
        kRotatePivotY = 1 << 16,
        kRotatePivotZ = 1 << 17,
        kScaleTransX = 1 << 18,
        kScaleTransY = 1 << 19,
        kScaleTransZ = 1 << 20,
        kRotateTransX = 1 << 21,
        kRotateTransY = 1 << 22,
        kRotateTransZ = 1 << 23,
        kRotateOrientX = 1 << 24,
        kRotateOrientY = 1 << 25,
        kRotateOrientZ = 1 << 26,
        kRotateOrder = 1 << 27,
        kAll = (1 << 28) - 1,
        kScale = kScaleX | kScaleY | kScaleZ,
        kShear = kShearXY | kShearXZ | kShearYZ,
        kRotation = kRotateX | kRotateY | kRotateZ,
        kTranslation = kTranslateX | kTranslateY | kTranslateZ,
        kScalePivot = kScalePivotX | kScalePivotY | kScalePivotZ,
        kRotatePivot = kRotatePivotX | kRotatePivotY | kRotatePivotZ,
        kScalePivotTrans = kScaleTransX | kScaleTransY | kScaleTransZ,
        kRotatePivotTrans = kRotateTransX | kRotateTransY | kRotateTransZ,
        kRotateOrient = kRotateOrientX | kRotateOrientY | kRotateOrientZ
    };
    typedef void (*MMessageParentChildFunction)(MDagMessage::DagMessage msgType, MDagPath& child, MDagPath& parent, void* clientData);
    typedef void (*MWorldMatrixModifiedFunction)(MObject& transformNode, MDagMessage::MatrixModifiedFlags& modified, void* clientData);

    static MCallbackId addParentAddedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addParentAddedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addParentRemovedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addParentRemovedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildAddedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildAddedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildRemovedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildRemovedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildReorderedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildReorderedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDagCallback(DagMessage msgType, MDagMessage::MMessageParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDagDagPathCallback(MDagPath& node, DagMessage msgType, MDagMessage::MMessageParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addAllDagChangesCallback(MDagMessage::MMessageParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addAllDagChangesDagPathCallback(MDagPath& node, MDagMessage::MMessageParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    // Instances have no messages of their own here: these are never called.
    static MCallbackId addInstanceAddedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addInstanceAddedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addInstanceRemovedCallback(MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addInstanceRemovedDagPathCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addMatrixModifiedCallback(MDagPath& affectedNode, MDagMessage::MWorldMatrixModifiedFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addWorldMatrixModifiedCallback(MDagPath& affectedNode, MDagMessage::MWorldMatrixModifiedFunction func, void* clientData = 0, MStatus* status = 0);
    // Obsolete forms.
    static MCallbackId addParentAddedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addParentRemovedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildAddedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildRemovedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addChildReorderedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addInstanceAddedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addInstanceRemovedCallback(MDagPath& node, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDagCallback(DagMessage msgType, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDagCallback(MDagPath& node, DagMessage msgType, MMessage::MParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addDagCallback(MDagPath& node, DagMessage msgType, MDagMessage::MMessageParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addAllDagChangesCallback(MDagPath& node, MDagMessage::MMessageParentChildFunction func, void* clientData = 0, MStatus* status = 0);
    static const char* className() { return "MDagMessage"; }
};

class MNodeMessage : public MMessage {
public:
    enum AttributeMessage {
        kConnectionMade = 0x01,
        kConnectionBroken = 0x02,
        kAttributeEval = 0x04,
        kAttributeSet = 0x08,
        kAttributeLocked = 0x10,
        kAttributeUnlocked = 0x20,
        kAttributeAdded = 0x40,
        kAttributeRemoved = 0x80,
        kAttributeRenamed = 0x100,
        kAttributeKeyable = 0x200,
        kAttributeUnkeyable = 0x400,
        kIncomingDirection = 0x800,
        kAttributeArrayAdded = 0x1000,
        kAttributeArrayRemoved = 0x2000,
        kOtherPlugSet = 0x4000,
        kLast = 0x8000
    };
    enum KeyableChangeMsg { kKeyChangeInvalid = 0, kMakeKeyable, kMakeUnkeyable, kKeyChangeLast };
    typedef void (*MAttr2PlugFunction)(MNodeMessage::AttributeMessage msg, MPlug& plug, MPlug& otherPlug, void* clientData);
    typedef void (*MAttrPlugFunction)(MNodeMessage::AttributeMessage msg, MPlug& plug, void* clientData);
    typedef void (*MKeyableFunction)(MPlug& plug, void* clientData, MNodeMessage::KeyableChangeMsg msg, bool& decision);
    static MCallbackId addAttributeChangedCallback(MObject& node, MNodeMessage::MAttr2PlugFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addAttributeAddedOrRemovedCallback(MObject& node, MNodeMessage::MAttrPlugFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNodeDirtyCallback(MObject& node, MMessage::MNodeFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNodeDirtyPlugCallback(MObject& node, MMessage::MNodePlugFunction func, void* clientData = 0, MStatus* status = 0);
    // With MObject::kNullObj, every node's.
    static MCallbackId addNameChangedCallback(MObject& node, MMessage::MNodeStringFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addUuidChangedCallback(MObject& node, MMessage::MNodeUuidFunction func, void* clientData = 0, MStatus* status = 0);
    // The modifier's edits are done once the callbacks return, before the node goes.
    static MCallbackId addNodeAboutToDeleteCallback(MObject& node, MMessage::MNodeModifierFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNodePreRemovalCallback(MObject& node, MMessage::MNodeFunction func, void* clientData = 0, MStatus* status = 0);
    // When the node goes for good: with the scene (a new scene, an open), not a delete an undo can bring back.
    static MCallbackId addNodeDestroyedCallback(MObject& node, MMessage::MBasicFunction func, void* clientData = 0, MStatus* status = 0);
    // Taken; keyability changes are not offered to it.
    static MCallbackId addKeyableChangeOverride(MPlug& plug, MNodeMessage::MKeyableFunction func, void* clientData = 0, MStatus* status = 0);
    // Obsolete forms.
    static MCallbackId addNodeDirtyCallback(MObject& node, MMessage::MBasicFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNodeDirtyCallback(MObject& node, MMessage::MNodePlugFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNameChangedCallback(MObject& node, MMessage::MNodeFunction func, void* clientData = 0, MStatus* status = 0);
    static MCallbackId addNodeAboutToDeleteCallback(MObject& node, MMessage::MModifierFunction func, void* clientData = 0, MStatus* status = 0);
    static const char* className() { return "MNodeMessage"; }
};

class MEventMessage : public MMessage {
public:
    static MCallbackId addEventCallback(const MString& event, MMessage::MBasicFunction func, void* clientData = 0, MStatus* status = 0);
    static void getEventNames(MStringArray& names);
    static const char* className() { return "MEventMessage"; }
};
