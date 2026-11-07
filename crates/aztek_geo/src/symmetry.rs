//! Mirror and radial symmetry of a mesh's points: building the maps, keeping them on the mesh, enforcing them.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::f64::consts::PI;
use std::sync::LazyLock;

use glam::{DMat3, DVec3};
use rayon::prelude::*;

use crate::attribute::{Index, INVALID};
use crate::bvh::MeshBvh;
use crate::mesh::Mesh;
use crate::topology::unique_sorted;

/// Points p with dot(normal, p) == offset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SymmetryPlane {
    pub normal: DVec3,
    pub offset: f64,
}

impl Default for SymmetryPlane {
    fn default() -> Self {
        Self { normal: DVec3::X, offset: 0.0 }
    }
}

impl SymmetryPlane {
    pub fn distance(&self, p: DVec3) -> f64 {
        self.normal.dot(p) - self.offset
    }
    pub fn reflect(&self, p: DVec3) -> DVec3 {
        p - 2.0 * self.distance(p) * self.normal
    }
    pub fn project(&self, p: DVec3) -> DVec3 {
        p - self.distance(p) * self.normal
    }
}

/// Plane normal to axis 0, 1 or 2 at offset along it (an object's pivot). Reflecting across it maps one coordinate v
/// to 2 * offset - v, which is exact at offset 0.
pub fn axis_plane(axis: i32, offset: f64) -> SymmetryPlane {
    let mut normal = DVec3::ZERO;
    normal[axis.clamp(0, 2) as usize] = 1.0;
    SymmetryPlane { normal, offset }
}

/// Largest radial count: the persistent layer packs the element into 8 bits.
pub const MAX_RADIAL_COUNT: i32 = 64;
/// Key of an on-plane (or on-axis) point in a symmetry layer.
pub const SYMMETRY_ON_PLANE: i32 = i32::MIN;
const SLOT_BITS: i64 = 256;

/// Values this close to 0, 1/2 or 1 are those exactly, so quarter and sixth turns about a coordinate axis stay exact.
fn snap_unit(v: f64) -> f64 {
    for t in [0.0, 0.5, 1.0, -0.5, -1.0] {
        if (v - t).abs() < 1e-12 {
            return t;
        }
    }
    v
}

fn snap_unit3(v: DVec3) -> DVec3 {
    DVec3::new(snap_unit(v.x), snap_unit(v.y), snap_unit(v.z))
}

fn coordinate_axis(v: DVec3) -> i32 {
    for k in 0..3 {
        if v[k] == 1.0 && v[(k + 1) % 3] == 0.0 && v[(k + 2) % 3] == 0.0 {
            return k as i32;
        }
    }
    -1
}

fn rotation(a: DVec3, angle: f64) -> DMat3 {
    let c = snap_unit(angle.cos());
    let s = snap_unit(angle.sin());
    let cross = [[0.0, a.z, -a.y], [-a.z, 0.0, a.x], [a.y, -a.x, 0.0]];
    let mut r = [[0.0f64; 3]; 3];
    for col in 0..3 {
        for row in 0..3 {
            r[col][row] =
                snap_unit(if col == row { c } else { 0.0 } + s * cross[col][row] + (1.0 - c) * a[row] * a[col]);
        }
    }
    DMat3::from_cols_array_2d(&r)
}

/// count-fold rotation about an axis through origin, and with mirror a mirror plane through the axis per wedge as
/// well (dihedral symmetry). Element g < count turns by g wedges; element count + k reflects across the reference
/// plane (the plane holding the axis and the reference direction) and then turns by k wedges. Element 0 is the
/// identity.
#[derive(Clone, Debug)]
pub struct RadialSymmetry {
    origin: DVec3,
    axis: DVec3,
    reference: DVec3,
    count: i32,
    mirror: bool,
    axis_index: i32,
    linear: Vec<DMat3>,
}

impl Default for RadialSymmetry {
    fn default() -> Self {
        Self {
            origin: DVec3::ZERO,
            axis: DVec3::Y,
            reference: DVec3::Z,
            count: 1,
            mirror: false,
            axis_index: 1,
            linear: vec![DMat3::IDENTITY],
        }
    }
}

impl PartialEq for RadialSymmetry {
    fn eq(&self, o: &Self) -> bool {
        self.origin == o.origin
            && self.axis == o.axis
            && self.reference == o.reference
            && self.count == o.count
            && self.mirror == o.mirror
    }
}

impl RadialSymmetry {
    /// reference is made perpendicular to axis; a zero or parallel one picks a direction of its own. count is clamped
    /// to 1..=MAX_RADIAL_COUNT.
    pub fn new(origin: DVec3, axis: DVec3, count: i32, mirror: bool, reference: DVec3) -> Self {
        let count = count.clamp(1, MAX_RADIAL_COUNT);
        let axis = if axis.length() > 1e-12 { snap_unit3(axis.normalize()) } else { DVec3::Y };
        let axis_index = coordinate_axis(axis);
        let r = reference - axis * reference.dot(axis);
        let reference = if r.length() > 1e-9 { snap_unit3(r.normalize()) } else { radial_reference(axis, 0.0) };
        let mut linear: Vec<DMat3> = (0..count).map(|k| rotation(axis, 2.0 * PI * k as f64 / count as f64)).collect();
        if mirror {
            let m = axis.cross(reference).normalize();
            let mut s = [[0.0f64; 3]; 3];
            for col in 0..3 {
                for row in 0..3 {
                    s[col][row] = snap_unit(if col == row { 1.0 } else { 0.0 } - 2.0 * m[row] * m[col]);
                }
            }
            let s = DMat3::from_cols_array_2d(&s);
            for k in 0..count as usize {
                let r = linear[k] * s;
                linear.push(DMat3::from_cols(snap_unit3(r.x_axis), snap_unit3(r.y_axis), snap_unit3(r.z_axis)));
            }
        }
        Self { origin, axis, reference, count, mirror, axis_index, linear }
    }

    pub fn count(&self) -> i32 {
        self.count
    }
    pub fn mirror(&self) -> bool {
        self.mirror
    }
    /// Number of group elements: count, or twice that with mirror.
    pub fn order(&self) -> i32 {
        if self.mirror {
            2 * self.count
        } else {
            self.count
        }
    }
    pub fn origin(&self) -> DVec3 {
        self.origin
    }
    pub fn axis(&self) -> DVec3 {
        self.axis
    }
    pub fn reference(&self) -> DVec3 {
        self.reference
    }
    /// 0, 1 or 2 when the axis is a coordinate axis, else -1.
    pub fn axis_index(&self) -> i32 {
        self.axis_index
    }
    pub fn reflects(&self, g: i32) -> bool {
        g >= self.count
    }

    /// The element that applies b, then a.
    pub fn compose(&self, a: i32, b: i32) -> i32 {
        let ra = a >= self.count;
        let rb = b >= self.count;
        let ka = if ra { a - self.count } else { a };
        let kb = if rb { b - self.count } else { b };
        let k = ((if ra { ka - kb } else { ka + kb }) % self.count + self.count) % self.count;
        if ra != rb {
            self.count + k
        } else {
            k
        }
    }

    pub fn inverse(&self, g: i32) -> i32 {
        if g >= self.count {
            g
        } else {
            (self.count - g) % self.count
        }
    }

    pub fn linear(&self, g: i32) -> &DMat3 {
        &self.linear[g as usize]
    }

    pub fn apply(&self, g: i32, p: DVec3) -> DVec3 {
        if g == 0 {
            p
        } else {
            self.origin + self.linear[g as usize] * (p - self.origin)
        }
    }

    pub fn apply_vector(&self, g: i32, v: DVec3) -> DVec3 {
        self.linear[g as usize] * v
    }

    /// Radians about the axis from the reference direction, in [0, 2 pi).
    pub fn angle(&self, p: DVec3) -> f64 {
        let d = p - self.origin;
        let mut a = d.dot(self.axis.cross(self.reference)).atan2(d.dot(self.reference));
        if a < 0.0 {
            a += 2.0 * PI;
        }
        if a >= 2.0 * PI {
            0.0
        } else {
            a
        }
    }

    /// The angle a wedge spans: 2 pi / count, or pi / count with mirror (the fundamental wedge).
    pub fn wedge_angle(&self) -> f64 {
        (if self.mirror { PI } else { 2.0 * PI }) / self.count as f64
    }

    /// The element that carries the fundamental wedge onto the one holding p.
    pub fn wedge_of(&self, p: DVec3) -> i32 {
        let hi = if self.mirror { 2 * self.count - 1 } else { self.count - 1 };
        let sector = ((self.angle(p) / self.wedge_angle()) as i32).clamp(0, hi);
        if !self.mirror {
            return sector;
        }
        // Even sectors are the fundamental wedge turned; odd ones its reflection, which runs the angle backward.
        if sector % 2 == 0 {
            sector / 2
        } else {
            self.count + ((sector + 1) / 2) % self.count
        }
    }

    pub fn axis_distance(&self, p: DVec3) -> f64 {
        let d = p - self.origin;
        (d - self.axis * d.dot(self.axis)).length()
    }

    pub fn project_to_axis(&self, p: DVec3) -> DVec3 {
        if self.axis_index >= 0 {
            let mut out = self.origin;
            out[self.axis_index as usize] = p[self.axis_index as usize];
            return out;
        }
        self.origin + self.axis * (p - self.origin).dot(self.axis)
    }

    /// The plane a reflecting element mirrors across, through the axis.
    pub fn mirror_plane(&self, g: i32) -> SymmetryPlane {
        let k = if g >= self.count { g - self.count } else { g };
        let a = PI * k as f64 / self.count as f64;
        let dir = snap_unit3(a.cos() * self.reference + a.sin() * self.axis.cross(self.reference));
        let normal = snap_unit3(self.axis.cross(dir).normalize());
        SymmetryPlane { normal, offset: normal.dot(self.origin) }
    }

