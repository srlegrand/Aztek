//! Geometry as the renderer holds it: triangle meshes in object space with their own hierarchy, placed by instances
//! that share them. Both can move over the shutter: a mesh by keys of its points, an instance by keys of its matrix.

use std::sync::Arc;

use glam::{Mat4, Vec2, Vec3};

use crate::bvh::{Aabb, Bvh};
use crate::math::Ray;

/// Which rays see a shape, as Arnold's visibility flags and Maya's render stats.
pub mod vis {
    pub const CAMERA: u8 = 1;
    pub const SHADOW: u8 = 2;
    pub const DIFFUSE_TRANSMIT: u8 = 4;
    pub const SPECULAR_TRANSMIT: u8 = 8;
    pub const VOLUME: u8 = 16;
    pub const DIFFUSE_REFLECT: u8 = 32;
    pub const SPECULAR_REFLECT: u8 = 64;
    pub const SUBSURFACE: u8 = 128;
    pub const ALL: u8 = 255;
}

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub name: String,
    /// Point positions, one list per motion key (at least one), evenly spaced over the shutter.
    pub keys: Vec<Vec<Vec3>>,
    /// Smooth normals per vertex, one list per key; empty for flat shading.
    pub normals: Vec<Vec<Vec3>>,
    /// Texture coordinates per vertex; empty when the mesh has none.
    pub uvs: Vec<Vec2>,
    pub tris: Vec<[u32; 3]>,
    /// Per triangle, the index into its instance's shader list.
    pub tri_shader: Vec<u16>,
    pub bvh: Bvh,
}

impl MeshData {
    pub fn new(name: &str, positions: Vec<Vec3>, normals: Vec<Vec3>, uvs: Vec<Vec2>, tris: Vec<[u32; 3]>, tri_shader: Vec<u16>) -> MeshData {
        let normals = if normals.len() == positions.len() { vec![normals] } else { Vec::new() };
        let mut m = MeshData { name: name.into(), keys: vec![positions], normals, uvs, tris, tri_shader, bvh: Bvh::default() };
        m.build();
        m
    }

    pub fn tri_bounds(&self) -> Vec<Aabb> {
        self.tris
            .iter()
            .map(|t| {
                let mut b = Aabb::EMPTY;
                for k in &self.keys {
                    for &i in t {
                        b.grow(k[i as usize]);
                    }
                }
                b
            })
            .collect()
    }

    pub fn build(&mut self) {
        if self.tri_shader.len() != self.tris.len() {
            self.tri_shader.resize(self.tris.len(), 0);
        }
        self.bvh = Bvh::build(&self.tri_bounds());
    }

    /// The same triangles with points moved: keeps the tree and refits it.
    pub fn refit(&mut self) {
        let b = self.tri_bounds();
        self.bvh.refit(&b);
    }

    pub fn bounds(&self) -> Aabb {
        self.bvh.bounds()
    }

    #[inline]
    fn points(&self, t: &[u32; 3], time: f32) -> (Vec3, Vec3, Vec3) {
        if self.keys.len() == 1 {
            let k = &self.keys[0];
            return (k[t[0] as usize], k[t[1] as usize], k[t[2] as usize]);
        }
        let (a, b, f) = key_span(self.keys.len(), time);
        let (ka, kb) = (&self.keys[a], &self.keys[b]);
        let l = |i: u32| ka[i as usize].lerp(kb[i as usize], f);
        (l(t[0]), l(t[1]), l(t[2]))
    }

    pub fn tri_points(&self, prim: u32, time: f32) -> (Vec3, Vec3, Vec3) {
        self.points(&self.tris[prim as usize], time)
    }

    pub fn memory_bytes(&self) -> usize {
        self.keys.iter().map(|k| k.len() * 12).sum::<usize>() + self.normals.iter().map(|k| k.len() * 12).sum::<usize>() + self.uvs.len() * 8 + self.tris.len() * 14 + self.bvh.nodes.len() * 32 + self.bvh.order.len() * 4
    }
}

/// The two keys either side of a shutter time and how far between them.
#[inline]
pub fn key_span(n: usize, time: f32) -> (usize, usize, f32) {
    if n <= 1 {
        return (0, 0, 0.0);
    }
    let x = time.clamp(0.0, 1.0) * (n - 1) as f32;
    let a = (x.floor() as usize).min(n - 2);
    (a, a + 1, x - a as f32)
}

