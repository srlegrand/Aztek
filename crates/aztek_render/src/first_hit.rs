//! One camera sample: the ray from the lens, transparency on the way in, and at the first surface the split Arnold
//! makes, a number of samples per ray type (diffuse, specular, transmission, subsurface), each squared, every light
//! sampled its own squared count. Results are split into the light AOVs by lobe and by direct or indirect.

use glam::{Vec2, Vec3};

use crate::bsdf::{Category, CATEGORIES};
use crate::geom::vis;
use crate::integrator::{clamp_max, Depth, Groups, Prev, Tracer, MAX_GROUPS};
use crate::math::{offset, Ray, Sampler};
use crate::shade::{self, Resolved};

/// Light AOV slots: direct and indirect for each lobe category, then the rest.
pub const DIRECT: usize = 0;
pub const INDIRECT: usize = 6;
pub const EMISSION: usize = 12;
pub const BACKGROUND: usize = 13;
pub const ALBEDO: usize = 14;
pub const VOLUME: usize = 15;
pub const SHADOW_MATTE: usize = 16;
pub const SLOTS: usize = 17;

#[derive(Clone, Copy, Debug)]
pub struct SampleOut {
    pub color: Vec3,
    pub alpha: f32,
    pub slots: [Vec3; SLOTS],
    pub groups: Groups,
    /// Data at the first opaque hit; z infinite where nothing was hit.
    pub z: f32,
    pub n: Vec3,
    pub p: Vec3,
    pub object: u32,
    pub material: u32,
}

impl Default for SampleOut {
    fn default() -> Self {
        SampleOut { color: Vec3::ZERO, alpha: 0.0, slots: [Vec3::ZERO; SLOTS], groups: [Vec3::ZERO; MAX_GROUPS], z: f32::INFINITY, n: Vec3::ZERO, p: Vec3::ZERO, object: 0, material: 0 }
    }
}

