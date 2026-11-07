//! Animation layers as the reference builds them: an `animLayer` node per layer with BaseAnimation as the root, and for each
//! channel in a layer a blend node (`animBlendNodeAdditiveDL`, `...Rotation`, `...Scale`, `animBlendNodeBoolean`, ...). The blends
//! of a channel are a chain: the output of one is `inputA` of the next, the base curve at the bottom, the channel at the
//! top. A rotation blend takes its three axes by component, as the reference's files connect them.
//! The nodes are in `anim_layer_nodes.rs`, the commands in `anim_layer_cmds.rs`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use aztek_ops::{fail, CmdResult, Commands};
use glam::{DMat3, DQuat, DVec3};

pub use crate::anim_layer_nodes::*;

use crate::anim::{self, OUTPUT, POST, PRE};
use crate::graph::Graph;
use crate::node::types;
use crate::plug::{NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::{euler_near, RotateOrder, Transform};

pub const BASE_LAYER: &str = "BaseAnimation";

/// Options > Key in Selected Layer(s) (0), Key in Last Active Layer(s) (1), Hybrid (2, the default).
pub static KEY_MODE: AtomicI64 = AtomicI64::new(2);
/// Show > Other Animation Tools: All Affecting (0), Active (1), Selected (2), Active and Selected (3).
pub static SHOW_FILTER: AtomicI64 = AtomicI64::new(0);
/// Options > Lock Muted Layer.
pub static LOCK_MUTED: AtomicBool = AtomicBool::new(false);

pub(crate) type Skip = HashSet<NodeId>;

// ---------------------------------------------------------------------------------------------------------------------
// Queries over the graph

pub fn is_layer(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().name == "animLayer")
}

pub fn is_blend(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().name.starts_with("animBlendNode"))
}

/// Blend or mute: nodes a channel's animation passes through.
pub fn is_routing(g: &Graph, id: NodeId) -> bool {
    is_blend(g, id) || crate::anim_filter::is_mute(g, id)
}

pub(crate) fn is_rotation(g: &Graph, blend: NodeId) -> bool {
    g.node(blend).is_some_and(|n| n.ty().name == "animBlendNodeAdditiveRotation")
}

pub(crate) fn blend_kind(g: &Graph, blend: NodeId) -> BlendKind {
    match g.node(blend).map(|n| n.ty().name.as_str()).unwrap_or("") {
        "animBlendNodeAdditiveRotation" => BlendKind::Rotation,
        "animBlendNodeAdditiveScale" => BlendKind::Scale,
        "animBlendNodeBoolean" | "animBlendNodeEnum" => BlendKind::Switch,
        _ => BlendKind::Add,
    }
}

/// A mute's source, or the mute itself when nothing feeds it.
fn through_mutes(g: &Graph, mut s: PlugRef) -> Option<PlugRef> {
    while crate::anim_filter::is_mute(g, s.node) {
        s = g.source(&PlugRef::new(s.node, crate::anim_filter::MUTE_INPUT as i32))?;
    }
    Some(s)
}

/// What drives a plug, whole vector or component, looking through mute nodes. None when nothing does.
/// A whole vector whose axes are driven by a rotation blend answers with that blend.
pub fn driver(g: &Graph, p: &PlugRef) -> Option<PlugRef> {
    let s = g.source(p).or_else(|| if p.component >= 0 { g.source(&p.whole()) } else { None });
    if let Some(s) = s {
        return through_mutes(g, s);
    }
    if p.component < 0 && g.attr(p).ty == PlugType::Vec3 {
        let s = g.source(&PlugRef::component(p.node, p.attr, 0))?;
        return is_blend(g, s.node).then_some(s);
    }
    None
}

/// Where a new curve for a plug connects: the input of its mute node when it has one, else the plug.
pub fn curve_slot(g: &Graph, p: &PlugRef) -> PlugRef {
    match g.source(p) {
        Some(s) if crate::anim_filter::is_mute(g, s.node) => PlugRef::new(s.node, crate::anim_filter::MUTE_INPUT as i32),
        _ => *p,
    }
}

/// The plugs a connection into a routing node ends up driving, or the plug itself.
pub fn final_plugs(g: &Graph, d: PlugRef) -> Vec<PlugRef> {
    if crate::anim_filter::is_mute(g, d.node) {
        return g.destinations(&PlugRef::new(d.node, crate::anim_filter::MUTE_OUTPUT as i32)).into_iter().flat_map(|x| final_plugs(g, x)).collect();
    }
    if !is_blend(g, d.node) || (d.attr != IN_A as i32 && d.attr != IN_B as i32) {
        return vec![d];
    }
    let mut out = Vec::new();
    if is_rotation(g, d.node) {
        let axes: Vec<i32> = if d.component >= 0 { vec![d.component] } else { vec![0, 1, 2] };
        for c in axes {
            for x in g.destinations(&PlugRef::component(d.node, BLEND_OUT as i32, c)) {
                out.extend(final_plugs(g, x));
            }
            for x in g.destinations(&PlugRef::new(d.node, BLEND_OUT as i32)) {
                for y in final_plugs(g, x) {
                    let whole = y.component < 0 && g.attr(&y).ty == PlugType::Vec3;
                    out.push(if whole { PlugRef::component(y.node, y.attr, c) } else { y });
                }
            }
        }
    } else {
        for x in g.destinations(&PlugRef::new(d.node, BLEND_OUT as i32)) {
            out.extend(final_plugs(g, x));
        }
    }
    out
}

pub(crate) fn input(g: &Graph, blend: NodeId, which: usize, p: &PlugRef) -> PlugRef {
    if is_rotation(g, blend) {
        PlugRef::component(blend, which as i32, p.component.max(0))
    } else {
        PlugRef::new(blend, which as i32)
    }
}

fn source_of(g: &Graph, plug: &PlugRef) -> Option<PlugRef> {
    g.source(plug).or_else(|| if plug.component >= 0 { g.source(&plug.whole()) } else { None }).and_then(|x| through_mutes(g, x))
}

/// The blends on a plug, topmost first.
pub fn chain(g: &Graph, p: &PlugRef) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut s = driver(g, p);
    while let Some(x) = s {
        if !is_blend(g, x.node) || out.contains(&x.node) {
            break;
        }
        out.push(x.node);
        s = source_of(g, &input(g, x.node, IN_A, p));
    }
    out
}

