//! Layers of a stage: `mayaUsdLayerEditor`, `mayaUsdEditTarget`, `usdLayer` for what those don't cover (save, reload, the
//! tree the Layer Editor shows), the Animal Logic USD plug-in's layer commands, and what a proxy keeps with the scene for unsaved layers.

use std::collections::HashMap;
use std::rc::Rc;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::graph::Graph;
use aztek_scene::plug::{NodeId, PlugValue};
use aztek_scene::session::Session;
use aztek_undo::FnAction;
use openusd::sdf::{self, LayerOffset};
use openusd::usd::EditTarget;
use serde_json::{json, Value as J};

use crate::prims::err;
use crate::stages::{self, Entry};
use crate::util::*;

/// Puts a layer's contents in place of what the stage holds for it.
pub fn replace_contents(e: &Entry, identifier: &str, text: &str) -> Result<(), String> {
    let new = sdf::Layer::from_bytes("restore.usda", text.as_bytes().to_vec()).map_err(|x| x.to_string())?;
    let data = sdf::Data::from_abstract(new.data()).map_err(|x| x.to_string())?;
    replace_data(e, identifier, &data)
}

fn replace_data(e: &Entry, identifier: &str, data: &sdf::Data) -> Result<(), String> {
    e.stage
        .batch_edit(&[identifier], |edits| {
            let d = edits[0].data_mut();
            for p in d.spec_paths() {
                if !p.is_abs_root() {
                    d.erase_spec(&p);
                }
            }
            for (path, spec) in data.iter() {
                if !path.is_abs_root() {
                    d.create_spec(path.clone(), spec.ty);
                }
                for (k, v) in &spec.fields {
                    d.set_field(path, k, v.clone());
                }
            }
            Ok(())
        })
        .map(|_| ())
        .map_err(|x| x.to_string())
}

/// Applies what the scene saved for layers that had unsaved edits.
pub fn restore_dirty(e: &Rc<Entry>, json_text: &str) {
    if json_text.is_empty() {
        return;
    }
    let Ok(map) = serde_json::from_str::<HashMap<String, String>>(json_text) else { return };
    for (id, text) in map {
        // An anonymous root gets a new name each session; "root" stands for it.
        let target = if id == "root" { e.stage.root_layer().identifier().to_string() } else { id };
        if let Err(x) = replace_contents(e, &target, &text) {
            log::warn!("USD: can't restore {target}: {x}");
        }
    }
}

fn is_anon_root(e: &Entry, id: &str) -> bool {
    e.file.is_empty() && e.stage.root_layer().identifier() == id
}

/// A layer's own sublayers, by identifier, strongest first. Read from what it authors, since the stage's stack is flattened
/// and leaves muted layers out; one that can't be found keeps its authored path.
pub fn direct_sublayers(e: &Entry, id: &str) -> Vec<String> {
    let Some(l) = e.stage.layer(id) else { return Vec::new() };
    let authored = l.pseudo_root().and_then(|r| r.sublayers()).unwrap_or_default();
    let anchor = l.resolved_path().and_then(|p| std::path::Path::new(p).parent().map(|d| d.to_path_buf()));
    drop(l);
    let known = e.stage.layer_identifiers();
    authored
        .into_iter()
        .map(|a| {
            if known.iter().any(|k| *k == a) {
                return a;
            }
            let joined = anchor.as_ref().map(|d| d.join(&a));
            let tidy = joined.and_then(|j| std::fs::canonicalize(&j).ok()).map(|j| j.to_string_lossy().into_owned());
            match tidy {
                Some(t) if known.iter().any(|k| *k == t) => t,
                _ => a,
            }
        })
        .collect()
}

