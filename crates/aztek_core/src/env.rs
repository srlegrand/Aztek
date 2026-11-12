//! Environment variables: the ones the reference documents, which of them Aztek reads, and how they are read.
//!
//! One rule for names: a variable the reference spells `MAYA_X` is read here as `AZTEK_X` first and `MAYA_X` second,
//! so a studio's existing environment works as it is and Aztek can still be told something different. For a list of
//! folders both are used, `AZTEK_X`'s folders first. Variables without the prefix (`XBMLANGPATH`, `PYTHONPATH`,
//! `TMPDIR`, `AW_JPEG_Q_FACTOR`) have one name.
//!
//! `VARS` is the whole list; `docs/ENVIRONMENT.md` is written from it.

use std::path::{Path, PathBuf};

/// Aztek's own spelling of a variable of the reference's: `MAYA_SCRIPT_PATH` gives `AZTEK_SCRIPT_PATH`.
pub fn own_name(name: &str) -> Option<String> {
    name.strip_prefix("MAYA_").map(|rest| format!("AZTEK_{rest}"))
}

fn raw(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|v| !v.is_empty())
}

/// A variable's value: Aztek's own spelling first, then the name as given. Empty counts as not set.
pub fn get(name: &str) -> Option<String> {
    own_name(name).and_then(|own| raw(&own)).or_else(|| raw(name))
}

/// Whether a switch is on: set, and not to 0.
pub fn on(name: &str) -> bool {
    get(name).is_some_and(|v| v.trim() != "0")
}

/// A list of folders: Aztek's own spelling's folders, then the name's as given, each once.
pub fn list(name: &str) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for var in own_name(name).into_iter().chain([name.to_string()]) {
        let Some(v) = std::env::var_os(&var) else { continue };
        for p in std::env::split_paths(&v) {
            if !p.as_os_str().is_empty() && !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

/// The folders icons are looked for in: `XBMLANGPATH`, where an entry may end in `/%B` (the reference's Linux
/// spelling, the file's name standing where `%B` is).
pub fn icon_dirs() -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = Vec::new();
    for p in list("XBMLANGPATH") {
        let text = p.to_string_lossy();
        let dir = match text.strip_suffix("%B") {
            Some(d) => PathBuf::from(d.trim_end_matches(['/', '\\'])),
            None => p.clone(),
        };
        if !dir.as_os_str().is_empty() && !out.contains(&dir) {
            out.push(dir);
        }
    }
    out
}

/// An icon's file: the name itself when it is a file, or the first folder of `XBMLANGPATH` that holds it.
pub fn find_icon(name: &str) -> Option<PathBuf> {
    if name.is_empty() {
        return None;
    }
    let given = Path::new(name);
    if given.is_file() {
        return Some(given.to_path_buf());
    }
    if given.is_absolute() {
        return None;
    }
    icon_dirs().into_iter().map(|d| d.join(name)).find(|p| p.is_file())
}

/// Whether Aztek acts on a variable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Read, and does what the reference's does.
    Read,
    /// Read, with a difference the note states.
    Partly,
    /// Not read.
    NotRead,
}

/// One variable: the name the reference documents, where it documents it, whether Aztek reads it, and what to know.
pub struct Var {
    pub name: &'static str,
    pub group: &'static str,
    pub status: Status,
    pub note: &'static str,
}

const fn v(name: &'static str, group: &'static str, status: Status, note: &'static str) -> Var {
    Var { name, group, status, note }
}

use Status::{NotRead, Partly, Read};

const FILES: &str = "File paths";
const GENERAL: &str = "General";
const RENDER: &str = "Rendering";
const LINUX: &str = "Linux";
const AUDIO: &str = "Audio";
const ASCII: &str = "Scene files";
const OWN: &str = "Aztek's own";