pub fn blend_layer(g: &Graph, blend: NodeId) -> Option<NodeId> {
    g.source(&PlugRef::new(blend, WEIGHT_B as i32)).map(|s| s.node).filter(|&l| is_layer(g, l))
}

pub fn base_layer(g: &Graph) -> Option<NodeId> {
    if let Some(id) = g.find(BASE_LAYER).ok().flatten() {
        if is_layer(g, id) {
            return Some(id);
        }
    }
    g.nodes().into_iter().find(|&id| is_layer(g, id) && layer_parent(g, id).is_none())
}

pub(crate) fn layer_parent(g: &Graph, l: NodeId) -> Option<NodeId> {
    g.destinations(&PlugRef::new(l, L_PARENT_LAYER as i32)).into_iter().find(|d| d.attr == L_CHILDREN as i32 && is_layer(g, d.node)).map(|d| d.node)
}

/// A layer's children, bottom of the stack first.
pub(crate) fn layer_children(g: &Graph, l: NodeId) -> Vec<NodeId> {
    g.elements(&PlugRef::new(l, L_CHILDREN as i32)).into_iter().filter_map(|e| g.source(&e)).map(|s| s.node).filter(|&c| is_layer(g, c)).collect()
}

/// All layers bottom to top: the base, then each layer followed by its children.
pub fn ordered_layers(g: &Graph) -> Vec<NodeId> {
    fn walk(g: &Graph, l: NodeId, out: &mut Vec<NodeId>) {
        out.push(l);
        for c in layer_children(g, l) {
            walk(g, c, out);
        }
    }
    let mut out = Vec::new();
    if let Some(b) = base_layer(g) {
        walk(g, b, &mut out);
    }
    out
}

pub(crate) fn layer_flag(g: &Graph, l: NodeId, attr: usize) -> bool {
    g.node(l).is_some_and(|n| n.data().value(attr).scalar().unwrap_or(0.0) != 0.0)
}

fn layer_num(g: &Graph, l: NodeId, attr: usize, fallback: f64) -> f64 {
    g.node(l).and_then(|n| n.data().value(attr).scalar()).unwrap_or(fallback)
}

/// Off because a sibling is soloed and it isn't.
fn solo_off(g: &Graph, l: NodeId) -> bool {
    if layer_flag(g, l, L_SOLO) {
        return false;
    }
    layer_parent(g, l).is_some_and(|p| layer_children(g, p).iter().any(|&c| layer_flag(g, c, L_SOLO)))
}

/// Left out of the result: muted, under a parent that is off, or muted by a sibling's solo.
pub(crate) fn layer_off(g: &Graph, l: NodeId) -> bool {
    layer_flag(g, l, L_MUTE) || solo_off(g, l) || layer_parent(g, l).is_some_and(|p| layer_off(g, p))
}

/// Whether keys may go on a layer.
pub(crate) fn can_key(g: &Graph, l: NodeId) -> bool {
    let mut up = layer_parent(g, l);
    while let Some(x) = up {
        if solo_off(g, x) {
            return false;
        }
        up = layer_parent(g, x);
    }
    !layer_flag(g, l, L_LOCK) && !solo_off(g, l) && !(LOCK_MUTED.load(Ordering::Relaxed) && layer_flag(g, l, L_MUTE))
}

/// The layer keys on this plug go to, by the key mode. None when there is nowhere to key.
pub(crate) fn key_target(g: &Graph, p: &PlugRef) -> Option<NodeId> {
    let mut stack: Vec<NodeId> = chain(g, p).iter().filter_map(|&b| blend_layer(g, b)).collect();
    stack.extend(base_layer(g));
    let picked = |attr: usize| stack.iter().copied().find(|&l| layer_flag(g, l, attr) && can_key(g, l));
    let last = || stack.iter().copied().find(|&l| can_key(g, l));
    match KEY_MODE.load(Ordering::Relaxed) {
        0 => picked(L_SELECTED),
        1 => last(),
        _ => picked(L_SELECTED).or_else(|| picked(L_PREFERRED)).or_else(last),
    }
}

/// The input of a chain holding a layer's value: its blend's inputB, or the bottom blend's inputA for the base.
pub(crate) fn slot(g: &Graph, p: &PlugRef, layer: Option<NodeId>) -> Option<PlugRef> {
    let c = chain(g, p);
    match layer {
        Some(l) if Some(l) != base_layer(g) => c.iter().find(|&&b| blend_layer(g, b) == Some(l)).map(|&b| input(g, b, IN_B, p)),
        _ => c.last().map(|&b| input(g, b, IN_A, p)),
    }
}

pub(crate) fn curve_in(g: &Graph, slot: &PlugRef) -> Option<NodeId> {
    let s = source_of(g, slot)?;
    (anim::is_curve(g, s.node) && s.attr == OUTPUT as i32).then_some(s.node)
}

/// The curve keys on a layered plug are read from.
pub fn best_curve(g: &Graph, p: &PlugRef) -> Option<NodeId> {
    curve_in(g, &slot(g, p, key_target(g, p).or_else(|| base_layer(g)))?)
}

/// Every layer a plug is in, base first, with its curve if it has one.
pub fn plug_layer_curves(g: &Graph, p: &PlugRef) -> Vec<(NodeId, Option<NodeId>)> {
    let c = chain(g, p);
    if c.is_empty() {
        return Vec::new();
    }
    let mut out: Vec<(NodeId, Option<NodeId>)> = base_layer(g).map(|b| (b, curve_in(g, &input(g, c[c.len() - 1], IN_A, p)))).into_iter().collect();
    for &b in c.iter().rev() {
        if let Some(l) = blend_layer(g, b) {
            out.push((l, curve_in(g, &input(g, b, IN_B, p))));
        }
    }
    out
}

/// The curves that count as active on a plug: the layer keys go to, and the selected layers it is in.
pub fn active_curves(g: &Graph, p: &PlugRef) -> Vec<NodeId> {
    let target = key_target(g, p);
    plug_layer_curves(g, p).into_iter().filter(|(l, _)| Some(*l) == target || layer_flag(g, *l, L_SELECTED)).filter_map(|(_, c)| c).collect()
}

