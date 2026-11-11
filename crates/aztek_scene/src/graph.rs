//! The scene's dependency graph plus its transform hierarchy.
//!
//! Shaped by ownership:
//! - `Graph::current()` is gone: the graph is an owned value.
//! - The undo stack lives in the graph (`UndoStack<Graph>`); `undo()`/`redo()` swap it out while it replays.
//! - Hooks and listeners are `Arc<dyn Fn>` called with the graph as an argument instead of capturing it.
//! - `Batch` is `begin_batch`/`end_batch` (or `batch(|g| ..)`), since an RAII guard would hold `&mut Graph`.
//! - The parallel evaluation takes each level's nodes out of the map, computes them on the rayon pool, and puts them
//!   back, instead of sharing raw pointers.

use std::any::Any;
use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use aztek_undo::{FnAction, UndoAction, UndoShared, UndoStack};
use glam::{DMat4, DVec3};
use rayon::prelude::*;

use crate::error::{GraphError, Result};
use crate::naming::{apply_case, NamingMode, NamingPolicy, Underscores};
use crate::node::{registered, EvalContext, Node, NodeType};
use crate::plug::{
    default_for, flags, from_json, to_json, type_name, AttrDef, NodeId, PlugRef, PlugType,
    PlugValue, NO_NODE,
};
use crate::transform::Transform;

/// Cached playback on or off, and the memory it may use in MB (0: half the machine's): the general prefs' keys.
pub const CACHED_PLAYBACK_PREF: &str = "/cachedPlayback";
pub const CACHED_PLAYBACK_MEMORY_PREF: &str = "/cachedPlaybackMemoryMB";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Created,
    Deleted,
    Renamed,
    Reparented,
    Value,
    Connection,
    Selection,
    Cleared,
    /// Many changes at once (a batch ended): anything may have changed. Sent only to batched listeners.
    Reset,
    /// A node gained or lost a user attr.
    Attrs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectMode {
    Replace,
    Add,
    Remove,
    Toggle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrivenEdit {
    /// The connection wins.
    Reject,
    /// The hook changed the source (set a key, say).
    Handled,
    /// Keep the given value until the time or the source changes.
    Hold,
}

/// Decides what a set on a driven scalar plug does. `src` is the plug driving `dst`.
pub type DrivenSetHook =
    Arc<dyn Fn(&mut Graph, &PlugRef, &PlugRef, f64) -> DrivenEdit + Send + Sync>;
/// The same for a whole Vec3 driven by a Vec3 source (a rotation curve).
pub type DrivenVecSetHook =
    Arc<dyn Fn(&mut Graph, &PlugRef, &PlugRef, DVec3) -> DrivenEdit + Send + Sync>;
/// Called by the outermost `delete_node` with the node and its descendants before anything is disconnected.
pub type DeleteHook = Arc<dyn Fn(&mut Graph, &[NodeId]) + Send + Sync>;
/// Called by `evaluate` before computing anything dirty (`after == false`) and once done (`after == true`).
pub type EvaluateHook = Arc<dyn Fn(&mut Graph, bool) + Send + Sync>;
/// `immediate` listeners hear every change, even inside a batch; others get one `Change::Reset` when it ends.
/// `Graph::changed_plug()` names the plug during a Value or Connection call.
pub type Listener = Arc<dyn Fn(&Graph, Change, NodeId) + Send + Sync>;
/// The active space's world frame for a DAG node (spaces are not implemented yet; they supply this).
pub type ActiveFrameFn = Arc<dyn Fn(&Node) -> Option<DMat4> + Send + Sync>;

#[derive(Default, Clone)]
struct Relations {
    parent: NodeId,
    children: Vec<NodeId>,
}

#[derive(Clone)]
struct AttrState {
    def: AttrDef,
    value: PlugValue,
    elements: Vec<(i32, PlugValue)>,
    lock: u8,
}

struct ListenerEntry {
    f: Listener,
    immediate: bool,
}

#[derive(Clone)]
struct PlaybackEntry {
    values: Vec<(usize, PlugValue)>,
    elements: Option<Vec<Vec<(i32, PlugValue)>>>,
    extra: Option<Arc<dyn Any + Send + Sync>>,
}

#[derive(Default)]
struct PlaybackInner {
    times: BTreeMap<i64, HashMap<NodeId, PlaybackEntry>>,
    /// Meshes already counted in bytes; the cache keeps them alive, so their addresses stay unique.
    counted: HashSet<usize>,
    bytes: usize,
    limit: usize,
}

struct PlaybackCache {
    inner: Mutex<PlaybackInner>,
}

fn time_key(seconds: f64) -> i64 {
    (seconds * 1e6).round() as i64
}

thread_local! {
    static DEFER_MESHES: Cell<i32> = const { Cell::new(0) };
}

/// While one is alive on this thread, `evaluate()` leaves meshes dirty, with whatever only they need (skins,
/// deformers), computing the transforms around them. For interactive edits that read joints many times per mouse move.
pub struct DeferMeshes(());

impl DeferMeshes {
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        DEFER_MESHES.with(|d| d.set(d.get() + 1));
        DeferMeshes(())
    }
}

impl Drop for DeferMeshes {
    fn drop(&mut self) {
        DEFER_MESHES.with(|d| d.set(d.get() - 1));
    }
}

fn next_structure() -> u64 {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

/// The attr part of a plug's name: attr, attr[3], or group[3].child for a grouped array's child.
fn attr_part(a: &AttrDef, p: &PlugRef) -> String {
    if p.component >= 0 && p.element >= 0 {
        return format!("{}[{}].{}", a.name, p.element, a.child_names[p.component as usize]);
    }
    if p.component >= 0 {
        return a.child_names[p.component as usize].clone();
    }
    if p.element < 0 {
        return a.name.clone();
    }
    // The reference's name: the object group's index is the second, under instance 0.
    if a.name == "instObjGroups.objectGroups" {
        return format!("instObjGroups[0].objectGroups[{}]", p.element);
    }
    let index = format!("[{}]", p.element);
    match a.name.find('.') {
        None => format!("{}{index}", a.name),
        Some(dot) => format!("{}{index}{}", &a.name[..dot], &a.name[dot..]),
    }
}

/// A whole non-negative number, or None.
fn parse_index(digits: &str) -> Option<i32> {
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    digits.parse().ok()
}

fn lock_bits(a: &AttrDef, component: i32) -> u8 {
    if a.ty != PlugType::Vec3 {
        return 1;
    }
    if component >= 0 {
        1u8 << component
    } else {
        7
    }
}

fn measure_value(v: &PlugValue, shared: &mut UndoShared, owned: &mut usize) {
    match v {
        PlugValue::Mesh(Some(m)) => {
            shared.add(Arc::as_ptr(m) as *const () as usize, m.memory_bytes())
        }
        PlugValue::Data(Some(d)) => shared.add(Arc::as_ptr(d) as *const () as usize, d.bytes()),
        PlugValue::String(s) => *owned += s.capacity(),
        _ => {}
    }
}

fn vec3_component(v: &PlugValue, c: i32) -> f64 {
    v.as_vec3()[c as usize]
}

/// v as def's type: through JSON where that reads, else by its first number, else def's default.
fn convert_value(v: &PlugValue, def: &AttrDef) -> PlugValue {
    if v.holds(def.ty) {
        return v.clone();
    }
    let j = to_json(v);
    let mut numeric = true;
    let number = if let PlugValue::String(s) = v {
        // strtod: the longest leading number.
        let t = s.trim_start();
        let mut best = None;
        for end in (1..=t.len()).rev() {
            if t.is_char_boundary(end) {
                if let Ok(d) = t[..end].parse::<f64>() {
                    best = Some(d);
                    break;
                }
            }
        }
        numeric = best.is_some();
        best.unwrap_or(0.0)
    } else if let Some(first) = j
        .as_array()
        .and_then(|a| a.first())
        .and_then(|x| x.as_f64())
    {
        first
    } else if let Some(b) = j.as_bool() {
        if b {
            1.0
        } else {
            0.0
        }
    } else if let Some(d) = j.as_f64() {
        d
    } else {
        numeric = false;
        0.0
    };
    if def.ty == PlugType::String {
        let text = |d: f64| {
            // printf %g
            let s = format!("{d}");
            if s.len() > 12 {
                format!("{d:e}")
            } else {
                s
            }
        };
        if let PlugValue::Vec3(x) = v {
            return PlugValue::String(format!("{} {} {}", text(x.x), text(x.y), text(x.z)));
        }
        if let PlugValue::Bool(b) = v {
            return PlugValue::String(if *b { "true" } else { "false" }.into());
        }
        if numeric {
            return PlugValue::String(text(number));
        }
    }
    let mut tries = Vec::new();
    if numeric {
        if matches!(def.ty, PlugType::Int | PlugType::Enum) {
            tries.push(serde_json::json!(number.round() as i64));
        } else {
            tries.push(serde_json::json!(number));
        }
        if def.ty == PlugType::Vec3 {
            tries.push(serde_json::json!([number, number, number]));
        }
    }
    tries.push(j);
    for t in &tries {
        if let Ok(out) = from_json(def, t) {
            if let PlugValue::Int(e) = out {
                if def.ty == PlugType::Enum && (e < 0 || e >= def.enum_names.len() as i64) {
                    continue;
                }
            }
            return out;
        }
    }
    def.default_value.clone()
}

/// Holds a deleted node so undo can put it back.
struct DeleteAction {
    id: NodeId,
    parent: NodeId,
    index: usize,
    held: Option<Box<Node>>,
    /// Recorded for a create: undo removes, redo restores.
    inverted: bool,
}

impl DeleteAction {
    fn restore(&mut self, g: &mut Graph) {
        if let Some(n) = self.held.take() {
            g.attach(n, self.parent, self.index);
        }
    }
    fn remove(&mut self, g: &mut Graph) {
        self.held = g.detach(self.id).map(|(n, _)| n);
    }
}

impl UndoAction<Graph> for DeleteAction {
    fn undo(&mut self, g: &mut Graph) {
        if self.inverted {
            self.remove(g)
        } else {
            self.restore(g)
        }
    }
    fn redo(&mut self, g: &mut Graph) {
        if self.inverted {
            self.restore(g)
        } else {
            self.remove(g)
        }
    }
    fn bytes(&self) -> usize {
        let mut n = std::mem::size_of::<Self>();
        if let Some(h) = &self.held {
            let mut ignored = UndoShared::default();
            n += std::mem::size_of::<Node>() + h.name.capacity();
            for v in &h.data.values {
                measure_value(v, &mut ignored, &mut n);
            }
        }
        n
    }
    fn shared_bytes(&self, shared: &mut UndoShared) {
        if let Some(h) = &self.held {
            let mut ignored = 0;
            for v in &h.data.values {
                measure_value(v, shared, &mut ignored);
            }
        }
    }
}

/// Everything the evaluation of one level reads, borrowed from the graph while that level's nodes are out of it.
struct EvalView<'a> {
    nodes: &'a IdMap<Box<Node>>,
    rel: &'a IdMap<Relations>,
    inputs: &'a BTreeMap<PlugRef, PlugRef>,
    held: &'a BTreeSet<PlugRef>,
    display_turns: &'a HashMap<NodeId, DMat4>,
    turned: &'a HashSet<NodeId>,
    playback: Option<&'a PlaybackCache>,
    active_frame: Option<&'a ActiveFrameFn>,
    ctx: EvalContext,
    pass: u64,
    cache: bool,
    /// Values of source plugs that are plain inputs: read here, since their node may be computing beside this one.
    passed: &'a HashMap<PlugRef, PlugValue>,
}

/// An attr a node only reads: its value doesn't depend on the node computing, so reading it needs no ordering against
/// the node, only against what drives the attr. This is what lets `pCube1.tx -> multiplyDivide1 -> pCube1.ty` run,
/// which is no cycle in the reference.
fn passes_through(a: &AttrDef) -> bool {
    a.has(flags::MESSAGE) || (a.has(flags::INPUT) && !a.has(flags::OUTPUT))
}

/// parentMatrix or parentInverseMatrix on a DAG node: they follow from the parent alone, so a node's own inputs may
/// depend on them. `joint.pim -> multMatrix -> decomposeMatrix -> joint.t` is no cycle in the reference.
fn from_parent(n: &Node, attr: i32) -> bool {
    n.data.ty.dag && (attr as usize == Transform::PARENT_MATRIX || attr as usize == crate::transform::DagNode::PARENT_INVERSE_MATRIX)
}

/// The plugs feeding this plug directly: its own source, its whole Vec3's or its components'.
fn feeds_of(inputs: &BTreeMap<PlugRef, PlugRef>, p: &PlugRef) -> Vec<PlugRef> {
    let mut out = Vec::new();
    if let Some(s) = inputs.get(p) {
        out.push(*s);
    }
    if p.component >= 0 {
        if let Some(s) = inputs.get(&PlugRef { component: -1, ..*p }) {
            out.push(*s);
        }
    } else {
        for c in 0..3 {
            if let Some(s) = inputs.get(&PlugRef { component: c, ..*p }) {
                out.push(*s);
            }
        }
    }
    out
}

/// An input of a DAG node that nothing it computes uses: an attribute added to it, its visibility and display settings. The reference's
/// worldMatrix doesn't depend on these, so what drives them may depend on the node. They are filled in once everything has computed.
fn computes_nothing(ty: &NodeType, attr: i32) -> bool {
    let added = ty.attrs[attr as usize].has(flags::USER);
    (ty.dag && (added || ((attr as usize) < crate::transform::DagNode::COUNT && attr as usize != Transform::PARENT_MATRIX))) || (added && ty.ignores_added_attrs)
}

/// transMinusRotatePivot on a transform: what is added to a position on a motion path to give translate, so it follows from the pivots
/// alone. `motionPath.xc + obj.tmrx -> obj.tx` is no cycle in the reference.
fn from_pivots(n: &Node, attr: i32) -> bool {
    attr as usize == Transform::TRANS_MINUS_ROTATE_PIVOT && n.data.ty.is_a("transform")
}

fn pivot_plugs(p: &PlugRef) -> [PlugRef; 2] {
    [PlugRef { attr: Transform::ROTATE_PIVOT as i32, ..*p }, PlugRef { attr: Transform::ROTATE_PIVOT_TRANSLATE as i32, ..*p }]
}

/// The nodes a read of `p` has to wait for: p's own node for an output, whatever drives p for a plain input.
fn read_after(nodes: &IdMap<Box<Node>>, rel: &IdMap<Relations>, inputs: &BTreeMap<PlugRef, PlugRef>, p: &PlugRef, out: &mut Vec<NodeId>, depth: u32) {
    let Some(n) = nodes.get(&p.node) else { return };
    if from_parent(n, p.attr) {
        if let Some(r) = rel.get(&p.node).filter(|r| r.parent != NO_NODE) {
            out.push(r.parent);
        }
        return;
    }
    if from_pivots(n, p.attr) {
        for q in pivot_plugs(p) {
            read_after(nodes, rel, inputs, &q, out, depth + 1);
        }
        return;
    }
    let pass = n.data.ty.attrs.get(p.attr as usize).is_some_and(passes_through);
    if !pass || depth > 64 {
        out.push(p.node);
        return;
    }
    for s in feeds_of(inputs, p) {
        read_after(nodes, rel, inputs, &s, out, depth + 1);
    }
}

fn stored_plug(n: &Node, p: &PlugRef) -> PlugValue {
    let whole = if p.element >= 0 {
        n.element(p.attr as usize, p.element).cloned().unwrap_or_else(|| n.ty().attrs[p.attr as usize].default_value.clone())
    } else {
        n.value(p.attr as usize).clone()
    };
    if p.component >= 0 {
        PlugValue::Float(vec3_component(&whole, p.component))
    } else {
        whole
    }
}

