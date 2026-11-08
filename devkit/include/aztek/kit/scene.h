// Aztek plug-in kit: plugs, paths, selections, the global functions, modifiers and the node function sets.
#ifndef AZTEK_KIT_SCENE_H
#define AZTEK_KIT_SCENE_H

#include <functional>

#include "math.h"

class MPlugArray;
class MDagPath;
class MDagPathArray;
class MSelectionList;
class MFnDagNode;
class MPxNode;

class MFnBase {
public:
    MFnBase();
    virtual ~MFnBase();
    virtual MFn::Type type() const;
    bool hasObj(MFn::Type t) const;
    bool hasObj(const MObject& o) const;
    MObject object(MStatus* status = 0) const;
    virtual MStatus setObject(const MObject& o);
    static const char* className() { return "MFnBase"; }

protected:
    MObject obj_;
};

class MPlug {
public:
    enum MValueSelector { kAll, kNonDefault, kChanged, kLastAttrSelector };
    enum FreeToChangeState { kFreeToChange, kNotFreeToChange, kChildrenNotFreeToChange };
    MPlug();
    MPlug(const MObject& node, const MObject& attribute);
    MPlug(const MPlug& other);
    ~MPlug();
    MPlug& operator=(const MPlug& other);
    bool operator==(const MPlug& other) const;
    bool operator==(const MObject& attr) const;
    bool operator!=(const MPlug& other) const;
    bool operator!=(const MObject& attr) const;
    bool isNull(MStatus* status = 0) const;
    MObject attribute(MStatus* status = 0) const;
    MObject node(MStatus* status = 0) const;
    MString name(MStatus* status = 0) const;
    MString partialName(bool includeNodeName = false, bool includeNonMandatoryIndices = false, bool includeInstancedIndices = false,
                        bool useAlias = false, bool useFullAttributePath = false, bool useLongNames = false, MStatus* status = 0) const;
    MString info(MStatus* status = 0) const;
    bool isNetworked(MStatus* status = 0) const;
    bool isArray(MStatus* status = 0) const;
    bool isElement(MStatus* status = 0) const;
    bool isCompound(MStatus* status = 0) const;
    bool isChild(MStatus* status = 0) const;
    bool isDynamic(MStatus* status = 0) const;
    bool isKeyable(MStatus* status = 0) const;
    MStatus setKeyable(bool on);
    bool isChannelBox(MStatus* status = 0) const;
    MStatus setChannelBox(bool on);
    bool isLocked(MStatus* status = 0) const;
    void setLocked(bool on, MStatus* status = 0);
    bool isConnected(MStatus* status = 0) const;
    bool isSource(MStatus* status = 0) const;
    bool isDestination(MStatus* status = 0) const;
    bool isFreeToChange(bool checkParents = true, bool checkChildren = true, MStatus* status = 0) const;
    unsigned int logicalIndex(MStatus* status = 0) const;
    MPlug array(MStatus* status = 0) const;
    MPlug parent(MStatus* status = 0) const;
    MPlug child(unsigned int i, MStatus* status = 0) const;
    MPlug child(const MObject& attr, MStatus* status = 0) const;
    unsigned int numChildren(MStatus* status = 0) const;
    unsigned int numElements(MStatus* status = 0) const;
    unsigned int evaluateNumElements(MStatus* status = 0) const;
    unsigned int numConnectedElements(MStatus* status = 0) const;
    unsigned int getExistingArrayAttributeIndices(MIntArray& indices, MStatus* status = 0) const;
    MPlug elementByLogicalIndex(unsigned int i, MStatus* status = 0) const;
    MPlug elementByPhysicalIndex(unsigned int i, MStatus* status = 0) const;
    MPlug operator[](unsigned int physical) const;
    bool connectedTo(MPlugArray& out, bool asDst, bool asSrc, MStatus* status = 0) const;
    MPlug source(MStatus* status = 0) const;
    MPlug sourceWithConversion(MStatus* status = 0) const;
    bool destinations(MPlugArray& out, MStatus* status = 0) const;
    MStatus selectAncestorLogicalIndex(unsigned int index, const MObject& attr = MObject::kNullObj);

