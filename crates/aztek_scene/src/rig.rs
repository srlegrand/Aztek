//! Skeletons as the reference has them: the joint node and its attributes, and the commands that build and edit joint chains
//! (joint, orientJoint, mirrorJoint, insertJoint, removeJoint, connectJoint, disconnectJoint, reroot). Smooth skinning
//! is in skin.rs, constraints in constraint.rs, IK in ik.rs.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short};
use crate::graph::{DrivenEdit, Graph, SelectMode};
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::{euler_to_matrix, matrix_to_euler, RotateOrder, Transform, TransformBehavior};

pub use crate::constraint::{ConstraintNode, CONSTRAINT_TYPES};
pub use crate::ik::{IkHandle, IkSolve};

pub(crate) fn plug(n: NodeId, a: usize) -> PlugRef {
    PlugRef::new(n, a as i32)
}

pub(crate) fn data_attr(name: &str, short: &str, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Data,
        default_value: PlugValue::Data(None),
        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
        doc: doc.into(),
        ..Default::default()
    }
}

pub(crate) fn matrix_in(name: &str, short: &str, array: bool, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: PlugType::Matrix,
        default_value: PlugValue::Matrix(DMat4::IDENTITY),
        flags: flags::INPUT | flags::HIDDEN | if array { flags::ARRAY } else { 0 },
        doc: doc.into(),
        ..Default::default()
    }
}

/// Data payloads: plain JSON kept on a node (bind matrices, weights, rest poses).
#[derive(Debug, Clone, Default)]
pub struct JsonData(pub Value);

