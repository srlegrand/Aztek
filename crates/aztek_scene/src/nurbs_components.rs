//! NURBS components other than CVs: a curve's edit points (`curve1.ep[0]`) and curve points (`curve1.u[0.5]`), a surface's
//! edit points (`surface1.ep[0][1]`), isoparms (`surface1.u[0.5]`, `surface1.v[1]`) and patches (`surface1.sf[0][0]`).
//!
//! They share the vertex selection with CVs, as high component ids. Edit points can be moved: as the reference's curveMoveEPCtx does,
//! the CVs shift so the curve passes through the new place, least moving those that bear on it most. Curve points, isoparms
//! and patches can be selected and asked about, not moved.

use aztek_geo::nurbs::{Form, NurbsCurve, NurbsSurface};
use aztek_ops::{CmdResult, Args};
use glam::DVec3;
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::plug::NodeId;

/// Edit point: EP | index, or on a surface EP | u << 12 | v.
pub const EP: u32 = 1 << 24;
/// Curve point or U isoparm: CURVE_U | parameter * 1000. V isoparm: ISO_V.
const CURVE_U: u32 = 1 << 25;
const ISO_V: u32 = 1 << 26;
/// Patch: PATCH | u << 12 | v.
const PATCH: u32 = 1 << 27;

pub fn is_encoded(id: u32) -> bool {
    id >= EP
}

fn param_id(base: u32, x: f64) -> u32 {
    base | ((x * 1000.0).round().max(0.0) as u32 & 0x00FF_FFFF)
}

fn param_of(id: u32) -> f64 {
    (id & 0x00FF_FFFF) as f64 / 1000.0
}

/// Trims a parameter's text to what was typed: 0.5, 1.
fn param_text(x: f64) -> String {
    let t = format!("{x:.3}");
    t.trim_end_matches('0').trim_end_matches('.').to_string()
}

fn int_range(part: &str, count: u32) -> Option<std::ops::RangeInclusive<u32>> {
    if part == "*" {
        return (count > 0).then(|| 0..=count - 1);
    }
    match part.split_once(':') {
        Some((a, b)) => Some(a.parse().ok()?..=b.parse().ok()?),
        None => {
            let i: u32 = part.parse().ok()?;
            Some(i..=i)
        }
    }
}

/// How many edit points a surface has each way.
fn surface_ep_counts(s: &NurbsSurface) -> (u32, u32) {
    let count = |k: &[f64], lo: f64, hi: f64| -> u32 {
        let mut n = 0;
        let mut last = f64::MIN;
        for &x in k {
            if x >= lo - 1e-9 && x <= hi + 1e-9 && (x - last).abs() > 1e-9 {
                n += 1;
                last = x;
            }
        }
        n
    };
    let (ul, uh) = s.domain_u();
    let (vl, vh) = s.domain_v();
    (count(&s.knots_u, ul, uh), count(&s.knots_v, vl, vh))
}

fn distinct(k: &[f64], lo: f64, hi: f64) -> Vec<f64> {
    let mut out: Vec<f64> = Vec::new();
    for &x in k {
        if x >= lo - 1e-9 && x <= hi + 1e-9 && out.last().is_none_or(|l| (x - *l).abs() > 1e-9) {
            out.push(x);
        }
    }
    out
}

fn surface_params(s: &NurbsSurface) -> (Vec<f64>, Vec<f64>) {
    let (ul, uh) = s.domain_u();
    let (vl, vh) = s.domain_v();
    (distinct(&s.knots_u, ul, uh), distinct(&s.knots_v, vl, vh))
}

/// The ids of curve1.ep[...] or curve1.u[...]. None for anything else.
pub fn parse_curve(g: &Graph, shape: NodeId, suffix: &str, inner: &str) -> Option<Vec<u32>> {
    let c = crate::curves::drawn_curve(g.node(shape)?)?;
    match suffix {
        "ep" => {
            let count = c.distinct_params().len() as u32;
            Some(int_range(inner, count)?.filter(|&i| i < count).map(|i| EP | i).collect())
        }
        "u" => {
            if inner == "*" {
                let (lo, hi) = c.domain();
                return Some(vec![param_id(CURVE_U, lo), param_id(CURVE_U, hi)]);
            }
            Some(vec![param_id(CURVE_U, inner.parse().ok()?)])
        }
        _ => None,
    }
}

