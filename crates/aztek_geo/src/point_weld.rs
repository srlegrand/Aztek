//! Numbering identical keys, and welding identical positions into shared points.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use glam::{DVec3, Vec3};
use rayon::prelude::*;

use crate::attribute::Index;

const BUCKET_BITS: u32 = 12;
const BUCKETS: usize = 1 << BUCKET_BITS;

/// Numbers keys that are equal, in parallel: `id_of[i]` is key i's number, counting distinct keys in order of first
/// appearance, and `first[n]` is where number n first appears.
pub fn number_distinct<K: Hash + Eq + Sync>(keys: &[K]) -> (Vec<Index>, Vec<Index>) {
    let n = keys.len();
    let bucket_of = |k: &K| {
        let mut h = DefaultHasher::new();
        k.hash(&mut h);
        (h.finish() >> (64 - BUCKET_BITS)) as usize
    };
    let bucket: Vec<u32> = keys.par_iter().map(|k| bucket_of(k) as u32).collect();
    // Indices grouped by bucket, each bucket in index order.
    let mut count = vec![0usize; BUCKETS + 1];
    for &b in &bucket {
        count[b as usize + 1] += 1;
    }
    for k in 0..BUCKETS {
        count[k + 1] += count[k];
    }
    let starts = count.clone();
    let mut items = vec![0 as Index; n];
    let mut fill = count;
    for (i, &b) in bucket.iter().enumerate() {
        items[fill[b as usize]] = i as Index;
        fill[b as usize] += 1;
    }
    // In each bucket the first appearance of a key comes first.
    let first_pairs: Vec<Vec<(Index, Index)>> = (0..BUCKETS)
        .into_par_iter()
        .map(|k| {
            let slice = &items[starts[k]..starts[k + 1]];
            let mut seen: HashMap<&K, Index> = HashMap::with_capacity(slice.len());
            slice
                .iter()
                .map(|&i| {
                    let f = *seen.entry(&keys[i as usize]).or_insert(i);
                    (i, f)
                })
                .collect()
        })
        .collect();
    let mut first_of = vec![0 as Index; n];
    for part in first_pairs {
        for (i, f) in part {
            first_of[i as usize] = f;
        }
    }
    let mut number = vec![0 as Index; n];
    let mut first = Vec::new();
    for i in 0..n {
        if first_of[i] == i as Index {
            number[i] = first.len() as Index;
            first.push(i as Index);
        }
    }
    let id_of = (0..n).into_par_iter().map(|i| number[first_of[i] as usize]).collect();
    (id_of, first)
}

/// Points for a list of corner positions, identical ones (bit for bit) sharing one: `point_of[i]` is corner i's
/// point and the returned points hold each position once, in order of first use. With signed_zero off, -0 and +0
/// count as the same.
pub fn weld_identical_f32(pos: &[Vec3], signed_zero: bool) -> (Vec<Index>, Vec<DVec3>) {
    let clean = |p: Vec3| if signed_zero { p } else { p + Vec3::ZERO };
    let keys: Vec<[u32; 3]> = pos.par_iter().map(|&p| clean(p).to_array().map(f32::to_bits)).collect();
    let (point_of, first) = number_distinct(&keys);
    let points = first.par_iter().map(|&k| clean(pos[k as usize]).as_dvec3()).collect();
    (point_of, points)
}

pub fn weld_identical_f64(pos: &[DVec3], signed_zero: bool) -> (Vec<Index>, Vec<DVec3>) {
    let clean = |p: DVec3| if signed_zero { p } else { p + DVec3::ZERO };
    let keys: Vec<[u64; 3]> = pos.par_iter().map(|&p| clean(p).to_array().map(f64::to_bits)).collect();
    let (point_of, first) = number_distinct(&keys);
    let points = first.par_iter().map(|&k| clean(pos[k as usize])).collect();
    (point_of, points)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_in_first_appearance_order() {
        let keys = [5, 3, 5, 7, 3, 9];
        let (id, first) = number_distinct(&keys);
        assert_eq!(id, vec![0, 1, 0, 2, 1, 3]);
        assert_eq!(first, vec![0, 1, 3, 5]);
        let (p, pts) = weld_identical_f32(&[Vec3::new(-0.0, 1.0, 2.0), Vec3::new(0.0, 1.0, 2.0)], false);
        assert_eq!(p, vec![0, 0]);
        assert_eq!(pts.len(), 1);
        let (p, _) = weld_identical_f32(&[Vec3::new(-0.0, 1.0, 2.0), Vec3::new(0.0, 1.0, 2.0)], true);
        assert_eq!(p, vec![0, 1]);
    }
}