impl crate::plug::Data for JsonData {
    fn kind(&self) -> &str {
        "rigData"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "rigData", "value": self.0})
    }
    fn bytes(&self) -> usize {
        self.0.to_string().len()
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn json_of(d: &NodeData, attr: usize) -> Value {
    match d.value(attr) {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<JsonData>().map(|j| j.0.clone()).unwrap_or(Value::Null),
        _ => Value::Null,
    }
}

pub(crate) fn set_json(g: &mut Graph, n: NodeId, attr: usize, v: Value) -> CmdResult<()> {
    g.set_value(plug(n, attr), PlugValue::Data(Some(Arc::new(JsonData(v)))))?;
    Ok(())
}

pub(crate) fn mat_json(m: &DMat4) -> Value {
    json!(m.to_cols_array().to_vec())
}

pub(crate) fn mat_from(v: &Value) -> DMat4 {
    let a: Vec<f64> = v.as_array().map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
    if a.len() == 16 {
        DMat4::from_cols_array(&a.try_into().unwrap())
    } else {
        DMat4::IDENTITY
    }
}

pub(crate) fn vec3_json(v: DVec3) -> Value {
    json!([v.x, v.y, v.z])
}

pub(crate) fn rot3(m: &DMat4) -> DMat3 {
    let m3 = DMat3::from_mat4(*m);
    DMat3::from_cols(m3.x_axis.normalize_or_zero(), m3.y_axis.normalize_or_zero(), m3.z_axis.normalize_or_zero())
}

pub(crate) fn mat3(degrees: DVec3, order: RotateOrder) -> DMat3 {
    DMat3::from_mat4(euler_to_matrix(degrees, order))
}

// ---------------------------------------------------------------------------------------------------------------------
// Joints

pub struct Joint;

impl Joint {
    pub const JOINT_ORIENT: usize = Transform::COUNT;
    pub const RADIUS: usize = Transform::COUNT + 1;
    pub const SEGMENT_SCALE_COMPENSATE: usize = Transform::COUNT + 2;
    pub const INVERSE_SCALE: usize = Transform::COUNT + 3;
    pub const PREFERRED_ANGLE: usize = Transform::COUNT + 4;
    pub const STIFFNESS: usize = Transform::COUNT + 5;
    pub const MIN_ROT_LIMIT: usize = Transform::LIMITS + 4;
    pub const MAX_ROT_LIMIT: usize = Transform::LIMITS + 5;
    /// minRotXLimitEnable, then Y and Z.
    pub const MIN_LIMIT_ENABLE: usize = Transform::COUNT + 6;
    /// maxRotXLimitEnable, then Y and Z.
    pub const MAX_LIMIT_ENABLE: usize = Transform::COUNT + 9;
    /// jointTypeX, then Y and Z: the axis may rotate.
    pub const JOINT_TYPE: usize = Transform::COUNT + 12;
    pub const DRAW_STYLE: usize = Transform::COUNT + 15;
    pub const DRAW_LABEL: usize = Transform::COUNT + 16;
    pub const SIDE: usize = Transform::COUNT + 17;
    pub const TYPE: usize = Transform::COUNT + 18;
    pub const OTHER_TYPE: usize = Transform::COUNT + 19;
}

impl TransformBehavior for Joint {
    fn pre_rotation(&self, d: &NodeData) -> DMat4 {
        euler_to_matrix(d.vec3(Joint::JOINT_ORIENT), RotateOrder::Xyz)
    }
    /// The reference's joint: [S] [RO] [R] [JO] [IS] [T] in its row vectors. IS undoes the parent joint's scale, which arrives
    /// through inverseScale, so a scaled segment does not scale the ones below.
    fn inverse_parent_scale(&self, d: &NodeData) -> DMat4 {
        if !d.bool(Joint::SEGMENT_SCALE_COMPENSATE) {
            return DMat4::IDENTITY;
        }
        let s = d.vec3(Joint::INVERSE_SCALE);
        let inv = |x: f64| if x.abs() > 1e-12 { 1.0 / x } else { 1.0 };
        DMat4::from_scale(DVec3::new(inv(s.x), inv(s.y), inv(s.z)))
    }
}

impl NodeBehavior for Joint {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let local = Transform::local_matrix_of(self, d);
        let parent = d.matrix(Transform::PARENT_MATRIX);
        d.set_output(Transform::MATRIX, local);
        d.set_output(Transform::WORLD_MATRIX, parent * local);
    }
    fn as_transform(&self) -> Option<&dyn TransformBehavior> {
        Some(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn is_joint(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("joint"))
}

pub fn world(g: &mut Graph, id: NodeId) -> DMat4 {
    match g.value(&plug(id, Transform::WORLD_MATRIX)) {
        Ok(PlugValue::Matrix(m)) => m,
        _ => DMat4::IDENTITY,
    }
}

pub(crate) fn parent_world(g: &mut Graph, id: NodeId) -> DMat4 {
    let p = g.parent(id);
    if p == NO_NODE {
        DMat4::IDENTITY
    } else {
        world(g, p)
    }
}

pub(crate) fn vec3_of(g: &Graph, id: NodeId, attr: usize) -> DVec3 {
    g.node(id).map(|n| n.data().vec3(attr)).unwrap_or(DVec3::ZERO)
}

pub(crate) fn set_vec3(g: &mut Graph, id: NodeId, attr: usize, v: DVec3) -> CmdResult<()> {
    g.set_value(plug(id, attr), PlugValue::Vec3(v))?;
    Ok(())
}

fn rotate_order(g: &Graph, id: NodeId) -> RotateOrder {
    RotateOrder::from_index(g.node(id).map(|n| n.data().int(Transform::ROTATE_ORDER)).unwrap_or(0))
}

/// The joint's rotate as now evaluated (an IK solver or animation may drive it).
pub(crate) fn rotate_now(g: &mut Graph, id: NodeId) -> DVec3 {
    g.value(&plug(id, Transform::ROTATE)).map(|v| v.as_vec3()).unwrap_or(DVec3::ZERO)
}

fn child_joints(g: &Graph, id: NodeId) -> Vec<NodeId> {
    g.children(id).iter().copied().filter(|&c| is_joint(g, c)).collect()
}

/// Children that have a place of their own to keep: everything but shapes and IK effectors, which follow their joint.
fn placed_children(g: &Graph, id: NodeId) -> Vec<NodeId> {
    g.children(id)
        .iter()
        .copied()
        .filter(|&c| g.node(c).is_some_and(|n| n.ty().is_a("transform") && !n.ty().is_a("ikEffector")))
        .collect()
}

/// A joint's inverseScale takes its parent joint's scale, as the reference wires it; a joint under anything else has none.
pub(crate) fn relink_inverse_scale(g: &mut Graph, child: NodeId) {
    if !is_joint(g, child) {
        return;
    }
    let p = g.parent(child);
    let dst = plug(child, Joint::INVERSE_SCALE);
    if p != NO_NODE && is_joint(g, p) {
        let _ = g.connect(&plug(p, Transform::SCALE), &dst, true);
    } else if g.source(&dst).is_some() {
        g.disconnect(&dst);
        let _ = g.set_value(dst, PlugValue::Vec3(DVec3::ONE));
    }
}

/// A joint at a world position under parent (none for a root), oriented like its parent.
pub fn create_joint(g: &mut Graph, name: &str, parent: NodeId, at: DVec3, radius: f64) -> CmdResult<NodeId> {
    let t = types::find("joint").unwrap();
    let j = g.create_node(&t, if name.is_empty() { "joint1" } else { name }, parent, true)?;
    let pw = if parent == NO_NODE { DMat4::IDENTITY } else { world(g, parent) };
    set_vec3(g, j, Transform::TRANSLATE, pw.inverse().transform_point3(at))?;
    g.set_value(plug(j, Joint::RADIUS), PlugValue::Float(radius))?;
    relink_inverse_scale(g, j);
    Ok(j)
}

/// Puts a joint at a world matrix with its rotate as it is: translate and jointOrient take the difference. Other
/// transforms take translate, rotate and scale.
pub(crate) fn set_joint_world(g: &mut Graph, j: NodeId, w: &DMat4) -> CmdResult<()> {
    g.evaluate();
    let pw = parent_world(g, j);
    let local = pw.inverse() * *w;
    if !is_joint(g, j) {
        Transform::set_local_matrix(g, j, &local)?;
        return Ok(());
    }
    let n = g.node(j).unwrap();
    let compensated = n.behavior().as_transform().map(|t| t.inverse_parent_scale(n.data())).unwrap_or(DMat4::IDENTITY);
    let order = RotateOrder::from_index(n.data().int(Transform::ROTATE_ORDER));
    let r = mat3(n.data().vec3(Transform::ROTATE), order) * mat3(n.data().vec3(Transform::ROTATE_AXIS), RotateOrder::Xyz);
    let frame = rot3(&(compensated.inverse() * local));
    set_vec3(g, j, Transform::TRANSLATE, local.w_axis.truncate())?;
    set_vec3(g, j, Joint::JOINT_ORIENT, matrix_to_euler(&(frame * r.transpose()), RotateOrder::Xyz))?;
    Ok(())
}

/// Runs an edit on a joint while its children keep their world placement.
fn keeping_children(g: &mut Graph, j: NodeId, edit: impl FnOnce(&mut Graph) -> CmdResult<()>) -> CmdResult<()> {
    g.evaluate();
    let kids = placed_children(g, j);
    let worlds: Vec<DMat4> = kids.iter().map(|&k| world(g, k)).collect();
    edit(g)?;
    for (k, w) in kids.into_iter().zip(worlds) {
        // A child some other node drives stays with its driver.
        let _ = set_joint_world(g, k, &w);
    }
    Ok(())
}

/// The joint's world rotation set to frame, with rotate and rotateAxis at zero: jointOrient takes it. Children keep
/// their world placement.
fn set_world_orientation(g: &mut Graph, j: NodeId, frame: DMat3) -> CmdResult<()> {
    let pw = rot3(&parent_world(g, j));
    keeping_children(g, j, |g| {
        set_vec3(g, j, Transform::ROTATE, DVec3::ZERO)?;
        set_vec3(g, j, Transform::ROTATE_AXIS, DVec3::ZERO)?;
        set_vec3(g, j, Joint::JOINT_ORIENT, matrix_to_euler(&(pw.transpose() * frame), RotateOrder::Xyz))
    })
}

/// The reference's -zso: rotateAxis to zero, jointOrient taking the change. Nothing moves.
fn zero_scale_orient(g: &mut Graph, j: NodeId) -> CmdResult<()> {
    let n = g.node(j).unwrap().data();
    let (jo, r, ra) = (
        mat3(n.vec3(Joint::JOINT_ORIENT), RotateOrder::Xyz),
        mat3(n.vec3(Transform::ROTATE), RotateOrder::from_index(n.int(Transform::ROTATE_ORDER))),
        mat3(n.vec3(Transform::ROTATE_AXIS), RotateOrder::Xyz),
    );
    let jo2 = jo * r * ra * r.transpose();
    set_vec3(g, j, Joint::JOINT_ORIENT, matrix_to_euler(&jo2, RotateOrder::Xyz))?;
    set_vec3(g, j, Transform::ROTATE_AXIS, DVec3::ZERO)
}

/// The reference's orientJoint: the first axis of `order` aims at the first child joint, the second toward `up` (a world axis
/// like "yup"). Joints with no child line up with their parent ("none" zeroes jointOrient).
pub fn orient_joint(g: &mut Graph, j: NodeId, order: &str, up: &str) -> CmdResult<()> {
    g.evaluate();
    if order == "none" {
        return keeping_children(g, j, |g| set_vec3(g, j, Joint::JOINT_ORIENT, DVec3::ZERO));
    }
    let o: Vec<char> = order.chars().collect();
    if o.len() != 3 {
        fail!("orientJoint: {order} is not an axis order like xyz");
    }
    let child = g.children(j).iter().copied().find(|&c| is_joint(g, c));
    let here = world(g, j).w_axis.truncate();
    let Some(child) = child else {
        // End joints take their parent's orientation.
        let p = g.parent(j);
        let frame = if p != NO_NODE && is_joint(g, p) { rot3(&world(g, p)) } else { DMat3::IDENTITY };
        return set_world_orientation(g, j, frame);
    };
    let aim = (world(g, child).w_axis.truncate() - here).normalize_or_zero();
    if aim == DVec3::ZERO {
        return Ok(());
    }
    let mut upv = match up {
        "xup" => DVec3::X,
        "xdown" => -DVec3::X,
        "ydown" => -DVec3::Y,
        "zup" => DVec3::Z,
        "zdown" => -DVec3::Z,
        _ => DVec3::Y,
    };
    if upv.cross(aim).length() < 1e-6 {
        upv = if aim.cross(DVec3::Z).length() > 1e-6 { DVec3::Z } else { DVec3::X };
    }
    let second = (upv - aim * upv.dot(aim)).normalize();
    let third = aim.cross(second);
    // Columns: the world direction of local X, Y, Z.
    let mut cols = [DVec3::ZERO; 3];
    let idx = |c: char| match c {
        'x' => 0,
        'y' => 1,
        _ => 2,
    };
    let (a, b, c) = (idx(o[0]), idx(o[1]), idx(o[2]));
    cols[a] = aim;
    cols[b] = second;
    // Right handed: the third axis completes the frame.
    let sign = if matches!((a, b), (0, 1) | (1, 2) | (2, 0)) { 1.0 } else { -1.0 };
    cols[c] = third * sign;
    set_world_orientation(g, j, DMat3::from_cols(cols[0], cols[1], cols[2]))
}

fn has_skin(g: &Graph, j: NodeId) -> bool {
    g.destinations(&plug(j, Transform::WORLD_MATRIX)).iter().any(|d| g.node(d.node).is_some_and(|n| n.ty().name == crate::skin::SKIN_TYPE))
}

// ---------------------------------------------------------------------------------------------------------------------
// Editing skeletons

fn copy_joint_settings(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    let n = g.node(from).unwrap().data();
    let mut vals = Vec::new();
    for a in [
        Transform::ROTATE_ORDER,
        Transform::ROTATE_AXIS,
        Transform::SCALE,
        Joint::SEGMENT_SCALE_COMPENSATE,
        Joint::PREFERRED_ANGLE,
        Joint::STIFFNESS,
        Joint::MIN_ROT_LIMIT,
        Joint::MAX_ROT_LIMIT,
        Joint::DRAW_STYLE,
        Joint::DRAW_LABEL,
        Joint::SIDE,
        Joint::TYPE,
        Joint::OTHER_TYPE,
    ] {
        vals.push((a, n.value(a).clone()));
    }
    for k in 0..3 {
        for base in [Joint::MIN_LIMIT_ENABLE, Joint::MAX_LIMIT_ENABLE, Joint::JOINT_TYPE] {
            vals.push((base + k, n.value(base + k).clone()));
        }
    }
    for (a, v) in vals {
        g.set_value(plug(to, a), v)?;
    }
    Ok(())
}

struct Mirror {
    /// The reflection across the plane through the world origin.
    m: DMat3,
    behavior: bool,
    search: Option<(String, String)>,
}

fn mirror_one(g: &mut Graph, src: NodeId, parent: NodeId, how: &Mirror, out: &mut Vec<NodeId>) -> CmdResult<()> {
    let w = world(g, src);
    let name = g.node(src).unwrap().name().to_string();
    let name = match &how.search {
        Some((a, b)) => name.replace(a.as_str(), b),
        None => name,
    };
    let radius = g.node(src).unwrap().data().float(Joint::RADIUS);
    let at = how.m * w.w_axis.truncate();
    let new = create_joint(g, &name, parent, at, radius)?;
    let rotate = rotate_now(g, src);
    copy_joint_settings(g, src, new)?;
    set_vec3(g, new, Transform::ROTATE, rotate)?;
    // Labels swap sides.
    let side = g.node(src).unwrap().data().int(Joint::SIDE);
    if side == 1 || side == 2 {
        g.set_value(plug(new, Joint::SIDE), PlugValue::Int(3 - side))?;
    }
    // Behaviour: every axis is the mirror image reversed, so equal rotate values make mirrored motion and a right
    // handed frame. Orientation: the same world axes as the source.
    let frame = if how.behavior { how.m * rot3(&w) * -1.0 } else { rot3(&w) };
    let order = rotate_order(g, new);
    let r = mat3(rotate, order) * mat3(vec3_of(g, new, Transform::ROTATE_AXIS), RotateOrder::Xyz);
    let pw = rot3(&parent_world(g, new));
    set_vec3(g, new, Joint::JOINT_ORIENT, matrix_to_euler(&(pw.transpose() * frame * r.transpose()), RotateOrder::Xyz))?;
    out.push(new);
    g.evaluate();
    for k in child_joints(g, src) {
        mirror_one(g, k, new, how, out)?;
    }
    Ok(())
}

/// The reference's insertJoint: a new joint under `j`, at its place and in its frame; j's child joints move under it.
pub fn insert_joint(g: &mut Graph, j: NodeId) -> CmdResult<NodeId> {
    if has_skin(g, j) {
        fail!("insertJoint: {} has skin attached", g.path(j));
    }
    let radius = g.node(j).unwrap().data().float(Joint::RADIUS);
    let kids = child_joints(g, j);
    let t = types::find("joint").unwrap();
    let new = g.create_node(&t, "joint1", j, true)?;
    g.set_value(plug(new, Joint::RADIUS), PlugValue::Float(radius))?;
    relink_inverse_scale(g, new);
    for k in kids {
        g.set_parent(k, new, false, usize::MAX)?;
    }
    crate::ik::refresh_handles(g)?;
    Ok(new)
}

/// The reference's removeJoint: the joint goes, its children stay where they are under its parent.
pub fn remove_joint(g: &mut Graph, j: NodeId) -> CmdResult<()> {
    if has_skin(g, j) {
        fail!("removeJoint: {} has skin attached", g.path(j));
    }
    g.evaluate();
    let p = g.parent(j);
    let kids = placed_children(g, j);
    let worlds: Vec<DMat4> = kids.iter().map(|&k| world(g, k)).collect();
    crate::ik::leave_chain(g, j)?;
    for (&k, w) in kids.iter().zip(&worlds) {
        g.set_parent(k, p, false, usize::MAX)?;
        let _ = set_joint_world(g, k, w);
    }
    g.delete_node(j)?;
    crate::ik::refresh_handles(g)
}

/// The reference's connectJoint. -pm parents `first` under `second` where it stands, a bone between them; -cm brings the first
/// skeleton's root to the second joint first.
pub fn connect_joint(g: &mut Graph, first: NodeId, second: NodeId, parent_mode: bool) -> CmdResult<()> {
    g.evaluate();
    let mut w = world(g, first);
    if !parent_mode {
        w.w_axis = world(g, second).w_axis;
    }
    g.set_parent(first, second, false, usize::MAX)?;
    set_joint_world(g, first, &w)?;
    crate::ik::refresh_handles(g)
}

/// The reference's disconnectJoint: the skeleton breaks at `j`, which becomes a root. The old parent keeps a new end joint at
/// that place, so its bone stays. Handles across the break go. Returns the new end joint.
pub fn disconnect_joint(g: &mut Graph, j: NodeId) -> CmdResult<NodeId> {
    let p = g.parent(j);
    if p == NO_NODE || !is_joint(g, p) {
        fail!("disconnectJoint: {} has no parent joint", g.path(j));
    }
    g.evaluate();
    let w = world(g, j);
    let radius = g.node(j).unwrap().data().float(Joint::RADIUS);
    let leaf = create_joint(g, "joint1", p, w.w_axis.truncate(), radius)?;
    g.set_parent(j, NO_NODE, false, usize::MAX)?;
    set_joint_world(g, j, &w)?;
    crate::ik::refresh_handles(g)?;
    Ok(leaf)
}

/// The reference's reroot: `j` becomes the root of its skeleton. Every joint stays where it is in the world.
pub fn reroot(g: &mut Graph, j: NodeId) -> CmdResult<()> {
    g.evaluate();
    let mut path = vec![j];
    let mut p = g.parent(j);
    while p != NO_NODE && is_joint(g, p) {
        path.push(p);
        p = g.parent(p);
    }
    if path.len() < 2 {
        return Ok(());
    }
    let worlds: Vec<DMat4> = path.iter().map(|&x| world(g, x)).collect();
    g.set_parent(j, p, false, usize::MAX)?;
    for pair in path.windows(2) {
        g.set_parent(pair[1], pair[0], false, usize::MAX)?;
    }
    for (&x, w) in path.iter().zip(&worlds) {
        set_joint_world(g, x, w)?;
    }
    crate::ik::refresh_handles(g)
}

// ---------------------------------------------------------------------------------------------------------------------
// Keeping rig nodes in step

/// What rig nodes copy from nodes they also drive, which a connection can't do: the graph orders whole nodes, so an
/// aim constraint reading the translate of the object whose rotate it drives would be a cycle. Runs around evaluation:
/// before it, so a direct edit is seen in the same pass; after it, so an animated value is seen too.
fn sync(g: &mut Graph, after: bool) {
    if after && g.rig_synced_pass == g.evaluation_pass() {
        return;
    }
    for _ in 0..3 {
        let mut changed = false;
        for id in g.nodes() {
            let Some(n) = g.node(id) else { continue };
            let name = n.ty().name.as_str();
            if name == "ikHandle" {
                changed |= crate::ik::sync_handle(g, id, after);
            } else if CONSTRAINT_TYPES.contains(&name) {
                changed |= crate::constraint::sync(g, id);
            }
        }
        if !after || !changed {
            break;
        }
        g.evaluate();
    }
    if after {
        g.rig_synced_pass = g.evaluation_pass();
    }
}

/// A set on a plug a rig node drives, routed to what the driver reads. None when the driver isn't a rig node.
pub(crate) fn driven_scalar_set(g: &mut Graph, dst: &PlugRef, src: &PlugRef, v: f64) -> Option<DrivenEdit> {
    let driver = g.node(src.node)?.ty().name.clone();
    if passive(&driver) {
        return Some(DrivenEdit::Handled);
    }
    if driver != "ikSolve" {
        return None;
    }
    let mut whole = g.value(&dst.whole()).map(|x| x.as_vec3()).unwrap_or(DVec3::ZERO);
    if dst.component >= 0 {
        whole[dst.component as usize] = v;
    }
    Some(crate::ik::set_fk(g, src, whole))
}

/// Drivers whose outputs the reference makes passive: a set on what they drive is taken without an error, as the
/// reference takes it, and what the driver gives stays (mGear sets a constrained scale this way, and builds on the
/// constraint's value).
fn passive(driver: &str) -> bool {
    driver == "pairBlend" || CONSTRAINT_TYPES.contains(&driver)
}

/// Installs what keeps rig nodes in step with the scene. The delete hook is shared with instancing.
pub fn install(g: &mut Graph) {
    g.add_evaluate_hook(Arc::new(|g: &mut Graph, after: bool| sync(g, after)));
}

/// A vector set on a channel IK or a geometry constraint drives. None leaves it to the animation hook.
pub(crate) fn driven_vec_set(g: &mut Graph, dst: &PlugRef, src: &PlugRef, v: DVec3) -> Option<DrivenEdit> {
    let driver = g.node(src.node).map(|n| n.ty().name.clone())?;
    if driver == "ikSolve" {
        return Some(crate::ik::set_fk(g, src, v));
    }
    if passive(&driver) && driver != "geometryConstraint" {
        return Some(DrivenEdit::Handled);
    }
    if driver == "geometryConstraint" && dst.attr as usize == Transform::TRANSLATE {
        return Some(crate::constraint::set_wanted_position(g, src.node, v));
    }
    None
}

/// Before a delete: nodes a rig needs go with it, and what the rig drives holds its pose.
pub(crate) fn before_delete(g: &mut Graph, doomed: &[NodeId]) {
    let any = g.nodes().iter().any(|&id| g.node(id).is_some_and(|n| n.ty().name == "ikHandle" || CONSTRAINT_TYPES.contains(&n.ty().name.as_str())));
    if !any {
        return;
    }
    g.evaluate();
    crate::constraint::before_delete(g, doomed);
    crate::ik::before_delete(g, doomed);
}

// ---------------------------------------------------------------------------------------------------------------------
// Types

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(register_types_now);
}

fn named_bool(name: &str, short: &str, def: bool, doc: &str) -> AttrDef {
    AttrDef { short_name: short.into(), ..bool_attr(name, def, doc) }
}

fn limit_attr(name: &str, short: &str, prefix: &str, def: f64) -> AttrDef {
    let mut a = vec3_attr(name, short, DVec3::splat(def), flags::INPUT | flags::STORABLE, "A rotate limit per axis, degrees, used when its enable is on.");
    let pair = if prefix == "min" { "mr" } else { "xr" };
    for (i, axis) in ["X", "Y", "Z"].into_iter().enumerate() {
        a.child_names[i] = format!("{prefix}Rot{axis}Limit");
        a.child_short[i] = format!("{pair}{}l", axis.to_lowercase());
    }
    a
}

/// A joint keeps its limit switches as one bool per axis (minRotXLimitEnable), so the transform's Vec3 of them gives up
/// its children's names here.
fn unused_enable(name: &str, short: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        short_name: short.into(),
        ty: crate::plug::PlugType::Vec3,
        default_value: PlugValue::Vec3(DVec3::ZERO),
        flags: flags::INPUT | flags::HIDDEN,
        ..Default::default()
    }
}

