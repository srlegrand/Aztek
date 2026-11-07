//! HumanIK's commands: the procedures scripts call, the T-pose checks, name templates, the standard skeleton, the control rig,
//! and baking. Nothing here is documented by the reference apart from `characterize` and `hikGlobals`: names and arguments are as community
//! scripts quote them, and what the checks say is written here to read like the reference's rather than copied from it.

use std::collections::BTreeMap;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::hik::*;
use crate::hik_nodes::*;
use crate::hik_slots::{mirror_slot, slot_index, slot_names, REQUIRED, SLOT_COUNT};
use crate::hik_solve::{position_of, rotation_of};
use crate::plug::{NodeId, PlugRef, NO_NODE};
use crate::rig::{is_joint, world};
use crate::session::Session;
use crate::transform::{matrix_to_euler, RotateOrder};

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn current(s: &Session) -> Option<String> {
    let g = &s.graph;
    let named = s.extras.more.characterize.get("currentCharacter").and_then(Value::as_str).filter(|n| g.find(n).ok().flatten().is_some()).map(str::to_string);
    named.or_else(|| characters(g).first().map(|&c| g.path(c)))
}

/// The character named first, or the current one.
fn the_character(s: &Session, a: &Args, cmd: &str) -> CmdResult<(String, NodeId)> {
    let name = match a.pos_str(0) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => match current(s) {
            Some(n) => n,
            None => fail!("{cmd}: no character"),
        },
    };
    let c = character(&s.graph, &name, cmd)?;
    Ok((name, c))
}

fn slot_arg(a: &Args, i: usize) -> Option<usize> {
    let v = a.positional.get(i)?;
    if let Some(n) = v.as_str() {
        return n.parse::<usize>().ok().filter(|&x| x < SLOT_COUNT).or_else(|| slot_index(n));
    }
    v.as_i64().map(|x| x as usize).filter(|&x| x < SLOT_COUNT)
}

fn set_current(s: &mut Session, name: &str) {
    s.extras.more.characterize.insert("currentCharacter".into(), json!(name));
}

// ---------------------------------------------------------------------------------------------------------------------
// The definition's checks

/// Each mapped slot's parent in the body, for checking the hierarchy.
fn body_parent(slot: &str) -> Option<&'static str> {
    Some(match slot {
        "LeftLeg" => "LeftUpLeg",
        "LeftFoot" => "LeftLeg",
        "RightLeg" => "RightUpLeg",
        "RightFoot" => "RightLeg",
        "LeftUpLeg" | "RightUpLeg" | "Spine" => "Hips",
        "Head" => "Spine",
        "LeftForeArm" => "LeftArm",
        "LeftHand" => "LeftForeArm",
        "RightForeArm" => "RightArm",
        "RightHand" => "RightForeArm",
        "LeftArm" | "RightArm" => "Spine",
        _ => return None,
    })
}

fn is_below(g: &Graph, mut n: NodeId, up: NodeId) -> bool {
    loop {
        n = g.parent(n);
        if n == NO_NODE {
            return false;
        }
        if n == up {
            return true;
        }
    }
}

fn angle_to(a: DVec3, b: DVec3) -> f64 {
    let (a, b) = (a.normalize_or_zero(), b.normalize_or_zero());
    a.dot(b).clamp(-1.0, 1.0).acos().to_degrees()
}