    bool asBool(MStatus* status = 0) const;
    char asChar(MStatus* status = 0) const;
    short asShort(MStatus* status = 0) const;
    int asInt(MStatus* status = 0) const;
    int64_t asInt64(MStatus* status = 0) const;
    float asFloat(MStatus* status = 0) const;
    double asDouble(MStatus* status = 0) const;
    MString asString(MStatus* status = 0) const;
    MAngle asMAngle(MStatus* status = 0) const;
    MDistance asMDistance(MStatus* status = 0) const;
    MTime asMTime(MStatus* status = 0) const;
    MObject asMObject(MStatus* status = 0) const;
    MStatus getValue(bool& v) const;
    MStatus getValue(int& v) const;
    MStatus getValue(short& v) const;
    MStatus getValue(float& v) const;
    MStatus getValue(double& v) const;
    MStatus getValue(MString& v) const;
    MStatus getValue(MObject& v) const;
    MStatus getValue(MAngle& v) const;
    MStatus getValue(MDistance& v) const;
    MStatus getValue(MTime& v) const;
    MStatus setValue(bool v);
    MStatus setValue(int v);
    MStatus setValue(short v);
    MStatus setValue(float v);
    MStatus setValue(double v);
    MStatus setValue(const MString& v);
    MStatus setValue(const char* v);
    MStatus setValue(const MObject& data);
    MStatus setValue(const MAngle& v);
    MStatus setValue(const MDistance& v);
    MStatus setValue(const MTime& v);
    MStatus setBool(bool v);
    MStatus setChar(char v);
    MStatus setShort(short v);
    MStatus setInt(int v);
    MStatus setInt64(int64_t v);
    MStatus setFloat(float v);
    MStatus setDouble(double v);
    MStatus setString(const MString& v);
    MStatus setMObject(const MObject& v);
    MStatus setMAngle(const MAngle& v);
    MStatus setMDistance(const MDistance& v);
    MStatus setMTime(const MTime& v);

    // The kit's own: the plug's name as the host's commands take it, and what it is made of.
    std::string azPath() const;
    std::string azAttrPath() const;
    int64_t azNode() const { return node_; }
    const std::shared_ptr<az::AttrSpec>& azSpec() const { return spec_; }
    int azElement() const { return element_; }
    static MPlug azMake(int64_t node, const std::string& attr, const std::shared_ptr<az::AttrSpec>& spec, int element, int parentElement = -1);

private:
    int64_t node_;
    std::string attr_;  // long name; a vec3 child's own name
    std::shared_ptr<az::AttrSpec> spec_;
    int element_;        // logical index when an element of an array, else -1
    int parentElement_;  // the element of an array compound this is a child of, else -1
};

AZ_ARRAY(MPlugArray, MPlug)

class MDagPath {
public:
    MDagPath();
    MDagPath(const MDagPath& other);
    ~MDagPath();
    MDagPath& operator=(const MDagPath& other);
    bool operator==(const MDagPath& other) const;
    bool operator!=(const MDagPath& other) const;
    static MStatus getAPathTo(const MObject& node, MDagPath& path);
    static MStatus getAllPathsTo(const MObject& node, MDagPathArray& paths);
    bool isValid(MStatus* status = 0) const;
    bool hasFn(MFn::Type t, MStatus* status = 0) const;
    MFn::Type apiType(MStatus* status = 0) const;
    MObject node(MStatus* status = 0) const;
    MObject transform(MStatus* status = 0) const;
    unsigned int length(MStatus* status = 0) const;
    MStatus push(const MObject& child);
    MStatus pop(unsigned int levels = 1);
    unsigned int childCount(MStatus* status = 0) const;
    MObject child(unsigned int i, MStatus* status = 0) const;
    MStatus extendToShape();
    MStatus extendToShapeDirectlyBelow(unsigned int i);
    MStatus numberOfShapesDirectlyBelow(unsigned int& count) const;
    unsigned int instanceNumber(MStatus* status = 0) const;
    bool isInstanced(MStatus* status = 0) const;
    MMatrix inclusiveMatrix(MStatus* status = 0) const;
    MMatrix exclusiveMatrix(MStatus* status = 0) const;
    MMatrix inclusiveMatrixInverse(MStatus* status = 0) const;
    MMatrix exclusiveMatrixInverse(MStatus* status = 0) const;
    MString fullPathName(MStatus* status = 0) const;
    MString partialPathName(MStatus* status = 0) const;
    bool isVisible(MStatus* status = 0) const;
    MStatus set(const MDagPath& src);

