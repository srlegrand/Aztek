//! Mesh booleans on Manifold (the `manifold-rust` crate),
//! with the seam cleanup a modeller would otherwise do by hand.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use aztek_geo::bvh::MeshBvh;
use aztek_geo::normals::set_hard_edges_by_angle;
use aztek_geo::ops::{bevel_edges, delete_faces, dissolve_edges, split_ngons, weld_points, BevelOptions, NgonSplitOptions};
use aztek_geo::triangulate::{triangulate, TriangulateOptions};
use aztek_geo::{colors, names, Index, Mesh, INVALID};
use glam::{DMat4, DVec3};
use manifold_rust::manifold::Manifold;
use manifold_rust::types::{Error as ManifoldError, MeshGL, OpType};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BooleanOp {
    Union,
    Difference,
    Intersection,
}

/// "union", "difference", "intersection" (or "intersect").
pub fn boolean_op_from_name(name: &str) -> Option<BooleanOp> {
    match name {
        "union" => Some(BooleanOp::Union),
        "difference" => Some(BooleanOp::Difference),
        "intersection" | "intersect" => Some(BooleanOp::Intersection),
        _ => None,
    }
}

/// Edge bool on the result: the intersection curve, where faces from different operands meet.
pub const BOOLEAN_SEAM_ATTR: &str = "booleanSeam";

#[derive(Clone, Copy)]
pub struct BooleanOperand<'a> {
    pub mesh: &'a Mesh,
    pub to_world: DMat4,
}

#[derive(Clone, Debug)]
pub struct BooleanOptions {
    pub op: BooleanOp,
    /// Weld seam points onto close neighbours and drop straight-through seam points.
    pub cleanup: bool,
    /// Weld distance for cleanup, cm; negative is a tenth of the median seam edge.
    pub snap_distance: f64,
    /// Cut the n-gons along the seam into quads and triangles that carry on the edge loops meeting them.
    pub split_ngons: bool,
    /// Remesh a band of faces around the seam into quads that follow it, keeping the band's border points so it welds
    /// back. Where the border can't be kept (its edges far from the band's spacing) the n-gon split runs instead.
    pub quad_band: bool,
    /// Rings of faces beyond the ones touching the seam that the band takes in.
    pub band_rings: i32,
    /// Seconds the band remesh may take before the n-gon split runs instead; 0 is unlimited.
    pub band_timeout: f64,
    /// Width of a rounded strip along the seam, cm; 0 leaves it sharp.
    pub fillet_width: f64,
    pub fillet_segments: i32,
    /// New edges sharper than this many degrees are marked hard.
    pub hard_angle: f64,
}

