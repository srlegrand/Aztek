//! Painting per-point attributes on deformers, as the reference's artAttrCtx: the same brush as paint skin weights, on a deformer's
//! weights or a blend shape's base and target weights. The attribute is named as the reference names it to artSetToolAndSelectAttr:
//! cluster.cluster1.weights, blendShape.blendShape1.baseWeights, blendShape.blendShape1.paintTargetWeights, with
//! targetWeights[2] for a target by index.
//!
//! Also the plug spellings of those weights for getAttr and setAttr: cluster1.weightList[0].weights[3],
//! blendShape1.inputTarget[0].baseWeights[3], blendShape1.inputTarget[0].inputTargetGroup[1].targetWeights[3].

use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::components;
use crate::deform::{self, f, floats_of, input_mesh, set_floats, Cmd, WEIGHTS};
use crate::deform_blend::{blend_of, set_blend, PAINT_TARGET};
use crate::graph::Graph;
use crate::plug::NodeId;
use crate::rig::world;
use crate::session::Session;
use crate::skin::{asked, modes, PaintCtx, PaintOp, Stamp};

/// The paint attribute tool's settings: the brush, and what it paints.
#[derive(Clone, Debug, Default)]
pub struct AttrCtx {
    pub paint: PaintCtx,
    pub attr: String,
    /// Names made by artAttrCtx, for setToolTo. They share these settings.
    pub tools: Vec<String>,
}

/// Where a deformer keeps some per-point values.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Slot {
    /// The deformer's weights; for a blend shape, the base weights.
    Weights,
    /// A blend shape target's weights.
    Target(i32),
    /// A skinCluster's blendWeights, for skinningMethod 2.
    SkinBlend,
    /// A per point map held in the node's own attribute, by index: cloth's input attract, mass and pins.
    Map(u16),
}

/// Cloth's paintable maps.
const CLOTH_MAPS: &[&str] = &["inputAttractPerVertex", "massPerVertex", "pinPerVertex"];

/// Whether a deformer type has weights to paint.
fn paintable(ty: &str) -> bool {
    crate::deform::is_deformer(ty)
}

/// What the tool can paint on a mesh, as artSetToolAndSelectAttr names it.
pub fn paintables(g: &Graph, mesh: NodeId) -> Vec<String> {
    let mut out = Vec::new();
    for h in crate::history::history_of(g, mesh) {
        let Some(n) = g.node(h) else { continue };
        let ty = n.ty().name.clone();
        if !paintable(&ty) {
            continue;
        }
        let name = n.name().to_string();
        if ty == crate::dynamics::CLOTH {
            for m in CLOTH_MAPS {
                out.push(format!("{ty}.{name}.{m}"));
            }
        } else if ty == "blendShape" {
            out.push(format!("blendShape.{name}.baseWeights"));
            for t in blend_of(g, h).targets {
                out.push(format!("blendShape.{name}.targetWeights[{}]", t.index));
            }
        } else {
            out.push(format!("{ty}.{name}.weights"));
        }
    }
    out
}

/// The deformer node on this mesh and the slot an attribute name stands for.
pub fn resolve(g: &Graph, attr: &str, mesh: NodeId) -> Option<(NodeId, Slot)> {
    let mut parts = attr.split('.');
    let (_ty, node, what) = (parts.next()?, parts.next()?, parts.next()?);
    let id = g.find(node).ok().flatten()?;
    let n = deform::node_on(g, id, mesh)?;
    let slot = match what {
        m if CLOTH_MAPS.contains(&m) => Slot::Map(g.node(n)?.ty().attr_index(m)? as u16),
        "weights" | "baseWeights" => Slot::Weights,
        "paintTargetWeights" => Slot::Target(g.node(n)?.data().int(PAINT_TARGET) as i32),
        w => Slot::Target(w.strip_prefix("targetWeights[")?.strip_suffix(']')?.parse().ok()?),
    };
    Some((n, slot))
}

/// The attribute the tool is on for this mesh: the one chosen, or else the first the mesh offers.
pub fn current(s: &Session, mesh: NodeId) -> Option<(NodeId, Slot)> {
    let g = &s.graph;
    if !s.attr_ctx.attr.is_empty() {
        if let Some(x) = resolve(g, &s.attr_ctx.attr, mesh) {
            return Some(x);
        }
    }
    paintables(g, mesh).first().and_then(|a| resolve(g, a, mesh))
}

