//! The image being rendered: every AOV a render keeps, accumulated sample by sample with the pixel filter's weight,
//! and per pixel what adaptive sampling needs to know about its noise.

use glam::{Vec3, Vec4};

use crate::first_hit::{SampleOut, ALBEDO, BACKGROUND, DIRECT, EMISSION, INDIRECT, SHADOW_MATTE, VOLUME};
use crate::scene::FilterKind;

/// What an AOV holds, and how it is filled.
#[derive(Clone, Debug, PartialEq)]
pub enum AovKind {
    Beauty,
    /// Sum of light slots.
    Slots(Vec<usize>),
    Group(usize),
    Albedo,
    Z,
    N,
    P,
    /// An id as a colour, from the object or material: mattes without cryptomatte.
    ObjectId,
    MaterialId,
    ShadowMatte,
    /// The beauty's alpha alone, white where covered.
    Alpha,
}

impl AovKind {
    /// Data AOVs take the sample nearest the pixel centre rather than a filtered mean.
    pub fn closest(&self) -> bool {
        matches!(self, AovKind::Z | AovKind::N | AovKind::P | AovKind::ObjectId | AovKind::MaterialId)
    }
}

/// The AOV names Harold knows, Arnold's names, for the Render Settings list.
pub const AOV_NAMES: &[&str] = &[
    "RGBA", "direct", "indirect", "diffuse", "diffuse_direct", "diffuse_indirect", "specular", "specular_direct", "specular_indirect", "coat", "coat_direct", "coat_indirect", "sheen", "sheen_direct", "sheen_indirect", "sss",
    "sss_direct", "sss_indirect", "transmission", "transmission_direct", "transmission_indirect", "emission", "background", "volume", "albedo", "diffuse_albedo", "N", "P", "Z", "ID", "object_id", "material_id", "shadow_matte", "A",
];

pub fn aov_kind(name: &str, groups: &[String]) -> Option<AovKind> {
    let cat = |c: &str| -> Option<usize> {
        Some(match c {
            "diffuse" => 0,
            "specular" => 1,
            "coat" => 2,
            "sheen" => 3,
            "transmission" => 4,
            "sss" => 5,
            _ => return None,
        })
    };
    if let Some(g) = name.strip_prefix("RGBA_") {
        return groups.iter().position(|x| x == g).map(AovKind::Group);
    }
    Some(match name {
        "RGBA" | "beauty" => AovKind::Beauty,
        "direct" => AovKind::Slots((0..6).map(|c| DIRECT + c).collect()),
        "indirect" => AovKind::Slots((0..6).map(|c| INDIRECT + c).collect()),
        "emission" => AovKind::Slots(vec![EMISSION]),
        "background" => AovKind::Slots(vec![BACKGROUND]),
        "volume" => AovKind::Slots(vec![VOLUME]),
        "albedo" | "diffuse_albedo" => AovKind::Albedo,
        "N" => AovKind::N,
        "P" => AovKind::P,
        "Z" => AovKind::Z,
        "ID" | "object_id" => AovKind::ObjectId,
        "material_id" => AovKind::MaterialId,
        "shadow_matte" => AovKind::ShadowMatte,
        "A" => AovKind::Alpha,
        _ => {
            if let Some(c) = cat(name) {
                AovKind::Slots(vec![DIRECT + c, INDIRECT + c])
            } else if let Some((c, rest)) = name.rsplit_once('_') {
                let c = cat(c)?;
                match rest {
                    "direct" => AovKind::Slots(vec![DIRECT + c]),
                    "indirect" => AovKind::Slots(vec![INDIRECT + c]),
                    _ => return None,
                }
            } else {
                return None;
            }
        }
    })
}

/// A colour from an id: stable, distinct, for mattes.
pub fn id_color(id: u32) -> Vec3 {
    let h = crate::math::hash_u32(id.wrapping_mul(2654435761).wrapping_add(1));
    Vec3::new((h & 255) as f32, ((h >> 8) & 255) as f32, ((h >> 16) & 255) as f32) / 255.0
}

