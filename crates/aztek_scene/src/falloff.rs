//! Deformer falloffs, as the reference has them since its 2022 release: a falloff node's outputWeightFunction is a
//! function from a geometry's points to weights, which a deformer's weightFunction multiplies into its own weights.
//! The nodes are in `falloff_nodes.rs`; primitiveFalloff, a transform, is here.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::Mesh;
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::plug::{MeshPtr, PlugValue};

/// What a weight function is evaluated on: the deformer's geometry, its geomMatrix (object to world), and the points
/// the deformer acts on, when it says.
pub struct Cx<'a> {
    pub mesh: &'a Mesh,
    pub geom: DMat4,
    pub members: Option<&'a [bool]>,
}

/// A ramp's points: (position, value, interpolation).
pub type Ramp = Vec<(f64, DVec3, i64)>;

/// From where the falloff starts (u 0) to where it ends (u 1): the ramp's value there, or 1 - u with no ramp.
fn shape(ramp: &Ramp, d: f64, start: f64, end: f64) -> f64 {
    let span = end - start;
    let u = if span.abs() < 1e-12 { if d <= start { 0.0 } else { 1.0 } } else { ((d - start) / span).clamp(0.0, 1.0) };
    match crate::utility_nodes::ramp_value(ramp, u) {
        Some(v) => v.x,
        None => 1.0 - u,
    }
}

#[derive(Debug, Clone)]
pub enum WeightFn {
    Uniform(f64),
    Primitive { plane: bool, inverse: DMat4, positive: DVec3, negative: DVec3, start: f64, end: f64, ramp: Ramp, world: bool },
    Proximity { geometry: Option<MeshPtr>, volume: i64, start: f64, end: f64, ramp: Ramp, world: bool, subset: String },
    Subset { tags: Option<String>, volume: bool, scale: f64, start: f64, end: f64, within: bool, ramp: Ramp },
    Constant(f64),
    /// (mode, weight, function) in order, over a base weight.
    Blend { base: f64, targets: Vec<(i64, f64, Option<Arc<WeightFn>>)> },
    /// A function evaluated on another geometry, carried over by nearest point.
    Transfer { source: Arc<WeightFn>, geometry: MeshPtr },
}

impl crate::plug::Data for WeightFn {
    fn kind(&self) -> &str {
        "weightFunction"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "weightFunction"})
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn function_of(v: &PlugValue) -> Option<&WeightFn> {
    match v {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<WeightFn>(),
        _ => None,
    }
}

pub fn value(f: WeightFn) -> PlugValue {
    PlugValue::Data(Some(Arc::new(f)))
}

fn point_at(cx: &Cx, p: u32, world: bool) -> DVec3 {
    let q = cx.mesh.position(p);
    if world {
        cx.geom.transform_point3(q)
    } else {
        q
    }
}

fn points(m: &Mesh) -> impl Iterator<Item = u32> + '_ {
    (0..m.point_capacity()).filter(|&p| m.point_valid(p))
}

/// Edge hops from the points marked, out to every point (f64::MAX where none reaches).
fn hops(m: &Mesh, from: &[bool]) -> Vec<f64> {
    let mut d = vec![f64::MAX; m.point_capacity() as usize];
    let mut queue = std::collections::VecDeque::new();
    for p in points(m) {
        if from.get(p as usize).copied().unwrap_or(false) {
            d[p as usize] = 0.0;
            queue.push_back(p);
        }
    }
    while let Some(p) = queue.pop_front() {
        let here = d[p as usize];
        let mut next = Vec::new();
        m.for_each_point_edge(p, |e| next.push(m.edge_other_point(e, p)));
        for q in next {
            if d[q as usize] > here + 1.0 {
                d[q as usize] = here + 1.0;
                queue.push_back(q);
            }
        }
    }
    d
}

/// Distance from each point to the nearest point marked.
fn nearest(m: &Mesh, from: &[bool]) -> Vec<f64> {
    let marked: Vec<DVec3> = points(m).filter(|&p| from.get(p as usize).copied().unwrap_or(false)).map(|p| m.position(p)).collect();
    (0..m.point_capacity()).map(|p| if m.point_valid(p) { marked.iter().map(|q| q.distance(m.position(p))).fold(f64::MAX, f64::min) } else { f64::MAX }).collect()
}

