//! The nodes of animation layers, with the attribute names the reference's files use: `animLayer` and the `animBlendNode*` family.
//! Layout read from the node reference and from a layered scene saved by the reference: a layer's `fgwt` and `bgwt` drive a blend's `wb`
//! and `wa`, its `oram` or `sam` the blend's `acm`, the blend's `msg` goes to the layer's `bnds`, and a child's `play`,
//! `sl` go to its parent's `cdly`, `chsl` while the parent's `csol`, `fgwt`, `omte` come back as `sslo`, `pwth`, `pmte`.

use std::any::Any;
use std::sync::Arc;

use glam::{DMat3, DQuat, DVec3};

use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};
use crate::transform::{euler_near, euler_to_matrix, RotateOrder};

// Blend node attrs.
pub const IN_A: usize = 0;
pub const IN_B: usize = 1;
pub const WEIGHT_A: usize = 2;
pub const WEIGHT_B: usize = 3;
pub const BLEND_OUT: usize = 4;
pub const ACCUMULATE: usize = 5;
pub const BLEND_ROT_ORDER: usize = 6;
pub const BLEND_MSG: usize = 7;
pub const BLEND_DEST: usize = 8;

// Layer attrs.
pub const L_MUTE: usize = 0;
pub const L_LOCK: usize = 1;
pub const L_SOLO: usize = 2;
pub const L_OVERRIDE: usize = 3;
pub const L_WEIGHT: usize = 4;
pub const L_FG: usize = 5;
pub const L_BG: usize = 6;
pub const L_PARENT_WEIGHT: usize = 7;
pub const L_RAM: usize = 8;
pub const L_SAM: usize = 9;
pub const L_SELECTED: usize = 10;
pub const L_PREFERRED: usize = 11;
pub const L_PASSTHROUGH: usize = 12;
pub const L_ORAM: usize = 13;
pub const L_OUT_MUTE: usize = 14;
pub const L_PARENT_MUTE: usize = 15;
pub const L_SIBLING_SOLO: usize = 16;
pub const L_CHILD_SOLOED: usize = 17;
pub const L_CHILDREN_SOLO: usize = 18;
pub const L_CHILDREN: usize = 19;
pub const L_PARENT_LAYER: usize = 20;
pub const L_BLENDS: usize = 21;
pub const L_GHOST: usize = 22;
pub const L_GHOST_COLOR: usize = 23;
pub const L_COLLAPSE: usize = 24;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum BlendKind {
    Add,
    Scale,
    Switch,
    Rotation,
}

struct BlendNode {
    kind: BlendKind,
}

/// power(x, w), keeping the sign of x.
fn signed_pow(x: f64, w: f64) -> f64 {
    if w == 0.0 {
        return 1.0;
    }
    x.abs().powf(w) * x.signum()
}

/// One blend's output from its inputs, by the formulas on the node pages. Boolean and enum: b unless the weight is 0.
pub fn blend_scalar(kind: BlendKind, a: f64, b: f64, wa: f64, wb: f64, accumulate: i64) -> f64 {
    match kind {
        BlendKind::Scale if accumulate != 0 => signed_pow(a, wa) * signed_pow(b, wb),
        BlendKind::Switch => {
            if wb > 0.0 {
                b
            } else {
                a
            }
        }
        _ => a * wa + b * wb,
    }
}

pub fn euler_quat(e: DVec3, order: RotateOrder) -> DQuat {
    DQuat::from_mat4(&euler_to_matrix(e, order)).normalize()
}

/// A rotation to a power, the short way round.
pub fn quat_pow(q: DQuat, e: f64) -> DQuat {
    let q = if q.w < 0.0 { -q } else { q };
    let (axis, angle) = q.to_axis_angle();
    if angle.abs() < 1e-12 {
        DQuat::IDENTITY
    } else {
        DQuat::from_axis_angle(axis, angle * e)
    }
}

