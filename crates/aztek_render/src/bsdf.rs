//! The standard surface's scattering as lobes: diffuse (Oren-Nayar), translucency for thin walls, sheen, specular and
//! coat reflection (GGX with visible normal sampling), metal (GGX with an F82-tint Fresnel), transmission (rough
//! dielectric refraction) and the entry into subsurface scattering. Layers are combined by albedo scaling, as the
//! standard surface specification describes: what a layer reflects on average is taken from what reaches those below.
//!
//! Lobes carry a category, Arnold's: diffuse, specular, coat, sheen, transmission, subsurface. Lighting AOVs split by it,
//! and the first hit draws its samples category by category.

use std::sync::OnceLock;

use glam::{Vec2, Vec3};

use crate::math::{cosine_hemisphere, Frame, INV_PI, PI};
use crate::shader::Params;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Category {
    Diffuse = 0,
    Specular = 1,
    Coat = 2,
    Sheen = 3,
    Transmission = 4,
    Sss = 5,
}

pub const CATEGORIES: [Category; 6] = [Category::Diffuse, Category::Specular, Category::Coat, Category::Sheen, Category::Transmission, Category::Sss];

impl Category {
    pub fn bit(self) -> u8 {
        1 << self as u8
    }
    pub fn name(self) -> &'static str {
        match self {
            Category::Diffuse => "diffuse",
            Category::Specular => "specular",
            Category::Coat => "coat",
            Category::Sheen => "sheen",
            Category::Transmission => "transmission",
            Category::Sss => "sss",
        }
    }
}

pub const ALL: u8 = 0x3f;

// ---------------------------------------------------------------------------------------------------------------------
// Microfacet pieces

#[inline]
fn ggx_d(h: Vec3, ax: f32, ay: f32) -> f32 {
    let x = h.x / ax;
    let y = h.y / ay;
    let d = x * x + y * y + h.z * h.z;
    1.0 / (PI * ax * ay * d * d)
}

#[inline]
fn ggx_lambda(w: Vec3, ax: f32, ay: f32) -> f32 {
    let z2 = w.z * w.z;
    if z2 <= 0.0 {
        return 1e10;
    }
    let a2 = (w.x * w.x * ax * ax + w.y * w.y * ay * ay) / z2;
    0.5 * ((1.0 + a2).sqrt() - 1.0)
}

#[inline]
fn ggx_g1(w: Vec3, ax: f32, ay: f32) -> f32 {
    1.0 / (1.0 + ggx_lambda(w, ax, ay))
}

#[inline]
fn ggx_g2(wo: Vec3, wi: Vec3, ax: f32, ay: f32) -> f32 {
    1.0 / (1.0 + ggx_lambda(wo, ax, ay) + ggx_lambda(wi, ax, ay))
}

/// A visible normal for wo above the surface (Heitz 2018), via the spherical cap of Dupuy and Benyoub.
fn sample_vndf(wo: Vec3, ax: f32, ay: f32, u: Vec2) -> Vec3 {
    let wh = Vec3::new(ax * wo.x, ay * wo.y, wo.z).normalize();
    let phi = 2.0 * PI * u.x;
    let z = (1.0 - u.y) * (1.0 + wh.z) - wh.z;
    let st = (1.0 - z * z).max(0.0).sqrt();
    let c = Vec3::new(st * phi.cos(), st * phi.sin(), z);
    let h = c + wh;
    Vec3::new(ax * h.x, ay * h.y, h.z.max(1e-6)).normalize()
}

/// Density of a visible normal for wo.
#[inline]
fn vndf_pdf(wo: Vec3, h: Vec3, ax: f32, ay: f32) -> f32 {
    if wo.z <= 0.0 {
        return 0.0;
    }
    ggx_g1(wo, ax, ay) * wo.dot(h).max(0.0) * ggx_d(h, ax, ay) / wo.z
}

/// Unpolarised Fresnel reflectance at a dielectric boundary, eta = n_transmitted / n_incident.
pub fn fresnel_dielectric(cos_i: f32, eta: f32) -> f32 {
    let c = cos_i.abs().min(1.0);
    let s2 = (1.0 - c * c) / (eta * eta);
    if s2 >= 1.0 {
        return 1.0;
    }
    let ct = (1.0 - s2).sqrt();
    let rs = (c - eta * ct) / (c + eta * ct);
    let rp = (eta * c - ct) / (eta * c + ct);
    0.5 * (rs * rs + rp * rp)
}