    /// The closest position every element in stabilizer leaves in place: the axis when it holds a rotation, else the
    /// one mirror plane, else p itself. Exact for coordinate axes.
    pub fn project_fixed(&self, p: DVec3, stabilizer: &[i32]) -> DVec3 {
        let mut reflection = -1;
        for &g in stabilizer {
            if g > 0 && g < self.count {
                return self.project_to_axis(p);
            }
            if g >= self.count {
                if reflection >= 0 && reflection != g {
                    return self.project_to_axis(p);
                }
                reflection = g;
            }
        }
        if reflection < 0 {
            return p;
        }
        let plane = self.mirror_plane(reflection);
        let k = coordinate_axis(plane.normal.abs());
        if k >= 0 {
            let mut out = p;
            out[k as usize] = plane.offset * plane.normal[k as usize];
            return out;
        }
        plane.project(p)
    }
}

/// The direction perpendicular to axis turned by phase degrees about it from the next coordinate axis (z for y, x for
/// z, y for x).
pub fn radial_reference(axis: DVec3, phase_degrees: f64) -> DVec3 {
    let a = if axis.length() > 1e-12 { axis.normalize() } else { DVec3::Y };
    let k = coordinate_axis(snap_unit3(a).abs());
    let mut base = DVec3::ZERO;
    if k >= 0 {
        base[((k + 1) % 3) as usize] = 1.0;
    } else {
        // The coordinate axis least aligned with a custom axis, made perpendicular to it.
        let m = a.abs();
        base[if m.x <= m.y && m.x <= m.z {
            0
        } else if m.y <= m.z {
            1
        } else {
            2
        }] = 1.0;
        base = (base - a * base.dot(a)).normalize();
    }
    let t = phase_degrees * PI / 180.0;
    snap_unit3(base * snap_unit(t.cos()) + a.cross(base) * snap_unit(t.sin()))
}

/// Radial symmetry about coordinate axis 0-2 through origin, wedge 0 starting phase degrees past the next axis.
pub fn axis_radial(axis: i32, origin: DVec3, count: i32, mirror: bool, phase_degrees: f64) -> RadialSymmetry {
    let mut a = DVec3::ZERO;
    a[axis.clamp(0, 2) as usize] = 1.0;
    RadialSymmetry::new(origin, a, count, mirror, radial_reference(a, phase_degrees))
}

/// Point int layer that keeps the symmetry pairs for an axis on the mesh, so they survive compaction, saving, undo
/// and any amount of position drift. Partners hold +k and -k (k > 0, the sign is the side), on-plane points hold
/// SYMMETRY_ON_PLANE, 0 is unpaired.
pub fn symmetry_layer_name(axis: i32) -> String {
    format!("symMirror{}", (b'X' + axis.clamp(0, 2) as u8) as char)
}

fn signature(r: &RadialSymmetry) -> String {
    // Rounded so a reference re-derived from the same settings names the same layer.
    let q = |v: f64| (v * 1e6).round() as i64;
    let a = r.axis();
    let f = r.reference();
    let h = (q(a.x).wrapping_mul(73856093)
        ^ q(a.y).wrapping_mul(19349663)
        ^ q(a.z).wrapping_mul(83492791)
        ^ q(f.x).wrapping_mul(2654435761)
        ^ q(f.y).wrapping_mul(40503)
        ^ q(f.z).wrapping_mul(1099511628211)) as u64;
    format!("{:08x}", (h ^ (h >> 32)) as u32)
}

/// Point int layer that keeps radial orbits on the mesh: orbit * 256 + slot + 1, SYMMETRY_ON_PLANE on the axis, 0
/// unmatched. Named for the group, its axis and its reference, so changing any of them starts afresh.
pub fn radial_layer_name(radial: &RadialSymmetry) -> String {
    let k = radial.axis_index();
    let mut name = format!(
        "symRadial{}{}{}",
        radial.count(),
        if radial.mirror() { "D" } else { "" },
        if k >= 0 { (b'X' + k as u8) as char } else { 'C' }
    );
    if k < 0 || radial.reference() != radial_reference(radial.axis(), 0.0) {
        name += "_";
        name += &signature(radial);
    }
    name
}

/// True for the name of any symmetry layer, mirror or radial.
pub fn is_symmetry_layer(name: &str) -> bool {
    name.starts_with("symMirror") || name.starts_with("symRadial")
}

/// Drops every stored symmetry layer, mirror and radial.
pub fn remove_symmetry_layers(mesh: &mut Mesh) {
    for name in mesh.point_attrs().names() {
        if is_symmetry_layer(&name) {
            mesh.point_attrs_mut().remove(&name);
        }
    }
}

/// Zeroes every stored symmetry key from first_point on: what an op copied onto the points it created means nothing.
pub fn clear_symmetry_keys(mesh: &mut Mesh, first_point: Index) {
    for name in mesh.point_attrs().names() {
        if !is_symmetry_layer(&name) {
            continue;
        }
        let keys = mesh.point_attrs_mut().get_mut::<i32>(&name);
        for k in keys.iter_mut().skip(first_point as usize) {
            *k = 0;
        }
    }
}

/// A mesh's symmetry setting: a mirror plane across a coordinate axis, or a radial group. Element 0 is the identity.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SymmetrySpec {
    pub axis: i32,
    pub offset: f64,
    pub radial: Option<RadialSymmetry>,
    /// A plane at no axis of the object, as world symmetry gives on a turned object. Pairs go by position.
    pub plane: Option<SymmetryPlane>,
    /// Preserve Seam off: points on the plane may leave it.
    pub free_seam: bool,
}

impl SymmetrySpec {
    pub fn is_radial(&self) -> bool {
        self.radial.is_some()
    }
    /// Number of elements: 2 for a mirror (element 1 reflects).
    pub fn order(&self) -> i32 {
        self.radial.as_ref().map_or(2, |r| r.order())
    }
    pub fn reflects(&self, g: i32) -> bool {
        self.radial.as_ref().map_or(g == 1, |r| r.reflects(g))
    }
    pub fn plane(&self) -> SymmetryPlane {
        self.plane.clone().unwrap_or_else(|| axis_plane(self.axis, self.offset))
    }
    pub fn apply(&self, g: i32, p: DVec3) -> DVec3 {
        if let Some(r) = &self.radial {
            return r.apply(g, p);
        }
        if let Some(pl) = &self.plane {
            return if g == 1 { pl.reflect(p) } else { p };
        }
        let mut r = p;
        if g == 1 {
            r[self.axis as usize] = 2.0 * self.offset - r[self.axis as usize];
        }
        r
    }
    pub fn apply_vector(&self, g: i32, v: DVec3) -> DVec3 {
        if let Some(r) = &self.radial {
            return r.apply_vector(g, v);
        }
        if let Some(pl) = &self.plane {
            return if g == 1 { v - pl.normal * (2.0 * v.dot(pl.normal)) } else { v };
        }
        let mut r = v;
        if g == 1 {
            r[self.axis as usize] = -r[self.axis as usize];
        }
        r
    }
    /// p moved onto the plane, or the axis or a mirror plane, when it lies within tolerance of it; else p.
    pub fn snap(&self, p: DVec3, tolerance: f64) -> DVec3 {
        if let Some(radial) = &self.radial {
            return snap_radial(radial, p, tolerance);
        }
        if let Some(pl) = &self.plane {
            let d = pl.normal.dot(p) - pl.offset;
            return if d.abs() <= tolerance { p - pl.normal * d } else { p };
        }
        let mut r = p;
        if (r[self.axis as usize] - self.offset).abs() <= tolerance {
            r[self.axis as usize] = self.offset;
        }
        r
    }
}

/// How SymmetryMap::persistent resolved the points.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SymmetrySync {
    /// Paired or on-plane from the stored layer.
    pub stored: Index,
    /// Had no stored partner, matched through their paired neighbours.
    pub topological: Index,
    /// Had no stored partner, matched by position within tolerance.
    pub by_position: Index,
    pub unmatched: Index,
}

#[derive(Clone, Copy, Debug)]
pub struct SymmetryCheck {
    /// Largest distance between a point and the reflection of its partner, of an on-plane point from the plane, or of
    /// an unmatched point's reflection from the surface.
    pub max_asymmetry: f64,
    pub worst_point: Index,
    /// Radial: orbits that are not fixed.
    pub pairs: Index,
    /// Radial: points on the axis or a mirror plane.
    pub on_plane: Index,
    pub unmatched: Index,
}

impl Default for SymmetryCheck {
    fn default() -> Self {
        Self { max_asymmetry: 0.0, worst_point: INVALID, pairs: 0, on_plane: 0, unmatched: 0 }
    }
}

type Cell = [i64; 3];

/// Points bucketed by cell, for nearest lookups within tolerance.
struct PointGrid {
    cell: f64,
    cells: HashMap<Cell, Vec<Index>>,
}

impl PointGrid {
    fn key(&self, p: DVec3) -> Cell {
        let q = (p / self.cell).floor();
        [q.x as i64, q.y as i64, q.z as i64]
    }

    fn new(mesh: &Mesh, tolerance: f64, points: impl IntoIterator<Item = Index>) -> Self {
        let mut grid = Self { cell: (tolerance * 2.0).max(1e-9), cells: HashMap::new() };
        for p in points {
            let k = grid.key(mesh.position(p));
            grid.cells.entry(k).or_default().push(p);
        }
        grid
    }

    /// The point nearest target within tolerance that accept takes; the lowest id among equals.
    fn nearest(&self, mesh: &Mesh, target: DVec3, tolerance: f64, accept: impl Fn(Index) -> bool) -> Index {
        let k = self.key(target);
        let mut best = tolerance * tolerance;
        let mut found = INVALID;
        for dz in -1..=1 {
            for dy in -1..=1 {
                for dx in -1..=1 {
                    let Some(list) = self.cells.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else { continue };
                    for &q in list {
                        if !accept(q) {
                            continue;
                        }
                        let d2 = (mesh.position(q) - target).length_squared();
                        if d2 <= best && (d2 < best || q < found) {
                            best = d2;
                            found = q;
                        }
                    }
                }
            }
        }
        found
    }
}

