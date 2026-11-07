//! Repeating a mesh by a layout, and the per-copy transform maths shared by the array modifier, the live array group
//! and duplicate count=. Matrices are column vectors.

use std::collections::{BTreeMap, HashMap};

use glam::{DMat3, DMat4, DVec3, IVec3, Mat3};

use crate::attribute::{names, Index};
use crate::mesh::Mesh;
use crate::ops::basic::weld_points;
use crate::topology::edges_of_faces;

/// Scale, then Euler XYZ rotation (degrees, X applied first), then translate.
pub fn trs_matrix(translate: DVec3, rotate: DVec3, scale: DVec3) -> DMat4 {
    DMat4::from_translation(translate)
        * DMat4::from_axis_angle(DVec3::Z, rotate.z.to_radians())
        * DMat4::from_axis_angle(DVec3::Y, rotate.y.to_radians())
        * DMat4::from_axis_angle(DVec3::X, rotate.x.to_radians())
        * DMat4::from_scale(scale)
}

/// `local` applied in `frame`'s space (its origin the pivot, its axes the step's axes): frame * local * frame^-1.
pub fn about_frame(frame: &DMat4, local: &DMat4) -> DMat4 {
    *frame * *local * frame.inverse()
}

/// A frame from a pivot point and axes (the columns of `axes`).
pub fn pivot_frame(pivot: DVec3, axes: &DMat3) -> DMat4 {
    let mut f = DMat4::from_mat3(*axes);
    f.w_axis = pivot.extend(1.0);
    f
}

