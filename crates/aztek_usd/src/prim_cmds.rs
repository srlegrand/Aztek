//! `usdPrim` and `usdAttr`: authoring and reading prims by name, and `getAttr`, `setAttr`, `delete`, `rename` taking prim
//! names. Everything authors on the stage's edit target and joins the scene's undo stack.

use std::collections::HashMap;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use aztek_scene::session::Session;
use openusd::gf;
use openusd::sdf::{self, ChildrenKey, ListOp, Payload, Reference, Value};
use openusd::tf::Token;
use openusd::usd::{LoadPolicy, NamespaceEditor, Prim, Stage, TimeCode};
use serde_json::{json, Map, Value as J};

use crate::prims::{err, name_of, resolve, split_name, Target};
use crate::stages;

/// A value as JSON for scripts and panels.
pub fn value_json(v: &Value) -> J {
    match v {
        Value::Bool(b) => json!(b),
        Value::Int(i) => json!(i),
        Value::Uint(i) => json!(i),
        Value::Int64(i) => json!(i),
        Value::Uint64(i) => json!(i),
        Value::Float(f) => json!(f),
        Value::Double(f) => json!(f),
        Value::Half(f) => json!(f.to_f64()),
        Value::String(s) => json!(s),
        Value::Token(t) => json!(t.as_str()),
        Value::AssetPath(a) => json!(a.asset_path()),
        Value::Vec2f(x) => json!([x.x, x.y]),
        Value::Vec2d(x) => json!([x.x, x.y]),
        Value::Vec3f(x) => json!([x.x, x.y, x.z]),
        Value::Vec3d(x) => json!([x.x, x.y, x.z]),
        Value::Vec4f(x) => json!([x.x, x.y, x.z, x.w]),
        Value::Vec4d(x) => json!([x.x, x.y, x.z, x.w]),
        Value::Quatf(q) => json!([q.w, q.x, q.y, q.z]),
        Value::Quatd(q) => json!([q.w, q.x, q.y, q.z]),
        Value::Matrix4d(m) => json!(m.0.to_vec()),
        Value::TokenVec(l) => json!(l.iter().map(|t| t.as_str()).collect::<Vec<_>>()),
        Value::StringVec(l) => json!(l),
        Value::IntVec(l) if l.len() <= 16 => json!(l),
        Value::FloatVec(l) if l.len() <= 16 => json!(l),
        Value::DoubleVec(l) if l.len() <= 16 => json!(l),
        Value::IntVec(l) => json!(format!("int[{}]", l.len())),
        Value::FloatVec(l) => json!(format!("float[{}]", l.len())),
        Value::DoubleVec(l) => json!(format!("double[{}]", l.len())),
        Value::Vec3fVec(l) if l.len() <= 4 => json!(l.iter().map(|x| [x.x, x.y, x.z]).collect::<Vec<_>>()),
        Value::Vec3fVec(l) => json!(format!("vec3f[{}]", l.len())),
        Value::Vec2fVec(l) => json!(format!("vec2f[{}]", l.len())),
        Value::Matrix4dVec(l) => json!(format!("matrix4d[{}]", l.len())),
        other => json!(format!("{other:?}").chars().take(80).collect::<String>()),
    }
}

/// A JSON value as the attribute's declared type.
pub fn json_value(ty: &str, v: &J) -> Result<Value, String> {
    let nums = || -> Vec<f64> { v.as_array().map_or_else(|| v.as_f64().into_iter().collect(), |a| a.iter().filter_map(J::as_f64).collect()) };
    let n = nums();
    let need = |k: usize| -> Result<(), String> {
        if n.len() < k {
            Err(format!("a {ty} needs {k} numbers"))
        } else {
            Ok(())
        }
    };
    let f = |i: usize| n[i] as f32;
    Ok(match ty {
        "bool" => Value::Bool(v.as_bool().unwrap_or(v.as_f64().unwrap_or(0.0) != 0.0)),
        "int" => {
            need(1)?;
            Value::Int(n[0] as i32)
        }
        "float" => {
            need(1)?;
            Value::Float(f(0))
        }
        "double" | "timecode" => {
            need(1)?;
            Value::Double(n[0])
        }
        "string" => Value::String(v.as_str().unwrap_or("").to_string()),
        "token" => Value::Token(v.as_str().unwrap_or("").into()),
        "asset" => Value::AssetPath(sdf::AssetPath::new(v.as_str().unwrap_or(""))),
        "float2" | "texCoord2f" => {
            need(2)?;
            Value::Vec2f(gf::vec2f(f(0), f(1)))
        }
        "double2" => {
            need(2)?;
            Value::Vec2d(gf::vec2d(n[0], n[1]))
        }
        "float3" | "color3f" | "point3f" | "vector3f" | "normal3f" => {
            need(3)?;
            Value::Vec3f(gf::vec3f(f(0), f(1), f(2)))
        }
        "double3" | "color3d" | "point3d" | "vector3d" | "normal3d" => {
            need(3)?;
            Value::Vec3d(gf::vec3d(n[0], n[1], n[2]))
        }
        "float4" | "color4f" => {
            need(4)?;
            Value::Vec4f(gf::vec4f(f(0), f(1), f(2), f(3)))
        }
        other => return Err(format!("setting a {other} isn't supported")),
    })
}