/// A rotation blend. By component (0) a weighted sum of Euler values. By layer (1) concatenated quaternions when adding
/// (wa = 1), a slerp when overriding with passthrough (wa = 1 - wb), and the layer's own rotation when opaque (wa = 0).
pub fn blend_rotation(a: DVec3, b: DVec3, wa: f64, wb: f64, accumulate: i64, order: RotateOrder) -> DVec3 {
    let sum = a * wa + b * wb;
    if accumulate == 0 {
        return sum;
    }
    let (qa, qb) = (euler_quat(a, order), euler_quat(b, order));
    let q = if wa >= 1.0 - 1e-12 {
        qa * quat_pow(qb, wb)
    } else if wa <= 1e-12 {
        quat_pow(qb, wb)
    } else {
        qa.slerp(qb, wb)
    };
    euler_near(&DMat3::from_quat(q.normalize()), order, sum)
}

fn num(d: &NodeData, at: usize) -> f64 {
    d.value(at).scalar().unwrap_or(0.0)
}

impl NodeBehavior for BlendNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let (wa, wb) = (num(d, WEIGHT_A), num(d, WEIGHT_B));
        let acc = num(d, ACCUMULATE) as i64;
        if self.kind == BlendKind::Rotation {
            let order = RotateOrder::from_index(num(d, BLEND_ROT_ORDER) as i64);
            let out = blend_rotation(d.vec3(IN_A), d.vec3(IN_B), wa, wb, acc, order);
            d.set_output(BLEND_OUT, PlugValue::Vec3(out));
            return;
        }
        let v = blend_scalar(self.kind, num(d, IN_A), num(d, IN_B), wa, wb, acc);
        let mut out = d.value(BLEND_OUT).clone();
        out.assign_scalar(v);
        d.set_output(BLEND_OUT, out);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct LayerNode;

impl NodeBehavior for LayerNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let off = d.bool(L_MUTE) || d.bool(L_PARENT_MUTE) || (d.bool(L_SIBLING_SOLO) && !d.bool(L_SOLO));
        let fg = if off { 0.0 } else { d.float(L_WEIGHT) * d.float(L_PARENT_WEIGHT) };
        // Passthrough lets the layers below show through as the weight falls; without it an override layer is opaque.
        let bg = if off || !d.bool(L_OVERRIDE) {
            1.0
        } else if d.bool(L_PASSTHROUGH) {
            1.0 - fg
        } else {
            0.0
        };
        let soloed = d.elements.get(L_CHILDREN_SOLO).is_some_and(|e| e.iter().any(|(_, v)| v.scalar().unwrap_or(0.0) != 0.0));
        let ram = d.int(L_RAM);
        d.set_output(L_FG, PlugValue::Float(fg));
        d.set_output(L_BG, PlugValue::Float(bg));
        d.set_output(L_OUT_MUTE, PlugValue::Bool(off));
        d.set_output(L_CHILD_SOLOED, PlugValue::Bool(soloed));
        d.set_output(L_ORAM, PlugValue::Int(ram));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn attr(name: &str, short: &str, ty: PlugType, def: PlugValue, fl: u32, doc: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: def, flags: fl, doc: doc.into(), ..Default::default() }
}

fn float_in(name: &str, short: &str, def: f64, keyable: bool, doc: &str) -> AttrDef {
    let fl = flags::INPUT | flags::STORABLE | if keyable { flags::KEYABLE } else { 0 };
    attr(name, short, PlugType::Float, PlugValue::Float(def), fl, doc)
}

fn bool_in(name: &str, short: &str, def: bool, doc: &str) -> AttrDef {
    attr(name, short, PlugType::Bool, PlugValue::Bool(def), flags::INPUT | flags::STORABLE, doc)
}

fn enum_in(name: &str, short: &str, names: &[&str], def: i64, doc: &str) -> AttrDef {
    let mut a = attr(name, short, PlugType::Enum, PlugValue::Int(def), flags::INPUT | flags::STORABLE, doc);
    a.enum_names = names.iter().map(|s| s.to_string()).collect();
    a
}