/// A node's channels that are in layers, keys on the active layer or not.
pub fn layered_plugs(g: &Graph, node: NodeId) -> Vec<PlugRef> {
    let mut out: Vec<PlugRef> = g.connections().into_iter().filter(|(d, s)| d.node == node && is_blend(g, s.node) && s.attr == BLEND_OUT as i32).map(|(d, _)| d).collect();
    out.sort_by_key(|d| (d.attr, d.component));
    out
}

/// The layers' curves on a plug that the Graph Editor and Dope Sheet show, by the Show filter, base first.
pub fn shown_curves(g: &Graph, p: &PlugRef) -> Vec<(NodeId, NodeId)> {
    let filter = SHOW_FILTER.load(Ordering::Relaxed);
    let target = key_target(g, p);
    plug_layer_curves(g, p)
        .into_iter()
        .filter(|(l, _)| match filter {
            1 => Some(*l) == target,
            2 => layer_flag(g, *l, L_SELECTED),
            3 => Some(*l) == target || layer_flag(g, *l, L_SELECTED),
            _ => true,
        })
        .filter_map(|(l, c)| c.map(|c| (l, c)))
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Values at any frame, without evaluating the graph

/// A curve's value at a frame, whatever the current time: keys only.
pub(crate) fn curve_at(g: &Graph, c: NodeId, t: f64) -> f64 {
    let (pre, post) = g.node(c).map(|n| (n.data().int(PRE), n.data().int(POST))).unwrap_or((0, 0));
    anim::keys_of(g, c).eval(t, anim::infinity(pre), anim::infinity(post))
}

/// A layer's foreground and background weights at a frame: its weight may be keyed.
pub(crate) fn layer_weights_at(g: &Graph, l: NodeId, t: f64) -> (f64, f64) {
    if layer_off(g, l) {
        return (0.0, 1.0);
    }
    let own = match curve_in(g, &PlugRef::new(l, L_WEIGHT as i32)) {
        Some(c) => curve_at(g, c, t),
        None => layer_num(g, l, L_WEIGHT, 1.0),
    };
    let parent = layer_parent(g, l).map(|p| layer_weights_at(g, p, t).0).unwrap_or(1.0);
    let fg = own * parent;
    let bg = if !layer_flag(g, l, L_OVERRIDE) {
        1.0
    } else if layer_flag(g, l, L_PASSTHROUGH) {
        1.0 - fg
    } else {
        0.0
    };
    (fg, bg)
}

/// wa, wb of a blend at a frame.
pub(crate) fn weights_at(g: &Graph, blend: NodeId, t: f64) -> (f64, f64) {
    match blend_layer(g, blend) {
        Some(l) => {
            let (fg, bg) = layer_weights_at(g, l, t);
            (bg, fg)
        }
        None => {
            let d = g.node(blend).map(|n| n.data());
            d.map(|d| (d.float(WEIGHT_A), d.float(WEIGHT_B))).unwrap_or((1.0, 1.0))
        }
    }
}

fn accumulation(g: &Graph, blend: NodeId) -> i64 {
    match (blend_layer(g, blend), blend_kind(g, blend)) {
        (Some(l), BlendKind::Rotation) => layer_num(g, l, L_RAM, 0.0) as i64,
        (Some(l), BlendKind::Scale) => layer_num(g, l, L_SAM, 1.0) as i64,
        _ => g.node(blend).and_then(|n| n.data().value(ACCUMULATE).scalar()).unwrap_or(0.0) as i64,
    }
}

fn rotate_order(g: &Graph, blend: NodeId) -> RotateOrder {
    let at = PlugRef::new(blend, BLEND_ROT_ORDER as i32);
    let plug = g.source(&at).unwrap_or(at);
    RotateOrder::from_index(g.stored_value(&plug).scalar().unwrap_or(0.0) as i64)
}

/// A blend input's value at a frame: a curve, a lower blend, or what is stored. Blends in `skip` pass input A along.
fn input_at(g: &Graph, plug: &PlugRef, p: &PlugRef, t: f64, skip: &Skip) -> f64 {
    if let Some(s) = source_of(g, plug) {
        if anim::is_curve(g, s.node) && s.attr == OUTPUT as i32 {
            return curve_at(g, s.node, t);
        }
        if is_blend(g, s.node) {
            return blend_at(g, s.node, p, t, skip);
        }
    }
    match g.stored_value(&plug.whole()) {
        PlugValue::Vec3(v) => v[plug.component.max(0) as usize],
        v => v.scalar().unwrap_or(0.0),
    }
}

/// The three axes of a rotation blend's input at a frame.
fn input_vec_at(g: &Graph, blend: NodeId, which: usize, t: f64, skip: &Skip) -> DVec3 {
    let axis = |c: i32| PlugRef::component(blend, which as i32, c);
    if let Some(s) = source_of(g, &axis(0)) {
        if is_rotation(g, s.node) {
            return blend_vec_at(g, s.node, t, skip);
        }
    }
    let probe = |c: i32| PlugRef::component(blend, which as i32, c);
    DVec3::new(input_at(g, &axis(0), &probe(0), t, skip), input_at(g, &axis(1), &probe(1), t, skip), input_at(g, &axis(2), &probe(2), t, skip))
}

/// A rotation blend's output at a frame.
pub(crate) fn blend_vec_at(g: &Graph, blend: NodeId, t: f64, skip: &Skip) -> DVec3 {
    let a = input_vec_at(g, blend, IN_A, t, skip);
    if skip.contains(&blend) {
        return a;
    }
    let b = input_vec_at(g, blend, IN_B, t, skip);
    let (wa, wb) = weights_at(g, blend, t);
    blend_rotation(a, b, wa, wb, accumulation(g, blend), rotate_order(g, blend))
}

/// What a blend outputs for this plug at a frame.
pub(crate) fn blend_at(g: &Graph, blend: NodeId, p: &PlugRef, t: f64, skip: &Skip) -> f64 {
    if is_rotation(g, blend) {
        return blend_vec_at(g, blend, t, skip)[p.component.max(0) as usize];
    }
    let a = input_at(g, &input(g, blend, IN_A, p), p, t, skip);
    if skip.contains(&blend) {
        return a;
    }
    let b = input_at(g, &input(g, blend, IN_B, p), p, t, skip);
    let (wa, wb) = weights_at(g, blend, t);
    blend_scalar(blend_kind(g, blend), a, b, wa, wb, accumulation(g, blend))
}

// ---------------------------------------------------------------------------------------------------------------------
// Solving for the value a layer has to hold

fn root_pow(v: f64, w: f64) -> f64 {
    v.abs().powf(1.0 / w) * v.signum()
}

/// The value a blend's input A needs for its output to be `want`, given input B.
fn solve_a(kind: BlendKind, want: f64, b: f64, wa: f64, wb: f64, acc: i64) -> f64 {
    match kind {
        BlendKind::Switch => want,
        BlendKind::Scale if acc != 0 => {
            let div = if wb == 0.0 { 1.0 } else { b.abs().powf(wb) * b.signum() };
            let v = if div.abs() > 1e-12 { want / div } else { want };
            if wa > 1e-9 {
                root_pow(v, wa)
            } else {
                want
            }
        }
        _ if wa.abs() > 1e-9 => (want - b * wb) / wa,
        _ => want,
    }
}

/// The value a blend's input B needs for its output to be `want`, given input A. A layer at no weight takes the value it
/// would need at full weight.
fn solve_b(kind: BlendKind, want: f64, a: f64, wa: f64, wb: f64, acc: i64) -> f64 {
    let wb = if wb.abs() > 1e-9 { wb } else { 1.0 };
    match kind {
        BlendKind::Switch => want,
        BlendKind::Scale if acc != 0 => {
            let base = if wa == 0.0 { 1.0 } else { a.abs().powf(wa) * a.signum() };
            let v = if base.abs() > 1e-12 { want / base } else { want };
            root_pow(v, wb)
        }
        _ => (want - a * wa) / wb,
    }
}

/// The rotation b must hold for the blend to give `want`.
fn solve_b_vec(want: DVec3, a: DVec3, wa: f64, wb: f64, acc: i64, order: RotateOrder, near: DVec3) -> DVec3 {
    let w = if wb.abs() > 1e-9 { wb } else { 1.0 };
    if acc == 0 {
        return (want - a * wa) / w;
    }
    let (qw, qa) = (euler_quat(want, order), euler_quat(a, order));
    let step = quat_pow(qa.inverse() * qw, 1.0 / w);
    let q: DQuat = if wa >= 1.0 - 1e-12 {
        step
    } else if wa <= 1e-12 {
        quat_pow(qw, 1.0 / w)
    } else {
        qa * step
    };
    euler_near(&DMat3::from_quat(q.normalize()), order, near)
}

/// The rotation a must hold for the blend to give `want`.
fn solve_a_vec(want: DVec3, b: DVec3, wa: f64, wb: f64, acc: i64, order: RotateOrder) -> DVec3 {
    if acc == 0 {
        return if wa.abs() > 1e-9 { (want - b * wb) / wa } else { want };
    }
    let (qw, qb) = (euler_quat(want, order), euler_quat(b, order));
    let q = if wa >= 1.0 - 1e-12 {
        qw * quat_pow(qb, wb).inverse()
    } else if wa <= 1e-12 {
        qw
    } else {
        // slerp(a, b, w) = a * (a^-1 b)^w has no closed form in a: a few steps of fixed point do.
        let mut qa = qw;
        for _ in 0..16 {
            qa = (qw * quat_pow(qa.inverse() * qb, wb).inverse()).normalize();
        }
        qa
    };
    euler_near(&DMat3::from_quat(q.normalize()), order, want)
}

/// The value a layer needs at frame t for the channel to come out at `want`, the layers above it staying as they are.
/// `from` starts the walk at that blend rather than the channel; blends in `skip` are treated as gone.
pub(crate) fn resolve(g: &Graph, p: &PlugRef, layer: Option<NodeId>, want: f64, t: f64, skip: &Skip, from: Option<NodeId>) -> f64 {
    let base = layer.is_none() || layer == base_layer(g);
    let mut d = want;
    let c = chain(g, p);
    let start = from.and_then(|f| c.iter().position(|&b| b == f)).unwrap_or(0);
    for &b in &c[start..] {
        if skip.contains(&b) {
            continue;
        }
        let (wa, wb) = weights_at(g, b, t);
        let (kind, acc) = (blend_kind(g, b), accumulation(g, b));
        if !base && blend_layer(g, b) == layer {
            let a = input_at(g, &input(g, b, IN_A, p), p, t, skip);
            return solve_b(kind, d, a, wa, wb, acc);
        }
        let vb = input_at(g, &input(g, b, IN_B, p), p, t, skip);
        d = solve_a(kind, d, vb, wa, wb, acc);
    }
    d
}

/// resolve for a whole rotate.
pub(crate) fn resolve_vec(g: &Graph, p: &PlugRef, layer: Option<NodeId>, want: DVec3, t: f64, skip: &Skip, from: Option<NodeId>) -> DVec3 {
    let base = layer.is_none() || layer == base_layer(g);
    let mut d = want;
    let c = chain(g, p);
    let start = from.and_then(|f| c.iter().position(|&b| b == f)).unwrap_or(0);
    for &b in &c[start..] {
        if skip.contains(&b) {
            continue;
        }
        let (wa, wb) = weights_at(g, b, t);
        let (acc, order) = (accumulation(g, b), rotate_order(g, b));
        if !base && blend_layer(g, b) == layer {
            let a = input_vec_at(g, b, IN_A, t, skip);
            let near = input_vec_at(g, b, IN_B, t, skip);
            return solve_b_vec(d, a, wa, wb, acc, order, near);
        }
        let vb = input_vec_at(g, b, IN_B, t, skip);
        d = solve_a_vec(d, vb, wa, wb, acc, order);
    }
    d
}

/// The value that nullifies a layer on a plug at frame t (`setKeyframe -identity`, the Zero Key). None on the base.
pub(crate) fn identity_value(g: &Graph, p: &PlugRef, layer: Option<NodeId>, t: f64) -> Option<f64> {
    let c = chain(g, p);
    let l = layer.filter(|&l| c.iter().any(|&b| blend_layer(g, b) == Some(l))).or_else(|| key_target(g, p))?;
    let &b = c.iter().find(|&&b| blend_layer(g, b) == Some(l))?;
    let kind = blend_kind(g, b);
    if kind == BlendKind::Switch || layer_flag(g, l, L_OVERRIDE) {
        return Some(input_at(g, &input(g, b, IN_A, p), p, t, &Skip::new()));
    }
    Some(if kind == BlendKind::Scale && accumulation(g, b) != 0 { 1.0 } else { 0.0 })
}

/// The name a curve in a layer takes: `pCube1_translateX_AnimLayer1_inputB`, `pCube1_rotate_AnimLayer1_inputBX`.
pub(crate) fn curve_name(g: &Graph, p: &PlugRef, layer: Option<NodeId>) -> String {
    let node = g.node(p.node).map(|n| n.name.clone()).unwrap_or_default();
    let on_base = layer.is_none() || layer == base_layer(g);
    if on_base {
        return format!("{node}_{}", anim::channel_name(g, p));
    }
    let lname = layer.and_then(|l| g.node(l)).map(|n| n.name.clone()).unwrap_or_default();
    let a = g.attr(p);
    if a.ty == PlugType::Vec3 && a.name == "rotate" {
        format!("{node}_rotate_{lname}_inputB{}", ["X", "Y", "Z"][p.component.max(0) as usize])
    } else {
        format!("{node}_{}_{lname}_inputB", anim::channel_name(g, p))
    }
}

/// The curve in a layer's slot for a plug, made if need be.
pub(crate) fn curve_for(g: &mut Graph, p: &PlugRef, layer: Option<NodeId>) -> CmdResult<Option<NodeId>> {
    let Some(sl) = slot(g, p, layer) else { return Ok(None) };
    if let Some(c) = curve_in(g, &sl) {
        return Ok(Some(c));
    }
    let name = curve_name(g, p, layer);
    let ty = anim::curve_type_for(g, p);
    Ok(Some(anim::new_curve(g, ty, &name, &sl)?))
}

/// What each axis of a layered rotate has to hold on its layer for the rotate to come out at `want` (the rotate as it stands,
/// held value included, when None). Solved before the first axis is keyed, since keying one moves the others. None when the
/// plug isn't a layered rotate or has nowhere to key.
pub fn solve_rotate_key(g: &mut Graph, p: &PlugRef, t: f64, layer: Option<NodeId>, want: Option<DVec3>) -> Option<DVec3> {
    let c = chain(g, p);
    if !c.first().is_some_and(|&b| is_rotation(g, b)) {
        return None;
    }
    let in_chain = |g: &Graph, l: NodeId| Some(l) == base_layer(g) || c.iter().any(|&b| blend_layer(g, b) == Some(l));
    let named = layer.filter(|&l| in_chain(g, l));
    if named.is_some_and(|l| !can_key(g, l)) {
        return None;
    }
    let target = named.or_else(|| key_target(g, p))?;
    let skip = Skip::new();
    let now = (anim::frame(g) - t).abs() < 1e-6;
    let want = match want {
        Some(w) => w,
        None => match g.value(&p.whole()) {
            Ok(PlugValue::Vec3(x)) if now => x,
            _ => blend_vec_at(g, c[0], t, &skip),
        },
    };
    Some(resolve_vec(g, p, Some(target), want, t, &skip, None))
}

/// The curve to key for a plug in layers, made if need be, and the value to give it. None when there is nowhere to key.
pub fn layer_curve_for_key(g: &mut Graph, p: &PlugRef, t: f64, layer: Option<NodeId>, value: f64, raw: bool) -> CmdResult<Option<(NodeId, f64)>> {
    let in_chain = |g: &Graph, l: NodeId| Some(l) == base_layer(g) || chain(g, p).iter().any(|&b| blend_layer(g, b) == Some(l));
    // A layer the channel isn't in is ignored, as the flag's page says.
    let named = layer.filter(|&l| in_chain(g, l));
    if named.is_some_and(|l| !can_key(g, l)) {
        return Ok(None);
    }
    let Some(target) = named.or_else(|| key_target(g, p)) else { return Ok(None) };
    let skip = Skip::new();
    let v = if raw {
        value
    } else if chain(g, p).first().is_some_and(|&b| is_rotation(g, b)) {
        let top = chain(g, p)[0];
        let now = (anim::frame(g) - t).abs() < 1e-6;
        // At the current frame a held rotate is what is being keyed, all three axes of it.
        let mut want = match g.value(&p.whole()) {
            Ok(PlugValue::Vec3(x)) if now => x,
            _ => blend_vec_at(g, top, t, &skip),
        };
        let c = p.component.max(0) as usize;
        want[c] = value;
        resolve_vec(g, p, Some(target), want, t, &skip, None)[c]
    } else {
        resolve(g, p, Some(target), value, t, &skip, None)
    };
    Ok(curve_for(g, p, Some(target))?.map(|c| (c, v)))
}

// ---------------------------------------------------------------------------------------------------------------------
// Editing the chains

fn blend_type_for(g: &Graph, p: &PlugRef) -> &'static str {
    let a = g.attr(p);
    match (a.ty, a.name.as_str()) {
        (PlugType::Vec3, "rotate") => "animBlendNodeAdditiveRotation",
        (PlugType::Vec3, "scale") => "animBlendNodeAdditiveScale",
        (PlugType::Vec3, "translate") => "animBlendNodeAdditiveDL",
        (PlugType::Vec3, "rotateAxis" | "jointOrient") => "animBlendNodeAdditiveDA",
        (PlugType::Bool, _) => "animBlendNodeBoolean",
        (PlugType::Enum, _) => "animBlendNodeEnum",
        (PlugType::Int, _) => "animBlendNodeAdditiveI32",
        _ => "animBlendNodeAdditive",
    }
}

