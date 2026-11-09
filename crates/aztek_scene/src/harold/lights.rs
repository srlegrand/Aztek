//! Lights into Harold's: Maya's six and Arnold's four, with their exposure, samples, radius, shadows and weights.
//! Maya's decay rate is not used: light falls off with the square of distance, as in Arnold, unless aiDecayType asks for
//! none.

use std::sync::Arc;

use aztek_render::geom::vis;
use aztek_render::lights::{Ies, Light, Shape};
use aztek_render::math::Frame;
use aztek_render::scene::Scene;
use aztek_render::shader::{Input, Material, Standard};
use aztek_render::texture::{ColorSpace, TextureKey};
use glam::{Mat4, Vec3};

use super::materials::MatBuilder;
use super::network::{get_b, get_f, get_s, get_v};
use crate::graph::Graph;
use crate::plug::NodeId;
use crate::transform::DagNode;

fn world(g: &Graph, id: NodeId) -> Mat4 {
    g.node(id).map(|n| n.data().matrix(DagNode::WORLD_MATRIX).as_mat4()).unwrap_or(Mat4::IDENTITY)
}

/// The dome's image: the file node feeding its colour.
fn dome_image(mb: &mut MatBuilder, id: NodeId) -> Option<TextureKey> {
    let p = super::network::attr(mb.nb.g, id, "color")?;
    let src = mb.nb.g.source(&p)?;
    let n = src.node;
    let ty = mb.nb.g.node(n)?.ty().name.clone();
    let attr = match ty.as_str() {
        "file" => "fileTextureName",
        "aiImage" => "filename",
        _ => {
            mb.nb.warn(format!("sky dome colour from {ty}: only an image file is honoured"));
            return None;
        }
    };
    let written = get_s(mb.nb.g, n, attr);
    let resolved = crate::shading::resolve_texture(&written, mb.nb.scene_path.as_deref(), mb.nb.project.as_deref())?;
    let space = ColorSpace::parse(&get_s(mb.nb.g, n, "colorSpace"));
    Some(TextureKey { path: resolved.display().to_string(), space })
}

