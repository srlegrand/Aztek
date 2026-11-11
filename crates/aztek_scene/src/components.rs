//! The component selection state, its commands and component paths.
//!
//! Selected mesh components per node, the reference style: the object selection decides which meshes are live (hilited) and
//! the mode decides which component kind clicks pick. Ids are those of the mesh's cage. Changes record undo. The
//! state lives in the graph (`Graph::components`) instead of a global.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, RwLock};

use aztek_geo::component_paths::{contained_faces, edge_path_between, face_loop, face_path_between, point_path_between};
use aztek_geo::topology::{
    boundary_loops, edge_loop, edge_loop_next, edge_ring, face_components, make_mask, quad_opposite_edge,
    region_boundary_edges,
};
use aztek_geo::{Index, Mesh, INVALID};
use aztek_ops::{fail, objects, ArgType, Args, CallSource, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use aztek_undo::FnAction;
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::isolation;
use crate::modifiers;
use crate::node::Node;
use crate::plug::{MeshPtr, NodeId, PlugRef};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum ComponentMode {
    #[default]
    Object,
    Vertex,
    Edge,
    Face,
}

pub type Ids = Vec<u32>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Replace,
    Add,
    Remove,
    Toggle,
}

pub type Listener = Arc<dyn Fn() + Send + Sync>;

/// The component selection, mode and their bookkeeping.
#[derive(Default)]
pub struct ComponentState {
    mode: ComponentMode,
    /// Vertex, edge, face.
    sets: [BTreeMap<NodeId, Ids>; 3],
    /// Picks per kind, newest last; entries may since be deselected or repeated, `ordered` sorts that out.
    picks: [Vec<(NodeId, u32)>; 3],
    listeners: BTreeMap<i32, Listener>,
    next_listener: i32,
    command_running: bool,
    /// Created by the top-level command now running.
    created: BTreeSet<NodeId>,
    /// The running command's new nodes stay in component mode.
    keep_mode: bool,
    /// Objects selected when the running top-level command began.
    selected_before: Vec<NodeId>,
    serial: u64,
    picked_wedge: BTreeMap<NodeId, i32>,
}

impl ComponentState {
    /// A node was created while a top-level command runs.
    pub(crate) fn node_created(&mut self, id: NodeId) {
        if self.command_running {
            self.created.insert(id);
        }
    }

    /// Deleted nodes drop their components.
    pub(crate) fn node_deleted(&mut self, id: NodeId) {
        let mut any = false;
        for s in &mut self.sets {
            any |= s.remove(&id).is_some();
        }
        if any {
            self.notify();
        }
    }

    /// A new scene drops every component.
    pub(crate) fn cleared(&mut self) {
        for s in &mut self.sets {
            s.clear();
        }
        for p in &mut self.picks {
            p.clear();
        }
        self.notify();
    }

    fn notify(&mut self) {
        self.serial += 1;
        let copy: Vec<Listener> = self.listeners.values().cloned().collect();
        for f in copy {
            f();
        }
    }
}

fn slot(kind: ComponentMode) -> usize {
    match kind {
        ComponentMode::Vertex => 0,
        ComponentMode::Edge => 1,
        ComponentMode::Face => 2,
        ComponentMode::Object => panic!("object mode has no components"),
    }
}

type Snapshot = [BTreeMap<NodeId, Ids>; 3];

fn snapshot(g: &Graph) -> Snapshot {
    g.components.sets.clone()
}

fn snapshot_bytes(s: &Snapshot) -> usize {
    s.iter().flat_map(|set| set.values()).map(|ids| 64 + ids.capacity() * 4).sum()
}

fn restore(g: &mut Graph, s: &Snapshot) {
    g.components.sets = s.clone();
    g.components.notify();
}

fn record(g: &mut Graph, before: Snapshot) {
    let after = snapshot(g);
    let bytes = snapshot_bytes(&before) + snapshot_bytes(&after);
    let action = FnAction::new(move |g: &mut Graph| restore(g, &before), move |g: &mut Graph| restore(g, &after))
        .selection(true)
        .sized(bytes, Default::default());
    g.undo_stack_mut().record(action);
}

pub fn mode(g: &Graph) -> ComponentMode {
    g.components.mode
}

/// Not undoable, like the reference's selectMode.
pub fn set_mode(g: &mut Graph, m: ComponentMode) {
    if g.components.mode == m {
        return;
    }
    g.components.mode = m;
    g.components.notify();
}

/// set_mode recorded on the undo stack, for commands that switch mode as part of an edit.
pub fn set_mode_undoable(g: &mut Graph, m: ComponentMode) {
    let before = mode(g);
    if before != m {
        record_mode(g, before, m);
    }
    set_mode(g, m);
}

fn record_mode(g: &mut Graph, before: ComponentMode, after: ComponentMode) {
    let action = FnAction::new(move |g: &mut Graph| set_mode(g, before), move |g: &mut Graph| set_mode(g, after)).selection(true);
    g.undo_stack_mut().record(action);
}

/// The running command's new nodes keep the component mode instead of landing in object mode.
pub fn keep_mode_after_create(g: &mut Graph) {
    g.components.keep_mode = true;
}

pub fn mode_name(m: ComponentMode) -> &'static str {
    match m {
        ComponentMode::Object => "object",
        ComponentMode::Vertex => "vertex",
        ComponentMode::Edge => "edge",
        ComponentMode::Face => "face",
    }
}

pub fn mode_from_name(name: &str) -> Result<ComponentMode, String> {
    match name {
        "object" => Ok(ComponentMode::Object),
        "vertex" | "point" | "points" | "vertices" => Ok(ComponentMode::Vertex),
        "edge" | "edges" => Ok(ComponentMode::Edge),
        "face" | "faces" => Ok(ComponentMode::Face),
        _ => Err("mode must be object, vertex, edge or face".into()),
    }
}

pub fn selected_all(g: &Graph, kind: ComponentMode) -> &BTreeMap<NodeId, Ids> {
    static NONE: BTreeMap<NodeId, Ids> = BTreeMap::new();
    if kind == ComponentMode::Object {
        return &NONE;
    }
    &g.components.sets[slot(kind)]
}

/// A mesh's selected components of kind; a transform stands for its mesh shape.
pub fn selected(g: &Graph, node: NodeId, kind: ComponentMode) -> &[u32] {
    // Object mode has no components, and a command asking in it just finds none.
    if kind == ComponentMode::Object {
        return &[];
    }
    let set = &g.components.sets[slot(kind)];
    if let Some(v) = set.get(&node) {
        return v.as_slice();
    }
    match as_mesh(g, node) {
        Some(m) if m != node => set.get(&m).map_or(&[], |v| v.as_slice()),
        _ => &[],
    }
}

/// Changes whenever a component selection or the mode does, so drawings of it can be cached.
pub fn serial(g: &Graph) -> u64 {
    g.components.serial
}

fn sorted_unique(mut v: Ids) -> Ids {
    v.sort_unstable();
    v.dedup();
    v
}

/// The selection of a kind in pick order, each component where it was last added; ones with no known pick follow in
/// id order.
pub fn ordered(g: &Graph, kind: ComponentMode) -> Vec<(NodeId, u32)> {
    let sets = &g.components.sets[slot(kind)];
    let picks = &g.components.picks[slot(kind)];
    let is_selected = |node: NodeId, id: u32| sets.get(&node).is_some_and(|ids| ids.binary_search(&id).is_ok());
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &(node, id) in picks.iter().rev() {
        if is_selected(node, id) && seen.insert((node, id)) {
            out.push((node, id));
        }
    }
    out.reverse();
    for (&node, ids) in sets {
        for &id in ids {
            if !seen.contains(&(node, id)) {
                out.push((node, id));
            }
        }
    }
    out
}

