//! Curve edits that join, cut or make curves, as live history nodes the way the reference makes them: attachCurve,
//! detachCurve and polyEdgeToCurve (Mesh > Create Curve From Edges). Their attributes are the reference's.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::nurbs::{Form, NurbsCurve};
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{matrix_attr, mesh_attr};
use crate::components::{self, ComponentMode};
use crate::curves::{bake_history, create_curve, curve_arg, curve_attr, curve_value, f_in, flat_numbers, int_in, local_curve, store_bool, to_value, CurveShape};
use crate::graph::{Graph, SelectMode};
use crate::mesh_nodes::MeshNode;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::plug;
use crate::session::Session;

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

// attachCurve: inputCurve1 and outputCurve first, so the node goes into a curve's history as any curve edit does.
const A_IN1: usize = 0;
const A_OUT: usize = 1;
const A_IN2: usize = 2;
const A_METHOD: usize = 3;
const A_BIAS: usize = 4;
const A_KEEP_KNOTS: usize = 5;
const A_REV1: usize = 6;
const A_REV2: usize = 7;
const A_PARAM: usize = 8;
const A_BKI: usize = 9;
const A_INPUTS: usize = 10;
/// Brings the second curve into the first's space when the first is changed in place.
const A_SPACE: usize = 11;

// detachCurve.
const D_IN: usize = 0;
const D_OUT: usize = 1;
const D_PARAM: usize = 2;
const D_KEEP: usize = 3;

// polyEdgeToCurve.
const E_MESH: usize = 0;
const E_OUT: usize = 1;
const E_COMPONENTS: usize = 2;
const E_MAT: usize = 3;
const E_DEGREE: usize = 4;
const E_FORM: usize = 5;

struct AttachNode;

