//! Lights, each sampled for direct lighting and, where it has a surface, found by rays so that its light is weighted by
//! multiple importance sampling against the material's own samples. Intensities follow Arnold's convention: colour
//! times intensity times two to the exposure, quadratic falloff, area lights divided by their area when normalised.

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};

use crate::math::{uniform_cone, uniform_sphere, uniform_triangle, Frame, Ray, PI, TAU};
use crate::texture::Image;

/// A photometric profile: candela by vertical and horizontal angle (degrees), read from an IES file.
#[derive(Clone, Debug)]
pub struct Ies {
    pub vertical: Vec<f32>,
    pub horizontal: Vec<f32>,
    /// candela[h][v]
    pub candela: Vec<Vec<f32>>,
    pub max: f32,
}

impl Ies {
    /// Reads LM-63 text with TILT=NONE.
    pub fn parse(text: &str) -> Option<Ies> {
        let at = text.find("TILT=")?;
        let rest = &text[at..];
        let line_end = rest.find('\n')?;
        if !rest[..line_end].contains("NONE") {
            return None;
        }
        let nums: Vec<f32> = rest[line_end..].split(|c: char| c.is_whitespace() || c == ',').filter(|s| !s.is_empty()).filter_map(|s| s.parse().ok()).collect();
        if nums.len() < 13 {
            return None;
        }
        let multiplier = nums[2];
        let (nv, nh) = (nums[3] as usize, nums[4] as usize);
        let mut k = 13;
        let vertical: Vec<f32> = nums.get(k..k + nv)?.to_vec();
        k += nv;
        let horizontal: Vec<f32> = nums.get(k..k + nh)?.to_vec();
        k += nh;
        let mut candela = Vec::with_capacity(nh);
        for _ in 0..nh {
            candela.push(nums.get(k..k + nv)?.iter().map(|c| c * multiplier).collect::<Vec<f32>>());
            k += nv;
        }
        let max = candela.iter().flatten().fold(0.0f32, |a, &b| a.max(b));
        Some(Ies { vertical, horizontal, candela, max })
    }

    /// Relative intensity, 0 to 1, for a direction in the light's frame (-z down the beam).
    pub fn at(&self, d: Vec3) -> f32 {
        if self.max <= 0.0 {
            return 0.0;
        }
        let v = (-d.z).clamp(-1.0, 1.0).acos().to_degrees();
        let mut h = d.y.atan2(d.x).to_degrees().rem_euclid(360.0);
        let hmax = *self.horizontal.last().unwrap_or(&0.0);
        if hmax <= 0.0 {
            h = 0.0;
        } else if hmax <= 90.0 {
            h = if h > 180.0 { 360.0 - h } else { h };
            h = if h > 90.0 { 180.0 - h } else { h };
        } else if hmax <= 180.0 && h > 180.0 {
            h = 360.0 - h;
        }
        let interp = |xs: &[f32], x: f32| -> (usize, usize, f32) {
            if xs.len() < 2 || x <= xs[0] {
                return (0, 0, 0.0);
            }
            for i in 1..xs.len() {
                if x <= xs[i] {
                    return (i - 1, i, (x - xs[i - 1]) / (xs[i] - xs[i - 1]).max(1e-6));
                }
            }
            (xs.len() - 1, xs.len() - 1, 0.0)
        };
        let (va, vb, vf) = interp(&self.vertical, v);
        if v > *self.vertical.last().unwrap_or(&180.0) {
            return 0.0;
        }
        let (ha, hb, hf) = interp(&self.horizontal, h);
        let c = |hi: usize| self.candela[hi][va] * (1.0 - vf) + self.candela[hi][vb] * vf;
        (c(ha) * (1.0 - hf) + c(hb) * hf) / self.max
    }
}

/// A piecewise constant 2D distribution over an image's pixels, for importance sampling the dome.
#[derive(Clone, Debug)]
pub struct Distribution2d {
    pub w: usize,
    pub h: usize,
    /// Per row, the cumulative of its columns, normalised.
    cond: Vec<Vec<f32>>,
    marg: Vec<f32>,
    total: f32,
    func: Vec<f32>,
}

