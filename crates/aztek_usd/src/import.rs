//! mayaUSDImport: a USD file's prims as Aztek nodes. Flags and defaults follow the reference's USD plug-in's command reference.

use std::collections::HashMap;

use aztek_geo::nurbs::{Form, NurbsCurve, NurbsSurface};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::anim::{self, Tangent};
use aztek_scene::graph::{Graph, SelectMode};
use aztek_scene::mesh_nodes::{self, MeshNode, UpAxis};
use aztek_scene::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::transform::{decompose_full, euler_near, matrix_to_euler, DagNode, RotateOrder, Transform};
use aztek_scene::{cameras, curves, instancing, node::types, surfaces};
use glam::{DMat3, DMat4, DVec3};
use openusd::gf;
use openusd::sdf::Value;
use openusd::tf::Token;
use openusd::usd::{self, InitialLoadSet, Prim, Stage, TimeCode};
use openusd_schemas::geom::XformQuery;
use serde_json::{json, Value as J};
use std::sync::Arc;

use crate::mesh_read::{at, f64_of, get_vec, read_gprim, read_mesh, token_of, ReadOpts};
use crate::util::*;

#[derive(Clone, Debug, Default)]
pub struct ImportOpts {
    pub file: String,
    pub prim_path: String,
    pub read_anim: bool,
    pub frame_range: Option<(f64, f64)>,
    pub parent: String,
    pub shading: bool,
    pub preferred_material: String,
    pub variants: Vec<(String, String)>,
    pub prim_variants: Vec<(String, String, String)>,
    pub import_instances: bool,
    pub up_axis: bool,
    pub unit: bool,
    pub axis_method: String,
    pub metadata: Vec<String>,
    pub read: ReadOpts,
}

pub(crate) struct Ctx<'a> {
    pub s: &'a mut Session,
    pub o: &'a ImportOpts,
    pub stage: Stage,
    /// Prim path to transform.
    pub nodes: HashMap<String, NodeId>,
    /// Prim path to mesh shape, and the file-to-mesh face list.
    pub meshes: HashMap<String, (NodeId, Vec<u32>)>,
    /// Prototype prim to the shapes first imported from it, by path inside the prototype.
    pub protos: HashMap<String, HashMap<String, NodeId>>,
    pub materials: HashMap<String, Option<String>>,
    pub display_groups: HashMap<[u32; 3], String>,
    pub times: Vec<f64>,
    pub warnings: Vec<String>,
}

