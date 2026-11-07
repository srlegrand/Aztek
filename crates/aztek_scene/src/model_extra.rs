//! Modelling commands: Bezier curves as NURBS (nurbsCurveToBezier, bezierCurveToNurbs, bezierInfo, bezierAnchorPreset,
//! bezierAnchorState), explicit UVs on surfaces (nurbsUVSet, nurbsCopyUVSet, nurbsEditUV) and polyInstallAction.
//!
//! Differs from the reference. A Bezier curve stays a nurbsCurve shape with its knots tripled and an added attribute marking it,
//! rather than becoming a bezierCurve shape. Anchors have no lasting state: smooth and even move the tangents once, and a
//! later drag does not keep them. Explicit surface UVs hold one set per surface and nothing draws from them yet.

use std::sync::Arc;

use aztek_geo::nurbs::{Form, NurbsCurve};
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::components::{self, ComponentMode};
use crate::curves::{self, curve_attr, curve_value};
use crate::graph::Graph;
use crate::ref_core::f;
use crate::node::{EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugType, PlugValue};
use crate::rig::{find, plug};
use crate::session::Session;
use crate::skin::asked;
use crate::surfaces;
use crate::system_cmds::q_flags;

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Float as Fl, Int as I};

const BEZIER_MARK: &str = "aztekBezier";
const UV_ATTR: &str = "aztekExplicitUV";

fn truth(v: &Value) -> bool {
    v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0)
}

// ---------------------------------------------------------------------------------------------------------------------
// Bezier curves

/// The same curve as a cubic with every interior knot tripled: anchors at every third CV. Other degrees are refit, so their
/// shape changes a little.
pub(crate) fn to_bezier(c: &NurbsCurve) -> NurbsCurve {
    let mut x = if c.form == Form::Open { c.clone() } else { c.clamped() };
    if x.degree != 3 {
        x = x.rebuilt(x.spans().max(1), 3);
    }
    loop {
        let (lo, hi) = x.domain();
        let mut next = None;
        let mut i = 0;
        while i < x.knots.len() {
            let u = x.knots[i];
            let run = x.knots[i..].iter().take_while(|k| (**k - u).abs() < 1e-9).count();
            if u > lo + 1e-9 && u < hi - 1e-9 && run < 3 {
                next = Some(u);
                break;
            }
            i += run;
        }
        match next {
            Some(u) => x = x.insert_knot_any(u),
            None => return x,
        }
    }
}

pub(crate) fn is_bezier(c: &NurbsCurve) -> bool {
    c.degree == 3 && c.cvs.len() % 3 == 1 && to_bezier(c).cvs.len() == c.cvs.len()
}

struct CurveFn(fn(&NurbsCurve) -> NurbsCurve);

impl NodeBehavior for CurveFn {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let out = curve_value(d.value(0)).map(|c| curves::to_value((self.0)(c))).unwrap_or(PlugValue::Data(None));
        d.set_output(1, out);
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let pair = || vec![curve_attr("inputCurve", "ic", flags::INPUT | flags::STORABLE | flags::HIDDEN, "The curve."), curve_attr("outputCurve", "oc", flags::OUTPUT | flags::HIDDEN, "The result.")];
        surfaces::register_one(NodeType { name: "nurbsCurveToBezier".into(), icon: "chart-line".into(), doc: "A NURBS curve with its knots tripled, so every third CV is an anchor.".into(), own_attrs: pair(), create: Some(Arc::new(|| Box::new(CurveFn(to_bezier)))), ..Default::default() });
        surfaces::register_one(NodeType { name: "bezierCurveToNurbs".into(), icon: "chart-line".into(), doc: "A Bezier curve as a plain NURBS curve: the same knots and CVs.".into(), own_attrs: pair(), create: Some(Arc::new(|| Box::new(CurveFn(NurbsCurve::clone)))), ..Default::default() });
    });
}

