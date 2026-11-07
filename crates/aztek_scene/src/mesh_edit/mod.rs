//! The helpers the component edit commands share, and
//! their registration.

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use aztek_geo::symmetry::{radial_layer_name, symmetry_layer_name, SymmetryMap, SymmetrySpec, SymmetrySync};
use aztek_geo::{Index, Mesh, MeshRemap, INVALID};
use aztek_ops::{fail, Args, CmdResult, CommandError, Commands};
use glam::DVec3;
use serde_json::Value;

use crate::components::{self, ComponentMode, Ids};
use crate::graph::Graph;
use crate::mesh_nodes::{symmetry_label, symmetry_spec};
use crate::modifiers;
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;

pub mod component_ops;
pub mod knife;
pub mod ref_poly;
pub mod mesh_ops;
pub mod normals;
pub mod symmetry;
pub mod tools;
pub mod transform;

/// Direct edits work on the base mesh under any modifiers; a procedural source is frozen first.
pub fn base_plug(g: &mut Graph, n: NodeId) -> CmdResult<PlugRef> {
    modifiers::edit_plug(g, n)
}

pub fn base_mesh(g: &mut Graph, p: &PlugRef) -> Mesh {
    g.evaluate();
    g.stored_value(p).as_mesh().map(|m| (**m).clone()).unwrap_or_default()
}

/// The mirror axis (0-2) of a mesh's symmetry, or -1 when off or radial.
pub fn symmetry_axis(g: &Graph, n: NodeId) -> i32 {
    let axis = g.node(n).map_or(0, |n| n.int(crate::mesh_nodes::MeshNode::SYMMETRY));
    if (1..=3).contains(&axis) {
        (axis - 1) as i32
    } else {
        -1
    }
}

/// Only for matching points that have no stored partner; stored pairs survive any drift.
pub fn symmetry_tolerance(mesh: &Mesh) -> f64 {
    (mesh.bounds().size().length() * 1e-5).max(1e-4)
}

pub fn symmetry_layer(spec: &SymmetrySpec) -> String {
    match &spec.radial {
        Some(r) => radial_layer_name(r),
        None => symmetry_layer_name(spec.axis),
    }
}

pub fn symmetry_name(spec: &SymmetrySpec) -> String {
    if spec.radial.is_some() {
        symmetry_label(spec)
    } else {
        ((b'X' + spec.axis.clamp(0, 2) as u8) as char).to_string()
    }
}

fn report_sync(path: &str, spec: &SymmetrySpec, s: &SymmetrySync, had_layer: bool) {
    let rematched = s.topological + s.by_position;
    if rematched == 0 {
        return;
    }
    if had_layer {
        log::warn!(
            "{path}: symmetry {} re-matched {rematched} points without a stored partner ({} by position), {} unmatched",
            symmetry_name(spec),
            s.by_position,
            s.unmatched
        );
    } else {
        log::info!("{path}: symmetry {} paired {rematched} points, {} unmatched", symmetry_name(spec), s.unmatched);
    }
}

/// The mesh's persistent symmetry pairs (or radial orbits), stored onto it if it has none yet.
pub fn symmetry_for(g: &Graph, n: NodeId, mesh: &mut Mesh) -> Option<SymmetryMap> {
    let spec = symmetry_spec(g, n)?;
    let had_layer = mesh.point_attrs().has(&symmetry_layer(&spec));
    let mut sync = SymmetrySync::default();
    let tol = symmetry_tolerance(mesh);
    let map = SymmetryMap::persistent(mesh, &spec, tol, Some(&mut sync));
    report_sync(&components::display_path(g, n), &spec, &sync, had_layer);
    Some(map)
}

/// After a topology op: pairs the points it created (every wedge was made together) and sets every moved or new
/// point's orbit to the exact images of its member in the driving wedge.
pub fn maintain_symmetry(g: &Graph, n: NodeId, mesh: &mut Mesh, before: &[DVec3], drive_wedge: i32) {
    let Some(spec) = symmetry_spec(g, n) else { return };
    let first_new = before.len() as Index;
    let tol = symmetry_tolerance(mesh);
    let map = SymmetryMap::persistent(mesh, &spec, tol, None);
    let touched: Ids = (0..mesh.point_capacity())
        .filter(|&p| {
            mesh.point_valid(p) && (p >= first_new || before[p as usize].to_array().map(f64::to_bits) != mesh.position(p).to_array().map(f64::to_bits))
        })
        .collect();
    map.enforce_wedge(mesh, &touched, drive_wedge);
}

