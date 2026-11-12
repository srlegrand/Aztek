// The C interface between a plug-in and the Aztek host. The host passes a table of function pointers when it loads a
// plug-in, so a plug-in links nothing of Aztek's and no C++ or Rust type crosses the line. Mirrored by hand in
// crates/aztek_host/src/native_plugin.rs: new entries go at the end of AzHost, and AZTEK_ABI_VERSION goes up.
//
// Strings are UTF-8. A char* given back is owned by the receiver and freed with string_free; strings for the host are
// made with string_new. Functions marked "any thread" may run inside a compute; the others fail there with an error.
#ifndef AZTEK_ABI_H
#define AZTEK_ABI_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define AZTEK_ABI_VERSION 1

typedef struct AzBlock AzBlock;
typedef struct AzMesh AzMesh;
typedef struct AzCurve AzCurve;

// What compute, doIt and the others return.
enum { AZ_OK = 0, AZ_UNKNOWN_PARAMETER = 1, AZ_FAILURE = 2, AZ_CRASHED = 3 };

// MPxNode::SchedulingType as the host sees it.
enum { AZ_SCHED_PARALLEL = 0, AZ_SCHED_SERIAL = 1, AZ_SCHED_GLOBALLY_SERIAL = 2, AZ_SCHED_UNTRUSTED = 3 };

// Node kinds a plug-in can register.
enum { AZ_NODE_DEPEND = 0, AZ_NODE_DEFORMER = 1 };

typedef struct AzMeshArrays {
    int64_t shape;
    const double* points;  // three per point
    size_t point_count;
    const int64_t* counts;
    size_t face_count;
    const int64_t* connects;
    size_t corner_count;
    const int64_t* corner_edges;  // corner_count
    const int64_t* edges;         // two per edge
    size_t edge_count;
    const double* corner_normals;  // three per corner
    const double* point_normals;   // three per point
    int has_uvs;
    const int64_t* corner_maps;  // corner_count
    const double* map_uvs;       // two per map
    size_t map_count;
    // The ids components use when they differ from the compact indices; empty otherwise.
    const int64_t* point_ids;
    size_t point_id_count;
    const int64_t* edge_ids;
    size_t edge_id_count;
    const int64_t* face_ids;
    size_t face_id_count;
    void* owner;  // the host's; given back to mesh_arrays_free
} AzMeshArrays;

// Filled in by the plug-in side, for each node type. Each runs under the plug-in's crash guard.
typedef struct AzNodeClass {
    void* ctx;
    void* (*create)(void* ctx, int64_t node_id, char** error);
    void (*destroy)(void* ctx, void* inst);
    int (*scheduling)(void* ctx, void* inst);
    // attr is the output's index on the type, element its array element or -1.
    int (*compute)(void* ctx, void* inst, int attr, int element, AzBlock* block, char** error);
} AzNodeClass;

typedef struct AzCommandClass {
    void* ctx;
    void* (*create)(void* ctx, int* undoable, char** error);
    void (*destroy)(void* ctx, void* inst);
    // call_json is {"args": [...], "flags": {...}}; result_json the command's result, or null.
    int (*do_it)(void* ctx, void* inst, const char* call_json, int* undoable, char** result_json, char** error);
    int (*undo_it)(void* ctx, void* inst, char** error);
    int (*redo_it)(void* ctx, void* inst, char** error);
} AzCommandClass;

typedef void (*AzCallback)(void* user, const char* payload_json);
typedef void (*AzJump)(void* buf, int signal);

