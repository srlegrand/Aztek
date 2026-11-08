//! Materials into Harold's: the standard surface as it is, and every older material a scene can hold translated to it,
//! with a warning naming what isn't honoured. The scene keeps the original nodes; this only reads them.

use std::collections::HashMap;

use aztek_render::shader::{Input, Material, Node, Out, Standard};
use glam::Vec3;

use super::network::{get_b, get_f, get_v, NetBuilder};
use crate::harold_nodes::STANDARD;
use crate::plug::NodeId;

pub struct MatBuilder<'a, 'b> {
    pub nb: &'b mut NetBuilder<'a>,
    pub materials: &'b mut Vec<Material>,
    pub names: &'b mut Vec<String>,
    pub built: HashMap<NodeId, u32>,
}

fn reverse(nb: &mut NetBuilder, i: Input) -> Input {
    match i {
        Input::Const(v) => Input::Const(Vec3::ONE - v),
        _ => Input::Link(nb.net.add(Node::Reverse(i)), Out::Rgb),
    }
}

fn mean(_nb: &mut NetBuilder, i: Input) -> Input {
    match i {
        Input::Const(v) => Input::c((v.x + v.y + v.z) / 3.0),
        Input::Link(n, Out::Rgb) => Input::Link(n, Out::Mean),
        other => other,
    }
}

fn scale(nb: &mut NetBuilder, i: Input, k: f32) -> Input {
    match i {
        Input::Const(v) => Input::Const(v * k),
        _ => Input::Link(nb.net.add(Node::MultiplyDivide { op: aztek_render::shader::MathOp::Multiply, a: i, b: Input::c(k) }), Out::Rgb),
    }
}

impl MatBuilder<'_, '_> {
    fn push(&mut self, name: &str, m: Material) -> u32 {
        self.materials.push(m);
        self.names.push(name.into());
        (self.materials.len() - 1) as u32
    }

    /// The material a material node becomes, made once.
    pub fn material(&mut self, id: NodeId) -> u32 {
        if let Some(&m) = self.built.get(&id) {
            return m;
        }
        let (ty, name) = {
            let n = self.nb.g.node(id);
            (n.map(|n| n.ty().name.clone()).unwrap_or_default(), n.map(|n| n.name().to_string()).unwrap_or_default())
        };
        // A placeholder against cycles through mixes.
        let slot = self.push(&name, Material::default());
        self.built.insert(id, slot);
        let m = self.translate(id, &ty, &name);
        self.materials[slot as usize] = m;
        slot
    }

