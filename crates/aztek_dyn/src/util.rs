//! Small shared pieces: a slice written from several threads at disjoint places, a seeded random source, and checks for
//! finite values.

use glam::DVec3;

/// The fewest items a thread takes in a parallel loop. Smaller loops stay on one thread: waking the pool for a few
/// hundred cheap constraints costs more than solving them.
pub const PAR_CHUNK: usize = 1024;

/// A slice that parallel loops write at indices no two of them share. The graph colouring guarantees that for the
/// constraints of one colour: they touch disjoint particles.
pub(crate) struct Shared<T> {
    ptr: *mut T,
    len: usize,
}

unsafe impl<T: Send> Send for Shared<T> {}
unsafe impl<T: Send> Sync for Shared<T> {}

impl<T: Copy> Shared<T> {
    pub fn new(v: &mut [T]) -> Self {
        Shared { ptr: v.as_mut_ptr(), len: v.len() }
    }
    /// Reads an element. Only sound while no other thread writes the same index.
    #[inline]
    pub fn get(&self, i: usize) -> T {
        assert!(i < self.len);
        unsafe { *self.ptr.add(i) }
    }
    /// Writes an element. Only sound while no other thread reads or writes the same index.
    #[inline]
    pub fn set(&self, i: usize, v: T) {
        assert!(i < self.len);
        unsafe { *self.ptr.add(i) = v }
    }
}

/// splitmix64: a seeded source of numbers, the same on every machine and thread count.
#[derive(Clone, Debug)]
pub struct Rng(pub u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// In [0, 1).
    pub fn float(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// In [-1, 1).
    pub fn signed(&mut self) -> f64 {
        self.float() * 2.0 - 1.0
    }
    /// A direction spread evenly over the sphere.
    pub fn direction(&mut self) -> DVec3 {
        let z = self.signed();
        let a = self.float() * std::f64::consts::TAU;
        let r = (1.0 - z * z).max(0.0).sqrt();
        DVec3::new(r * a.cos(), r * a.sin(), z)
    }
}

/// Mixes numbers into one seed.
pub fn hash3(a: u64, b: u64, c: u64) -> u64 {
    let mut r = Rng::new(a.wrapping_mul(0x100_0000_01B3) ^ b.rotate_left(21) ^ c.rotate_left(42));
    r.next_u64()
}

#[inline]
pub fn finite(v: DVec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

/// Shortens a vector to at most `most` long.
#[inline]
pub fn clamp_len(v: DVec3, most: f64) -> DVec3 {
    let l2 = v.length_squared();
    if l2 > most * most && l2 > 0.0 {
        v * (most / l2.sqrt())
    } else {
        v
    }
}

/// 0 to 1 with zero slope at both ends, so blends start and finish without a jolt.
#[inline]
pub fn smoothstep(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Parts switched off for finding faults: DYN_OFF=bend,limit,kin,friction. Read once.
pub fn off(part: &str) -> bool {
    static OFF: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    OFF.get_or_init(|| std::env::var("DYN_OFF").map(|v| v.split(',').map(str::to_string).collect()).unwrap_or_default()).iter().any(|p| p == part)
}
