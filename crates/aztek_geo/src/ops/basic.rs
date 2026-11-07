//! Delete, orientation, triangles and quads, merging, dissolve, whole-mesh ops, plane cuts and symmetrize.

use std::collections::HashMap;
use std::f64::consts::FRAC_PI_2;

use glam::{DMat3, DMat4, DVec3, DVec4, Mat3, Vec3};

use super::common::*;
use crate::attribute::{names, Domain, Index, INVALID};
use crate::mesh::Mesh;
use crate::symmetry::{apply_symmetry_snap, axis_plane, remove_symmetry_layers, RadialSymmetry, SymmetryMap, SymmetryPlane};
use crate::topology::*;
use crate::triangulate::{triangulate_face, TriangulateOptions};

// Delete. Leftover loose edges and points are removed too.

pub fn delete_faces(mesh: &mut Mesh, faces: &[Index], symmetry: Option<&SymmetryMap>) {
    let sel = match symmetry {
        Some(s) => s.expand_faces(mesh, faces),
        None => unique_sorted(faces),
    };
    remove_faces_clean(mesh, &sel);
}

/// Removes the edges and their faces.
pub fn delete_edges(mesh: &mut Mesh, edges: &[Index], symmetry: Option<&SymmetryMap>) {
    let sel = match symmetry {
        Some(s) => s.expand_edges(mesh, edges),
        None => unique_sorted(edges),
    };
    let faces = faces_of_edges(mesh, &sel);
    let near_edges = edges_of_faces(mesh, &faces);
    let mut near_points = points_of_faces(mesh, &faces);
    near_points.extend(points_of_edges(mesh, &sel));
    for &f in &faces {
        mesh.remove_face(f);
    }
    for &e in &sel {
        mesh.remove_edge(e);
    }
    mesh.remove_loose(&near_edges, &near_points);
}

/// Removes the points with their edges and faces.
pub fn delete_points(mesh: &mut Mesh, points: &[Index], symmetry: Option<&SymmetryMap>) {
    let sel = match symmetry {
        Some(s) => s.expand_points(points),
        None => unique_sorted(points),
    };
    let faces = faces_of_points(mesh, &sel);
    let near_edges = edges_of_faces(mesh, &faces);
    let near_points = points_of_faces(mesh, &faces);
    for &p in &sel {
        mesh.remove_point(p);
    }
    mesh.remove_loose(&near_edges, &near_points);
}

// Orientation

pub fn flip_normals(mesh: &mut Mesh, faces: &[Index]) {
    for f in unique_sorted(faces) {
        if !mesh.face_valid(f) {
            continue;
        }
        mesh.flip_face(f);
        let corners = mesh.face_corners(f);
        let n = mesh.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL);
        if !n.is_empty() {
            for c in corners {
                let v = &mut n[c as usize];
                *v = [-v[0], -v[1], -v[2]];
            }
        }
    }
}

/// Makes winding consistent within each connected part; closed parts face outward. All faces when empty.
pub fn conform_normals(mesh: &mut Mesh, faces: &[Index]) {
    let fc = mesh.face_capacity() as usize;
    let mut inside = vec![false; fc];
    if faces.is_empty() {
        for f in 0..fc {
            inside[f] = mesh.face_valid(f as Index);
        }
    } else {
        for &f in faces {
            if mesh.face_valid(f) {
                inside[f as usize] = true;
            }
        }
    }

    let mut flip = vec![-1i8; fc];
    let mut comp = Vec::new();
    let mut stack = Vec::new();
    for seed in 0..fc as Index {
        if !inside[seed as usize] || flip[seed as usize] >= 0 {
            continue;
        }
        comp.clear();
        flip[seed as usize] = 0;
        stack.push(seed);
        let mut closed = true;
        while let Some(f) = stack.pop() {
            comp.push(f);
            mesh.for_each_face_corner(f, |c| {
                let e = mesh.corner_edge(c);
                if !mesh.is_manifold_edge(e) {
                    closed = false;
                    return;
                }
                let r = mesh.corner_radial(c);
                let g = mesh.corner_face(r);
                if !inside[g as usize] {
                    closed = false;
                    return;
                }
                if flip[g as usize] >= 0 {
                    return;
                }
                let same_dir = mesh.corner_point(r) == mesh.corner_point(c);
                flip[g as usize] = flip[f as usize] ^ same_dir as i8;
                stack.push(g);
            });
        }

        let invert = if closed {
            let mut vol = 0.0;
            for &f in &comp {
                let c0 = mesh.face_corner(f);
                let a = mesh.position(mesh.corner_point(c0));
                let mut fv = 0.0;
                let mut c = mesh.corner_next(c0);
                while mesh.corner_next(c) != c0 {
                    fv += a.dot(mesh.position(mesh.corner_point(c)).cross(mesh.position(mesh.corner_next_point(c))));
                    c = mesh.corner_next(c);
                }
                vol += if flip[f as usize] != 0 { -fv } else { fv };
            }
            vol < 0.0
        } else {
            let flipped = comp.iter().filter(|&&f| flip[f as usize] != 0).count();
            flipped * 2 > comp.len()
        };
        if invert {
            for &f in &comp {
                flip[f as usize] ^= 1;
            }
        }
    }
    let to_flip: Vec<Index> = (0..fc as Index).filter(|&f| inside[f as usize] && flip[f as usize] == 1).collect();
    flip_normals(mesh, &to_flip);
}

// Triangles and quads

pub fn triangulate_faces(mesh: &mut Mesh, faces: &[Index], opts: &TriangulateOptions) -> Vec<Index> {
    let mut out = Vec::new();
    // Every face is split against the untouched mesh first, so the result matches what triangulate() displays.
    let mut todo = Vec::new();
    let mut tris = Vec::new();
    let mut first = Vec::new();
    for f in unique_sorted(faces) {
        if !mesh.face_valid(f) {
            continue;
        }
        if mesh.face_size(f) == 3 {
            out.push(f);
            continue;
        }
        todo.push(f);
        first.push(tris.len());
        triangulate_face(mesh, f, &mut tris, opts);
    }
    first.push(tris.len());
    for k in 0..todo.len() {
        let corners = &tris[first[k]..first[k + 1]];
        let pts: Vec<Index> = corners.iter().map(|&c| mesh.corner_point(c)).collect();
        mesh.remove_face(todo[k]);
        for t in (0..corners.len()).step_by(3) {
            let nf = mesh.add_face_from(&pts[t..t + 3], todo[k], &corners[t..t + 3]);
            if nf != INVALID {
                out.push(nf);
            }
        }
    }
    out
}

