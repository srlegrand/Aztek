//! Procedural textures: the reference's 2D patterns over UV (bulge, cloth, fractal, grid, mountain, water) and its 3D
//! ones through space (brownian, cloud, crater, granite, leather, marble, rock, snow, solidFractal, stucco,
//! volumeNoise, wood). Each takes the attributes the reference's node has, under the same names, and does what its
//! documentation says of them. The reference's own noise is not published, so a pattern here has the same character
//! and answers the same controls without matching it point for point.

use glam::{Mat4, Vec2, Vec3, Vec4};

use crate::math::hash3;
use crate::shader::perlin;

/// What every texture applies to its result: the reference's colour balance attributes.
#[derive(Clone, Copy, Debug)]
pub struct Balance {
    pub default_color: Vec3,
    pub color_gain: Vec3,
    pub color_offset: Vec3,
    pub alpha_gain: f32,
    pub alpha_offset: f32,
    pub alpha_is_luminance: bool,
    pub invert: bool,
}

impl Default for Balance {
    fn default() -> Self {
        Balance { default_color: Vec3::splat(0.5), color_gain: Vec3::ONE, color_offset: Vec3::ZERO, alpha_gain: 1.0, alpha_offset: 0.0, alpha_is_luminance: false, invert: false }
    }
}

impl Balance {
    pub fn apply(&self, mut c: Vec4) -> Vec4 {
        if self.invert {
            c = Vec4::new(1.0 - c.x, 1.0 - c.y, 1.0 - c.z, 1.0 - c.w);
        }
        if self.alpha_is_luminance {
            c.w = crate::math::luminance(c.truncate());
        }
        (c.truncate() * self.color_gain + self.color_offset).extend(c.w * self.alpha_gain + self.alpha_offset)
    }
}

fn rand3(ix: i32, iy: i32, iz: i32, salt: u32) -> f32 {
    (hash3(ix as u32 ^ salt.wrapping_mul(0x9e37_79b9), iy as u32, iz as u32) & 0xffff) as f32 / 65535.0
}