/// The ids of surface1.ep[...][...], .u[...], .v[...], .sf[...][...]. None for anything else.
pub fn parse_surface(g: &Graph, shape: NodeId, suffix: &str, inner: &str) -> Option<Vec<u32>> {
    let s = crate::surfaces::drawn_surface(g.node(shape)?)?;
    let two = |nu: u32, nv: u32, base: u32| -> Option<Vec<u32>> {
        let (a, b) = inner.split_once("][").unwrap_or((inner, "*"));
        let mut out = Vec::new();
        for u in int_range(a, nu)? {
            for v in int_range(b, nv)? {
                if u < nu && v < nv {
                    out.push(base | u << 12 | v);
                }
            }
        }
        Some(out)
    };
    match suffix {
        "ep" => {
            let (nu, nv) = surface_ep_counts(s);
            two(nu, nv, EP)
        }
        "sf" => two(s.spans_u() as u32, s.spans_v() as u32, PATCH),
        "u" => Some(vec![param_id(CURVE_U, inner.parse().ok()?)]),
        "v" => Some(vec![param_id(ISO_V, inner.parse().ok()?)]),
        _ => None,
    }
}

/// A component id as its text after the object: ep[2], u[0.5], sf[0][1].
pub fn format_id(g: &Graph, node: NodeId, id: u32) -> Option<String> {
    let curve = g.node(node)?.ty().is_a("nurbsCurve");
    Some(if id & PATCH != 0 {
        format!("sf[{}][{}]", (id & !PATCH) >> 12, id & 0xFFF)
    } else if id & ISO_V != 0 {
        format!("v[{}]", param_text(param_of(id)))
    } else if id & CURVE_U != 0 {
        format!("u[{}]", param_text(param_of(id)))
    } else if curve {
        format!("ep[{}]", id & !EP)
    } else {
        format!("ep[{}][{}]", (id & !EP) >> 12, id & 0xFFF)
    })
}

/// ls -flatten's one string per component, for the kinds here.
pub fn flatten(g: &mut Graph, id: NodeId, obj: &str, kind: &str, inner: &str) -> Option<Vec<String>> {
    if !matches!(kind, "ep" | "u" | "v" | "sf") {
        return None;
    }
    g.evaluate();
    let ids = if let Some(c) = crate::curves::as_curve(g, id) {
        (c, parse_curve(g, c, kind, inner)?)
    } else {
        let s = crate::surfaces::as_surface(g, id)?;
        (s, parse_surface(g, s, kind, inner)?)
    };
    Some(ids.1.into_iter().filter_map(|i| format_id(g, ids.0, i)).map(|t| format!("{obj}.{t}")).collect())
}

/// The weight each CV has at u: the curve with that CV alone at 1.
fn basis_at(c: &NurbsCurve, u: f64) -> Vec<f64> {
    let lap = c.distinct_cvs().len();
    (0..lap)
        .map(|j| {
            let mut unit = c.clone();
            unit.cvs.iter_mut().for_each(|p| *p = DVec3::ZERO);
            for k in 0..c.cvs.len() {
                if k % lap == j {
                    unit.cvs[k] = DVec3::X;
                }
            }
            unit.eval(u).x
        })
        .collect()
}

/// Shifts CVs so the curve passes through target at u, moving each as much as it bears there.
pub fn pull_curve(c: &mut NurbsCurve, u: f64, target: DVec3) {
    let basis = basis_at(c, u);
    let sum: f64 = basis.iter().map(|b| b * b).sum();
    if sum < 1e-12 {
        return;
    }
    let delta = (target - c.eval(u)) / sum;
    let lap = basis.len();
    for k in 0..c.cvs.len() {
        c.cvs[k] += delta * basis[k % lap];
    }
    if c.form == Form::Periodic {
        let spans = c.spans();
        for i in 0..c.degree {
            c.cvs[spans + i] = c.cvs[i % spans];
        }
    }
}

