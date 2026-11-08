// Sample dependency node: azScale multiplies a number and a vector by a factor; mode 1 adds instead.
//   loadPlugin scaleNode; createNode azScale; setAttr azScale1.input 3; getAttr azScale1.output;
#include <maya/MDataBlock.h>
#include <maya/MDataHandle.h>
#include <maya/MFnEnumAttribute.h>
#include <maya/MFnNumericAttribute.h>
#include <maya/MFnPlugin.h>
#include <maya/MPlug.h>
#include <maya/MPxNode.h>
#include <maya/MTypeId.h>

class ScaleNode : public MPxNode {
public:
    static void* creator() { return new ScaleNode; }

    static MStatus initialize() {
        MFnNumericAttribute n;
        aInput = n.create("input", "in", MFnNumericData::kDouble, 1.0);
        n.setKeyable(true);
        aFactor = n.create("factor", "fa", MFnNumericData::kDouble, 2.0);
        n.setKeyable(true);
        n.setMin(-100.0);
        n.setMax(100.0);
        aVector = n.createPoint("vector", "vec");
        n.setKeyable(true);
        aOutput = n.create("output", "out", MFnNumericData::kDouble, 0.0);
        n.setWritable(false);
        n.setStorable(false);
        aVectorOut = n.createPoint("vectorOut", "vo");
        n.setWritable(false);
        n.setStorable(false);
        MFnEnumAttribute e;
        aMode = e.create("mode", "md", 0);
        e.addField("multiply", 0);
        e.addField("add", 1);
        e.setKeyable(true);
        MObject all[] = {aInput, aFactor, aVector, aMode, aOutput, aVectorOut};
        for (int i = 0; i < 6; ++i) CHECK_MSTATUS_AND_RETURN_IT(addAttribute(all[i]));
        MObject ins[] = {aInput, aFactor, aMode};
        for (int i = 0; i < 3; ++i) attributeAffects(ins[i], aOutput);
        attributeAffects(aVector, aVectorOut);
        attributeAffects(aFactor, aVectorOut);
        attributeAffects(aMode, aVectorOut);
        return MS::kSuccess;
    }

    MStatus compute(const MPlug& plug, MDataBlock& data) override {
        double f = data.inputValue(aFactor).asDouble();
        bool add = data.inputValue(aMode).asShort() == 1;
        if (plug == aOutput) {
            double x = data.inputValue(aInput).asDouble();
            MDataHandle out = data.outputValue(aOutput);
            out.set(add ? x + f : x * f);
            out.setClean();
            return MS::kSuccess;
        }
        if (plug == aVectorOut || plug.parent() == aVectorOut) {
            float3& v = data.inputValue(aVector).asFloat3();
            float k = (float)f;
            MDataHandle out = data.outputValue(aVectorOut);
            if (add) out.set3Float(v[0] + k, v[1] + k, v[2] + k);
            else out.set3Float(v[0] * k, v[1] * k, v[2] * k);
            data.setClean(plug);
            return MS::kSuccess;
        }
        return MS::kUnknownParameter;
    }

    // Each node reads only its own inputs: any number may compute at once.
    SchedulingType schedulingType() const override { return kParallel; }

    static MTypeId id;
    static MObject aInput, aFactor, aVector, aMode, aOutput, aVectorOut;
};

// A block of ids for samples; a real plug-in uses the ids its vendor was given.
MTypeId ScaleNode::id(0x0013A4C0);
MObject ScaleNode::aInput, ScaleNode::aFactor, ScaleNode::aVector, ScaleNode::aMode, ScaleNode::aOutput, ScaleNode::aVectorOut;

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek samples", "1.0", "Any");
    return plugin.registerNode("azScale", ScaleNode::id, ScaleNode::creator, ScaleNode::initialize);
}

MStatus uninitializePlugin(MObject obj) {
    MFnPlugin plugin(obj);
    return plugin.deregisterNode(ScaleNode::id);
}
