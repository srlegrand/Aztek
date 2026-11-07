//! Edge loop tools that work along a selected loop: Offset Edge Loop and Edit Edge Flow.

use std::collections::BTreeMap;
use std::f64::consts::PI;

use glam::DVec3;

use crate::attribute::{Index, INVALID};
use crate::conform::{PointMove, PointMoves};
use crate::mesh::Mesh;
use crate::ops::basic::dissolve_edges;
use crate::ops::knife::{add_symmetric_images, knife_apply, KnifeKind, KnifePlan, KnifePoint, KnifeSegment};
use crate::symmetry::SymmetryMap;
use crate::topology::{chain_edges, edge_loop_next, make_mask, points_of_edges, unique_sorted, EdgePath};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OffsetSide {
    #[default]
    Both,
    /// The side the faces wound along the chain's direction lie on.
    Left,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum OffsetEnds {
    /// The new loops meet at an open chain's end point, splitting the faces before it.
    #[default]
    Converge,
    /// They carry on round the end point and meet on the edge continuing the loop.
    Wrap,
}

#[derive(Clone, Debug)]
pub struct OffsetLoopOptions<'a> {
    /// Fraction of each crossing edge's length, or with absolute a distance square to the chain.
    pub distance: f64,
    pub absolute: bool,
    pub side: OffsetSide,
    pub ends: OffsetEnds,
    /// Images of the chains under the symmetry are cut too; chains wholly off the driving side are left to their
    /// images.
    pub symmetry: Option<&'a SymmetryMap>,
}

impl Default for OffsetLoopOptions<'_> {
    fn default() -> Self {
        Self { distance: 0.25, absolute: false, side: OffsetSide::Both, ends: OffsetEnds::Converge, symmetry: None }
    }
}

#[derive(Clone, Debug)]
pub struct EdgeFlowOptions {
    /// 0 flat between the neighbours across the loop, 1 on the curve through them, above 1 exaggerated, negative
    /// inverted.
    pub amount: f64,
    /// The curve from the next points out across the loop (the reference's edge flow); off, from the surface normals there.
    pub edge_flow: bool,
    /// Scales the bend down (toward 1, flatter) or up (negative, fuller).
    pub tension: f64,
    /// Also eases each loop along its own length toward a smooth curve, 0 to 1, keeping its spacing.
    pub loop_smooth: f64,
    /// Passes of loop_smooth.
    pub iterations: i32,
}

impl Default for EdgeFlowOptions {
    fn default() -> Self {
        Self { amount: 1.0, edge_flow: true, tension: 0.0, loop_smooth: 0.0, iterations: 1 }
    }
}

fn mix3(a: DVec3, b: DVec3, t: f64) -> DVec3 {
    a * (1.0 - t) + b * t
}

/// The edge of face f at point p other than e.
fn other_edge_at(m: &Mesh, f: Index, p: Index, e: Index) -> Index {
    let c = m.find_corner(f, p);
    if c == INVALID {
        return INVALID;
    }
    let out = m.corner_edge(c);
    let inn = m.corner_edge(m.corner_prev(c));
    if out == e {
        inn
    } else {
        out
    }
}

/// e's faces: [0] the one wound along e from `from` (its left), [1] the other. INVALID where e has none.
fn edge_sides(m: &Mesh, e: Index, from: Index) -> [Index; 2] {
    let mut s = [INVALID, INVALID];
    m.for_each_edge_corner(e, |c| {
        let slot = if m.corner_point(c) == from && s[0] == INVALID { 0 } else { 1 };
        if s[slot] == INVALID {
            s[slot] = m.corner_face(c);
        }
    });
    s
}

/// The edges crossed going round point p from edge `from` through face f toward `to`.
#[derive(Default)]
struct Fan {
    rungs: Vec<Index>,
    /// faces[k] lies before rungs[k]; with reached, the last face lies before `to`.
    faces: Vec<Index>,
    reached: bool,
    /// Stopped at a border: the last rung has no face beyond it.
    border: bool,
}

