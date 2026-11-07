//! The stages proxy shapes show. A stage is not thread safe and is not a scene value, so stages live here on the main
//! thread, found through the node's uuid, and the node holds only what's needed to open one again: its attributes.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use aztek_scene::graph::Graph;
use aztek_scene::plug::{NodeId, PlugValue};
use aztek_scene::session::Session;
use aztek_undo::FnAction;
use openusd::sdf;
use openusd::usd::{CommittedChange, InitialLoadSet, Stage, StageSink, UndoStage};

use crate::util::*;

/// How many edits a stage can step back through; the undo stack's own limit is what users meet first.
const UNDO_DEPTH: usize = 4096;

pub struct Entry {
    pub stage: Stage,
    /// Records the inverse of each edit; `undo` reverts the newest.
    undo: UndoStage,
    /// Records every edit too, including an undo's revert: the top of this stack is what a redo replays.
    redo: UndoStage,
    pub id: i64,
    pub file: String,
    pub session_file: PathBuf,
    /// Moves on every composed change.
    pub version: Rc<Cell<u64>>,
    /// Paths changed since the viewer last looked: resynced and info-only.
    pub changes: Rc<RefCell<Changes>>,
    /// Layers refusing edits, by identifier. A layer's own permission isn't kept by the crate's stage.
    pub locked: RefCell<Vec<String>>,
    pub session_identifier: String,
    /// Layers edited since they were read or saved. The crate's own `is_dirty` only means an edit is in flight.
    pub dirty: Rc<RefCell<Vec<String>>>,
}

/// What changed, by the version it changed at, for whoever is following the stage. Past a limit the log gives up and says
/// everything did.
#[derive(Default)]
pub struct Changes {
    pub log: Vec<(u64, sdf::Path)>,
    pub everything_at: u64,
}

const CHANGE_LOG_LIMIT: usize = 20000;

impl Changes {
    fn note(&mut self, version: u64, paths: &[sdf::Path]) {
        self.log.extend(paths.iter().map(|p| (version, p.clone())));
        if self.log.len() > CHANGE_LOG_LIMIT {
            self.log.clear();
            self.everything_at = version;
        }
    }
}

struct Watch {
    version: Rc<Cell<u64>>,
    changes: Rc<RefCell<Changes>>,
    dirty: Rc<RefCell<Vec<String>>>,
}

impl Watch {
    fn bump(&self) {
        self.version.set(self.version.get() + 1);
    }
}

impl StageSink for Watch {
    fn after_commit(&self, _stage: &Stage, change: &CommittedChange<'_>) {
        self.bump();
        {
            let mut d = self.dirty.borrow_mut();
            for (id, _) in change.layer_changes {
                if !d.iter().any(|x| x == id) {
                    d.push(id.clone());
                }
            }
        }
        let v = self.version.get();
        let mut c = self.changes.borrow_mut();
        c.note(v, change.resynced);
        c.note(v, change.changed_info_only);
        c.note(v, change.asset_paths_resynced);
    }
    fn layer_muting_changed(&self, _stage: &Stage, _layer: &str, _muted: bool, _resynced: &[sdf::Path]) {
        self.bump();
        self.changes.borrow_mut().everything_at = self.version.get();
    }
    fn load_rules_changed(&self, _stage: &Stage, resynced: &[sdf::Path]) {
        self.bump();
        let v = self.version.get();
        self.changes.borrow_mut().note(v, resynced);
    }
    fn edit_target_changed(&self, _stage: &Stage) {
        self.version.set(self.version.get());
    }
}

thread_local! {
    static BY_NODE: RefCell<HashMap<String, Rc<Entry>>> = RefCell::new(HashMap::new());
    static BY_FILE: RefCell<HashMap<String, Rc<Entry>>> = RefCell::new(HashMap::new());
    static NEXT_ID: Cell<i64> = const { Cell::new(1) };
}

/// Stages live per thread; the file names they take are shared by the whole process.
fn next_seq() -> u64 {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
}

fn temp_session_file(text: &str) -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!("aztek_usd_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let n = next_seq();
    let path = dir.join(format!("session{n}.usda"));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(path)
}