pub fn translate(mb: &mut MatBuilder, scene: &mut Scene, warnings: &mut Vec<String>) -> Vec<(Light, NodeId)> {
    let mut out = Vec::new();
    let physical = mb.nb.g.find(super::settings::OPTIONS).ok().flatten().is_some_and(|o| get_b(mb.nb.g, o, "physicalLights", false));
    let shapes = crate::lights::all_shapes(mb.nb.g);
    for shape in shapes {
        if !super::visible(mb.nb.g, shape) {
            continue;
        }
        let g = &mut *mb.nb.g;
        let Some(ty) = g.node(shape).map(|n| n.ty().name.clone()) else { continue };
        let name = g.node(shape).map(|n| n.name().to_string()).unwrap_or_default();
        let w = world(g, shape);
        let pos = w.transform_point3(Vec3::ZERO);
        let fwd = w.transform_vector3(-Vec3::Z).normalize_or_zero();
        let exposure = get_f(g, shape, "aiExposure", 0.0) as f32;
        let intensity = get_f(g, shape, "intensity", 1.0) as f32;
        let color = get_v(g, shape, "color", Vec3::ONE) * intensity * 2f32.powf(exposure);
        let radius = get_f(g, shape, "aiRadius", 0.0) as f32;
        let normalize = get_b(g, shape, "aiNormalize", true) && get_b(g, shape, "normalize", true);
        // The reference's six lights light as the viewport lights them, unless physical lights are asked for.
        if !physical && !ty.starts_with("ai") {
            if let Some(sl) = crate::lights::scene_light(g, shape) {
                let angle = get_f(g, shape, "aiAngle", 0.0) as f32;
                let mut l = viewport_light(&sl, &name, radius, angle, exposure);
                let (decay, diffuse, specular) = (l.decay, l.diffuse, l.specular);
                fill_common(g, shape, &mut l, scene);
                l.decay = decay;
                l.diffuse = diffuse * get_f(g, shape, "aiDiffuse", 1.0) as f32;
                l.specular = specular * get_f(g, shape, "aiSpecular", 1.0) as f32;
                out.push((l, shape));
                continue;
            }
        }
        let shape_of = match ty.as_str() {
            "pointLight" => Shape::Point { pos, radius },
            "spotLight" => {
                let cone = get_f(g, shape, "coneAngle", 40_f64.to_radians()) as f32 * 0.5;
                let pen = get_f(g, shape, "penumbraAngle", 0.0) as f32;
                let (outer, inner) = if pen >= 0.0 { (cone + pen, cone) } else { (cone, (cone + pen).max(0.0)) };
                Shape::Spot { pos, radius, frame: Frame::from_n(fwd), cos_outer: outer.min(3.1).cos(), cos_inner: inner.cos(), dropoff: get_f(g, shape, "dropoff", 0.0) as f32 }
            }
            "directionalLight" => {
                let angle = get_f(g, shape, "aiAngle", 0.0).to_radians() as f32;
                Shape::Distant { dir: -fwd, cos_half: (angle * 0.5).cos() }
            }
            "ambientLight" => Shape::Ambient { pos, shade: get_f(g, shape, "ambientShade", 0.45) as f32 },
            "volumeLight" => {
                let Some(sl) = crate::lights::scene_light(g, shape) else { continue };
                Shape::Volume { pos, to_unit: w.inverse(), shape: sl.volume_shape, mode: sl.volume_dir, ambient: sl.volume_ambient, axis: fwd }
            }
            "areaLight" | "aiAreaLight" => {
                let kind = if ty == "aiAreaLight" { get_f(g, shape, "aiTranslator", 0.0) as i64 } else { 0 };
                match kind {
                    1 => {
                        let s = w.to_scale_rotation_translation().0;
                        Shape::Disk { center: pos, frame: Frame::from_nt(fwd, w.transform_vector3(Vec3::X).normalize_or_zero()), radius: s.x.abs().max(s.y.abs()), two_sided: false }
                    }
                    2 => {
                        let a = w.transform_point3(Vec3::new(0.0, -1.0, 0.0));
                        let b = w.transform_point3(Vec3::new(0.0, 1.0, 0.0));
                        let s = w.to_scale_rotation_translation().0;
                        Shape::Cylinder { a, b, radius: s.x.abs().max(s.z.abs()) }
                    }
                    _ => {
                        let corner = w.transform_point3(Vec3::new(-1.0, -1.0, 0.0));
                        let u = w.transform_point3(Vec3::new(1.0, -1.0, 0.0)) - corner;
                        let v = w.transform_point3(Vec3::new(-1.0, 1.0, 0.0)) - corner;
                        Shape::Quad { corner, u, v, n: fwd, two_sided: false, spread: get_f(g, shape, "aiSpread", 1.0) as f32 }
                    }
                }
            }
            "aiSkyDomeLight" => {
                let key = dome_image(mb, shape);
                let g = &mut *mb.nb.g;
                let format = get_f(g, shape, "format", 2.0) as i64;
                if format != 2 {
                    warnings.push(format!("{name}: only the latlong format is honoured"));
                }
                let image = key.and_then(|k| scene.textures.image(&k.path, k.space));
                // The dome turns with its transform.
                let (_, r, _) = w.to_scale_rotation_translation();
                let dome_color = if image.is_some() { Vec3::ONE * intensity * 2f32.powf(exposure) } else { color };
                let mut l = Light::new(&name, Shape::Dome { to_local: Mat4::from_quat(r.inverse()), image, dist: None }, dome_color);
                fill_common(g, shape, &mut l, scene);
                l.camera = get_f(g, shape, "camera", 1.0) as f32;
                out.push((l, shape));
                continue;
            }
            "aiPhotometricLight" => {
                let file = get_s(g, shape, "aiFilename");
                let ies = std::fs::read_to_string(&file).ok().and_then(|t| Ies::parse(&t)).map(Arc::new);
                if ies.is_none() {
                    warnings.push(format!("{name}: IES profile {file} not read"));
                }
                let mut l = Light::new(&name, Shape::Spot { pos, radius, frame: Frame::from_n(fwd), cos_outer: -1.0, cos_inner: -1.0, dropoff: 0.0 }, color);
                l.ies = ies;
                fill_common(g, shape, &mut l, scene);
                out.push((l, shape));
                continue;
            }
            "aiMeshLight" => {
                // The mesh it lights from: what feeds inMesh, found among the instances by name.
                let src = super::network::attr(g, shape, "inMesh").and_then(|p| g.source(&p)).map(|s| s.node);
                let Some(src) = src else {
                    warnings.push(format!("{name}: mesh light with no mesh"));
                    continue;
                };
                let path = g.path(src);
                let Some(inst) = scene.geo.instances.iter().position(|i| i.name == path) else { continue };
                let area_norm = normalize;
                // The mesh glows with the light's colour; normalised, its total is that of one unit of area.
                let mut st = Standard { base: Input::c(0.0), specular: Input::c(0.0), emission: Input::c(1.0), emission_color: Input::Const(color), ..Default::default() };
                let _ = area_norm;
                st.thin_walled = false;
                scene.materials.push(Material::Standard(Box::new(st)));
                scene.material_names.push(format!("{name}_emission"));
                let m = (scene.materials.len() - 1) as u32;
                let i = &mut scene.geo.instances[inst];
                i.shaders = vec![m; i.shaders.len().max(1)];
                if !get_b(g, shape, "lightVisible", false) {
                    i.visibility &= !vis::CAMERA;
                }
                let mut l = Light::new(&name, Shape::Mesh { inst: inst as u32, tris: Vec::new(), cdf: Vec::new(), area: 0.0 }, Vec3::ONE);
                fill_common(g, shape, &mut l, scene);
                out.push((l, shape));
                continue;
            }
            _ => continue,
        };
        let area = match &shape_of {
            Shape::Quad { u, v, .. } => u.cross(*v).length(),
            Shape::Disk { radius, .. } => std::f32::consts::PI * radius * radius,
            Shape::Cylinder { a, b, radius } => std::f32::consts::TAU * radius * (*b - *a).length(),
            _ => 1.0,
        };
        let c = if matches!(shape_of, Shape::Quad { .. } | Shape::Disk { .. } | Shape::Cylinder { .. }) && normalize { color / area.max(1e-6) } else { color };
        let mut l = Light::new(&name, shape_of, c);
        fill_common(g, shape, &mut l, scene);
        if ty == "areaLight" || ty == "aiAreaLight" {
            l.camera = get_f(g, shape, "camera", 0.0) as f32;
        }
        out.push((l, shape));
    }
    out
}

