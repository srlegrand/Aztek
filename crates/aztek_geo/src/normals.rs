//! Face, point, split, weighted normals and hard edges. Tangents come with
//! triangulation.

use glam::{DVec3, Vec3};
use rayon::prelude::*;

use crate::attribute::{names, Index};
use crate::mesh::Mesh;

const GRAIN: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NormalWeight {
    Uniform,
    Area,
    Angle,
    AreaAngle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CornerNormalOptions {
    pub weight: NormalWeight,
    /// Split along edges whose "hard" attribute is set. Boundary and non-manifold edges always split.
    pub use_hard_edges: bool,
}

impl Default for CornerNormalOptions {
    fn default() -> Self {
        Self {
            weight: NormalWeight::AreaAngle,
            use_hard_edges: true,
        }
    }
}

fn face_area_vectors(mesh: &Mesh) -> Vec<DVec3> {
    (0..mesh.face_capacity())
        .into_par_iter()
        .with_min_len(GRAIN)
        .map(|f| {
            if mesh.face_valid(f) {
                mesh.face_area_vector(f)
            } else {
                DVec3::ZERO
            }
        })
        .collect()
}

/// A corner's share of its fan normal; `angle` is the corner's angle, called only when the weight needs it.
fn contribution(area_vec: DVec3, w: NormalWeight, angle: impl Fn() -> f64) -> DVec3 {
    match w {
        NormalWeight::Uniform => area_vec.normalize(),
        NormalWeight::Area => area_vec,
        NormalWeight::Angle => area_vec.normalize() * angle(),
        NormalWeight::AreaAngle => area_vec * angle(),
    }
}

fn corner_contribution(mesh: &Mesh, c: Index, area_vec: DVec3, w: NormalWeight) -> DVec3 {
    contribution(area_vec, w, || corner_angle(mesh, c))
}

fn angle_at(p: DVec3, prev: DVec3, next: DVec3) -> f64 {
    let (a, b) = (prev - p, next - p);
    if a.length() <= 0.0 || b.length() <= 0.0 {
        return 0.0;
    }
    // atan2 stays accurate for angles near 0 and pi.
    a.cross(b).length().atan2(a.dot(b))
}

fn normalize_or(v: DVec3, fallback: DVec3) -> DVec3 {
    let len = v.length();
    if len > 1e-300 {
        v / len
    } else {
        fallback
    }
}

/// The corners around one point and the smooth fans they fall into.
#[derive(Default)]
struct FanScratch {
    corners: Vec<Index>,
    parent: Vec<Index>,
}

impl FanScratch {
    /// The fan of corners[i], as the index of one corner in it.
    fn find(&mut self, mut i: Index) -> Index {
        while self.parent[i as usize] != i {
            let gp = self.parent[self.parent[i as usize] as usize];
            self.parent[i as usize] = gp;
            i = gp;
        }
        i
    }
}

/// Fills s with p's corners grouped into fans: corners on either side of a soft manifold edge share one.
fn group_fans(mesh: &Mesh, p: Index, hard: &[bool], s: &mut FanScratch) {
    s.corners.clear();
    mesh.for_each_point_corner(p, |c| s.corners.push(c));
    let k = s.corners.len();
    s.parent.clear();
    s.parent.extend(0..k as Index);
    if k < 2 {
        return;
    }
    let mut joins = Vec::new();
    mesh.for_each_point_edge(p, |e| {
        if (!hard.is_empty() && hard[e as usize]) || !mesh.is_manifold_edge(e) {
            return;
        }
        let r1 = mesh.edge_corner(e);
        let r2 = mesh.corner_radial(r1);
        let at_p = |r: Index| {
            if mesh.corner_point(r) == p {
                r
            } else {
                mesh.corner_next(r)
            }
        };
        joins.push((at_p(r1), at_p(r2)));
    });
    for (ca, cb) in joins {
        let local = |c: Index| s.corners.iter().position(|&x| x == c).map(|i| i as Index);
        if let (Some(a), Some(b)) = (local(ca), local(cb)) {
            let ra = s.find(a);
            let rb = s.find(b);
            s.parent[ra as usize] = rb;
        }
    }
}

fn hard_edges<'a>(mesh: &'a Mesh, opts: &CornerNormalOptions) -> &'a [bool] {
    if opts.use_hard_edges {
        mesh.edge_attrs().get::<bool>(names::HARD)
    } else {
        &[]
    }
}