fn value_of(kind: &AovKind, s: &SampleOut) -> Vec4 {
    match kind {
        AovKind::Beauty => s.color.extend(s.alpha),
        AovKind::Slots(v) => v.iter().map(|&i| s.slots[i]).sum::<Vec3>().extend(s.alpha),
        AovKind::Group(g) => s.groups[*g % s.groups.len()].extend(s.alpha),
        AovKind::Albedo => s.slots[ALBEDO].extend(s.alpha),
        AovKind::Z => Vec4::new(s.z, s.z, s.z, 1.0),
        AovKind::N => s.n.extend(1.0),
        AovKind::P => s.p.extend(1.0),
        AovKind::ObjectId => if s.z.is_finite() { id_color(s.object).extend(1.0) } else { Vec4::ZERO },
        AovKind::MaterialId => if s.z.is_finite() { id_color(s.material + 7919).extend(1.0) } else { Vec4::ZERO },
        AovKind::ShadowMatte => s.slots[SHADOW_MATTE].extend(s.alpha),
        AovKind::Alpha => Vec4::splat(s.alpha),
    }
}

/// Offsets distributed as a filter, by inverting its tabulated cumulative.
#[derive(Clone, Debug)]
pub struct FilterTable {
    pub kind: FilterKind,
    pub width: f32,
    cdf: Vec<f32>,
    sign: Vec<f32>,
}

const FBINS: usize = 256;

impl FilterTable {
    pub fn new(kind: FilterKind, width: f32) -> FilterTable {
        let width = if kind == FilterKind::Closest { 1.0 } else { width.max(0.5) };
        let r = width * 0.5;
        let mut cdf = vec![0.0; FBINS + 1];
        let mut sign = vec![1.0; FBINS];
        for i in 0..FBINS {
            let x = -r + (i as f32 + 0.5) / FBINS as f32 * width;
            let f = kind.eval(x, width);
            sign[i] = if f < 0.0 { -1.0 } else { 1.0 };
            cdf[i + 1] = cdf[i] + f.abs();
        }
        let total = cdf[FBINS].max(1e-12);
        for c in &mut cdf {
            *c /= total;
        }
        FilterTable { kind, width, cdf, sign }
    }

    /// An offset from the pixel centre and the sign of its weight.
    pub fn sample(&self, u: f32) -> (f32, f32) {
        if self.kind == FilterKind::Closest {
            return (u - 0.5, 1.0);
        }
        let i = self.cdf.partition_point(|&c| c <= u).saturating_sub(1).min(FBINS - 1);
        let span = (self.cdf[i + 1] - self.cdf[i]).max(1e-12);
        let f = ((u - self.cdf[i]) / span).clamp(0.0, 1.0);
        (-self.width * 0.5 + (i as f32 + f) / FBINS as f32 * self.width, self.sign[i])
    }
}

#[derive(Clone, Debug)]
pub struct Film {
    pub width: u32,
    pub height: u32,
    pub aovs: Vec<(String, AovKind)>,
    /// Per AOV, per pixel, weighted sums.
    pub sums: Vec<Vec<Vec4>>,
    pub weights: Vec<f32>,
    /// For closest AOVs: how near the centre the kept sample was.
    pub nearest: Vec<f32>,
    pub counts: Vec<u32>,
    /// Running mean and sum of squares of the tone mapped luminance, for adaptive sampling.
    pub mean: Vec<f32>,
    pub m2: Vec<f32>,
    /// A pixel adaptive sampling has finished with.
    pub done: Vec<bool>,
}

