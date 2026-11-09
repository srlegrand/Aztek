//! What the Channel Box and Attribute Editor show for a selected prim, in the shapes they already read, so a prim edits like
//! a node. Its transform appears as channels whatever ops it holds, and some of a prim's state reads as attributes that
//! aren't USD properties: `active`, `loaded`, `instanceable`, `kind`, `variant:<set>`.

use aztek_scene::session::Session;
use aztek_scene::transform::{decompose_full, euler_near, RotateOrder};
use glam::DVec3;
use openusd::sdf::Value;
use openusd::usd::{LoadPolicy, Prim, TimeCode};
use serde_json::{json, Value as J};

use crate::prim_cmds::{value_json, variant_sets};
use crate::prims::{self, local_of, resolve, selected, set_local, Target};
use crate::stages;

/// The channels of a prim's transform: translate, rotate and scale from its local matrix.
pub fn trs_of(prim: &Prim, time: f64) -> (DVec3, DVec3, DVec3) {
    let (m, _) = local_of(prim, time);
    let (t, r, s, _) = decompose_full(&m);
    // An op holding angles keeps them, past 180 and all; other stacks come from the matrix.
    let authored = ["XYZ", "YZX", "ZXY", "XZY", "YXZ", "ZYX"].iter().enumerate().find_map(|(i, o)| {
        let a = prim.attribute(format!("xformOp:rotate{o}").as_str());
        a.has_authored_value().unwrap_or(false).then_some((a, i))
    });
    let e = match authored {
        Some((a, i)) => {
            let near = a.get_at::<openusd::gf::Vec3f>(TimeCode::new(time)).ok().flatten().map_or(DVec3::ZERO, |v| DVec3::new(v.x as f64, v.y as f64, v.z as f64));
            euler_near(&r, RotateOrder::from_index(i as i64), near * (std::f64::consts::PI / 180.0)) * (180.0 / std::f64::consts::PI)
        }
        None => aztek_scene::transform::matrix_to_euler(&r, RotateOrder::Xyz) * (180.0 / std::f64::consts::PI),
    };
    (t, e, s)
}

fn channel(plug_base: &str, attr: &str, nice: &str, value: f64, ty: &str, sampled: bool) -> J {
    json!({
        "attr": attr,
        "plug": format!("{plug_base}.{attr}"),
        "nice": nice,
        "type": ty,
        "value": value,
        "enum": [],
        "connected": false,
        // What the Channel Box colours a keyed channel with.
        "state": if sampled { "keyed" } else { "" },
    })
}

fn op_sampled(prim: &Prim, name_starts: &str) -> bool {
    prim.authored_attributes().unwrap_or_default().iter().any(|a| a.name().starts_with(name_starts) && a.num_time_samples().unwrap_or(0) > 0)
}

