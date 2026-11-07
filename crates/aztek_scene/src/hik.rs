//! HumanIK characters: definitions mapping joints to slots through `.ch`, T-pose checks, sources, baking and the procedures scripts
//! call. Nodes are in `hik_nodes.rs`, the retarget in `hik_solve.rs`, slot tables in `hik_slots.rs`, the control rig and the
//! commands for definitions' files in `hik_rig.rs`. Procedure names come from the reference's own scripts as community code quotes them;
//! The reference documents none of them.

use std::collections::BTreeMap;

use aztek_ops::{fail, CmdResult};
use glam::DVec3;
use serde_json::{json, Value};

use crate::graph::Graph;
pub use crate::hik_nodes::CHARACTER as TYPE;
pub use crate::hik_nodes::{k_slot, register_types};
use crate::hik_nodes::*;
pub use crate::hik_slots::slot_for_name;
use crate::hik_slots::{slot_index, slot_names, SLOT_COUNT};
use crate::hik_solve::rotation_of;
use crate::node::types;
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::rig::{is_joint, json_of, mat_json, set_json, world, Joint};
use crate::session::Session;
use crate::transform::{matrix_to_euler, RotateOrder, Transform};

pub(crate) fn character(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name)? {
        Some(id) if g.node(id).is_some_and(|n| n.ty().name == TYPE) => Ok(id),
        _ => fail!("{cmd}: {name} is not a character"),
    }
}

pub(crate) fn characters(g: &Graph) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().name == TYPE)).collect()
}

/// The joint in a slot.
pub(crate) fn slot_joint(g: &Graph, c: NodeId, i: usize) -> Option<NodeId> {
    g.source(&PlugRef::new(c, c_slot(i) as i32)).map(|s| s.node)
}

/// Slot names to the joints in them.
pub(crate) fn slots_of(g: &Graph, c: NodeId) -> BTreeMap<String, String> {
    (0..SLOT_COUNT).filter_map(|i| slot_joint(g, c, i).map(|j| (slot_names()[i].clone(), g.path(j)))).collect()
}

/// A joint's message plug, added to a plain transform when it hasn't one.
pub(crate) fn ch_plug(g: &mut Graph, id: NodeId) -> CmdResult<PlugRef> {
    if let Some(at) = g.node(id).and_then(|n| n.ty().attr_index("Character")) {
        return Ok(PlugRef::new(id, at as i32));
    }
    let at = g.add_attr(id, AttrDef { name: "Character".into(), short_name: "ch".into(), ty: PlugType::Int, default_value: PlugValue::Int(1), flags: flags::INPUT | flags::HIDDEN, doc: "Goes to a character's slot.".into(), ..Default::default() })?;
    Ok(PlugRef::new(id, at as i32))
}

/// Puts a joint in a slot, or empties it.
pub(crate) fn set_slot(g: &mut Graph, c: NodeId, slot: usize, joint: Option<NodeId>) -> CmdResult<()> {
    let at = PlugRef::new(c, c_slot(slot) as i32);
    g.disconnect(&at);
    // What the message plug last read says the slot is filled.
    g.set_value(at, PlugValue::Int(0))?;
    if let Some(j) = joint {
        let ch = ch_plug(g, j)?;
        g.connect(&ch, &at, true)?;
        stamp_slot(g, c, slot, j)?;
    }
    Ok(())
}

/// The joint's place now becomes the slot's T-pose: world translate, rotate and scale, and its joint orient.
fn stamp_slot(g: &mut Graph, c: NodeId, slot: usize, j: NodeId) -> CmdResult<()> {
    let m = world(g, j);
    let at = c_slot(slot);
    let r = glam::DMat3::from_quat(rotation_of(&m));
    let scale = DVec3::new(m.x_axis.truncate().length(), m.y_axis.truncate().length(), m.z_axis.truncate().length());
    let set = |g: &mut Graph, k: usize, v: DVec3| g.set_value(PlugRef::new(c, (at + k) as i32), PlugValue::Vec3(v));
    set(g, 1, m.w_axis.truncate())?;
    set(g, 2, matrix_to_euler(&r, RotateOrder::Xyz))?;
    set(g, 3, scale)?;
    if is_joint(g, j) {
        let jo = g.node(j).unwrap().data().vec3(Joint::JOINT_ORIENT);
        set(g, 4, jo)?;
    }
    Ok(())
}

