// Aztek plug-in kit: transforms, constraints, shapes, data types and IK solvers of a plug-in's own.
#include "internal.h"
#include <cmath>
#include <sstream>

using namespace az;

static void ok(MStatus* st) {
    if (st) *st = MS::kSuccess;
}

static MMatrix tmat(const MVector& v) {
    MMatrix m;
    m[3][0] = v.x;
    m[3][1] = v.y;
    m[3][2] = v.z;
    return m;
}

static MMatrix scaling(const MVector& v) {
    MMatrix m;
    m[0][0] = v.x;
    m[1][1] = v.y;
    m[2][2] = v.z;
    return m;
}

// The reference's shear: x picks up xy of y and xz of z, y picks up yz of z (points as rows).
static MMatrix shearing(const MVector& v) {
    MMatrix m;
    m[1][0] = v.x;
    m[2][0] = v.y;
    m[2][1] = v.z;
    return m;
}

static std::vector<double> flat(const MMatrix& m) {
    std::vector<double> out;
    for (int r = 0; r < 4; ++r)
        for (int c = 0; c < 4; ++c) out.push_back(m[r][c]);
    return out;
}

static Json flatJson(const MMatrix& m) {
    Json a = Json::array();
    for (int r = 0; r < 4; ++r)
        for (int c = 0; c < 4; ++c) a.push(m[r][c]);
    return a;
}

static MMatrix fromFlat(const Json& j) {
    MMatrix m;
    if (j.size() < 16) return m;
    for (int r = 0; r < 4; ++r)
        for (int c = 0; c < 4; ++c) m[r][c] = j[(size_t)(r * 4 + c)].num();
    return m;
}

static MVector vecOf(const Json& j) { return j.size() >= 3 ? MVector(j[(size_t)0].num(), j[(size_t)1].num(), j[(size_t)2].num()) : MVector(); }

// ---------------------------------------------------------------------------------------------------------------------
// MPxTransformationMatrix

MTypeId MPxTransformationMatrix::baseTransformationMatrixId(0x80000, 0);
const MPxTransformationMatrix MPxTransformationMatrix::identity;

MPxTransformationMatrix::MPxTransformationMatrix() : scaleValue(1, 1, 1), scalePivotValue(0, 0, 0), rotatePivotValue(0, 0, 0) {}
MPxTransformationMatrix::MPxTransformationMatrix(const MMatrix& m) : scaleValue(1, 1, 1), scalePivotValue(0, 0, 0), rotatePivotValue(0, 0, 0) { decomposeMatrix(m); }
MPxTransformationMatrix::MPxTransformationMatrix(const MPxTransformationMatrix& o) { *this = o; }
MPxTransformationMatrix::~MPxTransformationMatrix() {}
MPxTransformationMatrix& MPxTransformationMatrix::operator=(const MPxTransformationMatrix& o) {
    translationValue = o.translationValue;
    scaleValue = o.scaleValue;
    rotationValue = o.rotationValue;
    rotateOrientationValue = o.rotateOrientationValue;
    shearValue = o.shearValue;
    scalePivotValue = o.scalePivotValue;
    scalePivotTranslationValue = o.scalePivotTranslationValue;
    rotatePivotValue = o.rotatePivotValue;
    rotatePivotTranslationValue = o.rotatePivotTranslationValue;
    return *this;
}
void MPxTransformationMatrix::copyValues(MPxTransformationMatrix* o) {
    if (o) *this = *o;
}
MTypeId MPxTransformationMatrix::typeId() const { return baseTransformationMatrixId; }
bool MPxTransformationMatrix::operator==(const MPxTransformationMatrix& o) const { return isEquivalent(o, 0.0); }
bool MPxTransformationMatrix::operator!=(const MPxTransformationMatrix& o) const { return !isEquivalent(o, 0.0); }
bool MPxTransformationMatrix::isEquivalent(const MPxTransformationMatrix& o, double tol) const { return asMatrix().isEquivalent(o.asMatrix(), tol); }
MStatus MPxTransformationMatrix::reverse() { return decomposeMatrix(asMatrixInverse()); }
MStatus MPxTransformationMatrix::transformBy(const MTransformationMatrix& x, MSpace::Space, RotationOrder*) { return decomposeMatrix(asMatrix() * x.asMatrix()); }

MMatrix MPxTransformationMatrix::asMatrix() const {
    MVector sp(scalePivotValue), rp(rotatePivotValue);
    return tmat(-sp) * scaling(scaleValue) * shearing(shearValue) * tmat(sp) * tmat(scalePivotTranslationValue) * tmat(-rp) *
           rotateOrientationValue.asMatrix() * rotationValue.asMatrix() * tmat(rp) * tmat(rotatePivotTranslationValue) * tmat(translationValue);
}
MMatrix MPxTransformationMatrix::asMatrix(double percent) const {
    MPxTransformationMatrix part(*this);
    part.translationValue = translationValue * percent;
    part.rotationValue = rotationValue * percent;
    part.scaleValue = MVector(1, 1, 1) + (scaleValue - MVector(1, 1, 1)) * percent;
    part.shearValue = shearValue * percent;
    return part.asMatrix();
}
MMatrix MPxTransformationMatrix::asMatrixInverse() const { return asMatrix().inverse(); }
MMatrix MPxTransformationMatrix::asScaleMatrix() const {
    MVector sp(scalePivotValue);
    return tmat(-sp) * scaling(scaleValue) * shearing(shearValue) * tmat(sp) * tmat(scalePivotTranslationValue);
}
MMatrix MPxTransformationMatrix::asRotateMatrix() const {
    MVector rp(rotatePivotValue);
    return tmat(-rp) * rotateOrientationValue.asMatrix() * rotationValue.asMatrix() * tmat(rp) * tmat(rotatePivotTranslationValue);
}
MStatus MPxTransformationMatrix::decomposeMatrix(const MMatrix& m) {
    MTransformationMatrix t(m);
    translationValue = t.getTranslation(MSpace::kTransform);
    double s[3], sh[3];
    t.getScale(s, MSpace::kTransform);
    t.getShear(sh, MSpace::kTransform);
    scaleValue = MVector(s[0], s[1], s[2]);
    shearValue = MVector(sh[0], sh[1], sh[2]);
    rotationValue = MEulerRotation::decompose(t.asRotateMatrix(), rotationValue.order);
    rotateOrientationValue = MQuaternion();
    scalePivotValue = rotatePivotValue = MPoint(0, 0, 0);
    scalePivotTranslationValue = rotatePivotTranslationValue = MVector();
    return MS::kSuccess;
}
MVector MPxTransformationMatrix::translation(MSpace::Space, MStatus* st) const {
    ok(st);
    return translationValue;
}
MStatus MPxTransformationMatrix::translateTo(const MVector& v, MSpace::Space) {
    translationValue = v;
    return MS::kSuccess;
}
MStatus MPxTransformationMatrix::translateBy(const MVector& v, MSpace::Space) {
    translationValue += v;
    return MS::kSuccess;
}
MQuaternion MPxTransformationMatrix::rotation(MStatus* st) const {
    ok(st);
    return rotationValue.asQuaternion();
}
MEulerRotation MPxTransformationMatrix::eulerRotation(MStatus* st) const {
    ok(st);
    return rotationValue;
}
MStatus MPxTransformationMatrix::rotateTo(const MQuaternion& q, MSpace::Space) {
    rotationValue = MEulerRotation::decompose(q.asMatrix(), rotationValue.order);
    return MS::kSuccess;
}
MStatus MPxTransformationMatrix::rotateTo(const MEulerRotation& e, MSpace::Space) {
    rotationValue = e.reorder(rotationValue.order);
    return MS::kSuccess;
}
MStatus MPxTransformationMatrix::rotateBy(const MQuaternion& q, MSpace::Space) { return rotateTo(rotationValue.asQuaternion() * q); }
MStatus MPxTransformationMatrix::rotateBy(const MEulerRotation& e, MSpace::Space s) { return rotateBy(e.asQuaternion(), s); }
MPxTransformationMatrix::RotationOrder MPxTransformationMatrix::rotationOrder(MStatus* st) const { return convertRotationOrder(rotationValue.order, st); }
MStatus MPxTransformationMatrix::setRotationOrder(RotationOrder order, bool preserve) {
    MStatus st;
    MEulerRotation::RotationOrder o = convertRotationOrder(order, &st);
    if (!st) return st;
    if (preserve)
        rotationValue.reorderIt(o);
    else
        rotationValue.order = o;
    return MS::kSuccess;
}
MVector MPxTransformationMatrix::scale(MSpace::Space, MStatus* st) const {
    ok(st);
    return scaleValue;
}
MStatus MPxTransformationMatrix::scaleTo(const MVector& v, MSpace::Space) {
    scaleValue = v;
    return MS::kSuccess;
}
MStatus MPxTransformationMatrix::scaleBy(const MVector& v, MSpace::Space) {
    scaleValue = MVector(scaleValue.x * v.x, scaleValue.y * v.y, scaleValue.z * v.z);
    return MS::kSuccess;
}
MVector MPxTransformationMatrix::shear(MSpace::Space, MStatus* st) const {
    ok(st);
    return shearValue;
}
MStatus MPxTransformationMatrix::shearTo(const MVector& v, MSpace::Space) {
    shearValue = v;
    return MS::kSuccess;
}
MStatus MPxTransformationMatrix::shearBy(const MVector& v, MSpace::Space) {
    shearValue = MVector(shearValue.x * v.x, shearValue.y * v.y, shearValue.z * v.z);
    return MS::kSuccess;
}
MPoint MPxTransformationMatrix::scalePivot(MSpace::Space, MStatus* st) const {
    ok(st);
    return scalePivotValue;
}
MStatus MPxTransformationMatrix::setScalePivot(const MPoint& p, MSpace::Space, bool) {
    scalePivotValue = p;
    return MS::kSuccess;
}
MVector MPxTransformationMatrix::scalePivotTranslation(MSpace::Space, MStatus* st) const {
    ok(st);
    return scalePivotTranslationValue;
}
MStatus MPxTransformationMatrix::setScalePivotTranslation(const MVector& v, MSpace::Space) {
    scalePivotTranslationValue = v;
    return MS::kSuccess;
}
MPoint MPxTransformationMatrix::rotatePivot(MSpace::Space, MStatus* st) const {
    ok(st);
    return rotatePivotValue;
}
MStatus MPxTransformationMatrix::setRotatePivot(const MPoint& p, MSpace::Space, bool) {
    rotatePivotValue = p;
    return MS::kSuccess;
}
MVector MPxTransformationMatrix::rotatePivotTranslation(MSpace::Space, MStatus* st) const {
    ok(st);
    return rotatePivotTranslationValue;
}
MStatus MPxTransformationMatrix::setRotatePivotTranslation(const MVector& v, MSpace::Space) {
    rotatePivotTranslationValue = v;
    return MS::kSuccess;
}
MQuaternion MPxTransformationMatrix::rotateOrientation(MSpace::Space, MStatus* st) const {
    ok(st);
    return rotateOrientationValue;
}
MStatus MPxTransformationMatrix::setRotateOrientation(const MQuaternion& q, MSpace::Space, bool) {
    rotateOrientationValue = q;
    return MS::kSuccess;
}
MStatus MPxTransformationMatrix::setRotateOrientation(const MEulerRotation& e, MSpace::Space s, bool b) { return setRotateOrientation(e.asQuaternion(), s, b); }
MTransformationMatrix MPxTransformationMatrix::asTransformationMatrix() const { return MTransformationMatrix(asMatrix()); }
MPxTransformationMatrix* MPxTransformationMatrix::creator() { return new MPxTransformationMatrix(); }
MEulerRotation::RotationOrder MPxTransformationMatrix::convertRotationOrder(RotationOrder o, MStatus* st) {
    if (o <= kInvalid || o >= kLast) {
        if (st) *st = MS::kInvalidParameter;
        return MEulerRotation::kXYZ;
    }
    ok(st);
    return (MEulerRotation::RotationOrder)(o - 1);
}
MPxTransformationMatrix::RotationOrder MPxTransformationMatrix::convertRotationOrder(MEulerRotation::RotationOrder o, MStatus* st) {
    ok(st);
    return (RotationOrder)(o + 1);
}