fn set_union(a: &[u32], b: &[u32]) -> Ids {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() || j < b.len() {
        if j == b.len() || (i < a.len() && a[i] < b[j]) {
            out.push(a[i]);
            i += 1;
        } else if i == a.len() || b[j] < a[i] {
            out.push(b[j]);
            j += 1;
        } else {
            out.push(a[i]);
            i += 1;
            j += 1;
        }
    }
    out
}

fn set_difference(a: &[u32], b: &[u32]) -> Ids {
    a.iter().copied().filter(|x| b.binary_search(x).is_err()).collect()
}

fn set_symmetric_difference(a: &[u32], b: &[u32]) -> Ids {
    let mut out: Ids = set_difference(a, b);
    out.extend(set_difference(b, a));
    out.sort_unstable();
    out
}

/// Changes one node's selection of one kind. Replace clears that kind on every other node first.
pub fn select(g: &mut Graph, node: NodeId, kind: ComponentMode, ids: &[u32], op: Op) {
    let before = snapshot(g);
    let k = slot(kind);
    let input = sorted_unique(ids.to_vec());
    let st = &mut g.components;
    let sets = &mut st.sets[k];
    match op {
        Op::Replace => {
            sets.clear();
            sets.insert(node, input);
        }
        Op::Add => {
            let cur = sets.entry(node).or_default();
            *cur = set_union(cur, &input);
        }
        Op::Remove => {
            let cur = sets.entry(node).or_default();
            *cur = set_difference(cur, &input);
        }
        Op::Toggle => {
            let cur = sets.entry(node).or_default();
            *cur = set_symmetric_difference(cur, &input);
        }
    }
    if sets.get(&node).is_some_and(|v| v.is_empty()) {
        sets.remove(&node);
    }
    if op == Op::Replace {
        st.picks[k].clear();
    }
    if op != Op::Remove {
        for &id in ids {
            st.picks[k].push((node, id));
        }
    }
    if st.picks[k].len() > 4096 {
        let o = ordered(g, kind);
        if g.components.picks[k].len() > 4 * o.len() {
            g.components.picks[k] = o;
        }
    }
    record(g, before);
    g.components.notify();
}

pub fn clear(g: &mut Graph) {
    let before = snapshot(g);
    let st = &mut g.components;
    for s in &mut st.sets {
        s.clear();
    }
    for p in &mut st.picks {
        p.clear();
    }
    record(g, before);
    g.components.notify();
}

/// Replaces everything at once, as one undo step.
pub fn set_all(g: &mut Graph, points: BTreeMap<NodeId, Ids>, edges: BTreeMap<NodeId, Ids>, faces: BTreeMap<NodeId, Ids>) {
    let before = snapshot(g);
    let st = &mut g.components;
    st.sets = [points, edges, faces];
    for p in &mut st.picks {
        p.clear();
    }
    record(g, before);
    g.components.notify();
}

/// The cage of a mesh node, which component ids index; None for anything else.
pub fn mesh_of(g: &mut Graph, node: NodeId) -> Option<MeshPtr> {
    if !is_mesh(g, node) {
        return None;
    }
    modifiers::cage(g, node)
}

pub(crate) fn is_mesh(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("mesh"))
}

/// The mesh shape a name or a selected node stands for: the node itself when it is a mesh shape, else the first
/// visible mesh shape directly under it when it is a transform.
pub fn as_mesh(g: &Graph, id: NodeId) -> Option<NodeId> {
    crate::dag::shape_of_type(g, id, "mesh")
}

/// How a mesh shape is named to the user in component lists: through its transform, as `pCube1.f[0]` is.
pub fn display_path(g: &Graph, node: NodeId) -> String {
    g.path(crate::dag::transform_of(g, node))
}

/// Points touched by the node's selection of the current mode (verts as is, edge ends, face corners).
pub fn points_of(g: &mut Graph, node: NodeId) -> Ids {
    let Some(m) = mesh_of(g, node) else { return Vec::new() };
    let mut out = Vec::new();
    match mode(g) {
        ComponentMode::Vertex => {
            out.extend(selected(g, node, ComponentMode::Vertex).iter().copied().filter(|&p| m.point_valid(p)))
        }
        ComponentMode::Edge => {
            for &e in selected(g, node, ComponentMode::Edge) {
                if m.edge_valid(e) {
                    out.push(m.edge_point(e, 0));
                    out.push(m.edge_point(e, 1));
                }
            }
        }
        ComponentMode::Face => {
            for &f in selected(g, node, ComponentMode::Face) {
                if m.face_valid(f) {
                    m.for_each_face_corner(f, |c| out.push(m.corner_point(c)));
                }
            }
        }
        ComponentMode::Object => {}
    }
    sorted_unique(out)
}

pub fn add_listener(g: &mut Graph, f: Listener) -> i32 {
    let st = &mut g.components;
    st.next_listener += 1;
    st.listeners.insert(st.next_listener, f);
    st.next_listener
}

pub fn remove_listener(g: &mut Graph, id: i32) {
    g.components.listeners.remove(&id);
}

/// The reference's polyListComponentConversion: edges/faces to their points, points to edges/faces they fully cover, or with
/// touching to every edge/face that shares a point with them (Edge to Face: every face bordering an edge).
pub fn convert(m: &Mesh, from: ComponentMode, ids: &[u32], to: ComponentMode, touching: bool) -> Ids {
    if from == to {
        return ids.to_vec();
    }
    let points_of_ids = || {
        let mut pts = Vec::new();
        match from {
            ComponentMode::Edge => {
                for &e in ids {
                    if m.edge_valid(e) {
                        pts.push(m.edge_point(e, 0));
                        pts.push(m.edge_point(e, 1));
                    }
                }
            }
            ComponentMode::Face => {
                for &f in ids {
                    if m.face_valid(f) {
                        m.for_each_face_corner(f, |c| pts.push(m.corner_point(c)));
                    }
                }
            }
            _ => pts = ids.to_vec(),
        }
        sorted_unique(pts)
    };
    let mut out = Vec::new();
    if to == ComponentMode::Vertex {
        return points_of_ids();
    }
    if to == ComponentMode::Edge {
        if from == ComponentMode::Face {
            for &f in ids {
                if m.face_valid(f) {
                    m.for_each_face_corner(f, |c| out.push(m.corner_edge(c)));
                }
            }
            return sorted_unique(out);
        }
        // Edges with both ends selected, or either end when touching.
        let pts = points_of_ids();
        let inside: BTreeSet<u32> = pts.iter().copied().collect();
        for &p in &pts {
            m.for_each_point_edge(p, |e| {
                if touching || inside.contains(&m.edge_other_point(e, p)) {
                    out.push(e);
                }
            });
        }
        return sorted_unique(out);
    }
    if touching && from == ComponentMode::Edge {
        for &e in ids {
            if m.edge_valid(e) {
                m.for_each_edge_corner(e, |c| out.push(m.corner_face(c)));
            }
        }
        return sorted_unique(out);
    }
    // Faces fully covered by the selected points.
    let pts = points_of_ids();
    let inside: BTreeSet<u32> = pts.iter().copied().collect();
    for &p in &pts {
        for f in m.point_faces(p) {
            let mut all = true;
            if !touching {
                m.for_each_face_corner(f, |c| all = all && inside.contains(&m.corner_point(c)));
            }
            if all {
                out.push(f);
            }
        }
    }
    sorted_unique(out)
}

/// Every mesh in the hilite set: the selected meshes and the meshes under selected groups.
pub fn hilited_meshes(g: &Graph) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    let mut add = |id: NodeId| {
        if let Some(id) = as_mesh(g, id) {
            if seen.insert(id) {
                out.push(id);
            }
        }
    };
    for &id in g.selection() {
        add(id);
        for d in g.descendants(id) {
            add(d);
        }
    }
    out
}

pub fn all_ids(m: &Mesh, k: ComponentMode) -> Ids {
    let cap = match k {
        ComponentMode::Vertex => m.point_capacity(),
        ComponentMode::Edge => m.edge_capacity(),
        _ => m.face_capacity(),
    };
    (0..cap)
        .filter(|&i| match k {
            ComponentMode::Vertex => m.point_valid(i),
            ComponentMode::Edge => m.edge_valid(i),
            _ => m.face_valid(i),
        })
        .collect()
}