#[derive(Clone, Debug)]
pub struct QuadrangulateOptions {
    /// Largest angle between the two triangle normals.
    pub max_face_angle: f64,
    pub keep_hard_edges: bool,
    pub keep_seams: bool,
    pub keep_materials: bool,
}

impl Default for QuadrangulateOptions {
    fn default() -> Self {
        Self { max_face_angle: 40.0, keep_hard_edges: true, keep_seams: true, keep_materials: true }
    }
}

/// Joins pairs of neighbouring triangles into quads, best shaped first. Returns the resulting faces.
pub fn quadrangulate_faces(mesh: &mut Mesh, faces: &[Index], opts: &QuadrangulateOptions) -> Vec<Index> {
    let fc = mesh.face_capacity() as usize;
    let mut inside = vec![false; fc];
    for &f in faces {
        if mesh.face_valid(f) && mesh.face_size(f) == 3 {
            inside[f as usize] = true;
        }
    }
    let hard = mesh.edge_attrs().get::<bool>(names::HARD);
    let seam = mesh.edge_attrs().get::<bool>(names::SEAM);
    let mat = mesh.face_attrs().get::<i32>(names::MATERIAL);
    let cos_max = opts.max_face_angle.to_radians().cos();

    let mut cands: Vec<(f64, Index)> = Vec::new();
    for e in edges_of_faces(mesh, faces) {
        if !mesh.is_manifold_edge(e) {
            continue;
        }
        let c1 = mesh.edge_corner(e);
        let c2 = mesh.corner_radial(c1);
        let f1 = mesh.corner_face(c1);
        let f2 = mesh.corner_face(c2);
        if !inside[f1 as usize] || !inside[f2 as usize] || mesh.corner_point(c1) == mesh.corner_point(c2) {
            continue;
        }
        if (opts.keep_hard_edges && !hard.is_empty() && hard[e as usize])
            || (opts.keep_seams && !seam.is_empty() && seam[e as usize])
        {
            continue;
        }
        if opts.keep_materials && !mat.is_empty() && mat[f1 as usize] != mat[f2 as usize] {
            continue;
        }
        let n1 = mesh.face_normal(f1);
        let n2 = mesh.face_normal(f2);
        if n1.dot(n2) < cos_max {
            continue;
        }
        // The quad join_faces would make: q, x1, p, x2.
        let quad = [
            mesh.corner_point(mesh.corner_next(c1)),
            mesh.corner_point(mesh.corner_prev(c1)),
            mesh.corner_point(c1),
            mesh.corner_point(mesh.corner_prev(c2)),
        ];
        let n = (n1 + n2).normalize();
        let mut score = 0.0;
        let mut convex = true;
        for i in 0..4 {
            let p = mesh.position(quad[i]);
            let a = mesh.position(quad[(i + 3) % 4]) - p;
            let b = mesh.position(quad[(i + 1) % 4]) - p;
            convex &= b.cross(a).dot(n) > 0.0;
            let ang = a.cross(b).length().atan2(a.dot(b));
            score += (FRAC_PI_2 - ang).abs();
        }
        if convex {
            cands.push((score, e));
        }
    }
    cands.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let mut used = vec![false; fc];
    let mut out = Vec::new();
    for (_, edge) in cands {
        if !mesh.edge_valid(edge) || !mesh.is_manifold_edge(edge) {
            continue;
        }
        let c1 = mesh.edge_corner(edge);
        let f1 = mesh.corner_face(c1);
        let f2 = mesh.corner_face(mesh.corner_radial(c1));
        if used[f1 as usize] || used[f2 as usize] {
            continue;
        }
        let f = mesh.join_faces(edge);
        if f == INVALID {
            continue;
        }
        used[f1 as usize] = true;
        used[f2 as usize] = true;
        out.push(f);
    }
    for &f in faces {
        if inside[f as usize] && !used[f as usize] && mesh.face_valid(f) {
            out.push(f);
        }
    }
    unique_sorted(&out)
}

/// Splits each face into a fan of triangles around a new centre point. Returns the new faces.
pub fn poke_faces(mesh: &mut Mesh, faces: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for f in unique_sorted(faces) {
        if !mesh.face_valid(f) {
            continue;
        }
        let fc = copy_face(mesh, f);
        let center = mesh.face_centroid(f);
        let n = fc.points.len();
        mesh.remove_face(f);
        let cp = mesh.add_point(center);
        let w = vec![1.0f32 / n as f32; n];
        mesh.point_attrs_mut().mix(cp as usize, &fc.points, &w);
        let mut center_corner = INVALID;
        for i in 0..n {
            let pts = [fc.points[i], fc.points[(i + 1) % n], cp];
            let src = [fc.corners[i], fc.corners[(i + 1) % n], center_corner];
            let nf = mesh.add_face_from(&pts, f, &src);
            if nf == INVALID {
                continue;
            }
            let cc = mesh.corner_prev(mesh.face_corner(nf));
            if center_corner == INVALID {
                mix_corners(mesh, cc, &fc.corners, &w);
                center_corner = cc;
            }
            out.push(nf);
        }
    }
    out
}

/// Turns each edge one step within its two faces. Returns the new edges.
pub fn spin_edges(mesh: &mut Mesh, edges: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for e in unique_sorted(edges) {
        if !mesh.edge_valid(e) || !mesh.is_manifold_edge(e) {
            continue;
        }
        let p = mesh.edge_point(e, 0);
        let q = mesh.edge_point(e, 1);
        let f = mesh.join_faces(e);
        if f == INVALID {
            continue;
        }
        let cp = mesh.find_corner(f, p);
        let cq = mesh.find_corner(f, q);
        let nf = mesh.split_face(f, mesh.corner_next(cp), mesh.corner_next(cq));
        if nf == INVALID {
            continue;
        }
        let ne = mesh.last_split_edge();
        mesh.edge_attrs_mut().copy(ne as usize, e as usize);
        out.push(ne);
    }
    out
}