/// Red for what stops the character working, yellow for what will spoil the retarget, green otherwise.
pub(crate) fn definition_status(g: &mut Graph, c: NodeId) -> Value {
    g.evaluate();
    let mut status: BTreeMap<String, String> = BTreeMap::new();
    let mut messages: Vec<String> = Vec::new();
    let joint = |g: &Graph, n: &str| slot_index(n).and_then(|i| slot_joint(g, c, i));
    for i in 0..SLOT_COUNT {
        if slot_joint(g, c, i).is_some() {
            status.insert(slot_names()[i].clone(), "green".into());
        }
    }
    for n in REQUIRED {
        if joint(g, n).is_none() {
            status.insert(n.to_string(), "red".into());
            messages.push(format!("{n} is required and not mapped."));
        }
    }
    let at = |g: &mut Graph, n: &str| joint(g, n).map(|j| position_of(&world(g, j)));
    for n in REQUIRED {
        if let (Some(p), Some(j)) = (body_parent(n), joint(g, n)) {
            if let Some(up) = joint(g, p) {
                if !is_below(g, j, up) {
                    status.insert(n.to_string(), "red".into());
                    messages.push(format!("{n} must be below {p} in the hierarchy."));
                }
            }
        }
    }
    let warn = |status: &mut BTreeMap<String, String>, messages: &mut Vec<String>, n: &str, text: String| {
        if status.get(n).map(String::as_str) != Some("red") {
            status.insert(n.to_string(), "yellow".into());
        }
        messages.push(text);
    };
    // The T-pose: facing +Z, up +Y, arms along X, legs straight down, left on +X.
    for (side, sign) in [("Left", 1.0), ("Right", -1.0)] {
        for (a, b, dir, what) in [("Arm", "ForeArm", DVec3::X * sign, "along the X axis"), ("ForeArm", "Hand", DVec3::X * sign, "along the X axis"), ("UpLeg", "Leg", DVec3::NEG_Y, "straight down"), ("Leg", "Foot", DVec3::NEG_Y, "straight down")] {
            let (na, nb) = (format!("{side}{a}"), format!("{side}{b}"));
            if let (Some(pa), Some(pb)) = (at(g, &na), at(g, &nb)) {
                let off = angle_to(pb - pa, dir);
                if off > 15.0 {
                    warn(&mut status, &mut messages, &nb, format!("{na} should point {what} in the T-pose: it is {off:.0} degrees off."));
                }
            }
        }
        if let (Some(p), Some(h)) = (at(g, &format!("{side}UpLeg")), at(g, "Hips")) {
            if (p.x - h.x) * sign < 0.0 {
                let n = format!("{side}UpLeg");
                warn(&mut status, &mut messages, &n, format!("{n} is on the wrong side: the character should face +Z, left on +X."));
            }
        }
    }
    if let (Some(a), Some(b)) = (at(g, "Hips"), at(g, "Head")) {
        let off = angle_to(b - a, DVec3::Y);
        if off > 15.0 {
            warn(&mut status, &mut messages, "Head", format!("The back should be straight up in the T-pose: it is {off:.0} degrees off."));
        }
    }
    let red = status.values().any(|v| v == "red");
    let yellow = status.values().any(|v| v == "yellow");
    let word = if red { "Invalid" } else if yellow { "Valid, with warnings" } else { "Valid" };
    json!({"status": status, "messages": messages, "valid": !red, "summary": format!("Character definition: {word}")})
}

/// A stored stance slot: world place and rotation.
fn stance_get(g: &Graph, c: NodeId, i: usize) -> (DVec3, DVec3) {
    let d = g.node(c).unwrap().data();
    (d.vec3(c_slot(i) + 1), d.vec3(c_slot(i) + 2))
}

/// Turns each arm's stance about the shoulder so the upper arm lies along X, then about the elbow so the forearm does.
fn straighten_arms(g: &mut Graph, c: NodeId) -> CmdResult<usize> {
    use crate::transform::euler_to_matrix;
    let mut changed = 0;
    for (side, sign) in [("Left", 1.0), ("Right", -1.0)] {
        // Everything of this side from the arm out.
        let members: Vec<usize> = (0..SLOT_COUNT)
            .filter(|&i| {
                let n = &slot_names()[i];
                let tail = n.strip_prefix(side).or_else(|| n.strip_prefix("Leaf").and_then(|x| x.strip_prefix(side)));
                tail.is_some_and(|t| t.starts_with("Arm") || t.starts_with("ForeArm") || t.starts_with("Hand") || t.starts_with("InHand") || t.starts_with("FingerBase")) && slot_joint(g, c, i).is_some()
            })
            .collect();
        for (pivot, next, from_pivot) in [("Arm", "ForeArm", true), ("ForeArm", "Hand", false)] {
            let (Some(pi), Some(ni)) = (slot_index(&format!("{side}{pivot}")), slot_index(&format!("{side}{next}"))) else { continue };
            if slot_joint(g, c, pi).is_none() || slot_joint(g, c, ni).is_none() {
                continue;
            }
            let (p0, _) = stance_get(g, c, pi);
            let (n0, _) = stance_get(g, c, ni);
            let dir = (n0 - p0).normalize_or_zero();
            if dir == DVec3::ZERO {
                continue;
            }
            let turn = glam::DQuat::from_rotation_arc(dir, DVec3::X * sign);
            for &i in &members {
                let name = &slot_names()[i];
                let moves = from_pivot || !(name.contains("Arm") && !name.contains("ForeArm")) && i != pi || i == pi;
                if !moves {
                    continue;
                }
                let (t, r) = stance_get(g, c, i);
                let q = turn * glam::DQuat::from_mat4(&euler_to_matrix(r, RotateOrder::Xyz));
                let e = matrix_to_euler(&glam::DMat3::from_quat(q.normalize()), RotateOrder::Xyz);
                g.set_value(PlugRef::new(c, (c_slot(i) + 1) as i32), crate::plug::PlugValue::Vec3(p0 + turn * (t - p0)))?;
                g.set_value(PlugRef::new(c, (c_slot(i) + 2) as i32), crate::plug::PlugValue::Vec3(e))?;
            }
            changed = changed.max(members.len());
        }
    }
    Ok(changed)
}

