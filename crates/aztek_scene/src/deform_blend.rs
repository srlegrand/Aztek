//! blendShape as the reference's: targets with weights at .weight[i] under aliases named after them, in-betweens, live targets that
//! bake when deleted, weights per point on the base and on each target. Also sculptTarget and invertShape.
//!
//! A target holds items keyed as the reference keys them: 5000 + weight * 1000, so full strength at 1 is item 6000. Without a source
//! connected, an item holds its deltas. With one, the source's points are read each time and the deltas held are from the
//! last bake. Origin is local unless -o world is asked for: the reference's documentation calls world the command's default, but
//! its option box sends local and that is what morphing in place needs.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{execute, fail, objects, ArgType, Args, CallSource, CmdResult, Commands};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short, mesh_attr};
use crate::components;
use crate::deform::{self, f, floats_of, input_mesh, json_of, plug, weight_at, Cmd, Place, BASE_COUNT, DATA, ENVELOPE, GEOM, IN, OUT, WEIGHTS};
use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::rig::{world, JsonData};
use crate::session::Session;
use crate::skin::{asked, modes};
use crate::transform::Transform;

pub const WEIGHT: usize = BASE_COUNT;
const BLEND: usize = BASE_COUNT + 1;
pub(crate) const GEOM_TARGET: usize = BASE_COUNT + 2;
const TARGET_MATRIX: usize = BASE_COUNT + 3;
const ORIGIN: usize = BASE_COUNT + 4;
const TOPOLOGY: usize = BASE_COUNT + 5;
/// The target whose weights the paint tool works on.
pub const PAINT_TARGET: usize = BASE_COUNT + 6;
const SCULPT_TARGET: usize = BASE_COUNT + 7;
const SCULPT_STATE: usize = BASE_COUNT + 8;
/// Per-point target weights that can be keyed: element target * TARGET_KEY_STRIDE + point. An element overrides blendData's
/// weight for that point. The reference keeps these under inputTarget[].inputTargetGroup[].targetWeights[], which are not plugs here.
pub(crate) const TARGET_KEYS: usize = BASE_COUNT + 9;
pub(crate) const TARGET_KEY_STRIDE: i32 = 1_000_000;

const HERO: i32 = 6000;

fn item_key(index: i32, iti: i32) -> i32 {
    index * 100_000 + iti
}

fn iti_of(weight: f64) -> i32 {
    (weight * 1000.0).round() as i32 + 5000
}

fn weight_of(iti: i32) -> f64 {
    (iti - 5000) as f64 / 1000.0
}

#[derive(Debug, Clone, Default)]
pub struct BlendItem {
    pub iti: i32,
    /// An in-between that adds to where the full target has got to, rather than being a shape of its own.
    pub relative: bool,
    pub deltas: Vec<(u32, DVec3)>,
}

#[derive(Debug, Clone, Default)]
pub struct BlendTarget {
    pub index: i32,
    pub name: String,
    pub items: Vec<BlendItem>,
    /// Per point; none means 1 everywhere.
    pub weights: Option<Vec<f64>>,
}

/// A blendShape's targets, and the points they were measured from.
#[derive(Debug, Clone, Default)]
pub struct BlendData {
    pub rest: Vec<DVec3>,
    pub targets: Vec<BlendTarget>,
}

