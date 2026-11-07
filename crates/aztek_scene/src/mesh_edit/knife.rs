//! PolyKnife.

use std::collections::BTreeMap;

use aztek_geo::ops::{dissolve_edges, knife_apply, knife_loop_plan, knife_plan, KnifeKind, KnifeOptions, KnifePlan, KnifePoint};
use aztek_geo::symmetry::{clear_symmetry_keys, SymmetryMap};
use aztek_geo::{Index, Mesh, MeshRemap, INVALID};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DVec3};
use serde_json::{json, Value};

use super::*;
use crate::components::{self, ComponentMode as K};
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef};
use crate::session::Session;
use crate::transform::Transform;

type T = ArgType;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

/// A single knife point on an edge: splits that edge (and its symmetric images) with a new vertex there.
fn insert_edge_point(
    g: &mut Graph,
    n: NodeId,
    base: PlugRef,
    mut mesh: Mesh,
    k: &KnifePoint,
    sym: Option<&SymmetryMap>,
    select: bool,
) -> CmdResult {
    if !mesh.edge_valid(k.id) {
        fail!("polyKnife: {} has no edge {}", components::display_path(g, n), k.id);
    }
    let (a, b) = (mesh.edge_point(k.id, 0), mesh.edge_point(k.id, 1));
    // Each image edge is split at the image of the spot, so t flips when the image runs the other way.
    let mut splits = vec![(k.id, k.t)];
    if let Some(sym) = sym {
        for e in 1..sym.order() {
            let (ia, ib) = (sym.image(e, a), sym.image(e, b));
            if ia == INVALID || ib == INVALID {
                continue;
            }
            let ie = mesh.find_edge(ia, ib);
            if ie == INVALID || splits.iter().any(|s| s.0 == ie) {
                continue;
            }
            splits.push((ie, if mesh.edge_point(ie, 0) == ia { k.t } else { 1.0 - k.t }));
        }
    }

    g.open_chunk("Knife");
    let r = (|| -> CmdResult {
        let mut before = Vec::new();
        let mut drive = 0;
        if let Some(sym) = sym {
            before = mesh.positions().to_vec();
            drive = sym.majority_wedge(&[a, b]);
        }
        let first_new = mesh.point_capacity();
        let (mut points, mut edges) = (Vec::new(), Vec::new());
        for (e, t) in splits {
            points.push(mesh.split_edge(e, t));
            edges.push(mesh.last_split_edge());
        }
        clear_symmetry_keys(&mut mesh, first_new);
        if sym.is_some() {
            maintain_symmetry(g, n, &mut mesh, &before, drive);
        }
        let mut remap = MeshRemap::default();
        mesh.compact(Some(&mut remap));
        let mapped_points = remap_ids(&points, &remap.points);
        let mapped_edges = remap_ids(&edges, &remap.edges);
        store_edited(g, n, base, mesh, &remap)?;
        if select {
            components::set_all(g, BTreeMap::from([(n, mapped_points)]), BTreeMap::new(), BTreeMap::new());
            components::set_mode(g, K::Vertex);
        }
        Ok(json!(mapped_edges))
    })();
    g.close_chunk();
    r
}

const POINT_FORMS: &str = "polyKnife: each point is {vertex:}, {edge:, t:} or {face:, position:}";

