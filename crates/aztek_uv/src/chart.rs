//! UV islands prepared for solving.

use std::collections::HashMap;

use aztek_geo::triangulate::{triangulate_face, TriangulateOptions};
use aztek_geo::{names, Index, Mesh, INVALID};
use glam::{DVec2, DVec3};
use rayon::prelude::*;

/// One UV island prepared for solving. Vertices are UV wedges: a mesh point splits into several where seams (or
/// boundaries) separate its corners, so a cylinder cut along one seam has two vertices per seam point. Triangles come
/// from the faces' deterministic triangulation.
#[derive(Clone, Debug, Default)]
pub struct Chart {
    /// Mesh faces, ascending.
    pub faces: Vec<Index>,
    /// Mesh corners of those faces, face by face.
    pub corners: Vec<Index>,
    /// Per face, its first entry in corners; one extra at the end.
    pub face_start: Vec<Index>,
    /// Chart vertex per entry of corners.
    pub corner_vertex: Vec<Index>,
    /// Mesh point per vertex.
    pub vertex_point: Vec<Index>,
    /// 3D position per vertex.
    pub pos: Vec<DVec3>,
    pub uv: Vec<DVec2>,
    /// Per vertex, true keeps uv fixed in solves.
    pub pinned: Vec<bool>,
    /// Chart vertices, wound like the faces.
    pub tris: Vec<[Index; 3]>,
    /// Mesh face per triangle.
    pub tri_face: Vec<Index>,
}

impl Chart {
    pub fn vertex_count(&self) -> Index {
        self.pos.len() as Index
    }
    pub fn triangle_count(&self) -> Index {
        self.tris.len() as Index
    }

    pub fn area_3d(&self) -> f64 {
        self.tris
            .iter()
            .map(|t| {
                let (p0, p1, p2) = (self.pos[t[0] as usize], self.pos[t[1] as usize], self.pos[t[2] as usize]);
                0.5 * (p1 - p0).cross(p2 - p0).length()
            })
            .sum()
    }

    /// Signed; positive when the island is not mirrored.
    pub fn area_uv(&self) -> f64 {
        let mut a = 0.0;
        for t in &self.tris {
            let e1 = self.uv[t[1] as usize] - self.uv[t[0] as usize];
            let e2 = self.uv[t[2] as usize] - self.uv[t[0] as usize];
            a += 0.5 * (e1.x * e2.y - e1.y * e2.x);
        }
        a
    }

