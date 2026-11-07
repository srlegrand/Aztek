//! Loops, rings, boundaries, components and element conversions.

use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;

/// Sorted, unique copy.
pub fn unique_sorted(ids: &[Index]) -> Vec<Index> {
    let mut out = ids.to_vec();
    out.sort_unstable();
    out.dedup();
    out
}

/// Byte mask of size n with the given ids set.
pub fn make_mask(n: usize, ids: &[Index]) -> Vec<u8> {
    let mut mask = vec![0u8; n];
    for &i in ids {
        if (i as usize) < n {
            mask[i as usize] = 1;
        }
    }
    mask
}

/// The edge at p other than e on face f, INVALID if f does not hold e at p.
fn other_face_edge_at(mesh: &Mesh, f: Index, e: Index, p: Index) -> Index {
    let mut found = INVALID;
    mesh.for_each_face_corner(f, |c| {
        let fe = mesh.corner_edge(c);
        if fe != e && (mesh.edge_point(fe, 0) == p || mesh.edge_point(fe, 1) == p) {
            found = fe;
        }
    });
    found
}

/// At a valence-3 point, the loop along an n-gon's border (a cap's rim): e and the next edge share the n-gon and every
/// other face at p is a quad.
fn ngon_border_next(mesh: &Mesh, e: Index, p: Index) -> Index {
    if !mesh.is_manifold_edge(e) {
        return INVALID;
    }
    let c = mesh.edge_corner(e);
    let mut fa = mesh.corner_face(c);
    let mut fb = mesh.opposite_face(e, fa);
    if mesh.face_size(fa) == 4 {
        std::mem::swap(&mut fa, &mut fb);
    }
    if mesh.face_size(fa) < 5 || mesh.face_size(fb) != 4 {
        return INVALID;
    }
    let next = other_face_edge_at(mesh, fa, e, p);
    if next == INVALID || !mesh.is_manifold_edge(next) {
        return INVALID;
    }
    let beyond = mesh.opposite_face(next, fa);
    if beyond != INVALID && mesh.face_size(beyond) == 4 {
        next
    } else {
        INVALID
    }
}

/// Next loop edge after e at point p, INVALID where the loop stops.
pub fn edge_loop_next(mesh: &Mesh, e: Index, p: Index) -> Index {
    if mesh.is_boundary_edge(e) {
        // Follow the border through every point where it does not branch.
        let (mut next, mut count) = (INVALID, 0);
        mesh.for_each_point_edge(p, |x| {
            if x != e && mesh.is_boundary_edge(x) {
                next = x;
                count += 1;
            }
        });
        return if count == 1 { next } else { INVALID };
    }
    if mesh.is_boundary_point(p) {
        return INVALID;
    }
    match mesh.point_valence(p) {
        3 => ngon_border_next(mesh, e, p),
        4 => {
            // The edge sharing no face with e.
            let mut touching = Vec::new();
            mesh.for_each_edge_corner(e, |c| {
                touching.push(other_face_edge_at(mesh, mesh.corner_face(c), e, p))
            });
            let mut found = INVALID;
            mesh.for_each_point_edge(p, |x| {
                if x != e && !touching.contains(&x) {
                    found = x;
                }
            });
            found
        }
        _ => INVALID,
    }
}

/// Reference-style edge loop through e: crosses valence-4 points to the edge sharing no face with the current one, follows
/// borders wherever they do not branch, and runs along an n-gon's rim through valence-3 points between quads. Stops
/// at other poles. Ordered; closed loops start at e.
pub fn edge_loop(mesh: &Mesh, e: Index) -> Vec<Index> {
    if !mesh.edge_valid(e) {
        return Vec::new();
    }
    let mut seen = vec![0u8; mesh.edge_capacity() as usize];
    seen[e as usize] = 1;
    let mut forward = vec![e];
    // Walk from point 1 forward; if that does not close, walk from point 0 backward.
    let (mut cur, mut p) = (e, mesh.edge_point(e, 1));
    loop {
        let next = edge_loop_next(mesh, cur, p);
        if next == INVALID {
            break;
        }
        if next == e {
            return forward;
        }
        if seen[next as usize] != 0 {
            break;
        }
        seen[next as usize] = 1;
        forward.push(next);
        p = mesh.edge_other_point(next, p);
        cur = next;
    }
    let mut backward = Vec::new();
    let (mut cur, mut p) = (e, mesh.edge_point(e, 0));
    loop {
        let next = edge_loop_next(mesh, cur, p);
        if next == INVALID || seen[next as usize] != 0 {
            break;
        }
        seen[next as usize] = 1;
        backward.push(next);
        p = mesh.edge_other_point(next, p);
        cur = next;
    }
    backward.reverse();
    backward.extend(forward);
    backward
}