fn float_out(name: &str, short: &str, def: f64, doc: &str) -> AttrDef {
    attr(name, short, PlugType::Float, PlugValue::Float(def), flags::OUTPUT, doc)
}

fn bool_out(name: &str, short: &str, doc: &str) -> AttrDef {
    attr(name, short, PlugType::Bool, PlugValue::Bool(false), flags::OUTPUT, doc)
}

/// A message plug: no data flows, and a plain input needs no evaluation order against its node.
fn message(name: &str, short: &str, array: bool, doc: &str) -> AttrDef {
    let fl = flags::INPUT | flags::HIDDEN | if array { flags::ARRAY } else { 0 };
    attr(name, short, PlugType::Int, PlugValue::Int(0), fl, doc)
}

fn blend_attrs(kind: BlendKind, ty: PlugType, zero: PlugValue, rest_b: PlugValue) -> Vec<AttrDef> {
    let store = flags::INPUT | flags::STORABLE;
    let (ia, ib, o) = if kind == BlendKind::Rotation {
        (
            vec3_attr("inputA", "ia", DVec3::ZERO, store, "Input A: what the layers below come to."),
            vec3_attr("inputB", "ib", DVec3::ZERO, store, "Input B: this layer's value."),
            vec3_attr("output", "o", DVec3::ZERO, flags::OUTPUT, "The result."),
        )
    } else {
        (
            attr("inputA", "ia", ty, zero.clone(), store, "Input A: what the layers below come to."),
            attr("inputB", "ib", ty, rest_b, store, "Input B: this layer's value."),
            attr("output", "o", ty, zero, flags::OUTPUT, "The result."),
        )
    };
    let modes: &[&str] = if kind == BlendKind::Rotation { &["component", "layer"] } else { &["additive", "multiply"] };
    vec![
        ia,
        ib,
        float_in("weightA", "wa", 1.0, false, "Weight of input A."),
        float_in("weightB", "wb", 1.0, false, "Weight of input B."),
        o,
        enum_in("accumulationMode", "acm", modes, 0, "Rotations by component or by layer; scales added or multiplied."),
        enum_in("rotateOrder", "ro", &["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"], 0, "The channel's rotate order."),
        message("message", "msg", false, "Goes to the layer's blendNodes."),
        attr("destinationPlug", "dp", PlugType::String, PlugValue::String(String::new()), store, "The plug this blend ends up driving, written by animLayer -wbd."),
    ]
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let blend = |name: &str, kind: BlendKind, attrs: Vec<AttrDef>, doc: &str| {
            types::register_type(NodeType {
                name: name.into(),
                icon: "chart-line".into(),
                doc: doc.into(),
                own_attrs: attrs,
                create: Some(Arc::new(move || Box::new(BlendNode { kind }))),
                ..Default::default()
            })
            .expect(name);
        };
        let doubles = |rest: f64| blend_attrs(BlendKind::Add, PlugType::Float, PlugValue::Float(0.0), PlugValue::Float(rest));
        for (name, doc) in [
            ("animBlendNodeAdditive", "output = a * wa + b * wb, doubles."),
            ("animBlendNodeAdditiveDL", "output = a * wa + b * wb, a linear channel."),
            ("animBlendNodeAdditiveDA", "output = a * wa + b * wb, an angle channel."),
            ("animBlendNodeAdditiveF", "output = a * wa + b * wb, floats."),
            ("animBlendNodeAdditiveFL", "output = a * wa + b * wb, a linear float channel."),
            ("animBlendNodeAdditiveFA", "output = a * wa + b * wb, an angle float channel."),
            ("animBlendNodeAdditiveI16", "output = a * wa + b * wb, short integers."),
            ("animBlendNodeAdditiveI32", "output = a * wa + b * wb, integers."),
            ("animBlendNodeTime", "output = a * wa + b * wb, a time channel."),
        ] {
            blend(name, BlendKind::Add, doubles(0.0), doc);
        }
        let scale = blend_attrs(BlendKind::Scale, PlugType::Float, PlugValue::Float(0.0), PlugValue::Float(1.0));
        blend("animBlendNodeAdditiveScale", BlendKind::Scale, scale, "output = power(a, wa) * power(b, wb) when multiplying, a * wa + b * wb when adding.");
        let rotation = blend_attrs(BlendKind::Rotation, PlugType::Vec3, PlugValue::Vec3(DVec3::ZERO), PlugValue::Vec3(DVec3::ZERO));
        blend("animBlendNodeAdditiveRotation", BlendKind::Rotation, rotation, "Blends a whole rotate, by component or by layer.");
        let boolean = blend_attrs(BlendKind::Switch, PlugType::Bool, PlugValue::Bool(false), PlugValue::Bool(false));
        blend("animBlendNodeBoolean", BlendKind::Switch, boolean, "output = b if the weight is above 0, else a.");
        let enums = blend_attrs(BlendKind::Switch, PlugType::Int, PlugValue::Int(0), PlugValue::Int(0));
        blend("animBlendNodeEnum", BlendKind::Switch, enums, "output = b if the weight is above 0, else a.");

        types::register_type(NodeType {
            name: "animLayer".into(),
            icon: "chart-line".into(),
            doc: "One animation layer: its curves add to the layers below, or override them.".into(),
            own_attrs: vec![
                bool_in("mute", "mt", false, "The layer is left out of the result."),
                bool_in("lock", "lo", false, "The layer can't receive keys; its keys still play."),
                bool_in("solo", "sl", false, "Mutes this layer's siblings."),
                bool_in("override", "ovrd", false, "Replaces what is below instead of adding to it."),
                float_in("weight", "wgth", 1.0, true, "How much of the layer plays, 0 to 1."),
                float_out("foregroundWeight", "fgwt", 1.0, "Weight of the layer's blends: its own under its parent's, 0 when off."),
                float_out("backgroundWeight", "bgwt", 1.0, "Weight of what is below it."),
                float_in("parentWeight", "pwth", 1.0, false, "The parent layer's foreground weight."),
                enum_in("rotationAccumulationMode", "ram", &["component", "layer"], 0, "Rotations by component or by layer."),
                enum_in("scaleAccumulationMode", "sam", &["additive", "multiply"], 1, "Scales added or multiplied."),
                bool_in("selected", "slct", false, "A selected layer shows in the Time Slider and Graph Editor."),
                bool_in("preferred", "pref", false, "The best layer algorithm tries this layer first."),
                bool_in("passthrough", "pthg", true, "An override layer's weight lets the layers below show through."),
                attr("outRotationAccumulationMode", "oram", PlugType::Enum, PlugValue::Int(0), flags::OUTPUT, "The rotation mode its blends read."),
                bool_out("outMute", "omte", "Off, for its children."),
                bool_in("parentMute", "pmte", false, "The parent layer is off."),
                bool_in("siblingSolo", "sslo", false, "A sibling or this layer is soloed."),
                bool_out("childsoloed", "csol", "One of the children is soloed."),
                attr("childrenSolo", "chsl", PlugType::Bool, PlugValue::Bool(false), flags::INPUT | flags::HIDDEN | flags::ARRAY, "Solo of each child."),
                message("childrenLayers", "cdly", true, "Each child's parentLayer, in stack order."),
                message("parentLayer", "play", false, "Goes to the parent's childrenLayers."),
                message("blendNodes", "bnds", true, "The layer's blend nodes."),
                bool_in("ghost", "gh", false, "Show ghosts for the layer."),
                attr("ghostColor", "ghc", PlugType::Int, PlugValue::Int(5), flags::INPUT | flags::STORABLE, "Colour index of the layer's ghosts and key ticks."),
                bool_in("collapse", "coll", false, "Collapsed in the layer editor."),
            ],
            create: Some(Arc::new(|| Box::new(LayerNode))),
            ..Default::default()
        })
        .expect("animLayer");
    });
}