/// Angle at corner c between its two edges, radians.
pub fn corner_angle(mesh: &Mesh, c: Index) -> f64 {
    angle_at(
        mesh.position(mesh.corner_point(c)),
        mesh.position(mesh.corner_point(mesh.corner_prev(c))),
        mesh.position(mesh.corner_next_point(c)),
    )
}

/// Unit face normals per face capacity. Parallel.
pub fn face_normals(mesh: &Mesh) -> Vec<DVec3> {
    face_area_vectors(mesh)
        .into_par_iter()
        .with_min_len(GRAIN)
        .map(|v| normalize_or(v, DVec3::Y))
        .collect()
}

/// Smooth point normals per point capacity, ignoring hard edges.
pub fn point_normals(mesh: &Mesh, weight: NormalWeight) -> Vec<Vec3> {
    let area = face_area_vectors(mesh);
    (0..mesh.point_capacity())
        .into_par_iter()
        .with_min_len(GRAIN)
        .map(|p| {
            if !mesh.point_valid(p) {
                return Vec3::ZERO;
            }
            let mut sum = DVec3::ZERO;
            mesh.for_each_point_corner(p, |c| {
                sum += corner_contribution(mesh, c, area[mesh.corner_face(c) as usize], weight)
            });
            normalize_or(sum, DVec3::Y).as_vec3()
        })
        .collect()
}

/// Runs `f` for every valid point in parallel; each returns (corner, value) pairs, scattered into a corner-capacity
/// vector. Every corner belongs to one point, so no two points write the same slot.
fn per_point_corners<T: Copy + Send + Default>(
    mesh: &Mesh,
    f: impl Fn(Index, &mut FanScratch, &mut Vec<(Index, T)>) + Sync,
) -> Vec<T> {
    let parts: Vec<Vec<(Index, T)>> = (0..mesh.point_capacity())
        .into_par_iter()
        .with_min_len(GRAIN)
        .fold(
            || (FanScratch::default(), Vec::new()),
            |(mut s, mut out), p| {
                if mesh.point_valid(p) {
                    f(p, &mut s, &mut out);
                }
                (s, out)
            },
        )
        .map(|(_, out)| out)
        .collect();
    let mut result = vec![T::default(); mesh.corner_capacity() as usize];
    for part in parts {
        for (c, v) in part {
            result[c as usize] = v;
        }
    }
    result
}

/// Split normals per corner capacity: one smooth fan per run of soft edges around each point. Parallel.
pub fn corner_normals(mesh: &Mesh, opts: &CornerNormalOptions) -> Vec<Vec3> {
    let area = face_area_vectors(mesh);
    let hard = hard_edges(mesh, opts);
    per_point_corners(mesh, |p, s, out| {
        group_fans(mesh, p, hard, s);
        let k = s.corners.len();
        let mut sum = vec![DVec3::ZERO; k];
        for i in 0..k {
            let c = s.corners[i];
            let root = s.find(i as Index) as usize;
            sum[root] +=
                corner_contribution(mesh, c, area[mesh.corner_face(c) as usize], opts.weight);
        }
        for i in 0..k {
            let c = s.corners[i];
            let fallback = normalize_or(area[mesh.corner_face(c) as usize], DVec3::Y);
            let root = s.find(i as Index) as usize;
            out.push((c, normalize_or(sum[root], fallback).as_vec3()));
        }
    })
}

/// Writes `corner_normals` into the corner "normal" attribute.
pub fn update_corner_normals(mesh: &mut Mesh, opts: &CornerNormalOptions) {
    let n = corner_normals(mesh, opts);
    let dst = mesh
        .corner_attrs_mut()
        .get_or_add::<[f32; 3]>(names::NORMAL);
    for (d, v) in dst.iter_mut().zip(n) {
        *d = v.to_array();
    }
}