impl Default for BooleanOptions {
    fn default() -> Self {
        Self {
            op: BooleanOp::Difference,
            cleanup: true,
            snap_distance: -1.0,
            split_ngons: true,
            quad_band: false,
            band_rings: 1,
            band_timeout: 5.0,
            fillet_width: 0.0,
            fillet_segments: 2,
            hard_angle: 30.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BooleanResult {
    pub mesh: Mesh,
    /// Seam edges in the result, before any fillet replaced them.
    pub seam_edges: Index,
    pub error: String,
    /// The operand the error is about.
    pub bad_operand: Option<usize>,
    /// The seam band was remeshed into quads (BooleanOptions::quad_band).
    pub quad_band: bool,
}

impl BooleanResult {
    pub fn ok(&self) -> bool {
        self.error.is_empty()
    }
}

/// What the quad band asks of the quad remesher (the subset of retopo's QuadRemeshOptions it sets).
#[derive(Clone, Debug, Default)]
pub struct QuadRemeshRequest {
    pub seamless: bool,
    pub target_edge_length: f64,
    pub adaptivity: f64,
    pub pure_quads: bool,
    pub sharp_angle: f64,
    pub pinned_points: Vec<DVec3>,
    pub timeout: f64,
}

/// Remeshes a patch (with a "density" point attribute) into quads, or None when it fails.
pub type QuadRemesher = Arc<dyn Fn(&Mesh, &QuadRemeshRequest) -> Option<Mesh> + Send + Sync>;

fn remesher() -> &'static RwLock<Option<QuadRemesher>> {
    static R: OnceLock<RwLock<Option<QuadRemesher>>> = OnceLock::new();
    R.get_or_init(Default::default)
}

/// The retopo port registers its quad remesher here; until then the quad band falls back to the n-gon split.
pub fn set_quad_remesher(r: QuadRemesher) {
    *remesher().write().unwrap() = Some(r);
}

const PROPS: usize = 6; // x y z u v point, then rgba per color set
// Scratch attributes while the result is being cleaned up.
const ORIGIN_ATTR: &str = "booleanOrigin"; // point int: the operand point it is, plus one; 0 is new
const OPERAND_ATTR: &str = "booleanOperand"; // face int

/// World-space MeshGL with one vertex per corner (so uvs and colours survive), welded by position for manifoldness.
/// face_id is the polygon's id plus face_offset, so triangles of one polygon merge back. The point id rides along as a
/// property: Manifold copies it exactly onto points that survive and interpolates it on new ones.
fn to_manifold(mesh: &Mesh, world: &DMat4, face_offset: u32, color_sets: &[String]) -> MeshGL {
    let tris = triangulate(mesh, &TriangulateOptions::default());
    let uv = mesh.corner_attrs().get::<[f32; 2]>(&names::uv(0));
    let cols: Vec<&[[f32; 4]]> = color_sets.iter().map(|s| colors::values(mesh, s)).collect();
    let mut gl = MeshGL { num_prop: (PROPS + 4 * color_sets.len()) as u32, ..Default::default() };
    let num_prop = gl.num_prop as usize;
    let mut vert_of_corner: HashMap<Index, u32> = HashMap::new();
    for t in 0..tris.faces.len() {
        for k in 0..3 {
            let c = tris.corners[t * 3 + k];
            let next = (gl.vert_properties.len() / num_prop) as u32;
            let v = *vert_of_corner.entry(c).or_insert_with(|| {
                let point = mesh.corner_point(c);
                let p = world.transform_point3(mesh.position(point));
                let t2 = uv.get(c as usize).copied().unwrap_or([0.0; 2]);
                gl.vert_properties.extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32, t2[0], t2[1], point as f32]);
                for col in &cols {
                    let v = col.get(c as usize).copied().unwrap_or([1.0; 4]);
                    gl.vert_properties.extend_from_slice(&v);
                }
                next
            });
            gl.tri_verts.push(v);
        }
        gl.face_id.push(face_offset + tris.faces[t]);
    }
    gl.merge();
    gl
}

