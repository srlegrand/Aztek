//!
//! Isolate Select, one set per viewport: the objects a view shows, and for meshes isolated by their components, the
//! faces of them it shows. Display only: the scene is untouched. Face masks hold 1 per face that shows. The active
//! view's set is `Graph::active_isolation`, which the UI keeps pointed at the view component tools act in; unset in
//! batch, where nothing is hidden.

use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use aztek_geo::{Index, Mesh};
use serde_json::{json, Value};

use crate::components::{self, ComponentMode, Ids};
use crate::graph::Graph;
use crate::modifiers;
use crate::plug::{MeshPtr, NodeId, NO_NODE};

/// Faces kept on a partly shown mesh: sorted ids, and the face capacity when they were taken. Faces at or past it were
/// made since and show too.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Faces {
    pub ids: Ids,
    pub capacity: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Set {
    /// Every node the view shows, whole or in part. Empty when not isolating.
    pub nodes: HashSet<NodeId>,
    pub partial: BTreeMap<NodeId, Faces>,
    /// New and unique across the process on every change, so caches can key on it.
    pub version: u64,
}

impl Set {
    pub fn active(&self) -> bool {
        !self.nodes.is_empty()
    }
    /// Whether the node shows at all: always when not isolating.
    pub fn shows(&self, id: NodeId) -> bool {
        self.nodes.is_empty() || self.nodes.contains(&id)
    }
    pub fn is_partial(&self, id: NodeId) -> bool {
        self.partial.contains_key(&id)
    }
    /// The face mask for the node's mesh, or empty when every face shows.
    pub fn shown_faces(&self, id: NodeId, mesh: &Mesh) -> Vec<u8> {
        let Some(f) = self.partial.get(&id) else { return Vec::new() };
        let mut mask = vec![0u8; mesh.face_capacity() as usize];
        for &i in &f.ids {
            if (i as usize) < mask.len() {
                mask[i as usize] = 1;
            }
        }
        for i in f.capacity..mesh.face_capacity() {
            mask[i as usize] = 1;
        }
        mask
    }
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.partial.clear();
        self.touch();
    }
    pub fn touch(&mut self) {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        self.version = NEXT.fetch_add(1, Ordering::Relaxed);
    }
}

fn sorted_unique(mut v: Ids) -> Ids {
    v.sort_unstable();
    v.dedup();
    v
}

fn cage_of(g: &mut Graph, id: NodeId) -> Option<MeshPtr> {
    if components::is_mesh(g, id) {
        modifiers::cage(g, id)
    } else {
        None
    }
}

/// Folds faces made since the ids were taken into them, so a later capacity doesn't hide them.
fn settle(f: &mut Faces, m: &Mesh) {
    for i in f.capacity..m.face_capacity() {
        if m.face_valid(i) {
            f.ids.push(i);
        }
    }
    f.capacity = f.capacity.max(m.face_capacity());
}

/// The selection as isolation sees it: meshes with components of the current mode, and objects without.
struct Picked {
    faces: BTreeMap<NodeId, Ids>,
    objects: Vec<NodeId>,
}

fn picked(g: &mut Graph) -> Picked {
    let mut out = Picked { faces: BTreeMap::new(), objects: Vec::new() };
    let mode = components::mode(g);
    if mode != ComponentMode::Object {
        for (node, ids) in components::selected_all(g, mode).clone() {
            if ids.is_empty() {
                continue;
            }
            if let Some(m) = cage_of(g, node) {
                let f = faces_of(&m, mode, &ids);
                if !f.is_empty() {
                    out.faces.insert(node, f);
                }
            }
        }
    }
    for &id in g.selection() {
        let shape = components::as_mesh(g, id).unwrap_or(id);
        if !out.faces.contains_key(&id) && !out.faces.contains_key(&shape) {
            out.objects.push(id);
        }
    }
    out
}

/// Faces with any corner on the components: faces as they are, an edge's faces, a point's faces. Sorted.
pub fn faces_of(m: &Mesh, kind: ComponentMode, ids: &[u32]) -> Ids {
    let mut out = Vec::new();
    for &id in ids {
        match kind {
            ComponentMode::Face if id < m.face_capacity() && m.face_valid(id) => out.push(id),
            ComponentMode::Edge if id < m.edge_capacity() && m.edge_valid(id) => {
                m.for_each_edge_corner(id, |c| out.push(m.corner_face(c)))
            }
            ComponentMode::Vertex if id < m.point_capacity() && m.point_valid(id) => out.extend(m.point_faces(id)),
            _ => {}
        }
    }
    sorted_unique(out)
}