fn cdf_of(f: &[f32]) -> (Vec<f32>, f32) {
    let mut c = Vec::with_capacity(f.len() + 1);
    c.push(0.0);
    let mut s = 0.0;
    for &x in f {
        s += x;
        c.push(s);
    }
    if s > 0.0 {
        for x in &mut c {
            *x /= s;
        }
    } else {
        for (i, x) in c.iter_mut().enumerate() {
            *x = i as f32 / f.len() as f32;
        }
    }
    (c, s)
}

fn sample_cdf(c: &[f32], u: f32) -> (usize, f32) {
    let n = c.len() - 1;
    let i = c.partition_point(|&x| x <= u).saturating_sub(1).min(n - 1);
    let span = (c[i + 1] - c[i]).max(1e-12);
    (i, ((u - c[i]) / span).clamp(0.0, 0.999_999))
}

impl Distribution2d {
    pub fn new(func: Vec<f32>, w: usize, h: usize) -> Distribution2d {
        let mut cond = Vec::with_capacity(h);
        let mut rows = Vec::with_capacity(h);
        for y in 0..h {
            let (c, s) = cdf_of(&func[y * w..(y + 1) * w]);
            cond.push(c);
            rows.push(s);
        }
        let (marg, total) = cdf_of(&rows);
        Distribution2d { w, h, cond, marg, total, func }
    }
    /// A point in [0,1]^2 and its density there.
    pub fn sample(&self, u: Vec2) -> (Vec2, f32) {
        let (y, fy) = sample_cdf(&self.marg, u.y);
        let (x, fx) = sample_cdf(&self.cond[y], u.x);
        let p = Vec2::new((x as f32 + fx) / self.w as f32, (y as f32 + fy) / self.h as f32);
        (p, self.pdf(p))
    }
    pub fn pdf(&self, p: Vec2) -> f32 {
        if self.total <= 0.0 {
            return 1.0;
        }
        let x = ((p.x * self.w as f32) as usize).min(self.w - 1);
        let y = ((p.y * self.h as f32) as usize).min(self.h - 1);
        self.func[y * self.w + x] * (self.w * self.h) as f32 / self.total
    }
}

#[derive(Clone, Debug)]
pub enum Shape {
    /// A sphere of the radius, or a point at radius 0.
    Point { pos: Vec3, radius: f32 },
    Spot { pos: Vec3, radius: f32, frame: Frame, cos_outer: f32, cos_inner: f32, dropoff: f32 },
    /// Light arriving from `dir` (towards the light) across a cone of the given cosine.
    Distant { dir: Vec3, cos_half: f32 },
    /// A parallelogram from `corner` along `u` and `v`, emitting along `n`.
    Quad { corner: Vec3, u: Vec3, v: Vec3, n: Vec3, two_sided: bool, spread: f32 },
    Disk { center: Vec3, frame: Frame, radius: f32, two_sided: bool },
    /// From a to b (the axis) at the radius, emitting outward.
    Cylinder { a: Vec3, b: Vec3, radius: f32 },
    /// The environment: rotation, and an image or a constant colour.
    Dome { to_local: Mat4, image: Option<Arc<Image>>, dist: Option<Arc<Distribution2d>> },
    /// Emitting triangles of an instance, chosen by area.
    Mesh { inst: u32, tris: Vec<u32>, cdf: Vec<f32>, area: f32 },
    /// Maya's ambient light: light from all around unshadowed, and the shade part from its position.
    Ambient { pos: Vec3, shade: f32 },
    /// Maya's volume light: light from its centre that fades to nothing at the volume's edge.
    Volume { pos: Vec3, radius: f32 },
}

