//! Catmull-Clark subdivision and its point stencils.

use glam::DVec3;
use rayon::prelude::*;

use crate::attribute::{names, Domain, Index, INVALID};
use crate::mesh::Mesh;
use crate::mix::mix3;

const REFINE_GRAIN: usize = 2048;

#[derive(Clone, Copy, Debug)]
pub struct SubdivideOptions {
    pub levels: i32,
    /// Boundary edges stay sharp (the reference's default).
    pub sharp_boundary: bool,
    /// Normals stay split along hard edges; off, the result is smooth-shaded everywhere, like the reference's smooth preview.
    pub keep_hard_edges: bool,
}

impl Default for SubdivideOptions {
    fn default() -> Self {
        Self { levels: 1, sharp_boundary: true, keep_hard_edges: false }
    }
}

/// Where the cage's components went in subdivide's result.
#[derive(Clone, Debug, Default)]
pub struct SubdivideMap {
    /// Per cage point: the point it became, INVALID for a dead one.
    pub points: Vec<Index>,
    /// Per cage face: the point at its centre, INVALID for a dead one.
    pub face_points: Vec<Index>,
    /// Per result edge: the cage edge it runs along, INVALID for one inside a cage face.
    pub edges: Vec<Index>,
    /// Per result face: the cage face it is part of.
    pub faces: Vec<Index>,
}

/// Sparse linear map between point sets: out row i = sum of weights[k] * in point cols[k], k in
/// [offsets[i], offsets[i+1]).
#[derive(Clone, Debug)]
pub struct PointStencil {
    pub offsets: Vec<Index>,
    pub cols: Vec<Index>,
    pub weights: Vec<f64>,
    /// Point capacity of the input.
    pub inputs: Index,
}

impl Default for PointStencil {
    fn default() -> Self {
        Self { offsets: vec![0], cols: Vec::new(), weights: Vec::new(), inputs: 0 }
    }
}

impl PointStencil {
    pub fn rows(&self) -> Index {
        (self.offsets.len() - 1) as Index
    }
    pub fn apply(&self, input: &[DVec3]) -> Vec<DVec3> {
        (0..self.rows() as usize)
            .into_par_iter()
            .with_min_len(REFINE_GRAIN)
            .map(|i| {
                let mut sum = DVec3::ZERO;
                for k in self.offsets[i] as usize..self.offsets[i + 1] as usize {
                    sum += input[self.cols[k] as usize] * self.weights[k];
                }
                sum
            })
            .collect()
    }
}

fn copy_layout(out: &mut Mesh, src: &Mesh) {
    for d in Domain::ALL {
        out.attributes_mut(d).merge_layout(src.attributes(d));
    }
    *out.material_names_mut() = src.material_names().clone();
    *out.color_set_info_mut() = src.color_set_info().clone();
}

fn point_attributes(out: &mut Mesh, src: &Mesh, p_id: &[Index], e_id: &[Index], f_id: &[Index]) {
    if out.point_attrs().layer_count() == 0 {
        return;
    }
    let pa = out.point_attrs_mut();
    for (p, &id) in p_id.iter().enumerate() {
        if id != INVALID {
            pa.copy_from(src.point_attrs(), id as usize, p);
        }
    }
    for (e, &id) in e_id.iter().enumerate() {
        if id != INVALID {
            let s = [src.edge_point(e as Index, 0), src.edge_point(e as Index, 1)];
            pa.mix_from(src.point_attrs(), id as usize, &s, &[0.5, 0.5]);
        }
    }
    for (f, &id) in f_id.iter().enumerate() {
        if id != INVALID {
            let s = src.face_points(f as Index);
            let w = vec![1.0 / s.len() as f32; s.len()];
            pa.mix_from(src.point_attrs(), id as usize, &s, &w);
        }
    }
}