fn is_rotate_plug(g: &Graph, p: &PlugRef) -> bool {
    g.attr(p).ty == PlugType::Vec3 && g.attr(p).name == "rotate"
}

/// Writes a number into a plug whatever its type.
pub(crate) fn set_num(g: &mut Graph, p: PlugRef, v: f64) -> CmdResult<()> {
    let value = if p.component >= 0 {
        PlugValue::Float(v)
    } else {
        let mut x = g.attr(&p).default_value.clone();
        x.assign_scalar(v);
        x
    };
    g.set_value(p, value)?;
    Ok(())
}

/// Moves what is on one input to another: its connection, or the value it holds.
fn move_input(g: &mut Graph, from: &PlugRef, to: &PlugRef) -> CmdResult<()> {
    match g.source(from) {
        Some(s) => {
            g.disconnect(from);
            g.connect(&s, to, false)?;
        }
        None => {
            let v = g.value(from)?.scalar().unwrap_or(0.0);
            set_num(g, *to, v)?;
        }
    }
    Ok(())
}

/// What an unkeyed layer holds so that it changes nothing: its identity for an additive layer, what is under it for an
/// override layer or a switch.
pub(crate) fn rest_input(g: &mut Graph, blend: NodeId, layer: NodeId) -> CmdResult<()> {
    let kind = blend_kind(g, blend);
    let hold = kind == BlendKind::Switch || layer_flag(g, layer, L_OVERRIDE);
    let axes: Vec<i32> = if kind == BlendKind::Rotation { vec![0, 1, 2] } else { vec![-1] };
    for c in axes {
        let plug = |w: usize| if c >= 0 { PlugRef::component(blend, w as i32, c) } else { PlugRef::new(blend, w as i32) };
        let b = plug(IN_B);
        if g.source(&b).is_some() {
            continue;
        }
        let v = if hold {
            g.value(&plug(IN_A))?.scalar().unwrap_or(0.0)
        } else if kind == BlendKind::Scale && layer_flag(g, layer, L_SAM) {
            1.0
        } else {
            0.0
        };
        set_num(g, b, v)?;
    }
    Ok(())
}

