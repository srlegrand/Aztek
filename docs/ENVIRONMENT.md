# Environment variables

Written by `cargo run -p aztek_host --example gen_environment` from `crates/aztek_core/src/env.rs`. Do not edit by hand.

The list is the reference's environment pages for 2026, read on 12/11/2026, with Aztek's own variables at the end.

## Names

A variable spelled `MAYA_X` is read as `AZTEK_X` first and `MAYA_X` second. For a list of folders both are used,
`AZTEK_X`'s folders first. `MAYA_APP_DIR` and `AZTEK_APP_DIR` are the exception: they are two different folders.

## The environment file

- `Aztek.env`, or `Maya.env`, in the folder `AZTEK_ENV_DIR` or `MAYA_ENV_DIR` names. Without either: the first found
in the version's folder of the user folder, then in the user folder. One file is read.
- `NAME = value` a line. `#` starts a comment. `$NAME`, `${NAME}` and `%NAME%` are filled in. Folders are separated
by `:` (`;` on Windows).
- A variable the system already set is left as it is. The file's folders for `MAYA_SCRIPT_PATH`, `MAYA_PLUG_IN_PATH`,
`MAYA_MODULE_PATH`, `MAYA_SHELF_PATH`, `MAYA_PRESET_PATH`, `XBMLANGPATH`, `PYTHONPATH` and `PATH` are added after
the ones the system gave. The reference's page says the file replaces the system's value for some of these; how
the reference combines them has not been checked against the application.
- `MAYA_APP_DIR`, `AZTEK_APP_DIR`, `MAYA_LOCATION`, `AZTEK_LOCATION`, `HOME` and `USERPROFILE` are not taken from
the file; a warning says so.

## In a session

`getenv` and `putenv` read and set the process's variables. The path lists are read each time they are used, so a
`putenv` takes effect at once. `aztekEnvironment -q` lists every variable here with its value in force.

## File paths

| Variable | Read | Notes |
|---|---|---|
| `MAYA_APP_DIR` | partly | Where the reference's user folder is looked for when Aztek offers to use it. Aztek's own folder is moved with AZTEK_APP_DIR, which is a different thing and not a second spelling. Inside a session MAYA_APP_DIR is set to the user folder in use, so scripts that ask for it get the right place. Not taken from an environment file. |
| `MAYA_ENV_DIR` | yes | The one folder Aztek.env or Maya.env is read from. Without it: the version's folder of the user folder, then the user folder. |
| `MAYA_CMD_FILE_OUTPUT` | yes | The Script Editor's lines are also written to this file from the start, as cmdFileOutput does. |
| `MAYA_CONTENT_PATH` | no |  |
| `MAYA_WINDOWS_CRASH_LOG_DIR` | no |  |
| `MAYA_CUSTOM_TEMPLATE_PATH` | no |  |
| `MAYA_CUSTOM_TEMPLATE_WRITE_PATH` | no |  |
| `MAYA_MODULE_PATH` | yes | Folders of .mod files, ahead of the user folder's modules folders. |
| `MAYA_MOVIE_DIR` | no |  |
| `MAYA_LOCATION` | partly | Set at start to where Aztek is installed, whatever it was, so scripts that build paths from it stay inside Aztek. AZTEK_LOCATION is set to the same. Not taken from an environment file. |
| `MAYA_PACKAGE_PATH` | no |  |
| `MAYA_PLUG_IN_PATH` | yes | Folders plug-ins are found in, and what the Plug-in Manager lists. A module's plug-ins folder is added. |
| `MAYA_PRESET_PATH` | partly | Each entry is the folder above attrPresets. Aztek's own preset files (.json) are found there; the reference's .mel presets are not read. |
| `MAYA_PROJECT` | yes | The project a session starts in. |
| `MAYA_PROJECTS_DIR` | partly | The folder projects are kept in: what internalVar -userWorkspaceDir answers. The Preferences window does not show it. |
| `MAYA_SHELF_PATH` | partly | Each folder's shelf_*.mel files are shown as shelves, after the ones kept in the preferences; a name already there is not loaded twice. They are read at start and not written back. |
| `MAYA_SCRIPT_PATH` | yes | Folders MEL scripts are found in, for procedures not yet known and for source. A module's scripts folder is added. |
| `MAYA_TOOLCLIPS_PATH` | no |  |
| `PYTHONPATH` | yes | Folders Python modules are found in. The user folder's scripts folders are added after them. |
| `XBMLANGPATH` | yes | Folders icons are found in, for shelf buttons and for the image flags of interface commands. An entry may end in /%B. A module's icons folder and the user folder's prefs/icons are added. |
| `TMPDIR` | yes | The temporary folder: what internalVar -userTmpDir answers. TEMP on Windows. |

## General