/// Moves of a curve's components as moves of its CVs. Edit points pull on the CVs around them; those asked for alike settle
/// over a few passes.
pub fn curve_moves(c: &NurbsCurve, moves: &[(u32, DVec3)]) -> Vec<(u32, DVec3)> {
    let eps: Vec<(f64, DVec3)> = moves.iter().filter(|(i, _)| *i & EP != 0 && *i < CURVE_U).filter_map(|(i, p)| Some((*c.distinct_params().get((*i & !EP) as usize)?, *p))).collect();
    let mut out: Vec<(u32, DVec3)> = moves.iter().filter(|(i, _)| !is_encoded(*i)).copied().collect();
    if eps.is_empty() {
        return out;
    }
    let mut work = c.clone();
    for (i, p) in &out {
        if let Some(cv) = work.cvs.get_mut(*i as usize) {
            *cv = *p;
        }
    }
    for _ in 0..4 {
        for &(u, p) in &eps {
            pull_curve(&mut work, u, p);
        }
    }
    let lap = work.distinct_cvs().len();
    out = (0..lap).map(|i| (i as u32, work.cvs[i])).collect();
    out
}

/// A surface's edit points as (id, position).
pub fn surface_edit_points(s: &NurbsSurface) -> Vec<(u32, DVec3)> {
    let (us, vs) = surface_params(s);
    let mut out = Vec::with_capacity(us.len() * vs.len());
    for (i, &u) in us.iter().enumerate() {
        for (j, &v) in vs.iter().enumerate() {
            out.push((EP | (i as u32) << 12 | j as u32, s.eval(u, v)));
        }
    }
    out
}

/// Moves of a surface's components as moves of its CVs.
pub fn surface_moves(s: &NurbsSurface, moves: &[(u32, DVec3)]) -> Vec<(u32, DVec3)> {
    let (us, vs) = surface_params(s);
    let mut out: Vec<(u32, DVec3)> = moves.iter().filter(|(i, _)| !is_encoded(*i)).copied().collect();
    let eps: Vec<(f64, f64, DVec3)> = moves
        .iter()
        .filter(|(i, _)| *i & EP != 0 && *i < CURVE_U)
        .filter_map(|(i, p)| {
            let (a, b) = (((*i & !EP) >> 12) as usize, (*i & 0xFFF) as usize);
            Some((*us.get(a)?, *vs.get(b)?, *p))
        })
        .collect();
    if eps.is_empty() {
        return out;
    }
    let mut work = s.clone();
    let stride = work.distinct_v().max(1) as u32;
    for (i, p) in &out {
        let (u, v) = ((*i / stride) as usize, (*i % stride) as usize);
        if u < work.nu && v < work.nv {
            work.cvs[u * work.nv + v] = *p;
        }
    }
    for _ in 0..4 {
        for &(u, v, p) in &eps {
            let delta_all = p - work.eval(u, v);
            // The weight of a CV: the surface with that CV alone at 1.
            let (du, dv) = (work.distinct_u(), work.distinct_v());
            let mut w = Vec::new();
            for iu in 0..du {
                for iv in 0..dv {
                    let mut unit = work.clone();
                    unit.cvs.iter_mut().for_each(|c| *c = DVec3::ZERO);
                    unit.cvs[iu * work.nv + iv] = DVec3::X;
                    unit.rewrap();
                    let b = unit.eval(u, v).x;
                    if b.abs() > 1e-9 {
                        w.push((iu, iv, b));
                    }
                }
            }
            let sum: f64 = w.iter().map(|x| x.2 * x.2).sum();
            if sum < 1e-12 {
                continue;
            }
            for (iu, iv, b) in w {
                work.cvs[iu * work.nv + iv] += delta_all / sum * b;
            }
            work.rewrap();
        }
    }
    let (du, dv) = (work.distinct_u(), work.distinct_v());
    out = (0..du).flat_map(|u| (0..dv).map(move |v| (u, v))).map(|(u, v)| ((u * dv + v) as u32, work.cv(u, v))).collect();
    out
}

