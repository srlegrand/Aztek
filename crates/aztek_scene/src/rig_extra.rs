//! Rigging commands the other modules leave out: combinationShape, flexors (flexor, copyFlexor, jointLattice, boneLattice),
//! skeletonEmbed, characterize and the HumanIK helpers, the IK settings, instanceable, effector, relationship,
//! xformConstraint, weightsColor and keying per-point blend shape weights.
//!
//! Differs from the reference. A lattice flexor's node bulges the lattice only: its lattice and base follow the joints through
//! constraints and the ffd runs after the skin, where the reference's node also moves the lattice's two ends rigidly. The bulge's
//! shape is this module's own, since the reference does not document it. Settings with no effect say so in their command's text.

use std::f64::consts::{PI, TAU};
use std::sync::Arc;

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::{DQuat, DVec3, EulerRot};
use serde_json::{json, Value};

use crate::attr_defs::float_attr_short;
use crate::deform::{self, data_attr, matrix_in};
use crate::deform_lattice::{self, grid_of, DLM, DLP};
use crate::graph::{Graph, SelectMode};
use crate::ref_core::f;
use crate::node::{types, vec3_attr, NodeData};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{find, is_joint, plug, rot3, world};
use crate::session::Session;
use crate::skin::asked;
use crate::system_cmds::q_flags;
use crate::transform::Transform;

type Cmd = CommandDef<Session>;
use ArgType::{Any as A, Bool as B, Float as F, Int as I, String as S};

fn truth(v: &Value) -> bool {
    v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0)
}

/// A flag used once or many times, as a flat list.
fn many(v: Option<&Value>) -> Vec<Value> {
    match v {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(l)) => l.iter().flat_map(|x| many(Some(x))).collect(),
        Some(x) => vec![x.clone()],
    }
}

/// The nodes named, or the selection.
fn picked(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let names = objects(s, a);
    names.iter().map(|n| find(&s.graph, n.split('.').next().unwrap_or(n), cmd)).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// Flexor nodes

const FX_IN: usize = 0;
const FX_OUT: usize = 1;
const FX_ENV: usize = 2;
const FX_INSIDE: usize = 3;
const FX_OUTSIDE: usize = 4;
const FX_LEN_IN: usize = 5;
const FX_LEN_OUT: usize = 6;
const FX_LEFT: usize = 7;
const FX_RIGHT: usize = 8;
const FX_UPPER: usize = 9;
const FX_LOWER: usize = 10;
const FX_UPPER_0: usize = 11;
const FX_LOWER_0: usize = 12;
const FX_LATTICE: usize = 13;
const FX_BASE: usize = 14;
const FX_BEND_VECTOR: usize = 15;
const FX_BEND_MAGNITUDE: usize = 16;

pub(crate) fn is_flexor_type(ty: &str) -> bool {
    matches!(ty, "jointLattice" | "boneLattice")
}

/// Moves a lattice's points by how far the lower matrix has turned against the upper since binding.
pub(crate) fn lattice_bulge(d: &mut NodeData) {
        let Some(mut grid) = grid_of(d.value(FX_IN)).cloned() else {
            d.set_output(FX_OUT, PlugValue::Data(None));
            return;
        };
        let (um, lm, iu, il) = (d.matrix(FX_UPPER), d.matrix(FX_LOWER), d.matrix(FX_UPPER_0), d.matrix(FX_LOWER_0));
        let now = DQuat::from_mat3(&rot3(&(um.inverse() * lm)));
        let rest = DQuat::from_mat3(&rot3(&(iu.inverse() * il)));
        let (mut axis, mut angle) = (now * rest.inverse()).normalize().to_axis_angle();
        if angle > PI {
            angle -= TAU;
        }
        if angle < 0.0 {
            axis = -axis;
            angle = -angle;
        }
        let lat = d.matrix(FX_LATTICE);
        let size = DVec3::new(lat.x_axis.truncate().length(), lat.y_axis.truncate().length(), lat.z_axis.truncate().length()).max(DVec3::splat(1e-9));
        // The lattice's own Y runs along the bone. The child bone turns toward axis x Y.
        let bend_axis = rot3(&lat).transpose() * (rot3(&um) * axis);
        let inside = bend_axis.cross(DVec3::Y).normalize_or_zero();
        let side = DVec3::Y.cross(inside);
        let magnitude = angle / PI;
        let env = d.float(FX_ENV);
        let (crease, round, left, right) = (d.float(FX_INSIDE), d.float(FX_OUTSIDE), d.float(FX_LEFT), d.float(FX_RIGHT));
        let (len_in, len_out) = (d.float(FX_LEN_IN), d.float(FX_LEN_OUT));
        for p in &mut grid.pts {
            let radial = DVec3::new(p.x * size.x, 0.0, p.z * size.z).normalize_or_zero();
            let (c, l) = (radial.dot(inside), radial.dot(side));
            let amount = c.max(0.0) * crease + (-c).max(0.0) * round + l.max(0.0) * left + (-l).max(0.0) * right;
            let along = (PI * p.y.clamp(-0.5, 0.5)).cos().powi(2);
            let mut moved = radial * amount * magnitude * along;
            // Away from the joint along the bones, more toward the lattice's ends.
            moved.y += 2.0 * p.y * if p.y < 0.0 { len_in } else { len_out };
            *p += moved * env / size;
        }
        d.set_output(FX_OUT, PlugValue::Data(Some(Arc::new(grid))));
        d.set_output(FX_BEND_VECTOR, PlugValue::Vec3(axis));
        d.set_output(FX_BEND_MAGNITUDE, PlugValue::Float(magnitude));
}

fn distance(name: &str, short: &str, doc: &str) -> AttrDef {
    let mut x = float_attr_short(name, short, 0.0, doc, None, None);
    x.flags |= flags::KEYABLE;
    x
}

pub(crate) fn flexor_attrs(inside: (&str, &str), outside: (&str, &str)) -> Vec<AttrDef> {
    vec![
        data_attr("inputGeometry", "ig", "The lattice's points as they come."),
        AttrDef { name: "outputGeometry".into(), short_name: "og".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::OUTPUT | flags::HIDDEN, doc: "The lattice's points, moved.".into(), ..Default::default() },
        float_attr_short("envelope", "en", 1.0, "How much of the bulge applies.", None, None),
        distance(inside.0, inside.1, "Bulge of the lattice points on the inside of the bend; positive is outwards."),
        distance(outside.0, outside.1, "Bulge of the lattice points on the outside of the bend; positive is outwards."),
        distance("lengthIn", "li", "Moves the points on the upper half along the bone; positive is away from the middle."),
        distance("lengthOut", "lo", "Moves the points on the lower half along the bone; positive is away from the middle."),
        distance("widthLeft", "wl", "Bulge on the left side of the bend; positive is outwards."),
        distance("widthRight", "wr", "Bulge on the right side of the bend; positive is outwards."),
        matrix_in("upperMatrix", "um", "The upper transform's world matrix."),
        matrix_in("lowerMatrix", "lm", "The lower transform's world matrix."),
        // Saved with the node: the bend is measured from these.
        AttrDef { flags: flags::INPUT | flags::STORABLE, ..matrix_in("initialUpperMatrix", "iu", "upperMatrix when the node was made.") },
        AttrDef { flags: flags::INPUT | flags::STORABLE, ..matrix_in("initialLowerMatrix", "il", "lowerMatrix when the node was made.") },
        matrix_in("deformedLatticeMatrix", "md", "The lattice's world matrix."),
        matrix_in("baseLatticeMatrix", "mb", "The base lattice's world matrix."),
        vec3_attr("bendVector", "bv", DVec3::ZERO, flags::OUTPUT, "The axis of the bend, in the upper transform's space."),
        AttrDef { name: "bendMagnitude".into(), short_name: "bm".into(), ty: PlugType::Float, default_value: PlugValue::Float(0.0), flags: flags::OUTPUT, doc: "The bend's angle over 180 degrees.".into(), ..Default::default() },
    ]
}

const JOINT_SETTINGS: &[&str] = &["creasing", "rounding", "lengthIn", "lengthOut", "widthLeft", "widthRight"];
const BONE_SETTINGS: &[&str] = &["bicep", "tricep", "lengthIn", "lengthOut", "widthLeft", "widthRight"];

fn settings_of(ty: &str) -> &'static [&'static str] {
    if ty == "jointLattice" {
        JOINT_SETTINGS
    } else {
        BONE_SETTINGS
    }
}