fn multi(a: &Args, name: &str) -> Vec<String> {
    match a.get(name) {
        Some(J::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(J::String(s)) => vec![s.clone()],
        _ => Vec::new(),
    }
}

fn opts_of(a: &Args) -> CmdResult<ImportOpts> {
    let file = a.string("file", "");
    let file = if file.is_empty() { a.pos_str(0).unwrap_or("").to_string() } else { file };
    if file.is_empty() {
        fail!("mayaUSDImport: -file is required");
    }
    let flag = |n: &str, d: bool| a.get(n).map_or(d, |v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    let frame_range = match a.get("frameRange") {
        Some(J::Array(v)) => match (v.first().and_then(J::as_f64), v.get(1).and_then(J::as_f64)) {
            (Some(x), Some(y)) => Some((x, y)),
            _ => None,
        },
        _ => None,
    };
    let shading_list = multi(a, "shadingMode");
    let shading = shading_list.first().is_none_or(|m| m != "none");
    let pairs = multi(a, "variant").chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect();
    // -primVariant "path=set:selection"
    let mut prim_variants = Vec::new();
    for spec in multi(a, "primVariant") {
        if let Some((p, rest)) = spec.split_once('=') {
            if let Some((set, sel)) = rest.split_once(':') {
                prim_variants.push((p.to_string(), set.to_string(), sel.to_string()));
            }
        }
    }
    Ok(ImportOpts {
        file,
        prim_path: a.string("primPath", ""),
        read_anim: flag("readAnimData", false),
        frame_range,
        parent: a.string("parent", ""),
        shading,
        preferred_material: a.string("preferredMaterial", ""),
        variants: pairs,
        prim_variants,
        import_instances: flag("importInstances", true),
        up_axis: flag("upAxis", true),
        unit: flag("unit", true),
        axis_method: a.string("axisAndUnitMethod", "rotateScale"),
        metadata: if a.has("metadata") { multi(a, "metadata") } else { vec!["hidden".into(), "instanceable".into(), "kind".into()] },
        read: ReadOpts {
            exclude_primvars: multi(a, "excludePrimvar"),
            remap_uv: multi(a, "remapUVSetsTo").chunks(2).filter(|c| c.len() == 2).map(|c| (c[0].clone(), c[1].clone())).collect(),
        },
    })
}

/// A temporary session layer holding the variant selections asked for. None when there are none.
fn variant_session(o: &ImportOpts, start: &str) -> Result<Option<std::path::PathBuf>, String> {
    if o.variants.is_empty() && o.prim_variants.is_empty() {
        return Ok(None);
    }
    let mut all: Vec<(String, String, String)> = o.prim_variants.clone();
    let root = if start.is_empty() || start == "/" { String::new() } else { start.to_string() };
    for (set, sel) in &o.variants {
        all.push((root.clone(), set.clone(), sel.clone()));
    }
    let mut text = String::from("#usda 1.0\n");
    for (path, set, sel) in &all {
        let names: Vec<&str> = path.split('/').filter(|x| !x.is_empty()).collect();
        if names.is_empty() {
            continue;
        }
        for (i, n) in names.iter().enumerate() {
            text.push_str(&format!("{:indent$}over \"{n}\"", "", indent = i * 4));
            if i + 1 == names.len() {
                text.push_str(&format!(" (\n{:indent$}variants = {{ string {set} = \"{sel}\" }}\n{:indent2$})", "", "", indent = i * 4 + 4, indent2 = i * 4));
            }
            text.push_str(&format!("\n{:indent$}{{\n", "", indent = i * 4));
        }
        for i in (0..names.len()).rev() {
            text.push_str(&format!("{:indent$}}}\n", "", indent = i * 4));
        }
    }
    let path = std::env::temp_dir().join(format!("aztek_usd_variants_{}_{:p}.usda", std::process::id(), &text));
    std::fs::write(&path, text).map_err(|e| e.to_string())?;
    Ok(Some(path))
}

/// The attribute a type adds. Attributes on a node come through the graph so they save with it.
pub fn add_user_attr(g: &mut Graph, id: NodeId, name: &str, v: PlugValue) {
    let ty = match &v {
        PlugValue::Bool(_) => PlugType::Bool,
        PlugValue::Int(_) => PlugType::Int,
        PlugValue::Float(_) => PlugType::Float,
        PlugValue::Vec3(_) => PlugType::Vec3,
        PlugValue::String(_) => PlugType::String,
        _ => return,
    };
    if plug_of(g, id, name).is_none() {
        let def = AttrDef { name: name.into(), ty, default_value: v.clone(), flags: flags::INPUT | flags::STORABLE | flags::USER, ..Default::default() };
        if let Err(e) = g.add_attr(id, def) {
            log::warn!("USD: can't add {name}: {e}");
            return;
        }
    }
    set_attr(g, id, name, v);
}

impl Ctx<'_> {
    fn g(&mut self) -> &mut Graph {
        &mut self.s.graph
    }

    /// Keys along the samples; the first frame's value also stands as the attribute's value.
    fn key(&mut self, p: PlugRef, keys: &[(f64, f64)]) {
        for (f, v) in keys {
            if let Err(e) = anim::key_plug(self.g(), &p, *f, Some(*v), Some(Tangent::Linear), Some(Tangent::Linear)) {
                log::warn!("USD: can't key: {e}");
            }
        }
    }

    fn in_range(&self, f: f64) -> bool {
        self.o.frame_range.is_none_or(|(a, b)| f >= a && f <= b)
    }

    fn sample_times(&self, attr_times: Vec<f64>) -> Vec<f64> {
        if !self.o.read_anim {
            return Vec::new();
        }
        attr_times.into_iter().filter(|f| self.in_range(*f)).collect()
    }

    fn note_times(&mut self, t: &[f64]) {
        self.times.extend_from_slice(t);
    }

    /// Sets a vector channel, or keys it over `times`.
    fn set_channels(&mut self, node: NodeId, attr: &str, times: &[f64], read: &mut dyn FnMut(Option<f64>) -> DVec3) {
        let Some(p) = plug_of(&self.s.graph, node, attr) else { return };
        if times.len() > 1 {
            let values: Vec<DVec3> = times.iter().map(|t| read(Some(*t))).collect();
            let moves = |c: usize| values.iter().any(|v| v[c] != values[0][c]);
            for c in 0..3 {
                if moves(c) {
                    let keys: Vec<(f64, f64)> = times.iter().zip(&values).map(|(t, v)| (*t, v[c])).collect();
                    self.key(PlugRef::component(node, p.attr, c as i32), &keys);
                } else {
                    let _ = self.g().set_value(PlugRef::component(node, p.attr, c as i32), PlugValue::Float(values[0][c]));
                }
            }
            self.note_times(times);
        } else {
            let t = times.first().copied();
            let v = read(t);
            let _ = self.g().set_value(p, PlugValue::Vec3(v));
        }
    }
}

fn d3(a: &usd::Attribute, t: Option<f64>) -> Option<DVec3> {
    if let Ok(Some(v)) = a.get_at::<gf::Vec3d>(at(t)) {
        return Some(DVec3::new(v.x, v.y, v.z));
    }
    if let Ok(Some(v)) = a.get_at::<gf::Vec3f>(at(t)) {
        return Some(DVec3::new(v.x as f64, v.y as f64, v.z as f64));
    }
    None
}

/// Where an op stands in the stack the reference's attributes can hold. None for one they can't.
fn canonical_rank(op: &str) -> Option<usize> {
    Some(match op {
        "xformOp:translate" => 0,
        "xformOp:translate:rotatePivotTranslate" => 1,
        "xformOp:translate:rotatePivot" => 2,
        "xformOp:rotateXYZ" | "xformOp:rotateYZX" | "xformOp:rotateZXY" | "xformOp:rotateXZY" | "xformOp:rotateYXZ" | "xformOp:rotateZYX" => 3,
        "xformOp:rotateXYZ:rotateAxis" => 4,
        "!invert!xformOp:translate:rotatePivot" => 5,
        "xformOp:translate:scalePivotTranslate" => 6,
        "xformOp:translate:scalePivot" => 7,
        "xformOp:scale" => 9,
        "!invert!xformOp:translate:scalePivot" => 10,
        _ => return None,
    })
}

fn order_of(op: &str) -> Option<RotateOrder> {
    Some(RotateOrder::from_index(match op.strip_prefix("xformOp:rotate")? {
        "XYZ" => 0,
        "YZX" => 1,
        "ZXY" => 2,
        "XZY" => 3,
        "YXZ" => 4,
        "ZYX" => 5,
        _ => return None,
    }))
}

fn op_order(prim: &Prim) -> Vec<String> {
    prim.attribute("xformOpOrder").get::<Vec<Token>>().ok().flatten().map(|v| v.iter().map(|t| t.as_str().to_string()).collect()).unwrap_or_default()
}

/// True when the ops are a subset of the reference's stack in its order, with each pivot closed by its inverse.
fn fits_channels(ops: &[String]) -> bool {
    let mut last = None;
    for op in ops {
        let Some(r) = canonical_rank(op) else { return false };
        if last.is_some_and(|l| r <= l) {
            return false;
        }
        last = Some(r);
    }
    let has = |s: &str| ops.iter().any(|o| o == s);
    has("xformOp:translate:rotatePivot") == has("!invert!xformOp:translate:rotatePivot") && has("xformOp:translate:scalePivot") == has("!invert!xformOp:translate:scalePivot")
}

fn read_xform(c: &mut Ctx, prim: &Prim, node: NodeId) {
    let mut ops = op_order(prim);
    if ops.first().map(String::as_str) == Some("!resetXformStack!") {
        ops.remove(0);
        set_attr(c.g(), node, "inheritsTransform", PlugValue::Bool(false));
    }
    if ops.is_empty() {
        return;
    }
    if fits_channels(&ops) {
        for op in &ops {
            if op.starts_with("!invert!") {
                continue;
            }
            let a = prim.attribute(op.as_str());
            let times = c.sample_times(a.time_sample_times().unwrap_or_default());
            let mut read = |t: Option<f64>| d3(&a, t).unwrap_or(DVec3::ZERO);
            let attr = if let Some(ord) = order_of(op) {
                set_attr(c.g(), node, "rotateOrder", PlugValue::Int(ord_index(ord)));
                "rotate"
            } else {
                match op.as_str() {
                    "xformOp:translate" => "translate",
                    "xformOp:translate:rotatePivotTranslate" => "rotatePivotTranslate",
                    "xformOp:translate:rotatePivot" => "rotatePivot",
                    "xformOp:rotateXYZ:rotateAxis" => "rotateAxis",
                    "xformOp:translate:scalePivotTranslate" => "scalePivotTranslate",
                    "xformOp:translate:scalePivot" => "scalePivot",
                    "xformOp:scale" => "scale",
                    _ => continue,
                }
            };
            c.set_channels(node, attr, &times, &mut read);
        }
        return;
    }
    // Anything else goes through the matrix: same place, though the file's separate ops aren't kept.
    let Ok(q) = XformQuery::for_prim(prim) else { return };
    let times = c.sample_times(q.time_samples().unwrap_or_default());
    let local = |t: Option<f64>| -> DMat4 { q.local_transformation(at(t)).map(|m| from_gf(&m)).unwrap_or(DMat4::IDENTITY) };
    if times.len() > 1 {
        let mut prev = DVec3::ZERO;
        let (mut tk, mut rk, mut sk) = (Vec::new(), Vec::new(), Vec::new());
        for &f in &times {
            let (t, r, s, _) = decompose_full(&local(Some(f)));
            let e = euler_near(&r, RotateOrder::Xyz, prev);
            prev = e;
            tk.push((f, t));
            rk.push((f, e));
            sk.push((f, s));
        }
        for (attr, list) in [("translate", tk), ("rotate", rk), ("scale", sk)] {
            let Some(p) = plug_of(&c.s.graph, node, attr) else { continue };
            for k in 0..3 {
                let keys: Vec<(f64, f64)> = list.iter().map(|(f, v)| (*f, v[k])).collect();
                if keys.iter().any(|x| x.1 != keys[0].1) {
                    c.key(PlugRef::component(node, p.attr, k as i32), &keys);
                } else {
                    let _ = c.g().set_value(PlugRef::component(node, p.attr, k as i32), PlugValue::Float(keys[0].1));
                }
            }
        }
        c.note_times(&times);
    } else {
        let m = local(times.first().copied());
        let _ = Transform::set_local_matrix(c.g(), node, &m);
    }
}

fn ord_index(o: RotateOrder) -> i64 {
    match o {
        RotateOrder::Xyz => 0,
        RotateOrder::Yzx => 1,
        RotateOrder::Zxy => 2,
        RotateOrder::Xzy => 3,
        RotateOrder::Yxz => 4,
        RotateOrder::Zyx => 5,
    }
}

fn read_visibility(c: &mut Ctx, prim: &Prim, node: NodeId) {
    let a = prim.attribute("visibility");
    let times = c.sample_times(a.time_sample_times().unwrap_or_default());
    if times.len() > 1 {
        let keys: Vec<(f64, f64)> = times.iter().map(|&f| (f, f64::from(a.get_at::<Token>(TimeCode::new(f)).ok().flatten().is_none_or(|v| v.as_str() != "invisible")))).collect();
        let p = PlugRef::new(node, DagNode::VISIBILITY as i32);
        for (f, v) in keys {
            let _ = anim::key_plug(c.g(), &p, f, Some(v), Some(Tangent::Step), Some(Tangent::Step));
        }
        c.note_times(&times);
    } else if a.get_at::<Token>(at(None)).ok().flatten().is_some_and(|v| v.as_str() == "invisible") {
        let _ = c.g().set_value(PlugRef::new(node, DagNode::VISIBILITY as i32), PlugValue::Bool(false));
    }
}

fn read_custom(c: &mut Ctx, prim: &Prim, node: NodeId) {
    let meta = c.o.metadata.clone();
    if meta.iter().any(|m| m == "kind") {
        if let Ok(Some(k)) = prim.kind() {
            if !k.as_str().is_empty() {
                add_user_attr(c.g(), node, "USD_kind", PlugValue::String(k.as_str().into()));
            }
        }
    }
    if meta.iter().any(|m| m == "hidden") && prim.get_metadata::<bool>("hidden").ok().flatten() == Some(true) {
        add_user_attr(c.g(), node, "USD_hidden", PlugValue::Bool(true));
    }
    if meta.iter().any(|m| m == "instanceable") && prim.is_instanceable().unwrap_or(false) {
        add_user_attr(c.g(), node, "USD_instanceable", PlugValue::Bool(true));
    }
    if let Some(p) = token_of(prim, "purpose") {
        if p != "default" && prim.attribute("purpose").has_authored_value().unwrap_or(false) {
            add_user_attr(c.g(), node, "USD_purpose", PlugValue::String(p));
        }
    }
    let Ok(attrs) = prim.authored_attributes() else { return };
    let mut listed = serde_json::Map::new();
    for a in attrs {
        let name = a.name().to_string();
        let Some(rest) = name.strip_prefix("userProperties:") else {
            if a.is_custom().unwrap_or(false) && !name.contains(':') {
                if let Some(v) = value_to_plug(&a) {
                    add_user_attr(c.g(), node, &format!("USD_ATTR_{}", node_name(&name)), v);
                }
            }
            continue;
        };
        let attr_name = node_name(rest);
        if let Some(v) = value_to_plug(&a) {
            add_user_attr(c.g(), node, &attr_name, v);
            listed.insert(attr_name, json!({"usdAttrName": rest, "usdAttrType": "userProperties"}));
        }
    }
    if !listed.is_empty() {
        add_user_attr(c.g(), node, "USD_UserExportedAttributesJson", PlugValue::String(J::Object(listed).to_string()));
    }
}

fn value_to_plug(a: &usd::Attribute) -> Option<PlugValue> {
    let v = a.get_at::<Value>(at(None)).ok().flatten()?;
    Some(match v {
        Value::Bool(b) => PlugValue::Bool(b),
        Value::Int(i) => PlugValue::Int(i as i64),
        Value::Int64(i) => PlugValue::Int(i),
        Value::Float(f) => PlugValue::Float(f as f64),
        Value::Double(f) => PlugValue::Float(f),
        Value::String(s) => PlugValue::String(s),
        Value::Token(t) => PlugValue::String(t.as_str().to_string()),
        Value::Vec3f(x) => PlugValue::Vec3(DVec3::new(x.x as f64, x.y as f64, x.z as f64)),
        Value::Vec3d(x) => PlugValue::Vec3(DVec3::new(x.x, x.y, x.z)),
        _ => return None,
    })
}

const GROUPING: [&str; 4] = ["", "Xform", "Scope", "SkelRoot"];
const SKIP_SUBTREE: [&str; 8] = ["Material", "Shader", "NodeGraph", "Skeleton", "SkelAnimation", "BlendShape", "GeomSubset", "RenderSettings"];
const GPRIMS: [&str; 6] = ["Cube", "Sphere", "Cylinder", "Cone", "Capsule", "Plane"];

fn make_transform(c: &mut Ctx, name: &str, parent: NodeId) -> Result<NodeId, String> {
    let t = types::find("transform").expect("transform");
    c.g().create_node(&t, &node_name(name), parent, false).map_err(|e| e.to_string())
}

fn shape_under(c: &mut Ctx, tr: NodeId, ty: &str) -> Result<NodeId, String> {
    let name = format!("{}Shape", c.g().node(tr).map_or("shape", |n| n.name()));
    let t = types::find(ty).ok_or_else(|| format!("no node type {ty}"))?;
    c.g().create_node(&t, &name, tr, false).map_err(|e| e.to_string())
}

fn read_mesh_prim(c: &mut Ctx, prim: &Prim, tr: NodeId, proto: Option<(&str, &str)>) -> Result<(), String> {
    // A later instance of a prototype already read shares its shape.
    if let Some((proto_root, rel)) = proto {
        if let Some(&src) = c.protos.get(proto_root).and_then(|m| m.get(rel)) {
            let sh = shape_under(c, tr, "mesh")?;
            instancing::connect_instance(c.g(), src, sh).map_err(|e| e.to_string())?;
            return Ok(());
        }
    }
    let Some(r) = read_mesh(prim, None, &c.o.read) else {
        c.warnings.push(format!("{}: a mesh with no points", prim.path()));
        return Ok(());
    };
    if r.skipped_faces > 0 {
        c.warnings.push(format!("{}: {} faces repeat a point and were left out", prim.path(), r.skipped_faces));
    }
    let sh = shape_under(c, tr, "mesh")?;
    let scheme = r.scheme.clone();
    c.g().set_value(PlugRef::new(sh, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(r.mesh)))).map_err(|e| e.to_string())?;
    add_user_attr(c.g(), sh, "USD_subdivisionScheme", PlugValue::String(scheme));
    if let Ok(Some(d)) = prim.attribute("doubleSided").get::<bool>() {
        set_attr(c.g(), sh, "doubleSided", PlugValue::Bool(d));
    }
    let key = prim.path().as_str().to_string();
    c.meshes.insert(key, (sh, r.face_map));
    if let Some((proto_root, rel)) = proto {
        c.protos.entry(proto_root.to_string()).or_default().insert(rel.to_string(), sh);
    }
    Ok(())
}