#[inline]
fn tri_hit(o: Vec3, d: Vec3, p0: Vec3, p1: Vec3, p2: Vec3, tmax: f32) -> Option<(f32, f32, f32)> {
    let e1 = p1 - p0;
    let e2 = p2 - p0;
    let pv = d.cross(e2);
    let det = e1.dot(pv);
    if det.abs() < 1e-20 {
        return None;
    }
    let inv = 1.0 / det;
    let tv = o - p0;
    let u = tv.dot(pv) * inv;
    if !(0.0..=1.0).contains(&u) {
        return None;
    }
    let qv = tv.cross(e1);
    let v = d.dot(qv) * inv;
    if v < 0.0 || u + v > 1.0 {
        return None;
    }
    let t = e2.dot(qv) * inv;
    if t > 1e-7 && t < tmax {
        Some((t, u, v))
    } else {
        None
    }
}

#[derive(Clone, Debug)]
pub struct Instance {
    pub name: String,
    pub mesh: u32,
    /// Object to world, one per motion key (at least one).
    pub xforms: Vec<Mat4>,
    pub inverses: Vec<Mat4>,
    pub visibility: u8,
    /// Shadow rays pass it, its opacity ignored, when false (Arnold's opaque off reads opacity; on treats it as solid).
    pub opaque: bool,
    pub matte: bool,
    /// Materials, by the mesh's per triangle index.
    pub shaders: Vec<u32>,
    /// An object id for the ID AOV.
    pub id: u32,
    /// Index of the mesh light made from it, when it is one.
    pub light: Option<u32>,
    pub receive_shadows: bool,
    pub self_shadows: bool,
    /// Which of the first 64 lights light it (light linking); bits past 64 always do.
    pub light_mask: u64,
}

impl Instance {
    pub fn new(name: &str, mesh: u32, xform: Mat4, shaders: Vec<u32>) -> Instance {
        Instance { name: name.into(), mesh, xforms: vec![xform], inverses: vec![xform.inverse()], visibility: vis::ALL, opaque: true, matte: false, shaders, id: 0, light: None, receive_shadows: true, self_shadows: true, light_mask: u64::MAX }
    }
    pub fn set_xforms(&mut self, xs: Vec<Mat4>) {
        self.inverses = xs.iter().map(|m| m.inverse()).collect();
        self.xforms = xs;
    }
    pub fn xform_at(&self, time: f32) -> (Mat4, Mat4) {
        if self.xforms.len() == 1 {
            return (self.xforms[0], self.inverses[0]);
        }
        let (a, b, f) = key_span(self.xforms.len(), time);
        let m = lerp_mat(&self.xforms[a], &self.xforms[b], f);
        (m, m.inverse())
    }
    pub fn world_bounds(&self, mesh: &MeshData) -> Aabb {
        let b = mesh.bounds();
        let mut out = Aabb::EMPTY;
        for m in &self.xforms {
            out = out.join(&b.transformed(m));
        }
        // Rotation between keys sweeps outside the keys' boxes: pad by a little of the size.
        if self.xforms.len() > 1 {
            let pad = (out.max - out.min).length() * 0.05;
            out.min -= Vec3::splat(pad);
            out.max += Vec3::splat(pad);
        }
        out
    }
}

/// Matrices blended by decomposition, so rotation stays rigid.
pub fn lerp_mat(a: &Mat4, b: &Mat4, f: f32) -> Mat4 {
    let (sa, ra, ta) = a.to_scale_rotation_translation();
    let (sb, rb, tb) = b.to_scale_rotation_translation();
    if !(sa.is_finite() && sb.is_finite()) || sa.min_element().abs() < 1e-12 || sb.min_element().abs() < 1e-12 {
        return *a * (1.0 - f) + *b * f;
    }
    Mat4::from_scale_rotation_translation(sa.lerp(sb, f), ra.slerp(rb, f), ta.lerp(tb, f))
}

/// A ray's nearest hit.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub inst: u32,
    pub prim: u32,
    pub u: f32,
    pub v: f32,
}

/// The instances over the scene.
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    pub meshes: Vec<Arc<MeshData>>,
    pub instances: Vec<Instance>,
    pub tlas: Bvh,
}

impl Geometry {
    pub fn build(&mut self) {
        let b: Vec<Aabb> = self.instances.iter().map(|i| i.world_bounds(&self.meshes[i.mesh as usize])).collect();
        self.tlas = Bvh::build(&b);
    }

    pub fn bounds(&self) -> Aabb {
        self.tlas.bounds()
    }

