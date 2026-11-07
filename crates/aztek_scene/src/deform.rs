//! Deformers as the reference makes them from the Deform menu: nonlinear deformers (bend, twist, squash, flare, sine, wave)
//! with their handles, clusters, and blend shapes. Each sits in the mesh's history like a skinCluster. The other
//! deformers build on what is here: `deform_lattice`, `deform_soft`, `deform_mush`, `deform_wrap`, `deform_wire`, and
//! the general commands in `deform_cmds`.
//!
//! A node holds one mesh, so a deformer on several meshes is one chain node per mesh: the first is the leader, the
//! others have the leader's settings connected into theirs and read as one deformer through `group`.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{float_attr_short, mesh_attr, STORED_FLAGS};
use crate::components::{self, ComponentMode};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{world, JsonData};
use crate::session::Session;
use crate::transform::{Transform, TransformBehavior};

pub(crate) fn plug(n: NodeId, a: usize) -> PlugRef {
    PlugRef::new(n, a as i32)
}

pub(crate) fn data_attr(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Data,
        default_value: PlugValue::Data(None),
        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
        doc: doc.into(),
        ..Default::default()
    }
}

pub(crate) fn matrix_in(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Matrix,
        default_value: PlugValue::Matrix(DMat4::IDENTITY),
        flags: flags::INPUT | flags::HIDDEN,
        doc: doc.into(),
        ..Default::default()
    }
}

/// An integer setting with a short name.
pub(crate) fn int_attr(name: &str, short: &str, def: i64, doc: &str, lo: Option<f64>) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(def),
        flags: STORED_FLAGS | flags::KEYABLE,
        doc: doc.into(),
        min_value: lo,
        ..Default::default()
    }
}

pub(crate) fn json_of(d: &NodeData, attr: usize) -> Value {
    match d.value(attr) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<JsonData>().map(|j| j.0.clone()).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

pub(crate) fn mat_json(m: &DMat4) -> Value {
    json!(m.to_cols_array().to_vec())
}

pub(crate) fn mat_from(v: &Value) -> DMat4 {
    let a: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if a.len() == 16 {
        DMat4::from_cols_array(&a.try_into().unwrap())
    } else {
        DMat4::IDENTITY
    }
}

// Shared deformer plugs.
pub(crate) const IN: usize = 0;
pub(crate) const OUT: usize = 1;
pub(crate) const ENVELOPE: usize = 2;
pub(crate) const DATA: usize = 3;
pub(crate) const HANDLE: usize = 4;
/// Per-point weights (Floats); none means 1 everywhere.
pub(crate) const WEIGHTS: usize = 5;
/// World matrix of the mesh's transform. The older deformers keep theirs in deformerData from when they were made.
pub(crate) const GEOM: usize = 6;
/// Connected from the deformer's set (cluster1Set).
pub(crate) const DEFORMER_SET: usize = 7;
pub(crate) const BASE_COUNT: usize = 8;

pub const NONLINEAR: &[&str] = &["bend", "twist", "squash", "flare", "sine", "wave"];

pub fn is_deformer(type_name: &str) -> bool {
    NONLINEAR.contains(&type_name) || matches!(type_name, "cluster" | "blendShape" | "ffd" | "softMod" | "deltaMush" | "tension" | "wrap" | "wire") || crate::deform_extra::TYPES.contains(&type_name)
}

/// Heavy per-point data: weights, rest points, lattice mappings, wrap bindings. A flat list of numbers.
#[derive(Debug, Clone, Default)]
pub struct Floats(pub Vec<f64>);

impl crate::plug::Data for Floats {
    fn kind(&self) -> &str {
        "deformFloats"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "deformFloats", "values": self.0})
    }
    fn bytes(&self) -> usize {
        self.0.len() * 8
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn floats_of(d: &NodeData, attr: usize) -> Option<&Floats> {
    match d.value(attr) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<Floats>(),
        _ => None,
    }
}

pub(crate) fn set_floats(g: &mut Graph, n: NodeId, attr: usize, v: Vec<f64>) -> CmdResult<()> {
    g.set_value(plug(n, attr), PlugValue::Data(Some(Arc::new(Floats(v)))))?;
    Ok(())
}

/// Flattens points to x, y, z, x, y, z...
pub(crate) fn flat(points: &[DVec3]) -> Vec<f64> {
    points.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

pub(crate) fn unflat(v: &[f64]) -> Vec<DVec3> {
    v.chunks_exact(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
}

/// A point's weight: its entry times the envelope.
pub(crate) fn weight_at(weights: Option<&Floats>, p: u32, env: f64) -> f64 {
    weights.map_or(1.0, |w| w.0.get(p as usize).copied().unwrap_or(0.0)) * env
}

/// The mesh's points through f in the handle's space: p -> G^-1 H f(H^-1 G p).
fn through_handle(m: &mut Mesh, geom: &DMat4, handle: &DMat4, env: f64, weights: Option<&Floats>, f: impl Fn(DVec3) -> DVec3) {
    let to_h = handle.inverse() * *geom;
    let back = geom.inverse() * *handle;
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) {
            continue;
        }
        let w = weight_at(weights, p, env);
        if w == 0.0 {
            continue;
        }
        let rest = m.position(p);
        let moved = back.transform_point3(f(to_h.transform_point3(rest)));
        m.set_position(p, rest + (moved - rest) * w);
    }
}

pub struct NonLinear {
    kind: usize,
}