#[derive(Clone, Debug)]
pub struct Light {
    pub name: String,
    pub shape: Shape,
    /// Colour times intensity times 2^exposure; radiance for area lights after normalising.
    pub color: Vec3,
    /// The light's own colour from a texture (a dome's image is its own shape's).
    pub samples: u32,
    pub cast_shadows: bool,
    pub shadow_color: Vec3,
    pub shadow_density: f32,
    pub diffuse: f32,
    pub specular: f32,
    pub sss: f32,
    pub transmission: f32,
    pub volume: f32,
    pub max_bounces: u32,
    /// Which light group AOV gets its light.
    pub group: u16,
    /// Seen directly by camera rays (domes are; area lights are not, as Arnold's default).
    pub camera: f32,
    pub ies: Option<Arc<Ies>>,
    /// Decay exponent: 2 physical, 0 none.
    pub decay: f32,
}

impl Light {
    pub fn new(name: &str, shape: Shape, color: Vec3) -> Light {
        let camera = if matches!(shape, Shape::Dome { .. }) { 1.0 } else { 0.0 };
        Light { name: name.into(), shape, color, samples: 1, cast_shadows: true, shadow_color: Vec3::ZERO, shadow_density: 1.0, diffuse: 1.0, specular: 1.0, sss: 1.0, transmission: 1.0, volume: 1.0, max_bounces: 999, group: 0, camera, ies: None, decay: 2.0 }
    }

    pub fn is_delta(&self) -> bool {
        match &self.shape {
            Shape::Point { radius, .. } | Shape::Spot { radius, .. } => *radius <= 0.0,
            Shape::Distant { cos_half, .. } => *cos_half >= 1.0,
            Shape::Volume { .. } => true,
            _ => false,
        }
    }

    pub fn is_dome(&self) -> bool {
        matches!(self.shape, Shape::Dome { .. })
    }

    /// The weight a material lobe category gives this light.
    pub fn weight_for(&self, cat: crate::bsdf::Category) -> f32 {
        use crate::bsdf::Category::*;
        match cat {
            Diffuse | Sheen => self.diffuse,
            Specular | Coat => self.specular,
            Transmission => self.transmission,
            Sss => self.sss,
        }
    }
}

/// A sampled direction towards a light.
#[derive(Clone, Copy, Debug)]
pub struct LightSample {
    pub wi: Vec3,
    pub dist: f32,
    /// Incoming radiance before shadowing; for mesh lights the emission is looked up by the caller.
    pub le: Vec3,
    /// Solid angle density; for delta lights 1 with `delta` set.
    pub pdf: f32,
    pub delta: bool,
    /// Skip the shadow ray.
    pub unshadowed: bool,
    /// A mesh light's point: instance, triangle and barycentrics, for the caller to shade.
    pub mesh: Option<(u32, u32, f32, f32)>,
}

pub fn latlong_uv(d: Vec3) -> Vec2 {
    let u = 0.5 + d.x.atan2(-d.z) / TAU;
    let v = d.y.clamp(-1.0, 1.0).acos() / PI;
    Vec2::new(u.rem_euclid(1.0), v)
}

pub fn latlong_dir(uv: Vec2) -> Vec3 {
    let phi = (uv.x - 0.5) * TAU;
    let th = uv.y * PI;
    Vec3::new(th.sin() * phi.sin(), th.cos(), -th.sin() * phi.cos())
}

/// Dome radiance for a world direction, image looked up at the given width in UV.
pub fn dome_radiance(to_local: &Mat4, image: &Option<Arc<Image>>, color: Vec3, d: Vec3) -> Vec3 {
    match image {
        None => color,
        Some(img) => {
            let l = to_local.transform_vector3(d).normalize_or_zero();
            let uv = latlong_uv(l);
            // Image rows go down with v; sample() takes v up.
            img.sample(Vec2::new(uv.x, 1.0 - uv.y), 0.0).truncate() * color
        }
    }
}