/// Every joint's place now becomes the T-pose.
pub(crate) fn stamp_stance(g: &mut Graph, c: NodeId) -> CmdResult<()> {
    g.evaluate();
    for i in 0..SLOT_COUNT {
        if let Some(j) = slot_joint(g, c, i) {
            stamp_slot(g, c, i, j)?;
        }
    }
    refresh_rig(g, c)
}

/// Writes how the mapped joints hang together: each one's nearest mapped ancestor and what is between, parents first.
pub(crate) fn refresh_rig(g: &mut Graph, c: NodeId) -> CmdResult<()> {
    g.evaluate();
    let mapped: Vec<(usize, NodeId)> = (0..SLOT_COUNT).filter_map(|i| slot_joint(g, c, i).map(|j| (i, j))).collect();
    let slot_at = |n: NodeId| mapped.iter().find(|(_, j)| *j == n).map(|(i, _)| *i);
    let mut rows: Vec<(usize, Value)> = Vec::new();
    for &(i, j) in &mapped {
        let mut depth = 0;
        let mut up = g.parent(j);
        let mut parent = None;
        while up != NO_NODE {
            depth += 1;
            if parent.is_none() {
                parent = slot_at(up).map(|s| (s, up));
            }
            up = g.parent(up);
        }
        let dag_parent = g.parent(j);
        let top = slot_names()[i] == "Hips";
        let between = match parent {
            Some((_, n)) if dag_parent != NO_NODE => world(g, n).inverse() * world(g, dag_parent),
            // The top of the body follows rootParentGX; a limb under a group of its own stays over that group's place.
            None if dag_parent != NO_NODE && !top => world(g, dag_parent),
            _ => glam::DMat4::IDENTITY,
        };
        let filled = |n: &str| slot_index(n).filter(|s| mapped.iter().any(|(x, _)| x == s));
        let body = parent.map(|p| p.0).or_else(|| body_parent(&slot_names()[i], &filled));
        let d = g.node(j).unwrap().data();
        let joint = g.node(j).unwrap().ty().is_a("joint");
        let jo = if joint { d.vec3(Joint::JOINT_ORIENT) } else { DVec3::ZERO };
        let ra = d.vec3(Transform::ROTATE_AXIS);
        let ro = d.int(Transform::ROTATE_ORDER);
        let s = d.vec3(Transform::SCALE);
        rows.push((depth, json!({"slot": i, "parent": body, "up": parent.map(|x| x.0), "between": mat_json(&between), "jo": jo.to_array(), "ra": ra.to_array(), "ro": ro, "s": s.to_array()})));
    }
    // Parents first, by the body's links.
    let parent_of: BTreeMap<u64, Option<u64>> = rows.iter().map(|r| (r.1["slot"].as_u64().unwrap(), r.1["parent"].as_u64())).collect();
    let rank = |s: u64| {
        let mut n = 0;
        let mut at = parent_of.get(&s).copied().flatten();
        while let Some(x) = at {
            n += 1;
            at = parent_of.get(&x).copied().flatten();
            if n > SLOT_COUNT {
                break;
            }
        }
        n
    };
    rows.sort_by_key(|r| (rank(r.1["slot"].as_u64().unwrap()), r.0));
    let list: Vec<Value> = rows.into_iter().map(|r| r.1).collect();
    set_json(g, c, C_RIG, json!(list))
}