// ---------------------------------------------------------------------------------------------------------------------
// MPxTransform

#define BUILTIN(cls, var, name) MObject cls::var = attrObject(builtinAttr(name));
BUILTIN(MPxTransform, nodeBoundingBox, "nodeBoundingBox")
BUILTIN(MPxTransform, translate, "translate")
BUILTIN(MPxTransform, translateX, "translateX")
BUILTIN(MPxTransform, translateY, "translateY")
BUILTIN(MPxTransform, translateZ, "translateZ")
BUILTIN(MPxTransform, rotate, "rotate")
BUILTIN(MPxTransform, rotateX, "rotateX")
BUILTIN(MPxTransform, rotateY, "rotateY")
BUILTIN(MPxTransform, rotateZ, "rotateZ")
BUILTIN(MPxTransform, rotateOrder, "rotateOrder")
BUILTIN(MPxTransform, scale, "scale")
BUILTIN(MPxTransform, scaleX, "scaleX")
BUILTIN(MPxTransform, scaleY, "scaleY")
BUILTIN(MPxTransform, scaleZ, "scaleZ")
BUILTIN(MPxTransform, shear, "shear")
BUILTIN(MPxTransform, shearXY, "shearXY")
BUILTIN(MPxTransform, shearXZ, "shearXZ")
BUILTIN(MPxTransform, shearYZ, "shearYZ")
BUILTIN(MPxTransform, rotatePivot, "rotatePivot")
BUILTIN(MPxTransform, rotatePivotX, "rotatePivotX")
BUILTIN(MPxTransform, rotatePivotY, "rotatePivotY")
BUILTIN(MPxTransform, rotatePivotZ, "rotatePivotZ")
BUILTIN(MPxTransform, rotatePivotTranslate, "rotatePivotTranslate")
BUILTIN(MPxTransform, rotatePivotTranslateX, "rotatePivotTranslateX")
BUILTIN(MPxTransform, rotatePivotTranslateY, "rotatePivotTranslateY")
BUILTIN(MPxTransform, rotatePivotTranslateZ, "rotatePivotTranslateZ")
BUILTIN(MPxTransform, scalePivot, "scalePivot")
BUILTIN(MPxTransform, scalePivotX, "scalePivotX")
BUILTIN(MPxTransform, scalePivotY, "scalePivotY")
BUILTIN(MPxTransform, scalePivotZ, "scalePivotZ")
BUILTIN(MPxTransform, scalePivotTranslate, "scalePivotTranslate")
BUILTIN(MPxTransform, scalePivotTranslateX, "scalePivotTranslateX")
BUILTIN(MPxTransform, scalePivotTranslateY, "scalePivotTranslateY")
BUILTIN(MPxTransform, scalePivotTranslateZ, "scalePivotTranslateZ")
BUILTIN(MPxTransform, rotateAxis, "rotateAxis")
BUILTIN(MPxTransform, rotateAxisX, "rotateAxisX")
BUILTIN(MPxTransform, rotateAxisY, "rotateAxisY")
BUILTIN(MPxTransform, rotateAxisZ, "rotateAxisZ")
BUILTIN(MPxTransform, inheritsTransform, "inheritsTransform")
BUILTIN(MPxTransform, displayHandle, "displayHandle")
BUILTIN(MPxTransform, displayRotatePivot, "displayRotatePivot")
BUILTIN(MPxTransform, displayScalePivot, "displayScalePivot")
BUILTIN(MPxTransform, displayLocalAxis, "displayLocalAxis")
BUILTIN(MPxTransform, matrix, "matrix")
BUILTIN(MPxTransform, inverseMatrix, "inverseMatrix")
BUILTIN(MPxTransform, worldMatrix, "worldMatrix")
BUILTIN(MPxTransform, worldInverseMatrix, "worldInverseMatrix")
BUILTIN(MPxTransform, parentMatrix, "parentMatrix")
BUILTIN(MPxTransform, parentInverseMatrix, "parentInverseMatrix")
BUILTIN(MPxTransform, xformMatrix, "xformMatrix")
BUILTIN(MPxTransform, offsetParentMatrix, "offsetParentMatrix")
BUILTIN(MPxTransform, visibility, "visibility")
BUILTIN(MPxConstraint, enableRestPosition, "enableRestPosition")
BUILTIN(MPxConstraint, lockOutput, "lockOutput")
BUILTIN(MPxSurfaceShape, isRenderable, "isRenderable")
BUILTIN(MPxSurfaceShape, mControlPoints, "controlPoints")
BUILTIN(MPxSurfaceShape, mControlValueX, "xValue")
BUILTIN(MPxSurfaceShape, mControlValueY, "yValue")
BUILTIN(MPxSurfaceShape, mControlValueZ, "zValue")
BUILTIN(MPxSurfaceShape, mHasHistoryOnCreate, "hasHistoryOnCreate")
BUILTIN(MPxSurfaceShape, nodeBoundingBox, "nodeBoundingBox")
BUILTIN(MPxSurfaceShape, nodeBoundingBoxMin, "boundingBoxMin")
BUILTIN(MPxSurfaceShape, nodeBoundingBoxMax, "boundingBoxMax")
BUILTIN(MPxSurfaceShape, nodeBoundingBoxSize, "boundingBoxSize")
#undef BUILTIN