fn read_camera(c: &mut Ctx, prim: &Prim, tr: NodeId) -> Result<(), String> {
    let sh = shape_under(c, tr, cameras::TYPE)?;
    const INCH: f64 = 25.4;
    let ortho = token_of(prim, "projection").as_deref() == Some("orthographic");
    let g = &mut c.s.graph;
    let ha = f64_of(prim, "horizontalAperture", None).unwrap_or(20.955);
    let va = f64_of(prim, "verticalAperture", None).unwrap_or(15.2908);
    if ortho {
        set_attr(g, sh, "orthographic", PlugValue::Bool(true));
        set_attr(g, sh, "orthographicWidth", PlugValue::Float(ha / 10.0));
    } else {
        set_attr(g, sh, "horizontalFilmAperture", PlugValue::Float(ha / INCH));
        set_attr(g, sh, "verticalFilmAperture", PlugValue::Float(va / INCH));
    }
    if let Some(v) = f64_of(prim, "focalLength", None) {
        set_attr(g, sh, "focalLength", PlugValue::Float(v));
    }
    if let Some(v) = f64_of(prim, "focusDistance", None) {
        if v > 0.0 {
            set_attr(g, sh, "centerOfInterest", PlugValue::Float(v));
        }
    }
    if let Some(v) = f64_of(prim, "fStop", None) {
        set_attr(g, sh, "fStop", PlugValue::Float(v));
    }
    if let Ok(Some(r)) = prim.attribute("clippingRange").get_at::<gf::Vec2f>(at(None)) {
        set_attr(g, sh, "nearClipPlane", PlugValue::Float(r.x as f64));
        set_attr(g, sh, "farClipPlane", PlugValue::Float(r.y as f64));
    }
    // Animated lens channels key like any others.
    let fl = prim.attribute("focalLength");
    let times = c.sample_times(fl.time_sample_times().unwrap_or_default());
    if times.len() > 1 {
        let keys: Vec<(f64, f64)> = times.iter().map(|&f| (f, f64_of(prim, "focalLength", Some(f)).unwrap_or(35.0))).collect();
        if let Some(p) = plug_of(&c.s.graph, sh, "focalLength") {
            c.key(p, &keys);
        }
        c.note_times(&times);
    }
    Ok(())
}