/// Points a matrix input at a transform and records where it is now.
fn drive(g: &mut Graph, n: NodeId, attr: usize, rest: usize, by: Option<NodeId>) -> CmdResult<()> {
    let Some(by) = by else { return Ok(()) };
    let m = world(g, by);
    g.set_value(plug(n, rest), PlugValue::Matrix(m))?;
    g.connect(&plug(by, Transform::WORLD_MATRIX), &plug(n, attr), true)?;
    Ok(())
}

/// A flexor node on an ffd: between its lattice's points and the ffd. Returns the node.
fn insert_flexor(g: &mut Graph, ty: &str, ffd: NodeId, name: &str, upper: Option<NodeId>, lower: Option<NodeId>) -> CmdResult<NodeId> {
    let Some(feed) = g.source(&plug(ffd, DLP)) else { fail!("{ty}: the lattice's points are not connected") };
    let t = types::find(ty).unwrap();
    let n = g.create_node(&t, name, NO_NODE, true)?;
    g.disconnect(&plug(ffd, DLP));
    g.connect(&feed, &plug(n, FX_IN), false)?;
    g.connect(&plug(n, FX_OUT), &plug(ffd, DLP), false)?;
    let (lat, base) = (g.source(&plug(ffd, DLM)), g.source(&plug(ffd, deform_lattice::BLM)));
    if let Some(x) = lat {
        g.connect(&x, &plug(n, FX_LATTICE), false)?;
    }
    if let Some(x) = base {
        g.connect(&x, &plug(n, FX_BASE), false)?;
    }
    drive(g, n, FX_UPPER, FX_UPPER_0, upper)?;
    drive(g, n, FX_LOWER, FX_LOWER_0, lower)?;
    Ok(n)
}

fn joint_parent(g: &Graph, j: NodeId) -> Option<NodeId> {
    let p = g.parent(j);
    (p != NO_NODE && is_joint(g, p)).then_some(p)
}

fn joint_child(g: &Graph, j: NodeId) -> Option<NodeId> {
    g.children(j).iter().copied().find(|&c| is_joint(g, c))
}

fn opt_node(g: &Graph, a: &Args, flag: &str, cmd: &str) -> CmdResult<Option<NodeId>> {
    match a.get(flag).and_then(Value::as_str) {
        Some(n) => Ok(Some(find(g, n, cmd)?)),
        None => Ok(None),
    }
}

/// jointLattice and boneLattice: on the lattice named or selected; -q and -e on the node.
fn flexor_node_cmd(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    let names = objects(s, a);
    let Some(first) = names.first() else { fail!("{ty}: name a lattice, or the {ty} with -q or -e") };
    let settings = settings_of(ty);
    if a.bool("query") || a.bool("edit") {
        let n = find(&s.graph, first, ty)?;
        if !s.graph.node(n).is_some_and(|x| x.ty().name == ty) {
            fail!("{ty}: {first} is not a {ty}");
        }
        let g = &mut s.graph;
        if a.bool("query") {
            if asked(a, "joint") {
                return Ok(g.source(&plug(n, FX_LOWER)).map_or(Value::Null, |p| json!(g.path(p.node))));
            }
            if asked(a, "geometry") {
                let Some(ffd) = g.nodes().into_iter().find(|&d| g.source(&plug(d, DLP)).is_some_and(|p| p.node == n)) else { return Ok(json!([])) };
                return Ok(json!(deform::geometry_names(g, ffd)));
            }
            return deform::query_flags(g, n, a, settings);
        }
        deform::apply_flags(g, n, a, settings)?;
        return Ok(json!(g.path(n)));
    }
    let g = &mut s.graph;
    let ffd = deform_lattice::ffd_named(g, first)?;
    let joint = opt_node(g, a, "joint", ty)?;
    let upper = if ty == "jointLattice" { opt_node(g, a, "upperTransform", ty)? } else { opt_node(g, a, "transform", ty)? };
    let upper = upper.or_else(|| joint.and_then(|j| joint_parent(g, j)));
    let lower = if ty == "jointLattice" { opt_node(g, a, "lowerTransform", ty)?.or(joint) } else { joint };
    let name = a.string("name", &format!("{ty}1"));
    let n = insert_flexor(g, ty, ffd, &name, upper, lower)?;
    deform::apply_flags(g, n, a, settings)?;
    Ok(json!(g.path(n)))
}

