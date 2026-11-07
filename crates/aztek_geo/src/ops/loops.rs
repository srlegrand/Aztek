//! Edge loops across quad rings, ring splitting and region ring picking.

use super::knife::{knife_apply, knife_loop_plan, KnifePlan};
use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;
use crate::symmetry::SymmetryMap;
use crate::topology::{edge_ring, make_mask};

#[derive(Clone, Copy, Debug)]
pub struct EdgeLoopOptions {
    /// Where along each ring edge the loop goes, 0 to 1 from the start edge's first point; with several, their centre.
    pub fraction: f64,
    pub count: i32,
    /// With several loops, the gap between neighbours relative to even spacing: 1 spaces them as count+1 equal parts
    /// of the edge, smaller packs them round fraction. The group slides to stay on the edge and shrinks when it cannot.
    pub spacing: f64,
}

impl Default for EdgeLoopOptions {
    fn default() -> Self {
        Self { fraction: 0.5, count: 1, spacing: 1.0 }
    }
}

#[derive(Clone, Copy)]
struct RingEdge {
    edge: Index,
    /// The endpoint on the loop's "first" side.
    side_a: Index,
}

/// Ring through e with consistent sides, plus the quads between consecutive edges.
fn walk_ring(mesh: &Mesh, e: Index, ring: &mut Vec<RingEdge>, quads: &mut Vec<Index>, closed: &mut bool) -> bool {
    ring.clear();
    quads.clear();
    *closed = false;
    if !mesh.edge_valid(e) || mesh.edge_corner(e) == INVALID {
        return false;
    }
    let mut seen = vec![false; mesh.edge_capacity() as usize];

    // Steps across quad f from (edge, side) to the opposite edge and its matching side.
    let step = |edge: Index, side: Index, f: Index| -> Option<RingEdge> {
        if mesh.face_size(f) != 4 {
            return None;
        }
        let mut c = INVALID;
        mesh.for_each_face_corner(f, |x| {
            if mesh.corner_edge(x) == edge {
                c = x;
            }
        });
        if c == INVALID {
            return None;
        }
        let c2 = mesh.corner_next(mesh.corner_next(c));
        // In x->y->z->w, x pairs with w and y with z.
        let side_a = if mesh.corner_point(c) == side { mesh.corner_next_point(c2) } else { mesh.corner_point(c2) };
        Some(RingEdge { edge: mesh.corner_edge(c2), side_a })
    };

    let start = mesh.edge_point(e, 0);
    let mut fwd = vec![RingEdge { edge: e, side_a: start }];
    let mut fwd_quads = Vec::new();
    seen[e as usize] = true;
    let mut f = mesh.corner_face(mesh.edge_corner(e));
    let mut cur = RingEdge { edge: e, side_a: start };
    while f != INVALID {
        let Some(next) = step(cur.edge, cur.side_a, f) else { break };
        fwd_quads.push(f);
        if next.edge == e {
            *closed = true;
            break;
        }
        if seen[next.edge as usize] {
            fwd_quads.pop();
            break;
        }
        seen[next.edge as usize] = true;
        fwd.push(next);
        f = mesh.opposite_face(next.edge, f);
        cur = next;
    }
    if *closed {
        *ring = fwd;
        *quads = fwd_quads;
        return true;
    }

    let mut back = Vec::new();
    let mut back_quads = Vec::new();
    f = mesh.opposite_face(e, mesh.corner_face(mesh.edge_corner(e)));
    cur = RingEdge { edge: e, side_a: start };
    while f != INVALID {
        let Some(next) = step(cur.edge, cur.side_a, f) else { break };
        if seen[next.edge as usize] {
            break;
        }
        seen[next.edge as usize] = true;
        back_quads.push(f);
        back.push(next);
        f = mesh.opposite_face(next.edge, f);
        cur = next;
    }
    back.reverse();
    back_quads.reverse();
    *ring = back;
    ring.extend(fwd);
    *quads = back_quads;
    quads.extend(fwd_quads);
    true
}

/// Splits e at the fractions (ascending, measured from side_a); returns the new points from side_a on.
fn split_at(mesh: &mut Mesh, e: Index, side_a: Index, fracs: &[f64]) -> Vec<Index> {
    let from_a = mesh.edge_point(e, 0) == side_a;
    let mut f = fracs.to_vec();
    if !from_a {
        for x in f.iter_mut() {
            *x = 1.0 - *x;
        }
        f.reverse();
    }
    let mut out = Vec::new();
    let mut cur = e;
    let mut prev = 0.0;
    for x in f {
        out.push(mesh.split_edge(cur, (x - prev) / (1.0 - prev)));
        cur = mesh.last_split_edge();
        prev = x;
    }
    if !from_a {
        out.reverse();
    }
    out
}

fn shared_face(mesh: &Mesh, a: Index, b: Index) -> Index {
    let mut found = INVALID;
    mesh.for_each_point_corner(a, |c| {
        let f = mesh.corner_face(c);
        if found == INVALID && mesh.find_corner(f, b) != INVALID {
            found = f;
        }
    });
    found
}

/// The ascending fractions along each ring edge the loops go at.
pub fn edge_loop_fractions(opts: &EdgeLoopOptions) -> Vec<f64> {
    const LO: f64 = 1e-4;
    const HI: f64 = 1.0 - 1e-4;
    let count = opts.count.max(1);
    if count == 1 {
        return vec![opts.fraction.clamp(LO, HI)];
    }
    // Neighbours stay at least this far apart, so no loop lands on another.
    const MIN_GAP: f64 = 1e-3;
    let gap = MIN_GAP.max(opts.spacing.max(0.0) / (count + 1) as f64);
    let span = (gap * (count - 1) as f64).min(HI - LO);
    let first = (opts.fraction - span / 2.0).clamp(LO, HI - span);
    (0..count).map(|k| first + span * k as f64 / (count - 1) as f64).collect()
}

