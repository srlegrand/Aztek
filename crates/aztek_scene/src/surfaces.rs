//! NURBS surfaces: the nurbsSurface shape with its CVs as components (`surface1.cv[u][v]`), the primitives' make* nodes,
//! the surface edit commands, each a history node as in the reference, the info nodes, and nurbsToPoly's nurbsTessellate node.
//! The shape outputs a tessellated mesh and isoparm lines for the viewport. Trims travel on the surface itself; the commands
//! that make them are in nurbs_ops.rs.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::nurbs::{self, Form, MeshOptions, NurbsCurve, NurbsSurface, RebuildDir};
use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::{DMat4, DQuat, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::mesh_attr;
use crate::components::{self, ComponentMode, PointProvider};
use crate::curves::{f, f_in, float_out, int_in, int_out, store_bool, v_in};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugType, PlugValue, NO_NODE};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::transform::{DagNode, Shape};

// ---------------------------------------------------------------------------------------------------------------------
// Plug data

/// A surface on a plug.
#[derive(Debug, Clone, Default)]
pub struct SurfaceData(pub NurbsSurface);

impl crate::plug::Data for SurfaceData {
    fn kind(&self) -> &str {
        "nurbsSurfaceData"
    }
    fn to_json(&self) -> Value {
        let s = &self.0;
        json!({"kind": "nurbsSurfaceData", "degreeU": s.degree_u, "degreeV": s.degree_v, "formU": s.form_u.index(), "formV": s.form_v.index(),
               "nu": s.nu, "nv": s.nv, "cvs": s.cvs.iter().flat_map(|p| [p.x, p.y, p.z]).collect::<Vec<f64>>(),
               "knotsU": s.knots_u, "knotsV": s.knots_v, "trim": trim_to_json(&s.trim)})
    }
    fn bytes(&self) -> usize {
        self.0.cvs.len() * 24 + (self.0.knots_u.len() + self.0.knots_v.len()) * 8
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn surface_from_json(j: &Value) -> NurbsSurface {
    let nums = |k: &str| -> Vec<f64> { j.get(k).and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default() };
    let int = |k: &str| j.get(k).and_then(Value::as_u64).unwrap_or(0) as usize;
    NurbsSurface {
        degree_u: int("degreeU"),
        degree_v: int("degreeV"),
        form_u: Form::from_index(j.get("formU").and_then(Value::as_i64).unwrap_or(0)),
        form_v: Form::from_index(j.get("formV").and_then(Value::as_i64).unwrap_or(0)),
        nu: int("nu"),
        nv: int("nv"),
        cvs: nums("cvs").chunks(3).filter(|c| c.len() == 3).map(|c| DVec3::new(c[0], c[1], c[2])).collect(),
        knots_u: nums("knotsU"),
        knots_v: nums("knotsV"),
        trim: trim_from_json(j.get("trim").unwrap_or(&Value::Null)),
    }
}

pub(crate) fn trim_to_json(trim: &[nurbs::TrimRegion]) -> Value {
    Value::Array(trim.iter().map(|r| json!({"keepOdd": r.keep_odd, "loops": r.loops.iter().map(|l| l.iter().flat_map(|p| [p.x, p.y]).collect::<Vec<f64>>()).collect::<Vec<_>>()})).collect())
}

pub(crate) fn trim_from_json(j: &Value) -> Vec<nurbs::TrimRegion> {
    j.as_array()
        .into_iter()
        .flatten()
        .map(|r| nurbs::TrimRegion {
            keep_odd: r.get("keepOdd").and_then(Value::as_bool).unwrap_or(true),
            loops: r
                .get("loops")
                .and_then(Value::as_array)
                .into_iter()
                .flatten()
                .map(|l| l.as_array().map(|a| a.chunks(2).filter(|c| c.len() == 2).filter_map(|c| Some(glam::DVec2::new(c[0].as_f64()?, c[1].as_f64()?))).collect()).unwrap_or_default())
                .collect(),
        })
        .collect()
}

pub(crate) fn surface_value(v: &PlugValue) -> Option<&NurbsSurface> {
    match v {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<SurfaceData>().map(|c| &c.0),
        _ => None,
    }
}

pub fn to_value(s: NurbsSurface) -> PlugValue {
    PlugValue::Data(Some(Arc::new(SurfaceData(s))))
}

/// World-space isoparm segments on a plug, for drawing; never saved.
#[derive(Debug, Clone, Default)]
struct WireData(Vec<(DVec3, DVec3)>);

impl crate::plug::Data for WireData {
    fn kind(&self) -> &str {
        "nurbsSurfaceWires"
    }
    fn to_json(&self) -> Value {
        Value::Null
    }
    fn bytes(&self) -> usize {
        self.0.len() * 48
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn surface_attr(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: fl, doc: doc.into(), ..Default::default() }
}

fn stored_int(name: &str, short: &str, def: i64, doc: &str) -> AttrDef {
    AttrDef { flags: flags::INPUT | flags::STORABLE, ..int_in(name, short, def, doc) }
}

// ---------------------------------------------------------------------------------------------------------------------
// The shape

pub const SHAPE_TYPE: &str = "nurbsSurface";

/// Keeps the tessellation between evaluations, so moving the transform doesn't rebuild it.
#[derive(Default)]
pub struct SurfaceShape {
    cached: Option<(NurbsSurface, [i64; 4])>,
    mesh: Option<Arc<Mesh>>,
    lines: Vec<Vec<DVec3>>,
}

impl SurfaceShape {
    pub const CREATE: usize = Shape::COUNT;
    pub const CACHED: usize = Shape::COUNT + 1;
    pub const TWEAK: usize = Shape::COUNT + 2;
    pub const LOCAL: usize = Shape::COUNT + 3;
    pub const WORLD_SPACE: usize = Shape::COUNT + 4;
    pub const OUT_MESH: usize = Shape::COUNT + 5;
    pub const WIRES: usize = Shape::COUNT + 6;
    pub const DEGREE_U: usize = Shape::COUNT + 7;
    pub const DEGREE_V: usize = Shape::COUNT + 8;
    pub const SPANS_U: usize = Shape::COUNT + 9;
    pub const SPANS_V: usize = Shape::COUNT + 10;
    pub const FORM_U: usize = Shape::COUNT + 11;
    pub const FORM_V: usize = Shape::COUNT + 12;
    pub const MIN_U: usize = Shape::COUNT + 13;
    pub const MAX_U: usize = Shape::COUNT + 14;
    pub const MIN_V: usize = Shape::COUNT + 15;
    pub const MAX_V: usize = Shape::COUNT + 16;
    pub const CURVE_PRECISION: usize = Shape::COUNT + 17;
    pub const CURVE_PRECISION_SHADED: usize = Shape::COUNT + 18;
    pub const DIVISIONS_U: usize = Shape::COUNT + 19;
    pub const DIVISIONS_V: usize = Shape::COUNT + 20;
    pub const DISP_CV: usize = Shape::COUNT + 21;
    pub const DISP_HULL: usize = Shape::COUNT + 22;
    pub const DISP_EP: usize = Shape::COUNT + 23;
    pub const DISP_GEOMETRY: usize = Shape::COUNT + 24;
}

impl NodeBehavior for SurfaceShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let base = surface_value(d.value(Self::CREATE)).or(surface_value(d.value(Self::CACHED))).cloned();
        let Some(mut s) = base else {
            for p in [Self::LOCAL, Self::WORLD_SPACE, Self::WIRES] {
                d.set_output(p, PlugValue::Data(None));
            }
            d.set_output(Self::OUT_MESH, PlugValue::Mesh(None));
            self.cached = None;
            return;
        };
        if let PlugValue::Data(Some(t)) = d.value(Self::TWEAK) {
            if let Some(t) = t.as_any().downcast_ref::<crate::rig::JsonData>() {
                for e in t.0.as_array().into_iter().flatten() {
                    let index = |k: usize| e.get(k).and_then(Value::as_u64).unwrap_or(u64::MAX) as usize;
                    let x = |k: usize| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                    let (u, v) = (index(0), index(1));
                    if u < s.nu && v < s.nv {
                        s.cvs[u * s.nv + v] += DVec3::new(x(2), x(3), x(4));
                    }
                }
                s.rewrap();
            }
        }
        let display = [d.int(Self::DIVISIONS_U), d.int(Self::DIVISIONS_V), d.int(Self::CURVE_PRECISION), d.int(Self::CURVE_PRECISION_SHADED)];
        if self.cached.as_ref().is_none_or(|(c, k)| *c != s || *k != display) {
            let [du, dv, wire, shaded] = display;
            let steps = |div: i64| ((div.max(0) + 1) * shaded.max(1) * 4).clamp(1, 32) as usize;
            self.mesh = Some(Arc::new(s.tessellate(steps(du), steps(dv))));
            self.lines = s.isoparm_lines(du.max(0) as usize, dv.max(0) as usize, wire.max(1) as usize);
            self.cached = Some((s.clone(), display));
        }
        let world = d.matrix(DagNode::WORLD_MATRIX);
        let mut segs = Vec::new();
        for line in &self.lines {
            segs.extend(line.windows(2).map(|w| (world.transform_point3(w[0]), world.transform_point3(w[1]))));
        }
        d.set_output(Self::DEGREE_U, PlugValue::Int(s.degree_u as i64));
        d.set_output(Self::DEGREE_V, PlugValue::Int(s.degree_v as i64));
        d.set_output(Self::SPANS_U, PlugValue::Int(s.spans_u() as i64));
        d.set_output(Self::SPANS_V, PlugValue::Int(s.spans_v() as i64));
        d.set_output(Self::FORM_U, PlugValue::Int(s.form_u.index()));
        d.set_output(Self::FORM_V, PlugValue::Int(s.form_v.index()));
        let (ulo, uhi) = s.domain_u();
        let (vlo, vhi) = s.domain_v();
        d.set_output(Self::MIN_U, PlugValue::Float(ulo));
        d.set_output(Self::MAX_U, PlugValue::Float(uhi));
        d.set_output(Self::MIN_V, PlugValue::Float(vlo));
        d.set_output(Self::MAX_V, PlugValue::Float(vhi));
        d.set_output(Self::OUT_MESH, PlugValue::Mesh(self.mesh.clone()));
        d.set_output(Self::WIRES, PlugValue::Data(Some(Arc::new(WireData(segs)))));
        d.set_output(Self::WORLD_SPACE, to_value(s.transformed(&world)));
        d.set_output(Self::LOCAL, to_value(s));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn as_surface(g: &Graph, id: NodeId) -> Option<NodeId> {
    crate::dag::shape_of_type(g, id, SHAPE_TYPE)
}

pub fn is_surface(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a(SHAPE_TYPE))
}

/// The evaluated surface of a shape, object space.
pub fn local_surface(g: &mut Graph, shape: NodeId) -> Option<NurbsSurface> {
    g.evaluate();
    g.node(shape).and_then(|n| surface_value(n.data().value(SurfaceShape::LOCAL)).cloned())
}

/// The evaluated surface of a shape, world space.
pub fn world_surface(g: &mut Graph, shape: NodeId) -> Option<NurbsSurface> {
    g.evaluate();
    g.node(shape).and_then(|n| surface_value(n.data().value(SurfaceShape::WORLD_SPACE)).cloned())
}

/// The surface as last evaluated, world space, for drawing.
pub fn drawn_surface(n: &crate::node::Node) -> Option<&NurbsSurface> {
    surface_value(n.data().value(SurfaceShape::WORLD_SPACE))
}

/// The isoparm segments as last evaluated, world space.
pub fn wire_lines(n: &crate::node::Node) -> &[(DVec3, DVec3)] {
    match n.data().value(SurfaceShape::WIRES) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<WireData>().map(|w| w.0.as_slice()).unwrap_or(&[]),
        _ => &[],
    }
}

/// The CVs of one lap each way in world space, by component id, with the V count an id divides by.
pub fn drawn_cvs(n: &crate::node::Node) -> Option<(Vec<DVec3>, usize)> {
    let s = drawn_surface(n)?;
    let (du, dv) = (s.distinct_u(), s.distinct_v());
    Some(((0..du).flat_map(|u| (0..dv).map(move |v| s.cv(u, v))).collect(), dv))
}

pub(crate) fn set_cached(g: &mut Graph, shape: NodeId, s: NurbsSurface) -> CmdResult<()> {
    g.set_value(plug(shape, SurfaceShape::CACHED), to_value(s))?;
    Ok(())
}

/// How many CVs each way the shape's components count, as last evaluated.
fn counts(g: &Graph, shape: NodeId) -> (u32, u32) {
    g.node(shape).and_then(drawn_surface).map(|s| (s.distinct_u() as u32, s.distinct_v() as u32)).unwrap_or((0, 0))
}

/// The V count of a surface node's component ids; None for anything else.
pub fn id_stride(g: &Graph, node: NodeId) -> Option<u32> {
    is_surface(g, node).then(|| counts(g, node).1.max(1))
}

fn index_range(part: &str, count: u32) -> Option<std::ops::RangeInclusive<u32>> {
    if part == "*" {
        return (count > 0).then(|| 0..=count - 1);
    }
    match part.split_once(':') {
        Some((a, b)) => Some(a.parse().ok()?..=b.parse().ok()?),
        None => {
            let i: u32 = part.parse().ok()?;
            Some(i..=i)
        }
    }
}

/// Component ids from what's between the outer brackets of `cv[...]`: `1][2`, `0:2][*`; `*` or one index alone means
/// whole rows along U. Out of range indices are left out. None when it doesn't parse.
pub fn parse_cv_ranges(g: &Graph, shape: NodeId, inner: &str) -> Option<Vec<u32>> {
    let (nu, nv) = counts(g, shape);
    let (a, b) = inner.split_once("][").unwrap_or((inner, "*"));
    let (us, vs) = (index_range(a, nu), index_range(b, nv));
    let (us, vs) = if nu == 0 || nv == 0 { (us.or(Some(1..=0))?, vs.or(Some(1..=0))?) } else { (us?, vs?) };
    let mut out = Vec::new();
    for u in us {
        for v in vs.clone() {
            if u < nu && v < nv {
                out.push(u * nv + v);
            }
        }
    }
    Some(out)
}

/// One `cv[u][v]` per component.
pub fn format_cvs_flat(ids: &[u32], stride: u32) -> Vec<String> {
    let mut v = ids.to_vec();
    v.sort_unstable();
    v.dedup();
    v.into_iter().map(|i| format!("cv[{}][{}]", i / stride, i % stride)).collect()
}

/// Components folded into `cv[a:b][c:d]` blocks.
pub fn format_cvs(ids: &[u32], stride: u32) -> Vec<String> {
    let mut sorted = ids.to_vec();
    sorted.sort_unstable();
    sorted.dedup();
    // The runs along V within each U, in order.
    let mut runs: Vec<(u32, u32, u32)> = Vec::new();
    for i in sorted {
        let (u, v) = (i / stride, i % stride);
        match runs.last_mut() {
            Some(r) if r.0 == u && r.2 + 1 == v => r.2 = v,
            _ => runs.push((u, v, v)),
        }
    }
    let mut blocks: Vec<(u32, u32, u32, u32)> = Vec::new();
    for (u, a, b) in runs {
        if let Some(block) = blocks.iter_mut().find(|k| k.1 + 1 == u && k.2 == a && k.3 == b) {
            block.1 = u;
        } else {
            blocks.push((u, u, a, b));
        }
    }
    let span = |a: u32, b: u32| if a == b { a.to_string() } else { format!("{a}:{b}") };
    blocks.into_iter().map(|(u0, u1, a, b)| format!("cv[{}][{}]", span(u0, u1), span(a, b))).collect()
}

/// Points moved in object space: into the shape's own surface, or into its tweaks when history feeds it.
fn move_cvs(g: &mut Graph, shape: NodeId, moves: &[(u32, DVec3)]) {
    let Some(mut cur) = local_surface(g, shape) else { return };
    let stride = cur.distinct_v().max(1) as u32;
    for &(id, p) in moves {
        let (u, v) = ((id / stride) as usize, (id % stride) as usize);
        if u < cur.nu && v < cur.nv {
            cur.cvs[u * cur.nv + v] = p;
        }
    }
    cur.rewrap();
    if g.source(&plug(shape, SurfaceShape::CREATE)).is_none() {
        let _ = set_cached(g, shape, cur);
        return;
    }
    // Tweaks: offsets from what history gives.
    let upstream = g.node(shape).and_then(|n| surface_value(n.data().value(SurfaceShape::CREATE)).cloned()).unwrap_or_default();
    let mut list = Vec::new();
    for u in 0..cur.distinct_u() {
        for v in 0..cur.distinct_v() {
            let at = u * cur.nv + v;
            let d = cur.cvs[at] - upstream.cvs.get(at).copied().unwrap_or(cur.cvs[at]);
            if d.length_squared() > 1e-18 {
                list.push(json!([u, v, d.x, d.y, d.z]));
            }
        }
    }
    let _ = crate::rig::set_json(g, shape, SurfaceShape::TWEAK, json!(list));
}

fn register_provider() {
    components::register_point_provider(PointProvider {
        node_type: SHAPE_TYPE.into(),
        points: Some(Arc::new(|g: &mut Graph, node: NodeId| {
            let Some(s) = local_surface(g, node) else { return Vec::new() };
            let (du, dv) = (s.distinct_u(), s.distinct_v());
            let mut out: Vec<(u32, DVec3)> = (0..du).flat_map(|u| (0..dv).map(move |v| (u, v))).map(|(u, v)| ((u * dv + v) as u32, s.cv(u, v))).collect();
            out.extend(crate::nurbs_components::surface_edit_points(&s));
            out
        })),
        set_points: Some(Arc::new(|g: &mut Graph, node: NodeId, pts: &[(u32, DVec3)]| {
            let Some(s) = local_surface(g, node) else { return };
            let moves = crate::nurbs_components::surface_moves(&s, pts);
            move_cvs(g, node, &moves)
        })),
        edit_plug: None,
        frames: None,
        bake: Some(Arc::new(|g: &mut Graph, node: NodeId, m: &DMat4| {
            if let Some(s) = local_surface(g, node) {
                let _ = set_cached(g, node, s.transformed(m));
                let _ = crate::rig::set_json(g, node, SurfaceShape::TWEAK, Value::Null);
                g.disconnect(&plug(node, SurfaceShape::CREATE));
                let _ = g.set_value(plug(node, SurfaceShape::CREATE), PlugValue::Data(None));
            }
        })),
        soft: None,
        grid: None,
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// Surfaces from curves. curves.rs's nodes call these for their NURBS output.

/// The reference's loft: U across the sections, V along them.
#[derive(Clone, Copy)]
pub(crate) struct LoftOptions {
    pub close: bool,
    pub uniform: bool,
    pub degree: usize,
    pub section_spans: usize,
    pub auto_reverse: bool,
    pub reverse_normals: bool,
}

pub(crate) fn loft_surface(curves: &[NurbsCurve], o: LoftOptions) -> Option<NurbsSurface> {
    let n = curves.len();
    if n < 2 || curves.iter().any(|c| !c.is_valid()) {
        return None;
    }
    let mut list = curves.to_vec();
    if o.auto_reverse {
        for i in 1..n {
            if list[i].form != Form::Open || list[i - 1].form != Form::Open {
                continue;
            }
            let ends = |c: &NurbsCurve| (c.eval(c.domain().0), c.eval(c.domain().1));
            let (a0, a1) = ends(&list[i - 1]);
            let (b0, b1) = ends(&list[i]);
            if (b0 - a1).length() + (b1 - a0).length() < (b0 - a0).length() + (b1 - a1).length() {
                list[i] = list[i].reversed();
            }
        }
    }
    let sections = nurbs::compatible_curves(&list);
    let first = sections.first()?.clone();
    let mut ts = vec![0.0];
    for k in 1..n {
        let step = if o.uniform {
            1.0
        } else {
            let (a, b) = (&sections[k - 1], &sections[k]);
            a.cvs.iter().zip(&b.cvs).map(|(p, q)| (*p - *q).length()).sum::<f64>() / a.cvs.len().max(1) as f64
        };
        ts.push(ts[k - 1] + step.max(1e-9));
    }
    if !o.uniform {
        let scale = (n - 1) as f64 / ts[n - 1];
        ts.iter_mut().for_each(|t| *t *= scale);
    }
    // Two sections have nothing to curve between.
    let degree = if n == 2 && !o.close { 1 } else { o.degree.clamp(1, 3) };
    let ss = o.section_spans.max(1);
    let mut cols = Vec::with_capacity(first.cvs.len());
    for j in 0..first.cvs.len() {
        let pts: Vec<DVec3> = sections.iter().map(|c| c.cvs[j]).collect();
        let mut col = if o.close {
            NurbsCurve::interpolated_periodic(degree, &pts)
        } else {
            let (d0, d1) = end_derivatives(&pts, &ts);
            NurbsCurve::interpolated(degree, &pts, &ts, d0, d1)
        };
        let extra: Vec<f64> = if o.close {
            (0..n).flat_map(|k| (1..ss).map(move |i| k as f64 + i as f64 / ss as f64)).collect()
        } else {
            ts.windows(2).flat_map(|w| { let (a, b) = (w[0], w[1]); (1..ss).map(move |i| a + (b - a) * i as f64 / ss as f64) }).collect()
        };
        for t in extra {
            col = col.insert_knot_any(t);
        }
        cols.push(col);
    }
    let s = NurbsSurface::from_columns(&cols, first.degree, first.form, first.knots.clone());
    Some(if o.reverse_normals { s.reversed_v() } else { s })
}

/// Derivatives at the ends of a parameterised run of points, from the parabola through the last three.
fn end_derivatives(p: &[DVec3], t: &[f64]) -> (DVec3, DVec3) {
    let n = p.len();
    if n < 3 {
        let d = (p[n - 1] - p[0]) / (t[n - 1] - t[0]).max(1e-12);
        return (d, d);
    }
    let edge = |a: DVec3, b: DVec3, c: DVec3, h1: f64, h2: f64| (b - a) / h1 * (1.0 + h1 / (h1 + h2)) - (c - b) / h2 * (h1 / (h1 + h2));
    let d0 = edge(p[0], p[1], p[2], t[1] - t[0], t[2] - t[1]);
    let d1 = edge(p[n - 1], p[n - 2], p[n - 3], t[n - 1] - t[n - 2], t[n - 2] - t[n - 3]);
    (d0, -d1)
}

#[derive(Clone, Copy)]
pub(crate) struct ExtrudeOptions {
    pub extrude_type: i64,
    pub direction: DVec3,
    pub length: f64,
    pub scale: f64,
    pub rotation: f64,
    pub fixed_path: bool,
    pub profile_normal: bool,
}

/// The reference's extrude: U along the profile, V along the path. Distance moves it straight; flat keeps the profile's
/// orientation along the path; tube turns it with the path.
pub(crate) fn extrude_surface(profile: &NurbsCurve, path: Option<&NurbsCurve>, o: ExtrudeOptions) -> Option<NurbsSurface> {
    if !profile.is_valid() {
        return None;
    }
    let lap = profile.distinct_cvs();
    let center = lap.iter().copied().sum::<DVec3>() / lap.len().max(1) as f64;
    let straight = |off: DVec3| NurbsCurve::open(1, vec![DVec3::ZERO, off]);
    let (way, tube) = match path {
        None => (straight(o.direction.normalize_or_zero() * o.length), false),
        Some(p) if !p.is_valid() => return None,
        Some(p) if o.extrude_type == 0 => {
            let (lo, hi) = p.domain();
            (straight(p.eval(hi) - p.eval(lo)), false)
        }
        Some(p) => (p.clone(), o.extrude_type == 2),
    };
    let (lo, hi) = way.domain();
    let d = way.degree;
    let params: Vec<f64> = (0..way.cvs.len()).map(|j| (way.knots[j..j + d].iter().sum::<f64>() / d as f64).clamp(lo, hi)).collect();
    // The path's turning, carried along from its start without twist.
    let start_t = way.tangent(lo).normalize_or_zero();
    let mut last = start_t;
    let mut turn = DQuat::IDENTITY;
    let mut turns = Vec::with_capacity(params.len());
    for &t in &params {
        if tube {
            let now = way.tangent(t).normalize_or_zero();
            if now != DVec3::ZERO && last != DVec3::ZERO {
                turn = DQuat::from_rotation_arc(last, now) * turn;
                last = now;
            }
        }
        turns.push(turn);
    }
    let axis = if start_t == DVec3::ZERO { DVec3::Z } else { start_t };
    let q0 = way.cvs[0];
    // The profile turned so its plane is square to the path's start.
    let lift = if o.profile_normal && lap.len() > 2 {
        let mut n = DVec3::ZERO;
        for k in 0..lap.len() {
            n += (lap[k] - center).cross(lap[(k + 1) % lap.len()] - center);
        }
        let n = n.normalize_or_zero();
        let n = if n.dot(axis) < 0.0 { -n } else { n };
        if n == DVec3::ZERO { DQuat::IDENTITY } else { DQuat::from_rotation_arc(n, axis) }
    } else {
        DQuat::IDENTITY
    };
    Some(NurbsSurface::from_curves(profile, &way, |i, j| {
        let s = ((params[j] - lo) / (hi - lo).max(1e-12)).clamp(0.0, 1.0);
        let local = DQuat::from_axis_angle(axis, o.rotation.to_radians() * s) * (lift * (profile.cvs[i] - center) * (1.0 + (o.scale - 1.0) * s));
        let base = if o.fixed_path && path.is_some() { way.cvs[j] } else { center + way.cvs[j] - q0 };
        base + turns[j] * local
    }))
}

/// A surface for a closed degree 1 curve of four corners at right angles; None for anything else.
pub(crate) fn rectangle_surface(c: &NurbsCurve) -> Option<NurbsSurface> {
    if c.degree != 1 {
        return None;
    }
    let mut p = c.distinct_cvs().to_vec();
    if p.len() == 5 && (p[0] - p[4]).length() < 1e-9 {
        p.pop();
    }
    if p.len() != 4 || (p[0] - p[3] + p[2] - p[1]).length() > 1e-6 {
        return None;
    }
    let (a, b) = (p[1] - p[0], p[3] - p[0]);
    if a.length() < 1e-9 || b.length() < 1e-9 || a.dot(b).abs() > 1e-6 * a.length() * b.length() {
        return None;
    }
    Some(nurbs::patch_surface(p[0], a, b, 1, 1, 1))
}

/// Coons patch through three or four curves in order round the edge. Three leave one side as a point.
fn boundary_surface(curves: &[NurbsCurve]) -> Option<NurbsSurface> {
    if curves.len() < 3 || curves.iter().any(|c| !c.is_valid()) {
        return None;
    }
    let toward = |c: &NurbsCurve, from: DVec3| {
        let (lo, hi) = c.domain();
        if (c.eval(hi) - from).length() < (c.eval(lo) - from).length() {
            c.reversed()
        } else {
            c.clone()
        }
    };
    let ends = |c: &NurbsCurve| (c.eval(c.domain().0), c.eval(c.domain().1));
    let a = curves[0].clone();
    let (a0, a1) = ends(&a);
    let side1 = toward(&curves[1], a1);
    let (b, side0) = if curves.len() >= 4 {
        let side0 = toward(&curves[3], a0);
        let (_, top) = ends(&side0);
        (toward(&curves[2], top), side0)
    } else {
        let side0 = toward(&curves[2], a0);
        let (_, top) = ends(&side1);
        (NurbsCurve::open(1, vec![top, top]), side0)
    };
    let (ca, cb, cl, cr) = (a, b, side0, side1);
    let unit = |c: &NurbsCurve, t: f64| {
        let (lo, hi) = c.domain();
        c.eval(lo + (hi - lo) * t)
    };
    let (p00, p10, p01, p11) = (unit(&ca, 0.0), unit(&ca, 1.0), unit(&cb, 0.0), unit(&cb, 1.0));
    let plan = |x: &NurbsCurve, y: &NurbsCurve| {
        let degree = x.degree.max(y.degree);
        let spans = x.spans().max(y.spans()).max(1);
        let scale = 1.0 / spans as f64;
        (degree, Form::Open, nurbs::open_knots(spans + degree, degree).into_iter().map(|k| k * scale).collect::<Vec<f64>>())
    };
    Some(NurbsSurface::fitted(plan(&ca, &cb), plan(&cl, &cr), |u, v| {
        let ruled_u = unit(&ca, u).lerp(unit(&cb, u), v);
        let ruled_v = unit(&cl, v).lerp(unit(&cr, v), u);
        let bilinear = p00.lerp(p10, u).lerp(p01.lerp(p11, u), v);
        ruled_u + ruled_v - bilinear
    }))
}

/// One profile moved along two rails, turned and scaled so its ends stay on them. U along the profile, V along the rails.
fn birail_surface(profile: &NurbsCurve, rail0: &NurbsCurve, rail1: &NurbsCurve) -> Option<NurbsSurface> {
    if !profile.is_valid() || !rail0.is_valid() || !rail1.is_valid() {
        return None;
    }
    let (plo, phi) = profile.domain();
    let (p0, p1) = (profile.eval(plo), profile.eval(phi));
    let orient = |r: &NurbsCurve, from: DVec3| {
        let (lo, hi) = r.domain();
        if (r.eval(hi) - from).length() < (r.eval(lo) - from).length() {
            r.reversed()
        } else {
            r.clone()
        }
    };
    let (a, b) = (orient(rail0, p0), orient(rail1, p1));
    let chord = p1 - p0;
    if chord.length() < 1e-9 {
        return None;
    }
    let degree = a.degree.max(b.degree);
    let spans = a.spans().max(b.spans()).max(1);
    let knots = nurbs::open_knots(spans + degree, degree).into_iter().map(|k| k / spans as f64).collect();
    let unit = |c: &NurbsCurve, t: f64| {
        let (lo, hi) = c.domain();
        c.eval(lo + (hi - lo) * t)
    };
    let form = if profile.form == Form::Periodic { Form::Open } else { profile.form };
    let open = profile.clamped();
    Some(NurbsSurface::fitted((open.degree, form, open.knots.clone()), (degree, Form::Open, knots), |u, v| {
        let (pa, pb) = (unit(&a, v), unit(&b, v));
        let span = pb - pa;
        let turn = if span.length() > 1e-9 { DQuat::from_rotation_arc(chord.normalize(), span.normalize()) } else { DQuat::IDENTITY };
        pa + turn * ((open.eval(u) - p0) * (span.length() / chord.length()))
    }))
}

// ---------------------------------------------------------------------------------------------------------------------
// Primitives

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Make {
    Sphere,
    Cylinder,
    Cone,
    Torus,
    Plane,
    Cube,
}

const CUBE_FACES: [&str; 6] = ["top", "bottom", "left", "right", "front", "back"];

struct MakeNode {
    kind: Make,
}

const M_OUT: usize = 0;
const M_AXIS: usize = 1;
const M_PIVOT: usize = 2;
const M_P: usize = 3;

fn degree_of(d: i64) -> usize {
    if d == 1 {
        1
    } else {
        3
    }
}

/// Up the Y axis to the given axis, and over to the pivot.
fn placement(axis: DVec3, pivot: DVec3) -> DMat4 {
    let a = axis.normalize_or_zero();
    let a = if a == DVec3::ZERO { DVec3::Y } else { a };
    DMat4::from_translation(pivot) * DMat4::from_quat(DQuat::from_rotation_arc(DVec3::Y, a))
}

impl NodeBehavior for MakeNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let place = placement(d.vec3(M_AXIS), d.vec3(M_PIVOT));
        let round = |d: &NodeData| (d.float(M_P), d.float(M_P + 1), d.float(M_P + 2), degree_of(d.int(M_P + 3)), d.int(M_P + 4).max(3) as usize, d.int(M_P + 5).max(1) as usize);
        match self.kind {
            Make::Sphere => {
                let (r, ssw, esw, deg, sections, spans) = round(d);
                let s = nurbs::sphere_surface(ssw, esw, sections, spans, deg);
                let m = place * DMat4::from_scale(DVec3::splat(r));
                d.set_output(M_OUT, to_value(s.transformed(&m)));
            }
            Make::Cylinder | Make::Cone => {
                let (r, ssw, esw, deg, sections, spans) = round(d);
                let half = r * d.float(M_P + 8) * 0.5;
                let top = if self.kind == Make::Cone { 0.0 } else { r };
                let profile = NurbsCurve::line(DVec3::new(r, -half, 0.0), DVec3::new(top, half, 0.0), spans, deg);
                let s = nurbs::revolved(&profile, DVec3::Y, DVec3::ZERO, ssw, esw, sections, deg);
                d.set_output(M_OUT, to_value(s.transformed(&place)));
            }
            Make::Torus => {
                let (r, ssw, esw, deg, sections, spans) = round(d);
                let (minor, msw) = (r * d.float(M_P + 8), d.float(M_P + 9));
                let mut profile = nurbs::unit_arc(0.0, msw, spans.max(3), deg);
                for p in profile.cvs.iter_mut() {
                    *p = DVec3::new(r + minor * p.x, minor * p.y, 0.0);
                }
                let s = nurbs::revolved(&profile, DVec3::Y, DVec3::ZERO, ssw, esw, sections, deg);
                d.set_output(M_OUT, to_value(s.transformed(&place)));
            }
            Make::Plane => {
                let (w, lr, deg, pu, pv) = (d.float(M_P), d.float(M_P + 1), d.int(M_P + 2).max(1) as usize, d.int(M_P + 3).max(1) as usize, d.int(M_P + 4).max(1) as usize);
                let l = w * lr;
                let s = nurbs::patch_surface(DVec3::new(-w * 0.5, 0.0, l * 0.5), DVec3::X * w, -DVec3::Z * l, pu, pv, deg);
                d.set_output(M_OUT, to_value(s.transformed(&place)));
            }
            Make::Cube => {
                let (w, lr, hr, deg, pu, pv) = (d.float(M_P), d.float(M_P + 1), d.float(M_P + 2), d.int(M_P + 3).max(1) as usize, d.int(M_P + 4).max(1) as usize, d.int(M_P + 5).max(1) as usize);
                let (x, y, z) = (w * 0.5, w * hr * 0.5, w * lr * 0.5);
                let (dx, dy, dz) = (DVec3::X * w, DVec3::Y * w * hr, DVec3::Z * w * lr);
                // Each patch's U x V faces out.
                let faces = [
                    (DVec3::new(-x, y, z), dx, -dz),
                    (DVec3::new(-x, -y, -z), dx, dz),
                    (DVec3::new(-x, -y, -z), dz, dy),
                    (DVec3::new(x, -y, z), -dz, dy),
                    (DVec3::new(-x, -y, z), dx, dy),
                    (DVec3::new(x, -y, -z), -dx, dy),
                ];
                for (i, (o, a, b)) in faces.into_iter().enumerate() {
                    let s = nurbs::patch_surface(o, a, b, pu, pv, deg);
                    let out = if i == 0 { M_OUT } else { M_P + 5 + i };
                    d.set_output(out, to_value(s.transformed(&place)));
                }
            }
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn make_type(name: &str, kind: Make, doc: &str, params: Vec<AttrDef>) -> NodeType {
    let mut attrs = vec![
        surface_attr("outputSurface", "os", flags::OUTPUT | flags::HIDDEN, "The surface."),
        v_in("axis", "ax", DVec3::X, "The primitive's axis."),
        v_in("pivot", "p", DVec3::ZERO, "Its centre."),
    ];
    attrs.extend(params);
    NodeType { name: name.into(), icon: "chart-line".into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(move || Box::new(MakeNode { kind }))), ..Default::default() }
}

fn round_params(spans: i64) -> Vec<AttrDef> {
    vec![
        f_in("radius", "r", 1.0, "The radius."),
        f_in("startSweep", "ssw", 0.0, "Start, degrees."),
        f_in("endSweep", "esw", 360.0, "End, degrees."),
        int_in("degree", "d", 3, "1 linear, 3 cubic."),
        int_in("sections", "s", 8, "Sections around."),
        int_in("spans", "nsp", spans, "Spans along the axis."),
        store_bool("useTolerance", false, "Accepted for existing scripts: sections are as set."),
        f_in("tolerance", "tol", 0.01, "Accepted for existing scripts."),
    ]
}

// ---------------------------------------------------------------------------------------------------------------------
// Edits

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Op {
    Rebuild,
    Reverse,
    InsertKnot,
    Detach,
    Attach,
    Offset,
    Close,
}

struct OpNode {
    kind: Op,
}

const IN: usize = 0;
const OUT: usize = 1;
const P0: usize = 2;

/// The reference numbers these two ways: detachSurface and insertKnotSurface take 1 for U, 0 for V.
const DIR_U: i64 = 1;

impl NodeBehavior for OpNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(s) = surface_value(d.value(IN)).cloned() else {
            d.set_output(OUT, PlugValue::Data(None));
            return;
        };
        if !s.is_valid() {
            d.set_output(OUT, to_value(s));
            return;
        }
        let (trim, domain) = (s.trim.clone(), (s.domain_u(), s.domain_v()));
        let out = match self.kind {
            Op::Rebuild => {
                let dir = d.int(P0 + 5);
                if d.bool(P0 + 6) {
                    s
                } else {
                    s.rebuilt(
                        RebuildDir { on: dir != 1, spans: d.int(P0).max(0) as usize, degree: d.int(P0 + 2).max(1) as usize, form: s.form_u },
                        RebuildDir { on: dir != 0, spans: d.int(P0 + 1).max(0) as usize, degree: d.int(P0 + 3).max(1) as usize, form: s.form_v },
                        d.int(P0 + 4),
                    )
                }
            }
            Op::Reverse => match d.int(P0) {
                1 => s.reversed_v(),
                2 => s.reversed_u().reversed_v(),
                3 => s.swapped(),
                _ => s.reversed_u(),
            },
            Op::InsertKnot => {
                let (p, along_u, count, add) = (d.float(P0), d.int(P0 + 1) == DIR_U, d.int(P0 + 2).max(1), d.bool(P0 + 3));
                let knots = if along_u { &s.knots_u } else { &s.knots_v };
                let have = knots.iter().filter(|k| (**k - p).abs() < 1e-9).count() as i64;
                let times = if add { count } else { (count - have).max(0) };
                let mut r = s;
                for _ in 0..times {
                    r = if along_u { r.insert_knot_u(p) } else { r.insert_knot_v(p) };
                }
                r
            }
            Op::Detach => {
                let (a, b) = if d.int(P0 + 1) == DIR_U { s.split_u(d.float(P0)) } else { s.split_v(d.float(P0)) };
                d.set_output(P0 + 2, to_value(b));
                a
            }
            Op::Attach => match surface_value(d.value(P0 + 3)).cloned() {
                Some(o) if o.is_valid() => {
                    let (blend, bias) = (d.int(P0) == 1, d.float(P0 + 1));
                    if d.int(P0 + 2) == DIR_U {
                        s.attached_u(&o, blend, bias)
                    } else {
                        s.attached_v(&o, blend, bias)
                    }
                }
                _ => s,
            },
            Op::Offset => s.offset(d.float(P0)),
            Op::Close => match d.int(P0) {
                1 => s.closed_v(),
                2 => s.closed_u().closed_v(),
                _ => s.closed_u(),
            },
        };
        // A trim stays with the parameters it was made in.
        let mut out = out;
        if out.trim.is_empty() && (out.domain_u(), out.domain_v()) == domain {
            out.trim = trim;
        }
        d.set_output(OUT, to_value(out));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn op_type(name: &str, kind: Op, doc: &str, params: Vec<AttrDef>) -> NodeType {
    let mut attrs = vec![
        surface_attr("inputSurface", "is", flags::INPUT | flags::STORABLE | flags::HIDDEN, "The surface changed."),
        surface_attr("outputSurface", "os", flags::OUTPUT | flags::HIDDEN, "The result."),
    ];
    attrs.extend(params);
    NodeType { name: name.into(), icon: "chart-line".into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(move || Box::new(OpNode { kind }))), ..Default::default() }
}

/// boundary, square and the birail: a surface from curves.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FromCurves {
    Coons,
    Birail,
}

struct FromCurvesNode {
    kind: FromCurves,
}

impl NodeBehavior for FromCurvesNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let curves: Vec<NurbsCurve> = d.elements(0).iter().filter_map(|(_, v)| crate::curves::curve_value(v).cloned()).collect();
        let s = match self.kind {
            FromCurves::Coons => boundary_surface(&curves),
            FromCurves::Birail if curves.len() >= 3 => birail_surface(&curves[0], &curves[1], &curves[2]),
            FromCurves::Birail => None,
        };
        d.set_output(1, s.map(to_value).unwrap_or(PlugValue::Data(None)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn from_curves_type(name: &str, kind: FromCurves, doc: &str) -> NodeType {
    NodeType {
        name: name.into(),
        icon: "chart-line".into(),
        doc: doc.into(),
        own_attrs: vec![
            crate::curves::curve_attr("inputCurve", "ic", flags::INPUT | flags::ARRAY | flags::HIDDEN, "The curves."),
            surface_attr("outputSurface", "os", flags::OUTPUT | flags::HIDDEN, "The surface."),
        ],
        create: Some(Arc::new(move || Box::new(FromCurvesNode { kind }))),
        ..Default::default()
    }
}

/// curveFromSurfaceIso: an isoparm as a curve. Direction 0 is U, 1 is V.
struct IsoNode;

impl NodeBehavior for IsoNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(s) = surface_value(d.value(0)) else {
            d.set_output(1, PlugValue::Data(None));
            return;
        };
        let c = if d.int(3) == 0 { s.iso_u(d.float(2)) } else { s.iso_v(d.float(2)) };
        d.set_output(1, crate::curves::to_value(c));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Tessellation

/// nurbsToPoly's settings, as the nurbsTessellate node has them.
#[derive(Clone, Copy)]
pub struct TessOptions {
    /// 0 a polygon count, 1 standard fit, 2 general, 3 one vertex per CV.
    pub format: i64,
    pub quads: bool,
    pub count: i64,
    pub tolerance: f64,
    pub u_type: i64,
    pub u_number: i64,
    pub v_type: i64,
    pub v_number: i64,
}

/// Even steps across a direction's whole range.
fn even(params: Vec<f64>, steps: usize) -> Vec<f64> {
    let (lo, hi) = (params[0], params[params.len() - 1]);
    (0..=steps.max(1)).map(|i| lo + (hi - lo) * i as f64 / steps.max(1) as f64).collect()
}

pub fn tessellate(s: &NurbsSurface, o: TessOptions) -> Mesh {
    if !s.is_valid() {
        return Mesh::default();
    }
    let opts = MeshOptions { quads: o.quads, normals: false };
    match o.format {
        0 => {
            let target = (if o.quads { o.count } else { o.count / 2 }).max(1) as f64;
            let ratio = s.spans_u().max(1) as f64 / s.spans_v().max(1) as f64;
            let cu = (target * ratio).sqrt().round().max(1.0);
            let cv = (target / cu).round().max(1.0);
            s.mesh_at(&even(s.params_u(1), cu as usize), &even(s.params_v(1), cv as usize), opts)
        }
        1 => {
            // Doubled until the middles of the cells are within the tolerance, a fraction of the surface's size.
            let (lo, hi) = s.cvs.iter().fold((DVec3::MAX, DVec3::MIN), |(a, b), p| (a.min(*p), b.max(*p)));
            let tol = (hi - lo).length() * o.tolerance.max(1e-4);
            let mut per = 1;
            while per < 64 {
                let (us, vs) = (s.params_u(per), s.params_v(per));
                let mut worst = 0.0_f64;
                for a in us.windows(2) {
                    for b in vs.windows(2) {
                        let corners = (s.eval(a[0], b[0]) + s.eval(a[1], b[0]) + s.eval(a[0], b[1]) + s.eval(a[1], b[1])) * 0.25;
                        worst = worst.max((s.eval((a[0] + a[1]) * 0.5, (b[0] + b[1]) * 0.5) - corners).length());
                    }
                }
                if worst <= tol {
                    break;
                }
                per *= 2;
            }
            s.mesh_at(&s.params_u(per), &s.params_v(per), opts)
        }
        3 => s.mesh_at(&s.cv_params_u(), &s.cv_params_v(), opts),
        _ => {
            let pick = |ty: i64, n: i64, per: Vec<f64>, one: Vec<f64>| if ty == 3 { per } else { even(one, n.max(1) as usize) };
            let us = pick(o.u_type, o.u_number, s.params_u(o.u_number.max(1) as usize), s.params_u(1));
            let vs = pick(o.v_type, o.v_number, s.params_v(o.v_number.max(1) as usize), s.params_v(1));
            s.mesh_at(&us, &vs, opts)
        }
    }
}

struct TessellateNode;

impl NodeBehavior for TessellateNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(s) = surface_value(d.value(0)) else {
            d.set_output(1, PlugValue::Mesh(None));
            return;
        };
        let o = TessOptions {
            format: d.int(2),
            quads: d.int(3) == 1,
            count: d.int(4),
            tolerance: d.float(5),
            u_type: d.int(6),
            u_number: d.int(7),
            v_type: d.int(8),
            v_number: d.int(9),
        };
        d.set_output(1, PlugValue::Mesh(Some(Arc::new(tessellate(s, o)))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Info nodes

fn write_point_info(s: &NurbsSurface, u: f64, v: f64, d: &mut NodeData, first: usize) {
    let (p, pu, pv) = s.eval_ders(u, v);
    let n = s.normal(u, v);
    d.set_output(first, PlugValue::Vec3(p));
    d.set_output(first + 1, PlugValue::Vec3(n));
    d.set_output(first + 2, PlugValue::Vec3(pu));
    d.set_output(first + 3, PlugValue::Vec3(pv));
    d.set_output(first + 4, PlugValue::Vec3(pu.normalize_or_zero()));
    d.set_output(first + 5, PlugValue::Vec3(pv.normalize_or_zero()));
}

fn percent(s: &NurbsSurface, u: f64, v: f64) -> (f64, f64) {
    let (ulo, uhi) = s.domain_u();
    let (vlo, vhi) = s.domain_v();
    (ulo + (uhi - ulo) * u, vlo + (vhi - vlo) * v)
}

struct PointOnSurfaceNode;

impl NodeBehavior for PointOnSurfaceNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(s) = surface_value(d.value(0)).cloned() else { return };
        let (u, v) = (d.float(1), d.float(2));
        let (u, v) = if d.bool(3) { percent(&s, u, v) } else { (u, v) };
        write_point_info(&s, u, v, d, 4);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct ClosestPointNode;

impl NodeBehavior for ClosestPointNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(s) = surface_value(d.value(0)) else { return };
        let (u, v) = s.closest_point(d.vec3(1));
        let p = s.eval(u, v);
        d.set_output(2, PlugValue::Vec3(p));
        d.set_output(3, PlugValue::Float(u));
        d.set_output(4, PlugValue::Float(v));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct SurfaceInfoNode;

impl NodeBehavior for SurfaceInfoNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(s) = surface_value(d.value(0)).cloned() else { return };
        d.set_output(1, PlugValue::Int(s.degree_u as i64));
        d.set_output(2, PlugValue::Int(s.degree_v as i64));
        d.set_output(3, PlugValue::Int(s.spans_u() as i64));
        d.set_output(4, PlugValue::Int(s.spans_v() as i64));
        d.set_output(5, PlugValue::Int(s.form_u.index()));
        d.set_output(6, PlugValue::Int(s.form_v.index()));
        let data = |v: Value| PlugValue::Data(Some(Arc::new(crate::rig::JsonData(v))));
        d.set_output(7, data(json!(s.knots_u)));
        d.set_output(8, data(json!(s.knots_v)));
        d.set_output(9, data(json!(s.cvs.iter().map(|p| [p.x, p.y, p.z]).collect::<Vec<_>>())));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_one(t: NodeType) {
    let name = t.name.clone();
    types::register_type(t).expect(&name);
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        crate::curves::register_types();
        crate::plug::data::register_kind("nurbsSurfaceData", Arc::new(|j| Ok(Arc::new(SurfaceData(surface_from_json(j))) as Arc<dyn crate::plug::Data>)));
        let shape = types::find("shape").expect("shape");
        let stored = flags::INPUT | flags::STORABLE;
        register_one(NodeType {
            name: SHAPE_TYPE.into(),
            base: Some(shape),
            dag: true,
            icon: "chart-line".into(),
            doc: "A NURBS surface: its CVs are components, surface1.cv[u][v].".into(),
            own_attrs: vec![
                surface_attr("create", "cr", flags::INPUT | flags::HIDDEN, "From history."),
                surface_attr("cached", "cc", stored | flags::HIDDEN, "The surface itself, when no history feeds it."),
                crate::curves::curve_attr("controlPoints", "cp", stored | flags::HIDDEN, "CV offsets on top of history."),
                surface_attr("local", "l", flags::OUTPUT | flags::HIDDEN, "The surface in object space."),
                surface_attr("worldSpace", "ws", flags::OUTPUT | flags::HIDDEN, "The surface in world space."),
                mesh_attr("outMesh", "o", flags::OUTPUT | flags::HIDDEN, "The tessellation the viewport draws."),
                surface_attr("isoparmLines", "isl", flags::OUTPUT | flags::HIDDEN, "The isoparms the viewport draws."),
                int_out("degreeU", "du", "Degree along U."),
                int_out("degreeV", "dv", "Degree along V."),
                int_out("spansU", "su", "Spans along U."),
                int_out("spansV", "sv", "Spans along V."),
                int_out("formU", "fu", "0 open, 1 closed, 2 periodic."),
                int_out("formV", "fv", "0 open, 1 closed, 2 periodic."),
                float_out("minValueU", "mnu", "First parameter along U."),
                float_out("maxValueU", "mxu", "Last parameter along U."),
                float_out("minValueV", "mnv", "First parameter along V."),
                float_out("maxValueV", "mxv", "Last parameter along V."),
                stored_int("curvePrecision", "cpr", 4, "Points to a span of a drawn isoparm."),
                stored_int("curvePrecisionShaded", "cps", 1, "Shaded fineness."),
                stored_int("divisionsU", "nu", 0, "Extra drawn isoparms within a span along U."),
                stored_int("divisionsV", "nv", 0, "Extra drawn isoparms within a span along V."),
                store_bool("dispCV", false, "Show CVs when not in component mode."),
                store_bool("dispHull", false, "Show the hull."),
                store_bool("dispEP", false, "Show edit points."),
                store_bool("dispGeometry", true, "Show the surface."),
                crate::shading::inst_obj_groups(),
                crate::shading::object_groups(),
            ]
            .into_iter()
            .chain(crate::harold_nodes::shape_render_attrs())
            .collect(),
            create: Some(Arc::new(|| Box::<SurfaceShape>::default())),
            ..Default::default()
        });
        let mut sphere = round_params(4);
        sphere.push(f_in("heightRatio", "hr", 2.0, "Accepted for existing scripts."));
        register_one(make_type("makeNurbSphere", Make::Sphere, "A sphere.", sphere));
        for (name, kind, doc) in [("makeNurbCylinder", Make::Cylinder, "A cylinder."), ("makeNurbCone", Make::Cone, "A cone.")] {
            let mut p = round_params(1);
            p.push(f_in("heightRatio", "hr", 2.0, "Height over radius."));
            register_one(make_type(name, kind, doc, p));
        }
        let mut torus = round_params(4);
        torus.push(f_in("heightRatio", "hr", 0.5, "Tube radius over radius."));
        torus.push(f_in("minorSweep", "msw", 360.0, "Degrees of the tube's section."));
        register_one(make_type("makeNurbTorus", Make::Torus, "A torus.", torus));
        register_one(make_type(
            "makeNurbPlane",
            Make::Plane,
            "A plane.",
            vec![f_in("width", "w", 1.0, "The width."), f_in("lengthRatio", "lr", 1.0, "Length over width."), int_in("degree", "d", 3, "The degree."), int_in("patchesU", "u", 1, "Patches along U."), int_in("patchesV", "v", 1, "Patches along V.")],
        ));
        let mut cube = vec![
            f_in("width", "w", 1.0, "The width."),
            f_in("lengthRatio", "lr", 1.0, "Length over width."),
            f_in("heightRatio", "hr", 1.0, "Height over width."),
            int_in("degree", "d", 3, "The degree."),
            int_in("patchesU", "u", 1, "Patches along U."),
            int_in("patchesV", "v", 1, "Patches along V."),
        ];
        for i in 1..6 {
            cube.push(surface_attr(&format!("outputSurface{i}"), &format!("os{i}"), flags::OUTPUT | flags::HIDDEN, "Another side."));
        }
        register_one(make_type("makeNurbCube", Make::Cube, "Six patches, a cube.", cube));

        for t in [
            op_type(
                "rebuildSurface",
                Op::Rebuild,
                "Rebuilt with even spans.",
                vec![
                    int_in("spansU", "su", 4, "Spans along U; 0 keeps the count."),
                    int_in("spansV", "sv", 4, "Spans along V; 0 keeps the count."),
                    int_in("degreeU", "du", 3, "Degree along U."),
                    int_in("degreeV", "dv", 3, "Degree along V."),
                    int_in("keepRange", "kr", 1, "0 parameters 0 to 1, 1 as they were, 2 0 to the spans."),
                    int_in("direction", "dir", 2, "0 U, 1 V, 2 both."),
                    store_bool("keepControlPoints", false, "Leave the CVs as they are."),
                ],
            ),
            op_type("reverseSurface", Op::Reverse, "Direction reversed.", vec![int_in("direction", "d", 0, "0 U, 1 V, 2 both, 3 swap U and V.")]),
            op_type(
                "insertKnotSurface",
                Op::InsertKnot,
                "Knots added.",
                vec![f_in("parameter", "p", 0.0, "Where."), int_in("direction", "d", 1, "1 U, 0 V."), int_in("numberOfKnots", "nk", 1, "How many."), store_bool("addKnots", true, "Add them, rather than bring the knot's multiplicity up to the number.")],
            ),
            op_type(
                "detachSurface",
                Op::Detach,
                "Cut in two.",
                vec![f_in("parameter", "p", 0.0, "Where."), int_in("direction", "d", 1, "1 U, 0 V."), surface_attr("outputSurface1", "os1", flags::OUTPUT | flags::HIDDEN, "The second piece.")],
            ),
            op_type(
                "attachSurface",
                Op::Attach,
                "Two surfaces joined.",
                vec![
                    int_in("method", "m", 0, "0 connect, 1 blend."),
                    f_in("blendBias", "bb", 0.5, "Where a blend meets."),
                    int_in("direction", "d", 1, "1 U, 0 V."),
                    surface_attr("inputSurface1", "is1", flags::INPUT | flags::STORABLE | flags::HIDDEN, "The surface added."),
                ],
            ),
            op_type("offsetSurface", Op::Offset, "Moved along its normals.", vec![f_in("distance", "d", 1.0, "How far.")]),
            op_type("closeSurface", Op::Close, "Closed.", vec![int_in("direction", "d", 0, "0 U, 1 V, 2 both.")]),
            from_curves_type("boundary", FromCurves::Coons, "A surface inside three or four curves."),
            from_curves_type("squareSrf", FromCurves::Coons, "A surface inside four curves."),
            from_curves_type("singleProfileBirailSurface", FromCurves::Birail, "A profile along two rails."),
        ] {
            register_one(t);
        }
        register_one(NodeType {
            name: "curveFromSurfaceIso".into(),
            icon: "chart-line".into(),
            doc: "An isoparm as a curve.".into(),
            own_attrs: vec![
                surface_attr("inputSurface", "is", flags::INPUT | flags::HIDDEN, "The surface."),
                crate::curves::curve_attr("outputCurve", "oc", flags::OUTPUT | flags::HIDDEN, "The curve."),
                f_in("isoparmValue", "iv", 0.0, "The parameter."),
                int_in("isoparmDirection", "idr", 0, "0 U, 1 V."),
            ],
            create: Some(Arc::new(|| Box::new(IsoNode))),
            ..Default::default()
        });
        register_one(NodeType {
            name: "nurbsTessellate".into(),
            icon: "chart-line".into(),
            doc: "A surface as polygons.".into(),
            own_attrs: vec![
                surface_attr("inputSurface", "is", flags::INPUT | flags::HIDDEN, "The surface."),
                mesh_attr("outputPolygon", "op", flags::OUTPUT | flags::HIDDEN, "The mesh."),
                int_in("format", "f", 1, "0 count, 1 standard fit, 2 general, 3 CVs."),
                int_in("polygonType", "pt", 0, "0 triangles, 1 quads."),
                int_in("polygonCount", "pc", 200, "Polygons, for format 0."),
                f_in("fractionalTolerance", "ft", 0.01, "Tolerance for format 1, a fraction of the surface's size."),
                int_in("uType", "ut", 3, "1 per surface in 3D, 2 per surface, 3 per span."),
                int_in("uNumber", "un", 3, "How many along U, for format 2."),
                int_in("vType", "vt", 3, "1 per surface in 3D, 2 per surface, 3 per span."),
                int_in("vNumber", "vn", 3, "How many along V, for format 2."),
                f_in("chordHeightRatio", "chr", 0.9, "Accepted for existing scripts."),
                f_in("minEdgeLength", "mel", 0.001, "Accepted for existing scripts."),
                f_in("delta", "d", 0.1, "Accepted for existing scripts."),
                store_bool("useChordHeight", false, "Accepted for existing scripts."),
                store_bool("useChordHeightRatio", true, "Accepted for existing scripts."),
                f_in("chordHeight", "cht", 0.2, "Accepted for existing scripts."),
                store_bool("edgeSwap", false, "Accepted for existing scripts."),
                store_bool("normalizeTrimmedUVRange", true, "Accepted for existing scripts."),
                store_bool("matchRenderTessellation", false, "Accepted for existing scripts."),
                store_bool("useSurfaceShader", true, "Accepted for existing scripts."),
            ],
            create: Some(Arc::new(|| Box::new(TessellateNode))),
            ..Default::default()
        });
        let out3 = |name: &str, short: &str, doc: &str| vec3_attr(name, short, DVec3::ZERO, flags::OUTPUT, doc);
        register_one(NodeType {
            name: "pointOnSurfaceInfo".into(),
            icon: "chart-line".into(),
            doc: "A point on a surface at parameters.".into(),
            own_attrs: vec![
                surface_attr("inputSurface", "is", flags::INPUT | flags::HIDDEN, "The surface."),
                f_in("parameterU", "u", 0.0, "The U parameter."),
                f_in("parameterV", "v", 0.0, "The V parameter."),
                store_bool("turnOnPercentage", false, "The parameters are fractions 0 to 1 of the range."),
                out3("position", "p", "The point."),
                out3("normalizedNormal", "nn", "The unit normal."),
                out3("tangentU", "tu", "The derivative along U."),
                out3("tangentV", "tv", "The derivative along V."),
                out3("normalizedTangentU", "ntu", "The unit tangent along U."),
                out3("normalizedTangentV", "ntv", "The unit tangent along V."),
            ],
            create: Some(Arc::new(|| Box::new(PointOnSurfaceNode))),
            ..Default::default()
        });
        register_one(NodeType {
            name: "closestPointOnSurface".into(),
            icon: "chart-line".into(),
            doc: "The point of a surface nearest a position.".into(),
            own_attrs: vec![
                surface_attr("inputSurface", "is", flags::INPUT | flags::HIDDEN, "The surface."),
                v_in("inPosition", "ip", DVec3::ZERO, "The position."),
                out3("position", "p", "The nearest point."),
                float_out("parameterU", "u", "Its U parameter."),
                float_out("parameterV", "v", "Its V parameter."),
            ],
            create: Some(Arc::new(|| Box::new(ClosestPointNode))),
            ..Default::default()
        });
        register_one(NodeType {
            name: "surfaceInfo".into(),
            icon: "chart-line".into(),
            doc: "Facts about a surface.".into(),
            own_attrs: vec![
                surface_attr("inputSurface", "is", flags::INPUT | flags::HIDDEN, "The surface."),
                int_out("degreeU", "du", "Degree along U."),
                int_out("degreeV", "dv", "Degree along V."),
                int_out("spansU", "su", "Spans along U."),
                int_out("spansV", "sv", "Spans along V."),
                int_out("formU", "fu", "Form along U."),
                int_out("formV", "fv", "Form along V."),
                crate::curves::curve_attr("knotsU", "ku", flags::OUTPUT, "Knots along U."),
                crate::curves::curve_attr("knotsV", "kv", flags::OUTPUT, "Knots along V."),
                crate::curves::curve_attr("controlPoints", "cp", flags::OUTPUT, "The CVs."),
            ],
            create: Some(Arc::new(|| Box::new(SurfaceInfoNode))),
            ..Default::default()
        });
        register_provider();
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// Command helpers

type Cmd = CommandDef<Session>;

pub(crate) fn surface_arg(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = find(g, name, cmd)?;
    match as_surface(g, id) {
        Some(s) => Ok(s),
        None => fail!("{cmd}: {name} is not a NURBS surface"),
    }
}

pub(crate) fn curve_shape(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = find(g, name, cmd)?;
    match crate::curves::as_curve(g, id) {
        Some(c) => Ok(c),
        None => fail!("{cmd}: {name} is not a curve"),
    }
}

/// The surfaces at or under a node, as nurbsCube's six are under its group.
fn surfaces_under(g: &Graph, id: NodeId) -> Vec<NodeId> {
    let mut all = vec![id];
    all.extend(g.descendants(id));
    all.into_iter().filter(|&n| is_surface(g, n) && !g.node(n).is_some_and(|x| x.bool(DagNode::INTERMEDIATE_OBJECT))).collect()
}

/// delete -ch on a surface: the shape keeps what history gives, the nodes go.
pub fn bake_history(g: &mut Graph, shape: NodeId) -> CmdResult<()> {
    let Some(s) = local_surface(g, shape) else { return Ok(()) };
    let create = plug(shape, SurfaceShape::CREATE);
    let mut src = g.source(&create);
    g.disconnect(&create);
    // A disconnected plug keeps its last value, which would hide the shape's own surface.
    g.set_value(create, PlugValue::Data(None))?;
    set_cached(g, shape, s)?;
    let _ = g.set_value(plug(shape, SurfaceShape::TWEAK), PlugValue::Data(None));
    while let Some(p) = src {
        let is_op = g.node(p.node).is_some_and(|n| n.ty().attr_index("inputSurface") == Some(IN));
        let up = if is_op { g.source(&plug(p.node, IN)) } else { None };
        // Stop at nodes other things hang off, and leave shapes alone.
        let free = (0..g.node(p.node).map(|n| n.ty().attrs.len()).unwrap_or(0)).all(|a| g.destinations(&plug(p.node, a)).is_empty());
        if free && !g.node(p.node).is_some_and(|n| n.ty().dag) {
            g.delete_node(p.node)?;
        }
        src = up;
    }
    Ok(())
}

/// History nodes feeding a surface, nearest first, for deleting along with it.
pub fn surface_history(g: &Graph, shape: NodeId) -> Vec<NodeId> {
    crate::history::upstream_nodes(g, &plug(shape, SurfaceShape::CREATE)).into_iter().filter(|&h| g.node(h).is_some_and(|n| !n.ty().dag)).collect()
}

/// Puts a history node above a shape; returns the node. Tweaks go into the input so they aren't lost.
pub(crate) fn insert_op(g: &mut Graph, shape: NodeId, ty: &str, set: &[(usize, PlugValue)]) -> CmdResult<NodeId> {
    let current = local_surface(g, shape).unwrap_or_default();
    let t = types::find(ty).ok_or_else(|| aztek_ops::CommandError::new(format!("no node type {ty}")))?;
    let node = g.create_node(&t, &format!("{ty}1"), NO_NODE, true)?;
    let create = plug(shape, SurfaceShape::CREATE);
    match g.source(&create) {
        Some(src) if g.node(shape).is_some_and(|n| matches!(n.data().value(SurfaceShape::TWEAK), PlugValue::Data(None))) => {
            g.connect(&src, &plug(node, IN), false)?;
        }
        _ => {
            g.set_value(plug(node, IN), to_value(current))?;
            let _ = g.set_value(plug(shape, SurfaceShape::TWEAK), PlugValue::Data(None));
        }
    }
    for (attr, v) in set {
        g.set_value(plug(node, *attr), v.clone())?;
    }
    g.connect(&plug(node, OUT), &create, true)?;
    Ok(node)
}

pub fn create_surface(g: &mut Graph, name: &str, s: NurbsSurface) -> CmdResult<(NodeId, NodeId)> {
    let (tr, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, name, NO_NODE)?;
    set_cached(g, sh, s)?;
    Ok((tr, sh))
}

pub(crate) fn wants_history(g: &Graph, a: &Args) -> bool {
    a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(g.construction_history)
}

pub(crate) fn with_common(c: Cmd) -> Cmd {
    c.flag(f("constructionHistory", "ch", ArgType::Bool, "Keep a history node (on by default)."))
        .flag(f("replaceOriginal", "rpo", ArgType::Bool, "Change the surface itself (on by default)."))
        .flag(f("name", "n", ArgType::String, "Name of a new surface."))
        .flag(f("object", "o", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("polygon", "po", ArgType::Int, "Accepted for existing scripts."))
}

/// Runs an edit on the surfaces named or selected, with the reference's -ch and -rpo. Returns [surface, node].
pub(crate) fn run_op(s: &mut Session, a: &Args, cmd: &str, ty: &str, set: &[(usize, PlugValue)]) -> CmdResult {
    let names = objects(s, a);
    if names.is_empty() {
        fail!("{cmd}: name or select a surface");
    }
    let g = &mut s.graph;
    let ch = wants_history(g, a);
    let rpo = a.get("replaceOriginal").and_then(Value::as_bool).unwrap_or(true);
    let mut out = Vec::new();
    for n in names {
        let shape = surface_arg(g, &n, cmd)?;
        let target = if rpo {
            shape
        } else {
            let c = world_surface(g, shape).unwrap_or_default();
            create_surface(g, &a.string("name", ""), c)?.1
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

/// A mesh transform fed by a surface plug through a nurbsTessellate node.
pub(crate) fn tessellate_plug(g: &mut Graph, src: &crate::plug::PlugRef, name: &str, set: &[(usize, PlugValue)]) -> CmdResult<(NodeId, NodeId)> {
    let t = types::find("nurbsTessellate").expect("nurbsTessellate");
    let node = g.create_node(&t, "nurbsTessellate1", NO_NODE, true)?;
    g.connect(src, &plug(node, 0), false)?;
    for (attr, v) in set {
        g.set_value(plug(node, *attr), v.clone())?;
    }
    let (tr, sh) = crate::mesh_nodes::create_mesh_node(g, name)?;
    g.connect(&plug(node, 1), &plug(sh, crate::mesh_nodes::MeshNode::IN_MESH), true)?;
    Ok((tr, node))
}

const PER_SPAN: [(usize, i64); 2] = [(2, 2), (3, 1)];

pub(crate) fn per_span_settings() -> Vec<(usize, PlugValue)> {
    PER_SPAN.iter().map(|&(a, v)| (a, PlugValue::Int(v))).collect()
}

/// A primitive's transform, shape and make node, or with -po 1 a mesh in place of the surface.
fn make_primitive(s: &mut Session, a: &Args, ty: &str, name: &str, node_name: &str, set: &[(usize, PlugValue)]) -> CmdResult {
    let g = &mut s.graph;
    let ch = wants_history(g, a);
    let name = a.string("name", name);
    let polygon = a.int("polygon", 0) == 1;
    g.open_chunk(ty);
    let r = (|| -> CmdResult<(NodeId, NodeId, Option<NodeId>)> {
        let t = types::find(ty).expect("make type");
        let n = g.create_node(&t, node_name, NO_NODE, true)?;
        for (attr, v) in set {
            g.set_value(plug(n, *attr), v.clone())?;
        }
        if polygon {
            let (tr, _) = tessellate_plug(g, &plug(n, M_OUT), &name, &per_span_settings())?;
            return Ok((tr, n, None));
        }
        let (tr, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, &name, NO_NODE)?;
        g.connect(&plug(n, M_OUT), &plug(sh, SurfaceShape::CREATE), false)?;
        Ok((tr, n, Some(sh)))
    })();
    g.close_chunk();
    let (tr, n, sh) = r?;
    g.select(&[tr], SelectMode::Replace);
    g.evaluate();
    if !ch {
        match sh {
            Some(sh) => bake_history(g, sh)?,
            None => {
                if let Some(m) = components::as_mesh(g, tr) {
                    crate::history::delete_history(g, m)?;
                }
            }
        }
        return Ok(json!([g.path(tr)]));
    }
    Ok(json!([g.path(tr), g.path(n)]))
}

fn axis_pivot(a: &Args) -> Vec<(usize, PlugValue)> {
    vec![(M_AXIS, PlugValue::Vec3(crate::curves::v3(a, "axis", DVec3::X))), (M_PIVOT, PlugValue::Vec3(crate::curves::v3(a, "pivot", DVec3::ZERO)))]
}

fn round_settings(a: &Args, spans: i64) -> Vec<(usize, PlugValue)> {
    let mut set = axis_pivot(a);
    set.extend([
        (M_P, PlugValue::Float(a.float("radius", 1.0))),
        (M_P + 1, PlugValue::Float(a.float("startSweep", 0.0))),
        (M_P + 2, PlugValue::Float(a.float("endSweep", 360.0))),
        (M_P + 3, PlugValue::Int(a.int("degree", 3))),
        (M_P + 4, PlugValue::Int(a.int("sections", 8))),
        (M_P + 5, PlugValue::Int(a.int("spans", spans))),
        (M_P + 6, PlugValue::Bool(a.bool("useTolerance"))),
        (M_P + 7, PlugValue::Float(a.float("tolerance", 0.01))),
    ]);
    set
}

fn primitive_flags(c: Cmd) -> Cmd {
    c.flag(f("axis", "ax", ArgType::Vec3, "The axis (1 0 0)."))
        .flag(f("pivot", "p", ArgType::Vec3, "The centre."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the make node (on by default)."))
        .flag(f("name", "n", ArgType::String, "Name of the transform."))
        .flag(f("object", "o", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("polygon", "po", ArgType::Int, "1 makes polygons."))
}

fn round_flags(c: Cmd) -> Cmd {
    primitive_flags(c)
        .flag(f("radius", "r", ArgType::Float, "The radius (1)."))
        .flag(f("startSweep", "ssw", ArgType::Float, "Start, degrees (0)."))
        .flag(f("endSweep", "esw", ArgType::Float, "End, degrees (360)."))
        .flag(f("degree", "d", ArgType::Int, "1 linear, 3 cubic (3)."))
        .flag(f("sections", "s", ArgType::Int, "Sections around (8)."))
        .flag(f("spans", "nsp", ArgType::Int, "Spans along the axis."))
        .flag(f("useTolerance", "ut", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("heightRatio", "hr", ArgType::Float, "Height over radius."))
}

/// Curves named or selected, for a command making a surface from them, with its node on the output.
fn from_curves_cmd(s: &mut Session, a: &Args, cmd: &str, ty: &str, name: &str, node_name: &str, min: usize, max: usize) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    if names.len() < min || names.len() > max {
        fail!("{cmd}: name or select {min} to {max} curves");
    }
    let inputs: Vec<NodeId> = names.iter().map(|n| curve_shape(g, n, cmd)).collect::<CmdResult<_>>()?;
    let t = types::find(ty).expect("surface node type");
    let node = g.create_node(&t, node_name, NO_NODE, true)?;
    for (i, &shape) in inputs.iter().enumerate() {
        g.connect(&plug(shape, crate::curves::CurveShape::WORLD_SPACE), &crate::plug::PlugRef::element(node, 0, i as i32), false)?;
    }
    let polygon = a.int("polygon", 0) == 1;
    let name = a.string("name", name);
    let (tr, sh) = if polygon {
        tessellate_plug(g, &plug(node, 1), &name, &per_span_settings())?
    } else {
        let (tr, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, &name, NO_NODE)?;
        g.connect(&plug(node, 1), &plug(sh, SurfaceShape::CREATE), false)?;
        (tr, sh)
    };
    g.select(&[tr], SelectMode::Replace);
    g.evaluate();
    if !wants_history(g, a) {
        if polygon {
            if let Some(m) = components::as_mesh(g, tr) {
                crate::history::delete_history(g, m)?;
            }
        } else {
            bake_history(g, sh)?;
        }
        return Ok(json!([g.path(tr)]));
    }
    Ok(json!([g.path(tr), g.path(node)]))
}

fn curve_surface_flags(c: Cmd) -> Cmd {
    c.flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("polygon", "po", ArgType::Int, "0 NURBS, 1 polygons."))
        .flag(f("range", "rn", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the new object."))
        .flag(f("object", "o", ArgType::Bool, "Accepted for existing scripts."))
}

fn multi_params(a: &Args) -> Vec<f64> {
    fn walk(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Array(x) => x.iter().for_each(|i| walk(i, out)),
            x => out.extend(x.as_f64()),
        }
    }
    let mut out = Vec::new();
    if let Some(v) = a.get("parameter") {
        walk(v, &mut out);
    }
    out
}

/// surface.u[0.5] or surface.v[1]: the shape, 0 for U or 1 for V, and the parameter.
pub(crate) fn parse_isoparm(g: &Graph, name: &str) -> Option<(NodeId, i64, f64)> {
    // The parameter may have a decimal point of its own, so the dot wanted is the one before the bracket.
    let (head, rest) = name.split_once('[')?;
    let (obj, kind) = head.rsplit_once('.')?;
    let dir = match kind {
        "u" => 0,
        "v" => 1,
        _ => return None,
    };
    let value: f64 = rest.trim_end_matches(']').parse().ok()?;
    let id = g.find(obj).ok()??;
    Some((as_surface(g, id)?, dir, value))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    c.register(round_flags(Cmd::new("sphere", "Create a NURBS sphere with a makeNurbSphere node. Returns [nurbsSphere1, makeNurbSphere1].", |s, a| {
        let set = round_settings(a, 4);
        make_primitive(s, a, "makeNurbSphere", "nurbsSphere1", "makeNurbSphere1", &set)
    })));
    c.register(round_flags(Cmd::new("cylinder", "Create a NURBS cylinder with a makeNurbCylinder node. Returns [nurbsCylinder1, makeNurbCylinder1].", |s, a| {
        let mut set = round_settings(a, 1);
        set.push((M_P + 8, PlugValue::Float(a.float("heightRatio", 2.0))));
        make_primitive(s, a, "makeNurbCylinder", "nurbsCylinder1", "makeNurbCylinder1", &set)
    })));
    c.register(round_flags(Cmd::new("cone", "Create a NURBS cone with a makeNurbCone node. Returns [nurbsCone1, makeNurbCone1].", |s, a| {
        let mut set = round_settings(a, 1);
        set.push((M_P + 8, PlugValue::Float(a.float("heightRatio", 2.0))));
        make_primitive(s, a, "makeNurbCone", "nurbsCone1", "makeNurbCone1", &set)
    })));
    c.register(
        round_flags(Cmd::new("torus", "Create a NURBS torus with a makeNurbTorus node. Returns [nurbsTorus1, makeNurbTorus1].", |s, a| {
            let mut set = round_settings(a, 4);
            set.push((M_P + 8, PlugValue::Float(a.float("heightRatio", 0.5))));
            set.push((M_P + 9, PlugValue::Float(a.float("minorSweep", 360.0))));
            make_primitive(s, a, "makeNurbTorus", "nurbsTorus1", "makeNurbTorus1", &set)
        }))
        .flag(f("minorSweep", "msw", ArgType::Float, "Degrees of the tube's section (360).")),
    );
    c.register(
        primitive_flags(Cmd::new("nurbsPlane", "Create a NURBS plane with a makeNurbPlane node. Returns [nurbsPlane1, makeNurbPlane1].", |s, a| {
            let mut set = axis_pivot(a);
            set.extend([
                (M_P, PlugValue::Float(a.float("width", 1.0))),
                (M_P + 1, PlugValue::Float(a.float("lengthRatio", 1.0))),
                (M_P + 2, PlugValue::Int(a.int("degree", 3))),
                (M_P + 3, PlugValue::Int(a.int("patchesU", 1))),
                (M_P + 4, PlugValue::Int(a.int("patchesV", 1))),
            ]);
            make_primitive(s, a, "makeNurbPlane", "nurbsPlane1", "makeNurbPlane1", &set)
        }))
        .flag(f("width", "w", ArgType::Float, "The width (1)."))
        .flag(f("lengthRatio", "lr", ArgType::Float, "Length over width (1)."))
        .flag(f("degree", "d", ArgType::Int, "The degree (3)."))
        .flag(f("patchesU", "u", ArgType::Int, "Patches along U (1)."))
        .flag(f("patchesV", "v", ArgType::Int, "Patches along V (1).")),
    );
    c.register(
        primitive_flags(Cmd::new(
            "nurbsCube",
            "Create a cube of six NURBS patches under a group, with a makeNurbCube node. Returns [nurbsCube1, makeNurbCube1].",
            |s, a| {
                let g = &mut s.graph;
                let ch = wants_history(g, a);
                let name = a.string("name", "nurbsCube1");
                let mut set = axis_pivot(a);
                set.extend([
                    (M_P, PlugValue::Float(a.float("width", 1.0))),
                    (M_P + 1, PlugValue::Float(a.float("lengthRatio", 1.0))),
                    (M_P + 2, PlugValue::Float(a.float("heightRatio", 1.0))),
                    (M_P + 3, PlugValue::Int(a.int("degree", 3))),
                    (M_P + 4, PlugValue::Int(a.int("patchesU", 1))),
                    (M_P + 5, PlugValue::Int(a.int("patchesV", 1))),
                ]);
                g.open_chunk("nurbsCube");
                let r = (|| -> CmdResult<(NodeId, NodeId, Vec<NodeId>)> {
                    let t = types::find("makeNurbCube").expect("makeNurbCube");
                    let n = g.create_node(&t, "makeNurbCube1", NO_NODE, true)?;
                    for (attr, v) in &set {
                        g.set_value(plug(n, *attr), v.clone())?;
                    }
                    let group = g.create_node(&types::find("transform").expect("transform"), &name, NO_NODE, true)?;
                    let stem = g.node(group).map(|x| x.name().to_string()).unwrap_or_default();
                    let mut shapes = Vec::new();
                    for (i, face) in CUBE_FACES.iter().enumerate() {
                        let (_, sh) = crate::dag::create_with_shape(g, SHAPE_TYPE, &format!("{face}{stem}"), group)?;
                        let out = if i == 0 { M_OUT } else { M_P + 5 + i };
                        g.connect(&plug(n, out), &plug(sh, SurfaceShape::CREATE), false)?;
                        shapes.push(sh);
                    }
                    Ok((group, n, shapes))
                })();
                g.close_chunk();
                let (group, n, shapes) = r?;
                g.select(&[group], SelectMode::Replace);
                g.evaluate();
                if !ch {
                    for sh in shapes {
                        bake_history(g, sh)?;
                    }
                    return Ok(json!([g.path(group)]));
                }
                Ok(json!([g.path(group), g.path(n)]))
            },
        ))
        .flag(f("width", "w", ArgType::Float, "The width (1)."))
        .flag(f("lengthRatio", "lr", ArgType::Float, "Length over width (1)."))
        .flag(f("heightRatio", "hr", ArgType::Float, "Height over width (1)."))
        .flag(f("degree", "d", ArgType::Int, "The degree (3)."))
        .flag(f("patchesU", "u", ArgType::Int, "Patches along U (1)."))
        .flag(f("patchesV", "v", ArgType::Int, "Patches along V (1).")),
    );

    c.register(with_common(
        Cmd::new("rebuildSurface", "Rebuild with even spans: -su -sv spans (0 keeps the count), -du -dv degrees, -dir 0 U, 1 V, 2 both, -kr range. Returns [surface, node].", |s, a| {
            let set = vec![
                (P0, PlugValue::Int(a.int("spansU", 4))),
                (P0 + 1, PlugValue::Int(a.int("spansV", 4))),
                (P0 + 2, PlugValue::Int(a.int("degreeU", 3))),
                (P0 + 3, PlugValue::Int(a.int("degreeV", 3))),
                (P0 + 4, PlugValue::Int(a.int("keepRange", 1))),
                (P0 + 5, PlugValue::Int(a.int("direction", 2))),
                (P0 + 6, PlugValue::Bool(a.bool("keepControlPoints"))),
            ];
            run_op(s, a, "rebuildSurface", "rebuildSurface", &set)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("spansU", "su", ArgType::Int, "Spans along U (4)."))
        .flag(f("spansV", "sv", ArgType::Int, "Spans along V (4)."))
        .flag(f("degreeU", "du", ArgType::Int, "Degree along U (3)."))
        .flag(f("degreeV", "dv", ArgType::Int, "Degree along V (3)."))
        .flag(f("keepRange", "kr", ArgType::Int, "0 parameters 0 to 1, 1 as they were, 2 0 to the spans (1)."))
        .flag(f("direction", "dir", ArgType::Int, "0 U, 1 V, 2 both (2)."))
        .flag(f("keepControlPoints", "kcp", ArgType::Bool, "Leave the CVs as they are."))
        .flag(f("rebuildType", "rt", ArgType::Int, "Accepted for existing scripts (uniform)."))
        .flag(f("endKnots", "end", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("keepCorners", "kc", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("fitRebuild", "fr", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", ArgType::Float, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("reverseSurface", "Reverse the direction: -d 0 U, 1 V, 2 both, 3 swap U and V. Returns [surface, node].", |s, a| {
            let set = vec![(P0, PlugValue::Int(a.int("direction", 0)))];
            run_op(s, a, "reverseSurface", "reverseSurface", &set)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("direction", "d", ArgType::Int, "0 U, 1 V, 2 both, 3 swap (0).")),
    ));
    c.register(with_common(
        Cmd::new("insertKnotSurface", "Add knots at -p (repeat it) along -d, 1 U or 0 V: the shape stays. -nk how many. Returns [surface, node].", |s, a| {
            let params = multi_params(a);
            if params.is_empty() {
                fail!("insertKnotSurface: give -p");
            }
            let mut out = Value::Null;
            for p in params {
                let set = vec![(P0, PlugValue::Float(p)), (P0 + 1, PlugValue::Int(a.int("direction", 1))), (P0 + 2, PlugValue::Int(a.int("numberOfKnots", 1))), (P0 + 3, PlugValue::Bool(a.get("addKnots").and_then(Value::as_bool).unwrap_or(true)))];
                out = run_op(s, a, "insertKnotSurface", "insertKnotSurface", &set)?;
            }
            Ok(out)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("parameter", "p", ArgType::List, "Where. Repeat it.").multi(1))
        .flag(f("direction", "d", ArgType::Int, "1 U, 0 V (1)."))
        .flag(f("numberOfKnots", "nk", ArgType::Int, "How many (1)."))
        .flag(f("addKnots", "add", ArgType::Bool, "Add them (on), or bring the multiplicity up to the number."))
        .flag(f("insertBetween", "ib", ArgType::Bool, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("offsetSurface", "Move a surface along its normals by -d. Returns [surface, node].", |s, a| {
            let set = vec![(P0, PlugValue::Float(a.float("distance", 1.0)))];
            run_op(s, a, "offsetSurface", "offsetSurface", &set)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("distance", "d", ArgType::Float, "How far (1)."))
        .flag(f("method", "m", ArgType::Int, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("closeSurface", "Close a surface: -d 0 U, 1 V, 2 both. Returns [surface, node].", |s, a| {
            let set = vec![(P0, PlugValue::Int(a.int("direction", 0)))];
            run_op(s, a, "closeSurface", "closeSurface", &set)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("direction", "d", ArgType::Int, "0 U, 1 V, 2 both (0)."))
        .flag(f("preserveShape", "ps", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("blendBias", "bb", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("blendKnotInsertion", "bki", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("parameter", "p", ArgType::Float, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("detachSurface", "Cut a surface at -p (repeat it) along -d, 1 U or 0 V. Returns the pieces' transforms and the first node.", |s, a| {
            let names = objects(s, a);
            let g = &mut s.graph;
            let Some(n) = names.first() else { fail!("detachSurface: name or select a surface") };
            let mut shape = surface_arg(g, n, "detachSurface")?;
            let mut params = multi_params(a);
            params.sort_by(f64::total_cmp);
            if params.is_empty() {
                fail!("detachSurface: give -p");
            }
            let dir = a.int("direction", 1);
            let keep_history = wants_history(g, a);
            let rpo = a.get("replaceOriginal").and_then(Value::as_bool).unwrap_or(true);
            if !rpo {
                let copy = world_surface(g, shape).unwrap_or_default();
                shape = create_surface(g, &a.string("name", ""), copy)?.1;
            }
            let tr = crate::dag::transform_of(g, shape);
            let mut out = vec![json!(g.path(tr))];
            let mut first = None;
            let mut made = vec![shape];
            for p in params {
                let set = vec![(P0, PlugValue::Float(p)), (P0 + 1, PlugValue::Int(dir))];
                let node = insert_op(g, shape, "detachSurface", &set)?;
                first.get_or_insert(node);
                let (t2, s2) = crate::dag::create_with_shape(g, SHAPE_TYPE, &format!("{}detachedSurface1", g.node(tr).map(|x| x.name().to_string()).unwrap_or_default()), NO_NODE)?;
                g.connect(&plug(node, P0 + 2), &plug(s2, SurfaceShape::CREATE), false)?;
                out.push(json!(g.path(t2)));
                made.push(s2);
                shape = s2;
            }
            g.evaluate();
            if keep_history {
                if let Some(node) = first {
                    out.push(json!(g.path(node)));
                }
            } else {
                // Later pieces first, so each node goes once nothing hangs off it.
                for sh in made.into_iter().rev() {
                    bake_history(g, sh)?;
                }
            }
            Ok(Value::Array(out))
        })
        .positional("a surface (the selection when none)")
        .flag(f("parameter", "p", ArgType::List, "Where. Repeat it.").multi(1))
        .flag(f("direction", "d", ArgType::Int, "1 U, 0 V (1)."))
        .flag(f("keep", "k", ArgType::Bool, "Accepted for existing scripts.")),
    ));
    c.register(with_common(
        Cmd::new("attachSurface", "Join two surfaces along -d, 1 U or 0 V; -m 0 connects, 1 blends at -bb. The first takes the result with -rpo (on). Returns [surface, node].", |s, a| {
            let names = objects(s, a);
            let g = &mut s.graph;
            if names.len() != 2 {
                fail!("attachSurface: name or select two surfaces");
            }
            let other = surface_arg(g, &names[1], "attachSurface")?;
            let first = surface_arg(g, &names[0], "attachSurface")?;
            let tr = crate::dag::transform_of(g, first);
            // The second surface, as it sits in the first's space.
            let inv = crate::rig::world(g, tr).inverse();
            let o = world_surface(g, other).unwrap_or_default().transformed(&inv);
            let set = vec![
                (P0, PlugValue::Int(a.int("method", 0))),
                (P0 + 1, PlugValue::Float(a.float("blendBias", 0.5))),
                (P0 + 2, PlugValue::Int(a.int("direction", 1))),
                (P0 + 3, to_value(o)),
            ];
            let mut b = a.clone();
            b.positional = vec![json!(names[0])];
            run_op(s, &b, "attachSurface", "attachSurface", &set)
        })
        .positional("two surfaces (the selection when none)")
        .flag(f("method", "m", ArgType::Int, "0 connect, 1 blend (0)."))
        .flag(f("blendBias", "bb", ArgType::Float, "Where a blend meets (0.5)."))
        .flag(f("direction", "d", ArgType::Int, "1 U, 0 V (1)."))
        .flag(f("keepMultipleKnots", "kmk", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("blendKnotInsertion", "bki", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("parameter", "p", ArgType::Float, "Accepted for existing scripts.")),
    ));

    c.register(curve_surface_flags(
        Cmd::new("boundary", "A surface inside three or four curves, in order round the edge. Returns [boundarySurface1, boundary1].", |s, a| from_curves_cmd(s, a, "boundary", "boundary", "boundarySurface1", "boundary1", 3, 4))
            .positional("three or four curves (the selection when none)")
            .flag(f("order", "or", ArgType::Bool, "Accepted for existing scripts."))
            .flag(f("endPoint", "ep", ArgType::Int, "Accepted for existing scripts."))
            .flag(f("endPointTolerance", "ept", ArgType::Float, "Accepted for existing scripts.")),
    ));
    c.register(curve_surface_flags(
        Cmd::new("square", "A surface inside four curves, in order round the edge. Returns [squareSurface1, squareSrf1].", |s, a| from_curves_cmd(s, a, "square", "squareSrf", "squareSurface1", "squareSrf1", 4, 4))
            .positional("four curves (the selection when none)")
            .flag(f("degree", "d", ArgType::Int, "Accepted for existing scripts."))
            .flag(f("continuityType1", "cp", ArgType::Int, "Accepted for existing scripts."))
            .flag(f("endPointTolerance", "ept", ArgType::Float, "Accepted for existing scripts."))
            .flag(f("rebuildCurve1", "rc1", ArgType::Bool, "Accepted for existing scripts.")),
    ));
    for name in ["singleProfileBirailSurface", "birail"] {
        c.register(curve_surface_flags(
            Cmd::new(name, "A profile along two rails, its ends staying on them: profile, rail, rail. Returns [birailSurface1, singleProfileBirailSurface1].", |s, a| {
                from_curves_cmd(s, a, "singleProfileBirailSurface", "singleProfileBirailSurface", "birailSurface1", "singleProfileBirailSurface1", 3, 3)
            })
            .positional("a profile and two rails (the selection when none)")
            .flag(f("tangentContinuityProfile1", "tp1", ArgType::Bool, "Accepted for existing scripts."))
            .flag(f("transformMode", "tm", ArgType::Int, "Accepted for existing scripts.")),
        ));
    }

    c.register(
        Cmd::new(
            "nurbsToPoly",
            "Convert surfaces to polygons with a nurbsTessellate node. -f 0 count (-pc), 1 standard fit (-ft), 2 general (-ut -un -vt -vn), 3 CVs; -pt 0 \
             triangles, 1 quads. Returns [nurbsToPoly1, nurbsTessellate1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                if names.is_empty() {
                    fail!("nurbsToPoly: name or select a surface");
                }
                let ch = wants_history(g, a);
                let mut shapes = Vec::new();
                for n in &names {
                    let id = find(g, n, "nurbsToPoly")?;
                    let under = surfaces_under(g, id);
                    if under.is_empty() {
                        fail!("nurbsToPoly: {n} is not a NURBS surface");
                    }
                    shapes.extend(under);
                }
                let set = vec![
                    (2, PlugValue::Int(a.int("format", 1))),
                    (3, PlugValue::Int(a.int("polygonType", 0))),
                    (4, PlugValue::Int(a.int("polygonCount", 200))),
                    (5, PlugValue::Float(a.float("fractionalTolerance", 0.01))),
                    (6, PlugValue::Int(a.int("uType", 3))),
                    (7, PlugValue::Int(a.int("uNumber", 3))),
                    (8, PlugValue::Int(a.int("vType", 3))),
                    (9, PlugValue::Int(a.int("vNumber", 3))),
                ];
                let name = a.string("name", "nurbsToPoly1");
                let mut out = Vec::new();
                let mut made = Vec::new();
                for sh in shapes {
                    let (tr, node) = tessellate_plug(g, &plug(sh, SurfaceShape::WORLD_SPACE), &name, &set)?;
                    g.evaluate();
                    made.push(tr);
                    out.push(json!(g.path(tr)));
                    if ch {
                        out.push(json!(g.path(node)));
                    } else if let Some(m) = components::as_mesh(g, tr) {
                        crate::history::delete_history(g, m)?;
                    }
                }
                g.select(&made, SelectMode::Replace);
                Ok(Value::Array(out))
            },
        )
        .positional("surfaces (the selection when none)")
        .flag(f("format", "f", ArgType::Int, "0 count, 1 standard fit, 2 general, 3 CVs (1)."))
        .flag(f("polygonType", "pt", ArgType::Int, "0 triangles, 1 quads (0)."))
        .flag(f("polygonCount", "pc", ArgType::Int, "Polygons, for -f 0 (200)."))
        .flag(f("fractionalTolerance", "ft", ArgType::Float, "Tolerance for -f 1 (0.01)."))
        .flag(f("uType", "ut", ArgType::Int, "1 per surface in 3D, 2 per surface, 3 per span (3)."))
        .flag(f("uNumber", "un", ArgType::Int, "How many along U (3)."))
        .flag(f("vType", "vt", ArgType::Int, "1 per surface in 3D, 2 per surface, 3 per span (3)."))
        .flag(f("vNumber", "vn", ArgType::Int, "How many along V (3)."))
        .flag(f("chordHeightRatio", "chr", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("minEdgeLength", "mel", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("delta", "d", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("useChordHeight", "uch", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("useChordHeightRatio", "ucr", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("chordHeight", "cht", ArgType::Float, "Accepted for existing scripts."))
        .flag(f("edgeSwap", "es", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("normalizeTrimmedUVRange", "ntr", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("matchRenderTessellation", "mrt", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("useSurfaceShader", "uss", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("matchNormalDir", "mnd", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("name", "n", ArgType::String, "Name of the mesh.")),
    );

    c.register(
        Cmd::new(
            "duplicateCurve",
            "A curve from an isoparm: duplicateCurve surface1.u[0.5] or surface1.v[1], with a curveFromSurfaceIso node. A plain curve is copied. Returns [curve1, curveFromSurfaceIso1].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let Some(n) = names.first() else { fail!("duplicateCurve: name an isoparm, such as surface1.u[0.5]") };
                let ch = wants_history(g, a);
                let name = a.string("name", "");
                let Some((shape, dir, value)) = parse_isoparm(g, n) else {
                    let shape = curve_shape(g, n, "duplicateCurve")?;
                    let c = crate::curves::world_curve(g, shape).unwrap_or_default();
                    let (tr, _) = crate::curves::create_curve(g, &name, c, true)?;
                    return Ok(json!([g.path(tr)]));
                };
                let t = types::find("curveFromSurfaceIso").expect("curveFromSurfaceIso");
                let node = g.create_node(&t, "curveFromSurfaceIso1", NO_NODE, true)?;
                let from = if a.bool("local") { SurfaceShape::LOCAL } else { SurfaceShape::WORLD_SPACE };
                g.connect(&plug(shape, from), &plug(node, 0), false)?;
                g.set_value(plug(node, 2), PlugValue::Float(value))?;
                g.set_value(plug(node, 3), PlugValue::Int(dir))?;
                let (tr, sh) = crate::dag::create_with_shape(g, crate::curves::SHAPE_TYPE, &name, NO_NODE)?;
                g.connect(&plug(node, 1), &plug(sh, crate::curves::CurveShape::CREATE), false)?;
                g.select(&[tr], SelectMode::Replace);
                g.evaluate();
                if !ch {
                    crate::curves::bake_history(g, sh)?;
                    return Ok(json!([g.path(tr)]));
                }
                Ok(json!([g.path(tr), g.path(node)]))
            },
        )
        .positional("an isoparm or a curve (the selection when none)")
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("local", "l", ArgType::Bool, "Follow the surface in object space, not the world."))
        .flag(f("range", "rn", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("relative", "r", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name of the curve.")),
    );

    c.register(
        Cmd::new(
            "pointOnSurface",
            "Facts at parameters of a surface, world space: -u -v (-top fractions of the range), asking -p position, -no normal, -nn unit normal, -tu -tv \
             tangents, -ntu -ntv unit tangents (position when none).",
            |s, a| {
                let names = objects(s, a);
                let Some(n) = names.first() else { fail!("pointOnSurface: name or select a surface") };
                let g = &mut s.graph;
                let shape = surface_arg(g, n, "pointOnSurface")?;
                let Some(c) = world_surface(g, shape) else { fail!("pointOnSurface: {n} has no surface") };
                let (u, v) = (a.float("parameterU", 0.0), a.float("parameterV", 0.0));
                let (u, v) = if a.bool("turnOnPercentage") { percent(&c, u, v) } else { (u, v) };
                let (p, pu, pv) = c.eval_ders(u, v);
                let r = if a.bool("normal") {
                    pu.cross(pv)
                } else if a.bool("normalizedNormal") {
                    c.normal(u, v)
                } else if a.bool("tangentU") {
                    pu
                } else if a.bool("tangentV") {
                    pv
                } else if a.bool("normalizedTangentU") {
                    pu.normalize_or_zero()
                } else if a.bool("normalizedTangentV") {
                    pv.normalize_or_zero()
                } else {
                    p
                };
                Ok(json!([r.x, r.y, r.z]))
            },
        )
        .undoable(false)
        .positional("a surface (the selection when none)")
        .flag(f("parameterU", "u", ArgType::Float, "The U parameter."))
        .flag(f("parameterV", "v", ArgType::Float, "The V parameter."))
        .flag(f("turnOnPercentage", "top", ArgType::Bool, "The parameters are fractions 0 to 1 of the range."))
        .flag(f("position", "p", ArgType::Bool, "Ask for the point."))
        .flag(f("normal", "no", ArgType::Bool, "Ask for the normal."))
        .flag(f("normalizedNormal", "nn", ArgType::Bool, "Ask for the unit normal."))
        .flag(f("tangentU", "tu", ArgType::Bool, "Ask for the U derivative."))
        .flag(f("tangentV", "tv", ArgType::Bool, "Ask for the V derivative."))
        .flag(f("normalizedTangentU", "ntu", ArgType::Bool, "Ask for the unit U tangent."))
        .flag(f("normalizedTangentV", "ntv", ArgType::Bool, "Ask for the unit V tangent."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Accepted for existing scripts.")),
    );

    c.register(
        Cmd::new(
            "selectType",
            "Which components are picked: -cv, -v or -pv for points, -pe edges, -pf faces, each true or false; -q asks. Component modes do the masking here, so turning \
             one on switches to its mode.",
            |s, a| {
                let g = &mut s.graph;
                let kinds = [("controlVertex", ComponentMode::Vertex), ("vertex", ComponentMode::Vertex), ("polymeshVertex", ComponentMode::Vertex), ("polymeshEdge", ComponentMode::Edge), ("polymeshFace", ComponentMode::Face)];
                if a.has("query") && a.bool("query") {
                    for (flag, mode) in kinds {
                        if a.has(flag) {
                            return Ok(json!(components::mode(g) == mode));
                        }
                    }
                    return Ok(json!(false));
                }
                for (flag, mode) in kinds {
                    if a.has(flag) && a.bool(flag) {
                        components::set_mode(g, mode);
                    }
                }
                Ok(Value::Null)
            },
        )
        .undoable(false)
        .flag(f("controlVertex", "cv", ArgType::Bool, "CVs."))
        .flag(f("vertex", "v", ArgType::Bool, "Points."))
        .flag(f("polymeshVertex", "pv", ArgType::Bool, "Mesh vertices."))
        .flag(f("polymeshEdge", "pe", ArgType::Bool, "Mesh edges."))
        .flag(f("polymeshFace", "pf", ArgType::Bool, "Mesh faces."))
        .flag(f("query", "q", ArgType::Bool, "Ask rather than set.")),
    );

    c.register(
        Cmd::new(
            "displaySmoothness",
            "How finely surfaces draw: -du -dv isoparms within a span, -pw points to a drawn span, -ps shaded fineness. Acts on the objects named or selected, or every surface \
             with -all.",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                g.evaluate();
                let mut shapes = Vec::new();
                if a.bool("all") {
                    shapes = g.nodes().into_iter().filter(|&n| is_surface(g, n)).collect();
                } else {
                    // The hotkeys send this for whatever's selected, components and meshes included: those are skipped.
                    for n in &names {
                        if let Ok(Some(id)) = g.find(n) {
                            shapes.extend(surfaces_under(g, id));
                        }
                    }
                }
                for sh in shapes {
                    for (flag, attr) in [("divisionsU", SurfaceShape::DIVISIONS_U), ("divisionsV", SurfaceShape::DIVISIONS_V), ("pointsWire", SurfaceShape::CURVE_PRECISION), ("pointsShaded", SurfaceShape::CURVE_PRECISION_SHADED)] {
                        if a.has(flag) {
                            g.set_value(plug(sh, attr), PlugValue::Int(a.int(flag, 0)))?;
                        }
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects (the selection when none)")
        .flag(f("divisionsU", "du", ArgType::Int, "Isoparms within a span along U."))
        .flag(f("divisionsV", "dv", ArgType::Int, "Isoparms within a span along V."))
        .flag(f("pointsWire", "pw", ArgType::Int, "Points to a drawn span."))
        .flag(f("pointsShaded", "ps", ArgType::Int, "Shaded fineness."))
        .flag(f("polygonObject", "po", ArgType::Int, "Accepted for existing scripts."))
        .flag(f("all", "all", ArgType::Bool, "Every surface.")),
    );
}