/// ids with their mirror images added when the mesh node's symmetry is on; ids unchanged otherwise.
pub fn with_symmetry(g: &mut Graph, node: NodeId, kind: ComponentMode, ids: &[u32]) -> Ids {
    if ids.is_empty() || kind == ComponentMode::Object || !is_mesh(g, node) {
        return ids.to_vec();
    }
    let Some(cage) = modifiers::cage(g, node) else { return ids.to_vec() };
    let mut m = (*cage).clone();
    let Some(map) = crate::mesh_nodes::symmetry_map_for(g, node, &mut m, None) else {
        return ids.to_vec();
    };
    if map.is_empty() {
        return ids.to_vec();
    }
    let wedge = map.majority_wedge(&convert(&m, kind, ids, ComponentMode::Vertex, false));
    g.components.picked_wedge.insert(node, wedge);
    let out = match kind {
        ComponentMode::Vertex => map.expand_points(ids),
        ComponentMode::Edge => map.expand_edges(&m, ids),
        _ => map.expand_faces(&m, ids),
    };
    sorted_unique(out)
}

/// Of selected components, those on the side away from the one picked: the mirror images, drawn apart from it.
pub fn mirrored_side(g: &Graph, node: NodeId, kind: ComponentMode, ids: &[u32]) -> Ids {
    let wedge = picked_wedge(g, node);
    if wedge < 0 || ids.is_empty() {
        return Vec::new();
    }
    let Some(spec) = crate::mesh_nodes::symmetry_spec(g, node) else { return Vec::new() };
    let Some(cage) = modifiers::cage_cached(g, node) else { return Vec::new() };
    let map = aztek_geo::symmetry::SymmetryMap::build_for(&cage, &spec, crate::mesh_edit::symmetry_tolerance(&cage));
    ids.iter()
        .copied()
        .filter(|&id| {
            let pts = convert(&cage, kind, &[id], ComponentMode::Vertex, false);
            !pts.is_empty() && pts.iter().all(|&p| !map.fixed(p) && map.wedge(p) != wedge)
        })
        .collect()
}

/// The symmetry wedge (side) the ids last given to with_symmetry on this node mostly sat in, -1 when none.
pub fn picked_wedge(g: &Graph, node: NodeId) -> i32 {
    g.components.picked_wedge.get(&node).copied().unwrap_or(-1)
}

// ---------------------------------------------------------------------------------------------------------------------
// Point providers

pub type ProviderPoints = Arc<dyn Fn(&mut Graph, NodeId) -> Vec<(u32, DVec3)> + Send + Sync>;
pub type ProviderSetPoints = Arc<dyn Fn(&mut Graph, NodeId, &[(u32, DVec3)]) + Send + Sync>;
pub type ProviderEditPlug = Arc<dyn Fn(&mut Graph, NodeId) -> Result<PlugRef, CommandError> + Send + Sync>;
pub type ProviderFrames = Arc<dyn Fn(&mut Graph, NodeId, &[u32]) -> Vec<DMat3> + Send + Sync>;
pub type ProviderBake = Arc<dyn Fn(&mut Graph, NodeId, &DMat4) + Send + Sync>;
pub type ProviderSoft = Arc<dyn Fn(u32) -> bool + Send + Sync>;
pub type ProviderGrid = Arc<dyn Fn(&Graph, NodeId) -> [u32; 3] + Send + Sync>;

/// Point-like components of nodes that aren't meshes (curve points and handles): vertex mode picks, centres and moves
/// them through these, by node type. Ids are the provider's own.
#[derive(Clone, Default)]
pub struct PointProvider {
    pub node_type: String,
    /// Object-space position of every component, by id.
    pub points: Option<ProviderPoints>,
    /// Moves components to object-space positions, as an undoable edit of the node.
    pub set_points: Option<ProviderSetPoints>,
    /// The plug a live drag writes and restores, any history feeding it frozen first.
    pub edit_plug: Option<ProviderEditPlug>,
    /// Optional object-space frame of each given component, columns U, V and N.
    pub frames: Option<ProviderFrames>,
    /// Applies an object-space matrix to all of the node's geometry, as an undoable edit (Freeze Transformations).
    pub bake: Option<ProviderBake>,
    /// Optional: whether soft selection may pull this component along.
    pub soft: Option<ProviderSoft>,
    /// Points on a grid (a lattice), named pt[s][t][u]: its size. An id is s + t * S + u * S * T.
    pub grid: Option<ProviderGrid>,
}

fn providers() -> &'static RwLock<Vec<PointProvider>> {
    static P: RwLock<Vec<PointProvider>> = RwLock::new(Vec::new());
    &P
}

pub fn register_point_provider(p: PointProvider) {
    let mut all = providers().write().unwrap();
    if let Some(q) = all.iter_mut().find(|q| q.node_type == p.node_type) {
        *q = p;
    } else {
        all.push(p);
    }
}

/// The provider for the node's type or a base type; None for meshes and anything else.
pub fn point_provider(n: &Node) -> Option<PointProvider> {
    let all = providers().read().unwrap();
    let mut t = Some(n.ty());
    while let Some(ty) = t {
        if let Some(p) = all.iter().find(|p| p.node_type == ty.name) {
            return Some(p.clone());
        }
        t = ty.base.as_deref();
    }
    None
}

/// The node with points of its own a name stands for: the node, or the first such shape under a transform.
pub fn point_node(g: &Graph, id: NodeId) -> Option<NodeId> {
    let has = |x: NodeId| g.node(x).is_some_and(|n| point_provider(n).is_some());
    if has(id) {
        return Some(id);
    }
    if crate::dag::is_transform(g, id) {
        return crate::dag::shapes(g, id, false).into_iter().find(|&s| has(s));
    }
    None
}

/// The size of a node's grid of points, if they lie on one.
pub fn point_grid(g: &Graph, node: NodeId) -> Option<[u32; 3]> {
    let grid = point_provider(g.node(node)?)?.grid?;
    Some(grid(g, node))
}

/// Grid ids from the inside of pt[0:1][*][2], written "0:1][*][2". Sorted by s, then t, then u, as the reference lists them.
pub fn grid_ids(dims: [u32; 3], brackets: &str) -> Option<Ids> {
    let parts: Vec<&str> = brackets.split("][").collect();
    if parts.len() != 3 {
        return None;
    }
    let mut ranges = [(0, 0); 3];
    for i in 0..3 {
        let (a, b) = match parts[i].split_once(':') {
            _ if parts[i] == "*" => (0, dims[i].checked_sub(1)?),
            Some((a, b)) => (a.parse().ok()?, b.parse().ok()?),
            None => {
                let v = parts[i].parse().ok()?;
                (v, v)
            }
        };
        if a > b || b >= dims[i] {
            return None;
        }
        ranges[i] = (a, b);
    }
    let mut out = Vec::new();
    for s in ranges[0].0..=ranges[0].1 {
        for t in ranges[1].0..=ranges[1].1 {
            for u in ranges[2].0..=ranges[2].1 {
                out.push(s + t * dims[0] + u * dims[0] * dims[1]);
            }
        }
    }
    Some(out)
}

