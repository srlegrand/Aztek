//! Boundary First Flattening (Sawhney and Crane 2017). Conformal flattening where the
//! boundary is chosen first, either by its scale (lengths) or its curvature (shape), and the interior follows by a
//! harmonic extension. Every step is a solve against one of two Cholesky factorisations made per chart.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};

use aztek_geo::{Index, INVALID};
use glam::DVec2;

use crate::chart::Chart;
use crate::common::{edge_key, TWO_PI};
use crate::sparse::{Ldlt, Triplet};
use crate::unwrap::match_surface_area;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BffBoundary {
    /// Scale 0 along the boundary: boundary lengths kept, least area distortion.
    #[default]
    MinimalArea,
    /// Boundary on a circle.
    Disk,
    /// Straight sides between corners turning equally: four corners give a rectangle.
    Polygon,
    /// Target turning angle per boundary vertex (BffOptions::boundary_values).
    Curvature,
    /// Log scale factor per boundary vertex (BffOptions::boundary_values).
    ScaleFactors,
}

#[derive(Clone, Debug, Default)]
pub struct BffOptions {
    pub boundary: BffBoundary,
    /// Polygon: chart vertices on the boundary to turn at. Fewer than three picks the four sharpest.
    pub corners: Vec<Index>,
    /// Curvature or ScaleFactors values, one per vertex of BffSolver::boundary(), in its order.
    pub boundary_values: Vec<f64>,
    /// Cones placed greedily where the scale distortion is largest. MinimalArea boundary only.
    pub auto_cones: i32,
    /// Interior chart vertices to make cones, as well.
    pub cones: Vec<Index>,
    /// Close the boundary by bending it rather than stretching it, so its edge lengths come out exact.
    pub exact_lengths: bool,
}

#[derive(Clone, Debug, Default)]
pub struct BffResult {
    pub ok: bool,
    /// Cone vertices of the (cut) chart.
    pub cones: Vec<Index>,
    /// Chart edges cut open from the cones to the boundary.
    pub cut_edges: usize,
}

/// Cotan Laplacian (positive semi-definite) and the angle sum at each vertex, from 3D lengths.
struct Geometry {
    laplacian: Vec<Triplet>,
    angle_sum: Vec<f64>,
}

fn geometry(ch: &Chart) -> Geometry {
    let mut g = Geometry { laplacian: Vec::with_capacity(ch.tris.len() * 12), angle_sum: vec![0.0; ch.pos.len()] };
    for t in &ch.tris {
        for k in 0..3 {
            let (i, j, l) = (t[k] as usize, t[(k + 1) % 3] as usize, t[(k + 2) % 3] as usize);
            let (e1, e2) = (ch.pos[j] - ch.pos[i], ch.pos[l] - ch.pos[i]);
            let (cr, d) = (e1.cross(e2).length(), e1.dot(e2));
            g.angle_sum[i] += cr.atan2(d);
            if cr <= 1e-14 * e1.length() * e2.length() {
                continue;
            }
            let w = 0.5 * d / cr;
            g.laplacian.push(Triplet::new(j, j, w));
            g.laplacian.push(Triplet::new(l, l, w));
            g.laplacian.push(Triplet::new(j, l, -w));
            g.laplacian.push(Triplet::new(l, j, -w));
        }
    }
    g
}

/// A Laplacian restricted to its free vertices, factorised; fixed vertices enter the right-hand side.
#[derive(Default)]
struct Restricted {
    free: Vec<i64>,
    /// (free row, fixed vertex, value)
    coupling: Vec<Triplet>,
    n: usize,
    solver: Option<Ldlt>,
    ok: bool,
}

impl Restricted {
    fn new(full: &[Triplet], n: usize, fixed: &[bool]) -> Self {
        let mut free = vec![-1i64; n];
        let mut nf = 0usize;
        for v in 0..n {
            if !fixed[v] {
                free[v] = nf as i64;
                nf += 1;
            }
        }
        let mut coupling = Vec::new();
        let mut ff = Vec::with_capacity(full.len());
        let mut diag = 0.0;
        for t in full {
            let r = free[t.row];
            if r < 0 {
                continue;
            }
            let c = free[t.col];
            if c < 0 {
                coupling.push(Triplet::new(r as usize, t.col, t.value));
            } else {
                ff.push(Triplet::new(r as usize, c as usize, t.value));
                if r == c {
                    diag += t.value;
                }
            }
        }
        // A whisper of regularisation keeps vertices touched only by degenerate triangles solvable.
        let eps = 1e-12 * (diag / nf.max(1) as f64).max(1e-30);
        for i in 0..nf {
            ff.push(Triplet::new(i, i, eps));
        }
        if nf == 0 {
            return Self { free, coupling, n: 0, solver: None, ok: true };
        }
        let solver = Ldlt::new(nf, &ff);
        let ok = solver.ok();
        Self { free, coupling, n: nf, solver: Some(solver), ok }
    }

