//! A live extrude, remembering its faces or edges well enough to find
//! them again after the mesh under it changes: by id while the topology is the one they were taken on (the
//! capacities), else faces by centre and edges by `EdgeMatcher`. Components found nowhere are dropped.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use aztek_geo::fold_edges::{edge_ref, EdgeMatcher, EdgeRef};
use aztek_geo::ops::{extrude_edges, extrude_faces, EdgeExtrudeOptions, ExtrudeOptions};
use aztek_geo::symmetry::SymmetryMap;
use aztek_geo::{Index, Mesh, MeshRemap, INVALID};
use glam::{DVec3, IVec3};
use serde_json::{json, Value};

use crate::attr_defs::{mesh_attr, param};
use crate::components::{self, ComponentMode};
use crate::graph::Graph;
use crate::mesh_nodes::{symmetry_offset, MeshNode};
use crate::modifiers::{self, guarded, ModifierBehavior};
use crate::node::{traits, types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{data, flags, AttrDef, Data, DataPtr, MeshPtr, NodeId, PlugRef, PlugType, PlugValue};

pub const IN_MESH: usize = 0;
pub const ENABLED: usize = 1;
pub const EXPORT_ENABLED: usize = 2;
pub const THICKNESS: usize = 3;
pub const OFFSET: usize = 4;
pub const INDIVIDUAL: usize = 5;
pub const KEEP_BACK: usize = 6;
pub const COMPONENTS: usize = 7;
pub const OUT_MESH: usize = 8;
pub const LOST: usize = 9;

#[derive(Debug, Clone, Copy)]
pub struct FaceRef {
    pub face: Index,
    pub centre: DVec3,
}

/// The faces or edges an extrudeModifier extrudes.
#[derive(Debug, Clone, Default)]
pub struct ExtrudeComponents {
    pub edges: bool,
    pub faces: Vec<FaceRef>,
    pub edge_refs: Vec<EdgeRef>,
    pub point_count: Index,
    pub edge_count: Index,
    pub face_count: Index,
}

fn edge_ref_to_json(r: &EdgeRef) -> Value {
    json!([r.edge, r.p0, r.p1, r.a.x, r.a.y, r.a.z, r.b.x, r.b.y, r.b.z])
}

fn edge_ref_from_json(j: &Value) -> EdgeRef {
    let mut r = EdgeRef::default();
    let Some(a) = j.as_array().filter(|a| a.len() == 9) else { return r };
    let u = |i: usize| a[i].as_u64().unwrap_or(INVALID as u64) as Index;
    let f = |i: usize| a[i].as_f64().unwrap_or(0.0);
    r.edge = u(0);
    r.p0 = u(1);
    r.p1 = u(2);
    r.a = DVec3::new(f(3), f(4), f(5));
    r.b = DVec3::new(f(6), f(7), f(8));
    r
}

impl ExtrudeComponents {
    pub fn from_json(j: &Value) -> ExtrudeComponents {
        let u = |k: &str| j.get(k).and_then(Value::as_u64).unwrap_or(0) as Index;
        let mut c = ExtrudeComponents {
            edges: j.get("edges").and_then(Value::as_bool).unwrap_or(false),
            point_count: u("points"),
            edge_count: u("edgeCount"),
            face_count: u("faceCount"),
            ..Default::default()
        };
        for r in j.get("faces").and_then(Value::as_array).into_iter().flatten() {
            if let Some(a) = r.as_array().filter(|a| a.len() == 4) {
                let f = |i: usize| a[i].as_f64().unwrap_or(0.0);
                c.faces.push(FaceRef { face: a[0].as_u64().unwrap_or(INVALID as u64) as Index, centre: DVec3::new(f(1), f(2), f(3)) });
            }
        }
        for r in j.get("edgeRefs").and_then(Value::as_array).into_iter().flatten() {
            c.edge_refs.push(edge_ref_from_json(r));
        }
        c
    }

    /// Remembers ids (faces, or edges when `edges`) on `mesh`.
    pub fn take(mesh: &Mesh, edges: bool, ids: &[Index]) -> ExtrudeComponents {
        let mut c = ExtrudeComponents {
            edges,
            point_count: mesh.point_capacity(),
            edge_count: mesh.edge_capacity(),
            face_count: mesh.face_capacity(),
            ..Default::default()
        };
        for &id in ids {
            if edges && id < mesh.edge_capacity() && mesh.edge_valid(id) {
                c.edge_refs.push(edge_ref(mesh, id));
            } else if !edges && id < mesh.face_capacity() && mesh.face_valid(id) {
                c.faces.push(FaceRef { face: id, centre: mesh.face_centroid(id) });
            }
        }
        c
    }
}

impl Data for ExtrudeComponents {
    fn kind(&self) -> &str {
        "extrudeComponents"
    }
    fn to_json(&self) -> Value {
        let f: Vec<Value> = self.faces.iter().map(|r| json!([r.face, r.centre.x, r.centre.y, r.centre.z])).collect();
        let e: Vec<Value> = self.edge_refs.iter().map(edge_ref_to_json).collect();
        json!({
            "kind": "extrudeComponents",
            "edges": self.edges,
            "points": self.point_count,
            "edgeCount": self.edge_count,
            "faceCount": self.face_count,
            "faces": f,
            "edgeRefs": e,
        })
    }
    fn bytes(&self) -> usize {
        std::mem::size_of::<Self>() + self.faces.len() * std::mem::size_of::<FaceRef>() + self.edge_refs.len() * std::mem::size_of::<EdgeRef>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Finds faces again by their centre, on a hash grid of cells a few tolerances wide.
struct CentreMatcher<'a> {
    mesh: &'a Mesh,
    tolerance: f64,
    cell: f64,
    grid: HashMap<u64, Vec<Index>>,
}

impl<'a> CentreMatcher<'a> {
    fn new(m: &'a Mesh) -> Self {
        let tolerance = (m.bounds().size().length() * 1e-6).max(1e-6);
        let mut c = CentreMatcher { mesh: m, tolerance, cell: tolerance * 4.0, grid: HashMap::new() };
        for f in 0..m.face_capacity() {
            if m.face_valid(f) {
                let k = Self::key(c.cell_of(m.face_centroid(f)));
                c.grid.entry(k).or_default().push(f);
            }
        }
        c
    }

    fn cell_of(&self, p: DVec3) -> IVec3 {
        (p / self.cell).floor().as_ivec3()
    }

    fn key(c: IVec3) -> u64 {
        (c.x as u32 as u64).wrapping_mul(73856093) ^ (c.y as u32 as u64).wrapping_mul(19349663) ^ (c.z as u32 as u64).wrapping_mul(83492791)
    }

    fn find(&self, centre: DVec3) -> Index {
        let c = self.cell_of(centre);
        let mut best = INVALID;
        let mut best_distance = self.tolerance;
        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    if let Some(list) = self.grid.get(&Self::key(c + IVec3::new(dx, dy, dz))) {
                        for &f in list {
                            let d = (self.mesh.face_centroid(f) - centre).length();
                            if d <= best_distance {
                                best = f;
                                best_distance = d;
                            }
                        }
                    }
                }
            }
        }
        best
    }
}