/// Component strings for grid points: path.pt[1][2][0] each when flat, else as few boxes as a greedy pass finds.
pub fn grid_strings(path: &str, dims: [u32; 3], ids: &[u32], flat: bool) -> Vec<String> {
    let coords = |id: u32| (id % dims[0], (id / dims[0]) % dims[1], id / (dims[0] * dims[1]));
    let id_of = |s: u32, t: u32, u: u32| s + t * dims[0] + u * dims[0] * dims[1];
    let mut sorted: Vec<(u32, u32, u32)> = ids.iter().map(|&i| coords(i)).collect();
    sorted.sort_unstable();
    sorted.dedup();
    if flat {
        return sorted.into_iter().map(|(s, t, u)| format!("{path}.pt[{s}][{t}][{u}]")).collect();
    }
    let range = |a: u32, b: u32| if a == b { a.to_string() } else { format!("{a}:{b}") };
    let mut left: BTreeSet<u32> = ids.iter().copied().collect();
    let mut out = Vec::new();
    for (s, t, u) in sorted {
        if !left.contains(&id_of(s, t, u)) {
            continue;
        }
        let (mut s1, mut t1, mut u1) = (s, t, u);
        while s1 + 1 < dims[0] && left.contains(&id_of(s1 + 1, t, u)) {
            s1 += 1;
        }
        while t1 + 1 < dims[1] && (s..=s1).all(|x| left.contains(&id_of(x, t1 + 1, u))) {
            t1 += 1;
        }
        while u1 + 1 < dims[2] && (s..=s1).all(|x| (t..=t1).all(|y| left.contains(&id_of(x, y, u1 + 1)))) {
            u1 += 1;
        }
        for x in s..=s1 {
            for y in t..=t1 {
                for z in u..=u1 {
                    left.remove(&id_of(x, y, z));
                }
            }
        }
        out.push(format!("{path}.pt[{}][{}][{}]", range(s, s1), range(t, t1), range(u, u1)));
    }
    out
}

/// True when a selected node has points of its own (a curve), so component modes edit it rather than every mesh.
pub fn selection_has_point_nodes(g: &Graph) -> bool {
    g.selection().iter().any(|&id| g.node(id).is_some_and(|n| point_provider(n).is_some()))
}

// ---------------------------------------------------------------------------------------------------------------------
// Leaving component mode after creating something

/// Called by the session as a top-level command starts.
pub(crate) fn command_started(g: &mut Graph) {
    let selection = g.selection().to_vec();
    let st = &mut g.components;
    st.created.clear();
    st.keep_mode = false;
    st.command_running = true;
    st.selected_before = selection;
}

