//! The folder you're working in. Any folder works; Aztek only keeps its own
//! state under <root>/.aztek/, created the first time something needs to be saved there. Having no workspace is a
//! normal state.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::{json, Value};

use crate::prefs::PrefStore;

/// The workspace state folder's name.
pub const STATE_DIR: &str = ".aztek";

type ChangeFn = Arc<dyn Fn() + Send + Sync>;

struct State {
    root: PathBuf,
    listeners: Vec<ChangeFn>,
}

static STATE: Mutex<State> = Mutex::new(State { root: PathBuf::new(), listeners: Vec::new() });

fn weakly_canonical(p: &Path) -> PathBuf {
    if let Ok(c) = p.canonicalize() {
        return c;
    }
    // The longest existing prefix canonical, the rest appended.
    let mut existing = p.to_path_buf();
    let mut rest = Vec::new();
    while !existing.exists() {
        match (existing.file_name().map(|s| s.to_os_string()), existing.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                existing = parent.to_path_buf();
            }
            _ => return crate::paths::normalize(p),
        }
    }
    let mut out = existing.canonicalize().unwrap_or(existing);
    for r in rest.into_iter().rev() {
        out.push(r);
    }
    crate::paths::normalize(&out)
}

pub struct Workspace;

impl Workspace {
    /// Nearest ancestor of path (or path itself) containing .aztek/.
    pub fn detect(path: &Path) -> Option<PathBuf> {
        let mut p = weakly_canonical(path);
        if !p.is_dir() {
            p = p.parent()?.to_path_buf();
        }
        loop {
            if p.join(STATE_DIR).is_dir() {
                return Some(p);
            }
            match p.parent() {
                Some(parent) => p = parent.to_path_buf(),
                None => return None,
            }
        }
    }

    pub fn open(root: &Path) {
        let canon = weakly_canonical(root);
        let listeners = {
            let mut st = STATE.lock().unwrap();
            if canon == st.root {
                return;
            }
            st.root = canon.clone();
            st.listeners.clone()
        };
        // The workspace layer reads .aztek/ if it exists; writing creates it.
        let prefs = PrefStore::get();
        prefs.set_workspace_dir(&canon.join(STATE_DIR));

        let mut recent = prefs.value("general", "/recentFolders", json!([]));
        if !recent.is_array() {
            recent = json!([]);
        }
        let s = crate::paths::to_utf8(&canon);
        let list = recent.as_array_mut().unwrap();
        list.retain(|v| v.as_str() != Some(s.as_str()));
        list.insert(0, Value::String(s.clone()));
        list.truncate(15);
        prefs.set("general", "/recentFolders", recent, crate::prefs::PrefScope::User);

        log::info!("workspace: {s}");
        for f in listeners {
            f();
        }
    }

    pub fn close() {
        let listeners = {
            let mut st = STATE.lock().unwrap();
            if st.root.as_os_str().is_empty() {
                return;
            }
            st.root.clear();
            st.listeners.clone()
        };
        PrefStore::get().set_workspace_dir(Path::new(""));
        for f in listeners {
            f();
        }
    }

    pub fn is_open() -> bool {
        !STATE.lock().unwrap().root.as_os_str().is_empty()
    }

    /// The open workspace's root, None when none is open.
    pub fn root() -> Option<PathBuf> {
        let st = STATE.lock().unwrap();
        (!st.root.as_os_str().is_empty()).then(|| st.root.clone())
    }

    /// <root>/.aztek, created on demand. None when no workspace is open.
    pub fn state_dir(create: bool) -> Option<PathBuf> {
        let dir = Self::root()?.join(STATE_DIR);
        if create {
            let _ = std::fs::create_dir_all(&dir);
        }
        Some(dir)
    }

    pub fn on_changed(f: ChangeFn) {
        STATE.lock().unwrap().listeners.push(f);
    }
}
