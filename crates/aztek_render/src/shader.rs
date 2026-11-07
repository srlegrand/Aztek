//! Shading networks as the renderer runs them: a flat list of nodes, each input a constant or another node's output,
//! evaluated per shading sample. Materials are the standard surface and a few that stand beside it (flat, mix, shadow
//! matte); every other material a scene holds is translated to the standard surface before it reaches here.

use glam::{Vec2, Vec3, Vec4};

use crate::texture::TextureCache;

/// Which part of a node's output an input takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Out {
    Rgb,
    R,
    G,
    B,
    A,
    /// Mean of the three colour channels.
    Mean,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Input {
    Const(Vec3),
    Link(u32, Out),
}

impl Input {
    pub fn c(v: f32) -> Input {
        Input::Const(Vec3::splat(v))
    }
    pub fn rgb(r: f32, g: f32, b: f32) -> Input {
        Input::Const(Vec3::new(r, g, b))
    }
    pub fn is_const(&self) -> bool {
        matches!(self, Input::Const(_))
    }
}

impl Default for Input {
    fn default() -> Self {
        Input::Const(Vec3::ZERO)
    }
}

/// What a shading sample knows of where it is.
#[derive(Clone, Copy, Debug)]
pub struct ShadeCtx {
    pub p: Vec3,
    /// Point in object space, for 3D textures.
    pub p_obj: Vec3,
    pub n: Vec3,
    pub ng: Vec3,
    pub uv: Vec2,
    pub dpdu: Vec3,
    pub dpdv: Vec3,
    /// Footprint in UV units, for texture level.
    pub footprint: f32,
    /// Towards the viewer.
    pub wo: Vec3,
    pub front: bool,
    pub object_id: u32,
}

impl Default for ShadeCtx {
    fn default() -> Self {
        ShadeCtx { p: Vec3::ZERO, p_obj: Vec3::ZERO, n: Vec3::Z, ng: Vec3::Z, uv: Vec2::ZERO, dpdu: Vec3::X, dpdv: Vec3::Y, footprint: 0.0, wo: Vec3::Z, front: true, object_id: 0 }
    }
}

/// Maya's place2dTexture: frame then tiling.
#[derive(Clone, Copy, Debug)]
pub struct Place2d {
    pub coverage: Vec2,
    pub translate_frame: Vec2,
    pub rotate_frame: f32,
    pub mirror: [bool; 2],
    pub wrap: [bool; 2],
    pub stagger: bool,
    pub repeat: Vec2,
    pub offset: Vec2,
    pub rotate_uv: f32,
    pub noise: Vec2,
}

impl Default for Place2d {
    fn default() -> Self {
        Place2d { coverage: Vec2::ONE, translate_frame: Vec2::ZERO, rotate_frame: 0.0, mirror: [false; 2], wrap: [true; 2], stagger: false, repeat: Vec2::ONE, offset: Vec2::ZERO, rotate_uv: 0.0, noise: Vec2::ZERO }
    }
}

fn rotate_about_half(uv: Vec2, deg: f32) -> Vec2 {
    if deg == 0.0 {
        return uv;
    }
    let (s, c) = (-deg.to_radians()).sin_cos();
    let d = uv - Vec2::splat(0.5);
    Vec2::new(d.x * c - d.y * s, d.x * s + d.y * c) + Vec2::splat(0.5)
}