/// Adds a channel to a layer. A rotate goes in whole, as the reference's rotation blends do. Does nothing if already there.
pub fn add_plug(g: &mut Graph, layer: NodeId, p: &PlugRef) -> CmdResult<()> {
    // Everything is on the base already.
    if Some(layer) == base_layer(g) {
        return Ok(());
    }
    let rotate = is_rotate_plug(g, p);
    if !rotate && p.component < 0 && g.attr(p).ty == PlugType::Vec3 {
        for c in 0..3 {
            add_plug(g, layer, &PlugRef::component(p.node, p.attr, c))?;
        }
        return Ok(());
    }
    let probe = if rotate { PlugRef::component(p.node, p.attr, 0) } else { *p };
    let c = chain(g, &probe);
    if c.iter().any(|&b| blend_layer(g, b) == Some(layer)) {
        return Ok(());
    }
    let order = ordered_layers(g);
    let rank = |l: Option<NodeId>| l.and_then(|l| order.iter().position(|x| *x == l)).unwrap_or(0);
    let mine = rank(Some(layer));
    // The chain runs top first: the new blend goes above those of lower layers.
    let above = c.iter().copied().filter(|&b| rank(blend_layer(g, b)) > mine).next_back();

    let node_name = g.node(p.node).map(|n| n.name.clone()).unwrap_or_default();
    let layer_name = g.node(layer).map(|n| n.name.clone()).unwrap_or_default();
    let channel = if rotate { g.attr(p).name.clone() } else { anim::channel_name(g, p) };
    let ty = types::find(blend_type_for(g, p)).unwrap();
    let blend = g.create_node(&ty, &format!("{node_name}_{channel}_{layer_name}"), NO_NODE, true)?;

    g.connect(&PlugRef::new(layer, L_FG as i32), &PlugRef::new(blend, WEIGHT_B as i32), false)?;
    g.connect(&PlugRef::new(layer, L_BG as i32), &PlugRef::new(blend, WEIGHT_A as i32), false)?;
    g.connect(&PlugRef::new(blend, BLEND_MSG as i32), &PlugRef::new(layer, L_BLENDS as i32), false)?;
    if rotate {
        g.connect(&PlugRef::new(layer, L_ORAM as i32), &PlugRef::new(blend, ACCUMULATE as i32), false)?;
        let ro = PlugRef::new(p.node, Transform::ROTATE_ORDER as i32);
        g.connect(&ro, &PlugRef::new(blend, BLEND_ROT_ORDER as i32), false)?;
    } else if blend_kind(g, blend) == BlendKind::Scale {
        g.connect(&PlugRef::new(layer, L_SAM as i32), &PlugRef::new(blend, ACCUMULATE as i32), false)?;
    }

    if rotate {
        // A rotate driven whole gives that connection to inputA and takes the axes from then on.
        let whole = p.whole();
        if above.is_none() {
            if let Some(s) = g.source(&whole) {
                g.disconnect(&whole);
                g.connect(&s, &PlugRef::new(blend, IN_A as i32), false)?;
            }
        }
        for ax in 0..3 {
            let target = PlugRef::component(p.node, p.attr, ax);
            let dst = match above {
                Some(up) => PlugRef::component(up, IN_A as i32, ax),
                None => curve_slot(g, &target),
            };
            if g.source(&PlugRef::new(blend, IN_A as i32)).is_none() {
                move_input(g, &dst, &PlugRef::component(blend, IN_A as i32, ax))?;
            }
            g.connect(&PlugRef::component(blend, BLEND_OUT as i32, ax), &dst, true)?;
        }
    } else {
        let dst = match above {
            Some(up) => input(g, up, IN_A, p),
            None => curve_slot(g, p),
        };
        move_input(g, &dst, &PlugRef::new(blend, IN_A as i32))?;
        g.connect(&PlugRef::new(blend, BLEND_OUT as i32), &dst, true)?;
    }
    g.evaluate();
    rest_input(g, blend, layer)?;
    Ok(())
}