/// Rebuilds a polygon mesh in to_local space: triangles of the same input polygon merge back, faces carry their
/// operand and material, points that are operand points say which, and edges that were operand edges keep their flags.
fn from_manifold(gl: &MeshGL, to_local: &DMat4, operands: &[BooleanOperand], first_face: &[u32]) -> Mesh {
    let base = operands[0].mesh;
    let num_prop = gl.num_prop as usize;
    let operand_of_face = |id: u32| first_face.partition_point(|&f| f <= id) - 1;

    let verts = gl.num_vert();
    let mut canon: Vec<u32> = (0..verts as u32).collect();
    for (i, &from) in gl.merge_from_vert.iter().enumerate() {
        canon[from as usize] = gl.merge_to_vert[i];
    }
    let mut point_of = vec![INVALID; verts];
    let mut points = Vec::new();
    for v in 0..verts {
        let c = canon[v] as usize;
        if point_of[c] == INVALID {
            let p = &gl.vert_properties[c * num_prop..];
            point_of[c] = points.len() as Index;
            points.push(to_local.transform_point3(DVec3::new(p[0] as f64, p[1] as f64, p[2] as f64)));
        }
        point_of[v] = point_of[c];
    }
    let sizes = vec![3 as Index; gl.num_tri()];
    let idx: Vec<Index> = gl.tri_verts.iter().map(|&v| point_of[v as usize]).collect();
    let mut m = Mesh::from_polygons(&points, &sizes, &idx);

    // Which operand point each result point is, if any: its id survived exactly and it sits where that point went.
    let scale: Vec<f64> = operands.iter().map(|o| o.mesh.bounds().size().length() * 1e-5 + 1e-9).collect();
    let mut origin = vec![0i32; m.point_capacity() as usize];
    let mut operand_of_point = vec![INVALID; m.point_capacity() as usize];
    for t in 0..gl.num_tri() {
        for k in 0..3 {
            let v = gl.tri_verts[t * 3 + k] as usize;
            let p = point_of[v] as usize;
            if origin[p] != 0 {
                continue;
            }
            let o = operand_of_face(gl.face_id[t]);
            let src = operands[o].mesh;
            let id = gl.vert_properties[v * num_prop + 5];
            if id != id.floor() || id < 0.0 || id as Index >= src.point_capacity() || !src.point_valid(id as Index) {
                continue;
            }
            let w = operands[o].to_world.transform_point3(src.position(id as Index));
            let q = &gl.vert_properties[v * num_prop..];
            if (w - DVec3::new(q[0] as f64, q[1] as f64, q[2] as f64)).length() <= scale[o] {
                origin[p] = id as i32 + 1;
                operand_of_point[p] = o as Index;
            }
        }
    }
    m.point_attrs_mut().get_or_add::<i32>(ORIGIN_ATTR).copy_from_slice(&origin);

    let sets = colors::sets(base);
    for set in &sets {
        colors::add(&mut m, set, [1.0; 4]);
    }

    // Materials by name into the base's list, so a cutter's material shows inside the cut.
    let any_material = operands.iter().any(|o| o.mesh.face_attrs().has(names::MATERIAL));
    let mut material_remap: Vec<Vec<i32>> = vec![Vec::new(); operands.len()];
    if any_material {
        *m.material_names_mut() = base.material_names().clone();
        for (o, op) in operands.iter().enumerate() {
            for name in op.mesh.material_names() {
                let at = match m.material_names().iter().position(|n| n == name) {
                    Some(i) => i,
                    None => {
                        m.material_names_mut().push(name.clone());
                        m.material_names().len() - 1
                    }
                };
                material_remap[o].push(at as i32);
            }
        }
    }

    let corner_cap = m.corner_capacity() as usize;
    let face_cap = m.face_capacity() as usize;
    let mut uv = vec![[0f32; 2]; corner_cap];
    let mut cols: Vec<Vec<[f32; 4]>> = sets.iter().map(|s| colors::values(&m, s).to_vec()).collect();
    let mut operand_attr = vec![0i32; face_cap];
    let mut material = vec![0i32; face_cap];
    for f in 0..(m.face_capacity() as usize).min(gl.num_tri()) {
        let f = f as Index;
        if !m.face_valid(f) {
            continue;
        }
        let o = operand_of_face(gl.face_id[f as usize]);
        operand_attr[f as usize] = o as i32;
        if any_material {
            let src_mat = operands[o].mesh.face_attrs().get::<i32>(names::MATERIAL);
            let src_face = (gl.face_id[f as usize] - first_face[o]) as usize;
            let mi = src_mat.get(src_face).copied().unwrap_or(0);
            material[f as usize] = if mi >= 0 && (mi as usize) < material_remap[o].len() { material_remap[o][mi as usize] } else { 0 };
        }
        let mut k = 0;
        m.for_each_face_corner(f, |c| {
            let p = &gl.vert_properties[gl.tri_verts[f as usize * 3 + k] as usize * num_prop..];
            uv[c as usize] = [p[3], p[4]];
            for (s, col) in cols.iter_mut().enumerate() {
                if PROPS + 4 * s + 3 < num_prop {
                    let b = PROPS + 4 * s;
                    col[c as usize] = [p[b], p[b + 1], p[b + 2], p[b + 3]];
                }
            }
            k += 1;
        });
    }
    m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0)).copy_from_slice(&uv);
    for (s, set) in sets.iter().enumerate() {
        colors::values_mut(&mut m, set).copy_from_slice(&cols[s]);
    }
    m.face_attrs_mut().get_or_add::<i32>(OPERAND_ATTR).copy_from_slice(&operand_attr);
    if any_material {
        m.face_attrs_mut().get_or_add::<i32>(names::MATERIAL).copy_from_slice(&material);
    }
    *m.color_set_info_mut() = base.color_set_info().clone();

    // Edges between two points of one operand that were an edge there keep its flags.
    let edge_cap = m.edge_capacity() as usize;
    let mut hard = vec![false; edge_cap];
    let mut uv_seam = vec![false; edge_cap];
    let has_crease = operands.iter().any(|o| o.mesh.edge_attrs().has(names::CREASE));
    let mut crease = vec![0f32; edge_cap];
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) {
            continue;
        }
        let (a, b) = (m.edge_point(e, 0) as usize, m.edge_point(e, 1) as usize);
        let o = operand_of_point[a];
        if o == INVALID || o != operand_of_point[b] {
            continue;
        }
        let src = operands[o as usize].mesh;
        let se = src.find_edge((origin[a] - 1) as Index, (origin[b] - 1) as Index);
        if se == INVALID {
            continue;
        }
        let se = se as usize;
        hard[e as usize] = src.edge_attrs().get::<bool>(names::HARD).get(se).copied().unwrap_or(false);
        uv_seam[e as usize] = src.edge_attrs().get::<bool>(names::SEAM).get(se).copied().unwrap_or(false);
        if has_crease {
            crease[e as usize] = src.edge_attrs().get::<f32>(names::CREASE).get(se).copied().unwrap_or(0.0);
        }
    }
    m.edge_attrs_mut().get_or_add::<bool>(names::HARD).copy_from_slice(&hard);
    m.edge_attrs_mut().get_or_add::<bool>(names::SEAM).copy_from_slice(&uv_seam);
    if has_crease {
        m.edge_attrs_mut().get_or_add::<f32>(names::CREASE).copy_from_slice(&crease);
    }

    // Triangles of the same input polygon are one polygon again: dissolve the edges between them.
    let mut inner = Vec::new();
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) || m.edge_face_count(e) != 2 {
            continue;
        }
        let c = m.edge_corner(e);
        let (f0, f1) = (m.corner_face(c) as usize, m.corner_face(m.corner_radial(c)) as usize);
        if f0 < gl.face_id.len() && f1 < gl.face_id.len() && gl.face_id[f0] == gl.face_id[f1] {
            inner.push(e);
        }
    }
    dissolve_edges(&mut m, &inner, false);
    m.compact(None);
    m
}