fn walk_fan(m: &Mesh, p: Index, from: Index, mut f: Index, to: Index) -> Fan {
    let mut w = Fan::default();
    let mut cur = from;
    let start = f;
    let mut guard = 0;
    while f != INVALID && guard < 256 {
        guard += 1;
        w.faces.push(f);
        let e = other_edge_at(m, f, p, cur);
        if e == INVALID || e == from {
            break;
        }
        if e == to {
            w.reached = true;
            break;
        }
        w.rungs.push(e);
        f = m.opposite_face(e, f);
        if f == INVALID {
            w.border = m.is_boundary_edge(e);
        }
        if f == start {
            break;
        }
        cur = e;
    }
    w
}

/// The edge carrying e's loop on past p, or the one most nearly opposite e when the loop stops there.
fn loop_continuation(m: &Mesh, e: Index, p: Index) -> Index {
    let x = edge_loop_next(m, e, p);
    if x != INVALID {
        return x;
    }
    let at = m.position(p);
    let d = (m.position(m.edge_other_point(e, p)) - at).normalize();
    let mut best = INVALID;
    let mut best_dot = -0.5;
    m.for_each_point_edge(p, |x| {
        if x == e {
            return;
        }
        let dx = m.position(m.edge_other_point(x, p)) - at;
        let len = dx.length();
        if len < 1e-12 {
            return;
        }
        let c = d.dot(dx / len);
        if c < best_dot {
            best_dot = c;
            best = x;
        }
    });
    best
}

/// The chain's direction at points[i].
fn chain_tangent(m: &Mesh, path: &EdgePath, i: usize) -> DVec3 {
    let n = path.points.len();
    let prev = if path.closed || i > 0 { path.points[(i + n - 1) % n] } else { path.points[i] };
    let next = if path.closed || i + 1 < n { path.points[(i + 1) % n] } else { path.points[i] };
    let d = m.position(next) - m.position(prev);
    let len = d.length();
    if len > 1e-12 {
        d / len
    } else {
        DVec3::ZERO
    }
}

fn touches(m: &Mesh, f: Index, k: &KnifePoint) -> bool {
    match k.kind {
        KnifeKind::Point => m.find_corner(f, k.id) != INVALID,
        KnifeKind::Face => k.id == f,
        KnifeKind::Edge => {
            let mut found = false;
            m.for_each_edge_corner(k.id, |c| found = found || m.corner_face(c) == f);
            found
        }
    }
}

/// Where the offset loop crosses an edge or a face near a chain point.
struct Spotter<'a> {
    m: &'a Mesh,
    distance: f64,
    absolute: bool,
}

impl Spotter<'_> {
    /// On rung r out from p. along: r carries the chain on (a wrapped end), so the distance is not squared to it.
    fn on_edge(&self, r: Index, p: Index, tangent: DVec3, along: bool) -> KnifePoint {
        let m = self.m;
        let q = m.edge_other_point(r, p);
        let d = m.position(q) - m.position(p);
        let len = d.length();
        let mut t = self.distance;
        if self.absolute {
            let mut square = 1.0;
            if !along && len > 1e-12 && tangent.length() > 0.5 {
                let c = (d / len).dot(tangent);
                square = (1.0 - c * c).max(0.0).sqrt().max(0.25);
            }
            t = if len > 1e-12 { self.distance / square / len } else { 0.5 };
        }
        t = t.clamp(1e-3, 0.95);
        let mut k = KnifePoint::edge(r, if m.edge_point(r, 0) == p { t } else { 1.0 - t });
        k.position = mix3(m.position(p), m.position(q), t);
        k
    }

    /// Inside face f, the distance off both of its edges a and b at p.
    fn in_face(&self, f: Index, p: Index, a: Index, b: Index) -> KnifePoint {
        let m = self.m;
        let at = m.position(p);
        let to_centre = m.face_centroid(f) - at;
        let unit = |e: Index| {
            let d = m.position(m.edge_other_point(e, p)) - at;
            let len = d.length();
            if len > 1e-12 {
                d / len
            } else {
                d
            }
        };
        let ua = unit(a);
        let ub = unit(b);
        let mut w = ua + ub;
        // Straight through p: square off the edges, into the face.
        if w.length() < 1e-6 {
            w = m.face_normal(f).cross(ua);
        }
        if w.length() < 1e-12 {
            w = to_centre;
        }
        w = w.normalize();
        if w.dot(to_centre) < 0.0 {
            w = -w;
        }
        let sin_half = (0.5 * (1.0 - ua.dot(ub))).max(0.0).sqrt().max(0.2);
        let depth = to_centre.dot(w);
        // A fraction measures against the face's depth off the chain, as a rung's length does in a quad.
        let h = if self.absolute { self.distance } else { self.distance.clamp(0.0, 0.95) * 2.0 * depth * sin_half };
        let along = (h / sin_half).min(1.8 * depth.max(0.0));
        KnifePoint::face(f, at + w * along)
    }
}