/// Reflectance of a thin film of index n2 and thickness d (nm) between air and a base of index n3, per RGB wavelength.
pub fn thin_film(cos_i: f32, n2: f32, n3: f32, d_nm: f32) -> Vec3 {
    let n1 = 1.0f32;
    let c1 = cos_i.clamp(0.0, 1.0);
    let s1 = (1.0 - c1 * c1).max(0.0).sqrt();
    let s2 = n1 * s1 / n2;
    if s2 >= 1.0 {
        return Vec3::ONE;
    }
    let c2 = (1.0 - s2 * s2).sqrt();
    let s3 = n1 * s1 / n3;
    if s3 >= 1.0 {
        return Vec3::ONE;
    }
    let c3 = (1.0 - s3 * s3).sqrt();
    let mut out = [0.0f32; 3];
    for (k, lambda) in [650.0f32, 550.0, 450.0].iter().enumerate() {
        let delta = 4.0 * PI * n2 * d_nm * c2 / lambda;
        let mut total = 0.0;
        for p in 0..2 {
            let (r12, r23) = if p == 0 { ((n1 * c1 - n2 * c2) / (n1 * c1 + n2 * c2), (n2 * c2 - n3 * c3) / (n2 * c2 + n3 * c3)) } else { ((n2 * c1 - n1 * c2) / (n2 * c1 + n1 * c2), (n3 * c2 - n2 * c3) / (n3 * c2 + n2 * c3)) };
            let num = r12 * r12 + r23 * r23 + 2.0 * r12 * r23 * delta.cos();
            let den = 1.0 + r12 * r12 * r23 * r23 + 2.0 * r12 * r23 * delta.cos();
            total += num / den;
        }
        out[k] = (total * 0.5).clamp(0.0, 1.0);
    }
    Vec3::from_array(out)
}

/// The F82-tint conductor model: Schlick through F0, pulled down near grazing so that at cos 1/7 it is `tint` times
/// Schlick's value.
fn fresnel_f82(cos: f32, f0: Vec3, tint: Vec3) -> Vec3 {
    let c = cos.clamp(0.0, 1.0);
    let schlick = |f0: Vec3, c: f32| f0 + (Vec3::ONE - f0) * (1.0 - c).powi(5);
    let cb = 1.0 / 7.0;
    let fb = schlick(f0, cb);
    let a = fb * (Vec3::ONE - tint) / (cb * (1.0f32 - cb).powi(6));
    (schlick(f0, c) - a * c * (1.0 - c).powi(6)).clamp(Vec3::ZERO, Vec3::ONE)
}

// ---------------------------------------------------------------------------------------------------------------------
// Directional albedo tables, computed once by integration

const TN: usize = 16;

struct Tables {
    /// GGX reflection with Fresnel 1, by (cos, roughness).
    ggx: Vec<f32>,
    /// Dielectric GGX reflection by (cos, roughness, ior index).
    diel: Vec<f32>,
    /// Sheen by (cos, roughness).
    sheen: Vec<f32>,
}

const IORS: [f32; 8] = [1.0, 1.2, 1.33, 1.5, 1.7, 2.0, 2.4, 3.0];

fn tables() -> &'static Tables {
    static T: OnceLock<Tables> = OnceLock::new();
    T.get_or_init(|| {
        let n = 256;
        let mut ggx = vec![0.0; TN * TN];
        let mut diel = vec![0.0; TN * TN * IORS.len()];
        let mut sheen = vec![0.0; TN * TN];
        for ri in 0..TN {
            let rough = (ri as f32 + 0.5) / TN as f32;
            let a = (rough * rough).max(1e-4);
            for ci in 0..TN {
                let mu = ((ci as f32 + 0.5) / TN as f32).max(1e-3);
                let wo = Vec3::new((1.0 - mu * mu).sqrt(), 0.0, mu);
                let mut e = 0.0;
                let mut ed = [0.0f32; 8];
                let mut es = 0.0;
                for k in 0..n {
                    let u = Vec2::new(((k as f32) + 0.5) / n as f32, crate::math::to_unit((k as u32).reverse_bits()));
                    let h = sample_vndf(wo, a, a, u);
                    let wi = crate::math::reflect(wo, h);
                    if wi.z > 0.0 {
                        // weight = G2 / G1 for VNDF sampling.
                        let w = ggx_g2(wo, wi, a, a) / ggx_g1(wo, a, a);
                        e += w;
                        for (j, &ior) in IORS.iter().enumerate() {
                            ed[j] += w * fresnel_dielectric(wo.dot(h), ior);
                        }
                    }
                    let wi = cosine_hemisphere(u);
                    es += sheen_f(wo, wi, rough) * PI;
                }
                ggx[ri * TN + ci] = e / n as f32;
                for j in 0..IORS.len() {
                    diel[(j * TN + ri) * TN + ci] = ed[j] / n as f32;
                }
                sheen[ri * TN + ci] = (es / n as f32).min(1.0);
            }
        }
        Tables { ggx, diel, sheen }
    })
}