fn is_new(origin: &[i32], p: Index) -> bool {
    origin.get(p as usize).is_none_or(|&o| o == 0)
}

/// Marks the edges where faces of different operands meet. Returns them.
fn tag_seam(m: &mut Mesh) -> Vec<Index> {
    let operand = m.face_attrs().get::<i32>(OPERAND_ATTR).to_vec();
    let mut seam = vec![false; m.edge_capacity() as usize];
    let mut edges = Vec::new();
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) || m.edge_face_count(e) != 2 {
            continue;
        }
        let c = m.edge_corner(e);
        let (f0, f1) = (m.corner_face(c) as usize, m.corner_face(m.corner_radial(c)) as usize);
        if operand[f0] != operand[f1] {
            seam[e as usize] = true;
            edges.push(e);
        }
    }
    m.edge_attrs_mut().get_or_add::<bool>(BOOLEAN_SEAM_ATTR).copy_from_slice(&seam);
    edges
}

/// Welds new points into a neighbour closer than tol (onto an operand point when there is one, so the original shape
/// wins), then drops new points that only carry a straight line on. Kills the slivers a cut leaves near existing
/// points.
fn weld_seam(m: &mut Mesh, tol: f64) {
    let mut order: Vec<Index> = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.edge_length(e) < tol).collect();
    order.sort_by(|&a, &b| m.edge_length(a).total_cmp(&m.edge_length(b)));

    let origin = m.point_attrs().get::<i32>(ORIGIN_ATTR).to_vec();
    let mut target = vec![INVALID; m.point_capacity() as usize];
    let mut keep = vec![false; m.point_capacity() as usize];
    for e in order {
        let (mut a, mut b) = (m.edge_point(e, 0), m.edge_point(e, 1));
        let (na, nb) = (is_new(&origin, a), is_new(&origin, b));
        if !na && !nb {
            continue;
        }
        if (na && !nb) || (na && nb && a > b) {
            std::mem::swap(&mut a, &mut b); // b is the new one, a survives
        }
        if target[a as usize] != INVALID || target[b as usize] != INVALID || keep[b as usize] {
            continue;
        }
        target[b as usize] = a;
        keep[a as usize] = true;
    }
    weld_points(m, &target);

    for _ in 0..8 {
        let origin = m.point_attrs().get::<i32>(ORIGIN_ATTR).to_vec();
        let cap = m.point_capacity() as usize;
        let mut target = vec![INVALID; cap];
        let mut locked = vec![false; cap];
        let mut any = false;
        for p in 0..m.point_capacity() {
            if !m.point_valid(p) || !is_new(&origin, p) || locked[p as usize] || m.point_valence(p) != 2 {
                continue;
            }
            let mut ends = [INVALID; 2];
            let mut n = 0;
            m.for_each_point_edge(p, |e| {
                if n < 2 {
                    ends[n] = m.edge_other_point(e, p);
                    n += 1;
                }
            });
            if n != 2 || locked[ends[0] as usize] || locked[ends[1] as usize] {
                continue;
            }
            let (u, v) = (m.position(ends[0]) - m.position(p), m.position(ends[1]) - m.position(p));
            let (lu, lv) = (u.length(), v.length());
            // Within about half a degree of straight.
            if lu < 1e-300 || lv < 1e-300 || u.dot(v) / (lu * lv) > -0.99996 {
                continue;
            }
            target[p as usize] = ends[0];
            locked[p as usize] = true;
            locked[ends[0] as usize] = true;
            locked[ends[1] as usize] = true;
            any = true;
        }
        if !any {
            break;
        }
        weld_points(m, &target);
    }
    m.compact(None);
}

