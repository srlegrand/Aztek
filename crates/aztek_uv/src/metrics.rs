//! Distortion, overlaps and texel density.

use aztek_geo::triangulate::{triangulate_face, TriangulateOptions};
use aztek_geo::{Index, Mesh};
use glam::{DMat3, DVec2, DVec3, IVec2};
use rayon::prelude::*;

use crate::chart::{build_charts, island_faces, live_faces, uv_layer, Chart, ChartOptions, IslandSource};
use crate::common::Tri2;

/// Per-face distortion relative to the face's island, so packing scale does not count.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FaceDistortion {
    /// log2 of the face's uv-to-3D area ratio over its island's: 0 even, +1 twice as big in uv.
    pub area: f32,
    /// Anisotropy: largest over smallest stretch, minus 1. 0 for a conformal (shape kept) face.
    pub angle: f32,
    /// Wound against its island.
    pub folded: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DistortionSummary {
    /// Area weighted mean of |area|.
    pub mean_area: f64,
    pub max_area: f64,
    pub mean_angle: f64,
    pub max_angle: f64,
    pub folded: usize,
}

/// One number per face for display: 0 perfect, towards 1 badly stretched.
pub fn distortion_value(d: &FaceDistortion) -> f32 {
    let v = d.area.abs().max(d.angle);
    if d.folded {
        1.0
    } else {
        1.0 - (-2.0 * v).exp()
    }
}

fn separated(a: &Tri2, b: &Tri2, eps: f64) -> bool {
    for t in [a, b] {
        for i in 0..3 {
            let e = t[(i + 1) % 3] - t[i];
            let len = e.length();
            if len <= 0.0 {
                continue;
            }
            let n = DVec2::new(-e.y / len, e.x / len);
            let (mut a0, mut a1, mut b0, mut b1) = (f64::MAX, -f64::MAX, f64::MAX, -f64::MAX);
            for k in 0..3 {
                let (pa, pb) = (a[k].dot(n), b[k].dot(n));
                a0 = a0.min(pa);
                a1 = a1.max(pa);
                b0 = b0.min(pb);
                b1 = b1.max(pb);
            }
            if a1 <= b0 + eps || b1 <= a0 + eps {
                return true;
            }
        }
    }
    false
}

fn signed_area(t: &Tri2) -> f64 {
    let (e1, e2) = (t[1] - t[0], t[2] - t[0]);
    0.5 * (e1.x * e2.y - e1.y * e2.x)
}

fn area3(a: DVec3, b: DVec3, c: DVec3) -> f64 {
    0.5 * (b - a).cross(c - a).length()
}

fn to_d(v: [f32; 2]) -> DVec2 {
    DVec2::new(v[0] as f64, v[1] as f64)
}

fn poly_uv_area(mesh: &Mesh, uv: &[[f32; 2]], f: Index) -> f64 {
    let mut a = 0.0;
    mesh.for_each_face_corner(f, |c| {
        let (p, q) = (to_d(uv[c as usize]), to_d(uv[mesh.corner_next(c) as usize]));
        a += p.x * q.y - p.y * q.x;
    });
    0.5 * a
}

fn face_area3(mesh: &Mesh, f: Index, linear: &DMat3) -> f64 {
    let mut n = DVec3::ZERO;
    mesh.for_each_face_corner(f, |c| {
        let p = *linear * mesh.position(mesh.corner_point(c));
        let q = *linear * mesh.position(mesh.corner_next_point(c));
        n += p.cross(q);
    });
    0.5 * n.length()
}