MPxTransform::MPxTransform() : baseTransformationMatrix(0) {}
MPxTransform::MPxTransform(MPxTransformationMatrix* tm) : baseTransformationMatrix(tm) {}
MPxTransform::~MPxTransform() { delete baseTransformationMatrix; }
MPxNode::Type MPxTransform::type() const { return kTransformNode; }
void MPxTransform::postConstructor() {}
// The matrix registerTransform named, or the plain one.
MPxTransformationMatrix* MPxTransform::createTransformationMatrix() {
    NodeClass* c = classOfNode(azNodeId_);
    if (c && c->xformCreator) return c->xformCreator();
    if (c && c->xformCreatorOld) return (MPxTransformationMatrix*)c->xformCreatorOld();
    return new MPxTransformationMatrix();
}
bool MPxTransform::isBounded() const { return false; }
MBoundingBox MPxTransform::boundingBox() const { return MBoundingBox(); }
bool MPxTransform::treatAsTransform() const { return true; }
MStatus MPxTransform::compute(const MPlug&, MDataBlock&) { return MS::kUnknownParameter; }
MStatus MPxTransform::validateAndSetValue(const MPlug&, const MDataHandle&) { return MS::kUnknownParameter; }

MStatus MPxTransform::computeLocalTransformation(MPxTransformationMatrix* xform, MDataBlock& block) {
    if (!xform) return MS::kInvalidParameter;
    MEulerRotation::RotationOrder order = (MEulerRotation::RotationOrder)block.inputValue(rotateOrder).asShort();
    xform->translateTo(block.inputValue(translate).asVector());
    xform->setRotationOrder(MPxTransformationMatrix::convertRotationOrder(order), false);
    MVector r = block.inputValue(rotate).asVector();
    xform->rotateTo(MEulerRotation(r.x, r.y, r.z, order));
    xform->scaleTo(block.inputValue(scale).asVector());
    xform->shearTo(block.inputValue(shear).asVector());
    xform->setRotatePivot(MPoint(block.inputValue(rotatePivot).asVector()), MSpace::kTransform, false);
    xform->setRotatePivotTranslation(block.inputValue(rotatePivotTranslate).asVector());
    xform->setScalePivot(MPoint(block.inputValue(scalePivot).asVector()), MSpace::kTransform, false);
    xform->setScalePivotTranslation(block.inputValue(scalePivotTranslate).asVector());
    MVector ra = block.inputValue(rotateAxis).asVector();
    xform->setRotateOrientation(MEulerRotation(ra.x, ra.y, ra.z), MSpace::kTransform, false);
    return MS::kSuccess;
}

MMatrix MPxTransform::getMatrix(const MDGContext&, MStatus* st) {
    MDagPath p = MDagPath::azOf(azNodeId_);
    MStatus s;
    MMatrix m = p.inclusiveMatrix(&s) * p.exclusiveMatrixInverse(&s);
    if (st) *st = s;
    return m;
}
MMatrix MPxTransform::getMatrixInverse(const MDGContext& c, MStatus* st) { return getMatrix(c, st).inverse(); }
void MPxTransform::resetTransformation(const MMatrix& m) {
    if (baseTransformationMatrix) baseTransformationMatrix->decomposeMatrix(m);
}
void MPxTransform::resetTransformation(MPxTransformationMatrix* x) {
    if (baseTransformationMatrix && x) baseTransformationMatrix->copyValues(x);
}
MPxTransformationMatrix* MPxTransform::transformationMatrixPtr() const {
    MPxTransform* self = const_cast<MPxTransform*>(this);
    if (!self->baseTransformationMatrix) self->baseTransformationMatrix = self->createTransformationMatrix();
    return baseTransformationMatrix;
}
const MPxTransformationMatrix& MPxTransform::transformationMatrix() const { return *transformationMatrixPtr(); }

namespace {
struct MatrixCall {
    MPxTransform* node;
    MDataBlock* data;
    MStatus status;
    MMatrix out;
};
int runMatrix(void* p) {
    MatrixCall* c = (MatrixCall*)p;
    MPxTransformationMatrix* tm = c->node->transformationMatrixPtr();
    if (!tm) return AZ_FAILURE;
    c->status = c->node->computeLocalTransformation(tm, *c->data);
    if (c->status == MS::kUnknownParameter) return AZ_UNKNOWN_PARAMETER;
    if (!c->status) return AZ_FAILURE;
    c->out = tm->asMatrix();
    return AZ_OK;
}
}  // namespace

// The host's local_matrix: the transformation matrix, filled from the block, put together.
int nodeLocalMatrix(void* ctx, void* inst, AzBlock* block, double* out, char** error) {
    MPxTransform* t = dynamic_cast<MPxTransform*>((MPxNode*)inst);
    if (!t || !out) return AZ_UNKNOWN_PARAMETER;
    BlockImpl b;
    b.block = block;
    b.cls = (NodeClass*)ctx;
    b.node = host->block_node(block);
    MDataBlock data(&b);
    BlockImpl* was = currentBlock();
    currentBlock() = &b;
    MatrixCall c = {t, &data, MStatus(), MMatrix()};
    std::string e;
    int rc = guarded(runMatrix, &c, e);
    currentBlock() = was;
    if (rc == AZ_OK) {
        std::vector<double> m = flat(c.out);
        for (int i = 0; i < 16; ++i) out[i] = m[i];
    } else if (rc != AZ_UNKNOWN_PARAMETER && error) {
        if (e.empty()) e = c.status.errorString().asChar();
        *error = host->string_new(e.c_str());
    }
    return rc;
}

// ---------------------------------------------------------------------------------------------------------------------
// Constraints

MPxConstraint::MPxConstraint() {}
MPxConstraint::~MPxConstraint() {}
MPxNode::Type MPxConstraint::type() const { return kConstraintNode; }
MStatus MPxConstraint::getOutputAttributes(MObjectArray&) { return MS::kSuccess; }
void MPxConstraint::getOutputAttributes(MObjectArray& a) const { const_cast<MPxConstraint*>(this)->getOutputAttributes(a); }
const MObject MPxConstraint::passiveOutputAttribute() const { return MObject::kNullObj; }
const MObject MPxConstraint::weightAttribute() const { return MObject::kNullObj; }
const MObject MPxConstraint::targetAttribute() const { return MObject::kNullObj; }
MTypeId MPxConstraint::constraintTypeId() const { return typeId(); }
MPxConstraint::TargetType MPxConstraint::targetType() const { return kTargetTypeTransform; }