/// Isolates the selection: meshes with components selected in the current mode show only the faces those touch;
/// other selected objects show whole, with their children. Unselected parents stay hidden.
pub fn from_selection(g: &mut Graph) -> Set {
    let mut set = Set::default();
    add_selection(&mut set, g);
    set
}

/// Adds the selection the way from_selection takes it. A mesh shown whole stays whole.
pub fn add_selection(set: &mut Set, g: &mut Graph) {
    let p = picked(g);
    for (node, faces) in p.faces {
        let Some(m) = cage_of(g, node) else { continue };
        // Shown whole already: it stays whole.
        if set.nodes.contains(&node) && !set.partial.contains_key(&node) {
            continue;
        }
        let shown = set.nodes.contains(&node);
        let kept = set.partial.entry(node).or_default();
        if !shown {
            kept.capacity = m.face_capacity();
        } else {
            settle(kept, &m);
        }
        kept.ids.extend(faces);
        kept.ids = sorted_unique(std::mem::take(&mut kept.ids));
        kept.capacity = m.face_capacity();
        set.nodes.insert(node);
        // The shape's transform shows too, or nothing under it would draw.
        set.nodes.insert(crate::dag::transform_of(g, node));
    }
    for id in p.objects {
        set.nodes.insert(id);
        set.partial.remove(&id);
        for d in g.descendants(id) {
            set.nodes.insert(d);
            set.partial.remove(&d);
        }
    }
    set.touch();
}

/// Removes the selection: selected components' faces, or selected objects with their children.
pub fn remove_selection(set: &mut Set, g: &mut Graph) {
    if !set.active() {
        return;
    }
    let p = picked(g);
    for (node, faces) in p.faces {
        if !set.nodes.contains(&node) {
            continue;
        }
        let Some(m) = cage_of(g, node) else { continue };
        let whole = !set.partial.contains_key(&node);
        let kept = set.partial.entry(node).or_default();
        if whole {
            // Shown whole until now: every face but these.
            kept.capacity = m.face_capacity();
            kept.ids.extend((0..m.face_capacity()).filter(|&f| m.face_valid(f)));
        } else {
            settle(kept, &m);
        }
        kept.ids.retain(|f| faces.binary_search(f).is_err());
        if kept.ids.is_empty() {
            set.partial.remove(&node);
            set.nodes.remove(&node);
            set.nodes.remove(&crate::dag::transform_of(g, node));
        }
    }
    for id in p.objects {
        set.nodes.remove(&id);
        set.partial.remove(&id);
        for d in g.descendants(id) {
            set.nodes.remove(&d);
            set.partial.remove(&d);
        }
    }
    // Isolating nothing would show everything; keep an empty isolation showing nothing instead.
    if set.nodes.is_empty() {
        set.nodes.insert(NO_NODE);
    }
    set.touch();
}

/// {"nodes": [names], "faces": {name: {"ids": [...], "capacity": n}}}, or null when not isolating.
pub fn to_json(set: &Set, g: &Graph) -> Value {
    if !set.active() {
        return Value::Null;
    }
    let mut names: Vec<String> = set.nodes.iter().filter(|&&id| g.node(id).is_some()).map(|&id| g.path(id)).collect();
    names.sort();
    let mut faces = serde_json::Map::new();
    for (&id, f) in &set.partial {
        if g.node(id).is_some() {
            faces.insert(g.path(id), json!({"ids": f.ids, "capacity": f.capacity}));
        }
    }
    json!({"nodes": names, "faces": faces})
}