/// The plugs a blend drives, in the end.
pub(crate) fn blend_plugs(g: &Graph, blend: NodeId) -> Vec<PlugRef> {
    let mut out: Vec<PlugRef> = Vec::new();
    let inputs: Vec<PlugRef> = if is_rotation(g, blend) { (0..3).map(|c| PlugRef::component(blend, IN_A as i32, c)).collect() } else { vec![PlugRef::new(blend, IN_A as i32)] };
    for i in inputs {
        for p in final_plugs(g, i) {
            if !out.contains(&p) {
                out.push(p);
            }
        }
    }
    out
}

pub(crate) fn layer_blends(g: &Graph, layer: NodeId) -> Vec<NodeId> {
    g.destinations(&PlugRef::new(layer, L_FG as i32)).into_iter().filter(|d| is_blend(g, d.node) && d.attr == WEIGHT_B as i32).map(|d| d.node).collect()
}

/// A blend's input B plugs, one per axis.
pub(crate) fn inputs_b(g: &Graph, blend: NodeId) -> Vec<PlugRef> {
    if is_rotation(g, blend) {
        (0..3).map(|c| PlugRef::component(blend, IN_B as i32, c)).collect()
    } else {
        vec![PlugRef::new(blend, IN_B as i32)]
    }
}

/// Takes one blend out of its chain and deletes it, with the layer's curves on it.
pub(crate) fn remove_blend(g: &mut Graph, blend: NodeId) -> CmdResult<()> {
    let curves: Vec<NodeId> = inputs_b(g, blend).iter().filter_map(|b| curve_in(g, b)).collect();
    if is_rotation(g, blend) {
        let whole_in = PlugRef::new(blend, IN_A as i32);
        let whole_src = g.source(&whole_in);
        for c in 0..3 {
            let inp = PlugRef::component(blend, IN_A as i32, c);
            for d in g.destinations(&PlugRef::component(blend, BLEND_OUT as i32, c)) {
                g.disconnect(&d);
                match (whole_src, g.source(&inp)) {
                    (Some(s), _) => {
                        let target = d.whole();
                        g.connect(&s, &target, true)?;
                    }
                    (None, Some(s)) => {
                        g.connect(&s, &d, false)?;
                    }
                    (None, None) => {
                        let v = g.value(&inp)?.scalar().unwrap_or(0.0);
                        set_num(g, d, v)?;
                    }
                }
            }
        }
    } else {
        for d in g.destinations(&PlugRef::new(blend, BLEND_OUT as i32)) {
            g.disconnect(&d);
            let inp = PlugRef::new(blend, IN_A as i32);
            match g.source(&inp) {
                Some(s) => {
                    g.connect(&s, &d, false)?;
                }
                None => {
                    let v = g.value(&inp)?.scalar().unwrap_or(0.0);
                    set_num(g, d, v)?;
                }
            }
        }
    }
    g.delete_node(blend)?;
    for c in curves {
        g.delete_node(c)?;
    }
    Ok(())
}