fn plane_axis(plane: &SymmetryPlane) -> i32 {
    coordinate_axis(plane.normal)
}

/// Sides for a map whose pairs are known: the partner further along the normal is the positive one.
fn sides_from_positions(mesh: &Mesh, plane: &SymmetryPlane, mirror: &[Index]) -> Vec<i8> {
    let mut side = vec![0i8; mirror.len()];
    for p in 0..mirror.len() as Index {
        let q = mirror[p as usize];
        if q == INVALID || q == p || !mesh.point_valid(p) || !mesh.point_valid(q) {
            continue;
        }
        let dp = plane.distance(mesh.position(p));
        let dq = plane.distance(mesh.position(q));
        side[p as usize] = if dp > dq || (dp == dq && p < q) { 1 } else { -1 };
    }
    side
}

/// p onto the radial axis or the mirror plane it lies within tolerance of; p itself when it lies near neither.
fn snap_radial(radial: &RadialSymmetry, p: DVec3, tolerance: f64) -> DVec3 {
    if radial.axis_distance(p) <= tolerance {
        return radial.project_to_axis(p);
    }
    if radial.mirror() {
        for k in 0..radial.count() {
            let g = radial.count() + k;
            if radial.mirror_plane(g).distance(p).abs() <= tolerance {
                return radial.project_fixed(p, &[g]);
            }
        }
    }
    p
}

/// The element whose image of the orbit lies at the smallest angle: the orbit's member in the fundamental wedge.
fn canonical_element(mesh: &Mesh, radial: &RadialSymmetry, images: &[Index]) -> i32 {
    let mut best = 0;
    let mut best_angle = f64::MAX;
    for (g, &q) in images.iter().enumerate() {
        let mut a = radial.angle(mesh.position(q));
        if a > 2.0 * PI - 1e-9 {
            a = 0.0;
        }
        if a < best_angle - 1e-12 {
            best_angle = a;
            best = g as i32;
        }
    }
    best
}

fn face_centre(mesh: &Mesh, f: Index) -> DVec3 {
    let mut sum = DVec3::ZERO;
    let mut n = 0;
    mesh.for_each_face_corner(f, |c| {
        sum += mesh.position(mesh.corner_point(c));
        n += 1;
    });
    if n > 0 {
        sum / n as f64
    } else {
        sum
    }
}

/// A face isomorphism grown outward from corner c0 landing on corner d0: walking forward round both faces, or with
/// reversing, walking the image backward (an orientation-reversing map such as a reflection).
struct Isomorphism {
    point: Vec<Index>,
    point_inv: Vec<Index>,
    face: Vec<Index>,
    face_inv: Vec<Index>,
}

impl Isomorphism {
    fn new(mesh: &Mesh) -> Self {
        let pc = mesh.point_capacity() as usize;
        let fc = mesh.face_capacity() as usize;
        Self { point: vec![INVALID; pc], point_inv: vec![INVALID; pc], face: vec![INVALID; fc], face_inv: vec![INVALID; fc] }
    }

    fn grow(&mut self, mesh: &Mesh, c0: Index, d0: Index, reversing: bool) -> bool {
        let mut queue = VecDeque::from([(c0, d0)]);
        while let Some((c0f, d0f)) = queue.pop_front() {
            let f = mesh.corner_face(c0f);
            let g = mesh.corner_face(d0f);
            if self.face[f as usize] != INVALID {
                if self.face[f as usize] != g {
                    return false;
                }
                continue;
            }
            if mesh.face_size(f) != mesh.face_size(g) || self.face_inv[g as usize] != INVALID {
                return false;
            }
            self.face[f as usize] = g;
            self.face_inv[g as usize] = f;
            let (mut c, mut d) = (c0f, d0f);
            for _ in 0..mesh.face_size(f) {
                let (a, b) = (mesh.corner_point(c), mesh.corner_point(d));
                if self.point[a as usize] == INVALID && self.point_inv[b as usize] == INVALID {
                    self.point[a as usize] = b;
                    self.point_inv[b as usize] = a;
                } else if !(self.point[a as usize] == b && self.point_inv[b as usize] == a) {
                    return false;
                }
                let dn = if reversing { mesh.corner_prev(d) } else { mesh.corner_next(d) };
                let e = mesh.corner_edge(c);
                // The image of the edge from c to its next corner.
                let image = if reversing { dn } else { d };
                if mesh.is_manifold_edge(e) && mesh.is_manifold_edge(mesh.corner_edge(image)) {
                    let r = mesh.corner_radial(c);
                    let rm = mesh.corner_radial(image);
                    if self.face[mesh.corner_face(r) as usize] == INVALID {
                        queue.push_back((r, if reversing { mesh.corner_next(rm) } else { rm }));
                    }
                }
                c = mesh.corner_next(c);
                d = dn;
            }
        }
        true
    }
}

/// The corner of the face nearest g's image of c0's face that best lines up with g's image of the walk from c0.
fn image_corner(mesh: &Mesh, radial: &RadialSymmetry, g: i32, c0: Index, taken: &[Index]) -> Index {
    let f = mesh.corner_face(c0);
    let target = radial.apply(g, face_centre(mesh, f));
    let mut face = INVALID;
    let mut best = f64::MAX;
    for h in 0..mesh.face_capacity() {
        if !mesh.face_valid(h) || mesh.face_size(h) != mesh.face_size(f) || taken[h as usize] != INVALID {
            continue;
        }
        let d2 = (face_centre(mesh, h) - target).length_squared();
        if d2 < best {
            best = d2;
            face = h;
        }
    }
    if face == INVALID {
        return INVALID;
    }
    let reversing = radial.reflects(g);
    let mut corner = INVALID;
    best = f64::MAX;
    mesh.for_each_face_corner(face, |d0| {
        let mut err = 0.0;
        let (mut c, mut d) = (c0, d0);
        for _ in 0..mesh.face_size(f) {
            let diff = mesh.position(mesh.corner_point(d)) - radial.apply(g, mesh.position(mesh.corner_point(c)));
            err += diff.length_squared();
            c = mesh.corner_next(c);
            d = if reversing { mesh.corner_prev(d) } else { mesh.corner_next(d) };
        }
        if err < best {
            best = err;
            corner = d0;
        }
    });
    corner
}

/// The point map of element g grown over every face, seeded at c0 and then at each shell's face it has not reached.
fn topological_image(mesh: &Mesh, radial: &RadialSymmetry, g: i32, c0: Index) -> Vec<Index> {
    let mut iso = Isomorphism::new(mesh);
    let seed = |iso: &mut Isomorphism, c: Index| {
        let d = image_corner(mesh, radial, g, c, &iso.face_inv);
        d != INVALID && iso.grow(mesh, c, d, radial.reflects(g))
    };
    if !seed(&mut iso, c0) {
        return Vec::new();
    }
    for f in 0..mesh.face_capacity() {
        if mesh.face_valid(f) && iso.face[f as usize] == INVALID && !seed(&mut iso, mesh.face_corner(f)) {
            return Vec::new();
        }
    }
    iso.point
}

fn neighbours(mesh: &Mesh, p: Index) -> Vec<Index> {
    let mut out = Vec::new();
    mesh.for_each_point_edge(p, |e| out.push(mesh.edge_other_point(e, p)));
    out
}

fn adjacent(mesh: &Mesh, a: Index, b: Index) -> bool {
    a == b || mesh.find_edge(a, b) != INVALID
}

fn merged(a: &[Index], b: &[Index]) -> Vec<Index> {
    let mut out = a.to_vec();
    out.extend(b.iter().copied().filter(|&i| i != INVALID));
    unique_sorted(&out)
}

static NO_RADIAL: LazyLock<RadialSymmetry> = LazyLock::new(RadialSymmetry::default);

/// Symmetry of a mesh's points: the mirror partner of every point across a plane, or for radial symmetry each
/// point's orbit, its image under every element of the group. A mirror is the order 2 case (element 1 reflects), so
/// the generic calls (order, image, wedge, enforce_wedge...) serve both. Points on the plane (or axis) map to
/// themselves. Ids refer to the mesh the map was built from; rebuild after topology changes.
#[derive(Clone, Debug)]
pub struct SymmetryMap {
    /// Preserve Seam off: points on the plane may leave it.
    pub free_seam: bool,
    plane: SymmetryPlane,
    axis: i32,
    tolerance: f64,
    mirror: Vec<Index>,
    on_plane: Vec<u8>,
    side: Vec<i8>,
    unmatched: Index,

    // Radial: per point its orbit and slot (INVALID and -1 when unmatched), per orbit its point in every slot.
    radial: Option<RadialSymmetry>,
    orbit: Vec<Index>,
    slot: Vec<i16>,
    orbit_points: Vec<Index>,
    fixed: Vec<u8>,
}

impl Default for SymmetryMap {
    fn default() -> Self {
        Self {
            free_seam: false,
            plane: SymmetryPlane::default(),
            axis: -1,
            tolerance: 1e-3,
            mirror: Vec::new(),
            on_plane: Vec::new(),
            side: Vec::new(),
            unmatched: 0,
            radial: None,
            orbit: Vec::new(),
            slot: Vec::new(),
            orbit_points: Vec::new(),
            fixed: Vec::new(),
        }
    }
}