    /// Solves L_ff x_f = b_f - L_fc x_c. b is per vertex; x holds the fixed values and receives the free ones.
    fn solve(&self, b: &[f64], x: &mut [f64]) -> bool {
        if self.n == 0 {
            return true;
        }
        let mut rhs = vec![0.0; self.n];
        for (v, &f) in self.free.iter().enumerate() {
            if f >= 0 {
                rhs[f as usize] = b[v];
            }
        }
        for t in &self.coupling {
            rhs[t.row] -= t.value * x[t.col];
        }
        let Some(s) = self.solver.as_ref().and_then(|s| s.solve(&rhs)) else { return false };
        for (v, &f) in self.free.iter().enumerate() {
            if f >= 0 {
                x[v] = s[f as usize];
            }
        }
        true
    }
}

/// Log scale factors with u = 0 on the boundary and at the cones, flat everywhere else.
fn cone_scale(ch: &Chart, g: &Geometry, boundary: &[bool], cones: &[Index]) -> Vec<f64> {
    let v_count = ch.pos.len();
    let mut fixed = boundary.to_vec();
    for &c in cones {
        fixed[c as usize] = true;
    }
    let r = Restricted::new(&g.laplacian, v_count, &fixed);
    let mut u = vec![0.0; v_count];
    let b: Vec<f64> = (0..v_count).map(|v| -(TWO_PI - g.angle_sum[v])).collect();
    if !r.ok || !r.solve(&b, &mut u) {
        return Vec::new();
    }
    u
}

fn boundary_mask(ch: &Chart) -> Vec<bool> {
    let mut mask = vec![false; ch.pos.len()];
    for lp in ch.boundary_loops() {
        for v in lp {
            mask[v as usize] = true;
        }
    }
    mask
}

#[derive(Clone, Copy, PartialEq)]
struct Dist(f64);
impl PartialOrd for Dist {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Eq for Dist {}
impl Ord for Dist {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0.total_cmp(&other.0)
    }
}

/// Shortest paths along real (polygon) edges from each cone to the boundary or an earlier path.
fn cone_paths(ch: &Chart, cones: &[Index]) -> Vec<(Index, Index)> {
    let v_count = ch.pos.len();
    let mut adj: Vec<Vec<Index>> = vec![Vec::new(); v_count];
    for f in 0..ch.face_start.len().saturating_sub(1) {
        let (s, e) = (ch.face_start[f], ch.face_start[f + 1]);
        for k in s..e {
            let a = ch.corner_vertex[k as usize];
            let b = ch.corner_vertex[if k + 1 < e { k + 1 } else { s } as usize];
            adj[a as usize].push(b);
            adj[b as usize].push(a);
        }
    }
    let mut reached = boundary_mask(ch);
    let mut out = Vec::new();
    let mut dist = vec![0.0; v_count];
    let mut prev = vec![INVALID; v_count];
    for &cone in cones {
        if reached[cone as usize] {
            continue;
        }
        dist.fill(f64::INFINITY);
        prev.fill(INVALID);
        // Ties pop the lower vertex first, as std::greater on (distance, vertex) pairs does.
        let mut pq = BinaryHeap::new();
        dist[cone as usize] = 0.0;
        pq.push(Reverse((Dist(0.0), cone)));
        let mut hit = INVALID;
        while let Some(Reverse((Dist(d), v))) = pq.pop() {
            if d > dist[v as usize] {
                continue;
            }
            if reached[v as usize] {
                hit = v;
                break;
            }
            for &w in &adj[v as usize] {
                let nd = d + (ch.pos[w as usize] - ch.pos[v as usize]).length();
                if nd < dist[w as usize] {
                    dist[w as usize] = nd;
                    prev[w as usize] = v;
                    pq.push(Reverse((Dist(nd), w)));
                }
            }
        }
        let mut v = hit;
        while v != INVALID && prev[v as usize] != INVALID {
            out.push((prev[v as usize], v));
            reached[prev[v as usize] as usize] = true;
            v = prev[v as usize];
        }
    }
    out
}

