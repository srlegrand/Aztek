//! The reference commands scripts lean on that have no Aztek counterpart of their own: polyEvaluate, listConnections,
//! objectType, pointPosition. The polygon commands are in mesh_edit/ref_poly.

use aztek_geo::Index;
use aztek_ops::{execute, fail, objects, ArgType, CallSource, CmdResult, CommandDef, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Value};

use crate::components::{self, ComponentMode};
use crate::object_sets::Member;
use crate::plug::{NodeId, PlugRef};
use crate::session::Session;

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

/// The arguments (or the selection, components included) as members.
fn members(s: &Session, a: &aztek_ops::Args) -> CmdResult<Vec<Member>> {
    let g = &s.graph;
    let mut names = a.named();
    if names.is_empty() {
        names = components::component_strings(g, false);
        if names.is_empty() {
            names = g.selection().iter().map(|&i| g.path(i)).collect();
        }
    }
    crate::object_sets::parse(g, &names).map_err(aztek_ops::CommandError::new)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "polyEvaluate",
            "Counts on meshes or components: -v vertices, -e edges, -f faces, -t triangles, -uv UVs, -s shells, -b bounding \
             box. With no flag, a dictionary of the counts.",
            |s, a| {
                let ms = members(s, a)?;
                let g = &mut s.graph;
                let mut tot = [0i64; 6];
                let mut lo = DVec3::splat(f64::MAX);
                let mut hi = DVec3::splat(f64::MIN);
                for m in &ms {
                    let Some(mesh) = components::as_mesh(g, m.node) else { continue };
                    let Some(cage) = crate::modifiers::cage(g, mesh) else { continue };
                    if m.kind != ComponentMode::Object {
                        let i = match m.kind {
                            ComponentMode::Vertex => 0,
                            ComponentMode::Edge => 1,
                            _ => 2,
                        };
                        tot[i] += m.ids.len() as i64;
                        continue;
                    }
                    tot[0] += (0..cage.point_capacity()).filter(|&p| cage.point_valid(p)).count() as i64;
                    tot[1] += (0..cage.edge_capacity()).filter(|&e| cage.edge_valid(e)).count() as i64;
                    let faces: Vec<Index> = (0..cage.face_capacity()).filter(|&f| cage.face_valid(f)).collect();
                    tot[2] += faces.len() as i64;
                    tot[3] += faces.iter().map(|&f| cage.face_size(f) as i64 - 2).sum::<i64>();
                    let t = crate::uv_ops::topo(&cage);
                    tot[4] += t.maps.len() as i64;
                    tot[5] += aztek_geo::multibox::shells_of(&cage).1 as i64;
                    let w = crate::rig::world(g, crate::dag::transform_of(g, mesh));
                    for p in 0..cage.point_capacity() {
                        if cage.point_valid(p) {
                            let q = w.transform_point3(cage.position(p));
                            lo = lo.min(q);
                            hi = hi.max(q);
                        }
                    }
                }
                let pick = [("vertex", 0), ("edge", 1), ("face", 2), ("triangle", 3), ("uvcoord", 4), ("shell", 5)];
                for (flag, i) in pick {
                    if a.bool(flag) {
                        return Ok(json!(tot[i]));
                    }
                }
                if a.bool("boundingBox") {
                    return Ok(json!([[lo.x, hi.x], [lo.y, hi.y], [lo.z, hi.z]]));
                }
                Ok(json!({"vertex": tot[0], "edge": tot[1], "face": tot[2], "triangle": tot[3], "uvcoord": tot[4], "shell": tot[5]}))
            },
        )
        .positional("meshes or components (the selection when none)")
        .flag(f("vertex", "v", ArgType::Bool, "Vertex count."))
        .flag(f("edge", "e", ArgType::Bool, "Edge count."))
        .flag(f("face", "f", ArgType::Bool, "Face count."))
        .flag(f("triangle", "t", ArgType::Bool, "Triangle count."))
        .flag(f("uvcoord", "uv", ArgType::Bool, "UV count."))
        .flag(f("shell", "s", ArgType::Bool, "Shell count (UV shells)."))
        .flag(f("boundingBox", "b", ArgType::Bool, "World bounding box ((xmin, xmax), (ymin, ymax), (zmin, zmax))."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "listConnections",
            "Nodes (or plugs with -p) connected to the objects or plugs: -s sources, -d destinations (both by default), \
             -type filters by node type, -c pairs each with the plug on this side.",
            |s, a| {
                let g = &s.graph;
                let src = a.get("source").and_then(Value::as_bool).unwrap_or(true);
                let dst = a.get("destination").and_then(Value::as_bool).unwrap_or(true);
                let plugs = a.bool("plugs");
                let pairs = a.bool("connections");
                let ty = a.string("type", "");
                let mut out: Vec<Value> = Vec::new();
                for name in objects(s, a) {
                    let (node, attr): (NodeId, Option<PlugRef>) = if name.contains('.') {
                        let p = g.plug(&name)?;
                        (p.node, Some(p))
                    } else {
                        match g.find(&name)? {
                            Some(n) => (n, None),
                            None => fail!("listConnections: no object named {name}"),
                        }
                    };
                    for (d, sp) in g.connections() {
                        let mut hits = Vec::new();
                        if dst && sp.node == node && attr.as_ref().is_none_or(|p| p.attr == sp.attr) {
                            hits.push((sp, d));
                        }
                        if src && d.node == node && attr.as_ref().is_none_or(|p| p.attr == d.attr) {
                            hits.push((d, sp));
                        }
                        for (mine, other) in hits {
                            let Some(on) = g.node(other.node) else { continue };
                            if !ty.is_empty() && !on.ty().is_a(&ty) {
                                continue;
                            }
                            if pairs {
                                out.push(json!(g.plug_name(&mine)));
                            }
                            let v = if plugs { json!(g.plug_name(&other)) } else { json!(g.path(other.node)) };
                            if pairs || !out.contains(&v) {
                                out.push(v);
                            }
                        }
                    }
                }
                Ok(Value::Array(out))
            },
        )
        .positional("objects or plugs")
        .flag(f("source", "s", ArgType::Bool, "Include upstream connections (default on)."))
        .flag(f("destination", "d", ArgType::Bool, "Include downstream connections (default on)."))
        .flag(f("plugs", "p", ArgType::Bool, "Return plugs rather than nodes."))
        .flag(f("connections", "c", ArgType::Bool, "Pair each with the plug on this side."))
        .flag(f("type", "t", ArgType::String, "Only nodes of this type."))
        .flag(f("skipConversionNodes", "scn", ArgType::Bool, "Accepted for existing scripts."))
        .undoable(false),
    );

    c.register(
        Cmd::new("objectType", "The node type of an object (objectType -isType checks one).", |s, a| {
            let Some(name) = a.pos_str(0) else { fail!("objectType: name an object") };
            let g = &s.graph;
            let id = match g.find(name)? {
                Some(i) => i,
                None => fail!("objectType: no object named {name}"),
            };
            let t = g.node(id).unwrap().ty().name.clone();
            if let Some(is) = a.get("isType").and_then(Value::as_str) {
                return Ok(json!(t == is));
            }
            Ok(json!(t))
        })
        .positional("object")
        .flag(f("isType", "i", ArgType::String, "Whether it is of this type."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "pointPosition",
            "The world position of a point: a vertex, a joint or transform (its pivot), or a locator. -l for local space.",
            |s, a| {
                let Some(n) = a.named().into_iter().next() else { fail!("pointPosition: name a point") };
                let g = &mut s.graph;
                g.evaluate();
                let local = a.bool("local");
                if components::is_component_string(&n) {
                    if let Some(q) = crate::nurbs_components::point_of(g, &n, local) {
                        return Ok(json!([q.x, q.y, q.z]));
                    }
                    if let Some((node, ids, at)) = provided_points(g, std::slice::from_ref(&n)).into_iter().next() {
                        let Some(q) = ids.first().and_then(|id| at.get(id)) else { fail!("pointPosition: no point {n}") };
                        let w = crate::rig::world(g, crate::dag::transform_of(g, node));
                        let q = if local { *q } else { w.transform_point3(*q) };
                        return Ok(json!([q.x, q.y, q.z]));
                    }
                    let targets = crate::skin::points_named(g, &[n.clone()]);
                    let Some((mesh, ids)) = targets.into_iter().next() else { fail!("pointPosition: no point {n}") };
                    let Some(&p) = ids.first() else { fail!("pointPosition: no point {n}") };
                    let Some(m) = components::mesh_of(g, mesh) else { fail!("pointPosition: no point {n}") };
                    if p >= m.point_capacity() || !m.point_valid(p) {
                        fail!("pointPosition: no point {n}");
                    }
                    let w = crate::rig::world(g, crate::dag::transform_of(g, mesh));
                    let q = if local { m.position(p) } else { w.transform_point3(m.position(p)) };
                    return Ok(json!([q.x, q.y, q.z]));
                }
                let id = crate::rig::find(g, &n, "pointPosition")?;
                let q = crate::rig::world(g, crate::dag::transform_of(g, id)).w_axis.truncate();
                Ok(json!([q.x, q.y, q.z]))
            },
        )
        .undoable(false)
        .positional("a point")
        .flag(f("world", "w", ArgType::Bool, "World space (the default)."))
        .flag(f("local", "l", ArgType::Bool, "Object space.")),
    );
}