// Nonlinear attributes after the shared ones.
const NL_A: usize = BASE_COUNT;
const NL_B: usize = BASE_COUNT + 1;
const NL_C: usize = BASE_COUNT + 2;
const NL_LOW: usize = BASE_COUNT + 3;
const NL_HIGH: usize = BASE_COUNT + 4;
const NL_D: usize = BASE_COUNT + 5;

fn nonlinear(kind: usize, a: f64, b: f64, c: f64, d: f64, e: f64, lo: f64, hi: f64, p: DVec3) -> DVec3 {
    let span = (hi - lo).max(1e-9);
    match kind {
        0 => {
            // bend: curvature a (radians per unit) about Z.
            let curv = a;
            if curv.abs() < 1e-9 {
                return p;
            }
            let r = 1.0 / curv;
            let yc = p.y.clamp(lo, hi);
            let th = curv * yc;
            let (s, co) = th.sin_cos();
            let centre = DVec3::new(r - r * co, r * s, 0.0);
            let n = DVec3::new(co, -s, 0.0);
            let t = DVec3::new(s, co, 0.0);
            centre + n * p.x + t * (p.y - yc) + DVec3::Z * p.z
        }
        1 => {
            // twist: start angle a at the low bound to end angle b at the high one, degrees, about Y.
            let t = ((p.y - lo) / span).clamp(0.0, 1.0);
            let ang = (a + (b - a) * t).to_radians();
            let (s, co) = ang.sin_cos();
            DVec3::new(p.x * co + p.z * s, p.y, -p.x * s + p.z * co)
        }
        2 => {
            // squash: factor a along Y, bulging by expand b in XZ.
            let t = ((p.y - lo) / span).clamp(0.0, 1.0);
            let sy = 1.0 + a;
            let mid = (lo + hi) * 0.5;
            let y = if p.y >= lo && p.y <= hi { mid + (p.y - mid) * sy } else if p.y < lo { mid + (lo - mid) * sy + (p.y - lo) } else { mid + (hi - mid) * sy + (p.y - hi) };
            let bulge = 1.0 + (1.0 / sy.max(0.05).sqrt() - 1.0) * b * 4.0 * t * (1.0 - t);
            DVec3::new(p.x * bulge, y, p.z * bulge)
        }
        3 => {
            // flare: X scale from a to b, Z scale from d to e, c the curve.
            let t = ((p.y - lo) / span).clamp(0.0, 1.0);
            let curve = c * (std::f64::consts::PI * t).sin();
            let sx = a + (b - a) * t + curve;
            let sz = d + (e - d) * t + curve;
            DVec3::new(p.x * sx, p.y, p.z * sz)
        }
        4 => {
            // sine: amplitude a, wavelength b, offset c along Y, displacing X.
            if p.y < lo || p.y > hi {
                return p;
            }
            let wl = b.max(1e-6);
            DVec3::new(p.x + a * (std::f64::consts::TAU * p.y / wl + c).sin(), p.y, p.z)
        }
        _ => {
            // wave: rings in XZ, amplitude a, wavelength b, offset c, out to the high bound.
            let r = (p.x * p.x + p.z * p.z).sqrt();
            if r > hi.abs().max(1e-6) {
                return p;
            }
            let wl = b.max(1e-6);
            DVec3::new(p.x, p.y + a * (std::f64::consts::TAU * r / wl + c).sin(), p.z)
        }
    }
}