/// Up to n boundary indices with the sharpest turning, kept apart along the loop.
fn sharpest_corners(ch: &Chart, lp: &[Index], curvature: &[f64], n: usize) -> Vec<usize> {
    let nb = lp.len();
    let mut at = vec![0.0; nb];
    let mut total = 0.0;
    for i in 0..nb {
        at[i] = total;
        total += (ch.pos[lp[(i + 1) % nb] as usize] - ch.pos[lp[i] as usize]).length();
    }
    let mut order: Vec<usize> = (0..nb).collect();
    order.sort_by(|&a, &b| curvature[lp[b] as usize].partial_cmp(&curvature[lp[a] as usize]).unwrap_or(std::cmp::Ordering::Equal));
    let mut picked: Vec<usize> = Vec::new();
    for spacing in [total / (2.0 * n as f64), total / (4.0 * n as f64), 0.0] {
        for &i in &order {
            if picked.len() >= n {
                break;
            }
            let mut apart = !picked.contains(&i);
            for &p in &picked {
                let d = (at[i] - at[p]).abs();
                apart &= d.min(total - d) > spacing;
            }
            if apart {
                picked.push(i);
            }
        }
        if picked.len() >= n {
            break;
        }
    }
    picked.sort_unstable();
    picked
}

/// The cached operators of one disk chart.
pub struct BffSolver<'a> {
    chart: &'a Chart,
    lp: Vec<Index>,
    curvature: Vec<f64>,
    laplacian: Vec<Triplet>,
    /// Boundary fixed.
    interior: Restricted,
    /// One boundary vertex fixed.
    gauge: Restricted,
    ok: bool,
}

impl<'a> BffSolver<'a> {
    pub fn new(ch: &'a Chart) -> Self {
        let mut s = Self {
            chart: ch,
            lp: Vec::new(),
            curvature: Vec::new(),
            laplacian: Vec::new(),
            interior: Restricted::default(),
            gauge: Restricted::default(),
            ok: false,
        };
        let v_count = ch.pos.len();
        let mut loops = ch.boundary_loops();
        if v_count < 3 || loops.len() != 1 || ch.euler() != 1 {
            return s;
        }
        s.lp = loops.swap_remove(0);
        let mut is_b = vec![false; v_count];
        for &v in &s.lp {
            if is_b[v as usize] {
                return s; // pinched boundary
            }
            is_b[v as usize] = true;
        }
        let g = geometry(ch);
        s.curvature = (0..v_count).map(|v| if is_b[v] { std::f64::consts::PI } else { TWO_PI } - g.angle_sum[v]).collect();
        s.interior = Restricted::new(&g.laplacian, v_count, &is_b);
        let mut one = vec![false; v_count];
        one[s.lp[0] as usize] = true;
        s.gauge = Restricted::new(&g.laplacian, v_count, &one);
        s.laplacian = g.laplacian;
        s.ok = s.interior.ok && s.gauge.ok;
        s
    }

    /// False unless the chart is a disk (one boundary loop, Euler characteristic 1) and the systems factor.
    pub fn ok(&self) -> bool {
        self.ok
    }

    /// The boundary loop, counter-clockwise relative to the triangles.
    pub fn boundary(&self) -> &[Index] {
        &self.lp
    }

    /// Per chart vertex: angle defect inside (2 pi minus the angle sum), turning angle on the boundary (pi minus it).
    pub fn curvature(&self) -> &[f64] {
        &self.curvature
    }

    /// Target turning angles along the boundary for log scale factors u along it (flat interior).
    pub fn dirichlet_to_neumann(&self, ub: &[f64]) -> Vec<f64> {
        let v_count = self.chart.pos.len();
        let mut u = vec![0.0; v_count];
        for (i, &v) in self.lp.iter().enumerate() {
            u[v as usize] = ub[i];
        }
        let b: Vec<f64> = self.curvature.iter().map(|c| -c).collect();
        if !self.interior.solve(&b, &mut u) {
            return Vec::new();
        }
        let mut lu = vec![0.0; v_count];
        for t in &self.laplacian {
            lu[t.row] += t.value * u[t.col];
        }
        self.lp.iter().map(|&v| self.curvature[v as usize] + lu[v as usize]).collect()
    }

