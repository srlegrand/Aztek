//! The core commands: undo, scene and transform commands.
//!
//! Where a command reaches into a module that is not implemented yet, it does what it can without it, as noted:
//! materials (rename keeps slot names in step), components (per-axis edits on selected components), rig (IK
//! controllers posing their chains, joints' own pivots), references, the prefs store and the event bus.

use std::collections::BTreeMap;

use aztek_ops::{
    fail, objects, run, ArgType, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef,
};
use glam::{DMat3, DVec3, DVec4};
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::naming::NamingPolicy;
use crate::node::types;
use crate::plug::{
    default_for, flags, from_json, to_json, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE,
};
use crate::session::{Session, WalkGhost, WalkGhosts};
use crate::transform::{euler_to_matrix, RotateOrder, Transform};
use crate::transform_ops::{self as xf, Space};

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, short: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, default, doc)
}

fn node_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    g.find(name)?
        .ok_or_else(|| CommandError::new(format!("no node named '{name}'")))
}

fn ids_of(g: &Graph, names: &[String]) -> CmdResult<Vec<NodeId>> {
    names.iter().map(|n| node_named(g, n)).collect()
}

fn names_of(g: &Graph, ids: &[NodeId]) -> Value {
    Value::Array(
        ids.iter()
            .filter(|&&id| g.node(id).is_some())
            .map(|&id| json!(g.path(id)))
            .collect(),
    )
}

fn plug_arg(g: &Graph, v: &Value) -> CmdResult<PlugRef> {
    let path = v
        .as_str()
        .ok_or_else(|| CommandError::new(format!("expected a plug, got {v}")))?;
    Ok(g.plug(path)?)
}

fn vec3(v: [f64; 3]) -> DVec3 {
    DVec3::from_array(v)
}

fn vec3_json(v: DVec3) -> Value {
    json!([v.x, v.y, v.z])
}

/// Renaming material slots along with `Graph::rename` waits for materials.
fn rename_node(g: &mut Graph, id: NodeId, name: &str) -> CmdResult<String> {
    Ok(g.rename(id, name)?)
}

fn parent_arg(g: &Graph, a: &Args) -> CmdResult<NodeId> {
    if !a.has("parent") {
        return Ok(NO_NODE);
    }
    node_named(g, &a.string("parent", ""))
}

fn create(s: &mut Session, type_name: &str, a: &Args) -> CmdResult {
    let Some(t) = types::find(type_name) else {
        fail!("unknown node type '{type_name}'")
    };
    let parent = parent_arg(&s.graph, a)?;
    // A shape made on its own gets a transform of its own, as the reference does: createNode locator makes locator1 and
    // locatorShape1 and returns the shape.
    if t.is_a("shape") && (parent == NO_NODE || !crate::dag::is_transform(&s.graph, parent)) {
        let stem = format!("{}1", crate::dag::transform_stem(&t.name));
        let tt = types::find("transform").unwrap();
        let tr = s.graph.create_node(&tt, &stem, parent, true)?;
        let shape_name = match a.string("name", "") {
            n if n.is_empty() => crate::dag::shape_name(s.graph.node(tr).unwrap().name()),
            n => n,
        };
        let id = s.graph.create_node(&t, &shape_name, tr, true)?;
        if !a.bool("skipSelect") {
            s.graph.select(&[tr], SelectMode::Replace);
        }
        return Ok(json!(s.graph.path(id)));
    }
    let id = s
        .graph
        .create_node(&t, &a.string("name", ""), parent, true)?;
    if !a.bool("skipSelect") && t.dag {
        s.graph.select(&[id], SelectMode::Replace);
    }
    Ok(json!(s.graph.path(id)))
}

/// The transforms among ids, the ones pickWalk steps between.
fn walkable(g: &Graph, ids: &[NodeId]) -> Vec<NodeId> {
    ids.iter()
        .copied()
        .filter(|&id| g.node(id).is_some_and(|n| n.ty().is_a("transform")))
        .collect()
}

/// The remembered deleted objects while nothing has been selected since and they are still gone.
fn live_walk_ghosts<'a>(w: &'a WalkGhosts, g: &Graph) -> Option<&'a [WalkGhost]> {
    if w.ghosts.is_empty()
        || w.selection_serial != g.selection_serial()
        || !g.selection().is_empty()
    {
        return None;
    }
    if w.ghosts.iter().any(|gh| g.node(gh.id).is_some()) {
        return None;
    }
    Some(&w.ghosts)
}

/// Where pickWalk goes from a deleted object: its nearest surviving sibling that way, or its parent.
fn walk_from_ghost(g: &Graph, ghost: &WalkGhost, dir: &str) -> NodeId {
    if dir == "up" {
        return if ghost.parent != NO_NODE && g.node(ghost.parent).is_some() {
            ghost.parent
        } else {
            NO_NODE
        };
    }
    if dir != "left" && dir != "right" {
        return NO_NODE;
    }
    let Some(from) = ghost.siblings.iter().position(|&s| s == ghost.id) else {
        return NO_NODE;
    };
    let n = ghost.siblings.len();
    for step in 1..n {
        let sib = ghost.siblings[if dir == "right" {
            (from + step) % n
        } else {
            (from + n - step) % n
        }];
        if g.node(sib).is_some() && g.parent(sib) == ghost.parent {
            return sib;
        }
    }
    NO_NODE
}

/// Whether a child of compound `group` on the node is an array: from its declaration, or from its children.
fn compound_is_multi(s: &Session, id: NodeId, group: &str) -> CmdResult<bool> {
    if group.contains('.') || group.contains('[') {
        fail!("addAttr: compounds nest one level; '{group}' is already a child");
    }
    if let Some(&multi) = s.pending_compounds.get(&(id, group.to_string())) {
        return Ok(multi);
    }
    let n = s.graph.node(id).unwrap();
    let children = n.ty().group_children(group);
    if children.is_empty() {
        fail!(
            "addAttr: '{}' has no compound '{group}'; add it with attributeType='compound' first",
            n.name()
        );
    }
    Ok(n.ty().attrs[children[0]].is_array())
}

/// Every added attribute under the category, and the category's name out of usdApiSchemas (USD components).
fn delete_category(g: &mut Graph, id: NodeId, category: &str) -> CmdResult<()> {
    let count = g.node(id).unwrap().ty().attrs.len();
    for i in (0..count).rev() {
        let d = &g.node(id).unwrap().ty().attrs[i];
        if d.has(flags::USER) && d.category == category {
            g.delete_attr(&PlugRef::new(id, i as i32))?;
        }
    }
    let n = g.node(id).unwrap();
    let Some(list) = n.ty().attr_index("usdApiSchemas") else {
        return Ok(());
    };
    let d = &n.ty().attrs[list];
    if d.ty != PlugType::String || !d.is_array() {
        return Ok(());
    }
    let els = n.elements(list);
    let kept: Vec<PlugValue> = els
        .iter()
        .filter(|(_, v)| v.as_str() != category)
        .map(|(_, v)| v.clone())
        .collect();
    if kept.len() != els.len() {
        g.set_elements(&PlugRef::new(id, list as i32), &kept)?;
    }
    Ok(())
}

fn transforms_of(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut ids = Vec::new();
    for name in objects(s, a) {
        let Some(id) = s.graph.find(&name)? else {
            fail!("{cmd}: no node named '{name}'")
        };
        if s.graph.node(id).unwrap().ty().is_a("transform") {
            ids.push(id);
        }
    }
    if ids.is_empty() {
        fail!("{cmd}: nothing to transform");
    }
    Ok(ids)
}

/// x=, y=, z=: the axes named, with NaN for the others; None when none is.
fn axis_args(a: &Args) -> Option<DVec3> {
    let mut v = DVec3::NAN;
    let mut any = false;
    for (i, f) in ["x", "y", "z"].into_iter().enumerate() {
        if a.has(f) {
            v[i] = a.float(f, 0.0);
            any = true;
        }
    }
    any.then_some(v)
}

/// The axes named in v replace those of current.
fn with_axes(mut current: DVec3, v: DVec3) -> DVec3 {
    for i in 0..3 {
        if !v[i].is_nan() {
            current[i] = v[i];
        }
    }
    current
}

/// Per-axis flags act on the component selection when there is one and no objects were named. Component selection
/// is not implemented yet, so there never is.
fn on_components(_a: &Args) -> bool {
    false
}

fn space_arg(a: &Args) -> CmdResult<Space> {
    match a.string("space", "world").as_str() {
        "world" => Ok(Space::World),
        "object" => Ok(Space::Object),
        "parent" => Ok(Space::Parent),
        _ => fail!("space must be 'world', 'object' or 'parent'"),
    }
}

fn euler_rot(degrees: DVec3) -> DMat3 {
    DMat3::from_mat4(euler_to_matrix(degrees, RotateOrder::Xyz))
}

fn set_vec(g: &mut Graph, id: NodeId, attr: usize, v: DVec3) -> CmdResult<()> {
    Ok(g.set_value(PlugRef::new(id, attr as i32), v.into())?)
}

fn set_vec_unlocked(g: &mut Graph, id: NodeId, attr: usize, v: DVec3) -> CmdResult<()> {
    let p = PlugRef::new(id, attr as i32);
    let v = g.keep_locked(&p, v.into());
    Ok(g.set_value(p, v)?)
}