/// The layer tree of a stage, session first, as the Layer Editor shows it.
pub fn tree(e: &Entry) -> J {
    fn node(e: &Entry, id: &str, target: &str, role: &str, seen: &mut Vec<String>) -> J {
        let dirty = e.is_dirty(id);
        let (anon, path) = match e.stage.layer(id) {
            Some(l) => (l.is_anonymous() || is_anon_root(e, id), l.resolved_path().unwrap_or("").to_string()),
            None => (false, String::new()),
        };
        let mut kids = Vec::new();
        if !seen.iter().any(|x| x == id) {
            seen.push(id.to_string());
            for k in direct_sublayers(e, id) {
                kids.push(node(e, &k, target, "sublayer", seen));
            }
        }
        let name = std::path::Path::new(id).file_name().and_then(|x| x.to_str()).unwrap_or(id).to_string();
        json!({
            "identifier": id,
            "name": if role == "session" { "sessionLayer".to_string() } else if anon { "anonymousLayer".to_string() } else { name },
            "role": role,
            "path": path,
            "dirty": dirty,
            "anonymous": anon,
            "muted": e.stage.is_layer_muted(id),
            "locked": e.locked.borrow().iter().any(|x| x == id),
            "editTarget": id == target,
            "missing": e.stage.layer(id).is_none(),
            "sublayers": kids,
        })
    }
    let target = e.stage.edit_target().layer_identifier().to_string();
    let mut seen = Vec::new();
    let root = e.stage.root_layer().identifier().to_string();
    let mut out = Vec::new();
    if !e.session_identifier.is_empty() {
        out.push(node(e, &e.session_identifier, &target, "session", &mut seen));
    }
    out.push(node(e, &root, &target, "root", &mut seen));
    J::Array(out)
}

/// The stage a command means: the proxy named, or the only or selected one.
fn entry_for(s: &mut Session, proxy: Option<&str>) -> Result<(NodeId, Rc<Entry>), String> {
    let shape = match proxy.filter(|p| !p.is_empty()) {
        Some(p) => stages::proxy_named(&s.graph, p).ok_or_else(|| format!("no proxy shape named '{p}'"))?,
        None => {
            let sel: Vec<NodeId> = s.graph.selection().to_vec();
            let from_sel = sel.into_iter().find_map(|n| {
                if stages::is_proxy(&s.graph, n) {
                    Some(n)
                } else {
                    s.graph.children(n).iter().copied().find(|&c| stages::is_proxy(&s.graph, c))
                }
            });
            let from_prim = crate::prims::selected().first().and_then(|n| crate::prims::split_name(n).and_then(|(sh, _)| stages::proxy_named(&s.graph, sh)));
            match from_sel.or(from_prim).or_else(|| stages::all_proxies(&s.graph).first().copied()) {
                Some(n) => n,
                None => return Err("no proxy shape in the scene".to_string()),
            }
        }
    };
    let e = stages::entry_of(&mut s.graph, shape)?;
    Ok((shape, e))
}

/// The proxy showing a layer, for commands that name only the layer.
fn entry_with_layer(s: &mut Session, layer: &str, proxy: Option<&str>) -> Result<(NodeId, Rc<Entry>), String> {
    if proxy.is_some_and(|p| !p.is_empty()) {
        return entry_for(s, proxy);
    }
    for shape in stages::all_proxies(&s.graph) {
        let e = stages::entry_of(&mut s.graph, shape)?;
        if e.stage.layer(layer).is_some() {
            return Ok((shape, e));
        }
    }
    entry_for(s, None)
}

fn remember_state(g: &mut Graph, shape: NodeId, e: &Entry) {
    let target = e.stage.edit_target().layer_identifier().to_string();
    let root = e.stage.root_layer().identifier().to_string();
    let muted = e.stage.muted_layers().join(",");
    let locked = e.locked.borrow().join(",");
    g.without_undo(|g| {
        set_attr(g, shape, "editTargetLayer", PlugValue::String(if target == root { String::new() } else { target }));
        set_attr(g, shape, "mutedLayers", PlugValue::String(muted));
        set_attr(g, shape, "lockedLayers", PlugValue::String(locked));
    });
}

/// Applies the edit target, mutes and locks a proxy kept in its attributes.
pub fn restore_state(g: &mut Graph, shape: NodeId, e: &Rc<Entry>) {
    for m in get_str(g, shape, "mutedLayers").unwrap_or_default().split(',').filter(|x| !x.is_empty()) {
        e.stage.mute_layer(m);
    }
    *e.locked.borrow_mut() = get_str(g, shape, "lockedLayers").unwrap_or_default().split(',').filter(|x| !x.is_empty()).map(str::to_string).collect();
    let t = get_str(g, shape, "editTargetLayer").unwrap_or_default();
    if !t.is_empty() && e.stage.layer(&t).is_some() {
        let _ = e.stage.set_edit_target(EditTarget::for_layer(t));
    }
}