fn lookup2(t: &[f32], mu: f32, rough: f32) -> f32 {
    let x = (mu.clamp(0.0, 1.0) * TN as f32 - 0.5).clamp(0.0, (TN - 1) as f32);
    let y = (rough.clamp(0.0, 1.0) * TN as f32 - 0.5).clamp(0.0, (TN - 1) as f32);
    let (x0, y0) = (x.floor() as usize, y.floor() as usize);
    let (x1, y1) = ((x0 + 1).min(TN - 1), (y0 + 1).min(TN - 1));
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let v = |xx: usize, yy: usize| t[yy * TN + xx];
    let a = v(x0, y0) + (v(x1, y0) - v(x0, y0)) * fx;
    let b = v(x0, y1) + (v(x1, y1) - v(x0, y1)) * fx;
    a + (b - a) * fy
}

/// Mean reflectance of GGX with Fresnel 1: what single scattering keeps.
pub fn albedo_ggx(mu: f32, rough: f32) -> f32 {
    lookup2(&tables().ggx, mu, rough)
}

/// Mean reflectance of a dielectric GGX interface from outside.
pub fn albedo_dielectric(mu: f32, rough: f32, ior: f32) -> f32 {
    let t = tables();
    let ior = ior.clamp(1.0, 3.0);
    let j = IORS.iter().position(|&x| x >= ior).unwrap_or(IORS.len() - 1).max(1);
    let f = (ior - IORS[j - 1]) / (IORS[j] - IORS[j - 1]);
    let a = lookup2(&t.diel[(j - 1) * TN * TN..j * TN * TN], mu, rough);
    let b = lookup2(&t.diel[j * TN * TN..(j + 1) * TN * TN], mu, rough);
    a + (b - a) * f
}

pub fn albedo_sheen(mu: f32, rough: f32) -> f32 {
    lookup2(&tables().sheen, mu, rough)
}

/// Charlie sheen with Ashikhmin's visibility; times cos.
fn sheen_f(wo: Vec3, wi: Vec3, rough: f32) -> f32 {
    if wo.z <= 0.0 || wi.z <= 0.0 {
        return 0.0;
    }
    let h = (wo + wi).normalize();
    let r = rough.clamp(0.07, 1.0);
    let inv = 1.0 / r;
    let sin_h = (1.0 - h.z * h.z).max(0.0).sqrt();
    let d = (2.0 + inv) * sin_h.powf(inv) / (2.0 * PI);
    let v = 1.0 / (4.0 * (wi.z + wo.z - wi.z * wo.z));
    d * v * wi.z
}

// ---------------------------------------------------------------------------------------------------------------------
// Lobes

#[derive(Clone, Copy, Debug)]
pub enum LobeKind {
    Diffuse { color: Vec3, sigma: f32 },
    /// Diffuse through to the other side: thin walled subsurface, Maya's translucence.
    Translucent { color: Vec3 },
    Sheen { color: Vec3, rough: f32 },
    /// Dielectric reflection: specular and coat. `film` replaces Fresnel with a thin film's when present.
    Dielectric { tint: Vec3, ax: f32, ay: f32, eta: f32, film: Option<(f32, f32)> },
    Conductor { f0: Vec3, tint: Vec3, ax: f32, ay: f32, film: Option<(f32, f32)> },
    /// Refraction through a rough dielectric; `thin` passes straight through instead of bending.
    Refract { tint: Vec3, ax: f32, ay: f32, eta: f32, thin: bool },
    /// Into the medium beneath for a random walk; the integrator takes over.
    Subsurface { color: Vec3, radius: Vec3, anisotropy: f32 },
}

#[derive(Clone, Copy, Debug)]
pub struct Lobe {
    pub kind: LobeKind,
    /// Colour weight from layering, applied to what the lobe returns.
    pub weight: Vec3,
    /// How likely the lobe is picked when sampling.
    pub pick: f32,
    pub cat: Category,
    /// Uses the coat's frame rather than the base's.
    pub coat_frame: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub wi: Vec3,
    /// f * cos / pdf.
    pub weight: Vec3,
    pub pdf: f32,
    pub cat: Category,
    /// From a lobe so narrow that light sampling can't help: no MIS against lights.
    pub delta: bool,
    pub transmit: bool,
    pub lobe: usize,
}

/// What lighting from a direction contributes per category, and the density sampling would have picked it with.
#[derive(Clone, Copy, Debug, Default)]
pub struct Eval {
    pub f: [Vec3; 6],
    pub pdf: f32,
    /// The density of sampling within each category alone.
    pub pdf_cat: [f32; 6],
}

impl Eval {
    pub fn total(&self) -> Vec3 {
        self.f.iter().copied().sum()
    }
}

#[derive(Clone, Debug)]
pub struct Bsdf {
    pub frame: Frame,
    pub coat_frame: Frame,
    /// Geometric normal facing wo.
    pub ng: Vec3,
    pub lobes: [Option<Lobe>; 8],
    pub count: usize,
    pub emission: Vec3,
    /// The ray arrived from outside the object.
    pub entering: bool,
    pub thin_walled: bool,
    /// For the albedo AOV.
    pub albedo: Vec3,
}

