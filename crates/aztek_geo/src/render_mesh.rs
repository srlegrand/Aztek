//! Flat GPU-ready buffers built from a mesh.

use glam::Vec3;
use rayon::prelude::*;

use crate::attribute::{names, Index, INVALID};
use crate::colors;
use crate::mesh::Mesh;
use crate::normals::{corner_normals, corner_tangents, CornerNormalOptions};
use crate::par::Disjoint;
use crate::triangulate::{triangulate, TriangulateOptions};

const GRAIN: usize = 4096;

#[derive(Clone, Debug)]
pub struct RenderMeshOptions {
    pub max_uv_sets: i32,
    pub tangents: bool,
    pub colors: bool,
    /// Corner float4 layer the colors come from; empty is the mesh's current color set.
    pub color_layer: String,
    pub wireframe: bool,
    /// Group triangles into one section per face "material" id.
    pub sections: bool,
    /// Used when the mesh has no corner "normal" attribute.
    pub normals: CornerNormalOptions,
    pub triangulation: TriangulateOptions,
}

impl Default for RenderMeshOptions {
    fn default() -> Self {
        Self {
            max_uv_sets: 8,
            tangents: false,
            colors: true,
            color_layer: String::new(),
            wireframe: true,
            sections: true,
            normals: CornerNormalOptions::default(),
            triangulation: TriangulateOptions::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RenderSection {
    pub material: i32,
    pub first_index: Index,
    pub index_count: Index,
}

/// One vertex per unique combination of point and corner attributes, so vertices split exactly along hard edges,
/// uv seams and colour borders.
#[derive(Clone, Debug, Default)]
pub struct RenderMesh {
    pub positions: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    /// Empty unless requested and uv0 exists.
    pub tangents: Vec<[f32; 4]>,
    /// Empty unless the mesh has a color set.
    pub colors: Vec<[f32; 4]>,
    /// uv0, uv1, ... as present.
    pub uvs: Vec<Vec<[f32; 2]>>,
    /// Source point per vertex.
    pub vertex_point: Vec<Index>,
    /// One source corner per vertex.
    pub vertex_corner: Vec<Index>,
    /// Vertex per source corner, INVALID for dead corners.
    pub corner_vertex: Vec<Index>,
    /// 3 per triangle, ordered by section.
    pub indices: Vec<Index>,
    /// Source face per triangle.
    pub triangle_faces: Vec<Index>,
    /// Per triangle, bit k set when side k (vertex k to k + 1) is a polygon edge.
    pub triangle_sides: Vec<u8>,
    pub sections: Vec<RenderSection>,
    /// 2 per edge, into the vertex buffers.
    pub line_indices: Vec<Index>,
    /// Source edge per line.
    pub line_edges: Vec<Index>,
}

impl RenderMesh {
    pub fn vertex_count(&self) -> Index {
        self.positions.len() as Index
    }
    pub fn triangle_count(&self) -> Index {
        (self.indices.len() / 3) as Index
    }
}

struct CornerData<'a> {
    normals: &'a [Vec3],
    uvs: Vec<&'a [[f32; 2]]>,
    corner_color: &'a [[f32; 4]],
    point_color: &'a [[f32; 4]],
    tangents: &'a [[f32; 4]],
}

impl CornerData<'_> {
    fn same(&self, a: usize, b: usize) -> bool {
        self.normals[a] == self.normals[b]
            && self.uvs.iter().all(|uv| uv[a] == uv[b])
            && (self.corner_color.is_empty() || self.corner_color[a] == self.corner_color[b])
            && (self.tangents.is_empty() || self.tangents[a] == self.tangents[b])
    }
}

/// Parallel.
pub fn build_render_mesh(mesh: &Mesh, opts: &RenderMeshOptions) -> RenderMesh {
    let mut out = RenderMesh::default();
    let cc = mesh.corner_capacity() as usize;
    let pc = mesh.point_capacity() as usize;

    let stored: &[[f32; 3]] = mesh.corner_attrs().get::<[f32; 3]>(names::NORMAL);
    let normals_owned: Vec<Vec3> = if stored.len() == cc {
        stored.iter().map(|n| Vec3::from_array(*n)).collect()
    } else {
        corner_normals(mesh, &opts.normals)
    };
    let mut uvs = Vec::new();
    for i in 0..opts.max_uv_sets {
        let uv = mesh.corner_attrs().get::<[f32; 2]>(&names::uv(i));
        if uv.is_empty() {
            break;
        }
        uvs.push(uv);
    }
    let mut corner_color: &[[f32; 4]] = &[];
    let mut point_color: &[[f32; 4]] = &[];
    if opts.colors {
        let layer = if opts.color_layer.is_empty() {
            colors::layer_name(&colors::current(mesh))
        } else {
            opts.color_layer.clone()
        };
        corner_color = mesh.corner_attrs().get::<[f32; 4]>(&layer);
        if corner_color.is_empty() && opts.color_layer.is_empty() {
            point_color = mesh.point_attrs().get::<[f32; 4]>(names::COLOR);
        }
    }

    let tris = triangulate(mesh, &opts.triangulation);

    let tangents_owned = if opts.tangents && !uvs.is_empty() {
        corner_tangents(mesh, &tris, &normals_owned, &names::uv(0), true)
    } else {
        Vec::new()
    };
    let cd = CornerData {
        normals: &normals_owned,
        uvs,
        corner_color,
        point_color,
        tangents: &tangents_owned,
    };

    // Pass 1: slot of each corner among the unique vertices of its point. Dead corners and points keep none.
    let mut slot = vec![INVALID; cc];
    let mut base = vec![0u32; pc + 1];
    {
        let slot_w = Disjoint::new(&mut slot);
        base[1..].par_chunks_mut(GRAIN).enumerate().for_each(|(chunk, counts)| {
            let mut reps: Vec<Index> = Vec::new();
            for (i, count) in counts.iter_mut().enumerate() {
                let p = (chunk * GRAIN + i) as Index;
                if !mesh.point_valid(p) {
                    continue;
                }
                reps.clear();
                mesh.for_each_point_corner(p, |c| {
                    let s = reps
                        .iter()
                        .position(|&r| cd.same(r as usize, c as usize))
                        .unwrap_or_else(|| {
                            reps.push(c);
                            reps.len() - 1
                        });
                    // SAFETY: every corner belongs to exactly one point.
                    unsafe { slot_w.write(c as usize, s as Index) };
                });
                *count = reps.len() as Index;
            }
        });
    }
    let mut sum = 0u32;
    for b in base.iter_mut() {
        sum += *b;
        *b = sum;
    }
    let vertex_count = sum as usize;

    out.positions = vec![Vec3::ZERO; vertex_count];
    out.normals = vec![Vec3::ZERO; vertex_count];
    out.vertex_point = vec![0; vertex_count];
    out.vertex_corner = vec![0; vertex_count];
    out.uvs = vec![vec![[0.0; 2]; vertex_count]; cd.uvs.len()];
    let has_color = !cd.corner_color.is_empty() || !cd.point_color.is_empty();
    if has_color {
        out.colors = vec![[0.0; 4]; vertex_count];
    }
    if !cd.tangents.is_empty() {
        out.tangents = vec![[0.0; 4]; vertex_count];
    }

    // Pass 2: the first corner of each slot writes the vertex.
    let mut corner_vertex = vec![INVALID; cc];
    {
        let cv = Disjoint::new(&mut corner_vertex);
        let pos = Disjoint::new(&mut out.positions);
        let nrm = Disjoint::new(&mut out.normals);
        let vp = Disjoint::new(&mut out.vertex_point);
        let vc = Disjoint::new(&mut out.vertex_corner);
        let uvw: Vec<Disjoint<[f32; 2]>> = out.uvs.iter_mut().map(|u| Disjoint::new(u)).collect();
        let col = Disjoint::new(&mut out.colors);
        let tan = Disjoint::new(&mut out.tangents);
        (0..pc).into_par_iter().with_min_len(GRAIN).for_each(|p| {
            if !mesh.point_valid(p as Index) {
                return;
            }
            let position = mesh.position(p as Index).as_vec3();
            let mut next = 0;
            mesh.for_each_point_corner(p as Index, |c| {
                let c = c as usize;
                let v = (base[p] + slot[c]) as usize;
                // SAFETY: corners and the vertex range [base[p], base[p + 1]) belong to this point alone.
                unsafe {
                    cv.write(c, v as Index);
                    if slot[c] != next {
                        return;
                    }
                    next += 1;
                    pos.write(v, position);
                    nrm.write(v, cd.normals[c]);
                    vp.write(v, p as Index);
                    vc.write(v, c as Index);
                    for (i, uv) in cd.uvs.iter().enumerate() {
                        uvw[i].write(v, uv[c]);
                    }
                    if !cd.corner_color.is_empty() {
                        col.write(v, cd.corner_color[c]);
                    } else if !cd.point_color.is_empty() {
                        col.write(v, cd.point_color[p]);
                    }
                    if !cd.tangents.is_empty() {
                        tan.write(v, cd.tangents[c]);
                    }
                }
            });
        });
    }
    out.corner_vertex = corner_vertex;

    // Triangles, grouped by material with a stable counting sort.
    let tri_count = tris.triangle_count() as usize;
    let material = mesh.face_attrs().get::<i32>(names::MATERIAL);
    let mut order: Vec<Index> = vec![0; tri_count];
    if opts.sections && !material.is_empty() {
        let mat = |t: usize| material[tris.faces[t] as usize];
        let max_mat = (0..tri_count).map(mat).max().unwrap_or(0).max(0);
        let mut count = vec![0u32; max_mat as usize + 2];
        for t in 0..tri_count {
            count[mat(t).max(0) as usize + 1] += 1;
        }
        for m in 1..count.len() {
            if count[m] != 0 {
                out.sections.push(RenderSection {
                    material: (m - 1) as i32,
                    first_index: count[m - 1] * 3,
                    index_count: count[m] * 3,
                });
            }
            count[m] += count[m - 1];
        }
        for s in &mut out.sections {
            s.first_index = count[s.material as usize] * 3;
        }
        for t in 0..tri_count {
            let m = mat(t).max(0) as usize;
            order[count[m] as usize] = t as Index;
            count[m] += 1;
        }
    } else {
        for (t, o) in order.iter_mut().enumerate() {
            *o = t as Index;
        }
        if tri_count > 0 {
            out.sections.push(RenderSection { material: 0, first_index: 0, index_count: (tri_count * 3) as Index });
        }
    }

    out.indices = vec![0; tri_count * 3];
    out.triangle_faces = vec![0; tri_count];
    out.triangle_sides = vec![0; tri_count];
    {
        let cvx = &out.corner_vertex;
        out.indices
            .par_chunks_mut(3)
            .zip(out.triangle_faces.par_iter_mut())
            .zip(out.triangle_sides.par_iter_mut())
            .enumerate()
            .with_min_len(GRAIN * 4)
            .for_each(|(i, ((idx, face), sides_out))| {
                let t = order[i] as usize;
                let mut sides = 0u8;
                for k in 0..3 {
                    let a = tris.corners[t * 3 + k];
                    let b = tris.corners[t * 3 + (k + 1) % 3];
                    idx[k] = cvx[a as usize];
                    // A side between neighbouring corners of the polygon is one of its edges; the rest split it.
                    if mesh.corner_next(a) == b || mesh.corner_next(b) == a {
                        sides |= 1 << k;
                    }
                }
                *face = tris.faces[t];
                *sides_out = sides;
            });
    }

    if opts.wireframe {
        let ec = mesh.edge_capacity();
        for e in 0..ec {
            if mesh.edge_valid(e) && mesh.edge_corner(e) != INVALID {
                let c = mesh.edge_corner(e);
                out.line_indices.push(out.corner_vertex[c as usize]);
                out.line_indices.push(out.corner_vertex[mesh.corner_next(c) as usize]);
                out.line_edges.push(e);
            }
        }
    }
    out
}

/// New positions and normals for a mesh whose points moved, by the vertex to point and corner maps of an earlier
/// `build_render_mesh`: the topology, triangles and UVs are all as they were. Much cheaper than building again.
pub fn moved_vertices(mesh: &Mesh, vertex_point: &[Index], vertex_corner: &[Index], opts: &RenderMeshOptions) -> (Vec<Vec3>, Vec<Vec3>) {
    let stored: &[[f32; 3]] = mesh.corner_attrs().get::<[f32; 3]>(names::NORMAL);
    let computed: Vec<Vec3> = if stored.len() == mesh.corner_capacity() as usize { Vec::new() } else { corner_normals(mesh, &opts.normals) };
    let points = mesh.positions();
    let positions: Vec<Vec3> = vertex_point.par_iter().map(|&p| points[p as usize].as_vec3()).collect();
    let normals: Vec<Vec3> = vertex_corner
        .par_iter()
        .map(|&c| if computed.is_empty() { Vec3::from_array(stored[c as usize]) } else { computed[c as usize] })
        .collect();
    (positions, normals)
}

#[cfg(test)]
mod moved_tests {
    use super::*;
    use glam::DVec3;