    /// The nearest hit along a ray of the instances whose visibility has any of `mask`'s bits.
    pub fn intersect(&self, ray: &Ray, tmax: f32, mask: u8) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        self.tlas.traverse(ray.o, ray.d, tmax, |ii, tmax| {
            let inst = &self.instances[ii as usize];
            if inst.visibility & mask == 0 {
                return tmax;
            }
            let mesh = &self.meshes[inst.mesh as usize];
            let (_, inv) = inst.xform_at(ray.time);
            let o = inv.transform_point3(ray.o);
            let d = inv.transform_vector3(ray.d);
            // Object space distances equal world ones: the direction is not normalised.
            mesh.bvh.traverse(o, d, tmax, |p, tmax| {
                let (a, b, c) = mesh.tri_points(p, ray.time);
                match tri_hit(o, d, a, b, c, tmax) {
                    Some((t, u, v)) => {
                        best = Some(Hit { t, inst: ii, prim: p, u, v });
                        t
                    }
                    None => tmax,
                }
            })
        });
        best
    }

    /// Every hit up to tmax, unordered, for shadow rays through transparent things. `each` returns false to stop.
    pub fn intersect_all(&self, ray: &Ray, tmax: f32, mask: u8, mut each: impl FnMut(Hit) -> bool) -> bool {
        let mut stopped = false;
        self.tlas.traverse(ray.o, ray.d, tmax, |ii, tm| {
            let inst = &self.instances[ii as usize];
            if inst.visibility & mask == 0 {
                return tm;
            }
            let mesh = &self.meshes[inst.mesh as usize];
            let (_, inv) = inst.xform_at(ray.time);
            let o = inv.transform_point3(ray.o);
            let d = inv.transform_vector3(ray.d);
            let r = mesh.bvh.traverse(o, d, tmax, |p, tm2| {
                let (a, b, c) = mesh.tri_points(p, ray.time);
                if let Some((t, u, v)) = tri_hit(o, d, a, b, c, tmax) {
                    if !each(Hit { t, inst: ii, prim: p, u, v }) {
                        stopped = true;
                        return -1.0;
                    }
                }
                tm2
            });
            if r < 0.0 {
                -1.0
            } else {
                tm
            }
        });
        stopped
    }
}

/// Where a ray met a surface, in world space.
#[derive(Clone, Copy, Debug)]
pub struct SurfacePoint {
    pub p: Vec3,
    /// Geometric normal, facing the ray's origin.
    pub ng: Vec3,
    /// Shading normal, on the same side as ng.
    pub n: Vec3,
    pub uv: Vec2,
    pub dpdu: Vec3,
    pub dpdv: Vec3,
    /// The ray arrived on the side the triangle's winding faces.
    pub front: bool,
    /// Texture space per world length: for choosing a texture's level of detail.
    pub uv_density: f32,
    pub inst: u32,
    pub prim: u32,
    pub shader: u32,
}

impl Geometry {
    pub fn surface(&self, ray: &Ray, h: &Hit) -> SurfacePoint {
        let inst = &self.instances[h.inst as usize];
        let mesh = &self.meshes[inst.mesh as usize];
        let (m, inv) = inst.xform_at(ray.time);
        let t = mesh.tris[h.prim as usize];
        let (a, b, c) = mesh.tri_points(h.prim, ray.time);
        let w = 1.0 - h.u - h.v;
        let pa = m.transform_point3(a);
        let pb = m.transform_point3(b);
        let pc = m.transform_point3(c);
        let p = ray.at(h.t);
        let e1 = pb - pa;
        let e2 = pc - pa;
        let mut ng = e1.cross(e2);
        let area2 = ng.length();
        ng /= area2.max(1e-30);
        let front = ng.dot(ray.d) < 0.0;
        let nmat = inv.transpose();
        let mut n = if !mesh.normals.is_empty() {
            let nn = if mesh.normals.len() == 1 {
                let k = &mesh.normals[0];
                k[t[0] as usize] * w + k[t[1] as usize] * h.u + k[t[2] as usize] * h.v
            } else {
                let (ka, kb, f) = key_span(mesh.normals.len(), ray.time);
                let l = |i: u32| mesh.normals[ka][i as usize].lerp(mesh.normals[kb][i as usize], f);
                l(t[0]) * w + l(t[1]) * h.u + l(t[2]) * h.v
            };
            nmat.transform_vector3(nn).normalize_or_zero()
        } else {
            ng
        };
        if n.length_squared() < 0.5 {
            n = ng;
        }
        let (mut uv, mut dpdu, mut dpdv, mut density) = (Vec2::new(h.u, h.v), e1, e2, 1.0);
        if !mesh.uvs.is_empty() {
            let (ua, ub, uc) = (mesh.uvs[t[0] as usize], mesh.uvs[t[1] as usize], mesh.uvs[t[2] as usize]);
            uv = ua * w + ub * h.u + uc * h.v;
            let (d1, d2) = (ub - ua, uc - ua);
            let det = d1.x * d2.y - d1.y * d2.x;
            if det.abs() > 1e-20 {
                let r = 1.0 / det;
                dpdu = (e1 * d2.y - e2 * d1.y) * r;
                dpdv = (e2 * d1.x - e1 * d2.x) * r;
                density = (det.abs() / area2.max(1e-30)).sqrt();
            } else {
                let f = Frame3::from(ng);
                dpdu = f.0;
                dpdv = f.1;
            }
        }
        // Normals face the incoming ray; the shading normal follows the geometric one's side.
        let ng_out = if front { ng } else { -ng };
        if n.dot(ng_out) < 0.0 {
            n = -n;
        }
        let shader = inst.shaders.get(mesh.tri_shader[h.prim as usize] as usize).or(inst.shaders.first()).copied().unwrap_or(0);
        SurfacePoint { p, ng: ng_out, n, uv, dpdu, dpdv, front, uv_density: density, inst: h.inst, prim: h.prim, shader }
    }
}

