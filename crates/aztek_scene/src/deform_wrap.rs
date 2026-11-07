//! wrap: a mesh follows other meshes, its influences. Each point binds to the nearest triangles of the influences, held as
//! a place on the triangle plus an offset in the triangle's own frame, so it turns and moves with them.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::{DMat4, DVec3};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short, mesh_attr};
use crate::components;
use crate::deform::{self, data_attr, f, flat, floats_of, input_mesh, insert_at, plug, set_floats, unflat, weight_at, Cmd, Place, BASE_COUNT, ENVELOPE, GEOM, IN, OUT, WEIGHTS};
use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;
use crate::skin::{asked, modes};
use crate::transform::Transform;

const MAX_DISTANCE: usize = BASE_COUNT;
const EXCLUSIVE: usize = BASE_COUNT + 1;
const AUTO_THRESHOLD: usize = BASE_COUNT + 2;
const THRESHOLD: usize = BASE_COUNT + 3;
const FALLOFF_MODE: usize = BASE_COUNT + 4;
const DRIVER_POINTS: usize = BASE_COUNT + 5;
const DRIVER_MATRIX: usize = BASE_COUNT + 6;
const BIND: usize = BASE_COUNT + 7;
const REST: usize = BASE_COUNT + 8;

/// How many triangles a point can bind to.
const MOST_TRIANGLES: usize = 8;

/// One binding, as eleven numbers in the bind data: point, influence element, triangle's three points, the place on it
/// as two barycentric coordinates, the offset in its frame, weight.
const STRIDE: usize = 11;

/// A triangle's frame at a place on it: origin there, X along its first edge, Z its normal.
fn frame(a: DVec3, b: DVec3, c: DVec3, u: f64, v: f64) -> Option<(DVec3, DVec3, DVec3, DVec3)> {
    let x = (b - a).normalize_or_zero();
    let n = (b - a).cross(c - a).normalize_or_zero();
    if x == DVec3::ZERO || n == DVec3::ZERO {
        return None;
    }
    Some((a + (b - a) * u + (c - a) * v, x, n.cross(x), n))
}

/// The nearest place to p on a triangle: barycentric u along b - a and v along c - a.
fn closest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> (f64, f64) {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return (0.0, 0.0);
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return (1.0, 0.0);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return (d1 / (d1 - d3), 0.0);
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return (0.0, 1.0);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return (0.0, d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let t = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (1.0 - t, t);
    }
    let denom = 1.0 / (va + vb + vc);
    (vb * denom, vc * denom)
}

struct Wrap;

