// Aztek plug-in kit: GPU deformers, registered and never made. See aztek/kit/gpu.h.
#include "internal.h"
#include <aztek/kit/gpu.h>
#include <map>

MEvaluationNode::MEvaluationNode() {}
MObject MEvaluationNode::dependencyNode(MStatus* st) const {
    if (st) *st = MS::kFailure;
    return MObject();
}

bool MEvaluationNode::dirtyPlugExists(const MObject&, MStatus* st) const {
    if (st) *st = MS::kSuccess;
    return true;
}

MAutoCLMem::MAutoCLMem() : mem_(0) {}
void MAutoCLMem::attach(cl_mem m) { mem_ = m; }
void MAutoCLMem::reset() { mem_ = 0; }
bool MAutoCLMem::isNull() const { return mem_ == 0; }
const cl_mem* MAutoCLMem::getReadOnlyRef() const { return &mem_; }
cl_mem* MAutoCLMem::getReferenceForAssignment() { return &mem_; }
cl_mem MAutoCLMem::get() const { return mem_; }

MAutoCLKernel::MAutoCLKernel() : kernel_(0) {}
void MAutoCLKernel::attach(cl_kernel k) { kernel_ = k; }
void MAutoCLKernel::reset() { kernel_ = 0; }
bool MAutoCLKernel::isNull() const { return kernel_ == 0; }
cl_kernel MAutoCLKernel::get() const { return kernel_; }
const cl_kernel* MAutoCLKernel::getReadOnlyRef() const { return &kernel_; }

MAutoCLEvent::MAutoCLEvent() : event_(0) {}
void MAutoCLEvent::reset() { event_ = 0; }
bool MAutoCLEvent::isNull() const { return event_ == 0; }
cl_event MAutoCLEvent::get() const { return event_; }
const cl_event* MAutoCLEvent::getReadOnlyRef() const { return &event_; }
cl_event* MAutoCLEvent::getReferenceForAssignment() { return &event_; }

MGPUDeformerBuffer::MGPUDeformerBuffer() {}
bool MGPUDeformerBuffer::isValid() const { return false; }
const MAutoCLMem& MGPUDeformerBuffer::buffer() const { return mem_; }
const MAutoCLEvent& MGPUDeformerBuffer::bufferReadyEvent() const { return event_; }
void MGPUDeformerBuffer::setBufferReadyEvent(const MAutoCLEvent& e) { event_ = e; }
unsigned int MGPUDeformerBuffer::elementCount() const { return 0; }
unsigned int MGPUDeformerBuffer::elementSize() const { return 0; }
MString MGPUDeformerBuffer::name() const { return MString(); }

MGPUDeformerData::MGPUDeformerData() {}
MGPUDeformerBuffer MGPUDeformerData::getBuffer(const MString&) const { return MGPUDeformerBuffer(); }
void MGPUDeformerData::setBuffer(const MGPUDeformerBuffer&) {}

MPxGPUDeformer::MPxGPUDeformer() {}
MPxGPUDeformer::~MPxGPUDeformer() {}
MPxGPUDeformer::DeformerStatus MPxGPUDeformer::evaluate(MDataBlock&, const MEvaluationNode&, const MPlug&, const MGPUDeformerData&, MGPUDeformerData&) { return kDeformerFailure; }
void MPxGPUDeformer::terminate() {}
MString MPxGPUDeformer::sPositionsName() { return MString("positions"); }
MString MPxGPUDeformer::sNormalsName() { return MString("normals"); }
MString MPxGPUDeformer::sGeometryMatrixName() { return MString("geometryMatrix"); }
MString MPxGPUDeformer::sInverseGeometryMatrixName() { return MString("inverseGeometryMatrix"); }
MGPUDeformerBuffer MPxGPUDeformer::createOutputBuffer(const MGPUDeformerBuffer&) { return MGPUDeformerBuffer(); }
bool MPxGPUDeformer::hasAttributeChanged(const MEvaluationNode&, const MObject&) const { return true; }

MGPUDeformerRegistrationInfo::MGPUDeformerRegistrationInfo() {}
MGPUDeformerRegistrationInfo::~MGPUDeformerRegistrationInfo() {}
bool MGPUDeformerRegistrationInfo::validateNodeInGraph(MDataBlock&, const MEvaluationNode&, const MPlug&, MStringArray*) { return false; }
bool MGPUDeformerRegistrationInfo::validateNodeValues(MDataBlock&, const MEvaluationNode&, const MPlug&, MStringArray*) { return false; }

// Kept so that deregistering what was registered succeeds, as it does in the reference.
static std::map<std::string, MGPUDeformerRegistrationInfo*>& registered() {
    static std::map<std::string, MGPUDeformerRegistrationInfo*> r;
    return r;
}
MStatus MGPUDeformerRegistry::registerGPUDeformerCreator(const MString& node, const MString& id, MGPUDeformerRegistrationInfo* info) {
    std::string key = std::string(node.asChar()) + "\n" + id.asChar();
    if (registered().count(key)) return MS::kFailure;
    registered()[key] = info;
    return MS::kSuccess;
}
MStatus MGPUDeformerRegistry::deregisterGPUDeformerCreator(const MString& node, const MString& id) {
    std::string key = std::string(node.asChar()) + "\n" + id.asChar();
    return registered().erase(key) ? MS::kSuccess : MS::kFailure;
}
MStatus MGPUDeformerRegistry::addConditionalAttribute(const MString&, const MString&, const MObject&) { return MS::kSuccess; }
MStatus MGPUDeformerRegistry::removeConditionalAttribute(const MString&, const MString&, const MObject&) { return MS::kSuccess; }

cl_context MOpenCLInfo::getOpenCLContext() { return 0; }
cl_device_id MOpenCLInfo::getOpenCLDeviceId() { return 0; }
cl_command_queue MOpenCLInfo::getMayaDefaultOpenCLCommandQueue() { return 0; }
cl_command_queue MOpenCLInfo::getOpenCLCommandQueue() { return 0; }
MAutoCLKernel MOpenCLInfo::getOpenCLKernel(const MString&, const MString&) { return MAutoCLKernel(); }
MAutoCLKernel MOpenCLInfo::getOpenCLKernelFromString(const MString&, const MString&, const MString&) { return MAutoCLKernel(); }
bool MOpenCLInfo::releaseOpenCLKernel(MAutoCLKernel& k) {
    k.reset();
    return true;
}
void MOpenCLInfo::checkCLErrorStatus(cl_int) {}
size_t MOpenCLInfo::openCLMaxWorkGroupSize() { return 0; }
