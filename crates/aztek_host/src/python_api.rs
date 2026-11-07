//! The native queries the Python API (aztek.api.OpenMaya) is built on: nodes by id, plug descriptions, matrices, bulk
//! mesh data, ray hits and closest points. Everything else it does goes through the commands.

use aztek_geo::normals::{corner_normals, point_normals, CornerNormalOptions, NormalWeight};
use aztek_geo::{Index, Mesh, INVALID};
use aztek_scene::graph::Graph;
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::node_bounds::object_bounds;
use aztek_scene::plug::{flags, AttrDef, MeshPtr, NodeId, PlugRef, PlugType, NO_NODE};
use aztek_scene::scripted::value_to_json;
use aztek_scene::transform::{DagNode, Transform};
use aztek_scene::{components, curves, skin};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::core::Core;

fn type_name(t: PlugType) -> &'static str {
    match t {
        PlugType::Bool => "bool",
        PlugType::Int => "int",
        PlugType::Float => "float",
        PlugType::Vec3 => "vec3",
        PlugType::Matrix => "matrix",
        PlugType::String => "string",
        PlugType::Enum => "enum",
        PlugType::Mesh => "mesh",
        PlugType::Data => "data",
    }
}

fn describe_attr(a: &AttrDef) -> Value {
    let on = |f: u32| a.flags & f != 0;
    json!({
        "long": a.name,
        "short": a.short_name,
        "type": type_name(a.ty),
        "array": a.is_array(),
        "input": on(flags::INPUT),
        "output": on(flags::OUTPUT),
        "keyable": on(flags::KEYABLE),
        "storable": on(flags::STORABLE),
        "hidden": on(flags::HIDDEN),
        "channelBox": on(flags::CHANNEL_BOX),
        "user": on(flags::USER),
        "color": on(flags::COLOR),
        "children": if a.ty == PlugType::Vec3 { a.child_names.to_vec() } else { Vec::new() },
        "childShort": if a.ty == PlugType::Vec3 { a.child_short.to_vec() } else { Vec::new() },
        "enum": a.enum_names,
        "min": a.min_value,
        "max": a.max_value,
        "softMin": a.soft_min,
        "softMax": a.soft_max,
        "nice": a.nice_name,
        "default": value_to_json(&a.default_value),
    })
}

fn id_of(a: &Value) -> Result<NodeId, String> {
    a.get("id").and_then(Value::as_u64).ok_or_else(|| "(kInvalidParameter): Object is incompatible with this method".to_string())
}

fn text_of<'a>(a: &'a Value, k: &str) -> Result<&'a str, String> {
    a.get(k).and_then(Value::as_str).ok_or_else(|| format!("(kInvalidParameter): {k} expected"))
}

fn vec_of(a: &Value, k: &str) -> DVec3 {
    let n = |i: usize| a.get(k).and_then(|v| v.get(i)).and_then(Value::as_f64).unwrap_or(0.0);
    DVec3::new(n(0), n(1), n(2))
}

fn matrix_of(g: &Graph, id: NodeId, attr: usize) -> Vec<f64> {
    g.node(id).map_or(DMat4::IDENTITY, |n| n.matrix(attr)).to_cols_array().to_vec()
}

fn out_mesh(g: &mut Graph, id: NodeId) -> Result<(NodeId, Option<MeshPtr>), String> {
    let shape = components::as_mesh(g, id).ok_or("(kInvalidParameter): Object is incompatible with this method")?;
    g.evaluate();
    let m = g.node(shape).and_then(|n| n.data().mesh(MeshNode::OUT_MESH).cloned());
    Ok((shape, m))
}

/// The faces' triangles as a fan: (face, triangle in face, three points).
fn triangles(m: &Mesh, mut f: impl FnMut(i64, i64, [DVec3; 3])) {
    let mut face = 0;
    for id in 0..m.face_capacity() {
        if !m.face_valid(id) {
            continue;
        }
        let pts = m.face_points(id);
        for t in 1..pts.len().saturating_sub(1) {
            f(face, t as i64 - 1, [m.position(pts[0]), m.position(pts[t]), m.position(pts[t + 1])]);
        }
        face += 1;
    }
}