impl WeightFn {
    /// A weight per point of cx.mesh, by point id.
    pub fn weights(&self, cx: &Cx) -> Vec<f64> {
        let n = cx.mesh.point_capacity() as usize;
        match self {
            WeightFn::Uniform(w) | WeightFn::Constant(w) => vec![*w; n],
            WeightFn::Primitive { plane, inverse, positive, negative, start, end, ramp, world } => (0..n as u32)
                .map(|p| {
                    if !cx.mesh.point_valid(p) {
                        return 0.0;
                    }
                    let l = inverse.transform_point3(point_at(cx, p, *world));
                    let s = |v: f64, k: usize| {
                        let size = if v >= 0.0 { positive[k] } else { negative[k] };
                        if size.abs() < 1e-12 {
                            f64::MAX
                        } else {
                            v / size
                        }
                    };
                    // A sphere's ellipsoid by its sizes; a plane's distance along its Y.
                    let d = if *plane { s(l.y, 1) } else { DVec3::new(s(l.x, 0), s(l.y, 1), s(l.z, 2)).length() };
                    shape(ramp, d, *start, *end)
                })
                .collect(),
            WeightFn::Proximity { geometry, volume, start, end, ramp, world, subset } => {
                let Some(g) = geometry else { return vec![1.0; n] };
                let faces: Option<Vec<bool>> = if subset.trim().is_empty() || subset.trim() == "*" {
                    None
                } else {
                    crate::component_tags::eval(g, subset).ok().map(|s| crate::component_tags::cast(g, &s, crate::component_tags::Cat::F).on)
                };
                (0..n as u32)
                    .map(|p| {
                        if !cx.mesh.point_valid(p) {
                            return 0.0;
                        }
                        let q = point_at(cx, p, *world);
                        let Some((at, face)) = closest_face(g, q, faces.as_deref()) else { return 0.0 };
                        let side = (q - at).dot(g.face_normal(face));
                        let inside = match volume {
                            1 => side < 0.0,
                            2 => side > 0.0,
                            _ => false,
                        };
                        shape(ramp, if inside { 0.0 } else { q.distance(at) }, *start, *end)
                    })
                    .collect()
            }
            WeightFn::Subset { tags, volume, scale, start, end, within, ramp } => {
                let members: Vec<bool> = match tags {
                    Some(t) => crate::component_tags::points(cx.mesh, t).unwrap_or_else(|| vec![true; n]),
                    None => cx.members.map(<[bool]>::to_vec).unwrap_or_else(|| vec![true; n]),
                };
                let scale = scale.max(1e-6);
                let outside: Vec<bool> = (0..n).map(|p| !members[p] && cx.mesh.point_valid(p as u32)).collect();
                let (d_in, d_out) = if *volume { (nearest(cx.mesh, &outside), nearest(cx.mesh, &members)) } else { (hops(cx.mesh, &outside), hops(cx.mesh, &members)) };
                (0..n)
                    .map(|p| {
                        if !cx.mesh.point_valid(p as u32) {
                            0.0
                        } else if *within {
                            // Rising from the subset's edge inward.
                            if members[p] {
                                let d = if d_in[p] == f64::MAX { f64::MAX } else { (d_in[p] - 1.0).max(0.0) };
                                1.0 - shape(ramp, d / scale, *start, *end)
                            } else {
                                0.0
                            }
                        } else if members[p] {
                            1.0
                        } else {
                            shape(ramp, d_out[p] / scale, *start, *end)
                        }
                    })
                    .collect()
            }
            WeightFn::Blend { base, targets } => {
                let mut acc = vec![*base; n];
                for (mode, w, f) in targets {
                    let Some(f) = f else { continue };
                    let v = f.weights(cx);
                    for (a, &b) in acc.iter_mut().zip(&v) {
                        let r = match mode {
                            1 => *a * b,
                            2 => {
                                if b.abs() < 1e-12 {
                                    *a
                                } else {
                                    *a / b
                                }
                            }
                            3 => *a + b,
                            4 => *a - b,
                            5 | 7 => b,
                            6 => a.max(b),
                            _ => *a,
                        };
                        *a += (r - *a) * w;
                    }
                }
                acc
            }
            WeightFn::Transfer { source, geometry } => {
                let from = source.weights(&Cx { mesh: geometry, geom: cx.geom, members: None });
                (0..n as u32)
                    .map(|p| {
                        if !cx.mesh.point_valid(p) {
                            return 0.0;
                        }
                        let q = cx.mesh.position(p);
                        points(geometry).min_by(|&a, &b| geometry.position(a).distance_squared(q).total_cmp(&geometry.position(b).distance_squared(q))).and_then(|k| from.get(k as usize).copied()).unwrap_or(0.0)
                    })
                    .collect()
            }
        }
    }
}

