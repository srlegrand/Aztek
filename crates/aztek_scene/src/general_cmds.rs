//! General commands: displayAffected, expandedSelection, metadata (dataStructure, addMetadata, editMetadata, getMetadata),
//! nodeCast, nodePreset, sysFile, getFileList, stringArrayIntersector, setAttrMapping, makePaintable and uvLink.
//!
//! Differs from the reference. Structures and scene-level metadata live in the session and are not saved; metadata on a node is an
//! added string attribute, so it saves and undoes with the node. Presets are JSON files rather than MEL.

use std::collections::BTreeMap;
use std::path::PathBuf;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use serde_json::{json, Map, Value};

use crate::graph::Graph;
use crate::ref_core::{f, glob};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::skin::asked;
use crate::system_cmds::q_flags;

type Cmd = CommandDef<Session>;
use ArgType::{Any as A, Bool as B, Float as Fl, Int as I, List as L, String as S};

/// Session state of the commands in this module and its neighbours. None of it is saved with the scene or undone.
#[derive(Default)]
pub struct MoreState {
    pub hik_release_pinning: bool,
    pub characterize: BTreeMap<String, Value>,
    pub relationships: Vec<(String, String, Vec<String>, String)>,
    pub xform_type: String,
    /// Copy Vertex Weights: influence and weight.
    pub copied_weights: Vec<(String, f64)>,
    pub xform_along_normal: i64,
    /// Settings by command, for commands that only hold them.
    pub stored: BTreeMap<String, BTreeMap<String, Value>>,
    pub structures: BTreeMap<String, Vec<(String, String)>>,
    pub scene_metadata: Value,
    pub intersectors: BTreeMap<String, Intersector>,
    pub paintable: Vec<Paintable>,
    pub uv_links: Vec<(String, String)>,
    pub ghosting: BTreeMap<String, Value>,
    pub time_code: BTreeMap<String, f64>,
    pub display_affected: bool,
    pub install_action: String,
    pub install_constraint: bool,
    pub install_display: bool,
}

#[derive(Default, Clone)]
pub struct Intersector {
    pub allow_duplicates: bool,
    pub started: bool,
    pub items: Vec<String>,
}

#[derive(Clone)]
pub struct Paintable {
    pub node_type: String,
    pub attr: String,
    pub attr_type: String,
    pub ui_name: String,
    pub active: bool,
}

fn truth(v: &Value) -> bool {
    v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0)
}

