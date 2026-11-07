//! Fields: forces in space that act on whatever in the solver they are connected to.
//!
//! Each field gives a force at a point; the solver divides it by the point's mass, except gravity, which is an
//! acceleration. Falloff: `magnitude * 1 / (1 + d)^attenuation`, and with `use_max_distance` also
//! `* (1 - d / max_distance)` and nothing past it. d is the distance from the field's position, or from its axis for
//! the vortex and volume axis fields.

use glam::{DMat4, DVec3};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldKind {
    /// Constant acceleration along `direction`.
    Gravity,
    /// Pushes velocity towards `direction * speed`: wind.
    Air,
    /// Slows motion; with a direction, only motion along it.
    Drag,
    /// Pulls towards the field's position, scaled by the point's mass over the distance squared.
    Newton,
    /// Pushes away from the position (negative magnitude pulls).
    Radial,
    /// Noise that changes over space and time.
    Turbulence,
    /// Constant force along `direction`.
    Uniform,
    /// Swirls around `direction` through the position.
    Vortex,
    /// Away from, along and around an axis.
    VolumeAxis,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub kind: FieldKind,
    pub enabled: bool,
    pub magnitude: f64,
    pub attenuation: f64,
    pub max_distance: f64,
    pub use_max_distance: bool,
    /// World placement: the position is the translation; directions are given in world space.
    pub matrix: DMat4,
    pub direction: DVec3,
    /// Air: the wind's speed; drag uses `direction` only when `use_direction`.
    pub speed: f64,
    pub use_direction: bool,
    /// Air: how much of the existing motion is left alone (0 replaces it, 1 only adds).
    pub inherit_velocity: f64,
    /// Newton: distances under this count as this.
    pub min_distance: f64,
    /// Turbulence: noise frequency and phase.
    pub frequency: f64,
    pub phase: DVec3,
    /// Volume axis: how much pushes away from, along and around the axis.
    pub away_from_axis: f64,
    pub along_axis: f64,
    pub around_axis: f64,
    /// Bodies it acts on, by index; empty acts on none, `all` on every one.
    pub bodies: Vec<u32>,
    pub all: bool,
}

impl Field {
    pub fn new(kind: FieldKind) -> Field {
        let (magnitude, attenuation, direction) = match kind {
            FieldKind::Gravity => (9.8, 0.0, DVec3::NEG_Y),
            FieldKind::Air => (4.0, 0.0, DVec3::X),
            FieldKind::Drag => (0.05, 0.0, DVec3::X),
            FieldKind::Newton => (1.0, 1.0, DVec3::Y),
            FieldKind::Radial => (1.0, 1.0, DVec3::Y),
            FieldKind::Turbulence => (5.0, 1.0, DVec3::Y),
            FieldKind::Uniform => (1.0, 1.0, DVec3::X),
            FieldKind::Vortex => (5.0, 1.0, DVec3::Y),
            FieldKind::VolumeAxis => (5.0, 0.0, DVec3::Y),
        };
        Field {
            name: String::new(),
            kind,
            enabled: true,
            magnitude,
            attenuation,
            max_distance: 20.0,
            use_max_distance: matches!(kind, FieldKind::Air | FieldKind::Radial),
            matrix: DMat4::IDENTITY,
            direction,
            speed: 5.0,
            use_direction: false,
            inherit_velocity: 0.0,
            min_distance: 0.2,
            frequency: 1.0,
            phase: DVec3::ZERO,
            away_from_axis: 1.0,
            along_axis: 0.0,
            around_axis: 0.0,
            bodies: Vec::new(),
            all: false,
        }
    }

    pub fn acts_on(&self, body: u32) -> bool {
        self.enabled && (self.all || self.bodies.contains(&body))
    }

    fn falloff(&self, d: f64) -> f64 {
        let mut f = if self.attenuation != 0.0 { (1.0 / (1.0 + d)).powf(self.attenuation) } else { 1.0 };
        if self.use_max_distance {
            if d > self.max_distance {
                return 0.0;
            }
            f *= 1.0 - d / self.max_distance.max(1e-9);
        }
        f
    }

    /// (force, is an acceleration) at x moving at v, at time t seconds.
    pub fn force(&self, x: DVec3, v: DVec3, mass: f64, t: f64) -> (DVec3, bool) {
        let at = self.matrix.w_axis.truncate();
        let dir = self.direction.normalize_or_zero();
        let rel = x - at;
        let d = rel.length();
        match self.kind {
            FieldKind::Gravity => (dir * self.magnitude * self.falloff(d), true),
            FieldKind::Uniform => (dir * self.magnitude * self.falloff(d), false),
            FieldKind::Air => {
                let wind = dir * self.speed;
                let along = if self.inherit_velocity > 0.0 { v * (1.0 - self.inherit_velocity) } else { v };
                ((wind - along) * self.magnitude * 0.1 * self.falloff(d) * mass, false)
            }
            FieldKind::Drag => {
                let along = if self.use_direction && dir != DVec3::ZERO { dir * v.dot(dir) } else { v };
                (-along * self.magnitude * self.falloff(d) * mass, false)
            }
            FieldKind::Newton => {
                let r = d.max(self.min_distance.max(1e-6));
                (-rel.normalize_or_zero() * self.magnitude * mass / (r * r) * self.falloff(0.0) * if self.use_max_distance && d > self.max_distance { 0.0 } else { 1.0 }, false)
            }
            FieldKind::Radial => (rel.normalize_or_zero() * self.magnitude * self.falloff(d), false),
            FieldKind::Turbulence => {
                let q = x * self.frequency + self.phase + DVec3::splat(t * 0.5);
                let n = DVec3::new(noise(q), noise(q + DVec3::new(31.4, 7.1, 2.9)), noise(q + DVec3::new(-5.3, 19.7, 41.1)));
                (n * self.magnitude * self.falloff(d), false)
            }
            FieldKind::Vortex | FieldKind::VolumeAxis => {
                let axis = if dir == DVec3::ZERO { DVec3::Y } else { dir };
                let radial = rel - axis * rel.dot(axis);
                let r = radial.length();
                let around = axis.cross(radial).normalize_or_zero();
                let f = self.falloff(r);
                if self.kind == FieldKind::Vortex {
                    (around * self.magnitude * f, false)
                } else {
                    let out = radial.normalize_or_zero() * self.away_from_axis + axis * self.along_axis + around * self.around_axis;
                    (out * self.magnitude * f, false)
                }
            }
        }
    }
}

/// Smooth value noise in [-1, 1], the same everywhere it runs.
pub fn noise(p: DVec3) -> f64 {
    let i = p.floor();
    let f = p - i;
    let u = f * f * (DVec3::splat(3.0) - f * 2.0);
    let h = |x: f64, y: f64, z: f64| {
        let k = crate::util::hash3(x as i64 as u64, y as i64 as u64, z as i64 as u64);
        (k >> 11) as f64 / (1u64 << 52) as f64 - 1.0
    };
    let (x, y, z) = (i.x, i.y, i.z);
    let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
    let x00 = lerp(h(x, y, z), h(x + 1.0, y, z), u.x);
    let x10 = lerp(h(x, y + 1.0, z), h(x + 1.0, y + 1.0, z), u.x);
    let x01 = lerp(h(x, y, z + 1.0), h(x + 1.0, y, z + 1.0), u.x);
    let x11 = lerp(h(x, y + 1.0, z + 1.0), h(x + 1.0, y + 1.0, z + 1.0), u.x);
    lerp(lerp(x00, x10, u.y), lerp(x01, x11, u.y), u.z)
}