// ---------------------------------------------------------------------------------------------------------------------
// Mirror matching and templates

fn swap_side(name: &str) -> Vec<String> {
    let mut out = Vec::new();
    for (a, b) in [("Left", "Right"), ("left", "right"), ("L_", "R_"), ("l_", "r_"), ("_L", "_R"), ("_l", "_r")] {
        for (x, y) in [(a, b), (b, a)] {
            if name.contains(x) {
                out.push(name.replacen(x, y, 1));
            }
        }
    }
    out
}

/// Fills each empty slot from its opposite: by name with the side changed, else the joint nearest the mirrored place.
fn mirror_match(g: &mut Graph, c: NodeId) -> CmdResult<usize> {
    g.evaluate();
    let joints: Vec<NodeId> = g.nodes().into_iter().filter(|&n| is_joint(g, n)).collect();
    let mut n = 0;
    for i in 0..SLOT_COUNT {
        let m = mirror_slot(i);
        if m == i || slot_joint(g, c, m).is_some() {
            continue;
        }
        let Some(j) = slot_joint(g, c, i) else { continue };
        let name = g.path(j);
        let by_name = swap_side(&name).iter().find_map(|x| g.find(x).ok().flatten());
        let found = by_name.or_else(|| {
            let want = position_of(&world(g, j)) * DVec3::new(-1.0, 1.0, 1.0);
            let mut best: Option<(f64, NodeId)> = None;
            for &o in &joints {
                let d = (position_of(&world(g, o)) - want).length();
                if d < 1e-3 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, o));
                }
            }
            best.map(|b| b.1)
        });
        if let Some(o) = found {
            set_slot(g, c, m, Some(o))?;
            n += 1;
        }
    }
    refresh_rig(g, c)?;
    Ok(n)
}

fn xml_attr<'a>(tag: &'a str, key: &str) -> Option<&'a str> {
    let at = tag.find(&format!("{key}=\""))? + key.len() + 2;
    let end = tag[at..].find('"')? + at;
    Some(&tag[at..end])
}