/// A plain input's value as its node would pull it, following what drives it. Its drivers have already computed.
fn resolve_input(nodes: &IdMap<Box<Node>>, rel: &IdMap<Relations>, inputs: &BTreeMap<PlugRef, PlugRef>, held: &BTreeSet<PlugRef>, p: &PlugRef, depth: u32) -> PlugValue {
    let Some(n) = nodes.get(&p.node) else { return PlugValue::Float(0.0) };
    if from_parent(n, p.attr) {
        let parent = rel.get(&p.node).map_or(NO_NODE, |r| r.parent);
        let world = nodes.get(&parent).map_or(DMat4::IDENTITY, |up| up.matrix(Transform::WORLD_MATRIX));
        let inverse = p.attr as usize != Transform::PARENT_MATRIX;
        return PlugValue::Matrix(if !inverse || world.determinant().abs() < 1e-300 { if inverse { DMat4::IDENTITY } else { world } } else { world.inverse() });
    }
    if from_pivots(n, p.attr) && depth <= 64 {
        let [a, b] = pivot_plugs(p).map(|q| resolve_input(nodes, rel, inputs, held, &q, depth + 1));
        return match (a, b) {
            (PlugValue::Float(x), PlugValue::Float(y)) => PlugValue::Float(-(x + y)),
            (x, y) => PlugValue::Vec3(-(x.as_vec3() + y.as_vec3())),
        };
    }
    let a = &n.data.ty.attrs[p.attr as usize];
    if !passes_through(a) || depth > 64 {
        return stored_plug(n, p);
    }
    let read = |s: &PlugRef| resolve_input(nodes, rel, inputs, held, s, depth + 1);
    let mut v = stored_plug(n, p);
    if !held.contains(p) {
        if let Some(s) = inputs.get(p) {
            let sv = read(s);
            if std::mem::discriminant(&v) == std::mem::discriminant(&sv) {
                v = sv;
            } else if let Some(d) = sv.scalar() {
                v.assign_scalar(d);
            }
        } else if p.component >= 0 {
            if let Some(s) = inputs.get(&PlugRef { component: -1, ..*p }) {
                if let PlugValue::Vec3(x) = read(s) {
                    v = PlugValue::Float(x[p.component as usize]);
                }
            }
        } else if let PlugValue::Vec3(x) = &mut v {
            for c in 0..3 {
                let comp = PlugRef { component: c, ..*p };
                if held.contains(&comp) {
                    continue;
                }
                if let Some(s) = inputs.get(&comp) {
                    x[c as usize] = read(s).scalar().unwrap_or(x[c as usize]);
                }
            }
        }
    }
    if p.component < 0 {
        a.clamp(&mut v);
    }
    v
}

fn inputs_of(
    inputs: &BTreeMap<PlugRef, PlugRef>,
    id: NodeId,
) -> impl Iterator<Item = (&PlugRef, &PlugRef)> {
    let lo = PlugRef {
        node: id,
        attr: i32::MIN,
        component: i32::MIN,
        element: i32::MIN,
    };
    let hi = PlugRef {
        node: id,
        attr: i32::MAX,
        component: i32::MAX,
        element: i32::MAX,
    };
    inputs.range(lo..=hi)
}

impl EvalView<'_> {
    fn parent(&self, id: NodeId) -> NodeId {
        self.rel.get(&id).map(|r| r.parent).unwrap_or(NO_NODE)
    }

    /// The most a node's parent and sources computed this pass: any recompute upstream invalidates a restore.
    fn upstream_playback(&self, n: &Node) -> u8 {
        let mut most = 0;
        let mut visit = |up: NodeId| {
            if let Some(u) = self.nodes.get(&up) {
                if u.evaluated_pass == self.pass {
                    most = most.max(u.playback);
                }
            }
        };
        let p = self.parent(n.id);
        if p != NO_NODE {
            visit(p);
        }
        for (_, src) in inputs_of(self.inputs, n.id) {
            visit(src.node);
        }
        most
    }

    /// Puts back what the node computed at the current time, if kept.
    fn restore_cached(&self, n: &mut Node) -> bool {
        let c = self.playback.unwrap();
        let extra;
        {
            let inner = c.inner.lock().unwrap();
            let Some(t) = inner.times.get(&time_key(self.ctx.time)) else {
                return false;
            };
            let Some(e) = t.get(&n.id) else { return false };
            for (attr, v) in &e.values {
                n.data.values[*attr] = v.clone();
            }
            if let Some(els) = &e.elements {
                n.data.elements = els.clone();
            }
            extra = e.extra.clone();
        }
        n.behavior.restore_playback_extra(extra.as_ref());
        true
    }

    /// Returns whether it was kept.
    fn keep_cached(&self, n: &Node) -> bool {
        let c = self.playback.unwrap();
        // Outputs and whatever arrives by connection; unconnected inputs only change by an edit, which drops the cache.
        let attrs = &n.data.ty.attrs;
        let mut keep: Vec<bool> = (0..n.data.values.len())
            .map(|i| i < attrs.len() && !attrs[i].has(flags::INPUT))
            .collect();
        // Array outputs (an expression's) live in the elements too.
        let mut elements = attrs.iter().any(|a| a.is_array() && !a.has(flags::INPUT));
        for (dst, _) in inputs_of(self.inputs, n.id) {
            if dst.element >= 0 {
                elements = true;
            } else if (dst.attr as usize) < keep.len() {
                keep[dst.attr as usize] = true;
            }
        }
        if n.data.ty.dag {
            keep[Transform::PARENT_MATRIX] = true;
        }
        let values: Vec<(usize, PlugValue)> = keep
            .iter()
            .enumerate()
            .filter(|(_, k)| **k)
            .map(|(i, _)| (i, n.data.values[i].clone()))
            .collect();
        let extra = n.behavior.playback_extra(&self.ctx, &n.data);
        if !extra.keep {
            return false;
        }
        let entry = PlaybackEntry {
            values,
            elements: elements.then(|| n.data.elements.clone()),
            extra: extra.data,
        };

        let mut inner = c.inner.lock().unwrap();
        let key = time_key(self.ctx.time);
        let has = inner.times.get(&key).is_some_and(|t| t.contains_key(&n.id));
        if inner.bytes >= inner.limit && !has {
            return false;
        }
        let mut bytes = extra.bytes + entry.values.len() * std::mem::size_of::<PlugValue>();
        for (_, v) in &entry.values {
            if let PlugValue::Mesh(Some(m)) = v {
                if inner.counted.insert(Arc::as_ptr(m) as *const () as usize) {
                    bytes += m.memory_bytes();
                }
            }
        }
        inner.bytes += bytes;
        inner.times.entry(key).or_default().insert(n.id, entry);
        true
    }

    fn pull(&self, n: &mut Node) {
        let cache_node = self.cache && !self.turned.contains(&n.id);
        let upstream = if cache_node {
            self.upstream_playback(n)
        } else {
            0
        };
        if cache_node && upstream == 0 && self.restore_cached(n) {
            n.playback = 0;
            n.dirty = false;
            n.evaluated_pass = self.pass;
            return;
        }
        let dag = n.data.ty.dag;
        if dag {
            let p = self.parent(n.id);
            // A node free of its parent may be computed alongside it, or before it: it reads what the parent last held.
            let pw = if p != NO_NODE {
                self.nodes.get(&p).map_or_else(|| n.data.matrix(Transform::PARENT_MATRIX), |up| up.matrix(Transform::WORLD_MATRIX))
            } else {
                DMat4::IDENTITY
            };
            n.data.values[Transform::PARENT_MATRIX] = PlugValue::Matrix(pw);
        }
        let ty = n.data.ty.clone();
        for (dp, sp) in inputs_of(self.inputs, n.id) {
            if !self.held.is_empty() && self.held.contains(dp) {
                continue;
            }
            if computes_nothing(&ty, dp.attr) {
                continue;
            }
            let passed = self.passed.get(sp);
            let from_element;
            let sv = if let Some(v) = passed {
                v
            } else if sp.element >= 0 {
                let up = &self.nodes[&sp.node];
                from_element = up.element(sp.attr as usize, sp.element).cloned().unwrap_or_else(|| up.ty().attrs[sp.attr as usize].default_value.clone());
                &from_element
            } else {
                &self.nodes[&sp.node].data.values[sp.attr as usize]
            };
            // A passed value is already the component it names.
            let sp_component = if passed.is_some() { -1 } else { sp.component };
            let da = &ty.attrs[dp.attr as usize];
            let slot = if dp.element >= 0 {
                match n
                    .data
                    .elements
                    .get_mut(dp.attr as usize)
                    .and_then(|els| els.iter_mut().find(|(i, _)| *i == dp.element))
                {
                    Some((_, v)) => v,
                    None => continue,
                }
            } else {
                &mut n.data.values[dp.attr as usize]
            };
            if sp_component >= 0 || dp.component >= 0 {
                let s = if sp_component >= 0 {
                    vec3_component(sv, sp_component)
                } else {
                    sv.scalar().unwrap_or(0.0)
                };
                if dp.component >= 0 {
                    if let PlugValue::Vec3(x) = slot {
                        x[dp.component as usize] = s;
                    }
                } else {
                    slot.assign_scalar(s);
                }
                da.clamp(slot);
                continue;
            }
            // Numeric types convert across connections; a generic plug takes what comes.
            if std::mem::discriminant(slot) == std::mem::discriminant(sv) || (da.has(flags::GENERIC) && matches!(sv, PlugValue::Vec3(_) | PlugValue::Float(_))) {
                *slot = sv.clone();
            } else if let (Some(s), Some(_)) = (sv.scalar(), slot.scalar()) {
                slot.assign_scalar(s);
            }
            da.clamp(slot);
        }
        let parent_frame = if dag { n.data.matrix(Transform::PARENT_MATRIX) } else { DMat4::IDENTITY };
        let is_transform = dag && n.data.ty.is_a("transform");
        if is_transform {
            Transform::apply_limits(&mut n.data);
            // inheritsTransform off detaches the node from its parent; offsetParentMatrix sits between parent and local.
            let mut frame = parent_frame;
            if !n.data.bool(Transform::INHERITS_TRANSFORM) {
                frame = DMat4::IDENTITY;
            }
            let offset = n.data.matrix(Transform::OFFSET_PARENT_MATRIX);
            if offset != DMat4::IDENTITY {
                frame *= offset;
            }
            n.data.values[Transform::PARENT_MATRIX] = PlugValue::Matrix(frame);
        }
        if dag {
            if let Some(f) = self.active_frame {
                if let Some(frame) = f(n) {
                    n.data.values[Transform::PARENT_MATRIX] = PlugValue::Matrix(frame);
                }
            }
            if let Some(turn) = self.display_turns.get(&n.id) {
                let pm = n.data.matrix(Transform::PARENT_MATRIX);
                n.data.values[Transform::PARENT_MATRIX] = PlugValue::Matrix(*turn * pm);
            }
        }
        let ctx = self.ctx;
        n.behavior.set_node_name(&n.name);
        n.behavior.compute(&ctx, &mut n.data);
        if dag {
            crate::transform::DagNode::compute_derived(&mut n.data, &parent_frame, is_transform);
        }
        // Computed from a placeholder upstream means a placeholder too.
        if cache_node {
            n.playback = if upstream < 2 && self.keep_cached(n) {
                1
            } else {
                2
            };
        }
        n.dirty = false;
        n.evaluated_pass = self.pass;
    }
}

/// The scene's dependency graph plus its transform hierarchy. Main thread only, except `evaluate()` which fans compute
/// out over the rayon pool. Every mutation records undo.
/// Tool Settings' options that change what a component move does.
#[derive(Clone, Debug)]
pub struct MoveOptions {
    /// UVs keep the texture where it was on the surface as points move.
    pub preserve_uv: bool,
    /// "edge" or "surface": points slide along their edges or stay on their faces.
    pub constraint: String,
    /// Symmetry holds seam points on the plane.
    pub preserve_seam: bool,
}

impl Default for MoveOptions {
    fn default() -> Self {
        MoveOptions { preserve_uv: false, constraint: String::new(), preserve_seam: true }
    }
}

pub struct Graph {
    /// While a file is read: evaluating as each node arrives costs a pass per constraint and nothing reads the results.
    pub(crate) evaluation_suspended: bool,
    /// The evaluation pass rig nodes were last brought in step with the scene at (see rig.rs).
    pub(crate) rig_synced_pass: u64,
    nodes: IdMap<Box<Node>>,
    /// Moves whenever connections, parents or nodes change, which is what an evaluation order rests on.
    wiring: u64,
    /// Evaluation orders lately used, latest first. A drag repeats the same few passes every frame (a wire samples its
    /// drivers, then the scene evaluates), so each finds its order here instead of working it out again.
    plans: Vec<EvalPlan>,
    /// Short name -> nodes.
    names: HashMap<String, Vec<NodeId>>,
    /// For a wanted name already taken, the number its search for a free one last stopped at: every number before it
    /// is taken, so the next search starts there. Forgotten whenever a name is given up.
    name_hints: std::sync::Mutex<HashMap<String, i64>>,
    rel: IdMap<Relations>,
    roots: Vec<NodeId>,
    /// dst -> src
    inputs: BTreeMap<PlugRef, PlugRef>,
    /// src node -> dst plugs it drives
    outgoing_dst: HashMap<NodeId, Vec<PlugRef>>,
    selection: Vec<NodeId>,
    selection_serial: u64,
    /// Driven plugs keeping a user-set value.
    held: BTreeSet<PlugRef>,
    driven_set: Option<DrivenSetHook>,
    driven_vec_set: Option<DrivenVecSetHook>,
    delete_hook: Option<DeleteHook>,
    delete_depth: i32,
    listeners: BTreeMap<i32, ListenerEntry>,
    next_listener: i32,
    changed_plug: Option<PlugRef>,
    batch_depth: i32,
    batch_pending: bool,
    next_id: NodeId,
    ctx: EvalContext,
    naming: NamingPolicy,
    version: AtomicU64,
    structure: u64,
    visibility_version: u64,
    /// nodes() sorted, kept until the next structure change.
    sorted: Mutex<(u64, Vec<NodeId>)>,
    any_dirty: bool,
    evaluation_pass: u64,
    playback: Option<PlaybackCache>,
    playback_memory_mb: i64,
    display_turns: HashMap<NodeId, DMat4>,
    /// The turned nodes and their descendants, kept out of the playback cache.
    turned: HashSet<NodeId>,
    undo: UndoStack<Graph>,
    evaluate_hook: Option<EvaluateHook>,
    extra_evaluate_hooks: Vec<EvaluateHook>,
    in_evaluate_hook: bool,
    /// Stands in for `sim::sampling()`: simulations sampling their inputs at other times set it.
    sampling: bool,
    active_frame: Option<ActiveFrameFn>,
    /// Stands in for "the running command has no name flag" (`ops::currentArgs`), set by the command layer.
    command_default_names: bool,
    /// The component selection and mode.
    pub components: crate::components::ComponentState,
    /// The isolation set the viewport shows, if any.
    pub active_isolation: Option<crate::isolation::Set>,
    pub inherit_listeners: BTreeMap<i32, crate::isolation::InheritListener>,
    /// The sphere the latest polySpherize used.
    pub last_sphere: crate::conform_commands::SphereState,
    /// What the last hide hid, for showLastHidden.
    pub last_hidden: Vec<NodeId>,
    /// Whether poly edits record history nodes (constructionHistory).
    pub construction_history: bool,
    /// The move tools' options that change what a component move does.
    pub move_opts: MoveOptions,
    /// What Duplicate with Transform repeats; see duplicate::install.
    pub duplicate_pairs: crate::duplicate::Pairs,
    /// Namespaces that exist besides the root, full names ("rig:arm"). Those nodes live in exist too, by their names.
    pub namespaces: BTreeSet<String>,
    /// Where new nodes go ("" is the root): namespace -set.
    pub current_namespace: String,
}

impl Default for Graph {
    fn default() -> Self {
        Self::new()
    }
}

/// An evaluation order, good while nothing's rewired and the same nodes are dirty.
#[derive(Default)]
struct EvalPlan {
    wiring: u64,
    dirty: Vec<NodeId>,
    levels: Vec<Vec<NodeId>>,
    late: Vec<PlugRef>,
}

/// A quick hash for node ids: they're small numbers from the graph itself, so nobody's choosing them to collide.
#[derive(Default, Clone, Copy)]
pub struct IdHasher(u64);

impl std::hash::Hasher for IdHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        for b in bytes {
            self.write_u64(*b as u64);
        }
    }
    #[inline]
    fn write_u32(&mut self, v: u32) {
        self.write_u64(v as u64);
    }
    #[inline]
    fn write_u64(&mut self, v: u64) {
        self.0 = (self.0.rotate_left(5) ^ v).wrapping_mul(0x517c_c1b7_2722_0a95);
    }
    #[inline]
    fn write_usize(&mut self, v: usize) {
        self.write_u64(v as u64);
    }
}

type IdMap<V> = HashMap<NodeId, V, std::hash::BuildHasherDefault<IdHasher>>;

