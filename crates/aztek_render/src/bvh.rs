//! A bounding volume hierarchy over boxes: binned surface area heuristic build, refit for deforming geometry, and
//! traversal that visits the nearer child first. Used for triangles inside a mesh and for instances over the scene.

use glam::Vec3;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Default for Aabb {
    fn default() -> Self {
        Aabb::EMPTY
    }
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb { min: Vec3::splat(f32::INFINITY), max: Vec3::splat(f32::NEG_INFINITY) };
    pub fn of_points(ps: &[Vec3]) -> Aabb {
        let mut b = Aabb::EMPTY;
        for &p in ps {
            b.grow(p);
        }
        b
    }
    pub fn grow(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }
    pub fn join(&self, o: &Aabb) -> Aabb {
        Aabb { min: self.min.min(o.min), max: self.max.max(o.max) }
    }
    pub fn valid(&self) -> bool {
        self.min.x <= self.max.x
    }
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }
    pub fn area(&self) -> f32 {
        if !self.valid() {
            return 0.0;
        }
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }
    /// Entry distance of a ray, or infinity when it misses within [0, tmax].
    #[inline]
    pub fn hit(&self, o: Vec3, inv: Vec3, tmax: f32) -> f32 {
        let t0 = (self.min - o) * inv;
        let t1 = (self.max - o) * inv;
        let tn = t0.min(t1).max_element().max(0.0);
        let tf = t0.max(t1).min_element().min(tmax);
        // Robust against rounding at grazing angles (Ize 2013).
        if tn <= tf * 1.000_000_24 {
            tn
        } else {
            f32::INFINITY
        }
    }
    /// The box around this one through a matrix.
    pub fn transformed(&self, m: &glam::Mat4) -> Aabb {
        let mut b = Aabb::EMPTY;
        if !self.valid() {
            return b;
        }
        for i in 0..8 {
            let p = Vec3::new(if i & 1 == 0 { self.min.x } else { self.max.x }, if i & 2 == 0 { self.min.y } else { self.max.y }, if i & 4 == 0 { self.min.z } else { self.max.z });
            b.grow(m.transform_point3(p));
        }
        b
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Node {
    pub bounds: Aabb,
    /// First primitive of a leaf, or the left child of an inner node; the right child is next to it.
    pub first: u32,
    /// Primitives in a leaf, 0 for an inner node.
    pub count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Bvh {
    pub nodes: Vec<Node>,
    /// Primitive indices in leaf order.
    pub order: Vec<u32>,
}

const BINS: usize = 12;
const MAX_LEAF: usize = 8;

struct Prim {
    b: Aabb,
    c: Vec3,
}

impl Bvh {
    pub fn build(bounds: &[Aabb]) -> Bvh {
        let n = bounds.len();
        if n == 0 {
            return Bvh::default();
        }
        let prims: Vec<Prim> = bounds.iter().map(|b| Prim { b: *b, c: b.center() }).collect();
        let mut order: Vec<u32> = (0..n as u32).collect();
        let mut nodes = Vec::with_capacity(2 * n);
        nodes.push(Node::default());
        let mut stack = vec![(0usize, 0usize, n)];
        while let Some((ni, start, end)) = stack.pop() {
            let mut b = Aabb::EMPTY;
            let mut cb = Aabb::EMPTY;
            for &i in &order[start..end] {
                b = b.join(&prims[i as usize].b);
                cb.grow(prims[i as usize].c);
            }
            nodes[ni].bounds = b;
            let count = end - start;
            let split = if count <= 2 { None } else { best_split(&prims, &order[start..end], &b, &cb) };
            let leaf_cost = count as f32;
            let median = |order: &mut [u32]| {
                let ext = cb.max - cb.min;
                let axis = if ext.x >= ext.y && ext.x >= ext.z { 0 } else if ext.y >= ext.z { 1 } else { 2 };
                let s = &mut order[start..end];
                let h = s.len() / 2;
                s.select_nth_unstable_by(h, |a, b| prims[*a as usize].c[axis].total_cmp(&prims[*b as usize].c[axis]));
                start + h
            };
            let mid = match split {
                Some((axis, pos, cost)) if cost < leaf_cost || count > MAX_LEAF => {
                    let s = &mut order[start..end];
                    let mut l = 0;
                    for k in 0..s.len() {
                        if prims[s[k] as usize].c[axis] < pos {
                            s.swap(k, l);
                            l += 1;
                        }
                    }
                    if l == 0 || l == s.len() {
                        if count > MAX_LEAF {
                            median(&mut order)
                        } else {
                            end
                        }
                    } else {
                        start + l
                    }
                }
                _ if count > MAX_LEAF => median(&mut order),
                _ => end,
            };
            if mid == end {
                nodes[ni].first = start as u32;
                nodes[ni].count = count as u32;
                continue;
            }
            let left = nodes.len();
            nodes.push(Node::default());
            nodes.push(Node::default());
            nodes[ni].first = left as u32;
            nodes[ni].count = 0;
            stack.push((left + 1, mid, end));
            stack.push((left, start, mid));
        }
        Bvh { nodes, order }
    }

    /// New bounds for the same primitives, keeping the tree: for a mesh whose points moved.
    pub fn refit(&mut self, bounds: &[Aabb]) {
        for i in (0..self.nodes.len()).rev() {
            let n = self.nodes[i];
            let b = if n.count > 0 {
                let mut b = Aabb::EMPTY;
                for &p in &self.order[n.first as usize..(n.first + n.count) as usize] {
                    b = b.join(&bounds[p as usize]);
                }
                b
            } else {
                self.nodes[n.first as usize].bounds.join(&self.nodes[n.first as usize + 1].bounds)
            };
            self.nodes[i].bounds = b;
        }
    }

    pub fn bounds(&self) -> Aabb {
        self.nodes.first().map_or(Aabb::EMPTY, |n| n.bounds)
    }

    /// Visits leaves along a ray, nearest first. `leaf` gets each primitive and the current far limit, and returns a
    /// new far limit (smaller on a hit), or a negative number to stop at once.
    #[inline]
    pub fn traverse(&self, o: Vec3, d: Vec3, mut tmax: f32, mut leaf: impl FnMut(u32, f32) -> f32) -> f32 {
        if self.nodes.is_empty() {
            return tmax;
        }
        let inv = Vec3::new(safe_inv(d.x), safe_inv(d.y), safe_inv(d.z));
        let mut stack = [0u32; 64];
        let mut sp = 0usize;
        if self.nodes[0].bounds.hit(o, inv, tmax) == f32::INFINITY {
            return tmax;
        }
        let mut at = 0u32;
        loop {
            let n = &self.nodes[at as usize];
            if n.count > 0 {
                for k in n.first..n.first + n.count {
                    let r = leaf(self.order[k as usize], tmax);
                    if r < 0.0 {
                        return r;
                    }
                    tmax = r;
                }
            } else {
                let (a, b) = (n.first, n.first + 1);
                let ta = self.nodes[a as usize].bounds.hit(o, inv, tmax);
                let tb = self.nodes[b as usize].bounds.hit(o, inv, tmax);
                let (near, far, tn, tf) = if ta <= tb { (a, b, ta, tb) } else { (b, a, tb, ta) };
                if tn != f32::INFINITY {
                    if tf != f32::INFINITY && sp < 64 {
                        stack[sp] = far;
                        sp += 1;
                    }
                    at = near;
                    continue;
                }
            }
            // Pop, skipping nodes the current limit rules out.
            loop {
                if sp == 0 {
                    return tmax;
                }
                sp -= 1;
                let c = stack[sp];
                if self.nodes[c as usize].bounds.hit(o, inv, tmax) != f32::INFINITY {
                    at = c;
                    break;
                }
            }
        }
    }
}

#[inline]
fn safe_inv(x: f32) -> f32 {
    if x.abs() < 1e-30 {
        1e30f32.copysign(x)
    } else {
        1.0 / x
    }
}

fn best_split(prims: &[Prim], ids: &[u32], _b: &Aabb, cb: &Aabb) -> Option<(usize, f32, f32)> {
    let ext = cb.max - cb.min;
    let mut best: Option<(usize, f32, f32)> = None;
    for axis in 0..3 {
        if ext[axis] <= 1e-12 {
            continue;
        }
        let mut bins = [(Aabb::EMPTY, 0u32); BINS];
        let scale = BINS as f32 / ext[axis];
        for &i in ids {
            let p = &prims[i as usize];
            let k = (((p.c[axis] - cb.min[axis]) * scale) as usize).min(BINS - 1);
            bins[k].0 = bins[k].0.join(&p.b);
            bins[k].1 += 1;
        }
        let mut right_area = [0.0f32; BINS];
        let mut right_count = [0u32; BINS];
        let (mut rb, mut rc) = (Aabb::EMPTY, 0);
        for k in (1..BINS).rev() {
            rb = rb.join(&bins[k].0);
            rc += bins[k].1;
            right_area[k] = rb.area();
            right_count[k] = rc;
        }
        let (mut lb, mut lc) = (Aabb::EMPTY, 0);
        for k in 0..BINS - 1 {
            lb = lb.join(&bins[k].0);
            lc += bins[k].1;
            if lc == 0 || right_count[k + 1] == 0 {
                continue;
            }
            let cost = lb.area() * lc as f32 + right_area[k + 1] * right_count[k + 1] as f32;
            if best.is_none_or(|b| cost < b.2) {
                best = Some((axis, cb.min[axis] + (k + 1) as f32 / scale, cost));
            }
        }
    }
    // Relative to the parent's area, with the cost of one traversal step.
    let area = _b.area().max(1e-30);
    best.map(|(a, p, c)| (a, p, 0.5 + c / area))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nearest_box() {
        let boxes: Vec<Aabb> = (0..100).map(|i| Aabb { min: Vec3::new(i as f32 * 2.0, 0.0, 0.0), max: Vec3::new(i as f32 * 2.0 + 1.0, 1.0, 1.0) }).collect();
        let bvh = Bvh::build(&boxes);
        let mut hit = None;
        bvh.traverse(Vec3::new(-5.0, 0.5, 0.5), Vec3::X, 1e30, |p, tmax| {
            let t = boxes[p as usize].hit(Vec3::new(-5.0, 0.5, 0.5), Vec3::new(1.0, 1e30, 1e30), tmax);
            if t < tmax {
                hit = Some(p);
                t
            } else {
                tmax
            }
        });
        assert_eq!(hit, Some(0));
        let mut b2 = bvh.clone();
        let moved: Vec<Aabb> = boxes.iter().map(|b| Aabb { min: b.min + Vec3::Y * 3.0, max: b.max + Vec3::Y * 3.0 }).collect();
        b2.refit(&moved);
        assert!((b2.bounds().min.y - 3.0).abs() < 1e-6);
    }
}