impl NodeBehavior for NonLinear {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let data = json_of(d, DATA);
        let geom = mat_from(data.get("geom").unwrap_or(&Value::Null));
        let handle = d.matrix(HANDLE);
        let (a, b, c, dd, lo, hi) = (d.float(NL_A), d.float(NL_B), d.float(NL_C), d.float(NL_D), d.float(NL_LOW), d.float(NL_HIGH));
        let kind = self.kind;
        // Only flare has a seventh setting, endFlareZ.
        let ee = if kind == 3 { d.float(NL_D + 1) } else { 0.0 };
        let mut m = (*input).clone();
        through_handle(&mut m, &geom, &handle, d.float(ENVELOPE), floats_of(d, WEIGHTS), |p| nonlinear(kind, a, b, c, dd, ee, lo, hi, p));
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub struct Cluster;

impl NodeBehavior for Cluster {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let data = json_of(d, DATA);
        let geom = mat_from(data.get("geom").unwrap_or(&Value::Null));
        let bind = mat_from(data.get("bindPre").unwrap_or(&Value::Null));
        let weights = floats_of(d, WEIGHTS);
        // Files from before weightList keep the weights in deformerData.
        let old: Vec<f64> = data.get("weights").and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default();
        let h = d.matrix(HANDLE) * bind;
        let mut m = (*input).clone();
        let gi = geom.inverse();
        let env = d.float(ENVELOPE);
        for p in 0..m.point_capacity() {
            let w = if weights.is_some() || old.is_empty() { weight_at(weights, p, env) } else { old.get(p as usize).copied().unwrap_or(0.0) * env };
            if w == 0.0 || !m.point_valid(p) {
                continue;
            }
            let rest = m.position(p);
            let moved = gi.transform_point3(h.transform_point3(geom.transform_point3(rest)));
            m.set_position(p, rest + (moved - rest) * w);
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// A deformer handle: a transform drawn as the deformer's frame.
pub struct DeformHandle;
impl TransformBehavior for DeformHandle {}
impl NodeBehavior for DeformHandle {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let local = Transform::local_matrix_of(self, d);
        let parent = d.matrix(Transform::PARENT_MATRIX);
        d.set_output(Transform::MATRIX, local);
        d.set_output(Transform::WORLD_MATRIX, parent * local);
    }
    fn as_transform(&self) -> Option<&dyn TransformBehavior> {
        Some(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let xf = types::find("transform").expect("transform");
        types::register_type(NodeType {
            name: "deformHandle".into(),
            base: Some(xf),
            dag: true,
            icon: "axis-y".into(),
            doc: "A deformer's handle: move, turn and scale it to place the deformation.".into(),
            own_attrs: vec![AttrDef {
                name: "deformerType".into(),
                short_name: "dft".into(),
                ty: PlugType::String,
                default_value: PlugValue::String(String::new()),
                flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                doc: "What it handles: bend, cluster...".into(),
                ..Default::default()
            }],
            create: Some(Arc::new(|| Box::new(DeformHandle))),
            ..Default::default()
        })
        .expect("deformHandle");
        let shared = |extra: Vec<AttrDef>| {
            let mut v = vec![
                mesh_attr("inputPolymesh", "ip", flags::INPUT | flags::HIDDEN, "The mesh before the deformer."),
                mesh_attr("output", "out", flags::OUTPUT | flags::HIDDEN, "The deformed mesh."),
                float_attr_short("envelope", "en", 1.0, "How much of the deformation applies.", Some(0.0), Some(1.0)),
                data_attr("deformerData", "dd", "Bind matrices and weights."),
                matrix_in("handleMatrix", "hm", "The handle's world matrix."),
                data_attr("weightList", "wl", "Each point's weight (percent)."),
                matrix_in("geomMatrix", "gm", "World matrix of the deformed mesh."),
                AttrDef { flags: flags::INPUT | flags::HIDDEN, ..float_attr_short("deformerSet", "dfs", 0.0, "The deformer's set of points, connected from it.", None, None) },
            ];
            v.extend(extra);
            v
        };
        crate::plug::data::register_kind("deformFloats", Arc::new(|j| Ok(Arc::new(Floats(j.get("values").and_then(Value::as_array).map(|a| a.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()).unwrap_or_default())) as Arc<dyn crate::plug::Data>)));
        let gf = types::register_type(NodeType { name: "geometryFilter".into(), icon: "wave-sine".into(), doc: "What every deformer is: a mesh in, the deformed mesh out.".into(), own_attrs: shared(vec![]), ..Default::default() }).expect("geometryFilter");
        crate::deform_lattice::register_types(gf.clone());
        crate::deform_soft::register_types(gf.clone());
        crate::deform_mush::register_types(gf.clone());
        crate::deform_wrap::register_types(gf.clone());
        crate::deform_wire::register_types(gf.clone());
        crate::deform_blend::register_types(gf.clone());
        crate::deform_extra::register_types(gf.clone());
        let nl = types::register_type(NodeType { name: "nonLinear".into(), base: Some(gf.clone()), icon: "wave-sine".into(), doc: "A nonlinear deformer.".into(), ..Default::default() }).expect("nonLinear");
        let specs: [(&str, [(&str, &str, f64); 6]); 6] = [
            ("bend", [("curvature", "cur", 0.0), ("unusedB", "ub", 0.0), ("unusedC", "uc", 0.0), ("lowBound", "lb", -1.0), ("highBound", "hb", 1.0), ("unusedD", "ud", 0.0)]),
            ("twist", [("startAngle", "sa", 0.0), ("endAngle", "ea", 0.0), ("unusedC", "uc", 0.0), ("lowBound", "lb", -1.0), ("highBound", "hb", 1.0), ("unusedD", "ud", 0.0)]),
            ("squash", [("factor", "f", 0.0), ("expand", "exp", 1.0), ("unusedC", "uc", 0.0), ("lowBound", "lb", -1.0), ("highBound", "hb", 1.0), ("unusedD", "ud", 0.0)]),
            ("flare", [("startFlareX", "sfx", 1.0), ("endFlareX", "efx", 1.0), ("curve", "c", 0.0), ("lowBound", "lb", -1.0), ("highBound", "hb", 1.0), ("startFlareZ", "sfz", 1.0)]),
            ("sine", [("amplitude", "amp", 0.0), ("wavelength", "wav", 2.0), ("offset", "off", 0.0), ("lowBound", "lb", -1.0), ("highBound", "hb", 1.0), ("unusedD", "ud", 0.0)]),
            ("wave", [("amplitude", "amp", 0.0), ("wavelength", "wav", 1.0), ("offset", "off", 0.0), ("minRadius", "mnr", 0.0), ("maxRadius", "mxr", 1.0), ("unusedD", "ud", 0.0)]),
        ];
        for (k, (name, attrs)) in specs.iter().enumerate() {
            let mut own: Vec<AttrDef> = attrs
                .iter()
                .map(|(n, s, def)| {
                    let mut a = float_attr_short(n, s, *def, "", None, None);
                    if n.starts_with("unused") {
                        a.flags |= flags::HIDDEN;
                        a.flags &= !flags::KEYABLE;
                    }
                    a
                })
                .collect();
            if *name == "flare" {
                own.push(float_attr_short("endFlareZ", "efz", 1.0, "", None, None));
            }
            types::register_type(NodeType {
                name: name.to_string(),
                base: Some(nl.clone()),
                icon: "wave-sine".into(),
                doc: format!("The {name} nonlinear deformer: the mesh {name}s in its handle's frame."),
                own_attrs: own,
                create: Some(Arc::new(move || Box::new(NonLinear { kind: k }))),
                ..Default::default()
            })
            .expect(name);
        }
        types::register_type(NodeType {
            name: "cluster".into(),
            base: Some(gf.clone()),
            icon: "circle-letter-c".into(),
            doc: "Points that follow a handle, each by its weight.".into(),
            create: Some(Arc::new(|| Box::new(Cluster))),
            ..Default::default()
        })
        .expect("cluster");
    });
}

/// Where in the chain a new deformer goes: nearest the mesh (the reference's -after), just above the deformer nearest the mesh
/// (-before), or above every deformer (-frontOfChain).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Place {
    After,
    Before,
    Front,
}

impl Place {
    pub(crate) fn from_args(a: &aztek_ops::Args) -> Place {
        if a.bool("frontOfChain") {
            Place::Front
        } else if a.bool("before") {
            Place::Before
        } else {
            Place::After
        }
    }
}

/// Puts a deformer node at the bottom of the mesh's chain.
pub(crate) fn insert(g: &mut Graph, mesh: NodeId, ty: &str, name: &str) -> CmdResult<NodeId> {
    insert_at(g, mesh, ty, name, Place::After)
}

pub(crate) fn insert_at(g: &mut Graph, mesh: NodeId, ty: &str, name: &str, place: Place) -> CmdResult<NodeId> {
    let t = types::find(ty).unwrap();
    let (mut base, mut src) = crate::history::upstream(g, mesh)?;
    if place != Place::After {
        let chain: Vec<NodeId> = crate::history::history_of(g, mesh).into_iter().filter(|&h| g.node(h).is_some_and(|n| crate::rig::is_rig_chain_node(n.ty().name.as_str()))).collect();
        let target = if place == Place::Before { chain.first() } else { chain.last() };
        if let Some(&d) = target {
            if let Some(up) = g.source(&plug(d, IN)) {
                base = plug(d, IN);
                src = up;
            }
        }
    }
    let src = crate::deform_sets::orig_source(g, mesh, src)?;
    let n = g.create_node(&t, name, NO_NODE, true)?;
    g.disconnect(&base);
    g.connect(&src, &plug(n, IN), false)?;
    g.connect(&plug(n, OUT), &base, false)?;
    let xf = crate::dag::transform_of(g, mesh);
    g.connect(&plug(xf, Transform::WORLD_MATRIX), &plug(n, GEOM), false)?;
    crate::deform_sets::make_set(g, n, mesh)?;
    Ok(n)
}

/// A new deformer's input, evaluated.
pub(crate) fn input_mesh(g: &mut Graph, n: NodeId) -> Option<Arc<Mesh>> {
    g.evaluate();
    g.node(n)?.data().mesh(IN).cloned()
}

/// Limits a new deformer to some points: 1 on those and 0 elsewhere, and only those in its deformer set. None leaves all in.
pub(crate) fn limit_to(g: &mut Graph, n: NodeId, capacity: u32, pts: &Option<Vec<u32>>) -> CmdResult<()> {
    if let Some(w) = weights_for(capacity, pts) {
        set_floats(g, n, WEIGHTS, w)?;
    }
    if let (Some(ids), Some(mesh)) = (pts, mesh_fed_by(g, n)) {
        crate::deform_sets::replace_members(g, n, mesh, ids)?;
    }
    Ok(())
}

/// Weights for a deformer limited to some points: 1 on those, 0 elsewhere. None when all points are in.
pub(crate) fn weights_for(capacity: u32, pts: &Option<Vec<u32>>) -> Option<Vec<f64>> {
    pts.as_ref().map(|v| {
        let mut w = vec![0.0; capacity as usize];
        for &p in v {
            if let Some(x) = w.get_mut(p as usize) {
                *x = 1.0;
            }
        }
        w
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// One deformer over several meshes

/// Connects leader's settings into follower's, so the two read as one deformer.
pub(crate) fn link_follower(g: &mut Graph, leader: NodeId, follower: NodeId) -> CmdResult<()> {
    crate::deform_sets::merge_sets(g, leader, follower)?;
    let attrs: Vec<(usize, bool)> = g.node(leader).unwrap().ty().attrs.iter().enumerate().map(|(i, a)| (i, a.is_array() || a.has(flags::HIDDEN) || !a.has(flags::INPUT) || a.ty == PlugType::Mesh || a.ty == PlugType::Data || a.ty == PlugType::Matrix)).collect();
    for (i, skip) in attrs {
        if !skip {
            g.connect(&plug(leader, i), &plug(follower, i), false)?;
        }
    }
    Ok(())
}

/// The first node of a deformer's group.
pub(crate) fn leader_of(g: &Graph, n: NodeId) -> NodeId {
    match g.source(&plug(n, ENVELOPE)) {
        Some(s) if g.node(s.node).zip(g.node(n)).is_some_and(|(a, b)| a.ty().name == b.ty().name) => s.node,
        _ => n,
    }
}

/// The deformer's nodes, the leader first.
pub(crate) fn group(g: &Graph, n: NodeId) -> Vec<NodeId> {
    let leader = leader_of(g, n);
    let name = g.node(leader).map(|x| x.ty().name.clone()).unwrap_or_default();
    let mut out = vec![leader];
    for d in g.destinations(&plug(leader, ENVELOPE)) {
        if d.attr == ENVELOPE as i32 && g.node(d.node).is_some_and(|x| x.ty().name == name) && !out.contains(&d.node) {
            out.push(d.node);
        }
    }
    out
}

/// The mesh a chain node feeds, following the chain down.
pub(crate) fn mesh_fed_by(g: &Graph, n: NodeId) -> Option<NodeId> {
    let mut at = n;
    for _ in 0..4096 {
        let out = g.node(at)?.ty().attr_index("output").unwrap_or(OUT);
        let dst = g.destinations(&plug(at, out));
        if let Some(m) = dst.iter().find(|d| crate::components::is_mesh(g, d.node)) {
            return Some(m.node);
        }
        at = dst.first()?.node;
    }
    None
}

/// The deformed shapes of a deformer, as deformer -q -g answers.
pub(crate) fn geometry_names(g: &Graph, n: NodeId) -> Vec<String> {
    group(g, n).into_iter().filter_map(|x| mesh_fed_by(g, x)).map(|m| g.path(m)).collect()
}

/// A further mesh for a deformer: a chain node with the leader's settings connected in, the same drivers, and whatever
/// it binds taken from this mesh.
pub(crate) fn add_geometry(g: &mut Graph, leader: NodeId, mesh: NodeId, pts: &Option<Vec<u32>>) -> CmdResult<NodeId> {
    let (ty, name) = {
        let l = g.node(leader).unwrap();
        (l.ty().name.clone(), l.name().to_string())
    };
    let n = insert(g, mesh, &ty, &name)?;
    link_follower(g, leader, n)?;
    let attrs: Vec<(usize, AttrDef)> = g.node(leader).unwrap().ty().attrs.iter().cloned().enumerate().collect();
    for (i, a) in attrs {
        if matches!(i, IN | OUT | GEOM | WEIGHTS) || !a.has(flags::INPUT) {
            continue;
        }
        if a.is_array() {
            for e in g.elements(&plug(leader, i)) {
                let to = PlugRef::element(n, i as i32, e.element);
                match g.source(&e) {
                    Some(s) => {
                        g.connect(&s, &to, false)?;
                    }
                    None => {
                        let v = g.value(&e)?;
                        g.set_value(to, v)?;
                    }
                }
            }
        } else if g.source(&plug(n, i)).is_none() {
            match g.source(&plug(leader, i)) {
                Some(s) => {
                    g.connect(&s, &plug(n, i), false)?;
                }
                None if a.ty == PlugType::Data => {
                    let v = g.stored_value(&plug(leader, i)).clone();
                    g.set_value(plug(n, i), v)?;
                }
                None => {}
            }
        }
    }
    // The older deformers keep the mesh's matrix in their data.
    let mut data = g.node(n).map(|x| json_of(x.data(), DATA)).unwrap_or(Value::Null);
    if data.get("geom").is_some() {
        let geom = world(g, crate::dag::transform_of(g, mesh));
        data["geom"] = mat_json(&geom);
        set_data(g, n, data)?;
    }
    if let Some(input) = input_mesh(g, n) {
        limit_to(g, n, input.point_capacity(), pts)?;
    }
    rebind(g, n)?;
    Ok(n)
}

/// Takes from a node's own mesh what it binds at creation.
fn rebind(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    match g.node(n).map(|x| x.ty().name.clone()).unwrap_or_default().as_str() {
        "ffd" if crate::deform_lattice::is_frozen(g, n) => crate::deform_lattice::capture_stu(g, n),
        "deltaMush" | "tension" => crate::deform_mush::capture_rest(g, n),
        "wrap" => crate::deform_wrap::bind(g, n),
        "softMod" => crate::deform_soft::clear_selected(g, n),
        "blendShape" => crate::deform_blend::rebind(g, n),
        _ => Ok(()),
    }
}

/// Sets the attributes these flags name, where the flag has the attribute's own name.
pub(crate) fn apply_flags(g: &mut Graph, n: NodeId, a: &aztek_ops::Args, names: &[&str]) -> CmdResult<()> {
    for name in names {
        let Some(v) = a.get(name) else { continue };
        let Some(i) = g.node(n).and_then(|x| x.ty().attr_index(name)) else { continue };
        let def = g.node(n).unwrap().ty().attrs[i].clone();
        g.set_value(plug(n, i), crate::plug::from_json(&def, v)?)?;
    }
    Ok(())
}

/// Query mode's answer for attributes named like their flags.
pub(crate) fn query_flags(g: &mut Graph, n: NodeId, a: &aztek_ops::Args, names: &[&str]) -> CmdResult {
    for name in names {
        if crate::skin::asked(a, name) {
            let Some(i) = g.node(n).and_then(|x| x.ty().attr_index(name)) else { continue };
            return Ok(crate::plug::to_json(&g.value(&plug(n, i))?));
        }
    }
    fail!("nothing to query")
}

/// The flags every deformer command shares.
pub(crate) fn common_flags(c: Cmd) -> Cmd {
    c.flag(f("name", "n", ArgType::String, "Name of the deformer."))
        .flag(f("before", "bf", ArgType::Bool, "Go just above the deformer nearest the mesh."))
        .flag(f("after", "af", ArgType::Bool, "Go nearest the mesh, after the other deformers (the default)."))
        .flag(f("frontOfChain", "foc", ArgType::Bool, "Go above every other deformer."))
        .flag(f("envelope", "en", ArgType::Float, "How much of the deformation applies (1)."))
}

/// The node of a deformer's group on this mesh.
pub(crate) fn node_on(g: &Graph, n: NodeId, mesh: NodeId) -> Option<NodeId> {
    let chain = crate::history::history_of(g, mesh);
    group(g, n).into_iter().find(|x| chain.contains(x))
}

/// Takes a chain node out of its chain, joining what fed it to what it fed.
pub(crate) fn splice(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    let src = g.source(&plug(n, IN));
    let dst = g.destinations(&plug(n, OUT));
    g.disconnect(&plug(n, IN));
    for d in dst {
        g.disconnect(&d);
        if let Some(s) = &src {
            g.connect(s, &d, false)?;
        }
    }
    Ok(())
}

/// Whether an input of a deformer is its own handle or lattice, which goes when the deformer goes.
fn driver_attr(name: &str) -> bool {
    matches!(name, "handleMatrix" | "handleLocalMatrix" | "startPosMatrix" | "deformedLatticePoints" | "deformedLatticeMatrix" | "baseLatticeMatrix")
}

fn is_deformer_node(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| is_deformer(n.ty().name.as_str()))
}

/// The transforms of a deformer's handle or lattice.
fn driver_roots(g: &Graph, n: NodeId) -> Vec<NodeId> {
    let Some(node) = g.node(n) else { return Vec::new() };
    let mut out = Vec::new();
    for (i, a) in node.ty().attrs.iter().enumerate() {
        if driver_attr(&a.name) {
            if let Some(s) = g.source(&plug(n, i)) {
                let root = crate::dag::transform_of(g, s.node);
                if !out.contains(&root) {
                    out.push(root);
                }
            }
        }
    }
    out
}

/// The first handle or lattice driving a deformer.
pub fn driver_of(g: &Graph, n: NodeId) -> Option<NodeId> {
    driver_roots(g, n).into_iter().next()
}

/// The deformers a handle or lattice drives.
pub fn driven_deformers(g: &Graph, root: NodeId) -> Vec<NodeId> {
    let mut sources = g.descendants(root);
    sources.push(root);
    let mut out = Vec::new();
    for s in sources {
        let Some(node) = g.node(s) else { continue };
        for i in 0..node.ty().attrs.len() {
            for d in g.destinations(&plug(s, i)) {
                let Some(dn) = g.node(d.node) else { continue };
                if is_deformer(dn.ty().name.as_str()) && driver_attr(&dn.ty().attrs[d.attr as usize].name) && !out.contains(&d.node) {
                    out.push(d.node);
                }
            }
        }
    }
    out
}

/// delete's first step, as the reference deletes a deformer: its handle or lattice goes with it and the other way round, and the
/// chain closes up so the mesh keeps what fed the deformer. Returns everything to delete.
pub(crate) fn expand_delete(g: &mut Graph, ids: Vec<NodeId>) -> CmdResult<Vec<NodeId>> {
    let mut out = ids.clone();
    let mut queue = Vec::new();
    for &id in &ids {
        queue.push(id);
        queue.extend(g.descendants(id));
    }
    let mut seen = Vec::new();
    let mut deformers = Vec::new();
    while let Some(id) = queue.pop() {
        if seen.contains(&id) {
            continue;
        }
        seen.push(id);
        if is_deformer_node(g, id) {
            deformers.push(id);
            if let Some(set) = crate::deform_sets::set_of(g, id) {
                if !out.contains(&set) {
                    out.push(set);
                }
            }
            for x in group(g, id) {
                queue.push(x);
                if !out.contains(&x) {
                    out.push(x);
                }
            }
            for r in driver_roots(g, id) {
                queue.push(r);
                if !out.contains(&r) {
                    out.push(r);
                }
            }
        } else {
            // A target about to go: what the blend shapes take from it stays with them, baked.
            crate::deform_blend::bake_from(g, id)?;
            for d in driven_deformers(g, id) {
                queue.push(d);
                if !out.contains(&d) {
                    out.push(d);
                }
            }
        }
    }
    for d in deformers {
        let mesh = mesh_fed_by(g, d);
        splice(g, d)?;
        if let Some(m) = mesh {
            crate::deform_sets::drop_unused_orig(g, m)?;
        }
    }
    // Children go with their parents.
    let all = out.clone();
    out.retain(|&x| !all.iter().any(|&y| y != x && g.descendants(y).contains(&x)));
    Ok(out)
}

/// Handles and lattices of these deformers, for deleting once the deformers are gone (delete -ch).
pub(crate) fn orphan_drivers(g: &Graph, doomed: &[NodeId]) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &d in doomed {
        if is_deformer_node(g, d) {
            for r in driver_roots(g, d) {
                if driven_deformers(g, r).iter().all(|x| doomed.contains(x)) && !out.contains(&r) {
                    out.push(r);
                }
            }
        }
    }
    out
}

pub(crate) fn set_data(g: &mut Graph, n: NodeId, v: Value) -> CmdResult<()> {
    g.set_value(plug(n, DATA), PlugValue::Data(Some(Arc::new(JsonData(v)))))?;
    Ok(())
}

pub(crate) fn make_handle(g: &mut Graph, name: &str, kind: &str, at: DVec3, scale: DVec3) -> CmdResult<NodeId> {
    let t = types::find("deformHandle").unwrap();
    let h = g.create_node(&t, name, NO_NODE, true)?;
    g.set_value(plug(h, Transform::TRANSLATE), PlugValue::Vec3(at))?;
    g.set_value(plug(h, Transform::SCALE), PlugValue::Vec3(scale))?;
    let attr = g.node(h).unwrap().ty().attr_index("deformerType").unwrap();
    g.set_value(plug(h, attr), PlugValue::String(kind.into()))?;
    Ok(h)
}

/// Meshes from the arguments or selection, with the points picked on each (None for all).
pub(crate) fn targets(s: &mut Session, a: &aztek_ops::Args) -> CmdResult<Vec<(NodeId, Option<Vec<u32>>)>> {
    let g = &mut s.graph;
    let mut out: Vec<(NodeId, Option<Vec<u32>>)> = Vec::new();
    let names = a.named();
    if !names.is_empty() {
        return named_targets(g, &names);
    }
    let mode = components::mode(g);
    if mode != ComponentMode::Object {
        for (node, ids) in components::selected_all(g, mode).clone() {
            let Some(m) = components::as_mesh(g, node) else { continue };
            let Some(cage) = crate::modifiers::cage(g, m) else { continue };
            out.push((m, Some(components::convert(&cage, mode, &ids, ComponentMode::Vertex, false))));
        }
        if !out.is_empty() {
            return Ok(out);
        }
    }
    for id in g.selection().to_vec() {
        if let Some(m) = components::as_mesh(g, id) {
            out.push((m, None));
        }
    }
    return Ok(out);
}

/// Meshes and vertices by name, with the points named on each (None for all).
pub(crate) fn named_targets(g: &mut Graph, names: &[String]) -> CmdResult<Vec<(NodeId, Option<Vec<u32>>)>> {
    let mut out: Vec<(NodeId, Option<Vec<u32>>)> = Vec::new();
    for n in names {
        if components::is_component_string(n) {
            for (m, ids) in crate::skin::points_named(g, std::slice::from_ref(n)) {
                match out.iter_mut().find(|e| e.0 == m) {
                    Some((_, Some(v))) => v.extend(ids),
                    Some(_) => {}
                    None => out.push((m, Some(ids))),
                }
            }
            continue;
        }
        let Some(id) = g.find(n)? else { fail!("no object named {n}") };
        let Some(m) = components::as_mesh(g, id) else { fail!("{n} is not a mesh") };
        if !out.iter().any(|e| e.0 == m) {
            out.push((m, None));
        }
    }
    Ok(out)
}

pub(crate) fn world_bounds(g: &mut Graph, mesh: NodeId) -> (DVec3, DVec3) {
    world_bounds_of(g, mesh, None)
}

/// World bounds of a mesh's points, or just some of them.
pub(crate) fn world_bounds_of(g: &mut Graph, mesh: NodeId, pts: Option<&[u32]>) -> (DVec3, DVec3) {
    let cage = crate::modifiers::cage(g, mesh);
    let w = world(g, crate::dag::transform_of(g, mesh));
    let mut lo = DVec3::splat(f64::MAX);
    let mut hi = DVec3::splat(f64::MIN);
    if let Some(m) = cage {
        for p in 0..m.point_capacity() {
            if m.point_valid(p) && pts.is_none_or(|v| v.contains(&p)) {
                let q = w.transform_point3(m.position(p));
                lo = lo.min(q);
                hi = hi.max(q);
            }
        }
    }
    if lo.x > hi.x {
        return (DVec3::ZERO, DVec3::ZERO);
    }
    (lo, hi)
}

pub(crate) type Cmd = CommandDef<Session>;

pub(crate) fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        Cmd::new(
            "nonLinear",
            "Add a nonlinear deformer (-type bend, twist, squash, flare, sine or wave) to the selected meshes, with a handle \
             fitted to them. Returns [deformer, handle].",
            |s, a| {
                let kind = a.string("type", "bend");
                if !NONLINEAR.contains(&kind.as_str()) {
                    fail!("nonLinear: type must be one of {}", NONLINEAR.join(", "));
                }
                let meshes = targets(s, a)?;
                if meshes.is_empty() {
                    fail!("nonLinear: select meshes");
                }
                let g = &mut s.graph;
                g.evaluate();
                let mut lo = DVec3::splat(f64::MAX);
                let mut hi = DVec3::splat(f64::MIN);
                for (m, _) in &meshes {
                    let (l, h) = world_bounds(g, *m);
                    lo = lo.min(l);
                    hi = hi.max(h);
                }
                let centre = (lo + hi) * 0.5;
                let half = ((hi - lo) * 0.5).max(DVec3::splat(1e-3));
                let sc = if kind == "wave" { DVec3::splat(half.x.max(half.z)) } else { DVec3::splat(half.y) };
                let name = a.string("name", &format!("{kind}1"));
                g.open_chunk("Nonlinear");
                let r = (|| -> CmdResult<(NodeId, NodeId)> {
                    let h = make_handle(g, &format!("{name}Handle"), &kind, centre, sc)?;
                    let mut first = NO_NODE;
                    for (m, _) in &meshes {
                        let geom = world(g, crate::dag::transform_of(g, *m));
                        let n = insert(g, *m, &kind, &name)?;
                        set_data(g, n, json!({"geom": mat_json(&geom)}))?;
                        g.connect(&plug(h, Transform::WORLD_MATRIX), &plug(n, HANDLE), false)?;
                        for (k, v) in [("curvature", "curvature"), ("startAngle", "startAngle"), ("endAngle", "endAngle"), ("factor", "factor"), ("amplitude", "amplitude"), ("wavelength", "wavelength")] {
                            if let Some(x) = a.get(k).and_then(Value::as_f64) {
                                if let Some(i) = g.node(n).unwrap().ty().attr_index(v) {
                                    g.set_value(plug(n, i), PlugValue::Float(x))?;
                                }
                            }
                        }
                        if first == NO_NODE {
                            first = n;
                        } else {
                            link_follower(g, first, n)?;
                        }
                    }
                    g.select(&[h], SelectMode::Replace);
                    Ok((first, h))
                })();
                g.close_chunk();
                let (n, h) = r?;
                Ok(json!([g.path(n), g.path(h)]))
            },
        )
        .positional("meshes (the selection when none)")
        .flag(f("name", "n", ArgType::String, "Name of the deformer; its handle takes the name plus Handle."))
        .flag(f("type", "typ", ArgType::String, "bend, twist, squash, flare, sine or wave.").choices(NONLINEAR))
        .flag(f("curvature", "cur", ArgType::Float, "bend: how far it bends."))
        .flag(f("startAngle", "sa", ArgType::Float, "twist: degrees at the low bound."))
        .flag(f("endAngle", "ea", ArgType::Float, "twist: degrees at the high bound."))
        .flag(f("factor", "f", ArgType::Float, "squash: stretch (positive) or squash (negative)."))
        .flag(f("amplitude", "amp", ArgType::Float, "sine, wave: height of the waves."))
        .flag(f("wavelength", "wav", ArgType::Float, "sine, wave: length of the waves.")),
    );