impl NodeBehavior for Wrap {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let (Some(bind), Some(rest)) = (floats_of(d, BIND), floats_of(d, REST)) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let rest = unflat(&rest.0);
        // Each influence's points in world space.
        let mut drivers: Vec<(i32, Vec<DVec3>)> = Vec::new();
        for (i, v) in d.elements(DRIVER_POINTS) {
            let Some(mesh) = v.as_mesh() else { continue };
            let w = d.element(DRIVER_MATRIX, *i).map_or(DMat4::IDENTITY, |m| m.as_matrix());
            drivers.push((*i, mesh.positions().iter().map(|p| w.transform_point3(*p)).collect()));
        }
        let mut sum = vec![DVec3::ZERO; input.point_capacity() as usize];
        let mut bound = vec![false; sum.len()];
        for e in bind.0.chunks_exact(STRIDE) {
            let (p, inf) = (e[0] as usize, e[1] as i32);
            let Some((_, pts)) = drivers.iter().find(|(i, _)| *i == inf) else { continue };
            let (Some(&a), Some(&b), Some(&c)) = (pts.get(e[2] as usize), pts.get(e[3] as usize), pts.get(e[4] as usize)) else { continue };
            // Bound to a point of a driver with no faces, such as a curve: it keeps its offset.
            if e[2] == e[3] && e[3] == e[4] {
                if p < sum.len() {
                    sum[p] += (a + DVec3::new(e[7], e[8], e[9])) * e[10];
                    bound[p] = true;
                }
                continue;
            }
            let Some((o, x, y, n)) = frame(a, b, c, e[5], e[6]) else { continue };
            if p < sum.len() {
                sum[p] += (o + x * e[7] + y * e[8] + n * e[9]) * e[10];
                bound[p] = true;
            }
        }
        let gi = d.matrix(GEOM).inverse();
        let env = d.float(ENVELOPE);
        let weights = floats_of(d, WEIGHTS);
        let mut m = (*input).clone();
        for p in 0..m.point_capacity() {
            let w = weight_at(weights, p, env);
            let i = p as usize;
            if w == 0.0 || !m.point_valid(p) || !bound[i] || i >= rest.len() {
                continue;
            }
            let at = m.position(p);
            m.set_position(p, at + gi.transform_vector3(sum[i] - rest[i]) * w);
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_types(gf: Arc<NodeType>) {
    let array = |a: AttrDef| AttrDef { flags: a.flags | flags::ARRAY, ..a };
    types::register_type(NodeType {
        name: "wrap".into(),
        base: Some(gf),
        icon: "box-model".into(),
        doc: "Makes a mesh follow the meshes wrapped round it.".into(),
        own_attrs: vec![
            float_attr_short("maxDistance", "md", 1.0, "How far a point looks for triangles to bind to, cm; 0 has no limit.", Some(0.0), None),
            AttrDef { short_name: "ebn".into(), ..bool_attr("exclusiveBind", false, "Each point binds to its nearest triangle only.") },
            AttrDef { short_name: "awt".into(), ..bool_attr("autoWeightThreshold", true, "Every point binds to its nearest triangle, however far.") },
            float_attr_short("weightThreshold", "wth", 0.0, "With autoWeightThreshold off: drop bindings of less weight than this.", Some(0.0), Some(1.0)),
            AttrDef { short_name: "fom".into(), ..enum_attr("falloffMode", &["Volume", "Surface"], 0, "Bind to any triangle in reach, or only ones touching the nearest.") },
            array(mesh_attr("driverPoints", "dp", flags::INPUT | flags::HIDDEN, "The influences' meshes.")),
            AttrDef { name: "driverMatrix".into(), short_name: "dm".into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: flags::INPUT | flags::HIDDEN | flags::ARRAY, doc: "The influences' world matrices.".into(), ..Default::default() },
            data_attr("wrapBind", "wb", "Each point's bindings."),
            data_attr("restPoints", "rp", "The points in world space when bound."),
        ],
        create: Some(Arc::new(|| Box::new(Wrap))),
        ..Default::default()
    })
    .expect("wrap");
}

/// Binds a wrap's points to its influences as they are now.
pub(crate) fn bind(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    let Some(input) = input_mesh(g, n) else { return Ok(()) };
    let node = g.node(n).unwrap();
    let geom = node.data().matrix(GEOM);
    let (max_distance, exclusive, auto, threshold, surface) = (node.float(MAX_DISTANCE), node.bool(EXCLUSIVE), node.bool(AUTO_THRESHOLD), node.float(THRESHOLD), node.int(FALLOFF_MODE) == 1);
    // Every influence's triangles, in world space.
    let mut tris: Vec<(i32, [u32; 3], [DVec3; 3])> = Vec::new();
    let mut loose: Vec<(i32, u32, DVec3)> = Vec::new();
    for (i, v) in node.elements(DRIVER_POINTS) {
        let Some(m) = v.as_mesh() else { continue };
        let w = node.element(DRIVER_MATRIX, *i).map_or(DMat4::IDENTITY, |x| x.as_matrix());
        if m.num_faces() == 0 {
            loose.extend((0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| (*i, p, w.transform_point3(m.position(p)))));
        }
        for face in (0..m.face_capacity()).filter(|&x| m.face_valid(x)) {
            let pts = m.face_points(face);
            for k in 1..pts.len().saturating_sub(1) {
                let t = [pts[0], pts[k], pts[k + 1]];
                tris.push((*i, t, t.map(|p| w.transform_point3(m.position(p)))));
            }
        }
    }
    let points: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
    let valid: Vec<bool> = (0..input.point_capacity()).map(|p| input.point_valid(p)).collect();
    let bound: Vec<Vec<f64>> = (0..points.len())
        .into_par_iter()
        .map(|p| {
            if !valid[p] {
                return Vec::new();
            }
            let q = points[p];
            if tris.is_empty() {
                let Some(near) = loose.iter().min_by(|a, b| (a.2 - q).length_squared().total_cmp(&(b.2 - q).length_squared())) else { return Vec::new() };
                let (off, id) = (q - near.2, near.1 as f64);
                return vec![p as f64, near.0 as f64, id, id, id, 0.0, 0.0, off.x, off.y, off.z, 1.0];
            }
            let mut near: Vec<(f64, usize, f64, f64)> = tris
                .iter()
                .enumerate()
                .map(|(i, (_, _, t))| {
                    let (u, v) = closest_on_triangle(q, t[0], t[1], t[2]);
                    ((t[0] + (t[1] - t[0]) * u + (t[2] - t[0]) * v - q).length(), i, u, v)
                })
                .collect();
            near.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
            let nearest = tris[near[0].1];
            let keep: Vec<(f64, usize, f64, f64)> = if exclusive {
                vec![near[0]]
            } else {
                let touches = |i: usize| tris[i].0 == nearest.0 && tris[i].1.iter().any(|x| nearest.1.contains(x));
                near.iter().enumerate().filter(|(k, e)| (*k == 0 && auto) || ((max_distance <= 0.0 || e.0 <= max_distance) && (!surface || touches(e.1)))).map(|(_, e)| *e).take(MOST_TRIANGLES).collect()
            };
            // Nearer triangles count for more.
            let raw: Vec<f64> = keep.iter().map(|e| 1.0 / (e.0 * e.0 + 1e-8)).collect();
            let total: f64 = raw.iter().sum();
            let mut chosen: Vec<(usize, f64)> = raw.iter().enumerate().map(|(i, w)| (i, w / total)).filter(|(_, w)| auto || *w >= threshold).collect();
            let kept: f64 = chosen.iter().map(|c| c.1).sum();
            chosen.iter_mut().for_each(|c| c.1 /= kept.max(1e-12));
            let mut out = Vec::with_capacity(chosen.len() * STRIDE);
            for (i, w) in chosen {
                let (_, t, u, v) = keep[i];
                let (inf, ids, at) = &tris[t];
                let Some((o, x, y, nrm)) = frame(at[0], at[1], at[2], u, v) else { continue };
                let off = q - o;
                out.extend([p as f64, *inf as f64, ids[0] as f64, ids[1] as f64, ids[2] as f64, u, v, off.dot(x), off.dot(y), off.dot(nrm), w]);
            }
            out
        })
        .collect();
    set_floats(g, n, BIND, bound.concat())?;
    set_floats(g, n, REST, flat(&points))
}

fn influences(g: &Graph, n: NodeId) -> Vec<(PlugRef, NodeId)> {
    g.elements(&plug(n, DRIVER_POINTS)).into_iter().filter_map(|e| g.source(&e).map(|s| (e, s.node))).collect()
}

fn add_influence(g: &mut Graph, n: NodeId, name: &str) -> CmdResult<()> {
    let id = crate::rig::find(g, name, "wrap")?;
    let Some(mesh) = components::as_mesh(g, id) else { fail!("wrap: {name} is not a mesh") };
    if influences(g, n).iter().any(|(_, m)| *m == mesh) {
        return Ok(());
    }
    let index = g.elements(&plug(n, DRIVER_POINTS)).last().map_or(0, |e| e.element + 1);
    let xf = crate::dag::transform_of(g, mesh);
    g.connect(&plug(mesh, MeshNode::OUT_MESH), &PlugRef::element(n, DRIVER_POINTS as i32, index), false)?;
    g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(n, DRIVER_MATRIX as i32, index), false)?;
    Ok(())
}

fn remove_influence(g: &mut Graph, n: NodeId, name: &str) -> CmdResult<()> {
    let id = crate::rig::find(g, name, "wrap")?;
    let Some(mesh) = components::as_mesh(g, id) else { fail!("wrap: {name} is not a mesh") };
    let Some((e, _)) = influences(g, n).into_iter().find(|(_, m)| *m == mesh) else { fail!("wrap: {name} is not an influence of {}", g.path(n)) };
    let index = e.element;
    g.disconnect(&e);
    g.disconnect(&PlugRef::element(n, DRIVER_MATRIX as i32, index));
    g.remove_element(&e)?;
    g.remove_element(&PlugRef::element(n, DRIVER_MATRIX as i32, index))?;
    Ok(())
}

fn string_list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(a)) => a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