    // The kit's own.
    static MDagPath azOf(int64_t node);
    int64_t azNode() const { return node_; }

private:
    int64_t node_;
};

AZ_ARRAY(MDagPathArray, MDagPath)

class MSelectionList {
public:
    enum MergeStrategy { kMergeNormal, kXORWithList, kRemoveFromList };
    MSelectionList();
    MSelectionList(const MSelectionList& other);
    ~MSelectionList();
    MSelectionList& operator=(const MSelectionList& other);
    MStatus clear();
    bool isEmpty(MStatus* status = 0) const;
    unsigned int length(MStatus* status = 0) const;
    MStatus add(const MString& pattern, bool searchChildNamespacesToo = false);
    MStatus add(const MObject& object, bool mergeWithExisting = false);
    MStatus add(const MDagPath& path, const MObject& component = MObject::kNullObj, bool mergeWithExisting = false);
    MStatus add(const MPlug& plug, bool mergeWithExisting = true);
    MStatus remove(unsigned int i);
    MStatus merge(const MSelectionList& other, MergeStrategy strategy = kMergeNormal);
    MStatus toggle(const MDagPath& path, const MObject& component);
    bool hasItem(const MDagPath& path, const MObject& component = MObject::kNullObj, MStatus* status = 0) const;
    bool hasItem(const MObject& object, MStatus* status = 0) const;
    MStatus getDependNode(unsigned int i, MObject& out) const;
    MStatus getDagPath(unsigned int i, MDagPath& out, MObject& component) const;
    MStatus getDagPath(unsigned int i, MDagPath& out) const;
    MStatus getPlug(unsigned int i, MPlug& out) const;
    MStatus getSelectionStrings(MStringArray& out) const;
    MStatus getSelectionStrings(unsigned int i, MStringArray& out) const;

    // The kit's own: the items as the host names them.
    const std::vector<std::string>& azItems() const { return items_; }

private:
    std::vector<std::string> items_;
};