/// The values, one per point of the given capacity. What was never painted is 1.
pub fn read(g: &Graph, n: NodeId, slot: Slot, capacity: usize) -> Vec<f64> {
    let stored = match slot {
        Slot::Weights => g.node(n).and_then(|x| floats_of(x.data(), WEIGHTS)).map(|w| w.0.clone()),
        Slot::Target(i) => blend_of(g, n).targets.into_iter().find(|t| t.index == i).and_then(|t| t.weights),
        Slot::SkinBlend => g.node(n).and_then(|x| floats_of(x.data(), crate::skin::SK_BLEND)).map(|w| w.0.clone()),
        Slot::Map(a) => g.node(n).and_then(|x| floats_of(x.data(), a as usize)).map(|w| w.0.clone()),
    };
    // Unpainted: no blend, no pins; everything else at full strength.
    let is_pins = matches!(slot, Slot::Map(a) if g.node(n).is_some_and(|x| x.ty().attrs[a as usize].name == "pinPerVertex"));
    let unset = if slot == Slot::SkinBlend || is_pins { 0.0 } else { 1.0 };
    let mut w = stored.unwrap_or_else(|| vec![unset; capacity]);
    w.resize(capacity, 0.0);
    w
}

/// Sets the values, as one undoable change.
pub fn write(g: &mut Graph, n: NodeId, slot: Slot, w: Vec<f64>) -> CmdResult<()> {
    match slot {
        Slot::Weights => set_floats(g, n, WEIGHTS, w),
        Slot::SkinBlend => set_floats(g, n, crate::skin::SK_BLEND, w),
        Slot::Map(a) => set_floats(g, n, a as usize, w),
        Slot::Target(i) => {
            let mut b = blend_of(g, n);
            let Some(t) = b.targets.iter_mut().find(|t| t.index == i) else { fail!("no target {i} on {}", g.path(n)) };
            t.weights = Some(w);
            set_blend(g, n, b)
        }
    }
}

/// Which points the brush may touch: members of the deformer's set. A blend shape's target weights go by the same set.
pub fn mask(g: &Graph, n: NodeId, capacity: usize) -> Option<Vec<bool>> {
    crate::deform_sets::member_mask(g, n, capacity)
}

fn apply_op(now: f64, op: PaintOp, value: f64, k: f64, invert: bool) -> f64 {
    let next = match op {
        PaintOp::Replace => now + ((if invert { 1.0 - value } else { value }) - now) * k,
        PaintOp::Add => now + (if invert { -value } else { value }) * k,
        PaintOp::Scale => now * (1.0 + ((if invert { 1.0 / value.max(1e-6) } else { value }) - 1.0) * k),
        PaintOp::Smooth => now,
    };
    next.clamp(0.0, 1.0)
}

fn average_around(w: &[f64], m: &Mesh, p: u32) -> Option<f64> {
    let (mut sum, mut count) = (0.0, 0.0);
    m.for_each_point_edge(p, |e| {
        sum += w.get(m.edge_other_point(e, p) as usize).copied().unwrap_or(0.0);
        count += 1.0;
    });
    (count > 0.0).then(|| sum / count)
}

/// One dab of the brush on some values, as skin::stamp does for skin weights. Returns the points touched.
#[allow(clippy::too_many_arguments)]
pub fn stamp(w: &mut [f64], mask: Option<&[bool]>, m: &Mesh, world: &DMat4, center: DVec3, normal: Option<DVec3>, ctx: &PaintCtx, invert: bool, smooth: bool) -> usize {
    let mut hit = Vec::new();
    for p in 0..m.point_capacity() {
        if !m.point_valid(p) || p as usize >= w.len() || mask.is_some_and(|x| !x.get(p as usize).copied().unwrap_or(false)) {
            continue;
        }
        let wp = world.transform_point3(m.position(p));
        let t = match ctx.stamp {
            Stamp::Square => (wp - center).abs().max_element() / ctx.radius.max(1e-9),
            _ => (wp - center).length() / ctx.radius.max(1e-9),
        };
        if t >= 1.0 {
            continue;
        }
        if let Some(n) = normal {
            let mut pn = DVec3::ZERO;
            for face in m.point_faces(p) {
                pn += m.face_area_vector(face);
            }
            if world.transform_vector3(pn).normalize_or_zero().dot(n) < 0.0 {
                continue;
            }
        }
        hit.push((p, crate::skin::falloff(ctx.stamp, t) * ctx.opacity));
    }
    let op = if smooth { PaintOp::Smooth } else { ctx.op };
    if op == PaintOp::Smooth {
        let old = w.to_vec();
        for &(p, k) in &hit {
            if let Some(avg) = average_around(&old, m, p) {
                w[p as usize] = old[p as usize] + (avg - old[p as usize]) * k;
            }
        }
    } else {
        for &(p, k) in &hit {
            w[p as usize] = apply_op(w[p as usize], op, ctx.value, k, invert);
        }
    }
    hit.len()
}

