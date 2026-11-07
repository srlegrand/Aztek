//! Layered JSON prefs, built-in defaults < studio folders < user dir <
//! workspace .aztek/.
//!
//! Studio folders (AZTEK_STUDIO_PREFS, or the "prefs" list in studio.json beside the exe; most specific first) hold
//! the same files as the user dir, usually on a network share. Their values are defaults the user can override,
//! except keys listed in a file's "$locked" array (JSON pointers; "/" locks the whole file), which always take the
//! studio value and ignore set(). A local copy keeps them applying while the share is offline.
//!
//! A "file" is a name like "general" or "markingMenus/modeling"; keys are JSON pointers ("/viewport/fov"). Every set()
//! is written to disk shortly after on the IO thread, and edits made to the files outside Aztek are picked up and
//! reported through on_changed. Nothing waits for shutdown.

use std::collections::{BTreeMap, HashMap};
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant, SystemTime};

use serde_json::{json, Value};

use crate::{main_queue, paths};

const WRITE_DELAY: Duration = Duration::from_millis(200);
const POLL_INTERVAL: Duration = Duration::from_millis(250);
/// Studio folders are usually on a network share; checking them every tick would hammer it.
const STUDIO_POLL_INTERVAL: Duration = Duration::from_secs(5);
const LOCKED_KEY: &str = "$locked";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PrefScope {
    #[default]
    User,
    Workspace,
}

const DEFAULTS: usize = 0;
const USER: usize = 1;
const WORKSPACE: usize = 2;
const LAYER_COUNT: usize = 3;

#[derive(Clone)]
struct Doc {
    data: Value,
    loaded: bool,
    dirty: bool,
    dirty_at: Instant,
    mtime: Option<SystemTime>,
    /// Exact text last read or written, to ignore our own writes.
    on_disk: String,
}

impl Default for Doc {
    fn default() -> Self {
        Self { data: json!({}), loaded: false, dirty: false, dirty_at: Instant::now(), mtime: None, on_disk: String::new() }
    }
}

#[derive(Default, Clone)]
struct Layer {
    dir: PathBuf,
    docs: BTreeMap<String, Doc>,
    /// Studio layers: the files the folder held at the last poll, to notice files added or removed.
    files: Vec<String>,
    scanned: bool,
}

impl Layer {
    fn at(dir: PathBuf) -> Self {
        Self { dir, ..Default::default() }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// JSON pointers

fn unescape(token: &str) -> String {
    token.replace("~1", "/").replace("~0", "~")
}

/// A key as a JSON pointer: "" for the whole document.
fn pointer(key: &str) -> String {
    if key.is_empty() || key == "/" {
        String::new()
    } else if key.starts_with('/') {
        key.to_string()
    } else {
        format!("/{key}")
    }
}

fn tokens(ptr: &str) -> Vec<String> {
    if ptr.is_empty() {
        Vec::new()
    } else {
        ptr[1..].split('/').map(unescape).collect()
    }
}

fn get<'a>(v: &'a Value, ptr: &str) -> Option<&'a Value> {
    v.pointer(ptr)
}

/// Sets the value at ptr, creating objects on the way (and growing arrays), as nlohmann's operator[] does.
fn set_at(v: &mut Value, ptr: &str, value: Value) {
    let mut cur = v;
    for t in tokens(ptr) {
        if cur.is_null() {
            *cur = if t.parse::<usize>().is_ok() { json!([]) } else { json!({}) };
        }
        cur = match cur {
            Value::Array(a) => match t.parse::<usize>() {
                Ok(i) => {
                    if a.len() <= i {
                        a.resize(i + 1, Value::Null);
                    }
                    &mut a[i]
                }
                Err(_) => return,
            },
            Value::Object(o) => o.entry(t).or_insert(Value::Null),
            _ => return,
        };
    }
    *cur = value;
}

