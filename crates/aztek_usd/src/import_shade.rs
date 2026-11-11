//! UsdPreviewSurface materials as lambert, blinn or standardSurface with file textures, and their bindings as shading group
//! membership.

use aztek_geo::attribute::INVALID;
use aztek_scene::plug::{NodeId, PlugValue};
use aztek_scene::shading;
use glam::DVec3;
use openusd::gf;
use openusd::sdf::{self, AssetPath};
use openusd::tf::Token;
use openusd::usd::{Attribute, Prim};
use serde_json::{json, Value as J};

use crate::import::Ctx;
use crate::mesh_read::{at, constant_display_color, get_vec, token_of};
use crate::util::*;

fn result_name(v: &J) -> String {
    match v {
        J::String(s) => s.clone(),
        J::Array(a) => a.first().and_then(J::as_str).unwrap_or("").to_string(),
        _ => String::new(),
    }
}

fn find(c: &Ctx, name: &str) -> Option<NodeId> {
    c.s.graph.find(name).ok().flatten()
}

/// The first prim an attribute connects to.
fn source_of(c: &Ctx, a: &Attribute) -> Option<(Prim, String)> {
    let t = a.connections().ok()?;
    let first = t.first()?;
    let (prim, prop) = first.split_property()?;
    Some((c.stage.prim(prim).ok()?, prop.to_string()))
}

fn v3f_of(a: &Attribute) -> Option<DVec3> {
    let v = a.get_at::<gf::Vec3f>(at(None)).ok().flatten()?;
    Some(DVec3::new(v.x as f64, v.y as f64, v.z as f64))
}

fn float_of(a: &Attribute) -> Option<f64> {
    a.get_at::<f32>(at(None)).ok().flatten().map(f64::from)
}

fn shader_id(p: &Prim) -> String {
    token_of(p, "info:id").unwrap_or_default()
}

/// The surface shader a Material's output reaches.
fn surface_shader(c: &Ctx, mat: &Prim) -> Option<Prim> {
    for name in ["outputs:surface", "outputs:displayColor:surface", "outputs:mtlx:surface"] {
        if let Some((p, _)) = source_of(c, &mat.attribute(name)) {
            return Some(p);
        }
    }
    None
}

/// A file node reading the texture a connection comes from, placed as the shader graph places it.
fn make_texture(c: &mut Ctx, tex: &Prim) -> Result<Option<NodeId>, String> {
    let Ok(Some(asset)) = tex.attribute("inputs:file").get_at::<AssetPath>(at(None)) else { return Ok(None) };
    let path = crate::mesh_read::asset_on_disk(asset.resolved_path().unwrap_or(asset.asset_path()));
    let name = node_name(tex.path().name().unwrap_or("file"));
    let made = c.s.run("shadingNode", json!({"args": ["file"], "asTexture": true, "name": name})).map_err(|e| e.to_string())?;
    let file = find(c, &result_name(&made)).ok_or("no file node")?;
    let place_made = c.s.run("shadingNode", json!({"args": ["place2dTexture"], "asUtility": true})).map_err(|e| e.to_string())?;
    let place = find(c, &result_name(&place_made)).ok_or("no placement")?;
    shading::connect_placement(&mut c.s.graph, place, file).map_err(|e| e.to_string())?;
    let g = &mut c.s.graph;
    set_attr(g, file, "fileTextureName", PlugValue::String(path));
    let space = token_of(tex, "inputs:sourceColorSpace").unwrap_or_default();
    set_attr(g, file, "colorSpace", PlugValue::String(if space == "raw" { "Raw".into() } else { "sRGB".into() }));
    for (input, wrap, mirror) in [("inputs:wrapS", "wrapU", "mirrorU"), ("inputs:wrapT", "wrapV", "mirrorV")] {
        match token_of(tex, input).as_deref() {
            Some("black" | "clamp") => set_attr(g, place, wrap, PlugValue::Bool(false)),
            Some("mirror") => set_attr(g, place, mirror, PlugValue::Bool(true)),
            _ => {}
        }
    }
    // A UsdTransform2d between the texture and its st reader.
    if let Some((xf, _)) = source_of(c, &tex.attribute("inputs:st")) {
        if shader_id(&xf) == "UsdTransform2d" {
            let name = c.s.graph.path(place);
            let put = |c: &mut Ctx, attr: &str, v: f64| {
                let _ = c.s.run("setAttr", json!({"args": [format!("{name}.{attr}"), v]}));
            };
            if let Ok(Some(v)) = xf.attribute("inputs:scale").get_at::<gf::Vec2f>(at(None)) {
                put(c, "repeatU", v.x as f64);
                put(c, "repeatV", v.y as f64);
            }
            if let Ok(Some(v)) = xf.attribute("inputs:translation").get_at::<gf::Vec2f>(at(None)) {
                put(c, "offsetU", v.x as f64);
                put(c, "offsetV", v.y as f64);
            }
            if let Some(v) = float_of(&xf.attribute("inputs:rotation")) {
                put(c, "rotateUV", v);
            }
        }
    }
    Ok(Some(file))
}