pub fn from_json(j: &Value, g: &Graph) -> Set {
    // A name that has since become ambiguous restores nothing rather than failing the load.
    let find = |path: &str| g.find(path).ok().flatten();
    let mut set = Set::default();
    if !j.is_object() {
        return set;
    }
    for name in j.get("nodes").and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[]) {
        if let Some(id) = name.as_str().and_then(find) {
            set.nodes.insert(id);
        }
    }
    if let Some(faces) = j.get("faces").and_then(Value::as_object) {
        for (name, f) in faces {
            let Some(id) = find(name) else { continue };
            if !set.nodes.contains(&id) || !f.is_object() {
                continue;
            }
            let ids: Ids = f
                .get("ids")
                .and_then(Value::as_array)
                .map(|a| a.iter().filter_map(Value::as_u64).map(|x| x as u32).collect())
                .unwrap_or_default();
            let capacity = f.get("capacity").and_then(Value::as_u64).unwrap_or(0) as u32;
            set.partial.insert(id, Faces { ids: sorted_unique(ids), capacity });
        }
    }
    // Saved isolation whose nodes are all gone still isolates (shows nothing), as it did when saved.
    if set.nodes.is_empty() && j.get("nodes").is_some() {
        set.nodes.insert(NO_NODE);
    }
    set.touch();
    set
}

/// Per-kind masks derived from a face mask: an edge or point shows when any face on it does. Empty masks mean all
/// show.
#[derive(Debug, Clone, Default)]
pub struct Visibility {
    pub faces: Vec<u8>,
    pub edges: Vec<u8>,
    pub points: Vec<u8>,
}

impl Visibility {
    pub fn all(&self) -> bool {
        self.faces.is_empty()
    }
    pub fn face(&self, f: u32) -> bool {
        self.faces.is_empty() || self.faces.get(f as usize).is_some_and(|&x| x != 0)
    }
    pub fn edge(&self, e: u32) -> bool {
        self.edges.is_empty() || self.edges.get(e as usize).is_some_and(|&x| x != 0)
    }
    pub fn point(&self, p: u32) -> bool {
        self.points.is_empty() || self.points.get(p as usize).is_some_and(|&x| x != 0)
    }
    pub fn shows(&self, kind: ComponentMode, id: u32) -> bool {
        match kind {
            ComponentMode::Vertex => self.point(id),
            ComponentMode::Edge => self.edge(id),
            ComponentMode::Face => self.face(id),
            ComponentMode::Object => true,
        }
    }
}

pub fn visibility(m: &Mesh, mut shown_faces: Vec<u8>) -> Visibility {
    let mut v = Visibility::default();
    if shown_faces.is_empty() {
        return v;
    }
    shown_faces.resize(m.face_capacity() as usize, 0);
    v.edges = vec![0; m.edge_capacity() as usize];
    v.points = vec![0; m.point_capacity() as usize];
    for f in 0..m.face_capacity() {
        if shown_faces[f as usize] == 0 || !m.face_valid(f) {
            continue;
        }
        m.for_each_face_corner(f, |c| {
            v.edges[m.corner_edge(c) as usize] = 1;
            v.points[m.corner_point(c) as usize] = 1;
        });
    }
    v.faces = shown_faces;
    v
}

/// The ids that show; all of them when the mask is empty.
pub fn visible_only(vis: &Visibility, kind: ComponentMode, mut ids: Ids) -> Ids {
    if !vis.all() {
        ids.retain(|&id| vis.shows(kind, id));
    }
    ids
}

/// The run of an ordered chain (a loop, ring or face loop) through chain[seed] that shows, stopping at the first
/// hidden element either way; wrapping around the ends when the chain is closed. Empty when the seed itself is hidden.
pub fn clip_chain(chain: &[u32], seed: usize, closed: bool, shows: impl Fn(u32) -> bool) -> Ids {
    if seed >= chain.len() || !shows(chain[seed]) {
        return Vec::new();
    }
    let n = chain.len();
    let mut hi = seed;
    let mut count = 1;
    while count < n && (closed || hi + 1 < n) && shows(chain[(hi + 1) % n]) {
        hi = (hi + 1) % n;
        count += 1;
    }
    if count == n {
        return chain.to_vec();
    }
    let mut lo = seed;
    while count < n && (closed || lo > 0) && shows(chain[(lo + n - 1) % n]) {
        lo = (lo + n - 1) % n;
        count += 1;
    }
    (0..count).map(|i| chain[(lo + i) % n]).collect()
}