fn tokens_field(v: Option<Value>) -> Vec<String> {
    match v {
        Some(Value::TokenVec(l)) => l.iter().map(|t| t.as_str().to_string()).collect(),
        Some(Value::StringVec(l)) => l,
        _ => Vec::new(),
    }
}

/// Variant sets on a prim, with their options, from every spec that contributes.
pub fn variant_sets(prim: &Prim) -> Vec<(String, Vec<String>, String)> {
    let mut names: Vec<String> = Vec::new();
    let mut options: HashMap<String, Vec<String>> = HashMap::new();
    let stage = prim.stage().clone();
    for site in prim.prim_stack().unwrap_or_default() {
        let Some(layer) = stage.layer(&site.layer) else { continue };
        let d = layer.data();
        let sets = tokens_field(d.try_field(&site.path, ChildrenKey::VariantSetChildren.as_str()).ok().flatten().map(|c| c.into_owned()));
        for set in sets {
            if !names.contains(&set) {
                names.push(set.clone());
            }
            if let Ok(vp) = site.path.append_variant_selection(&set, "") {
                let opts = tokens_field(d.try_field(&vp, ChildrenKey::VariantChildren.as_str()).ok().flatten().map(|c| c.into_owned()));
                let e = options.entry(set.clone()).or_default();
                for o in opts {
                    if !e.contains(&o) {
                        e.push(o);
                    }
                }
            }
        }
    }
    let chosen: HashMap<String, String> = prim.variant_sets().get_all_variant_selections().unwrap_or_default().into_iter().collect();
    names.into_iter().map(|n| { let o = options.remove(&n).unwrap_or_default(); let c = chosen.get(&n).cloned().unwrap_or_default(); (n, o, c) }).collect()
}

/// Arcs authored on a prim: references and payloads, for the panel.
fn arcs(prim: &Prim) -> (Vec<J>, Vec<J>) {
    let (mut refs, mut pays) = (Vec::new(), Vec::new());
    let stage = prim.stage().clone();
    for site in prim.prim_stack().unwrap_or_default() {
        let Some(layer) = stage.layer(&site.layer) else { continue };
        let d = layer.data();
        if let Ok(Some(v)) = d.try_field(&site.path, "references") {
            if let Value::ReferenceListOp(l) = v.as_ref() {
                for r in l.prepended_items.iter().chain(&l.appended_items).chain(&l.explicit_items).chain(&l.added_items) {
                    refs.push(json!({"asset": r.asset_path, "prim": r.prim_path.as_str(), "layer": site.layer}));
                }
            }
        }
        if let Ok(Some(v)) = d.try_field(&site.path, "payload") {
            match v.as_ref() {
                Value::PayloadListOp(l) => {
                    for r in l.prepended_items.iter().chain(&l.appended_items).chain(&l.explicit_items).chain(&l.added_items) {
                        pays.push(json!({"asset": r.asset_path, "prim": r.prim_path.as_str(), "layer": site.layer}));
                    }
                }
                Value::Payload(r) => pays.push(json!({"asset": r.asset_path, "prim": r.prim_path.as_str(), "layer": site.layer})),
                _ => {}
            }
        }
    }
    (refs, pays)
}