/// Joins plugs that measure different things through a unitConversion node, one per axis of a whole vector.
fn connect_converted(s: &mut Session, src: PlugRef, dst: PlugRef, factor: f64, force: bool) -> CmdResult {
    let whole = s.graph.attr(&dst).ty == PlugType::Vec3 && dst.component < 0 && src.component < 0 && s.graph.attr(&src).ty == PlugType::Vec3;
    let pairs: Vec<(PlugRef, PlugRef)> = if whole {
        (0..3).map(|k| (PlugRef::component(src.node, src.attr, k), PlugRef::component(dst.node, dst.attr, k))).collect()
    } else {
        vec![(src, dst.clone())]
    };
    for (from, to) in pairs {
        let node = s.run("createNode", json!({"args": ["unitConversion"]}))?;
        let node = node.as_str().unwrap_or_default().to_string();
        s.run("setAttr", json!({"args": [format!("{node}.conversionFactor"), factor]}))?;
        let g = &mut s.graph;
        let (input, output) = (g.plug(&format!("{node}.input"))?, g.plug(&format!("{node}.output"))?);
        g.connect(&from, &input, false)?;
        g.connect(&output, &to, force)?;
    }
    Ok(json!(s.graph.plug_name(&dst)))
}

pub fn register(c: &mut Commands<Session>) {
    register_undo(c);
    register_scene(c);
    register_attrs(c);
    register_queries(c);
    register_transform(c);
}

fn register_undo(c: &mut Commands<Session>) {
    c.register(
        Cmd::new("undo", "Undo the last command.", |s, _| {
            let label = s.graph.undo_stack().undo_label();
            if !s.graph.undo() {
                fail!("nothing to undo");
            }
            log::info!("undo: {label}");
            Ok(json!(label))
        })
        .undoable(false),
    );
    c.register(
        Cmd::new("redo", "Redo the last undone command.", |s, _| {
            let label = s.graph.undo_stack().redo_label();
            if !s.graph.redo() {
                fail!("nothing to redo");
            }
            log::info!("redo: {label}");
            Ok(json!(label))
        })
        .undoable(false),
    );
    c.register(
        Cmd::new(
            "flushUndo",
            "Clear undo history. which='past' drops the steps undo would revert, so undo cannot step back past this point; \
             'future' drops the redo steps; 'all' drops both.",
            |s, a| {
                let which = a.string("which", "past");
                let u = s.graph.undo_stack_mut();
                match which.as_str() {
                    "past" => u.flush_past(),
                    "future" => u.flush_future(),
                    "all" => u.flush(),
                    _ => fail!("flushUndo: which must be past, future or all"),
                }
                log::info!("undo history flushed ({which})");
                Ok(Value::Null)
            },
        )
        .flag(flag("which", "w", T::String, json!("past"), "past, future or all.").choices(&["past", "future", "all"]))
        .undoable(false),
    );
    c.register(
        Cmd::new(
            "undoJump",
            "Undo or redo in one go until the given number of undo steps remain, as listed by undoInfo(history=True).",
            |s, a| {
                let u = s.graph.undo_stack();
                let pos = a.int("position", u.undo_count() as i64);
                let total = u.undo_count() + u.redo_count();
                if pos < 0 || !s.graph.jump_to(pos as usize) {
                    fail!("undoJump: position must be 0 to {total}");
                }
                Ok(json!(s.graph.undo_stack().undo_count()))
            },
        )
        .flag(flag("position", "p", T::Int, Value::Null, "Undo steps left afterwards: 0 undoes everything.").min(0.0))
        .undoable(false),
    );
    c.register(
        Cmd::new(
            "undoInfo",
            "Query or control the undo queue. openChunk/closeChunk group everything in between into one undo step.",
            |s, a| {
                let u = s.graph.undo_stack_mut();
                if a.bool("openChunk") {
                    u.open_chunk(a.string("chunkName", "chunk"));
                }
                if a.bool("closeChunk") {
                    u.close_chunk();
                }
                if a.has("state") {
                    u.set_enabled(a.bool("state"));
                }
                if a.has("limit") {
                    u.set_limit(a.int("limit", 0).max(0) as usize);
                }
                if a.has("memoryBudget") {
                    u.set_byte_budget((a.float("memoryBudget", 0.0).max(0.0) * 1024.0 * 1024.0) as usize);
                }
                let mut out = json!({"undo": u.undo_label(), "redo": u.redo_label(), "state": u.enabled(), "memory": u.memory_bytes()});
                if a.bool("history") {
                    let steps: Vec<Value> = u.steps().iter().map(|s| json!({"label": s.label, "selection": s.selection_only})).collect();
                    out["steps"] = Value::Array(steps);
                    out["position"] = json!(u.undo_count());
                }
                Ok(out)
            },
        )
        .flag(flag("openChunk", "ock", T::Bool, Value::Null, "Start a chunk."))
        .flag(flag("closeChunk", "cck", T::Bool, Value::Null, "End a chunk."))
        .flag(flag("chunkName", "cn", T::String, Value::Null, "Label for openChunk."))
        .flag(flag("state", "st", T::Bool, Value::Null, "Turn undo recording on or off."))
        .flag(flag("limit", "l", T::Int, Value::Null, "Max undo steps, 0 for infinite.").min(0.0))
        .flag(flag("memoryBudget", "mb", T::Float, Value::Null, "Drop the oldest steps past this many MB, 0 for unlimited.").min(0.0))
        .flag(flag("history", "h", T::Bool, Value::Null, "Also return every step's label, oldest first, and the current position."))
        .undoable(false),
    );
}