    /// Ordered boundary loops of chart vertices, longest (by 3D length) first.
    pub fn boundary_loops(&self) -> Vec<Vec<Index>> {
        let key = |a: Index, b: Index| ((a as u64) << 32) | b as u64;
        let mut directed: HashMap<u64, i32> = HashMap::new();
        for t in &self.tris {
            for i in 0..3 {
                *directed.entry(key(t[i], t[(i + 1) % 3])).or_insert(0) += 1;
            }
        }
        // Outgoing boundary edges per vertex, ascending, so a vertex the boundary passes twice is walked the same
        // way every run.
        let mut next: HashMap<Index, Vec<Index>> = HashMap::new();
        let mut edges = Vec::new();
        for t in &self.tris {
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                if !directed.contains_key(&key(b, a)) {
                    next.entry(a).or_default().push(b);
                    edges.push(key(a, b));
                }
            }
        }
        for v in next.values_mut() {
            v.sort_unstable();
        }
        edges.sort_unstable();
        let mut used: HashMap<u64, bool> = HashMap::new();
        let mut loops = Vec::new();
        let mut lengths = Vec::new();
        for &start in &edges {
            if used.get(&start).copied().unwrap_or(false) {
                continue;
            }
            let mut lp = Vec::new();
            let (a, mut b) = ((start >> 32) as Index, (start & 0xffff_ffff) as Index);
            used.insert(start, true);
            lp.push(a);
            let mut len = (self.pos[b as usize] - self.pos[a as usize]).length();
            let first = a;
            while b != first && lp.len() <= edges.len() {
                lp.push(b);
                let nb = next
                    .get(&b)
                    .and_then(|outs| outs.iter().copied().find(|&n| !used.get(&key(b, n)).copied().unwrap_or(false)));
                let Some(nb) = nb else { break };
                used.insert(key(b, nb), true);
                len += (self.pos[nb as usize] - self.pos[b as usize]).length();
                b = nb;
            }
            loops.push(lp);
            lengths.push(len);
        }
        let mut order: Vec<usize> = (0..loops.len()).collect();
        order.sort_by(|&x, &y| lengths[y].partial_cmp(&lengths[x]).unwrap_or(std::cmp::Ordering::Equal));
        order.into_iter().map(|i| std::mem::take(&mut loops[i])).collect()
    }

    /// Vertices minus edges plus faces of the triangulated chart: 1 for a disk.
    pub fn euler(&self) -> i32 {
        let mut edges = Vec::with_capacity(self.tris.len() * 3);
        for t in &self.tris {
            for i in 0..3 {
                let (a, b) = (t[i], t[(i + 1) % 3]);
                edges.push(((a.min(b) as u64) << 32) | a.max(b) as u64);
            }
        }
        edges.sort_unstable();
        edges.dedup();
        self.pos.len() as i32 - edges.len() as i32 + self.tris.len() as i32
    }

    pub fn uv_min(&self) -> DVec2 {
        self.uv.iter().fold(DVec2::splat(f64::MAX), |m, &p| m.min(p))
    }

    pub fn uv_max(&self) -> DVec2 {
        self.uv.iter().fold(DVec2::splat(-f64::MAX), |m, &p| m.max(p))
    }

    pub fn uv_centroid(&self) -> DVec2 {
        let mut s = DVec2::ZERO;
        let mut w = 0.0;
        for t in &self.tris {
            let (u0, u1, u2) = (self.uv[t[0] as usize], self.uv[t[1] as usize], self.uv[t[2] as usize]);
            let (e1, e2) = (u1 - u0, u2 - u0);
            let a = (e1.x * e2.y - e1.y * e2.x).abs();
            s += a * (u0 + u1 + u2) / 3.0;
            w += a;
        }
        if w > 0.0 {
            return s / w;
        }
        if self.uv.is_empty() {
            DVec2::ZERO
        } else {
            (self.uv_min() + self.uv_max()) * 0.5
        }
    }
}

/// How islands are found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum IslandSource {
    /// Split along edge "seam", mesh boundaries and non-manifold edges.
    #[default]
    Seams,
    /// Split wherever the corners of the uv set disagree across an edge.
    Uvs,
}

#[derive(Clone, Debug)]
pub struct ChartOptions<'a> {
    pub source: IslandSource,
    pub uv_set: &'a str,
    /// Extra edges to treat as seams, per edge capacity (optional).
    pub extra_seams: &'a [bool],
    /// With IslandSource::Seams, whether the mesh's own "seam" edges split.
    pub use_seam_attr: bool,
    /// Corner bool attribute marking pinned uvs; any pinned corner pins its wedge.
    pub pin_attr: &'a str,
}

impl Default for ChartOptions<'_> {
    fn default() -> Self {
        Self { source: IslandSource::Seams, uv_set: "uv0", extra_seams: &[], use_seam_attr: true, pin_attr: "uvPin" }
    }
}

struct UnionFind {
    parent: Vec<Index>,
}

impl UnionFind {
    fn new(n: usize) -> Self {
        Self { parent: (0..n as Index).collect() }
    }

    fn find(&mut self, mut x: Index) -> Index {
        while self.parent[x as usize] != x {
            let p = self.parent[self.parent[x as usize] as usize];
            self.parent[x as usize] = p;
            x = p;
        }
        x
    }

    fn unite(&mut self, a: Index, b: Index) {
        let (a, b) = (self.find(a), self.find(b));
        if a == b {
            return;
        }
        // Lower id wins so roots do not depend on join order.
        if a < b {
            self.parent[b as usize] = a;
        } else {
            self.parent[a as usize] = b;
        }
    }
}