    /// Log scale factors along the boundary that give target turning angles k (which must sum to 2 pi).
    pub fn neumann_to_dirichlet(&self, kb: &[f64]) -> Vec<f64> {
        let v_count = self.chart.pos.len();
        let mut b: Vec<f64> = self.curvature.iter().map(|c| -c).collect();
        let mut u = vec![0.0; v_count];
        for (i, &v) in self.lp.iter().enumerate() {
            b[v as usize] = kb[i] - self.curvature[v as usize];
        }
        if !self.gauge.solve(&b, &mut u) {
            return Vec::new();
        }
        self.lp.iter().map(|&v| u[v as usize]).collect()
    }

    /// Lays out the boundary from its scale u and turning k, closes it and extends it harmonically inside.
    pub fn flatten(&self, ub: &[f64], kb: &[f64], exact_lengths: bool) -> Option<Vec<DVec2>> {
        let ch = self.chart;
        let (nb, v_count) = (self.lp.len(), ch.pos.len());
        if ub.len() != nb || kb.len() != nb {
            return None;
        }
        let mut len = vec![0.0; nb];
        let mut phi = vec![0.0; nb];
        for i in 0..nb {
            let (a, b) = (self.lp[i] as usize, self.lp[(i + 1) % nb] as usize);
            len[i] = (0.5 * (ub[i] + ub[(i + 1) % nb])).exp() * (ch.pos[b] - ch.pos[a]).length();
        }
        for i in 1..nb {
            phi[i] = phi[i - 1] + kb[i];
        }

        if exact_lengths {
            // Gauss-Newton on the edge directions: the smallest turn changes that close the loop.
            let total: f64 = len.iter().sum();
            for _ in 0..30 {
                let mut r = DVec2::ZERO;
                let (mut g00, mut g01, mut g11) = (0.0, 0.0, 0.0);
                for i in 0..nb {
                    let (c, s) = (phi[i].cos(), phi[i].sin());
                    r += len[i] * DVec2::new(c, s);
                    g00 += len[i] * len[i] * s * s;
                    g01 -= len[i] * len[i] * s * c;
                    g11 += len[i] * len[i] * c * c;
                }
                if r.length() < 1e-13 * total {
                    break;
                }
                let det = g00 * g11 - g01 * g01;
                if det.abs() < 1e-300 {
                    return None;
                }
                let lam = DVec2::new((g11 * r.x - g01 * r.y) / det, (-g01 * r.x + g00 * r.y) / det);
                for i in 0..nb {
                    phi[i] -= len[i] * (-phi[i].sin() * lam.x + phi[i].cos() * lam.y);
                }
            }
        } else {
            // Smallest length change (weighted by the lengths) that closes the polygon.
            let mut r = DVec2::ZERO;
            let (mut g00, mut g01, mut g11) = (0.0, 0.0, 0.0);
            for i in 0..nb {
                let (c, s) = (phi[i].cos(), phi[i].sin());
                r += len[i] * DVec2::new(c, s);
                g00 += len[i] * c * c;
                g01 += len[i] * c * s;
                g11 += len[i] * s * s;
            }
            let det = g00 * g11 - g01 * g01;
            if det.abs() > 1e-300 {
                let lam = DVec2::new((g11 * r.x - g01 * r.y) / det, (-g01 * r.x + g00 * r.y) / det);
                for i in 0..nb {
                    len[i] -= len[i] * (phi[i].cos() * lam.x + phi[i].sin() * lam.y);
                }
            }
        }

        let (mut x, mut y, zero) = (vec![0.0; v_count], vec![0.0; v_count], vec![0.0; v_count]);
        let mut at = DVec2::ZERO;
        for i in 0..nb {
            x[self.lp[i] as usize] = at.x;
            y[self.lp[i] as usize] = at.y;
            at += len[i] * DVec2::new(phi[i].cos(), phi[i].sin());
        }
        // A conformal map's coordinates are harmonic, so the harmonic extension of its boundary is the map.
        if !self.interior.solve(&zero, &mut x) || !self.interior.solve(&zero, &mut y) {
            return None;
        }
        Some((0..v_count).map(|v| DVec2::new(x[v], y[v])).collect())
    }
}