/// Takes a layer off its parent's lists.
fn unwire_parent(g: &mut Graph, layer: NodeId) -> CmdResult<()> {
    let Some(parent) = layer_parent(g, layer) else { return Ok(()) };
    for e in g.elements(&PlugRef::new(parent, L_CHILDREN as i32)) {
        if g.source(&e).is_some_and(|s| s.node == layer) {
            g.remove_element(&e)?;
        }
    }
    for e in g.elements(&PlugRef::new(parent, L_CHILDREN_SOLO as i32)) {
        if g.source(&e).is_some_and(|s| s.node == layer) {
            g.remove_element(&e)?;
        }
    }
    for at in [L_PARENT_WEIGHT, L_PARENT_MUTE, L_SIBLING_SOLO] {
        g.disconnect(&PlugRef::new(layer, at as i32));
    }
    set_num(g, PlugRef::new(layer, L_PARENT_WEIGHT as i32), 1.0)?;
    set_num(g, PlugRef::new(layer, L_PARENT_MUTE as i32), 0.0)?;
    set_num(g, PlugRef::new(layer, L_SIBLING_SOLO as i32), 0.0)?;
    Ok(())
}

/// Puts a layer on top of a parent's children, with the connections the reference's files show.
fn wire_parent(g: &mut Graph, layer: NodeId, parent: NodeId) -> CmdResult<()> {
    g.connect(&PlugRef::new(layer, L_PARENT_LAYER as i32), &PlugRef::new(parent, L_CHILDREN as i32), false)?;
    g.connect(&PlugRef::new(layer, L_SOLO as i32), &PlugRef::new(parent, L_CHILDREN_SOLO as i32), false)?;
    g.connect(&PlugRef::new(parent, L_CHILD_SOLOED as i32), &PlugRef::new(layer, L_SIBLING_SOLO as i32), false)?;
    g.connect(&PlugRef::new(parent, L_FG as i32), &PlugRef::new(layer, L_PARENT_WEIGHT as i32), false)?;
    g.connect(&PlugRef::new(parent, L_OUT_MUTE as i32), &PlugRef::new(layer, L_PARENT_MUTE as i32), false)?;
    Ok(())
}

pub(crate) fn set_parent(g: &mut Graph, layer: NodeId, parent: NodeId) -> CmdResult<()> {
    let mut up = Some(parent);
    while let Some(x) = up {
        if x == layer {
            fail!("animLayer: a layer can't go under itself");
        }
        up = layer_parent(g, x);
    }
    unwire_parent(g, layer)?;
    wire_parent(g, layer, parent)?;
    rebuild_chains(g, layer)
}

/// A layer's channels and its child layers stay with the stack; when no layers are left the base goes too.
pub fn before_delete(g: &mut Graph, id: NodeId) -> CmdResult<()> {
    if !is_layer(g, id) {
        return Ok(());
    }
    if Some(id) == base_layer(g) {
        if !layer_children(g, id).is_empty() {
            fail!("delete: {BASE_LAYER} goes when the last layer does");
        }
        return Ok(());
    }
    for b in layer_blends(g, id) {
        remove_blend(g, b)?;
    }
    let parent = layer_parent(g, id);
    for c in layer_children(g, id) {
        unwire_parent(g, c)?;
        if let Some(p) = parent {
            wire_parent(g, c, p)?;
        }
    }
    unwire_parent(g, id)
}

/// After deleting layers: the base goes with the last of them.
pub fn after_delete(g: &mut Graph) -> CmdResult<()> {
    if let Some(b) = base_layer(g) {
        if layer_children(g, b).is_empty() {
            g.delete_node(b)?;
        }
    }
    Ok(())
}

pub(crate) fn ensure_base(g: &mut Graph) -> CmdResult<NodeId> {
    if let Some(b) = base_layer(g) {
        return Ok(b);
    }
    let t = types::find("animLayer").unwrap();
    let b = g.create_node(&t, BASE_LAYER, NO_NODE, true)?;
    // The reference's own base layer is saved with this on.
    g.set_value(PlugRef::new(b, L_OVERRIDE as i32), PlugValue::Bool(true))?;
    Ok(b)
}

