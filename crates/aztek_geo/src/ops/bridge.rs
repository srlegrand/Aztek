//! Bridge: strips of faces joining pairs of border chains.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::f64::consts::PI;

use glam::{DMat3, DVec3};

use super::basic::merge_to_target;
use super::common::{add_face_from, mix_corners, remove_faces_clean};
use crate::attribute::{Index, INVALID};
use crate::mesh::Mesh;
use crate::mix::{mix3, mixd};
use crate::normals::set_hard_edges_by_angle;
use crate::topology::{chain_edges, EdgePath};

/// How each column of the bridge runs from the source border to the target border.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BridgeCurve {
    /// Straight lines between paired points.
    #[default]
    Linear,
    /// A cubic leaving each border square to it in the plane of its faces (the reference Blend, Blender Blend Surface).
    Blend,
    /// The section carried along BridgeOptions::path from one border's centre to the other's (the reference Curve).
    Path,
    /// Fits the faces behind each border: a straight line or circular arc when the edge flow on both sides lies on
    /// one, else a curve matching their slope and bend. A deleted strip of a cylinder, sphere or torus comes back
    /// exactly.
    Auto,
}

/// How several selected loops connect (Blender's Connect Loops).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BridgeConnect {
    /// Each loop with its nearest facing partner.
    #[default]
    Pairs,
    /// One run through every loop in order of distance, like a tube through rings.
    Chain,
    /// Chain, with the last loop joined back to the first.
    ChainClosed,
}

/// Falloff of the profile bulge across the bridge (Blender's Profile Shape).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BridgeProfile {
    #[default]
    Smooth,
    Sphere,
    Root,
    Sharp,
    Linear,
    InverseSquare,
    Constant,
}

/// How a bridge between borders of different edge counts makes up the difference.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BridgeTransition {
    /// 3-to-1 quad patterns spread over the rows; one triangle where the difference is odd.
    #[default]
    Quads,
    /// Triangle fans in the middle row.
    Triangles,
}

#[derive(Clone, Debug)]
pub struct BridgeOptions {
    /// Extra rows of faces between the borders; -1 picks them from the gap, the edges around it and the open ends.
    pub divisions: i32,
    pub curve: BridgeCurve,
    /// 0 straightens every curve type to Linear, 1 is the full curve.
    pub smoothness: f64,
    /// Handle length leaving the source and entering the target, as a multiple of the gap (Blend and Auto).
    pub tension_source: f64,
    pub tension_target: f64,
    /// Steps the target's pairing round a closed loop (the reference Bridge Offset, Blender Twist).
    pub offset: i32,
    /// Reads that side the other way round, crossing the bridge (the reference Direction).
    pub reverse_source: bool,
    pub reverse_target: bool,
    /// Turns the middle rows about the bridge's axis, growing from 0 at the source to this at the target, degrees.
    pub twist: f64,
    /// Scale of the middle rows about their centre: 1 leaves them, 0.5 pinches to half.
    pub taper: f64,
    /// Where across the bridge the taper is strongest, 0 at the source, 1 at the target.
    pub bias: f64,
    /// Optional ramp of taper strength across the bridge, values evenly spaced from source to target.
    pub taper_curve: Vec<f64>,
    /// Bulge (positive) or pinch (negative) of the middle rows, as a fraction of the gap (Blender Profile Factor).
    pub profile_factor: f64,
    pub profile_shape: BridgeProfile,
    /// The curve Path follows, in the mesh's space; its ends map onto the two borders' centres.
    pub path: Vec<DVec3>,
    pub transition: BridgeTransition,
    /// Open bridges: rows meeting the border that joins the two sides' ends land on its points and weld; rows with
    /// no point split that border. Off leaves the end rows wherever the curve puts them.
    pub weld_ends: bool,
    /// How far a row may be from a border point and still weld to it, as a fraction of the row spacing.
    pub weld_tolerance: f64,
    /// Smoothing passes over the new inner points.
    pub relax: i32,
    /// New edges sharper than this are hard; negative leaves edge hardness alone.
    pub smoothing_angle: f64,
    /// Collapses the two borders into one at merge_factor instead of bridging (0 onto the source, 1 onto the target).
    pub merge: bool,
    pub merge_factor: f64,
    pub connect: BridgeConnect,
    /// Points that must pair: the source's and the target's start, then any further (source, target) pairs.
    pub pairs: Vec<(Index, Index)>,
}