fn register_types_now() {
    crate::plug::data::register_kind("rigData", Arc::new(|j| Ok(Arc::new(JsonData(j.get("value").cloned().unwrap_or(Value::Null))) as Arc<dyn crate::plug::Data>)));
    let xf = types::find("transform").expect("transform");
    let store = flags::INPUT | flags::STORABLE;
    let mut own = vec![
        vec3_attr("jointOrient", "jo", DVec3::ZERO, store, "Orientation between the parent and rotate, Euler XYZ degrees."),
        float_attr_short("radius", "radi", 1.0, "Display size of the joint.", Some(0.0), None),
        named_bool("segmentScaleCompensate", "ssc", true, "This joint doesn't inherit its parent joint's scale."),
        vec3_attr("inverseScale", "is", DVec3::ONE, store | flags::HIDDEN, "The parent joint's scale, undone when segmentScaleCompensate is on."),
        vec3_attr("preferredAngle", "pa", DVec3::ZERO, store, "The rest pose IK solvers start from and bend away from."),
        vec3_attr("stiffness", "st", DVec3::ZERO, store, "Resistance to rotating, per axis, 0 to 100."),
        limit_attr("minRotLimit", "mnrl", "min", -360.0),
        limit_attr("maxRotLimit", "mxrl", "max", 360.0),
        unused_enable("minRotLimitEnable", "mrle"),
        unused_enable("maxRotLimitEnable", "xrle"),
    ];
    for (prefix, short) in [("min", "mn"), ("max", "mx")] {
        for axis in ["X", "Y", "Z"] {
            own.push(named_bool(&format!("{prefix}Rot{axis}LimitEnable"), &format!("{short}{}e", axis.to_lowercase()), false, "This rotate limit applies."));
        }
    }
    for axis in ["X", "Y", "Z"] {
        own.push(named_bool(&format!("jointType{axis}"), &format!("jt{}", axis.to_lowercase()), true, "The joint may rotate about this axis."));
    }
    own.push(AttrDef { short_name: "ds".into(), ..enum_attr("drawStyle", &["Bone", "Multi-child as Box", "None"], 0, "How the joint is drawn.") });
    // The reference's short name dl is displayLayer here.
    own.push(named_bool("drawLabel", "drl", false, "Draw the label at the joint."));
    own.push(AttrDef { short_name: "sd".into(), ..enum_attr("side", &["Center", "Left", "Right", "None"], 0, "Which side of the body, for labelling.") });
    own.push(AttrDef {
        short_name: "typ".into(),
        ..enum_attr(
            "type",
            &[
                "None", "Root", "Hip", "Knee", "Foot", "Toe", "Spine", "Neck", "Head", "Collar", "Shoulder", "Elbow", "Hand", "Finger", "Thumb", "PropA", "PropB", "PropC", "Other",
                "Index Finger", "Middle Finger", "Ring Finger", "Pinky Finger", "Extra Finger", "Big Toe", "Index Toe", "Middle Toe", "Ring Toe", "Pinky Toe", "Foot Thumb",
            ],
            0,
            "What the joint is, for labelling.",
        )
    });
    own.push(AttrDef {
        name: "otherType".into(),
        short_name: "otp".into(),
        ty: PlugType::String,
        default_value: PlugValue::String("jointNN".into()),
        flags: store,
        doc: "The label when type is Other.".into(),
        ..Default::default()
    });
    // A message plug to a character's slot. Its 1 tells the character the slot is filled.
    own.push(AttrDef {
        name: "Character".into(),
        short_name: "ch".into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(1),
        flags: flags::INPUT | flags::HIDDEN,
        doc: "Goes to a character's slot.".into(),
        ..Default::default()
    });
    let joint = NodeType {
        name: "joint".into(),
        base: Some(xf),
        dag: true,
        icon: "bone".into(),
        doc: "A skeleton joint: a transform with a jointOrient, drawn as a ball with bones to its child joints.".into(),
        own_attrs: own,
        create: Some(Arc::new(|| Box::new(Joint))),
        ..Default::default()
    };
    types::register_type(joint).expect("joint");
    crate::ik::register_types();
    crate::constraint::register_types();
    // A copied skeleton's inverseScale follows its copied parents.
    crate::duplicate::add_fixup(Arc::new(|g: &mut Graph, copied: &std::collections::HashMap<NodeId, NodeId>| {
        for &c in copied.values() {
            relink_inverse_scale(g, c);
        }
    }));
}