const MIN_ALPHA: f32 = 1e-4;
/// Below this alpha a lobe counts as a mirror for MIS.
const DELTA_ALPHA: f32 = 2.5e-3;

fn alphas(rough: f32, aniso: f32) -> (f32, f32) {
    let a = (rough * rough).max(MIN_ALPHA);
    if aniso <= 0.0 {
        return (a, a);
    }
    let ax = a * (2.0 / (1.0 + (1.0 - aniso).powi(2))).sqrt();
    (ax.max(MIN_ALPHA), (ax * (1.0 - aniso)).max(MIN_ALPHA))
}

impl Bsdf {
    fn push(&mut self, l: Lobe) {
        if self.count < 8 && l.pick > 0.0 && l.weight.max_element() > 0.0 {
            self.lobes[self.count] = Some(l);
            self.count += 1;
        }
    }

    /// The standard surface at a point. `wo` towards the viewer; `entering` says the ray came from outside.
    pub fn standard(p: &Params, wo: Vec3, ng: Vec3, entering: bool) -> Bsdf {
        let n = p.n;
        let mut frame = Frame::from_nt(n, p.tangent);
        if p.specular_rotation != 0.0 {
            let (s, c) = (p.specular_rotation * 2.0 * PI).sin_cos();
            frame = Frame::from_nt(n, frame.t * c + frame.b * s);
        }
        let coat_frame = Frame::from_nt(p.coat_n, p.tangent);
        let mut b = Bsdf { frame, coat_frame, ng, lobes: [None; 8], count: 0, emission: Vec3::ZERO, entering, thin_walled: p.thin_walled, albedo: Vec3::ZERO };
        let mu = wo.dot(n).abs().max(1e-4);
        let mu_c = wo.dot(p.coat_n).abs().max(1e-4);

        // Coat on top: what it reflects on average leaves the layers below, tinted by its colour.
        let mut under = Vec3::ONE;
        let coat = p.coat.clamp(0.0, 1.0);
        let mut spec_rough = p.specular_roughness;
        let mut base_color = p.base_color;
        if coat > 0.0 {
            let (ax, ay) = alphas(p.coat_roughness, p.coat_anisotropy);
            let eta = if entering || p.thin_walled { p.coat_ior.max(1.0001) } else { 1.0 / p.coat_ior.max(1.0001) };
            let e = albedo_dielectric(mu_c, p.coat_roughness, p.coat_ior);
            b.push(Lobe { kind: LobeKind::Dielectric { tint: Vec3::ONE, ax, ay, eta, film: None }, weight: Vec3::splat(coat), pick: coat * e.max(0.04), cat: Category::Coat, coat_frame: true });
            under = Vec3::ONE.lerp(p.coat_color * (1.0 - e), coat);
            // The coat deepens colours beneath it and roughens what's under by its own roughness.
            if p.coat_affect_color > 0.0 {
                let g = 1.0 + p.coat_affect_color * coat;
                base_color = Vec3::new(base_color.x.powf(g), base_color.y.powf(g), base_color.z.powf(g));
            }
            if p.coat_affect_roughness > 0.0 {
                spec_rough = spec_rough + (1.0 - spec_rough) * (coat * p.coat_affect_roughness * p.coat_roughness);
            }
        }
        b.emission = p.emission * under;

        // Sheen over the base.
        let sheen = p.sheen.clamp(0.0, 1.0);
        if sheen > 0.0 {
            let e = albedo_sheen(mu, p.sheen_roughness);
            b.push(Lobe { kind: LobeKind::Sheen { color: p.sheen_color, rough: p.sheen_roughness }, weight: under * sheen, pick: (sheen * e * p.sheen_color.max_element()).max(0.01) * under.max_element(), cat: Category::Sheen, coat_frame: false });
            under *= 1.0 - sheen * e * p.sheen_color.max_element();
        }

        let metal = p.metalness.clamp(0.0, 1.0);
        let (ax, ay) = alphas(spec_rough, p.specular_anisotropy);
        let film = (p.thin_film_thickness > 0.0).then_some((p.thin_film_thickness, p.thin_film_ior));
        if metal > 0.0 {
            let f0 = (base_color * p.base).clamp(Vec3::ZERO, Vec3::ONE);
            let tint = p.specular_color * p.specular;
            // Multiple scattering lost by single scattering GGX comes back in the Fresnel's colour (Turquin 2019).
            let e = albedo_ggx(mu, spec_rough).max(0.05);
            let ms = Vec3::ONE + f0 * ((1.0 - e) / e);
            b.push(Lobe { kind: LobeKind::Conductor { f0, tint, ax, ay, film }, weight: under * metal * ms, pick: metal * under.max_element() * (0.3 + f0.max_element()), cat: Category::Specular, coat_frame: false });
        }
        let dielectric = 1.0 - metal;
        if dielectric > 0.0 {
            let ior = p.specular_ior.max(1.0001);
            let eta = if entering || p.thin_walled { ior } else { 1.0 / ior };
            let spec = p.specular.clamp(0.0, 1.0);
            let w = under * dielectric;
            let e_spec = if entering || p.thin_walled { albedo_dielectric(mu, spec_rough, ior) } else { 0.0 };
            if spec > 0.0 {
                b.push(Lobe { kind: LobeKind::Dielectric { tint: p.specular_color, ax, ay, eta, film }, weight: w * spec, pick: w.max_element() * spec * (e_spec.max(0.06) + if p.transmission > 0.0 { 0.2 } else { 0.0 }), cat: Category::Specular, coat_frame: false });
            }
            let trans = p.transmission.clamp(0.0, 1.0);
            if trans > 0.0 {
                let (tax, tay) = alphas((spec_rough + p.transmission_extra_roughness).clamp(0.0, 1.0), p.specular_anisotropy);
                // With depth, colour comes from absorption in the volume, not at the surface.
                let tint = if p.transmission_depth > 0.0 && !p.thin_walled { Vec3::ONE } else { p.transmission_color };
                b.push(Lobe { kind: LobeKind::Refract { tint, ax: tax, ay: tay, eta, thin: p.thin_walled }, weight: w * trans * Vec3::splat(1.0).lerp(Vec3::splat(1.0), spec), pick: w.max_element() * trans, cat: Category::Transmission, coat_frame: false });
            }
            // Beneath the specular layer, only what it lets through.
            let rest = w * (1.0 - trans) * (1.0 - spec * e_spec);
            let sss = p.subsurface.clamp(0.0, 1.0);
            if sss > 0.0 {
                if p.thin_walled {
                    b.push(Lobe { kind: LobeKind::Translucent { color: p.subsurface_color }, weight: rest * sss, pick: (rest * sss).max_element() * p.subsurface_color.max_element(), cat: Category::Sss, coat_frame: false });
                } else {
                    b.push(Lobe { kind: LobeKind::Subsurface { color: p.subsurface_color, radius: p.subsurface_radius * p.subsurface_scale, anisotropy: p.subsurface_anisotropy }, weight: rest * sss, pick: (rest * sss).max_element(), cat: Category::Sss, coat_frame: false });
                }
            }
            let diff = rest * (1.0 - sss);
            let dc = base_color * p.base;
            if dc.max_element() > 0.0 {
                b.push(Lobe { kind: LobeKind::Diffuse { color: dc, sigma: p.diffuse_roughness }, weight: diff, pick: (diff * dc).max_element(), cat: Category::Diffuse, coat_frame: false });
            }
            b.albedo += diff * dc + rest * sss * p.subsurface_color + w * trans * p.transmission_color;
        }
        b.albedo += under * metal * base_color * p.base;
        b
    }