/// The distribution of a dome image's light over directions, at a size cheap enough to build.
pub fn dome_distribution(img: &Image) -> Distribution2d {
    let w = img.width.clamp(8, 512) as usize;
    let h = (w / 2).max(4);
    let mut lum = vec![0.0f32; w * h];
    for y in 0..h {
        let v = (y as f32 + 0.5) / h as f32;
        for x in 0..w {
            let u = (x as f32 + 0.5) / w as f32;
            let c = img.sample(Vec2::new(u, 1.0 - v), 1.0 / w as f32).truncate();
            lum[y * w + x] = crate::math::luminance(c).max(0.0);
        }
    }
    // Each cell takes the brightest of its neighbours: radiance is looked up between texels, and a dim cell beside the
    // sun must not be sampled too rarely for the light that spills into it.
    let mut f = vec![0.0; w * h];
    for y in 0..h {
        let s = ((y as f32 + 0.5) / h as f32 * PI).sin();
        for x in 0..w {
            let mut m = 0.0f32;
            for dy in -1i32..=1 {
                let yy = (y as i32 + dy).clamp(0, h as i32 - 1) as usize;
                for dx in -1i32..=1 {
                    let xx = (x as i32 + dx).rem_euclid(w as i32) as usize;
                    m = m.max(lum[yy * w + xx]);
                }
            }
            f[y * w + x] = (m + 1e-4) * s;
        }
    }
    Distribution2d::new(f, w, h)
}

fn sphere_sample(c: Vec3, r: f32, p: Vec3, u: Vec2) -> Option<(Vec3, f32, f32)> {
    let to = c - p;
    let d2 = to.length_squared();
    if d2 <= r * r {
        return None;
    }
    let d = d2.sqrt();
    let sin2 = r * r / d2;
    let cos_max = (1.0 - sin2).max(0.0).sqrt();
    let f = Frame::from_n(to / d);
    let wi = f.to_world(uniform_cone(u, cos_max));
    // Distance to the sphere's near side along wi.
    let b = wi.dot(to);
    let disc = (b * b - (d2 - r * r)).max(0.0);
    let t = b - disc.sqrt();
    Some((wi, t.max(0.0), 1.0 / (TAU * (1.0 - cos_max)).max(1e-12)))
}

fn sphere_pdf(c: Vec3, r: f32, p: Vec3) -> f32 {
    let d2 = (c - p).length_squared();
    if d2 <= r * r {
        return 0.0;
    }
    let cos_max = (1.0 - r * r / d2).max(0.0).sqrt();
    1.0 / (TAU * (1.0 - cos_max)).max(1e-12)
}

fn ray_sphere(c: Vec3, r: f32, ray: &Ray, tmax: f32) -> Option<f32> {
    let oc = ray.o - c;
    let b = oc.dot(ray.d);
    let cc = oc.length_squared() - r * r;
    let disc = b * b - cc;
    if disc < 0.0 {
        return None;
    }
    let s = disc.sqrt();
    let t = if -b - s > 1e-5 { -b - s } else { -b + s };
    (t > 1e-5 && t < tmax).then_some(t)
}