pub(crate) fn same_uv(a: [f32; 2], b: [f32; 2]) -> bool {
    (a[0] - b[0]).abs() <= 1e-6 && (a[1] - b[1]).abs() <= 1e-6
}

/// Mesh id to local index: a dense table when the subset is a good part of the mesh, else a hash map.
enum IdMap {
    Dense(Vec<Index>),
    Hash(HashMap<Index, Index>),
}

impl IdMap {
    fn new(capacity: usize, count: usize) -> Self {
        if count * 8 > capacity {
            IdMap::Dense(vec![INVALID; capacity])
        } else {
            IdMap::Hash(HashMap::with_capacity(count * 2))
        }
    }

    fn set(&mut self, key: Index, value: Index) {
        match self {
            IdMap::Dense(t) => t[key as usize] = value,
            IdMap::Hash(h) => {
                h.insert(key, value);
            }
        }
    }

    fn get(&self, key: Index) -> Index {
        match self {
            IdMap::Dense(t) => t[key as usize],
            IdMap::Hash(h) => h.get(&key).copied().unwrap_or(INVALID),
        }
    }
}

/// Islands of a face subset, in local indices so the cost follows the subset, not the mesh.
struct Grouping {
    /// Per input face.
    face_chart: Vec<Index>,
    /// Local corner to mesh corner, face by face.
    corners: Vec<Index>,
    /// Per input face, plus one.
    face_start: Vec<Index>,
    /// Per local corner.
    corner_root: Vec<Index>,
    chart_count: Index,
}

fn group(mesh: &Mesh, faces: &[Index], opts: &ChartOptions) -> Grouping {
    let mut corners = Vec::new();
    let mut face_start = Vec::new();
    let mut corner_local = IdMap::new(mesh.corner_capacity() as usize, faces.len() * 4);
    let mut face_local = IdMap::new(mesh.face_capacity() as usize, faces.len());
    for (i, &f) in faces.iter().enumerate() {
        face_local.set(f, i as Index);
        face_start.push(corners.len() as Index);
        mesh.for_each_face_corner(f, |c| {
            corner_local.set(c, corners.len() as Index);
            corners.push(c);
        });
    }
    face_start.push(corners.len() as Index);

    let seam: &[bool] = if opts.use_seam_attr { mesh.edge_attrs().get::<bool>(names::SEAM) } else { &[] };
    let uv = mesh.corner_attrs().get::<[f32; 2]>(opts.uv_set);
    let by_uv = opts.source == IslandSource::Uvs && !uv.is_empty();

    let mut face_sets = UnionFind::new(faces.len());
    let mut corner_sets = UnionFind::new(corners.len());
    for &own in &corners {
        let e = mesh.corner_edge(own);
        let extra = opts.extra_seams.get(e as usize).copied().unwrap_or(false);
        if extra || (!by_uv && seam.get(e as usize).copied().unwrap_or(false)) {
            continue;
        }
        let mut c = [INVALID; 3];
        let mut n = 0;
        mesh.for_each_edge_corner(e, |r| {
            if n < 3 && face_local.get(mesh.corner_face(r)) != INVALID {
                c[n] = r;
                n += 1;
            }
        });
        // Each edge once: from its lowest corner in the set.
        if n != 2 || c[0].min(c[1]) != own || mesh.corner_face(c[0]) == mesh.corner_face(c[1]) {
            continue;
        }
        let opposite = mesh.corner_point(c[0]) != mesh.corner_point(c[1]);
        // Corner pairs that sit on the same point on either side of the edge.
        let (a0, a1) = (c[0], if opposite { mesh.corner_next(c[1]) } else { c[1] });
        let (b0, b1) = (mesh.corner_next(c[0]), if opposite { c[1] } else { mesh.corner_next(c[1]) });
        if by_uv && (!same_uv(uv[a0 as usize], uv[a1 as usize]) || !same_uv(uv[b0 as usize], uv[b1 as usize])) {
            continue;
        }
        face_sets.unite(face_local.get(mesh.corner_face(c[0])), face_local.get(mesh.corner_face(c[1])));
        corner_sets.unite(corner_local.get(a0), corner_local.get(a1));
        corner_sets.unite(corner_local.get(b0), corner_local.get(b1));
    }

    let mut face_chart = vec![0; faces.len()];
    let mut root_chart = vec![INVALID; faces.len()];
    let mut chart_count = 0;
    for i in 0..faces.len() {
        let r = face_sets.find(i as Index) as usize;
        if root_chart[r] == INVALID {
            root_chart[r] = chart_count;
            chart_count += 1;
        }
        face_chart[i] = root_chart[r];
    }
    let corner_root = (0..corners.len()).map(|c| corner_sets.find(c as Index)).collect();
    Grouping { face_chart, corners, face_start, corner_root, chart_count }
}