struct Frame3(Vec3, Vec3);
impl From<Vec3> for Frame3 {
    fn from(n: Vec3) -> Self {
        let f = crate::math::Frame::from_n(n);
        Frame3(f.t, f.b)
    }
}

/// Two triangles making a unit quad in the xy plane, for tests and simple scenes.
pub fn quad(a: Vec3, b: Vec3, c: Vec3, d: Vec3) -> MeshData {
    MeshData::new("quad", vec![a, b, c, d], Vec::new(), vec![Vec2::new(0.0, 0.0), Vec2::new(1.0, 0.0), Vec2::new(1.0, 1.0), Vec2::new(0.0, 1.0)], vec![[0, 1, 2], [0, 2, 3]], vec![0, 0])
}

/// A UV sphere with smooth normals.
pub fn sphere(center: Vec3, radius: f32, rings: u32, segments: u32) -> MeshData {
    let mut p = Vec::new();
    let mut n = Vec::new();
    let mut uv = Vec::new();
    for r in 0..=rings {
        let th = r as f32 / rings as f32 * std::f32::consts::PI;
        for s in 0..=segments {
            let ph = s as f32 / segments as f32 * std::f32::consts::TAU;
            let d = Vec3::new(th.sin() * ph.cos(), th.cos(), th.sin() * ph.sin());
            p.push(center + d * radius);
            n.push(d);
            uv.push(Vec2::new(s as f32 / segments as f32, 1.0 - r as f32 / rings as f32));
        }
    }
    let mut tris = Vec::new();
    let row = segments + 1;
    for r in 0..rings {
        for s in 0..segments {
            let (a, b, c, d) = (r * row + s, r * row + s + 1, (r + 1) * row + s + 1, (r + 1) * row + s);
            if r != 0 {
                tris.push([a, b, c]);
            }
            if r != rings - 1 {
                tris.push([a, c, d]);
            }
        }
    }
    let k = tris.len();
    MeshData::new("sphere", p, n, uv, tris, vec![0; k])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instanced_sphere_hits() {
        let mut g = Geometry::default();
        g.meshes.push(Arc::new(sphere(Vec3::ZERO, 1.0, 32, 64)));
        g.instances.push(Instance::new("a", 0, Mat4::from_translation(Vec3::new(0.0, 0.0, -5.0)), vec![0]));
        g.instances.push(Instance::new("b", 0, Mat4::from_translation(Vec3::new(3.0, 0.0, -5.0)) * Mat4::from_scale(Vec3::splat(2.0)), vec![0]));
        g.build();
        let r = Ray::new(Vec3::ZERO, -Vec3::Z, 0.0);
        let h = g.intersect(&r, 1e30, vis::ALL).unwrap();
        assert!((h.t - 4.0).abs() < 0.01, "{}", h.t);
        let s = g.surface(&r, &h);
        assert!(s.n.dot(Vec3::Z) > 0.99 && s.front);
        let r = Ray::new(Vec3::new(3.0, 0.0, 0.0), -Vec3::Z, 0.0);
        let h = g.intersect(&r, 1e30, vis::ALL).unwrap();
        assert_eq!(h.inst, 1);
        assert!((h.t - 3.0).abs() < 0.02, "{}", h.t);
    }
}