/// The slot before this one in the body, for rigs whose limbs don't hang under the spine in the hierarchy.
fn body_parent(slot: &str, filled: &dyn Fn(&str) -> Option<usize>) -> Option<usize> {
    let top_spine = || (1..=9).rev().find_map(|k| filled(&format!("Spine{k}"))).or_else(|| filled("Spine")).or_else(|| filled("Hips"));
    let top_neck = || (1..=9).rev().find_map(|k| filled(&format!("Neck{k}"))).or_else(|| filled("Neck")).or_else(top_spine);
    let numbered = |word: &str, k: u32| (0..k).rev().find_map(|x| filled(&if x == 0 { word.to_string() } else { format!("{word}{x}") }));
    for side in ["Left", "Right"] {
        if let Some(rest) = slot.strip_prefix(side) {
            let named = |n: &str| filled(&format!("{side}{n}"));
            return match rest {
                "UpLeg" => filled("Hips"),
                "Leg" => named("UpLeg").or_else(|| filled("Hips")),
                "Foot" => named("Leg").or_else(|| named("UpLeg")),
                "ToeBase" => named("Foot"),
                "Shoulder" => top_spine(),
                "Arm" => named("Shoulder").or_else(top_spine),
                "ForeArm" => named("Arm"),
                "Hand" => named("ForeArm"),
                _ if rest.starts_with("Hand") => named("Hand"),
                _ if rest.starts_with("Foot") => named("Foot"),
                _ => None,
            };
        }
    }
    match slot {
        "Hips" | "Reference" => None,
        "Spine" => filled("Hips"),
        "Neck" => top_spine(),
        "Head" => top_neck(),
        _ => {
            if let Some(k) = slot.strip_prefix("Spine").and_then(|x| x.parse::<u32>().ok()) {
                numbered("Spine", k).or_else(|| filled("Hips"))
            } else if let Some(k) = slot.strip_prefix("Neck").and_then(|x| x.parse::<u32>().ok()) {
                numbered("Neck", k).or_else(top_spine)
            } else {
                None
            }
        }
    }
}

fn is_locked(g: &Graph, c: NodeId) -> bool {
    g.node(c).is_some_and(|n| n.data().bool(C_LOCK))
}

/// Makes a character and its properties node, as the reference pairs them.
pub(crate) fn new_character(s: &mut Session, name: &str) -> CmdResult<NodeId> {
    let g = &mut s.graph;
    let t = types::find(TYPE).unwrap();
    let c = g.create_node(&t, name, NO_NODE, true)?;
    let t = types::find("HIKProperty2State").unwrap();
    let p = g.create_node(&t, "HIKproperties1", NO_NODE, true)?;
    g.connect(&PlugRef::new(p, P_MSG as i32), &PlugRef::new(c, C_PROPERTY as i32), false)?;
    // The reference writes each slot's rotation limits so.
    for i in 0..172 {
        let at = c_slot(i);
        g.set_value(PlugRef::new(c, (at + 5) as i32), PlugValue::Vec3(DVec3::splat(-45.0)))?;
        g.set_value(PlugRef::new(c, (at + 6) as i32), PlugValue::Vec3(DVec3::splat(45.0)))?;
    }
    s.extras.more.characterize.insert("currentCharacter".into(), json!(g.path(c)));
    Ok(c)
}

/// Names `Spine1` a first spine joint called spine_01, and so on down: slots go in order along the chain.
fn compact_chain(found: &mut BTreeMap<String, NodeId>, g: &Graph, word: &str) {
    let mut members: Vec<(String, NodeId)> = found.iter().filter(|(k, _)| *k == word || k.strip_prefix(word).is_some_and(|r| r.parse::<u32>().is_ok())).map(|(k, v)| (k.clone(), *v)).collect();
    let depth = |mut n: NodeId| {
        let mut d = 0;
        while n != NO_NODE {
            d += 1;
            n = g.parent(n);
        }
        d
    };
    members.sort_by_key(|(_, n)| depth(*n));
    for (k, _) in &members {
        found.remove(k);
    }
    for (i, (_, n)) in members.into_iter().enumerate().take(10) {
        found.insert(if i == 0 { word.to_string() } else { format!("{word}{i}") }, n);
    }
}