/// xform on a curve's CVs: None when what's named isn't on a curve.
fn xform_cvs(g: &mut crate::graph::Graph, a: &aztek_ops::Args, named: &[String], ws: bool) -> CmdResult<Option<Value>> {
    use glam::DVec3;
    let mut out = Vec::new();
    let mut any = false;
    for n in named {
        let Some((obj, comp)) = n.split_once('.') else { continue };
        let Ok(Some(id)) = g.find(obj) else { continue };
        if let Some(values) = crate::nurbs_components::xform(g, a, id, comp, ws) {
            any = true;
            out.extend(values?);
            continue;
        }
        if let Some(shape) = crate::surfaces::as_surface(g, id) {
            any = true;
            let inner = comp.split_once('[').map(|x| x.1.trim_end_matches(']')).unwrap_or("");
            let Some(ids) = crate::surfaces::parse_cv_ranges(g, shape, inner) else { fail!("xform: bad component range in {n}") };
            let w = crate::rig::world(g, crate::dag::transform_of(g, shape));
            let Some(pp) = g.node(shape).and_then(crate::components::point_provider) else { continue };
            let Some(points) = pp.points else { continue };
            let current: std::collections::HashMap<u32, DVec3> = points(g, shape).into_iter().collect();
            if a.bool("query") {
                for i in ids {
                    let Some(&p) = current.get(&i) else { continue };
                    let q = if ws { w.transform_point3(p) } else { p };
                    out.extend([json!(q.x), json!(q.y), json!(q.z)]);
                }
            } else {
                let Some(t) = a.vec3("translation").map(DVec3::from_array) else { fail!("xform: on components only -t is available") };
                let inv = w.inverse();
                let rel = a.bool("relative");
                let moves: Vec<(u32, DVec3)> = ids
                    .into_iter()
                    .filter_map(|i| {
                        let p = *current.get(&i)?;
                        Some((
                            i,
                            match (rel, ws) {
                                (true, true) => p + inv.transform_vector3(t),
                                (true, false) => p + t,
                                (false, true) => inv.transform_point3(t),
                                (false, false) => t,
                            },
                        ))
                    })
                    .collect();
                if let Some(set) = pp.set_points {
                    set(g, shape, &moves);
                }
            }
            continue;
        }
        let Some(shape) = crate::curves::as_curve(g, id) else { continue };
        any = true;
        let Some(c) = crate::curves::local_curve(g, shape) else { continue };
        let w = crate::rig::world(g, crate::dag::transform_of(g, shape));
        let inner = comp.split_once('[').map(|x| x.1.trim_end_matches(']')).unwrap_or("");
        let count = c.distinct_cvs().len() as u32;
        let ids: Vec<u32> = match inner {
            "*" => (0..count).collect(),
            r => match r.split_once(':') {
                Some((a, b)) => (a.parse().unwrap_or(0)..=b.parse().unwrap_or(0)).collect(),
                None => r.parse().ok().into_iter().collect(),
            },
        };
        if a.bool("query") {
            for i in ids {
                let Some(&p) = c.cvs.get(i as usize) else { continue };
                let q = if ws { w.transform_point3(p) } else { p };
                out.extend([json!(q.x), json!(q.y), json!(q.z)]);
            }
        } else {
            let Some(t) = a.vec3("translation").map(DVec3::from_array) else { fail!("xform: on components only -t is available") };
            let inv = w.inverse();
            let rel = a.bool("relative");
            let moves: Vec<(u32, DVec3)> = ids
                .into_iter()
                .filter_map(|i| {
                    let p = *c.cvs.get(i as usize)?;
                    Some((
                        i,
                        if rel {
                            if ws { p + inv.transform_vector3(t) } else { p + t }
                        } else if ws {
                            inv.transform_point3(t)
                        } else {
                            t
                        },
                    ))
                })
                .collect();
            if let Some(n) = g.node(shape) {
                if let Some(pp) = crate::components::point_provider(n) {
                    if let Some(set) = pp.set_points {
                        set(g, shape, &moves);
                    }
                }
            }
        }
    }
    Ok(any.then_some(Value::Array(out)))
}