/// Pairs of triangles whose interiors overlap in uv. Triangles in the same group are never compared with each other
/// when same_group_ok is set. Parallel, grid accelerated. Flags are per triangle.
pub fn triangle_overlaps(tris: &[Tri2], flags: &mut Vec<bool>, group: &[Index], same_group_ok: bool) -> usize {
    flags.clear();
    flags.resize(tris.len(), false);
    if tris.len() < 2 {
        return 0;
    }
    let mut lo = Vec::with_capacity(tris.len());
    let mut hi = Vec::with_capacity(tris.len());
    let (mut all0, mut all1) = (DVec2::splat(f64::MAX), DVec2::splat(-f64::MAX));
    let mut sum_ext = 0.0;
    for t in tris {
        let (l, h) = (t[0].min(t[1].min(t[2])), t[0].max(t[1].max(t[2])));
        all0 = all0.min(l);
        all1 = all1.max(h);
        sum_ext += (h.x - l.x).max(h.y - l.y);
        lo.push(l);
        hi.push(h);
    }
    let ext = all1 - all0;
    let eps = 1e-7 * ext.x.max(ext.y);
    let cell = (sum_ext / tris.len() as f64 * 1.5).max(ext.x.max(ext.y) / 2048.0);
    if cell <= 0.0 {
        return 0;
    }
    let gx = ((ext.x / cell).ceil() as i32).max(1);
    let gy = ((ext.y / cell).ceil() as i32).max(1);
    let cell_of = |p: DVec2| {
        IVec2::new((((p.x - all0.x) / cell) as i32).clamp(0, gx - 1), (((p.y - all0.y) / cell) as i32).clamp(0, gy - 1))
    };
    let cells = gx as usize * gy as usize;
    let mut start = vec![0u32; cells + 1];
    let mut skip = vec![false; tris.len()];
    for i in 0..tris.len() {
        if signed_area(&tris[i]).abs() <= eps * eps {
            skip[i] = true;
            continue;
        }
        let (a, b) = (cell_of(lo[i]), cell_of(hi[i]));
        for y in a.y..=b.y {
            for x in a.x..=b.x {
                start[y as usize * gx as usize + x as usize + 1] += 1;
            }
        }
    }
    for i in 1..start.len() {
        start[i] += start[i - 1];
    }
    let mut items = vec![0u32; *start.last().unwrap() as usize];
    let mut fill: Vec<u32> = start[..cells].to_vec();
    for i in 0..tris.len() {
        if skip[i] {
            continue;
        }
        let (a, b) = (cell_of(lo[i]), cell_of(hi[i]));
        for y in a.y..=b.y {
            for x in a.x..=b.x {
                let c = y as usize * gx as usize + x as usize;
                items[fill[c] as usize] = i as u32;
                fill[c] += 1;
            }
        }
    }

    let found: Vec<(u32, u32)> = (0..cells)
        .into_par_iter()
        .flat_map_iter(|c| {
            let (cx, cy) = ((c % gx as usize) as i32, (c / gx as usize) as i32);
            let mut out = Vec::new();
            for s in start[c]..start[c + 1] {
                for t in s + 1..start[c + 1] {
                    let (i, j) = (items[s as usize] as usize, items[t as usize] as usize);
                    if !group.is_empty() && same_group_ok && group[i] == group[j] {
                        continue;
                    }
                    let (l, h) = (lo[i].max(lo[j]), hi[i].min(hi[j]));
                    if l.x >= h.x - eps || l.y >= h.y - eps {
                        continue;
                    }
                    // Only the cell holding the overlap's lower corner reports the pair.
                    if cell_of(l) != IVec2::new(cx, cy) {
                        continue;
                    }
                    if !separated(&tris[i], &tris[j], eps) {
                        out.push((i as u32, j as u32));
                    }
                }
            }
            out
        })
        .collect();
    for &(i, j) in &found {
        flags[i as usize] = true;
        flags[j as usize] = true;
    }
    found.len()
}