fn mark_bezier(g: &mut Graph, shape: NodeId, on: bool) -> CmdResult<()> {
    let at = match g.node(shape).and_then(|n| n.ty().attr_index(BEZIER_MARK)) {
        Some(i) => i,
        None if !on => return Ok(()),
        None => g.add_attr(shape, AttrDef { name: BEZIER_MARK.into(), short_name: "azbz".into(), ty: PlugType::Bool, default_value: PlugValue::Bool(false), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: "Set by nurbsCurveToBezier.".into(), ..Default::default() })?,
    };
    g.set_value(plug(shape, at), PlugValue::Bool(on))?;
    Ok(())
}

fn convert(s: &mut Session, a: &Args, cmd: &str, on: bool) -> CmdResult {
    let names = objects(s, a);
    if names.is_empty() {
        fail!("{cmd}: name or select a curve");
    }
    for n in &names {
        let g = &mut s.graph;
        let shape = curves::curve_arg(g, n, cmd)?;
        if !on {
            let curve = curves::local_curve(g, shape).unwrap_or_default();
            if !is_bezier(&curve) {
                fail!("{cmd}: {n} is not a Bezier curve");
            }
        }
    }
    let out = curves::run_op(s, a, cmd, cmd, &[])?;
    for n in &names {
        let g = &mut s.graph;
        let shape = curves::curve_arg(g, n, cmd)?;
        mark_bezier(g, shape, on)?;
    }
    Ok(out)
}

/// Curves with CVs selected: (shape, CV ids).
fn selected_cvs(g: &Graph) -> Vec<(NodeId, Vec<u32>)> {
    components::selected_all(g, ComponentMode::Vertex)
        .iter()
        .filter_map(|(&n, ids)| {
            let shape = curves::as_curve(g, n)?;
            // select -clear leaves the component lists alone, so only those of selected objects count.
            let chosen = g.is_selected(n) || g.is_selected(shape) || g.is_selected(crate::dag::transform_of(g, shape));
            (chosen && !ids.is_empty()).then(|| (shape, ids.clone()))
        })
        .collect()
}

fn bezier_info(s: &mut Session, a: &Args) -> CmdResult {
    if let Some(v) = a.get("anchorFromCV").and_then(Value::as_i64) {
        return Ok(json!((v + 1) / 3));
    }
    if let Some(v) = a.get("cvFromAnchor").and_then(Value::as_i64) {
        return Ok(json!(v * 3));
    }
    let all: Vec<u32> = selected_cvs(&s.graph).into_iter().flat_map(|x| x.1).collect();
    let (anchors, tangents) = (all.iter().filter(|c| *c % 3 == 0).count(), all.iter().filter(|c| *c % 3 != 0).count());
    let yes = |b: bool| json!(i64::from(b));
    Ok(if a.bool("isAnchorSelected") {
        yes(anchors > 0)
    } else if a.bool("isTangentSelected") {
        yes(tangents > 0)
    } else if a.bool("onlyAnchorsSelected") {
        yes(anchors > 0 && tangents == 0)
    } else if a.bool("onlyTangentsSelected") {
        yes(tangents > 0 && anchors == 0)
    } else {
        fail!("bezierInfo: use one of -afc, -cfa, -ias, -its, -oas, -ots")
    })
}

/// The anchors behind selected CVs, on Bezier curves, and runs `edit` on each. Returns how many.
fn on_anchors(s: &mut Session, cmd: &str, edit: impl Fn(&mut Vec<DVec3>, usize)) -> CmdResult {
    let picked = selected_cvs(&s.graph);
    if picked.is_empty() {
        fail!("{cmd}: select Bezier anchors or tangents");
    }
    let mut count = 0;
    for (shape, ids) in picked {
        let g = &mut s.graph;
        let curve = curves::local_curve(g, shape).unwrap_or_default();
        if !is_bezier(&curve) {
            fail!("{cmd}: {} is not a Bezier curve", g.path(shape));
        }
        let mut anchors: Vec<usize> = ids.iter().map(|&c| (c as usize + 1) / 3).collect();
        anchors.sort_unstable();
        anchors.dedup();
        let mut cvs = curve.cvs.clone();
        for &an in &anchors {
            edit(&mut cvs, an);
            count += 1;
        }
        let path = g.path(shape);
        let points: Vec<[f64; 3]> = cvs.iter().map(|p| p.to_array()).collect();
        s.run("setCurvePoints", json!({"args": [path], "points": points}))?;
    }
    Ok(json!(count))
}

