//! The path tracer below the first hit: shadow rays through transparent things, light sampling with multiple
//! importance sampling, the random walk for subsurface scattering, fog, and paths of one sample per bounce.
//! The camera's first hit, where samples split by ray type, is in `first_hit.rs`.

use std::cell::Cell;

use glam::Vec3;

use crate::bsdf::{Bsdf, Category, ALL};
use crate::geom::{vis, SurfacePoint};
use crate::lights::Shape;
use crate::math::{luminance, offset, power_heuristic, Frame, Pcg, Ray, Sampler};
use crate::scene::Scene;
use crate::shade::{self, Resolved};

pub const MAX_GROUPS: usize = 8;
pub type Groups = [Vec3; MAX_GROUPS];

pub struct Tracer<'a> {
    pub s: &'a Scene,
    pub rays: Cell<u64>,
}

/// How deep a path is, per ray type, and what kind of bounce made the current ray.
#[derive(Clone, Copy, Debug, Default)]
pub struct Depth {
    pub total: u32,
    pub diffuse: u32,
    pub specular: u32,
    pub transmission: u32,
    pub volume: u32,
    pub transparency: u32,
    /// A diffuse bounce happened: caustics are cut after it unless a material asks for them.
    pub after_diffuse: bool,
}

/// What the previous vertex knew, for weighting light found by its sample.
#[derive(Clone, Copy, Debug)]
pub struct Prev {
    pub pdf: f32,
    pub delta: bool,
    /// Samples taken of the BSDF and of each light at that vertex: counts enter the MIS weights.
    pub bsdf_count: f32,
    pub first: bool,
    pub cat: Category,
    pub n: Vec3,
    /// The lights along the next segment were counted already.
    pub silent: bool,
}

/// A homogeneous medium a ray is inside: glass with depth, or fog.
#[derive(Clone, Copy, Debug)]
pub struct Medium {
    pub sigma_a: Vec3,
    pub sigma_s: Vec3,
    pub g: f32,
}

pub fn hg_sample(wo: Vec3, g: f32, u: glam::Vec2) -> Vec3 {
    let cos = if g.abs() < 1e-3 {
        1.0 - 2.0 * u.x
    } else {
        let s = (1.0 - g * g) / (1.0 - g + 2.0 * g * u.x);
        -(1.0 + g * g - s * s) / (2.0 * g)
    };
    let sin = (1.0 - cos * cos).max(0.0).sqrt();
    let phi = 2.0 * crate::math::PI * u.y;
    // wo points back along the incoming ray; scatter about the forward direction.
    Frame::from_n(-wo).to_world(Vec3::new(sin * phi.cos(), sin * phi.sin(), cos))
}

pub fn hg_pdf(cos: f32, g: f32) -> f32 {
    let d = 1.0 + g * g - 2.0 * g * cos;
    (1.0 - g * g) / (4.0 * crate::math::PI * d * d.max(1e-9).sqrt())
}

/// Clamps a colour's largest channel, keeping its hue.
pub fn clamp_max(c: Vec3, limit: f32) -> Vec3 {
    let m = c.max_element();
    if m > limit && m.is_finite() {
        c * (limit / m)
    } else if !m.is_finite() {
        Vec3::ZERO
    } else {
        c
    }
}

