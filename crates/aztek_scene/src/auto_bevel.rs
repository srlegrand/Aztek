//! The autoBevel modifier, which bevels fold edges found live on its input by
//! angle, side and length rules, plus edges picked by hand, and its commands.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use aztek_geo::fold_edges::{angle_edges, edge_ref, fold_edges, weighted_edges, EdgeMatcher, EdgeRef, FoldEdgeOptions, FoldSide};
use aztek_geo::ops::{bevel_edges, BevelOptions};
use aztek_geo::{AttrType, Index, Mesh, MeshRemap, INVALID};
use aztek_ops::{execute, fail, objects, ArgType, Args, CallSource, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Map, Value};

use crate::attr_defs::mesh_attr;
use crate::components::{self, ComponentMode, Op};
use crate::graph::{Graph, SelectMode};
use crate::modifiers::{self, guarded, ModifierBehavior, BYPASS_LAYER};
use crate::node::{traits, types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{data, flags, AttrDef, Data, MeshPtr, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;

pub const IN_MESH: usize = 0;
pub const ENABLED: usize = 1;
pub const MODE: usize = 2;
pub const MIN_ANGLE: usize = 3;
pub const MAX_ANGLE: usize = 4;
pub const SIDE: usize = 5;
pub const THIN: usize = 6;
pub const MIN_LENGTH: usize = 7;
pub const COMPLETE_CORNERS: usize = 8;
pub const WEIGHT_THRESHOLD: usize = 9;
pub const OFFSET: usize = 10;
pub const SEGMENTS: usize = 11;
pub const PROFILE: usize = 12;
pub const CLAMP: usize = 13;
pub const SMOOTHING_ANGLE: usize = 14;
pub const SHOW_EDGES: usize = 15;
pub const EDITING: usize = 16;
pub const OVERRIDES: usize = 17;
pub const EDIT_BACKUP: usize = 18;
pub const OUT_MESH: usize = 19;
pub const CHOSEN: usize = 20;
pub const EDGE_COUNT: usize = 21;
pub const LOST_OVERRIDES: usize = 22;

const AUTO: i64 = 0;
const AUTO_OVERRIDES: i64 = 1;
const MANUAL: i64 = 2;
const ANGLE: i64 = 3;
const WEIGHT: i64 = 4;

const MODE_NAMES: [&str; 5] = ["auto", "autoOverrides", "manual", "angle", "weight"];
const SIDE_NAMES: [&str; 3] = ["both", "outside", "inside"];

/// Hand edits to an autoBevelModifier's edge set, stored as edge refs on the modifier's input mesh so they survive
/// upstream topology changes. The capacities say which mesh the ids belong to.
#[derive(Debug, Clone, Default)]
pub struct BevelOverrides {
    pub add: Vec<EdgeRef>,
    pub remove: Vec<EdgeRef>,
    pub points: Index,
    pub edges: Index,
    pub faces: Index,
}

fn ref_to_json(r: &EdgeRef) -> Value {
    json!([r.edge, r.p0, r.p1, r.a.x, r.a.y, r.a.z, r.b.x, r.b.y, r.b.z])
}

fn ref_from_json(j: &Value) -> EdgeRef {
    let mut r = EdgeRef::default();
    let Some(a) = j.as_array().filter(|a| a.len() == 9) else { return r };
    let u = |i: usize| a[i].as_u64().unwrap_or(INVALID as u64) as Index;
    let f = |i: usize| a[i].as_f64().unwrap_or(0.0);
    r.edge = u(0);
    r.p0 = u(1);
    r.p1 = u(2);
    r.a = glam::DVec3::new(f(3), f(4), f(5));
    r.b = glam::DVec3::new(f(6), f(7), f(8));
    r
}

impl BevelOverrides {
    pub fn from_json(j: &Value) -> BevelOverrides {
        let u = |k: &str| j.get(k).and_then(Value::as_u64).unwrap_or(0) as Index;
        let refs = |k: &str| j.get(k).and_then(Value::as_array).map(|l| l.iter().map(ref_from_json).collect()).unwrap_or_default();
        BevelOverrides { add: refs("add"), remove: refs("remove"), points: u("points"), edges: u("edges"), faces: u("faces") }
    }
}

impl Data for BevelOverrides {
    fn kind(&self) -> &str {
        "bevelOverrides"
    }
    fn to_json(&self) -> Value {
        let added: Vec<Value> = self.add.iter().map(ref_to_json).collect();
        let removed: Vec<Value> = self.remove.iter().map(ref_to_json).collect();
        json!({"kind": "bevelOverrides", "points": self.points, "edges": self.edges, "faces": self.faces, "add": added, "remove": removed})
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// What an autoBevelModifier picked on its last evaluation: edge ids on its input mesh.
#[derive(Clone, Default)]
pub struct BevelEdgeSet {
    pub mesh: Option<MeshPtr>,
    pub edges: Vec<Index>,
    /// Overrides that matched no edge.
    pub lost: i32,
}

impl fmt::Debug for BevelEdgeSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("BevelEdgeSet").field("edges", &self.edges.len()).field("lost", &self.lost).finish()
    }
}

impl Data for BevelEdgeSet {
    fn kind(&self) -> &str {
        "bevelEdgeSet"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "bevelEdgeSet", "count": self.edges.len(), "lost": self.lost})
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Opaque json payload: the selection to restore when Edit Edges ends.
#[derive(Debug)]
struct JsonData {
    value: Value,
}

impl Data for JsonData {
    fn kind(&self) -> &str {
        "bevelEditBackup"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "bevelEditBackup", "value": self.value})
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn fold_options(d: &NodeData) -> FoldEdgeOptions {
    FoldEdgeOptions {
        min_angle: d.float(MIN_ANGLE),
        max_angle: d.float(MAX_ANGLE),
        side: match d.int(SIDE).clamp(0, 2) {
            0 => FoldSide::Both,
            1 => FoldSide::Outside,
            _ => FoldSide::Inside,
        },
        thin: d.float(THIN),
        min_length: d.float(MIN_LENGTH),
        complete_corners: d.bool(COMPLETE_CORNERS),
    }
}

/// The edges the mode's rule picks, before overrides.
fn rule_edges(m: &Mesh, d: &NodeData) -> Vec<Index> {
    match d.int(MODE) {
        AUTO | AUTO_OVERRIDES => fold_edges(m, &fold_options(d)),
        ANGLE => angle_edges(m, d.float(MIN_ANGLE)),
        WEIGHT => weighted_edges(m, d.float(WEIGHT_THRESHOLD)),
        _ => Vec::new(),
    }
}

fn overrides_of(d: &NodeData) -> Option<BevelOverrides> {
    d.value(OVERRIDES).as_data().and_then(|p| p.as_any().downcast_ref::<BevelOverrides>().cloned())
}

/// Rule plus overrides; plain auto ignores the overrides. Returns the edges and how many overrides were lost.
fn choose_edges(m: &Mesh, d: &NodeData) -> (Vec<Index>, i32) {
    let rule = rule_edges(m, d);
    let o = overrides_of(d);
    let Some(o) = o.filter(|o| d.int(MODE) != AUTO && !(o.add.is_empty() && o.remove.is_empty())) else { return (rule, 0) };
    let mut on = vec![false; m.edge_capacity() as usize];
    for e in rule {
        on[e as usize] = true;
    }
    let matcher = EdgeMatcher::new(m);
    let same = m.point_capacity() == o.points && m.edge_capacity() == o.edges && m.face_capacity() == o.faces;
    let mut lost = 0;
    for (refs, value) in [(&o.remove, false), (&o.add, true)] {
        for r in refs {
            let (e, _) = matcher.find(r, same);
            if e == INVALID {
                lost += 1;
                continue;
            }
            on[e as usize] = value;
        }
    }
    ((0..on.len() as Index).filter(|&e| on[e as usize]).collect(), lost)
}

#[derive(Clone, Default)]
pub struct AutoBevelNode {
    error: String,
}

impl NodeBehavior for AutoBevelNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        self.error.clear();
        let input = d.mesh(IN_MESH).cloned();
        let mut set = BevelEdgeSet { mesh: input.clone(), ..Default::default() };
        d.set_output(OUT_MESH, PlugValue::Mesh(input.clone()));
        d.set_output(EDGE_COUNT, PlugValue::Int(0));
        d.set_output(LOST_OVERRIDES, PlugValue::Int(0));
        let editing = d.bool(EDITING);
        let skip = match &input {
            None => true,
            Some(i) => (!d.bool(ENABLED) && !editing) || modifiers::bypassed(Some(i)),
        };
        if !skip {
            let input = input.unwrap();
            let opts = BevelOptions {
                offset: d.float(OFFSET),
                segments: d.int(SEGMENTS).clamp(1, 32) as i32,
                profile: d.float(PROFILE),
                clamp: d.bool(CLAMP),
                smoothing_angle: d.float(SMOOTHING_ANGLE),
                ..Default::default()
            };
            let data = d.clone();
            let r = guarded(|| {
                let (edges, lost) = choose_edges(&input, &data);
                let out = if editing {
                    let mut shown = (*input).clone();
                    shown.point_attrs_mut().add(BYPASS_LAYER, AttrType::Bool);
                    Some(shown)
                } else if !edges.is_empty() {
                    let mut m = (*input).clone();
                    bevel_edges(&mut m, &edges, &opts);
                    m.compact(None);
                    Some(m)
                } else {
                    None
                };
                Ok((edges, lost, out))
            });
            match r {
                Ok((edges, lost, out)) => {
                    d.set_output(EDGE_COUNT, PlugValue::Int(edges.len() as i64));
                    d.set_output(LOST_OVERRIDES, PlugValue::Int(lost as i64));
                    set.edges = edges;
                    set.lost = lost;
                    if let Some(m) = out {
                        d.set_output(OUT_MESH, PlugValue::Mesh(Some(Arc::new(m))));
                    }
                }
                Err(e) => {
                    log::warn!("autoBevelModifier: {e}");
                    self.error = e;
                }
            }
        }
        d.set_output(CHOSEN, PlugValue::Data(Some(Arc::new(set))));
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

impl ModifierBehavior for AutoBevelNode {
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

pub fn is_auto_bevel(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.behavior().as_any().is::<AutoBevelNode>())
}

pub fn editing(g: &Graph, id: NodeId) -> bool {
    is_auto_bevel(g, id) && g.node(id).unwrap().bool(EDITING)
}

/// The modifier's last picked edges, None before it evaluated. Evaluates the graph.
pub fn chosen(g: &mut Graph, modifier: NodeId) -> Option<BevelEdgeSet> {
    if !is_auto_bevel(g, modifier) {
        return None;
    }
    g.evaluate();
    g.node(modifier)?.value(CHOSEN).as_data().and_then(|d| d.as_any().downcast_ref::<BevelEdgeSet>().cloned())
}

/// The mesh node at the top of the modifier's chain, NO_NODE if it feeds none.
pub fn mesh_of(g: &Graph, modifier: NodeId) -> NodeId {
    let mut cur = modifier;
    for _ in 0..4096 {
        let Some(n) = g.node(cur) else { return NO_NODE };
        let Some(m) = n.behavior().as_modifier() else { return NO_NODE };
        let dsts = g.destinations(&PlugRef::new(cur, m.out_attr() as i32));
        let Some(first) = dsts.first() else { return NO_NODE };
        if g.node(first.node).unwrap().ty().is_a("mesh") {
            return first.node;
        }
        cur = first.node;
    }
    NO_NODE
}

/// A direct edit compacted the base mesh: overrides of an autoBevelModifier sitting first in the stack follow their
/// edges to the new ids. Records undo with the edit.
pub fn base_remapped(g: &mut Graph, mesh: NodeId, remap: &MeshRemap, base: &Mesh) {
    let Some(&first) = modifiers::stack(g, mesh).first() else { return };
    if !is_auto_bevel(g, first) {
        return;
    }
    let Some(o) = overrides_of(g.node(first).unwrap().data()) else { return };
    if o.add.is_empty() && o.remove.is_empty() {
        return;
    }
    let mut next = o;
    next.points = base.point_capacity();
    next.edges = base.edge_capacity();
    next.faces = base.face_capacity();
    for r in next.add.iter_mut().chain(next.remove.iter_mut()) {
        let e = remap.edges.get(r.edge as usize).copied().unwrap_or(INVALID);
        // Gone: keep the old ids out of the way so only its position can find it again.
        if e == INVALID || !base.edge_valid(e) {
            r.edge = INVALID;
            r.p0 = INVALID;
            r.p1 = INVALID;
        } else {
            *r = edge_ref(base, e);
        }
    }
    let _ = g.set_value(PlugRef::new(first, OVERRIDES as i32), PlugValue::Data(Some(Arc::new(next))));
}

fn bevel_param(name: &str, ty: PlugType, def: PlugValue, doc: &str, extra: u32, lo: Option<f64>, hi: Option<f64>) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty,
        default_value: def,
        flags: flags::INPUT | flags::STORABLE | extra,
        doc: doc.into(),
        min_value: lo,
        max_value: hi,
        ..Default::default()
    }
}

