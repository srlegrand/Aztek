//! The commands general to every deformer: deformer, percent, reorderDeformers.

use aztek_ops::{execute, fail, ArgType, Args, CallSource, CmdResult, Commands};
use serde_json::{json, Map, Value};

use crate::components;
use crate::deform::{self, f, floats_of, input_mesh, is_deformer, plug, set_floats, Cmd, IN, OUT, WEIGHTS};
use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef};
use crate::session::Session;
use crate::skin::{asked, modes};

/// The deformer a name stands for.
pub(crate) fn deformer_named(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = crate::rig::find(g, name, cmd)?;
    if g.node(id).is_some_and(|n| is_deformer(n.ty().name.as_str())) {
        return Ok(id);
    }
    fail!("{cmd}: {name} is not a deformer")
}

/// The command that makes a deformer type.
fn maker(ty: &str) -> Option<(&'static str, Option<&str>)> {
    Some(match ty {
        "ffd" => ("lattice", None),
        "cluster" => ("cluster", None),
        "softMod" => ("softMod", None),
        "deltaMush" => ("deltaMush", None),
        "tension" => ("tension", None),
        "wrap" => ("wrap", None),
        "wire" => ("wire", None),
        "blendShape" => ("blendShape", None),
        "sculpt" => ("sculpt", None),
        "jiggle" => ("jiggle", None),
        "shrinkWrap" => ("shrinkWrap", None),
        "morph" => ("morph", None),
        "wrinkle" => ("wrinkle", None),
        "proximityWrap" => ("proximityWrap", None),
        "solidify" => ("solidify", None),
        "pointOnCurve" => ("pointOnCurveDeformer", None),
        "textureDeformer" => ("textureDeformer", None),
        "curveWarp" => ("curveWarp", None),
        t if deform::NONLINEAR.contains(&t) => ("nonLinear", Some(ty)),
        _ => return None,
    })
}

/// A plug-in's deformer on the objects named or selected: one node per mesh, the first the leader, as for any deformer.
fn create_plugin(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    let mut targets = deform::targets(s, a)?;
    let names: Vec<String> = named_geometry(a);
    if !names.is_empty() {
        targets.extend(deform::named_targets(&mut s.graph, &names)?);
    }
    if targets.is_empty() {
        fail!("deformer: nothing to deform");
    }
    // Named as the reference names a new node: the type and a number.
    let name = a.string("name", &format!("{ty}1"));
    let g = &mut s.graph;
    let (mesh, pts) = targets.remove(0);
    let n = deform::insert_at(g, mesh, ty, &name, deform::Place::from_args(a))?;
    if let Some(input) = input_mesh(g, n) {
        deform::limit_to(g, n, input.point_capacity(), &pts)?;
    }
    for (m, p) in targets {
        deform::add_geometry(g, n, m, &p)?;
    }
    Ok(json!([g.path(n)]))
}

fn create(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    if crate::scripted::is_plugin_deformer(ty) {
        return create_plugin(s, a, ty);
    }
    let Some((cmd, kind)) = maker(ty) else {
        fail!("deformer: can't make a {ty} (ffd, cluster, softMod, deltaMush, tension, wrap, wire, blendShape, sculpt, jiggle, shrinkWrap, morph, wrinkle, proximityWrap, solidify, pointOnCurve, textureDeformer, curveWarp or a nonlinear type)")
    };
    let mut call = Map::new();
    let mut args: Vec<Value> = a.named().into_iter().map(Value::String).collect();
    if let Some(g) = a.get("geometry") {
        match g {
            Value::Array(v) => args.extend(v.iter().cloned()),
            v if v.is_string() => args.push(v.clone()),
            _ => {}
        }
    }
    if !args.is_empty() {
        call.insert("args".into(), Value::Array(args));
    }
    for k in ["name", "before", "after", "frontOfChain"] {
        if let Some(v) = a.get(k) {
            call.insert(k.into(), v.clone());
        }
    }
    if let Some(t) = kind {
        call.insert("type".into(), json!(t));
    }
    let made = execute(s, cmd, &Value::Object(call), CallSource::Script)?;
    let first = made.as_array().and_then(|v| v.first()).cloned().unwrap_or(made);
    Ok(json!([first]))
}

