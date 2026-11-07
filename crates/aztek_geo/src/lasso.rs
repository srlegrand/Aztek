//! A freehand lasso in screen space: the outline the cursor drew, closed from its last point back to its first.

use glam::DVec2;

/// Even-odd containment, so a lasso drawn crossing itself leaves its overlaps out. Fewer than 3 points hold nothing.
pub fn contains(poly: &[DVec2], p: DVec2) -> bool {
    if poly.len() < 3 {
        return false;
    }
    let mut inside = false;
    let mut j = poly.len() - 1;
    for i in 0..poly.len() {
        let a = poly[i];
        let b = poly[j];
        if (a.y > p.y) != (b.y > p.y) && p.x < (b.x - a.x) * (p.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Whether segments ab and cd cross or touch.
pub fn segments_cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let cross = |o: DVec2, p: DVec2, q: DVec2| (p.x - o.x) * (q.y - o.y) - (p.y - o.y) * (q.x - o.x);
    let within =
        |o: DVec2, p: DVec2, q: DVec2| o.x.min(p.x) <= q.x && q.x <= o.x.max(p.x) && o.y.min(p.y) <= q.y && q.y <= o.y.max(p.y);
    let d1 = cross(c, d, a);
    let d2 = cross(c, d, b);
    let d3 = cross(a, b, c);
    let d4 = cross(a, b, d);
    if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0)) && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0)) {
        return true;
    }
    (d1 == 0.0 && within(c, d, a))
        || (d2 == 0.0 && within(c, d, b))
        || (d3 == 0.0 && within(a, b, c))
        || (d4 == 0.0 && within(a, b, d))
}

/// Whether a closed region and a shape overlap at all: a point of either inside the other, or outlines crossing. A
/// shape of two points is a segment.
pub fn touches(region: &[DVec2], shape: &[DVec2]) -> bool {
    if shape.iter().any(|&p| contains(region, p)) {
        return true;
    }
    if !region.is_empty() && contains(shape, region[0]) {
        return true;
    }
    if region.is_empty() || shape.is_empty() {
        return false;
    }
    let shape_edges = if shape.len() == 2 { 1 } else { shape.len() };
    let mut j = region.len() - 1;
    for i in 0..region.len() {
        for k in 0..shape_edges {
            if segments_cross(region[j], region[i], shape[k], shape[(k + 1) % shape.len()]) {
                return true;
            }
        }
        j = i;
    }
    false
}

/// Appends p unless it is within min_distance of the last point (2 in the editor). Returns whether it was added.
pub fn extend(poly: &mut Vec<DVec2>, p: DVec2, min_distance: f64) -> bool {
    if poly.last().is_some_and(|l| l.distance(p) < min_distance) {
        return false;
    }
    poly.push(p);
    true
}

/// Min and max corners; both zero when empty.
pub fn bounds(poly: &[DVec2]) -> (DVec2, DVec2) {
    let Some(&first) = poly.first() else { return (DVec2::ZERO, DVec2::ZERO) };
    poly.iter().fold((first, first), |(lo, hi), &p| (lo.min(p), hi.max(p)))
}
