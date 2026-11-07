//! Small maths shared by the renderer: rays, frames, sampling warps, hashing and the low discrepancy sequence.

use glam::{Mat4, Vec2, Vec3};

pub const PI: f32 = std::f32::consts::PI;
pub const INV_PI: f32 = 1.0 / PI;
pub const TAU: f32 = 2.0 * PI;

#[derive(Clone, Copy, Debug)]
pub struct Ray {
    pub o: Vec3,
    pub d: Vec3,
    /// Shutter time, 0 to 1.
    pub time: f32,
}

impl Ray {
    pub fn new(o: Vec3, d: Vec3, time: f32) -> Ray {
        Ray { o, d, time }
    }
    pub fn at(&self, t: f32) -> Vec3 {
        self.o + self.d * t
    }
}

/// An origin pushed off a surface along its geometric normal, scaled to the magnitude of the point so large scenes keep
/// working in single precision.
pub fn offset(p: Vec3, ng: Vec3, outward: bool) -> Vec3 {
    let scale = 1e-4 * (1.0 + p.abs().max_element());
    if outward {
        p + ng * scale
    } else {
        p - ng * scale
    }
}

/// An orthonormal frame around a unit normal, without branches that cost precision (Duff et al. 2017).
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub t: Vec3,
    pub b: Vec3,
    pub n: Vec3,
}

impl Frame {
    pub fn from_n(n: Vec3) -> Frame {
        let s = 1.0f32.copysign(n.z);
        let a = -1.0 / (s + n.z);
        let b = n.x * n.y * a;
        let t = Vec3::new(1.0 + s * n.x * n.x * a, s * b, -s * n.x);
        let bb = Vec3::new(b, s + n.y * n.y * a, -n.y);
        Frame { t, b: bb, n }
    }
    /// A frame whose first axis follows a tangent where it can.
    pub fn from_nt(n: Vec3, t: Vec3) -> Frame {
        let t = (t - n * n.dot(t)).normalize_or_zero();
        if t.length_squared() < 0.5 {
            return Frame::from_n(n);
        }
        Frame { t, b: n.cross(t), n }
    }
    pub fn to_local(&self, v: Vec3) -> Vec3 {
        Vec3::new(v.dot(self.t), v.dot(self.b), v.dot(self.n))
    }
    pub fn to_world(&self, v: Vec3) -> Vec3 {
        self.t * v.x + self.b * v.y + self.n * v.z
    }
}

pub fn cosine_hemisphere(u: Vec2) -> Vec3 {
    let d = concentric_disk(u);
    let z = (1.0 - d.length_squared()).max(0.0).sqrt();
    Vec3::new(d.x, d.y, z)
}

pub fn concentric_disk(u: Vec2) -> Vec2 {
    let a = 2.0 * u - Vec2::ONE;
    if a.x == 0.0 && a.y == 0.0 {
        return Vec2::ZERO;
    }
    let (r, th) = if a.x.abs() > a.y.abs() { (a.x, PI / 4.0 * (a.y / a.x)) } else { (a.y, PI / 2.0 - PI / 4.0 * (a.x / a.y)) };
    Vec2::new(r * th.cos(), r * th.sin())
}