impl Light {
    /// Samples a direction from point p (normal n, for lights from all around) towards the light.
    pub fn sample(&self, p: Vec3, n: Vec3, u: Vec2) -> Option<LightSample> {
        let none = |wi: Vec3, dist: f32, le: Vec3, pdf: f32, delta: bool| Some(LightSample { wi, dist, le, pdf, delta, unshadowed: false, mesh: None });
        match &self.shape {
            Shape::Point { pos, radius } => {
                if *radius > 0.0 {
                    let (wi, t, pdf) = sphere_sample(*pos, *radius, p, u)?;
                    let le = self.color / (PI * radius * radius);
                    return none(wi, t, le, pdf, false);
                }
                let to = *pos - p;
                let d = to.length();
                none(to / d, d, self.color / d.powf(self.decay).max(1e-12), 1.0, true)
            }
            Shape::Spot { pos, radius, frame, cos_outer, cos_inner, dropoff } => {
                let (wi, dist, mut le, pdf, delta) = if *radius > 0.0 {
                    // A disk facing down the beam.
                    let d = crate::math::concentric_disk(u) * *radius;
                    let q = *pos + frame.t * d.x + frame.b * d.y;
                    let to = q - p;
                    let dist = to.length();
                    let wi = to / dist;
                    let cos_l = (-wi).dot(frame.n);
                    if cos_l <= 0.0 {
                        return None;
                    }
                    let area = PI * radius * radius;
                    (wi, dist, self.color / area, dist * dist / (cos_l * area), false)
                } else {
                    let to = *pos - p;
                    let d = to.length();
                    (to / d, d, self.color / d.powf(self.decay).max(1e-12), 1.0, true)
                };
                let c = (-wi).dot(frame.n);
                le *= spot_falloff(c, *cos_outer, *cos_inner, *dropoff);
                if let Some(ies) = &self.ies {
                    le *= ies.at(frame.to_local(-wi) * Vec3::new(1.0, 1.0, -1.0));
                }
                if le.max_element() <= 0.0 {
                    return None;
                }
                none(wi, dist, le, pdf, delta)
            }
            Shape::Distant { dir, cos_half } => {
                if *cos_half >= 1.0 {
                    return none(*dir, f32::INFINITY, self.color, 1.0, true);
                }
                let f = Frame::from_n(*dir);
                let wi = f.to_world(uniform_cone(u, *cos_half));
                let solid = TAU * (1.0 - cos_half);
                // Irradiance from the cone equals the light's intensity, as Arnold's distant light.
                none(wi, f32::INFINITY, self.color / solid, 1.0 / solid, false)
            }
            Shape::Quad { corner, u: eu, v: ev, n: ln, two_sided, spread } => {
                let q = *corner + *eu * u.x + *ev * u.y;
                let to = q - p;
                let dist = to.length();
                let wi = to / dist;
                let mut cos_l = (-wi).dot(*ln);
                if *two_sided {
                    cos_l = cos_l.abs();
                }
                if cos_l <= 1e-6 {
                    return None;
                }
                let area = eu.cross(*ev).length();
                let le = self.color * spread_falloff(cos_l, *spread);
                none(wi, dist, le, dist * dist / (cos_l * area), false)
            }
            Shape::Disk { center, frame, radius, two_sided } => {
                let d = crate::math::concentric_disk(u) * *radius;
                let q = *center + frame.t * d.x + frame.b * d.y;
                let to = q - p;
                let dist = to.length();
                let wi = to / dist;
                let mut cos_l = (-wi).dot(frame.n);
                if *two_sided {
                    cos_l = cos_l.abs();
                }
                if cos_l <= 1e-6 {
                    return None;
                }
                let area = PI * radius * radius;
                none(wi, dist, self.color, dist * dist / (cos_l * area), false)
            }
            Shape::Cylinder { a, b, radius } => {
                let axis = *b - *a;
                let len = axis.length();
                let f = Frame::from_n(axis / len.max(1e-9));
                let phi = TAU * u.y;
                let nrm = f.t * phi.cos() + f.b * phi.sin();
                let q = *a + axis * u.x + nrm * *radius;
                let to = q - p;
                let dist = to.length();
                let wi = to / dist;
                let cos_l = (-wi).dot(nrm);
                if cos_l <= 1e-6 {
                    return None;
                }
                let area = TAU * radius * len;
                none(wi, dist, self.color, dist * dist / (cos_l * area), false)
            }
            Shape::Dome { to_local, image, dist } => {
                let (wi, pdf) = match dist {
                    Some(d) => {
                        let (uv, pdf_uv) = d.sample(u);
                        let l = latlong_dir(uv);
                        let s = (uv.y * PI).sin();
                        if s <= 1e-6 {
                            return None;
                        }
                        (to_local.inverse().transform_vector3(l).normalize(), pdf_uv / (2.0 * PI * PI * s))
                    }
                    None => {
                        // Cosine about the normal for a constant dome: less noise than uniform.
                        let f = Frame::from_n(n);
                        let d = crate::math::cosine_hemisphere(u);
                        (f.to_world(d), d.z / PI)
                    }
                };
                if pdf <= 0.0 {
                    return None;
                }
                let le = dome_radiance(to_local, image, self.color, wi);
                none(wi, f32::INFINITY, le, pdf, false)
            }
            Shape::Mesh { .. } => None,
            Shape::Ambient { pos, shade } => {
                if u.x < 0.5 {
                    let f = Frame::from_n(n);
                    let d = crate::math::cosine_hemisphere(Vec2::new(u.x * 2.0, u.y));
                    let s = LightSample { wi: f.to_world(d), dist: f32::INFINITY, le: self.color * (1.0 - shade) / PI, pdf: d.z / PI * 0.5, delta: false, unshadowed: true, mesh: None };
                    return Some(s);
                }
                let to = *pos - p;
                let dist = to.length();
                Some(LightSample { wi: to / dist, dist, le: self.color * *shade * 2.0, pdf: 1.0, delta: true, unshadowed: true, mesh: None })
            }
            Shape::Volume { pos, radius } => {
                let to = *pos - p;
                let d = to.length();
                if d >= *radius {
                    return None;
                }
                none(to / d.max(1e-9), d, self.color * (1.0 - d / radius), 1.0, true)
            }
        }
    }