/// The input's texture source, if it connects to a UsdUVTexture.
fn texture_input(c: &Ctx, a: &Attribute) -> Option<Prim> {
    let (p, _) = source_of(c, a)?;
    (shader_id(&p) == "UsdUVTexture").then_some(p)
}

fn connect(c: &mut Ctx, from: &str, to: &str) -> Result<(), String> {
    c.s.run("connectAttr", json!({"args": [from, to], "force": true})).map(|_| ()).map_err(|e| e.to_string())
}

/// The material type to make: asked for, or what the shader's inputs suggest.
fn material_type(c: &Ctx, sh: &Prim) -> String {
    if !c.o.preferred_material.is_empty() && c.o.preferred_material != "none" {
        return c.o.preferred_material.clone();
    }
    let spec = sh.attribute("inputs:useSpecularWorkflow").get_at::<i32>(at(None)).ok().flatten() == Some(1);
    if spec {
        return "blinn".into();
    }
    let metallic = float_of(&sh.attribute("inputs:metallic")).is_some() || sh.attribute("inputs:metallic").has_authored_connections().unwrap_or(false);
    let rough = float_of(&sh.attribute("inputs:roughness"));
    if metallic || rough.is_some_and(|r| r < 1.0) {
        return "standardSurface".into();
    }
    "lambert".into()
}

fn make_group(c: &mut Ctx, name: &str) -> Result<String, String> {
    let made = c.s.run("sets", json!({"renderable": true, "noSurfaceShader": true, "empty": true, "name": name})).map_err(|e| e.to_string())?;
    Ok(result_name(&made))
}

/// The shading group for a Material prim, made once. None when the prim has no surface shader.
fn group_for(c: &mut Ctx, path: &sdf::Path) -> Result<Option<String>, String> {
    let key = path.as_str().to_string();
    if let Some(v) = c.materials.get(&key) {
        return Ok(v.clone());
    }
    let result = build_material(c, path)?;
    c.materials.insert(key, result.clone());
    Ok(result)
}

fn build_material(c: &mut Ctx, path: &sdf::Path) -> Result<Option<String>, String> {
    let mat = c.stage.prim(path.clone()).map_err(|e| e.to_string())?;
    if !mat.is_valid().unwrap_or(false) {
        return Ok(None);
    }
    let mat_name = node_name(path.name().unwrap_or("material"));
    if mat_name == shading::INITIAL_SG {
        return Ok(Some(mat_name));
    }
    let Some(sh) = surface_shader(c, &mat) else { return Ok(None) };
    let ty = material_type(c, &sh);
    let name = node_name(sh.path().name().unwrap_or("surface"));
    let made = c.s.run("shadingNode", json!({"args": [ty.clone()], "asShader": true, "name": name})).map_err(|e| e.to_string())?;
    let shader_name = result_name(&made);
    let sn = find(c, &shader_name).ok_or("no shader")?;
    let sg = make_group(c, &mat_name)?;
    connect(c, &format!("{shader_name}.outColor"), &format!("{sg}.surfaceShader"))?;

    let standard = ty == "standardSurface";
    let color_attr = if standard { "baseColor" } else { "color" };
    let d = sh.attribute("inputs:diffuseColor");
    if let Some(tp) = texture_input(c, &d) {
        if let Some(file) = make_texture(c, &tp)? {
            let f = c.s.graph.path(file);
            connect(c, &format!("{f}.outColor"), &format!("{shader_name}.{color_attr}"))?;
        }
    } else if let Some(v) = v3f_of(&d) {
        set_attr(&mut c.s.graph, sn, color_attr, PlugValue::Vec3(v));
    }
    let op = sh.attribute("inputs:opacity");
    if let Some(tp) = texture_input(c, &op) {
        if let Some(file) = make_texture(c, &tp)? {
            let f = c.s.graph.path(file);
            if !standard {
                connect(c, &format!("{f}.outTransparency"), &format!("{shader_name}.transparency"))?;
            }
        }
    } else if let Some(v) = float_of(&op) {
        if standard {
            set_attr(&mut c.s.graph, sn, "opacity", PlugValue::Vec3(DVec3::splat(v)));
        } else {
            set_attr(&mut c.s.graph, sn, "transparency", PlugValue::Vec3(DVec3::splat(1.0 - v)));
        }
    }
    let g = &mut c.s.graph;
    if let Some(v) = v3f_of(&sh.attribute("inputs:emissiveColor")) {
        if standard {
            set_attr(g, sn, "emissionColor", PlugValue::Vec3(v));
            set_attr(g, sn, "emission", PlugValue::Float(1.0));
        } else {
            set_attr(g, sn, "incandescence", PlugValue::Vec3(v));
        }
    }
    if let Some(r) = float_of(&sh.attribute("inputs:roughness")) {
        match ty.as_str() {
            "blinn" => set_attr(g, sn, "eccentricity", PlugValue::Float(r)),
            "standardSurface" => set_attr(g, sn, "specularRoughness", PlugValue::Float(r)),
            "phong" => set_attr(g, sn, "cosinePower", PlugValue::Float((2.0 / (r * r).max(1e-4) - 2.0).max(2.0))),
            _ => {}
        }
    }
    if let Some(v) = v3f_of(&sh.attribute("inputs:specularColor")) {
        set_attr(g, sn, "specularColor", PlugValue::Vec3(v));
    }
    if standard {
        if let Some(m) = float_of(&sh.attribute("inputs:metallic")) {
            set_attr(g, sn, "metalness", PlugValue::Float(m));
        }
    }
    Ok(Some(sg))
}