impl Graph {
    pub fn new() -> Self {
        Self {
            rig_synced_pass: u64::MAX,
            evaluation_suspended: false,
            nodes: Default::default(),
            wiring: 0,
            plans: Vec::new(),
            names: HashMap::new(),
            name_hints: std::sync::Mutex::new(HashMap::new()),
            rel: Default::default(),
            roots: Vec::new(),
            inputs: BTreeMap::new(),
            outgoing_dst: HashMap::new(),
            selection: Vec::new(),
            selection_serial: 0,
            held: BTreeSet::new(),
            driven_set: None,
            driven_vec_set: None,
            delete_hook: None,
            delete_depth: 0,
            listeners: BTreeMap::new(),
            next_listener: 1,
            changed_plug: None,
            batch_depth: 0,
            batch_pending: false,
            next_id: 1,
            ctx: EvalContext::default(),
            naming: NamingPolicy::default(),
            version: AtomicU64::new(1),
            structure: next_structure(),
            visibility_version: 0,
            sorted: Mutex::new((0, Vec::new())),
            any_dirty: false,
            evaluation_pass: 0,
            playback: None,
            playback_memory_mb: 0,
            display_turns: HashMap::new(),
            turned: HashSet::new(),
            undo: UndoStack::new(),
            evaluate_hook: None,
            extra_evaluate_hooks: Vec::new(),
            in_evaluate_hook: false,
            sampling: false,
            active_frame: None,
            command_default_names: false,
            components: Default::default(),
            active_isolation: None,
            inherit_listeners: BTreeMap::new(),
            last_sphere: Default::default(),
            last_hidden: Vec::new(),
            construction_history: true,
            move_opts: MoveOptions::default(),
            duplicate_pairs: Default::default(),
            namespaces: BTreeSet::new(),
            current_namespace: String::new(),
        }
    }

