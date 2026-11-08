//! A bounding volume hierarchy over item bounds. Built once (a median split on the longest axis, four items a leaf),
//! then refitted as the items move, which keeps queries tight without building again every step. Queries visit items in
//! a fixed order, the same on any thread.

use glam::DVec3;

use crate::geom::Aabb;

#[derive(Clone, Copy, Debug)]
struct Node {
    lo: DVec3,
    hi: DVec3,
    /// A leaf: items[first..first + count]. Otherwise count is 0 and the children are first and first + 1.
    first: u32,
    count: u32,
}

#[derive(Clone, Debug, Default)]
pub struct Bvh {
    nodes: Vec<Node>,
    items: Vec<u32>,
    /// How many items it was built for.
    pub len: usize,
}

const LEAF: usize = 4;

impl Bvh {
    pub fn build(bounds: &[Aabb]) -> Bvh {
        let mut items: Vec<u32> = (0..bounds.len() as u32).filter(|&i| bounds[i as usize].valid()).collect();
        let centre: Vec<DVec3> = bounds.iter().map(|b| if b.valid() { (b.lo + b.hi) * 0.5 } else { DVec3::ZERO }).collect();
        let mut nodes = vec![Node { lo: DVec3::ZERO, hi: DVec3::ZERO, first: 0, count: 0 }];
        // (node, range of items)
        let mut stack = vec![(0usize, 0usize, items.len())];
        while let Some((ni, a, b)) = stack.pop() {
            let mut bb = Aabb::EMPTY;
            for &i in &items[a..b] {
                bb.add(bounds[i as usize].lo);
                bb.add(bounds[i as usize].hi);
            }
            if b - a <= LEAF {
                nodes[ni] = Node { lo: bb.lo, hi: bb.hi, first: a as u32, count: (b - a) as u32 };
                continue;
            }
            let mut cb = Aabb::EMPTY;
            for &i in &items[a..b] {
                cb.add(centre[i as usize]);
            }
            let size = cb.size();
            let axis = if size.x >= size.y && size.x >= size.z { 0 } else if size.y >= size.z { 1 } else { 2 };
            let mid = (a + b) / 2;
            items[a..b].select_nth_unstable_by(mid - a, |&p, &q| centre[p as usize][axis].total_cmp(&centre[q as usize][axis]).then(p.cmp(&q)));
            let left = nodes.len();
            nodes.push(Node { lo: DVec3::ZERO, hi: DVec3::ZERO, first: 0, count: 0 });
            nodes.push(Node { lo: DVec3::ZERO, hi: DVec3::ZERO, first: 0, count: 0 });
            nodes[ni] = Node { lo: bb.lo, hi: bb.hi, first: left as u32, count: 0 };
            stack.push((left, a, mid));
            stack.push((left + 1, mid, b));
        }
        Bvh { nodes, items, len: bounds.len() }
    }

    /// New bounds for the same items: every node takes its children's. Children always come after their parent.
    pub fn refit(&mut self, bounds: &[Aabb]) {
        for ni in (0..self.nodes.len()).rev() {
            let n = self.nodes[ni];
            let mut bb = Aabb::EMPTY;
            if n.count > 0 {
                for &i in &self.items[n.first as usize..(n.first + n.count) as usize] {
                    if let Some(x) = bounds.get(i as usize).filter(|x| x.valid()) {
                        bb.add(x.lo);
                        bb.add(x.hi);
                    }
                }
            } else if n.first > 0 {
                for c in [n.first as usize, n.first as usize + 1] {
                    bb.add(self.nodes[c].lo);
                    bb.add(self.nodes[c].hi);
                }
            }
            self.nodes[ni].lo = bb.lo;
            self.nodes[ni].hi = bb.hi;
        }
    }

    /// Every item whose bounds meet `b` (tested against `bounds`), in a fixed order.
    #[inline]
    pub fn query(&self, b: &Aabb, bounds: &[Aabb], mut f: impl FnMut(u32)) {
        if self.items.is_empty() || !b.valid() {
            return;
        }
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let n = self.nodes[stack[top] as usize];
            if !(n.lo.cmple(b.hi).all() && b.lo.cmple(n.hi).all()) {
                continue;
            }
            if n.count > 0 {
                for &i in &self.items[n.first as usize..(n.first + n.count) as usize] {
                    if bounds[i as usize].overlaps(b) {
                        f(i);
                    }
                }
            } else if top + 2 <= stack.len() {
                // Right first so the left is visited first.
                stack[top] = n.first + 1;
                stack[top + 1] = n.first;
                top += 2;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::Rng;

    #[test]
    fn finds_exactly_what_overlaps() {
        let mut r = Rng::new(3);
        let bounds: Vec<Aabb> = (0..5000)
            .map(|_| {
                let c = DVec3::new(r.signed(), r.signed(), r.signed()) * 10.0;
                Aabb { lo: c, hi: c + DVec3::new(r.float(), r.float(), r.float()) * 0.5 }
            })
            .collect();
        let mut tree = Bvh::build(&bounds);
        for pass in 0..2 {
            for _ in 0..200 {
                let c = DVec3::new(r.signed(), r.signed(), r.signed()) * 10.0;
                let q = Aabb { lo: c, hi: c + DVec3::splat(r.float() * 2.0) };
                let mut got = Vec::new();
                tree.query(&q, &bounds, |i| got.push(i));
                got.sort_unstable();
                let want: Vec<u32> = (0..bounds.len() as u32).filter(|&i| bounds[i as usize].overlaps(&q)).collect();
                assert_eq!(got, want, "pass {pass}");
            }
            tree.refit(&bounds);
        }
    }
}