fn set_target(s: &mut Session, shape: NodeId, e: &Rc<Entry>, layer: &str) -> Result<(), String> {
    if e.stage.layer(layer).is_none() {
        return Err(format!("{layer} isn't a layer of this stage"));
    }
    if e.stage.is_layer_muted(layer) {
        return Err(format!("{layer} is muted, so it can't be the edit target"));
    }
    if e.locked.borrow().iter().any(|x| x == layer) {
        return Err(format!("{layer} is locked, so it can't be the edit target"));
    }
    let before = e.stage.edit_target().layer_identifier().to_string();
    e.stage.set_edit_target(EditTarget::for_layer(layer.to_string())).map_err(|x| x.to_string())?;
    remember_state(&mut s.graph, shape, e);
    let (a, b, l) = (e.clone(), e.clone(), layer.to_string());
    s.graph.undo_stack_mut().record(FnAction::new(
        move |_g: &mut Graph| {
            let _ = a.stage.set_edit_target(EditTarget::for_layer(before.clone()));
        },
        move |_g: &mut Graph| {
            let _ = b.stage.set_edit_target(EditTarget::for_layer(l.clone()));
        },
    ));
    Ok(())
}

fn set_muted(s: &mut Session, shape: NodeId, e: &Rc<Entry>, layer: &str, on: bool) -> Result<(), String> {
    if e.stage.root_layer().identifier() == layer {
        return Err("the root layer can't be muted".to_string());
    }
    let apply = |en: &Entry, l: &str, m: bool| {
        if m {
            en.stage.mute_layer(l);
        } else {
            en.stage.unmute_layer(l);
        }
    };
    apply(e, layer, on);
    // Edits can't go to what isn't composed.
    if on && e.stage.edit_target().layer_identifier() == layer {
        let _ = e.stage.set_edit_target(e.stage.edit_target_root());
    }
    remember_state(&mut s.graph, shape, e);
    let (a, b, l1, l2) = (e.clone(), e.clone(), layer.to_string(), layer.to_string());
    s.graph.undo_stack_mut().record(FnAction::new(move |_g: &mut Graph| apply(&a, &l1, !on), move |_g: &mut Graph| apply(&b, &l2, on)));
    Ok(())
}

fn set_locked(s: &mut Session, shape: NodeId, e: &Rc<Entry>, layer: &str, on: bool) {
    let apply = |en: &Entry, l: &str, m: bool| {
        let mut v = en.locked.borrow_mut();
        v.retain(|x| x != l);
        if m {
            v.push(l.to_string());
        }
    };
    apply(e, layer, on);
    if on && e.stage.edit_target().layer_identifier() == layer {
        let _ = e.stage.set_edit_target(e.stage.edit_target_root());
    }
    remember_state(&mut s.graph, shape, e);
    let (a, b, l1, l2) = (e.clone(), e.clone(), layer.to_string(), layer.to_string());
    s.graph.undo_stack_mut().record(FnAction::new(move |_g: &mut Graph| apply(&a, &l1, !on), move |_g: &mut Graph| apply(&b, &l2, on)));
}

fn insert_sublayer(s: &mut Session, e: &Rc<Entry>, parent: &str, pos: usize, path: &str) -> Result<String, String> {
    let layer = sdf::Layer::open(path).map_err(|x| format!("{path}: {x}"))?;
    let id = layer.identifier().to_string();
    e.clone().edit(s, "sublayer", |st| st.insert_layer(parent, pos, layer, LayerOffset::default()).map_err(|x| x.to_string()))?;
    Ok(id)
}