fn make_entry(stage: Stage, file: String, session_file: PathBuf) -> Rc<Entry> {
    let version = Rc::new(Cell::new(1));
    let changes = Rc::new(RefCell::new(Changes::default()));
    let dirty = Rc::new(RefCell::new(Vec::new()));
    stage.add_sink(Watch { version: version.clone(), changes: changes.clone(), dirty: dirty.clone() });
    let id = NEXT_ID.with(|c| {
        c.set(c.get() + 1);
        c.get()
    });
    let session_identifier = stage.session_layer().map(|l| l.identifier().to_string()).unwrap_or_default();
    Rc::new(Entry {
        undo: UndoStage::with_capacity(stage.clone(), UNDO_DEPTH),
        redo: UndoStage::with_capacity(stage.clone(), UNDO_DEPTH * 2),
        stage,
        id,
        file,
        session_file,
        version,
        changes,
        locked: RefCell::new(Vec::new()),
        session_identifier,
        dirty,
    })
}

/// Opens a stage with a session layer of the text given. An empty file makes an anonymous root layer.
pub fn open_entry(file: &str, session_text: &str, load_payloads: bool) -> Result<Rc<Entry>, String> {
    let session = if session_text.is_empty() { "#usda 1.0\n" } else { session_text };
    let session_file = temp_session_file(session)?;
    let b = Stage::builder()
        .schema_registry(openusd_schemas::schema_registry())
        .resolver(crate::extras::AztekResolver::new())
        .load(if load_payloads { InitialLoadSet::LoadAll } else { InitialLoadSet::LoadNone })
        .session_layer(session_file.to_string_lossy().into_owned());
    let stage = if file.is_empty() {
        let n = next_seq();
        b.in_memory(format!("anon_root_{n}.usda")).map_err(|e| e.to_string())?
    } else {
        b.open(file).map_err(|e| format!("{file}: {e}"))?
    };
    for e in stage.composition_errors() {
        log::warn!("USD: {e}");
    }
    Ok(make_entry(stage, file.to_string(), session_file))
}

/// Whether a node is a proxy shape of either name.
pub fn is_proxy(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a(crate::proxy::BASE_TYPE))
}

/// The proxy a name stands for: itself, or the shape under a transform.
pub fn proxy_named(g: &Graph, name: &str) -> Option<NodeId> {
    let id = g.find(name).ok().flatten()?;
    if is_proxy(g, id) {
        return Some(id);
    }
    g.children(id).iter().copied().find(|&c| is_proxy(g, c))
}

/// Every proxy shape in the scene.
pub fn all_proxies(g: &Graph) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&n| is_proxy(g, n)).collect()
}

fn key_of(g: &Graph, shape: NodeId) -> String {
    g.node(shape).map_or_else(String::new, |n| {
        let u = n.uuid();
        if u.is_empty() {
            format!("id{shape}")
        } else {
            u.to_string()
        }
    })
}

/// The stage a proxy shows, opened on first use and again when its file changes.
pub fn entry_of(g: &mut Graph, shape: NodeId) -> Result<Rc<Entry>, String> {
    let key = key_of(g, shape);
    let file = get_str(g, shape, "filePath").unwrap_or_default();
    let existing = BY_NODE.with(|m| m.borrow().get(&key).cloned());
    if let Some(e) = &existing {
        if e.file == file {
            return Ok(e.clone());
        }
    }
    let share = get_f64(g, shape, "shareStage").unwrap_or(1.0) != 0.0 && !file.is_empty();
    let load = get_f64(g, shape, "loadPayloads").unwrap_or(1.0) != 0.0;
    let file_key = format!("{file}|{load}");
    let shared = if share { BY_FILE.with(|m| m.borrow().get(&file_key).cloned()) } else { None };
    let entry = match shared {
        Some(e) => e,
        None => {
            let session = get_str(g, shape, "serializedSessionLayer").unwrap_or_default();
            let e = open_entry(&file, &session, load)?;
            let dirty = get_str(g, shape, "serializedDirtyLayers").unwrap_or_default();
            crate::layers::restore_dirty(&e, &dirty);
            if share {
                BY_FILE.with(|m| m.borrow_mut().insert(file_key, e.clone()));
            }
            e
        }
    };
    BY_NODE.with(|m| m.borrow_mut().insert(key, entry.clone()));
    crate::layers::restore_state(g, shape, &entry);
    // What the node reports. Not an undoable change: it says what is open, not what the user asked for.
    let (id, root, sess) = (entry.id, entry.stage.root_layer().identifier().to_string(), entry.session_identifier.clone());
    g.without_undo(|g| {
        set_attr(g, shape, "stageCacheId", PlugValue::Int(id));
        set_attr(g, shape, "rootLayerName", PlugValue::String(root));
        set_attr(g, shape, "sessionLayerName", PlugValue::String(sess));
    });
    Ok(entry)
}