fn corner_attributes(out: &mut Mesh, src: &Mesh, oc: Index, c: Index, cp: Index, cn: Index, corners: &[Index], avg: &[f32]) {
    let n1 = out.corner_next(oc);
    let n2 = out.corner_next(n1);
    let pv = out.corner_prev(oc);
    let ca = out.corner_attrs_mut();
    ca.copy_from(src.corner_attrs(), oc as usize, c as usize);
    ca.mix_from(src.corner_attrs(), n1 as usize, &[c, cn], &[0.5, 0.5]);
    ca.mix_from(src.corner_attrs(), n2 as usize, corners, avg);
    ca.mix_from(src.corner_attrs(), pv as usize, &[cp, c], &[0.5, 0.5]);
}

fn drop_crease(out: &mut Mesh) {
    for v in out.edge_attrs_mut().get_mut::<f32>(names::CREASE) {
        *v = (*v - 1.0).max(0.0);
    }
}

/// The subdivided mesh built at once: every face's quads laid out in source order, then their attributes copied face
/// by face. None when the builder dropped a face, for the face-by-face build to handle.
fn build_quads(
    src: &Mesh,
    pos: &[DVec3],
    p_id: &[Index],
    e_id: &[Index],
    f_id: &[Index],
    has_crease: bool,
    map: Option<&mut SubdivideMap>,
) -> Option<Mesh> {
    let fc = src.face_capacity();
    // The first quad of each source face: one per corner.
    let mut first_quad = vec![0 as Index; fc as usize + 1];
    for f in 0..fc {
        first_quad[f as usize + 1] = first_quad[f as usize] + if f_id[f as usize] != INVALID { src.face_size(f) } else { 0 };
    }
    let quads = first_quad[fc as usize];
    let sizes = vec![4 as Index; quads as usize];
    let corners: Vec<Index> = (0..fc)
        .into_par_iter()
        .with_min_len(REFINE_GRAIN)
        .flat_map_iter(|f| {
            let mut out = Vec::new();
            if f_id[f as usize] != INVALID {
                for c in src.face_corners(f) {
                    out.extend([
                        p_id[src.corner_point(c) as usize],
                        e_id[src.corner_edge(c) as usize],
                        f_id[f as usize],
                        e_id[src.corner_edge(src.corner_prev(c)) as usize],
                    ]);
                }
            }
            out
        })
        .collect();
    let mut out = Mesh::from_polygons(pos, &sizes, &corners);
    if out.face_capacity() != quads {
        return None;
    }
    copy_layout(&mut out, src);
    point_attributes(&mut out, src, p_id, e_id, f_id);
    let mut map = map;
    if let Some(m) = map.as_deref_mut() {
        m.points = p_id.to_vec();
        m.face_points = f_id.to_vec();
        m.edges = vec![INVALID; out.edge_capacity() as usize];
        m.faces = vec![INVALID; out.face_capacity() as usize];
    }
    let has_corner_attrs = out.corner_attrs().layer_count() > 0;
    let has_edge_attrs = out.edge_attrs().layer_count() > 0;
    for f in 0..fc {
        if f_id[f as usize] == INVALID {
            continue;
        }
        let face_corners = src.face_corners(f);
        let n = face_corners.len();
        let avg = vec![1.0 / n as f32; n];
        for i in 0..n {
            let c = face_corners[i];
            let cp = face_corners[(i + n - 1) % n];
            let cn = face_corners[(i + 1) % n];
            let nf = first_quad[f as usize] + i as Index;
            out.face_attrs_mut().copy_from(src.face_attrs(), nf as usize, f as usize);
            let oc = out.face_corner(nf);
            // Each half of a source edge sits in a quad of every face on that edge; the edge's own first face writes
            // it.
            let mut outer = |out: &mut Mesh, out_edge: Index, src_edge: Index| {
                if src.corner_face(src.edge_corner(src_edge)) != f {
                    return;
                }
                if has_edge_attrs {
                    out.edge_attrs_mut().copy_from(src.edge_attrs(), out_edge as usize, src_edge as usize);
                }
                if let Some(m) = map.as_deref_mut() {
                    m.edges[out_edge as usize] = src_edge;
                }
            };
            let e0 = out.corner_edge(oc);
            let e3 = out.corner_edge(out.corner_prev(oc));
            outer(&mut out, e0, src.corner_edge(c));
            outer(&mut out, e3, src.corner_edge(cp));
            if let Some(m) = map.as_deref_mut() {
                m.faces[nf as usize] = f;
            }
            if has_corner_attrs {
                corner_attributes(&mut out, src, oc, c, cp, cn, &face_corners, &avg);
            }
        }
    }
    if has_crease {
        drop_crease(&mut out);
    }
    Some(out)
}