MPxConstraintCommand::MPxConstraintCommand() {}
MPxConstraintCommand::~MPxConstraintCommand() {}
MStatus MPxConstraintCommand::appendSyntax() { return MS::kSuccess; }
MStatus MPxConstraintCommand::parseArgs(const MArgList&) { return MS::kSuccess; }
MTypeId MPxConstraintCommand::constraintTypeId() const { return MTypeId(); }
const MObject& MPxConstraintCommand::constraintInstancedAttribute() const { return MObject::kNullObj; }
const MObject& MPxConstraintCommand::constraintOutputAttribute() const { return MObject::kNullObj; }
const MObject& MPxConstraintCommand::constraintTargetInstancedAttribute() const { return MObject::kNullObj; }
const MObject& MPxConstraintCommand::constraintTargetAttribute() const { return MObject::kNullObj; }
const MObject& MPxConstraintCommand::constraintTargetWeightAttribute() const { return MObject::kNullObj; }
const MObject& MPxConstraintCommand::objectAttribute() const { return MObject::kNullObj; }
MPxConstraintCommand::TargetType MPxConstraintCommand::targetType() const { return kTransform; }
void MPxConstraintCommand::createdConstraint(MPxConstraint*) {}
MStatus MPxConstraintCommand::setRestPosition(MObject&) { return MS::kSuccess; }
bool MPxConstraintCommand::supportsOffset() const { return false; }
bool MPxConstraintCommand::hasVectorFlags() const { return false; }
bool MPxConstraintCommand::hasUpVectorFlags() const { return false; }
MStatus MPxConstraintCommand::handleNewTargets(MObject&) { return MS::kSuccess; }
MSyntax& MPxConstraintCommand::syntax() { return azSyntax; }
// Not undoable on its own: the commands it runs are, each in the same undo step.
bool MPxConstraintCommand::isUndoable() const { return false; }

static std::string attrName(const MObject& attr) {
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (!d || !d->attr) return std::string();
    return d->attr->builtin.empty() ? d->attr->name : d->attr->builtin;
}

static MStatus connect(const std::string& a, const std::string& b) {
    MStatus st;
    Json args = Json::array();
    args.push(a).push(b);
    command("connectAttr", args, Json::object().set("force", true), &st);
    return st;
}

// An attribute's plug for target `index`: under the array compound it sits in, or its own element.
static std::string elementPlug(const MObject& attr, int index) {
    const std::shared_ptr<ObjectData>& d = attr.azData();
    if (!d || !d->attr) return std::string();
    const AttrSpec* s = d->attr.get();
    if (s->parent && s->parent->array) return s->parent->name + "[" + std::to_string(index) + "]." + s->name;
    if (s->array) return s->name + "[" + std::to_string(index) + "]";
    return attrName(attr);
}

MStatus MPxConstraintCommand::connectTargetAttribute(MDagPath& target, int index, MObject& targetAttr, MObject& constraintAttr, bool targetIsArray) {
    std::string node = nodePath(azConstraint.azData() ? azConstraint.azData()->node : -1);
    std::string src = std::string(target.fullPathName().asChar()) + "." + attrName(targetAttr) + (targetIsArray ? "[0]" : "");
    return connect(src, node + "." + elementPlug(constraintAttr, index));
}

MStatus MPxConstraintCommand::connectObjectAttribute(MDagPath& child, MObject& objectAttr, MObject& constraintAttr, bool objectIsArray) {
    std::string node = nodePath(azConstraint.azData() ? azConstraint.azData()->node : -1);
    std::string obj = std::string(child.fullPathName().asChar()) + "." + attrName(objectAttr) + (objectIsArray ? "[0]" : "");
    return connect(obj, node + "." + attrName(constraintAttr));
}

MStatus MPxConstraintCommand::connectTargetMatrixAttribute(MDagPath& target, int index, MObject& constraintAttr) {
    MObject wm = MPxTransform::worldMatrix;
    return connectTargetAttribute(target, index, wm, constraintAttr, false);
}

MStatus MPxConstraintCommand::connectTarget(MDagPath& target, int index) {
    MObject a = constraintTargetInstancedAttribute(), b = constraintTargetAttribute();
    if (a.isNull() || b.isNull()) return MS::kSuccess;
    return connectTargetAttribute(target, index, a, b, attrName(a) == "worldMatrix" || attrName(a) == "worldMesh");
}

MStatus MPxConstraintCommand::connectObjectAndConstraint(MDGModifier&) { return MS::kSuccess; }

// The default work, as the virtual functions name it: the constraint made under the driven object, the object's
// instanced attribute into the constraint, the constraint's output onto the object's attribute, each target connected
// with its weight at 1.
MStatus MPxConstraintCommand::doIt(const MArgList& args) {
    MStatus st;
    std::vector<std::string> names;
    for (size_t i = 0; i < args.azItems().size(); ++i) {
        const std::string& t = args.azItems()[i].text;
        if (!args.azItems()[i].number && !t.empty() && t[0] != '-') names.push_back(t);
    }
    if (names.empty()) {
        Json sel = command("ls", Json::array(), Json::object().set("selection", true).set("long", true), &st);
        for (size_t i = 0; i < sel.size(); ++i) names.push_back(sel[i].str());
    }
    if (names.size() < 2) return fail(MStatus::kInvalidParameter, "(kInvalidParameter): name the targets, then the object to constrain");
    parseArgs(args);
    std::string object = names.back();
    names.pop_back();
    NodeClass* cls = classOfId(constraintTypeId());
    if (!cls) return fail(MStatus::kFailure, "(kFailure): the constraint's node type isn't registered");
    std::string shortName = object.substr(object.rfind('|') == std::string::npos ? 0 : object.rfind('|') + 1);
    Json made = command("createNode", args1(cls->name), Json::object().set("parent", object).set("name", shortName + "_" + cls->name + "1"), &st);
    if (!st) return st;
    std::string node = made.isArray() ? made[(size_t)0].str() : made.str();
    MSelectionList sl;
    sl.add(MString(node.c_str()));
    sl.getDependNode(0, azConstraint);
    MSelectionList os;
    os.add(MString(object.c_str()));
    MDagPath objectPath;
    os.getDagPath(0, objectPath);
    MObject inst = constraintInstancedAttribute();
    if (!inst.isNull()) {
        MObject pim = MPxTransform::parentInverseMatrix;
        connectObjectAttribute(objectPath, pim, inst, false);
    }
    MObject out = constraintOutputAttribute(), objAttr = objectAttribute();
    if (!out.isNull() && !objAttr.isNull()) connect(node + "." + attrName(out), std::string(objectPath.fullPathName().asChar()) + "." + attrName(objAttr));
    MObject weight = constraintTargetWeightAttribute();
    for (size_t i = 0; i < names.size(); ++i) {
        MSelectionList ts;
        ts.add(MString(names[i].c_str()));
        MDagPath tp;
        if (!ts.getDagPath(0, tp)) continue;
        connectTarget(tp, (int)i);
        if (!weight.isNull()) {
            Json a = args1(node + "." + elementPlug(weight, (int)i));
            a.push(1.0);
            command("setAttr", a, Json::object());
        }
    }
    MDGModifier mod;
    connectObjectAndConstraint(mod);
    mod.doIt();
    handleNewTargets(azConstraint);
    MPxConstraint* c = 0;
    {
        std::lock_guard<std::mutex> g(cls->lock);
        std::map<int64_t, MPxNode*>::iterator it = cls->instances.find(azConstraint.azData() ? azConstraint.azData()->node : -1);
        if (it != cls->instances.end()) c = dynamic_cast<MPxConstraint*>(it->second);
    }
    createdConstraint(c);
    setResult(MString(node.c_str()));
    return MS::kSuccess;
}
MStatus MPxConstraintCommand::undoIt() { return MS::kSuccess; }
MStatus MPxConstraintCommand::redoIt() { return MS::kSuccess; }

// ---------------------------------------------------------------------------------------------------------------------
// Shapes

MSelectionMask::MSelectionMask() : bits_(0) {}
MSelectionMask::MSelectionMask(SelectionType t) : bits_(1ull << t) {}
MStatus MSelectionMask::setMask(SelectionType t) {
    bits_ = 1ull << t;
    return MS::kSuccess;
}
MStatus MSelectionMask::addMask(SelectionType t) {
    bits_ |= 1ull << t;
    return MS::kSuccess;
}
bool MSelectionMask::intersects(SelectionType t, MStatus* st) const {
    ok(st);
    return (bits_ & (1ull << t)) != 0;
}
bool MSelectionMask::intersects(const MSelectionMask& o, MStatus* st) const {
    ok(st);
    return (bits_ & o.bits_) != 0;
}