    /// A plain lambert: for tests and simple cases.
    pub fn lambert(color: Vec3, n: Vec3, ng: Vec3) -> Bsdf {
        let mut b = Bsdf { frame: Frame::from_n(n), coat_frame: Frame::from_n(n), ng, lobes: [None; 8], count: 0, emission: Vec3::ZERO, entering: true, thin_walled: false, albedo: color };
        b.push(Lobe { kind: LobeKind::Diffuse { color, sigma: 0.0 }, weight: Vec3::ONE, pick: 1.0, cat: Category::Diffuse, coat_frame: false });
        b
    }

    pub fn has(&self, mask: u8) -> bool {
        self.lobes[..self.count].iter().flatten().any(|l| mask & l.cat.bit() != 0)
    }

    /// Categories present.
    pub fn mask(&self) -> u8 {
        self.lobes[..self.count].iter().flatten().fold(0, |m, l| m | l.cat.bit())
    }

    fn pick_total(&self, mask: u8) -> f32 {
        self.lobes[..self.count].iter().flatten().filter(|l| mask & l.cat.bit() != 0).map(|l| l.pick).sum()
    }

    /// Whether every lobe in the mask is a mirror or clear refraction: light sampling is pointless there.
    pub fn is_delta(&self, mask: u8) -> bool {
        self.lobes[..self.count].iter().flatten().filter(|l| mask & l.cat.bit() != 0).all(|l| lobe_delta(&l.kind))
    }

    fn lobe_eval(&self, l: &Lobe, wo_w: Vec3, wi_w: Vec3) -> (Vec3, f32) {
        let fr = if l.coat_frame { &self.coat_frame } else { &self.frame };
        let wo = fr.to_local(wo_w);
        let wi = fr.to_local(wi_w);
        // Which side of the real surface the light is: the geometric normal decides reflection versus transmission.
        let reflect = wi_w.dot(self.ng) * wo_w.dot(self.ng) > 0.0;
        eval_lobe(&l.kind, wo, wi, reflect)
    }