/// pointPosition of a component here. None when it isn't one of these.
pub fn point_of(g: &mut Graph, text: &str, local: bool) -> Option<DVec3> {
    let (head, rest) = text.split_once('[')?;
    let (obj, kind) = head.rsplit_once('.')?;
    if !matches!(kind, "ep" | "u" | "uv") {
        return None;
    }
    let inner = rest.trim_end_matches(']');
    let id = g.find(obj).ok()??;
    if let Some(c) = crate::curves::as_curve(g, id) {
        let cu = if local { crate::curves::local_curve(g, c)? } else { crate::curves::world_curve(g, c)? };
        let u = if kind == "ep" { *cu.distinct_params().get(inner.parse::<usize>().ok()?)? } else { inner.parse().ok()? };
        return Some(cu.eval(u));
    }
    let sh = crate::surfaces::as_surface(g, id)?;
    let s = if local { crate::surfaces::local_surface(g, sh)? } else { crate::surfaces::world_surface(g, sh)? };
    let (a, b) = inner.split_once("][")?;
    if kind == "ep" {
        let (us, vs) = surface_params(&s);
        return Some(s.eval(*us.get(a.parse::<usize>().ok()?)?, *vs.get(b.parse::<usize>().ok()?)?));
    }
    Some(s.eval(a.parse().ok()?, b.parse().ok()?))
}

/// xform on these components: -q -t, and -t moves edit points. None when the string isn't one of these.
pub fn xform(g: &mut Graph, a: &Args, id: NodeId, comp: &str, ws: bool) -> Option<CmdResult<Vec<Value>>> {
    let (kind, inner) = comp.split_once('[')?;
    if !matches!(kind, "ep" | "u" | "v" | "sf") {
        return None;
    }
    let inner = inner.trim_end_matches(']');
    let curve = crate::curves::as_curve(g, id);
    let surface = crate::surfaces::as_surface(g, id);
    let shape = curve.or(surface)?;
    g.evaluate();
    let ids = if curve.is_some() { parse_curve(g, shape, kind, inner)? } else { parse_surface(g, shape, kind, inner)? };
    let w = crate::rig::world(g, crate::dag::transform_of(g, shape));
    let pp = g.node(shape).and_then(crate::components::point_provider)?;
    let here: Vec<DVec3> = ids
        .iter()
        .filter_map(|&i| {
            if let Some(c) = curve {
                let cu = crate::curves::local_curve(g, c)?;
                if i & CURVE_U != 0 {
                    return Some(cu.eval(param_of(i)));
                }
                return cu.edit_points().get((i & !EP) as usize).copied();
            }
            let s = crate::surfaces::local_surface(g, shape)?;
            surface_edit_points(&s).into_iter().find(|(k, _)| *k == i).map(|x| x.1)
        })
        .collect();
    if a.bool("query") {
        return Some(Ok(here.iter().flat_map(|p| {
            let q = if ws { w.transform_point3(*p) } else { *p };
            [json!(q.x), json!(q.y), json!(q.z)]
        }).collect()));
    }
    if kind != "ep" {
        return Some(Err(aztek_ops::CommandError::new(format!("xform: {kind} components can't be moved"))));
    }
    let Some(t) = a.vec3("translation").map(DVec3::from_array) else { return Some(Err(aztek_ops::CommandError::new("xform: on components only -t is available"))) };
    let inv = w.inverse();
    let rel = a.bool("relative");
    let moves: Vec<(u32, DVec3)> = ids
        .iter()
        .zip(&here)
        .map(|(&i, &p)| {
            (
                i,
                match (rel, ws) {
                    (true, true) => p + inv.transform_vector3(t),
                    (true, false) => p + t,
                    (false, true) => inv.transform_point3(t),
                    (false, false) => t,
                },
            )
        })
        .collect();
    if let Some(set) = pp.set_points {
        set(g, shape, &moves);
    }
    g.evaluate();
    Some(Ok(Vec::new()))
}