/// The Channel Box for the selected prim. None when no prim is selected, so nodes show as usual.
pub fn channel_box(s: &mut Session) -> Option<J> {
    let name = selected().last().cloned()?;
    let t = resolve(&mut s.graph, &name).ok()?;
    let time = stages::time_of(&mut s.graph, t.shape);
    let prim = t.entry.stage.prim(t.path.clone()).ok()?;
    if !prim.is_valid().unwrap_or(false) {
        return None;
    }
    let (tr, ro, sc) = trs_of(&prim, time);
    let mut ch = Vec::new();
    for (word, nice, v, op) in [("translate", "Translate", tr, "xformOp:translate"), ("rotate", "Rotate", ro, "xformOp:rotate"), ("scale", "Scale", sc, "xformOp:scale")] {
        let sampled = op_sampled(&prim, op);
        for (k, axis) in ["X", "Y", "Z"].iter().enumerate() {
            ch.push(channel(&name, &format!("{word}{axis}"), &format!("{nice} {axis}"), v[k], "float", sampled));
        }
    }
    let vis = prim.attribute("visibility").get_at::<openusd::tf::Token>(TimeCode::new(time)).ok().flatten().is_none_or(|v| v.as_str() != "invisible");
    ch.push(json!({"attr": "visibility", "plug": format!("{name}.visibility"), "nice": "Visibility", "type": "bool", "value": vis, "enum": [], "connected": false, "state": ""}));
    // The prim's other simple values under the shape's heading.
    let mut own = Vec::new();
    for a in prim.authored_attributes().unwrap_or_default() {
        let n = a.name().to_string();
        if n.starts_with("xformOp") || n == "visibility" || n == "purpose" {
            continue;
        }
        let v = a.get_at::<Value>(TimeCode::new(time)).ok().flatten();
        let sampled = a.num_time_samples().unwrap_or(0) > 0;
        match v {
            Some(Value::Double(x)) => own.push(channel(&name, &n, &n, x, "float", sampled)),
            Some(Value::Float(x)) => own.push(channel(&name, &n, &n, x as f64, "float", sampled)),
            Some(Value::Int(x)) => own.push(channel(&name, &n, &n, x as f64, "int", sampled)),
            _ => {}
        }
    }
    let leaf = t.path.name().unwrap_or("").to_string();
    let ty = prim.type_name().ok().flatten().map_or(String::new(), |x| x.as_str().to_string());
    let shapes = if own.is_empty() { json!([]) } else { json!([{"name": format!("{leaf} ({ty})"), "path": name, "type": ty, "channels": own}]) };
    Some(json!({"name": leaf, "path": name, "type": ty, "channels": ch, "shapes": shapes, "inputs": [], "count": selected().len() + s.graph.selection().len()}))
}

fn field(attr: &str, plug: &str, nice: &str, ty: &str, value: J, enums: Vec<String>, output: bool, doc: &str) -> J {
    json!({
        "attr": attr, "plug": plug, "nice": nice, "type": ty, "value": value, "enum": enums,
        "children": ["", "", ""], "output": output, "keyable": !output, "category": "", "doc": doc,
        "min": J::Null, "max": J::Null, "connected": false, "source": J::Null, "displayColor": false,
        "color": ty == "vec3" && attr.to_ascii_lowercase().contains("color"), "pair": false, "file": false, "mappable": false,
    })
}

