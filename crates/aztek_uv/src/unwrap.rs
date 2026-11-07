//! LSCM, Tutte, SLIM and the chart unwrap that picks between them.

use std::sync::atomic::{AtomicUsize, Ordering};

use aztek_geo::{Index, INVALID};
use glam::DVec2;
use rayon::prelude::*;

use crate::bff::{bff, BffOptions};
use crate::chart::Chart;
use crate::common::{flip_root, jacobian, signed_area2, svd2, tri_frame, TriFrame, TWO_PI};
use crate::sparse::{Ldlt, Triplet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum UnwrapMethod {
    /// LSCM only: angle preserving, fast, area can drift on curved islands.
    Conformal,
    /// LSCM start refined by SLIM (symmetric Dirichlet): low area and angle distortion, no flips.
    #[default]
    Slim,
    /// Boundary First Flattening: conformal with boundary lengths kept; SLIM when it folds or the chart is pinned.
    Bff,
}

#[derive(Clone, Debug)]
pub struct UnwrapOptions {
    pub method: UnwrapMethod,
    /// SLIM iterations; it stops early once the energy settles.
    pub iterations: i32,
    /// Keep pinned vertices where they are. Two or more pins also fix the island's scale and placement.
    pub use_pins: bool,
}

impl Default for UnwrapOptions {
    fn default() -> Self {
        Self { method: UnwrapMethod::Slim, iterations: 12, use_pins: true }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UnwrapResult {
    pub ok: bool,
    /// LSCM folded, so SLIM started from a Tutte embedding.
    pub used_tutte: bool,
    pub iterations: i32,
    /// Symmetric Dirichlet energy per area minus its isometric minimum (0 = no distortion).
    pub distortion: f64,
}

fn frames(ch: &Chart, scale: f64) -> Vec<TriFrame> {
    ch.tris
        .iter()
        .map(|t| tri_frame(ch.pos[t[0] as usize] * scale, ch.pos[t[1] as usize] * scale, ch.pos[t[2] as usize] * scale))
        .collect()
}

fn dirichlet(s1: f64, s2: f64) -> f64 {
    s1 * s1 + 1.0 / (s1 * s1) + s2 * s2 + 1.0 / (s2 * s2)
}

fn distortions(ch: &Chart, fr: &[TriFrame], uv: &[DVec2]) -> Vec<f64> {
    let mut out = vec![0.0; ch.tris.len()];
    for (i, t) in ch.tris.iter().enumerate() {
        if fr[i].area <= 0.0 {
            continue;
        }
        let j = jacobian(&fr[i], uv[t[0] as usize], uv[t[1] as usize], uv[t[2] as usize]);
        if j.det() <= 0.0 {
            out[i] = f64::INFINITY;
            continue;
        }
        let s = svd2(&j);
        out[i] = dirichlet(s.s1, s.s2);
    }
    out
}

fn farthest_pair(ch: &Chart) -> (Index, Index) {
    let (mut lo, mut hi) = (glam::DVec3::splat(f64::MAX), glam::DVec3::splat(-f64::MAX));
    for &p in &ch.pos {
        lo = lo.min(p);
        hi = hi.max(p);
    }
    let ext = hi - lo;
    let axis = if ext.x >= ext.y && ext.x >= ext.z {
        0
    } else if ext.y >= ext.z {
        1
    } else {
        2
    };
    let (mut a, mut b) = (0 as Index, 0 as Index);
    for v in 0..ch.vertex_count() {
        if ch.pos[v as usize][axis] < ch.pos[a as usize][axis] {
            a = v;
        }
        if ch.pos[v as usize][axis] > ch.pos[b as usize][axis] {
            b = v;
        }
    }
    // One sweep refines the pair towards the true diameter.
    for _ in 0..2 {
        let mut far = a;
        let mut best = -1.0;
        for v in 0..ch.vertex_count() {
            let d = (ch.pos[v as usize] - ch.pos[a as usize]).length();
            if d > best {
                best = d;
                far = v;
            }
        }
        b = far;
        std::mem::swap(&mut a, &mut b);
    }
    (a.min(b), a.max(b))
}

/// Solves M x = rhs over free dofs; dofs with fixed[i] keep x[i]. M is assembled from triplets over all dofs.
fn solve_constrained(trips: &[Triplet], n: usize, fixed: &[bool], x: &mut [f64], rhs_full: Option<&[f64]>) -> bool {
    let mut free_id = vec![INVALID; n];
    let mut nf = 0usize;
    for i in 0..n {
        if !fixed[i] {
            free_id[i] = nf as Index;
            nf += 1;
        }
    }
    if nf == 0 {
        return true;
    }
    let mut ff = Vec::with_capacity(trips.len() + nf);
    let mut rhs = vec![0.0; nf];
    if let Some(full) = rhs_full {
        for i in 0..n {
            if free_id[i] != INVALID {
                rhs[free_id[i] as usize] = full[i];
            }
        }
    }
    let mut diag = 0.0;
    for t in trips {
        let (r, c) = (free_id[t.row], free_id[t.col]);
        if r == INVALID {
            continue;
        }
        if c == INVALID {
            rhs[r as usize] -= t.value * x[t.col];
        } else {
            ff.push(Triplet::new(r as usize, c as usize, t.value));
            if r == c {
                diag += t.value;
            }
        }
    }
    // A whisper of regularisation keeps vertices only touched by degenerate triangles solvable.
    let eps = 1e-10 * (diag / nf as f64).max(1e-30);
    for i in 0..nf {
        ff.push(Triplet::new(i, i, eps));
    }
    let Some(sol) = Ldlt::new(nf, &ff).solve(&rhs) else { return false };
    for i in 0..n {
        if free_id[i] != INVALID {
            x[i] = sol[free_id[i] as usize];
        }
    }
    true
}

/// BFF with lengths kept along the boundary; false (chart untouched) when it fails or folds.
fn flatten_bff(ch: &mut Chart) -> bool {
    let mut trial = ch.clone();
    if !bff(&mut trial, &BffOptions::default()).ok || folded_triangles(&trial) > 0 {
        return false;
    }
    ch.uv = trial.uv;
    true
}

/// Least squares conformal map. Uses the chart's pinned vertices when there are two or more, else pins the two vertices
/// farthest apart. False when the system is singular.
pub fn lscm(ch: &mut Chart, use_pins: bool) -> bool {
    let v_count = ch.vertex_count() as usize;
    if v_count < 3 || ch.tris.is_empty() {
        return false;
    }
    let fr = frames(ch, 1.0);
    let mut trips = Vec::with_capacity(ch.tris.len() * 36);
    for (i, t) in ch.tris.iter().enumerate() {
        let f = &fr[i];
        if f.area <= 0.0 {
            continue;
        }
        // du/dx - dv/dy and du/dy + dv/dx, over dofs u0 u1 u2 v0 v1 v2.
        let r1 = [f.g[0].x, f.g[1].x, f.g[2].x, -f.g[0].y, -f.g[1].y, -f.g[2].y];
        let r2 = [f.g[0].y, f.g[1].y, f.g[2].y, f.g[0].x, f.g[1].x, f.g[2].x];
        let mut dof = [0usize; 6];
        for j in 0..3 {
            dof[j] = t[j] as usize;
            dof[3 + j] = v_count + t[j] as usize;
        }
        for r in 0..6 {
            for c in 0..6 {
                trips.push(Triplet::new(dof[r], dof[c], f.area * (r1[r] * r1[c] + r2[r] * r2[c])));
            }
        }
    }

    let mut fixed = vec![false; 2 * v_count];
    let mut x = vec![0.0; 2 * v_count];
    let pins = if use_pins { ch.pinned.iter().filter(|&&p| p).count() } else { 0 };
    if pins >= 2 {
        for v in 0..v_count {
            if ch.pinned[v] {
                fixed[v] = true;
                fixed[v_count + v] = true;
                x[v] = ch.uv[v].x;
                x[v_count + v] = ch.uv[v].y;
            }
        }
    } else {
        let (a, b) = farthest_pair(ch);
        if a == b {
            return false;
        }
        let (a, b) = (a as usize, b as usize);
        fixed[a] = true;
        fixed[v_count + a] = true;
        fixed[b] = true;
        fixed[v_count + b] = true;
        x[b] = (ch.pos[b] - ch.pos[a]).length();
    }
    if !solve_constrained(&trips, 2 * v_count, &fixed, &mut x, None) {
        return false;
    }
    for v in 0..v_count {
        ch.uv[v] = DVec2::new(x[v], x[v_count + v]);
    }
    true
}

/// Tutte embedding: the outer boundary on a circle, interior at neighbour averages. Never folds.
pub fn tutte(ch: &mut Chart) -> bool {
    let v_count = ch.vertex_count() as usize;
    let loops = ch.boundary_loops();
    if loops.is_empty() || loops[0].len() < 3 {
        return false;
    }
    let outer = &loops[0];
    let mut total = 0.0;
    let mut along = vec![0.0; outer.len()];
    for i in 0..outer.len() {
        along[i] = total;
        total += (ch.pos[outer[(i + 1) % outer.len()] as usize] - ch.pos[outer[i] as usize]).length();
    }
    if total <= 0.0 {
        return false;
    }
    let radius = total / TWO_PI;
    let mut fixed = vec![false; v_count];
    let (mut xu, mut xv) = (vec![0.0; v_count], vec![0.0; v_count]);
    for (i, &v) in outer.iter().enumerate() {
        let a = along[i] / total * TWO_PI;
        fixed[v as usize] = true;
        xu[v as usize] = radius * a.cos();
        xv[v as usize] = radius * a.sin();
    }
    let mut edges = Vec::new();
    for t in &ch.tris {
        for i in 0..3 {
            let (a, b) = (t[i], t[(i + 1) % 3]);
            edges.push(((a.min(b) as u64) << 32) | a.max(b) as u64);
        }
    }
    edges.sort_unstable();
    edges.dedup();
    let mut trips = Vec::with_capacity(edges.len() * 4);
    for e in edges {
        let (a, b) = ((e >> 32) as usize, (e & 0xffff_ffff) as usize);
        trips.push(Triplet::new(a, a, 1.0));
        trips.push(Triplet::new(b, b, 1.0));
        trips.push(Triplet::new(a, b, -1.0));
        trips.push(Triplet::new(b, a, -1.0));
    }
    if !solve_constrained(&trips, v_count, &fixed, &mut xu, None) || !solve_constrained(&trips, v_count, &fixed, &mut xv, None) {
        return false;
    }
    for v in 0..v_count {
        ch.uv[v] = DVec2::new(xu[v], xv[v]);
    }
    true
}

/// Triangles whose uv winding disagrees with the chart's overall orientation, or are degenerate.
pub fn folded_triangles(ch: &Chart) -> usize {
    let sign = if ch.area_uv() < 0.0 { -1.0 } else { 1.0 };
    let fr = frames(ch, 1.0);
    ch.tris
        .iter()
        .enumerate()
        .filter(|(i, t)| {
            fr[*i].area > 0.0 && sign * signed_area2(ch.uv[t[0] as usize], ch.uv[t[1] as usize], ch.uv[t[2] as usize]) <= 0.0
        })
        .count()
}

/// Uniform scale about the origin so uv area equals 3D area.
pub fn match_surface_area(ch: &mut Chart) {
    let (a3, au) = (ch.area_3d(), ch.area_uv().abs());
    if a3 <= 0.0 || au <= 0.0 {
        return;
    }
    let s = (a3 / au).sqrt();
    for p in ch.uv.iter_mut() {
        *p *= s;
    }
}

/// Symmetric Dirichlet distortion of the chart's current uvs after normalising scale (0 = isometric).
pub fn chart_distortion(ch: &Chart) -> f64 {
    let (a3, au) = (ch.area_3d(), ch.area_uv().abs());
    if a3 <= 0.0 || au <= 0.0 {
        return f64::INFINITY;
    }
    let fr = frames(ch, 1.0 / a3.sqrt());
    let s = 1.0 / au.sqrt();
    let flip = if ch.area_uv() < 0.0 { -1.0 } else { 1.0 };
    let uv: Vec<DVec2> = ch.uv.iter().map(|p| DVec2::new(p.x * flip, p.y) * s).collect();
    let d = distortions(ch, &fr, &uv);
    let (mut e, mut w) = (0.0, 0.0);
    for i in 0..d.len() {
        e += fr[i].area * d[i];
        w += fr[i].area;
    }
    if w > 0.0 {
        e / w / 4.0 - 1.0
    } else {
        0.0
    }
}

/// Scalable Locally Injective Maps (Rabinovich et al. 2017) minimising symmetric Dirichlet energy from a fold-free
/// start. Returns the iterations run, or -1 when the start folds.
pub fn slim(ch: &mut Chart, iterations: i32, use_pins: bool) -> i32 {
    let v_count = ch.vertex_count() as usize;
    let t_count = ch.tris.len();
    if v_count < 3 || t_count == 0 {
        return 0;
    }
    let (a3, au) = (ch.area_3d(), ch.area_uv().abs());
    if a3 <= 0.0 || au <= 0.0 {
        return -1;
    }
    let pinned = use_pins && ch.pinned.iter().any(|&p| p);

    // Work at unit area so energies, weights and tolerances are scale free.
    let fr = frames(ch, 1.0 / a3.sqrt());
    let su = 1.0 / au.sqrt();
    let mirror = ch.area_uv() < 0.0 && !pinned;
    let mut x = vec![0.0; 2 * v_count];
    for v in 0..v_count {
        x[v] = ch.uv[v].x * su * if mirror { -1.0 } else { 1.0 };
        x[v_count + v] = ch.uv[v].y * su;
    }
    let pin_x = x.clone();

    let uv_at = |s: &[f64], v: Index| DVec2::new(s[v as usize], s[v_count + v as usize]);
    for (i, t) in ch.tris.iter().enumerate() {
        if fr[i].area > 0.0 && signed_area2(uv_at(&x, t[0]), uv_at(&x, t[1]), uv_at(&x, t[2])) <= 0.0 {
            return -1;
        }
    }

    let pin_weight = 1e5;
    let energy = |s: &[f64]| -> f64 {
        let mut e = 0.0;
        for (i, t) in ch.tris.iter().enumerate() {
            if fr[i].area <= 0.0 {
                continue;
            }
            let j = jacobian(&fr[i], uv_at(s, t[0]), uv_at(s, t[1]), uv_at(s, t[2]));
            if j.det() <= 0.0 {
                return f64::INFINITY;
            }
            let d = svd2(&j);
            e += fr[i].area * dirichlet(d.s1, d.s2);
        }
        if pinned {
            for v in 0..v_count {
                if ch.pinned[v] {
                    let (du, dv) = (s[v] - pin_x[v], s[v_count + v] - pin_x[v_count + v]);
                    e += pin_weight * (du * du + dv * dv);
                }
            }
        }
        e
    };

    // Big charts drop the anisotropic coupling between u and v: half the unknowns per solve and one factorisation
    // serves both, at the price of a few more iterations.
    let coupled = v_count <= 10000;
    let bs = if coupled { 6 } else { 3 };
    let n = if coupled { 2 * v_count } else { v_count };
    let dof = |t: &[Index; 3], k: usize| if k < 3 { t[k] as usize } else { v_count + t[k - 3] as usize };
    let bb = bs * bs;

    // Fixed sparsity: one bs x bs block per triangle, plus the diagonal.
    let assemble = |local: &[f64], diag: &[f64]| -> Vec<Triplet> {
        let mut trips = Vec::with_capacity(t_count * bb + n);
        for (i, t) in ch.tris.iter().enumerate() {
            for r in 0..bs {
                for c in 0..bs {
                    trips.push(Triplet::new(dof(t, r), dof(t, c), local[i * bb + r * bs + c]));
                }
            }
        }
        for (i, &d) in diag.iter().enumerate() {
            trips.push(Triplet::new(i, i, d));
        }
        trips
    };
    let mut solver = Ldlt::analyze(n, &assemble(&vec![0.0; t_count * bb], &vec![0.0; n]));

    let mut e0 = energy(&x);
    let mut done = 0;
    let mut local = vec![0.0; t_count * bb];
    let mut local_rhs = vec![0.0; t_count * 6];
    for _ in 0..iterations {
        // Local step: per-triangle proxy rotation and weights.
        local.par_chunks_mut(bb).zip(local_rhs.par_chunks_mut(6)).enumerate().for_each(|(i, (k, b))| {
            k.fill(0.0);
            b.fill(0.0);
            let f = &fr[i];
            if f.area <= 0.0 {
                return;
            }
            let t = &ch.tris[i];
            let j = jacobian(f, uv_at(&x, t[0]), uv_at(&x, t[1]), uv_at(&x, t[2]));
            let s = svd2(&j);
            let (s1, s2) = (s.s1.max(1e-8), s.s2.max(1e-8));
            let weight = |sv: f64| if (sv - 1.0).abs() < 1e-6 { 2.0 } else { ((sv - 1.0 / (sv * sv * sv)) / (sv - 1.0)).sqrt() };
            let (w1, w2) = (weight(s1), weight(s2));
            let ang = s.phi + s.theta;
            let rot = [[ang.cos(), -ang.sin()], [ang.sin(), ang.cos()]];
            if !coupled {
                // Isotropic proxy: w^2 |grad u - R0|^2 + w^2 |grad v - R1|^2.
                let w = f.area * 0.5 * (w1 * w1 + w2 * w2);
                for p in 0..3 {
                    b[p] += w * (f.g[p].x * rot[0][0] + f.g[p].y * rot[0][1]);
                    b[3 + p] += w * (f.g[p].x * rot[1][0] + f.g[p].y * rot[1][1]);
                    for q in 0..3 {
                        k[p * 3 + q] = w * f.g[p].dot(f.g[q]);
                    }
                }
                return;
            }
            let (cp, sp) = (s.phi.cos(), s.phi.sin());
            // W = U diag(w) U^T with U = Rot(phi); R = Rot(phi + theta).
            let wm = [[w1 * cp * cp + w2 * sp * sp, (w1 - w2) * cp * sp], [(w1 - w2) * cp * sp, w1 * sp * sp + w2 * cp * cp]];
            for r in 0..2 {
                for c in 0..2 {
                    let mut a = [0.0; 6];
                    for kk in 0..3 {
                        a[kk] = wm[r][0] * f.g[kk][c];
                        a[3 + kk] = wm[r][1] * f.g[kk][c];
                    }
                    let bv = wm[r][0] * rot[0][c] + wm[r][1] * rot[1][c];
                    for p in 0..6 {
                        b[p] += f.area * bv * a[p];
                        for q in 0..6 {
                            k[p * 6 + q] += f.area * a[p] * a[q];
                        }
                    }
                }
            }
        });

        // Global step.
        let mut diag = vec![0.0; n];
        let mut rhs = vec![0.0; 2 * v_count];
        for (i, t) in ch.tris.iter().enumerate() {
            for r in 0..bs {
                diag[dof(t, r)] += local[i * bb + r * bs + r];
            }
            for p in 0..6 {
                rhs[if p < 3 { t[p] as usize } else { v_count + t[p - 3] as usize }] += local_rhs[i * 6 + p];
            }
        }
        let trace: f64 = diag.iter().sum();
        // Proximal term: removes the translation null space without biasing the shape.
        let prox = 1e-8 * trace / n as f64;
        let mut extra = vec![prox; n];
        for i in 0..2 * v_count {
            rhs[i] += prox * x[i];
        }
        if pinned {
            for v in 0..v_count {
                if ch.pinned[v] {
                    extra[v] += pin_weight;
                    if coupled {
                        extra[v_count + v] += pin_weight;
                    }
                    rhs[v] += pin_weight * pin_x[v];
                    rhs[v_count + v] += pin_weight * pin_x[v_count + v];
                }
            }
        }
        if !solver.factorize(&assemble(&local, &extra)) {
            break;
        }
        let target = if coupled {
            solver.solve(&rhs)
        } else {
            match (solver.solve(&rhs[..v_count]), solver.solve(&rhs[v_count..])) {
                (Some(mut a), Some(b)) => {
                    a.extend(b);
                    Some(a)
                }
                _ => None,
            }
        };
        let Some(target) = target else { break };
        let dir: Vec<f64> = target.iter().zip(&x).map(|(t, x)| t - x).collect();

        // Largest step before any triangle flips.
        let mut t_max = 1.0f64;
        for (i, t) in ch.tris.iter().enumerate() {
            if fr[i].area <= 0.0 {
                continue;
            }
            let j0 = jacobian(&fr[i], uv_at(&x, t[0]), uv_at(&x, t[1]), uv_at(&x, t[2]));
            let jd = jacobian(&fr[i], uv_at(&dir, t[0]), uv_at(&dir, t[1]), uv_at(&dir, t[2]));
            t_max = t_max.min(0.8 * flip_root(&j0, &jd, 1e-14));
        }
        let mut step = t_max.min(1.0);
        let step_to = |step: f64| -> Vec<f64> { x.iter().zip(&dir).map(|(x, d)| x + step * d).collect() };
        let mut trial = step_to(step);
        let mut e1 = energy(&trial);
        while e1 > e0 && step > 1e-6 {
            step *= 0.5;
            trial = step_to(step);
            e1 = energy(&trial);
        }
        if !(e1 <= e0) {
            break;
        }
        x = trial;
        done += 1;
        let settled = (e0 - e1) < 2e-4 * e0;
        e0 = e1;
        if settled {
            break;
        }
    }

    // Back to 3D scale, or to the pins' own frame.
    let back = if pinned { 1.0 / su } else { a3.sqrt() };
    for v in 0..v_count {
        ch.uv[v] = DVec2::new(x[v], x[v_count + v]) * back;
    }
    if pinned {
        for v in 0..v_count {
            if ch.pinned[v] {
                ch.uv[v] = DVec2::new(pin_x[v], pin_x[v_count + v]) / su;
            }
        }
    }
    done
}

/// Flattens one chart, which must be a disk (one boundary loop). Without pins the uvs come out at 3D scale (uv area
/// equals surface area) with the bounding box's lower corner at the origin.
pub fn unwrap_chart(ch: &mut Chart, opts: &UnwrapOptions) -> UnwrapResult {
    let mut r = UnwrapResult::default();
    if ch.tris.is_empty() {
        return r;
    }
    let pins = if opts.use_pins { ch.pinned.iter().filter(|&&p| p).count() } else { 0 };
    let pin_uv: Vec<DVec2> = (0..ch.pinned.len()).filter(|&v| ch.pinned[v]).map(|v| ch.uv[v]).collect();

    let via_bff = opts.method == UnwrapMethod::Bff && pins < 2 && flatten_bff(ch);
    let mut ok = via_bff || lscm(ch, opts.use_pins);
    if !via_bff && ok && pins < 2 {
        // LSCM pins two points arbitrarily; mirror if it came out inverted and fix the scale.
        if ch.area_uv() < 0.0 {
            for p in ch.uv.iter_mut() {
                p.x = -p.x;
            }
        }
        match_surface_area(ch);
    }
    if !via_bff && opts.method != UnwrapMethod::Conformal {
        if (!ok || folded_triangles(ch) > 0) && tutte(ch) {
            match_surface_area(ch);
            ok = true;
            r.used_tutte = true;
        }
        if ok {
            let n = slim(ch, opts.iterations, opts.use_pins && pins >= 1);
            r.iterations = n.max(0);
        }
    }
    if !ok {
        return r;
    }

    if pins == 1 {
        let mut offset = DVec2::ZERO;
        for v in 0..ch.pinned.len() {
            if ch.pinned[v] {
                offset = pin_uv[0] - ch.uv[v];
            }
        }
        for p in ch.uv.iter_mut() {
            *p += offset;
        }
    } else if pins == 0 {
        let lo = ch.uv_min();
        for p in ch.uv.iter_mut() {
            *p -= lo;
        }
    }
    r.ok = true;
    r.distortion = chart_distortion(ch);
    r
}

/// All charts, in parallel. Returns how many succeeded.
pub fn unwrap_charts(charts: &mut [Chart], opts: &UnwrapOptions) -> usize {
    let ok = AtomicUsize::new(0);
    // Big charts first so the pool does not finish on one straggler.
    let mut refs: Vec<&mut Chart> = charts.iter_mut().collect();
    refs.sort_by(|a, b| b.tris.len().cmp(&a.tris.len()));
    refs.into_par_iter().with_max_len(1).for_each(|ch| {
        if unwrap_chart(ch, opts).ok {
            ok.fetch_add(1, Ordering::Relaxed);
        }
    });
    ok.into_inner()
}
