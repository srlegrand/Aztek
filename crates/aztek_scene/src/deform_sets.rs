//! What a deformed mesh gains as in the reference: the intermediate original shape (pCubeShape1Orig) feeding its deformers, and for
//! each deformer a set of points (cluster1Set). The set is the deformer's membership: a point out of it does not move and
//! can't be given weight. Also the deformableShape command.

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use serde_json::json;

use crate::components::{self, ComponentMode};
use crate::deform::{self, f, floats_of, input_mesh, plug, set_floats, Cmd, DEFORMER_SET, WEIGHTS};
use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::node::types;
use crate::object_sets::{self, Member};
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::DagNode;

// ---------------------------------------------------------------------------------------------------------------------
// The original shape

fn is_intermediate(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("mesh") && n.bool(DagNode::INTERMEDIATE_OBJECT))
}

/// Whether a mesh has deformers or a skin in its history.
pub fn is_deformed(g: &Graph, mesh: NodeId) -> bool {
    crate::history::history_of(g, mesh).into_iter().any(|h| g.node(h).is_some_and(|n| crate::rig::is_rig_chain_node(n.ty().name.as_str())))
}

/// Whether a node is an intermediate shape feeding a deformer or a skin.
pub fn is_original(g: &Graph, id: NodeId) -> bool {
    is_intermediate(g, id) && g.destinations(&plug(id, MeshNode::OUT_MESH)).iter().any(|d| g.node(d.node).is_some_and(|n| crate::rig::is_rig_chain_node(n.ty().name.as_str())))
}

/// The intermediate shape feeding this mesh's deformers, if it has one.
pub fn orig_of(g: &Graph, mesh: NodeId) -> Option<NodeId> {
    crate::history::history_of(g, mesh).into_iter().find(|&h| is_intermediate(g, h))
}

/// What the first deformer on a mesh reads: its original shape, made here if the mesh has none. A mesh with no history
/// already has an intermediate shape holding its points, which takes the name; other meshes get one under their transform,
/// after the visible shape, between what made the mesh and the deformer.
pub(crate) fn orig_source(g: &mut Graph, mesh: NodeId, src: PlugRef) -> CmdResult<PlugRef> {
    if is_deformed(g, mesh) {
        return Ok(src);
    }
    let wanted = format!("{}Orig", g.node(mesh).map(|n| n.name().to_string()).unwrap_or_default());
    if is_intermediate(g, src.node) {
        g.rename(src.node, &wanted)?;
        return Ok(src);
    }
    let xf = crate::dag::transform_of(g, mesh);
    let t = types::find("mesh").unwrap();
    let orig = g.create_node(&t, &wanted, if xf == mesh { crate::plug::NO_NODE } else { xf }, false)?;
    g.set_value(plug(orig, DagNode::INTERMEDIATE_OBJECT), PlugValue::Bool(true))?;
    g.connect(&src, &plug(orig, MeshNode::IN_MESH), false)?;
    Ok(plug(orig, MeshNode::OUT_MESH))
}

