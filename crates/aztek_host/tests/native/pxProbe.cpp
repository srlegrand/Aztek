// Node types of each kind a plug-in can add: a data type and two nodes passing it, a transform that rocks, a shape with
// its own bounds, a constraint that averages its targets with its command, and an IK solver that aims the chain.
#include <maya/MDagPath.h>
#include <maya/MFnCompoundAttribute.h>
#include <maya/MFnDependencyNode.h>
#include <maya/MFnIkEffector.h>
#include <maya/MFnIkHandle.h>
#include <maya/MFnIkJoint.h>
#include <maya/MFnNumericAttribute.h>
#include <maya/MFnPlugin.h>
#include <maya/MFnPluginData.h>
#include <maya/MFnTypedAttribute.h>
#include <maya/MIkHandleGroup.h>
#include <maya/MPxConstraint.h>
#include <maya/MPxConstraintCommand.h>
#include <maya/MPxData.h>
#include <maya/MPxIkSolverNode.h>
#include <maya/MPxNode.h>
#include <maya/MPxSurfaceShape.h>
#include <maya/MPxTransform.h>
#include <maya/MPxTransformationMatrix.h>
#include <maya/MQuaternion.h>
#include <cmath>
#include <vector>

// ---- data

class Points : public MPxData {
public:
    static const MTypeId id;
    static void* creator() { return new Points; }
    void copy(const MPxData& o) override { values = static_cast<const Points&>(o).values; }
    MTypeId typeId() const override { return id; }
    MString name() const override { return "azPoints"; }
    MStatus writeASCII(std::ostream& out) override {
        out << values.size();
        for (double v : values) out << " " << v;
        return MS::kSuccess;
    }
    MStatus readASCII(const MArgList& args, unsigned int& last) override {
        if (args.length() == 0) return MS::kFailure;
        unsigned int n = args.asInt(0);
        values.clear();
        for (unsigned int i = 0; i < n && i + 1 < args.length(); ++i) values.push_back(args.asDouble(i + 1));
        last = n;
        return MS::kSuccess;
    }
    std::vector<double> values;
};
const MTypeId Points::id(0x7f101);

class MakePoints : public MPxNode {
public:
    static const MTypeId id;
    static MObject count, points;
    static void* creator() { return new MakePoints; }
    static MStatus init() {
        MFnNumericAttribute n;
        count = n.create("count", "cnt", MFnNumericData::kInt, 0);
        MFnTypedAttribute t;
        points = t.create("points", "pts", Points::id);
        t.setWritable(false);
        addAttribute(count);
        addAttribute(points);
        attributeAffects(count, points);
        return MS::kSuccess;
    }
    MStatus compute(const MPlug& plug, MDataBlock& data) override {
        if (plug != points) return MS::kUnknownParameter;
        int c = data.inputValue(count).asInt();
        MFnPluginData fn;
        MObject o = fn.create(Points::id);
        Points* p = static_cast<Points*>(fn.data());
        for (int i = 0; i < c; ++i) p->values.push_back(i * 0.5);
        data.outputValue(points).set(o);
        return MS::kSuccess;
    }
};
const MTypeId MakePoints::id(0x7f102);
MObject MakePoints::count, MakePoints::points;

class SumPoints : public MPxNode {
public:
    static const MTypeId id;
    static MObject points, sum;
    static void* creator() { return new SumPoints; }
    static MStatus init() {
        MFnTypedAttribute t;
        points = t.create("points", "pts", Points::id);
        MFnNumericAttribute n;
        sum = n.create("sum", "sum", MFnNumericData::kDouble, 0);
        n.setWritable(false);
        addAttribute(points);
        addAttribute(sum);
        attributeAffects(points, sum);
        return MS::kSuccess;
    }
    MStatus compute(const MPlug& plug, MDataBlock& data) override {
        if (plug != sum) return MS::kUnknownParameter;
        const Points* p = static_cast<const Points*>(data.inputValue(points).asPluginData());
        double s = 0;
        if (p)
            for (double v : p->values) s += v;
        data.outputValue(sum).set(s);
        return MS::kSuccess;
    }
};
const MTypeId SumPoints::id(0x7f103);
MObject SumPoints::points, SumPoints::sum;