/// Slot to joint name from a template: any element with key and value, or slot and joint, attributes. The reference's own template
/// layout is not documented, so ours is the simple one and this reads anything of that shape.
fn read_template(text: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    for tag in text.split('<').skip(1) {
        let tag = tag.split('>').next().unwrap_or("");
        let k = xml_attr(tag, "key").or(xml_attr(tag, "slot"));
        let v = xml_attr(tag, "value").or(xml_attr(tag, "joint"));
        if let (Some(k), Some(v)) = (k, v) {
            if !v.is_empty() {
                out.push((k.to_string(), v.to_string()));
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// The standard skeleton

fn make_skeleton(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.string("name", "Character1");
    let h = a.float("height", 170.0) / 170.0;
    let spines = a.int("spineCount", 3).clamp(1, 6);
    let mut out: Vec<String> = Vec::new();
    let mut slots: BTreeMap<String, String> = BTreeMap::new();
    let mut joint = |s: &mut Session, slot: &str, parent: Option<&str>, p: [f64; 3]| -> CmdResult<String> {
        match parent {
            Some(x) => s.run("select", json!({"args": [x]}))?,
            None => s.run("select", json!({"clear": true}))?,
        };
        let n = format!("{name}_{slot}");
        let r = s.run("joint", json!({"position": [p[0] * h, p[1] * h, p[2] * h], "name": n, "radius": 3.0 * h}))?;
        let made = r.as_str().unwrap_or(&n).to_string();
        slots.insert(slot.to_string(), made.clone());
        out.push(made.clone());
        Ok(made)
    };
    let hips = joint(s, "Hips", None, [0.0, 95.0, 0.0])?;
    let mut last = hips.clone();
    for i in 0..spines {
        let slot = if i == 0 { "Spine".to_string() } else { format!("Spine{i}") };
        last = joint(s, &slot, Some(&last), [0.0, 95.0 + 45.0 * (i + 1) as f64 / spines as f64, 0.0])?;
    }
    let chest = last.clone();
    let neck = joint(s, "Neck", Some(&chest), [0.0, 148.0, 0.0])?;
    joint(s, "Head", Some(&neck), [0.0, 158.0, 0.0])?;
    for (side, x) in [("Left", 1.0), ("Right", -1.0)] {
        let sh = joint(s, &format!("{side}Shoulder"), Some(&chest), [x * 6.0, 142.0, 0.0])?;
        let arm = joint(s, &format!("{side}Arm"), Some(&sh), [x * 18.0, 142.0, 0.0])?;
        let fore = joint(s, &format!("{side}ForeArm"), Some(&arm), [x * 45.0, 142.0, -1.0])?;
        let hand = joint(s, &format!("{side}Hand"), Some(&fore), [x * 70.0, 142.0, 0.0])?;
        if a.bool("fingers") {
            for (k, fname) in ["Thumb", "Index", "Middle", "Ring", "Pinky"].iter().enumerate() {
                let z = 3.0 - k as f64 * 1.5;
                let mut prev = hand.clone();
                for seg in 1..=3 {
                    prev = joint(s, &format!("{side}Hand{fname}{seg}"), Some(&prev), [x * (72.0 + seg as f64 * 2.5), 142.0, z])?;
                }
            }
        }
        let up = joint(s, &format!("{side}UpLeg"), Some(&hips), [x * 9.0, 90.0, 0.0])?;
        let leg = joint(s, &format!("{side}Leg"), Some(&up), [x * 9.0, 50.0, 1.0])?;
        let foot = joint(s, &format!("{side}Foot"), Some(&leg), [x * 9.0, 8.0, 0.0])?;
        joint(s, &format!("{side}ToeBase"), Some(&foot), [x * 9.0, 2.0, 12.0])?;
    }
    s.run("select", json!({"args": [hips]}))?;
    if a.bool("character") {
        let c = new_character(s, &name)?;
        let g = &mut s.graph;
        g.evaluate();
        for (slot, j) in &slots {
            let (Some(i), Some(id)) = (slot_index(slot), g.find(j)?) else { continue };
            set_slot(g, c, i, Some(id))?;
        }
        refresh_rig(g, c)?;
    }
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// The control rig: FK down the middle, IK hands and feet with pole vectors

fn at(g: &mut Graph, name: &str) -> Option<DVec3> {
    g.evaluate();
    let id = g.find(name).ok().flatten()?;
    Some(world(g, id).w_axis.truncate())
}

fn control(s: &mut Session, name: &str, at_p: DVec3, group: &str) -> CmdResult<String> {
    let r = s.run("spaceLocator", json!({"name": name}))?;
    let n = r.get(0).and_then(Value::as_str).unwrap_or(name).to_string();
    s.run("move", json!({"args": [n.clone()], "to": at_p.to_array()}))?;
    s.run("parent", json!({"args": [n.clone(), group]}))?;
    Ok(n)
}

fn rig_group(cname: &str) -> String {
    format!("{cname}_Ctrl_Reference")
}

pub(crate) fn has_control_rig(g: &Graph, c: NodeId) -> bool {
    g.find(&rig_group(&g.path(c))).ok().flatten().is_some()
}

/// Returns the controls with the joint each answers to, parents first.
fn create_control_rig(s: &mut Session, cname: &str) -> CmdResult<Vec<(String, String)>> {
    let c = character(&s.graph, cname, "hikCreateControlRig")?;
    let slots = slots_of(&s.graph, c);
    if !slots.contains_key("Hips") {
        fail!("hikCreateControlRig: {cname} has no Hips");
    }
    let grp = rig_group(cname);
    s.run("group", json!({"empty": true, "name": grp}))?;
    let mut made = vec![(grp.clone(), String::new())];
    let mut parent_ctrl = grp.clone();
    let mut order = vec!["Hips".to_string(), "Spine".to_string()];
    for i in 1..=9 {
        order.push(format!("Spine{i}"));
    }
    order.push("Neck".into());
    order.push("Head".into());
    for slot in order {
        let Some(j) = slots.get(&slot).cloned() else { continue };
        let Some(p) = at(&mut s.graph, &j) else { continue };
        let ctrl = control(s, &format!("{cname}_Ctrl_{slot}"), p, &parent_ctrl)?;
        if slot == "Hips" {
            s.run("parentConstraint", json!({"args": [ctrl.clone(), j.clone()], "maintainOffset": true}))?;
        } else {
            s.run("orientConstraint", json!({"args": [ctrl.clone(), j.clone()], "maintainOffset": true}))?;
        }
        made.push((ctrl.clone(), j));
        parent_ctrl = ctrl;
    }
    let hips_ctrl = format!("{cname}_Ctrl_Hips");
    for side in ["Left", "Right"] {
        for (root, mid, end, eff, pole_dir) in [("Arm", "ForeArm", "Hand", "HandEffector", DVec3::NEG_Z), ("UpLeg", "Leg", "Foot", "AnkleEffector", DVec3::Z)] {
            let (Some(r), Some(m), Some(e)) = (slots.get(&format!("{side}{root}")), slots.get(&format!("{side}{mid}")), slots.get(&format!("{side}{end}"))) else { continue };
            let (r, m, e) = (r.clone(), m.clone(), e.clone());
            let (Some(pe), Some(pm)) = (at(&mut s.graph, &e), at(&mut s.graph, &m)) else { continue };
            let ctrl = control(s, &format!("{cname}_Ctrl_{side}{eff}"), pe, &grp)?;
            let pole = control(s, &format!("{cname}_Ctrl_{side}{mid}Pole"), pm + pole_dir * 30.0, &grp)?;
            let h = s.run("ikHandle", json!({"startJoint": r, "endEffector": e.clone(), "solver": "ikRPsolver", "name": format!("{cname}_{side}{end}IK")}))?;
            let hn = h.get(0).and_then(Value::as_str).unwrap_or("").to_string();
            s.run("parent", json!({"args": [hn.clone(), ctrl.clone()]}))?;
            s.run("poleVectorConstraint", json!({"args": [pole.clone(), hn]}))?;
            s.run("orientConstraint", json!({"args": [ctrl.clone(), e.clone()], "maintainOffset": true}))?;
            if root == "Arm" {
                s.run("parent", json!({"args": [pole.clone(), hips_ctrl.clone()]}))?;
            }
            made.push((ctrl, e));
        }
    }
    s.run("select", json!({"args": [grp]}))?;
    Ok(made)
}

fn constraints_on(g: &Graph, joint: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    for (d, src) in g.connections() {
        if d.node == joint && crate::rig::CONSTRAINT_TYPES.contains(&g.node(src.node).map(|n| n.ty().name.as_str()).unwrap_or("")) && !out.contains(&src.node) {
            out.push(src.node);
        }
    }
    out
}

fn delete_control_rig(s: &mut Session, cn: &str) -> CmdResult<()> {
    let c = character(&s.graph, cn, "hikDeleteControlRig")?;
    let joints: Vec<String> = slots_of(&s.graph, c).into_values().collect();
    let mut doomed = Vec::new();
    for j in &joints {
        if let Some(id) = s.graph.find(j)? {
            doomed.extend(constraints_on(&s.graph, id));
        }
    }
    for d in doomed {
        if s.graph.node(d).is_some() {
            let p = s.graph.path(d);
            s.run("delete", json!({"args": [p]}))?;
        }
    }
    let grp = rig_group(cn);
    if s.graph.find(&grp)?.is_some() {
        s.run("delete", json!({"args": [grp]}))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Baking

fn bake_range(s: &mut Session, a: &Args) -> CmdResult<(f64, f64)> {
    let r = crate::anim::range();
    let _ = s;
    Ok(crate::anim::time_range(a)?.filter(|x| x.0.is_finite() && x.1.is_finite()).unwrap_or((r.min, r.max)))
}

fn euler_filter(s: &mut Session, plugs: &[Value]) {
    let curves: Vec<String> = plugs.iter().filter_map(Value::as_str).filter(|p| p.contains(".r")).filter_map(|p| s.graph.plug(p).ok()).filter_map(|p| crate::anim::curve_of(&s.graph, &p)).map(|c| s.graph.path(c)).collect();
    if !curves.is_empty() {
        let _ = s.run("filterCurve", json!({"args": curves}));
    }
}

/// Keys the character's joints with what they evaluate to, and takes them off the solver.
fn bake_to_skeleton(s: &mut Session, a: &Args) -> CmdResult {
    let (_, c) = the_character(s, a, "hikBakeToSkeleton")?;
    let (t0, t1) = bake_range(s, a)?;
    let joints: Vec<String> = slots_of(&s.graph, c).into_iter().filter(|(k, _)| k != "Reference").map(|(_, v)| v).collect();
    let mut plugs: Vec<Value> = Vec::new();
    for j in &joints {
        for n in ["translateX", "translateY", "translateZ", "rotateX", "rotateY", "rotateZ"] {
            plugs.push(json!(format!("{j}.{n}")));
        }
    }
    s.graph.open_chunk("Bake to Skeleton");
    let r = (|| -> CmdResult<Value> {
        let r = s.run("bakeResults", json!({"args": plugs.clone(), "time": format!("{t0}:{t1}"), "sampleBy": a.float("sampleBy", 1.0)}))?;
        release_after_bake(&mut s.graph, c)?;
        // Control rig constraints give way too.
        let doomed: Vec<NodeId> = joints.iter().filter_map(|j| s.graph.find(j).ok().flatten()).flat_map(|id| constraints_on(&s.graph, id)).collect();
        for d in doomed {
            if s.graph.node(d).is_some() {
                let p = s.graph.path(d);
                s.run("delete", json!({"args": [p]}))?;
            }
        }
        if a.get("eulerFilter").and_then(Value::as_bool).unwrap_or(true) {
            euler_filter(s, &plugs);
        }
        Ok(r)
    })();
    s.graph.close_chunk();
    r
}

/// Samples the retarget, builds a control rig on the joints, and keys its controls to the same motion.
fn bake_to_control_rig(s: &mut Session, a: &Args) -> CmdResult {
    let (cn, c) = the_character(s, a, "hikBakeToControlRig")?;
    let (t0, t1) = bake_range(s, a)?;
    let by = a.float("sampleBy", 1.0).max(1e-3);
    let now = crate::anim::frame(&s.graph);
    let ids: Vec<(String, NodeId)> = slots_of(&s.graph, c).into_iter().filter_map(|(k, v)| s.graph.find(&v).ok().flatten().map(|id| (k, id))).collect();
    let mut frames: Vec<(f64, BTreeMap<NodeId, DMat4>)> = Vec::new();
    let mut t = t0;
    while t <= t1 + 1e-9 {
        crate::anim::set_frame(&mut s.graph, t);
        s.graph.evaluate();
        let mut m = BTreeMap::new();
        for (_, id) in &ids {
            m.insert(*id, world(&mut s.graph, *id));
        }
        frames.push((t, m));
        t += by;
    }
    crate::anim::set_frame(&mut s.graph, now);
    s.graph.open_chunk("Bake to Control Rig");
    let r = (|| -> CmdResult<Value> {
        set_source(s, &cn, "None")?;
        s.graph.evaluate();
        if has_control_rig(&s.graph, c) {
            delete_control_rig(s, &cn)?;
        }
        let made = create_control_rig(s, &cn)?;
        s.graph.evaluate();
        // Each control's offset from its joint as built; controls are turned by nothing yet.
        let mut rows: Vec<(NodeId, NodeId, DMat4)> = Vec::new();
        for (ctrl, joint) in &made {
            if joint.is_empty() {
                continue;
            }
            let (Some(cid), Some(jid)) = (s.graph.find(ctrl)?, s.graph.find(joint)?) else { continue };
            let off = world(&mut s.graph, jid).inverse() * world(&mut s.graph, cid);
            rows.push((cid, jid, off));
        }
        for (t, poses) in &frames {
            let mut placed: BTreeMap<NodeId, DMat4> = BTreeMap::new();
            for &(cid, jid, off) in &rows {
                let Some(w) = poses.get(&jid) else { continue };
                let want = *w * off;
                placed.insert(cid, want);
                let up = s.graph.parent(cid);
                let parent = placed.get(&up).copied().unwrap_or_else(|| world(&mut s.graph, up));
                let local = parent.inverse() * want;
                let e = matrix_to_euler(&glam::DMat3::from_quat(rotation_of(&local)), RotateOrder::Xyz);
                let p = position_of(&local);
                let name = s.graph.path(cid);
                for (ch, v) in [("tx", p.x), ("ty", p.y), ("tz", p.z), ("rx", e.x), ("ry", e.y), ("rz", e.z)] {
                    s.run("setKeyframe", json!({"args": [format!("{name}.{ch}")], "time": *t, "value": v}))?;
                }
            }
        }
        Ok(json!(made.len()))
    })();
    s.graph.close_chunk();
    r
}

include!("hik_commands.rs");