    /// A mesh light's sample: a point on one of its triangles. The caller has the geometry.
    pub fn sample_mesh(&self, geo: &crate::geom::Geometry, p: Vec3, time: f32, u: Vec2, u2: f32) -> Option<LightSample> {
        let Shape::Mesh { inst, tris, cdf, area } = &self.shape else { return None };
        let k = cdf.partition_point(|&c| c <= u2).min(tris.len().saturating_sub(1));
        let prim = *tris.get(k)?;
        let ins = &geo.instances[*inst as usize];
        let mesh = &geo.meshes[ins.mesh as usize];
        let (m, _) = ins.xform_at(time);
        let (a, b, c) = mesh.tri_points(prim, time);
        let (a, b, c) = (m.transform_point3(a), m.transform_point3(b), m.transform_point3(c));
        let (b1, b2) = uniform_triangle(u);
        let q = a * (1.0 - b1 - b2) + b * b1 + c * b2;
        let ng = (b - a).cross(c - a).normalize_or_zero();
        let to = q - p;
        let dist = to.length();
        let wi = to / dist;
        let cos_l = wi.dot(ng).abs();
        if cos_l <= 1e-6 || *area <= 0.0 {
            return None;
        }
        Some(LightSample { wi, dist, le: self.color, pdf: dist * dist / (cos_l * area), delta: false, unshadowed: false, mesh: Some((*inst, prim, b1, b2)) })
    }

    /// Density, per solid angle from p, of this light's sampling having picked direction wi that met it at distance t
    /// with surface cosine cos_l (for area and mesh lights).
    pub fn pdf_area(&self, dist: f32, cos_l: f32) -> f32 {
        let area = match &self.shape {
            Shape::Quad { u, v, .. } => u.cross(*v).length(),
            Shape::Disk { radius, .. } => PI * radius * radius,
            Shape::Cylinder { a, b, radius } => TAU * radius * (*b - *a).length(),
            Shape::Mesh { area, .. } => *area,
            Shape::Spot { radius, .. } => PI * radius * radius,
            _ => return 0.0,
        };
        if area <= 0.0 || cos_l <= 0.0 {
            return 0.0;
        }
        dist * dist / (cos_l * area)
    }