/// Caps the boundary loops through the given edges (all holes when empty). Returns the new faces.
pub fn fill_holes(mesh: &mut Mesh, edges: &[Index]) -> Vec<Index> {
    let sel = make_mask(mesh.edge_capacity() as usize, edges);
    let mut out = Vec::new();
    for lp in boundary_loops(mesh) {
        if !lp.closed || lp.points.len() < 3 {
            continue;
        }
        let wanted = edges.is_empty() || lp.edges.iter().any(|&e| sel[e as usize] != 0);
        if !wanted {
            continue;
        }
        let src: Vec<Index> = lp.edges.iter().map(|&e| mesh.corner_next(mesh.edge_corner(e))).collect();
        let src_face = mesh.corner_face(mesh.edge_corner(lp.edges[0]));
        let f = mesh.add_face_from(&lp.points, src_face, &src);
        if f != INVALID {
            out.push(f);
        }
    }
    out
}

// Merging

/// Welds points: target[p] is the point p merges into (or p / INVALID to keep it). Survivors keep their positions.
/// Faces that collapse below three points are removed.
pub fn weld_points(mesh: &mut Mesh, target: &[Index]) {
    weld_points_rebuilt(mesh, target, None);
}

fn cell_of(p: DVec3, cell: f64) -> [i64; 3] {
    let q = (p / cell).floor();
    [q.x as i64, q.y as i64, q.z as i64]
}

/// Welds clusters, each into its lowest id, optionally moving it to the cluster average.
fn weld_clusters(mesh: &mut Mesh, uf: &mut UnionFind, members: &[Index], average: bool) -> Vec<Index> {
    let mut target = vec![INVALID; mesh.point_capacity() as usize];
    let mut sums: HashMap<Index, (DVec3, i32)> = HashMap::new();
    for &p in members {
        let r = uf.find(p);
        target[p as usize] = r;
        let s = sums.entry(r).or_insert((DVec3::ZERO, 0));
        s.0 += mesh.position(p);
        s.1 += 1;
    }
    let mut survivors = Vec::new();
    for (&r, s) in &sums {
        if average {
            mesh.set_position(r, s.0 / s.1 as f64);
        }
        survivors.push(r);
    }
    weld_points(mesh, &target);
    unique_sorted(&survivors)
}

#[derive(Clone, Copy, Debug)]
pub struct MergeOptions<'a> {
    pub distance: f64,
    /// Merged points move to their average instead of staying at the lowest id.
    pub average: bool,
    pub symmetry: Option<&'a SymmetryMap>,
}

impl Default for MergeOptions<'_> {
    fn default() -> Self {
        Self { distance: 0.01, average: false, symmetry: None }
    }
}

/// Welds points closer than the distance (all points when the selection is empty). Returns survivors.
pub fn merge_by_distance(mesh: &mut Mesh, points: &[Index], opts: &MergeOptions) -> Vec<Index> {
    let sel: Vec<Index> = if points.is_empty() {
        (0..mesh.point_capacity()).filter(|&p| mesh.point_valid(p)).collect()
    } else {
        match opts.symmetry {
            Some(s) => s.expand_points(points),
            None => unique_sorted(points),
        }
    };
    let cell = opts.distance.max(1e-12);
    let d2max = opts.distance * opts.distance;
    let mut grid: HashMap<[i64; 3], Vec<Index>> = HashMap::new();
    for &p in &sel {
        grid.entry(cell_of(mesh.position(p), cell)).or_default().push(p);
    }
    let mut uf = UnionFind::new(mesh.point_capacity() as usize);
    for &p in &sel {
        let k = cell_of(mesh.position(p), cell);
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else { continue };
                    for &q in list {
                        if q > p && (mesh.position(q) - mesh.position(p)).length_squared() <= d2max {
                            uf.unite(p, q);
                        }
                    }
                }
            }
        }
    }
    let first_new = mesh.point_capacity();
    let out = weld_clusters(mesh, &mut uf, &sel, opts.average);
    apply_symmetry_snap(mesh, opts.symmetry, first_new);
    out
}

/// Welds all given points into one at their centre.
pub fn merge_to_center(mesh: &mut Mesh, points: &[Index]) -> Index {
    let sel = unique_sorted(points);
    if sel.is_empty() {
        return INVALID;
    }
    let mut uf = UnionFind::new(mesh.point_capacity() as usize);
    for &p in &sel {
        uf.unite(sel[0], p);
    }
    weld_clusters(mesh, &mut uf, &sel, true).first().copied().unwrap_or(INVALID)
}

/// Welds the given points into target, keeping its position.
pub fn merge_to_target(mesh: &mut Mesh, points: &[Index], target: Index) -> Index {
    if !mesh.point_valid(target) {
        return INVALID;
    }
    let mut map = vec![INVALID; mesh.point_capacity() as usize];
    for &p in points {
        if mesh.point_valid(p) {
            map[p as usize] = target;
        }
    }
    map[target as usize] = INVALID;
    weld_points(mesh, &map);
    target
}

/// One weld of a target weld: source merges into target.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WeldPair {
    pub source: Index,
    pub target: Index,
}

impl Default for WeldPair {
    fn default() -> Self {
        Self { source: INVALID, target: INVALID }
    }
}

/// Target weld: each source merges into its target, which keeps its position, or with center both meet halfway. A
/// pair whose source is another pair's target is dropped, so welds never chain. Returns the surviving targets.
pub fn target_weld(mesh: &mut Mesh, pairs: &[WeldPair], center: bool) -> Vec<Index> {
    let pc = mesh.point_capacity() as usize;
    let mut map = vec![INVALID; pc];
    let mut is_target = vec![false; pc];
    let mut out = Vec::new();
    for w in pairs {
        if !mesh.point_valid(w.source)
            || !mesh.point_valid(w.target)
            || w.source == w.target
            || is_target[w.source as usize]
            || map[w.source as usize] != INVALID
            || map[w.target as usize] != INVALID
        {
            continue;
        }
        if center {
            let mid = (mesh.position(w.source) + mesh.position(w.target)) * 0.5;
            mesh.set_position(w.target, mid);
        }
        map[w.source as usize] = w.target;
        is_target[w.target as usize] = true;
        out.push(w.target);
    }
    weld_points(mesh, &map);
    unique_sorted(&out)
}

