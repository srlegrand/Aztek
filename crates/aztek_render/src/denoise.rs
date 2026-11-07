//! A simple denoiser: non-local means on the beauty, each neighbour's weight also falling with how different its albedo
//! and normal are, so edges between materials and shapes stay sharp. Pure Rust, parallel over rows. Off by default.

use glam::{Vec3, Vec4};
use rayon::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct DenoiseSettings {
    /// Search radius in pixels.
    pub radius: i32,
    /// Patch radius in pixels.
    pub patch: i32,
    /// Filter strength on colour differences.
    pub strength: f32,
    pub albedo_sigma: f32,
    pub normal_sigma: f32,
}

impl Default for DenoiseSettings {
    fn default() -> Self {
        DenoiseSettings { radius: 7, patch: 1, strength: 0.45, albedo_sigma: 0.08, normal_sigma: 0.25 }
    }
}

fn tonemap(c: Vec3) -> Vec3 {
    c.max(Vec3::ZERO) / (Vec3::ONE + c.max(Vec3::ZERO))
}

/// Denoises `color`, guided by `albedo` and `normal` where given.
pub fn denoise(w: u32, h: u32, color: &[Vec4], albedo: Option<&[Vec4]>, normal: Option<&[Vec4]>, s: &DenoiseSettings) -> Vec<Vec4> {
    let (w, h) = (w as i32, h as i32);
    let tm: Vec<Vec3> = color.iter().map(|c| tonemap(c.truncate())).collect();
    let at = |x: i32, y: i32| (y.clamp(0, h - 1) * w + x.clamp(0, w - 1)) as usize;
    let k2 = s.strength * s.strength;
    let pn = ((2 * s.patch + 1) * (2 * s.patch + 1)) as f32;
    (0..h)
        .into_par_iter()
        .flat_map_iter(|y| {
            let tm = &tm;
            (0..w).map(move |x| {
                let i = at(x, y);
                let mut sum = Vec4::ZERO;
                let mut wsum = 0.0f32;
                for dy in -s.radius..=s.radius {
                    for dx in -s.radius..=s.radius {
                        let (nx, ny) = (x + dx, y + dy);
                        if nx < 0 || ny < 0 || nx >= w || ny >= h {
                            continue;
                        }
                        let j = at(nx, ny);
                        let mut d = 0.0f32;
                        for py in -s.patch..=s.patch {
                            for px in -s.patch..=s.patch {
                                let a = tm[at(x + px, y + py)];
                                let b = tm[at(nx + px, ny + py)];
                                d += (a - b).length_squared();
                            }
                        }
                        let mut wt = (-(d / pn) / k2 * 30.0).exp();
                        if let Some(al) = albedo {
                            wt *= (-(al[i] - al[j]).truncate().length_squared() / (s.albedo_sigma * s.albedo_sigma)).exp();
                        }
                        if let Some(n) = normal {
                            wt *= (-(n[i] - n[j]).truncate().length_squared() / (s.normal_sigma * s.normal_sigma)).exp();
                        }
                        sum += color[j] * wt;
                        wsum += wt;
                    }
                }
                if wsum > 0.0 {
                    sum / wsum
                } else {
                    color[i]
                }
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_noise_goes_down_edges_stay() {
        let (w, h) = (32u32, 32u32);
        let mut rng = crate::math::Pcg::new(5);
        let mut img = Vec::new();
        let mut albedo = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let base = if x < 16 { 0.2 } else { 0.8 };
                let _ = y;
                img.push(Vec4::new(base + (rng.next_f32() - 0.5) * 0.1, base, base, 1.0));
                albedo.push(Vec4::splat(base));
            }
        }
        let out = denoise(w, h, &img, Some(&albedo), None, &DenoiseSettings::default());
        let var = |v: &[Vec4]| {
            let xs: Vec<f32> = (0..h as usize).flat_map(|y| (0..8).map(move |x| y * 32 + x)).map(|i| v[i].x).collect();
            let m = xs.iter().sum::<f32>() / xs.len() as f32;
            xs.iter().map(|x| (x - m) * (x - m)).sum::<f32>() / xs.len() as f32
        };
        assert!(var(&out) < var(&img) * 0.5);
        assert!((out[16 * 32 + 15].x - 0.2).abs() < 0.05 && (out[16 * 32 + 16].x - 0.8).abs() < 0.05);
    }
}