/// Every variable the reference's environment pages document (2026 pages, read 12/11/2026), the two its start-up
/// reads besides, and Aztek's own.
pub const VARS: &[Var] = &[
    v("MAYA_APP_DIR", FILES, Partly, "Where the reference's user folder is looked for when Aztek offers to use it. Aztek's own folder is moved with AZTEK_APP_DIR, which is a different thing and not a second spelling. Inside a session MAYA_APP_DIR is set to the user folder in use, so scripts that ask for it get the right place. Not taken from an environment file."),
    v("MAYA_ENV_DIR", FILES, Read, "The one folder Aztek.env or Maya.env is read from. Without it: the version's folder of the user folder, then the user folder."),
    v("MAYA_CMD_FILE_OUTPUT", FILES, Read, "The Script Editor's lines are also written to this file from the start, as cmdFileOutput does."),
    v("MAYA_CONTENT_PATH", FILES, NotRead, ""),
    v("MAYA_WINDOWS_CRASH_LOG_DIR", FILES, NotRead, ""),
    v("MAYA_CUSTOM_TEMPLATE_PATH", FILES, NotRead, ""),
    v("MAYA_CUSTOM_TEMPLATE_WRITE_PATH", FILES, NotRead, ""),
    v("MAYA_MODULE_PATH", FILES, Read, "Folders of .mod files, ahead of the user folder's modules folders."),
    v("MAYA_MOVIE_DIR", FILES, NotRead, ""),
    v("MAYA_LOCATION", FILES, Partly, "Set at start to where Aztek is installed, whatever it was, so scripts that build paths from it stay inside Aztek. AZTEK_LOCATION is set to the same. Not taken from an environment file."),
    v("MAYA_PACKAGE_PATH", FILES, NotRead, ""),
    v("MAYA_PLUG_IN_PATH", FILES, Read, "Folders plug-ins are found in, and what the Plug-in Manager lists. A module's plug-ins folder is added."),
    v("MAYA_PRESET_PATH", FILES, Partly, "Each entry is the folder above attrPresets. Aztek's own preset files (.json) are found there; the reference's .mel presets are not read."),
    v("MAYA_PROJECT", FILES, Read, "The project a session starts in."),
    v("MAYA_PROJECTS_DIR", FILES, Partly, "The folder projects are kept in: what internalVar -userWorkspaceDir answers. The Preferences window does not show it."),
    v("MAYA_SHELF_PATH", FILES, Partly, "Each folder's shelf_*.mel files are shown as shelves, after the ones kept in the preferences; a name already there is not loaded twice. They are read at start and not written back."),
    v("MAYA_SCRIPT_PATH", FILES, Read, "Folders MEL scripts are found in, for procedures not yet known and for source. A module's scripts folder is added."),
    v("MAYA_TOOLCLIPS_PATH", FILES, NotRead, ""),
    v("PYTHONPATH", FILES, Read, "Folders Python modules are found in. The user folder's scripts folders are added after them."),
    v("XBMLANGPATH", FILES, Read, "Folders icons are found in, for shelf buttons and for the image flags of interface commands. An entry may end in /%B. A module's icons folder and the user folder's prefs/icons are added."),
    v("TMPDIR", FILES, Read, "The temporary folder: what internalVar -userTmpDir answers. TEMP on Windows."),
    v("MAYA_ALIAS_LICENSE_TIMEOUT", GENERAL, NotRead, "Licensing: Aztek has none."),
    v("MAYA_ALLOW_REFERENCED_CAMERA_SEQUENCER", GENERAL, NotRead, ""),
    v("MAYA_CACHE_DISABLE_TIME_EDITOR", GENERAL, NotRead, ""),
    v("MAYA_DEBUG_CORE_ON_CRASH", GENERAL, NotRead, ""),
    v("MAYA_DEBUG_ENABLE_CRASH_REPORTING", GENERAL, NotRead, ""),
    v("MAYA_DEBUG_SIGTERM_AS_SIGINT", GENERAL, NotRead, ""),
    v("MAYA_DET_TANGENT", GENERAL, NotRead, ""),
    v("MAYA_DISABLE_ADP", GENERAL, NotRead, "Analytics: Aztek sends none."),
    v("MAYA_DISABLE_BACKSPACE_DELETE", GENERAL, Read, "Backspace no longer deletes the selection. Read at start."),
    v("MAYA_DISABLE_LOD_VISIBILITY", GENERAL, NotRead, ""),
    v("MAYA_DISABLE_PACKED_MULTI_ATTRIBUTES", GENERAL, NotRead, ""),
    v("MAYA_ENABLE_MULTI_DRAW_CONSOLIDATION", GENERAL, NotRead, ""),
    v("MAYA_FORCE_REF_READ", GENERAL, NotRead, ""),
    v("MAYA_FBX_LOG_FILENAME", GENERAL, NotRead, ""),
    v("MAYA_FBX_LOG_DATETIME_ISO", GENERAL, NotRead, ""),
    v("MAYA_MULTI_SKIN_CLUSTER", GENERAL, NotRead, ""),
    v("MAYA_NO_HOME", GENERAL, NotRead, "Aztek has no home screen: it starts in the workspace."),
    v("MAYA_NO_HOME_ICON", GENERAL, NotRead, "Aztek has no home icon."),
    v("MAYA_NO_WARNING_FOR_MISSING_DEFAULT_RENDERER", GENERAL, NotRead, ""),
    v("MAYA_PARALLEL_MEMCPY", GENERAL, NotRead, ""),
    v("MAYA_IP_TYPE", GENERAL, NotRead, ""),
    v("MAYA_HELP_URL", GENERAL, NotRead, ""),
    v("MAYA_OVERRIDE_UI", GENERAL, NotRead, ""),
    v("MAYA_PAINT_EFFECTS_THREADS", GENERAL, NotRead, ""),
    v("WINEDITOR", GENERAL, NotRead, ""),
    v("MAYA_SCRIPT_EDITOR_STDOUT", GENERAL, Read, "The Script Editor's lines are also written to standard output."),
    v("MAYA_UI_LANGUAGE", GENERAL, NotRead, "Aztek's interface is in English only."),
    v("MAYA_DISALLOW_DUPLICATE_ATTRIBUTE_NAMES", GENERAL, NotRead, "Aztek always refuses a second attribute of the same name on a node."),
    v("MAYA_USE_ALTERNATE_UNFOLD_SHIM", GENERAL, NotRead, ""),
    v("MAYA_USE_MALLOC", GENERAL, NotRead, ""),
    v("MAYA_USE_VARYING_INT", GENERAL, NotRead, ""),
    v("MAYA_DISABLE_CASCADING", GENERAL, NotRead, "Licensing: Aztek has none."),
    v("MAYA_CER_INCLUDE_SCENE_NAME", GENERAL, NotRead, ""),
    v("MAYA_FORCE_PANEL_FOCUS", GENERAL, NotRead, ""),
    v("MAYA_ENABLE_NG_CONSOLE_STATS", GENERAL, NotRead, ""),
    v("MAYA_LOCK_FOREGROUND_WINDOW", GENERAL, NotRead, ""),
    v("MAYA_DISABLE_PLUGIN_SCENE_MODIFIED_WARNING", GENERAL, NotRead, ""),
    v("MAYA_ENABLE_LEGACY_INCORRECT_NODE_NAMES", GENERAL, NotRead, ""),
    v("MAYA_SKIP_BLOCK_CHECK", GENERAL, NotRead, ""),
    v("MAYA_PYTHON_USER_PACKAGES_PRIORITY", GENERAL, NotRead, ""),
    v("MAYA_WRITE_HIK_V1", GENERAL, NotRead, ""),
    v("MAYA_USER_CER_APP", GENERAL, NotRead, ""),
    v("MAYA_AUTOSEND_CER", GENERAL, NotRead, ""),
    v("MAYA_AUTOSEND_CER_DESCRIPTION", GENERAL, NotRead, ""),
    v("MAYA_SECURE_OPTOUT", GENERAL, NotRead, ""),
    v("MAYA_SKIP_USERSETUP_PY", GENERAL, Read, "userSetup.py files are not run at start. Not on the reference's environment pages; its start-up reads it."),
    v("AW_JPEG_Q_FACTOR", RENDER, Partly, "The quality of JPEG files Harold writes, 1 to 100. Without it Aztek writes at 75."),
    v("AW_JPEG_SUB_SAMPLING", RENDER, NotRead, ""),
    v("MAYA_NO_JITTER_FINAL_COLOR", RENDER, NotRead, ""),
    v("MAYA_RENDERER_RT_BACKGROUND_COLOR", RENDER, NotRead, ""),
    v("MAYA_HW_FILE_TEXTURE_RESOLUTION_OVERRIDE", RENDER, NotRead, ""),
    v("MAYA_OFFSCREEN_HRB", RENDER, NotRead, ""),
    v("MAYA_EXR_LONGNAME", RENDER, NotRead, ""),
    v("MAYA_QUICKTIME_ENCODING_GAMMA", RENDER, NotRead, ""),
    v("OCIO", RENDER, NotRead, ""),
    v("OCIO_ACTIVE_DISPLAYS", RENDER, NotRead, ""),
    v("OCIO_ACTIVE_VIEWS", RENDER, NotRead, ""),
    v("OCIO_INACTIVE_COLORSPACES", RENDER, NotRead, ""),
    v("MAYA_COLOR_MANAGEMENT_POLICY_FILE", RENDER, NotRead, ""),
    v("MAYA_COLOR_MANAGEMENT_POLICY_LOCK", RENDER, NotRead, ""),
    v("MAYA_COLOR_MGT_NO_LOGGING", RENDER, NotRead, ""),
    v("MAYA_ENABLE_DEPRECATED_VIEWPORT", RENDER, NotRead, ""),
    v("MAYA_VP2_DEVICE_OVERRIDE", RENDER, NotRead, "Aztek's view draws with Vulkan only."),
    v("MAYA_ALLOW_OPENGL_REMOTE_SESSION", RENDER, NotRead, ""),
    v("MAYA_VP2_USE_LINEAR_NC_SPACE_DEPTH_PRIORITY", RENDER, NotRead, ""),
    v("MAYA_VP2_OFF_NON_PE_FRAGMENT", RENDER, NotRead, ""),
    v("MAYA_WANT_LEGACY_VP2_DEFAULT_LIGHTING", RENDER, NotRead, ""),
    v("MAYA_FORCE_DX_DEBUG_DEVICE", RENDER, NotRead, ""),
    v("MAYA_OBJECT_SORT_USING_BOUNDING_BOX", RENDER, NotRead, ""),
    v("MAYA_GPUCACHE_VP2_OVERRIDE_API", RENDER, NotRead, ""),
    v("MAYA_FORCE_DX_WARP", RENDER, NotRead, ""),
    v("MAYA_OGS_GPU_MEMORY_LIMIT", RENDER, NotRead, ""),
    v("MAYA_VP2_PAUSE_ON_STARTUP", RENDER, NotRead, ""),
    v("MAYA_ALLOW_RENDER_LAYER_SWITCHING", RENDER, NotRead, ""),
    v("MAYA_BATCH_DISABLE_UNDO_MANAGER_FLUSH_ON_EXIT", RENDER, NotRead, ""),
    v("SHADERFX_CUSTOMUSERPATH", RENDER, NotRead, ""),
    v("MAYA_TEXCOORD_ORIENTATION", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_GLOBAL_TEMPLATE_PATH", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_GLOBAL_PRESETS_PATH", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_USE_UNTITLED_COLLECTIONS", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_INCLUDE_ALL_LIGHTS", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_DISPLAY_RS_NODES", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_DISABLE_RENDER_SETTINGS_AOVS_EXPORT", RENDER, NotRead, ""),
    v("MAYA_RENDER_SETUP_ENABLE_WARNING_ICONS", RENDER, NotRead, ""),
    v("MAYA_WORKSPACES_ALWAYS_SHOW_TAB", LINUX, NotRead, ""),
    v("MAYA_DEBUG_NO_SIGINT_HANDLING", LINUX, NotRead, ""),
    v("__GL_MAYA_OPTIMIZE", LINUX, NotRead, ""),
    v("MAYA_MMSET_DEFAULT_XCURSOR", LINUX, NotRead, ""),
    v("MAYA_SOUND_SCRUB", LINUX, NotRead, ""),
    v("MAYA_SET_PARENT_HINT", LINUX, NotRead, ""),
    v("MAYA_STYLUS_DEVICE", LINUX, NotRead, ""),
    v("MAYA_SET_XERROR", LINUX, NotRead, ""),
    v("MAYA_FORCE_SHOW_ACTIVATE", LINUX, NotRead, ""),
    v("MAYA_ACTIVE_STEREO_REFRESH", LINUX, NotRead, ""),
    v("XLIB_SKIP_ARGB_VISUALS", LINUX, NotRead, ""),
    v("MAYA_TTIP_OFFSETX", LINUX, NotRead, ""),
    v("MAYA_TTIP_OFFSETY", LINUX, NotRead, ""),
    v("MAYA_AUDIO_LIBRARY", AUDIO, NotRead, ""),
    v("MAYA_AUDIO_OPENAL_DEVICE", AUDIO, NotRead, ""),
    v("MAYA_ASCII_ENABLE_BULK_PARSING", ASCII, NotRead, ""),
    v("MAYA_ASCII_SUPPORT_MAC_LINE_ENDINGS", ASCII, NotRead, ""),
    v("AZTEK_APP_DIR", OWN, Read, "Aztek's own user folder, in place of ~/aztek (Documents/aztek on Windows)."),
    v("AZTEK_PREFS", OWN, Read, "Where Aztek keeps its preferences, in place of ~/.aztek."),
    v("AZTEK_CONFIG_DIR", OWN, Read, "Where Aztek keeps its settings, in place of the system's settings folder."),
    v("AZTEK_STUDIO_PREFS", OWN, Read, "Folders of preferences a studio provides."),
    v("AZTEK_LOCATION", OWN, Read, "Set at start to where Aztek is installed."),
    v("AZTEK_MSAA", OWN, Read, "Samples a pixel the view starts with: 1, 2, 4 or 8. Overrides the saved choice."),
    v("AZTEK_UPDATE_URL", OWN, Read, "Where updates are asked about."),
    v("AZTEK_ECHO", OWN, Read, "The Script Editor's lines are also written to standard error."),
    v("AZTEK_PROFILE", OWN, Read, "Timings of the view's drawing are written out."),
];