    c.register(
        Cmd::new(
            "cluster",
            "Make a cluster of the selected vertices (or whole meshes): a handle they follow. Returns [cluster, handle].",
            |s, a| {
                let meshes = targets(s, a)?;
                if meshes.is_empty() {
                    fail!("cluster: select vertices or meshes");
                }
                let g = &mut s.graph;
                g.evaluate();
                // The handle sits at the middle of the picked points.
                let mut sum = DVec3::ZERO;
                let mut n_pts = 0.0;
                for (m, pts) in &meshes {
                    let cage = crate::modifiers::cage(g, *m).unwrap();
                    let w = world(g, crate::dag::transform_of(g, *m));
                    for p in 0..cage.point_capacity() {
                        if cage.point_valid(p) && pts.as_ref().is_none_or(|v| v.contains(&p)) {
                            sum += w.transform_point3(cage.position(p));
                            n_pts += 1.0;
                        }
                    }
                }
                let centre = if n_pts > 0.0 { sum / n_pts } else { DVec3::ZERO };
                let name = a.string("name", "cluster1");
                g.open_chunk("Cluster");
                let r = (|| -> CmdResult<(NodeId, NodeId)> {
                    // As the reference's: at zero, its pivot on the points' middle, so its translate is how far it has moved.
                    let h = make_handle(g, &format!("{name}Handle"), "cluster", DVec3::ZERO, DVec3::ONE)?;
                    g.set_value(plug(h, Transform::ROTATE_PIVOT), PlugValue::Vec3(centre))?;
                    g.set_value(plug(h, Transform::SCALE_PIVOT), PlugValue::Vec3(centre))?;
                    g.evaluate();
                    let hw = world(g, h);
                    let mut first = NO_NODE;
                    for (m, pts) in &meshes {
                        let cage = crate::modifiers::cage(g, *m).unwrap();
                        let geom = world(g, crate::dag::transform_of(g, *m));
                        let n = insert(g, *m, "cluster", &name)?;
                        set_data(g, n, json!({"geom": mat_json(&geom), "bindPre": mat_json(&hw.inverse())}))?;
                        limit_to(g, n, cage.point_capacity(), pts)?;
                        g.connect(&plug(h, Transform::WORLD_MATRIX), &plug(n, HANDLE), false)?;
                        if first == NO_NODE {
                            first = n;
                        } else {
                            link_follower(g, first, n)?;
                        }
                    }
                    components::set_mode_undoable(g, ComponentMode::Object);
                    g.select(&[h], SelectMode::Replace);
                    Ok((first, h))
                })();
                g.close_chunk();
                let (n, h) = r?;
                Ok(json!([g.path(n), g.path(h)]))
            },
        )
        .positional("meshes or vertices (the selection when none)")
        .flag(f("name", "n", ArgType::String, "Name of the cluster; its handle takes the name plus Handle.")),
    );

}