pub fn remap_ids(ids: &[Index], map: &[Index]) -> Ids {
    ids.iter().filter_map(|&i| map.get(i as usize).copied().filter(|&m| m != INVALID)).collect()
}

#[derive(Debug, Clone, Default)]
pub struct EditResult {
    pub kind: Option<ComponentMode>,
    pub ids: Ids,
    /// Whatever else the op reports back to its command (what it clamped, what it skipped).
    pub info: Value,
}

pub fn faces(ids: Ids) -> EditResult {
    EditResult { kind: Some(ComponentMode::Face), ids, info: Value::Null }
}
pub fn edges(ids: Ids) -> EditResult {
    EditResult { kind: Some(ComponentMode::Edge), ids, info: Value::Null }
}
pub fn points(ids: Ids) -> EditResult {
    EditResult { kind: Some(ComponentMode::Vertex), ids, info: Value::Null }
}

pub type EditFn<'a> = dyn FnMut(&mut Mesh, &Ids, Option<&SymmetryMap>) -> CmdResult<EditResult> + 'a;
pub type HandledWhole<'a> = dyn FnMut(&mut Session, NodeId) -> CmdResult<bool> + 'a;

fn slot(k: ComponentMode) -> usize {
    match k {
        ComponentMode::Vertex => 0,
        ComponentMode::Edge => 1,
        _ => 2,
    }
}

/// Follows a compaction: every kind's selection on node moves to the new ids.
pub(crate) fn remap_selection(sel: &mut [BTreeMap<NodeId, Ids>; 3], node: NodeId, remap: &MeshRemap) {
    let maps = [&remap.points, &remap.edges, &remap.faces];
    for k in 0..3 {
        if let Some(ids) = sel[k].get(&node) {
            let kept = remap_ids(ids, maps[k]);
            if kept.is_empty() {
                sel[k].remove(&node);
            } else {
                sel[k].insert(node, kept);
            }
        }
    }
}

pub(crate) fn current_selection(g: &Graph) -> [BTreeMap<NodeId, Ids>; 3] {
    [
        components::selected_all(g, ComponentMode::Vertex).clone(),
        components::selected_all(g, ComponentMode::Edge).clone(),
        components::selected_all(g, ComponentMode::Face).clone(),
    ]
}

/// Writes an edited base mesh back and tells whatever tracks its ids.
pub(crate) fn store_edited(g: &mut Graph, node: NodeId, base: PlugRef, mesh: Mesh, remap: &MeshRemap) -> CmdResult<()> {
    let edited = Arc::new(mesh);
    g.set_value(base, PlugValue::Mesh(Some(edited.clone())))?;
    modifiers::base_remapped(g, node, remap, &edited);
    Ok(())
}

/// Runs `f` on every mesh with a component selection, converted to `kind`. Each mesh is edited on a copy, compacted,
/// written back to inMesh (undoable) and its component selection replaced by what the op returns.
pub fn edit_selected(
    s: &mut Session,
    label: &str,
    kind: ComponentMode,
    f: &mut EditFn,
    allow_whole: bool,
    handled_whole: Option<&mut HandledWhole>,
) -> CmdResult {
    edit_selected_impl(s, label, kind, f, allow_whole, handled_whole, None)
}

/// edit_selected for an op registered with history: each mesh gets a history node for it when history is on.
pub fn edit_selected_op(s: &mut Session, node_type: &str, a: &Args, allow_whole: bool, handled_whole: Option<&mut HandledWhole>) -> CmdResult {
    edit_selected_op_info(s, node_type, a, allow_whole, handled_whole).map(|(v, _)| v)
}

/// edit_selected_op that also returns each mesh's EditResult::info.
pub fn edit_selected_op_info(
    s: &mut Session,
    node_type: &str,
    a: &Args,
    allow_whole: bool,
    handled_whole: Option<&mut HandledWhole>,
) -> CmdResult<(Value, Vec<Value>)> {
    let op = crate::history::op(node_type).ok_or_else(|| CommandError::new(format!("no history op {node_type}")))?;
    let run = op.run;
    let mut infos = Vec::new();
    let v = edit_selected_impl(
        s,
        op.label,
        op.kind,
        &mut |m, sel, sym| {
            let r = run(a, m, sel, sym)?;
            infos.push(r.info.clone());
            Ok(r)
        },
        allow_whole,
        handled_whole,
        Some((&op, a)),
    )?;
    Ok((v, infos))
}