fn sharpness(src: &Mesh, crease: &[f32], e: Index) -> f64 {
    if !src.is_manifold_edge(e) {
        return 1.0;
    }
    if crease.is_empty() {
        0.0
    } else {
        (crease[e as usize] as f64).clamp(0.0, 1.0)
    }
}

fn subdivide_once(src: &Mesh, opts: &SubdivideOptions, map: Option<&mut SubdivideMap>) -> Mesh {
    let pc = src.point_capacity();
    let ec = src.edge_capacity();
    let fc = src.face_capacity();
    let crease = src.edge_attrs().get::<f32>(names::CREASE);

    // Dense ids: points, then edges, then faces.
    let mut p_id = vec![INVALID; pc as usize];
    let mut e_id = vec![INVALID; ec as usize];
    let mut f_id = vec![INVALID; fc as usize];
    let mut next: Index = 0;
    for p in 0..pc {
        if src.point_valid(p) {
            p_id[p as usize] = next;
            next += 1;
        }
    }
    for e in 0..ec {
        if src.edge_valid(e) {
            e_id[e as usize] = next;
            next += 1;
        }
    }
    for f in 0..fc {
        if src.face_valid(f) {
            f_id[f as usize] = next;
            next += 1;
        }
    }
    let mut pos = vec![DVec3::ZERO; next as usize];

    let face_pt: Vec<DVec3> =
        (0..fc).into_par_iter().map(|f| if src.face_valid(f) { src.face_centroid(f) } else { DVec3::ZERO }).collect();
    for f in 0..fc {
        if src.face_valid(f) {
            pos[f_id[f as usize] as usize] = face_pt[f as usize];
        }
    }
    let edge_pos: Vec<Option<DVec3>> = (0..ec)
        .into_par_iter()
        .with_min_len(REFINE_GRAIN)
        .map(|e| {
            if !src.edge_valid(e) {
                return None;
            }
            let mid = (src.position(src.edge_point(e, 0)) + src.position(src.edge_point(e, 1))) * 0.5;
            let s = sharpness(src, crease, e);
            let mut smooth = mid;
            if s < 1.0 {
                let c = src.edge_corner(e);
                let faces = (face_pt[src.corner_face(c) as usize] + face_pt[src.corner_face(src.corner_radial(c)) as usize]) * 0.5;
                smooth = (mid + faces) * 0.5;
            }
            Some(mix3(smooth, mid, s))
        })
        .collect();
    for (e, p) in edge_pos.into_iter().enumerate() {
        if let Some(p) = p {
            pos[e_id[e] as usize] = p;
        }
    }
    let point_pos: Vec<Option<DVec3>> = (0..pc)
        .into_par_iter()
        .with_min_len(REFINE_GRAIN)
        .map(|p| {
            if !src.point_valid(p) {
                return None;
            }
            let pp = src.position(p);
            let mut r = DVec3::ZERO;
            let mut fsum = DVec3::ZERO;
            let mut crease_sum = DVec3::ZERO;
            let (mut n, mut nf, mut n_sharp) = (0, 0, 0);
            let mut sharp_sum = 0.0;
            src.for_each_point_edge(p, |e| {
                let q = src.position(src.edge_other_point(e, p));
                r += (pp + q) * 0.5;
                n += 1;
                let s = sharpness(src, crease, e);
                if s > 0.0 {
                    n_sharp += 1;
                    sharp_sum += s;
                    crease_sum += q;
                }
            });
            src.for_each_point_corner(p, |c| {
                fsum += face_pt[src.corner_face(c) as usize];
                nf += 1;
            });
            if n == 0 || nf == 0 {
                return Some(pp);
            }
            let smooth = if n >= 3 && nf == n {
                (fsum / nf as f64 + 2.0 * r / n as f64 + (n - 3) as f64 * pp) / n as f64
            } else {
                pp
            };
            let mut result = smooth;
            let boundary_corner = nf < n && n == 2;
            if n_sharp == 2 && !(boundary_corner && opts.sharp_boundary) {
                result = mix3(smooth, (6.0 * pp + crease_sum) / 8.0, sharp_sum / 2.0);
            } else if n_sharp > 2 || (boundary_corner && opts.sharp_boundary) {
                result = mix3(smooth, pp, if n_sharp > 0 { (sharp_sum / n_sharp as f64).min(1.0) } else { 1.0 });
            }
            Some(result)
        })
        .collect();
    for (p, v) in point_pos.into_iter().enumerate() {
        if let Some(v) = v {
            pos[p_id[p] as usize] = v;
        }
    }

    let has_crease = !crease.is_empty();
    let mut map = map;
    if let Some(built) = build_quads(src, &pos, &p_id, &e_id, &f_id, has_crease, map.as_deref_mut()) {
        return built;
    }

    let mut out = Mesh::new();
    copy_layout(&mut out, src);
    out.reserve(
        next as usize,
        src.num_edges() as usize * 2 + src.num_corners() as usize,
        src.num_corners() as usize,
        src.num_corners() as usize * 4,
    );
    for &p in &pos {
        out.add_point(p);
    }
    point_attributes(&mut out, src, &p_id, &e_id, &f_id);

    let has_corner_attrs = out.corner_attrs().layer_count() > 0;
    if let Some(m) = map.as_deref_mut() {
        m.points = p_id.clone();
        m.face_points = f_id.clone();
        m.edges.clear();
        m.faces.clear();
    }
    for f in 0..fc {
        if f_id[f as usize] == INVALID {
            continue;
        }
        let corners = src.face_corners(f);
        let n = corners.len();
        let avg = vec![1.0 / n as f32; n];
        for i in 0..n {
            let c = corners[i];
            let cp = corners[(i + n - 1) % n];
            let cn = corners[(i + 1) % n];
            let quad = [
                p_id[src.corner_point(c) as usize],
                e_id[src.corner_edge(c) as usize],
                f_id[f as usize],
                e_id[src.corner_edge(cp) as usize],
            ];
            let edge_cap = out.edge_capacity();
            let nf = out.add_face(&quad);
            if nf == INVALID {
                continue;
            }
            out.face_attrs_mut().copy_from(src.face_attrs(), nf as usize, f as usize);
            let oc = out.face_corner(nf);
            // Outer sub-edges come from the original edges; the two inner ones are new and plain.
            let e0 = out.corner_edge(oc);
            let e3 = out.corner_edge(out.corner_prev(oc));
            if let Some(m) = map.as_deref_mut() {
                m.edges.resize(out.edge_capacity() as usize, INVALID);
                m.edges[e0 as usize] = src.corner_edge(c);
                m.edges[e3 as usize] = src.corner_edge(cp);
                m.faces.resize(out.face_capacity() as usize, INVALID);
                m.faces[nf as usize] = f;
            }
            if e0 >= edge_cap {
                out.edge_attrs_mut().copy_from(src.edge_attrs(), e0 as usize, src.corner_edge(c) as usize);
            }
            if e3 >= edge_cap {
                out.edge_attrs_mut().copy_from(src.edge_attrs(), e3 as usize, src.corner_edge(cp) as usize);
            }
            if has_corner_attrs {
                corner_attributes(&mut out, src, oc, c, cp, cn, &corners, &avg);
            }
        }
    }
    if has_crease {
        drop_crease(&mut out);
    }
    out
}