/// Live faces as a list, or the given ones filtered to live faces.
pub fn live_faces(mesh: &Mesh, faces: &[Index]) -> Vec<Index> {
    if faces.is_empty() {
        return (0..mesh.face_capacity()).filter(|&f| mesh.face_valid(f)).collect();
    }
    let mut out = faces.to_vec();
    out.sort_unstable();
    out.dedup();
    out.retain(|&f| mesh.face_valid(f));
    out
}

/// Face id lists of the islands, without building chart geometry.
pub fn island_faces(mesh: &Mesh, faces: &[Index], opts: &ChartOptions) -> Vec<Vec<Index>> {
    let live = live_faces(mesh, faces);
    let g = group(mesh, &live, opts);
    let mut out = vec![Vec::new(); g.chart_count as usize];
    for (i, &f) in live.iter().enumerate() {
        out[g.face_chart[i] as usize].push(f);
    }
    out
}

/// Every face of the islands (among all live faces) that hold at least one of the seed faces, ascending.
pub fn islands_of_faces(mesh: &Mesh, seeds: &[Index], opts: &ChartOptions) -> Vec<Index> {
    let mut out = Vec::new();
    if seeds.is_empty() {
        return out;
    }
    let mut seed = vec![false; mesh.face_capacity() as usize];
    for &f in seeds {
        if let Some(s) = seed.get_mut(f as usize) {
            *s = true;
        }
    }
    for island in island_faces(mesh, &[], opts) {
        if island.iter().any(|&f| seed[f as usize]) {
            out.extend(island);
        }
    }
    out.sort_unstable();
    out
}

/// Islands of the given faces (all live faces when empty). Charts and their vertices come out in a stable order, so
/// the same mesh always gives the same charts. Current uvs are loaded when the set exists.
pub fn build_charts(mesh: &Mesh, faces: &[Index], opts: &ChartOptions) -> Vec<Chart> {
    let live = live_faces(mesh, faces);
    let g = group(mesh, &live, opts);
    let mut charts = vec![Chart::default(); g.chart_count as usize];
    let mut chart_local_faces = vec![Vec::new(); g.chart_count as usize];
    for (i, &f) in live.iter().enumerate() {
        charts[g.face_chart[i] as usize].faces.push(f);
        chart_local_faces[g.face_chart[i] as usize].push(i as Index);
    }

    let uv = mesh.corner_attrs().get::<[f32; 2]>(opts.uv_set);
    let pin = mesh.corner_attrs().get::<bool>(opts.pin_attr);
    // Charts own disjoint corners, so each is built on its own.
    charts.par_iter_mut().zip(chart_local_faces.par_iter()).for_each(|(ch, local)| {
            let mut root_vertex: HashMap<Index, Index> = HashMap::new();
            let mut corner_vertex: HashMap<Index, Index> = HashMap::new();
            for &lf in local {
                ch.face_start.push(ch.corners.len() as Index);
                for lc in g.face_start[lf as usize]..g.face_start[lf as usize + 1] {
                    let c = g.corners[lc as usize];
                    let next = ch.pos.len() as Index;
                    let v = *root_vertex.entry(g.corner_root[lc as usize]).or_insert(next);
                    if v == next {
                        let p = mesh.corner_point(c);
                        ch.vertex_point.push(p);
                        ch.pos.push(mesh.position(p));
                        ch.uv.push(uv.get(c as usize).map_or(DVec2::ZERO, |u| DVec2::new(u[0] as f64, u[1] as f64)));
                        ch.pinned.push(false);
                    }
                    ch.corners.push(c);
                    ch.corner_vertex.push(v);
                    corner_vertex.insert(c, v);
                    if pin.get(c as usize).copied().unwrap_or(false) {
                        ch.pinned[v as usize] = true;
                    }
                }
            }
            ch.face_start.push(ch.corners.len() as Index);
            let mut tri = Vec::new();
            for &f in &ch.faces {
                tri.clear();
                triangulate_face(mesh, f, &mut tri, &TriangulateOptions::default());
                for t in tri.chunks_exact(3) {
                    ch.tris.push([corner_vertex[&t[0]], corner_vertex[&t[1]], corner_vertex[&t[2]]]);
                    ch.tri_face.push(f);
                }
            }
        });
    charts
}