fn edit_selected_impl(
    s: &mut Session,
    label: &str,
    kind: ComponentMode,
    f: &mut EditFn,
    allow_whole: bool,
    mut handled_whole: Option<&mut HandledWhole>,
    history: Option<(&crate::history::HistoryOp, &Args)>,
) -> CmdResult {
    s.graph.evaluate();
    let mut targets: BTreeMap<NodeId, Ids> = BTreeMap::new();
    let from = components::mode(&s.graph);
    if from != ComponentMode::Object {
        let sel = components::selected_all(&s.graph, from).clone();
        for (node, ids) in sel {
            if !components::is_mesh(&s.graph, node) {
                continue;
            }
            if let Some(cage) = modifiers::cage(&mut s.graph, node) {
                targets.insert(node, components::convert(&cage, from, &ids, kind, false));
            }
        }
    }
    let mut handled = false;
    if targets.is_empty() && allow_whole {
        for sel in s.graph.selection().to_vec() {
            let Some(id) = components::as_mesh(&s.graph, sel) else { continue };
            let done = match handled_whole.as_mut() {
                Some(h) => h(s, id)?,
                None => false,
            };
            if done {
                handled = true;
            } else {
                targets.insert(id, Vec::new());
            }
        }
    }
    if targets.is_empty() && handled {
        return Ok(Value::Null);
    }
    if targets.is_empty() {
        fail!("{label}: no components selected");
    }

    let g = &mut s.graph;
    g.open_chunk(label);
    let r = apply_targets(g, kind, targets, f, history, Some(from));
    g.close_chunk();
    r.map(|made| names_of(&s.graph, &made))
}

/// The history nodes an edit made, as commands return them; Null when it recorded none.
pub(crate) fn names_of(g: &Graph, made: &[NodeId]) -> Value {
    if made.is_empty() {
        Value::Null
    } else {
        Value::Array(made.iter().map(|&n| Value::String(g.path(n))).collect())
    }
}

/// Runs an edit on the given components of each mesh, recording a history node when history is on and the call's
/// constructionHistory flag allows it. `follow` is the component mode the components were selected from: the result
/// becomes the selection. Without it they were named in the call, and the selection only follows the new ids.
pub(crate) fn apply_targets(
    g: &mut Graph,
    kind: ComponentMode,
    targets: BTreeMap<NodeId, Ids>,
    f: &mut EditFn,
    history: Option<(&crate::history::HistoryOp, &Args)>,
    follow: Option<ComponentMode>,
) -> CmdResult<Vec<NodeId>> {
    let mut made = Vec::new();
    let mut new_sel = current_selection(g);
    let mut result_kind = None;
    for (node, sel) in targets {
        let spec = symmetry_spec(g, node);
        // Radial symmetry isn't recorded in history yet: those edits stay destructive.
        let record = history.filter(|(_, a)| {
            crate::history::can_record(g, node)
                && spec.as_ref().is_none_or(|s| s.radial.is_none() && s.plane.is_none())
                && a.get("constructionHistory").and_then(Value::as_bool) != Some(false)
        });
        let (base, mut mesh) = if record.is_some() {
            g.evaluate();
            (None, modifiers::cage(g, node).map(|m| (*m).clone()).unwrap_or_default())
        } else {
            let base = base_plug(g, node)?;
            let mesh = base_mesh(g, &base);
            (Some(base), mesh)
        };
        if spec.is_some() {
            // Stores the pairs onto the mesh (and reports a re-match) before the edit runs.
            symmetry_for(g, node, &mut mesh);
        }
        let applied = crate::history::apply_edit(&mut mesh, spec.as_ref(), &sel, kind, f)?;
        let (r, remap) = (applied.result, applied.remap);
        // The other kinds' selections follow the new ids, so they come back when their mode does.
        remap_selection(&mut new_sel, node, &remap);
        if let (Some(k), true) = (r.kind, follow.is_some()) {
            result_kind = Some(k);
            let maps = [&remap.points, &remap.edges, &remap.faces];
            let mapped = remap_ids(&r.ids, maps[slot(k)]);
            if mapped.is_empty() {
                new_sel[slot(k)].remove(&node);
            } else {
                new_sel[slot(k)].insert(node, mapped);
            }
        }
        match (base, record) {
            (Some(base), _) => store_edited(g, node, base, mesh, &remap)?,
            (None, Some((op, a))) => {
                let h = crate::history::insert(g, node, op, a, &sel, spec.as_ref())?;
                let name = a.string("name", "");
                if !name.is_empty() {
                    g.rename(h, &name)?;
                }
                made.push(h);
                modifiers::base_remapped(g, node, &remap, &Arc::new(mesh));
            }
            (None, None) => unreachable!(),
        }
    }
    let [p, e, fs] = new_sel;
    components::set_all(g, p, e, fs);
    // A component mode is the user's: repeated edits (bridge after bridge) must not bounce it to the result's kind.
    if let (Some(k), Some(ComponentMode::Object)) = (result_kind, follow) {
        components::set_mode_undoable(g, k);
    }
    Ok(made)
}