fn add_anonymous(s: &mut Session, e: &Rc<Entry>, parent: &str, name: &str) -> Result<String, String> {
    let layer = sdf::Layer::new_anonymous(if name.is_empty() { "anonymousLayer1" } else { name });
    let id = layer.identifier().to_string();
    e.clone().edit(s, "sublayer", |st| st.insert_layer(parent, 0, layer, LayerOffset::default()).map_err(|x| x.to_string()))?;
    Ok(id)
}

fn clear_layer(s: &mut Session, e: &Rc<Entry>, id: &str) -> Result<(), String> {
    let empty = {
        let mut d = sdf::Data::new();
        d.create_spec(sdf::Path::abs_root(), sdf::SpecType::PseudoRoot);
        d
    };
    let l = id.to_string();
    e.clone().edit(s, "clear", |_| replace_data(e, &l, &empty))
}

/// Discards unsaved edits: the file's contents again, or nothing for an anonymous layer.
fn reload(s: &mut Session, e: &Rc<Entry>, id: &str) -> Result<(), String> {
    let path = e.stage.layer(id).and_then(|l| l.resolved_path().map(str::to_string));
    match path.filter(|p| std::path::Path::new(p).exists()) {
        Some(p) => {
            let fresh = sdf::Layer::open(&p).map_err(|x| format!("{p}: {x}"))?;
            let data = sdf::Data::from_abstract(fresh.data()).map_err(|x| x.to_string())?;
            let l = id.to_string();
            e.clone().edit(s, "reload", |_| replace_data(e, &l, &data))?;
            e.set_clean(id);
            Ok(())
        }
        None => clear_layer(s, e, id),
    }
}

/// Everything composed into one layer, in place of the root's contents.
fn flatten_root(s: &mut Session, e: &Rc<Entry>) -> Result<(), String> {
    let flat = e.stage.flatten().map_err(|x| x.to_string())?;
    let data = sdf::Data::from_abstract(flat.data()).map_err(|x| x.to_string())?;
    let root = e.stage.root_layer().identifier().to_string();
    e.clone().edit(s, "flatten", |_| replace_data(e, &root, &data))
}

/// Writes a layer to its file, or to `to`. An anonymous root saved under a name becomes the stage's file.
fn save(s: &mut Session, shape: NodeId, e: &Rc<Entry>, id: &str, to: Option<&str>) -> Result<String, String> {
    let dest = to.filter(|x| !x.is_empty());
    let root_anon = is_anon_root(e, id);
    match dest {
        Some(file) => {
            let l = e.stage.layer(id).ok_or_else(|| format!("no layer {id}"))?;
            l.export(file).map_err(|x| format!("{file}: {x}"))?;
            drop(l);
            e.set_clean(id);
            if root_anon {
                let session = e.stage.session_layer().and_then(|x| x.export_to_string().ok()).unwrap_or_default();
                s.graph.without_undo(|g| set_attr(g, shape, "serializedSessionLayer", PlugValue::String(session)));
                set_attr(&mut s.graph, shape, "filePath", PlugValue::String(file.to_string()));
                stages::entry_of(&mut s.graph, shape)?;
            }
            Ok(file.to_string())
        }
        None => {
            if root_anon || e.stage.layer(id).is_some_and(|l| l.is_anonymous()) {
                return Err(format!("{id} is anonymous: give it a file"));
            }
            let mut l = e.stage.layer_mut(id).ok_or_else(|| format!("no layer {id}"))?;
            l.save().map_err(|x| format!("{id}: {x}"))?;
            let out = l.identifier().to_string();
            drop(l);
            e.set_clean(id);
            Ok(out)
        }
    }
}