/// `corner_normals`' topology work done once and reused while only points move: which smooth fan each corner is in,
/// in face-vertex order (valid faces in id order, each from face_corner). `compute` is then a few linear passes.
pub struct CornerNormalPlan {
    opts: CornerNormalOptions,
    topology: u64,
    hard: Vec<bool>,
    face_offsets: Vec<u32>,
    row_point: Vec<u32>,
    /// Index into face_offsets.
    row_face: Vec<u32>,
    row_fan: Vec<u32>,
    fan_offsets: Vec<u32>,
    fan_rows: Vec<u32>,
    area: Vec<DVec3>,
    row_angle: Vec<f64>,
    fan_normal: Vec<Vec3>,
}

fn scan(v: &mut [u32]) -> u32 {
    let mut sum = 0;
    for x in v.iter_mut() {
        let t = *x;
        *x = sum;
        sum += t;
    }
    sum
}

impl CornerNormalPlan {
    pub fn new(mesh: &Mesh, opts: &CornerNormalOptions) -> Self {
        let hard = hard_edges(mesh, opts).to_vec();
        let faces: Vec<Index> = (0..mesh.face_capacity())
            .filter(|&f| mesh.face_valid(f))
            .collect();
        let mut face_offsets: Vec<u32> = faces.iter().map(|&f| mesh.face_size(f)).collect();
        face_offsets.push(0);
        let rows = scan(&mut face_offsets) as usize;

        let mut row_point = vec![0u32; rows];
        let mut row_face = vec![0u32; rows];
        let mut corner_row = vec![0u32; mesh.corner_capacity() as usize];
        for (i, &f) in faces.iter().enumerate() {
            let mut row = face_offsets[i];
            mesh.for_each_face_corner(f, |c| {
                row_point[row as usize] = mesh.corner_point(c);
                row_face[row as usize] = i as u32;
                corner_row[c as usize] = row;
                row += 1;
            });
        }

        // Fans are numbered point by point, so each row first gets its fan's ordinal among the fans of its point.
        let per_point: Vec<(u32, Vec<(u32, u32)>)> = (0..mesh.point_capacity())
            .into_par_iter()
            .with_min_len(GRAIN)
            .map_init(FanScratch::default, |s, p| {
                if !mesh.point_valid(p) {
                    return (0, Vec::new());
                }
                group_fans(mesh, p, &hard, s);
                let k = s.corners.len();
                let mut ordinal = vec![0u32; k];
                let mut fans = 0;
                for i in 0..k {
                    if s.find(i as Index) == i as Index {
                        ordinal[i] = fans;
                        fans += 1;
                    }
                }
                let assign = (0..k)
                    .map(|i| {
                        let root = s.find(i as Index) as usize;
                        (corner_row[s.corners[i] as usize], ordinal[root])
                    })
                    .collect();
                (fans, assign)
            })
            .collect();
        let mut row_fan = vec![0u32; rows];
        let mut fan_base: Vec<u32> = per_point.iter().map(|(n, _)| *n).collect();
        fan_base.push(0);
        let fans = scan(&mut fan_base) as usize;
        for (_, assign) in &per_point {
            for &(row, ord) in assign {
                row_fan[row as usize] = ord;
            }
        }
        let mut fan_offsets = vec![0u32; fans + 1];
        for row in 0..rows {
            row_fan[row] += fan_base[row_point[row] as usize];
            fan_offsets[row_fan[row] as usize] += 1;
        }
        scan(&mut fan_offsets);
        let mut fan_rows = vec![0u32; rows];
        let mut cursor = fan_offsets[..fans].to_vec();
        for row in 0..rows {
            let f = row_fan[row] as usize;
            fan_rows[cursor[f] as usize] = row as u32;
            cursor[f] += 1;
        }
        // Filled in row order, so each fan's rows are already ascending: a fan sums its corners in the same order
        // every time.
        Self {
            opts: *opts,
            topology: mesh.topology_id(),
            hard,
            face_offsets,
            row_point,
            row_face,
            row_fan,
            fan_offsets,
            fan_rows,
            area: Vec::new(),
            row_angle: Vec::new(),
            fan_normal: Vec::new(),
        }
    }

    /// True while mesh has the topology and hard edges this plan was made from.
    pub fn fits(&self, mesh: &Mesh) -> bool {
        mesh.topology_id() == self.topology && hard_edges(mesh, &self.opts) == self.hard.as_slice()
    }
    /// Face-vertex rows, num_corners().
    pub fn rows(&self) -> usize {
        self.row_point.len()
    }
    /// Where each valid face's rows start, plus the total.
    pub fn face_offsets(&self) -> &[u32] {
        &self.face_offsets
    }