fn texts(v: Option<&Value>) -> Vec<String> {
    match v {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(l)) => l.iter().flat_map(|x| texts(Some(x))).collect(),
        Some(Value::String(t)) => vec![t.clone()],
        Some(x) => vec![x.to_string()],
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// expandedSelection

fn expanded_selection(s: &mut Session, a: &Args) -> CmdResult {
    let kind = a.string("expansionType", "DG");
    if !matches!(kind.as_str(), "DG" | "EG" | "SG") {
        fail!("expandedSelection: type {kind} is not DG, EG or SG");
    }
    let depth = a.int("depth", 1).max(0);
    let g = &s.graph;
    let links = g.connections();
    let mut seen: Vec<NodeId> = g.selection().to_vec();
    let mut front = seen.clone();
    for _ in 0..depth {
        let mut next = Vec::new();
        for &n in &front {
            for (d, src) in &links {
                let other = if d.node == n {
                    src.node
                } else if src.node == n {
                    d.node
                } else {
                    continue;
                };
                if !seen.contains(&other) {
                    seen.push(other);
                    next.push(other);
                }
            }
        }
        front = next;
    }
    Ok(json!(seen.iter().map(|&n| g.path(n)).collect::<Vec<_>>()))
}

// ---------------------------------------------------------------------------------------------------------------------
// Metadata

const META_ATTR: &str = "aztekMetadata";

fn meta_of(g: &mut Graph, n: NodeId) -> Value {
    let Some(i) = g.node(n).and_then(|x| x.ty().attr_index(META_ATTR)) else { return json!({}) };
    match g.value(&plug(n, i)) {
        Ok(PlugValue::String(t)) => serde_json::from_str(&t).unwrap_or(json!({})),
        _ => json!({}),
    }
}

fn set_meta(g: &mut Graph, n: NodeId, v: &Value) -> CmdResult<()> {
    let at = match g.node(n).and_then(|x| x.ty().attr_index(META_ATTR)) {
        Some(i) => i,
        None => g.add_attr(n, AttrDef { name: META_ATTR.into(), short_name: "azmd".into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: "Metadata streams, as JSON.".into(), ..Default::default() })?,
    };
    g.set_value(plug(n, at), PlugValue::String(v.to_string()))?;
    Ok(())
}

/// What a metadata command acts on: (node, channel from its components, indices). A node of None is the scene.
fn meta_targets(s: &mut Session, a: &Args) -> CmdResult<Vec<(Option<NodeId>, Option<String>, Vec<String>)>> {
    if a.bool("scene") {
        return Ok(vec![(None, None, Vec::new())]);
    }
    let mut names = a.named();
    if names.is_empty() {
        let v = s.run("ls", json!({"selection": true, "flatten": true}))?;
        names = texts(Some(&v));
    }
    if names.is_empty() {
        fail!("metadata: name or select an object, or use -scene");
    }
    let mut out: Vec<(Option<NodeId>, Option<String>, Vec<String>)> = Vec::new();
    for name in names {
        let (obj, comp) = match name.split_once('.') {
            Some((o, c)) => (o.to_string(), Some(c.to_string())),
            None => (name.clone(), None),
        };
        let id = find(&s.graph, &obj, "metadata")?;
        let id = crate::components::as_mesh(&s.graph, id).unwrap_or(id);
        let parsed = comp.as_deref().and_then(|c| {
            let (kind, rest) = c.split_once('[')?;
            let channel = match kind {
                "vtx" => "vertex",
                "e" => "edge",
                "f" => "face",
                _ => return None,
            };
            Some((channel.to_string(), rest.trim_end_matches(']').to_string()))
        });
        match parsed {
            Some((channel, index)) => match out.iter_mut().find(|x| x.0 == Some(id) && x.1.as_deref() == Some(channel.as_str())) {
                Some(x) => x.2.push(index),
                None => out.push((Some(id), Some(channel), vec![index])),
            },
            None => out.push((Some(id), None, Vec::new())),
        }
    }
    Ok(out)
}

fn read_meta(s: &mut Session, node: Option<NodeId>) -> Value {
    match node {
        Some(n) => meta_of(&mut s.graph, n),
        None => {
            if s.extras.more.scene_metadata.is_null() {
                json!({})
            } else {
                s.extras.more.scene_metadata.clone()
            }
        }
    }
}

fn write_meta(s: &mut Session, node: Option<NodeId>, v: Value) -> CmdResult<()> {
    match node {
        Some(n) => set_meta(&mut s.graph, n, &v),
        None => {
            s.extras.more.scene_metadata = v;
            Ok(())
        }
    }
}

fn data_structure(s: &mut Session, a: &Args) -> CmdResult {
    let more = &mut s.extras.more;
    if a.bool("removeAll") {
        more.structures.clear();
        return Ok(Value::Null);
    }
    if a.bool("remove") {
        more.structures.remove(&a.string("name", ""));
        return Ok(Value::Null);
    }
    if a.bool("query") {
        if asked(a, "format") {
            return Ok(json!(["raw"]));
        }
        let Some(name) = a.get("name").and_then(Value::as_str).filter(|n| !n.is_empty()) else { return Ok(json!(more.structures.keys().collect::<Vec<_>>())) };
        let Some(members) = more.structures.get(name) else { fail!("dataStructure: no structure {name}") };
        if asked(a, "listMemberNames") {
            let with_type = asked(a, "dataType");
            return Ok(json!(members.iter().flat_map(|(t, m)| if with_type { vec![m.clone(), t.clone()] } else { vec![m.clone()] }).collect::<Vec<_>>()));
        }
        if asked(a, "asString") {
            let body: Vec<String> = members.iter().map(|(t, m)| format!("{t}={m}")).collect();
            return Ok(json!(format!("name={name}:{}", body.join(":"))));
        }
        return Ok(json!(name));
    }
    let text = if let Some(p) = a.get("asFile").and_then(Value::as_str) {
        std::fs::read_to_string(p).map_err(|e| aztek_ops::CommandError::new(format!("dataStructure: {p}: {e}")))?.split_whitespace().collect::<Vec<_>>().join(":")
    } else {
        a.string("asString", "")
    };
    let mut name = String::new();
    let mut members = Vec::new();
    for part in text.split(':').filter(|p| !p.is_empty()) {
        let Some((k, v)) = part.split_once('=') else { fail!("dataStructure: {part} is not type=name") };
        if k == "name" {
            name = v.to_string();
        } else {
            members.push((k.to_string(), v.to_string()));
        }
    }
    if name.is_empty() {
        fail!("dataStructure: the description has no name=");
    }
    more.structures.insert(name.clone(), members);
    Ok(json!(name))
}

fn add_metadata(s: &mut Session, a: &Args) -> CmdResult {
    let structure = a.string("structure", "");
    let stream = a.string("streamName", "");
    if !s.extras.more.structures.contains_key(&structure) {
        fail!("addMetadata: no structure {structure}");
    }
    if stream.is_empty() {
        fail!("addMetadata: give -streamName");
    }
    let channel = a.string("channelName", &a.string("channelType", "vertex"));
    let index_type = a.string("indexType", "numeric");
    let mut out = Vec::new();
    for (node, _, _) in meta_targets(s, a)? {
        let mut m = read_meta(s, node);
        m[&stream] = json!({"structure": structure, "channel": channel, "indexType": index_type, "values": {}});
        write_meta(s, node, m)?;
        out.push(json!(node.map_or("scene".to_string(), |n| s.graph.path(n))));
    }
    Ok(Value::Array(out))
}

/// The indices flags name; failing that, those of the selected components.
fn index_list(a: &Args, from_components: &[String]) -> Vec<String> {
    let mut out = texts(a.get("index"));
    if let (Some(lo), Some(hi)) = (a.get("startIndex"), a.get("endIndex")) {
        let num = |v: &Value| v.as_i64().or_else(|| v.as_str().and_then(|t| t.parse().ok()));
        if let (Some(lo), Some(hi)) = (num(lo), num(hi)) {
            out.extend((lo..=hi).map(|i| i.to_string()));
        }
    }
    if out.is_empty() {
        out = from_components.to_vec();
    }
    out
}

fn edit_metadata(s: &mut Session, a: &Args) -> CmdResult {
    let stream_flag = a.string("streamName", "");
    let channel_flag = a.get("channelName").or(a.get("channelType")).and_then(Value::as_str).map(str::to_string);
    let remove = a.bool("remove");
    let member = a.string("memberName", "");
    if !remove && member.is_empty() {
        fail!("editMetadata: give -memberName, or -remove");
    }
    let value = a.get("stringValue").or(a.get("value")).cloned();
    if !remove && value.is_none() {
        fail!("editMetadata: give -value or -stringValue");
    }
    for (node, comp_channel, comps) in meta_targets(s, a)? {
        let indices = index_list(a, &comps);
        if indices.is_empty() {
            fail!("editMetadata: select components, or give -index or -startIndex and -endIndex");
        }
        let channel = comp_channel.or_else(|| channel_flag.clone());
        let mut m = read_meta(s, node);
        let Some(streams) = m.as_object_mut() else { fail!("editMetadata: bad metadata") };
        let mut touched = false;
        for (name, st) in streams.iter_mut() {
            if (!stream_flag.is_empty() && *name != stream_flag) || channel.as_deref().is_some_and(|c| st["channel"] != c) {
                continue;
            }
            touched = true;
            let structure = st["structure"].as_str().unwrap_or("").to_string();
            if !remove && !s.extras.more.structures.get(&structure).is_some_and(|x| x.iter().any(|(_, n)| *n == member)) {
                fail!("editMetadata: {structure} has no member {member}");
            }
            for i in &indices {
                if remove {
                    if let Some(v) = st["values"].as_object_mut() {
                        v.remove(i);
                    }
                } else {
                    st["values"][i][&member] = value.clone().unwrap();
                }
            }
        }
        if !touched {
            fail!("editMetadata: no stream to edit; run addMetadata first");
        }
        write_meta(s, node, m)?;
    }
    Ok(json!(true))
}

fn get_metadata(s: &mut Session, a: &Args) -> CmdResult {
    let stream_flag = a.string("streamName", "");
    let channel_flag = a.get("channelName").or(a.get("channelType")).and_then(Value::as_str).map(str::to_string);
    let member = a.string("memberName", "");
    let mut out = Vec::new();
    for (node, comp_channel, comps) in meta_targets(s, a)? {
        let channel = comp_channel.or_else(|| channel_flag.clone());
        let m = read_meta(s, node);
        let Some(streams) = m.as_object() else { continue };
        let chosen: Vec<(&String, &Value)> = streams.iter().filter(|(n, st)| (stream_flag.is_empty() || **n == stream_flag) && channel.as_deref().is_none_or(|c| st["channel"] == c)).collect();
        if a.bool("listStreamNames") {
            out.extend(chosen.iter().map(|x| json!(x.0)));
            continue;
        }
        if a.bool("listChannelNames") {
            for x in &chosen {
                if !out.contains(&x.1["channel"]) {
                    out.push(x.1["channel"].clone());
                }
            }
            continue;
        }
        for (_, st) in chosen {
            let structure = st["structure"].as_str().unwrap_or("");
            let Some(members) = s.extras.more.structures.get(structure) else { continue };
            if a.bool("listMemberNames") {
                out.extend(members.iter().map(|x| json!(x.1)));
                continue;
            }
            if a.bool("dataType") {
                if let Some(x) = members.iter().find(|x| x.1 == member) {
                    return Ok(json!(x.0));
                }
                continue;
            }
            let wanted: Vec<&str> = if member.is_empty() { members.iter().map(|x| x.1.as_str()).collect() } else { vec![member.as_str()] };
            let Some(values) = st["values"].as_object() else { continue };
            let asked_for = index_list(a, &comps);
            let mut keys: Vec<&String> = values.keys().filter(|k| asked_for.is_empty() || asked_for.contains(k)).collect();
            keys.sort_by_key(|k| (k.parse::<i64>().unwrap_or(i64::MAX), k.to_string()));
            for k in keys {
                for w in &wanted {
                    if let Some(v) = values[k].get(*w) {
                        out.push(v.clone());
                    }
                }
            }
        }
    }
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// nodeCast

fn node_cast(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let [source, target] = names.as_slice() else { fail!("nodeCast: give the source and the target") };
    let g = &mut s.graph;
    let (from, to) = (find(g, source, "nodeCast")?, find(g, target, "nodeCast")?);
    if from == to {
        fail!("nodeCast: the nodes are the same");
    }
    let (from_ty, to_ty) = (g.node(from).unwrap().ty().clone(), g.node(to).unwrap().ty().clone());
    if g.node(from).unwrap().is_locked() || g.node(to).unwrap().is_locked() {
        fail!("nodeCast: a node is locked");
    }
    if !from_ty.is_a(to_ty.name.as_str()) && !a.bool("force") {
        fail!("nodeCast: {} is not a kind of {}", from_ty.name, to_ty.name);
    }
    // Check everything first, so a failure leaves the nodes as they were.
    let links = g.connections();
    let mut moves: Vec<(PlugRef, PlugRef, PlugRef, PlugRef)> = Vec::new();
    let mut to_add: Vec<AttrDef> = Vec::new();
    for (d, src) in &links {
        if d.node != to && src.node != to {
            continue;
        }
        let swap = |p: &PlugRef| -> Option<PlugRef> {
            if p.node != to {
                return Some(*p);
            }
            let def = g.node(to)?.ty().attrs.get(p.attr as usize)?.clone();
            let at = g.node(from)?.ty().attr_index(&def.name)?;
            Some(PlugRef { node: from, attr: at as i32, component: p.component, element: p.element })
        };
        match (swap(d), swap(src)) {
            (Some(nd), Some(ns)) => moves.push((*d, *src, nd, ns)),
            _ => {
                let p = if d.node == to { d } else { src };
                let def = g.node(to).unwrap().ty().attrs[p.attr as usize].clone();
                if a.bool("copyDynamicAttrs") && def.flags & flags::USER != 0 {
                    to_add.push(def);
                } else if !a.bool("disconnectUnmatchedAttrs") {
                    fail!("nodeCast: {} has no attribute {}", from_ty.name, def.name);
                } else {
                }
            }
        }
    }
    g.open_chunk("Node Cast");
    let r = (|| -> CmdResult<()> {
        for def in &to_add {
            if g.node(from).unwrap().ty().attr_index(&def.name).is_none() {
                g.add_attr(from, def.clone())?;
            }
        }
        for (d, src) in &links {
            if d.node == to || src.node == to {
                g.disconnect(d);
            }
        }
        // Values move before the connections, so nothing writes to a driven plug.
        let common: Vec<(usize, usize)> = to_ty.attrs.iter().enumerate().filter(|(_, x)| x.flags & flags::INPUT != 0 && x.flags & flags::STORABLE != 0 && !x.is_array()).filter_map(|(i, x)| Some((i, g.node(from)?.ty().attr_index(&x.name)?))).collect();
        for (ti, fi) in common {
            let (tv, fv) = (g.value(&plug(to, ti))?, g.value(&plug(from, fi))?);
            let _ = g.set_value(plug(from, fi), tv);
            if a.bool("swapValues") {
                let _ = g.set_value(plug(to, ti), fv);
            }
        }
        for (_, _, nd, ns) in &moves {
            g.connect(ns, nd, true)?;
        }
        if g.node(from).unwrap().ty().dag && g.node(to).unwrap().ty().dag {
            for k in g.children(to).to_vec() {
                if k != from {
                    g.set_parent(k, from, false, usize::MAX)?;
                }
            }
            let p = g.parent(to);
            if p != g.parent(from) && p != from {
                g.set_parent(from, p, false, usize::MAX)?;
            }
        }
        if a.bool("swapNames") {
            let (x, y) = (g.node(from).unwrap().name().to_string(), g.node(to).unwrap().name().to_string());
            g.rename(to, "nodeCastSwap__")?;
            g.rename(from, &y)?;
            g.rename(to, &x)?;
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(json!(0))
}

// ---------------------------------------------------------------------------------------------------------------------
// nodePreset

fn preset_dir(ty: &str) -> PathBuf {
    aztek_core::paths::user_config_dir().join("presets").join("attrPresets").join(ty)
}

fn valid_preset_name(n: &str) -> bool {
    !n.is_empty() && n.chars().all(|c| c.is_alphanumeric() || c == '_')
}

fn preset_pair(a: &Args, flag: &str) -> Option<(String, String)> {
    let t = texts(a.get(flag));
    match t.as_slice() {
        [x, y] => Some((x.clone(), y.clone())),
        _ => None,
    }
}

fn node_preset(s: &mut Session, a: &Args) -> CmdResult {
    if let Some(n) = a.get("isValidName").and_then(Value::as_str) {
        return Ok(json!(valid_preset_name(n)));
    }
    let g = &mut s.graph;
    let type_of = |g: &Graph, n: &str| -> CmdResult<String> { Ok(g.node(find(g, n, "nodePreset")?).unwrap().ty().name.to_string()) };
    if let Some(n) = a.get("list").and_then(Value::as_str) {
        let ty = type_of(g, n)?;
        let mut out: Vec<String> = std::fs::read_dir(preset_dir(&ty)).map(|d| d.flatten().filter_map(|e| e.path().file_stem().map(|x| x.to_string_lossy().to_string())).collect()).unwrap_or_default();
        out.sort();
        return Ok(json!(out));
    }
    let file = |ty: &str, name: &str| preset_dir(ty).join(format!("{name}.json"));
    if let Some((n, name)) = preset_pair(a, "exists") {
        return Ok(json!(file(&type_of(g, &n)?, &name).exists()));
    }
    if let Some((n, name)) = preset_pair(a, "delete") {
        return Ok(json!(std::fs::remove_file(file(&type_of(g, &n)?, &name)).is_ok()));
    }
    if let Some((n, name)) = preset_pair(a, "save") {
        if !valid_preset_name(&name) {
            fail!("nodePreset: {name} is not a valid preset name");
        }
        let id = find(g, &n, "nodePreset")?;
        let ty = g.node(id).unwrap().ty().clone();
        let only: Vec<String> = a.string("attributes", "").split_whitespace().map(str::to_string).collect();
        let mut out = Map::new();
        for (i, def) in ty.attrs.iter().enumerate() {
            if def.flags & flags::INPUT == 0 || def.flags & flags::STORABLE == 0 || def.is_array() || def.flags & flags::HIDDEN != 0 {
                continue;
            }
            if !only.is_empty() && !only.contains(&def.name) && !only.contains(&def.short_name) {
                continue;
            }
            if let Ok(v) = g.value(&plug(id, i)) {
                out.insert(def.name.clone(), crate::plug::to_json(&v));
            }
        }
        std::fs::create_dir_all(preset_dir(&ty.name)).map_err(|e| aztek_ops::CommandError::new(format!("nodePreset: {e}")))?;
        std::fs::write(file(&ty.name, &name), Value::Object(out).to_string()).map_err(|e| aztek_ops::CommandError::new(format!("nodePreset: {e}")))?;
        return Ok(json!(true));
    }
    if let Some((n, name)) = preset_pair(a, "load") {
        let id = find(g, &n, "nodePreset")?;
        let ty = g.node(id).unwrap().ty().clone();
        let Ok(text) = std::fs::read_to_string(file(&ty.name, &name)) else { fail!("nodePreset: {} has no preset {name}", ty.name) };
        let Ok(Value::Object(values)) = serde_json::from_str::<Value>(&text) else { fail!("nodePreset: {name} can't be read") };
        for (k, v) in values {
            let Some(i) = ty.attr_index(&k) else { continue };
            if g.source(&plug(id, i)).is_some() {
                continue;
            }
            if let Ok(x) = crate::plug::from_json(&ty.attrs[i], &v) {
                g.set_value(plug(id, i), x)?;
            }
        }
        return Ok(json!(true));
    }
    fail!("nodePreset: use one of -save, -load, -list, -delete, -exists, -isValidName");
}

// ---------------------------------------------------------------------------------------------------------------------
// Files

fn sys_file(_s: &mut Session, a: &Args) -> CmdResult {
    let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("sysFile: give a path") };
    let ok = if a.bool("delete") {
        std::fs::remove_file(&path).is_ok()
    } else if a.bool("makeDir") {
        std::fs::create_dir_all(&path).is_ok()
    } else if a.bool("removeEmptyDir") {
        std::fs::remove_dir(&path).is_ok()
    } else if let Some(to) = a.get("copy").and_then(Value::as_str) {
        std::fs::copy(&path, to).is_ok()
    } else if let Some(to) = a.get("rename").or(a.get("move")).and_then(Value::as_str) {
        std::fs::rename(&path, to).is_ok()
    } else {
        fail!("sysFile: use one of -copy, -delete, -makeDir, -move, -removeEmptyDir, -rename")
    };
    Ok(json!(ok))
}

fn get_file_list(_s: &mut Session, a: &Args) -> CmdResult {
    let dir = a.string("folder", ".");
    let spec = a.string("filespec", "*");
    let Ok(rd) = std::fs::read_dir(&dir) else { return Ok(json!([])) };
    let mut out: Vec<String> = rd.flatten().map(|e| e.file_name().to_string_lossy().to_string()).filter(|n| glob(&spec, n)).collect();
    out.sort();
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// stringArrayIntersector

fn intersector(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).map_or_else(|| format!("stringArrayIntersector{}", s.extras.more.intersectors.len() + 1), str::to_string);
    let map = &mut s.extras.more.intersectors;
    if a.bool("exists") {
        return Ok(json!(map.contains_key(&name)));
    }
    if a.bool("query") {
        let Some(x) = map.get(&name) else { fail!("stringArrayIntersector: no intersector {name}") };
        if asked(a, "allowDuplicates") {
            return Ok(json!(x.allow_duplicates));
        }
        return Ok(json!(x.items));
    }
    if !a.bool("edit") {
        if map.contains_key(&name) {
            fail!("stringArrayIntersector: {name} already exists");
        }
        map.insert(name.clone(), Intersector { allow_duplicates: a.bool("allowDuplicates"), ..Default::default() });
    }
    let Some(x) = map.get_mut(&name) else { fail!("stringArrayIntersector: no intersector {name}") };
    if a.bool("reset") {
        x.started = false;
        x.items.clear();
    }
    if a.has("intersect") {
        let mut incoming = texts(a.get("intersect"));
        if !x.allow_duplicates {
            let mut seen = std::collections::HashSet::new();
            incoming.retain(|t| seen.insert(t.clone()));
        }
        if x.started {
            let mut left = incoming;
            let mut kept = Vec::new();
            for item in &x.items {
                if let Some(i) = left.iter().position(|t| t == item) {
                    left.remove(i);
                    kept.push(item.clone());
                }
            }
            x.items = kept;
        } else {
            x.items = incoming;
            x.started = true;
        }
    }
    Ok(json!(name))
}

// ---------------------------------------------------------------------------------------------------------------------
// makePaintable and uvLink

fn make_paintable(s: &mut Session, a: &Args) -> CmdResult {
    let list = &mut s.extras.more.paintable;
    if a.bool("clearAll") {
        list.clear();
        return Ok(Value::Null);
    }
    if let Some(v) = a.get("activateAll").filter(|_| !a.bool("query")) {
        list.iter_mut().for_each(|p| p.active = truth(v));
        return Ok(Value::Null);
    }
    let names = a.named();
    if a.bool("query") {
        let out: Vec<Value> = list.iter().filter(|p| names.first().is_none_or(|t| *t == p.node_type) && names.get(1).is_none_or(|t| *t == p.attr)).flat_map(|p| [json!(p.node_type), json!(p.attr), json!(p.attr_type), json!(p.ui_name)]).collect();
        return Ok(Value::Array(out));
    }
    let [ty, attr, ..] = names.as_slice() else { fail!("makePaintable: give the node type and the attribute") };
    if a.bool("remove") {
        list.retain(|p| !(p.node_type == *ty && p.attr == *attr));
        return Ok(Value::Null);
    }
    let active = a.get("activate").map_or(true, truth);
    match list.iter_mut().find(|p| p.node_type == *ty && p.attr == *attr) {
        Some(p) => {
            p.active = active;
            if a.has("attrType") {
                p.attr_type = a.string("attrType", "");
            }
            if a.has("uiName") {
                p.ui_name = a.string("uiName", "");
            }
        }
        None => list.push(Paintable { node_type: ty.clone(), attr: attr.clone(), attr_type: a.string("attrType", "multiFloat"), ui_name: a.string("uiName", attr), active }),
    }
    Ok(Value::Null)
}

fn uv_link(s: &mut Session, a: &Args) -> CmdResult {
    let (set, tex) = (a.get("uvSet").and_then(Value::as_str).map(str::to_string), a.get("texture").and_then(Value::as_str).map(str::to_string));
    if a.bool("isValid") {
        let g = &s.graph;
        let exists = |n: &str| g.find(n.split('.').next().unwrap_or(n)).ok().flatten().is_some();
        return Ok(json!(tex.as_deref().is_none_or(exists) && set.as_deref().is_none_or(exists)));
    }
    let links = &mut s.extras.more.uv_links;
    if a.bool("query") {
        if let Some(u) = set {
            return Ok(json!(links.iter().filter(|l| l.0 == u).map(|l| l.1.clone()).collect::<Vec<_>>()));
        }
        if let Some(t) = tex {
            let only = a.get("queryObject").and_then(Value::as_str).map(str::to_string);
            return Ok(json!(links.iter().filter(|l| l.1 == t && only.as_deref().is_none_or(|o| l.0.starts_with(&format!("{o}.")))).map(|l| l.0.clone()).collect::<Vec<_>>()));
        }
        fail!("uvLink: query needs -uvSet or -texture");
    }
    let (Some(u), Some(t)) = (set, tex) else { fail!("uvLink: give -uvSet and -texture") };
    if a.bool("b") {
        links.retain(|l| !(l.0 == u && l.1 == t));
        return Ok(json!(format!("Broke link between {u} and {t}")));
    }
    if !links.contains(&(u.clone(), t.clone())) {
        links.push((u.clone(), t.clone()));
    }
    Ok(json!(format!("Linked {u} and {t}")))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        q_flags(Cmd::new("displayAffected", "Held for scripts: whether objects affected by the selection get their special colour. The views do not draw that colour. Takes on or off; -q reads it.", |s, a| {
            if !a.bool("query") {
                if let Some(v) = a.positional.first() {
                    s.extras.more.display_affected = truth(v);
                }
            }
            Ok(json!(s.extras.more.display_affected))
        }))
        .positional("on or off")
        .undoable(false),
    );
    c.register(
        Cmd::new("expandedSelection", "The selection and what is connected to it: -d steps (1), -et DG, EG or SG. EG and SG follow the same connections as DG: there is no separate evaluation graph here, and clusters are not reported.", expanded_selection)
            .flag(f("depth", "d", I, "Steps away from the selection."))
            .flag(f("expansionType", "et", S, "DG, EG or SG."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("dataStructure", "Define a metadata structure: -fmt raw -asString \"name=N:int32=ID:string=label\", or -af a file. -q lists them; -q -n N with -lmn, -dt or -asString reads one. -rm and -ral remove. Structures live in the session and are not saved.", data_structure))
            .flag(f("asFile", "af", S, "A file holding the description."))
            .flag(f("asString", "asString", A, "The description."))
            .flag(f("dataType", "dt", B, "With -lmn: each member's type too."))
            .flag(f("format", "fmt", A, "raw is the one there is."))
            .flag(f("listMemberNames", "lmn", A, "The member names."))
            .flag(f("name", "n", S, "The structure asked about."))
            .flag(f("remove", "rm", B, "Remove -n."))
            .flag(f("removeAll", "ral", B, "Remove all."))
            .undoable(false),
    );
    let meta_flags = |c: Cmd| {
        c.positional("objects or components (the selection when none)")
            .flag(f("channelName", "cn", S, "The channel: vertex, edge, face or your own."))
            .flag(f("channelType", "cht", S, "Obsolete: as -cn."))
            .flag(f("endIndex", "eix", A, "Last index of a range."))
            .flag(f("index", "idx", A, "An index.").multi(1))
            .flag(f("indexType", "idt", S, "The index's type: numeric unless given."))
            .flag(f("memberName", "mn", S, "The structure's member."))
            .flag(f("scene", "scn", B, "On the scene rather than a node."))
            .flag(f("startIndex", "six", A, "First index of a range."))
            .flag(f("streamName", "stn", S, "The stream."))
    };
    c.register(meta_flags(Cmd::new("addMetadata", "Attach a metadata stream of a structure to objects: -structure, -stn, -cn, -idt. Returns what it went on.", add_metadata)).flag(f("structure", "str", S, "The structure.")));
    c.register(meta_flags(Cmd::new("editMetadata", "Set metadata: -stn, -mn, -v or -sv, on selected components or -idx, -six and -eix. -rm removes those indices. Returns true.", edit_metadata)).flag(f("value", "v", Fl, "A number.")).flag(f("stringValue", "sv", S, "A string.")).flag(f("remove", "rm", B, "Remove the indices.")));
    c.register(meta_flags(Cmd::new("getMetadata", "Read metadata values, flattened in index order: -stn, -mn, -cn, indices as for editMetadata. -lsn, -lcn, -lmn list names, -dt a member's type.", get_metadata)).flag(f("dataType", "dt", B, "The member's type.")).flag(f("listChannelNames", "lcn", B, "The channels.")).flag(f("listMemberNames", "lmn", B, "The members.")).flag(f("listStreamNames", "lsn", B, "The streams.")).undoable(false));
    c.register(
        Cmd::new("nodeCast", "Replace the target node with the source: its connections, hierarchy and values move to the source, whose type must be the target's or a kind of it (-f skips that). -cda, -dua, -sn, -sv. Returns 0. -dsa and -dsj are accepted and change nothing. Referenced nodes are not refused.", node_cast)
            .positional("source, then target")
            .flag(f("copyDynamicAttrs", "cda", B, "Bring added attributes the source lacks."))
            .flag(f("disableAPICallbacks", "dsa", B, "Accepted for existing scripts; has no effect."))
            .flag(f("disableScriptJobCallbacks", "dsj", B, "Accepted for existing scripts; has no effect."))
            .flag(f("disconnectUnmatchedAttrs", "dua", B, "Drop connections the source has no attribute for."))
            .flag(f("force", "f", B, "Try even when the types differ."))
            .flag(f("swapNames", "sn", B, "Swap the names."))
            .flag(f("swapValues", "sv", B, "Exchange common values rather than copy them.")),
    );
    c.register(
        Cmd::new("nodePreset", "Save and load a node's settings by name: -sv node name, -ld node name, -ls node, -del node name, -ex node name, -ivn name; -atr limits what is saved. Less than the reference: multis and hidden attributes are left out, files are JSON, -ctm does nothing.", node_preset)
            .flag(f("attributes", "atr", S, "Attributes to save, separated by spaces."))
            .flag(f("custom", "ctm", S, "Accepted for existing scripts; has no effect."))
            .flag(f("delete", "delete", L, "Node and preset."))
            .flag(f("exists", "ex", L, "Node and preset."))
            .flag(f("isValidName", "ivn", S, "A name to check."))
            .flag(f("list", "ls", S, "The node."))
            .flag(f("load", "ld", L, "Node and preset."))
            .flag(f("save", "sv", L, "Node and preset."))
            .undoable(false),
    );
    c.register(
        Cmd::new("sysFile", "File operations on a path: -cp, -delete, -md, -mov, -red, -ren. Returns whether it worked. Not undoable, unlike what the reference's reference says.", sys_file)
            .positional("the path")
            .flag(f("copy", "cp", S, "Copy to this name."))
            .flag(f("delete", "delete", B, "Delete the file."))
            .flag(f("makeDir", "md", B, "Make the directory and those above it."))
            .flag(f("move", "mov", S, "As -rename."))
            .flag(f("removeEmptyDir", "red", B, "Remove the directory if empty."))
            .flag(f("rename", "ren", S, "Rename to this name."))
            .undoable(false),
    );
    c.register(Cmd::new("getFileList", "File names in -fld matching -fs, sorted.", get_file_list).flag(f("filespec", "fs", S, "A wildcard.")).flag(f("folder", "fld", S, "The directory.")).undoable(false));
    c.register(
        q_flags(Cmd::new("stringArrayIntersector", "An object holding the intersection of string arrays so far: name it, -e -i array, -q reads it, -e -r starts again, -ex asks, -ad keeps duplicates. -dt and -ut are accepted and do nothing.", intersector))
            .positional("its name")
            .flag(f("allowDuplicates", "ad", B, "Keep duplicate entries."))
            .flag(f("defineTemplate", "dt", S, "Accepted for existing scripts; has no effect."))
            .flag(f("exists", "ex", B, "Whether it exists."))
            .flag(f("intersect", "i", L, "An array."))
            .flag(f("reset", "r", B, "Start again."))
            .flag(f("useTemplate", "ut", S, "Accepted for existing scripts; has no effect."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("setAttrMapping", "Held only: an offset and scale for a device attachment. Input devices are not supported, so it has no effect. -q reads a flag back.", |s, a| {
            crate::rig_extra::stored(s, a, "setAttrMapping", &["absolute", "attribute", "axis", "clutch", "device", "offset", "relative", "scale", "selection"])
        }))
        .flag(f("absolute", "a", B, "Absolute mapping."))
        .flag(f("attribute", "at", S, "The attribute."))
        .flag(f("axis", "ax", S, "The device axis."))
        .flag(f("clutch", "c", S, "The clutch button."))
        .flag(f("device", "d", S, "The device."))
        .flag(f("offset", "o", Fl, "The offset."))
        .flag(f("relative", "r", B, "Relative mapping."))
        .flag(f("scale", "s", Fl, "The scale."))
        .flag(f("selection", "sl", B, "On the selected objects."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("makePaintable", "Held for scripts: registers node type and attribute as paintable. -at, -ui, -ac, -aca, -rm, -ca; -q lists type, attribute, kind and name for each. The Paint Attributes tool does not read this list: it paints deformer weights only.", make_paintable))
            .positional("node type and attribute")
            .flag(f("activate", "ac", B, "Turn it on or off."))
            .flag(f("activateAll", "aca", B, "Turn all on or off."))
            .flag(f("altAttribute", "aa", S, "Accepted for existing scripts; has no effect.").multi(1))
            .flag(f("attrType", "at", S, "intArray, doubleArray, vectorArray, multiInteger, multiFloat, multiDouble or multiVector."))
            .flag(f("clearAll", "ca", B, "Remove all."))
            .flag(f("remove", "rm", B, "Remove this one."))
            .flag(f("shapeMode", "sm", S, "Accepted for existing scripts; has no effect."))
            .flag(f("uiName", "ui", S, "The name shown."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("uvLink", "Held for scripts: links between UV sets and textures. -uvs and -t make one (or -m), -b breaks it; -q -uvs lists textures, -q -t lists UV sets, -qo limits to an object; -iv checks the objects exist. No uvChooser node is made and nothing draws from it. The list is not saved.", uv_link))
            .flag(f("b", "b", B, "Break the link."))
            .flag(f("isValid", "iv", B, "Whether the texture or UV set exists."))
            .flag(f("make", "m", B, "Make the link: what happens without -b."))
            .flag(f("queryObject", "qo", S, "With -q -t: only this object."))
            .flag(f("texture", "t", S, "The texture."))
            .flag(f("uvSet", "uvs", S, "The UV set plug."))
            .undoable(false),
    );
}