/// One of the reference's lights as the viewport lights it. The viewport's matte surface takes colour times light times
/// the cosine, where the renderer's divides by pi: so the light's colour here is pi times the viewport's. Lights with a
/// surface are sized so that, far off, they light as the viewport's single point of them does.
pub fn viewport_light(sl: &crate::lights::SceneLight, name: &str, radius: f32, angle_deg: f32, exposure: f32) -> Light {
    use crate::lights::Kind;
    let w = sl.world.as_mat4();
    let pos = w.transform_point3(Vec3::ZERO);
    let axis = w.transform_vector3(-Vec3::Z).normalize_or_zero();
    let c = Vec3::from_array(sl.color) * std::f32::consts::PI * 2f32.powf(exposure);
    let shape = match sl.kind {
        Kind::Ambient => Shape::Ambient { pos, shade: sl.ambient_shade as f32 },
        Kind::Directional => Shape::Distant { dir: -axis, cos_half: (angle_deg.to_radians() * 0.5).cos() },
        Kind::Point => Shape::Point { pos, radius },
        Kind::Spot => {
            let (outer, inner) = crate::lights::spot_cones(sl);
            Shape::Spot { pos, radius, frame: Frame::from_n(axis), cos_outer: outer as f32, cos_inner: inner as f32, dropoff: sl.dropoff as f32 }
        }
        Kind::Area => {
            let corner = w.transform_point3(Vec3::new(-1.0, -1.0, 0.0));
            let u = w.transform_point3(Vec3::new(1.0, -1.0, 0.0)) - corner;
            let v = w.transform_point3(Vec3::new(-1.0, 1.0, 0.0)) - corner;
            let area = u.cross(v).length().max(1e-9);
            // The viewport's light is pi c from the nearest point; spread over the area, times the half sides when
            // the light isn't normalised.
            let k = if sl.by_area { u.length() * v.length() * 0.25 } else { 1.0 };
            let mut l = Light::new(name, Shape::Quad { corner, u, v, n: axis, two_sided: false, spread: 1.0 }, c * k / area);
            l.decay = sl.decay as f32;
            l.diffuse = sl.diffuse as i32 as f32;
            l.specular = sl.specular as i32 as f32;
            return l;
        }
        Kind::Volume => Shape::Volume { pos, to_unit: w.inverse(), shape: sl.volume_shape, mode: sl.volume_dir, ambient: sl.volume_ambient, axis },
    };
    let mut l = Light::new(name, shape, c);
    // Decay: point and spot lights by their rate; directional, ambient and volume lights don't fall off.
    l.decay = if matches!(sl.kind, Kind::Point | Kind::Spot) { sl.decay as f32 } else { 0.0 };
    // The all-round part of ambient and volume lights is taken whatever emitDiffuse says, as the viewport does.
    l.diffuse = if sl.diffuse || matches!(sl.kind, Kind::Ambient) { 1.0 } else { 0.0 };
    l.specular = sl.specular as i32 as f32;
    l
}