impl SymmetryMap {
    /// Matches each point to the nearest point at its reflection, within tolerance.
    pub fn build(mesh: &Mesh, plane: &SymmetryPlane, tolerance: f64) -> Self {
        let mut map = Self { plane: *plane, tolerance, ..Default::default() };
        map.plane.normal = plane.normal.normalize();
        let pc = mesh.point_capacity();
        map.on_plane = vec![0; pc as usize];
        let grid = PointGrid::new(mesh, tolerance, (0..pc).filter(|&p| mesh.point_valid(p)));
        let mp = map.plane;
        let results: Vec<(Index, bool)> = (0..pc)
            .into_par_iter()
            .with_min_len(4096)
            .map(|p| {
                if !mesh.point_valid(p) {
                    return (INVALID, false);
                }
                let pos = mesh.position(p);
                if mp.distance(pos).abs() <= tolerance {
                    return (p, true);
                }
                (grid.nearest(mesh, mp.reflect(pos), tolerance, |_| true), false)
            })
            .collect();
        map.mirror = results.iter().map(|r| r.0).collect();
        for (p, r) in results.iter().enumerate() {
            map.on_plane[p] = r.1 as u8;
        }
        map.unmatched = (0..pc).filter(|&p| mesh.point_valid(p) && map.mirror[p as usize] == INVALID).count() as Index;
        map.axis = plane_axis(&map.plane);
        map.side = sides_from_positions(mesh, &map.plane, &map.mirror);
        map
    }

    /// Matches by topology, walking outward from seed_edge, which must lie on the plane with two faces. Works on
    /// meshes whose positions are no longer symmetric. Empty map if the topology is not symmetric.
    pub fn build_topological(mesh: &Mesh, seed_edge: Index, plane: &SymmetryPlane) -> Self {
        let mut map = Self { plane: *plane, ..Default::default() };
        map.plane.normal = plane.normal.normalize();
        if !mesh.edge_valid(seed_edge) || !mesh.is_manifold_edge(seed_edge) {
            return map;
        }
        let pc = mesh.point_capacity();
        let mut mirror = vec![INVALID; pc as usize];
        let mut face_mirror = vec![INVALID; mesh.face_capacity() as usize];
        let pair = |mirror: &mut Vec<Index>, a: Index, b: Index| {
            if mirror[a as usize] == INVALID && mirror[b as usize] == INVALID {
                mirror[a as usize] = b;
                mirror[b as usize] = a;
                return true;
            }
            mirror[a as usize] == b && mirror[b as usize] == a
        };

        // Each entry pairs a corner walked forward with a corner of the mirror face walked backward.
        let r1 = mesh.edge_corner(seed_edge);
        let r2 = mesh.corner_radial(r1);
        if mesh.corner_point(r1) == mesh.corner_point(r2) {
            return map;
        }
        let mut queue = VecDeque::from([(r1, mesh.corner_next(r2))]);
        let mut ok = true;
        while ok {
            let Some((c0, d0)) = queue.pop_front() else { break };
            let f = mesh.corner_face(c0);
            let g = mesh.corner_face(d0);
            if face_mirror[f as usize] != INVALID {
                ok = face_mirror[f as usize] == g;
                continue;
            }
            if mesh.face_size(f) != mesh.face_size(g) || face_mirror[g as usize] != INVALID {
                ok = false;
                break;
            }
            face_mirror[f as usize] = g;
            face_mirror[g as usize] = f;
            let (mut c, mut d) = (c0, d0);
            for _ in 0..mesh.face_size(f) {
                if !pair(&mut mirror, mesh.corner_point(c), mesh.corner_point(d)) {
                    ok = false;
                    break;
                }
                let dn = mesh.corner_prev(d);
                let e = mesh.corner_edge(c);
                let em = mesh.corner_edge(dn);
                if mesh.is_manifold_edge(e) && mesh.is_manifold_edge(em) {
                    let r = mesh.corner_radial(c);
                    let rm = mesh.corner_radial(dn);
                    if face_mirror[mesh.corner_face(r) as usize] == INVALID {
                        queue.push_back((r, mesh.corner_next(rm)));
                    }
                }
                c = mesh.corner_next(c);
                d = dn;
            }
        }
        if !ok {
            return map;
        }

        map.mirror = mirror;
        map.on_plane = vec![0; pc as usize];
        for p in 0..pc {
            if !mesh.point_valid(p) {
                continue;
            }
            if map.mirror[p as usize] == p {
                map.on_plane[p as usize] = 1;
            } else if map.mirror[p as usize] == INVALID {
                map.unmatched += 1;
            }
        }
        map.axis = plane_axis(&map.plane);
        map.side = sides_from_positions(mesh, &map.plane, &map.mirror);
        map
    }

    /// Matches each point's image under every element to the nearest point there, within tolerance.
    pub fn build_radial(mesh: &Mesh, radial: &RadialSymmetry, tolerance: f64) -> Self {
        let pc = mesh.point_capacity();
        let n = radial.order() as usize;
        let mut map = Self::radial_empty(radial, pc);
        map.tolerance = tolerance;
        let valid: Vec<Index> = (0..pc).filter(|&p| mesh.point_valid(p)).collect();
        let grid = PointGrid::new(mesh, tolerance, valid.iter().copied());
        let found: Vec<Vec<Index>> = valid
            .par_iter()
            .with_min_len(2048)
            .map(|&p| {
                let pos = mesh.position(p);
                let on_axis = radial.axis_distance(pos) <= tolerance;
                (0..n as i32)
                    .map(|g| {
                        if g == 0 || (on_axis && !radial.reflects(g)) {
                            p
                        } else {
                            grid.nearest(mesh, radial.apply(g, pos), tolerance, |_| true)
                        }
                    })
                    .collect()
            })
            .collect();
        for (i, &p) in valid.iter().enumerate() {
            if map.orbit[p as usize] != INVALID {
                continue;
            }
            let images = &found[i];
            if images.contains(&INVALID) {
                continue;
            }
            let rep = canonical_element(mesh, radial, images);
            map.add_orbit(mesh, images, rep);
        }
        map.count_unmatched(mesh);
        map
    }

    fn radial_empty(radial: &RadialSymmetry, pc: Index) -> Self {
        Self {
            radial: Some(radial.clone()),
            orbit: vec![INVALID; pc as usize],
            slot: vec![-1; pc as usize],
            fixed: vec![0; pc as usize],
            ..Default::default()
        }
    }

    /// Matches by topology, walking outward from seed_edge (anywhere with a face) and the faces its rotated (and
    /// reflected) copies land nearest to. Works while positions have drifted by less than a wedge. Empty map if the
    /// topology is not symmetric.
    pub fn build_radial_topological(mesh: &Mesh, seed_edge: Index, radial: &RadialSymmetry) -> Self {
        let mut map = Self { radial: Some(radial.clone()), ..Default::default() };
        if !mesh.edge_valid(seed_edge) || mesh.edge_corner(seed_edge) == INVALID {
            return map;
        }
        let pc = mesh.point_capacity();
        let count = radial.count();
        let n = radial.order() as usize;
        let seed = mesh.edge_corner(seed_edge);
        let mut turn = Vec::new();
        let mut flip = Vec::new();
        if count > 1 {
            turn = topological_image(mesh, radial, 1, seed);
            if turn.is_empty() {
                return map;
            }
        }
        if radial.mirror() {
            flip = topological_image(mesh, radial, count, seed);
            if flip.is_empty() {
                return map;
            }
        }
        map = Self::radial_empty(radial, pc);
        map.tolerance = 1e-4f64.max(mesh.bounds().size().length() * 1e-5);
        let mut images = vec![INVALID; n];
        for p in 0..pc {
            if !mesh.point_valid(p) || map.orbit[p as usize] != INVALID {
                continue;
            }
            let mut ok = true;
            let mut q = p;
            for k in 0..count as usize {
                if !ok {
                    break;
                }
                images[k] = q;
                q = if count > 1 { turn[q as usize] } else { q };
                ok = q != INVALID;
            }
            // The turns must close up after count steps.
            ok = ok && q == p;
            if ok && radial.mirror() {
                let mut r = flip[p as usize];
                ok = r != INVALID && flip[r as usize] == p;
                for k in 0..count as usize {
                    if !ok {
                        break;
                    }
                    images[count as usize + k] = r;
                    r = if count > 1 { turn[r as usize] } else { r };
                    ok = r != INVALID;
                }
            }
            if ok {
                let rep = canonical_element(mesh, radial, &images);
                map.add_orbit(mesh, &images, rep);
            }
        }
        map.count_unmatched(mesh);
        map
    }