/// The closest place on a mesh's faces (those marked, when given) and its face.
fn closest_face(m: &Mesh, p: DVec3, only: Option<&[bool]>) -> Option<(DVec3, u32)> {
    let mut best: Option<(f64, DVec3, u32)> = None;
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) || only.is_some_and(|o| !o.get(f as usize).copied().unwrap_or(false)) {
            continue;
        }
        let pts = m.face_points(f);
        for k in 1..pts.len().saturating_sub(1) {
            let q = crate::utility_nodes::closest_on_tri(p, m.position(pts[0]), m.position(pts[k]), m.position(pts[k + 1]));
            let d = q.distance_squared(p);
            if best.as_ref().is_none_or(|b| d < b.0) {
                best = Some((d, q, f));
            }
        }
    }
    best.map(|b| (b.1, b.2))
}

// ---------------------------------------------------------------------------------------------------------------------
// primitiveFalloff: a transform whose place and shape weigh the points.

struct PrimitiveFalloff;

static TRANSFORM: crate::transform::Transform = crate::transform::Transform;

impl crate::node::NodeBehavior for PrimitiveFalloff {
    fn compute(&mut self, ctx: &crate::node::EvalContext, d: &mut crate::node::NodeData) {
        let mut t = crate::transform::Transform;
        crate::node::NodeBehavior::compute(&mut t, ctx, d);
        let ty = d.ty().clone();
        let at = |n: &str| ty.attr_index(n).unwrap();
        let ramp = crate::utility_nodes::ramp_points(d, "ramp.ramp_Position", "ramp.ramp_FloatValue", "ramp.ramp_Interp");
        let f = WeightFn::Primitive {
            plane: d.int(at("primitive")) == 1,
            inverse: d.matrix(crate::transform::Transform::WORLD_MATRIX).inverse(),
            positive: d.vec3(at("positiveSize")),
            negative: d.vec3(at("negativeSize")),
            start: d.float(at("start")),
            end: d.float(at("end")),
            ramp,
            world: d.int(at("vertexSpace")) == 1,
        };
        d.set_output(at("outputWeightFunction"), value(f));
    }
    fn as_transform(&self) -> Option<&dyn crate::transform::TransformBehavior> {
        Some(&TRANSFORM)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_types() {
    use crate::plug::{flags, AttrDef, PlugType};
    let Some(transform) = crate::node::types::find("transform") else { return };
    if crate::node::types::find("primitiveFalloff").is_some() {
        return;
    }
    let keyable = flags::INPUT | flags::STORABLE | flags::KEYABLE;
    let store = flags::INPUT | flags::STORABLE;
    let float = |name: &str, short: &str, v: f64| AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Float, default_value: PlugValue::Float(v), flags: keyable, ..Default::default() };
    let size = |name: &str, short: &str| {
        let mut a = crate::attr_defs::vec3_attr(name, short, DVec3::ONE, keyable, "");
        a.unit = aztek_ops::units::Unit::Linear;
        a.min_value = Some(0.0);
        a
    };
    let en = |name: &str, short: &str, names: &[&str]| AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Enum,
        default_value: PlugValue::Int(0),
        flags: store,
        enum_names: names.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    };
    let mut attrs = vec![
        float("start", "st", 0.0),
        float("end", "ed", 1.0),
        en("primitive", "pmtv", &["Sphere", "Plane"]),
        size("positiveSize", "ps"),
        size("negativeSize", "ns"),
        en("vertexSpace", "vspc", &["Object Space", "World Space"]),
        AttrDef { name: "useOriginalGeometry".into(), short_name: "uo".into(), ty: PlugType::Bool, default_value: PlugValue::Bool(true), flags: store, ..Default::default() },
        AttrDef { name: "outputWeightFunction".into(), short_name: "wft".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::OUTPUT | flags::HIDDEN, doc: "The falloff, for a deformer's weightFunction.".into(), ..Default::default() },
    ];
    attrs.extend(crate::utility_nodes::ramp_attrs("ramp", "rmp"));
    let _ = crate::node::types::register_type(crate::node::NodeType {
        name: "primitiveFalloff".into(),
        base: Some(transform),
        dag: true,
        icon: "sphere".into(),
        doc: "A falloff: points weigh by how far they are inside a sphere or past a plane at this transform.".into(),
        own_attrs: attrs,
        create: Some(Arc::new(|| Box::new(PrimitiveFalloff))),
        ..Default::default()
    });
}
