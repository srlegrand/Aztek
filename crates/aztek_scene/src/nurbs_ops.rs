//! The rest of the NURBS commands: fillets, bevels, intersections, projection and curves on surfaces, trim and untrim,
//! stitching, raw surface creation, arcs and text. Each has its history node, one kind of node computing with a plain
//! function. Trims live on the surface (see aztek_geo's TrimRegion).
//!
//! Differs from the reference: surface-surface intersection is sampled, a polyline fit through it, accurate to the tessellation.
//! Bevels are one surface with the front, side and back as its spans; bevelPlus caps need -po 1 or closed curves (the caps
//! are trimmed surfaces). circularFillet needs its surfaces to come within reach of each other's offsets. nurbsBoolean
//! returns a polygon mesh: there are no trimmed NURBS solids here. roundConstantRadius isn't here: it needs NURBS solids to
//! trim back. stitchSurface works on boundaries only and copies its partner when run, rather than following it. offsetCurveOnSurface
//! offsets in parameter space.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::nurbs::{self, Form, NurbsCurve, NurbsSurface, TrimRegion};
use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::{DVec2, DVec3};
use serde_json::{json, Value};

use crate::curves::{self, curve_attr, curve_value, f, f_in, int_in, v_in, CurveShape};
use crate::graph::{Graph, SelectMode};
use crate::ref_core::asked;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::surfaces::{self, surface_attr, surface_value, SurfaceShape};
use crate::system_cmds::q_flags;
use crate::transform::Transform;

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Float as Fl, Int as I, String as S};

const STORE: u32 = flags::INPUT | flags::STORABLE;

/// A node computing with a plain function. Its attributes come in order: inputs, outputs, settings.
struct FnNode(fn(&mut NodeData));

impl NodeBehavior for FnNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        (self.0)(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn node_type(name: &str, doc: &str, attrs: Vec<AttrDef>, run: fn(&mut NodeData)) -> NodeType {
    NodeType { name: name.into(), icon: "chart-line".into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(move || Box::new(FnNode(run)))), ..Default::default() }
}

fn cin(name: &str, short: &str) -> AttrDef {
    curve_attr(name, short, STORE | flags::HIDDEN, "A curve.")
}
fn cout(name: &str, short: &str) -> AttrDef {
    curve_attr(name, short, flags::OUTPUT | flags::HIDDEN, "The result.")
}
fn sin(name: &str, short: &str) -> AttrDef {
    surface_attr(name, short, STORE | flags::HIDDEN, "A surface.")
}
fn sout(name: &str, short: &str) -> AttrDef {
    surface_attr(name, short, flags::OUTPUT | flags::HIDDEN, "The result.")
}
fn bin(name: &str, short: &str, def: bool) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Bool, default_value: PlugValue::Bool(def), flags: STORE, doc: "A setting.".into(), ..Default::default() }
}
fn data_in(name: &str, short: &str) -> AttrDef {
    crate::rig::data_attr(name, short, "Worked out when the command ran.")
}

fn crv(d: &NodeData, i: usize) -> Option<NurbsCurve> {
    curve_value(d.value(i)).cloned().filter(NurbsCurve::is_valid)
}
fn srf(d: &NodeData, i: usize) -> Option<NurbsSurface> {
    surface_value(d.value(i)).cloned().filter(NurbsSurface::is_valid)
}
fn put_curve(d: &mut NodeData, i: usize, c: Option<NurbsCurve>) {
    d.set_output(i, c.map_or(PlugValue::Data(None), curves::to_value));
}
fn put_surface(d: &mut NodeData, i: usize, s: Option<NurbsSurface>) {
    d.set_output(i, s.map_or(PlugValue::Data(None), surfaces::to_value));
}

// ---------------------------------------------------------------------------------------------------------------------
// Curve fillets

/// A round fillet of a radius between two curves, with the parameters it leaves each at.
fn fillet_arc(c1: &NurbsCurve, c2: &NurbsCurve, radius: f64, cp1: Option<f64>, cp2: Option<f64>) -> Option<(NurbsCurve, f64, f64)> {
    // Where they meet, or come closest.
    let (ulo, uhi) = c1.domain();
    let n = 64 * c1.spans().max(1);
    let (mut bu, mut bv, mut best) = (ulo, c2.domain().0, f64::MAX);
    for k in 0..=n {
        let u = ulo + (uhi - ulo) * k as f64 / n as f64;
        let p = c1.eval(u);
        let v = c2.closest_param(p);
        let dist = p.distance(c2.eval(v));
        if dist < best {
            (bu, bv, best) = (u, v, dist);
        }
    }
    let plane = {
        let (a, b) = (c1.tangent(bu), c2.tangent(bv));
        let n = a.cross(b);
        if n.length_squared() > 1e-16 {
            n.normalize()
        } else {
            c1.plane_normal()
        }
    };
    // Which way each curve runs from the meeting point: the side the parameter asks for, or the longer one.
    let (vlo, vhi) = c2.domain();
    let side_u = cp1.map_or(if uhi - bu >= bu - ulo { 1.0 } else { -1.0 }, |c| (c - bu).signum());
    let side_v = cp2.map_or(if vhi - bv >= bv - vlo { 1.0 } else { -1.0 }, |c| (c - bv).signum());
    let (t1, t2) = (c1.tangent(bu).normalize_or_zero() * side_u, c2.tangent(bv).normalize_or_zero() * side_v);
    let inward = (t1 + t2).normalize_or_zero();
    if inward == DVec3::ZERO {
        return None;
    }
    // Walk along the first curve for the place its offset point is radius from the second.
    let centre_at = |u: f64| -> DVec3 {
        let t = c1.tangent(u).normalize_or_zero();
        let mut nn = plane.cross(t).normalize_or_zero();
        if nn.dot(inward) < 0.0 {
            nn = -nn;
        }
        c1.eval(u) + nn * radius
    };
    let gap = |u: f64| -> f64 {
        let c = centre_at(u);
        let q = c2.eval(c2.closest_param(c));
        c.distance(q) - radius
    };
    let end = if side_u > 0.0 { uhi } else { ulo };
    let steps = 400;
    let (mut a, mut fa) = (bu, gap(bu));
    let mut hit = None;
    for k in 1..=steps {
        let u = bu + (end - bu) * k as f64 / steps as f64;
        let fu = gap(u);
        if fa.signum() != fu.signum() {
            hit = Some((a, u));
            break;
        }
        (a, fa) = (u, fu);
    }
    let (mut lo, mut hi) = hit?;
    let flo = gap(lo);
    for _ in 0..50 {
        let mid = (lo + hi) * 0.5;
        if gap(mid).signum() == flo.signum() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    let u = (lo + hi) * 0.5;
    let c = centre_at(u);
    let v = c2.closest_param(c);
    let (p1, p2) = (c1.eval(u), c2.eval(v));
    let arc = three_point_arc(p1, c + ((p1 - c).normalize_or_zero() + (p2 - c).normalize_or_zero()).normalize_or_zero() * radius, p2, 3, 8)?;
    Some((arc, u, v))
}

fn fillet_freeform(c1: &NurbsCurve, c2: &NurbsCurve, depth: f64, bias: f64, cp1: Option<f64>, cp2: Option<f64>) -> Option<NurbsCurve> {
    let u = cp1.unwrap_or_else(|| c1.domain().1);
    let v = cp2.unwrap_or_else(|| c2.domain().0);
    let (p0, p1) = (c1.eval(u), c2.eval(v));
    let (lo, hi) = c2.domain();
    let (t0, t1) = (c1.tangent(u).normalize_or_zero(), c2.tangent(v).normalize_or_zero() * if v <= (lo + hi) * 0.5 { 1.0 } else { -1.0 });
    let len = p0.distance(p1);
    let (k0, k1) = (depth * len * (1.0 + bias) / 3.0, depth * len * (1.0 - bias) / 3.0);
    Some(NurbsCurve::open(3, vec![p0, p0 + t0 * k0, p1 - t1 * k1, p1]))
}

/// A circular arc through three points. None when they lie on a line.
fn three_point_arc(a: DVec3, b: DVec3, c: DVec3, degree: usize, sections: usize) -> Option<NurbsCurve> {
    let (centre, r, axis) = arc_centre(a, b, c)?;
    let (x, y) = ((a - centre) / r, axis.cross((a - centre) / r));
    let turn = |p: DVec3| (p - centre).dot(y).atan2((p - centre).dot(x)).rem_euclid(std::f64::consts::TAU);
    Some(nurbs::circle(centre, axis, x, r, degree, sections.max(1), turn(c).to_degrees()))
}

fn arc_centre(a: DVec3, b: DVec3, c: DVec3) -> Option<(DVec3, f64, DVec3)> {
    let (ab, ac) = (b - a, c - a);
    let n = ab.cross(ac);
    if n.length_squared() < 1e-18 {
        return None;
    }
    let centre = a + n.cross(ab * ac.length_squared() - ac * ab.length_squared()) / (2.0 * n.length_squared());
    Some((centre, centre.distance(a), n.normalize()))
}

// ---------------------------------------------------------------------------------------------------------------------
// Curve-curve and surface-surface intersections

fn curve_intersections(a: &NurbsCurve, b: &NurbsCurve, tol: f64, dir: Option<DVec3>) -> Vec<(f64, f64)> {
    let flat = |p: DVec3| dir.map_or(p, |d| p - d * p.dot(d));
    let params = |c: &NurbsCurve| -> Vec<f64> {
        let (lo, hi) = c.domain();
        let n = 64 * c.spans().max(1);
        (0..=n).map(|k| lo + (hi - lo) * k as f64 / n as f64).collect()
    };
    let (pa, pb) = (params(a), params(b));
    let qa: Vec<DVec3> = pa.iter().map(|&u| flat(a.eval(u))).collect();
    let qb: Vec<DVec3> = pb.iter().map(|&u| flat(b.eval(u))).collect();
    let mut hits: Vec<(f64, f64)> = Vec::new();
    for i in 0..qa.len() - 1 {
        for j in 0..qb.len() - 1 {
            let (p1, d1, p2, d2) = (qa[i], qa[i + 1] - qa[i], qb[j], qb[j + 1] - qb[j]);
            let r = p1 - p2;
            let (aa, bb, cc, dd, ee) = (d1.dot(d1), d1.dot(d2), d2.dot(d2), d1.dot(r), d2.dot(r));
            let den = aa * cc - bb * bb;
            if den < 1e-18 {
                continue;
            }
            let (s, t) = (((bb * ee - cc * dd) / den).clamp(0.0, 1.0), ((aa * ee - bb * dd) / den).clamp(0.0, 1.0));
            if (p1 + d1 * s).distance(p2 + d2 * t) > tol.max(1e-6) {
                continue;
            }
            // Finish on the true curves.
            let (mut u, mut v) = (pa[i] + (pa[i + 1] - pa[i]) * s, pb[j] + (pb[j + 1] - pb[j]) * t);
            for _ in 0..4 {
                v = b.closest_param(a.eval(u));
                u = a.closest_param(b.eval(v));
            }
            if !hits.iter().any(|h| (h.0 - u).abs() < 1e-3 && (h.1 - v).abs() < 1e-3) {
                hits.push((u, v));
            }
        }
    }
    hits.sort_by(|x, y| x.0.total_cmp(&y.0));
    hits
}

fn tri_plane_segment(tri: [DVec3; 3], o: DVec3, n: DVec3) -> Option<(DVec3, DVec3)> {
    let d = tri.map(|p| (p - o).dot(n));
    let mut out = Vec::new();
    for k in 0..3 {
        let (a, b) = (d[k], d[(k + 1) % 3]);
        if (a > 0.0) != (b > 0.0) {
            out.push(tri[k].lerp(tri[(k + 1) % 3], a / (a - b)));
        }
    }
    (out.len() == 2).then(|| (out[0], out[1]))
}

/// Polylines where two meshes cross: triangle pairs' segments chained end to end.
fn mesh_intersection(a: &Mesh, b: &Mesh) -> Vec<Vec<DVec3>> {
    let tris = |m: &Mesh| -> Vec<[DVec3; 3]> {
        let mut out = Vec::new();
        for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f)) {
            let p = m.face_points(f);
            for k in 1..p.len().saturating_sub(1) {
                out.push([m.position(p[0]), m.position(p[k]), m.position(p[k + 1])]);
            }
        }
        out
    };
    let (ta, tb) = (tris(a), tris(b));
    let mut segs: Vec<(DVec3, DVec3)> = Vec::new();
    for p in &ta {
        let lo = p[0].min(p[1]).min(p[2]);
        let hi = p[0].max(p[1]).max(p[2]);
        let np = (p[1] - p[0]).cross(p[2] - p[0]);
        if np.length_squared() < 1e-24 {
            continue;
        }
        for q in &tb {
            let (qlo, qhi) = (q[0].min(q[1]).min(q[2]), q[0].max(q[1]).max(q[2]));
            if qhi.x < lo.x || qlo.x > hi.x || qhi.y < lo.y || qlo.y > hi.y || qhi.z < lo.z || qlo.z > hi.z {
                continue;
            }
            let nq = (q[1] - q[0]).cross(q[2] - q[0]);
            if nq.length_squared() < 1e-24 {
                continue;
            }
            // Each triangle cut by the other's plane; the overlap of the two cuts along the common line.
            let (Some(s1), Some(s2)) = (tri_plane_segment(*p, q[0], nq.normalize()), tri_plane_segment(*q, p[0], np.normalize())) else { continue };
            let dir = np.cross(nq).normalize_or_zero();
            if dir == DVec3::ZERO {
                continue;
            }
            let span = |s: (DVec3, DVec3)| {
                let (x, y) = (s.0.dot(dir), s.1.dot(dir));
                if x <= y { (x, y, s.0, s.1) } else { (y, x, s.1, s.0) }
            };
            let (a1, b1, pa1, pb1) = span(s1);
            let (a2, b2, pa2, pb2) = span(s2);
            let (start, end) = (a1.max(a2), b1.min(b2));
            if end - start <= 1e-9 {
                continue;
            }
            segs.push((if a1 > a2 { pa1 } else { pa2 }, if b1 < b2 { pb1 } else { pb2 }));
        }
    }
    chain_segments(segs)
}

fn chain_segments(mut segs: Vec<(DVec3, DVec3)>) -> Vec<Vec<DVec3>> {
    let near = |a: DVec3, b: DVec3| a.distance_squared(b) < 1e-10;
    let mut out = Vec::new();
    while let Some((a, b)) = segs.pop() {
        let mut line = vec![a, b];
        loop {
            let tail = *line.last().unwrap();
            let head = line[0];
            if let Some(i) = segs.iter().position(|s| near(s.0, tail) || near(s.1, tail)) {
                let s = segs.swap_remove(i);
                line.push(if near(s.0, tail) { s.1 } else { s.0 });
            } else if let Some(i) = segs.iter().position(|s| near(s.0, head) || near(s.1, head)) {
                let s = segs.swap_remove(i);
                line.insert(0, if near(s.0, head) { s.1 } else { s.0 });
            } else {
                break;
            }
        }
        out.push(line);
    }
    out.sort_by_key(|l: &Vec<DVec3>| std::cmp::Reverse(l.len()));
    out
}

/// A degree 1 curve through a run of points, thinned where they crowd.
fn polyline_curve(pts: &[DVec3]) -> NurbsCurve {
    let mut keep: Vec<DVec3> = Vec::with_capacity(pts.len());
    for &p in pts {
        if keep.last().is_none_or(|l| l.distance(p) > 1e-6) {
            keep.push(p);
        }
    }
    if keep.len() < 2 {
        keep.push(keep[0] + DVec3::X * 1e-6);
    }
    NurbsCurve::open(1, keep)
}

fn surface_mesh(s: &NurbsSurface, per: usize) -> Mesh {
    let mut u = s.clone();
    u.trim.clear();
    u.tessellate(per, per)
}

// ---------------------------------------------------------------------------------------------------------------------
// Nodes

// Attribute slots shared by most nodes here: 0 first input, 1 output, settings after.
const IN: usize = 0;
const OUT: usize = 1;
const P: usize = 2;

/// projectCurve: the curve's samples along a direction onto the surface, as a curve in (u, v).
fn project_samples(c: &NurbsCurve, s: &NurbsSurface, dir: Option<DVec3>, samples: usize) -> Vec<DVec2> {
    let m = surface_mesh(s, 16);
    let bvh = MeshBvh::new(&m);
    let (lo, hi) = c.domain();
    let n = samples.max(2);
    let mut out: Vec<DVec2> = Vec::new();
    for k in 0..=n {
        let p = c.eval(lo + (hi - lo) * k as f64 / n as f64);
        let hit = match dir {
            Some(d) => {
                let d = d.normalize_or_zero();
                let (a, b) = (bvh.raycast_with(p, d, f64::MAX, true), bvh.raycast_with(p, -d, f64::MAX, true));
                match (a, b) {
                    (Some(x), Some(y)) => Some(if x.t <= y.t { p + d * x.t } else { p - d * y.t }),
                    (Some(x), None) => Some(p + d * x.t),
                    (None, Some(y)) => Some(p - d * y.t),
                    (None, None) => None,
                }
            }
            // Along the surface's normal is to the closest point.
            None => Some(p),
        };
        let Some(q) = hit else { continue };
        let (u, v) = s.closest_point(q);
        let at = DVec2::new(u, v);
        if out.last().is_none_or(|l| l.distance(at) > 1e-7) {
            out.push(at);
        }
    }
    out
}