/// Per face capacity; faces outside the set (or without uvs) stay zero. All live faces when empty.
pub fn face_distortion(mesh: &Mesh, uv_set: &str, faces: &[Index]) -> Vec<FaceDistortion> {
    let mut out = vec![FaceDistortion::default(); mesh.face_capacity() as usize];
    if mesh.corner_attrs().get::<[f32; 2]>(uv_set).is_empty() {
        return out;
    }
    let co = ChartOptions { source: IslandSource::Uvs, uv_set, ..Default::default() };
    let charts = build_charts(mesh, faces, &co);
    let per_chart: Vec<Vec<(Index, FaceDistortion)>> = charts
        .par_iter()
        .map(|ch| {
            let (a3, au) = (ch.area_3d(), ch.area_uv());
            if a3 <= 0.0 || au.abs() <= 0.0 {
                return Vec::new();
            }
            let k = au.abs() / a3;
            let sign = if au < 0.0 { -1.0 } else { 1.0 };
            let nf = ch.faces.len();
            let (mut fu, mut f3, mut fa) = (vec![0.0; nf], vec![0.0; nf], vec![0.0; nf]);
            let mut fold = vec![false; nf];
            let mut fi = 0;
            for (t, tri) in ch.tris.iter().enumerate() {
                while ch.faces[fi] != ch.tri_face[t] {
                    fi += 1;
                }
                let (p0, p1, p2) = (ch.pos[tri[0] as usize], ch.pos[tri[1] as usize], ch.pos[tri[2] as usize]);
                let a = area3(p0, p1, p2);
                let u: Tri2 = [ch.uv[tri[0] as usize], ch.uv[tri[1] as usize], ch.uv[tri[2] as usize]];
                let su = signed_area(&u) * sign;
                f3[fi] += a;
                fu[fi] += su;
                if a <= 0.0 {
                    continue;
                }
                if su <= 0.0 && su.abs() > 1e-12 * k * a {
                    fold[fi] = true;
                }
                // Singular values of the 3D-to-uv map from its first fundamental form.
                let (e1, e2) = (p1 - p0, p2 - p0);
                let l1 = e1.length();
                if l1 <= 0.0 {
                    continue;
                }
                let x = e1 / l1;
                let qx = e2.dot(x);
                let qy = (e2 - qx * x).length();
                if qy <= 0.0 {
                    continue;
                }
                let (d1, d2) = (u[1] - u[0], u[2] - u[0]);
                // J maps (l1,0)->d1 and (qx,qy)->d2.
                let c0 = d1 / l1;
                let c1 = (d2 - qx * c0) / qy;
                let (e, f, g) = (c0.dot(c0), c0.dot(c1), c1.dot(c1));
                let disc = ((e - g) * (e - g) + 4.0 * f * f).max(0.0).sqrt();
                let s1 = (0.5 * (e + g + disc)).max(0.0).sqrt();
                let s2 = (0.5 * (e + g - disc)).max(0.0).sqrt();
                let aniso = if s2 > 1e-12 { s1 / s2 - 1.0 } else { 100.0 };
                fa[fi] += a * aniso.min(100.0);
            }
            (0..nf)
                .map(|i| {
                    let mut d = FaceDistortion::default();
                    if f3[i] > 0.0 {
                        let ratio = fu[i].max(1e-12 * k * f3[i]) / (k * f3[i]);
                        d.area = ratio.log2() as f32;
                        d.angle = (fa[i] / f3[i]) as f32;
                    }
                    d.folded = fold[i];
                    (ch.faces[i], d)
                })
                .collect()
        })
        .collect();
    for list in per_chart {
        for (f, d) in list {
            out[f as usize] = d;
        }
    }
    out
}

pub fn summarize(mesh: &Mesh, d: &[FaceDistortion], faces: &[Index]) -> DistortionSummary {
    let mut s = DistortionSummary::default();
    let mut w = 0.0;
    for f in live_faces(mesh, faces) {
        let a = mesh.face_area(f);
        let fd = &d[f as usize];
        s.mean_area += a * fd.area.abs() as f64;
        s.mean_angle += a * fd.angle as f64;
        s.max_area = s.max_area.max(fd.area.abs() as f64);
        s.max_angle = s.max_angle.max(fd.angle as f64);
        s.folded += fd.folded as usize;
        w += a;
    }
    if w > 0.0 {
        s.mean_area /= w;
        s.mean_angle /= w;
    }
    s
}