/// Corner uvs of a set, added (zeroed) when missing.
pub fn uv_layer<'a>(mesh: &'a mut Mesh, uv_set: &str) -> &'a mut [[f32; 2]] {
    mesh.corner_attrs_mut().get_or_add::<[f32; 2]>(uv_set)
}

/// Writes each chart's vertex uvs into its corners, adding the uv set if needed.
pub fn write_charts(mesh: &mut Mesh, charts: &[Chart], uv_set: &str) {
    let uv = uv_layer(mesh, uv_set);
    for ch in charts {
        for (i, &c) in ch.corners.iter().enumerate() {
            let v = ch.uv[ch.corner_vertex[i] as usize];
            uv[c as usize] = [v.x as f32, v.y as f32];
        }
    }
}

/// Sets "seam" on the edges of the faces wherever the uv set splits (or the face set ends) and clears it where both
/// sides agree. Edges away from the faces are left alone. Returns the seam count among them.
pub fn mark_seams_from_uvs(mesh: &mut Mesh, faces: &[Index], uv_set: &str) -> Index {
    let live = live_faces(mesh, faces);
    let mut in_set = vec![false; mesh.face_capacity() as usize];
    for &f in &live {
        in_set[f as usize] = true;
    }
    let uv = mesh.corner_attrs().get::<[f32; 2]>(uv_set);
    let mut state = vec![-1i8; mesh.edge_capacity() as usize];
    for &f in &live {
        mesh.for_each_face_corner(f, |c| {
            let e = mesh.corner_edge(c);
            if state[e as usize] >= 0 {
                return;
            }
            let mut split = false;
            let mut count = 0;
            mesh.for_each_edge_corner(e, |r| {
                count += 1;
                if r == c {
                    return;
                }
                if !in_set[mesh.corner_face(r) as usize] {
                    split = true;
                    return;
                }
                if uv.is_empty() {
                    return;
                }
                let same = mesh.corner_point(r) == mesh.corner_point(c);
                let (a, b) = if same {
                    (uv[r as usize], uv[mesh.corner_next(r) as usize])
                } else {
                    (uv[mesh.corner_next(r) as usize], uv[r as usize])
                };
                if !same_uv(a, uv[c as usize]) || !same_uv(b, uv[mesh.corner_next(c) as usize]) {
                    split = true;
                }
            });
            state[e as usize] = if split && count > 1 { 1 } else { 0 };
        });
    }
    let seam = mesh.edge_attrs_mut().get_or_add::<bool>(names::SEAM);
    let mut n = 0;
    for (e, &s) in state.iter().enumerate() {
        if s >= 0 {
            seam[e] = s == 1;
            n += (s == 1) as Index;
        }
    }
    n
}

/// Unsigned angle between the two faces of a manifold edge, degrees; 0 when flat, 180 when folded.
pub fn dihedral_angle(mesh: &Mesh, e: Index) -> f64 {
    let c = mesh.edge_corner(e);
    if c == INVALID {
        return 0.0;
    }
    let r = mesh.corner_radial(c);
    if r == c {
        return 0.0;
    }
    let d = mesh.face_normal(mesh.corner_face(c)).dot(mesh.face_normal(mesh.corner_face(r)));
    d.clamp(-1.0, 1.0).acos().to_degrees()
}