pub fn register_types() {
    if types::find("autoBevelModifier").is_some() {
        return;
    }
    data::register_kind("bevelOverrides", Arc::new(|j| Ok(Arc::new(BevelOverrides::from_json(j)) as Arc<dyn Data>)));
    data::register_kind(
        "bevelEditBackup",
        Arc::new(|j| Ok(Arc::new(JsonData { value: j.get("value").cloned().unwrap_or(Value::Null) }) as Arc<dyn Data>)),
    );
    let k = flags::KEYABLE;
    let f = PlugValue::Float;
    let b = PlugValue::Bool;
    let mut mode = bevel_param(
        "mode",
        PlugType::Enum,
        PlugValue::Int(AUTO),
        "auto: fold edges by the rule. autoOverrides: the rule plus edges added and removed by hand. manual: only the \
         hand-picked edges. angle: every edge sharper than minAngle (Blender's angle limit). weight: edges whose \
         bevelWeight is above weightThreshold.",
        k,
        None,
        None,
    );
    mode.enum_names = MODE_NAMES.iter().map(|s| s.to_string()).collect();
    let mut side = bevel_param("side", PlugType::Enum, PlugValue::Int(0), "Which folds: both, outside (convex) or inside (concave).", k, None, None);
    side.enum_names = SIDE_NAMES.iter().map(|s| s.to_string()).collect();
    let out = |name: &str, ty: PlugType, def: PlugValue, extra: u32, doc: &str| AttrDef {
        name: name.into(),
        ty,
        default_value: def,
        flags: flags::OUTPUT | extra,
        doc: doc.into(),
        ..Default::default()
    };
    let hidden_in = |name: &str, doc: &str| AttrDef {
        name: name.into(),
        ty: PlugType::Data,
        default_value: PlugValue::Data(None),
        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
        doc: doc.into(),
        ..Default::default()
    };
    let t = NodeType {
        name: "autoBevelModifier".into(),
        icon: "stack-2".into(),
        doc: "Bevels the fold edges it finds live on its input by angle, side and length rules, plus edges picked by hand. \
              Use it for game-art bevels that keep following the mesh as you edit it."
            .into(),
        own_attrs: vec![
            mesh_attr("inMesh", "", flags::INPUT | flags::HIDDEN, ""),
            bevel_param("enabled", PlugType::Bool, b(true), "Turn the modifier off without removing it.", k, None, None),
            mode,
            bevel_param("minAngle", PlugType::Float, f(30.0), "Smallest fold to bevel, degrees between the face normals.", k, Some(0.0), Some(180.0)),
            bevel_param("maxAngle", PlugType::Float, f(180.0), "Largest fold to bevel, degrees.", k, Some(0.0), Some(180.0)),
            side,
            bevel_param(
                "thin",
                PlugType::Float,
                f(0.0),
                "Skip thickness edges: shorter than this or beside a face narrower than this, cm. 0 is off.",
                k,
                Some(0.0),
                None,
            ),
            bevel_param("minLength", PlugType::Float, f(0.0), "Skip edges shorter than this, cm.", k, Some(0.0), None),
            bevel_param("completeCorners", PlugType::Bool, b(true), "Where folds meet at an angle, add the quad loops that join them.", k, None, None),
            bevel_param("weightThreshold", PlugType::Float, f(0.0), "weight mode: bevel edges whose bevelWeight is above this.", k, Some(0.0), Some(1.0)),
            bevel_param("offset", PlugType::Float, f(1.0), "Bevel width, cm.", k, Some(0.0), None),
            bevel_param("segments", PlugType::Int, PlugValue::Int(2), "Edges across each bevel, 1 to 32. More is rounder.", k, Some(1.0), Some(32.0)),
            bevel_param("profile", PlugType::Float, f(0.5), "Profile shape: 0.5 is a round arc, 0.25 a straight chamfer.", k, Some(0.0), Some(1.0)),
            bevel_param("clamp", PlugType::Bool, b(true), "Keep the width under half the shortest neighbouring edge.", k, None, None),
            bevel_param(
                "smoothingAngle",
                PlugType::Float,
                f(30.0),
                "New edges sharper than this are hard; negative leaves them soft.",
                k,
                None,
                Some(180.0),
            ),
            bevel_param(
                "showEdges",
                PlugType::Bool,
                b(false),
                "Highlight the picked edges in the viewport even when the modifier isn't selected.",
                0,
                None,
                None,
            ),
            bevel_param("editing", PlugType::Bool, b(false), "Edit Edges is in progress.", flags::HIDDEN, None, None),
            hidden_in("overrides", "Edges added and removed by hand."),
            hidden_in("editBackup", "Selection to restore when Edit Edges ends."),
            mesh_attr("outMesh", "", flags::OUTPUT | flags::HIDDEN, ""),
            out("chosen", PlugType::Data, PlugValue::Data(None), flags::HIDDEN, ""),
            out("edgeCount", PlugType::Int, PlugValue::Int(0), 0, "Edges bevelled."),
            out("lostOverrides", PlugType::Int, PlugValue::Int(0), 0, "Hand-picked edges no longer found on the input mesh."),
        ],
        create: Some(Arc::new(|| Box::new(AutoBevelNode::default()))),
        ..Default::default()
    };
    types::register_type(t).expect("autoBevelModifier");
    modifiers::add_remap_hook(Arc::new(base_remapped));
}