/// Flood: the brush's operation over the selected points of the mesh, all of them if none is selected.
fn flood(s: &mut Session, mesh: NodeId) -> CmdResult<()> {
    let Some((n, slot)) = current(s, mesh) else { return Ok(()) };
    let ctx = s.attr_ctx.paint.clone();
    let g = &mut s.graph;
    let Some(m) = components::mesh_of(g, mesh) else { return Ok(()) };
    let size = m.point_capacity() as usize;
    let mut w = read(g, n, slot, size);
    let allowed = mask(g, n, size);
    let ids = crate::skin::selected_points(g, mesh);
    let old = w.clone();
    for p in ids {
        if allowed.as_ref().is_some_and(|x| !x[p as usize]) {
            continue;
        }
        if ctx.op == PaintOp::Smooth {
            if let Some(avg) = average_around(&old, &m, p) {
                w[p as usize] = old[p as usize] + (avg - old[p as usize]) * ctx.opacity;
            }
        } else {
            w[p as usize] = apply_op(old[p as usize], ctx.op, ctx.value, ctx.opacity, false);
        }
    }
    write(g, n, slot, w)
}

// ---------------------------------------------------------------------------------------------------------------------
// Plug spellings

fn index_in(text: &str, name: &str) -> Option<String> {
    Some(text.strip_prefix(name)?.strip_prefix('[')?.strip_suffix(']')?.to_string())
}

/// The ids of "3", "0:7" or "*" against a size; none means them all.
fn ids_of(range: Option<&str>, size: usize) -> Option<Vec<usize>> {
    let Some(r) = range.filter(|r| *r != "*") else { return Some((0..size).collect()) };
    let (a, b) = r.split_once(':').unwrap_or((r, r));
    let (a, b): (usize, usize) = (a.parse().ok()?, b.parse().ok()?);
    Some((a..=b).collect())
}

/// A per-point weights plug: (node, slot, range), for what getAttr and setAttr can't reach through a PlugRef.
fn weights_path(g: &Graph, path: &str) -> Option<(NodeId, Slot, Option<String>)> {
    let (node, rest) = path.split_once('.')?;
    let id = g.find(node).ok().flatten()?;
    let ty = g.node(id)?;
    if ty.ty().name == crate::skin::SKIN_TYPE {
        let range = if rest == "blendWeights" || rest == "bw" { None } else { Some(index_in(rest, "blendWeights").or_else(|| index_in(rest, "bw"))?) };
        return Some((id, Slot::SkinBlend, range));
    }
    if !deform::is_deformer(ty.ty().name.as_str()) {
        return None;
    }
    let parts: Vec<&str> = rest.split('.').collect();
    let pick = |g_index: &str| -> Option<NodeId> { deform::group(g, id).get(g_index.parse::<usize>().ok()?).copied() };
    let range_of = |text: &str, name: &str| -> Option<Option<String>> {
        if text == name {
            return Some(None);
        }
        index_in(text, name).map(Some)
    };
    // Long and short names, as the reference takes either.
    let elem = |text: &str, long: &str, brief: &str| index_in(text, long).or_else(|| index_in(text, brief));
    let tail = |text: &str, long: &str, brief: &str| range_of(text, long).or_else(|| range_of(text, brief));
    match parts[..] {
        [wl, w] if wl.starts_with("weightList") || wl.starts_with("wl[") => Some((pick(&elem(wl, "weightList", "wl")?)?, Slot::Weights, tail(w, "weights", "w")?)),
        [it, bw] if it.starts_with("inputTarget") || it.starts_with("it[") => Some((pick(&elem(it, "inputTarget", "it")?)?, Slot::Weights, tail(bw, "baseWeights", "bw")?)),
        [it, tg, tw] if it.starts_with("inputTarget") || it.starts_with("it[") => {
            let n = pick(&elem(it, "inputTarget", "it")?)?;
            let t: i32 = elem(tg, "inputTargetGroup", "itg")?.parse().ok()?;
            Some((n, Slot::Target(t), tail(tw, "targetWeights", "tw")?))
        }
        _ => None,
    }
}