/// Edge opposite e in quad f, INVALID if f is not a quad containing e.
pub fn quad_opposite_edge(mesh: &Mesh, e: Index, f: Index) -> Index {
    if !mesh.face_valid(f) || mesh.face_size(f) != 4 {
        return INVALID;
    }
    let mut found = INVALID;
    mesh.for_each_face_corner(f, |c| {
        if mesh.corner_edge(c) == e {
            found = mesh.corner_edge(mesh.corner_next(mesh.corner_next(c)));
        }
    });
    found
}

/// Edges opposite e across quads, in order, both directions.
pub fn edge_ring(mesh: &Mesh, e: Index) -> Vec<Index> {
    if !mesh.edge_valid(e) || mesh.edge_corner(e) == INVALID {
        return vec![e];
    }
    let mut seen = vec![0u8; mesh.edge_capacity() as usize];
    seen[e as usize] = 1;
    let mut walk = |start_face: Index, out: &mut Vec<Index>| -> bool {
        let (mut cur, mut f) = (e, start_face);
        while f != INVALID {
            let opp = quad_opposite_edge(mesh, cur, f);
            if opp == INVALID {
                return false;
            }
            if opp == e {
                return true;
            }
            if seen[opp as usize] != 0 {
                return false;
            }
            seen[opp as usize] = 1;
            out.push(opp);
            f = mesh.opposite_face(opp, f);
            cur = opp;
        }
        false
    };
    let f0 = mesh.corner_face(mesh.edge_corner(e));
    let mut forward = vec![e];
    if walk(f0, &mut forward) {
        return forward;
    }
    let mut backward = Vec::new();
    let f1 = mesh.opposite_face(e, f0);
    if f1 != INVALID {
        walk(f1, &mut backward);
    }
    backward.reverse();
    backward.extend(forward);
    backward
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EdgePath {
    /// Ordered; for a closed path the first point is not repeated.
    pub points: Vec<Index>,
    /// edges[i] joins points[i] and points[i+1].
    pub edges: Vec<Index>,
    pub closed: bool,
}

/// Boundary edges (one face) chained into ordered loops, following face winding.
pub fn boundary_loops(mesh: &Mesh) -> Vec<EdgePath> {
    let mut out = Vec::new();
    let mut seen = vec![0u8; mesh.edge_capacity() as usize];
    for e in 0..mesh.edge_capacity() {
        if !mesh.edge_valid(e) || seen[e as usize] != 0 || !mesh.is_boundary_edge(e) {
            continue;
        }
        let mut path = EdgePath::default();
        // Walk against the face winding so the loop runs the way a filling face would.
        let mut cur = e;
        let mut p = mesh.corner_next_point(mesh.edge_corner(e));
        loop {
            seen[cur as usize] = 1;
            path.points.push(p);
            path.edges.push(cur);
            let q = mesh.edge_other_point(cur, p);
            let mut next = INVALID;
            mesh.for_each_point_edge(q, |x| {
                // The face on x must arrive at q, so the fill direction carries on away from it.
                if x != cur
                    && seen[x as usize] == 0
                    && mesh.is_boundary_edge(x)
                    && next == INVALID
                    && mesh.corner_next_point(mesh.edge_corner(x)) == q
                {
                    next = x;
                }
            });
            if next == INVALID {
                path.closed = q == path.points[0];
                if !path.closed {
                    path.points.push(q);
                }
                break;
            }
            p = q;
            cur = next;
        }
        out.push(path);
    }
    out
}

/// Chains a set of edges into ordered paths. Branching points end paths.
pub fn chain_edges(mesh: &Mesh, edges: &[Index]) -> Vec<EdgePath> {
    let inside = make_mask(mesh.edge_capacity() as usize, edges);
    let mut used = vec![0u8; mesh.edge_capacity() as usize];
    let degree = |p: Index| {
        let mut d = 0;
        mesh.for_each_point_edge(p, |x| d += inside[x as usize] as i32);
        d
    };
    let mut out = Vec::new();
    let mut trace = |e: Index, start: Index, used: &mut Vec<u8>| {
        let mut path = EdgePath::default();
        let (mut p, mut cur) = (start, e);
        while cur != INVALID {
            used[cur as usize] = 1;
            path.points.push(p);
            path.edges.push(cur);
            p = mesh.edge_other_point(cur, p);
            if p == path.points[0] {
                path.closed = true;
                break;
            }
            let from = cur;
            cur = INVALID;
            if degree(p) == 2 {
                mesh.for_each_point_edge(p, |x| {
                    if x != from && inside[x as usize] != 0 && used[x as usize] == 0 {
                        cur = x;
                    }
                });
            }
        }
        if !path.closed {
            path.points.push(p);
        }
        out.push(path);
    };
    // Open chains first, starting from ends, then what remains is closed.
    for &e in edges {
        if !mesh.edge_valid(e) || used[e as usize] != 0 {
            continue;
        }
        for s in 0..2 {
            if used[e as usize] == 0 && degree(mesh.edge_point(e, s)) != 2 {
                trace(e, mesh.edge_point(e, s), &mut used);
            }
        }
    }
    for &e in edges {
        if mesh.edge_valid(e) && used[e as usize] == 0 {
            trace(e, mesh.edge_point(e, 0), &mut used);
        }
    }
    out
}

/// Component id per face (INVALID for deleted faces), connected through shared edges not set in `block_edge`.
/// Returns the count.
pub fn face_components(mesh: &Mesh, component: &mut Vec<Index>, block_edge: &[u8]) -> Index {
    *component = vec![INVALID; mesh.face_capacity() as usize];
    let mut count = 0;
    let mut stack = Vec::new();
    for f in 0..mesh.face_capacity() {
        if !mesh.face_valid(f) || component[f as usize] != INVALID {
            continue;
        }
        component[f as usize] = count;
        stack.push(f);
        while let Some(g) = stack.pop() {
            mesh.for_each_face_corner(g, |c| {
                let e = mesh.corner_edge(c);
                if !block_edge.is_empty() && block_edge[e as usize] != 0 {
                    return;
                }
                mesh.for_each_edge_corner(e, |r| {
                    let h = mesh.corner_face(r);
                    if component[h as usize] == INVALID {
                        component[h as usize] = count;
                        stack.push(h);
                    }
                });
            });
        }
        count += 1;
    }
    count
}

/// Edges with exactly one side in the face set (or on the mesh boundary).
pub fn region_boundary_edges(mesh: &Mesh, faces: &[Index]) -> Vec<Index> {
    let inside = make_mask(mesh.face_capacity() as usize, faces);
    edges_of_faces(mesh, faces)
        .into_iter()
        .filter(|&e| {
            let (mut i, mut o) = (0, 0);
            mesh.for_each_edge_corner(e, |c| {
                if inside[mesh.corner_face(c) as usize] != 0 {
                    i += 1
                } else {
                    o += 1
                }
            });
            i >= 1 && (o >= 1 || i == 1)
        })
        .collect()
}

pub fn faces_of_points(mesh: &Mesh, points: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &p in points {
        if mesh.point_valid(p) {
            mesh.for_each_point_corner(p, |c| out.push(mesh.corner_face(c)));
        }
    }
    unique_sorted(&out)
}

pub fn faces_of_edges(mesh: &Mesh, edges: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &e in edges {
        if mesh.edge_valid(e) {
            mesh.for_each_edge_corner(e, |c| out.push(mesh.corner_face(c)));
        }
    }
    unique_sorted(&out)
}

pub fn points_of_faces(mesh: &Mesh, faces: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &f in faces {
        if mesh.face_valid(f) {
            mesh.for_each_face_corner(f, |c| out.push(mesh.corner_point(c)));
        }
    }
    unique_sorted(&out)
}

pub fn edges_of_faces(mesh: &Mesh, faces: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &f in faces {
        if mesh.face_valid(f) {
            mesh.for_each_face_corner(f, |c| out.push(mesh.corner_edge(c)));
        }
    }
    unique_sorted(&out)
}

pub fn points_of_edges(mesh: &Mesh, edges: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &e in edges {
        if mesh.edge_valid(e) {
            out.push(mesh.edge_point(e, 0));
            out.push(mesh.edge_point(e, 1));
        }
    }
    unique_sorted(&out)
}
