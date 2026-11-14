//! What compiled plug-ins' messages (the reference's MDGMessage, MDagMessage, MNodeMessage, MSceneMessage) are made
//! from. While something is watching, the graph notes each change it makes here, with what a listener can no longer
//! look up afterwards (a deleted node's name and type, a renamed node's old name, a broken connection's source). The
//! host takes the notes after each command, at any depth, and calls the plug-ins; it also hears every command start
//! and end, so file commands can send their before and after messages.
//!
//! Nothing is noted and no hook runs while nothing watches.

use std::cell::RefCell;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::OnceLock;

use aztek_ops::{Args, CommandError};

use crate::plug::{NodeId, PlugRef};
use crate::session::Session;

/// A node that has left the graph, as it was.
#[derive(Debug, Clone)]
pub struct Departed {
    pub id: NodeId,
    pub name: String,
    pub ty: String,
    /// Its type and the types it derives from, its own first.
    pub chain: Vec<String>,
    pub dag: bool,
    pub owner: String,
    pub parent: NodeId,
    pub path: String,
    pub full: String,
    /// Gone with the whole scene (a new scene, an open): for good, not to an undo step.
    pub cleared: bool,
}

#[derive(Debug, Clone)]
pub enum Event {
    Added(NodeId),
    Removed(Departed),
    Renamed { id: NodeId, old: String },
    /// A DAG node's parent or place among its siblings changed. The same parent: a reorder.
    Reparented { id: NodeId, old_parent: NodeId, new_parent: NodeId },
    /// A value was set on a plug: a whole attribute, an element, a component.
    Set(PlugRef),
    /// An array's element went.
    ElementRemoved(PlugRef),
    Connected { src: PlugRef, dst: PlugRef, made: bool },
    Locked { plug: PlugRef, locked: bool },
    Keyable { plug: PlugRef, keyable: bool },
    AttrAdded { node: NodeId, attr: usize, name: String },
    AttrRemoved { node: NodeId, name: String, short: String },
    AttrRenamed { node: NodeId, attr: usize, old: String },
    Uuid(NodeId),
}

/// What is watched: the `NOTE_*` kinds the graph notes, and `NOTE_HOOKS` for the command hooks alone.
static WATCHING: AtomicU32 = AtomicU32::new(0);

pub const NOTE_NODES: u32 = 1;
pub const NOTE_NAMES: u32 = 2;
pub const NOTE_DAG: u32 = 4;
pub const NOTE_PLUGS: u32 = 8;
pub const NOTE_CONNECTIONS: u32 = 16;
pub const NOTE_UUIDS: u32 = 32;
pub const NOTE_HOOKS: u32 = 1 << 31;

thread_local! {
    static NOTES: RefCell<Vec<(u64, Event)>> = const { RefCell::new(Vec::new()) };
}

/// Whether anything is watching: the hooks run.
#[inline]
pub fn watching() -> bool {
    WATCHING.load(Ordering::Relaxed) != 0
}

/// Whether the graph notes a kind of change.
#[inline]
pub fn wants(kind: u32) -> bool {
    WATCHING.load(Ordering::Relaxed) & kind != 0
}

/// What the host's watches need: `NOTE_*` kinds, with `NOTE_HOOKS` when anything at all is watched.
pub fn set_watching(mask: u32) {
    WATCHING.store(mask, Ordering::Relaxed);
    if mask == 0 {
        NOTES.with(|n| n.borrow_mut().clear());
    }
}

/// Called by the graph. `graph` is its serial, so one session doesn't hear another's.
pub(crate) fn note(graph: u64, e: Event) {
    NOTES.with(|n| n.borrow_mut().push((graph, e)));
}

/// The notes made for a graph since the last take. Others' are dropped.
pub fn take(graph: u64) -> Vec<Event> {
    NOTES.with(|n| {
        let all = std::mem::take(&mut *n.borrow_mut());
        all.into_iter().filter(|(g, _)| *g == graph).map(|(_, e)| e).collect()
    })
}

pub fn pending() -> bool {
    NOTES.with(|n| !n.borrow().is_empty())
}

/// What the host does around commands. `before` may change the call's args (a check callback picking another file)
/// or refuse it; `after` and `flush` send what the notes hold.
pub struct Hooks {
    pub before: fn(&mut Session, &str, &mut Args) -> Result<(), CommandError>,
    pub after: fn(&mut Session, &str, &Args, bool),
    pub flush: fn(&mut Session),
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

pub fn set_hooks(h: Hooks) {
    let _ = HOOKS.set(h);
}

pub(crate) fn before(s: &mut Session, name: &str, args: &mut Args) -> Result<(), CommandError> {
    match HOOKS.get() {
        Some(h) if watching() => (h.before)(s, name, args),
        _ => Ok(()),
    }
}

pub(crate) fn after(s: &mut Session, name: &str, args: &Args, ok: bool) {
    if let Some(h) = HOOKS.get().filter(|_| watching()) {
        (h.after)(s, name, args, ok);
    }
}

/// Sends what's noted, outside any command: after an undo, from the host's idle.
pub fn flush(s: &mut Session) {
    if let Some(h) = HOOKS.get().filter(|_| watching()) {
        (h.flush)(s);
    }
}
