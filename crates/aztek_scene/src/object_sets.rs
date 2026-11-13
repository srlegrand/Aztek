//! The reference's object sets. An `objectSet` node (non-DAG) holds a list of members:
//! whole nodes, or vertices, edges or faces of a mesh by component id (the ids of the mesh's outMesh, which direct
//! edits keep up to date). quickSelect marks it as a quick select set. Game export takes a set as one exportable unit.
//!
//! Members are held by node id, so they follow renames. A deleted member drops out of every query and is left out when
//! the scene is saved; undoing the delete brings it back. Referenced members are rebound by name when their reference
//! reloads. In files, members are written by name, relative to the set's namespace when the set comes from a
//! reference.

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use aztek_geo::{MeshRemap, INVALID};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use serde_json::{json, Value};

use crate::components::{self, ComponentMode, Ids};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, EvalContext, Node, NodeBehavior, NodeData, NodeType};
use crate::plug::{data, flags, AttrDef, Data, DataPtr, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

pub const TYPE_NAME: &str = "objectSet";
pub const MEMBERS_KIND: &str = "objectSetMembers";

pub const QUICK_SELECT: usize = 0;
pub const MEMBERS: usize = 1;
/// Connected into a deformer's deformerSet when this is its set.
pub const USED_BY: usize = 2;

/// One member: a whole node (kind Object) or some of a mesh's components (ids sorted, unique).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub node: NodeId,
    pub kind: ComponentMode,
    pub ids: Ids,
}

impl Member {
    pub fn object(node: NodeId) -> Self {
        Member { node, kind: ComponentMode::Object, ids: Vec::new() }
    }
}

/// Binding cache, refreshed by bind(); the entry's meaning never changes.
#[derive(Debug, Clone, Default)]
struct Binding {
    id: NodeId,
    name: String,
    referenced: bool,
}

/// The stored member list. Entries remember the node's id and name: the id is authoritative while the node lives, the
/// name binds entries read from a file and rebinds referenced members after their reference reloads.
#[derive(Debug)]
struct Entry {
    bound: Mutex<Binding>,
    kind: ComponentMode,
    ids: Ids,
}

impl Clone for Entry {
    fn clone(&self) -> Self {
        Entry { bound: Mutex::new(self.bound.lock().unwrap().clone()), kind: self.kind, ids: self.ids.clone() }
    }
}

#[derive(Debug, Default)]
struct SetMembers {
    entries: Vec<Entry>,
}

fn kind_key(k: ComponentMode) -> &'static str {
    match k {
        ComponentMode::Vertex => "vertices",
        ComponentMode::Edge => "edges",
        ComponentMode::Face => "faces",
        ComponentMode::Object => "",
    }
}

fn kind_suffix(k: ComponentMode) -> &'static str {
    match k {
        ComponentMode::Vertex => "vtx",
        ComponentMode::Edge => "e",
        ComponentMode::Face => "f",
        ComponentMode::Object => "",
    }
}

impl SetMembers {
    fn json_with(&self, g: Option<&Graph>) -> Value {
        let mut list = Vec::new();
        for e in &self.entries {
            let b = e.bound.lock().unwrap().clone();
            let live = g.and_then(|g| if b.id != NO_NODE && g.node(b.id).is_some() { Some(if e.kind == ComponentMode::Object { g.path(b.id) } else { crate::components::display_path(g, b.id) }) } else { None });
            let name = match live {
                Some(n) => n,
                // unbound or its reference is unloaded: keep it for when it comes back
                None if b.id == NO_NODE || b.referenced || g.is_none() => b.name.clone(),
                None => continue,
            };
            let mut m = json!({"node": name});
            if e.kind != ComponentMode::Object {
                m[kind_key(e.kind)] = json!(e.ids);
            }
            list.push(m);
        }
        json!({"kind": MEMBERS_KIND, "members": list})
    }

    fn from_json(j: &Value) -> DataPtr {
        let mut out = SetMembers::default();
        for m in j.get("members").and_then(Value::as_array).into_iter().flatten() {
            let Some(name) = m.get("node").and_then(Value::as_str) else { continue };
            let mut e = Entry { bound: Mutex::new(Binding { name: name.into(), ..Default::default() }), kind: ComponentMode::Object, ids: Vec::new() };
            for k in [ComponentMode::Vertex, ComponentMode::Edge, ComponentMode::Face] {
                if let Some(ids) = m.get(kind_key(k)).and_then(Value::as_array) {
                    e.kind = k;
                    e.ids.extend(ids.iter().filter_map(Value::as_u64).map(|v| v as u32));
                }
            }
            out.entries.push(e);
        }
        Arc::new(out)
    }
}

