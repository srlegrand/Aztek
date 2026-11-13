// Aztek plug-in kit: the OpenCL types and calls a GPU deformer names. Aztek has no OpenCL device, so no GPU deformer
// is ever made and these calls only answer that nothing could be done.
#pragma once
#include <cstddef>
#include <cstdint>

typedef int32_t cl_int;
typedef uint32_t cl_uint;
typedef uint64_t cl_ulong;
typedef float cl_float;
typedef cl_uint cl_bool;
typedef uint64_t cl_mem_flags;
typedef cl_uint cl_kernel_work_group_info;
typedef struct _cl_mem* cl_mem;
typedef struct _cl_kernel* cl_kernel;
typedef struct _cl_event* cl_event;
typedef struct _cl_command_queue* cl_command_queue;
typedef struct _cl_context* cl_context;
typedef struct _cl_device_id* cl_device_id;

#define CL_SUCCESS 0
#define CL_INVALID_VALUE -30
#define CL_FALSE 0
#define CL_TRUE 1
#define CL_MEM_READ_WRITE (1 << 0)
#define CL_MEM_WRITE_ONLY (1 << 1)
#define CL_MEM_READ_ONLY (1 << 2)
#define CL_MEM_USE_HOST_PTR (1 << 3)
#define CL_MEM_ALLOC_HOST_PTR (1 << 4)
#define CL_MEM_COPY_HOST_PTR (1 << 5)
#define CL_KERNEL_WORK_GROUP_SIZE 0x11B0

inline cl_mem clCreateBuffer(cl_context, cl_mem_flags, size_t, void*, cl_int* err) {
    if (err) *err = CL_INVALID_VALUE;
    return 0;
}
inline cl_int clSetKernelArg(cl_kernel, cl_uint, size_t, const void*) { return CL_INVALID_VALUE; }
inline cl_int clEnqueueWriteBuffer(cl_command_queue, cl_mem, cl_bool, size_t, size_t, const void*, cl_uint, const cl_event*, cl_event*) { return CL_INVALID_VALUE; }
inline cl_int clEnqueueReadBuffer(cl_command_queue, cl_mem, cl_bool, size_t, size_t, void*, cl_uint, const cl_event*, cl_event*) { return CL_INVALID_VALUE; }
inline cl_int clEnqueueNDRangeKernel(cl_command_queue, cl_kernel, cl_uint, const size_t*, const size_t*, const size_t*, cl_uint, const cl_event*, cl_event*) { return CL_INVALID_VALUE; }
inline cl_int clGetKernelWorkGroupInfo(cl_kernel, cl_device_id, cl_kernel_work_group_info, size_t, void*, size_t*) { return CL_INVALID_VALUE; }
inline cl_int clReleaseMemObject(cl_mem) { return CL_SUCCESS; }
inline cl_int clReleaseEvent(cl_event) { return CL_SUCCESS; }
inline cl_int clReleaseKernel(cl_kernel) { return CL_SUCCESS; }
inline cl_int clFinish(cl_command_queue) { return CL_SUCCESS; }