    /// Lighting from wi, split by category, for the lobes in `mask`, with the mixture pdf of sampling within `mask`.
    pub fn eval(&self, wo: Vec3, wi: Vec3, mask: u8) -> Eval {
        let total = self.pick_total(mask);
        let mut out = Eval::default();
        if total <= 0.0 {
            return out;
        }
        let mut picks = [0.0f32; 6];
        for l in self.lobes[..self.count].iter().flatten() {
            if mask & l.cat.bit() == 0 {
                continue;
            }
            let (f, pdf) = self.lobe_eval(l, wo, wi);
            let c = l.cat as usize;
            out.f[c] += f * l.weight;
            out.pdf += pdf * l.pick / total;
            out.pdf_cat[c] += pdf * l.pick;
            picks[c] += l.pick;
        }
        for c in 0..6 {
            if picks[c] > 0.0 {
                out.pdf_cat[c] /= picks[c];
            }
        }
        out
    }

    pub fn sample(&self, wo_w: Vec3, mask: u8, u_pick: f32, u: Vec2) -> Option<Sample> {
        let total = self.pick_total(mask);
        if total <= 0.0 {
            return None;
        }
        let mut x = u_pick * total;
        let mut chosen = None;
        for (i, l) in self.lobes[..self.count].iter().enumerate() {
            let Some(l) = l else { continue };
            if mask & l.cat.bit() == 0 {
                continue;
            }
            chosen = Some(i);
            if x < l.pick {
                break;
            }
            x -= l.pick;
        }
        let li = chosen?;
        let l = self.lobes[li].unwrap();
        let fr = if l.coat_frame { &self.coat_frame } else { &self.frame };
        let wo = fr.to_local(wo_w);
        let wi = sample_lobe(&l.kind, wo, u, self.entering)?;
        let wi_w = fr.to_world(wi);
        let transmit = wi_w.dot(self.ng) * wo_w.dot(self.ng) < 0.0;
        if matches!(l.kind, LobeKind::Subsurface { .. }) {
            // The walk starts going in; its weight is applied by the integrator.
            return Some(Sample { wi: wi_w, weight: l.weight * (total / l.pick), pdf: 1.0, cat: Category::Sss, delta: false, transmit: true, lobe: li });
        }
        if lobe_delta(&l.kind) {
            // A mirror or clear refraction: only this lobe matters along wi; the others' density there is nil.
            let (f, pdf) = self.lobe_eval(&l, wo_w, wi_w);
            if pdf <= 0.0 {
                return None;
            }
            let w = f * l.weight / (pdf * l.pick / total);
            return Some(Sample { wi: wi_w, weight: w, pdf: pdf * l.pick / total, cat: l.cat, delta: true, transmit, lobe: li });
        }
        let e = self.eval(wo_w, wi_w, mask);
        if e.pdf <= 0.0 {
            return None;
        }
        // The weight as the sum of every lobe, so mixing them stays unbiased.
        let w = e.total() / e.pdf;
        Some(Sample { wi: wi_w, weight: w, pdf: e.pdf, cat: l.cat, delta: false, transmit, lobe: li })
    }
}

fn lobe_delta(k: &LobeKind) -> bool {
    match k {
        LobeKind::Dielectric { ax, ay, .. } | LobeKind::Conductor { ax, ay, .. } | LobeKind::Refract { ax, ay, .. } => ax.max(*ay) < DELTA_ALPHA,
        _ => false,
    }
}

fn diel_fresnel(cos: f32, eta: f32, film: Option<(f32, f32)>) -> Vec3 {
    match film {
        Some((d, n2)) if eta > 1.0 => thin_film(cos, n2, eta, d),
        _ => Vec3::splat(fresnel_dielectric(cos, eta)),
    }
}