/// The material a prim is bound to, inherited down namespace.
fn bound_material(prim: &Prim) -> Option<sdf::Path> {
    let mut p = Some(prim.clone());
    while let Some(q) = p {
        if let Ok(t) = q.relationship("material:binding").targets() {
            if let Some(first) = t.first() {
                return Some(first.clone());
            }
        }
        p = q.parent();
    }
    None
}

/// `shape.f[a:b]` strings for a sorted list of faces.
fn face_ranges(shape: &str, mut faces: Vec<u32>) -> Vec<String> {
    faces.sort_unstable();
    faces.dedup();
    let mut out = Vec::new();
    let mut i = 0;
    while i < faces.len() {
        let mut j = i;
        while j + 1 < faces.len() && faces[j + 1] == faces[j] + 1 {
            j += 1;
        }
        out.push(if i == j { format!("{shape}.f[{}]", faces[i]) } else { format!("{shape}.f[{}:{}]", faces[i], faces[j]) });
        i = j + 1;
    }
    out
}

fn assign(c: &mut Ctx, members: Vec<String>, sg: &str) -> Result<(), String> {
    c.s.run("sets", json!({"args": members, "edit": true, "forceElement": sg})).map(|_| ()).map_err(|e| e.to_string())
}

pub fn read_all(c: &mut Ctx) -> Result<(), String> {
    let mut meshes: Vec<(String, NodeId, Vec<u32>)> = c.meshes.iter().map(|(k, v)| (k.clone(), v.0, v.1.clone())).collect();
    meshes.sort_by(|a, b| a.0.cmp(&b.0));
    for (path, shape, face_map) in meshes {
        let prim = c.stage.prim(path.as_str()).map_err(|e| e.to_string())?;
        let name = c.s.graph.path(shape);
        if let Some(m) = bound_material(&prim) {
            if let Some(sg) = group_for(c, &m)? {
                if sg != shading::INITIAL_SG {
                    assign(c, vec![name.clone()], &sg)?;
                }
            }
        } else if let Some(col) = constant_display_color(&prim) {
            let sg = display_group(c, col)?;
            assign(c, vec![name.clone()], &sg)?;
        }
        for k in prim.children().unwrap_or_default() {
            if k.type_name().ok().flatten().is_none_or(|t| t.as_str() != "GeomSubset") {
                continue;
            }
            if k.attribute("familyName").get::<Token>().ok().flatten().is_some_and(|t| t.as_str() != "materialBind") {
                continue;
            }
            let Some(m) = bound_material_on(&k) else { continue };
            let Some(sg) = group_for(c, &m)? else { continue };
            let ids = get_vec::<i32>(&k.attribute("indices"), None).unwrap_or_default();
            let faces: Vec<u32> = ids
                .iter()
                .filter_map(|&i| {
                    let f = *face_map.get(i.max(0) as usize)?;
                    (f != INVALID).then_some(f)
                })
                .collect();
            if !faces.is_empty() {
                assign(c, face_ranges(&name, faces), &sg)?;
            }
        }
    }
    Ok(())
}

fn bound_material_on(k: &Prim) -> Option<sdf::Path> {
    k.relationship("material:binding").targets().ok().and_then(|t| t.first().cloned())
}

/// A lambert for prims with a display colour and no material; prims of one colour share it.
fn display_group(c: &mut Ctx, col: [f32; 3]) -> Result<String, String> {
    let key = col.map(f32::to_bits);
    if let Some(g) = c.display_groups.get(&key) {
        return Ok(g.clone());
    }
    let made = c.s.run("shadingNode", json!({"args": ["lambert"], "asShader": true, "name": "displayColorMat"})).map_err(|e| e.to_string())?;
    let shader = result_name(&made);
    let sn = find(c, &shader).ok_or("no shader")?;
    set_attr(&mut c.s.graph, sn, "color", PlugValue::Vec3(DVec3::new(col[0] as f64, col[1] as f64, col[2] as f64)));
    let sg = make_group(c, &format!("{shader}SG"))?;
    connect(c, &format!("{shader}.outColor"), &format!("{sg}.surfaceShader"))?;
    c.display_groups.insert(key, sg.clone());
    Ok(sg)
}
