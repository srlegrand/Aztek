//! Shading networks into the renderer's: each texture and utility node upstream of a material becomes one network node,
//! shared by everything that reads it. Unconnected attributes become constants, read through the graph so keys and
//! expressions apply at the frame being rendered.

use std::collections::HashMap;

use aztek_render::shader::{Blend, Compare, Input, MathOp, Network, Node, NormalMod, Out, Place2d, Ramp, RampInterp, Sampler, SumOp};
use aztek_render::texture::{ColorSpace, TextureKey};
use glam::{Vec2, Vec3, Vec4};

use crate::graph::Graph;
use crate::plug::{NodeId, PlugRef, PlugType, PlugValue};

pub struct NetBuilder<'a> {
    pub g: &'a mut Graph,
    pub net: &'a mut Network,
    pub built: HashMap<NodeId, u32>,
    pub warnings: Vec<String>,
    pub scene_path: Option<String>,
    pub project: Option<String>,
    /// Textures off: file nodes give their default colour.
    pub ignore_textures: bool,
    pub textures: std::sync::Arc<aztek_render::texture::TextureCache>,
}

pub fn attr(g: &Graph, id: NodeId, name: &str) -> Option<PlugRef> {
    g.node(id)?.ty().attr_index(name).map(|i| PlugRef::new(id, i as i32))
}

pub fn get(g: &mut Graph, id: NodeId, name: &str) -> Option<PlugValue> {
    let p = attr(g, id, name)?;
    g.value(&p).ok()
}

pub fn get_f(g: &mut Graph, id: NodeId, name: &str, d: f64) -> f64 {
    get(g, id, name).and_then(|v| v.scalar()).unwrap_or(d)
}

pub fn get_b(g: &mut Graph, id: NodeId, name: &str, d: bool) -> bool {
    get(g, id, name).and_then(|v| v.scalar()).map_or(d, |x| x != 0.0)
}

pub fn get_v(g: &mut Graph, id: NodeId, name: &str, d: Vec3) -> Vec3 {
    match get(g, id, name) {
        Some(PlugValue::Vec3(v)) => v.as_vec3(),
        Some(v) => v.scalar().map_or(d, |x| Vec3::splat(x as f32)),
        None => d,
    }
}

pub fn get_s(g: &mut Graph, id: NodeId, name: &str) -> String {
    match get(g, id, name) {
        Some(PlugValue::String(s)) => s,
        _ => String::new(),
    }
}

/// Which output of a node a connection takes, from the source attribute.
fn out_of(g: &Graph, src: &PlugRef) -> Out {
    if src.component >= 0 {
        return [Out::R, Out::G, Out::B][src.component.clamp(0, 2) as usize];
    }
    let a = g.attr(src);
    let n = a.name.as_str();
    if n.ends_with("Alpha") || n == "outAlpha" {
        return Out::A;
    }
    match n {
        "outU" | "outValueX" | "outputX" | "outFloat" | "outValue" | "facingRatio" | "flippedNormal" => Out::R,
        "outV" | "outValueY" | "outputY" => Out::G,
        "outValueZ" | "outputZ" => Out::B,
        _ if a.ty != PlugType::Vec3 => Out::R,
        _ => Out::Rgb,
    }
}