/// Greedy cone placement: repeatedly the interior vertex with the largest |log scale| given the cones so far.
pub fn place_cones(ch: &Chart, count: i32, fixed_cones: &[Index]) -> Vec<Index> {
    let mut cones = fixed_cones.to_vec();
    if count <= 0 || ch.tris.is_empty() {
        return cones;
    }
    let g = geometry(ch);
    let boundary = boundary_mask(ch);
    for _ in 0..count {
        let u = cone_scale(ch, &g, &boundary, &cones);
        if u.is_empty() {
            break;
        }
        let mut best = INVALID;
        let mut best_u = 1e-9;
        for v in 0..ch.vertex_count() {
            if !boundary[v as usize] && !cones.contains(&v) && u[v as usize].abs() > best_u {
                best_u = u[v as usize].abs();
                best = v;
            }
        }
        if best == INVALID {
            break;
        }
        cones.push(best);
    }
    cones
}

/// Cuts the chart open along the given chart edges (pairs of vertices). Returns the source vertex per vertex.
pub fn cut_chart(ch: &mut Chart, edges: &[(Index, Index)]) -> Vec<Index> {
    let v0 = ch.vertex_count();
    let mut origin: Vec<Index> = (0..v0).collect();
    if edges.is_empty() {
        return origin;
    }
    let mut cut = HashSet::new();
    let mut touched = Vec::new();
    for &(a, b) in edges {
        cut.insert(edge_key(a, b));
        touched.push(a);
        touched.push(b);
    }
    touched.sort_unstable();
    touched.dedup();

    let mut incident: Vec<Vec<(usize, usize)>> = vec![Vec::new(); v0 as usize];
    for (i, t) in ch.tris.iter().enumerate() {
        for k in 0..3 {
            incident[t[k] as usize].push((i, k));
        }
    }

    for &v in &touched {
        let list = &incident[v as usize];
        if list.is_empty() {
            continue;
        }
        let mut parent: Vec<usize> = (0..list.len()).collect();
        fn find(parent: &mut [usize], mut x: usize) -> usize {
            while parent[x] != x {
                parent[x] = parent[parent[x]];
                x = parent[x];
            }
            x
        }
        let mut seen: HashMap<Index, usize> = HashMap::new();
        for (e, &(ti, k)) in list.iter().enumerate() {
            let t = ch.tris[ti];
            for s in [1, 2] {
                let w = t[(k + s) % 3];
                if cut.contains(&edge_key(v, w)) {
                    continue;
                }
                match seen.get(&w) {
                    None => {
                        seen.insert(w, e);
                    }
                    Some(&other) => {
                        let (a, b) = (find(&mut parent, e), find(&mut parent, other));
                        parent[a] = b;
                    }
                }
            }
        }
        let mut group_vertex: HashMap<usize, Index> = HashMap::new();
        let first = find(&mut parent, 0);
        group_vertex.insert(first, v);
        for (e, &(ti, k)) in list.iter().enumerate() {
            let root = find(&mut parent, e);
            let w = *group_vertex.entry(root).or_insert_with(|| {
                let w = ch.pos.len() as Index;
                ch.pos.push(ch.pos[v as usize]);
                ch.uv.push(ch.uv[v as usize]);
                ch.pinned.push(ch.pinned[v as usize]);
                ch.vertex_point.push(ch.vertex_point[v as usize]);
                origin.push(v);
                w
            });
            ch.tris[ti][k] = w;
        }
    }

    let mut face_vertex: HashMap<u64, Index> = HashMap::new();
    for (i, t) in ch.tris.iter().enumerate() {
        for &w in t {
            face_vertex.insert(((ch.tri_face[i] as u64) << 32) | origin[w as usize] as u64, w);
        }
    }
    for f in 0..ch.face_start.len().saturating_sub(1) {
        for k in ch.face_start[f]..ch.face_start[f + 1] {
            if let Some(&w) = face_vertex.get(&(((ch.faces[f] as u64) << 32) | ch.corner_vertex[k as usize] as u64)) {
                ch.corner_vertex[k as usize] = w;
            }
        }
    }
    origin
}

