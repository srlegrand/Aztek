//! The nodes of HumanIK with the attribute names the reference's files use. A joint's `.ch` goes to a character's slot; `HIKSK2State` reads a
//! source's joints into a character state; `HIKRetargeterNode` turns it into the target's; `HIKSolverNode` passes it on or holds the
//! stance; `HIKState2SK` puts out each joint's channels. States and definitions travel as data, as they do in the reference.
//!
//! In the reference `HIKState2SK` also reads each joint's parent matrix (`.pm` to `.HipsPGX`). The graph here orders whole nodes, so that would be
//! a cycle; what those connections carry is kept in the definition's rig information, and `.ma` writes them for the reference.

use std::any::Any;
use std::sync::Arc;

use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::hik_slots::{slot_names, SLOT_COUNT};
use crate::hik_solve::{retarget, to_local, Definition, Options, RigSlot, State};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType, IN_STORE};
use crate::plug::{flags, AttrDef, Data, DataPtr, PlugType, PlugValue};
use crate::rig::{data_attr, json_of};
use crate::transform::{euler_to_matrix, RotateOrder};

pub const CHARACTER: &str = "HIKCharacterNode";

// HIKCharacterNode.
pub const C_OUT_DEF: usize = 0;
pub const C_LOCK: usize = 1;
pub const C_PROPERTY: usize = 2;
pub const C_RIG: usize = 3;
pub const C_SLOTS: usize = 4;
pub const C_PER_SLOT: usize = 7;
/// The message plug of a slot; T, R, S, JointOrient, MinRLimit and MaxRLimit follow it.
pub fn c_slot(i: usize) -> usize {
    C_SLOTS + i * C_PER_SLOT
}

// HIKSK2State, HIKFK2State.
pub const S_OUT: usize = 0;
pub const S_DEF: usize = 1;
pub const S_GX: usize = 2;

// HIKRetargeterNode.
pub const R_OUT: usize = 0;
pub const R_STATE: usize = 1;
pub const R_DEF_SRC: usize = 2;
pub const R_DEF_DST: usize = 3;
pub const R_PROP_SRC: usize = 4;
pub const R_PROP_DST: usize = 5;
pub const R_REFERENCE: usize = 6;

// HIKSolverNode.
pub const V_OUT: usize = 0;
pub const V_STATE: usize = 1;
pub const V_DEF: usize = 2;
pub const V_PROP: usize = 3;
pub const V_STANCE: usize = 4;

// HIKState2SK.
pub const K_DEF: usize = 0;
pub const K_STATE: usize = 1;
pub const K_ROOT: usize = 2;
pub const K_SAVED: usize = 3;
pub const K_SLOTS: usize = 4;
/// A slot's T output; R and S follow it.
pub fn k_slot(i: usize) -> usize {
    K_SLOTS + i * 3
}

// HIKProperty2State.
pub const P_OUT: usize = 0;
pub const P_MSG: usize = 1;
pub const P_FIRST: usize = 2;

macro_rules! payload {
    ($name:ident, $inner:ty, $kind:literal) => {
        #[derive(Debug)]
        pub struct $name(pub $inner);
        impl Data for $name {
            fn kind(&self) -> &str {
                $kind
            }
            fn to_json(&self) -> Value {
                json!({"kind": $kind})
            }
            fn as_any(&self) -> &dyn Any {
                self
            }
        }
    };
}

payload!(DefinitionData, Definition, "HIKCharacter");
payload!(StateData, State, "HIKCharacterState");
payload!(PropertyData, Options, "HIKPropertySetState");

fn data_of<T: 'static>(d: &NodeData, at: usize) -> Option<&T> {
    match d.value(at) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<T>(),
        _ => None,
    }
}

fn put(d: &mut NodeData, at: usize, x: DataPtr) {
    d.set_output(at, PlugValue::Data(Some(x)));
}

fn stance_matrix(t: DVec3, r: DVec3, s: DVec3) -> DMat4 {
    DMat4::from_translation(t) * euler_to_matrix(r, RotateOrder::Xyz) * DMat4::from_scale(s)
}

/// A rig slot from the JSON the definition keeps.
fn rig_from(v: &Value) -> Vec<RigSlot> {
    let vec3 = |x: &Value| DVec3::new(x[0].as_f64().unwrap_or(0.0), x[1].as_f64().unwrap_or(0.0), x[2].as_f64().unwrap_or(0.0));
    v.as_array()
        .map(|a| {
            a.iter()
                .map(|r| RigSlot {
                    slot: r["slot"].as_u64().unwrap_or(0) as usize,
                    parent: r["parent"].as_u64().map(|x| x as usize),
                    local_parent: r["up"].as_u64().map(|x| x as usize),
                    between: crate::rig::mat_from(&r["between"]),
                    joint_orient: vec3(&r["jo"]),
                    rotate_axis: vec3(&r["ra"]),
                    rotate_order: r["ro"].as_i64().unwrap_or(0),
                    local_scale: vec3(&r["s"]),
                })
                .collect()
        })
        .unwrap_or_default()
}