impl crate::plug::Data for BlendData {
    fn kind(&self) -> &str {
        "blendData"
    }
    fn to_json(&self) -> Value {
        let targets: Vec<Value> = self
            .targets
            .iter()
            .map(|t| {
                let items: Vec<Value> = t
                    .items
                    .iter()
                    .map(|i| {
                        let flat: Vec<f64> = i.deltas.iter().flat_map(|(p, d)| [*p as f64, d.x, d.y, d.z]).collect();
                        json!({"iti": i.iti, "relative": i.relative, "deltas": flat})
                    })
                    .collect();
                json!({"index": t.index, "name": t.name, "items": items, "weights": t.weights})
            })
            .collect();
        json!({"kind": "blendData", "rest": deform::flat(&self.rest), "targets": targets})
    }
    fn bytes(&self) -> usize {
        self.rest.len() * 24 + self.targets.iter().map(|t| t.items.iter().map(|i| i.deltas.len() * 32).sum::<usize>() + t.weights.as_ref().map_or(0, |w| w.len() * 8)).sum::<usize>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn numbers(v: Option<&Value>) -> Vec<f64> {
    v.and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default()
}

fn blend_from_json(j: &Value) -> BlendData {
    let targets = j.get("targets").and_then(Value::as_array).cloned().unwrap_or_default();
    BlendData {
        rest: deform::unflat(&numbers(j.get("rest"))),
        targets: targets
            .iter()
            .map(|t| BlendTarget {
                index: t.get("index").and_then(Value::as_i64).unwrap_or(0) as i32,
                name: t.get("name").and_then(Value::as_str).unwrap_or("").to_string(),
                weights: t.get("weights").filter(|w| w.is_array()).map(|w| numbers(Some(w))),
                items: t
                    .get("items")
                    .and_then(Value::as_array)
                    .map(|items| {
                        items
                            .iter()
                            .map(|i| BlendItem {
                                iti: i.get("iti").and_then(Value::as_i64).unwrap_or(HERO as i64) as i32,
                                relative: i.get("relative").and_then(Value::as_bool).unwrap_or(false),
                                deltas: numbers(i.get("deltas")).chunks_exact(4).map(|c| (c[0] as u32, DVec3::new(c[1], c[2], c[3]))).collect(),
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

/// A blendShape's data as it stands.
pub fn blend_of(g: &Graph, n: NodeId) -> BlendData {
    g.node(n).and_then(|x| data_of(x.data()).cloned()).unwrap_or_default()
}

fn data_of(d: &NodeData) -> Option<&BlendData> {
    match d.value(BLEND) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<BlendData>(),
        _ => None,
    }
}

pub fn set_blend(g: &mut Graph, n: NodeId, b: BlendData) -> CmdResult<()> {
    g.set_value(plug(n, BLEND), PlugValue::Data(Some(Arc::new(b))))?;
    Ok(())
}

fn is_blend(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("blendShape"))
}

/// What a live source says an item's deltas are.
fn live_deltas(rest: &[DVec3], mesh: &Mesh, world_origin: Option<(DMat4, DMat4)>) -> Vec<(u32, DVec3)> {
    let mut out = Vec::new();
    for p in 0..mesh.point_capacity().min(rest.len() as u32) {
        if !mesh.point_valid(p) {
            continue;
        }
        let at = mesh.position(p);
        let r = rest[p as usize];
        let d = match world_origin {
            Some((target, geom)) => geom.inverse().transform_vector3(target.transform_point3(at) - geom.transform_point3(r)),
            None => at - r,
        };
        if d.length_squared() > 1e-18 {
            out.push((p, d));
        }
    }
    out
}

struct BlendNode;

impl NodeBehavior for BlendNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let env = d.float(ENVELOPE);
        let mut m = (*input).clone();
        let cap = m.point_capacity() as usize;
        let mut total = vec![DVec3::ZERO; cap];
        if let Some(b) = data_of(d) {
            let geom = d.matrix(GEOM);
            let is_world = d.int(ORIGIN) == 1;
            let keyed: std::collections::HashMap<i32, f64> = d.elements(TARGET_KEYS).iter().map(|(i, v)| (*i, v.as_float())).collect();
            let deltas_of = |t: &BlendTarget, it: &BlendItem| -> Vec<(u32, DVec3)> {
                let key = item_key(t.index, it.iti);
                if let Some(PlugValue::Mesh(Some(live))) = d.element(GEOM_TARGET, key) {
                    let origin = is_world.then(|| {
                        let tm = match d.element(TARGET_MATRIX, key) {
                            Some(PlugValue::Matrix(x)) => *x,
                            _ => DMat4::IDENTITY,
                        };
                        (tm, geom)
                    });
                    return live_deltas(&b.rest, live, origin);
                }
                it.deltas.clone()
            };
            for t in &b.targets {
                let w = d.element(WEIGHT, t.index).map_or(0.0, |v| v.as_float());
                if w == 0.0 || t.items.is_empty() {
                    continue;
                }
                let hero = t.items.iter().max_by_key(|i| i.iti).unwrap();
                let add = |it: &BlendItem, k: f64, total: &mut Vec<DVec3>| {
                    if k == 0.0 {
                        return;
                    }
                    let at_tw = |p: u32| {
                        if let Some(v) = keyed.get(&(t.index * TARGET_KEY_STRIDE + p as i32)) {
                            return *v;
                        }
                        t.weights.as_ref().map_or(1.0, |v| v.get(p as usize).copied().unwrap_or(0.0))
                    };
                    for (p, dv) in deltas_of(t, it) {
                        if let Some(x) = total.get_mut(p as usize) {
                            *x += dv * k * at_tw(p);
                        }
                    }
                    if it.relative {
                        for (p, dv) in deltas_of(t, hero) {
                            if let Some(x) = total.get_mut(p as usize) {
                                *x += dv * k * weight_of(it.iti) * at_tw(p);
                            }
                        }
                    }
                };
                // The items in weight order, with nothing at all at weight 0.
                let mut knots: Vec<(f64, Option<&BlendItem>)> = t.items.iter().map(|i| (weight_of(i.iti), Some(i))).collect();
                if !knots.iter().any(|k| k.0 == 0.0) {
                    knots.push((0.0, None));
                }
                knots.sort_by(|a, b| a.0.total_cmp(&b.0));
                // The span holding w, or the nearest span, which carries on in a straight line past the ends.
                let s = knots.windows(2).position(|k| w <= k[1].0).unwrap_or(knots.len().saturating_sub(2));
                if knots.len() < 2 {
                    continue;
                }
                let ((xa, ia), (xb, ib)) = (knots[s], knots[s + 1]);
                let along = (w - xa) / (xb - xa).max(1e-9);
                if let Some(i) = ia {
                    add(i, 1.0 - along, &mut total);
                }
                if let Some(i) = ib {
                    add(i, along, &mut total);
                }
            }
        } else {
            // Scenes from when each target had an attribute of its own and its deltas in deformerData.
            let data = json_of(d, DATA);
            for t in data.get("targets").and_then(Value::as_array).cloned().unwrap_or_default() {
                let Some(i) = t.get("name").and_then(Value::as_str).and_then(|n| d.ty().attr_index(n)) else { continue };
                let w = d.float(i);
                for e in t.get("deltas").and_then(Value::as_array).cloned().unwrap_or_default() {
                    let c = |k: usize| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                    if let Some(x) = total.get_mut(c(0) as usize) {
                        *x += DVec3::new(c(1), c(2), c(3)) * w;
                    }
                }
            }
        }
        let base = floats_of(d, WEIGHTS);
        for p in 0..cap as u32 {
            let dv = total[p as usize];
            if dv != DVec3::ZERO && m.point_valid(p) {
                let k = weight_at(base, p, env);
                let at = m.position(p);
                m.set_position(p, at + dv * k);
            }
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_types(gf: Arc<NodeType>) {
    crate::plug::data::register_kind("blendData", Arc::new(|j| Ok(Arc::new(blend_from_json(j)) as Arc<dyn crate::plug::Data>)));
    let mut weight = float_attr_short("weight", "w", 0.0, "Each target's weight, by its index.", None, None);
    weight.flags |= flags::ARRAY | flags::KEYABLE;
    let mut target_keys = float_attr_short("targetWeightKeys", "twk", 1.0, "Keyed per-point target weights, element target * 1000000 + point; see setKeyframeBlendshapeTargetWts.", None, None);
    target_keys.flags |= flags::ARRAY | flags::KEYABLE | flags::HIDDEN;
    let mut geom_target = mesh_attr("inputGeomTarget", "igt", flags::INPUT | flags::HIDDEN | flags::ARRAY, "Live targets: one element per target item, index * 100000 + item.");
    geom_target.ty = PlugType::Mesh;
    let hidden_int = |name: &str, short: &str, def: i64, doc: &str| AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(def), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: doc.into(), ..Default::default() };
    types::register_type(NodeType {
        name: "blendShape".into(),
        base: Some(gf),
        icon: "shape".into(),
        doc: "Blends the mesh toward target shapes, one weight per target.".into(),
        own_attrs: vec![
            weight,
            deform::data_attr("blendData", "bsd", "Each target's items and weights per point, and the points they were measured from."),
            geom_target,
            crate::rig::matrix_in("inputTargetMatrix", "itm", true, "World matrices of the live targets, as inputGeomTarget."),
            AttrDef { short_name: "o".into(), ..enum_attr("origin", &["Local", "World"], 0, "Measure targets in object space, or in world space.") },
            AttrDef { short_name: "tc".into(), ..bool_attr("topologyCheck", true, "Refuse targets whose points don't match the base's.") },
            hidden_int("paintTargetIndex", "pti", 0, "The target whose weights the paint tool works on."),
            hidden_int("sculptTargetIndex", "sti", -1, "The target being edited through the base, or -1."),
            deform::data_attr("sculptState", "sts", "The base's points and tweaks when a target edit began."),
            target_keys,
        ],
        create: Some(Arc::new(|| Box::new(BlendNode))),
        ..Default::default()
    })
    .expect("blendShape");
}

// ---------------------------------------------------------------------------------------------------------------------
// Aliases

fn free_alias(g: &Graph, n: NodeId, wanted: &str) -> String {
    let taken = |x: &str| g.aliases(n).iter().any(|a| a.0 == x) || g.node(n).is_some_and(|k| k.ty().attr_index(x).is_some());
    if !taken(wanted) {
        return wanted.to_string();
    }
    (1..).map(|k| format!("{wanted}{k}")).find(|x| !taken(x)).unwrap()
}

// ---------------------------------------------------------------------------------------------------------------------
// Targets

/// The blendShape a name stands for, the leader of its group.
pub fn blend_named(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = crate::rig::find(g, name, cmd)?;
    if is_blend(g, id) {
        return Ok(deform::leader_of(g, id));
    }
    if let Some(m) = components::as_mesh(g, id) {
        if let Some(b) = crate::history::history_of(g, m).into_iter().find(|&h| is_blend(g, h)) {
            return Ok(deform::leader_of(g, b));
        }
    }
    fail!("{cmd}: {name} is not a blendShape")
}

/// A mesh's evaluated points as a target gives them.
fn out_mesh(g: &mut Graph, mesh: NodeId) -> Option<Arc<Mesh>> {
    g.evaluate();
    g.value(&plug(mesh, MeshNode::OUT_MESH)).ok().and_then(|v| v.as_mesh().cloned())
}

struct AddTarget {
    index: i32,
    weight: f64,
    in_between: bool,
    relative: bool,
    topology: bool,
}

/// Adds a target or an in-between to a node of the group, live from a shape.
fn add_target(g: &mut Graph, n: NodeId, shape: NodeId, spec: &AddTarget) -> CmdResult<()> {
    let Some(tm) = out_mesh(g, shape) else { fail!("blendShape: {} has no mesh", g.path(shape)) };
    let mut b = blend_of(g, n);
    if spec.topology && tm.point_capacity() as usize != b.rest.len() {
        fail!("blendShape: {} has a different number of points from the base", g.path(shape));
    }
    let xf = crate::dag::transform_of(g, shape);
    let tw = world(g, xf);
    let geom = world(g, deform::mesh_fed_by(g, n).map_or(xf, |m| crate::dag::transform_of(g, m)));
    let is_world = g.node(n).unwrap().data().int(ORIGIN) == 1;
    let deltas = live_deltas(&b.rest, &tm, is_world.then_some((tw, geom)));
    let iti = iti_of(spec.weight);
    let leader = deform::leader_of(g, n);
    let tname = g.node(xf).map(|x| x.name().to_string()).unwrap_or_default();
    let is_new = !b.targets.iter().any(|t| t.index == spec.index);
    if is_new {
        let pos = b.targets.iter().position(|t| t.index > spec.index).unwrap_or(b.targets.len());
        b.targets.insert(pos, BlendTarget { index: spec.index, name: String::new(), items: Vec::new(), weights: None });
    }
    let t = b.targets.iter_mut().find(|t| t.index == spec.index).unwrap();
    if !spec.in_between {
        let old: Vec<i32> = t.items.iter().map(|i| i.iti).collect();
        t.items.clear();
        for o in old {
            let key = item_key(spec.index, o);
            g.disconnect(&PlugRef::element(n, GEOM_TARGET as i32, key));
            g.disconnect(&PlugRef::element(n, TARGET_MATRIX as i32, key));
        }
    }
    t.items.retain(|i| i.iti != iti);
    let hero_of_new = t.items.iter().max_by_key(|i| i.iti).map(|h| h.deltas.clone()).unwrap_or_default();
    let mut deltas = deltas;
    if spec.relative && spec.in_between {
        // A relative in-between holds only what it adds to the full target at this weight.
        let k = spec.weight;
        let mut dense: std::collections::BTreeMap<u32, DVec3> = deltas.iter().copied().collect();
        for (p, d) in hero_of_new {
            *dense.entry(p).or_default() -= d * k;
        }
        deltas = dense.into_iter().filter(|x| x.1.length_squared() > 1e-18).collect();
    }
    t.items.push(BlendItem { iti, relative: spec.relative && spec.in_between, deltas });
    t.items.sort_by_key(|i| i.iti);
    let needs_name = t.name.is_empty();
    let live_ok = !(spec.relative && spec.in_between);
    if needs_name {
        let alias = free_alias(g, leader, &tname);
        t.name = alias.clone();
        g.set_alias(leader, &alias, Some(format!("weight[{}]", spec.index)));
    }
    set_blend(g, n, b)?;
    if live_ok {
        let key = item_key(spec.index, iti);
        g.connect(&plug(shape, MeshNode::OUT_MESH), &PlugRef::element(n, GEOM_TARGET as i32, key), true)?;
        g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(n, TARGET_MATRIX as i32, key), true)?;
    }
    let we = PlugRef::element(leader, WEIGHT as i32, spec.index);
    if is_new && g.node(leader).unwrap().data().element(WEIGHT, spec.index).is_none() {
        g.set_value(we, PlugValue::Float(0.0))?;
    }
    if n != leader && g.source(&PlugRef::element(n, WEIGHT as i32, spec.index)).is_none() {
        g.connect(&we, &PlugRef::element(n, WEIGHT as i32, spec.index), false)?;
    }
    Ok(())
}

fn remove_target(g: &mut Graph, leader: NodeId, index: i32, item: Option<i32>) -> CmdResult<()> {
    for x in deform::group(g, leader) {
        let mut b = blend_of(g, x);
        let Some(t) = b.targets.iter_mut().find(|t| t.index == index) else { continue };
        let doomed: Vec<i32> = t.items.iter().map(|i| i.iti).filter(|&i| item.is_none_or(|k| k == i)).collect();
        for iti in &doomed {
            let key = item_key(index, *iti);
            for at in [GEOM_TARGET, TARGET_MATRIX] {
                g.disconnect(&PlugRef::element(x, at as i32, key));
            }
        }
        t.items.retain(|i| !doomed.contains(&i.iti));
        if t.items.is_empty() {
            b.targets.retain(|t| t.index != index);
            let e = PlugRef::element(x, WEIGHT as i32, index);
            if g.node(x).unwrap().data().element(WEIGHT, index).is_some() {
                g.remove_element(&e)?;
            }
        }
        set_blend(g, x, b)?;
    }
    let path = format!("weight[{index}]");
    if !blend_of(g, leader).targets.iter().any(|t| t.index == index) {
        for (alias, _) in g.aliases(leader).into_iter().filter(|x| x.1 == path) {
            g.set_alias(leader, &alias, None);
        }
    }
    Ok(())
}

/// Every target item a shape feeds keeps what the shape says now: for a shape about to be deleted.
pub(crate) fn bake_from(g: &mut Graph, id: NodeId) -> CmdResult<()> {
    if !components::is_mesh(g, id) {
        return Ok(());
    }
    let feeds: Vec<PlugRef> = g.destinations(&plug(id, MeshNode::OUT_MESH)).into_iter().filter(|d| d.attr == GEOM_TARGET as i32 && is_blend(g, d.node)).collect();
    if feeds.is_empty() {
        return Ok(());
    }
    let Some(tm) = out_mesh(g, id) else { return Ok(()) };
    for d in feeds {
        let (index, iti) = (d.element / 100_000, d.element % 100_000);
        let mut b = blend_of(g, d.node);
        let origin = (g.node(d.node).unwrap().data().int(ORIGIN) == 1).then(|| {
            let xf = crate::dag::transform_of(g, id);
            let tw = world(g, xf);
            let geom = g.node(d.node).unwrap().data().matrix(GEOM);
            (tw, geom)
        });
        let deltas = live_deltas(&b.rest, &tm, origin);
        if let Some(it) = b.targets.iter_mut().find(|t| t.index == index).and_then(|t| t.items.iter_mut().find(|i| i.iti == iti)) {
            it.deltas = deltas;
        }
        set_blend(g, d.node, b)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// The command

/// Flat values of a flag, however it was nested: -t a 0 b 1 -t c 1 d 1, or Python's [(a, 0, b, 1), ...].
fn flat_values(v: Option<&Value>) -> Vec<Value> {
    fn walk(v: &Value, out: &mut Vec<Value>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            x => out.push(x.clone()),
        }
    }
    let mut out = Vec::new();
    if let Some(v) = v {
        walk(v, &mut out);
    }
    out
}

struct TargetSpec {
    base: String,
    index: i32,
    target: String,
    weight: f64,
}

fn target_specs(a: &Args) -> CmdResult<Vec<TargetSpec>> {
    let flat = flat_values(a.get("target").filter(|v| !v.is_boolean()));
    if flat.len() % 4 != 0 {
        fail!("blendShape: -t takes the base, the target's index, the target and its weight");
    }
    flat.chunks(4)
        .map(|c| {
            let (Some(base), Some(target)) = (c[0].as_str(), c[2].as_str()) else { fail!("blendShape: -t takes the base, the target's index, the target and its weight") };
            Ok(TargetSpec { base: base.to_string(), index: c[1].as_i64().unwrap_or(0) as i32, target: target.to_string(), weight: c[3].as_f64().unwrap_or(1.0) })
        })
        .collect()
}

fn mesh_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let id = crate::rig::find(g, name, "blendShape")?;
    match components::as_mesh(g, id) {
        Some(m) => Ok(m),
        None => fail!("blendShape: {name} is not a mesh"),
    }
}

fn apply_weights(g: &mut Graph, leader: NodeId, a: &Args) -> CmdResult<()> {
    if a.get("weight").is_some_and(Value::is_boolean) {
        return Ok(());
    }
    let flat = flat_values(a.get("weight"));
    for c in flat.chunks_exact(2) {
        let (i, w) = (c[0].as_i64().unwrap_or(0) as i32, c[1].as_f64().unwrap_or(0.0));
        g.set_value(PlugRef::element(leader, WEIGHT as i32, i), PlugValue::Float(w))?;
    }
    Ok(())
}

fn add_specs(g: &mut Graph, leader: NodeId, specs: &[TargetSpec], a: &Args) -> CmdResult<()> {
    let relative = a.string("inBetweenType", "absolute") == "relative";
    let topology = a.get("topologyCheck").map_or_else(|| g.node(leader).unwrap().data().bool(TOPOLOGY), |v| v.as_bool().unwrap_or(v.as_i64() != Some(0))) && !a.bool("foreignObject");
    for sp in specs {
        let base = mesh_named(g, &sp.base)?;
        let shape = mesh_named(g, &sp.target)?;
        let n = match deform::node_on(g, leader, base) {
            Some(n) => n,
            None => {
                let x = deform::add_geometry(g, leader, base, &None)?;
                rebind(g, x)?;
                x
            }
        };
        add_target(g, n, shape, &AddTarget { index: sp.index, weight: sp.weight, in_between: a.bool("inBetween"), relative, topology })?;
    }
    Ok(())
}

/// A further base starts with no targets of its own, its weights following the leader's.
pub(crate) fn rebind(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    for at in [GEOM_TARGET, TARGET_MATRIX] {
        for e in g.elements(&plug(n, at)) {
            g.disconnect(&e);
        }
    }
    let leader = deform::leader_of(g, n);
    let Some(input) = input_mesh(g, n) else { return Ok(()) };
    set_blend(g, n, BlendData { rest: input.positions().to_vec(), targets: Vec::new() })?;
    for e in g.elements(&plug(leader, WEIGHT)) {
        g.connect(&e, &PlugRef::element(n, WEIGHT as i32, e.element), true)?;
    }
    Ok(())
}

fn create(s: &mut Session, a: &Args) -> CmdResult {
    let specs = target_specs(a)?;
    let names = if a.bool("ignoreSelected") { a.named() } else { objects(s, a) };
    let g = &mut s.graph;
    // Targets, then the base; or with -t, the bases come from the flag.
    let base = match (specs.first(), names.last()) {
        (Some(sp), _) => sp.base.clone(),
        (None, Some(b)) => b.clone(),
        _ => fail!("blendShape: select the targets, then the base mesh"),
    };
    let base = mesh_named(g, &base)?;
    let positional: Vec<String> = if specs.is_empty() { names[..names.len() - 1].to_vec() } else { names.clone() };
    let place = if a.bool("parallel") { Place::After } else { Place::from_args(a) };
    g.evaluate();
    // A refusal leaves nothing behind.
    let check = a.get("topologyCheck").is_none_or(|v| v.as_bool().unwrap_or(v.as_i64() != Some(0))) && !a.bool("foreignObject");
    if check {
        let Some(own) = crate::modifiers::cage(g, base) else { fail!("blendShape: no base mesh") };
        for t in positional.iter().chain(specs.iter().map(|sp| &sp.target)) {
            let shape = mesh_named(g, t)?;
            if out_mesh(g, shape).is_none_or(|m| m.point_capacity() != own.point_capacity()) {
                fail!("blendShape: {t} has a different number of points from the base");
            }
        }
    }
    let chunk = "Blend Shape";
    g.open_chunk(chunk);
    let r = (|| -> CmdResult<NodeId> {
        let n = deform::insert_at(g, base, "blendShape", &a.string("name", "blendShape1"), place)?;
        match a.string("origin", "local").as_str() {
            "world" => g.set_value(plug(n, ORIGIN), PlugValue::Int(1))?,
            "local" => {}
            o => fail!("blendShape: origin {o} is not world or local"),
        }
        if let Some(v) = a.get("topologyCheck") {
            g.set_value(plug(n, TOPOLOGY), PlugValue::Bool(v.as_bool().unwrap_or(v.as_i64() != Some(0))))?;
        }
        if let Some(e) = a.get("envelope").and_then(Value::as_f64) {
            g.set_value(plug(n, ENVELOPE), PlugValue::Float(e))?;
        }
        // Measured from what comes in; in parallel, from the original, whatever the other deformers did.
        let rest = if a.bool("parallel") {
            let orig = crate::deform_sets::orig_of(g, base);
            orig.and_then(|o| out_mesh(g, o))
        } else {
            input_mesh(g, n)
        };
        let Some(rest) = rest else { fail!("blendShape: no base mesh") };
        set_blend(g, n, BlendData { rest: rest.positions().to_vec(), targets: Vec::new() })?;
        let bname = g.path(base);
        let mut all: Vec<TargetSpec> = Vec::new();
        for (k, t) in positional.iter().enumerate() {
            all.push(TargetSpec { base: bname.clone(), index: k as i32, target: t.clone(), weight: 1.0 });
        }
        all.extend(specs);
        add_specs(g, n, &all, a)?;
        apply_weights(g, n, a)?;
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    Ok(json!([g.path(n)]))
}

fn query(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first() else { fail!("blendShape: name a blendShape to query") };
    let g = &mut s.graph;
    let n = blend_named(g, name, "blendShape")?;
    if asked(a, "weightCount") {
        return Ok(json!(blend_of(g, n).targets.len()));
    }
    if asked(a, "weight") {
        let mut out = Vec::new();
        for e in g.elements(&plug(n, WEIGHT)) {
            out.push(g.value(&e)?.as_float());
        }
        return Ok(json!(out));
    }
    if asked(a, "geometry") {
        return Ok(json!(deform::geometry_names(g, n)));
    }
    if asked(a, "geometryIndices") {
        return Ok(json!((0..deform::geometry_names(g, n).len()).collect::<Vec<_>>()));
    }
    if asked(a, "target") {
        // The target shapes still in the scene, as the reference answers: a baked target has none.
        let mut out = Vec::new();
        for t in blend_of(g, n).targets {
            let hero = t.items.iter().map(|i| i.iti).max().unwrap_or(HERO);
            if let Some(src) = g.source(&PlugRef::element(n, GEOM_TARGET as i32, item_key(t.index, hero))) {
                out.push(g.path(src.node));
            }
        }
        return Ok(json!(out));
    }
    if asked(a, "envelope") {
        return Ok(json!(g.value(&plug(n, ENVELOPE))?.as_float()));
    }
    if asked(a, "origin") {
        return Ok(json!(if g.node(n).unwrap().data().int(ORIGIN) == 1 { "world" } else { "local" }));
    }
    if asked(a, "topologyCheck") {
        return Ok(json!(g.node(n).unwrap().data().bool(TOPOLOGY)));
    }
    fail!("blendShape: query -w, -wc, -t, -g, -gi, -en, -o or -tc")
}

fn edit(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first() else { fail!("blendShape: name a blendShape to edit") };
    let specs = target_specs(a)?;
    let g = &mut s.graph;
    let n = blend_named(g, name, "blendShape")?;
    g.open_chunk("Blend Shape");
    let r = (|| -> CmdResult<()> {
        if a.bool("remove") && !specs.is_empty() {
            for sp in &specs {
                let item = a.bool("inBetween").then(|| iti_of(sp.weight));
                remove_target(g, n, sp.index, item)?;
            }
        } else {
            add_specs(g, n, &specs, a)?;
        }
        if let Some(geo) = a.get("geometry").and_then(Value::as_str) {
            let m = mesh_named(g, geo)?;
            if a.bool("remove") {
                let Some(x) = deform::node_on(g, n, m) else { fail!("blendShape: {geo} is not deformed by {name}") };
                deform::splice(g, x)?;
                if x != n {
                    g.delete_node(x)?;
                }
            } else if deform::node_on(g, n, m).is_none() {
                let x = deform::add_geometry(g, n, m, &None)?;
                rebind(g, x)?;
            }
        }
        // The reference's -rtd zeroes a target's deltas: index, then item, 0 for all of them. A name removes the target.
        let rtd = flat_values(a.get("resetTargetDelta").or(a.get("removeTarget")));
        if let Some(target) = rtd.first().and_then(Value::as_str) {
            let leader_data = blend_of(g, n);
            let Some(t) = leader_data.targets.iter().find(|t| t.name == target) else { fail!("blendShape: no target {target} on {name}") };
            remove_target(g, n, t.index, None)?;
        } else if let Some(i) = rtd.first().and_then(Value::as_i64) {
            let item = rtd.get(1).and_then(Value::as_i64).unwrap_or(0) as i32;
            for x in deform::group(g, n) {
                let mut b = blend_of(g, x);
                if let Some(t) = b.targets.iter_mut().find(|t| t.index == i as i32) {
                    for it in t.items.iter_mut().filter(|it| item == 0 || it.iti == item) {
                        it.deltas.clear();
                        g.disconnect(&PlugRef::element(x, GEOM_TARGET as i32, item_key(i as i32, it.iti)));
                        g.disconnect(&PlugRef::element(x, TARGET_MATRIX as i32, item_key(i as i32, it.iti)));
                    }
                }
                set_blend(g, x, b)?;
            }
        }
        apply_weights(g, n, a)?;
        if let Some(e) = a.get("envelope").and_then(Value::as_f64) {
            g.set_value(plug(n, ENVELOPE), PlugValue::Float(e))?;
        }
        if let Some(v) = a.get("topologyCheck") {
            g.set_value(plug(n, TOPOLOGY), PlugValue::Bool(v.as_bool().unwrap_or(v.as_i64() != Some(0))))?;
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(json!([g.path(n)]))
}

// ---------------------------------------------------------------------------------------------------------------------
// sculptTarget

fn tweak_nodes_below(g: &Graph, n: NodeId, mesh: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    for h in crate::history::history_of(g, mesh) {
        if h == n {
            break;
        }
        if g.node(h).is_some_and(|x| x.ty().name == crate::history::TWEAK_TYPE) {
            out.push(h);
        }
    }
    out
}

fn sculpt_target(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first() else { fail!("sculptTarget: name a blendShape") };
    let g = &mut s.graph;
    let n = blend_named(g, name, "sculptTarget")?;
    if a.bool("query") {
        return Ok(json!(g.node(n).unwrap().data().int(SCULPT_TARGET)));
    }
    let Some(mesh) = deform::mesh_fed_by(g, n) else { fail!("sculptTarget: {name} has no geometry") };
    let wanted = a.int("target", -1) as i32;
    if a.bool("regenerate") {
        // Back to a shape in the scene, live again.
        let b = blend_of(g, n);
        let Some(t) = b.targets.iter().find(|t| t.index == wanted) else { fail!("sculptTarget: no target {wanted} on {name}") };
        let iti = a.get("inbetweenWeight").and_then(Value::as_f64).map_or_else(|| t.items.iter().map(|i| i.iti).max().unwrap_or(HERO), iti_of);
        let Some(it) = t.items.iter().find(|i| i.iti == iti) else { fail!("sculptTarget: target {wanted} has no item at that weight") };
        let tname = t.name.clone();
        let deltas = it.deltas.clone();
        let rest = b.rest.clone();
        let made = execute(s, "duplicate", &json!({"args": [s.graph.path(crate::dag::transform_of(&s.graph, mesh))], "name": tname}), CallSource::Script)?;
        let new_xf = made.as_array().and_then(|v| v.first()).and_then(Value::as_str).unwrap_or("").to_string();
        let g = &mut s.graph;
        let shape = mesh_named(g, &new_xf)?;
        let Some(base) = input_mesh(g, n) else { fail!("sculptTarget: no base mesh") };
        let mut m = (*base).clone();
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                m.set_position(p, rest.get(p as usize).copied().unwrap_or(m.position(p)));
            }
        }
        for (p, d) in deltas {
            if m.point_valid(p) {
                let at = m.position(p);
                m.set_position(p, at + d);
            }
        }
        crate::history::delete_history(g, shape)?;
        g.set_value(plug(shape, MeshNode::IN_MESH), PlugValue::Mesh(Some(Arc::new(m))))?;
        let key = item_key(wanted, iti);
        g.connect(&plug(shape, MeshNode::OUT_MESH), &PlugRef::element(n, GEOM_TARGET as i32, key), true)?;
        let xf = crate::dag::transform_of(g, shape);
        g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(n, TARGET_MATRIX as i32, key), true)?;
        return Ok(json!([new_xf]));
    }
    let now = g.node(n).unwrap().data().int(SCULPT_TARGET) as i32;
    g.open_chunk("Sculpt Target");
    let r = (|| -> CmdResult<()> {
        if now >= 0 {
            // Leaving: what was moved through the base goes into the target, and the tweaks that moved it go.
            let state = json_of(g.node(n).unwrap().data(), SCULPT_STATE);
            let before = deform::unflat(&numbers(state.get("points")));
            if let Some(after) = crate::modifiers::cage(g, mesh) {
                let mut b = blend_of(g, n);
                let w = g.node(n).unwrap().data().element(WEIGHT, now).map_or(1.0, |v| v.as_float());
                let k = if w.abs() > 1e-3 { 1.0 / w } else { 1.0 };
                if let Some(t) = b.targets.iter_mut().find(|t| t.index == now) {
                    let wanted_iti = state.get("item").and_then(Value::as_i64).map(|x| x as i32);
                    let it = wanted_iti.and_then(|want| t.items.iter().position(|i| i.iti == want)).or_else(|| t.items.iter().enumerate().max_by_key(|x| x.1.iti).map(|x| x.0));
                    if let Some(i) = it {
                        let iti = t.items[i].iti;
                        let mut dense: std::collections::BTreeMap<u32, DVec3> = t.items[i].deltas.iter().copied().collect();
                        for p in 0..after.point_capacity().min(before.len() as u32) {
                            if after.point_valid(p) {
                                let change = after.position(p) - before[p as usize];
                                if change.length_squared() > 1e-18 {
                                    *dense.entry(p).or_default() += change * k;
                                }
                            }
                        }
                        t.items[i].deltas = dense.into_iter().filter(|x| x.1.length_squared() > 1e-18).collect();
                        // The target's own shape no longer says what it holds.
                        let key = item_key(now, iti);
                        g.disconnect(&PlugRef::element(n, GEOM_TARGET as i32, key));
                        g.disconnect(&PlugRef::element(n, TARGET_MATRIX as i32, key));
                    }
                }
                set_blend(g, n, b)?;
            }
            for tw in tweak_nodes_below(g, n, mesh) {
                let saved = state.get("tweaks").and_then(|x| x.get(g.path(tw))).cloned();
                crate::history::set_tweaks_json(g, tw, saved.as_ref())?;
            }
            g.set_value(plug(n, SCULPT_TARGET), PlugValue::Int(-1))?;
            g.set_value(plug(n, SCULPT_STATE), PlugValue::Data(None))?;
        }
        if wanted >= 0 {
            if !blend_of(g, n).targets.iter().any(|t| t.index == wanted) {
                fail!("sculptTarget: no target {wanted} on {name}");
            }
            let Some(at) = crate::modifiers::cage(g, mesh) else { fail!("sculptTarget: no geometry") };
            let mut tweaks = serde_json::Map::new();
            for tw in tweak_nodes_below(g, n, mesh) {
                tweaks.insert(g.path(tw), crate::history::tweaks_json(g, tw));
            }
            let mut st = json!({"points": deform::flat(at.positions()), "tweaks": tweaks});
            if let Some(ib) = a.get("inbetweenWeight").and_then(Value::as_f64) {
                st["item"] = json!(iti_of(ib));
            }
            g.set_value(plug(n, SCULPT_STATE), PlugValue::Data(Some(Arc::new(JsonData(st)))))?;
            g.set_value(plug(n, SCULPT_TARGET), PlugValue::Int(wanted as i64))?;
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(json!(wanted))
}

// ---------------------------------------------------------------------------------------------------------------------
// invertShape

/// The mesh's output with its original's points moved by offset: how the deformers answer. Nothing of it stays.
fn probe(g: &mut Graph, mesh: NodeId, orig: NodeId, base: &Arc<Mesh>, offset: DVec3) -> Vec<DVec3> {
    let mut moved = (**base).clone();
    for p in 0..moved.point_capacity() {
        if moved.point_valid(p) {
            let at = moved.position(p);
            moved.set_position(p, at + offset);
        }
    }
    let in_plug = plug(orig, MeshNode::IN_MESH);
    let src = g.source(&in_plug);
    let stored = g.stored_value(&in_plug).clone();
    g.without_undo(|g| {
        g.disconnect(&in_plug);
        let _ = g.set_value(in_plug, PlugValue::Mesh(Some(Arc::new(moved))));
        g.evaluate();
        let out = crate::modifiers::cage(g, mesh).map(|m| m.positions().to_vec()).unwrap_or_default();
        let _ = g.set_value(in_plug, stored);
        if let Some(s) = src {
            let _ = g.connect(&s, &in_plug, true);
        }
        g.evaluate();
        out
    })
}

/// The reference's invertShape: the shape that, as a target in front of the deformers, gives the corrected mesh at this pose. Each
/// point's response to the deformers comes from nudging the original along each axis.
fn invert_shape(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let [base_name, corrected] = &names[..] else { fail!("invertShape: name the deformed mesh, then the corrected mesh") };
    let g = &mut s.graph;
    let (base, fixed) = (mesh_named(g, base_name)?, mesh_named(g, corrected)?);
    let Some(orig) = crate::deform_sets::orig_of(g, base) else { fail!("invertShape: {base_name} has no deformers") };
    let (Some(rest), Some(goal), Some(posed)) = (out_mesh(g, orig), out_mesh(g, fixed), crate::modifiers::cage(g, base)) else { fail!("invertShape: no geometry") };
    if goal.point_capacity() != rest.point_capacity() {
        fail!("invertShape: {corrected} has a different number of points from {base_name}");
    }
    let eps = 0.01;
    let axes = [probe(g, base, orig, &rest, DVec3::X * eps), probe(g, base, orig, &rest, DVec3::Y * eps), probe(g, base, orig, &rest, DVec3::Z * eps)];
    let mut out = (*rest).clone();
    for p in 0..out.point_capacity() {
        if !out.point_valid(p) {
            continue;
        }
        let i = p as usize;
        let now = posed.position(p);
        let jac = DMat3::from_cols((axes[0][i] - now) / eps, (axes[1][i] - now) / eps, (axes[2][i] - now) / eps);
        let change = goal.position(p) - now;
        let solved = if jac.determinant().abs() > 1e-9 { jac.inverse() * change } else { change };
        out.set_position(p, rest.position(p) + solved);
    }
    let name = format!("{}_inverted", g.node(crate::dag::transform_of(g, fixed)).map(|n| n.name().to_string()).unwrap_or_default());
    let made = execute(s, "duplicate", &json!({"args": [corrected], "name": name}), CallSource::Script)?;
    let new_xf = made.as_array().and_then(|v| v.first()).and_then(Value::as_str).unwrap_or("").to_string();
    let g = &mut s.graph;
    let shape = mesh_named(g, &new_xf)?;
    crate::history::delete_history(g, shape)?;
    g.set_value(plug(shape, MeshNode::IN_MESH), PlugValue::Mesh(Some(Arc::new(out))))?;
    Ok(json!(new_xf))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        modes(deform::common_flags(Cmd::new(
            "blendShape",
            "Blend shapes: blendShape target... base. Each target gets an element of .weight under an alias named after it: setAttr \
             blendShape1.pSphere2 0.5. -t base index target weight adds targets, with -ib as in-betweens at that weight. -e -rm -t removes, \
             -e -rtd index item clears deltas. Query -w, -wc, -t, -g, -en, -o, -tc.",
            |s, a| {
                if a.bool("query") {
                    return query(s, a);
                }
                if a.bool("edit") {
                    return edit(s, a);
                }
                create(s, a)
            },
        )))
        .positional("targets, then the base; the blendShape with -e and -q")
        .flag(f("weight", "w", ArgType::Any, "index weight, repeatable. Query: all the weights.").multi(2))
        .flag(f("target", "t", ArgType::Any, "base index target weight, repeatable. Query: the target shapes.").multi(4))
        .flag(f("inBetween", "ib", ArgType::Bool, "-t adds an in-between at its weight rather than replacing the target."))
        .flag(f("inBetweenType", "ibt", ArgType::String, "absolute (the default), or relative to the full target."))
        .flag(f("topologyCheck", "tc", ArgType::Any, "Refuse targets with a different number of points (on)."))
        .flag(f("origin", "o", ArgType::String, "local (the default) or world."))
        .flag(f("parallel", "par", ArgType::Bool, "Work alongside the deformers already there: measured from the original, added to their result."))
        .flag(f("remove", "rm", ArgType::Bool, "With -e -t: remove the target. With -e -g: remove the geometry."))
        .flag(f("removeTarget", "rmt", ArgType::Any, "With -e: a target's name, to remove it."))
        .flag(f("resetTargetDelta", "rtd", ArgType::Any, "With -e: target index and item (0 for all), to clear their deltas; a name removes the target.").multi(2))
        .flag(f("geometry", "g", ArgType::Any, "With -e: geometry to add or, with -rm, remove. Query: the deformed shapes."))
        .flag(f("geometryIndices", "gi", ArgType::Bool, "Query: the indices of the deformed shapes."))
        .flag(f("weightCount", "wc", ArgType::Any, "Query: how many targets."))
        .flag(f("foreignObject", "fg", ArgType::Bool, "Targets need not match the base: points are taken by number as far as they go."))
        .flag(f("ignoreSelected", "ie", ArgType::Bool, "Use only the objects named, not the selection.")),
    );
    c.register(
        modes(Cmd::new(
            "sculptTarget",
            "Edit a blend shape target through the base: sculptTarget -e -t 0 blendShape1 starts, and edits of the base go into target 0 when \
             -t -1 ends it. -e -r -t 0 makes the target's shape again, live, and returns it. -q gives the target being edited.",
            sculpt_target,
        ))
        .positional("a blendShape")
        .flag(f("target", "t", ArgType::Int, "The target to edit, -1 for none."))
        .flag(f("inbetweenWeight", "ibw", ArgType::Float, "Which in-between of the target."))
        .flag(f("regenerate", "r", ArgType::Bool, "Make the target's shape again.")),
    );
    c.register(
        Cmd::new(
            "invertShape",
            "invertShape deformed corrected: a new mesh, the shape which as a target in front of the deformers gives the corrected mesh at \
             this pose. Returns its transform.",
            invert_shape,
        )
        .positional("the deformed mesh, then the corrected mesh"),
    );
}