impl Place2d {
    /// The texture's UV for a surface UV, None outside the covered frame. `tiles` keeps the integer part (UDIMs).
    pub fn apply(&self, uv: Vec2, tiles: bool) -> Option<Vec2> {
        let mut q = rotate_about_half(uv - self.translate_frame, self.rotate_frame);
        if self.coverage != Vec2::ONE {
            let cov = self.coverage.max(Vec2::splat(1e-6));
            for k in 0..2 {
                let mut x = q[k];
                if self.wrap[k] {
                    x -= x.floor();
                }
                if !(0.0..cov[k]).contains(&x) && cov[k] < 1.0 {
                    return None;
                }
                q[k] = x / cov[k];
            }
        }
        q = q * self.repeat + self.offset;
        if self.stagger && (q.y.floor() as i64) % 2 != 0 {
            q.x += 0.5;
        }
        q = rotate_about_half(q - q.floor(), self.rotate_uv) + q.floor();
        for k in 0..2 {
            if self.mirror[k] && (q[k].floor() as i64).rem_euclid(2) == 1 {
                q[k] = q[k].floor() + 1.0 - (q[k] - q[k].floor());
            }
            if !self.wrap[k] && !(0.0..1.0).contains(&q[k]) {
                return None;
            }
            if !tiles {
                q[k] -= q[k].floor();
            }
        }
        Some(q)
    }
    pub fn scale(&self) -> f32 {
        (self.repeat / self.coverage.max(Vec2::splat(1e-6))).abs().max_element()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RampInterp {
    None,
    Linear,
    ExpUp,
    ExpDown,
    Smooth,
    Bump,
    Spike,
}

impl RampInterp {
    pub fn from_index(i: i64) -> RampInterp {
        match i {
            0 => RampInterp::None,
            2 => RampInterp::ExpUp,
            3 => RampInterp::ExpDown,
            4 => RampInterp::Smooth,
            5 => RampInterp::Bump,
            6 => RampInterp::Spike,
            _ => RampInterp::Linear,
        }
    }
}

/// Sorted keys of a ramp, each with its own interpolation to the next.
#[derive(Clone, Debug, Default)]
pub struct Ramp {
    pub keys: Vec<(f32, Vec3, RampInterp)>,
}

impl Ramp {
    pub fn new(mut keys: Vec<(f32, Vec3, RampInterp)>) -> Ramp {
        keys.sort_by(|a, b| a.0.total_cmp(&b.0));
        Ramp { keys }
    }
    pub fn at(&self, t: f32) -> Vec3 {
        let k = &self.keys;
        if k.is_empty() {
            return Vec3::splat(t.clamp(0.0, 1.0));
        }
        if t <= k[0].0 {
            return k[0].1;
        }
        for w in k.windows(2) {
            let (a, b) = (&w[0], &w[1]);
            if t <= b.0 {
                let span = (b.0 - a.0).max(1e-9);
                let f = (t - a.0) / span;
                let g = match a.2 {
                    RampInterp::None => 0.0,
                    RampInterp::Linear => f,
                    RampInterp::ExpUp => f * f,
                    RampInterp::ExpDown => 1.0 - (1.0 - f) * (1.0 - f),
                    RampInterp::Smooth => f * f * (3.0 - 2.0 * f),
                    RampInterp::Bump => {
                        return a.1.lerp(b.1, f) * (1.0 - 0.5 * (f * std::f32::consts::PI).sin()).max(0.0);
                    }
                    RampInterp::Spike => {
                        return a.1.lerp(b.1, f) * (1.0 + 0.5 * (f * std::f32::consts::PI).sin());
                    }
                };
                return a.1.lerp(b.1, g);
            }
        }
        k[k.len() - 1].1
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MathOp {
    None,
    Multiply,
    Divide,
    Power,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SumOp {
    None,
    Sum,
    Subtract,
    Average,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Compare {
    Equal,
    NotEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
}

/// samplerInfo's outputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sampler {
    FacingRatio,
    Uv,
    PointWorld,
    PointObject,
    NormalCamera,
    FlippedNormal,
}

/// A layered texture's blend modes, Maya's numbering.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Blend {
    None,
    Over,
    In,
    Out,
    Add,
    Subtract,
    Multiply,
    Difference,
    Lighten,
    Darken,
    Saturate,
    Desaturate,
    Illuminate,
}

impl Blend {
    pub fn from_index(i: i64) -> Blend {
        use Blend::*;
        [None, Over, In, Out, Add, Subtract, Multiply, Difference, Lighten, Darken, Saturate, Desaturate, Illuminate].get(i as usize).copied().unwrap_or(Over)
    }
}

#[derive(Clone, Debug)]
pub enum Node {
    Constant(Vec4),
    /// An image: Maya's file node and aiImage.
    File { tex: std::sync::Arc<crate::texture::TexHandle>, place: Place2d, uv: Option<Input>, default: Vec3, gain: Vec3, offset: Vec3, alpha_gain: f32, alpha_offset: f32, invert: bool, alpha_is_luminance: bool, exposure: f32 },
    /// A place2dTexture alone: its UV in x and y.
    Place { place: Place2d, uv: Option<Input> },
    Checker { c1: Input, c2: Input, contrast: f32, place: Place2d, uv: Option<Input> },
    RampTex { ramp: Ramp, kind: i64, place: Place2d, uv: Option<Input>, u_wave: f32, v_wave: f32, noise: f32, noise_freq: f32 },
    Noise { threshold: f32, amplitude: f32, ratio: f32, freq_ratio: f32, depth: u32, frequency: f32, inflection: bool, time: f32, place: Place2d, uv: Option<Input>, gain: Vec3, offset: Vec3 },
    Layered { layers: Vec<(Input, Input, Blend, bool)> },
    MultiplyDivide { op: MathOp, a: Input, b: Input },
    PlusMinus { op: SumOp, inputs: Vec<Input> },
    BlendColors { a: Input, b: Input, blender: Input },
    Remap { input: Input, in_min: f32, in_max: f32, out_min: f32, out_max: f32, value: Ramp, color: Ramp, colour_out: bool },
    Reverse(Input),
    Clamp { input: Input, min: Input, max: Input },
    Luminance(Input),
    Gamma { input: Input, gamma: Input },
    SetRange { value: Input, min: Input, max: Input, old_min: Input, old_max: Input },
    Condition { a: Input, b: Input, op: Compare, if_true: Input, if_false: Input },
    Contrast { input: Input, contrast: Input, bias: Input },
    HsvToRgb(Input),
    RgbToHsv(Input),
    SamplerInfo(Sampler),
    /// aiUtility and the like: a constant colour, or a debug view.
    Utility { mode: i64, color: Input },
    /// Three scalars into a colour: channels connected one by one.
    Combine(Input, Input, Input),
    /// Ambient occlusion and curvature need rays: their result is filled by the integrator, here a stand-in.
    Traced { kind: i64, white: Input, black: Input, samples: u32, distance: f32 },
}

pub fn perlin(p: Vec3) -> f32 {
    // Gradient noise on a hashed lattice, quintic fade; range about -1 to 1.
    let pi = p.floor();
    let f = p - pi;
    let fade = |t: f32| t * t * t * (t * (t * 6.0 - 15.0) + 10.0);
    let (u, v, w) = (fade(f.x), fade(f.y), fade(f.z));
    let grad = |ix: i32, iy: i32, iz: i32, d: Vec3| {
        let h = crate::math::hash3(ix as u32, iy as u32, iz as u32);
        let g = match h & 15 {
            0 => Vec3::new(1.0, 1.0, 0.0),
            1 => Vec3::new(-1.0, 1.0, 0.0),
            2 => Vec3::new(1.0, -1.0, 0.0),
            3 => Vec3::new(-1.0, -1.0, 0.0),
            4 => Vec3::new(1.0, 0.0, 1.0),
            5 => Vec3::new(-1.0, 0.0, 1.0),
            6 => Vec3::new(1.0, 0.0, -1.0),
            7 => Vec3::new(-1.0, 0.0, -1.0),
            8 => Vec3::new(0.0, 1.0, 1.0),
            9 => Vec3::new(0.0, -1.0, 1.0),
            10 => Vec3::new(0.0, 1.0, -1.0),
            11 => Vec3::new(0.0, -1.0, -1.0),
            12 => Vec3::new(1.0, 1.0, 0.0),
            13 => Vec3::new(-1.0, 1.0, 0.0),
            14 => Vec3::new(0.0, -1.0, 1.0),
            _ => Vec3::new(0.0, -1.0, -1.0),
        };
        g.dot(d)
    };
    let (x, y, z) = (pi.x as i32, pi.y as i32, pi.z as i32);
    let mut acc = [0.0f32; 8];
    for (k, a) in acc.iter_mut().enumerate() {
        let (dx, dy, dz) = ((k & 1) as i32, ((k >> 1) & 1) as i32, ((k >> 2) & 1) as i32);
        *a = grad(x + dx, y + dy, z + dz, f - Vec3::new(dx as f32, dy as f32, dz as f32));
    }
    let l = |a: f32, b: f32, t: f32| a + (b - a) * t;
    let x0 = l(l(acc[0], acc[1], u), l(acc[2], acc[3], u), v);
    let x1 = l(l(acc[4], acc[5], u), l(acc[6], acc[7], u), v);
    l(x0, x1, w)
}

fn hsv_to_rgb(c: Vec3) -> Vec3 {
    let h = (c.x / 60.0).rem_euclid(6.0);
    let (s, v) = (c.y, c.z);
    let i = h.floor();
    let f = h - i;
    let (p, q, t) = (v * (1.0 - s), v * (1.0 - s * f), v * (1.0 - s * (1.0 - f)));
    match i as i32 {
        0 => Vec3::new(v, t, p),
        1 => Vec3::new(q, v, p),
        2 => Vec3::new(p, v, t),
        3 => Vec3::new(p, q, v),
        4 => Vec3::new(t, p, v),
        _ => Vec3::new(v, p, q),
    }
}

fn rgb_to_hsv(c: Vec3) -> Vec3 {
    let mx = c.max_element();
    let mn = c.min_element();
    let d = mx - mn;
    let h = if d <= 0.0 {
        0.0
    } else if mx == c.x {
        60.0 * ((c.y - c.z) / d).rem_euclid(6.0)
    } else if mx == c.y {
        60.0 * ((c.z - c.x) / d + 2.0)
    } else {
        60.0 * ((c.x - c.y) / d + 4.0)
    };
    Vec3::new(h, if mx > 0.0 { d / mx } else { 0.0 }, mx)
}

/// The nodes of every network in a scene, and the images they read.
#[derive(Default)]
pub struct Network {
    pub nodes: Vec<Node>,
}

impl Network {
    pub fn add(&mut self, n: Node) -> u32 {
        self.nodes.push(n);
        (self.nodes.len() - 1) as u32
    }

    pub fn eval(&self, i: Input, ctx: &ShadeCtx, tex: &TextureCache) -> Vec3 {
        match i {
            Input::Const(v) => v,
            Input::Link(n, out) => {
                let v = self.eval_node(n, ctx, tex, 0);
                pick(v, out)
            }
        }
    }

    pub fn eval_f(&self, i: Input, ctx: &ShadeCtx, tex: &TextureCache) -> f32 {
        match i {
            Input::Const(v) => v.x,
            Input::Link(..) => self.eval(i, ctx, tex).x,
        }
    }

    fn sub(&self, i: Input, ctx: &ShadeCtx, tex: &TextureCache, depth: u32) -> Vec3 {
        match i {
            Input::Const(v) => v,
            Input::Link(n, out) => pick(self.eval_node(n, ctx, tex, depth + 1), out),
        }
    }

    fn sub4(&self, i: Input, ctx: &ShadeCtx, tex: &TextureCache, depth: u32) -> Vec4 {
        match i {
            Input::Const(v) => v.extend(1.0),
            Input::Link(n, Out::Rgb) => self.eval_node(n, ctx, tex, depth + 1),
            Input::Link(n, out) => Vec3::splat(pick(self.eval_node(n, ctx, tex, depth + 1), out).x).extend(1.0),
        }
    }

    fn uv_of(&self, uv: &Option<Input>, ctx: &ShadeCtx, tex: &TextureCache, depth: u32) -> Vec2 {
        match uv {
            Some(i) => self.sub(*i, ctx, tex, depth).truncate(),
            None => ctx.uv,
        }
    }

    /// A node's output: colour in xyz, alpha in w.
    pub fn eval_node(&self, n: u32, ctx: &ShadeCtx, tex: &TextureCache, depth: u32) -> Vec4 {
        if depth > 64 {
            return Vec4::ZERO;
        }
        let Some(node) = self.nodes.get(n as usize) else { return Vec4::ZERO };
        let s = |i: Input| self.sub(i, ctx, tex, depth);
        let f = |i: Input| self.sub(i, ctx, tex, depth).x;
        match node {
            Node::Constant(v) => *v,
            Node::Place { place, uv } => {
                let q = place.apply(self.uv_of(uv, ctx, tex, depth), false).unwrap_or(Vec2::ZERO);
                Vec4::new(q.x, q.y, 0.0, 1.0)
            }
            Node::File { tex: handle, place, uv, default, gain, offset, alpha_gain, alpha_offset, invert, alpha_is_luminance, exposure } => {
                let found = place.apply(self.uv_of(uv, ctx, tex, depth), handle.tiled()).and_then(|q| handle.sample(q, ctx.footprint * place.scale()));
                let Some(mut c) = found else { return Vec4::new(default.x, default.y, default.z, 0.0) };
                if *exposure != 0.0 {
                    let e = 2f32.powf(*exposure);
                    c = Vec4::new(c.x * e, c.y * e, c.z * e, c.w);
                }
                if *alpha_is_luminance {
                    c.w = crate::math::luminance(c.truncate());
                }
                if *invert {
                    c = Vec4::new(1.0 - c.x, 1.0 - c.y, 1.0 - c.z, 1.0 - c.w);
                }
                let rgb = c.truncate() * *gain + *offset;
                rgb.extend(c.w * alpha_gain + alpha_offset)
            }
            Node::Checker { c1, c2, contrast, place, uv } => {
                let Some(q) = place.apply(self.uv_of(uv, ctx, tex, depth), false) else { return Vec4::ZERO };
                // Four squares along each side of the tile.
                let k = ((q.x * 4.0).floor() as i64 + (q.y * 4.0).floor() as i64).rem_euclid(2);
                let (a, b) = (s(*c1), s(*c2));
                let mid = (a + b) * 0.5;
                let (a, b) = (mid + (a - mid) * *contrast, mid + (b - mid) * *contrast);
                (if k == 0 { a } else { b }).extend(1.0)
            }
            Node::RampTex { ramp, kind, place, uv, u_wave, v_wave, noise, noise_freq } => {
                let Some(mut q) = place.apply(self.uv_of(uv, ctx, tex, depth), false) else { return Vec4::ZERO };
                if *u_wave != 0.0 {
                    q.x += u_wave * (q.y * std::f32::consts::TAU).sin();
                }
                if *v_wave != 0.0 {
                    q.y += v_wave * (q.x * std::f32::consts::TAU).sin();
                }
                if *noise != 0.0 {
                    let nz = perlin(Vec3::new(q.x * noise_freq * 10.0, q.y * noise_freq * 10.0, 0.5));
                    q += Vec2::splat(nz * noise);
                }
                let d = q - Vec2::splat(0.5);
                let t = match kind {
                    1 => q.x,
                    2 => (q.x + q.y) * 0.5,
                    3 => (d.y.atan2(d.x) / std::f32::consts::TAU).rem_euclid(1.0),
                    4 => d.length() * 2.0,
                    5 => d.x.abs().max(d.y.abs()) * 2.0,
                    6 => q.x * q.y,
                    _ => q.y,
                };
                let c = ramp.at(t);
                c.extend(crate::math::luminance(c))
            }
            Node::Noise { threshold, amplitude, ratio, freq_ratio, depth: octaves, frequency, inflection, time, place, uv, gain, offset } => {
                let Some(q) = place.apply(self.uv_of(uv, ctx, tex, depth), false) else { return Vec4::ZERO };
                let (mut sum, mut amp, mut fr) = (0.0, 1.0, *frequency);
                for _ in 0..(*octaves).clamp(1, 12) {
                    let mut v = perlin(Vec3::new(q.x * fr, q.y * fr, *time));
                    if *inflection {
                        v = v.abs();
                    }
                    sum += v * amp;
                    amp *= ratio;
                    fr *= freq_ratio;
                }
                let v = (threshold + amplitude * (if *inflection { sum } else { sum * 0.5 + 0.5 })).clamp(0.0, 1.0);
                (Vec3::splat(v) * *gain + *offset).extend(v)
            }
            Node::Layered { layers } => {
                // Layer 0 is on top: composite from the bottom up.
                let mut acc = Vec3::ZERO;
                let mut acc_a = 0.0f32;
                for (color, alpha, blend, visible) in layers.iter().rev() {
                    if !visible {
                        continue;
                    }
                    let c4 = self.sub4(*color, ctx, tex, depth);
                    let c = c4.truncate();
                    let a = f(*alpha).clamp(0.0, 1.0);
                    acc = match blend {
                        Blend::None => c,
                        Blend::Over => acc.lerp(c, a),
                        Blend::In => acc * a,
                        Blend::Out => acc * (1.0 - a),
                        Blend::Add => acc + c * a,
                        Blend::Subtract => acc - c * a,
                        Blend::Multiply => acc.lerp(acc * c, a),
                        Blend::Difference => acc.lerp((acc - c).abs(), a),
                        Blend::Lighten => acc.lerp(acc.max(c), a),
                        Blend::Darken => acc.lerp(acc.min(c), a),
                        Blend::Saturate => acc * (Vec3::ONE + c * a),
                        Blend::Desaturate => acc * (Vec3::ONE - c * a),
                        Blend::Illuminate => acc * (Vec3::ONE + 2.0 * c * a),
                    };
                    acc_a = acc_a + a * (1.0 - acc_a);
                }
                acc.extend(acc_a)
            }
            Node::MultiplyDivide { op, a, b } => {
                let (x, y) = (s(*a), s(*b));
                match op {
                    MathOp::None => x,
                    MathOp::Multiply => x * y,
                    MathOp::Divide => Vec3::new(if y.x != 0.0 { x.x / y.x } else { 0.0 }, if y.y != 0.0 { x.y / y.y } else { 0.0 }, if y.z != 0.0 { x.z / y.z } else { 0.0 }),
                    MathOp::Power => Vec3::new(x.x.powf(y.x), x.y.powf(y.y), x.z.powf(y.z)),
                }
                .extend(1.0)
            }
            Node::PlusMinus { op, inputs } => {
                let vals: Vec<Vec3> = inputs.iter().map(|i| s(*i)).collect();
                let r = match op {
                    SumOp::None => vals.first().copied().unwrap_or(Vec3::ZERO),
                    SumOp::Sum => vals.iter().sum(),
                    SumOp::Subtract => vals.iter().skip(1).fold(vals.first().copied().unwrap_or(Vec3::ZERO), |a, b| a - *b),
                    SumOp::Average => vals.iter().sum::<Vec3>() / vals.len().max(1) as f32,
                };
                r.extend(1.0)
            }
            Node::BlendColors { a, b, blender } => {
                let t = f(*blender);
                (s(*a) * t + s(*b) * (1.0 - t)).extend(1.0)
            }
            Node::Remap { input, in_min, in_max, out_min, out_max, value, color, colour_out } => {
                let x = f(*input);
                let t = if in_max != in_min { (x - in_min) / (in_max - in_min) } else { 0.0 };
                if *colour_out {
                    (Vec3::splat(*out_min) + color.at(t) * (out_max - out_min)).extend(1.0)
                } else {
                    Vec3::splat(out_min + value.at(t).x * (out_max - out_min)).extend(1.0)
                }
            }
            Node::Reverse(i) => (Vec3::ONE - s(*i)).extend(1.0),
            Node::Clamp { input, min, max } => s(*input).clamp(s(*min), s(*max).max(s(*min))).extend(1.0),
            Node::Luminance(i) => Vec3::splat(s(*i).dot(Vec3::new(0.3, 0.59, 0.11))).extend(1.0),
            Node::Gamma { input, gamma } => {
                let (x, g) = (s(*input), s(*gamma));
                let p = |a: f32, b: f32| if b > 0.0 { a.max(0.0).powf(1.0 / b) } else { 0.0 };
                Vec3::new(p(x.x, g.x), p(x.y, g.y), p(x.z, g.z)).extend(1.0)
            }
            Node::SetRange { value, min, max, old_min, old_max } => {
                let (v, a, b, oa, ob) = (s(*value), s(*min), s(*max), s(*old_min), s(*old_max));
                let ch = |v: f32, a: f32, b: f32, oa: f32, ob: f32| if ob != oa { a + ((v - oa) / (ob - oa)).clamp(0.0, 1.0) * (b - a) } else { a };
                Vec3::new(ch(v.x, a.x, b.x, oa.x, ob.x), ch(v.y, a.y, b.y, oa.y, ob.y), ch(v.z, a.z, b.z, oa.z, ob.z)).extend(1.0)
            }
            Node::Condition { a, b, op, if_true, if_false } => {
                let (x, y) = (f(*a), f(*b));
                let r = match op {
                    Compare::Equal => x == y,
                    Compare::NotEqual => x != y,
                    Compare::Greater => x > y,
                    Compare::GreaterEqual => x >= y,
                    Compare::Less => x < y,
                    Compare::LessEqual => x <= y,
                };
                s(if r { *if_true } else { *if_false }).extend(1.0)
            }
            Node::Contrast { input, contrast, bias } => {
                let (v, c, b) = (s(*input), s(*contrast), s(*bias));
                let ch = |v: f32, c: f32, b: f32| {
                    let b = b.clamp(1e-4, 1.0 - 1e-4);
                    let x = v.max(0.0).powf((0.5f32).ln() / b.ln());
                    let y = if x < 0.5 { 0.5 * (2.0 * x).powf(c) } else { 1.0 - 0.5 * (2.0 - 2.0 * x).powf(c) };
                    y
                };
                Vec3::new(ch(v.x, c.x, b.x), ch(v.y, c.y, b.y), ch(v.z, c.z, b.z)).extend(1.0)
            }
            Node::HsvToRgb(i) => hsv_to_rgb(s(*i)).extend(1.0),
            Node::RgbToHsv(i) => rgb_to_hsv(s(*i)).extend(1.0),
            Node::SamplerInfo(k) => match k {
                Sampler::FacingRatio => Vec3::splat(ctx.n.dot(ctx.wo).abs()).extend(1.0),
                Sampler::Uv => Vec4::new(ctx.uv.x, ctx.uv.y, 0.0, 1.0),
                Sampler::PointWorld => ctx.p.extend(1.0),
                Sampler::PointObject => ctx.p_obj.extend(1.0),
                Sampler::NormalCamera => ctx.n.extend(1.0),
                Sampler::FlippedNormal => Vec3::splat(if ctx.front { 0.0 } else { 1.0 }).extend(1.0),
            },
            Node::Utility { mode, color } => match mode {
                1 => (ctx.n * 0.5 + Vec3::splat(0.5)).extend(1.0),
                2 => Vec4::new(ctx.uv.x.fract(), ctx.uv.y.fract(), 0.0, 1.0),
                3 => {
                    let h = crate::math::hash_u32(ctx.object_id.wrapping_add(17));
                    Vec4::new((h & 255) as f32 / 255.0, ((h >> 8) & 255) as f32 / 255.0, ((h >> 16) & 255) as f32 / 255.0, 1.0)
                }
                _ => s(*color).extend(1.0),
            },
            Node::Traced { white, .. } => s(*white).extend(1.0),
            Node::Combine(a, b, c) => Vec3::new(f(*a), f(*b), f(*c)).extend(1.0),
        }
    }

    /// Whether an input reaches a Traced node, which needs the integrator.
    pub fn traced(&self, i: Input) -> Option<u32> {
        let Input::Link(n, _) = i else { return None };
        matches!(self.nodes.get(n as usize), Some(Node::Traced { .. })).then_some(n)
    }
}

fn pick(v: Vec4, out: Out) -> Vec3 {
    match out {
        Out::Rgb => v.truncate(),
        Out::R => Vec3::splat(v.x),
        Out::G => Vec3::splat(v.y),
        Out::B => Vec3::splat(v.z),
        Out::A => Vec3::splat(v.w),
        Out::Mean => Vec3::splat((v.x + v.y + v.z) / 3.0),
    }
}

/// How a material bends its shading normal.
#[derive(Clone, Copy, Debug, Default)]
pub enum NormalMod {
    #[default]
    None,
    /// A height, scaled, by finite differences in UV.
    Bump { height: Input, depth: f32 },
    /// A tangent space normal map (0 to 1 encoded), or object space when `tangent` is false.
    Map { map: Input, strength: f32, flip_r: bool, flip_g: bool, swap: bool, tangent: bool, signed: bool },
}

/// The standard surface's parameters, as its published specification names them.
#[derive(Clone, Debug)]
pub struct Standard {
    pub base: Input,
    pub base_color: Input,
    pub diffuse_roughness: Input,
    pub metalness: Input,
    pub specular: Input,
    pub specular_color: Input,
    pub specular_roughness: Input,
    pub specular_ior: Input,
    pub specular_anisotropy: Input,
    pub specular_rotation: Input,
    pub transmission: Input,
    pub transmission_color: Input,
    pub transmission_depth: Input,
    pub transmission_scatter: Input,
    pub transmission_scatter_anisotropy: Input,
    pub transmission_extra_roughness: Input,
    pub subsurface: Input,
    pub subsurface_color: Input,
    pub subsurface_radius: Input,
    pub subsurface_scale: Input,
    pub subsurface_anisotropy: Input,
    pub sheen: Input,
    pub sheen_color: Input,
    pub sheen_roughness: Input,
    pub coat: Input,
    pub coat_color: Input,
    pub coat_roughness: Input,
    pub coat_anisotropy: Input,
    pub coat_rotation: Input,
    pub coat_ior: Input,
    pub coat_affect_color: Input,
    pub coat_affect_roughness: Input,
    pub coat_normal: NormalMod,
    pub thin_film_thickness: Input,
    pub thin_film_ior: Input,
    pub emission: Input,
    pub emission_color: Input,
    pub opacity: Input,
    pub thin_walled: bool,
    pub normal: NormalMod,
    /// Arnold's: caustics off cuts specular paths after a diffuse bounce.
    pub caustics: bool,
    pub internal_reflections: bool,
    pub exit_to_background: bool,
    pub indirect_diffuse: f32,
    pub indirect_specular: f32,
}

impl Default for Standard {
    /// The specification's defaults.
    fn default() -> Self {
        let c = Input::c;
        Standard {
            base: c(0.8),
            base_color: c(1.0),
            diffuse_roughness: c(0.0),
            metalness: c(0.0),
            specular: c(1.0),
            specular_color: c(1.0),
            specular_roughness: c(0.2),
            specular_ior: c(1.5),
            specular_anisotropy: c(0.0),
            specular_rotation: c(0.0),
            transmission: c(0.0),
            transmission_color: c(1.0),
            transmission_depth: c(0.0),
            transmission_scatter: c(0.0),
            transmission_scatter_anisotropy: c(0.0),
            transmission_extra_roughness: c(0.0),
            subsurface: c(0.0),
            subsurface_color: c(1.0),
            subsurface_radius: c(1.0),
            subsurface_scale: c(1.0),
            subsurface_anisotropy: c(0.0),
            sheen: c(0.0),
            sheen_color: c(1.0),
            sheen_roughness: c(0.3),
            coat: c(0.0),
            coat_color: c(1.0),
            coat_roughness: c(0.1),
            coat_anisotropy: c(0.0),
            coat_rotation: c(0.0),
            coat_ior: c(1.5),
            coat_affect_color: c(0.0),
            coat_affect_roughness: c(0.0),
            coat_normal: NormalMod::None,
            thin_film_thickness: c(0.0),
            thin_film_ior: c(1.5),
            emission: c(0.0),
            emission_color: c(1.0),
            opacity: c(1.0),
            thin_walled: false,
            normal: NormalMod::None,
            caustics: false,
            internal_reflections: true,
            exit_to_background: false,
            indirect_diffuse: 1.0,
            indirect_specular: 1.0,
        }
    }
}

/// Everything resolved at one shading point.
#[derive(Clone, Copy, Debug, Default)]
pub struct Params {
    pub base: f32,
    pub base_color: Vec3,
    pub diffuse_roughness: f32,
    pub metalness: f32,
    pub specular: f32,
    pub specular_color: Vec3,
    pub specular_roughness: f32,
    pub specular_ior: f32,
    pub specular_anisotropy: f32,
    pub specular_rotation: f32,
    pub transmission: f32,
    pub transmission_color: Vec3,
    pub transmission_depth: f32,
    pub transmission_scatter: Vec3,
    pub transmission_scatter_anisotropy: f32,
    pub transmission_extra_roughness: f32,
    pub subsurface: f32,
    pub subsurface_color: Vec3,
    pub subsurface_radius: Vec3,
    pub subsurface_scale: f32,
    pub subsurface_anisotropy: f32,
    pub sheen: f32,
    pub sheen_color: Vec3,
    pub sheen_roughness: f32,
    pub coat: f32,
    pub coat_color: Vec3,
    pub coat_roughness: f32,
    pub coat_anisotropy: f32,
    pub coat_rotation: f32,
    pub coat_ior: f32,
    pub coat_affect_color: f32,
    pub coat_affect_roughness: f32,
    pub thin_film_thickness: f32,
    pub thin_film_ior: f32,
    pub emission: Vec3,
    pub opacity: Vec3,
    pub thin_walled: bool,
    /// The shading normal after bump or normal map, and the coat's.
    pub n: Vec3,
    pub coat_n: Vec3,
    pub tangent: Vec3,
}

#[derive(Clone, Debug)]
pub enum Material {
    Standard(Box<Standard>),
    /// Unlit colour: surfaceShader, aiFlat.
    Flat { color: Input, opacity: Input },
    /// Two materials blended (aiMixShader, layeredShader's pairs): by `mix`, or added.
    Mix { a: u32, b: u32, mix: Input, add: bool },
    /// Invisible but for the shadows it catches.
    ShadowMatte { shadow_color: Input, background: bool },
}

impl Default for Material {
    fn default() -> Self {
        Material::Standard(Box::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ramp_interpolates() {
        let r = Ramp::new(vec![(1.0, Vec3::ONE, RampInterp::Linear), (0.0, Vec3::ZERO, RampInterp::Linear)]);
        assert!((r.at(0.25).x - 0.25).abs() < 1e-6);
        assert_eq!(r.at(2.0), Vec3::ONE);
    }

    #[test]
    fn place_repeats_and_wraps() {
        let p = Place2d { repeat: Vec2::splat(2.0), ..Default::default() };
        let q = p.apply(Vec2::new(0.75, 0.25), false).unwrap();
        assert!((q - Vec2::new(0.5, 0.5)).length() < 1e-5, "{q}");
        let p = Place2d { coverage: Vec2::splat(0.5), wrap: [false; 2], ..Default::default() };
        assert!(p.apply(Vec2::new(0.75, 0.25), false).is_none());
    }

    #[test]
    fn utility_nodes() {
        let mut net = Network::default();
        let tex = TextureCache::new(0);
        let a = net.add(Node::Constant(Vec4::new(0.2, 0.4, 0.6, 1.0)));
        let m = net.add(Node::MultiplyDivide { op: MathOp::Multiply, a: Input::Link(a, Out::Rgb), b: Input::c(2.0) });
        let r = net.add(Node::Reverse(Input::Link(m, Out::Rgb)));
        let v = net.eval(Input::Link(r, Out::Rgb), &ShadeCtx::default(), &tex);
        assert!((v - Vec3::new(0.6, 0.2, -0.2)).length() < 1e-5);
        let h = net.add(Node::RgbToHsv(Input::rgb(1.0, 0.0, 0.0)));
        let back = net.add(Node::HsvToRgb(Input::Link(h, Out::Rgb)));
        assert!((net.eval(Input::Link(back, Out::Rgb), &ShadeCtx::default(), &tex) - Vec3::X).length() < 1e-5);
    }
}