/// The Attribute Editor for the selected prim, or the named one.
pub fn attribute_editor(s: &mut Session, arg: &str) -> Option<J> {
    let name = if crate::prims::split_name(arg).is_some() { arg.to_string() } else if arg.is_empty() { selected().last().cloned()? } else { return None };
    let t = resolve(&mut s.graph, &name).ok()?;
    let time = stages::time_of(&mut s.graph, t.shape);
    let prim = t.entry.stage.prim(t.path.clone()).ok()?;
    if !prim.is_valid().unwrap_or(false) {
        return None;
    }
    let plug = |a: &str| format!("{name}.{a}");
    let mut attrs = Vec::new();
    let ty = prim.type_name().ok().flatten().map_or(String::new(), |x| x.as_str().to_string());
    attrs.push(field("primType", &plug("primType"), "Type", "string", json!(ty), vec![], true, "The prim's schema."));
    attrs.push(field("active", &plug("active"), "Active", "bool", json!(prim.is_active().unwrap_or(true)), vec![], false, "Whether the prim and what's below it compose."));
    attrs.push(field("instanceable", &plug("instanceable"), "Instanceable", "bool", json!(prim.is_instanceable().unwrap_or(false)), vec![], false, "Composes as a shared prototype."));
    let kind = prim.kind().ok().flatten().map_or(String::new(), |x| x.as_str().to_string());
    attrs.push(field("kind", &plug("kind"), "Kind", "string", json!(kind), vec![], false, "The model kind."));
    let info = crate::prim_cmds::prim_info(&prim, time);
    if info["hasPayload"].as_bool().unwrap_or(false) {
        attrs.push(field("loaded", &plug("loaded"), "Payload Loaded", "bool", json!(prim.is_loaded().unwrap_or(true)), vec![], false, "Load or unload its payload."));
    }
    for arc in info["references"].as_array().into_iter().flatten() {
        let text = format!("{} {}", arc["asset"].as_str().unwrap_or(""), arc["prim"].as_str().unwrap_or(""));
        attrs.push(field("reference", &plug("reference"), "Reference", "string", json!(text.trim()), vec![], true, "A reference on this prim."));
    }
    for arc in info["payloads"].as_array().into_iter().flatten() {
        let text = format!("{} {}", arc["asset"].as_str().unwrap_or(""), arc["prim"].as_str().unwrap_or(""));
        attrs.push(field("payload", &plug("payload"), "Payload", "string", json!(text.trim()), vec![], true, "A payload on this prim."));
    }
    for (set, options, chosen) in variant_sets(&prim) {
        let i = options.iter().position(|o| *o == chosen).unwrap_or(0);
        let attr = format!("variant:{set}");
        attrs.push(field(&attr, &plug(&attr), &format!("Variant: {set}"), "enum", json!(i), options, false, "A variant set."));
    }
    for a in prim.attributes().unwrap_or_default() {
        let n = a.name().to_string();
        let v = a.get_at::<Value>(TimeCode::new(time)).ok().flatten();
        let sampled = a.num_time_samples().unwrap_or(0);
        let doc = if sampled > 0 { format!("{sampled} time samples") } else { a.type_name().ok().flatten().map(|x| x.to_string()).unwrap_or_default() };
        let (kind, value, ro) = match &v {
            Some(Value::Bool(b)) => ("bool", json!(b), false),
            Some(Value::Int(x)) => ("int", json!(x), false),
            Some(Value::Float(x)) => ("float", json!(x), false),
            Some(Value::Double(x)) => ("float", json!(x), false),
            Some(Value::String(x)) => ("string", json!(x), false),
            Some(Value::Token(x)) => ("string", json!(x.as_str()), false),
            Some(Value::AssetPath(x)) => ("string", json!(x.asset_path()), false),
            Some(Value::Vec3f(x)) => ("vec3", json!([x.x, x.y, x.z]), false),
            Some(Value::Vec3d(x)) => ("vec3", json!([x.x, x.y, x.z]), false),
            Some(other) => ("string", value_json(other), true),
            None => continue,
        };
        attrs.push(field(&n, &plug(&n), &n, kind, value, vec![], ro, &doc));
    }
    let leaf = t.path.name().unwrap_or("").to_string();
    Some(json!({"tabs": [{"name": leaf, "path": name, "type": ty, "doc": "A USD prim.", "attrs": attrs}], "current": 0}))
}

/// Reads an attribute that isn't a USD property. None when the name is an ordinary one.
pub fn virtual_get(t: &Target, attr: &str, time: f64) -> Option<J> {
    let prim = t.entry.stage.prim(t.path.clone()).ok()?;
    let axis = |w: &str| attr.strip_prefix(w).and_then(|x| ["X", "Y", "Z"].iter().position(|a| *a == x));
    let (tr, ro, sc) = trs_of(&prim, time);
    if let Some(k) = axis("translate") {
        return Some(json!(tr[k]));
    }
    if let Some(k) = axis("rotate") {
        return Some(json!(ro[k]));
    }
    if let Some(k) = axis("scale") {
        return Some(json!(sc[k]));
    }
    match attr {
        "translate" => Some(json!(tr.to_array())),
        "rotate" => Some(json!(ro.to_array())),
        "scale" => Some(json!(sc.to_array())),
        "active" => Some(json!(prim.is_active().unwrap_or(true))),
        "loaded" => Some(json!(prim.is_loaded().unwrap_or(true))),
        "instanceable" => Some(json!(prim.is_instanceable().unwrap_or(false))),
        "kind" => Some(json!(prim.kind().ok().flatten().map_or(String::new(), |x| x.as_str().to_string()))),
        _ => {
            let set = attr.strip_prefix("variant:")?;
            variant_sets(&prim).into_iter().find(|x| x.0 == set).map(|(_, o, c)| json!(o.iter().position(|x| *x == c).unwrap_or(0)))
        }
    }
}

fn as_bool(v: &J) -> bool {
    v.as_bool().unwrap_or(v.as_f64().unwrap_or(0.0) != 0.0)
}

