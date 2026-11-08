// Aztek plug-in kit: the entry points the host looks for. aztek_add_plugin compiles this into every plug-in, where it
// calls the plug-in's own initializePlugin and uninitializePlugin.
#include <aztek/abi.h>
#include <maya/MFnPlugin.h>
#include <maya/MObject.h>
#include <maya/MStatus.h>

MStatus initializePlugin(MObject obj);
MStatus uninitializePlugin(MObject obj);

namespace az {
typedef MStatus (*EntryFn)(MObject);
int load(const AzHost* h, void* plugin, char** error, EntryFn init);
int unload(char** error, EntryFn uninit);
}

#if defined(_WIN32)
#define AZ_ENTRY extern "C" __declspec(dllexport)
#else
#define AZ_ENTRY extern "C" __attribute__((visibility("default")))
#endif

AZ_ENTRY int aztek_plugin_abi(void) { return AZTEK_ABI_VERSION; }
AZ_ENTRY int aztek_plugin_load(const AzHost* host, void* plugin, char** error) { return az::load(host, plugin, error, &initializePlugin); }
AZ_ENTRY int aztek_plugin_unload(char** error) { return az::unload(error, &uninitializePlugin); }