/// The source edge's two points paired with the target edge's the shorter way round; a shared point pairs with
/// itself.
pub fn edge_weld_pairs(mesh: &Mesh, source: Index, target: Index) -> [WeldPair; 2] {
    let (s0, s1) = (mesh.edge_point(source, 0), mesh.edge_point(source, 1));
    let (t0, t1) = (mesh.edge_point(target, 0), mesh.edge_point(target, 1));
    let d = |a: Index, b: Index| mesh.position(a).distance(mesh.position(b));
    let mut crossed = d(s0, t1) + d(s1, t0) < d(s0, t0) + d(s1, t1);
    if s0 == t0 || s1 == t1 {
        crossed = false;
    } else if s0 == t1 || s1 == t0 {
        crossed = true;
    }
    if crossed {
        [WeldPair { source: s0, target: t1 }, WeldPair { source: s1, target: t0 }]
    } else {
        [WeldPair { source: s0, target: t0 }, WeldPair { source: s1, target: t1 }]
    }
}

/// Collapses each connected group of edges to its centre. Returns the resulting points.
pub fn collapse_edges(mesh: &mut Mesh, edges: &[Index]) -> Vec<Index> {
    let mut uf = UnionFind::new(mesh.point_capacity() as usize);
    for &e in edges {
        if mesh.edge_valid(e) {
            uf.unite(mesh.edge_point(e, 0), mesh.edge_point(e, 1));
        }
    }
    let members = points_of_edges(mesh, edges);
    weld_clusters(mesh, &mut uf, &members, true)
}

// Dissolve

/// Removes a point that has exactly two edges, joining them. Returns the rebuilt faces.
fn dissolve_two_valent(mesh: &mut Mesh, p: Index) -> Vec<Index> {
    let mut edges = Vec::new();
    mesh.for_each_point_edge(p, |e| edges.push(e));
    if edges.len() != 2 {
        return Vec::new();
    }
    let copies: Vec<FaceCopy> = mesh.point_faces(p).iter().map(|&f| copy_face(mesh, f)).collect();
    for fc in &copies {
        mesh.remove_face(fc.face);
    }
    mesh.remove_point(p);
    let mut out = Vec::new();
    for mut fc in copies {
        let i = fc.points.iter().position(|&x| x == p).unwrap();
        let before = (i + fc.points.len() - 1) % fc.points.len();
        fc.points.remove(i);
        fc.corners.remove(i);
        // The joined edge takes the attributes of the edge leading into p.
        let into = fc.edges[before];
        fc.edges.remove(i);
        let at = if i == 0 { fc.edges.len() - 1 } else { before };
        fc.edges[at] = into;
        let nf = add_face_from(mesh, &fc.points, fc.face, &fc.corners, &fc.edges);
        if nf != INVALID {
            out.push(nf);
        }
    }
    out
}

/// Two edges at p, each with the same faces.
fn is_winged_point(mesh: &Mesh, p: Index) -> bool {
    let mut edges = Vec::new();
    mesh.for_each_point_edge(p, |e| edges.push(e));
    if edges.len() != 2 {
        return false;
    }
    let faces_of = |e: Index| {
        let mut v = Vec::new();
        mesh.for_each_edge_corner(e, |c| v.push(mesh.corner_face(c)));
        v.sort_unstable();
        v
    };
    let a = faces_of(edges[0]);
    !a.is_empty() && a == faces_of(edges[1])
}

/// Removes edges by merging their faces; with clean, endpoints left with two edges on the same faces go too (the reference).
pub fn dissolve_edges(mesh: &mut Mesh, edges: &[Index], clean: bool) -> Vec<Index> {
    let mut faces = Vec::new();
    let ends = points_of_edges(mesh, edges);
    for e in unique_sorted(edges) {
        if !mesh.edge_valid(e) {
            continue;
        }
        let f = mesh.join_faces(e);
        if f != INVALID {
            faces.push(f);
        }
    }
    if clean {
        for p in ends {
            if mesh.point_valid(p) && is_winged_point(mesh, p) {
                faces.extend(dissolve_two_valent(mesh, p));
            }
        }
    }
    unique_sorted(&faces).into_iter().filter(|&f| mesh.face_valid(f)).collect()
}

/// Removes points by merging the faces around them. Returns the merged faces.
pub fn dissolve_points(mesh: &mut Mesh, points: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    let mut fan = Vec::new();
    for p in unique_sorted(points) {
        if !mesh.point_valid(p) {
            continue;
        }
        if mesh.point_valence(p) == 2 {
            out.extend(dissolve_two_valent(mesh, p));
            continue;
        }
        let mut closed = false;
        if !point_fan(mesh, p, &mut fan, &mut closed) || fan.len() < 2 {
            continue;
        }

        // Chains of each face without p, walked from the last fan face back to the first.
        let mut pts = Vec::new();
        let mut corners = Vec::new();
        let mut edges = Vec::new();
        for k in (0..fan.len()).rev() {
            let c = fan[k];
            let mut first = true;
            let mut x = mesh.corner_next(c);
            while x != c {
                if first && k + 1 != fan.len() {
                    first = false;
                    x = mesh.corner_next(x);
                    continue;
                }
                first = false;
                pts.push(mesh.corner_point(x));
                corners.push(x);
                edges.push(mesh.corner_edge(x));
                x = mesh.corner_next(x);
            }
        }
        if closed {
            pts.pop();
            corners.pop();
            edges.pop();
        }
        let mut sorted = pts.clone();
        sorted.sort_unstable();
        if pts.len() < 3 || sorted.windows(2).any(|w| w[0] == w[1]) {
            continue;
        }
        let src_face = mesh.corner_face(fan[0]);
        let fan_faces: Vec<Index> = fan.iter().map(|&c| mesh.corner_face(c)).collect();
        for f in fan_faces {
            mesh.remove_face(f);
        }
        mesh.remove_point(p);
        let nf = add_face_from(mesh, &pts, src_face, &corners, &edges);
        if nf != INVALID {
            out.push(nf);
        }
    }
    out
}