/// Maps the joints under these roots by name, with `strip` taken off each name first.
pub(crate) fn name_match(g: &mut Graph, c: NodeId, roots: &[NodeId], strip: &str) -> CmdResult<usize> {
    let mut found: BTreeMap<String, NodeId> = BTreeMap::new();
    for &r in roots {
        let mut all = vec![r];
        all.extend(g.descendants(r));
        for j in all {
            if !g.node(j).is_some_and(|n| n.ty().is_a("transform")) || (!is_joint(g, j) && roots.iter().all(|x| *x != j)) {
                continue;
            }
            let path = g.path(j);
            let short = path.rsplit(['|', ':']).next().unwrap_or(&path);
            let name = short.strip_prefix(strip).unwrap_or(short);
            if let Some(slot) = slot_for_name(name) {
                found.entry(slot).or_insert(j);
            }
        }
    }
    compact_chain(&mut found, g, "Spine");
    compact_chain(&mut found, g, "Neck");
    let n = found.len();
    for (slot, j) in found {
        if let Some(i) = slot_index(&slot) {
            set_slot(g, c, i, Some(j))?;
        }
    }
    refresh_rig(g, c)?;
    Ok(n)
}

// ---------------------------------------------------------------------------------------------------------------------
// Sources

fn dest_of_type(g: &Graph, c: NodeId, ty: &str) -> Option<NodeId> {
    g.destinations(&PlugRef::new(c, C_OUT_DEF as i32)).into_iter().map(|d| d.node).find(|&n| g.node(n).is_some_and(|x| x.ty().name == ty && n != c))
}

/// The nodes that take a target's state to its joints, made if need be: the solver and the HIKState2SK.
fn ensure_output(g: &mut Graph, c: NodeId) -> CmdResult<(NodeId, NodeId)> {
    let def = PlugRef::new(c, C_OUT_DEF as i32);
    let solver = match g.destinations(&def).into_iter().find(|d| d.attr == V_DEF as i32 && g.node(d.node).is_some_and(|n| n.ty().name == "HIKSolverNode")) {
        Some(d) => d.node,
        None => {
            let t = types::find("HIKSolverNode").unwrap();
            let v = g.create_node(&t, "HIKSolverNode1", NO_NODE, true)?;
            g.connect(&def, &PlugRef::new(v, V_DEF as i32), false)?;
            if let Some(p) = g.source(&PlugRef::new(c, C_PROPERTY as i32)) {
                g.connect(&PlugRef::new(p.node, P_OUT as i32), &PlugRef::new(v, V_PROP as i32), false)?;
            }
            v
        }
    };
    let out = match dest_of_type(g, c, "HIKState2SK") {
        Some(k) => k,
        None => {
            let t = types::find("HIKState2SK").unwrap();
            let k = g.create_node(&t, "HIKState2SK1", NO_NODE, true)?;
            g.connect(&def, &PlugRef::new(k, K_DEF as i32), false)?;
            g.connect(&PlugRef::new(solver, V_OUT as i32), &PlugRef::new(k, K_STATE as i32), false)?;
            k
        }
    };
    Ok((solver, out))
}

const CHANNELS: [(usize, usize); 3] = [(Transform::TRANSLATE, 0), (Transform::ROTATE, 1), (Transform::SCALE, 2)];

/// Whether a target's joints are being driven.
fn is_driving(g: &Graph, k: NodeId) -> bool {
    (0..SLOT_COUNT).any(|i| !g.destinations(&PlugRef::component(k, k_slot(i) as i32 + 1, 0)).is_empty())
}