typedef struct AzHost {
    uint32_t abi_version;
    uint32_t size;  // sizeof the host's table: entries past it aren't there

    // Strings. Any thread.
    char* (*string_new)(const char* text);
    void (*string_free)(char* text);

    // Writes to the script history: 0 info, 2 error, 4 warning, 1 result. Any thread; shown later from inside a compute.
    void (*log)(const char* text, int kind);

    // Scene access. Each answers {"ok": value} or {"error": text}.
    char* (*query)(const char* op, const char* args_json);
    char* (*command)(const char* name, const char* args_json, const char* flags_json);
    char* (*mel)(const char* source);
    char* (*python)(const char* source);
    uint64_t (*scene_version)(void);

    // Registering, from initializePlugin and uninitializePlugin. Return null or an error to free.
    char* (*plugin_info)(void* plugin, const char* vendor, const char* version, const char* required_api);
    char* (*register_command)(void* plugin, const char* name, const char* flags_json, int raw_args, const AzCommandClass* cls);
    char* (*deregister_command)(void* plugin, const char* name);
    char* (*register_node)(void* plugin, const char* name, uint32_t type_id, int kind, const char* attrs_json, const char* outputs_json, const AzNodeClass* cls);
    char* (*deregister_node)(void* plugin, const char* name);
    int (*attr_index)(const char* type_name, const char* attr_name);
    // Runs a callback when the host's aztekPluginCallback command asks for its id: scriptJob events call it.
    int64_t (*callback_add)(void* plugin, AzCallback fn, void* user);
    void (*callback_remove)(int64_t id);

    // A scene mesh's arrays. mesh_arrays_of works on a handle and runs on any thread.
    int (*mesh_arrays)(int64_t node, AzMeshArrays* out, char** error);
    int (*mesh_arrays_of)(const AzMesh* mesh, AzMeshArrays* out);
    void (*mesh_arrays_free)(AzMeshArrays* arrays);

    // Mesh handles, for what a compute reads and writes. Any thread, except mesh_of_node.
    AzMesh* (*mesh_of_node)(int64_t node, char** error);
    AzMesh* (*mesh_build)(const double* points, size_t point_count, const int64_t* counts, size_t face_count, const int64_t* connects, size_t corner_count);
    AzMesh* (*mesh_with_points)(const AzMesh* mesh, const double* points, size_t point_count);
    AzMesh* (*mesh_retain)(const AzMesh* mesh);
    void (*mesh_release)(AzMesh* mesh);

    // A node's data during its compute. Any thread; the block lasts for that compute.
    int64_t (*block_node)(AzBlock* block);
    double (*block_time)(AzBlock* block);
    double (*block_number)(AzBlock* block, int attr, int element, int component);
    void (*block_vec3)(AzBlock* block, int attr, int element, double* out);
    void (*block_matrix)(AzBlock* block, int attr, int element, double* out);
    const char* (*block_string)(AzBlock* block, int attr, int element);
    AzMesh* (*block_mesh)(AzBlock* block, int attr, int element);
    int (*block_element_count)(AzBlock* block, int attr);
    int (*block_element_at)(AzBlock* block, int attr, int physical);
    void (*block_set_number)(AzBlock* block, int attr, int element, int component, double value);
    void (*block_set_vec3)(AzBlock* block, int attr, int element, const double* value);
    void (*block_set_matrix)(AzBlock* block, int attr, int element, const double* value);
    void (*block_set_string)(AzBlock* block, int attr, int element, const char* value);
    void (*block_set_mesh)(AzBlock* block, int attr, int element, const AzMesh* mesh);
    // Per-point weights of a deformer; null means 1 everywhere.
    const double* (*block_weights)(AzBlock* block, size_t* count);

    // POSIX crash guard: while entered on this thread, a fatal signal calls jump(buf, signal). Any thread.
    void* (*guard_enter)(AzJump jump, void* buf);
    void (*guard_leave)(void* previous);

    // Curves a compute reads and writes: a handle, released with curve_release. Any thread. A table whose size ends
    // before these has none of them.
    AzCurve* (*block_curve)(AzBlock* block, int attr, int element);
    void (*curve_release)(AzCurve* curve);
    void (*curve_info)(const AzCurve* curve, int* degree, int* form, size_t* cv_count, size_t* knot_count, double* domain);
    void (*curve_cvs)(const AzCurve* curve, double* xyz);
    void (*curve_eval)(const AzCurve* curve, double param, double* point, double* tangent);
    double (*curve_length)(const AzCurve* curve);
    double (*curve_param_at_length)(const AzCurve* curve, double length);
    double (*curve_length_at_param)(const AzCurve* curve, double param);
    double (*curve_closest_param)(const AzCurve* curve, const double* point);
    AzCurve* (*curve_with_cvs)(const AzCurve* curve, const double* xyz, size_t cv_count);
    void (*block_set_curve)(AzBlock* block, int attr, int element, const AzCurve* curve);
} AzHost;

// What every plug-in exports. Written by the kit's entry source, not by hand.
typedef int (*AzPluginAbiFn)(void);
typedef int (*AzPluginLoadFn)(const AzHost* host, void* plugin, char** error);
typedef int (*AzPluginUnloadFn)(char** error);

#ifdef __cplusplus
}
#endif

#endif