/// Sets an attribute that isn't a USD property. False when the name is an ordinary one.
pub fn virtual_set(s: &mut Session, t: &Target, attr: &str, v: &J, time: f64) -> Result<bool, String> {
    let entry = t.entry.clone();
    let path = t.path.clone();
    let axis = |w: &str| attr.strip_prefix(w).and_then(|x| ["X", "Y", "Z"].iter().position(|a| *a == x));
    let chan = [("translate", 0usize), ("rotate", 1), ("scale", 2)].iter().find_map(|(w, i)| {
        if attr == *w {
            return Some((*i, None));
        }
        axis(w).map(|k| (*i, Some(k)))
    });
    if let Some((which, comp)) = chan {
        entry.check_target()?;
        let prim = entry.stage.prim(path.clone()).map_err(|e| e.to_string())?;
        let (mut tr, mut ro, mut sc) = trs_of(&prim, time);
        let target = [&mut tr, &mut ro, &mut sc];
        let slot = target.into_iter().nth(which).expect("three");
        match comp {
            Some(k) => slot[k] = v.as_f64().ok_or("a number is needed")?,
            None => {
                let a = v.as_array().ok_or("three numbers are needed")?;
                for k in 0..3 {
                    slot[k] = a.get(k).and_then(J::as_f64).ok_or("three numbers are needed")?;
                }
            }
        }
        // Rebuilt the way set_local reads it apart: an op stack's order stands.
        let order = crate::prims::rotation_order(&prim);
        let m = glam::DMat4::from_translation(tr) * glam::DMat4::from_mat3(glam::DMat3::from_mat4(aztek_scene::transform::euler_to_matrix(ro * (std::f64::consts::PI / 180.0), order))) * glam::DMat4::from_scale(sc);
        entry.edit(s, "setAttr", |st| {
            let p = st.prim(path.clone()).map_err(|e| e.to_string())?;
            set_local(&p, &m, time)?;
            prims::keep_euler(&p, ro, time)
        })?;
        return Ok(true);
    }
    match attr {
        "visibility" => {
            entry.check_target()?;
            let val = if as_bool(v) { "inherited" } else { "invisible" };
            entry.edit(s, "setAttr", |st| {
                let p = st.prim(path.clone()).map_err(|e| e.to_string())?;
                let ap = p.attribute("visibility").path().clone();
                st.create_attribute(ap, "token").map_err(|e| e.to_string())?.set(Value::Token(val.into())).map_err(|e| e.to_string())?;
                Ok(())
            })?;
            Ok(true)
        }
        "active" | "instanceable" | "kind" => {
            entry.check_target()?;
            entry.edit(s, "setAttr", |st| {
                let p = st.prim(path.clone()).map_err(|e| e.to_string())?;
                match attr {
                    "active" => p.set_active(as_bool(v)).map(|_| ()),
                    "instanceable" => p.set_instanceable(as_bool(v)).map(|_| ()),
                    _ => p.set_kind(v.as_str().unwrap_or("")).map(|_| ()),
                }
                .map_err(|e| e.to_string())
            })?;
            Ok(true)
        }
        "loaded" => {
            let r = if as_bool(v) { entry.stage.load(path.clone(), LoadPolicy::default()) } else { entry.stage.unload(path.clone()) };
            r.map_err(|e| e.to_string())?;
            Ok(true)
        }
        _ => {
            let Some(set) = attr.strip_prefix("variant:") else { return Ok(false) };
            entry.check_target()?;
            let prim = entry.stage.prim(path.clone()).map_err(|e| e.to_string())?;
            let (_, options, _) = variant_sets(&prim).into_iter().find(|x| x.0 == set).ok_or_else(|| format!("no variant set {set}"))?;
            let choice = match v {
                J::String(x) => x.clone(),
                other => options.get(other.as_f64().unwrap_or(0.0) as usize).cloned().ok_or("no such variant")?,
            };
            crate::prim_cmds::run_set_variant(s, t, set, &choice)?;
            Ok(true)
        }
    }
}