/// Whatever command creates something and selects it, the result lands in object mode so the new node shows as
/// selected. Picking another object from the UI leaves component mode too, the reference style; scripts keep it.
pub(crate) fn command_finished(g: &mut Graph, name: &str, source: CallSource, ok: bool) {
    g.components.command_running = false;
    let keep = std::mem::take(&mut g.components.keep_mode);
    if ok && g.components.mode != ComponentMode::Object && !keep {
        let created = g.selection().iter().any(|id| g.components.created.contains(id));
        let picked_another = name == "select"
            && source == CallSource::Ui
            && g.selection().iter().any(|id| !g.components.selected_before.contains(id));
        if created || picked_another {
            set_mode(g, ComponentMode::Object);
        }
    }
    g.components.created.clear();
    g.components.selected_before.clear();
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

fn mesh_node(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let Some(id) = g.find(name)? else { fail!("{cmd}: no node named '{name}'") };
    match as_mesh(g, id) {
        Some(m) => Ok(m),
        None => fail!("{cmd}: {name} is not a mesh"),
    }
}

fn json_u32(v: &Value) -> CmdResult<u32> {
    v.as_u64().map(|x| x as u32).ok_or_else(|| CommandError::new(format!("expected an id, got {v}")))
}

/// Ids, where an element may be a [first, last] range.
pub fn ids_arg(j: &Value) -> CmdResult<Ids> {
    let mut out = Vec::new();
    for v in j.as_array().map(Vec::as_slice).unwrap_or(&[]) {
        if let Some(r) = v.as_array().filter(|r| r.len() == 2) {
            let (a, b) = (json_u32(&r[0])?, json_u32(&r[1])?);
            out.extend(a..=b);
        } else {
            out.push(json_u32(v)?);
        }
    }
    Ok(out)
}

fn op_arg(a: &Args, fallback: Op) -> Op {
    if a.bool("add") {
        Op::Add
    } else if a.bool("deselect") {
        Op::Remove
    } else if a.bool("toggle") {
        Op::Toggle
    } else {
        fallback
    }
}

/// The edge continuing an edge's loop through one of its points: at a point of four edges, the one sharing no face with it.
fn loop_next(m: &Mesh, e: Index, p: Index) -> Option<Index> {
    let mut around = Vec::new();
    m.for_each_point_edge(p, |x| around.push(x));
    if around.len() != 4 {
        return None;
    }
    let mut faces = Vec::new();
    m.for_each_edge_corner(e, |c| faces.push(m.corner_face(c)));
    around.into_iter().find(|&x| {
        if x == e {
            return false;
        }
        let mut shared = false;
        m.for_each_edge_corner(x, |c| shared |= faces.contains(&m.corner_face(c)));
        !shared
    })
}

/// Grow or Shrink Along Loop: selected edges reach one edge further along their loops, or their open ends draw in by one.
fn along_loop(g: &mut Graph, grow: bool) {
    g.open_chunk(if grow { "Grow Selection" } else { "Shrink Selection" });
    for (node, ids) in selected_all(g, ComponentMode::Edge).clone() {
        let Some(m) = mesh_of(g, node) else { continue };
        let set: std::collections::BTreeSet<Index> = ids.iter().copied().collect();
        let mut out = Vec::new();
        for &e in &ids {
            let mut open_end = false;
            for side in 0..2 {
                let next = loop_next(&m, e, m.edge_point(e, side));
                match next {
                    Some(n) if set.contains(&n) => {}
                    Some(n) if grow => out.push(n),
                    _ => open_end = true,
                }
            }
            if !grow && open_end {
                out.push(e);
            }
        }
        select(g, node, ComponentMode::Edge, &out, if grow { Op::Add } else { Op::Remove });
    }
    g.close_chunk();
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new("selectMode", "Switch between object, vertex, edge and face selection. With no argument, returns the current mode.", |s, a| {
            if let Some(name) = a.pos_str(0) {
                // Each kind keeps its own selection, so coming back to a mode brings back what was selected in it.
                let next = mode_from_name(name).map_err(|e| CommandError::new(format!("selectMode: {e}")))?;
                set_mode(&mut s.graph, next);
            }
            Ok(json!(mode_name(mode(&s.graph))))
        })
        .positional("[mode]")
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "selectComponents",
            "Select vertices, edges or faces of a mesh by id. Ids may be [first, last] ranges. Without add/deselect/toggle it \
             replaces the selection of that kind.",
            |s, a| {
                let objs = objects(s, a);
                let Some(last) = objs.last() else { fail!("selectComponents: no mesh") };
                let g = &mut s.graph;
                let found = g.find(last)?;
                let provided = found.and_then(|id| g.node(id)).is_some_and(|n| point_provider(n).is_some());
                if provided && (a.has("edges") || a.has("faces")) {
                    fail!("selectComponents: {last} has only points");
                }
                let node = if provided { found.unwrap() } else { mesh_node(g, last, "selectComponents")? };
                let op = op_arg(a, Op::Replace);
                g.open_chunk("Select Components");
                let mut any = false;
                let mut result = Ok(Value::Null);
                for (flag, kind) in [("vertices", ComponentMode::Vertex), ("edges", ComponentMode::Edge), ("faces", ComponentMode::Face)] {
                    if !a.has(flag) {
                        continue;
                    }
                    let ids = match ids_arg(a.get(flag).unwrap()) {
                        Ok(ids) => ids,
                        Err(e) => {
                            result = Err(e);
                            break;
                        }
                    };
                    let ids = if a.bool("symmetric") { with_symmetry(g, node, kind, &ids) } else { ids };
                    select(g, node, kind, &ids, op);
                    if mode(g) == ComponentMode::Object || !any {
                        set_mode(g, kind);
                    }
                    any = true;
                }
                g.close_chunk();
                result?;
                if !any {
                    fail!("selectComponents: give vertices=, edges= or faces=");
                }
                Ok(Value::Null)
            },
        )
        .positional("mesh")
        .flag(flag("vertices", T::List, Value::Null, "Point ids."))
        .flag(flag("edges", T::List, Value::Null, "Edge ids."))
        .flag(flag("faces", T::List, Value::Null, "Face ids."))
        .flag(flag("add", T::Bool, json!(false), "Add to the selection."))
        .flag(flag("deselect", T::Bool, json!(false), "Remove from the selection."))
        .flag(flag("toggle", T::Bool, json!(false), "Toggle."))
        .flag(flag("symmetric", T::Bool, json!(false), "Also the mirror images, when the mesh's symmetry is on.")),
    );

    c.register(Cmd::new(
        "symmetrizeSelection",
        "Add the mirror images of the component selection, on each mesh whose symmetry is on.",
        |s, _| {
            let g = &mut s.graph;
            let kind = mode(g);
            if kind == ComponentMode::Object {
                fail!("symmetrizeSelection: select components");
            }
            let current = selected_all(g, kind).clone();
            g.chunk("Symmetrize Selection", |g| {
                for (node, ids) in current {
                    let ids = with_symmetry(g, node, kind, &ids);
                    select(g, node, kind, &ids, Op::Add);
                }
            });
            Ok(Value::Null)
        },
    ));

    c.register(
        Cmd::new(
            "componentSelection",
            "Selected components: {mesh: {\"vertices\": [...], \"edges\": [...], \"faces\": [...]}}. ordered=True gives a flat \
             list [[mesh, kind, id], ...] in pick order instead (kind is vertex, edge or face).",
            |s, a| {
                let g = &s.graph;
                if a.bool("ordered") {
                    let mut out = Vec::new();
                    for (name, kind) in [("vertex", ComponentMode::Vertex), ("edge", ComponentMode::Edge), ("face", ComponentMode::Face)] {
                        for (node, id) in ordered(g, kind) {
                            if g.node(node).is_some() {
                                out.push(json!([display_path(g, node), name, id]));
                            }
                        }
                    }
                    return Ok(Value::Array(out));
                }
                let mut out = serde_json::Map::new();
                for (key, kind) in [("vertices", ComponentMode::Vertex), ("edges", ComponentMode::Edge), ("faces", ComponentMode::Face)] {
                    for (&node, ids) in selected_all(g, kind) {
                        if g.node(node).is_some() {
                            let entry = out.entry(display_path(g, node)).or_insert_with(|| json!({}));
                            entry[key] = json!(ids);
                        }
                    }
                }
                Ok(Value::Object(out))
            },
        )
        .flag(flag(
            "ordered",
            T::Bool,
            json!(false),
            "True: a flat list [[mesh, kind, id], ...] in the order the components were picked. False: grouped per mesh \
             and kind, ids sorted ascending, pick order lost.",
        ))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "convertSelection",
            "Convert the component selection to another kind and switch to that mode. In object mode the selected meshes \
             convert as a whole: every component of that kind.",
            |s, a| {
                let Some(name) = a.pos_str(0) else { fail!("convertSelection: give vertex, edge or face") };
                let to = mode_from_name(name).map_err(CommandError::new)?;
                let g = &mut s.graph;
                let from = mode(g);
                if to == ComponentMode::Object {
                    fail!("convertSelection: give vertex, edge or face");
                }
                let mut sets: [BTreeMap<NodeId, Ids>; 3] = Default::default();
                if from == ComponentMode::Object {
                    // A whole object stands for all of its components.
                    for node in hilited_meshes(g) {
                        if let Some(m) = mesh_of(g, node) {
                            let vis = isolation::active_visibility(g, node, &m);
                            sets[slot(to)].insert(node, isolation::visible_only(&vis, to, all_ids(&m, to)));
                        }
                    }
                    if sets[slot(to)].is_empty() {
                        fail!("convertSelection: select a mesh or some components");
                    }
                } else {
                    for (node, ids) in selected_all(g, from).clone() {
                        if let Some(m) = mesh_of(g, node) {
                            let vis = isolation::active_visibility(g, node, &m);
                            sets[slot(to)].insert(node, isolation::visible_only(&vis, to, convert(&m, from, &ids, to, true)));
                        }
                    }
                }
                let [p, e, f] = sets;
                set_all(g, p, e, f);
                if from != to {
                    record_mode(g, from, to);
                }
                set_mode(g, to);
                Ok(Value::Null)
            },
        )
        .positional("to"),
    );

    c.register(Cmd::new(
        "growSelection",
        "Grow the component selection by one ring of neighbours: points to the points of their faces, edges and faces to \
         those sharing a point with the selection.",
        |s, a| {
            let g = &mut s.graph;
            let k = mode(g);
            if k == ComponentMode::Object {
                return Ok(Value::Null);
            }
            if a.bool("alongLoop") && k == ComponentMode::Edge {
                along_loop(g, true);
                return Ok(Value::Null);
            }
            g.open_chunk("Grow Selection");
            for (node, ids) in selected_all(g, k).clone() {
                let Some(m) = mesh_of(g, node) else { continue };
                let pts = convert(&m, k, &ids, ComponentMode::Vertex, false);
                let mut grown = Vec::new();
                for p in pts {
                    if !m.point_valid(p) {
                        continue;
                    }
                    if k == ComponentMode::Edge {
                        m.for_each_point_edge(p, |e| grown.push(e));
                    } else {
                        for f in m.point_faces(p) {
                            if k == ComponentMode::Face {
                                grown.push(f);
                            } else {
                                m.for_each_face_corner(f, |c| grown.push(m.corner_point(c)));
                            }
                        }
                    }
                }
                let vis = isolation::active_visibility(g, node, &m);
                let ids = isolation::visible_only(&vis, k, grown);
                select(g, node, k, &ids, Op::Add);
            }
            g.close_chunk();
            Ok(Value::Null)
        },
    ).flag(flag("alongLoop", T::Bool, json!(false), "Selected edges reach one further along their loops."))
    );

    c.register(
        Cmd::new(
            "selectLoop",
            "Extend the selection to full loops (or rings with ring=True): edges along their own loops, vertices along the \
             loops of the edges joining them, faces along the face loops crossing the edges they share.",
            |s, a| {
                let g = &mut s.graph;
                let k = mode(g);
                if k == ComponentMode::Object {
                    return Ok(Value::Null);
                }
                let ring = a.bool("ring") && k != ComponentMode::Face;
                g.open_chunk("Select Loop");
                for (node, ids) in selected_all(g, k).clone() {
                    let Some(m) = mesh_of(g, node) else { continue };
                    let vis = isolation::active_visibility(g, node, &m);
                    let mut out = Vec::new();
                    if k == ComponentMode::Face {
                        let inside = make_mask(m.face_capacity() as usize, &ids);
                        for e in convert(&m, ComponentMode::Face, &ids, ComponentMode::Edge, false) {
                            let mut faces = 0;
                            m.for_each_edge_corner(e, |c| faces += inside[m.corner_face(c) as usize] as i32);
                            if faces >= 2 {
                                out.extend(isolation::clip_loop(&m, &vis, &face_loop(&m, e), e, ComponentMode::Face, false));
                            }
                        }
                    } else {
                        let edges = if k == ComponentMode::Vertex {
                            convert(&m, ComponentMode::Vertex, &ids, ComponentMode::Edge, false)
                        } else {
                            ids.clone()
                        };
                        for e in edges {
                            let chain = if ring { edge_ring(&m, e) } else { edge_loop(&m, e) };
                            out.extend(isolation::clip_loop(&m, &vis, &chain, e, ComponentMode::Edge, ring));
                        }
                        if k == ComponentMode::Vertex {
                            out = convert(&m, ComponentMode::Edge, &sorted_unique(out), ComponentMode::Vertex, false);
                        }
                    }
                    select(g, node, k, &out, Op::Add);
                }
                g.close_chunk();
                Ok(Value::Null)
            },
        )
        .flag(flag("ring", T::Bool, json!(false), "Select rings instead of loops. Faces have no ring and take their loops.")),
    );

    c.register(Cmd::new("shrinkSelection", "Shrink the component selection by dropping what lies on its border.", |s, a| {
        let g = &mut s.graph;
        let k = mode(g);
        if k == ComponentMode::Object {
            return Ok(Value::Null);
        }
        if a.bool("alongLoop") && k == ComponentMode::Edge {
            along_loop(g, false);
            return Ok(Value::Null);
        }
        g.open_chunk("Shrink Selection");
        // A copy: removing a mesh's last component erases its entry from the live selection.
        for (node, ids) in selected_all(g, k).clone() {
            let Some(m) = mesh_of(g, node) else { continue };
            let mut drop = Vec::new();
            match k {
                ComponentMode::Face => {
                    let inside = make_mask(m.face_capacity() as usize, &ids);
                    // The mesh's own boundary counts as unselected surroundings too.
                    for &f in &ids {
                        for p in m.face_points(f) {
                            if m.is_boundary_point(p) || m.point_faces(p).iter().any(|&h| inside[h as usize] == 0) {
                                drop.push(f);
                                break;
                            }
                        }
                    }
                }
                ComponentMode::Edge => {
                    // The inverse of grow: an edge stays only if every edge at both its ends is selected.
                    let inside = make_mask(m.edge_capacity() as usize, &ids);
                    let border = |p: Index| {
                        let mut out = m.is_boundary_point(p);
                        m.for_each_point_edge(p, |e| out = out || inside[e as usize] == 0);
                        out
                    };
                    for &e in &ids {
                        if m.edge_valid(e) && (border(m.edge_point(e, 0)) || border(m.edge_point(e, 1))) {
                            drop.push(e);
                        }
                    }
                }
                _ => {
                    let pts = convert(&m, k, &ids, ComponentMode::Vertex, false);
                    let inside = make_mask(m.point_capacity() as usize, &pts);
                    let mut border = vec![false; m.point_capacity() as usize];
                    for &p in &pts {
                        if m.is_boundary_point(p) {
                            border[p as usize] = true;
                        }
                        for f in m.point_faces(p) {
                            for q in m.face_points(f) {
                                if inside[q as usize] == 0 {
                                    border[p as usize] = true;
                                }
                            }
                        }
                    }
                    drop.extend(ids.iter().copied().filter(|&id| border[id as usize]));
                }
            }
            if !drop.is_empty() {
                select(g, node, k, &drop, Op::Remove);
            }
        }
        g.close_chunk();
        Ok(Value::Null)
    }).flag(flag("alongLoop", T::Bool, json!(false), "Open ends draw in by one along their loops.")));

    c.register(Cmd::new(
        "selectBorder",
        "Select border edges: the perimeter of selected faces, or the whole open boundary through selected edges or vertices.",
        |s, _| {
            let g = &mut s.graph;
            let k = mode(g);
            if k == ComponentMode::Object {
                return Ok(Value::Null);
            }
            let mut out: BTreeMap<NodeId, Ids> = BTreeMap::new();
            for (node, ids) in selected_all(g, k).clone() {
                let Some(m) = mesh_of(g, node) else { continue };
                let mut edges = Vec::new();
                if k == ComponentMode::Face {
                    edges = region_boundary_edges(&m, &ids);
                } else {
                    let mut touched = make_mask(m.point_capacity() as usize, &convert(&m, k, &ids, ComponentMode::Vertex, false));
                    if k == ComponentMode::Edge {
                        touched = vec![0; m.point_capacity() as usize];
                    }
                    let edge_in = if k == ComponentMode::Edge { make_mask(m.edge_capacity() as usize, &ids) } else { Vec::new() };
                    for lp in boundary_loops(&m) {
                        let hit = (0..lp.edges.len()).any(|i| {
                            if k == ComponentMode::Edge {
                                edge_in[lp.edges[i] as usize] != 0
                            } else {
                                touched[lp.points[i] as usize] != 0
                            }
                        });
                        if hit {
                            edges.extend_from_slice(&lp.edges);
                        }
                    }
                }
                if !edges.is_empty() {
                    out.insert(node, sorted_unique(edges));
                }
            }
            let points = selected_all(g, ComponentMode::Vertex).clone();
            let faces = selected_all(g, ComponentMode::Face).clone();
            set_all(g, points, out, faces);
            set_mode(g, ComponentMode::Edge);
            Ok(Value::Null)
        },
    ));

    c.register(
        Cmd::new("selectByAngle", "Grow the face selection across edges where neighbouring faces bend less than an angle.", |s, a| {
            let g = &mut s.graph;
            if mode(g) != ComponentMode::Face {
                fail!("selectByAngle: no faces selected");
            }
            let cos_limit = (a.float("angle", 30.0) * std::f64::consts::PI / 180.0).cos();
            g.open_chunk("Select by Angle");
            for (node, ids) in selected_all(g, ComponentMode::Face).clone() {
                let Some(m) = mesh_of(g, node) else { continue };
                let mut inside = make_mask(m.face_capacity() as usize, &ids);
                let vis = isolation::active_visibility(g, node, &m);
                let mut work: Vec<Index> = ids.clone();
                let mut grown = Vec::new();
                while let Some(f) = work.pop() {
                    if !m.face_valid(f) {
                        continue;
                    }
                    let nf = m.face_normal(f);
                    for c in m.face_corners(f) {
                        let e = m.corner_edge(c);
                        if !m.is_manifold_edge(e) {
                            continue;
                        }
                        let h = m.opposite_face(e, f);
                        if h == INVALID || inside[h as usize] != 0 || !vis.face(h) || nf.dot(m.face_normal(h)) < cos_limit {
                            continue;
                        }
                        inside[h as usize] = 1;
                        grown.push(h);
                        work.push(h);
                    }
                }
                if !grown.is_empty() {
                    select(g, node, ComponentMode::Face, &grown, Op::Add);
                }
            }
            g.close_chunk();
            Ok(Value::Null)
        })
        .flag(flag("angle", T::Float, json!(30.0), "Largest bend between neighbouring faces, degrees.").min(0.0).max(180.0))
        .adjustable(true),
    );

    c.register(Cmd::new("selectAllComponents", "Select every component of the current kind on the hilited meshes.", |s, _| {
        let g = &mut s.graph;
        let k = mode(g);
        if k == ComponentMode::Object {
            return Ok(Value::Null);
        }
        g.open_chunk("Select All Components");
        let mut first = true;
        for node in hilited_meshes(g) {
            let Some(m) = mesh_of(g, node) else { continue };
            let vis = isolation::active_visibility(g, node, &m);
            let ids = isolation::visible_only(&vis, k, all_ids(&m, k));
            select(g, node, k, &ids, if first { Op::Replace } else { Op::Add });
            first = false;
        }
        g.close_chunk();
        Ok(Value::Null)
    }));

    c.register(Cmd::new("InvertSelection", "The runtime command for invertSelection.", |s, _| s.run("invertSelection", json!({}))));
    c.register(Cmd::new(
        "invertSelection",
        "Invert the selection: the unselected components of the current kind on the hilited meshes, or in object mode every \
         unselected object. An isolated view inverts only what it shows.",
        |s, _| {
            let g = &mut s.graph;
            let k = mode(g);
            if k == ComponentMode::Object {
                let iso = isolation::active(g).cloned();
                let shown: Vec<NodeId> = g
                    .nodes()
                    .into_iter()
                    .filter(|&id| !crate::dag::is_shape(g, id) && !crate::cameras::is_default_camera(g, id))
                    .filter(|&id| g.node(id).is_some_and(|n| n.ty().dag) && iso.as_ref().map_or(true, |s| s.shows(id)))
                    .collect();
                g.select(&shown, SelectMode::Toggle);
                return Ok(Value::Null);
            }
            g.open_chunk("Invert Selection");
            for node in hilited_meshes(g) {
                if let Some(m) = mesh_of(g, node) {
                    let vis = isolation::active_visibility(g, node, &m);
                    let ids = isolation::visible_only(&vis, k, all_ids(&m, k));
                    select(g, node, k, &ids, Op::Toggle);
                }
            }
            g.close_chunk();
            Ok(Value::Null)
        },
    ));

    register_path_commands(c);
}

