//! softMod: a handle that moves points by a falloff, from the selected points or a centre. Like a cluster whose weights
//! come from distance, worked out each time from where the points are.

use std::any::Any;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short};
use crate::components::{self, ComponentMode};
use crate::deform::{self, data_attr, f, floats_of, insert_at, json_of, mat_from, mat_json, matrix_in, plug, set_data, set_floats, weight_at, Cmd, Place, BASE_COUNT, DATA, ENVELOPE, GEOM, HANDLE, IN, OUT};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType, IN_STORE};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::world;
use crate::session::Session;
use crate::skin::modes;
use crate::transform::{Shape, Transform};

const RADIUS: usize = BASE_COUNT;
const MODE: usize = BASE_COUNT + 1;
const CENTER: usize = BASE_COUNT + 2;
const AROUND: usize = BASE_COUNT + 3;
const RELATIVE: usize = BASE_COUNT + 4;
const IN_X: usize = BASE_COUNT + 5;
const LOCAL: usize = BASE_COUNT + 8;
const SELECTED: usize = BASE_COUNT + 9;
const CURVE_POS: usize = BASE_COUNT + 10;
const CURVE_VAL: usize = BASE_COUNT + 11;
const CURVE_INTERP: usize = BASE_COUNT + 12;

/// One falloffCurve entry, as the reference's ramps: position along the radius, weight, then how it goes on to the next: 0 none,
/// 1 linear, 2 smooth, 3 spline.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Knot {
    at: f64,
    value: f64,
    interp: i64,
}