/// Every ray hit: [t, face, triangle, barycentric 1, barycentric 2].
fn ray_hits(m: &Mesh, origin: DVec3, dir: DVec3, both: bool, max: f64) -> Vec<[f64; 5]> {
    let mut hits = Vec::new();
    triangles(m, |face, tri, [a, b, c]| {
        let (e1, e2) = (b - a, c - a);
        let p = dir.cross(e2);
        let det = e1.dot(p);
        if det.abs() < 1e-14 {
            return;
        }
        let s = origin - a;
        let u = s.dot(p) / det;
        if !(-1e-9..=1.0 + 1e-9).contains(&u) {
            return;
        }
        let q = s.cross(e1);
        let v = dir.dot(q) / det;
        if v < -1e-9 || u + v > 1.0 + 1e-9 {
            return;
        }
        let t = e2.dot(q) / det;
        if t.abs() > max || (!both && t < 0.0) {
            return;
        }
        hits.push([t, face as f64, tri as f64, u, v]);
    });
    hits.sort_by(|a, b| a[0].abs().total_cmp(&b[0].abs()));
    // Through the diagonal of a face, both its triangles answer: one hit.
    hits.dedup_by(|a, b| a[1] == b[1] && (a[0] - b[0]).abs() < 1e-9);
    hits
}