/// USD counts one more knot at each end than the reference's list.
fn ref_knots(k: &[f64]) -> Vec<f64> {
    if k.len() >= 2 {
        k[1..k.len() - 1].to_vec()
    } else {
        k.to_vec()
    }
}

fn read_curves(c: &mut Ctx, prim: &Prim, tr: NodeId, ty: &str) -> Result<(), String> {
    let Some(points) = get_vec::<gf::Vec3f>(&prim.attribute("points"), None) else { return Ok(()) };
    let counts = get_vec::<i32>(&prim.attribute("curveVertexCounts"), None).unwrap_or_else(|| vec![points.len() as i32]);
    let orders = get_vec::<i32>(&prim.attribute("order"), None).unwrap_or_default();
    let knots = get_vec::<f64>(&prim.attribute("knots"), None).unwrap_or_default();
    let basis_type = token_of(prim, "type").unwrap_or_else(|| "cubic".to_string());
    let mut pstart = 0usize;
    let mut kstart = 0usize;
    for (i, &n) in counts.iter().enumerate() {
        let n = n.max(0) as usize;
        if pstart + n > points.len() || n < 2 {
            pstart += n;
            continue;
        }
        let cvs: Vec<DVec3> = points[pstart..pstart + n].iter().map(|p| DVec3::new(p.x as f64, p.y as f64, p.z as f64)).collect();
        pstart += n;
        let curve = if ty == "NurbsCurves" {
            let order = orders.get(i).copied().unwrap_or(4).max(2) as usize;
            let degree = order - 1;
            let k = n + order;
            let list = knots.get(kstart..kstart + k).map(<[f64]>::to_vec);
            kstart += k;
            match list {
                Some(l) => {
                    let wrapped = n > degree && (0..degree).all(|d| cvs[d] == cvs[n - degree + d]);
                    let closed = !wrapped && cvs[0] == cvs[n - 1];
                    let form = if wrapped { Form::Periodic } else if closed { Form::Closed } else { Form::Open };
                    NurbsCurve::new(degree, form, cvs, ref_knots(&l))
                }
                None => NurbsCurve::open(degree, cvs),
            }
        } else {
            NurbsCurve::open(if basis_type == "linear" { 1 } else { 3 }, cvs)
        };
        let (t2, s2) = curves::create_curve_node(c.g(), "usdCurve", curve).map_err(|e| e.to_string())?;
        c.g().set_parent(s2, tr, false, usize::MAX).map_err(|e| e.to_string())?;
        let shape_name = format!("{}Shape{}", c.g().node(tr).map_or("", |n| n.name()), if i == 0 { String::new() } else { i.to_string() });
        let _ = c.g().rename(s2, &shape_name);
        c.g().delete_node(t2).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn form_of(t: Option<String>) -> Form {
    match t.as_deref() {
        Some("closed") => Form::Closed,
        Some("periodic") => Form::Periodic,
        _ => Form::Open,
    }
}

fn read_patch(c: &mut Ctx, prim: &Prim, tr: NodeId) -> Result<(), String> {
    let Some(points) = get_vec::<gf::Vec3f>(&prim.attribute("points"), None) else { return Ok(()) };
    let int = |n: &str, d: i32| prim.attribute(n).get_at::<i32>(at(None)).ok().flatten().unwrap_or(d);
    let (nu, nv) = (int("uVertexCount", 0).max(0) as usize, int("vVertexCount", 0).max(0) as usize);
    if nu * nv != points.len() || nu == 0 {
        c.warnings.push(format!("{}: patch counts don't fit its points", prim.path()));
        return Ok(());
    }
    let mut cvs = vec![DVec3::ZERO; points.len()];
    for v in 0..nv {
        for u in 0..nu {
            let p = points[v * nu + u];
            cvs[u * nv + v] = DVec3::new(p.x as f64, p.y as f64, p.z as f64);
        }
    }
    let surf = NurbsSurface {
        degree_u: (int("uOrder", 4) - 1).max(1) as usize,
        degree_v: (int("vOrder", 4) - 1).max(1) as usize,
        form_u: form_of(token_of(prim, "uForm")),
        form_v: form_of(token_of(prim, "vForm")),
        nu,
        nv,
        cvs,
        knots_u: ref_knots(&get_vec::<f64>(&prim.attribute("uKnots"), None).unwrap_or_default()),
        knots_v: ref_knots(&get_vec::<f64>(&prim.attribute("vKnots"), None).unwrap_or_default()),
        trim: Vec::new(),
    };
    let (t2, s2) = surfaces::create_surface(c.g(), "usdSurface", surf).map_err(|e| e.to_string())?;
    c.g().set_parent(s2, tr, false, usize::MAX).map_err(|e| e.to_string())?;
    let shape_name = format!("{}Shape", c.g().node(tr).map_or("", |n| n.name()));
    let _ = c.g().rename(s2, &shape_name);
    c.g().delete_node(t2).map_err(|e| e.to_string())?;
    Ok(())
}

/// A transform for a prim with its xform ops, nothing else.
pub(crate) fn read_group(c: &mut Ctx, prim: &Prim, parent: NodeId) -> Result<NodeId, String> {
    let name = prim.path().name().unwrap_or("prim").to_string();
    let tr = make_transform(c, &name, parent)?;
    read_xform(c, prim, tr);
    Ok(tr)
}

/// Reads a prim and what's below it.
pub(crate) fn read_prim(c: &mut Ctx, prim: &Prim, parent: NodeId, proto: Option<(String, String)>) -> Result<Option<NodeId>, String> {
    let ty = prim.type_name().map_err(|e| e.to_string())?.map_or(String::new(), |t| t.as_str().to_string());
    if SKIP_SUBTREE.contains(&ty.as_str()) {
        return Ok(None);
    }
    let name = prim.path().name().unwrap_or("prim").to_string();
    let spec = prim.specifier().ok().flatten();
    if matches!(spec, Some(openusd::sdf::Specifier::Class)) {
        return Ok(None);
    }
    let tr = make_transform(c, &name, parent)?;
    let key = prim.path().as_str().to_string();
    c.nodes.insert(key, tr);
    read_xform(c, prim, tr);
    read_visibility(c, prim, tr);
    read_custom(c, prim, tr);

    // Instancing: everything below an instance is known by its path inside the prototype.
    let mut proto = proto;
    if c.o.import_instances && prim.is_instance().unwrap_or(false) {
        if let Ok(Some(p)) = prim.prototype() {
            proto = Some((p.as_str().to_string(), String::new()));
        }
    }
    let proto_ref = proto.as_ref().map(|(a, b)| (a.as_str(), b.as_str()));
    // A script's translator takes the prim and what's below it.
    if crate::extras::translator_for(&ty).is_some() {
        let parent_name = c.s.graph.full_path(tr);
        let file = c.o.file.clone();
        crate::extras::run_translator(c.s, &ty, &file, prim.path().as_str(), &parent_name);
        return Ok(Some(tr));
    }
    match ty.as_str() {
        "Mesh" => read_mesh_prim(c, prim, tr, proto_ref)?,
        "Camera" => read_camera(c, prim, tr)?,
        "NurbsCurves" | "BasisCurves" | "HermiteCurves" => read_curves(c, prim, tr, &ty)?,
        "NurbsPatch" => read_patch(c, prim, tr)?,
        t if GPRIMS.contains(&t) => {
            if let Some(m) = read_gprim(prim, t, None) {
                let sh = shape_under(c, tr, "mesh")?;
                c.g().set_value(PlugRef::new(sh, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(m)))).map_err(|e| e.to_string())?;
                c.meshes.insert(prim.path().as_str().to_string(), (sh, Vec::new()));
            }
        }
        "PointInstancer" => read_instancer(c, prim, tr)?,
        t if GROUPING.contains(&t) => {}
        other => {
            let msg = format!("{}: type {other} comes in as a transform", prim.path());
            log::debug!("{msg}");
        }
    }
    if ty == "PointInstancer" {
        return Ok(Some(tr));
    }
    let kids = prim.children().map_err(|e| e.to_string())?;
    for k in kids {
        if !k.is_active().unwrap_or(true) {
            continue;
        }
        let sub = proto.as_ref().map(|(root, rel)| (root.clone(), format!("{rel}/{}", k.path().name().unwrap_or(""))));
        read_prim(c, &k, tr, sub)?;
    }
    Ok(Some(tr))
}

/// Each point of a PointInstancer as a transform over instances of its prototype.
fn read_instancer(c: &mut Ctx, prim: &Prim, tr: NodeId) -> Result<(), String> {
    let protos = prim.relationship("prototypes").targets().unwrap_or_default();
    let idx = get_vec::<i32>(&prim.attribute("protoIndices"), None).unwrap_or_default();
    let pos = get_vec::<gf::Vec3f>(&prim.attribute("positions"), None).unwrap_or_default();
    let ori = get_vec::<gf::Quath>(&prim.attribute("orientations"), None).unwrap_or_default();
    let scl = get_vec::<gf::Vec3f>(&prim.attribute("scales"), None).unwrap_or_default();
    for (i, &k) in idx.iter().enumerate() {
        let Some(target) = protos.get(k.max(0) as usize) else { continue };
        let Ok(tp) = c.stage.prim(target.clone()) else { continue };
        let holder = make_transform(c, &format!("instance{i}"), tr)?;
        if let Some(p) = pos.get(i) {
            set_attr(c.g(), holder, "translate", PlugValue::Vec3(DVec3::new(p.x as f64, p.y as f64, p.z as f64)));
        }
        if let Some(q) = ori.get(i) {
            let dq = glam::DQuat::from_xyzw(q.x.to_f64(), q.y.to_f64(), q.z.to_f64(), q.w.to_f64());
            let e = matrix_to_euler(&DMat3::from_quat(dq), RotateOrder::Xyz);
            set_attr(c.g(), holder, "rotate", PlugValue::Vec3(e));
        }
        if let Some(s) = scl.get(i) {
            set_attr(c.g(), holder, "scale", PlugValue::Vec3(DVec3::new(s.x as f64, s.y as f64, s.z as f64)));
        }
        read_prim(c, &tp, holder, Some((target.as_str().to_string(), String::new())))?;
    }
    Ok(())
}

/// The transform that takes the stage's axis and units to the scene's. None when they agree or it isn't wanted.
fn conversion(c: &mut Ctx) -> Option<DMat4> {
    let mut m = DMat4::IDENTITY;
    let mut any = false;
    if c.o.up_axis && c.o.axis_method != "overwritePrefs" {
        let up = c.stage.stage_metadata("upAxis").ok().flatten().and_then(|v| match v {
            Value::Token(t) => Some(t.as_str().to_string()),
            _ => None,
        });
        let scene_z = c.s.graph.up_axis == UpAxis::Z;
        match up.as_deref() {
            Some("Z") if !scene_z => {
                m = DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2);
                any = true;
            }
            Some("Y") if scene_z => {
                m = DMat4::from_rotation_x(std::f64::consts::FRAC_PI_2);
                any = true;
            }
            _ => {}
        }
    }
    if c.o.unit {
        let mpu = c.stage.stage_metadata("metersPerUnit").ok().flatten().and_then(|v| match v {
            Value::Double(d) => Some(d),
            Value::Float(f) => Some(f as f64),
            _ => None,
        });
        // Fallback for a file saying nothing is USD's: centimetres.
        let mpu = mpu.unwrap_or(0.01);
        let ratio = mpu / 0.01;
        if (ratio - 1.0).abs() > 1e-9 {
            m = DMat4::from_scale(DVec3::splat(ratio)) * m;
            any = true;
        }
    }
    any.then_some(m)
}

