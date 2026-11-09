//! mayaUSDExport: the scene to a USD file. Flags and defaults follow the reference's USD plug-in's command reference.

use std::collections::{BTreeMap, HashMap, HashSet};

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::anim;
use aztek_scene::graph::Graph;
use aztek_scene::plug::{NodeId, NO_NODE};
use aztek_scene::rig;
use aztek_scene::session::Session;
use glam::{DMat4, DVec3};
use openusd::sdf::{Specifier, Value};
use openusd::tf::Token;
use serde_json::{json, Value as J};

use crate::out::{self, Out, Track};
use crate::util::*;

#[derive(Clone, Debug)]
pub struct Opts {
    pub file: String,
    pub default_format: String,
    pub selection: bool,
    pub export_roots: Vec<String>,
    pub export_uvs: bool,
    pub export_color_sets: bool,
    pub export_display_color: bool,
    pub export_visibility: bool,
    pub export_instances: bool,
    pub export_blend_shapes: bool,
    pub export_materials: bool,
    pub assigned_only: bool,
    pub skels: String,
    pub skin: String,
    pub shading_mode: String,
    pub merge: bool,
    pub strip_namespaces: bool,
    pub euler_filter: bool,
    pub worldspace: bool,
    pub static_single: bool,
    pub default_cameras: bool,
    pub include_empty: bool,
    pub preserve_uv_names: bool,
    pub default_scheme: String,
    pub materials_scope: String,
    pub root_prim: String,
    pub root_prim_type: String,
    pub default_prim: Option<String>,
    pub kind: String,
    pub frames: Vec<f64>,
    pub relative_textures: String,
    pub filter_types: Vec<String>,
    pub exclude_types: Vec<String>,
    pub remap_uv: Vec<(String, String)>,
    pub custom_layer_data: Vec<(String, String, String)>,
    pub meters_per_unit: f64,
    pub up_axis: String,
    pub out_dir: String,
    pub stages_as_refs: bool,
}

/// One prim's attribute: its type and value over the frames, plus the fields written beside it.
#[derive(Default)]
pub struct Tracked {
    pub ty: String,
    pub track: Track,
    pub interpolation: Option<String>,
    pub element_size: i32,
    pub custom: bool,
    pub uniform: bool,
}

/// Every sampled attribute, by prim then name, in the order first seen.
#[derive(Default)]
pub struct Tracks {
    pub map: BTreeMap<String, Vec<(String, Tracked)>>,
}