/// A named autoBevelModifier, or the one editing (else the topmost) in a named or selected mesh's stack.
fn modifier_arg(s: &Session, a: &Args, cmd: &str) -> CmdResult<NodeId> {
    let g = &s.graph;
    for name in objects(s, a) {
        let Ok(Some(n)) = g.find(&name) else { continue };
        if is_auto_bevel(g, n) {
            return Ok(n);
        }
        if let Some(n) = crate::components::as_mesh(g, n) {
            let stack = modifiers::stack(g, n);
            if let Some(&id) = stack.iter().find(|&&id| editing(g, id)) {
                return Ok(id);
            }
            if let Some(&id) = stack.iter().rev().find(|&&id| is_auto_bevel(g, id)) {
                return Ok(id);
            }
        }
    }
    fail!("{cmd}: give an autoBevel modifier or a mesh that has one")
}

fn end_edit(g: &mut Graph, m: NodeId) -> CmdResult<()> {
    let mesh = mesh_of(g, m);
    let backup = g
        .node(m)
        .unwrap()
        .value(EDIT_BACKUP)
        .as_data()
        .and_then(|d| d.as_any().downcast_ref::<JsonData>().map(|b| b.value.clone()))
        .unwrap_or(Value::Null);
    g.set_value(PlugRef::new(m, EDITING as i32), PlugValue::Bool(false))?;
    g.set_value(PlugRef::new(m, EDIT_BACKUP as i32), PlugValue::Data(None))?;
    if mesh != NO_NODE {
        modifiers::restore_selection(g, mesh, &backup);
    }
    Ok(())
}