/// Splits each edge into count+1 pieces. Returns the new points.
pub fn subdivide_edges(mesh: &mut Mesh, edges: &[Index], count: i32) -> Vec<Index> {
    let mut out = Vec::new();
    for e in unique_sorted(edges) {
        if !mesh.edge_valid(e) {
            continue;
        }
        let mut cur = e;
        for i in 0..count {
            out.push(mesh.split_edge(cur, 1.0 / (count + 1 - i) as f64));
            cur = mesh.last_split_edge();
        }
    }
    out
}

// Whole-mesh

/// Splits faces between selected points that are consecutive (among the selected) around a face but not already
/// joined by an edge, the reference's Connect. Returns the new edges.
pub fn connect_points(mesh: &mut Mesh, points: &[Index]) -> Vec<Index> {
    let chosen = make_mask(mesh.point_capacity() as usize, points);
    let mut work = faces_of_points(mesh, points);
    let mut out = Vec::new();
    while let Some(f) = work.pop() {
        if !mesh.face_valid(f) {
            continue;
        }
        let corners: Vec<Index> =
            mesh.face_corners(f).into_iter().filter(|&c| chosen[mesh.corner_point(c) as usize] != 0).collect();
        if corners.len() < 2 {
            continue;
        }
        for i in 0..corners.len() {
            let a = corners[i];
            let b = corners[(i + 1) % corners.len()];
            if a == b
                || mesh.corner_next(a) == b
                || mesh.corner_prev(a) == b
                || mesh.find_edge(mesh.corner_point(a), mesh.corner_point(b)) != INVALID
            {
                continue;
            }
            let g = mesh.split_face(f, a, b);
            if g == INVALID {
                continue;
            }
            out.push(mesh.last_split_edge());
            // Both halves may still hold another unjoined pair.
            work.push(f);
            work.push(g);
            break;
        }
    }
    out
}

/// A new mesh holding copies of the given faces with all attributes.
pub fn extract_faces(mesh: &Mesh, faces: &[Index]) -> Mesh {
    let mut out = Mesh::new();
    for d in Domain::ALL {
        out.attributes_mut(d).merge_layout(mesh.attributes(d));
    }
    *out.material_names_mut() = mesh.material_names().clone();
    *out.color_set_info_mut() = mesh.color_set_info().clone();
    let mut pmap = vec![INVALID; mesh.point_capacity() as usize];
    for p in points_of_faces(mesh, faces) {
        let np = out.add_point(mesh.position(p));
        pmap[p as usize] = np;
        out.point_attrs_mut().copy_from(mesh.point_attrs(), np as usize, p as usize);
    }
    let mut pts = Vec::new();
    for f in unique_sorted(faces) {
        if !mesh.face_valid(f) {
            continue;
        }
        pts.clear();
        mesh.for_each_face_corner(f, |c| pts.push(pmap[mesh.corner_point(c) as usize]));
        let edge_cap = out.edge_capacity();
        let nf = out.add_face(&pts);
        if nf == INVALID {
            continue;
        }
        out.face_attrs_mut().copy_from(mesh.face_attrs(), nf as usize, f as usize);
        let mut c = out.face_corner(nf);
        for oc in mesh.face_corners(f) {
            out.corner_attrs_mut().copy_from(mesh.corner_attrs(), c as usize, oc as usize);
            let e = out.corner_edge(c);
            if e >= edge_cap {
                out.edge_attrs_mut().copy_from(mesh.edge_attrs(), e as usize, mesh.corner_edge(oc) as usize);
            }
            c = out.corner_next(c);
        }
    }
    out
}

pub fn combine(meshes: &[&Mesh]) -> Mesh {
    let mut out = Mesh::new();
    for m in meshes {
        out.append(m);
    }
    out
}

/// One mesh per connected part.
pub fn separate(mesh: &Mesh) -> Vec<Mesh> {
    let mut comp = Vec::new();
    let count = face_components(mesh, &mut comp, &[]);
    let mut groups = vec![Vec::new(); count as usize];
    for f in 0..mesh.face_capacity() {
        if comp[f as usize] != INVALID {
            groups[comp[f as usize] as usize].push(f);
        }
    }
    groups.iter().map(|g| extract_faces(mesh, g)).collect()
}

/// What cut_along_plane changed.
#[derive(Clone, Debug, Default)]
pub struct PlaneCut {
    /// Edges lying on the cut.
    pub edges: Vec<Index>,
    /// Faces the cut split off (the originals keep one side).
    pub new_faces: Vec<Index>,
}