/// What a rig drives above the mapped joints (a limb's group following a constraint, say) would depend on the joints being
/// solved. The reference reads each joint's parent matrix live; the graph here orders whole nodes, so those transforms hold still
/// while the character is driven, keys and all else kept, and get their connections back after.
fn hold_groups(g: &mut Graph, c: NodeId, saved: &mut Vec<Value>) -> CmdResult<()> {
    let mapped: Vec<NodeId> = (0..SLOT_COUNT).filter_map(|i| slot_joint(g, c, i)).collect();
    let mut seen: Vec<NodeId> = Vec::new();
    for &j in &mapped {
        let mut up = g.parent(j);
        while up != NO_NODE && !mapped.contains(&up) && !seen.contains(&up) {
            seen.push(up);
            if g.node(up).is_some_and(|n| n.ty().is_a("transform")) {
                for (attr, _) in CHANNELS {
                    let whole = PlugRef::new(up, attr as i32);
                    let plugs: Vec<PlugRef> = std::iter::once(whole).chain((0..3).map(|a| PlugRef::component(up, attr as i32, a))).collect();
                    for p in plugs {
                        let Some(src) = g.source(&p) else { continue };
                        if crate::anim::is_curve(g, src.node) {
                            continue;
                        }
                        saved.push(json!({"plug": g.plug_name(&p), "value": Value::Null, "source": g.plug_name(&src)}));
                        let held = g.value(&p)?;
                        g.disconnect(&p);
                        g.set_value(p, held)?;
                    }
                }
            }
            up = g.parent(up);
        }
    }
    Ok(())
}

/// Connects a HIKState2SK to its joints, keeping what they held.
fn drive_joints(g: &mut Graph, c: NodeId, k: NodeId) -> CmdResult<()> {
    if is_driving(g, k) {
        return Ok(());
    }
    g.evaluate();
    let mut saved = Vec::new();
    hold_groups(g, c, &mut saved)?;
    refresh_rig(g, c)?;
    let reference = slot_index("Reference").unwrap();
    for i in 0..SLOT_COUNT {
        if i == reference {
            continue;
        }
        let Some(j) = slot_joint(g, c, i) else { continue };
        for (attr, offset) in CHANNELS {
            for axis in 0..3 {
                let p = PlugRef::component(j, attr as i32, axis);
                if g.is_locked(&p) {
                    continue;
                }
                let held = g.value(&p)?.scalar().unwrap_or(0.0);
                let src = g.source(&p);
                saved.push(json!({"plug": g.plug_name(&p), "value": held, "source": src.map(|s| g.plug_name(&s))}));
                if src.is_some() {
                    g.disconnect(&p);
                }
                g.connect(&PlugRef::component(k, (k_slot(i) + offset) as i32, axis), &p, true)?;
            }
        }
    }
    // The world above the top of the mapped joints.
    let top = json_of(g.node(c).unwrap().data(), C_RIG).as_array().and_then(|a| a.iter().find(|r| r["parent"].is_null()).and_then(|r| r["slot"].as_u64()));
    if let Some(j) = top.and_then(|s| slot_joint(g, c, s as usize)) {
        let up = g.parent(j);
        if up != NO_NODE {
            g.connect(&PlugRef::new(up, Transform::WORLD_MATRIX as i32), &PlugRef::new(k, K_ROOT as i32), true)?;
        }
    }
    set_json(g, k, K_SAVED, json!(saved))
}