/// Named points of nodes that aren't meshes (a lattice's pt[s][t][u]): the node, the ids in order, each point's position.
#[allow(clippy::type_complexity)]
fn provided_points(g: &mut crate::graph::Graph, named: &[String]) -> Vec<(NodeId, Vec<u32>, std::collections::HashMap<u32, glam::DVec3>)> {
    let Ok(members) = crate::object_sets::parse(g, named) else { return Vec::new() };
    let mut out = Vec::new();
    for m in members {
        let Some(points) = g.node(m.node).and_then(components::point_provider).and_then(|pp| pp.points) else { continue };
        let at = points(g, m.node).into_iter().collect();
        out.push((m.node, m.ids, at));
    }
    out
}

/// xform on points: pCube1.vtx[0:3]. A query returns x, y, z per point, world space with -ws; a set moves them all to the
/// position, or by it with -r.
pub(crate) fn xform_components(s: &mut Session, a: &aztek_ops::Args, named: &[String]) -> CmdResult {
    use glam::DVec3;
    let g = &mut s.graph;
    g.evaluate();
    let ws = a.bool("worldSpace");
    if let Some(r) = xform_cvs(g, a, named, ws)? {
        return Ok(r);
    }
    let targets = crate::skin::points_named(g, named);
    let provided = provided_points(g, named);
    if a.bool("query") {
        let mut out = Vec::new();
        for (node, ids, at) in &provided {
            let w = crate::rig::world(g, crate::dag::transform_of(g, *node));
            for q in ids.iter().filter_map(|id| at.get(id)) {
                let q = if ws { w.transform_point3(*q) } else { *q };
                out.extend([json!(q.x), json!(q.y), json!(q.z)]);
            }
        }
        for (mesh, ids) in &targets {
            let Some(m) = crate::components::mesh_of(g, *mesh) else { continue };
            let w = crate::rig::world(g, crate::dag::transform_of(g, *mesh));
            for &p in ids {
                if !m.point_valid(p) {
                    continue;
                }
                let q = if ws { w.transform_point3(m.position(p)) } else { m.position(p) };
                out.extend([json!(q.x), json!(q.y), json!(q.z)]);
            }
        }
        return Ok(Value::Array(out));
    }
    let Some(t) = a.vec3("translation").map(DVec3::from_array) else { fail!("xform: on components only -t is available") };
    let rel = a.bool("relative");
    for (node, ids, at) in provided {
        let Some(set_points) = g.node(node).and_then(components::point_provider).and_then(|pp| pp.set_points) else { continue };
        let inv = crate::rig::world(g, crate::dag::transform_of(g, node)).inverse();
        let moves: Vec<(u32, DVec3)> = ids
            .into_iter()
            .filter_map(|id| at.get(&id).map(|q| (id, *q)))
            .map(|(id, q)| {
                let to = match (rel, ws) {
                    (true, true) => q + inv.transform_vector3(t),
                    (true, false) => q + t,
                    (false, true) => inv.transform_point3(t),
                    (false, false) => t,
                };
                (id, to)
            })
            .collect();
        set_points(g, node, &moves);
    }
    for (mesh, ids) in targets {
        let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
        let w = crate::rig::world(g, crate::dag::transform_of(g, mesh));
        let inv = w.inverse();
        let moves: Vec<(u32, DVec3)> = ids
            .into_iter()
            .filter(|&p| m.point_valid(p))
            .map(|p| {
                let at = if rel {
                    if ws { m.position(p) + inv.transform_vector3(t) } else { m.position(p) + t }
                } else if ws {
                    inv.transform_point3(t)
                } else {
                    t
                };
                (p, at)
            })
            .collect();
        crate::mesh_edit::transform::set_points(g, mesh, &moves)?;
    }
    Ok(Value::Null)
}

/// move, rotate and scale given components (pCube1.vtx[0:3]): the component command on them, leaving the selection as it
/// was. None when no components are named.
pub(crate) fn on_components(s: &mut Session, a: &aztek_ops::Args, target: &str) -> CmdResult<Option<Value>> {
    let named = a.named();
    let comps: Vec<String> = named.iter().filter(|n| components::is_component_string(n)).cloned().collect();
    if comps.is_empty() {
        return Ok(None);
    }
    let mode = components::mode(&s.graph);
    let objects: Vec<String> = s.graph.selection().iter().map(|&i| s.graph.path(i)).collect();
    let before = components::component_strings(&s.graph, false);
    let by = a.get("by").or(a.get("to")).cloned().unwrap_or(json!([0.0, 0.0, 0.0]));
    let scene = &mut *s;
    execute(scene, "select", &json!({"args": comps}), CallSource::Script)?;
    let r = execute(scene, target, &json!({"by": by}), CallSource::Script);
    execute(scene, "select", &json!({"args": if before.is_empty() { objects } else { before }}), CallSource::Script)?;
    components::set_mode(&mut scene.graph, mode);
    r?;
    Ok(Some(Value::Null))
}
