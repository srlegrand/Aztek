//! Smooth skinning as the reference does it: the skinCluster node (linear and dual quaternion), binding, skinCluster's edit and
//! query modes, skinPercent, copySkinWeights (mirror and copy), the bind pose (dagPose) and the paint brush behind
//! artAttrSkinPaintCtx.

use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;

use aztek_geo::bvh::MeshBvh;
use rayon::prelude::*;
use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DQuat, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{float_attr_short, mesh_attr};
use crate::components::{self, ComponentMode};
use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{data_attr, find, is_joint, mat_from, mat_json, matrix_in, plug, world, JsonData};
use crate::session::Session;
use crate::transform::Transform;

pub const SKIN_TYPE: &str = "skinCluster";
const SK_IN: usize = 0;
const SK_OUT: usize = 1;
const SK_MATRIX: usize = 2;
const SK_BIND: usize = 3;
const SK_ENVELOPE: usize = 4;
const SK_METHOD: usize = 5;
pub(crate) const SK_MAX_INF: usize = 6;
const SK_MAINTAIN: usize = 7;
const SK_DROPOFF: usize = 8;
const SK_NORMALIZE: usize = 9;
const SK_HEAT: usize = 10;
pub(crate) const SK_BLEND: usize = 11;
const SK_MOVE: usize = 12;
const SK_BIND_METHOD: usize = 13;

/// What a skinCluster stores: per influence (by matrix[] element) its bind pre-matrix and lock, per point its weights.
#[derive(Debug, Clone, Default)]
pub struct SkinBind {
    pub bind_pre: Vec<DMat4>,
    pub geom: DMat4,
    /// Per point: (influence element, weight), no zero entries.
    pub weights: Vec<Vec<(u32, f64)>>,
    pub locks: Vec<bool>,
    /// Edits leave the other influences alone: normalizeWeights is not interactive. Read from the node, not stored.
    pub raw_edit: bool,
}

impl SkinBind {
    fn from_json(v: &Value) -> SkinBind {
        let weights = v
            .get("weights")
            .and_then(Value::as_array)
            .map(|pts| {
                pts.iter()
                    .map(|p| p.as_array().map(|a| a.chunks(2).filter_map(|c| Some((c.first()?.as_u64()? as u32, c.get(1)?.as_f64()?))).collect()).unwrap_or_default())
                    .collect()
            })
            .unwrap_or_default();
        SkinBind {
            bind_pre: v.get("bindPre").and_then(Value::as_array).map(|a| a.iter().map(mat_from).collect()).unwrap_or_default(),
            geom: mat_from(v.get("geom").unwrap_or(&Value::Null)),
            weights,
            locks: v.get("locks").and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_bool().unwrap_or(false)).collect()).unwrap_or_default(),
            raw_edit: false,
        }
    }

    fn locked(&self, inf: u32) -> bool {
        self.locks.get(inf as usize).copied().unwrap_or(false)
    }
}