impl Default for BridgeOptions {
    fn default() -> Self {
        Self {
            divisions: 0,
            curve: BridgeCurve::Linear,
            smoothness: 1.0,
            tension_source: 1.0,
            tension_target: 1.0,
            offset: 0,
            reverse_source: false,
            reverse_target: false,
            twist: 0.0,
            taper: 1.0,
            bias: 0.5,
            taper_curve: Vec::new(),
            profile_factor: 0.0,
            profile_shape: BridgeProfile::Smooth,
            path: Vec::new(),
            transition: BridgeTransition::Quads,
            weld_ends: true,
            weld_tolerance: 0.35,
            relax: 0,
            smoothing_angle: -1.0,
            merge: false,
            merge_factor: 0.5,
            connect: BridgeConnect::Pairs,
            pairs: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct BridgeResult {
    pub faces: Vec<Index>,
    /// Rows of faces the bridges ended up with (the most of any pair).
    pub rows: i32,
    /// Bridge rows that landed on existing border points at the open ends.
    pub welded: i32,
    /// Rows that split an end border because no point was near.
    pub split: i32,
    /// End border points no row reached; they stay as extra corners of the end faces.
    pub unwelded: i32,
    pub triangles: i32,
    pub pairs_bridged: i32,
    pub warnings: Vec<String>,
}

type D3 = DVec3;

const EPS: f64 = 1e-12;
const DENSE: usize = 64;

fn unit(v: D3) -> D3 {
    let l = v.length();
    if l > EPS {
        v / l
    } else {
        D3::ZERO
    }
}

fn smoothstep(x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    x * x * (3.0 - 2.0 * x)
}

fn skew(k: D3) -> DMat3 {
    DMat3::from_cols(D3::new(0.0, k.z, -k.y), D3::new(-k.z, 0.0, k.x), D3::new(k.y, -k.x, 0.0))
}

fn outer(a: D3, b: D3) -> DMat3 {
    DMat3::from_cols(a * b.x, a * b.y, a * b.z)
}

/// Rotation taking unit a onto unit b (Rodrigues).
fn rotation_between(a: D3, b: D3) -> DMat3 {
    let v = a.cross(b);
    let c = a.dot(b);
    let s = v.length();
    if s < 1e-12 {
        if c > 0.0 {
            return DMat3::IDENTITY;
        }
        let axis = unit(a.cross(if a.x.abs() < 0.9 { D3::X } else { D3::Y }));
        return outer(axis, axis) * 2.0 - DMat3::IDENTITY;
    }
    let k = skew(v / s);
    DMat3::IDENTITY + k * s + (k * k) * (1.0 - c)
}

fn rotation_about(axis: D3, radians: f64) -> DMat3 {
    let k = skew(unit(axis));
    DMat3::IDENTITY + k * radians.sin() + (k * k) * (1.0 - radians.cos())
}

/// Normalised arc length at each point; a closed run's total includes the closing edge.
fn arc_params(p: &[D3], closed: bool) -> Vec<f64> {
    let mut s = vec![0.0; p.len()];
    let mut acc = 0.0;
    for i in 1..p.len() {
        acc += p[i - 1].distance(p[i]);
        s[i] = acc;
    }
    let total = acc + if closed && p.len() > 1 { p[p.len() - 1].distance(p[0]) } else { 0.0 };
    for v in s.iter_mut() {
        *v = if total > EPS { *v / total } else { 0.0 };
    }
    s
}

/// A polyline with its cumulative length, sampled by fraction of that length.
#[derive(Clone, Debug, Default)]
struct Dense {
    p: Vec<D3>,
    s: Vec<f64>,
}

impl Dense {
    fn finish(&mut self) {
        self.s = vec![0.0; self.p.len()];
        for i in 1..self.p.len() {
            self.s[i] = self.s[i - 1] + self.p[i - 1].distance(self.p[i]);
        }
    }
    fn length(&self) -> f64 {
        self.s.last().copied().unwrap_or(0.0)
    }
    fn at(&self, f: f64) -> D3 {
        if self.p.len() < 2 || self.length() < EPS {
            return self.p.first().copied().unwrap_or(D3::ZERO);
        }
        let x = f.clamp(0.0, 1.0) * self.length();
        let k = self.s.partition_point(|&v| v <= x).clamp(1, self.p.len() - 1);
        let seg = self.s[k] - self.s[k - 1];
        mix3(self.p[k - 1], self.p[k], if seg > EPS { (x - self.s[k - 1]) / seg } else { 0.0 })
    }
}

// Pairing borders

fn path_centre(mesh: &Mesh, p: &EdgePath) -> D3 {
    let sum: D3 = p.points.iter().map(|&pt| mesh.position(pt)).sum();
    if p.points.is_empty() {
        sum
    } else {
        sum / p.points.len() as f64
    }
}

/// Which way the border opens: away from the faces along it. Zero for wire edges.
fn open_side(mesh: &Mesh, p: &EdgePath) -> D3 {
    let mut sum = D3::ZERO;
    for &e in &p.edges {
        if mesh.edge_corner(e) == INVALID {
            continue;
        }
        let mid = (mesh.position(mesh.edge_point(e, 0)) + mesh.position(mesh.edge_point(e, 1))) * 0.5;
        let d = mid - mesh.face_centroid(mesh.corner_face(mesh.edge_corner(e)));
        let len = d.length();
        if len > 1e-12 {
            sum += d / len;
        }
    }
    sum
}

/// A hole's border (or a wire loop) opens every way, so any direction suits it.
fn opens_toward(open: D3, p: &EdgePath, d: D3) -> bool {
    open.length() < 0.25 * p.edges.len() as f64 || open.dot(d) > 0.0
}

/// A closed loop faces a partner lying off its plane (a gap in a tube), not one beside it in the plane (two holes in
/// a sheet).
fn loops_face(mesh: &Mesh, lp: &EdgePath, centre: D3, other: D3) -> bool {
    let mut normal = D3::ZERO;
    let n = lp.points.len();
    for i in 0..n {
        normal += (mesh.position(lp.points[i]) - centre).cross(mesh.position(lp.points[(i + 1) % n]) - centre);
    }
    let mut radius: f64 = lp.points.iter().map(|&p| mesh.position(p).distance(centre)).sum();
    radius /= n.max(1) as f64;
    // Concentric loops (an annulus) face each other whatever their planes.
    let d = other - centre;
    if normal.length() < 1e-12 || d.length() < 0.25 * radius {
        return true;
    }
    normal.normalize().dot(d.normalize()).abs() > 0.5
}

/// The path cut at its corners into open runs; a closed path with fewer than two corners stays whole.
fn split_at_corners(mesh: &Mesh, p: &EdgePath) -> Vec<EdgePath> {
    // Sharper than an octagon's turn, so rounded borders stay whole.
    let cos_corner = 50f64.to_radians().cos();
    let np = p.points.len();
    let ne = p.edges.len();
    let dir = |e: usize| {
        let d = mesh.position(p.points[(e + 1) % np]) - mesh.position(p.points[e]);
        let len = d.length();
        if len > 1e-12 {
            d / len
        } else {
            d
        }
    };
    let mut corners = Vec::new();
    let range = if p.closed { 0..np } else { 1..np - 1 };
    for i in range {
        if dir((i + ne - 1) % ne).dot(dir(i)) < cos_corner {
            corners.push(i);
        }
    }
    if p.closed && corners.len() < 2 {
        return vec![p.clone()];
    }
    if !p.closed {
        corners.insert(0, 0);
        corners.push(np - 1);
    }
    let count = if p.closed { corners.len() } else { corners.len() - 1 };
    let mut runs = Vec::new();
    for k in 0..count {
        let a = corners[k];
        let b = if p.closed && k + 1 == corners.len() { corners[0] + ne } else { corners[k + 1] };
        let mut run = EdgePath::default();
        for i in a..b {
            run.points.push(p.points[i % np]);
            run.edges.push(p.edges[i % ne]);
        }
        run.points.push(p.points[b % np]);
        runs.push(run);
    }
    runs
}

type PathPair = (EdgePath, EdgePath);

/// The two runs of one border to bridge: with four or more runs, the longest opposite pair, matching lengths first
/// and nearest after; with two or three (a sliver between two curves, a triangle), the two longest, sharing a corner.
fn sides_to_bridge(mesh: &Mesh, p: &EdgePath) -> Option<PathPair> {
    let runs = split_at_corners(mesh, p);
    let n = runs.len();
    if n < 2 || (n == 2 && !p.closed) {
        return None;
    }
    let adjacent = |i: usize, j: usize| j == i + 1 || (p.closed && i == 0 && j == n - 1);
    let mut best: Option<((usize, i32, f64), usize, usize)> = None;
    for i in 0..n {
        for j in i + 1..n {
            if n >= 4 && adjacent(i, j) {
                continue;
            }
            let li = runs[i].edges.len();
            let lj = runs[j].edges.len();
            let d = path_centre(mesh, &runs[i]).distance(path_centre(mesh, &runs[j]));
            // Longer shorter side wins, then equal lengths, then nearer.
            let key = (li.min(lj), (li == lj) as i32, -d);
            let better = match &best {
                None => true,
                Some((bk, _, _)) => key.partial_cmp(bk) == Some(std::cmp::Ordering::Greater),
            };
            if better {
                best = Some((key, i, j));
            }
        }
    }
    best.map(|(_, i, j)| (runs[i].clone(), runs[j].clone()))
}

fn bridgeable(m: &Mesh, e: Index) -> bool {
    e < m.edge_capacity() && m.edge_valid(e) && m.edge_face_count(e) <= 1
}

fn plan_pairs(mesh: &Mesh, edges: &[Index], connect: BridgeConnect) -> Vec<PathPair> {
    let mut border: Vec<Index> = edges.iter().copied().filter(|&e| bridgeable(mesh, e)).collect();
    border.sort_unstable();
    border.dedup();
    let mut paths = chain_edges(mesh, &border);
    paths.retain(|p| !p.edges.is_empty());
    let n = paths.len();
    let centres: Vec<D3> = paths.iter().map(|p| path_centre(mesh, p)).collect();
    let open: Vec<D3> = paths.iter().map(|p| open_side(mesh, p)).collect();
    let mut out = Vec::new();

    if connect != BridgeConnect::Pairs && n >= 2 {
        // Start at one end of the farthest pair and step to the nearest loop left.
        let mut start = 0;
        let mut far = -1.0;
        for i in 0..n {
            for j in 0..n {
                let d = centres[i].distance(centres[j]);
                if d > far {
                    far = d;
                    start = i;
                }
            }
        }
        let mut order = vec![start];
        let mut used = vec![false; n];
        used[start] = true;
        while order.len() < n {
            let mut best = 0;
            let mut bd = f64::MAX;
            for j in 0..n {
                if !used[j] {
                    let d = centres[*order.last().unwrap()].distance(centres[j]);
                    if d < bd {
                        bd = d;
                        best = j;
                    }
                }
            }
            used[best] = true;
            order.push(best);
        }
        for k in 0..n - 1 {
            out.push((paths[order[k]].clone(), paths[order[k + 1]].clone()));
        }
        if connect == BridgeConnect::ChainClosed && n >= 3 {
            out.push((paths[*order.last().unwrap()].clone(), paths[order[0]].clone()));
        }
        return out;
    }

    // Borders that open toward each other pair first, so a tube with two gaps bridges the gaps, not the band between
    // them.
    let mut candidates: Vec<(bool, f64, usize, usize)> = Vec::new();
    for i in 0..n {
        for j in i + 1..n {
            if paths[i].closed != paths[j].closed {
                continue;
            }
            if paths[i].closed && !loops_face(mesh, &paths[i], centres[i], centres[j]) {
                continue;
            }
            let facing = opens_toward(open[i], &paths[i], centres[j] - centres[i])
                && opens_toward(open[j], &paths[j], centres[i] - centres[j]);
            let same = paths[i].edges.len() == paths[j].edges.len();
            candidates.push((!facing, centres[i].distance(centres[j]) * if same { 1.0 } else { 1.5 }, i, j));
        }
    }
    candidates.sort_by(|x, y| (x.0, x.1).partial_cmp(&(y.0, y.1)).unwrap_or(std::cmp::Ordering::Equal));

    let mut paired = vec![false; n];
    for &(_, _, a, b) in &candidates {
        if !paired[a] && !paired[b] {
            paired[a] = true;
            paired[b] = true;
            out.push((paths[a].clone(), paths[b].clone()));
        }
    }
    for i in 0..n {
        if !paired[i] {
            if let Some(sides) = sides_to_bridge(mesh, &paths[i]) {
                out.push(sides);
            }
        }
    }
    out
}

// One bridge

/// A border chain in the order the strip runs along it.
#[derive(Clone, Debug, Default)]
struct Side {
    pts: Vec<Index>,
    /// edges[i] joins pts[i] and pts[i + 1]; a closed side's last joins back to pts[0].
    edges: Vec<Index>,
    closed: bool,
    wire: bool,
}

fn reverse_side(s: &mut Side) {
    if s.closed {
        s.pts[1..].reverse();
    } else {
        s.pts.reverse();
    }
    s.edges.reverse();
}

fn rotate_side(s: &mut Side, start: usize) {
    if !s.closed || start == 0 {
        return;
    }
    s.pts.rotate_left(start);
    s.edges.rotate_left(start);
}

/// The side read the way a face filling the gap runs along it: against the existing face.
fn side_from(m: &Mesh, p: &EdgePath) -> Side {
    let mut s = Side { pts: p.points.clone(), edges: p.edges.clone(), closed: p.closed, wire: true };
    if s.edges.iter().any(|&e| m.edge_corner(e) != INVALID) {
        s.wire = false;
    }
    let c = m.edge_corner(s.edges[0]);
    if c != INVALID && m.corner_point(c) == s.pts[0] {
        reverse_side(&mut s);
    }
    s
}

/// The existing face's corner at side point i, INVALID on wire.
fn side_corner(m: &Mesh, s: &Side, i: usize) -> Index {
    let ne = s.edges.len();
    for ei in [if i < ne { i } else { ne - 1 }, if i > 0 { i - 1 } else if s.closed { ne - 1 } else { 0 }] {
        let e = s.edges[ei];
        let c = m.edge_corner(e);
        if c != INVALID {
            return if m.corner_point(c) == s.pts[i] {
                c
            } else if m.corner_next_point(c) == s.pts[i] {
                m.corner_next(c)
            } else {
                INVALID
            };
        }
    }
    INVALID
}

/// What the strip needs at each side point: where it leaves the mesh and the edge flow behind it.
#[derive(Clone, Debug)]
struct SidePoint {
    pos: D3,
    /// Unit, square to the side, in the faces' tangent plane, away from them.
    out: D3,
    /// Up to two points across the faces, nearest first.
    behind: Vec<D3>,
    corner: Index,
}

/// Per side point, with other the far side, whose points are never read as edge flow behind this one.
fn side_points(m: &Mesh, s: &Side, other: &Side, other_centre: D3) -> Vec<SidePoint> {
    let far: HashSet<Index> = other.pts.iter().copied().collect();
    let n = s.pts.len();
    let own: HashSet<Index> = s.edges.iter().copied().collect();
    let mut out = Vec::with_capacity(n);
    for k in 0..n {
        let pos = m.position(s.pts[k]);
        let corner = side_corner(m, s, k);
        let prev = if k > 0 {
            m.position(s.pts[k - 1])
        } else if s.closed {
            m.position(s.pts[n - 1])
        } else {
            pos
        };
        let next = if k + 1 < n {
            m.position(s.pts[k + 1])
        } else if s.closed {
            m.position(s.pts[0])
        } else {
            pos
        };
        let along = unit(next - prev);
        let mut nrm = D3::ZERO;
        let mut away = D3::ZERO;
        let mut visit = |ei: usize| {
            let c = m.edge_corner(s.edges[ei]);
            if c != INVALID {
                let f = m.corner_face(c);
                nrm += m.face_normal(f);
                away += pos - m.face_centroid(f);
            }
        };
        if k > 0 || s.closed {
            visit(if k > 0 { k - 1 } else { s.edges.len() - 1 });
        }
        if k + 1 < n || s.closed {
            visit(if k < s.edges.len() { k } else { s.edges.len() - 1 });
        }
        let want = if away.length() > EPS { away } else { other_centre - pos };
        let mut d = unit(along.cross(nrm));
        if d.length() < 0.5 {
            d = unit(want - along * want.dot(along));
        } else if d.dot(want) < 0.0 {
            d = -d;
        }
        let mut sp = SidePoint { pos, out: d, behind: Vec::new(), corner };

        // Across the faces: the inner edge leaving most directly away from the gap, then straight on.
        if !s.wire {
            let p0 = s.pts[k];
            let mut best = INVALID;
            let mut best_dot = -0.2;
            m.for_each_point_edge(p0, |e| {
                if own.contains(&e) || m.edge_face_count(e) < 2 {
                    return;
                }
                let dd = unit(m.position(m.edge_other_point(e, p0)) - sp.pos).dot(sp.out);
                if dd < best_dot {
                    best_dot = dd;
                    best = e;
                }
            });
            if best != INVALID && !far.contains(&m.edge_other_point(best, p0)) {
                let cur = m.edge_other_point(best, p0);
                sp.behind.push(m.position(cur));
                let mut straight = INVALID;
                let mut sd = 0.5;
                let heading = unit(m.position(cur) - m.position(p0));
                m.for_each_point_edge(cur, |e| {
                    if e == best {
                        return;
                    }
                    let dd = unit(m.position(m.edge_other_point(e, cur)) - m.position(cur)).dot(heading);
                    if dd > sd {
                        sd = dd;
                        straight = e;
                    }
                });
                if straight != INVALID && !far.contains(&m.edge_other_point(straight, cur)) {
                    sp.behind.push(m.position(m.edge_other_point(straight, cur)));
                }
            }
        }
        out.push(sp);
    }
    out
}

/// Where a column meets one side: on point a, or t of the way to point b (indices into the side).
#[derive(Clone, Copy, Debug, Default)]
struct Station {
    a: usize,
    b: usize,
    t: f64,
}

impl Station {
    fn exact(&self) -> bool {
        self.a == self.b || self.t <= 1e-12
    }
}

#[derive(Clone, Debug, Default)]
struct StationData {
    pos: D3,
    out: D3,
    behind: Vec<D3>,
}

fn at_station(sp: &[SidePoint], st: &Station) -> StationData {
    if st.exact() {
        let x = &sp[st.a];
        return StationData { pos: x.pos, out: x.out, behind: x.behind.clone() };
    }
    let x = &sp[st.a];
    let y = &sp[st.b];
    let behind = (0..x.behind.len().min(y.behind.len())).map(|j| mix3(x.behind[j], y.behind[j], st.t)).collect();
    StationData { pos: mix3(x.pos, y.pos, st.t), out: unit(mix3(x.out, y.out, st.t)), behind }
}

/// Columns of the big side for each small point, in order, by arc length; forced[i] pins small point i to a column.
fn assign_columns(big: &[D3], small: &[D3], closed: bool, forced: &HashMap<usize, usize>, cost: &mut f64) -> Vec<usize> {
    let nb = big.len();
    let n = small.len();
    let pb = arc_params(big, closed);
    let ps = arc_params(small, closed);
    let mut col = vec![0usize; n];
    for i in 1..n {
        let lo = col[i - 1] + 1;
        let hi = nb - (n - i);
        let mut best = lo;
        if !closed && i + 1 == n {
            best = nb - 1;
        } else if let Some(&f) = forced.get(&i) {
            best = f.clamp(lo, hi);
        } else {
            let mut bd = f64::MAX;
            for c in lo..=hi {
                let d = (pb[c] - ps[i]).abs();
                if d < bd {
                    bd = d;
                    best = c;
                }
            }
        }
        col[i] = best;
    }
    *cost = (0..n).map(|i| (small[i] - big[col[i]]).length_squared()).sum();
    col
}

/// Moves small points one column so the extra big-side columns come in even runs between them: two extra columns end
/// in a 3-to-1 of quads, one alone needs a triangle. Odd runs pair with the next odd run; pinned points stay.
fn even_up(col: &mut [usize], nb: usize, closed: bool, forced: &HashMap<usize, usize>) {
    let n = col.len();
    let segs = if closed { n } else { n - 1 };
    let extra = |col: &[usize], k: usize| (if k + 1 < n { col[k + 1] } else { nb }) - col[k] - 1;
    let odd: Vec<usize> = (0..segs).filter(|&k| extra(col, k) % 2 == 1).collect();
    let mut i = 0;
    while i + 1 < odd.len() {
        let (k1, k2) = (odd[i], odd[i + 1]);
        let pinned = (k1 + 1..=k2).any(|p| forced.contains_key(&p));
        if !(pinned || (!closed && k2 + 1 >= n)) {
            for c in &mut col[k1 + 1..=k2] {
                *c += 1;
            }
        }
        i += 2;
    }
}

/// Circle through points (all in one plane).
#[derive(Clone, Copy, Debug)]
struct Circle {
    centre: D3,
    u: D3,
    w: D3,
    radius: f64,
}

impl Circle {
    fn angle(&self, p: D3) -> f64 {
        (p - self.centre).dot(self.w).atan2((p - self.centre).dot(self.u))
    }
    fn at(&self, a: f64) -> D3 {
        self.centre + self.radius * (a.cos() * self.u + a.sin() * self.w)
    }
}

fn fit_circle(pts: &[D3], tol: f64) -> Option<Circle> {
    if pts.len() < 3 {
        return None;
    }
    let c: D3 = pts.iter().copied().sum::<D3>() / pts.len() as f64;
    let mut n = D3::ZERO;
    for i in 0..pts.len() {
        n += (pts[i] - c).cross(pts[(i + 1) % pts.len()] - c);
    }
    if n.length() < EPS {
        return None;
    }
    let n = n.normalize();
    if pts.iter().any(|p| (*p - c).dot(n).abs() > tol) {
        return None;
    }
    let u = unit(pts[0] - c - n * (pts[0] - c).dot(n));
    let w = n.cross(u);
    // Kasa fit: x^2 + y^2 + D x + E y + F = 0 in least squares.
    let mut ata = DMat3::ZERO;
    let mut atb = D3::ZERO;
    for p in pts {
        let x = (*p - c).dot(u);
        let y = (*p - c).dot(w);
        let row = D3::new(x, y, 1.0);
        ata += outer(row, row);
        atb += row * -(x * x + y * y);
    }
    if ata.determinant().abs() < 1e-30 {
        return None;
    }
    let sol = ata.inverse() * atb;
    let cx = -sol.x / 2.0;
    let cy = -sol.y / 2.0;
    let r2 = cx * cx + cy * cy - sol.z;
    if r2 <= 0.0 {
        return None;
    }
    let out = Circle { centre: c + cx * u + cy * w, u, w, radius: r2.sqrt() };
    if pts.iter().any(|p| (p.distance(out.centre) - out.radius).abs() > tol) {
        return None;
    }
    Some(out)
}

fn wrap_pi(mut a: f64) -> f64 {
    while a > PI {
        a -= 2.0 * PI;
    }
    while a < -PI {
        a += 2.0 * PI;
    }
    a
}

/// Unit direction and curvature vector (per unit length) at p0 from the points behind it, nearest first.
fn slope_and_bend(p0: D3, behind: &[D3], fallback: D3) -> (D3, D3) {
    if behind.is_empty() {
        return (fallback, D3::ZERO);
    }
    if behind.len() == 1 {
        return (unit(p0 - behind[0]), D3::ZERO);
    }
    let h1 = p0.distance(behind[0]);
    let h2 = behind[0].distance(behind[1]);
    let (s0, s1, s2) = (-(h1 + h2), -h1, 0.0);
    if h1 < EPS || h2 < EPS {
        return (unit(p0 - behind[0]), D3::ZERO);
    }
    let (y0, y1, y2) = (behind[1], behind[0], p0);
    let d1 = y0 * ((s2 - s1) / ((s0 - s1) * (s0 - s2)))
        + y1 * ((s2 - s0) / ((s1 - s0) * (s1 - s2)))
        + y2 * (((s2 - s0) + (s2 - s1)) / ((s2 - s0) * (s2 - s1)));
    let d2 = 2.0 * (y0 / ((s0 - s1) * (s0 - s2)) + y1 / ((s1 - s0) * (s1 - s2)) + y2 / ((s2 - s0) * (s2 - s1)));
    let l = d1.length();
    if l < EPS {
        return (unit(p0 - behind[0]), D3::ZERO);
    }
    let dir = d1 / l;
    (dir, (d2 - dir * d2.dot(dir)) / (l * l))
}

#[allow(clippy::too_many_arguments)]
fn quintic(p0: D3, v0: D3, a0: D3, p1: D3, v1: D3, a1: D3, t: f64) -> D3 {
    let (t2, t3) = (t * t, t * t * t);
    let (t4, t5) = (t3 * t, t3 * t * t);
    let h0 = 1.0 - 10.0 * t3 + 15.0 * t4 - 6.0 * t5;
    let h1 = t - 6.0 * t3 + 8.0 * t4 - 3.0 * t5;
    let h2 = 0.5 * t2 - 1.5 * t3 + 1.5 * t4 - 0.5 * t5;
    let h3 = 0.5 * t3 - t4 + 0.5 * t5;
    let h4 = -4.0 * t3 + 7.0 * t4 - 3.0 * t5;
    let h5 = 10.0 * t3 - 15.0 * t4 + 6.0 * t5;
    h0 * p0 + h1 * v0 + h2 * a0 + h3 * a1 + h4 * v1 + h5 * p1
}

fn polyline_length(p: &[D3]) -> f64 {
    p.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// The column's curve from a (source) to b (target) as a dense polyline, before smoothness.
fn column_curve(a: &StationData, b: &StationData, o: &BridgeOptions, both_wire: bool) -> Vec<D3> {
    let l = a.pos.distance(b.pos);
    let blend = || {
        let v0 = a.out * (l * o.tension_source);
        let v1 = -b.out * (l * o.tension_target);
        (0..=DENSE)
            .map(|i| {
                let t = i as f64 / DENSE as f64;
                let (t2, t3) = (t * t, t * t * t);
                (2.0 * t3 - 3.0 * t2 + 1.0) * a.pos + (t3 - 2.0 * t2 + t) * v0 + (-2.0 * t3 + 3.0 * t2) * b.pos + (t3 - t2) * v1
            })
            .collect::<Vec<D3>>()
    };
    if l < EPS || o.curve == BridgeCurve::Linear || both_wire {
        return vec![a.pos, b.pos];
    }
    if o.curve == BridgeCurve::Blend || (a.behind.is_empty() && b.behind.is_empty()) {
        return blend();
    }

    // Auto: the edge flow either side, nearest-first behind each end.
    let mut spacing = 0.0f64;
    for s in [a, b] {
        if let Some(&first) = s.behind.first() {
            spacing = spacing.max(s.pos.distance(first));
        }
    }
    let tol = 1e-3 * l.max(spacing);
    let mut all: Vec<D3> = a.behind.iter().rev().copied().collect();
    all.push(a.pos);
    all.push(b.pos);
    all.extend(b.behind.iter().copied());

    let ab = (b.pos - a.pos) / l;
    let mut line = all.iter().all(|p| ((*p - a.pos) - ab * (*p - a.pos).dot(ab)).length() <= tol);
    for p in &a.behind {
        line = line && (*p - a.pos).dot(ab) < 0.0;
    }
    for p in &b.behind {
        line = line && (*p - b.pos).dot(ab) > 0.0;
    }
    if line {
        return vec![a.pos, b.pos];
    }

    if all.len() >= 4 {
        if let Some(c) = fit_circle(&all, tol) {
            let ta = c.angle(a.pos);
            let tb = c.angle(b.pos);
            // Sweep away from the points behind a (or toward those behind b).
            let dir_sign = if !a.behind.is_empty() {
                -(1.0f64.copysign(wrap_pi(c.angle(a.behind[0]) - ta)))
            } else {
                1.0f64.copysign(wrap_pi(c.angle(b.behind[0]) - tb))
            };
            let mut sweep = dir_sign * (tb - ta);
            while sweep <= 0.0 {
                sweep += 2.0 * PI;
            }
            while sweep > 2.0 * PI {
                sweep -= 2.0 * PI;
            }
            // A gap wider than the rest of the circle means the fit is a coincidence, not the shape.
            if sweep < 1.5 * PI {
                return (0..=DENSE).map(|i| c.at(ta + dir_sign * sweep * i as f64 / DENSE as f64)).collect();
            }
        }
    }

    let (da, ka) = slope_and_bend(a.pos, &a.behind, a.out);
    let (db, kb) = slope_and_bend(b.pos, &b.behind, b.out);
    let mut t_len = l;
    let mut out = Vec::new();
    for _ in 0..3 {
        out = (0..=DENSE)
            .map(|i| {
                quintic(
                    a.pos,
                    da * (t_len * o.tension_source),
                    ka * (t_len * t_len),
                    b.pos,
                    -db * (t_len * o.tension_target),
                    kb * (t_len * t_len),
                    i as f64 / DENSE as f64,
                )
            })
            .collect();
        t_len = polyline_length(&out);
    }
    out
}

fn profile_falloff(shape: BridgeProfile, x: f64) -> f64 {
    let x = x.clamp(0.0, 1.0);
    match shape {
        BridgeProfile::Smooth => x * x * (3.0 - 2.0 * x),
        BridgeProfile::Sphere => (2.0 * x - x * x).max(0.0).sqrt(),
        BridgeProfile::Root => x.sqrt(),
        BridgeProfile::Sharp => x * x,
        BridgeProfile::Linear => x,
        BridgeProfile::InverseSquare => x * (2.0 - x),
        BridgeProfile::Constant => {
            if x > 0.0 {
                1.0
            } else {
                0.0
            }
        }
    }
}

fn sample_ramp(ramp: &[f64], v: f64) -> f64 {
    if ramp.is_empty() {
        return 1.0;
    }
    if ramp.len() == 1 {
        return ramp[0];
    }
    let x = v.clamp(0.0, 1.0) * (ramp.len() - 1) as f64;
    let k = (x as usize).min(ramp.len() - 2);
    mixd(ramp[k], ramp[k + 1], x - k as f64)
}

/// The border joining the end of one side to the end of the other, walking away from skip_edge; empty if none.
fn end_border(m: &Mesh, from: Index, skip_edge: Index, to: Index, stop: &HashSet<Index>, chain_edges: &mut Vec<Index>) -> Vec<Index> {
    let mut chain = vec![from];
    chain_edges.clear();
    let mut cur = from;
    let mut prev_e = skip_edge;
    for _ in 0..1024 {
        let mut next = INVALID;
        let mut count = 0;
        m.for_each_point_edge(cur, |e| {
            if e != prev_e && m.is_boundary_edge(e) {
                next = e;
                count += 1;
            }
        });
        if count != 1 {
            return Vec::new();
        }
        let p = m.edge_other_point(next, cur);
        chain.push(p);
        chain_edges.push(next);
        if p == to {
            return chain;
        }
        if stop.contains(&p) {
            return Vec::new();
        }
        cur = p;
        prev_e = next;
    }
    Vec::new()
}

/// Attribute sources of a strip point: corners and points with weights.
#[derive(Clone, Debug, Default)]
struct Sources {
    corners: Vec<Index>,
    points: Vec<Index>,
    corner_w: Vec<f32>,
    point_w: Vec<f32>,
}

/// Open ends: point ids per strip row, extra border points per band, and the displacement of each half row.
#[derive(Clone, Debug, Default)]
struct End {
    active: bool,
    ids: Vec<Index>,
    extras: Vec<Vec<Index>>,
    shift: Vec<D3>,
    /// Per row, the side face's corner at a reused border point.
    existing_corner: Vec<Index>,
}

struct PairBridge<'a> {
    mesh: &'a mut Mesh,
    o: &'a BridgeOptions,
    r: &'a mut BridgeResult,
    s0: Side,
    s1: Side,
    p0: Vec<SidePoint>,
    p1: Vec<SidePoint>,
    closed: bool,
    big_is_0: bool,
    n: usize,
    /// Per column: where it meets each side, and its curve.
    st0: Vec<Station>,
    st1: Vec<Station>,
    curve: Vec<Dense>,
    /// Per column, the band (counted from the big side) in which it ends; i32::MAX for columns reaching the small side.
    drop: Vec<i32>,
    rows: i32,
    /// Arc fraction at each half row.
    row_v: Vec<f64>,
    end: [End; 2],
    /// Per half row.
    centre: Vec<D3>,
    axis: D3,
    normal: D3,
    ids: HashMap<u64, Index>,
    new_points: Vec<Index>,
    src_face: Index,
}

impl<'a> PairBridge<'a> {
    fn new(m: &'a mut Mesh, p: &EdgePath, q: &EdgePath, o: &'a BridgeOptions, r: &'a mut BridgeResult) -> Self {
        let s0 = side_from(m, p);
        let s1 = side_from(m, q);
        Self {
            mesh: m,
            o,
            r,
            s0,
            s1,
            p0: Vec::new(),
            p1: Vec::new(),
            closed: false,
            big_is_0: true,
            n: 0,
            st0: Vec::new(),
            st1: Vec::new(),
            curve: Vec::new(),
            drop: Vec::new(),
            rows: 1,
            row_v: Vec::new(),
            end: [End::default(), End::default()],
            centre: Vec::new(),
            axis: D3::ZERO,
            normal: D3::ZERO,
            ids: HashMap::new(),
            new_points: Vec::new(),
            src_face: INVALID,
        }
    }

    fn set_up(&mut self) -> bool {
        if self.s0.closed != self.s1.closed {
            self.r.warnings.push("a closed loop cannot bridge to an open chain".into());
            return false;
        }
        self.closed = self.s0.closed;
        let min_pts = if self.closed { 3 } else { 2 };
        if self.s0.pts.len() < min_pts || self.s1.pts.len() < min_pts {
            return false;
        }
        // Read the target the same way round as the source; a wire side has no faces to say which way, so pairing
        // decides.
        if !self.s1.wire {
            reverse_side(&mut self.s1);
        }
        let m: &Mesh = self.mesh;
        let positions = |s: &Side| -> Vec<D3> { s.pts.iter().map(|&p| m.position(p)).collect() };

        // Explicit pairs: the first fixes both starts, the rest pin columns.
        let mut pins = Vec::new();
        for &(a, b) in &self.o.pairs {
            let (a_in0, b_in1) = (self.s0.pts.contains(&a), self.s1.pts.contains(&b));
            let (a_in1, b_in0) = (self.s1.pts.contains(&a), self.s0.pts.contains(&b));
            if a_in0 && b_in1 {
                pins.push((a, b));
            } else if a_in1 && b_in0 {
                pins.push((b, a));
            }
        }
        if self.closed && !pins.is_empty() {
            let i0 = self.s0.pts.iter().position(|&p| p == pins[0].0).unwrap();
            rotate_side(&mut self.s0, i0);
            let i1 = self.s1.pts.iter().position(|&p| p == pins[0].1).unwrap();
            rotate_side(&mut self.s1, i1);
        }

        self.big_is_0 = self.s0.pts.len() >= self.s1.pts.len();
        let big_is_0 = self.big_is_0;
        let (big, mut small) = if big_is_0 { (self.s0.clone(), self.s1.clone()) } else { (self.s1.clone(), self.s0.clone()) };
        let big_p = positions(&big);
        let n = small.pts.len();

        let forced_for = |sm: &Side| {
            let mut f = HashMap::new();
            for &(a, b) in &pins {
                let (bp, sp) = if big_is_0 { (a, b) } else { (b, a) };
                if let (Some(bi), Some(si)) = (big.pts.iter().position(|&x| x == bp), sm.pts.iter().position(|&x| x == sp)) {
                    f.insert(si, bi);
                }
            }
            f
        };

        // Search the small side's start and direction (crossing only when it pays well, or for wire).
        let pinned_start = self.closed && !pins.is_empty();
        let mut best_cost = [f64::MAX; 2];
        let mut best_start = [0usize; 2];
        for crossed in 0..2 {
            let mut trial = small.clone();
            if crossed == 1 {
                reverse_side(&mut trial);
            }
            let starts = if self.closed && !pinned_start { n } else { 1 };
            for s in 0..starts {
                let mut rot = trial.clone();
                rotate_side(&mut rot, s);
                let mut cost = 0.0;
                assign_columns(&big_p, &positions(&rot), self.closed, &forced_for(&rot), &mut cost);
                if cost < best_cost[crossed] {
                    best_cost[crossed] = cost;
                    best_start[crossed] = s;
                }
            }
        }
        let any_wire = self.s0.wire || self.s1.wire;
        let mut crossed =
            if any_wire { best_cost[1] < best_cost[0] } else { pins.is_empty() && best_cost[1] < 0.25 * best_cost[0] };
        crossed = crossed != (self.o.reverse_source != self.o.reverse_target);
        if crossed {
            reverse_side(&mut small);
        }
        if self.closed {
            rotate_side(&mut small, best_start[if crossed { 1 } else { 0 }]);
        }
        if self.closed && self.o.offset != 0 {
            rotate_side(&mut small, ((self.o.offset % n as i32) + n as i32) as usize % n);
        }

        let mut cost = 0.0;
        let mut col = assign_columns(&big_p, &positions(&small), self.closed, &forced_for(&small), &mut cost);
        if self.o.transition == BridgeTransition::Quads {
            even_up(&mut col, big.pts.len(), self.closed, &forced_for(&small));
        }
        self.n = big.pts.len();
        let nn = self.n;

        // Stations: big side exact at every column, small side in between where it has no point.
        let pb = arc_params(&big_p, self.closed);
        let big_st: Vec<Station> = (0..nn).map(|c| Station { a: c, b: c, t: 0.0 }).collect();
        let mut small_st = vec![Station::default(); nn];
        let segs = if self.closed { n } else { n - 1 };
        for i in 0..segs {
            let c0 = col[i];
            let c1 = if i + 1 < n { col[i + 1] } else { nn };
            let p0 = pb[c0];
            let p1 = if c1 < nn { pb[c1] } else { 1.0 };
            for (c, st) in small_st.iter_mut().enumerate().take(c1).skip(c0) {
                let t = if c == c0 {
                    0.0
                } else if p1 - p0 > EPS {
                    (pb[c] - p0) / (p1 - p0)
                } else {
                    0.0
                };
                *st = Station { a: i, b: (i + 1) % n, t };
            }
        }
        if !self.closed {
            small_st[nn - 1] = Station { a: n - 1, b: n - 1, t: 0.0 };
        }
        self.drop = small_st.iter().map(|s| if s.exact() { i32::MAX } else { -1 }).collect();
        if big_is_0 {
            self.s1 = small;
            self.st0 = big_st;
            self.st1 = small_st;
        } else {
            self.s0 = small;
            self.st0 = small_st;
            self.st1 = big_st;
        }

        let m: &Mesh = self.mesh;
        let avg = |s: &Side| s.pts.iter().map(|&p| m.position(p)).sum::<D3>() / s.pts.len() as f64;
        let c0 = avg(&self.s0);
        let c1 = avg(&self.s1);
        self.p0 = side_points(m, &self.s0, &self.s1, c1);
        self.p1 = side_points(m, &self.s1, &self.s0, c0);

        for s in [&self.s0, &self.s1] {
            for &e in &s.edges {
                if self.src_face == INVALID && m.edge_corner(e) != INVALID {
                    self.src_face = m.corner_face(m.edge_corner(e));
                }
            }
        }
        true
    }

    fn pick_rows(&mut self) {
        let both_wire = self.s0.wire && self.s1.wire;
        let nn = self.n;
        self.curve = vec![Dense::default(); nn];
        let a: Vec<StationData> = (0..nn).map(|c| at_station(&self.p0, &self.st0[c])).collect();
        let b: Vec<StationData> = (0..nn).map(|c| at_station(&self.p1, &self.st1[c])).collect();

        if self.o.curve == BridgeCurve::Path && self.o.path.len() >= 2 && polyline_length(&self.o.path) > EPS {
            // The path carried from one side's centre to the other's, each side's section turned along with it.
            let c_a = a.iter().map(|s| s.pos).sum::<D3>() / nn as f64;
            let c_b = b.iter().map(|s| s.pos).sum::<D3>() / nn as f64;
            let ps = self.o.path[0];
            let pe = *self.o.path.last().unwrap();
            let rot = rotation_between(unit(pe - ps), unit(c_b - c_a));
            let scale = if pe.distance(ps) > EPS { c_b.distance(c_a) / pe.distance(ps) } else { 1.0 };
            let mut path = Dense::default();
            for &p in &self.o.path {
                path.p.push(c_a + rot * (p - ps) * scale);
            }
            path.finish();
            let along: Vec<D3> = (0..=DENSE).map(|i| path.at(i as f64 / DENSE as f64)).collect();
            let tangent: Vec<D3> =
                (0..=DENSE).map(|i| unit(along[(i + 1).min(DENSE)] - along[i.saturating_sub(1)])).collect();
            let mut from_start = vec![DMat3::IDENTITY; DENSE + 1];
            for i in 1..=DENSE {
                from_start[i] = rotation_between(tangent[i - 1], tangent[i]) * from_start[i - 1];
            }
            let end_inv = from_start[DENSE].transpose();
            for c in 0..nn {
                let o_a = a[c].pos - c_a;
                let o_b = b[c].pos - c_b;
                let d = &mut self.curve[c];
                for i in 0..=DENSE {
                    let t = i as f64 / DENSE as f64;
                    let straight = mix3(a[c].pos, b[c].pos, t);
                    let curved = along[i] + (1.0 - t) * (from_start[i] * o_a) + t * (from_start[i] * end_inv * o_b);
                    d.p.push(mix3(straight, curved, self.o.smoothness));
                }
                d.finish();
            }
        } else {
            if self.o.curve == BridgeCurve::Path {
                self.r.warnings.push("Path needs a curve; bridged with Blend".into());
            }
            let mut o = self.o.clone();
            if o.curve == BridgeCurve::Path {
                o.curve = BridgeCurve::Blend;
            }
            for c in 0..nn {
                let curve = column_curve(&a[c], &b[c], &o, both_wire);
                let d = &mut self.curve[c];
                let k = curve.len() - 1;
                for i in 0..=DENSE {
                    let t = i as f64 / DENSE as f64;
                    let cv = if k == 1 { mix3(curve[0], curve[1], t) } else { curve[i.min(k)] };
                    d.p.push(mix3(mix3(a[c].pos, b[c].pos, t), cv, self.o.smoothness));
                }
                d.finish();
            }
        }

        // Spacing of the edge flow either side, for row count and grading.
        let (mut ha, mut hb, mut len) = (0.0, 0.0, 0.0);
        let (mut na, mut nb) = (0, 0);
        for c in 0..nn {
            len += self.curve[c].length();
            if let Some(&f) = a[c].behind.first() {
                ha += a[c].pos.distance(f);
                na += 1;
            }
            if let Some(&f) = b[c].behind.first() {
                hb += b[c].pos.distance(f);
                nb += 1;
            }
        }
        len /= nn as f64;
        ha = if na > 0 { ha / na as f64 } else { 0.0 };
        hb = if nb > 0 { hb / nb as f64 } else { 0.0 };
        let mut edge_len = 0.0;
        let mut ne = 0;
        for s in [&self.s0, &self.s1] {
            for &e in &s.edges {
                edge_len += self.mesh.edge_length(e);
                ne += 1;
            }
        }
        edge_len = if ne > 0 { edge_len / ne as f64 } else { 1.0 };

        // Open ends: the border joining the sides' ends, which may fix the row count.
        let mut end_chain: [Vec<Index>; 2] = [Vec::new(), Vec::new()];
        let mut end_edges: [Vec<Index>; 2] = [Vec::new(), Vec::new()];
        if !self.closed {
            let stop: HashSet<Index> = self.s0.pts.iter().chain(self.s1.pts.iter()).copied().collect();
            for e in 0..2 {
                let c = if e == 1 { nn - 1 } else { 0 };
                let from = self.s0.pts[self.st0[c].a];
                let to = self.s1.pts[self.st1[c].a];
                if from == to {
                    end_chain[e] = vec![from];
                    continue;
                }
                if !self.o.weld_ends {
                    continue;
                }
                let skip = self.s0.edges[if e == 1 { self.s0.edges.len() - 1 } else { 0 }];
                end_chain[e] = end_border(self.mesh, from, skip, to, &stop, &mut end_edges[e]);
            }
        }

        let mut rows = self.o.divisions + 1;
        if self.o.divisions < 0 {
            let mut from_ends = -1;
            for e in 0..2 {
                if end_chain[e].len() >= 2 {
                    from_ends = from_ends.max(end_chain[e].len() as i32 - 1);
                }
            }
            let h = if na > 0 && nb > 0 {
                (ha + hb) / 2.0
            } else if na > 0 {
                ha
            } else if nb > 0 {
                hb
            } else {
                edge_len
            };
            rows = if from_ends > 0 { from_ends } else { (len / h.max(EPS)).round() as i32 };
        }
        self.rows = rows.clamp(1, 256);

        // Rows graded from the spacing behind the source to that behind the target (Auto), else even.
        let hh = 2 * self.rows;
        let grade = self.o.curve == BridgeCurve::Auto && na > 0 && nb > 0 && ha > EPS && hb > EPS;
        self.row_v = (0..=hh)
            .map(|h| {
                let x = h as f64 / hh as f64;
                // Spacing grows linearly from ha to hb: its integral, normalised.
                if grade {
                    (ha * x + 0.5 * (hb - ha) * x * x) / (0.5 * (ha + hb))
                } else {
                    x
                }
            })
            .collect();

        // Columns that do not reach the small side end in a band: 3-to-1 quads in pairs, one 2-to-1 for an odd count.
        {
            let keep: Vec<usize> = (0..nn).filter(|&c| self.drop[c] != -1).collect();
            let rr = self.rows;
            let count = if self.closed { keep.len() } else { keep.len() - 1 };
            for k in 0..count {
                let c0 = keep[k];
                let c1 = if k + 1 < keep.len() { keep[k + 1] } else { keep[0] + nn };
                let gone: Vec<usize> = (c0 + 1..c1).map(|c| c % nn).collect();
                if gone.is_empty() {
                    continue;
                }
                if self.o.transition == BridgeTransition::Triangles {
                    for &c in &gone {
                        self.drop[c] = rr / 2;
                    }
                    continue;
                }
                // Pairs of columns end together (3-to-1); an odd one out (2-to-1, one triangle) sits mid-run, where it
                // shows least.
                let mut groups: Vec<Vec<usize>> = Vec::new();
                let twos = gone.len() / 2;
                let single = if gone.len() % 2 == 1 { twos / 2 } else { usize::MAX };
                let mut g = 0;
                let mut idx = 0;
                while idx < gone.len() {
                    if g == single {
                        groups.push(vec![gone[idx]]);
                        idx += 1;
                    } else {
                        groups.push(vec![gone[idx], gone[idx + 1]]);
                        idx += 2;
                    }
                    g += 1;
                }
                let gg = groups.len() as i32;
                if gg <= rr {
                    let start = (rr - gg) / 2;
                    for (g, grp) in groups.iter().enumerate() {
                        for &c in grp {
                            self.drop[c] = start + g as i32;
                        }
                    }
                } else {
                    for (g, grp) in groups.iter().enumerate() {
                        for &c in grp {
                            self.drop[c] = (g as i32).min(rr - 1);
                        }
                    }
                }
            }
        }

        // Open ends: weld the rows to the border between the sides' ends.
        for e in 0..2 {
            let rows = self.rows as usize;
            let mut end = End {
                active: false,
                ids: Vec::new(),
                extras: vec![Vec::new(); rows],
                shift: vec![D3::ZERO; hh as usize + 1],
                existing_corner: vec![INVALID; rows + 1],
            };
            if self.closed || end_chain[e].is_empty() {
                self.end[e] = end;
                continue;
            }
            end.active = true;
            let c = if e == 1 { nn - 1 } else { 0 };
            end.ids = vec![INVALID; rows + 1];
            end.ids[0] = self.s0.pts[self.st0[c].a];
            end.ids[rows] = self.s1.pts[self.st1[c].a];
            let chain = &mut end_chain[e];
            if chain.len() == 1 {
                end.ids.fill(chain[0]);
                for h in 0..=hh as usize {
                    end.shift[h] = self.mesh.position(chain[0]) - self.curve[c].at(self.row_v[h]);
                }
                self.end[e] = end;
                continue;
            }
            let edges = &mut end_edges[e];
            let fractions = |mesh: &Mesh, chain: &[Index]| {
                let mut f = vec![0.0; chain.len()];
                for i in 1..chain.len() {
                    f[i] = f[i - 1] + mesh.position(chain[i - 1]).distance(mesh.position(chain[i]));
                }
                let total = *f.last().unwrap();
                for x in f.iter_mut() {
                    *x = if total > EPS { *x / total } else { 0.0 };
                }
                f
            };
            let f = fractions(self.mesh, chain);
            let inner = chain.len() - 2;
            // Chain point -> strip row it serves.
            let mut row_of = vec![-1i32; chain.len()];
            row_of[0] = 0;
            *row_of.last_mut().unwrap() = self.rows;
            if inner == rows - 1 {
                for (i, r) in row_of.iter_mut().enumerate().take(chain.len() - 1).skip(1) {
                    *r = i as i32;
                }
            } else {
                let tol = self.o.weld_tolerance / self.rows as f64;
                let mut next = 1;
                for r in 1..self.rows {
                    let want = self.row_v[2 * r as usize];
                    let mut best = 0;
                    let mut bd = tol;
                    for i in next..chain.len() - 1 {
                        let d = (f[i] - want).abs();
                        if d <= bd {
                            bd = d;
                            best = i;
                        }
                    }
                    if best != 0 {
                        row_of[best] = r;
                        next = best + 1;
                    }
                }
            }
            // Rows with no point split the border where they fall.
            let mut row_ids = vec![INVALID; rows + 1];
            for i in 0..chain.len() {
                if row_of[i] >= 0 {
                    row_ids[row_of[i] as usize] = chain[i];
                }
            }
            for r in 1..rows {
                if row_ids[r] != INVALID {
                    self.r.welded += 1;
                    continue;
                }
                let fr = fractions(self.mesh, chain);
                let want = self.row_v[2 * r];
                let mut j = 0;
                while j + 2 < chain.len() && fr[j + 1] < want {
                    j += 1;
                }
                let span = fr[j + 1] - fr[j];
                let t = if span > EPS { ((want - fr[j]) / span).clamp(0.05, 0.95) } else { 0.5 };
                let edge = edges[j];
                let forward = self.mesh.edge_point(edge, 0) == chain[j];
                let np = self.mesh.split_edge(edge, if forward { t } else { 1.0 - t });
                let other = self.mesh.last_split_edge();
                chain.insert(j + 1, np);
                edges[j] = if forward { edge } else { other };
                edges.insert(j + 1, if forward { other } else { edge });
                row_of.insert(j + 1, r as i32);
                row_ids[r] = np;
                self.r.split += 1;
            }
            for i in 0..chain.len() {
                if row_of[i] < 0 {
                    // A border point between rows: it stays a corner of the end face of its band.
                    let band = row_of[..i].iter().rev().find(|&&x| x >= 0).copied().unwrap_or(0);
                    end.extras[(band as usize).min(rows - 1)].push(chain[i]);
                    self.r.unwelded += 1;
                }
            }
            end.ids = row_ids.clone();
            for r in 0..=rows {
                let ci = chain.iter().position(|&x| x == row_ids[r]).unwrap_or(chain.len());
                let ei = if ci < edges.len() { ci } else { edges.len() - 1 };
                let cr = self.mesh.edge_corner(edges[ei]);
                if cr != INVALID {
                    end.existing_corner[r] = if self.mesh.corner_point(cr) == row_ids[r] { cr } else { self.mesh.corner_next(cr) };
                }
                end.shift[2 * r] = self.mesh.position(row_ids[r]) - self.curve[c].at(self.row_v[2 * r]);
            }
            for h in (1..hh as usize).step_by(2) {
                end.shift[h] = 0.5 * (end.shift[h - 1] + end.shift[h + 1]);
            }
            self.end[e] = end;
        }

        // Row centres and the bridge's axis for taper, profile and twist.
        self.centre = (0..=hh).map(|h| (0..nn).map(|c| self.base(c, h)).sum::<D3>() / nn as f64).collect();
        self.axis = unit(self.centre[hh as usize] - self.centre[0]);
        for sp in self.p0.iter().chain(self.p1.iter()) {
            if sp.corner != INVALID {
                self.normal += self.mesh.face_normal(self.mesh.corner_face(sp.corner));
            }
        }
    }

    fn base(&self, c: usize, h: i32) -> D3 {
        let mut p = self.curve[c].at(self.row_v[h as usize]);
        if self.closed {
            return p;
        }
        // Rows the open ends moved carry over to the columns near them, fading out.
        let f = (self.n as f64 / 3.0).clamp(1.0, 4.0);
        for e in 0..2 {
            if self.end[e].active {
                let d = if e == 1 { (self.n - 1 - c) as f64 } else { c as f64 };
                if d < f {
                    p += self.end[e].shift[h as usize] * smoothstep(1.0 - d / f);
                }
            }
        }
        p
    }

    fn shaped(&self, c: usize, h: i32) -> D3 {
        let mut p = self.base(c, h);
        let hh = 2 * self.rows;
        if h == 0 || h == hh {
            return p;
        }
        let v = h as f64 / hh as f64;
        let centre = self.centre[h as usize];
        if self.o.taper != 1.0 {
            let bias = self.o.bias.clamp(0.01, 0.99);
            let x = if v < bias { v / bias } else { (1.0 - v) / (1.0 - bias) };
            let s = 1.0 + (self.o.taper - 1.0) * smoothstep(x) * sample_ramp(&self.o.taper_curve, v);
            p = centre + (p - centre) * s;
        }
        if self.o.profile_factor != 0.0 {
            let amount = self.o.profile_factor * profile_falloff(self.o.profile_shape, 1.0 - (2.0 * v - 1.0).abs());
            if self.closed {
                p = centre + (p - centre) * (1.0 + amount);
            } else {
                let c0 = if c > 0 { c - 1 } else { c };
                let c1 = if c + 1 < self.n { c + 1 } else { c };
                let across = self.base(c1, h) - self.base(c0, h);
                let along = self.base(c, (h + 1).min(hh)) - self.base(c, (h - 1).max(0));
                let mut n = unit(across.cross(along));
                if n.dot(self.normal) < 0.0 {
                    n = -n;
                }
                p += n * (amount * self.curve[c].length());
            }
        }
        if self.o.twist != 0.0 && self.axis.length() > 0.5 {
            p = centre + rotation_about(self.axis, self.o.twist.to_radians() * v) * (p - centre);
        }
        p
    }

    fn sources_at(&self, c: usize, h: i32) -> Sources {
        let mut s = Sources::default();
        let v = h as f64 / (2 * self.rows) as f64;
        let mut add = |sp: &[SidePoint], side: &Side, st: &Station, w: f64| {
            if w <= 0.0 {
                return;
            }
            let ta = if st.exact() { 1.0 } else { 1.0 - st.t };
            for (k, wk) in [(st.a, ta), (st.b, if st.exact() { 0.0 } else { st.t })] {
                if wk <= 0.0 {
                    continue;
                }
                if sp[k].corner != INVALID {
                    s.corners.push(sp[k].corner);
                    s.corner_w.push((w * wk) as f32);
                }
                s.points.push(side.pts[k]);
                s.point_w.push((w * wk) as f32);
            }
        };
        add(&self.p0, &self.s0, &self.st0[c], 1.0 - v);
        add(&self.p1, &self.s1, &self.st1[c], v);
        s
    }

    fn point_at(&mut self, c: usize, h: i32) -> Index {
        let hh = 2 * self.rows;
        let e = if c > 0 { 1 } else { 0 };
        if !self.closed && (c == 0 || c == self.n - 1) && self.end[e].active && h % 2 == 0 {
            return self.end[e].ids[(h / 2) as usize];
        }
        if h == 0 && self.st0[c].exact() {
            return self.s0.pts[self.st0[c].a];
        }
        if h == hh && self.st1[c].exact() {
            return self.s1.pts[self.st1[c].a];
        }
        let key = ((c as u64) << 32) | h as u32 as u64;
        if let Some(&id) = self.ids.get(&key) {
            return id;
        }
        let pos = self.shaped(c, h);
        let np = self.mesh.add_point(pos);
        let s = self.sources_at(c, h);
        if !s.points.is_empty() {
            self.mesh.point_attrs_mut().mix(np as usize, &s.points, &s.point_w);
        }
        self.ids.insert(key, np);
        self.new_points.push(np);
        np
    }

    fn build_faces(&mut self) {
        let rr = self.rows;
        let nn = self.n;
        let big_is_0 = self.big_is_0;
        // Rows counted from the big side: alive at row b iff b <= drop.
        let alive = |drop: &[i32], c: usize, b: i32| drop[c] >= b;
        let to_strip = |big_half_row: i32| if big_is_0 { big_half_row } else { 2 * rr - big_half_row };

        // (column, strip half row).
        let mut polys: Vec<Vec<(usize, i32)>> = Vec::new();
        for b in 0..rr {
            let bottom: Vec<usize> = (0..nn).filter(|&c| alive(&self.drop, c, b + 1)).collect();
            let pairs = if self.closed { bottom.len() } else { bottom.len() - 1 };
            let (top, mid, bot) = (to_strip(2 * b), to_strip(2 * b + 1), to_strip(2 * b + 2));
            for k in 0..pairs {
                let b0 = bottom[k];
                let b1 = bottom[(k + 1) % bottom.len()];
                let mut t = vec![b0];
                let mut c = (b0 + 1) % nn;
                while c != b1 {
                    if alive(&self.drop, c, b) {
                        t.push(c);
                    }
                    c = (c + 1) % nn;
                }
                t.push(b1);
                let edges_top = t.len() - 1;
                if edges_top == 1 {
                    polys.push(vec![(b0, top), (b1, top), (b1, bot), (b0, bot)]);
                } else if edges_top == 2 && self.o.transition == BridgeTransition::Quads {
                    polys.push(vec![(t[0], top), (t[1], top), (t[1], mid), (t[0], bot)]);
                    polys.push(vec![(t[1], top), (t[2], top), (t[2], bot), (t[1], mid)]);
                    polys.push(vec![(t[0], bot), (t[1], mid), (t[2], bot)]);
                    self.r.triangles += 1;
                } else if edges_top == 3 && self.o.transition == BridgeTransition::Quads {
                    polys.push(vec![(t[0], top), (t[1], top), (t[1], mid), (t[0], bot)]);
                    polys.push(vec![(t[1], top), (t[2], top), (t[2], mid), (t[1], mid)]);
                    polys.push(vec![(t[2], top), (t[3], top), (t[3], bot), (t[2], mid)]);
                    polys.push(vec![(t[0], bot), (t[1], mid), (t[2], mid), (t[3], bot)]);
                } else {
                    for j in 0..edges_top - 1 {
                        polys.push(vec![(t[j], top), (t[j + 1], top), (t[0], bot)]);
                        self.r.triangles += 1;
                    }
                    polys.push(vec![(t[edges_top - 1], top), (t[edges_top], top), (t[edges_top], bot), (t[0], bot)]);
                }
            }
        }

        // Points, with the open ends' leftover border points as extra corners of their band's face.
        struct Poly {
            pts: Vec<Index>,
            src: Vec<Sources>,
        }
        let mut out: Vec<Poly> = Vec::new();
        for refs in &polys {
            let mut poly = Poly { pts: Vec::new(), src: Vec::new() };
            for i in 0..refs.len() {
                let (xc, xh) = refs[i];
                let id = self.point_at(xc, xh);
                let mut s = self.sources_at(xc, xh);
                let ei = if xc > 0 { 1 } else { 0 };
                let at_end = !self.closed && (xc == 0 || xc == nn - 1) && self.end[ei].active;
                if at_end && xh % 2 == 0 {
                    let cr = self.end[ei].existing_corner[(xh / 2) as usize];
                    if cr != INVALID {
                        s = Sources { corners: vec![cr], points: Vec::new(), corner_w: vec![1.0], point_w: Vec::new() };
                    }
                }
                poly.pts.push(id);
                poly.src.push(s);
                let (yc, yh) = refs[(i + 1) % refs.len()];
                if at_end && yc == xc && (yh - xh).abs() == 2 {
                    let mut extra = self.end[ei].extras[(xh.min(yh) / 2) as usize].clone();
                    if yh < xh {
                        extra.reverse();
                    }
                    for p in extra {
                        poly.pts.push(p);
                        poly.src.push(Sources::default());
                    }
                }
            }
            // Collapsed ends repeat a point; drop the repeats.
            let mut clean = Poly { pts: Vec::new(), src: Vec::new() };
            for (p, s) in poly.pts.into_iter().zip(poly.src) {
                if clean.pts.last() != Some(&p) {
                    clean.pts.push(p);
                    clean.src.push(s);
                }
            }
            while clean.pts.len() > 1 && clean.pts.first() == clean.pts.last() {
                clean.pts.pop();
                clean.src.pop();
            }
            if clean.pts.len() >= 3 {
                out.push(clean);
            }
        }

        // Wind to continue the faces along the borders; with none (wire), facing away from the middle.
        let mut flip = false;
        let mut decided = false;
        let side_edges: HashSet<Index> = self.s0.edges.iter().chain(self.s1.edges.iter()).copied().collect();
        'find: for p in &out {
            for i in 0..p.pts.len() {
                let (u, w) = (p.pts[i], p.pts[(i + 1) % p.pts.len()]);
                let e = self.mesh.find_edge(u, w);
                if e == INVALID || !side_edges.contains(&e) {
                    continue;
                }
                let c = self.mesh.edge_corner(e);
                if c != INVALID {
                    flip = self.mesh.corner_point(c) == u;
                    decided = true;
                    break 'find;
                }
            }
        }
        if !decided && !out.is_empty() {
            let m: &Mesh = self.mesh;
            let mut mid = D3::ZERO;
            let mut count = 0;
            for p in &out {
                for &id in &p.pts {
                    mid += m.position(id);
                    count += 1;
                }
            }
            mid /= count as f64;
            let mut sum = 0.0;
            for p in &out {
                let mut n = D3::ZERO;
                let mut c = D3::ZERO;
                for i in 0..p.pts.len() {
                    n += m.position(p.pts[i]).cross(m.position(p.pts[(i + 1) % p.pts.len()]));
                    c += m.position(p.pts[i]);
                }
                c /= p.pts.len() as f64;
                sum += n.dot(c - mid);
            }
            flip = sum < 0.0;
        }

        for mut p in out {
            if flip {
                p.pts.reverse();
                p.src.reverse();
            }
            let f = add_face_from(self.mesh, &p.pts, self.src_face, &[], &[]);
            if f == INVALID {
                continue;
            }
            self.r.faces.push(f);
            let mut c = self.mesh.face_corner(f);
            for s in &p.src {
                if !s.corners.is_empty() {
                    mix_corners(self.mesh, c, &s.corners, &s.corner_w);
                }
                c = self.mesh.corner_next(c);
            }
        }
    }

    fn merge(&mut self) {
        // Each big-side point merges into the small-side point its column is nearest, at merge_factor between the
        // sides.
        let (big, small, st) = if self.big_is_0 { (&self.s0, &self.s1, &self.st1) } else { (&self.s1, &self.s0, &self.st0) };
        let mut into: BTreeMap<Index, Vec<Index>> = BTreeMap::new();
        let mut at: HashMap<Index, D3> = HashMap::new();
        let f = self.o.merge_factor.clamp(0.0, 1.0);
        for c in 0..self.n {
            let bp = big.pts[c];
            let sp = small.pts[if st[c].exact() || st[c].t < 0.5 { st[c].a } else { st[c].b }];
            if bp == sp {
                continue;
            }
            into.entry(sp).or_default().push(bp);
            if st[c].exact() {
                let (src, tgt) = if self.big_is_0 { (bp, sp) } else { (sp, bp) };
                at.insert(sp, mix3(self.mesh.position(src), self.mesh.position(tgt), f));
            }
        }
        for (sp, mut bps) in into {
            if !self.mesh.point_valid(sp) {
                continue;
            }
            if let Some(&p) = at.get(&sp) {
                self.mesh.set_position(sp, p);
            }
            bps.retain(|&p| self.mesh.point_valid(p));
            if !bps.is_empty() {
                merge_to_target(self.mesh, &bps, sp);
            }
        }
        self.r.pairs_bridged += 1;
    }

    fn run(&mut self) {
        if !self.set_up() {
            return;
        }
        if self.o.merge {
            self.merge();
            return;
        }
        self.pick_rows();
        self.build_faces();
        self.r.rows = self.r.rows.max(self.rows);
        self.r.pairs_bridged += 1;

        if self.o.relax > 0 && !self.new_points.is_empty() {
            for _ in 0..self.o.relax {
                let m: &Mesh = self.mesh;
                let next: Vec<(Index, D3)> = self
                    .new_points
                    .iter()
                    .filter_map(|&p| {
                        let mut sum = D3::ZERO;
                        let mut k = 0;
                        m.for_each_point_edge(p, |e| {
                            sum += m.position(m.edge_other_point(e, p));
                            k += 1;
                        });
                        (k > 0).then(|| (p, 0.5 * m.position(p) + 0.5 * sum / k as f64))
                    })
                    .collect();
                for (p, pos) in next {
                    self.mesh.set_position(p, pos);
                }
            }
        }
    }
}

/// Bridges the selected edges: border (or wire) chains pair up per opts.connect and each pair is joined with a strip
/// of faces. Borders of different lengths pair by arc length. Returns the new faces and what happened at the ends.
pub fn bridge(mesh: &mut Mesh, edges: &[Index], opts: &BridgeOptions) -> BridgeResult {
    let mut r = BridgeResult::default();
    for (a, b) in plan_pairs(mesh, edges, opts.connect) {
        // Later pairs were planned before earlier ones split end borders or merged points; skip any that went stale.
        let fresh = a.edges.iter().chain(b.edges.iter()).all(|&e| mesh.edge_valid(e) && mesh.edge_face_count(e) <= 1);
        if !fresh {
            r.warnings.push("a border changed before its turn to bridge; bridge it again".into());
            continue;
        }
        PairBridge::new(mesh, &a, &b, opts, &mut r).run();
    }
    if opts.smoothing_angle >= 0.0 && !r.faces.is_empty() {
        let mut new_edges: Vec<Index> = Vec::new();
        for &f in &r.faces {
            if mesh.face_valid(f) {
                new_edges.extend(mesh.face_corners(f).iter().map(|&c| mesh.corner_edge(c)));
            }
        }
        new_edges.sort_unstable();
        new_edges.dedup();
        set_hard_edges_by_angle(mesh, opts.smoothing_angle, &new_edges);
    }
    r
}

/// Deletes the faces and bridges the holes they leave (the reference's face bridge).
pub fn bridge_faces(mesh: &mut Mesh, faces: &[Index], opts: &BridgeOptions) -> BridgeResult {
    let sel: HashSet<Index> = faces.iter().copied().filter(|&f| mesh.face_valid(f)).collect();
    // The regions' borders that another face keeps once the selection is gone.
    let mut rim = Vec::new();
    for &f in &sel {
        for c in mesh.face_corners(f) {
            let e = mesh.corner_edge(c);
            let mut other = false;
            mesh.for_each_edge_corner(e, |ec| other = other || !sel.contains(&mesh.corner_face(ec)));
            if other {
                rim.push(e);
            }
        }
    }
    rim.sort_unstable();
    rim.dedup();
    let mut list: Vec<Index> = sel.into_iter().collect();
    list.sort_unstable();
    remove_faces_clean(mesh, &list);
    rim.retain(|&e| mesh.edge_valid(e));
    bridge(mesh, &rim, opts)
}

/// Joins exactly two border chains, read from edges, with no pairing search beyond them. Returns the new faces.
pub fn bridge_edge_loops(mesh: &mut Mesh, edges: &[Index], opts: &BridgeOptions) -> Vec<Index> {
    let pairs = plan_pairs(mesh, edges, BridgeConnect::Pairs);
    let Some((a, b)) = pairs.into_iter().next() else { return Vec::new() };
    let mut r = BridgeResult::default();
    PairBridge::new(mesh, &a, &b, opts, &mut r).run();
    if opts.smoothing_angle >= 0.0 && !r.faces.is_empty() {
        let new_edges: Vec<Index> =
            r.faces.iter().flat_map(|&f| mesh.face_corners(f)).map(|c| mesh.corner_edge(c)).collect();
        set_hard_edges_by_angle(mesh, opts.smoothing_angle, &new_edges);
    }
    r.faces
}

/// Splits a selection of edges into the pairs of border chains to bridge. Edges off the border are ignored. Whole
/// chains pair with their nearest facing partner, preferring the same length; a chain left over (one loop round a
/// slot, or a U) is cut at its corners and its two longest opposite sides pair, leaving the short ends that join them.
pub fn plan_bridges(mesh: &Mesh, edges: &[Index]) -> Vec<Vec<Index>> {
    plan_pairs(mesh, edges, BridgeConnect::Pairs)
        .into_iter()
        .map(|(a, b)| {
            let mut both = a.edges;
            both.extend(b.edges);
            both
        })
        .collect()
}

/// Bridges every pair plan_bridges finds. Returns the new faces.
pub fn bridge_edges(mesh: &mut Mesh, edges: &[Index], opts: &BridgeOptions) -> Vec<Index> {
    bridge(mesh, edges, opts).faces
}