pub fn uniform_sphere(u: Vec2) -> Vec3 {
    let z = 1.0 - 2.0 * u.x;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = TAU * u.y;
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

/// A direction in a cone about +z of the given cosine, uniform in solid angle.
pub fn uniform_cone(u: Vec2, cos_max: f32) -> Vec3 {
    let z = 1.0 - u.x * (1.0 - cos_max);
    let r = (1.0 - z * z).max(0.0).sqrt();
    let phi = TAU * u.y;
    Vec3::new(r * phi.cos(), r * phi.sin(), z)
}

pub fn uniform_triangle(u: Vec2) -> (f32, f32) {
    // Heitz's low distortion mapping, area preserving.
    let (b0, b1);
    if u.y > u.x {
        b0 = u.x / 2.0;
        b1 = u.y - b0;
    } else {
        b1 = u.y / 2.0;
        b0 = u.x - b1;
    }
    (b0, b1)
}

pub fn power_heuristic(a: f32, b: f32) -> f32 {
    let (a2, b2) = (a * a, b * b);
    if a2 + b2 <= 0.0 || !a2.is_finite() {
        return if a > 0.0 { 1.0 } else { 0.0 };
    }
    a2 / (a2 + b2)
}

pub fn luminance(c: Vec3) -> f32 {
    c.dot(Vec3::new(0.2126, 0.7152, 0.0722))
}

pub fn reflect(v: Vec3, n: Vec3) -> Vec3 {
    2.0 * v.dot(n) * n - v
}

/// A point through a matrix in single precision.
pub fn xf_point(m: &Mat4, p: Vec3) -> Vec3 {
    m.transform_point3(p)
}

// ---------------------------------------------------------------------------------------------------------------------
// Hashing and random numbers

#[inline]
pub fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

#[inline]
pub fn hash3(a: u32, b: u32, c: u32) -> u32 {
    hash_u32(a ^ hash_u32(b ^ hash_u32(c.wrapping_add(0x9e37_79b9))))
}

#[inline]
pub fn to_unit(x: u32) -> f32 {
    // 24 bits, so the result is strictly below 1.
    (x >> 8) as f32 * (1.0 / 16_777_216.0)
}

/// A small fast generator for the dimensions past what the sequence covers.
#[derive(Clone, Debug)]
pub struct Pcg {
    state: u64,
}

impl Pcg {
    pub fn new(seed: u64) -> Pcg {
        let mut p = Pcg { state: seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407) };
        p.next_u32();
        p
    }
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let xs = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xs.rotate_right(rot)
    }
    pub fn next_f32(&mut self) -> f32 {
        to_unit(self.next_u32())
    }
    pub fn next_2d(&mut self) -> Vec2 {
        Vec2::new(self.next_f32(), self.next_f32())
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Sobol, Owen scrambled: the first two dimensions of the Sobol sequence form a (0,2)-sequence, which stratifies any
// power of two count of samples. Each pair of dimensions a sampler asks for is scrambled with its own seed and the
// sample index is shuffled per pixel, so the pairs are decorrelated (Burley 2020).

const SOBOL_DIM1: [u32; 32] = {
    let mut m = [0u32; 32];
    // Direction numbers for the second dimension: m_k = 1, v_k = v_{k-1} ^ (v_{k-1} >> 1).
    let mut v: u32 = 1 << 31;
    let mut i = 0;
    while i < 32 {
        m[i] = v;
        v ^= v >> 1;
        i += 1;
    }
    m
};

#[inline]
fn sobol0(i: u32) -> u32 {
    i.reverse_bits()
}

#[inline]
fn sobol1(mut i: u32) -> u32 {
    let mut r = 0u32;
    let mut k = 0;
    while i != 0 {
        if i & 1 != 0 {
            r ^= SOBOL_DIM1[k];
        }
        i >>= 1;
        k += 1;
    }
    r
}

/// Laine-Karras style nested uniform scramble on reversed bits.
#[inline]
fn lk_permute(mut x: u32, seed: u32) -> u32 {
    x = x.wrapping_add(seed);
    x ^= x.wrapping_mul(0x6c50_b47c);
    x ^= x.wrapping_mul(0xb82f_1e52);
    x ^= x.wrapping_mul(0xc7af_e638);
    x ^= x.wrapping_mul(0x8d22_f6e6);
    x
}

#[inline]
fn owen(x: u32, seed: u32) -> u32 {
    lk_permute(x.reverse_bits(), seed).reverse_bits()
}

/// One pixel sample's stream of dimensions.
#[derive(Clone, Debug)]
pub struct Sampler {
    index: u32,
    seed: u32,
    dim: u32,
    rng: Pcg,
}

impl Sampler {
    pub fn new(px: u32, py: u32, index: u32, seed: u32) -> Sampler {
        let pixel = hash3(px, py, seed);
        Sampler { index, seed: pixel, dim: 0, rng: Pcg::new(((pixel as u64) << 32) | index as u64) }
    }
    /// A 2D sample from the next pair of dimensions. The first several pairs are scrambled Sobol, after that random.
    pub fn next_2d(&mut self) -> Vec2 {
        let d = self.dim;
        self.dim += 1;
        if d >= 24 {
            return self.rng.next_2d();
        }
        let s = hash_u32(self.seed ^ d.wrapping_mul(0x9e37_79b9));
        let i = owen(self.index, s);
        let x = owen(sobol0(i), hash_u32(s ^ 0x1));
        let y = owen(sobol1(i), hash_u32(s ^ 0x2));
        Vec2::new(to_unit(x), to_unit(y))
    }
    pub fn next_1d(&mut self) -> f32 {
        self.next_2d().x
    }
    /// Random numbers for choices that must not use up sequence dimensions.
    pub fn rng(&mut self) -> &mut Pcg {
        &mut self.rng
    }
    /// The stream for one of n split samples at a hit: a fresh set of dimensions, stratified over the n.
    pub fn split(&self, k: u32, n: u32, salt: u32) -> Sampler {
        Sampler { index: self.index.wrapping_mul(n).wrapping_add(k), seed: hash3(self.seed, salt, self.dim), dim: 0, rng: Pcg::new(((hash_u32(self.seed ^ salt) as u64) << 32) | (self.index.wrapping_mul(n).wrapping_add(k)) as u64) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sobol_pairs_are_stratified() {
        // Any 16 consecutive points from the start fall one per cell of a 4 x 4 grid.
        let mut cells = [0; 16];
        for i in 0..16u32 {
            let x = to_unit(sobol0(i));
            let y = to_unit(sobol1(i));
            cells[(x * 4.0) as usize + 4 * (y * 4.0) as usize] += 1;
        }
        assert!(cells.iter().all(|&c| c == 1), "{cells:?}");
    }

    #[test]
    fn sampler_mean_is_half() {
        let mut sum = Vec2::ZERO;
        let n = 4096;
        for i in 0..n {
            let mut s = Sampler::new(3, 7, i, 1);
            sum += s.next_2d();
        }
        let m = sum / n as f32;
        assert!((m.x - 0.5).abs() < 0.01 && (m.y - 0.5).abs() < 0.01, "{m}");
    }

    #[test]
    fn frame_is_orthonormal() {
        for n in [Vec3::Z, -Vec3::Z, Vec3::new(0.3, -0.5, 0.8).normalize(), Vec3::X] {
            let f = Frame::from_n(n);
            assert!(f.t.dot(f.b).abs() < 1e-5 && f.t.dot(n).abs() < 1e-5 && (f.t.length() - 1.0).abs() < 1e-5);
            let v = Vec3::new(0.2, 0.4, 0.9);
            assert!((f.to_local(f.to_world(v)) - v).length() < 1e-5);
        }
    }
}
