//! NURBS curves: the nurbsCurve shape with its CVs as components, makeNurbCircle and the curve commands (curve, circle,
//! rebuildCurve, reverseCurve, closeCurve, ...), each edit a history node as in the reference; the info nodes (curveInfo,
//! nearestPointOnCurve, pointOnCurveInfo); and building surfaces from curves (extrude, revolve, loft, planar): NURBS by
//! default, polygons with -po 1, as in the reference.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::nurbs::{self, Form, NurbsCurve};
use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DQuat, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, float_attr_short, mesh_attr};
use crate::components::{self, PointProvider};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::transform::{DagNode, Shape};

/// A curve on a plug.
#[derive(Debug, Clone, Default)]
pub struct CurveData(pub NurbsCurve);

impl crate::plug::Data for CurveData {
    fn kind(&self) -> &str {
        "nurbsCurveData"
    }
    fn to_json(&self) -> Value {
        let c = &self.0;
        json!({"kind": "nurbsCurveData", "degree": c.degree, "form": c.form.index(),
               "cvs": c.cvs.iter().flat_map(|p| [p.x, p.y, p.z]).collect::<Vec<f64>>(), "knots": c.knots})
    }
    fn bytes(&self) -> usize {
        self.0.cvs.len() * 24 + self.0.knots.len() * 8
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn curve_from_json(j: &Value) -> NurbsCurve {
    let nums = |k: &str| -> Vec<f64> { j.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default() };
    NurbsCurve {
        degree: j.get("degree").and_then(Value::as_u64).unwrap_or(1) as usize,
        form: Form::from_index(j.get("form").and_then(Value::as_i64).unwrap_or(0)),
        cvs: nums("cvs").chunks(3).filter(|c| c.len() == 3).map(|c| DVec3::new(c[0], c[1], c[2])).collect(),
        knots: nums("knots"),
    }
}

pub(crate) fn curve_value(v: &PlugValue) -> Option<&NurbsCurve> {
    match v {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<CurveData>().map(|c| &c.0),
        _ => None,
    }
}

pub fn to_value(c: NurbsCurve) -> PlugValue {
    PlugValue::Data(Some(Arc::new(CurveData(c))))
}

pub(crate) fn curve_attr(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: fl, doc: doc.into(), ..Default::default() }
}

pub(crate) fn int_out(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::OUTPUT, doc: doc.into(), ..Default::default() }
}

pub(crate) fn float_out(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Float, default_value: PlugValue::Float(0.0), flags: flags::OUTPUT, doc: doc.into(), ..Default::default() }
}

pub(crate) fn int_in(name: &str, short: &str, def: i64, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(def), flags: flags::INPUT | flags::STORABLE | flags::KEYABLE, doc: doc.into(), ..Default::default() }
}

pub(crate) fn store_bool(name: &str, def: bool, doc: &str) -> AttrDef {
    AttrDef { flags: flags::INPUT | flags::STORABLE, ..bool_attr(name, def, doc) }
}

// ---------------------------------------------------------------------------------------------------------------------
// The shape

pub const SHAPE_TYPE: &str = "nurbsCurve";

pub struct CurveShape;

impl CurveShape {
    pub const CREATE: usize = Shape::COUNT;
    pub const CACHED: usize = Shape::COUNT + 1;
    pub const TWEAK: usize = Shape::COUNT + 2;
    pub const LOCAL: usize = Shape::COUNT + 3;
    pub const WORLD_SPACE: usize = Shape::COUNT + 4;
    pub const DEGREE: usize = Shape::COUNT + 5;
    pub const SPANS: usize = Shape::COUNT + 6;
    pub const FORM: usize = Shape::COUNT + 7;
    pub const MIN_VALUE: usize = Shape::COUNT + 8;
    pub const MAX_VALUE: usize = Shape::COUNT + 9;
    pub const DISP_CV: usize = Shape::COUNT + 10;
    pub const DISP_HULL: usize = Shape::COUNT + 11;
    pub const DISP_EP: usize = Shape::COUNT + 12;
    pub const LINE_WIDTH: usize = Shape::COUNT + 13;
    pub const CONTROL_POINTS: usize = Shape::COUNT + 14;
    /// editPoints: xValueEp, yValueEp, zValueEp, three arrays.
    pub const EDIT_POINTS: usize = Shape::COUNT + 15;
}

impl NodeBehavior for CurveShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let base = curve_value(d.value(Self::CREATE)).or(curve_value(d.value(Self::CACHED))).cloned();
        let Some(mut c) = base else {
            d.set_output(Self::LOCAL, PlugValue::Data(None));
            d.set_output(Self::WORLD_SPACE, PlugValue::Data(None));
            return;
        };
        if let PlugValue::Data(Some(t)) = d.value(Self::TWEAK) {
            if let Some(t) = t.as_any().downcast_ref::<crate::rig::JsonData>() {
                for e in t.0.as_array().into_iter().flatten() {
                    let i = e.get(0).and_then(Value::as_u64).unwrap_or(u64::MAX) as usize;
                    let f = |k: usize| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                    if let Some(p) = c.cvs.get_mut(i) {
                        *p += DVec3::new(f(1), f(2), f(3));
                    }
                }
                // The wrapped CVs of a periodic curve follow their first lap.
                if c.form == Form::Periodic {
                    let lap = c.spans();
                    for i in 0..c.degree {
                        c.cvs[lap + i] = c.cvs[i % lap];
                    }
                }
            }
        }
        // With history feeding the shape a control point is a move on top of it, as the reference's tweaks are; with none it is the CV.
        let moves = curve_value(d.value(Self::CREATE)).is_some();
        for (i, v) in d.elements(Self::CONTROL_POINTS) {
            if let (Some(p), PlugValue::Vec3(x)) = (c.cvs.get_mut(*i as usize), v) {
                *p = if moves { *p + *x } else { *x };
            }
        }
        if c.form == Form::Periodic && !d.elements(Self::CONTROL_POINTS).is_empty() {
            let lap = c.spans();
            for i in 0..c.degree {
                c.cvs[lap + i] = c.cvs[i % lap];
            }
        }
        let world = d.matrix(DagNode::WORLD_MATRIX);
        d.set_output(Self::DEGREE, PlugValue::Int(c.degree as i64));
        d.set_output(Self::SPANS, PlugValue::Int(c.spans() as i64));
        d.set_output(Self::FORM, PlugValue::Int(c.form.index()));
        let (lo, hi) = c.domain();
        d.set_output(Self::MIN_VALUE, PlugValue::Float(lo));
        d.set_output(Self::MAX_VALUE, PlugValue::Float(hi));
        // Where the curve is at each knot, in object space.
        if d.elements.len() <= Self::EDIT_POINTS + 2 {
            d.elements.resize_with(d.values.len(), Vec::new);
        }
        let mut us: Vec<f64> = c.knots.iter().copied().filter(|u| *u >= lo && *u <= hi).collect();
        us.dedup();
        for k in 0..3 {
            d.elements[Self::EDIT_POINTS + k] = us.iter().enumerate().map(|(i, u)| (i as i32, PlugValue::Float(c.eval(*u)[k]))).collect();
        }
        d.set_output(Self::WORLD_SPACE, to_value(c.transformed(&world)));
        d.set_output(Self::LOCAL, to_value(c));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn as_curve(g: &Graph, id: NodeId) -> Option<NodeId> {
    crate::dag::shape_of_type(g, id, SHAPE_TYPE)
}

/// The evaluated curve of a shape, object space.
pub fn local_curve(g: &mut Graph, shape: NodeId) -> Option<NurbsCurve> {
    g.evaluate();
    g.node(shape).and_then(|n| curve_value(n.data().value(CurveShape::LOCAL)).cloned())
}

/// The evaluated curve of a shape, world space.
pub fn world_curve(g: &mut Graph, shape: NodeId) -> Option<NurbsCurve> {
    g.evaluate();
    g.node(shape).and_then(|n| curve_value(n.data().value(CurveShape::WORLD_SPACE)).cloned())
}

/// The curve as last evaluated, for drawing.
pub fn drawn_curve(n: &crate::node::Node) -> Option<&NurbsCurve> {
    curve_value(n.data().value(CurveShape::WORLD_SPACE))
}

/// Line segments of an evaluated shape, world space.
pub fn curve_lines(n: &crate::node::Node) -> Vec<(DVec3, DVec3)> {
    let Some(c) = drawn_curve(n) else { return Vec::new() };
    let pts = c.tessellate(12);
    pts.windows(2).map(|w| (w[0], w[1])).collect()
}

/// Replaces a curve's geometry; history above it stops counting.
pub(crate) fn set_curve_data(g: &mut Graph, shape: NodeId, c: NurbsCurve) -> CmdResult<()> {
    set_cached(g, shape, c)
}

pub(crate) fn set_cached(g: &mut Graph, shape: NodeId, c: NurbsCurve) -> CmdResult<()> {
    g.set_value(plug(shape, CurveShape::CACHED), to_value(c))?;
    Ok(())
}

/// Points moved in object space: into the shape's own curve, or into its tweaks when history feeds it.
fn move_cvs(g: &mut Graph, shape: NodeId, moves: &[(u32, DVec3)]) {
    let Some(cur) = local_curve(g, shape) else { return };
    let has_history = g.source(&plug(shape, CurveShape::CREATE)).is_some();
    if !has_history {
        let mut c = cur;
        for &(i, p) in moves {
            if let Some(cv) = c.cvs.get_mut(i as usize) {
                *cv = p;
            }
        }
        if c.form == Form::Periodic {
            let lap = c.spans();
            for i in 0..c.degree {
                c.cvs[lap + i] = c.cvs[i % lap];
            }
        }
        let _ = set_cached(g, shape, c);
        return;
    }
    // Tweaks: offsets from what history gives.
    let upstream = g.node(shape).and_then(|n| curve_value(n.data().value(CurveShape::CREATE)).cloned()).unwrap_or_default();
    let mut all = cur.cvs.clone();
    for &(i, p) in moves {
        if let Some(cv) = all.get_mut(i as usize) {
            *cv = p;
        }
    }
    let list: Vec<Value> = all
        .iter()
        .enumerate()
        .filter_map(|(i, p)| {
            let d = *p - upstream.cvs.get(i).copied().unwrap_or(*p);
            (d.length_squared() > 1e-18).then(|| json!([i, d.x, d.y, d.z]))
        })
        .collect();
    let _ = crate::rig::set_json(g, shape, CurveShape::TWEAK, json!(list));
}