// ---------------------------------------------------------------------------------------------------------------------
// Component paths

struct Target {
    node: NodeId,
    /// The cage, which component ids index.
    mesh: MeshPtr,
}

fn mesh_arg(s: &mut Session, a: &Args, cmd: &str) -> CmdResult<Target> {
    let objs = objects(s, a);
    let Some(last) = objs.last() else { fail!("{cmd}: no mesh") };
    let g = &mut s.graph;
    let node = mesh_node(g, last, cmd)?;
    g.evaluate();
    let Some(mesh) = modifiers::cage(g, node) else { fail!("{cmd}: {last} has no mesh") };
    Ok(Target { node, mesh })
}

fn select_and_show(g: &mut Graph, node: NodeId, kind: ComponentMode, ids: &[u32], op: Op, a: &Args) {
    let ids = if a.bool("symmetric") { with_symmetry(g, node, kind, ids) } else { ids.to_vec() };
    select(g, node, kind, &ids, op);
    set_mode(g, kind);
}

/// The loop or ring through two neighbouring components: vertices sharing an edge, edges meeting along a loop or
/// facing each other across a quad, faces sharing an edge. Empty when they are not neighbours.
fn neighbour_loop(m: &Mesh, vis: &isolation::Visibility, kind: ComponentMode, a: Index, b: Index) -> Ids {
    match kind {
        ComponentMode::Vertex => {
            if !m.point_valid(a) || !m.point_valid(b) {
                return Vec::new();
            }
            let e = m.find_edge(a, b);
            if e == INVALID {
                return Vec::new();
            }
            let lp = isolation::clip_loop(m, vis, &edge_loop(m, e), e, ComponentMode::Edge, false);
            convert(m, ComponentMode::Edge, &lp, ComponentMode::Vertex, false)
        }
        ComponentMode::Edge => {
            if !m.edge_valid(a) || !m.edge_valid(b) {
                return Vec::new();
            }
            for i in 0..2 {
                if edge_loop_next(m, a, m.edge_point(a, i)) == b {
                    return isolation::clip_loop(m, vis, &edge_loop(m, a), a, ComponentMode::Edge, false);
                }
            }
            let mut facing = false;
            m.for_each_edge_corner(a, |c| facing = facing || quad_opposite_edge(m, a, m.corner_face(c)) == b);
            if facing {
                isolation::clip_loop(m, vis, &edge_ring(m, a), a, ComponentMode::Edge, true)
            } else {
                Vec::new()
            }
        }
        _ => {
            if !m.face_valid(a) || !m.face_valid(b) {
                return Vec::new();
            }
            let mut shared = INVALID;
            m.for_each_face_corner(a, |c| {
                let e = m.corner_edge(c);
                m.for_each_edge_corner(e, |d| {
                    if m.corner_face(d) == b {
                        shared = e;
                    }
                });
            });
            if shared == INVALID {
                Vec::new()
            } else {
                isolation::clip_loop(m, vis, &face_loop(m, shared), shared, ComponentMode::Face, false)
            }
        }
    }
}