    /// Map from the pairs stored on the mesh for an axis plane. Points without a valid stored partner are matched
    /// among themselves, first through their paired neighbours, then by position within tolerance, and the new pairs
    /// are stored back. A mesh without the layer gets it built here.
    pub fn persistent_axis(mesh: &mut Mesh, axis: i32, tolerance: f64, sync: Option<&mut SymmetrySync>, offset: f64) -> Self {
        let mut map = Self::default();
        map.axis = axis.clamp(0, 2);
        map.plane = axis_plane(map.axis, offset);
        map.tolerance = tolerance;
        let pc = mesh.point_capacity();
        map.mirror = vec![INVALID; pc as usize];
        map.on_plane = vec![0; pc as usize];
        map.side = vec![0; pc as usize];
        let mut info = SymmetrySync::default();

        let layer = symmetry_layer_name(map.axis);
        let mut keys = mesh.point_attrs_mut().get_or_add::<i32>(&layer).to_vec();
        let m: &Mesh = mesh;
        let pos = |p: Index| m.position(p);

        // A key held by several points on a side (an op copied it onto new points) pairs them by nearest reflection.
        #[derive(Default)]
        struct Group {
            pos: Vec<Index>,
            neg: Vec<Index>,
        }
        let mut groups: BTreeMap<i32, Group> = BTreeMap::new();
        for p in 0..pc {
            let k = keys[p as usize];
            if !m.point_valid(p) || k == 0 {
                continue;
            }
            if k == SYMMETRY_ON_PLANE {
                map.mirror[p as usize] = p;
                map.on_plane[p as usize] = 1;
                info.stored += 1;
            } else if k > 0 {
                groups.entry(k).or_default().pos.push(p);
            } else {
                groups.entry(-k).or_default().neg.push(p);
            }
        }
        for g in groups.values() {
            let mut used = vec![false; g.neg.len()];
            for &a in &g.pos {
                let mut best = f64::MAX;
                let mut found = g.neg.len();
                for (i, &b) in g.neg.iter().enumerate() {
                    let d2 = (map.reflected(pos(a)) - pos(b)).length_squared();
                    if !used[i] && (found == g.neg.len() || d2 < best) {
                        best = d2;
                        found = i;
                    }
                }
                if found == g.neg.len() {
                    continue;
                }
                used[found] = true;
                let b = g.neg[found];
                map.mirror[a as usize] = b;
                map.mirror[b as usize] = a;
                map.side[a as usize] = 1;
                map.side[b as usize] = -1;
                info.stored += 2;
            }
        }

        // The rest are new or orphaned, so whatever key they hold means nothing any more.
        let mut fresh = Vec::new();
        for p in 0..pc {
            if m.point_valid(p) && map.mirror[p as usize] == INVALID {
                keys[p as usize] = 0;
                fresh.push(p);
            }
        }

        // A point's partner neighbours the partners of its paired neighbours. Ops build both halves together, so
        // this finds new points' partners however far the old ones have drifted.
        let match_topologically = |map: &mut Self, info: &mut SymmetrySync| {
            let mut changed = true;
            while changed {
                changed = false;
                for &a in &fresh {
                    if map.mirror[a as usize] != INVALID {
                        continue;
                    }
                    let anchors: Vec<Index> = neighbours(m, a)
                        .into_iter()
                        .filter(|&n| map.mirror[n as usize] != INVALID)
                        .map(|n| map.mirror[n as usize])
                        .collect();
                    if anchors.is_empty() {
                        continue;
                    }
                    let mut candidates = neighbours(m, anchors[0]);
                    candidates.push(a);
                    let target = map.reflected(pos(a));
                    let mut best = f64::MAX;
                    let mut found = INVALID;
                    for &b in &candidates {
                        if map.mirror[b as usize] != INVALID {
                            continue;
                        }
                        let mut ok = true;
                        if b == a {
                            ok = map.plane.distance(pos(a)).abs() <= map.tolerance;
                        }
                        for &mm in &anchors {
                            ok = ok && (b == a || mm != b) && adjacent(m, b, mm);
                        }
                        let d2 = (pos(b) - target).length_squared();
                        if ok && d2 < best {
                            best = d2;
                            found = b;
                        }
                    }
                    if found == INVALID {
                        continue;
                    }
                    if found == a {
                        map.mirror[a as usize] = a;
                        map.on_plane[a as usize] = 1;
                        info.topological += 1;
                    } else {
                        map.mirror[a as usize] = found;
                        map.mirror[found as usize] = a;
                        info.topological += 2;
                    }
                    changed = true;
                }
            }
        };

        let match_by_position = |map: &mut Self, info: &mut SymmetrySync| {
            let tol = map.tolerance.max(1e-12);
            let mut grid = PointGrid { cell: tol * 2.0, cells: HashMap::new() };
            for &p in &fresh {
                if map.mirror[p as usize] == INVALID {
                    let k = grid.key(pos(p));
                    grid.cells.entry(k).or_default().push(p);
                }
            }
            for &a in &fresh {
                if map.mirror[a as usize] != INVALID {
                    continue;
                }
                if map.plane.distance(pos(a)).abs() <= tol {
                    map.mirror[a as usize] = a;
                    map.on_plane[a as usize] = 1;
                    info.by_position += 1;
                    continue;
                }
                let target = map.reflected(pos(a));
                let k = grid.key(target);
                let mut best = tol * tol;
                let mut found = INVALID;
                for dz in -1..=1 {
                    for dy in -1..=1 {
                        for dx in -1..=1 {
                            let Some(list) = grid.cells.get(&[k[0] + dx, k[1] + dy, k[2] + dz]) else { continue };
                            for &b in list {
                                if b == a || map.mirror[b as usize] != INVALID {
                                    continue;
                                }
                                let d2 = (pos(b) - target).length_squared();
                                if d2 <= best {
                                    best = d2;
                                    found = b;
                                }
                            }
                        }
                    }
                }
                if found != INVALID {
                    map.mirror[a as usize] = found;
                    map.mirror[found as usize] = a;
                    info.by_position += 2;
                }
            }
        };

        let mut renumber = false;
        if !fresh.is_empty() {
            match_topologically(&mut map, &mut info);
            match_by_position(&mut map, &mut info);
            match_topologically(&mut map, &mut info);

            for &a in &fresh {
                let b = map.mirror[a as usize];
                if b == INVALID || b == a || map.side[a as usize] != 0 {
                    continue;
                }
                let da = map.plane.distance(pos(a));
                let db = map.plane.distance(pos(b));
                let plus = if da > db || (da == db && a < b) { a } else { b };
                map.side[plus as usize] = 1;
                map.side[if plus == a { b } else { a } as usize] = -1;
            }

            let mut next: i64 = 1;
            for p in 0..pc {
                let k = keys[p as usize];
                if m.point_valid(p) && k != 0 && k != SYMMETRY_ON_PLANE {
                    next = next.max(k.unsigned_abs() as i64 + 1);
                }
            }
            if next + fresh.len() as i64 >= i32::MAX as i64 {
                // Out of keys: renumber densely.
                renumber = true;
            } else {
                for &a in &fresh {
                    let b = map.mirror[a as usize];
                    if b == a {
                        keys[a as usize] = SYMMETRY_ON_PLANE;
                    } else if b != INVALID && map.side[a as usize] > 0 {
                        keys[a as usize] = next as i32;
                        keys[b as usize] = -next as i32;
                        next += 1;
                    }
                }
            }
        }
        mesh.point_attrs_mut().get_mut::<i32>(&layer).copy_from_slice(&keys);
        if renumber {
            map.store_axis(mesh, map.axis);
        }
        map.unmatched = (0..pc).filter(|&p| mesh.point_valid(p) && map.mirror[p as usize] == INVALID).count() as Index;
        info.unmatched = map.unmatched;
        if let Some(s) = sync {
            *s = info;
        }
        map
    }

