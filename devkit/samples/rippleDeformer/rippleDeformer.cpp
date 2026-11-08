// Sample deformer: azRipple lifts each point by a wave across X and Z, times the envelope and the point's weight.
//   loadPlugin rippleDeformer; polyPlane -sx 20 -sy 20; deformer -type azRipple; setAttr azRipple1.amplitude 0.5;
#include <maya/MDataBlock.h>
#include <maya/MFnNumericAttribute.h>
#include <maya/MFnPlugin.h>
#include <maya/MItGeometry.h>
#include <maya/MPoint.h>
#include <maya/MPxDeformerNode.h>
#include <maya/MTypeId.h>

#include <cmath>

class RippleDeformer : public MPxDeformerNode {
public:
    static void* creator() { return new RippleDeformer; }

    static MStatus initialize() {
        MFnNumericAttribute n;
        aAmplitude = n.create("amplitude", "amp", MFnNumericData::kDouble, 0.25);
        n.setKeyable(true);
        aFrequency = n.create("frequency", "fq", MFnNumericData::kDouble, 1.0);
        n.setKeyable(true);
        n.setMin(0.0);
        addAttribute(aAmplitude);
        addAttribute(aFrequency);
        attributeAffects(aAmplitude, outputGeom);
        attributeAffects(aFrequency, outputGeom);
        return MS::kSuccess;
    }

    MStatus deform(MDataBlock& data, MItGeometry& iter, const MMatrix&, unsigned int multiIndex) override {
        float env = data.inputValue(envelope).asFloat();
        double amp = data.inputValue(aAmplitude).asDouble();
        double freq = data.inputValue(aFrequency).asDouble();
        if (env == 0.0f || amp == 0.0) return MS::kSuccess;
        for (iter.reset(); !iter.isDone(); iter.next()) {
            MPoint p = iter.position();
            float w = weightValue(data, multiIndex, iter.index());
            p.y += std::sin(p.x * freq) * std::cos(p.z * freq) * amp * env * w;
            iter.setPosition(p);
        }
        return MS::kSuccess;
    }

    static MTypeId id;
    static MObject aAmplitude, aFrequency;
};

MTypeId RippleDeformer::id(0x0013A4C1);
MObject RippleDeformer::aAmplitude, RippleDeformer::aFrequency;

MStatus initializePlugin(MObject obj) {
    MFnPlugin plugin(obj, "Aztek samples", "1.0", "Any");
    return plugin.registerNode("azRipple", RippleDeformer::id, RippleDeformer::creator, RippleDeformer::initialize, MPxNode::kDeformerNode);
}

MStatus uninitializePlugin(MObject obj) {
    MFnPlugin plugin(obj);
    return plugin.deregisterNode(RippleDeformer::id);
}