pub fn with_mirror(m: &Mesh, s: Option<&SymmetryMap>, ids: &[Index], kind: ComponentMode) -> Ids {
    let Some(s) = s else { return ids.to_vec() };
    match kind {
        ComponentMode::Vertex => s.expand_points(ids),
        ComponentMode::Edge => s.expand_edges(m, ids),
        ComponentMode::Face => s.expand_faces(m, ids),
        ComponentMode::Object => ids.to_vec(),
    }
}

pub fn mesh_arg(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name)? {
        Some(id) => match components::as_mesh(g, id) {
            Some(m) => Ok(m),
            None => fail!("{cmd}: {name} is not a mesh"),
        },
        None => fail!("{cmd}: {name} is not a mesh"),
    }
}

/// The mesh= flag's mesh, else the first selected or hilited mesh.
pub fn mesh_flag(g: &Graph, a: &Args, cmd: &str) -> CmdResult<NodeId> {
    let name = a.string("mesh", "");
    if !name.is_empty() {
        return mesh_arg(g, &name, cmd);
    }
    let mut candidates = g.selection().to_vec();
    if components::mode(g) != components::ComponentMode::Object {
        candidates.extend(components::selected_all(g, components::mode(g)).keys().copied());
    }
    candidates
        .into_iter()
        .find_map(|id| components::as_mesh(g, id))
        .ok_or_else(|| CommandError::new(format!("{cmd}: give mesh= or select a mesh")))
}

pub fn vec(j: &Value) -> DVec3 {
    let c = |i: usize| j.get(i).and_then(Value::as_f64).unwrap_or(0.0);
    DVec3::new(c(0), c(1), c(2))
}

/// The first spline of a named curve as a world-space polyline, for polyBridge's Path. Registered by the curve module.
pub type CurvePathFn = Arc<dyn Fn(&mut Graph, NodeId) -> Result<Vec<DVec3>, String> + Send + Sync>;

static CURVE_PATH: RwLock<Option<CurvePathFn>> = RwLock::new(None);

pub fn set_curve_path_provider(f: CurvePathFn) {
    *CURVE_PATH.write().unwrap() = Some(f);
}

pub(crate) fn curve_path(g: &mut Graph, curve: NodeId) -> Result<Vec<DVec3>, String> {
    let f = CURVE_PATH.read().unwrap().clone();
    match f {
        Some(f) => f(g, curve),
        None => Err(format!("{} is not a curve", g.path(curve))),
    }
}

/// Commands whose last run can be adjusted.
pub const ADJUSTABLE: [&str; 15] = [
    "polyExtrude",
    "polyInset",
    "polyBevel",
    "polyInsertLoop",
    "polySplitRings",
    "polyOffsetLoop",
    "polyMerge",
    "polyBridge",
    "polyGridFill",
    "polyQuadrangulate",
    "polySubdivide",
    "polyMirror",
    "polyTriangulate",
    "polyReduce",
    "polySoftEdge",
];

pub fn register(c: &mut Commands<Session>) {
    component_ops::register(c);
    knife::register(c);
    mesh_ops::register(c);
    normals::register(c);
    symmetry::register(c);
    tools::register(c);
    transform::register(c);
    crate::soft_select::register(c);
    for name in ADJUSTABLE {
        c.set_adjustable(name, true);
    }
}