/// Lines drawing a deformer handle, in world space.
pub fn handle_lines(kind: &str, w: &DMat4) -> Vec<(DVec3, DVec3)> {
    let p = |x: f64, y: f64, z: f64| w.transform_point3(DVec3::new(x, y, z));
    let mut out = Vec::new();
    match kind {
        "sculpt" => {
            for plane in 0..3 {
                let n = 32;
                for i in 0..n {
                    let (a, b) = (i as f64 / n as f64 * std::f64::consts::TAU, (i + 1) as f64 / n as f64 * std::f64::consts::TAU);
                    let at = |t: f64| match plane {
                        0 => p(t.cos(), t.sin(), 0.0),
                        1 => p(t.cos(), 0.0, t.sin()),
                        _ => p(0.0, t.cos(), t.sin()),
                    };
                    out.push((at(a), at(b)));
                }
            }
        }
        "cluster" | "softMod" | "wrinkle" => {
            // A letter in the view's plane is the reference's; a small diamond reads the same here.
            let k = 0.25;
            let pts = [p(k, 0.0, 0.0), p(0.0, k, 0.0), p(-k, 0.0, 0.0), p(0.0, -k, 0.0)];
            for i in 0..4 {
                out.push((pts[i], pts[(i + 1) % 4]));
            }
            out.push((p(0.0, 0.0, -k), p(0.0, 0.0, k)));
        }
        "wave" => {
            let n = 24;
            for i in 0..n {
                let (a, b) = (i as f64 / n as f64 * std::f64::consts::TAU, (i + 1) as f64 / n as f64 * std::f64::consts::TAU);
                out.push((p(a.cos(), 0.0, a.sin()), p(b.cos(), 0.0, b.sin())));
            }
            out.push((p(-1.0, 0.0, 0.0), p(1.0, 0.0, 0.0)));
        }
        _ => {
            out.push((p(0.0, -1.0, 0.0), p(0.0, 1.0, 0.0)));
            for y in [-1.0, 1.0] {
                out.push((p(-0.15, y, 0.0), p(0.15, y, 0.0)));
                out.push((p(0.0, y, -0.15), p(0.0, y, 0.15)));
            }
        }
    }
    out
}