fn op_flags() -> Vec<FlagDef> {
    vec![
        flag("add", T::Bool, json!(false), "Add to the selection."),
        flag("deselect", T::Bool, json!(false), "Remove from the selection."),
        flag("toggle", T::Bool, json!(false), "Toggle."),
        flag("symmetric", T::Bool, json!(false), "Also the mirror images, when the mesh's symmetry is on."),
    ]
}

fn kind_arg(a: &Args, cmd: &str, fallback: &str) -> CmdResult<ComponentMode> {
    match mode_from_name(&a.string("kind", fallback)) {
        Ok(ComponentMode::Object) | Err(_) => fail!("{cmd}: kind must be vertex, edge or face"),
        Ok(k) => Ok(k),
    }
}

fn register_path_commands(c: &mut Commands<Session>) {
    let mut loop_cmd = Cmd::new(
        "selectLoopThrough",
        "Select the edge loop (or ring) through one edge of a mesh, or with faces=True the face loop crossing it. Without \
         add/deselect/toggle it replaces the selection of that kind. Returns the ids.",
        |s, a| {
            let t = mesh_arg(s, a, "selectLoopThrough")?;
            if !a.has("edge") {
                fail!("selectLoopThrough: give edge=");
            }
            let e = a.int("edge", 0) as Index;
            if !t.mesh.edge_valid(e) {
                fail!("selectLoopThrough: no edge {e}");
            }
            let faces = a.bool("faces");
            let ring = !faces && a.bool("ring");
            let chain = if faces {
                face_loop(&t.mesh, e)
            } else if ring {
                edge_ring(&t.mesh, e)
            } else {
                edge_loop(&t.mesh, e)
            };
            let g = &mut s.graph;
            let vis = isolation::active_visibility(g, t.node, &t.mesh);
            let kind = if faces { ComponentMode::Face } else { ComponentMode::Edge };
            let ids = isolation::clip_loop(&t.mesh, &vis, &chain, e, kind, ring);
            g.chunk("Select Loop", |g| select_and_show(g, t.node, kind, &ids, op_arg(a, Op::Replace), a));
            Ok(json!(ids))
        },
    )
    .positional("mesh")
    .flag(flag("edge", T::Int, Value::Null, "The edge the loop runs through.").min(0.0))
    .flag(flag("ring", T::Bool, json!(false), "The edge ring instead of the loop."))
    .flag(flag("faces", T::Bool, json!(false), "The faces of the edge's ring: the face loop crossing the edge."));
    for f in op_flags() {
        loop_cmd = loop_cmd.flag(f);
    }
    c.register(loop_cmd);

    let mut connected = Cmd::new(
        "selectConnected",
        "Select every component of a kind on the connected piece (faces joined through shared edges) that one component \
         belongs to. Without add/deselect/toggle it replaces the selection of that kind. Returns the ids.",
        |s, a| {
            let t = mesh_arg(s, a, "selectConnected")?;
            if !a.has("id") {
                fail!("selectConnected: give id=");
            }
            let kind = kind_arg(a, "selectConnected", "face")?;
            let m = &*t.mesh;
            let id = a.int("id", 0) as Index;
            let valid = match kind {
                ComponentMode::Vertex => m.point_valid(id),
                ComponentMode::Edge => m.edge_valid(id),
                _ => m.face_valid(id),
            };
            if !valid {
                fail!("selectConnected: no {} {id}", mode_name(kind));
            }
            let mut shell = Vec::new();
            face_components(m, &mut shell, &[]);
            let mut want = vec![false; shell.len() + 1];
            let mut seed = |f: Index| {
                if (f as usize) < shell.len() && shell[f as usize] != INVALID {
                    want[shell[f as usize] as usize] = true;
                }
            };
            match kind {
                ComponentMode::Face => seed(id),
                ComponentMode::Edge => m.for_each_edge_corner(id, |c| seed(m.corner_face(c))),
                _ => {
                    for f in m.point_faces(id) {
                        seed(f);
                    }
                }
            }
            let faces: Ids =
                (0..shell.len() as Index).filter(|&f| shell[f as usize] != INVALID && want[shell[f as usize] as usize]).collect();
            // A loose edge or point has no faces to spread through, so it is its own piece.
            let ids = if faces.is_empty() { vec![id] } else { convert(m, ComponentMode::Face, &faces, kind, false) };
            let g = &mut s.graph;
            let vis = isolation::active_visibility(g, t.node, m);
            let ids = isolation::visible_only(&vis, kind, ids);
            g.chunk("Select Connected", |g| select_and_show(g, t.node, kind, &ids, op_arg(a, Op::Replace), a));
            Ok(json!(ids))
        },
    )
    .positional("mesh")
    .flag(flag("kind", T::String, json!("face"), "vertex | edge | face").choices(&["vertex", "edge", "face"]))
    .flag(flag("id", T::Int, Value::Null, "A component of the piece to select.").min(0.0));
    for f in op_flags() {
        connected = connected.flag(f);
    }
    c.register(connected);

    c.register(
        Cmd::new(
            "selectPathBetween",
            "Select the components between two of a mesh's vertices, edges or faces: the shorter way along a loop or ring \
             they share, else the shortest path across the surface. Neighbours (vertices on one edge, edges along a loop or \
             across a quad, faces sharing an edge) select the whole loop or ring through both instead. Adds to the selection \
             unless replace or deselect. Returns the ids in order.",
            |s, a| {
                let t = mesh_arg(s, a, "selectPathBetween")?;
                if !a.has("from") || !a.has("to") {
                    fail!("selectPathBetween: give from= and to=");
                }
                let kind = kind_arg(a, "selectPathBetween", "edge")?;
                let from = a.int("from", 0) as Index;
                let to = a.int("to", 0) as Index;
                let g = &mut s.graph;
                let vis = isolation::active_visibility(g, t.node, &t.mesh);
                let mut path = neighbour_loop(&t.mesh, &vis, kind, from, to);
                if path.is_empty() {
                    path = match kind {
                        ComponentMode::Vertex => point_path_between(&t.mesh, from, to),
                        ComponentMode::Edge => edge_path_between(&t.mesh, from, to),
                        _ => face_path_between(&t.mesh, from, to),
                    };
                }
                if path.is_empty() {
                    fail!("selectPathBetween: no path between {from} and {to}");
                }
                let op = if a.bool("deselect") {
                    Op::Remove
                } else if a.bool("replace") {
                    Op::Replace
                } else {
                    Op::Add
                };
                g.chunk("Select Path", |g| select_and_show(g, t.node, kind, &path, op, a));
                Ok(json!(path))
            },
        )
        .positional("mesh")
        .flag(flag("kind", T::String, json!("edge"), "vertex | edge | face").choices(&["vertex", "edge", "face"]))
        .flag(flag("from", T::Int, Value::Null, "Id the path starts at.").min(0.0))
        .flag(flag("to", T::Int, Value::Null, "Id the path ends at.").min(0.0))
        .flag(flag("replace", T::Bool, json!(false), "Replace the selection of that kind."))
        .flag(flag("deselect", T::Bool, json!(false), "Remove the path from the selection."))
        .flag(flag("symmetric", T::Bool, json!(false), "Also the mirror images, when the mesh's symmetry is on.")),
    );

    c.register(
        Cmd::new(
            "selectContainedFaces",
            "Select the faces enclosed by the selected edges: a flood fill that stops at selected edges and mesh borders. On \
             each piece of a mesh the smaller regions are kept (every one but the largest by area), or with larger=True only \
             the largest. Switches to face mode.",
            |s, a| {
                let g = &mut s.graph;
                g.evaluate();
                let larger = a.bool("larger");
                let mut faces: BTreeMap<NodeId, Ids> = BTreeMap::new();
                let mut out = serde_json::Map::new();
                for (node, edges) in selected_all(g, ComponentMode::Edge).clone() {
                    let Some(m) = modifiers::cage(g, node) else { continue };
                    if g.node(node).is_none() || edges.is_empty() {
                        continue;
                    }
                    let ids = contained_faces(&m, &edges, !larger);
                    if !ids.is_empty() {
                        out.insert(display_path(g, node), json!(ids));
                        faces.insert(node, ids);
                    }
                }
                if faces.is_empty() {
                    fail!("selectContainedFaces: select edges that close off a region");
                }
                let edges = selected_all(g, ComponentMode::Edge).clone();
                set_all(g, BTreeMap::new(), edges, faces);
                set_mode(g, ComponentMode::Face);
                Ok(Value::Object(out))
            },
        )
        .flag(flag("larger", T::Bool, json!(false), "Keep the largest region instead of the smaller ones.")),
    );
}