impl Tracks {
    pub fn slot(&mut self, prim: &str, name: &str, ty: &str) -> &mut Tracked {
        let list = self.map.entry(prim.to_string()).or_default();
        let i = match list.iter().position(|(n, _)| n == name) {
            Some(i) => i,
            None => {
                list.push((name.to_string(), Tracked { ty: ty.to_string(), ..Default::default() }));
                list.len() - 1
            }
        };
        &mut list[i].1
    }
    /// A value at a frame, or the default when not sampling.
    pub fn put(&mut self, prim: &str, name: &str, ty: &str, time: Option<f64>, v: Value) {
        let t = self.slot(prim, name, ty);
        match time {
            Some(f) => t.track.samples.push((f, v)),
            None => t.track.default = Some(v),
        }
    }
    pub fn take(&mut self, prim: &str, name: &str) -> Option<Tracked> {
        let list = self.map.get_mut(prim)?;
        let i = list.iter().position(|(n, _)| n == name)?;
        Some(list.remove(i).1)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Group,
    Mesh,
    Camera,
    Curve,
    Surface,
    Proxy,
}

#[derive(Clone, Debug)]
pub struct Item {
    pub xform: NodeId,
    pub shape: NodeId,
    /// The prim holding the transform.
    pub path: String,
    /// The prim holding the shape: the same prim when merged.
    pub shape_path: String,
    pub kind: Kind,
    /// The shape draws another mesh's data: its prim references the prototype.
    pub instance_of: Option<NodeId>,
    pub is_root: bool,
}

fn multi(a: &Args, name: &str) -> Vec<String> {
    match a.get(name) {
        Some(J::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(J::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

/// `-frameRange a b`, `-frameStride`, `-frameSample` and `-extraFrame`: the sorted times to write. Empty for a static export.
fn frames_of(a: &Args) -> Vec<f64> {
    let Some(J::Array(r)) = a.get("frameRange") else { return Vec::new() };
    let (Some(start), Some(end)) = (r.first().and_then(J::as_f64), r.get(1).and_then(J::as_f64)) else { return Vec::new() };
    if end <= start && !a.has("extraFrame") {
        return Vec::new();
    }
    let stride = a.float("frameStride", 1.0).max(1e-3);
    let offsets: Vec<f64> = match a.get("frameSample") {
        Some(J::Array(v)) => v.iter().filter_map(J::as_f64).collect(),
        Some(J::Number(n)) => n.as_f64().into_iter().collect(),
        _ => vec![0.0],
    };
    let mut out = Vec::new();
    let mut f = start;
    while f <= end + 1e-9 {
        for o in &offsets {
            out.push(f + o);
        }
        f += stride;
    }
    match a.get("extraFrame") {
        Some(J::Array(v)) => out.extend(v.iter().filter_map(J::as_f64)),
        Some(J::Number(n)) => out.extend(n.as_f64()),
        _ => {}
    }
    out.sort_by(f64::total_cmp);
    out.dedup_by(|x, y| (*x - *y).abs() < 1e-9);
    out
}

fn opts_of(a: &Args, s: &Session) -> CmdResult<Opts> {
    let file = a.string("file", "");
    let file = if file.is_empty() { a.pos_str(0).unwrap_or("").to_string() } else { file };
    if file.is_empty() {
        fail!("mayaUSDExport: -file is required");
    }
    let flag = |n: &str, d: bool| a.get(n).map_or(d, |v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    let abs = std::path::absolute(&file).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| file.clone());
    let out_dir = std::path::Path::new(&abs).parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default();
    let pairs = |n: &str, k: usize| -> Vec<Vec<String>> { multi(a, n).chunks(k).filter(|c| c.len() == k).map(<[String]>::to_vec).collect() };
    let up = a.string("upAxis", "mayaPrefs");
    let scene_up = if s.graph.up_axis == aztek_scene::mesh_nodes::UpAxis::Z { "Z" } else { "Y" };
    Ok(Opts {
        file: abs,
        default_format: a.string("defaultUSDFormat", "usdc"),
        selection: a.bool("selection"),
        export_roots: multi(a, "exportRoots"),
        export_uvs: flag("exportUVs", true),
        export_color_sets: flag("exportColorSets", true),
        export_display_color: flag("exportDisplayColor", false),
        export_visibility: flag("exportVisibility", true),
        export_instances: flag("exportInstances", true),
        export_blend_shapes: flag("exportBlendShapes", false),
        export_materials: flag("exportMaterials", true),
        assigned_only: flag("exportAssignedMaterials", true),
        skels: a.string("exportSkels", "none"),
        skin: a.string("exportSkin", "none"),
        shading_mode: a.string("shadingMode", "useRegistry"),
        merge: flag("mergeTransformAndShape", true),
        strip_namespaces: flag("stripNamespaces", false),
        euler_filter: flag("eulerFilter", false),
        worldspace: flag("worldspace", false),
        static_single: flag("staticSingleSample", false),
        default_cameras: a.bool("defaultCameras"),
        include_empty: flag("includeEmptyTransforms", true),
        preserve_uv_names: flag("preserveUVSetNames", false),
        default_scheme: a.string("defaultMeshScheme", "catmullClark"),
        materials_scope: a.string("materialsScopeName", "Looks"),
        root_prim: { let r = a.string("rootPrim", ""); if r.is_empty() { a.string("parentScope", "") } else { r } },
        root_prim_type: a.string("rootPrimType", "Scope"),
        default_prim: a.get("defaultPrim").and_then(J::as_str).map(str::to_string),
        kind: a.string("kind", ""),
        frames: frames_of(a),
        relative_textures: a.string("exportRelativeTextures", "automatic"),
        filter_types: multi(a, "filterTypes"),
        exclude_types: multi(a, "excludeExportTypes"),
        remap_uv: pairs("remapUVSetsTo", 2).into_iter().map(|c| (c[0].clone(), c[1].clone())).collect(),
        custom_layer_data: pairs("customLayerData", 3).into_iter().map(|c| (c[0].clone(), c[1].clone(), c[2].clone())).collect(),
        meters_per_unit: a.float("metersPerUnit", 0.0),
        up_axis: match up.as_str() { "mayaPrefs" | "none" | "" => scene_up.to_string(), o => o.to_ascii_uppercase() },
        out_dir,
        stages_as_refs: flag("exportStagesAsRefs", true),
    })
}

const DEFAULT_CAMERAS: [&str; 4] = ["persp", "top", "front", "side"];

/// What the walk treats a shape as; None for a shape not written.
fn kind_of_shape(type_name: &str) -> Option<Kind> {
    match type_name {
        "mesh" => Some(Kind::Mesh),
        "camera" => Some(Kind::Camera),
        "nurbsCurve" => Some(Kind::Curve),
        "nurbsSurface" => Some(Kind::Surface),
        "mayaUsdProxyShape" | "AL_usdmaya_ProxyShape" => Some(Kind::Proxy),
        _ => None,
    }
}

fn excluded(o: &Opts, type_name: &str, k: Kind) -> bool {
    if o.filter_types.iter().any(|t| t == type_name) {
        return true;
    }
    let family = match k {
        Kind::Mesh | Kind::Curve | Kind::Surface => "Meshes",
        Kind::Camera => "Cameras",
        _ => "",
    };
    o.exclude_types.iter().any(|t| t == family)
}

pub(crate) fn shapes_of(g: &Graph, x: NodeId) -> Vec<NodeId> {
    g.children(x)
        .iter()
        .copied()
        .filter(|&c| {
            g.node(c).is_some_and(|n| {
                !n.ty().is_a("transform") && kind_of_shape(&n.ty().name).is_some() && !n.bool(aztek_scene::transform::DagNode::INTERMEDIATE_OBJECT)
            })
        })
        .collect()
}

/// Walks the DAG below `x`, one Item per prim.
fn walk(g: &Graph, o: &Opts, x: NodeId, parent_path: &str, plan: &crate::export_skel::Plan, seen_meshes: &mut HashMap<u64, NodeId>, out: &mut Vec<Item>, is_root: bool) {
    let Some(n) = g.node(x) else { return };
    if rig::is_joint(g, x) {
        // Written as a Skeleton where skeletons are on; other children of a joint still count.
        for &c in g.children(x) {
            if !rig::is_joint(g, c) && g.node(c).is_some_and(|k| k.ty().is_a("transform")) {
                walk(g, o, c, parent_path, plan, seen_meshes, out, false);
            }
        }
        return;
    }
    if !n.ty().is_a("transform") {
        return;
    }
    let parent = plan.parent_override.get(&x).cloned().unwrap_or_else(|| parent_path.to_string());
    let name = prim_name(n.name(), o.strip_namespaces);
    let path = format!("{parent}/{name}");
    let shapes = shapes_of(g, x);
    let usable: Vec<(NodeId, Kind)> = shapes
        .iter()
        .filter_map(|&c| {
            let tn = g.node(c)?.ty().name.clone();
            let k = kind_of_shape(&tn)?;
            (!excluded(o, &tn, k)).then_some((c, k))
        })
        .collect();
    let mut item = Item { xform: x, shape: NO_NODE, path: path.clone(), shape_path: path.clone(), kind: Kind::Group, instance_of: None, is_root };
    if usable.len() == 1 && o.merge {
        item.shape = usable[0].0;
        item.kind = usable[0].1;
        fill_instance(g, o, &mut item, seen_meshes);
        out.push(item);
    } else {
        out.push(item.clone());
        for (c, k) in usable {
            let sname = prim_name(g.node(c).map_or("shape", |n| n.name()), o.strip_namespaces);
            let mut sub = Item { xform: NO_NODE, shape: c, path: format!("{path}/{sname}"), shape_path: format!("{path}/{sname}"), kind: k, instance_of: None, is_root: false };
            fill_instance(g, o, &mut sub, seen_meshes);
            out.push(sub);
        }
    }
    for &c in g.children(x) {
        walk(g, o, c, &path, plan, seen_meshes, out, false);
    }
}

/// A mesh drawing the same data as one already walked becomes an instance of it.
fn fill_instance(g: &Graph, o: &Opts, item: &mut Item, _seen: &mut HashMap<u64, NodeId>) {
    if !o.export_instances || item.kind != Kind::Mesh {
        return;
    }
    let root = aztek_scene::instancing::root(g, item.shape);
    if root != item.shape && root != NO_NODE {
        item.instance_of = Some(root);
    }
}

/// The reference's stack of ops, outermost first: value, identity to leave out, whether the op is an inverse.
pub(crate) fn xform_sample(g: &mut Graph, x: NodeId, world: bool, prim: &str, time: Option<f64>, t: &mut Tracks) {
    if world {
        let m = g.node(x).map(|n| n.matrix(aztek_scene::transform::Transform::WORLD_MATRIX)).unwrap_or(DMat4::IDENTITY);
        t.put(prim, "xformOp:transform", "matrix4d", time, Value::Matrix4d(to_gf(&m)));
        return;
    }
    let order = aztek_scene::transform::RotateOrder::from_index(get_f64(g, x, "rotateOrder").unwrap_or(0.0) as i64);
    let rot_name = format!("xformOp:rotate{}", rotate_suffix(order));
    let v = |g: &mut Graph, a: &str| get_v3(g, x, a).unwrap_or(DVec3::ZERO);
    let d3 = |v: DVec3| Value::Vec3d(openusd::gf::vec3d(v.x, v.y, v.z));
    let f3 = |v: DVec3| Value::Vec3f(v3f(v));
    let translate = v(g, "translate");
    t.put(prim, "xformOp:translate", "double3", time, d3(translate));
    let a = v(g, "rotatePivotTranslate");
    t.put(prim, "xformOp:translate:rotatePivotTranslate", "double3", time, d3(a));
    let a = v(g, "rotatePivot");
    t.put(prim, "xformOp:translate:rotatePivot", "double3", time, d3(a));
    // A stage's rotations are degrees.
    let a = v(g, "rotate") * (180.0 / std::f64::consts::PI);
    t.put(prim, &rot_name, "float3", time, f3(a));
    let a = v(g, "rotateAxis") * (180.0 / std::f64::consts::PI);
    t.put(prim, "xformOp:rotateXYZ:rotateAxis", "float3", time, f3(a));
    let a = v(g, "scalePivotTranslate");
    t.put(prim, "xformOp:translate:scalePivotTranslate", "double3", time, d3(a));
    let a = v(g, "scalePivot");
    t.put(prim, "xformOp:translate:scalePivot", "double3", time, d3(a));
    let a = get_v3(g, x, "scale").unwrap_or(DVec3::ONE);
    t.put(prim, "xformOp:scale", "float3", time, f3(a));
    if let Some(sh) = get_v3(g, x, "shear") {
        // USD has no shear op: a matrix in the same place, as the reference exporter writes.
        let m = aztek_scene::transform::shear_matrix(sh);
        t.put(prim, "xformOp:transform:shear", "matrix4d", time, Value::Matrix4d(to_gf(&m)));
    }
}

pub(crate) fn rotate_suffix(o: aztek_scene::transform::RotateOrder) -> &'static str {
    use aztek_scene::transform::RotateOrder as R;
    match o {
        R::Xyz => "XYZ",
        R::Yzx => "YZX",
        R::Zxy => "ZXY",
        R::Xzy => "XZY",
        R::Yxz => "YXZ",
        R::Zyx => "ZYX",
    }
}

fn is_identity_value(name: &str, v: &Value) -> bool {
    match v {
        Value::Vec3d(x) => x.x == 0.0 && x.y == 0.0 && x.z == 0.0,
        Value::Vec3f(x) if name == "xformOp:scale" => x.x == 1.0 && x.y == 1.0 && x.z == 1.0,
        Value::Vec3f(x) => x.x == 0.0 && x.y == 0.0 && x.z == 0.0,
        Value::Matrix4d(m) => util_is_identity(m),
        _ => false,
    }
}

fn util_is_identity(m: &openusd::gf::Matrix4d) -> bool {
    from_gf(m) == DMat4::IDENTITY
}

/// The ops still worth writing, in the reference's order, with pivots closed by their inverses.
fn write_xform_ops(out: &mut Out, prim: &str, t: &mut Tracks, o: &Opts) -> Result<(), String> {
    let names: Vec<String> = t.map.get(prim).map(|l| l.iter().filter(|(n, _)| n.starts_with("xformOp:")).map(|(n, _)| n.clone()).collect()).unwrap_or_default();
    if names.is_empty() {
        return Ok(());
    }
    let mut keep: Vec<String> = Vec::new();
    for n in &names {
        let mut tr = t.take(prim, n).expect("listed");
        if o.euler_filter && n.starts_with("xformOp:rotate") && !n.ends_with("rotateAxis") {
            euler_filter(&mut tr.track.samples);
        }
        let animated = !tr.track.samples.is_empty() && tr.track.samples.iter().any(|(_, v)| *v != tr.track.samples[0].1);
        tr.track.settle();
        let idle = !animated && tr.track.default.as_ref().is_some_and(|v| is_identity_value(n, v));
        if idle {
            continue;
        }
        out.attr_track(prim, n, &tr.ty, &tr.track)?;
        keep.push(n.clone());
    }
    let has = |s: &str| keep.iter().any(|k| k == s);
    let mut order: Vec<Token> = Vec::new();
    let mut push = |s: &str| order.push(Token::from(s));
    if has("xformOp:transform") {
        push("xformOp:transform");
    }
    if has("xformOp:translate") {
        push("xformOp:translate");
    }
    if has("xformOp:translate:rotatePivotTranslate") {
        push("xformOp:translate:rotatePivotTranslate");
    }
    let rp = has("xformOp:translate:rotatePivot");
    if rp {
        push("xformOp:translate:rotatePivot");
    }
    for k in &keep {
        if k.starts_with("xformOp:rotate") && !k.ends_with(":rotateAxis") {
            push(k);
        }
    }
    if has("xformOp:rotateXYZ:rotateAxis") {
        push("xformOp:rotateXYZ:rotateAxis");
    }
    if rp {
        push("!invert!xformOp:translate:rotatePivot");
    }
    if has("xformOp:translate:scalePivotTranslate") {
        push("xformOp:translate:scalePivotTranslate");
    }
    let sp = has("xformOp:translate:scalePivot");
    if sp {
        push("xformOp:translate:scalePivot");
    }
    if has("xformOp:transform:shear") {
        push("xformOp:transform:shear");
    }
    if has("xformOp:scale") {
        push("xformOp:scale");
    }
    if sp {
        push("!invert!xformOp:translate:scalePivot");
    }
    if !order.is_empty() {
        out.uniform(prim, "xformOpOrder", "token[]", Value::TokenVec(order))?;
    }
    Ok(())
}

/// Each sample's angles moved by whole turns to stay nearest the sample before.
fn euler_filter(samples: &mut [(f64, Value)]) {
    let mut prev: Option<[f32; 3]> = None;
    for (_, v) in samples.iter_mut() {
        if let Value::Vec3f(a) = v {
            let mut c = [a.x, a.y, a.z];
            if let Some(p) = prev {
                for i in 0..3 {
                    c[i] -= ((c[i] - p[i]) / 360.0).round() * 360.0;
                }
            }
            *a = openusd::gf::vec3f(c[0], c[1], c[2]);
            prev = Some(c);
        }
    }
}

fn visibility_sample(g: &mut Graph, x: NodeId, prim: &str, time: Option<f64>, t: &mut Tracks) {
    let vis = get_f64(g, x, "visibility").unwrap_or(1.0) != 0.0;
    t.put(prim, "visibility", "token", time, Value::Token(if vis { "inherited".into() } else { "invisible".into() }));
}

/// The prim types and fields that come with each item, once.
fn write_item_prim(out: &mut Out, it: &Item, ty: &str) -> Result<(), String> {
    out.prim(&it.path, Specifier::Def, ty)
}

pub fn run(s: &mut Session, a: &Args) -> CmdResult {
    let o = opts_of(a, s)?;
    export(s, &o).map_err(aztek_ops::CommandError::new)?;
    let after = a.string("melPostCallback", "");
    if !after.is_empty() && s.commands.find("eval").is_some() {
        let _ = s.run("eval", json!({"args": [after]}));
    }
    Ok(json!(o.file))
}

pub fn export(s: &mut Session, o: &Opts) -> Result<(), String> {
    s.graph.evaluate();
    let start_time = s.graph.time();
    let result = export_inner(s, o);
    s.graph.set_time(start_time);
    s.graph.evaluate();
    result
}

fn roots_of(s: &mut Session, o: &Opts) -> Result<Vec<NodeId>, String> {
    let g = &s.graph;
    let mut roots: Vec<NodeId> = Vec::new();
    if !o.export_roots.is_empty() {
        for r in &o.export_roots {
            match g.find(r).map_err(|e| e.to_string())? {
                Some(id) => roots.push(id),
                None => return Err(format!("mayaUSDExport: no node named '{r}'")),
            }
        }
    } else if o.selection {
        roots = g.selection().to_vec();
        if roots.is_empty() {
            return Err("mayaUSDExport: -selection with nothing selected".into());
        }
    } else {
        roots = g.roots().to_vec();
    }
    // A shape stands for its transform; a selected child of a selected parent is covered by the parent.
    let mut up: Vec<NodeId> = roots.iter().map(|&r| if g.node(r).is_some_and(|n| n.ty().is_a("transform")) { r } else { g.parent(r) }).collect();
    let set: HashSet<NodeId> = up.iter().copied().collect();
    up.retain(|&r| {
        let mut p = g.parent(r);
        while p != NO_NODE {
            if set.contains(&p) {
                return false;
            }
            p = g.parent(p);
        }
        true
    });
    up.dedup();
    if !o.default_cameras {
        up.retain(|&r| g.node(r).is_none_or(|n| !DEFAULT_CAMERAS.contains(&n.name())));
    }
    Ok(up)
}

fn export_inner(s: &mut Session, o: &Opts) -> Result<(), String> {
    let roots = roots_of(s, o)?;
    let base = if o.root_prim.is_empty() { String::new() } else { format!("/{}", prim_name(o.root_prim.trim_start_matches('/'), false)) };
    let plan = crate::export_skel::plan(s, o, &roots, &base)?;
    let mut items = Vec::new();
    let mut seen = HashMap::new();
    for &r in &roots {
        walk(&s.graph, o, r, &base, &plan, &mut seen, &mut items, true);
    }
    if !o.include_empty {
        prune_empty(&mut items);
    }
    let mut out = Out::new();
    out.layer_meta("upAxis", Value::Token(o.up_axis.as_str().into()));
    let mpu = if o.meters_per_unit > 0.0 { o.meters_per_unit } else { 0.01 };
    out.layer_meta("metersPerUnit", Value::Double(mpu));
    if !o.frames.is_empty() {
        out.layer_meta(openusd::sdf::FieldKey::StartTimeCode, Value::Double(o.frames[0]));
        out.layer_meta(openusd::sdf::FieldKey::EndTimeCode, Value::Double(o.frames[o.frames.len() - 1]));
        out.layer_meta(openusd::sdf::FieldKey::TimeCodesPerSecond, Value::Double(anim::fps()));
        out.layer_meta(openusd::sdf::FieldKey::FramesPerSecond, Value::Double(anim::fps()));
    }
    if !base.is_empty() {
        let ty = if o.root_prim_type.eq_ignore_ascii_case("xform") { "Xform" } else { "Scope" };
        out.prim(&base, Specifier::Def, ty)?;
    }
    for (path, ty) in &plan.new_prims {
        out.prim(path, Specifier::Def, ty)?;
    }

    // Sampling.
    let mut tracks = Tracks::default();
    let times: Vec<Option<f64>> = if o.frames.is_empty() { vec![None] } else { o.frames.iter().map(|f| Some(*f)).collect() };
    for &time in &times {
        if let Some(f) = time {
            anim::set_frame(&mut s.graph, f);
        }
        s.graph.evaluate();
        for it in &items {
            sample_item(s, o, it, time, &mut tracks, &plan);
        }
        crate::export_skel::sample(s, o, &plan, time, &mut tracks);
    }
    if !o.frames.is_empty() {
        anim::set_frame(&mut s.graph, o.frames[0]);
        s.graph.evaluate();
    }

    // Writing.
    let proto = crate::export_geo::write_instance_prototypes(s, o, &items, &base, &mut out)?;
    for it in &items {
        write_item(s, o, it, &mut out, &mut tracks, &plan, &proto)?;
    }
    crate::export_skel::write(s, o, &plan, &mut out, &mut tracks)?;
    let looks = crate::export_mat::write(s, o, &items, &roots, &base, &mut out, &proto)?;
    let _ = looks;

    // What the walk left over: everything sampled on prims that exist, written as sampled.
    let rest: Vec<String> = tracks.map.keys().cloned().collect();
    for prim in rest {
        if !out.has_prim(&prim) {
            continue;
        }
        write_xform_ops(&mut out, &prim, &mut tracks, o)?;
        let list = tracks.map.remove(&prim).unwrap_or_default();
        for (name, mut tr) in list {
            if !o.static_single || tr.track.samples.len() != 1 {
                tr.track.settle();
            } else {
                tr.track.settle();
            }
            let pp = out.attr_track(&prim, &name, &tr.ty, &tr.track)?;
            if let Some(i) = &tr.interpolation {
                out.attr_field(&pp, "interpolation", Value::Token(i.as_str().into()));
            }
            if tr.element_size > 1 {
                out.attr_field(&pp, "elementSize", Value::Int(tr.element_size));
            }
            if tr.custom {
                out.attr_field(&pp, openusd::sdf::FieldKey::Custom, Value::Bool(true));
            }
            if tr.uniform {
                out.attr_field(&pp, openusd::sdf::FieldKey::Variability, Value::Variability(openusd::sdf::Variability::Uniform));
            }
        }
    }

    // Root prims and the default prim.
    let default_prim = default_prim_name(o, &base, &items);
    if let Some(d) = default_prim {
        out.layer_meta(openusd::sdf::FieldKey::DefaultPrim, Value::Token(d.as_str().into()));
    }
    if !o.kind.is_empty() {
        for it in items.iter().filter(|i| i.is_root) {
            out.prim_field(&it.path, openusd::sdf::FieldKey::Kind, Value::Token(o.kind.as_str().into()))?;
        }
    }
    if !o.custom_layer_data.is_empty() {
        let mut d = HashMap::new();
        for (k, v, ty) in &o.custom_layer_data {
            let val = match ty.as_str() {
                "bool" => Value::Bool(v == "1" || v == "true"),
                "int" => Value::Int(v.parse().unwrap_or(0)),
                "float" | "double" => Value::Double(v.parse().unwrap_or(0.0)),
                _ => Value::String(v.clone()),
            };
            d.insert(k.clone(), val);
        }
        out.layer_meta(openusd::sdf::FieldKey::CustomLayerData, Value::Dictionary(d));
    }
    out.finish()?;
    out::write(&out.data, &o.file, &o.default_format)
}

fn default_prim_name(o: &Opts, base: &str, items: &[Item]) -> Option<String> {
    match &o.default_prim {
        Some(d) if d.is_empty() => None,
        Some(d) => Some(prim_name(d.trim_start_matches('/'), false)),
        None => {
            if !base.is_empty() {
                return Some(base.trim_start_matches('/').to_string());
            }
            // Only a single root: importing a file reads just its default prim, which would lose the others.
            let mut tops: Vec<&str> = items.iter().filter(|i| i.is_root).map(|i| i.path.trim_start_matches('/').split('/').next().unwrap_or("")).collect();
            tops.sort_unstable();
            tops.dedup();
            (tops.len() == 1).then(|| tops[0].to_string())
        }
    }
}

fn prune_empty(items: &mut Vec<Item>) {
    loop {
        let parents: HashSet<String> = items.iter().filter_map(|i| i.path.rsplit_once('/').map(|(p, _)| p.to_string())).collect();
        let before = items.len();
        items.retain(|i| i.kind != Kind::Group || parents.contains(&i.path));
        if items.len() == before {
            break;
        }
    }
}

fn sample_item(s: &mut Session, o: &Opts, it: &Item, time: Option<f64>, t: &mut Tracks, plan: &crate::export_skel::Plan) {
    if it.xform != NO_NODE {
        xform_sample(&mut s.graph, it.xform, o.worldspace && it.is_root, &it.path, time, t);
        if o.export_visibility {
            visibility_sample(&mut s.graph, it.xform, &it.path, time, t);
        }
    }
    if it.instance_of.is_some() {
        return;
    }
    match it.kind {
        Kind::Mesh => crate::export_geo::sample_mesh(s, o, it, time, t, plan),
        Kind::Camera => crate::export_geo::sample_camera(s, it, time, t),
        Kind::Curve => crate::export_geo::sample_curve(s, it, time, t),
        Kind::Surface => crate::export_geo::sample_surface(s, it, time, t),
        Kind::Group | Kind::Proxy => {}
    }
}

fn write_item(s: &mut Session, o: &Opts, it: &Item, out: &mut Out, t: &mut Tracks, plan: &crate::export_skel::Plan, proto: &HashMap<NodeId, String>) -> Result<(), String> {
    let ty = match it.kind {
        Kind::Group => plan.skel_roots.get(&it.xform).map_or("Xform", |_| "SkelRoot"),
        Kind::Mesh => "Mesh",
        Kind::Camera => "Camera",
        Kind::Curve => "NurbsCurves",
        Kind::Surface => "NurbsPatch",
        Kind::Proxy => "Xform",
    };
    if it.xform == NO_NODE {
        // A shape of its own, under its transform's prim.
        write_item_prim(out, it, ty)?;
    } else {
        write_item_prim(out, it, if it.instance_of.is_some() { "" } else { ty })?;
        write_xform_ops(out, &it.path, t, o)?;
        crate::export_geo::write_custom(s, it.xform, &it.path, out)?;
    }
    if let Some(src) = it.instance_of {
        if let Some(p) = proto.get(&src) {
            crate::export_geo::write_instance_arc(out, &it.path, p)?;
        }
        return Ok(());
    }
    match it.kind {
        Kind::Mesh => crate::export_geo::write_mesh_static(s, o, it, out, t, plan)?,
        Kind::Proxy => crate::export_geo::write_proxy(s, o, it, out)?,
        _ => {}
    }
    Ok(())
}

pub fn register(c: &mut Commands<Session>) {
    let b = |n: &str, sn: &str, d: &str| FlagDef::new(n, sn, ArgType::Any, J::Null, d);
    let mut def = CommandDef::new(
        "mayaUSDExport",
        "Write the scene to a USD file: usda, usdc, usdz or usd by the file's extension. Flags as the reference's USD plug-in's. Returns the file.",
        run,
    )
    .undoable(false);
    for (n, sn, d) in [
        ("file", "f", "The file to write. Its extension picks the format."),
        ("defaultUSDFormat", "duf", "usdc or usda, for a .usd file."),
        ("selection", "sl", "Only what is selected and below it."),
        ("exportRoots", "ert", "DAG subtrees to write, without their parents."),
        ("exportUVs", "uvs", "UV sets as st primvars (on)."),
        ("exportColorSets", "cls", "Colour sets as primvars (on)."),
        ("exportDisplayColor", "dsp", "The material's colour as primvars:displayColor (off)."),
        ("exportVisibility", "vis", "visibility, animated or not (on)."),
        ("exportInstances", "ein", "Instanced shapes as USD instances (on)."),
        ("exportBlendShapes", "ebs", "Blend shapes as BlendShape prims (off)."),
        ("exportMaterials", "mat", "Materials (on)."),
        ("exportAssignedMaterials", "ama", "Only materials something uses (on)."),
        ("exportSkels", "skl", "none, auto or explicit: skeletons as Skeleton prims."),
        ("exportSkin", "skn", "none, auto or explicit: skinClusters through UsdSkel."),
        ("shadingMode", "shd", "useRegistry or none."),
        ("convertMaterialsTo", "cmt", "UsdPreviewSurface."),
        ("mergeTransformAndShape", "mt", "A transform and its one shape become one prim (on)."),
        ("stripNamespaces", "sn", "Drop namespaces from prim names (off)."),
        ("eulerFilter", "ef", "Keep animated rotations from jumping by whole turns (off)."),
        ("worldspace", "wsp", "Root prims take their world matrix (off)."),
        ("staticSingleSample", "sss", "Accepted: values that do not change are always written static."),
        ("defaultCameras", "dc", "Include persp, top, front and side."),
        ("includeEmptyTransforms", "iet", "Keep transforms with nothing below them (on)."),
        ("preserveUVSetNames", "puv", "Keep UV set names rather than st1, st2 (off)."),
        ("defaultMeshScheme", "dms", "Subdivision scheme of meshes that say nothing: catmullClark, loop, bilinear or none."),
        ("materialsScopeName", "msn", "The scope materials go under (Looks)."),
        ("rootPrim", "rpm", "A prim everything is written under."),
        ("parentScope", "psc", "Older name of rootPrim."),
        ("rootPrimType", "rpt", "Scope or Xform."),
        ("defaultPrim", "dp", "The layer's default prim; empty for none."),
        ("kind", "k", "The kind root prims take."),
        ("frameRange", "fr", "First and last frame of an animated export."),
        ("frameStride", "fst", "Frames between samples (1)."),
        ("frameSample", "fs", "Offsets sampled at each stride."),
        ("extraFrame", "exf", "Further frames to sample."),
        ("exportRelativeTextures", "rtx", "automatic, relative or absolute."),
        ("filterTypes", "ft", "Node types to leave out."),
        ("excludeExportTypes", "eet", "Meshes or Cameras: families to leave out."),
        ("remapUVSetsTo", "ruv", "Pairs of a UV set and the primvar name it is written as."),
        ("customLayerData", "cld", "Triples of key, value and type for the layer's customLayerData."),
        ("metersPerUnit", "mpu", "metersPerUnit written; 0.01 by default."),
        ("upAxis", "upa", "mayaPrefs, y or z."),
        ("unit", "unt", "Accepted: mayaPrefs."),
        ("exportStagesAsRefs", "sar", "Proxy shapes as references to their files (on)."),
        ("melPerFrameCallback", "mfc", "Accepted, not run."),
        ("melPostCallback", "mpc", "MEL run when the export is done."),
        ("verbose", "v", "Accepted."),
    ] {
        let arity = match n {
            "frameRange" | "remapUVSetsTo" => 2,
            "customLayerData" => 3,
            "exportRoots" | "frameSample" | "extraFrame" | "filterTypes" | "excludeExportTypes" | "convertMaterialsTo" => 1,
            _ => 0,
        };
        // These take no value in MEL.
        let no_value = matches!(n, "selection" | "defaultCameras" | "verbose");
        let f = if no_value { FlagDef::new(n, sn, ArgType::Bool, J::Null, d) } else { b(n, sn, d) };
        def = def.flag(if arity > 0 { f.multi(arity) } else { f });
    }
    c.register(def);
}