    /// What the fast path gives is what building again gives.
    #[test]
    fn moved_vertices_match_a_full_build() {
        let pts = [DVec3::new(0., 0., 0.), DVec3::new(1., 0., 0.), DVec3::new(1., 1., 0.), DVec3::new(0., 1., 0.), DVec3::new(0.5, 0.5, 1.)];
        let m = Mesh::from_polygons(&pts, &[4, 3, 3, 3, 3], &[0, 3, 2, 1, 0, 1, 4, 1, 2, 4, 2, 3, 4, 3, 0, 4]);
        let opts = RenderMeshOptions { max_uv_sets: 1, ..Default::default() };
        let first = build_render_mesh(&m, &opts);
        let moved = m.with_positions(m.positions().iter().map(|p| DVec3::new(p.x * 2.0, p.y + 1.0, p.z * p.z + p.x)).collect());
        assert!(m.only_points_differ(&moved));
        let again = build_render_mesh(&moved, &opts);
        let (positions, normals) = moved_vertices(&moved, &first.vertex_point, &first.vertex_corner, &opts);
        assert_eq!(again.positions.len(), positions.len());
        for v in 0..positions.len() {
            assert!((positions[v] - again.positions[v]).length() < 1e-6, "{v}");
            assert!((normals[v] - again.normals[v]).length() < 1e-6, "{v}");
        }
    }
}