fn run_layer_editor(s: &mut Session, a: &Args) -> CmdResult {
    let layer = a.pos_str(0).unwrap_or("").to_string();
    let pair = |n: &str| -> Option<(J, J)> {
        let v = a.get(n)?.as_array()?;
        Some((v.first()?.clone(), v.get(1)?.clone()))
    };
    let proxy = pair("muteLayer").or(pair("lockLayer")).and_then(|(_, p)| p.as_str().map(str::to_string));
    let (shape, e) = entry_with_layer(s, &layer, proxy.as_deref()).map_err(err)?;
    let id = if layer.is_empty() { e.stage.root_layer().identifier().to_string() } else { layer };
    let as_bool = |v: &J| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0);
    let mut result = J::Null;
    if let Some((pos, path)) = pair("insertSubPath") {
        let path = path.as_str().unwrap_or("").to_string();
        result = json!(insert_sublayer(s, &e, &id, pos.as_i64().unwrap_or(0).max(0) as usize, &path).map_err(err)?);
    }
    if let Some((pos, _)) = pair("removeSubPath") {
        let kids = direct_sublayers(&e, &id);
        if let Some(k) = kids.get(pos.as_i64().unwrap_or(0).max(0) as usize).cloned() {
            let parent = id.clone();
            e.clone().edit(s, "sublayer", |st| st.remove_layer(&parent, &k).map(|_| ()).map_err(|x| x.to_string())).map_err(err)?;
        }
    }
    if let Some(name) = a.get("addAnonymous").and_then(J::as_str) {
        result = json!(add_anonymous(s, &e, &id, name).map_err(err)?);
    }
    if a.bool("clear") {
        clear_layer(s, &e, &id).map_err(err)?;
    }
    if a.bool("discardEdits") {
        reload(s, &e, &id).map_err(err)?;
    }
    if a.bool("flatten") {
        flatten_root(s, &e).map_err(err)?;
    }
    if let Some((on, _)) = pair("muteLayer") {
        set_muted(s, shape, &e, &id, as_bool(&on)).map_err(err)?;
    }
    if let Some((on, _)) = pair("lockLayer") {
        set_locked(s, shape, &e, &id, on.as_i64().unwrap_or(i64::from(as_bool(&on))) != 0);
    }
    Ok(result)
}

fn run_edit_target(s: &mut Session, a: &Args) -> CmdResult {
    let proxy = a.pos_str(0).map(str::to_string);
    let (shape, e) = entry_for(s, proxy.as_deref()).map_err(err)?;
    if a.bool("query") {
        return Ok(json!(e.stage.edit_target().layer_identifier()));
    }
    let layer = a.string("editTarget", "");
    set_target(s, shape, &e, &layer).map_err(err)?;
    Ok(json!(layer))
}

fn run_usd_layer(s: &mut Session, a: &Args) -> CmdResult {
    let layer = a.pos_str(0).unwrap_or("").to_string();
    let proxy = a.string("proxy", "");
    let (shape, e) = entry_with_layer(s, &layer, Some(&proxy)).map_err(err)?;
    if a.bool("query") {
        return Ok(tree(&e));
    }
    let id = if layer.is_empty() { e.stage.root_layer().identifier().to_string() } else { layer };
    if a.bool("save") {
        return save(s, shape, &e, &id, None).map(|x| json!(x)).map_err(err);
    }
    if a.has("saveAs") {
        return save(s, shape, &e, &id, Some(&a.string("saveAs", ""))).map(|x| json!(x)).map_err(err);
    }
    if a.bool("reload") {
        reload(s, &e, &id).map_err(err)?;
    }
    if a.has("flattenTo") {
        let f = a.string("flattenTo", "");
        e.stage.flatten().map_err(err)?.export(&f).map_err(err)?;
        return Ok(json!(f));
    }
    if a.bool("saveDirty") {
        let mut saved = Vec::new();
        for l in dirty_layers(&e) {
            if !e.stage.layer(&l).is_some_and(|x| x.is_anonymous()) && !is_anon_root(&e, &l) && l != e.session_identifier {
                saved.push(save(s, shape, &e, &l, None).map_err(err)?);
            }
        }
        return Ok(json!(saved));
    }
    Ok(J::Null)
}

pub fn dirty_layers(e: &Entry) -> Vec<String> {
    e.dirty.borrow().clone()
}

