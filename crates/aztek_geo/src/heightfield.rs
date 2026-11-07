//! Heights over the XZ plane on a regular grid.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU32, Ordering};

use glam::{DVec2, DVec3};
use rayon::prelude::*;

use crate::attribute::{names, Index};
use crate::mesh::{Bounds, Mesh};
use crate::triangulate::{triangulate, TriangulateOptions};

/// Heights over the XZ plane on a regular grid, Y up, in scene units. Sample (x, z) sits at origin + (x, z) * cell and
/// is stored at z * nx + x, so rows run along +X and advance along +Z. Masks are named values on the same grid (0 to 1
/// by convention). Everything that generates heights works from world positions, so neighbouring heightfields match.
#[derive(Clone, Debug)]
pub struct Heightfield {
    pub nx: i32,
    pub nz: i32,
    pub cell: f64,
    pub origin: DVec2,
    pub heights: Vec<f32>,
    pub masks: BTreeMap<String, Vec<f32>>,
}

impl Default for Heightfield {
    fn default() -> Self {
        Self { nx: 0, nz: 0, cell: 100.0, origin: DVec2::ZERO, heights: Vec::new(), masks: BTreeMap::new() }
    }
}

/// Grid coordinates of a world position, clamped to the grid, as the cell's corner and the fraction across it.
#[derive(Clone, Copy, Default)]
struct Bilinear {
    x0: i32,
    z0: i32,
    x1: i32,
    z1: i32,
    fx: f64,
    fz: f64,
}

fn locate(h: &Heightfield, wx: f64, wz: f64) -> Bilinear {
    let gx = ((wx - h.origin.x) / h.cell).clamp(0.0, (h.nx - 1) as f64);
    let gz = ((wz - h.origin.y) / h.cell).clamp(0.0, (h.nz - 1) as f64);
    let x0 = (gx as i32).min(h.nx - 2);
    let z0 = (gz as i32).min(h.nz - 2);
    Bilinear { x0, z0, x1: x0 + 1, z1: z0 + 1, fx: gx - x0 as f64, fz: gz - z0 as f64 }
}

fn lerp2(v: &[f32], h: &Heightfield, b: &Bilinear) -> f64 {
    let a = v[h.index(b.x0, b.z0)] as f64 * (1.0 - b.fx) + v[h.index(b.x1, b.z0)] as f64 * b.fx;
    let c = v[h.index(b.x0, b.z1)] as f64 * (1.0 - b.fx) + v[h.index(b.x1, b.z1)] as f64 * b.fx;
    a * (1.0 - b.fz) + c * b.fz
}

/// Sample indices kept at a step: 0, step, 2 step ... and always the last.
fn stepped(n: i32, step: i32) -> Vec<i32> {
    let mut out: Vec<i32> = (0..n - 1).step_by(step as usize).collect();
    out.push(n - 1);
    out
}

impl Heightfield {
    pub fn size(&self) -> usize {
        self.nx as usize * self.nz as usize
    }
    pub fn is_empty(&self) -> bool {
        self.nx < 2 || self.nz < 2
    }
    pub fn index(&self, x: i32, z: i32) -> usize {
        z as usize * self.nx as usize + x as usize
    }
    pub fn at(&self, x: i32, z: i32) -> f32 {
        self.heights[self.index(x, z)]
    }
    pub fn sample_pos(&self, x: i32, z: i32) -> DVec2 {
        self.origin + DVec2::new(x as f64, z as f64) * self.cell
    }
    pub fn position(&self, x: i32, z: i32) -> DVec3 {
        let p = self.sample_pos(x, z);
        DVec3::new(p.x, self.at(x, z) as f64, p.y)
    }
    /// Width and depth covered, sample centre to sample centre.
    pub fn extent(&self) -> DVec2 {
        DVec2::new((self.nx - 1) as f64, (self.nz - 1) as f64) * self.cell
    }
    pub fn contains(&self, wx: f64, wz: f64) -> bool {
        let e = self.extent();
        wx >= self.origin.x && wz >= self.origin.y && wx <= self.origin.x + e.x && wz <= self.origin.y + e.y
    }

    /// Bilinear height at a world XZ position, clamped to the edge.
    pub fn height_at(&self, wx: f64, wz: f64) -> f64 {
        if self.is_empty() {
            return self.heights.first().map_or(0.0, |&h| h as f64);
        }
        lerp2(&self.heights, self, &locate(self, wx, wz))
    }

    /// Unit upward normal of the bilinear surface.
    pub fn normal_at(&self, wx: f64, wz: f64) -> DVec3 {
        if self.is_empty() {
            return DVec3::Y;
        }
        let d = self.cell * 0.5;
        let dx = self.height_at(wx + d, wz) - self.height_at(wx - d, wz);
        let dz = self.height_at(wx, wz + d) - self.height_at(wx, wz - d);
        DVec3::new(-dx, 2.0 * d, -dz).normalize()
    }