/// A plain cut on a mesh with history: a polySplit node holding the path in the mesh's own space.
fn knife_recorded(g: &mut Graph, n: NodeId, a: &Args) -> CmdResult {
    let world = g.node(n).unwrap().matrix(Transform::WORLD_MATRIX);
    let to_local = world.inverse();
    let mut flags = serde_json::Map::new();
    let mut pts = Vec::new();
    for p in a.get("points").and_then(Value::as_array).into_iter().flatten() {
        let Some(o) = p.as_object() else { fail!("{POINT_FORMS}") };
        let mut q = o.clone();
        if let Some(at) = o.get("position") {
            let l = to_local.transform_point3(vec(at));
            q.insert("position".into(), json!([l.x, l.y, l.z]));
        }
        pts.push(Value::Object(q));
    }
    flags.insert("points".into(), Value::Array(pts));
    flags.insert("closed".into(), json!(a.bool("closed")));
    flags.insert("through".into(), json!(a.bool("through")));
    flags.insert("loopEdge".into(), json!(a.int("loopEdge", -1)));
    flags.insert("fraction".into(), json!(a.float("fraction", 0.5)));
    if a.has("eye") {
        let e = to_local.transform_point3(DVec3::from_array(a.vec3("eye").unwrap()));
        flags.insert("eye".into(), json!([e.x, e.y, e.z]));
        flags.insert("useEye".into(), json!(true));
    } else if a.has("direction") {
        let d = DMat3::from_mat4(world).inverse() * DVec3::from_array(a.vec3("direction").unwrap());
        flags.insert("direction".into(), json!([d.x, d.y, d.z]));
        flags.insert("useDirection".into(), json!(true));
    }
    let args = Args { positional: Vec::new(), flags };
    let Some(op) = crate::history::op("polySplit") else { fail!("polyKnife: no polySplit op") };
    let mut mesh = modifiers::cage(g, n).map(|m| (*m).clone()).unwrap_or_default();
    let cut = (op.run)(&args, &mut mesh, &Vec::new(), None)?.ids;
    if cut.is_empty() {
        fail!("polyKnife: the path crosses no faces it can cut");
    }
    let mut remap = MeshRemap::default();
    mesh.compact(Some(&mut remap));
    let mapped = remap_ids(&cut, &remap.edges);
    g.open_chunk("Knife");
    let r = (|| -> CmdResult {
        let mut sel = current_selection(g);
        remap_selection(&mut sel, n, &remap);
        crate::history::insert(g, n, &op, &args, &[], None)?;
        modifiers::base_remapped(g, n, &remap, &Arc::new(mesh));
        let [p, e, f] = sel;
        components::set_all(g, p, e, f);
        if a.bool("select") {
            components::set_all(g, BTreeMap::new(), BTreeMap::from([(n, mapped.clone())]), BTreeMap::new());
            if components::mode(g) == K::Object {
                components::set_mode(g, K::Edge);
            }
        }
        Ok(json!(mapped))
    })();
    g.close_chunk();
    r
}

