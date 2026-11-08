//! A spatial hash: items by the grid cells their bounds cover, counted and sorted into one flat table. Built serially,
//! so the order of what a query visits never depends on threads.

use glam::DVec3;

use crate::geom::Aabb;

pub struct SpatialHash {
    cell: f64,
    mask: u64,
    /// starts[b]..starts[b + 1] index `ids` for bucket b.
    starts: Vec<u32>,
    ids: Vec<u32>,
    big: Vec<u32>,
}

/// An item covering more cells than this on any axis goes on a list every query returns, so it never makes the table
/// huge (a floor of two triangles under small cloth).
const MOST_CELLS: i64 = 8;

/// A cell's bucket key, mixed so that neighbouring cells spread over the whole table.
#[inline]
fn key(x: i64, y: i64, z: i64) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F).rotate_left(21) ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9).rotate_left(42);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

impl SpatialHash {
    pub fn empty() -> Self {
        SpatialHash { cell: 1.0, mask: 0, starts: vec![0, 0], ids: Vec::new(), big: Vec::new() }
    }

    #[inline]
    fn cell_of(&self, p: DVec3) -> (i64, i64, i64) {
        let s = 1.0 / self.cell;
        ((p.x * s).floor() as i64, (p.y * s).floor() as i64, (p.z * s).floor() as i64)
    }

    /// The cell range the bounds cover, and whether it was cut short.
    fn cells(&self, b: &Aabb) -> Option<((i64, i64, i64), (i64, i64, i64), bool)> {
        if !b.valid() {
            return None;
        }
        let lo = self.cell_of(b.lo);
        let mut hi = self.cell_of(b.hi);
        let cut = hi.0 - lo.0 > MOST_CELLS || hi.1 - lo.1 > MOST_CELLS || hi.2 - lo.2 > MOST_CELLS;
        hi.0 = hi.0.min(lo.0 + MOST_CELLS);
        hi.1 = hi.1.min(lo.1 + MOST_CELLS);
        hi.2 = hi.2.min(lo.2 + MOST_CELLS);
        Some((lo, hi, cut))
    }

    /// Builds from item bounds; `cell` is the grid spacing.
    pub fn build(cell: f64, items: &[Aabb]) -> Self {
        let cell = if cell.is_finite() && cell > 1e-9 { cell } else { 1.0 };
        let buckets = (items.len() * 2).max(64).next_power_of_two();
        let mut h = SpatialHash { cell, mask: buckets as u64 - 1, starts: vec![0; buckets + 1], ids: Vec::new(), big: Vec::new() };
        let mut entries: Vec<(u32, u32)> = Vec::with_capacity(items.len() * 2);
        for (i, b) in items.iter().enumerate() {
            let Some((lo, hi, cut)) = h.cells(b) else { continue };
            if cut {
                h.big.push(i as u32);
                continue;
            }
            for x in lo.0..=hi.0 {
                for y in lo.1..=hi.1 {
                    for z in lo.2..=hi.2 {
                        entries.push(((key(x, y, z) & h.mask) as u32, i as u32));
                    }
                }
            }
        }
        for (b, _) in &entries {
            h.starts[*b as usize + 1] += 1;
        }
        for b in 0..buckets {
            h.starts[b + 1] += h.starts[b];
        }
        let mut fill = h.starts.clone();
        h.ids = vec![0; entries.len()];
        for (b, i) in entries {
            let at = &mut fill[b as usize];
            h.ids[*at as usize] = i;
            *at += 1;
        }
        h
    }

    /// Sizes, for finding slow queries.
    pub fn describe(&self) -> String {
        let longest = self.starts.windows(2).map(|w| w[1] - w[0]).max().unwrap_or(0);
        format!("{} entries in {} buckets, longest {}, {} big", self.ids.len(), self.starts.len() - 1, longest, self.big.len())
    }

    /// Every item whose cells meet the bounds and whose own bounds meet them, each once, in the table's order: the
    /// same whatever thread asks. `seen` is the caller's scratch, one entry per item, kept between calls.
    pub fn query_marked(&self, b: &Aabb, items: &[Aabb], seen: &mut Seen, out: &mut Vec<u32>) {
        out.clear();
        seen.next(items.len());
        let mut take = |i: u32, out: &mut Vec<u32>| {
            if seen.mark(i) && items[i as usize].overlaps(b) {
                out.push(i);
            }
        };
        for &i in &self.big {
            take(i, out);
        }
        if self.ids.is_empty() {
            return;
        }
        let Some((lo, hi, cut)) = self.cells(b) else { return };
        if cut {
            for &i in &self.ids {
                take(i, out);
            }
            return;
        }
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    let k = (key(x, y, z) & self.mask) as usize;
                    for &i in &self.ids[self.starts[k] as usize..self.starts[k + 1] as usize] {
                        take(i, out);
                    }
                }
            }
        }
    }

    /// Every item whose cells meet the bounds, each once, ascending.
    pub fn query(&self, b: &Aabb, out: &mut Vec<u32>) {
        out.clear();
        out.extend_from_slice(&self.big);
        if self.ids.is_empty() {
            return;
        }
        let Some((lo, hi, cut)) = self.cells(b) else { return };
        if cut {
            // Wider than the table's cells reach: everything.
            let mut all: Vec<u32> = self.ids.clone();
            all.extend_from_slice(&self.big);
            all.sort_unstable();
            all.dedup();
            *out = all;
            return;
        }
        for x in lo.0..=hi.0 {
            for y in lo.1..=hi.1 {
                for z in lo.2..=hi.2 {
                    let k = (key(x, y, z) & self.mask) as usize;
                    out.extend_from_slice(&self.ids[self.starts[k] as usize..self.starts[k + 1] as usize]);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
    }
}

/// Marks for one caller's queries: which items it has already given.
#[derive(Default)]
pub struct Seen {
    stamp: Vec<u32>,
    now: u32,
}

impl Seen {
    fn next(&mut self, items: usize) {
        if self.stamp.len() < items {
            self.stamp.resize(items, 0);
        }
        self.now = self.now.wrapping_add(1);
        if self.now == 0 {
            self.stamp.iter_mut().for_each(|s| *s = 0);
            self.now = 1;
        }
    }
    #[inline]
    fn mark(&mut self, i: u32) -> bool {
        let s = &mut self.stamp[i as usize];
        if *s == self.now {
            false
        } else {
            *s = self.now;
            true
        }
    }
}
