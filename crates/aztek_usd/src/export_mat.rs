//! Shading groups as Material prims with a UsdPreviewSurface, file nodes as UsdUVTexture, and the bindings on meshes.

use std::collections::{HashMap, HashSet};

use aztek_scene::plug::{NodeId, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::shading::{self, DrawMaterial, TextureRef};
use openusd::gf;
use openusd::sdf::{AssetPath, Specifier, Value};

use crate::export::{Item, Kind, Opts};
use crate::out::{Out, Track};
use crate::util::*;

/// How a texture's path is written: relative to the file being written where it can be, unless asked otherwise.
pub fn texture_path(o: &Opts, path: &str) -> String {
    if o.relative_textures == "absolute" {
        return std::path::absolute(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string());
    }
    let abs = std::path::absolute(path).map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| path.to_string());
    relative_to(&abs, &o.out_dir).unwrap_or_else(|| path.to_string())
}

fn c3(c: [f32; 3]) -> Value {
    Value::Vec3f(gf::vec3f(c[0], c[1], c[2]))
}

/// A UsdUVTexture, with its st reader and a UsdTransform2d when the placement moves anything.
fn write_texture(o: &Opts, s: &Session, mat: &str, t: &TextureRef, out: &mut Out) -> Result<String, String> {
    let name = prim_name(s.graph.node(t.file).map_or("file", |n| n.name()), o.strip_namespaces);
    let path = format!("{mat}/{name}");
    if out.has_prim(&path) {
        return Ok(path);
    }
    out.prim(&path, Specifier::Def, "Shader")?;
    out.uniform(&path, "info:id", "token", Value::Token("UsdUVTexture".into()))?;
    let file = t.resolved.clone().unwrap_or_else(|| t.written.clone());
    let written = texture_path(o, &file);
    out.attr(&path, "inputs:file", "asset", Value::AssetPath(AssetPath::new(written)))?;
    out.attr(&path, "inputs:sourceColorSpace", "token", Value::Token(if t.srgb { "sRGB".into() } else { "raw".into() }))?;
    let wrap = |on: bool, mirror: bool| Value::Token(if mirror { "mirror".into() } else if on { "repeat".into() } else { "black".into() });
    out.attr(&path, "inputs:wrapS", "token", wrap(t.wrap[0], t.mirror[0]))?;
    out.attr(&path, "inputs:wrapT", "token", wrap(t.wrap[1], t.mirror[1]))?;
    out.attr_track(&path, "outputs:rgb", "float3", &Track::default())?;
    out.attr_track(&path, "outputs:a", "float", &Track::default())?;

    let reader = format!("{mat}/TexCoordReader");
    if !out.has_prim(&reader) {
        out.prim(&reader, Specifier::Def, "Shader")?;
        out.uniform(&reader, "info:id", "token", Value::Token("UsdPrimvarReader_float2".into()))?;
        out.attr(&reader, "inputs:varname", "string", Value::String("st".into()))?;
        out.attr_track(&reader, "outputs:result", "float2", &Track::default())?;
    }
    let moved = t.repeat != [1.0, 1.0] || t.offset != [0.0, 0.0] || t.rotate_uv != 0.0;
    if moved {
        let xf = format!("{path}_transform");
        out.prim(&xf, Specifier::Def, "Shader")?;
        out.uniform(&xf, "info:id", "token", Value::Token("UsdTransform2d".into()))?;
        out.connect(&xf, "inputs:in", "float2", &format!("{reader}.outputs:result"))?;
        out.attr(&xf, "inputs:scale", "float2", Value::Vec2f(gf::vec2f(t.repeat[0], t.repeat[1])))?;
        out.attr(&xf, "inputs:translation", "float2", Value::Vec2f(gf::vec2f(t.offset[0], t.offset[1])))?;
        out.attr(&xf, "inputs:rotation", "float", Value::Float(t.rotate_uv.to_degrees()))?;
        out.attr_track(&xf, "outputs:result", "float2", &Track::default())?;
        out.connect(&path, "inputs:st", "float2", &format!("{xf}.outputs:result"))?;
    } else {
        out.connect(&path, "inputs:st", "float2", &format!("{reader}.outputs:result"))?;
    }
    Ok(path)
}