fn register_scene(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "namingConvention",
            "Naming rules for this workspace (.aztek/naming.json). Query with no flags; check lists violations; fix renames \
             them, whether or not enforcement is on. check and fix cover the given objects and everything under them, or the \
             whole scene; referenced nodes keep their names.",
            |s, a| {
                for (f, key) in [("enable", "enabled"), ("mode", "mode"), ("uniqueNames", "uniqueNames")] {
                    if let Some(v) = a.get(f) {
                        s.naming_prefs[key] = v.clone();
                    }
                }
                if a.has("enable") || a.has("mode") || a.has("uniqueNames") {
                    s.load_naming_policy();
                }
                if a.bool("check") || a.bool("fix") {
                    let mut policy = s.graph.naming_policy().clone();
                    policy.enabled = true;
                    let mut ids = Vec::new();
                    if a.positional.is_empty() {
                        ids = s.graph.nodes();
                    } else {
                        for o in a.named() {
                            let Some(id) = s.graph.find(&o)? else { fail!("namingConvention: no object '{o}'") };
                            ids.push(id);
                            ids.extend(s.graph.descendants(id));
                        }
                    }
                    let mut out = Vec::new();
                    for id in ids {
                        let n = s.graph.node(id).unwrap();
                        if n.reference_id() != 0 {
                            continue;
                        }
                        let was = n.name().to_string();
                        let mut want = policy.conform(n.ty(), &was);
                        if want == was {
                            continue;
                        }
                        let at = s.graph.path(id);
                        if a.bool("fix") {
                            want = rename_node(&mut s.graph, id, &want)?;
                        }
                        out.push(json!([at, want]));
                    }
                    return Ok(Value::Array(out));
                }
                let mut q = NamingPolicy::defaults();
                if let (Some(q), Some(p)) = (q.as_object_mut(), s.naming_prefs.as_object()) {
                    for (k, v) in p {
                        q.insert(k.clone(), v.clone());
                    }
                }
                Ok(q)
            },
        )
        .positional("[objects]")
        .flag(flag("enable", "e", T::Bool, Value::Null, "Turn enforcement on or off for the workspace (user prefs if none open)."))
        .flag(flag("mode", "m", T::String, Value::Null, "fix | warn | block"))
        .flag(flag(
            "uniqueNames",
            "un",
            T::Bool,
            Value::Null,
            "Make names unique across the whole scene, not just among siblings (workspace, else user prefs).",
        ))
        .flag(flag("check", "c", T::Bool, json!(false), "Return [[node, expectedName], ...] for names that break the rules."))
        .flag(flag("fix", "f", T::Bool, json!(false), "Rename every node that breaks the rules.")),
    );

    c.register(
        Cmd::new("createNode", "Create a node of the given type. Returns its path (the name, unless another node shares it).", |s, a| {
            let Some(t) = a.pos_str(0) else { fail!("createNode: node type required") };
            let t = t.to_string();
            create(s, &t, a)
        })
        .positional("type")
        .flag(flag("name", "n", T::String, Value::Null, "Name; made unique among its siblings."))
        .flag(flag("parent", "p", T::String, Value::Null, "Parent transform."))
        .flag(flag("skipSelect", "ss", T::Bool, json!(false), "Don't select the new node.")),
    );

    c.register(
        Cmd::new(
            "spaceLocator",
            "Create a locator: a transform with a locator shape under it. Returns [transform].",
            |s, a| {
                let name = a.string("name", "");
                let (tr, sh) = crate::dag::create_with_shape(&mut s.graph, "locator", &name, NO_NODE)?;
                if let Some(p) = a.vec3("position") {
                    s.graph.set_value(PlugRef::new(sh, crate::transform::Locator::LOCAL_POSITION as i32), vec3(p).into())?;
                }
                s.graph.select(&[tr], SelectMode::Replace);
                Ok(json!([s.graph.path(tr)]))
            },
        )
        .flag(flag(
            "name",
            "n",
            T::String,
            Value::Null,
            "Name; made unique among its siblings.",
        ))
        .flag(flag(
            "position",
            "p",
            T::Vec3,
            Value::Null,
            "Where the locator shape sits in its transform's space (localPosition).",
        )),
    );

    c.register(
        Cmd::new(
            "group",
            "Group objects under a new transform, or create an empty group with empty=True.",
            |s, a| {
                let members = if a.bool("empty") {
                    Vec::new()
                } else {
                    ids_of(&s.graph, &objects(s, a))?
                };
                let mut c = a.clone();
                if !c.has("name") {
                    c.flags.insert("name".into(), json!("group1"));
                }
                c.flags.insert("skipSelect".into(), json!(true));
                let name = create(s, "transform", &c)?;
                let g = node_named(&s.graph, name.as_str().unwrap())?;
                for m in members {
                    s.graph.set_parent(m, g, true, usize::MAX)?;
                }
                s.graph.select(&[g], SelectMode::Replace);
                Ok(json!(s.graph.path(g)))
            },
        )
        .positional("objects")
        .flag(flag("name", "n", T::String, Value::Null, "Group name."))
        .flag(flag(
            "empty",
            "em",
            T::Bool,
            json!(false),
            "Create an empty group.",
        ))
        .flag(flag(
            "parent",
            "p",
            T::String,
            Value::Null,
            "Parent for the new group.",
        )),
    );

    c.register(
        Cmd::new("delete", "Delete objects (and their children). Defaults to the selection.", |s, a| {
            // The reference refuses to delete the default cameras.
            if !a.bool("constructionHistory") {
                for n in objects(s, a) {
                    if let Ok(Some(id)) = s.graph.find(&n) {
                        if crate::cameras::is_startup(&s.graph, id) {
                            fail!("delete: {n} is a startup camera; camera -e -startupCamera false lets it go");
                        }
                    }
                }
            }
            let ids = ids_of(&s.graph, &objects(s, a))?;
            let g = &mut s.graph;
            if a.bool("constructionHistory") {
                g.open_chunk("Delete History");
                let r = (|| -> CmdResult<()> {
                    for &id in &ids {
                        let mut meshes: Vec<NodeId> = crate::components::as_mesh(g, id).into_iter().collect();
                        for d in g.descendants(id) {
                            if crate::components::is_mesh(g, d) && !meshes.contains(&d) && !g.node(d).unwrap().bool(crate::transform::DagNode::INTERMEDIATE_OBJECT) {
                                meshes.push(d);
                            }
                        }
                        for m in meshes {
                            crate::history::delete_history(g, m)?;
                        }
                        if let Some(c) = crate::curves::as_curve(g, id) {
                            crate::curves::bake_history(g, c)?;
                        }
                        let mut under = g.descendants(id);
                        under.push(id);
                        for d in under {
                            if crate::surfaces::is_surface(g, d) {
                                crate::surfaces::bake_history(g, d)?;
                            }
                        }
                    }
                    Ok(())
                })();
                g.close_chunk();
                r?;
                return Ok(Value::Null);
            }
            for &id in &ids {
                crate::anim_layers::before_delete(g, id)?;
            }
            // A deformer goes with its handle or lattice, and the chain closes up.
            let ids = crate::deform::expand_delete(g, ids)?;
            // A mesh's history goes with it, unless something else uses it.
            let mut history = Vec::new();
            let mut all_doomed = Vec::new();
            for &id in &ids {
                all_doomed.push(id);
                all_doomed.extend(g.descendants(id));
            }
            history.extend(crate::anim::orphans(g, &all_doomed));
            for &id in &ids {
                let mut under = g.descendants(id);
                under.push(id);
                for d in under {
                    if crate::components::is_mesh(g, d) {
                        let (base, _) = crate::modifiers::chain_base(g, d);
                        history.extend(crate::history::upstream_nodes(g, &base).into_iter().filter(|&h| !g.node(h).unwrap().ty().dag));
                    }
                    if crate::surfaces::is_surface(g, d) {
                        history.extend(crate::surfaces::surface_history(g, d));
                    }
                }
            }
            for &id in &ids {
                let mut doomed = g.descendants(id);
                doomed.push(id);
                for d in doomed {
                    if g.node(d).unwrap().reference_id() != 0 {
                        fail!("delete: '{}' comes from a reference; remove or unload the reference instead", g.path(d));
                    }
                    if g.node(d).unwrap().is_locked() {
                        fail!("delete: cannot delete locked node '{}'", g.path(d));
                    }
                }
            }
            let mut ghosts = Vec::new();
            for &id in &ids {
                if g.is_selected(id) {
                    let parent = g.parent(id);
                    let sibs = if parent == NO_NODE { g.roots().to_vec() } else { g.children(parent).to_vec() };
                    ghosts.push(WalkGhost { id, parent, siblings: walkable(g, &sibs) });
                }
            }
            for id in ids {
                g.delete_node(id)?;
            }
            for h in history {
                if g.node(h).is_some() && g.node(h).unwrap().ty().attrs.iter().enumerate().all(|(i, _)| g.destinations(&PlugRef::new(h, i as i32)).iter().all(|d| crate::anim_layers::is_layer(g, d.node))) {
                    g.delete_node(h)?;
                }
            }
            if g.selection().is_empty() {
                s.walk_ghosts = WalkGhosts { selection_serial: g.selection_serial(), ghosts };
            }
            Ok(Value::Null)
        })
        .positional("objects")
        .flag(FlagDef::new("constructionHistory", "ch", T::Bool, json!(false), "Delete the objects' construction history instead: each mesh keeps what its history makes now.")),
    );

    c.register(
        Cmd::new("rename", "rename(old, new) or rename(new) for the single selected object. Returns the renamed node's path.", |s, a| {
            let g = &mut s.graph;
            let (target, new_name) = match (a.positional.len(), a.pos_str(0), a.pos_str(1)) {
                (2, Some(t), Some(n)) => (t.to_string(), n.to_string()),
                (1, Some(n), _) if g.selection().len() == 1 => (g.path(g.selection()[0]), n.to_string()),
                _ => fail!("rename: expected (object, newName)"),
            };
            let id = node_named(g, &target)?;
            if g.node(id).unwrap().reference_id() != 0 {
                fail!("rename: '{target}' comes from a reference and keeps its name");
            }
            if g.node(id).unwrap().is_locked() {
                fail!("rename: cannot rename locked node '{target}'");
            }
            let new_name = g.resolve_hashes(&new_name);
            rename_node(g, id, &new_name)?;
            Ok(json!(g.path(id)))
        })
        .positional("[object] newName"),
    );

    c.register(
        Cmd::new(
            "parent",
            "parent(child..., parent), or parent(child..., world=True). Keeps world transforms unless relative=True. chain=True \
             parents each object under the next one, in order: (a, b, c) puts a under b and b under c. index=N places the \
             objects, in order, at position N among the parent's children (0 is first), which also reorders objects already \
             under that parent. Leaves the moved objects selected.",
            |s, a| {
                let mut objs = objects(s, a);
                let keep = !a.bool("relative");
                if a.bool("chain") {
                    if objs.len() < 2 {
                        fail!("parent: chain needs at least two objects");
                    }
                    let ids = ids_of(&s.graph, &objs)?;
                    for w in ids.windows(2) {
                        s.graph.set_parent(w[0], w[1], keep, usize::MAX)?;
                    }
                    return Ok(json!(objs));
                }
                let mut p = NO_NODE;
                if !a.bool("world") {
                    if objs.len() < 2 {
                        fail!("parent: need children and a parent");
                    }
                    p = node_named(&s.graph, objs.last().unwrap())?;
                    objs.pop();
                }
                let ids = ids_of(&s.graph, &objs)?;
                let g = &mut s.graph;
                if !a.has("index") {
                    for &c in &ids {
                        g.set_parent(c, p, keep, usize::MAX)?;
                    }
                    g.select(&ids, SelectMode::Replace);
                    return Ok(json!(objs));
                }
                let index = a.int("index", 0);
                if index < 0 {
                    fail!("parent: index must be 0 or more");
                }
                let sib_of = |g: &Graph| if p == NO_NODE { g.roots().to_vec() } else { g.children(p).to_vec() };
                let others: Vec<NodeId> = sib_of(g).into_iter().filter(|x| !ids.contains(x)).collect();
                // Each object goes in front of the sibling it should precede, so earlier moves don't shift later ones.
                let mut before = others.get(index as usize).copied().unwrap_or(NO_NODE);
                for &id in ids.iter().rev() {
                    let sib = sib_of(g);
                    let mut at = sib.iter().position(|&x| x == before).unwrap_or(sib.len());
                    if let Some(me) = sib.iter().position(|&x| x == id) {
                        if me < at {
                            at -= 1;
                        }
                    }
                    g.set_parent(id, p, keep, at)?;
                    before = id;
                }
                g.select(&ids, SelectMode::Replace);
                Ok(json!(objs))
            },
        )
        .positional("objects... parent")
        .flag(flag("world", "w", T::Bool, json!(false), "Unparent to the world."))
        .flag(flag("relative", "r", T::Bool, json!(false), "Keep local transforms instead of world."))
        .flag(flag("chain", "ch", T::Bool, json!(false), "Parent in selection order, each under the next."))
        .flag(flag("index", "i", T::Int, Value::Null, "Position among the parent's children, counted without the moved objects.")),
    );

    c.register(
        Cmd::new("newScene", "Clear the scene. Not undoable.", |s, _| {
            aztek_ops::units::reset();
            s.graph.clear();
            s.extras = Default::default();
            crate::anim::set_frame(&mut s.graph, crate::anim::range().min);
            s.mark_saved();
            Ok(Value::Null)
        })
        .undoable(false),
    );

}