fn smooth(a: f32, b: f32, x: f32) -> f32 {
    if (b - a).abs() < 1e-9 {
        return if x < a { 0.0 } else { 1.0 };
    }
    let t = ((x - a) / (b - a)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Octaves of noise from `lo` to `hi` (fractional ends fade in), each `ratio` as strong and `lacunarity` as fine as
/// the one before. About -1 to 1; with `inflection` each octave is folded, which gives creases.
fn fractal(p: Vec3, lo: f32, hi: f32, ratio: f32, lacunarity: f32, inflection: bool) -> f32 {
    let (mut sum, mut amp, mut freq, mut power) = (0.0, 1.0, 1.0, 0.0);
    let top = hi.max(lo).min(24.0);
    let mut k = 0.0;
    while k < top {
        let weight = (top - k).min(1.0) * if k + 1.0 > lo { (k + 1.0 - lo).min(1.0) } else { 0.0 };
        if weight > 0.0 {
            let n = perlin(p * freq + Vec3::splat(k * 17.3));
            sum += weight * amp * if inflection { n.abs() * 2.0 - 1.0 } else { n };
            power += (weight * amp) * (weight * amp);
        }
        amp *= ratio;
        freq *= lacunarity;
        k += 1.0;
    }
    // More octaves add detail, not range: the sum keeps the spread one octave has.
    if power > 0.0 {
        sum / power.sqrt()
    } else {
        0.0
    }
}

/// The nearest and second nearest feature points of a jittered lattice: distances, and the nearest cell's number.
fn cells(p: Vec3, randomness: f32) -> (f32, f32, u32) {
    let base = p.floor();
    let (mut f1, mut f2, mut id) = (f32::MAX, f32::MAX, 0u32);
    for dz in -1..=1 {
        for dy in -1..=1 {
            for dx in -1..=1 {
                let c = base + Vec3::new(dx as f32, dy as f32, dz as f32);
                let (ix, iy, iz) = (c.x as i32, c.y as i32, c.z as i32);
                let jitter = Vec3::new(rand3(ix, iy, iz, 1), rand3(ix, iy, iz, 2), rand3(ix, iy, iz, 3)) - Vec3::splat(0.5);
                let d = (c + Vec3::splat(0.5) + jitter * randomness.clamp(0.0, 1.0) - p).length();
                if d < f1 {
                    f2 = f1;
                    f1 = d;
                    id = hash3(ix as u32, iy as u32, iz as u32);
                } else if d < f2 {
                    f2 = d;
                }
            }
        }
    }
    (f1, f2, id)
}

/// A 2D texture, by its node type.
#[derive(Clone, Debug)]
pub enum Tex2d {
    Bulge { u_width: f32, v_width: f32 },
    Cloth { gap: Vec3, u_color: Vec3, v_color: Vec3, u_width: f32, v_width: f32, u_wave: f32, v_wave: f32, randomness: f32, width_spread: f32, bright_spread: f32 },
    Fractal { amplitude: f32, threshold: f32, ratio: f32, frequency_ratio: f32, level_min: f32, level_max: f32, bias: f32, inflection: bool, time: f32 },
    Grid { line: Vec3, filler: Vec3, u_width: f32, v_width: f32, contrast: f32 },
    Mountain { snow: Vec3, rock: Vec3, amplitude: f32, snow_roughness: f32, rock_roughness: f32, boundary: f32, snow_altitude: f32, snow_dropoff: f32, snow_slope: f32, depth_max: f32 },
    Water { waves: u32, wave_time: f32, wave_velocity: f32, wave_amplitude: f32, wave_frequency: f32, sub_wave_frequency: f32, smoothness: f32, wind: Vec2, ripple_time: f32, ripple_frequency: f32, ripple_amplitude: f32, drop_size: f32, ripple_origin: Vec2, group_velocity: f32, phase_velocity: f32, spread_start: f32, spread_rate: f32 },
}

fn mountain_height(q: Vec2, roughness: f32, depth: f32) -> f32 {
    0.5 + 0.5 * fractal(Vec3::new(q.x * 4.0, q.y * 4.0, 3.7), 0.0, depth.clamp(1.0, 20.0), roughness.clamp(0.0, 1.0), 2.0, false)
}

impl Tex2d {
    /// The colour and alpha at a placed UV.
    pub fn eval(&self, q: Vec2) -> Vec4 {
        match self {
            Tex2d::Bulge { u_width, v_width } => {
                // Level along a border of the widths, rising to the middle of the tile.
                let side = |t: f32, w: f32| {
                    let t = t.rem_euclid(1.0);
                    let w = w.clamp(0.0, 0.5);
                    let d = t.min(1.0 - t);
                    if d <= w {
                        0.0
                    } else {
                        (((d - w) / (0.5 - w).max(1e-6)) * std::f32::consts::FRAC_PI_2).sin()
                    }
                };
                let h = side(q.x, *u_width) * side(q.y, *v_width);
                Vec4::new(h, h, h, h)
            }
            Tex2d::Cloth { gap, u_color, v_color, u_width, v_width, u_wave, v_wave, randomness, width_spread, bright_spread } => {
                let mut q = q * 4.0;
                if *randomness > 0.0 {
                    q += Vec2::new(perlin(Vec3::new(q.x * 3.0, q.y * 3.0, 1.3)), perlin(Vec3::new(q.x * 3.0, q.y * 3.0, 7.9))) * *randomness * 0.25;
                }
                let (cu, cv) = (q.x.floor(), q.y.floor());
                let (fu, fv) = (q.x - cu, q.y - cv);
                // A thread along U fills a band across V of its width, and the other way round. Each thread has its
                // own narrowing and darkening.
                let r = |k: f32, salt: u32| rand3(k as i32, salt as i32, 0, salt);
                let uw = (*u_width * (1.0 - *width_spread * r(cv, 11))).clamp(0.0, 1.0);
                let vw = (*v_width * (1.0 - *width_spread * r(cu, 12))).clamp(0.0, 1.0);
                let in_u = (fv - 0.5).abs() < uw * 0.5;
                let in_v = (fu - 0.5).abs() < vw * 0.5;
                let u_over = (cu as i64 + cv as i64).rem_euclid(2) == 0;
                let wave = |across: f32, along: f32, amount: f32, over: bool| {
                    // Round across its width, and bending over or under along its length.
                    let round = (1.0 - (across * 2.0).powi(2)).max(0.0).sqrt();
                    let bend = 0.5 + 0.5 * (along * std::f32::consts::TAU).cos() * if over { 1.0 } else { -1.0 };
                    1.0 - amount.clamp(0.0, 1.0) * (1.0 - round * (0.5 + 0.5 * bend))
                };
                let u_shade = wave((fv - 0.5) / uw.max(1e-6), fu, *u_wave, u_over) * (1.0 - *bright_spread * r(cv, 13));
                let v_shade = wave((fu - 0.5) / vw.max(1e-6), fv, *v_wave, !u_over) * (1.0 - *bright_spread * r(cu, 14));
                let c = match (in_u, in_v) {
                    (true, true) => {
                        if u_over {
                            *u_color * u_shade
                        } else {
                            *v_color * v_shade
                        }
                    }
                    (true, false) => *u_color * u_shade,
                    (false, true) => *v_color * v_shade,
                    (false, false) => *gap,
                };
                c.extend(if in_u || in_v { 1.0 } else { 0.0 })
            }
            Tex2d::Fractal { amplitude, threshold, ratio, frequency_ratio, level_min, level_max, bias, inflection, time } => {
                let n = fractal(Vec3::new(q.x * 2.0, q.y * 2.0, *time), *level_min, *level_max, *ratio, *frequency_ratio, *inflection);
                let mut v = 0.5 + 0.5 * n * *amplitude;
                if *bias != 0.0 {
                    // Bias pushes the middle of the range up or down.
                    let b = (0.5 + 0.5 * bias.clamp(-1.0, 1.0)).clamp(1e-3, 1.0 - 1e-3);
                    v = v.clamp(0.0, 1.0).powf(b.ln() / 0.5f32.ln());
                }
                let v = (v + *threshold).clamp(0.0, 1.0);
                Vec4::new(v, v, v, v)
            }
            Tex2d::Grid { line, filler, u_width, v_width, contrast } => {
                let (fu, fv) = ((q.x * 4.0).rem_euclid(1.0), (q.y * 4.0).rem_euclid(1.0));
                let on = fu.min(1.0 - fu) < *u_width * 0.5 || fv.min(1.0 - fv) < *v_width * 0.5;
                let mid = (*line + *filler) * 0.5;
                let c = if on { mid + (*line - mid) * *contrast } else { mid + (*filler - mid) * *contrast };
                c.extend(if on { 1.0 } else { 0.0 })
            }
            Tex2d::Mountain { snow, rock, amplitude, snow_roughness, rock_roughness, boundary, snow_altitude, snow_dropoff, snow_slope, depth_max } => {
                let h = mountain_height(q, *rock_roughness, *depth_max) * *amplitude;
                let e = 1.0 / 256.0;
                let slope = Vec2::new(mountain_height(q + Vec2::new(e, 0.0), *rock_roughness, *depth_max) - mountain_height(q, *rock_roughness, *depth_max), mountain_height(q + Vec2::new(0.0, e), *rock_roughness, *depth_max) - mountain_height(q, *rock_roughness, *depth_max)).length() / e * 0.04;
                // Snow lies above its altitude where the ground is not too steep; its line is broken up by the boundary.
                let line = *snow_altitude + (perlin(Vec3::new(q.x * 24.0, q.y * 24.0, 9.1)) * 0.1 * *boundary) * snow_roughness.max(0.0);
                let above = smooth(line, line + 1.0 / snow_dropoff.max(1e-3) * 0.25, h);
                let flat = 1.0 - smooth(*snow_slope * 0.5, *snow_slope, slope);
                rock.lerp(*snow, above * flat).extend(h.clamp(0.0, 1.0))
            }
            Tex2d::Water { waves, wave_time, wave_velocity, wave_amplitude, wave_frequency, sub_wave_frequency, smoothness, wind, ripple_time, ripple_frequency, ripple_amplitude, drop_size, ripple_origin, group_velocity, phase_velocity, spread_start, spread_rate } => {
                let mut h = 0.0;
                let dir0 = if wind.length_squared() > 0.0 { wind.normalize() } else { Vec2::X };
                for k in 0..(*waves).min(64) {
                    // Each wave a little off the wind and finer than the last, the finer ones weaker as smoothness asks.
                    let turn = (rand3(k as i32, 0, 0, 21) - 0.5) * 1.2;
                    let dir = Vec2::new(dir0.x * turn.cos() - dir0.y * turn.sin(), dir0.x * turn.sin() + dir0.y * turn.cos());
                    let f = *wave_frequency * (1.0 + k as f32 * *sub_wave_frequency * 8.0);
                    let phase = rand3(k as i32, 1, 0, 22) * std::f32::consts::TAU;
                    let amp = *wave_amplitude / (1.0 + k as f32).powf(smoothness.max(0.0) * 0.5);
                    h += amp * ((q.dot(dir) * f - *wave_time * *wave_velocity) * std::f32::consts::TAU + phase).sin();
                }
                if *ripple_time > 0.0 && *ripple_amplitude != 0.0 {
                    // Rings out from where a drop fell: a group of them travelling at the group velocity, each crest
                    // at the phase velocity, the group widening as it goes.
                    let d = (q - *ripple_origin).length();
                    let centre = *ripple_time * *group_velocity;
                    let width = (*spread_start + *spread_rate * *ripple_time).max(1e-4) + *drop_size * 0.5;
                    let envelope = (-((d - centre) / width).powi(2)).exp();
                    h += *ripple_amplitude * envelope * ((d - *ripple_time * *phase_velocity) * *ripple_frequency * std::f32::consts::TAU).cos() / (1.0 + centre * 2.0);
                }
                let v = (0.5 + h).clamp(0.0, 1.0);
                Vec4::new(v, v, v, v)
            }
        }
    }
}

/// A 3D texture, by its node type.
#[derive(Clone, Debug)]
pub enum Tex3d {
    Brownian { lacunarity: f32, increment: f32, octaves: f32, weight: Vec3 },
    Cloud { color1: Vec3, color2: Vec3, contrast: f32, amplitude: f32, depth: Vec2, ripples: Vec3, soft_edges: bool, edge_thresh: f32, center_thresh: f32, transp_range: f32, ratio: f32 },
    Crater { shaker: f32, channel1: Vec3, channel2: Vec3, channel3: Vec3, melt: f32, balance: f32, frequency: f32 },
    Granite { color1: Vec3, color2: Vec3, color3: Vec3, filler: Vec3, cell_size: f32, density: f32, mix_ratio: f32, spottyness: f32, randomness: f32, threshold: f32, creases: bool },
    Leather { cell: Vec3, crease: Vec3, cell_size: f32, density: f32, spottyness: f32, randomness: f32, threshold: f32, creases: bool },
    Marble { filler: Vec3, vein: Vec3, vein_width: f32, diffusion: f32, contrast: f32, amplitude: f32, ratio: f32, ripples: Vec3, depth: Vec2 },
    Rock { color1: Vec3, color2: Vec3, grain_size: f32, diffusion: f32, mix_ratio: f32 },
    Snow { snow: Vec3, surface: Vec3, threshold: f32, depth_decay: f32, thickness: f32 },
    SolidFractal { threshold: f32, amplitude: f32, ratio: f32, frequency_ratio: f32, ripples: Vec3, depth: Vec2, bias: f32, inflection: bool, time: f32 },
    Stucco { shaker: f32, channel1: Vec3, channel2: Vec3 },
    VolumeNoise { threshold: f32, amplitude: f32, ratio: f32, frequency_ratio: f32, depth_max: f32, inflection: bool, time: f32, frequency: f32, scale: Vec3, origin: Vec3, noise_type: i64, density: f32, spottyness: f32, size_rand: f32, randomness: f32, num_waves: u32 },
    Wood { filler: Vec3, vein: Vec3, vein_spread: f32, layer_size: f32, randomness: f32, age: f32, grain: Vec3, grain_contrast: f32, grain_spacing: f32, center: Vec2, amplitude: Vec2, ratio: f32, ripples: Vec3, depth: Vec2 },
}

impl Tex3d {
    /// The colour and alpha at a point of the texture's space, with the surface's normal there.
    pub fn eval(&self, p: Vec3, n: Vec3) -> Vec4 {
        match self {
            Tex3d::Brownian { lacunarity, increment, octaves, weight } => {
                // Each octave weaker by the lacunarity raised to the increment: small increments stay rough.
                let q = p * *weight;
                let ratio = lacunarity.max(1.0001).powf(-increment.max(0.0));
                let v = (0.5 + 0.5 * fractal(q, 0.0, octaves.max(1.0), ratio, *lacunarity, false)).clamp(0.0, 1.0);
                Vec4::new(v, v, v, v)
            }
            Tex3d::Cloud { color1, color2, contrast, amplitude, depth, ripples, soft_edges, edge_thresh, center_thresh, transp_range, ratio } => {
                let f = 0.5 + 0.5 * fractal(p * *ripples, depth.x, depth.y, *ratio, 2.0, false) * *amplitude;
                let t = smooth(0.5 - contrast.clamp(0.0, 1.0) * 0.5, 0.5 + contrast.clamp(0.0, 1.0) * 0.5, f);
                let mut a = f.clamp(0.0, 1.0);
                if *soft_edges {
                    // Thinner towards the silhouette of the object it is on, from the middle threshold to the edge one.
                    let facing = n.z.abs();
                    a *= smooth(*center_thresh, edge_thresh.max(*center_thresh + 1e-3), facing).max(1.0 - *transp_range);
                }
                color1.lerp(*color2, t).extend(a)
            }
            Tex3d::Crater { shaker, channel1, channel2, channel3, melt, balance, frequency } => {
                let q = p * *frequency;
                let w = |salt: f32| {
                    let v = 0.5 + 0.5 * perlin(q * shaker.max(1e-3) * 0.5 + Vec3::splat(salt));
                    smooth(*melt * 0.5, 1.0 - *melt * 0.5, v)
                };
                let (a, b, c) = (w(0.0), w(31.7), w(73.1));
                let mix = (*channel1 * a + *channel2 * b + *channel3 * c) / (a + b + c).max(1e-4);
                let flat = (*channel1 + *channel2 + *channel3) / 3.0;
                mix.lerp(flat, balance.clamp(0.0, 1.0)).extend(((a + b + c) / 3.0).clamp(0.0, 1.0))
            }
            Tex3d::Granite { color1, color2, color3, filler, cell_size, density, mix_ratio, spottyness, randomness, threshold, creases } => {
                let (f1, f2, id) = cells(p / cell_size.max(1e-4), *randomness);
                let pick = (id & 0xffff) as f32 / 65535.0;
                // The mix ratio moves the shares of the three colours: 0 all the first, 1 all the third.
                let (a, b) = (1.0 - mix_ratio.clamp(0.0, 1.0), 1.0 - mix_ratio.clamp(0.0, 1.0) * 0.5);
                let mut c = if pick < a * 0.667 { *color1 } else if pick < b { *color2 } else { *color3 };
                c *= 1.0 + *spottyness * (((id >> 16) & 0xff) as f32 / 255.0 - 0.5);
                let edge = f2 - f1;
                let inside = f1 < threshold.clamp(0.0, 1.0) * density.max(0.0) && (!*creases || edge > 0.05);
                (if inside { c } else { *filler }).extend(if inside { 1.0 } else { 0.0 })
            }
            Tex3d::Leather { cell, crease, cell_size, density, spottyness, randomness, threshold, creases } => {
                let (f1, f2, id) = cells(p / cell_size.max(1e-4), *randomness);
                let shade = 1.0 + *spottyness * ((id & 0xff) as f32 / 255.0 - 0.5) * 2.0;
                let reach = threshold.clamp(0.0, 1.0) * density.max(0.0);
                // A cell is its colour out to the threshold, blending into the crease past it; creases are the lines
                // where two cells meet.
                let mut t = 1.0 - smooth(reach * 0.75, reach, f1);
                if *creases {
                    t *= smooth(0.0, 0.12, f2 - f1);
                }
                crease.lerp(*cell * shade, t).extend(t)
            }
            Tex3d::Marble { filler, vein, vein_width, diffusion, contrast, amplitude, ratio, ripples, depth } => {
                // The bands are bent by the coarse octaves; the fine ones only roughen their edges.
                let q = p * *ripples;
                let turb = (fractal(q, depth.x, depth.y.min(depth.x + 4.0), *ratio * 0.7, 2.0, false) + 0.08 * fractal(q * 16.0, 0.0, (depth.y - 4.0).clamp(0.0, 4.0), *ratio, 2.0, false)) * *amplitude * 0.5;
                let s = ((p.y + turb) * std::f32::consts::PI).sin().abs();
                // Veins where the bands cross zero, their pigment spreading into the filler by the diffusion.
                let w = vein_width.clamp(1e-4, 1.0);
                let t = 1.0 - smooth(w * (1.0 - diffusion.clamp(0.0, 1.0)), w + diffusion.clamp(0.0, 1.0) * (1.0 - w), s);
                let t = smooth(0.5 - (1.0 - contrast.clamp(0.0, 1.0)) * 0.5, 0.5 + (1.0 - contrast.clamp(0.0, 1.0)) * 0.5 + 1e-4, t);
                filler.lerp(*vein, t).extend(t)
            }
            Tex3d::Rock { color1, color2, grain_size, diffusion, mix_ratio } => {
                let v = 0.5 + 0.5 * perlin(p / grain_size.max(1e-4));
                let m = mix_ratio.clamp(0.0, 1.0);
                let d = diffusion.clamp(0.0, 1.0) * 0.5;
                let t = smooth(m - d, m + d + 1e-4, v);
                color2.lerp(*color1, t).extend(t)
            }
            Tex3d::Snow { snow, surface, threshold, depth_decay, thickness } => {
                // Snow settles where the surface faces up, less the steeper it is and none past the threshold.
                let up = n.y;
                let t = if up > *threshold { 1.0 - (-(up - *threshold) * depth_decay.max(0.0)).exp() } else { 0.0 };
                let t = (t * thickness.max(0.0)).clamp(0.0, 1.0);
                surface.lerp(*snow, t).extend(t)
            }
            Tex3d::SolidFractal { threshold, amplitude, ratio, frequency_ratio, ripples, depth, bias, inflection, time } => {
                let n = fractal(p * *ripples + Vec3::new(0.0, 0.0, *time), depth.x, depth.y, *ratio, *frequency_ratio, *inflection);
                let mut v = 0.5 + 0.5 * n * *amplitude;
                if *bias != 0.0 {
                    let b = (0.5 + 0.5 * bias.clamp(-1.0, 1.0)).clamp(1e-3, 1.0 - 1e-3);
                    v = v.clamp(0.0, 1.0).powf(b.ln() / 0.5f32.ln());
                }
                let v = (v + *threshold).clamp(0.0, 1.0);
                Vec4::new(v, v, v, v)
            }
            Tex3d::Stucco { shaker, channel1, channel2 } => {
                let v = 0.5 + 0.5 * fractal(p * (shaker.max(1e-3) / 10.0), 0.0, 4.0, 0.6, 2.0, false);
                let t = smooth(0.35, 0.65, v);
                channel1.lerp(*channel2, t).extend(t)
            }
            Tex3d::VolumeNoise { threshold, amplitude, ratio, frequency_ratio, depth_max, inflection, time, frequency, scale, origin, noise_type, density, spottyness, size_rand, randomness, num_waves } => {
                let q = (p - *origin) * *frequency / scale.max(Vec3::splat(1e-4));
                let depth = depth_max.clamp(1.0, 16.0);
                let v = match noise_type {
                    // Billow: soft blobs in a jittered lattice.
                    1 => {
                        let (mut sum, mut amp, mut f) = (0.0, 1.0, 1.0);
                        for k in 0..depth as u32 {
                            let (f1, _, id) = cells(q * f + Vec3::splat(k as f32 * 9.7 + *time), *randomness);
                            let size = 0.6 * (1.0 - *size_rand * ((id >> 8) & 0xff) as f32 / 255.0);
                            let bright = 1.0 - *spottyness * (id & 0xff) as f32 / 255.0;
                            sum += amp * bright * (1.0 - smooth(0.0, size.max(1e-3), f1)) * density.max(0.0);
                            amp *= *ratio;
                            f *= *frequency_ratio;
                        }
                        sum
                    }
                    // Waves: sines in random directions.
                    2 => {
                        let mut sum = 0.0;
                        let waves = (*num_waves).clamp(1, 64);
                        for k in 0..waves {
                            let d = Vec3::new(rand3(k as i32, 1, 0, 5) - 0.5, rand3(k as i32, 2, 0, 5) - 0.5, rand3(k as i32, 3, 0, 5) - 0.5).normalize_or_zero();
                            sum += ((q.dot(d) + rand3(k as i32, 4, 0, 5) + *time) * std::f32::consts::TAU).sin();
                        }
                        0.5 + 0.5 * sum / waves as f32
                    }
                    // Wispy: noise smeared by a second noise.
                    3 => {
                        let warp = Vec3::new(perlin(q * 0.5 + Vec3::splat(3.1)), perlin(q * 0.5 + Vec3::splat(11.3)), perlin(q * 0.5 + Vec3::splat(23.9)));
                        0.5 + 0.5 * fractal(q + warp * 2.0 + Vec3::new(0.0, 0.0, *time), 0.0, depth, *ratio, *frequency_ratio, *inflection)
                    }
                    _ => 0.5 + 0.5 * fractal(q + Vec3::new(0.0, 0.0, *time), 0.0, depth, *ratio, *frequency_ratio, *inflection),
                };
                let v = (v * *amplitude + *threshold).clamp(0.0, 1.0);
                Vec4::new(v, v, v, v)
            }
            Tex3d::Wood { filler, vein, vein_spread, layer_size, randomness, age, grain, grain_contrast, grain_spacing, center, amplitude, ratio, ripples, depth } => {
                // Rings round an axis along Z, seen as on a plank cut from the trunk: the pattern is the same all the
                // way through.
                let wobble = Vec2::new(fractal(p * *ripples, depth.x, depth.y, *ratio, 2.0, false) * amplitude.x, fractal(p * *ripples + Vec3::splat(41.0), depth.x, depth.y, *ratio, 2.0, false) * amplitude.y);
                let d = Vec2::new(p.x, p.y) - *center + wobble;
                let r = d.length();
                let ring = r / layer_size.max(1e-4);
                let k = ring.floor().min(age.max(1.0));
                let width = 1.0 + *randomness * (rand3(k as i32, 0, 0, 31) - 0.5);
                let f = ((ring - k) / width.max(0.1)).clamp(0.0, 1.0);
                // Each ring: filler, darkening into the vein at its outer edge.
                let t = smooth(1.0 - vein_spread.clamp(0.0, 1.0) * 2.0, 1.0, f);
                let mut c = filler.lerp(*vein, t);
                // The grain: dark flecks, as often as its spacing.
                let g = rand3((p.x / grain_spacing.max(1e-4)).floor() as i32, (p.y / grain_spacing.max(1e-4)).floor() as i32, (p.z / (grain_spacing.max(1e-4) * 8.0)).floor() as i32, 33);
                if g > 0.8 {
                    c = c.lerp(*grain, grain_contrast.clamp(0.0, 1.0) * (g - 0.8) * 5.0);
                }
                c.extend(t)
            }
        }
    }
}

/// The matrix a 3D texture's placement gives is world to texture; a texture set local reads the object's own space.
pub fn texture_point(to_texture: &Mat4, world: Vec3, object: Vec3, local: bool) -> Vec3 {
    to_texture.transform_point3(if local { object } else { world })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn patterns_answer_their_controls() {
        // A grid is its line colour on the lines and its filler between; no contrast is the mean of the two.
        let grid = |contrast: f32| Tex2d::Grid { line: Vec3::ONE, filler: Vec3::ZERO, u_width: 0.1, v_width: 0.1, contrast };
        assert_eq!(grid(1.0).eval(Vec2::new(0.001, 0.1)).truncate(), Vec3::ONE);
        assert_eq!(grid(1.0).eval(Vec2::new(0.125, 0.125)).truncate(), Vec3::ZERO);
        assert_eq!(grid(0.0).eval(Vec2::new(0.125, 0.125)).truncate(), Vec3::splat(0.5));
        // A bulge is level on its border and highest in the middle.
        let bulge = Tex2d::Bulge { u_width: 0.1, v_width: 0.1 };
        assert_eq!(bulge.eval(Vec2::new(0.05, 0.5)).x, 0.0);
        assert!((bulge.eval(Vec2::new(0.5, 0.5)).x - 1.0).abs() < 1e-5);
        // Cloth shows its gap colour between threads narrower than the cell.
        let cloth = Tex2d::Cloth { gap: Vec3::new(1.0, 0.0, 0.0), u_color: Vec3::ONE, v_color: Vec3::splat(0.5), u_width: 0.5, v_width: 0.5, u_wave: 0.0, v_wave: 0.0, randomness: 0.0, width_spread: 0.0, bright_spread: 0.0 };
        assert_eq!(cloth.eval(Vec2::new(0.01, 0.01)).truncate(), Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(cloth.eval(Vec2::new(0.01, 0.125)).truncate(), Vec3::ONE, "a thread along U");
        // Snow lies on what faces up and not on what faces down.
        let snow = Tex3d::Snow { snow: Vec3::ONE, surface: Vec3::new(0.5, 0.0, 0.0), threshold: 0.5, depth_decay: 5.0, thickness: 1.0 };
        assert!(snow.eval(Vec3::ZERO, Vec3::Y).x > 0.9);
        assert_eq!(snow.eval(Vec3::ZERO, -Vec3::Y).truncate(), Vec3::new(0.5, 0.0, 0.0));
        // Rock at a mix ratio of 0 and 1 is one colour or the other.
        let rock = |m: f32| Tex3d::Rock { color1: Vec3::X, color2: Vec3::Y, grain_size: 0.01, diffusion: 0.0, mix_ratio: m };
        assert_eq!(rock(1.01).eval(Vec3::new(0.3, 0.2, 0.1), Vec3::Z).truncate(), Vec3::Y);
        assert_eq!(rock(-0.01).eval(Vec3::new(0.3, 0.2, 0.1), Vec3::Z).truncate(), Vec3::X);
        // Everything stays finite and in range over a spread of points.
        let all: Vec<Tex3d> = vec![
            Tex3d::Brownian { lacunarity: 4.0, increment: 0.1, octaves: 3.0, weight: Vec3::ONE },
            Tex3d::Cloud { color1: Vec3::ZERO, color2: Vec3::ONE, contrast: 0.5, amplitude: 1.0, depth: Vec2::new(0.0, 8.0), ripples: Vec3::ONE, soft_edges: true, edge_thresh: 1.0, center_thresh: 0.0, transp_range: 0.5, ratio: 0.707 },
            Tex3d::Crater { shaker: 1.5, channel1: Vec3::X, channel2: Vec3::Y, channel3: Vec3::Z, melt: 0.0, balance: 0.0, frequency: 2.0 },
            Tex3d::Granite { color1: Vec3::ZERO, color2: Vec3::new(0.549, 0.784, 0.392), color3: Vec3::new(0.627, 0.824, 0.823), filler: Vec3::new(0.588, 0.294, 0.196), cell_size: 0.15, density: 1.0, mix_ratio: 0.5, spottyness: 0.3, randomness: 1.0, threshold: 0.5, creases: true },
            Tex3d::Leather { cell: Vec3::new(0.373, 0.157, 0.059), crease: Vec3::new(0.235, 0.118, 0.0), cell_size: 0.5, density: 1.0, spottyness: 0.1, randomness: 0.5, threshold: 0.83, creases: true },
            Tex3d::Marble { filler: Vec3::ONE, vein: Vec3::new(0.298, 0.0, 0.0), vein_width: 0.1, diffusion: 0.5, contrast: 0.5, amplitude: 1.5, ratio: 0.707, ripples: Vec3::ONE, depth: Vec2::new(0.0, 20.0) },
            Tex3d::SolidFractal { threshold: 0.0, amplitude: 1.0, ratio: 0.707, frequency_ratio: 2.0, ripples: Vec3::ONE, depth: Vec2::new(0.0, 8.0), bias: 0.0, inflection: false, time: 0.0 },
            Tex3d::Stucco { shaker: 20.0, channel1: Vec3::X, channel2: Vec3::Z },
            Tex3d::VolumeNoise { threshold: 0.0, amplitude: 1.0, ratio: 0.707, frequency_ratio: 2.0, depth_max: 3.0, inflection: false, time: 0.0, frequency: 8.0, scale: Vec3::ONE, origin: Vec3::ZERO, noise_type: 1, density: 1.0, spottyness: 0.1, size_rand: 0.0, randomness: 1.0, num_waves: 5 },
            Tex3d::Wood { filler: Vec3::new(0.824, 0.627, 0.471), vein: Vec3::new(0.157, 0.078, 0.039), vein_spread: 0.25, layer_size: 0.05, randomness: 0.5, age: 20.0, grain: Vec3::new(0.118, 0.039, 0.0), grain_contrast: 0.5, grain_spacing: 0.01, center: Vec2::new(0.5, -0.5), amplitude: Vec2::new(0.1, 0.1), ratio: 0.35, ripples: Vec3::ONE, depth: Vec2::new(0.0, 8.0) },
        ];
        for t in &all {
            let mut varies = false;
            let first = t.eval(Vec3::new(0.13, 0.27, 0.41), Vec3::Z);
            for k in 0..200 {
                let p = Vec3::new((k as f32 * 0.37).sin() * 3.0, (k as f32 * 0.11).cos() * 3.0, k as f32 * 0.05);
                let c = t.eval(p, Vec3::Z);
                assert!(c.is_finite() && c.min_element() >= -1e-4 && c.max_element() <= 1.5, "{t:?} at {p}: {c}");
                varies |= (c - first).abs().max_element() > 1e-3;
            }
            assert!(varies, "{t:?} is one colour everywhere");
        }
    }
}
