// Aztek plug-in kit: animation curves and skin clusters, read and edited in the scene.
#pragma once
#include "iterators.h"

// What the reference keeps to undo a curve's edits. Edits here are undone with the scene's undo; this is taken and not
// used.
class MAnimCurveChange {
public:
    MAnimCurveChange() {}
    MStatus undoIt() { return MStatus(); }
    MStatus redoIt() { return MStatus(); }
};

class MFnAnimCurve : public MFnDependencyNode {
public:
    enum AnimCurveType {
        kAnimCurveTA = 0,
        kAnimCurveTL,
        kAnimCurveTT,
        kAnimCurveTU,
        kAnimCurveUA,
        kAnimCurveUL,
        kAnimCurveUT,
        kAnimCurveUU,
        kAnimCurveUnknown
    };
    enum TangentType {
        kTangentGlobal = 0,
        kTangentFixed,
        kTangentLinear,
        kTangentFlat,
        kTangentSmooth,
        kTangentStep,
        kTangentSlow,
        kTangentFast,
        kTangentClamped,
        kTangentPlateau,
        kTangentStepNext,
        kTangentAuto
    };
    enum InfinityType { kConstant = 0, kLinear, kCycle = 3, kCycleRelative, kOscillate };
    MFnAnimCurve();
    MFnAnimCurve(const MObject& o, MStatus* status = 0);
    // The curve driving a plug.
    MFnAnimCurve(const MPlug& plug, MStatus* status = 0);
    virtual MFn::Type type() const;
    static const char* className() { return "MFnAnimCurve"; }
    AnimCurveType animCurveType(MStatus* status = 0) const;
    bool isTimeInput(MStatus* status = 0) const;
    bool isUnitlessInput(MStatus* status = 0) const;
    bool isWeighted(MStatus* status = 0) const;
    unsigned int numKeys(MStatus* status = 0) const;
    unsigned int numKeyframes(MStatus* status = 0) const;
    MTime time(unsigned int index, MStatus* status = 0);
    double unitlessInput(unsigned int index, MStatus* status = 0);
    // A key's value, in the reference's internal units: radians on an angular curve.
    double value(unsigned int index, MStatus* status = 0);
    // The curve's value at a time (or at an input, for a curve driven by a plug), in internal units.
    double evaluate(const MTime& at, MStatus* status = 0);
    MStatus evaluate(const MTime& at, double& value);
    MStatus evaluate(double unitlessInput, double& value);
    bool find(const MTime& at, unsigned int& index, MStatus* status = 0);
    bool find(double unitlessInput, unsigned int& index, MStatus* status = 0);
    unsigned int findClosest(const MTime& at, MStatus* status = 0);
    unsigned int addKey(const MTime& at, double value, TangentType inType = kTangentGlobal, TangentType outType = kTangentGlobal, MAnimCurveChange* change = 0, MStatus* status = 0);
    MStatus remove(unsigned int index, MAnimCurveChange* change = 0);
    MStatus setValue(unsigned int index, double value, MAnimCurveChange* change = 0);
    MStatus setTime(unsigned int index, const MTime& at, MAnimCurveChange* change = 0);
    TangentType inTangentType(unsigned int index, MStatus* status = 0);
    TangentType outTangentType(unsigned int index, MStatus* status = 0);
    MStatus setInTangentType(unsigned int index, TangentType t, MAnimCurveChange* change = 0);
    MStatus setOutTangentType(unsigned int index, TangentType t, MAnimCurveChange* change = 0);
    InfinityType preInfinityType(MStatus* status = 0) const;
    InfinityType postInfinityType(MStatus* status = 0) const;
    MStatus setPreInfinityType(InfinityType t, MAnimCurveChange* change = 0);
    MStatus setPostInfinityType(InfinityType t, MAnimCurveChange* change = 0);

private:
    std::vector<double> azColumn(const char* flag, MStatus* status) const;
    bool azAngular() const;
    std::string azName() const;
};

class MFnSkinCluster : public MFnGeometryFilter {
public:
    MFnSkinCluster();
    MFnSkinCluster(const MObject& o, MStatus* status = 0);
    virtual MFn::Type type() const;
    static const char* className() { return "MFnSkinCluster"; }
    // The joints and other influences, in the order the influence indices below count them.
    unsigned int influenceObjects(MDagPathArray& out, MStatus* status = 0) const;
    // The influence's logical index: the element of the cluster's matrix array it drives.
    unsigned int indexForInfluenceObject(const MDagPath& influence, MStatus* status = 0) const;
    // Every influence's weight on each point, point by point.
    MStatus getWeights(const MDagPath& shape, const MObject& components, MDoubleArray& weights, unsigned int& influenceCount) const;
    // One influence's weights, by its place among influenceObjects.
    MStatus getWeights(const MDagPath& shape, const MObject& components, unsigned int influenceIndex, MDoubleArray& weights) const;
    MStatus getWeights(const MDagPath& shape, const MObject& components, MIntArray& influenceIndices, MDoubleArray& weights) const;
    MStatus setWeights(const MDagPath& shape, const MObject& components, unsigned int influenceIndex, double value, bool normalize = true, MDoubleArray* oldValues = 0);
    MStatus setWeights(const MDagPath& shape, const MObject& components, MIntArray& influenceIndices, MDoubleArray& values, bool normalize = true, MDoubleArray* oldValues = 0);
    MStatus getPointsAffectedByInfluence(const MDagPath& influence, MSelectionList& result, MDoubleArray& weights);
    // Dual quaternion blend weights: none are kept here, so 0 for each point.
    MStatus getBlendWeights(const MDagPath& shape, const MObject& components, MDoubleArray& weights);

private:
    bool azData(std::vector<std::pair<int, int64_t> >& influences, std::vector<std::vector<double> >& weights, int64_t& geometry) const;
};
