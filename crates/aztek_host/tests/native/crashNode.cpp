// A plug-in that fails on purpose: azCrash writes through a null pointer when crash is 1; azThrow throws.
#include <maya/MDataBlock.h>
#include <maya/MFnNumericAttribute.h>
#include <maya/MFnPlugin.h>
#include <maya/MPxNode.h>

#include <stdexcept>

class CrashNode : public MPxNode {
public:
    static void* creator() { return new CrashNode; }
    static MStatus initialize() {
        MFnNumericAttribute n;
        aCrash = n.create("crash", "cr", MFnNumericData::kInt, 0);
        aOutput = n.create("output", "out", MFnNumericData::kDouble, 0.0);
        n.setWritable(false);
        addAttribute(aCrash);
        addAttribute(aOutput);
        attributeAffects(aCrash, aOutput);
        return MS::kSuccess;
    }
    MStatus compute(const MPlug& plug, MDataBlock& data) override {
        if (plug != aOutput) return MS::kUnknownParameter;
        if (data.inputValue(aCrash).asInt() == 1) {
            volatile int* p = 0;
            *p = 1;
        }
        data.outputValue(aOutput).set(1.0);
        return MS::kSuccess;
    }
    static MObject aCrash, aOutput;
};
MObject CrashNode::aCrash, CrashNode::aOutput;

class ThrowNode : public MPxNode {
public:
    static void* creator() { return new ThrowNode; }
    static MStatus initialize() {
        MFnNumericAttribute n;
        aInput = n.create("input", "in", MFnNumericData::kDouble, 0.0);
        aOutput = n.create("output", "out", MFnNumericData::kDouble, 0.0);
        n.setWritable(false);
        addAttribute(aInput);
        addAttribute(aOutput);
        attributeAffects(aInput, aOutput);
        return MS::kSuccess;
    }
    MStatus compute(const MPlug& plug, MDataBlock& data) override {
        if (plug != aOutput) return MS::kUnknownParameter;
        if (data.inputValue(aInput).asDouble() != 0.0) throw std::runtime_error("input is not zero");
        data.outputValue(aOutput).set(0.0);
        return MS::kSuccess;
    }
    static MObject aInput, aOutput;
};
MObject ThrowNode::aInput, ThrowNode::aOutput;

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek tests", "1.0");
    MStatus st = plugin.registerNode("azCrash", MTypeId(0x0013A4F0), CrashNode::creator, CrashNode::initialize);
    if (st) st = plugin.registerNode("azThrow", MTypeId(0x0013A4F1), ThrowNode::creator, ThrowNode::initialize);
    return st;
}

MStatus uninitializePlugin(MObject obj) {
    MFnPlugin plugin(obj);
    plugin.deregisterNode(MTypeId(0x0013A4F0));
    return plugin.deregisterNode(MTypeId(0x0013A4F1));
}