MPxSurfaceShape::MPxSurfaceShape() {}
MPxSurfaceShape::~MPxSurfaceShape() {}
MPxNode::Type MPxSurfaceShape::type() const { return kSurfaceShape; }
bool MPxSurfaceShape::isBounded() const { return false; }
MBoundingBox MPxSurfaceShape::boundingBox() const { return MBoundingBox(); }
MObject MPxSurfaceShape::geometryData() const { return MObject(); }
MObject MPxSurfaceShape::localShapeInAttr() const { return MObject(); }
MObject MPxSurfaceShape::localShapeOutAttr() const { return MObject(); }
MObject MPxSurfaceShape::worldShapeOutAttr() const { return MObject(); }
MObject MPxSurfaceShape::cachedShapeAttr() const { return MObject(); }
bool MPxSurfaceShape::deleteComponents(const MObjectArray&, MDoubleArray&) { return false; }
bool MPxSurfaceShape::undeleteComponents(const MObjectArray&, MDoubleArray&) { return false; }
void MPxSurfaceShape::componentToPlugs(MObject&, MSelectionList&) const {}
bool MPxSurfaceShape::match(const MSelectionMask&, const MObjectArray&) const { return false; }
MObject MPxSurfaceShape::createFullVertexGroup() const { return MObject(); }
MObject MPxSurfaceShape::createFullRenderGroup() const { return MObject(); }
MSelectionMask MPxSurfaceShape::getShapeSelectionMask() const { return MSelectionMask(MSelectionMask::kSelectObjectsMask); }
MSelectionMask MPxSurfaceShape::getComponentSelectionMask() const { return MSelectionMask(); }
bool MPxSurfaceShape::excludeAsPluginShape() const { return true; }
void MPxSurfaceShape::transformUsing(const MMatrix&, const MObjectArray&, MVertexCachingMode, MPointArray*) {}
void MPxSurfaceShape::tweakUsing(const MMatrix&, const MObjectArray&, MVertexCachingMode, MPointArray*, MArrayDataHandle&) {}
void MPxSurfaceShape::childChanged(MBoundingBoxChangedMode) {}

MPxSurfaceShapeUI::MPxSurfaceShapeUI() {}
MPxSurfaceShapeUI::~MPxSurfaceShapeUI() {}
void MPxSurfaceShapeUI::draw(const MDrawRequest&, M3dView&) const {}
bool MPxSurfaceShapeUI::select(MSelectInfo&, MSelectionList&, MPointArray&) const { return false; }
MPxSurfaceShape* MPxSurfaceShapeUI::surfaceShape() const { return 0; }
MPxSurfaceShapeUI* MPxSurfaceShapeUI::surfaceShapeUI(const MDagPath&, MStatus* st) {
    if (st) *st = MS::kNotImplemented;
    return 0;
}

namespace {
struct BoxCall {
    MPxSurfaceShape* shape;
    bool bounded;
    MBoundingBox box;
};
int runBox(void* p) {
    BoxCall* c = (BoxCall*)p;
    c->bounded = c->shape->isBounded();
    if (c->bounded) c->box = c->shape->boundingBox();
    return AZ_OK;
}
}  // namespace

// The host's bounding_box: what the shape says, while its data block is the one the host gave.
int nodeBoundingBox(void* ctx, void* inst, AzBlock* block, double* out) {
    MPxSurfaceShape* s = dynamic_cast<MPxSurfaceShape*>((MPxNode*)inst);
    if (!s || !out) return 0;
    BlockImpl b;
    b.block = block;
    b.cls = (NodeClass*)ctx;
    b.node = host->block_node(block);
    BlockImpl* was = currentBlock();
    currentBlock() = &b;
    BoxCall c = {s, false, MBoundingBox()};
    std::string e;
    int rc = guarded(runBox, &c, e);
    currentBlock() = was;
    if (rc != AZ_OK || !c.bounded) return 0;
    MPoint lo = c.box.min(), hi = c.box.max();
    out[0] = lo.x;
    out[1] = lo.y;
    out[2] = lo.z;
    out[3] = hi.x;
    out[4] = hi.y;
    out[5] = hi.z;
    return 1;
}

// ---------------------------------------------------------------------------------------------------------------------
// Data

PluginDataBox::~PluginDataBox() {
    if (owned) delete data;
}

std::vector<PluginDataType*>& az::pluginDataTypes() {
    static std::vector<PluginDataType*> all;
    return all;
}
PluginDataType* az::pluginDataTypeOfId(const MTypeId& id) {
    for (size_t i = 0; i < pluginDataTypes().size(); ++i)
        if (pluginDataTypes()[i]->id == id) return pluginDataTypes()[i];
    return 0;
}
PluginDataType* az::pluginDataTypeNamed(const std::string& name) {
    for (size_t i = 0; i < pluginDataTypes().size(); ++i)
        if (pluginDataTypes()[i]->name == name) return pluginDataTypes()[i];
    return 0;
}

MPxData::MPxData() {}
MPxData::~MPxData() {}
MStatus MPxData::readASCII(const MArgList&, unsigned int&) { return MS::kNotImplemented; }
MStatus MPxData::readBinary(std::istream&, unsigned int) { return MS::kNotImplemented; }
MStatus MPxData::writeASCII(std::ostream&) { return MS::kNotImplemented; }
MStatus MPxData::writeBinary(std::ostream&) { return MS::kNotImplemented; }

MFnPluginData::MFnPluginData() {}
MFnPluginData::MFnPluginData(MObject& o, MStatus* st) {
    obj_ = o;
    if (st) *st = (o.azData() && o.azData()->pxData) ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
}
MFn::Type MFnPluginData::type() const { return MFn::kPluginData; }
MTypeId MFnPluginData::typeId(MStatus* st) const {
    const MPxData* d = constData(st);
    return d ? d->typeId() : MTypeId();
}
MPxData* MFnPluginData::data(MStatus* st) {
    const std::shared_ptr<ObjectData>& d = obj_.azData();
    MPxData* p = d && d->pxData ? d->pxData->data : 0;
    if (st) *st = p ? MStatus(MS::kSuccess) : MStatus(MS::kInvalidParameter);
    return p;
}
const MPxData* MFnPluginData::constData(MStatus* st) const { return const_cast<MFnPluginData*>(this)->data(st); }
MObject MFnPluginData::create(const MTypeId& id, MStatus* st) {
    PluginDataType* t = pluginDataTypeOfId(id);
    MPxData* p = t && t->creator ? (MPxData*)t->creator() : 0;
    if (!p) {
        if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): no data type has that id");
        return MObject();
    }
    std::shared_ptr<ObjectData> d = std::make_shared<ObjectData>();
    d->type = MFn::kPluginData;
    d->pxData = std::make_shared<PluginDataBox>();
    d->pxData->data = p;
    d->pxData->typeName = t->name;
    obj_ = MObject(d);
    ok(st);
    return obj_;
}

namespace {
// readASCII's words: spaces apart, a quoted run one word.
MArgList wordsOf(const std::string& text) {
    MArgList out;
    size_t i = 0;
    while (i < text.size()) {
        while (i < text.size() && isspace((unsigned char)text[i])) ++i;
        if (i >= text.size()) break;
        std::string w;
        bool quoted = text[i] == '"';
        if (quoted) {
            ++i;
            while (i < text.size() && text[i] != '"') {
                if (text[i] == '\\' && i + 1 < text.size()) ++i;
                w += text[i++];
            }
            ++i;
        } else {
            while (i < text.size() && !isspace((unsigned char)text[i])) w += text[i++];
        }
        MArgList::Item it;
        it.text = w;
        char* end = 0;
        it.value = strtod(w.c_str(), &end);
        it.number = !quoted && !w.empty() && end && *end == 0;
        out.azAdd(it);
    }
    return out;
}

struct DataCall {
    PluginDataType* t;
    MPxData* d;
    std::string text;
    int which;  // 0 create, 1 destroy, 2 write, 3 read
    int rc;
};
int runData(void* p) {
    DataCall* c = (DataCall*)p;
    switch (c->which) {
    case 0:
        c->d = c->t->creator ? (MPxData*)c->t->creator() : 0;
        return c->d ? AZ_OK : AZ_FAILURE;
    case 1:
        delete c->d;
        return AZ_OK;
    case 2: {
        std::ostringstream os;
        os.precision(17);
        MStatus st = c->d->writeASCII(os);
        c->text = st ? os.str() : std::string();
        return AZ_OK;
    }
    default: {
        MArgList words = wordsOf(c->text);
        unsigned int last = 0;
        return c->d->readASCII(words, last) ? AZ_OK : AZ_FAILURE;
    }
    }
}
void* dataCreate(void* ctx) {
    DataCall c = {(PluginDataType*)ctx, 0, std::string(), 0, 0};
    std::string e;
    return guarded(runData, &c, e) == AZ_OK ? c.d : 0;
}
void dataDestroy(void* ctx, void* d) {
    DataCall c = {(PluginDataType*)ctx, (MPxData*)d, std::string(), 1, 0};
    std::string e;
    guarded(runData, &c, e);
}
char* dataWrite(void* ctx, void* d) {
    DataCall c = {(PluginDataType*)ctx, (MPxData*)d, std::string(), 2, 0};
    std::string e;
    guarded(runData, &c, e);
    return host->string_new(c.text.c_str());
}
int dataRead(void* ctx, void* d, const char* text) {
    DataCall c = {(PluginDataType*)ctx, (MPxData*)d, text ? text : "", 3, 0};
    std::string e;
    return guarded(runData, &c, e);
}
}  // namespace