impl NetBuilder<'_> {
    pub fn warn(&mut self, w: String) {
        if !self.warnings.contains(&w) {
            self.warnings.push(w);
        }
    }

    /// An attribute of a node as an input: its connection, or its value.
    pub fn input(&mut self, id: NodeId, name: &str) -> Input {
        let Some(p) = attr(self.g, id, name) else { return Input::c(0.0) };
        if let Some(src) = self.g.source(&p) {
            if let Some(i) = self.link(&src) {
                return i;
            }
        }
        let def = self.g.attr(&p).clone();
        if def.ty == PlugType::Vec3 {
            let kids: Vec<Option<PlugRef>> = (0..3).map(|k| self.g.source(&PlugRef::component(id, p.attr, k))).collect();
            if kids.iter().any(Option::is_some) {
                let val = get_v(self.g, id, name, Vec3::ZERO);
                let mut ch = [Input::c(0.0); 3];
                for k in 0..3 {
                    ch[k] = match &kids[k] {
                        Some(s) => self.link(s).unwrap_or(Input::c(val[k])),
                        None => Input::c(val[k]),
                    };
                }
                // The same scalar on all three: no need to combine.
                if ch[0] == ch[1] && ch[1] == ch[2] {
                    return ch[0];
                }
                let n = self.net.add(Node::Combine(ch[0], ch[1], ch[2]));
                return Input::Link(n, Out::Rgb);
            }
        }
        match self.g.value(&p) {
            Ok(PlugValue::Vec3(v)) => Input::Const(v.as_vec3()),
            Ok(v) => Input::c(v.scalar().unwrap_or(0.0) as f32),
            Err(_) => Input::c(0.0),
        }
    }

    /// A scalar attribute alone, as a plain number: for settings that can't take a texture.
    pub fn num(&mut self, id: NodeId, name: &str, d: f64) -> f32 {
        get_f(self.g, id, name, d) as f32
    }

    fn link(&mut self, src: &PlugRef) -> Option<Input> {
        // A value from outside shading (a control's attribute, an expression): its value at this frame.
        let ty = self.g.node(src.node)?.ty().name.clone();
        if !is_shading(&ty) {
            return Some(match self.g.value(src) {
                Ok(PlugValue::Vec3(v)) => Input::Const(v.as_vec3()),
                Ok(v) => Input::c(v.scalar().unwrap_or(0.0) as f32),
                Err(_) => Input::c(0.0),
            });
        }
        let n = self.node(src.node)?;
        Some(Input::Link(n, out_of(self.g, src)))
    }

    fn place(&mut self, id: NodeId) -> Place2d {
        let g = &mut *self.g;
        let two = |g: &mut Graph, a: &str, d: f32| {
            let v = get_v(g, id, a, Vec3::new(d, d, 0.0));
            Vec2::new(v.x, v.y)
        };
        Place2d {
            coverage: two(g, "coverage", 1.0),
            translate_frame: two(g, "translateFrame", 0.0),
            rotate_frame: get_f(g, id, "rotateFrame", 0.0).to_degrees() as f32,
            mirror: [get_b(g, id, "mirrorU", false), get_b(g, id, "mirrorV", false)],
            wrap: [get_b(g, id, "wrapU", true), get_b(g, id, "wrapV", true)],
            stagger: get_b(g, id, "stagger", false),
            repeat: two(g, "repeatUV", 1.0),
            offset: two(g, "offset", 0.0),
            rotate_uv: get_f(g, id, "rotateUV", 0.0).to_degrees() as f32,
            noise: two(g, "noiseUV", 0.0),
        }
    }

    /// A UV input that isn't the surface's own: a place2dTexture's is, so it's left to the node's own placement.
    fn uv_input(&mut self, id: NodeId) -> Option<Input> {
        let p = attr(self.g, id, "uvCoord")?;
        let src = self.g.source(&p)?;
        let ty = self.g.node(src.node)?.ty().name.clone();
        if ty == "place2dTexture" {
            return None;
        }
        self.link(&src)
    }

    fn ramp_of(&mut self, id: NodeId, pos: &str, val: &str, interp: Option<&str>, node_interp: RampInterp) -> Ramp {
        let g = &*self.g;
        let Some(n) = g.node(id) else { return Ramp::default() };
        let t = n.ty();
        let (Some(pa), Some(va)) = (t.attr_index(pos), t.attr_index(val)) else { return Ramp::default() };
        let ia = interp.and_then(|x| t.attr_index(x));
        let d = n.data();
        let keys = d
            .elements(pa)
            .iter()
            .map(|(k, p)| {
                let v = d.element_or(va, *k);
                let c = match v {
                    PlugValue::Vec3(v) => v.as_vec3(),
                    other => Vec3::splat(other.scalar().unwrap_or(0.0) as f32),
                };
                let it = ia.map_or(node_interp, |i| {
                    // remapValue's interpolations: None, Linear, Smooth, Spline.
                    match d.element_or(i, *k).scalar().unwrap_or(1.0) as i64 {
                        0 => RampInterp::None,
                        2 | 3 => RampInterp::Smooth,
                        _ => RampInterp::Linear,
                    }
                });
                (p.scalar().unwrap_or(0.0) as f32, c, it)
            })
            .collect();
        Ramp::new(keys)
    }

    fn texture_key(&mut self, id: NodeId, file_attr: &str) -> Option<TextureKey> {
        let written = get_s(self.g, id, file_attr);
        if written.is_empty() {
            return None;
        }
        let space = ColorSpace::parse(&get_s(self.g, id, "colorSpace"));
        let tiling = get_f(self.g, id, "uvTilingMode", 0.0) as i64;
        let mut pattern = written.clone();
        // UDIM by Mari's numbering: the file names one tile, the token stands for all.
        if tiling == 3 && !pattern.to_lowercase().contains("<udim>") {
            if let Some(i) = find_udim(&pattern) {
                pattern = format!("{}<UDIM>{}", &pattern[..i], &pattern[i + 4..]);
            }
        }
        let resolved = if pattern.to_lowercase().contains("<udim>") || pattern.to_lowercase().contains("<uvtile>") {
            // Resolve the folder through one tile that exists.
            let probe = written.clone();
            let r = crate::shading::resolve_texture(&probe, self.scene_path.as_deref(), self.project.as_deref());
            match r {
                Some(p) => {
                    let dir = p.parent().map(|d| d.display().to_string()).unwrap_or_default();
                    let file = std::path::Path::new(&pattern).file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or(pattern.clone());
                    format!("{dir}/{file}")
                }
                None => pattern.clone(),
            }
        } else {
            match crate::shading::resolve_texture(&written, self.scene_path.as_deref(), self.project.as_deref()) {
                Some(p) => p.display().to_string(),
                None => {
                    self.warn(format!("{}: texture {written} not found", self.g.node(id).map(|n| n.name().to_string()).unwrap_or_default()));
                    return None;
                }
            }
        };
        Some(TextureKey { path: resolved, space })
    }

    /// The network node for a scene node, made once.
    pub fn node(&mut self, id: NodeId) -> Option<u32> {
        if let Some(&n) = self.built.get(&id) {
            return Some(n);
        }
        let ty = self.g.node(id)?.ty().name.clone();
        let name = self.g.node(id)?.name().to_string();
        // Guard against cycles: a placeholder first.
        let slot = self.net.add(Node::Constant(Vec4::ZERO));
        self.built.insert(id, slot);
        let node = match ty.as_str() {
            "file" | "aiImage" => {
                let file_attr = if ty == "file" { "fileTextureName" } else { "filename" };
                let key = if self.ignore_textures { None } else { self.texture_key(id, file_attr) };
                let place = if ty == "file" { self.place(id) } else { Place2d { repeat: Vec2::new(self.num(id, "sscale", 1.0), self.num(id, "tscale", 1.0)), offset: Vec2::new(self.num(id, "soffset", 0.0), self.num(id, "toffset", 0.0)), ..Default::default() } };
                let uv = self.uv_input(id);
                let g = &mut *self.g;
                let (default, gain, offset) = if ty == "file" { (get_v(g, id, "defaultColor", Vec3::splat(0.5)), get_v(g, id, "colorGain", Vec3::ONE), get_v(g, id, "colorOffset", Vec3::ZERO)) } else { (get_v(g, id, "missingTextureColor", Vec3::ZERO), get_v(g, id, "multiply", Vec3::ONE), get_v(g, id, "offset", Vec3::ZERO)) };
                match key {
                    Some(key) => Node::File { tex: std::sync::Arc::new(aztek_render::texture::TexHandle::new(key, self.textures.clone())), place, uv, default, gain, offset, alpha_gain: get_f(g, id, "alphaGain", 1.0) as f32, alpha_offset: get_f(g, id, "alphaOffset", 0.0) as f32, invert: get_b(g, id, "invert", false), alpha_is_luminance: get_b(g, id, "alphaIsLuminance", false), exposure: get_f(g, id, "exposure", 0.0) as f32 },
                    None => Node::Constant(default.extend(1.0)),
                }
            }
            "place2dTexture" => {
                let place = self.place(id);
                Node::Place { place, uv: None }
            }
            "checker" => {
                let (c1, c2) = (self.input(id, "color1"), self.input(id, "color2"));
                let place = self.place_from_input(id);
                Node::Checker { c1, c2, contrast: self.num(id, "contrast", 1.0), place, uv: self.uv_input(id) }
            }
            "ramp" => {
                let interp = RampInterp::from_index(get_f(self.g, id, "interpolation", 1.0) as i64);
                let ramp = self.ramp_of(id, "colorEntryList.position", "colorEntryList.color", None, interp);
                let place = self.place_from_input(id);
                let g = &mut *self.g;
                Node::RampTex { ramp, kind: get_f(g, id, "type", 0.0) as i64, place, uv: None, u_wave: get_f(g, id, "uWave", 0.0) as f32, v_wave: get_f(g, id, "vWave", 0.0) as f32, noise: get_f(g, id, "noise", 0.0) as f32, noise_freq: get_f(g, id, "noiseFreq", 0.5) as f32 }
            }
            "noise" => {
                let place = self.place_from_input(id);
                let g = &mut *self.g;
                Node::Noise {
                    threshold: get_f(g, id, "threshold", 0.0) as f32,
                    amplitude: get_f(g, id, "amplitude", 1.0) as f32,
                    ratio: get_f(g, id, "ratio", 0.707) as f32,
                    freq_ratio: get_f(g, id, "frequencyRatio", 2.0) as f32,
                    depth: get_f(g, id, "depthMax", 3.0) as u32,
                    frequency: get_f(g, id, "frequency", 8.0) as f32,
                    inflection: get_b(g, id, "inflection", false),
                    time: get_f(g, id, "time", 0.0) as f32,
                    place,
                    uv: None,
                    gain: get_v(g, id, "colorGain", Vec3::ONE),
                    offset: get_v(g, id, "colorOffset", Vec3::ZERO),
                }
            }
            "layeredTexture" => {
                let mut layers = Vec::new();
                let elems: Vec<i32> = self.elements(id, "inputs.color");
                for k in elems {
                    let color = self.element_input(id, "inputs.color", k);
                    let alpha = self.element_input(id, "inputs.alpha", k);
                    let blend = Blend::from_index(self.element_value(id, "inputs.blendMode", k).unwrap_or(1.0) as i64);
                    let visible = self.element_value(id, "inputs.isVisible", k).unwrap_or(1.0) != 0.0;
                    layers.push((color, alpha, blend, visible));
                }
                Node::Layered { layers }
            }
            "multiplyDivide" => {
                let op = match get_f(self.g, id, "operation", 1.0) as i64 {
                    0 => MathOp::None,
                    2 => MathOp::Divide,
                    3 => MathOp::Power,
                    _ => MathOp::Multiply,
                };
                Node::MultiplyDivide { op, a: self.input(id, "input1"), b: self.input(id, "input2") }
            }
            "plusMinusAverage" => {
                let op = match get_f(self.g, id, "operation", 1.0) as i64 {
                    0 => SumOp::None,
                    2 => SumOp::Subtract,
                    3 => SumOp::Average,
                    _ => SumOp::Sum,
                };
                let mut inputs = Vec::new();
                for a in ["input3D", "input1D"] {
                    for k in self.elements(id, a) {
                        inputs.push(self.element_input(id, a, k));
                    }
                }
                Node::PlusMinus { op, inputs }
            }
            "blendColors" => Node::BlendColors { a: self.input(id, "color1"), b: self.input(id, "color2"), blender: self.input(id, "blender") },
            "remapValue" => {
                let value = self.ramp_of(id, "value.value_Position", "value.value_FloatValue", Some("value.value_Interp"), RampInterp::Linear);
                let color = self.ramp_of(id, "color.color_Position", "color.color_Color", Some("color.color_Interp"), RampInterp::Linear);
                let g = &mut *self.g;
                let (a, b, c, d) = (get_f(g, id, "inputMin", 0.0) as f32, get_f(g, id, "inputMax", 1.0) as f32, get_f(g, id, "outputMin", 0.0) as f32, get_f(g, id, "outputMax", 1.0) as f32);
                // Which output is read decides colour or value; both are asked for through the same node, so make
                // the colour version and let a scalar read take its first channel. A value read of a colour ramp would
                // be wrong, so value ramps win when there are no colour points.
                let colour_out = !color.keys.is_empty() && value.keys.is_empty();
                Node::Remap { input: self.input(id, "inputValue"), in_min: a, in_max: b, out_min: c, out_max: d, value, color, colour_out }
            }
            "reverse" => Node::Reverse(self.input(id, "input")),
            "clamp" => Node::Clamp { input: self.input(id, "input"), min: self.input(id, "min"), max: self.input(id, "max") },
            "luminance" => Node::Luminance(self.input(id, "value")),
            "gammaCorrect" => Node::Gamma { input: self.input(id, "value"), gamma: self.input(id, "gamma") },
            "setRange" => Node::SetRange { value: self.input(id, "value"), min: self.input(id, "min"), max: self.input(id, "max"), old_min: self.input(id, "oldMin"), old_max: self.input(id, "oldMax") },
            "condition" => {
                let op = match get_f(self.g, id, "operation", 0.0) as i64 {
                    1 => Compare::NotEqual,
                    2 => Compare::Greater,
                    3 => Compare::GreaterEqual,
                    4 => Compare::Less,
                    5 => Compare::LessEqual,
                    _ => Compare::Equal,
                };
                Node::Condition { a: self.input(id, "firstTerm"), b: self.input(id, "secondTerm"), op, if_true: self.input(id, "colorIfTrue"), if_false: self.input(id, "colorIfFalse") }
            }
            "contrast" => Node::Contrast { input: self.input(id, "value"), contrast: self.input(id, "contrast"), bias: self.input(id, "bias") },
            "hsvToRgb" => Node::HsvToRgb(self.input(id, "inHsv")),
            "rgbToHsv" => Node::RgbToHsv(self.input(id, "inRgb")),
            "samplerInfo" => Node::SamplerInfo(Sampler::FacingRatio),
            "aiAmbientOcclusion" => Node::Traced { kind: 0, white: self.input(id, "white"), black: self.input(id, "black"), samples: self.num(id, "samples", 3.0).max(1.0) as u32, distance: self.num(id, "farClip", 100.0) },
            "aiCurvature" => Node::Traced { kind: 1, white: Input::c(1.0), black: Input::c(0.0), samples: self.num(id, "samples", 3.0).max(1.0) as u32, distance: self.num(id, "radius", 0.1) },
            "aiUtility" | "aiFlat" | "surfaceShader" => Node::Constant(get_v(self.g, id, if ty == "surfaceShader" { "outColor" } else { "color" }, Vec3::ONE).extend(1.0)),
            "standardSurface" | "aiStandardSurface" | "lambert" | "blinn" | "phong" | "phongE" | "anisotropic" => {
                // A material read as a colour: its main colour.
                let a = if ty == "aiStandardSurface" { "base_color" } else if ty == "standardSurface" { "baseColor" } else { "color" };
                let i = self.input(id, a);
                self.net.nodes[slot as usize] = Node::Utility { mode: 0, color: i };
                return Some(slot);
            }
            other => {
                // Something Harold has no node for: its value at this frame, from the graph.
                self.warn(format!("{name}: {other} is read as its current value, not shaded per sample"));
                let p = attr(self.g, id, "outColor").or_else(|| attr(self.g, id, "output")).or_else(|| attr(self.g, id, "outValue"));
                let v = p.and_then(|p| self.g.value(&p).ok());
                match v {
                    Some(PlugValue::Vec3(v)) => Node::Constant(v.as_vec3().extend(1.0)),
                    Some(v) => Node::Constant(Vec4::splat(v.scalar().unwrap_or(0.0) as f32)),
                    None => Node::Constant(Vec4::new(0.5, 0.5, 0.5, 1.0)),
                }
            }
        };
        self.net.nodes[slot as usize] = node;
        Some(slot)
    }

    /// A procedural texture's placement, from the place2dTexture feeding its uvCoord.
    fn place_from_input(&mut self, id: NodeId) -> Place2d {
        let src = attr(self.g, id, "uvCoord").and_then(|p| self.g.source(&p));
        match src {
            Some(s) if self.g.node(s.node).is_some_and(|n| n.ty().name == "place2dTexture") => self.place(s.node),
            _ => Place2d::default(),
        }
    }

    fn elements(&self, id: NodeId, name: &str) -> Vec<i32> {
        let Some(n) = self.g.node(id) else { return Vec::new() };
        let Some(a) = n.ty().attr_index(name) else { return Vec::new() };
        let mut ks: Vec<i32> = n.data().elements(a).iter().map(|(k, _)| *k).collect();
        // Connected elements count even with no value of their own.
        for k in 0..64 {
            if self.g.source(&PlugRef::element(id, a as i32, k)).is_some() && !ks.contains(&k) {
                ks.push(k);
            }
        }
        ks.sort_unstable();
        ks
    }

    fn element_input(&mut self, id: NodeId, name: &str, k: i32) -> Input {
        let Some(a) = self.g.node(id).and_then(|n| n.ty().attr_index(name)) else { return Input::c(0.0) };
        let p = PlugRef::element(id, a as i32, k);
        if let Some(src) = self.g.source(&p) {
            if let Some(i) = self.link(&src) {
                return i;
            }
        }
        match self.g.value(&p) {
            Ok(PlugValue::Vec3(v)) => Input::Const(v.as_vec3()),
            Ok(v) => Input::c(v.scalar().unwrap_or(0.0) as f32),
            Err(_) => Input::c(0.0),
        }
    }

    fn element_value(&mut self, id: NodeId, name: &str, k: i32) -> Option<f64> {
        let a = self.g.node(id)?.ty().attr_index(name)?;
        self.g.value(&PlugRef::element(id, a as i32, k)).ok()?.scalar()
    }

    /// What a material's normalCamera (or another normal input) is fed by: a bump or a normal map.
    pub fn normal_mod(&mut self, id: NodeId, name: &str) -> NormalMod {
        let Some(src) = attr(self.g, id, name).and_then(|p| self.g.source(&p)) else { return NormalMod::None };
        let n = src.node;
        let ty = self.g.node(n).map(|x| x.ty().name.clone()).unwrap_or_default();
        match ty.as_str() {
            "bump2d" => {
                let interp = get_f(self.g, n, "bumpInterp", 0.0) as i64;
                let input = self.input(n, "bumpValue");
                if interp == 0 {
                    NormalMod::Bump { height: input, depth: self.num(n, "bumpDepth", 1.0) }
                } else {
                    // A normal map through bump2d: the colour plug feeding bumpValue, read as RGB.
                    let map = match input {
                        Input::Link(k, _) => Input::Link(k, Out::Rgb),
                        other => other,
                    };
                    NormalMod::Map { map, strength: self.num(n, "bumpDepth", 1.0), flip_r: false, flip_g: false, swap: false, tangent: interp == 1, signed: false }
                }
            }
            "aiBump2d" => NormalMod::Bump { height: self.input(n, "bumpMap"), depth: self.num(n, "bumpHeight", 1.0) },
            "aiNormalMap" => {
                let map = self.input(n, "input");
                let g = &mut *self.g;
                NormalMod::Map { map, strength: get_f(g, n, "strength", 1.0) as f32, flip_r: get_b(g, n, "invertX", false), flip_g: get_b(g, n, "invertY", false), swap: get_b(g, n, "order", false), tangent: get_b(g, n, "tangentSpace", true), signed: !get_b(g, n, "colorToSigned", true) }
            }
            other => {
                self.warn(format!("{}: normal from {other} not honoured", self.g.node(id).map(|x| x.name().to_string()).unwrap_or_default()));
                NormalMod::None
            }
        }
    }
}