    /// Bilinear mask value, clamped to the edge; 0 when the mask doesn't exist.
    pub fn mask_at(&self, name: &str, wx: f64, wz: f64) -> f64 {
        match self.find_mask(name) {
            Some(m) if !self.is_empty() => lerp2(m, self, &locate(self, wx, wz)),
            _ => 0.0,
        }
    }

    /// Lowest and highest height.
    pub fn range(&self) -> DVec2 {
        if self.heights.is_empty() {
            return DVec2::ZERO;
        }
        let lo = self.heights.iter().cloned().fold(f32::INFINITY, f32::min);
        let hi = self.heights.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        DVec2::new(lo as f64, hi as f64)
    }

    pub fn bounds(&self) -> Bounds {
        let mut b = Bounds::default();
        if self.heights.is_empty() {
            return b;
        }
        let r = self.range();
        let e = self.extent();
        b.extend(DVec3::new(self.origin.x, r.x, self.origin.y));
        b.extend(DVec3::new(self.origin.x + e.x, r.y, self.origin.y + e.y));
        b
    }

    /// The mask's values, added as zeros when missing.
    pub fn mask(&mut self, name: &str) -> &mut Vec<f32> {
        let size = self.size();
        let m = self.masks.entry(name.to_string()).or_insert_with(|| vec![0.0; size]);
        m.resize(size, 0.0);
        m
    }

    pub fn find_mask(&self, name: &str) -> Option<&Vec<f32>> {
        self.masks.get(name)
    }
}

/// nx by nz samples, cell apart, all at height h.
pub fn make_heightfield(nx: i32, nz: i32, cell: f64, origin: DVec2, h: f32) -> Heightfield {
    let mut out = Heightfield {
        nx: nx.max(2),
        nz: nz.max(2),
        cell: if cell > 0.0 { cell } else { 1.0 },
        origin,
        ..Default::default()
    };
    out.heights = vec![h; out.size()];
    out
}

/// Covering size (width along X, depth along Z) centred on center, with resolution samples along the longer side.
pub fn make_heightfield_covering(center: DVec2, size: DVec2, resolution: i32, h: f32) -> Heightfield {
    let size = size.max(DVec2::splat(1e-6));
    let resolution = resolution.max(2);
    let cell = size.x.max(size.y) / (resolution - 1) as f64;
    let nx = ((size.x / cell).round() as i32 + 1).max(2);
    let nz = ((size.y / cell).round() as i32 + 1).max(2);
    let extent = DVec2::new((nx - 1) as f64, (nz - 1) as f64) * cell;
    make_heightfield(nx, nz, cell, center - extent * 0.5, h)
}

#[derive(Clone, Debug)]
pub struct HeightfieldMeshOptions {
    /// Keep every step-th sample; the last row and column are always kept so the edges stay put.
    pub step: i32,
    /// Corner uv 0 to 1 over the whole grid.
    pub uvs: bool,
    /// Masks as point float attributes of the same name.
    pub masks: bool,
}

impl Default for HeightfieldMeshOptions {
    fn default() -> Self {
        Self { step: 1, uvs: true, masks: true }
    }
}

/// Quads facing +Y.
pub fn heightfield_to_mesh(h: &Heightfield, o: &HeightfieldMeshOptions) -> Mesh {
    if h.is_empty() {
        return Mesh::new();
    }
    let xs = stepped(h.nx, o.step.max(1));
    let zs = stepped(h.nz, o.step.max(1));
    let (cx, cz) = (xs.len(), zs.len());
    let points: Vec<DVec3> = (0..cx * cz).into_par_iter().map(|k| h.position(xs[k % cx], zs[k / cx])).collect();
    let sizes = vec![4 as Index; (cx - 1) * (cz - 1)];
    let mut idx: Vec<Index> = Vec::with_capacity(sizes.len() * 4);
    let id = |i: usize, j: usize| (j * cx + i) as Index;
    for j in 0..cz - 1 {
        for i in 0..cx - 1 {
            // Rows advance toward +Z, so this order faces +Y.
            idx.extend_from_slice(&[id(i, j), id(i, j + 1), id(i + 1, j + 1), id(i + 1, j)]);
        }
    }
    let mut m = Mesh::from_polygons(&points, &sizes, &idx);
    if o.masks {
        for (name, values) in &h.masks {
            if values.len() != h.size() {
                continue;
            }
            let dst = m.point_attrs_mut().get_or_add::<f32>(name);
            for j in 0..cz {
                for i in 0..cx {
                    dst[j * cx + i] = values[h.index(xs[i], zs[j])];
                }
            }
        }
    }
    if o.uvs {
        let ux = 1.0 / (h.nx - 1).max(1) as f64;
        let uz = 1.0 / (h.nz - 1).max(1) as f64;
        let corner_points: Vec<Index> = (0..m.corner_capacity()).map(|c| m.corner_point(c)).collect();
        let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0));
        for (c, &p) in corner_points.iter().enumerate().take(uv.len()) {
            let i = p as usize % cx;
            let j = p as usize / cx;
            uv[c] = [(xs[i] as f64 * ux) as f32, (zs[j] as f64 * uz) as f32];
        }
    }
    m
}