impl NodeBehavior for AttachNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let mut parts: Vec<NurbsCurve> = Vec::new();
        if let Some(c) = curve_value(d.value(A_IN1)).cloned() {
            parts.push(if d.bool(A_REV1) { c.reversed() } else { c });
        }
        let space = d.matrix(A_SPACE);
        if let Some(c) = curve_value(d.value(A_IN2)).cloned() {
            let c = c.transformed(&space);
            parts.push(if d.bool(A_REV2) { c.reversed() } else { c });
        }
        for (_, v) in d.elements(A_INPUTS) {
            if let Some(c) = curve_value(v) {
                parts.push(c.transformed(&space));
            }
        }
        let mut it = parts.into_iter();
        let out = it.next().map(|first| it.fold(first, |acc, c| acc.attached(&c)));
        d.set_output(A_OUT, out.map(to_value).unwrap_or(PlugValue::Data(None)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct DetachNode;

impl NodeBehavior for DetachNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(c) = curve_value(d.value(D_IN)).cloned() else {
            d.set_output_elements(D_OUT, Vec::new());
            return;
        };
        let mut params: Vec<f64> = d.elements(D_PARAM).iter().map(|(_, v)| v.as_float()).collect();
        params.sort_by(f64::total_cmp);
        let mut rest = if c.form == Form::Periodic { c.opened() } else { c };
        let mut pieces = Vec::new();
        for u in params {
            let (a, b) = rest.split(u);
            pieces.push(a);
            rest = b;
        }
        pieces.push(rest);
        d.set_output_elements(D_OUT, pieces.into_iter().enumerate().map(|(i, p)| (i as i32, to_value(p))).collect());
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The edges as one chain of points, end to end, and whether it closes.
fn edge_chain(m: &aztek_geo::Mesh, ids: &[u32]) -> Option<(Vec<u32>, bool)> {
    let mut left: Vec<u32> = ids.iter().copied().filter(|&e| m.edge_valid(e)).collect();
    if left.is_empty() {
        return None;
    }
    let first = left.remove(0);
    let mut chain = vec![m.edge_point(first, 0), m.edge_point(first, 1)];
    loop {
        let tail = *chain.last().unwrap();
        let head = chain[0];
        if let Some(i) = left.iter().position(|&e| m.edge_point(e, 0) == tail || m.edge_point(e, 1) == tail) {
            let e = left.remove(i);
            chain.push(m.edge_other_point(e, tail));
        } else if let Some(i) = left.iter().position(|&e| m.edge_point(e, 0) == head || m.edge_point(e, 1) == head) {
            let e = left.remove(i);
            chain.insert(0, m.edge_other_point(e, head));
        } else {
            break;
        }
    }
    let closed = chain.len() > 2 && chain.first() == chain.last();
    if closed {
        chain.pop();
    }
    Some((chain, closed))
}

struct EdgeToCurveNode;

impl NodeBehavior for EdgeToCurveNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let mesh = d.value(E_MESH).as_mesh().cloned();
        let ids = crate::history::parse_components(d.string(E_COMPONENTS));
        let out = mesh.as_ref().and_then(|m| {
            let (chain, closed) = edge_chain(m, &ids)?;
            let w = d.matrix(E_MAT);
            let pts: Vec<DVec3> = chain.iter().map(|&p| w.transform_point3(m.position(p))).collect();
            let degree = (d.int(E_DEGREE).max(1) as usize).min(pts.len().saturating_sub(1).max(1));
            // form: 0 open, 1 periodic, 2 best guess (closed when the edges close).
            let periodic = match d.int(E_FORM) {
                0 => false,
                1 => true,
                _ => closed,
            };
            Some(if periodic && pts.len() > degree { NurbsCurve::periodic(degree, pts) } else { NurbsCurve::through(degree, &pts) })
        });
        d.set_output(E_OUT, out.map(to_value).unwrap_or(PlugValue::Data(None)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn array(name: &str, short: &str, ty: PlugType, def: PlugValue, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: def, flags: flags::INPUT | flags::STORABLE | flags::ARRAY, doc: doc.into(), ..Default::default() }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register_types_once);
}

fn register_types_once() {
    let hidden_in = flags::INPUT | flags::STORABLE | flags::HIDDEN;
    for t in [
        NodeType {
            name: "attachCurve".into(),
            icon: "chart-line".into(),
            doc: "Joins curves end to end.".into(),
            own_attrs: vec![
                curve_attr("inputCurve1", "ic1", hidden_in, "The first curve."),
                curve_attr("outputCurve", "oc", flags::OUTPUT | flags::HIDDEN, "The joined curve."),
                curve_attr("inputCurve2", "ic2", hidden_in, "The second curve."),
                int_in("method", "m", 0, "0 connect, 1 blend."),
                f_in("blendBias", "bb", 0.5, "Which curve a blend leans to."),
                store_bool("keepMultipleKnots", true, "Keep the knots where the curves meet."),
                store_bool("reverse1", false, "Reverse the first curve first."),
                store_bool("reverse2", false, "Reverse the second curve first."),
                f_in("parameter", "p", 0.1, "Where a blend's knot goes."),
                store_bool("blendKnotInsertion", false, "Insert a knot for a blend."),
                curve_attr("inputCurves", "ics", flags::INPUT | flags::STORABLE | flags::ARRAY | flags::HIDDEN, "More curves, joined in order."),
                matrix_attr("space", flags::INPUT | flags::STORABLE | flags::HIDDEN, "Brings the other curves into the first one's space."),
            ],
            create: Some(Arc::new(|| Box::new(AttachNode))),
            ..Default::default()
        },
        NodeType {
            name: "detachCurve".into(),
            icon: "chart-line".into(),
            doc: "Cuts a curve at its parameters: one curve out per piece.".into(),
            own_attrs: vec![
                curve_attr("inputCurve", "ic", hidden_in, "The curve cut."),
                curve_attr("outputCurve", "oc", flags::OUTPUT | flags::ARRAY | flags::HIDDEN, "The pieces, in order."),
                array("parameter", "p", PlugType::Float, PlugValue::Float(0.0), "Where to cut."),
                array("keep", "k", PlugType::Bool, PlugValue::Bool(true), "Which pieces are kept."),
            ],
            create: Some(Arc::new(|| Box::new(DetachNode))),
            ..Default::default()
        },
        NodeType {
            name: "polyEdgeToCurve".into(),
            icon: "chart-line".into(),
            doc: "A curve through a chain of a mesh's edges.".into(),
            own_attrs: vec![
                mesh_attr("inputPolymesh", "ipm", flags::INPUT | flags::HIDDEN, "The mesh."),
                curve_attr("outputcurve", "oc", flags::OUTPUT | flags::HIDDEN, "The curve, in world space."),
                AttrDef {
                    name: "inputComponents".into(),
                    short_name: "ics".into(),
                    ty: PlugType::String,
                    default_value: PlugValue::String(String::new()),
                    flags: flags::INPUT | flags::STORABLE,
                    doc: "The edges, as e[0:3].".into(),
                    ..Default::default()
                },
                matrix_attr("inputMat", flags::INPUT | flags::HIDDEN, "The mesh's world matrix."),
                int_in("degree", "dg", 3, "1 linear, 3 cubic."),
                int_in("form", "f", 2, "0 open, 1 periodic, 2 best guess."),
                store_bool("conformToSmoothMeshPreview", true, "Follow the smooth mesh preview."),
                int_in("displaySmoothMesh", "dsm", 0, "The smooth preview it follows."),
                int_in("smoothLevel", "lev", 2, "The smooth level it follows."),
                mesh_attr("inputSmoothPolymesh", "ism", flags::INPUT | flags::HIDDEN, "The smoothed mesh."),
            ],
            create: Some(Arc::new(|| Box::new(EdgeToCurveNode))),
            ..Default::default()
        },
    ] {
        let name = t.name.clone();
        types::register_type(t).expect(&name);
    }
}

/// A new curve object whose shape a history node's output feeds.
fn curve_fed_by(g: &mut Graph, name: &str, from: PlugRef) -> CmdResult<(NodeId, NodeId)> {
    let (tr, sh) = create_curve(g, name, NurbsCurve::default(), false)?;
    g.connect(&from, &plug(sh, CurveShape::CREATE), true)?;
    Ok((tr, sh))
}

/// The upstream of a curve's create plug, or its own curve as a value: what a node put into its history reads.
fn feed(g: &mut Graph, shape: NodeId, into: PlugRef) -> CmdResult<()> {
    let current = local_curve(g, shape).unwrap_or_default();
    let create = plug(shape, CurveShape::CREATE);
    match g.source(&create) {
        Some(src) if g.node(shape).is_some_and(|n| matches!(n.data().value(CurveShape::TWEAK), PlugValue::Data(None))) => g.connect(&src, &into, false).map(|_| ()).map_err(Into::into),
        _ => {
            g.set_value(into, to_value(current))?;
            let _ = g.set_value(plug(shape, CurveShape::TWEAK), PlugValue::Data(None));
            Ok(())
        }
    }
}

fn history_on(g: &Graph, a: &Args) -> bool {
    a.get("constructionHistory").and_then(Value::as_bool).unwrap_or(g.construction_history)
}

fn attach(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    if names.len() < 2 {
        fail!("attachCurve: name or select two curves");
    }
    let shapes: Vec<NodeId> = names.iter().map(|n| curve_arg(g, n, "attachCurve")).collect::<CmdResult<_>>()?;
    let rpo = a.get("replaceOriginal").and_then(Value::as_bool).unwrap_or(true);
    let t = types::find("attachCurve").expect("attachCurve");
    let node = g.create_node(&t, "attachCurve1", NO_NODE, true)?;
    g.set_value(plug(node, A_METHOD), PlugValue::Int(a.int("method", 0)))?;
    g.set_value(plug(node, A_BIAS), PlugValue::Float(a.float("blendBias", 0.5)))?;
    g.set_value(plug(node, A_KEEP_KNOTS), PlugValue::Bool(a.get("keepMultipleKnots").and_then(Value::as_bool).unwrap_or(true)))?;
    g.set_value(plug(node, A_REV1), PlugValue::Bool(a.bool("reverse1")))?;
    g.set_value(plug(node, A_REV2), PlugValue::Bool(a.bool("reverse2")))?;
    g.set_value(plug(node, A_PARAM), PlugValue::Float(a.float("parameter", 0.1)))?;
    g.set_value(plug(node, A_BKI), PlugValue::Bool(a.bool("blendKnotInsertion")))?;
    let first = shapes[0];
    for (k, &sh) in shapes.iter().enumerate().skip(1) {
        let to = if k == 1 { plug(node, A_IN2) } else { PlugRef::element(node, A_INPUTS as i32, (k - 2) as i32) };
        g.connect(&plug(sh, CurveShape::WORLD_SPACE), &to, false)?;
    }
    let target = if rpo {
        feed(g, first, plug(node, A_IN1))?;
        let tr = crate::dag::transform_of(g, first);
        let inv = crate::rig::world(g, tr).inverse();
        g.set_value(plug(node, A_SPACE), PlugValue::Matrix(inv))?;
        g.connect(&plug(node, A_OUT), &plug(first, CurveShape::CREATE), true)?;
        first
    } else {
        g.connect(&plug(first, CurveShape::WORLD_SPACE), &plug(node, A_IN1), false)?;
        g.set_value(plug(node, A_SPACE), PlugValue::Matrix(DMat4::IDENTITY))?;
        curve_fed_by(g, &a.string("name", "attachedCurve1"), plug(node, A_OUT))?.1
    };
    g.evaluate();
    let tr = crate::dag::transform_of(g, target);
    let mut out = vec![json!(g.path(tr))];
    if history_on(g, a) {
        out.push(json!(g.path(node)));
    } else {
        bake_history(g, target)?;
    }
    g.select(&[tr], SelectMode::Replace);
    Ok(Value::Array(out))
}

fn detach(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let Some(n) = names.first() else { fail!("detachCurve: name or select a curve") };
    let shape = curve_arg(g, n, "detachCurve")?;
    let mut params = Vec::new();
    if let Some(v) = a.get("parameter") {
        flat_numbers(v, &mut params);
    }
    if params.is_empty() {
        fail!("detachCurve: give -p");
    }
    params.sort_by(f64::total_cmp);
    let rpo = a.get("replaceOriginal").and_then(Value::as_bool).unwrap_or(true);
    let t = types::find("detachCurve").expect("detachCurve");
    let node = g.create_node(&t, "detachCurve1", NO_NODE, true)?;
    for (i, u) in params.iter().enumerate() {
        g.set_value(PlugRef::element(node, D_PARAM as i32, i as i32), PlugValue::Float(*u))?;
    }
    let tr = crate::dag::transform_of(g, shape);
    let base = g.node(tr).map(|x| x.name.clone()).unwrap_or_default();
    let world = crate::rig::world(g, tr);
    feed(g, shape, plug(node, D_IN))?;
    let mut made: Vec<NodeId> = Vec::new();
    for i in 0..=params.len() {
        let from = PlugRef::element(node, D_OUT as i32, i as i32);
        g.set_value(PlugRef::element(node, D_KEEP as i32, i as i32), PlugValue::Bool(true))?;
        if i == 0 && rpo {
            g.connect(&from, &plug(shape, CurveShape::CREATE), true)?;
            made.push(shape);
        } else {
            let (t2, s2) = curve_fed_by(g, &format!("{base}detachedCurve{}", i + 1), from)?;
            // The pieces are in the curve's own space: their transforms take its place in the world.
            crate::transform::Transform::set_local_matrix(g, t2, &world).map_err(|e| aztek_ops::CommandError::new(e.to_string()))?;
            made.push(s2);
        }
    }
    g.evaluate();
    let ch = history_on(g, a);
    let mut out: Vec<Value> = made.iter().map(|&sh| json!(g.path(crate::dag::transform_of(g, sh)))).collect();
    if ch {
        out.push(json!(g.path(node)));
    } else {
        for &sh in &made {
            bake_history(g, sh)?;
        }
        if g.node(node).is_some() {
            g.delete_node(node)?;
        }
    }
    Ok(Value::Array(out))
}

fn edges_to_curve(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let degree = a.int("degree", 3).max(1);
    let form = a.int("form", 2);
    let ch = history_on(g, a);
    let mut out = Vec::new();
    let sel = g.selection().to_vec();
    for &item in &sel {
        let Some(shape) = components::as_mesh(g, item) else { continue };
        if components::mode(g) != ComponentMode::Edge {
            continue;
        }
        let ids = components::selected(g, shape, ComponentMode::Edge).to_vec();
        if ids.is_empty() {
            continue;
        }
        let t = types::find("polyEdgeToCurve").expect("polyEdgeToCurve");
        let node = g.create_node(&t, "polyEdgeToCurve1", NO_NODE, true)?;
        g.set_value(plug(node, E_COMPONENTS), PlugValue::String(crate::history::format_components(ComponentMode::Edge, &ids)))?;
        g.set_value(plug(node, E_DEGREE), PlugValue::Int(degree))?;
        g.set_value(plug(node, E_FORM), PlugValue::Int(form))?;
        g.connect(&plug(shape, MeshNode::OUT_MESH), &plug(node, E_MESH), false)?;
        let xf = crate::dag::transform_of(g, shape);
        g.connect(&plug(xf, crate::transform::Transform::WORLD_MATRIX), &plug(node, E_MAT), false)?;
        let (tr, sh) = curve_fed_by(g, &a.string("name", "polyToCurve1"), plug(node, E_OUT))?;
        g.evaluate();
        out.push(json!(g.path(tr)));
        if ch {
            out.push(json!(g.path(node)));
        } else {
            bake_history(g, sh)?;
            if g.node(node).is_some() {
                g.delete_node(node)?;
            }
        }
    }
    if out.is_empty() {
        fail!("polyToCurve: select edges of a mesh");
    }
    if let Some(Value::String(first)) = out.first() {
        if let Ok(Some(id)) = g.find(first) {
            g.select(&[id], SelectMode::Replace);
        }
    }
    Ok(Value::Array(out))
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        Cmd::new(
            "attachCurve",
            "Join curves end to end through an attachCurve node. -rpo (on by default) changes the first curve; off, a new curve. Returns [curve, node].",
            attach,
        )
        .positional("curves (the selection when none)")
        .flag(f("replaceOriginal", "rpo", ArgType::Bool, "The first curve takes the result (on by default)."))
        .flag(f("name", "n", ArgType::String, "Name of the new curve."))
        .flag(f("method", "m", ArgType::Int, "0 connect, 1 blend."))
        .flag(f("blendBias", "bb", ArgType::Float, "Which curve a blend leans to."))
        .flag(f("blendKnotInsertion", "bki", ArgType::Bool, "Insert a knot for a blend."))
        .flag(f("keepMultipleKnots", "kmk", ArgType::Bool, "Keep the knots where they meet."))
        .flag(f("parameter", "p", ArgType::Float, "Where a blend's knot goes."))
        .flag(f("reverse1", "rv1", ArgType::Bool, "Reverse the first curve."))
        .flag(f("reverse2", "rv2", ArgType::Bool, "Reverse the second curve."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("object", "o", ArgType::Bool, "Make an object (accepted for existing scripts).")),
    );
    c.register(
        Cmd::new(
            "detachCurve",
            "Cut a curve at -p (repeat it) through a detachCurve node, one curve per piece. -rpo (on by default): the first piece is the curve itself. Returns the pieces, then the node.",
            detach,
        )
        .positional("a curve (the selection when none)")
        .flag(f("parameter", "p", ArgType::List, "Where to cut. Repeat it.").multi(1))
        .flag(f("replaceOriginal", "rpo", ArgType::Bool, "The first piece is the curve itself (on by default)."))
        .flag(f("keep", "k", ArgType::Bool, "Keep the pieces (accepted; all are kept)."))
        .flag(f("name", "n", ArgType::String, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("object", "o", ArgType::Bool, "Make objects (accepted for existing scripts).")),
    );
    c.register(
        Cmd::new(
            "polyToCurve",
            "A curve through a chain of selected edges, as the reference's Mesh > Create Curve From Edges, fed live by a polyEdgeToCurve node: -dg degree (3), -f 0 open, 1 periodic, 2 best guess. Returns [curve, node].",
            edges_to_curve,
        )
        .flag(f("degree", "dg", ArgType::Int, "1 linear, 3 cubic."))
        .flag(f("form", "f", ArgType::Int, "0 open, 1 periodic, 2 best guess."))
        .flag(f("conformToSmoothMeshPreview", "usm", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("constructionHistory", "ch", ArgType::Bool, "Keep the node (on by default)."))
        .flag(f("name", "n", ArgType::String, "Name of the curve.")),
    );
}