/// step^1 * base .. step^count * base: each repeat applies the step once more on the left.
pub fn repeat_matrices(base: &DMat4, step: &DMat4, count: i32) -> Vec<DMat4> {
    let mut out = Vec::with_capacity(count.max(0) as usize);
    let mut cur = *base;
    for _ in 0..count {
        cur = *step * cur;
        out.push(cur);
    }
    out
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ArrayMode {
    #[default]
    Linear,
    Radial,
    Grid,
}

/// Where array copies go, in the space of the mesh being repeated. Linear and radial copies each apply one step
/// transform to the previous copy about the centre frame, so a linear step with a rotation curves and a radial step
/// with a rise climbs a helix. Grid copies are translations only.
#[derive(Clone, Debug)]
pub struct ArrayLayout {
    pub mode: ArrayMode,
    /// Copies including the original, linear and radial.
    pub count: i32,
    /// Copies including the original along each axis, grid.
    pub grid_count: IVec3,
    /// Step between copies (linear), or spacing along each axis (grid), in the centre frame.
    pub offset: DVec3,
    /// Extra Euler XYZ rotation per copy, degrees, about the centre (linear and radial).
    pub rotate: DVec3,
    /// Scale per copy, about the centre (linear and radial).
    pub scale: DVec3,
    /// Radial: the centre frame's axis the copies turn about, 0 x, 1 y, 2 z.
    pub axis: i32,
    /// Radial: the angle the copies span (degrees), or with step_angle the angle between neighbours.
    pub angle: f64,
    pub step_angle: bool,
    /// Radial: distance each copy climbs along the axis, for helices.
    pub rise: f64,
    /// The centre frame (origin is the pivot, axes orient radial and grid layouts), in mesh space.
    pub frame: DMat4,
}

impl Default for ArrayLayout {
    fn default() -> Self {
        Self {
            mode: ArrayMode::Linear,
            count: 3,
            grid_count: IVec3::new(3, 3, 1),
            offset: DVec3::ZERO,
            rotate: DVec3::ZERO,
            scale: DVec3::ONE,
            axis: 1,
            angle: 360.0,
            step_angle: false,
            rise: 0.0,
            frame: DMat4::IDENTITY,
        }
    }
}

/// Copies the layout makes, the original included.
pub fn array_copy_count(l: &ArrayLayout) -> i32 {
    if l.mode == ArrayMode::Grid {
        return l.grid_count.x.max(1) * l.grid_count.y.max(1) * l.grid_count.z.max(1);
    }
    l.count.max(1)
}

/// True when a radial layout spans a whole turn (by total angle, or step angle times count), without rise.
pub fn array_closes_ring(l: &ArrayLayout) -> bool {
    if l.mode != ArrayMode::Radial || l.count < 2 || l.rise.abs() > 0.0 {
        return false;
    }
    let span = if l.step_angle { l.angle.abs() * l.count as f64 } else { l.angle.abs() };
    (span - 360.0).abs() < 1e-6
}

/// Degrees between neighbouring radial copies: a full turn shares out over count, a partial arc puts copies at both
/// ends.
pub fn array_step_angle(l: &ArrayLayout) -> f64 {
    if l.step_angle {
        return l.angle;
    }
    let n = l.count.max(1);
    if (l.angle.abs() - 360.0).abs() < 1e-6 {
        return l.angle / n as f64;
    }
    if n > 1 {
        l.angle / (n - 1) as f64
    } else {
        0.0
    }
}

/// The transform from one linear or radial copy to the next, in the centre frame.
pub fn array_step(l: &ArrayLayout) -> DMat4 {
    let mut step = DMat4::IDENTITY;
    if l.mode == ArrayMode::Radial {
        let mut axis = DVec3::ZERO;
        axis[l.axis.clamp(0, 2) as usize] = 1.0;
        step = DMat4::from_axis_angle(axis, array_step_angle(l).to_radians()) * DMat4::from_translation(axis * l.rise);
    }
    step * trs_matrix(l.offset, l.rotate, l.scale)
}

/// Grid cell of copy k: x varies fastest.
pub fn array_grid_cell(l: &ArrayLayout, k: i32) -> IVec3 {
    let nx = l.grid_count.x.max(1);
    let ny = l.grid_count.y.max(1);
    IVec3::new(k % nx, (k / nx) % ny, k / (nx * ny))
}

/// Copy k's transform in mesh space; copy 0 is the original.
pub fn array_copy_transform(l: &ArrayLayout, k: i32) -> DMat4 {
    let mut local = DMat4::IDENTITY;
    if l.mode == ArrayMode::Grid {
        local = DMat4::from_translation(l.offset * array_grid_cell(l, k).as_dvec3());
    } else {
        let step = array_step(l);
        for _ in 0..k {
            local = local * step;
        }
    }
    about_frame(&l.frame, &local)
}

/// Every copy's transform in mesh space, copy 0 (the original, identity) first.
pub fn array_copy_transforms(l: &ArrayLayout) -> Vec<DMat4> {
    let n = array_copy_count(l);
    if l.mode == ArrayMode::Grid {
        return (0..n).map(|k| array_copy_transform(l, k)).collect();
    }
    let mut out = vec![DMat4::IDENTITY];
    let step = about_frame(&l.frame, &array_step(l));
    out.extend(repeat_matrices(&DMat4::IDENTITY, &step, n - 1));
    out
}

#[derive(Clone, Debug)]
pub struct ArrayOptions {
    pub layout: ArrayLayout,
    /// Merge points of neighbouring copies that land within tolerance, and drop the walls two copies share.
    pub weld: bool,
    /// Also weld the last copy to the first. A radial layout spanning a whole turn always does.
    pub merge_ends: bool,
    pub tolerance: f64,
}

impl Default for ArrayOptions {
    fn default() -> Self {
        Self { layout: ArrayLayout::default(), weld: false, merge_ends: false, tolerance: 0.01 }
    }
}

fn find_root(parent: &mut [Index], mut p: Index) -> Index {
    while parent[p as usize] != p {
        parent[p as usize] = parent[parent[p as usize] as usize];
        p = parent[p as usize];
    }
    p
}

/// True when copies a and b sit next to each other in the layout, so their seams may weld.
fn adjacent_copies(o: &ArrayOptions, copies: i32, a: i32, b: i32) -> bool {
    let l = &o.layout;
    if l.mode == ArrayMode::Grid {
        let d = (array_grid_cell(l, a) - array_grid_cell(l, b)).abs();
        return d.x + d.y + d.z == 1;
    }
    if (a - b).abs() == 1 {
        return true;
    }
    let ends = o.merge_ends || array_closes_ring(l);
    ends && copies > 2 && a.min(b) == 0 && a.max(b) == copies - 1
}

/// Removes faces that two different copies both put on the same welded points: the seam walls of a closed wedge,
/// which would otherwise be left inside the ring as non-manifold faces. Edges left without faces go too; returns the
/// walls' points, which may be left loose once welded.
fn remove_shared_walls(m: &mut Mesh, root: &[Index], per_copy: Index) -> Vec<Index> {
    let mut by_points: BTreeMap<Vec<Index>, Vec<Index>> = BTreeMap::new();
    for f in 0..m.face_capacity() {
        if !m.face_valid(f) {
            continue;
        }
        let mut key: Vec<Index> = m.face_points(f).iter().map(|&p| root[p as usize]).collect();
        key.sort_unstable();
        by_points.entry(key).or_default().push(f);
    }
    let mut walls = Vec::new();
    for faces in by_points.values() {
        if faces.len() < 2 {
            continue;
        }
        let copy = m.face_points(faces[0])[0] / per_copy;
        if faces.iter().any(|&f| m.face_points(f)[0] / per_copy != copy) {
            walls.extend_from_slice(faces);
        }
    }
    let points: Vec<Index> = walls.iter().flat_map(|&f| m.face_points(f)).collect();
    let edges = edges_of_faces(m, &walls);
    for &f in &walls {
        m.remove_face(f);
    }
    m.remove_loose(&edges, &[]);
    points
}

fn weld_copies(m: &mut Mesh, o: &ArrayOptions, copies: i32, per_copy: Index) {
    let tol = o.tolerance.max(1e-9);
    let n = m.point_capacity();
    let cell_of = |p: DVec3| (p / tol).floor().as_ivec3();
    let mut grid: HashMap<IVec3, Vec<Index>> = HashMap::new();
    for p in 0..n {
        if m.point_valid(p) {
            grid.entry(cell_of(m.position(p))).or_default().push(p);
        }
    }

    let mut parent: Vec<Index> = (0..n).collect();
    let mut any = false;
    for p in 0..n {
        if !m.point_valid(p) {
            continue;
        }
        let pos = m.position(p);
        let c = cell_of(pos);
        let copy_p = (p / per_copy) as i32;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = grid.get(&(c + IVec3::new(dx, dy, dz))) else { continue };
                    for &q in list {
                        // Each pair once, and never two points of the same copy: those coincide by the modeller's
                        // choice.
                        let copy_q = (q / per_copy) as i32;
                        if q <= p || copy_q == copy_p || !adjacent_copies(o, copies, copy_p, copy_q) {
                            continue;
                        }
                        if pos.distance(m.position(q)) > tol {
                            continue;
                        }
                        let a = find_root(&mut parent, p);
                        let b = find_root(&mut parent, q);
                        if a != b {
                            parent[a.max(b) as usize] = a.min(b);
                            any = true;
                        }
                    }
                }
            }
        }
    }
    if !any {
        return;
    }
    let root: Vec<Index> = (0..n).map(|p| find_root(&mut parent, p)).collect();
    let wall_points: Vec<Index> = remove_shared_walls(m, &root, per_copy).iter().map(|&p| root[p as usize]).collect();
    weld_points(m, &root);
    m.remove_loose(&[], &wall_points);
}

