//! Geometry behind the polygon commands that had no op of their own: moving component groups in their local frames,
//! vertex extrude and chamfer, circularize, splitting vertices and edges apart, face subdivision, facet merging,
//! vertex reordering, and isotropic remeshing.

use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::attribute::{Index, INVALID};
use crate::bvh::MeshBvh;
use crate::mesh::Mesh;
use crate::ops::basic::{collapse_edges, dissolve_points, poke_faces, quadrangulate_faces, triangulate_faces, QuadrangulateOptions};
use crate::ops::common::UnionFind;
use crate::triangulate::TriangulateOptions;

/// The average normal of the faces round a point.
pub fn point_normal(m: &Mesh, p: Index) -> DVec3 {
    let mut n = DVec3::ZERO;
    for f in m.point_faces(p) {
        n += m.face_normal(f);
    }
    n.normalize_or_zero()
}

// ---------------------------------------------------------------------------------------------------------------------
// Moving groups of points

/// A group of points that moves as one in its own frame: one face, one region, one edge or one vertex.
#[derive(Clone, Debug, Default)]
pub struct MoveGroup {
    pub points: Vec<Index>,
    pub center: DVec3,
    pub normal: DVec3,
    pub tangent: DVec3,
}

/// The settings every polyMove and extrude node shares. World values apply about the pivot, local ones in each group's
/// frame (x along its first edge, z along its normal).
#[derive(Clone, Debug)]
pub struct MoveSpec {
    pub translate: DVec3,
    /// Degrees.
    pub rotate: DVec3,
    pub scale: DVec3,
    pub pivot: DVec3,
    pub local_translate: DVec3,
    pub local_rotate: DVec3,
    pub local_scale: DVec3,
    /// Used for local z when not zero, instead of each group's normal.
    pub local_direction: DVec3,
    pub gain: f64,
    pub random: f64,
    pub gravity: DVec3,
    pub weight: f64,
    pub magnet: DVec3,
    pub attraction: f64,
    /// Object space to the space the world values are in.
    pub to_world: DMat4,
}

impl Default for MoveSpec {
    fn default() -> Self {
        Self {
            translate: DVec3::ZERO,
            rotate: DVec3::ZERO,
            scale: DVec3::ONE,
            pivot: DVec3::ZERO,
            local_translate: DVec3::ZERO,
            local_rotate: DVec3::ZERO,
            local_scale: DVec3::ONE,
            local_direction: DVec3::ZERO,
            gain: 1.0,
            random: 0.0,
            gravity: DVec3::ZERO,
            weight: 0.0,
            magnet: DVec3::ZERO,
            attraction: 0.0,
            to_world: DMat4::IDENTITY,
        }
    }
}

fn euler_degrees(v: DVec3) -> DQuat {
    DQuat::from_euler(glam::EulerRot::ZYX, v.z.to_radians(), v.y.to_radians(), v.x.to_radians())
}

/// A repeatable number in 0..1 from a seed.
fn jitter(seed: u32) -> f64 {
    let mut x = seed.wrapping_mul(2654435761).wrapping_add(0x9e3779b9);
    x ^= x >> 15;
    x = x.wrapping_mul(2246822519);
    x ^= x >> 13;
    x as f64 / u32::MAX as f64
}