fn fill_common(g: &mut Graph, shape: NodeId, l: &mut Light, scene: &mut Scene) {
    l.samples = get_f(g, shape, "aiSamples", 1.0).max(0.0) as u32;
    l.cast_shadows = get_b(g, shape, "aiCastShadows", true);
    l.shadow_color = get_v(g, shape, "shadowColor", Vec3::ZERO);
    l.shadow_density = get_f(g, shape, "aiShadowDensity", 1.0) as f32;
    let emit_d = get_b(g, shape, "emitDiffuse", true);
    let emit_s = get_b(g, shape, "emitSpecular", true);
    l.diffuse = get_f(g, shape, "aiDiffuse", 1.0) as f32 * if emit_d { 1.0 } else { 0.0 };
    l.specular = get_f(g, shape, "aiSpecular", 1.0) as f32 * if emit_s { 1.0 } else { 0.0 };
    l.sss = get_f(g, shape, "aiSss", 1.0) as f32;
    l.transmission = get_f(g, shape, "aiTransmission", 1.0) as f32;
    l.volume = get_f(g, shape, "aiVolume", 1.0) as f32;
    l.max_bounces = get_f(g, shape, "aiMaxBounces", 999.0).max(0.0) as u32;
    l.decay = if get_f(g, shape, "aiDecayType", 1.0) as i64 == 0 { 0.0 } else { 2.0 };
    let group = get_s(g, shape, "aiAov");
    l.group = scene.group_index(&group);
}

/// Surfaces that glow are lights too: each instance with an emitting material gets a mesh light over the triangles of
/// that material, so they are sampled directly rather than found by chance.
pub fn emissive(scene: &mut Scene) {
    let emits = |m: &Material| match m {
        Material::Standard(s) => !matches!(s.emission, Input::Const(v) if v.x <= 0.0) && !matches!(s.emission_color, Input::Const(v) if v.max_element() <= 0.0),
        _ => false,
    };
    let mut add = Vec::new();
    for (ii, inst) in scene.geo.instances.iter().enumerate() {
        if inst.light.is_some() || scene.lights.iter().any(|l| matches!(l.shape, Shape::Mesh { inst: i, .. } if i as usize == ii)) {
            continue;
        }
        let mesh = &scene.geo.meshes[inst.mesh as usize];
        let glowing: Vec<usize> = inst.shaders.iter().enumerate().filter(|(_, &m)| scene.materials.get(m as usize).is_some_and(emits)).map(|(k, _)| k).collect();
        if glowing.is_empty() {
            continue;
        }
        let tris: Vec<u32> = (0..mesh.tris.len() as u32).filter(|&t| glowing.contains(&(mesh.tri_shader[t as usize] as usize))).collect();
        if tris.is_empty() {
            continue;
        }
        add.push(Light::new(&format!("{}_glow", inst.name), Shape::Mesh { inst: ii as u32, tris, cdf: Vec::new(), area: 0.0 }, Vec3::ONE));
    }
    scene.lights.extend(add);
}