impl Tracer<'_> {
    fn samples_for(&self, c: Category) -> u32 {
        let o = &self.s.options;
        match c {
            Category::Diffuse => o.diffuse_samples,
            Category::Specular | Category::Coat | Category::Sheen => o.specular_samples,
            Category::Transmission => o.transmission_samples,
            Category::Sss => o.sss_samples,
        }
    }

    /// A camera sample through image point `ndc` (0 to 1 from the top left).
    pub fn camera_sample(&self, ndc: Vec2, sampler: &mut Sampler) -> SampleOut {
        let s = self.s;
        let o = &s.options;
        let lens = sampler.next_2d();
        let time = if o.ignore_motion_blur { 0.5 } else { sampler.next_1d() };
        let cam = &s.camera;
        let lens = if o.ignore_dof || !cam.dof { glam::Vec2::splat(0.5) } else { lens };
        let mut ray = cam.ray(ndc, s.aspect(), lens, time);
        let pixel_angle = cam.pixel_angle(o.width, s.aspect());
        let mut out = SampleOut::default();
        let mut throughput = Vec3::ONE;
        for _ in 0..=o.transparency_depth {
            let hit = self.intersect(&ray, f32::INFINITY, vis::CAMERA);
            let t = hit.map_or(f32::INFINITY, |h| h.t);
            // Fog on the way to the first surface.
            if s.atmosphere.is_some() && !o.ignore_atmosphere {
                let prev = Prev { pdf: 1.0, delta: true, bsdf_count: 1.0, first: false, cat: Category::Diffuse, n: ray.d, silent: false };
                let mut g = [Vec3::ZERO; MAX_GROUPS];
                let a = s.atmosphere.unwrap();
                let len = self.fog_length(&ray, t);
                if len > 0.0 {
                    let _ = prev;
                    let tr = (-a.density * len).exp();
                    let fog = self.fog_single(&ray, t, sampler, &mut g);
                    out.slots[VOLUME] += throughput * fog;
                    out.color += throughput * fog;
                    for (x, y) in out.groups.iter_mut().zip(g.iter()) {
                        *x += throughput * *y;
                    }
                    throughput *= tr;
                }
            }
            let Some(h) = hit else {
                // The background: domes seen by the camera, alpha stays clear.
                let mut g = [Vec3::ZERO; MAX_GROUPS];
                let prev = Prev { pdf: 1.0, delta: true, bsdf_count: 1.0, first: false, cat: Category::Diffuse, n: ray.d, silent: false };
                let mut bg = self.lights_along(&ray, f32::INFINITY, &prev, 0, true, &mut g, throughput);
                if !s.lights.iter().any(|l| l.is_dome() && l.camera > 0.0) {
                    bg += throughput * o.background;
                }
                out.slots[BACKGROUND] += bg;
                out.color += bg;
                for (x, y) in out.groups.iter_mut().zip(g.iter()) {
                    *x += *y;
                }
                break;
            };
            let sp = s.geo.surface(&ray, &h);
            let inst = &s.geo.instances[h.inst as usize];
            let wo = -ray.d;
            let footprint = (pixel_angle * h.t + cam.pixel_width(o.width)) * sp.uv_density / wo.dot(sp.n).abs().max(0.2);
            let ctx = shade::ctx_of(s, &sp, wo, footprint, time);
            if out.z.is_infinite() {
                out.z = h.t;
                out.n = sp.n;
                out.p = sp.p;
                out.object = inst.id;
                out.material = sp.shader;
            }
            if inst.matte {
                // A hold out: blocks, shows nothing, clears alpha.
                out.alpha = 0.0;
                return out;
            }
            let (mat, mw) = shade::resolve(s, sp.shader, &ctx, sampler.rng());
            let opacity = shade::opacity(s, sp.shader, &ctx, sampler.rng());
            if opacity.min_element() < 1.0 {
                let q = 1.0 - (opacity.x + opacity.y + opacity.z) / 3.0;
                if sampler.rng().next_f32() < q {
                    throughput *= (Vec3::ONE - opacity) / q.max(1e-6);
                    ray = Ray::new(offset(sp.p, sp.ng, false), ray.d, ray.time);
                    continue;
                }
                throughput *= opacity / (1.0 - q).max(1e-6);
            }
            throughput *= mw;
            out.alpha = 1.0;
            match &mat {
                Resolved::Flat { color, .. } => {
                    let v = throughput * *color;
                    out.slots[EMISSION] += v;
                    out.color += v;
                    out.groups[0] += v;
                }
                Resolved::ShadowMatte { shadow_color, background } => {
                    let (lit, open) = self.shadow_fraction(&sp, time, sampler);
                    let shadow = if open > 0.0 { (1.0 - lit / open).clamp(0.0, 1.0) } else { 0.0 };
                    out.slots[SHADOW_MATTE] += Vec3::splat(shadow);
                    out.color += throughput * *shadow_color * shadow;
                    if *background {
                        let mut g = [Vec3::ZERO; MAX_GROUPS];
                        let prev = Prev { pdf: 1.0, delta: true, bsdf_count: 1.0, first: false, cat: Category::Diffuse, n: ray.d, silent: false };
                        out.color += self.lights_along(&Ray::new(ray.o, ray.d, ray.time), f32::INFINITY, &prev, 0, true, &mut g, throughput) * (1.0 - shadow);
                    }
                    out.alpha = shadow;
                }
                Resolved::Standard(_) => {
                    let mut ao = 1.0;
                    if let Some(tr) = s.traced.get(sp.shader as usize).copied().flatten() {
                        ao = self.occlusion(&sp, tr.samples, tr.distance, time, sampler);
                    }
                    if let Some((bsdf, params)) = self.bsdf_at(&sp, &mat, wo, time, ao, footprint) {
                        self.shade_first(&sp, &bsdf, &params, wo, time, throughput, sampler, &mut out);
                    }
                }
            }
            break;
        }
        if let Some(c) = o.aa_clamp {
            out.color = clamp_max(out.color, c);
            if o.clamp_aovs {
                for s in &mut out.slots {
                    *s = clamp_max(*s, c);
                }
            }
        }
        out
    }

    /// Single scattering in fog along a camera ray segment: one point, by its density, lit directly.
    fn fog_single(&self, ray: &Ray, t: f32, sampler: &mut Sampler, g: &mut Groups) -> Vec3 {
        let Some(a) = self.s.atmosphere else { return Vec3::ZERO };
        let len = self.fog_length(ray, t);
        if len <= 0.0 || a.density <= 0.0 {
            return Vec3::ZERO;
        }
        let b = self.s.bounds;
        let inv = Vec3::ONE / ray.d;
        let tn = ((b.min - ray.o) * inv).min((b.max - ray.o) * inv).max_element().max(0.0);
        // Distance by the density, limited to the segment: weight is the chance of scattering in it.
        let p_scatter = 1.0 - (-a.density * len).exp();
        let u = sampler.rng().next_f32();
        let d = -(1.0 - u * p_scatter).ln() / a.density;
        let p = ray.at(tn + d);
        let mut sum = Vec3::ZERO;
        let samples = self.s.options.volume_samples.max(1);
        let n = samples * samples;
        for _ in 0..n {
            for light in &self.s.lights {
                if light.volume <= 0.0 {
                    continue;
                }
                let Some(ls) = light.sample(p, -ray.d, sampler.next_2d()) else { continue };
                let ph = crate::integrator::hg_pdf(ray.d.dot(ls.wi), a.anisotropy);
                let w = if ls.delta { 1.0 } else { crate::math::power_heuristic(ls.pdf, ph) };
                let tr = if light.cast_shadows { self.transmittance(p, ls.wi, ls.dist, ray.time, sampler.rng()) } else { Vec3::ONE };
                let v = a.color * ls.le * tr * (ph * w * light.volume / ls.pdf) * p_scatter / n as f32;
                sum += v;
                g[light.group as usize % MAX_GROUPS] += v;
            }
        }
        sum
    }

    /// For a shadow matte: direct light that arrives, and what would arrive with nothing in the way.
    fn shadow_fraction(&self, sp: &crate::geom::SurfacePoint, time: f32, sampler: &mut Sampler) -> (f32, f32) {
        let (mut lit, mut open) = (0.0, 0.0);
        for light in &self.s.lights {
            if !light.cast_shadows {
                continue;
            }
            let Some(ls) = light.sample(sp.p, sp.n, sampler.next_2d()) else { continue };
            let cos = ls.wi.dot(sp.n).max(0.0);
            let e = crate::math::luminance(ls.le) * cos / ls.pdf;
            open += e;
            let from = offset(sp.p, sp.ng, true);
            lit += e * crate::math::luminance(self.transmittance(from, ls.wi, ls.dist, time, sampler.rng()));
        }
        (lit, open)
    }

    /// The first hit's lighting, split by category.
    #[allow(clippy::too_many_arguments)]
    fn shade_first(&self, sp: &crate::geom::SurfacePoint, bsdf: &crate::bsdf::Bsdf, params: &crate::shader::Params, wo: Vec3, time: f32, throughput: Vec3, sampler: &mut Sampler, out: &mut SampleOut) {
        let s = self.s;
        let o = &s.options;
        out.slots[ALBEDO] += bsdf.albedo;
        let em = throughput * bsdf.emission;
        out.slots[EMISSION] += em;
        out.color += em;
        out.groups[0] += em;
        let present = bsdf.mask();
        let mut counts = [0.0f32; 6];
        for c in CATEGORIES {
            let n = self.samples_for(c);
            counts[c as usize] = (n * n) as f32;
        }
        // Categories with zero samples are off, Arnold's way of turning a ray type off.
        let mut mask = 0u8;
        for c in CATEGORIES {
            if present & c.bit() != 0 && counts[c as usize] > 0.0 {
                mask |= c.bit();
            }
        }
        // Direct light, everything but subsurface which is lit where its walks come out.
        let mut g = [Vec3::ZERO; MAX_GROUPS];
        let direct = self.direct(sp, bsdf, wo, mask & !Category::Sss.bit(), 0, &counts, true, 0, time, sampler, &mut g);
        for c in 0..6 {
            let v = throughput * direct[c];
            out.slots[DIRECT + c] += v;
            out.color += v;
        }
        for (x, y) in out.groups.iter_mut().zip(g.iter()) {
            *x += throughput * *y;
        }
        // Indirect, and direct found by the BSDF's own samples: per category, its square of samples.
        for c in CATEGORIES {
            if mask & c.bit() == 0 {
                continue;
            }
            let ci = c as usize;
            let n = counts[ci] as u32;
            let inv = 1.0 / n as f32;
            for k in 0..n {
                let mut sub = sampler.split(k, n, ci as u32 + 1);
                let Some(smp) = bsdf.sample(wo, c.bit(), sub.next_1d(), sub.next_2d()) else { continue };
                let mut depth = Depth::default();
                if !self.depth_allows(&depth, c) {
                    // Depth 0 for this type: direct light only, as Arnold does with a depth of 0.
                    if c != Category::Sss {
                        let mut g2 = [Vec3::ZERO; MAX_GROUPS];
                        let ray = Ray::new(offset(sp.p, sp.ng, !smp.transmit), smp.wi, time);
                        let hit_t = self.intersect(&ray, f32::INFINITY, vis::ALL).map_or(f32::INFINITY, |h| h.t);
                        let prev = Prev { pdf: smp.pdf, delta: smp.delta, bsdf_count: counts[ci], first: true, cat: c, n: sp.n, silent: false };
                        let v = self.lights_along(&ray, hit_t, &prev, 1, false, &mut g2, throughput * smp.weight * inv);
                        out.slots[DIRECT + ci] += v;
                        out.color += v;
                        for (x, y) in out.groups.iter_mut().zip(g2.iter()) {
                            *x += *y;
                        }
                    }
                    continue;
                }
                let mask_vis = Self::bump(&mut depth, c);
                let mut w = throughput * smp.weight * inv;
                let mut g2 = [Vec3::ZERO; MAX_GROUPS];
                let (ray, prev, medium) = if c == Category::Sss {
                    let Some((exit, ww, _)) = self.sss_walk(sp, params, smp.wi, time, &mut sub) else { continue };
                    w *= ww;
                    // Light where the walk comes out: a diffuse exit lit by every light, then onward.
                    let lam = crate::bsdf::Bsdf::lambert(Vec3::ONE, exit.ng, exit.ng);
                    let d = self.direct(&exit, &lam, exit.ng, Category::Diffuse.bit(), Category::Diffuse.bit(), &[1.0; 6], false, 1, time, &mut sub, &mut g2);
                    let v = w * d[0];
                    out.slots[DIRECT + ci] += v;
                    out.color += v;
                    let Some(s2) = lam.sample(exit.ng, Category::Diffuse.bit(), 0.0, sub.next_2d()) else {
                        for (x, y) in out.groups.iter_mut().zip(g2.iter()) {
                            *x += w * *y;
                        }
                        continue;
                    };
                    for (x, y) in out.groups.iter_mut().zip(g2.iter()) {
                        *x += w * *y;
                    }
                    g2 = [Vec3::ZERO; MAX_GROUPS];
                    w *= s2.weight;
                    (Ray::new(offset(exit.p, exit.ng, true), s2.wi, time), Prev { pdf: s2.pdf, delta: false, bsdf_count: 1.0, first: false, cat: Category::Diffuse, n: exit.ng, silent: false }, None)
                } else {
                    let medium = if smp.transmit && c == Category::Transmission && !params.thin_walled && sp.front && params.transmission_depth > 0.0 { Some(crate::path::transmission_medium(params)) } else { None };
                    (Ray::new(offset(sp.p, sp.ng, !smp.transmit), smp.wi, time), Prev { pdf: smp.pdf, delta: smp.delta, bsdf_count: counts[ci], first: true, cat: c, n: sp.n, silent: false }, medium)
                };
                // The first segment's light (lights met directly) is direct; the rest indirect.
                let hit = self.intersect(&ray, f32::INFINITY, mask_vis);
                let t = hit.map_or(f32::INFINITY, |h| h.t);
                let mut g_direct = [Vec3::ZERO; MAX_GROUPS];
                let direct_v = if medium.is_none() { self.lights_along(&ray, t, &prev, 1, false, &mut g_direct, w) } else { Vec3::ZERO };
                out.slots[DIRECT + ci] += direct_v;
                out.color += direct_v;
                for (x, y) in out.groups.iter_mut().zip(g_direct.iter()) {
                    *x += *y;
                }
                if hit.is_none() && medium.is_none() {
                    continue;
                }
                // The rest of the path, without the lights already counted on its first segment.
                let l = self.li_after(ray, hit, depth, prev, medium, mask_vis, &mut sub, &mut g2);
                let raw = w * l;
                let v = clamp_max(raw, o.indirect_clamp);
                let scale = if raw.max_element() > 0.0 { v.max_element() / raw.max_element() } else { 1.0 };
                out.slots[INDIRECT + ci] += v;
                out.color += v;
                for (x, y) in out.groups.iter_mut().zip(g2.iter()) {
                    *x += w * *y * scale;
                }
            }
        }
    }

    /// The rest of a path whose first segment's lights were counted already: those are skipped, the rest traced.
    #[allow(clippy::too_many_arguments)]
    fn li_after(&self, ray: Ray, _hit: Option<crate::geom::Hit>, depth: Depth, prev: Prev, medium: Option<crate::integrator::Medium>, mask_vis: u8, sampler: &mut Sampler, groups: &mut Groups) -> Vec3 {
        self.li(ray, depth, Prev { silent: true, ..prev }, Vec3::ONE, medium, mask_vis, sampler, groups)
    }
}
