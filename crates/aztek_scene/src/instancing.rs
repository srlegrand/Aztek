//!
//! Instances and references, built from connections rather than DAG multi-parenting:
//!
//!   instance:   Source.outMesh -> Instance.inMesh          (shares everything, no stack of its own)
//!   reference:  base -> [own modifiers] -> Reference.inMesh (shares the base under the source's stack)
//!
//! where base is the generator or meshSource at the bottom of the source's stack. Component edits on any member land
//! on the shared base (modifiers::edit_plug and cage resolve an instance to its root), so editing one edits all.

use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;

use aztek_geo::{Index, Mesh, INVALID};
use aztek_ops::{fail, objects, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::mesh_nodes::MeshNode;
use crate::modifiers::{self, chain_base, modifier_plugs, MeshSourceNode};
use crate::node::types;
use crate::plug::{flags, NodeId, PlugRef, PlugValue, NO_NODE};
use crate::session::Session;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Shares nothing.
    None,
    /// Owns data other meshes instance or reference.
    Source,
    /// inMesh driven by another mesh's outMesh.
    Instance,
    /// Shares the source's base mesh, with a modifier stack of its own.
    Reference,
}

pub fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Source => "source",
        Kind::Instance => "instance",
        Kind::Reference => "reference",
        Kind::None => "none",
    }
}

fn is_mesh(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("mesh"))
}

fn plug(node: NodeId, attr: usize) -> PlugRef {
    PlugRef::new(node, attr as i32)
}

fn in_attr_of(g: &Graph, id: NodeId) -> Option<usize> {
    let n = g.node(id)?;
    if n.ty().is_a("mesh") {
        return Some(MeshNode::IN_MESH);
    }
    if let Some(m) = n.behavior().as_modifier() {
        return Some(m.in_attr());
    }
    n.ty().attr_index("inMesh")
}

/// Copies every storable input value that isn't driven by a connection; driven ones share their source.
fn copy_node(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    let t = g.node(from).unwrap().type_arc().clone();
    for (i, def) in t.attrs.iter().enumerate() {
        if def.flags & flags::INPUT == 0 {
            continue;
        }
        if def.flags & flags::ARRAY != 0 {
            g.copy_elements(from, to, i as i32, true)?;
        } else if let Some(src) = g.source(&plug(from, i)) {
            g.connect(&src, &plug(to, i), false)?;
        } else {
            let v = g.node(from).unwrap().value(i).clone();
            g.set_value(plug(to, i), v)?;
        }
    }
    Ok(())
}

/// Deletes the non-DAG chain feeding a plug, node by node, as long as nothing else uses it.
fn drop_history(g: &mut Graph, dst: &PlugRef) -> CmdResult<()> {
    let mut src = g.source(dst);
    g.disconnect(dst);
    while let Some(s) = src {
        let Some(up) = g.node(s.node) else { return Ok(()) };
        if up.ty().dag || !g.destinations(&s).is_empty() {
            return Ok(());
        }
        let next = in_attr_of(g, s.node).and_then(|i| g.source(&plug(s.node, i)));
        g.delete_node(s.node)?;
        src = next;
    }
    Ok(())
}

/// The base-sharing member a reference counts as made from: the lowest id among the meshes on its base.
fn reference_source(g: &Graph, mesh: NodeId) -> NodeId {
    let Some(base) = base_plug(g, mesh) else { return NO_NODE };
    let fed = meshes_fed_by(g, &base);
    if fed.len() > 1 {
        fed[0]
    } else {
        NO_NODE
    }
}

/// The mesh whose outMesh drives this mesh's inMesh directly, or NO_NODE.
pub fn instance_parent(g: &Graph, mesh: NodeId) -> NodeId {
    if !is_mesh(g, mesh) {
        return NO_NODE;
    }
    match g.source(&plug(mesh, MeshNode::IN_MESH)) {
        Some(src) if src.attr == MeshNode::OUT_MESH as i32 && is_mesh(g, src.node) => src.node,
        _ => NO_NODE,
    }
}