/// Forgets every stage: a new scene starts over.
pub fn forget_all() {
    BY_NODE.with(|m| m.borrow_mut().clear());
    BY_FILE.with(|m| m.borrow_mut().clear());
}

/// A stage opened by a script, not shown by a proxy unless one opens the same file. Opening a file twice gives one stage.
pub fn standalone(file: &str, load: bool) -> Result<Rc<Entry>, String> {
    let key = format!("{file}|{load}");
    if !file.is_empty() {
        if let Some(e) = BY_FILE.with(|m| m.borrow().get(&key).cloned()) {
            return Ok(e);
        }
    }
    let e = open_entry(file, "", load)?;
    if file.is_empty() {
        BY_FILE.with(|m| m.borrow_mut().insert(format!("anon{}", e.id), e.clone()));
    } else {
        BY_FILE.with(|m| m.borrow_mut().insert(key, e.clone()));
    }
    Ok(e)
}

/// All open stages, each once.
pub fn open_entries() -> Vec<Rc<Entry>> {
    let mut out: Vec<Rc<Entry>> = Vec::new();
    BY_FILE.with(|m| {
        for e in m.borrow().values() {
            if !out.iter().any(|x| Rc::ptr_eq(x, e)) {
                out.push(e.clone());
            }
        }
    });
    BY_NODE.with(|m| {
        for e in m.borrow().values() {
            if !out.iter().any(|x| Rc::ptr_eq(x, e)) {
                out.push(e.clone());
            }
        }
    });
    out
}

pub fn entry_by_id(id: i64) -> Option<Rc<Entry>> {
    open_entries().into_iter().find(|e| e.id == id)
}

/// The proxy's time in the stage's time codes: its time plug when something drives it, else the scene's frame.
pub fn time_of(g: &mut Graph, shape: NodeId) -> f64 {
    if let Some(p) = plug_of(g, shape, "time") {
        if g.source(&p).is_some() {
            if let Some(v) = g.value(&p).ok().and_then(|v| v.scalar()) {
                return v;
            }
        }
    }
    aztek_scene::anim::frame(g)
}

impl Entry {
    pub fn is_dirty(&self, layer: &str) -> bool {
        self.dirty.borrow().iter().any(|x| x == layer)
    }

    pub fn set_clean(&self, layer: &str) {
        self.dirty.borrow_mut().retain(|x| x != layer);
    }

    /// Runs edits as one undoable step of the scene's undo stack. Whatever `f` authors on the stage goes into the step;
    /// an error still records what got done before it, since it did happen.
    pub fn edit<R>(self: &Rc<Self>, s: &mut Session, label: &str, f: impl FnOnce(&Stage) -> Result<R, String>) -> Result<R, String> {
        let before = self.undo.undo_depth();
        let r = f(&self.stage);
        let steps = self.undo.undo_depth().saturating_sub(before);
        if steps > 0 {
            self.record(&mut s.graph, label, steps);
        }
        r
    }

    fn record(self: &Rc<Self>, g: &mut Graph, _label: &str, steps: usize) {
        let (a, b) = (self.clone(), self.clone());
        g.undo_stack_mut().record(FnAction::new(
            move |_g: &mut Graph| {
                for _ in 0..steps {
                    if let Err(e) = a.undo.undo() {
                        log::warn!("USD: undo failed: {e}");
                    }
                }
            },
            move |_g: &mut Graph| {
                for _ in 0..steps {
                    if let Err(e) = b.redo.undo() {
                        log::warn!("USD: redo failed: {e}");
                    }
                }
            },
        ));
    }

    /// Edits made outside commands, as Python does, join the scene's undo stack here.
    pub fn sweep(self: &Rc<Self>, g: &mut Graph, known_depth: usize) -> usize {
        let now = self.undo.undo_depth();
        if now > known_depth {
            self.record(g, "USD", now - known_depth);
        }
        now
    }

    pub fn depth(&self) -> usize {
        self.undo.undo_depth()
    }

    /// Refuses edits to a locked layer, as the reference's USD plug-in does. Returns the identifier of the edit target's layer.
    pub fn check_target(&self) -> Result<String, String> {
        let id = self.stage.edit_target().layer_identifier().to_string();
        if self.locked.borrow().iter().any(|x| *x == id) {
            return Err(format!("layer {id} is locked"));
        }
        if self.stage.is_layer_muted(&id) {
            return Err(format!("layer {id} is muted"));
        }
        Ok(id)
    }
}