/// The highest surface of the mesh's faces above each sample, found by rasterizing them from above. Samples no face
/// covers get fill; also returns how many were hit.
pub fn mesh_to_heightfield(m: &Mesh, nx: i32, nz: i32, cell: f64, origin: DVec2, fill: f32) -> (Heightfield, usize) {
    let mut h = make_heightfield(nx, nz, cell, origin, f32::NEG_INFINITY);
    let tris = triangulate(m, &TriangulateOptions::default());
    let cells: Vec<AtomicU32> = h.heights.iter().map(|v| AtomicU32::new(v.to_bits())).collect();
    {
        let h = &h;
        (0..tris.faces.len()).into_par_iter().for_each(|t| {
            let p: [DVec3; 3] = std::array::from_fn(|k| m.position(m.corner_point(tris.corners[t * 3 + k])));
            let g: [DVec2; 3] = std::array::from_fn(|k| (DVec2::new(p[k].x, p[k].z) - h.origin) / h.cell);
            let area = (g[1].x - g[0].x) * (g[2].y - g[0].y) - (g[2].x - g[0].x) * (g[1].y - g[0].y);
            if area.abs() < 1e-14 {
                return;
            }
            let min_x = g[0].x.min(g[1].x).min(g[2].x);
            let max_x = g[0].x.max(g[1].x).max(g[2].x);
            let min_z = g[0].y.min(g[1].y).min(g[2].y);
            let max_z = g[0].y.max(g[1].y).max(g[2].y);
            let x0 = ((min_x - 1e-9).ceil() as i32).max(0);
            let x1 = ((max_x + 1e-9).floor() as i32).min(h.nx - 1);
            let z0 = ((min_z - 1e-9).ceil() as i32).max(0);
            let z1 = ((max_z + 1e-9).floor() as i32).min(h.nz - 1);
            for z in z0..=z1 {
                for x in x0..=x1 {
                    let q = DVec2::new(x as f64, z as f64);
                    let w0 = ((g[1].x - q.x) * (g[2].y - q.y) - (g[2].x - q.x) * (g[1].y - q.y)) / area;
                    let w1 = ((g[2].x - q.x) * (g[0].y - q.y) - (g[0].x - q.x) * (g[2].y - q.y)) / area;
                    let w2 = 1.0 - w0 - w1;
                    let eps = -1e-9;
                    if w0 < eps || w1 < eps || w2 < eps {
                        continue;
                    }
                    let y = (w0 * p[0].y + w1 * p[1].y + w2 * p[2].y) as f32;
                    let cell = &cells[h.index(x, z)];
                    let _ = cell.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |old| {
                        (y > f32::from_bits(old)).then_some(y.to_bits())
                    });
                }
            }
        });
    }
    let mut hit = 0;
    for (v, c) in h.heights.iter_mut().zip(&cells) {
        *v = f32::from_bits(c.load(Ordering::Relaxed));
        if v.is_infinite() {
            *v = fill;
        } else {
            hit += 1;
        }
    }
    (h, hit)
}

/// Resampled bilinearly onto another grid, masks included; outside the source the edge values carry on.
pub fn resampled(h: &Heightfield, nx: i32, nz: i32, cell: f64, origin: DVec2) -> Heightfield {
    let mut out = make_heightfield(nx, nz, cell, origin, 0.0);
    if h.is_empty() {
        return out;
    }
    let at: Vec<Bilinear> = (0..out.size())
        .into_par_iter()
        .map(|k| {
            let x = (k % out.nx as usize) as i32;
            let z = (k / out.nx as usize) as i32;
            let p = out.sample_pos(x, z);
            locate(h, p.x, p.y)
        })
        .collect();
    let fill_from = |src: &[f32], dst: &mut [f32]| {
        dst.par_iter_mut().enumerate().for_each(|(k, d)| *d = lerp2(src, h, &at[k]) as f32);
    };
    fill_from(&h.heights, &mut out.heights);
    for (name, values) in &h.masks {
        if values.len() == h.size() {
            fill_from(values, out.mask(name));
        }
    }
    out
}