struct CharacterNode;

impl NodeBehavior for CharacterNode {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let mut def = Definition::empty();
        for i in 0..SLOT_COUNT {
            let at = c_slot(i);
            if d.value(at).scalar().unwrap_or(0.0) != 0.0 {
                def.stance[i] = Some(stance_matrix(d.vec3(at + 1), d.vec3(at + 2), d.vec3(at + 3)));
            }
        }
        def.rig = rig_from(&json_of(d, C_RIG));
        put(d, C_OUT_DEF, Arc::new(DefinitionData(def)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct SourceState;

impl NodeBehavior for SourceState {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let mut state: State = vec![None; SLOT_COUNT];
        if let Some(def) = data_of::<DefinitionData>(d, S_DEF) {
            for (i, slot) in state.iter_mut().enumerate() {
                if def.0.stance[i].is_some() {
                    *slot = Some(d.matrix(S_GX + i));
                }
            }
        }
        put(d, S_OUT, Arc::new(StateData(state)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct Retargeter;

impl NodeBehavior for Retargeter {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let out = match (data_of::<StateData>(d, R_STATE), data_of::<DefinitionData>(d, R_DEF_SRC), data_of::<DefinitionData>(d, R_DEF_DST)) {
            (Some(s), Some(a), Some(b)) => {
                let o = data_of::<PropertyData>(d, R_PROP_DST).map(|p| p.0.clone()).unwrap_or_default();
                retarget(&a.0, &s.0, &b.0, &o)
            }
            _ => vec![None; SLOT_COUNT],
        };
        put(d, R_OUT, Arc::new(StateData(out)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct Solver;

impl NodeBehavior for Solver {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let stance = d.bool(V_STANCE);
        let def = data_of::<DefinitionData>(d, V_DEF).map(|x| x.0.stance.clone());
        let out: State = match (stance, def, data_of::<StateData>(d, V_STATE)) {
            (true, Some(st), _) => st,
            (false, _, Some(s)) => s.0.clone(),
            (false, Some(st), None) => st,
            _ => vec![None; SLOT_COUNT],
        };
        put(d, V_OUT, Arc::new(StateData(out)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct StateToSkeleton;

impl NodeBehavior for StateToSkeleton {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let (Some(def), Some(state)) = (data_of::<DefinitionData>(d, K_DEF).map(|x| x.0.rig.clone()), data_of::<StateData>(d, K_STATE).map(|x| x.0.clone())) else { return };
        let root = d.matrix(K_ROOT);
        for r in &def {
            let Some(world) = state[r.slot] else { continue };
            let parent = match r.local_parent {
                Some(p) => state[p].unwrap_or(root),
                None if r.between == DMat4::IDENTITY => root,
                None => DMat4::IDENTITY,
            };
            let at = k_slot(r.slot);
            let (t, e) = to_local(r, &world, &parent, d.vec3(at + 1));
            d.set_output(at, PlugValue::Vec3(t));
            d.set_output(at + 1, PlugValue::Vec3(e));
            d.set_output(at + 2, PlugValue::Vec3(r.local_scale));
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Property names with their defaults. The ones a file from the reference shows are read from one; the Retarget Specific ones and the Reach
/// ones follow the attribute names in the guide and could not be checked against a file.
pub fn property_list() -> &'static [(String, f64)] {
    static LIST: std::sync::OnceLock<Vec<(String, f64)>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        let mut v: Vec<(String, f64)> = Vec::new();
        for (n, def) in [
            ("MatchSource", 0.0), ("Mirror", 0.0), ("ScaleCompensationMode", 1.0), ("ScaleCompensation", 100.0), ("HipsHeightCompensationMode", 1.0), ("HipsHeightCompensation", 0.0), ("AnkleProximityCompensationMode", 1.0), ("AnkleProximityCompensation", 0.0),
            ("AnkleHeightCompensationMode", 1.0), ("AnkleHeightCompensation", 0.0), ("MassCenterCompensationMode", 0.0), ("MassCenterCompensation", 80.0), ("ReachActorLeftWrist", 0.0), ("ReachActorRightWrist", 0.0),
            ("lkr", 0.5), ("rkr", 0.5), ("FootBottomToAnkle", 0.0), ("FootBackToAnkle", 0.0), ("FootMiddleToAnkle", 0.0), ("FootFrontToMiddle", 0.0), ("FootInToAnkle", 0.0), ("FootOutToAnkle", 0.0),
            ("HandBottomToWrist", 0.0), ("HandBackToWrist", 0.0), ("HandMiddleToWrist", 0.0), ("HandFrontToMiddle", 0.0), ("HandInToWrist", 0.0), ("HandOutToWrist", 0.0),
            ("CtrlPullLeftFoot", 1.0), ("CtrlPullRightFoot", 1.0), ("CtrlChestPullLeftHand", 1.0), ("CtrlChestPullRightHand", 1.0),
        ] {
            v.push((n.to_string(), def));
        }
        for limb in ["LeftHand", "RightHand", "LeftFoot", "RightFoot"] {
            for digit in ["Thumb", "Index", "Middle", "Ring", "Pinky", "ExtraFinger"] {
                v.push((format!("{limb}{digit}Tip"), 0.0));
            }
        }
        const ROLLS: [&str; 8] = ["LeftUpLeg", "LeftLeg", "RightUpLeg", "RightLeg", "LeftArm", "LeftForeArm", "RightArm", "RightForeArm"];
        for r in ROLLS {
            v.push((format!("{r}RollEx"), 0.0));
        }
        for k in 1..=5 {
            for r in ROLLS {
                v.push((format!("ParamLeaf{r}Roll{k}Mode"), 0.0));
                v.push((format!("ParamLeaf{r}Roll{k}"), 0.0));
            }
        }
        v
    })
}

fn property_index(name: &str) -> usize {
    P_FIRST + property_list().iter().position(|(n, _)| n == name).unwrap_or(0)
}

struct Properties;

impl NodeBehavior for Properties {
    fn compute(&mut self, _c: &EvalContext, d: &mut NodeData) {
        let v = |n: &str| d.float(property_index(n));
        let o = Options {
            match_source: v("MatchSource") != 0.0,
            mirror: v("Mirror") != 0.0,
            action_space_mode: v("ScaleCompensationMode") as i64,
            action_space: v("ScaleCompensation"),
            hips_level_mode: v("HipsHeightCompensationMode") as i64,
            hips_level: v("HipsHeightCompensation"),
            feet_spacing_mode: v("AnkleProximityCompensationMode") as i64,
            feet_spacing: v("AnkleProximityCompensation"),
            ankle_height_mode: v("AnkleHeightCompensationMode") as i64,
            ankle_height: v("AnkleHeightCompensation"),
            reach_hands: [v("ReachActorLeftWrist"), v("ReachActorRightWrist")],
        };
        put(d, P_OUT, Arc::new(PropertyData(o)));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Holds what it is given: nodes a file may carry whose work is not done here.
struct Holder;

impl NodeBehavior for Holder {
    fn compute(&mut self, _c: &EvalContext, _d: &mut NodeData) {}
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn data_in(name: &str) -> AttrDef {
    let mut a = data_attr(name, name, "A HumanIK state or definition.");
    a.flags = flags::INPUT | flags::HIDDEN;
    a
}

fn data_out(name: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: name.into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::OUTPUT | flags::HIDDEN, doc: "A HumanIK state or definition.".into(), ..Default::default() }
}

fn message(name: &str, short: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty: PlugType::Int, default_value: PlugValue::Int(0), flags: flags::INPUT | flags::HIDDEN, doc: "A message plug.".into(), ..Default::default() }
}

fn slot_vec(name: &str, def: f64, fl: u32) -> AttrDef {
    let mut a = vec3_attr(name, name, DVec3::splat(def), fl | flags::HIDDEN, "");
    for (k, c) in ["x", "y", "z"].iter().enumerate() {
        a.child_names[k] = format!("{name}{c}");
        a.child_short[k] = format!("{name}{c}");
    }
    a
}

fn matrix_in(name: &str) -> AttrDef {
    AttrDef { name: name.into(), short_name: name.into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: IN_STORE | flags::HIDDEN, doc: "A world matrix.".into(), ..Default::default() }
}

fn reg(name: &str, doc: &str, attrs: Vec<AttrDef>, make: crate::node::CreateFn) {
    types::register_type(NodeType { name: name.into(), icon: "bone".into(), doc: doc.into(), own_attrs: attrs, create: Some(make), ..Default::default() }).expect(name);
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let mut c = vec![data_out("OutputCharacterDefinition"), AttrDef { name: "InputCharacterizationLock".into(), short_name: "InputCharacterizationLock".into(), ty: PlugType::Bool, default_value: PlugValue::Bool(false), flags: IN_STORE | flags::HIDDEN, doc: "The definition is locked.".into(), ..Default::default() }, message("propertyState", "propertyState"), data_attr("rigInfo", "rigInfo", "How the mapped joints hang together; what the reference reads through .pm.")];
        for s in slot_names() {
            c.push(message(s, s));
            c.push(slot_vec(&format!("{s}T"), 0.0, IN_STORE));
            c.push(slot_vec(&format!("{s}R"), 0.0, IN_STORE));
            c.push(slot_vec(&format!("{s}S"), 1.0, IN_STORE));
            c.push(slot_vec(&format!("{s}JointOrient"), 0.0, IN_STORE));
            c.push(slot_vec(&format!("{s}MinRLimit"), 0.0, IN_STORE));
            c.push(slot_vec(&format!("{s}MaxRLimit"), 0.0, IN_STORE));
        }
        reg(CHARACTER, "A character definition: joints by slot, and its T-pose.", c, Arc::new(|| Box::new(CharacterNode)));

        for (name, doc) in [("HIKSK2State", "A skeleton's joints as a character state."), ("HIKFK2State", "A control rig's FK joints as a character state.")] {
            let mut a = vec![data_out("OutputCharacterState"), data_in("InputCharacterDefinition")];
            a.extend(slot_names().iter().map(|s| matrix_in(&format!("{s}GX"))));
            reg(name, doc, a, Arc::new(|| Box::new(SourceState)));
        }
        reg(
            "HIKRetargeterNode",
            "Turns a source character's state into the target's.",
            vec![data_out("OutputCharacterState"), data_in("InputCharacterState"), data_in("InputCharacterDefinitionSrc"), data_in("InputCharacterDefinitionDst"), data_in("InputSrcPropertySetState"), data_in("InputDstPropertySetState"), matrix_in("referenceGX")],
            Arc::new(|| Box::new(Retargeter)),
        );
        let stance = AttrDef { name: "InputStance".into(), short_name: "InputStance".into(), ty: PlugType::Bool, default_value: PlugValue::Bool(false), flags: IN_STORE | flags::HIDDEN, doc: "Hold the T-pose.".into(), ..Default::default() };
        reg("HIKSolverNode", "The full body solve: here it passes the state on or holds the stance.", vec![data_out("OutputCharacterState"), data_in("InputCharacterState"), data_in("InputCharacterDefinition"), data_in("InputPropertySetState"), stance], Arc::new(|| Box::new(Solver)));

        let mut k = vec![data_in("InputCharacterDefinition"), data_in("InputCharacterState"), matrix_in("rootParentGX"), data_attr("savedInputs", "savedInputs", "What the joints held before this node drove them.")];
        for s in slot_names() {
            k.push(slot_vec(&format!("{s}T"), 0.0, flags::OUTPUT));
            k.push(slot_vec(&format!("{s}R"), 0.0, flags::OUTPUT));
            k.push(slot_vec(&format!("{s}S"), 1.0, flags::OUTPUT));
        }
        reg("HIKState2SK", "A character state as each joint's channels.", k, Arc::new(|| Box::new(StateToSkeleton)));

        let mut p = vec![data_out("OutputPropertySetState"), message("message", "msg")];
        for (n, def) in property_list() {
            p.push(AttrDef { name: n.clone(), short_name: n.clone(), ty: PlugType::Float, default_value: PlugValue::Float(*def), flags: IN_STORE | flags::KEYABLE, doc: String::new(), ..Default::default() });
        }
        reg("HIKProperty2State", "A character's HumanIK properties.", p, Arc::new(|| Box::new(Properties)));

        for (name, doc) in [
            ("HIKEffector2State", "A control rig's effectors as a state. Held for files."),
            ("HIKPinning2State", "A control rig's pinning as a state. Held for files."),
            ("HIKState2FK", "A state as a control rig's FK joints. Held for files."),
            ("HIKState2GlobalSK", "A state in world space. Held for files."),
            ("HIKState2Effector", "A state as a control rig's effectors. Held for files."),
            ("HIKControlSetNode", "A control rig's set. Held for files."),
            ("CustomRigRetargeterNode", "A custom rig's retargeter. Held for files."),
        ] {
            reg(name, doc, vec![data_out("OutputCharacterState"), data_in("InputCharacterState"), data_in("InputCharacterDefinition")], Arc::new(|| Box::new(Holder)));
        }
    });
}
