//! A path of one sample per bounce, from a ray leaving a surface: what arrives back along it.

use glam::Vec3;

use crate::bsdf::{Bsdf, Category};
use crate::geom::vis;
use crate::integrator::{clamp_max, hg_pdf, hg_sample, Depth, Groups, Medium, Prev, Tracer};
use crate::math::{luminance, offset, power_heuristic, Ray, Sampler};
use crate::shade::{self, Resolved};

impl Tracer<'_> {
    /// Fog: a scatter point on the segment, or None to go on to the surface. Weighted so the estimate stays unbiased.
    fn fog_event(&self, ray: &Ray, t: f32, sampler: &mut Sampler) -> Option<(f32, Medium)> {
        let a = self.s.atmosphere?;
        if self.s.options.ignore_atmosphere || a.density <= 0.0 {
            return None;
        }
        let len = self.fog_length(ray, t);
        if len <= 0.0 {
            return None;
        }
        let d = -(1.0 - sampler.rng().next_f32()).ln() / a.density;
        // Where the ray enters the bounds.
        let b = self.s.bounds;
        let inv = Vec3::ONE / ray.d;
        let t0 = (b.min - ray.o) * inv;
        let t1 = (b.max - ray.o) * inv;
        let tn = t0.min(t1).max_element().max(0.0);
        (d < len).then_some((tn + d, Medium { sigma_a: Vec3::ZERO, sigma_s: a.color * a.density, g: a.anisotropy }))
    }

    /// Light at a point in fog from every light, through the phase function.
    fn fog_direct(&self, p: Vec3, wo: Vec3, m: &Medium, time: f32, sampler: &mut Sampler, groups: &mut Groups) -> Vec3 {
        let mut sum = Vec3::ZERO;
        for light in &self.s.lights {
            if light.volume <= 0.0 {
                continue;
            }
            let Some(ls) = light.sample(p, -wo, sampler.next_2d()) else { continue };
            let ph = hg_pdf((-wo).dot(ls.wi), m.g);
            let w = if ls.delta { 1.0 } else { power_heuristic(ls.pdf, ph) };
            let tr = if light.cast_shadows { self.transmittance(p, ls.wi, ls.dist, time, sampler.rng()) } else { Vec3::ONE };
            let v = ls.le * tr * (ph * w * light.volume / ls.pdf);
            sum += v;
            groups[light.group as usize % crate::integrator::MAX_GROUPS] += v;
        }
        sum
    }

    /// Radiance arriving back along `ray`, for a path already `depth` deep, weighted by `throughput` on the way.
    #[allow(clippy::too_many_arguments)]
    pub fn li(&self, mut ray: Ray, mut depth: Depth, mut prev: Prev, mut throughput: Vec3, mut medium: Option<Medium>, mut mask_vis: u8, sampler: &mut Sampler, groups: &mut Groups) -> Vec3 {
        let mut sum = Vec3::ZERO;
        let s = self.s;
        let o = &s.options;
        let mut caustics = false;
        for _ in 0..256 {
            let hit = self.intersect(&ray, f32::INFINITY, mask_vis);
            let t = hit.map_or(f32::INFINITY, |h| h.t);
            // Inside glass with depth: absorbed along the way, maybe scattered.
            if let Some(m) = medium {
                let st = m.sigma_a + m.sigma_s;
                let sm = st.element_sum() / 3.0;
                if m.sigma_s.max_element() > 0.0 && depth.volume < o.volume_depth.max(4) {
                    let d = -(1.0 - sampler.rng().next_f32()).ln() / sm.max(1e-9);
                    if d < t {
                        let tr = (-st * d).exp();
                        let pdf = sm * (-sm * d).exp();
                        throughput *= m.sigma_s * tr / pdf.max(1e-12);
                        let p = ray.at(d);
                        ray = Ray::new(p, hg_sample(-ray.d, m.g, sampler.next_2d()), ray.time);
                        depth.volume += 1;
                        prev.delta = true;
                        continue;
                    }
                    let tr = (-st * t.min(1e6)).exp();
                    throughput *= tr / (-sm * t.min(1e6)).exp().max(1e-12);
                } else {
                    throughput *= (-m.sigma_a * t.min(1e6)).exp();
                }
            } else if let Some((d, fm)) = self.fog_event(&ray, t, sampler) {
                // Scattered in fog: light it, and carry on in a new direction.
                let p = ray.at(d);
                let a = s.atmosphere.unwrap();
                throughput *= a.color;
                sum += clamp_max(throughput * self.fog_direct(p, -ray.d, &fm, ray.time, sampler, groups), o.indirect_clamp);
                if depth.volume >= o.volume_depth {
                    break;
                }
                depth.volume += 1;
                depth.total += 1;
                let d2 = hg_sample(-ray.d, fm.g, sampler.next_2d());
                prev = Prev { pdf: hg_pdf(ray.d.dot(d2), fm.g), delta: false, bsdf_count: 1.0, first: false, cat: Category::Diffuse, n: d2, silent: false };
                ray = Ray::new(p, d2, ray.time);
                mask_vis = vis::VOLUME;
                continue;
            }
            sum += self.lights_along(&ray, t, &prev, depth.total, false, groups, throughput);
            prev.silent = false;
            let Some(h) = hit else { break };
            let sp = s.geo.surface(&ray, &h);
            let wo = -ray.d;
            let inst = &s.geo.instances[h.inst as usize];
            let footprint = 0.0;
            let ctx = shade::ctx_of(s, &sp, wo, footprint, ray.time);
            let (mat, mw) = shade::resolve(s, sp.shader, &ctx, sampler.rng());
            throughput *= mw;
            // Matte objects block and show nothing.
            if inst.matte {
                break;
            }
            // Transparency: pass straight through by chance.
            let opacity = shade::opacity(s, sp.shader, &ctx, sampler.rng());
            if opacity.min_element() < 1.0 {
                let q = 1.0 - (opacity.x + opacity.y + opacity.z) / 3.0;
                if sampler.rng().next_f32() < q {
                    if depth.transparency >= o.transparency_depth {
                        break;
                    }
                    depth.transparency += 1;
                    throughput *= (Vec3::ONE - opacity) / q.max(1e-6);
                    ray = Ray::new(offset(sp.p, sp.ng, false), ray.d, ray.time);
                    continue;
                }
                throughput *= opacity / (1.0 - q).max(1e-6);
            }
            let mut ao = 1.0;
            if let Some(tr) = s.traced.get(sp.shader as usize).copied().flatten() {
                ao = self.occlusion(&sp, tr.samples.min(4), tr.distance, ray.time, sampler);
            }
            let (bsdf, params) = match &mat {
                Resolved::Flat { color, .. } => {
                    sum += throughput * *color;
                    break;
                }
                Resolved::ShadowMatte { .. } => {
                    ray = Ray::new(offset(sp.p, sp.ng, false), ray.d, ray.time);
                    continue;
                }
                Resolved::Standard(st) => {
                    caustics |= st.caustics;
                    match self.bsdf_at(&sp, &mat, wo, ray.time, ao, footprint) {
                        Some(x) => x,
                        None => break,
                    }
                }
            };
            // Emission met by a BSDF sample: weighted against sampling it as a mesh light.
            if bsdf.emission.max_element() > 0.0 {
                let w = match inst.light {
                    Some(li) if !prev.delta => {
                        let l = &s.lights[li as usize];
                        let cos_l = wo.dot(sp.ng).abs();
                        power_heuristic(prev.bsdf_count * prev.pdf, l.pdf_area(h.t, cos_l))
                    }
                    _ => 1.0,
                };
                let v = throughput * bsdf.emission * w;
                sum += v;
                groups[0] += v;
            }
            let mask = self.allowed_mask(&depth, caustics);
            let mut bsdf = bsdf;
            let counts = [1.0f32; 6];
            // Lit by every lobe, whatever depth is left; after a diffuse bounce specular light makes caustics, off
            // unless asked for.
            let spec = Category::Specular.bit() | Category::Coat.bit() | Category::Transmission.bit();
            let lit = bsdf.mask() & !Category::Sss.bit() & if depth.after_diffuse && !caustics { !spec } else { 0xff };
            let cont = bsdf.mask() & mask;
            if lit != 0 && !bsdf.is_delta(lit) {
                let mut g2 = [Vec3::ZERO; crate::integrator::MAX_GROUPS];
                let d = self.direct(&sp, &bsdf, wo, lit, cont & !Category::Sss.bit(), &counts, false, depth.total, ray.time, sampler, &mut g2);
                let dsum: Vec3 = d.iter().copied().sum();
                let c = clamp_max(throughput * dsum, o.indirect_clamp);
                let scale = if (throughput * dsum).max_element() > 0.0 { c.max_element() / (throughput * dsum).max_element() } else { 1.0 };
                sum += c;
                for (a, b) in groups.iter_mut().zip(g2.iter()) {
                    *a += throughput * *b * scale;
                }
            }
            if mask == 0 {
                break;
            }
            let Some(smp) = bsdf.sample(wo, mask & bsdf.mask(), sampler.next_1d(), sampler.next_2d()) else { break };
            if !self.depth_allows(&depth, smp.cat) {
                break;
            }
            mask_vis = Self::bump(&mut depth, smp.cat);
            throughput *= smp.weight;
            if smp.cat == Category::Sss {
                let Some((exit, w, _)) = self.sss_walk(&sp, &params, smp.wi, ray.time, sampler) else { break };
                throughput *= w;
                // Out of the walk: light the exit point as a diffuse surface facing out.
                let out_n = exit.ng;
                bsdf = Bsdf::lambert(Vec3::ONE, out_n, out_n);
                let d = self.direct(&exit, &bsdf, out_n, Category::Diffuse.bit(), Category::Diffuse.bit(), &counts, false, depth.total, ray.time, sampler, groups);
                sum += clamp_max(throughput * d[0], o.indirect_clamp);
                let Some(s2) = bsdf.sample(out_n, Category::Diffuse.bit(), 0.0, sampler.next_2d()) else { break };
                throughput *= s2.weight;
                prev = Prev { pdf: s2.pdf, delta: false, bsdf_count: 1.0, first: false, cat: Category::Diffuse, n: out_n, silent: false };
                ray = Ray::new(offset(exit.p, exit.ng, true), s2.wi, ray.time);
                medium = None;
                continue;
            }
            if smp.transmit && smp.cat == Category::Transmission && !params.thin_walled {
                medium = if sp.front && params.transmission_depth > 0.0 { Some(transmission_medium(&params)) } else { None };
            }
            prev = Prev { pdf: smp.pdf, delta: smp.delta, bsdf_count: 1.0, first: false, cat: smp.cat, n: sp.n, silent: false };
            ray = Ray::new(offset(sp.p, sp.ng, !smp.transmit), smp.wi, ray.time);
            // Russian roulette past a few bounces.
            if depth.total >= 3 {
                let q = luminance(throughput).clamp(0.0, 1.0).max(0.05);
                if q < 1.0 {
                    if sampler.rng().next_f32() > q {
                        break;
                    }
                    throughput /= q;
                }
            }
            if throughput.max_element() <= 0.0 || !throughput.is_finite() {
                break;
            }
        }
        sum
    }

    pub fn sss_walk(&self, sp: &crate::geom::SurfacePoint, p: &crate::shader::Params, wi: Vec3, time: f32, sampler: &mut Sampler) -> Option<(crate::geom::SurfacePoint, Vec3, Vec3)> {
        let radius = p.subsurface_radius * p.subsurface_scale.max(1e-6);
        let (exit, w, d) = self.walk(sp, wi, p.subsurface_color, radius, p.subsurface_anisotropy, time, sampler)?;
        // The walk's albedo mapping gives the colour on average; keep it bounded.
        Some((exit, w.min(Vec3::splat(4.0)), d))
    }
}

/// The volume inside glass with transmission depth: coloured by absorption at that depth, scattering if asked.
pub fn transmission_medium(p: &crate::shader::Params) -> Medium {
    let d = p.transmission_depth.max(1e-6);
    let c = p.transmission_color.clamp(Vec3::splat(1e-4), Vec3::ONE);
    let sigma_a = -Vec3::new(c.x.ln(), c.y.ln(), c.z.ln()) / d;
    Medium { sigma_a, sigma_s: p.transmission_scatter / d, g: p.transmission_scatter_anisotropy }
}