/// f * cos and pdf of one lobe, in its local frame.
fn eval_lobe(k: &LobeKind, wo: Vec3, wi: Vec3, reflect: bool) -> (Vec3, f32) {
    match *k {
        LobeKind::Diffuse { color, sigma } => {
            if !reflect || wi.z <= 0.0 || wo.z <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            let f = if sigma > 0.0 {
                // Oren-Nayar, qualitative form.
                let s2 = sigma * sigma;
                let a = 1.0 - 0.5 * s2 / (s2 + 0.33);
                let bb = 0.45 * s2 / (s2 + 0.09);
                let (so, si) = ((1.0 - wo.z * wo.z).max(0.0).sqrt(), (1.0 - wi.z * wi.z).max(0.0).sqrt());
                let cos_phi = if so > 1e-4 && si > 1e-4 { (wo.x * wi.x + wo.y * wi.y) / (so * si) } else { 0.0 };
                let (sa, tb) = if wi.z < wo.z { (si, so / wo.z) } else { (so, si / wi.z) };
                a + bb * cos_phi.max(0.0) * sa * tb
            } else {
                1.0
            };
            (color * (f * INV_PI * wi.z), wi.z * INV_PI)
        }
        LobeKind::Translucent { color } => {
            if reflect || wi.z >= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            (color * (-wi.z * INV_PI), -wi.z * INV_PI)
        }
        LobeKind::Sheen { color, rough } => {
            if !reflect || wi.z <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            (color * sheen_f(wo, wi, rough), wi.z * INV_PI)
        }
        LobeKind::Dielectric { tint, ax, ay, eta, film } => {
            if !reflect || wi.z <= 0.0 || wo.z <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            let h = (wo + wi).normalize();
            let d = ggx_d(h, ax, ay);
            let f = diel_fresnel(wo.dot(h), eta, film);
            let g = ggx_g2(wo, wi, ax, ay);
            let val = tint * f * (d * g / (4.0 * wo.z));
            (val, vndf_pdf(wo, h, ax, ay) / (4.0 * wo.dot(h)).max(1e-9))
        }
        LobeKind::Conductor { f0, tint, ax, ay, film } => {
            if !reflect || wi.z <= 0.0 || wo.z <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            let h = (wo + wi).normalize();
            let d = ggx_d(h, ax, ay);
            let mut f = fresnel_f82(wo.dot(h), f0, tint);
            if let Some((th, n2)) = film {
                let l = crate::math::luminance(f0).clamp(0.0, 0.99);
                let n3 = (1.0 + l.sqrt()) / (1.0 - l.sqrt());
                let plain = fresnel_dielectric(wo.dot(h), n3).max(1e-4);
                f = (f * thin_film(wo.dot(h), n2, n3, th) / plain).min(Vec3::ONE);
            }
            let g = ggx_g2(wo, wi, ax, ay);
            (f * (d * g / (4.0 * wo.z)), vndf_pdf(wo, h, ax, ay) / (4.0 * wo.dot(h)).max(1e-9))
        }
        LobeKind::Refract { tint, ax, ay, eta, thin } => {
            if reflect || wo.z <= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            if thin {
                // Straight through: the mirror of a reflection about the surface plane.
                let wr = Vec3::new(wi.x, wi.y, -wi.z);
                if wr.z <= 0.0 {
                    return (Vec3::ZERO, 0.0);
                }
                let h = (wo + wr).normalize();
                let d = ggx_d(h, ax, ay);
                let f = fresnel_dielectric(wo.dot(h), eta);
                let g = ggx_g2(wo, wr, ax, ay);
                return (tint * ((1.0 - f) * d * g / (4.0 * wo.z)), vndf_pdf(wo, h, ax, ay) / (4.0 * wo.dot(h)).max(1e-9));
            }
            if wi.z >= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            // Generalised half vector for refraction (Walter 2007).
            let mut h = -(wo + wi * eta).normalize();
            if h.z < 0.0 {
                h = -h;
            }
            let (ho, hi) = (wo.dot(h), wi.dot(h));
            if ho <= 0.0 || hi >= 0.0 {
                return (Vec3::ZERO, 0.0);
            }
            let f = fresnel_dielectric(ho, eta);
            let d = ggx_d(h, ax, ay);
            let g = ggx_g2(wo, wi, ax, ay);
            let denom = (ho + eta * hi).powi(2);
            // Radiance scales by 1/eta^2 crossing into denser media; Arnold-style renderers leave it out of the BSDF
            // so that light paths and camera paths agree. Left out here too.
            let val = (1.0 - f) * d * g * ho * (-hi) * eta * eta / (denom * wo.z);
            let pdf = vndf_pdf(wo, h, ax, ay) * eta * eta * (-hi) / denom;
            (tint * val, pdf)
        }
        LobeKind::Subsurface { .. } => (Vec3::ZERO, 0.0),
    }
}

fn refract(wo: Vec3, h: Vec3, eta: f32) -> Option<Vec3> {
    let c = wo.dot(h);
    let s2 = (1.0 - c * c).max(0.0) / (eta * eta);
    if s2 >= 1.0 {
        return None;
    }
    let ct = (1.0 - s2).sqrt();
    Some(-wo / eta + h * (c / eta - ct))
}