const SETTINGS: &[&str] = &["maxDistance", "exclusiveBind", "autoWeightThreshold", "weightThreshold", "falloffMode", "envelope"];

fn create(s: &mut Session, a: &Args) -> CmdResult {
    let mut names = objects(s, a);
    let mut wrappers = string_list(a.get("influence"));
    // The reference's menu order: the meshes to deform, then the wrapper.
    if wrappers.is_empty() && names.len() > 1 {
        wrappers.push(names.pop().unwrap());
    }
    let g = &mut s.graph;
    let meshes = deform::named_targets(g, &names)?;
    if meshes.is_empty() {
        fail!("wrap: select the meshes to deform, then the wrapper");
    }
    let name = a.string("name", "wrap1");
    g.open_chunk("Wrap");
    let r = (|| -> CmdResult<NodeId> {
        let n = insert_at(g, meshes[0].0, "wrap", &name, Place::from_args(a))?;
        deform::apply_flags(g, n, a, SETTINGS)?;
        for w in &wrappers {
            add_influence(g, n, w)?;
        }
        bind(g, n)?;
        if let Some(input) = input_mesh(g, n) {
            deform::limit_to(g, n, input.point_capacity(), &meshes[0].1)?;
        }
        for (m, pts) in &meshes[1..] {
            deform::add_geometry(g, n, *m, pts)?;
        }
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    Ok(json!([g.path(n)]))
}

fn wrap_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let n = crate::deform_cmds::deformer_named(g, name, "wrap")?;
    if !g.node(n).is_some_and(|x| x.ty().is_a("wrap")) {
        fail!("wrap: {name} is not a wrap");
    }
    Ok(deform::leader_of(g, n))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        modes(deform::common_flags(Cmd::new(
            "wrap",
            "Make meshes follow a wrapper: wrap -inf wrapper mesh, or select the meshes and then the wrapper. Returns [wrap1]. With -e on \
             a wrap (say one from deformer -type wrap): -inf adds an influence and binds again, -ri removes one, -rb binds again as things \
             are now. With -q: -inf and the settings.",
            |s, a| {
                if a.bool("query") {
                    let Some(name) = a.named().into_iter().next() else { fail!("wrap: name a wrap to query") };
                    let n = wrap_named(&s.graph, &name)?;
                    if asked(a, "influence") {
                        let g = &s.graph;
                        return Ok(json!(influences(g, n).into_iter().map(|(_, m)| g.path(m)).collect::<Vec<_>>()));
                    }
                    return deform::query_flags(&mut s.graph, n, a, SETTINGS);
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("wrap: name a wrap to edit") };
                    let g = &mut s.graph;
                    let n = wrap_named(g, &name)?;
                    deform::apply_flags(g, n, a, SETTINGS)?;
                    let (add, remove) = (string_list(a.get("influence")), string_list(a.get("removeInfluence")));
                    for x in deform::group(g, n) {
                        for w in &add {
                            add_influence(g, x, w)?;
                        }
                        for w in &remove {
                            remove_influence(g, x, w)?;
                        }
                        if !add.is_empty() || !remove.is_empty() || a.bool("rebind") {
                            bind(g, x)?;
                        }
                    }
                    return Ok(json!([g.path(n)]));
                }
                create(s, a)
            },
        )))
        .positional("meshes to deform, then the wrapper unless -inf names it (the selection when none); the wrap with -e and -q")
        .flag(f("influence", "inf", ArgType::Any, "The wrapper mesh, or several. With -e: add. Query: the influences."))
        .flag(f("removeInfluence", "ri", ArgType::Any, "With -e: the influence to remove."))
        .flag(f("rebind", "rb", ArgType::Bool, "With -e: bind again as things are now."))
        .flag(f("maxDistance", "md", ArgType::Float, "How far a point looks for triangles to bind to, cm (1); 0 has no limit."))
        .flag(f("exclusiveBind", "ebn", ArgType::Bool, "Each point binds to its nearest triangle only."))
        .flag(f("autoWeightThreshold", "awt", ArgType::Bool, "Every point binds to its nearest triangle, however far (on)."))
        .flag(f("weightThreshold", "wth", ArgType::Float, "With -awt off: drop bindings of less weight than this."))
        .flag(f("falloffMode", "fom", ArgType::Int, "0 any triangle in reach, 1 only ones touching the nearest.")),
    );
}