/// Cuts faces along the plane through origin with that normal, keeping both sides: crossing edges get a point on the
/// plane and crossing faces are split there. Points within tolerance snap onto it. `within` (optional) limits the
/// cut to plane points it accepts, such as a disc; `faces` (optional) to those faces, whose edges shared with other
/// faces still gain the point.
pub fn cut_along_plane(
    m: &mut Mesh,
    origin: DVec3,
    normal: DVec3,
    tol: f64,
    within: Option<&dyn Fn(DVec3) -> bool>,
    faces: &[Index],
) -> PlaneCut {
    let mut out = PlaneCut::default();
    let len = normal.length();
    if len <= 0.0 {
        return out;
    }
    let n = normal / len;
    let dist = |x: DVec3| n.dot(x - origin);
    let inside = |x: DVec3| within.is_none_or(|w| w(x - dist(x) * n));
    let on_plane = |m: &Mesh, p: Index| {
        let x = m.position(p);
        dist(x).abs() <= tol && inside(x)
    };
    let side = |m: &Mesh, p: Index| {
        if on_plane(m, p) {
            0
        } else if dist(m.position(p)) >= 0.0 {
            1
        } else {
            -1
        }
    };

    let todo: Vec<Index> =
        if faces.is_empty() { (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect() } else { faces.to_vec() };
    let mut edge_in = vec![false; m.edge_capacity() as usize];
    for &f in &todo {
        if !m.face_valid(f) {
            continue;
        }
        for c in m.face_corners(f) {
            edge_in[m.corner_edge(c) as usize] = true;
            let p = m.corner_point(c);
            if on_plane(m, p) {
                let x = m.position(p);
                m.set_position(p, x - dist(x) * n);
            }
        }
    }

    for e in 0..edge_in.len() as Index {
        if !edge_in[e as usize] || !m.edge_valid(e) {
            continue;
        }
        let a = m.position(m.edge_point(e, 0));
        let b = m.position(m.edge_point(e, 1));
        let da = dist(a);
        let db = dist(b);
        if !((da > tol && db < -tol) || (da < -tol && db > tol)) {
            continue;
        }
        let t = da / (da - db);
        let x = a + (b - a) * t;
        if !inside(x) {
            continue;
        }
        let mid = m.split_edge(e, t);
        m.set_position(mid, x - dist(x) * n);
    }

    // Each sign change round a face now passes through an on-plane point: split between consecutive ones.
    for &f in &todo {
        let mut face = f;
        while m.face_valid(face) {
            let mut pos = false;
            let mut neg = false;
            let mut start = INVALID;
            for c in m.face_corners(face) {
                let sd = side(m, m.corner_point(c));
                pos |= sd > 0;
                neg |= sd < 0;
                if start == INVALID && sd == 0 && side(m, m.corner_next_point(c)) > 0 {
                    start = c;
                }
            }
            if !pos || !neg || start == INVALID {
                break;
            }
            let mut end = m.corner_next(start);
            while side(m, m.corner_point(end)) != 0 {
                end = m.corner_next(end);
            }
            if end == start || m.corner_next(start) == end || m.corner_prev(start) == end {
                break;
            }
            let rest = m.split_face(face, start, end);
            if rest == INVALID {
                break;
            }
            out.new_faces.push(rest);
            face = rest;
        }
    }

    let split_count = out.new_faces.len();
    out.new_faces.extend_from_slice(&todo);
    for &f in &out.new_faces {
        if !m.face_valid(f) {
            continue;
        }
        for c in m.face_corners(f) {
            let e = m.corner_edge(c);
            if on_plane(m, m.edge_point(e, 0)) && on_plane(m, m.edge_point(e, 1)) && !out.edges.contains(&e) {
                out.edges.push(e);
            }
        }
    }
    out.new_faces.truncate(split_count);
    out
}

fn transform_point(m: &DMat4, p: DVec3) -> DVec3 {
    (*m * DVec4::new(p.x, p.y, p.z, 1.0)).truncate()
}

/// Cuts every face crossing the plane along it, then removes the faces on the side opposite keep_sign (+1 or -1)
/// and faces lying in the plane. Points within tolerance of the plane snap onto it.
pub fn cut_at_plane(mesh: &mut Mesh, plane: &SymmetryPlane, keep_sign: i32, tolerance: f64, space: &DMat4) {
    let identity = *space == DMat4::IDENTITY;
    let inv = if identity { *space } else { space.inverse() };
    let to_plane = |p: DVec3| if identity { p } else { transform_point(space, p) };
    let from_plane = |p: DVec3| if identity { p } else { transform_point(&inv, p) };
    let distance = |m: &Mesh, p: Index| plane.distance(to_plane(m.position(p)));
    let side = |m: &Mesh, p: Index| {
        let d = distance(m, p);
        if d > tolerance {
            1
        } else if d < -tolerance {
            -1
        } else {
            0
        }
    };

    for p in 0..mesh.point_capacity() {
        if mesh.point_valid(p) && side(mesh, p) == 0 {
            let v = from_plane(plane.project(to_plane(mesh.position(p))));
            mesh.set_position(p, v);
        }
    }

    let edge_cap = mesh.edge_capacity();
    for e in 0..edge_cap {
        if !mesh.edge_valid(e) {
            continue;
        }
        let a = mesh.edge_point(e, 0);
        let b = mesh.edge_point(e, 1);
        if side(mesh, a) * side(mesh, b) >= 0 {
            continue;
        }
        let da = distance(mesh, a);
        let db = distance(mesh, b);
        let m = mesh.split_edge(e, da / (da - db));
        let v = from_plane(plane.project(to_plane(mesh.position(m))));
        mesh.set_position(m, v);
    }

    // Every sign change round a face now passes through an on-plane point, so each positive run is bounded by two.
    for f in 0..mesh.face_capacity() {
        let mut face = f;
        while mesh.face_valid(face) {
            let mut pos = false;
            let mut neg = false;
            let mut start = INVALID;
            for c in mesh.face_corners(face) {
                let s = side(mesh, mesh.corner_point(c));
                pos |= s > 0;
                neg |= s < 0;
                if start == INVALID && s == 0 && side(mesh, mesh.corner_next_point(c)) > 0 {
                    start = c;
                }
            }
            if !pos || !neg || start == INVALID {
                break;
            }
            let mut end = mesh.corner_next(start);
            while side(mesh, mesh.corner_point(end)) != 0 {
                end = mesh.corner_next(end);
            }
            let rest = mesh.split_face(face, start, end);
            if rest == INVALID {
                break;
            }
            face = rest;
        }
    }

    let mut drop = Vec::new();
    for f in 0..mesh.face_capacity() {
        if !mesh.face_valid(f) {
            continue;
        }
        let mut kept = false;
        let mut discarded = false;
        mesh.for_each_face_corner(f, |c| {
            let s = side(mesh, mesh.corner_point(c));
            kept |= s == keep_sign;
            discarded |= s == -keep_sign;
        });
        if discarded || !kept {
            drop.push(f);
        }
    }
    for f in drop {
        mesh.remove_face(f);
    }
    mesh.remove_all_loose();
}

/// Which side of the plane a mirror keeps: Positive keeps the side the normal points to and rebuilds the other from
/// it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MirrorDirection {
    Positive,
    Negative,
    #[default]
    Both,
}

#[derive(Clone, Debug)]
pub struct MirrorOptions {
    pub plane: SymmetryPlane,
    /// Positive and Negative cut the mesh along the plane first; Both mirrors all of it.
    pub direction: MirrorDirection,
    /// Maps mesh points into the space the plane is given in, such as the object's world matrix.
    pub space: DMat4,
    /// Merge points on the plane with their mirror images.
    pub weld: bool,
    /// Points this close to the plane, in plane space, count as on it.
    pub tolerance: f64,
}

impl Default for MirrorOptions {
    fn default() -> Self {
        Self {
            plane: SymmetryPlane::default(),
            direction: MirrorDirection::Both,
            space: DMat4::IDENTITY,
            weld: true,
            tolerance: 1e-3,
        }
    }
}