/// The mesh owning the data: instance_parent followed to its end. The mesh itself when it isn't an instance.
pub fn root(g: &Graph, mesh: NodeId) -> NodeId {
    let mut cur = mesh;
    let mut seen = BTreeSet::from([cur]);
    loop {
        let up = instance_parent(g, cur);
        if up == NO_NODE || !seen.insert(up) {
            return cur;
        }
        cur = up;
    }
}

/// The plug feeding the bottom of a mesh's modifier stack (a generator's or meshSource's output), when there is one.
pub fn base_plug(g: &Graph, mesh: NodeId) -> Option<PlugRef> {
    if !is_mesh(g, mesh) {
        return None;
    }
    let (_, src) = chain_base(g, mesh);
    let src = src?;
    if g.node(src.node)?.ty().dag {
        return None;
    }
    Some(src)
}

/// Mesh nodes whose stack bottom is fed by `plug`, directly or through a chain of modifiers.
pub fn meshes_fed_by(g: &Graph, p: &PlugRef) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut todo = g.destinations(p);
    while let Some(d) = todo.pop() {
        let Some(n) = g.node(d.node) else { continue };
        if n.ty().is_a("mesh") {
            if d.attr == MeshNode::IN_MESH as i32 {
                out.push(d.node);
            }
            continue;
        }
        if let Some((input, _, output)) = modifier_plugs(g, d.node) {
            if d.attr == input as i32 {
                todo.extend(g.destinations(&plug(d.node, output)));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Meshes driven directly by `mesh`'s outMesh.
pub fn direct_instances(g: &Graph, mesh: NodeId) -> Vec<NodeId> {
    if !is_mesh(g, mesh) {
        return Vec::new();
    }
    let mut out: Vec<NodeId> = g
        .destinations(&plug(mesh, MeshNode::OUT_MESH))
        .into_iter()
        .filter(|d| d.attr == MeshNode::IN_MESH as i32 && is_mesh(g, d.node))
        .map(|d| d.node)
        .collect();
    out.sort_unstable();
    out
}

/// Every mesh sharing data with `mesh` through instance links or a shared base, itself included, by id.
/// The shape nodes that are one shape to a script: this mesh and its instances, not what merely references them. In
/// the reference these are one node under several parents; here each parent holds a node of its own, kept the same.
pub fn one_shape(g: &Graph, mesh: NodeId) -> Vec<NodeId> {
    if !is_mesh(g, mesh) {
        return Vec::new();
    }
    let mut seen = BTreeSet::from([mesh]);
    let mut todo = vec![mesh];
    while let Some(m) = todo.pop() {
        let up = instance_parent(g, m);
        for id in std::iter::once(up).chain(direct_instances(g, m)) {
            if id != NO_NODE && seen.insert(id) {
                todo.push(id);
            }
        }
    }
    seen.into_iter().collect()
}

pub fn members(g: &Graph, mesh: NodeId) -> Vec<NodeId> {
    if !is_mesh(g, mesh) {
        return Vec::new();
    }
    let mut seen = BTreeSet::from([mesh]);
    let mut todo = vec![mesh];
    while let Some(m) = todo.pop() {
        let mut visit = |id: NodeId| {
            if id != NO_NODE && seen.insert(id) {
                todo.push(id);
            }
        };
        visit(instance_parent(g, m));
        for c in direct_instances(g, m) {
            visit(c);
        }
        if let Some(base) = base_plug(g, m) {
            for r in meshes_fed_by(g, &base) {
                visit(r);
            }
        }
    }
    seen.into_iter().collect()
}

pub fn kind(g: &Graph, mesh: NodeId) -> Kind {
    if !is_mesh(g, mesh) {
        return Kind::None;
    }
    if instance_parent(g, mesh) != NO_NODE {
        return Kind::Instance;
    }
    let ref_source = reference_source(g, mesh);
    if ref_source != NO_NODE && ref_source != mesh {
        return Kind::Reference;
    }
    if ref_source == mesh || !direct_instances(g, mesh).is_empty() {
        return Kind::Source;
    }
    Kind::None
}

/// What a member shares from: an instance's root, a reference's source, the mesh itself for a source, else NO_NODE.
pub fn source(g: &Graph, mesh: NodeId) -> NodeId {
    match kind(g, mesh) {
        Kind::Instance => root(g, mesh),
        Kind::Reference => reference_source(g, mesh),
        Kind::Source => mesh,
        Kind::None => NO_NODE,
    }
}

fn create(g: &mut Graph, type_name: &str) -> CmdResult<NodeId> {
    let t = types::find(type_name).unwrap();
    Ok(g.create_node(&t, "", NO_NODE, true)?)
}

/// Makes `target` (a mesh node with no history of its own) an instance of `src`'s root: its inMesh is driven by the
/// root's outMesh.
pub fn connect_instance(g: &mut Graph, src: NodeId, target: NodeId) -> CmdResult<()> {
    let from = root(g, src);
    let input = plug(target, MeshNode::IN_MESH);
    drop_history(g, &input)?;
    g.set_value(input, PlugValue::Mesh(None))?;
    g.connect(&plug(from, MeshNode::OUT_MESH), &input, false)?;
    Ok(())
}

/// Makes `target` (a mesh node with no history of its own) a reference of `src`'s root: the root's base moves into a
/// meshSource when it stores it itself, and `target` gets copies of the root's modifiers on top of that shared base.
pub fn connect_reference(g: &mut Graph, src: NodeId, target: NodeId) -> CmdResult<()> {
    let from = root(g, src);
    let (bottom, base) = chain_base(g, from);
    let base = match base {
        Some(b) => b,
        None => {
            // The source stores its base itself; it moves into a meshSource both can share.
            let holder = create(g, "meshSource")?;
            let stored = g.stored_value(&bottom).clone();
            g.set_value(plug(holder, MeshSourceNode::MESH), stored)?;
            g.set_value(bottom, PlugValue::Mesh(None))?;
            let b = plug(holder, MeshSourceNode::OUT_MESH);
            g.connect(&b, &bottom, false)?;
            b
        }
    };
    let input = plug(target, MeshNode::IN_MESH);
    drop_history(g, &input)?;
    g.set_value(input, PlugValue::Mesh(None))?;
    let mut top = input;
    for &m in modifiers::stack(g, from).iter().rev() {
        let t = g.node(m).unwrap().type_arc().clone();
        let copy = g.create_node(&t, "", NO_NODE, true)?;
        let (copy_in, _, copy_out) = modifier_plugs(g, copy).unwrap();
        for (i, def) in t.attrs.iter().enumerate() {
            if def.flags & flags::INPUT == 0 || i == copy_in {
                continue;
            }
            if def.flags & flags::ARRAY != 0 {
                g.copy_elements(m, copy, i as i32, true)?;
            } else if let Some(s) = g.source(&plug(m, i)) {
                g.connect(&s, &plug(copy, i), false)?;
            } else {
                let v = g.node(m).unwrap().value(i).clone();
                g.set_value(plug(copy, i), v)?;
            }
        }
        g.connect(&plug(copy, copy_out), &top, false)?;
        top = plug(copy, copy_in);
    }
    g.connect(&base, &top, false)?;
    Ok(())
}

/// Copies the non-DAG history feeding src_mesh (generator, modifiers, base source) onto dst_mesh, or its stored base
/// when it has none, so dst_mesh gets a live stack of its own. `copies` maps history nodes already copied, so
/// upstream nodes shared within one operation are copied once. A DAG node feeding the stack is shared, not copied.
pub fn copy_history(g: &mut Graph, src_mesh: NodeId, dst_mesh: NodeId, copies: &mut HashMap<NodeId, NodeId>) -> CmdResult<()> {
    let mut src_plug = plug(src_mesh, MeshNode::IN_MESH);
    let mut dst_plug = plug(dst_mesh, MeshNode::IN_MESH);
    if g.source(&src_plug).is_none() {
        let v = g.stored_value(&src_plug).clone();
        g.set_value(dst_plug, v)?;
        return Ok(());
    }
    // Walk the chain from the mesh down, copying each history node and reconnecting the copies in the same shape.
    while let Some(src) = g.source(&src_plug) {
        let up = src.node;
        let t = g.node(up).unwrap().type_arc().clone();
        if t.dag {
            g.connect(&src, &dst_plug, false)?;
            break;
        }
        let copy_id = match copies.get(&up) {
            Some(&c) => c,
            None => {
                let copy = g.create_node(&t, "", NO_NODE, true)?;
                for (i, def) in t.attrs.iter().enumerate() {
                    if def.flags & flags::INPUT == 0 {
                        continue;
                    }
                    if def.flags & flags::ARRAY != 0 {
                        g.copy_elements(up, copy, i as i32, false)?;
                    } else if g.source(&plug(up, i)).is_none() {
                        let v = g.node(up).unwrap().value(i).clone();
                        g.set_value(plug(copy, i), v)?;
                    }
                }
                copies.insert(up, copy);
                copy
            }
        };
        g.connect(&PlugRef::new(copy_id, src.attr), &dst_plug, false)?;
        let Some(in_attr) = t.attr_index("inMesh") else { break };
        src_plug = plug(up, in_attr);
        dst_plug = plug(copy_id, in_attr);
    }
    Ok(())
}

/// Moves `from`'s data (its inMesh connection or stored base) to `to`, one of its direct instances, and re-points its
/// other direct instances at `to`. `from` is left without a mesh.
pub fn promote(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    let from_in = plug(from, MeshNode::IN_MESH);
    let to_in = plug(to, MeshNode::IN_MESH);
    let others = direct_instances(g, from);
    g.disconnect(&to_in);
    if let Some(src) = g.source(&from_in) {
        g.disconnect(&from_in);
        g.connect(&src, &to_in, false)?;
    } else {
        let v = g.stored_value(&from_in).clone();
        g.set_value(to_in, v)?;
        g.set_value(from_in, PlugValue::Mesh(None))?;
    }
    for other in others {
        if other != to {
            let input = plug(other, MeshNode::IN_MESH);
            g.disconnect(&input);
            g.connect(&plug(to, MeshNode::OUT_MESH), &input, false)?;
        }
    }
    Ok(())
}

fn own_base(g: &mut Graph, mesh: NodeId) -> CmdResult<()> {
    let (bottom, base) = chain_base(g, mesh);
    let base = base.unwrap();
    let t = g.node(base.node).unwrap().type_arc().clone();
    let own = g.create_node(&t, "", NO_NODE, true)?;
    copy_node(g, base.node, own)?;
    g.disconnect(&bottom);
    g.connect(&PlugRef::new(own, base.attr), &bottom, false)?;
    Ok(())
}

/// Gives a member its own data: an instance gets a copy of its root's history, a reference its own copy of the
/// shared base, and a source hands its data to its first instance and keeps a copy. False when it shares nothing.
pub fn make_unique(g: &mut Graph, mesh: NodeId) -> CmdResult<bool> {
    match kind(g, mesh) {
        Kind::Instance => {
            let from = root(g, mesh);
            g.disconnect(&plug(mesh, MeshNode::IN_MESH));
            copy_history(g, from, mesh, &mut HashMap::new())?;
            Ok(true)
        }
        Kind::Reference => {
            own_base(g, mesh)?;
            Ok(true)
        }
        Kind::Source => {
            let inst = direct_instances(g, mesh);
            if let Some(&heir) = inst.first() {
                promote(g, mesh, heir)?;
                copy_history(g, heir, mesh, &mut HashMap::new())?;
                return Ok(true);
            }
            // Only references share its base: it keeps the stack and takes a base of its own.
            own_base(g, mesh)?;
            Ok(true)
        }
        Kind::None => Ok(false),
    }
}

/// Same topology (point and face order) and points within tolerance, object space.
pub fn same_geometry(a: &Mesh, b: &Mesh, tolerance: f64) -> bool {
    if a.num_points() != b.num_points() || a.num_faces() != b.num_faces() || a.num_corners() != b.num_corners() {
        return false;
    }
    // Compact numbering on both sides, so holes left by deleted elements don't matter.
    let numbering = |m: &Mesh| {
        let mut out = vec![INVALID; m.point_capacity() as usize];
        let mut next = 0;
        for p in 0..m.point_capacity() {
            if m.point_valid(p) {
                out[p as usize] = next;
                next += 1;
            }
        }
        out
    };
    let (na, nb) = (numbering(a), numbering(b));
    let pa: Vec<Index> = (0..a.point_capacity()).filter(|&p| a.point_valid(p)).collect();
    let pb: Vec<Index> = (0..b.point_capacity()).filter(|&p| b.point_valid(p)).collect();
    let tol2 = tolerance * tolerance;
    for i in 0..pa.len() {
        let d = a.position(pa[i]) - b.position(pb[i]);
        if d.dot(d) > tol2 {
            return false;
        }
    }
    let fa: Vec<Index> = (0..a.face_capacity()).filter(|&f| a.face_valid(f)).collect();
    let fb: Vec<Index> = (0..b.face_capacity()).filter(|&f| b.face_valid(f)).collect();
    for i in 0..fa.len() {
        let ca = a.face_points(fa[i]);
        let cb = b.face_points(fb[i]);
        if ca.len() != cb.len() {
            return false;
        }
        for k in 0..ca.len() {
            if na[ca[k] as usize] != nb[cb[k] as usize] {
                return false;
            }
        }
    }
    true
}

/// Drops target's own history and makes it an instance of src's root.
pub fn replace_with_instance(g: &mut Graph, src: NodeId, target: NodeId) -> CmdResult<()> {
    for id in modifiers::stack(g, target) {
        if modifiers::editing_input(g, id) {
            fail!("{} is editing its edges: apply or cancel that first", crate::components::display_path(g, id));
        }
    }
    connect_instance(g, src, target)
}

/// Before nodes are deleted: each doomed source hands its data to its first surviving instance. The graph's delete
/// hook.
pub fn before_delete(g: &mut Graph, doomed: &[NodeId]) {
    let gone: BTreeSet<NodeId> = doomed.iter().copied().collect();
    for &id in doomed {
        let Some(n) = g.node(id) else { continue };
        // A file reference keeps its own connections; unloading one must not rewire the scene around it.
        if n.reference_id() != 0 || !n.ty().is_a("mesh") {
            continue;
        }
        for heir in direct_instances(g, id) {
            if !gone.contains(&heir) && g.node(heir).is_some_and(|h| h.reference_id() == 0) {
                if let Err(e) = promote(g, id, heir) {
                    log::warn!("{e}");
                }
                break;
            }
        }
    }
}

/// Installs before_delete as the graph's delete hook.
pub fn install(g: &mut Graph) {
    g.set_delete_hook(Arc::new(|g: &mut Graph, doomed: &[NodeId]| {
        crate::rig::before_delete(g, doomed);
        before_delete(g, doomed);
    }));
}

type Cmd = CommandDef<Session>;

fn meshes(s: &Session, a: &aztek_ops::Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let g = &s.graph;
    let mut out = Vec::new();
    for name in objects(s, a) {
        let Some(id) = g.find(&name)? else { fail!("{cmd}: no node named '{name}'") };
        if let Some(m) = crate::components::as_mesh(g, id) {
            out.push(m);
        }
    }
    Ok(out)
}

fn names(g: &Graph, ids: &[NodeId]) -> Value {
    Value::Array(ids.iter().map(|&id| json!(crate::components::display_path(g, id))).collect())
}

/// makeUnique, selectInstances, selectInstanceSource, convertToInstance, instanceInfo.
pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "makeUnique",
            "Give instances and references data of their own, so edits to them stop reaching the other members. An \
             instance gets a live copy of its source's stack, a reference its own base mesh; a source hands its data to \
             its first instance and keeps a copy. Returns the meshes changed.",
            |s, a| {
                let ids = meshes(s, a, "makeUnique")?;
                let mut changed = Vec::new();
                for id in ids {
                    if make_unique(&mut s.graph, id)? {
                        changed.push(id);
                    }
                }
                Ok(names(&s.graph, &changed))
            },
        )
        .positional("meshes")
        .examples(&["makeUnique(\"Bolt3\")"]),
    );

    c.register(
        Cmd::new(
            "selectInstances",
            "Select every mesh sharing data with the given ones: their source, instances and references. Returns the selection.",
            |s, a| {
                let ids = meshes(s, a, "selectInstances")?;
                let g = &mut s.graph;
                let mut out: Vec<NodeId> = Vec::new();
                for id in ids {
                    for m in members(g, id) {
                        if !out.contains(&m) {
                            out.push(m);
                        }
                    }
                }
                g.select(&out, if a.bool("add") { SelectMode::Add } else { SelectMode::Replace });
                Ok(names(g, &out))
            },
        )
        .positional("meshes")
        .flag(FlagDef::new("add", "", ArgType::Bool, json!(false), "Add to the selection instead of replacing it.")),
    );

    c.register(
        Cmd::new(
            "selectInstanceSource",
            "Select the source of each given instance or reference (a source selects itself). Returns the selection.",
            |s, a| {
                let ids = meshes(s, a, "selectInstanceSource")?;
                let g = &mut s.graph;
                let mut out: Vec<NodeId> = Vec::new();
                for id in ids {
                    let src = source(g, id);
                    if src != NO_NODE && !out.contains(&src) {
                        out.push(src);
                    }
                }
                if out.is_empty() {
                    fail!("selectInstanceSource: none of these meshes is an instance or reference");
                }
                g.select(&out, SelectMode::Replace);
                Ok(names(g, &out))
            },
        )
        .positional("meshes"),
    );

    c.register(
        Cmd::new(
            "convertToInstance",
            "Replace meshes identical to a source with instances of it, dropping their own copies of the data: cleans up \
             imported kits. The source is the last mesh given (the lead); with only the source, every other mesh in the \
             scene is checked. Returns the meshes converted.",
            |s, a| {
                let mut given = meshes(s, a, "convertToInstance")?;
                let g = &mut s.graph;
                let Some(last) = given.pop() else { fail!("convertToInstance: give the meshes to convert and the source last") };
                let src = root(g, last);
                let all = given.is_empty();
                if all {
                    for id in g.nodes() {
                        if is_mesh(g, id) && !g.node(id).unwrap().ty().is_a("lattice") && g.node(id).unwrap().reference_id() == 0 {
                            given.push(id);
                        }
                    }
                }
                g.evaluate();
                let Some(want) = g.node(src).unwrap().data().mesh(MeshNode::OUT_MESH).cloned() else {
                    fail!("convertToInstance: {} has no mesh", crate::components::display_path(g, src))
                };
                let tol = a.float("tolerance", 0.001);
                let force = a.bool("force") && !all;
                let mut converted = Vec::new();
                for id in given {
                    if id == src || root(g, id) == src {
                        continue;
                    }
                    let have = g.node(id).unwrap().data().mesh(MeshNode::OUT_MESH).cloned();
                    if !force && !have.is_some_and(|h| same_geometry(&want, &h, tol)) {
                        continue;
                    }
                    replace_with_instance(g, src, id)?;
                    converted.push(id);
                }
                Ok(names(g, &converted))
            },
        )
        .positional("meshes... source")
        .flag(FlagDef::new("tolerance", "", ArgType::Float, json!(0.001), "Points may differ by this much, object space, cm.").min(0.0))
        .flag(FlagDef::new(
            "force",
            "",
            ArgType::Bool,
            json!(false),
            "Convert the given meshes even when their geometry differs from the source's.",
        ))
        .examples(&["convertToInstance(\"Bolt\")  # every copy of Bolt in the scene", "convertToInstance(\"Bolt2\", \"Bolt3\", \"Bolt\")"]),
    );

    c.register(
        Cmd::new("instanceInfo", "How a mesh shares data: {kind: none | source | instance | reference, source, members}.", |s, a| {
            let objs = objects(s, a);
            let Some(last) = objs.last() else { fail!("instanceInfo: no mesh") };
            let g = &s.graph;
            let Some(id) = g.find(last)?.and_then(|id| crate::components::as_mesh(g, id)) else { fail!("instanceInfo: {last} is not a mesh") };
            let k = kind(g, id);
            let src = source(g, id);
            Ok(json!({
                "kind": kind_name(k),
                "source": if src == NO_NODE { Value::Null } else { json!(crate::components::display_path(g, src)) },
                "members": if k == Kind::None { json!([]) } else { names(g, &members(g, id)) },
            }))
        })
        .positional("mesh")
        .undoable(false),
    );
}