/// Takes a layer's blends out and puts them back where the stack now says, curves kept. Its children follow.
fn rebuild_chains(g: &mut Graph, layer: NodeId) -> CmdResult<()> {
    for b in layer_blends(g, layer) {
        let Some(p) = blend_plugs(g, b).first().copied() else { continue };
        let inputs = inputs_b(g, b);
        let saved: Vec<(Option<PlugRef>, f64)> = inputs.iter().map(|i| (g.source(i), g.value(i).ok().and_then(|x| x.scalar()).unwrap_or(0.0))).collect();
        for (i, (src, _)) in inputs.iter().zip(&saved) {
            if src.is_some() {
                g.disconnect(i);
            }
        }
        let rotate = is_rotation(g, b);
        let plug = if rotate { p.whole() } else { p };
        remove_blend(g, b)?;
        add_plug(g, layer, &plug)?;
        let probe = if rotate { PlugRef::component(plug.node, plug.attr, 0) } else { plug };
        let Some(&new_b) = chain(g, &probe).iter().find(|&&x| blend_layer(g, x) == Some(layer)) else { continue };
        for (i, (src, v)) in inputs_b(g, new_b).into_iter().zip(saved) {
            match src {
                Some(s) => {
                    g.connect(&s, &i, true)?;
                }
                None => set_num(g, i, v)?,
            }
        }
    }
    for c in layer_children(g, layer) {
        rebuild_chains(g, c)?;
    }
    Ok(())
}

/// Puts a layer next to another in the stack, and rebuilds its channels' chains in that order.
pub(crate) fn move_layer(g: &mut Graph, layer: NodeId, other: NodeId, after: bool) -> CmdResult<()> {
    let Some(parent) = layer_parent(g, other) else { fail!("animLayer: can't move next to {BASE_LAYER}") };
    let mut up = Some(parent);
    while let Some(x) = up {
        if x == layer {
            fail!("animLayer: a layer can't go under itself");
        }
        up = layer_parent(g, x);
    }
    unwire_parent(g, layer)?;
    let mut sibs = layer_children(g, parent);
    let at = sibs.iter().position(|&x| x == other).unwrap_or(0) + usize::from(after);
    sibs.insert(at, layer);
    for &s in &sibs {
        if s != layer {
            unwire_parent(g, s)?;
        }
    }
    for &s in &sibs {
        wire_parent(g, s, parent)?;
    }
    rebuild_chains(g, layer)
}

/// Before a bake's keys go on: where they go, and what happens to the layers they replace. By the bakeResults page, a layer
/// with baked channels is muted unless -ral takes the channels out or -rba the animation; -rwl limits which layers give way.
pub(crate) fn prepare_bake(g: &mut Graph, plugs: &[PlugRef], a: &aztek_ops::Args) -> CmdResult<Option<NodeId>> {
    let dest = match a.get("destinationLayer").and_then(serde_json::Value::as_str) {
        Some(n) => match g.find(n)?.filter(|&x| is_layer(g, x)) {
            Some(l) => Some(l),
            None => fail!("bakeResults: no layer named {n}"),
        },
        None if a.bool("bakeOnOverrideLayer") => {
            let base = ensure_base(g)?;
            let t = types::find("animLayer").unwrap();
            let l = g.create_node(&t, "BakeResults", NO_NODE, true)?;
            set_parent(g, l, base)?;
            g.set_value(PlugRef::new(l, L_OVERRIDE as i32), PlugValue::Bool(true))?;
            Some(l)
        }
        None => None,
    };
    let only: Option<Vec<NodeId>> = a.get("resolveWithoutLayer").map(|v| {
        let names: Vec<String> = match v {
            serde_json::Value::String(s) => vec![s.clone()],
            serde_json::Value::Array(x) => x.iter().filter_map(|y| y.as_str().map(str::to_string)).collect(),
            _ => Vec::new(),
        };
        names.iter().filter_map(|n| g.find(n).ok().flatten()).collect()
    });
    let top_override = a.bool("bakeOnOverrideLayer");
    let mut layers: Vec<NodeId> = Vec::new();
    for p in plugs {
        for &b in &chain(g, p) {
            if let Some(l) = blend_layer(g, b) {
                let goes = Some(l) != dest && only.as_ref().is_none_or(|o| o.contains(&l)) && !top_override;
                if goes && !layers.contains(&l) {
                    layers.push(l);
                }
            }
        }
    }
    let remove_attr = a.bool("removeBakedAttributeFromLayer");
    let remove_anim = a.bool("removeBakedAnimFromLayer");
    for &l in &layers {
        if remove_attr {
            for p in plugs {
                if let Some(&b) = chain(g, p).iter().find(|&&b| blend_layer(g, b) == Some(l)) {
                    remove_blend(g, b)?;
                }
            }
        } else if remove_anim {
            for p in plugs {
                if let Some(c) = slot(g, p, Some(l)).filter(|_| chain(g, p).iter().any(|&b| blend_layer(g, b) == Some(l))).and_then(|sl| curve_in(g, &sl)) {
                    g.delete_node(c)?;
                    if let Some(&b) = chain(g, p).iter().find(|&&b| blend_layer(g, b) == Some(l)) {
                        rest_input(g, b, l)?;
                    }
                }
            }
        } else {
            g.set_value(PlugRef::new(l, L_MUTE as i32), PlugValue::Bool(true))?;
        }
    }
    Ok(dest)
}

/// A bake's samples as keys on a layer's curve for this plug, each solved so the channel comes out at the sample.
pub(crate) fn bake_onto(g: &mut Graph, p: &PlugRef, target: Option<NodeId>, sampled: &[(f64, f64)], keep: bool, range: (f64, f64)) -> CmdResult<()> {
    let skip = Skip::new();
    let solved: Vec<(f64, f64)> = sampled.iter().map(|&(t, v)| (t, resolve(g, p, target, v, t, &skip, None))).collect();
    let Some(c) = curve_for(g, p, target)? else { return Ok(()) };
    let mut k = anim::keys_of(g, c);
    if keep {
        k.keys.retain(|x| x.t < range.0 - 1e-6 || x.t > range.1 + 1e-6);
    } else {
        k.keys.clear();
    }
    let r = anim::range();
    for (t, v) in solved {
        k.keys.push(anim::Key::new(t, v, r.default_in, r.default_out));
    }
    k.sort();
    anim::set_keys(g, c, k)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    crate::anim_layer_cmds::register(c);
}