fn smoothstep(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn curve_value(knots: &[Knot], t: f64) -> f64 {
    let Some(first) = knots.first() else { return 1.0 - smoothstep(t) };
    if t <= first.at {
        return first.value;
    }
    let i = knots.iter().rposition(|k| k.at <= t).unwrap_or(0);
    let a = knots[i];
    let Some(&b) = knots.get(i + 1) else { return a.value };
    let span = (b.at - a.at).max(1e-12);
    let x = (t - a.at) / span;
    match a.interp {
        0 => a.value,
        1 => a.value + (b.value - a.value) * x,
        3 => {
            // Catmull-Rom through the neighbours.
            let before = if i > 0 { knots[i - 1].value } else { a.value };
            let after = knots.get(i + 2).map_or(b.value, |k| k.value);
            let (m0, m1) = ((b.value - before) * 0.5, (after - a.value) * 0.5);
            let (x2, x3) = (x * x, x * x * x);
            (2.0 * x3 - 3.0 * x2 + 1.0) * a.value + (x3 - 2.0 * x2 + x) * m0 + (-2.0 * x3 + 3.0 * x2) * b.value + (x3 - x2) * m1
        }
        _ => a.value + (b.value - a.value) * smoothstep(x),
    }
}

fn knots_of(d: &NodeData) -> Vec<Knot> {
    let mut out: Vec<(i32, Knot)> = d
        .elements(CURVE_POS)
        .iter()
        .map(|(i, v)| {
            let value = d.element(CURVE_VAL, *i).map_or(0.0, |x| x.as_float());
            let interp = d.element(CURVE_INTERP, *i).map_or(2, |x| x.as_int());
            (*i, Knot { at: v.as_float(), value, interp })
        })
        .collect();
    out.sort_by(|a, b| a.1.at.partial_cmp(&b.1.at).unwrap_or(Ordering::Equal));
    out.into_iter().map(|(_, k)| k).collect()
}

#[derive(PartialEq, Clone, Copy)]
struct Near(f64, u32);
impl Eq for Near {}
impl Ord for Near {
    fn cmp(&self, o: &Self) -> Ordering {
        o.0.partial_cmp(&self.0).unwrap_or(Ordering::Equal)
    }
}
impl PartialOrd for Near {
    fn partial_cmp(&self, o: &Self) -> Option<Ordering> {
        Some(self.cmp(o))
    }
}

/// Distance from the seeds along the mesh's edges, for points up to the radius.
fn along_surface(m: &Mesh, world_pts: &[DVec3], seeds: &[u32], radius: f64) -> Vec<f64> {
    let mut dist = vec![f64::MAX; world_pts.len()];
    let mut heap = BinaryHeap::new();
    for &s in seeds {
        if let Some(d) = dist.get_mut(s as usize) {
            *d = 0.0;
            heap.push(Near(0.0, s));
        }
    }
    while let Some(Near(d, p)) = heap.pop() {
        if d > dist[p as usize] || d > radius {
            continue;
        }
        m.for_each_point_edge(p, |e| {
            let q = m.edge_other_point(e, p);
            let next = d + (world_pts[p as usize] - world_pts[q as usize]).length();
            if next < dist[q as usize] {
                dist[q as usize] = next;
                heap.push(Near(next, q));
            }
        });
    }
    dist
}

/// What the falloff depends on, to tell when the weights must be worked out again.
#[derive(PartialEq, Default)]
struct Key {
    points: Vec<DVec3>,
    radius: f64,
    surface: bool,
    center: DVec3,
    seeds: Vec<u32>,
    axes: [bool; 3],
    knots: Vec<Knot>,
}

#[derive(Default)]
struct SoftMod {
    key: Key,
    falloff: Vec<f64>,
}

fn falloff(m: &Mesh, k: &Key) -> Vec<f64> {
    let radius = k.radius.max(1e-9);
    let mask = DVec3::new(k.axes[0] as u8 as f64, k.axes[1] as u8 as f64, k.axes[2] as u8 as f64);
    let dist: Vec<f64> = if k.surface {
        let seeds = if k.seeds.is_empty() {
            let nearest = (0..m.point_capacity()).filter(|&p| m.point_valid(p)).min_by(|&a, &b| {
                let d = |p: u32| (k.points[p as usize] - k.center).length_squared();
                d(a).partial_cmp(&d(b)).unwrap_or(Ordering::Equal)
            });
            nearest.into_iter().collect()
        } else {
            k.seeds.clone()
        };
        along_surface(m, &k.points, &seeds, radius)
    } else {
        k.points
            .iter()
            .map(|q| {
                if k.seeds.is_empty() {
                    ((*q - k.center) * mask).length()
                } else {
                    k.seeds.iter().filter_map(|&s| k.points.get(s as usize)).map(|s| ((*q - *s) * mask).length()).fold(f64::MAX, f64::min)
                }
            })
            .collect()
    };
    dist.into_iter().map(|x| if x >= radius { 0.0 } else { curve_value(&k.knots, x / radius) }).collect()
}

impl NodeBehavior for SoftMod {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let geom = d.matrix(GEOM);
        let points: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
        let around = d.bool(AROUND);
        let key = Key {
            points,
            radius: d.float(RADIUS),
            surface: d.int(MODE) == 1,
            center: d.vec3(CENTER),
            seeds: if around { floats_of(d, SELECTED).map(|s| s.0.iter().map(|x| *x as u32).collect()).unwrap_or_default() } else { Vec::new() },
            axes: [d.bool(IN_X), d.bool(IN_X + 1), d.bool(IN_X + 2)],
            knots: knots_of(d),
        };
        if key != self.key || self.falloff.len() != input.point_capacity() as usize {
            self.falloff = falloff(&input, &key);
            self.key = key;
        }
        let data = json_of(d, DATA);
        let h = if d.bool(RELATIVE) {
            d.matrix(LOCAL) * mat_from(data.get("bindLocal").unwrap_or(&Value::Null))
        } else {
            d.matrix(HANDLE) * mat_from(data.get("bindPre").unwrap_or(&Value::Null))
        };
        let gi = geom.inverse();
        let env = d.float(ENVELOPE);
        let weights_owned = crate::deform::weights_of(d, &input);
        let weights = weights_owned.as_deref();
        let mut m = (*input).clone();
        for p in 0..m.point_capacity() {
            let w = self.falloff[p as usize] * weight_at(weights, p, env);
            if w == 0.0 || !m.point_valid(p) {
                continue;
            }
            let q = self.key.points[p as usize];
            let at = m.position(p);
            m.set_position(p, at + gi.transform_vector3(h.transform_point3(q) - q) * w);
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct SoftModHandle;

impl NodeBehavior for SoftModHandle {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn curve_attr(child: &str, def: f64, doc: &str) -> AttrDef {
    AttrDef {
        name: format!("falloffCurve.falloffCurve_{child}"),
        ty: if child == "Interp" { PlugType::Int } else { PlugType::Float },
        default_value: if child == "Interp" { PlugValue::Int(def as i64) } else { PlugValue::Float(def) },
        flags: flags::INPUT | flags::STORABLE | flags::ARRAY,
        doc: doc.into(),
        ..Default::default()
    }
}

pub(crate) fn register_types(gf: Arc<NodeType>) {
    let shape = types::find("shape").expect("shape");
    types::register_type(NodeType {
        name: "softModHandle".into(),
        base: Some(shape),
        dag: true,
        icon: "circle-letter-s".into(),
        doc: "Draws a softMod's handle.".into(),
        own_attrs: vec![vec3_attr("origin", "or", DVec3::ZERO, IN_STORE, "Where the handle is drawn, object space.")],
        create: Some(Arc::new(|| Box::new(SoftModHandle))),
        ..Default::default()
    })
    .expect("softModHandle");
    let axis = |name: &str, short: &str, doc: &str| AttrDef { short_name: short.into(), ..bool_attr(name, true, doc) };
    types::register_type(NodeType {
        name: "softMod".into(),
        base: Some(gf),
        icon: "circle-letter-s".into(),
        doc: "Moves points with a handle, by how far they are from its centre or from the selected points.".into(),
        own_attrs: vec![
            float_attr_short("falloffRadius", "fr", 5.0, "How far the effect reaches, cm.", Some(0.0), None),
            AttrDef { short_name: "fom".into(), ..enum_attr("falloffMode", &["Volume", "Surface"], 0, "Measure straight through space, or along the surface.") },
            vec3_attr("falloffCenter", "fc", DVec3::ZERO, IN_STORE, "Centre of the falloff, world space."),
            AttrDef { short_name: "fas".into(), ..bool_attr("falloffAroundSelection", true, "Measure from the points selected when it was made, not from the centre.") },
            AttrDef { short_name: "rel".into(), ..bool_attr("relative", false, "Only the handle's own transform counts, not its parents'.") },
            axis("falloffInX", "fix", "Distance counts along X."),
            axis("falloffInY", "fiy", "Distance counts along Y."),
            axis("falloffInZ", "fiz", "Distance counts along Z."),
            matrix_in("handleLocalMatrix", "hlm", "The handle's local matrix."),
            data_attr("selectedPoints", "sep", "The points selected when it was made."),
            curve_attr("Position", 0.0, "Where along the radius, 0 to 1."),
            curve_attr("FloatValue", 1.0, "The weight there."),
            curve_attr("Interp", 2.0, "On to the next: 0 none, 1 linear, 2 smooth, 3 spline."),
        ],
        create: Some(Arc::new(|| Box::<SoftMod>::default())),
        ..Default::default()
    })
    .expect("softMod");
}

/// Point ids mean nothing on another mesh: a further mesh falls off around the centre.
pub(crate) fn clear_selected(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    g.set_value(plug(n, SELECTED), PlugValue::Data(None))?;
    Ok(())
}

fn soft_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let id = crate::rig::find(g, name, "softMod")?;
    if g.node(id).is_some_and(|n| n.ty().is_a("softMod")) {
        return Ok(id);
    }
    match deform::driven_deformers(g, id).into_iter().find(|&d| g.node(d).is_some_and(|n| n.ty().is_a("softMod"))) {
        Some(d) => Ok(deform::leader_of(g, d)),
        None => fail!("softMod: {name} is not a softMod"),
    }
}

const SETTINGS: &[&str] = &["falloffRadius", "falloffMode", "falloffCenter", "falloffAroundSelection", "relative", "envelope"];

fn create(s: &mut Session, a: &Args) -> CmdResult {
    let meshes = deform::targets(s, a)?;
    if meshes.is_empty() {
        fail!("softMod: select meshes or vertices");
    }
    let g = &mut s.graph;
    g.evaluate();
    // The middle of the selected points, or of the objects.
    let (mut sum, mut count) = (DVec3::ZERO, 0.0);
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for (m, pts) in &meshes {
        let (l, h) = deform::world_bounds(g, *m);
        lo = lo.min(l);
        hi = hi.max(h);
        if let (Some(pts), Some(cage)) = (pts, crate::modifiers::cage(g, *m)) {
            let w = world(g, crate::dag::transform_of(g, *m));
            for &p in pts {
                if cage.point_valid(p) {
                    sum += w.transform_point3(cage.position(p));
                    count += 1.0;
                }
            }
        }
    }
    let centre = if count > 0.0 { sum / count } else { (lo + hi) * 0.5 };
    let centre = a.vec3("falloffCenter").map_or(centre, DVec3::from_array);
    let name = a.string("name", "softMod1");
    g.open_chunk("Soft Modification");
    let r = (|| -> CmdResult<(NodeId, NodeId)> {
        let n = insert_at(g, meshes[0].0, "softMod", &name, Place::from_args(a))?;
        let stem = g.node(n).unwrap().name().to_string();
        let (xf, shape) = crate::dag::create_with_shape(g, "softModHandle", &format!("{stem}Handle"), NO_NODE)?;
        // The handle stays at zero with its pivot on the centre, so its translate is how far it has moved.
        g.set_value(plug(shape, Shape::COUNT), PlugValue::Vec3(centre))?;
        g.set_value(plug(xf, Transform::ROTATE_PIVOT), PlugValue::Vec3(centre))?;
        g.set_value(plug(xf, Transform::SCALE_PIVOT), PlugValue::Vec3(centre))?;
        g.connect(&plug(xf, Transform::WORLD_MATRIX), &plug(n, HANDLE), false)?;
        g.connect(&plug(xf, Transform::MATRIX), &plug(n, LOCAL), false)?;
        g.set_value(plug(n, CENTER), PlugValue::Vec3(centre))?;
        deform::apply_flags(g, n, a, SETTINGS)?;
        g.evaluate();
        let bind: DMat4 = world(g, xf).inverse();
        let local = match g.value(&plug(xf, Transform::MATRIX)) {
            Ok(PlugValue::Matrix(m)) => m.inverse(),
            _ => DMat4::IDENTITY,
        };
        set_data(g, n, json!({"bindPre": mat_json(&bind), "bindLocal": mat_json(&local)}))?;
        // The curve the reference starts with.
        for (i, (at, value)) in [(0.0, 1.0), (1.0, 0.0)].into_iter().enumerate() {
            g.set_value(PlugRef::element(n, CURVE_POS as i32, i as i32), PlugValue::Float(at))?;
            g.set_value(PlugRef::element(n, CURVE_VAL as i32, i as i32), PlugValue::Float(value))?;
            g.set_value(PlugRef::element(n, CURVE_INTERP as i32, i as i32), PlugValue::Int(2))?;
        }
        for (k, (m, pts)) in meshes.iter().enumerate() {
            let at = if k == 0 { n } else { deform::add_geometry(g, n, *m, &None)? };
            match pts {
                Some(pts) => set_floats(g, at, SELECTED, pts.iter().map(|&p| p as f64).collect())?,
                None => {}
            }
        }
        components::set_mode_undoable(g, ComponentMode::Object);
        g.select(&[xf], SelectMode::Replace);
        Ok((n, xf))
    })();
    g.close_chunk();
    let (n, xf) = r?;
    Ok(json!([g.path(n), g.path(xf)]))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        modes(deform::common_flags(Cmd::new(
            "softMod",
            "Add a soft modification to the selected vertices or meshes: softMod -fr 2 -fas true. Returns [softMod1, softMod1Handle]. Move \
             the handle and points follow by their falloff. With vertices it falls off around them; with a mesh, around its centre or \
             -fc. -e and -q on the softMod or handle: -fr, -fom, -fc, -fas, -rel, -en.",
            |s, a| {
                if a.bool("query") {
                    let Some(name) = a.named().into_iter().next() else { fail!("softMod: name a softMod to query") };
                    let n = soft_named(&s.graph, &name)?;
                    return deform::query_flags(&mut s.graph, n, a, SETTINGS);
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("softMod: name a softMod to edit") };
                    let n = soft_named(&s.graph, &name)?;
                    deform::apply_flags(&mut s.graph, n, a, SETTINGS)?;
                    return Ok(json!([s.graph.path(n)]));
                }
                create(s, a)
            },
        )))
        .positional("vertices or meshes (the selection when none); the softMod with -e and -q")
        .flag(f("falloffRadius", "fr", ArgType::Float, "How far the effect reaches, cm (5)."))
        .flag(f("falloffMode", "fom", ArgType::Int, "0 straight through space, 1 along the surface."))
        .flag(f("falloffCenter", "fc", ArgType::Vec3, "Centre of the falloff, world space."))
        .flag(f("falloffAroundSelection", "fas", ArgType::Bool, "Fall off around the selected points rather than the centre (on)."))
        .flag(f("relative", "rel", ArgType::Bool, "Only the handle's own transform counts, not its parents'.")),
    );
}