| Variable | Read | Notes |
|---|---|---|
| `MAYA_ALIAS_LICENSE_TIMEOUT` | no | Licensing: Aztek has none. |
| `MAYA_ALLOW_REFERENCED_CAMERA_SEQUENCER` | no |  |
| `MAYA_CACHE_DISABLE_TIME_EDITOR` | no |  |
| `MAYA_DEBUG_CORE_ON_CRASH` | no |  |
| `MAYA_DEBUG_ENABLE_CRASH_REPORTING` | no |  |
| `MAYA_DEBUG_SIGTERM_AS_SIGINT` | no |  |
| `MAYA_DET_TANGENT` | no |  |
| `MAYA_DISABLE_ADP` | no | Analytics: Aztek sends none. |
| `MAYA_DISABLE_BACKSPACE_DELETE` | yes | Backspace no longer deletes the selection. Read at start. |
| `MAYA_DISABLE_LOD_VISIBILITY` | no |  |
| `MAYA_DISABLE_PACKED_MULTI_ATTRIBUTES` | no |  |
| `MAYA_ENABLE_MULTI_DRAW_CONSOLIDATION` | no |  |
| `MAYA_FORCE_REF_READ` | no |  |
| `MAYA_FBX_LOG_FILENAME` | no |  |
| `MAYA_FBX_LOG_DATETIME_ISO` | no |  |
| `MAYA_MULTI_SKIN_CLUSTER` | no |  |
| `MAYA_NO_HOME` | no | Aztek has no home screen: it starts in the workspace. |
| `MAYA_NO_HOME_ICON` | no | Aztek has no home icon. |
| `MAYA_NO_WARNING_FOR_MISSING_DEFAULT_RENDERER` | no |  |
| `MAYA_PARALLEL_MEMCPY` | no |  |
| `MAYA_IP_TYPE` | no |  |
| `MAYA_HELP_URL` | no |  |
| `MAYA_OVERRIDE_UI` | no |  |
| `MAYA_PAINT_EFFECTS_THREADS` | no |  |
| `WINEDITOR` | no |  |
| `MAYA_SCRIPT_EDITOR_STDOUT` | yes | The Script Editor's lines are also written to standard output. |
| `MAYA_UI_LANGUAGE` | no | Aztek's interface is in English only. |
| `MAYA_DISALLOW_DUPLICATE_ATTRIBUTE_NAMES` | no | Aztek always refuses a second attribute of the same name on a node. |
| `MAYA_USE_ALTERNATE_UNFOLD_SHIM` | no |  |
| `MAYA_USE_MALLOC` | no |  |
| `MAYA_USE_VARYING_INT` | no |  |
| `MAYA_DISABLE_CASCADING` | no | Licensing: Aztek has none. |
| `MAYA_CER_INCLUDE_SCENE_NAME` | no |  |
| `MAYA_FORCE_PANEL_FOCUS` | no |  |
| `MAYA_ENABLE_NG_CONSOLE_STATS` | no |  |
| `MAYA_LOCK_FOREGROUND_WINDOW` | no |  |
| `MAYA_DISABLE_PLUGIN_SCENE_MODIFIED_WARNING` | no |  |
| `MAYA_ENABLE_LEGACY_INCORRECT_NODE_NAMES` | no |  |
| `MAYA_SKIP_BLOCK_CHECK` | no |  |
| `MAYA_PYTHON_USER_PACKAGES_PRIORITY` | no |  |
| `MAYA_WRITE_HIK_V1` | no |  |
| `MAYA_USER_CER_APP` | no |  |
| `MAYA_AUTOSEND_CER` | no |  |
| `MAYA_AUTOSEND_CER_DESCRIPTION` | no |  |
| `MAYA_SECURE_OPTOUT` | no |  |
| `MAYA_SKIP_USERSETUP_PY` | yes | userSetup.py files are not run at start. Not on the reference's environment pages; its start-up reads it. |

## Rendering

