// Aztek plug-in kit: GPU deformers and the evaluation graph's nodes.
//
// Aztek deforms on the CPU only. A plug-in may register a GPU override for its deformer; the registration is accepted
// and kept, and the deformer's own deform() is what runs. The OpenCL calls such an override makes are declared in
// <clew/clew_cl.h> and do nothing.
#pragma once
#include "plugin.h"
#include <clew/clew_cl.h>

class MEvaluationNode {
public:
    MEvaluationNode();
    MObject dependencyNode(MStatus* status = 0) const;
    bool dirtyPlugExists(const MObject& attribute, MStatus* status = 0) const;
};

// Holds an OpenCL object; Aztek never makes one, so these are always empty.
class MAutoCLMem {
public:
    MAutoCLMem();
    void attach(cl_mem m);
    void reset();
    bool isNull() const;
    const cl_mem* getReadOnlyRef() const;
    cl_mem* getReferenceForAssignment();
    cl_mem get() const;

private:
    cl_mem mem_;
};

class MAutoCLKernel {
public:
    MAutoCLKernel();
    void attach(cl_kernel k);
    void reset();
    bool isNull() const;
    cl_kernel get() const;
    const cl_kernel* getReadOnlyRef() const;

private:
    cl_kernel kernel_;
};

class MAutoCLEvent {
public:
    MAutoCLEvent();
    void reset();
    bool isNull() const;
    cl_event get() const;
    const cl_event* getReadOnlyRef() const;
    cl_event* getReferenceForAssignment();

private:
    cl_event event_;
};

class MGPUDeformerBuffer {
public:
    MGPUDeformerBuffer();
    bool isValid() const;
    const MAutoCLMem& buffer() const;
    const MAutoCLEvent& bufferReadyEvent() const;
    void setBufferReadyEvent(const MAutoCLEvent& e);
    unsigned int elementCount() const;
    unsigned int elementSize() const;
    MString name() const;

private:
    MAutoCLMem mem_;
    MAutoCLEvent event_;
};

class MGPUDeformerData {
public:
    MGPUDeformerData();
    MGPUDeformerBuffer getBuffer(const MString& name) const;
    void setBuffer(const MGPUDeformerBuffer& buffer);
};

class MPxGPUDeformer {
public:
    enum DeformerStatus { kDeformerSuccess = 0, kDeformerFailure, kDeformerRetryMainThread, kDeformerPassThrough };
    MPxGPUDeformer();
    virtual ~MPxGPUDeformer();
    virtual DeformerStatus evaluate(MDataBlock& block, const MEvaluationNode& evaluationNode, const MPlug& plug, const MGPUDeformerData& inputData, MGPUDeformerData& outputData);
    virtual void terminate();
    static MString sPositionsName();
    static MString sNormalsName();
    static MString sGeometryMatrixName();
    static MString sInverseGeometryMatrixName();
    MGPUDeformerBuffer createOutputBuffer(const MGPUDeformerBuffer& inputBuffer);
    bool hasAttributeChanged(const MEvaluationNode& evaluationNode, const MObject& attr) const;
};

class MGPUDeformerRegistrationInfo {
public:
    MGPUDeformerRegistrationInfo();
    virtual ~MGPUDeformerRegistrationInfo();
    virtual MPxGPUDeformer* createGPUDeformer() = 0;
    virtual bool validateNodeInGraph(MDataBlock& block, const MEvaluationNode& evaluationNode, const MPlug& plug, MStringArray* messages);
    virtual bool validateNodeValues(MDataBlock& block, const MEvaluationNode& evaluationNode, const MPlug& plug, MStringArray* messages);
};

class MGPUDeformerRegistry {
public:
    static MStatus registerGPUDeformerCreator(const MString& nodeClassName, const MString& registrantId, MGPUDeformerRegistrationInfo* info);
    static MStatus deregisterGPUDeformerCreator(const MString& nodeClassName, const MString& registrantId);
    static MStatus addConditionalAttribute(const MString& nodeClassName, const MString& registrantId, const MObject& attr);
    static MStatus removeConditionalAttribute(const MString& nodeClassName, const MString& registrantId, const MObject& attr);
};

class MOpenCLInfo {
public:
    static cl_context getOpenCLContext();
    static cl_device_id getOpenCLDeviceId();
    static cl_command_queue getMayaDefaultOpenCLCommandQueue();
    static cl_command_queue getOpenCLCommandQueue();
    static MAutoCLKernel getOpenCLKernel(const MString& fileName, const MString& kernelName);
    static MAutoCLKernel getOpenCLKernelFromString(const MString& source, const MString& uniqueName, const MString& kernelName);
    static bool releaseOpenCLKernel(MAutoCLKernel& kernel);
    static void checkCLErrorStatus(cl_int err);
    static size_t openCLMaxWorkGroupSize();
};
