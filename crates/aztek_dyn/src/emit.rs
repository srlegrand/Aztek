//! Emitters: where new particles come from. Each frame an emitter makes `rate * dt` particles, carrying the fraction
//! over, at places and speeds drawn from a random source seeded by the emitter and the frame, so a replay matches.

use glam::{DMat4, DVec3};

use crate::util::{hash3, Rng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmitterKind {
    /// From the position, in every direction.
    Omni,
    /// From the position, along the direction within the spread cone.
    Directional,
    /// From the surface of a mesh, along its normals.
    Surface,
    /// From inside a box or sphere placed by the matrix.
    Volume,
    /// From along a curve, given as a polyline.
    Curve,
    /// From each given point.
    Point,
}

#[derive(Clone, Debug)]
pub struct Emitter {
    pub name: String,
    pub kind: EmitterKind,
    pub enabled: bool,
    /// Particles per second.
    pub rate: f64,
    pub speed: f64,
    pub speed_random: f64,
    /// Directional: the cone's half angle as a fraction of 180 degrees (0 a line, 1 every direction).
    pub spread: f64,
    pub direction: DVec3,
    pub matrix: DMat4,
    /// Volume: sphere instead of a unit box.
    pub sphere: bool,
    /// Surface: triangles in world space; Curve and Point: points in world space.
    pub points: Vec<DVec3>,
    pub triangles: Vec<[u32; 3]>,
    /// Added to every new particle's velocity: the emitter's own motion times this.
    pub inherit_velocity: f64,
    pub velocity: DVec3,
    pub seed: u64,
    pub carry: f64,
}

impl Emitter {
    pub fn new(kind: EmitterKind) -> Emitter {
        Emitter {
            name: String::new(),
            kind,
            enabled: true,
            rate: 100.0,
            speed: 1.0,
            speed_random: 0.0,
            spread: 0.0,
            direction: DVec3::X,
            matrix: DMat4::IDENTITY,
            sphere: false,
            points: Vec::new(),
            triangles: Vec::new(),
            inherit_velocity: 0.0,
            velocity: DVec3::ZERO,
            seed: 0,
            carry: 0.0,
        }
    }

    /// The particles made over dt seconds at this frame: (position, velocity).
    pub fn emit(&mut self, frame: i64, dt: f64) -> Vec<(DVec3, DVec3)> {
        if !self.enabled || !(self.rate > 0.0) || !(dt > 0.0) {
            return Vec::new();
        }
        let want = self.rate * dt + self.carry;
        let n = want.floor().clamp(0.0, 1e6);
        self.carry = (want - n).clamp(0.0, 1.0);
        let n = n as usize;
        let mut rng = Rng::new(hash3(self.seed, frame as u64, 0x5eed));
        let at = self.matrix.w_axis.truncate();
        let dir = self.matrix.transform_vector3(self.direction).normalize_or(DVec3::X);
        let mut out = Vec::with_capacity(n);
        // Areas for picking surface triangles evenly.
        let mut areas = Vec::new();
        if self.kind == EmitterKind::Surface {
            let mut sum = 0.0;
            for t in &self.triangles {
                let [a, b, c] = t.map(|i| self.points.get(i as usize).copied().unwrap_or(DVec3::ZERO));
                sum += (b - a).cross(c - a).length() * 0.5;
                areas.push(sum);
            }
        }
        for k in 0..n {
            let speed = self.speed * (1.0 + self.speed_random * rng.signed());
            let (p, d) = match self.kind {
                EmitterKind::Omni => (at, rng.direction()),
                EmitterKind::Directional => (at, cone(dir, self.spread, &mut rng)),
                EmitterKind::Volume => {
                    let local = loop {
                        let q = DVec3::new(rng.signed(), rng.signed(), rng.signed());
                        if !self.sphere || q.length_squared() <= 1.0 {
                            break q * if self.sphere { 1.0 } else { 0.5 };
                        }
                    };
                    (self.matrix.transform_point3(local), cone(dir, self.spread, &mut rng))
                }
                EmitterKind::Surface => {
                    let total = areas.last().copied().unwrap_or(0.0);
                    if total <= 0.0 {
                        continue;
                    }
                    let pick = rng.float() * total;
                    let t = areas.partition_point(|&a| a < pick).min(self.triangles.len() - 1);
                    let [a, b, c] = self.triangles[t].map(|i| self.points.get(i as usize).copied().unwrap_or(DVec3::ZERO));
                    let (mut u, mut v) = (rng.float(), rng.float());
                    if u + v > 1.0 {
                        u = 1.0 - u;
                        v = 1.0 - v;
                    }
                    let n = (b - a).cross(c - a).normalize_or(DVec3::Y);
                    (a + (b - a) * u + (c - a) * v, cone(n, self.spread, &mut rng))
                }
                EmitterKind::Curve => {
                    if self.points.len() < 2 {
                        (self.points.first().copied().unwrap_or(at), rng.direction())
                    } else {
                        let s = rng.float() * (self.points.len() - 1) as f64;
                        let i = (s.floor() as usize).min(self.points.len() - 2);
                        let f = s - i as f64;
                        (self.points[i].lerp(self.points[i + 1], f), rng.direction())
                    }
                }
                EmitterKind::Point => {
                    if self.points.is_empty() {
                        (at, cone(dir, self.spread, &mut rng))
                    } else {
                        (self.points[k % self.points.len()], cone(dir, self.spread, &mut rng))
                    }
                }
            };
            out.push((p, d * speed + self.velocity * self.inherit_velocity));
        }
        out
    }
}

/// A direction within the cone around `axis`; spread 1 covers the sphere.
fn cone(axis: DVec3, spread: f64, rng: &mut Rng) -> DVec3 {
    let spread = spread.clamp(0.0, 1.0);
    if spread <= 0.0 {
        return axis;
    }
    let cos_max = (spread * std::f64::consts::PI).cos();
    let z = 1.0 - rng.float() * (1.0 - cos_max);
    let a = rng.float() * std::f64::consts::TAU;
    let r = (1.0 - z * z).max(0.0).sqrt();
    let (u, w) = axis.any_orthonormal_pair();
    (u * (r * a.cos()) + w * (r * a.sin()) + axis * z).normalize_or(axis)
}