fn flexor_node_def(ty: &'static str, doc: &'static str) -> Cmd {
    let c = q_flags(Cmd::new(ty, doc, move |s, a| flexor_node_cmd(s, a, ty)))
        .positional("the lattice (the selection when none); the node with -q and -e")
        .flag(f("name", "n", S, "Name of the node."))
        .flag(f("joint", "j", S, "The joint whose bend drives the bulge."))
        .flag(f("geometry", "g", A, "With -q: what the lattice deforms."))
        .flag(f("lengthIn", "li", F, "Moves the points on the upper half along the bone."))
        .flag(f("lengthOut", "lo", F, "Moves the points on the lower half along the bone."))
        .flag(f("widthLeft", "wl", F, "Bulge on the left of the bend."))
        .flag(f("widthRight", "wr", F, "Bulge on the right of the bend."));
    if ty == "jointLattice" {
        c.flag(f("creasing", "cr", F, "Bulge on the inside of the bend."))
            .flag(f("rounding", "ro", F, "Bulge on the outside of the bend."))
            .flag(f("upperTransform", "ut", S, "The transform of the upper part: the joint's parent by default."))
            .flag(f("lowerTransform", "lt", S, "The transform of the lower part: the joint by default."))
            .flag(f("upperBindSkin", "ub", S, "Accepted for existing scripts; has no effect."))
            .flag(f("lowerBindSkin", "lb", S, "Accepted for existing scripts; has no effect."))
    } else {
        c.flag(f("bicep", "bi", F, "Bulge on the inside of the bend."))
            .flag(f("tricep", "tr", F, "Bulge on the outside of the bend."))
            .flag(f("transform", "t", S, "The transform the bend is measured against: the joint's parent by default."))
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// flexor and copyFlexor

const FLEXOR_TYPES: &[&str] = &["jointLattice", "boneLattice", "sculpt"];

/// Meshes bound to a joint.
fn bound_meshes(g: &Graph, joint: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    for id in g.nodes() {
        if !g.node(id).is_some_and(|n| n.ty().is_a("skinCluster")) {
            continue;
        }
        if crate::skin::influences(g, id).iter().any(|&(_, j)| j == joint) {
            if let Some(m) = crate::skin::skin_geometry(g, id) {
                if !out.contains(&m) {
                    out.push(m);
                }
            }
        }
    }
    out
}

/// The widest a mesh gets around a line, within the line's length: how big a lattice round it has to be.
fn width_around(g: &mut Graph, meshes: &[NodeId], from: DVec3, to: DVec3) -> f64 {
    let (dir, len) = ((to - from).normalize_or_zero(), from.distance(to));
    let mut most = 0f64;
    for &mesh in meshes {
        let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
        let w = world(g, crate::dag::transform_of(g, mesh));
        for p in (0..m.point_capacity()).filter(|&p| m.point_valid(p)) {
            let q = w.transform_point3(m.position(p)) - from;
            let along = q.dot(dir);
            if (0.0..=len).contains(&along) {
                most = most.max((q - dir * along).length());
            }
        }
    }
    most
}

/// The numbers after a flag in a deformer command string: -dv 4 6 4.
fn dc_numbers(dc: &str, flag: &str, count: usize) -> Option<Vec<f64>> {
    let toks: Vec<&str> = dc.split_whitespace().collect();
    let at = toks.iter().position(|t| *t == flag)?;
    let v: Vec<f64> = toks.get(at + 1..at + 1 + count)?.iter().filter_map(|t| t.parse().ok()).collect();
    (v.len() == count).then_some(v)
}

struct FlexorPlan {
    ty: String,
    /// The ends of the lattice's long axis.
    from: DVec3,
    to: DVec3,
    upper: Option<NodeId>,
    lower: Option<NodeId>,
}

fn plan_flexor(g: &mut Graph, ty: &str, joint: NodeId, at_bones: bool) -> Option<FlexorPlan> {
    let pos = |g: &mut Graph, j: NodeId| world(g, j).w_axis.truncate();
    if at_bones {
        let c = joint_child(g, joint)?;
        let (from, to) = (pos(g, joint), pos(g, c));
        return Some(FlexorPlan { ty: ty.into(), from, to, upper: Some(joint), lower: Some(c) });
    }
    let (p, c) = (joint_parent(g, joint)?, joint_child(g, joint)?);
    let (from, to) = (pos(g, p), pos(g, c));
    Some(FlexorPlan { ty: ty.into(), from, to, upper: Some(p), lower: Some(joint) })
}

/// Makes a lattice flexor. Returns the flexor node and the ffd.
fn make_lattice_flexor(s: &mut Session, plan: &FlexorPlan, meshes: &[NodeId], dims: [f64; 3], no_scale: bool) -> CmdResult<(NodeId, NodeId)> {
    let g = &mut s.graph;
    g.evaluate();
    let dir = (plan.to - plan.from).normalize_or_zero();
    let len = plan.from.distance(plan.to).max(1e-3);
    let measured = if no_scale { 0.0 } else { width_around(g, meshes, plan.from, plan.to) };
    let across = if measured > 1e-6 { measured * 2.2 } else { len * 0.5 };
    let (rz, ry, rx) = DQuat::from_rotation_arc(DVec3::Y, dir).to_euler(EulerRot::ZYX);
    let centre = (plan.from + plan.to) * 0.5;
    let names: Vec<String> = meshes.iter().map(|&m| g.path(m)).collect();
    let made = s.run(
        "lattice",
        json!({"args": names, "divisions": dims, "position": centre.to_array(), "scale": [across, len, across], "rotation": [rx.to_degrees(), ry.to_degrees(), rz.to_degrees()]}),
    )?;
    let list = made.as_array().cloned().unwrap_or_default();
    let (Some(ffd), Some(lat), Some(base)) = (list.first().and_then(Value::as_str), list.get(1).and_then(Value::as_str), list.get(2).and_then(Value::as_str)) else {
        fail!("flexor: lattice gave no nodes")
    };
    let (ffd, lat, base) = (ffd.to_string(), lat.to_string(), base.to_string());
    let upper_name = plan.upper.map(|u| s.graph.path(u));
    let lower_name = plan.lower.map(|l| s.graph.path(l));
    match (&upper_name, &lower_name) {
        (Some(u), Some(l)) if plan.ty == "jointLattice" => {
            // The lattice stays halfway between the two bones.
            for t in [&lat, &base] {
                s.run("pointConstraint", json!({"args": [l, t], "maintainOffset": true}))?;
                s.run("orientConstraint", json!({"args": [u, l, t], "maintainOffset": true}))?;
            }
        }
        (Some(u), _) => {
            s.run("parent", json!({"args": [lat, base, u]}))?;
        }
        _ => {}
    }
    let g = &mut s.graph;
    let ffd_id = find(g, &ffd, "flexor")?;
    let name = format!("{}1", plan.ty);
    let n = insert_flexor(g, &plan.ty, ffd_id, &name, plan.upper, plan.lower)?;
    Ok((n, ffd_id))
}

fn flexor_cmd(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        if asked(a, "list") {
            return Ok(json!(FLEXOR_TYPES));
        }
        fail!("flexor: only -list can be queried");
    }
    let ty = a.string("type", "jointLattice");
    if !FLEXOR_TYPES.contains(&ty.as_str()) {
        fail!("flexor: no flexor type {ty}. flexor -q -list gives them.");
    }
    let at_bones = a.bool("atBones");
    if at_bones == a.bool("atJoints") && at_bones {
        fail!("flexor: use one of -atBones and -atJoints");
    }
    let nodes = picked(s, a, "flexor")?;
    let g = &mut s.graph;
    let chosen: Vec<NodeId> = nodes.iter().copied().filter(|&n| is_joint(g, n)).collect();
    let others: Vec<NodeId> = nodes.iter().copied().filter(|&n| !is_joint(g, n)).filter_map(|n| crate::components::as_mesh(g, n)).collect();
    if chosen.is_empty() {
        fail!("flexor: select joints");
    }
    let to_skeleton = a.bool("toSkeleton");
    let mut joints = Vec::new();
    for j in chosen.iter().copied() {
        joints.push(j);
        if to_skeleton {
            joints.extend(g.descendants(j).into_iter().filter(|&k| is_joint(g, k)));
        }
    }
    joints.dedup();
    let dc = a.string("deformerCommand", "");
    let dims = dc_numbers(&dc, "-dv", 3).or_else(|| dc_numbers(&dc, "-divisions", 3)).map_or([2.0, 5.0, 2.0], |v| [v[0], v[1], v[2]]);
    let no_scale = a.bool("noScale");
    let mut out = Vec::new();
    for j in joints {
        let g = &mut s.graph;
        let Some(plan) = plan_flexor(g, &ty, j, at_bones) else {
            if to_skeleton {
                continue;
            }
            let what = if at_bones { "a child joint" } else { "a parent and a child joint" };
            fail!("flexor: {} needs {what}", g.path(j));
        };
        let mut meshes = others.clone();
        for m in bound_meshes(g, j) {
            if !meshes.contains(&m) {
                meshes.push(m);
            }
        }
        if let Some(l) = plan.lower {
            for m in bound_meshes(g, l) {
                if !meshes.contains(&m) {
                    meshes.push(m);
                }
            }
        }
        if meshes.is_empty() {
            fail!("flexor: no geometry is bound to {}; bind it or name the meshes", g.path(j));
        }
        if ty == "sculpt" {
            let names: Vec<String> = meshes.iter().map(|&m| g.path(m)).collect();
            let mut call = json!({"args": names});
            if let Some(v) = dc_numbers(&dc, "-mxd", 1).or_else(|| dc_numbers(&dc, "-maxDisplacement", 1)) {
                call["maxDisplacement"] = json!(v[0]);
            }
            let made = s.run("sculpt", call)?;
            let list = made.as_array().cloned().unwrap_or_default();
            let (Some(n), Some(tool)) = (list.first().and_then(Value::as_str), list.get(1).and_then(Value::as_str)) else { fail!("flexor: sculpt gave no nodes") };
            let g = &mut s.graph;
            let at = world(g, j).w_axis.truncate();
            let (n, tool) = (n.to_string(), tool.to_string());
            s.run("xform", json!({"args": [tool], "worldSpace": true, "translation": at.to_array()}))?;
            let path = s.graph.path(j);
            s.run("parent", json!({"args": [tool, path]}))?;
            out.push(json!(n));
            continue;
        }
        let (n, _) = make_lattice_flexor(s, &plan, &meshes, dims, no_scale)?;
        out.push(json!(s.graph.path(n)));
    }
    Ok(Value::Array(out))
}

fn copy_flexor_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let [lattice, dest] = names.as_slice() else { fail!("copyFlexor: give the flexor's lattice and the joint to copy to") };
    let g = &mut s.graph;
    let ffd = deform_lattice::ffd_named(g, lattice)?;
    let dest = find(g, dest, "copyFlexor")?;
    if !is_joint(g, dest) {
        fail!("copyFlexor: {} is not a joint", g.path(dest));
    }
    let Some(flex) = g.source(&plug(ffd, DLP)).map(|p| p.node).filter(|&n| g.node(n).is_some_and(|x| is_flexor_type(x.ty().name.as_str()))) else {
        fail!("copyFlexor: {lattice} is not a flexor's lattice")
    };
    let ty = g.node(flex).unwrap().ty().name.to_string();
    let Some((shape, _, _)) = deform_lattice::parts(g, ffd) else { fail!("copyFlexor: {lattice} has no lattice") };
    let Some(plan) = plan_flexor(g, &ty, dest, ty == "boneLattice") else { fail!("copyFlexor: {} has no bone to carry the flexor", g.path(dest)) };
    let mut meshes = bound_meshes(g, dest);
    if meshes.is_empty() {
        let names = deform::geometry_names(g, ffd);
        meshes = names.iter().filter_map(|n| g.find(n).ok().flatten()).filter_map(|n| crate::components::as_mesh(g, n)).collect();
    }
    if meshes.is_empty() {
        fail!("copyFlexor: no geometry to deform");
    }
    let dims = deform_lattice::current_grid(g, shape).map_or([2.0, 5.0, 2.0], |x| [x.dims[0] as f64, x.dims[1] as f64, x.dims[2] as f64]);
    let tweaks = g.node(shape).map(|n| n.data().value(deform_lattice::CONTROL_POINTS).clone());
    let (n, new_ffd) = make_lattice_flexor(s, &plan, &meshes, dims, false)?;
    let g = &mut s.graph;
    for name in settings_of(&ty).iter().chain(["envelope"].iter()) {
        let at = g.node(n).and_then(|x| x.ty().attr_index(name));
        if let Some(i) = at {
            let v = g.value(&plug(flex, i))?;
            g.set_value(plug(n, i), v)?;
        }
    }
    if let (Some(t), Some((new_shape, _, _))) = (tweaks, deform_lattice::parts(g, new_ffd)) {
        g.set_value(plug(new_shape, deform_lattice::CONTROL_POINTS), t)?;
    }
    Ok(json!(g.path(n)))
}

