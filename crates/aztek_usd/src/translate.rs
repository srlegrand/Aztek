//! Between prims and nodes. Edit As Scene Data pulls a prim into the scene as nodes and hides its USD copy; Merge to USD
//! pushes them back to the edit target. Duplicate copies either way and keeps no link. MayaReference prims bring a scene
//! file in as a file reference and can bake to a USD cache. A transform can also drive a prim live.

use std::cell::RefCell;
use std::collections::HashMap;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::graph::Graph;
use aztek_scene::node::types;
use aztek_scene::plug::{NodeId, PlugValue, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::transform::{DagNode, Transform};
use glam::DMat4;
use openusd::sdf::{self, ListOp, SpecType, Specifier, Value};
use openusd::tf::Token;
use openusd::usd::Prim;
use serde_json::{json, Map, Value as J};

use crate::import::{self, ImportOpts};
use crate::prims::{self, err, name_of, resolve, split_name, Target};
use crate::stages;
use crate::util::*;

const PULL_GROUP: &str = "__mayaUsd__";
const ATTR_PRIM: &str = "USD_pulledPrimPath";
const ATTR_PROXY: &str = "USD_pulledProxy";

fn pull_group(s: &mut Session) -> Result<NodeId, String> {
    if let Some(id) = s.graph.find(PULL_GROUP).map_err(|e| e.to_string())? {
        return Ok(id);
    }
    let t = types::find("transform").ok_or("no transform type")?;
    s.graph.create_node(&t, PULL_GROUP, NO_NODE, false).map_err(|e| e.to_string())
}

/// Reads a prim of an open stage into nodes under `parent`. Returns its top transform.
fn read_into_scene(s: &mut Session, t: &Target, parent: NodeId) -> Result<NodeId, String> {
    let prim = t.entry.stage.prim(t.path.clone()).map_err(|e| e.to_string())?;
    if !prim.is_valid().unwrap_or(false) {
        return Err(format!("no prim {}", t.path));
    }
    let o = ImportOpts {
        file: t.entry.file.clone(),
        prim_path: t.path.as_str().to_string(),
        read_anim: true,
        shading: true,
        import_instances: false,
        metadata: vec!["kind".into()],
        ..Default::default()
    };
    import::read_stage_prim(s, &o, &t.entry.stage, &prim, parent)
}

fn add_exclude(s: &mut Session, shape: NodeId, path: &str) {
    let now = get_str(&mut s.graph, shape, "excludePrimPaths").unwrap_or_default();
    let mut list: Vec<&str> = now.split(',').map(str::trim).filter(|x| !x.is_empty()).collect();
    if !list.contains(&path) {
        list.push(path);
    }
    let v = list.join(",");
    set_attr(&mut s.graph, shape, "excludePrimPaths", PlugValue::String(v));
}

fn remove_exclude(s: &mut Session, shape: NodeId, path: &str) {
    let now = get_str(&mut s.graph, shape, "excludePrimPaths").unwrap_or_default();
    let v = now.split(',').map(str::trim).filter(|x| !x.is_empty() && *x != path).collect::<Vec<_>>().join(",");
    set_attr(&mut s.graph, shape, "excludePrimPaths", PlugValue::String(v));
}

/// Pulls a prim: nodes in the scene, its USD copy hidden, a link kept on the nodes.
pub fn pull(s: &mut Session, name: &str) -> Result<NodeId, String> {
    let t = resolve(&mut s.graph, name)?;
    let time = stages::time_of(&mut s.graph, t.shape);
    let parent_world = prims::parent_world(&mut s.graph, &t, time);
    let group = pull_group(s)?;
    // Under a holder standing where the prim's parent stands, so the node keeps the prim's own values.
    let holder_type = types::find("transform").ok_or("no transform type")?;
    let holder_name = format!("{}_pull", node_name(t.path.name().unwrap_or("prim")));
    let holder = s.graph.create_node(&holder_type, &holder_name, group, true).map_err(|e| e.to_string())?;
    Transform::set_local_matrix(&mut s.graph, holder, &parent_world).map_err(|e| e.to_string())?;
    let top = read_into_scene(s, &t, holder)?;
    let proxy_name = s.graph.full_path(t.shape);
    import::add_user_attr(&mut s.graph, top, ATTR_PRIM, PlugValue::String(t.path.as_str().to_string()));
    import::add_user_attr(&mut s.graph, top, ATTR_PROXY, PlugValue::String(proxy_name));
    add_exclude(s, t.shape, t.path.as_str());
    let before = prims::selected();
    let _ = before;
    s.run("select", json!({"args": [s.graph.path(top)]})).map_err(|e| e.to_string())?;
    Ok(top)
}

fn pulled_link(s: &mut Session, name: &str) -> Result<(NodeId, NodeId, String), String> {
    let id = s.graph.find(name).map_err(|e| e.to_string())?.ok_or_else(|| format!("no node named '{name}'"))?;
    let top = if plug_of(&s.graph, id, ATTR_PRIM).is_some() { id } else { s.graph.parent(id) };
    let path = get_str(&mut s.graph, top, ATTR_PRIM).filter(|x| !x.is_empty()).ok_or_else(|| format!("{name} wasn't pulled from USD"))?;
    let proxy = get_str(&mut s.graph, top, ATTR_PROXY).unwrap_or_default();
    let shape = stages::proxy_named(&s.graph, &proxy).ok_or_else(|| format!("no proxy shape named '{proxy}'"))?;
    Ok((top, shape, path))
}

fn export_to_temp(s: &mut Session, node: NodeId, extra: &Map<String, J>) -> Result<(String, String), String> {
    let dir = std::env::temp_dir().join(format!("aztek_usd_{}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let file = dir.join(format!("push{node}_{n}.usdc")).to_string_lossy().into_owned();
    let mut call = Map::new();
    call.insert("file".into(), json!(file));
    call.insert("exportRoots".into(), json!([s.graph.full_path(node)]));
    call.insert("exportMaterials".into(), json!(false));
    call.insert("exportDisplayColor".into(), json!(false));
    for (k, v) in extra {
        call.insert(k.clone(), v.clone());
    }
    s.run("mayaUSDExport", J::Object(call)).map_err(|e| e.to_string())?;
    let top = prim_name(s.graph.node(node).map_or("", |n| n.name()), false);
    Ok((file, format!("/{top}")))
}

fn remap_path(p: &sdf::Path, from: &sdf::Path, to: &sdf::Path) -> sdf::Path {
    p.replace_prefix(from, to).unwrap_or_else(|| p.clone())
}

fn remap_value(v: &Value, from: &sdf::Path, to: &sdf::Path) -> Value {
    match v {
        Value::PathListOp(l) => {
            let f = |x: &Vec<sdf::Path>| -> Vec<sdf::Path> { x.iter().map(|p| remap_path(p, from, to)).collect() };
            let mut o = ListOp::default();
            o.explicit = l.explicit;
            o.explicit_items = f(&l.explicit_items);
            o.prepended_items = f(&l.prepended_items);
            o.appended_items = f(&l.appended_items);
            o.added_items = f(&l.added_items);
            o.deleted_items = f(&l.deleted_items);
            o.ordered_items = f(&l.ordered_items);
            Value::PathListOp(o)
        }
        Value::PathVec(l) => Value::PathVec(l.iter().map(|p| remap_path(p, from, to)).collect()),
        other => other.clone(),
    }
}

/// Writes a file's prim onto the stage's edit target at `to`, in place of what that layer held there.
fn write_specs(t: &Target, file: &str, from: &str, to: &sdf::Path) -> Result<(), String> {
    let layer = sdf::Layer::open(file).map_err(|e| format!("{file}: {e}"))?;
    let data = sdf::Data::from_abstract(layer.data()).map_err(|e| e.to_string())?;
    let from = sdf::Path::new(from).map_err(|e| e.to_string())?;
    let target = t.entry.stage.edit_target().layer_identifier().to_string();
    t.entry
        .stage
        .batch_edit(&[target.as_str()], |edits| {
            let d = edits[0].data_mut();
            for p in d.spec_paths() {
                if p.has_prefix(to) {
                    d.erase_spec(&p);
                }
            }
            // The way down: `over`s where the layer has nothing, each named by its parent.
            let mut parent = sdf::Path::abs_root();
            let mut chain: Vec<sdf::Path> = to.ancestors_below_root().collect();
            chain.reverse();
            if chain.first() != Some(to) && chain.last() != Some(to) {
                chain.push(to.clone());
            }
            for a in chain {
                let name = a.name().unwrap_or("").to_string();
                if d.spec_type(&parent).is_none() {
                    d.create_spec(parent.clone(), SpecType::PseudoRoot);
                }
                let key: &str = sdf::ChildrenKey::PrimChildren.into();
                let mut kids: Vec<Token> = match d.try_field(&parent, key).ok().flatten().map(|c| c.into_owned()) {
                    Some(Value::TokenVec(k)) => k,
                    _ => Vec::new(),
                };
                if !kids.iter().any(|k| k.as_str() == name) {
                    kids.push(name.as_str().into());
                    d.set_field(&parent, key, Value::TokenVec(kids));
                }
                if a != *to && d.spec_type(&a).is_none() {
                    d.create_spec(a.clone(), SpecType::Prim);
                    let sp: &str = sdf::FieldKey::Specifier.into();
                    d.set_field(&a, sp, Value::Specifier(Specifier::Over));
                }
                parent = a;
            }
            for (path, spec) in data.iter() {
                if !path.has_prefix(&from) {
                    continue;
                }
                let new = remap_path(path, &from, to);
                d.create_spec(new.clone(), spec.ty);
                for (k, v) in &spec.fields {
                    d.set_field(&new, k, remap_value(v, &from, to));
                }
            }
            Ok(())
        })
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Merge to USD: the nodes' current state onto the edit target, then the nodes go and the prim shows again.
pub fn merge(s: &mut Session, name: &str, extra: &Map<String, J>) -> Result<String, String> {
    let (top, shape, path) = pulled_link(s, name)?;
    let (file, from) = export_to_temp(s, top, extra)?;
    let prim_full = name_of(&s.graph, shape, &path);
    let t = resolve(&mut s.graph, &prim_full)?;
    t.entry.check_target()?;
    let e = t.entry.clone();
    let to = t.path.clone();
    e.clone().edit(s, "merge", |_| write_specs(&t, &file, &from, &to))?;
    let _ = std::fs::remove_file(&file);
    discard_nodes(s, top)?;
    remove_exclude(s, shape, &path);
    Ok(prim_full)
}

fn discard_nodes(s: &mut Session, top: NodeId) -> Result<(), String> {
    let holder = s.graph.parent(top);
    let group = if holder == NO_NODE { NO_NODE } else { s.graph.parent(holder) };
    let doomed = if holder != NO_NODE && s.graph.node(holder).is_some_and(|n| n.name().ends_with("_pull")) { holder } else { top };
    let name = s.graph.full_path(doomed);
    s.run("delete", json!({"args": [name]})).map_err(|e| e.to_string())?;
    if group != NO_NODE && s.graph.children(group).is_empty() {
        let n = s.graph.full_path(group);
        s.run("delete", json!({"args": [n]})).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Throws the nodes away: the prim shows as it was.
pub fn discard(s: &mut Session, name: &str) -> Result<String, String> {
    let (top, shape, path) = pulled_link(s, name)?;
    discard_nodes(s, top)?;
    remove_exclude(s, shape, &path);
    Ok(name_of(&s.graph, shape, &path))
}

/// Duplicate: prim to nodes under a node, or nodes to a prim under a prim. Nothing stays linked.
pub fn duplicate(s: &mut Session, src: &str, dst: &str) -> Result<String, String> {
    if split_name(src).is_some() {
        let t = resolve(&mut s.graph, src)?;
        let parent = if dst.is_empty() || dst == "|world" {
            NO_NODE
        } else {
            s.graph.find(dst).map_err(|e| e.to_string())?.ok_or_else(|| format!("no node named '{dst}'"))?
        };
        let top = read_into_scene(s, &t, parent)?;
        // It stands where the prim stands in the world, under whatever it was given.
        let time = stages::time_of(&mut s.graph, t.shape);
        let world = prims::world_of(&mut s.graph, &t, time);
        let up = if parent == NO_NODE { DMat4::IDENTITY } else { s.graph.node(parent).map_or(DMat4::IDENTITY, |n| n.matrix(DagNode::WORLD_MATRIX)) };
        Transform::set_local_matrix(&mut s.graph, top, &(up.inverse() * world)).map_err(|e| e.to_string())?;
        return Ok(s.graph.path(top));
    }
    let node = s.graph.find(src).map_err(|e| e.to_string())?.ok_or_else(|| format!("no node named '{src}'"))?;
    let (shape_name, parent_path) = split_name(dst).ok_or_else(|| format!("{dst} isn't a prim"))?;
    let shape = stages::proxy_named(&s.graph, shape_name).ok_or_else(|| format!("no proxy shape named '{shape_name}'"))?;
    let leaf = prim_name(s.graph.node(node).map_or("", |n| n.name()), false);
    let path = if parent_path == "/" { format!("/{leaf}") } else { format!("{parent_path}/{leaf}") };
    let full = name_of(&s.graph, shape, &path);
    let (file, from) = export_to_temp(s, node, &Map::new())?;
    let t = resolve(&mut s.graph, &full)?;
    t.entry.check_target()?;
    let e = t.entry.clone();
    let to = t.path.clone();
    e.clone().edit(s, "duplicate", |_| write_specs(&t, &file, &from, &to))?;
    let _ = std::fs::remove_file(&file);
    Ok(full)
}

fn comma_list(v: Option<&J>) -> Vec<String> {
    let text = v.and_then(J::as_str).unwrap_or("");
    text.split(',').map(str::trim).filter(|y| !y.is_empty()).map(str::to_string).collect()
}

fn first_str(a: &Args) -> Result<String, aztek_ops::CommandError> {
    if let Some(n) = a.pos_str(0) {
        return Ok(n.to_string());
    }
    Err(err("name the prim or node"))
}

fn flags_of(a: &Args) -> Map<String, J> {
    let mut m = Map::new();
    for k in ["exportMaterials", "exportUVs", "exportColorSets", "exportSkels", "exportSkin", "exportBlendShapes", "frameRange"] {
        if let Some(v) = a.get(k) {
            m.insert(k.to_string(), v.clone());
        }
    }
    m
}

// MayaReference prims.

fn reference_prim_info(prim: &Prim) -> Option<(String, String)> {
    let ty = prim.type_name().ok().flatten()?;
    if !matches!(ty.as_str(), "MayaReference" | "ALMayaReference") {
        return None;
    }
    let a = prim.attribute("mayaReference");
    let asset = a.get::<sdf::AssetPath>().ok().flatten()?;
    let file = asset.resolved_path().map_or_else(|| asset.asset_path().to_string(), str::to_string);
    let ns = prim.attribute("mayaNamespace").get::<String>().ok().flatten().unwrap_or_default();
    Some((file, ns))
}

/// Loads a reference prim's file as a file reference, under a group standing where the prim stands.
fn load_reference(s: &mut Session, name: &str) -> Result<String, String> {
    let t = resolve(&mut s.graph, name)?;
    let prim = t.entry.stage.prim(t.path.clone()).map_err(|e| e.to_string())?;
    let (file, ns) = reference_prim_info(&prim).ok_or_else(|| format!("{name} is not a reference reference prim"))?;
    let ns = if ns.is_empty() { node_name(t.path.name().unwrap_or("ref")) } else { ns };
    let before: Vec<NodeId> = s.graph.nodes();
    let made = aztek_scene::references::create(s, &file, &ns, None, false).map_err(|e| format!("{file}: {e}"))?;
    let time = stages::time_of(&mut s.graph, t.shape);
    let world = prims::world_of(&mut s.graph, &t, time);
    let group_type = types::find("transform").ok_or("no transform type")?;
    let group = s.graph.create_node(&group_type, &format!("{}_group", node_name(t.path.name().unwrap_or("ref"))), NO_NODE, true).map_err(|e| e.to_string())?;
    Transform::set_local_matrix(&mut s.graph, group, &world).map_err(|e| e.to_string())?;
    let new_roots: Vec<NodeId> = s.graph.roots().iter().copied().filter(|n| !before.contains(n) && *n != group).collect();
    for r in new_roots {
        s.graph.set_parent(r, group, false, usize::MAX).map_err(|e| e.to_string())?;
    }
    import::add_user_attr(&mut s.graph, group, ATTR_PRIM, PlugValue::String(t.path.as_str().to_string()));
    let proxy_name = s.graph.full_path(t.shape);
    import::add_user_attr(&mut s.graph, group, ATTR_PROXY, PlugValue::String(proxy_name));
    add_exclude(s, t.shape, t.path.as_str());
    let _ = made;
    Ok(s.graph.path(group))
}

/// Bakes a loaded reference to a USD file and references that file from the prim. The reference stays loaded.
fn cache_reference(s: &mut Session, a: &Args) -> CmdResult {
    let name = first_str(a)?;
    let out = a.string("file", "");
    if out.is_empty() {
        return Err(err("mayaUsdCacheMayaReference: -file is required"));
    }
    let (top, shape, path) = pulled_link(s, &name).map_err(err)?;
    let mut call = Map::new();
    call.insert("file".into(), json!(out));
    call.insert("exportRoots".into(), json!([s.graph.full_path(top)]));
    for (k, v) in flags_of(a) {
        call.insert(k, v);
    }
    s.run("mayaUSDExport", J::Object(call))?;
    // The cache sits on the prim's parent as a sibling reference target, picked up through a payload-free reference.
    let prim = name_of(&s.graph, shape, &path);
    let leaf = prim_name(s.graph.node(top).map_or("", |n| n.name()), false);
    s.run("usdPrim", json!({"args": [prim], "addReference": out, "targetPrim": format!("/{leaf}")}))?;
    Ok(json!(out))
}

// A transform driving a prim.

thread_local! {
    static DRIVERS: RefCell<HashMap<String, (String, DMat4)>> = RefCell::new(HashMap::new());
}

fn run_drive(s: &mut Session, a: &Args) -> CmdResult {
    let name = first_str(a)?;
    if a.bool("remove") {
        DRIVERS.with(|d| d.borrow_mut().remove(&name));
        return Ok(J::Null);
    }
    let node = a.string("driver", "");
    if s.graph.find(&node).map_err(err)?.is_none() {
        return Err(err(format!("no node named '{node}'")));
    }
    let _ = resolve(&mut s.graph, &name).map_err(err)?;
    DRIVERS.with(|d| d.borrow_mut().insert(name, (node, DMat4::ZERO)));
    drive_prims(&mut s.graph);
    Ok(J::Null)
}

/// Follows driving transforms, on the session layer: live, and not part of what the stage saves unless asked.
pub fn drive_prims(g: &mut Graph) {
    crate::extras::follow_cameras(g);
    let list: Vec<(String, String, DMat4)> = DRIVERS.with(|d| d.borrow().iter().map(|(k, v)| (k.clone(), v.0.clone(), v.1)).collect());
    if list.is_empty() {
        return;
    }
    g.evaluate();
    for (prim, node, last) in list {
        let Some(id) = g.find(&node).ok().flatten() else { continue };
        let Some(m) = g.node(id).map(|n| n.matrix(DagNode::WORLD_MATRIX)) else { continue };
        if m == last {
            continue;
        }
        let Ok(t) = resolve(g, &prim) else { continue };
        let time = stages::time_of(g, t.shape);
        let parent = prims::parent_world(g, &t, time);
        let local = parent.inverse() * m;
        if let Some(target) = t.entry.stage.edit_target_session() {
            if let Ok(ctx) = t.entry.stage.edit_context(target) {
                if let Ok(p) = t.entry.stage.prim(t.path.clone()) {
                    if let Err(e) = prims::set_local(&p, &local, time) {
                        log::warn!("USD: driving {prim}: {e}");
                    }
                }
                drop(ctx);
            }
        }
        DRIVERS.with(|d| {
            if let Some(x) = d.borrow_mut().get_mut(&prim) {
                x.1 = m;
            }
        });
    }
}

pub fn register(c: &mut Commands<Session>) {
    let f = |n: &str, sn: &str, ty: ArgType, d: &str| FlagDef::new(n, sn, ty, J::Null, d);
    c.register(
        CommandDef::new("mayaUsdEditAsMaya", "Edit As Scene Data: a prim becomes nodes under __mayaUsd__, its USD copy hidden. Returns the top node.", |s, a| {
            let n = first_str(a)?;
            let top = pull(s, &n).map_err(err)?;
            Ok(json!(s.graph.path(top)))
        })
        .positional("proxy,/path"),
    );
    let mut m = CommandDef::new("mayaUsdMergeToUsd", "Merge to USD: pulled nodes go back to the edit target and are removed. Returns the prim.", |s, a| {
        let n = first_str(a)?;
        merge(s, &n, &flags_of(a)).map(|x| json!(x)).map_err(err)
    })
    .positional("node");
    for (n, sn) in [("exportUVs", "uvs"), ("exportColorSets", "cls"), ("exportMaterials", "mat"), ("exportSkels", "skl"), ("exportSkin", "skn"), ("exportBlendShapes", "ebs")] {
        m = m.flag(f(n, sn, ArgType::Any, "As mayaUSDExport."));
    }
    c.register(m.flag(f("frameRange", "fr", ArgType::Any, "Bake samples over this range.").multi(2)));
    c.register(
        CommandDef::new("mayaUsdDiscardEdits", "Throw pulled nodes away: the prim shows as it was. Returns the prim.", |s, a| {
            let n = first_str(a)?;
            discard(s, &n).map(|x| json!(x)).map_err(err)
        })
        .positional("node"),
    );
    c.register(
        CommandDef::new("mayaUsdDuplicate", "Copy a prim to nodes, or nodes to a prim: mayaUsdDuplicate source destination. Nothing stays linked. Returns the copy.", |s, a| {
            let src = first_str(a)?;
            let dst = a.pos_str(1).unwrap_or("").to_string();
            duplicate(s, &src, &dst).map(|x| json!(x)).map_err(err)
        })
        .positional("source destination"),
    );
    c.register(
        CommandDef::new("AL_usdmaya_TranslatePrim", "As the Animal Logic USD plug-in: -ip paths imports prims as nodes, -tp paths tears them down, in the proxy -p.", |s: &mut Session, a: &Args| {
            let proxy = a.string("proxy", "");
            let shape = stages::proxy_named(&s.graph, &proxy).ok_or_else(|| err(format!("no proxy shape named '{proxy}'")))?;
            let full = s.graph.full_path(shape);
            let paths = comma_list;
            let mut out = Vec::new();
            for p in paths(a.get("importPaths")) {
                let top = pull(s, &format!("{full},{p}")).map_err(err)?;
                out.push(json!(s.graph.path(top)));
            }
            let torn = paths(a.get("teardownPaths"));
            if !torn.is_empty() {
                let nodes: Vec<NodeId> = s.graph.nodes();
                for id in nodes {
                    let path = get_str(&mut s.graph, id, ATTR_PRIM).unwrap_or_default();
                    if !path.is_empty() && torn.contains(&path) {
                        let n = s.graph.full_path(id);
                        discard(s, &n).map_err(err)?;
                    }
                }
            }
            Ok(J::Array(out))
        })
        .flag(f("proxy", "p", ArgType::String, "The proxy shape."))
        .flag(f("importPaths", "ip", ArgType::String, "Prims to import, separated by commas."))
        .flag(f("teardownPaths", "tp", ArgType::String, "Prims to tear down, separated by commas."))
        .flag(f("forceImport", "fi", ArgType::Bool, "Accepted."))
        .flag(f("forceDefault", "fd", ArgType::Bool, "Accepted.")),
    );
    c.register(
        CommandDef::new("mayaUsdLoadMayaReference", "A MayaReference or ALMayaReference prim's file as a file reference under a group at the prim. Returns the group.", |s, a| {
            let n = first_str(a)?;
            load_reference(s, &n).map(|x| json!(x)).map_err(err)
        })
        .positional("proxy,/path"),
    );
    c.register(
        CommandDef::new("mayaUsdCacheMayaReference", "Bake a loaded reference's group to a USD file and reference it from the prim.", cache_reference)
            .positional("group")
            .flag(f("file", "f", ArgType::String, "The cache file."))
            .flag(f("frameRange", "fr", ArgType::Any, "The frames to bake.").multi(2))
            .flag(f("exportSkels", "skl", ArgType::Any, "As mayaUSDExport."))
            .flag(f("exportSkin", "skn", ArgType::Any, "As mayaUSDExport.")),
    );
    c.register(
        CommandDef::new("usdDriveFromNode", "A transform drives a prim live, through the session layer: usdDriveFromNode proxy,/path -driver node. -remove stops it.", run_drive)
            .positional("proxy,/path")
            .flag(f("driver", "d", ArgType::String, "The driving transform."))
            .flag(f("remove", "rm", ArgType::Bool, "Stop driving."))
            .undoable(false),
    );
}