/// The tangent CVs either side of an anchor, where they exist.
fn sides(cvs: &[DVec3], an: usize) -> (usize, Option<usize>, Option<usize>) {
    let at = an * 3;
    (at, at.checked_sub(1), (at + 1 < cvs.len()).then_some(at + 1))
}

fn make_smooth(cvs: &mut [DVec3], an: usize) {
    let (at, before, after) = sides(cvs, an);
    if let (Some(i), Some(o)) = (before, after) {
        let dir = (cvs[o] - cvs[i]).normalize_or_zero();
        let (li, lo) = (cvs[i].distance(cvs[at]), cvs[o].distance(cvs[at]));
        cvs[i] = cvs[at] - dir * li;
        cvs[o] = cvs[at] + dir * lo;
    }
}

fn make_even(cvs: &mut [DVec3], an: usize) {
    let (at, before, after) = sides(cvs, an);
    if let (Some(i), Some(o)) = (before, after) {
        let len = (cvs[i].distance(cvs[at]) + cvs[o].distance(cvs[at])) * 0.5;
        cvs[i] = cvs[at] + (cvs[i] - cvs[at]).normalize_or_zero() * len;
        cvs[o] = cvs[at] + (cvs[o] - cvs[at]).normalize_or_zero() * len;
    }
}