impl<'a> Tracer<'a> {
    pub fn new(s: &'a Scene) -> Tracer<'a> {
        Tracer { s, rays: Cell::new(0) }
    }

    fn count(&self) {
        self.rays.set(self.rays.get() + 1);
    }

    pub fn intersect(&self, ray: &Ray, tmax: f32, mask: u8) -> Option<crate::geom::Hit> {
        self.count();
        self.s.geo.intersect(ray, tmax, mask)
    }

    /// The fraction of light that gets from p along wi over dist: transparent surfaces and fog let some through.
    pub fn transmittance(&self, p: Vec3, wi: Vec3, dist: f32, time: f32, rng: &mut Pcg) -> Vec3 {
        if self.s.options.ignore_shadows {
            return Vec3::ONE;
        }
        self.count();
        let ray = Ray::new(p, wi, time);
        let tmax = if dist.is_finite() { dist * (1.0 - 1e-4) - 1e-4 } else { f32::INFINITY };
        let geo = &self.s.geo;
        let mut t = Vec3::ONE;
        geo.intersect_all(&ray, tmax, vis::SHADOW, |h| {
            let inst = &geo.instances[h.inst as usize];
            if inst.opaque {
                t = Vec3::ZERO;
                return false;
            }
            let sp = geo.surface(&ray, &h);
            let ctx = shade::ctx_of(self.s, &sp, -wi, 0.0, time);
            let o = shade::opacity(self.s, sp.shader, &ctx, rng);
            t *= Vec3::ONE - o;
            t.max_element() > 1e-4
        });
        if let (Some(a), false) = (self.s.atmosphere, self.s.options.ignore_atmosphere) {
            t *= (-Vec3::splat(a.density) * self.fog_length(&ray, tmax)).exp();
        }
        t
    }

    /// How much of a ray segment lies inside the scene's bounds, where fog is.
    pub fn fog_length(&self, ray: &Ray, tmax: f32) -> f32 {
        let b = self.s.bounds;
        if !b.valid() {
            return 0.0;
        }
        let inv = Vec3::ONE / ray.d;
        let t0 = (b.min - ray.o) * inv;
        let t1 = (b.max - ray.o) * inv;
        let tn = t0.min(t1).max_element().max(0.0);
        let tf = t0.max(t1).min_element().min(tmax);
        (tf - tn).max(0.0)
    }

    /// Direct light at a surface point from every light, split by category through the BSDF restricted to `mask`. At the
    /// first hit each category's samples are weighted against its own count and density; below it, against the
    /// mixture of the categories in `cont` the path goes on with, and not at all for those it can't take any more.
    #[allow(clippy::too_many_arguments)]
    pub fn direct(&self, sp: &SurfacePoint, bsdf: &Bsdf, wo: Vec3, mask: u8, cont: u8, bsdf_counts: &[f32; 6], first: bool, depth_total: u32, time: f32, sampler: &mut Sampler, groups: &mut Groups) -> [Vec3; 6] {
        let mut out = [Vec3::ZERO; 6];
        if self.s.options.ignore_lights {
            return out;
        }
        let inst = &self.s.geo.instances[sp.inst as usize];
        let back = bsdf.has(Category::Transmission.bit() | Category::Sss.bit()) || bsdf.thin_walled;
        for (li, light) in self.s.lights.iter().enumerate() {
            if li < 64 && inst.light_mask & (1 << li) == 0 {
                continue;
            }
            if depth_total > light.max_bounces {
                continue;
            }
            let m = if first { (light.samples * light.samples).max(1) } else { 1 };
            let inv_m = 1.0 / m as f32;
            for k in 0..m {
                let u = sampler.next_2d();
                let u2 = sampler.rng().next_f32();
                let ls = if matches!(light.shape, Shape::Mesh { .. }) { light.sample_mesh(&self.s.geo, sp.p, time, u, u2) } else { light.sample(sp.p, sp.n, u) };
                let _ = k;
                let Some(ls) = ls else { continue };
                if ls.pdf <= 0.0 {
                    continue;
                }
                // Light from behind the surface only reaches a material that lets it through.
                if !back && ls.wi.dot(sp.ng) <= 0.0 {
                    continue;
                }
                let mut le = ls.le;
                if let Some((mi, prim, b1, b2)) = ls.mesh {
                    // A mesh light's radiance is its own colour times the surface's emission.
                    le *= self.mesh_emission(mi, prim, b1, b2, ls.wi, time, sampler.rng());
                }
                if le.max_element() <= 0.0 {
                    continue;
                }
                let e = bsdf.eval(wo, ls.wi, mask);
                let pb_mix = if first || ls.delta || cont == 0 {
                    0.0
                } else if cont == mask {
                    e.pdf
                } else {
                    bsdf.eval(wo, ls.wi, cont).pdf
                };
                let mut any = Vec3::ZERO;
                let mut contrib = [Vec3::ZERO; 6];
                for (c, f) in e.f.iter().enumerate() {
                    if f.max_element() <= 0.0 {
                        continue;
                    }
                    let cat = crate::bsdf::CATEGORIES[c];
                    let lw = light.weight_for(cat);
                    if lw <= 0.0 {
                        continue;
                    }
                    let w = if ls.delta {
                        1.0
                    } else if first {
                        power_heuristic(m as f32 * ls.pdf, bsdf_counts[c] * e.pdf_cat[c])
                    } else {
                        let pb = if cont & cat.bit() != 0 { pb_mix } else { 0.0 };
                        power_heuristic(ls.pdf, pb)
                    };
                    contrib[c] = *f * le * (w * lw * inv_m / ls.pdf);
                    any += contrib[c];
                }
                if any.max_element() <= 0.0 {
                    continue;
                }
                // Low light: not worth a shadow ray.
                if luminance(any) < self.s.options.low_light_threshold {
                    continue;
                }
                let tr = if ls.unshadowed || !light.cast_shadows || !inst.receive_shadows {
                    Vec3::ONE
                } else {
                    let from = offset(sp.p, sp.ng, ls.wi.dot(sp.ng) > 0.0);
                    let t = self.transmittance(from, ls.wi, ls.dist, time, sampler.rng());
                    // Shadow colour and density: what's blocked comes back tinted.
                    let blocked = (Vec3::ONE - t) * light.shadow_density;
                    Vec3::ONE - blocked + blocked * light.shadow_color
                };
                for c in 0..6 {
                    let v = contrib[c] * tr;
                    out[c] += v;
                    groups[light.group as usize % MAX_GROUPS] += v;
                }
            }
        }
        out
    }

    /// The emission of a mesh light's surface at a point: the material's emission, or white for a plain mesh light.
    #[allow(clippy::too_many_arguments)]
    fn mesh_emission(&self, inst: u32, prim: u32, b1: f32, b2: f32, wi: Vec3, time: f32, rng: &mut Pcg) -> Vec3 {
        let geo = &self.s.geo;
        let ins = &geo.instances[inst as usize];
        let mesh = &geo.meshes[ins.mesh as usize];
        let (m, _) = ins.xform_at(time);
        let (a, b, c) = mesh.tri_points(prim, time);
        let p = m.transform_point3(a * (1.0 - b1 - b2) + b * b1 + c * b2);
        // A ray from the light point back towards p finds the same triangle: rebuild its surface point that way.
        let o = p - wi * 1e-3;
        let ray = Ray::new(o - wi * 1e-3, wi, time);
        let h = crate::geom::Hit { t: 2e-3, inst, prim, u: b1, v: b2 };
        let sp = geo.surface(&ray, &h);
        let ctx = shade::ctx_of(self.s, &sp, -wi, 0.0, time);
        match shade::resolve(self.s, sp.shader, &ctx, rng) {
            (Resolved::Standard(s), w) => {
                let net = &self.s.network;
                net.eval(s.emission_color, &ctx, &self.s.textures) * net.eval_f(s.emission, &ctx, &self.s.textures) * w
            }
            (Resolved::Flat { color, .. }, w) => color * w,
            _ => Vec3::ZERO,
        }
    }

    /// Light from area lights and the dome along a ray that met nothing nearer than `t`, weighted against the light
    /// sampling the previous vertex did.
    pub fn lights_along(&self, ray: &Ray, t: f32, prev: &Prev, depth_total: u32, camera: bool, groups: &mut Groups, throughput: Vec3) -> Vec3 {
        let mut sum = Vec3::ZERO;
        if prev.silent {
            return sum;
        }
        for light in &self.s.lights {
            if depth_total > light.max_bounces {
                continue;
            }
            let lw = if camera { light.camera } else { light.weight_for(prev.cat) };
            if lw <= 0.0 {
                continue;
            }
            let found = if light.is_dome() {
                if t.is_finite() {
                    None
                } else {
                    light.dome_eval(ray.d, prev.n).map(|(le, pdf)| (le, pdf))
                }
            } else {
                light.intersect(ray, t).map(|(_, le, pdf)| (le, pdf))
            };
            let Some((le, pdf)) = found else { continue };
            let m = if prev.first { (light.samples * light.samples).max(1) as f32 } else { 1.0 };
            let w = if camera || prev.delta { 1.0 } else { power_heuristic(prev.bsdf_count * prev.pdf, m * pdf) };
            let v = throughput * le * (w * lw);
            sum += v;
            groups[light.group as usize % MAX_GROUPS] += v;
        }
        sum
    }

    /// Chiang et al.'s mapping from the colour a deep slab should show to a single scattering albedo.
    pub fn sss_albedo(a: f32) -> f32 {
        let a = a.clamp(0.0, 0.999);
        let x = 4.09712 + 4.20863 * a - (9.59217 + 41.6808 * a + 17.7126 * a * a).sqrt();
        (1.0 - x * x).clamp(0.0, 0.9999)
    }

    /// A random walk inside an object from where a ray went in, until it finds the surface again. The exit point, and
    /// the walk's weight. Chromatic: one channel picks each distance, the three are weighted together.
    pub fn walk(&self, sp: &SurfacePoint, wi: Vec3, color: Vec3, radius: Vec3, g: f32, time: f32, sampler: &mut Sampler) -> Option<(SurfacePoint, Vec3, Vec3)> {
        let albedo = Vec3::new(Self::sss_albedo(color.x), Self::sss_albedo(color.y), Self::sss_albedo(color.z));
        let s = |a: f32| 1.9 - a + 3.5 * (a - 0.8) * (a - 0.8);
        let sigma_t = Vec3::new(1.0 / (radius.x * s(color.x)).max(1e-5), 1.0 / (radius.y * s(color.y)).max(1e-5), 1.0 / (radius.z * s(color.z)).max(1e-5));
        let sigma_s = sigma_t * albedo;
        let mut p = offset(sp.p, sp.ng, false);
        let mut d = wi;
        let mut w = Vec3::ONE;
        for _ in 0..256 {
            let rng = sampler.rng();
            let ch = (rng.next_f32() * 3.0).min(2.999) as usize;
            let st = sigma_t[ch];
            let dist = -(1.0 - rng.next_f32()).ln() / st;
            let ray = Ray::new(p, d, time);
            self.count();
            let hit = self.s.geo.intersect(&ray, dist, vis::ALL).filter(|h| h.inst == sp.inst);
            match hit {
                Some(h) => {
                    let tr = (-sigma_t * h.t).exp();
                    let pdf = (tr.x + tr.y + tr.z) / 3.0;
                    w *= tr / pdf.max(1e-12);
                    let mut exit = self.s.geo.surface(&ray, &h);
                    // Met from inside: the exit faces out.
                    exit.ng = -exit.ng;
                    exit.n = -exit.n;
                    exit.front = true;
                    return Some((exit, w, d));
                }
                None => {
                    let tr = (-sigma_t * dist).exp();
                    let pdf = (sigma_t * tr).element_sum() / 3.0;
                    w *= sigma_s * tr / pdf.max(1e-12);
                    p += d * dist;
                    d = hg_sample(-d, g, rng.next_2d());
                    // Russian roulette once the walk is mostly absorbed.
                    let m = w.max_element();
                    if m < 0.2 {
                        if rng.next_f32() > m * 5.0 {
                            return None;
                        }
                        w /= m * 5.0;
                    }
                }
            }
        }
        None
    }

    /// A bsdf for a resolved material at a point, with what the path needs to know beside it.
    pub fn bsdf_at(&self, sp: &SurfacePoint, mat: &Resolved, wo: Vec3, time: f32, ao: f32, footprint: f32) -> Option<(Bsdf, crate::shader::Params)> {
        let Resolved::Standard(s) = mat else { return None };
        let ctx = shade::ctx_of(self.s, sp, wo, footprint, time);
        let mut p = shade::params(self.s, s, &ctx, ao);
        // Bent normals stay on the side of the geometric normal the ray came from.
        if p.n.dot(sp.ng) < 0.0 {
            p.n = (p.n - sp.ng * (p.n.dot(sp.ng) - 1e-3)).normalize();
        }
        let b = Bsdf::standard(&p, wo, sp.ng, sp.front || p.thin_walled);
        Some((b, p))
    }

    /// Ambient occlusion at a point: the share of short rays that get away.
    pub fn occlusion(&self, sp: &SurfacePoint, samples: u32, distance: f32, time: f32, sampler: &mut Sampler) -> f32 {
        let n = samples.max(1);
        let f = Frame::from_n(sp.n);
        let mut open = 0;
        for _ in 0..n {
            let d = f.to_world(crate::math::cosine_hemisphere(sampler.next_2d()));
            let r = Ray::new(offset(sp.p, sp.ng, true), d, time);
            if self.intersect(&r, distance, vis::SHADOW).is_none() {
                open += 1;
            }
        }
        open as f32 / n as f32
    }

    pub fn depth_allows(&self, d: &Depth, cat: Category) -> bool {
        let o = &self.s.options;
        if d.total >= o.total_depth {
            return false;
        }
        match cat {
            Category::Diffuse | Category::Sheen | Category::Sss => d.diffuse < o.diffuse_depth,
            Category::Specular | Category::Coat => d.specular < o.specular_depth,
            Category::Transmission => d.transmission < o.transmission_depth,
        }
    }

    pub fn bump(d: &mut Depth, cat: Category) -> u8 {
        d.total += 1;
        match cat {
            Category::Diffuse | Category::Sheen | Category::Sss => {
                d.diffuse += 1;
                d.after_diffuse = true;
                vis::DIFFUSE_REFLECT | vis::DIFFUSE_TRANSMIT
            }
            Category::Specular | Category::Coat => {
                d.specular += 1;
                vis::SPECULAR_REFLECT
            }
            Category::Transmission => {
                d.transmission += 1;
                vis::SPECULAR_TRANSMIT
            }
        }
    }

    /// Mask of lobe categories a path may still take.
    pub fn allowed_mask(&self, d: &Depth, caustics: bool) -> u8 {
        let mut m = 0;
        for c in crate::bsdf::CATEGORIES {
            if self.depth_allows(d, c) {
                m |= c.bit();
            }
        }
        // Specular after diffuse are caustics: off unless the material asks.
        if d.after_diffuse && !caustics {
            m &= !(Category::Specular.bit() | Category::Coat.bit() | Category::Transmission.bit());
        }
        m & ALL
    }
}