fn sample_lobe(k: &LobeKind, wo: Vec3, u: Vec2, _entering: bool) -> Option<Vec3> {
    match *k {
        LobeKind::Diffuse { .. } | LobeKind::Sheen { .. } => Some(cosine_hemisphere(u)),
        LobeKind::Translucent { .. } | LobeKind::Subsurface { .. } => {
            let d = cosine_hemisphere(u);
            Some(Vec3::new(d.x, d.y, -d.z))
        }
        LobeKind::Dielectric { ax, ay, .. } | LobeKind::Conductor { ax, ay, .. } => {
            if wo.z <= 0.0 {
                return None;
            }
            let h = sample_vndf(wo, ax, ay, u);
            let wi = crate::math::reflect(wo, h);
            (wi.z > 0.0).then_some(wi)
        }
        LobeKind::Refract { ax, ay, eta, thin, .. } => {
            if wo.z <= 0.0 {
                return None;
            }
            let h = sample_vndf(wo, ax, ay, u);
            if thin {
                let r = crate::math::reflect(wo, h);
                return (r.z > 0.0).then_some(Vec3::new(r.x, r.y, -r.z));
            }
            let wi = refract(wo, h, eta)?;
            (wi.z < 0.0).then_some(wi)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::math::Pcg;

    fn params() -> Params {
        Params { base: 1.0, base_color: Vec3::ONE, specular: 1.0, specular_color: Vec3::ONE, specular_ior: 1.5, specular_roughness: 0.3, opacity: Vec3::ONE, n: Vec3::Z, coat_n: Vec3::Z, tangent: Vec3::X, coat_color: Vec3::ONE, coat_ior: 1.5, sheen_color: Vec3::ONE, transmission_color: Vec3::ONE, subsurface_color: Vec3::ONE, subsurface_radius: Vec3::ONE, subsurface_scale: 1.0, thin_film_ior: 1.5, ..Default::default() }
    }

    /// Estimates the directional albedo by sampling, and the same by uniform integration of eval: they must agree.
    fn check(b: &Bsdf, wo: Vec3) -> (Vec3, Vec3) {
        let mut rng = Pcg::new(7);
        let n = 200_000;
        let mut by_sampling = Vec3::ZERO;
        for _ in 0..n {
            if let Some(s) = b.sample(wo, ALL, rng.next_f32(), rng.next_2d()) {
                if !s.delta {
                    by_sampling += s.weight;
                }
            }
        }
        let mut by_eval = Vec3::ZERO;
        for _ in 0..n {
            let wi = crate::math::uniform_sphere(rng.next_2d());
            by_eval += b.eval(wo, wi, ALL).total() * (4.0 * PI);
        }
        (by_sampling / n as f32, by_eval / n as f32)
    }

    #[test]
    fn sampling_matches_evaluation() {
        let wo = Vec3::new(0.5, 0.1, 0.86).normalize();
        let mut p = params();
        p.specular_roughness = 0.5;
        let b = Bsdf::standard(&p, wo, Vec3::Z, true);
        let (s, e) = check(&b, wo);
        assert!((s - e).abs().max_element() < 0.02, "{s} {e}");
        p.metalness = 1.0;
        p.base_color = Vec3::new(0.9, 0.6, 0.3);
        let b = Bsdf::standard(&p, wo, Vec3::Z, true);
        let (s, e) = check(&b, wo);
        assert!((s - e).abs().max_element() < 0.02, "{s} {e}");
        p.metalness = 0.0;
        p.transmission = 1.0;
        p.specular_roughness = 0.4;
        let b = Bsdf::standard(&p, wo, Vec3::Z, true);
        let (s, e) = check(&b, wo);
        assert!((s - e).abs().max_element() < 0.03, "{s} {e}");
        p.transmission = 0.0;
        p.coat = 1.0;
        p.sheen = 1.0;
        let b = Bsdf::standard(&p, wo, Vec3::Z, true);
        let (s, e) = check(&b, wo);
        assert!((s - e).abs().max_element() < 0.02, "{s} {e}");
    }

    #[test]
    fn white_dielectric_and_metal_conserve_energy() {
        // The white standard surface reflects close to all it receives: diffuse beneath the specular's leftovers.
        for &mu in &[0.9f32, 0.5, 0.2] {
            let wo = Vec3::new((1.0 - mu * mu).sqrt(), 0.0, mu);
            let p = params();
            let b = Bsdf::standard(&p, wo, Vec3::Z, true);
            let (s, _) = check(&b, wo);
            assert!(s.x > 0.93 && s.x < 1.02, "dielectric at {mu}: {s}");
            let mut p = params();
            p.metalness = 1.0;
            p.specular_roughness = 0.6;
            let b = Bsdf::standard(&p, wo, Vec3::Z, true);
            let (s, _) = check(&b, wo);
            assert!(s.x > 0.93 && s.x < 1.03, "metal at {mu}: {s}");
        }
    }

    #[test]
    fn fresnel_values() {
        assert!((fresnel_dielectric(1.0, 1.5) - 0.04).abs() < 1e-4);
        assert_eq!(fresnel_dielectric(0.1, 1.0 / 1.5), 1.0);
        let f = thin_film(1.0, 1.5, 1.5, 300.0);
        assert!((f.x - 0.04).abs() < 0.01, "a film of the base's index changes nothing: {f}");
    }
}