    /// The same for radial symmetry, from the orbits stored in its layer.
    pub fn persistent_radial(mesh: &mut Mesh, radial: &RadialSymmetry, tolerance: f64, sync: Option<&mut SymmetrySync>) -> Self {
        let pc = mesh.point_capacity();
        let count = radial.count();
        let n = radial.order() as usize;
        let mut map = Self::radial_empty(radial, pc);
        map.tolerance = tolerance;
        let mut info = SymmetrySync::default();

        let layer = radial_layer_name(radial);
        let mut keys = mesh.point_attrs_mut().get_or_add::<i32>(&layer).to_vec();
        let m: &Mesh = mesh;
        let pos = |p: Index| m.position(p);

        // Ordered by stored orbit id, so the same layer always resolves the same way.
        let mut groups: BTreeMap<i64, Vec<(i32, Index)>> = BTreeMap::new();
        let mut axis_points = Vec::new();
        let mut next_id: i64 = 0;
        for p in 0..pc {
            let key = keys[p as usize];
            if !m.point_valid(p) || key == 0 {
                continue;
            }
            if key == SYMMETRY_ON_PLANE {
                axis_points.push(p);
                continue;
            }
            let k = key as i64 - 1;
            if k < 0 || k % SLOT_BITS >= n as i64 {
                continue;
            }
            groups.entry(k / SLOT_BITS).or_default().push(((k % SLOT_BITS) as i32, p));
            next_id = next_id.max(k / SLOT_BITS + 1);
        }
        let mut images = vec![INVALID; n];
        for &p in &axis_points {
            images.fill(p);
            if map.add_orbit(m, &images, 0) {
                info.stored += 1;
            }
        }
        for members in groups.values() {
            images.fill(INVALID);
            for &(slot, p) in members {
                if images[slot as usize] == INVALID {
                    images[slot as usize] = p;
                }
            }
            // On a mirror plane: the orbit's slot 0 point stored the reflection that fixes it, the rest their turns.
            if radial.mirror() && images[0] == INVALID {
                let mut plane = -1;
                let mut filled = 0;
                for k in 0..count {
                    if images[(count + k) as usize] != INVALID {
                        plane = k;
                        filled += 1;
                    }
                }
                if filled == 1 {
                    images[0] = images[(count + plane) as usize];
                    for j in 0..count {
                        images[(count + j) as usize] = images[(((j - plane) % count + count) % count) as usize];
                    }
                }
            }
            if images.contains(&INVALID) {
                continue;
            }
            if map.add_orbit(m, &images, 0) {
                info.stored += members.len() as Index;
            }
        }

        let mut fresh = Vec::new();
        for p in 0..pc {
            if m.point_valid(p) && map.orbit[p as usize] == INVALID {
                keys[p as usize] = 0;
                fresh.push(p);
            }
        }
        let stored_orbits = (map.orbit_points.len() / n) as Index;

        let distinct = |imgs: &[Index]| -> Index { (0..n).filter(|&g| !imgs[..g].contains(&imgs[g])).count() as Index };

        // A new point's image under g neighbours the images of its matched neighbours. Ops build every wedge
        // together, so this finds new points' images however far the old ones have drifted.
        let match_topologically = |map: &mut Self, info: &mut SymmetrySync, images: &mut Vec<Index>| {
            let mut changed = true;
            while changed {
                changed = false;
                for &a in &fresh {
                    if map.orbit[a as usize] != INVALID {
                        continue;
                    }
                    let anchors: Vec<Index> =
                        neighbours(m, a).into_iter().filter(|&nb| map.orbit[nb as usize] != INVALID).collect();
                    if anchors.is_empty() {
                        continue;
                    }
                    let mut ok = true;
                    images[0] = a;
                    for g in 1..n as i32 {
                        if !ok {
                            break;
                        }
                        let target = radial.apply(g, pos(a));
                        let anchor_image = map.image(g, anchors[0]);
                        let mut candidates = vec![a];
                        candidates.extend(neighbours(m, anchor_image));
                        let mut best = f64::MAX;
                        let mut found = INVALID;
                        for &b in &candidates {
                            if map.orbit[b as usize] != INVALID {
                                continue;
                            }
                            let mut fits = b != a || (target - pos(a)).length() <= map.tolerance;
                            for &mm in &anchors {
                                let mi = map.image(g, mm);
                                fits = fits && mi != b && m.find_edge(b, mi) != INVALID;
                            }
                            let d2 = (pos(b) - target).length_squared();
                            if fits && d2 < best {
                                best = d2;
                                found = b;
                            }
                        }
                        images[g as usize] = found;
                        ok = found != INVALID;
                        // The same point under two elements only where both put a in the same place.
                        for h in 0..g {
                            if !ok {
                                break;
                            }
                            if images[h as usize] == found {
                                ok = (radial.apply(h, pos(a)) - target).length() <= map.tolerance;
                            }
                        }
                    }
                    if !ok {
                        continue;
                    }
                    // a takes the wedge of a neighbour, so a new orbit's slots line up with the old ones around it.
                    let mut rep = -1;
                    for &mm in &anchors {
                        if map.fixed[mm as usize] == 0 {
                            rep = radial.inverse(map.slot[mm as usize] as i32);
                            break;
                        }
                    }
                    if rep < 0 {
                        rep = canonical_element(m, radial, images);
                    }
                    if map.add_orbit(m, images, rep) {
                        info.topological += distinct(images);
                        changed = true;
                    }
                }
            }
        };

        let match_by_position = |map: &mut Self, info: &mut SymmetrySync, images: &mut Vec<Index>| {
            let tol = map.tolerance.max(1e-12);
            let open: Vec<Index> = fresh.iter().copied().filter(|&p| map.orbit[p as usize] == INVALID).collect();
            let grid = PointGrid::new(m, tol, open.iter().copied());
            for &a in &open {
                if map.orbit[a as usize] != INVALID {
                    continue;
                }
                let on_axis = radial.axis_distance(pos(a)) <= tol;
                let mut ok = true;
                for g in 0..n as i32 {
                    if !ok {
                        break;
                    }
                    images[g as usize] = if g == 0 || (on_axis && !radial.reflects(g)) {
                        a
                    } else {
                        grid.nearest(m, radial.apply(g, pos(a)), tol, |q| map.orbit[q as usize] == INVALID)
                    };
                    ok = images[g as usize] != INVALID;
                }
                if ok {
                    let rep = canonical_element(m, radial, images);
                    if map.add_orbit(m, images, rep) {
                        info.by_position += distinct(images);
                    }
                }
            }
        };

        let mut renumber = false;
        if !fresh.is_empty() {
            match_topologically(&mut map, &mut info, &mut images);
            match_by_position(&mut map, &mut info, &mut images);
            match_topologically(&mut map, &mut info, &mut images);

            let orbits = (map.orbit_points.len() / n) as Index;
            if next_id + (orbits - stored_orbits) as i64 >= (i32::MAX as i64 - SLOT_BITS) / SLOT_BITS {
                // Out of ids: renumber densely.
                renumber = true;
            } else {
                for o in stored_orbits..orbits {
                    let base = o as usize * n;
                    let first = map.orbit_points[base];
                    if map.orbit_points[base..base + n].iter().all(|&q| q == first) {
                        keys[first as usize] = SYMMETRY_ON_PLANE;
                    } else {
                        for g in 0..n {
                            let q = map.orbit_points[base + g];
                            if map.slot[q as usize] as usize == g {
                                keys[q as usize] = (next_id * SLOT_BITS + map.stored_slot(q) as i64 + 1) as i32;
                            }
                        }
                    }
                    next_id += 1;
                }
            }
        }
        mesh.point_attrs_mut().get_mut::<i32>(&layer).copy_from_slice(&keys);
        if renumber {
            map.store(mesh);
        }
        map.count_unmatched(mesh);
        info.unmatched = map.unmatched;
        if let Some(s) = sync {
            *s = info;
        }
        map
    }

    /// The mirror or radial persistent map the spec describes.
    pub fn persistent(mesh: &mut Mesh, spec: &SymmetrySpec, tolerance: f64, sync: Option<&mut SymmetrySync>) -> Self {
        if let Some(pl) = &spec.plane {
            let mut map = Self::build(mesh, pl, tolerance);
            map.free_seam = spec.free_seam;
            return map;
        }
        let mut map = Self::persistent_of(mesh, spec, tolerance, sync);
        map.free_seam = spec.free_seam;
        map
    }

    fn persistent_of(mesh: &mut Mesh, spec: &SymmetrySpec, tolerance: f64, sync: Option<&mut SymmetrySync>) -> Self {
        match &spec.radial {
            Some(r) => Self::persistent_radial(mesh, r, tolerance, sync),
            None => Self::persistent_axis(mesh, spec.axis, tolerance, sync, spec.offset),
        }
    }

    /// build() or build_radial() as the spec describes, by position.
    pub fn build_for(mesh: &Mesh, spec: &SymmetrySpec, tolerance: f64) -> Self {
        match &spec.radial {
            Some(r) => Self::build_radial(mesh, r, tolerance),
            None => Self::build(mesh, &spec.plane(), tolerance),
        }
    }

    /// Writes this map into the mesh's layer for axis, replacing what was there.
    pub fn store_axis(&self, mesh: &mut Mesh, axis: i32) {
        if self.radial.is_some() {
            self.store(mesh);
            return;
        }
        let sides = if self.side.is_empty() { sides_from_positions(mesh, &self.plane, &self.mirror) } else { self.side.clone() };
        let valid: Vec<bool> = (0..mesh.point_capacity()).map(|p| mesh.point_valid(p)).collect();
        let keys = mesh.point_attrs_mut().get_or_add::<i32>(&symmetry_layer_name(axis));
        let mut next = 1;
        for p in 0..valid.len() as Index {
            if !valid[p as usize] {
                continue;
            }
            let q = self.mirror_point(p);
            if q == INVALID || !valid.get(q as usize).copied().unwrap_or(false) || self.mirror_point(q) != p {
                keys[p as usize] = 0;
            } else if q == p {
                keys[p as usize] = SYMMETRY_ON_PLANE;
            } else if p < q {
                let plus = p as usize >= sides.len() || sides[p as usize] >= 0;
                keys[p as usize] = if plus { next } else { -next };
                keys[q as usize] = if plus { -next } else { next };
                next += 1;
            }
        }
    }

    /// Writes this map into its own layer: the axis plane's for a mirror, the radial one for radial symmetry.
    pub fn store(&self, mesh: &mut Mesh) {
        let Some(radial) = &self.radial else {
            self.store_axis(mesh, if self.axis < 0 { 0 } else { self.axis });
            return;
        };
        let valid: Vec<bool> = (0..mesh.point_capacity()).map(|p| mesh.point_valid(p)).collect();
        let n = self.order() as usize;
        let mut out = vec![0i32; valid.len()];
        let mut id: i64 = 0;
        for base in (0..self.orbit_points.len()).step_by(n) {
            let first = self.orbit_points[base];
            if self.orbit_points[base..base + n].iter().all(|&q| q == first) {
                if valid[first as usize] {
                    out[first as usize] = SYMMETRY_ON_PLANE;
                }
                continue;
            }
            for g in 0..n {
                let q = self.orbit_points[base + g];
                if valid[q as usize] && self.slot[q as usize] as usize == g {
                    out[q as usize] = (id * SLOT_BITS + self.stored_slot(q) as i64 + 1) as i32;
                }
            }
            id += 1;
        }
        let keys = mesh.point_attrs_mut().get_or_add::<i32>(&radial_layer_name(radial));
        let len = keys.len().min(out.len());
        keys[..len].copy_from_slice(&out[..len]);
    }

    fn reflected(&self, p: DVec3) -> DVec3 {
        if self.axis < 0 {
            return self.plane.reflect(p);
        }
        let mut r = p;
        r[self.axis as usize] = 2.0 * self.plane.offset - r[self.axis as usize];
        r
    }

    /// Sets each given point's follower to the exact reflection of its partner on the driving side (drive > 0: the
    /// positive side), and on-plane points exactly onto the plane. Radial: drive > 0 drives from wedge 0, else
    /// wedge 1.
    pub fn enforce(&self, mesh: &mut Mesh, points: &[Index], drive: i32) {
        if self.radial.is_some() {
            self.enforce_wedge(mesh, points, if drive < 0 && self.order() > 1 { 1 } else { 0 });
            return;
        }
        let drive = if drive < 0 { -1 } else { 1 };
        for &p in points {
            if !mesh.point_valid(p) {
                continue;
            }
            let q = self.mirror_point(p);
            if q == INVALID || !mesh.point_valid(q) {
                continue;
            }
            if q == p {
                if self.free_seam {
                    continue;
                }
                let mut v = mesh.position(p);
                if self.axis >= 0 {
                    v[self.axis as usize] = self.plane.offset;
                } else {
                    v = self.plane.project(v);
                }
                mesh.set_position(p, v);
                continue;
            }
            let p_drives = self.side(p) == drive || (self.side(p) == 0 && p < q);
            let (from, to) = if p_drives { (p, q) } else { (q, p) };
            let r = self.reflected(mesh.position(from));
            mesh.set_position(to, r);
        }
    }

    /// enforce() over every point.
    pub fn symmetrize(&self, mesh: &mut Mesh, drive: i32) {
        let all: Vec<Index> = (0..mesh.point_capacity()).filter(|&p| mesh.point_valid(p)).collect();
        self.enforce(mesh, &all, drive);
    }