fn components_match(m: &Mesh, c: &ExtrudeComponents) -> bool {
    m.point_capacity() == c.point_count && m.edge_capacity() == c.edge_count && m.face_capacity() == c.face_count
}

/// What a modifier extrudes on its current input. No stored components means every face.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub input: Option<MeshPtr>,
    pub kind: ComponentMode,
    pub ids: Vec<Index>,
    /// Stored components found nowhere on the input.
    pub lost: i32,
}

/// Finds stored components on m; none stored means every face.
pub fn resolve_components(m: &Mesh, c: Option<&ExtrudeComponents>) -> Resolved {
    let mut r = Resolved { input: None, kind: ComponentMode::Face, ids: Vec::new(), lost: 0 };
    let Some(c) = c else {
        r.ids = (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect();
        return r;
    };
    let same = components_match(m, c);
    if c.edges {
        r.kind = ComponentMode::Edge;
        let matcher = EdgeMatcher::new(m);
        for e in &c.edge_refs {
            let (found, _) = matcher.find(e, same);
            if found != INVALID {
                r.ids.push(found);
            } else {
                r.lost += 1;
            }
        }
    } else {
        let mut matcher: Option<CentreMatcher> = None;
        for fr in &c.faces {
            let mut f = if same && fr.face < m.face_capacity() && m.face_valid(fr.face) { fr.face } else { INVALID };
            if f == INVALID {
                f = matcher.get_or_insert_with(|| CentreMatcher::new(m)).find(fr.centre);
            }
            if f != INVALID {
                r.ids.push(f);
            } else {
                r.lost += 1;
            }
        }
    }
    r.ids.sort_unstable();
    r.ids.dedup();
    r
}

fn components_of(d: &NodeData) -> Option<ExtrudeComponents> {
    d.value(COMPONENTS).as_data().and_then(|p| p.as_any().downcast_ref::<ExtrudeComponents>().cloned())
}

#[derive(Clone, Default)]
pub struct ExtrudeModifierNode {
    error: String,
}

impl NodeBehavior for ExtrudeModifierNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        self.error.clear();
        let input = d.mesh(IN_MESH).cloned();
        d.set_output(OUT_MESH, PlugValue::Mesh(input.clone()));
        d.set_output(LOST, PlugValue::Int(0));
        let Some(input) = input else { return };
        if !d.bool(ENABLED) || modifiers::bypassed(Some(&input)) {
            return;
        }
        let r = resolve_components(&input, components_of(d).as_ref());
        d.set_output(LOST, PlugValue::Int(r.lost as i64));
        if r.ids.is_empty() {
            return;
        }
        let (thickness, offset, individual, keep_back) = (d.float(THICKNESS), d.float(OFFSET), d.bool(INDIVIDUAL), d.bool(KEEP_BACK));
        let result = guarded(|| {
            let mut m = (*input).clone();
            if r.kind == ComponentMode::Edge {
                extrude_edges(&mut m, &r.ids, &EdgeExtrudeOptions { thickness, offset, symmetry: None });
            } else {
                extrude_faces(&mut m, &r.ids, &ExtrudeOptions { offset: thickness, individual, symmetry: None, inset: offset, keep_back });
            }
            m.compact(None);
            Ok(m)
        });
        match result {
            Ok(m) => d.set_output(OUT_MESH, PlugValue::Mesh(Some(Arc::new(m)))),
            Err(e) => {
                log::warn!("extrudeModifier: {e}");
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

impl ModifierBehavior for ExtrudeModifierNode {
    fn in_attr(&self) -> usize {
        IN_MESH
    }
    fn enabled_attr(&self) -> usize {
        ENABLED
    }
    fn out_attr(&self) -> usize {
        OUT_MESH
    }
    fn error(&self) -> &str {
        &self.error
    }
}

pub fn is_extrude(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.behavior().as_any().is::<ExtrudeModifierNode>())
}

/// Evaluates the graph. None when the modifier has no input.
pub fn resolve(g: &mut Graph, modifier: NodeId) -> Option<Resolved> {
    if !is_extrude(g, modifier) {
        return None;
    }
    g.evaluate();
    let n = g.node(modifier)?;
    let input = n.data().mesh(IN_MESH).cloned()?;
    let mut r = resolve_components(&input, components_of(n.data()).as_ref());
    r.input = Some(input);
    Some(r)
}

/// The mesh's selected faces, or edges in edge mode, with their mirror images under symmetry; None in object mode or
/// with none.
pub fn selected_components(g: &mut Graph, mesh: NodeId) -> Option<DataPtr> {
    let mode = components::mode(g);
    if mode == ComponentMode::Object {
        return None;
    }
    let sel = components::selected(g, mesh, mode).to_vec();
    if sel.is_empty() {
        return None;
    }
    let cage = modifiers::cage(g, mesh)?;
    let edges = mode == ComponentMode::Edge;
    let mut ids = components::convert(&cage, mode, &sel, if edges { ComponentMode::Edge } else { ComponentMode::Face }, false);
    let n = g.node(mesh).unwrap();
    let axis = n.int(MeshNode::SYMMETRY);
    if (1..=3).contains(&axis) {
        let mut copy = (*cage).clone();
        let tolerance = (copy.bounds().size().length() * 1e-5).max(1e-4);
        let map = SymmetryMap::persistent_axis(&mut copy, (axis - 1) as i32, tolerance, None, symmetry_offset(g, mesh, (axis - 1) as i32));
        ids = if edges { map.expand_edges(&copy, &ids) } else { map.expand_faces(&copy, &ids) };
    }
    Some(Arc::new(ExtrudeComponents::take(&cage, edges, &ids)))
}

/// c moved to base's ids after a compaction, or None when c was taken on other topology.
pub fn remap_components(c: Option<&ExtrudeComponents>, remap: &MeshRemap, base: &Mesh) -> Option<ExtrudeComponents> {
    let c = c?;
    // Ids taken on some other topology were already matched by position; the remap can't say where they went.
    if c.face_count as usize > remap.faces.len() || c.edge_count as usize > remap.edges.len() {
        return None;
    }
    let mut next = c.clone();
    next.point_count = base.point_capacity();
    next.edge_count = base.edge_capacity();
    next.face_count = base.face_capacity();
    for r in &mut next.faces {
        let f = remap.faces.get(r.face as usize).copied().unwrap_or(INVALID);
        // Gone: only its centre can find it again.
        if f == INVALID || !base.face_valid(f) {
            r.face = INVALID;
        } else {
            *r = FaceRef { face: f, centre: base.face_centroid(f) };
        }
    }
    for r in &mut next.edge_refs {
        let e = remap.edges.get(r.edge as usize).copied().unwrap_or(INVALID);
        if e == INVALID || !base.edge_valid(e) {
            r.edge = INVALID;
            r.p0 = INVALID;
            r.p1 = INVALID;
        } else {
            *r = edge_ref(base, e);
        }
    }
    Some(next)
}

/// A direct edit compacted the base mesh: the components of an extrudeModifier first in the stack follow to the new
/// ids.
pub fn base_remapped(g: &mut Graph, mesh: NodeId, remap: &MeshRemap, base: &Mesh) {
    let Some(&first) = modifiers::stack(g, mesh).first() else { return };
    if !is_extrude(g, first) {
        return;
    }
    let current = components_of(g.node(first).unwrap().data());
    if let Some(next) = remap_components(current.as_ref(), remap, base) {
        let _ = g.set_value(PlugRef::new(first, COMPONENTS as i32), PlugValue::Data(Some(Arc::new(next))));
    }
}

pub fn register_types() {
    if types::find("extrudeModifier").is_some() {
        return;
    }
    data::register_kind("extrudeComponents", Arc::new(|j| Ok(Arc::new(ExtrudeComponents::from_json(j)) as DataPtr)));
    let t = NodeType {
        name: "extrudeModifier".into(),
        icon: "stack-2".into(),
        doc: "Extrudes faces, or grows strips off edges, kept live: the faces or edges selected when it was added follow \
              later edits under it, and its thickness and offset stay adjustable. Without a selection it extrudes every face."
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
            param("thickness", PlugType::Float, 10.0, "Distance along the normals, cm.", None, None),
            param(
                "offset",
                PlugType::Float,
                0.0,
                "Faces: inward distance of the extruded border, cm; negative grows them. Edges: outward distance from their \
                 faces, cm.",
                None,
                None,
            ),
            param("individual", PlugType::Bool, false, "Extrude each face separately instead of connected faces as regions.", None, None),
            param(
                "keepBack",
                PlugType::Bool,
                true,
                "Where the faces only border open edges, as a whole open shell, keep a copy facing back at the base.",
                None,
                None,
            ),
            AttrDef {
                name: "components".into(),
                ty: PlugType::Data,
                default_value: PlugValue::Data(None),
                flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                doc: "The faces or edges to extrude.".into(),
                ..Default::default()
            },
            mesh_attr("outMesh", "", flags::OUTPUT | flags::HIDDEN, ""),
            AttrDef {
                name: "lostComponents".into(),
                ty: PlugType::Int,
                default_value: PlugValue::Int(0),
                flags: flags::OUTPUT,
                doc: "Faces or edges no longer found on the input mesh.".into(),
                ..Default::default()
            },
        ],
        create: Some(Arc::new(|| Box::new(ExtrudeModifierNode::default()))),
        ..Default::default()
    };
    types::register_type(t).expect("extrudeModifier");
    // addModifier's hook: the new modifier takes the mesh's selected faces, or edges in edge mode, with their mirror
    // images under symmetry.
    modifiers::set_create_hook(
        "extrudeModifier",
        Arc::new(|g, modifier, mesh| {
            if let Some(c) = selected_components(g, mesh) {
                let _ = g.set_value(PlugRef::new(modifier, COMPONENTS as i32), PlugValue::Data(Some(c)));
            }
        }),
    );
    modifiers::add_remap_hook(Arc::new(base_remapped));
}