fn named_geometry(a: &Args) -> Vec<String> {
    match a.get("geometry") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn deformer_cmd(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let Some(name) = a.named().into_iter().next() else { fail!("deformer: name a deformer to query") };
        let g = &s.graph;
        let n = deformer_named(g, &name, "deformer")?;
        if asked(a, "geometryIndices") {
            return Ok(json!((0..deform::geometry_names(g, n).len()).collect::<Vec<_>>()));
        }
        if asked(a, "geometry") {
            return Ok(json!(deform::geometry_names(g, n)));
        }
        fail!("deformer: query -g or -gi");
    }
    if a.bool("edit") {
        let Some(name) = a.named().into_iter().next() else { fail!("deformer: name a deformer to edit") };
        let geometry = named_geometry(a);
        if geometry.is_empty() {
            fail!("deformer: -e needs -g");
        }
        let remove = a.bool("remove");
        let g = &mut s.graph;
        let n = deformer_named(g, &name, "deformer")?;
        let leader = deform::leader_of(g, n);
        for geo in geometry {
            let targets = if components::is_component_string(&geo) {
                crate::skin::points_named(g, std::slice::from_ref(&geo)).into_iter().map(|(m, ids)| (m, Some(ids))).collect()
            } else {
                let id = crate::rig::find(g, &geo, "deformer")?;
                let Some(m) = components::as_mesh(g, id) else { fail!("deformer: {geo} is not a mesh") };
                vec![(m, None)]
            };
            for (m, pts) in targets {
                let on = deform::node_on(g, leader, m);
                // Points named of a mesh already on the deformer: membership, by way of its set.
                if let (Some(_), Some(ids), Some(set)) = (on, &pts, crate::deform_sets::set_of(g, leader)) {
                    let list = [crate::object_sets::Member { node: m, kind: components::ComponentMode::Vertex, ids: ids.clone() }];
                    if remove {
                        crate::object_sets::remove(g, set, &list)?;
                    } else {
                        crate::object_sets::add(g, set, &list)?;
                    }
                    continue;
                }
                if remove {
                    let Some(x) = on else { fail!("deformer: {geo} is not deformed by {name}") };
                    deform::splice(g, x)?;
                    // A further mesh's node has no other use. The leader stays, holding the settings.
                    if x != leader {
                        g.delete_node(x)?;
                    }
                } else {
                    if on.is_some() {
                        fail!("deformer: {geo} is already deformed by {name}");
                    }
                    deform::add_geometry(g, leader, m, &pts)?;
                }
            }
        }
        return Ok(json!([g.path(n)]));
    }
    let ty = a.string("type", "");
    if ty.is_empty() {
        fail!("deformer: give -type");
    }
    create(s, a, &ty)
}

/// The points percent works on, per mesh: (node on that mesh, ids).
fn percent_targets(g: &mut Graph, n: NodeId, names: &[String]) -> CmdResult<Vec<(NodeId, Vec<u32>)>> {
    let mut named: Vec<String> = names.to_vec();
    if named.is_empty() {
        named = components::component_strings(g, false);
    }
    let mut out = Vec::new();
    if named.is_empty() {
        for x in deform::group(g, n) {
            let Some(input) = input_mesh(g, x) else { continue };
            out.push((x, (0..input.point_capacity()).filter(|&p| input.point_valid(p)).collect()));
        }
        return Ok(out);
    }
    for (mesh, ids) in crate::skin::points_named(g, &named) {
        let Some(x) = deform::node_on(g, n, mesh) else { fail!("percent: {} is not deformed by {}", g.path(mesh), g.path(n)) };
        out.push((x, ids));
    }
    Ok(out)
}

fn percent(s: &mut Session, a: &Args) -> CmdResult {
    let mut names = a.named();
    if names.is_empty() {
        fail!("percent: name a deformer");
    }
    let name = names.remove(0);
    let g = &mut s.graph;
    let n = deformer_named(g, &name, "percent")?;
    if g.node(n).is_some_and(|x| x.ty().is_a("blendShape")) {
        fail!("percent: a blendShape has a weight per target instead");
    }
    let targets = percent_targets(g, n, &names)?;
    let mut read = Vec::new();
    for (x, ids) in targets {
        let Some(input) = input_mesh(g, x) else { continue };
        let size = input.point_capacity() as usize;
        let mut w = floats_of(g.node(x).unwrap().data(), WEIGHTS).map(|f| f.0.clone()).unwrap_or_else(|| vec![1.0; size]);
        w.resize(size, 0.0);
        if a.bool("query") {
            read.extend(ids.iter().filter_map(|&p| w.get(p as usize)).copied());
            continue;
        }
        let value = a.float("value", 1.0);
        let members = crate::deform_sets::member_mask(g, x, size);
        for p in ids {
            // A point out of the deformer's set has no weight to set.
            if members.as_ref().is_some_and(|m| !m.get(p as usize).copied().unwrap_or(false)) {
                continue;
            }
            if let Some(v) = w.get_mut(p as usize) {
                *v = if a.bool("multiply") { *v * value } else { value };
            }
        }
        set_floats(g, x, WEIGHTS, w)?;
    }
    if a.bool("query") {
        return Ok(json!(read));
    }
    Ok(Value::Null)
}