/// One side's offset loop along one chain, appended to plan as segments.
struct SideCutter<'a> {
    m: &'a Mesh,
    spot: &'a Spotter<'a>,
    plan: &'a mut KnifePlan,
    prev: Option<KnifePoint>,
    first: Option<(KnifePoint, Index)>,
}

impl SideCutter<'_> {
    /// Adds k, joined to the spot before it through face f when both lie on f.
    fn add(&mut self, k: KnifePoint, f: Index) {
        if let Some(prev) = &self.prev {
            if f != INVALID && touches(self.m, f, prev) && touches(self.m, f, &k) {
                self.plan.segments.push(KnifeSegment { face: f, a: *prev, b: k, mirrored: false });
            }
        }
        if self.first.is_none() {
            self.first = Some((k, f));
        }
        self.prev = Some(k);
    }

    fn cut(&mut self) {
        self.prev = None;
    }

    /// Spots met walking out from the chain: spots[k] follows faces[k]. Outward runs away from the chain (toward the
    /// next point); inward runs back to it, so the walk is emitted reversed, starting fresh.
    fn emit(&mut self, spots: &[KnifePoint], faces: &[Index], outward: bool) {
        if outward {
            for k in 0..spots.len() {
                self.add(spots[k], faces[k]);
            }
            return;
        }
        self.cut();
        for k in (0..spots.len()).rev() {
            self.add(spots[k], if k + 1 < faces.len() { faces[k + 1] } else { INVALID });
        }
    }

    fn rung_spots(&self, w: &Fan, p: Index, tangent: DVec3) -> Vec<KnifePoint> {
        w.rungs.iter().map(|&r| self.spot.on_edge(r, p, tangent, false)).collect()
    }
}

fn plan_side(m: &Mesh, path: &EdgePath, side: usize, spot: &Spotter, ends: OffsetEnds, plan: &mut KnifePlan) {
    let np = path.points.len();
    let ne = path.edges.len();
    if ne == 0 {
        return;
    }
    let mut sc = SideCutter { m, spot, plan, prev: None, first: None };
    let side_face = |ei: usize| edge_sides(m, path.edges[ei], path.points[ei])[side];
    for i in 0..np {
        let p = path.points[i];
        let has_in = path.closed || i > 0;
        let has_out = path.closed || i + 1 < np;
        let ia = (i + ne - 1) % ne;
        let a = if has_in { path.edges[ia] } else { INVALID };
        let b = if has_out { path.edges[i] } else { INVALID };
        let fa = if has_in { side_face(ia) } else { INVALID };
        let fb = if has_out { side_face(i) } else { INVALID };
        let tangent = chain_tangent(m, path, i);

        if has_in && has_out {
            let mut from_b = fa == INVALID;
            if fa != INVALID {
                let w = walk_fan(m, p, a, fa, b);
                if w.reached && w.rungs.is_empty() {
                    sc.add(spot.in_face(fa, p, a, b), fa);
                    continue;
                }
                let spots = sc.rung_spots(&w, p, tangent);
                sc.emit(&spots, &w.faces, true);
                if w.reached {
                    continue;
                }
                sc.cut();
                from_b = w.border;
            } else {
                sc.cut();
            }
            if from_b && fb != INVALID {
                let w = walk_fan(m, p, b, fb, a);
                let spots = sc.rung_spots(&w, p, tangent);
                sc.emit(&spots, &w.faces, false);
            }
            continue;
        }

        // An open chain's end: has_in means it is the last point, so the walk out from it runs on along the path.
        let e = if has_in { a } else { b };
        let fe = if has_in { fa } else { fb };
        if fe == INVALID {
            sc.cut();
            continue;
        }
        if m.is_boundary_point(p) {
            let w = walk_fan(m, p, e, fe, INVALID);
            let spots = sc.rung_spots(&w, p, tangent);
            sc.emit(&spots, &w.faces, has_in);
            continue;
        }
        let x = loop_continuation(m, e, p);
        if ends == OffsetEnds::Converge || x == INVALID {
            let mut k = KnifePoint::point(p);
            k.position = m.position(p);
            if !has_in {
                sc.cut();
            }
            sc.add(k, fe);
            continue;
        }
        let w = walk_fan(m, p, e, fe, x);
        let mut spots = sc.rung_spots(&w, p, tangent);
        if w.reached {
            spots.push(spot.on_edge(x, p, tangent, true));
        }
        sc.emit(&spots, &w.faces, has_in);
    }
    if path.closed && sc.prev.is_some() {
        if let Some((k, f)) = sc.first {
            sc.add(k, f);
        }
    }
}