// ---------------------------------------------------------------------------------------------------------------------
// combinationShape

fn target_index(g: &Graph, bs: NodeId, a: &Args, index_flag: &str, name_flag: &str) -> CmdResult<Vec<i32>> {
    let mut out: Vec<i32> = many(a.get(index_flag)).iter().filter_map(|v| v.as_i64()).map(|i| i as i32).collect();
    let data = crate::deform_blend::blend_of(g, bs);
    for v in many(a.get(name_flag)) {
        let Some(n) = v.as_str() else { continue };
        match data.targets.iter().find(|t| t.name == n) {
            Some(t) => out.push(t.index),
            None => fail!("combinationShape: no target {n} on {}", g.path(bs)),
        }
    }
    Ok(out)
}

fn weight_plug(bs: NodeId, index: i32) -> PlugRef {
    PlugRef::element(bs, crate::deform_blend::WEIGHT as i32, index)
}

fn combination_on(g: &Graph, bs: NodeId, index: i32) -> Option<NodeId> {
    g.source(&weight_plug(bs, index)).map(|p| p.node).filter(|&n| g.node(n).is_some_and(|x| x.ty().name == "combinationShape"))
}

/// The driver plugs of a combination node: (input element, blend shape target).
fn drivers_of(g: &Graph, cs: NodeId) -> Vec<(i32, i32)> {
    let Some(iw) = g.node(cs).and_then(|n| n.ty().attr_index("inputWeight")) else { return Vec::new() };
    let mut out: Vec<(i32, i32)> = g.connections().into_iter().filter(|(d, _)| d.node == cs && d.attr == iw as i32).map(|(d, s)| (d.element, s.element)).collect();
    out.sort();
    out
}