/// Adds a mirrored copy, after cutting away the discarded side unless the direction is Both. Returns the new faces.
pub fn mirror_mesh(mesh: &mut Mesh, opts: &MirrorOptions) -> Vec<Index> {
    let mut plane = opts.plane;
    let len = plane.normal.length();
    plane.normal /= len;
    plane.offset /= len;
    if opts.direction != MirrorDirection::Both {
        cut_at_plane(
            mesh,
            &plane,
            if opts.direction == MirrorDirection::Positive { 1 } else { -1 },
            opts.tolerance,
            &opts.space,
        );
    }

    // Off identity the reflection is the plane-space one carried back into mesh space.
    let identity = opts.space == DMat4::IDENTITY;
    let inv = if identity { opts.space } else { opts.space.inverse() };
    let mut reflect = DMat4::IDENTITY;
    if !identity {
        let mut r = DMat4::IDENTITY.to_cols_array_2d();
        for col in 0..3 {
            for row in 0..3 {
                r[col][row] -= 2.0 * plane.normal[row] * plane.normal[col];
            }
        }
        let t = 2.0 * plane.offset * plane.normal;
        r[3] = [t.x, t.y, t.z, 1.0];
        reflect = inv * DMat4::from_cols_array_2d(&r) * opts.space;
    }
    let linear: Mat3 = DMat3::from_mat4(reflect).as_mat3();
    let normal_xform = linear.inverse().transpose();
    let in_plane = |p: DVec3| if identity { p } else { transform_point(&opts.space, p) };

    let live: Vec<Index> = (0..mesh.face_capacity()).filter(|&f| mesh.face_valid(f)).collect();
    let mut copy = extract_faces(mesh, &live);
    for p in copy.positions_mut() {
        *p = if identity { plane.reflect(*p) } else { transform_point(&reflect, *p) };
    }
    for f in 0..copy.face_capacity() {
        copy.flip_face(f);
    }
    let n = plane.normal.as_vec3();
    for v in copy.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL) {
        let x = Vec3::from_array(*v);
        let r = if identity { x - 2.0 * x.dot(n) * n } else { (normal_xform * x).normalize() };
        *v = r.to_array();
    }
    for v in copy.corner_attrs_mut().get_mut::<[f32; 4]>(names::TANGENT) {
        let t = Vec3::new(v[0], v[1], v[2]);
        let t = if identity { t - 2.0 * t.dot(n) * n } else { (linear * t).normalize() };
        *v = [t.x, t.y, t.z, -v[3]];
    }

    // extract_faces keeps the original point order, so copy point i matches the i-th used point.
    let used = points_of_faces(mesh, &live);
    let first_face = mesh.face_capacity();
    let offset = mesh.append(&copy);
    let mut out: Vec<Index> = (first_face..mesh.face_capacity()).filter(|&f| mesh.face_valid(f)).collect();

    if opts.weld {
        let mut target = vec![INVALID; mesh.point_capacity() as usize];
        for (i, &p) in used.iter().enumerate() {
            let at = in_plane(mesh.position(p));
            if plane.distance(at).abs() <= opts.tolerance {
                let v = if identity { plane.project(at) } else { transform_point(&inv, plane.project(at)) };
                mesh.set_position(p, v);
                target[(offset + i as Index) as usize] = p;
            }
        }
        weld_points(mesh, &target);
        let mut alive: Vec<Index> = out.iter().copied().filter(|&f| mesh.face_valid(f)).collect();
        // Welding rebuilds faces on the seam with new ids.
        for f in first_face..mesh.face_capacity() {
            if mesh.face_valid(f) && !alive.contains(&f) {
                alive.push(f);
            }
        }
        out = unique_sorted(&alive);
    }
    out
}

/// Rebuilds the mesh symmetric about an axis plane at offset along the axis: the side opposite keep_sign (+1 or -1)
/// is cut away and replaced by the mirror of the kept side, welded on the plane. Compacts the mesh and stores the
/// exact pairs in the axis's symmetry layer (other axes' layers are dropped). Returns the map.
pub fn symmetrize_topology(mesh: &mut Mesh, axis: i32, keep_sign: i32, tolerance: f64, offset: f64) -> SymmetryMap {
    let axis = axis.clamp(0, 2);
    let o = MirrorOptions {
        plane: axis_plane(axis, offset),
        direction: if keep_sign < 0 { MirrorDirection::Negative } else { MirrorDirection::Positive },
        tolerance,
        ..Default::default()
    };
    mirror_mesh(mesh, &o);
    // The mirrored half carries copies of the kept half's keys, which pair nothing.
    remove_symmetry_layers(mesh);
    mesh.compact(None);
    let map = SymmetryMap::build(mesh, &o.plane, tolerance);
    map.store_axis(mesh, axis);
    map
}

/// Disjoint sets where the first argument's root survives a union.
struct KeepSets {
    parent: Vec<Index>,
}

impl KeepSets {
    fn new(n: Index) -> Self {
        Self { parent: (0..n).collect() }
    }
    fn find(&mut self, mut p: Index) -> Index {
        while self.parent[p as usize] != p {
            let gp = self.parent[self.parent[p as usize] as usize];
            self.parent[p as usize] = gp;
            p = gp;
        }
        p
    }
    fn unite(&mut self, keep: Index, other: Index) {
        let a = self.find(keep);
        let b = self.find(other);
        if a != b {
            self.parent[b as usize] = a;
        }
    }
}

/// wedge carried by element g of radial: positions, corner normals and tangents, and winding when g reflects.
fn carried(wedge: &Mesh, radial: &RadialSymmetry, g: i32) -> Mesh {
    let mut copy = wedge.clone();
    if g == 0 {
        return copy;
    }
    for p in copy.positions_mut() {
        *p = radial.apply(g, *p);
    }
    let linear = radial.linear(g).as_mat3();
    for c in copy.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL) {
        *c = (linear * Vec3::from_array(*c)).to_array();
    }
    let reflects = radial.reflects(g);
    for c in copy.corner_attrs_mut().get_mut::<[f32; 4]>(names::TANGENT) {
        let t = linear * Vec3::new(c[0], c[1], c[2]);
        *c = [t.x, t.y, t.z, if reflects { -c[3] } else { c[3] }];
    }
    if reflects {
        for f in 0..copy.face_capacity() {
            copy.flip_face(f);
        }
    }
    copy
}