/// Plan of the reference's Offset Edge Loop: a new loop either side of each chain of the given edges, through the crossing
/// edges at the distance. A side with no crossing edge at a point is crossed inside its face. Chains ending on a
/// border end there; chains ending inside the mesh end as opts.ends says.
pub fn offset_edge_loop_plan(mesh: &Mesh, edges: &[Index], opts: &OffsetLoopOptions) -> KnifePlan {
    let mut valid: Vec<Index> = edges.iter().copied().filter(|&e| mesh.edge_valid(e)).collect();
    let sym = opts.symmetry.filter(|s| !s.is_empty());
    let mut drive = 0;
    if let Some(sym) = sym {
        drive = sym.majority_wedge(&points_of_edges(mesh, &valid));
        valid = sym.expand_edges(mesh, &valid);
    }
    let mut plan = KnifePlan::default();
    let spot = Spotter { m: mesh, distance: opts.distance, absolute: opts.absolute };
    for path in chain_edges(mesh, &valid) {
        // A chain off the driving side gets the image of its driving partner instead, so one-sided offsets mirror.
        if let Some(sym) = sym {
            if !path.points.iter().any(|&p| sym.drives(p, drive)) {
                continue;
            }
        }
        if opts.side != OffsetSide::Right {
            plan_side(mesh, &path, 0, &spot, opts.ends, &mut plan);
        }
        if opts.side != OffsetSide::Left {
            plan_side(mesh, &path, 1, &spot, opts.ends, &mut plan);
        }
    }
    if let Some(sym) = sym {
        add_symmetric_images(mesh, &mut plan, sym);
    }
    plan
}

/// Cuts offset_edge_loop_plan; with remove_original the chains' own edges are dissolved after. Returns the new edges.
pub fn offset_edge_loops(mesh: &mut Mesh, edges: &[Index], opts: &OffsetLoopOptions, remove_original: bool) -> Vec<Index> {
    let plan = offset_edge_loop_plan(mesh, edges, opts);
    let mut original = edges.to_vec();
    if let Some(sym) = opts.symmetry.filter(|s| !s.is_empty()) {
        original = sym.expand_edges(mesh, &original);
    }
    let mut made = knife_apply(mesh, &plan);
    if !remove_original || made.is_empty() {
        return made;
    }
    dissolve_edges(mesh, &original, true);
    made.retain(|&e| mesh.edge_valid(e));
    made
}

/// A line across a loop point: the anchors a and b either side, the points beyond them (INVALID when none), and where
/// the point goes between the anchors, by its count of edges to each so moving points spread evenly.
#[derive(Clone, Copy)]
struct Across {
    a: Index,
    b: Index,
    a2: Index,
    b2: Index,
    t: f64,
}

struct Anchor {
    point: Index,
    beyond: Index,
    steps: i32,
}