| Variable | Read | Notes |
|---|---|---|
| `AW_JPEG_Q_FACTOR` | partly | The quality of JPEG files Harold writes, 1 to 100. Without it Aztek writes at 75. |
| `AW_JPEG_SUB_SAMPLING` | no |  |
| `MAYA_NO_JITTER_FINAL_COLOR` | no |  |
| `MAYA_RENDERER_RT_BACKGROUND_COLOR` | no |  |
| `MAYA_HW_FILE_TEXTURE_RESOLUTION_OVERRIDE` | no |  |
| `MAYA_OFFSCREEN_HRB` | no |  |
| `MAYA_EXR_LONGNAME` | no |  |
| `MAYA_QUICKTIME_ENCODING_GAMMA` | no |  |
| `OCIO` | no |  |
| `OCIO_ACTIVE_DISPLAYS` | no |  |
| `OCIO_ACTIVE_VIEWS` | no |  |
| `OCIO_INACTIVE_COLORSPACES` | no |  |
| `MAYA_COLOR_MANAGEMENT_POLICY_FILE` | no |  |
| `MAYA_COLOR_MANAGEMENT_POLICY_LOCK` | no |  |
| `MAYA_COLOR_MGT_NO_LOGGING` | no |  |
| `MAYA_ENABLE_DEPRECATED_VIEWPORT` | no |  |
| `MAYA_VP2_DEVICE_OVERRIDE` | no | Aztek's view draws with Vulkan only. |
| `MAYA_ALLOW_OPENGL_REMOTE_SESSION` | no |  |
| `MAYA_VP2_USE_LINEAR_NC_SPACE_DEPTH_PRIORITY` | no |  |
| `MAYA_VP2_OFF_NON_PE_FRAGMENT` | no |  |
| `MAYA_WANT_LEGACY_VP2_DEFAULT_LIGHTING` | no |  |
| `MAYA_FORCE_DX_DEBUG_DEVICE` | no |  |
| `MAYA_OBJECT_SORT_USING_BOUNDING_BOX` | no |  |
| `MAYA_GPUCACHE_VP2_OVERRIDE_API` | no |  |
| `MAYA_FORCE_DX_WARP` | no |  |
| `MAYA_OGS_GPU_MEMORY_LIMIT` | no |  |
| `MAYA_VP2_PAUSE_ON_STARTUP` | no |  |
| `MAYA_ALLOW_RENDER_LAYER_SWITCHING` | no |  |
| `MAYA_BATCH_DISABLE_UNDO_MANAGER_FLUSH_ON_EXIT` | no |  |
| `SHADERFX_CUSTOMUSERPATH` | no |  |
| `MAYA_TEXCOORD_ORIENTATION` | no |  |
| `MAYA_RENDER_SETUP_GLOBAL_TEMPLATE_PATH` | no |  |
| `MAYA_RENDER_SETUP_GLOBAL_PRESETS_PATH` | no |  |
| `MAYA_RENDER_SETUP_USE_UNTITLED_COLLECTIONS` | no |  |
| `MAYA_RENDER_SETUP_INCLUDE_ALL_LIGHTS` | no |  |
| `MAYA_RENDER_SETUP_DISPLAY_RS_NODES` | no |  |
| `MAYA_RENDER_SETUP_DISABLE_RENDER_SETTINGS_AOVS_EXPORT` | no |  |
| `MAYA_RENDER_SETUP_ENABLE_WARNING_ICONS` | no |  |

## Linux

| Variable | Read | Notes |
|---|---|---|
| `MAYA_WORKSPACES_ALWAYS_SHOW_TAB` | no |  |
| `MAYA_DEBUG_NO_SIGINT_HANDLING` | no |  |
| `__GL_MAYA_OPTIMIZE` | no |  |
| `MAYA_MMSET_DEFAULT_XCURSOR` | no |  |
| `MAYA_SOUND_SCRUB` | no |  |
| `MAYA_SET_PARENT_HINT` | no |  |
| `MAYA_STYLUS_DEVICE` | no |  |
| `MAYA_SET_XERROR` | no |  |
| `MAYA_FORCE_SHOW_ACTIVATE` | no |  |
| `MAYA_ACTIVE_STEREO_REFRESH` | no |  |
| `XLIB_SKIP_ARGB_VISUALS` | no |  |
| `MAYA_TTIP_OFFSETX` | no |  |
| `MAYA_TTIP_OFFSETY` | no |  |

## Audio

| Variable | Read | Notes |
|---|---|---|
| `MAYA_AUDIO_LIBRARY` | no |  |
| `MAYA_AUDIO_OPENAL_DEVICE` | no |  |

## Scene files

| Variable | Read | Notes |
|---|---|---|
| `MAYA_ASCII_ENABLE_BULK_PARSING` | no |  |
| `MAYA_ASCII_SUPPORT_MAC_LINE_ENDINGS` | no |  |

## Aztek's own

| Variable | Read | Notes |
|---|---|---|
| `AZTEK_APP_DIR` | yes | Aztek's own user folder, in place of ~/aztek (Documents/aztek on Windows). |
| `AZTEK_PREFS` | yes | Where Aztek keeps its preferences, in place of ~/.aztek. |
| `AZTEK_CONFIG_DIR` | yes | Where Aztek keeps its settings, in place of the system's settings folder. |
| `AZTEK_STUDIO_PREFS` | yes | Folders of preferences a studio provides. |
| `AZTEK_LOCATION` | yes | Set at start to where Aztek is installed. |
| `AZTEK_MSAA` | yes | Samples a pixel the view starts with: 1, 2, 4 or 8. Overrides the saved choice. |
| `AZTEK_UPDATE_URL` | yes | Where updates are asked about. |
| `AZTEK_ECHO` | yes | The Script Editor's lines are also written to standard error. |
| `AZTEK_PROFILE` | yes | Timings of the view's drawing are written out. |

18 of the reference's 122 are read.
