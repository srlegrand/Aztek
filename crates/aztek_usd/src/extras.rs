//! Cameras from prims, statistics, collections, an asset resolver with search paths and a hook, and schema translators from
//! scripts.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::io;
use std::sync::Arc;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::graph::Graph;
use aztek_scene::node::{types, NodeType};
use aztek_scene::plug::{flags, AttrDef, NodeId, PlugType, PlugValue, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::transform::Transform;
use openusd::ar::{Asset, DefaultResolver, ResolvedPath, Resolver};
use openusd::tf::Token;
use openusd::usd::PrimPredicate;
use serde_json::{json, Map, Value as J};

use crate::mesh_read::{f64_of, token_of};
use crate::prims::{self, err, resolve};
use crate::stages;
use crate::util::*;

pub const CAMERA_TYPE: &str = "ProxyUsdGeomCamera";

// ---------------------------------------------------------------------------------------------------------------------
// Cameras

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        aztek_scene::cameras::register_types();
        let base = types::find("camera").expect("camera");
        let text = |n: &str, s: &str, d: &str| AttrDef { name: n.into(), short_name: s.into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: flags::INPUT | flags::STORABLE, doc: d.into(), ..Default::default() };
        types::register_type(NodeType {
            name: CAMERA_TYPE.into(),
            create: base.create.clone(),
            base: Some(base),
            dag: true,
            icon: "camera".into(),
            doc: "A camera following a Camera prim, to look through.".into(),
            own_attrs: vec![text("stage", "stg", "The proxy shape."), text("path", "pth", "The Camera prim.")],
            ..Default::default()
        })
        .expect("camera proxy");
    });
}

fn run_camera(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).map(str::to_string).or_else(|| prims::selected().last().cloned()).ok_or_else(|| err("usdCamera: name a Camera prim"))?;
    let t = resolve(&mut s.graph, &name).map_err(err)?;
    let prim = t.entry.stage.prim(t.path.clone()).map_err(err)?;
    if prim.type_name().ok().flatten().is_none_or(|x| x.as_str() != "Camera") {
        return Err(err(format!("{name} is not a Camera prim")));
    }
    let leaf = node_name(t.path.name().unwrap_or("camera"));
    let (tr, sh) = aztek_scene::dag::create_with_shape(&mut s.graph, CAMERA_TYPE, &format!("{leaf}Proxy"), NO_NODE).map_err(err)?;
    let shape_name = s.graph.full_path(t.shape);
    set_attr(&mut s.graph, sh, "stage", PlugValue::String(shape_name));
    set_attr(&mut s.graph, sh, "path", PlugValue::String(t.path.as_str().to_string()));
    follow(&mut s.graph, sh);
    if a.bool("lookThrough") && s.commands.find("lookThru").is_some() {
        let n = s.graph.path(tr);
        s.run("lookThru", json!({"args": [n]}))?;
    }
    Ok(json!(s.graph.path(tr)))
}

fn follow(g: &mut Graph, sh: NodeId) {
    let proxy = get_str(g, sh, "stage").unwrap_or_default();
    let path = get_str(g, sh, "path").unwrap_or_default();
    let name = format!("{proxy},{path}");
    let Ok(t) = resolve(g, &name) else { return };
    let time = stages::time_of(g, t.shape);
    let world = prims::world_of(g, &t, time);
    let Ok(prim) = t.entry.stage.prim(t.path.clone()) else { return };
    const INCH: f64 = 25.4;
    let up = g.parent(sh);
    g.without_undo(|g| {
        let _ = Transform::set_local_matrix(g, up, &world);
        let ortho = token_of(&prim, "projection").as_deref() == Some("orthographic");
        set_attr(g, sh, "orthographic", PlugValue::Bool(ortho));
        let ha = f64_of(&prim, "horizontalAperture", Some(time)).unwrap_or(20.955);
        let va = f64_of(&prim, "verticalAperture", Some(time)).unwrap_or(15.2908);
        if ortho {
            set_attr(g, sh, "orthographicWidth", PlugValue::Float(ha / 10.0));
        } else {
            set_attr(g, sh, "horizontalFilmAperture", PlugValue::Float(ha / INCH));
            set_attr(g, sh, "verticalFilmAperture", PlugValue::Float(va / INCH));
        }
        if let Some(v) = f64_of(&prim, "focalLength", Some(time)) {
            set_attr(g, sh, "focalLength", PlugValue::Float(v));
        }
        if let Ok(Some(r)) = prim.attribute("clippingRange").get_at::<openusd::gf::Vec2f>(openusd::usd::TimeCode::new(time)) {
            set_attr(g, sh, "nearClipPlane", PlugValue::Float(r.x as f64));
            set_attr(g, sh, "farClipPlane", PlugValue::Float(r.y as f64));
        }
    });
}