fn select_chosen(g: &mut Graph, m: NodeId, mesh: NodeId) {
    let edges = chosen(g, m).map(|s| s.edges).unwrap_or_default();
    g.select(&[mesh], SelectMode::Replace);
    components::select(g, mesh, ComponentMode::Edge, &edges, Op::Replace);
    components::set_mode(g, ComponentMode::Edge);
}

fn overrides_for(m: &Mesh, add: &[Index], remove: &[Index]) -> BevelOverrides {
    BevelOverrides {
        points: m.point_capacity(),
        edges: m.edge_capacity(),
        faces: m.face_capacity(),
        add: add.iter().map(|&e| edge_ref(m, e)).collect(),
        remove: remove.iter().map(|&e| edge_ref(m, e)).collect(),
    }
}

fn enum_arg(a: &Args, flag: &str, names: &[&str], cmd: &str) -> CmdResult<i64> {
    let v = a.string(flag, "");
    match names.iter().position(|n| *n == v) {
        Some(i) => Ok(i as i64),
        None => fail!("{cmd}: unknown {flag} '{v}'"),
    }
}

pub fn register(c: &mut Commands<Session>) {
    type T = ArgType;
    type Cmd = CommandDef<Session>;
    let nf = |name: &str, ty, doc: &str| FlagDef::new(name, "", ty, Value::Null, doc);
    let rule_flags = vec![
        nf("minAngle", T::Float, "Smallest fold, degrees between the face normals.").min(0.0).max(180.0),
        nf("maxAngle", T::Float, "Largest fold, degrees.").min(0.0).max(180.0),
        nf("side", T::String, "both, outside (convex) or inside (concave)."),
        nf("thin", T::Float, "Skip thickness edges narrower than this, cm.").min(0.0),
        nf("minLength", T::Float, "Skip edges shorter than this, cm.").min(0.0),
        nf("completeCorners", T::Bool, "Add the quad loops joining folds that meet at an angle."),
    ];
    let mut create_flags = rule_flags.clone();
    create_flags.extend([
        nf("mode", T::String, "auto, autoOverrides, manual, angle or weight."),
        nf("weightThreshold", T::Float, "weight mode threshold.").min(0.0).max(1.0),
        nf("offset", T::Float, "Width, cm.").min(0.0),
        nf("segments", T::Int, "Edges across each bevel, 1 to 32.").min(1.0).max(32.0),
        nf("profile", T::Float, "0.5 round, 0.25 straight.").min(0.0).max(1.0),
        nf("clamp", T::Bool, "Clamp the width."),
        nf("smoothingAngle", T::Float, "Hard-edge angle for the new edges.").max(180.0),
    ]);
    let names: Vec<String> = create_flags.iter().map(|f| f.name.clone()).collect();
    let mut cmd = Cmd::new(
        "autoBevel",
        "Add an autoBevel modifier to the top of each mesh's stack: bevels fold edges found live on its input mesh. \
         Returns the modifier names.",
        move |s, a| {
            let mut params = Map::new();
            for n in &names {
                if let Some(v) = a.get(n) {
                    params.insert(n.clone(), v.clone());
                }
            }
            if a.has("mode") {
                params.insert("mode".into(), json!(enum_arg(a, "mode", &MODE_NAMES, "autoBevel")?));
            }
            if a.has("side") {
                params.insert("side".into(), json!(enum_arg(a, "side", &SIDE_NAMES, "autoBevel")?));
            }
            let mut out = Vec::new();
            for name in objects(s, a) {
                if s.graph.find(&name).ok().flatten().and_then(|n| crate::components::as_mesh(&s.graph, n)).is_some() {
                    let call = json!({"args": [name], "kind": "autoBevel", "params": params});
                    out.push(execute(s, "addModifier", &call, CallSource::Script)?);
                }
            }
            if out.is_empty() {
                fail!("autoBevel: select or name a mesh");
            }
            Ok(Value::Array(out))
        },
    )
    .positional("meshes");
    for f in create_flags {
        cmd = cmd.flag(f);
    }
    c.register(cmd);

    c.register(
        Cmd::new("bevelEdgeInfo", "The edges an autoBevel modifier picks on its input mesh: {edges, count, lost, editing}.", |s, a| {
            let m = modifier_arg(s, a, "bevelEdgeInfo")?;
            let set = chosen(&mut s.graph, m);
            let edges = set.as_ref().map(|s| s.edges.clone()).unwrap_or_default();
            Ok(json!({"edges": edges, "count": edges.len(), "lost": set.map_or(0, |s| s.lost), "editing": editing(&s.graph, m)}))
        })
        .positional("modifier or mesh")
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "bevelEditEdges",
            "Show an autoBevel modifier's input mesh (it and later modifiers pass through) with its edges selected, so \
             they can be changed with the normal selection tools. Finish with bevelApplyEdges or bevelCancelEdges.",
            |s, a| {
                let m = modifier_arg(s, a, "bevelEditEdges")?;
                let g = &mut s.graph;
                let mesh = mesh_of(g, m);
                if mesh == NO_NODE {
                    fail!("bevelEditEdges: {} feeds no mesh", g.path(m));
                }
                for id in modifiers::stack(g, mesh) {
                    if id != m && modifiers::editing_input(g, id) {
                        fail!("bevelEditEdges: {} is already editing its edges", g.path(id));
                    }
                }
                if !editing(g, m) {
                    g.evaluate();
                    let value = modifiers::selection_snapshot(g, mesh);
                    g.set_value(PlugRef::new(m, EDIT_BACKUP as i32), PlugValue::Data(Some(Arc::new(JsonData { value }))))?;
                    g.set_value(PlugRef::new(m, EDITING as i32), PlugValue::Bool(true))?;
                }
                select_chosen(g, m, mesh);
                Ok(json!(g.path(m)))
            },
        )
        .positional("modifier or mesh"),
    );

    c.register(
        Cmd::new(
            "bevelApplyEdges",
            "Store the selected edges as an autoBevel modifier's edge set (as overrides of its rule) and end Edit Edges. \
             Returns {added, removed}.",
            |s, a| {
                let m = modifier_arg(s, a, "bevelApplyEdges")?;
                let g = &mut s.graph;
                if !editing(g, m) {
                    fail!("bevelApplyEdges: {} is not editing its edges", g.path(m));
                }
                let mesh = mesh_of(g, m);
                g.evaluate();
                let input = g.node(m).unwrap().data().mesh(IN_MESH).cloned();
                let (Some(input), true) = (input, mesh != NO_NODE) else {
                    fail!("bevelApplyEdges: {} has no input mesh", g.path(m))
                };
                let mesh_data = &*input;
                let cap = mesh_data.edge_capacity() as usize;
                let (mut want, mut rule) = (vec![false; cap], vec![false; cap]);
                for &e in components::selected(g, mesh, ComponentMode::Edge) {
                    if mesh_data.edge_valid(e) {
                        want[e as usize] = true;
                    }
                }
                let data = g.node(m).unwrap().data().clone();
                let mode = data.int(MODE);
                if mode != MANUAL {
                    for e in rule_edges(mesh_data, &data) {
                        rule[e as usize] = true;
                    }
                }
                let (mut add, mut remove) = (Vec::new(), Vec::new());
                for e in 0..cap {
                    if want[e] && !rule[e] {
                        add.push(e as Index);
                    } else if !want[e] && rule[e] {
                        remove.push(e as Index);
                    }
                }
                g.set_value(PlugRef::new(m, OVERRIDES as i32), PlugValue::Data(Some(Arc::new(overrides_for(mesh_data, &add, &remove)))))?;
                if mode == AUTO && (!add.is_empty() || !remove.is_empty()) {
                    g.set_value(PlugRef::new(m, MODE as i32), PlugValue::Int(AUTO_OVERRIDES))?;
                }
                end_edit(g, m)?;
                Ok(json!({"added": add.len(), "removed": remove.len()}))
            },
        )
        .positional("modifier or mesh"),
    );

    c.register(
        Cmd::new("bevelCancelEdges", "End an autoBevel modifier's Edit Edges without changing its edge set.", |s, a| {
            let m = modifier_arg(s, a, "bevelCancelEdges")?;
            if !editing(&s.graph, m) {
                fail!("bevelCancelEdges: {} is not editing its edges", s.graph.path(m));
            }
            end_edit(&mut s.graph, m)?;
            Ok(Value::Null)
        })
        .positional("modifier or mesh"),
    );

    c.register(
        Cmd::new("bevelClearOverrides", "Drop an autoBevel modifier's hand-picked edges, going back to its rule.", |s, a| {
            let m = modifier_arg(s, a, "bevelClearOverrides")?;
            s.graph.set_value(PlugRef::new(m, OVERRIDES as i32), PlugValue::Data(None))?;
            Ok(Value::Null)
        })
        .positional("modifier or mesh"),
    );

    c.register(
        Cmd::new(
            "bevelMeasureThin",
            "Set an autoBevel modifier's thin to 1.5 times the longest selected edge (pick a thickness edge). Returns it.",
            |s, a| {
                let m = modifier_arg(s, a, "bevelMeasureThin")?;
                let g = &mut s.graph;
                let mesh = mesh_of(g, m);
                let cage = if mesh != NO_NODE { modifiers::cage(g, mesh) } else { None };
                let mut longest: f64 = 0.0;
                if let Some(cage) = &cage {
                    for &e in components::selected(g, mesh, ComponentMode::Edge) {
                        if cage.edge_valid(e) {
                            longest = longest.max(cage.edge_length(e));
                        }
                    }
                }
                if longest <= 0.0 {
                    let who = if mesh != NO_NODE { g.path(mesh) } else { g.path(m) };
                    fail!("bevelMeasureThin: select a thickness edge on {who}");
                }
                g.set_value(PlugRef::new(m, THIN as i32), PlugValue::Float(longest * 1.5))?;
                Ok(json!(longest * 1.5))
            },
        )
        .positional("modifier or mesh"),
    );

    let df = |name: &str, ty, def: Value, doc: &str| FlagDef::new(name, "", ty, def, doc);
    c.register(
        Cmd::new(
            "selectFoldEdges",
            "Select fold edges of meshes by the autoBevel rule (on the editable cage). Returns the number selected.",
            |s, a| {
                let o = FoldEdgeOptions {
                    min_angle: a.float("minAngle", 30.0),
                    max_angle: a.float("maxAngle", 180.0),
                    side: match enum_arg(a, "side", &SIDE_NAMES, "selectFoldEdges")? {
                        0 => FoldSide::Both,
                        1 => FoldSide::Outside,
                        _ => FoldSide::Inside,
                    },
                    thin: a.float("thin", 0.0),
                    min_length: a.float("minLength", 0.0),
                    complete_corners: a.bool("completeCorners"),
                };
                let mut first = !a.bool("add");
                let mut total = 0;
                let mut any = false;
                for name in objects(s, a) {
                    let g = &mut s.graph;
                    let Ok(Some(n)) = g.find(&name) else { continue };
                    let Some(n) = crate::components::as_mesh(g, n) else { continue };
                    let Some(cage) = modifiers::cage(g, n) else { continue };
                    any = true;
                    let edges = fold_edges(&cage, &o);
                    total += edges.len();
                    components::select(g, n, ComponentMode::Edge, &edges, if first { Op::Replace } else { Op::Add });
                    first = false;
                }
                if !any {
                    fail!("selectFoldEdges: select or name a mesh");
                }
                components::set_mode(&mut s.graph, ComponentMode::Edge);
                Ok(json!(total))
            },
        )
        .positional("meshes")
        .flag(df("add", T::Bool, json!(false), "Add to the edge selection instead of replacing it."))
        .flag(df("minAngle", T::Float, json!(30.0), "Smallest fold, degrees between the face normals.").min(0.0).max(180.0))
        .flag(df("maxAngle", T::Float, json!(180.0), "Largest fold, degrees.").min(0.0).max(180.0))
        .flag(df("side", T::String, json!("both"), "both, outside (convex) or inside (concave)."))
        .flag(df("thin", T::Float, json!(0.0), "Skip thickness edges narrower than this, cm.").min(0.0))
        .flag(df("minLength", T::Float, json!(0.0), "Skip edges shorter than this, cm.").min(0.0))
        .flag(df("completeCorners", T::Bool, json!(true), "Add the quad loops joining folds that meet at an angle.")),
    );
}