fn combination_shape(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let Some(bs_name) = a.get("blendShape").and_then(Value::as_str) else { fail!("combinationShape: give -blendShape") };
    let bs = deform::leader_of(g, crate::deform_blend::blend_named(g, bs_name, "combinationShape")?);
    let targets = target_index(g, bs, a, "combinationTargetIndex", "combinationTargetName")?;
    let driver_ids = target_index(g, bs, a, "driverTargetIndex", "driverTargetName")?;
    let target = targets.first().copied();
    let existing = target.and_then(|t| combination_on(g, bs, t));
    if a.bool("query") {
        if asked(a, "exist") {
            return Ok(json!(existing.is_some()));
        }
        let Some(cs) = existing else { fail!("combinationShape: that target has no combination shape") };
        if asked(a, "allDrivers") {
            return Ok(json!(drivers_of(g, cs).iter().map(|x| x.1).collect::<Vec<_>>()));
        }
        if asked(a, "combineMethod") {
            return Ok(crate::plug::to_json(&g.value(&plug(cs, g.node(cs).unwrap().ty().attr_index("combinationMethod").unwrap()))?));
        }
        fail!("combinationShape: ask for -exist, -allDrivers or -combineMethod");
    }
    let Some(target) = target else { fail!("combinationShape: give -combinationTargetIndex or -combinationTargetName") };
    let cm = a.get("combineMethod").and_then(Value::as_i64);
    if a.bool("edit") {
        let Some(cs) = existing else { fail!("combinationShape: target {target} has no combination shape") };
        let iw = g.node(cs).unwrap().ty().attr_index("inputWeight").unwrap();
        if a.bool("removeDriver") {
            let all = a.bool("allDrivers");
            for (element, driver) in drivers_of(g, cs) {
                if all || driver_ids.contains(&driver) {
                    g.disconnect(&PlugRef::element(cs, iw as i32, element));
                }
            }
            if drivers_of(g, cs).is_empty() {
                g.disconnect(&weight_plug(bs, target));
                g.delete_node(cs)?;
                return Ok(Value::Null);
            }
        }
        if a.bool("addDriver") {
            let mut next = drivers_of(g, cs).iter().map(|x| x.0 + 1).max().unwrap_or(0);
            for d in &driver_ids {
                g.connect(&weight_plug(bs, *d), &PlugRef::element(cs, iw as i32, next), false)?;
                next += 1;
            }
        }
        if let Some(m) = cm {
            let at = g.node(cs).unwrap().ty().attr_index("combinationMethod").unwrap();
            g.set_value(plug(cs, at), PlugValue::Int(m))?;
        }
        return Ok(json!(g.path(cs)));
    }
    if existing.is_some() {
        fail!("combinationShape: target {target} already has one; use -edit");
    }
    if driver_ids.is_empty() {
        fail!("combinationShape: give drivers with -driverTargetIndex or -driverTargetName");
    }
    if driver_ids.contains(&target) {
        fail!("combinationShape: target {target} can't drive itself");
    }
    let t = types::find("combinationShape").unwrap();
    let cs = g.create_node(&t, "combinationShape1", NO_NODE, true)?;
    let ty = g.node(cs).unwrap().ty();
    let (iw, ow, method) = (ty.attr_index("inputWeight").unwrap(), ty.attr_index("outputWeight").unwrap(), ty.attr_index("combinationMethod").unwrap());
    for (k, d) in driver_ids.iter().enumerate() {
        g.connect(&weight_plug(bs, *d), &PlugRef::element(cs, iw as i32, k as i32), false)?;
    }
    if let Some(m) = cm {
        g.set_value(plug(cs, method), PlugValue::Int(m))?;
    }
    g.connect(&plug(cs, ow), &weight_plug(bs, target), true)?;
    Ok(json!(target))
}

// ---------------------------------------------------------------------------------------------------------------------
// skeletonEmbed

fn world_points(g: &mut Graph, meshes: &[NodeId]) -> Vec<DVec3> {
    let mut out = Vec::new();
    for &mesh in meshes {
        let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
        let w = world(g, crate::dag::transform_of(g, mesh));
        out.extend((0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| w.transform_point3(m.position(p))));
    }
    out
}

/// A chain along the bounds' longest axis, through the middle of evenly spaced slices of the points.
fn medial_chain(points: &[DVec3], count: usize) -> Vec<DVec3> {
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for p in points {
        lo = lo.min(*p);
        hi = hi.max(*p);
    }
    let size = hi - lo;
    let axis = if size.y >= size.x && size.y >= size.z {
        1
    } else if size.x >= size.z {
        0
    } else {
        2
    };
    let (start, length) = (lo[axis], size[axis].max(1e-9));
    let mut sums = vec![(DVec3::ZERO, 0usize); count];
    for p in points {
        let k = (((p[axis] - start) / length * count as f64) as usize).min(count - 1);
        sums[k].0 += *p;
        sums[k].1 += 1;
    }
    let middle = (lo + hi) * 0.5;
    (0..count)
        .map(|k| {
            let at = start + length * k as f64 / (count - 1).max(1) as f64;
            let mut q = if sums[k].1 > 0 { sums[k].0 / sums[k].1 as f64 } else { middle };
            q[axis] = at;
            q
        })
        .collect()
}