MStatus MFnPlugin::registerData(const MString& name, const MTypeId& id, MCreatorFunction creator, MPxData::Type type) {
    if (!host || !creator) return fail(MStatus::kInvalidParameter, "registerData: no creator");
    if (host->abi_version < 3) return fail(MStatus::kNotImplemented, "registerData: this Aztek is too old for plug-in data types");
    if (pluginDataTypeOfId(id)) return fail(MStatus::kFailure, std::string("registerData ") + name.asChar() + ": a data type with that id is already registered");
    PluginDataType* t = new PluginDataType();
    t->name = name.asChar();
    t->id = id;
    t->creator = creator;
    t->type = type;
    t->cls.ctx = t;
    t->cls.create = dataCreate;
    t->cls.destroy = dataDestroy;
    t->cls.write_ascii = dataWrite;
    t->cls.read_ascii = dataRead;
    std::string err = take(host->register_data(pluginToken, t->name.c_str(), id.id(), &t->cls));
    if (!err.empty()) {
        delete t;
        return fail(MStatus::kFailure, err);
    }
    pluginDataTypes().push_back(t);
    return MS::kSuccess;
}

MStatus MFnPlugin::deregisterData(const MTypeId& id) {
    PluginDataType* t = pluginDataTypeOfId(id);
    if (!t || !host) return fail(MStatus::kFailure, "deregisterData: no data type has that id");
    std::string err = take(host->deregister_data(pluginToken, t->name.c_str()));
    if (!err.empty()) return fail(MStatus::kFailure, err);
    // Values made can outlive this: the type stays until the plug-in goes.
    return MS::kSuccess;
}

MStatus MFnPlugin::registerShape(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init, MCreatorFunction, const MString* classification) {
    return registerNode(name, id, creator, init, MPxNode::kSurfaceShape, classification);
}
MStatus MFnPlugin::registerShape(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init, const MString* classification) {
    return registerNode(name, id, creator, init, MPxNode::kSurfaceShape, classification);
}

static MStatus transformRegistered(const MString& name, const MTypeId& id, MCreateXformMatrixFunction make, MCreatorFunction makeOld, const MTypeId& xformId,
                                   MStatus st) {
    if (!st) return st;
    NodeClass* c = classOfId(id);
    if (!c) return fail(MStatus::kFailure, std::string("registerTransform ") + name.asChar() + ": the node type didn't register");
    c->xformCreator = make;
    c->xformCreatorOld = makeOld;
    c->xformId = xformId;
    c->hasXform = true;
    return st;
}
MStatus MFnPlugin::registerTransform(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init, MCreateXformMatrixFunction xformCreator,
                                     const MTypeId& xformId, const MString* classification) {
    return transformRegistered(name, id, xformCreator, 0, xformId, registerNode(name, id, creator, init, MPxNode::kTransformNode, classification));
}
MStatus MFnPlugin::registerTransform(const MString& name, const MTypeId& id, MCreatorFunction creator, MInitializeFunction init, MCreatorFunction xformCreator,
                                     const MTypeId& xformId, const MString* classification) {
    return transformRegistered(name, id, 0, xformCreator, xformId, registerNode(name, id, creator, init, MPxNode::kTransformNode, classification));
}
MTypeId MFnPlugin::matrixTypeIdFromXformId(const MTypeId& xformTypeId, MStatus* st) {
    NodeClass* c = classOfId(xformTypeId);
    if (!c || !c->hasXform) {
        if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): no transform of that id was registered with registerTransform");
        return MPxTransformationMatrix::baseTransformationMatrixId;
    }
    ok(st);
    return c->xformId;
}

MStatus MFnPlugin::registerIkSolver(const MString& name, MCreatorFunction) {
    return fail(MStatus::kFailure, std::string("registerIkSolver ") + name.asChar() + ": obsolete in the reference; register the solver as a node with MPxNode::kIkSolverNode");
}
MStatus MFnPlugin::deregisterIkSolver(const MString& name) {
    return fail(MStatus::kFailure, std::string("deregisterIkSolver ") + name.asChar() + ": obsolete in the reference; deregister the solver node");
}

// ---------------------------------------------------------------------------------------------------------------------
// IK solvers

MIkHandleGroup::MIkHandleGroup() : azPriority(1), azSolver(0) {}
MIkHandleGroup::~MIkHandleGroup() {}
int MIkHandleGroup::priority(MStatus* st) const {
    ok(st);
    return azPriority;
}
int MIkHandleGroup::solverID(MStatus* st) const {
    ok(st);
    return azSolver;
}
bool MIkHandleGroup::solverPriority(int* p, MStatus* st) const {
    ok(st);
    if (p) *p = azPriority;
    return true;
}
MStatus MIkHandleGroup::setPriority(int p) {
    azPriority = p;
    return MS::kSuccess;
}
MStatus MIkHandleGroup::setSolverID(int id) {
    azSolver = id;
    return MS::kSuccess;
}
MStatus MIkHandleGroup::checkEffectorAtGoal() { return MS::kSuccess; }
bool MIkHandleGroup::isUnderControl(MStatus* st) const {
    ok(st);
    return true;
}
unsigned int MIkHandleGroup::handleCount() const { return (unsigned int)azHandles.size(); }
MObject MIkHandleGroup::handle(unsigned int i) { return i < azHandles.size() ? nodeObject(azHandles[i]) : MObject(); }
unsigned int MIkHandleGroup::dofCount() const { return 0; }
unsigned int MIkHandleGroup::solve(MStatus* st) {
    if (st) *st = MS::kNotImplemented;
    return 0;
}

MPxIkSolverNode::MPxIkSolverNode()
    : group_(0), tolerance_(1e-5), iterations_(20), rotatePlane_(false), singleChain_(false), positionOnly_(false), limits_(false), unique_(false) {}
MPxIkSolverNode::~MPxIkSolverNode() {}
MPxNode::Type MPxIkSolverNode::type() const { return kIkSolverNode; }
MStatus MPxIkSolverNode::preSolve() { return MS::kSuccess; }
MStatus MPxIkSolverNode::doSolve() { return MS::kSuccess; }
MStatus MPxIkSolverNode::postSolve(MStatus s) { return s; }
MString MPxIkSolverNode::solverTypeName() const { return typeName(); }
bool MPxIkSolverNode::rotatePlane(MStatus* st) const {
    ok(st);
    return rotatePlane_;
}
MStatus MPxIkSolverNode::setRotatePlane(bool b) {
    rotatePlane_ = b;
    return MS::kSuccess;
}
bool MPxIkSolverNode::singleChainOnly(MStatus* st) const {
    ok(st);
    return singleChain_;
}
MStatus MPxIkSolverNode::setSingleChainOnly(bool b) {
    singleChain_ = b;
    return MS::kSuccess;
}
bool MPxIkSolverNode::positionOnly(MStatus* st) const {
    ok(st);
    return positionOnly_;
}
MStatus MPxIkSolverNode::setPositionOnly(bool b) {
    positionOnly_ = b;
    return MS::kSuccess;
}
bool MPxIkSolverNode::supportJointLimits(MStatus* st) const {
    ok(st);
    return limits_;
}
MStatus MPxIkSolverNode::setSupportJointLimits(bool b) {
    limits_ = b;
    return MS::kSuccess;
}
bool MPxIkSolverNode::uniqueSolution(MStatus* st) const {
    ok(st);
    return unique_;
}
MStatus MPxIkSolverNode::setUniqueSolution(bool b) {
    unique_ = b;
    return MS::kSuccess;
}
bool MPxIkSolverNode::isSingleChainOnly() const { return singleChain_; }
bool MPxIkSolverNode::isPositionOnly() const { return positionOnly_; }
bool MPxIkSolverNode::hasJointLimitSupport() const { return limits_; }
bool MPxIkSolverNode::hasUniqueSolution() const { return unique_; }
bool MPxIkSolverNode::groupHandlesByTopology() const { return true; }
MStatus MPxIkSolverNode::setFuncValueTolerance(double t) {
    tolerance_ = t;
    return MS::kSuccess;
}
MStatus MPxIkSolverNode::setMaxIterations(int n) {
    iterations_ = n;
    return MS::kSuccess;
}
MIkHandleGroup* MPxIkSolverNode::handleGroup() const { return group_; }
void MPxIkSolverNode::setHandleGroup(MIkHandleGroup* g) { group_ = g; }
const MMatrix* MPxIkSolverNode::toWorldSpace() const { return &toWorld_; }
const MMatrix* MPxIkSolverNode::toSolverSpace() const { return &toSolver_; }
double MPxIkSolverNode::funcValueTolerance() const { return tolerance_; }
int MPxIkSolverNode::maxIterations() const { return iterations_; }
void MPxIkSolverNode::snapHandle(MObject&) {}
bool MPxIkSolverNode::isAttributeCreatedBySolver(MObject&) const { return false; }