fn apply_conversion(c: &mut Ctx, roots: &mut [NodeId], parent: NodeId, m: DMat4) -> Result<(), String> {
    // A group holds it, so animated roots stay as they are in the file.
    let holder = make_transform(c, "usdImportRoot", parent)?;
    Transform::set_local_matrix(c.g(), holder, &m).map_err(|e| e.to_string())?;
    for r in roots.iter() {
        c.g().set_parent(*r, holder, false, usize::MAX).map_err(|e| e.to_string())?;
    }
    if let Some(first) = roots.first_mut() {
        *first = holder;
    }
    Ok(())
}

pub fn import(s: &mut Session, o: &ImportOpts) -> Result<Vec<NodeId>, String> {
    if !std::path::Path::new(&o.file).exists() {
        return Err(format!("mayaUSDImport: no file {}", o.file));
    }
    let session_file = variant_session(o, &o.prim_path)?;
    let mut b = Stage::builder().schema_registry(openusd_schemas::schema_registry()).resolver(crate::extras::AztekResolver::new()).load(InitialLoadSet::LoadAll);
    if let Some(f) = &session_file {
        b = b.session_layer(f.to_string_lossy().into_owned());
    }
    let stage = b.open(&o.file).map_err(|e| format!("{}: {e}", o.file))?;
    if let Some(f) = &session_file {
        let _ = std::fs::remove_file(f);
    }
    for e in stage.composition_errors() {
        log::warn!("USD: {e}");
    }

    let parent = if o.parent.is_empty() {
        NO_NODE
    } else {
        match s.graph.find(&o.parent).map_err(|e| e.to_string())? {
            Some(p) => p,
            None => return Err(format!("mayaUSDImport: no node named '{}'", o.parent)),
        }
    };
    let mut c = Ctx { s, o, stage: stage.clone(), nodes: HashMap::new(), meshes: HashMap::new(), protos: HashMap::new(), materials: HashMap::new(), display_groups: HashMap::new(), times: Vec::new(), warnings: Vec::new() };

    // What to start from: the path given, else the default prim, else everything.
    let mut starts: Vec<Prim> = Vec::new();
    let wanted = if !o.prim_path.is_empty() {
        o.prim_path.clone()
    } else {
        stage.default_prim().map_or_else(|| "/".to_string(), |d| format!("/{}", d.as_str()))
    };
    if wanted == "/" {
        for n in stage.root_prims().map_err(|e| e.to_string())? {
            let path = format!("/{}", n.as_str());
            starts.push(stage.prim(path.as_str()).map_err(|e| e.to_string())?);
        }
    } else {
        let p = stage.prim(wanted.as_str()).map_err(|e| e.to_string())?;
        if !p.is_valid().unwrap_or(false) {
            return Err(format!("mayaUSDImport: {} has no prim {wanted}", o.file));
        }
        starts.push(p);
    }

    let mut roots = Vec::new();
    for p in starts {
        if !p.is_active().unwrap_or(true) {
            continue;
        }
        if let Some(id) = read_prim(&mut c, &p, parent, None)? {
            roots.push(id);
        }
    }
    if let Some(m) = conversion(&mut c) {
        apply_conversion(&mut c, &mut roots, parent, m)?;
        roots.truncate(1);
    }
    crate::import_skel::read_all(&mut c, &roots)?;
    if o.shading {
        crate::import_shade::read_all(&mut c)?;
    }
    let mut times = std::mem::take(&mut c.times);
    let warnings = std::mem::take(&mut c.warnings);
    drop(c);
    for w in warnings {
        log::warn!("USD: {w}");
    }
    if !times.is_empty() {
        times.sort_by(f64::total_cmp);
        let (lo, hi) = (times[0], times[times.len() - 1]);
        let _ = s.run("playbackOptions", json!({"minTime": lo, "maxTime": hi, "animationStartTime": lo, "animationEndTime": hi}));
    }
    s.graph.select(&roots, SelectMode::Replace);
    Ok(roots)
}