/// Before a scene saves: what proxies must carry for their layers to come back as they are. Unmodified layers stay
/// references to their files and add nothing.
pub fn before_save(s: &mut Session) -> Vec<String> {
    let mut named = Vec::new();
    for shape in stages::all_proxies(&s.graph) {
        let Ok(e) = stages::entry_of(&mut s.graph, shape) else { continue };
        let session = e.stage.session_layer().and_then(|l| l.export_to_string().ok()).unwrap_or_default();
        let session = if session_is_empty(&session) { String::new() } else { session };
        let mut dirty = serde_json::Map::new();
        for id in dirty_layers(&e) {
            if id == e.session_identifier {
                continue;
            }
            if let Some(text) = e.stage.layer(&id).and_then(|l| l.export_to_string().ok()) {
                let key = if is_anon_root(&e, &id) { "root".to_string() } else { id.clone() };
                dirty.insert(key, json!(text));
                named.push(id);
            }
        }
        // An anonymous root has no file, so it always travels with the scene.
        if e.file.is_empty() && !dirty.contains_key("root") {
            if let Ok(text) = e.stage.root_layer().export_to_string() {
                dirty.insert("root".to_string(), json!(text));
            }
        }
        let text = if dirty.is_empty() { String::new() } else { J::Object(dirty).to_string() };
        remember_state(&mut s.graph, shape, &e);
        s.graph.without_undo(|g| {
            set_attr(g, shape, "serializedSessionLayer", PlugValue::String(session));
            set_attr(g, shape, "serializedDirtyLayers", PlugValue::String(text));
        });
    }
    named
}

fn session_is_empty(text: &str) -> bool {
    !text.lines().skip(1).any(|l| !l.trim().is_empty())
}

// the Animal Logic USD plug-in's names for the same things.

fn al_create_layer(s: &mut Session, a: &Args) -> CmdResult {
    let proxy = a.string("proxy", "");
    let (_, e) = entry_for(s, Some(&proxy)).map_err(err)?;
    let root = e.stage.root_layer().identifier().to_string();
    let open = a.string("open", "");
    if open.is_empty() {
        return add_anonymous(s, &e, &root, "").map(|x| json!(x)).map_err(err);
    }
    insert_sublayer(s, &e, &root, 0, &open).map(|x| json!(x)).map_err(err)
}

fn al_edit_target(s: &mut Session, a: &Args) -> CmdResult {
    let proxy = a.string("proxy", "");
    let (shape, e) = entry_for(s, Some(&proxy)).map_err(err)?;
    if a.bool("query") {
        return Ok(json!(e.stage.edit_target().layer_identifier()));
    }
    let layer = a.string("layer", "");
    set_target(s, shape, &e, &layer).map_err(err)?;
    Ok(json!(layer))
}

fn al_get_layers(s: &mut Session, a: &Args) -> CmdResult {
    let proxy = a.string("proxy", "");
    let (_, e) = entry_for(s, Some(&proxy)).map_err(err)?;
    if a.bool("rootLayer") {
        return Ok(json!(e.stage.root_layer().identifier()));
    }
    if a.bool("sessionLayer") {
        return Ok(json!(e.session_identifier));
    }
    if a.bool("muted") {
        return Ok(json!(e.stage.muted_layers()));
    }
    Ok(json!(e.stage.layer_identifiers()))
}

fn al_save(s: &mut Session, a: &Args) -> CmdResult {
    let layer = a.string("layer", "");
    let (shape, e) = entry_with_layer(s, &layer, Some(&a.string("proxy", ""))).map_err(err)?;
    let id = if layer.is_empty() { e.stage.root_layer().identifier().to_string() } else { layer };
    if a.has("string") && a.bool("string") {
        let text = e.stage.layer(&id).ok_or_else(|| err(format!("no layer {id}")))?.export_to_string().map_err(err)?;
        return Ok(json!(text));
    }
    let f = a.string("filename", "");
    save(s, shape, &e, &id, if f.is_empty() { None } else { Some(f.as_str()) }).map(|x| json!(x)).map_err(err)
}