/// Node types translated into the network; any other source is read as its value.
fn is_shading(ty: &str) -> bool {
    matches!(
        ty,
        "file" | "aiImage" | "place2dTexture" | "checker" | "ramp" | "noise" | "layeredTexture" | "multiplyDivide" | "plusMinusAverage" | "blendColors" | "remapValue" | "reverse" | "clamp" | "luminance" | "gammaCorrect" | "setRange" | "condition" | "contrast" | "hsvToRgb" | "rgbToHsv" | "samplerInfo" | "aiAmbientOcclusion" | "aiCurvature" | "aiUtility" | "aiFlat" | "surfaceShader" | "standardSurface" | "aiStandardSurface" | "lambert" | "blinn" | "phong" | "phongE" | "anisotropic" | "projection" | "fractal" | "cloth" | "grid" | "bulge" | "stucco" | "leather" | "marble" | "wood" | "granite" | "rock" | "crater" | "solidFractal" | "volumeNoise" | "mountain" | "water" | "envSphere" | "envBall"
    )
}

/// Where a run of four digits between 1001 and 1999 sits in a file name: Mari's UDIM number.
fn find_udim(path: &str) -> Option<usize> {
    let name_start = path.rfind(['/', '\\']).map_or(0, |i| i + 1);
    let b = path.as_bytes();
    let mut i = path.len().saturating_sub(4);
    while i >= name_start && i + 4 <= path.len() {
        let s = &path[i..i + 4];
        let before = i == 0 || !b[i - 1].is_ascii_digit();
        let after = i + 4 >= b.len() || !b[i + 4].is_ascii_digit();
        if before && after && s.chars().all(|c| c.is_ascii_digit()) && s.starts_with('1') {
            return Some(i);
        }
        if i == 0 {
            break;
        }
        i -= 1;
    }
    None
}

#[cfg(test)]
mod tests {
    #[test]
    fn udim_number_found() {
        assert_eq!(super::find_udim("/a/b/skin.1001.png"), Some(10));
        assert_eq!(super::find_udim("/a/2020/skin_col.png"), None);
    }
}