/// The first point past q, on the line from p through q, that is not in sel: moving points shape no tangent.
fn beyond_selected(m: &Mesh, rung: Index, q: Index, p: Index, sel: &[u8]) -> Index {
    let mut from = q;
    let mut e = edge_loop_next(m, rung, q);
    let mut guard = 0;
    while e != INVALID && guard < 1024 {
        guard += 1;
        let r = m.edge_other_point(e, from);
        if r == p || r == q {
            return INVALID;
        }
        if sel[r as usize] == 0 {
            return r;
        }
        e = edge_loop_next(m, e, r);
        from = r;
    }
    INVALID
}

/// Out from p along rung to the first point not in sel, and the point beyond that: the curve is anchored on points
/// that stay, running straight on through moving ones. Stops on a moving point at a border, or none round a ring.
fn anchor_across(m: &Mesh, rung: Index, p: Index, sel: &[u8]) -> Anchor {
    let mut from = p;
    let mut e = rung;
    for steps in 1..1024 {
        let q = m.edge_other_point(e, from);
        if q == p {
            break;
        }
        if sel[q as usize] == 0 {
            return Anchor { point: q, beyond: beyond_selected(m, e, q, from, sel), steps };
        }
        let next = edge_loop_next(m, e, q);
        if next == INVALID {
            return Anchor { point: q, beyond: INVALID, steps };
        }
        from = q;
        e = next;
    }
    Anchor { point: INVALID, beyond: INVALID, steps: 0 }
}

/// The line between the anchors out along rungs ra and rb, or None when either side has none.
fn across_line(m: &Mesh, p: Index, ra: Index, rb: Index, sel: &[u8]) -> Option<Across> {
    let a = anchor_across(m, ra, p, sel);
    let b = anchor_across(m, rb, p, sel);
    if a.point == INVALID || b.point == INVALID {
        return None;
    }
    Some(Across { a: a.point, b: b.point, a2: a.beyond, b2: b.beyond, t: a.steps as f64 / (a.steps + b.steps) as f64 })
}

/// Of the rungs, the one most square to the tangent.
fn squarest(m: &Mesh, p: Index, rungs: &[Index], tangent: DVec3) -> Index {
    let mut best = INVALID;
    let mut best_cos = 2.0;
    for &r in rungs {
        let d = m.position(m.edge_other_point(r, p)) - m.position(p);
        let len = d.length();
        let c = if len > 1e-12 { (d / len).dot(tangent).abs() } else { 1.0 };
        if c < best_cos {
            best_cos = c;
            best = r;
        }
    }
    best
}

/// The crossing edges on one side of chain point p between its chain edges a and b (either may be INVALID at an
/// end).
fn side_rungs(m: &Mesh, p: Index, a: Index, fa: Index, b: Index, fb: Index) -> Vec<Index> {
    let mut out = Vec::new();
    if a != INVALID && b != INVALID {
        if fa != INVALID {
            let w = walk_fan(m, p, a, fa, b);
            out.extend_from_slice(&w.rungs);
            if w.reached || !w.border {
                return out;
            }
        }
        if fb != INVALID {
            out.extend(walk_fan(m, p, b, fb, a).rungs);
        }
        return out;
    }
    let e = if a != INVALID { a } else { b };
    let fe = if a != INVALID { fa } else { fb };
    if fe == INVALID {
        return out;
    }
    let to = if m.is_boundary_point(p) { INVALID } else { loop_continuation(m, e, p) };
    out.extend(walk_fan(m, p, e, fe, to).rungs);
    out
}

fn chain_lines(m: &Mesh, path: &EdgePath, sel: &[u8], lines: &mut BTreeMap<Index, Vec<Across>>) {
    let np = path.points.len();
    let ne = path.edges.len();
    for i in 0..np {
        let p = path.points[i];
        let has_in = path.closed || i > 0;
        let has_out = path.closed || i + 1 < np;
        let ia = (i + ne - 1) % ne;
        let a = if has_in { path.edges[ia] } else { INVALID };
        let b = if has_out { path.edges[i] } else { INVALID };
        let sa = if has_in { edge_sides(m, a, path.points[ia]) } else { [INVALID, INVALID] };
        let sb = if has_out { edge_sides(m, b, p) } else { [INVALID, INVALID] };
        let tangent = chain_tangent(m, path, i);
        let mut rung = [INVALID; 2];
        for s in 0..2 {
            rung[s] = squarest(m, p, &side_rungs(m, p, a, sa[s], b, sb[s]), tangent);
        }
        if rung[0] == INVALID || rung[1] == INVALID {
            continue;
        }
        if let Some(l) = across_line(m, p, rung[0], rung[1], sel) {
            lines.entry(p).or_default().push(l);
        }
    }
}