MFnIkHandle::MFnIkHandle() {}
MFnIkHandle::MFnIkHandle(const MObject& o, MStatus* st) : MFnTransform(o, st) {}
MFnIkHandle::MFnIkHandle(const MDagPath& p, MStatus* st) : MFnTransform(p, st) {}
MFn::Type MFnIkHandle::type() const { return MFn::kIkHandle; }
MFnIkEffector::MFnIkEffector() {}
MFnIkEffector::MFnIkEffector(const MObject& o, MStatus* st) : MFnTransform(o, st) {}
MFnIkEffector::MFnIkEffector(const MDagPath& p, MStatus* st) : MFnTransform(p, st) {}
MFn::Type MFnIkEffector::type() const { return MFn::kIkEffector; }

static MStatus pathFromCommand(const MDagPath& handle, const char* flag, MDagPath& out) {
    MStatus st;
    Json r = command("ikHandle", args1(std::string(handle.fullPathName().asChar())), Json::object().set("query", true).set(flag, true), &st);
    if (!st) return st;
    std::string name = r.isArray() ? r[(size_t)0].str() : r.str();
    MSelectionList sl;
    sl.add(MString(name.c_str()));
    return sl.getDagPath(0, out);
}
MStatus MFnIkHandle::getStartJoint(MDagPath& p) const { return pathFromCommand(path_, "startJoint", p); }
MStatus MFnIkHandle::getEffector(MDagPath& p) const { return pathFromCommand(path_, "endEffector", p); }
MObject MFnIkHandle::solver(MStatus* st) const {
    Json r = command("ikHandle", args1(std::string(path_.fullPathName().asChar())), Json::object().set("query", true).set("solver", true), st);
    MSelectionList sl;
    sl.add(MString((r.isArray() ? r[(size_t)0].str() : r.str()).c_str()));
    MObject o;
    sl.getDependNode(0, o);
    return o;
}
static double handleNumber(const MDagPath& p, const char* attr, MStatus* st) {
    std::vector<double> v = numbers(command("getAttr", args1(std::string(p.fullPathName().asChar()) + "." + attr), Json::object(), st));
    return v.empty() ? 0.0 : v[0];
}
double MFnIkHandle::priority(MStatus* st) const { return handleNumber(path_, "priority", st); }
double MFnIkHandle::weight(MStatus* st) const { return handleNumber(path_, "weight", st); }
double MFnIkHandle::poWeight(MStatus* st) const { return handleNumber(path_, "poWeight", st); }

// While a solver solves: the handle, its effector and its chain, as the host sent them, answering the queries and
// commands the kit's classes make of them.
namespace {
const double kDeg = 57.29577951308232;

struct IkNode {
    int64_t id = -1, parent = 0;
    std::string type, path, full, name;
    MVector t, jo, ra, s = MVector(1, 1, 1), is = MVector(1, 1, 1), r, pa;
    int ro = 0;
};

struct IkScene {
    std::vector<int64_t> chain;
    int64_t end = -1, handle = -1, effector = -1;
    MMatrix parentWorld, handleWorld;
    MVector pole;
    double twist = 0, roll = 0;
    std::map<int64_t, IkNode> nodes;