    /// `corner_normals(mesh)` in row order into out (rows() long). Parallel.
    pub fn compute(&mut self, mesh: &Mesh, out: &mut [Vec3]) {
        let pos = mesh.positions();
        let angles = matches!(
            self.opts.weight,
            NormalWeight::Angle | NormalWeight::AreaAngle
        );
        let faces = self.face_offsets.len() - 1;
        let (offsets, rp) = (&self.face_offsets, &self.row_point);
        // Face by face, so the positions each face needs are loaded once for its area and its corner angles.
        let per_face: Vec<(DVec3, Vec<f64>)> = (0..faces)
            .into_par_iter()
            .with_min_len(GRAIN)
            .map(|f| {
                let (first, end) = (offsets[f], offsets[f + 1]);
                let origin = pos[rp[first as usize] as usize];
                let mut n = DVec3::ZERO;
                let mut ang = Vec::new();
                for row in first..end {
                    let p = pos[rp[row as usize] as usize];
                    let next =
                        pos[rp[if row + 1 == end { first } else { row + 1 } as usize] as usize];
                    n += (p - origin).cross(next - origin);
                    if angles {
                        ang.push(angle_at(
                            p,
                            pos[rp[if row == first { end - 1 } else { row - 1 } as usize] as usize],
                            next,
                        ));
                    }
                }
                (n, ang)
            })
            .collect();
        self.area = per_face.iter().map(|(n, _)| *n).collect();
        self.row_angle = if angles {
            per_face.into_iter().flat_map(|(_, a)| a).collect()
        } else {
            Vec::new()
        };

        let fans = self.fan_offsets.len() - 1;
        let (fo, fr, rf, area, ra, w) = (
            &self.fan_offsets,
            &self.fan_rows,
            &self.row_face,
            &self.area,
            &self.row_angle,
            self.opts.weight,
        );
        self.fan_normal = (0..fans)
            .into_par_iter()
            .with_min_len(GRAIN)
            .map(|fan| {
                let mut sum = DVec3::ZERO;
                for j in fo[fan]..fo[fan + 1] {
                    let row = fr[j as usize] as usize;
                    sum += contribution(area[rf[row] as usize], w, || ra[row]);
                }
                // Zero marks a degenerate fan, whose corners take their own face's normal.
                let len = sum.length();
                if len > 1e-300 {
                    (sum / len).as_vec3()
                } else {
                    Vec3::ZERO
                }
            })
            .collect();
        let (fan_n, row_fan) = (&self.fan_normal, &self.row_fan);
        out[..self.rows()]
            .par_iter_mut()
            .with_min_len(GRAIN)
            .enumerate()
            .for_each(|(row, o)| {
                let n = fan_n[row_fan[row] as usize];
                *o = if n != Vec3::ZERO {
                    n
                } else {
                    normalize_or(area[rf[row] as usize], DVec3::Y).as_vec3()
                };
            });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeightedNormalMode {
    FaceArea,
    CornerAngle,
    AreaAngle,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeightedNormalOptions {
    pub mode: WeightedNormalMode,
    /// Within each smooth fan, faces weighing less than this fraction of the fan's heaviest face are left out, so large
    /// flat faces keep their own normal across a chamfer or bevel beside them. 0 keeps every face.
    pub dominance: f64,
    /// Split along "hard" edges; off smooths across them too.
    pub keep_sharp: bool,
    /// Only the faces with the highest face int "faceStrength" (0 weak, 1 medium, 2 strong) in a fan count, when present.
    pub use_face_strength: bool,
}

impl Default for WeightedNormalOptions {
    fn default() -> Self {
        Self {
            mode: WeightedNormalMode::FaceArea,
            dominance: 0.5,
            keep_sharp: true,
            use_face_strength: true,
        }
    }
}

/// Face-weighted split normals per corner capacity (3ds Max/Blender Weighted Normal): each soft fan takes the normal
/// of its dominant faces, so a low poly with chamfered edges shades its big faces flat. Parallel.
pub fn weighted_normals(mesh: &Mesh, opts: &WeightedNormalOptions) -> Vec<Vec3> {
    let area = face_area_vectors(mesh);
    let hard: &[bool] = if opts.keep_sharp {
        mesh.edge_attrs().get::<bool>(names::HARD)
    } else {
        &[]
    };
    let strength: &[i32] = if opts.use_face_strength {
        mesh.face_attrs().get::<i32>("faceStrength")
    } else {
        &[]
    };
    let dominance = opts.dominance.clamp(0.0, 1.0);
    let weight_of = |c: Index| {
        let a = area[mesh.corner_face(c) as usize].length();
        match opts.mode {
            WeightedNormalMode::FaceArea => a,
            WeightedNormalMode::CornerAngle => corner_angle(mesh, c),
            WeightedNormalMode::AreaAngle => a * corner_angle(mesh, c),
        }
    };
    let strength_of = |c: Index| {
        if strength.is_empty() {
            0
        } else {
            strength[mesh.corner_face(c) as usize]
        }
    };
    per_point_corners(mesh, |p, s, out| {
        group_fans(mesh, p, hard, s);
        let k = s.corners.len();
        if k == 0 {
            return;
        }
        let roots: Vec<usize> = (0..k).map(|i| s.find(i as Index) as usize).collect();
        let mut max_strength = vec![i32::MIN; k];
        for i in 0..k {
            max_strength[roots[i]] = max_strength[roots[i]].max(strength_of(s.corners[i]));
        }
        let mut weight = vec![0.0; k];
        let mut max_weight = vec![0.0f64; k];
        for i in 0..k {
            weight[i] = weight_of(s.corners[i]);
            if strength_of(s.corners[i]) == max_strength[roots[i]] {
                max_weight[roots[i]] = max_weight[roots[i]].max(weight[i]);
            }
        }
        let mut sum = vec![DVec3::ZERO; k];
        for i in 0..k {
            let c = s.corners[i];
            let r = roots[i];
            if strength_of(c) != max_strength[r] || weight[i] < dominance * max_weight[r] {
                continue;
            }
            sum[r] += normalize_or(area[mesh.corner_face(c) as usize], DVec3::ZERO) * weight[i];
        }
        for i in 0..k {
            let c = s.corners[i];
            let fallback = normalize_or(area[mesh.corner_face(c) as usize], DVec3::Y);
            out.push((c, normalize_or(sum[roots[i]], fallback).as_vec3()));
        }
    })
}

/// Writes `weighted_normals` into the corner "normal" attribute, for the given faces' corners only when faces is given.
pub fn apply_weighted_normals(mesh: &mut Mesh, opts: &WeightedNormalOptions, faces: &[Index]) {
    let had = mesh.corner_attrs().has(names::NORMAL);
    let before = if had || faces.is_empty() {
        Vec::new()
    } else {
        corner_normals(mesh, &CornerNormalOptions::default())
    };
    let n = weighted_normals(mesh, opts);
    let mut targets = Vec::new();
    for &f in faces {
        if mesh.face_valid(f) {
            mesh.for_each_face_corner(f, |c| targets.push(c));
        }
    }
    let dst = mesh
        .corner_attrs_mut()
        .get_or_add::<[f32; 3]>(names::NORMAL);
    if faces.is_empty() {
        for (d, v) in dst.iter_mut().zip(&n) {
            *d = v.to_array();
        }
        return;
    }
    // Corners outside the faces keep what they shaded with: their stored normal, or the computed one when none was.
    if !had {
        for (d, v) in dst.iter_mut().zip(&before) {
            *d = v.to_array();
        }
    }
    for c in targets {
        dst[c as usize] = n[c as usize].to_array();
    }
}

/// Marks edges hard where neighbouring faces meet at more than the angle (the reference's Set Normal Angle).
pub fn set_hard_edges_by_angle(mesh: &mut Mesh, degrees: f64, edges: &[Index]) {
    let cos_limit = degrees.to_radians().cos();
    let m = &*mesh;
    let judge = |e: Index| -> Option<bool> {
        if !m.edge_valid(e) {
            return None;
        }
        if !m.is_manifold_edge(e) {
            return Some(false);
        }
        let c = m.edge_corner(e);
        let a = m.face_normal(m.corner_face(c));
        let b = m.face_normal(m.corner_face(m.corner_radial(c)));
        Some(a.dot(b) < cos_limit - 1e-12)
    };
    let results: Vec<(Index, bool)> = if edges.is_empty() {
        (0..m.edge_capacity())
            .into_par_iter()
            .with_min_len(GRAIN)
            .filter_map(|e| judge(e).map(|h| (e, h)))
            .collect()
    } else {
        edges
            .iter()
            .filter_map(|&e| judge(e).map(|h| (e, h)))
            .collect()
    };
    let hard = mesh.edge_attrs_mut().get_or_add::<bool>(names::HARD);
    for (e, h) in results {
        hard[e as usize] = h;
    }
}

/// Clears "hard" on the given edges, or all.
pub fn set_soft_edges(mesh: &mut Mesh, edges: &[Index]) {
    let hard = mesh.edge_attrs_mut().get_or_add::<bool>(names::HARD);
    if edges.is_empty() {
        hard.fill(false);
    }
    for &e in edges {
        if (e as usize) < hard.len() {
            hard[e as usize] = false;
        }
    }
}

// Tangents

use crate::triangulate::{triangulate, TriangulateOptions, Triangulation};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TangentOptions<'a> {
    pub uv_set: &'a str,
    /// Use the corner "normal" attribute if present instead of recomputing.
    pub use_stored_normals: bool,
}

impl Default for TangentOptions<'_> {
    fn default() -> Self {
        Self {
            uv_set: "uv0",
            use_stored_normals: true,
        }
    }
}

struct MikkData<'a> {
    mesh: &'a Mesh,
    tris: &'a Triangulation,
    normals: &'a [Vec3],
    uv: &'a [[f32; 2]],
    /// Per triangle: write its tangents; empty writes all.
    own: &'a [bool],
    out: Vec<(Index, [f32; 4])>,
}