fn loose_lines(m: &Mesh, p: Index, sel: &[u8], lines: &mut BTreeMap<Index, Vec<Across>>) {
    let mut used: Vec<Index> = Vec::new();
    m.for_each_point_edge(p, |e| {
        let partner = edge_loop_next(m, e, p);
        if partner == INVALID || used.contains(&e) || used.contains(&partner) {
            return;
        }
        used.push(e);
        used.push(partner);
        if let Some(l) = across_line(m, p, e, partner, sel) {
            lines.entry(p).or_default().push(l);
        }
    });
}

/// On the circle through p0, p1 and p2, the point t of the way from p1 to p2 along the arc between them that does not
/// pass p0. Straight between p1 and p2 when the three are in line.
fn arc_point(p0: DVec3, p1: DVec3, p2: DVec3, t: f64) -> DVec3 {
    let u = p0 - p1;
    let v = p2 - p1;
    let w = u.cross(v);
    let ww = w.dot(w);
    if ww < 1e-12 * u.dot(u) * v.dot(v) {
        return mix3(p1, p2, t);
    }
    let c = p1 + (u.dot(u) * v.cross(w) + v.dot(v) * w.cross(u)) / (2.0 * ww);
    let n = w / ww.sqrt();
    let r1 = p1 - c;
    let r2 = p2 - c;
    let r0 = p0 - c;
    let angle = |a: DVec3, b: DVec3| a.cross(b).dot(n).atan2(a.dot(b));
    let mut phi = angle(r1, r2);
    let psi = angle(r1, r0);
    if psi * phi > 0.0 && psi.abs() < phi.abs() {
        phi -= (2.0 * PI).copysign(phi);
    }
    let a = t * phi;
    c + r1 * a.cos() + n.cross(r1) * a.sin() + n * n.dot(r1) * (1.0 - a.cos())
}

/// The chord turned into the surface's tangent plane at a point, chord long.
fn normal_tangent(normal: DVec3, chord: DVec3) -> Option<DVec3> {
    if normal.length() < 0.5 {
        return None;
    }
    let t = chord - normal * chord.dot(normal);
    let len = t.length();
    if len < 1e-9 * chord.length().max(1.0) {
        return None;
    }
    Some(t / len * chord.length())
}

fn hermite(a: DVec3, ma: DVec3, b: DVec3, mb: DVec3, u: f64) -> DVec3 {
    let u2 = u * u;
    let u3 = u2 * u;
    (2.0 * u3 - 3.0 * u2 + 1.0) * a + (u3 - 2.0 * u2 + u) * ma + (-2.0 * u3 + 3.0 * u2) * b + (u3 - u2) * mb
}

/// The point t of the way along the Hermite span by arc length, so points at even t sit evenly along it.
fn at_length(a: DVec3, ma: DVec3, b: DVec3, mb: DVec3, t: f64) -> DVec3 {
    const N: usize = 64;
    let mut len = [0.0; N + 1];
    let mut prev = a;
    for k in 1..=N {
        let q = hermite(a, ma, b, mb, k as f64 / N as f64);
        len[k] = len[k - 1] + prev.distance(q);
        prev = q;
    }
    if len[N] < 1e-12 {
        return mix3(a, b, t);
    }
    let want = t * len[N];
    let k = len.partition_point(|&x| x < want).clamp(1, N);
    let span = len[k] - len[k - 1];
    let u = ((k - 1) as f64 + if span > 1e-15 { (want - len[k - 1]) / span } else { 0.0 }) / N as f64;
    hermite(a, ma, b, mb, u)
}