fn register_attrs(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "setAttr",
            "setAttr('node.attr', value). Vec3 takes a list or three numbers. lock=True/False locks or unlocks the plug, with or \
             without a value: a locked plug can't be changed until it is unlocked.",
            |s, a| {
                let lock_only = a.has("lock") && a.positional.len() == 1;
                let keyable = a.get("keyable").or(a.get("channelBox")).and_then(Value::as_bool);
                if (a.positional.len() < 2 && !lock_only && keyable.is_none()) || a.pos_str(0).is_none() {
                    fail!("setAttr: expected (plug, value)");
                }
                let g = &mut s.graph;
                let p = plug_arg(g, &a.positional[0])?;
                if let Some(on) = keyable {
                    crate::anim::set_keyable(g, &p, on)?;
                    if a.positional.len() < 2 && !lock_only {
                        return Ok(Value::Null);
                    }
                }
                // Unlock before setting and lock after, so setAttr(plug, v, lock=...) works either way.
                if a.has("lock") && !a.bool("lock") {
                    g.set_locked(&p, false)?;
                }
                if !lock_only && g.attr(&p).is_array() && p.element < 0 {
                    let Some(list) = a.positional[1].as_array() else {
                        fail!("setAttr: {} is an array; give a list of its values", g.plug_name(&p))
                    };
                    let def = g.attr(&p).clone();
                    let values = list.iter().map(|v| from_json(&def, v)).collect::<Result<Vec<_>, _>>()?;
                    g.set_elements(&p, &values)?;
                } else if !lock_only {
                    let v = if a.positional.len() == 4 { json!(a.positional[1..4]) } else { a.positional[1].clone() };
                    let mut def = g.attr(&p).clone();
                    if p.component >= 0 {
                        def.ty = PlugType::Float;
                    }
                    g.set_value(p, from_json(&def, &v)?)?;
                }
                if a.has("lock") && a.bool("lock") {
                    g.set_locked(&p, true)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("plug [value]")
        .flag(flag("lock", "l", T::Bool, Value::Null, "Lock (True) or unlock (False) the plug."))
        .flag(flag("keyable", "k", T::Bool, Value::Null, "Make the channel keyable, or not: it shows in the channel box and S keys it."))
        .flag(flag("channelBox", "cb", T::Bool, Value::Null, "As -k: the channel box shows keyable channels.")),
    );

    c.register(
        Cmd::new(
            "getAttr",
            "Value of 'node.attr'; a whole array gives its elements' values in index order. lock=True returns whether it is \
             locked instead; multiIndices=True an array's element indices.",
            |s, a| {
                let Some(first) = a.positional.first().filter(|v| v.is_string()) else { fail!("getAttr: expected a plug") };
                let g = &mut s.graph;
                let p = plug_arg(g, first)?;
                if a.bool("lock") {
                    return Ok(json!(g.is_locked(&p)));
                }
                if a.bool("keyable") {
                    return Ok(json!(crate::anim::is_keyable(g, &p)));
                }
                if a.bool("multiIndices") {
                    return Ok(json!(g.elements(&p).iter().map(|e| e.element).collect::<Vec<_>>()));
                }
                if g.attr(&p).is_array() && p.element < 0 {
                    let mut out = Vec::new();
                    for e in g.elements(&p) {
                        out.push(to_json(&g.value(&e)?));
                    }
                    return Ok(Value::Array(out));
                }
                Ok(to_json(&g.value(&p)?))
            },
        )
        .positional("plug")
        .flag(flag("lock", "l", T::Bool, json!(false), "Return the lock state instead of the value."))
        .flag(flag("keyable", "k", T::Bool, json!(false), "Return whether the channel is keyable instead of the value."))
        .flag(flag("multiIndices", "mi", T::Bool, json!(false), "Return an array's element indices, ascending."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "connectAttr",
            "connectAttr('src.attr', 'dst.attr'). A whole array input ('loft1.sections') gets a new last element. Returns the \
             destination plug.",
            |s, a| {
                if a.positional.len() != 2 {
                    fail!("connectAttr: expected (src, dst)");
                }
                let (src, dst) = {
                    let g = &mut s.graph;
                    (plug_arg(g, &a.positional[0])?, plug_arg(g, &a.positional[1])?)
                };
                // Numbers mean what people read them as: a translate of 1 driving a rotate turns it 1 degree.
                let factor = aztek_ops::units::factor(s.graph.attr(&dst).unit) / aztek_ops::units::factor(s.graph.attr(&src).unit);
                if aztek_ops::units::user_call() && (factor - 1.0).abs() > 1e-12 && !s.graph.attr(&dst).is_array() {
                    return connect_converted(s, src, dst, factor, a.bool("force"));
                }
                let g = &mut s.graph;
                let d = g.connect(&src, &dst, a.bool("force"))?;
                Ok(json!(g.plug_name(&d)))
            },
        )
        .positional("src dst")
        .flag(flag("force", "f", T::Bool, json!(false), "Replace an existing input connection.")),
    );

    c.register(
        Cmd::new(
            "disconnectAttr",
            "disconnectAttr('dst.attr'), or (src, dst). A whole array input loses every element, or with src only the ones src \
             drives.",
            |s, a| {
                let Some(last) = a.positional.last() else { fail!("disconnectAttr: expected a plug") };
                let d = plug_arg(&mut s.graph, last)?;
                if !s.graph.attr(&d).is_array() || d.element >= 0 {
                    let parts: Vec<PlugRef> = if s.graph.attr(&d).ty == PlugType::Vec3 && d.component < 0 && s.graph.source(&d).is_none() {
                        (0..3).map(|k| PlugRef::component(d.node, d.attr, k)).collect()
                    } else {
                        vec![d.clone()]
                    };
                    for p in parts {
                        let conversion = s.graph.source(&p).filter(|sp| s.graph.node(sp.node).is_some_and(|n| n.ty().name == "unitConversion"));
                        s.graph.disconnect(&p);
                        // A conversion node goes when what it joined is apart.
                        if let Some(c) = conversion {
                            let name = s.graph.path(c.node);
                            s.run("delete", json!({"args": [name]}))?;
                        }
                    }
                    return Ok(Value::Null);
                }
                let g = &mut s.graph;
                let from = if a.positional.len() > 1 { Some(plug_arg(g, &a.positional[0])?) } else { None };
                for e in g.elements(&d) {
                    if from.is_none() || g.source(&e) == from {
                        g.disconnect(&e);
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("[src] dst"),
    );

    c.register(
        Cmd::new(
            "addAttr",
            "Add an attribute to objects (the reference's addAttr): attributeType for a single value or, with multi, an array of them; \
             dataType for strings and arrays (stringArray, doubleArray, Int32Array, vectorArray). attributeType='compound' \
             groups the attributes then added with parent=; a multi compound gives each element every child, \
             node.targets[2].weight. Compounds nest one level. Saved with the scene. Returns the new plugs.",
            add_attr,
        )
        .positional("objects")
        .flag(flag("longName", "ln", T::String, Value::Null, "The attribute's name."))
        .flag(flag("shortName", "sn", T::String, Value::Null, "A short name."))
        .flag(flag("niceName", "nn", T::String, Value::Null, "The label editors show."))
        .flag(flag(
            "attributeType",
            "at",
            T::String,
            Value::Null,
            "bool, long, float, double, double3, enum, matrix or compound (Aztek's int, vec3 too).",
        ))
        .flag(flag("parent", "p", T::String, Value::Null, "The compound this attribute is a child of."))
        .flag(flag("numberOfChildren", "nc", T::Int, Value::Null, "Accepted for existing scripts: a compound takes any number of children."))
        .flag(flag(
            "dataType",
            "dt",
            T::String,
            Value::Null,
            "string, stringArray, doubleArray, floatArray, Int32Array, vectorArray or matrix.",
        ))
        .flag(flag("multi", "m", T::Bool, json!(false), "An array of attributeType: any number of elements, attr[0], attr[1] ..."))
        .flag(flag("defaultValue", "dv", T::Any, Value::Null, "Default value (each element's, for an array)."))
        .flag(flag("minValue", "min", T::Float, Value::Null, "Lowest value it takes: lower values are clamped."))
        .flag(flag("maxValue", "max", T::Float, Value::Null, "Highest value it takes: higher values are clamped."))
        .flag(flag("softMinValue", "smn", T::Float, Value::Null, "Slider minimum."))
        .flag(flag("softMaxValue", "smx", T::Float, Value::Null, "Slider maximum."))
        .flag(flag("enumName", "en", T::String, Value::Null, "Enum labels, 'low:medium:high'."))
        .flag(flag("keyable", "k", T::Bool, json!(false), "Show it in the channel box, where it can be keyed."))
        .flag(flag("hidden", "h", T::Bool, json!(false), "Hide it from editors."))
        .flag(flag("usedAsColor", "uac", T::Bool, json!(false), "A double3 holding a colour: editors show a swatch."))
        .flag(flag("usedAsFilename", "uaf", T::Bool, json!(false), "A string holding a file path: editors offer a file browser."))
        .flag(flag("category", "", T::String, Value::Null, "A heading the Attribute Editor groups it under."))
        .flag(flag("doc", "", T::String, Value::Null, "What it is for: editors show it as a tooltip.")),
    );

    c.register(
        Cmd::new(
            "deleteAttr",
            "Delete attributes added with addAttr: deleteAttr('node.attr'), deleteAttr(objects, attribute=name), or a whole \
             component with deleteAttr(objects, category=name), which also takes the name out of the object's usdApiSchemas.",
            |s, a| {
                if a.has("category") {
                    let category = a.string("category", "");
                    for o in objects(s, a) {
                        let id = node_named(&s.graph, &o)?;
                        delete_category(&mut s.graph, id, &category)?;
                    }
                    return Ok(Value::Null);
                }
                let mut paths = Vec::new();
                if a.has("attribute") {
                    for o in objects(s, a) {
                        let id = node_named(&s.graph, &o)?;
                        paths.push(format!("{}.{}", s.graph.path(id), a.string("attribute", "")));
                    }
                } else {
                    paths = a.named();
                }
                if paths.is_empty() {
                    fail!("deleteAttr: expected 'node.attr'");
                }
                for path in paths {
                    // A compound goes with all its children.
                    if let Some(dot) = path.find('.') {
                        let id = node_named(&s.graph, &path[..dot])?;
                        let group = path[dot + 1..].to_string();
                        let n = s.graph.node(id).unwrap();
                        let children = n.ty().group_children(&group);
                        let pending = s.pending_compounds.remove(&(id, group.clone())).is_some();
                        if pending || (!children.is_empty() && n.ty().attr_index(&group).is_none()) {
                            for c in children.into_iter().rev() {
                                s.graph.delete_attr(&PlugRef::new(id, c as i32))?;
                            }
                            continue;
                        }
                    }
                    let p = s.graph.plug(&path)?;
                    s.graph.delete_attr(&p.whole())?;
                }
                Ok(Value::Null)
            },
        )
        .positional("plugs or objects")
        .flag(flag("attribute", "at", T::String, Value::Null, "The attribute to delete from each object."))
        .flag(flag("category", "", T::String, Value::Null, "Delete every added attribute under this Attribute Editor heading.")),
    );

    c.register(
        Cmd::new(
            "removeMultiInstance",
            "Remove array elements with their connections and values: 'node.attr[3]', or 'node.group[3]' for every child of a \
             grouped array.",
            |s, a| {
                for path in a.named() {
                    // node.group[3]: no attr is named group, its children are group.<child>.
                    if let (Some(dot), Some(open)) = (path.find('.'), path.rfind('[')) {
                        if open > dot && path.ends_with(']') {
                            let id = node_named(&s.graph, &path[..dot])?;
                            let group = &path[dot + 1..open];
                            let n = s.graph.node(id).unwrap();
                            let children = n.ty().group_children(group);
                            if !children.is_empty() && n.ty().attr_index(group).is_none() {
                                let Ok(index) = path[open + 1..path.len() - 1].parse::<i32>() else {
                                    fail!("removeMultiInstance: bad element index in '{path}'")
                                };
                                for c in children {
                                    s.graph.remove_element(&PlugRef::element(id, c as i32, index))?;
                                }
                                continue;
                            }
                        }
                    }
                    let p = s.graph.plug(&path)?;
                    s.graph.remove_element(&p)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("plugs"),
    );

    c.register(
        Cmd::new("listAttr", "Attribute names of a node.", |s, a| {
            let objs = objects(s, a);
            let Some(first) = objs.first() else {
                fail!("listAttr: no object")
            };
            let id = node_named(&s.graph, first)?;
            let n = s.graph.node(id).unwrap();
            let mut out = Vec::new();
            for (i, d) in n.ty().attrs.iter().enumerate() {
                if d.has(flags::HIDDEN)
                    || (a.bool("keyable") && !d.has(flags::KEYABLE))
                    || (a.bool("userDefined") && !d.has(flags::USER))
                {
                    continue;
                }
                if !a.bool("locked") {
                    out.push(json!(d.name));
                } else if d.ty != PlugType::Vec3 && n.lock_mask(i) != 0 {
                    out.push(json!(d.name));
                } else if d.ty == PlugType::Vec3 {
                    for c in 0..3 {
                        if n.lock_mask(i) & (1 << c) != 0 {
                            out.push(json!(d.child_names[c]));
                        }
                    }
                }
            }
            Ok(Value::Array(out))
        })
        .positional("object")
        .flag(flag(
            "keyable",
            "k",
            T::Bool,
            json!(false),
            "Only keyable attributes.",
        ))
        .flag(flag(
            "locked",
            "l",
            T::Bool,
            json!(false),
            "Only locked plugs; a Vec3 lists each locked component (translateX).",
        ))
        .flag(flag(
            "userDefined",
            "ud",
            T::Bool,
            json!(false),
            "Only attributes added with addAttr.",
        ))
        .undoable(false),
    );
}

fn add_attr(s: &mut Session, a: &Args) -> CmdResult {
    let objs = objects(s, a);
    if objs.is_empty() {
        fail!("addAttr: no object");
    }
    if !a.has("longName") {
        fail!("addAttr: give longName");
    }
    let name = a.string("longName", "");
    if a.string("attributeType", "") == "compound" {
        if a.has("parent") {
            fail!("addAttr: compounds nest one level; '{name}' can't have a parent");
        }
        if !Graph::valid_name(&name) {
            fail!("addAttr: invalid attribute name '{name}'");
        }
        let mut out = Vec::new();
        for o in objs {
            let id = node_named(&s.graph, &o)?;
            let n = s.graph.node(id).unwrap();
            if n.ty().attr_index(&name).is_some()
                || !n.ty().group_children(&name).is_empty()
                || s.pending_compounds.contains_key(&(id, name.clone()))
            {
                fail!("addAttr: '{}' already has an attribute '{name}'", n.name());
            }
            s.pending_compounds
                .insert((id, name.clone()), a.bool("multi"));
            out.push(json!(format!("{}.{name}", s.graph.path(id))));
        }
        return Ok(Value::Array(out));
    }
    let kinds: BTreeMap<&str, (PlugType, bool)> = [
        ("bool", (PlugType::Bool, false)),
        ("long", (PlugType::Int, false)),
        ("short", (PlugType::Int, false)),
        ("byte", (PlugType::Int, false)),
        ("int", (PlugType::Int, false)),
        ("float", (PlugType::Float, false)),
        ("double", (PlugType::Float, false)),
        ("doubleLinear", (PlugType::Float, false)),
        ("doubleAngle", (PlugType::Float, false)),
        ("double3", (PlugType::Vec3, false)),
        ("float3", (PlugType::Vec3, false)),
        ("vec3", (PlugType::Vec3, false)),
        ("enum", (PlugType::Enum, false)),
        ("matrix", (PlugType::Matrix, false)),
        ("string", (PlugType::String, false)),
        ("stringArray", (PlugType::String, true)),
        ("doubleArray", (PlugType::Float, true)),
        ("floatArray", (PlugType::Float, true)),
        ("Int32Array", (PlugType::Int, true)),
        ("intArray", (PlugType::Int, true)),
        ("vectorArray", (PlugType::Vec3, true)),
        ("pointArray", (PlugType::Vec3, true)),
    ]
    .into_iter()
    .collect();
    let kind = if a.has("dataType") {
        a.string("dataType", "")
    } else {
        a.string("attributeType", "float")
    };
    let Some(&(ty, array)) = kinds.get(kind.as_str()) else {
        fail!("addAttr: unknown type '{kind}'")
    };
    let mut def = AttrDef {
        unit: match kind.as_str() {
            "doubleAngle" => aztek_ops::units::Unit::Angle,
            "doubleLinear" => aztek_ops::units::Unit::Linear,
            _ => aztek_ops::units::Unit::None,
        },
        name: name.clone(),
        short_name: a.string("shortName", ""),
        nice_name: a.string("niceName", ""),
        doc: a.string("doc", ""),
        ty,
        default_value: default_for(ty),
        category: a.string("category", ""),
        ..Default::default()
    };
    if array || a.bool("multi") {
        def.flags |= flags::ARRAY;
    }
    if ty == PlugType::Enum {
        def.enum_names = a
            .string("enumName", "")
            .split(':')
            .filter(|n| !n.is_empty())
            .map(str::to_string)
            .collect();
        if def.enum_names.is_empty() {
            fail!("addAttr: an enum needs enumName, 'a:b:c'");
        }
    }
    if let Some(dv) = a.get("defaultValue") {
        def.default_value = from_json(&def, dv)?;
    }
    def.min_value = a.get("minValue").and_then(Value::as_f64);
    def.max_value = a.get("maxValue").and_then(Value::as_f64);
    def.soft_min = a.get("softMinValue").and_then(Value::as_f64);
    def.soft_max = a.get("softMaxValue").and_then(Value::as_f64);
    // A script's numbers are in the current unit of what the attr measures.
    let k = aztek_ops::units::factor(def.unit);
    if k != 1.0 {
        match &mut def.default_value {
            PlugValue::Float(x) => *x *= k,
            PlugValue::Vec3(x) => *x *= k,
            _ => {}
        }
        for b in [&mut def.min_value, &mut def.max_value, &mut def.soft_min, &mut def.soft_max] {
            if let Some(x) = b {
                *x *= k;
            }
        }
    }
    for (f, bit) in [
        ("keyable", flags::KEYABLE),
        ("hidden", flags::HIDDEN),
        ("usedAsColor", flags::COLOR),
        ("usedAsFilename", flags::FILE),
    ] {
        if a.bool(f) {
            def.flags |= bit;
        }
    }
    let group = a.string("parent", "");
    let mut out = Vec::new();
    for o in objs {
        let id = node_named(&s.graph, &o)?;
        let mut child = def.clone();
        if !group.is_empty() {
            let multi = compound_is_multi(s, id, &group)?;
            if multi && child.is_array() {
                fail!("addAttr: '{group}' is a multi compound: its children hold one value per element, not arrays");
            }
            child.name = format!("{group}.{}", def.name);
            if !def.short_name.is_empty() {
                child.short_name = format!("{group}.{}", def.short_name);
            }
            if multi {
                child.flags |= flags::ARRAY;
            }
        }
        let index = s.graph.add_attr(id, child)?;
        s.pending_compounds.remove(&(id, group.clone()));
        out.push(json!(s.graph.plug_name(&PlugRef::new(id, index as i32))));
    }
    Ok(Value::Array(out))
}

fn register_queries(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "ls",
            "List nodes, optionally filtered by type or the selection.",
            |s, a| {
                let g = &s.graph;
                // In a component mode the selection is the components (on the hilited objects).
                if a.bool("selection") && a.positional.is_empty() && a.get("type").is_none() {
                    let comps = crate::components::component_strings(g, a.bool("flatten"));
                    if !comps.is_empty() {
                        return Ok(json!(comps));
                    }
                }
                let ids = if a.bool("selection") {
                    g.selection().to_vec()
                } else {
                    g.nodes()
                };
                // A short name matching several nodes lists them all, like a pattern would.
                let names: Vec<&str> = a.positional.iter().filter_map(Value::as_str).collect();
                let filtered = !names.is_empty();
                let wanted: Vec<NodeId> = names.iter().flat_map(|n| g.find_all(n)).collect();
                let ty = a.get("type").and_then(Value::as_str);
                let out: Vec<Value> = ids
                    .into_iter()
                    .filter(|&id| {
                        let n = g.node(id).unwrap();
                        ty.is_none_or(|t| n.ty().is_a(t))
                            && (!a.bool("dag") || n.ty().dag)
                            && (!a.bool("referencedNodes") || (n.reference_id() != 0 && n.ty().name != crate::references::REF_NODE_TYPE))
                            && (!filtered || wanted.contains(&id))
                    })
                    .map(|id| json!(g.path(id)))
                    .collect();
                Ok(Value::Array(out))
            },
        )
        .positional("names")
        .flag(flag(
            "type",
            "typ",
            T::String,
            Value::Null,
            "Only nodes of this type (or derived).",
        ))
        .flag(flag("referencedNodes", "rn", T::Bool, json!(false), "Only nodes from references."))
        .flag(flag(
            "selection",
            "sl",
            T::Bool,
            json!(false),
            "Only selected nodes, in selection order.",
        ))
        .flag(flag(
            "dag",
            "",
            T::Bool,
            json!(false),
            "Only transforms in the hierarchy.",
        ))
        .flag(flag("flatten", "fl", T::Bool, json!(false), "Selected components one per item (pCube1.vtx[3]) rather than as ranges."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "select",
            "Select objects. Replaces the selection unless add/deselect/toggle.",
            |s, a| {
                if a.bool("clear") {
                    if !crate::components::component_strings(&s.graph, false).is_empty() {
                        let _ = crate::components::select_strings(&mut s.graph, &[], SelectMode::Replace);
                    }
                    s.graph.select(&[], SelectMode::Replace);
                    return Ok(Value::Null);
                }
                let mode = if a.bool("add") {
                    SelectMode::Add
                } else if a.bool("deselect") {
                    SelectMode::Remove
                } else if a.bool("toggle") {
                    SelectMode::Toggle
                } else {
                    SelectMode::Replace
                };
                let names = a.named();
                let (comps, objs): (Vec<String>, Vec<String>) = names.into_iter().partition(|n| crate::components::is_component_string(n));
                let mut ids = ids_of(&s.graph, &objs)?;
                if !comps.is_empty() {
                    let g = &mut s.graph;
                    let kind = crate::components::select_strings(g, &comps, mode).map_err(CommandError::new)?;
                    // The meshes the components are on are hilited, as selecting components does.
                    for m in crate::object_sets::parse(g, &comps).map_err(CommandError::new)? {
                        let xf = crate::dag::transform_of(g, m.node);
                        if !ids.contains(&xf) && mode != SelectMode::Remove {
                            ids.push(xf);
                        }
                    }
                    if let Some(k) = kind {
                        crate::components::set_mode_undoable(g, k);
                    }
                    let add_mode = if mode == SelectMode::Replace { SelectMode::Replace } else { SelectMode::Add };
                    if mode != SelectMode::Remove {
                        g.select(&ids, add_mode);
                    }
                    return Ok(Value::Null);
                }
                // Replacing with objects replaces any components too.
                if mode == SelectMode::Replace && !crate::components::component_strings(&s.graph, false).is_empty() {
                    let _ = crate::components::select_strings(&mut s.graph, &[], SelectMode::Replace);
                }
                s.graph.select(&ids, mode);
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("add", "add", T::Bool, json!(false), ""))
        .flag(flag("deselect", "d", T::Bool, json!(false), ""))
        .flag(flag("toggle", "tgl", T::Bool, json!(false), ""))
        .flag(flag(
            "clear",
            "cl",
            T::Bool,
            json!(false),
            "Select nothing.",
        )),
    );

    c.register(
        Cmd::new(
            "pickWalk",
            "Walk the selection through the hierarchy like the arrow keys: up to the parent, down to the first child, \
             left/right to the previous/next sibling (wrapping). Right after a delete, with nothing selected since, it walks \
             from where the deleted objects were.",
            |s, a| {
                let dir = a.string("direction", "down");
                if !["up", "down", "left", "right"].contains(&dir.as_str()) {
                    fail!("pickWalk: direction must be up, down, left or right");
                }
                let g = &mut s.graph;
                let mut next: Vec<NodeId> = Vec::new();
                if let Some(ghosts) = live_walk_ghosts(&s.walk_ghosts, g) {
                    for ghost in ghosts {
                        let to = walk_from_ghost(g, ghost, &dir);
                        if to != NO_NODE && !next.contains(&to) {
                            next.push(to);
                        }
                    }
                    if !next.is_empty() {
                        g.select(&next, SelectMode::Replace);
                    }
                    return Ok(Value::Null);
                }
                for &id in g.selection() {
                    let mut to = id;
                    match dir.as_str() {
                        "up" => {
                            if g.parent(id) != NO_NODE {
                                to = g.parent(id)
                            }
                        }
                        "down" => {
                            if let Some(&k) = walkable(g, g.children(id)).first() {
                                to = k;
                            }
                        }
                        _ => {
                            let parent = g.parent(id);
                            let sibs = walkable(g, if parent == NO_NODE { g.roots() } else { g.children(parent) });
                            if let Some(i) = sibs.iter().position(|&x| x == id) {
                                if sibs.len() > 1 {
                                    let n = sibs.len();
                                    to = sibs[if dir == "right" { (i + 1) % n } else { (i + n - 1) % n }];
                                }
                            }
                        }
                    }
                    if !next.contains(&to) {
                        next.push(to);
                    }
                }
                if next != g.selection() {
                    g.select(&next, SelectMode::Replace);
                }
                Ok(Value::Null)
            },
        )
        .flag(flag("direction", "", T::String, json!("down"), "up, down, left or right.")),
    );

    c.register(Cmd::new(
        "selectHierarchy",
        "Add every transform below each selected object to the selection, parents before children.",
        |s, _| {
            let g = &mut s.graph;
            fn visit(g: &Graph, id: NodeId, out: &mut Vec<NodeId>) {
                if !out.contains(&id) {
                    out.push(id);
                }
                for &k in g.children(id) {
                    if g.node(k).is_some_and(|n| n.ty().is_a("transform")) {
                        visit(g, k, out);
                    }
                }
            }
            let mut out = Vec::new();
            for &id in g.selection() {
                visit(g, id, &mut out);
            }
            if out != g.selection() {
                g.select(&out, SelectMode::Replace);
            }
            Ok(Value::Null)
        },
    ));

    c.register(
        Cmd::new(
            "objExists",
            "True if a node with this name or path exists.",
            |s, a| {
                Ok(json!(a
                    .pos_str(0)
                    .is_some_and(|n| !s.graph.find_all(n).is_empty())))
            },
        )
        .positional("name")
        .undoable(false),
    );

    c.register(
        Cmd::new("nodeType", "Type name of a node.", |s, a| {
            let objs = objects(s, a);
            let Some(first) = objs.first() else {
                fail!("nodeType: no object")
            };
            let id = node_named(&s.graph, first)?;
            Ok(json!(s.graph.node(id).unwrap().ty().name))
        })
        .positional("object")
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "listRelatives",
            "Children (default), parent or all descendants.",
            |s, a| {
                let objs = objects(s, a);
                let Some(first) = objs.first() else {
                    fail!("listRelatives: no object")
                };
                let g = &s.graph;
                let id = node_named(g, first)?;
                Ok(if a.bool("parent") {
                    names_of(g, &[g.parent(id)])
                } else if a.bool("allDescendents") {
                    names_of(g, &g.descendants(id))
                } else {
                    names_of(g, g.children(id))
                })
            },
        )
        .positional("object")
        .flag(flag("parent", "p", T::Bool, json!(false), ""))
        .flag(flag("allDescendents", "ad", T::Bool, json!(false), ""))
        .flag(flag("children", "c", T::Bool, json!(false), "The children: what it lists by default."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "xform",
            "Query or set the transform of objects, in local or world space.\n\nSets every flag given on each object, or with \
             query=True returns one value of the first object: pass the flag to read as True. Values are local to the parent \
             unless worldSpace is on.",
            xform,
        )
        .positional("Transforms to edit or query; the selection when none are given.")
        .flag(
            flag(
                "query",
                "q",
                T::Bool,
                json!(false),
                "Return a value of the first object instead of setting. Pick it by passing one of translation, rotation, scale \
                 or matrix as True.",
            )
            .example("xform(\"cube1\", query=True, translation=True)"),
        )
        .flag(
            flag(
                "worldSpace",
                "ws",
                T::Bool,
                json!(false),
                "Read and write in world space instead of relative to the parent. Rotation and scale queries stay local.",
            )
            .example("xform(\"cube1\", worldSpace=True, translation=(0, 2, 0))"),
        )
        .flag(flag("relative", "r", T::Bool, json!(false), "On components: -t moves by the offset."))
        .flag(flag("absolute", "a", T::Bool, json!(false), "The values are absolute (the default)."))
        .flag(flag("objectSpace", "os", T::Bool, json!(false), "Object space (the default)."))
        .flag(
            flag("translation", "t", T::Any, Value::Null, "Position as (x, y, z) to set, or True to read it with query.")
                .example("xform(\"cube1\", translation=(1, 0, 0))"),
        )
        .flag(
            flag("rotation", "ro", T::Any, Value::Null, "Euler rotation in degrees as (x, y, z) to set, or True to read it with query.")
                .example("xform(\"cube1\", rotation=(0, 45, 0))"),
        )
        .flag(flag("rotateAxis", "ra", T::Any, Value::Null, "rotateAxis as (x, y, z) to set, or True to read it with query."))
        .flag(
            flag("scale", "s", T::Any, Value::Null, "Scale as (x, y, z) to set, or True to read it with query.")
                .example("xform(\"cube1\", scale=(2, 2, 2))"),
        )
        .flag(
            flag(
                "matrix",
                "m",
                T::Any,
                Value::Null,
                "The whole transform as 16 floats, column-major, to set; or True to read it with query.",
            )
            .example("xform(\"cube1\", query=True, worldSpace=True, matrix=True)"),
        )
        .examples(&["xform(\"cube1\", translation=(0, 1, 0), rotation=(0, 90, 0))", "pos = xform(\"cube1\", query=True, worldSpace=True, translation=True)"]),
    );
}

fn xform(s: &mut Session, a: &Args) -> CmdResult {
    let named = objects(s, a);
    if named.iter().any(|n| crate::components::is_component_string(n)) {
        return crate::ref_cmds::xform_components(s, a, &named);
    }
    let objs = ids_of(&s.graph, &named)?;
    let g = &mut s.graph;
    let ws = a.bool("worldSpace");
    if a.bool("query") {
        let Some(&id) = objs.first() else {
            fail!("xform: no object")
        };
        g.evaluate();
        let n = g.node(id).unwrap();
        let m = n.matrix(if ws {
            Transform::WORLD_MATRIX
        } else {
            Transform::MATRIX
        });
        if a.has("matrix") {
            return Ok(to_json(&PlugValue::Matrix(m)));
        }
        if a.has("translation") {
            return Ok(if ws {
                vec3_json(m.w_axis.truncate())
            } else {
                to_json(n.value(Transform::TRANSLATE))
            });
        }
        if a.has("rotation") {
            return Ok(to_json(n.value(Transform::ROTATE)));
        }
        if a.has("scale") {
            return Ok(to_json(n.value(Transform::SCALE)));
        }
        if a.has("rotateAxis") {
            return Ok(to_json(n.value(Transform::ROTATE_AXIS)));
        }
        fail!("xform: query what?");
    }
    let need = |f: &str| {
        a.vec3(f)
            .map(vec3)
            .ok_or_else(|| CommandError::new(format!("xform: {f} needs (x, y, z)")))
    };
    for id in objs {
        if !g.node(id).unwrap().ty().dag {
            continue;
        }
        if let Some(mj) = a.get("matrix") {
            let mut m = from_json(
                &AttrDef {
                    ty: PlugType::Matrix,
                    ..Default::default()
                },
                mj,
            )?
            .as_matrix();
            if ws {
                g.evaluate();
                m = g
                    .node(id)
                    .unwrap()
                    .matrix(Transform::PARENT_MATRIX)
                    .inverse()
                    * m;
            }
            Transform::set_local_matrix(g, id, &m)?;
        }
        if a.has("rotateAxis") {
            set_vec(g, id, Transform::ROTATE_AXIS, need("rotateAxis")?)?;
        }
        if a.has("rotation") {
            set_vec(g, id, Transform::ROTATE, need("rotation")?)?;
        }
        if a.has("scale") {
            set_vec(g, id, Transform::SCALE, need("scale")?)?;
        }
        if a.has("translation") {
            let t = need("translation")?;
            if ws {
                g.evaluate();
                let n = g.node(id).unwrap();
                let mut world = n.matrix(Transform::WORLD_MATRIX);
                world.w_axis = DVec4::from((t, 1.0));
                let local = n.matrix(Transform::PARENT_MATRIX).inverse() * world;
                Transform::set_local_matrix(g, id, &local)?;
            } else {
                set_vec(g, id, Transform::TRANSLATE, t)?;
            }
        }
    }
    Ok(Value::Null)
}

/// Where an IK controller would pose its chain's joints. The edit runs as it is.
fn pose_controllers(
    s: &mut Session,
    _ids: &[NodeId],
    f: impl FnOnce(&mut Session) -> CmdResult<()>,
) -> CmdResult<()> {
    f(s)
}

/// Joints are not implemented yet.
fn is_joint(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("joint"))
}

fn register_transform(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "move",
            "Move objects. move(by=(x, y, z)) moves relative in `space`; move(to=(x, y, z)) puts each pivot at a world \
             position. A selected parent and child move once.\n\nmove(x=0) sets one world axis and leaves the others: each \
             selected object's pivot, or with components selected every selected point, so move(x=0) puts an edge loop on the \
             centre line.",
            |s, a| {
                if let Some(r) = crate::ref_cmds::on_components(s, a, "moveComponents")? {
                    return Ok(r);
                }
                if let Some(axes) = axis_args(a) {
                    if on_components(a) {
                        let (mut factor, mut pivot) = (DVec3::ONE, DVec3::ZERO);
                        for i in 0..3 {
                            if !axes[i].is_nan() {
                                factor[i] = 0.0;
                                pivot[i] = axes[i];
                            }
                        }
                        run(s, "scaleComponents", json!({"by": vec3_json(factor), "pivot": vec3_json(pivot)}))?;
                        return Ok(Value::Null);
                    }
                    let ids = transforms_of(s, a, "move")?;
                    pose_controllers(s, &ids.clone(), |s| {
                        for id in xf::top_level(&s.graph, &ids) {
                            let pivot = xf::world_pivot(&mut s.graph, id);
                            xf::move_by(&mut s.graph, &[id], with_axes(pivot, axes) - pivot, Space::World)?;
                        }
                        Ok(())
                    })?;
                    return Ok(Value::Null);
                }
                let ids = transforms_of(s, a, "move")?;
                if !a.has("to") && !a.has("by") {
                    fail!("move: give by=, to= or x=, y=, z=");
                }
                // An IK controller poses its chain's joints.
                pose_controllers(s, &ids.clone(), |s| {
                    if let Some(to) = a.vec3("to") {
                        for id in xf::top_level(&s.graph, &ids) {
                            let delta = vec3(to) - xf::world_pivot(&mut s.graph, id);
                            xf::move_by(&mut s.graph, &[id], delta, Space::World)?;
                        }
                    } else {
                        xf::move_by(&mut s.graph, &ids, vec3(a.vec3("by").unwrap()), space_arg(a)?)?;
                    }
                    Ok(())
                })?;
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("by", "", T::Vec3, Value::Null, "Relative offset, cm."))
        .flag(flag("to", "", T::Vec3, Value::Null, "Absolute world position of the pivot, cm."))
        .flag(flag("x", "", T::Float, Value::Null, "World X for the pivot, or for every selected point, cm."))
        .flag(flag("y", "", T::Float, Value::Null, "World Y, cm."))
        .flag(flag("z", "", T::Float, Value::Null, "World Z, cm."))
        .flag(flag("space", "", T::String, json!("world"), "'world', 'object' or 'parent' for `by`.")),
    );

    c.register(
        Cmd::new(
            "rotate",
            "Rotate objects about their pivots. rotate(by=(x, y, z)) adds a rotation (degrees, XYZ order) in `space`; \
             rotate(to=(x, y, z)) sets the local Euler angles.\n\nrotate(x=90) sets one local angle and leaves the others. With \
             components selected it turns them that far about the world axis through their centre instead.",
            |s, a| {
                if let Some(r) = crate::ref_cmds::on_components(s, a, "rotateComponents")? {
                    return Ok(r);
                }
                if let Some(axes) = axis_args(a) {
                    if on_components(a) {
                        run(s, "rotateComponents", json!({"by": vec3_json(with_axes(DVec3::ZERO, axes))}))?;
                        return Ok(Value::Null);
                    }
                    let ids = transforms_of(s, a, "rotate")?;
                    pose_controllers(s, &ids.clone(), |s| {
                        for &id in &ids {
                            let cur = s.graph.node(id).unwrap().vec3(Transform::ROTATE);
                            set_vec_unlocked(&mut s.graph, id, Transform::ROTATE, with_axes(cur, axes))?;
                        }
                        Ok(())
                    })?;
                    return Ok(Value::Null);
                }
                let ids = transforms_of(s, a, "rotate")?;
                if !a.has("to") && !a.has("by") {
                    fail!("rotate: give by=, to= or x=, y=, z=");
                }
                pose_controllers(s, &ids.clone(), |s| {
                    if let Some(to) = a.vec3("to") {
                        for &id in &ids {
                            set_vec_unlocked(&mut s.graph, id, Transform::ROTATE, vec3(to))?;
                        }
                    } else {
                        xf::rotate_by(&mut s.graph, &ids, &euler_rot(vec3(a.vec3("by").unwrap())), space_arg(a)?)?;
                    }
                    Ok(())
                })?;
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("by", "", T::Vec3, Value::Null, "Relative rotation, degrees."))
        .flag(flag("to", "", T::Vec3, Value::Null, "Absolute local rotation, degrees."))
        .flag(flag("x", "", T::Float, Value::Null, "Local X rotation, degrees; on components, a turn about world X."))
        .flag(flag("y", "", T::Float, Value::Null, "Local Y rotation, degrees."))
        .flag(flag("z", "", T::Float, Value::Null, "Local Z rotation, degrees."))
        .flag(flag("space", "", T::String, json!("world"), "'world', 'object' or 'parent' for `by`.")),
    );

    c.register(
        Cmd::new(
            "scale",
            "Scale objects along their own axes. scale(by=(x, y, z)) multiplies; scale(to=(x, y, z)) sets.\n\nscale(x=2) sets \
             one local scale and leaves the others. With components selected it scales them by that factor along the world axis \
             about their centre instead, so scale(y=0) flattens them.",
            |s, a| {
                if let Some(r) = crate::ref_cmds::on_components(s, a, "scaleComponents")? {
                    return Ok(r);
                }
                if let Some(axes) = axis_args(a) {
                    if on_components(a) {
                        run(s, "scaleComponents", json!({"by": vec3_json(with_axes(DVec3::ONE, axes))}))?;
                        return Ok(Value::Null);
                    }
                    for id in transforms_of(s, a, "scale")? {
                        let cur = s.graph.node(id).unwrap().vec3(Transform::SCALE);
                        set_vec_unlocked(&mut s.graph, id, Transform::SCALE, with_axes(cur, axes))?;
                    }
                    return Ok(Value::Null);
                }
                let ids = transforms_of(s, a, "scale")?;
                if let Some(to) = a.vec3("to") {
                    for id in ids {
                        set_vec_unlocked(&mut s.graph, id, Transform::SCALE, vec3(to))?;
                    }
                } else if let Some(by) = a.vec3("by") {
                    xf::scale_by(&mut s.graph, &ids, vec3(by))?;
                } else {
                    fail!("scale: give by=, to= or x=, y=, z=");
                }
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("by", "", T::Vec3, Value::Null, "Factor per axis."))
        .flag(flag("to", "", T::Vec3, Value::Null, "Absolute scale."))
        .flag(flag("x", "", T::Float, Value::Null, "Local X scale; on components, a factor along world X."))
        .flag(flag("y", "", T::Float, Value::Null, "Local Y scale."))
        .flag(flag("z", "", T::Float, Value::Null, "Local Z scale.")),
    );

    c.register(
        Cmd::new(
            "setPivot",
            "Move the rotate and scale pivots to a world position, or orient the pivot, without moving the objects. A joint's \
             pivot is the joint itself: position= moves the joint and orient= sets its jointOrient, leaving its children where \
             they are.",
            |s, a| {
                if !a.has("position") && !a.has("orient") {
                    fail!("setPivot: give position= or orient=");
                }
                for id in transforms_of(s, a, "setPivot")? {
                    if is_joint(&s.graph, id) {
                        fail!("setPivot: joints are not supported yet");
                    }
                    if let Some(p) = a.vec3("position") {
                        xf::set_world_pivot(&mut s.graph, id, vec3(p))?;
                    }
                    if let Some(o) = a.vec3("orient") {
                        set_vec(&mut s.graph, id, Transform::PIVOT_ORIENT, vec3(o))?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("position", "", T::Vec3, Value::Null, "World position, cm."))
        .flag(flag(
            "orient",
            "",
            T::Vec3,
            Value::Null,
            "Pivot orientation relative to the object's axes, Euler XYZ degrees (pivotOrient); on a joint, its jointOrient.",
        )),
    );

    c.register(
        Cmd::new(
            "resetPivot",
            "Put the pivots back at the object's origin, keeping the object where it is.",
            |s, a| {
                for id in transforms_of(s, a, "resetPivot")? {
                    xf::reset_pivot(&mut s.graph, id)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("objects"),
    );

    c.register(
        Cmd::new(
            "zeroPivotRotation",
            "Line the pivots' axes up with the world (or with the object's own axes), leaving their positions and the objects \
             alone.",
            |s, a| {
                let world = a.string("space", "world") == "world";
                for id in transforms_of(s, a, "zeroPivotRotation")? {
                    if world {
                        xf::set_world_pivot_frame(&mut s.graph, id, &DMat3::IDENTITY)?;
                    } else {
                        set_vec(&mut s.graph, id, Transform::PIVOT_ORIENT, DVec3::ZERO)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("space", "", T::String, json!("world"), "Axes to line up with.").choices(&["world", "object"]))
        .adjustable(true),
    );

    c.register(
        Cmd::new(
            "zeroPivot",
            "Move the pivots to the world origin, keeping the object where it is.",
            |s, a| {
                for id in transforms_of(s, a, "zeroPivot")? {
                    xf::set_world_pivot(&mut s.graph, id, DVec3::ZERO)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("objects"),
    );

    c.register(
        Cmd::new(
            "matchPivot",
            "Copy the first object's pivot onto the others, keeping them where they are. position=False keeps their pivot \
             positions and orient=False their pivot orientations.",
            |s, a| {
                let (position, orient) = (a.bool("position"), a.bool("orient"));
                if !position && !orient {
                    fail!("matchPivot: position and orient are both off");
                }
                let ids = transforms_of(s, a, "matchPivot")?;
                if ids.len() < 2 {
                    fail!("matchPivot: select the object to copy from, then the ones to change");
                }
                let g = &mut s.graph;
                g.evaluate();
                let at = xf::world_pivot(g, ids[0]);
                let axes = xf::pivot_frame(g, ids[0]);
                for &id in &ids[1..] {
                    if orient {
                        xf::set_world_pivot_frame(g, id, &axes)?;
                    }
                    if position {
                        xf::set_world_pivot(g, id, at)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects (the first is the source)")
        .flag(flag("position", "p", T::Bool, json!(true), "Copy the pivot's world position."))
        .flag(flag("orient", "o", T::Bool, json!(true), "Copy the pivot's world axes."))
        .adjustable(true),
    );

    c.register(
        Cmd::new(
            "pivot",
            "World position of an object's rotate pivot.",
            |s, a| {
                let ids = transforms_of(s, a, "pivot")?;
                Ok(vec3_json(xf::world_pivot(&mut s.graph, ids[0])))
            },
        )
        .positional("object")
        .undoable(false),
    );
}