impl Film {
    pub fn new(width: u32, height: u32, names: &[String], groups: &[String]) -> Film {
        let n = (width * height) as usize;
        let mut aovs = vec![("RGBA".to_string(), AovKind::Beauty)];
        for name in names {
            if aovs.iter().any(|(a, _)| a == name) {
                continue;
            }
            if let Some(k) = aov_kind(name, groups) {
                aovs.push((name.clone(), k));
            } else {
                log::warn!("Harold: no AOV named {name}");
            }
        }
        let sums = aovs.iter().map(|_| vec![Vec4::ZERO; n]).collect();
        Film { width, height, aovs, sums, weights: vec![0.0; n], nearest: vec![f32::INFINITY; n], counts: vec![0; n], mean: vec![0.0; n], m2: vec![0.0; n], done: vec![false; n] }
    }

    pub fn clear(&mut self) {
        for s in &mut self.sums {
            s.fill(Vec4::ZERO);
        }
        self.weights.fill(0.0);
        self.nearest.fill(f32::INFINITY);
        self.counts.fill(0);
        self.mean.fill(0.0);
        self.m2.fill(0.0);
        self.done.fill(false);
    }

    /// Adds one camera sample to pixel i, with filter weight w at distance d from the centre.
    pub fn add(&mut self, i: usize, s: &SampleOut, w: f32, d: f32) {
        let closer = d < self.nearest[i];
        if closer {
            self.nearest[i] = d;
        }
        for (k, (_, kind)) in self.aovs.iter().enumerate() {
            let v = value_of(kind, s);
            if kind.closest() {
                if closer {
                    self.sums[k][i] = v;
                }
            } else if v.is_finite() {
                self.sums[k][i] += v * w;
            }
        }
        self.weights[i] += w;
        let l = crate::math::luminance(s.color);
        let t = if l.is_finite() { l.max(0.0) / (1.0 + l.max(0.0)) } else { 0.0 };
        self.counts[i] += 1;
        let n = self.counts[i] as f32;
        let delta = t - self.mean[i];
        self.mean[i] += delta / n;
        self.m2[i] += delta * (t - self.mean[i]);
    }

    /// Standard error of a pixel's mean, on tone mapped luminance.
    pub fn error(&self, i: usize) -> f32 {
        let n = self.counts[i] as f32;
        if n < 2.0 {
            return f32::INFINITY;
        }
        (self.m2[i] / (n - 1.0) / n).sqrt()
    }

    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.aovs.iter().position(|(a, _)| a == name)
    }

    /// An AOV's pixels, normalised, top row first.
    pub fn pixels(&self, aov: usize) -> Vec<Vec4> {
        let closest = self.aovs[aov].1.closest();
        self.sums[aov]
            .iter()
            .zip(&self.weights)
            .map(|(s, w)| {
                if closest {
                    *s
                } else if w.abs() > 1e-12 {
                    *s / *w
                } else {
                    Vec4::ZERO
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_resolve() {
        let g = vec!["default".to_string(), "key".to_string()];
        assert_eq!(aov_kind("diffuse_direct", &g), Some(AovKind::Slots(vec![DIRECT])));
        assert_eq!(aov_kind("RGBA_key", &g), Some(AovKind::Group(1)));
        assert_eq!(aov_kind("specular", &g), Some(AovKind::Slots(vec![DIRECT + 1, INDIRECT + 1])));
        assert!(aov_kind("nonsense", &g).is_none());
        for n in AOV_NAMES {
            assert!(aov_kind(n, &g).is_some(), "{n}");
        }
    }

    #[test]
    fn gaussian_offsets_centre() {
        let t = FilterTable::new(FilterKind::Gaussian, 2.0);
        let (mut sum, mut sq) = (0.0, 0.0);
        for i in 0..1000 {
            let (x, s) = t.sample((i as f32 + 0.5) / 1000.0);
            assert!(x.abs() <= 1.0 && s > 0.0);
            sum += x;
            sq += x * x;
        }
        assert!((sum / 1000.0).abs() < 1e-3);
        assert!(sq / 1000.0 < 0.2);
    }
}