/// One prim of a stage that's already open, with what's below it, read under `parent`. Returns its top transform.
pub fn read_stage_prim(s: &mut Session, o: &ImportOpts, stage: &Stage, prim: &Prim, parent: NodeId) -> Result<NodeId, String> {
    let mut c = Ctx { s, o, stage: stage.clone(), nodes: HashMap::new(), meshes: HashMap::new(), protos: HashMap::new(), materials: HashMap::new(), display_groups: HashMap::new(), times: Vec::new(), warnings: Vec::new() };
    let top = read_prim(&mut c, prim, parent, None)?.ok_or_else(|| format!("{} isn't a prim that reads into nodes", prim.path()))?;
    let roots = [top];
    crate::import_skel::read_all(&mut c, &roots)?;
    if o.shading {
        crate::import_shade::read_all(&mut c)?;
    }
    for w in std::mem::take(&mut c.warnings) {
        log::warn!("USD: {w}");
    }
    Ok(top)
}

pub fn run(s: &mut Session, a: &Args) -> CmdResult {
    let o = opts_of(a)?;
    s.graph.open_chunk("mayaUSDImport");
    let r = import(s, &o);
    s.graph.close_chunk();
    let roots = r.map_err(aztek_ops::CommandError::new)?;
    let names: Vec<J> = roots.iter().map(|&n| json!(s.graph.path(n))).collect();
    Ok(J::Array(names))
}