impl Data for SetMembers {
    fn kind(&self) -> &str {
        MEMBERS_KIND
    }
    fn to_json(&self) -> Value {
        self.json_with(None)
    }
    fn to_json_in(&self, g: &Graph) -> Value {
        self.json_with(Some(g))
    }
    fn bytes(&self) -> usize {
        64 + self.entries.iter().map(|e| std::mem::size_of::<Entry>() + e.bound.lock().unwrap().name.len() + e.ids.len() * 4).sum::<usize>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Looks the entry's node up by name: relative to the set's namespace first, as files from a reference store them.
fn find_by_name(g: &Graph, set_namespace: &str, name: &str) -> Option<NodeId> {
    if !set_namespace.is_empty() {
        // ambiguous: unbound until it is made unique
        if let Ok(Some(id)) = g.find(&format!("{set_namespace}:{name}")) {
            return Some(id);
        }
    }
    g.find(name).ok().flatten()
}

/// The live node an entry means.
fn lookup(g: &Graph, set_namespace: &str, b: &Binding) -> Option<NodeId> {
    if b.id != NO_NODE && g.node(b.id).is_some() {
        return Some(b.id);
    }
    // A deleted scene node stays deleted; only file entries and reloaded references bind by name.
    if b.id != NO_NODE && !b.referenced {
        return None;
    }
    let n = find_by_name(g, set_namespace, &b.name)?;
    if b.id != NO_NODE && g.node(n).unwrap().reference_id() == 0 {
        return None;
    }
    Some(n)
}

/// lookup() that also refreshes the entry's cache.
fn bind(g: &Graph, set_namespace: &str, e: &Entry) -> Option<NodeId> {
    let mut b = e.bound.lock().unwrap();
    let mut n = lookup(g, set_namespace, &b)?;
    // Components saved under their transform's name belong to its shape.
    if e.kind != ComponentMode::Object {
        n = crate::components::as_mesh(g, n).unwrap_or(n);
    }
    b.id = n;
    b.name = g.path(n);
    b.referenced = g.node(n).unwrap().reference_id() != 0;
    Some(n)
}

fn stored(g: &Graph, set: NodeId) -> Option<Arc<dyn Data>> {
    let n = g.node(set)?;
    if !is_set(n) {
        return None;
    }
    n.value(MEMBERS).as_data().cloned()
}

fn with_stored<R>(g: &Graph, set: NodeId, f: impl FnOnce(&SetMembers) -> R) -> Option<R> {
    let d = stored(g, set)?;
    d.as_any().downcast_ref::<SetMembers>().map(f)
}

/// Live members, in joining order.
fn resolve(g: &Graph, set: NodeId) -> Vec<Member> {
    let Some(n) = g.node(set) else { return Vec::new() };
    let ns = n.namespace().to_string();
    with_stored(g, set, |s| {
        s.entries.iter().filter_map(|e| bind(g, &ns, e).map(|node| Member { node, kind: e.kind, ids: e.ids.clone() })).collect()
    })
    .unwrap_or_default()
}

fn normalize(ids: &mut Ids) {
    ids.sort_unstable();
    ids.dedup();
}

/// Writes a member list as the set's value, as one undoable set. Live members only.
fn store(g: &mut Graph, set: NodeId, list: &[Member]) -> CmdResult<()> {
    let ns = g.node(set).unwrap().namespace().to_string();
    let mut d = SetMembers::default();
    // Members of an unloaded reference aren't live, but still belong to the set.
    if let Some(kept) = with_stored(g, set, |old| {
        old.entries
            .iter()
            .filter(|e| {
                let b = e.bound.lock().unwrap().clone();
                (b.id == NO_NODE || b.referenced) && lookup(g, &ns, &b).is_none()
            })
            .cloned()
            .collect::<Vec<_>>()
    }) {
        d.entries = kept;
    }
    for m in list {
        let Some(n) = g.node(m.node) else { continue };
        d.entries.push(Entry {
            bound: Mutex::new(Binding { id: m.node, name: g.path(m.node), referenced: n.reference_id() != 0 }),
            kind: m.kind,
            ids: m.ids.clone(),
        });
    }
    g.set_value(PlugRef::new(set, MEMBERS as i32), PlugValue::Data(Some(Arc::new(d))))?;
    Ok(())
}

fn merged(mut list: Vec<Member>, extra: &[Member]) -> Vec<Member> {
    // Where each node and kind already is in the list: a set of thousands is merged in one pass, not one per member.
    let key = |m: &Member| (m.node, std::mem::discriminant(&m.kind));
    let mut at: std::collections::HashMap<_, usize> = std::collections::HashMap::with_capacity(list.len() + extra.len());
    for (i, m) in list.iter().enumerate() {
        at.entry(key(m)).or_insert(i);
    }
    for m in extra {
        let mut m = m.clone();
        normalize(&mut m.ids);
        if m.kind != ComponentMode::Object && m.ids.is_empty() {
            continue;
        }
        match at.get(&key(&m)) {
            None => {
                at.insert(key(&m), list.len());
                list.push(m);
            }
            Some(&i) if m.kind != ComponentMode::Object => {
                let x = &mut list[i];
                x.ids.extend(m.ids);
                normalize(&mut x.ids);
            }
            Some(_) => {}
        }
    }
    list
}

pub fn is_set(n: &Node) -> bool {
    n.ty().is_a(TYPE_NAME)
}

/// Every set in creation order; quick_select filters to quick select sets (true) or plain ones (false).
pub fn all(g: &Graph, quick_select: Option<bool>) -> Vec<NodeId> {
    let mut ids = g.nodes();
    ids.sort_unstable();
    ids.into_iter()
        .filter(|&id| {
            let n = g.node(id).unwrap();
            is_set(n) && quick_select.is_none_or(|q| n.bool(QUICK_SELECT) == q)
        })
        .collect()
}

/// Live members in the order they joined, one entry per node and kind.
pub fn members(g: &Graph, set: NodeId) -> Vec<Member> {
    resolve(g, set)
}

/// Distinct nodes the set touches: object members and the meshes its components belong to.
pub fn nodes_of(g: &Graph, set: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    for m in resolve(g, set) {
        if !out.contains(&m.node) {
            out.push(m.node);
        }
    }
    out
}

/// Sets that have the node as a member, whole or by its components.
pub fn sets_of(g: &Graph, node: NodeId) -> Vec<NodeId> {
    all(g, None).into_iter().filter(|&s| resolve(g, s).iter().any(|m| m.node == node)).collect()
}

/// The current mode's selected components, or the selected objects when there are none. Sets are left out.
pub fn from_selection(g: &Graph) -> Vec<Member> {
    let mut out = Vec::new();
    let mode = components::mode(g);
    if mode != ComponentMode::Object {
        for (&node, ids) in components::selected_all(g, mode) {
            if !ids.is_empty() && g.node(node).is_some() {
                out.push(Member { node, kind: mode, ids: ids.clone() });
            }
        }
    }
    if !out.is_empty() {
        return out;
    }
    g.selection().iter().copied().filter(|&id| g.node(id).is_some_and(|n| !is_set(n))).map(Member::object).collect()
}

/// The ids of "3" or "0:7".
fn plain_ids(range: &str) -> Option<Ids> {
    let (a, b) = range.split_once(':').unwrap_or((range, range));
    let (a, b): (u32, u32) = (a.parse().ok()?, b.parse().ok()?);
    (a <= b).then(|| (a..=b).collect())
}

/// Reference-style member strings: "pCube1", "pCube1.vtx[3]", "pCube1.e[0:7]", "pCube1.f[2]".
pub fn parse(g: &Graph, items: &[String]) -> Result<Vec<Member>, String> {
    let mut out = Vec::new();
    for item in items {
        if let Ok(Some(n)) = g.find(item) {
            // A set may hold sets, as the reference's do; selecting it selects what they hold.
            out.push(Member::object(n));
            continue;
        }
        // A parameter has a point of its own: curve1.u[0.5].
        let dot = item[..item.find('[').unwrap_or(item.len())].rfind('.');
        let open = item[dot.unwrap_or(0)..].find('[').map(|i| i + dot.unwrap_or(0));
        let (Some(dot), Some(open)) = (dot, open) else { return Err(format!("no object named {item}")) };
        if !item.ends_with(']') {
            return Err(format!("no object named {item}"));
        }
        let node_name = &item[..dot];
        let suffix = &item[dot + 1..open];
        let n = match g.find(node_name) {
            Ok(Some(n)) => n,
            _ => return Err(format!("no object named {node_name}")),
        };
        // A surface's CVs too, by cv[u][v].
        if let Some(sh) = crate::surfaces::as_surface(g, n) {
            if suffix != "cv" {
                let inner = &item[open + 1..item.len() - 1];
                if let Some(ids) = crate::nurbs_components::parse_surface(g, sh, suffix, inner) {
                    out = merged(out, &[Member { node: sh, kind: ComponentMode::Vertex, ids }]);
                    continue;
                }
            }
            if suffix != "cv" {
                return Err(format!("unknown component '{suffix}' in {item} (cv)"));
            }
            let Some(ids) = crate::surfaces::parse_cv_ranges(g, sh, &item[open + 1..item.len() - 1]) else {
                return Err(format!("bad component range in {item}"));
            };
            out = merged(out, &[Member { node: sh, kind: ComponentMode::Vertex, ids }]);
            continue;
        }
        // A curve's CVs are its vertex components.
        if let Some(c) = crate::curves::as_curve(g, n) {
            if suffix != "cv" {
                let inner = &item[open + 1..item.len() - 1];
                if let Some(ids) = crate::nurbs_components::parse_curve(g, c, suffix, inner) {
                    out = merged(out, &[Member { node: c, kind: ComponentMode::Vertex, ids }]);
                    continue;
                }
            }
            if suffix != "cv" {
                return Err(format!("unknown component '{suffix}' in {item} (cv)"));
            }
            let range = &item[open + 1..item.len() - 1];
            let count = g.node(c).and_then(|x| crate::curves::drawn_curve(x).map(|k| k.distinct_cvs().len() as u32)).unwrap_or(0);
            let (first, last) = match range {
                "*" => (0, count.saturating_sub(1)),
                r => match r.split_once(':') {
                    Some((a, b)) => (a.parse().unwrap_or(0), b.parse().unwrap_or(0)),
                    None => {
                        let i = r.parse().map_err(|_| format!("bad component range in {item}"))?;
                        (i, i)
                    }
                },
            };
            out = merged(out, &[Member { node: c, kind: ComponentMode::Vertex, ids: (first..=last).collect() }]);
            continue;
        }
        if let Some(pn) = crate::components::point_node(g, n) {
            let range = &item[open + 1..item.len() - 1];
            let ids = match crate::components::point_grid(g, pn) {
                Some(dims) if suffix == "pt" => crate::components::grid_ids(dims, range),
                _ if matches!(suffix, "pt" | "cv" | "vtx") => plain_ids(range),
                _ => None,
            };
            let Some(ids) = ids else { return Err(format!("bad component range in {item}")) };
            out = merged(out, &[Member { node: pn, kind: ComponentMode::Vertex, ids }]);
            continue;
        }
        let Some(n) = crate::components::as_mesh(g, n) else {
            return Err(format!("{node_name} is not a mesh, so it has no components"));
        };
        let kind = match suffix {
            "vtx" | "pt" => ComponentMode::Vertex,
            "e" => ComponentMode::Edge,
            "f" => ComponentMode::Face,
            _ => return Err(format!("unknown component '{suffix}' in {item} (vtx, e or f)")),
        };
        let range = &item[open + 1..item.len() - 1];
        if range == "*" {
            let ids: Vec<u32> = match g.node(n).and_then(|x| x.data().mesh(crate::mesh_nodes::MeshNode::OUT_MESH).cloned()) {
                Some(m) => match kind {
                    ComponentMode::Vertex => (0..m.point_capacity()).filter(|&i| m.point_valid(i)).collect(),
                    ComponentMode::Edge => (0..m.edge_capacity()).filter(|&i| m.edge_valid(i)).collect(),
                    _ => (0..m.face_capacity()).filter(|&i| m.face_valid(i)).collect(),
                },
                None => Vec::new(),
            };
            out = merged(out, &[Member { node: n, kind, ids }]);
            continue;
        }
        let parse_index = |s: &str| if s.bytes().all(|b| b.is_ascii_digit()) && !s.is_empty() { s.parse::<u32>().ok() } else { None };
        let (first, last) = match range.find(':') {
            Some(c) => (parse_index(&range[..c]), parse_index(&range[c + 1..])),
            None => (parse_index(range), parse_index(range)),
        };
        let (Some(first), Some(last)) = (first, last) else { return Err(format!("bad component range in {item}")) };
        if last < first {
            return Err(format!("bad component range in {item}"));
        }
        out = merged(out, &[Member { node: n, kind, ids: (first..=last).collect() }]);
    }
    Ok(out)
}

/// The members as parse() reads them, consecutive ids folded into ranges.
pub fn format(g: &Graph, list: &[Member]) -> Vec<String> {
    let mut out = Vec::new();
    for m in list {
        if g.node(m.node).is_none() {
            continue;
        }
        let path = if m.kind == ComponentMode::Object { g.path(m.node) } else { crate::components::display_path(g, m.node) };
        if m.kind == ComponentMode::Object {
            out.push(path);
            continue;
        }
        if let Some(dims) = crate::components::point_grid(g, m.node) {
            out.extend(crate::components::grid_strings(&path, dims, &m.ids, false));
            continue;
        }
        if let Some(stride) = crate::surfaces::id_stride(g, m.node) {
            out.extend(crate::surfaces::format_cvs(&m.ids, stride).into_iter().map(|c| format!("{path}.{c}")));
            continue;
        }
        let prefix = format!("{path}.{}[", kind_suffix(m.kind));
        let mut i = 0;
        while i < m.ids.len() {
            let mut j = i;
            while j + 1 < m.ids.len() && m.ids[j + 1] == m.ids[j] + 1 {
                j += 1;
            }
            out.push(if j > i { format!("{prefix}{}:{}]", m.ids[i], m.ids[j]) } else { format!("{prefix}{}]", m.ids[i]) });
            i = j + 1;
        }
    }
    out
}

pub fn create(g: &mut Graph, name: &str, list: &[Member], quick_select: bool) -> CmdResult<NodeId> {
    let t = types::find(TYPE_NAME).unwrap();
    let set = g.create_node(&t, if name.is_empty() { "set1" } else { name }, NO_NODE, true)?;
    if quick_select {
        g.set_value(PlugRef::new(set, QUICK_SELECT as i32), PlugValue::Bool(true))?;
    }
    if !list.is_empty() {
        store(g, set, &merged(Vec::new(), list))?;
    }
    Ok(set)
}

pub fn add(g: &mut Graph, set: NodeId, list: &[Member]) -> CmdResult<()> {
    if crate::shading::is_shading_group(g, set) {
        return crate::shading::assign(g, set, list);
    }
    for m in list {
        if m.kind == ComponentMode::Object && g.node(m.node).is_some_and(is_set) && (m.node == set || holds_set(g, m.node, set)) {
            fail!("sets: {} would hold itself", g.path(set));
        }
    }
    let deformer = crate::deform_sets::deformer_of_set(g, set);
    let converted = if deformer.is_some() { crate::deform_sets::as_points(g, list) } else { Vec::new() };
    let list = if deformer.is_some() { &converted[..] } else { list };
    let before = resolve(g, set);
    let after = merged(before.clone(), list);
    if after != before {
        store(g, set, &after)?;
    }
    if let Some(d) = deformer {
        crate::deform_sets::members_changed(g, d, list, true)?;
    }
    Ok(())
}

/// A set's members replaced as they are, with no change to weights: for a deformer's set, kept in step by its callers.
/// Which of these nodes a set holds, whole or by components, told from the ids its entries are bound to. None when an
/// entry is not bound to a live node (read from a file and not yet looked up, or of an unloaded reference): `members`
/// answers then. For asking about a few nodes without naming every member of a large set.
pub(crate) fn holds_any(g: &Graph, set: NodeId, nodes: &[NodeId]) -> Option<Vec<NodeId>> {
    with_stored(g, set, |s| {
        let mut out = Vec::new();
        for e in &s.entries {
            let b = e.bound.lock().unwrap();
            if b.id == NO_NODE || b.referenced {
                return None;
            }
            if nodes.contains(&b.id) && g.node(b.id).is_some() && !out.contains(&b.id) {
                out.push(b.id);
            }
        }
        Some(out)
    })
    .unwrap_or(Some(Vec::new()))
}

/// Adds whole-object members a set does not hold yet, as one undoable set, leaving its other entries as they are:
/// what `set_members_raw` stores for the old members and these, without naming every old member again.
pub(crate) fn append_objects_raw(g: &mut Graph, set: NodeId, extra: &[NodeId]) -> CmdResult<()> {
    let ns = g.node(set).map(|n| n.namespace().to_string()).unwrap_or_default();
    let mut d = SetMembers::default();
    if let Some(kept) = with_stored(g, set, |old| {
        old.entries
            .iter()
            .filter(|e| {
                let b = e.bound.lock().unwrap().clone();
                // A member whose node was deleted is dropped, as a full store drops it.
                if b.id != NO_NODE && !b.referenced {
                    return g.node(b.id).is_some();
                }
                // Read from a file, or of a reference: bound now if its node is there, kept by name if not.
                bind(g, &ns, e);
                true
            })
            .cloned()
            .collect::<Vec<_>>()
    }) {
        d.entries = kept;
    }
    for &id in extra {
        let Some(n) = g.node(id) else { continue };
        d.entries.push(Entry { bound: Mutex::new(Binding { id, name: g.path(id), referenced: n.reference_id() != 0 }), kind: ComponentMode::Object, ids: Vec::new() });
    }
    g.set_value(PlugRef::new(set, MEMBERS as i32), PlugValue::Data(Some(Arc::new(d))))?;
    Ok(())
}

pub(crate) fn set_members_raw(g: &mut Graph, set: NodeId, list: &[Member]) -> CmdResult<()> {
    store(g, set, &merged(Vec::new(), list))
}

/// An object member removes the node with all its components; a component member removes just those ids.
pub fn remove(g: &mut Graph, set: NodeId, list: &[Member]) -> CmdResult<()> {
    if crate::shading::is_shading_group(g, set) {
        return crate::shading::unassign(g, set, list);
    }
    let deformer = crate::deform_sets::deformer_of_set(g, set);
    let converted = if deformer.is_some() { crate::deform_sets::as_points(g, list) } else { Vec::new() };
    let list = if deformer.is_some() { &converted[..] } else { list };
    let before = resolve(g, set);
    let mut after = before.clone();
    for r in list {
        if r.kind == ComponentMode::Object {
            after.retain(|m| m.node != r.node);
            continue;
        }
        for m in &mut after {
            if m.node == r.node && m.kind == r.kind {
                m.ids.retain(|id| r.ids.binary_search(id).is_err());
            }
        }
    }
    after.retain(|m| m.kind == ComponentMode::Object || !m.ids.is_empty());
    if after != before {
        store(g, set, &after)?;
    }
    if let Some(d) = deformer {
        crate::deform_sets::members_changed(g, d, list, false)?;
    }
    Ok(())
}

/// Selects the members of the sets: their nodes as objects, their components in component mode.
/// Whether `outer` holds `inner`, directly or through sets it holds.
fn holds_set(g: &Graph, outer: NodeId, inner: NodeId) -> bool {
    let mut todo = vec![outer];
    let mut seen = Vec::new();
    while let Some(s) = todo.pop() {
        if seen.contains(&s) {
            continue;
        }
        seen.push(s);
        for m in resolve(g, s) {
            if m.kind == ComponentMode::Object && g.node(m.node).is_some_and(is_set) {
                if m.node == inner {
                    return true;
                }
                todo.push(m.node);
            }
        }
    }
    false
}

/// A set's members with the sets it holds opened up, as selecting it takes them.
pub fn flattened(g: &Graph, set: NodeId) -> Vec<Member> {
    let mut out = Vec::new();
    let mut todo = vec![set];
    let mut seen = Vec::new();
    while let Some(s) = todo.pop() {
        if seen.contains(&s) {
            continue;
        }
        seen.push(s);
        for m in resolve(g, s) {
            if m.kind == ComponentMode::Object && g.node(m.node).is_some_and(is_set) {
                todo.push(m.node);
            } else {
                out.push(m);
            }
        }
    }
    out
}

pub fn select(g: &mut Graph, sets: &[NodeId], add: bool) {
    let mut objects = if add { g.selection().to_vec() } else { Vec::new() };
    let mut picked: [BTreeMap<NodeId, Ids>; 3] = Default::default();
    if add {
        picked = [
            components::selected_all(g, ComponentMode::Vertex).clone(),
            components::selected_all(g, ComponentMode::Edge).clone(),
            components::selected_all(g, ComponentMode::Face).clone(),
        ];
    }
    let slot = |k: ComponentMode| match k {
        ComponentMode::Vertex => 0,
        ComponentMode::Edge => 1,
        _ => 2,
    };
    let mut first_kind = ComponentMode::Object;
    for &set in sets {
        for m in flattened(g, set) {
            // A component member selects the mesh's transform, as selecting components does.
            let obj = if m.kind == ComponentMode::Object { m.node } else { crate::dag::transform_of(g, m.node) };
            if !objects.contains(&obj) {
                objects.push(obj);
            }
            if m.kind == ComponentMode::Object {
                continue;
            }
            if first_kind == ComponentMode::Object {
                first_kind = m.kind;
            }
            let ids = picked[slot(m.kind)].entry(m.node).or_default();
            ids.extend(m.ids);
            normalize(ids);
        }
    }
    g.select(&objects, SelectMode::Replace);
    let [p, e, f] = picked;
    components::set_all(g, p, e, f);
    if first_kind != ComponentMode::Object {
        // Stay in the current component mode when the set has components of that kind.
        let now = components::mode(g);
        let has_now = sets.iter().any(|&s| resolve(g, s).iter().any(|m| m.kind == now));
        components::set_mode(g, if has_now { now } else { first_kind });
    } else if !add {
        components::set_mode(g, ComponentMode::Object);
    }
}

/// A direct edit compacted a mesh: component members follow their elements to the new ids. Records undo.
pub fn mesh_remapped(g: &mut Graph, mesh: NodeId, remap: &MeshRemap) {
    for s in all(g, None) {
        let mut list = resolve(g, s);
        let mut changed = false;
        for m in &mut list {
            if m.node != mesh || m.kind == ComponentMode::Object {
                continue;
            }
            let map = match m.kind {
                ComponentMode::Vertex => &remap.points,
                ComponentMode::Edge => &remap.edges,
                _ => &remap.faces,
            };
            let mut ids: Ids = m.ids.iter().filter_map(|&i| map.get(i as usize).copied().filter(|&v| v != INVALID)).collect();
            normalize(&mut ids);
            if ids != m.ids {
                m.ids = ids;
                changed = true;
            }
        }
        if changed {
            list.retain(|m| m.kind == ComponentMode::Object || !m.ids.is_empty());
            let _ = store(g, s, &list);
        }
    }
}

/// { name, quickSelect, members (as format()) }.
pub fn info(g: &Graph, set: NodeId) -> Value {
    json!({
        "name": g.path(set),
        "quickSelect": g.node(set).unwrap().bool(QUICK_SELECT),
        "members": format(g, &resolve(g, set)),
    })
}

#[derive(Clone, Default)]
struct ObjectSetNode;

impl NodeBehavior for ObjectSetNode {
    fn compute(&mut self, _ctx: &EvalContext, _d: &mut NodeData) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    if types::find(TYPE_NAME).is_some() {
        return;
    }
    data::register_kind(MEMBERS_KIND, Arc::new(|j| Ok(SetMembers::from_json(j))));
    let t = NodeType {
        name: TYPE_NAME.into(),
        icon: "list-details".into(),
        doc: "An object set: a named list of objects and mesh components. Quick select sets select theirs from Edit > \
              Quick Select Sets; the game exporter exports a set as one unit."
            .into(),
        own_attrs: vec![
            AttrDef {
                name: "quickSelect".into(),
                short_name: "qs".into(),
                ty: PlugType::Bool,
                default_value: PlugValue::Bool(false),
                flags: flags::INPUT | flags::STORABLE,
                doc: "List it under Edit > Quick Select Sets.".into(),
                ..Default::default()
            },
            AttrDef {
                name: "members".into(),
                short_name: "mem".into(),
                ty: PlugType::Data,
                default_value: PlugValue::Data(None),
                flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                doc: "The objects and components in the set.".into(),
                ..Default::default()
            },
            AttrDef {
                name: "usedBy".into(),
                short_name: "ub".into(),
                ty: PlugType::Float,
                default_value: PlugValue::Float(0.0),
                flags: flags::INPUT | flags::HIDDEN,
                doc: "Tells the deformer this is the set of: connected into its deformerSet.".into(),
                ..Default::default()
            },
        ],
        create: Some(Arc::new(|| Box::new(ObjectSetNode))),
        ..Default::default()
    };
    types::register_type(t).expect("objectSet");
    // Component members follow direct edits of their mesh.
    crate::modifiers::add_remap_hook(Arc::new(|g, mesh, remap, _base| mesh_remapped(g, mesh, remap)));
}

fn set_arg(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name) {
        Ok(Some(n)) if is_set(g.node(n).unwrap()) => Ok(n),
        _ => fail!("{cmd}: {name} is not a set"),
    }
}

fn members_arg(g: &Graph, items: &[String], cmd: &str) -> CmdResult<Vec<Member>> {
    parse(g, items).map_err(|e| CommandError::new(format!("{cmd}: {e}")))
}

fn strings(values: &[Value]) -> Vec<String> {
    let mut out = Vec::new();
    for v in values {
        if let Some(s) = v.as_str() {
            out.push(s.to_string());
        } else if let Some(list) = v.as_array() {
            out.extend(list.iter().filter_map(Value::as_str).map(str::to_string));
        }
    }
    out
}

/// The first positional names the set; the rest are members, the selection when there are none.
fn set_and_members(g: &Graph, a: &Args, cmd: &str) -> CmdResult<(NodeId, Vec<Member>)> {
    let Some(first) = a.pos_str(0) else { fail!("{cmd}: name the set first") };
    let set = set_arg(g, first, cmd)?;
    let items = strings(&a.positional[1..]);
    Ok((set, if items.is_empty() { from_selection(g) } else { members_arg(g, &items, cmd)? }))
}

fn sets_arg(g: &Graph, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut out = Vec::new();
    for name in strings(&a.positional) {
        out.push(set_arg(g, &name, cmd)?);
    }
    if out.is_empty() {
        out = g.selection().iter().copied().filter(|&id| is_set(g.node(id).unwrap())).collect();
    }
    if out.is_empty() {
        fail!("{cmd}: name a set");
    }
    Ok(out)
}

pub fn register(c: &mut Commands<Session>) {
    type T = ArgType;
    type Cmd = CommandDef<Session>;
    if c.find("createSet").is_some() {
        return;
    }
    c.register(
        Cmd::new(
            "createSet",
            "Create an object set holding the members (the selected components in component mode, else the selected \
             objects). Members are object names or components like pCube1.vtx[0:3], pCube1.e[5], pCube1.f[2]. Returns its \
             name.",
            |s, a| {
                let g = &mut s.graph;
                let list = if a.bool("empty") {
                    Vec::new()
                } else {
                    let items = strings(&a.positional);
                    if items.is_empty() {
                        from_selection(g)
                    } else {
                        members_arg(g, &items, "createSet")?
                    }
                };
                let set = create(g, &a.string("name", ""), &list, a.bool("quickSelect"))?;
                Ok(json!(g.path(set)))
            },
        )
        .positional("members")
        .flag(FlagDef::new("name", "n", T::String, json!(""), "Name; set1 when empty, made unique."))
        .flag(FlagDef::new("quickSelect", "qs", T::Bool, json!(false), "Make it a quick select set (Edit > Quick Select Sets)."))
        .flag(FlagDef::new("empty", "e", T::Bool, json!(false), "Make it with no members, whatever is selected.")),
    );

    c.register(
        Cmd::new("addToSet", "Add members (the selection by default) to a set. Returns the set's members.", |s, a| {
            let g = &mut s.graph;
            let (set, list) = set_and_members(g, a, "addToSet")?;
            add(g, set, &list)?;
            Ok(json!(format(g, &members(g, set))))
        })
        .positional("set, members"),
    );

    c.register(
        Cmd::new(
            "removeFromSet",
            "Take members (the selection by default) out of a set. An object takes its components with it. Returns the \
             set's members.",
            |s, a| {
                let g = &mut s.graph;
                let (set, list) = set_and_members(g, a, "removeFromSet")?;
                remove(g, set, &list)?;
                Ok(json!(format(g, &members(g, set))))
            },
        )
        .positional("set, members"),
    );

    c.register(
        Cmd::new(
            "selectSetMembers",
            "Select what sets hold: their objects, and their components in component mode. Returns the members.",
            |s, a| {
                let g = &mut s.graph;
                let sets = sets_arg(g, a, "selectSetMembers")?;
                select(g, &sets, a.bool("add"));
                let out: Vec<String> = sets.iter().flat_map(|&set| format(g, &members(g, set))).collect();
                Ok(json!(out))
            },
        )
        .positional("sets")
        .flag(FlagDef::new("add", "", T::Bool, json!(false), "Add them to the selection instead of replacing it.")),
    );

    c.register(
        Cmd::new("deleteSet", "Delete sets; their members stay.", |s, a| {
            let g = &mut s.graph;
            for id in sets_arg(g, a, "deleteSet")? {
                g.delete_node(id)?;
            }
            Ok(Value::Null)
        })
        .positional("sets"),
    );

    c.register(
        Cmd::new(
            "listSets",
            "Every object set's name in creation order. quickSelect=True lists only quick select sets, False only plain \
             ones; object= lists the sets that hold that object. info=True returns {name, quickSelect, members} each.",
            |s, a| {
                let g = &s.graph;
                let quick = a.has("quickSelect").then(|| a.bool("quickSelect"));
                let mut holding = Vec::new();
                if a.has("object") {
                    let name = a.string("object", "");
                    match g.find(&name)? {
                        Some(n) => holding = sets_of(g, n),
                        None => fail!("listSets: no object named {name}"),
                    }
                }
                let mut out = Vec::new();
                for set in all(g, quick) {
                    if a.has("object") && !holding.contains(&set) {
                        continue;
                    }
                    out.push(if a.bool("info") { info(g, set) } else { json!(g.path(set)) });
                }
                Ok(Value::Array(out))
            },
        )
        .flag(FlagDef::new("quickSelect", "qs", T::Bool, Value::Null, "Only quick select sets (True) or only plain sets (False)."))
        .flag(FlagDef::new("object", "o", T::String, Value::Null, "Only sets holding this object, whole or by its components."))
        .flag(FlagDef::new("info", "i", T::Bool, json!(false), "Return each set as a dict instead of its name."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "setMembers",
            "A set's members as names and components (pCube1, pCube1.vtx[0:3]). nodes=True gives the distinct objects \
             instead, component owners included.",
            |s, a| {
                let g = &s.graph;
                let Some(name) = a.pos_str(0) else { fail!("setMembers: name the set") };
                let set = set_arg(g, name, "setMembers")?;
                if a.bool("nodes") {
                    let names: Vec<String> = nodes_of(g, set).into_iter().map(|id| crate::components::display_path(g, id)).collect();
                    return Ok(json!(names));
                }
                Ok(json!(format(g, &members(g, set))))
            },
        )
        .positional("set")
        .flag(FlagDef::new("nodes", "", T::Bool, json!(false), "Return the objects the set touches instead of its members."))
        .undoable(false),
    );
}
