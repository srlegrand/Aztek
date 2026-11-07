//! The proxy shape: a stage in the DAG with no nodes per prim. `mayaUsdProxyShape` has the reference's USD plug-in's attributes;
//! `AL_usdmaya_ProxyShape` answers to the Animal Logic USD plug-in's names for the same thing.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::dag;
use aztek_scene::graph::SelectMode;
use aztek_scene::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use aztek_scene::plug::{flags, AttrDef, PlugType, PlugValue, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::transform::Shape;
use serde_json::{json, Value as J};

use crate::util::*;

pub const BASE_TYPE: &str = "mayaUsdProxyShapeBase";
pub const TYPE: &str = "mayaUsdProxyShape";
pub const AL_TYPE: &str = "AL_usdmaya_ProxyShape";

struct Proxy;

impl NodeBehavior for Proxy {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

const STORE: u32 = flags::INPUT | flags::STORABLE;

fn text(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: STORE, doc: doc.into(), ..Default::default() }
}
fn file(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef { flags: STORE | flags::FILE, ..text(name, short, doc) }
}
fn on(name: &str, short: &str, d: bool, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Bool, default_value: PlugValue::Bool(d), flags: STORE, doc: doc.into(), ..Default::default() }
}
fn int(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(-1), flags: fl, doc: doc.into(), ..Default::default() }
}
fn time(name: &str, short: &str, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Float, default_value: PlugValue::Float(0.0), flags: fl, doc: doc.into(), ..Default::default() }
}

/// Registers the node types. Safe to call again.
pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(make_types);
}

fn make_types() {
    let shape = types::find("shape").expect("shape");
    let create = Some(Arc::new(|| Box::new(Proxy) as Box<dyn NodeBehavior>) as aztek_scene::node::CreateFn);
    let base = types::register_type(NodeType {
        name: BASE_TYPE.into(),
        base: Some(shape),
        dag: true,
        time_dependent: true,
        icon: "box".into(),
        doc: "A USD stage in the scene: its prims draw and can be edited with no node per prim.".into(),
        own_attrs: vec![
            file("filePath", "fp", "The stage's root layer. Empty for an anonymous one."),
            text("primPath", "pp", "The prim to show with what's below it; empty for the whole stage."),
            text("excludePrimPaths", "epp", "Prims to leave out, separated by commas."),
            time("time", "tm", STORE | flags::KEYABLE, "The stage's time code, when connected; otherwise the scene's frame stands."),
            time("outTime", "otm", flags::OUTPUT, "The time the stage shows."),
            on("loadPayloads", "lpl", true, "Load payloads on opening."),
            on("shareStage", "scmp", true, "Proxies of one file share a stage. Off gives this one its own."),
            int("stageCacheId", "stcid", STORE, "The open stage's id."),
            int("outStageCacheId", "ostcid", flags::OUTPUT, "The open stage's id."),
            AttrDef { name: "outStageData".into(), short_name: "od".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::OUTPUT | flags::HIDDEN, doc: "The stage, to other nodes.".into(), ..Default::default() },
            AttrDef { name: "inStageData".into(), short_name: "isd".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::INPUT | flags::HIDDEN, doc: "A stage from another node.".into(), ..Default::default() },
            on("drawRenderPurpose", "drp", false, "Draw prims of render purpose."),
            on("drawProxyPurpose", "dpp", true, "Draw prims of proxy purpose."),
            on("drawGuidePurpose", "dgp", false, "Draw prims of guide purpose."),
            AttrDef { name: "complexity".into(), short_name: "cplx".into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: STORE, doc: "Accepted; meshes draw as authored.".into(), ..Default::default() },
            text("rootLayerName", "rln", "The root layer's identifier."),
            text("sessionLayerName", "sln", "The session layer's identifier."),
            AttrDef { flags: STORE | flags::HIDDEN | flags::MULTILINE, ..text("serializedSessionLayer", "ssl", "The session layer's text, kept with the scene.") },
            AttrDef { flags: STORE | flags::HIDDEN | flags::MULTILINE, ..text("serializedDirtyLayers", "sdl", "Unsaved layers, kept with the scene as JSON of identifier and text.") },
            text("editTargetLayer", "etl", "The layer edits go to, when not the root layer."),
            text("mutedLayers", "mtl", "Muted layers, separated by commas."),
            text("lockedLayers", "lkl", "Locked layers, separated by commas."),
        ],
        create: create.clone(),
        ..Default::default()
    })
    .expect("proxy base");
    types::register_type(NodeType { name: TYPE.into(), base: Some(base.clone()), dag: true, time_dependent: true, icon: "box".into(), doc: "A USD stage in the scene.".into(), own_attrs: Vec::new(), create: create.clone(), ..Default::default() }).expect("proxy");
    types::register_type(NodeType {
        name: AL_TYPE.into(),
        base: Some(base),
        dag: true,
        time_dependent: true,
        icon: "box".into(),
        doc: "A USD stage in the scene, as the Animal Logic USD plug-in names it.".into(),
        own_attrs: vec![
            on("unloaded", "ul", false, "Leave payloads unloaded on opening."),
            on("displayGuides", "dg", false, "Draw prims of guide purpose."),
            on("displayRenderGuides", "drg", false, "Draw prims of render purpose."),
        ],
        create,
        ..Default::default()
    })
    .expect("AL proxy");
}

