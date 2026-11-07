//! Edge bevel: strips across bevelled edges with superellipse profiles, and vertex meshes at their ends.

use std::cell::Cell;
use std::collections::{BTreeMap, HashMap};
use std::f64::consts::FRAC_PI_2;

use glam::{DMat3, DVec2, DVec3, DVec4};

use super::common::*;
use crate::attribute::{names, AttrType, Index, INVALID};
use crate::mesh::Mesh;
use crate::mix::mix3;
use crate::normals::set_hard_edges_by_angle;
use crate::symmetry::{apply_symmetry_snap, SymmetryMap};
use crate::topology::{edges_of_faces, points_of_edges, unique_sorted};

#[derive(Clone, Copy, Debug)]
pub struct BevelOptions<'a> {
    /// Width from the original edge to each new edge, measured perpendicular in the faces.
    pub offset: f64,
    pub segments: i32,
    /// 0.5 round, 0.25 straight, towards 1 square.
    pub profile: f64,
    /// Keep each offset under half of the shortest neighbouring edge.
    pub clamp: bool,
    /// New edges sharper than this are marked hard; negative leaves them soft.
    pub smoothing_angle: f64,
    pub symmetry: Option<&'a SymmetryMap>,
    /// When set, receives the offset actually used after clamping.
    pub applied_offset: Option<&'a Cell<f64>>,
}

impl Default for BevelOptions<'_> {
    fn default() -> Self {
        Self {
            offset: 5.0,
            segments: 1,
            profile: 0.5,
            clamp: true,
            smoothing_angle: 30.0,
            symmetry: None,
            applied_offset: None,
        }
    }
}

/// An unbevelled edge closer than this (sine of the angle) to the bevelled one cannot be slid along.
const MIN_SLIDE: f64 = 0.2;
/// Faces closer than this (1 - cosine of the angle between them) to lying flat on each other have no room for a strip.
const FLAT_FOLD: f64 = 1e-6;

/// Corner attribute recipe from corners of the original faces.
#[derive(Clone, Debug, Default)]
struct Mix {
    src: Vec<Index>,
    w: Vec<f32>,
}

impl Mix {
    fn one(c: Index) -> Self {
        Self { src: vec![c], w: vec![1.0] }
    }
    fn two(a: Index, b: Index, t: f64) -> Self {
        Self { src: vec![a, b], w: vec![(1.0 - t) as f32, t as f32] }
    }
    fn lerp(a: &Mix, b: &Mix, t: f64) -> Self {
        let mut m = Mix::default();
        m.add(a, 1.0 - t);
        m.add(b, t);
        m
    }
    fn add(&mut self, m: &Mix, s: f64) {
        for i in 0..m.src.len() {
            self.src.push(m.src[i]);
            self.w.push((m.w[i] as f64 * s) as f32);
        }
    }
    fn apply(&self, mesh: &mut Mesh, c: Index) {
        if self.src.is_empty() {
            return;
        }
        // Merged per source, so non-float layers take the corner with the most weight overall.
        let mut s: Vec<Index> = Vec::new();
        let mut ws: Vec<f32> = Vec::new();
        for i in 0..self.src.len() {
            match s.iter().position(|&x| x == self.src[i]) {
                None => {
                    s.push(self.src[i]);
                    ws.push(self.w[i]);
                }
                Some(at) => ws[at] += self.w[i],
            }
        }
        mesh.corner_attrs_mut().mix(c as usize, &s, &ws);
    }
}