pub fn register(c: &mut Commands<Session>) {
    let b = |n: &str, sn: &str, d: &str| FlagDef::new(n, sn, ArgType::Any, J::Null, d);
    let mut def = CommandDef::new("mayaUSDImport", "Read a USD file into the scene as nodes. Flags as the reference's USD plug-in's. Returns the new root transforms.", run);
    for (n, sn, d, arity) in [
        ("file", "f", "The USD file to read.", 0u8),
        ("primPath", "pp", "The prim to read with what's below it; / for everything. Empty reads the default prim, or everything if there is none.", 0),
        ("readAnimData", "ani", "Time samples become keys (off).", 0),
        ("frameRange", "fr", "Only time samples in this range.", 2),
        ("parent", "p", "A transform to read under.", 0),
        ("shadingMode", "shd", "none, or useRegistry UsdPreviewSurface (the default).", 2),
        ("preferredMaterial", "prm", "lambert, blinn, phong or standardSurface; empty chooses from the shader.", 0),
        ("variant", "var", "A variant set and selection on the prim read.", 2),
        ("primVariant", "pv", "path=set:selection, for any prim.", 1),
        ("importInstances", "ii", "USD instances become shape instances (on).", 0),
        ("upAxis", "upa", "Turn a file on another up axis to the scene's (on).", 0),
        ("unit", "unt", "Scale a file in other units to centimetres (on).", 0),
        ("axisAndUnitMethod", "aum", "rotateScale or overwritePrefs.", 0),
        ("metadata", "md", "Metadata kept as attributes: hidden, instanceable, kind.", 1),
        ("excludePrimvar", "epv", "Primvars not to read.", 1),
        ("remapUVSetsTo", "ruv", "Pairs of a primvar and the UV set it becomes.", 2),
        ("apiSchema", "api", "Accepted.", 1),
        ("jobContext", "jc", "Accepted.", 1),
        ("chaser", "chr", "Accepted.", 1),
        ("useAsAnimationCache", "uac", "Accepted: animated points read their first sample.", 0),
        ("importUSDZTextures", "itx", "Accepted.", 0),
        ("importRelativeTextures", "rtx", "Accepted.", 0),
        ("verbose", "v", "Accepted.", 0),
    ] {
        let f = if n == "verbose" { FlagDef::new(n, sn, ArgType::Bool, J::Null, d) } else { b(n, sn, d) };
        def = def.flag(if arity > 0 { f.multi(arity) } else { f });
    }
    c.register(def);
    let _ = mesh_nodes::primitive_stem;
}