// ---- transform

class RockMatrix : public MPxTransformationMatrix {
public:
    static const MTypeId id;
    static MPxTransformationMatrix* creator() { return new RockMatrix; }
    double rock = 0;
    MMatrix asMatrix() const override { return MEulerRotation(rock, 0, 0).asMatrix() * MPxTransformationMatrix::asMatrix(); }
};
const MTypeId RockMatrix::id(0x7f104);

class Rock : public MPxTransform {
public:
    static const MTypeId id;
    static MObject rockX;
    static void* creator() { return new Rock; }
    static MStatus init() {
        MFnNumericAttribute n;
        rockX = n.create("rockX", "rx2", MFnNumericData::kDouble, 0);
        n.setKeyable(true);
        addAttribute(rockX);
        attributeAffects(rockX, MPxTransform::matrix);
        return MS::kSuccess;
    }
    MPxTransformationMatrix* createTransformationMatrix() override { return new RockMatrix; }
    MStatus computeLocalTransformation(MPxTransformationMatrix* xform, MDataBlock& block) override {
        static_cast<RockMatrix*>(xform)->rock = block.inputValue(rockX).asDouble();
        return MPxTransform::computeLocalTransformation(xform, block);
    }
};
const MTypeId Rock::id(0x7f105);
MObject Rock::rockX;

// ---- shape

class BoxShape : public MPxSurfaceShape {
public:
    static const MTypeId id;
    static MObject size;
    static void* creator() { return new BoxShape; }
    static MStatus init() {
        MFnNumericAttribute n;
        size = n.create("size", "sz", MFnNumericData::kDouble, 1);
        addAttribute(size);
        return MS::kSuccess;
    }
    bool isBounded() const override { return true; }
    MBoundingBox boundingBox() const override {
        double s = MPlug(thisMObject(), size).asDouble() * 0.5;
        return MBoundingBox(MPoint(-s, -s, -s), MPoint(s, s, s));
    }
};
const MTypeId BoxShape::id(0x7f106);
MObject BoxShape::size;

// ---- constraint

class AvgConstraint : public MPxConstraint {
public:
    static const MTypeId id;
    static MObject target, targetTranslate, targetWeight, constraintTranslate;
    static void* creator() { return new AvgConstraint; }
    static MStatus init() {
        MFnNumericAttribute n;
        targetTranslate = n.create("targetTranslate", "tt", MFnNumericData::k3Double);
        targetWeight = n.create("targetWeight", "tw", MFnNumericData::kDouble, 1);
        MFnCompoundAttribute c;
        target = c.create("target", "tg");
        c.addChild(targetTranslate);
        c.addChild(targetWeight);
        c.setArray(true);
        constraintTranslate = n.create("constraintTranslate", "ct", MFnNumericData::k3Double);
        n.setWritable(false);
        addAttribute(target);
        addAttribute(constraintTranslate);
        attributeAffects(target, constraintTranslate);
        return MS::kSuccess;
    }
    MStatus compute(const MPlug& plug, MDataBlock& data) override {
        if (plug != constraintTranslate && plug.parent() != constraintTranslate) return MS::kUnknownParameter;
        MArrayDataHandle a = data.inputArrayValue(target);
        MVector sum;
        double w = 0;
        for (unsigned int i = 0; i < a.elementCount(); ++i, a.next()) {
            MDataHandle e = a.inputValue();
            double k = e.child(targetWeight).asDouble();
            sum += e.child(targetTranslate).asVector() * k;
            w += k;
        }
        data.outputValue(constraintTranslate).set(w > 0 ? sum / w : MVector());
        return MS::kSuccess;
    }
    const MObject weightAttribute() const override { return targetWeight; }
    const MObject targetAttribute() const override { return target; }
};
const MTypeId AvgConstraint::id(0x7f107);
MObject AvgConstraint::target, AvgConstraint::targetTranslate, AvgConstraint::targetWeight, AvgConstraint::constraintTranslate;