fn apply_preset(cvs: &mut [DVec3], an: usize, preset: i64) {
    let (at, before, after) = sides(cvs, an);
    let anchor = cvs[at];
    let prev = before.map(|_| cvs[at - 3]);
    let next = after.map(|_| cvs[at + 3]);
    match preset {
        // Corner: the tangents drawn in to the anchor.
        2 => {
            if let Some(i) = before {
                cvs[i] = anchor;
            }
            if let Some(o) = after {
                cvs[o] = anchor;
            }
        }
        // Bezier Corner: each tangent a third of the way to its neighbour.
        1 => {
            if let (Some(i), Some(p)) = (before, prev) {
                cvs[i] = anchor + (p - anchor) / 3.0;
            }
            if let (Some(o), Some(n)) = (after, next) {
                cvs[o] = anchor + (n - anchor) / 3.0;
            }
        }
        // Bezier: smooth and even, along the line between the neighbours.
        _ => match (before, after, prev, next) {
            (Some(i), Some(o), Some(p), Some(n)) => {
                let dir = (n - p).normalize_or_zero();
                let len = (anchor.distance(p) + anchor.distance(n)) / 6.0;
                cvs[i] = anchor - dir * len;
                cvs[o] = anchor + dir * len;
            }
            (Some(i), None, Some(p), None) => cvs[i] = anchor + (p - anchor) / 3.0,
            (None, Some(o), None, Some(n)) => cvs[o] = anchor + (n - anchor) / 3.0,
            _ => {}
        },
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Explicit UVs on surfaces

fn surface_named(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = find(g, name.split('.').next().unwrap_or(name), cmd)?;
    match surfaces::as_surface(g, id) {
        Some(sh) => Ok(sh),
        None => fail!("{cmd}: {name} is not a NURBS surface"),
    }
}

fn uv_state(g: &Graph, shape: NodeId) -> Value {
    let Some(n) = g.node(shape) else { return Value::Null };
    let Some(i) = n.ty().attr_index(UV_ATTR) else { return Value::Null };
    match n.data().value(i) {
        PlugValue::String(t) => serde_json::from_str(t).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

fn set_uv_state(g: &mut Graph, shape: NodeId, v: &Value) -> CmdResult<()> {
    let at = match g.node(shape).and_then(|n| n.ty().attr_index(UV_ATTR)) {
        Some(i) => i,
        None => g.add_attr(shape, AttrDef { name: UV_ATTR.into(), short_name: "azuv".into(), ty: PlugType::String, default_value: PlugValue::String(String::new()), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: "Explicit UVs per CV, as JSON.".into(), ..Default::default() })?,
    };
    g.set_value(plug(shape, at), PlugValue::String(v.to_string()))?;
    Ok(())
}

/// The implicit parameterization at each CV, evenly over 0 to 1: u, v per CV id.
fn implicit_uvs(g: &mut Graph, shape: NodeId) -> CmdResult<Vec<f64>> {
    let Some(srf) = surfaces::local_surface(g, shape) else { fail!("nurbsUVSet: the surface has no data") };
    let (nu, nv) = (srf.distinct_u(), srf.distinct_v());
    let mut out = Vec::with_capacity(nu * nv * 2);
    for i in 0..nu {
        for j in 0..nv {
            out.push(i as f64 / (nu.max(2) - 1) as f64);
            out.push(j as f64 / (nv.max(2) - 1) as f64);
        }
    }
    Ok(out)
}

fn nurbs_uv_set(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(first) = names.first() else { fail!("nurbsUVSet: name or select a surface") };
    let g = &mut s.graph;
    let shape = surface_named(g, first, "nurbsUVSet")?;
    let state = uv_state(g, shape);
    if a.bool("query") {
        if asked(a, "useExplicit") {
            return Ok(json!(state["use"].as_bool().unwrap_or(false)));
        }
        if asked(a, "create") {
            return Ok(json!(state["uvs"].is_array()));
        }
        fail!("nurbsUVSet: ask for -create or -useExplicit");
    }
    let mut next = if state.is_object() { state } else { json!({"use": false}) };
    if a.bool("create") && !next["uvs"].is_array() {
        next["uvs"] = json!(implicit_uvs(g, shape)?);
    }
    if let Some(v) = a.get("useExplicit") {
        if truth(v) && !next["uvs"].is_array() {
            fail!("nurbsUVSet: the surface has no explicit UV set; use -create first");
        }
        next["use"] = json!(truth(v));
    }
    set_uv_state(g, shape, &next)?;
    Ok(json!(true))
}

fn nurbs_copy_uv_set(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let Some(first) = names.first() else { fail!("nurbsCopyUVSet: name or select a surface") };
    let g = &mut s.graph;
    let shape = surface_named(g, first, "nurbsCopyUVSet")?;
    let state = uv_state(g, shape);
    let mut next = if state.is_object() { state } else { json!({"use": false}) };
    next["uvs"] = json!(implicit_uvs(g, shape)?);
    set_uv_state(g, shape, &next)?;
    Ok(json!(true))
}

/// CV ids an edit acts on: surf.cv[...] as named, or those selected.
fn uv_targets(s: &Session, a: &Args) -> CmdResult<Vec<(NodeId, Vec<u32>)>> {
    let g = &s.graph;
    let mut out = Vec::new();
    for name in a.named() {
        let shape = surface_named(g, &name, "nurbsEditUV")?;
        let ids = match name.split_once(".cv[") {
            Some((_, rest)) => surfaces::parse_cv_ranges(g, shape, rest.trim_end_matches(']')).unwrap_or_default(),
            None => Vec::new(),
        };
        out.push((shape, ids));
    }
    if out.is_empty() {
        for (&n, ids) in components::selected_all(g, ComponentMode::Vertex) {
            if let Some(shape) = surfaces::as_surface(g, n) {
                if g.is_selected(n) || g.is_selected(shape) || g.is_selected(crate::dag::transform_of(g, shape)) {
                    out.push((shape, ids.clone()));
                }
            }
        }
    }
    Ok(out)
}

fn nurbs_edit_uv(s: &mut Session, a: &Args) -> CmdResult {
    let targets = uv_targets(s, a)?;
    if targets.is_empty() {
        fail!("nurbsEditUV: select surface CVs");
    }
    let query = a.bool("query");
    let mut answer = Vec::new();
    for (shape, ids) in targets {
        let g = &mut s.graph;
        let mut state = uv_state(g, shape);
        let Some(stored) = state["uvs"].as_array() else { fail!("nurbsEditUV: {} has no explicit UV set; use nurbsUVSet -create", g.path(shape)) };
        let mut uvs: Vec<f64> = stored.iter().filter_map(Value::as_f64).collect();
        let picked: Vec<usize> = if ids.is_empty() { (0..uvs.len() / 2).collect() } else { ids.iter().map(|&i| i as usize).filter(|&i| i * 2 + 1 < uvs.len()).collect() };
        if query {
            let (want_u, want_v) = (asked(a, "uValue"), asked(a, "vValue"));
            for i in picked {
                if want_u || !want_v {
                    answer.push(json!(uvs[i * 2]));
                }
                if want_v || !want_u {
                    answer.push(json!(uvs[i * 2 + 1]));
                }
            }
            continue;
        }
        let relative = a.get("relative").map_or(true, truth);
        let (pu, pv) = (a.float("pivotU", 0.0), a.float("pivotV", 0.0));
        let angle = (a.float("angle", 0.0) * a.float("rotateRatio", 1.0)).to_radians();
        let (su, sv) = (a.float("scaleU", 1.0), a.float("scaleV", 1.0));
        let (du, dv) = (a.get("uValue").and_then(Value::as_f64), a.get("vValue").and_then(Value::as_f64));
        for i in picked {
            let (mut u, mut v) = (uvs[i * 2], uvs[i * 2 + 1]);
            if a.has("scaleU") || a.has("scaleV") {
                u = pu + (u - pu) * su;
                v = pv + (v - pv) * sv;
            }
            if a.has("angle") {
                let (x, y) = (u - pu, v - pv);
                u = pu + x * angle.cos() - y * angle.sin();
                v = pv + x * angle.sin() + y * angle.cos();
            }
            if let Some(d) = du {
                u = if relative { u + d } else { d };
            }
            if let Some(d) = dv {
                v = if relative { v + d } else { d };
            }
            uvs[i * 2] = u;
            uvs[i * 2 + 1] = v;
        }
        state["uvs"] = json!(uvs);
        set_uv_state(g, shape, &state)?;
    }
    Ok(if query { Value::Array(answer) } else { json!(true) })
}

// ---------------------------------------------------------------------------------------------------------------------
// polyInstallAction

/// What an action works on, and whether only borders will do.
fn action_kind(name: &str) -> Option<(ComponentMode, bool)> {
    Some(match name {
        "polyCloseBorder" => (ComponentMode::Edge, true),
        "polyBevel" | "polyExtrudeEdge" | "polySplitEdge" | "polyDelEdge" | "polySewEdge" | "polyCrease" | "polyCollapseEdge" | "polyFlipEdge" | "polySpinEdge" => (ComponentMode::Edge, false),
        "polyExtrudeFacet" | "polySubdivideFacet" | "polyChipOff" | "polyPoke" | "polyQuad" | "polyTriangulate" | "polyDelFacet" | "polyCollapseFacet" | "polyDuplicateFacet" | "polyExtractFacet" | "polyAverageNormal" | "polyMapCut" | "polyProjection" => (ComponentMode::Face, false),
        "polyMergeVertex" | "polyDelVertex" | "polyChipVertex" | "polyExtrudeVertex" | "polyMoveVertex" => (ComponentMode::Vertex, false),
        _ => return None,
    })
}

fn poly_install_action(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let more = &s.extras.more;
        if asked(a, "commandName") {
            return Ok(json!(more.install_action));
        }
        if asked(a, "installConstraint") {
            return Ok(json!(i64::from(more.install_constraint)));
        }
        if asked(a, "installDisplay") {
            return Ok(json!(i64::from(more.install_display)));
        }
        fail!("polyInstallAction: ask for -cn, -ic or -id");
    }
    if a.bool("commandName") {
        return Ok(json!(s.extras.more.install_action));
    }
    let name = a.pos_str(0).map(str::to_string);
    let (mut ic, mut id) = (a.bool("installConstraint"), a.bool("installDisplay"));
    let (mut uc, mut ud) = (a.bool("uninstallConstraint"), a.bool("uninstallDisplay"));
    let converting = a.bool("convertSelection");
    match &name {
        // Shortcuts: an action alone installs both; nothing at all takes both off.
        Some(_) if !ic && !id && !converting => {
            ic = true;
            id = true;
        }
        None if !uc && !ud => {
            uc = true;
            ud = true;
        }
        _ => {}
    }
    if uc {
        s.run("polySelectConstraint", json!({"disable": true}))?;
        s.extras.more.install_constraint = false;
    }
    if ud && s.extras.more.install_display {
        s.run("polyOptions", json!({"displayBorder": false}))?;
        s.extras.more.install_display = false;
    }
    let Some(name) = name else { return Ok(json!([])) };
    let Some((mode, border)) = action_kind(&name) else { fail!("polyInstallAction: no action {name}") };
    if converting {
        let convert_flag = match mode {
            ComponentMode::Vertex => "toVertex",
            ComponentMode::Edge => "toEdge",
            _ => "toFace",
        };
        let selected = s.run("ls", json!({"selection": true}))?;
        if selected.as_array().is_some_and(|l| !l.is_empty()) {
            let mut call = json!({"args": selected});
            call[convert_flag] = json!(true);
            let list = s.run("polyListComponentConversion", call)?;
            s.run("select", json!({"args": list}))?;
        }
    }
    s.extras.more.install_action = name;
    if id && border {
        s.run("polyOptions", json!({"displayBorder": true}))?;
        s.extras.more.install_display = true;
    }
    if !ic {
        return Ok(json!([]));
    }
    components::set_mode_undoable(&mut s.graph, mode);
    if border {
        s.run("polySelectConstraint", json!({"mode": 3, "type": 0x8000, "where": 1}))?;
    }
    s.extras.more.install_constraint = true;
    s.run("ls", json!({"selection": true, "flatten": true}))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        curves::with_common(Cmd::new("nurbsCurveToBezier", "Make a curve Bezier: a cubic with every third CV an anchor, keeping its shape. Other degrees are refit and change a little. Returns [object, node]. Less than the reference: the shape stays a nurbsCurve with an added attribute marking it.", |s, a| convert(s, a, "nurbsCurveToBezier", true)))
            .positional("curves (the selection when none)"),
    );
    c.register(
        curves::with_common(Cmd::new("bezierCurveToNurbs", "Make a Bezier curve a plain NURBS curve, keeping its CVs and knots. Returns [object, node].", |s, a| convert(s, a, "bezierCurveToNurbs", false)))
            .positional("curves (the selection when none)"),
    );
    c.register(
        Cmd::new("bezierInfo", "Questions about Bezier CVs: -afc a CV's anchor, -cfa an anchor's CV; -ias, -its, -oas, -ots about what is selected, as 1 or 0.", bezier_info)
            .flag(f("anchorFromCV", "afc", I, "A CV index."))
            .flag(f("cvFromAnchor", "cfa", I, "An anchor index."))
            .flag(f("isAnchorSelected", "ias", B, "Is an anchor selected."))
            .flag(f("isTangentSelected", "its", B, "Is a tangent selected."))
            .flag(f("onlyAnchorsSelected", "oas", B, "Are only anchors selected."))
            .flag(f("onlyTangentsSelected", "ots", B, "Are only tangents selected."))
            .undoable(false),
    );
    c.register(
        Cmd::new("bezierAnchorPreset", "Set selected anchors' tangents: -p 0 Bezier (smooth and even, along the line between the neighbours), 1 Bezier Corner (each a third of the way to its neighbour), 2 Corner (drawn in to the anchor). Returns how many anchors. The exact tangent lengths are this command's own: the reference does not document them.", |s, a| {
            let p = a.int("preset", 0);
            if !(0..=2).contains(&p) {
                fail!("bezierAnchorPreset: -p is 0, 1 or 2");
            }
            on_anchors(s, "bezierAnchorPreset", move |cvs, an| apply_preset(cvs, an, p))
        })
        .flag(f("preset", "p", I, "0 Bezier, 1 Bezier Corner, 2 Corner.")),
    );
    c.register(
        Cmd::new("bezierAnchorState", "Set selected anchors or their tangents: -sm true lines the tangents up, -ev true makes them the same length. Returns how many anchors. Less than the reference: there is no lasting state, so false changes nothing and later drags do not keep the tangents smooth or even.", |s, a| {
            let (smooth, even) = (a.get("smooth").is_some_and(truth), a.get("even").is_some_and(truth));
            on_anchors(s, "bezierAnchorState", move |cvs, an| {
                if smooth {
                    make_smooth(cvs, an);
                }
                if even {
                    make_even(cvs, an);
                }
            })
        })
        .flag(f("even", "ev", B, "Even weighting."))
        .flag(f("smooth", "sm", B, "Smooth rather than broken.")),
    );
    c.register(
        q_flags(Cmd::new("nurbsUVSet", "Explicit UVs on a surface: -c makes the set from the implicit ones, -ue switches to it. Returns true. Less than the reference: one set per surface, the implicit values are evenly spaced by CV rather than taken from the knots, and nothing draws from them yet.", nurbs_uv_set))
            .positional("a surface (the selection when none)")
            .flag(f("create", "c", B, "Make the explicit set."))
            .flag(f("useExplicit", "ue", B, "Use the explicit set.")),
    );
    c.register(
        Cmd::new("nurbsCopyUVSet", "Copy the implicit parameterization to the surface's explicit set, as the reference's debugging command does. Returns true. The values are evenly spaced by CV.", nurbs_copy_uv_set)
            .positional("a surface (the selection when none)"),
    );
    c.register(
        q_flags(Cmd::new("nurbsEditUV", "Edit the explicit UVs of surface CVs: -u, -v moves (absolute with -r false), -a degrees with -rr, -su, -sv about -pu, -pv (0 by default). -q gives u, v per CV; -q -u or -q -v one of them. Needs nurbsUVSet -c first.", nurbs_edit_uv))
            .positional("surface.cv[u][v] (the selection when none)")
            .flag(f("angle", "a", Fl, "Degrees to rotate."))
            .flag(f("pivotU", "pu", Fl, "The pivot in U."))
            .flag(f("pivotV", "pv", Fl, "The pivot in V."))
            .flag(f("relative", "r", B, "Add to the values (on by default)."))
            .flag(f("rotateRatio", "rr", Fl, "Multiplies the angle (1)."))
            .flag(f("rotation", "rot", B, "Editing rotation: -a says how much."))
            .flag(f("scale", "s", B, "Editing scale: -su and -sv say how much."))
            .flag(f("scaleU", "su", Fl, "Scale in U."))
            .flag(f("scaleV", "sv", Fl, "Scale in V."))
            .flag(f("uValue", "u", ArgType::Any, "U."))
            .flag(f("vValue", "v", ArgType::Any, "V.")),
    );
    c.register(
        q_flags(Cmd::new("polyInstallAction", "Prepare for a poly action: -ic the component mode and for polyCloseBorder a border constraint, -id show borders, -cs convert the selection, -uc and -ud take them off. An action alone does -ic -id; no arguments does -uc -ud. Returns the selection on -ic. Less than the reference: only the actions it knows have a mode, and only polyCloseBorder a constraint and display.", poly_install_action))
            .positional("the action")
            .flag(f("commandName", "cn", B, "The action installed."))
            .flag(f("convertSelection", "cs", B, "Convert selected objects to the action's components."))
            .flag(f("installConstraint", "ic", B, "Install the mode and constraint."))
            .flag(f("installDisplay", "id", B, "Install the display."))
            .flag(f("keepInstances", "ki", B, "Accepted for existing scripts; has no effect."))
            .flag(f("uninstallConstraint", "uc", B, "Take the constraint off."))
            .flag(f("uninstallDisplay", "ud", B, "Take the display off."))
            .undoable(false),
    );
}