fn median_length(m: &Mesh, edges: &[Index]) -> f64 {
    let mut lengths: Vec<f64> = edges.iter().map(|&e| m.edge_length(e)).collect();
    if lengths.is_empty() {
        return 0.0;
    }
    let mid = lengths.len() / 2;
    let (_, v, _) = lengths.select_nth_unstable_by(mid, |a, b| a.total_cmp(b));
    *v
}

/// Faces sharing a point with the given ones, grown rings times.
fn grow_faces(m: &Mesh, mut faces: Vec<Index>, rings: i32) -> Vec<Index> {
    let mut inside = vec![false; m.face_capacity() as usize];
    for &f in &faces {
        inside[f as usize] = true;
    }
    for _ in 0..rings {
        let mut next = faces.clone();
        for &f in &faces {
            for p in m.face_points(f) {
                for g in m.point_faces(p) {
                    if !inside[g as usize] {
                        inside[g as usize] = true;
                        next.push(g);
                    }
                }
            }
        }
        faces = next;
    }
    faces
}

/// Remeshes the band of faces into quads that follow the seam, keeping the band's border points so the patch welds
/// back into the faces around it. False, leaving m as it was, when the remesh can't keep the border exactly (border
/// edges much longer or shorter than the band's own spacing), or no quad remesher is registered.
fn requad_band(m: &mut Mesh, band: &[Index], hard_angle: f64, timeout: f64) -> bool {
    let Some(remesh) = remesher().read().unwrap().clone() else { return false };
    let mut uv_sets = Vec::new();
    let mut k = 0;
    while m.corner_attrs().has(&names::uv(k)) {
        uv_sets.push(names::uv(k));
        k += 1;
    }
    let mut patch = Mesh::default();
    let mut to_patch = vec![INVALID; m.point_capacity() as usize];
    for &f in band {
        let mut pts = m.face_points(f);
        for p in pts.iter_mut() {
            if to_patch[*p as usize] == INVALID {
                to_patch[*p as usize] = patch.add_point(m.position(*p));
            }
            *p = to_patch[*p as usize];
        }
        let pf = patch.add_face(&pts);
        if pf == INVALID {
            continue;
        }
        for set in &uv_sets {
            let from = m.corner_attrs().get::<[f32; 2]>(set).to_vec();
            let (src, dst) = (m.face_corners(f), patch.face_corners(pf));
            let to = patch.corner_attrs_mut().get_or_add::<[f32; 2]>(set);
            for k in 0..src.len().min(dst.len()) {
                to[dst[k] as usize] = from.get(src[k] as usize).copied().unwrap_or([0.0; 2]);
            }
        }
    }
    let border_edges: Vec<Index> = (0..patch.edge_capacity()).filter(|&e| patch.edge_valid(e) && patch.is_boundary_edge(e)).collect();
    let pins: Vec<DVec3> =
        (0..patch.point_capacity()).filter(|&p| patch.point_valid(p) && patch.is_boundary_point(p)).map(|p| patch.position(p)).collect();
    if pins.is_empty() || border_edges.is_empty() {
        return false;
    }

    // The band keeps its own density: finer where the seam's edges are shorter than the border's.
    let mut o = QuadRemeshRequest {
        seamless: true,
        target_edge_length: median_length(&patch, &border_edges),
        adaptivity: 0.0,
        ..Default::default()
    };
    {
        let mut density = vec![0f32; patch.point_capacity() as usize];
        for p in 0..patch.point_capacity() {
            if !patch.point_valid(p) {
                continue;
            }
            let (mut sum, mut n) = (0.0, 0);
            patch.for_each_point_edge(p, |e| {
                sum += patch.edge_length(e);
                n += 1;
            });
            let local = if n > 0 { sum / n as f64 } else { o.target_edge_length };
            let ratio = o.target_edge_length / local.max(1e-9 * o.target_edge_length);
            density[p as usize] = (ratio * ratio).clamp(1.0, 8.0) as f32;
        }
        patch.point_attrs_mut().get_or_add::<f32>("density").copy_from_slice(&density);
    }
    o.pure_quads = false;
    o.sharp_angle = hard_angle;
    o.pinned_points = pins;
    o.timeout = timeout;
    let Some(mut q) = remesh(&patch, &o) else { return false };

    // Stitch the patch's border onto the band's: pair points that are each other's nearest, split a patch border edge
    // for each band point left over, and fold extra patch border points into a paired neighbour.
    let band_points: Vec<Index> = (0..m.point_capacity())
        .filter(|&p| to_patch[p as usize] != INVALID && patch.is_boundary_point(to_patch[p as usize]))
        .collect();
    let patch_border =
        |q: &Mesh| -> Vec<Index> { (0..q.point_capacity()).filter(|&v| q.point_valid(v) && q.is_boundary_point(v)).collect() };
    let reach = 0.6 * o.target_edge_length;
    let mut pair_of = vec![INVALID; m.point_capacity() as usize]; // band point -> patch point
    let border = patch_border(&q);
    let nearest_band = |at: DVec3| {
        let (mut best, mut d) = (INVALID, f64::MAX);
        for &p in &band_points {
            let l = (m.position(p) - at).length();
            if l < d {
                d = l;
                best = p;
            }
        }
        best
    };
    for &p in &band_points {
        let (mut best, mut d) = (INVALID, reach);
        for &v in &border {
            let l = (q.position(v) - m.position(p)).length();
            if l < d {
                d = l;
                best = v;
            }
        }
        if best != INVALID && nearest_band(q.position(best)) == p {
            pair_of[p as usize] = best;
        }
    }
    for &p in &band_points {
        if pair_of[p as usize] != INVALID {
            continue;
        }
        let (mut best_edge, mut best_d, mut best_t) = (INVALID, f64::MAX, 0.5);
        for e in 0..q.edge_capacity() {
            if !q.edge_valid(e) || !q.is_boundary_edge(e) {
                continue;
            }
            let (a0, a1) = (q.position(q.edge_point(e, 0)), q.position(q.edge_point(e, 1)));
            let t = ((m.position(p) - a0).dot(a1 - a0) / (a1 - a0).dot(a1 - a0).max(1e-300)).clamp(0.0, 1.0);
            let d = (a0 + t * (a1 - a0) - m.position(p)).length();
            if d < best_d {
                best_d = d;
                best_edge = e;
                best_t = t;
            }
        }
        if best_edge == INVALID || best_d > o.target_edge_length {
            return false;
        }
        pair_of[p as usize] = q.split_edge(best_edge, best_t.clamp(0.05, 0.95));
    }
    let mut q_target = vec![INVALID; q.point_capacity() as usize];
    let mut paired = vec![false; q.point_capacity() as usize];
    for &p in &band_points {
        q.set_position(pair_of[p as usize], m.position(p));
        paired[pair_of[p as usize] as usize] = true;
    }
    for v in patch_border(&q) {
        if paired[v as usize] {
            continue;
        }
        let (mut into, mut d) = (INVALID, f64::MAX);
        q.for_each_point_edge(v, |e| {
            let w = q.edge_other_point(e, v);
            if q.is_boundary_edge(e) && paired[w as usize] && q.edge_length(e) < d {
                d = q.edge_length(e);
                into = w;
            }
        });
        if into == INVALID {
            return false;
        }
        q_target[v as usize] = into;
    }
    weld_points(&mut q, &q_target);
    q.compact(None);
    // A border face that took two of the band's points has six sides or more.
    let large: Vec<Index> = (0..q.face_capacity()).filter(|&f| q.face_valid(f) && q.face_size(f) > 5).collect();
    split_ngons(&mut q, &large, &NgonSplitOptions::default());

    // Uvs from the band under each corner, probed a little toward its face's middle so a corner on a uv seam reads its
    // own side.
    if !uv_sets.is_empty() {
        let bvh = MeshBvh::new(&patch);
        for set in &uv_sets {
            let from = patch.corner_attrs().get::<[f32; 2]>(set).to_vec();
            let mut to = q.corner_attrs_mut().get_or_add::<[f32; 2]>(set).to_vec();
            for f in 0..q.face_capacity() {
                if !q.face_valid(f) {
                    continue;
                }
                let middle = q.face_centroid(f);
                q.for_each_face_corner(f, |c| {
                    let p = q.position(q.corner_point(c));
                    if let Some(hit) = bvh.closest_point(p + 0.05 * (middle - p)) {
                        to[c as usize] = bvh.interpolate(&from, hit.triangle, hit.bary);
                    }
                });
            }
            q.corner_attrs_mut().get_mut::<[f32; 2]>(set).copy_from_slice(&to);
        }
    }

    // The paired patch points now sit exactly on their band points; weld them by position.
    delete_faces(m, band, None);
    let offset = m.append(&q);
    let mut target = vec![INVALID; m.point_capacity() as usize];
    for v in offset..m.point_capacity() {
        if !m.point_valid(v) || !m.is_boundary_point(v) {
            continue;
        }
        if let Some(&p) = band_points.iter().find(|&&p| m.point_valid(p) && m.position(p) == m.position(v)) {
            target[v as usize] = p;
        }
    }
    weld_points(m, &target);
    true
}

