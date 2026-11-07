//! Helpers the modeling ops share.

use std::collections::HashMap;

use glam::DVec2;

use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;
use crate::topology::{edges_of_faces, faces_of_points, points_of_faces};

/// Offsets and thicknesses clamped where points would cross stop this share short of the crossing.
pub const CLAMP_MARGIN: f64 = 0.02;
/// Points a clamped offset brings closer than this share of the offset are welded.
pub const WELD_SHARE: f64 = 1e-7;

/// add_face that also copies attributes: face from src_face, corner i from src_corners[i], and the edge from point i
/// to i+1 from src_edges[i] when that edge is new. Any source may be INVALID or empty.
pub fn add_face_from(mesh: &mut Mesh, points: &[Index], src_face: Index, src_corners: &[Index], src_edges: &[Index]) -> Index {
    let edge_cap = mesh.edge_capacity();
    let f = mesh.add_face_from(points, src_face, src_corners);
    if f == INVALID || src_edges.is_empty() {
        return f;
    }
    let mut c = mesh.face_corner(f);
    for i in 0..points.len() {
        let e = mesh.corner_edge(c);
        if e >= edge_cap && i < src_edges.len() && src_edges[i] != INVALID {
            mesh.edge_attrs_mut().copy(e as usize, src_edges[i] as usize);
        }
        c = mesh.corner_next(c);
    }
    f
}

/// A live face with exactly these points in any rotation and direction, or INVALID.
pub fn find_face_with_points(mesh: &Mesh, points: &[Index]) -> Index {
    if points.len() < 3 {
        return INVALID;
    }
    let e = mesh.find_edge(points[0], points[1]);
    if e == INVALID {
        return INVALID;
    }
    let mut found = INVALID;
    mesh.for_each_edge_corner(e, |c| {
        let f = mesh.corner_face(c);
        if found != INVALID || mesh.face_size(f) as usize != points.len() {
            return;
        }
        if points.iter().all(|&p| mesh.find_corner(f, p) != INVALID) {
            found = f;
        }
    });
    found
}

/// Removes faces, then edges and points they leave loose.
pub fn remove_faces_clean(mesh: &mut Mesh, faces: &[Index]) {
    let edges = edges_of_faces(mesh, faces);
    let points = points_of_faces(mesh, faces);
    for &f in faces {
        mesh.remove_face(f);
    }
    mesh.remove_loose(&edges, &points);
}

/// Corners around p in order, each next one across the previous one's outgoing edge. Open fans start at the boundary.
/// False when p is non-manifold (several fans or a non-manifold edge inside).
pub fn point_fan(mesh: &Mesh, p: Index, corners: &mut Vec<Index>, closed: &mut bool) -> bool {
    corners.clear();
    *closed = false;
    let mut all = Vec::new();
    mesh.for_each_point_corner(p, |c| all.push(c));
    if all.is_empty() {
        return false;
    }
    let start = all
        .iter()
        .copied()
        .find(|&c| !mesh.is_manifold_edge(mesh.corner_edge(mesh.corner_prev(c))))
        .unwrap_or(all[0]);
    let mut cur = start;
    loop {
        corners.push(cur);
        let e = mesh.corner_edge(cur);
        if !mesh.is_manifold_edge(e) {
            break;
        }
        let r = mesh.corner_radial(cur);
        if mesh.corner_point(r) == p {
            return false;
        }
        let next = mesh.corner_next(r);
        if next == start {
            *closed = true;
            break;
        }
        if corners.len() > all.len() {
            return false;
        }
        cur = next;
    }
    corners.len() == all.len()
}

/// Corner of the other face on c's edge at the same point as c, for a manifold edge; INVALID otherwise.
pub fn across_at_point(mesh: &Mesh, c: Index, p: Index) -> Index {
    let e = mesh.corner_edge(c);
    if !mesh.is_manifold_edge(e) {
        return INVALID;
    }
    let r = mesh.corner_radial(c);
    if mesh.corner_point(r) == p {
        r
    } else {
        mesh.corner_next(r)
    }
}

/// Mix of corner attributes into dst from corners with weights.
pub fn mix_corners(mesh: &mut Mesh, dst: Index, srcs: &[Index], weights: &[f32]) {
    mesh.corner_attrs_mut().mix(dst as usize, srcs, weights);
}

/// Snapshot of a face before it is removed: points, corners and edges in order.
#[derive(Clone, Debug, Default)]
pub struct FaceCopy {
    pub face: Index,
    pub points: Vec<Index>,
    pub corners: Vec<Index>,
    pub edges: Vec<Index>,
}

pub fn copy_face(mesh: &Mesh, f: Index) -> FaceCopy {
    let mut fc = FaceCopy { face: f, ..Default::default() };
    mesh.for_each_face_corner(f, |c| {
        fc.points.push(mesh.corner_point(c));
        fc.corners.push(c);
        fc.edges.push(mesh.corner_edge(c));
    });
    fc
}