/// Moves the groups' points; a point in several groups takes the average of what they ask.
pub fn move_groups(m: &mut Mesh, groups: &[MoveGroup], spec: &MoveSpec) {
    let world = spec.to_world;
    let to_local = world.inverse();
    let about_pivot = DMat4::from_translation(spec.pivot + spec.translate + spec.gravity * spec.weight)
        * DMat4::from_quat(euler_degrees(spec.rotate))
        * DMat4::from_scale(spec.scale)
        * DMat4::from_translation(-spec.pivot);
    let mut sums: BTreeMap<Index, (DVec3, f64)> = BTreeMap::new();
    for (k, g) in groups.iter().enumerate() {
        let normal = if spec.local_direction.length_squared() > 1e-12 { spec.local_direction.normalize() } else { g.normal };
        let x = (g.tangent - normal * g.tangent.dot(normal)).normalize_or_zero();
        let y = normal.cross(x);
        let frame = DMat3::from_cols(x, y, normal);
        let inv = frame.transpose();
        let spin = DMat3::from_quat(euler_degrees(spec.local_rotate));
        let amount = spec.gain * if spec.random != 0.0 { 1.0 + spec.random * (jitter(k as u32) * 2.0 - 1.0) } else { 1.0 };
        for &p in &g.points {
            let at = m.position(p);
            let d = inv * (at - g.center);
            let d = spin * (d * (DVec3::ONE + (spec.local_scale - DVec3::ONE) * amount));
            let local = g.center + frame * (d + spec.local_translate * amount);
            let w = about_pivot.transform_point3(world.transform_point3(local));
            let mut out = to_local.transform_point3(w);
            if spec.attraction != 0.0 {
                out = out.lerp(to_local.transform_point3(spec.magnet), (spec.attraction * spec.weight).clamp(-1.0, 1.0));
            }
            let e = sums.entry(p).or_insert((DVec3::ZERO, 0.0));
            e.0 += out;
            e.1 += 1.0;
        }
    }
    for (p, (sum, n)) in sums {
        m.set_position(p, sum / n);
    }
}

/// A group for some faces: centre of their points, area-weighted normal, first edge as tangent.
pub fn face_group(m: &Mesh, faces: &[Index]) -> MoveGroup {
    let mut points = BTreeSet::new();
    let mut area = DVec3::ZERO;
    let mut tangent = DVec3::X;
    let mut first = true;
    for &f in faces {
        if !m.face_valid(f) {
            continue;
        }
        area += m.face_area_vector(f);
        let pts = m.face_points(f);
        if first && pts.len() > 1 {
            tangent = (m.position(pts[1]) - m.position(pts[0])).normalize_or_zero();
            first = false;
        }
        points.extend(pts);
    }
    let points: Vec<Index> = points.into_iter().collect();
    let center = points.iter().map(|&p| m.position(p)).sum::<DVec3>() / points.len().max(1) as f64;
    MoveGroup { points, center, normal: area.normalize_or_zero(), tangent }
}

/// Face groups: each face alone, or each connected region when together.
pub fn face_groups(m: &Mesh, faces: &[Index], together: bool) -> Vec<MoveGroup> {
    if !together {
        return faces.iter().filter(|&&f| m.face_valid(f)).map(|&f| face_group(m, &[f])).collect();
    }
    regions(m, faces).iter().map(|r| face_group(m, r)).collect()
}

/// The faces split into regions joined through shared edges.
pub fn regions(m: &Mesh, faces: &[Index]) -> Vec<Vec<Index>> {
    let set: BTreeSet<Index> = faces.iter().copied().filter(|&f| m.face_valid(f)).collect();
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for &f in &set {
        if !seen.insert(f) {
            continue;
        }
        let mut region = vec![f];
        let mut todo = vec![f];
        while let Some(x) = todo.pop() {
            for c in m.face_corners(x) {
                m.for_each_edge_corner(m.corner_edge(c), |r| {
                    let o = m.corner_face(r);
                    if set.contains(&o) && seen.insert(o) {
                        region.push(o);
                        todo.push(o);
                    }
                });
            }
        }
        out.push(region);
    }
    out
}

/// One group per edge.
pub fn edge_groups(m: &Mesh, edges: &[Index]) -> Vec<MoveGroup> {
    let mut out = Vec::new();
    for &e in edges {
        if !m.edge_valid(e) {
            continue;
        }
        let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
        let normal = (point_normal(m, a) + point_normal(m, b)).normalize_or_zero();
        out.push(MoveGroup {
            points: vec![a, b],
            center: (m.position(a) + m.position(b)) * 0.5,
            normal,
            tangent: (m.position(b) - m.position(a)).normalize_or_zero(),
        });
    }
    out
}