thread_local! {
    static CAMERAS: RefCell<(u64, Vec<NodeId>)> = const { RefCell::new((u64::MAX, Vec::new())) };
}

/// Cameras following prims follow them. The list of them is looked for only when the scene's nodes changed.
pub fn follow_cameras(g: &mut Graph) {
    let shape = g.structure();
    let list = CAMERAS.with(|c| {
        let mut c = c.borrow_mut();
        if c.0 != shape {
            c.0 = shape;
            c.1 = g.nodes().into_iter().filter(|&n| g.node(n).is_some_and(|x| x.ty().name == CAMERA_TYPE)).collect();
        }
        c.1.clone()
    });
    for sh in list {
        follow(g, sh);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Statistics and collections

fn run_stats(s: &mut Session, a: &Args) -> CmdResult {
    let proxy = a.pos_str(0).unwrap_or("").to_string();
    let shape = stages::proxy_named(&s.graph, &proxy).ok_or_else(|| err(format!("no proxy shape named '{proxy}'")))?;
    let e = stages::entry_of(&mut s.graph, shape).map_err(err)?;
    let (mut total, mut active, mut instances, mut models) = (0u64, 0u64, 0u64, 0u64);
    let mut by_type: BTreeMap<String, u64> = BTreeMap::new();
    let mut paths = Vec::new();
    e.stage.traverse(PrimPredicate::ALL, |p| paths.push(p.clone())).map_err(err)?;
    for p in paths {
        let Ok(pr) = e.stage.prim(p) else { continue };
        total += 1;
        if pr.is_active().unwrap_or(true) {
            active += 1;
        }
        if pr.is_instance().unwrap_or(false) {
            instances += 1;
        }
        if pr.is_model().unwrap_or(false) {
            models += 1;
        }
        let ty = pr.type_name().ok().flatten().map_or_else(|| "(none)".to_string(), |x| if x.as_str().is_empty() { "(none)".to_string() } else { x.as_str().to_string() });
        *by_type.entry(ty).or_default() += 1;
    }
    let mut layers = Vec::new();
    for id in e.stage.layer_identifiers() {
        let path = e.stage.layer(&id).and_then(|l| l.resolved_path().map(str::to_string));
        let size = path.as_ref().and_then(|p| std::fs::metadata(p).ok()).map_or(0, |m| m.len());
        let specs = e.stage.layer(&id).map_or(0, |l| l.data().spec_paths().len());
        layers.push(json!({"identifier": id, "bytes": size, "specs": specs, "dirty": e.is_dirty(&id), "muted": e.stage.is_layer_muted(&id)}));
    }
    let prototypes = e.stage.prototypes().map_or(0, |v| v.len());
    Ok(json!({"prims": total, "active": active, "instances": instances, "models": models, "prototypes": prototypes, "byType": by_type, "layers": layers}))
}

/// A prim's collections, each with what it includes among the stage's prims, by its expansion rule.
fn run_collections(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).ok_or_else(|| err("usdCollections: name a prim"))?.to_string();
    let t = resolve(&mut s.graph, &name).map_err(err)?;
    let prim = t.entry.stage.prim(t.path.clone()).map_err(err)?;
    let mut names: Vec<String> = Vec::new();
    for p in prim.property_names().unwrap_or_default() {
        let n = p.as_str();
        if let Some(rest) = n.strip_prefix("collection:") {
            let c = rest.split(':').next().unwrap_or("").to_string();
            if !c.is_empty() && !names.contains(&c) {
                names.push(c);
            }
        }
    }
    let mut paths = Vec::new();
    t.entry.stage.traverse(PrimPredicate::DEFAULT, |p| paths.push(p.as_str().to_string())).map_err(err)?;
    let mut out = Vec::new();
    for c in names {
        let includes = prim.relationship(format!("collection:{c}:includes").as_str()).targets().unwrap_or_default();
        let excludes = prim.relationship(format!("collection:{c}:excludes").as_str()).targets().unwrap_or_default();
        let rule = prim.attribute(format!("collection:{c}:expansionRule").as_str()).get::<Token>().ok().flatten().map_or_else(|| "expandPrims".to_string(), |x| x.as_str().to_string());
        let under = |x: &str, root: &str| x == root || (x.starts_with(root) && x.as_bytes().get(root.len()) == Some(&b'/'));
        let members: Vec<&String> = paths
            .iter()
            .filter(|x| {
                let inside = if rule == "explicitOnly" { includes.iter().any(|i| i.as_str() == x.as_str()) } else { includes.iter().any(|i| under(x, i.as_str())) };
                inside && !excludes.iter().any(|i| under(x, i.as_str()))
            })
            .collect();
        out.push(json!({"name": c, "rule": rule, "members": members}));
    }
    Ok(J::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// The asset resolver

pub type ResolveHook = Arc<dyn Fn(&str) -> Option<String>>;

thread_local! {
    static SEARCH_PATHS: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
    static MAP: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
    static HOOK: RefCell<Option<ResolveHook>> = const { RefCell::new(None) };
}

pub fn set_resolve_hook(h: Option<ResolveHook>) {
    HOOK.with(|c| *c.borrow_mut() = h);
}

/// What a stage opens assets through: the map first, then a script's hook, then the search paths.
pub struct AztekResolver {
    inner: DefaultResolver,
}

impl AztekResolver {
    pub fn new() -> Self {
        let mut paths: Vec<String> = SEARCH_PATHS.with(|c| c.borrow().clone());
        if let Ok(v) = std::env::var("PXR_AR_DEFAULT_SEARCH_PATH") {
            paths.extend(std::env::split_paths(&v).map(|p| p.to_string_lossy().into_owned()));
        }
        AztekResolver { inner: DefaultResolver::with_search_paths(paths) }
    }

    fn mapped(&self, asset: &str) -> Option<String> {
        if let Some(m) = MAP.with(|c| c.borrow().get(asset).cloned()) {
            return Some(m);
        }
        let hook = HOOK.with(|c| c.borrow().clone());
        hook.and_then(|h| h(asset))
    }
}

impl Default for AztekResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl Resolver for AztekResolver {
    fn create_identifier(&self, asset_path: &str, anchor: Option<&ResolvedPath>) -> String {
        if self.mapped(asset_path).is_some() {
            return asset_path.to_string();
        }
        self.inner.create_identifier(asset_path, anchor)
    }
    fn resolve(&self, asset_path: &str) -> Option<ResolvedPath> {
        match self.mapped(asset_path) {
            Some(m) => self.inner.resolve(&m),
            None => self.inner.resolve(asset_path),
        }
    }
    fn resolve_for_new_asset(&self, asset_path: &str) -> Option<ResolvedPath> {
        self.inner.resolve_for_new_asset(asset_path)
    }
    fn open_asset(&self, resolved_path: &ResolvedPath) -> io::Result<Box<dyn Asset>> {
        self.inner.open_asset(resolved_path)
    }
}

fn run_resolver(_s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let map: Map<String, J> = MAP.with(|c| c.borrow().iter().map(|(k, v)| (k.clone(), json!(v))).collect());
        return Ok(json!({"searchPaths": SEARCH_PATHS.with(|c| c.borrow().clone()), "map": map}));
    }
    if let Some(p) = a.get("addSearchPath").and_then(J::as_str) {
        SEARCH_PATHS.with(|c| c.borrow_mut().push(p.to_string()));
    }
    if a.bool("clearSearchPaths") {
        SEARCH_PATHS.with(|c| c.borrow_mut().clear());
    }
    if let Some(J::Array(v)) = a.get("map") {
        if let (Some(k), Some(x)) = (v.first().and_then(J::as_str), v.get(1).and_then(J::as_str)) {
            MAP.with(|c| c.borrow_mut().insert(k.to_string(), x.to_string()));
        }
    }
    Ok(J::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// Schema translators

thread_local! {
    static TRANSLATORS: RefCell<HashMap<String, String>> = RefCell::new(HashMap::new());
}

/// The script reading prims of a type into nodes, if one is registered.
pub fn translator_for(type_name: &str) -> Option<String> {
    TRANSLATORS.with(|c| c.borrow().get(type_name).cloned())
}

/// Runs a registered translator for a prim, under a node. True when one ran.
pub fn run_translator(s: &mut Session, type_name: &str, file: &str, prim: &str, parent: &str) -> bool {
    let Some(code) = translator_for(type_name) else { return false };
    let q = |x: &str| x.replace('\\', "\\\\").replace('\'', "\\'");
    let call = code.replace("{file}", &q(file)).replace("{prim}", &q(prim)).replace("{parent}", &q(parent));
    if let Err(e) = s.run("aztekPython", json!({"args": [call]})) {
        log::warn!("USD: translator for {type_name}: {e}");
    }
    true
}

fn run_register_translator(_s: &mut Session, a: &Args) -> CmdResult {
    let ty = a.string("type", "");
    if a.bool("remove") {
        TRANSLATORS.with(|c| c.borrow_mut().remove(&ty));
        return Ok(J::Null);
    }
    if a.bool("query") {
        return Ok(json!(TRANSLATORS.with(|c| c.borrow().keys().cloned().collect::<Vec<_>>())));
    }
    TRANSLATORS.with(|c| c.borrow_mut().insert(ty, a.string("command", "")));
    Ok(J::Null)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    let f = |n: &str, sn: &str, ty: ArgType, d: &str| FlagDef::new(n, sn, ty, J::Null, d);
    c.register(
        CommandDef::new("usdCamera", "A camera following a Camera prim: its transform and lens read from the stage at the current time. -lookThrough makes the view use it. Returns the transform.", run_camera)
            .positional("proxy,/path")
            .flag(f("lookThrough", "lt", ArgType::Bool, "Look through it.")),
    );
    c.register(CommandDef::new("usdStats", "A stage's numbers as JSON: prims by type, instances, models, prototypes, layers and their sizes.", run_stats).positional("proxy").undoable(false));
    c.register(CommandDef::new("usdCollections", "A prim's collections as JSON: name, rule, members.", run_collections).positional("proxy,/path").undoable(false));
    c.register(
        CommandDef::new("usdResolver", "How assets resolve for stages opened from now on: -addSearchPath, -map logical real; -q reads them. A Python hook goes through pxr.Ar.SetResolveHook.", run_resolver)
            .flag(f("query", "q", ArgType::Bool, "Read."))
            .flag(f("addSearchPath", "asp", ArgType::String, "A directory to search."))
            .flag(f("clearSearchPaths", "csp", ArgType::Bool, "Forget them."))
            .flag(f("map", "m", ArgType::Any, "A logical asset path and the file it stands for.").multi(2))
            .undoable(false),
    );
    c.register(
        CommandDef::new("usdRegisterTranslator", "A script reads prims of a type into nodes on import and pull. The command is Python with {file}, {prim} and {parent} replaced.", run_register_translator)
            .flag(f("type", "t", ArgType::String, "The prim type."))
            .flag(f("command", "c", ArgType::String, "The Python to run."))
            .flag(f("remove", "rm", ArgType::Bool, "Unregister it."))
            .flag(f("query", "q", ArgType::Bool, "The types registered."))
            .undoable(false),
    );
    c.register(CommandDef::new("AL_usdmaya_ListEvents", "The events USD work raises, for scriptJob.", |_s, _a| Ok(json!(["usdStageChanged", "usdLayerChanged", "usdEditTargetChanged"]))).undoable(false));
}