fn skeleton_embed(s: &mut Session, a: &Args) -> CmdResult {
    let nodes = picked(s, a, "skeletonEmbed")?;
    let g = &mut s.graph;
    let meshes: Vec<NodeId> = nodes.iter().filter_map(|&n| crate::components::as_mesh(g, n)).collect();
    if meshes.is_empty() {
        fail!("skeletonEmbed: name or select meshes");
    }
    g.evaluate();
    let points = world_points(g, &meshes);
    if points.is_empty() {
        fail!("skeletonEmbed: the meshes have no points");
    }
    if a.bool("query") && asked(a, "mergedMesh") {
        let mut faces = Vec::new();
        let mut offset = 0u32;
        for &mesh in &meshes {
            let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
            let mut remap = vec![0u32; m.point_capacity() as usize];
            let mut k = 0;
            for p in (0..m.point_capacity()).filter(|&p| m.point_valid(p)) {
                remap[p as usize] = offset + k;
                k += 1;
            }
            for fc in (0..m.face_capacity()).filter(|&x| m.face_valid(x)) {
                faces.push(m.face_points(fc).into_iter().map(|p| remap[p as usize]).collect::<Vec<_>>());
            }
            offset += k;
        }
        return Ok(json!({"points": points.iter().map(|p| p.to_array()).collect::<Vec<_>>(), "faces": faces}));
    }
    let count = a.int("jointCount", 5).clamp(2, 64) as usize;
    let chain = medial_chain(&points, count);
    if a.bool("query") {
        return Ok(json!(chain.iter().map(|p| p.to_array()).collect::<Vec<_>>()));
    }
    let keep: Vec<NodeId> = s.graph.selection().to_vec();
    s.graph.select(&[], SelectMode::Replace);
    let mut out = Vec::new();
    for p in &chain {
        out.push(s.run("joint", json!({"position": p.to_array()}))?);
    }
    s.graph.select(&keep, SelectMode::Replace);
    Ok(Value::Array(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// characterize and the HumanIK helpers

const FBIK_NOT_HERE: &[&str] = &["activatePivot", "addAuxEffector", "addFloorContactPlane", "addMissingEffectors", "changePivotPlacement", "placeNewPivot", "effectors", "fkSkeleton", "stancePose", "attributeFromHIKProperty", "attributeFromHIKPropertyMode"];

fn characterize(s: &mut Session, a: &Args) -> CmdResult {
    for fl in FBIK_NOT_HERE {
        if a.has(fl) {
            fail!("characterize: -{fl} needs the full body solver, which is not here");
        }
    }
    for fl in ["posture", "autoActivateBodyPart", "pinHandFeet"] {
        if a.bool("query") && asked(a, fl) {
            return Ok(s.extras.more.characterize.get(fl).cloned().unwrap_or_else(|| if fl == "posture" { json!("biped") } else { json!(fl == "autoActivateBodyPart") }));
        }
    }
    if a.bool("query") {
        fail!("characterize: ask for -posture, -autoActivateBodyPart or -pinHandFeet");
    }
    for fl in ["autoActivateBodyPart", "pinHandFeet"] {
        if let Some(v) = a.get(fl) {
            s.extras.more.characterize.insert(fl.to_string(), json!(truth(v)));
        }
    }
    let posture = a.string("posture", "biped");
    if !matches!(posture.as_str(), "biped" | "quadruped") {
        fail!("characterize: posture {posture} is not biped or quadruped");
    }
    s.extras.more.characterize.insert("posture".into(), json!(posture));
    let roots = objects(s, a);
    let map = a.string("sourceSkeleton", "");
    let pairs: Vec<(String, String)> = map.split(',').filter_map(|e| {
        let mut t = e.split_whitespace();
        Some((t.next()?.to_string(), t.next()?.to_string()))
    }).collect();
    if roots.is_empty() && pairs.is_empty() {
        fail!("characterize: select a skeleton, or give -sourceSkeleton");
    }
    let name = a.string("name", "Character1");
    let c = s.run("hikCreateCharacter", json!({"args": roots, "name": name}))?;
    let Some(cn) = c.as_str().map(str::to_string) else { fail!("characterize: no character made") };
    for (joint, slot) in pairs {
        s.run("setCharacterObject", json!({"args": [joint, cn, slot, 0]}))?;
    }
    Ok(json!(cn))
}

/// What a character depends on: its joints, then nodes connected to it.
fn hik_depend_nodes(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = objects(s, a).into_iter().next() else { fail!("hikGetDependNodes: name a character") };
    let g = &s.graph;
    let c = crate::hik::character(g, &name, "hikGetDependNodes")?;
    let mut out: Vec<String> = crate::hik::slots_of(g, c).into_values().collect();
    for (d, src) in g.connections() {
        let other = if d.node == c {
            src.node
        } else if src.node == c {
            d.node
        } else {
            continue;
        };
        let p = g.path(other);
        if !out.contains(&p) {
            out.push(p);
        }
    }
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// Small commands

fn ik_system_info(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let sys = crate::ik::ensure_system(g)?;
    let at = g.node(sys).unwrap().ty().attr_index("globalSnap").unwrap();
    if a.bool("query") {
        return Ok(crate::plug::to_json(&g.value(&plug(sys, at))?));
    }
    if let Some(v) = a.get("globalSnapHandle") {
        g.set_value(plug(sys, at), PlugValue::Bool(truth(v)))?;
    }
    Ok(Value::Null)
}

fn effector_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let nodes = picked(s, a, "effector")?;
    let g = &mut s.graph;
    let Some(&e) = nodes.iter().find(|&&n| g.node(n).is_some_and(|x| x.ty().is_a("ikEffector"))) else { fail!("effector: name or select an effector") };
    let hide = g.node(e).unwrap().ty().attr_index("hideDisplay").unwrap();
    if a.bool("query") {
        if asked(a, "hide") {
            return Ok(crate::plug::to_json(&g.value(&plug(e, hide))?));
        }
        if asked(a, "name") {
            return Ok(json!(g.node(e).unwrap().name()));
        }
        fail!("effector: ask for -name or -hide");
    }
    if let Some(v) = a.get("hide") {
        g.set_value(plug(e, hide), PlugValue::Bool(truth(v)))?;
    }
    if let Some(n) = a.get("name").and_then(Value::as_str) {
        g.rename(e, n)?;
    }
    Ok(json!(g.path(e)))
}

const NOT_INSTANCEABLE: &str = "aztekNotInstanceable";

fn is_instanceable(g: &Graph, n: NodeId) -> bool {
    let Some(node) = g.node(n) else { return true };
    match node.ty().attr_index(NOT_INSTANCEABLE) {
        Some(i) => !matches!(node.data().value(i), PlugValue::Bool(true)),
        None => true,
    }
}

fn set_instanceable(g: &mut Graph, n: NodeId, allow: bool) -> CmdResult<()> {
    let at = match g.node(n).and_then(|x| x.ty().attr_index(NOT_INSTANCEABLE)) {
        Some(i) => i,
        None if allow => return Ok(()),
        None => g.add_attr(n, AttrDef { name: NOT_INSTANCEABLE.into(), short_name: "azni".into(), ty: PlugType::Bool, default_value: PlugValue::Bool(false), flags: flags::INPUT | flags::STORABLE | flags::HIDDEN, doc: "Set by instanceable -allow false.".into(), ..Default::default() })?,
    };
    g.set_value(plug(n, at), PlugValue::Bool(!allow))?;
    Ok(())
}

fn instanceable(s: &mut Session, a: &Args) -> CmdResult {
    let nodes = picked(s, a, "instanceable")?;
    let g = &mut s.graph;
    if a.bool("query") {
        return Ok(json!(nodes.iter().map(|&n| is_instanceable(g, n)).collect::<Vec<_>>()));
    }
    let Some(allow) = a.get("allow").map(truth) else { fail!("instanceable: give -allow") };
    let mut out = Vec::new();
    let mut todo = nodes;
    let mut i = 0;
    while i < todo.len() {
        let n = todo[i];
        i += 1;
        set_instanceable(g, n, allow)?;
        out.push(allow);
        for &k in g.children(n).to_vec().iter() {
            let shape = crate::dag::is_shape(g, k);
            if (shape && a.bool("shape")) || (!shape && a.bool("recursive")) {
                todo.push(k);
            }
        }
    }
    Ok(json!(out))
}

/// Refuses what `instance` is handed when it or anything below it is flagged.
fn check_instanceable(s: &Session, a: &Args) -> CmdResult<()> {
    let g = &s.graph;
    for name in objects(s, a) {
        let Ok(Some(n)) = g.find(&name) else { continue };
        if !is_instanceable(g, n) {
            fail!("'{}' is non-instanceable and cannot be instanced.", g.path(n));
        }
        if g.descendants(n).iter().any(|&k| !is_instanceable(g, k)) {
            fail!("'{}' has non-instanceable children thus it cannot be instanced.", g.path(n));
        }
    }
    Ok(())
}

fn relationship(s: &mut Session, a: &Args) -> CmdResult {
    let names = a.named();
    let more = &mut s.extras.more;
    if a.bool("query") {
        let (Some(ty), Some(owner)) = (names.first(), names.get(1)) else { fail!("relationship: give the type and the owner") };
        let found: Vec<&Vec<String>> = more.relationships.iter().filter(|r| r.0 == *ty && r.1 == *owner).map(|r| &r.2).collect();
        return Ok(json!(found.into_iter().flatten().collect::<Vec<_>>()));
    }
    let [ty, owner, members @ ..] = names.as_slice() else { fail!("relationship: give the type, the owner, then what is related") };
    let members = members.to_vec();
    if a.bool("b") {
        more.relationships.retain(|r| !(r.0 == *ty && r.1 == *owner && r.2 == members));
    } else {
        more.relationships.push((ty.clone(), owner.clone(), members, a.string("relationshipData", "")));
    }
    Ok(Value::Null)
}

fn xform_constraint(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        if asked(a, "alongNormal") {
            return Ok(json!(s.extras.more.xform_along_normal));
        }
        if asked(a, "live") {
            return Ok(json!(!s.live_surface.is_empty()));
        }
        return Ok(json!(if s.extras.more.xform_type.is_empty() { "none" } else { s.extras.more.xform_type.as_str() }));
    }
    if let Some(t) = a.get("type").and_then(Value::as_str) {
        if !matches!(t, "none" | "surface" | "edge" | "live") {
            fail!("xformConstraint: type {t} is not none, surface, edge or live");
        }
        s.extras.more.xform_type = t.to_string();
        s.graph.move_opts.constraint = if matches!(t, "edge" | "surface") { t.to_string() } else { String::new() };
    }
    if a.has("alongNormal") {
        s.extras.more.xform_along_normal = a.int("alongNormal", 0);
    }
    Ok(Value::Null)
}

/// A store of named settings for a command with no effect yet: the flags given replace what's held, -q reads them.
pub(crate) fn stored(s: &mut Session, a: &Args, cmd: &str, names: &[&str]) -> CmdResult {
    let map = s.extras.more.stored.entry(cmd.to_string()).or_default();
    if a.bool("query") {
        for n in names {
            if asked(a, n) {
                return Ok(map.get(*n).cloned().unwrap_or(Value::Null));
            }
        }
        fail!("{cmd}: name the flag to query");
    }
    for n in names {
        if let Some(v) = a.get(n) {
            map.insert((*n).to_string(), v.clone());
        }
    }
    Ok(Value::Null)
}

fn key_target_weights(s: &mut Session, a: &Args) -> CmdResult {
    let nodes = picked(s, a, "setKeyframeBlendshapeTargetWts")?;
    let mut count = 0i64;
    for n in nodes {
        let g = &mut s.graph;
        let mesh = crate::components::as_mesh(g, n);
        // A target: a live connection into a blendShape's geometry input.
        let mut jobs: Vec<(NodeId, Option<i32>)> = Vec::new();
        if let Some(m) = mesh {
            for (d, src) in g.connections() {
                if src.node == m && d.attr == crate::deform_blend::GEOM_TARGET as i32 && g.node(d.node).is_some_and(|x| x.ty().name == "blendShape") {
                    jobs.push((d.node, Some(d.element / 100_000)));
                }
            }
            if jobs.is_empty() {
                let found: Vec<NodeId> = crate::history::history_of(g, m).into_iter().filter(|&h| g.node(h).is_some_and(|x| x.ty().name == "blendShape")).collect();
                jobs.extend(found.into_iter().map(|b| (b, None)));
            }
        }
        for (bs, only) in jobs {
            let g = &mut s.graph;
            let Some(input) = deform::input_mesh(g, bs) else { continue };
            let points = input.point_capacity();
            let name = g.path(bs);
            let data = crate::deform_blend::blend_of(g, bs);
            for t in data.targets.iter().filter(|t| only.is_none_or(|i| i == t.index)) {
                for p in (0..points).filter(|&p| input.point_valid(p)) {
                    let w = t.weights.as_ref().map_or(1.0, |v| v.get(p as usize).copied().unwrap_or(0.0));
                    let at = t.index * crate::deform_blend::TARGET_KEY_STRIDE + p as i32;
                    let plug_name = format!("{name}.targetWeightKeys[{at}]");
                    s.run("setAttr", json!({"args": [plug_name, w]}))?;
                    s.run("setKeyframe", json!({"args": [plug_name]}))?;
                    count += 1;
                }
            }
        }
    }
    Ok(json!(count))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(flexor_node_def(
        "jointLattice",
        "A flexor's node on a lattice: the joint's bend bulges the lattice's points. Returns the node. Less than the reference: it does not move the lattice's ends rigidly (flexor keeps the lattice on the joints with constraints) and the bind skin flags do nothing.",
    ));
    c.register(flexor_node_def(
        "boneLattice",
        "A flexor's node on a lattice along a bone: the joint's bend bulges the lattice's points. Returns the node. Less than the reference: it does not move the lattice's ends rigidly (flexor parents the lattice to the bone).",
    ));
    c.register(
        q_flags(Cmd::new(
            "flexor",
            "A deformer at joints or bones: -typ jointLattice (the default), boneLattice or sculpt, -aj or -ab, -ts for the whole skeleton. Deforms what the joints are bound to, and any meshes named. -q -l lists the types. Returns the new nodes. Less than the reference: of -dc only the lattice's -dv and the sculpt's -mxd are read; a sculpt flexor follows its joint but no attribute drives it; the lattice sits after the skin.",
            flexor_cmd,
        ))
        .positional("joints, and any extra meshes (the selection when none)")
        .flag(f("atBones", "ab", B, "At bones."))
        .flag(f("atJoints", "aj", B, "At joints (the default)."))
        .flag(f("deformerCommand", "dc", S, "The deformer's command text: -dv and -mxd are read from it."))
        .flag(f("list", "l", B, "With -q: the types."))
        .flag(f("name", "n", S, "Obsolete in the reference; has no effect."))
        .flag(f("noScale", "ns", B, "Do not size the flexor to the geometry nearby."))
        .flag(f("toSkeleton", "ts", B, "Every joint below those selected."))
        .flag(f("type", "typ", S, "Which flexor.")),
    );
    c.register(
        Cmd::new("copyFlexor", "Copy a flexor's settings and lattice tweaks to another joint: the flexor's lattice, then the joint. Returns the new node. Less than the reference: a mirrored bone does not reflect the lattice.", copy_flexor_cmd)
            .positional("the flexor's lattice and the joint"),
    );
    c.register(
        q_flags(Cmd::new(
            "combinationShape",
            "Drive a blend shape target from others through a combinationShape node: -bs, -cti or -ctn, -dti or -dtn, -cm 0 multiply, 1 lowest, 2 lowest smooth. With -e: -add, -rmd, -ald, -cm; removing the last driver deletes the node. -q: -ex, -ald, -cm. Returns the target's index, or with -e the node.",
            combination_shape,
        ))
        .flag(f("addDriver", "add", B, "With -e: add the drivers."))
        .flag(f("allDrivers", "ald", B, "All the drivers."))
        .flag(f("blendShape", "bs", S, "The blend shape."))
        .flag(f("combinationTargetIndex", "cti", I, "The driven target."))
        .flag(f("combinationTargetName", "ctn", S, "The driven target."))
        .flag(f("combineMethod", "cm", I, "0 multiplication, 1 lowest weighting, 2 lowest weighting (smooth)."))
        .flag(f("driverTargetIndex", "dti", A, "A driver.").multi(1))
        .flag(f("driverTargetName", "dtn", A, "A driver.").multi(1))
        .flag(f("exist", "ex", B, "With -q: whether the target has one."))
        .flag(f("removeDriver", "rmd", B, "With -e: remove the drivers.")),
    );
    c.register(
        q_flags(Cmd::new(
            "skeletonEmbed",
            "Fit a joint chain inside meshes: a plain heuristic, a straight run along the bounds' longest axis through the middle of each slice of points. The reference embeds a whole skeleton from a voxel segmentation; -sm and -sr are accepted and change nothing. -jc is this command's own: how many joints (5). Returns the joints; -q the positions; -q -mm the merged mesh as points and faces.",
            skeleton_embed,
        ))
        .positional("meshes (the selection when none)")
        .flag(f("mergedMesh", "mm", B, "With -q: the meshes merged."))
        .flag(f("segmentationMethod", "sm", I, "Accepted for existing scripts; has no effect."))
        .flag(f("segmentationResolution", "sr", I, "Accepted for existing scripts; has no effect."))
        .flag(f("jointCount", "jc", I, "How many joints.")),
    );
    let mut ch = q_flags(Cmd::new(
        "characterize",
        "Make a character from a skeleton: the joints named or selected go to slots by their names, -sk \"joint slot, joint slot\" sets others, -nm names it. Returns the character (a HIKCharacterNode). Less than the reference: -type label reads names too; the pivot, effector and stance flags fail, since the full body solver is not here; -pos and -aab are only held.",
        characterize,
    ))
    .positional("the skeleton's root (the selection when none)")
    .flag(f("name", "nm", S, "The character's name."))
    .flag(f("posture", "pos", S, "biped or quadruped."))
    .flag(f("sourceSkeleton", "sk", S, "Joints and the slots they fill."))
    .flag(f("type", "typ", S, "label or name: names are read either way."))
    .flag(f("autoActivateBodyPart", "aab", B, "Held for scripts."))
    .flag(f("pinHandFeet", "phf", B, "Held for scripts."));
    for (long, short) in [("activatePivot", "apv"), ("addAuxEffector", "aae"), ("addFloorContactPlane", "afp"), ("addMissingEffectors", "ame"), ("changePivotPlacement", "cpp"), ("placeNewPivot", "pnp"), ("effectors", "ef"), ("fkSkeleton", "fk"), ("stancePose", "sp"), ("attributeFromHIKProperty", "ahk"), ("attributeFromHIKPropertyMode", "mhk")] {
        ch = ch.flag(f(long, short, A, "Not available: fails."));
    }
    c.register(ch);
    c.register(
        q_flags(Cmd::new("hikGlobals", "Held for scripts: -rap, the release all pinning flag. It has no effect, since the full body solver is not here. Returns the state.", |s, a| {
            if let Some(v) = a.get("releaseAllPinning").filter(|_| !a.bool("query")) {
                s.extras.more.hik_release_pinning = truth(v);
            }
            Ok(json!(s.extras.more.hik_release_pinning))
        }))
        .flag(f("releaseAllPinning", "rap", B, "Ignore pinning states."))
        .undoable(false),
    );
    c.register(
        Cmd::new("hikGetDependNodes", "The nodes a character depends on: its joints, then what is connected to it. The reference does not document this command, so what it returns is inferred from the name.", hik_depend_nodes).positional("a character").undoable(false),
    );
    c.register(
        q_flags(Cmd::new("weightsColor", "Held for scripts: the false colour settings for showing a deformer's weights. Nothing is drawn from them: the views have no weight display for this. -q reads them.", |s, a| {
            stored(s, a, "weightsColor", &["colorRamp", "deformer", "falseColor", "outOfRangeColor", "rampMaxColor", "rampMinColor", "useColorRamp", "useMaxMinColor"])
        }))
        .positional("objects")
        .flag(f("colorRamp", "cr", S, "A ramp."))
        .flag(f("deformer", "dfm", S, "The deformer."))
        .flag(f("falseColor", "fc", B, "False colour on or off."))
        .flag(f("outOfRangeColor", "orc", ArgType::Vec3, "Colour outside the deformer's set."))
        .flag(f("rampMaxColor", "rxc", ArgType::Vec3, "Colour at or over the maximum."))
        .flag(f("rampMinColor", "rmc", ArgType::Vec3, "Colour at or under the minimum."))
        .flag(f("useColorRamp", "ucr", B, "Use the ramp."))
        .flag(f("useMaxMinColor", "umc", B, "Use the out of range colours."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("ikfkDisplayMethod", "Held for scripts: how IK/FK blending shows on a selected handle, -d none, ik, fk or ikfk. The views do not draw it. -q -d reads it.", |s, a| {
            stored(s, a, "ikfkDisplayMethod", &["display"]).map(|v| if a.bool("query") && v.is_null() { json!("ikfk") } else { v })
        }))
        .flag(f("display", "d", S, "none, ik, fk or ikfk."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("ikSystemInfo", "The ikSystem's snap switch: -gsh off means no handle snaps to its effector while IK is off. -q reads it.", ik_system_info))
            .flag(f("globalSnapHandle", "gsh", B, "Whether handles snap.")),
    );
    c.register(
        q_flags(Cmd::new("effector", "An IK effector's name and hide flag: -n, -hi, in -e and -q. hideDisplay is only held: the views do not draw effectors. Returns the effector.", effector_cmd))
            .positional("the effector (the selection when none)")
            .flag(f("hide", "hi", B, "Hide it when attached to a handle."))
            .flag(f("name", "n", S, "Its name.")),
    );
    c.register(
        q_flags(Cmd::new("instanceable", "Whether nodes may be instanced: -a false makes instance refuse them, -r their children too, -s their shapes too. Returns the states, and -q reads them. duplicate -ilf is not checked.", instanceable))
            .positional("nodes (the selection when none)")
            .flag(f("allow", "a", B, "Whether they can be instanced."))
            .flag(f("recursive", "r", B, "And the non-shape children."))
            .flag(f("shape", "s", B, "And the shape children.")),
    );
    crate::ref_core::extend(c, "instance", vec![], |s, a, prev| {
        check_instanceable(s, a)?;
        prev(s, a)
    });
    c.register(
        q_flags(Cmd::new("relationship", "Held for scripts: relationship type owner members, -b breaks it, -rd its data; -q type owner lists the members. The reference makes the connections on reading a file; this only holds the list, and it is not saved.", relationship))
            .positional("the type, the owner, then what is related")
            .flag(f("b", "b", B, "Break it."))
            .flag(f("relationshipData", "rd", S, "Data for it."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("xformConstraint", "Held for scripts: the transform tools' constraint, -t none, surface, edge or live, -n along normals. The tools do not honour it yet. -q -t, -q -n and -q -l read it; -l is whether there is a live surface.", xform_constraint))
            .flag(f("type", "t", S, "none, surface, edge or live."))
            .flag(f("alongNormal", "n", I, "Along the normals first."))
            .flag(f("live", "l", B, "With -q: whether a live surface is set."))
            .undoable(false),
    );
    c.register(
        Cmd::new("setKeyframeBlendshapeTargetWts", "Key per-point blend shape target weights at the current time: a selected base keys all its targets, a selected live target only itself. Returns how many keys. The keys go on the node's targetWeightKeys elements, which override the painted weight for that point: the reference has them under inputTarget, which are not plugs here.", key_target_weights)
            .positional("a base or a target (the selection when none)"),
    );
    let warp_flags = c.find("curveWarp").map(|d| d.flags.clone()).unwrap_or_default();
    let mut warp = Cmd::new("createCurveWarp", "Make a curveWarp on a mesh and a curve, as the Deform menu's Create Curve Warp does. The reference does not document this as a command; it runs curveWarp with what it is given.", |s, a| {
            let mut call = serde_json::Map::new();
            call.insert("args".into(), json!(objects(s, a)));
            for (k, v) in &a.flags {
                if !v.is_null() {
                    call.insert(k.clone(), v.clone());
                }
            }
            s.run("curveWarp", Value::Object(call))
        })
        .positional("the mesh, then the curve (the selection when none)");
    warp.flags = warp_flags;
    c.register(warp);
}
