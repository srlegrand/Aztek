//! One-way subdivision kept live, with Edit Selection.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::loop_tools::{edit_edge_flow, EdgeFlowOptions};
use aztek_geo::ops::{region_ring_edges, split_rings};
use aztek_geo::{AttrType, Index, Mesh, MeshRemap};
use aztek_ops::{fail, objects, Args, CmdResult, CommandDef, Commands};
use serde_json::{json, Value};

use crate::attr_defs::{mesh_attr, param};
use crate::components::{self, ComponentMode, Op};
use crate::extrude_modifier::{remap_components, resolve_components, selected_components, ExtrudeComponents};
use crate::graph::{Graph, SelectMode};
use crate::modifiers::{self, guarded, ModifierBehavior, BYPASS_LAYER};
use crate::node::{traits, types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{data, flags, AttrDef, Data, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;

pub const IN_MESH: usize = 0;
pub const ENABLED: usize = 1;
pub const EXPORT_ENABLED: usize = 2;
pub const COUNT: usize = 3;
pub const DIRECTION: usize = 4;
pub const EDGE_FLOW: usize = 5;
pub const COMPONENTS: usize = 6;
pub const EDITING: usize = 7;
pub const EDIT_BACKUP: usize = 8;
pub const OUT_MESH: usize = 9;
pub const LOST: usize = 10;

/// The selection to put back when Edit Selection ends.
#[derive(Debug)]
struct EditBackup {
    value: Value,
}

impl Data for EditBackup {
    fn kind(&self) -> &str {
        "splitRingsEditBackup"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "splitRingsEditBackup", "value": self.value})
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn components_of(d: &NodeData) -> Option<ExtrudeComponents> {
    d.value(COMPONENTS).as_data().and_then(|p| p.as_any().downcast_ref::<ExtrudeComponents>().cloned())
}

#[derive(Clone, Default)]
pub struct SplitRingsModifierNode {
    error: String,
}

impl NodeBehavior for SplitRingsModifierNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        self.error.clear();
        let input = d.mesh(IN_MESH).cloned();
        d.set_output(OUT_MESH, PlugValue::Mesh(input.clone()));
        d.set_output(LOST, PlugValue::Int(0));
        let Some(input) = input else { return };
        if modifiers::bypassed(Some(&input)) {
            return;
        }
        if d.bool(EDITING) {
            let mut shown = (*input).clone();
            shown.point_attrs_mut().add(BYPASS_LAYER, AttrType::Bool);
            d.set_output(OUT_MESH, PlugValue::Mesh(Some(Arc::new(shown))));
            return;
        }
        if !d.bool(ENABLED) {
            return;
        }
        let r = resolve_components(&input, components_of(d).as_ref());
        d.set_output(LOST, PlugValue::Int(r.lost as i64));
        if r.ids.is_empty() {
            return;
        }
        let (across, count, flow) = (d.int(DIRECTION) == 1, d.int(COUNT).clamp(1, 32) as i32, d.bool(EDGE_FLOW));
        let result = guarded(|| {
            let mut m = (*input).clone();
            let ring_edges = if r.kind == ComponentMode::Face { region_ring_edges(&m, &r.ids, across) } else { r.ids.clone() };
            let made = split_rings(&mut m, &ring_edges, count, None);
            if flow && !made.is_empty() {
                for mv in edit_edge_flow(&m, &made, &[], &EdgeFlowOptions::default()) {
                    m.set_position(mv.point, mv.position);
                }
            }
            m.compact(None);
            Ok(m)
        });
        match result {
            Ok(m) => d.set_output(OUT_MESH, PlugValue::Mesh(Some(Arc::new(m)))),
            Err(e) => {
                log::warn!("splitRingsModifier: {e}");
                self.error = e;
            }
        }
    }
    fn traits(&self) -> u32 {
        traits::MODIFIER
    }
    fn as_modifier(&self) -> Option<&dyn ModifierBehavior> {
        Some(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ModifierBehavior for SplitRingsModifierNode {
    fn in_attr(&self) -> usize {
        IN_MESH
    }
    fn enabled_attr(&self) -> usize {
        ENABLED
    }
    fn out_attr(&self) -> usize {
        OUT_MESH
    }
    fn editing_input(&self, data: &NodeData) -> bool {
        data.bool(EDITING)
    }
    fn error(&self) -> &str {
        &self.error
    }
}

pub fn is_split_rings(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.behavior().as_any().is::<SplitRingsModifierNode>())
}

pub fn editing(g: &Graph, id: NodeId) -> bool {
    is_split_rings(g, id) && g.node(id).unwrap().bool(EDITING)
}

/// The edges or faces a splitRingsModifier splits, found on its current input.
#[derive(Debug, Clone, Default)]
pub struct Selection {
    pub kind: ComponentMode,
    pub ids: Vec<Index>,
    pub lost: i32,
    /// No stored components: the whole mesh.
    pub whole: bool,
}

pub fn selection(g: &mut Graph, modifier: NodeId) -> Selection {
    let mut out = Selection { kind: ComponentMode::Face, ..Default::default() };
    if !is_split_rings(g, modifier) {
        return out;
    }
    g.evaluate();
    let n = g.node(modifier).unwrap();
    let stored = components_of(n.data());
    out.whole = stored.is_none();
    let Some(input) = n.data().mesh(IN_MESH).cloned() else { return out };
    let r = resolve_components(&input, stored.as_ref());
    out.kind = r.kind;
    out.ids = r.ids;
    out.lost = r.lost;
    out
}

/// A direct edit compacted the base mesh: the components of a splitRingsModifier first in the stack follow to the new
/// ids.
pub fn base_remapped(g: &mut Graph, mesh: NodeId, remap: &MeshRemap, base: &Mesh) {
    let Some(&first) = modifiers::stack(g, mesh).first() else { return };
    if !is_split_rings(g, first) {
        return;
    }
    let current = components_of(g.node(first).unwrap().data());
    if let Some(next) = remap_components(current.as_ref(), remap, base) {
        let _ = g.set_value(PlugRef::new(first, COMPONENTS as i32), PlugValue::Data(Some(Arc::new(next))));
    }
}

pub fn register_types() {
    if types::find("splitRingsModifier").is_some() {
        return;
    }
    data::register_kind(
        "splitRingsEditBackup",
        Arc::new(|j| Ok(Arc::new(EditBackup { value: j.get("value").cloned().unwrap_or(Value::Null) }) as Arc<dyn Data>)),
    );
    let mut direction = param(
        "direction",
        PlugType::Enum,
        0i64,
        "Faces: along adds loops down the selection's long way, across its short way.",
        None,
        None,
    );
    direction.enum_names = vec!["along".into(), "across".into()];
    let hidden = |name: &str, ty: PlugType, def: PlugValue, doc: &str| AttrDef {
        name: name.into(),
        ty,
        default_value: def,
        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
        doc: doc.into(),
        ..Default::default()
    };
    let t = NodeType {
        name: "splitRingsModifier".into(),
        icon: "stack-2".into(),
        doc: "Subdivides one way, kept live: loops across the rings of the edges or faces selected when it was added, \
              curved to the surface. Without a selection it splits the whole mesh."
            .into(),
        own_attrs: vec![
            mesh_attr("inMesh", "", flags::INPUT | flags::HIDDEN, ""),
            param("enabled", PlugType::Bool, true, "Turn the modifier off without removing it.", None, None),
            AttrDef {
                name: "exportEnabled".into(),
                ty: PlugType::Bool,
                default_value: PlugValue::Bool(true),
                flags: flags::INPUT | flags::STORABLE,
                doc: "Apply the modifier when exporting.".into(),
                ..Default::default()
            },
            param("count", PlugType::Int, 1i64, "Loops per ring, evenly spaced, 1 to 32.", Some(1.0), Some(32.0)),
            direction,
            param("edgeFlow", PlugType::Bool, true, "Curve the new loops to follow the surrounding surface rather than lie flat.", None, None),
            hidden("components", PlugType::Data, PlugValue::Data(None), "The edges or faces whose rings are split."),
            hidden("editing", PlugType::Bool, PlugValue::Bool(false), "Edit Selection is in progress."),
            hidden("editBackup", PlugType::Data, PlugValue::Data(None), "The selection to restore when Edit Selection ends."),
            mesh_attr("outMesh", "", flags::OUTPUT | flags::HIDDEN, ""),
            AttrDef {
                name: "lostComponents".into(),
                ty: PlugType::Int,
                default_value: PlugValue::Int(0),
                flags: flags::OUTPUT,
                doc: "Edges or faces no longer found on the input mesh.".into(),
                ..Default::default()
            },
        ],
        create: Some(Arc::new(|| Box::new(SplitRingsModifierNode::default()))),
        ..Default::default()
    };
    types::register_type(t).expect("splitRingsModifier");
    // addModifier's hook: the new modifier takes the selected edges or faces; with none it splits the whole mesh.
    modifiers::set_create_hook(
        "splitRingsModifier",
        Arc::new(|g, modifier, mesh| {
            if let Some(c) = selected_components(g, mesh) {
                let _ = g.set_value(PlugRef::new(modifier, COMPONENTS as i32), PlugValue::Data(Some(c)));
            }
        }),
    );
    modifiers::add_remap_hook(Arc::new(base_remapped));
}

/// A named splitRingsModifier, or the one editing (else the topmost) in a named or selected mesh's stack.
fn modifier_arg(s: &Session, a: &Args, cmd: &str) -> CmdResult<NodeId> {
    let g = &s.graph;
    for name in objects(s, a) {
        let Ok(Some(n)) = g.find(&name) else { continue };
        if is_split_rings(g, n) {
            return Ok(n);
        }
        if let Some(n) = crate::components::as_mesh(g, n) {
            let stack = modifiers::stack(g, n);
            if let Some(&id) = stack.iter().find(|&&id| editing(g, id)) {
                return Ok(id);
            }
            if let Some(&id) = stack.iter().rev().find(|&&id| is_split_rings(g, id)) {
                return Ok(id);
            }
        }
    }
    fail!("{cmd}: give a splitRings modifier or a mesh that has one")
}

fn mesh_arg(g: &Graph, m: NodeId, cmd: &str) -> CmdResult<NodeId> {
    let mesh = modifiers::mesh_of(g, m);
    if mesh == crate::plug::NO_NODE {
        fail!("{cmd}: {} feeds no mesh", g.path(m));
    }
    Ok(mesh)
}

fn end_edit(g: &mut Graph, m: NodeId, mesh: NodeId) -> CmdResult<()> {
    let backup = g
        .node(m)
        .unwrap()
        .value(EDIT_BACKUP)
        .as_data()
        .and_then(|d| d.as_any().downcast_ref::<EditBackup>().map(|b| b.value.clone()))
        .unwrap_or(Value::Null);
    g.set_value(PlugRef::new(m, EDITING as i32), PlugValue::Bool(false))?;
    g.set_value(PlugRef::new(m, EDIT_BACKUP as i32), PlugValue::Data(None))?;
    modifiers::restore_selection(g, mesh, &backup);
    Ok(())
}

pub fn register(c: &mut Commands<Session>) {
    type Cmd = CommandDef<Session>;
    c.register(
        Cmd::new(
            "splitRingsEditSelection",
            "Show a splitRings modifier's input mesh (it and later modifiers pass through) with the edges or faces it \
             splits selected, so they can be changed with the normal selection tools. Finish with splitRingsApplySelection \
             or splitRingsCancelSelection.",
            |s, a| {
                let m = modifier_arg(s, a, "splitRingsEditSelection")?;
                let g = &mut s.graph;
                let mesh = mesh_arg(g, m, "splitRingsEditSelection")?;
                for id in modifiers::stack(g, mesh) {
                    if id != m && modifiers::editing_input(g, id) {
                        fail!("splitRingsEditSelection: {} is already editing its edges", g.path(id));
                    }
                }
                if !editing(g, m) {
                    g.evaluate();
                    let value = modifiers::selection_snapshot(g, mesh);
                    g.set_value(PlugRef::new(m, EDIT_BACKUP as i32), PlugValue::Data(Some(Arc::new(EditBackup { value }))))?;
                    g.set_value(PlugRef::new(m, EDITING as i32), PlugValue::Bool(true))?;
                }
                let sel = selection(g, m);
                g.select(&[mesh], SelectMode::Replace);
                components::select(g, mesh, sel.kind, &sel.ids, Op::Replace);
                components::set_mode(g, sel.kind);
                Ok(json!(g.path(m)))
            },
        )
        .positional("modifier or mesh"),
    );

    c.register(
        Cmd::new(
            "splitRingsApplySelection",
            "Store the selected edges or faces (with their mirror images under symmetry) as what a splitRings modifier \
             splits, and end Edit Selection. Returns how many it stored.",
            |s, a| {
                let m = modifier_arg(s, a, "splitRingsApplySelection")?;
                let g = &mut s.graph;
                if !editing(g, m) {
                    fail!("splitRingsApplySelection: {} is not editing its selection", g.path(m));
                }
                let mesh = mesh_arg(g, m, "splitRingsApplySelection")?;
                let Some(picked) = selected_components(g, mesh) else {
                    fail!("splitRingsApplySelection: select the edges or faces to split, or cancel")
                };
                let c = picked.as_any().downcast_ref::<ExtrudeComponents>().unwrap();
                let count = if c.edges { c.edge_refs.len() } else { c.faces.len() };
                g.set_value(PlugRef::new(m, COMPONENTS as i32), PlugValue::Data(Some(picked)))?;
                end_edit(g, m, mesh)?;
                Ok(json!(count))
            },
        )
        .positional("modifier or mesh"),
    );

    c.register(
        Cmd::new("splitRingsCancelSelection", "End a splitRings modifier's Edit Selection without changing what it splits.", |s, a| {
            let m = modifier_arg(s, a, "splitRingsCancelSelection")?;
            let g = &mut s.graph;
            if !editing(g, m) {
                fail!("splitRingsCancelSelection: {} is not editing its selection", g.path(m));
            }
            let mesh = mesh_arg(g, m, "splitRingsCancelSelection")?;
            end_edit(g, m, mesh)?;
            Ok(Value::Null)
        })
        .positional("modifier or mesh"),
    );
}