/// Catmull-Clark subdivision. Edge "crease" of 1 or more is sharp (and drops by one per level), fractions blend
/// between smooth and sharp. Corner uvs and colours are interpolated linearly per face.
pub fn subdivide(mesh: &Mesh, opts: &SubdivideOptions, map: Option<&mut SubdivideMap>) -> Mesh {
    let mut cur = mesh.clone();
    let mut map = map;
    for i in 0..opts.levels {
        let Some(m) = map.as_deref_mut() else {
            cur = subdivide_once(&cur, opts, None);
            continue;
        };
        let mut once = SubdivideMap::default();
        cur = subdivide_once(&cur, opts, Some(&mut once));
        if i == 0 {
            *m = once;
            continue;
        }
        // Compose: the cage's points and face centres stay put, and each new piece keeps its parent's cage edge or
        // face.
        for p in m.points.iter_mut().chain(m.face_points.iter_mut()) {
            if *p != INVALID {
                *p = once.points[*p as usize];
            }
        }
        for e in once.edges.iter_mut() {
            if *e != INVALID {
                *e = m.edges[*e as usize];
            }
        }
        for f in once.faces.iter_mut() {
            if *f != INVALID {
                *f = m.faces[*f as usize];
            }
        }
        m.edges = once.edges;
        m.faces = once.faces;
    }
    if !opts.keep_hard_edges && opts.levels > 0 {
        cur.edge_attrs_mut().remove(names::HARD);
        cur.corner_attrs_mut().remove(names::NORMAL);
    }
    if let Some(m) = map {
        m.edges.resize(cur.edge_capacity() as usize, INVALID);
        m.faces.resize(cur.face_capacity() as usize, INVALID);
    }
    cur
}

