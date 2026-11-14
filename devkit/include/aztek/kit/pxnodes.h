// Aztek plug-in kit: transforms, constraints, shapes, data and IK solvers of a plug-in's own.
#pragma once
#include "viewport.h"
#include "anim.h"

// ---------------------------------------------------------------------------------------------------------------------
// Transforms

// What a plug-in's transform makes its matrix from. asMatrix puts the parts together as the reference does:
// [Sp]^-1 [S] [Sh] [Sp] [St] [Rp]^-1 [Ra] [R] [Rp] [Rt] [T], points as rows.
class MPxTransformationMatrix {
public:
    enum RotationOrder { kInvalid, kXYZ, kYZX, kZXY, kXZY, kYXZ, kZYX, kLast };
    MPxTransformationMatrix();
    MPxTransformationMatrix(const MMatrix& m);
    MPxTransformationMatrix(const MPxTransformationMatrix& other);
    virtual ~MPxTransformationMatrix();
    MPxTransformationMatrix& operator=(const MPxTransformationMatrix& other);
    virtual void copyValues(MPxTransformationMatrix* other);
    virtual MTypeId typeId() const;
    bool operator==(const MPxTransformationMatrix& other) const;
    bool operator!=(const MPxTransformationMatrix& other) const;
    bool isEquivalent(const MPxTransformationMatrix& other, double tolerance = 1e-10) const;
    virtual MStatus reverse();
    virtual MStatus transformBy(const MTransformationMatrix& xform, MSpace::Space space = MSpace::kTransform, MPxTransformationMatrix::RotationOrder* order = 0);
    virtual MMatrix asMatrix() const;
    virtual MMatrix asMatrix(double percent) const;
    virtual MMatrix asMatrixInverse() const;
    virtual MMatrix asScaleMatrix() const;
    virtual MMatrix asRotateMatrix() const;
    virtual MStatus decomposeMatrix(const MMatrix& m);
    virtual MVector translation(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus translateTo(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MStatus translateBy(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MQuaternion rotation(MStatus* status = 0) const;
    virtual MEulerRotation eulerRotation(MStatus* status = 0) const;
    virtual MStatus rotateTo(const MQuaternion& q, MSpace::Space space = MSpace::kTransform);
    virtual MStatus rotateTo(const MEulerRotation& e, MSpace::Space space = MSpace::kTransform);
    virtual MStatus rotateBy(const MQuaternion& q, MSpace::Space space = MSpace::kTransform);
    virtual MStatus rotateBy(const MEulerRotation& e, MSpace::Space space = MSpace::kTransform);
    RotationOrder rotationOrder(MStatus* status = 0) const;
    virtual MStatus setRotationOrder(RotationOrder order, bool preserve = true);
    virtual MVector scale(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus scaleTo(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MStatus scaleBy(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MVector shear(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus shearTo(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MStatus shearBy(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MPoint scalePivot(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus setScalePivot(const MPoint& p, MSpace::Space space = MSpace::kTransform, bool balance = true);
    virtual MVector scalePivotTranslation(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus setScalePivotTranslation(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MPoint rotatePivot(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus setRotatePivot(const MPoint& p, MSpace::Space space = MSpace::kTransform, bool balance = true);
    virtual MVector rotatePivotTranslation(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus setRotatePivotTranslation(const MVector& v, MSpace::Space space = MSpace::kTransform);
    virtual MQuaternion rotateOrientation(MSpace::Space space = MSpace::kTransform, MStatus* status = 0) const;
    virtual MStatus setRotateOrientation(const MQuaternion& q, MSpace::Space space = MSpace::kTransform, bool balance = true);
    virtual MStatus setRotateOrientation(const MEulerRotation& e, MSpace::Space space = MSpace::kTransform, bool balance = true);
    MTransformationMatrix asTransformationMatrix() const;
    static MPxTransformationMatrix* creator();
    static MTypeId baseTransformationMatrixId;
    static const MPxTransformationMatrix identity;
    static MEulerRotation::RotationOrder convertRotationOrder(RotationOrder order, MStatus* status = 0);
    static RotationOrder convertRotationOrder(MEulerRotation::RotationOrder order, MStatus* status = 0);

protected:
    MVector translationValue;
    MVector scaleValue;
    MEulerRotation rotationValue;
    MQuaternion rotateOrientationValue;
    MVector shearValue;
    MPoint scalePivotValue;
    MVector scalePivotTranslationValue;
    MPoint rotatePivotValue;
    MVector rotatePivotTranslationValue;
};

class MPxTransform : public MPxNode {
public:
    MPxTransform();
    MPxTransform(MPxTransformationMatrix* tm);
    virtual ~MPxTransform();
    virtual Type type() const;
    virtual void postConstructor();
    virtual MPxTransformationMatrix* createTransformationMatrix();
    virtual bool isBounded() const;
    virtual MBoundingBox boundingBox() const;
    virtual bool treatAsTransform() const;
    // The matrix from the data block: by default the transform's channels into the transformation matrix.
    virtual MStatus computeLocalTransformation(MPxTransformationMatrix* xform, MDataBlock& block);
    virtual MStatus compute(const MPlug& plug, MDataBlock& data);
    // Taken and not called: channels set here are not offered to the plug-in first.
    virtual MStatus validateAndSetValue(const MPlug& plug, const MDataHandle& handle);
    virtual MMatrix getMatrix(const MDGContext& context = MDGContext::fsNormal, MStatus* status = 0);
    virtual MMatrix getMatrixInverse(const MDGContext& context = MDGContext::fsNormal, MStatus* status = 0);
    virtual void resetTransformation(const MMatrix& m);
    virtual void resetTransformation(MPxTransformationMatrix* xform);
    MPxTransformationMatrix* transformationMatrixPtr() const;
    const MPxTransformationMatrix& transformationMatrix() const;
    MPxTransformationMatrix* baseTransformationMatrix;

    static MObject nodeBoundingBox;
    static MObject translate, translateX, translateY, translateZ;
    static MObject rotate, rotateX, rotateY, rotateZ;
    static MObject rotateOrder;
    static MObject scale, scaleX, scaleY, scaleZ;
    static MObject shear, shearXY, shearXZ, shearYZ;
    static MObject rotatePivot, rotatePivotX, rotatePivotY, rotatePivotZ;
    static MObject rotatePivotTranslate, rotatePivotTranslateX, rotatePivotTranslateY, rotatePivotTranslateZ;
    static MObject scalePivot, scalePivotX, scalePivotY, scalePivotZ;
    static MObject scalePivotTranslate, scalePivotTranslateX, scalePivotTranslateY, scalePivotTranslateZ;
    static MObject rotateAxis, rotateAxisX, rotateAxisY, rotateAxisZ;
    static MObject inheritsTransform;
    static MObject displayHandle, displayRotatePivot, displayScalePivot, displayLocalAxis;
    static MObject matrix, inverseMatrix, worldMatrix, worldInverseMatrix, parentMatrix, parentInverseMatrix, xformMatrix;
    static MObject offsetParentMatrix;
    static MObject visibility;
};

// ---------------------------------------------------------------------------------------------------------------------
// Constraints

// A constraint node: in the DAG, under the object it drives. Its compute makes the outputs the command connects.
class MPxConstraint : public MPxNode {
public:
    enum TargetType { kTargetTypeNone, kTargetTypeTransform, kTargetTypeGeometry };
    MPxConstraint();
    virtual ~MPxConstraint();
    virtual Type type() const;
    virtual MStatus getOutputAttributes(MObjectArray& attributeArray);
    virtual const MObject passiveOutputAttribute() const;
    virtual const MObject weightAttribute() const;
    virtual const MObject targetAttribute() const;
    virtual void getOutputAttributes(MObjectArray& attributeArray) const;
    virtual MTypeId constraintTypeId() const;
    virtual MPxConstraint::TargetType targetType() const;
    static MObject enableRestPosition;
    static MObject lockOutput;
};

// The command that makes a constraint: the last object named (or selected) is driven, the others are its targets. The
// virtual functions name the attributes it connects.
class MPxConstraintCommand : public MPxCommand {
public:
    enum TargetType { kTransform, kGeometryShape };
    MPxConstraintCommand();
    virtual ~MPxConstraintCommand();
    virtual MStatus doIt(const MArgList& args);
    virtual MStatus undoIt();
    virtual MStatus redoIt();
    virtual bool isUndoable() const;
    virtual MStatus appendSyntax();
    virtual MStatus parseArgs(const MArgList& argList);
    virtual MTypeId constraintTypeId() const;
    virtual const MObject& constraintInstancedAttribute() const;
    virtual const MObject& constraintOutputAttribute() const;
    virtual const MObject& constraintTargetInstancedAttribute() const;
    virtual const MObject& constraintTargetAttribute() const;
    virtual const MObject& constraintTargetWeightAttribute() const;
    virtual const MObject& objectAttribute() const;
    virtual TargetType targetType() const;
    virtual MStatus connectTarget(MDagPath& targetPath, int index);
    virtual MStatus connectObjectAndConstraint(MDGModifier& modifier);
    virtual void createdConstraint(MPxConstraint* constraint);
    virtual MStatus setRestPosition(MObject& restPosition);
    virtual bool supportsOffset() const;
    virtual bool hasVectorFlags() const;
    virtual bool hasUpVectorFlags() const;
    virtual MStatus handleNewTargets(MObject& constraint);
    MStatus connectTargetAttribute(MDagPath& targetPath, int index, MObject& targetAttribute, MObject& constraintAttribute, bool targetAttrIsArray = false);
    MStatus connectObjectAttribute(MDagPath& childPath, MObject& objectAttribute, MObject& constraintAttribute, bool objectAttrIsArray = false);
    MStatus connectTargetMatrixAttribute(MDagPath& targetPath, int index, MObject& constraintAttribute);
    MSyntax& syntax();

    // The kit's own.
    MSyntax azSyntax;
    MObject azConstraint;
};

// ---------------------------------------------------------------------------------------------------------------------
// Shapes

// What can be picked: kept for the plug-ins that name it; nothing picks with it yet.
class MSelectionMask {
public:
    enum SelectionType { kSelectHandles, kSelectLocalAxis, kSelectObjectsMask, kSelectNurbsCurves, kSelectNurbsSurfaces, kSelectMeshes, kSelectPointsForGravity, kSelectComponentsMask, kSelectCVs, kSelectMeshVerts, kSelectMeshEdges, kSelectMeshFaces, kSelectVertices, kSelectEdges, kSelectFacets, kSelectJoints, kSelectLocators, kSelectIkHandles, kSelectCurves, kSelectSurfaces, kSelectAll };
    MSelectionMask();
    MSelectionMask(SelectionType type);
    MStatus setMask(SelectionType type);
    MStatus addMask(SelectionType type);
    bool intersects(SelectionType type, MStatus* status = 0) const;
    bool intersects(const MSelectionMask& other, MStatus* status = 0) const;

private:
    unsigned long long bits_;
};
class MDrawRequest {};
class MSelectInfo {};

// A shape whose geometry is the plug-in's. Its bounding box is the plug-in's; nothing draws it yet.
class MPxSurfaceShape : public MPxNode {
public:
    enum MatchResult { kMatchOk, kMatchNone, kMatchTooMany, kMatchInvalidName, kMatchInvalidAttribute, kMatchInvalidAttributeIndex,
                       kMatchInvalidAttributeRange, kMatchInvalidAttributeDim };
    enum MVertexCachingMode { kNormal = 0, kUpdate, kReset, kRestore };
    enum MBoundingBoxChangedMode { kBoundingBoxChanged, kObjectChanged, kVertexChanged };
    MPxSurfaceShape();
    virtual ~MPxSurfaceShape();
    virtual Type type() const;
    virtual bool isBounded() const;
    virtual MBoundingBox boundingBox() const;
    virtual MObject geometryData() const;
    virtual MObject localShapeInAttr() const;
    virtual MObject localShapeOutAttr() const;
    virtual MObject worldShapeOutAttr() const;
    virtual MObject cachedShapeAttr() const;
    virtual bool deleteComponents(const MObjectArray& componentList, MDoubleArray& undoInfo);
    virtual bool undeleteComponents(const MObjectArray& componentList, MDoubleArray& undoInfo);
    virtual void componentToPlugs(MObject& component, MSelectionList& selectionList) const;
    virtual bool match(const MSelectionMask& mask, const MObjectArray& componentList) const;
    virtual MObject createFullVertexGroup() const;
    virtual MObject createFullRenderGroup() const;
    virtual MSelectionMask getShapeSelectionMask() const;
    virtual MSelectionMask getComponentSelectionMask() const;
    virtual bool excludeAsPluginShape() const;
    virtual void transformUsing(const MMatrix& mat, const MObjectArray& componentList, MVertexCachingMode cachingMode = kNormal, MPointArray* pointCache = 0);
    virtual void tweakUsing(const MMatrix& mat, const MObjectArray& componentList, MVertexCachingMode cachingMode, MPointArray* pointCache, MArrayDataHandle& handle);
    void childChanged(MBoundingBoxChangedMode mode = kBoundingBoxChanged);

    static MObject isRenderable;
    static MObject mControlPoints, mControlValueX, mControlValueY, mControlValueZ;
    static MObject mHasHistoryOnCreate;
    static MObject nodeBoundingBox, nodeBoundingBoxMin, nodeBoundingBoxMax, nodeBoundingBoxSize;
};

// What would draw a shape in the reference's legacy viewport: kept so plug-ins build, never called.
class MPxSurfaceShapeUI {
public:
    MPxSurfaceShapeUI();
    virtual ~MPxSurfaceShapeUI();
    virtual void draw(const MDrawRequest& request, M3dView& view) const;
    virtual bool select(MSelectInfo& selectInfo, MSelectionList& selectionList, MPointArray& worldSpaceSelectPts) const;
    MPxSurfaceShape* surfaceShape() const;
    static MPxSurfaceShapeUI* surfaceShapeUI(const MDagPath& path, MStatus* status = 0);
};

// ---------------------------------------------------------------------------------------------------------------------
// Data

class MFnPluginData : public MFnData {
public:
    MFnPluginData();
    MFnPluginData(MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    MTypeId typeId(MStatus* status = 0) const;
    // The value: the object a creator made. A value read from a plug is the host's; leave it as it is.
    MPxData* data(MStatus* status = 0);
    const MPxData* constData(MStatus* status = 0) const;
    MObject create(const MTypeId& id, MStatus* status = 0);
};

// ---------------------------------------------------------------------------------------------------------------------
// IK solvers

class MIkHandleGroup {
public:
    MIkHandleGroup();
    virtual ~MIkHandleGroup();
    int priority(MStatus* status = 0) const;
    int solverID(MStatus* status = 0) const;
    bool solverPriority(int*, MStatus* status = 0) const;
    MStatus setPriority(int priority);
    MStatus setSolverID(int id);
    MStatus checkEffectorAtGoal();
    bool isUnderControl(MStatus* status = 0) const;
    unsigned int handleCount() const;
    MObject handle(unsigned int index);
    unsigned int dofCount() const;
    unsigned int solve(MStatus* status = 0);

    // The kit's own.
    std::vector<int64_t> azHandles;
    int azPriority, azSolver;
};

// While a solver solves, its handle, effector and joints answer from what the host sent: their channels, matrices and
// world positions as the solve leaves them. Nothing else in the scene can be reached.
class MPxIkSolverNode : public MPxNode {
public:
    MPxIkSolverNode();
    virtual ~MPxIkSolverNode();
    virtual Type type() const;
    virtual MStatus preSolve();
    virtual MStatus doSolve();
    virtual MStatus postSolve(MStatus status);
    virtual MString solverTypeName() const;
    bool rotatePlane(MStatus* status = 0) const;
    MStatus setRotatePlane(bool rotatePlane);
    bool singleChainOnly(MStatus* status = 0) const;
    MStatus setSingleChainOnly(bool singleChainOnly);
    bool positionOnly(MStatus* status = 0) const;
    MStatus setPositionOnly(bool positionOnly);
    bool supportJointLimits(MStatus* status = 0) const;
    MStatus setSupportJointLimits(bool supportJointLimits);
    bool uniqueSolution(MStatus* status = 0) const;
    MStatus setUniqueSolution(bool uniqueSolution);
    virtual bool isSingleChainOnly() const;
    virtual bool isPositionOnly() const;
    virtual bool hasJointLimitSupport() const;
    virtual bool hasUniqueSolution() const;
    virtual bool groupHandlesByTopology() const;
    virtual MStatus setFuncValueTolerance(double tolerance);
    virtual MStatus setMaxIterations(int value);
    MIkHandleGroup* handleGroup() const;
    virtual void setHandleGroup(MIkHandleGroup* group);
    const MMatrix* toWorldSpace() const;
    const MMatrix* toSolverSpace() const;
    double funcValueTolerance() const;
    int maxIterations() const;
    virtual void snapHandle(MObject& handle);
    virtual bool isAttributeCreatedBySolver(MObject& attr) const;

private:
    MIkHandleGroup* group_;
    MMatrix toWorld_, toSolver_;
    double tolerance_;
    int iterations_;
    bool rotatePlane_, singleChain_, positionOnly_, limits_, unique_;
};

class MFnIkHandle : public MFnTransform {
public:
    MFnIkHandle();
    MFnIkHandle(const MObject& o, MStatus* status = 0);
    MFnIkHandle(const MDagPath& path, MStatus* status = 0);
    virtual MFn::Type type() const;
    MStatus getStartJoint(MDagPath& jointPath) const;
    MStatus getEffector(MDagPath& effectorPath) const;
    MObject solver(MStatus* status = 0) const;
    double priority(MStatus* status = 0) const;
    double weight(MStatus* status = 0) const;
    double poWeight(MStatus* status = 0) const;
};

class MFnIkEffector : public MFnTransform {
public:
    MFnIkEffector();
    MFnIkEffector(const MObject& o, MStatus* status = 0);
    MFnIkEffector(const MDagPath& path, MStatus* status = 0);
    virtual MFn::Type type() const;
};