    /// Where a ray meets this light's own surface: distance, radiance, and the solid angle density of sampling it.
    /// Mesh lights are part of the geometry and are found there instead.
    pub fn intersect(&self, ray: &Ray, tmax: f32) -> Option<(f32, Vec3, f32)> {
        match &self.shape {
            Shape::Point { pos, radius } if *radius > 0.0 => {
                let t = ray_sphere(*pos, *radius, ray, tmax)?;
                Some((t, self.color / (PI * radius * radius), sphere_pdf(*pos, *radius, ray.o)))
            }
            Shape::Quad { corner, u, v, n, two_sided, spread } => {
                let denom = ray.d.dot(*n);
                if denom.abs() < 1e-9 {
                    return None;
                }
                let t = (*corner - ray.o).dot(*n) / denom;
                if t <= 1e-5 || t >= tmax {
                    return None;
                }
                let q = ray.at(t) - *corner;
                let (uu, vv) = (u.dot(*u), v.dot(*v));
                let (a, b) = (q.dot(*u) / uu, q.dot(*v) / vv);
                if !(0.0..=1.0).contains(&a) || !(0.0..=1.0).contains(&b) {
                    return None;
                }
                let mut cos_l = -denom;
                if *two_sided {
                    cos_l = cos_l.abs();
                }
                if cos_l <= 0.0 {
                    return None;
                }
                Some((t, self.color * spread_falloff(cos_l, *spread), self.pdf_area(t, cos_l)))
            }
            Shape::Disk { center, frame, radius, two_sided } => {
                let denom = ray.d.dot(frame.n);
                if denom.abs() < 1e-9 {
                    return None;
                }
                let t = (*center - ray.o).dot(frame.n) / denom;
                if t <= 1e-5 || t >= tmax || (ray.at(t) - *center).length_squared() > radius * radius {
                    return None;
                }
                let mut cos_l = -denom;
                if *two_sided {
                    cos_l = cos_l.abs();
                }
                if cos_l <= 0.0 {
                    return None;
                }
                Some((t, self.color, self.pdf_area(t, cos_l)))
            }
            Shape::Cylinder { a, b, radius } => {
                let axis = (*b - *a).normalize();
                let len = (*b - *a).length();
                let oc = ray.o - *a;
                let d = ray.d - axis * ray.d.dot(axis);
                let o = oc - axis * oc.dot(axis);
                let qa = d.dot(d);
                let qb = 2.0 * d.dot(o);
                let qc = o.dot(o) - radius * radius;
                let disc = qb * qb - 4.0 * qa * qc;
                if qa < 1e-12 || disc < 0.0 {
                    return None;
                }
                let s = disc.sqrt();
                for t in [(-qb - s) / (2.0 * qa), (-qb + s) / (2.0 * qa)] {
                    if t <= 1e-5 || t >= tmax {
                        continue;
                    }
                    let h = (ray.at(t) - *a).dot(axis);
                    if h < 0.0 || h > len {
                        continue;
                    }
                    let nrm = (ray.at(t) - *a - axis * h).normalize();
                    let cos_l = -ray.d.dot(nrm);
                    if cos_l <= 0.0 {
                        return None;
                    }
                    return Some((t, self.color, self.pdf_area(t, cos_l)));
                }
                None
            }
            _ => None,
        }
    }

    /// The dome's radiance in a direction, and its sampling density there.
    pub fn dome_eval(&self, d: Vec3, n: Vec3) -> Option<(Vec3, f32)> {
        let Shape::Dome { to_local, image, dist } = &self.shape else { return None };
        let le = dome_radiance(to_local, image, self.color, d);
        let pdf = match dist {
            Some(dd) => {
                let uv = latlong_uv(to_local.transform_vector3(d).normalize_or_zero());
                let s = (uv.y * PI).sin();
                if s <= 1e-6 {
                    0.0
                } else {
                    dd.pdf(uv) / (2.0 * PI * PI * s)
                }
            }
            None => d.dot(n).max(0.0) / PI,
        };
        Some((le, pdf))
    }

    /// Total power, roughly: for choosing lights and for statistics.
    pub fn power(&self) -> f32 {
        crate::math::luminance(self.color)
    }
}

/// Maya's spot falloff: full inside the inner cone, smooth to nothing at the outer, and cos^dropoff.
pub fn spot_falloff(c: f32, cos_outer: f32, cos_inner: f32, dropoff: f32) -> f32 {
    if c <= cos_outer {
        return 0.0;
    }
    let mut f = 1.0;
    if c < cos_inner {
        let t = ((c - cos_outer) / (cos_inner - cos_outer).max(1e-6)).clamp(0.0, 1.0);
        f = t * t * (3.0 - 2.0 * t);
    }
    if dropoff > 0.0 {
        f *= c.max(0.0).powf(dropoff);
    }
    f
}