pub fn is_rig_chain_node(type_name: &str) -> bool {
    type_name == crate::skin::SKIN_TYPE || crate::deform::is_deformer(type_name)
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

pub(crate) type Cmd = CommandDef<Session>;

pub(crate) fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

pub(crate) fn find(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name)? {
        Some(id) => Ok(id),
        None => fail!("{cmd}: no object named {name}"),
    }
}

fn named_or_selected(s: &Session, a: &Args) -> Vec<String> {
    objects(s, a)
}

/// The flags asked about in query mode.
pub(crate) fn asked(a: &Args) -> Vec<String> {
    a.get("queried").and_then(Value::as_array).map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

const ORDERS: [&str; 6] = ["xyz", "yzx", "zxy", "xzy", "yxz", "zyx"];
const AXES: [(&str, usize); 3] = [("X", 0), ("Y", 1), ("Z", 2)];

fn bool_pair(v: &Value) -> (bool, bool) {
    let one = |x: Option<&Value>| x.map(|x| x.as_bool().unwrap_or(x.as_f64().is_some_and(|f| f != 0.0))).unwrap_or(false);
    (one(v.get(0)), one(v.get(1)))
}

/// The settings -e and create share.
fn apply_joint_flags(g: &mut Graph, id: NodeId, a: &Args) -> CmdResult<()> {
    let set_float = |g: &mut Graph, attr: usize, k: usize, v: f64| -> CmdResult<()> {
        g.set_value(PlugRef::component(id, attr as i32, k as i32), PlugValue::Float(v))?;
        Ok(())
    };
    if let Some(r) = a.get("radius").and_then(Value::as_f64) {
        g.set_value(plug(id, Joint::RADIUS), PlugValue::Float(r))?;
    }
    if let Some(o) = a.vec3("orientation") {
        set_vec3(g, id, Joint::JOINT_ORIENT, DVec3::from_array(o))?;
    }
    if let Some(o) = a.vec3("scaleOrientation") {
        set_vec3(g, id, Transform::ROTATE_AXIS, DVec3::from_array(o))?;
    }
    if let Some(o) = a.vec3("scale") {
        set_vec3(g, id, Transform::SCALE, DVec3::from_array(o))?;
    }
    if a.has("rotationOrder") {
        let name = a.string("rotationOrder", "xyz");
        let Some(i) = ORDERS.iter().position(|o| *o == name) else { fail!("joint: {name} is not a rotation order") };
        g.set_value(plug(id, Transform::ROTATE_ORDER), PlugValue::Int(i as i64))?;
    }
    for flag in ["scaleCompensate", "segmentScaleCompensate"] {
        if a.has(flag) {
            g.set_value(plug(id, Joint::SEGMENT_SCALE_COMPENSATE), PlugValue::Bool(a.bool(flag)))?;
        }
    }
    if a.has("degreeOfFreedom") {
        let dof = a.string("degreeOfFreedom", "xyz");
        for (axis, k) in [('x', 0), ('y', 1), ('z', 2)] {
            g.set_value(plug(id, Joint::JOINT_TYPE + k), PlugValue::Bool(dof.contains(axis)))?;
        }
    }
    if let Some(v) = a.vec3("stiffness") {
        set_vec3(g, id, Joint::STIFFNESS, DVec3::from_array(v))?;
    }
    for (axis, k) in AXES {
        if let Some(v) = a.get(&format!("angle{axis}")).and_then(Value::as_f64) {
            set_float(g, Transform::ROTATE, k, v)?;
        }
        if let Some(v) = a.get(&format!("stiffness{axis}")).and_then(Value::as_f64) {
            set_float(g, Joint::STIFFNESS, k, v)?;
        }
        if let Some(l) = a.get(&format!("limit{axis}")) {
            let (lo, hi) = (l.get(0).and_then(Value::as_f64), l.get(1).and_then(Value::as_f64));
            let (Some(lo), Some(hi)) = (lo, hi) else { fail!("joint: -limit{axis} takes a lower and an upper angle") };
            set_float(g, Joint::MIN_ROT_LIMIT, k, lo)?;
            set_float(g, Joint::MAX_ROT_LIMIT, k, hi)?;
        }
        if let Some(l) = a.get(&format!("limitSwitch{axis}")) {
            let (lo, hi) = bool_pair(l);
            g.set_value(plug(id, Joint::MIN_LIMIT_ENABLE + k), PlugValue::Bool(lo))?;
            g.set_value(plug(id, Joint::MAX_LIMIT_ENABLE + k), PlugValue::Bool(hi))?;
        }
    }
    if a.bool("setPreferredAngles") {
        let r = rotate_now(g, id);
        set_vec3(g, id, Joint::PREFERRED_ANGLE, r)?;
    }
    if a.bool("assumePreferredAngles") {
        let pa = vec3_of(g, id, Joint::PREFERRED_ANGLE);
        set_vec3(g, id, Transform::ROTATE, pa)?;
    }
    Ok(())
}

fn joint_query(g: &mut Graph, id: NodeId, a: &Args) -> CmdResult {
    let flag = asked(a).into_iter().find(|n| !matches!(n.as_str(), "relative" | "absolute" | "children" | "component")).unwrap_or_else(|| "position".into());
    g.evaluate();
    let n = g.node(id).unwrap().data();
    let vec = |v: DVec3| json!([v.x, v.y, v.z]);
    for (axis, k) in AXES {
        if flag == format!("angle{axis}") {
            return Ok(json!(n.vec3(Transform::ROTATE)[k]));
        }
        if flag == format!("stiffness{axis}") {
            return Ok(json!(n.vec3(Joint::STIFFNESS)[k]));
        }
        if flag == format!("limit{axis}") {
            return Ok(json!([n.vec3(Joint::MIN_ROT_LIMIT)[k], n.vec3(Joint::MAX_ROT_LIMIT)[k]]));
        }
        if flag == format!("limitSwitch{axis}") {
            return Ok(json!([n.bool(Joint::MIN_LIMIT_ENABLE + k), n.bool(Joint::MAX_LIMIT_ENABLE + k)]));
        }
    }
    Ok(match flag.as_str() {
        "position" => {
            if a.bool("relative") {
                vec(n.vec3(Transform::TRANSLATE))
            } else {
                vec(n.matrix(Transform::WORLD_MATRIX).w_axis.truncate())
            }
        }
        // A one element array, as the reference returns it.
        "radius" => json!([n.float(Joint::RADIUS)]),
        "orientation" => vec(n.vec3(Joint::JOINT_ORIENT)),
        "scaleOrientation" => vec(n.vec3(Transform::ROTATE_AXIS)),
        "scale" => vec(n.vec3(Transform::SCALE)),
        "stiffness" => vec(n.vec3(Joint::STIFFNESS)),
        "rotationOrder" => json!(ORDERS[n.int(Transform::ROTATE_ORDER).clamp(0, 5) as usize]),
        "scaleCompensate" | "segmentScaleCompensate" => json!(n.bool(Joint::SEGMENT_SCALE_COMPENSATE)),
        "degreeOfFreedom" => {
            let dof: String = ['x', 'y', 'z'].iter().enumerate().filter(|(k, _)| n.bool(Joint::JOINT_TYPE + k)).map(|(_, c)| *c).collect();
            json!(dof)
        }
        "name" => json!(g.node(id).unwrap().name()),
        other => fail!("joint: can't query -{other}"),
    })
}

fn mirror_command(s: &mut Session, a: &Args) -> CmdResult {
    let names = named_or_selected(s, a);
    let g = &mut s.graph;
    let m = if a.bool("mirrorYZ") {
        DVec3::new(-1.0, 1.0, 1.0)
    } else if a.bool("mirrorXZ") {
        DVec3::new(1.0, -1.0, 1.0)
    } else if a.bool("mirrorXY") {
        DVec3::new(1.0, 1.0, -1.0)
    } else {
        fail!("mirrorJoint: give the plane: -mirrorYZ, -mirrorXY or -mirrorXZ");
    };
    let search = match a.get("searchReplace").and_then(Value::as_array).map(Vec::as_slice) {
        Some([x, y]) => match (x.as_str(), y.as_str()) {
            (Some(x), Some(y)) => Some((x.to_string(), y.to_string())),
            _ => fail!("mirrorJoint: -searchReplace takes two strings"),
        },
        Some(_) => fail!("mirrorJoint: -searchReplace takes two strings"),
        None => None,
    };
    let how = Mirror { m: DMat3::from_diagonal(m), behavior: a.bool("mirrorBehavior"), search };
    if names.is_empty() {
        fail!("mirrorJoint: name or select a joint");
    }
    let mut made = Vec::new();
    for n in names {
        let id = find(g, &n, "mirrorJoint")?;
        if !is_joint(g, id) {
            fail!("mirrorJoint: {n} is not a joint");
        }
        g.evaluate();
        let p = g.parent(id);
        mirror_one(g, id, p, &how, &mut made)?;
    }
    if let Some(&first) = made.first() {
        g.select(&[first], SelectMode::Replace);
    }
    Ok(json!(made.iter().map(|&j| g.path(j)).collect::<Vec<_>>()))
}

fn one_joint(s: &Session, a: &Args, cmd: &str) -> CmdResult<NodeId> {
    let names = named_or_selected(s, a);
    let Some(n) = names.first() else { fail!("{cmd}: name or select a joint") };
    let id = find(&s.graph, n, cmd)?;
    if !is_joint(&s.graph, id) {
        fail!("{cmd}: {n} is not a joint");
    }
    Ok(id)
}

pub fn register(c: &mut Commands<Session>) {
    register_types();

    let mut joint = Cmd::new(
        "joint",
        "Create a joint at -p (world), under the selected joint, and select it. With -e: -oj orients joints (xyz...), \
         -sao the secondary axis (yup...), -ch their children too, -zso zeroes scale orient, -spa and -apa set and assume \
         preferred angles. -q asks about one flag; alone it returns the position.",
        |s, a| {
            let names = a.named();
            let g = &mut s.graph;
            if a.has("exists") && !a.bool("query") {
                let n = a.string("exists", "");
                return Ok(json!(g.find(&n)?.is_some_and(|id| is_joint(g, id))));
            }
            if a.bool("query") {
                let names = if names.is_empty() { g.selection().iter().map(|&i| g.path(i)).collect() } else { names };
                let Some(n) = names.first() else { fail!("joint: name a joint") };
                if asked(a).iter().any(|x| x == "exists") {
                    return Ok(json!(g.find(n)?.is_some_and(|id| is_joint(g, id))));
                }
                let id = find(g, n, "joint")?;
                return joint_query(g, id, a);
            }
            if a.bool("edit") {
                let names = if names.is_empty() { g.selection().iter().map(|&i| g.path(i)).collect() } else { names };
                let order = a.string("orientJoint", "");
                let up = a.string("secondaryAxisOrient", "yup");
                let mut todo = Vec::new();
                for n in names {
                    let id = find(g, &n, "joint")?;
                    todo.push(id);
                    if a.bool("children") {
                        for d in g.descendants(id) {
                            if is_joint(g, d) {
                                todo.push(d);
                            }
                        }
                    }
                }
                for id in todo {
                    if !order.is_empty() {
                        orient_joint(g, id, &order, &up)?;
                    }
                    if let Some(p) = a.vec3("position") {
                        g.evaluate();
                        let pw = parent_world(g, id);
                        let p = DVec3::from_array(p);
                        let local = if a.bool("relative") { vec3_of(g, id, Transform::TRANSLATE) + p } else { pw.inverse().transform_point3(p) };
                        if a.bool("component") {
                            keeping_children(g, id, |g| set_vec3(g, id, Transform::TRANSLATE, local))?;
                        } else {
                            set_vec3(g, id, Transform::TRANSLATE, local)?;
                        }
                    }
                    apply_joint_flags(g, id, a)?;
                    if a.bool("zeroScaleOrient") {
                        zero_scale_orient(g, id)?;
                    }
                }
                return Ok(Value::Null);
            }
            let parent = g.selection().last().copied().filter(|&id| is_joint(g, id)).unwrap_or(NO_NODE);
            g.evaluate();
            let p = DVec3::from_array(a.vec3("position").unwrap_or([0.0; 3]));
            let at = if a.bool("relative") && parent != NO_NODE { world(g, parent).transform_point3(p) } else { p };
            let j = create_joint(g, &a.string("name", ""), parent, at, a.float("radius", 1.0))?;
            apply_joint_flags(g, j, a)?;
            // At creation -oj orients the joint this one extends.
            if parent != NO_NODE && a.has("orientJoint") {
                orient_joint(g, parent, &a.string("orientJoint", "xyz"), &a.string("secondaryAxisOrient", "yup"))?;
            }
            g.select(&[j], SelectMode::Replace);
            Ok(json!(g.path(j)))
        },
    )
    .positional("joints (edit and query)")
    .flag(f("position", "p", ArgType::Vec3, "World position (or offset from the parent joint with -r)."))
    .flag(f("relative", "r", ArgType::Bool, "-p is relative to the parent joint."))
    .flag(f("absolute", "a", ArgType::Bool, "-p is a world position (the default)."))
    .flag(f("component", "co", ArgType::Bool, "Edit: -p moves this joint alone; its children stay."))
    .flag(f("name", "n", ArgType::String, "Name of the new joint."))
    .flag(f("radius", "rad", ArgType::Float, "Display size."))
    .flag(f("orientation", "o", ArgType::Vec3, "jointOrient, degrees."))
    .flag(f("scaleOrientation", "so", ArgType::Vec3, "rotateAxis, degrees."))
    .flag(f("scale", "s", ArgType::Vec3, "Scale."))
    .flag(f("scaleCompensate", "sc", ArgType::Bool, "segmentScaleCompensate."))
    .flag(f("segmentScaleCompensate", "ssc", ArgType::Bool, "The joint doesn't inherit its parent joint's scale."))
    .flag(f("rotationOrder", "roo", ArgType::String, "xyz, yzx, zxy, xzy, yxz or zyx."))
    .flag(f("degreeOfFreedom", "dof", ArgType::String, "The axes that may rotate: xyz, xz, y..."))
    .flag(f("stiffness", "st", ArgType::Vec3, "Stiffness per axis."))
    .flag(f("assumePreferredAngles", "apa", ArgType::Bool, "Set rotate to the preferred angle."))
    .flag(f("setPreferredAngles", "spa", ArgType::Bool, "Set the preferred angle to rotate."))
    .flag(f("orientJoint", "oj", ArgType::String, "Aim axis order, xyz, yzx... or none."))
    .flag(f("secondaryAxisOrient", "sao", ArgType::String, "Where the second axis points, yup, zup, xdown..."))
    .flag(f("children", "ch", ArgType::Bool, "Edit: the joints below too."))
    .flag(f("zeroScaleOrient", "zso", ArgType::Bool, "Set rotateAxis to zero; jointOrient takes the change."))
    .flag(f("exists", "ex", ArgType::String, "Whether a joint of this name exists."))
    .flag(f("edit", "e", ArgType::Bool, "Change existing joints."))
    .flag(f("query", "q", ArgType::Bool, "Query a joint (its world position unless a flag is asked about)."))
    .flag(f("queried", "", ArgType::List, "Flags asked about in query mode."));
    for (axis, k) in [("X", "x"), ("Y", "y"), ("Z", "z")] {
        joint = joint
            .flag(f(&format!("angle{axis}"), &format!("a{k}"), ArgType::Float, "The rotate of this axis."))
            .flag(f(&format!("stiffness{axis}"), &format!("st{k}"), ArgType::Float, "The stiffness of this axis."))
            .flag(f(&format!("limit{axis}"), &format!("l{k}"), ArgType::List, "Lower and upper rotate limit of this axis, degrees.").multi(2))
            .flag(f(&format!("limitSwitch{axis}"), &format!("ls{k}"), ArgType::List, "Whether the lower and the upper limit of this axis apply.").multi(2));
    }
    c.register(joint);

    c.register(
        Cmd::new(
            "orientJoint",
            "Orient joints: the first axis of the order aims at the child joint, the second toward -sao. Joints below follow \
             unless -ch 0.",
            |s, a| {
                let names = named_or_selected(s, a);
                let g = &mut s.graph;
                let order = a.positional.iter().find_map(|v| v.as_str().filter(|x| x.len() == 3 && x.chars().all(|c| "xyz".contains(c)) || *x == "none")).unwrap_or("xyz").to_string();
                let up = a.string("secondaryAxisOrient", "yup");
                for n in names.iter().filter(|n| **n != order) {
                    let id = find(g, n, "orientJoint")?;
                    let mut todo = vec![id];
                    if a.get("children").and_then(Value::as_bool).unwrap_or(true) {
                        todo.extend(g.descendants(id).into_iter().filter(|&d| is_joint(g, d)));
                    }
                    for j in todo {
                        orient_joint(g, j, &order, &up)?;
                        if a.bool("zeroScaleOrient") {
                            zero_scale_orient(g, j)?;
                        }
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("order (xyz), then joints")
        .flag(f("secondaryAxisOrient", "sao", ArgType::String, "yup, zup, xdown..."))
        .flag(f("zeroScaleOrient", "zso", ArgType::Bool, "Set rotateAxis to zero; jointOrient takes the change."))
        .flag(f("children", "ch", ArgType::Bool, "The joints below too (default on).")),
    );

    c.register(
        Cmd::new(
            "mirrorJoint",
            "Mirror a joint and the joints below it across a world plane: a new hierarchy beside the old one. -mb keeps mirrored \
             behaviour (equal rotate values make mirrored motion); without it the new joints keep the old world axes. Returns \
             the new joints.",
            mirror_command,
        )
        .positional("joints")
        .flag(f("mirrorYZ", "myz", ArgType::Bool, "Across the YZ plane."))
        .flag(f("mirrorXY", "mxy", ArgType::Bool, "Across the XY plane."))
        .flag(f("mirrorXZ", "mxz", ArgType::Bool, "Across the XZ plane."))
        .flag(f("mirrorBehavior", "mb", ArgType::Bool, "Mirror the behaviour, not the orientation."))
        .flag(f("searchReplace", "sr", ArgType::List, "Two strings: replace the first with the second in the new names.").multi(2)),
    );

    c.register(
        Cmd::new("insertJoint", "Add a joint under the named or selected joint, at its place; its child joints move under the new one. Returns the new joint.", |s, a| {
            let id = one_joint(s, a, "insertJoint")?;
            let g = &mut s.graph;
            let new = insert_joint(g, id)?;
            Ok(json!(g.path(new)))
        })
        .positional("joint"),
    );

    c.register(
        Cmd::new("removeJoint", "Remove a joint from its skeleton. Its children stay where they are, under its parent.", |s, a| {
            let id = one_joint(s, a, "removeJoint")?;
            remove_joint(&mut s.graph, id)?;
            Ok(Value::Null)
        })
        .positional("joint"),
    );

    c.register(
        Cmd::new(
            "connectJoint",
            "Join two skeletons: the second joint becomes the first one's parent. -pm keeps the first where it is, with a bone \
             between them; -cm (the default) moves the first skeleton's root to the second joint.",
            |s, a| {
                let names = named_or_selected(s, a);
                let g = &mut s.graph;
                let [first, second] = names.as_slice() else { fail!("connectJoint: name or select two joints") };
                let (x, y) = (find(g, first, "connectJoint")?, find(g, second, "connectJoint")?);
                if !is_joint(g, x) || !is_joint(g, y) {
                    fail!("connectJoint: name or select two joints");
                }
                connect_joint(g, x, y, a.bool("parentMode"))?;
                Ok(Value::Null)
            },
        )
        .positional("the joint to connect, then its new parent")
        .flag(f("connectMode", "cm", ArgType::Bool, "Move the first joint to the second."))
        .flag(f("parentMode", "pm", ArgType::Bool, "Keep the first joint where it is.")),
    );

    c.register(
        Cmd::new("disconnectJoint", "Break a skeleton at a joint, deleting handles across the break. Returns the new end joint left on the old parent, then the joint.", |s, a| {
            let id = one_joint(s, a, "disconnectJoint")?;
            let g = &mut s.graph;
            let leaf = disconnect_joint(g, id)?;
            Ok(json!([g.path(leaf), g.path(id)]))
        })
        .positional("joint"),
    );

    c.register(
        Cmd::new("reroot", "Make a joint the root of its skeleton. Every joint stays where it is.", |s, a| {
            let id = one_joint(s, a, "reroot")?;
            reroot(&mut s.graph, id)?;
            Ok(Value::Null)
        })
        .positional("joint"),
    );
}