/// weld_points, also giving each face it rebuilt its new id, or INVALID when the face collapsed or doubled another.
pub fn weld_points_rebuilt(mesh: &mut Mesh, target: &[Index], mut rebuilt: Option<&mut HashMap<Index, Index>>) {
    let mapped = |p: Index| {
        if (p as usize) < target.len() && target[p as usize] != INVALID {
            target[p as usize]
        } else {
            p
        }
    };
    let moving: Vec<Index> = (0..target.len() as Index).filter(|&p| mesh.point_valid(p) && mapped(p) != p).collect();
    if moving.is_empty() {
        return;
    }
    let faces = faces_of_points(mesh, &moving);
    let near_edges = edges_of_faces(mesh, &faces);
    let copies: Vec<FaceCopy> = faces.iter().map(|&f| copy_face(mesh, f)).collect();
    for &f in &faces {
        mesh.remove_face(f);
    }
    for &p in &moving {
        mesh.remove_point(p);
    }

    let mut pts = Vec::new();
    let mut corners = Vec::new();
    let mut src_edges = Vec::new();
    for fc in &copies {
        pts.clear();
        corners.clear();
        src_edges.clear();
        // Collapse runs of corners that land on the same point; the run's last edge leads on.
        for i in 0..fc.points.len() {
            let p = mapped(fc.points[i]);
            if pts.last() == Some(&p) {
                *src_edges.last_mut().unwrap() = fc.edges[i];
                continue;
            }
            pts.push(p);
            corners.push(fc.corners[i]);
            src_edges.push(fc.edges[i]);
        }
        while pts.len() > 1 && pts.last() == pts.first() {
            pts.pop();
            corners.pop();
            src_edges.pop();
        }
        let nf = if pts.len() < 3 || find_face_with_points(mesh, &pts) != INVALID {
            INVALID
        } else {
            add_face_from(mesh, &pts, fc.face, &corners, &src_edges)
        };
        if let Some(r) = rebuilt.as_deref_mut() {
            r.insert(fc.face, nf);
        }
    }
    let survivors: Vec<Index> = moving.iter().map(|&p| mapped(p)).collect();
    mesh.remove_loose(&near_edges, &survivors);
}

/// No two edges of the closed polygon cross or touch, apart from neighbours sharing their corner.
pub fn polygon_is_simple(p: &[DVec2]) -> bool {
    let n = p.len();
    let cross = |a: DVec2, b: DVec2| a.x * b.y - a.y * b.x;
    for i in 0..n {
        let a = p[i];
        let b = p[(i + 1) % n];
        if a.distance(b) <= 0.0 {
            return false;
        }
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let c = p[j];
            let d = p[(j + 1) % n];
            let d1 = cross(b - a, c - a);
            let d2 = cross(b - a, d - a);
            let d3 = cross(d - c, a - c);
            let d4 = cross(d - c, b - c);
            if d1 == 0.0 && d2 == 0.0 {
                // Collinear: only overlap counts.
                let dir = b - a;
                let t0 = (c - a).dot(dir);
                let t1 = (d - a).dot(dir);
                let len = dir.dot(dir);
                if t0.max(t1) > 0.0 && t0.min(t1) < len {
                    return false;
                }
                continue;
            }
            if ((d1 > 0.0) != (d2 > 0.0) || d1 == 0.0 || d2 == 0.0) && ((d3 > 0.0) != (d4 > 0.0) || d3 == 0.0 || d4 == 0.0) {
                return false;
            }
        }
    }
    true
}

/// q lies inside the closed polygon, by the even-odd rule.
pub fn inside_polygon(poly: &[DVec2], q: DVec2) -> bool {
    let mut inside = false;
    let n = poly.len();
    let mut j = n.wrapping_sub(1);
    for i in 0..n {
        let a = poly[i];
        let b = poly[j];
        if (a.y > q.y) != (b.y > q.y) && q.x < (b.x - a.x) * (q.y - a.y) / (b.y - a.y) + a.x {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Disjoint sets whose root is always the lowest id of the set.
pub struct UnionFind {
    pub parent: Vec<Index>,
}

impl UnionFind {
    pub fn new(n: usize) -> Self {
        Self { parent: (0..n as Index).collect() }
    }
    pub fn find(&mut self, mut i: Index) -> Index {
        while self.parent[i as usize] != i {
            let gp = self.parent[self.parent[i as usize] as usize];
            self.parent[i as usize] = gp;
            i = gp;
        }
        i
    }
    pub fn unite(&mut self, a: Index, b: Index) {
        let a = self.find(a);
        let b = self.find(b);
        if a != b {
            self.parent[a.max(b) as usize] = a.min(b);
        }
    }
}