fn uv_curve(pts: &[DVec2]) -> NurbsCurve {
    let p: Vec<DVec3> = pts.iter().map(|q| DVec3::new(q.x, q.y, 0.0)).collect();
    polyline_curve(&p)
}

fn tangent_blend(a: &NurbsCurve, ta: &[DVec3], b: &NurbsCurve, tb: &[DVec3], depth: f64, bias: f64, samples: usize) -> Option<NurbsSurface> {
    let n = samples.max(2);
    let (alo, ahi) = a.domain();
    let (blo, bhi) = b.domain();
    let mut cols = Vec::with_capacity(n + 1);
    for k in 0..=n {
        let t = k as f64 / n as f64;
        let (p, q) = (a.eval(alo + (ahi - alo) * t), b.eval(blo + (bhi - blo) * t));
        let at = |l: &[DVec3]| l.get((t * (l.len().max(1) - 1) as f64).round() as usize).copied().unwrap_or(DVec3::ZERO);
        let len = p.distance(q);
        let (k0, k1) = (depth * len * (1.0 + bias) / 3.0, depth * len * (1.0 - bias) / 3.0);
        cols.push(NurbsCurve::open(3, vec![p, p + at(ta).normalize_or_zero() * k0, q - at(tb).normalize_or_zero() * k1, q]));
    }
    let v_knots = nurbs::open_knots(n + 1, 1);
    let s = NurbsSurface::from_columns(&cols, 1, Form::Open, v_knots);
    Some(s)
}

/// The direction a surface leaves its edge at a place along it.
fn out_of_edge(s: &NurbsSurface, dir: i64, value: f64, along: f64) -> DVec3 {
    let ((ulo, uhi), (vlo, vhi)) = (s.domain_u(), s.domain_v());
    let (u, v) = if dir == 0 { (value, along) } else { (along, value) };
    let (_, du, dv) = s.eval_ders(u, v);
    let (t0, lo, hi) = if dir == 0 { (du, ulo, uhi) } else { (dv, vlo, vhi) };
    if (value - lo).abs() < (hi - value).abs() {
        -t0
    } else {
        t0
    }
}

fn along_domain(s: &NurbsSurface, dir: i64) -> (f64, f64) {
    if dir == 0 {
        s.domain_v()
    } else {
        s.domain_u()
    }
}

/// One side of a blend: its curve, and the surface and edge it leaves when it is an edge.
struct BlendRail {
    curve: NurbsCurve,
    edge: Option<(NurbsSurface, i64, f64)>,
    start: f64,
    end: f64,
    anchor: f64,
}

impl BlendRail {
    fn fraction(&self, t: f64) -> f64 {
        let x = self.start + (self.end - self.start) * t + self.anchor;
        if self.curve.form == Form::Open {
            x.clamp(0.0, 1.0)
        } else {
            x.rem_euclid(1.0)
        }
    }
    fn at(&self, t: f64) -> DVec3 {
        let (lo, hi) = self.curve.domain();
        self.curve.eval(lo + (hi - lo) * self.fraction(t))
    }
    /// The way the blend leaves this rail, or None for a bare curve.
    fn leaving(&self, t: f64) -> Option<DVec3> {
        let (s, dir, value) = self.edge.as_ref()?;
        let (lo, hi) = along_domain(s, *dir);
        Some(out_of_edge(s, *dir, *value, lo + (hi - lo) * self.fraction(t)).normalize_or_zero())
    }
}

/// ffBlendSrf: cubics between two rails, each leaving along its surface when it is an edge, straight across when a curve.
fn blend_rails(d: &NodeData) -> Option<NurbsSurface> {
    let side = |cin: usize, rail: usize, dir: usize, par: usize, start: usize, end: usize, anchor: usize| -> Option<BlendRail> {
        let (curve, edge) = match srf(d, rail) {
            Some(s) => {
                let (di, value) = (d.int(dir), d.float(par));
                let iso = if di == 0 { s.iso_u(value) } else { s.iso_v(value) };
                (iso, Some((s, di, value)))
            }
            None => (crv(d, cin)?, None),
        };
        Some(BlendRail { curve, edge, start: d.float(start), end: d.float(end), anchor: d.float(anchor) })
    };
    let left = side(IN, P + 1, P + 3, P + 4, P + 7, P + 8, P + 11)?;
    let mut right = side(P, P + 2, P + 5, P + 6, P + 9, P + 10, P + 12)?;
    const N: usize = 32;
    if d.bool(P + 13) && left.curve.form != Form::Open && right.curve.form != Form::Open {
        // Both closed: start the right where the two line up best.
        let cost = |r: &BlendRail| (0..=N).map(|k| left.at(k as f64 / N as f64).distance(r.at(k as f64 / N as f64))).sum::<f64>();
        let mut best = (f64::MAX, 0.0);
        for k in 0..64 {
            right.anchor = k as f64 / 64.0;
            let c = cost(&right);
            if c < best.0 {
                best = (c, right.anchor);
            }
        }
        right.anchor = best.1;
    }
    let auto = d.bool(P + 14);
    let sign = |flip: usize, rev: usize| {
        let mut x = 1.0;
        if d.bool(flip) {
            x = -x;
        }
        if !auto && d.bool(rev) {
            x = -x;
        }
        x
    };
    let (left_sign, right_sign) = (sign(P + 15, P + 17), sign(P + 16, P + 18));
    let mut cols = Vec::with_capacity(N + 1);
    for k in 0..=N {
        let t = k as f64 / N as f64;
        let (p, q) = (left.at(t), right.at(t));
        let across = (q - p).normalize_or_zero();
        let out = left.leaving(t).unwrap_or(across) * left_sign;
        let back = right.leaving(t).unwrap_or(-across) * right_sign;
        let reach = p.distance(q) / 3.0;
        cols.push(NurbsCurve::open(3, vec![p, p + out * reach, q + back * reach, q]));
    }
    Some(NurbsSurface::from_columns(&cols, 1, Form::Open, nurbs::open_knots(N + 1, 1)))
}

/// A constant radius fillet along where two surface edges meet. Each section is worked out as if the surfaces were flat at
/// that place, which is exact for planes and close when they curve gently.
fn round_fillet(a: &NurbsSurface, da: i64, pa: f64, b: &NurbsSurface, db: i64, pb: f64, radius: f64) -> Option<NurbsSurface> {
    const N: usize = 16;
    let (iso_a, iso_b) = (if da == 0 { a.iso_u(pa) } else { a.iso_v(pa) }, if db == 0 { b.iso_u(pb) } else { b.iso_v(pb) });
    let (lo, hi) = iso_a.domain();
    let (along_a, along_b) = (along_domain(a, da), along_domain(b, db));
    let (blo, bhi) = iso_b.domain();
    let mut sections = Vec::with_capacity(N + 1);
    for k in 0..=N {
        let u = lo + (hi - lo) * k as f64 / N as f64;
        let p = iso_a.eval(u);
        let w = iso_b.closest_param(p);
        let at_a = along_a.0 + (along_a.1 - along_a.0) * (u - lo) / (hi - lo).max(1e-12);
        let at_b = along_b.0 + (along_b.1 - along_b.0) * (w - blo) / (bhi - blo).max(1e-12);
        // Into each surface from the edge.
        let ua = -out_of_edge(a, da, pa, at_a).normalize_or_zero();
        let ub = -out_of_edge(b, db, pb, at_b).normalize_or_zero();
        let half = ua.angle_between(ub) * 0.5;
        if half < 1e-3 || half > std::f64::consts::FRAC_PI_2 - 1e-3 {
            return None;
        }
        let mid = (ua + ub).normalize_or_zero();
        let from_edge = radius / half.tan();
        let centre = p + mid * (radius / half.sin());
        sections.push(three_point_arc(p + ua * from_edge, centre - mid * radius, p + ub * from_edge, 3, 4)?);
    }
    loft_of(&sections)
}

/// The boundary of a surface an isoparm flag names: (curve, tangents across, pointing into the gap).
fn across_tangents(s: &NurbsSurface, dir: i64, value: f64, samples: usize) -> (NurbsCurve, Vec<DVec3>) {
    let iso = if dir == 0 { s.iso_u(value) } else { s.iso_v(value) };
    let ((ulo, uhi), (vlo, vhi)) = (s.domain_u(), s.domain_v());
    let n = samples.max(2);
    let list = (0..=n)
        .map(|k| {
            let t = k as f64 / n as f64;
            let (u, v) = if dir == 0 { (value, vlo + (vhi - vlo) * t) } else { (ulo + (uhi - ulo) * t, value) };
            let (_, du, dv) = s.eval_ders(u, v);
            let (t0, lo, hi) = if dir == 0 { (du, ulo, uhi) } else { (dv, vlo, vhi) };
            // Out of the surface at its edge; along itself anywhere else.
            if (value - lo).abs() < (hi - value).abs() { -t0 } else { t0 }
        })
        .collect();
    (iso, list)
}

/// Offsets a curve sideways in its plane, sampling. Keeps the curve's own run.
fn plane_offset(c: &NurbsCurve, distance: f64) -> NurbsCurve {
    if distance.abs() < 1e-12 {
        return c.clone();
    }
    c.offset(distance, None)
}

/// bevel's sections: (offset out in the plane, along the normal) from the front round to the back.
fn bevel_sections(c: &NurbsCurve, width: f64, depth: f64, extrude: f64, back: bool) -> Vec<NurbsCurve> {
    let n = c.plane_normal();
    let up = if n == DVec3::ZERO { DVec3::Z } else { n };
    let mut prof: Vec<(f64, f64)> = vec![(0.0, 0.0), (width, -depth), (width, -depth - extrude)];
    if back {
        prof.push((0.0, -2.0 * depth - extrude));
    }
    prof.iter().map(|&(o, z)| plane_offset(c, o).transformed(&glam::DMat4::from_translation(up * z))).collect()
}

fn loft_of(list: &[NurbsCurve]) -> Option<NurbsSurface> {
    surfaces::loft_surface(list, surfaces::LoftOptions { close: false, uniform: true, degree: 1, section_spans: 1, auto_reverse: false, reverse_normals: false })
}

fn round_sections(c: &NurbsCurve, width: f64, depth: f64, extrude: f64, sides: usize, back: bool) -> Vec<NurbsCurve> {
    let n = c.plane_normal();
    let up = if n == DVec3::ZERO { DVec3::Z } else { n };
    let mut prof: Vec<(f64, f64)> = Vec::new();
    let steps = sides.max(1);
    for k in 0..=steps {
        let t = k as f64 / steps as f64 * std::f64::consts::FRAC_PI_2;
        prof.push((width * t.sin(), -depth * (1.0 - t.cos())));
    }
    prof.push((width, -depth - extrude));
    if back {
        for k in (0..steps).rev() {
            let t = k as f64 / steps as f64 * std::f64::consts::FRAC_PI_2;
            prof.push((width * t.sin(), -depth - extrude - depth * (1.0 - t.cos())));
        }
    }
    prof.iter().map(|&(o, z)| plane_offset(c, o).transformed(&glam::DMat4::from_translation(up * z))).collect()
}

fn closed_outline(c: &NurbsCurve) -> bool {
    let (lo, hi) = c.domain();
    c.form != Form::Open || c.eval(lo).distance(c.eval(hi)) < 1e-6
}

/// A plane patch round a closed curve with the curve as its trim.
pub(crate) fn trimmed_plane(c: &NurbsCurve) -> Option<NurbsSurface> {
    // An open curve closes straight across.
    let pts = c.tessellate(16);
    let n = c.plane_normal();
    if n == DVec3::ZERO {
        return None;
    }
    let (x, y) = {
        let t = (pts[1] - pts[0]).normalize_or_zero();
        let t = if t == DVec3::ZERO { n.any_orthonormal_vector() } else { t };
        (t, n.cross(t).normalize_or_zero())
    };
    let o = pts[0];
    let flat: Vec<DVec2> = pts.iter().map(|p| DVec2::new((*p - o).dot(x), (*p - o).dot(y))).collect();
    let lo = flat.iter().fold(DVec2::splat(f64::MAX), |a, b| a.min(*b));
    let hi = flat.iter().fold(DVec2::splat(f64::MIN), |a, b| a.max(*b));
    let size = (hi - lo).max(DVec2::splat(1e-6));
    let corner = o + x * lo.x + y * lo.y;
    let mut s = nurbs::patch_surface(corner, x * size.x, y * size.y, 1, 1, 1);
    let (ul, uh) = s.domain_u();
    let (vl, vh) = s.domain_v();
    let mut loop_uv: Vec<DVec2> = flat.iter().map(|p| DVec2::new(ul + (p.x - lo.x) / size.x * (uh - ul), vl + (p.y - lo.y) / size.y * (vh - vl))).collect();
    if loop_uv.len() > 1 && loop_uv[0].distance(*loop_uv.last().unwrap()) < 1e-9 {
        loop_uv.pop();
    }
    s.trim = vec![TrimRegion { loops: vec![loop_uv], keep_odd: true }];
    Some(s)
}

/// circularFillet's sections: where the offsets of the surfaces meet is the arc's centre.
fn circular_fillet(a: &NurbsSurface, b: &NurbsSurface, r1: f64, r2: f64) -> Option<NurbsSurface> {
    let (oa, ob) = (a.offset(r1), b.offset(r2));
    let lines = mesh_intersection(&surface_mesh(&oa, 16), &surface_mesh(&ob, 16));
    let spine = lines.first()?;
    if spine.len() < 2 {
        return None;
    }
    let step = (spine.len() / 16).max(1);
    let mut sections: Vec<NurbsCurve> = Vec::new();
    for (k, &c) in spine.iter().enumerate() {
        if k % step != 0 && k + 1 != spine.len() {
            continue;
        }
        let (ua, va) = a.closest_point(c);
        let (ub, vb) = b.closest_point(c);
        let (p, q) = (a.eval(ua, va), b.eval(ub, vb));
        let mid = c + ((p - c).normalize_or_zero() + (q - c).normalize_or_zero()).normalize_or_zero() * c.distance(p);
        sections.push(three_point_arc(p, mid, q, 3, 4)?);
    }
    if sections.len() < 2 {
        return None;
    }
    loft_of(&sections)
}

/// stitch: a boundary's CVs moved onto the target curve, by bias.
fn stitch_boundary(s: &NurbsSurface, dir: i64, value: f64, target: &NurbsCurve, bias: f64) -> NurbsSurface {
    let mut o = s.clone();
    let (nu, nv) = (s.nu, s.nv);
    let mut row: Vec<(usize, usize)> = Vec::new();
    if dir == 0 {
        let (lo, hi) = s.domain_u();
        let i = if (value - lo).abs() < (hi - value).abs() { 0 } else { nu - 1 };
        row.extend((0..nv).map(|j| (i, j)));
    } else {
        let (lo, hi) = s.domain_v();
        let j = if (value - lo).abs() < (hi - value).abs() { 0 } else { nv - 1 };
        row.extend((0..nu).map(|i| (i, j)));
    }
    for (i, j) in row {
        let p = s.cv(i, j);
        let q = target.eval(target.closest_param(p));
        o.cvs[i * nv + j] = p.lerp(q, bias);
    }
    o.rewrap();
    o
}

/// The boundary of a surface nearest a curve: the direction and parameter.
fn nearest_boundary(s: &NurbsSurface, other: &NurbsSurface) -> (i64, f64) {
    let ((ul, uh), (vl, vh)) = (s.domain_u(), s.domain_v());
    let mut best = (0, ul, f64::MAX);
    for (dir, value) in [(0, ul), (0, uh), (1, vl), (1, vh)] {
        let iso = if dir == 0 { s.iso_u(value) } else { s.iso_v(value) };
        let (lo, hi) = iso.domain();
        let mut sum = 0.0;
        for k in 0..=8 {
            let p = iso.eval(lo + (hi - lo) * k as f64 / 8.0);
            let (u, v) = other.closest_point(p);
            sum += p.distance(other.eval(u, v));
        }
        if sum < best.2 {
            best = (dir, value, sum);
        }
    }
    (best.0, best.1)
}

fn node_uv_curve(d: &NodeData, i: usize) -> Option<NurbsCurve> {
    crv(d, i)
}

fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let reg = surfaces::register_one;
        reg(node_type(
            "ffBlendSrf",
            "A surface between two rails: curves, or edges it leaves along their surfaces.",
            vec![
                cin("leftCurve", "lc"),
                sout("outputSurface", "os"),
                cin("rightCurve", "rc"),
                sin("leftRail", "lr"),
                sin("rightRail", "rr"),
                int_in("leftDirection", "ld", 0, "0 U, 1 V: which way the left edge's isoparm runs."),
                f_in("leftParameter", "lp", 0.0, "The left edge's parameter."),
                int_in("rightDirection", "rd", 0, "0 U, 1 V: which way the right edge's isoparm runs."),
                f_in("rightParameter", "rp", 0.0, "The right edge's parameter."),
                f_in("leftStart", "ls", 0.0, "Where the blend starts on the left, 0 to 1."),
                f_in("leftEnd", "le", 1.0, "Where the blend ends on the left, 0 to 1."),
                f_in("rightStart", "rs", 0.0, "Where the blend starts on the right, 0 to 1."),
                f_in("rightEnd", "re", 1.0, "Where the blend ends on the right, 0 to 1."),
                f_in("leftAnchor", "la", 0.0, "On a closed left rail: where it starts."),
                f_in("rightAnchor", "ra", 0.0, "On a closed right rail: where it starts."),
                bin("autoAnchor", "aa", true),
                bin("autoNormal", "an", true),
                bin("flipLeftNormal", "fln", false),
                bin("flipRightNormal", "frn", false),
                bin("reverseLeft", "rvl", false),
                bin("reverseRight", "rvr", false),
                bin("multipleKnots", "mk", true),
                f_in("positionTolerance", "pt", 0.1, "Held: the blend meets its rails exactly."),
                f_in("tangentTolerance", "tt", 0.1, "Held: no fitting to a tolerance is done."),
            ],
            |d| put_surface(d, OUT, blend_rails(d)),
        ));
        reg(node_type(
            "roundConstantRadius",
            "A constant radius fillet where two surface edges meet.",
            vec![
                sin("inputSurface", "is"),
                sout("filletSurface", "fs"),
                sin("inputSurfaceB", "isb"),
                int_in("direction1", "d1", 0, "0 U, 1 V."),
                f_in("parameter1", "p1", 0.0, "The first edge."),
                int_in("direction2", "d2", 0, "0 U, 1 V."),
                f_in("parameter2", "p2", 0.0, "The second edge."),
                f_in("radius", "r", 1.0, "The radius."),
                f_in("tolerance", "tol", 0.01, "Held: sections are worked out directly."),
            ],
            |d| {
                let (Some(a), Some(b)) = (srf(d, IN), srf(d, P)) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let out = round_fillet(&a, d.int(P + 1), d.float(P + 2), &b, d.int(P + 3), d.float(P + 4), d.float(P + 5));
                put_surface(d, OUT, out);
            },
        ));
        reg(node_type(
            "filletCurve",
            "A round or free-form fillet between two curves.",
            vec![cin("primaryCurve", "pc"), cout("outputCurve", "oc"), cin("secondaryCurve", "sc"), f_in("radius", "r", 1.0, "The circle's radius."), bin("circular", "cir", true), f_in("freeformBlend", "fb", 0.5, "A setting."), f_in("depth", "d", 0.5, "How far a free-form fillet reaches."), f_in("bias", "b", 0.0, "Which way it leans."), f_in("curveParameter1", "cp1", -1.0, "Where on the first curve, or -1."), f_in("curveParameter2", "cp2", -1.0, "Where on the second curve, or -1.")],
            |d| {
                let (Some(c1), Some(c2)) = (crv(d, IN), crv(d, P)) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let par = |x: f64| (x >= 0.0).then_some(x);
                let (cp1, cp2) = (par(d.float(P + 6)), par(d.float(P + 7)));
                let out = if d.bool(P + 2) { fillet_arc(&c1, &c2, d.float(P + 1), cp1, cp2).map(|x| x.0) } else { fillet_freeform(&c1, &c2, d.float(P + 4), d.float(P + 5), cp1, cp2) };
                put_curve(d, OUT, out);
            },
        ));
        reg(node_type(
            "curveFromSurfaceCoS",
            "A curve on a surface in 3D, from its (u, v) curve.",
            vec![sin("inputSurface", "is"), cout("outputCurve", "oc"), cin("curveOnSurface", "cos")],
            |d| {
                let (Some(s), Some(uv)) = (srf(d, IN), node_uv_curve(d, P)) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let pts = uv.tessellate(if uv.degree > 1 { 12 } else { 1 });
                let on: Vec<DVec3> = pts.iter().map(|p| s.eval(p.x, p.y)).collect();
                put_curve(d, OUT, Some(polyline_curve(&on)));
            },
        ));
        reg(node_type(
            "projectCurve",
            "A curve's shadow on a surface along a direction or the surface's normal.",
            vec![cin("inputCurve", "ic"), cout("outputCurve", "oc"), sin("inputSurface", "is"), v_in("direction", "d", DVec3::Z, "Which way."), bin("useNormal", "un", false), int_in("curveSamples", "cs", 50, "How many samples.")],
            |d| {
                let (Some(c), Some(s)) = (crv(d, IN), srf(d, P)) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let dir = (!d.bool(P + 2)).then(|| d.vec3(P + 1));
                let uv = project_samples(&c, &s, dir, d.int(P + 3).max(2) as usize);
                put_curve(d, OUT, (uv.len() >= 2).then(|| uv_curve(&uv)));
            },
        ));
        reg(node_type(
            "projectTangent",
            "Turns a curve's end to follow another curve or surface where it meets it.",
            vec![cin("inputCurve", "ic"), cout("outputCurve", "oc"), cin("tangentCurve", "tc"), sin("tangentSurface", "ts"), f_in("tangentScale", "tsc", 1.0, "How far the next CV goes."), f_in("rotate", "ro", 0.0, "Degrees about the end."), bin("reverseTangent", "rt", false), int_in("tangentDirection", "td", 1, "1 U or 2 V on a surface.")],
            |d| {
                let Some(mut c) = crv(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                if c.cvs.len() < 2 {
                    put_curve(d, OUT, Some(c));
                    return;
                }
                let (lo, hi) = c.domain();
                let (at_end, end) = {
                    let (p0, p1) = (c.eval(lo), c.eval(hi));
                    let near = |p: DVec3| -> f64 {
                        if let Some(t) = crv(d, P) {
                            return t.eval(t.closest_param(p)).distance(p);
                        }
                        if let Some(s) = srf(d, P + 1) {
                            let (u, v) = s.closest_point(p);
                            return s.eval(u, v).distance(p);
                        }
                        f64::MAX
                    };
                    if near(p1) <= near(p0) { (true, p1) } else { (false, p0) }
                };
                let dir = if let Some(t) = crv(d, P) {
                    t.tangent(t.closest_param(end))
                } else if let Some(s) = srf(d, P + 1) {
                    let (u, v) = s.closest_point(end);
                    let (_, du, dv) = s.eval_ders(u, v);
                    if d.int(P + 5) == 2 { dv } else { du }
                } else {
                    DVec3::ZERO
                };
                let mut dir = dir.normalize_or_zero();
                if d.bool(P + 4) {
                    dir = -dir;
                }
                let n = c.cvs.len();
                let (tip, next) = if at_end { (n - 1, n - 2) } else { (0, 1) };
                let along = c.cvs[next] - c.cvs[tip];
                if dir.dot(along) < 0.0 {
                    dir = -dir;
                }
                let axis = dir.cross(along).normalize_or_zero();
                if axis != DVec3::ZERO {
                    dir = glam::DQuat::from_axis_angle(axis, d.float(P + 3).to_radians()) * dir;
                }
                c.cvs[next] = c.cvs[tip] + dir * along.length() * d.float(P + 2);
                put_curve(d, OUT, Some(c));
            },
        ));
        reg(node_type(
            "hardenPointCurve",
            "Makes a curve pass through its CVs by raising the knot multiplicity there.",
            vec![cin("inputCurve", "ic"), cout("outputCurve", "oc"), crate::rig::data_attr("inputComponents", "ics", "The CVs."), int_in("multiplicity", "m", -1, "How many knots; -1 as many as the degree.")],
            |d| {
                let Some(mut c) = crv(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let ids: Vec<usize> = crate::rig::json_of(d, P).as_array().into_iter().flatten().filter_map(Value::as_u64).map(|x| x as usize).collect();
                let m = d.int(P + 1);
                let times = if m < 0 { c.degree } else { (m as usize).min(c.degree) };
                for i in ids {
                    if c.form != Form::Open || i == 0 || i + 1 >= c.cvs.len() || times == 0 {
                        continue;
                    }
                    // The knots this CV's span covers come together on their middle.
                    let first = i;
                    let last = (i + c.degree - 1).min(c.knots.len() - 1);
                    let chosen: Vec<usize> = (first..=last).collect();
                    let mean = chosen.iter().map(|&k| c.knots[k]).sum::<f64>() / chosen.len() as f64;
                    let take = times.min(chosen.len());
                    let skip = (chosen.len() - take) / 2;
                    for &k in chosen.iter().skip(skip).take(take) {
                        c.knots[k] = mean;
                    }
                }
                put_curve(d, OUT, Some(c));
            },
        ));
        reg(node_type(
            "pointCurveConstraint",
            "Makes a curve pass through a point at a parameter.",
            vec![cin("inputCurve", "ic"), cout("outputCurve", "oc"), f_in("parameter", "pr", 0.0, "Where on the curve."), v_in("position", "p", DVec3::ZERO, "Where it goes."), f_in("weight", "w", 1.0, "How much.")],
            |d| {
                let Some(mut c) = crv(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (u, at, w) = (d.float(P), d.vec3(P + 1), d.float(P + 2));
                let target = c.eval(u).lerp(at, w);
                crate::nurbs_components::pull_curve(&mut c, u, target);
                put_curve(d, OUT, Some(c));
            },
        ));
        reg(node_type(
            "offsetCurveOnSurface",
            "A curve on a surface offset sideways in (u, v).",
            vec![cin("inputCurve", "ic"), cout("outputCurve", "oc"), f_in("distance", "d", 1.0, "How far, in parameter space.")],
            |d| {
                let Some(c) = crv(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let dist = d.float(P);
                let pts: Vec<DVec3> = c.tessellate(8).windows(2).map(|w| {
                    let t = (w[1] - w[0]).normalize_or_zero();
                    w[0] + DVec3::new(-t.y, t.x, 0.0) * dist
                }).collect();
                put_curve(d, OUT, (pts.len() >= 2).then(|| polyline_curve(&pts)));
            },
        ));
        reg(node_type(
            "makeThreePointCircularArc",
            "A circular arc through three points.",
            vec![v_in("point1", "pt1", DVec3::ZERO, "Start."), cout("outputCurve", "oc"), v_in("point2", "pt2", DVec3::X, "Middle."), v_in("point3", "pt3", DVec3::Y, "End."), int_in("degree", "d", 3, "The degree."), int_in("sections", "s", 8, "The sections."), v_in("center", "c", DVec3::ZERO, "Asked by -q."), f_in("radius", "r", 1.0, "Asked by -q.")],
            |d| {
                let (a, b, c) = (d.vec3(IN), d.vec3(P), d.vec3(P + 1));
                put_curve(d, OUT, three_point_arc(a, b, c, d.int(P + 2).max(1) as usize, d.int(P + 3).max(1) as usize));
            },
        ));
        reg(node_type(
            "makeTwoPointCircularArc",
            "A circular arc between two points with a radius.",
            vec![v_in("point1", "pt1", DVec3::ZERO, "Start."), cout("outputCurve", "oc"), v_in("point2", "pt2", DVec3::X, "End."), v_in("directionVector", "dv", DVec3::Z, "The arc's normal."), f_in("radius", "r", 1.0, "The radius."), bin("toggleArc", "tac", false), int_in("degree", "d", 3, "The degree."), int_in("sections", "s", 8, "The sections.")],
            |d| {
                let (a, b, n) = (d.vec3(IN), d.vec3(P), d.vec3(P + 1).normalize_or_zero());
                let half = a.distance(b) * 0.5;
                let r = d.float(P + 2).abs().max(half);
                let mid = (a + b) * 0.5;
                let side = n.cross(b - a).normalize_or_zero();
                let h = (r * r - half * half).max(0.0).sqrt();
                let (far, near_arc) = (d.bool(P + 3), mid + side * (r - h));
                let apex = if far { mid - side * (r + h) } else { near_arc };
                put_curve(d, OUT, three_point_arc(a, apex, b, d.int(P + 4).max(1) as usize, d.int(P + 5).max(1) as usize));
            },
        ));
        reg(node_type(
            "makeTextCurves",
            "Text as curves.",
            vec![AttrDef { name: "text".into(), short_name: "t".into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: STORE, doc: "The words.".into(), ..Default::default() }, AttrDef { name: "font".into(), short_name: "f".into(), ty: PlugType::String, default_value: PlugValue::String("Stroke".into()), flags: STORE, doc: "Accepted: one stroke font here.".into(), ..Default::default() }],
            |_d| {},
        ));
        reg(node_type(
            "intersectSurface",
            "Where two surfaces cross, as curves.",
            vec![sin("inputSurface1", "is1"), AttrDef { flags: flags::OUTPUT | flags::ARRAY | flags::HIDDEN, ..cout("outputCurve", "oc") }, sin("inputSurface2", "is2")],
            |d| {
                let (Some(a), Some(b)) = (srf(d, IN), srf(d, P)) else {
                    d.set_output_elements(OUT, Vec::new());
                    return;
                };
                let list = mesh_intersection(&surface_mesh(&a, 16), &surface_mesh(&b, 16));
                d.set_output_elements(OUT, list.iter().enumerate().map(|(i, l)| (i as i32, curves::to_value(polyline_curve(l)))).collect());
            },
        ));
        reg(node_type(
            "subSurface",
            "A part of a surface, by patches.",
            vec![sin("inputSurface", "is"), sout("outputSurface", "os"), int_in("firstFaceU", "ffu", 0, "First patch in U."), int_in("firstFaceV", "ffv", 0, "First patch in V."), int_in("faceCountU", "fcu", 1, "How many in U."), int_in("faceCountV", "fcv", 1, "How many in V.")],
            |d| {
                let Some(s) = srf(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (fu, fv, cu, cv) = (d.int(P).max(0) as usize, d.int(P + 1).max(0) as usize, d.int(P + 2).max(1) as usize, d.int(P + 3).max(1) as usize);
                let cut = |s: &NurbsSurface, first: usize, count: usize, along_u: bool| -> NurbsSurface {
                    let (lo, hi) = if along_u { s.domain_u() } else { s.domain_v() };
                    let spans = if along_u { s.spans_u() } else { s.spans_v() }.max(1);
                    let step = (hi - lo) / spans as f64;
                    let (a, b) = (lo + step * first as f64, lo + step * (first + count).min(spans) as f64);
                    let mut r = s.clone();
                    if a > lo + 1e-9 {
                        r = if along_u { r.split_u(a).1 } else { r.split_v(a).1 };
                    }
                    if b < hi - 1e-9 {
                        r = if along_u { r.split_u(b).0 } else { r.split_v(b).0 };
                    }
                    r
                };
                let r = cut(&s, fu, cu, true);
                let r = cut(&r, fv, cv, false);
                put_surface(d, OUT, Some(r));
            },
        ));
        reg(node_type(
            "extendSurface",
            "A surface lengthened past its edges.",
            vec![sin("inputSurface", "is"), sout("outputSurface", "os"), f_in("distance", "d", 1.0, "How far."), int_in("extendDirection", "ed", 0, "0 U, 1 V, 2 both."), int_in("extendSide", "es", 1, "0 start, 1 end, 2 both."), int_in("extensionType", "et", 0, "0 tangent, 1 extrapolate.")],
            |d| {
                let Some(s) = srf(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (dist, dir, side) = (d.float(P), d.int(P + 1), d.int(P + 2));
                let (st, en) = (side != 1, side != 0);
                let along_u = |x: &NurbsSurface| x.map_u(|c| c.extended(dist, st, en));
                let along_v = |x: &NurbsSurface| x.swapped().map_u(|c| c.extended(dist, st, en)).swapped();
                let r = match dir {
                    1 => along_v(&s),
                    2 => along_v(&along_u(&s)),
                    _ => along_u(&s),
                };
                put_surface(d, OUT, Some(r));
            },
        ));
        reg(node_type(
            "smoothTangentSrf",
            "Lines a surface's CVs up across a knot so it's smooth there.",
            vec![sin("inputSurface", "is"), sout("outputSurface", "os"), f_in("parameter", "p", 0.0, "Which isoparm."), int_in("direction", "d", 1, "0 V, 1 U."), int_in("smoothness", "s", 1, "Accepted.")],
            |d| {
                let Some(s) = srf(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (par, along_u) = (d.float(P), d.int(P + 1) == 1);
                let mut w = if along_u { s.clone() } else { s.swapped() };
                let params = w.cv_params_u();
                let at = params.iter().enumerate().min_by(|x, y| (x.1 - par).abs().total_cmp(&(y.1 - par).abs())).map(|x| x.0).unwrap_or(0);
                if at > 0 && at + 1 < w.nu {
                    for j in 0..w.nv {
                        let (a, p, b) = (w.cv(at - 1, j), w.cv(at, j), w.cv(at + 1, j));
                        let dir = (b - a).normalize_or_zero();
                        w.cvs[(at - 1) * w.nv + j] = p - dir * (p - a).length();
                        w.cvs[(at + 1) * w.nv + j] = p + dir * (b - p).length();
                    }
                }
                put_surface(d, OUT, Some(if along_u { w } else { w.swapped() }));
            },
        ));
        reg(node_type(
            "trim",
            "Trims a surface along curves on it.",
            vec![sin("inputSurface", "is"), sout("outputSurface", "os"), data_in("trimLoops", "tl"), bin("keepInside", "ki", true)],
            |d| {
                let Some(mut s) = srf(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let loops = surfaces::trim_from_json(&crate::rig::json_of(d, P));
                for mut r in loops {
                    r.keep_odd = d.bool(P + 1);
                    s.trim.push(r);
                }
                put_surface(d, OUT, Some(s));
            },
        ));
        reg(node_type(
            "untrim",
            "Takes a surface's trims off.",
            vec![sin("inputSurface", "is"), sout("outputSurface", "os")],
            |d| {
                let Some(mut s) = srf(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                s.trim.clear();
                put_surface(d, OUT, Some(s));
            },
        ));
        reg(node_type(
            "stitchSrf",
            "Moves a surface's edge onto another's.",
            vec![sin("inputSurface", "is"), sout("outputSurface", "os"), cin("targetEdge", "te"), int_in("edgeDirection", "ed", 0, "0 U, 1 V."), f_in("edgeParameter", "ep", 0.0, "Which edge."), f_in("bias", "b", 0.5, "0 stays, 1 goes all the way.")],
            |d| {
                let (Some(s), Some(t)) = (srf(d, IN), crv(d, P)) else {
                    let out = srf(d, IN);
                    put_surface(d, OUT, out);
                    return;
                };
                put_surface(d, OUT, Some(stitch_boundary(&s, d.int(P + 1), d.float(P + 2), &t, d.float(P + 3))));
            },
        ));
        reg(node_type(
            "bevel",
            "A bevelled wall round a curve.",
            vec![cin("inputCurve", "ic"), sout("outputSurface", "os"), f_in("width", "w", 0.1, "How far out."), f_in("depth", "d", 0.1, "How deep the bevel."), f_in("extrudeDepth", "ed", 1.0, "How deep the wall."), int_in("bevelShapeType", "bst", 1, "1 straight, 2 arc."), bin("backFace", "bf", true)],
            |d| {
                let Some(c) = crv(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (w, dp, e, back) = (d.float(P), d.float(P + 1), d.float(P + 2), d.bool(P + 4));
                let list = if d.int(P + 3) == 2 { round_sections(&c, w, dp, e, 4, back) } else { bevel_sections(&c, w, dp, e, back) };
                put_surface(d, OUT, loft_of(&list));
            },
        ));
        reg(node_type(
            "bevelPlus",
            "A bevelled wall round a curve, rounded, with caps.",
            vec![cin("inputCurve", "ic"), sout("outputSurface", "os"), f_in("width", "w", 0.1, "How far out."), f_in("depth", "d", 0.1, "How deep the bevel."), f_in("extrudeDepth", "ed", 1.0, "How deep the wall."), int_in("numberOfSides", "ns", 4, "1 to 4: straight, or more round."), int_in("capSides", "cap", 4, "1 none, 2 front, 3 back, 4 both."), crate::attr_defs::mesh_attr("outputPoly", "op", flags::OUTPUT | flags::HIDDEN, "The wall with its caps, as polygons.")],
            |d| {
                let Some(c) = crv(d, IN) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    d.set_output(P + 5, PlugValue::Mesh(None));
                    return;
                };
                let (w, dp, ed, cap) = (d.float(P), d.float(P + 1), d.float(P + 2), d.int(P + 4));
                let list = round_sections(&c, w, dp, ed, d.int(P + 3).max(1) as usize, true);
                let wall = loft_of(&list);
                let mut m = wall.as_ref().map_or_else(Mesh::default, |s| s.tessellate(4, 1));
                if closed_outline(&c) {
                    let up = {
                        let n = c.plane_normal();
                        if n == DVec3::ZERO { DVec3::Z } else { n }
                    };
                    for (on, z, flip) in [(cap == 2 || cap == 4, 0.0, false), (cap == 3 || cap == 4, -2.0 * dp - ed, true)] {
                        if !on {
                            continue;
                        }
                        let mut pts = c.transformed(&glam::DMat4::from_translation(up * z)).tessellate(8);
                        if pts.len() > 1 && pts[0].distance(*pts.last().unwrap()) < 1e-6 {
                            pts.pop();
                        }
                        if flip {
                            pts.reverse();
                        }
                        let ids: Vec<u32> = pts.iter().map(|p| m.add_point(*p)).collect();
                        if ids.len() >= 3 {
                            m.add_face(&ids);
                        }
                    }
                }
                put_surface(d, OUT, wall);
                d.set_output(P + 5, PlugValue::Mesh(Some(Arc::new(m))));
            },
        ));
        reg(node_type(
            "rbfSrf",
            "A round fillet between two surfaces.",
            vec![sin("primarySurface", "ps"), sout("outputSurface", "os"), sin("secondarySurface", "ss"), f_in("primaryRadius", "pr", 1.0, "Offset from the first, signed."), f_in("secondaryRadius", "sr", 1.0, "Offset from the second, signed.")],
            |d| {
                let (Some(a), Some(b)) = (srf(d, IN), srf(d, P)) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (r1, r2) = (d.float(P + 1), d.float(P + 2));
                put_surface(d, OUT, circular_fillet(&a, &b, r1, r2));
            },
        ));
        reg(node_type(
            "ffFilletSrf",
            "A free-form blend between two edges.",
            vec![sin("inputSurface1", "is1"), sout("outputSurface", "os"), sin("inputSurface2", "is2"), int_in("direction1", "d1", 0, "0 U, 1 V."), f_in("parameter1", "p1", 0.0, "Which isoparm."), int_in("direction2", "d2", 0, "0 U, 1 V."), f_in("parameter2", "p2", 0.0, "Which isoparm."), f_in("depth", "dp", 0.5, "How far the tangents reach."), f_in("bias", "b", 0.0, "Which way it leans.")],
            |d| {
                let (Some(a), Some(b)) = (srf(d, IN), srf(d, P)) else {
                    d.set_output(OUT, PlugValue::Data(None));
                    return;
                };
                let (c1, t1) = across_tangents(&a, d.int(P + 1), d.float(P + 2), 16);
                let (c2, t2) = across_tangents(&b, d.int(P + 3), d.float(P + 4), 16);
                // Tangents into the gap at the second edge point back along it.
                let t2: Vec<DVec3> = t2.iter().map(|v| -*v).collect();
                put_surface(d, OUT, tangent_blend(&c1, &t1, &c2, &t2, d.float(P + 5), d.float(P + 6), 16));
            },
        ));
        reg(node_type(
            "plane",
            "A construction plane.",
            vec![f_in("width", "w", 1.0, "Its width."), f_in("length", "l", 1.0, "Its length.")],
            |_d| {},
        ));
        types::add_file_alias("planarSrf", "planarTrim");
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands' parts

fn connect_world_curve(g: &mut Graph, shape: NodeId, node: NodeId, attr: usize) -> CmdResult<()> {
    g.connect(&plug(shape, CurveShape::WORLD_SPACE), &plug(node, attr), true)?;
    Ok(())
}

fn connect_world_surface(g: &mut Graph, shape: NodeId, node: NodeId, attr: usize) -> CmdResult<()> {
    g.connect(&plug(shape, SurfaceShape::WORLD_SPACE), &plug(node, attr), true)?;
    Ok(())
}

fn new_node(g: &mut Graph, ty: &str, name: &str) -> CmdResult<NodeId> {
    let t = types::find(ty).unwrap_or_else(|| panic!("no node type {ty}"));
    Ok(g.create_node(&t, name, NO_NODE, true)?)
}

fn at(g: &Graph, n: NodeId, attr: &str) -> usize {
    g.node(n).unwrap().ty().attr_index(attr).unwrap_or_else(|| panic!("no attribute {attr}"))
}

fn set_f(g: &mut Graph, n: NodeId, attr: &str, v: f64) -> CmdResult<()> {
    let i = at(g, n, attr);
    g.set_value(plug(n, i), PlugValue::Float(v))?;
    Ok(())
}
fn set_i(g: &mut Graph, n: NodeId, attr: &str, v: i64) -> CmdResult<()> {
    let i = at(g, n, attr);
    g.set_value(plug(n, i), PlugValue::Int(v))?;
    Ok(())
}
fn set_b(g: &mut Graph, n: NodeId, attr: &str, v: bool) -> CmdResult<()> {
    let i = at(g, n, attr);
    g.set_value(plug(n, i), PlugValue::Bool(v))?;
    Ok(())
}
fn set_v(g: &mut Graph, n: NodeId, attr: &str, v: DVec3) -> CmdResult<()> {
    let i = at(g, n, attr);
    g.set_value(plug(n, i), PlugValue::Vec3(v))?;
    Ok(())
}

/// A new curve object fed by a node's output; without history it keeps the result and the node goes.
fn curve_from(g: &mut Graph, a: &Args, node: NodeId, out: PlugRef, name: &str) -> CmdResult {
    let (tr, sh) = crate::dag::create_with_shape(g, curves::SHAPE_TYPE, &a.string("name", name), NO_NODE)?;
    g.connect(&out, &plug(sh, CurveShape::CREATE), false)?;
    g.select(&[tr], SelectMode::Replace);
    g.evaluate();
    if !surfaces::wants_history(g, a) {
        curves::bake_history(g, sh)?;
        if g.node(node).is_some() {
            g.delete_node(node)?;
        }
        return Ok(json!([g.path(tr)]));
    }
    Ok(json!([g.path(tr), g.path(node)]))
}

/// A new surface object (or with -po 1 a mesh) fed by a node's output.
fn surface_from(g: &mut Graph, a: &Args, node: NodeId, out: PlugRef, name: &str) -> CmdResult {
    let polygon = a.int("polygon", 0) == 1;
    let wanted = a.string("name", name);
    let (tr, sh) = if polygon {
        surfaces::tessellate_plug(g, &out, &wanted, &surfaces::per_span_settings())?
    } else {
        let (tr, sh) = crate::dag::create_with_shape(g, surfaces::SHAPE_TYPE, &wanted, NO_NODE)?;
        g.connect(&out, &plug(sh, SurfaceShape::CREATE), false)?;
        (tr, sh)
    };
    g.select(&[tr], SelectMode::Replace);
    g.evaluate();
    if !surfaces::wants_history(g, a) {
        if polygon {
            if let Some(m) = crate::components::as_mesh(g, tr) {
                crate::history::delete_history(g, m)?;
            }
        } else {
            surfaces::bake_history(g, sh)?;
        }
        if g.node(node).is_some() {
            g.delete_node(node)?;
        }
        return Ok(json!([g.path(tr)]));
    }
    Ok(json!([g.path(tr), g.path(node)]))
}

fn make_flags(c: Cmd) -> Cmd {
    c.flag(f("constructionHistory", "ch", B, "Keep the node (on by default)."))
        .flag(f("name", "n", S, "The new object's name."))
        .flag(f("object", "o", B, "Accepted for existing scripts."))
        .flag(f("range", "rn", B, "Accepted for existing scripts."))
        .flag(f("caching", "cch", B, "Accepted for existing scripts."))
        .flag(f("nodeState", "nds", I, "Accepted for existing scripts."))
        .flag(f("tolerance", "tol", Fl, "Accepted for existing scripts."))
        .flag(f("polygon", "po", I, "0 NURBS, 1 polygons."))
}

/// A surface's edge named surf.u[x] or surf.v[x], or a plain surface: (shape, Some((dir, value))).
fn edge_arg(g: &Graph, name: &str, cmd: &str) -> CmdResult<(NodeId, Option<(i64, f64)>)> {
    if let Some((sh, dir, v)) = surfaces::parse_isoparm(g, name) {
        return Ok((sh, Some((dir, v))));
    }
    Ok((surfaces::surface_arg(g, name, cmd)?, None))
}

/// curve1 or surface->curve1: its loop in (u, v), by its own curve on surface or by closest points.
fn loop_on(g: &mut Graph, name: &str, s: &NurbsSurface) -> CmdResult<Vec<DVec2>> {
    let id = find(g, name, "trim")?;
    let Some(shape) = curves::as_curve(g, id) else { fail!("trim: {name} is not a curve") };
    if let Some(src) = g.source(&plug(shape, CurveShape::CREATE)) {
        if g.node(src.node).is_some_and(|n| n.ty().name == "curveFromSurfaceCoS") {
            let v = g.value(&plug(src.node, P))?;
            if let Some(c) = curve_value(&v) {
                let mut pts: Vec<DVec2> = c.tessellate(if c.degree > 1 { 12 } else { 1 }).iter().map(|p| DVec2::new(p.x, p.y)).collect();
                if pts.len() > 1 && pts[0].distance(*pts.last().unwrap()) < 1e-6 {
                    pts.pop();
                }
                return Ok(pts);
            }
        }
    }
    let c = curves::world_curve(g, shape).unwrap_or_default();
    let mut pts: Vec<DVec2> = c.tessellate(12).iter().map(|p| { let (u, v) = s.closest_point(*p); DVec2::new(u, v) }).collect();
    if pts.len() > 1 && pts[0].distance(*pts.last().unwrap()) < 1e-6 {
        pts.pop();
    }
    Ok(pts)
}

fn trim_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.len() < 2 {
        fail!("trim: name a surface and the closed curves on it");
    }
    let g = &mut s.graph;
    let shape = surfaces::surface_arg(g, &names[0], "trim")?;
    let Some(surf) = surfaces::world_surface(g, shape) else { fail!("trim: {} has no surface", names[0]) };
    let mut loops = Vec::new();
    for n in &names[1..] {
        loops.push(loop_on(g, n, &surf)?);
    }
    let (ulo, uhi) = surf.domain_u();
    let (vlo, vhi) = surf.domain_v();
    // -lu -lv go 0 to 1 over the surface's parameters. The region holding them is kept, or with -sl 1 thrown away.
    let at_uv = DVec2::new(ulo + (uhi - ulo) * a.float("locatorU", 0.5), vlo + (vhi - vlo) * a.float("locatorV", 0.5));
    let probe = TrimRegion { loops: loops.clone(), keep_odd: true };
    let inside = probe.keeps(at_uv);
    let keep_inside = if a.int("selected", 0) == 0 { inside } else { !inside };
    let region = json!([{"keepOdd": keep_inside, "loops": loops.iter().map(|l| l.iter().flat_map(|p| [p.x, p.y]).collect::<Vec<f64>>()).collect::<Vec<_>>()}]);
    g.open_chunk("Trim");
    let r = (|| -> CmdResult<NodeId> {
        let node = surfaces::insert_op(g, shape, "trim", &[(P, PlugValue::Data(Some(Arc::new(crate::rig::JsonData(region.clone()))))), (P + 1, PlugValue::Bool(keep_inside))])?;
        Ok(node)
    })();
    g.close_chunk();
    let node = r?;
    g.evaluate();
    if !surfaces::wants_history(g, a) {
        surfaces::bake_history(g, shape)?;
        return Ok(json!([g.path(crate::dag::transform_of(g, shape))]));
    }
    Ok(json!([g.path(crate::dag::transform_of(g, shape)), g.path(node)]))
}

/// A curve on a surface: a curve object with its (u, v) curve on a node that follows the surface.
fn make_cos(g: &mut Graph, surf_shape: NodeId, uv: NurbsCurve, a: &Args) -> CmdResult<(NodeId, NodeId)> {
    let n = new_node(g, "curveFromSurfaceCoS", "curveFromSurfaceCoS1")?;
    connect_world_surface(g, surf_shape, n, IN)?;
    g.set_value(plug(n, P), curves::to_value(uv))?;
    let (tr, sh) = crate::dag::create_with_shape(g, curves::SHAPE_TYPE, &a.string("name", "curve1"), NO_NODE)?;
    g.connect(&plug(n, OUT), &plug(sh, CurveShape::CREATE), false)?;
    Ok((tr, n))
}

fn cos_name(g: &Graph, surf_shape: NodeId, curve: NodeId) -> String {
    format!("{}->{}", g.path(surf_shape), g.path(curve))
}

fn curve_on_surface(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(name) = names.first() else { fail!("curveOnSurface: name a surface") };
    let g = &mut s.graph;
    let shape = surfaces::surface_arg(g, name, "curveOnSurface")?;
    let mut nums = Vec::new();
    if let Some(v) = a.get("positionUV") {
        flat(v, &mut nums);
    }
    if nums.len() < 4 || nums.len() % 2 != 0 {
        fail!("curveOnSurface: give at least two -uv points");
    }
    let pts: Vec<DVec3> = nums.chunks(2).map(|c| DVec3::new(c[0], c[1], 0.0)).collect();
    let degree = a.int("degree", 3).max(1) as usize;
    let degree = degree.min(pts.len() - 1);
    let knots: Vec<f64> = match a.get("knot") {
        Some(v) => {
            let mut k = Vec::new();
            flat(v, &mut k);
            k
        }
        None => Vec::new(),
    };
    let uv = if a.bool("periodic") {
        NurbsCurve::periodic(degree, pts)
    } else if knots.len() == pts.len() + degree - 1 {
        NurbsCurve::new(degree, Form::Open, pts, knots)
    } else {
        NurbsCurve::open(degree, pts)
    };
    g.open_chunk("curveOnSurface");
    let r = make_cos(g, shape, uv, a);
    g.close_chunk();
    let (tr, _) = r?;
    g.evaluate();
    Ok(json!(cos_name(g, shape, tr)))
}

fn flat(v: &Value, out: &mut Vec<f64>) {
    match v {
        Value::Array(l) => l.iter().for_each(|x| flat(x, out)),
        x => out.extend(x.as_f64()),
    }
}

fn nums_flag(a: &Args, flag: &str) -> Vec<f64> {
    let mut out = Vec::new();
    if let Some(v) = a.get(flag) {
        flat(v, &mut out);
    }
    out
}

fn form_of(v: Option<&Value>) -> Form {
    match v.and_then(Value::as_str).unwrap_or("open") {
        "periodic" | "p" => Form::Periodic,
        "closed" | "c" => Form::Closed,
        _ => match v.and_then(Value::as_i64) {
            Some(i) => Form::from_index(i),
            None => Form::Open,
        },
    }
}

fn raw_surface(s: &mut Session, a: &Args) -> CmdResult {
    let (du, dv) = (a.int("degreeU", 1).max(1) as usize, a.int("degreeV", 1).max(1) as usize);
    let (fu, fv) = (form_of(a.get("formU")), form_of(a.get("formV")));
    let (ku, kv) = (nums_flag(a, "knotU"), nums_flag(a, "knotV"));
    let pts = nums_flag(a, "point");
    let pw = nums_flag(a, "pointWeight");
    let cvs: Vec<DVec3> = if !pts.is_empty() {
        pts.chunks(3).filter(|c| c.len() == 3).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
    } else {
        pw.chunks(4).filter(|c| c.len() == 4).map(|c| DVec3::new(c[0], c[1], c[2])).collect()
    };
    if cvs.is_empty() {
        fail!("surface: give -p");
    }
    if ku.is_empty() || kv.is_empty() {
        fail!("surface: give -ku and -kv");
    }
    let (nu, nv) = ((ku.len() + 1).saturating_sub(du), (kv.len() + 1).saturating_sub(dv));
    if nu * nv != cvs.len() {
        fail!("surface: {} knots in U and {} in V want {} points, not {}", ku.len(), kv.len(), nu * nv, cvs.len());
    }
    let surf = NurbsSurface { degree_u: du, degree_v: dv, form_u: fu, form_v: fv, nu, nv, cvs, knots_u: ku, knots_v: kv, trim: Vec::new() };
    let g = &mut s.graph;
    let (tr, _) = surfaces::create_surface(g, &a.string("name", "surface1"), surf)?;
    g.select(&[tr], SelectMode::Replace);
    g.evaluate();
    Ok(json!(g.path(tr)))
}

fn make_plane(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let size = a.float("size", 1.0);
    let (w, l) = (a.float("width", size), a.float("length", size));
    let t = crate::node::types::find("transform").unwrap();
    let tr = g.create_node(&t, &a.string("name", "plane1"), NO_NODE, true)?;
    if let Some(p) = a.vec3("position") {
        g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(DVec3::from_array(p)))?;
    }
    if let Some(r) = a.vec3("rotation") {
        g.set_value(plug(tr, Transform::ROTATE), PlugValue::Vec3(DVec3::from_array(r)))?;
    }
    // The plane as a square curve under its transform, since nothing draws a sketch plane by itself.
    let outline = NurbsCurve::open(1, vec![DVec3::new(-w / 2.0, 0.0, -l / 2.0), DVec3::new(w / 2.0, 0.0, -l / 2.0), DVec3::new(w / 2.0, 0.0, l / 2.0), DVec3::new(-w / 2.0, 0.0, l / 2.0), DVec3::new(-w / 2.0, 0.0, -l / 2.0)]);
    let name = crate::dag::shape_name(&g.node(tr).unwrap().name().to_string());
    let shape_type = types::find(curves::SHAPE_TYPE).unwrap();
    let sh = g.create_node(&shape_type, &name, tr, false)?;
    curves::set_cached(g, sh, outline)?;
    g.select(&[tr], SelectMode::Replace);
    Ok(json!(g.path(tr)))
}

fn text_curves(s: &mut Session, a: &Args) -> CmdResult {
    let text = a.string("text", "");
    if text.is_empty() {
        fail!("textCurves: give -t");
    }
    let g = &mut s.graph;
    g.open_chunk("Text");
    let r = (|| -> CmdResult<(NodeId, NodeId)> {
        let node = new_node(g, "makeTextCurves", "makeTextCurves1")?;
        let i = at(g, node, "text");
        g.set_value(plug(node, i), PlugValue::String(text.clone()))?;
        let t = types::find("transform").unwrap();
        let stem: String = text.chars().filter(|c| c.is_alphanumeric()).collect();
        let default_name = if stem.is_empty() { "Text_1".to_string() } else { format!("Text_{stem}_1") };
        let group = g.create_node(&t, &a.string("name", &default_name), NO_NODE, true)?;
        // Letters a unit high.
        let k = 1.0 / 6.0;
        let mut x = 0.0;
        for c in text.chars() {
            let strokes = aztek_geo::stroke_font::char_strokes(c);
            if !strokes.is_empty() {
                let letter = g.create_node(&t, &format!("Char_{}_1", if c.is_alphanumeric() { c } else { 'x' }), group, true)?;
                for stroke in strokes {
                    let pts: Vec<DVec3> = stroke.iter().map(|q| DVec3::new(x + q.0 * k, q.1 * k, 0.0)).collect();
                    if pts.len() < 2 {
                        continue;
                    }
                    let (_, sh) = crate::dag::create_with_shape(g, curves::SHAPE_TYPE, "curve1", letter)?;
                    curves::set_cached(g, sh, NurbsCurve::open(1, pts))?;
                }
            }
            x += aztek_geo::stroke_font::char_advance(c) * k;
        }
        Ok((group, node))
    })();
    g.close_chunk();
    let (group, node) = r?;
    g.select(&[group], SelectMode::Replace);
    g.evaluate();
    Ok(json!([g.path(group), g.path(node)]))
}

fn stitch_pair(g: &mut Graph, a: &Args, first: (NodeId, Option<(i64, f64)>), second: (NodeId, Option<(i64, f64)>), bias: f64, out: &mut Vec<Value>) -> CmdResult<()> {
    let (Some(s1), Some(s2)) = (surfaces::local_surface(g, first.0), surfaces::local_surface(g, second.0)) else { fail!("stitchSurface: no surface") };
    let (Some(w1), Some(w2)) = (surfaces::world_surface(g, first.0), surfaces::world_surface(g, second.0)) else { fail!("stitchSurface: no surface") };
    let e1 = first.1.unwrap_or_else(|| nearest_boundary(&w1, &w2));
    let e2 = second.1.unwrap_or_else(|| nearest_boundary(&w2, &w1));
    let iso = |x: &NurbsSurface, e: (i64, f64)| if e.0 == 0 { x.iso_u(e.1) } else { x.iso_v(e.1) };
    let (t1, t2) = (iso(&w2, e2), iso(&w1, e1));
    let to_local = |sh: NodeId, g: &mut Graph, c: NurbsCurve| {
        let w = crate::rig::world(g, crate::dag::transform_of(g, sh));
        c.transformed(&w.inverse())
    };
    let (t1, t2) = (to_local(first.0, g, t1), to_local(second.0, g, t2));
    let _ = (s1, s2);
    let ch = surfaces::wants_history(g, a);
    for (shape, edge, target, b) in [(first.0, e1, t1, bias), (second.0, e2, t2, 1.0 - bias)] {
        let set = [(P, curves::to_value(target)), (P + 1, PlugValue::Int(edge.0)), (P + 2, PlugValue::Float(edge.1)), (P + 3, PlugValue::Float(b))];
        let node = surfaces::insert_op(g, shape, "stitchSrf", &set)?;
        g.evaluate();
        out.push(json!(g.path(crate::dag::transform_of(g, shape))));
        if ch {
            out.push(json!(g.path(node)));
        } else {
            surfaces::bake_history(g, shape)?;
        }
    }
    Ok(())
}

fn stitch_surface(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let [x, y] = names.as_slice() else { fail!("stitchSurface: name two surfaces or two edges") };
    let g = &mut s.graph;
    let (e1, e2) = (edge_arg(g, x, "stitchSurface")?, edge_arg(g, y, "stitchSurface")?);
    let mut out = Vec::new();
    g.open_chunk("Stitch");
    let r = stitch_pair(g, a, e1, e2, a.float("bias", 0.5), &mut out);
    g.close_chunk();
    r?;
    Ok(Value::Array(out))
}

fn stitch_points(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.is_empty() {
        fail!("stitchSurfacePoints: name CVs");
    }
    let bias_equal = a.get("equalWeight").is_none_or(|v| v.as_bool().unwrap_or(true));
    let g = &mut s.graph;
    let members = crate::object_sets::parse(g, &names).map_err(aztek_ops::CommandError::new)?;
    let mut spots: Vec<(NodeId, u32, DVec3)> = Vec::new();
    for m in &members {
        let w = crate::rig::world(g, crate::dag::transform_of(g, m.node));
        let Some(pp) = g.node(m.node).and_then(crate::components::point_provider) else { continue };
        let Some(points) = pp.points else { continue };
        let at: std::collections::HashMap<u32, DVec3> = points(g, m.node).into_iter().collect();
        for &i in &m.ids {
            if let Some(p) = at.get(&i) {
                spots.push((m.node, i, w.transform_point3(*p)));
            }
        }
    }
    if spots.len() < 2 {
        fail!("stitchSurfacePoints: name at least two CVs");
    }
    let mean = if bias_equal { spots.iter().map(|x| x.2).sum::<DVec3>() / spots.len() as f64 } else { spots[0].2 };
    g.open_chunk("Stitch Points");
    for &(node, i, _) in &spots {
        let w = crate::rig::world(g, crate::dag::transform_of(g, node));
        if let Some(set) = g.node(node).and_then(crate::components::point_provider).and_then(|p| p.set_points) {
            set(g, node, &[(i, w.inverse().transform_point3(mean))]);
        }
    }
    g.close_chunk();
    g.evaluate();
    Ok(Value::Array(names.iter().map(|n| json!(n)).collect()))
}

fn global_stitch(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.len() < 2 {
        fail!("globalStitch: name two or more surfaces");
    }
    let max = a.float("maxSeparation", 0.1);
    let g = &mut s.graph;
    let mut shapes = Vec::new();
    for n in &names {
        shapes.push(surfaces::surface_arg(g, n, "globalStitch")?);
    }
    let mut out = Vec::new();
    g.open_chunk("Global Stitch");
    let r = (|| -> CmdResult<()> {
        for i in 0..shapes.len() {
            for j in i + 1..shapes.len() {
                let (Some(wa), Some(wb)) = (surfaces::world_surface(g, shapes[i]), surfaces::world_surface(g, shapes[j])) else { continue };
                let (e1, e2) = (nearest_boundary(&wa, &wb), nearest_boundary(&wb, &wa));
                let iso = |x: &NurbsSurface, e: (i64, f64)| if e.0 == 0 { x.iso_u(e.1) } else { x.iso_v(e.1) };
                let (c1, c2) = (iso(&wa, e1), iso(&wb, e2));
                let (lo, hi) = c1.domain();
                let gap = (0..=8).map(|k| {
                    let p = c1.eval(lo + (hi - lo) * k as f64 / 8.0);
                    p.distance(c2.eval(c2.closest_param(p)))
                }).fold(0.0, f64::max);
                if gap <= max {
                    stitch_pair(g, a, (shapes[i], Some(e1)), (shapes[j], Some(e2)), 0.5, &mut out)?;
                }
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    Ok(Value::Array(out))
}

fn make_single_surface(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.len() < 2 {
        fail!("makeSingleSurface: name two or more surfaces");
    }
    let g = &mut s.graph;
    let mut joined: Option<NurbsSurface> = None;
    for n in &names {
        let sh = surfaces::surface_arg(g, n, "makeSingleSurface")?;
        let Some(w) = surfaces::world_surface(g, sh) else { fail!("makeSingleSurface: {n} has no surface") };
        joined = Some(match joined {
            None => w,
            Some(j) => {
                // Lay the second along whichever side of the first it meets.
                let (e, _) = nearest_boundary(&j, &w);
                if e == 0 { j.attached_u(&w, false, 0.5) } else { j.attached_v(&w, false, 0.5) }
            }
        });
    }
    let (tr, _) = surfaces::create_surface(g, &a.string("name", "surface1"), joined.unwrap())?;
    g.select(&[tr], SelectMode::Replace);
    g.evaluate();
    Ok(json!([g.path(tr)]))
}

fn nurbs_select(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    use crate::components::{self, ComponentMode as K};
    let mut shapes = Vec::new();
    for n in &names {
        let base = n.split('.').next().unwrap_or(n);
        if let Some(id) = g.find(base)? {
            if let Some(sh) = surfaces::as_surface(g, id) {
                shapes.push(sh);
            }
        }
    }
    if shapes.is_empty() {
        for (&node, _) in components::selected_all(g, K::Vertex) {
            if surfaces::is_surface(g, node) {
                shapes.push(node);
            }
        }
    }
    if shapes.is_empty() {
        fail!("nurbsSelect: name or select a surface");
    }
    for sh in shapes {
        let Some(surf) = surfaces::local_surface(g, sh) else { continue };
        let (nu, nv) = (surf.distinct_u() as u32, surf.distinct_v() as u32);
        let mut ids: Vec<u32> = components::selected(g, sh, K::Vertex).to_vec();
        let id = |u: u32, v: u32| u * nv + v;
        let any_border = ["borderSelection", "leftBorder", "rightBorder", "topBorder", "bottomBorder"].iter().any(|f| a.bool(f));
        if any_border {
            ids.clear();
            if a.bool("borderSelection") || a.bool("leftBorder") {
                ids.extend((0..nv).map(|v| id(0, v)));
            }
            if a.bool("borderSelection") || a.bool("rightBorder") {
                ids.extend((0..nv).map(|v| id(nu - 1, v)));
            }
            if a.bool("borderSelection") || a.bool("bottomBorder") {
                ids.extend((0..nu).map(|u| id(u, 0)));
            }
            if a.bool("borderSelection") || a.bool("topBorder") {
                ids.extend((0..nu).map(|u| id(u, nv - 1)));
            }
        }
        for _ in 0..a.int("growSelection", 0).max(0) {
            let set: std::collections::BTreeSet<u32> = ids.iter().copied().collect();
            for &i in &set {
                let (u, v) = (i / nv, i % nv);
                if u > 0 {
                    ids.push(id(u - 1, v));
                }
                if u + 1 < nu {
                    ids.push(id(u + 1, v));
                }
                if v > 0 {
                    ids.push(id(u, v - 1));
                }
                if v + 1 < nv {
                    ids.push(id(u, v + 1));
                }
            }
        }
        for _ in 0..a.int("shrinkSelection", 0).max(0) {
            let set: std::collections::BTreeSet<u32> = ids.iter().copied().collect();
            ids.retain(|&i| {
                let (u, v) = (i / nv, i % nv);
                u > 0 && u + 1 < nu && v > 0 && v + 1 < nv && set.contains(&id(u - 1, v)) && set.contains(&id(u + 1, v)) && set.contains(&id(u, v - 1)) && set.contains(&id(u, v + 1))
            });
        }
        ids.sort_unstable();
        ids.dedup();
        components::select(g, sh, K::Vertex, &ids, components::Op::Replace);
        components::set_mode(g, K::Vertex);
    }
    Ok(Value::Null)
}

fn nurbs_boolean(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let [x, y] = names.as_slice() else { fail!("nurbsBoolean: name two surfaces") };
    let mut meshes = Vec::new();
    s.graph.open_chunk("NURBS Boolean");
    let r = (|| -> CmdResult<Value> {
        for n in [x, y] {
            let made = s.run("nurbsToPoly", json!({"args": [n], "constructionHistory": false, "polygonType": 1, "format": 3, "uNumber": 12, "vNumber": 12, "uType": 3, "vType": 3}))?;
            meshes.push(made[0].clone());
        }
        // The reference's -op 0 union, 1 difference, 2 intersection; polyCBoolOp counts from 1.
        s.run("polyCBoolOp", json!({"args": meshes, "op": a.int("operation", 0) + 1, "constructionHistory": false}))
    })();
    s.graph.close_chunk();
    r
}

/// blend2's rail: a curve, or surface.u[x] or surface.v[x].
fn connect_rail(g: &mut Graph, n: NodeId, name: &str, curve_at: usize, rail_at: usize, dir_attr: &str, par_attr: &str) -> CmdResult<()> {
    if let Some((sh, dir, value)) = surfaces::parse_isoparm(g, name) {
        connect_world_surface(g, sh, n, rail_at)?;
        set_i(g, n, dir_attr, dir)?;
        set_f(g, n, par_attr, value)?;
        return Ok(());
    }
    let sh = surfaces::curve_shape(g, name, "blend2")?;
    connect_world_curve(g, sh, n, curve_at)
}

fn blend2(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if a.int("crvsInFirstRail", 1) != 1 || names.len() != 2 {
        fail!("blend2: give two rails, one curve or edge each; rails of several curves are not supported");
    }
    let g = &mut s.graph;
    g.open_chunk("Blend");
    let r = (|| -> CmdResult {
        let n = new_node(g, "ffBlendSrf", "ffBlendSrf1")?;
        connect_rail(g, n, &names[0], IN, P + 1, "leftDirection", "leftParameter")?;
        connect_rail(g, n, &names[1], P, P + 2, "rightDirection", "rightParameter")?;
        for name in ["leftStart", "leftEnd", "rightStart", "rightEnd", "leftAnchor", "rightAnchor", "positionTolerance", "tangentTolerance"] {
            if let Some(v) = a.get(name).and_then(Value::as_f64) {
                set_f(g, n, name, v)?;
            }
        }
        for name in ["autoAnchor", "autoNormal", "flipLeftNormal", "flipRightNormal", "reverseLeft", "reverseRight", "multipleKnots"] {
            if a.has(name) {
                set_b(g, n, name, a.bool(name))?;
            }
        }
        surface_from(g, a, n, plug(n, OUT), "blend2Surface1")
    })();
    g.close_chunk();
    r
}

fn round_constant_radius(s: &mut Session, a: &Args) -> CmdResult {
    for flag in ["append", "sidea", "sideb", "side"] {
        if a.has(flag) && a.get(flag).is_some_and(|v| v.as_bool() != Some(false)) {
            fail!("roundConstantRadius: -{flag} is not supported: no compound edges, and each call makes its own nodes");
        }
    }
    let names = a.named();
    if names.len() < 2 || names.len() % 2 != 0 {
        fail!("roundConstantRadius: give edges in pairs: surface.u[x] surface.v[y]");
    }
    let radii: Vec<f64> = match a.get("radiuss") {
        Some(Value::Array(l)) => l.iter().filter_map(Value::as_f64).collect(),
        Some(v) => v.as_f64().into_iter().collect(),
        None => Vec::new(),
    };
    let g = &mut s.graph;
    g.open_chunk("Round");
    let r = (|| -> CmdResult {
        let (mut surfaces_made, mut nodes) = (Vec::new(), Vec::new());
        for (k, pair) in names.chunks(2).enumerate() {
            let (e1, e2) = (edge_arg(g, &pair[0], "roundConstantRadius")?, edge_arg(g, &pair[1], "roundConstantRadius")?);
            let (Some((d1, p1)), Some((d2, p2))) = (e1.1, e2.1) else { fail!("roundConstantRadius: name edges: surface.u[x] or surface.v[x]") };
            let n = new_node(g, "roundConstantRadius", "roundConstantRadius1")?;
            connect_world_surface(g, e1.0, n, IN)?;
            connect_world_surface(g, e2.0, n, P)?;
            set_i(g, n, "direction1", d1)?;
            set_f(g, n, "parameter1", p1)?;
            set_i(g, n, "direction2", d2)?;
            set_f(g, n, "parameter2", p2)?;
            let radius = radii.get(k).or(radii.first()).copied().unwrap_or(1.0);
            set_f(g, n, "radius", radius)?;
            let made = surface_from(g, a, n, plug(n, OUT), "roundedEdge1")?;
            let list = made.as_array().cloned().unwrap_or_default();
            surfaces_made.extend(list.first().cloned());
            nodes.extend(list.get(1).cloned());
        }
        surfaces_made.extend(nodes);
        Ok(Value::Array(surfaces_made))
    })();
    g.close_chunk();
    r
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        make_flags(Cmd::new(
            "blend2",
            "A surface between two rails, curves or surface edges (surface.u[x]): -ls, -le, -rs, -re the ranges, -la, -ra on closed rails, -aa lines closed rails up, -fln, -frn flip the tangents off an edge, -rvl, -rvr with -an false. Returns [surface, node]. Less than the reference: one curve per rail, so -cfr must be 1; the result meets its rails exactly and -pt, -tt and -mk are only held.",
            blend2,
        ))
        .positional("two rails (the selection when none)")
        .flag(f("autoAnchor", "aa", B, "On closed rails, line them up."))
        .flag(f("autoNormal", "an", B, "Take the directions from the geometry."))
        .flag(f("crvsInFirstRail", "cfr", I, "Curves in the first rail: only 1 here."))
        .flag(f("flipLeftNormal", "fln", B, "Flip the tangent off the left."))
        .flag(f("flipRightNormal", "frn", B, "Flip the tangent off the right."))
        .flag(f("leftAnchor", "la", Fl, "Where a closed left rail starts."))
        .flag(f("leftEnd", "le", Fl, "Where it ends on the left (1)."))
        .flag(f("leftStart", "ls", Fl, "Where it starts on the left."))
        .flag(f("multipleKnots", "mk", B, "Held."))
        .flag(f("positionTolerance", "pt", Fl, "Held."))
        .flag(f("reverseLeft", "rvl", B, "With -an false: reverse off the left."))
        .flag(f("reverseRight", "rvr", B, "With -an false: reverse off the right."))
        .flag(f("rightAnchor", "ra", Fl, "Where a closed right rail starts."))
        .flag(f("rightEnd", "re", Fl, "Where it ends on the right (1)."))
        .flag(f("rightStart", "rs", Fl, "Where it starts on the right."))
        .flag(f("tangentTolerance", "tt", Fl, "Held.")),
    );
    c.register(
        make_flags(Cmd::new(
            "roundConstantRadius",
            "Constant radius fillets where surface edges meet: edges in pairs, surface.u[x] surface.v[y], -rad the radius (one for all, or one per pair; 1 by default). Returns the fillets, then the nodes. Less than the reference: it makes the fillets but does not trim the surfaces or build corners, and -a, -sa, -sb and -s (compound edges) fail.",
            round_constant_radius,
        ))
        .positional("edges in pairs")
        .flag(f("append", "a", B, "Not supported: fails."))
        .flag(f("radiuss", "rad", ArgType::Any, "The radius, one or one per pair.").multi(1))
        .flag(f("side", "s", ArgType::Any, "Not supported: fails."))
        .flag(f("sidea", "sa", I, "Not supported: fails."))
        .flag(f("sideb", "sb", I, "Not supported: fails.")),
    );
    register_types();
    let sec = |c: Cmd, name: &'static str, short: &'static str, doc: &'static str| c.flag(f(name, short, Fl, doc));

    c.register(make_flags(
        Cmd::new("filletCurve", "A fillet between two curves: -cir circular with -r radius (the default), or -fb free-form with -d, -b; -cp1, -cp2 where. Returns [curve, node].", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("filletCurve: name two curves") };
            let g = &mut s.graph;
            let (c1, c2) = (curves::curve_arg(g, x, "filletCurve")?, curves::curve_arg(g, y, "filletCurve")?);
            g.open_chunk("Fillet");
            let r = (|| -> CmdResult {
                let n = new_node(g, "filletCurve", "filletCurve1")?;
                connect_world_curve(g, c1, n, IN)?;
                connect_world_curve(g, c2, n, P)?;
                set_f(g, n, "radius", a.float("radius", 1.0))?;
                set_b(g, n, "circular", a.get("freeformBlend").is_none() && a.get("circular").and_then(Value::as_bool).unwrap_or(true))?;
                set_f(g, n, "depth", a.float("depth", 0.5))?;
                set_f(g, n, "bias", a.float("bias", 0.0))?;
                set_f(g, n, "curveParameter1", a.float("curveParameter1", -1.0))?;
                set_f(g, n, "curveParameter2", a.float("curveParameter2", -1.0))?;
                curve_from(g, a, n, plug(n, OUT), "filletCurve1")
            })();
            g.close_chunk();
            let out = r?;
            if a.bool("trim") || a.bool("join") {
                let g = &mut s.graph;
                g.evaluate();
                if let (Some(w1), Some(w2)) = (curves::world_curve(g, c1), curves::world_curve(g, c2)) {
                    if let Some((_, u, v)) = fillet_arc(&w1, &w2, a.float("radius", 1.0), None, None) {
                        for (sh, cu, at) in [(c1, w1, u), (c2, w2, v)] {
                            let (x0, x1) = cu.split(at);
                            let (lo, hi) = cu.domain();
                            let keep = if at - lo >= hi - at { x0 } else { x1 };
                            let w = crate::rig::world(g, crate::dag::transform_of(g, sh));
                            curves::bake_history(g, sh)?;
                            curves::set_cached(g, sh, keep.transformed(&w.inverse()))?;
                        }
                    }
                }
            }
            Ok(out)
        })
        .positional("two curves")
        .flag(f("radius", "r", Fl, "The radius (1)."))
        .flag(f("circular", "cir", B, "A circular fillet (the default)."))
        .flag(f("freeformBlend", "fb", Fl, "A free-form fillet."))
        .flag(f("depth", "d", Fl, "How far a free-form fillet reaches."))
        .flag(f("bias", "b", Fl, "Which way it leans."))
        .flag(f("blendControl", "bc", B, "Accepted for existing scripts."))
        .flag(f("curveParameter1", "cp1", Fl, "Where on the first curve."))
        .flag(f("curveParameter2", "cp2", Fl, "Where on the second curve."))
        .flag(f("join", "jn", B, "Trim the curves back too."))
        .flag(f("trim", "t", B, "Trim the curves back.")),
    ));
    c.register(make_flags(
        Cmd::new("bevel", "A bevelled wall round a curve: -w width, -d depth, -ed wall depth, -bst 1 straight or 2 arc. Returns [surface, node].", |s, a| {
            let names = objects(s, a);
            let Some(n) = names.first() else { fail!("bevel: name or select a curve") };
            let g = &mut s.graph;
            let shape = curves::curve_arg(g, n, "bevel")?;
            g.open_chunk("Bevel");
            let r = (|| -> CmdResult {
                let node = new_node(g, "bevel", "bevel1")?;
                connect_world_curve(g, shape, node, IN)?;
                set_f(g, node, "width", a.float("width", 0.1))?;
                set_f(g, node, "depth", a.float("depth", 0.1))?;
                set_f(g, node, "extrudeDepth", a.float("extrudeDepth", 1.0))?;
                set_i(g, node, "bevelShapeType", a.int("bevelShapeType", 1))?;
                surface_from(g, a, node, plug(node, OUT), "bevelSurface1")
            })();
            g.close_chunk();
            r
        })
        .positional("a curve")
        .flag(f("width", "w", Fl, "How far out (0.1)."))
        .flag(f("depth", "d", Fl, "How deep the bevel (0.1)."))
        .flag(f("extrudeDepth", "ed", Fl, "How deep the wall (1)."))
        .flag(f("bevelShapeType", "bst", I, "1 straight, 2 arc."))
        .flag(f("cornerType", "ct", I, "Accepted for existing scripts.")),
    ));
    c.register(make_flags(
        Cmd::new("bevelPlus", "A bevel with caps: -w, -d, -ed, -ns sides of the round, -cap 1 none, 2 front, 3 back, 4 both, -po 1 polygons (the default here). Returns [surface or group, node].", |s, a| {
            let names = objects(s, a);
            let Some(n) = names.first() else { fail!("bevelPlus: name or select a curve") };
            let g = &mut s.graph;
            let shape = curves::curve_arg(g, n, "bevelPlus")?;
            let polygon = a.int("polygon", 1) == 1;
            let world = curves::world_curve(g, shape).unwrap_or_default();
            let (w, d, ed) = (a.float("width", 0.1), a.float("depth", 0.1), a.float("extrudeDepth", 1.0));
            let cap = a.int("capSides", 4);
            let mut args = a.clone();
            args.flags.insert("polygon".into(), json!(if polygon { 1 } else { 0 }));
            g.open_chunk("Bevel Plus");
            let r = (|| -> CmdResult {
                let node = new_node(g, "bevelPlus", "bevelPlus1")?;
                connect_world_curve(g, shape, node, IN)?;
                set_f(g, node, "width", w)?;
                set_f(g, node, "depth", d)?;
                set_f(g, node, "extrudeDepth", ed)?;
                set_i(g, node, "numberOfSides", a.int("numberOfSides", 4))?;
                set_i(g, node, "capSides", cap)?;
                if polygon {
                    let (tr, sh) = crate::mesh_nodes::create_mesh_node(g, &a.string("name", "bevelPlusSurface1"))?;
                    g.connect(&plug(node, P + 5), &plug(sh, crate::mesh_nodes::MeshNode::IN_MESH), true)?;
                    g.select(&[tr], SelectMode::Replace);
                    g.evaluate();
                    if !surfaces::wants_history(g, a) {
                        crate::history::delete_history(g, sh)?;
                        return Ok(json!([g.path(tr)]));
                    }
                    return Ok(json!([g.path(tr), g.path(node)]));
                }
                let out = surface_from(g, &args, node, plug(node, OUT), "bevelPlusSurface1")?;
                if !closed_outline(&world) || cap == 1 {
                    return Ok(out);
                }
                // The caps fill the front and back.
                let up = {
                    let n = world.plane_normal();
                    if n == DVec3::ZERO { DVec3::Z } else { n }
                };
                let wall = out[0].as_str().unwrap_or("").to_string();
                let Some(wall_id) = g.find(&wall)? else { return Ok(out) };
                let mut extras: Vec<(f64, f64)> = Vec::new();
                if cap == 2 || cap == 4 {
                    extras.push((0.0, 0.0));
                }
                if cap == 3 || cap == 4 {
                    extras.push((0.0, -2.0 * d - ed));
                }
                let t = types::find("transform").unwrap();
                let group = g.create_node(&t, &a.string("name", "bevelPlus1Group"), NO_NODE, true)?;
                g.set_parent(wall_id, group, true, usize::MAX)?;
                for (k, &(o, z)) in extras.iter().enumerate() {
                    let outline = plane_offset(&world, o).transformed(&glam::DMat4::from_translation(up * z));
                    if let Some(p) = trimmed_plane(&outline) {
                        let (tr, _) = surfaces::create_surface(g, &format!("bevelPlusCap{}", k + 1), p)?;
                        g.set_parent(tr, group, true, usize::MAX)?;
                    }
                }
                g.select(&[group], SelectMode::Replace);
                g.evaluate();
                Ok(json!([g.path(group), g.path(node)]))
            })();
            g.close_chunk();
            r
        })
        .positional("a curve")
        .flag(f("width", "w", Fl, "How far out (0.1)."))
        .flag(f("depth", "d", Fl, "How deep the bevel (0.1)."))
        .flag(f("extrudeDepth", "ed", Fl, "How deep the wall (1)."))
        .flag(f("numberOfSides", "ns", I, "Sides of the round."))
        .flag(f("capSides", "cap", I, "1 none, 2 front, 3 back, 4 both."))
        .flag(f("innerStyle", "is", I, "Accepted for existing scripts."))
        .flag(f("outerStyle", "os", I, "Accepted for existing scripts.")),
    ));
    c.register(make_flags(
        Cmd::new("circularFillet", "A round fillet between two surfaces: -pr, -sr offsets from each (signed, on their normals). Returns [surface, node].", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("circularFillet: name two surfaces") };
            let g = &mut s.graph;
            let (s1, s2) = (surfaces::surface_arg(g, x, "circularFillet")?, surfaces::surface_arg(g, y, "circularFillet")?);
            let r0 = a.float("radius", 1.0);
            g.open_chunk("Circular Fillet");
            let r = (|| -> CmdResult {
                let n = new_node(g, "rbfSrf", "rbfSrf1")?;
                connect_world_surface(g, s1, n, IN)?;
                connect_world_surface(g, s2, n, P)?;
                set_f(g, n, "primaryRadius", a.float("primaryRadius", r0))?;
                set_f(g, n, "secondaryRadius", a.float("secondaryRadius", r0))?;
                surface_from(g, a, n, plug(n, OUT), "circularFillet1")
            })();
            g.close_chunk();
            r
        })
        .positional("two surfaces")
        .flag(f("radius", "r", Fl, "Both offsets (1)."))
        .flag(f("primaryRadius", "pr", Fl, "Offset from the first."))
        .flag(f("secondaryRadius", "sr", Fl, "Offset from the second."))
        .flag(f("curveOnSurface", "cos", B, "Accepted for existing scripts.")),
    ));
    c.register(make_flags(
        Cmd::new("freeFormFillet", "A blend between two edges (surface.u[x] or surface.v[x]) leaving each along its surface: -d depth, -bb bias. Returns [surface, node].", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("freeFormFillet: name two surface edges") };
            let g = &mut s.graph;
            let (e1, e2) = (edge_arg(g, x, "freeFormFillet")?, edge_arg(g, y, "freeFormFillet")?);
            let (Some((d1, p1)), Some((d2, p2))) = (e1.1, e2.1) else { fail!("freeFormFillet: name edges: surface.u[x] or surface.v[x]") };
            g.open_chunk("Free Form Fillet");
            let r = (|| -> CmdResult {
                let n = new_node(g, "ffFilletSrf", "ffFilletSrf1")?;
                connect_world_surface(g, e1.0, n, IN)?;
                connect_world_surface(g, e2.0, n, P)?;
                set_i(g, n, "direction1", d1)?;
                set_f(g, n, "parameter1", p1)?;
                set_i(g, n, "direction2", d2)?;
                set_f(g, n, "parameter2", p2)?;
                set_f(g, n, "depth", a.float("depth", 0.5))?;
                set_f(g, n, "bias", a.float("bias", 0.0))?;
                surface_from(g, a, n, plug(n, OUT), "freeFormFillet1")
            })();
            g.close_chunk();
            r
        })
        .positional("two edges")
        .flag(f("depth", "d", Fl, "How far the tangents reach (0.5)."))
        .flag(f("bias", "bb", Fl, "Which way it leans."))
        .flag(f("positionTolerance", "pt", Fl, "Accepted for existing scripts."))
        .flag(f("tangentTolerance", "tt", Fl, "Accepted for existing scripts.")),
    ));
    c.register(
        Cmd::new("curveIntersect", "Where two curves cross, as text: \"u1 u2 u1 u2 ...\" on the first and second. -ud -d flattens along a direction first; -tol how close.", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("curveIntersect: name two curves") };
            let g = &mut s.graph;
            let (c1, c2) = (curves::curve_arg(g, x, "curveIntersect")?, curves::curve_arg(g, y, "curveIntersect")?);
            let (Some(w1), Some(w2)) = (curves::world_curve(g, c1), curves::world_curve(g, c2)) else { fail!("curveIntersect: no curve") };
            let dir = a.bool("useDirection").then(|| a.vec3("direction").map_or(DVec3::Z, DVec3::from_array).normalize_or_zero());
            let hits = curve_intersections(&w1, &w2, a.float("tolerance", 1e-3), dir);
            let mut t = String::new();
            for (u, v) in hits {
                t.push_str(&format!("{u} {v} "));
            }
            Ok(json!(t))
        })
        .positional("two curves")
        .flag(f("tolerance", "tol", Fl, "How close counts (0.001)."))
        .flag(f("useDirection", "ud", B, "Flatten along -d first."))
        .flag(f("direction", "d", ArgType::Vec3, "Which way."))
        .flag(f("constructionHistory", "ch", B, "Accepted for existing scripts."))
        .undoable(false),
    );
    c.register(make_flags(
        Cmd::new("intersect", "Curves where two surfaces cross, sampled from their tessellation: one object per run, -cos on the first surface. Returns [curves..., node].", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("intersect: name two surfaces") };
            let g = &mut s.graph;
            let (s1, s2) = (surfaces::surface_arg(g, x, "intersect")?, surfaces::surface_arg(g, y, "intersect")?);
            g.open_chunk("Intersect");
            let r = (|| -> CmdResult {
                let n = new_node(g, "intersectSurface", "intersectSurface1")?;
                connect_world_surface(g, s1, n, IN)?;
                connect_world_surface(g, s2, n, P)?;
                g.evaluate();
                let count = g.node(n).unwrap().elements(OUT).len();
                if count == 0 {
                    fail!("intersect: the surfaces don't cross");
                }
                let mut out = Vec::new();
                let cos = a.bool("curveOnSurface");
                for i in 0..count {
                    let el = PlugRef::element(n, OUT as i32, i as i32);
                    if cos {
                        let w = g.node(n).unwrap().elements(OUT).iter().find(|e| e.0 == i as i32).and_then(|e| curve_value(&e.1).cloned()).unwrap_or_default();
                        let Some(surf) = surfaces::world_surface(g, s1) else { continue };
                        let uv: Vec<DVec2> = w.cvs.iter().map(|p| { let (u, v) = surf.closest_point(*p); DVec2::new(u, v) }).collect();
                        let (tr, _) = make_cos(g, s1, uv_curve(&uv), a)?;
                        out.push(json!(cos_name(g, s1, tr)));
                    } else {
                        let (tr, sh) = crate::dag::create_with_shape(g, curves::SHAPE_TYPE, &a.string("name", "intersectionCurve1"), NO_NODE)?;
                        g.connect(&el, &plug(sh, CurveShape::CREATE), false)?;
                        out.push(json!(g.path(tr)));
                    }
                }
                g.evaluate();
                if surfaces::wants_history(g, a) {
                    out.push(json!(g.path(n)));
                }
                Ok(Value::Array(out))
            })();
            g.close_chunk();
            r
        })
        .positional("two surfaces")
        .flag(f("curveOnSurface", "cos", B, "As curves on the first surface."))
        .flag(f("firstSurface", "fs", B, "Accepted for existing scripts.")),
    ));
    c.register(make_flags(
        Cmd::new("projectCurve", "A curve's shadow on a surface as a curve on it: -d the way (0 0 1), -un along the normal. Returns [surface->curve, node].", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("projectCurve: name a curve and a surface") };
            let g = &mut s.graph;
            let (c1, s1) = (curves::curve_arg(g, x, "projectCurve")?, surfaces::surface_arg(g, y, "projectCurve")?);
            g.open_chunk("Project Curve");
            let r = (|| -> CmdResult {
                let n = new_node(g, "projectCurve", "projectCurve1")?;
                connect_world_curve(g, c1, n, IN)?;
                connect_world_surface(g, s1, n, P)?;
                if let Some(d) = a.vec3("direction") {
                    set_v(g, n, "direction", DVec3::from_array(d))?;
                }
                set_b(g, n, "useNormal", a.bool("useNormal"))?;
                set_i(g, n, "curveSamples", a.int("curveSamples", 50))?;
                let cos = new_node(g, "curveFromSurfaceCoS", "curveFromSurfaceCoS1")?;
                connect_world_surface(g, s1, cos, IN)?;
                g.connect(&plug(n, OUT), &plug(cos, P), false)?;
                let (tr, sh) = crate::dag::create_with_shape(g, curves::SHAPE_TYPE, &a.string("name", "curve1"), NO_NODE)?;
                g.connect(&plug(cos, OUT), &plug(sh, CurveShape::CREATE), false)?;
                g.evaluate();
                Ok(json!([cos_name(g, s1, tr), g.path(n)]))
            })();
            g.close_chunk();
            r
        })
        .positional("a curve and a surface")
        .flag(f("direction", "d", ArgType::Vec3, "Which way."))
        .flag(f("useNormal", "un", B, "Along the surface's normal."))
        .flag(f("curveSamples", "cs", I, "How many samples (50).")),
    ));
    c.register(curves::with_common(
        Cmd::new("projectTangent", "Turn a curve's end to follow what it meets: another curve or a surface. -ts scale, -ro rotate, -rt reverse, -td 1 U or 2 V. Returns [curve, node].", |s, a| {
            let names = objects(s, a);
            let [x, y] = names.as_slice() else { fail!("projectTangent: name a curve and what to follow") };
            let g = &mut s.graph;
            let shape = curves::curve_arg(g, x, "projectTangent")?;
            let target = find(g, y.split('.').next().unwrap_or(y), "projectTangent")?;
            g.open_chunk("Project Tangent");
            let r = (|| -> CmdResult {
                let node = curves::insert_op(g, shape, "projectTangent", &[])?;
                if let Some(t) = curves::as_curve(g, target) {
                    connect_world_curve(g, t, node, P)?;
                } else if let Some(t) = surfaces::as_surface(g, target) {
                    connect_world_surface(g, t, node, P + 1)?;
                } else {
                    fail!("projectTangent: {y} isn't a curve or surface");
                }
                set_f(g, node, "tangentScale", a.float("tangentScale", 1.0))?;
                set_f(g, node, "rotate", a.float("rotate", 0.0))?;
                set_b(g, node, "reverseTangent", a.bool("reverseTangent"))?;
                set_i(g, node, "tangentDirection", a.int("tangentDirection", 1))?;
                g.evaluate();
                if !surfaces::wants_history(g, a) {
                    curves::bake_history(g, shape)?;
                    return Ok(json!([g.path(crate::dag::transform_of(g, shape))]));
                }
                Ok(json!([g.path(crate::dag::transform_of(g, shape)), g.path(node)]))
            })();
            g.close_chunk();
            r
        })
        .positional("a curve and what to follow")
        .flag(f("tangentScale", "tsc", Fl, "How far the next CV goes (1)."))
        .flag(f("rotate", "ro", Fl, "Degrees."))
        .flag(f("reverseTangent", "rt", B, "The other way."))
        .flag(f("tangentDirection", "td", I, "1 U or 2 V."))
        .flag(f("curvature", "cvt", B, "Accepted for existing scripts."))
        .flag(f("ignoreEdges", "ie", B, "Accepted for existing scripts.")),
    ));
    c.register(
        Cmd::new("curveOnSurface", "A curve on a surface: -d degree, -uv u v for each point, -k knots, -per periodic. Returns surface->curve.", curve_on_surface)
            .positional("a surface")
            .flag(f("degree", "d", I, "The degree (3)."))
            .flag(f("positionUV", "uv", ArgType::Any, "A point.").multi(2))
            .flag(f("knot", "k", ArgType::Any, "A knot.").multi(1))
            .flag(f("periodic", "per", B, "Closed."))
            .flag(f("name", "n", S, "The curve's name."))
            .flag(f("append", "a", B, "Accepted for existing scripts."))
            .flag(f("replace", "r", B, "Accepted for existing scripts."))
            .flag(f("positionTolerance", "pt", Fl, "Accepted for existing scripts."))
            .flag(f("tangentTolerance", "tt", Fl, "Accepted for existing scripts.")),
    );
    c.register(make_flags(
        Cmd::new("trim", "Trim a surface along closed curves on it: the region holding -lu -lv (0 to 1, the middle by default) stays, or with -sl 1 goes. Returns [surface, node].", trim_cmd)
            .positional("a surface, then curves on it")
            .flag(f("locatorU", "lu", Fl, "Where, in U (0.5)."))
            .flag(f("locatorV", "lv", Fl, "Where, in V (0.5)."))
            .flag(f("selected", "sl", I, "0 keeps that region, 1 throws it away."))
            .flag(f("shrink", "sh", B, "Accepted for existing scripts.")),
    ));
    c.register(surfaces::with_common(
        Cmd::new("untrim", "Take a surface's trims off. Returns [surface, node].", |s, a| surfaces::run_op(s, a, "untrim", "untrim", &[]))
            .positional("surfaces (the selection when none)")
            .flag(f("curveOnSurface", "cos", B, "Accepted for existing scripts."))
            .flag(f("noChanges", "nc", B, "Accepted for existing scripts.")),
    ));
    c.register(make_flags(
        Cmd::new("stitchSurface", "Close the gap between two surfaces' edges (surface.u[x] or the nearest): -b 0.5 meets halfway, 0 moves only the second, 1 only the first. Returns the surfaces and nodes.", stitch_surface)
            .positional("two surfaces or edges")
            .flag(f("bias", "b", Fl, "Which moves (0.5)."))
            .flag(f("cascade", "c", B, "Accepted for existing scripts."))
            .flag(f("keepG0Continuity", "kg0", B, "Accepted for existing scripts."))
            .flag(f("keepG1Continuity", "kg1", B, "Accepted for existing scripts."))
            .flag(f("numberOfSamples", "ns", I, "Accepted for existing scripts."))
            .flag(f("stepCount", "sc", I, "Accepted for existing scripts."))
            .flag(f("fixBoundary", "fb", B, "Accepted for existing scripts."))
            .flag(f("replaceOriginal", "rpo", B, "Accepted: the surfaces themselves change.")),
    ));
    c.register(
        Cmd::new("stitchSurfacePoints", "Move CVs to one place: their mean, or -ew 0 the first. Returns them.", stitch_points)
            .positional("CVs")
            .flag(f("equalWeight", "ew", ArgType::Any, "Their mean (the default), or the first."))
            .flag(f("bias", "b", Fl, "Accepted for existing scripts."))
            .flag(f("constructionHistory", "ch", B, "Accepted for existing scripts."))
            .flag(f("cascade", "c", B, "Accepted for existing scripts."))
            .flag(f("keepG0Continuity", "kg0", B, "Accepted for existing scripts."))
            .flag(f("keepG1Continuity", "kg1", B, "Accepted for existing scripts."))
            .flag(f("replaceOriginal", "rpo", B, "Accepted: they change in place."))
            .flag(f("stepCount", "sc", I, "Accepted for existing scripts."))
            .flag(f("tolerance", "tol", Fl, "Accepted for existing scripts.")),
    );
    c.register(make_flags(
        Cmd::new("globalStitch", "Stitch a whole set of surfaces: every pair of edges within -ms of each other. Returns the surfaces and nodes.", global_stitch)
            .positional("surfaces")
            .flag(f("maxSeparation", "ms", Fl, "How far apart edges may be (0.1)."))
            .flag(f("lockSurface", "lk", B, "Accepted for existing scripts."))
            .flag(f("modificationResistance", "mr", Fl, "Accepted for existing scripts."))
            .flag(f("sampling", "sam", I, "Accepted for existing scripts."))
            .flag(f("stitchCorners", "sc", I, "Accepted for existing scripts."))
            .flag(f("stitchEdges", "se", I, "Accepted for existing scripts."))
            .flag(f("stitchPartialEdges", "spe", B, "Accepted for existing scripts."))
            .flag(f("stitchSmoothness", "ss", I, "Accepted for existing scripts.")),
    ));
    c.register(make_flags(
        Cmd::new("extendSurface", "Lengthen a surface: -d distance, -ed 0 U, 1 V, 2 both, -es 0 start, 1 end, 2 both, -et 0 tangent. Returns [surface, node].", |s, a| {
            let set = [(P, PlugValue::Float(a.float("distance", 1.0))), (P + 1, PlugValue::Int(a.int("extendDirection", 0))), (P + 2, PlugValue::Int(a.int("extendSide", 1))), (P + 3, PlugValue::Int(a.int("extensionType", 0)))];
            surfaces::run_op(s, a, "extendSurface", "extendSurface", &set)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("distance", "d", Fl, "How far (1)."))
        .flag(f("extendDirection", "ed", I, "0 U, 1 V, 2 both."))
        .flag(f("extendSide", "es", I, "0 start, 1 end, 2 both."))
        .flag(f("extensionType", "et", I, "Accepted: tangent lines."))
        .flag(f("extendMethod", "em", I, "Accepted for existing scripts."))
        .flag(f("join", "jn", B, "Accepted for existing scripts."))
        .flag(f("replaceOriginal", "rpo", B, "Change the surface itself (on by default).")),
    ));
    c.register(make_flags(
        Cmd::new("duplicateSurface", "A copy of a surface or of patches of it, surface.sf[0][1]: -l keeps following it. Returns [surface, node].", |s, a| {
            let names = objects(s, a);
            let Some(name) = names.first() else { fail!("duplicateSurface: name a surface") };
            let g = &mut s.graph;
            let (base, part) = match name.split_once(".sf[") {
                Some((b, p)) => (b.to_string(), Some(p.trim_end_matches(']').to_string())),
                None => (name.clone(), None),
            };
            let shape = surfaces::surface_arg(g, &base, "duplicateSurface")?;
            let Some(surf) = surfaces::local_surface(g, shape) else { fail!("duplicateSurface: no surface") };
            let range = |t: &str, max: usize| -> (i64, i64) {
                match (t, t.split_once(':')) {
                    ("*", _) => (0, max as i64),
                    (_, Some((a, b))) => {
                        let (x, y) = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
                        (x, y - x + 1)
                    }
                    _ => (t.parse().unwrap_or(0), 1),
                }
            };
            let (fu, cu, fv, cv) = match &part {
                Some(p) => {
                    let (a0, b0) = p.split_once("][").unwrap_or((p, "*"));
                    let (x, y) = (range(a0, surf.spans_u()), range(b0, surf.spans_v()));
                    (x.0, x.1, y.0, y.1)
                }
                None => (0, surf.spans_u() as i64, 0, surf.spans_v() as i64),
            };
            g.open_chunk("Duplicate Surface");
            let r = (|| -> CmdResult {
                let n = new_node(g, "subSurface", "subSurface1")?;
                connect_world_surface(g, shape, n, IN)?;
                set_i(g, n, "firstFaceU", fu)?;
                set_i(g, n, "firstFaceV", fv)?;
                set_i(g, n, "faceCountU", cu)?;
                set_i(g, n, "faceCountV", cv)?;
                let mut args = a.clone();
                if a.bool("local") {
                    args.flags.insert("constructionHistory".into(), json!(true));
                }
                surface_from(g, &args, n, plug(n, OUT), "duplicatedSurface1")
            })();
            g.close_chunk();
            r
        })
        .positional("a surface or patches")
        .flag(f("local", "l", B, "Keep following it."))
        .flag(f("mergeItems", "mi", B, "Accepted for existing scripts.")),
    ));
    c.register(
        Cmd::new("surface", "A surface from raw data: -du -dv degrees, -fu -fv open, closed or periodic, -ku -kv one knot each, -p x y z one point each (or -pw with a weight), V running fastest.", raw_surface)
            .flag(f("degreeU", "du", I, "Degree in U."))
            .flag(f("degreeV", "dv", I, "Degree in V."))
            .flag(f("formU", "fu", ArgType::Any, "open, closed or periodic."))
            .flag(f("formV", "fv", ArgType::Any, "open, closed or periodic."))
            .flag(f("knotU", "ku", ArgType::Any, "A knot in U.").multi(1))
            .flag(f("knotV", "kv", ArgType::Any, "A knot in V.").multi(1))
            .flag(f("point", "p", ArgType::Any, "A CV.").multi(3))
            .flag(f("pointWeight", "pw", ArgType::Any, "A CV with a weight, dropped: surfaces aren't rational here.").multi(4))
            .flag(f("name", "n", S, "The surface's name."))
            .flag(f("objectSpace", "ob", B, "Accepted for existing scripts."))
            .flag(f("worldSpace", "ws", B, "Accepted for existing scripts.")),
    );
    c.register(
        Cmd::new("plane", "A construction plane: -s size, -w -lr each way, -p where, -r rotation. Returns its transform.", make_plane)
            .flag(f("size", "s", Fl, "Its size (1)."))
            .flag(f("width", "w", Fl, "Its width."))
            .flag(f("length", "l", Fl, "Its length."))
            .flag(f("position", "p", ArgType::Vec3, "Where."))
            .flag(f("rotation", "r", ArgType::Vec3, "How turned, degrees."))
            .flag(f("name", "n", S, "Its name.")),
    );
    for (name, from, doc) in [("squareSurface", "square", "A surface from four curves in order round its edge: square under its other name."), ("planarSrf", "planar", "A trimmed plane in a closed curve: planar under its node's name.")] {
        if let Some(mut def) = c.find(from).cloned() {
            def.name = name.to_string();
            def.doc = doc.to_string();
            c.register(def);
        }
    }
    c.register(surfaces::with_common(
        Cmd::new("smoothTangentSurface", "Line a surface's CVs up across the knot at -p so it's smooth there: -d 1 U, 0 V. Returns [surface, node].", |s, a| {
            let set = [(P, PlugValue::Float(a.float("parameter", 0.0))), (P + 1, PlugValue::Int(a.int("direction", 1))), (P + 2, PlugValue::Int(a.int("smoothness", 1)))];
            surfaces::run_op(s, a, "smoothTangentSurface", "smoothTangentSrf", &set)
        })
        .positional("surfaces (the selection when none)")
        .flag(f("parameter", "p", Fl, "Which isoparm."))
        .flag(f("direction", "d", I, "1 U, 0 V."))
        .flag(f("smoothness", "s", I, "Accepted for existing scripts.")),
    ));
    c.register(curves::with_common(
        Cmd::new("hardenPointCurve", "Make a curve pass through CVs by raising the knot multiplicity: curve.cv[i], -m how many (-1 up to the degree). Returns [curve, node].", |s, a| {
            let names = objects(s, a);
            let Some(first) = names.first() else { fail!("hardenPointCurve: name CVs") };
            let g = &mut s.graph;
            let members = crate::object_sets::parse(g, &names).map_err(aztek_ops::CommandError::new)?;
            let Some(mem) = members.first() else { fail!("hardenPointCurve: no CVs in {first}") };
            let shape = mem.node;
            let ids = mem.ids.clone();
            g.open_chunk("Harden");
            let r = (|| -> CmdResult {
                let node = curves::insert_op(g, shape, "hardenPointCurve", &[(P, PlugValue::Data(Some(Arc::new(crate::rig::JsonData(json!(ids)))))), (P + 1, PlugValue::Int(a.int("multiplicity", -1)))])?;
                g.evaluate();
                Ok(json!([g.path(crate::dag::transform_of(g, shape)), g.path(node)]))
            })();
            g.close_chunk();
            r
        })
        .positional("CVs (the selection when none)")
        .flag(f("multiplicity", "m", I, "How many knots, -1 as many as the degree.")),
    ));
    c.register(curves::with_common(
        Cmd::new("offsetCurveOnSurface", "A curve on a surface offset sideways: -d distance in parameter space. Returns [surface->curve, node].", |s, a| {
            let names = objects(s, a);
            let Some(n) = names.first() else { fail!("offsetCurveOnSurface: name a curve on a surface") };
            let g = &mut s.graph;
            let shape = curves::curve_arg(g, n, "offsetCurveOnSurface")?;
            let Some(src) = g.source(&plug(shape, CurveShape::CREATE)).filter(|x| g.node(x.node).is_some_and(|m| m.ty().name == "curveFromSurfaceCoS")) else { fail!("offsetCurveOnSurface: {n} isn't a curve on a surface") };
            let Some(surf) = g.source(&plug(src.node, IN)) else { fail!("offsetCurveOnSurface: {n} has lost its surface") };
            g.open_chunk("Offset Curve On Surface");
            let r = (|| -> CmdResult {
                let off = new_node(g, "offsetCurveOnSurface", "offsetCurveOnSurface1")?;
                let uv = g.value(&plug(src.node, P))?;
                g.set_value(plug(off, IN), uv)?;
                set_f(g, off, "distance", a.float("distance", 1.0))?;
                let cos = new_node(g, "curveFromSurfaceCoS", "curveFromSurfaceCoS1")?;
                g.connect(&surf, &plug(cos, IN), false)?;
                g.connect(&plug(off, OUT), &plug(cos, P), false)?;
                let (tr, sh) = crate::dag::create_with_shape(g, curves::SHAPE_TYPE, &a.string("name", "curve1"), NO_NODE)?;
                g.connect(&plug(cos, OUT), &plug(sh, CurveShape::CREATE), false)?;
                g.evaluate();
                Ok(json!([cos_name(g, surf.node, tr), g.path(off)]))
            })();
            g.close_chunk();
            r
        })
        .positional("a curve on a surface")
        .flag(f("distance", "d", Fl, "How far.")),
    ));
    c.register(make_flags(Cmd::new("makeSingleSurface", "Join surfaces meeting edge to edge into one. Returns [surface].", make_single_surface).positional("surfaces").flag(f("stitchTolerance", "st", Fl, "Accepted for existing scripts."))));
    c.register(
        Cmd::new("nurbsSelect", "Pick a surface's CVs: -bs the border, -lb -rb -tb -bb one side, -gs -ss grow or shrink what's picked.", nurbs_select)
            .positional("surfaces (the selection when none)")
            .flag(f("borderSelection", "bs", B, "The whole border."))
            .flag(f("leftBorder", "lb", B, "The first row in U."))
            .flag(f("rightBorder", "rb", B, "The last row in U."))
            .flag(f("bottomBorder", "bb", B, "The first row in V."))
            .flag(f("topBorder", "tb", B, "The last row in V."))
            .flag(f("growSelection", "gs", I, "Rings to add."))
            .flag(f("shrinkSelection", "ss", I, "Rings to take off.")),
    );
    c.register(
        Cmd::new("nurbsBoolean", "A boolean of two surfaces, as a polygon mesh: there are no NURBS solids to trim back. -op 0 union, 1 difference, 2 intersection.", nurbs_boolean)
            .positional("two surfaces")
            .flag(f("operation", "op", I, "0 union, 1 difference, 2 intersection."))
            .flag(f("smartConnection", "sc", B, "Accepted for existing scripts."))
            .flag(f("constructionHistory", "ch", B, "Accepted for existing scripts."))
            .flag(f("nsrfsInFirstShell", "nsf", I, "Accepted for existing scripts."))
            .flag(f("name", "n", S, "Accepted for existing scripts.")),
    );
    c.register(Cmd::new("textCurves", "Text as curves: -t words, -f font accepted (one stroke font). A group, a transform per letter, a curve per stroke. Returns [group, node].", text_curves).flag(f("text", "t", S, "The words.")).flag(f("font", "f", S, "Accepted: one stroke font here.")).flag(f("name", "n", S, "The group's name.")).flag(f("object", "o", B, "Accepted for existing scripts.")));
    c.register(sec(
        q_flags(Cmd::new("threePointArc", "A circular arc through -p1 -p2 -p3: -d degree, -s sections. Returns [curve, node]. -q -c, -q -r on the node read centre and radius.", |s, a| {
            let names = a.named();
            let g = &mut s.graph;
            if a.bool("query") {
                let Some(n) = names.first().and_then(|x| g.find(x).ok().flatten()) else { fail!("threePointArc: name the node") };
                let (p1, p2, p3) = (g.value(&plug(n, IN))?, g.value(&plug(n, P))?, g.value(&plug(n, P + 1))?);
                let as_v = |v: &PlugValue| if let PlugValue::Vec3(x) = v { *x } else { DVec3::ZERO };
                let Some((c, r, nr)) = arc_centre(as_v(&p1), as_v(&p2), as_v(&p3)) else { fail!("threePointArc: the points lie on a line") };
                if asked(a, "center") {
                    return Ok(json!([c.x, c.y, c.z]));
                }
                if asked(a, "radius") {
                    return Ok(json!(r));
                }
                if asked(a, "normal") {
                    return Ok(json!([nr.x, nr.y, nr.z]));
                }
                fail!("threePointArc: query what?");
            }
            let (Some(p1), Some(p2), Some(p3)) = (a.vec3("point1"), a.vec3("point2"), a.vec3("point3")) else { fail!("threePointArc: give -p1 -p2 -p3") };
            g.open_chunk("Three Point Arc");
            let r = (|| -> CmdResult {
                let n = new_node(g, "makeThreePointCircularArc", "makeThreePointCircularArc1")?;
                set_v(g, n, "point1", DVec3::from_array(p1))?;
                set_v(g, n, "point2", DVec3::from_array(p2))?;
                set_v(g, n, "point3", DVec3::from_array(p3))?;
                set_i(g, n, "degree", a.int("degree", 3))?;
                set_i(g, n, "sections", a.int("sections", 8))?;
                curve_from(g, a, n, plug(n, OUT), "curve1")
            })();
            g.close_chunk();
            r
        }))
        .positional("the node, with -q")
        .flag(f("point1", "p1", ArgType::Vec3, "Start."))
        .flag(f("point2", "p2", ArgType::Vec3, "Middle."))
        .flag(f("point3", "p3", ArgType::Vec3, "End."))
        .flag(f("degree", "d", I, "The degree (3)."))
        .flag(f("sections", "s", I, "The sections (8)."))
        .flag(f("center", "c", B, "With -q: the centre."))
        .flag(f("normal", "nr", B, "With -q: the normal."))
        .flag(f("constructionHistory", "ch", B, "Keep the node (on by default)."))
        .flag(f("name", "n", S, "The curve's name."))
        .flag(f("object", "o", B, "Accepted for existing scripts."))
        .flag(f("spans", "sp", I, "Accepted for existing scripts.")),
        "radius",
        "r",
        "With -q: the radius.",
    ));
    c.register(
        Cmd::new("twoPointArc", "A circular arc between -p1 and -p2 with radius -r on the side -dv's normal gives: -tac the long way round, -d degree, -s sections. Returns [curve, node].", |s, a| {
            let (Some(p1), Some(p2)) = (a.vec3("point1"), a.vec3("point2")) else { fail!("twoPointArc: give -p1 -p2") };
            let g = &mut s.graph;
            g.open_chunk("Two Point Arc");
            let r = (|| -> CmdResult {
                let n = new_node(g, "makeTwoPointCircularArc", "makeTwoPointCircularArc1")?;
                set_v(g, n, "point1", DVec3::from_array(p1))?;
                set_v(g, n, "point2", DVec3::from_array(p2))?;
                if let Some(d) = a.vec3("directionVector") {
                    set_v(g, n, "directionVector", DVec3::from_array(d))?;
                }
                set_f(g, n, "radius", a.float("radius", 1.0))?;
                set_b(g, n, "toggleArc", a.bool("toggleArc"))?;
                set_i(g, n, "degree", a.int("degree", 3))?;
                set_i(g, n, "sections", a.int("sections", 8))?;
                curve_from(g, a, n, plug(n, OUT), "curve1")
            })();
            g.close_chunk();
            r
        })
        .flag(f("point1", "p1", ArgType::Vec3, "Start."))
        .flag(f("point2", "p2", ArgType::Vec3, "End."))
        .flag(f("directionVector", "dv", ArgType::Vec3, "The arc's normal."))
        .flag(f("radius", "r", Fl, "The radius (1)."))
        .flag(f("toggleArc", "tac", B, "The long way round."))
        .flag(f("degree", "d", I, "The degree (3)."))
        .flag(f("sections", "s", I, "The sections (8)."))
        .flag(f("constructionHistory", "ch", B, "Keep the node (on by default)."))
        .flag(f("name", "n", S, "The curve's name."))
        .flag(f("object", "o", B, "Accepted for existing scripts.")),
    );
    c.register(
        Cmd::new("pointCurveConstraint", "Make a curve pass through a point at curve.u[x] or curve.ep[i]: -p position (where it is now), -w weight. Returns [curve, node].", |s, a| {
            let names = objects(s, a);
            let Some(n) = names.first() else { fail!("pointCurveConstraint: name a point on a curve") };
            let g = &mut s.graph;
            let Some((head, rest)) = n.split_once('[') else { fail!("pointCurveConstraint: name a point: curve.u[x] or curve.ep[i]") };
            let Some((obj, kind)) = head.rsplit_once('.') else { fail!("pointCurveConstraint: name a point: curve.u[x] or curve.ep[i]") };
            let shape = curves::curve_arg(g, obj, "pointCurveConstraint")?;
            let Some(cu) = curves::local_curve(g, shape) else { fail!("pointCurveConstraint: no curve") };
            let inner = rest.trim_end_matches(']');
            let param = if kind == "ep" { cu.distinct_params().get(inner.parse::<usize>().unwrap_or(usize::MAX)).copied() } else { inner.parse().ok() };
            let Some(param) = param else { fail!("pointCurveConstraint: no such point: {n}") };
            let here = a.vec3("position").map_or(cu.eval(param), DVec3::from_array);
            g.open_chunk("Point Curve Constraint");
            let r = (|| -> CmdResult {
                let node = curves::insert_op(g, shape, "pointCurveConstraint", &[(P, PlugValue::Float(param)), (P + 1, PlugValue::Vec3(here)), (P + 2, PlugValue::Float(a.float("weight", 1.0)))])?;
                g.evaluate();
                Ok(json!([g.path(crate::dag::transform_of(g, shape)), g.path(node)]))
            })();
            g.close_chunk();
            r
        })
        .positional("a point on a curve")
        .flag(f("position", "p", ArgType::Vec3, "Where it goes."))
        .flag(f("weight", "w", Fl, "How much (1)."))
        .flag(f("pointWeight", "pw", Fl, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", B, "Keep the node (on by default)."))
        .flag(f("replaceOriginal", "rpo", B, "Accepted: the curve itself changes."))
        .flag(f("name", "n", S, "Accepted for existing scripts."))
        .flag(f("object", "o", B, "Accepted for existing scripts.")),
    );
}