/// Inserts loops across the edge ring through ring_edge. Returns the new loop edges.
pub fn insert_edge_loop(mesh: &mut Mesh, ring_edge: Index, opts: &EdgeLoopOptions) -> Vec<Index> {
    let mut ring = Vec::new();
    let mut quads = Vec::new();
    let mut closed = false;
    if !walk_ring(mesh, ring_edge, &mut ring, &mut quads, &mut closed) || quads.is_empty() {
        return Vec::new();
    }
    let fracs = edge_loop_fractions(opts);
    let cuts: Vec<Vec<Index>> = ring.iter().map(|r| split_at(mesh, r.edge, r.side_a, &fracs)).collect();
    let mut out = Vec::new();
    let n = ring.len();
    for i in 0..quads.len() {
        let a = &cuts[i];
        let b = &cuts[(i + 1) % n];
        for k in 0..a.len() {
            let f = shared_face(mesh, a[k], b[k]);
            if f == INVALID {
                continue;
            }
            let (fa, fb) = (mesh.find_corner(f, a[k]), mesh.find_corner(f, b[k]));
            if mesh.split_face(f, fa, fb) != INVALID {
                out.push(mesh.last_split_edge());
            }
        }
    }
    out
}

/// Subdivides one way: count evenly spaced loops across each distinct ring through the edges (a ring given twice, or
/// through its mirror image under symmetry, is cut once). Returns the new loop edges.
pub fn split_rings(mesh: &mut Mesh, ring_edges: &[Index], count: i32, symmetry: Option<&SymmetryMap>) -> Vec<Index> {
    let sym = symmetry.filter(|s| !s.is_empty());
    let mut covered = vec![false; mesh.edge_capacity() as usize];
    let cover = |covered: &mut Vec<bool>, e: Index| {
        for x in edge_ring(mesh, e) {
            covered[x as usize] = true;
        }
        covered[e as usize] = true;
    };
    let mut reps = Vec::new();
    for &e in ring_edges {
        if !mesh.edge_valid(e) || covered[e as usize] {
            continue;
        }
        reps.push(e);
        cover(&mut covered, e);
        if let Some(s) = sym {
            for k in 1..s.order() {
                let me = s.image_edge(mesh, k, e);
                if me != INVALID && !covered[me as usize] {
                    cover(&mut covered, me);
                }
            }
        }
    }

    // Splitting keeps every existing edge id, so the later rings' edges stay valid while earlier ones are cut.
    let opts = EdgeLoopOptions { fraction: 0.5, count: count.max(1), ..Default::default() };
    let mut out = Vec::new();
    for e in reps {
        if !mesh.edge_valid(e) {
            continue;
        }
        match sym {
            None => out.extend(insert_edge_loop(mesh, e, &opts)),
            Some(s) => {
                let mut plan = KnifePlan::default();
                for t in edge_loop_fractions(&opts) {
                    plan.segments.extend(knife_loop_plan(mesh, e, t, Some(s)).segments);
                }
                out.extend(knife_apply(mesh, &plan));
            }
        }
    }
    out.retain(|&e| mesh.edge_valid(e));
    out
}

/// One edge per quad of the faces, choosing its ring direction: across picks the ring that runs the region's short
/// way (cutting it adds lines across a strip), along the long way. Ties go by the quad's own edge lengths.
pub fn region_ring_edges(mesh: &Mesh, faces: &[Index], across: bool) -> Vec<Index> {
    let inside = make_mask(mesh.face_capacity() as usize, faces);
    let opposite = |f: Index, e: Index| {
        let mut out = INVALID;
        mesh.for_each_face_corner(f, |c| {
            if mesh.corner_edge(c) == e {
                out = mesh.corner_edge(mesh.corner_next(mesh.corner_next(c)));
            }
        });
        out
    };
    // Edges of e's ring inside the region; a closed ring counts each of its edges twice, alike for all of them.
    let in_region = |e: Index| {
        let mut n = 1usize;
        mesh.for_each_edge_corner(e, |c| {
            let mut edge = e;
            let mut f = mesh.corner_face(c);
            let mut guard = 0;
            while guard < mesh.edge_capacity() && f != INVALID && inside[f as usize] != 0 && mesh.face_size(f) == 4 {
                edge = opposite(f, edge);
                if edge == INVALID || edge == e {
                    break;
                }
                n += 1;
                f = mesh.opposite_face(edge, f);
                guard += 1;
            }
        });
        n
    };

    let mut out = Vec::new();
    for &f in faces {
        if !mesh.face_valid(f) || mesh.face_size(f) != 4 {
            continue;
        }
        let c = mesh.face_corner(f);
        let e0 = mesh.corner_edge(c);
        let e1 = mesh.corner_edge(mesh.corner_next(c));
        let n0 = in_region(e0);
        let n1 = in_region(e1);
        let mut pick0 = if across { n0 < n1 } else { n0 > n1 };
        if n0 == n1 {
            // A strip's short way crosses its long edges.
            let l0 = mesh.edge_length(e0);
            let l1 = mesh.edge_length(e1);
            pick0 = if across { l0 >= l1 } else { l0 < l1 };
        }
        out.push(if pick0 { e0 } else { e1 });
    }
    out
}