struct Flow<'a> {
    o: &'a EdgeFlowOptions,
    normals: Vec<DVec3>,
}

impl Flow<'_> {
    /// Where line l puts its point: t of the way from a to b, on the curve through the points beyond, eased by
    /// amount.
    fn target(&self, pos: &[DVec3], l: &Across) -> DVec3 {
        let pa = pos[l.a as usize];
        let pb = pos[l.b as usize];
        let chord = pb - pa;
        let flat = pa + chord * l.t;
        let ease = self.o.amount * (1.0 - self.o.tension);
        // Arcs through the points beyond, blended toward the nearer end: a circle comes back exactly, evenly spaced.
        let from_a = (self.o.edge_flow && l.a2 != INVALID).then(|| arc_point(pos[l.a2 as usize], pa, pb, l.t));
        let from_b = (self.o.edge_flow && l.b2 != INVALID).then(|| arc_point(pos[l.b2 as usize], pb, pa, 1.0 - l.t));
        match (from_a, from_b) {
            (Some(fa), Some(fb)) => return flat + ease * ((1.0 - l.t) * fa + l.t * fb - flat),
            (Some(f), None) | (None, Some(f)) => return flat + ease * (f - flat),
            _ => {}
        }

        let mut ma = normal_tangent(self.normals[l.a as usize], chord);
        let mut mb = normal_tangent(self.normals[l.b as usize], chord);
        if ma.is_none() {
            if let Some(b) = mb {
                ma = Some((3.0 * chord - b) * 0.5);
            }
        }
        if mb.is_none() {
            if let Some(a) = ma {
                mb = Some((3.0 * chord - a) * 0.5);
            }
        }
        let on_curve = at_length(pa, ma.unwrap_or(chord), pb, mb.unwrap_or(chord), l.t);
        flat + ease * (on_curve - flat)
    }

    /// The move for a point on these lines. A loose point's lines each place it along themselves; what they say off
    /// their own direction is averaged, so a bend both see is not taken twice.
    fn displacement(&self, pos: &[DVec3], ls: &[Across], v: DVec3) -> DVec3 {
        if ls.len() == 1 {
            return self.target(pos, &ls[0]) - v;
        }
        let mut along = DVec3::ZERO;
        let mut off = DVec3::ZERO;
        for l in ls {
            let d = self.target(pos, l) - v;
            let chord = pos[l.b as usize] - pos[l.a as usize];
            let len = chord.length();
            let a = if len > 1e-12 { chord / len * d.dot(chord / len) } else { DVec3::ZERO };
            along += a;
            off += d - a;
        }
        along + off / ls.len() as f64
    }
}

/// Eases the moved points of a chain toward the four-point curve through their loop neighbours, square to the loop.
fn smooth_along(path: &EdgePath, movable: &[u8], amount: f64, pos: &mut [DVec3]) {
    let n = path.points.len() as i64;
    let mut next: Vec<(Index, DVec3)> = Vec::new();
    {
        let at = |i: i64| pos[path.points[((i % n + n) % n) as usize] as usize];
        for i in 0..n as usize {
            let p = path.points[i];
            if movable[p as usize] == 0 || (!path.closed && (i == 0 || i + 1 == n as usize)) {
                continue;
            }
            let k = i as i64;
            let wide = if path.closed { n >= 5 } else { i >= 2 && (i as i64) + 2 < n };
            let target = if wide {
                (9.0 * (at(k - 1) + at(k + 1)) - (at(k - 2) + at(k + 2))) / 16.0
            } else {
                (at(k - 1) + at(k + 1)) * 0.5
            };
            let mut d = target - pos[p as usize];
            let t = at(k + 1) - at(k - 1);
            let len = t.length();
            if len > 1e-12 {
                d -= t / len * d.dot(t / len);
            }
            next.push((p, pos[p as usize] + amount * d));
        }
    }
    for (p, q) in next {
        pos[p as usize] = q;
    }
}