    pub fn check(&self, mesh: &Mesh) -> SymmetryCheck {
        let mut out = SymmetryCheck::default();
        let mut unmatched = Vec::new();
        for p in 0..mesh.point_capacity() {
            if !mesh.point_valid(p) {
                continue;
            }
            let q = self.mirror_point(p);
            if q == INVALID || !mesh.point_valid(q) {
                out.unmatched += 1;
                unmatched.push(p);
                continue;
            }
            let err;
            if self.radial.is_some() {
                // Off by how far the orbit's wedge 0 point, carried to p's slot, lands from p; fixed points also by
                // how far off their axis or plane.
                let rep = self.orbit_point(p, 0);
                let mut e = (self.transform(self.slot[p as usize] as i32, mesh.position(rep)) - mesh.position(p)).length();
                if self.fixed(p) {
                    out.on_plane += 1;
                    e = e.max((self.constrain(p, mesh.position(p)) - mesh.position(p)).length());
                } else if self.slot[p as usize] == 0 {
                    out.pairs += 1;
                }
                err = e;
            } else if q == p {
                out.on_plane += 1;
                err = self.plane.distance(mesh.position(p)).abs();
            } else {
                if p < q {
                    out.pairs += 1;
                }
                err = (self.reflected(mesh.position(q)) - mesh.position(p)).length();
            }
            if err > out.max_asymmetry {
                out.max_asymmetry = err;
                out.worst_point = p;
            }
        }
        // A point without a partner is off by how far its reflection lies from the surface.
        if !unmatched.is_empty() && mesh.num_faces() > 0 {
            let bvh = MeshBvh::new(mesh);
            for p in unmatched {
                if let Some(hit) = bvh.closest_point(self.transform(1 % self.order(), mesh.position(p))) {
                    if hit.distance > out.max_asymmetry {
                        out.max_asymmetry = hit.distance;
                        out.worst_point = p;
                    }
                }
            }
        }
        out
    }

    /// +1 or -1 for the side a paired point belongs to, 0 on the plane or unpaired. Radial: +1 in wedge 0, -1 in the
    /// others.
    pub fn side(&self, p: Index) -> i32 {
        if self.radial.is_some() {
            let w = self.wedge(p);
            return if w == 0 {
                1
            } else if w > 0 {
                -1
            } else {
                0
            };
        }
        self.side.get(p as usize).map_or(0, |&s| s as i32)
    }

    /// The side most of the points are on, +1 when tied.
    pub fn majority_side(&self, points: &[Index]) -> i32 {
        if self.radial.is_some() {
            return if self.majority_wedge(points) == 0 { 1 } else { -1 };
        }
        let sum: i64 = points.iter().map(|&p| self.side(p) as i64).sum();
        if sum < 0 {
            -1
        } else {
            1
        }
    }