pub fn register(c: &mut Commands<Session>) {
    let f = |n: &str, sn: &str, ty: ArgType, d: &str| FlagDef::new(n, sn, ty, J::Null, d);
    c.register(
        CommandDef::new("mayaUsdLayerEditor", "Edit a layer of a stage, named by identifier. Flags as the reference's USD plug-in's: sublayers, clearing, discarding edits, muting, locking.", run_layer_editor)
            .positional("layer identifier")
            .flag(f("insertSubPath", "is", ArgType::Any, "A position and a file to add as a sublayer.").multi(2))
            .flag(f("removeSubPath", "rs", ArgType::Any, "A position, and the proxy.").multi(2))
            .flag(f("addAnonymous", "aa", ArgType::String, "Add a new anonymous sublayer of this name."))
            .flag(f("discardEdits", "de", ArgType::Bool, "Back to the file's contents."))
            .flag(f("clear", "cl", ArgType::Bool, "Empty the layer."))
            .flag(f("flatten", "fl", ArgType::Bool, "Compose everything into the root layer. Less than the reference's USD plug-in: references and payloads flatten too."))
            .flag(f("muteLayer", "mt", ArgType::Any, "On or off, and the proxy.").multi(2))
            .flag(f("lockLayer", "lk", ArgType::Any, "0 unlocked or 1 locked, and the proxy.").multi(2))
            .flag(f("skipSystemLocked", "ssl", ArgType::Bool, "Accepted."))
            .undoable(true),
    );
    c.register(
        CommandDef::new("mayaUsdEditTarget", "The layer edits go to. -q -editTarget reads it; -e -editTarget layer sets it.", run_edit_target)
            .positional("proxy")
            .flag(f("query", "q", ArgType::Bool, "Read."))
            .flag(f("edit", "e", ArgType::Bool, "Set."))
            .flag(f("editTarget", "et", ArgType::Any, "The layer's identifier.")),
    );
    c.register(
        CommandDef::new("usdLayer", "Layer work the reference's USD plug-in does in its editor: -q gives the layer tree as JSON; -save, -saveAs, -reload, -flattenTo, -saveDirty.", run_usd_layer)
            .positional("layer identifier")
            .flag(f("query", "q", ArgType::Bool, "The layer tree."))
            .flag(f("proxy", "p", ArgType::String, "The proxy shape."))
            .flag(f("save", "s", ArgType::Bool, "Write it to its file."))
            .flag(f("saveAs", "sa", ArgType::String, "Write it to this file."))
            .flag(f("reload", "rl", ArgType::Bool, "Back to the file's contents."))
            .flag(f("flattenTo", "ft", ArgType::String, "Write the composed stage to this file."))
            .flag(f("saveDirty", "sd", ArgType::Bool, "Write every unsaved layer that has a file."))
            .undoable(true),
    );
    c.register(
        CommandDef::new("aztekUsdBeforeSave", "What a scene save runs: proxies take in their session and unsaved layers. Returns the unsaved layers.", |s, _a| Ok(json!(before_save(s)))).undoable(false),
    );
    let p = || f("proxy", "p", ArgType::String, "The proxy shape.");
    c.register(CommandDef::new("AL_usdmaya_LayerCreateLayer", "Add a sublayer to the root layer: -open a file, or a new anonymous one.", al_create_layer).flag(p()).flag(f("open", "o", ArgType::String, "The file.")));
    c.register(CommandDef::new("AL_usdmaya_LayerCurrentEditTarget", "The layer edits go to: -q reads, -layer sets.", al_edit_target).flag(p()).flag(f("query", "q", ArgType::Bool, "Read.")).flag(f("layer", "l", ArgType::String, "The layer's identifier.")));
    c.register(
        CommandDef::new("AL_usdmaya_LayerGetLayers", "The stage's layers; -rootLayer, -sessionLayer or -muted narrow it.", al_get_layers)
            .flag(p())
            .flag(f("rootLayer", "rl", ArgType::Bool, "Only the root."))
            .flag(f("sessionLayer", "sl", ArgType::Bool, "Only the session."))
            .flag(f("muted", "m", ArgType::Bool, "Only muted ones.")),
    );
    c.register(
        CommandDef::new("AL_usdmaya_LayerSave", "Write a layer: to its file, to -filename, or as text with -string.", al_save)
            .flag(p())
            .flag(f("layer", "l", ArgType::String, "The layer's identifier; the root by default."))
            .flag(f("filename", "f", ArgType::String, "The file."))
            .flag(f("string", "s", ArgType::Bool, "Return its text.")),
    );
}