/// With no deformers left on a mesh, its original between the mesh and what made it has no more use. One that holds the
/// mesh's points stays: the mesh has nothing else.
pub(crate) fn drop_unused_orig(g: &mut Graph, mesh: NodeId) -> CmdResult<()> {
    let chain = crate::history::history_of(g, mesh);
    if chain.iter().any(|&h| g.node(h).is_some_and(|n| crate::rig::is_rig_chain_node(n.ty().name.as_str()))) {
        return Ok(());
    }
    let Some(orig) = chain.into_iter().find(|&h| is_intermediate(g, h)) else { return Ok(()) };
    let Some(up) = g.source(&plug(orig, MeshNode::IN_MESH)) else { return Ok(()) };
    for d in g.destinations(&plug(orig, MeshNode::OUT_MESH)) {
        g.disconnect(&d);
        g.connect(&up, &d, false)?;
    }
    g.delete_node(orig)?;
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Deformer sets

/// A deformer's set.
pub fn set_of(g: &Graph, n: NodeId) -> Option<NodeId> {
    let leader = deform::leader_of(g, n);
    g.source(&plug(leader, DEFORMER_SET)).map(|s| s.node)
}

/// The deformer a set belongs to.
pub fn deformer_of_set(g: &Graph, set: NodeId) -> Option<NodeId> {
    let node = g.node(set)?;
    if !node.ty().is_a(object_sets::TYPE_NAME) {
        return None;
    }
    g.destinations(&plug(set, object_sets::USED_BY)).first().map(|d| deform::leader_of(g, d.node))
}

/// Every valid point of a mesh.
fn all_points(g: &mut Graph, mesh: NodeId) -> Vec<u32> {
    crate::modifiers::cage(g, mesh).map(|m| (0..m.point_capacity()).filter(|&p| m.point_valid(p)).collect()).unwrap_or_default()
}

/// A new deformer's set, holding all of the mesh's points: cluster1Set.
pub(crate) fn make_set(g: &mut Graph, n: NodeId, mesh: NodeId) -> CmdResult<()> {
    let name = format!("{}Set", g.node(n).map(|x| x.name().to_string()).unwrap_or_default());
    let ids = all_points(g, mesh);
    let set = object_sets::create(g, &name, &[Member { node: mesh, kind: ComponentMode::Vertex, ids }], false)?;
    g.connect(&plug(set, object_sets::USED_BY), &plug(n, DEFORMER_SET), false)?;
    Ok(())
}

/// A deformer's points on one mesh become these. Weights are the caller's to keep in step.
pub(crate) fn replace_members(g: &mut Graph, n: NodeId, mesh: NodeId, ids: &[u32]) -> CmdResult<()> {
    let Some(set) = set_of(g, n) else { return Ok(()) };
    let mut list: Vec<Member> = object_sets::members(g, set).into_iter().filter(|m| m.node != mesh).collect();
    if !ids.is_empty() {
        list.push(Member { node: mesh, kind: ComponentMode::Vertex, ids: ids.to_vec() });
    }
    object_sets::set_members_raw(g, set, &list)
}

/// A further mesh's node joins its leader: its points go in the leader's set, and its own set goes.
pub(crate) fn merge_sets(g: &mut Graph, leader: NodeId, follower: NodeId) -> CmdResult<()> {
    let own = g.source(&plug(follower, DEFORMER_SET)).map(|s| s.node);
    let Some(lead) = g.source(&plug(leader, DEFORMER_SET)).map(|s| s.node) else { return Ok(()) };
    if let Some(own) = own.filter(|&o| o != lead) {
        let mut list = object_sets::members(g, lead);
        list.extend(object_sets::members(g, own));
        object_sets::set_members_raw(g, lead, &list)?;
        g.delete_node(own)?;
    }
    if g.source(&plug(follower, DEFORMER_SET)).is_none() {
        g.connect(&plug(lead, object_sets::USED_BY), &plug(follower, DEFORMER_SET), false)?;
    }
    Ok(())
}

/// Members as a deformer's set holds them: points of mesh shapes. Whole objects are all their points.
pub(crate) fn as_points(g: &mut Graph, list: &[Member]) -> Vec<Member> {
    let mut out = Vec::new();
    for m in list {
        let Some(mesh) = components::as_mesh(g, m.node) else { continue };
        let ids = match m.kind {
            ComponentMode::Object => all_points(g, mesh),
            ComponentMode::Vertex => m.ids.clone(),
            kind => match crate::modifiers::cage(g, mesh) {
                Some(c) => components::convert(&c, kind, &m.ids, ComponentMode::Vertex, false),
                None => continue,
            },
        };
        out.push(Member { node: mesh, kind: ComponentMode::Vertex, ids });
    }
    out
}

/// After a set changed: points taken out have no weight, points put in have the full weight, and a mesh not yet deformed
/// is added to the deformer.
pub(crate) fn members_changed(g: &mut Graph, deformer: NodeId, list: &[Member], added: bool) -> CmdResult<()> {
    for m in list {
        let Some(x) = deform::node_on(g, deformer, m.node) else {
            if added {
                let set = set_of(g, deformer);
                let keep = set.map(|s| object_sets::members(g, s)).unwrap_or_default();
                deform::add_geometry(g, deformer, m.node, &Some(m.ids.clone()))?;
                // Adding geometry put these in again by way of the new node's own set: the set stays what was asked for.
                if let Some(s) = set {
                    object_sets::set_members_raw(g, s, &keep)?;
                }
            }
            continue;
        };
        let Some(input) = input_mesh(g, x) else { continue };
        let size = input.point_capacity() as usize;
        let now = g.node(x).and_then(|n| floats_of(n.data(), WEIGHTS)).map(|w| w.0.clone());
        if added && now.is_none() {
            continue;
        }
        let mut w = now.unwrap_or_else(|| vec![1.0; size]);
        w.resize(size, 0.0);
        for &p in &m.ids {
            if let Some(v) = w.get_mut(p as usize) {
                *v = if added { 1.0 } else { 0.0 };
            }
        }
        set_floats(g, x, WEIGHTS, w)?;
    }
    Ok(())
}

/// Which points of the mesh under this deformer node are in its set; None when it has no set.
pub fn member_mask(g: &Graph, n: NodeId, capacity: usize) -> Option<Vec<bool>> {
    let set = set_of(g, n)?;
    let mesh = deform::mesh_fed_by(g, n)?;
    let mut out = vec![false; capacity];
    for m in object_sets::members(g, set) {
        if m.node != mesh {
            continue;
        }
        for p in m.ids {
            if let Some(x) = out.get_mut(p as usize) {
                *x = true;
            }
        }
    }
    Some(out)
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

fn deformable_shape(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first() else { fail!("deformableShape: name a mesh") };
    let id = crate::rig::find(&s.graph, name, "deformableShape")?;
    let Some(mesh) = components::as_mesh(&s.graph, id) else { fail!("deformableShape: {name} is not a mesh") };
    let g = &mut s.graph;
    let path = g.path(mesh);
    if a.bool("createOriginalGeometry") {
        if orig_of(g, mesh).is_none() {
            let (base, src) = crate::history::upstream(g, mesh)?;
            let orig = orig_source(g, mesh, src)?;
            if g.source(&base) != Some(orig) {
                g.disconnect(&base);
                g.connect(&orig, &base, false)?;
            }
        }
    }
    if a.bool("createOriginalGeometry") || a.bool("originalGeometry") {
        return Ok(json!(orig_of(g, mesh).map(|o| vec![format!("{}.worldMesh", g.path(o))]).unwrap_or_default()));
    }
    if a.bool("outputGeometry") || a.bool("localShapeOutAttr") {
        return Ok(json!([if a.bool("localShapeOutAttr") { "outMesh".to_string() } else { format!("{path}.outMesh") }]));
    }
    if a.bool("localShapeInAttr") {
        return Ok(json!(["inMesh"]));
    }
    if a.bool("worldShapeOutAttr") {
        return Ok(json!(["worldMesh"]));
    }
    fail!("deformableShape: give -cog, -og, -ogm, -lsi, -lso or -wso")
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "deformableShape",
            "The plugs of a deformable shape: -cog makes the intermediate original if there is none and returns its output, -og returns \
             that output, -ogm the shape's own, -lsi and -lso the local in and out attributes, -wso the world out attribute.",
            deformable_shape,
        )
        .positional("a mesh")
        .flag(f("createOriginalGeometry", "cog", ArgType::Bool, "Make the original shape if there is none."))
        .flag(f("originalGeometry", "og", ArgType::Bool, "The original shape's output plug."))
        .flag(f("outputGeometry", "ogm", ArgType::Bool, "The shape's output plug."))
        .flag(f("localShapeInAttr", "lsi", ArgType::Bool, "The local input attribute."))
        .flag(f("localShapeOutAttr", "lso", ArgType::Bool, "The local output attribute."))
        .flag(f("worldShapeOutAttr", "wso", ArgType::Bool, "The world output attribute.")),
    );
}