/// Combines the operands in order (the first is the base) into a mesh in to_local space. Each operand's polygons come
/// back whole away from the seam, with their uvs, the base's colour sets, hard edges, creases, uv seams and materials.
pub fn mesh_boolean(operands: &[BooleanOperand], to_local: &DMat4, opts: &BooleanOptions) -> BooleanResult {
    let mut r = BooleanResult::default();
    if operands.is_empty() {
        r.error = "no base mesh".into();
        return r;
    }
    let color_sets = colors::sets(operands[0].mesh);
    let mut first_face = Vec::new();
    let mut offset = 0u32;
    let mut result: Option<Manifold> = None;
    for (i, op) in operands.iter().enumerate() {
        first_face.push(offset);
        let solid = Manifold::from_mesh_gl(&to_manifold(op.mesh, &op.to_world, offset, &color_sets));
        offset += op.mesh.face_capacity();
        if solid.status() != ManifoldError::NoError {
            r.error = format!("isn't a closed solid ({})", solid.status() as i32);
            r.bad_operand = Some(i);
            return r;
        }
        result = Some(match result {
            None => solid,
            Some(acc) => acc.boolean(
                &solid,
                match opts.op {
                    BooleanOp::Union => OpType::Add,
                    BooleanOp::Difference => OpType::Subtract,
                    BooleanOp::Intersection => OpType::Intersect,
                },
            ),
        });
    }

    let mut m = from_manifold(&result.unwrap().get_mesh_gl(-1), to_local, operands, &first_face);
    let mut seam = tag_seam(&mut m);
    if opts.cleanup && !seam.is_empty() {
        let tol = if opts.snap_distance >= 0.0 { opts.snap_distance } else { 0.1 * median_length(&m, &seam) };
        if tol > 0.0 {
            weld_seam(&mut m, tol);
            seam = tag_seam(&mut m);
        }
    }
    r.seam_edges = seam.len() as Index;

    // Points on the seam can be operand points too, where the cut runs exactly through them.
    let mut on_seam = vec![false; m.point_capacity() as usize];
    for &e in &seam {
        on_seam[m.edge_point(e, 0) as usize] = true;
        on_seam[m.edge_point(e, 1) as usize] = true;
    }

    if opts.fillet_width > 0.0 && !seam.is_empty() {
        bevel_edges(
            &mut m,
            &seam,
            &BevelOptions {
                offset: opts.fillet_width,
                segments: opts.fillet_segments.max(1),
                smoothing_angle: opts.hard_angle,
                ..Default::default()
            },
        );
    }

    // Everything the cut made: faces and edges touching the seam or a point no operand had.
    let origin = m.point_attrs().get::<i32>(ORIGIN_ATTR).to_vec();
    let touched_point = |p: Index, origin: &[i32]| is_new(origin, p) || on_seam.get(p as usize).copied().unwrap_or(false);
    let mut touched_faces = Vec::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let mut touched = false;
        m.for_each_face_corner(f, |c| touched |= touched_point(m.corner_point(c), &origin));
        if touched {
            touched_faces.push(f);
        }
    }
    let mut requadded = false;
    if opts.quad_band && !touched_faces.is_empty() {
        let band = grow_faces(&m, touched_faces.clone(), opts.band_rings.max(0));
        requadded = requad_band(&mut m, &band, opts.hard_angle, opts.band_timeout);
    }
    if requadded {
        r.quad_band = true;
    } else if opts.split_ngons {
        split_ngons(&mut m, &touched_faces, &NgonSplitOptions::default());
    }
    let origin = m.point_attrs().get::<i32>(ORIGIN_ATTR).to_vec();
    let touched_edges: Vec<Index> = (0..m.edge_capacity())
        .filter(|&e| m.edge_valid(e) && (touched_point(m.edge_point(e, 0), &origin) || touched_point(m.edge_point(e, 1), &origin)))
        .collect();
    set_hard_edges_by_angle(&mut m, opts.hard_angle, &touched_edges);

    m.point_attrs_mut().remove(ORIGIN_ATTR);
    m.face_attrs_mut().remove(OPERAND_ATTR);
    m.compact(None);
    r.mesh = m;
    r
}