/// clip_chain for what edge_loop, edge_ring (ring) or face_loop (kind Face) returned through edge e, seeded at e or at
/// a shown face on it, closed when its ends meet on the mesh. The chain as is when everything shows.
pub fn clip_loop(m: &Mesh, vis: &Visibility, chain: &[u32], e: u32, kind: ComponentMode, ring: bool) -> Ids {
    if vis.all() || chain.is_empty() {
        return chain.to_vec();
    }
    let face_has_edge = |f: Index, x: Index| {
        let mut has = false;
        if m.face_valid(f) {
            m.for_each_face_corner(f, |c| has = has || m.corner_edge(c) == x);
        }
        has
    };
    let edges_share_face = |a: Index, b: Index| {
        let mut shared = false;
        m.for_each_edge_corner(a, |c| shared = shared || face_has_edge(m.corner_face(c), b));
        shared
    };
    let first = chain[0];
    let last = *chain.last().unwrap();
    if kind == ComponentMode::Face {
        let seed = (0..chain.len()).find(|&i| vis.face(chain[i]) && face_has_edge(chain[i], e)).unwrap_or(chain.len());
        let mut adjacent = false;
        if chain.len() > 2 {
            m.for_each_face_corner(first, |c| adjacent = adjacent || face_has_edge(last, m.corner_edge(c)));
        }
        return clip_chain(chain, seed, adjacent, |f| vis.face(f));
    }
    let seed = chain.iter().position(|&x| x == e).unwrap_or(chain.len());
    let mut closed = false;
    if chain.len() > 2 {
        closed = if ring {
            edges_share_face(first, last)
        } else {
            m.edge_point(first, 0) == m.edge_point(last, 0)
                || m.edge_point(first, 0) == m.edge_point(last, 1)
                || m.edge_point(first, 1) == m.edge_point(last, 0)
                || m.edge_point(first, 1) == m.edge_point(last, 1)
        };
    }
    clip_chain(chain, seed, closed, |x| vis.edge(x))
}

/// Face mask of mesh after levels of smooth preview, from the base mesh's mask; empty if the mapping can't be made.
pub fn subdivided_faces(base: &Mesh, levels: i32, shown_faces: &[u8], subdivided_capacity: u32) -> Vec<u8> {
    if levels <= 0 {
        return shown_faces.to_vec();
    }
    // Smoothing splits each live face, in id order, into one quad per corner, then every quad into four per level
    // after.
    let mut out: Vec<u8> = Vec::with_capacity(subdivided_capacity as usize);
    for f in 0..base.face_capacity() {
        if base.face_valid(f) {
            let s = shown_faces.get(f as usize).copied().unwrap_or(1);
            out.extend(std::iter::repeat(s).take(base.face_size(f) as usize));
        }
    }
    for _ in 1..levels {
        out = out.iter().flat_map(|&s| [s; 4]).collect();
    }
    if out.len() != subdivided_capacity as usize {
        return Vec::new();
    }
    out
}

/// The active view's isolation, when it isolates anything.
pub fn active(g: &Graph) -> Option<&Set> {
    g.active_isolation.as_ref().filter(|s| s.active())
}

/// What the active view shows of the node's mesh (its cage, which component ids index).
pub fn active_visibility(g: &Graph, id: NodeId, mesh: &Mesh) -> Visibility {
    match active(g) {
        Some(s) if s.is_partial(id) => visibility(mesh, s.shown_faces(id, mesh)),
        _ => Visibility::default(),
    }
}

pub type InheritListener = Arc<dyn Fn(&[NodeId], &[NodeId]) + Send + Sync>;

/// Commands that make objects out of others (combine, extract) report it, so views isolating any source show the
/// result.
pub fn inherit(g: &mut Graph, made: &[NodeId], from: &[NodeId]) {
    let listeners: Vec<InheritListener> = g.inherit_listeners.values().cloned().collect();
    for f in listeners {
        f(made, from);
    }
}

pub fn add_inherit_listener(g: &mut Graph, f: InheritListener) -> i32 {
    let id = g.inherit_listeners.keys().next_back().copied().unwrap_or(0) + 1;
    g.inherit_listeners.insert(id, f);
    id
}

pub fn remove_inherit_listener(g: &mut Graph, id: i32) {
    g.inherit_listeners.remove(&id);
}

/// Adds made to the set when it shows any of from whole or in part. True when it changed.
pub fn inherit_into(set: &mut Set, made: &[NodeId], from: &[NodeId]) -> bool {
    if !set.active() || !from.iter().any(|id| set.nodes.contains(id)) {
        return false;
    }
    set.nodes.extend(made.iter().copied());
    set.touch();
    true
}