fn knife(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let n = mesh_flag(g, a, "polyKnife")?;
    // One point on an edge or a deleted edge keep their own handling; everything else is a cut along a path.
    let one_point = a.get("points").and_then(Value::as_array).is_some_and(|l| l.len() == 1);
    if a.int("deleteEdge", -1) < 0 && !one_point && crate::history::can_record(g, n) && crate::mesh_nodes::symmetry_spec(g, n).is_none() {
        return knife_recorded(g, n, a);
    }
    let name = components::display_path(g, n);
    let base = base_plug(g, n)?;
    let mut mesh = base_mesh(g, &base);
    let world = g.node(n).unwrap().matrix(Transform::WORLD_MATRIX);
    let to_local = world.inverse();
    let sym = symmetry_for(g, n, &mut mesh);

    let delete_edge = a.int("deleteEdge", -1);
    if delete_edge >= 0 {
        let de = delete_edge as Index;
        if !mesh.edge_valid(de) {
            fail!("polyKnife: {name} has no edge {delete_edge}");
        }
        g.open_chunk("Delete Edge");
        let r = (|| -> CmdResult {
            let mut before = Vec::new();
            let mut drive = 0;
            if let Some(sym) = &sym {
                before = mesh.positions().to_vec();
                drive = sym.majority_wedge(&components::convert(&mesh, K::Edge, &[de], K::Vertex, false));
            }
            let all = with_mirror(&mesh, sym.as_ref(), &[de], K::Edge);
            dissolve_edges(&mut mesh, &all, true);
            if sym.is_some() {
                maintain_symmetry(g, n, &mut mesh, &before, drive);
            }
            let mut remap = MeshRemap::default();
            mesh.compact(Some(&mut remap));
            let mut sel = current_selection(g);
            remap_selection(&mut sel, n, &remap);
            store_edited(g, n, base, mesh, &remap)?;
            let [p, e, f] = sel;
            components::set_all(g, p, e, f);
            Ok(json!([]))
        })();
        g.close_chunk();
        return r;
    }

    let loop_edge = a.int("loopEdge", -1);
    let plan: KnifePlan = if loop_edge >= 0 {
        if !mesh.edge_valid(loop_edge as Index) {
            fail!("polyKnife: {name} has no edge {loop_edge}");
        }
        knife_loop_plan(&mesh, loop_edge as Index, a.float("fraction", 0.5), sym.as_ref())
    } else {
        let mut pts = Vec::new();
        let list = a.get("points").and_then(Value::as_array).cloned().unwrap_or_default();
        for p in &list {
            let Some(o) = p.as_object() else { fail!("{POINT_FORMS}") };
            let id = |k: &str| o.get(k).and_then(Value::as_u64).map(|v| v as Index);
            if let Some(v) = id("vertex") {
                pts.push(KnifePoint::point(v));
            } else if let Some(e) = id("edge") {
                pts.push(KnifePoint::edge(e, o.get("t").and_then(Value::as_f64).unwrap_or(0.5).clamp(0.0, 1.0)));
            } else if let (Some(f), Some(pos)) = (id("face"), o.get("position")) {
                pts.push(KnifePoint::face(f, to_local.transform_point3(vec(pos))));
            } else {
                fail!("{POINT_FORMS}");
            }
        }
        if pts.len() == 1 && pts[0].kind == KnifeKind::Edge {
            return insert_edge_point(g, n, base, mesh, &pts[0], sym.as_ref(), a.bool("select"));
        }
        if pts.len() < 2 {
            fail!("polyKnife: give at least two points, or one on an edge");
        }
        let mut o = KnifeOptions { eye: None, view_dir: DVec3::ZERO, closed: a.bool("closed"), through: a.bool("through"), symmetry: sym.as_ref() };
        if a.has("eye") {
            o.eye = Some(to_local.transform_point3(DVec3::from_array(a.vec3("eye").unwrap())));
        } else if a.has("direction") {
            o.view_dir = DMat3::from_mat4(world).inverse() * DVec3::from_array(a.vec3("direction").unwrap());
        }
        knife_plan(&mesh, &pts, &o)
    };

    g.open_chunk("Knife");
    let r = (|| -> CmdResult {
        let mut before = Vec::new();
        let mut drive = 0;
        if let Some(sym) = &sym {
            before = mesh.positions().to_vec();
            let touched: Vec<Index> = plan.segments.iter().filter(|s| !s.mirrored).flat_map(|s| mesh.face_points(s.face)).collect();
            drive = sym.majority_wedge(&touched);
        }
        let first_new = mesh.point_capacity();
        let cut = knife_apply(&mut mesh, &plan);
        if cut.is_empty() {
            fail!("polyKnife: the path crosses no faces it can cut");
        }
        clear_symmetry_keys(&mut mesh, first_new);
        if sym.is_some() {
            maintain_symmetry(g, n, &mut mesh, &before, drive);
        }
        let mut remap = MeshRemap::default();
        mesh.compact(Some(&mut remap));
        let mapped = remap_ids(&cut, &remap.edges);
        store_edited(g, n, base, mesh, &remap)?;
        if a.bool("select") {
            components::set_all(g, BTreeMap::new(), BTreeMap::from([(n, mapped.clone())]), BTreeMap::new());
            if components::mode(g) == K::Object {
                components::set_mode(g, K::Edge);
            }
        }
        Ok(json!(mapped))
    })();
    g.close_chunk();
    r
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::new(
            "polyKnife",
            "Knife / multi-cut: cut a mesh along a path of points, splitting the faces between them and the edges the path \
             crosses. Between two points the cut follows the surface under the straight screen line joining them (seen \
             from eye= or along direction=; without either, along the points' average normal). With loopEdge= it inserts \
             an edge loop across that edge's quad ring instead, and with deleteEdge= it dissolves that edge. Follows the \
             mesh's symmetry. A single point on an edge inserts a vertex there. Returns the new edges.",
            knife,
        )
        .flag(flag("mesh", T::String, Value::Null, "Mesh to cut; default: the selected or hilited mesh."))
        .flag(flag(
            "points",
            T::List,
            Value::Null,
            "Path points: {\"vertex\": id}, {\"edge\": id, \"t\": 0..1 from its first point}, or {\"face\": id, \"position\": \
             [x, y, z] world}.",
        ))
        .flag(flag("closed", T::Bool, json!(false), "Also cut from the last point back to the first."))
        .flag(flag("through", T::Bool, json!(false), "Cut every face under the line, hidden faces too."))
        .flag(flag("eye", T::Vec3, Value::Null, "World eye of a perspective view."))
        .flag(flag("direction", T::Vec3, Value::Null, "World view direction of an orthographic view."))
        .flag(flag("loopEdge", T::Int, json!(-1), "Insert an edge loop across this edge's ring instead of cutting a path.").min(-1.0))
        .flag(flag("fraction", T::Float, json!(0.5), "Where along loopEdge the loop crosses it, from its first point.").min(0.0).max(1.0))
        .flag(flag("deleteEdge", T::Int, json!(-1), "Dissolve this edge, joining its two faces, instead of cutting.").min(-1.0))
        .flag(flag("select", T::Bool, json!(true), "Select the new edges.")),
    );
}