/// Edit Edge Flow: each point of the edges' chains moves onto a curve through the nearest unselected points across
/// its loop: the blend of the two circular arcs through them and each point beyond, which gives back any circle
/// exactly, so the loop follows the surrounding curvature. Selected points between two anchors spread evenly by edge
/// count, as subdividing would place them. Points given loose (on no chain) are placed by both directions across them.
/// Points with nothing across on one side stay. Returns a move for every point considered, unmoved ones included.
pub fn edit_edge_flow(mesh: &Mesh, edges: &[Index], points: &[Index], opts: &EdgeFlowOptions) -> PointMoves {
    let valid: Vec<Index> = edges.iter().copied().filter(|&e| mesh.edge_valid(e)).collect();
    let chains = chain_edges(mesh, &valid);
    let mut considered: Vec<Index> = Vec::new();
    for path in &chains {
        considered.extend_from_slice(&path.points);
    }
    considered.extend(points.iter().copied().filter(|&p| mesh.point_valid(p)));
    let considered = unique_sorted(&considered);
    let sel = make_mask(mesh.point_capacity() as usize, &considered);

    let mut lines: BTreeMap<Index, Vec<Across>> = BTreeMap::new();
    for path in &chains {
        chain_lines(mesh, path, &sel, &mut lines);
    }
    for &p in points {
        if mesh.point_valid(p) {
            loose_lines(mesh, p, &sel, &mut lines);
        }
    }

    let mut normals = vec![DVec3::ZERO; mesh.point_capacity() as usize];
    for f in 0..mesh.face_capacity() {
        if mesh.face_valid(f) {
            let a = mesh.face_area_vector(f);
            mesh.for_each_face_corner(f, |c| normals[mesh.corner_point(c) as usize] += a);
        }
    }
    for n in &mut normals {
        let len = n.length();
        if len > 1e-12 {
            *n /= len;
        }
    }
    let flow = Flow { o: opts, normals };

    let mut pos: Vec<DVec3> = mesh.positions().to_vec();
    let mut movable = vec![0u8; mesh.point_capacity() as usize];
    for (&p, ls) in &lines {
        movable[p as usize] = (!ls.is_empty()) as u8;
    }
    for _ in 0..opts.iterations.max(1) {
        let next: Vec<(Index, DVec3)> = lines
            .iter()
            .filter(|(_, ls)| !ls.is_empty())
            .map(|(&p, ls)| (p, pos[p as usize] + flow.displacement(&pos, ls, pos[p as usize])))
            .collect();
        for (p, q) in next {
            pos[p as usize] = q;
        }
        if opts.loop_smooth != 0.0 {
            for path in &chains {
                smooth_along(path, &movable, opts.loop_smooth, &mut pos);
            }
        }
    }

    considered.iter().map(|&p| PointMove { point: p, position: pos[p as usize] }).collect()
}

/// Splits a point selection into the edges of its chains (edges between selected points that each have at most two
/// selected neighbours) and loose points (the rest), the inputs edit_edge_flow takes.
pub fn flow_selection_of_points(mesh: &Mesh, points: &[Index]) -> (Vec<Index>, Vec<Index>) {
    let mut edges = Vec::new();
    let mut loose = Vec::new();
    let inside = make_mask(mesh.point_capacity() as usize, points);
    let mut degree = vec![0i32; mesh.point_capacity() as usize];
    let mut between = Vec::new();
    let sorted = unique_sorted(points);
    for &p in &sorted {
        if mesh.point_valid(p) {
            mesh.for_each_point_edge(p, |e| {
                let q = mesh.edge_other_point(e, p);
                if inside[q as usize] == 0 {
                    return;
                }
                degree[p as usize] += 1;
                if p < q {
                    between.push(e);
                }
            });
        }
    }
    let mut on_chain = vec![0u8; mesh.point_capacity() as usize];
    for e in between {
        let p = mesh.edge_point(e, 0);
        let q = mesh.edge_point(e, 1);
        if degree[p as usize] <= 2 && degree[q as usize] <= 2 {
            edges.push(e);
            on_chain[p as usize] = 1;
            on_chain[q as usize] = 1;
        }
    }
    for p in sorted {
        if mesh.point_valid(p) && on_chain[p as usize] == 0 {
            loose.push(p);
        }
    }
    (edges, loose)
}