/// Pairs the fundamental wedge's points on its far boundary with those on its near one, turned back by a wedge,
/// nearest first. Empty when the two seams hold different numbers of points.
fn wedge_seam_pairs(wedge: &Mesh, radial: &RadialSymmetry, tolerance: f64) -> Vec<(Index, Index)> {
    let a = radial.axis();
    let u = radial.reference();
    let v = a.cross(u);
    let o = radial.origin();
    let t = radial.wedge_angle();
    let far = t.cos() * u + t.sin() * v;
    let mut near_seam = Vec::new();
    let mut far_seam = Vec::new();
    for p in 0..wedge.point_capacity() {
        if !wedge.point_valid(p) {
            continue;
        }
        let d = wedge.position(p) - o;
        if d.dot(a.cross(u)).abs() <= tolerance && d.dot(u) >= -tolerance {
            near_seam.push(p);
        }
        if d.dot(a.cross(far)).abs() <= tolerance && d.dot(far) >= -tolerance {
            far_seam.push(p);
        }
    }
    let mut out = Vec::new();
    if near_seam.len() != far_seam.len() || near_seam.len() > 4096 {
        return out;
    }
    let back = radial.inverse(1);
    let mut all = Vec::new();
    for (i, &fp) in far_seam.iter().enumerate() {
        let at = radial.apply(back, wedge.position(fp));
        for (j, &np) in near_seam.iter().enumerate() {
            all.push(((at - wedge.position(np)).length(), i, j));
        }
    }
    all.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut used_far = vec![false; far_seam.len()];
    let mut used_near = vec![false; near_seam.len()];
    for (_, i, j) in all {
        if !used_far[i] && !used_near[j] {
            used_far[i] = true;
            used_near[j] = true;
            out.push((far_seam[i], near_seam[j]));
        }
    }
    out
}

/// Cuts away everything outside the wedge of element keep: the fundamental wedge carried by keep.
pub fn cut_to_wedge(mesh: &mut Mesh, radial: &RadialSymmetry, keep: i32, tolerance: f64) {
    if radial.order() < 2 {
        return;
    }
    // The fundamental wedge runs from the reference direction to wedge_angle past it: in front of the first plane,
    // behind the second. keep's wedge is bounded by those planes carried by keep.
    let a = radial.axis();
    let u = radial.reference();
    let v = a.cross(u);
    let t = radial.wedge_angle();
    let bounds = [a.cross(u), a.cross(t.cos() * u + t.sin() * v)];
    for (k, b) in bounds.iter().enumerate() {
        let normal = radial.apply_vector(keep.clamp(0, radial.order() - 1), *b).normalize();
        let plane = SymmetryPlane { normal, offset: normal.dot(radial.origin()) };
        cut_at_plane(mesh, &plane, if k == 0 { 1 } else { -1 }, tolerance, &DMat4::IDENTITY);
    }
}

/// Rebuilds the mesh exactly radially symmetric: everything outside the wedge of element keep is cut away, and the
/// kept wedge is copied into every other slot (reflected ones flipped for dihedral symmetry) and welded along the
/// seams and the axis. Compacts the mesh, drops every other symmetry layer and stores the orbits. Returns the map.
pub fn symmetrize_radial_topology(mesh: &mut Mesh, radial: &RadialSymmetry, keep: i32, tolerance: f64) -> SymmetryMap {
    let n = radial.order();
    let keep = keep.clamp(0, n - 1);
    remove_symmetry_layers(mesh);
    cut_to_wedge(mesh, radial, keep, tolerance);

    let live: Vec<Index> = (0..mesh.face_capacity()).filter(|&f| mesh.face_valid(f)).collect();
    // The kept wedge carried back onto the fundamental one, then into every slot.
    let wedge = carried(&extract_faces(mesh, &live), radial, radial.inverse(keep));
    *mesh = Mesh::new();
    let offsets: Vec<Index> = (0..n).map(|g| mesh.append(&carried(&wedge, radial, g))).collect();

    let mut sets = KeepSets::new(mesh.point_capacity());
    // Turns alone meet a copy's far seam against the next copy's near one, which need not lie at the same spots: pair
    // them up, the near seam's points surviving. Reflections meet each seam against its own mirror image.
    if !radial.mirror() && radial.count() > 1 {
        for (far, near_point) in wedge_seam_pairs(&wedge, radial, tolerance) {
            for g in 0..n {
                sets.unite(offsets[radial.compose(1, g) as usize] + near_point, offsets[g as usize] + far);
            }
        }
    }

    // Whatever else coincides (the axis, a mirror seam) merges into the first point there.
    let cell = (tolerance * 2.0).max(1e-9);
    let mut grid: HashMap<[i64; 3], Vec<Index>> = HashMap::new();
    for p in 0..mesh.point_capacity() {
        if !mesh.point_valid(p) {
            continue;
        }
        let at = mesh.position(p);
        let k = cell_of(at, cell);
        let mut found = INVALID;
        'search: for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    if let Some(list) = grid.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) {
                        if let Some(&q) = list.iter().find(|&&q| (mesh.position(q) - at).length() <= tolerance) {
                            found = q;
                            break 'search;
                        }
                    }
                }
            }
        }
        if found != INVALID {
            sets.unite(found, p);
        } else {
            grid.entry(k).or_default().push(p);
        }
    }
    let mut target = vec![INVALID; mesh.point_capacity() as usize];
    let mut any = false;
    for p in 0..mesh.point_capacity() {
        if mesh.point_valid(p) && sets.find(p) != p {
            target[p as usize] = sets.find(p);
            any = true;
        }
    }
    if any {
        weld_points(mesh, &target);
    }
    mesh.remove_all_loose();
    mesh.compact(None);

    let map = SymmetryMap::build_radial(mesh, radial, tolerance);
    let points: Vec<Index> = (0..mesh.point_capacity()).collect();
    map.snap_to_plane(mesh, &points);
    map.symmetrize_wedge(mesh, 0);
    map.store(mesh);
    map
}