/// Everything the panels show about a prim.
pub fn prim_info(prim: &Prim, time: f64) -> J {
    let mut attrs = Vec::new();
    for a in prim.attributes().unwrap_or_default() {
        let ty = a.type_name().ok().flatten().map(|t| t.to_string()).unwrap_or_default();
        let n = a.num_time_samples().unwrap_or(0);
        let value = a.get_at::<Value>(TimeCode::new(time)).ok().flatten();
        attrs.push(json!({
            "name": a.name(),
            "type": ty,
            "value": value.as_ref().map_or(J::Null, value_json),
            "samples": n,
            "authored": a.has_authored_value().unwrap_or(false),
            "custom": a.is_custom().unwrap_or(false),
        }));
    }
    let vs: Vec<J> = variant_sets(prim).into_iter().map(|(n, o, c)| json!({"name": n, "options": o, "selection": c})).collect();
    let (refs, pays) = arcs(prim);
    let idx = prim.prim_index();
    let _ = idx;
    json!({
        "path": prim.path().as_str(),
        "type": prim.type_name().ok().flatten().map_or(String::new(), |t| t.as_str().to_string()),
        "specifier": format!("{:?}", prim.specifier().ok().flatten()),
        "kind": prim.kind().ok().flatten().map_or(String::new(), |t| t.as_str().to_string()),
        "active": prim.is_active().unwrap_or(true),
        "loaded": prim.is_loaded().unwrap_or(true),
        "hasPayload": !pays.is_empty(),
        "instanceable": prim.is_instanceable().unwrap_or(false),
        "instance": prim.is_instance().unwrap_or(false),
        "attributes": attrs,
        "variantSets": vs,
        "references": refs,
        "payloads": pays,
        "layers": prim.prim_stack().unwrap_or_default().iter().map(|x| x.layer.clone()).collect::<Vec<_>>(),
        "apiSchemas": prim.api_schemas().unwrap_or_default().iter().map(|t| t.as_str().to_string()).collect::<Vec<_>>(),
    })
}

/// A prim's children for the outliner: name, type, active, and whether it has children of its own.
pub fn prim_children(stage: &Stage, parent: &str) -> Result<J, String> {
    let names: Vec<Token> = if parent == "/" {
        stage.root_prims().map_err(|e| e.to_string())?
    } else {
        stage.prim(parent).map_err(|e| e.to_string())?.child_names().map_err(|e| e.to_string())?
    };
    let mut out = Vec::with_capacity(names.len());
    for n in names {
        let path = if parent == "/" { format!("/{}", n.as_str()) } else { format!("{parent}/{}", n.as_str()) };
        let p = stage.prim(path.as_str()).map_err(|e| e.to_string())?;
        out.push(json!({
            "name": n.as_str(),
            "path": path,
            "type": p.type_name().ok().flatten().map_or(String::new(), |t| t.as_str().to_string()),
            "active": p.is_active().unwrap_or(true),
            "loaded": p.is_loaded().unwrap_or(true),
            "instance": p.is_instance().unwrap_or(false),
            "kids": p.child_names().map(|c| !c.is_empty()).unwrap_or(false),
        }));
    }
    Ok(J::Array(out))
}