/// d turned by the rotation taking unit normal from onto to, about the hinge when they are opposite.
fn unfold(d: DVec3, from: DVec3, to: DVec3, hinge: DVec3) -> DVec3 {
    let mut k = from.cross(to);
    let mut s = k.length();
    let mut c = from.dot(to);
    if s < 1e-9 {
        if c > 0.0 || hinge.length() <= 0.0 {
            return d;
        }
        k = hinge.normalize();
        s = 0.0;
        c = -1.0;
    } else {
        k /= s;
    }
    d * c + k.cross(d) * s + k * k.dot(d) * (1.0 - c)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ChainKind {
    /// The bevelled point itself.
    Point,
    /// Along an unbevelled edge.
    Slide,
    /// Inside the face.
    Offset,
    /// Added on a straight side of the vertex mesh.
    Extra,
}

/// One of the points replacing the bevelled point in a fan face, in face order.
#[derive(Clone, Debug)]
struct ChainPoint {
    kind: ChainKind,
    /// Slide: the edge it moves along.
    edge: Index,
    /// Displacement per unit offset.
    dir: DVec3,
    point: Index,
    mix: Mix,
    /// Attributes for the edge to the next point of the face.
    src_edge: Index,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FaceKind {
    /// No bevelled edge in the face at this point.
    Keep,
    /// Both edges bevelled.
    Miter,
    /// In-edge bevelled, slides along the out-edge.
    SlideOut,
    /// Out-edge bevelled, slides along the in-edge.
    SlideIn,
    /// In-edge bevelled but the out-edge cannot be slid along.
    OffsetIn,
    /// Out-edge bevelled but the in-edge cannot be slid along.
    OffsetOut,
}

/// Fan of one bevelled point. Face i lies between edges[i] (coming in) and out_edge(i).
#[derive(Clone, Debug)]
struct Vert {
    point: Index,
    closed: bool,
    corners: Vec<Index>,
    edges: Vec<Index>,
    kind: Vec<FaceKind>,
    offset_dir: Vec<DVec3>,
    chain: Vec<Vec<ChainPoint>>,
    /// Profile across edges[i] when it is bevelled.
    profile_at: Vec<usize>,
}

impl Vert {
    fn size(&self) -> usize {
        self.corners.len()
    }
    fn in_edge(&self, i: usize) -> Index {
        self.edges[i]
    }
    fn out_edge(&self, i: usize) -> Index {
        self.edges[if self.closed { (i + 1) % self.corners.len() } else { i + 1 }]
    }
}

fn super_r(profile: f64) -> f64 {
    let pr = profile.clamp(0.01, 0.999);
    2.0 * 0.5f64.ln() / pr.ln()
}

/// Point of the unit superellipse x^r + y^r = 1 at t in [0, 1], a quarter turn. Symmetric in t and 1 - t.
fn super_xy(t: f64, r: f64) -> DVec2 {
    if t <= 0.0 {
        return DVec2::new(1.0, 0.0);
    }
    if t >= 1.0 {
        return DVec2::new(0.0, 1.0);
    }
    let c = (t * FRAC_PI_2).cos();
    let s = ((1.0 - t) * FRAC_PI_2).cos();
    DVec2::new(c.powf(2.0 / r), s.powf(2.0 / r))
}

/// Profile across a bevelled edge at one of its ends: the superellipse o + u x + w y from lo to hi.
#[derive(Clone, Debug)]
struct Profile {
    lo: Index,
    hi: Index,
    o: DVec3,
    u: DVec3,
    w: DVec3,
    linear: bool,
    /// Tangency constraints dot(n, o) = d.
    planes: Vec<DVec4>,
    /// Interior points, lo to hi.
    ids: Vec<Index>,
    /// Direction of the bevelled edge it crosses.
    axis: DVec3,
    /// The kept point, lying on the profile in segment through_seg (lo to hi).
    through: Index,
    through_seg: usize,
}

impl Profile {
    fn new(lo: Index, hi: Index) -> Self {
        Self {
            lo,
            hi,
            o: DVec3::ZERO,
            u: DVec3::ZERO,
            w: DVec3::ZERO,
            linear: false,
            planes: Vec::new(),
            ids: Vec::new(),
            axis: DVec3::ZERO,
            through: INVALID,
            through_seg: 0,
        }
    }
    fn at(&self, s: f64, r: f64) -> DVec3 {
        if self.linear {
            return self.o + self.u * (1.0 - s) + self.w * s;
        }
        let xy = super_xy(s, r);
        self.o + self.u * xy.x + self.w * xy.y
    }
}

fn safe_normalize(v: DVec3) -> DVec3 {
    let l = v.length();
    if l > 1e-300 {
        v / l
    } else {
        DVec3::ZERO
    }
}

/// The point nearest `near` that best satisfies the planes in the least-squares sense; directions the planes leave
/// free stay at `near`.
fn nearest_on_planes(planes: &[DVec4], near: DVec3) -> DVec3 {
    let mut a = [[0.0f64; 3]; 3];
    let mut y = [0.0f64; 3];
    for pl in planes {
        let n = pl.truncate();
        let res = pl.w - n.dot(near);
        for i in 0..3 {
            y[i] += n[i] * res;
            for j in 0..3 {
                a[i][j] += n[i] * n[j];
            }
        }
    }
    let mut v = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    for _ in 0..32 {
        if a[0][1] * a[0][1] + a[0][2] * a[0][2] + a[1][2] * a[1][2] < 1e-28 {
            break;
        }
        for p in 0..2 {
            for q in p + 1..3 {
                if a[p][q].abs() < 1e-300 {
                    continue;
                }
                let theta = (a[q][q] - a[p][p]) / (2.0 * a[p][q]);
                let t = (if theta >= 0.0 { 1.0 } else { -1.0 }) / (theta.abs() + (theta * theta + 1.0).sqrt());
                let c = 1.0 / (t * t + 1.0).sqrt();
                let s = t * c;
                for k in 0..3 {
                    let (akp, akq) = (a[k][p], a[k][q]);
                    a[k][p] = c * akp - s * akq;
                    a[k][q] = s * akp + c * akq;
                }
                for k in 0..3 {
                    let (apk, aqk) = (a[p][k], a[q][k]);
                    a[p][k] = c * apk - s * aqk;
                    a[q][k] = s * apk + c * aqk;
                }
                for k in 0..3 {
                    let (vkp, vkq) = (v[k][p], v[k][q]);
                    v[k][p] = c * vkp - s * vkq;
                    v[k][q] = s * vkp + c * vkq;
                }
            }
        }
    }
    let top = a[0][0].max(a[1][1]).max(a[2][2]).max(1e-300);
    let mut out = near;
    let yv = DVec3::new(y[0], y[1], y[2]);
    for i in 0..3 {
        if a[i][i] <= 1e-7 * top {
            continue;
        }
        let e = DVec3::new(v[0][i], v[1][i], v[2][i]);
        out += e * (e.dot(yv) / a[i][i]);
    }
    out
}

/// A face corner with its two neighbours, kept so corner recipes can be made after the face is rebuilt.
#[derive(Clone, Copy, Debug)]
struct CornerTri {
    c: Index,
    prev: Index,
    next: Index,
    o: DVec3,
    a: DVec3,
    b: DVec3,
}

impl CornerTri {
    fn of(mesh: &Mesh, c: Index) -> Self {
        let prev = mesh.corner_prev(c);
        let next = mesh.corner_next(c);
        let o = mesh.position(mesh.corner_point(c));
        Self {
            c,
            prev,
            next,
            o,
            a: mesh.position(mesh.corner_point(prev)) - o,
            b: mesh.position(mesh.corner_point(next)) - o,
        }
    }

    /// Recipe for a point in the corner's face, extrapolated from the triangle.
    fn mix(&self, pos: DVec3) -> Mix {
        let d = pos - self.o;
        let aa = self.a.dot(self.a);
        let ab = self.a.dot(self.b);
        let bb = self.b.dot(self.b);
        let det = aa * bb - ab * ab;
        if det.abs() <= 1e-12 * aa * bb {
            return Mix::one(self.c);
        }
        let al = (bb * self.a.dot(d) - ab * self.b.dot(d)) / det;
        let be = (aa * self.b.dot(d) - ab * self.a.dot(d)) / det;
        Mix { src: vec![self.c, self.prev, self.next], w: vec![(1.0 - al - be) as f32, al as f32, be as f32] }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SideType {
    /// A profile across a bevelled edge.
    Arc,
    /// An edge of a rebuilt fan face, chain[face][at + 1] to chain[face][at].
    Straight,
    /// Across the mesh border.
    Open,
}

/// Side of a vertex mesh between two of its boundary points.
#[derive(Clone, Copy, Debug)]
struct BevelSide {
    ty: SideType,
    profile: usize,
    face: usize,
    at: usize,
}

impl BevelSide {
    fn arc(profile: usize) -> Self {
        Self { ty: SideType::Arc, profile, face: 0, at: 0 }
    }
    fn straight(face: usize, at: usize) -> Self {
        Self { ty: SideType::Straight, profile: 0, face, at }
    }
    fn open() -> Self {
        Self { ty: SideType::Open, profile: 0, face: 0, at: 0 }
    }
}

/// The hole around a bevelled point: nodes[k] to nodes[k + 1] along sides[k], wound like the faces that will fill it.
#[derive(Clone, Debug, Default)]
struct Hole {
    nodes: Vec<Index>,
    sides: Vec<BevelSide>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Fill {
    #[default]
    None,
    /// Terminal edge: the profile goes into the face beside it.
    Merge,
    /// One face.
    Polygon,
    /// Quads around the kept point.
    Fan,
    /// Blender's quad grid corner.
    Grid,
    /// The boundary visits a point twice: one face per simple loop.
    Split,
}

#[derive(Clone, Debug, Default)]
struct VertMesh {
    hole: Hole,
    fill: Fill,
    /// Grid: the n + 1 points of each side, node to node.
    side_ids: Vec<Vec<Index>>,
}

/// Welds the ends of the new edges (from first_edge on) a clamp closed, so faces that ran out of room close fully.
/// Faces left with under three points go, and faces follows the ones that were rebuilt.
fn weld_closed_edges(mesh: &mut Mesh, first_edge: Index, tol: f64, faces: &mut Vec<Index>) {
    let mut target = vec![INVALID; mesh.point_capacity() as usize];
    let find = |target: &[Index], mut p: Index| {
        while target[p as usize] != INVALID {
            p = target[p as usize];
        }
        p
    };
    let mut any = false;
    for e in first_edge..mesh.edge_capacity() {
        if mesh.edge_valid(e) && mesh.edge_length(e) <= tol {
            let a = find(&target, mesh.edge_point(e, 0));
            let b = find(&target, mesh.edge_point(e, 1));
            if a != b {
                target[a.max(b) as usize] = a.min(b);
                any = true;
            }
        }
    }
    if !any {
        return;
    }
    for p in 0..target.len() as Index {
        if target[p as usize] != INVALID {
            target[p as usize] = find(&target, p);
        }
    }
    let mut rebuilt = HashMap::new();
    weld_points_rebuilt(mesh, &target, Some(&mut rebuilt));
    for f in faces.iter_mut() {
        if let Some(&n) = rebuilt.get(f) {
            *f = n;
        }
    }
    faces.retain(|&f| f != INVALID);
}

/// Sets "seam" on the edges of the faces where the corners on the two sides carry different wedge labels.
fn mark_wedge_seams(mesh: &mut Mesh, faces: &[Index], label: &HashMap<Index, i32>) {
    let of = |c: Index| label.get(&c).copied().unwrap_or(-1);
    let differ = |c: Index, r: Index| {
        let (a, b) = (of(c), of(r));
        a >= 0 && b >= 0 && a != b
    };
    let mut set = Vec::new();
    let mut any = false;
    for &f in faces {
        for c in mesh.face_corners(f) {
            let r = mesh.corner_radial(c);
            if r == c || r == INVALID || mesh.corner_radial(r) != c {
                continue;
            }
            let same = mesh.corner_point(r) == mesh.corner_point(c);
            let seam = differ(c, if same { r } else { mesh.corner_next(r) })
                || differ(mesh.corner_next(c), if same { mesh.corner_next(r) } else { r });
            set.push((mesh.corner_edge(c), seam));
            any |= seam;
        }
    }
    if !any && !mesh.edge_attrs().has(names::SEAM) {
        return;
    }
    let seam = mesh.edge_attrs_mut().get_or_add::<bool>(names::SEAM);
    for (e, on) in set {
        seam[e as usize] = on;
    }
}

/// Faces with no more area than a square of side tol.
fn flat_faces(mesh: &Mesh, tol: f64) -> usize {
    (0..mesh.face_capacity()).filter(|&f| mesh.face_valid(f) && mesh.face_area(f) <= tol * tol).count()
}

/// What a clamp does with edges whose ends would meet.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Close {
    /// Close them on a copy, keeping it unless that leaves flat faces, otherwise stop short.
    Try,
    /// Close them.
    Force,
}

#[derive(Clone, Copy)]
struct FanFrame {
    tri: CornerTri,
    n: DVec3,
    face: Index,
}

#[derive(Clone, Copy, Default)]
struct SlideAcc {
    sum: f64,
    count: i32,
    /// Per unit offset, along the edge.
    dist: f64,
    point: Index,
    /// Fraction of the edge from the bevelled point.
    t: f64,
}

/// Corner recipes of each strip end, across from a (the face where the edge goes out) to b.
#[derive(Clone, Debug)]
struct StripEnd {
    a: Index,
    b: Index,
    ma: Mix,
    mb: Mix,
    label: i32,
}

impl Default for StripEnd {
    fn default() -> Self {
        Self { a: INVALID, b: INVALID, ma: Mix::default(), mb: Mix::default(), label: -1 }
    }
}

impl StripEnd {
    fn at(&self, p: Index) -> &Mix {
        if p == self.a {
            &self.ma
        } else {
            &self.mb
        }
    }
}

struct Insert {
    vert: usize,
    face: usize,
    at: usize,
    pts: Vec<ChainPoint>,
}

fn extra(point: Index, mix: Mix, src: Index) -> ChainPoint {
    ChainPoint { kind: ChainKind::Extra, edge: INVALID, dir: DVec3::ZERO, point, mix, src_edge: src }
}

fn emit(
    mesh: &mut Mesh,
    out: &mut Vec<Index>,
    corner_label: &mut HashMap<Index, i32>,
    pts: &[Index],
    src_face: Index,
    mix: &[Mix],
    labels: &[i32],
) {
    let f = mesh.add_face_from(pts, src_face, &[]);
    if f == INVALID {
        return;
    }
    out.push(f);
    let mut c = mesh.face_corner(f);
    for k in 0..pts.len() {
        mix[k].apply(mesh, c);
        corner_label.insert(c, labels[k]);
        c = mesh.corner_next(c);
    }
}

fn arc_ids(profiles: &[Profile], prof: usize, from: Index) -> Vec<Index> {
    let mut ids = profiles[prof].ids.clone();
    if from != profiles[prof].lo {
        ids.reverse();
    }
    ids
}

fn arc_at(profiles: &[Profile], prof: usize, from: Index, s: f64, r: f64) -> DVec3 {
    let p = &profiles[prof];
    p.at(if from == p.lo { s } else { 1.0 - s }, r)
}

/// The strip surface nearest a grid corner's boundary, in profile coordinates.
#[derive(Clone, Copy)]
struct Surface {
    o: DVec3,
    u: DVec3,
    w: DVec3,
    d: DVec3,
    inv: DMat3,
}

impl Surface {
    /// Profile coordinates of p: x along u, y along w, z along the edge.
    fn coords(&self, p: DVec3) -> DVec3 {
        self.inv * (p - self.o)
    }
    fn snap(&self, p: DVec3, r: f64) -> DVec3 {
        let q = self.coords(p);
        let x = q.x.max(0.0);
        let y = q.y.max(0.0);
        let top = x.max(y);
        if top <= 0.0 {
            return p;
        }
        let norm = top * ((x / top).powf(r) + (y / top).powf(r)).powf(1.0 / r);
        self.o + (self.u * x + self.w * y) / norm + self.d * q.z
    }
}

#[allow(clippy::too_many_arguments)]
fn zip_terminal(
    mesh: &mut Mesh,
    v: &Vert,
    vi: usize,
    h: &Hole,
    segments: i32,
    r: f64,
    profiles: &mut [Profile],
    welded: &mut Vec<Index>,
    inserts: &mut Vec<Insert>,
) -> bool {
    if h.nodes.len() != 3 {
        return false;
    }
    let Some(kv) = h.nodes.iter().position(|&p| p == v.point) else { return false };
    let to_a = h.sides[kv];
    let arc = h.sides[(kv + 1) % 3];
    let from_b = h.sides[(kv + 2) % 3];
    if to_a.ty != SideType::Straight || arc.ty != SideType::Arc || from_b.ty != SideType::Straight {
        return false;
    }
    let a = h.nodes[(kv + 1) % 3];
    let b = h.nodes[(kv + 2) % 3];
    let mut full = vec![a];
    full.extend(arc_ids(profiles, arc.profile, a));
    full.push(b);
    let pv = mesh.position(v.point);
    // Zip when the fan would be flat, face against the surface, or fold back over a face beside it.
    let mut fan_normal = DVec3::ZERO;
    let mut surface = DVec3::ZERO;
    for j in 0..full.len() - 1 {
        fan_normal += (mesh.position(full[j]) - pv).cross(mesh.position(full[j + 1]) - pv);
    }
    for &c in &v.corners {
        surface += mesh.face_normal(mesh.corner_face(c));
    }
    let chord = mesh.position(a).distance(mesh.position(b));
    if fan_normal.length() > 1e-9 * chord * chord {
        let nf = fan_normal.normalize();
        let fold = nf
            .dot(mesh.face_normal(mesh.corner_face(v.corners[to_a.face])))
            .min(nf.dot(mesh.face_normal(mesh.corner_face(v.corners[from_b.face]))));
        if fold > -0.9 && nf.dot(safe_normalize(surface)) > -0.25 {
            return false;
        }
    }
    // The point moves onto the profile where it comes nearest, welding to a profile point close to there.
    let n = segments;
    let mut s_best = 0.5;
    let mut d_best = 1e300;
    for k in 0..=64 * n {
        let sk = k as f64 / (64 * n) as f64;
        let d = arc_at(profiles, arc.profile, a, sk, r).distance(pv);
        if d < d_best {
            d_best = d;
            s_best = sk;
        }
    }
    let forward = a == profiles[arc.profile].lo;
    let jw = (s_best * n as f64).round() as i64;
    // full[1, stop) goes to the face on a's side, full[cut, end - 1) to b's.
    let (stop, cut);
    if jw > 0 && jw < n as i64 && (s_best * n as f64 - jw as f64).abs() <= 0.25 {
        let pr = &mut profiles[arc.profile];
        let idx = if forward { jw as usize - 1 } else { pr.ids.len() - jw as usize };
        let slot_pos = mesh.position(pr.ids[idx]);
        mesh.set_position(v.point, slot_pos);
        welded.push(pr.ids[idx]);
        pr.ids[idx] = v.point;
        stop = jw as usize;
        cut = jw as usize + 1;
    } else {
        let seg = ((s_best * n as f64).floor() as i64).clamp(0, n as i64 - 1) as usize;
        let at = arc_at(profiles, arc.profile, a, s_best, r);
        mesh.set_position(v.point, at);
        let pr = &mut profiles[arc.profile];
        pr.through = v.point;
        pr.through_seg = if forward { seg } else { n as usize - 1 - seg };
        stop = seg + 1;
        cut = seg + 1;
    }
    let mut insert = |side: &BevelSide, from: usize, to: usize| {
        let c = v.corners[side.face];
        let src = v.chain[side.face][side.at].src_edge;
        let pts: Vec<ChainPoint> =
            (from..to).map(|j| extra(full[j], CornerTri::of(mesh, c).mix(mesh.position(full[j])), src)).collect();
        if !pts.is_empty() {
            inserts.push(Insert { vert: vi, face: side.face, at: side.at, pts });
        }
    };
    // Face order runs a to the kept point on one side and the kept point to b on the other.
    insert(&to_a, 1, stop);
    insert(&from_b, cut, full.len() - 1);
    true
}

fn bevel(mesh: &mut Mesh, edges_in: &[Index], opts: &BevelOptions, close: Close) -> Vec<Index> {
    let segments = opts.segments.max(1);
    let r = super_r(opts.profile);
    let mut sel = match opts.symmetry {
        Some(s) => s.expand_edges(mesh, edges_in),
        None => unique_sorted(edges_in),
    };

    // Only manifold, consistently wound edges between manifold points, whose faces do not fold flat onto each other.
    let mut bev = vec![false; mesh.edge_capacity() as usize];
    {
        let mut fan = Vec::new();
        let mut keep = Vec::new();
        for &e in &sel {
            if !mesh.edge_valid(e) || !mesh.is_manifold_edge(e) {
                continue;
            }
            let c = mesh.edge_corner(e);
            if mesh.corner_point(c) == mesh.corner_point(mesh.corner_radial(c)) {
                continue;
            }
            if mesh.face_normal(mesh.corner_face(c)).dot(mesh.face_normal(mesh.corner_face(mesh.corner_radial(c))))
                < FLAT_FOLD - 1.0
            {
                continue;
            }
            let mut ok = true;
            for s in 0..2 {
                if !ok {
                    break;
                }
                let mut closed = false;
                ok = point_fan(mesh, mesh.edge_point(e, s), &mut fan, &mut closed);
            }
            if ok {
                bev[e as usize] = true;
                keep.push(e);
            }
        }
        sel = keep;
    }
    if sel.is_empty() {
        return Vec::new();
    }

    let first_new_point = mesh.point_capacity();
    let edge_cap0 = mesh.edge_capacity();

    let mut verts: Vec<Vert> = Vec::new();
    let mut vert_of: HashMap<Index, usize> = HashMap::new();
    for p in points_of_edges(mesh, &sel) {
        let mut corners = Vec::new();
        let mut closed = false;
        point_fan(mesh, p, &mut corners, &mut closed);
        let mut edges: Vec<Index> = corners.iter().map(|&c| mesh.corner_edge(mesh.corner_prev(c))).collect();
        if !closed {
            edges.push(mesh.corner_edge(*corners.last().unwrap()));
        }
        vert_of.insert(p, verts.len());
        verts.push(Vert {
            point: p,
            closed,
            corners,
            edges,
            kind: Vec::new(),
            offset_dir: Vec::new(),
            chain: Vec::new(),
            profile_at: Vec::new(),
        });
    }

    // UV wedges: the fan faces of each point grouped by the uv seams between them (edge "seam" or a uv split). New
    // corners take a wedge label and their uvs from that wedge, and new edges are seams exactly where labels differ.
    // A strip over a seam belongs to the side whose face has the lower id, so its one seam runs along the other side;
    // a vertex mesh belongs to the wedge of the fan's lowest face.
    let mut fans: Vec<Vec<FanFrame>> = vec![Vec::new(); verts.len()];
    let mut wedge: Vec<Vec<i32>> = vec![Vec::new(); verts.len()];
    let mut strip_seam = vec![false; mesh.edge_capacity() as usize];
    {
        let ca = mesh.corner_attrs();
        let uvs: Vec<&[[f32; 2]]> =
            ca.layers().iter().filter(|l| l.ty() == AttrType::Float2).map(|l| l.values::<[f32; 2]>()).collect();
        let uv_differ = |c1: Index, c2: Index| {
            uvs.iter().any(|uv| {
                let (a, b) = (uv[c1 as usize], uv[c2 as usize]);
                (a[0] - b[0]).abs() > 1e-6 || (a[1] - b[1]).abs() > 1e-6
            })
        };
        let seam = mesh.edge_attrs().get::<bool>(names::SEAM);
        let seam_attr = |e: Index| (e as usize) < seam.len() && seam[e as usize];
        for &e in &sel {
            let c = mesh.edge_corner(e);
            let rc = mesh.corner_radial(c);
            let same = mesh.corner_point(rc) == mesh.corner_point(c);
            strip_seam[e as usize] = seam_attr(e)
                || uv_differ(c, if same { rc } else { mesh.corner_next(rc) })
                || uv_differ(mesh.corner_next(c), if same { mesh.corner_next(rc) } else { rc });
        }
        let mut wedges = 0;
        for (vi, v) in verts.iter().enumerate() {
            let n = v.size();
            for &c in &v.corners {
                fans[vi].push(FanFrame { tri: CornerTri::of(mesh, c), n: mesh.face_normal(mesh.corner_face(c)), face: mesh.corner_face(c) });
            }
            let cut = |i: usize| {
                (i > 0 || v.closed) && (seam_attr(v.in_edge(i)) || uv_differ(v.corners[i], v.corners[(i + n - 1) % n]))
            };
            let w = &mut wedge[vi];
            let first = wedges;
            wedges += 1;
            *w = vec![first; n];
            for i in 1..n {
                w[i] = if cut(i) {
                    let x = wedges;
                    wedges += 1;
                    x
                } else {
                    w[i - 1]
                };
            }
            let last = *w.last().unwrap();
            if v.closed && !cut(0) {
                for x in w.iter_mut() {
                    if *x == last {
                        *x = first;
                    }
                }
            }
        }
    }
    let lowest_face = |vi: usize| {
        let mut best = 0;
        for i in 1..fans[vi].len() {
            if fans[vi][i].face < fans[vi][best].face {
                best = i;
            }
        }
        best
    };

    // Boundary point directions per unit offset. Slides on an edge are shared by the faces on both sides.
    let mut slides: BTreeMap<(usize, Index), SlideAcc> = BTreeMap::new();
    for (vi, v) in verts.iter_mut().enumerate() {
        let pv = mesh.position(v.point);
        v.kind = vec![FaceKind::Keep; v.size()];
        v.offset_dir = vec![DVec3::ZERO; v.size()];
        for i in 0..v.size() {
            let b_in = bev[v.in_edge(i) as usize];
            let b_out = bev[v.out_edge(i) as usize];
            if !b_in && !b_out {
                continue;
            }
            let c = v.corners[i];
            let n = mesh.face_normal(mesh.corner_face(c));
            let e_in = safe_normalize(mesh.position(mesh.corner_point(mesh.corner_prev(c))) - pv);
            let e_out = safe_normalize(mesh.position(mesh.corner_next_point(c)) - pv);
            let perp_in = safe_normalize((e_in - n * e_in.dot(n)).cross(n));
            let perp_out = safe_normalize(n.cross(e_out - n * e_out.dot(n)));
            if b_in && b_out {
                v.kind[i] = FaceKind::Miter;
                v.offset_dir[i] = (perp_in + perp_out) / (1.0 + perp_in.dot(perp_out)).max(0.05);
            } else if b_in {
                let s = e_out.dot(perp_in);
                if s >= MIN_SLIDE {
                    v.kind[i] = FaceKind::SlideOut;
                    let acc = slides.entry((vi, v.out_edge(i))).or_default();
                    acc.sum += 1.0 / s;
                    acc.count += 1;
                } else {
                    v.kind[i] = FaceKind::OffsetIn;
                    v.offset_dir[i] = perp_in;
                }
            } else {
                let s = e_in.dot(perp_out);
                if s >= MIN_SLIDE {
                    v.kind[i] = FaceKind::SlideIn;
                    let acc = slides.entry((vi, v.in_edge(i))).or_default();
                    acc.sum += 1.0 / s;
                    acc.count += 1;
                } else {
                    v.kind[i] = FaceKind::OffsetOut;
                    v.offset_dir[i] = perp_out;
                }
            }
        }
    }
    for acc in slides.values_mut() {
        acc.dist = acc.sum / acc.count as f64;
    }
    let slide_dir = |verts: &[Vert], vi: usize, e: Index| {
        let p = verts[vi].point;
        safe_normalize(mesh.position(mesh.edge_other_point(e, p)) - mesh.position(p)) * slides[&(vi, e)].dist
    };

    // Chains: what replaces the point in each fan face, in face order.
    for vi in 0..verts.len() {
        let size = verts[vi].size();
        let mut chains = vec![Vec::new(); size];
        for (i, ch) in chains.iter_mut().enumerate() {
            let v = &verts[vi];
            let (inn, out) = (v.in_edge(i), v.out_edge(i));
            let s_in = slides.contains_key(&(vi, inn));
            let s_out = slides.contains_key(&(vi, out));
            let slide = |ch: &mut Vec<ChainPoint>, e: Index, src: Index| {
                ch.push(ChainPoint {
                    kind: ChainKind::Slide,
                    edge: e,
                    dir: slide_dir(&verts, vi, e),
                    point: INVALID,
                    mix: Mix::default(),
                    src_edge: src,
                })
            };
            let offset = |ch: &mut Vec<ChainPoint>, src: Index| {
                ch.push(ChainPoint {
                    kind: ChainKind::Offset,
                    edge: INVALID,
                    dir: v.offset_dir[i],
                    point: INVALID,
                    mix: Mix::default(),
                    src_edge: src,
                })
            };
            let point = |ch: &mut Vec<ChainPoint>, src: Index| {
                ch.push(ChainPoint {
                    kind: ChainKind::Point,
                    edge: INVALID,
                    dir: DVec3::ZERO,
                    point: v.point,
                    mix: Mix::default(),
                    src_edge: src,
                })
            };
            match v.kind[i] {
                FaceKind::Miter => offset(ch, out),
                FaceKind::SlideOut => slide(ch, out, out),
                FaceKind::SlideIn => slide(ch, inn, out),
                FaceKind::OffsetIn => {
                    offset(ch, INVALID);
                    point(ch, out);
                    if s_out {
                        slide(ch, out, out);
                    }
                }
                FaceKind::OffsetOut => {
                    if s_in {
                        slide(ch, inn, inn);
                    }
                    point(ch, INVALID);
                    offset(ch, out);
                }
                FaceKind::Keep => {
                    if s_in {
                        slide(ch, inn, if s_out { INVALID } else { inn });
                    }
                    if !(s_in && s_out) {
                        point(ch, out);
                    }
                    if s_out {
                        slide(ch, out, out);
                    }
                }
            }
        }
        verts[vi].chain = chains;
    }

    // Corner -> (vert, fan index).
    let mut fan_slot: HashMap<Index, (usize, usize)> = HashMap::new();
    let mut touched_faces = Vec::new();
    for (vi, v) in verts.iter().enumerate() {
        for (i, &c) in v.corners.iter().enumerate() {
            fan_slot.insert(c, (vi, i));
            touched_faces.push(mesh.corner_face(c));
        }
    }
    let touched_faces = unique_sorted(&touched_faces);

    // Clamp: along every edge of a rebuilt face, the points replacing its two ends must not pass each other. Where
    // they meet the edge closes, and its points are welded at the end.
    let mut offset = opts.offset.max(0.0);
    let mut collapses = false;
    if opts.clamp {
        for &f in &touched_faces {
            for c in mesh.face_corners(f) {
                let cn = mesh.corner_next(c);
                let a = mesh.position(mesh.corner_point(c));
                let b = mesh.position(mesh.corner_point(cn));
                let len = a.distance(b);
                if len <= 0.0 {
                    continue;
                }
                let da = fan_slot.get(&c).map_or(DVec3::ZERO, |&(vi, i)| verts[vi].chain[i].last().unwrap().dir);
                let db = fan_slot.get(&cn).map_or(DVec3::ZERO, |&(vi, i)| verts[vi].chain[i][0].dir);
                let k = (da - db).dot((b - a) / len);
                if k > 1e-12 && len / k < offset {
                    offset = len / k;
                    collapses = true;
                }
            }
        }
    }
    // Points from different corners of one face can still cross inside it, as across a thin leg: shrink until none do.
    if opts.clamp && offset > 0.0 {
        // Closed edges count once, and a face that closes to a line or a point is simple.
        let simple_at = |f: Index, off: f64| {
            let n = mesh.face_normal(f);
            let ax = safe_normalize(n.cross(if n.x.abs() < 0.9 { DVec3::X } else { DVec3::Y }));
            let ay = n.cross(ax);
            let tol = WELD_SHARE * off;
            let mut poly: Vec<DVec2> = Vec::new();
            let mut add = |q: DVec3| {
                let uv = DVec2::new(q.dot(ax), q.dot(ay));
                if poly.last().is_none_or(|l| l.distance(uv) > tol) {
                    poly.push(uv);
                }
            };
            for c in mesh.face_corners(f) {
                let p = mesh.position(mesh.corner_point(c));
                match fan_slot.get(&c) {
                    None => add(p),
                    Some(&(vi, i)) => {
                        for cp in &verts[vi].chain[i] {
                            add(p + cp.dir * off);
                        }
                    }
                }
            }
            while poly.len() > 1 && poly.last().unwrap().distance(poly[0]) <= tol {
                poly.pop();
            }
            poly.len() < 3 || polygon_is_simple(&poly)
        };
        let crossing: Vec<Index> =
            touched_faces.iter().copied().filter(|&f| !simple_at(f, offset) && simple_at(f, offset * 1e-6)).collect();
        if !crossing.is_empty() {
            let (mut lo, mut hi) = (0.0, offset);
            for _ in 0..40 {
                let mid = 0.5 * (lo + hi);
                if crossing.iter().all(|&f| simple_at(f, mid)) {
                    lo = mid;
                } else {
                    hi = mid;
                }
            }
            offset = lo * (1.0 - CLAMP_MARGIN);
            collapses = false;
        }
    }
    // Closing is clean where the meeting points weld; where a point lands on a vertex mesh's side instead, the patch
    // goes flat, and the bevel stops short of it.
    if collapses && close == Close::Try {
        let mut closed = mesh.clone();
        let made = bevel(&mut closed, edges_in, opts, Close::Force);
        let tol = 1e-3 * offset;
        if flat_faces(&closed, tol) <= flat_faces(mesh, tol) {
            *mesh = closed;
            return made;
        }
        offset *= 1.0 - CLAMP_MARGIN;
        collapses = false;
    }
    if let Some(cell) = opts.applied_offset {
        cell.set(offset);
    }
    if offset <= 0.0 {
        return Vec::new();
    }

    // Points.
    for (&(vi, e), acc) in slides.iter_mut() {
        let p = verts[vi].point;
        let w = mesh.edge_other_point(e, p);
        acc.t = acc.dist * offset / mesh.edge_length(e);
        acc.point = mesh.add_point(mix3(mesh.position(p), mesh.position(w), acc.t));
        mesh.point_attrs_mut().mix(acc.point as usize, &[p, w], &[(1.0 - acc.t) as f32, acc.t as f32]);
    }
    for vi in 0..verts.len() {
        let pv = mesh.position(verts[vi].point);
        let vpoint = verts[vi].point;
        for i in 0..verts[vi].size() {
            let c = verts[vi].corners[i];
            let in_edge = verts[vi].in_edge(i);
            for k in 0..verts[vi].chain[i].len() {
                let cp = &verts[vi].chain[i][k];
                let (kind, edge, dir) = (cp.kind, cp.edge, cp.dir);
                let (point, mix) = match kind {
                    ChainKind::Point => (cp.point, Mix::one(c)),
                    ChainKind::Slide => {
                        let acc = &slides[&(vi, edge)];
                        let is_in = edge == in_edge;
                        (acc.point, Mix::two(c, if is_in { mesh.corner_prev(c) } else { mesh.corner_next(c) }, acc.t))
                    }
                    ChainKind::Offset => {
                        let np = mesh.add_point(pv + dir * offset);
                        mesh.point_attrs_mut().copy(np as usize, vpoint as usize);
                        (np, CornerTri::of(mesh, c).mix(mesh.position(np)))
                    }
                    ChainKind::Extra => (cp.point, cp.mix.clone()),
                };
                let cp = &mut verts[vi].chain[i][k];
                cp.point = point;
                cp.mix = mix;
            }
        }
    }

    // Profiles at each end of each bevelled edge, shared by the strips whose ends meet the same two points.
    let mut profiles: Vec<Profile> = Vec::new();
    let mut profile_of: BTreeMap<(usize, Index, Index), usize> = BTreeMap::new();
    for vi in 0..verts.len() {
        let size = verts[vi].size();
        verts[vi].profile_at = vec![usize::MAX; size];
        for i in 0..size {
            let v = &verts[vi];
            if !bev[v.in_edge(i) as usize] {
                continue;
            }
            let j = (i + size - 1) % size;
            let a = v.chain[j].last().unwrap().point;
            let b = v.chain[i][0].point;
            let key = (vi, a.min(b), a.max(b));
            let idx = *profile_of.entry(key).or_insert_with(|| {
                profiles.push(Profile::new(key.1, key.2));
                profiles.len() - 1
            });
            // The profile leaves a tangent to the face of a and arrives tangent to the face of b.
            let na = mesh.face_normal(mesh.corner_face(v.corners[j]));
            let nb = mesh.face_normal(mesh.corner_face(v.corners[i]));
            let pr = &mut profiles[idx];
            if pr.planes.is_empty() {
                pr.axis = safe_normalize(mesh.position(mesh.edge_other_point(v.in_edge(i), v.point)) - mesh.position(v.point));
            }
            pr.planes.push(na.extend(na.dot(mesh.position(b))));
            pr.planes.push(nb.extend(nb.dot(mesh.position(a))));
            verts[vi].profile_at[i] = idx;
        }
    }
    let mut profile_vert = vec![0usize; profiles.len()];
    for (key, &idx) in &profile_of {
        profile_vert[idx] = key.0;
    }
    for k in 0..profiles.len() {
        let lo = mesh.position(profiles[k].lo);
        let hi = mesh.position(profiles[k].hi);
        let mid = (lo + hi) * 0.5;
        let chord = lo.distance(hi);
        let pr = &mut profiles[k];
        pr.o = nearest_on_planes(&pr.planes, mid);
        let away = pr.o.distance(mid);
        pr.linear = away <= 1e-9 * chord || away > 1e3 * chord;
        if pr.linear {
            pr.o = DVec3::ZERO;
            pr.u = lo;
            pr.w = hi;
        } else {
            pr.u = lo - pr.o;
            pr.w = hi - pr.o;
        }
        let src = verts[profile_vert[k]].point;
        for s in 1..segments {
            let np = mesh.add_point(profiles[k].at(s as f64 / segments as f64, r));
            mesh.point_attrs_mut().copy(np as usize, src as usize);
            profiles[k].ids.push(np);
        }
    }

    let mut ends: Vec<Vec<StripEnd>> = vec![Vec::new(); verts.len()];
    let mut end_of_profile: Vec<Option<(usize, usize)>> = vec![None; profiles.len()];
    for (vi, v) in verts.iter().enumerate() {
        ends[vi] = vec![StripEnd::default(); v.size()];
        for i in 0..v.size() {
            let e = v.in_edge(i);
            if !bev[e as usize] {
                continue;
            }
            let j = (i + v.size() - 1) % v.size();
            let a = v.chain[j].last().unwrap();
            let b = &v.chain[i][0];
            let mut se = StripEnd { a: a.point, b: b.point, ma: a.mix.clone(), mb: b.mix.clone(), label: wedge[vi][i] };
            if strip_seam[e as usize] {
                // The far side continues the owner's uvs, its face unfolded about the edge into the owner's plane.
                let own_a = fans[vi][j].face < fans[vi][i].face;
                let own = &fans[vi][if own_a { j } else { i }];
                let opp = &fans[vi][if own_a { i } else { j }];
                let p = if own_a { b.point } else { a.point };
                let q = own.tri.o + unfold(mesh.position(p) - own.tri.o, opp.n, own.n, profiles[v.profile_at[i]].axis);
                if own_a {
                    se.mb = own.tri.mix(q);
                } else {
                    se.ma = own.tri.mix(q);
                }
                se.label = wedge[vi][if own_a { j } else { i }];
            }
            ends[vi][i] = se;
            if end_of_profile[v.profile_at[i]].is_none() {
                end_of_profile[v.profile_at[i]] = Some((vi, i));
            }
        }
    }
    // Recipes of every chain point in each fan face it belongs to, for the vertex meshes.
    let mut chain_mix: Vec<HashMap<Index, Vec<(usize, Mix)>>> = vec![HashMap::new(); verts.len()];
    for (vi, v) in verts.iter().enumerate() {
        for i in 0..v.size() {
            for cp in &v.chain[i] {
                chain_mix[vi].entry(cp.point).or_default().push((i, cp.mix.clone()));
            }
        }
    }

    // Vertex meshes: walk the hole around each point against the fan so the fill winds like its neighbours.
    let mut vmesh: Vec<VertMesh> = vec![VertMesh::default(); verts.len()];
    let mut inserts: Vec<Insert> = Vec::new();
    // A terminal edge on a point that stays for its other edges, where a fan from the point would have no area (a
    // flat surface) or fold back over its neighbours (the profile bulging toward a convex corner): the point moves
    // onto the profile instead and each face beside it takes the profile up to the point.
    let mut welded: Vec<Index> = Vec::new();
    for vi in 0..verts.len() {
        let v = &verts[vi];
        let mut h = Hole::default();
        let n = v.size();
        let mut pending: Option<BevelSide> = None;
        for step in 0..n {
            let i = n - 1 - step;
            let ch = &v.chain[i];
            for k in (0..ch.len()).rev() {
                if k + 1 < ch.len() {
                    h.sides.push(BevelSide::straight(i, k));
                } else if let Some(p) = pending.take() {
                    h.sides.push(p);
                } else if !h.nodes.is_empty() {
                    // Shared with the previous face across an unbevelled edge.
                    continue;
                }
                h.nodes.push(ch[k].point);
            }
            if bev[v.in_edge(i) as usize] {
                pending = Some(BevelSide::arc(v.profile_at[i]));
            } else if i == 0 && !v.closed {
                pending = Some(BevelSide::open());
            }
        }
        match pending {
            Some(p) => h.sides.push(p),
            None => {
                h.nodes.pop();
            }
        }

        // Drop profiles walked out and straight back: in-line points, and strips meeting along their miter.
        let mut changed = true;
        while changed && !h.sides.is_empty() {
            changed = false;
            let kk = h.nodes.len();
            if kk == 2
                && h.sides[0].ty == SideType::Arc
                && h.sides[1].ty == SideType::Arc
                && h.sides[0].profile == h.sides[1].profile
            {
                h.nodes.clear();
                h.sides.clear();
                break;
            }
            if kk > 2 {
                for k in 0..kk {
                    let k1 = (k + 1) % kk;
                    let k2 = (k + 2) % kk;
                    if h.sides[k].ty != SideType::Arc
                        || h.sides[k1].ty != SideType::Arc
                        || h.sides[k].profile != h.sides[k1].profile
                        || h.nodes[k] != h.nodes[k2]
                    {
                        continue;
                    }
                    let mut nodes = Vec::new();
                    let mut sides = Vec::new();
                    for q in 0..kk {
                        if q == k1 || q == k2 {
                            continue;
                        }
                        nodes.push(h.nodes[q]);
                        sides.push(if q == k { h.sides[k2] } else { h.sides[q] });
                    }
                    h.nodes = nodes;
                    h.sides = sides;
                    changed = true;
                    break;
                }
            }
        }

        let kk = h.nodes.len();
        if kk < 2 {
            vmesh[vi].hole = h;
            continue;
        }
        let arcs = h.sides.iter().filter(|s| s.ty == SideType::Arc).count();
        let opens = h.sides.iter().filter(|s| s.ty == SideType::Open).count();
        if kk == 2 {
            // Two profiles between the same points are one and the same, so the walk dropped them already.
            if arcs == 1 && opens == 0 {
                // Terminal edge: the profile replaces the straight cut in the face beside it.
                let s = if h.sides[0].ty == SideType::Straight { 0 } else { 1 };
                let st = h.sides[s];
                let arc = h.sides[1 - s];
                let from = h.nodes[1 - s];
                let lo = &v.chain[st.face][st.at];
                let hi = &v.chain[st.face][st.at + 1];
                // Spaced like the strip's end, so the two agree wherever the face shares the strip's uvs.
                let pts: Vec<ChainPoint> = arc_ids(&profiles, arc.profile, from)
                    .into_iter()
                    .enumerate()
                    .map(|(k, p)| extra(p, Mix::lerp(&lo.mix, &hi.mix, (k + 1) as f64 / segments as f64), lo.src_edge))
                    .collect();
                inserts.push(Insert { vert: vi, face: st.face, at: st.at, pts });
                vmesh[vi].fill = Fill::Merge;
            }
            vmesh[vi].hole = h;
            continue;
        }
        let mut sorted = h.nodes.clone();
        sorted.sort_unstable();
        let fill = if sorted.windows(2).any(|w| w[0] == w[1]) {
            Fill::Split
        } else if zip_terminal(mesh, v, vi, &h, segments, r, &mut profiles, &mut welded, &mut inserts) {
            Fill::None
        } else if segments == 1 || arcs == 0 {
            Fill::Polygon
        } else if h.nodes.contains(&v.point) {
            Fill::Fan
        } else {
            Fill::Grid
        };
        if fill == Fill::Grid {
            // Every side gets the profile's segment count, so straight ones are split, in their face too.
            let mut side_ids = vec![Vec::new(); kk];
            for k in 0..kk {
                let s = h.sides[k];
                let a = h.nodes[k];
                let b = h.nodes[(k + 1) % kk];
                let ids = &mut side_ids[k];
                ids.push(a);
                if s.ty == SideType::Arc {
                    ids.extend(arc_ids(&profiles, s.profile, a));
                } else {
                    for j in 1..segments {
                        let np = mesh.add_point(mix3(mesh.position(a), mesh.position(b), j as f64 / segments as f64));
                        mesh.point_attrs_mut().copy(np as usize, v.point as usize);
                        ids.push(np);
                    }
                }
                ids.push(b);
                if s.ty == SideType::Straight {
                    let c = v.corners[s.face];
                    let src = v.chain[s.face][s.at].src_edge;
                    let pts: Vec<ChainPoint> = (1..ids.len() - 1)
                        .rev()
                        .map(|j| extra(ids[j], CornerTri::of(mesh, c).mix(mesh.position(ids[j])), src))
                        .collect();
                    inserts.push(Insert { vert: vi, face: s.face, at: s.at, pts });
                }
            }
            vmesh[vi].side_ids = side_ids;
        }
        vmesh[vi].fill = fill;
        vmesh[vi].hole = h;
    }
    // By vert and face, the later spots in a chain first, so earlier insert positions stay put.
    inserts.sort_by(|a, b| (a.vert, a.face, b.at).cmp(&(b.vert, b.face, a.at)));
    for ins in inserts {
        let ch = &mut verts[ins.vert].chain[ins.face];
        let at = ins.at + 1;
        ch.splice(at..at, ins.pts);
    }

    // Rebuild every face around a bevelled point.
    let mut corner_label: HashMap<Index, i32> = HashMap::new();
    let old_edges = edges_of_faces(mesh, &touched_faces);
    for &f in &touched_faces {
        let mut pts = Vec::new();
        let mut edge_src = Vec::new();
        let mut mixes: Vec<Mix> = Vec::new();
        let mut labels = Vec::new();
        for c in mesh.face_corners(f) {
            match fan_slot.get(&c) {
                None => {
                    pts.push(mesh.corner_point(c));
                    mixes.push(Mix::one(c));
                    edge_src.push(mesh.corner_edge(c));
                    labels.push(-1);
                }
                Some(&(vi, i)) => {
                    for cp in &verts[vi].chain[i] {
                        pts.push(cp.point);
                        mixes.push(cp.mix.clone());
                        edge_src.push(cp.src_edge);
                        labels.push(wedge[vi][i]);
                    }
                }
            }
        }
        mesh.remove_face(f);
        let nf = add_face_from(mesh, &pts, f, &[], &edge_src);
        if nf == INVALID {
            continue;
        }
        let mut c = mesh.face_corner(nf);
        for k in 0..pts.len() {
            mixes[k].apply(mesh, c);
            if labels[k] >= 0 {
                corner_label.insert(c, labels[k]);
            }
            c = mesh.corner_next(c);
        }
    }

    let mut out: Vec<Index> = Vec::new();

    // Strips.
    for &e in &sel {
        let mut side: [Vec<Index>; 2] = [Vec::new(), Vec::new()];
        let mut side_mix: [Vec<Mix>; 2] = [Vec::new(), Vec::new()];
        let mut through: [(usize, Index, Mix); 2] = [(usize::MAX, INVALID, Mix::default()), (usize::MAX, INVALID, Mix::default())];
        let mut src_face = INVALID;
        let mut label = [-1i32; 2];
        for s in 0..2 {
            let vi = vert_of[&mesh.edge_point(e, s)];
            let v = &verts[vi];
            let mut i = 0;
            while v.in_edge(i) != e {
                i += 1;
            }
            // The face where e goes out comes first.
            let j = (i + v.size() - 1) % v.size();
            let a = v.chain[j].last().unwrap();
            let b = &v.chain[i][0];
            side[s].push(a.point);
            side[s].extend(arc_ids(&profiles, v.profile_at[i], a.point));
            side[s].push(b.point);
            let se = &ends[vi][i];
            label[s] = se.label;
            for k in 0..=segments {
                side_mix[s].push(Mix::lerp(&se.ma, &se.mb, k as f64 / segments as f64));
            }
            let pr = &profiles[v.profile_at[i]];
            if pr.through != INVALID {
                let seg = if a.point == pr.lo { pr.through_seg } else { segments as usize - 1 - pr.through_seg };
                through[s] = (seg, pr.through, Mix::lerp(&se.ma, &se.mb, (seg as f64 + 0.5) / segments as f64));
            }
            if s == 0 {
                src_face = mesh.corner_face(v.corners[j]);
            }
        }
        let (pa, pb) = (&side[0], &side[1]);
        let k = segments as usize;
        for j in 0..k {
            let mut pts = vec![pb[k - j], pa[j]];
            let mut mix = vec![side_mix[1][k - j].clone(), side_mix[0][j].clone()];
            let mut labels = vec![label[1], label[0]];
            if through[0].0 == j {
                pts.push(through[0].1);
                mix.push(through[0].2.clone());
                labels.push(label[0]);
            }
            pts.extend([pa[j + 1], pb[k - j - 1]]);
            mix.extend([side_mix[0][j + 1].clone(), side_mix[1][k - j - 1].clone()]);
            labels.extend([label[0], label[1]]);
            if through[1].0 == k - j - 1 {
                pts.push(through[1].1);
                mix.push(through[1].2.clone());
                labels.push(label[1]);
            }
            emit(mesh, &mut out, &mut corner_label, &pts, src_face, &mix, &labels);
        }
    }

    // Vertex meshes.
    let g_center = {
        let num = (1.0f64 / 3.0).powf(1.0 / r) - 1.0 / 3.0;
        let den = (2.0 / 3.0) * (0.5f64.powf(1.0 / r) - 0.5);
        if den.abs() > 1e-9 {
            num / den
        } else {
            1.0
        }
    };
    for vi in 0..verts.len() {
        let v = &verts[vi];
        let vm = &vmesh[vi];
        let h = &vm.hole;
        let kk = h.nodes.len();
        let src_face = mesh.corner_face(v.corners[0]);
        if vm.fill == Fill::None || vm.fill == Fill::Merge || kk == 0 {
            continue;
        }

        // The patch takes the uvs of its wedge: from strips and fan faces in it, unfolded from the rest.
        let rf = fans[vi][lowest_face(vi)];
        let wv = wedge[vi][lowest_face(vi)];
        let strip_of = |sd: &BevelSide, a: Index, b: Index| -> Option<&StripEnd> {
            if sd.ty != SideType::Arc {
                return None;
            }
            let (evi, ei) = end_of_profile[sd.profile]?;
            let se = &ends[evi][ei];
            let has = |p: Index| se.a == p || se.b == p;
            (se.label == wv && has(a) && has(b)).then_some(se)
        };
        let mut node_val: Vec<Mix> = vec![Mix::default(); kk];
        for k in 0..kk {
            let p = h.nodes[k];
            let se = strip_of(&h.sides[(k + kk - 1) % kk], h.nodes[(k + kk - 1) % kk], p)
                .or_else(|| strip_of(&h.sides[k], p, h.nodes[(k + 1) % kk]));
            if let Some(se) = se {
                node_val[k] = se.at(p).clone();
                continue;
            }
            let list = chain_mix[vi].get(&p).filter(|l| !l.is_empty());
            if let Some(l) = list {
                if let Some(im) = l.iter().find(|im| wedge[vi][im.0] == wv) {
                    node_val[k] = im.1.clone();
                    continue;
                }
            }
            let n = list.map_or(rf.n, |l| fans[vi][l[0].0].n);
            node_val[k] = rf.tri.mix(rf.tri.o + unfold(mesh.position(p) - rf.tri.o, n, rf.n, DVec3::ZERO));
        }
        let side_mix_at = |k: usize, t: f64| {
            let a = h.nodes[k];
            let b = h.nodes[(k + 1) % kk];
            match strip_of(&h.sides[k], a, b) {
                Some(se) => Mix::lerp(se.at(a), se.at(b), t),
                None => Mix::lerp(&node_val[k], &node_val[(k + 1) % kk], t),
            }
        };
        let mut pm: HashMap<Index, Mix> = HashMap::new();
        for k in 0..kk {
            pm.entry(h.nodes[k]).or_insert_with(|| node_val[k].clone());
        }
        for k in 0..kk {
            if h.sides[k].ty == SideType::Arc {
                let mid = arc_ids(&profiles, h.sides[k].profile, h.nodes[k]);
                for (j, &p) in mid.iter().enumerate() {
                    if !pm.contains_key(&p) {
                        pm.insert(p, side_mix_at(k, (j + 1) as f64 / segments as f64));
                    }
                }
            }
        }
        let face = |mesh: &mut Mesh, out: &mut Vec<Index>, cl: &mut HashMap<Index, i32>, pm: &HashMap<Index, Mix>, pts: &[Index]| {
            let m: Vec<Mix> = pts.iter().map(|p| pm.get(p).cloned().unwrap_or_else(|| rf.tri.mix(mesh.position(*p)))).collect();
            let lab = vec![wv; pts.len()];
            emit(mesh, out, cl, pts, src_face, &m, &lab);
        };
        // The whole boundary, profile points included.
        let ring = || {
            let mut all = Vec::new();
            for k in 0..kk {
                all.push(h.nodes[k]);
                if h.sides[k].ty == SideType::Arc {
                    all.extend(arc_ids(&profiles, h.sides[k].profile, h.nodes[k]));
                }
            }
            all
        };

        match vm.fill {
            Fill::None | Fill::Merge => {}
            Fill::Polygon => face(mesh, &mut out, &mut corner_label, &pm, &ring()),
            Fill::Split => {
                let mut stack: Vec<Index> = Vec::new();
                for p in ring() {
                    if let Some(at) = stack.iter().position(|&x| x == p) {
                        let sub = stack[at..].to_vec();
                        stack.truncate(at + 1);
                        if sub.len() >= 3 {
                            face(mesh, &mut out, &mut corner_label, &pm, &sub);
                        }
                        continue;
                    }
                    stack.push(p);
                }
                if stack.len() >= 3 {
                    face(mesh, &mut out, &mut corner_label, &pm, &stack);
                }
            }
            Fill::Fan => {
                let mut all = ring();
                let at = all.iter().position(|&x| x == v.point).unwrap();
                all.rotate_left(at);
                // Quads around the kept point, from both ends, with a triangle in the middle when the count is odd.
                let mut lo = 1usize;
                let mut hi = all.len() - 1;
                let quads = (hi - lo) / 2;
                for _ in 0..quads.div_ceil(2) {
                    face(mesh, &mut out, &mut corner_label, &pm, &[v.point, all[lo], all[lo + 1], all[lo + 2]]);
                    lo += 2;
                }
                for _ in 0..quads / 2 {
                    face(mesh, &mut out, &mut corner_label, &pm, &[v.point, all[hi - 2], all[hi - 1], all[hi]]);
                    hi -= 2;
                }
                if hi - lo == 1 {
                    face(mesh, &mut out, &mut corner_label, &pm, &[v.point, all[lo], all[hi]]);
                }
            }
            Fill::Grid => {
                let n = segments;
                let n2 = 2 * n;
                let (nu, n2u) = (n as usize, n2 as usize);
                // Each side sampled at half steps: the grid's spokes run from side middles to the centre.
                let mut half: Vec<Vec<DVec3>> = vec![vec![DVec3::ZERO; n2u + 1]; kk];
                for k in 0..kk {
                    let a = h.nodes[k];
                    let b = h.nodes[(k + 1) % kk];
                    for j in 0..=n2u {
                        let s = j as f64 / n2 as f64;
                        half[k][j] = if h.sides[k].ty == SideType::Arc {
                            arc_at(&profiles, h.sides[k].profile, a, s, r)
                        } else {
                            mix3(mesh.position(a), mesh.position(b), s)
                        };
                    }
                    half[k][0] = mesh.position(a);
                    half[k][n2u] = mesh.position(b);
                }
                let centre = |hh: &[Vec<DVec3>]| {
                    let mut corners = DVec3::ZERO;
                    let mut bulge = DVec3::ZERO;
                    for s in hh {
                        corners += s[0];
                        bulge += s[nu] - (s[0] + s[n2u]) * 0.5;
                    }
                    (corners + g_center * bulge) / kk as f64
                };
                let coons = |hh: &[Vec<DVec3>], z: DVec3, i: usize, a: i32, b: i32| {
                    let sv = &hh[i];
                    let pv = &hh[(i + kk - 1) % kk];
                    let s = a as f64 / n as f64;
                    let t = b as f64 / n as f64;
                    let p0b = pv[(n2 - b) as usize];
                    let pnb = mix3(sv[nu], z, t);
                    let pa0 = sv[a as usize];
                    let pan = mix3(pv[nu], z, s);
                    (1.0 - s) * p0b + s * pnb + (1.0 - t) * pa0 + t * pan
                        - ((1.0 - s) * (1.0 - t) * sv[0] + s * (1.0 - t) * sv[nu] + (1.0 - s) * t * pv[nu] + s * t * z)
                };
                let z = centre(&half);
                // The same patch over the sides' recipes; they vary linearly, so the centre is the corners' average.
                let mut half_mix: Vec<Vec<Mix>> = vec![Vec::new(); kk];
                let mut z_mix = Mix::default();
                for k in 0..kk {
                    for j in 0..=n2 {
                        half_mix[k].push(side_mix_at(k, j as f64 / n2 as f64));
                    }
                    z_mix.add(&half_mix[k][0], 1.0 / kk as f64);
                    for j in 1..nu {
                        pm.entry(vm.side_ids[k][j]).or_insert_with(|| half_mix[k][2 * j].clone());
                    }
                }
                let coons_mix = |i: usize, a: i32, b: i32| {
                    let sv = &half_mix[i];
                    let pv = &half_mix[(i + kk - 1) % kk];
                    let s = a as f64 / n as f64;
                    let t = b as f64 / n as f64;
                    let mut m = Mix::default();
                    m.add(&pv[(n2 - b) as usize], 1.0 - s);
                    m.add(&sv[a as usize], 1.0 - t);
                    m.add(&sv[0], -(1.0 - s) * (1.0 - t));
                    m.add(&z_mix, s * t);
                    m
                };

                // A corner of three profiles sharing one centre is the affine image of a superellipsoid octant: snap
                // to it.
                let mut ideal = false;
                let mut c_centre = DVec3::ZERO;
                let mut axes = DMat3::IDENTITY;
                let mut inv = DMat3::IDENTITY;
                let mut ideal_half: Vec<Vec<DVec3>> = Vec::new();
                let mut ideal_z = DVec3::ZERO;
                if kk == 3 && h.sides.iter().all(|s| s.ty == SideType::Arc) {
                    for s in &h.sides {
                        c_centre += if profiles[s.profile].linear { DVec3::ZERO } else { profiles[s.profile].o };
                    }
                    c_centre /= 3.0;
                    let mut shortest = 1e300f64;
                    let mut cols = [DVec3::ZERO; 3];
                    for k in 0..3 {
                        cols[k] = mesh.position(h.nodes[k]) - c_centre;
                        shortest = shortest.min(cols[k].length());
                    }
                    axes = DMat3::from_cols(cols[0], cols[1], cols[2]);
                    let mut spread = 0.0f64;
                    let mut curved = true;
                    for s in &h.sides {
                        curved &= !profiles[s.profile].linear;
                        spread = spread.max(profiles[s.profile].o.distance(c_centre));
                    }
                    let det = axes.determinant();
                    ideal = curved
                        && spread <= 0.25 * shortest
                        && det.abs() > 1e-3 * cols[0].length() * cols[1].length() * cols[2].length();
                    if ideal {
                        inv = axes.inverse();
                        ideal_half = vec![vec![DVec3::ZERO; n2u + 1]; 3];
                        for k in 0..3 {
                            for j in 0..=n2u {
                                let xy = super_xy(j as f64 / n2 as f64, r);
                                ideal_half[k][j] = c_centre + cols[k] * xy.x + cols[(k + 1) % 3] * xy.y;
                            }
                        }
                        ideal_z = centre(&ideal_half);
                    }
                }
                let snap = |p: DVec3| {
                    let q = inv * (p - c_centre);
                    let top = q.x.abs().max(q.y.abs()).max(q.z.abs());
                    if top <= 0.0 {
                        return p;
                    }
                    let norm = top
                        * ((q.x.abs() / top).powf(r) + (q.y.abs() / top).powf(r) + (q.z.abs() / top).powf(r)).powf(1.0 / r);
                    c_centre + axes * (q / norm)
                };
                // Otherwise the strip surface nearest the whole boundary lends the grid its dome: exact for a
                // junction along one rounded edge, close for corners where nearly in-line edges meet.
                let mut fit: Option<Surface> = None;
                let mut fit_half: Vec<Vec<DVec3>> = Vec::new();
                let mut fit_z = DVec3::ZERO;
                if !ideal {
                    let mut size = 0.0f64;
                    for sd in &half {
                        size = size.max(sd[0].distance(*sd.last().unwrap()));
                    }
                    let mut best = 0.25 * size;
                    for sd in &h.sides {
                        if sd.ty != SideType::Arc || profiles[sd.profile].linear {
                            continue;
                        }
                        let pr = &profiles[sd.profile];
                        // The section square to the edge: the same surface, but points move straight onto it.
                        let u = pr.u - pr.axis * pr.u.dot(pr.axis);
                        let w = pr.w - pr.axis * pr.w.dot(pr.axis);
                        let frame = DMat3::from_cols(u, w, pr.axis);
                        if frame.determinant().abs() <= 1e-3 * u.length() * w.length() {
                            continue;
                        }
                        let surf = Surface { o: pr.o, u, w, d: pr.axis, inv: frame.inverse() };
                        // The boundary has to stay within the quarter turn the profile sweeps.
                        let mut err = 0.0f64;
                        for sd2 in &half {
                            for &p in sd2 {
                                let q = surf.coords(p);
                                err = err.max(if q.x < -0.02 || q.y < -0.02 { 1e300 } else { surf.snap(p, r).distance(p) });
                            }
                        }
                        if err < best {
                            best = err;
                            fit = Some(surf);
                        }
                    }
                    if let Some(f) = fit {
                        fit_half = half.clone();
                        for sd in fit_half.iter_mut() {
                            for p in sd.iter_mut() {
                                *p = f.snap(*p, r);
                            }
                        }
                        fit_z = centre(&fit_half);
                    }
                }
                let position = |i: usize, a: i32, b: i32| {
                    let mut p = coons(&half, z, i, a, b);
                    if ideal {
                        let q = coons(&ideal_half, ideal_z, i, a, b);
                        p += snap(q) - q;
                    } else if let Some(f) = fit {
                        let q = coons(&fit_half, fit_z, i, a, b);
                        p += f.snap(q, r) - q;
                    }
                    p
                };

                let mut made: BTreeMap<(usize, i32, i32), Index> = BTreeMap::new();
                let vpoint = v.point;
                let mut new_point = |mesh: &mut Mesh, pm: &mut HashMap<Index, Mix>, key: (usize, i32, i32), i: usize, a: i32, b: i32| {
                    if let Some(&p) = made.get(&key) {
                        return p;
                    }
                    let p = mesh.add_point(position(i, a, b));
                    mesh.point_attrs_mut().copy(p as usize, vpoint as usize);
                    made.insert(key, p);
                    pm.entry(p).or_insert_with(|| coons_mix(i, a, b));
                    p
                };
                // Point (a, b) in half steps of sector i: a runs along side i from its node, b back along side i - 1.
                let mut g = |mesh: &mut Mesh, pm: &mut HashMap<Index, Mix>, i: usize, a: i32, b: i32| -> Index {
                    let prev = (i + kk - 1) % kk;
                    if b == 0 {
                        return vm.side_ids[i][(a / 2) as usize];
                    }
                    if a == 0 {
                        return vm.side_ids[prev][(n - b / 2) as usize];
                    }
                    if a == n && b == n {
                        return new_point(mesh, pm, (usize::MAX, 0, 0), i, a, b);
                    }
                    if a == n {
                        return new_point(mesh, pm, (i, -1, b), i, a, b);
                    }
                    if b == n {
                        return new_point(mesh, pm, (prev, -1, a), i, a, b);
                    }
                    new_point(mesh, pm, (i, a, b), i, a, b)
                };
                let m = n / 2;
                for i in 0..kk {
                    for q in 0..m {
                        for p in 0..m {
                            let f = [
                                g(mesh, &mut pm, i, 2 * p, 2 * q),
                                g(mesh, &mut pm, i, 2 * p + 2, 2 * q),
                                g(mesh, &mut pm, i, 2 * p + 2, 2 * q + 2),
                                g(mesh, &mut pm, i, 2 * p, 2 * q + 2),
                            ];
                            face(mesh, &mut out, &mut corner_label, &pm, &f);
                        }
                    }
                    if n % 2 == 1 {
                        let next = (i + 1) % kk;
                        for q in 0..m {
                            let f = [
                                g(mesh, &mut pm, i, n - 1, 2 * q),
                                g(mesh, &mut pm, next, 2 * q, n - 1),
                                g(mesh, &mut pm, next, 2 * q + 2, n - 1),
                                g(mesh, &mut pm, i, n - 1, 2 * q + 2),
                            ];
                            face(mesh, &mut out, &mut corner_label, &pm, &f);
                        }
                    }
                }
                if n % 2 == 1 {
                    let middle: Vec<Index> = (0..kk).map(|i| g(mesh, &mut pm, i, n - 1, n - 1)).collect();
                    face(mesh, &mut out, &mut corner_label, &pm, &middle);
                }
            }
        }
    }

    out.retain(|&f| mesh.face_valid(f));

    let old_points: Vec<Index> = verts.iter().map(|v| v.point).collect();
    for p in welded {
        mesh.remove_point(p);
    }
    mesh.remove_loose(&old_edges, &old_points);
    mark_wedge_seams(mesh, &out, &corner_label);

    if opts.smoothing_angle >= 0.0 {
        let new_edges: Vec<Index> = edges_of_faces(mesh, &out).into_iter().filter(|&e| e >= edge_cap0).collect();
        set_hard_edges_by_angle(mesh, opts.smoothing_angle, &new_edges);
    }
    if collapses {
        weld_closed_edges(mesh, edge_cap0, WELD_SHARE * offset, &mut out);
    }
    apply_symmetry_snap(mesh, opts.symmetry, first_new_point);
    out
}

/// Bevels manifold edges. Returns the new bevel and corner faces.
pub fn bevel_edges(mesh: &mut Mesh, edges: &[Index], opts: &BevelOptions) -> Vec<Index> {
    bevel(mesh, edges, opts, Close::Try)
}