/// Faces (per face capacity) whose uvs overlap another face's or fold over. Returns how many.
pub fn overlapping_faces(mesh: &Mesh, uv_set: &str, flags: &mut Vec<bool>, faces: &[Index]) -> usize {
    flags.clear();
    flags.resize(mesh.face_capacity() as usize, false);
    let uv = mesh.corner_attrs().get::<[f32; 2]>(uv_set);
    if uv.is_empty() {
        return 0;
    }
    let live = live_faces(mesh, faces);
    let mut tris = Vec::new();
    let mut group = Vec::new();
    let mut corners = Vec::new();
    for &f in &live {
        corners.clear();
        triangulate_face(mesh, f, &mut corners, &TriangulateOptions::default());
        for t in corners.chunks_exact(3) {
            tris.push([to_d(uv[t[0] as usize]), to_d(uv[t[1] as usize]), to_d(uv[t[2] as usize])]);
            group.push(f);
        }
    }
    let mut tri_flags = Vec::new();
    triangle_overlaps(&tris, &mut tri_flags, &group, true);
    for (i, &g) in group.iter().enumerate() {
        if tri_flags[i] {
            flags[g as usize] = true;
        }
    }
    let d = face_distortion(mesh, uv_set, &live);
    for &f in &live {
        if d[f as usize].folded {
            flags[f as usize] = true;
        }
    }
    live.iter().filter(|&&f| flags[f as usize]).count()
}

/// True when the chart's uv triangles overlap one another anywhere.
pub fn chart_overlaps(ch: &Chart) -> bool {
    let tris: Vec<Tri2> = ch.tris.iter().map(|t| [ch.uv[t[0] as usize], ch.uv[t[1] as usize], ch.uv[t[2] as usize]]).collect();
    let mut flags = Vec::new();
    triangle_overlaps(&tris, &mut flags, &ch.tri_face, true) > 0
}

/// px per 3D unit for a texture of the given resolution: sqrt(uv area / surface area) * resolution.
pub fn texel_density(mesh: &Mesh, uv_set: &str, faces: &[Index], resolution: f64, linear: &DMat3) -> f64 {
    let uv = mesh.corner_attrs().get::<[f32; 2]>(uv_set);
    if uv.is_empty() {
        return 0.0;
    }
    let (mut au, mut a3) = (0.0, 0.0);
    for f in live_faces(mesh, faces) {
        au += poly_uv_area(mesh, uv, f).abs();
        a3 += face_area3(mesh, f, linear);
    }
    if a3 > 0.0 {
        (au / a3).sqrt() * resolution
    } else {
        0.0
    }
}

/// Scales each uv island (about its centre) so it has the given density, as the reference's Set Texel Density does. linear maps
/// object to world space so density counts world size.
pub fn set_texel_density(mesh: &mut Mesh, uv_set: &str, faces: &[Index], resolution: f64, density: f64, linear: &DMat3) {
    if mesh.corner_attrs().get::<[f32; 2]>(uv_set).is_empty() || density <= 0.0 {
        return;
    }
    let co = ChartOptions { source: IslandSource::Uvs, uv_set, ..Default::default() };
    let islands = island_faces(mesh, faces, &co);
    for isl in islands {
        let cur = texel_density(mesh, uv_set, &isl, resolution, linear);
        if cur <= 0.0 {
            continue;
        }
        let s = (density / cur) as f32;
        let corners: Vec<Index> = isl.iter().flat_map(|&f| mesh.face_corners(f)).collect();
        let uv = uv_layer(mesh, uv_set);
        let (mut lo, mut hi) = ([f32::MAX; 2], [-f32::MAX; 2]);
        for &c in &corners {
            let p = uv[c as usize];
            lo = [lo[0].min(p[0]), lo[1].min(p[1])];
            hi = [hi[0].max(p[0]), hi[1].max(p[1])];
        }
        let mid = [(lo[0] + hi[0]) * 0.5, (lo[1] + hi[1]) * 0.5];
        for &c in &corners {
            let p = uv[c as usize];
            uv[c as usize] = [mid[0] + (p[0] - mid[0]) * s, mid[1] + (p[1] - mid[1]) * s];
        }
    }
}