/// getAttr of a per-point weights plug. None when this isn't one.
pub(crate) fn get_weights(g: &mut Graph, path: &str) -> Option<CmdResult> {
    let (n, slot, range) = weights_path(g, path)?;
    let size = input_mesh(g, n)?.point_capacity() as usize;
    let w = read(g, n, slot, size);
    let ids = ids_of(range.as_deref(), size)?;
    let single = range.as_deref().is_some_and(|r| !r.contains(':') && r != "*");
    let vals: Vec<f64> = ids.iter().map(|&i| w.get(i).copied().unwrap_or(0.0)).collect();
    Some(Ok(if single { json!(vals[0]) } else { json!(vals) }))
}

/// setAttr of a per-point weights plug. None when this isn't one.
pub(crate) fn set_weights(g: &mut Graph, path: &str, v: &[Value]) -> Option<CmdResult> {
    let (n, slot, range) = weights_path(g, path)?;
    let size = input_mesh(g, n)?.point_capacity() as usize;
    let mut w = read(g, n, slot, size);
    let ids = ids_of(range.as_deref(), size)?;
    let flat: Vec<f64> = v.iter().flat_map(|x| x.as_array().cloned().unwrap_or_else(|| vec![x.clone()])).filter_map(|x| x.as_f64()).collect();
    for (k, &i) in ids.iter().enumerate() {
        let Some(&x) = flat.get(k).or(flat.last()) else { break };
        if let Some(e) = w.get_mut(i) {
            *e = x;
        }
    }
    Some(write(g, n, slot, w).map(|_| Value::Null))
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

fn set_brush(ctx: &mut PaintCtx, a: &Args, cmd: &str) -> CmdResult<()> {
    if a.has("value") {
        ctx.value = a.float("value", 1.0).clamp(0.0, 1.0);
    }
    if a.has("opacity") {
        ctx.opacity = a.float("opacity", 1.0).clamp(0.0, 1.0);
    }
    if a.has("radius") {
        ctx.radius = a.float("radius", 1.0).max(1e-4);
    }
    if a.has("selectedattroper") {
        ctx.op = crate::skin::op_named(&a.string("selectedattroper", ""), cmd)?;
    }
    if a.has("stampProfile") {
        ctx.stamp = crate::skin::stamp_named(&a.string("stampProfile", ""), cmd)?;
    }
    if a.has("colorfeedback") {
        ctx.color_feedback = a.bool("colorfeedback");
    }
    Ok(())
}

pub fn select_attr(s: &mut Session, attr: &str) -> CmdResult<()> {
    let mut parts = attr.split('.');
    let (Some(_), Some(node), Some(_)) = (parts.next(), parts.next(), parts.next()) else { fail!("artAttrCtx: {attr} is not nodeType.node.attribute") };
    if s.graph.find(node)?.is_none() {
        fail!("artAttrCtx: no node named {node}");
    }
    s.attr_ctx.attr = attr.to_string();
    Ok(())
}

fn meshes_selected(s: &Session) -> Vec<NodeId> {
    let g = &s.graph;
    let mut out = Vec::new();
    for &id in g.selection() {
        if let Some(m) = components::as_mesh(g, id) {
            if !out.contains(&m) {
                out.push(m);
            }
        }
    }
    if out.is_empty() {
        out.extend(components::selected_all(g, components::mode(g)).keys().filter_map(|&n| components::as_mesh(g, n)));
    }
    out
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(modes(
        Cmd::new(
            "artAttrCtx",
            "The paint attribute tool's settings. Edit and query: -val value, -op opacity, -r radius, -sao the operation (absolute, additive, \
             scale, smooth), -stP the stamp (gaussian, poly, solid, square), -asl what it paints, as cluster.cluster1.weights. -clear floods. \
             -q -pa lists what a selected mesh offers.",
            |s, a| {
                if a.bool("query") {
                    let ctx = &s.attr_ctx.paint;
                    for (flag, v) in [
                        ("value", json!(ctx.value)),
                        ("opacity", json!(ctx.opacity)),
                        ("radius", json!(ctx.radius)),
                        ("selectedattroper", json!(crate::skin::op_name(ctx.op))),
                        ("stampProfile", json!(crate::skin::stamp_name(ctx.stamp))),
                        ("colorfeedback", json!(ctx.color_feedback)),
                        ("attrSelected", json!(s.attr_ctx.attr)),
                    ] {
                        if asked(a, flag) {
                            return Ok(v);
                        }
                    }
                    if asked(a, "paintableAttrs") {
                        let list: Vec<String> = meshes_selected(s).into_iter().flat_map(|m| paintables(&s.graph, m)).collect();
                        return Ok(json!(list));
                    }
                    fail!("artAttrCtx: nothing to query");
                }
                let mut ctx = s.attr_ctx.paint.clone();
                set_brush(&mut ctx, a, "artAttrCtx")?;
                s.attr_ctx.paint = ctx;
                if a.has("attrSelected") {
                    select_attr(s, &a.string("attrSelected", ""))?;
                }
                if a.bool("clear") {
                    for m in meshes_selected(s) {
                        flood(s, m)?;
                    }
                }
                if a.bool("edit") {
                    return Ok(Value::Null);
                }
                let name = a.pos_str(0).unwrap_or("artAttrCtx1").to_string();
                if !s.attr_ctx.tools.contains(&name) {
                    s.attr_ctx.tools.push(name.clone());
                }
                Ok(json!(name))
            },
        )
        .positional("the tool's name")
        .flag(f("value", "val", ArgType::Float, "The value the brush paints, 0 to 1."))
        .flag(f("opacity", "op", ArgType::Float, "How much of the value each dab applies."))
        .flag(f("radius", "r", ArgType::Float, "Brush radius in world units."))
        .flag(f("selectedattroper", "sao", ArgType::String, "absolute (replace), additive, scale or smooth."))
        .flag(f("stampProfile", "stP", ArgType::String, "gaussian, poly, solid or square."))
        .flag(f("attrSelected", "asl", ArgType::String, "What it paints: cluster.cluster1.weights."))
        .flag(f("paintableAttrs", "pa", ArgType::Bool, "Query: what the selected meshes offer."))
        .flag(f("colorfeedback", "cl", ArgType::Bool, "Show the values in colour."))
        .flag(f("clear", "clr", ArgType::Bool, "Flood: the operation over the selection's points.")),
    ));
    c.register(
        Cmd::new(
            "attrPaintStamp",
            "One dab of the paint attribute brush at a world point, from the tool's settings: for scripts and tests. -inv paints the inverse, as \
             Ctrl does in the tool; -sm smooths, as Shift does. Returns how many points it touched.",
            |s, a| {
                let names = objects(s, a);
                let Some(name) = names.first() else { fail!("attrPaintStamp: name a deformed mesh") };
                let id = crate::rig::find(&s.graph, name, "attrPaintStamp")?;
                let Some(mesh) = components::as_mesh(&s.graph, id) else { fail!("attrPaintStamp: {name} is not a mesh") };
                let Some(at) = a.vec3("position") else { fail!("attrPaintStamp: -p x y z") };
                let Some((n, slot)) = current(s, mesh) else { fail!("attrPaintStamp: {name} has nothing to paint") };
                let ctx = s.attr_ctx.paint.clone();
                let g = &mut s.graph;
                let Some(m) = components::mesh_of(g, mesh) else { fail!("attrPaintStamp: no geometry") };
                let xf = crate::dag::transform_of(g, mesh);
                let wm = world(g, xf);
                let size = m.point_capacity() as usize;
                let mut w = read(g, n, slot, size);
                let allowed = mask(g, n, size);
                let touched = stamp(&mut w, allowed.as_deref(), &m, &wm, DVec3::from_array(at), None, &ctx, a.bool("invert"), a.bool("smooth"));
                write(g, n, slot, w)?;
                Ok(json!(touched))
            },
        )
        .positional("a deformed mesh")
        .flag(f("position", "p", ArgType::Vec3, "World centre of the dab."))
        .flag(f("invert", "inv", ArgType::Bool, "Paint the inverse."))
        .flag(f("smooth", "sm", ArgType::Bool, "Smooth instead.")),
    );
}