class AvgCommand : public MPxConstraintCommand {
public:
    static void* creator() { return new AvgCommand; }
    MTypeId constraintTypeId() const override { return AvgConstraint::id; }
    const MObject& constraintInstancedAttribute() const override { return MObject::kNullObj; }
    const MObject& constraintOutputAttribute() const override { return AvgConstraint::constraintTranslate; }
    const MObject& constraintTargetInstancedAttribute() const override { return MPxTransform::translate; }
    const MObject& constraintTargetAttribute() const override { return AvgConstraint::targetTranslate; }
    const MObject& constraintTargetWeightAttribute() const override { return AvgConstraint::targetWeight; }
    const MObject& objectAttribute() const override { return MPxTransform::translate; }
};

// ---- IK solver: turns the start joint so the chain's end points at the handle.

class AimSolver : public MPxIkSolverNode {
public:
    static const MTypeId id;
    static void* creator() { return new AimSolver; }
    static MStatus init() { return MS::kSuccess; }
    MString solverTypeName() const override { return "azAimSolver"; }
    MStatus doSolve() override {
        MIkHandleGroup* g = handleGroup();
        if (!g || g->handleCount() == 0) return MS::kFailure;
        MObject h = g->handle(0);
        MDagPath hp;
        MDagPath::getAPathTo(h, hp);
        MFnIkHandle handle(hp);
        MDagPath start, eff;
        handle.getStartJoint(start);
        handle.getEffector(eff);
        MFnIkJoint joint(start);
        MPoint goal = handle.rotatePivot(MSpace::kWorld);
        MPoint root = joint.rotatePivot(MSpace::kWorld);
        MPoint end = MFnIkEffector(eff).rotatePivot(MSpace::kWorld);
        MQuaternion q(end - root, goal - root);
        return joint.rotateBy(q, MSpace::kWorld);
    }
};
const MTypeId AimSolver::id(0x7f108);

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek tests", "1.0", "Any");
    MStatus st = plugin.registerData("azPoints", Points::id, Points::creator);
    if (st) st = plugin.registerNode("azMakePoints", MakePoints::id, MakePoints::creator, MakePoints::init);
    if (st) st = plugin.registerNode("azSumPoints", SumPoints::id, SumPoints::creator, SumPoints::init);
    if (st) st = plugin.registerNode("azRockTransform", Rock::id, Rock::creator, Rock::init, MPxNode::kTransformNode);
    if (st) st = plugin.registerShape("azBoxShape", BoxShape::id, BoxShape::creator, BoxShape::init, 0);
    if (st) st = plugin.registerNode("azAvgConstraint", AvgConstraint::id, AvgConstraint::creator, AvgConstraint::init, MPxNode::kConstraintNode);
    if (st) st = plugin.registerConstraintCommand("azAvgConstraint", AvgCommand::creator);
    if (st) st = plugin.registerNode("azAimSolver", AimSolver::id, AimSolver::creator, AimSolver::init, MPxNode::kIkSolverNode);
    return st;
}

MStatus uninitializePlugin(MObject obj) {
    MFnPlugin plugin(obj);
    plugin.deregisterNode(AimSolver::id);
    plugin.deregisterConstraintCommand("azAvgConstraint");
    plugin.deregisterNode(AvgConstraint::id);
    plugin.deregisterNode(BoxShape::id);
    plugin.deregisterNode(Rock::id);
    plugin.deregisterNode(SumPoints::id);
    plugin.deregisterNode(MakePoints::id);
    return plugin.deregisterData(Points::id);
}