fn add_reference(prim: Prim, asset: &str, target: &str, payload: bool) -> Result<(), String> {
    let prim_path = if target.is_empty() { sdf::Path::default() } else { sdf::Path::new(target).map_err(|e| e.to_string())? };
    if payload {
        let item = Payload { asset_path: asset.to_string(), prim_path, ..Default::default() };
        prim.update_metadata("payload", move |cur| {
            let mut l = match cur {
                Some(Value::PayloadListOp(l)) => l,
                _ => ListOp::default(),
            };
            l.prepended_items.insert(0, item);
            Some(Value::PayloadListOp(l))
        })
        .map_err(|e| e.to_string())?;
    } else {
        let item = Reference { asset_path: asset.to_string(), prim_path, ..Default::default() };
        prim.update_metadata("references", move |cur| {
            let mut l = match cur {
                Some(Value::ReferenceListOp(l)) => l,
                _ => ListOp::default(),
            };
            l.prepended_items.insert(0, item);
            Some(Value::ReferenceListOp(l))
        })
        .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn set_variant(prim: Prim, set: &str, selection: &str) -> Result<(), String> {
    let (set, selection) = (set.to_string(), selection.to_string());
    prim.update_metadata("variantSelection", move |cur| {
        let mut m = match cur {
            Some(Value::VariantSelectionMap(m)) => m,
            _ => HashMap::new(),
        };
        m.insert(set, selection);
        Some(Value::VariantSelectionMap(m))
    })
    .map_err(|e| e.to_string())?;
    Ok(())
}

/// What the Python module authors on a prim: a variant selection, a reference, a payload or a material binding.
pub fn author_arc(st: &Stage, op: &str, path: &str, x: &str, y: &str) -> Result<(), String> {
    let p = st.prim(path).map_err(|e| e.to_string())?;
    match op {
        "setVariant" => set_variant(p, x, y),
        "addReference" => add_reference(p, x, y, false),
        "addPayload" => add_reference(p, x, y, true),
        "bindMaterial" => bind_material(st, &p, x),
        other => Err(format!("no {other}")),
    }
}

/// Sets a variant selection through the scene's undo stack.
pub fn run_set_variant(s: &mut Session, t: &Target, set: &str, choice: &str) -> Result<(), String> {
    let path = t.path.clone();
    t.entry.edit(s, "variant", |st| set_variant(st.prim(path.clone()).map_err(|e| e.to_string())?, set, choice))
}

fn bind_material(stage: &Stage, prim: &Prim, material: &str) -> Result<(), String> {
    let p = prim.clone().apply_api("MaterialBindingAPI").map_err(|e| e.to_string())?;
    let rel = stage.relationship_builder(p.relationship("material:binding").path().clone());
    rel.set_targets([sdf::Path::new(material).map_err(|e| e.to_string())?]).build().map_err(|e| e.to_string())?;
    Ok(())
}

/// Deletes on the edit target; what a weaker opinion or an arc still composes is deactivated instead, as a delete can't
/// remove an opinion it doesn't own.
fn remove_prim(stage: &Stage, path: &sdf::Path) -> Result<(), String> {
    let mut ed = NamespaceEditor::new(stage);
    ed.delete_prim(path.clone()).map_err(|e| e.to_string())?;
    if ed.can_apply().is_ok() {
        ed.apply().map_err(|e| e.to_string())?;
    }
    if stage.prim(path.clone()).map_err(|e| e.to_string())?.is_valid().unwrap_or(false) {
        stage.prim(path.clone()).map_err(|e| e.to_string())?.set_active(false).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn run_prim(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).map(str::to_string).or_else(|| crate::prims::selected().last().cloned()).ok_or_else(|| err("usdPrim: name a prim, or select one"))?;
    let t: Target = resolve(&mut s.graph, &name).map_err(err)?;
    let entry = t.entry.clone();
    let path = t.path.clone();
    let time = stages::time_of(&mut s.graph, t.shape);
    let shape = t.shape;

    if a.bool("query") {
        let prim = entry.stage.prim(path.clone()).map_err(err)?;
        if a.bool("children") {
            return prim_children(&entry.stage, path.as_str()).map_err(err);
        }
        if a.bool("exists") {
            return Ok(json!(prim.is_valid().unwrap_or(false)));
        }
        if !prim.is_valid().unwrap_or(false) {
            return Err(err(format!("usdPrim: no prim {path}")));
        }
        return Ok(prim_info(&prim, time));
    }

    entry.check_target().map_err(err)?;
    let mut result = J::Null;
    let p = path.clone();
    let define = a.get("define").and_then(J::as_str).map(str::to_string);
    let remove = a.bool("remove");
    let rename = a.get("rename").and_then(J::as_str).map(str::to_string);
    let reparent = a.get("reparent").and_then(J::as_str).map(str::to_string);
    let active = a.get("active").map(|v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    let instanceable = a.get("instanceable").map(|v| v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0));
    let kind = a.get("kind").and_then(J::as_str).map(str::to_string);
    let purpose = a.get("purpose").and_then(J::as_str).map(str::to_string);
    let reference = a.get("addReference").and_then(J::as_str).map(str::to_string);
    let payload = a.get("addPayload").and_then(J::as_str).map(str::to_string);
    let target = a.string("targetPrim", "");
    let variant: Vec<String> = a.get("setVariant").and_then(J::as_array).map(|l| l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let material = a.get("assignMaterial").and_then(J::as_str).map(str::to_string);

    let mut new_path: Option<sdf::Path> = None;
    entry
        .edit(s, "usdPrim", |st| {
            if let Some(ty) = &define {
                let pr = st.define_prim(p.clone()).map_err(|e| e.to_string())?;
                if !ty.is_empty() {
                    pr.set_type_name(ty.as_str()).map_err(|e| e.to_string())?;
                }
            }
            if let Some(v) = active {
                st.prim(p.clone()).map_err(|e| e.to_string())?.set_active(v).map_err(|e| e.to_string())?;
            }
            if let Some(v) = instanceable {
                st.prim(p.clone()).map_err(|e| e.to_string())?.set_instanceable(v).map_err(|e| e.to_string())?;
            }
            if let Some(k) = &kind {
                st.prim(p.clone()).map_err(|e| e.to_string())?.set_kind(k.as_str()).map_err(|e| e.to_string())?;
            }
            if let Some(v) = &purpose {
                let ap = st.prim(p.clone()).map_err(|e| e.to_string())?.attribute("purpose").path().clone();
                let at = st.create_attribute(ap, "token").map_err(|e| e.to_string())?;
                at.set(Value::Token(v.as_str().into())).map_err(|e| e.to_string())?;
            }
            if let Some(f) = &reference {
                add_reference(st.prim(p.clone()).map_err(|e| e.to_string())?, f, &target, false)?;
            }
            if let Some(f) = &payload {
                add_reference(st.prim(p.clone()).map_err(|e| e.to_string())?, f, &target, true)?;
            }
            if variant.len() >= 2 {
                set_variant(st.prim(p.clone()).map_err(|e| e.to_string())?, &variant[0], &variant[1])?;
            }
            if let Some(m) = &material {
                bind_material(st, &st.prim(p.clone()).map_err(|e| e.to_string())?, m)?;
            }
            if let Some(n) = &rename {
                let pr = st.prim(p.clone()).map_err(|e| e.to_string())?;
                let mut ed = NamespaceEditor::new(st);
                ed.rename_prim(&pr, n).map_err(|e| e.to_string())?;
                ed.apply().map_err(|e| e.to_string())?;
                new_path = p.parent().and_then(|q| q.append_path(n.as_str()).ok());
            }
            if let Some(to) = &reparent {
                let pr = st.prim(p.clone()).map_err(|e| e.to_string())?;
                let up = st.prim(to.as_str()).map_err(|e| e.to_string())?;
                let mut ed = NamespaceEditor::new(st);
                ed.reparent_prim(&pr, &up).map_err(|e| e.to_string())?;
                ed.apply().map_err(|e| e.to_string())?;
                new_path = p.name().and_then(|n| up.path().append_path(n).ok());
            }
            if remove {
                remove_prim(st, &p)?;
            }
            Ok(())
        })
        .map_err(err)?;
    if let Some(np) = new_path {
        result = json!(name_of(&s.graph, shape, np.as_str()));
    } else if define.is_some() {
        result = json!(name_of(&s.graph, shape, p.as_str()));
    }
    // Load state isn't authored: it belongs to the stage, and undoes through the same stack.
    if a.bool("load") || a.bool("unload") {
        let load = a.bool("load");
        let (e1, e2, q1, q2) = (entry.clone(), entry.clone(), path.clone(), path.clone());
        let do_it = move |e: &stages::Entry, q: &sdf::Path, on: bool| {
            let r = if on { e.stage.load(q.clone(), LoadPolicy::default()) } else { e.stage.unload(q.clone()) };
            if let Err(x) = r {
                log::warn!("USD: {x}");
            }
        };
        do_it(&entry, &path, load);
        s.graph.undo_stack_mut().record(aztek_undo::FnAction::new(move |_g: &mut aztek_scene::graph::Graph| do_it(&e1, &q1, !load), move |_g: &mut aztek_scene::graph::Graph| do_it(&e2, &q2, load)));
    }
    Ok(result)
}

/// `shape,/prim.attr` into (prim name, attribute).
fn split_attr(name: &str) -> Option<(String, String)> {
    let (shape, rest) = split_name(name)?;
    let (prim, attr) = rest.rsplit_once('.')?;
    Some((format!("{shape},{prim}"), attr.to_string()))
}

fn get_usd_attr(s: &mut Session, name: &str, at_time: Option<f64>) -> CmdResult {
    let (prim, attr) = split_attr(name).ok_or_else(|| err(format!("{name} is not prim.attribute")))?;
    let t = resolve(&mut s.graph, &prim).map_err(err)?;
    let time = at_time.unwrap_or_else(|| stages::time_of(&mut s.graph, t.shape));
    if let Some(v) = crate::panels::virtual_get(&t, &attr, time) {
        return Ok(v);
    }
    let p = t.entry.stage.prim(t.path.clone()).map_err(err)?;
    let v = p.attribute(attr.as_str()).get_at::<Value>(TimeCode::new(time)).map_err(err)?;
    Ok(v.as_ref().map_or(J::Null, value_json))
}

fn set_usd_attr(s: &mut Session, name: &str, v: &J, ty: Option<String>, key: bool) -> CmdResult {
    let (prim, attr) = split_attr(name).ok_or_else(|| err(format!("{name} is not prim.attribute")))?;
    let t = resolve(&mut s.graph, &prim).map_err(err)?;
    let time = stages::time_of(&mut s.graph, t.shape);
    if crate::panels::virtual_set(s, &t, &attr, v, time).map_err(err)? {
        return Ok(J::Null);
    }
    t.entry.check_target().map_err(err)?;
    let entry = t.entry.clone();
    let path = t.path.clone();
    let v = v.clone();
    entry
        .edit(s, "setAttr", move |st| {
            let pr = st.prim(path.clone()).map_err(|e| e.to_string())?;
            let a = pr.attribute(attr.as_str());
            let declared = a.type_name().ok().flatten().map(|x| x.to_string());
            let ty_name = declared.clone().or(ty).ok_or_else(|| format!("{attr} doesn't exist: give it a type"))?;
            let val = json_value(&ty_name, &v)?;
            let ap = a.path().clone();
            let keyed = key || a.num_time_samples().unwrap_or(0) > 0;
            let at = st.create_attribute(ap, ty_name.as_str()).map_err(|e| e.to_string())?;
            if keyed {
                at.set_at(val, TimeCode::new(time)).map_err(|e| e.to_string())?;
            } else {
                at.set(val).map_err(|e| e.to_string())?;
            }
            Ok(())
        })
        .map_err(err)?;
    Ok(J::Null)
}

fn run_attr(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.pos_str(0).ok_or_else(|| err("usdAttr: name prim.attribute"))?.to_string();
    if a.bool("query") {
        if a.bool("timeSamples") {
            let (prim, attr) = split_attr(&name).ok_or_else(|| err("usdAttr: name prim.attribute"))?;
            let t = resolve(&mut s.graph, &prim).map_err(err)?;
            let p = t.entry.stage.prim(t.path.clone()).map_err(err)?;
            return Ok(json!(p.attribute(attr.as_str()).time_sample_times().map_err(err)?));
        }
        return get_usd_attr(s, &name, a.get("time").and_then(J::as_f64));
    }
    let v = a.get("value").cloned().unwrap_or(J::Null);
    set_usd_attr(s, &name, &v, a.get("type").and_then(J::as_str).map(str::to_string), a.bool("key"))
}

/// Authors time samples on a prim: the named attribute, or every op of its transform. Returns how many were keyed.
fn key_prim(s: &mut Session, name: &str, at_time: Option<f64>, value: Option<&J>) -> Result<i64, String> {
    let (prim_name, attr) = match split_attr(name) {
        Some(x) => (x.0, Some(x.1)),
        None => (name.to_string(), None),
    };
    let t = resolve(&mut s.graph, &prim_name)?;
    t.entry.check_target()?;
    let time = at_time.unwrap_or_else(|| stages::time_of(&mut s.graph, t.shape));
    let now = stages::time_of(&mut s.graph, t.shape);
    let path = t.path.clone();
    let value = value.cloned();
    t.entry.clone().edit(s, "setKeyframe", move |st| {
        let p = st.prim(path.clone()).map_err(|e| e.to_string())?;
        let mut names: Vec<String> = match attr {
            // translateX and friends key the op that holds them.
            Some(a) if a.starts_with("translate") => vec!["xformOp:translate".to_string()],
            Some(a) if a.starts_with("scale") => vec!["xformOp:scale".to_string()],
            Some(a) if a.starts_with("rotate") => p.authored_attributes().map_err(|e| e.to_string())?.iter().map(|x| x.name().to_string()).filter(|n| n.starts_with("xformOp:rotate")).collect(),
            Some(a) => vec![a],
            None => p.authored_attributes().map_err(|e| e.to_string())?.iter().map(|x| x.name().to_string()).filter(|n| n.starts_with("xformOp:")).collect(),
        };
        names.retain(|n| !n.ends_with("rotateAxis"));
        let mut n = 0;
        for name in names {
            let a = p.attribute(name.as_str());
            let Some(ty) = a.type_name().map_err(|e| e.to_string())?.map(|x| x.to_string()) else { continue };
            let v = match &value {
                Some(j) => json_value(&ty, j)?,
                None => match a.get_at::<Value>(TimeCode::new(now)).map_err(|e| e.to_string())? {
                    Some(v) => v,
                    None => continue,
                },
            };
            a.set_at(v, TimeCode::new(time)).map_err(|e| e.to_string())?;
            n += 1;
        }
        Ok(n)
    })
}

fn wrap(c: &mut Commands<Session>, name: &str, f: impl Fn(&mut Session, &Args, &aztek_ops::RunFn<Session>) -> CmdResult + Send + Sync + 'static) {
    let Some(old) = c.find(name).cloned() else { return };
    let prev = old.run.clone();
    let mut d: CommandDef<Session> = old;
    d.run = std::sync::Arc::new(move |s, a| f(s, a, &prev));
    c.register(d);
}

pub fn register(c: &mut Commands<Session>) {
    let f = |n: &str, sn: &str, ty: ArgType, d: &str| FlagDef::new(n, sn, ty, J::Null, d);
    c.register(
        CommandDef::new("usdPrim", "Author or read a prim named proxy,/path; with no name, the selected prim. Returns the prim's new name when it moved or was made. With -q, a JSON description; -children and -exists narrow it.", run_prim)
            .positional("proxy,/path")
            .flag(f("query", "q", ArgType::Bool, "Read, don't author."))
            .flag(f("children", "ch", ArgType::Bool, "With -q: the children."))
            .flag(f("exists", "ex", ArgType::Bool, "With -q: whether the prim is there."))
            .flag(f("define", "def", ArgType::String, "Define the prim, of this type; empty for none."))
            .flag(f("remove", "rm", ArgType::Bool, "Delete it, or deactivate what can't be deleted from the edit target."))
            .flag(f("rename", "rn", ArgType::String, "A new name among its siblings."))
            .flag(f("reparent", "rp", ArgType::String, "A new parent's path."))
            .flag(f("active", "act", ArgType::Any, "Activate or deactivate."))
            .flag(f("instanceable", "ins", ArgType::Any, "Whether it composes as an instance."))
            .flag(f("kind", "k", ArgType::String, "The model kind."))
            .flag(f("purpose", "pur", ArgType::String, "default, render, proxy or guide."))
            .flag(f("load", "l", ArgType::Bool, "Load its payloads."))
            .flag(f("unload", "ul", ArgType::Bool, "Unload its payloads."))
            .flag(f("addReference", "ar", ArgType::String, "A file to reference."))
            .flag(f("addPayload", "ap", ArgType::String, "A file to add as a payload."))
            .flag(f("targetPrim", "tp", ArgType::String, "The prim in that file; empty for its default prim."))
            .flag(f("setVariant", "sv", ArgType::Any, "A variant set and its selection.").multi(2))
            .flag(f("assignMaterial", "am", ArgType::String, "The path of a Material prim to bind.")),
    );
    c.register(
        CommandDef::new("usdAttr", "Read or author an attribute named proxy,/path.attr. A sampled attribute takes a sample at the current time; -key makes one. -q reads at the current time, or -time; -timeSamples lists the times.", run_attr)
            .positional("proxy,/path.attr")
            .flag(f("query", "q", ArgType::Bool, "Read."))
            .flag(f("value", "v", ArgType::Any, "The value."))
            .flag(f("type", "typ", ArgType::String, "The type, for a new attribute."))
            .flag(f("key", "k", ArgType::Bool, "Author a sample at the current time."))
            .flag(f("time", "t", ArgType::Float, "With -q: the time to read at."))
            .flag(f("timeSamples", "ts", ArgType::Bool, "With -q: the times it's sampled at.")),
    );
    wrap(c, "getAttr", |s, a, prev| match a.pos_str(0) {
        Some(n) if split_name(n).is_some() => get_usd_attr(s, &n.to_string(), a.get("time").and_then(J::as_f64)),
        _ => prev(s, a),
    });
    wrap(c, "setAttr", |s, a, prev| match a.pos_str(0) {
        Some(n) if split_name(n).is_some() => {
            let name = n.to_string();
            let rest: Vec<J> = a.positional[1..].to_vec();
            let v = if rest.len() == 1 { rest[0].clone() } else { J::Array(rest) };
            set_usd_attr(s, &name, &v, None, false)
        }
        _ => prev(s, a),
    });
    wrap(c, "delete", |s, a, prev| {
        let names: Vec<String> = a.positional.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let usd: Vec<String> = if names.is_empty() { crate::prims::selected() } else { names.iter().filter(|n| split_name(n).is_some()).cloned().collect() };
        for n in &usd {
            let mut flags = Map::new();
            flags.insert("args".into(), json!([n]));
            flags.insert("remove".into(), json!(true));
            s.run("usdPrim", J::Object(flags))?;
        }
        if !usd.is_empty() && names.is_empty() {
            s.run("select", json!({"clear": true}))?;
        }
        let plain: Vec<J> = names.iter().filter(|n| split_name(n).is_none()).map(|x| json!(x)).collect();
        if plain.is_empty() && !usd.is_empty() {
            return Ok(J::Null);
        }
        let mut call = a.clone();
        call.positional = plain;
        prev(s, &call)
    });
    wrap(c, "setKeyframe", |s, a, prev| {
        let names: Vec<String> = a.positional.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        let named: Vec<String> = names.iter().filter(|n| split_name(n).is_some()).cloned().collect();
        let targets = if named.is_empty() && names.is_empty() { crate::prims::selected() } else { named };
        if targets.is_empty() {
            return prev(s, a);
        }
        let mut keyed = 0;
        for n in &targets {
            keyed += key_prim(s, n, a.get("time").and_then(J::as_f64), a.get("value")).map_err(err)?;
        }
        Ok(json!(keyed))
    });
    wrap(c, "parent", |s, a, prev| {
        let names: Vec<String> = a.positional.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
        if names.len() < 2 || !names.iter().all(|n| split_name(n).is_some()) {
            return prev(s, a);
        }
        let (kids, up) = names.split_at(names.len() - 1);
        let to = split_name(&up[0]).map_or("/", |x| x.1).to_string();
        let mut out = Vec::new();
        for k in kids {
            out.push(s.run("usdPrim", json!({"args": [k], "reparent": to}))?);
        }
        Ok(J::Array(out))
    });
    c.register(
        CommandDef::new("AL_usdmaya_ProxyShapeFindLoadable", "Prims with payloads under -primPath of a proxy; -loaded or -unloaded narrow it.", |s: &mut Session, a: &Args| {
            let proxy = a.pos_str(0).unwrap_or("").to_string();
            let shape = stages::proxy_named(&s.graph, &proxy).ok_or_else(|| err(format!("no proxy shape named '{proxy}'")))?;
            let e = stages::entry_of(&mut s.graph, shape).map_err(err)?;
            let root = a.string("primPath", "/");
            let all = e.stage.find_loadable(root.as_str()).map_err(err)?;
            let loaded = e.stage.load_set().map_err(err)?;
            let keep = |p: &sdf::Path| match (a.bool("loaded"), a.bool("unloaded")) {
                (true, false) => loaded.contains(p),
                (false, true) => !loaded.contains(p),
                _ => true,
            };
            Ok(json!(all.iter().filter(|p| keep(p)).map(|p| p.as_str().to_string()).collect::<Vec<_>>()))
        })
        .positional("proxy")
        .flag(f("primPath", "pp", ArgType::String, "Only under this prim."))
        .flag(f("loaded", "l", ArgType::Bool, "Only loaded ones."))
        .flag(f("unloaded", "ul", ArgType::Bool, "Only unloaded ones."))
        .undoable(false),
    );
    wrap(c, "rename", |s, a, prev| match a.pos_str(0) {
        Some(n) if split_name(n).is_some() => {
            let new = a.pos_str(1).ok_or_else(|| err("rename: a new name is required"))?.to_string();
            s.run("usdPrim", json!({"args": [n], "rename": new}))
        }
        _ => prev(s, a),
    });
}