fn register_provider() {
    components::register_point_provider(PointProvider {
        node_type: SHAPE_TYPE.into(),
        points: Some(Arc::new(|g: &mut Graph, node: NodeId| {
            let Some(c) = local_curve(g, node) else { return Vec::new() };
            let mut out: Vec<(u32, DVec3)> = c.distinct_cvs().iter().enumerate().map(|(i, p)| (i as u32, *p)).collect();
            out.extend(c.edit_points().into_iter().enumerate().map(|(i, p)| (crate::nurbs_components::EP | i as u32, p)));
            out
        })),
        set_points: Some(Arc::new(|g: &mut Graph, node: NodeId, pts: &[(u32, DVec3)]| {
            let Some(c) = local_curve(g, node) else { return };
            let moves = crate::nurbs_components::curve_moves(&c, pts);
            move_cvs(g, node, &moves)
        })),
        edit_plug: None,
        frames: None,
        bake: Some(Arc::new(|g: &mut Graph, node: NodeId, m: &DMat4| {
            if let Some(c) = local_curve(g, node) {
                let _ = set_cached(g, node, c.transformed(m));
                let _ = crate::rig::set_json(g, node, CurveShape::TWEAK, Value::Null);
                g.disconnect(&plug(node, CurveShape::CREATE));
                let _ = g.set_value(plug(node, CurveShape::CREATE), PlugValue::Data(None));
            }
        })),
        soft: None,
        grid: None,
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// History

/// A node making a curve from settings, or changing the curve at its input.
pub struct CurveNode {
    kind: Kind,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Circle,
    Square,
    Rebuild,
    Reverse,
    Close,
    Smooth,
    Fit,
    Extend,
    Offset,
    InsertKnot,
}

const IN: usize = 0;
const OUT: usize = 1;
const P0: usize = 2;

impl NodeBehavior for CurveNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let input = curve_value(d.value(IN)).cloned();
        let c = match self.kind {
            Kind::Circle => {
                let (normal, center, first) = (d.vec3(P0), d.vec3(P0 + 1), d.vec3(P0 + 2));
                let (r, sw) = (d.float(P0 + 3), d.float(P0 + 6));
                Some(nurbs::circle(center, normal, first, r, d.int(P0 + 4).max(1) as usize, d.int(P0 + 5).max(3) as usize, sw))
            }
            Kind::Square => {
                let (normal, center) = (d.vec3(P0), d.vec3(P0 + 1));
                let (sl, deg) = (d.float(P0 + 2), d.int(P0 + 3).max(1) as usize);
                let (sl2, spans) = (d.float(P0 + 4), d.int(P0 + 5).max(1) as usize);
                let n = normal.normalize_or_zero();
                let n = if n == DVec3::ZERO { DVec3::Y } else { n };
                let (u, v) = n.any_orthonormal_pair();
                let (a, b) = (u * sl * 0.5, v * sl2 * 0.5);
                let corners = [center + a + b, center - a + b, center - a - b, center + a - b];
                // Each side is a curve of its own, its CVs evenly along it; the whole square is outputCurve.
                let count = deg + spans;
                for k in 0..4 {
                    let (from, to) = (corners[k], corners[(k + 1) % 4]);
                    let cvs: Vec<DVec3> = (0..count).map(|i| from.lerp(to, i as f64 / (count - 1) as f64)).collect();
                    d.set_output(P0 + 6 + k, to_value(NurbsCurve::open(deg, cvs)));
                }
                let mut pts: Vec<DVec3> = corners.to_vec();
                pts.push(corners[0]);
                Some(NurbsCurve::open(1, pts))
            }
            Kind::Rebuild => input.map(|c| {
                let spans = d.int(P0);
                let deg = d.int(P0 + 1).max(1) as usize;
                let keep_cvs = d.bool(P0 + 2);
                if keep_cvs {
                    NurbsCurve::open(deg, c.distinct_cvs().to_vec())
                } else {
                    c.rebuilt(if spans <= 0 { c.spans() } else { spans as usize }, deg)
                }
            }),
            Kind::Reverse => input.map(|c| c.reversed()),
            Kind::Close => input.map(|c| c.closed()),
            Kind::Smooth => input.map(|c| c.smoothed(d.float(P0), d.int(P0 + 1).max(1) as usize)),
            Kind::Fit => input.map(|c| {
                let pts = c.distinct_cvs().to_vec();
                NurbsCurve::through(3, &pts)
            }),
            Kind::Extend => input.map(|c| c.extended(d.float(P0), d.bool(P0 + 1), d.bool(P0 + 2))),
            Kind::Offset => input.map(|c| {
                let n = d.vec3(P0 + 1);
                c.offset(d.float(P0), (n != DVec3::ZERO).then_some(n))
            }),
            Kind::InsertKnot => input.map(|c| {
                let mut out = c;
                let u = d.float(P0);
                for _ in 0..d.int(P0 + 1).max(1) {
                    out = out.insert_knot(u);
                }
                out
            }),
        };
        d.set_output(OUT, c.map(to_value).unwrap_or(PlugValue::Data(None)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn curve_op_type(name: &str, kind: Kind, doc: &str, params: Vec<AttrDef>) -> NodeType {
    let mut attrs = vec![
        curve_attr("inputCurve", "ic", flags::INPUT | flags::STORABLE | flags::HIDDEN, "The curve changed."),
        curve_attr("outputCurve", "oc", flags::OUTPUT | flags::HIDDEN, "The result."),
    ];
    attrs.extend(params);
    NodeType {
        name: name.into(),
        icon: "chart-line".into(),
        doc: doc.into(),
        own_attrs: attrs,
        create: Some(Arc::new(move || Box::new(CurveNode { kind }))),
        ..Default::default()
    }
}

/// A node reading curves: the info nodes.
struct InfoNode;

impl NodeBehavior for InfoNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(c) = curve_value(d.value(0)).cloned() else { return };
        d.set_output(1, PlugValue::Float(c.length()));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// pointOnCurveInfo: position, tangent and normal at a parameter.
struct PointOnCurveNode;

impl NodeBehavior for PointOnCurveNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(c) = curve_value(d.value(0)).cloned() else { return };
        let u = if d.bool(2) {
            let (lo, hi) = c.domain();
            lo + (hi - lo) * d.float(1)
        } else {
            d.float(1)
        };
        let t = c.tangent(u);
        d.set_output(3, PlugValue::Vec3(c.eval(u)));
        d.set_output(4, PlugValue::Vec3(t));
        d.set_output(5, PlugValue::Vec3(t.normalize_or_zero()));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// nearestPointOnCurve.
struct NearestNode;

impl NodeBehavior for NearestNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(c) = curve_value(d.value(0)).cloned() else { return };
        let p = d.vec3(1);
        let u = c.closest_param(p);
        d.set_output(2, PlugValue::Float(u));
        d.set_output(3, PlugValue::Vec3(c.eval(u)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Meshes from curves

#[derive(Clone, Copy, PartialEq, Eq)]
enum SurfKind {
    Extrude,
    Revolve,
    Loft,
    Planar,
    Sweep,
}

struct SurfNode {
    kind: SurfKind,
}

const S_CURVES: usize = 0;
const S_OUT_SURFACE: usize = 1;
const S_OUT: usize = 2;
const S_POLYGON: usize = 3;
const S_P: usize = 4;

fn grid_mesh(rows: &[Vec<DVec3>], close_u: bool, close_v: bool) -> Mesh {
    let mut m = Mesh::default();
    let nv = rows.len();
    if nv == 0 {
        return m;
    }
    let nu = rows[0].len();
    let ids: Vec<Vec<u32>> = rows.iter().map(|r| r.iter().map(|p| m.add_point(*p)).collect()).collect();
    let umax = if close_u { nu } else { nu.saturating_sub(1) };
    let vmax = if close_v { nv } else { nv.saturating_sub(1) };
    for v in 0..vmax {
        for u in 0..umax {
            let (a, b, c, e) = (ids[v][u], ids[v][(u + 1) % nu], ids[(v + 1) % nv][(u + 1) % nu], ids[(v + 1) % nv][u]);
            m.add_face(&[a, b, c, e]);
        }
    }
    m
}

/// Rotation frames along a path, carrying the first along without twist.
fn frames_along(path: &[DVec3]) -> Vec<DMat3> {
    let mut out = Vec::with_capacity(path.len());
    let tangent = |i: usize| -> DVec3 {
        let a = path[i.saturating_sub(1)];
        let b = path[(i + 1).min(path.len() - 1)];
        (b - a).normalize_or_zero()
    };
    let mut last_t = tangent(0);
    let mut x = last_t.any_orthonormal_vector();
    for i in 0..path.len() {
        let t = tangent(i);
        let q = if t != DVec3::ZERO && last_t != DVec3::ZERO { DQuat::from_rotation_arc(last_t, t) } else { DQuat::IDENTITY };
        x = (q * x).normalize_or_zero();
        out.push(DMat3::from_cols(x, t.cross(x), t));
        last_t = if t != DVec3::ZERO { t } else { last_t };
    }
    out
}

fn is_closed(c: &NurbsCurve) -> bool {
    c.form != Form::Open || (c.eval(c.domain().0) - c.eval(c.domain().1)).length() < 1e-6
}

fn open_points(c: &NurbsCurve, per_span: usize) -> (Vec<DVec3>, bool) {
    let mut pts = c.tessellate(per_span);
    let closed = is_closed(c);
    if closed && pts.len() > 1 {
        pts.pop();
    }
    (pts, closed)
}

impl NodeBehavior for SurfNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let curves: Vec<NurbsCurve> = d.elements(S_CURVES).iter().filter_map(|(_, v)| curve_value(v).cloned()).collect();
        if self.kind != SurfKind::Sweep && d.int(S_POLYGON) != 1 {
            let surface = match self.kind {
                SurfKind::Extrude => curves.first().and_then(|profile| {
                    let o = crate::surfaces::ExtrudeOptions {
                        extrude_type: d.int(S_P + 2),
                        direction: d.vec3(S_P + 4),
                        length: d.float(S_P + 5),
                        scale: d.float(S_P),
                        rotation: d.float(S_P + 1),
                        fixed_path: d.bool(S_P + 6),
                        profile_normal: d.bool(S_P + 7),
                    };
                    crate::surfaces::extrude_surface(profile, curves.get(1), o)
                }),
                SurfKind::Revolve => curves.first().filter(|c| c.is_valid()).map(|c| {
                    let (start, end) = (d.float(S_P + 4), d.float(S_P + 5));
                    nurbs::revolved(c, d.vec3(S_P), d.vec3(S_P + 1), start, end, d.int(S_P + 2).max(3) as usize, d.int(S_P + 6).clamp(1, 3) as usize)
                }),
                SurfKind::Loft => crate::surfaces::loft_surface(
                    &curves,
                    crate::surfaces::LoftOptions {
                        close: d.bool(S_P),
                        uniform: d.bool(S_P + 2),
                        degree: d.int(S_P + 3).max(1) as usize,
                        section_spans: d.int(S_P + 4).max(1) as usize,
                        auto_reverse: d.bool(S_P + 5),
                        reverse_normals: d.bool(S_P + 6),
                    },
                ),
                SurfKind::Planar => curves.first().and_then(|c| crate::surfaces::rectangle_surface(c).or_else(|| crate::nurbs_ops::trimmed_plane(c))),
                SurfKind::Sweep => None,
            };
            d.set_output(S_OUT_SURFACE, surface.map(crate::surfaces::to_value).unwrap_or(PlugValue::Data(None)));
            d.set_output(S_OUT, PlugValue::Mesh(None));
            return;
        }
        d.set_output(S_OUT_SURFACE, PlugValue::Data(None));
        let mesh = match self.kind {
            SurfKind::Extrude => (|| {
                let profile = curves.first()?;
                let (pts, closed_u) = open_points(profile, d.int(S_P + 3).max(1) as usize);
                let scale = d.float(S_P);
                let rotation = d.float(S_P + 1);
                let extrude_type = d.int(S_P + 2);
                let rows: Vec<Vec<DVec3>> = match curves.get(1) {
                    Some(path) if extrude_type != 0 => {
                        let spans = (path.spans() * 8).max(2);
                        let path_pts = path.tessellate(8);
                        let _ = spans;
                        let fr = frames_along(&path_pts);
                        let start = path_pts[0];
                        let f0 = fr[0];
                        // At path (tube): the profile's first placement is carried along; the profile stays where it
                        // sits relative to the path's start.
                        let local: Vec<DVec3> = pts.iter().map(|p| f0.transpose() * (*p - start)).collect();
                        let n = path_pts.len();
                        (0..n)
                            .map(|i| {
                                let s = i as f64 / (n - 1).max(1) as f64;
                                let (sc, ang) = (1.0 + (scale - 1.0) * s, rotation.to_radians() * s);
                                let turn = DMat3::from_rotation_z(ang);
                                local.iter().map(|l| path_pts[i] + fr[i] * (turn * (*l * DVec3::new(sc, sc, 1.0)))).collect()
                            })
                            .collect()
                    }
                    Some(path) => {
                        // Distance: straight along the path's overall direction.
                        let (lo, hi) = path.domain();
                        let off = path.eval(hi) - path.eval(lo);
                        (0..=8).map(|i| pts.iter().map(|p| *p + off * (i as f64 / 8.0)).collect()).collect()
                    }
                    None => {
                        let dir = d.vec3(S_P + 4);
                        let len = d.float(S_P + 5);
                        (0..=1).map(|i| pts.iter().map(|p| *p + dir.normalize_or_zero() * len * i as f64).collect()).collect()
                    }
                };
                Some(grid_mesh(&rows, closed_u, false))
            })(),
            SurfKind::Revolve => (|| {
                let c = curves.first()?;
                let (pts, closed_u) = open_points(c, d.int(S_P + 3).max(1) as usize);
                let axis = d.vec3(S_P).normalize_or_zero();
                let axis = if axis == DVec3::ZERO { DVec3::Y } else { axis };
                let pivot = d.vec3(S_P + 1);
                let sections = d.int(S_P + 2).max(3) as usize;
                let (start, end) = (d.float(S_P + 4), d.float(S_P + 5));
                let full = ((end - start).abs() - 360.0).abs() < 1e-6;
                let rows_n = if full { sections } else { sections + 1 };
                let rows: Vec<Vec<DVec3>> = (0..rows_n)
                    .map(|i| {
                        let a = (start + (end - start) * i as f64 / sections as f64).to_radians();
                        let q = DQuat::from_axis_angle(axis, a);
                        pts.iter().map(|p| pivot + q * (*p - pivot)).collect()
                    })
                    .collect();
                Some(grid_mesh(&rows, closed_u, full))
            })(),
            SurfKind::Loft => (|| {
                if curves.len() < 2 {
                    return None;
                }
                let per = d.int(S_P + 1).max(1) as usize;
                let close_v = d.bool(S_P);
                let (first, closed_u) = open_points(&curves[0], per);
                let n = first.len();
                // Each section sampled at the same number of points.
                let rows: Vec<Vec<DVec3>> = curves
                    .iter()
                    .map(|c| {
                        let (lo, hi) = c.domain();
                        (0..n)
                            .map(|i| {
                                let t = i as f64 / if closed_u { n as f64 } else { (n - 1).max(1) as f64 };
                                c.eval(lo + (hi - lo) * t)
                            })
                            .collect()
                    })
                    .collect();
                Some(grid_mesh(&rows, closed_u, close_v))
            })(),
            SurfKind::Planar => (|| {
                let c = curves.first()?;
                let (pts, _) = open_points(c, 8);
                let mut m = Mesh::default();
                let ids: Vec<u32> = pts.iter().map(|p| m.add_point(*p)).collect();
                if ids.len() >= 3 {
                    m.add_face(&ids);
                }
                Some(m)
            })(),
            SurfKind::Sweep => (|| {
                let path = curves.first()?;
                let radius = d.float(S_P);
                let sides = d.int(S_P + 1).max(3) as usize;
                let per = d.int(S_P + 2).max(1) as usize;
                let scale_end = d.float(S_P + 3);
                let twist = d.float(S_P + 4);
                let path_pts = path.tessellate(per);
                let fr = frames_along(&path_pts);
                let closed_v = is_closed(path);
                let n = if closed_v { path_pts.len() - 1 } else { path_pts.len() };
                let rows: Vec<Vec<DVec3>> = (0..n)
                    .map(|i| {
                        let s = i as f64 / (n - 1).max(1) as f64;
                        let (r, a0) = (radius * (1.0 + (scale_end - 1.0) * s), twist.to_radians() * s);
                        (0..sides)
                            .map(|k| {
                                let a = a0 + k as f64 / sides as f64 * std::f64::consts::TAU;
                                path_pts[i] + fr[i] * DVec3::new(a.cos() * r, a.sin() * r, 0.0)
                            })
                            .collect()
                    })
                    .collect();
                Some(grid_mesh(&rows, true, closed_v))
            })(),
        };
        d.set_output(S_OUT, PlugValue::Mesh(mesh.map(Arc::new)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn surf_type(name: &str, kind: SurfKind, doc: &str, params: Vec<AttrDef>) -> NodeType {
    let mut attrs = vec![
        curve_attr("inputCurve", "ic", flags::INPUT | flags::ARRAY | flags::HIDDEN, "The curves it's made from."),
        curve_attr("outputSurface", "os", flags::OUTPUT | flags::HIDDEN, "The NURBS surface."),
        mesh_attr("outputMesh", "om", flags::OUTPUT | flags::HIDDEN, "The mesh, with polygon on."),
        int_in("polygon", "po", 0, "0 NURBS, 1 polygons."),
    ];
    attrs.extend(params);
    NodeType { name: name.into(), icon: "chart-line".into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(move || Box::new(SurfNode { kind }))), ..Default::default() }
}

pub(crate) fn f_in(name: &str, short: &str, def: f64, doc: &str) -> AttrDef {
    float_attr_short(name, short, def, doc, None, None)
}

pub(crate) fn v_in(name: &str, short: &str, def: DVec3, doc: &str) -> AttrDef {
    vec3_attr(name, short, def, flags::INPUT | flags::STORABLE | flags::KEYABLE, doc)
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        types::register_builtins();
        crate::plug::data::register_kind("nurbsCurveData", Arc::new(|j| Ok(Arc::new(CurveData(curve_from_json(j))) as Arc<dyn crate::plug::Data>)));
        let shape = types::find("shape").expect("shape");
        let stored = flags::INPUT | flags::STORABLE;
        types::register_type(NodeType {
            name: SHAPE_TYPE.into(),
            base: Some(shape),
            dag: true,
            icon: "chart-line".into(),
            doc: "A NURBS curve: its CVs are components, curve1.cv[0].".into(),
            own_attrs: vec![
                curve_attr("create", "cr", flags::INPUT | flags::HIDDEN, "From history."),
                curve_attr("cached", "cc", stored | flags::HIDDEN, "The curve itself, when no history feeds it."),
                curve_attr("cvTweaks", "cvtw", stored | flags::HIDDEN, "CV offsets on top of history."),
                curve_attr("local", "l", flags::OUTPUT | flags::HIDDEN, "The curve in object space."),
                curve_attr("worldSpace", "ws", flags::OUTPUT | flags::HIDDEN, "The curve in world space."),
                int_out("degree", "d", "The degree."),
                int_out("spans", "sp", "How many spans."),
                int_out("form", "f", "0 open, 1 closed, 2 periodic."),
                float_out("minValue", "min", "First parameter."),
                float_out("maxValue", "max", "Last parameter."),
                store_bool("dispCV", false, "Show CVs when not in component mode."),
                store_bool("dispHull", false, "Show the hull."),
                store_bool("dispEP", false, "Show edit points."),
                AttrDef { flags: stored, ..f_in("lineWidth", "lw", -1.0, "Line width; -1 uses the default.") },
                crate::attr_defs::vec3_attr("controlPoints", "cp", DVec3::ZERO, flags::INPUT | flags::HIDDEN | flags::ARRAY, "A CV's place in object space, when something drives it."),
                AttrDef { flags: flags::OUTPUT | flags::HIDDEN | flags::ARRAY, ..float_out("editPoints.xValueEp", "eps.xve", "An edit point's X.") },
                AttrDef { flags: flags::OUTPUT | flags::HIDDEN | flags::ARRAY, ..float_out("editPoints.yValueEp", "eps.yve", "An edit point's Y.") },
                AttrDef { flags: flags::OUTPUT | flags::HIDDEN | flags::ARRAY, ..float_out("editPoints.zValueEp", "eps.zve", "An edit point's Z.") },
            ],
            create: Some(Arc::new(|| Box::new(CurveShape))),
            ..Default::default()
        })
        .expect(SHAPE_TYPE);
        for t in [
            curve_op_type(
                "makeNurbCircle",
                Kind::Circle,
                "A circle or arc.",
                vec![
                    v_in("normal", "nr", DVec3::Z, "The normal."),
                    v_in("center", "c", DVec3::ZERO, "The centre."),
                    v_in("firstPoint", "fp", DVec3::X, "Where the first section starts."),
                    f_in("radius", "r", 1.0, "The radius."),
                    int_in("degree", "d", 3, "The degree."),
                    int_in("sections", "s", 8, "Sections around."),
                    f_in("sweep", "sw", 360.0, "Degrees covered."),
                ],
            ),
            curve_op_type(
                "makeNurbsSquare",
                Kind::Square,
                "A square.",
                vec![
                    v_in("normal", "nr", DVec3::Y, "The normal."),
                    v_in("center", "c", DVec3::ZERO, "The centre."),
                    f_in("sideLength1", "sl1", 1.0, "The side."),
                    int_in("degree", "d", 3, "The degree of each side."),
                    f_in("sideLength2", "sl2", 1.0, "The other side."),
                    int_in("spansPerSide", "sps", 1, "Spans along each side."),
                    curve_attr("outputCurve1", "oc1", flags::OUTPUT | flags::HIDDEN, "A side."),
                    curve_attr("outputCurve2", "oc2", flags::OUTPUT | flags::HIDDEN, "A side."),
                    curve_attr("outputCurve3", "oc3", flags::OUTPUT | flags::HIDDEN, "A side."),
                    curve_attr("outputCurve4", "oc4", flags::OUTPUT | flags::HIDDEN, "A side."),
                ],
            ),
            curve_op_type(
                "rebuildCurve",
                Kind::Rebuild,
                "Rebuilt with even spans.",
                vec![int_in("spans", "s", 4, "Spans; 0 keeps the count."), int_in("degree", "d", 3, "The degree."), store_bool("keepControlPoints", false, "Keep the CVs as they are.")],
            ),
            curve_op_type("reverseCurve", Kind::Reverse, "Direction reversed.", vec![]),
            curve_op_type("closeCurve", Kind::Close, "Closed.", vec![]),
            curve_op_type("smoothCurve", Kind::Smooth, "CVs smoothed.", vec![f_in("smoothness", "s", 0.5, "How far each pass moves."), int_in("iterations", "i", 1, "Passes.")]),
            curve_op_type("fitBspline", Kind::Fit, "A cubic through the CVs.", vec![]),
            curve_op_type(
                "extendCurve",
                Kind::Extend,
                "Lengthened along its ends.",
                vec![f_in("distance", "d", 1.0, "How far."), store_bool("extendStart", false, "At the start."), store_bool("extendEnd", true, "At the end.")],
            ),
            curve_op_type("offsetCurve", Kind::Offset, "Offset in its plane.", vec![f_in("distance", "d", 1.0, "How far."), v_in("normal", "nr", DVec3::ZERO, "The plane's normal; zero works it out.")]),
            curve_op_type("insertKnotCurve", Kind::InsertKnot, "A knot added.", vec![f_in("parameter", "p", 0.0, "Where."), int_in("numberOfKnots", "nk", 1, "How many.")]),
        ] {
            let name = t.name.clone();
            types::register_type(t).expect(&name);
        }
        types::register_type(NodeType {
            name: "curveInfo".into(),
            icon: "chart-line".into(),
            doc: "Facts about a curve: its arc length.".into(),
            own_attrs: vec![curve_attr("inputCurve", "ic", flags::INPUT | flags::HIDDEN, "The curve."), float_out("arcLength", "al", "Length along the curve.")],
            create: Some(Arc::new(|| Box::new(InfoNode))),
            ..Default::default()
        })
        .expect("curveInfo");
        types::register_type(NodeType {
            name: "pointOnCurveInfo".into(),
            icon: "chart-line".into(),
            doc: "A point on a curve at a parameter.".into(),
            own_attrs: vec![
                curve_attr("inputCurve", "ic", flags::INPUT | flags::HIDDEN, "The curve."),
                f_in("parameter", "pr", 0.0, "The parameter, or a fraction of the length with turnOnPercentage."),
                AttrDef { short_name: "top".into(), ..store_bool("turnOnPercentage", false, "parameter is a fraction 0 to 1 of the curve's parameter range.") },
                vec3_attr("position", "p", DVec3::ZERO, flags::OUTPUT, "The point."),
                vec3_attr("tangent", "t", DVec3::ZERO, flags::OUTPUT, "The tangent."),
                vec3_attr("normalizedTangent", "nt", DVec3::ZERO, flags::OUTPUT, "The unit tangent."),
            ],
            create: Some(Arc::new(|| Box::new(PointOnCurveNode))),
            ..Default::default()
        })
        .expect("pointOnCurveInfo");
        types::register_type(NodeType {
            name: "nearestPointOnCurve".into(),
            icon: "chart-line".into(),
            doc: "The point of a curve nearest a position.".into(),
            own_attrs: vec![
                curve_attr("inputCurve", "ic", flags::INPUT | flags::HIDDEN, "The curve."),
                v_in("inPosition", "ip", DVec3::ZERO, "The position."),
                float_out("parameter", "pr", "The parameter of the nearest point."),
                vec3_attr("position", "p", DVec3::ZERO, flags::OUTPUT, "The nearest point."),
            ],
            create: Some(Arc::new(|| Box::new(NearestNode))),
            ..Default::default()
        })
        .expect("nearestPointOnCurve");
        for t in [
            surf_type(
                "extrude",
                SurfKind::Extrude,
                "A profile swept along a path or a direction.",
                vec![
                    f_in("scale", "sc", 1.0, "Scale at the end of the path."),
                    f_in("rotation", "ro", 0.0, "Twist at the end of the path, degrees."),
                    int_in("extrudeType", "et", 2, "0 distance, 1 flat, 2 tube."),
                    int_in("divisions", "dv", 8, "Mesh points per span of the profile."),
                    v_in("direction", "d", DVec3::Y, "Direction with no path."),
                    f_in("length", "l", 1.0, "Length with no path."),
                    AttrDef { short_name: "fpt".into(), ..store_bool("fixedPath", false, "The surface sits on the path, not where the profile is.") },
                    AttrDef { short_name: "upn".into(), ..store_bool("useProfileNormal", false, "The profile's normal is turned along the path's start.") },
                    int_in("useComponentPivot", "ucp", 0, "Where the profile's pivot is: 1 its middle."),
                ],
            ),
            surf_type(
                "revolve",
                SurfKind::Revolve,
                "A profile turned about an axis.",
                vec![
                    v_in("axis", "ax", DVec3::Y, "The axis."),
                    v_in("pivot", "p", DVec3::ZERO, "A point on the axis."),
                    int_in("sections", "s", 8, "Sections around."),
                    int_in("divisions", "dv", 4, "Mesh points per span of the profile."),
                    f_in("startSweep", "ssw", 0.0, "Start, degrees."),
                    f_in("endSweep", "esw", 360.0, "End, degrees."),
                    int_in("degree", "d", 3, "Degree around."),
                ],
            ),
            surf_type(
                "loft",
                SurfKind::Loft,
                "A surface through curves in order.",
                vec![
                    store_bool("close", false, "Back to the first curve."),
                    int_in("divisions", "dv", 8, "Mesh points per span of the first curve."),
                    store_bool("uniform", false, "Even parameters between the curves, rather than by their distance."),
                    int_in("degree", "d", 3, "Degree across the curves."),
                    int_in("sectionSpans", "ss", 1, "Spans between each pair of curves."),
                    store_bool("autoReverse", true, "Turn curves round to run alike."),
                    store_bool("reverseSurfaceNormals", false, "Flip the normals."),
                ],
            ),
            surf_type("planarTrim", SurfKind::Planar, "A face inside a closed curve.", vec![]),
            surf_type(
                "sweepMeshCreator",
                SurfKind::Sweep,
                "A tube along a curve.",
                vec![
                    f_in("scaleProfileX", "spx", 0.1, "Radius of the profile."),
                    int_in("profilePolySides", "pps", 8, "Sides of the profile."),
                    int_in("interpolationSteps", "is", 8, "Rings per span of the curve."),
                    f_in("taper", "tp", 1.0, "Radius scale at the end."),
                    f_in("twist", "tw", 0.0, "Twist at the end, degrees."),
                ],
            ),
        ] {
            let name = t.name.clone();
            types::register_type(t).expect(&name);
        }
        register_provider();
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

type Cmd = CommandDef<Session>;

pub(crate) fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn flat_numbers(v: &Value, out: &mut Vec<f64>) {
    match v {
        Value::Array(a) => a.iter().for_each(|x| flat_numbers(x, out)),
        x => out.extend(x.as_f64()),
    }
}

fn points_flag(a: &Args, name: &str) -> Vec<DVec3> {
    let mut nums = Vec::new();
    if let Some(v) = a.get(name) {
        flat_numbers(v, &mut nums);
    }
    nums.chunks(3).filter(|c| c.len() == 3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
}

pub(crate) fn create_curve(g: &mut Graph, name: &str, c: NurbsCurve, select: bool) -> CmdResult<(NodeId, NodeId)> {
    let (tr, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, name, NO_NODE)?;
    set_cached(g, sh, c)?;
    if select {
        g.select(&[tr], SelectMode::Replace);
    }
    Ok((tr, sh))
}

/// A new curve object, selection untouched.
pub fn create_curve_node(g: &mut Graph, name: &str, c: NurbsCurve) -> CmdResult<(NodeId, NodeId)> {
    create_curve(g, name, c, false)
}

pub(crate) fn curve_arg(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = find(g, name, cmd)?;
    match as_curve(g, id) {
        Some(c) => Ok(c),
        None => fail!("{cmd}: {name} is not a curve"),
    }
}

/// Puts a history node above a curve shape; returns the node. The shape's tweaks go into the input so they aren't lost.
pub(crate) fn insert_op(g: &mut Graph, shape: NodeId, ty: &str, set: &[(usize, PlugValue)]) -> CmdResult<NodeId> {
    let current = local_curve(g, shape).unwrap_or_default();
    let t = types::find(ty).ok_or_else(|| aztek_ops::CommandError::new(format!("no node type {ty}")))?;
    let node = g.create_node(&t, &format!("{ty}1"), NO_NODE, true)?;
    let create = plug(shape, CurveShape::CREATE);
    match g.source(&create) {
        Some(src) if g.node(shape).is_some_and(|n| matches!(n.data().value(CurveShape::TWEAK), PlugValue::Data(None))) => {
            g.connect(&src, &plug(node, IN), false)?;
        }
        _ => {
            g.set_value(plug(node, IN), to_value(current))?;
            let _ = g.set_value(plug(shape, CurveShape::TWEAK), PlugValue::Data(None));
        }
    }
    for (attr, v) in set {
        g.set_value(plug(node, *attr), v.clone())?;
    }
    g.connect(&plug(node, OUT), &create, true)?;
    Ok(node)
}

pub(crate) fn with_common(c: Cmd) -> Cmd {
    c.flag(f("constructionHistory", "ch", ArgType::Bool, "Keep a history node (on by default)."))
        .flag(f("replaceOriginal", "rpo", ArgType::Bool, "Change the curve itself (on by default)."))
        .flag(f("name", "n", ArgType::String, "Name of a new curve."))
        .flag(f("object", "o", ArgType::Bool, "Make an object (accepted for existing scripts)."))
}

/// Runs a curve op on the curves named or selected, with the reference's -ch and -rpo. Returns [curve, node].
pub(crate) fn run_op(s: &mut Session, a: &Args, cmd: &str, ty: &str, set: &[(usize, PlugValue)]) -> CmdResult {
    let names = objects(s, a);
    if names.is_empty() {
        fail!("{cmd}: name or select a curve");
    }
    let g = &mut s.graph;
    let ch = a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(g.construction_history);
    let rpo = a.get("replaceOriginal").and_then(Value::as_bool).unwrap_or(true);
    let mut out = Vec::new();
    for n in names {
        let shape = curve_arg(g, &n, cmd)?;
        let target = if rpo {
            shape
        } else {
            let c = local_curve(g, shape).unwrap_or_default();
            let tr = crate::dag::transform_of(g, shape);
            let wm = crate::rig::world(g, tr);
            create_curve(g, &a.string("name", ""), c.transformed(&wm), false)?.1
        };
        let node = insert_op(g, target, ty, set)?;
        g.evaluate();
        out.push(json!(g.path(crate::dag::transform_of(g, target))));
        if ch {
            out.push(json!(g.path(node)));
        } else {
            bake_history(g, target)?;
        }
    }
    Ok(Value::Array(out))
}

/// The reference's delete -ch on a curve: the shape keeps what history gives, the nodes go.
pub fn bake_history(g: &mut Graph, shape: NodeId) -> CmdResult<()> {
    let Some(c) = local_curve(g, shape) else { return Ok(()) };
    let create = plug(shape, CurveShape::CREATE);
    let mut src = g.source(&create);
    g.disconnect(&create);
    // A disconnected plug keeps its last value, which would hide the shape's own curve.
    g.set_value(create, PlugValue::Data(None))?;
    set_cached(g, shape, c)?;
    let _ = g.set_value(plug(shape, CurveShape::TWEAK), PlugValue::Data(None));
    while let Some(p) = src {
        let up = g.source(&plug(p.node, IN));
        // Stop at nodes other things hang off.
        if g.destinations(&p).is_empty() {
            g.delete_node(p.node)?;
        }
        src = up;
    }
    Ok(())
}

/// History nodes feeding a curve, nearest first.
pub fn curve_history(g: &Graph, shape: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut src = g.source(&plug(shape, CurveShape::CREATE));
    while let Some(p) = src {
        out.push(p.node);
        src = g.source(&plug(p.node, IN));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn surf_output(g: &mut Graph, ty: &str, name: &str, node_name: &str, inputs: &[NodeId], set: &[(usize, PlugValue)], polygon: bool) -> CmdResult<(NodeId, NodeId)> {
    let t = types::find(ty).ok_or_else(|| aztek_ops::CommandError::new(format!("no node type {ty}")))?;
    let node = g.create_node(&t, node_name, NO_NODE, true)?;
    for (i, &shape) in inputs.iter().enumerate() {
        g.connect(&plug(shape, CurveShape::WORLD_SPACE), &PlugRef::element(node, S_CURVES as i32, i as i32), false)?;
    }
    for (attr, v) in set {
        g.set_value(plug(node, *attr), v.clone())?;
    }
    g.set_value(plug(node, S_POLYGON), PlugValue::Int(polygon as i64))?;
    let tr = if polygon {
        let (tr, sh) = crate::mesh_nodes::create_mesh_node(g, name)?;
        g.connect(&plug(node, S_OUT), &plug(sh, crate::mesh_nodes::MeshNode::IN_MESH), true)?;
        tr
    } else {
        let (tr, sh) = crate::dag::create_with_shape(g, crate::surfaces::SHAPE_TYPE, name, NO_NODE)?;
        g.connect(&plug(node, S_OUT_SURFACE), &plug(sh, crate::surfaces::SurfaceShape::CREATE), false)?;
        tr
    };
    g.select(&[tr], SelectMode::Replace);
    Ok((tr, node))
}

fn surf_flags(c: Cmd) -> Cmd {
    c.flag(f("polygon", "po", ArgType::Int, "0 NURBS (the default), 1 polygons."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("range", "rn", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the new object."))
}

fn finish_surf(g: &mut Graph, a: &Args, tr: NodeId, node: NodeId) -> CmdResult {
    let ch = a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(g.construction_history);
    if !ch {
        if let Some(m) = components::as_mesh(g, tr) {
            crate::history::delete_history(g, m)?;
        } else if let Some(s) = crate::surfaces::as_surface(g, tr) {
            crate::surfaces::bake_history(g, s)?;
        }
        return Ok(json!([g.path(tr)]));
    }
    Ok(json!([g.path(tr), g.path(node)]))
}

pub(crate) fn v3(a: &Args, name: &str, def: DVec3) -> DVec3 {
    a.vec3(name).map(DVec3::from_array).unwrap_or(def)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(
        Cmd::new(
            "curve",
            "Create a NURBS curve: curve -d 3 -p 0 0 0 -p 1 0 0 -p 2 1 0 -p 3 1 0. -k gives knots, -per a periodic curve, -ep edit \
             points to pass through, -bezier takes Bezier points. Given a curve: -a appends the points, -r replaces it. Returns its \
             transform.",
            |s, a| {
                let names = a.named();
                let g = &mut s.graph;
                let degree = a.int("degree", 3).max(1) as usize;
                let mut pts = points_flag(a, "point");
                let eps = points_flag(a, "editPoint");
                let mut knots = Vec::new();
                if let Some(k) = a.get("knot") {
                    flat_numbers(k, &mut knots);
                }
                let periodic = a.bool("periodic");
                let existing = match names.first() {
                    Some(n) => Some(curve_arg(g, n, "curve")?),
                    None => None,
                };
                if a.bool("append") {
                    let Some(shape) = existing else { fail!("curve: -a needs a curve") };
                    let old = local_curve(g, shape).unwrap_or_default();
                    let mut all = old.cvs.clone();
                    all.extend(pts);
                    let next = NurbsCurve::open(a.get("degree").map(|_| degree).unwrap_or(old.degree), all);
                    bake_history(g, shape)?;
                    set_cached(g, shape, next)?;
                    return Ok(json!(g.path(crate::dag::transform_of(g, shape))));
                }
                let build = if !eps.is_empty() {
                    NurbsCurve::through(degree, &eps)
                } else if pts.len() < 2 {
                    fail!("curve: give at least two points with -p");
                } else if a.bool("bezier") {
                    // Bezier points: a CV with every third point on the curve, knots with multiplicity 3.
                    let spans = (pts.len() - 1) / 3;
                    let mut k = Vec::new();
                    for i in 0..=spans {
                        for _ in 0..3 {
                            k.push(i as f64);
                        }
                    }
                    NurbsCurve::new(3, Form::Open, pts, k)
                } else if periodic {
                    let degree = degree.min(pts.len().saturating_sub(1)).max(1);
                    if knots.len() == pts.len() + degree - 1 {
                        NurbsCurve::new(degree, Form::Periodic, std::mem::take(&mut pts), knots)
                    } else {
                        // The CVs given are a lap, with or without their wrap.
                        let lap = if pts.len() > degree && pts[..degree] == pts[pts.len() - degree..] { pts[..pts.len() - degree].to_vec() } else { pts };
                        NurbsCurve::periodic(degree, lap)
                    }
                } else {
                    let degree = degree.min(pts.len().saturating_sub(1)).max(1);
                    if !knots.is_empty() {
                        if knots.len() != pts.len() + degree - 1 {
                            fail!("curve: {} CVs of degree {degree} take {} knots, not {}", pts.len(), pts.len() + degree - 1, knots.len());
                        }
                        NurbsCurve::new(degree, Form::Open, pts, knots)
                    } else {
                        NurbsCurve::open(degree, pts)
                    }
                };
                if let Some(shape) = existing {
                    if !a.bool("replace") {
                        fail!("curve: naming a curve needs -a or -r");
                    }
                    bake_history(g, shape)?;
                    set_cached(g, shape, build)?;
                    return Ok(json!(g.path(crate::dag::transform_of(g, shape))));
                }
                let (tr, _) = create_curve(g, &a.string("name", ""), build, true)?;
                Ok(json!(g.path(tr)))
            },
        )
        .positional("a curve, with -a or -r")
        .flag(f("degree", "d", ArgType::Int, "Degree (3)."))
        .flag(f("point", "p", ArgType::List, "A CV: x y z. Repeat it.").multi(3))
        .flag(f("editPoint", "ep", ArgType::List, "An edit point to pass through: x y z. Repeat it.").multi(3))
        .flag(f("knot", "k", ArgType::List, "A knot. Repeat it; CVs + degree - 1 of them.").multi(1))
        .flag(f("periodic", "per", ArgType::Bool, "A closed, periodic curve."))
        .flag(f("bezier", "bez", ArgType::Bool, "The points are Bezier points."))
        .flag(f("append", "a", ArgType::Bool, "Add the points to the curve named."))
        .flag(f("replace", "r", ArgType::Bool, "Replace the curve named."))
        .flag(f("worldSpace", "ws", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("objectSpace", "os", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the transform.")),
    );

    c.register(
        Cmd::new(
            "circle",
            "Create a NURBS circle, with a makeNurbCircle node (-nr normal, -c centre, -r radius, -d degree, -s sections, -sw sweep). \
             Returns [nurbsCircle1, makeNurbCircle1].",
            |s, a| {
                let g = &mut s.graph;
                let ch = a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(g.construction_history);
                let name = a.string("name", "nurbsCircle1");
                g.open_chunk("Circle");
                let r = (|| -> CmdResult<(NodeId, NodeId, NodeId)> {
                    let (tr, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, &name, NO_NODE)?;
                    let t = types::find("makeNurbCircle").unwrap();
                    let n = g.create_node(&t, "makeNurbCircle1", NO_NODE, true)?;
                    g.set_value(plug(n, P0), PlugValue::Vec3(v3(a, "normal", DVec3::Z)))?;
                    g.set_value(plug(n, P0 + 1), PlugValue::Vec3(v3(a, "center", DVec3::ZERO)))?;
                    g.set_value(plug(n, P0 + 2), PlugValue::Vec3(v3(a, "firstPoint", DVec3::X)))?;
                    g.set_value(plug(n, P0 + 3), PlugValue::Float(a.float("radius", 1.0)))?;
                    g.set_value(plug(n, P0 + 4), PlugValue::Int(a.int("degree", 3)))?;
                    g.set_value(plug(n, P0 + 5), PlugValue::Int(a.int("sections", 8)))?;
                    g.set_value(plug(n, P0 + 6), PlugValue::Float(a.float("sweep", 360.0)))?;
                    g.connect(&plug(n, OUT), &plug(sh, CurveShape::CREATE), false)?;
                    Ok((tr, sh, n))
                })();
                g.close_chunk();
                let (tr, sh, n) = r?;
                g.select(&[tr], SelectMode::Replace);
                g.evaluate();
                if !ch {
                    bake_history(g, sh)?;
                    return Ok(json!([g.path(tr)]));
                }
                Ok(json!([g.path(tr), g.path(n)]))
            },
        )
        .flag(f("normal", "nr", ArgType::Vec3, "The circle's normal (0 0 1)."))
        .flag(f("center", "c", ArgType::Vec3, "The centre."))
        .flag(f("firstPoint", "fp", ArgType::Vec3, "Toward the first section's start."))
        .flag(f("radius", "r", ArgType::Float, "Radius (1)."))
        .flag(f("degree", "d", ArgType::Int, "Degree (3)."))
        .flag(f("sections", "s", ArgType::Int, "Sections (8)."))
        .flag(f("sweep", "sw", ArgType::Float, "Degrees covered (360)."))
        .flag(f("useTolerance", "ut", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("object", "o", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the transform.")),
    );

    c.register(
        Cmd::new(
            "nurbsSquare",
            "Create a square curve (-nr normal, -c centre, -sl1 side). Returns [nurbsSquare1, makeNurbsSquare1].",
            |s, a| {
                let g = &mut s.graph;
                let name = a.string("name", "nurbsSquare1");
                let (tr, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, &name, NO_NODE)?;
                let t = types::find("makeNurbsSquare").unwrap();
                let n = g.create_node(&t, "makeNurbsSquare1", NO_NODE, true)?;
                g.set_value(plug(n, P0), PlugValue::Vec3(v3(a, "normal", DVec3::Y)))?;
                g.set_value(plug(n, P0 + 1), PlugValue::Vec3(v3(a, "center", DVec3::ZERO)))?;
                g.set_value(plug(n, P0 + 2), PlugValue::Float(a.float("sideLength1", 1.0)))?;
                g.connect(&plug(n, OUT), &plug(sh, CurveShape::CREATE), false)?;
                g.select(&[tr], SelectMode::Replace);
                Ok(json!([g.path(tr), g.path(n)]))
            },
        )
        .flag(f("normal", "nr", ArgType::Vec3, "The normal."))
        .flag(f("center", "c", ArgType::Vec3, "The centre."))
        .flag(f("sideLength1", "sl1", ArgType::Float, "The side (1)."))
        .flag(f("name", "n", ArgType::String, "Name of the transform.")),
    );

    c.register(with_common(
        Cmd::new("rebuildCurve", "Rebuild with even spans: -s spans (0 keeps the count), -d degree, -kcp keeps the CVs. Returns [curve, node].", |s, a| {
            let set = vec![
                (P0, PlugValue::Int(a.int("spans", 4))),
                (P0 + 1, PlugValue::Int(a.int("degree", 3))),
                (P0 + 2, PlugValue::Bool(a.bool("keepControlPoints"))),
            ];
            run_op(s, a, "rebuildCurve", "rebuildCurve", &set)
        })
        .positional("curves (the selection when none)")
        .flag(f("spans", "s", ArgType::Int, "Spans."))
        .flag(f("degree", "d", ArgType::Int, "Degree."))
        .flag(f("keepControlPoints", "kcp", ArgType::Bool, "Keep the CVs."))
        .flag(f("rebuildType", "rt", ArgType::Int, "Accepted for existing scripts (uniform)."))
        .flag(f("endKnots", "end", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("keepRange", "kr", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("keepEndPoints", "kep", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("keepTangents", "kt", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts.")),
    ));
    c.register(with_common(Cmd::new("reverseCurve", "Reverse the direction. Returns [curve, node].", |s, a| run_op(s, a, "reverseCurve", "reverseCurve", &[])).positional("curves (the selection when none)")));
    c.register(with_common(
        Cmd::new("closeCurve", "Close a curve. Returns [curve, node].", |s, a| run_op(s, a, "closeCurve", "closeCurve", &[]))
            .positional("curves (the selection when none)")
            .flag(f("preserveShape", "ps", ArgType::Int, "Accepted for existing scripts."))
            .flag(f("blendBias", "bb", ArgType::Float, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("smoothCurve", "Smooth the CVs: -s how far, -i passes. Returns [curve, node].", |s, a| {
            let set = vec![(P0, PlugValue::Float(a.float("smoothness", 0.5))), (P0 + 1, PlugValue::Int(a.int("iterations", 1)))];
            run_op(s, a, "smoothCurve", "smoothCurve", &set)
        })
        .positional("curves (the selection when none)")
        .flag(f("smoothness", "s", ArgType::Float, "How far each pass moves."))
        .flag(f("iterations", "i", ArgType::Int, "Passes.")),
    ));
    c.register(with_common(Cmd::new("fitBspline", "A cubic through the curve's CVs. Returns [curve, node].", |s, a| run_op(s, a, "fitBspline", "fitBspline", &[])).positional("curves (the selection when none)").flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts."))));
    c.register(with_common(
        Cmd::new("extendCurve", "Lengthen a curve along its tangent: -d distance, -s 0 end, 1 start, 2 both. Returns [curve, node].", |s, a| {
            let which = a.int("start", 0);
            let set = vec![(P0, PlugValue::Float(a.float("distance", 1.0))), (P0 + 1, PlugValue::Bool(which >= 1)), (P0 + 2, PlugValue::Bool(which != 1))];
            run_op(s, a, "extendCurve", "extendCurve", &set)
        })
        .positional("curves (the selection when none)")
        .flag(f("distance", "d", ArgType::Float, "How far."))
        .flag(f("start", "s", ArgType::Int, "0 the end, 1 the start, 2 both."))
        .flag(f("extensionType", "et", ArgType::Int, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("offsetCurve", "A copy offset in the curve's plane: -d distance, -nr normal. Returns [curve, node].", |s, a| {
            let set = vec![(P0, PlugValue::Float(a.float("distance", 1.0))), (P0 + 1, PlugValue::Vec3(v3(a, "normal", DVec3::ZERO)))];
            let mut b = a.clone();
            b.flags.entry("replaceOriginal".to_string()).or_insert(json!(false));
            run_op(s, &b, "offsetCurve", "offsetCurve", &set)
        })
        .positional("curves (the selection when none)")
        .flag(f("distance", "d", ArgType::Float, "How far."))
        .flag(f("normal", "nr", ArgType::Vec3, "The plane's normal."))
        .flag(f("useGivenNormal", "ugn", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("insertKnotCurve", "Add knots at -p: curve keeps its shape. Returns [curve, node].", |s, a| {
            let set = vec![(P0, PlugValue::Float(a.float("parameter", 0.0))), (P0 + 1, PlugValue::Int(a.int("numberOfKnots", 1)))];
            run_op(s, a, "insertKnotCurve", "insertKnotCurve", &set)
        })
        .positional("curves (the selection when none)")
        .flag(f("parameter", "p", ArgType::Float, "Where."))
        .flag(f("numberOfKnots", "nk", ArgType::Int, "How many."))
        .flag(f("addKnots", "add", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("insertBetween", "ib", ArgType::Bool, "Accepted for existing scripts.")),
    ));

    c.register(
        Cmd::new(
            "detachCurve",
            "Cut a curve at -p (repeat it). With -rpo the first piece replaces it. Returns the pieces' transforms.",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let Some(n) = names.first() else { fail!("detachCurve: name or select a curve") };
                let shape = curve_arg(g, n, "detachCurve")?;
                let c = local_curve(g, shape).unwrap_or_default();
                let mut params = Vec::new();
                if let Some(v) = a.get("parameter") {
                    flat_numbers(v, &mut params);
                }
                params.sort_by(f64::total_cmp);
                if params.is_empty() {
                    fail!("detachCurve: give -p");
                }
                let mut pieces = Vec::new();
                let mut rest = if c.form == Form::Periodic { c.opened() } else { c.clone() };
                for u in params {
                    let (a0, b0) = rest.split(u);
                    pieces.push(a0);
                    rest = b0;
                }
                pieces.push(rest);
                let tr = crate::dag::transform_of(g, shape);
                let base = g.node(tr).map(|x| x.name.clone()).unwrap_or_default();
                let wm = crate::rig::world(g, tr);
                let mut out = Vec::new();
                let rpo = a.bool("replaceOriginal");
                for (i, p) in pieces.into_iter().enumerate() {
                    if i == 0 && rpo {
                        bake_history(g, shape)?;
                        set_cached(g, shape, p)?;
                        out.push(json!(g.path(tr)));
                    } else {
                        let (t2, _) = create_curve(g, &format!("{base}detachedCurve{}", i + 1), p.transformed(&wm), false)?;
                        out.push(json!(g.path(t2)));
                    }
                }
                Ok(Value::Array(out))
            },
        )
        .positional("a curve (the selection when none)")
        .flag(f("parameter", "p", ArgType::List, "Where to cut. Repeat it.").multi(1))
        .flag(f("replaceOriginal", "rpo", ArgType::Bool, "The first piece replaces the curve."))
        .flag(f("keep", "k", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "attachCurve",
            "Join curves in order. With -rpo the first takes the result. Returns the transform.",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                if names.len() < 2 {
                    fail!("attachCurve: name or select two curves");
                }
                let mut shapes = Vec::new();
                for n in &names {
                    shapes.push(curve_arg(g, n, "attachCurve")?);
                }
                let mut out = world_curve(g, shapes[0]).unwrap_or_default();
                for &sh in &shapes[1..] {
                    let o = world_curve(g, sh).unwrap_or_default();
                    out = out.attached(&o);
                }
                if a.bool("replaceOriginal") {
                    let tr = crate::dag::transform_of(g, shapes[0]);
                    let inv = crate::rig::world(g, tr).inverse();
                    bake_history(g, shapes[0])?;
                    set_cached(g, shapes[0], out.transformed(&inv))?;
                    return Ok(json!(g.path(tr)));
                }
                let (tr, _) = create_curve(g, &a.string("name", "attachedCurve1"), out, true)?;
                Ok(json!(g.path(tr)))
            },
        )
        .positional("curves (the selection when none)")
        .flag(f("replaceOriginal", "rpo", ArgType::Bool, "The first curve takes the result."))
        .flag(f("name", "n", ArgType::String, "Name of the new curve."))
        .flag(f("method", "m", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("blendBias", "bb", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new("setCurvePoints", "Move a curve's CVs at once, in object space: -p [[x, y, z], ...], from the first CV. A periodic curve's wrapped CVs follow their first lap.", |s, a| {
            let names = objects(s, a);
            let Some(n) = names.first() else { fail!("setCurvePoints: name or select a curve") };
            let g = &mut s.graph;
            let shape = curve_arg(g, n, "setCurvePoints")?;
            let pts = points_flag(a, "points");
            let Some(cur) = local_curve(g, shape) else { fail!("setCurvePoints: {n} has no curve") };
            if pts.len() > cur.cvs.len() {
                fail!("setCurvePoints: {n} has {} CVs, got {}", cur.cvs.len(), pts.len());
            }
            let moves: Vec<(u32, DVec3)> = pts.iter().enumerate().map(|(i, p)| (i as u32, *p)).collect();
            move_cvs(g, shape, &moves);
            Ok(json!(pts.len()))
        })
        .positional("a curve (the selection when none)")
        .flag(f("points", "p", ArgType::List, "[[x, y, z], ...].")),
    );

    c.register(
        Cmd::new("arclen", "The length of a curve along its arc, world space.", |s, a| {
            let names = objects(s, a);
            let Some(n) = names.first() else { fail!("arclen: name or select a curve") };
            let g = &mut s.graph;
            let shape = curve_arg(g, n, "arclen")?;
            Ok(json!(world_curve(g, shape).map(|c| c.length()).unwrap_or(0.0)))
        })
        .undoable(false)
        .positional("a curve (the selection when none)")
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "pointOnCurve",
            "Facts at a parameter of a curve: -pr parameter (-top a fraction of its length), asking -p position, -t tangent, -nt unit \
             tangent (position when none).",
            |s, a| {
                let names = objects(s, a);
                let Some(n) = names.first() else { fail!("pointOnCurve: name or select a curve") };
                let g = &mut s.graph;
                let shape = curve_arg(g, n, "pointOnCurve")?;
                let Some(c) = world_curve(g, shape) else { fail!("pointOnCurve: {n} has no curve") };
                let u = a.float("parameter", 0.0);
                let u = if a.bool("turnOnPercentage") { c.param_at_fraction(u) } else { u };
                let v = if a.bool("normalizedTangent") {
                    c.tangent(u).normalize_or_zero()
                } else if a.bool("tangent") {
                    c.tangent(u)
                } else {
                    c.eval(u)
                };
                Ok(json!([v.x, v.y, v.z]))
            },
        )
        .undoable(false)
        .positional("a curve (the selection when none)")
        .flag(f("parameter", "pr", ArgType::Float, "The parameter."))
        .flag(f("turnOnPercentage", "top", ArgType::Bool, "-pr is a fraction 0 to 1 of the length."))
        .flag(f("position", "p", ArgType::Bool, "Ask for the point."))
        .flag(f("tangent", "t", ArgType::Bool, "Ask for the tangent."))
        .flag(f("normalizedTangent", "nt", ArgType::Bool, "Ask for the unit tangent."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "polyToCurve",
            "A curve through a loop of selected edges (the reference's Mesh > Create Curve From Edges): -d the degree (3), -f 0 open or 2 best. \
             Returns [curve].",
            |s, a| {
                let g = &mut s.graph;
                let mode = components::mode(g);
                let degree = a.int("degree", 3).max(1) as usize;
                let mut result = None;
                for &sel in g.selection().to_vec().iter() {
                    let Some(shape) = components::as_mesh(g, sel) else { continue };
                    let ids = components::selected(g, shape, components::ComponentMode::Edge).to_vec();
                    if ids.is_empty() || mode != components::ComponentMode::Edge {
                        continue;
                    }
                    let Some(m) = components::mesh_of(g, shape) else { continue };
                    // Walk the edges end to end.
                    let mut left = ids.clone();
                    let first = left.remove(0);
                    let mut chain = vec![m.edge_point(first, 0), m.edge_point(first, 1)];
                    loop {
                        let tail = *chain.last().unwrap();
                        let head = chain[0];
                        if let Some(i) = left.iter().position(|&e| m.edge_point(e, 0) == tail || m.edge_point(e, 1) == tail) {
                            let e = left.remove(i);
                            chain.push(m.edge_other_point(e, tail));
                        } else if let Some(i) = left.iter().position(|&e| m.edge_point(e, 0) == head || m.edge_point(e, 1) == head) {
                            let e = left.remove(i);
                            chain.insert(0, m.edge_other_point(e, head));
                        } else {
                            break;
                        }
                    }
                    let closed = chain.first() == chain.last();
                    let w = crate::rig::world(g, crate::dag::transform_of(g, shape));
                    let mut pts: Vec<DVec3> = chain.iter().map(|&p| w.transform_point3(m.position(p))).collect();
                    if closed {
                        pts.pop();
                    }
                    let c = if closed {
                        let s = NurbsCurve::periodic(degree, pts);
                        s
                    } else {
                        NurbsCurve::through(degree, &pts)
                    };
                    let (tr, _) = create_curve(g, &a.string("name", "polyToCurve1"), c, true)?;
                    result = Some(tr);
                }
                match result {
                    Some(tr) => Ok(json!([g.path(tr)])),
                    None => fail!("polyToCurve: select edges of a mesh"),
                }
            },
        )
        .flag(f("degree", "d", ArgType::Int, "Degree."))
        .flag(f("form", "f", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the curve.")),
    );

    c.register(surf_flags(
        Cmd::new(
            "extrude",
            "Sweep a profile curve along a path curve (or -d -l a direction and length): extrude profile path -et 2 -sc 1 -ro 0. Returns \
             [extrudedSurface1, extrude1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                if names.is_empty() {
                    fail!("extrude: name or select a profile and a path");
                }
                let inputs: Vec<NodeId> = names.iter().map(|n| curve_arg(g, n, "extrude")).collect::<CmdResult<_>>()?;
                let set = vec![
                    (S_P, PlugValue::Float(a.float("scale", 1.0))),
                    (S_P + 1, PlugValue::Float(a.float("rotation", 0.0))),
                    (S_P + 2, PlugValue::Int(a.int("extrudeType", 2))),
                    (S_P + 3, PlugValue::Int(a.int("divisions", 8))),
                    (S_P + 4, PlugValue::Vec3(v3(a, "direction", DVec3::Y))),
                    (S_P + 5, PlugValue::Float(a.float("length", 1.0))),
                    (S_P + 6, PlugValue::Bool(a.bool("fixedPath"))),
                ];
                let (tr, node) = surf_output(g, "extrude", &a.string("name", "extrudedSurface1"), "extrude1", &inputs, &set, a.int("polygon", 0) == 1)?;
                finish_surf(g, a, tr, node)
            },
        )
        .positional("the profile, then the path (the selection when none)")
        .flag(f("extrudeType", "et", ArgType::Int, "0 distance, 1 flat, 2 tube (2)."))
        .flag(f("scale", "sc", ArgType::Float, "Scale at the path's end."))
        .flag(f("rotation", "ro", ArgType::Float, "Twist at the path's end, degrees."))
        .flag(f("direction", "d", ArgType::Vec3, "Direction, with no path."))
        .flag(f("length", "l", ArgType::Float, "Length, with no path."))
        .flag(f("divisions", "dv", ArgType::Int, "Mesh points per span of the profile."))
        .flag(f("fixedPath", "fpt", ArgType::Bool, "The surface sits on the path, not where the profile is."))
        .flag(f("useComponentPivot", "ucp", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("useProfileNormal", "upn", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("reverseSurfaceIfPathReversed", "rsp", ArgType::Bool, "Accepted for existing scripts.")),
    ));

    c.register(surf_flags(
        Cmd::new(
            "revolve",
            "Turn a curve about an axis: revolve curve1 -ax 0 1 0 -p 0 0 0 -s 8 -ssw 0 -esw 360. Returns [revolvedSurface1, revolve1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let Some(n) = names.first() else { fail!("revolve: name or select a curve") };
                let shape = curve_arg(g, n, "revolve")?;
                let set = vec![
                    (S_P, PlugValue::Vec3(v3(a, "axis", DVec3::Y))),
                    (S_P + 1, PlugValue::Vec3(v3(a, "pivot", DVec3::ZERO))),
                    (S_P + 2, PlugValue::Int(a.int("sections", 8))),
                    (S_P + 3, PlugValue::Int(a.int("divisions", 4))),
                    (S_P + 4, PlugValue::Float(a.float("startSweep", 0.0))),
                    (S_P + 5, PlugValue::Float(a.float("endSweep", 360.0))),
                    (S_P + 6, PlugValue::Int(a.int("degree", 3))),
                ];
                let (tr, node) = surf_output(g, "revolve", &a.string("name", "revolvedSurface1"), "revolve1", &[shape], &set, a.int("polygon", 0) == 1)?;
                finish_surf(g, a, tr, node)
            },
        )
        .positional("a curve (the selection when none)")
        .flag(f("axis", "ax", ArgType::Vec3, "The axis."))
        .flag(f("pivot", "p", ArgType::Vec3, "A point on the axis."))
        .flag(f("sections", "s", ArgType::Int, "Sections (8)."))
        .flag(f("divisions", "dv", ArgType::Int, "Mesh points per span of the curve."))
        .flag(f("startSweep", "ssw", ArgType::Float, "Start, degrees."))
        .flag(f("endSweep", "esw", ArgType::Float, "End, degrees."))
        .flag(f("degree", "d", ArgType::Int, "Degree around (3)."))
        .flag(f("useTolerance", "ut", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts.")),
    ));

    c.register(surf_flags(
        Cmd::new(
            "loft",
            "A surface through curves in order: loft c1 c2 c3 -c 0. Returns [loftedSurface1, loft1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                if names.len() < 2 {
                    fail!("loft: name or select two curves or more");
                }
                let inputs: Vec<NodeId> = names.iter().map(|n| curve_arg(g, n, "loft")).collect::<CmdResult<_>>()?;
                let set = vec![
                    (S_P, PlugValue::Bool(a.bool("close"))),
                    (S_P + 1, PlugValue::Int(a.int("divisions", 8))),
                    (S_P + 2, PlugValue::Bool(a.bool("uniform"))),
                    (S_P + 3, PlugValue::Int(a.int("degree", 3))),
                    (S_P + 4, PlugValue::Int(a.int("sectionSpans", 1))),
                    (S_P + 5, PlugValue::Bool(a.get("autoReverse").and_then(Value::as_bool).unwrap_or(true))),
                    (S_P + 6, PlugValue::Bool(a.bool("reverseSurfaceNormals"))),
                ];
                let (tr, node) = surf_output(g, "loft", &a.string("name", "loftedSurface1"), "loft1", &inputs, &set, a.int("polygon", 0) == 1)?;
                finish_surf(g, a, tr, node)
            },
        )
        .positional("curves in order (the selection when none)")
        .flag(f("close", "c", ArgType::Bool, "Back to the first curve."))
        .flag(f("divisions", "dv", ArgType::Int, "Mesh points per span of the first curve."))
        .flag(f("uniform", "u", ArgType::Bool, "Even parameters between the curves, rather than by their distance."))
        .flag(f("autoReverse", "ar", ArgType::Bool, "Turn curves round to run alike (on)."))
        .flag(f("degree", "d", ArgType::Int, "Degree across the curves, 1 to 3 (3)."))
        .flag(f("sectionSpans", "ss", ArgType::Int, "Spans between each pair of curves (1)."))
        .flag(f("reverseSurfaceNormals", "rsn", ArgType::Bool, "Flip the normals.")),
    ));

    c.register(surf_flags(
        Cmd::new(
            "planar",
            "A surface filling a closed curve: a rectangle makes a plain patch, anything else a plane trimmed to it; -po 1 a polygon \
             face. Returns [planarTrimmedSurface1, planarTrim1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let Some(n) = names.first() else { fail!("planar: name or select a closed curve") };
                let shape = curve_arg(g, n, "planar")?;
                let polygon = a.int("polygon", 0) == 1;
                let (tr, node) = surf_output(g, "planarTrim", &a.string("name", "planarTrimmedSurface1"), "planarTrim1", &[shape], &[], polygon)?;
                finish_surf(g, a, tr, node)
            },
        )
        .positional("a closed curve (the selection when none)")
        .flag(f("degree", "d", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts.")),
    ));

    c.register(surf_flags(
        Cmd::new(
            "sweepMeshFromCurve",
            "A tube along a curve with a sweepMeshCreator node: -scaleProfileX radius, -profilePolySides, -interpolationSteps, -taper, \
             -twist. Returns [sweep1, sweepMeshCreator1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let Some(n) = names.first() else { fail!("sweepMeshFromCurve: name or select a curve") };
                let shape = curve_arg(g, n, "sweepMeshFromCurve")?;
                let set = vec![
                    (S_P, PlugValue::Float(a.float("scaleProfileX", 0.1))),
                    (S_P + 1, PlugValue::Int(a.int("profilePolySides", 8))),
                    (S_P + 2, PlugValue::Int(a.int("interpolationSteps", 8))),
                    (S_P + 3, PlugValue::Float(a.float("taper", 1.0))),
                    (S_P + 4, PlugValue::Float(a.float("twist", 0.0))),
                ];
                let (tr, node) = surf_output(g, "sweepMeshCreator", &a.string("name", "sweep1"), "sweepMeshCreator1", &[shape], &set, true)?;
                finish_surf(g, a, tr, node)
            },
        )
        .positional("a curve (the selection when none)")
        .flag(f("scaleProfileX", "spx", ArgType::Float, "Radius of the profile."))
        .flag(f("profilePolySides", "pps", ArgType::Int, "Sides of the profile."))
        .flag(f("interpolationSteps", "is", ArgType::Int, "Rings per span."))
        .flag(f("taper", "tp", ArgType::Float, "Radius scale at the end."))
        .flag(f("twist", "tw", ArgType::Float, "Twist at the end, degrees.")),
    ));
}