/// Lets the joints go. With `restore` they get back what they held; after a bake only what wasn't keyed.
fn release_joints(g: &mut Graph, k: NodeId, restore: bool) -> CmdResult<()> {
    let saved = json_of(g.node(k).unwrap().data(), K_SAVED);
    for i in 0..SLOT_COUNT {
        for offset in 0..3 {
            for axis in 0..3 {
                for d in g.destinations(&PlugRef::component(k, (k_slot(i) + offset) as i32, axis)) {
                    g.disconnect(&d);
                }
            }
        }
    }
    for row in saved.as_array().cloned().unwrap_or_default() {
        let Some(name) = row["plug"].as_str() else { continue };
        let Ok(p) = g.plug(name) else { continue };
        if !restore && g.source(&p).is_some() {
            continue;
        }
        // A group held still: it goes back to following, also after a bake unless that would pull the baked joints away.
        if row["value"].is_null() {
            if restore {
                if let Some(s) = row["source"].as_str().and_then(|s| g.plug(s).ok()) {
                    g.connect(&s, &p, true)?;
                }
            }
            continue;
        }
        let v = row["value"].as_f64().unwrap_or(0.0);
        let keyed = !restore && g.attr(&p).name != "scale";
        match row["source"].as_str().and_then(|s| g.plug(s).ok()) {
            Some(s) if restore => {
                g.connect(&s, &p, true)?;
            }
            _ if keyed => {}
            _ => g.set_value(p, PlugValue::Float(v))?,
        }
    }
    set_json(g, k, K_SAVED, json!([]))
}

/// The state node a source character's joints feed, made if need be.
fn ensure_source_state(g: &mut Graph, src: NodeId) -> CmdResult<NodeId> {
    let n = match dest_of_type(g, src, "HIKSK2State") {
        Some(n) => n,
        None => {
            let t = types::find("HIKSK2State").unwrap();
            let n = g.create_node(&t, "HIKSK2State1", NO_NODE, true)?;
            g.connect(&PlugRef::new(src, C_OUT_DEF as i32), &PlugRef::new(n, S_DEF as i32), false)?;
            n
        }
    };
    // The definition may have changed since.
    for i in 0..SLOT_COUNT {
        let gx = PlugRef::new(n, (S_GX + i) as i32);
        match slot_joint(g, src, i) {
            Some(j) => {
                g.connect(&PlugRef::new(j, Transform::WORLD_MATRIX as i32), &gx, true)?;
            }
            None => g.disconnect(&gx),
        }
    }
    Ok(n)
}

fn properties_of(g: &Graph, c: NodeId) -> Option<NodeId> {
    g.source(&PlugRef::new(c, C_PROPERTY as i32)).map(|s| s.node)
}

/// What drives a character now: "None", "Stance", "Control Rig", or a character's name.
pub(crate) fn current_source(g: &Graph, c: NodeId) -> String {
    let Some(k) = dest_of_type(g, c, "HIKState2SK") else { return if crate::hik_rig::has_control_rig(g, c) { "Control Rig".into() } else { "None".into() } };
    if !is_driving(g, k) {
        return if crate::hik_rig::has_control_rig(g, c) { "Control Rig".into() } else { "None".into() };
    }
    let Some(solver) = g.source(&PlugRef::new(k, K_STATE as i32)).map(|s| s.node) else { return "None".into() };
    if g.node(solver).is_some_and(|n| n.data().bool(V_STANCE)) {
        return "Stance".into();
    }
    match g.source(&PlugRef::new(solver, V_STATE as i32)).map(|s| s.node) {
        Some(n) if g.node(n).is_some_and(|x| x.ty().name == "HIKRetargeterNode") => g.source(&PlugRef::new(n, R_DEF_SRC as i32)).map(|s| g.path(s.node)).unwrap_or_else(|| "None".into()),
        Some(n) if g.node(n).is_some_and(|x| x.ty().name == "HIKFK2State") => "Control Rig".into(),
        _ => "None".into(),
    }
}