    pub fn plane(&self) -> &SymmetryPlane {
        &self.plane
    }
    /// 0, 1 or 2 for a plane normal to an axis, -1 for any other plane (and radial symmetry).
    pub fn axis(&self) -> i32 {
        self.axis
    }
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }
    pub fn is_empty(&self) -> bool {
        if self.radial.is_some() {
            self.orbit.is_empty()
        } else {
            self.mirror.is_empty()
        }
    }

    /// The partner across the plane; for radial symmetry the image under element 1 (one wedge on).
    pub fn mirror_point(&self, p: Index) -> Index {
        if self.radial.is_some() {
            return self.image(1, p);
        }
        self.mirror.get(p as usize).copied().unwrap_or(INVALID)
    }

    /// On the plane; for radial symmetry fixed() (on the axis or a dihedral mirror plane).
    pub fn on_plane(&self, p: Index) -> bool {
        if self.radial.is_some() {
            return self.fixed(p);
        }
        self.on_plane.get(p as usize).is_some_and(|&v| v != 0)
    }

    /// Points without a partner.
    pub fn unmatched_count(&self) -> Index {
        self.unmatched
    }

    pub fn mirror_edge(&self, mesh: &Mesh, e: Index) -> Index {
        self.image_edge(mesh, 1 % self.order(), e)
    }
    pub fn mirror_face(&self, mesh: &Mesh, f: Index) -> Index {
        self.image_face(mesh, 1 % self.order(), f)
    }

    /// The selection plus its mirror (every image, for radial symmetry), sorted and unique.
    pub fn expand_points(&self, points: &[Index]) -> Vec<Index> {
        let images: Vec<Index> = (1..self.order()).flat_map(|g| points.iter().map(move |&p| self.image(g, p))).collect();
        merged(points, &images)
    }
    pub fn expand_edges(&self, mesh: &Mesh, edges: &[Index]) -> Vec<Index> {
        let images: Vec<Index> =
            (1..self.order()).flat_map(|g| edges.iter().map(move |&e| self.image_edge(mesh, g, e))).collect();
        merged(edges, &images)
    }
    pub fn expand_faces(&self, mesh: &Mesh, faces: &[Index]) -> Vec<Index> {
        let images: Vec<Index> =
            (1..self.order()).flat_map(|g| faces.iter().map(move |&f| self.image_face(mesh, g, f))).collect();
        merged(faces, &images)
    }

    /// Just the mirror images (element 1's, for radial symmetry), in input order; INVALID where there is none.
    pub fn mirror_points(&self, points: &[Index]) -> Vec<Index> {
        points.iter().map(|&p| self.mirror_point(p)).collect()
    }
    pub fn mirror_edges(&self, mesh: &Mesh, edges: &[Index]) -> Vec<Index> {
        edges.iter().map(|&e| self.mirror_edge(mesh, e)).collect()
    }
    pub fn mirror_faces(&self, mesh: &Mesh, faces: &[Index]) -> Vec<Index> {
        faces.iter().map(|&f| self.mirror_face(mesh, f)).collect()
    }

    /// Snaps the given points within tolerance onto the plane (radial: the axis or a mirror plane).
    pub fn snap_to_plane(&self, mesh: &mut Mesh, points: &[Index]) {
        if let Some(radial) = &self.radial {
            // Fixed points stay on their axis or plane however far an op moved them.
            for &p in points {
                if mesh.point_valid(p) {
                    let pos = mesh.position(p);
                    let v = if self.fixed(p) { self.constrain(p, pos) } else { snap_radial(radial, pos, self.tolerance) };
                    mesh.set_position(p, v);
                }
            }
            return;
        }
        for &p in points {
            if mesh.point_valid(p) && self.plane.distance(mesh.position(p)).abs() <= self.tolerance {
                let v = self.plane.project(mesh.position(p));
                mesh.set_position(p, v);
            }
        }
    }

    /// Snaps all points of the mesh from first_point on.
    pub fn snap_new_points(&self, mesh: &mut Mesh, first_point: Index) {
        for p in first_point..mesh.point_capacity() {
            if !mesh.point_valid(p) {
                continue;
            }
            let pos = mesh.position(p);
            if let Some(radial) = &self.radial {
                mesh.set_position(p, snap_radial(radial, pos, self.tolerance));
            } else if self.plane.distance(pos).abs() <= self.tolerance {
                mesh.set_position(p, self.plane.project(pos));
            }
        }
    }

    pub fn is_radial(&self) -> bool {
        self.radial.is_some()
    }
    /// The radial group; only meaningful when is_radial().
    pub fn radial(&self) -> &RadialSymmetry {
        self.radial.as_ref().unwrap_or(&NO_RADIAL)
    }
    /// Number of group elements: 2 for a mirror.
    pub fn order(&self) -> i32 {
        self.radial.as_ref().map_or(2, |r| r.order())
    }

    /// p's image under element g: g 0 is p, for a mirror g 1 its partner. INVALID when unmatched.
    pub fn image(&self, g: i32, p: Index) -> Index {
        let Some(radial) = &self.radial else {
            return if g == 0 { p } else { self.mirror_point(p) };
        };
        if p as usize >= self.orbit.len() || self.orbit[p as usize] == INVALID {
            return INVALID;
        }
        self.orbit_points[self.orbit[p as usize] as usize * self.order() as usize
            + radial.compose(g, self.slot[p as usize] as i32) as usize]
    }

    /// The member of p's orbit in slot g: g's image of the orbit's point in wedge 0 (a mirror's positive side).
    pub fn orbit_point(&self, p: Index, g: i32) -> Index {
        if self.radial.is_none() {
            let q = self.mirror_point(p);
            if q == INVALID {
                return if g == 0 { p } else { INVALID };
            }
            return if (g == 0) == (self.side(p) >= 0) { p } else { q };
        }
        if p as usize >= self.orbit.len() || self.orbit[p as usize] == INVALID {
            return INVALID;
        }
        self.orbit_points[self.orbit[p as usize] as usize * self.order() as usize + g as usize]
    }

    pub fn image_edge(&self, mesh: &Mesh, g: i32, e: Index) -> Index {
        if !mesh.edge_valid(e) {
            return INVALID;
        }
        let a = self.image(g, mesh.edge_point(e, 0));
        let b = self.image(g, mesh.edge_point(e, 1));
        if a == INVALID || b == INVALID {
            INVALID
        } else {
            mesh.find_edge(a, b)
        }
    }

    pub fn image_face(&self, mesh: &Mesh, g: i32, f: Index) -> Index {
        if !mesh.face_valid(f) {
            return INVALID;
        }
        let e = self.image_edge(mesh, g, mesh.corner_edge(mesh.face_corner(f)));
        if e == INVALID {
            return INVALID;
        }
        let mut found = INVALID;
        mesh.for_each_edge_corner(e, |r| {
            let h = mesh.corner_face(r);
            if found != INVALID || mesh.face_size(h) != mesh.face_size(f) {
                return;
            }
            let mut all = true;
            mesh.for_each_face_corner(f, |fc| {
                let m = self.image(g, mesh.corner_point(fc));
                all &= m != INVALID && mesh.find_corner(h, m) != INVALID;
            });
            if all {
                found = h;
            }
        });
        found
    }

    /// Position image under element g.
    pub fn transform(&self, g: i32, p: DVec3) -> DVec3 {
        match &self.radial {
            Some(r) => r.apply(g, p),
            None => {
                if g == 0 {
                    p
                } else {
                    self.reflected(p)
                }
            }
        }
    }

    /// Direction image under element g.
    pub fn transform_vector(&self, g: i32, v: DVec3) -> DVec3 {
        match &self.radial {
            Some(r) => r.apply_vector(g, v),
            None => {
                if g == 0 {
                    v
                } else {
                    v - 2.0 * v.dot(self.plane.normal) * self.plane.normal
                }
            }
        }
    }

    /// True when element g reverses orientation (a mirror's element 1, a dihedral group's reflections).
    pub fn reflects(&self, g: i32) -> bool {
        self.radial.as_ref().map_or(g == 1, |r| r.reflects(g))
    }
    /// The element that undoes g.
    pub fn inverse(&self, g: i32) -> i32 {
        self.radial.as_ref().map_or(g, |r| r.inverse(g))
    }
    /// The element applying b, then a.
    pub fn compose(&self, a: i32, b: i32) -> i32 {
        self.radial.as_ref().map_or(a ^ b, |r| r.compose(a, b))
    }

    /// The slot a matched point fills (a mirror's positive side 0, negative 1); -1 for fixed or unmatched points.
    pub fn wedge(&self, p: Index) -> i32 {
        if self.radial.is_none() {
            let s = self.side(p);
            return if s > 0 {
                0
            } else if s < 0 {
                1
            } else {
                -1
            };
        }
        if p as usize >= self.orbit.len() || self.orbit[p as usize] == INVALID || self.fixed[p as usize] != 0 {
            return -1;
        }
        self.slot[p as usize] as i32
    }

    /// The wedge most of the points are in, 0 when tied or none are.
    pub fn majority_wedge(&self, points: &[Index]) -> i32 {
        let mut count = vec![0i64; self.order() as usize];
        for &p in points {
            let w = self.wedge(p);
            if w >= 0 {
                count[w as usize] += 1;
            }
        }
        // First maximum, as std::max_element.
        let mut best = 0;
        for (i, &c) in count.iter().enumerate() {
            if c > count[best] {
                best = i;
            }
        }
        best as i32
    }

    /// True when p is its orbit's point in wedge w, so it drives the rest (fixed and unmatched points always do).
    pub fn drives(&self, p: Index, w: i32) -> bool {
        let q = self.orbit_point(p, w);
        q == p || q == INVALID
    }

    /// On the plane, the axis, or a dihedral mirror plane: some element besides the identity leaves it in place.
    pub fn fixed(&self, p: Index) -> bool {
        if self.radial.is_none() {
            return self.on_plane.get(p as usize).is_some_and(|&v| v != 0);
        }
        self.fixed.get(p as usize).is_some_and(|&v| v != 0)
    }

    /// The elements that leave an orbit's slot 0 point in place, identity included.
    fn stabilizer(&self, orbit: Index) -> Vec<i32> {
        let n = self.order() as usize;
        let base = orbit as usize * n;
        (0..n as i32).filter(|&g| self.orbit_points[base + g as usize] == self.orbit_points[base]).collect()
    }

    /// pos moved onto the set p must stay on (its plane or axis); pos unchanged for points that are not fixed.
    pub fn constrain(&self, p: Index, pos: DVec3) -> DVec3 {
        if !self.fixed(p) {
            return pos;
        }
        let Some(radial) = &self.radial else {
            let mut v = pos;
            if self.axis >= 0 {
                v[self.axis as usize] = self.plane.offset;
            } else {
                v = self.plane.project(v);
            }
            return v;
        };
        // The slot 0 point's stabilizer, carried to p's slot: conjugate by the element that took slot 0 to p.
        let s = self.slot[p as usize] as i32;
        let stab: Vec<i32> = self
            .stabilizer(self.orbit[p as usize])
            .into_iter()
            .map(|h| radial.compose(s, radial.compose(h, radial.inverse(s))))
            .collect();
        radial.project_fixed(pos, &stab)
    }

    /// Sets each given point's orbit to the exact images of its member in wedge w, fixed points onto their plane or
    /// axis.
    pub fn enforce_wedge(&self, mesh: &mut Mesh, points: &[Index], w: i32) {
        let Some(radial) = &self.radial else {
            self.enforce(mesh, points, if w == 1 { -1 } else { 1 });
            return;
        };
        let n = self.order();
        let w = w.clamp(0, n - 1);
        let mut done = vec![false; self.orbit_points.len() / n as usize];
        for &p in points {
            if !mesh.point_valid(p) || p as usize >= self.orbit.len() || self.orbit[p as usize] == INVALID {
                continue;
            }
            let o = self.orbit[p as usize];
            if done[o as usize] {
                continue;
            }
            done[o as usize] = true;
            let base = o as usize * n as usize;
            let driver = self.orbit_points[base + w as usize];
            if !mesh.point_valid(driver) {
                continue;
            }
            let mut rep = radial.apply(radial.inverse(w), mesh.position(driver));
            if self.fixed[driver as usize] != 0 {
                rep = radial.project_fixed(rep, &self.stabilizer(o));
            }
            // A point filling several slots (on the axis or a mirror plane) is written once, from its first slot.
            for g in 0..n {
                let q = self.orbit_points[base + g as usize];
                if mesh.point_valid(q) && self.slot[q as usize] as i32 == g {
                    mesh.set_position(q, radial.apply(g, rep));
                }
            }
        }
    }

    /// enforce_wedge() over every point.
    pub fn symmetrize_wedge(&self, mesh: &mut Mesh, w: i32) {
        let all: Vec<Index> = (0..mesh.point_capacity()).filter(|&p| mesh.point_valid(p)).collect();
        self.enforce_wedge(mesh, &all, w);
    }

    /// Valid points without a partner or orbit, for highlighting.
    pub fn unmatched_points(&self, mesh: &Mesh) -> Vec<Index> {
        (0..mesh.point_capacity())
            .filter(|&p| mesh.point_valid(p) && self.image(1 % self.order(), p) == INVALID)
            .collect()
    }

    /// Replaces every orbit's values (per point id) by their symmetric average: each member's value carried back to
    /// slot 0 by its element, averaged, and carried out again. positions: points (fixed ones also land on their plane
    /// or axis); else directions, such as displacements.
    pub fn average_orbits(&self, values: &mut [DVec3], positions: bool) {
        let n = self.order();
        let mut members = vec![INVALID; n as usize];
        for p in 0..values.len() as Index {
            // Each orbit once, from its slot 0 point.
            if self.orbit_point(p, 0) != p {
                continue;
            }
            let mut ok = true;
            for g in 0..n {
                if !ok {
                    break;
                }
                members[g as usize] = self.orbit_point(p, g);
                ok = members[g as usize] != INVALID && (members[g as usize] as usize) < values.len();
            }
            if !ok {
                continue;
            }
            let mut sum = DVec3::ZERO;
            for g in 0..n {
                let v = values[members[g as usize] as usize];
                sum += if positions { self.transform(self.inverse(g), v) } else { self.transform_vector(self.inverse(g), v) };
            }
            let mut avg = sum / n as f64;
            if positions {
                avg = self.constrain(p, avg);
            }
            for g in 0..n {
                values[members[g as usize] as usize] =
                    if positions { self.transform(g, avg) } else { self.transform_vector(g, avg) };
            }
        }
    }

    /// Adds an orbit from the points at p's image under every element (images[g] for g); false if it clashes.
    fn add_orbit(&mut self, mesh: &Mesh, images: &[Index], rep_element: i32) -> bool {
        let n = self.order();
        for &q in images {
            if q == INVALID || !mesh.point_valid(q) || self.orbit[q as usize] != INVALID {
                return false;
            }
        }
        let radial = self.radial.as_ref().unwrap();
        let orbit = (self.orbit_points.len() / n as usize) as Index;
        let base = self.orbit_points.len();
        for g in 0..n {
            self.orbit_points.push(images[radial.compose(g, rep_element) as usize]);
        }
        for g in 0..n {
            let q = self.orbit_points[base + g as usize];
            if self.orbit[q as usize] == orbit {
                self.fixed[q as usize] = 1;
                continue;
            }
            self.orbit[q as usize] = orbit;
            self.slot[q as usize] = g as i16;
        }
        true
    }

    fn count_unmatched(&mut self, mesh: &Mesh) {
        self.unmatched =
            (0..mesh.point_capacity()).filter(|&p| mesh.point_valid(p) && self.orbit[p as usize] == INVALID).count() as Index;
    }

    /// The slot p's key stores: its own, except an orbit's slot 0 point on a mirror plane, which stores its
    /// reflection.
    fn stored_slot(&self, p: Index) -> i32 {
        let s = self.slot[p as usize] as i32;
        if s != 0 || self.fixed[p as usize] == 0 {
            return s;
        }
        // The orbit's slot 0 point on a mirror plane stores the reflection that fixes it, which reading needs.
        let radial = self.radial.as_ref().unwrap();
        self.stabilizer(self.orbit[p as usize]).into_iter().find(|&h| radial.reflects(h)).unwrap_or(0)
    }
}

/// Runs after an op: snaps new points and the old on-plane points it moved back onto the plane (radial: the axis or a
/// mirror plane).
pub fn apply_symmetry_snap(mesh: &mut Mesh, symmetry: Option<&SymmetryMap>, first_new_point: Index) {
    let Some(sym) = symmetry else { return };
    let on_plane: Vec<Index> = (0..first_new_point.min(mesh.point_capacity())).filter(|&p| sym.on_plane(p)).collect();
    sym.snap_to_plane(mesh, &on_plane);
    sym.snap_new_points(mesh, first_new_point);
}