fn write_material(s: &mut Session, o: &Opts, sg: NodeId, d: &DrawMaterial, scope: &str, out: &mut Out) -> Result<String, String> {
    let sg_name = prim_name(s.graph.node(sg).map_or("material", |n| n.name()), o.strip_namespaces);
    let mat = format!("{scope}/{sg_name}");
    if out.has_prim(&mat) {
        return Ok(mat);
    }
    out.prim(&mat, Specifier::Def, "Material")?;
    let shader_name = prim_name(s.graph.node(d.material).map_or("surface", |n| n.name()), o.strip_namespaces);
    let shader = format!("{mat}/{shader_name}");
    out.prim(&shader, Specifier::Def, "Shader")?;
    out.uniform(&shader, "info:id", "token", Value::Token("UsdPreviewSurface".into()))?;
    if let Some(t) = &d.color_map {
        let tex = write_texture(o, s, &mat, t, out)?;
        out.connect(&shader, "inputs:diffuseColor", "color3f", &format!("{tex}.outputs:rgb"))?;
    } else {
        out.attr(&shader, "inputs:diffuseColor", "color3f", c3(d.color))?;
    }
    if let Some(t) = &d.alpha_map {
        let tex = write_texture(o, s, &mat, t, out)?;
        out.connect(&shader, "inputs:opacity", "float", &format!("{tex}.outputs:a"))?;
    } else if d.transparency > 0.0 {
        out.attr(&shader, "inputs:opacity", "float", Value::Float(1.0 - d.transparency))?;
    }
    if d.incandescence != [0.0, 0.0, 0.0] {
        out.attr(&shader, "inputs:emissiveColor", "color3f", c3(d.incandescence))?;
    }
    let type_name = s.graph.node(d.material).map_or(String::new(), |n| n.ty().name.clone());
    let g = &mut s.graph;
    let m = d.material;
    match type_name.as_str() {
        "blinn" | "phong" | "phongE" | "anisotropic" => {
            let rough = match type_name.as_str() {
                "blinn" => get_f64(g, m, "eccentricity").unwrap_or(0.3),
                "phong" => (2.0 / (get_f64(g, m, "cosinePower").unwrap_or(20.0) + 2.0)).sqrt(),
                _ => get_f64(g, m, "roughness").unwrap_or(0.5),
            };
            out.attr(&shader, "inputs:useSpecularWorkflow", "int", Value::Int(1))?;
            out.attr(&shader, "inputs:roughness", "float", Value::Float(rough as f32))?;
            if let Some(c) = get_v3(g, m, "specularColor") {
                out.attr(&shader, "inputs:specularColor", "color3f", Value::Vec3f(v3f(c)))?;
            }
        }
        "standardSurface" => {
            out.attr(&shader, "inputs:metallic", "float", Value::Float(get_f64(g, m, "metalness").unwrap_or(0.0) as f32))?;
            out.attr(&shader, "inputs:roughness", "float", Value::Float(get_f64(g, m, "specularRoughness").unwrap_or(0.2) as f32))?;
        }
        _ => {
            out.attr(&shader, "inputs:roughness", "float", Value::Float(1.0))?;
        }
    }
    out.attr_track(&shader, "outputs:surface", "token", &Track::default())?;
    out.connect(&mat, "outputs:surface", "token", &format!("{shader}.outputs:surface"))?;
    Ok(mat)
}

fn bind(out: &mut Out, prim: &str, material: &str) -> Result<(), String> {
    out.apply_api(prim, "MaterialBindingAPI")?;
    out.rel(prim, "material:binding", &[material.to_string()])
}