    fn standard(&mut self, id: NodeId, snake: bool) -> Standard {
        let nb = &mut *self.nb;
        let mut s = Standard::default();
        let mut set = |field: &str, v: Input| match field {
            "base" => s.base = v,
            "baseColor" => s.base_color = v,
            "diffuseRoughness" => s.diffuse_roughness = v,
            "metalness" => s.metalness = v,
            "specular" => s.specular = v,
            "specularColor" => s.specular_color = v,
            "specularRoughness" => s.specular_roughness = v,
            "specularIOR" => s.specular_ior = v,
            "specularAnisotropy" => s.specular_anisotropy = v,
            "specularRotation" => s.specular_rotation = v,
            "transmission" => s.transmission = v,
            "transmissionColor" => s.transmission_color = v,
            "transmissionDepth" => s.transmission_depth = v,
            "transmissionScatter" => s.transmission_scatter = v,
            "transmissionScatterAnisotropy" => s.transmission_scatter_anisotropy = v,
            "transmissionExtraRoughness" => s.transmission_extra_roughness = v,
            "subsurface" => s.subsurface = v,
            "subsurfaceColor" => s.subsurface_color = v,
            "subsurfaceRadius" => s.subsurface_radius = v,
            "subsurfaceScale" => s.subsurface_scale = v,
            "subsurfaceAnisotropy" => s.subsurface_anisotropy = v,
            "sheen" => s.sheen = v,
            "sheenColor" => s.sheen_color = v,
            "sheenRoughness" => s.sheen_roughness = v,
            "coat" => s.coat = v,
            "coatColor" => s.coat_color = v,
            "coatRoughness" => s.coat_roughness = v,
            "coatAnisotropy" => s.coat_anisotropy = v,
            "coatRotation" => s.coat_rotation = v,
            "coatIOR" => s.coat_ior = v,
            "coatAffectColor" => s.coat_affect_color = v,
            "coatAffectRoughness" => s.coat_affect_roughness = v,
            "thinFilmThickness" => s.thin_film_thickness = v,
            "thinFilmIOR" => s.thin_film_ior = v,
            "emission" => s.emission = v,
            "emissionColor" => s.emission_color = v,
            "opacity" => s.opacity = v,
            _ => {}
        };
        for &(camel, _, sn, _, _) in STANDARD {
            let v = nb.input(id, if snake { sn } else { camel });
            set(camel, v);
        }
        s.thin_walled = get_b(nb.g, id, if snake { "thin_walled" } else { "thinWalled" }, false);
        s.normal = nb.normal_mod(id, "normalCamera");
        s.coat_normal = nb.normal_mod(id, if snake { "coat_normal" } else { "coatNormal" });
        if snake {
            s.caustics = get_b(nb.g, id, "caustics", false);
            s.internal_reflections = get_b(nb.g, id, "internal_reflections", true);
            s.indirect_diffuse = get_f(nb.g, id, "indirect_diffuse", 1.0) as f32;
            s.indirect_specular = get_f(nb.g, id, "indirect_specular", 1.0) as f32;
        }
        if get_f(nb.g, id, if snake { "transmission_dispersion" } else { "transmissionDispersion" }, 0.0) > 0.0 {
            let n = nb.g.node(id).map(|n| n.name().to_string()).unwrap_or_default();
            nb.warn(format!("{n}: transmission dispersion not honoured"));
        }
        s
    }

    /// Maya's lambert family into the standard surface.
    fn legacy(&mut self, id: NodeId, ty: &str, name: &str) -> Material {
        let nb = &mut *self.nb;
        let mut s = Standard::default();
        let mut skipped: Vec<&str> = Vec::new();
        let diffuse = nb.input(id, "diffuse");
        s.base = diffuse;
        s.base_color = nb.input(id, "color");
        s.normal = nb.normal_mod(id, "normalCamera");
        // Transparency: see-through by opacity, or bent by refraction when refractions are on.
        let transparency = nb.input(id, "transparency");
        let has_transparency = !matches!(transparency, Input::Const(v) if v.max_element() <= 0.0);
        if has_transparency {
            if get_b(nb.g, id, "refractions", false) {
                s.transmission = mean(nb, transparency);
                s.transmission_color = nb.input(id, "color");
                s.specular_ior = Input::c(nb.num(id, "refractiveIndex", 1.0).max(1.0));
            } else {
                s.opacity = reverse(nb, transparency);
            }
        }
        let inc = nb.input(id, "incandescence");
        if !matches!(inc, Input::Const(v) if v.max_element() <= 0.0) {
            s.emission = Input::c(1.0);
            s.emission_color = inc;
        }
        // Ambient colour: the surface's colour times it, given to itself, as the viewport adds it.
        if get_v(nb.g, id, "ambientColor", Vec3::ZERO).max_element() > 0.0 || !nb.input(id, "ambientColor").is_const() {
            let (color, amb) = (nb.input(id, "color"), nb.input(id, "ambientColor"));
            s.ambient = match (color, amb) {
                (Input::Const(a), Input::Const(b)) => Input::Const(a * b),
                _ => Input::Link(nb.net.add(Node::MultiplyDivide { op: aztek_render::shader::MathOp::Multiply, a: color, b: amb }), Out::Rgb),
            };
        }
        for (a, label) in [("translucence", "translucence"), ("glowIntensity", "glow")] {
            let v = get_v(nb.g, id, a, Vec3::ZERO);
            if v.max_element() > 0.0 {
                skipped.push(label);
            }
        }
        match ty {
            "lambert" => s.specular = Input::c(0.0),
            "blinn" | "phong" | "phongE" | "anisotropic" | "rampShader" => {
                // The highlight's colour at Maya's default of 0.5 reads as the standard surface's full specular.
                let sc = nb.input(id, "specularColor");
                s.specular_color = scale(nb, sc, 2.0);
                s.specular = Input::c(1.0);
                let rough = match ty {
                    "blinn" => nb.num(id, "eccentricity", 0.3).max(0.0).sqrt() * 0.8,
                    "phong" => {
                        // Exponent n to GGX alpha: alpha^2 = 2 / (n + 2); roughness = sqrt(alpha).
                        let n = nb.num(id, "cosinePower", 20.0).max(1.0);
                        (2.0 / (n + 2.0)).sqrt().sqrt()
                    }
                    "phongE" => nb.num(id, "roughness", 0.5) * 0.8,
                    "anisotropic" => {
                        let (sx, sy) = (nb.num(id, "spreadX", 13.0).max(0.1), nb.num(id, "spreadY", 3.0).max(0.1));
                        s.specular_anisotropy = Input::c(((sx - sy).abs() / (sx + sy)).min(1.0));
                        s.specular_rotation = Input::c(nb.num(id, "angle", 0.0) / 360.0 + if sy > sx { 0.25 } else { 0.0 });
                        nb.num(id, "roughness", 0.7) * 0.6
                    }
                    _ => {
                        skipped.push("colour ramps");
                        nb.num(id, "eccentricity", 0.3).max(0.0).sqrt() * 0.8
                    }
                };
                s.specular_roughness = Input::c(rough.clamp(0.0, 1.0));
                let refl = nb.num(id, "reflectivity", 0.5);
                if refl > 0.0 && get_v(nb.g, id, "reflectedColor", Vec3::ZERO).max_element() > 0.0 {
                    skipped.push("reflected colour");
                }
            }
            _ => {}
        }
        if !skipped.is_empty() {
            nb.warn(format!("{name} ({ty}): {} not honoured", skipped.join(", ")));
        }
        Material::Standard(Box::new(s))
    }