/// Mesh copied into place per copy transform, with all attributes. Mirroring copies have their faces flipped.
pub fn transform_copy(mesh: &mut Mesh, xf: &DMat4) {
    for p in mesh.positions_mut() {
        *p = xf.transform_point3(*p);
    }
    let nm: Mat3 = DMat3::from_mat4(*xf).inverse().transpose().as_mat3();
    for n in mesh.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL) {
        let v = (nm * glam::Vec3::from(*n)).normalize();
        *n = v.into();
    }
    if DMat3::from_mat4(*xf).determinant() < 0.0 {
        for f in 0..mesh.face_capacity() {
            if mesh.face_valid(f) {
                mesh.flip_face(f);
            }
        }
    }
}

/// The mesh repeated by the layout: copy 0 is the input, the rest follow in layout order. With weld on, seams between
/// neighbouring copies merge and faces two copies both put on a seam (the cut walls of a modelled wedge) are removed,
/// so a closed wedge arrayed round a full turn becomes one closed, manifold ring.
pub fn array_mesh(input: &Mesh, opts: &ArrayOptions) -> Mesh {
    let mut base = input.clone();
    base.compact(None);
    let mut out = base.clone();
    let l = &opts.layout;
    let copies = array_copy_count(l);
    let per_copy = base.point_capacity();
    if copies < 2 || per_copy == 0 {
        return out;
    }
    let step = array_step(l);
    let frame_inv = l.frame.inverse();
    let mut local = DMat4::IDENTITY;
    for k in 1..copies {
        let xf = if l.mode == ArrayMode::Grid {
            array_copy_transform(l, k)
        } else {
            local = local * step;
            l.frame * local * frame_inv
        };
        let mut copy = base.clone();
        transform_copy(&mut copy, &xf);
        out.append(&copy);
    }
    if opts.weld {
        weld_copies(&mut out, opts, copies, per_copy);
        out.compact(None);
    }
    out
}