class MGlobal {
public:
    enum ListAdjustment { kReplaceList, kXORWithList, kAddToList, kRemoveFromList, kAddToHeadOfList };
    enum MMayaState { kBatch, kInteractive, kBaseUIMode, kLibraryApp };
    enum MSelectionMode { kSelectObjectMode, kSelectComponentMode, kSelectRootMode, kSelectLeafMode, kSelectTemplateMode };
    static MStatus executeCommand(const MString& command, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executeCommand(const MString& command, int& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executeCommand(const MString& command, double& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executeCommand(const MString& command, MString& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executeCommand(const MString& command, MIntArray& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executeCommand(const MString& command, MDoubleArray& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executeCommand(const MString& command, MStringArray& result, bool displayEnabled = false, bool undoEnabled = false);
    static MString executeCommandStringResult(const MString& command, bool displayEnabled = false, bool undoEnabled = false, MStatus* status = 0);
    static MStatus executeCommandOnIdle(const MString& command, bool displayEnabled = false);
    static MStatus executePythonCommand(const MString& command, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executePythonCommand(const MString& command, int& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executePythonCommand(const MString& command, double& result, bool displayEnabled = false, bool undoEnabled = false);
    static MStatus executePythonCommand(const MString& command, MString& result, bool displayEnabled = false, bool undoEnabled = false);
    static MString executePythonCommandStringResult(const MString& command, bool displayEnabled = false, bool undoEnabled = false, MStatus* status = 0);
    static MStatus executePythonCommandOnIdle(const MString& command, bool displayEnabled = false);
    static MStatus sourceFile(const MString& file);
    static void displayInfo(const MString& text);
    static void displayWarning(const MString& text);
    static void displayError(const MString& text);
    static MStatus getActiveSelectionList(MSelectionList& out, bool orderedSelectionIfAvailable = false);
    static MStatus setActiveSelectionList(const MSelectionList& list, ListAdjustment how = kReplaceList);
    static MStatus getSelectionListByName(const MString& name, MSelectionList& out);
    static MStatus selectByName(const MString& name, ListAdjustment how = kAddToList);
    static MStatus select(const MObject& object, ListAdjustment how = kAddToList);
    static MStatus select(const MDagPath& path, const MObject& component, ListAdjustment how = kAddToList);
    static MStatus clearSelectionList();
    static MStatus getLiveList(MSelectionList& out);
    static MMayaState mayaState(MStatus* status = 0);
    static int apiVersion();
    static MString mayaVersion();
    static MString mayaName();
    static MString defaultErrorLogPathName(MStatus* status = 0);
    static bool optionVarIntValue(const MString& name, bool* exists = 0);
    static double optionVarDoubleValue(const MString& name, bool* exists = 0);
    static MString optionVarStringValue(const MString& name, bool* exists = 0);
    static bool setOptionVarValue(const MString& name, int value);
    static bool setOptionVarValue(const MString& name, double value);
    static bool setOptionVarValue(const MString& name, const MString& value);
    static bool removeOptionVar(const MString& name);
    static MStatus viewFrame(const MTime& time);
    static MStatus viewFrame(double frame);
    static bool isUndoing();
    static bool isRedoing();
    static MSelectionMode selectionMode(MStatus* status = 0);
    static MStatus setSelectionMode(MSelectionMode mode);
};

class MDGModifier {
public:
    MDGModifier();
    virtual ~MDGModifier();
    MObject createNode(const MString& type, MStatus* status = 0);
    MObject createNode(const MTypeId& id, MStatus* status = 0);
    MStatus deleteNode(const MObject& node, bool includeParents = false);
    MStatus renameNode(const MObject& node, const MString& name);
    MStatus connect(const MPlug& src, const MPlug& dst);
    MStatus connect(const MObject& srcNode, const MObject& srcAttr, const MObject& dstNode, const MObject& dstAttr);
    MStatus disconnect(const MPlug& src, const MPlug& dst);
    MStatus disconnect(const MObject& srcNode, const MObject& srcAttr, const MObject& dstNode, const MObject& dstAttr);
    MStatus addAttribute(const MObject& node, const MObject& attr);
    MStatus removeAttribute(const MObject& node, const MObject& attr);
    MStatus newPlugValue(const MPlug& plug, const MObject& data);
    MStatus newPlugValueBool(const MPlug& plug, bool v);
    MStatus newPlugValueChar(const MPlug& plug, char v);
    MStatus newPlugValueShort(const MPlug& plug, short v);
    MStatus newPlugValueInt(const MPlug& plug, int v);
    MStatus newPlugValueInt64(const MPlug& plug, int64_t v);
    MStatus newPlugValueFloat(const MPlug& plug, float v);
    MStatus newPlugValueDouble(const MPlug& plug, double v);
    MStatus newPlugValueString(const MPlug& plug, const MString& v);
    MStatus newPlugValueMAngle(const MPlug& plug, const MAngle& v);
    MStatus newPlugValueMDistance(const MPlug& plug, const MDistance& v);
    MStatus newPlugValueMTime(const MPlug& plug, const MTime& v);
    MStatus commandToExecute(const MString& command);
    MStatus pythonCommandToExecute(const MString& command);
    MStatus setNodeLockState(const MObject& node, bool locked);
    MStatus doIt();
    MStatus undoIt();

protected:
    struct Op {
        std::function<MStatus()> do_;
        std::function<MStatus()> undo_;
    };
    void azQueue(const std::function<MStatus()>& d, const std::function<MStatus()>& u);
    MObject azCreate(const std::string& type, int64_t parent, MStatus* status);
    std::vector<Op> ops_;
    size_t done_;
};

class MDagModifier : public MDGModifier {
public:
    MDagModifier();
    virtual ~MDagModifier();
    MObject createNode(const MString& type, const MObject& parent = MObject::kNullObj, MStatus* status = 0);
    MObject createNode(const MTypeId& id, const MObject& parent = MObject::kNullObj, MStatus* status = 0);
    MStatus reparentNode(const MObject& node, const MObject& newParent = MObject::kNullObj);
};

class MFnDependencyNode : public MFnBase {
public:
    enum MAttrClass { kLocalDynamicAttr = 1, kNormalAttr, kExtensionAttr, kInvalidAttr };
    MFnDependencyNode();
    MFnDependencyNode(const MObject& o, MStatus* status = 0);
    virtual ~MFnDependencyNode();
    virtual MFn::Type type() const;
    static const char* className() { return "MFnDependencyNode"; }
    MObject create(const MString& type, MStatus* status = 0);
    MObject create(const MString& type, const MString& name, MStatus* status = 0);
    MObject create(const MTypeId& id, MStatus* status = 0);
    MObject create(const MTypeId& id, const MString& name, MStatus* status = 0);
    MString name(MStatus* status = 0) const;
    MString absoluteName(MStatus* status = 0) const;
    MString setName(const MString& name, bool createNamespace = false, MStatus* status = 0);
    MString typeName(MStatus* status = 0) const;
    MTypeId typeId(MStatus* status = 0) const;
    MString pluginName(MStatus* status = 0) const;
    MString uuidString() const;
    MPlug findPlug(const MString& name, bool wantNetworkedPlug, MStatus* status = 0) const;
    MPlug findPlug(const MString& name, MStatus* status = 0) const;
    MPlug findPlug(const MObject& attr, bool wantNetworkedPlug, MStatus* status = 0) const;
    MPlug findPlug(const MObject& attr, MStatus* status = 0) const;
    unsigned int attributeCount(MStatus* status = 0) const;
    MObject attribute(unsigned int i, MStatus* status = 0) const;
    MObject attribute(const MString& name, MStatus* status = 0) const;
    bool hasAttribute(const MString& name, MStatus* status = 0) const;
    MStatus addAttribute(const MObject& attr);
    MStatus removeAttribute(const MObject& attr);
    MAttrClass attributeClass(const MObject& attr, MStatus* status = 0) const;
    MStatus getConnections(MPlugArray& out) const;
    bool isFromReferencedFile(MStatus* status = 0) const;
    bool isShared(MStatus* status = 0) const;
    bool isDefaultNode(MStatus* status = 0) const;
    bool isLocked(MStatus* status = 0) const;
    MStatus setLocked(bool locked);
    bool canBeWritten(MStatus* status = 0) const;
    MPxNode* userNode(MStatus* status = 0) const;
    MString classification(const MString& type);

protected:
    int64_t azId(MStatus* status) const;
};

class MFnDagNode : public MFnDependencyNode {
public:
    enum { kNextPos = 0xFFFFFFFF };
    MFnDagNode();
    MFnDagNode(const MObject& o, MStatus* status = 0);
    MFnDagNode(const MDagPath& path, MStatus* status = 0);
    virtual ~MFnDagNode();
    virtual MFn::Type type() const;
    static const char* className() { return "MFnDagNode"; }
    MStatus setObject(const MDagPath& path);
    virtual MStatus setObject(const MObject& o);
    MObject create(const MString& type, const MObject& parent = MObject::kNullObj, MStatus* status = 0);
    MObject create(const MString& type, const MString& name, const MObject& parent = MObject::kNullObj, MStatus* status = 0);
    unsigned int parentCount(MStatus* status = 0) const;
    MObject parent(unsigned int i, MStatus* status = 0) const;
    unsigned int childCount(MStatus* status = 0) const;
    MObject child(unsigned int i, MStatus* status = 0) const;
    MStatus addChild(MObject& child, unsigned int index = kNextPos, bool keepExistingParents = false);
    MStatus removeChild(MObject& child);
    bool hasParent(const MObject& node, MStatus* status = 0) const;
    bool hasChild(const MObject& node, MStatus* status = 0) const;
    bool isChildOf(const MObject& node, MStatus* status = 0) const;
    bool isParentOf(const MObject& node, MStatus* status = 0) const;
    MDagPath dagPath(MStatus* status = 0) const;
    MStatus getPath(MDagPath& path) const;
    MStatus getAllPaths(MDagPathArray& paths) const;
    MString fullPathName(MStatus* status = 0) const;
    MString partialPathName(MStatus* status = 0) const;
    MDagPath dagRoot(MStatus* status = 0);
    bool isIntermediateObject(MStatus* status = 0) const;
    MStatus setIntermediateObject(bool on);
    bool inUnderWorld(MStatus* status = 0) const;
    bool isInstanced(bool indirect = true, MStatus* status = 0) const;
    unsigned int instanceCount(bool total, MStatus* status = 0) const;
    MBoundingBox boundingBox(MStatus* status = 0) const;
    MMatrix transformationMatrix(MStatus* status = 0) const;
    MObject duplicate(bool instance = false, bool instanceLeaf = false, MStatus* status = 0) const;
    MColor objectColorRGB(MStatus* status = 0) const;

protected:
    MDagPath path_;
};

class MFnTransform : public MFnDagNode {
public:
    MFnTransform();
    MFnTransform(const MObject& o, MStatus* status = 0);
    MFnTransform(const MDagPath& path, MStatus* status = 0);
    virtual ~MFnTransform();
    virtual MFn::Type type() const;
    static const char* className() { return "MFnTransform"; }
    MObject create(const MObject& parent = MObject::kNullObj, MStatus* status = 0);
    MTransformationMatrix transformation(MStatus* status = 0) const;
    MStatus set(const MTransformationMatrix& m);
    MVector getTranslation(MSpace::Space space, MStatus* status = 0) const;
    MVector translation(MSpace::Space space, MStatus* status = 0) const;
    MStatus setTranslation(const MVector& v, MSpace::Space space);
    MStatus translateBy(const MVector& v, MSpace::Space space);
    MStatus getRotation(MQuaternion& q, MSpace::Space space = MSpace::kTransform) const;
    MStatus getRotation(MEulerRotation& e) const;
    MStatus getRotation(double r[3], MTransformationMatrix::RotationOrder& order) const;
    MStatus setRotation(const MQuaternion& q, MSpace::Space space = MSpace::kTransform);
    MStatus setRotation(const MEulerRotation& e);
    MStatus setRotation(const double r[3], MTransformationMatrix::RotationOrder order);
    MStatus rotateBy(const MQuaternion& q, MSpace::Space space = MSpace::kTransform);
    MStatus rotateBy(const MEulerRotation& e, MSpace::Space space = MSpace::kTransform);
    MStatus getRotationQuaternion(double& x, double& y, double& z, double& w, MSpace::Space space = MSpace::kTransform) const;
    MStatus setRotationQuaternion(double x, double y, double z, double w, MSpace::Space space = MSpace::kTransform);
    MTransformationMatrix::RotationOrder rotationOrder(MStatus* status = 0) const;
    MStatus setRotationOrder(MTransformationMatrix::RotationOrder order, bool reorder);
    MStatus getScale(double s[3]) const;
    MStatus setScale(const double s[3]);
    MStatus scaleBy(const double s[3]);
    MStatus getShear(double s[3]) const;
    MStatus setShear(const double s[3]);
    MPoint rotatePivot(MSpace::Space space, MStatus* status = 0) const;
    MStatus setRotatePivot(const MPoint& p, MSpace::Space space, bool balance = true);
    MPoint scalePivot(MSpace::Space space, MStatus* status = 0) const;
    MStatus setScalePivot(const MPoint& p, MSpace::Space space, bool balance = true);
    MQuaternion rotateOrientation(MSpace::Space space, MStatus* status = 0) const;
    MStatus setRotateOrientation(const MQuaternion& q, MSpace::Space space, bool balance = true);
    MStatus resetFromRestPosition();
};

#endif