fn remove_at(v: &mut Value, ptr: &str) -> bool {
    let mut t = tokens(ptr);
    let Some(last) = t.pop() else { return false };
    let parent = t.iter().fold(String::new(), |acc, s| format!("{acc}/{}", s.replace('~', "~0").replace('/', "~1")));
    match v.pointer_mut(&parent) {
        Some(Value::Object(o)) => o.remove(&last).is_some(),
        Some(Value::Array(a)) => match last.parse::<usize>() {
            Ok(i) if i < a.len() => {
                a.remove(i);
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// RFC 7386 merge patch.
pub fn merge_patch(target: &mut Value, patch: &Value) {
    let Value::Object(p) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = json!({});
    }
    let t = target.as_object_mut().unwrap();
    for (k, v) in p {
        if v.is_null() {
            t.remove(k);
        } else {
            merge_patch(t.entry(k.clone()).or_insert(Value::Null), v);
        }
    }
}

/// True when `locked` is `key` or one of its parents ("/fileDialog" locks "/fileDialog/delete").
fn lock_covers(locked: &str, key: &str) -> bool {
    let l = if locked.is_empty() || locked.starts_with('/') { locked.to_string() } else { format!("/{locked}") };
    key == l || l == "/" || l.is_empty() || (key.starts_with(&l) && key.len() > l.len() && key.as_bytes()[l.len()] == b'/')
}

// ---------------------------------------------------------------------------------------------------------------------
// Files

/// Every .json under dir, as pref file names ("shelves/Rigging"), sorted.
fn list_pref_files(dir: &Path) -> Vec<String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) {
        let Ok(rd) = std::fs::read_dir(dir) else { return };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if p.extension().is_some_and(|x| x == "json") {
                if let Ok(rel) = p.with_extension("").strip_prefix(root) {
                    out.push(paths::to_utf8(rel));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn read_text(path: &Path) -> Option<String> {
    std::fs::read(path).ok().map(|b| String::from_utf8_lossy(&b).into_owned())
}

fn mtime(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path).and_then(|m| m.modified()).ok()
}

fn write_atomic(path: &Path, text: &str) -> bool {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    if std::fs::write(&tmp, text).is_err() {
        return false;
    }
    std::fs::rename(&tmp, path).is_ok()
}

/// Strips // and /* */ comments outside strings, as nlohmann's parser does when told to ignore them.
fn strip_comments(text: &str) -> String {
    strip_comments_utf8(text)
}

fn strip_comments_utf8(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let (mut i, mut in_str) = (0, false);
    while i < chars.len() {
        let c = chars[i];
        if in_str {
            out.push(c);
            if c == '\\' && i + 1 < chars.len() {
                out.push(chars[i + 1]);
                i += 2;
                continue;
            }
            if c == '"' {
                in_str = false;
            }
            i += 1;
            continue;
        }
        if c == '"' {
            in_str = true;
        } else if c == '/' && chars.get(i + 1) == Some(&'/') {
            while i < chars.len() && chars[i] != '\n' {
                i += 1;
            }
            continue;
        } else if c == '/' && chars.get(i + 1) == Some(&'*') {
            i += 2;
            while i + 1 < chars.len() && !(chars[i] == '*' && chars[i + 1] == '/') {
                i += 1;
            }
            i += 2;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

fn parse_into(path: &Path, text: &str, d: &mut Doc) -> bool {
    match serde_json::from_str::<Value>(&strip_comments(text)) {
        Ok(v @ Value::Object(_)) => {
            d.data = v;
            true
        }
        _ => {
            log::warn!("prefs: {} is not a valid JSON object, keeping previous values", path.display());
            false
        }
    }
}

/// AZTEK_STUDIO_PREFS (';' separated on Windows, ':' elsewhere), else the "prefs" list of studio.json beside the exe.
fn configured_studio_dirs() -> Vec<PathBuf> {
    if let Some(env) = std::env::var_os("AZTEK_STUDIO_PREFS").filter(|v| !v.is_empty()) {
        return std::env::split_paths(&env).filter(|p| !p.as_os_str().is_empty()).collect();
    }
    let config = paths::executable_dir().join("studio.json");
    let mut dirs = Vec::new();
    if let Some(text) = read_text(&config) {
        if let Ok(j) = serde_json::from_str::<Value>(&strip_comments(&text)) {
            for d in j.get("prefs").and_then(Value::as_array).into_iter().flatten() {
                if let Some(s) = d.as_str() {
                    let dir = PathBuf::from(s);
                    dirs.push(if dir.is_absolute() { dir } else { paths::executable_dir().join(dir) });
                }
            }
        }
    }
    dirs
}

// ---------------------------------------------------------------------------------------------------------------------

pub type ChangeFn = Arc<dyn Fn(&str) + Send + Sync>;

struct Inner {
    layers: [Layer; LAYER_COUNT],
    /// Read-only studio folders, most specific first. They sit between the defaults and the user's prefs.
    studio: Vec<Layer>,
    /// Last good copy of each studio file, so a share that is offline still applies its values and locks.
    studio_cache: PathBuf,
    studio_polled: Instant,
    listeners: BTreeMap<i32, ChangeFn>,
    /// value() results by file and key (None: no layer has it), dropped whenever anything changes.
    memo: HashMap<String, Option<Value>>,
    /// set_defaults, per file, oldest first.
    contributed: BTreeMap<String, Vec<(String, Value)>>,
    next_listener: i32,
    flush_requested: bool,
    stop: bool,
}

struct Shared {
    m: Mutex<Inner>,
    cv: Condvar,
    flushed: Condvar,
}

pub struct PrefStore {
    shared: Arc<Shared>,
    io: Mutex<Option<JoinHandle<()>>>,
}

fn path_for(layer: &Layer, file: &str) -> PathBuf {
    layer.dir.join(format!("{file}.json"))
}

impl Inner {
    fn doc(&mut self, li: usize, file: &str) -> &mut Doc {
        let layer = &mut self.layers[li];
        let dir = layer.dir.clone();
        let d = layer.docs.entry(file.to_string()).or_default();
        if !d.loaded {
            load(&dir, file, d);
        }
        d
    }

    fn studio_cache_path(&self, layer: &Layer, file: &str) -> Option<PathBuf> {
        if self.studio_cache.as_os_str().is_empty() {
            return None;
        }
        let mut h = std::collections::hash_map::DefaultHasher::new();
        paths::to_utf8(&layer.dir).hash(&mut h);
        Some(self.studio_cache.join(h.finish().to_string()).join(format!("{file}.json")))
    }

    fn studio_doc(&mut self, i: usize, file: &str) -> &Doc {
        let loaded = self.studio[i].docs.get(file).is_some_and(|d| d.loaded);
        if !loaded {
            let cache = self.studio_cache_path(&self.studio[i], file);
            let layer = &mut self.studio[i];
            let dir = layer.dir.clone();
            let d = layer.docs.entry(file.to_string()).or_default();
            load_studio(&dir, file, d, cache.as_deref());
        }
        &self.studio[i].docs[file]
    }

    fn is_locked(&mut self, file: &str, key: &str) -> bool {
        for i in 0..self.studio.len() {
            let d = self.studio_doc(i, file);
            let Some(Value::Array(locks)) = d.data.get(LOCKED_KEY) else { continue };
            if locks.iter().any(|l| l.as_str().is_some_and(|l| lock_covers(l, key))) {
                return true;
            }
        }
        false
    }

    fn value_uncached(&mut self, file: &str, key: &str) -> Option<Value> {
        let ptr = pointer(key);
        let locked = self.is_locked(file, &ptr);
        for li in (0..LAYER_COUNT).rev() {
            if li == DEFAULTS {
                for i in 0..self.studio.len() {
                    if let Some(v) = get(&self.studio_doc(i, file).data, &ptr) {
                        return Some(v.clone());
                    }
                }
                if let Some(c) = self.contributed.get(file) {
                    for (_, data) in c.iter().rev() {
                        if let Some(v) = get(data, &ptr) {
                            return Some(v.clone());
                        }
                    }
                }
            }
            if self.layers[li].dir.as_os_str().is_empty() || (locked && li != DEFAULTS) {
                continue;
            }
            if let Some(v) = get(&self.doc(li, file).data, &ptr) {
                return Some(v.clone());
            }
        }
        None
    }

    fn write_dirty(guard: &mut MutexGuard<Inner>, force: bool) {
        let now = Instant::now();
        let mut pending: Vec<(usize, String, PathBuf, String)> = Vec::new();
        for li in USER..LAYER_COUNT {
            let layer = &mut guard.layers[li];
            if layer.dir.as_os_str().is_empty() {
                continue;
            }
            let dir = layer.dir.clone();
            for (file, d) in layer.docs.iter_mut() {
                if !d.dirty || (!force && now - d.dirty_at < WRITE_DELAY) {
                    continue;
                }
                d.dirty = false;
                let text = serde_json::to_string_pretty(&d.data).unwrap_or_default() + "\n";
                d.on_disk = text.clone();
                pending.push((li, file.clone(), dir.join(format!("{file}.json")), text));
            }
        }
        if pending.is_empty() {
            return;
        }
        for (_, _, path, text) in &pending {
            if !write_atomic(path, text) {
                log::error!("prefs: failed to write {}", path.display());
            }
        }
        for (li, file, path, _) in pending {
            if let Some(d) = guard.layers[li].docs.get_mut(&file) {
                d.mtime = mtime(&path);
            }
        }
    }

    fn poll_studio(&mut self, changed: &mut Vec<String>) {
        for i in 0..self.studio.len() {
            if !self.studio[i].dir.is_dir() {
                continue; // offline: keep what was loaded
            }
            let files = list_pref_files(&self.studio[i].dir);
            if self.studio[i].scanned {
                let (a, b) = (&self.studio[i].files, &files);
                for f in a.iter().filter(|f| !b.contains(f)).chain(b.iter().filter(|f| !a.contains(f))) {
                    changed.push(f.clone());
                }
            }
            self.studio[i].files = files;
            self.studio[i].scanned = true;
            let names: Vec<String> = self.studio[i].docs.keys().cloned().collect();
            for file in names {
                let path = path_for(&self.studio[i], &file);
                let mt = mtime(&path);
                let d = self.studio[i].docs.get_mut(&file).unwrap();
                if mt == d.mtime {
                    continue;
                }
                d.mtime = mt;
                let text = read_text(&path);
                if text.clone().unwrap_or_default() == d.on_disk {
                    continue;
                }
                d.on_disk = text.clone().unwrap_or_default();
                match &text {
                    None => d.data = json!({}),
                    Some(t) => {
                        if !parse_into(&path, t, d) {
                            continue;
                        }
                    }
                }
                if let Some(cache) = self.studio_cache_path(&self.studio[i], &file) {
                    match &text {
                        Some(t) => {
                            write_atomic(&cache, t);
                        }
                        None => {
                            let _ = std::fs::remove_file(&cache);
                        }
                    }
                }
                changed.push(file);
            }
        }
    }

    fn poll_external(&mut self) -> Vec<String> {
        let mut changed = Vec::new();
        if self.studio_polled.elapsed() >= STUDIO_POLL_INTERVAL {
            self.studio_polled = Instant::now();
            self.poll_studio(&mut changed);
        }
        for li in USER..LAYER_COUNT {
            let layer = &mut self.layers[li];
            if layer.dir.as_os_str().is_empty() {
                continue;
            }
            let dir = layer.dir.clone();
            for (file, d) in layer.docs.iter_mut() {
                if d.dirty {
                    continue;
                }
                let path = dir.join(format!("{file}.json"));
                let Some(mt) = mtime(&path) else { continue };
                if Some(mt) == d.mtime {
                    continue;
                }
                d.mtime = Some(mt);
                let Some(text) = read_text(&path) else { continue };
                if text == d.on_disk {
                    continue;
                }
                d.on_disk = text.clone();
                if parse_into(&path, &text, d) {
                    changed.push(file.clone());
                }
            }
        }
        if !changed.is_empty() {
            self.memo.clear();
        }
        changed.sort();
        changed.dedup();
        changed
    }
}

fn load(dir: &Path, file: &str, d: &mut Doc) {
    d.loaded = true;
    if dir.as_os_str().is_empty() {
        return;
    }
    let path = dir.join(format!("{file}.json"));
    d.mtime = mtime(&path);
    if let Some(text) = read_text(&path) {
        d.on_disk = text.clone();
        parse_into(&path, &text, d);
    }
}

/// Reads a studio file and refreshes its cached copy; when the folder can't be reached, uses that copy.
fn load_studio(dir: &Path, file: &str, d: &mut Doc, cache: Option<&Path>) {
    d.loaded = true;
    let path = dir.join(format!("{file}.json"));
    if !dir.is_dir() {
        if let Some(c) = cache {
            if let Some(text) = read_text(c) {
                parse_into(c, &text, d);
            }
        }
        return;
    }
    d.mtime = mtime(&path);
    let Some(text) = read_text(&path) else {
        if let Some(c) = cache {
            let _ = std::fs::remove_file(c);
        }
        return;
    };
    d.on_disk = text.clone();
    if parse_into(&path, &text, d) {
        if let Some(c) = cache {
            if read_text(c).as_deref() != Some(text.as_str()) {
                write_atomic(c, &text);
            }
        }
    }
}

impl Shared {
    fn notify(self: &Arc<Self>, file: String) {
        let me = self.clone();
        main_queue::post(move || {
            let copy: Vec<ChangeFn> = me.m.lock().unwrap().listeners.values().cloned().collect();
            for f in copy {
                f(&file);
            }
        });
    }
}

impl PrefStore {
    pub fn get() -> &'static PrefStore {
        static STORE: OnceLock<PrefStore> = OnceLock::new();
        STORE.get_or_init(PrefStore::new)
    }

    fn new() -> Self {
        let mut layers: [Layer; LAYER_COUNT] = Default::default();
        layers[DEFAULTS].dir = paths::resources_dir().join("defaults");
        layers[USER].dir = paths::user_config_dir().join("prefs");
        let studio: Vec<Layer> = configured_studio_dirs().into_iter().map(Layer::at).collect();
        for l in &studio {
            log::info!("prefs: studio folder {}", paths::to_utf8(&l.dir));
        }
        let inner = Inner {
            layers,
            studio,
            studio_cache: paths::user_config_dir().join("studioCache"),
            studio_polled: Instant::now(),
            listeners: BTreeMap::new(),
            memo: HashMap::new(),
            contributed: BTreeMap::new(),
            next_listener: 1,
            flush_requested: false,
            stop: false,
        };
        PrefStore { shared: Arc::new(Shared { m: Mutex::new(inner), cv: Condvar::new(), flushed: Condvar::new() }), io: Mutex::new(None) }
    }

    /// Starts the IO thread. Change callbacks are delivered on the main thread via main_queue.
    pub fn start(&self) {
        let mut io = self.io.lock().unwrap();
        if io.is_some() {
            return;
        }
        self.shared.m.lock().unwrap().stop = false;
        let shared = self.shared.clone();
        *io = Some(
            std::thread::Builder::new()
                .name("Prefs IO".into())
                .spawn(move || {
                    let mut g = shared.m.lock().unwrap();
                    while !g.stop {
                        g = shared.cv.wait_timeout(g, POLL_INTERVAL).unwrap().0;
                        let force = g.flush_requested;
                        Inner::write_dirty(&mut g, force);
                        let changed = g.poll_external();
                        for file in changed {
                            log::info!("prefs: reloaded {file}");
                            shared.notify(file);
                        }
                        if g.flush_requested {
                            g.flush_requested = false;
                            shared.flushed.notify_all();
                        }
                    }
                    Inner::write_dirty(&mut g, true);
                })
                .expect("prefs thread"),
        );
    }

    pub fn stop(&self) {
        let Some(h) = self.io.lock().unwrap().take() else { return };
        self.shared.m.lock().unwrap().stop = true;
        self.shared.cv.notify_all();
        let _ = h.join();
    }

    /// Point the workspace layer at <root>/.aztek, or detach it with an empty path.
    pub fn set_workspace_dir(&self, dir: &Path) {
        let mut g = self.shared.m.lock().unwrap();
        Inner::write_dirty(&mut g, true);
        g.layers[WORKSPACE] = Layer::at(dir.to_path_buf());
        g.memo.clear();
    }

    pub fn value(&self, file: &str, key: &str, fallback: Value) -> Value {
        let mut g = self.shared.m.lock().unwrap();
        let memo_key = format!("{file}|{key}");
        if let Some(v) = g.memo.get(&memo_key) {
            return v.clone().unwrap_or(fallback);
        }
        let found = g.value_uncached(file, key);
        g.memo.insert(memo_key, found.clone());
        found.unwrap_or(fallback)
    }

    pub fn string(&self, file: &str, key: &str, fallback: &str) -> String {
        self.value(file, key, Value::Null).as_str().map(str::to_string).unwrap_or_else(|| fallback.to_string())
    }
    pub fn bool(&self, file: &str, key: &str, fallback: bool) -> bool {
        self.value(file, key, Value::Null).as_bool().unwrap_or(fallback)
    }
    pub fn f64(&self, file: &str, key: &str, fallback: f64) -> f64 {
        self.value(file, key, Value::Null).as_f64().unwrap_or(fallback)
    }
    pub fn i64(&self, file: &str, key: &str, fallback: i64) -> i64 {
        self.value(file, key, Value::Null).as_i64().unwrap_or(fallback)
    }

    /// Whole merged document for a file.
    pub fn merged(&self, file: &str) -> Value {
        let mut g = self.shared.m.lock().unwrap();
        let mut out = json!({});
        // defaults, contributions and studio: what locked keys resolve to
        let mut under = json!({});
        let mut locked = Vec::new();
        for li in 0..LAYER_COUNT {
            if !g.layers[li].dir.as_os_str().is_empty() {
                let d = g.doc(li, file).data.clone();
                merge_patch(&mut out, &d);
            }
            if li != DEFAULTS {
                continue;
            }
            if let Some(c) = g.contributed.get(file) {
                for (_, data) in c.clone() {
                    merge_patch(&mut out, &data);
                }
            }
            for i in (0..g.studio.len()).rev() {
                let mut data = g.studio_doc(i, file).data.clone();
                if let Some(obj) = data.as_object_mut() {
                    if let Some(l) = obj.remove(LOCKED_KEY) {
                        if let Value::Array(a) = l {
                            locked.extend(a);
                        }
                    }
                }
                merge_patch(&mut out, &data);
            }
            under = out.clone();
        }
        for l in locked {
            let Some(l) = l.as_str() else { continue };
            let ptr = pointer(l);
            if ptr.is_empty() {
                out = under.clone();
            } else if let Some(v) = get(&under, &ptr) {
                set_at(&mut out, &ptr, v.clone());
            } else {
                remove_at(&mut out, &ptr);
            }
        }
        out
    }

    pub fn set(&self, file: &str, key: &str, value: Value, scope: PrefScope) {
        {
            let mut g = self.shared.m.lock().unwrap();
            let ptr = pointer(key);
            if g.is_locked(file, &ptr) {
                return;
            }
            let li = if scope == PrefScope::Workspace && !g.layers[WORKSPACE].dir.as_os_str().is_empty() { WORKSPACE } else { USER };
            let d = g.doc(li, file);
            if get(&d.data, &ptr) == Some(&value) {
                return;
            }
            if ptr.is_empty() {
                d.data = value;
            } else {
                set_at(&mut d.data, &ptr, value);
            }
            d.dirty = true;
            d.dirty_at = Instant::now();
            g.memo.clear();
        }
        self.shared.cv.notify_all();
        self.shared.notify(file.to_string());
    }

    pub fn remove(&self, file: &str, key: &str, scope: PrefScope) {
        {
            let mut g = self.shared.m.lock().unwrap();
            let li = if scope == PrefScope::Workspace && !g.layers[WORKSPACE].dir.as_os_str().is_empty() { WORKSPACE } else { USER };
            let ptr = pointer(key);
            let d = g.doc(li, file);
            if !remove_at(&mut d.data, &ptr) {
                return;
            }
            d.dirty = true;
            d.dirty_at = Instant::now();
            g.memo.clear();
        }
        self.shared.cv.notify_all();
        self.shared.notify(file.to_string());
    }

    /// The user dir's own document for a file, without the layers around it.
    pub fn user_doc(&self, file: &str) -> Value {
        self.shared.m.lock().unwrap().doc(USER, file).data.clone()
    }

    /// Replaces the user dir's document for a file; an empty object leaves the file to its defaults.
    pub fn set_user_doc(&self, file: &str, data: Value) {
        {
            let mut g = self.shared.m.lock().unwrap();
            let d = g.doc(USER, file);
            if d.data == data {
                return;
            }
            d.data = if data.is_object() { data } else { json!({}) };
            d.dirty = true;
            d.dirty_at = Instant::now();
            g.memo.clear();
        }
        self.shared.cv.notify_all();
        self.shared.notify(file.to_string());
    }

    /// Defaults contributed at runtime, by extensions and modes: they sit above the built-in defaults and below the
    /// user's, so a user edit still wins and removing them brings the built-ins back. Each is kept under a layer id
    /// ("<owner>:<what>") so it can be taken away again; later layers win over earlier ones. Never written to disk.
    pub fn set_defaults(&self, file: &str, layer: &str, data: Value) {
        {
            let mut g = self.shared.m.lock().unwrap();
            let list = g.contributed.entry(file.to_string()).or_default();
            match list.iter_mut().find(|e| e.0 == layer) {
                Some(e) => e.1 = data,
                None => list.push((layer.to_string(), data)),
            }
            g.memo.clear();
        }
        self.shared.notify(file.to_string());
    }

    /// False when that layer contributed nothing to file.
    pub fn remove_defaults(&self, file: &str, layer: &str) -> bool {
        {
            let mut g = self.shared.m.lock().unwrap();
            let Some(list) = g.contributed.get_mut(file) else { return false };
            let before = list.len();
            list.retain(|e| e.0 != layer);
            let n = before - list.len();
            if list.is_empty() {
                g.contributed.remove(file);
            }
            if n == 0 {
                return false;
            }
            g.memo.clear();
        }
        self.shared.notify(file.to_string());
        true
    }

    /// Removes every contribution whose layer id starts with prefix. Returns the count.
    pub fn remove_defaults_with_prefix(&self, prefix: &str) -> usize {
        let mut files = Vec::new();
        let mut count = 0;
        {
            let mut g = self.shared.m.lock().unwrap();
            g.contributed.retain(|file, list| {
                let before = list.len();
                list.retain(|e| !e.0.starts_with(prefix));
                let n = before - list.len();
                if n > 0 {
                    files.push(file.clone());
                }
                count += n;
                !list.is_empty()
            });
            if count > 0 {
                g.memo.clear();
            }
        }
        for f in files {
            self.shared.notify(f);
        }
        count
    }

    /// Files with contributed defaults whose name starts with prefix ("markingMenus/"), sorted.
    pub fn contributed_files(&self, prefix: &str) -> Vec<String> {
        self.shared.m.lock().unwrap().contributed.keys().filter(|f| f.starts_with(prefix)).cloned().collect()
    }

    /// Whether any layer contributed to file.
    pub fn has_contributed(&self, file: &str) -> bool {
        self.shared.m.lock().unwrap().contributed.contains_key(file)
    }

    /// Block until pending writes hit disk.
    pub fn flush(&self) {
        let running = self.io.lock().unwrap().is_some();
        let mut g = self.shared.m.lock().unwrap();
        if !running {
            Inner::write_dirty(&mut g, true);
            return;
        }
        g.flush_requested = true;
        self.shared.cv.notify_all();
        let _g = self.shared.flushed.wait_while(g, |i| i.flush_requested).unwrap();
    }

    pub fn on_changed(&self, f: ChangeFn) -> i32 {
        let mut g = self.shared.m.lock().unwrap();
        let id = g.next_listener;
        g.next_listener += 1;
        g.listeners.insert(id, f);
        id
    }

    pub fn remove_on_changed(&self, id: i32) {
        self.shared.m.lock().unwrap().listeners.remove(&id);
    }

    pub fn user_dir(&self) -> PathBuf {
        self.shared.m.lock().unwrap().layers[USER].dir.clone()
    }

    /// Whether a studio folder locks this key (or a parent of it).
    pub fn is_locked(&self, file: &str, key: &str) -> bool {
        self.shared.m.lock().unwrap().is_locked(file, &pointer(key))
    }

    /// Replaces the studio folders, most specific first; the environment sets them at startup.
    pub fn set_studio_dirs(&self, dirs: Vec<PathBuf>) {
        let files: Vec<String> = {
            let mut g = self.shared.m.lock().unwrap();
            let files = g.studio.iter().flat_map(|l| l.docs.keys().cloned()).collect();
            g.studio = dirs.into_iter().map(Layer::at).collect();
            g.memo.clear();
            files
        };
        for f in files {
            self.shared.notify(f);
        }
    }

    pub fn studio_dirs(&self) -> Vec<PathBuf> {
        self.shared.m.lock().unwrap().studio.iter().map(|l| l.dir.clone()).collect()
    }

    /// Files the studio folders provide whose name starts with prefix ("shelves/"), sorted, from the local copy when
    /// offline.
    pub fn studio_files(&self, prefix: &str) -> Vec<String> {
        let g = self.shared.m.lock().unwrap();
        let mut out = Vec::new();
        for l in &g.studio {
            let root = if l.dir.is_dir() {
                Some(l.dir.clone())
            } else {
                g.studio_cache_path(l, "x").and_then(|p| p.parent().map(Path::to_path_buf))
            };
            let Some(root) = root.filter(|r| r.is_dir()) else { continue };
            out.extend(list_pref_files(&root).into_iter().filter(|n| n.starts_with(prefix)));
        }
        out.sort();
        out.dedup();
        out
    }

    /// Points the default and user layers at other folders (tests, a portable install).
    pub fn set_layer_dirs(&self, defaults: &Path, user: &Path) {
        let mut g = self.shared.m.lock().unwrap();
        Inner::write_dirty(&mut g, true);
        g.layers[DEFAULTS] = Layer::at(defaults.to_path_buf());
        g.layers[USER] = Layer::at(user.to_path_buf());
        g.memo.clear();
    }
}

impl Drop for PrefStore {
    fn drop(&mut self) {
        self.stop();
    }
}