/// One group per vertex.
pub fn vertex_groups(m: &Mesh, points: &[Index]) -> Vec<MoveGroup> {
    let mut out = Vec::new();
    for &p in points {
        if !m.point_valid(p) {
            continue;
        }
        let normal = point_normal(m, p);
        let mut tangent = DVec3::X;
        let e = m.point_edge(p);
        if e != INVALID {
            tangent = (m.position(m.edge_other_point(e, p)) - m.position(p)).normalize_or_zero();
        }
        out.push(MoveGroup { points: vec![p], center: m.position(p), normal, tangent });
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Vertex extrude and chamfer

/// Cuts a ring round p: each edge at p gets a new point at at(edge length) from p, and each face at p loses its corner
/// there to a triangle with p. Returns the triangles.
fn cut_ring(m: &mut Mesh, p: Index, at: &dyn Fn(f64) -> f64) -> Vec<Index> {
    let mut edges = Vec::new();
    m.for_each_point_edge(p, |e| edges.push(e));
    for e in edges {
        let t = at(m.edge_length(e)).clamp(1e-4, 0.999);
        m.split_edge(e, if m.edge_point(e, 0) == p { t } else { 1.0 - t });
    }
    let mut tris = Vec::new();
    for f in m.point_faces(p) {
        let c = m.find_corner(f, p);
        if c == INVALID {
            continue;
        }
        let (prev, next) = (m.corner_prev(c), m.corner_next(c));
        if m.corner_next(next) == prev {
            tris.push(f);
            continue;
        }
        if m.split_face(f, prev, next) != INVALID {
            // Whichever of the two now holds p is the triangle.
            tris.extend(m.point_faces(p).into_iter().filter(|&x| m.face_size(x) == 3));
        }
    }
    tris.sort_unstable();
    tris.dedup();
    tris
}

/// polyExtrudeVertex: each vertex becomes the tip of a pyramid, width along each edge at its base, length along the
/// vertex normal, divisions rings up its sides. Returns the pyramid's faces.
pub fn extrude_vertices(m: &mut Mesh, points: &[Index], width: f64, length: f64, divisions: i32) -> Vec<Index> {
    let mut out = Vec::new();
    for &p in points {
        if !m.point_valid(p) {
            continue;
        }
        let normal = point_normal(m, p);
        cut_ring(m, p, &|len| width / len.max(1e-12));
        m.set_position(p, m.position(p) + normal * length);
        for k in 1..divisions.max(1) {
            let t = 1.0 / (divisions - k + 1) as f64;
            // From the tip: the far end of each spoke keeps its place, so rings land evenly up it.
            cut_ring(m, p, &|_| 1.0 - t);
        }
        out.extend(m.point_faces(p));
    }
    out.sort_unstable();
    out.dedup();
    out
}

/// Chamfer Vertices: each vertex is cut off, leaving one face where it was. Returns those faces.
pub fn chamfer_vertices(m: &mut Mesh, points: &[Index], width: f64) -> Vec<Index> {
    let mut out = Vec::new();
    for &p in points {
        if !m.point_valid(p) {
            continue;
        }
        cut_ring(m, p, &|len| width / len.max(1e-12));
        out.extend(dissolve_points(m, &[p]));
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Circularize

/// Puts an ordered loop of points on its best-fit circle: centre at their average, radius their average distance plus
/// radial_offset, in the plane of their area vector. even spaces them equally from the first point's angle.
pub fn circularize_loop(m: &mut Mesh, loop_points: &[Index], radial_offset: f64, even: bool, twist_degrees: f64) {
    let n = loop_points.len();
    if n < 3 {
        return;
    }
    let at: Vec<DVec3> = loop_points.iter().map(|&p| m.position(p)).collect();
    let center = at.iter().copied().sum::<DVec3>() / n as f64;
    let mut normal = DVec3::ZERO;
    for i in 0..n {
        normal += (at[i] - center).cross(at[(i + 1) % n] - center);
    }
    let normal = normal.normalize_or_zero();
    if normal == DVec3::ZERO {
        return;
    }
    let flat: Vec<DVec3> = at.iter().map(|&q| (q - center) - normal * (q - center).dot(normal)).collect();
    let radius = flat.iter().map(|v| v.length()).sum::<f64>() / n as f64 + radial_offset;
    let x = flat[0].normalize_or_zero();
    let y = normal.cross(x);
    let start = twist_degrees.to_radians();
    for i in 0..n {
        let a = if even {
            start + std::f64::consts::TAU * i as f64 / n as f64
        } else {
            flat[i].dot(y).atan2(flat[i].dot(x)) + start
        };
        m.set_position(loop_points[i], center + (x * a.cos() + y * a.sin()) * radius);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Splitting apart

fn new_copy(m: &mut Mesh, p: Index) -> Index {
    let q = m.add_point(m.position(p));
    m.point_attrs_mut().copy(q as usize, p as usize);
    q
}

fn move_corner(m: &mut Mesh, f: Index, from: Index, to: Index) {
    let pts: Vec<Index> = m.face_points(f).into_iter().map(|p| if p == from { to } else { p }).collect();
    m.set_face_points(f, &pts);
}

/// polySplitVertex: each face at each point gets its own copy of it. Returns the new points.
pub fn split_vertices(m: &mut Mesh, points: &[Index]) -> Vec<Index> {
    let mut out = Vec::new();
    for &p in points {
        if !m.point_valid(p) {
            continue;
        }
        for f in m.point_faces(p).into_iter().skip(1) {
            let q = new_copy(m, p);
            move_corner(m, f, p, q);
            out.push(q);
        }
    }
    m.remove_all_loose();
    out
}

/// polySplitEdge: the faces either side of the edges come apart wherever the edges separate them at a point. Returns
/// the new points.
pub fn detach_edges(m: &mut Mesh, edges: &[Index]) -> Vec<Index> {
    let cut: BTreeSet<Index> = edges.iter().copied().filter(|&e| m.edge_valid(e)).collect();
    let mut points = BTreeSet::new();
    for &e in &cut {
        points.insert(m.edge_point(e, 0));
        points.insert(m.edge_point(e, 1));
    }
    let mut plan: Vec<(Index, Vec<Vec<Index>>)> = Vec::new();
    for &p in &points {
        let faces = m.point_faces(p);
        let at: HashMap<Index, usize> = faces.iter().enumerate().map(|(i, &f)| (f, i)).collect();
        let mut uf = UnionFind::new(faces.len());
        m.for_each_point_edge(p, |e| {
            if cut.contains(&e) {
                return;
            }
            let mut first = None;
            m.for_each_edge_corner(e, |c| {
                let i = at[&m.corner_face(c)];
                match first {
                    None => first = Some(i),
                    Some(j) => {
                        uf.unite(i as Index, j as Index);
                    }
                }
            });
        });
        let mut groups: BTreeMap<Index, Vec<Index>> = BTreeMap::new();
        for (i, &f) in faces.iter().enumerate() {
            groups.entry(uf.find(i as Index)).or_default().push(f);
        }
        if groups.len() > 1 {
            plan.push((p, groups.into_values().collect()));
        }
    }
    let mut out = Vec::new();
    for (p, groups) in plan {
        for g in groups.into_iter().skip(1) {
            let q = new_copy(m, p);
            for f in g {
                move_corner(m, f, p, q);
            }
            out.push(q);
        }
    }
    m.remove_all_loose();
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Face subdivision

/// One level of polySubdivideFacet in quads: each edge of the faces gets its midpoint (shared with whatever is beyond
/// it), and each face a centre joined to those midpoints. Returns the new faces.
fn subdivide_quads_once(m: &mut Mesh, faces: &[Index]) -> Vec<Index> {
    let mut edges = BTreeSet::new();
    let faces: Vec<Index> = faces.iter().copied().filter(|&f| m.face_valid(f)).collect();
    let centers: HashMap<Index, DVec3> = faces.iter().map(|&f| (f, m.face_centroid(f))).collect();
    let corners_before: HashMap<Index, Vec<Index>> = faces.iter().map(|&f| (f, m.face_points(f))).collect();
    for &f in &faces {
        for c in m.face_corners(f) {
            edges.insert(m.corner_edge(c));
        }
    }
    for e in edges {
        m.split_edge(e, 0.5);
    }
    let mut out = Vec::new();
    for &f in &faces {
        // Every point that wasn't there before is a midpoint.
        let old: BTreeSet<Index> = corners_before[&f].iter().copied().collect();
        let mids: Vec<Index> = m.face_points(f).into_iter().filter(|p| !old.contains(p)).collect();
        if mids.len() < 2 {
            out.push(f);
            continue;
        }
        // A line across the face gives the centre; each piece then fans from it to its midpoints.
        let half = mids.len() / 2;
        let (a, b) = (m.find_corner(f, mids[0]), m.find_corner(f, mids[half]));
        let g = m.split_face(f, a, b);
        if g == INVALID {
            out.push(f);
            continue;
        }
        let center = m.split_edge(m.last_split_edge(), 0.5);
        m.set_position(center, centers[&f]);
        let mut pieces = vec![f, g];
        for &mid in mids.iter().filter(|&&q| q != mids[0] && q != mids[half]) {
            for k in 0..pieces.len() {
                let piece = pieces[k];
                let (cc, cm) = (m.find_corner(piece, center), m.find_corner(piece, mid));
                if cc == INVALID || cm == INVALID {
                    continue;
                }
                let n = m.split_face(piece, cc, cm);
                if n != INVALID {
                    pieces.push(n);
                }
                break;
            }
        }
        out.extend(pieces);
    }
    out
}

/// polySubdivideFacet: mode 0 quads, mode 1 triangles, divisions levels. Returns the new faces.
pub fn subdivide_facets(m: &mut Mesh, faces: &[Index], divisions: i32, triangles: bool) -> Vec<Index> {
    let mut cur = faces.to_vec();
    for _ in 0..divisions.max(0) {
        cur = if triangles { poke_faces(m, &cur) } else { subdivide_quads_once(m, &cur) };
    }
    cur
}

// ---------------------------------------------------------------------------------------------------------------------
// Merging two faces

/// polyMergeFacet: joins two faces of different shells with the same number of corners, removing both and welding
/// their points. mode: 0 second moves to first, 1 both to the average, 2 first moves to second, 6 nothing moves.
/// Returns the welded points.
pub fn merge_facets(m: &mut Mesh, first: Index, second: Index, mode: i64) -> Vec<Index> {
    if !m.face_valid(first) || !m.face_valid(second) || first == second {
        return Vec::new();
    }
    let a = m.face_points(first);
    let mut b = m.face_points(second);
    if a.len() != b.len() {
        return Vec::new();
    }
    // The faces look at each other, so they run opposite ways; the closest rotation pairs them.
    b.reverse();
    let n = a.len();
    let cost = |shift: usize| -> f64 { (0..n).map(|i| m.position(a[i]).distance_squared(m.position(b[(i + shift) % n]))).sum() };
    let shift = (0..n).min_by(|&x, &y| cost(x).total_cmp(&cost(y))).unwrap_or(0);
    let pairs: Vec<(Index, Index)> = (0..n).map(|i| (a[i], b[(i + shift) % n])).collect();
    if pairs.iter().any(|(x, y)| x == y) {
        return Vec::new();
    }
    m.remove_face(first);
    m.remove_face(second);
    let mut target = vec![INVALID; m.point_capacity() as usize];
    for &(x, y) in &pairs {
        let (px, py) = (m.position(x), m.position(y));
        let at = match mode.rem_euclid(3) {
            _ if mode >= 6 => px,
            0 => px,
            1 => (px + py) * 0.5,
            _ => py,
        };
        m.set_position(x, at);
        target[y as usize] = x;
    }
    crate::ops::basic::weld_points(m, &target);
    pairs.iter().map(|p| p.0).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Reordering

/// New point order for meshReorder: the start points, then outward through the edges breadth-first, each point's
/// neighbours in the order they come round it.
pub fn reorder_from(m: &Mesh, start: &[Index]) -> Vec<Index> {
    let mut seen = vec![false; m.point_capacity() as usize];
    let mut order = Vec::new();
    let mut todo = VecDeque::new();
    let mut seeds: Vec<Index> = start.iter().copied().filter(|&p| m.point_valid(p)).collect();
    seeds.extend((0..m.point_capacity()).filter(|&p| m.point_valid(p)));
    for s in seeds {
        if !seen[s as usize] {
            seen[s as usize] = true;
            todo.push_back(s);
            if start.contains(&s) {
                continue;
            }
        }
        while let Some(p) = todo.pop_front() {
            order.push(p);
            m.for_each_point_edge(p, |e| {
                let q = m.edge_other_point(e, p);
                if !seen[q as usize] {
                    seen[q as usize] = true;
                    todo.push_back(q);
                }
            });
        }
    }
    order
}

// ---------------------------------------------------------------------------------------------------------------------
// Remeshing

#[derive(Clone, Debug)]
pub struct RemeshOptions {
    /// Edges longer than this are split.
    pub max_edge: f64,
    /// Edges shorter than this fraction of max_edge are collapsed.
    pub collapse: f64,
    /// Tangential smoothing per pass, 0 to 1.
    pub smooth: f64,
    /// Border edges get split too.
    pub tessellate_borders: bool,
    pub passes: i32,
}

impl Default for RemeshOptions {
    fn default() -> Self {
        Self { max_edge: 1.0, collapse: 0.5, smooth: 0.5, tessellate_borders: true, passes: 4 }
    }
}

/// Whether e can collapse without pinching the surface: its ends share only the opposite corners of its faces.
fn collapse_is_safe(m: &Mesh, e: Index) -> bool {
    let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
    let neighbours = |p: Index| {
        let mut s = BTreeSet::new();
        m.for_each_point_edge(p, |x| {
            s.insert(m.edge_other_point(x, p));
        });
        s
    };
    let (na, nb) = (neighbours(a), neighbours(b));
    let common = na.intersection(&nb).count() as Index;
    common == m.edge_face_count(e) && na.len() + nb.len() > 4 + common as usize
}

/// Splits a triangle-pair edge and rejoins its faces as triangles.
fn split_into_triangles(m: &mut Mesh, e: Index, region: &mut BTreeSet<Index>) {
    let mut faces = Vec::new();
    m.for_each_edge_corner(e, |c| faces.push(m.corner_face(c)));
    let mid = m.split_edge(e, 0.5);
    for f in faces {
        let c = m.find_corner(f, mid);
        if c == INVALID || m.face_size(f) < 4 {
            continue;
        }
        let across = m.corner_next(m.corner_next(c));
        let g = m.split_face(f, c, across);
        if g != INVALID && region.contains(&f) {
            region.insert(g);
        }
    }
}

/// Isotropic remesh of the faces: they are triangulated, then long edges split, short ones collapsed, edges flipped
/// toward valence six and points relaxed across the surface and back onto it. Points on the region's border hold their
/// place. Returns the resulting faces.
pub fn remesh(m: &mut Mesh, faces: &[Index], o: &RemeshOptions) -> Vec<Index> {
    let reference = {
        let mut r = m.clone();
        r.compact(None);
        r
    };
    let bvh = MeshBvh::new(&reference);
    let tri = TriangulateOptions::default();
    let mut region: BTreeSet<Index> = triangulate_faces(m, faces, &tri).into_iter().collect();
    let max_edge = o.max_edge.max(1e-9);
    for _ in 0..o.passes.max(1) {
        // Which points may move: every face round them in the region and not on an open border.
        let held = |m: &Mesh, region: &BTreeSet<Index>, p: Index| m.is_boundary_point(p) || m.point_faces(p).iter().any(|f| !region.contains(f));

        // Split.
        let mut guard = 0;
        loop {
            let long: Vec<Index> = region_edges(m, &region)
                .into_iter()
                .filter(|&e| m.edge_length(e) > max_edge && (o.tessellate_borders || !m.is_boundary_edge(e)))
                .collect();
            guard += 1;
            if long.is_empty() || guard > 8 {
                break;
            }
            for e in long {
                if m.edge_valid(e) {
                    split_into_triangles(m, e, &mut region);
                }
            }
        }

        // Collapse.
        let short = o.collapse * max_edge;
        for e in region_edges(m, &region) {
            if !m.edge_valid(e) || m.edge_length(e) >= short || !collapse_is_safe(m, e) {
                continue;
            }
            let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
            let (ha, hb) = (held(m, &region, a), held(m, &region, b));
            if ha && hb {
                continue;
            }
            let keep = if ha { Some(m.position(a)) } else if hb { Some(m.position(b)) } else { None };
            let merged = collapse_edges(m, &[e]);
            if let (Some(k), Some(&p)) = (keep, merged.first()) {
                m.set_position(p, k);
            }
        }
        region.retain(|&f| m.face_valid(f));

        // Flip toward valence six.
        for e in region_edges(m, &region) {
            if !m.edge_valid(e) || m.edge_face_count(e) != 2 {
                continue;
            }
            let mut fs = Vec::new();
            m.for_each_edge_corner(e, |c| fs.push(m.corner_face(c)));
            if fs.iter().any(|f| !region.contains(f) || m.face_size(*f) != 3) {
                continue;
            }
            let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
            let opposite = |f: Index| m.face_points(f).into_iter().find(|&p| p != a && p != b).unwrap_or(INVALID);
            let (c, d) = (opposite(fs[0]), opposite(fs[1]));
            if c == INVALID || d == INVALID || c == d || m.find_edge(c, d) != INVALID {
                continue;
            }
            let target = |p: Index| if m.is_boundary_point(p) { 4 } else { 6 };
            let dev = |p: Index, v: i64| (v - target(p)).abs();
            let val = |p: Index| m.point_valence(p) as i64;
            let now = dev(a, val(a)) + dev(b, val(b)) + dev(c, val(c)) + dev(d, val(d));
            let then = dev(a, val(a) - 1) + dev(b, val(b) - 1) + dev(c, val(c) + 1) + dev(d, val(d) + 1);
            if then < now {
                crate::ops::basic::spin_edges(m, &[e]);
            }
        }
        region.retain(|&f| m.face_valid(f));

        // Relax across the surface, then back onto it.
        let points: BTreeSet<Index> = region.iter().flat_map(|&f| m.face_points(f)).collect();
        let moved: Vec<(Index, DVec3)> = points
            .iter()
            .filter(|&&p| !held(m, &region, p))
            .filter_map(|&p| {
                let mut sum = DVec3::ZERO;
                let mut n = 0.0;
                m.for_each_point_edge(p, |e| {
                    sum += m.position(m.edge_other_point(e, p));
                    n += 1.0;
                });
                if n == 0.0 {
                    return None;
                }
                let d = sum / n - m.position(p);
                let normal = point_normal(m, p);
                let at = m.position(p) + (d - normal * d.dot(normal)) * o.smooth;
                Some((p, bvh.closest_point(at).map_or(at, |c| c.point)))
            })
            .collect();
        for (p, at) in moved {
            m.set_position(p, at);
        }
    }
    region.into_iter().collect()
}

fn region_edges(m: &Mesh, region: &BTreeSet<Index>) -> Vec<Index> {
    let mut out = BTreeSet::new();
    for &f in region {
        if m.face_valid(f) {
            for c in m.face_corners(f) {
                out.insert(m.corner_edge(c));
            }
        }
    }
    out.into_iter().collect()
}

/// Automatic retopology: an isotropic remesh sized for about target_faces quads, its triangles then paired into quads
/// where they make good ones. Returns the faces.
pub fn retopo(m: &mut Mesh, faces: &[Index], target_faces: i64, quad_angle: f64) -> Vec<Index> {
    let area: f64 = faces.iter().filter(|&&f| m.face_valid(f)).map(|&f| m.face_area(f)).sum();
    let target = target_faces.max(1) as f64;
    // Two equilateral triangles make a quad of about the edge length squared times root three over two.
    let edge = (area / target / (3f64.sqrt() * 0.5)).sqrt() * 1.15;
    let o = RemeshOptions { max_edge: edge, collapse: 0.6, smooth: 0.6, tessellate_borders: true, passes: 5 };
    let tris = remesh(m, faces, &o);
    let q = QuadrangulateOptions { max_face_angle: quad_angle, keep_hard_edges: false, keep_seams: false, keep_materials: true };
    quadrangulate_faces(m, &tris, &q)
}
