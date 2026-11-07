//!
//! All primitives are Y-up, in centimetres, centred on the origin unless `base_at_origin` is set, with
//! non-overlapping "uv0", hard edges where the surface creases, and "seam" on UV island borders.

use std::collections::BTreeMap;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI, TAU};

use glam::{DVec2, DVec3, IVec3, Vec2};
use rayon::prelude::*;

use crate::attribute::{names, AttrType, Index, INVALID};
use crate::mesh::Mesh;
use crate::mix::{mix2, mix3, mixd, mixf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CubeOptions {
    pub width: f64,
    pub height: f64,
    pub depth: f64,
    pub subdiv_x: i32,
    pub subdiv_y: i32,
    pub subdiv_z: i32,
    pub base_at_origin: bool,
}
impl Default for CubeOptions {
    fn default() -> Self {
        Self {
            width: 100.0,
            height: 100.0,
            depth: 100.0,
            subdiv_x: 1,
            subdiv_y: 1,
            subdiv_z: 1,
            base_at_origin: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaneOptions {
    pub width: f64,
    pub depth: f64,
    pub subdiv_x: i32,
    pub subdiv_z: i32,
}
impl Default for PlaneOptions {
    fn default() -> Self {
        Self {
            width: 100.0,
            depth: 100.0,
            subdiv_x: 1,
            subdiv_z: 1,
        }
    }
}

/// How the flat end of a cylinder or cone is filled inside its rings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapFill {
    /// A grid of quads joined to the rim by a ring of quads; one triangle when the side count is odd. Wedges pair up
    /// their fan.
    Quads,
    /// Triangles meeting at the centre.
    Fan,
    /// One polygon.
    Ngon,
    /// Left open.
    None,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SphereOptions {
    pub radius: f64,
    pub subdiv_axis: i32,
    pub subdiv_height: i32,
    pub base_at_origin: bool,
    /// Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom (-Y).
    pub fraction: f64,
}
impl Default for SphereOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            subdiv_axis: 20,
            subdiv_height: 20,
            base_at_origin: false,
            fraction: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CylinderOptions {
    pub radius: f64,
    pub height: f64,
    pub subdiv_axis: i32,
    pub subdiv_height: i32,
    /// Rings of quads around each cap's fill; 0 and 1 both leave just the fill.
    pub subdiv_caps: i32,
    pub caps: bool,
    pub base_at_origin: bool,
    /// Portion of a full turn swept around Y from +X: 1 is whole, 0.5 half.
    pub fraction: f64,
    /// Close the two flat ends a partial sweep leaves.
    pub cut_caps: bool,
    pub cap_fill: CapFill,
}
impl Default for CylinderOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            height: 100.0,
            subdiv_axis: 20,
            subdiv_height: 1,
            subdiv_caps: 0,
            caps: true,
            base_at_origin: false,
            fraction: 1.0,
            cut_caps: true,
            cap_fill: CapFill::Quads,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CapsuleOptions {
    pub radius: f64,
    /// End to end, caps included; never less than the diameter.
    pub height: f64,
    pub subdiv_axis: i32,
    pub subdiv_height: i32,
    /// Rings on each hemispherical cap.
    pub subdiv_caps: i32,
    pub base_at_origin: bool,
}
impl Default for CapsuleOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            height: 200.0,
            subdiv_axis: 20,
            subdiv_height: 1,
            subdiv_caps: 6,
            base_at_origin: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ConeOptions {
    pub radius: f64,
    pub height: f64,
    pub subdiv_axis: i32,
    pub subdiv_height: i32,
    pub subdiv_cap: i32,
    pub base_at_origin: bool,
    /// Portion of a full turn swept around Y from +X: 1 is whole, 0.5 half.
    pub fraction: f64,
    /// Close the two flat ends a partial sweep leaves.
    pub cut_caps: bool,
    pub cap_fill: CapFill,
    /// Cut the point off at tip_radius and fill it like the base, so a quads fill leaves no triangles at the tip.
    pub cap_tip: bool,
    pub tip_radius: f64,
}
impl Default for ConeOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            height: 100.0,
            subdiv_axis: 20,
            subdiv_height: 1,
            subdiv_cap: 0,
            base_at_origin: false,
            fraction: 1.0,
            cut_caps: true,
            cap_fill: CapFill::Quads,
            cap_tip: false,
            tip_radius: 0.05,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TorusOptions {
    pub radius: f64,
    pub section_radius: f64,
    pub subdiv_axis: i32,
    pub subdiv_height: i32,
    /// Portion of a full turn swept around Y from +X: 1 is whole, 0.5 half.
    pub fraction: f64,
    /// Close the two flat ends a partial sweep leaves.
    pub cut_caps: bool,
}
impl Default for TorusOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            section_radius: 20.0,
            subdiv_axis: 20,
            subdiv_height: 20,
            fraction: 1.0,
            cut_caps: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DiscOptions {
    pub radius: f64,
    pub subdiv_axis: i32,
    /// 0 makes a single n-gon.
    pub subdiv_rings: i32,
    /// Portion of a full turn swept around Y from +X: 1 is whole, 0.5 half.
    pub fraction: f64,
    /// How the innermost ring is filled; None counts as Fan, since a disc is all fill.
    pub cap_fill: CapFill,
}
impl Default for DiscOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            subdiv_axis: 20,
            subdiv_rings: 1,
            fraction: 1.0,
            cap_fill: CapFill::Fan,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PipeOptions {
    pub radius: f64,
    pub thickness: f64,
    pub height: f64,
    pub subdiv_axis: i32,
    pub subdiv_height: i32,
    pub base_at_origin: bool,
    /// Portion of a full turn swept around Y from +X: 1 is whole, 0.5 half.
    pub fraction: f64,
    /// Close the two flat ends a partial sweep leaves.
    pub cut_caps: bool,
}
impl Default for PipeOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            thickness: 10.0,
            height: 100.0,
            subdiv_axis: 20,
            subdiv_height: 1,
            base_at_origin: false,
            fraction: 1.0,
            cut_caps: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuadSphereOptions {
    pub radius: f64,
    pub subdiv: i32,
    /// Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom (-Y).
    pub fraction: f64,
}
impl Default for QuadSphereOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            subdiv: 3,
            fraction: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IcoSphereOptions {
    pub radius: f64,
    /// 1 is the plain icosahedron; each level splits every triangle in four.
    pub subdivisions: i32,
    pub base_at_origin: bool,
    /// Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom (-Y).
    pub fraction: f64,
}
impl Default for IcoSphereOptions {
    fn default() -> Self {
        Self {
            radius: 50.0,
            subdivisions: 3,
            base_at_origin: false,
            fraction: 1.0,
        }
    }
}

fn set_uv0(m: &mut Mesh, uvs: &[Vec2]) {
    let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0));
    for (dst, src) in uv.iter_mut().zip(uvs) {
        *dst = src.to_array();
    }
}

struct GridFace {
    origin: IVec3,
    u: IVec3,
    v: IVec3,
    nu: i32,
    nv: i32,
    uv_min: Vec2,
    uv_max: Vec2,
}

/// Six welded grids on the surface of an sx*sy*sz lattice (five with open_bottom); pos maps lattice coords to space.
fn build_box(
    sx: i32,
    sy: i32,
    sz: i32,
    pos: impl Fn(IVec3) -> DVec3,
    hard_sides: bool,
    open_bottom: bool,
) -> Mesh {
    let mut m = Mesh::new();
    let (x, y, z) = (IVec3::X, IVec3::Y, IVec3::Z);
    let v2 = Vec2::new;
    // The reference's cross layout: a column of front, top, back, bottom with right and left beside the front.
    let faces = [
        GridFace {
            origin: IVec3::new(0, 0, sz),
            u: x,
            v: y,
            nu: sx,
            nv: sy,
            uv_min: v2(0.375, 0.0),
            uv_max: v2(0.625, 0.25),
        }, // +Z
        GridFace {
            origin: IVec3::new(0, sy, sz),
            u: x,
            v: -z,
            nu: sx,
            nv: sz,
            uv_min: v2(0.375, 0.25),
            uv_max: v2(0.625, 0.5),
        }, // +Y
        GridFace {
            origin: IVec3::new(0, sy, 0),
            u: x,
            v: -y,
            nu: sx,
            nv: sy,
            uv_min: v2(0.375, 0.5),
            uv_max: v2(0.625, 0.75),
        }, // -Z
        GridFace {
            origin: IVec3::new(0, 0, 0),
            u: x,
            v: z,
            nu: sx,
            nv: sz,
            uv_min: v2(0.375, 0.75),
            uv_max: v2(0.625, 1.0),
        }, // -Y
        GridFace {
            origin: IVec3::new(sx, 0, sz),
            u: -z,
            v: y,
            nu: sz,
            nv: sy,
            uv_min: v2(0.625, 0.0),
            uv_max: v2(0.875, 0.25),
        }, // +X
        GridFace {
            origin: IVec3::new(0, 0, 0),
            u: z,
            v: y,
            nu: sz,
            nv: sy,
            uv_min: v2(0.125, 0.0),
            uv_max: v2(0.375, 0.25),
        }, // -X
    ];
    let lattice = (sx + 1) as usize * (sy + 1) as usize * (sz + 1) as usize;
    let mut ids = vec![INVALID; lattice];
    let face_count =
        2 * (sx as usize * sy as usize + sy as usize * sz as usize + sx as usize * sz as usize);
    m.reserve(lattice, face_count * 2, face_count, face_count * 4);
    let mut side = Vec::with_capacity(face_count);
    let mut corner_uv = Vec::with_capacity(face_count * 4);
    for (s, g) in faces.iter().enumerate() {
        if open_bottom && s == 3 {
            continue;
        }
        for b in 0..g.nv {
            for a in 0..g.nu {
                let mut point = |c: IVec3| {
                    let i = (c.z as usize * (sy + 1) as usize + c.y as usize) * (sx + 1) as usize
                        + c.x as usize;
                    if ids[i] == INVALID {
                        ids[i] = m.add_point(pos(c));
                    }
                    ids[i]
                };
                let q = [
                    point(g.origin + g.u * a + g.v * b),
                    point(g.origin + g.u * (a + 1) + g.v * b),
                    point(g.origin + g.u * (a + 1) + g.v * (b + 1)),
                    point(g.origin + g.u * a + g.v * (b + 1)),
                ];
                if m.add_face(&q) == INVALID {
                    continue;
                }
                side.push(s);
                let d = g.uv_max - g.uv_min;
                let (u0, u1) = (a as f32 / g.nu as f32, (a + 1) as f32 / g.nu as f32);
                let (v0, v1) = (b as f32 / g.nv as f32, (b + 1) as f32 / g.nv as f32);
                corner_uv.push(g.uv_min + d * v2(u0, v0));
                corner_uv.push(g.uv_min + d * v2(u1, v0));
                corner_uv.push(g.uv_min + d * v2(u1, v1));
                corner_uv.push(g.uv_min + d * v2(u0, v1));
            }
        }
    }
    set_uv0(&mut m, &corner_uv);
    if hard_sides {
        let ec = m.edge_capacity();
        let hard: Vec<bool> = (0..ec)
            .map(|e| {
                let c = m.edge_corner(e);
                let r = m.corner_radial(c);
                side[m.corner_face(c) as usize] != side[m.corner_face(r) as usize]
            })
            .collect();
        m.edge_attrs_mut()
            .get_or_add::<bool>(names::HARD)
            .copy_from_slice(&hard);
    }
    mark_uv_seams(&mut m, &names::uv(0));
    m
}

#[derive(Clone, Copy)]
struct ProfilePoint {
    r: f64,
    y: f64,
    hard: bool,
}

fn pp(r: f64, y: f64) -> ProfilePoint {
    ProfilePoint { r, y, hard: false }
}

#[derive(Clone, Copy, PartialEq)]
enum BandMode {
    Cylindrical,
    Planar,
}

#[derive(Clone, Copy)]
struct Band {
    from: i32,
    to: i32,
    mode: BandMode,
    /// Cylindrical v range.
    v: Vec2,
    /// Planar.
    center: Vec2,
    /// Planar uv per cm.
    scale: f32,
    /// How a band ending in a pole closes.
    fill: CapFill,
}

fn cyl_band(from: i32, to: i32, v: Vec2) -> Band {
    Band {
        from,
        to,
        mode: BandMode::Cylindrical,
        v,
        center: Vec2::splat(0.5),
        scale: 0.0,
        fill: CapFill::Fan,
    }
}

fn planar_band(from: i32, to: i32, center: Vec2, scale: f32, fill: CapFill) -> Band {
    Band {
        from,
        to,
        mode: BandMode::Planar,
        v: Vec2::ZERO,
        center,
        scale,
        fill,
    }
}

/// Closes both ends of a partial lathe with the profile polygon at each, run back along the axis when the profile is
/// open. The caps get their own uv islands in a strip along the bottom, with the swept surface scaled into the space above.
#[allow(clippy::too_many_arguments)]
fn close_cuts(
    m: &mut Mesh,
    corner_uv: &mut Vec<Vec2>,
    profile: &[ProfilePoint],
    pole: &[bool],
    pid: &dyn Fn(usize, i32) -> Index,
    axis: i32,
    sweep: f64,
    closed: bool,
) -> Vec<Index> {
    let np = profile.len();
    let (mut axis_first, mut axis_last) = (INVALID, INVALID);
    struct Cap {
        pts: Vec<Index>,
        local: Vec<DVec2>,
    }
    let mut caps = Vec::new();
    for end in 0..2 {
        let out = if end == 1 {
            DVec3::new(-sweep.sin(), 0.0, -sweep.cos())
        } else {
            DVec3::Z
        };
        let mut pts: Vec<Index> = (0..np)
            .map(|k| pid(k, if end == 1 { axis } else { 0 }))
            .collect();
        if !closed {
            if !pole[np - 1] {
                if axis_last == INVALID {
                    axis_last = m.add_point(DVec3::new(0.0, profile[np - 1].y, 0.0));
                }
                pts.push(axis_last);
            }
            if !pole[0] {
                if axis_first == INVALID {
                    axis_first = m.add_point(DVec3::new(0.0, profile[0].y, 0.0));
                }
                pts.push(axis_first);
            }
        }
        let mut normal = DVec3::ZERO;
        for a in 0..pts.len() {
            normal += m
                .position(pts[a])
                .cross(m.position(pts[(a + 1) % pts.len()]));
        }
        if normal.length() < 1e-9 {
            continue;
        }
        if normal.dot(out) < 0.0 {
            pts.reverse();
        }
        let u = DVec3::Y.cross(out);
        let local = pts
            .iter()
            .map(|&p| DVec2::new(m.position(p).dot(u), m.position(p).y))
            .collect();
        caps.push(Cap { pts, local });
    }
    if caps.is_empty() {
        return Vec::new();
    }
    for uv in corner_uv.iter_mut() {
        *uv = *uv * 0.78 + Vec2::new(0.0, 0.22);
    }
    let mut faces = Vec::new();
    for (k, cap) in caps.iter().enumerate() {
        let f = m.add_face(&cap.pts);
        if f == INVALID {
            continue;
        }
        faces.push(f);
        let (mut lo, mut hi) = (DVec2::splat(f64::MAX), DVec2::splat(-f64::MAX));
        for l in &cap.local {
            lo = lo.min(*l);
            hi = hi.max(*l);
        }
        let box_min = DVec2::new(0.01 + 0.5 * k as f64, 0.01);
        let box_size = DVec2::new(0.48, 0.19);
        let extent = (hi - lo).max(DVec2::splat(1e-12));
        let scale = (box_size.x / extent.x).min(box_size.y / extent.y);
        let offset = box_min + (box_size - extent * scale) * 0.5;
        for l in &cap.local {
            corner_uv.push((offset + (*l - lo) * scale).as_vec2());
        }
    }
    faces
}

/// Revolves a profile in (r, y) around +Y. Profile points with r == 0 become a single pole point. Walking the profile
/// with the outside on the right gives outward faces. A closed profile wraps its last band back to point 0. A fraction
/// below 1 sweeps only that part of the turn, from +X toward -Z; cut_caps then closes each cut end with the profile
/// polygon, run back along the axis when it is open.
fn lathe(
    profile: &[ProfilePoint],
    bands: &[Band],
    axis: i32,
    closed: bool,
    fraction: f64,
    cut_caps: bool,
) -> Mesh {
    let mut m = Mesh::new();
    let np = profile.len();
    let partial = fraction < 1.0 - 1e-9;
    let sweep = TAU * fraction.clamp(0.01, 1.0);
    let ring = if partial { axis + 1 } else { axis };
    let mut ring_start = vec![0 as Index; np];
    let mut pole = vec![false; np];
    for k in 0..np {
        pole[k] = profile[k].r.abs() < 1e-12;
        ring_start[k] = m.point_capacity();
        if pole[k] {
            m.add_point(DVec3::new(0.0, profile[k].y, 0.0));
            continue;
        }
        for i in 0..ring {
            let t = sweep * i as f64 / axis as f64;
            m.add_point(DVec3::new(
                profile[k].r * t.cos(),
                profile[k].y,
                -profile[k].r * t.sin(),
            ));
        }
    }
    let pid = |k: usize, i: i32| -> Index {
        if pole[k] {
            ring_start[k]
        } else {
            ring_start[k] + (i % ring) as Index
        }
    };

    let mut corner_uv: Vec<Vec2> = Vec::new();
    let planar_uv = |m: &Mesh, b: &Band, p: Index, facing_up: bool| {
        let x = m.position(p);
        b.center + Vec2::new(x.x as f32, if facing_up { -x.z as f32 } else { x.z as f32 }) * b.scale
    };
    let mut pts: Vec<Index> = Vec::new();
    let mut uvs: Vec<Vec2> = Vec::new();
    fn emit(m: &mut Mesh, pts: &mut Vec<Index>, uvs: &mut Vec<Vec2>, corner_uv: &mut Vec<Vec2>) {
        if m.add_face(pts) != INVALID {
            corner_uv.extend_from_slice(uvs);
        }
        pts.clear();
        uvs.clear();
    }
    // Cap faces are built wound for a top cap (rim i, rim i + 1, inside) and reversed for a bottom one.
    let emit_cap = |m: &mut Mesh,
                    corner_uv: &mut Vec<Vec2>,
                    b: &Band,
                    mut face: Vec<Index>,
                    bottom: bool,
                    up: bool| {
        if bottom {
            face.reverse();
        }
        let uvs: Vec<Vec2> = face.iter().map(|&p| planar_uv(m, b, p, up)).collect();
        let (mut f, mut u) = (face, uvs);
        emit(m, &mut f, &mut u, corner_uv);
    };
    // Fills a whole circle of rim points with a Coons-patch grid of a x c quads (2(a + c) border points), joined to the
    // rim by a ring of quads. An odd rim has one point more than the grid's border, so its ring closes with a triangle.
    let quad_cap =
        |m: &mut Mesh, corner_uv: &mut Vec<Vec2>, b: &Band, rim: usize, bottom: bool, up: bool| {
            let border = axis - axis % 2;
            let a = border / 4;
            let c = border / 2 - a;
            let y = profile[rim].y;
            // Sized so the ring is about as wide as the grid's cells.
            let rho = profile[rim].r / (1.0 + 1.3 / a.max(c) as f64);
            let on_circle = |j: i32| {
                // Each border point faces its rim point; with an odd rim the last border gap spans the triangle's two segments.
                let t = TAU * (if j == border { axis } else { j }) as f64 / axis as f64;
                DVec3::new(rho * t.cos(), y, -rho * t.sin())
            };
            let corner = [0, a, a + c, 2 * a + c, border];
            let mut lp = vec![DVec3::ZERO; border as usize];
            for side in 0..4 {
                let len = corner[side + 1] - corner[side];
                for k in 0..len {
                    let j = corner[side] + k;
                    // Straight sides rounded toward the circle keep the grid's corner quads square.
                    lp[j as usize] = mix3(
                        mix3(
                            on_circle(corner[side]),
                            on_circle(corner[side + 1]),
                            k as f64 / len as f64,
                        ),
                        on_circle(j),
                        0.4,
                    );
                }
            }
            let border_index = |u: i32, v: i32| -> usize {
                (if v == 0 {
                    u
                } else if u == a {
                    a + v
                } else if v == c {
                    a + c + (a - u)
                } else {
                    (2 * a + c + (c - v)) % border
                }) as usize
            };
            let inner: Vec<Index> = lp.iter().map(|&p| m.add_point(p)).collect();
            let mut grid = vec![0 as Index; ((a + 1) * (c + 1)) as usize];
            let at = |u: i32, v: i32| lp[border_index(u, v)];
            for v in 0..=c {
                for u in 0..=a {
                    if u == 0 || v == 0 || u == a || v == c {
                        grid[(v * (a + 1) + u) as usize] = inner[border_index(u, v)];
                        continue;
                    }
                    let s = u as f64 / a as f64;
                    let t = v as f64 / c as f64;
                    let p =
                        (1.0 - s) * at(0, v) + s * at(a, v) + (1.0 - t) * at(u, 0) + t * at(u, c)
                            - ((1.0 - s) * (1.0 - t) * at(0, 0)
                                + s * (1.0 - t) * at(a, 0)
                                + (1.0 - s) * t * at(0, c)
                                + s * t * at(a, c));
                    grid[(v * (a + 1) + u) as usize] = m.add_point(p);
                }
            }
            let g = |u: i32, v: i32| grid[(v * (a + 1) + u) as usize];
            for v in 0..c {
                for u in 0..a {
                    emit_cap(
                        m,
                        corner_uv,
                        b,
                        vec![g(u, v), g(u + 1, v), g(u + 1, v + 1), g(u, v + 1)],
                        bottom,
                        up,
                    );
                }
            }
            for j in 0..border {
                emit_cap(
                    m,
                    corner_uv,
                    b,
                    vec![
                        pid(rim, j),
                        pid(rim, j + 1),
                        inner[((j + 1) % border) as usize],
                        inner[j as usize],
                    ],
                    bottom,
                    up,
                );
            }
            if border < axis {
                emit_cap(
                    m,
                    corner_uv,
                    b,
                    vec![pid(rim, axis - 1), pid(rim, 0), inner[0]],
                    bottom,
                    up,
                );
            }
        };

    for b in bands {
        let mut len = vec![0.0f64];
        for k in b.from..b.to {
            let p0 = profile[k as usize % np];
            let p1 = profile[(k + 1) as usize % np];
            len.push(len.last().unwrap() + (p1.r - p0.r).hypot(p1.y - p0.y));
        }
        let total = *len.last().unwrap();
        let v_at = |k: i32| {
            let t = (len[(k - b.from) as usize] / total.max(1e-12)) as f32;
            mixf(b.v.x, b.v.y, t)
        };
        for k in b.from..b.to {
            let (k0, k1) = (k as usize % np, (k + 1) as usize % np);
            if pole[k0] && pole[k1] {
                continue;
            }
            let up = profile[k1].r < profile[k0].r;
            if b.fill == CapFill::Quads && (pole[k0] || pole[k1]) && (partial || axis >= 4) {
                let rim = if pole[k0] { k1 } else { k0 };
                let centre = pid(if pole[k0] { k0 } else { k1 }, 0);
                if !partial {
                    quad_cap(&mut m, &mut corner_uv, b, rim, pole[k0], up);
                } else {
                    // A wedge pairs up its fan: each quad spans two segments, an odd one out stays a triangle.
                    let mut i = 0;
                    while i < axis {
                        let face = if i + 1 < axis {
                            vec![pid(rim, i), pid(rim, i + 1), pid(rim, i + 2), centre]
                        } else {
                            vec![pid(rim, i), pid(rim, i + 1), centre]
                        };
                        emit_cap(&mut m, &mut corner_uv, b, face, pole[k0], up);
                        i += 2;
                    }
                }
                continue;
            }
            if b.fill != CapFill::Fan && (pole[k0] || pole[k1]) {
                let rim = if pole[k0] { k1 } else { k0 };
                // A partial sweep's cap is a wedge, so it keeps its centre point.
                if partial && pole[k0] {
                    pts.push(pid(k0, 0));
                }
                for i in 0..ring {
                    pts.push(pid(rim, if pole[k0] { ring - 1 - i } else { i }));
                }
                if partial && pole[k1] {
                    pts.push(pid(k1, 0));
                }
                for &p in &pts {
                    uvs.push(planar_uv(&m, b, p, up));
                }
                emit(&mut m, &mut pts, &mut uvs, &mut corner_uv);
                continue;
            }
            for i in 0..axis {
                let (u0, u1) = (i as f32 / axis as f32, (i + 1) as f32 / axis as f32);
                let um = (u0 + u1) * 0.5;
                let (v0, v1) = (v_at(k), v_at(k + 1));
                let mut add = |kk: i32, ii: i32, u: f32, v: f32| {
                    let p = pid(kk as usize % np, ii);
                    pts.push(p);
                    uvs.push(if b.mode == BandMode::Cylindrical {
                        Vec2::new(u, v)
                    } else {
                        planar_uv(&m, b, p, up)
                    });
                };
                if pole[k0] {
                    add(k, i, um, v0);
                    add(k + 1, i + 1, u1, v1);
                    add(k + 1, i, u0, v1);
                } else if pole[k1] {
                    add(k, i, u0, v0);
                    add(k, i + 1, u1, v0);
                    add(k + 1, i, um, v1);
                } else {
                    add(k, i, u0, v0);
                    add(k, i + 1, u1, v0);
                    add(k + 1, i + 1, u1, v1);
                    add(k + 1, i, u0, v1);
                }
                emit(&mut m, &mut pts, &mut uvs, &mut corner_uv);
            }
        }
    }

    let cap_faces = if partial && cut_caps {
        close_cuts(
            &mut m,
            &mut corner_uv,
            profile,
            &pole,
            &pid,
            axis,
            sweep,
            closed,
        )
    } else {
        Vec::new()
    };
    set_uv0(&mut m, &corner_uv);

    let mut hard_edges = Vec::new();
    for k in 0..np {
        if profile[k].hard && !pole[k] {
            for i in 0..axis {
                let e = m.find_edge(pid(k, i), pid(k, i + 1));
                if e != INVALID {
                    hard_edges.push(e);
                }
            }
        }
    }
    for &f in &cap_faces {
        m.for_each_face_corner(f, |c| hard_edges.push(m.corner_edge(c)));
    }
    let hard = m.edge_attrs_mut().get_or_add::<bool>(names::HARD);
    for e in hard_edges {
        hard[e as usize] = true;
    }
    // N-gon caps leave their centre point unused.
    m.remove_all_loose();
    if m.has_deleted() {
        m.compact(None);
    }
    mark_uv_seams(&mut m, &names::uv(0));
    m
}

fn shift_y(m: &mut Mesh, dy: f64) {
    for p in m.positions_mut() {
        p.y += dy;
    }
}

// Planar cap islands in the lower half of uv space, radius just under a quarter.
const CAP_SCALE: f32 = 0.24;
const CAP_LEFT: Vec2 = Vec2::new(0.25, 0.25);
const CAP_RIGHT: Vec2 = Vec2::new(0.75, 0.25);

/// Sets edge "seam" where the corners on either side disagree on the UV set. Returns the count.
pub fn mark_uv_seams(mesh: &mut Mesh, uv_set: &str) -> Index {
    let uv = mesh.corner_attrs().get::<[f32; 2]>(uv_set);
    if uv.is_empty() {
        return 0;
    }
    let differs =
        |a: [f32; 2], b: [f32; 2]| (a[0] - b[0]).abs() > 1e-6 || (a[1] - b[1]).abs() > 1e-6;
    let m = &*mesh;
    let seam: Vec<bool> = (0..m.edge_capacity())
        .into_par_iter()
        .with_min_len(16384)
        .map(|e| {
            if !m.edge_valid(e) || m.edge_corner(e) == INVALID {
                return false;
            }
            let c = m.edge_corner(e);
            let p = m.corner_point(c);
            let mut out = false;
            m.for_each_edge_corner(e, |r| {
                if r == c {
                    return;
                }
                // Corner r runs along e either way; find its uvs at c's two points.
                let same = m.corner_point(r) == p;
                let a = if same {
                    uv[r as usize]
                } else {
                    uv[m.corner_next(r) as usize]
                };
                let b = if same {
                    uv[m.corner_next(r) as usize]
                } else {
                    uv[r as usize]
                };
                if differs(a, uv[c as usize]) || differs(b, uv[m.corner_next(c) as usize]) {
                    out = true;
                }
            });
            out
        })
        .collect();
    let count = seam.iter().filter(|&&s| s).count() as Index;
    mesh.edge_attrs_mut()
        .get_or_add::<bool>(names::SEAM)
        .copy_from_slice(&seam);
    count
}

pub fn make_cube(o: &CubeOptions) -> Mesh {
    let (sx, sy, sz) = (o.subdiv_x.max(1), o.subdiv_y.max(1), o.subdiv_z.max(1));
    let size = DVec3::new(o.width, o.height, o.depth);
    let offset = DVec3::new(
        -0.5 * o.width,
        if o.base_at_origin {
            0.0
        } else {
            -0.5 * o.height
        },
        -0.5 * o.depth,
    );
    let n = DVec3::new(sx as f64, sy as f64, sz as f64);
    build_box(
        sx,
        sy,
        sz,
        |c| offset + size * c.as_dvec3() / n,
        true,
        false,
    )
}

/// Subdivided cube projected onto a sphere: all quads, no poles.
pub fn make_quad_sphere(o: &QuadSphereOptions) -> Mesh {
    let n = o.subdiv.max(1);
    let q = FRAC_PI_4;
    let tan3 = |v: DVec3| DVec3::new(v.x.tan(), v.y.tan(), v.z.tan());
    if o.fraction >= 1.0 - 1e-6 {
        return build_box(
            n,
            n,
            n,
            |c| {
                // Tangent warp spreads the grid evenly over the sphere.
                let a = c.as_dvec3() / n as f64 * 2.0 - 1.0;
                tan3(a * q).normalize() * o.radius
            },
            false,
            false,
        );
    }
    // The top face keeps its grid; each side column runs down its meridian from the top face to the cut circle.
    let s = 1.0 - 2.0 * o.fraction.clamp(0.01, 1.0);
    let rho = (1.0 - s * s).sqrt();
    // Shallow domes shrink the top face so it stays well inside the cut.
    let g = ((s.acos() * 2f64.sqrt().atan() / FRAC_PI_2).tan() / 2f64.sqrt()).min(1.0);
    let cut_row = |h: f64| (s * h / rho).atan() / q;
    let m = ((0.5 * n as f64 * (1.0 - cut_row(g))).round() as i32).max(1);
    build_box(
        n,
        m,
        n,
        |c| {
            let t = DVec2::new(c.x as f64, c.z as f64) / n as f64 * 2.0 - 1.0;
            let h = g * DVec2::new((t.x * q).tan(), (t.y * q).tan());
            if c.y == 0 {
                let d = h.normalize() * rho;
                return DVec3::new(d.x, s, d.y) * o.radius;
            }
            let ay = if c.y == m {
                1.0
            } else {
                mixd(cut_row(h.length()), 1.0, c.y as f64 / m as f64)
            };
            DVec3::new(h.x, (ay * q).tan(), h.y).normalize() * o.radius
        },
        false,
        true,
    )
}

/// Geodesic sphere of near-equal triangles: no poles, no pinching.
pub fn make_ico_sphere(o: &IcoSphereOptions) -> Mesh {
    let t = (1.0 + 5f64.sqrt()) * 0.5;
    let d = DVec3::new;
    let corners = [
        d(-1.0, t, 0.0),
        d(1.0, t, 0.0),
        d(-1.0, -t, 0.0),
        d(1.0, -t, 0.0),
        d(0.0, -1.0, t),
        d(0.0, 1.0, t),
        d(0.0, -1.0, -t),
        d(0.0, 1.0, -t),
        d(t, 0.0, -1.0),
        d(t, 0.0, 1.0),
        d(-t, 0.0, -1.0),
        d(-t, 0.0, 1.0),
    ];
    let faces: [[usize; 3]; 20] = [
        [0, 11, 5],
        [0, 5, 1],
        [0, 1, 7],
        [0, 7, 10],
        [0, 10, 11],
        [1, 5, 9],
        [5, 11, 4],
        [11, 10, 2],
        [10, 7, 6],
        [7, 1, 8],
        [3, 9, 4],
        [3, 4, 2],
        [3, 2, 6],
        [3, 6, 8],
        [3, 8, 9],
        [4, 9, 5],
        [2, 4, 11],
        [6, 2, 10],
        [8, 6, 7],
        [9, 8, 1],
    ];
    // Each level splits every triangle in four, as in Blender.
    let n = 1i32 << (o.subdivisions - 1).clamp(0, 7);
    let cut = o.fraction < 1.0 - 1e-6;
    let yc = 1.0 - 2.0 * o.fraction.clamp(0.01, 1.0);
    let rho = (1.0 - yc * yc).sqrt();
    let on_cut = |s: DVec3| {
        let dd = DVec2::new(s.x, s.z).normalize() * rho;
        DVec3::new(dd.x, yc, dd.y)
    };
    // Points this close to the cut move onto it, so clipping leaves no slivers; never so far that a shallow cap flattens.
    let snap = (0.3 * 1.05 / n as f64).min(0.25 * (1.0 - yc));

    let mut m = Mesh::new();
    // Lattice points on shared icosahedron edges coincide exactly, so welding by rounded position is safe.
    let mut welded: BTreeMap<[i64; 3], Index> = BTreeMap::new();
    let uv_of = |s: DVec3| {
        Vec2::new(
            (0.5 + s.x.atan2(s.z) / TAU) as f32,
            (0.5 + s.y.clamp(-1.0, 1.0).asin() / PI) as f32,
        )
    };
    let mut uvs: Vec<Vec2> = Vec::new();
    let mut add_tri = |m: &mut Mesh, a: DVec3, b: DVec3, c: DVec3| {
        let mut ss = [a.normalize(), b.normalize(), c.normalize()];
        if cut {
            for p in &mut ss {
                if (p.y - yc).abs() < snap && DVec2::new(p.x, p.z).length() > 1e-6 {
                    *p = on_cut(*p);
                }
            }
        }
        let mut tri = [uv_of(ss[0]), uv_of(ss[1]), uv_of(ss[2])];
        // Unwrap triangles straddling the u seam, and give pole corners the u of their face.
        let max_u = tri[0].x.max(tri[1].x).max(tri[2].x);
        for uv in &mut tri {
            if max_u - uv.x > 0.5 {
                uv.x += 1.0;
            }
        }
        for k in 0..3 {
            if ss[k].y.abs() > 1.0 - 1e-9 {
                tri[k].x = (tri[(k + 1) % 3].x + tri[(k + 2) % 3].x) * 0.5;
            }
        }
        let mut ids: Vec<Index> = Vec::new();
        let mut poly_uv: Vec<Vec2> = Vec::new();
        let mut keep = |m: &mut Mesh, p: DVec3, uv: Vec2| {
            let key = [
                (p.x * 1e9).round() as i64,
                (p.y * 1e9).round() as i64,
                (p.z * 1e9).round() as i64,
            ];
            let id = *welded
                .entry(key)
                .or_insert_with(|| m.add_point(p * o.radius));
            if ids.last() == Some(&id) || ids.first() == Some(&id) {
                return;
            }
            ids.push(id);
            poly_uv.push(uv);
        };
        let mut above = !cut;
        for k in 0..3 {
            let (p0, p1) = (ss[k], ss[(k + 1) % 3]);
            let in0 = !cut || p0.y >= yc - 1e-12;
            let in1 = !cut || p1.y >= yc - 1e-12;
            if in0 {
                keep(m, p0, tri[k]);
                above = above || p0.y > yc + 1e-12;
            }
            if in0 != in1 {
                let tt = (p0.y - yc) / (p0.y - p1.y);
                keep(
                    m,
                    on_cut(mix3(p0, p1, tt)),
                    mix2(tri[k], tri[(k + 1) % 3], tt as f32),
                );
            }
        }
        // A triangle touching the cut only along its edge, or lying in it, leaves nothing.
        if ids.len() < 3 || !above || m.add_face(&ids) == INVALID {
            return;
        }
        uvs.extend(poly_uv);
    };
    for f in &faces {
        let (a, b, c) = (corners[f[0]], corners[f[1]], corners[f[2]]);
        let at =
            |i: i32, j: i32| a + (b - a) * (i as f64 / n as f64) + (c - a) * (j as f64 / n as f64);
        for j in 0..n {
            for i in 0..n - j {
                add_tri(&mut m, at(i, j), at(i + 1, j), at(i, j + 1));
                if i + j + 1 < n {
                    add_tri(&mut m, at(i + 1, j), at(i + 1, j + 1), at(i, j + 1));
                }
            }
        }
    }
    set_uv0(&mut m, &uvs);
    m.edge_attrs_mut().add(names::HARD, AttrType::Bool);
    m.edge_attrs_mut().add(names::SEAM, AttrType::Bool);
    m.remove_all_loose();
    if m.has_deleted() {
        m.compact(None);
    }
    mark_uv_seams(&mut m, &names::uv(0));
    if o.base_at_origin {
        shift_y(&mut m, -(if cut { yc } else { -1.0 }) * o.radius);
    }
    m
}

/// In XZ facing +Y.
pub fn make_plane(o: &PlaneOptions) -> Mesh {
    let (sx, sz) = (o.subdiv_x.max(1), o.subdiv_z.max(1));
    let mut m = Mesh::new();
    let (ux, uz) = (sx as usize, sz as usize);
    m.reserve(
        (ux + 1) * (uz + 1),
        ux * uz * 2 + ux + uz,
        ux * uz,
        ux * uz * 4,
    );
    for j in 0..=sz {
        for i in 0..=sx {
            m.add_point(DVec3::new(
                (i as f64 / sx as f64 - 0.5) * o.width,
                0.0,
                (0.5 - j as f64 / sz as f64) * o.depth,
            ));
        }
    }
    let mut uvs = Vec::with_capacity(ux * uz * 4);
    let id = |i: i32, j: i32| (j * (sx + 1) + i) as Index;
    for j in 0..sz {
        for i in 0..sx {
            // Rows advance toward -Z, so this order faces +Y.
            m.add_face(&[id(i, j), id(i + 1, j), id(i + 1, j + 1), id(i, j + 1)]);
            let (fx, fz) = (sx as f32, sz as f32);
            uvs.push(Vec2::new(i as f32 / fx, j as f32 / fz));
            uvs.push(Vec2::new((i + 1) as f32 / fx, j as f32 / fz));
            uvs.push(Vec2::new((i + 1) as f32 / fx, (j + 1) as f32 / fz));
            uvs.push(Vec2::new(i as f32 / fx, (j + 1) as f32 / fz));
        }
    }
    set_uv0(&mut m, &uvs);
    m.edge_attrs_mut().add(names::HARD, AttrType::Bool);
    m.edge_attrs_mut().add(names::SEAM, AttrType::Bool);
    m
}

pub fn make_sphere(o: &SphereOptions) -> Mesh {
    // The cut sits on a ring: the rings kept are spread evenly from it to the top pole, about as many as that arc had.
    let cut = if o.fraction >= 1.0 - 1e-9 {
        0.0
    } else {
        (2.0 * o.fraction.clamp(0.01, 1.0) - 1.0).acos()
    };
    let h = (if cut > 0.0 { 1 } else { 2 })
        .max((o.subdiv_height.max(2) as f64 * (PI - cut) / PI).round() as i32);
    let profile: Vec<ProfilePoint> = (0..=h)
        .map(|j| {
            let phi = cut + (PI - cut) * j as f64 / h as f64;
            pp(
                if j == h || (j == 0 && cut == 0.0) {
                    0.0
                } else {
                    o.radius * phi.sin()
                },
                -o.radius * phi.cos(),
            )
        })
        .collect();
    let mut m = lathe(
        &profile,
        &[cyl_band(0, h, Vec2::new(0.0, 1.0))],
        o.subdiv_axis.max(3),
        false,
        1.0,
        false,
    );
    if o.base_at_origin {
        shift_y(&mut m, o.radius * cut.cos());
    }
    m
}

pub fn make_cylinder(o: &CylinderOptions) -> Mesh {
    let hs = o.subdiv_height.max(1);
    let rings = o.subdiv_caps.max(1);
    let y0 = if o.base_at_origin {
        0.0
    } else {
        -0.5 * o.height
    };
    let mut profile = Vec::new();
    let mut bands = Vec::new();
    let scale = CAP_SCALE / o.radius as f32;
    let caps = o.caps && o.cap_fill != CapFill::None;
    if caps {
        for r in 0..rings {
            profile.push(pp(o.radius * r as f64 / rings as f64, y0));
        }
        bands.push(planar_band(0, rings, CAP_LEFT, scale, o.cap_fill));
    }
    let side_start = profile.len() as i32;
    for j in 0..=hs {
        profile.push(ProfilePoint {
            r: o.radius,
            y: y0 + o.height * j as f64 / hs as f64,
            hard: caps && (j == 0 || j == hs),
        });
    }
    bands.push(cyl_band(side_start, side_start + hs, Vec2::new(0.5, 1.0)));
    if caps {
        let top = profile.len() as i32 - 1;
        for r in (0..rings).rev() {
            profile.push(pp(o.radius * r as f64 / rings as f64, y0 + o.height));
        }
        bands.push(planar_band(top, top + rings, CAP_RIGHT, scale, o.cap_fill));
    }
    lathe(
        &profile,
        &bands,
        o.subdiv_axis.max(3),
        false,
        o.fraction,
        caps && o.cut_caps,
    )
}

pub fn make_capsule(o: &CapsuleOptions) -> Mesh {
    let caps = o.subdiv_caps.max(1);
    let hs = o.subdiv_height.max(1);
    let half = (0.5 * o.height - o.radius).max(0.0);
    let mut profile = Vec::new();
    for j in 0..=caps {
        let phi = 0.5 * PI * j as f64 / caps as f64;
        profile.push(pp(
            if j == 0 { 0.0 } else { o.radius * phi.sin() },
            -half - o.radius * phi.cos(),
        ));
    }
    if half > 0.0 {
        for j in 1..=hs {
            profile.push(pp(o.radius, -half + 2.0 * half * j as f64 / hs as f64));
        }
    }
    for j in 1..=caps {
        let phi = 0.5 * PI * j as f64 / caps as f64;
        profile.push(pp(
            if j == caps { 0.0 } else { o.radius * phi.cos() },
            half + o.radius * phi.sin(),
        ));
    }
    let n = profile.len() as i32 - 1;
    let mut m = lathe(
        &profile,
        &[cyl_band(0, n, Vec2::new(0.0, 1.0))],
        o.subdiv_axis.max(3),
        false,
        1.0,
        false,
    );
    if o.base_at_origin {
        shift_y(&mut m, half + o.radius);
    }
    m
}

pub fn make_cone(o: &ConeOptions) -> Mesh {
    let hs = o.subdiv_height.max(1);
    let cap = o.cap_fill != CapFill::None;
    let rings = if cap { o.subdiv_cap.max(1) } else { 0 };
    let y0 = if o.base_at_origin {
        0.0
    } else {
        -0.5 * o.height
    };
    let tip = if o.cap_tip {
        o.tip_radius.clamp(0.0, o.radius)
    } else {
        0.0
    };
    let tip_cap = cap && tip > 0.0;
    let mut profile = Vec::new();
    for r in 0..rings {
        profile.push(pp(o.radius * r as f64 / rings as f64, y0));
    }
    for j in 0..=hs {
        let t = j as f64 / hs as f64;
        profile.push(ProfilePoint {
            r: mixd(o.radius, tip, t),
            y: y0 + o.height * j as f64 / hs as f64,
            hard: (cap && j == 0) || (tip_cap && j == hs),
        });
    }
    let mut bands = vec![cyl_band(rings, rings + hs, Vec2::new(0.5, 1.0))];
    if cap {
        bands.insert(
            0,
            planar_band(0, rings, CAP_LEFT, CAP_SCALE / o.radius as f32, o.cap_fill),
        );
    }
    if tip_cap {
        profile.push(pp(0.0, y0 + o.height));
        bands.push(planar_band(
            rings + hs,
            rings + hs + 1,
            CAP_RIGHT,
            CAP_SCALE / tip as f32,
            o.cap_fill,
        ));
    }
    lathe(
        &profile,
        &bands,
        o.subdiv_axis.max(3),
        false,
        o.fraction,
        cap && o.cut_caps,
    )
}

pub fn make_torus(o: &TorusOptions) -> Mesh {
    let h = o.subdiv_height.max(3);
    let profile: Vec<ProfilePoint> = (0..h)
        .map(|j| {
            // Starting on the inside keeps the uv seam out of sight.
            let t = TAU * j as f64 / h as f64 + PI;
            pp(
                o.radius + o.section_radius * t.cos(),
                o.section_radius * t.sin(),
            )
        })
        .collect();
    lathe(
        &profile,
        &[cyl_band(0, h, Vec2::new(0.0, 1.0))],
        o.subdiv_axis.max(3),
        true,
        o.fraction,
        o.cut_caps,
    )
}

/// In XZ facing +Y.
pub fn make_disc(o: &DiscOptions) -> Mesh {
    let rs = o.subdiv_rings.max(0);
    let rings = rs.max(1);
    let profile: Vec<ProfilePoint> = (0..=rings)
        .rev()
        .map(|r| pp(o.radius * r as f64 / rings as f64, 0.0))
        .collect();
    let fill = if rs == 0 {
        CapFill::Ngon
    } else if o.cap_fill == CapFill::None {
        CapFill::Fan
    } else {
        o.cap_fill
    };
    lathe(
        &profile,
        &[planar_band(
            0,
            rings,
            Vec2::splat(0.5),
            0.5 / o.radius as f32,
            fill,
        )],
        o.subdiv_axis.max(3),
        false,
        o.fraction,
        false,
    )
}

pub fn make_pipe(o: &PipeOptions) -> Mesh {
    let hs = o.subdiv_height.max(1);
    let y0 = if o.base_at_origin {
        0.0
    } else {
        -0.5 * o.height
    };
    let ri = (o.radius - o.thickness).max(0.0);
    let mut profile = Vec::new();
    for j in 0..=hs {
        profile.push(ProfilePoint {
            r: o.radius,
            y: y0 + o.height * j as f64 / hs as f64,
            hard: j == 0 || j == hs,
        });
    }
    for j in (0..=hs).rev() {
        profile.push(ProfilePoint {
            r: ri,
            y: y0 + o.height * j as f64 / hs as f64,
            hard: j == 0 || j == hs,
        });
    }
    let inner_top = hs + 1;
    let inner_bottom = 2 * hs + 1;
    let scale = CAP_SCALE / o.radius as f32;
    let bands = [
        cyl_band(0, hs, Vec2::new(0.5, 0.75)),
        planar_band(hs, inner_top, CAP_RIGHT, scale, CapFill::Fan),
        cyl_band(inner_top, inner_bottom, Vec2::new(0.75, 1.0)),
        planar_band(
            inner_bottom,
            inner_bottom + 1,
            CAP_LEFT,
            scale,
            CapFill::Fan,
        ),
    ];
    lathe(
        &profile,
        &bands,
        o.subdiv_axis.max(3),
        true,
        o.fraction,
        o.cut_caps,
    )
}

// ---------------------------------------------------------------------------------------------------------------------
// Prism, pyramid, helix, platonic solids, soccer ball, super shape

/// Corner UVs from a planar view down Y, fitted to 0..1.
fn planar_uvs(m: &mut Mesh) {
    let b = m.bounds();
    let size = b.size();
    let (sx, sz) = (size.x.max(1e-9), size.z.max(1e-9));
    let corners: Vec<(Index, DVec3)> = (0..m.corner_capacity()).filter(|&c| m.corner_valid(c)).map(|c| (c, m.position(m.corner_point(c)))).collect();
    let uv = m.corner_attrs_mut().get_or_add::<[f32; 2]>(names::uv(0).as_str());
    for (c, p) in corners {
        uv[c as usize] = [((p.x - b.min.x) / sx) as f32, ((p.z - b.min.z) / sz) as f32];
    }
}

fn polygons(points: &[DVec3], faces: &[Vec<Index>]) -> Mesh {
    let sizes: Vec<Index> = faces.iter().map(|f| f.len() as Index).collect();
    let indices: Vec<Index> = faces.iter().flatten().copied().collect();
    let mut m = Mesh::from_polygons(points, &sizes, &indices);
    planar_uvs(&mut m);
    m
}

/// Rings of quads closing an n-gon cap: rings of points shrink to the middle, the innermost closed by one n-gon.
/// first is the rim's first point id, n its count; flip reverses the winding.
fn cap_rings(points: &mut Vec<DVec3>, faces: &mut Vec<Vec<Index>>, first: Index, n: Index, rings: i32, flip: bool) {
    let mut rim: Vec<Index> = (first..first + n).collect();
    let center: DVec3 = rim.iter().map(|&p| points[p as usize]).sum::<DVec3>() / n as f64;
    let add = |mut f: Vec<Index>, faces: &mut Vec<Vec<Index>>| {
        if flip {
            f.reverse();
        }
        faces.push(f);
    };
    for r in 1..rings.max(1) {
        let t = r as f64 / rings as f64;
        let start = points.len() as Index;
        for i in 0..n {
            let q = points[(first + i) as usize];
            points.push(q.lerp(center, t));
        }
        let inner: Vec<Index> = (start..start + n).collect();
        for i in 0..n as usize {
            let j = (i + 1) % n as usize;
            add(vec![rim[i], rim[j], inner[j], inner[i]], faces);
        }
        rim = inner;
    }
    add(rim, faces);
}

/// polyPrism: an n-sided column, length along Y, sides side_length wide.
pub fn make_prism(length: f64, side_length: f64, sides: i32, sub_height: i32, sub_caps: i32) -> Mesh {
    let n = sides.max(3) as Index;
    let rows = sub_height.max(1) as Index;
    let radius = side_length / (2.0 * (PI / n as f64).sin());
    let mut points = Vec::new();
    for r in 0..=rows {
        let y = length * (r as f64 / rows as f64 - 0.5);
        for i in 0..n {
            let a = TAU * i as f64 / n as f64;
            points.push(DVec3::new(radius * a.cos(), y, -radius * a.sin()));
        }
    }
    let mut faces = Vec::new();
    for r in 0..rows {
        for i in 0..n {
            let j = (i + 1) % n;
            faces.push(vec![r * n + i, r * n + j, (r + 1) * n + j, (r + 1) * n + i]);
        }
    }
    cap_rings(&mut points, &mut faces, 0, n, sub_caps, true);
    cap_rings(&mut points, &mut faces, rows * n, n, sub_caps, false);
    polygons(&points, &faces)
}

/// polyPyramid: an n-sided base with sides side_length long and one tip, centred on the origin.
pub fn make_pyramid(side_length: f64, sides: i32, sub_height: i32, sub_caps: i32) -> Mesh {
    let n = sides.max(3) as Index;
    let rows = sub_height.max(1) as Index;
    let radius = side_length / (2.0 * (PI / n as f64).sin());
    let height = (side_length * side_length - radius * radius).max(side_length * side_length * 0.25).sqrt();
    let mut points = Vec::new();
    for r in 0..rows {
        let t = r as f64 / rows as f64;
        for i in 0..n {
            let a = TAU * i as f64 / n as f64 + PI / n as f64;
            points.push(DVec3::new(radius * (1.0 - t) * a.cos(), height * (t - 0.5), -radius * (1.0 - t) * a.sin()));
        }
    }
    let tip = points.len() as Index;
    points.push(DVec3::new(0.0, height * 0.5, 0.0));
    let mut faces = Vec::new();
    for r in 0..rows {
        for i in 0..n {
            let j = (i + 1) % n;
            if r + 1 == rows {
                faces.push(vec![r * n + i, r * n + j, tip]);
            } else {
                faces.push(vec![r * n + i, r * n + j, (r + 1) * n + j, (r + 1) * n + i]);
            }
        }
    }
    cap_rings(&mut points, &mut faces, 0, n, sub_caps, true);
    polygons(&points, &faces)
}

/// polyHelix: a tube of section radius wound coils times over height, the whole width across. direction 1 is
/// counterclockwise going up.
#[allow(clippy::too_many_arguments)]
pub fn make_helix(coils: f64, height: f64, width: f64, radius: f64, direction: i32, sub_axis: i32, sub_coil: i32, sub_caps: i32) -> Mesh {
    let around = sub_axis.max(3) as Index;
    let steps = (sub_coil.max(1) as f64 * coils.max(0.01)).round().max(1.0) as Index;
    let sign = if direction == 0 { -1.0 } else { 1.0 };
    let mut points = Vec::new();
    for s in 0..=steps {
        let t = s as f64 / steps as f64;
        let a = sign * TAU * coils * t;
        let center = DVec3::new(width * 0.5 * a.cos(), height * (t - 0.5), -width * 0.5 * a.sin());
        let out = DVec3::new(a.cos(), 0.0, -a.sin());
        let along = DVec3::new(-sign * width * 0.5 * TAU * coils * a.sin(), height, -sign * width * 0.5 * TAU * coils * a.cos()).normalize_or_zero();
        let side = along.cross(out).normalize_or_zero();
        let up = side.cross(along).normalize_or_zero();
        for i in 0..around {
            let b = TAU * i as f64 / around as f64;
            points.push(center + (up * b.cos() + side * b.sin()) * radius);
        }
    }
    let mut faces = Vec::new();
    for s in 0..steps {
        for i in 0..around {
            let j = (i + 1) % around;
            faces.push(vec![s * around + i, s * around + j, (s + 1) * around + j, (s + 1) * around + i]);
        }
    }
    cap_rings(&mut points, &mut faces, 0, around, sub_caps, true);
    cap_rings(&mut points, &mut faces, steps * around, around, sub_caps, false);
    polygons(&points, &faces)
}

fn icosahedron() -> (Vec<DVec3>, Vec<[Index; 3]>) {
    let t = (1.0 + 5f64.sqrt()) * 0.5;
    let raw = [
        (-1.0, t, 0.0), (1.0, t, 0.0), (-1.0, -t, 0.0), (1.0, -t, 0.0),
        (0.0, -1.0, t), (0.0, 1.0, t), (0.0, -1.0, -t), (0.0, 1.0, -t),
        (t, 0.0, -1.0), (t, 0.0, 1.0), (-t, 0.0, -1.0), (-t, 0.0, 1.0),
    ];
    let pts = raw.iter().map(|&(x, y, z)| DVec3::new(x, y, z).normalize()).collect();
    let tris = vec![
        [0, 11, 5], [0, 5, 1], [0, 1, 7], [0, 7, 10], [0, 10, 11],
        [1, 5, 9], [5, 11, 4], [11, 10, 2], [10, 7, 6], [7, 1, 8],
        [3, 9, 4], [3, 4, 2], [3, 2, 6], [3, 6, 8], [3, 8, 9],
        [4, 9, 5], [2, 4, 11], [6, 2, 10], [8, 6, 7], [9, 8, 1],
    ];
    (pts, tris)
}

/// The triangles round each vertex of a closed triangle mesh, walked in order.
fn fans(n: usize, tris: &[[Index; 3]]) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    for v in 0..n as Index {
        let mut fan = Vec::new();
        let start = tris.iter().position(|t| t.contains(&v)).unwrap();
        let mut cur = start;
        loop {
            fan.push(cur);
            let t = tris[cur];
            let i = t.iter().position(|&x| x == v).unwrap();
            let next_v = t[(i + 2) % 3];
            // The triangle across the edge v - next_v runs it the other way.
            cur = tris.iter().position(|o| (0..3).any(|k| o[k] == v && o[(k + 1) % 3] == next_v)).unwrap();
            if cur == start {
                break;
            }
        }
        out.push(fan);
    }
    out
}

fn dodecahedron() -> (Vec<DVec3>, Vec<Vec<Index>>) {
    let (pts, tris) = icosahedron();
    let centers: Vec<DVec3> = tris.iter().map(|t| (pts[t[0] as usize] + pts[t[1] as usize] + pts[t[2] as usize]).normalize()).collect();
    let faces = fans(pts.len(), &tris).into_iter().map(|f| f.into_iter().map(|i| i as Index).collect()).collect();
    (centers, faces)
}

/// polyPlatonicSolid: 0 dodecahedron, 1 icosahedron, 2 octahedron, 3 tetrahedron, points radius from the centre.
pub fn make_platonic(solid: i32, radius: f64) -> Mesh {
    let (pts, faces): (Vec<DVec3>, Vec<Vec<Index>>) = match solid {
        1 => {
            let (p, t) = icosahedron();
            (p, t.iter().map(|x| x.to_vec()).collect())
        }
        2 => (
            vec![DVec3::X, DVec3::Y, DVec3::Z, DVec3::NEG_X, DVec3::NEG_Y, DVec3::NEG_Z],
            vec![vec![0, 1, 2], vec![2, 1, 3], vec![3, 1, 5], vec![5, 1, 0], vec![2, 4, 0], vec![3, 4, 2], vec![5, 4, 3], vec![0, 4, 5]],
        ),
        3 => (
            [DVec3::new(1.0, 1.0, 1.0), DVec3::new(-1.0, -1.0, 1.0), DVec3::new(-1.0, 1.0, -1.0), DVec3::new(1.0, -1.0, -1.0)].iter().map(|v| v.normalize()).collect(),
            vec![vec![0, 2, 1], vec![0, 1, 3], vec![0, 3, 2], vec![1, 2, 3]],
        ),
        _ => dodecahedron(),
    };
    let scaled: Vec<DVec3> = pts.iter().map(|p| *p * radius).collect();
    polygons(&scaled, &faces)
}

/// The soccer ball: a truncated icosahedron, 12 pentagons and 20 hexagons, points radius from the centre.
pub fn make_soccer_ball(radius: f64) -> Mesh {
    let (pts, tris) = icosahedron();
    let mut out: Vec<DVec3> = Vec::new();
    let mut keyed: BTreeMap<(Index, Index), Index> = BTreeMap::new();
    let mut at = |a: Index, b: Index| -> Index {
        *keyed.entry((a, b)).or_insert_with(|| {
            out.push(pts[a as usize].lerp(pts[b as usize], 1.0 / 3.0).normalize() * radius);
            (out.len() - 1) as Index
        })
    };
    let mut faces = Vec::new();
    for t in &tris {
        let [a, b, c] = *t;
        faces.push(vec![at(a, b), at(b, a), at(b, c), at(c, b), at(c, a), at(a, c)]);
    }
    for (v, fan) in fans(pts.len(), &tris).into_iter().enumerate() {
        let v = v as Index;
        let mut face = Vec::new();
        for ti in fan {
            let t = tris[ti];
            let i = t.iter().position(|&x| x == v).unwrap();
            face.push(at(v, t[(i + 1) % 3]));
        }
        faces.push(face);
    }
    polygons(&out, &faces)
}

/// One side of a superformula: how far out the shape reaches at angle a.
fn superformula(a: f64, m: f64, n1: f64, n2: f64, n3: f64) -> f64 {
    let p = (m * a / 4.0).cos().abs().powf(n2) + (m * a / 4.0).sin().abs().powf(n3);
    if p <= 0.0 {
        return 1.0;
    }
    p.powf(-1.0 / n1.max(1e-6))
}

/// polySuperShape: the spherical product of two superformulas, closed at its poles.
pub fn make_super_shape(radius: f64, around: i32, down: i32, h: [f64; 4], v: [f64; 4]) -> Mesh {
    let n = around.max(3) as Index;
    let rows = down.max(2) as Index;
    let mut points = vec![];
    let surface = |theta: f64, phi: f64| {
        let r1 = superformula(theta, h[0], h[1], h[2], h[3]);
        let r2 = superformula(phi, v[0], v[1], v[2], v[3]);
        DVec3::new(r1 * theta.cos() * r2 * phi.cos(), r2 * phi.sin(), -r1 * theta.sin() * r2 * phi.cos()) * radius
    };
    points.push(surface(0.0, -FRAC_PI_2));
    for r in 1..rows {
        let phi = -FRAC_PI_2 + PI * r as f64 / rows as f64;
        for i in 0..n {
            points.push(surface(TAU * i as f64 / n as f64, phi));
        }
    }
    let top = points.len() as Index;
    points.push(surface(0.0, FRAC_PI_2));
    let ring = |r: Index, i: Index| 1 + (r - 1) * n + i % n;
    let mut faces = Vec::new();
    for i in 0..n {
        faces.push(vec![0, ring(1, i + 1), ring(1, i)]);
        faces.push(vec![top, ring(rows - 1, i), ring(rows - 1, i + 1)]);
    }
    for r in 1..rows - 1 {
        for i in 0..n {
            faces.push(vec![ring(r, i), ring(r, i + 1), ring(r + 1, i + 1), ring(r + 1, i)]);
        }
    }
    polygons(&points, &faces)
}
