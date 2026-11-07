//! `usdPy`: what the `pxr` module in Aztek's Python stands on. A stage is an id; values cross as JSON with whole arrays.
//! Authoring goes through the scene's undo stack, so a script's edit to a proxy's stage undoes like any other.

use std::rc::Rc;

use aztek_ops::{ArgType, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use aztek_scene::session::Session;
use openusd::gf;
use openusd::sdf::{self, LayerOffset, Value, Variability};
use openusd::tf::Token;
use openusd::usd::{EditTarget, LoadPolicy, PrimPredicate, TimeCode};
use serde_json::{json, Value as J};

use crate::prim_cmds::{self, json_value};
use crate::prims::err;
use crate::stages::{self, Entry};

/// A value with its arrays whole, for scripts.
pub fn full_json(v: &Value) -> J {
    match v {
        Value::IntVec(l) => json!(l),
        Value::FloatVec(l) => json!(l),
        Value::DoubleVec(l) => json!(l),
        Value::BoolVec(l) => json!(l),
        Value::StringVec(l) => json!(l),
        Value::Vec2fVec(l) => json!(l.iter().map(|x| [x.x, x.y]).collect::<Vec<_>>()),
        Value::Vec2dVec(l) => json!(l.iter().map(|x| [x.x, x.y]).collect::<Vec<_>>()),
        Value::Vec3fVec(l) => json!(l.iter().map(|x| [x.x, x.y, x.z]).collect::<Vec<_>>()),
        Value::Vec3dVec(l) => json!(l.iter().map(|x| [x.x, x.y, x.z]).collect::<Vec<_>>()),
        Value::Vec4fVec(l) => json!(l.iter().map(|x| [x.x, x.y, x.z, x.w]).collect::<Vec<_>>()),
        Value::Vec3hVec(l) => json!(l.iter().map(|x| [x.x.to_f64(), x.y.to_f64(), x.z.to_f64()]).collect::<Vec<_>>()),
        Value::QuatfVec(l) => json!(l.iter().map(|q| [q.w, q.x, q.y, q.z]).collect::<Vec<_>>()),
        Value::Matrix4dVec(l) => json!(l.iter().map(|m| m.0.to_vec()).collect::<Vec<_>>()),
        other => prim_cmds::value_json(other),
    }
}

/// An array value from JSON, by its type.
pub fn array_value(ty: &str, v: &J) -> Result<Value, String> {
    let list = v.as_array().ok_or_else(|| format!("a {ty} needs a list"))?;
    let nums = |x: &J| -> Vec<f64> { x.as_array().map_or_else(|| x.as_f64().into_iter().collect(), |a| a.iter().filter_map(J::as_f64).collect()) };
    let need = |x: &[f64], k: usize| if x.len() >= k { Ok(()) } else { Err(format!("each of a {ty} needs {k} numbers")) };
    let elem = ty.trim_end_matches("[]");
    Ok(match elem {
        "int" => Value::IntVec(list.iter().map(|x| x.as_f64().unwrap_or(0.0) as i32).collect()),
        "float" => Value::FloatVec(list.iter().map(|x| x.as_f64().unwrap_or(0.0) as f32).collect()),
        "double" => Value::DoubleVec(list.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect()),
        "bool" => Value::BoolVec(list.iter().map(|x| x.as_bool().unwrap_or(false)).collect()),
        "string" => Value::StringVec(list.iter().map(|x| x.as_str().unwrap_or("").to_string()).collect()),
        "token" => Value::TokenVec(list.iter().map(|x| Token::from(x.as_str().unwrap_or(""))).collect()),
        "float2" | "texCoord2f" => {
            let mut o = Vec::with_capacity(list.len());
            for x in list {
                let n = nums(x);
                need(&n, 2)?;
                o.push(gf::vec2f(n[0] as f32, n[1] as f32));
            }
            Value::Vec2fVec(o)
        }
        "float3" | "point3f" | "vector3f" | "normal3f" | "color3f" => {
            let mut o = Vec::with_capacity(list.len());
            for x in list {
                let n = nums(x);
                need(&n, 3)?;
                o.push(gf::vec3f(n[0] as f32, n[1] as f32, n[2] as f32));
            }
            Value::Vec3fVec(o)
        }
        "double3" | "point3d" | "vector3d" | "normal3d" | "color3d" => {
            let mut o = Vec::with_capacity(list.len());
            for x in list {
                let n = nums(x);
                need(&n, 3)?;
                o.push(gf::vec3d(n[0], n[1], n[2]));
            }
            Value::Vec3dVec(o)
        }
        "float4" | "color4f" => {
            let mut o = Vec::with_capacity(list.len());
            for x in list {
                let n = nums(x);
                need(&n, 4)?;
                o.push(gf::vec4f(n[0] as f32, n[1] as f32, n[2] as f32, n[3] as f32));
            }
            Value::Vec4fVec(o)
        }
        "matrix4d" => {
            let mut o = Vec::with_capacity(list.len());
            for x in list {
                let n = nums(x);
                need(&n, 16)?;
                let mut m = [0.0; 16];
                m.copy_from_slice(&n[..16]);
                o.push(gf::Matrix4d(m));
            }
            Value::Matrix4dVec(o)
        }
        other => return Err(format!("setting a {other}[] isn't supported")),
    })
}

fn to_value(ty: &str, v: &J) -> Result<Value, String> {
    if ty.ends_with("[]") {
        return array_value(ty, v);
    }
    if ty == "matrix4d" {
        let n: Vec<f64> = v.as_array().map_or_else(Vec::new, |a| a.iter().filter_map(J::as_f64).collect());
        if n.len() < 16 {
            return Err("a matrix4d needs 16 numbers".into());
        }
        let mut m = [0.0; 16];
        m.copy_from_slice(&n[..16]);
        return Ok(Value::Matrix4d(gf::Matrix4d(m)));
    }
    if ty == "quatf" {
        let n: Vec<f64> = v.as_array().map_or_else(Vec::new, |a| a.iter().filter_map(J::as_f64).collect());
        if n.len() < 4 {
            return Err("a quatf needs 4 numbers".into());
        }
        return Ok(Value::Quatf(gf::Quatf { w: n[0] as f32, x: n[1] as f32, y: n[2] as f32, z: n[3] as f32 }));
    }
    json_value(ty, v)
}

fn text(a: &J, i: usize) -> String {
    a.get(i).and_then(J::as_str).unwrap_or("").to_string()
}

fn at_time(a: &J, i: usize) -> Option<TimeCode> {
    a.get(i).and_then(J::as_f64).map(TimeCode::new)
}

fn stage_id(a: &J) -> i64 {
    a.get(0).and_then(J::as_i64).unwrap_or(0)
}

fn entry(a: &J) -> Result<Rc<Entry>, String> {
    let id = stage_id(a);
    stages::entry_by_id(id).ok_or_else(|| format!("no open stage {id}"))
}

fn e(x: impl std::fmt::Display) -> String {
    x.to_string()
}

fn run_op(s: &mut Session, op: &str, a: &J) -> Result<J, String> {
    match op {
        // Stages.
        "open" => {
            let file = text(a, 0);
            let load = a.get(1).and_then(J::as_bool).unwrap_or(true);
            let en = stages::standalone(&file, load)?;
            Ok(json!(en.id))
        }
        "createNew" | "createInMemory" => {
            let file = if op == "createNew" { text(a, 0) } else { String::new() };
            if !file.is_empty() {
                std::fs::write(&file, "#usda 1.0\n").map_err(|x| format!("{file}: {x}"))?;
            }
            let en = stages::standalone(&file, true)?;
            Ok(json!(en.id))
        }
        "ofProxy" => {
            let name = text(a, 0);
            let shape = stages::proxy_named(&s.graph, &name).ok_or_else(|| format!("no proxy shape named '{name}'"))?;
            Ok(json!(stages::entry_of(&mut s.graph, shape)?.id))
        }
        "rootLayer" => Ok(json!(entry(a)?.stage.root_layer().identifier())),
        "sessionLayer" => Ok(json!(entry(a)?.session_identifier)),
        "layerStack" => Ok(json!(entry(a)?.stage.layer_identifiers())),
        "save" => {
            let en = entry(a)?;
            for l in en.dirty.borrow().clone() {
                if l == en.session_identifier || en.stage.layer(&l).is_some_and(|x| x.is_anonymous()) || (en.file.is_empty() && en.stage.root_layer().identifier() == l) {
                    continue;
                }
                let mut layer = en.stage.layer_mut(&l).ok_or_else(|| format!("no layer {l}"))?;
                layer.save().map_err(e)?;
                drop(layer);
                en.set_clean(&l);
            }
            Ok(J::Null)
        }
        "export" => {
            let en = entry(a)?;
            let flat = en.stage.flatten().map_err(e)?;
            flat.export(text(a, 1)).map_err(e)?;
            Ok(json!(true))
        }
        "exportToString" => Ok(json!(entry(a)?.stage.flatten().map_err(e)?.export_to_string().map_err(e)?)),
        "metadataGet" => {
            let en = entry(a)?;
            let v = en.stage.stage_metadata(text(a, 1)).map_err(e)?;
            Ok(v.as_ref().map_or(J::Null, full_json))
        }
        "metadataSet" => {
            let en = entry(a)?;
            let root = en.stage.root_layer().identifier().to_string();
            let key = text(a, 1);
            let val = a.get(2).cloned().unwrap_or(J::Null);
            let v = match &val {
                J::String(x) if key == "upAxis" || key == "defaultPrim" => Value::Token(x.as_str().into()),
                J::String(x) => Value::String(x.clone()),
                J::Number(n) => Value::Double(n.as_f64().unwrap_or(0.0)),
                J::Bool(b) => Value::Bool(*b),
                _ => return Err(format!("can't author {key}")),
            };
            en.clone().edit(s, "metadata", |st| {
                st.batch_edit(&[root.as_str()], |edits| {
                    let p = sdf::Path::abs_root();
                    let d = edits[0].data_mut();
                    if d.spec_type(&p).is_none() {
                        d.create_spec(p.clone(), sdf::SpecType::PseudoRoot);
                    }
                    d.set_field(&p, &key, v.clone());
                    Ok(())
                })
                .map(|_| ())
                .map_err(e)
            })?;
            Ok(J::Null)
        }
        "defaultPrim" => Ok(json!(entry(a)?.stage.default_prim().map_or(String::new(), |t| t.as_str().to_string()))),
        "setDefaultPrim" => {
            let en = entry(a)?;
            let n = text(a, 1);
            en.clone().edit(s, "defaultPrim", |st| st.set_default_prim(n.as_str()).map_err(e))?;
            Ok(J::Null)
        }
        "timeRange" => {
            let en = entry(a)?;
            Ok(json!([en.stage.start_time_code(), en.stage.end_time_code(), en.stage.time_codes_per_second()]))
        }
        "setTimeRange" => {
            let en = entry(a)?;
            let (lo, hi) = (a.get(1).and_then(J::as_f64), a.get(2).and_then(J::as_f64));
            en.clone().edit(s, "timeRange", |st| {
                if let Some(x) = lo {
                    st.set_start_time_code(x).map_err(e)?;
                }
                if let Some(x) = hi {
                    st.set_end_time_code(x).map_err(e)?;
                }
                Ok(())
            })?;
            Ok(J::Null)
        }
        "traverse" => {
            let en = entry(a)?;
            let all = a.get(1).and_then(J::as_bool).unwrap_or(false);
            let pred = if all { PrimPredicate::ALL } else { PrimPredicate::DEFAULT };
            let mut out = Vec::new();
            en.stage.traverse(pred, |p| out.push(p.as_str().to_string())).map_err(e)?;
            Ok(json!(out))
        }
        "define" | "override" => {
            let en = entry(a)?;
            en.check_target()?;
            let (p, ty) = (text(a, 1), text(a, 2));
            en.clone().edit(s, "define", |st| {
                let pr = if op == "define" { st.define_prim(p.as_str()).map_err(e)? } else { st.override_prim(p.as_str()).map_err(e)? };
                if !ty.is_empty() {
                    pr.set_type_name(ty.as_str()).map_err(e)?;
                }
                Ok(())
            })?;
            Ok(json!(p))
        }
        "removePrim" => {
            let en = entry(a)?;
            en.check_target()?;
            let p = text(a, 1);
            en.clone().edit(s, "remove", |st| st.remove_prim(p.as_str()).map_err(e))?;
            Ok(json!(true))
        }
        "setEditTarget" => {
            let en = entry(a)?;
            en.stage.set_edit_target(EditTarget::for_layer(text(a, 1))).map_err(e)?;
            Ok(J::Null)
        }
        "editTarget" => Ok(json!(entry(a)?.stage.edit_target().layer_identifier())),

        // Prims.
        "primInfo" => {
            let en = entry(a)?;
            let pr = en.stage.prim(text(a, 1).as_str()).map_err(e)?;
            if !pr.is_valid().unwrap_or(false) {
                return Ok(J::Null);
            }
            Ok(json!({
                "type": pr.type_name().ok().flatten().map_or(String::new(), |t| t.as_str().to_string()),
                "specifier": format!("{:?}", pr.specifier().ok().flatten().unwrap_or(sdf::Specifier::Over)),
                "active": pr.is_active().unwrap_or(true),
                "loaded": pr.is_loaded().unwrap_or(true),
                "instanceable": pr.is_instanceable().unwrap_or(false),
                "instance": pr.is_instance().unwrap_or(false),
                "kind": pr.kind().ok().flatten().map_or(String::new(), |t| t.as_str().to_string()),
                "children": pr.child_names().unwrap_or_default().iter().map(|t| t.as_str().to_string()).collect::<Vec<_>>(),
                "properties": pr.property_names().unwrap_or_default().iter().map(|t| t.as_str().to_string()).collect::<Vec<_>>(),
                "apiSchemas": pr.api_schemas().unwrap_or_default().iter().map(|t| t.as_str().to_string()).collect::<Vec<_>>(),
            }))
        }
        "primSet" => {
            let en = entry(a)?;
            en.check_target()?;
            let (p, key) = (text(a, 1), text(a, 2));
            let val = a.get(3).cloned().unwrap_or(J::Null);
            en.clone().edit(s, "primSet", |st| {
                let pr = st.prim(p.as_str()).map_err(e)?;
                match key.as_str() {
                    "active" => pr.set_active(val.as_bool().unwrap_or(true)).map(|_| ()),
                    "instanceable" => pr.set_instanceable(val.as_bool().unwrap_or(false)).map(|_| ()),
                    "kind" => pr.set_kind(val.as_str().unwrap_or("")).map(|_| ()),
                    "typeName" => pr.set_type_name(val.as_str().unwrap_or("")).map(|_| ()),
                    "hidden" => pr.set_hidden(val.as_bool().unwrap_or(false)).map(|_| ()),
                    "applyAPI" => pr.add_applied_schema(val.as_str().unwrap_or("")).map(|_| ()),
                    other => return Err(format!("can't set {other}")),
                }
                .map_err(e)
            })?;
            Ok(J::Null)
        }
        "load" | "unload" => {
            let en = entry(a)?;
            let p = text(a, 1);
            if op == "load" {
                en.stage.load(p.as_str(), LoadPolicy::default()).map_err(e)?;
            } else {
                en.stage.unload(p.as_str()).map_err(e)?;
            }
            Ok(J::Null)
        }
        "variants" => {
            let en = entry(a)?;
            let pr = en.stage.prim(text(a, 1).as_str()).map_err(e)?;
            let v: Vec<J> = prim_cmds::variant_sets(&pr).into_iter().map(|(n, o, c)| json!({"name": n, "options": o, "selection": c})).collect();
            Ok(J::Array(v))
        }
        "setVariant" | "addReference" | "addPayload" | "bindMaterial" => {
            let en = entry(a)?;
            en.check_target()?;
            let p = text(a, 1);
            let (x, y) = (text(a, 2), text(a, 3));
            en.clone().edit(s, op, |st| prim_cmds::author_arc(st, op, &p, &x, &y))?;
            Ok(J::Null)
        }

        // Attributes.
        "attrInfo" => {
            let en = entry(a)?;
            let at = en.stage.attribute(text(a, 1).as_str()).map_err(e)?;
            Ok(json!({
                "type": at.type_name().ok().flatten().map_or(String::new(), |t| t.to_string()),
                "defined": at.is_defined().unwrap_or(false),
                "authored": at.has_authored_value().unwrap_or(false),
                "custom": at.is_custom().unwrap_or(false),
                "samples": at.time_sample_times().unwrap_or_default(),
                "interpolation": at.get_metadata::<Token>("interpolation").ok().flatten().map_or(String::new(), |t| t.as_str().to_string()),
            }))
        }
        "attrGet" => {
            let en = entry(a)?;
            let at = en.stage.attribute(text(a, 1).as_str()).map_err(e)?;
            let v = match at_time(a, 2) {
                Some(t) => at.get_at::<Value>(t).map_err(e)?,
                None => at.get_at::<Value>(TimeCode::EARLIEST).map_err(e)?,
            };
            Ok(v.as_ref().map_or(J::Null, full_json))
        }
        "attrCreate" => {
            let en = entry(a)?;
            en.check_target()?;
            let (p, ty) = (text(a, 1), text(a, 2));
            let custom = a.get(3).and_then(J::as_bool).unwrap_or(false);
            let uniform = a.get(4).and_then(J::as_bool).unwrap_or(false);
            en.clone().edit(s, "attr", |st| {
                let mut b = st.attribute_builder(p.as_str(), ty.as_str()).custom(custom);
                if uniform {
                    b = b.variability(Variability::Uniform);
                }
                b.build().map_err(e)?;
                Ok(())
            })?;
            Ok(json!(p))
        }
        "attrSet" => {
            let en = entry(a)?;
            en.check_target()?;
            let (p, val) = (text(a, 1), a.get(2).cloned().unwrap_or(J::Null));
            let time = at_time(a, 3);
            en.clone().edit(s, "attrSet", |st| {
                let at = st.attribute(p.as_str()).map_err(e)?;
                let ty = at.type_name().map_err(e)?.map(|t| t.to_string()).ok_or_else(|| format!("{p} has no type: create it first"))?;
                let v = to_value(&ty, &val)?;
                match time {
                    Some(t) => at.set_at(v, t).map(|_| ()),
                    None => at.set(v).map(|_| ()),
                }
                .map_err(e)
            })?;
            Ok(json!(true))
        }
        "attrClear" => {
            let en = entry(a)?;
            let p = text(a, 1);
            en.clone().edit(s, "attrClear", |st| st.attribute(p.as_str()).map_err(e)?.clear().map(|_| ()).map_err(e))?;
            Ok(J::Null)
        }
        "attrMeta" => {
            let en = entry(a)?;
            let (p, key, val) = (text(a, 1), text(a, 2), a.get(3).cloned().unwrap_or(J::Null));
            let key: &'static str = match key.as_str() {
                "interpolation" => "interpolation",
                "elementSize" => "elementSize",
                other => return Err(format!("can't set {other} on an attribute")),
            };
            en.clone().edit(s, "attrMeta", |st| {
                let at = st.attribute(p.as_str()).map_err(e)?;
                let v = match val.as_i64() {
                    Some(i) => Value::Int(i as i32),
                    None => Value::Token(val.as_str().unwrap_or("").into()),
                };
                at.set_metadata(key, v).map(|_| ()).map_err(e)
            })?;
            Ok(J::Null)
        }
        "relTargets" => {
            let en = entry(a)?;
            let r = en.stage.relationship(text(a, 1).as_str()).map_err(e)?;
            Ok(json!(r.targets().unwrap_or_default().iter().map(|p| p.as_str().to_string()).collect::<Vec<_>>()))
        }
        "relSet" => {
            let en = entry(a)?;
            let p = text(a, 1);
            let targets: Vec<sdf::Path> = a.get(2).and_then(J::as_array).map_or_else(Vec::new, |l| l.iter().filter_map(|x| x.as_str().and_then(|q| sdf::Path::new(q).ok())).collect());
            en.clone().edit(s, "rel", |st| st.relationship_builder(p.as_str()).set_targets(targets.clone()).build().map(|_| ()).map_err(e))?;
            Ok(J::Null)
        }

        // Layers.
        "layerInfo" => {
            let en = entry(a)?;
            let id = text(a, 1);
            let l = en.stage.layer(&id).ok_or_else(|| format!("no layer {id}"))?;
            let subs = l.pseudo_root().and_then(|r| r.sublayers()).unwrap_or_default();
            Ok(json!({"identifier": id, "realPath": l.resolved_path().unwrap_or(""), "anonymous": l.is_anonymous(), "dirty": en.is_dirty(&id), "subLayers": subs}))
        }
        "layerText" => {
            let en = entry(a)?;
            let id = text(a, 1);
            let l = en.stage.layer(&id).ok_or_else(|| format!("no layer {id}"))?;
            Ok(json!(l.export_to_string().map_err(e)?))
        }
        "layerExport" => {
            let en = entry(a)?;
            let id = text(a, 1);
            let l = en.stage.layer(&id).ok_or_else(|| format!("no layer {id}"))?;
            l.export(text(a, 2)).map_err(e)?;
            Ok(json!(true))
        }
        "layerImportText" => {
            let en = entry(a)?;
            let (id, t) = (text(a, 1), text(a, 2));
            en.clone().edit(s, "layer", |_| crate::layers::replace_contents(&en, &id, &t))?;
            Ok(json!(true))
        }
        "layerSave" => {
            let en = entry(a)?;
            let id = text(a, 1);
            let mut l = en.stage.layer_mut(&id).ok_or_else(|| format!("no layer {id}"))?;
            l.save().map_err(e)?;
            drop(l);
            en.set_clean(&id);
            Ok(json!(true))
        }
        "layerAddSub" => {
            let en = entry(a)?;
            let (parent, file) = (text(a, 1), text(a, 2));
            let pos = a.get(3).and_then(J::as_u64).unwrap_or(0) as usize;
            let layer = sdf::Layer::open(&file).map_err(|x| format!("{file}: {x}"))?;
            let out = layer.identifier().to_string();
            en.clone().edit(s, "sublayer", |st| st.insert_layer(&parent, pos, layer, LayerOffset::default()).map_err(e))?;
            Ok(json!(out))
        }
        other => Err(format!("usdPy: no operation {other}")),
    }
}

fn run(s: &mut Session, a: &Args) -> CmdResult {
    let op = a.pos_str(0).ok_or_else(|| CommandError::new("usdPy: an operation is required"))?.to_string();
    let rest = J::Array(a.positional.iter().skip(1).cloned().collect());
    run_op(s, &op, &rest).map_err(err)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(CommandDef::new("usdPy", "What the pxr Python module calls: an operation and its values.", run).positional("operation values...").flag(FlagDef::new("quiet", "q", ArgType::Bool, J::Null, "Accepted.")));
}