    /// The material connected into a plug, as a shader input of a mix or layer.
    fn upstream_material(&mut self, id: NodeId, attr: &str) -> Option<u32> {
        let p = super::network::attr(self.nb.g, id, attr)?;
        let src = self.nb.g.source(&p)?;
        let t = self.nb.g.node(src.node)?.type_arc().clone();
        crate::shading::is_material_type(&t).then(|| self.material(src.node))
    }

    fn translate(&mut self, id: NodeId, ty: &str, name: &str) -> Material {
        match ty {
            "standardSurface" => Material::Standard(Box::new(self.standard(id, false))),
            "aiStandardSurface" => Material::Standard(Box::new(self.standard(id, true))),
            "lambert" | "blinn" | "phong" | "phongE" | "anisotropic" | "rampShader" => self.legacy(id, ty, name),
            "surfaceShader" => {
                let c = self.nb.input(id, "outColor");
                let t = self.nb.input(id, "outTransparency");
                Material::Flat { color: c, opacity: reverse(self.nb, t) }
            }
            "aiFlat" => Material::Flat { color: self.nb.input(id, "color"), opacity: Input::c(1.0) },
            "aiUtility" => {
                let mode = match get_f(self.nb.g, id, "colorMode", 0.0) as i64 {
                    1..=3 => 1,
                    5 => 2,
                    14 | 20 => 3,
                    _ => 0,
                };
                let color = self.nb.input(id, "color");
                let n = self.nb.net.add(Node::Utility { mode, color });
                Material::Flat { color: Input::Link(n, Out::Rgb), opacity: Input::c(1.0) }
            }
            "aiAmbientOcclusion" => {
                let n = self.nb.node(id).unwrap_or(0);
                Material::Flat { color: Input::Link(n, Out::Rgb), opacity: Input::c(1.0) }
            }
            "aiShadowMatte" => Material::ShadowMatte { shadow_color: self.nb.input(id, "shadowColor"), background: get_b(self.nb.g, id, "background", false) },
            "aiMixShader" => {
                let a = self.upstream_material(id, "shader1").unwrap_or(0);
                let b = self.upstream_material(id, "shader2").unwrap_or(0);
                let add = get_f(self.nb.g, id, "mode", 0.0) as i64 == 1;
                Material::Mix { a, b, mix: self.nb.input(id, "mix"), add }
            }
            "layeredShader" => {
                // Layer 0 on top, each seen through the transparency of those above.
                let elems: Vec<i32> = {
                    let g = &*self.nb.g;
                    let a = g.node(id).and_then(|n| n.ty().attr_index("inputs.color"));
                    let mut ks: Vec<i32> = (0..32).filter(|&k| a.is_some_and(|a| g.source(&crate::plug::PlugRef::element(id, a as i32, k)).is_some())).collect();
                    ks.sort_unstable();
                    ks
                };
                let mut below: Option<u32> = None;
                for &k in elems.iter().rev() {
                    let a = self.nb.g.node(id).and_then(|n| n.ty().attr_index("inputs.color")).unwrap() as i32;
                    let src = self.nb.g.source(&crate::plug::PlugRef::element(id, a, k)).unwrap();
                    let t = self.nb.g.node(src.node).map(|n| n.type_arc().clone());
                    let m = if t.is_some_and(|t| crate::shading::is_material_type(&t)) { self.material(src.node) } else { 0 };
                    let ta = self.nb.g.node(id).and_then(|n| n.ty().attr_index("inputs.transparency")).unwrap() as i32;
                    let tp = crate::plug::PlugRef::element(id, ta, k);
                    let transp = match self.nb.g.value(&tp) {
                        Ok(crate::plug::PlugValue::Vec3(v)) => (v.x + v.y + v.z) as f32 / 3.0,
                        _ => 0.0,
                    };
                    below = Some(match below {
                        None => m,
                        Some(b) => self.push(&format!("{name}_layer{k}"), Material::Mix { a: b, b: m, mix: Input::c(1.0 - transp), add: false }),
                    });
                }
                match below {
                    Some(m) => self.materials[m as usize].clone(),
                    None => Material::default(),
                }
            }
            "mia_material" | "mia_material_x" | "mia_material_x_passes" => {
                let nb = &mut *self.nb;
                let mut s = Standard::default();
                s.base = nb.input(id, "diffuse_weight");
                s.base_color = nb.input(id, "diffuse");
                s.diffuse_roughness = nb.input(id, "diffuse_roughness");
                s.specular = nb.input(id, "reflectivity");
                s.specular_color = nb.input(id, "refl_color");
                s.specular_roughness = Input::c((1.0 - nb.num(id, "refl_gloss", 1.0)).clamp(0.0, 1.0));
                if get_b(nb.g, id, "refl_is_metal", false) {
                    s.metalness = nb.input(id, "reflectivity");
                }
                s.transmission = nb.input(id, "transparency");
                s.transmission_color = nb.input(id, "refr_color");
                s.specular_ior = Input::c(nb.num(id, "refr_ior", 1.4).max(1.0));
                s.transmission_extra_roughness = Input::c((1.0 - nb.num(id, "refr_gloss", 1.0)).clamp(0.0, 1.0));
                s.opacity = nb.input(id, "cutout_opacity");
                s.emission = Input::c(1.0);
                s.emission_color = nb.input(id, "additional_color");
                s.thin_walled = get_b(nb.g, id, "thin_walled", false);
                let aniso = nb.num(id, "anisotropy", 1.0);
                if aniso != 1.0 {
                    s.specular_anisotropy = Input::c((1.0 - 1.0 / aniso.max(1e-3)).abs().min(1.0));
                }
                let mut skipped = vec!["custom reflection curve (brdf)"];
                if nb.num(id, "translucency", 0.0) > 0.0 {
                    skipped.push("translucency");
                }
                nb.warn(format!("{name} ({ty}): {} not honoured", skipped.join(", ")));
                Material::Standard(Box::new(s))
            }
            other => {
                self.nb.warn(format!("{name}: material type {other} rendered as the default material"));
                Material::default()
            }
        }
    }
}