/// Swaps two nodes of a mesh's chain, leaving those between them in place.
fn swap_in_chain(g: &mut Graph, mesh: NodeId, x: NodeId, y: NodeId) -> CmdResult<()> {
    let chain = crate::history::history_of(g, mesh);
    let (Some(ix), Some(iy)) = (chain.iter().position(|&c| c == x), chain.iter().position(|&c| c == y)) else {
        fail!("reorderDeformers: both deformers must be on {}", g.path(mesh));
    };
    let (near, far) = (ix.min(iy), ix.max(iy));
    let mut segment = chain[near..=far].to_vec();
    let Some(up) = g.source(&plug(*segment.last().unwrap(), IN)) else { fail!("reorderDeformers: {} has no input", g.path(chain[far])) };
    let down: Vec<PlugRef> = g.destinations(&plug(segment[0], OUT));
    for &n in &segment {
        g.disconnect(&plug(n, IN));
    }
    for d in &down {
        g.disconnect(d);
    }
    let last = segment.len() - 1;
    segment.swap(0, last);
    g.connect(&up, &plug(segment[last], IN), false)?;
    for k in (1..=last).rev() {
        g.connect(&plug(segment[k], OUT), &plug(segment[k - 1], IN), false)?;
    }
    for d in &down {
        g.connect(&plug(segment[0], OUT), d, false)?;
    }
    Ok(())
}

fn reorder(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    if names.len() < 2 {
        fail!("reorderDeformers: name two deformers, then optionally the geometry");
    }
    let g = &mut s.graph;
    let chain_node = |g: &Graph, name: &str| -> CmdResult<NodeId> {
        let id = crate::rig::find(g, name, "reorderDeformers")?;
        if g.node(id).is_some_and(|n| crate::rig::is_rig_chain_node(n.ty().name.as_str())) {
            Ok(id)
        } else {
            fail!("reorderDeformers: {name} is not a deformer")
        }
    };
    let (x, y) = (chain_node(g, &names[0])?, chain_node(g, &names[1])?);
    let meshes: Vec<NodeId> = match names.get(2) {
        Some(geo) => {
            let id = crate::rig::find(g, geo, "reorderDeformers")?;
            let Some(m) = components::as_mesh(g, id) else { fail!("reorderDeformers: {geo} is not a mesh") };
            vec![m]
        }
        None => deform::group(g, x).into_iter().filter_map(|n| deform::mesh_fed_by(g, n)).filter(|&m| deform::node_on(g, y, m).is_some()).collect(),
    };
    if meshes.is_empty() {
        fail!("reorderDeformers: {} and {} share no geometry", names[0], names[1]);
    }
    for m in meshes {
        let (Some(nx), Some(ny)) = (deform::node_on(g, x, m), deform::node_on(g, y, m)) else {
            fail!("reorderDeformers: both deformers must be on {}", g.path(m));
        };
        if nx != ny {
            swap_in_chain(g, m, nx, ny)?;
        }
    }
    Ok(Value::Null)
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        modes(Cmd::new(
            "deformer",
            "Make a deformer: deformer -type ffd [-n name] objects, with ffd, cluster, softMod, deltaMush, tension, wrap, wire or a \
             nonlinear type. Returns [node]. -q -g gives the deformed shapes, -q -gi their indices. -e -g obj adds geometry, -e -rm -g \
             obj takes it off.",
            deformer_cmd,
        ))
        .positional("objects to deform (the selection when none); the deformer with -e and -q")
        .flag(f("type", "typ", ArgType::String, "The node type to make."))
        .flag(f("name", "n", ArgType::String, "Name of the deformer."))
        .flag(f("geometry", "g", ArgType::Any, "Geometry to deform, add or remove. Query: the deformed shapes."))
        .flag(f("geometryIndices", "gi", ArgType::Bool, "Query: the indices of the deformed shapes."))
        .flag(f("remove", "rm", ArgType::Bool, "With -e -g: take the geometry off the deformer."))
        .flag(f("before", "bf", ArgType::Bool, "Go just above the deformer nearest the mesh."))
        .flag(f("after", "af", ArgType::Bool, "Go nearest the mesh (the default)."))
        .flag(f("frontOfChain", "foc", ArgType::Bool, "Go above every other deformer.")),
    );
    c.register(
        modes(Cmd::new(
            "percent",
            "Set or query deformer weights of points: percent -v 0.5 cluster1 pCube1.vtx[0:3]. -m multiplies what is there. -q -v returns \
             one weight per point. With no points named: the selected components, else all the deformer's points.",
            percent,
        ))
        .positional("the deformer, then points or meshes")
        .flag(f("value", "v", ArgType::Float, "The weight."))
        .flag(f("multiply", "m", ArgType::Bool, "Multiply the weights by the value instead of setting them.")),
    );
    c.register(
        Cmd::new(
            "reorderDeformers",
            "Swap two deformers' places in a mesh's history: reorderDeformers cluster1 ffd1 pCube1. Without geometry, on every mesh both are on.",
            reorder,
        )
        .positional("two deformers, then optionally the geometry"),
    );
}
