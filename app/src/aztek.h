// The Rust host library's C interface. Strings are UTF-8; returned strings are freed with aztek_free.
#pragma once
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

void aztek_free(char* p);
void aztek_init(void);
// The address of the main window, for Python tools that parent Qt windows to it.
void aztek_set_main_window(void* window);
// The application's start-up: reads the saved preferences (up axis, new scene units, optionVars) and keeps them.
void aztek_use_prefs(void);
// Once the main window is up: loads the plug-ins marked to load at start and runs the user folder's userSetup scripts.
void aztek_startup_scripts(void);
char* aztek_mel(const char* src);
int aztek_python(const char* src);
char* aztek_exec(const char* name, const char* call_json);
char* aztek_exec_quiet(const char* name, const char* call_json);
void aztek_retract(void);
char* aztek_ask(const char* name, const char* call_json);
char* aztek_take_log(void);
void aztek_log(const char* text, int kind);
uint64_t aztek_version(void);
char* aztek_query(const char* what, const char* arg);
char* aztek_file(const char* op, const char* path);
void aztek_undo(void);
void aztek_redo(void);

// The feedback report. The link and text from {title, description, version, os, qt, attachments}: {url, title, body,
// fullBody, shortened}. A zipped copy of the scene, the scene untouched: {ok: bytes} or {error}.
char* aztek_feedback_issue(const char* call_json);
char* aztek_feedback_scene_zip(const char* path);

// Updates. The latest release on GitHub: {tag, name, body, url, assets: [{name, url, size, digest}]} or {error}.
// A download to a file, checked against the digest when given: null when it worked, else the reason. Both block.
char* aztek_update_latest(void);
char* aztek_update_download(const char* url, const char* path, const char* digest);
void aztek_update_progress(uint64_t* done, uint64_t* total);
void aztek_update_cancel(void);

// Script UI: operations for the shell to mirror (null when none), what the user did, and the hook for what must answer
// at once. Dialogs run the event loop inside the hook, on top of a suspended command.
char* aztek_ui_take_ops(void);
void aztek_ui_event(const char* json);
typedef void (*AztekUiHook)(const char* kind, const char* json);
void aztek_ui_set_hook(AztekUiHook hook);
void aztek_ui_answer(const char* json);
void aztek_ui_progress_cancel(void);
int aztek_ui_in_hook(void);
void aztek_idle(void);

// An image file as RGBA, top row first; null when it can't be read. Given back with its size.
unsigned char* aztek_image_load(const char* path, int* width, int* height);
void aztek_image_free(unsigned char* data, int width, int height);

// Harold, the renderer: the render view's status JSON (an interactive render restarts here after an edit), an AOV or a
// kept snapshot (index >= 0) as RGBA after exposure and gamma (free with aztek_image_free), and batch rendering from
// the command line with no display (`aztek -render ...`), returning the exit code.
char* aztek_render_status(void);
unsigned char* aztek_render_image(const char* aov, int snapshot, double exposure, double gamma, int* width, int* height);
int aztek_render_batch(int argc, const char* const* argv);
int aztek_batch(int argc, const char* const* argv);

enum { AZTEK_XCB = 1, AZTEK_XLIB = 2, AZTEK_WIN32 = 3, AZTEK_WAYLAND = 4, AZTEK_APPKIT = 5 };

int aztek_view_create(int platform, void* display, uint64_t window, void* extra, int width, int height, double dpr,
                      const char* camera);
void aztek_view_destroy(int id);
void aztek_view_resize(int id, int width, int height);
void aztek_view_render(int id);
int aztek_view_mouse(int id, int kind, int button, double x, double y, int mods, int buttons);
void aztek_view_wheel(int id, double delta);
void aztek_view_set(int id, const char* key, const char* value_json);
char* aztek_view_get(int id, const char* key);
int aztek_view_capture(int id, const char* path);
char* aztek_view_hit(int id, double x, double y);

#ifdef __cplusplus
}
#endif
