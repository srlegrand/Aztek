//! Closest points, barycentric coordinates and the inside test for meshes.

use glam::DVec3;

/// Which part of a triangle a closest point lies on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Feature {
    Face,
    Edge,
    Vertex,
}

/// The closest point on triangle abc to p, as barycentric weights (on a, b, c) and the feature it lies on.
/// Ericson, Real-Time Collision Detection 5.1.5.
pub fn closest_on_triangle(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> (DVec3, Feature) {
    let ab = b - a;
    let ac = c - a;
    let ap = p - a;
    let d1 = ab.dot(ap);
    let d2 = ac.dot(ap);
    if d1 <= 0.0 && d2 <= 0.0 {
        return (DVec3::new(1.0, 0.0, 0.0), Feature::Vertex);
    }
    let bp = p - b;
    let d3 = ab.dot(bp);
    let d4 = ac.dot(bp);
    if d3 >= 0.0 && d4 <= d3 {
        return (DVec3::new(0.0, 1.0, 0.0), Feature::Vertex);
    }
    let vc = d1 * d4 - d3 * d2;
    if vc <= 0.0 && d1 >= 0.0 && d3 <= 0.0 {
        let v = d1 / (d1 - d3);
        return (DVec3::new(1.0 - v, v, 0.0), Feature::Edge);
    }
    let cp = p - c;
    let d5 = ab.dot(cp);
    let d6 = ac.dot(cp);
    if d6 >= 0.0 && d5 <= d6 {
        return (DVec3::new(0.0, 0.0, 1.0), Feature::Vertex);
    }
    let vb = d5 * d2 - d1 * d6;
    if vb <= 0.0 && d2 >= 0.0 && d6 <= 0.0 {
        let w = d2 / (d2 - d6);
        return (DVec3::new(1.0 - w, 0.0, w), Feature::Edge);
    }
    let va = d3 * d6 - d5 * d4;
    if va <= 0.0 && (d4 - d3) >= 0.0 && (d5 - d6) >= 0.0 {
        let w = (d4 - d3) / ((d4 - d3) + (d5 - d6));
        return (DVec3::new(0.0, 1.0 - w, w), Feature::Edge);
    }
    let denom = va + vb + vc;
    if denom.abs() < 1e-300 || !denom.is_finite() {
        // Degenerate: fall back to the nearest corner.
        let (da, db, dc) = (ap.length_squared(), bp.length_squared(), cp.length_squared());
        let w = if da <= db && da <= dc {
            DVec3::X
        } else if db <= dc {
            DVec3::Y
        } else {
            DVec3::Z
        };
        return (w, Feature::Vertex);
    }
    let v = vb / denom;
    let w = vc / denom;
    (DVec3::new(1.0 - v - w, v, w), Feature::Face)
}

#[inline]
pub fn at(w: DVec3, a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    a * w.x + b * w.y + c * w.z
}

/// The triangle's unit normal, or zero for a degenerate one.
#[inline]
pub fn normal(a: DVec3, b: DVec3, c: DVec3) -> DVec3 {
    (b - a).cross(c - a).normalize_or_zero()
}

/// The solid angle triangle abc spans seen from p, over 4 pi: summed over a closed mesh, 1 inside and 0 outside.
pub fn winding(p: DVec3, a: DVec3, b: DVec3, c: DVec3) -> f64 {
    let (a, b, c) = (a - p, b - p, c - p);
    let (la, lb, lc) = (a.length(), b.length(), c.length());
    let num = a.dot(b.cross(c));
    let den = la * lb * lc + a.dot(b) * lc + b.dot(c) * la + c.dot(a) * lb;
    2.0 * num.atan2(den) / (4.0 * std::f64::consts::PI)
}

/// Axis-aligned bounds.
#[derive(Clone, Copy, Debug)]
pub struct Aabb {
    pub lo: DVec3,
    pub hi: DVec3,
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb { lo: DVec3::splat(f64::INFINITY), hi: DVec3::splat(f64::NEG_INFINITY) };
    pub fn of(points: &[DVec3]) -> Aabb {
        let mut b = Aabb::EMPTY;
        for &p in points {
            b.add(p);
        }
        b
    }
    #[inline]
    pub fn add(&mut self, p: DVec3) {
        self.lo = self.lo.min(p);
        self.hi = self.hi.max(p);
    }
    pub fn overlaps(&self, o: &Aabb) -> bool {
        self.lo.cmple(o.hi).all() && o.lo.cmple(self.hi).all()
    }
    pub fn grow(self, r: f64) -> Aabb {
        Aabb { lo: self.lo - DVec3::splat(r), hi: self.hi + DVec3::splat(r) }
    }
    pub fn valid(&self) -> bool {
        self.lo.x <= self.hi.x && super::util::finite(self.lo) && super::util::finite(self.hi)
    }
    pub fn size(&self) -> DVec3 {
        if self.valid() {
            self.hi - self.lo
        } else {
            DVec3::ZERO
        }
    }
}