fn closest_on_triangle(p: DVec3, [a, b, c]: [DVec3; 3]) -> DVec3 {
    let (ab, ac, ap) = (b - a, c - a, p - a);
    let (d1, d2) = (ab.dot(ap), ac.dot(ap));
    if d1 <= 0.0 && d2 <= 0.0 {
        return a;
    }
    let bp = p - b;
    let (d3, d4) = (ab.dot(bp), ac.dot(bp));
    if d3 >= 0.0 && d4 <= d3 {
        return b;
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        return a + ab * (d1 / (d1 - d3));
    }
    let cp = p - c;
    let (d5, d6) = (ab.dot(cp), ac.dot(cp));
    if d6 >= 0.0 && d5 <= d6 {
        return c;
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        return a + ac * (d2 / (d2 - d6));
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && d4 - d3 >= 0.0 && d5 - d6 >= 0.0 {
        return b + (c - b) * ((d4 - d3) / ((d4 - d3) + (d5 - d6)));
    }
    let denom = 1.0 / (va + vb + vc);
    a + ab * (vb * denom) + ac * (vc * denom)
}

/// A JSON query from Python.
pub fn query(core: &mut Core, op: &str, a: &Value) -> Result<Value, String> {
    let g = &mut core.s.graph;
    Ok(match op {
        "node.find" => json!(g.find(text_of(a, "name")?).map_err(|e| e.to_string())?),
        "node.exists" => json!(g.node(id_of(a)?).is_some()),
        "node.all" => json!(g.nodes()),
        "node.roots" => json!(g.roots()),
        "node.info" => {
            let id = id_of(a)?;
            let Some(n) = g.node(id) else { return Ok(Value::Null) };
            let mut chain = Vec::new();
            let mut t = Some(n.ty());
            while let Some(ty) = t {
                chain.push(ty.name.clone());
                t = ty.base.as_deref();
            }
            let dag = n.ty().dag;
            json!({
                "id": id,
                "name": n.name(),
                "type": n.ty().name,
                "chain": chain,
                "dag": dag,
                "owner": n.ty().owner,
                "path": g.path(id),
                "full": if dag { g.full_path(id) } else { n.name().to_string() },
                "parent": g.parent(id),
                "children": g.children(id),
                "intermediate": dag && n.bool(DagNode::INTERMEDIATE_OBJECT),
            })
        }
        "node.attrs" => {
            let n = g.node(id_of(a)?).ok_or("(kInvalidParameter): Object does not exist")?;
            Value::Array(n.ty().attrs.iter().map(describe_attr).collect())
        }
        "node.connections" => {
            let id = id_of(a)?;
            // The graph lists them destination first; here it's source, destination.
            let pairs: Vec<(PlugRef, PlugRef)> = g.connections().into_iter().filter(|(d, s)| s.node == id || d.node == id).collect();
            Value::Array(pairs.iter().map(|(d, s)| json!([g.plug_name(s), g.plug_name(d)])).collect())
        }
        "plug.info" => {
            let p = g.plug(text_of(a, "name")?).map_err(|e| e.to_string())?;
            let attr = g.attr(&p);
            let mut d = describe_attr(attr);
            let o = d.as_object_mut().unwrap();
            o.insert("node".into(), json!(p.node));
            o.insert("component".into(), json!(p.component));
            o.insert("element".into(), json!(p.element));
            o.insert("name".into(), json!(g.plug_name(&p)));
            o.insert("locked".into(), json!(g.is_locked(&p)));
            d
        }
        "plug.setFlag" => {
            let p = g.plug(text_of(a, "name")?).map_err(|e| e.to_string())?;
            let bit = if text_of(a, "flag")? == "keyable" { flags::KEYABLE } else { flags::CHANNEL_BOX };
            let mut def = g.attr(&p).clone();
            if a.get("on").and_then(Value::as_bool).unwrap_or(false) {
                def.flags |= bit;
            } else {
                def.flags &= !bit;
            }
            g.edit_attr(&p, def).map_err(|e| e.to_string())?;
            Value::Null
        }
        "plug.connections" => {
            let p = g.plug(text_of(a, "name")?).map_err(|e| e.to_string())?;
            let source = g.source(&p).map(|s| g.plug_name(&s));
            let dests: Vec<String> = g.destinations(&p).iter().map(|d| g.plug_name(d)).collect();
            json!({"source": source, "dests": dests})
        }
        "plug.elements" => {
            let p = g.plug(text_of(a, "name")?).map_err(|e| e.to_string())?;
            json!(g.elements(&p).iter().map(|e| e.element).collect::<Vec<i32>>())
        }
        "dag.matrices" => {
            let id = id_of(a)?;
            let n = g.node(id).ok_or("(kInvalidParameter): Object does not exist")?;
            if !n.ty().dag {
                return Err("(kInvalidParameter): Object is incompatible with this method".into());
            }
            let is_xform = n.ty().is_a("transform");
            g.evaluate();
            json!({
                "world": matrix_of(g, id, DagNode::WORLD_MATRIX),
                "parent": matrix_of(g, id, DagNode::PARENT_MATRIX),
                "local": if is_xform { matrix_of(g, id, Transform::MATRIX) } else { DMat4::IDENTITY.to_cols_array().to_vec() },
            })
        }
        "dag.bounds" => {
            g.evaluate();
            let b = object_bounds(g, id_of(a)?);
            if b.valid() {
                json!([b.min.x, b.min.y, b.min.z, b.max.x, b.max.y, b.max.z])
            } else {
                Value::Null
            }
        }
        "mesh.hits" => {
            let (_, m) = out_mesh(g, id_of(a)?)?;
            let Some(m) = m else { return Ok(json!([])) };
            let max = a.get("max").and_then(Value::as_f64).unwrap_or(f64::MAX);
            let both = a.get("both").and_then(Value::as_bool).unwrap_or(false);
            json!(ray_hits(&m, vec_of(a, "origin"), vec_of(a, "dir"), both, max))
        }
        "mesh.closest" => {
            let (_, m) = out_mesh(g, id_of(a)?)?;
            let Some(m) = m else { return Ok(Value::Null) };
            let p = vec_of(a, "point");
            let mut best = (f64::MAX, DVec3::ZERO, -1);
            triangles(&m, |face, _, tri| {
                let q = closest_on_triangle(p, tri);
                let d = q.distance_squared(p);
                if d < best.0 {
                    best = (d, q, face);
                }
            });
            if best.2 < 0 {
                return Ok(Value::Null);
            }
            json!([best.1.x, best.1.y, best.1.z, best.2])
        }
        "curve.data" => {
            let id = id_of(a)?;
            let shape = curves::as_curve(g, id).ok_or("(kInvalidParameter): Object is incompatible with this method")?;
            let Some(c) = curves::local_curve(g, shape) else { return Err("(kFailure): no curve data".into()) };
            let (lo, hi) = c.domain();
            json!({
                "shape": shape,
                "degree": c.degree,
                "form": c.form.index(),
                "spans": c.spans(),
                "cvs": c.cvs.iter().flat_map(|p| [p.x, p.y, p.z]).collect::<Vec<f64>>(),
                "knots": c.knots,
                "domain": [lo, hi],
                "length": c.length(),
            })
        }
        "curve.eval" => {
            let id = id_of(a)?;
            let shape = curves::as_curve(g, id).ok_or("(kInvalidParameter): Object is incompatible with this method")?;
            let Some(c) = curves::local_curve(g, shape) else { return Err("(kFailure): no curve data".into()) };
            let u = a.get("param").and_then(Value::as_f64).unwrap_or(0.0);
            let v3 = |p: DVec3| json!([p.x, p.y, p.z]);
            match text_of(a, "what")? {
                "point" => v3(c.eval(u)),
                "tangent" => v3(c.tangent(u)),
                "closest" => {
                    let t = c.closest_param(vec_of(a, "point"));
                    json!({"param": t, "point": v3(c.eval(t))})
                }
                "paramAtLength" => {
                    let total = c.length();
                    let want = a.get("length").and_then(Value::as_f64).unwrap_or(0.0);
                    json!(c.param_at_fraction(if total > 0.0 { want / total } else { 0.0 }))
                }
                other => return Err(format!("(kInvalidParameter): {other}")),
            }
        }
        "anim.eval" => {
            use aztek_scene::anim::{keys_of, Infinity};
            let t = a.get("frame").and_then(Value::as_f64).unwrap_or(0.0);
            json!(keys_of(g, id_of(a)?).eval(t, Infinity::Constant, Infinity::Constant))
        }
        "skin.data" => {
            let id = id_of(a)?;
            let inf: Vec<Value> = skin::influences(g, id).iter().map(|(i, j)| json!([i, j])).collect();
            let weights: Vec<Value> = skin::bind_of(g, id).map(|b| b.weights.iter().map(|w| json!(w.iter().flat_map(|&(i, x)| [i as f64, x]).collect::<Vec<f64>>())).collect()).unwrap_or_default();
            json!({"influences": inf, "weights": weights, "geometry": skin::skin_geometry(g, id).unwrap_or(NO_NODE)})
        }
        other => return Err(format!("(kInvalidParameter): unknown query {other}")),
    })
}

/// A mesh as flat lists, compact indices, everything by face in order.
#[derive(Default)]
pub struct MeshArrays {
    pub shape: NodeId,
    pub points: Vec<f64>,
    pub counts: Vec<i64>,
    pub connects: Vec<i64>,
    /// Per corner: the edge from its point to the next.
    pub corner_edges: Vec<i64>,
    /// Two points per edge.
    pub edges: Vec<i64>,
    pub corner_normals: Vec<f64>,
    pub point_normals: Vec<f64>,
    pub has_uvs: bool,
    /// Per corner: its UV (map) id.
    pub corner_maps: Vec<i64>,
    /// u, v per map.
    pub map_uvs: Vec<f64>,
    /// The ids components use, when they differ from the compact indices (the mesh has holes); empty otherwise.
    pub point_ids: Vec<i64>,
    pub edge_ids: Vec<i64>,
    pub face_ids: Vec<i64>,
}

pub fn mesh_arrays(core: &mut Core, id: NodeId) -> Result<MeshArrays, String> {
    let (shape, m) = out_mesh(&mut core.s.graph, id)?;
    let mut o = MeshArrays { shape, ..Default::default() };
    let Some(m) = m else { return Ok(o) };
    let mut point_at = vec![INVALID; m.point_capacity() as usize];
    let mut next: Index = 0;
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            point_at[p as usize] = next;
            next += 1;
            let q = m.position(p);
            o.points.extend([q.x, q.y, q.z]);
        }
    }
    let mut edge_at = vec![INVALID; m.edge_capacity() as usize];
    let mut next: Index = 0;
    for e in 0..m.edge_capacity() {
        if m.edge_valid(e) {
            edge_at[e as usize] = next;
            next += 1;
            o.edges.extend([point_at[m.edge_point(e, 0) as usize] as i64, point_at[m.edge_point(e, 1) as usize] as i64]);
        }
    }
    let normals = corner_normals(&m, &CornerNormalOptions::default());
    let smooth = point_normals(&m, NormalWeight::Angle);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            let n = smooth[p as usize];
            o.point_normals.extend([n.x as f64, n.y as f64, n.z as f64]);
        }
    }
    let topo = aztek_scene::uv_ops::topo(&m);
    o.has_uvs = m.corner_attrs().has(aztek_scene::uv_ops::UV);
    let uv = m.corner_attrs().get::<[f32; 2]>(aztek_scene::uv_ops::UV);
    for corners in &topo.maps {
        let u = uv.get(corners[0] as usize).copied().unwrap_or([0.0, 0.0]);
        o.map_uvs.extend([u[0] as f64, u[1] as f64]);
    }
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let corners = m.face_corners(f);
        o.counts.push(corners.len() as i64);
        for c in corners {
            o.connects.push(point_at[m.corner_point(c) as usize] as i64);
            o.corner_edges.push(edge_at[m.corner_edge(c) as usize] as i64);
            o.corner_maps.push(topo.corner_map[c as usize] as i64);
            let n = normals[c as usize];
            o.corner_normals.extend([n.x as f64, n.y as f64, n.z as f64]);
        }
    }
    if m.has_deleted() {
        o.point_ids = (0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| p as i64).collect();
        o.edge_ids = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)).map(|e| e as i64).collect();
        o.face_ids = (0..m.face_capacity()).filter(|&f| m.face_valid(f)).map(|f| f as i64).collect();
    }
    Ok(o)
}