/// Arnold's quad light spread: 1 is diffuse, smaller focuses the light along the normal.
fn spread_falloff(cos_l: f32, spread: f32) -> f32 {
    if spread >= 1.0 {
        return 1.0;
    }
    let s = spread.max(1e-3);
    let p = (1.0 / s - 1.0).max(0.0) * 2.0;
    cos_l.powf(p) * (p + 2.0) / 2.0
}

/// A mesh light's triangles by area.
pub fn mesh_light_shape(geo: &crate::geom::Geometry, inst: u32, tris: Option<Vec<u32>>) -> Shape {
    let ins = &geo.instances[inst as usize];
    let mesh = &geo.meshes[ins.mesh as usize];
    let (m, _) = ins.xform_at(0.5);
    let tris = tris.unwrap_or_else(|| (0..mesh.tris.len() as u32).collect());
    let mut cdf = Vec::with_capacity(tris.len());
    let mut total = 0.0;
    for &t in &tris {
        let (a, b, c) = mesh.tri_points(t, 0.5);
        let (a, b, c) = (m.transform_point3(a), m.transform_point3(b), m.transform_point3(c));
        total += 0.5 * (b - a).cross(c - a).length();
        cdf.push(total);
    }
    for c in &mut cdf {
        *c /= total.max(1e-30);
    }
    Shape::Mesh { inst, tris, cdf, area: total }
}

pub fn sample_sphere_dir(u: Vec2) -> Vec3 {
    uniform_sphere(u)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distribution_follows_function() {
        let d = Distribution2d::new(vec![1.0, 3.0, 0.0, 0.0], 2, 2);
        let mut right = 0;
        let mut rng = crate::math::Pcg::new(3);
        for _ in 0..10000 {
            let (p, pdf) = d.sample(rng.next_2d());
            assert!(p.y < 0.5 && pdf > 0.0);
            if p.x > 0.5 {
                right += 1;
            }
        }
        assert!((right as f32 / 10000.0 - 0.75).abs() < 0.02);
        assert!((d.pdf(Vec2::new(0.75, 0.25)) - 3.0).abs() < 1e-5);
    }

    #[test]
    fn latlong_round_trip() {
        for d in [Vec3::X, -Vec3::Z, Vec3::new(0.3, 0.5, 0.2).normalize()] {
            let back = latlong_dir(latlong_uv(d));
            assert!((back - d).length() < 1e-4, "{d} {back}");
        }
    }

    #[test]
    fn sphere_light_irradiance_matches_point() {
        // A sphere light of intensity I gives I / d^2 at distance d, the same as a point light.
        let l = Light::new("s", Shape::Point { pos: Vec3::new(0.0, 0.0, 10.0), radius: 1.0 }, Vec3::ONE * 100.0);
        let mut rng = crate::math::Pcg::new(9);
        let n = 20000;
        let mut e = 0.0;
        for _ in 0..n {
            if let Some(s) = l.sample(Vec3::ZERO, Vec3::Z, rng.next_2d()) {
                e += s.le.x * s.wi.z / s.pdf;
            }
        }
        let e = e / n as f32;
        assert!((e - 1.0).abs() < 0.01, "{e}");
    }

    #[test]
    fn ies_reads() {
        let text = "IESNA:LM-63-2002\nTILT=NONE\n1 1000 1 3 1 1 2 0 0 0\n1 1 100\n0 45 90\n0\n100 50 0\n";
        let ies = Ies::parse(text).unwrap();
        assert!((ies.at(Vec3::new(0.0, 0.0, -1.0)) - 1.0).abs() < 1e-4);
        assert!((ies.at(Vec3::new(1.0, 0.0, 0.0)) - 0.0).abs() < 1e-4);
    }
}