/// subdivide_once's position rules as weights instead of positions. Must stay in step with it.
fn stencil_once(src: &Mesh, opts: &SubdivideOptions) -> PointStencil {
    let pc = src.point_capacity();
    let ec = src.edge_capacity();
    let fc = src.face_capacity();
    let crease = src.edge_attrs().get::<f32>(names::CREASE);

    type Terms = Vec<(Index, f64)>;
    let add_face = |t: &mut Terms, f: Index, w: f64| {
        let each = w / src.face_size(f) as f64;
        src.for_each_face_corner(f, |c| t.push((src.corner_point(c), each)));
    };

    let mut rows: Vec<Terms> = Vec::with_capacity((src.num_points() + src.num_edges() + src.num_faces()) as usize);
    for p in 0..pc {
        if !src.point_valid(p) {
            continue;
        }
        let (mut n, mut nf, mut n_sharp) = (0, 0, 0);
        let mut sharp_sum = 0.0;
        let mut crease_nbrs: Vec<Index> = Vec::new();
        src.for_each_point_edge(p, |e| {
            n += 1;
            let s = sharpness(src, crease, e);
            if s > 0.0 {
                n_sharp += 1;
                sharp_sum += s;
                crease_nbrs.push(src.edge_other_point(e, p));
            }
        });
        src.for_each_point_corner(p, |_| nf += 1);
        if n == 0 || nf == 0 {
            rows.push(vec![(p, 1.0)]);
            continue;
        }
        // smooth = (F/nf + 2R/n + (n-3)P) / n, R the edge midpoints' sum and F the face points' sum.
        let mut smooth: Terms = Vec::new();
        if n >= 3 && nf == n {
            let inv = 1.0 / n as f64;
            src.for_each_point_corner(p, |c| add_face(&mut smooth, src.corner_face(c), inv / nf as f64));
            src.for_each_point_edge(p, |e| {
                smooth.push((p, inv * inv));
                smooth.push((src.edge_other_point(e, p), inv * inv));
            });
            smooth.push((p, (n - 3) as f64 * inv));
        } else {
            smooth.push((p, 1.0));
        }
        let boundary_corner = nf < n && n == 2;
        let mut mix = 0.0;
        let mut sharp: Terms = Vec::new();
        if n_sharp == 2 && !(boundary_corner && opts.sharp_boundary) {
            mix = sharp_sum / 2.0;
            sharp.push((p, 6.0 / 8.0));
            for &q in &crease_nbrs {
                sharp.push((q, 1.0 / 8.0));
            }
        } else if n_sharp > 2 || (boundary_corner && opts.sharp_boundary) {
            mix = if n_sharp > 0 { (sharp_sum / n_sharp as f64).min(1.0) } else { 1.0 };
            sharp.push((p, 1.0));
        }
        let mut t: Terms = smooth.iter().map(|&(q, w)| (q, w * (1.0 - mix))).collect();
        t.extend(sharp.iter().map(|&(q, w)| (q, w * mix)));
        rows.push(t);
    }
    for e in 0..ec {
        if !src.edge_valid(e) {
            continue;
        }
        let s = sharpness(src, crease, e);
        let mut t: Terms = vec![(src.edge_point(e, 0), 0.5 * s), (src.edge_point(e, 1), 0.5 * s)];
        if s < 1.0 {
            let k = 1.0 - s;
            t.push((src.edge_point(e, 0), 0.25 * k));
            t.push((src.edge_point(e, 1), 0.25 * k));
            let c = src.edge_corner(e);
            add_face(&mut t, src.corner_face(c), 0.25 * k);
            add_face(&mut t, src.corner_face(src.corner_radial(c)), 0.25 * k);
        }
        rows.push(t);
    }
    for f in 0..fc {
        if src.face_valid(f) {
            let mut t = Vec::new();
            add_face(&mut t, f, 1.0);
            rows.push(t);
        }
    }

    let mut out = PointStencil { inputs: pc, ..Default::default() };
    for mut t in rows {
        t.sort_by_key(|a| a.0);
        let mut i = 0;
        while i < t.len() {
            let mut j = i;
            let mut w = 0.0;
            while j < t.len() && t[j].0 == t[i].0 {
                w += t[j].1;
                j += 1;
            }
            if w != 0.0 {
                out.cols.push(t[i].0);
                out.weights.push(w);
            }
            i = j;
        }
        out.offsets.push(out.cols.len() as Index);
    }
    out
}