/// Writes every material used and binds it. Returns the scope's path when one was made.
pub fn write(s: &mut Session, o: &Opts, items: &[Item], _roots: &[NodeId], base: &str, out: &mut Out, proto: &HashMap<NodeId, String>) -> Result<Option<String>, String> {
    if !o.export_materials && !o.export_display_color || o.shading_mode == "none" {
        return Ok(None);
    }
    let root_items: Vec<&Item> = items.iter().filter(|i| i.is_root).collect();
    let scope = if !base.is_empty() {
        format!("{base}/{}", o.materials_scope)
    } else if root_items.len() == 1 && root_items[0].kind == Kind::Group {
        format!("{}/{}", root_items[0].path, o.materials_scope)
    } else {
        format!("/{}", o.materials_scope)
    };
    let scene = s.scene_path.clone();
    let project = shading::project_dir(s);
    let mut cache: HashMap<NodeId, Option<DrawMaterial>> = HashMap::new();
    let mut used: HashSet<NodeId> = HashSet::new();
    let mut targets: Vec<(NodeId, String)> = items.iter().filter(|i| i.kind == Kind::Mesh && i.instance_of.is_none() && i.shape != NO_NODE).map(|i| (i.shape, i.shape_path.clone())).collect();
    targets.extend(proto.iter().map(|(k, v)| (*k, v.clone())));

    for (shape, prim) in targets {
        let groups = shading::membership_of(&s.graph, shape);
        if groups.is_empty() {
            continue;
        }
        let whole = groups.len() == 1 && groups[0].1.is_none();
        for (sg, faces) in groups {
            let d = match cache.get(&sg) {
                Some(x) => x.clone(),
                None => {
                    let x = shading::draw_material(&mut s.graph, sg, scene.as_deref(), project.as_deref());
                    cache.insert(sg, x.clone());
                    x
                }
            };
            let Some(d) = d else { continue };
            used.insert(sg);
            if o.export_display_color && whole {
                let t = Track { default: Some(Value::Vec3fVec(vec![gf::vec3f(d.color[0], d.color[1], d.color[2])])), samples: Vec::new() };
                out.primvar(&prim, "displayColor", "color3f[]", &t, "constant", None, 1)?;
            }
            if !o.export_materials {
                continue;
            }
            if !out.has_prim(&scope) {
                out.prim(&scope, Specifier::Def, "Scope")?;
            }
            let mat = write_material(s, o, sg, &d, &scope, out)?;
            match faces {
                None => bind(out, &prim, &mat)?,
                Some(ids) => {
                    // Some of the mesh's faces: a GeomSubset, in the family UsdShade reads.
                    let name = prim_name(s.graph.node(sg).map_or("subset", |n| n.name()), o.strip_namespaces);
                    let sub = format!("{prim}/{name}");
                    out.prim(&sub, Specifier::Def, "GeomSubset")?;
                    out.uniform(&sub, "elementType", "token", Value::Token("face".into()))?;
                    out.uniform(&sub, "familyName", "token", Value::Token("materialBind".into()))?;
                    out.attr(&sub, "indices", "int[]", Value::IntVec(ids.iter().map(|i| *i as i32).collect()))?;
                    bind(out, &sub, &mat)?;
                    out.apply_api(&prim, "MaterialBindingAPI")?;
                    out.uniform(&prim, "subsetFamily:materialBind:familyType", "token", Value::Token("nonOverlapping".into()))?;
                }
            }
        }
    }
    if o.export_materials && !o.assigned_only {
        for sg in shading::all_groups(&s.graph) {
            if used.contains(&sg) {
                continue;
            }
            if let Some(d) = shading::draw_material(&mut s.graph, sg, scene.as_deref(), project.as_deref()) {
                if !out.has_prim(&scope) {
                    out.prim(&scope, Specifier::Def, "Scope")?;
                }
                write_material(s, o, sg, &d, &scope, out)?;
            }
        }
    }
    Ok(out.has_prim(&scope).then_some(scope))
}