impl MikkData<'_> {
    fn corner(&self, face: usize, vert: usize) -> Index {
        self.tris.corners[face * 3 + vert]
    }
}

impl mikktspace::Geometry for MikkData<'_> {
    fn num_faces(&self) -> usize {
        self.tris.triangle_count() as usize
    }
    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }
    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.mesh
            .position(self.mesh.corner_point(self.corner(face, vert)))
            .as_vec3()
            .to_array()
    }
    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.corner(face, vert) as usize].to_array()
    }
    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.uv[self.corner(face, vert) as usize]
    }
    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        if !self.own.is_empty() && !self.own[face] {
            return;
        }
        let c = self.corner(face, vert);
        self.out.push((c, tangent));
    }
}

/// Per point, one representative of every point at the same float position. MikkTSpace welds by value, not by id.
fn welded_points(mesh: &Mesh) -> Vec<Index> {
    let key = |p: Index| {
        let f = mesh.position(p).as_vec3() + Vec3::ZERO; // -0 and 0 weld
        [f.x.to_bits(), f.y.to_bits(), f.z.to_bits()]
    };
    let mut order: Vec<Index> = (0..mesh.point_capacity())
        .filter(|&p| mesh.point_valid(p))
        .collect();
    order.par_sort_by_key(|&p| key(p));
    let mut weld = vec![Index::MAX; mesh.point_capacity() as usize];
    for i in 0..order.len() {
        weld[order[i] as usize] = if i > 0 && key(order[i]) == key(order[i - 1]) {
            weld[order[i - 1] as usize]
        } else {
            order[i]
        };
    }
    weld
}