    IkNode* named(const std::string& n) {
        for (std::map<int64_t, IkNode>::iterator it = nodes.begin(); it != nodes.end(); ++it) {
            const IkNode& x = it->second;
            if (x.path == n || x.full == n || x.name == n || ("|" + x.path) == n) return &it->second;
        }
        return 0;
    }
    MMatrix local(const IkNode& n) const {
        if (n.type == "ikHandle") return handleWorld * parentOf(n).inverse();
        if (n.type != "joint") return tmat(n.t);
        MEulerRotation r(n.r.x, n.r.y, n.r.z, (MEulerRotation::RotationOrder)n.ro);
        return scaling(n.s) * MEulerRotation(n.ra.x, n.ra.y, n.ra.z).asMatrix() * r.asMatrix() * MEulerRotation(n.jo.x, n.jo.y, n.jo.z).asMatrix() * scaling(n.is) *
               tmat(n.t);
    }
    MMatrix parentOf(const IkNode& n) const {
        std::map<int64_t, IkNode>::const_iterator p = nodes.find(n.parent);
        return p == nodes.end() ? (n.type == "ikHandle" ? MMatrix() : parentWorld) : world(p->second);
    }
    MMatrix world(const IkNode& n) const {
        if (n.type == "ikHandle") return handleWorld;
        return local(n) * parentOf(n);
    }
};

thread_local IkScene* solving = 0;

IkNode nodeOfJson(const Json& j, const char* type) {
    IkNode n;
    n.id = j["id"].integer(-1);
    n.parent = j["parent"].integer(0);
    n.path = j["path"].str();
    n.full = j["full"].str();
    n.name = j["name"].str();
    n.type = type;
    return n;
}

Json infoOf(const IkNode& n) {
    Json chain = Json::array();
    chain.push(n.type);
    chain.push("transform");
    chain.push("dagNode");
    Json o = Json::object();
    o.set("id", Json(n.id)).set("name", n.name).set("type", n.type).set("chain", chain).set("dag", true).set("owner", "");
    o.set("path", n.path).set("full", n.full).set("parent", Json(n.parent)).set("children", Json::array()).set("intermediate", false);
    return o;
}

// An attribute of the scene's nodes: its value as the reference's getAttr gives it (degrees), and how to set it.
bool attrValue(IkScene& sc, IkNode& n, const std::string& attr, Json& out) {
    MVector v;
    bool angle = false;
    std::string base = attr;
    int comp = -1;
    static const char* vecs[][2] = {{"translate", "t"}, {"rotate", "r"}, {"scale", "s"}, {"jointOrient", "jo"}, {"rotateAxis", "ra"}, {"preferredAngle", "pa"}, {"poleVector", "pv"}};
    for (size_t k = 0; k < sizeof vecs / sizeof vecs[0]; ++k) {
        std::string l = vecs[k][0], s = vecs[k][1];
        if (attr == l || attr == s) base = l;
        for (int c = 0; c < 3; ++c) {
            std::string axis(1, (char)('X' + c)), lower(1, (char)('x' + c));
            if (attr == l + axis || attr == s + lower) {
                base = l;
                comp = c;
            }
        }
    }
    if (base == "translate") v = n.t;
    else if (base == "rotate") v = n.r, angle = true;
    else if (base == "scale") v = n.s;
    else if (base == "jointOrient") v = n.jo, angle = true;
    else if (base == "rotateAxis") v = n.ra, angle = true;
    else if (base == "preferredAngle") v = n.pa, angle = true;
    else if (base == "poleVector" && n.type == "ikHandle") v = sc.pole;
    else if ((attr == "rotateOrder" || attr == "ro")) {
        out = Json((int64_t)n.ro);
        return true;
    } else if (n.type == "ikHandle" && (attr == "twist" || attr == "twi")) {
        out = Json(sc.twist * kDeg);
        return true;
    } else if (n.type == "ikHandle" && (attr == "roll" || attr == "rol")) {
        out = Json(sc.roll * kDeg);
        return true;
    } else
        return false;
    if (angle) v = v * kDeg;
    if (comp >= 0) {
        out = Json(v[comp]);
    } else {
        out = Json::array();
        out.push(v.x).push(v.y).push(v.z);
    }
    return true;
}

bool split(const std::string& plug, std::string& node, std::string& attr) {
    size_t dot = plug.find('.');
    if (dot == std::string::npos) return false;
    node = plug.substr(0, dot);
    attr = plug.substr(dot + 1);
    return true;
}

bool standIn(bool isQuery, const char* name, const Json& args, const Json& flags, Json& out, MStatus* st) {
    IkScene* sc = solving;
    if (!sc) return false;
    std::string op = name;
    ok(st);
    if (isQuery && (op == "node.info" || op == "dag.matrices")) {
        std::map<int64_t, IkNode>::iterator it = sc->nodes.find(args["id"].integer(-1));
        if (it == sc->nodes.end()) {
            if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): only the handle and its chain answer while a solver solves");
            out = Json();
            return true;
        }
        if (op == "node.info") {
            out = infoOf(it->second);
        } else {
            out = Json::object();
            out.set("world", flatJson(sc->world(it->second))).set("parent", flatJson(sc->parentOf(it->second))).set("local", flatJson(sc->local(it->second)));
        }
        return true;
    }
    if (isQuery && op == "node.find") {
        IkNode* n = sc->named(args["name"].str());
        out = n ? Json(n->id) : Json();
        return true;
    }
    if (isQuery && op == "node.exists") {
        out = Json(sc->nodes.count(args["id"].integer(-1)) > 0);
        return true;
    }
    if (!isQuery && op == "ls" && args.size() == 1 && !flags.has("selection")) {
        IkNode* n = sc->named(args[(size_t)0].str());
        out = Json::array();
        if (n) out.push(n->full);
        return true;
    }
    if (isQuery && op == "plug.info") {
        std::string node, attr;
        IkNode* n = split(args["name"].str(), node, attr) ? sc->named(node) : 0;
        Json v;
        if (!n || !attrValue(*sc, *n, attr, v)) {
            if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): no such plug while a solver solves");
            out = Json();
            return true;
        }
        out = Json::object();
        out.set("long", attr).set("short", attr).set("type", v.isArray() ? "vec3" : "float").set("node", Json(n->id)).set("component", Json((int64_t)-1));
        out.set("element", Json((int64_t)-1)).set("name", args["name"].str()).set("locked", false).set("children", Json::array());
        return true;
    }
    if (isQuery) return false;
    if (op == "getAttr") {
        std::string node, attr;
        IkNode* n = split(args[(size_t)0].str(), node, attr) ? sc->named(node) : 0;
        if (!n || !attrValue(*sc, *n, attr, out)) {
            if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): only the handle's and chain's channels answer while a solver solves");
            out = Json();
        }
        return true;
    }
    if (op == "setAttr") {
        std::string node, attr;
        IkNode* n = split(args[(size_t)0].str(), node, attr) ? sc->named(node) : 0;
        std::vector<double> v;
        for (size_t i = 1; i < args.size(); ++i) v.push_back(args[i].num());
        bool rot = attr == "rotate" || attr == "r";
        int comp = attr == "rotateX" || attr == "rx" ? 0 : attr == "rotateY" || attr == "ry" ? 1 : attr == "rotateZ" || attr == "rz" ? 2 : -1;
        if (!n || n->type != "joint" || (!rot && comp < 0) || v.empty()) {
            if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): a solver sets only its joints' rotations");
            return true;
        }
        if (rot && v.size() >= 3) n->r = MVector(v[0], v[1], v[2]) / kDeg;
        if (comp >= 0) n->r[comp] = v[0] / kDeg;
        out = Json();
        return true;
    }
    if (op == "xform") {
        IkNode* n = sc->named(args[(size_t)0].str());
        if (!n || !flags["query"].truth()) {
            if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): a solver can only ask where its handle and joints are");
            return true;
        }
        bool world = flags["worldSpace"].truth();
        MMatrix m = world ? sc->world(*n) : sc->local(*n);
        out = Json::array();
        out.push(m[3][0]).push(m[3][1]).push(m[3][2]);
        return true;
    }
    if (op == "ikHandle" && flags["query"].truth()) {
        if (flags["startJoint"].truth() && !sc->chain.empty()) out = Json(sc->nodes[sc->chain[0]].full);
        else if (flags["endEffector"].truth() && sc->effector > 0) out = Json(sc->nodes[sc->effector].full);
        else if (st) *st = fail(MStatus::kInvalidParameter, "(kInvalidParameter): not while a solver solves");
        return true;
    }
    if (st) *st = fail(MStatus::kFailure, std::string("(kFailure): ") + op + " can't run while a solver solves");
    out = Json();
    return true;
}

struct SolveCall {
    MPxIkSolverNode* solver;
    MStatus status;
};
int runSolve(void* p) {
    SolveCall* c = (SolveCall*)p;
    MStatus st = c->solver->preSolve();
    if (st) st = c->solver->doSolve();
    c->status = c->solver->postSolve(st);
    return c->status ? AZ_OK : AZ_FAILURE;
}
}  // namespace

// The host's ik_solve: the scene above, the solver's preSolve, doSolve and postSolve, the joints' rotates back.
int nodeIkSolve(void*, void* inst, const char* input, char** output, char** error) {
    MPxIkSolverNode* solver = dynamic_cast<MPxIkSolverNode*>((MPxNode*)inst);
    if (!solver) return AZ_UNKNOWN_PARAMETER;
    Json in = Json::parse(input ? input : "{}");
    const Json& nodes = in["nodes"];
    const Json& joints = in["joints"];
    IkScene sc;
    for (size_t i = 0; i < nodes["joints"].size(); ++i) {
        IkNode n = nodeOfJson(nodes["joints"][i], "joint");
        const Json& j = joints[i];
        n.t = vecOf(j["t"]);
        n.jo = vecOf(j["jo"]);
        n.ra = vecOf(j["ra"]);
        n.s = vecOf(j["s"]);
        n.is = vecOf(j["is"]);
        n.pa = vecOf(j["pa"]);
        n.r = n.pa;
        n.ro = (int)j["ro"].integer(0);
        sc.chain.push_back(n.id);
        sc.nodes[n.id] = n;
    }
    IkNode end = nodeOfJson(nodes["end"], "joint");
    end.t = vecOf(in["end"]);
    sc.end = end.id;
    sc.nodes[end.id] = end;
    if (nodes["effector"].isObject()) {
        IkNode eff = nodeOfJson(nodes["effector"], "ikEffector");
        eff.t = end.t;
        sc.effector = eff.id;
        sc.nodes[eff.id] = eff;
    }
    IkNode h = nodeOfJson(nodes["handle"], "ikHandle");
    sc.handle = h.id;
    sc.nodes[h.id] = h;
    sc.parentWorld = fromFlat(in["parentMatrix"]);
    sc.handleWorld = fromFlat(in["handleMatrix"]);
    sc.pole = vecOf(in["poleVector"]);
    sc.twist = in["twist"].num();
    sc.roll = in["roll"].num();

    MIkHandleGroup group;
    group.azHandles.push_back(sc.handle);
    solver->setHandleGroup(&group);
    IkScene* was = solving;
    solving = &sc;
    SceneStandIn before = sceneStandIn();
    sceneStandIn() = standIn;
    SolveCall c = {solver, MStatus()};
    std::string e;
    int rc = guarded(runSolve, &c, e);
    sceneStandIn() = before;
    solving = was;
    solver->setHandleGroup(0);
    if (rc != AZ_OK) {
        if (e.empty()) e = c.status.errorString().asChar();
        if (error) *error = host->string_new(e.c_str());
        return rc == AZ_CRASHED ? AZ_CRASHED : AZ_FAILURE;
    }
    Json rotates = Json::array();
    for (size_t i = 0; i < sc.chain.size(); ++i) {
        const MVector& r = sc.nodes[sc.chain[i]].r;
        Json v = Json::array();
        v.push(r.x).push(r.y).push(r.z);
        rotates.push(v);
    }
    Json out = Json::object();
    out.set("rotate", rotates);
    if (output) *output = host->string_new(out.dump().c_str());
    return AZ_OK;
}