/// A transform and proxy shape, as `Create USD Stage` makes them.
pub fn create_stage(s: &mut Session, shape_type: &str, name: &str, file: &str, prim: &str) -> Result<(aztek_scene::plug::NodeId, aztek_scene::plug::NodeId), String> {
    let (tr, sh) = dag::create_with_shape(&mut s.graph, shape_type, name, NO_NODE).map_err(|e| e.to_string())?;
    let g = &mut s.graph;
    if !file.is_empty() {
        set_attr(g, sh, "filePath", PlugValue::String(file.to_string()));
    }
    if !prim.is_empty() {
        set_attr(g, sh, "primPath", PlugValue::String(prim.to_string()));
    }
    g.select(&[tr], SelectMode::Replace);
    Ok((tr, sh))
}

/// The name for a new stage's transform: the file's stem, or `stage1`.
fn stem(file: &str) -> String {
    let st = std::path::Path::new(file).file_stem().and_then(|x| x.to_str()).unwrap_or("");
    if st.is_empty() {
        "stage1".to_string()
    } else {
        format!("{}1", node_name(st))
    }
}

fn from_file(s: &mut Session, a: &Args) -> CmdResult {
    let file = a.pos_str(0).map(str::to_string).unwrap_or_else(|| a.string("file", ""));
    if file.is_empty() {
        fail!("mayaUsd_createStageFromFile: a file is required");
    }
    if !std::path::Path::new(&file).exists() {
        fail!("mayaUsd_createStageFromFile: no file {file}");
    }
    let name = stem(&file);
    let (_, sh) = create_stage(s, TYPE, &name, &file, "").map_err(aztek_ops::CommandError::new)?;
    Ok(json!(s.graph.path(sh)))
}

fn with_new_layer(s: &mut Session, _a: &Args) -> CmdResult {
    let (_, sh) = create_stage(s, TYPE, "stage1", "", "").map_err(aztek_ops::CommandError::new)?;
    crate::stages::entry_of(&mut s.graph, sh).map_err(aztek_ops::CommandError::new)?;
    Ok(json!(s.graph.path(sh)))
}

/// AL_usdmaya_ProxyShapeImport: -file and -name, optionally under transforms given as arguments. Returns the shapes.
fn al_import(s: &mut Session, a: &Args) -> CmdResult {
    let file = a.string("file", "");
    let id = a.int("stageId", 0);
    if file.is_empty() && id == 0 {
        fail!("AL_usdmaya_ProxyShapeImport: -file or -stageId is required");
    }
    let name = a.string("name", "");
    let name = if name.is_empty() { "AL_usdmaya_Proxy1".to_string() } else { name };
    let (tr, sh) = create_stage(s, AL_TYPE, &name, &file, &a.string("primPath", "")).map_err(aztek_ops::CommandError::new)?;
    let g = &mut s.graph;
    if a.has("excludePrimPath") {
        set_attr(g, sh, "excludePrimPaths", PlugValue::String(a.string("excludePrimPath", "")));
    }
    if a.get("unloaded").is_some_and(|v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0)) {
        set_attr(g, sh, "unloaded", PlugValue::Bool(true));
        set_attr(g, sh, "loadPayloads", PlugValue::Bool(false));
    }
    if let Some(name) = a.pos_str(0) {
        if let Some(p) = g.find(name).map_err(|e| aztek_ops::CommandError::new(e.to_string()))? {
            g.set_parent(tr, p, false, usize::MAX).map_err(|e| aztek_ops::CommandError::new(e.to_string()))?;
        }
    }
    Ok(json!([g.path(sh)]))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    let f = |n: &str, sn: &str, ty: ArgType, d: &str| FlagDef::new(n, sn, ty, J::Null, d);
    c.register(CommandDef::new("mayaUsd_createStageFromFile", "A proxy shape on an existing USD file. Returns the shape.", from_file).positional("file").flag(f("file", "f", ArgType::String, "The file.")));
    c.register(CommandDef::new("mayaUsd_createStageWithNewLayer", "A proxy shape on a new anonymous layer. Returns the shape.", with_new_layer));
    c.register(
        CommandDef::new("AL_usdmaya_ProxyShapeImport", "A proxy shape on a USD file, as the Animal Logic USD plug-in makes one. Returns the shapes.", al_import)
            .positional("transform to go under")
            .flag(f("file", "f", ArgType::String, "The file."))
            .flag(f("name", "n", ArgType::String, "The transform's name."))
            .flag(f("primPath", "pp", ArgType::String, "The prim to show."))
            .flag(f("excludePrimPath", "epp", ArgType::String, "Prims to leave out."))
            .flag(f("unloaded", "ul", ArgType::Any, "Leave payloads unloaded."))
            .flag(f("stageId", "sid", ArgType::Int, "An open stage's id.")),
    );
}