impl crate::plug::Data for SkinBind {
    fn kind(&self) -> &str {
        "skinBind"
    }
    fn to_json(&self) -> Value {
        let w: Vec<Value> = self.weights.iter().map(|w| json!(w.iter().flat_map(|&(i, x)| [json!(i), json!(x)]).collect::<Vec<_>>())).collect();
        json!({"kind": "skinBind", "bindPre": self.bind_pre.iter().map(mat_json).collect::<Vec<_>>(), "geom": mat_json(&self.geom), "weights": w, "locks": self.locks})
    }
    fn bytes(&self) -> usize {
        self.bind_pre.len() * 128 + self.weights.iter().map(|w| w.len() * 16 + 24).sum::<usize>()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn bind_from_value(v: &PlugValue) -> Option<Arc<SkinBind>> {
    match v {
        PlugValue::Data(Some(x)) => {
            if let Some(b) = x.as_any().downcast_ref::<SkinBind>() {
                return Some(Arc::new(b.clone()));
            }
            // Scenes saved before the typed payload.
            x.as_any().downcast_ref::<JsonData>().map(|j| Arc::new(SkinBind::from_json(&j.0)))
        }
        _ => None,
    }
}

pub fn bind_of(g: &Graph, sk: NodeId) -> Option<Arc<SkinBind>> {
    let n = g.node(sk)?;
    let mut b = bind_from_value(n.data().value(SK_BIND))?;
    if let Some(x) = Arc::get_mut(&mut b) {
        x.raw_edit = n.data().int(SK_NORMALIZE) != 1;
    }
    Some(b)
}

fn set_bind(g: &mut Graph, sk: NodeId, b: SkinBind) -> CmdResult<()> {
    g.set_value(plug(sk, SK_BIND), PlugValue::Data(Some(Arc::new(b))))?;
    Ok(())
}

#[derive(Default)]
pub struct SkinNode {
    flat: Option<FlatBind>,
}

/// Points handed to one thread at a time: small enough to balance, large enough not to waste time handing them out.
const POINTS_PER_TASK: usize = 1024;

/// The bind as it's computed from: each point's influences side by side in memory, so a mesh of any size stays cheap. Kept
/// while the bind it came from is the same one, which holds it so its address can't come round again.
struct FlatBind {
    source: crate::plug::DataPtr,
    bind_pre: Vec<DMat4>,
    geom: DMat4,
    offsets: Vec<u32>,
    influence: Vec<u32>,
    weight: Vec<f64>,
}

impl FlatBind {
    fn new(source: crate::plug::DataPtr) -> Option<FlatBind> {
        let any = source.as_any();
        let parsed;
        let bind: &SkinBind = if let Some(b) = any.downcast_ref::<SkinBind>() {
            b
        } else {
            // Scenes saved before the typed payload.
            parsed = SkinBind::from_json(&any.downcast_ref::<JsonData>()?.0);
            &parsed
        };
        let total: usize = bind.weights.iter().map(Vec::len).sum();
        let (mut offsets, mut influence, mut weight) = (Vec::with_capacity(bind.weights.len() + 1), Vec::with_capacity(total), Vec::with_capacity(total));
        offsets.push(0);
        for w in &bind.weights {
            for &(i, v) in w {
                influence.push(i);
                weight.push(v);
            }
            offsets.push(influence.len() as u32);
        }
        let (bind_pre, geom) = (bind.bind_pre.clone(), bind.geom);
        Some(FlatBind { source, bind_pre, geom, offsets, influence, weight })
    }
}

/// A rigid transform as a dual quaternion: (real, dual).
fn dual_quat(m: &DMat4) -> (DQuat, DQuat) {
    let m3 = DMat3::from_mat4(*m);
    let r = DQuat::from_mat3(&DMat3::from_cols(m3.x_axis.normalize_or_zero(), m3.y_axis.normalize_or_zero(), m3.z_axis.normalize_or_zero())).normalize();
    let t = m.w_axis.truncate();
    let d = DQuat::from_xyzw(t.x, t.y, t.z, 0.0) * r;
    (r, DQuat::from_xyzw(d.x * 0.5, d.y * 0.5, d.z * 0.5, d.w * 0.5))
}

fn dq_transform(real: DQuat, dual: DQuat, p: DVec3) -> DVec3 {
    let len = real.length();
    if len < 1e-12 {
        return p;
    }
    let (r, d) = (real / len, dual / len);
    let t = d * r.conjugate();
    r * p + DVec3::new(t.x, t.y, t.z) * 2.0
}

impl NodeBehavior for SkinNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(SK_IN).cloned() else {
            d.set_output(SK_OUT, PlugValue::Mesh(None));
            return;
        };
        // Move joints mode: the joints go where they like and the mesh stays.
        if d.bool(SK_MOVE) {
            d.set_output(SK_OUT, PlugValue::Mesh(Some(input)));
            return;
        }
        let PlugValue::Data(Some(held)) = d.value(SK_BIND) else {
            d.set_output(SK_OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        if !self.flat.as_ref().is_some_and(|f| Arc::ptr_eq(&f.source, held)) {
            let source = held.clone();
            self.flat = FlatBind::new(source);
        }
        let Some(flat) = self.flat.as_ref() else {
            d.set_output(SK_OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let env = d.float(SK_ENVELOPE);
        let method = d.int(SK_METHOD);
        let blend = crate::deform::floats_of(d, SK_BLEND);
        let blend: &[f64] = blend.as_ref().map_or(&[], |b| b.0.as_slice());
        // One per influence: where the bind pose goes now, from the point in world space; None for one that isn't connected.
        let mats: Vec<Option<DMat4>> = flat
            .bind_pre
            .iter()
            .enumerate()
            .map(|(i, pre)| match d.element(SK_MATRIX, i as i32) {
                Some(PlugValue::Matrix(m)) => Some(*m * *pre),
                _ => None,
            })
            .collect();
        let duals: Vec<Option<(DQuat, DQuat)>> = if method != 0 { mats.iter().map(|m| m.as_ref().map(dual_quat)).collect() } else { Vec::new() };
        let (geom, gi) = (flat.geom, flat.geom.inverse());
        let rest = input.positions();
        let mut out = rest.to_vec();
        let (offsets, influence, weight) = (&flat.offsets, &flat.influence, &flat.weight);
        // Points in chunks across every core; each writes only its own.
        out.par_chunks_mut(POINTS_PER_TASK).enumerate().for_each(|(chunk, pts)| {
            for (k, pos) in pts.iter_mut().enumerate() {
                let p = chunk * POINTS_PER_TASK + k;
                if p + 1 >= offsets.len() || !input.point_valid(p as aztek_geo::Index) {
                    continue;
                }
                let (from, to) = (offsets[p] as usize, offsets[p + 1] as usize);
                if from == to {
                    continue;
                }
                let wp = geom.transform_point3(*pos);
                let mut dq_pos = None;
                if method != 0 {
                    let mut real = DQuat::from_xyzw(0.0, 0.0, 0.0, 0.0);
                    let mut dual = real;
                    let mut tw = 0.0;
                    let mut pivot: Option<DQuat> = None;
                    for e in from..to {
                        let Some(Some((r, q))) = duals.get(influence[e] as usize) else { continue };
                        let wt = weight[e];
                        let first = *pivot.get_or_insert(*r);
                        let s = if first.dot(*r) < 0.0 { -wt } else { wt };
                        real = real + *r * s;
                        dual = dual + *q * s;
                        tw += wt;
                    }
                    if tw <= 0.0 {
                        continue;
                    }
                    dq_pos = Some(dq_transform(real, dual, wp));
                }
                let mut linear_pos = None;
                if method != 1 {
                    let mut acc = DVec3::ZERO;
                    let mut tw = 0.0;
                    for e in from..to {
                        if let Some(Some(mm)) = mats.get(influence[e] as usize) {
                            acc += mm.transform_point3(wp) * weight[e];
                            tw += weight[e];
                        }
                    }
                    if tw <= 0.0 {
                        continue;
                    }
                    linear_pos = Some(acc / tw);
                }
                // Weight blended: each point between the two by its blend weight, 0 linear and 1 dual quaternion.
                let world_pos = match (linear_pos, dq_pos) {
                    (Some(l), Some(q)) => l.lerp(q, blend.get(p).copied().unwrap_or(0.0)),
                    (Some(l), None) => l,
                    (None, Some(q)) => q,
                    (None, None) => continue,
                };
                let skinned = gi.transform_point3(world_pos);
                *pos += (skinned - *pos) * env;
            }
        });
        d.set_output(SK_OUT, PlugValue::Mesh(Some(Arc::new(input.with_positions(out)))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct PoseNode;

impl NodeBehavior for PoseNode {
    fn compute(&mut self, _ctx: &EvalContext, _d: &mut NodeData) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn int_attr(name: &str, short: &str, default: i64, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(default),
        flags: flags::INPUT | flags::STORABLE,
        doc: doc.into(),
        ..Default::default()
    }
}

fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        crate::plug::data::register_kind("skinBind", Arc::new(|j| Ok(Arc::new(SkinBind::from_json(j)) as Arc<dyn crate::plug::Data>)));
        types::register_type(NodeType {
            name: SKIN_TYPE.into(),
            icon: "bone".into(),
            doc: "Smooth skin: the mesh follows its influence joints by weight.".into(),
            own_attrs: vec![
                mesh_attr("inputPolymesh", "ip", flags::INPUT | flags::HIDDEN, "The mesh in bind pose."),
                mesh_attr("output", "out", flags::OUTPUT | flags::HIDDEN, "The skinned mesh."),
                matrix_in("matrix", "ma", true, "Each influence's world matrix."),
                data_attr("bindData", "bd", "Bind pre-matrices, the mesh's bind matrix, locks and the weights."),
                float_attr_short("envelope", "en", 1.0, "How much of the skinning applies.", Some(0.0), Some(1.0)),
                int_attr("skinningMethod", "sm", 0, "0 classic linear, 1 dual quaternion, 2 blended by blendWeights."),
                int_attr("maxInfluences", "mi", 5, "The most influences per point when maintainMaxInfluences is on."),
                AttrDef { flags: flags::INPUT | flags::STORABLE, ..crate::attr_defs::bool_attr("maintainMaxInfluences", false, "Hold points to maxInfluences while weights are edited.") },
                float_attr_short("dropoffRate", "dr", 4.0, "How fast influence falls off with distance at bind.", Some(0.1), None),
                int_attr("normalizeWeights", "nw", 1, "0 none, 1 interactive, 2 post."),
                float_attr_short("heatmapFalloff", "hmf", 0.68, "Heat map bind: 1 keeps the heat as solved, less tightens it.", Some(0.0), Some(1.0)),
                data_attr("blendWeights", "bw", "Each point between linear (0) and dual quaternion (1), with skinningMethod 2."),
                AttrDef { flags: flags::INPUT | flags::STORABLE, ..crate::attr_defs::bool_attr("moveJointsMode", false, "Joints move without moving the mesh.") },
                int_attr("bindMethod", "bm", 0, "How the skin was bound: 0 closest distance, 1 closest in hierarchy, 2 heat map, 3 geodesic voxel."),
            ],
            create: Some(Arc::new(|| Box::new(SkinNode::default()))),
            ..Default::default()
        })
        .expect(SKIN_TYPE);
        types::register_type(NodeType {
            name: "dagPose".into(),
            icon: "bone".into(),
            doc: "A saved pose of a skeleton (bindPose1).".into(),
            own_attrs: vec![
                matrix_in("worldMatrix", "wm", true, "Each member's world matrix, which keeps the link through renames."),
                data_attr("poseData", "pd", "Each member's local transform when the pose was saved."),
                AttrDef { flags: flags::INPUT | flags::STORABLE, ..crate::attr_defs::bool_attr("bindPose", false, "This pose is a bind pose.") },
            ],
            create: Some(Arc::new(|| Box::new(PoseNode))),
            ..Default::default()
        })
        .expect("dagPose");
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// Weight arithmetic

/// Rescales the unlocked weights so the point sums to 1; locked ones stay. A point with nothing unlocked stays too.
pub fn normalize_point(pw: &mut Vec<(u32, f64)>, b: &SkinBind) {
    let locked: f64 = pw.iter().filter(|x| b.locked(x.0)).map(|x| x.1).sum();
    let free: f64 = pw.iter().filter(|x| !b.locked(x.0)).map(|x| x.1).sum();
    if free <= 1e-12 {
        return;
    }
    let k = (1.0 - locked).max(0.0) / free;
    for x in pw.iter_mut() {
        if !b.locked(x.0) {
            x.1 *= k;
        }
    }
    pw.retain(|x| x.1 > 1e-9);
}

/// Sets one influence's weight on a point, the rest of the unlocked influences making up the difference; if none of them
/// holds any weight, `fallback` takes it. Locked influences never change, and a locked influence can't be painted.
pub fn set_point_weight(pw: &mut Vec<(u32, f64)>, inf: u32, value: f64, b: &SkinBind, fallback: Option<u32>) {
    if b.locked(inf) {
        return;
    }
    if b.raw_edit {
        pw.retain(|x| x.0 != inf);
        if value > 1e-9 {
            pw.push((inf, value.max(0.0)));
            pw.sort_by_key(|x| x.0);
        }
        return;
    }
    let locked: f64 = pw.iter().filter(|x| x.0 != inf && b.locked(x.0)).map(|x| x.1).sum();
    let avail = (1.0 - locked).max(0.0);
    let value = value.clamp(0.0, avail);
    let others: f64 = pw.iter().filter(|x| x.0 != inf && !b.locked(x.0)).map(|x| x.1).sum();
    let rest = avail - value;
    let mut next: Vec<(u32, f64)> = Vec::with_capacity(pw.len() + 1);
    let mut value = value;
    for &(i, x) in pw.iter() {
        if i == inf {
            continue;
        }
        if b.locked(i) {
            next.push((i, x));
        } else if others > 1e-12 {
            next.push((i, x * rest / others));
        }
    }
    if others <= 1e-12 && rest > 1e-12 {
        match fallback {
            Some(fb) if fb != inf && !b.locked(fb) => next.push((fb, rest)),
            // Nowhere for the weight to go: the influence keeps it.
            _ => value = avail,
        }
    }
    if value > 1e-9 {
        next.push((inf, value));
    }
    next.retain(|x| x.1 > 1e-9);
    next.sort_by_key(|x| x.0);
    *pw = next;
}

/// The largest weights only, renormalized.
fn hold_max(pw: &mut Vec<(u32, f64)>, max: usize, b: &SkinBind) {
    if pw.len() <= max {
        return;
    }
    pw.sort_by(|x, y| y.1.total_cmp(&x.1));
    pw.truncate(max.max(1));
    pw.sort_by_key(|x| x.0);
    normalize_point(pw, b);
}

fn weight_at(pw: &[(u32, f64)], inf: u32) -> f64 {
    pw.iter().find(|x| x.0 == inf).map(|x| x.1).unwrap_or(0.0)
}

fn neighbours(m: &Mesh, p: u32) -> Vec<u32> {
    let mut out = Vec::new();
    m.for_each_point_edge(p, |e| out.push(m.edge_other_point(e, p)));
    out
}

/// A point's weights averaged with its neighbours', blended in by `amount`; locked influences stay as they are.
fn smoothed_point(old: &[Vec<(u32, f64)>], m: &Mesh, p: u32, amount: f64, b: &SkinBind) -> Vec<(u32, f64)> {
    let own = &old[p as usize];
    let around = neighbours(m, p);
    if around.is_empty() {
        return own.clone();
    }
    let mut avg: HashMap<u32, f64> = HashMap::new();
    for &n in &around {
        for &(i, x) in old.get(n as usize).map(|v| v.as_slice()).unwrap_or(&[]) {
            *avg.entry(i).or_default() += x / around.len() as f64;
        }
    }
    let mut keys: Vec<u32> = own.iter().map(|x| x.0).chain(avg.keys().copied()).collect();
    keys.sort_unstable();
    keys.dedup();
    let mut next = Vec::with_capacity(keys.len());
    for i in keys {
        let a = weight_at(own, i);
        let v = if b.locked(i) { a } else { a + (avg.get(&i).copied().unwrap_or(0.0) - a) * amount };
        if v > 1e-9 {
            next.push((i, v));
        }
    }
    normalize_point(&mut next, b);
    next
}

// ---------------------------------------------------------------------------------------------------------------------
// The paint brush

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PaintOp {
    Replace,
    Add,
    Scale,
    Smooth,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Stamp {
    Gaussian,
    Poly,
    Solid,
    Square,
}

/// The tool's settings, the reference's defaults.
#[derive(Clone, Debug)]
pub struct PaintCtx {
    pub radius: f64,
    pub opacity: f64,
    pub value: f64,
    pub op: PaintOp,
    pub stamp: Stamp,
    pub influence: String,
    pub color_feedback: bool,
}

impl Default for PaintCtx {
    fn default() -> Self {
        PaintCtx { radius: 1.0, opacity: 1.0, value: 1.0, op: PaintOp::Replace, stamp: Stamp::Gaussian, influence: String::new(), color_feedback: true }
    }
}

pub(crate) fn falloff(stamp: Stamp, t: f64) -> f64 {
    if t >= 1.0 {
        return 0.0;
    }
    match stamp {
        Stamp::Gaussian => (-(t * t) * 4.0).exp(),
        Stamp::Poly => (1.0 - t * t).powi(2),
        Stamp::Solid | Stamp::Square => 1.0,
    }
}

fn bind_position(b: &SkinBind, inf: u32) -> DVec3 {
    b.bind_pre.get(inf as usize).map(|m| m.inverse().w_axis.truncate()).unwrap_or(DVec3::ZERO)
}

/// The unlocked influence other than `inf` bound nearest a world point: where weight goes when nothing else holds any.
fn nearest_other(b: &SkinBind, inf: u32, at: DVec3) -> Option<u32> {
    (0..b.bind_pre.len() as u32)
        .filter(|&i| i != inf && !b.locked(i))
        .min_by(|&x, &y| (bind_position(b, x) - at).length_squared().total_cmp(&(bind_position(b, y) - at).length_squared()))
}

/// One dab of the brush: the points of `m` (posed, in object space) within reach of a world centre change. A dab from the
/// stroke's start repeats on `b`, so the opacity builds up as the brush passes. `normal` skips points facing away.
pub fn stamp(b: &mut SkinBind, m: &Mesh, world: &DMat4, center: DVec3, normal: Option<DVec3>, inf: u32, ctx: &PaintCtx, invert: bool, smooth: bool) -> usize {
    let mut hit = Vec::new();
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) || p as usize >= b.weights.len() {
            continue;
        }
        let wp = world.transform_point3(m.position(p));
        let t = (wp - center).length() / ctx.radius.max(1e-9);
        if t >= 1.0 {
            continue;
        }
        if let Some(n) = normal {
            let mut pn = DVec3::ZERO;
            for f in m.point_faces(p) {
                pn += m.face_area_vector(f);
            }
            let pn = world.transform_vector3(pn).normalize_or_zero();
            if pn.dot(n) < 0.0 {
                continue;
            }
        }
        hit.push((p, falloff(ctx.stamp, t) * ctx.opacity, wp));
    }
    let op = if smooth { PaintOp::Smooth } else { ctx.op };
    if op == PaintOp::Smooth {
        let old = b.weights.clone();
        for &(p, f, _) in &hit {
            let next = smoothed_point(&old, m, p, f, b);
            b.weights[p as usize] = next;
        }
        return hit.len();
    }
    let value = ctx.value;
    for &(p, f, wp) in &hit {
        let now = weight_at(&b.weights[p as usize], inf);
        let next = match op {
            PaintOp::Replace => now + ((if invert { 1.0 - value } else { value }) - now) * f,
            PaintOp::Add => now + (if invert { -value } else { value }) * f,
            PaintOp::Scale => now * (1.0 + ((if invert { 1.0 / value.max(1e-6) } else { value }) - 1.0) * f),
            PaintOp::Smooth => now,
        };
        let fb = nearest_other(b, inf, wp);
        let mut pw = std::mem::take(&mut b.weights[p as usize]);
        set_point_weight(&mut pw, inf, next, b, fb);
        b.weights[p as usize] = pw;
    }
    hit.len()
}

/// Sets a stroke's result as one undoable change.
pub fn commit_paint(g: &mut Graph, sk: NodeId, b: SkinBind) -> CmdResult<()> {
    set_bind(g, sk, b)
}

/// The influence the tool is painting: the one set in the context, or else the first on the skin.
pub fn paint_influence(s: &Session, sk: NodeId) -> Option<u32> {
    let g = &s.graph;
    let list = influences(g, sk);
    if !s.paint_ctx.influence.is_empty() {
        if let Ok(Some(j)) = g.find(&s.paint_ctx.influence) {
            if let Some(x) = list.iter().find(|x| x.1 == j) {
                return Some(x.0 as u32);
            }
        }
    }
    list.first().map(|x| x.0 as u32)
}

/// The weight of one influence per point, for the colour feedback.
pub fn point_weights(b: &SkinBind, inf: u32) -> Vec<f64> {
    b.weights.iter().map(|w| weight_at(w, inf)).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Queries used by the rest of the scene

pub fn skin_of(g: &Graph, mesh: NodeId) -> Option<NodeId> {
    crate::history::history_of(g, mesh).into_iter().find(|&h| g.node(h).is_some_and(|n| n.ty().name == SKIN_TYPE))
}

/// (matrix element, influence), by element.
pub fn influences(g: &Graph, skin: NodeId) -> Vec<(i32, NodeId)> {
    let mut v: Vec<(i32, NodeId)> = g
        .connections()
        .into_iter()
        .filter(|(d, s)| d.node == skin && d.attr == SK_MATRIX as i32 && d.element >= 0 && s.attr == Transform::WORLD_MATRIX as i32)
        .map(|(d, s)| (d.element, s.node))
        .collect();
    v.sort();
    v
}

/// The skinCluster a name stands for: itself, or the one on a mesh, a transform or a component's mesh.
pub fn skin_named(g: &Graph, name: &str) -> Option<NodeId> {
    let base = name.split('.').next().unwrap_or(name);
    let id = g.find(base).ok().flatten()?;
    if g.node(id).is_some_and(|n| n.ty().name == SKIN_TYPE) {
        return Some(id);
    }
    components::as_mesh(g, id).and_then(|m| skin_of(g, m))
}

/// The mesh shape a skinCluster deforms.
pub fn skin_geometry(g: &Graph, sk: NodeId) -> Option<NodeId> {
    let mut out = vec![plug(sk, SK_OUT)];
    while let Some(p) = out.pop() {
        for d in g.destinations(&p) {
            let n = g.node(d.node)?;
            if n.ty().is_a("mesh") {
                return Some(d.node);
            }
            if let Some(i) = n.ty().attr_index("output") {
                out.push(plug(d.node, i));
            }
        }
    }
    None
}

/// Points named: pCube1.vtx[0:3] (or a whole mesh, or any component converted), per mesh.
pub fn points_named(g: &mut Graph, names: &[String]) -> Vec<(NodeId, Vec<u32>)> {
    let mut out: Vec<(NodeId, Vec<u32>)> = Vec::new();
    for n in names {
        let (obj, comp) = match n.split_once('.') {
            Some((o, c)) => (o, Some(c)),
            None => (n.as_str(), None),
        };
        let Ok(Some(id)) = g.find(obj) else { continue };
        let Some(mesh) = components::as_mesh(g, id) else { continue };
        let ids: Vec<u32> = match comp {
            None => {
                let Some(m) = components::mesh_of(g, mesh) else { continue };
                (0..m.point_capacity()).filter(|&p| m.point_valid(p)).collect()
            }
            Some(c) => {
                let Some(m) = components::mesh_of(g, mesh) else { continue };
                let (kind, inner) = match c.split_once('[') {
                    Some((k, r)) => (k, r.trim_end_matches(']')),
                    None => continue,
                };
                let mode = match kind {
                    "vtx" | "pt" | "cv" => ComponentMode::Vertex,
                    "e" => ComponentMode::Edge,
                    "f" => ComponentMode::Face,
                    _ => continue,
                };
                let ids = expand_range(inner, match mode {
                    ComponentMode::Vertex => m.point_capacity(),
                    ComponentMode::Edge => m.edge_capacity(),
                    _ => m.face_capacity(),
                });
                if mode == ComponentMode::Vertex {
                    ids
                } else {
                    components::convert(&m, mode, &ids, ComponentMode::Vertex, false).to_vec()
                }
            }
        };
        match out.iter_mut().find(|x| x.0 == mesh) {
            Some(e) => e.1.extend(ids),
            None => out.push((mesh, ids)),
        }
    }
    out
}

fn expand_range(s: &str, cap: u32) -> Vec<u32> {
    let mut v = Vec::new();
    for part in s.split(',') {
        if part == "*" {
            v.extend(0..cap);
        } else if let Some((a, b)) = part.split_once(':') {
            let (a, b): (u32, u32) = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
            v.extend(a..=b);
        } else if let Ok(i) = part.parse() {
            v.push(i);
        }
    }
    v
}

/// The selected points of a mesh: its selected components as points, or all of them when none are.
pub(crate) fn selected_points(g: &mut Graph, mesh: NodeId) -> Vec<u32> {
    let Some(m) = components::mesh_of(g, mesh) else { return Vec::new() };
    let mode = components::mode(g);
    if mode != ComponentMode::Object {
        let ids = components::selected(g, mesh, mode);
        if !ids.is_empty() {
            return components::convert(&m, mode, ids, ComponentMode::Vertex, false).to_vec();
        }
    }
    (0..m.point_capacity()).filter(|&p| m.point_valid(p)).collect()
}

fn seg_dist(p: DVec3, a: DVec3, b: DVec3) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    (p - (a + ab * t)).length()
}

/// Bind weights. Closest distance (method 0): a point goes to its nearest bones, falling off with distance^dropoff.
/// Closest in hierarchy (method 1): only the nearest bone and those joined to it take part, so an arm against a body
/// stays with the arm. Heat map and geodesic voxel binds (2, 3) use closest in hierarchy.
fn bind_weights(m: &Mesh, geom: &DMat4, bones: &[(DVec3, DVec3)], joined: &[Vec<usize>], method: i64, max_inf: usize, dropoff: f64) -> Vec<Vec<(u32, f64)>> {
    let mut out = vec![Vec::new(); m.point_capacity() as usize];
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let wp = geom.transform_point3(m.position(p));
        let mut d: Vec<(usize, f64)> = bones.iter().enumerate().map(|(i, (a, b))| (i, seg_dist(wp, *a, *b).max(1e-6))).collect();
        d.sort_by(|a, b| a.1.total_cmp(&b.1));
        if method >= 1 {
            let first = d[0].0;
            d.retain(|x| x.0 == first || joined[first].contains(&x.0));
        }
        d.truncate(max_inf.max(1));
        let nearest = d[0].1;
        let mut w: Vec<(u32, f64)> = d.iter().map(|&(i, x)| (i as u32, (nearest / x).powf(dropoff))).collect();
        w.retain(|x| x.1 > 1e-3);
        let s: f64 = w.iter().map(|x| x.1).sum();
        for x in w.iter_mut() {
            x.1 /= s;
        }
        w.sort_by_key(|x| x.0);
        out[p as usize] = w;
    }
    out
}

fn bone_of(g: &mut Graph, j: NodeId) -> (DVec3, DVec3) {
    let a0 = world(g, j).w_axis.truncate();
    let kid = g.children(j).iter().copied().find(|&k| is_joint(g, k));
    let b0 = kid.map(|k| world(g, k).w_axis.truncate()).unwrap_or(a0);
    (a0, b0)
}

// ---------------------------------------------------------------------------------------------------------------------
// The bind pose

fn local_of(g: &Graph, j: NodeId) -> Value {
    let v = |a: usize| g.node(j).map(|n| n.data().vec3(a)).unwrap_or(DVec3::ZERO);
    let (t, r, s) = (v(Transform::TRANSLATE), v(Transform::ROTATE), v(Transform::SCALE));
    json!({"t": [t.x, t.y, t.z], "r": [r.x, r.y, r.z], "s": [s.x, s.y, s.z]})
}

fn pose_node(g: &mut Graph, name: &str, members: &[NodeId], bind: bool) -> CmdResult<NodeId> {
    let t = types::find("dagPose").unwrap();
    let n = g.create_node(&t, name, NO_NODE, true)?;
    let data: Vec<Value> = members.iter().map(|&j| local_of(g, j)).collect();
    crate::rig::set_json(g, n, 1, json!(data))?;
    if bind {
        g.set_value(plug(n, 2), PlugValue::Bool(true))?;
    }
    for (i, &j) in members.iter().enumerate() {
        g.connect(&plug(j, Transform::WORLD_MATRIX), &PlugRef::element(n, 0, i as i32), false)?;
    }
    Ok(n)
}

fn pose_members(g: &Graph, pose: NodeId) -> Vec<(i32, NodeId)> {
    let mut v: Vec<(i32, NodeId)> = g.connections().into_iter().filter(|(d, _)| d.node == pose && d.attr == 0 && d.element >= 0).map(|(d, s)| (d.element, s.node)).collect();
    v.sort();
    v
}

fn restore_pose(g: &mut Graph, pose: NodeId, only: Option<&[NodeId]>) -> CmdResult<Vec<NodeId>> {
    let data = g.node(pose).map(|n| crate::rig::json_of(n.data(), 1)).unwrap_or(Value::Null);
    let mut done = Vec::new();
    for (i, j) in pose_members(g, pose) {
        if only.is_some_and(|o| !o.contains(&j)) {
            continue;
        }
        let Some(e) = data.get(i as usize) else { continue };
        let vec = |k: &str, d: f64| {
            let f = |x: usize| e.get(k).and_then(|a| a.get(x)).and_then(Value::as_f64).unwrap_or(d);
            DVec3::new(f(0), f(1), f(2))
        };
        for (attr, v) in [(Transform::TRANSLATE, vec("t", 0.0)), (Transform::ROTATE, vec("r", 0.0)), (Transform::SCALE, vec("s", 1.0))] {
            let p = plug(j, attr);
            // Driven channels (constraints, IK, keys) stay with their drivers, as in the reference.
            if g.source(&p).is_none() {
                g.set_value(p, PlugValue::Vec3(v))?;
            }
        }
        done.push(j);
    }
    Ok(done)
}

fn bind_pose_of(g: &Graph, joint: NodeId) -> Option<NodeId> {
    g.nodes().into_iter().find(|&n| g.node(n).is_some_and(|x| x.ty().name == "dagPose" && x.data().bool(2)) && pose_members(g, n).iter().any(|x| x.1 == joint))
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

/// Whether query mode asked about a flag.
pub(crate) fn asked(a: &Args, name: &str) -> bool {
    a.bool(name) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == name))
}

pub(crate) fn modes(c: Cmd) -> Cmd {
    c.flag(f("edit", "e", ArgType::Bool, "Edit mode."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode."))
}

/// Flat name/value pairs from `[j, w, j, w]` or Python's `[(j, w), (j, w)]` or `(j, w)`.
fn pairs(v: &Value) -> Vec<(String, f64)> {
    let mut flat = Vec::new();
    fn walk(v: &Value, out: &mut Vec<Value>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| walk(x, out)),
            x => out.push(x.clone()),
        }
    }
    walk(v, &mut flat);
    flat.chunks(2).filter_map(|c| Some((c.first()?.as_str()?.to_string(), c.get(1)?.as_f64()?))).collect()
}

fn names_list(v: &Value) -> Vec<String> {
    match v {
        Value::String(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn skeleton_of(g: &Graph, joint: NodeId) -> Vec<NodeId> {
    let mut root = joint;
    while g.parent(root) != NO_NODE && is_joint(g, g.parent(root)) {
        root = g.parent(root);
    }
    let mut all = vec![root];
    all.extend(g.descendants(root).into_iter().filter(|&d| is_joint(g, d)));
    all
}

fn hold_all(g: &Graph, sk: NodeId, b: &mut SkinBind) {
    let n = g.node(sk);
    if n.is_some_and(|n| n.data().bool(SK_MAINTAIN)) {
        let max = n.map(|n| n.data().int(SK_MAX_INF)).unwrap_or(5).max(1) as usize;
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for w in b.weights.iter_mut() {
            hold_max(w, max, &locks);
        }
    }
}

fn bind(s: &mut Session, a: &Args, names: &[String]) -> CmdResult {
    let g = &mut s.graph;
    let mut joints = Vec::new();
    let mut mesh = None;
    for n in names {
        let id = find(g, n, "skinCluster")?;
        if g.node(id).is_some_and(|x| x.ty().is_a("transform")) && !components::as_mesh(g, id).is_some() {
            joints.push(id);
        } else if let Some(m) = components::as_mesh(g, id) {
            mesh = Some(m);
        }
    }
    let Some(mesh) = mesh else { fail!("skinCluster: name or select joints and a mesh") };
    if joints.is_empty() {
        fail!("skinCluster: name or select the joints to bind to");
    }
    if skin_of(g, mesh).is_some() {
        fail!("skinCluster: {} is already skinned", components::display_path(g, mesh));
    }
    // The whole skeleton unless -tsb.
    if !a.bool("toSelectedBones") {
        if let Some(&j0) = joints.iter().find(|&&j| is_joint(g, j)) {
            for j in skeleton_of(g, j0) {
                if !joints.contains(&j) {
                    joints.push(j);
                }
            }
        }
    }
    g.evaluate();
    let Some(cage) = crate::modifiers::cage(g, mesh) else { fail!("skinCluster: not a mesh") };
    let geom = world(g, crate::dag::transform_of(g, mesh));
    let jw: Vec<DMat4> = joints.iter().map(|&j| world(g, j)).collect();
    let bones: Vec<(DVec3, DVec3)> = joints.iter().map(|&j| bone_of(g, j)).collect();
    let joined: Vec<Vec<usize>> = joints
        .iter()
        .map(|&j| {
            joints
                .iter()
                .enumerate()
                .filter(|&(_, &o)| o != j && (g.parent(o) == j || g.parent(j) == o))
                .map(|(i, _)| i)
                .collect()
        })
        .collect();
    let max_inf = a.int("maximumInfluences", 5).max(1);
    let dropoff = a.float("dropoffRate", 4.0);
    let method = a.int("bindMethod", 0);
    let heat_falloff = a.float("heatmapFalloff", 0.68);
    let weights = if method == 2 { crate::skin_extra::heat_weights(&cage, &geom, &bones, max_inf as usize, heat_falloff) } else { bind_weights(&cage, &geom, &bones, &joined, method, max_inf as usize, dropoff) };
    let data = SkinBind { bind_pre: jw.iter().map(|m| m.inverse()).collect(), geom, weights, locks: vec![false; joints.len()], raw_edit: false };
    g.open_chunk("Bind Skin");
    let r = (|| -> CmdResult<NodeId> {
        let t = types::find(SKIN_TYPE).unwrap();
        let (base, src) = crate::history::upstream(g, mesh)?;
        let src = crate::deform_sets::orig_source(g, mesh, src)?;
        let sk = g.create_node(&t, &a.string("name", "skinCluster1"), NO_NODE, true)?;
        set_bind(g, sk, data)?;
        g.set_value(plug(sk, SK_HEAT), PlugValue::Float(heat_falloff))?;
        g.set_value(plug(sk, SK_BIND_METHOD), PlugValue::Int(method))?;
        g.set_value(plug(sk, SK_MAX_INF), PlugValue::Int(max_inf))?;
        g.set_value(plug(sk, SK_DROPOFF), PlugValue::Float(dropoff))?;
        g.set_value(plug(sk, SK_METHOD), PlugValue::Int(a.int("skinMethod", 0)))?;
        g.set_value(plug(sk, SK_NORMALIZE), PlugValue::Int(a.int("normalizeWeights", 1)))?;
        if a.bool("obeyMaxInfluences") {
            g.set_value(plug(sk, SK_MAINTAIN), PlugValue::Bool(true))?;
        }
        for (i, &j) in joints.iter().enumerate() {
            g.connect(&plug(j, Transform::WORLD_MATRIX), &PlugRef::element(sk, SK_MATRIX as i32, i as i32), false)?;
        }
        g.disconnect(&base);
        g.connect(&src, &plug(sk, SK_IN), false)?;
        g.connect(&plug(sk, SK_OUT), &base, false)?;
        // Joints the skeleton's bind pose doesn't yet know go in a bind pose of their own.
        let skeleton: Vec<NodeId> = joints.iter().copied().filter(|&j| is_joint(g, j)).collect();
        if !skeleton.is_empty() && !skeleton.iter().all(|&j| bind_pose_of(g, j).is_some()) {
            pose_node(g, "bindPose1", &skeleton, true)?;
        }
        Ok(sk)
    })();
    g.close_chunk();
    let sk = r?;
    Ok(json!([g.path(sk)]))
}

/// Works a skinCluster's weights out again with a bind method, keeping its influences and bind matrices.
pub fn rebind_weights(g: &mut Graph, sk: NodeId, method: i64, max_inf: usize, dropoff: f64) -> CmdResult<()> {
    let Some(mut bind) = bind_of(g, sk).map(|b| (*b).clone()) else { fail!("{}: no bind data", g.path(sk)) };
    g.evaluate();
    let input = g.value(&plug(sk, SK_IN))?;
    let Some(cage) = input.as_mesh().cloned() else { fail!("{}: no input mesh", g.path(sk)) };
    let joints: Vec<NodeId> = influences(g, sk).into_iter().map(|(_, j)| j).collect();
    let bones: Vec<(DVec3, DVec3)> = joints.iter().map(|&j| bone_of(g, j)).collect();
    let joined: Vec<Vec<usize>> = joints.iter().map(|&j| joints.iter().enumerate().filter(|&(_, &o)| o != j && (g.parent(o) == j || g.parent(j) == o)).map(|(i, _)| i).collect()).collect();
    bind.weights = bind_weights(&cage, &bind.geom, &bones, &joined, method, max_inf, dropoff);
    set_bind(g, sk, bind)
}

fn unbind(g: &mut Graph, mesh: NodeId, keep_history: bool) -> CmdResult<()> {
    let Some(sk) = skin_of(g, mesh) else { return Ok(()) };
    if keep_history {
        // The skinning stops but the node stays, as Detach Skin with history kept does.
        g.set_value(plug(sk, SK_ENVELOPE), PlugValue::Float(0.0))?;
        return Ok(());
    }
    let src = g.source(&plug(sk, SK_IN));
    let dsts = g.destinations(&plug(sk, SK_OUT));
    g.delete_node(sk)?;
    if let Some(src) = src {
        for d in dsts {
            g.connect(&src, &d, true)?;
        }
    }
    Ok(())
}

fn skin_cluster_query(s: &mut Session, a: &Args, names: &[String]) -> CmdResult {
    let g = &mut s.graph;
    let n = match names.first() {
        Some(n) => n.clone(),
        None => match g.selection().last() {
            Some(&id) => g.path(id),
            None => fail!("skinCluster: name a skinCluster or a skinned mesh"),
        },
    };
    let Some(sk) = skin_named(g, &n) else { fail!("skinCluster: {n} has no skin") };
    let attr_i = |g: &Graph, at: usize| g.node(sk).map(|x| x.data().int(at)).unwrap_or(0);
    if asked(a, "geometry") {
        return Ok(json!(skin_geometry(g, sk).map(|m| vec![g.path(m)]).unwrap_or_default()));
    }
    if asked(a, "maximumInfluences") {
        return Ok(json!(attr_i(g, SK_MAX_INF)));
    }
    if asked(a, "obeyMaxInfluences") {
        return Ok(json!(g.node(sk).is_some_and(|x| x.data().bool(SK_MAINTAIN))));
    }
    if asked(a, "skinMethod") {
        return Ok(json!(attr_i(g, SK_METHOD)));
    }
    if asked(a, "normalizeWeights") {
        return Ok(json!(attr_i(g, SK_NORMALIZE)));
    }
    if asked(a, "bindMethod") {
        return Ok(json!(attr_i(g, SK_BIND_METHOD)));
    }
    if asked(a, "heatmapFalloff") {
        return Ok(json!(g.node(sk).map(|x| x.data().float(SK_HEAT)).unwrap_or(0.68)));
    }
    if asked(a, "moveJointsMode") {
        return Ok(json!(g.node(sk).is_some_and(|x| x.data().bool(SK_MOVE))));
    }
    if asked(a, "dropoffRate") {
        return Ok(json!(g.node(sk).map(|x| x.data().float(SK_DROPOFF)).unwrap_or(4.0)));
    }
    let list = influences(g, sk);
    let bind = bind_of(g, sk).unwrap_or_default();
    if asked(a, "lockWeights") {
        let name = a.get("influence").and_then(Value::as_str).map(str::to_string);
        let Some(name) = name else { fail!("skinCluster: -q -lw needs -inf joint") };
        let j = find(g, &name, "skinCluster")?;
        let e = list.iter().find(|x| x.1 == j);
        return Ok(json!(e.is_some_and(|e| bind.locked(e.0 as u32))));
    }
    if asked(a, "weightedInfluence") {
        let used: Vec<String> = list
            .iter()
            .filter(|(i, _)| bind.weights.iter().any(|w| weight_at(w, *i as u32) > 0.0))
            .map(|&(_, j)| g.path(j))
            .collect();
        return Ok(json!(used));
    }
    Ok(json!(list.iter().map(|x| g.path(x.1)).collect::<Vec<_>>()))
}

fn skin_cluster_edit(s: &mut Session, a: &Args, names: &[String]) -> CmdResult {
    if a.bool("unbind") || a.bool("unbindKeepHistory") {
        let g = &mut s.graph;
        let list: Vec<String> = if names.is_empty() { g.selection().iter().map(|&i| g.path(i)).collect() } else { names.to_vec() };
        for n in &list {
            let id = find(g, n, "skinCluster")?;
            let mesh = if g.node(id).is_some_and(|x| x.ty().name == SKIN_TYPE) { skin_geometry(g, id) } else { components::as_mesh(g, id) };
            if let Some(m) = mesh {
                unbind(g, m, a.bool("unbindKeepHistory"))?;
            }
        }
        return Ok(Value::Null);
    }
    let g = &mut s.graph;
    let named = match names.first() {
        Some(n) => n.clone(),
        None => match g.selection().last() {
            Some(&id) => g.path(id),
            None => fail!("skinCluster: name a skinCluster or a skinned mesh"),
        },
    };
    let Some(sk) = skin_named(g, &named) else { fail!("skinCluster: {named} has no skin") };
    let mut b = (*bind_of(g, sk).unwrap_or_default()).clone();
    g.evaluate();
    let mut changed = false;
    if a.has("moveJointsMode") {
        let on = a.bool("moveJointsMode");
        let was = g.node(sk).is_some_and(|x| x.data().bool(SK_MOVE));
        g.set_value(plug(sk, SK_MOVE), PlugValue::Bool(on))?;
        // Leaving it: where the joints now are is the bind pose, so the mesh stays where it is.
        if was && !on {
            for (i, j) in influences(g, sk) {
                b.bind_pre[i as usize] = world(g, j).inverse();
            }
            changed = true;
        }
    }
    if a.has("heatmapFalloff") {
        g.set_value(plug(sk, SK_HEAT), PlugValue::Float(a.float("heatmapFalloff", 0.68)))?;
    }
    for (flag, at) in [("skinMethod", SK_METHOD), ("maximumInfluences", SK_MAX_INF), ("normalizeWeights", SK_NORMALIZE)] {
        if a.has(flag) {
            g.set_value(plug(sk, at), PlugValue::Int(a.int(flag, 0)))?;
        }
    }
    if a.has("obeyMaxInfluences") {
        g.set_value(plug(sk, SK_MAINTAIN), PlugValue::Bool(a.bool("obeyMaxInfluences")))?;
        changed = true;
    }
    if let Some(list) = a.get("addInfluence") {
        for n in names_list(list) {
            let j = find(g, &n, "skinCluster")?;
            let cur = influences(g, sk);
            if cur.iter().any(|x| x.1 == j) {
                continue;
            }
            let i = cur.iter().map(|x| x.0 + 1).max().unwrap_or(0).max(b.bind_pre.len() as i32);
            if b.bind_pre.len() <= i as usize {
                b.bind_pre.resize(i as usize + 1, DMat4::IDENTITY);
                b.locks.resize(i as usize + 1, false);
            }
            b.bind_pre[i as usize] = world(g, j).inverse();
            g.connect(&plug(j, Transform::WORLD_MATRIX), &PlugRef::element(sk, SK_MATRIX as i32, i), false)?;
            let wt = a.float("weight", 0.0);
            if a.bool("useGeometry") {
                let Some(inf_mesh) = components::as_mesh(g, j) else { fail!("skinCluster: {n} is not a mesh, for -ug") };
                let (Some(own), Some(other)) = (skin_geometry(g, sk).and_then(|m| crate::modifiers::cage(g, m)), crate::modifiers::cage(g, inf_mesh)) else { fail!("skinCluster: no geometry") };
                let xf = crate::dag::transform_of(g, inf_mesh);
                let wm = world(g, xf);
                let rate = a.float("dropoffRate", 4.0);
                crate::skin_extra::geometry_influence_weights(&mut b, &own, i as u32, &other, &wm, rate);
            } else if wt > 0.0 {
                for w in b.weights.iter_mut() {
                    w.push((i as u32, wt));
                    normalize_point(w, &SkinBind { locks: b.locks.clone(), ..Default::default() });
                }
            }
            changed = true;
        }
    }
    let mut removal = Vec::new();
    if let Some(list) = a.get("removeInfluence") {
        for n in names_list(list) {
            removal.push(find(g, &n, "skinCluster")?);
        }
    }
    if a.bool("removeUnusedInfluence") {
        for (i, j) in influences(g, sk) {
            if !b.weights.iter().any(|w| weight_at(w, i as u32) > 0.0) {
                removal.push(j);
            }
        }
    }
    for j in removal {
        let cur = influences(g, sk);
        let Some(&(i, _)) = cur.iter().find(|x| x.1 == j) else { continue };
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for w in b.weights.iter_mut() {
            if weight_at(w, i as u32) > 0.0 {
                w.retain(|x| x.0 != i as u32);
                normalize_point(w, &locks);
            }
        }
        g.disconnect(&PlugRef::element(sk, SK_MATRIX as i32, i));
        changed = true;
    }
    if a.has("lockWeights") {
        let name = a.get("influence").and_then(Value::as_str).map(str::to_string);
        let cur = influences(g, sk);
        let targets: Vec<i32> = match name {
            Some(n) => {
                let j = find(g, &n, "skinCluster")?;
                cur.iter().filter(|x| x.1 == j).map(|x| x.0).collect()
            }
            None => cur.iter().map(|x| x.0).collect(),
        };
        b.locks.resize(b.bind_pre.len(), false);
        for i in targets {
            b.locks[i as usize] = a.bool("lockWeights");
        }
        changed = true;
    }
    if a.bool("forceNormalizeWeights") {
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        let mut ids: Vec<u32> = Vec::new();
        if let Some(m) = skin_geometry(g, sk) {
            ids = selected_points(g, m);
        }
        for p in ids {
            if let Some(w) = b.weights.get_mut(p as usize) {
                normalize_point(w, &locks);
            }
        }
        changed = true;
    }
    if a.has("smoothWeights") {
        let Some(mesh) = skin_geometry(g, sk) else { fail!("skinCluster: no geometry") };
        let Some(m) = crate::modifiers::cage(g, mesh) else { fail!("skinCluster: no geometry") };
        let amount = a.float("smoothWeights", 0.5).clamp(0.0, 1.0);
        let iters = a.int("smoothWeightsMaxIterations", 2).max(1);
        let ids = selected_points(g, mesh);
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for _ in 0..iters {
            let old = b.weights.clone();
            for &p in &ids {
                if (p as usize) < old.len() {
                    b.weights[p as usize] = smoothed_point(&old, &m, p, amount, &locks);
                }
            }
        }
        changed = true;
    }
    if a.bool("recacheBindMatrices") {
        for (i, j) in influences(g, sk) {
            b.bind_pre[i as usize] = world(g, j).inverse();
        }
        changed = true;
    }
    if changed {
        hold_all(g, sk, &mut b);
        set_bind(g, sk, b)?;
    }
    Ok(Value::Null)
}

fn skin_percent(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let g = &mut s.graph;
    let Some(skin_name) = names.first() else { fail!("skinPercent: name the skinCluster") };
    let Some(sk) = skin_named(g, skin_name) else { fail!("skinPercent: no skinCluster {skin_name}") };
    let list = influences(g, sk);
    let targets = if names.len() > 1 {
        points_named(g, &names[1..])
    } else {
        match skin_geometry(g, sk) {
            Some(m) => {
                let ids = selected_points(g, m);
                vec![(m, ids)]
            }
            None => Vec::new(),
        }
    };
    let pts: Vec<u32> = targets.iter().flat_map(|x| x.1.iter().copied()).collect();
    let mut b = (*bind_of(g, sk).unwrap_or_default()).clone();
    let inf_named = |g: &Graph, n: &str| -> CmdResult<u32> {
        let j = find(g, n, "skinPercent")?;
        match list.iter().find(|x| x.1 == j) {
            Some(e) => Ok(e.0 as u32),
            None => fail!("skinPercent: {n} is not an influence of {skin_name}"),
        }
    };
    if a.bool("query") {
        let ib = a.float("ignoreBelow", 0.0);
        let wants_value = asked(a, "value");
        let t = a.get("transform");
        if let (Some(tn), true) = (t.and_then(Value::as_str), true) {
            // One influence's weight: averaged over the points, as the reference.
            let i = inf_named(g, tn)?;
            let total: f64 = pts.iter().map(|&p| b.weights.get(p as usize).map(|w| weight_at(w, i)).unwrap_or(0.0)).sum();
            return Ok(json!(total / pts.len().max(1) as f64));
        }
        if pts.is_empty() {
            fail!("skinPercent: name a vertex");
        }
        let avg = |i: i32| -> f64 { pts.iter().map(|&p| b.weights.get(p as usize).map(|w| weight_at(w, i as u32)).unwrap_or(0.0)).sum::<f64>() / pts.len() as f64 };
        let kept: Vec<&(i32, NodeId)> = list.iter().filter(|x| ib <= 0.0 || avg(x.0) > ib).collect();
        if wants_value {
            return Ok(json!(kept.iter().map(|x| avg(x.0)).collect::<Vec<_>>()));
        }
        return Ok(json!(kept.iter().map(|x| g.path(x.1)).collect::<Vec<_>>()));
    }
    if let Some(v) = a.get("dqBlendWeight").and_then(Value::as_f64) {
        let size = b.weights.len();
        let mut bw = g.node(sk).and_then(|n| crate::deform::floats_of(n.data(), SK_BLEND)).map(|x| x.0.clone()).unwrap_or_else(|| vec![0.0; size]);
        bw.resize(size, 0.0);
        for &p in &pts {
            if let Some(x) = bw.get_mut(p as usize) {
                *x = v.clamp(0.0, 1.0);
            }
        }
        crate::deform::set_floats(g, sk, SK_BLEND, bw)?;
        if !["transformValue", "transformMoveWeights", "pruneWeights", "normalize", "resetToDefault"].iter().any(|x| a.has(x)) {
            return Ok(Value::Null);
        }
    }
    let mut touched = false;
    if a.bool("resetToDefault") {
        let Some(mesh) = skin_geometry(g, sk) else { fail!("skinPercent: no geometry") };
        let Some(m) = crate::modifiers::cage(g, mesh) else { fail!("skinPercent: no geometry") };
        let bones: Vec<(DVec3, DVec3)> = list.iter().map(|x| x.1).collect::<Vec<_>>().into_iter().map(|j| bone_of(g, j)).collect();
        let joined = vec![Vec::new(); bones.len()];
        let mi = g.node(sk).map(|n| n.data().int(SK_MAX_INF)).unwrap_or(5).max(1) as usize;
        let dr = g.node(sk).map(|n| n.data().float(SK_DROPOFF)).unwrap_or(4.0);
        let fresh = bind_weights(&m, &b.geom, &bones, &joined, 0, mi, dr);
        for &p in &pts {
            if let (Some(w), Some(n)) = (b.weights.get_mut(p as usize), fresh.get(p as usize)) {
                *w = n.iter().map(|&(i, x)| (list[i as usize].0 as u32, x)).collect();
            }
        }
        touched = true;
    }
    if let Some(tv) = a.get("transformValue") {
        let rel = a.bool("relative");
        let sets: Vec<(u32, f64)> = pairs(tv).iter().map(|(n, v)| Ok((inf_named(g, n)?, *v))).collect::<CmdResult<_>>()?;
        let zero_rest = a.bool("zeroRemainingInfluences");
        let locks = SkinBind { locks: b.locks.clone(), raw_edit: b.raw_edit, ..Default::default() };
        for &p in &pts {
            let Some(mut pw) = b.weights.get(p as usize).cloned() else { continue };
            if zero_rest {
                pw.retain(|x| locks.locked(x.0));
            }
            let at = b.geom.transform_point3(DVec3::ZERO);
            for &(i, v) in &sets {
                let now = weight_at(&pw, i);
                let next = if rel { now + v } else { v };
                set_point_weight(&mut pw, i, next, &locks, nearest_other(&b, i, at));
            }
            b.weights[p as usize] = pw;
        }
        touched = true;
    }
    if let Some(tm) = a.get("transformMoveWeights") {
        let list_names = names_list(tm);
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for pair in list_names.chunks(2) {
            if pair.len() < 2 {
                continue;
            }
            let (from, to) = (inf_named(g, &pair[0])?, inf_named(g, &pair[1])?);
            for &p in &pts {
                let Some(pw) = b.weights.get_mut(p as usize) else { continue };
                let x = weight_at(pw, from);
                if x > 0.0 && !locks.locked(from) && !locks.locked(to) {
                    let y = weight_at(pw, to);
                    pw.retain(|e| e.0 != from && e.0 != to);
                    pw.push((to, x + y));
                    pw.sort_by_key(|e| e.0);
                }
            }
        }
        touched = true;
    }
    if a.has("pruneWeights") {
        let t = a.float("pruneWeights", 0.0);
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for &p in &pts {
            if let Some(pw) = b.weights.get_mut(p as usize) {
                pw.retain(|x| x.1 >= t || locks.locked(x.0));
                normalize_point(pw, &locks);
            }
        }
        touched = true;
    }
    if a.bool("normalize") {
        let locks = SkinBind { locks: b.locks.clone(), ..Default::default() };
        for &p in &pts {
            if let Some(pw) = b.weights.get_mut(p as usize) {
                normalize_point(pw, &locks);
            }
        }
        touched = true;
    }
    if !touched {
        fail!("skinPercent: nothing to do; give -tv, -tmw, -prw, -nrm, -rtd or -q");
    }
    hold_all(g, sk, &mut b);
    set_bind(g, sk, b)?;
    Ok(Value::Null)
}

/// Where a point's weights come from on a source mesh: pairs of source point and share.
fn surface_sample(m: &Mesh, bvh: Option<&MeshBvh>, near: &[(u32, DVec3)], p: DVec3) -> Vec<(u32, f64)> {
    if let Some(bvh) = bvh {
        if let Some(c) = bvh.closest_point(p) {
            let pts = m.face_points(c.face);
            let mut w: Vec<(u32, f64)> = pts.iter().map(|&q| (q, 1.0 / ((m.position(q) - c.point).length_squared() + 1e-12))).collect();
            let s: f64 = w.iter().map(|x| x.1).sum();
            for x in w.iter_mut() {
                x.1 /= s;
            }
            return w;
        }
    }
    near.iter()
        .min_by(|a, b| (a.1 - p).length_squared().total_cmp(&(b.1 - p).length_squared()))
        .map(|x| vec![(x.0, 1.0)])
        .unwrap_or_default()
}

fn short_name(g: &Graph, id: NodeId) -> String {
    g.node(id).map(|n| n.name.clone()).unwrap_or_default()
}

/// Left/right spellings a mirrored joint's name might use.
fn mirrored_name(name: &str) -> Option<String> {
    for (a, b) in [("L_", "R_"), ("R_", "L_"), ("l_", "r_"), ("r_", "l_"), ("left", "right"), ("right", "left"), ("Left", "Right"), ("Right", "Left")] {
        if let Some(rest) = name.strip_prefix(a) {
            return Some(format!("{b}{rest}"));
        }
    }
    for (a, b) in [("_L", "_R"), ("_R", "_L"), ("_l", "_r"), ("_r", "_l")] {
        if let Some(rest) = name.strip_suffix(a) {
            return Some(format!("{rest}{b}"));
        }
    }
    for (a, b) in [("left", "right"), ("right", "left"), ("Left", "Right"), ("Right", "Left")] {
        if name.contains(a) {
            return Some(name.replacen(a, b, 1));
        }
    }
    None
}

/// How source influences map to destination ones: per -ia in turn (the reference's default closestJoint).
fn influence_map(g: &mut Graph, src: &[(i32, NodeId)], dst: &[(i32, NodeId)], rules: &[String], mirror: Option<usize>) -> HashMap<u32, u32> {
    let mut out = HashMap::new();
    let flip = |p: DVec3| match mirror {
        Some(ax) => {
            let mut q = p;
            q[ax] = -q[ax];
            q
        }
        None => p,
    };
    let dst_pos: Vec<(u32, DVec3, String)> = dst
        .iter()
        .map(|&(i, j)| (i as u32, world(g, j).w_axis.truncate(), short_name(g, j)))
        .collect();
    for &(i, j) in src {
        let name = short_name(g, j);
        let at = flip(world(g, j).w_axis.truncate());
        let mut found = None;
        for r in rules {
            found = match r.as_str() {
                "name" | "label" => {
                    let want = if mirror.is_some() { mirrored_name(&name).unwrap_or_else(|| name.clone()) } else { name.clone() };
                    dst_pos.iter().find(|d| d.2 == want).map(|d| d.0)
                }
                "oneToOne" => dst.get(src.iter().position(|x| x.0 == i).unwrap_or(usize::MAX)).map(|x| x.0 as u32),
                _ => dst_pos.iter().min_by(|a, b| (a.1 - at).length_squared().total_cmp(&(b.1 - at).length_squared())).map(|d| d.0),
            };
            if found.is_some() {
                break;
            }
        }
        if let Some(d) = found {
            out.insert(i as u32, d);
        }
    }
    out
}

fn copy_skin_weights(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let source = match a.get("sourceSkin").and_then(Value::as_str) {
        Some(n) => skin_named(g, n),
        None => names.first().and_then(|n| skin_named(g, n)),
    };
    let dest = match a.get("destinationSkin").and_then(Value::as_str) {
        Some(n) => skin_named(g, n),
        None => names.last().and_then(|n| skin_named(g, n)),
    };
    let (Some(src), Some(dst)) = (source, dest) else { fail!("copySkinWeights: name or select a skinned source and destination") };
    let (Some(src_mesh), Some(dst_mesh)) = (skin_geometry(g, src), skin_geometry(g, dst)) else { fail!("copySkinWeights: no geometry") };
    let (Some(sm), Some(dm)) = (crate::modifiers::cage(g, src_mesh), crate::modifiers::cage(g, dst_mesh)) else { fail!("copySkinWeights: no geometry") };
    let mirror = if a.bool("noMirror") || src != dst && !a.has("mirrorMode") {
        None
    } else {
        match a.string("mirrorMode", "YZ").to_uppercase().as_str() {
            "YZ" => Some(0usize),
            "XZ" => Some(1),
            "XY" => Some(2),
            m => fail!("copySkinWeights: mirror mode {m} is not YZ, XZ or XY"),
        }
    };
    let sb = bind_of(g, src).unwrap_or_default();
    let mut db = (*bind_of(g, dst).unwrap_or_default()).clone();
    let rules: Vec<String> = match a.get("influenceAssociation") {
        Some(v) => names_list(v),
        None => vec!["closestJoint".into()],
    };
    g.evaluate();
    let (sl, dl) = (influences(g, src), influences(g, dst));
    let map = influence_map(g, &sl, &dl, &rules, mirror);
    let sg = sb.geom;
    let dg = db.geom;
    let uv_lookup = a.has("uvSpace").then(|| crate::skin_extra::UvLookup::new(&sm));
    let sa = a.string("surfaceAssociation", "closestPoint");
    let bvh = (sa == "closestPoint" || sa == "rayCast").then(|| MeshBvh::new(&sm));
    let near: Vec<(u32, DVec3)> = (0..sm.point_capacity()).filter(|&p| sm.point_valid(p)).map(|p| (p, sm.position(p))).collect();
    let sgi = sg.inverse();
    let inverse = a.bool("mirrorInverse");
    let wanted = selected_points(g, dst_mesh);
    let locks = SkinBind { locks: db.locks.clone(), ..Default::default() };
    for p in wanted {
        if p as usize >= db.weights.len() {
            continue;
        }
        let wp = dg.transform_point3(dm.position(p));
        let query = match mirror {
            Some(ax) => {
                // The default direction copies the positive side onto the negative side; -mi the other way.
                let on_dest = if inverse { wp[ax] > 1e-9 } else { wp[ax] < -1e-9 };
                if !on_dest {
                    continue;
                }
                let mut q = wp;
                q[ax] = -q[ax];
                q
            }
            None => wp,
        };
        let by_uv = uv_lookup.as_ref().and_then(|l| crate::skin_extra::uv_of_point(&dm, p).and_then(|uv| l.sample(uv)));
        let samples = by_uv.unwrap_or_else(|| surface_sample(&sm, bvh.as_ref(), &near, sgi.transform_point3(query)));
        let mut acc: HashMap<u32, f64> = HashMap::new();
        for (q, share) in samples {
            for &(i, x) in sb.weights.get(q as usize).map(|v| v.as_slice()).unwrap_or(&[]) {
                if let Some(&d) = map.get(&i) {
                    *acc.entry(d).or_default() += x * share;
                }
            }
        }
        if acc.is_empty() {
            continue;
        }
        let mut next: Vec<(u32, f64)> = acc.into_iter().filter(|x| x.1 > 1e-6).collect();
        // Locked influences keep their weight.
        let held: Vec<(u32, f64)> = db.weights[p as usize].iter().copied().filter(|x| locks.locked(x.0)).collect();
        next.retain(|x| !locks.locked(x.0));
        next.extend(held);
        next.sort_by_key(|x| x.0);
        if a.get("normalize").is_none_or(|v| v.as_bool().unwrap_or(true)) {
            normalize_point(&mut next, &locks);
        }
        db.weights[p as usize] = next;
    }
    hold_all(g, dst, &mut db);
    set_bind(g, dst, db)?;
    Ok(Value::Null)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(modes(
        Cmd::new(
            "skinCluster",
            "Bind skin: the mesh named last (or selected last) follows the joints, weighted by closest distance. -mi caps the \
             influences per point, -dr sets the falloff, -bm the bind method, -sm the skinning method, -tsb binds to only the \
             joints given. Edit: -ub unbinds, -ubk unbinds keeping history, -ai and -ri add and remove influences, -lw with -inf \
             locks weights, -fnw normalizes, -sw smooths the selected points. Query: -inf, -wi, -g, -mi, -dr, -sm, -lw.",
            |s, a| {
                let names = objects(s, a);
                if a.bool("query") {
                    return skin_cluster_query(s, a, &names);
                }
                if a.bool("edit") {
                    return skin_cluster_edit(s, a, &names);
                }
                bind(s, a, &names)
            },
        )
        .positional("joints, then the mesh (the selection when none); a skinCluster or skinned mesh with -e and -q")
        .flag(f("toSelectedBones", "tsb", ArgType::Bool, "Only the joints given, not their whole skeleton."))
        .flag(f("maximumInfluences", "mi", ArgType::Int, "Most joints per point (5)."))
        .flag(f("obeyMaxInfluences", "omi", ArgType::Bool, "Hold points to maximumInfluences while weights are edited."))
        .flag(f("dropoffRate", "dr", ArgType::Float, "Falloff with distance (4)."))
        .flag(f("heatmapFalloff", "hmf", ArgType::Float, "Heat map bind: 1 keeps the heat as solved, less tightens it (0.68)."))
        .flag(f("moveJointsMode", "mjm", ArgType::Bool, "With -e: on, joints move without moving the mesh; off, where they are becomes the bind pose."))
        .flag(f("useGeometry", "ug", ArgType::Bool, "With -ai: the influence is a mesh, and points take weight by their distance from its surface."))
        .flag(f("baseShape", "bsh", ArgType::Any, "Accepted for existing scripts."))
        .flag(f("polySmoothness", "ps", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("bindMethod", "bm", ArgType::Int, "0 closest distance, 1 closest in hierarchy, 2 heat map, 3 geodesic voxel."))
        .flag(f("skinMethod", "sm", ArgType::Int, "0 classic linear, 1 dual quaternion, 2 blended point by point by .blendWeights."))
        .flag(f("normalizeWeights", "nw", ArgType::Int, "0 none, 1 interactive, 2 post."))
        .flag(f("weightDistribution", "wd", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("includeHiddenSelections", "ihs", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the skinCluster."))
        .flag(f("unbind", "ub", ArgType::Bool, "With -e: remove the skin, back to the bind pose."))
        .flag(f("unbindKeepHistory", "ubk", ArgType::Bool, "With -e: stop the skinning, keeping the node and its weights."))
        .flag(f("addInfluence", "ai", ArgType::Any, "With -e: joints to add.").multi(1))
        .flag(f("removeInfluence", "ri", ArgType::Any, "With -e: joints to remove; their weight goes to the others.").multi(1))
        .flag(f("removeUnusedInfluence", "rui", ArgType::Bool, "With -e: remove influences that hold no weight."))
        .flag(f("weight", "wt", ArgType::Float, "With -ai: starting weight of the added influence."))
        .flag(f("influence", "inf", ArgType::Any, "With -e -lw: the joint. Query: the influences."))
        .flag(f("weightedInfluence", "wi", ArgType::Bool, "Query: influences holding weight."))
        .flag(f("geometry", "g", ArgType::Bool, "Query: the deformed shape."))
        .flag(f("lockWeights", "lw", ArgType::Any, "With -inf: lock or unlock an influence's weights; alone on -e, every influence."))
        .flag(f("forceNormalizeWeights", "fnw", ArgType::Bool, "With -e: normalize the selected points now."))
        .flag(f("smoothWeights", "sw", ArgType::Float, "With -e: smooth the selected points' weights by this much (0 to 1)."))
        .flag(f("smoothWeightsMaxIterations", "swi", ArgType::Int, "With -sw: passes (2)."))
        .flag(f("recacheBindMatrices", "rbm", ArgType::Bool, "With -e: the joints' current pose becomes the bind pose.")),
    ));

    c.register(modes(
        Cmd::new(
            "skinPercent",
            "Set or query skin weights of points: skinPercent -tv joint1 1 skinCluster1 pCube1.vtx[0]. -tv joint value (multi), -r \
             relative, -zri zeroes the rest, -tmw from to moves weight, -prw prunes weights below a value, -nrm normalizes, -rtd \
             resets to the default bind. Query: -v the weights, -t the influences or one influence's weight, -ib ignores small ones.",
            skin_percent,
        )
        .positional("skinCluster, then points or components (the selection when none)")
        .flag(f("transformValue", "tv", ArgType::List, "joint weight pairs.").multi(2))
        .flag(f("dqBlendWeight", "dqb", ArgType::Float, "The points' blend between linear and dual quaternion, for skinningMethod 2."))
        .flag(f("transformMoveWeights", "tmw", ArgType::List, "from to pairs: moves the weight of one influence to another.").multi(2))
        .flag(f("relative", "r", ArgType::Bool, "-tv adds to the current weight."))
        .flag(f("zeroRemainingInfluences", "zri", ArgType::Bool, "-tv zeroes the influences it doesn't name."))
        .flag(f("pruneWeights", "prw", ArgType::Float, "Removes weights below this value, normalizing what's left."))
        .flag(f("normalize", "nrm", ArgType::Bool, "Normalizes the points."))
        .flag(f("resetToDefault", "rtd", ArgType::Bool, "Back to the weights of a default bind."))
        .flag(f("ignoreBelow", "ib", ArgType::Float, "Query: skip influences below this value."))
        .flag(f("transform", "t", ArgType::Any, "Query: one influence's weight, or the list of influences."))
        .flag(f("value", "v", ArgType::Bool, "Query: weights per influence.")),
    ));

    c.register(
        Cmd::new(
            "copySkinWeights",
            "Copy weights between skins, or mirror them on one skin: copySkinWeights -ss skinCluster1 -ds skinCluster2 -nm, or \
             copySkinWeights -ss skinCluster1 -ds skinCluster1 -mm YZ -mi. Selected points limit what the destination takes.",
            copy_skin_weights,
        )
        .positional("source then destination (the selection when none)")
        .flag(f("sourceSkin", "ss", ArgType::String, "Source skinCluster."))
        .flag(f("destinationSkin", "ds", ArgType::String, "Destination skinCluster."))
        .flag(f("mirrorMode", "mm", ArgType::String, "Mirror plane: YZ (default), XZ or XY."))
        .flag(f("mirrorInverse", "mi", ArgType::Bool, "Copy from the negative side to the positive."))
        .flag(f("noMirror", "nm", ArgType::Bool, "A plain copy."))
        .flag(f("surfaceAssociation", "sa", ArgType::String, "closestPoint (default), rayCast or closestComponent."))
        .flag(f("influenceAssociation", "ia", ArgType::Any, "closestJoint (default), closestBone, oneToOne, label or name, tried in turn.").multi(1))
        .flag(f("normalize", "nr", ArgType::Bool, "Normalize the copied weights (default on)."))
        .flag(f("uvSpace", "uv", ArgType::Any, "Source and destination UV sets: points read the source at their UV position, between the corners of the face there. Sets beyond map1 aren't kept here.").multi(2))
        .flag(f("sampleSpace", "spa", ArgType::Int, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "findRelatedSkinCluster",
            "The skinCluster on a mesh or its transform, or an empty string.",
            |s, a| {
                let Some(n) = a.named().into_iter().next() else { fail!("findRelatedSkinCluster: name a mesh") };
                Ok(json!(skin_named(&s.graph, &n).map(|sk| s.graph.path(sk)).unwrap_or_default()))
            },
        )
        .undoable(false)
        .positional("a mesh or its transform"),
    );

    c.register(modes(
        Cmd::new(
            "artAttrSkinPaintCtx",
            "The paint skin weights tool's settings. Edit and query: -val value, -op opacity, -r radius, -sao the operation \
             (absolute, additive, scale, smooth), -stP the stamp (gaussian, poly, solid, square), -inf the influence. -clear \
             floods the mesh with the value.",
            |s, a| {
                if a.bool("query") {
                    let c = &s.paint_ctx;
                    for (flag, v) in [
                        ("value", json!(c.value)),
                        ("opacity", json!(c.opacity)),
                        ("radius", json!(c.radius)),
                        ("selectedattroper", json!(op_name(c.op))),
                        ("stampProfile", json!(stamp_name(c.stamp))),
                        ("influence", json!(c.influence)),
                        ("colorfeedback", json!(c.color_feedback)),
                    ] {
                        if asked(a, flag) {
                            return Ok(v);
                        }
                    }
                    return Ok(Value::Null);
                }
                if a.has("value") {
                    s.paint_ctx.value = a.float("value", 1.0).clamp(0.0, 1.0);
                }
                if a.has("opacity") {
                    s.paint_ctx.opacity = a.float("opacity", 1.0).clamp(0.0, 1.0);
                }
                if a.has("radius") {
                    s.paint_ctx.radius = a.float("radius", 1.0).max(1e-4);
                }
                if a.has("selectedattroper") {
                    s.paint_ctx.op = match a.string("selectedattroper", "").as_str() {
                        "absolute" => PaintOp::Replace,
                        "additive" => PaintOp::Add,
                        "scale" => PaintOp::Scale,
                        "smooth" => PaintOp::Smooth,
                        o => fail!("artAttrSkinPaintCtx: unknown operation {o}"),
                    };
                }
                if a.has("stampProfile") {
                    s.paint_ctx.stamp = match a.string("stampProfile", "").as_str() {
                        "gaussian" => Stamp::Gaussian,
                        "poly" => Stamp::Poly,
                        "solid" => Stamp::Solid,
                        "square" => Stamp::Square,
                        o => fail!("artAttrSkinPaintCtx: unknown stamp {o}"),
                    };
                }
                if a.has("influence") {
                    s.paint_ctx.influence = a.string("influence", "");
                }
                if a.has("colorfeedback") {
                    s.paint_ctx.color_feedback = a.bool("colorfeedback");
                }
                if a.bool("clear") {
                    // Flood.
                    let names = {
                        let sel: Vec<NodeId> = s.graph.selection().to_vec();
                        sel
                    };
                    for id in names {
                        let Some(m) = components::as_mesh(&s.graph, id) else { continue };
                        let Some(sk) = skin_of(&s.graph, m) else { continue };
                        let Some(inf) = paint_influence(s, sk) else { continue };
                        flood(s, sk, m, inf)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("the tool's name")
        .flag(f("value", "val", ArgType::Float, "The value the brush paints, 0 to 1."))
        .flag(f("opacity", "op", ArgType::Float, "How much of the value each dab applies."))
        .flag(f("radius", "r", ArgType::Float, "Brush radius in world units."))
        .flag(f("selectedattroper", "sao", ArgType::String, "absolute (replace), additive, scale or smooth."))
        .flag(f("stampProfile", "stP", ArgType::String, "gaussian, poly, solid or square."))
        .flag(f("influence", "inf", ArgType::String, "The joint painted."))
        .flag(f("colorfeedback", "cl", ArgType::Bool, "Show the weights in colour."))
        .flag(f("clear", "clr", ArgType::Bool, "Flood: paint the value over the selection's points.")),
    ));

    c.register(
        Cmd::new(
            "skinPaintStamp",
            "One dab of the paint skin weights brush at a world point, from the tool's settings: for scripts and tests. -inv paints \
             the inverse, as Ctrl does in the tool; -sm smooths, as Shift does.",
            |s, a| {
                let names = objects(s, a);
                let Some(n) = names.first() else { fail!("skinPaintStamp: name a skinned mesh") };
                let Some(sk) = skin_named(&s.graph, n) else { fail!("skinPaintStamp: {n} has no skin") };
                let Some(mesh) = skin_geometry(&s.graph, sk) else { fail!("skinPaintStamp: no geometry") };
                let Some(at) = a.vec3("position") else { fail!("skinPaintStamp: -p x y z") };
                let Some(inf) = paint_influence(s, sk) else { fail!("skinPaintStamp: no influences") };
                s.graph.evaluate();
                let Some(m) = components::mesh_of(&mut s.graph, mesh) else { fail!("skinPaintStamp: no geometry") };
                let xf = crate::dag::transform_of(&s.graph, mesh);
                let w = world(&mut s.graph, xf);
                let mut b = (*bind_of(&s.graph, sk).unwrap_or_default()).clone();
                let ctx = s.paint_ctx.clone();
                let n = stamp(&mut b, &m, &w, DVec3::from_array(at), None, inf, &ctx, a.bool("invert"), a.bool("smooth"));
                hold_all(&s.graph, sk, &mut b);
                set_bind(&mut s.graph, sk, b)?;
                Ok(json!(n))
            },
        )
        .positional("a skinned mesh")
        .flag(f("position", "p", ArgType::Vec3, "World centre of the dab."))
        .flag(f("invert", "inv", ArgType::Bool, "Paint the inverse."))
        .flag(f("smooth", "sm", ArgType::Bool, "Smooth instead.")),
    );

    c.register(modes(
        Cmd::new(
            "dagPose",
            "Saved poses of skeletons. -restore -bp returns the joints named to their bind pose; -save stores a pose; -q -bp lists \
             the bind poses of the joints.",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let mut joints: Vec<NodeId> = Vec::new();
                let mut poses: Vec<NodeId> = Vec::new();
                for n in &names {
                    let id = find(g, n, "dagPose")?;
                    if g.node(id).is_some_and(|x| x.ty().name == "dagPose") {
                        poses.push(id);
                    } else {
                        joints.push(id);
                    }
                }
                if a.bool("query") {
                    let mut out = Vec::new();
                    for &j in &joints {
                        if let Some(p) = bind_pose_of(g, j) {
                            let name = g.path(p);
                            if !out.contains(&name) {
                                out.push(name);
                            }
                        }
                    }
                    return Ok(json!(out));
                }
                if a.bool("save") {
                    let name = a.string("name", "pose1");
                    let mut all = Vec::new();
                    for &j in &joints {
                        for k in skeleton_of(g, j) {
                            if !all.contains(&k) {
                                all.push(k);
                            }
                        }
                    }
                    let p = pose_node(g, &name, &all, a.bool("bindPose"))?;
                    return Ok(json!(g.path(p)));
                }
                if a.bool("restore") {
                    let mut targets = poses;
                    if targets.is_empty() {
                        for &j in &joints {
                            if let Some(p) = bind_pose_of(g, j) {
                                if !targets.contains(&p) {
                                    targets.push(p);
                                }
                            }
                        }
                    }
                    if targets.is_empty() {
                        fail!("dagPose: no bind pose for what's named");
                    }
                    let only: Option<Vec<NodeId>> = if joints.is_empty() || a.bool("global") {
                        None
                    } else {
                        let mut all = Vec::new();
                        for &j in &joints {
                            all.push(j);
                            all.extend(g.descendants(j));
                        }
                        Some(all)
                    };
                    for p in targets {
                        restore_pose(g, p, only.as_deref())?;
                    }
                    return Ok(Value::Null);
                }
                fail!("dagPose: give -restore, -save or -q");
            },
        )
        .positional("joints or a dagPose (the selection when none)")
        .flag(f("restore", "r", ArgType::Bool, "Return to the pose."))
        .flag(f("save", "s", ArgType::Bool, "Save a new pose."))
        .flag(f("bindPose", "bp", ArgType::Bool, "The bind pose."))
        .flag(f("global", "g", ArgType::Bool, "The whole pose, not only what's below the joints named."))
        .flag(f("name", "n", ArgType::String, "Name of a saved pose.")),
    ));
}

fn flood(s: &mut Session, sk: NodeId, mesh: NodeId, inf: u32) -> CmdResult<()> {
    let Some(m) = components::mesh_of(&mut s.graph, mesh) else { return Ok(()) };
    let mut b = (*bind_of(&s.graph, sk).unwrap_or_default()).clone();
    let ctx = s.paint_ctx.clone();
    let ids = selected_points(&mut s.graph, mesh);
    if ctx.op == PaintOp::Smooth {
        let old = b.weights.clone();
        for &p in &ids {
            if (p as usize) < old.len() {
                b.weights[p as usize] = smoothed_point(&old, &m, p, ctx.opacity, &b);
            }
        }
    } else {
        for &p in &ids {
            let Some(w) = b.weights.get(p as usize) else { continue };
            let now = weight_at(w, inf);
            let next = match ctx.op {
                PaintOp::Replace => now + (ctx.value - now) * ctx.opacity,
                PaintOp::Add => now + ctx.value * ctx.opacity,
                _ => now * (1.0 + (ctx.value - 1.0) * ctx.opacity),
            };
            let at = b.geom.transform_point3(m.position(p));
            let fb = nearest_other(&b, inf, at);
            let mut pw = std::mem::take(&mut b.weights[p as usize]);
            set_point_weight(&mut pw, inf, next, &b, fb);
            b.weights[p as usize] = pw;
        }
    }
    hold_all(&s.graph, sk, &mut b);
    set_bind(&mut s.graph, sk, b)
}

pub(crate) fn op_named(o: &str, cmd: &str) -> CmdResult<PaintOp> {
    Ok(match o {
        "absolute" => PaintOp::Replace,
        "additive" => PaintOp::Add,
        "scale" => PaintOp::Scale,
        "smooth" => PaintOp::Smooth,
        o => fail!("{cmd}: unknown operation {o}"),
    })
}

pub(crate) fn stamp_named(o: &str, cmd: &str) -> CmdResult<Stamp> {
    Ok(match o {
        "gaussian" => Stamp::Gaussian,
        "poly" => Stamp::Poly,
        "solid" => Stamp::Solid,
        "square" => Stamp::Square,
        o => fail!("{cmd}: unknown stamp {o}"),
    })
}

pub(crate) fn op_name(o: PaintOp) -> &'static str {
    match o {
        PaintOp::Replace => "absolute",
        PaintOp::Add => "additive",
        PaintOp::Scale => "scale",
        PaintOp::Smooth => "smooth",
    }
}

pub(crate) fn stamp_name(s: Stamp) -> &'static str {
    match s {
        Stamp::Gaussian => "gaussian",
        Stamp::Poly => "poly",
        Stamp::Solid => "solid",
        Stamp::Square => "square",
    }
}