/// The entry for a name, given in either spelling.
pub fn var(name: &str) -> Option<&'static Var> {
    VARS.iter().find(|x| x.name == name).or_else(|| VARS.iter().find(|x| own_name(x.name).as_deref() == Some(name)))
}

fn status_word(s: Status) -> &'static str {
    match s {
        Read => "yes",
        Partly => "partly",
        NotRead => "no",
    }
}

/// `docs/ENVIRONMENT.md`.
pub fn markdown() -> String {
    let mut out = String::from(
        "# Environment variables\n\n\
Written by `cargo run -p aztek_host --example gen_environment` from `crates/aztek_core/src/env.rs`. Do not edit by hand.\n\n\
The list is the reference's environment pages for 2026, read on 12/11/2026, with Aztek's own variables at the end.\n\n\
## Names\n\n\
A variable spelled `MAYA_X` is read as `AZTEK_X` first and `MAYA_X` second. For a list of folders both are used,\n\
`AZTEK_X`'s folders first. `MAYA_APP_DIR` and `AZTEK_APP_DIR` are the exception: they are two different folders.\n\n\
## The environment file\n\n\
- `Aztek.env`, or `Maya.env`, in the folder `AZTEK_ENV_DIR` or `MAYA_ENV_DIR` names. Without either: the first found\n\
  in the version's folder of the user folder, then in the user folder. One file is read.\n\
- `NAME = value` a line. `#` starts a comment. `$NAME`, `${NAME}` and `%NAME%` are filled in. Folders are separated\n\
  by `:` (`;` on Windows).\n\
- A variable the system already set is left as it is. The file's folders for `MAYA_SCRIPT_PATH`, `MAYA_PLUG_IN_PATH`,\n\
  `MAYA_MODULE_PATH`, `MAYA_SHELF_PATH`, `MAYA_PRESET_PATH`, `XBMLANGPATH`, `PYTHONPATH` and `PATH` are added after\n\
  the ones the system gave. The reference's page says the file replaces the system's value for some of these; how\n\
  the reference combines them has not been checked against the application.\n\
- `MAYA_APP_DIR`, `AZTEK_APP_DIR`, `MAYA_LOCATION`, `AZTEK_LOCATION`, `HOME` and `USERPROFILE` are not taken from\n\
  the file; a warning says so.\n\n\
## In a session\n\n\
`getenv` and `putenv` read and set the process's variables. The path lists are read each time they are used, so a\n\
`putenv` takes effect at once. `aztekEnvironment -q` lists every variable here with its value in force.\n",
    );
    let mut group = "";
    for x in VARS {
        if x.group != group {
            group = x.group;
            out.push_str(&format!("\n## {group}\n\n| Variable | Read | Notes |\n|---|---|---|\n"));
        }
        out.push_str(&format!("| `{}` | {} | {} |\n", x.name, status_word(x.status), x.note));
    }
    let read = VARS.iter().filter(|x| x.group != OWN && x.status != NotRead).count();
    let all = VARS.iter().filter(|x| x.group != OWN).count();
    out.push_str(&format!("\n{read} of the reference's {all} are read.\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_values_and_lists() {
        // One test: the environment is the whole process's.
        std::env::set_var("MAYA_AZ_ENV_T", "theirs");
        assert_eq!(get("MAYA_AZ_ENV_T").as_deref(), Some("theirs"));
        std::env::set_var("AZTEK_AZ_ENV_T", "ours");
        assert_eq!(get("MAYA_AZ_ENV_T").as_deref(), Some("ours"), "Aztek's spelling wins");
        std::env::set_var("AZTEK_AZ_ENV_T", "");
        assert_eq!(get("MAYA_AZ_ENV_T").as_deref(), Some("theirs"), "empty is not set");
        assert!(on("MAYA_AZ_ENV_T"));
        std::env::set_var("MAYA_AZ_ENV_T", "0");
        assert!(!on("MAYA_AZ_ENV_T"));
        assert!(!on("MAYA_AZ_ENV_NOT_THERE"));

        let join = |a: &[&str]| std::env::join_paths(a).unwrap();
        std::env::set_var("MAYA_AZ_ENV_L", join(&["/b", "/c"]));
        std::env::set_var("AZTEK_AZ_ENV_L", join(&["/a", "/b"]));
        assert_eq!(list("MAYA_AZ_ENV_L"), [PathBuf::from("/a"), PathBuf::from("/b"), PathBuf::from("/c")]);

        let dir = std::env::temp_dir().join(format!("aztek_env_icons_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("plain")).unwrap();
        std::fs::create_dir_all(dir.join("pct")).unwrap();
        std::fs::write(dir.join("plain/one.png"), b"x").unwrap();
        std::fs::write(dir.join("pct/two.png"), b"x").unwrap();
        std::env::set_var("XBMLANGPATH", join(&[&dir.join("plain").to_string_lossy(), &format!("{}/%B", dir.join("pct").display())]));
        assert_eq!(icon_dirs(), [dir.join("plain"), dir.join("pct")]);
        assert_eq!(find_icon("one.png"), Some(dir.join("plain/one.png")));
        assert_eq!(find_icon("two.png"), Some(dir.join("pct/two.png")), "an entry ending in /%B is a folder");
        assert_eq!(find_icon("three.png"), None);
        let whole = dir.join("pct/two.png");
        assert_eq!(find_icon(&whole.to_string_lossy()), Some(whole));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_list_names_each_variable_once() {
        let mut names: Vec<&str> = VARS.iter().map(|x| x.name).collect();
        names.sort();
        let before = names.len();
        names.dedup();
        assert_eq!(before, names.len());
        assert_eq!(var("AZTEK_SCRIPT_PATH").map(|x| x.name), Some("MAYA_SCRIPT_PATH"));
        assert!(markdown().contains("| `XBMLANGPATH` | yes |"));
    }
}