/// Chunk boundaries in triangles, about size apart and never inside a face, so each corner is written by one chunk.
fn chunk_bounds(tris: &Triangulation, size: Index) -> Vec<Index> {
    let n = tris.triangle_count();
    let mut bounds = vec![0];
    let mut t = size;
    while t < n {
        while t < n && tris.faces[t as usize] == tris.faces[t as usize - 1] {
            t += 1;
        }
        if t < n {
            bounds.push(t);
        }
        t += size;
    }
    bounds.push(n);
    bounds
}

/// MikkTSpace tangents per corner capacity (xyz tangent, w bitangent sign), the same as Unreal's, computed on the given
/// triangulation (the one used for export). Empty without the uv set. Runs in parallel chunks, each with the
/// triangles around it; the result is the same as one call.
pub fn corner_tangents(
    mesh: &Mesh,
    tris: &Triangulation,
    normals: &[Vec3],
    uv_set: &str,
    parallel: bool,
) -> Vec<[f32; 4]> {
    let uv = mesh.corner_attrs().get::<[f32; 2]>(uv_set);
    if uv.is_empty() || normals.len() < mesh.corner_capacity() as usize {
        return Vec::new();
    }
    let mut out = vec![[1.0f32, 0.0, 0.0, 1.0]; mesh.corner_capacity() as usize];
    let run = |part: &Triangulation, own: &[bool]| {
        let mut d = MikkData {
            mesh,
            tris: part,
            normals,
            uv,
            own,
            out: Vec::new(),
        };
        mikktspace::generate_tangents(&mut d);
        d.out
    };
    let n = tris.triangle_count();
    let chunk = (1u32 << 15).max(n / (4 * rayon::current_num_threads() as Index).max(1));
    let bounds = if parallel {
        chunk_bounds(tris, chunk)
    } else {
        Vec::new()
    };
    if bounds.len() <= 2 {
        for (c, t) in run(tris, &[]) {
            out[c as usize] = t;
        }
        return out;
    }
    // A corner's tangent depends only on the triangles around its welded point. Each chunk runs with every triangle
    // touching it, in the original order, and writes only its own corners: the same result as one call.
    let weld = welded_points(mesh);
    let mut offsets = vec![0u32; mesh.point_capacity() as usize + 1];
    for &c in &tris.corners {
        offsets[weld[mesh.corner_point(c) as usize] as usize + 1] += 1;
    }
    for i in 1..offsets.len() {
        offsets[i] += offsets[i - 1];
    }
    let mut around = vec![0 as Index; *offsets.last().unwrap() as usize];
    let mut fill = offsets[..offsets.len() - 1].to_vec();
    for (i, &c) in tris.corners.iter().enumerate() {
        let w = weld[mesh.corner_point(c) as usize] as usize;
        around[fill[w] as usize] = (i / 3) as Index;
        fill[w] += 1;
    }
    let parts: Vec<Vec<(Index, [f32; 4])>> = (0..bounds.len() - 1)
        .into_par_iter()
        .map(|ci| {
            let (lo, hi) = (bounds[ci], bounds[ci + 1]);
            let mut halo = Vec::new();
            for i in lo as usize * 3..hi as usize * 3 {
                let w = weld[mesh.corner_point(tris.corners[i]) as usize] as usize;
                for k in offsets[w]..offsets[w + 1] {
                    let t = around[k as usize];
                    if t < lo || t >= hi {
                        halo.push(t);
                    }
                }
            }
            halo.sort_unstable();
            halo.dedup();
            let mut part = Triangulation::default();
            let mut own = Vec::new();
            let mut add = |t: Index, mine: bool| {
                part.corners
                    .extend_from_slice(&tris.corners[t as usize * 3..t as usize * 3 + 3]);
                part.faces.push(tris.faces[t as usize]);
                own.push(mine);
            };
            let split = halo.partition_point(|&t| t < lo);
            for &t in &halo[..split] {
                add(t, false);
            }
            for t in lo..hi {
                add(t, true);
            }
            for &t in &halo[split..] {
                add(t, false);
            }
            run(&part, &own)
        })
        .collect();
    for part in parts {
        for (c, t) in part {
            out[c as usize] = t;
        }
    }
    out
}

/// Triangulates, then writes the corner "tangent" attribute. False without the uv set.
pub fn update_tangents(mesh: &mut Mesh, opts: &TangentOptions) -> bool {
    if !mesh.corner_attrs().has(opts.uv_set) {
        return false;
    }
    let stored = mesh.corner_attrs().get::<[f32; 3]>(names::NORMAL);
    let normals: Vec<Vec3> =
        if !opts.use_stored_normals || stored.len() != mesh.corner_capacity() as usize {
            corner_normals(mesh, &CornerNormalOptions::default())
        } else {
            stored.iter().map(|&n| Vec3::from_array(n)).collect()
        };
    let tris = triangulate(mesh, &TriangulateOptions::default());
    let t = corner_tangents(mesh, &tris, &normals, opts.uv_set, true);
    if t.is_empty() {
        return false;
    }
    mesh.corner_attrs_mut()
        .get_or_add::<[f32; 4]>(names::TANGENT)
        .copy_from_slice(&t);
    true
}
