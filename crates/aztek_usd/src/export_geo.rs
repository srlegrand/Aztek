//! What the exporter writes for each shape: meshes, cameras, curves, surfaces, instances, proxy shapes and custom attributes.

use std::collections::HashMap;
use std::sync::Arc;

use aztek_geo::attribute::names;
use aztek_geo::normals::{corner_normals, CornerNormalOptions};
use aztek_geo::nurbs::Form;
use aztek_geo::{colors, Mesh};
use aztek_scene::plug::{flags, NodeId, PlugRef, PlugValue, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::{curves, surfaces};
use glam::DVec3;
use openusd::gf;
use openusd::sdf::{FieldKey, ListOp, Reference, Specifier, Value};

use crate::export::{Item, Opts, Tracks};
use crate::export_skel::Plan;
use crate::out::{Out, Track};
use crate::util::*;

/// The mesh with no gaps in its numbering, as USD's arrays need.
pub(crate) fn clean(m: Arc<Mesh>) -> Arc<Mesh> {
    if m.has_deleted() {
        let mut c = (*m).clone();
        c.compact(None);
        Arc::new(c)
    } else {
        m
    }
}

fn points_of(m: &Mesh) -> Vec<gf::Vec3f> {
    m.positions().iter().map(|p| v3f(*p)).collect()
}

fn extent_of(points: impl Iterator<Item = DVec3>) -> Value {
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    let mut any = false;
    for p in points {
        lo = lo.min(p);
        hi = hi.max(p);
        any = true;
    }
    if !any {
        lo = DVec3::ZERO;
        hi = DVec3::ZERO;
    }
    Value::Vec3fVec(vec![v3f(lo), v3f(hi)])
}

fn scheme_of(s: &mut Session, o: &Opts, shape: NodeId) -> String {
    get_str(&mut s.graph, shape, "USD_subdivisionScheme").filter(|x| !x.is_empty()).unwrap_or_else(|| o.default_scheme.clone())
}

fn corner_order(m: &Mesh) -> Vec<u32> {
    let mut out = Vec::with_capacity(m.num_corners() as usize);
    for f in 0..m.num_faces() {
        out.extend(m.face_corners(f));
    }
    out
}

pub fn sample_mesh(s: &mut Session, o: &Opts, it: &Item, time: Option<f64>, t: &mut Tracks, plan: &Plan) {
    // Skinned or blended meshes hold their rest points: UsdSkel poses them.
    if plan.rest.contains_key(&it.shape) {
        return;
    }
    let Some(m) = crate::util::evaluated_mesh(&mut s.graph, it.shape) else { return };
    let m = clean(m);
    t.put(&it.shape_path, "points", "point3f[]", time, Value::Vec3fVec(points_of(&m)));
    t.put(&it.shape_path, "extent", "float3[]", time, extent_of(m.positions().iter().copied()));
    if scheme_of(s, o, it.shape) == "none" {
        let n = corner_normals(&m, &CornerNormalOptions::default());
        let list: Vec<gf::Vec3f> = corner_order(&m).iter().map(|&c| { let v = n[c as usize]; gf::vec3f(v.x, v.y, v.z) }).collect();
        t.put(&it.shape_path, "normals", "normal3f[]", time, Value::Vec3fVec(list));
        t.slot(&it.shape_path, "normals", "normal3f[]").interpolation = Some("faceVarying".into());
    }
}

/// The primvar name a UV set is written as.
fn uv_primvar(o: &Opts, set: &str, ordinal: usize) -> String {
    if let Some((_, to)) = o.remap_uv.iter().find(|(from, _)| from == set) {
        return to.clone();
    }
    if ordinal == 0 && set == "map1" {
        return "st".into();
    }
    if o.preserve_uv_names {
        return prim_name(set, false);
    }
    if ordinal == 0 {
        "st".into()
    } else {
        format!("st{ordinal}")
    }
}

/// Values with duplicates folded: (distinct values, an index for each input).
fn index_vec2(values: impl Iterator<Item = [f32; 2]>) -> (Vec<gf::Vec2f>, Vec<i32>) {
    let mut seen: HashMap<[u32; 2], i32> = HashMap::new();
    let (mut out, mut idx) = (Vec::new(), Vec::new());
    for v in values {
        let k = [v[0].to_bits(), v[1].to_bits()];
        let i = *seen.entry(k).or_insert_with(|| {
            out.push(gf::vec2f(v[0], v[1]));
            out.len() as i32 - 1
        });
        idx.push(i);
    }
    (out, idx)
}

pub fn write_mesh_static(s: &mut Session, o: &Opts, it: &Item, out: &mut Out, t: &mut Tracks, plan: &Plan) -> Result<(), String> {
    let shape = it.shape;
    let prim = it.shape_path.clone();
    let m = match plan.rest.get(&shape) {
        Some(r) => r.clone(),
        None => match crate::util::evaluated_mesh(&mut s.graph, shape) {
            Some(m) => clean(m),
            None => return Ok(()),
        },
    };
    write_mesh_topology(s, o, shape, &prim, &m, out)?;
    if plan.rest.contains_key(&shape) {
        t.put(&prim, "points", "point3f[]", None, Value::Vec3fVec(points_of(&m)));
        t.put(&prim, "extent", "float3[]", None, extent_of(m.positions().iter().copied()));
    }
    write_custom(s, shape, &prim, out)?;
    Ok(())
}

pub(crate) fn write_mesh_topology(s: &mut Session, o: &Opts, shape: NodeId, prim: &str, m: &Mesh, out: &mut Out) -> Result<(), String> {
    let mut counts = Vec::with_capacity(m.num_faces() as usize);
    let mut indices = Vec::with_capacity(m.num_corners() as usize);
    for f in 0..m.num_faces() {
        let pts = m.face_points(f);
        counts.push(pts.len() as i32);
        indices.extend(pts.iter().map(|&p| p as i32));
    }
    out.attr(prim, "faceVertexCounts", "int[]", Value::IntVec(counts))?;
    out.attr(prim, "faceVertexIndices", "int[]", Value::IntVec(indices))?;
    let scheme = scheme_of(s, o, shape);
    out.uniform(prim, "subdivisionScheme", "token", Value::Token(scheme.as_str().into()))?;
    let double = get_f64(&mut s.graph, shape, "doubleSided").is_none_or(|v| v != 0.0);
    out.uniform(prim, "doubleSided", "bool", Value::Bool(double))?;

    // Invisible faces.
    let holes: Vec<i32> = if m.face_attrs().has("hole") {
        m.face_attrs().get::<bool>("hole").iter().enumerate().filter(|(_, h)| **h).map(|(i, _)| i as i32).collect()
    } else {
        Vec::new()
    };
    if !holes.is_empty() {
        out.attr(prim, "holeIndices", "int[]", Value::IntVec(holes))?;
    }

    // Creases.
    if m.edge_attrs().has(names::CREASE) {
        let cr = m.edge_attrs().get::<f32>(names::CREASE);
        let (mut ci, mut cl, mut cs) = (Vec::new(), Vec::new(), Vec::new());
        for e in 0..m.num_edges() {
            let v = cr[e as usize];
            if v > 0.0 {
                ci.push(m.edge_point(e, 0) as i32);
                ci.push(m.edge_point(e, 1) as i32);
                cl.push(2);
                cs.push(v);
            }
        }
        if !cl.is_empty() {
            out.attr(prim, "creaseIndices", "int[]", Value::IntVec(ci))?;
            out.attr(prim, "creaseLengths", "int[]", Value::IntVec(cl))?;
            out.attr(prim, "creaseSharpnesses", "float[]", Value::FloatVec(cs))?;
        }
    }

    let order = corner_order(m);
    if o.export_uvs {
        write_uvs(o, prim, m, &order, out)?;
    }
    if o.export_color_sets {
        write_color_sets(prim, m, &order, out)?;
    }
    Ok(())
}

fn write_uvs(o: &Opts, prim: &str, m: &Mesh, order: &[u32], out: &mut Out) -> Result<(), String> {
    let current = m.face_attrs().names().into_iter().find_map(|n| n.strip_prefix("uvcurrent:").map(str::to_string)).unwrap_or_else(|| "map1".to_string());
    let mut sets: Vec<(String, String)> = Vec::new();
    if m.corner_attrs().has(&names::uv(0)) {
        sets.push((current, names::uv(0)));
    }
    for n in m.corner_attrs().names() {
        if let Some(set) = n.strip_prefix("uvset:") {
            sets.push((set.to_string(), n.clone()));
        }
    }
    for (i, (set, layer)) in sets.iter().enumerate() {
        let uv = m.corner_attrs().get::<[f32; 2]>(layer);
        let (values, idx) = index_vec2(order.iter().map(|&c| uv[c as usize]));
        let t = Track { default: Some(Value::Vec2fVec(values)), samples: Vec::new() };
        out.primvar(prim, &uv_primvar(o, set, i), "texCoord2f[]", &t, "faceVarying", Some(idx), 1)?;
    }
    Ok(())
}

fn write_color_sets(prim: &str, m: &Mesh, order: &[u32], out: &mut Out) -> Result<(), String> {
    for set in colors::sets(m) {
        let vals = colors::values(m, &set);
        if vals.is_empty() {
            continue;
        }
        let rgb: Vec<gf::Vec3f> = order.iter().map(|&c| { let v = vals[c as usize]; gf::vec3f(v[0], v[1], v[2]) }).collect();
        let has_alpha = order.iter().any(|&c| vals[c as usize][3] != 1.0);
        if set == "displayColor" {
            let t = Track { default: Some(Value::Vec3fVec(rgb)), samples: Vec::new() };
            out.primvar(prim, "displayColor", "color3f[]", &t, "faceVarying", None, 1)?;
            if has_alpha {
                let a: Vec<f32> = order.iter().map(|&c| vals[c as usize][3]).collect();
                let t = Track { default: Some(Value::FloatVec(a)), samples: Vec::new() };
                out.primvar(prim, "displayOpacity", "float[]", &t, "faceVarying", None, 1)?;
            }
        } else if has_alpha {
            let rgba: Vec<gf::Vec4f> = order.iter().map(|&c| { let v = vals[c as usize]; gf::vec4f(v[0], v[1], v[2], v[3]) }).collect();
            let t = Track { default: Some(Value::Vec4fVec(rgba)), samples: Vec::new() };
            out.primvar(prim, &prim_name(&set, false), "color4f[]", &t, "faceVarying", None, 1)?;
        } else {
            let t = Track { default: Some(Value::Vec3fVec(rgb)), samples: Vec::new() };
            out.primvar(prim, &prim_name(&set, false), "color3f[]", &t, "faceVarying", None, 1)?;
        }
    }
    Ok(())
}

pub fn sample_camera(s: &mut Session, it: &Item, time: Option<f64>, t: &mut Tracks) {
    let g = &mut s.graph;
    let c = it.shape;
    let p = it.shape_path.clone();
    const INCH: f64 = 25.4;
    let hfa = get_f64(g, c, "horizontalFilmAperture").unwrap_or(1.41732);
    let vfa = get_f64(g, c, "verticalFilmAperture").unwrap_or(0.94488);
    let fl = get_f64(g, c, "focalLength").unwrap_or(35.0);
    let ortho = get_f64(g, c, "orthographic").unwrap_or(0.0) != 0.0;
    let (ha, va) = if ortho {
        // An orthographic aperture is the view's width, in tenths of a scene unit.
        let w = get_f64(g, c, "orthographicWidth").unwrap_or(30.0) * 10.0;
        (w, w * vfa / hfa.max(1e-9))
    } else {
        (hfa * INCH, vfa * INCH)
    };
    t.put(&p, "focalLength", "float", time, Value::Float(fl as f32));
    t.put(&p, "horizontalAperture", "float", time, Value::Float(ha as f32));
    t.put(&p, "verticalAperture", "float", time, Value::Float(va as f32));
    let near = get_f64(g, c, "nearClipPlane").unwrap_or(0.1);
    let far = get_f64(g, c, "farClipPlane").unwrap_or(10000.0);
    t.put(&p, "clippingRange", "float2", time, Value::Vec2f(gf::vec2f(near as f32, far as f32)));
    t.put(&p, "focusDistance", "float", time, Value::Float(get_f64(g, c, "centerOfInterest").unwrap_or(5.0) as f32));
    if let Some(fs) = get_f64(g, c, "fStop") {
        t.put(&p, "fStop", "float", time, Value::Float(fs as f32));
    }
    t.put(&p, "projection", "token", time, Value::Token(if ortho { "orthographic".into() } else { "perspective".into() }));
}

/// A curve's or surface's knots as USD counts them: the reference's list has one fewer at each end.
fn usd_knots(k: &[f64], form: Form) -> Vec<f64> {
    if k.is_empty() {
        return Vec::new();
    }
    let n = k.len();
    let (front, back) = if form == Form::Periodic && n >= 2 {
        (k[0] - (k[n - 1] - k[n - 2]), k[n - 1] + (k[1] - k[0]))
    } else {
        (k[0], k[n - 1])
    };
    let mut out = Vec::with_capacity(n + 2);
    out.push(front);
    out.extend_from_slice(k);
    out.push(back);
    out
}

pub fn sample_curve(s: &mut Session, it: &Item, time: Option<f64>, t: &mut Tracks) {
    let Some(c) = curves::local_curve(&mut s.graph, it.shape) else { return };
    let p = &it.shape_path;
    let pts: Vec<gf::Vec3f> = c.cvs.iter().map(|q| v3f(*q)).collect();
    t.put(p, "points", "point3f[]", time, Value::Vec3fVec(pts));
    t.put(p, "extent", "float3[]", time, extent_of(c.cvs.iter().copied()));
    t.put(p, "curveVertexCounts", "int[]", time, Value::IntVec(vec![c.cvs.len() as i32]));
    t.put(p, "order", "int[]", time, Value::IntVec(vec![c.degree as i32 + 1]));
    t.put(p, "knots", "double[]", time, Value::DoubleVec(usd_knots(&c.knots, c.form)));
    let (lo, hi) = c.domain();
    t.put(p, "ranges", "double2[]", time, Value::Vec2dVec(vec![gf::vec2d(lo, hi)]));
    t.put(p, "widths", "float[]", time, Value::FloatVec(vec![1.0]));
    t.slot(p, "widths", "float[]").interpolation = Some("constant".into());
}

fn form_token(f: Form) -> Value {
    Value::Token(match f {
        Form::Open => "open".into(),
        Form::Closed => "closed".into(),
        Form::Periodic => "periodic".into(),
    })
}

pub fn sample_surface(s: &mut Session, it: &Item, time: Option<f64>, t: &mut Tracks) {
    let Some(c) = surfaces::local_surface(&mut s.graph, it.shape) else { return };
    let p = &it.shape_path;
    // Aztek runs v fastest within u; USD runs u fastest within v.
    let mut pts = Vec::with_capacity(c.cvs.len());
    for v in 0..c.nv {
        for u in 0..c.nu {
            pts.push(v3f(c.cvs[u * c.nv + v]));
        }
    }
    t.put(p, "points", "point3f[]", time, Value::Vec3fVec(pts));
    t.put(p, "extent", "float3[]", time, extent_of(c.cvs.iter().copied()));
    t.put(p, "uVertexCount", "int", time, Value::Int(c.nu as i32));
    t.put(p, "vVertexCount", "int", time, Value::Int(c.nv as i32));
    t.put(p, "uOrder", "int", time, Value::Int(c.degree_u as i32 + 1));
    t.put(p, "vOrder", "int", time, Value::Int(c.degree_v as i32 + 1));
    t.put(p, "uKnots", "double[]", time, Value::DoubleVec(usd_knots(&c.knots_u, c.form_u)));
    t.put(p, "vKnots", "double[]", time, Value::DoubleVec(usd_knots(&c.knots_v, c.form_v)));
    t.put(p, "uForm", "token", time, form_token(c.form_u));
    t.put(p, "vForm", "token", time, form_token(c.form_v));
    let range = |k: &[f64], d: usize, n: usize| gf::vec2d(k.get(d.wrapping_sub(1)).copied().unwrap_or(0.0), k.get(n.wrapping_sub(1)).copied().unwrap_or(1.0));
    t.put(p, "uRange", "double2", time, Value::Vec2d(range(&c.knots_u, c.degree_u, c.nu)));
    t.put(p, "vRange", "double2", time, Value::Vec2d(range(&c.knots_v, c.degree_v, c.nv)));
}

/// The source mesh of every instance gets a prim under the hidden root the reference exporter uses.
pub fn write_instance_prototypes(s: &mut Session, o: &Opts, items: &[Item], base: &str, out: &mut Out) -> Result<HashMap<NodeId, String>, String> {
    let mut map = HashMap::new();
    let sources: Vec<NodeId> = {
        let mut v: Vec<NodeId> = items.iter().filter_map(|i| i.instance_of).collect();
        v.sort_unstable();
        v.dedup();
        v
    };
    if sources.is_empty() {
        return Ok(map);
    }
    let root = format!("{base}/MayaExportedInstanceSources");
    out.prim(&root, Specifier::Over, "")?;
    for src in sources {
        let name = prim_name(s.graph.node(src).map_or("mesh", |n| n.name()), o.strip_namespaces);
        let path = format!("{root}/{name}");
        out.prim(&path, Specifier::Def, "Mesh")?;
        let Some(m) = crate::util::evaluated_mesh(&mut s.graph, src) else { continue };
        let m = clean(m);
        write_mesh_topology(s, o, src, &path, &m, out)?;
        out.attr(&path, "points", "point3f[]", Value::Vec3fVec(points_of(&m)))?;
        out.attr(&path, "extent", "float3[]", extent_of(m.positions().iter().copied()))?;
        map.insert(src, path);
    }
    Ok(map)
}

pub fn write_instance_arc(out: &mut Out, prim: &str, proto: &str) -> Result<(), String> {
    out.prim_field(prim, FieldKey::Instanceable, Value::Bool(true))?;
    let r = Reference { prim_path: crate::out::p(proto)?, ..Default::default() };
    out.prim_field(prim, FieldKey::References, Value::ReferenceListOp(ListOp::prepended(vec![r])))
}

/// A proxy shape is written as a reference to its file, with the prim it shows.
pub fn write_proxy(s: &mut Session, o: &Opts, it: &Item, out: &mut Out) -> Result<(), String> {
    if !o.stages_as_refs {
        return Ok(());
    }
    let file = get_str(&mut s.graph, it.shape, "filePath").unwrap_or_default();
    if file.is_empty() {
        return Ok(());
    }
    let written = crate::export_mat::texture_path(o, &file);
    let prim = get_str(&mut s.graph, it.shape, "primPath").unwrap_or_default();
    let target = if !prim.is_empty() && prim != "/" { Some(crate::out::p(&prim)?) } else { None };
    if o.stages_as_payloads {
        let mut r = openusd::sdf::Payload { asset_path: written, ..Default::default() };
        if let Some(t) = target {
            r.prim_path = t;
        }
        return out.prim_field(&it.path, FieldKey::Payload, Value::PayloadListOp(ListOp::prepended(vec![r])));
    }
    let mut r = Reference { asset_path: written, ..Default::default() };
    if let Some(t) = target {
        r.prim_path = t;
    }
    out.prim_field(&it.path, FieldKey::References, Value::ReferenceListOp(ListOp::prepended(vec![r])))
}

fn plug_value(v: &PlugValue) -> Option<(&'static str, Value)> {
    Some(match v {
        PlugValue::Bool(b) => ("bool", Value::Bool(*b)),
        PlugValue::Int(i) => ("int", Value::Int(*i as i32)),
        PlugValue::Float(f) => ("double", Value::Double(*f)),
        PlugValue::String(x) => ("string", Value::String(x.to_string())),
        PlugValue::Vec3(x) => ("double3", Value::Vec3d(gf::vec3d(x.x, x.y, x.z))),
        _ => return None,
    })
}

/// Attributes tagged for USD: `USD_ATTR_x` writes `x`; `USD_UserExportedAttributesJson` lists others with their USD names;
/// `USD_kind` and `USD_purpose` set the prim's kind and purpose.
pub fn write_custom(s: &mut Session, node: NodeId, prim: &str, out: &mut Out) -> Result<(), String> {
    if node == NO_NODE {
        return Ok(());
    }
    let names: Vec<(usize, String)> = match s.graph.node(node) {
        Some(n) => n.ty().attrs.iter().enumerate().filter(|(_, a)| a.flags & flags::USER != 0).map(|(i, a)| (i, a.name.clone())).collect(),
        None => return Ok(()),
    };
    if names.is_empty() {
        return Ok(());
    }
    let mut listed: HashMap<String, (String, String)> = HashMap::new();
    if let Some(text) = get_str(&mut s.graph, node, "USD_UserExportedAttributesJson") {
        if let Ok(serde_json::Value::Object(map)) = serde_json::from_str::<serde_json::Value>(&text) {
            for (attr, cfg) in map {
                let to = cfg.get("usdAttrName").and_then(|v| v.as_str()).unwrap_or("").to_string();
                let ty = cfg.get("usdAttrType").and_then(|v| v.as_str()).unwrap_or("").to_string();
                listed.insert(attr, (to, ty));
            }
        }
    }
    for (i, name) in names {
        let value = s.graph.value(&PlugRef::new(node, i as i32)).map_err(|e| e.to_string())?;
        match name.as_str() {
            "USD_UserExportedAttributesJson" | "USD_subdivisionScheme" => continue,
            "USD_kind" => {
                if let PlugValue::String(k) = &value {
                    out.prim_field(prim, FieldKey::Kind, Value::Token(k.as_str().into()))?;
                }
                continue;
            }
            "USD_purpose" => {
                if let PlugValue::String(k) = &value {
                    out.uniform(prim, "purpose", "token", Value::Token(k.as_str().into()))?;
                }
                continue;
            }
            _ => {}
        }
        let target = if let Some(rest) = name.strip_prefix("USD_ATTR_") {
            rest.to_string()
        } else if let Some((to, ty)) = listed.get(&name) {
            let base = if to.is_empty() { name.clone() } else { to.clone() };
            match ty.as_str() {
                "primvar" => format!("primvars:{base}"),
                _ if !to.is_empty() => base,
                _ => format!("userProperties:{base}"),
            }
        } else {
            continue;
        };
        if let Some((ty, v)) = plug_value(&value) {
            let pp = out.attr(prim, &target, ty, v)?;
            out.attr_field(&pp, FieldKey::Custom, Value::Bool(true));
        }
    }
    Ok(())
}