/// Flattens a disk chart in place. Without pins it comes out at 3D scale with its lower corner at the origin. Cones cut
/// the chart open along shortest paths to the boundary, which changes its vertices (corner_vertex follows).
pub fn bff(ch: &mut Chart, opts: &BffOptions) -> BffResult {
    let mut r = BffResult::default();
    if ch.tris.is_empty() {
        return r;
    }

    let mut cone_u = Vec::new();
    if opts.boundary == BffBoundary::MinimalArea && (opts.auto_cones > 0 || !opts.cones.is_empty()) {
        let boundary = boundary_mask(ch);
        let fixed: Vec<Index> = opts.cones.iter().copied().filter(|&c| c < ch.vertex_count() && !boundary[c as usize]).collect();
        let cones = place_cones(ch, opts.auto_cones, &fixed);
        if !cones.is_empty() {
            let g = geometry(ch);
            cone_u = cone_scale(ch, &g, &boundary, &cones);
            if cone_u.is_empty() {
                return r;
            }
            let paths = cone_paths(ch, &cones);
            let origin = cut_chart(ch, &paths);
            cone_u = origin.iter().map(|&o| cone_u[o as usize]).collect();
            r.cones = cones;
            r.cut_edges = paths.len();
        }
    }

    let uv = {
        let s = BffSolver::new(ch);
        if !s.ok() {
            return r;
        }
        let lp = s.boundary().to_vec();
        let nb = lp.len();
        let mut u = vec![0.0; nb];
        let mut k = Vec::new();
        let lengths_for = |uu: &[f64]| -> Vec<f64> {
            (0..nb)
                .map(|i| {
                    (0.5 * (uu[i] + uu[(i + 1) % nb])).exp() * (ch.pos[lp[(i + 1) % nb] as usize] - ch.pos[lp[i] as usize]).length()
                })
                .collect()
        };
        match opts.boundary {
            BffBoundary::MinimalArea => {
                if !cone_u.is_empty() {
                    for i in 0..nb {
                        u[i] = cone_u[lp[i] as usize];
                    }
                }
                k = s.dirichlet_to_neumann(&u);
            }
            BffBoundary::ScaleFactors => {
                if opts.boundary_values.len() != nb {
                    return r;
                }
                u = opts.boundary_values.clone();
                k = s.dirichlet_to_neumann(&u);
            }
            BffBoundary::Curvature => {
                if opts.boundary_values.len() != nb {
                    return r;
                }
                k = opts.boundary_values.clone();
                let shift = (TWO_PI - k.iter().sum::<f64>()) / nb as f64;
                for x in k.iter_mut() {
                    *x += shift;
                }
                u = s.neumann_to_dirichlet(&k);
            }
            BffBoundary::Disk => {
                // Curvature proportional to length makes a circle; a few rounds settle the lengths it implies.
                let mut it = 0;
                while it < 4 && u.len() == nb {
                    let l = lengths_for(&u);
                    let total: f64 = l.iter().sum();
                    k = (0..nb).map(|i| TWO_PI * 0.5 * (l[(i + nb - 1) % nb] + l[i]) / total).collect();
                    u = s.neumann_to_dirichlet(&k);
                    it += 1;
                }
            }
            BffBoundary::Polygon => {
                let mut corners: Vec<usize> = opts.corners.iter().filter_map(|c| lp.iter().position(|v| v == c)).collect();
                corners.sort_unstable();
                corners.dedup();
                if corners.len() < 3 {
                    corners = sharpest_corners(ch, &lp, s.curvature(), 4.min(nb));
                }
                if corners.len() < 3 {
                    return r;
                }
                k = vec![0.0; nb];
                for &c in &corners {
                    k[c] = TWO_PI / corners.len() as f64;
                }
                u = s.neumann_to_dirichlet(&k);
            }
        }
        if k.len() != nb || u.len() != nb {
            return r;
        }
        let exact = opts.exact_lengths && opts.boundary != BffBoundary::Polygon && opts.boundary != BffBoundary::Disk;
        let Some(uv) = s.flatten(&u, &k, exact) else { return r };
        uv
    };
    if uv.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return r;
    }
    ch.uv = uv;
    if ch.area_uv() < 0.0 {
        for p in ch.uv.iter_mut() {
            p.x = -p.x;
        }
    }
    let lengths_matter = matches!(opts.boundary, BffBoundary::MinimalArea | BffBoundary::ScaleFactors);
    if !lengths_matter {
        match_surface_area(ch);
    }
    let lo = ch.uv_min();
    for p in ch.uv.iter_mut() {
        *p -= lo;
    }
    r.ok = true;
    r
}