/// One stencil per level of subdivide(mesh, opts): positions are linear in the cage, so subdivide's points equal the
/// stencils applied in order to the cage's positions. result, when given, receives the subdivided mesh.
pub fn subdivide_stencils(mesh: &Mesh, opts: &SubdivideOptions, result: Option<&mut Mesh>) -> Vec<PointStencil> {
    let mut out = Vec::new();
    let mut cur = mesh.clone();
    for _ in 0..opts.levels {
        out.push(stencil_once(&cur, opts));
        cur = subdivide_once(&cur, opts, None);
    }
    if let Some(r) = result {
        *r = cur;
    }
    out
}

#[cfg(test)]
mod stencil_tests {
    use super::*;

    /// Moving a mesh's points then applying the stencils gives what subdividing the moved mesh gives.
    #[test]
    fn stencils_match_subdividing_a_deformed_mesh() {
        let pts = [DVec3::new(0., 0., 0.), DVec3::new(1., 0., 0.), DVec3::new(1., 1., 0.), DVec3::new(0., 1., 0.), DVec3::new(0., 0., 1.), DVec3::new(1., 0., 1.), DVec3::new(1., 1., 1.), DVec3::new(0., 1., 1.)];
        let m = Mesh::from_polygons(&pts, &[4; 6], &[0, 3, 2, 1, 4, 5, 6, 7, 0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6, 3, 0, 4, 7]);
        for levels in 1..=3 {
            let o = SubdivideOptions { levels, ..Default::default() };
            let shown = subdivide(&m, &o, None);
            let stencils = subdivide_stencils(&m, &o, None);
            let moved = m.with_positions(m.positions().iter().map(|p| DVec3::new(p.x * 2.0 + p.y, p.y - p.z * 0.5, p.z * p.z)).collect());
            let mut points = moved.positions().to_vec();
            for s in &stencils {
                points = s.apply(&points);
            }
            let full = subdivide(&moved, &o, None);
            assert_eq!(shown.positions().len(), points.len());
            for (a, b) in full.positions().iter().zip(&points) {
                assert!((*a - *b).length() < 1e-9, "level {levels}");
            }
        }
    }
}