/// The current mode's selected components as the reference writes them: pCube1.f[0:3], one range per item, or one id per
/// item with flatten.
pub fn component_strings(g: &Graph, flatten: bool) -> Vec<String> {
    let kind = mode(g);
    if kind == ComponentMode::Object {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (&node, ids) in selected_all(g, kind) {
        if ids.is_empty() || g.node(node).is_none() {
            continue;
        }
        let path = display_path(g, node);
        // Edit points, curve points, isoparms and patches of NURBS shapes spell themselves.
        let (special, plain): (Vec<u32>, Vec<u32>) = ids.iter().partition(|&&i| crate::nurbs_components::is_encoded(i));
        for &i in &special {
            if let Some(t) = crate::nurbs_components::format_id(g, node, i) {
                out.push(format!("{path}.{t}"));
            }
        }
        if plain.is_empty() {
            continue;
        }
        let ids = &plain;
        if let Some(dims) = point_grid(g, node) {
            out.extend(grid_strings(&path, dims, ids, flatten));
            continue;
        }
        if let Some(stride) = crate::surfaces::id_stride(g, node) {
            let list = if flatten { crate::surfaces::format_cvs_flat(ids, stride) } else { crate::surfaces::format_cvs(ids, stride) };
            out.extend(list.into_iter().map(|c| format!("{path}.{c}")));
            continue;
        }
        if flatten {
            let p = match kind {
                ComponentMode::Vertex if !is_mesh(g, node) => "cv",
                ComponentMode::Vertex => "vtx",
                ComponentMode::Edge => "e",
                _ => "f",
            };
            let mut v = ids.clone();
            v.sort_unstable();
            v.dedup();
            out.extend(v.into_iter().map(|i| format!("{path}.{p}[{i}]")));
        } else {
            let curve = kind == ComponentMode::Vertex && !is_mesh(g, node);
            out.extend(crate::history::format_components(kind, ids).split_whitespace().map(|c| {
                if curve {
                    format!("{path}.{}", c.replacen("vtx", "cv", 1))
                } else {
                    format!("{path}.{c}")
                }
            }));
        }
    }
    out
}

/// Whether a name is a component string (pCube1.f[0:3]) rather than a node or plug.
pub fn is_component_string(name: &str) -> bool {
    // A parameter has a point of its own: curve1.u[0.5].
    name.ends_with(']') && name.find('[').is_some_and(|o| name[..o].contains('.'))
}

/// select's component half: members from component strings, combined with the current component selection by mode.
pub fn select_strings(g: &mut Graph, items: &[String], how: crate::graph::SelectMode) -> Result<Option<ComponentMode>, String> {
    use crate::graph::SelectMode as M;
    // NURBS components count against what's evaluated.
    if items.iter().any(|i| is_component_string(i)) {
        g.evaluate();
    }
    let members = crate::object_sets::parse(g, items)?;
    let mut sets = [selected_all(g, ComponentMode::Vertex).clone(), selected_all(g, ComponentMode::Edge).clone(), selected_all(g, ComponentMode::Face).clone()];
    if how == M::Replace {
        for s in &mut sets {
            s.clear();
        }
    }
    let mut first = None;
    for m in members {
        if m.kind == ComponentMode::Object {
            continue;
        }
        first.get_or_insert(m.kind);
        let set = sets[slot(m.kind)].entry(m.node).or_default();
        match how {
            M::Replace | M::Add => set.extend(m.ids),
            M::Remove => set.retain(|i| !m.ids.contains(i)),
            M::Toggle => {
                for i in m.ids {
                    if let Some(at) = set.iter().position(|&x| x == i) {
                        set.remove(at);
                    } else {
                        set.push(i);
                    }
                }
            }
        }
        set.sort_unstable();
        set.dedup();
    }
    for s in &mut sets {
        s.retain(|_, v| !v.is_empty());
    }
    let [p, e, f] = sets;
    set_all(g, p, e, f);
    Ok(first)
}