    fn restructured(&mut self) {
        self.structure = next_structure();
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Undo

    pub fn undo_stack(&self) -> &UndoStack<Graph> {
        &self.undo
    }
    pub fn undo_stack_mut(&mut self) -> &mut UndoStack<Graph> {
        &mut self.undo
    }

    fn with_undo<R>(&mut self, f: impl FnOnce(&mut UndoStack<Graph>, &mut Graph) -> R) -> R {
        let mut stack = std::mem::replace(&mut self.undo, UndoStack::replaying_placeholder());
        let r = f(&mut stack, self);
        let mut placeholder = std::mem::replace(&mut self.undo, stack);
        self.undo.take_listeners_from(&mut placeholder);
        r
    }

    /// Runs edits the undo stack doesn't hear about: a view's camera following the mouse, the default nodes of a new scene.
    pub fn without_undo<R>(&mut self, f: impl FnOnce(&mut Graph) -> R) -> R {
        self.with_undo(|_, g| f(g))
    }

    pub fn undo(&mut self) -> bool {
        self.with_undo(|s, g| s.undo(g))
    }
    pub fn redo(&mut self) -> bool {
        self.with_undo(|s, g| s.redo(g))
    }
    pub fn jump_to(&mut self, position: usize) -> bool {
        self.with_undo(|s, g| s.jump_to(position, g))
    }
    pub fn open_chunk(&mut self, label: impl Into<String>) {
        self.undo.open_chunk(label)
    }
    pub fn close_chunk(&mut self) {
        self.undo.close_chunk()
    }
    pub fn cancel_chunk(&mut self) {
        self.with_undo(|s, g| s.cancel_chunk(g))
    }
    pub fn rollback_to(&mut self, mark: usize) {
        self.with_undo(|s, g| s.rollback_to(mark, g))
    }
    pub fn open_amend(&mut self, label: impl Into<String>) -> bool {
        let label = label.into();
        self.with_undo(|s, g| s.open_amend(label, g))
    }
    pub fn reopen_newest(&mut self, label: impl Into<String>) -> bool {
        let label = label.into();
        self.with_undo(|s, g| s.reopen_newest(label, g))
    }
    /// Runs `f` as one undo step.
    pub fn chunk<R>(&mut self, label: &str, f: impl FnOnce(&mut Graph) -> R) -> R {
        self.open_chunk(label);
        let r = f(self);
        self.close_chunk();
        r
    }

    fn record(
        &mut self,
        undo: impl FnMut(&mut Graph) + 'static,
        redo: impl FnMut(&mut Graph) + 'static,
    ) {
        self.undo.record(FnAction::new(undo, redo));
    }

    fn record_held(
        &mut self,
        undo: impl FnMut(&mut Graph) + 'static,
        redo: impl FnMut(&mut Graph) + 'static,
        held: &[PlugValue],
    ) {
        let mut owned = std::mem::size_of::<FnAction<Graph>>();
        let mut shared = UndoShared::default();
        for v in held {
            measure_value(v, &mut shared, &mut owned);
        }
        self.undo
            .record(FnAction::new(undo, redo).sized(owned, shared));
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Playback cache

    /// While on, what every node computes is kept per time, and a full `evaluate()` at a kept time restores it instead
    /// of computing. Any change to the scene but a selection drops what is kept.
    pub fn set_cached_playback(&mut self, on: bool) {
        if on == self.cached_playback() {
            return;
        }
        self.playback = on.then(|| PlaybackCache {
            inner: Mutex::new(PlaybackInner::default()),
        });
        self.clear_playback_cache();
    }
    pub fn cached_playback(&self) -> bool {
        self.playback.is_some()
    }
    /// The memory cached playback may use in MB; 0 is half the machine's.
    pub fn set_playback_memory_mb(&mut self, mb: i64) {
        self.playback_memory_mb = mb;
        self.clear_playback_cache();
    }

    pub fn clear_playback_cache(&mut self) {
        let Some(p) = &self.playback else { return };
        let mb = self.playback_memory_mb;
        let mut inner = p.inner.lock().unwrap();
        inner.times.clear();
        inner.counted.clear();
        inner.bytes = 0;
        inner.limit = if mb > 0 {
            (mb as usize) << 20
        } else {
            physical_memory_bytes() / 2
        };
    }

    /// Every kept time in seconds, ascending.
    pub fn playback_cache_times(&self) -> Vec<f64> {
        let Some(p) = &self.playback else {
            return Vec::new();
        };
        p.inner
            .lock()
            .unwrap()
            .times
            .keys()
            .map(|k| *k as f64 / 1e6)
            .collect()
    }

    /// Roughly what the kept results hold in memory.
    pub fn playback_cache_bytes(&self) -> usize {
        self.playback
            .as_ref()
            .map(|p| p.inner.lock().unwrap().bytes)
            .unwrap_or(0)
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Nodes

    /// Not undoable.
    pub fn clear(&mut self) {
        self.restructured();
        self.nodes.clear();
        self.names.clear();
        self.name_hints.lock().unwrap().clear();
        self.rel.clear();
        self.roots.clear();
        self.wiring += 1;
        self.inputs.clear();
        self.outgoing_dst.clear();
        self.selection.clear();
        self.selection_serial += 1;
        self.held.clear();
        self.display_turns.clear();
        self.turned.clear();
        self.ctx = EvalContext::default();
        self.undo.clear();
        self.components.cleared();
        self.last_hidden.clear();
        if let Some(set) = &mut self.active_isolation {
            set.clear();
        }
        self.changed(Change::Cleared, NO_NODE, None);
    }

    /// What the reference makes of a name it can't take: each bad character becomes an underscore, a leading digit gets one before it.
    pub fn legal_name(name: &str) -> String {
        let mut out = String::with_capacity(name.len() + 1);
        let mut start = true;
        for c in name.chars() {
            if c == ':' {
                out.push(c);
                start = true;
                continue;
            }
            if c.is_ascii_digit() && start {
                out.push('_');
            }
            out.push(if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' });
            start = false;
        }
        out
    }

    pub fn valid_name(name: &str) -> bool {
        if name.is_empty() {
            return false;
        }
        let mut start = true;
        for c in name.chars() {
            if c == ':' {
                start = true;
                continue;
            }
            let alpha = c.is_ascii_alphabetic() || c == '_';
            let digit = c.is_ascii_digit();
            if !(alpha || (digit && !start)) {
                return false;
            }
            start = false;
        }
        true
    }

    /// global: unique across the scene; otherwise among the children of parent.
    fn unique_name(&self, wanted: &str, parent: NodeId, self_id: NodeId, global: bool) -> String {
        let taken = |n: &str| {
            self.names.get(n).is_some_and(|ids| {
                ids.iter()
                    .any(|&id| id != self_id && (global || self.parent(id) == parent))
            })
        };
        if !taken(wanted) {
            return wanted.to_string();
        }
        let digits = wanted.len() - wanted.bytes().rev().take_while(u8::is_ascii_digit).count();
        let stem = &wanted[..digits];
        let width = wanted.len() - digits;
        let mut n: i64 = if digits < wanted.len() {
            wanted[digits..].parse().unwrap_or(1)
        } else {
            1
        };
        // Across the scene and for a new node, what is taken only grows until a name is given up: the search goes on
        // from where the last one for this name stopped, so the thousandth pCube1 does not count from 1 again.
        let hinted = global && self_id == NO_NODE;
        if hinted {
            if let Some(&h) = self.name_hints.lock().unwrap().get(wanted) {
                n = n.max(h);
            }
        }
        loop {
            // A trailing number keeps its padding: arm_01 gives arm_02, ctl_009 gives ctl_010.
            let candidate = format!("{stem}{n:0width$}");
            if !taken(&candidate) {
                if hinted {
                    self.name_hints.lock().unwrap().insert(wanted.to_string(), n);
                }
                return candidate;
            }
            n += 1;
        }
    }

    /// `thing#` as the next free name: each run of hashes is a number padded to its length, counted from 1.
    pub fn resolve_hashes(&self, wanted: &str) -> String {
        let Some(start) = wanted.find('#') else { return wanted.to_string() };
        let len = wanted[start..].bytes().take_while(|b| *b == b'#').count();
        let (head, tail) = (&wanted[..start], &wanted[start + len..]);
        let mut n = 1;
        loop {
            let candidate = format!("{head}{n:0len$}{tail}");
            if candidate.contains('#') {
                let inner = self.resolve_hashes(&candidate);
                if !self.names.contains_key(&inner) {
                    return inner;
                }
            } else if !self.names.contains_key(&candidate) {
                return candidate;
            }
            n += 1;
        }
    }

    /// Applies the naming policy; errors in block mode when a user-given name doesn't conform.
    fn conform_name(&self, ty: &NodeType, wanted: &str, user_given: bool) -> Result<String> {
        let fixed = self.naming.conform(ty, wanted);
        if !user_given || fixed == wanted {
            return Ok(fixed);
        }
        match self.naming.mode {
            NamingMode::Fix => Ok(fixed),
            NamingMode::Warn => {
                log::warn!("'{wanted}' breaks the naming convention (expected '{fixed}')");
                Ok(wanted.to_string())
            }
            NamingMode::Block => Err(GraphError::invalid(format!(
                "'{wanted}' breaks the naming convention; use '{fixed}'"
            ))),
        }
    }

    /// A name nobody chose (empty, or a command's own default when its call has no name flag) is made unique across
    /// the scene, so new nodes don't make existing names ambiguous.
    fn default_name(&self, name: &str, apply_naming: bool) -> bool {
        name.is_empty() || (apply_naming && self.command_default_names)
    }

    /// The command layer sets this while a command runs without a name flag, so the default names it gives are made
    /// unique across the scene.
    pub fn set_command_default_names(&mut self, on: bool) {
        self.command_default_names = on;
    }

    /// What `wanted` would become for a node under `parent` (NO_NODE for a root or a non-DAG node).
    /// The next name no node in the scene holds, counting on from `wanted`'s trailing number.
    pub fn next_scene_name(&self, wanted: &str) -> String {
        self.unique_name(wanted, NO_NODE, NO_NODE, true)
    }

    pub fn available_name(&self, wanted: &str, parent: NodeId) -> String {
        self.unique_name(wanted, parent, NO_NODE, self.naming.unique_names)
    }

    fn erase_name(&mut self, name: &str, id: NodeId) {
        self.name_hints.lock().unwrap().clear();
        if let Some(ids) = self.names.get_mut(name) {
            if let Some(i) = ids.iter().position(|&x| x == id) {
                ids.remove(i);
            }
            if ids.is_empty() {
                self.names.remove(name);
            }
        }
    }

    fn siblings_mut(&mut self, parent: NodeId) -> &mut Vec<NodeId> {
        if parent == NO_NODE {
            &mut self.roots
        } else {
            &mut self.rel.entry(parent).or_default().children
        }
    }

    fn siblings(&self, parent: NodeId) -> &[NodeId] {
        if parent == NO_NODE {
            &self.roots
        } else {
            self.children(parent)
        }
    }

    fn attach(&mut self, mut n: Box<Node>, parent: NodeId, index: usize) {
        self.restructured();
        let id = n.id;
        self.names.entry(n.name.clone()).or_default().push(id);
        if n.data.ty.dag {
            self.rel.entry(id).or_default().parent = parent;
            let sib = self.siblings_mut(parent);
            let at = index.min(sib.len());
            sib.insert(at, id);
        }
        n.dirty = true;
        self.any_dirty = true;
        self.nodes.insert(id, n);
        self.wiring += 1;
        self.components.node_created(id);
        self.changed(Change::Created, id, None);
    }

    fn detach(&mut self, id: NodeId) -> Option<(Box<Node>, usize)> {
        self.wiring += 1;
        let n = self.nodes.remove(&id)?;
        self.restructured();
        self.erase_name(&n.name.clone(), id);
        let before = self.selection.len();
        self.selection.retain(|&x| x != id);
        if self.selection.len() != before {
            self.selection_serial += 1;
        }
        let mut index = 0;
        if let Some(rel) = self.rel.remove(&id) {
            let sib = self.siblings_mut(rel.parent);
            if let Some(pos) = sib.iter().position(|&x| x == id) {
                index = pos;
                sib.remove(pos);
            }
        }
        self.components.node_deleted(id);
        self.changed(Change::Deleted, id, None);
        Some((n, index))
    }

    /// `apply_naming = false` keeps the name as given (file loads); it is still made unique among its siblings, or
    /// across the scene with `NamingPolicy::unique_names`. Default names (empty) are always made unique across the scene.
    pub fn create_node(
        &mut self,
        ty: &Arc<NodeType>,
        name: &str,
        parent: NodeId,
        apply_naming: bool,
    ) -> Result<NodeId> {
        let create = ty
            .create
            .clone()
            .ok_or_else(|| GraphError::invalid(format!("node type '{}' is abstract", ty.name)))?;
        if parent != NO_NODE && !self.node(parent).is_some_and(|p| p.ty().dag) {
            return Err(GraphError::invalid("parent must be a transform"));
        }
        let fixed;
        let name = if !name.is_empty() && !Self::valid_name(name) {
            fixed = Self::legal_name(name);
            fixed.as_str()
        } else {
            name
        };

        let reg = registered(ty);
        let mut n = Box::new(Node::new(reg.clone(), create()));
        n.id = self.next_id;
        n.behavior.set_node_id(n.id);
        self.next_id += 1;
        // The reference's default: the type name numbered from 1 (transform1, polyCube1). A naming convention, when on, styles it.
        let fallback = if self.naming.enabled {
            format!("{}1", apply_case(&ty.name, self.naming.style_for(ty), &Underscores::default()))
        } else {
            format!("{}1", ty.name)
        };
        let p = if ty.dag { parent } else { NO_NODE };
        let wanted = if name.is_empty() {
            fallback
        } else if apply_naming {
            self.conform_name(ty, name, true)?
        } else {
            name.to_string()
        };
        // Scene files name their nodes in full; everything else lands in the current namespace.
        let wanted = match apply_naming || name.is_empty() {
            true if !self.current_namespace.is_empty() && !wanted.contains(':') => format!("{}:{wanted}", self.current_namespace),
            _ => wanted,
        };
        n.name = self.unique_name(
            &wanted,
            p,
            NO_NODE,
            self.naming.unique_names || self.default_name(name, apply_naming),
        );
        if let Some(colon) = n.name.rfind(':') {
            n.namespace = n.name[..colon].to_string();
        }
        n.data.values = reg.attrs.iter().map(|a| a.default_value.clone()).collect();
        // A copy of a node with user attrs (ty is its private type) gets them too, as part of being created.
        if ty.user_of.is_some() {
            n.data.ty = Arc::new((**ty).clone());
            n.own_type = true;
            for a in &ty.attrs[n.data.values.len()..] {
                n.data.values.push(a.default_value.clone());
            }
        }

        let id = n.id;
        self.attach(n, p, usize::MAX);
        let index = self.siblings(p).len().saturating_sub(1);
        // Recorded as an inverted delete: undo removes, redo restores.
        self.undo.record(DeleteAction {
            id,
            parent: p,
            index,
            held: None,
            inverted: true,
        });
        Ok(id)
    }

    /// Gives a node the identity it had when saved. Not undoable.
    pub fn next_node_id(&self) -> NodeId {
        self.next_id
    }

    pub fn set_uuid(&mut self, id: NodeId, uuid: &str) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.uuid = uuid.to_string();
        }
    }

    /// Marks a node as loaded from a file reference (0 = owned by the scene). Not undoable; references manage it.
    pub fn set_reference(&mut self, id: NodeId, reference: u32) {
        if let Some(n) = self.nodes.get_mut(&id) {
            n.reference = reference;
        }
    }

    /// lockNode. Undoable.
    pub fn set_node_locked(&mut self, id: NodeId, locked: bool) {
        let Some(old) = self.node(id).map(|n| n.locked) else { return };
        if old == locked {
            return;
        }
        fn raw(g: &mut Graph, id: NodeId, on: bool) {
            if let Some(n) = g.nodes.get_mut(&id) {
                n.locked = on;
            }
            g.changed(Change::Value, id, None);
        }
        raw(self, id, locked);
        self.record(move |g| raw(g, id, old), move |g| raw(g, id, locked));
    }

    pub fn delete_node(&mut self, id: NodeId) -> Result<()> {
        if self.node(id).is_none() {
            return Ok(());
        }
        if self.delete_depth == 0 {
            if let Some(hook) = self.delete_hook.clone() {
                let mut doomed = vec![id];
                doomed.extend(self.descendants(id));
                self.delete_depth += 1;
                hook(self, &doomed);
                self.delete_depth -= 1;
            }
        }
        self.delete_depth += 1;
        for child in self.children(id).to_vec() {
            if let Err(e) = self.delete_node(child) {
                self.delete_depth -= 1;
                return Err(e);
            }
        }
        let inbound: Vec<PlugRef> = self
            .inputs
            .iter()
            .filter(|(d, s)| d.node == id || s.node == id)
            .map(|(d, _)| *d)
            .collect();
        for dst in inbound {
            self.disconnect(&dst);
        }
        if self.is_selected(id) {
            let sel: Vec<NodeId> = self
                .selection
                .iter()
                .copied()
                .filter(|&x| x != id)
                .collect();
            self.select(&sel, SelectMode::Replace);
        }
        let p = self.parent(id);
        if let Some((held, index)) = self.detach(id) {
            self.undo.record(DeleteAction {
                id,
                parent: p,
                index,
                held: Some(held),
                inverted: false,
            });
        }
        self.delete_depth -= 1;
        Ok(())
    }

    pub fn node(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(&id).map(|b| b.as_ref())
    }

    /// For subsystems that own a node's behaviour state (simulations and the like). Values go through `set_value`.
    pub fn behavior_mut(&mut self, id: NodeId) -> Option<&mut dyn crate::node::NodeBehavior> {
        self.nodes.get_mut(&id).map(|n| n.behavior.as_mut())
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        self.nodes.get_mut(&id).expect("no such node")
    }

    fn name_of(&self, id: NodeId) -> &str {
        &self.nodes[&id].name
    }

    /// id's ancestors' names match segs (leaf last, already known to match); absolute also requires a root.
    fn matches_path(&self, id: NodeId, segs: &[&str], absolute: bool) -> bool {
        let mut cur = id;
        for seg in segs[..segs.len() - 1].iter().rev() {
            cur = self.parent(cur);
            if cur == NO_NODE || self.name_of(cur) != *seg {
                return false;
            }
        }
        !absolute || self.parent(cur) == NO_NODE
    }

    /// Every node `path` matches, by id.
    pub fn find_all(&self, path: &str) -> Vec<NodeId> {
        let absolute = path.starts_with('|');
        let path = if absolute { &path[1..] } else { path };
        let segs: Vec<&str> = path.split('|').collect();
        if segs.iter().any(|s| s.is_empty()) {
            return Vec::new();
        }
        let mut out: Vec<NodeId> = self
            .names
            .get(*segs.last().unwrap())
            .map(|ids| {
                ids.iter()
                    .copied()
                    .filter(|&id| {
                        (segs.len() == 1 && !absolute) || self.matches_path(id, &segs, absolute)
                    })
                    .collect()
            })
            .unwrap_or_default();
        out.sort_unstable();
        out
    }

    /// Resolves a short name ("Arm"), a partial path ("L|Arm", any unique run of parents ending at the node) or a full
    /// path ("|Body|L|Arm"). None when nothing matches; `Ambiguous`, listing the full paths, when several do.
    pub fn find(&self, path: &str) -> Result<Option<NodeId>> {
        // surface->curve: a curve on a surface goes by its own name.
        let path = path.rsplit_once("->").map_or(path, |x| x.1);
        if !path.contains('|') {
            match self.names.get(path).map(Vec::as_slice) {
                None | Some([]) => return Ok(None),
                Some([one]) => return Ok(Some(*one)),
                _ => {}
            }
        }
        let all = self.find_all(path);
        match all.len() {
            0 => Ok(None),
            1 => Ok(Some(all[0])),
            _ => {
                let list: Vec<String> = all.iter().map(|&id| self.full_path(id)).collect();
                Err(GraphError::Ambiguous(format!(
                    "'{path}' is ambiguous: it matches {}; use a path",
                    list.join(", ")
                )))
            }
        }
    }

    /// `find` that wants exactly one node.
    pub fn find_node(&self, path: &str) -> Result<&Node> {
        let id = self
            .find(path)?
            .ok_or_else(|| GraphError::invalid(format!("no node named '{path}'")))?;
        Ok(self.node(id).unwrap())
    }

    /// "|Body|L|Arm".
    pub fn full_path(&self, id: NodeId) -> String {
        let mut chain = Vec::new();
        let mut cur = id;
        while cur != NO_NODE {
            match self.nodes.get(&cur) {
                Some(n) => chain.push(n.name.as_str()),
                None => break,
            }
            cur = self.parent(cur);
        }
        chain.iter().rev().map(|n| format!("|{n}")).collect()
    }

    /// The shortest path that resolves back to this node alone: its name when that is unique. "" for no node.
    pub fn path(&self, id: NodeId) -> String {
        let Some(n) = self.nodes.get(&id) else {
            return String::new();
        };
        let same = self.names.get(&n.name).map(Vec::as_slice).unwrap_or(&[]);
        if same.len() <= 1 {
            return n.name.clone();
        }
        let mut chain = Vec::new(); // leaf first
        let mut cur = id;
        while cur != NO_NODE {
            chain.push(self.name_of(cur));
            cur = self.parent(cur);
        }
        for k in 1..=chain.len() {
            let segs: Vec<&str> = chain[..k].iter().rev().copied().collect();
            let matches = same
                .iter()
                .filter(|&&other| self.matches_path(other, &segs, false))
                .take(2)
                .count();
            if matches == 1 {
                return segs.join("|");
            }
        }
        self.full_path(id)
    }

    pub fn nodes(&self) -> Vec<NodeId> {
        let mut s = self.sorted.lock().unwrap();
        if s.0 != self.structure {
            let mut ids: Vec<NodeId> = self.nodes.keys().copied().collect();
            ids.sort_unstable();
            *s = (self.structure, ids);
        }
        s.1.clone()
    }

    fn rename_raw(&mut self, id: NodeId, name: &str) {
        let old = self.name_of(id).to_string();
        self.erase_name(&old, id);
        let n = self.node_mut(id);
        n.name = name.to_string();
        n.namespace = name
            .rfind(':')
            .map(|c| name[..c].to_string())
            .unwrap_or_default();
        self.names.entry(name.to_string()).or_default().push(id);
        self.changed(Change::Renamed, id, None);
    }

    /// Returns the name actually given, which is made unique (see `create_node`).
    pub fn rename(&mut self, id: NodeId, name: &str) -> Result<String> {
        let n = self
            .node(id)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        if name.is_empty() {
            return Err(GraphError::invalid("a node's name can't be empty"));
        }
        let fixed = Self::legal_name(name);
        let name = if Self::valid_name(name) { name } else { fixed.as_str() };
        let old_name = n.name.clone();
        let conformed = self.conform_name(n.ty(), name, true)?;
        let new_name = self.unique_name(&conformed, self.parent(id), id, self.naming.unique_names);
        if new_name == old_name {
            return Ok(new_name);
        }
        self.rename_raw(id, &new_name);
        let (o, nn) = (old_name, new_name.clone());
        self.record(
            move |g| g.rename_raw(id, &o),
            move |g| g.rename_raw(id, &nn),
        );
        Ok(new_name)
    }

    /// Runs `f` with the graph evaluating at another time, then comes back; held values survive it (getAttr -t).
    pub fn at_time<R>(&mut self, seconds: f64, f: impl FnOnce(&mut Graph) -> R) -> R {
        let was = self.ctx.time;
        let held = self.held.clone();
        self.set_time(seconds);
        let r = f(self);
        self.set_time(was);
        self.held = held;
        r
    }

    /// (keyable, shown in the channel box) of a plug: its own setting, else its attr's.
    pub fn channel_state(&self, p: &PlugRef) -> (bool, bool) {
        let Some(n) = self.node(p.node) else { return (false, false) };
        if let Some(&s) = n.channels.get(&(p.attr, p.component)) {
            return s;
        }
        if p.component >= 0 {
            if let Some(&s) = n.channels.get(&(p.attr, -1)) {
                return s;
            }
        }
        let a = &n.ty().attrs[p.attr as usize];
        (a.has(flags::KEYABLE), a.has(flags::CHANNEL_BOX))
    }

    /// setAttr -k and -cb. Undoable.
    pub fn set_channel_state(&mut self, p: &PlugRef, keyable: Option<bool>, channel_box: Option<bool>) {
        let (k, c) = self.channel_state(p);
        let now = (keyable.unwrap_or(k), channel_box.unwrap_or(if keyable == Some(true) { false } else { c }));
        let key = (p.attr, p.component);
        let id = p.node;
        let old = self.node(id).and_then(|n| n.channels.get(&key).copied());
        let apply = move |g: &mut Graph, v: Option<(bool, bool)>| {
            let n = g.node_mut(id);
            match v {
                Some(s) => {
                    n.channels.insert(key, s);
                }
                None => {
                    n.channels.remove(&key);
                }
            }
            g.changed(Change::Attrs, id, None);
        };
        apply(self, Some(now));
        self.record(move |g| apply(g, old), move |g| apply(g, Some(now)));
    }

    /// aliasAttr: `alias` stands for the attribute `target` (node-relative). None removes the alias. Undoable.
    pub fn set_alias(&mut self, id: NodeId, alias: &str, target: Option<String>) {
        let name = alias.to_string();
        let old = self.node(id).and_then(|n| n.aliases.get(alias).cloned());
        let apply = move |g: &mut Graph, v: &Option<String>| {
            let n = g.node_mut(id);
            match v {
                Some(t) => {
                    n.aliases.insert(name.clone(), t.clone());
                }
                None => {
                    n.aliases.remove(&name);
                }
            }
            g.changed(Change::Attrs, id, None);
        };
        let (a, b) = (apply.clone(), target.clone());
        apply(self, &target);
        self.record(move |g| a(g, &old), move |g| apply(g, &b));
    }

    /// Adds or removes a namespace from the known list. Undoable.
    pub fn set_namespace_known(&mut self, name: &str, known: bool) {
        let n = name.to_string();
        let was = self.namespaces.contains(name);
        let apply = move |g: &mut Graph, on: bool| {
            if on {
                g.namespaces.insert(n.clone());
            } else {
                g.namespaces.remove(&n);
            }
        };
        let a = apply.clone();
        apply(self, known);
        self.record(move |g| a(g, was), move |g| apply(g, known));
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Hierarchy

    pub fn parent(&self, id: NodeId) -> NodeId {
        self.rel.get(&id).map(|r| r.parent).unwrap_or(NO_NODE)
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.rel
            .get(&id)
            .map(|r| r.children.as_slice())
            .unwrap_or(&[])
    }

    pub fn roots(&self) -> &[NodeId] {
        &self.roots
    }

    pub fn descendants(&self, id: NodeId) -> Vec<NodeId> {
        let mut out = Vec::new();
        let mut stack: Vec<NodeId> = self.children(id).iter().rev().copied().collect();
        while let Some(n) = stack.pop() {
            out.push(n);
            stack.extend(self.children(n).iter().rev());
        }
        out
    }

    fn reparent_raw(&mut self, child: NodeId, new_parent: NodeId, index: usize) {
        self.wiring += 1;
        let old_parent = self.parent(child);
        self.siblings_mut(old_parent).retain(|&x| x != child);
        self.rel.entry(child).or_default().parent = new_parent;
        let sib = self.siblings_mut(new_parent);
        let at = index.min(sib.len());
        sib.insert(at, child);
        self.mark_dirty(child);
        self.changed(Change::Reparented, child, None);
    }

    /// `index` is the position the child takes among the parent's children (`usize::MAX` appends). With the same
    /// parent and an index it only reorders.
    pub fn set_parent(
        &mut self,
        child: NodeId,
        new_parent: NodeId,
        keep_world: bool,
        index: usize,
    ) -> Result<()> {
        if !self.node(child).is_some_and(|c| c.ty().dag) {
            return Err(GraphError::invalid("only transforms can be parented"));
        }
        if new_parent != NO_NODE {
            if !self.node(new_parent).is_some_and(|p| p.ty().dag) {
                return Err(GraphError::invalid("parent must be a transform"));
            }
            let mut a = new_parent;
            while a != NO_NODE {
                if a == child {
                    return Err(GraphError::invalid("cannot parent a node under itself"));
                }
                a = self.parent(a);
            }
        }
        let old_parent = self.parent(child);
        let old_sib = self.siblings(old_parent);
        let old_index = old_sib
            .iter()
            .position(|&x| x == child)
            .unwrap_or(old_sib.len());
        if old_parent == new_parent {
            let new_index = index.min(old_sib.len() - 1);
            if index == usize::MAX || new_index == old_index {
                return Ok(());
            }
            self.reparent_raw(child, new_parent, new_index);
            self.record(
                move |g| g.reparent_raw(child, old_parent, old_index),
                move |g| g.reparent_raw(child, new_parent, new_index),
            );
            return Ok(());
        }

        let world = if keep_world {
            self.evaluate();
            self.node(child).unwrap().matrix(Transform::WORLD_MATRIX)
        } else {
            DMat4::IDENTITY
        };

        self.reparent_raw(child, new_parent, index);
        let new_index = self
            .siblings(new_parent)
            .iter()
            .position(|&x| x == child)
            .unwrap();
        self.record(
            move |g| g.reparent_raw(child, old_parent, old_index),
            move |g| g.reparent_raw(child, new_parent, new_index),
        );
        // Sibling names stay unique, so only a clash under the new parent renames.
        let old_name = self.name_of(child).to_string();
        let fixed = self.unique_name(&old_name, new_parent, child, self.naming.unique_names);
        if fixed != old_name {
            self.rename_raw(child, &fixed);
            let f = fixed.clone();
            self.record(
                move |g| g.rename_raw(child, &old_name),
                move |g| g.rename_raw(child, &f),
            );
        }

        crate::rig::relink_inverse_scale(self, child);
        // A constrained object stays where its constraint puts it, which already accounts for the new parent.
        if keep_world && !crate::constraint::is_constrained(self, child) {
            let parent_world = if new_parent != NO_NODE {
                self.node(new_parent)
                    .unwrap()
                    .matrix(Transform::WORLD_MATRIX)
            } else {
                DMat4::IDENTITY
            };
            Transform::set_local_matrix(self, child, &(parent_world.inverse() * world))?;
        }
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Plugs

    /// "node.attr" or "node.tx", the node part resolved as `find` does.
    pub fn plug(&self, path: &str) -> Result<PlugRef> {
        let dot = path
            .find('.')
            .ok_or_else(|| GraphError::invalid(format!("expected node.attr, got '{path}'")))?;
        let node_part = &path[..dot];
        let id = self
            .find(node_part)?
            .ok_or_else(|| GraphError::invalid(format!("no node named '{node_part}'")))?;
        let n = self.node(id).unwrap();
        let mut attr_name: String = path[dot + 1..].to_string();
        // instObjGroups[0].objectGroups[3]: an object group's index is the one that counts, the instance's is left out.
        if let Some(rest) = attr_name.strip_prefix("iog[").or_else(|| attr_name.strip_prefix("instObjGroups[")) {
            if let Some((_, tail)) = rest.split_once("].") {
                if tail.starts_with("og[") {
                    attr_name = format!("iog.{tail}");
                } else if tail.starts_with("objectGroups[") {
                    attr_name = format!("instObjGroups.{tail}");
                }
            }
        }
        let mut element = -1;
        if let Some(close) = attr_name.find("].") {
            // group[3].child names the child attr "group.child", element 3.
            let open = attr_name[..close].rfind('[');
            element = open
                .and_then(|o| parse_index(&attr_name[o + 1..close]))
                .unwrap_or(-1);
            if element < 0 {
                return Err(GraphError::invalid(format!(
                    "bad element index in '{path}'"
                )));
            }
            let open = open.unwrap();
            attr_name = format!("{}.{}", &attr_name[..open], &attr_name[close + 2..]);
        } else if attr_name.ends_with(']') {
            let open = attr_name.rfind('[');
            element = open
                .and_then(|o| parse_index(&attr_name[o + 1..attr_name.len() - 1]))
                .unwrap_or(-1);
            if element < 0 {
                return Err(GraphError::invalid(format!(
                    "bad element index in '{path}'"
                )));
            }
            attr_name.truncate(open.unwrap());
        }
        for (i, a) in n.ty().attrs.iter().enumerate() {
            if a.name == attr_name || a.short_name == attr_name || a.aliases.iter().any(|x| *x == attr_name) {
                // The reference writes one-element outputs as arrays: worldMatrix[0], worldSpace[0].
                if element > 0 && !a.is_array() {
                    return Err(GraphError::invalid(format!(
                        "'{}.{}' is not an array",
                        n.name, a.name
                    )));
                }
                let element = if a.is_array() { element } else { -1 };
                return Ok(PlugRef {
                    node: id,
                    attr: i as i32,
                    component: -1,
                    element,
                });
            }
            if a.ty == PlugType::Vec3 && element < 0 {
                for c in 0..3 {
                    if a.child_names[c] == attr_name || a.child_short[c] == attr_name {
                        return Ok(PlugRef::component(id, i as i32, c as i32));
                    }
                }
            }
        }
        // group[3].child with either spelling of each half, or a child of a Vec3 element (input3D[3].input3Dx).
        if let (true, Some((group, child))) = (element >= 0, attr_name.split_once('.')) {
            for (i, a) in n.ty().attrs.iter().enumerate() {
                if a.ty == PlugType::Vec3 && a.is_array() && (a.name == group || a.short_name == group) {
                    for c in 0..3 {
                        if !child.is_empty() && (a.child_names[c] == child || a.child_short[c] == child) {
                            return Ok(PlugRef { node: id, attr: i as i32, component: c as i32, element });
                        }
                    }
                }
                // group[3].vec.child spelled out in long names.
                if a.ty == PlugType::Vec3 && a.is_array() {
                    if let Some((head, leaf)) = attr_name.rsplit_once('.') {
                        if a.name == head {
                            if let Some(c) = (0..3).find(|&c| a.child_names[c] == leaf || a.child_short[c] == leaf) {
                                return Ok(PlugRef { node: id, attr: i as i32, component: c as i32, element });
                            }
                        }
                    }
                }
                let (Some((lg, lc)), Some((sg, sc))) = (a.name.split_once('.'), a.short_name.split_once('.')) else { continue };
                if lg != group && sg != group {
                    continue;
                }
                if lc == child || sc == child {
                    return Ok(PlugRef { node: id, attr: i as i32, component: -1, element });
                }
                if a.ty == PlugType::Vec3 {
                    for c in 0..3 {
                        if a.child_names[c] == child || a.child_short[c] == child {
                            return Ok(PlugRef { node: id, attr: i as i32, component: c as i32, element });
                        }
                    }
                }
            }
        }
        if let Some(target) = n.aliases.get(attr_name.as_str()) {
            return self.plug(&format!("{node_part}.{target}"));
        }
        // A child of a plain compound by its own name: node.child for node.group.child.
        if element < 0 {
            let suffix = format!(".{attr_name}");
            if let Some(i) = n.ty().attrs.iter().position(|a| !a.is_array() && a.name.ends_with(&suffix)) {
                return Ok(PlugRef::new(id, i as i32));
            }
        }
        // pCube1.translate.translateX and cpm.result.position: the last part names the attr.
        if !attr_name.contains('[') {
            if let Some((_, last)) = attr_name.rsplit_once('.') {
                return self.plug(&format!("{node_part}.{last}"));
            }
        }
        // Numbered attrs an array replaced, as older scene files and scripts name them.
        if element < 0 {
            if let Some((i, Some(k))) = n.ty().file_attr_index(&attr_name) {
                return Ok(PlugRef::element(id, i as i32, k));
            }
        }
        // As in the reference, a transform answers for its shape's attributes: curve1.spans is curveShape1.spans.
        if n.ty().is_a("transform") {
            for &c in self.children(id) {
                let Some(k) = self.node(c) else { continue };
                if !k.ty().dag || k.ty().is_a("transform") || k.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT) {
                    continue;
                }
                for (i, a) in k.ty().attrs.iter().enumerate() {
                    if a.name == attr_name || a.short_name == attr_name {
                        let element = if a.is_array() { element } else { -1 };
                        return Ok(PlugRef { node: c, attr: i as i32, component: -1, element });
                    }
                }
            }
        }
        // An alias, as blendShape1.pSphere2 is blendShape1.weight[0].
        if let Some((_, target)) = self.aliases(id).into_iter().find(|(alias, _)| *alias == attr_name) {
            return self.plug(&format!("{node_part}.{target}"));
        }
        Err(GraphError::invalid(format!(
            "'{}' has no attribute '{attr_name}'",
            n.name
        )))
    }

    /// A node's attribute aliases: (alias, attribute as weight[0]).
    pub fn aliases(&self, id: NodeId) -> Vec<(String, String)> {
        self.node(id).map(|n| n.aliases().iter().map(|(a, t)| (a.clone(), t.clone())).collect()).unwrap_or_default()
    }

    /// The alias of a plug, if it has one.
    pub fn alias_of(&self, p: &PlugRef) -> Option<String> {
        let n = self.node(p.node)?;
        let a = &n.ty().attrs[p.attr as usize];
        let name = if p.element >= 0 { format!("{}[{}]", a.name, p.element) } else { a.name.clone() };
        self.aliases(p.node).into_iter().find(|x| x.1 == name).map(|x| x.0)
    }

    /// Writes a value derived from other nodes (a solver's copy of its joints): no undo entry, since undoing the edit
    /// it came from derives it again.
    pub(crate) fn set_derived(&mut self, p: &PlugRef, v: &PlugValue) {
        self.set_raw(p, v);
    }

    /// `connect` without an undo entry or checks, for a connection that follows from the hierarchy.
    pub(crate) fn connect_derived(&mut self, src: &PlugRef, dst: &PlugRef) {
        self.disconnect_raw(dst);
        self.connect_raw(src, dst);
    }

    pub(crate) fn disconnect_derived(&mut self, dst: &PlugRef) {
        self.disconnect_raw(dst);
    }

    /// The node's shortest unique path, a dot and the attr.
    pub fn plug_name(&self, p: &PlugRef) -> String {
        match self.node(p.node) {
            Some(n) => format!(
                "{}.{}",
                self.path(p.node),
                attr_part(&n.ty().attrs[p.attr as usize], p)
            ),
            None => "?".into(),
        }
    }

    /// `plug_name` with the node's full path: stable while other nodes come and go, for files and baselines.
    pub fn full_plug_name(&self, p: &PlugRef) -> String {
        match self.node(p.node) {
            Some(n) => format!(
                "{}.{}",
                self.full_path(p.node),
                attr_part(&n.ty().attrs[p.attr as usize], p)
            ),
            None => "?".into(),
        }
    }

    pub fn attr(&self, p: &PlugRef) -> &AttrDef {
        &self.nodes[&p.node].data.ty.attrs[p.attr as usize]
    }

    fn element_slot(&mut self, p: &PlugRef) -> &mut PlugValue {
        let n = self.nodes.get_mut(&p.node).unwrap();
        let count = n.data.ty.attrs.len();
        if n.data.elements.len() < count {
            n.data.elements.resize_with(count, Vec::new);
        }
        let default = n.data.ty.attrs[p.attr as usize].default_value.clone();
        let els = &mut n.data.elements[p.attr as usize];
        let at = els.partition_point(|(i, _)| *i < p.element);
        if at == els.len() || els[at].0 != p.element {
            els.insert(at, (p.element, default));
        }
        &mut els[at].1
    }

    fn erase_element_raw(&mut self, p: &PlugRef) {
        let Some(n) = self.nodes.get_mut(&p.node) else {
            return;
        };
        if (p.attr as usize) >= n.data.elements.len() {
            return;
        }
        n.data.elements[p.attr as usize].retain(|(i, _)| *i != p.element);
        self.mark_dirty(p.node);
        self.changed(Change::Value, p.node, Some(*p));
    }

    fn set_elements_raw(&mut self, array: &PlugRef, elements: &[(i32, PlugValue)]) {
        let n = self.node_mut(array.node);
        let count = n.data.ty.attrs.len();
        if n.data.elements.len() < count {
            n.data.elements.resize_with(count, Vec::new);
        }
        n.data.elements[array.attr as usize] = elements.to_vec();
        self.mark_dirty(array.node);
        self.changed(Change::Value, array.node, Some(*array));
    }

    fn set_raw(&mut self, p: &PlugRef, v: &PlugValue) {
        if p.element >= 0 {
            let slot = self.element_slot(p);
            if p.component >= 0 {
                if let PlugValue::Vec3(x) = slot {
                    x[p.component as usize] = v.scalar().unwrap_or(0.0);
                }
            } else {
                *slot = v.clone();
            }
        } else {
            let slot = &mut self.node_mut(p.node).data.values[p.attr as usize];
            if p.component >= 0 {
                if let PlugValue::Vec3(x) = slot {
                    x[p.component as usize] = v.scalar().unwrap_or(0.0);
                }
            } else {
                *slot = v.clone();
            }
        }
        self.mark_dirty(p.node);
        self.changed(Change::Value, p.node, Some(*p));
    }

    // Locks

    pub fn is_locked(&self, p: &PlugRef) -> bool {
        let Some(n) = self.node(p.node) else {
            return false;
        };
        if p.attr < 0 {
            return false;
        }
        let bits = lock_bits(&n.ty().attrs[p.attr as usize], p.component);
        n.lock_mask(p.attr as usize) & bits == bits
    }

    /// True when the plug or any of its components is locked.
    pub fn any_locked(&self, p: &PlugRef) -> bool {
        self.node(p.node).is_some_and(|n| {
            p.attr >= 0
                && n.lock_mask(p.attr as usize)
                    & lock_bits(&n.ty().attrs[p.attr as usize], p.component)
                    != 0
        })
    }

    /// The reference's attribute lock: a locked plug rejects `set_value`. A whole Vec3 counts as locked when all three
    /// components are; setting it fails only when a locked component would change. Undoable.
    pub fn set_locked(&mut self, p: &PlugRef, locked: bool) -> Result<()> {
        let n = self
            .node(p.node)
            .filter(|_| p.attr >= 0)
            .ok_or_else(|| GraphError::invalid("no such plug"))?;
        let old = n.lock_mask(p.attr as usize);
        let bits = lock_bits(&n.ty().attrs[p.attr as usize], p.component);
        let now = if locked { old | bits } else { old & !bits };
        if now == old {
            return Ok(());
        }
        let (id, attr) = (p.node, p.attr as usize);
        self.set_lock_raw(id, attr, now);
        self.record(
            move |g| g.set_lock_raw(id, attr, old),
            move |g| g.set_lock_raw(id, attr, now),
        );
        Ok(())
    }

    fn set_lock_raw(&mut self, id: NodeId, attr: usize, mask: u8) {
        let Some(n) = self.nodes.get_mut(&id) else {
            return;
        };
        if n.locks.len() <= attr {
            let len = n.data.ty.attrs.len().max(attr + 1);
            n.locks.resize(len, 0);
        }
        n.locks[attr] = mask;
        self.changed(Change::Value, id, Some(PlugRef::new(id, attr as i32)));
    }

    /// v with the plug's locked parts put back to their stored values, for tools that move only what they may.
    pub fn keep_locked(&self, p: &PlugRef, mut v: PlugValue) -> PlugValue {
        let Some(n) = self.node(p.node) else { return v };
        if p.attr < 0 {
            return v;
        }
        let mask = n.lock_mask(p.attr as usize);
        if mask == 0 {
            return v;
        }
        let stored = n.value(p.attr as usize);
        if self.is_locked(p) {
            return if p.component >= 0 {
                PlugValue::Float(vec3_component(stored, p.component))
            } else {
                stored.clone()
            };
        }
        if let (PlugValue::Vec3(x), true) = (&mut v, p.component < 0) {
            let s = stored.as_vec3();
            for c in 0..3 {
                if mask & (1 << c) != 0 {
                    x[c] = s[c];
                }
            }
        }
        v
    }

    // Values

    /// Errors when the plug is driven by a connection, unless the driven-set hook takes the edit. Setting a whole Vec3
    /// whose components are driven stores it and offers each changed driven component to the hook.
    pub fn set_value(&mut self, p: PlugRef, mut v: PlugValue) -> Result<()> {
        let n = self
            .node(p.node)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        let a = n.ty().attrs[p.attr as usize].clone();
        if !a.has(flags::INPUT) {
            return Err(GraphError::invalid(format!(
                "{} is an output and can't be set",
                self.plug_name(&p)
            )));
        }
        if a.is_array() && !a.has(flags::STORABLE) {
            return Err(GraphError::invalid(format!(
                "{} is an array input and takes connections, not values",
                self.plug_name(&p)
            )));
        }
        if a.is_array() && p.element < 0 {
            let name = self.plug_name(&p);
            return Err(GraphError::invalid(format!(
                "{name} is an array: set one element ({name}[0]), or all with setElements"
            )));
        }
        if matches!(a.ty, PlugType::Int | PlugType::Float | PlugType::Vec3) {
            a.clamp(&mut v);
        }
        let mask = n.lock_mask(p.attr as usize);
        if mask != 0 {
            match (&v, p.component >= 0) {
                (PlugValue::Vec3(x), false) => {
                    let s = n.vec3(p.attr as usize);
                    for c in 0..3 {
                        if mask & (1 << c) != 0 && x[c] != s[c] {
                            return Err(GraphError::invalid(format!(
                                "{} is locked and can't be changed",
                                self.plug_name(&PlugRef::component(p.node, p.attr, c as i32))
                            )));
                        }
                    }
                }
                _ => {
                    if self.is_locked(&p) {
                        return Err(GraphError::invalid(format!(
                            "{} is locked and can't be changed",
                            self.plug_name(&p)
                        )));
                    }
                }
            }
        }
        if let Some(src) = self.source(&p) {
            if let (PlugValue::Vec3(vec), true) = (&v, p.component < 0) {
                if !self.set_driven_vec(&p, &src, *vec) {
                    return Err(GraphError::invalid(format!(
                        "{} is driven by a connection",
                        self.plug_name(&p)
                    )));
                }
                return Ok(());
            }
            match v.scalar() {
                Some(d) if self.set_driven(&p, &src, d) => return Ok(()),
                _ => {
                    return Err(GraphError::invalid(format!(
                        "{} is driven by a connection",
                        self.plug_name(&p)
                    )))
                }
            }
        }
        if p.component >= 0 && p.element < 0 {
            if let Some(src) = self.source(&p.whole()) {
                let mut whole = self.value(&p.whole())?.as_vec3();
                let Some(d) = v.scalar() else {
                    return Err(GraphError::invalid(format!(
                        "{} is driven by a connection",
                        self.plug_name(&p)
                    )));
                };
                whole[p.component as usize] = d;
                if !self.set_driven_vec(&p.whole(), &src, whole) {
                    return Err(GraphError::invalid(format!(
                        "{} is driven by a connection",
                        self.plug_name(&p)
                    )));
                }
                return Ok(());
            }
        }

        let n = self.node(p.node).unwrap();
        if p.element >= 0 {
            let was = n.element(p.attr as usize, p.element).cloned();
            // One component of a Vec3 element: the element as a whole changes.
            let (p, v) = if p.component >= 0 {
                let whole = PlugRef { component: -1, ..p };
                if self.source(&whole).is_some() {
                    return Err(GraphError::invalid(format!("{} is driven by a connection", self.plug_name(&whole))));
                }
                let mut x = was.as_ref().unwrap_or(&a.default_value).as_vec3();
                x[p.component as usize] = v.scalar().unwrap_or(0.0);
                (whole, PlugValue::Vec3(x))
            } else {
                (p, v)
            };
            if was.as_ref() == Some(&v) {
                return Ok(());
            }
            self.set_raw(&p, &v);
            self.record(
                move |g| match &was {
                    Some(old) => g.set_raw(&p, old),
                    None => g.erase_element_raw(&p),
                },
                move |g| g.set_raw(&p, &v),
            );
            return Ok(());
        }
        let stored = n.value(p.attr as usize);
        let old = if p.component >= 0 {
            PlugValue::Float(vec3_component(stored, p.component))
        } else {
            stored.clone()
        };
        if old == v {
            return Ok(());
        }
        self.set_raw(&p, &v);
        let held = if matches!(
            v,
            PlugValue::Mesh(_) | PlugValue::Data(_) | PlugValue::String(_)
        ) {
            vec![old.clone(), v.clone()]
        } else {
            Vec::new()
        };
        let (o, nv) = (old.clone(), v.clone());
        self.record_held(
            move |g| g.set_raw(&p, &o),
            move |g| g.set_raw(&p, &nv),
            &held,
        );

        if p.component < 0 && a.ty == PlugType::Vec3 {
            let (now, was) = (v.as_vec3(), old.as_vec3());
            for c in 0..3 {
                let comp = PlugRef::component(p.node, p.attr, c);
                if let Some(src) = self.source(&comp) {
                    if now[c as usize] != was[c as usize] {
                        self.set_driven(&comp, &src, now[c as usize]);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn set_driven_set_hook(&mut self, f: DrivenSetHook) {
        self.driven_set = Some(f);
    }
    pub fn set_driven_vec_set_hook(&mut self, f: DrivenVecSetHook) {
        self.driven_vec_set = Some(f);
    }
    /// Called by the outermost `delete_node` with the node and its descendants before anything is disconnected, so data
    /// other nodes depend on can move. Edits it makes record undo.
    pub fn set_delete_hook(&mut self, f: DeleteHook) {
        self.delete_hook = Some(f);
    }
    /// True while a driven plug shows a held value instead of its source's.
    pub fn is_held(&self, dst: &PlugRef) -> bool {
        self.held.contains(dst)
    }

    /// Routes a set on a driven scalar through the hook. False when it is rejected.
    fn set_driven(&mut self, dst: &PlugRef, src: &PlugRef, v: f64) -> bool {
        let edit = match self.driven_set.clone() {
            Some(h) => h(self, dst, src, v),
            None => DrivenEdit::Reject,
        };
        if edit == DrivenEdit::Reject {
            return false;
        }
        if edit == DrivenEdit::Hold {
            let was = self.held.contains(dst);
            let slot = self.nodes[&dst.node].value(dst.attr as usize);
            let old = if dst.component >= 0 {
                vec3_component(slot, dst.component)
            } else {
                slot.scalar().unwrap_or(0.0)
            };
            let d = *dst;
            self.hold_raw(&d, &PlugValue::Float(v));
            self.record(
                move |g| {
                    if was {
                        g.hold_raw(&d, &PlugValue::Float(old))
                    } else {
                        g.release_raw(&d)
                    }
                },
                move |g| g.hold_raw(&d, &PlugValue::Float(v)),
            );
        }
        true
    }

    fn set_driven_vec(&mut self, dst: &PlugRef, src: &PlugRef, v: DVec3) -> bool {
        if src.component >= 0 || self.attr(src).ty != PlugType::Vec3 {
            return false;
        }
        let edit = match self.driven_vec_set.clone() {
            Some(h) => h(self, dst, src, v),
            None => DrivenEdit::Reject,
        };
        if edit == DrivenEdit::Reject {
            return false;
        }
        if edit == DrivenEdit::Hold {
            let was = self.held.contains(dst);
            let old = self.nodes[&dst.node].vec3(dst.attr as usize);
            let d = *dst;
            self.hold_raw(&d, &PlugValue::Vec3(v));
            self.record(
                move |g| {
                    if was {
                        g.hold_raw(&d, &PlugValue::Vec3(old))
                    } else {
                        g.release_raw(&d)
                    }
                },
                move |g| g.hold_raw(&d, &PlugValue::Vec3(v)),
            );
        }
        true
    }

    fn hold_raw(&mut self, dst: &PlugRef, v: &PlugValue) {
        let slot = &mut self.node_mut(dst.node).data.values[dst.attr as usize];
        match (v, dst.component >= 0) {
            (PlugValue::Vec3(x), _) => *slot = PlugValue::Vec3(*x),
            (_, true) => {
                if let PlugValue::Vec3(x) = slot {
                    x[dst.component as usize] = v.scalar().unwrap_or(0.0);
                }
            }
            (_, false) => slot.assign_scalar(v.scalar().unwrap_or(0.0)),
        }
        self.mark_dirty(dst.node);
        self.held.insert(*dst);
        self.changed(Change::Value, dst.node, Some(*dst));
    }

    fn release_raw(&mut self, dst: &PlugRef) {
        self.held.remove(dst);
        self.mark_dirty(dst.node);
        self.changed(Change::Value, dst.node, Some(*dst));
    }

    /// Raw stored value, no evaluation.
    pub fn stored_value(&self, p: &PlugRef) -> &PlugValue {
        self.nodes[&p.node].value(p.attr as usize)
    }

    /// Evaluates first if the node is dirty. Component plugs return a float.
    pub fn value(&mut self, p: &PlugRef) -> Result<PlugValue> {
        let n = self
            .node(p.node)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        // Only what this plug depends on: reading a joint mid-drag must not recompute the skinned meshes downstream.
        if self.any_dirty && n.dirty {
            self.evaluate_with(Some(&[p.node]));
        }
        let n = self.node(p.node).unwrap();
        if p.element >= 0 {
            return Ok(stored_plug(n, p));
        }
        let v = n.value(p.attr as usize);
        Ok(if p.component >= 0 {
            PlugValue::Float(vec3_component(v, p.component))
        } else {
            v.clone()
        })
    }

    pub fn connections(&self) -> Vec<(PlugRef, PlugRef)> {
        self.inputs.iter().map(|(d, s)| (*d, *s)).collect()
    }

    /// Whether reading `src` needs `dst`. An output depends on all its node's inputs and on its parents; a plain input
    /// only on what drives it, so `pCube1.tx -> multiplyDivide1 -> pCube1.ty` is fine and
    /// `pCube1.worldMatrix -> multMatrix1 -> pCube1.tx` is not. A direct attribute to attribute link back to `dst`'s node
    /// stays refused, as it always was here.
    pub(crate) fn reads_depend_on(&self, src: &PlugRef, dst: &PlugRef) -> bool {
        let dst_computes_nothing = self.node(dst.node).is_some_and(|n| computes_nothing(n.type_arc(), dst.attr));
        let mut stack = vec![(*src, src.node != dst.node)];
        let mut seen_nodes = HashSet::new();
        let mut seen_plugs = HashSet::new();
        while let Some((p, direct)) = stack.pop() {
            let Some(n) = self.node(p.node) else { continue };
            if !seen_plugs.insert(p) {
                continue;
            }
            if from_parent(n, p.attr) {
                let parent = self.parent(p.node);
                if parent != NO_NODE {
                    stack.push((PlugRef::new(parent, Transform::WORLD_MATRIX as i32), false));
                }
                continue;
            }
            if from_pivots(n, p.attr) {
                stack.extend(pivot_plugs(&p).map(|q| (q, direct)));
                continue;
            }
            let a = &n.ty().attrs[p.attr as usize];
            if passes_through(a) {
                let same_element = p.element < 0 || dst.element < 0 || p.element == dst.element;
                let same_part = p.component < 0 || dst.component < 0 || p.component == dst.component;
                if p.node == dst.node && (direct || (p.attr == dst.attr && same_element && same_part)) {
                    return true;
                }
                stack.extend(feeds_of(&self.inputs, &p).into_iter().map(|s| (s, direct)));
                continue;
            }
            if p.node == dst.node && !dst_computes_nothing {
                return true;
            }
            if !seen_nodes.insert(p.node) {
                continue;
            }
            let ty = n.type_arc();
            stack.extend(inputs_of(&self.inputs, p.node).filter(|(d, _)| !computes_nothing(ty, d.attr)).map(|(_, s)| (*s, false)));
            let parent = self.parent(p.node);
            if parent != NO_NODE && !n.ty().free_of_parent {
                stack.push((PlugRef::new(parent, Transform::WORLD_MATRIX as i32), false));
            }
        }
        false
    }

    // Connections

    fn connect_raw(&mut self, src: &PlugRef, dst: &PlugRef) {
        if dst.element >= 0 && self.nodes.contains_key(&dst.node) {
            self.element_slot(dst);
        }
        self.held.remove(dst);
        self.wiring += 1;
        self.inputs.insert(*dst, *src);
        self.outgoing_dst.entry(src.node).or_default().push(*dst);
        self.mark_dirty(dst.node);
        self.changed(Change::Connection, dst.node, Some(*dst));
    }

    fn disconnect_raw(&mut self, dst: &PlugRef) {
        self.wiring += 1;
        let Some(src) = self.inputs.remove(dst) else {
            return;
        };
        if let Some(v) = self.outgoing_dst.get_mut(&src.node) {
            v.retain(|d| d != dst);
        }
        self.held.remove(dst);
        if let Some(n) = self.nodes.get(&dst.node) {
            let a = &n.data.ty.attrs[dst.attr as usize];
            // A storable array keeps the element, holding the last value pulled through; an input-only one drops it.
            if dst.element >= 0 {
                if !a.has(flags::STORABLE) {
                    self.erase_element_raw(dst);
                }
            } else if a.has(flags::RESET_ON_DISCONNECT) {
                let d = a.default_value.clone();
                self.node_mut(dst.node).data.values[dst.attr as usize] = d;
            }
        }
        self.mark_dirty(dst.node);
        self.changed(Change::Connection, dst.node, Some(*dst));
    }

    /// Either end may be one component of a Vec3 (node.tx), which carries a float. A component destination and its
    /// whole Vec3 can't both be driven; `force` replaces whichever is there. A destination that is a whole array input
    /// gets a new element after its last one. Returns the destination connected.
    pub fn connect(&mut self, src: &PlugRef, to: &PlugRef, force: bool) -> Result<PlugRef> {
        if self.node(src.node).is_none() || self.node(to.node).is_none() {
            return Err(GraphError::invalid("no such node"));
        }
        let sa = self.attr(src).clone();
        let da = self.attr(to).clone();
        if !da.has(flags::INPUT) {
            return Err(GraphError::invalid(format!(
                "{} is not an input",
                self.plug_name(to)
            )));
        }
        if to.element >= 0 && !da.is_array() {
            return Err(GraphError::invalid(format!(
                "{} is not an array",
                self.plug_name(&to.whole())
            )));
        }
        if da.is_array() && to.component >= 0 && to.element < 0 {
            return Err(GraphError::invalid("array inputs have no components"));
        }
        let mut dst = *to;
        if da.is_array() && dst.element < 0 {
            let used = self.elements(&dst);
            dst.element = used.last().map(|e| e.element + 1).unwrap_or(0);
        }
        if (src.component >= 0 && sa.ty != PlugType::Vec3)
            || (dst.component >= 0 && da.ty != PlugType::Vec3)
        {
            return Err(GraphError::invalid("only vec3 plugs have components"));
        }
        let st = if src.component >= 0 {
            PlugType::Float
        } else {
            sa.ty
        };
        let dt = if dst.component >= 0 {
            PlugType::Float
        } else {
            da.ty
        };
        let numeric = |t: PlugType| {
            matches!(
                t,
                PlugType::Float | PlugType::Int | PlugType::Bool | PlugType::Enum
            )
        };
        let generic = (sa.has(flags::GENERIC) || da.has(flags::GENERIC)) && [st, dt].iter().all(|&t| numeric(t) || t == PlugType::Vec3);
        if st != dt && !(numeric(st) && numeric(dt)) && !generic {
            return Err(GraphError::invalid(format!(
                "type mismatch: {} -> {}",
                type_name(st),
                type_name(dt)
            )));
        }
        if self.reads_depend_on(src, &dst) {
            return Err(GraphError::invalid("connection would create a cycle"));
        }

        let mut clashing = Vec::new();
        if dst.component >= 0 {
            let whole = PlugRef { component: -1, ..dst };
            if self.source(&whole).is_some() {
                clashing.push(whole);
            }
        } else if da.ty == PlugType::Vec3 {
            for c in 0..3 {
                let comp = PlugRef { component: c, ..dst };
                if self.source(&comp).is_some() {
                    clashing.push(comp);
                }
            }
        }
        if !clashing.is_empty() {
            if !force {
                return Err(GraphError::invalid(format!(
                    "{} overlaps a driven plug; use force",
                    self.plug_name(&dst)
                )));
            }
            for c in clashing {
                self.disconnect(&c);
            }
        }

        if let Some(existing) = self.source(&dst) {
            if existing == *src {
                return Ok(dst);
            }
            if !force {
                return Err(GraphError::invalid(format!(
                    "{} is already connected; use force",
                    self.plug_name(&dst)
                )));
            }
            self.disconnect(&dst);
        }
        // Undo puts back the value the plug held before it was driven, not the last one pulled through the connection.
        let mut before = None;
        let mut new_element = false;
        // Meshes and data too: they are shared, not copied, and a mesh's first history node is undone this way. Left
        // out, an evaluation between the edit and its undo left the edited mesh on the shape.
        let heavy = false;
        if dst.element >= 0 {
            match self
                .node(dst.node)
                .unwrap()
                .element(dst.attr as usize, dst.element)
            {
                None => new_element = true,
                Some(v) if !heavy && dst.component >= 0 => before = Some(PlugValue::Float(vec3_component(v, dst.component))),
                Some(v) if !heavy => before = Some(v.clone()),
                Some(_) => {}
            }
        } else if !heavy {
            let v = self.node(dst.node).unwrap().value(dst.attr as usize);
            before = Some(if dst.component >= 0 {
                PlugValue::Float(vec3_component(v, dst.component))
            } else {
                v.clone()
            });
        }
        self.connect_raw(src, &dst);
        let s = *src;
        self.record(
            move |g| {
                g.disconnect_raw(&dst);
                if new_element {
                    g.erase_element_raw(&dst);
                } else if let Some(b) = &before {
                    if g.nodes.contains_key(&dst.node) {
                        g.set_raw(&dst, b);
                    }
                }
            },
            move |g| g.connect_raw(&s, &dst),
        );
        Ok(dst)
    }

    pub fn disconnect(&mut self, dst: &PlugRef) {
        let Some(s) = self.source(dst) else { return };
        let d = *dst;
        self.disconnect_raw(&d);
        self.record(
            move |g| g.connect_raw(&s, &d),
            move |g| g.disconnect_raw(&d),
        );
    }

    pub fn source(&self, dst: &PlugRef) -> Option<PlugRef> {
        self.inputs.get(dst).copied()
    }

    pub fn destinations(&self, src: &PlugRef) -> Vec<PlugRef> {
        self.outgoing_dst
            .get(&src.node)
            .map(|dsts| {
                dsts.iter()
                    .filter(|d| self.inputs.get(d) == Some(src))
                    .copied()
                    .collect()
            })
            .unwrap_or_default()
    }

    // Arrays

    /// An array's elements, connected or holding a value, by ascending index.
    pub fn elements(&self, array: &PlugRef) -> Vec<PlugRef> {
        self.node(array.node)
            .map(|n| {
                n.elements(array.attr as usize)
                    .iter()
                    .map(|(i, _)| PlugRef::element(array.node, array.attr, *i))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// The element indices any child of a grouped array uses, ascending.
    pub fn group_indices(&self, id: NodeId, group: &str) -> Vec<i32> {
        let mut out = Vec::new();
        if let Some(n) = self.node(id) {
            for a in n.ty().group_children(group) {
                out.extend(n.elements(a).iter().map(|(i, _)| *i));
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Makes a storable array's elements values[0..n-1], dropping its other ones. Errors when one of them is connected.
    pub fn set_elements(&mut self, array: &PlugRef, values: &[PlugValue]) -> Result<()> {
        let n = self
            .node(array.node)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        let whole = array.whole();
        let a = &n.ty().attrs[array.attr as usize];
        if !a.is_array() || !a.has(flags::STORABLE) || !a.has(flags::INPUT) {
            return Err(GraphError::invalid(format!(
                "{} is not an array that holds values",
                self.plug_name(&whole)
            )));
        }
        if self.is_locked(&whole) {
            return Err(GraphError::invalid(format!(
                "{} is locked and can't be changed",
                self.plug_name(&whole)
            )));
        }
        for e in self.elements(&whole) {
            if self.source(&e).is_some() {
                return Err(GraphError::invalid(format!(
                    "{} is driven by a connection",
                    self.plug_name(&e)
                )));
            }
        }
        let before: Vec<(i32, PlugValue)> = n.elements(array.attr as usize).to_vec();
        let after: Vec<(i32, PlugValue)> = values
            .iter()
            .cloned()
            .enumerate()
            .map(|(i, v)| (i as i32, v))
            .collect();
        if before == after {
            return Ok(());
        }
        self.set_elements_raw(&whole, &after);
        self.record(
            move |g| g.set_elements_raw(&whole, &before),
            move |g| g.set_elements_raw(&whole, &after),
        );
        Ok(())
    }

    /// Removes an element: its connection and its value.
    pub fn remove_element(&mut self, e: &PlugRef) -> Result<()> {
        if self.node(e.node).is_none() || e.element < 0 {
            return Err(GraphError::invalid(
                "removeElement: expected an element plug (node.attr[n])",
            ));
        }
        self.disconnect(e);
        let Some(was) = self
            .node(e.node)
            .unwrap()
            .element(e.attr as usize, e.element)
            .cloned()
        else {
            return Ok(());
        };
        let e = *e;
        self.erase_element_raw(&e);
        self.record(
            move |g| g.set_raw(&e, &was),
            move |g| g.erase_element_raw(&e),
        );
        Ok(())
    }

    /// Gives node `to` the elements node `from` has on the array attr (both of one type): the values the unconnected
    /// ones hold, and with `share_sources` the connected ones' sources too.
    pub fn copy_elements(
        &mut self,
        from: NodeId,
        to: NodeId,
        attr: i32,
        share_sources: bool,
    ) -> Result<()> {
        let stores = self
            .node(from)
            .ok_or_else(|| GraphError::invalid("no such node"))?
            .ty()
            .attrs[attr as usize]
            .has(flags::STORABLE);
        for e in self.elements(&PlugRef::new(from, attr)) {
            let dst = PlugRef::element(to, attr, e.element);
            if let Some(src) = self.source(&e) {
                if share_sources {
                    self.connect(&src, &dst, true)?;
                }
            } else if stores {
                let v = self.value(&e)?;
                self.set_value(dst, v)?;
            }
        }
        Ok(())
    }

    // User attrs

    /// Every connection and held plug on id's attrs renumbered by map(old index).
    fn remap_attr_refs(&mut self, id: NodeId, map: impl Fn(i32) -> i32) {
        let moved = |mut p: PlugRef| {
            if p.node == id {
                p.attr = map(p.attr);
            }
            p
        };
        self.wiring += 1;
        self.inputs = std::mem::take(&mut self.inputs)
            .into_iter()
            .map(|(d, s)| (moved(d), moved(s)))
            .collect();
        for dsts in self.outgoing_dst.values_mut() {
            for d in dsts.iter_mut() {
                *d = moved(*d);
            }
        }
        self.held = std::mem::take(&mut self.held)
            .into_iter()
            .map(moved)
            .collect();
    }

    fn shift_attr_refs(&mut self, id: NodeId, from: i32, delta: i32) {
        self.remap_attr_refs(id, |a| if a >= from { a + delta } else { a });
    }

    /// The node's private type, made on first use.
    fn own_type_mut(&mut self, id: NodeId) -> &mut NodeType {
        let n = self.node_mut(id);
        if !n.own_type {
            let mut t = (*n.data.ty).clone();
            t.user_of = Some(registered(&n.data.ty));
            n.data.ty = Arc::new(t);
            n.own_type = true;
        }
        Arc::make_mut(&mut n.data.ty)
    }

    fn insert_attr_raw(&mut self, id: NodeId, index: usize, s: &AttrState) {
        self.own_type_mut(id);
        self.shift_attr_refs(id, index as i32, 1);
        let n = self.node_mut(id);
        Arc::make_mut(&mut n.data.ty)
            .attrs
            .insert(index, s.def.clone());
        n.data.values.insert(index, s.value.clone());
        if !n.locks.is_empty() || s.lock != 0 {
            let len = n.locks.len().max(index);
            n.locks.resize(len, 0);
            n.locks.insert(index, s.lock);
        }
        if !n.data.elements.is_empty() || !s.elements.is_empty() {
            let len = n.data.elements.len().max(index);
            n.data.elements.resize_with(len, Vec::new);
            n.data.elements.insert(index, s.elements.clone());
        }
        self.mark_dirty(id);
        self.changed(Change::Attrs, id, None);
    }

    fn remove_attr_raw(&mut self, id: NodeId, index: usize) -> AttrState {
        let n = self.node_mut(id);
        let s = AttrState {
            def: n.data.ty.attrs[index].clone(),
            value: n.data.values[index].clone(),
            elements: n.data.elements(index).to_vec(),
            lock: n.lock_mask(index),
        };
        Arc::make_mut(&mut n.data.ty).attrs.remove(index);
        n.data.values.remove(index);
        if index < n.locks.len() {
            n.locks.remove(index);
        }
        if index < n.data.elements.len() {
            n.data.elements.remove(index);
        }
        self.shift_attr_refs(id, index as i32 + 1, -1);
        self.mark_dirty(id);
        self.changed(Change::Attrs, id, None);
        s
    }

    fn check_attr_name(name: &str) -> Result<()> {
        // group.child: one level of grouping.
        let ok = match name.find('.') {
            None => Self::valid_name(name),
            Some(dot) => Self::valid_name(&name[..dot]) && Self::valid_name(&name[dot + 1..]),
        };
        if ok {
            Ok(())
        } else {
            Err(GraphError::invalid(format!(
                "invalid attribute name '{name}'"
            )))
        }
    }

    /// Adds an attr to one node, after its type's, flagged USER (plus INPUT and STORABLE). A Vec3 without child names
    /// gets <name>X/Y/Z. Errors when the name or short name is taken. Returns its index.
    pub fn add_attr(&mut self, id: NodeId, mut def: AttrDef) -> Result<usize> {
        let n = self
            .node(id)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        Self::check_attr_name(&def.name)?;
        let t = n.ty();
        for want in [&def.name, &def.short_name] {
            if !want.is_empty() && t.attr_index(want).is_some() {
                return Err(GraphError::invalid(format!(
                    "'{}' already has an attribute '{want}'",
                    n.name
                )));
            }
        }
        let dot = def.name.find('.');
        let group = dot.map(|d| &def.name[..d]).unwrap_or(&def.name);
        let clash = if dot.is_none() {
            !t.group_children(group).is_empty()
        } else {
            t.attr_index(group).is_some()
        };
        if clash {
            return Err(GraphError::invalid(format!(
                "'{}' already has an attribute '{group}'",
                n.name
            )));
        }
        def.flags |= flags::USER | flags::INPUT | flags::STORABLE;
        if def.has(flags::MESSAGE) {
            def.flags &= !flags::STORABLE;
        }
        if def.ty == PlugType::Vec3 {
            for (c, axis) in ['X', 'Y', 'Z'].into_iter().enumerate() {
                if def.child_names[c].is_empty() {
                    def.child_names[c] = format!("{}{axis}", def.name);
                }
            }
        }
        let index = t.attrs.len();
        let s = AttrState {
            value: def.default_value.clone(),
            def,
            elements: Vec::new(),
            lock: 0,
        };
        self.insert_attr_raw(id, index, &s);
        self.record(
            move |g| {
                g.remove_attr_raw(id, index);
            },
            move |g| g.insert_attr_raw(id, index, &s),
        );
        Ok(index)
    }

    fn touching(&self, p: &PlugRef) -> Vec<PlugRef> {
        self.inputs
            .iter()
            .filter(|(d, s)| {
                (d.node == p.node && d.attr == p.attr) || (s.node == p.node && s.attr == p.attr)
            })
            .map(|(d, _)| *d)
            .collect()
    }

    /// Removes a user attr with its connections and value. Later user attrs move down one index.
    pub fn delete_attr(&mut self, p: &PlugRef) -> Result<()> {
        let n = self
            .node(p.node)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        if !n.ty().attrs[p.attr as usize].has(flags::USER) {
            return Err(GraphError::invalid(format!(
                "{} belongs to the node's type; only attributes added with addAttr can be deleted",
                self.plug_name(&p.whole())
            )));
        }
        for dst in self.touching(p) {
            self.disconnect(&dst);
        }
        let (id, index) = (p.node, p.attr as usize);
        let s = self.remove_attr_raw(id, index);
        self.record(
            move |g| g.insert_attr_raw(id, index, &s),
            move |g| {
                g.remove_attr_raw(id, index);
            },
        );
        Ok(())
    }

    fn replace_attr_raw(&mut self, id: NodeId, index: usize, s: &AttrState) {
        self.own_type_mut(id).attrs[index] = s.def.clone();
        let n = self.node_mut(id);
        n.data.values[index] = s.value.clone();
        if !n.data.elements.is_empty() || !s.elements.is_empty() {
            let len = n.data.elements.len().max(index + 1);
            n.data.elements.resize_with(len, Vec::new);
            n.data.elements[index] = s.elements.clone();
        }
        if !n.locks.is_empty() || s.lock != 0 {
            let len = n.locks.len().max(index + 1);
            n.locks.resize(len, 0);
            n.locks[index] = s.lock;
        }
        self.mark_dirty(id);
        self.changed(Change::Attrs, id, None);
    }

    /// Replaces a user attr's definition: name, type, hints, flags. Connections, keys and drivers stay on it unless the
    /// type changes, which drops them; the value converts to the new type where it can and takes the new default where
    /// it can't. Undoable.
    pub fn edit_attr(&mut self, p: &PlugRef, mut def: AttrDef) -> Result<()> {
        let n = self
            .node(p.node)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        let old = n.ty().attrs[p.attr as usize].clone();
        if !old.has(flags::USER) {
            return Err(GraphError::invalid(format!(
                "{} belongs to the node's type; only attributes added with addAttr can be edited",
                self.plug_name(&p.whole())
            )));
        }
        Self::check_attr_name(&def.name)?;
        let t = n.ty();
        for want in [&def.name, &def.short_name] {
            if want.is_empty() {
                continue;
            }
            if t.attr_index(want).is_some_and(|at| at != p.attr as usize) {
                return Err(GraphError::invalid(format!(
                    "'{}' already has an attribute '{want}'",
                    n.name
                )));
            }
            for (i, a) in t.attrs.iter().enumerate() {
                if i != p.attr as usize
                    && a.ty == PlugType::Vec3
                    && (a.child_names.iter().any(|c| c == want)
                        || a.child_short.iter().any(|c| c == want))
                {
                    return Err(GraphError::invalid(format!(
                        "'{}' already has an attribute '{want}'",
                        n.name
                    )));
                }
            }
        }
        def.flags |= flags::USER | flags::INPUT | flags::STORABLE;
        if def.ty == PlugType::Vec3
            && (def.child_names[0].is_empty()
                || (old.ty == PlugType::Vec3
                    && def.child_names == old.child_names
                    && old.child_names[0] == format!("{}X", old.name)))
        {
            for (c, axis) in ['X', 'Y', 'Z'].into_iter().enumerate() {
                def.child_names[c] = format!("{}{axis}", def.name);
            }
        }
        if def.ty != PlugType::Vec3 {
            def.child_names = Default::default();
        }
        if !def.default_value.holds(def.ty) {
            let probe = AttrDef {
                ty: def.ty,
                default_value: default_for(def.ty),
                enum_names: def.enum_names.clone(),
                ..Default::default()
            };
            def.default_value = convert_value(&def.default_value, &probe);
        }
        let retyped = def.ty != old.ty || (def.flags & flags::ARRAY) != (old.flags & flags::ARRAY);
        if retyped {
            for dst in self.touching(p) {
                self.disconnect(&dst);
            }
        }
        let (id, index) = (p.node, p.attr as usize);
        let n = self.node(id).unwrap();
        let before = AttrState {
            def: old.clone(),
            value: n.value(index).clone(),
            elements: n.elements(index).to_vec(),
            lock: n.lock_mask(index),
        };
        let mut after = AttrState {
            value: convert_value(&before.value, &def),
            def: def.clone(),
            elements: Vec::new(),
            lock: if retyped { 0 } else { before.lock },
        };
        if def.is_array() {
            if old.is_array() {
                after.elements = before
                    .elements
                    .iter()
                    .map(|(i, v)| (*i, convert_value(v, &def)))
                    .collect();
            }
            after.value = def.default_value.clone();
        } else if old.is_array() {
            after.value = before
                .elements
                .first()
                .map(|(_, v)| convert_value(v, &def))
                .unwrap_or_else(|| def.default_value.clone());
        }
        self.replace_attr_raw(id, index, &after);
        self.record(
            move |g| g.replace_attr_raw(id, index, &before),
            move |g| g.replace_attr_raw(id, index, &after),
        );
        Ok(())
    }

    fn move_attr_raw(&mut self, id: NodeId, from: usize, to: usize) {
        fn rot<T: Default>(v: &mut Vec<T>, len: usize, from: usize, to: usize) {
            if v.is_empty() {
                return;
            }
            if v.len() < len {
                v.resize_with(len, T::default);
            }
            if from < to {
                v[from..=to].rotate_left(1);
            } else {
                v[to..=from].rotate_right(1);
            }
        }
        let n = self.node_mut(id);
        let len = n.data.ty.attrs.len();
        if from < to {
            n.data.values[from..=to].rotate_left(1);
        } else {
            n.data.values[to..=from].rotate_right(1);
        }
        rot(&mut n.data.elements, len, from, to);
        rot(&mut n.locks, len, from, to);
        let attrs = &mut Arc::make_mut(&mut n.data.ty).attrs;
        if from < to {
            attrs[from..=to].rotate_left(1);
        } else {
            attrs[to..=from].rotate_right(1);
        }
        let (f, t) = (from as i32, to as i32);
        self.remap_attr_refs(id, |a| {
            if a == f {
                t
            } else if f < t && a > f && a <= t {
                a - 1
            } else if t < f && a >= t && a < f {
                a + 1
            } else {
                a
            }
        });
        self.mark_dirty(id);
        self.changed(Change::Attrs, id, None);
    }

    /// Moves a user attr to index `to`, another user attr's place, shifting the ones between. Undoable.
    pub fn move_attr(&mut self, id: NodeId, from: usize, to: usize) -> Result<()> {
        let n = self
            .node(id)
            .ok_or_else(|| GraphError::invalid("no such node"))?;
        let attrs = &n.ty().attrs;
        for i in [from, to] {
            if i >= attrs.len() || !attrs[i].has(flags::USER) {
                return Err(GraphError::invalid(
                    "only attributes added with addAttr can be reordered, among themselves",
                ));
            }
        }
        if from == to {
            return Ok(());
        }
        self.move_attr_raw(id, from, to);
        self.record(
            move |g| g.move_attr_raw(id, to, from),
            move |g| g.move_attr_raw(id, from, to),
        );
        Ok(())
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Evaluation

    fn mark_dirty(&mut self, id: NodeId) {
        self.any_dirty = true;
        self.version.fetch_add(1, Ordering::Relaxed);
        // The start is walked even when dirty, once, to drop the held values on its outputs. With none held there is
        // nothing to do; loading a rig connects into the same high fan-out nodes thousands of times.
        if self.held.is_empty() && self.nodes.get(&id).is_some_and(|n| n.dirty) {
            return;
        }
        let mut first = true;
        let mut stack = vec![id];
        while let Some(cur) = stack.pop() {
            let Some(n) = self.nodes.get_mut(&cur) else {
                continue;
            };
            // Already dirty means everything downstream already is. A node can drive itself (a.tx -> a.foo), so this
            // has to stop the walk at the start node too when it comes back round.
            if n.dirty && !first {
                continue;
            }
            n.dirty = true;
            if let Some(dsts) = self.outgoing_dst.get(&cur) {
                for dst in dsts {
                    // A held value lasts only until its source changes.
                    if first {
                        self.held.remove(dst);
                    }
                    // A message link carries nothing to recompute.
                    if self.nodes.get(&dst.node).is_some_and(|d| d.data.ty.attrs[dst.attr as usize].has(flags::MESSAGE)) {
                        continue;
                    }
                    stack.push(dst.node);
                }
            }
            stack.extend(
                self.rel
                    .get(&cur)
                    .map(|r| r.children.as_slice())
                    .unwrap_or(&[]),
            );
            first = false;
        }
    }

    /// Marks a node for recompute on the next `evaluate()`, like an input change would.
    pub fn touch(&mut self, id: NodeId) {
        self.mark_dirty(id);
    }

    /// View-only turns (turntables): each node's parentMatrix is premultiplied by its matrix as it evaluates, so it and
    /// everything under it draws, picks and edits turned. Not undoable, not saved.
    pub fn set_display_turns(&mut self, turns: HashMap<NodeId, DMat4>) {
        let mut roots_changed = turns.len() != self.display_turns.len();
        let old: Vec<(NodeId, DMat4)> = self.display_turns.iter().map(|(k, v)| (*k, *v)).collect();
        for (id, m) in old {
            match turns.get(&id) {
                None => {
                    roots_changed = true;
                    self.mark_dirty(id);
                }
                Some(t) if *t != m => self.mark_dirty(id),
                _ => {}
            }
        }
        let new_ids: Vec<NodeId> = turns
            .keys()
            .filter(|k| !self.display_turns.contains_key(k))
            .copied()
            .collect();
        for id in new_ids {
            roots_changed = true;
            self.mark_dirty(id);
        }
        self.display_turns = turns;
        if !roots_changed {
            return;
        }
        self.turned.clear();
        let ids: Vec<NodeId> = self.display_turns.keys().copied().collect();
        for id in ids {
            self.turned.insert(id);
            let d = self.descendants(id);
            self.turned.extend(d);
        }
    }
    pub fn display_turns(&self) -> &HashMap<NodeId, DMat4> {
        &self.display_turns
    }

    pub fn time(&self) -> f64 {
        self.ctx.time
    }

    /// There is no event bus, so listeners on time go through whatever owns the graph.
    pub fn set_time(&mut self, seconds: f64) {
        if self.ctx.time == seconds {
            return;
        }
        self.ctx.time = seconds;
        self.held.clear();
        let timed: Vec<NodeId> = self
            .nodes
            .iter()
            .filter(|(_, n)| n.data.ty.time_dependent)
            .map(|(id, _)| *id)
            .collect();
        for id in timed {
            self.mark_dirty(id);
        }
        self.version.fetch_add(1, Ordering::Relaxed);
    }

    /// Counts evaluations that computed something.
    pub fn evaluation_pass(&self) -> u64 {
        self.evaluation_pass
    }
    /// Bumped on every change; the renderer uses it to skip unchanged frames.
    pub fn version(&self) -> u64 {
        self.version.load(Ordering::Relaxed)
    }
    /// Changes whenever a node is created or deleted or the graph is cleared, to a value no other graph has had.
    pub fn structure(&self) -> u64 {
        self.structure
    }
    /// Bumped by every change that can alter which nodes are hidden.
    pub fn visibility_version(&self) -> u64 {
        self.visibility_version
    }

    /// Called on the main thread by `evaluate()`: before computing anything dirty and once done. Evaluations the hook
    /// makes itself don't call it again. Simulations use it to step on the main thread.
    pub fn set_evaluate_hook(&mut self, hook: EvaluateHook) {
        self.evaluate_hook = Some(hook);
    }
    /// Further hooks, called after the one set above in the order added.
    pub fn add_evaluate_hook(&mut self, hook: EvaluateHook) {
        self.extra_evaluate_hooks.push(hook);
    }
    /// Stands in for `sim::sampling()`.
    pub fn set_sampling(&mut self, on: bool) {
        self.sampling = on;
    }
    /// Whether the graph is being sampled at other times (bakes, trails): simulations hold rather than step.
    pub fn sampling(&self) -> bool {
        self.sampling
    }
    pub fn set_active_frame_fn(&mut self, f: ActiveFrameFn) {
        self.active_frame = Some(f);
    }

    pub fn evaluate(&mut self) {
        if self.evaluation_suspended {
            return;
        }
        if DEFER_MESHES.with(|d| d.get()) > 0 && self.any_dirty {
            let roots: Vec<NodeId> = self
                .nodes
                .iter()
                .filter(|(_, n)| n.dirty && n.data.ty.dag && !n.data.ty.is_a("mesh"))
                .map(|(id, _)| *id)
                .collect();
            self.evaluate_with(Some(&roots));
            return;
        }
        self.evaluate_with(None);
    }

    /// Computes only what the given nodes depend on, leaving the rest dirty for the next `evaluate()`, without the
    /// evaluate hooks.
    /// Nodes an evaluate could not reach because they wait on each other. Their values are stale: a node-level loop.
    pub fn stuck_nodes(&self) -> Vec<NodeId> {
        self.nodes.iter().filter(|(_, n)| n.dirty).map(|(id, _)| *id).collect()
    }

    pub fn evaluate_upstream(&mut self, nodes: &[NodeId]) {
        self.evaluate_dirty(Some(nodes));
    }

    fn evaluate_with(&mut self, only: Option<&[NodeId]>) {
        let mut hooks: Vec<EvaluateHook> = self.evaluate_hook.iter().cloned().collect();
        hooks.extend(self.extra_evaluate_hooks.iter().cloned());
        if hooks.is_empty() || self.in_evaluate_hook {
            self.evaluate_dirty(only);
            return;
        }
        let call = |g: &mut Graph, after: bool| {
            g.in_evaluate_hook = true;
            for h in &hooks {
                h(g, after);
            }
            g.in_evaluate_hook = false;
        };
        if self.any_dirty {
            call(self, false);
        }
        self.evaluate_dirty(only);
        call(self, true);
    }

    /// The dirty nodes in levels that can each compute at once, and the inputs that only need passing on after.
    fn make_plan(&self, dirty: Vec<NodeId>) -> EvalPlan {
        let mut pending: HashMap<NodeId, i32> = dirty.iter().map(|&id| (id, 0)).collect();
        let mut downstream: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
        let mut add_edge = |up: NodeId, down: NodeId, pending: &mut HashMap<NodeId, i32>| {
            if up == down || !pending.contains_key(&up) {
                return;
            }
            downstream.entry(up).or_default().push(down);
            *pending.get_mut(&down).unwrap() += 1;
        };
        for &id in &dirty {
            let p = self.parent(id);
            if p != NO_NODE && !self.nodes.get(&id).is_some_and(|n| n.data.ty.free_of_parent) {
                add_edge(p, id, &mut pending);
            }
        }
        let mut reads = Vec::new();
        let mut late = Vec::new();
        for (dst, src) in &self.inputs {
            if pending.contains_key(&dst.node) {
                if self.nodes.get(&dst.node).is_some_and(|n| computes_nothing(&n.data.ty, dst.attr)) {
                    late.push(*dst);
                    continue;
                }
                reads.clear();
                read_after(&self.nodes, &self.rel, &self.inputs, src, &mut reads, 0);
                for &up in &reads {
                    add_edge(up, dst.node, &mut pending);
                }
            }
        }

        let mut level: Vec<NodeId> = dirty
            .iter()
            .copied()
            .filter(|id| pending[id] == 0)
            .collect();
        let mut levels = Vec::new();
        while !level.is_empty() {
            let mut next = Vec::new();
            for id in &level {
                if let Some(ds) = downstream.get(id) {
                    for d in ds {
                        let c = pending.get_mut(d).unwrap();
                        *c -= 1;
                        if *c == 0 {
                            next.push(*d);
                        }
                    }
                }
            }
            levels.push(std::mem::replace(&mut level, next));
        }
        EvalPlan { wiring: self.wiring, dirty, levels, late }
    }

    fn evaluate_dirty(&mut self, only: Option<&[NodeId]>) {
        if !self.any_dirty {
            return;
        }
        // Just the dirty nodes `only` depends on. Upstream of a clean node is clean, so the walk stops at clean nodes.
        let mut needed = HashSet::new();
        if let Some(only) = only {
            let mut stack = only.to_vec();
            while let Some(id) = stack.pop() {
                if !self.nodes.get(&id).is_some_and(|n| n.dirty) || !needed.insert(id) {
                    continue;
                }
                let p = self.parent(id);
                if p != NO_NODE && !self.nodes.get(&id).is_some_and(|n| n.data.ty.free_of_parent) {
                    stack.push(p);
                }
                stack.extend(inputs_of(&self.inputs, id).map(|(_, s)| s.node));
            }
            if needed.is_empty() {
                return;
            }
        }
        self.any_dirty = false;
        self.evaluation_pass += 1;
        let pass = self.evaluation_pass;
        // Partial and simulation-sampling evaluations compute only what a reader needs, not the scene at this time.
        let cache = self.playback.is_some() && only.is_none() && !self.sampling;

        // Kahn levels over the dirty subgraph; each level computes in parallel.
        let mut dirty = Vec::new();
        for (id, n) in &self.nodes {
            if n.dirty {
                if only.is_some() && !needed.contains(id) {
                    self.any_dirty = true;
                } else {
                    dirty.push(*id);
                }
            }
        }
        dirty.sort_unstable();
        // The order only depends on what's dirty and how things are wired, which a drag or playback repeats every frame.
        let wiring = self.wiring;
        self.plans.retain(|p| p.wiring == wiring);
        let plan = match self.plans.iter().position(|p| p.dirty == dirty && !p.levels.is_empty()) {
            Some(i) => self.plans.remove(i),
            None => self.make_plan(dirty),
        };
        for level in &plan.levels {
            // Plain inputs read from other nodes, while those are all still in the map.
            let mut passed = HashMap::new();
            for id in level {
                for (_, src) in inputs_of(&self.inputs, *id) {
                    let pass = self.nodes.get(&src.node).is_some_and(|n| from_parent(n, src.attr) || from_pivots(n, src.attr) || passes_through(&n.data.ty.attrs[src.attr as usize]));
                    if pass && !passed.contains_key(src) {
                        passed.insert(*src, resolve_input(&self.nodes, &self.rel, &self.inputs, &self.held, src, 0));
                    }
                }
            }
            let mut owned: Vec<Box<Node>> = level
                .iter()
                .map(|id| self.nodes.remove(id).unwrap())
                .collect();
            {
                let view = EvalView {
                    nodes: &self.nodes,
                    rel: &self.rel,
                    inputs: &self.inputs,
                    held: &self.held,
                    display_turns: &self.display_turns,
                    turned: &self.turned,
                    playback: self.playback.as_ref(),
                    active_frame: self.active_frame.as_ref(),
                    ctx: self.ctx,
                    pass,
                    cache,
                    passed: &passed,
                };
                if owned.len() < 16 {
                    for n in owned.iter_mut() {
                        view.pull(n);
                    }
                } else {
                    owned.par_iter_mut().for_each(|n| view.pull(n));
                }
            }
            for n in owned {
                self.nodes.insert(n.id, n);
            }
        }
        for &dst in &plan.late {
            if self.held.contains(&dst) {
                continue;
            }
            let v = resolve_input(&self.nodes, &self.rel, &self.inputs, &self.held, &dst, 0);
            let Some(n) = self.nodes.get_mut(&dst.node) else { continue };
            let slot = if dst.element >= 0 {
                match n.data.elements.get_mut(dst.attr as usize).and_then(|els| els.iter_mut().find(|(i, _)| *i == dst.element)) {
                    Some((_, x)) => x,
                    None => continue,
                }
            } else {
                &mut n.data.values[dst.attr as usize]
            };
            if dst.component >= 0 {
                if let (PlugValue::Vec3(x), Some(s)) = (&mut *slot, v.scalar()) {
                    x[dst.component as usize] = s;
                }
            } else if std::mem::discriminant(slot) == std::mem::discriminant(&v) {
                *slot = v;
            } else if let Some(s) = v.scalar() {
                slot.assign_scalar(s);
            }
        }
        self.plans.insert(0, plan);
        self.plans.truncate(32);
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Selection

    pub fn selection(&self) -> &[NodeId] {
        &self.selection
    }

    fn select_raw(&mut self, ids: Vec<NodeId>) {
        self.selection = ids;
        self.selection_serial += 1;
        self.version.fetch_add(1, Ordering::Relaxed);
        self.changed(Change::Selection, NO_NODE, None);
    }

    pub fn select(&mut self, ids: &[NodeId], mode: SelectMode) {
        let mut next = if mode == SelectMode::Replace {
            Vec::new()
        } else {
            self.selection.clone()
        };
        for &id in ids {
            if self.node(id).is_none() {
                continue;
            }
            let at = next.iter().position(|&x| x == id);
            match (mode, at) {
                (SelectMode::Replace | SelectMode::Add, None) => next.push(id),
                (SelectMode::Remove, Some(i)) => {
                    next.remove(i);
                }
                (SelectMode::Toggle, Some(i)) => {
                    next.remove(i);
                }
                (SelectMode::Toggle, None) => next.push(id),
                _ => {}
            }
        }
        if next == self.selection {
            return;
        }
        let old = self.selection.clone();
        self.select_raw(next.clone());
        self.undo.record(
            FnAction::new(
                move |g: &mut Graph| g.select_raw(old.clone()),
                move |g: &mut Graph| g.select_raw(next.clone()),
            )
            .selection(true),
        );
    }

    pub fn is_selected(&self, id: NodeId) -> bool {
        self.selection.contains(&id)
    }
    /// Bumped whenever the selection changes, including when a deleted node leaves it.
    pub fn selection_serial(&self) -> u64 {
        self.selection_serial
    }

    // ---------------------------------------------------------------------------------------------------------------
    // Listeners

    pub fn add_listener(&mut self, f: Listener, immediate: bool) -> i32 {
        let id = self.next_listener;
        self.next_listener += 1;
        self.listeners.insert(id, ListenerEntry { f, immediate });
        id
    }
    pub fn remove_listener(&mut self, id: i32) {
        self.listeners.remove(&id);
    }
    /// Inside a listener's Value or Connection call: the plug that changed, when one did.
    pub fn changed_plug(&self) -> Option<&PlugRef> {
        self.changed_plug.as_ref()
    }

    /// Scene loads and imports: batched listeners hear nothing until the end, then one Reset. Nests.
    pub fn begin_batch(&mut self) {
        self.batch_depth += 1;
    }
    pub fn end_batch(&mut self) {
        self.batch_depth -= 1;
        if self.batch_depth > 0 || !self.batch_pending {
            return;
        }
        self.batch_pending = false;
        let ls: Vec<Listener> = self
            .listeners
            .values()
            .filter(|l| !l.immediate)
            .map(|l| l.f.clone())
            .collect();
        for f in ls {
            f(self, Change::Reset, NO_NODE);
        }
    }
    pub fn batch<R>(&mut self, f: impl FnOnce(&mut Graph) -> R) -> R {
        self.begin_batch();
        let r = f(self);
        self.end_batch();
        r
    }
    pub fn batching(&self) -> bool {
        self.batch_depth > 0
    }

    fn changes_visibility(&self, c: Change, id: NodeId, plug: Option<&PlugRef>) -> bool {
        if matches!(c, Change::Selection | Change::Renamed) {
            return false;
        }
        if !matches!(c, Change::Value | Change::Connection) {
            return true;
        }
        let Some(plug) = plug else { return true };
        if !self.node(id).is_some_and(|n| n.ty().dag) {
            return true;
        }
        plug.attr as usize == Transform::VISIBILITY
            || plug.attr as usize == Transform::DISPLAY_LAYER
            || plug.attr as usize == crate::transform::DagNode::OVERRIDE_ENABLED
            || plug.attr as usize == crate::transform::DagNode::OVERRIDE_VISIBILITY
    }

    fn changed(&mut self, c: Change, id: NodeId, plug: Option<PlugRef>) {
        self.version.fetch_add(1, Ordering::Relaxed);
        if self.changes_visibility(c, plug.map(|p| p.node).unwrap_or(id), plug.as_ref()) {
            self.visibility_version += 1;
        }
        if c != Change::Selection && !self.sampling {
            self.clear_playback_cache();
        }
        if self.listeners.is_empty() {
            return;
        }
        let outer = std::mem::replace(&mut self.changed_plug, plug);
        let mut call = Vec::new();
        for l in self.listeners.values() {
            if self.batch_depth == 0 || l.immediate {
                call.push(l.f.clone());
            } else {
                self.batch_pending = true;
            }
        }
        for f in call {
            f(self, c, id);
        }
        self.changed_plug = outer;
    }

    pub fn set_naming_policy(&mut self, policy: NamingPolicy) {
        self.naming = policy;
    }
    pub fn naming_policy(&self) -> &NamingPolicy {
        &self.naming
    }
    /// NamingPolicy::unique_names: names unique across the scene rather than among siblings.
    pub fn unique_names(&self) -> bool {
        self.naming.unique_names
    }
}

fn physical_memory_bytes() -> usize {
    let mut s = sysinfo::System::new();
    s.refresh_memory();
    s.total_memory() as usize
}