/// Sets what drives a character. The UI's menu has " Control Rig" with a leading space in the reference's scripts; both read.
pub(crate) fn set_source(s: &mut Session, target: &str, source: &str) -> CmdResult<usize> {
    let c = character(&s.graph, target, "hikSetCurrentSource")?;
    let source = source.trim();
    let g = &mut s.graph;
    g.open_chunk("HumanIK source");
    let r = (|| -> CmdResult<usize> {
        if let Some(k) = dest_of_type(g, c, "HIKState2SK") {
            if is_driving(g, k) {
                release_joints(g, k, true)?;
            }
        }
        if let Some(v) = dest_of_type(g, c, "HIKSolverNode") {
            let old = g.source(&PlugRef::new(v, V_STATE as i32)).map(|x| x.node);
            g.disconnect(&PlugRef::new(v, V_STATE as i32));
            g.set_value(PlugRef::new(v, V_STANCE as i32), PlugValue::Bool(false))?;
            if let Some(n) = old.filter(|&n| g.node(n).is_some_and(|x| x.ty().name == "HIKRetargeterNode")) {
                g.delete_node(n)?;
            }
        }
        match source {
            "" | "None" | "Control Rig" => return Ok(0),
            _ => {}
        }
        if !is_locked(g, c) {
            lock_character(g, c, true)?;
        }
        let (solver, k) = ensure_output(g, c)?;
        if source == "Stance" {
            g.set_value(PlugRef::new(solver, V_STANCE as i32), PlugValue::Bool(true))?;
        } else {
            let from = character(g, source, "hikSetCurrentSource")?;
            if from == c {
                fail!("hikSetCurrentSource: a character can't be its own source");
            }
            if !is_locked(g, from) {
                lock_character(g, from, true)?;
            }
            let state = ensure_source_state(g, from)?;
            let t = types::find("HIKRetargeterNode").unwrap();
            let re = g.create_node(&t, "HIKRetargeterNode1", NO_NODE, true)?;
            g.connect(&PlugRef::new(state, S_OUT as i32), &PlugRef::new(re, R_STATE as i32), false)?;
            g.connect(&PlugRef::new(from, C_OUT_DEF as i32), &PlugRef::new(re, R_DEF_SRC as i32), false)?;
            g.connect(&PlugRef::new(c, C_OUT_DEF as i32), &PlugRef::new(re, R_DEF_DST as i32), false)?;
            if let Some(p) = properties_of(g, from) {
                g.connect(&PlugRef::new(p, P_OUT as i32), &PlugRef::new(re, R_PROP_SRC as i32), false)?;
            }
            if let Some(p) = properties_of(g, c) {
                g.connect(&PlugRef::new(p, P_OUT as i32), &PlugRef::new(re, R_PROP_DST as i32), false)?;
            }
            g.connect(&PlugRef::new(re, R_OUT as i32), &PlugRef::new(solver, V_STATE as i32), true)?;
        }
        drive_joints(g, c, k)?;
        Ok((0..SLOT_COUNT).filter(|&i| slot_joint(g, c, i).is_some()).count())
    })();
    s.graph.close_chunk();
    r
}

pub(crate) fn lock_character(g: &mut Graph, c: NodeId, on: bool) -> CmdResult<()> {
    if on {
        stamp_stance(g, c)?;
    }
    g.set_value(PlugRef::new(c, C_LOCK as i32), PlugValue::Bool(on))?;
    Ok(())
}

pub(crate) fn locked(g: &Graph, c: NodeId) -> bool {
    is_locked(g, c)
}

pub(crate) fn release_after_bake(g: &mut Graph, c: NodeId) -> CmdResult<()> {
    if let Some(k) = dest_of_type(g, c, "HIKState2SK") {
        release_joints(g, k, false)?;
    }
    if let Some(v) = dest_of_type(g, c, "HIKSolverNode") {
        g.disconnect(&PlugRef::new(v, V_STATE as i32));
    }
    Ok(())
}

/// After reading a file: joints' places aren't to be trusted as a T-pose, but how they hang together is.
pub fn after_load(g: &mut Graph) {
    for c in characters(g) {
        let _ = refresh_rig(g, c);
    }
}

pub use crate::hik_rig::register;
