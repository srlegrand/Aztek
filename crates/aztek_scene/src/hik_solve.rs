//! The retarget: a source character's joints in the world to a target character's, both defined in T-pose. HumanIK's own solver
//! isn't documented, so this is ours, to the behaviour the Retarget Specific attributes describe.
//!
//! Each mapped joint turns in the world as its source does, measured from their T-poses. Spines and necks of different
//! joint counts are matched by position along the chain, and a joint with no source follows its parent. The hips move as the
//! source's do, scaled by the Action Space Compensation (Auto: the ratio of hip heights), and the ankles are placed the same
//! way with a two bone solve that keeps the knee on the side the rotations gave it, so feet stay on the floor at any leg length.

use glam::{DMat3, DMat4, DQuat, DVec3};

use crate::hik_slots::{mirror_slot, slot_index, slot_names, SLOT_COUNT};

/// One joint's place in its rig, read when the definition is locked or changes.
#[derive(Debug, Clone)]
pub struct RigSlot {
    pub slot: usize,
    /// The slot it hangs from in the body: its nearest mapped ancestor, or when a rig keeps its limbs under groups of their
    /// own, the slot before it in the body.
    pub parent: Option<usize>,
    /// The nearest mapped ancestor in the hierarchy, which its channels are local to.
    pub local_parent: Option<usize>,
    /// From that ancestor's world to this joint's DAG parent's: unmapped joints between. With no such ancestor, the DAG parent's
    /// world as it stands.
    pub between: DMat4,
    pub joint_orient: DVec3,
    pub rotate_axis: DVec3,
    pub rotate_order: i64,
    pub local_scale: DVec3,
}

/// What a HIKCharacterNode says: the T-pose of each mapped slot, and how its joints hang together, parents first.
#[derive(Debug, Clone, Default)]
pub struct Definition {
    pub stance: Vec<Option<DMat4>>,
    pub rig: Vec<RigSlot>,
}

impl Definition {
    pub fn empty() -> Self {
        Definition { stance: vec![None; SLOT_COUNT], rig: Vec::new() }
    }
    fn has(&self, name: &str) -> Option<usize> {
        slot_index(name).filter(|&i| self.stance[i].is_some())
    }
}

/// World matrices by slot.
pub type State = Vec<Option<DMat4>>;

/// The Retarget Specific attributes the solve reads. Modes are 0 Off, 1 Auto, 2 User.
#[derive(Debug, Clone)]
pub struct Options {
    pub match_source: bool,
    pub mirror: bool,
    pub action_space_mode: i64,
    pub action_space: f64,
    pub hips_level_mode: i64,
    pub hips_level: f64,
    pub feet_spacing_mode: i64,
    pub feet_spacing: f64,
    pub ankle_height_mode: i64,
    pub ankle_height: f64,
    /// Reach translation by limb, 0 to 100: left and right hand, left and right foot. Hands default to 0.
    pub reach_hands: [f64; 2],
}

impl Default for Options {
    fn default() -> Self {
        Options { match_source: false, mirror: false, action_space_mode: 1, action_space: 100.0, hips_level_mode: 1, hips_level: 0.0, feet_spacing_mode: 1, feet_spacing: 0.0, ankle_height_mode: 1, ankle_height: 0.0, reach_hands: [0.0, 0.0] }
    }
}

/// The rotation of a matrix with scale taken out. A mirrored frame is a rotation times -1, as constraints take it apart.
pub fn rotation_of(m: &DMat4) -> DQuat {
    let n = |v: glam::DVec4| {
        let v = v.truncate();
        let l = v.length();
        if l > 1e-12 {
            v / l
        } else {
            DVec3::ZERO
        }
    };
    let mut r = DMat3::from_cols(n(m.x_axis), n(m.y_axis), n(m.z_axis));
    if r.determinant() < 0.0 {
        r *= -1.0;
    }
    DQuat::from_mat3(&r).normalize()
}

pub fn position_of(m: &DMat4) -> DVec3 {
    m.w_axis.truncate()
}

fn scale_of(m: &DMat4) -> DVec3 {
    DVec3::new(m.x_axis.truncate().length(), m.y_axis.truncate().length(), m.z_axis.truncate().length())
}

/// A world matrix with this rotation and place, keeping the stance's scale.
fn compose(q: DQuat, p: DVec3, like: &DMat4) -> DMat4 {
    let flip = DMat3::from_cols(like.x_axis.truncate(), like.y_axis.truncate(), like.z_axis.truncate()).determinant() < 0.0;
    let s = scale_of(like) * if flip { -1.0 } else { 1.0 };
    DMat4::from_scale_rotation_translation(s, q, p)
}

fn mirror_q(q: DQuat) -> DQuat {
    DQuat::from_xyzw(q.x, -q.y, -q.z, q.w)
}

fn mirror_v(v: DVec3) -> DVec3 {
    DVec3::new(-v.x, v.y, v.z)
}

/// The rotation taking a onto b, the short way.
fn arc(a: DVec3, b: DVec3) -> DQuat {
    let (la, lb) = (a.length(), b.length());
    if la < 1e-12 || lb < 1e-12 {
        return DQuat::IDENTITY;
    }
    DQuat::from_rotation_arc(a / la, b / lb)
}

/// How high a character stands: its hips above the lower of its ankles, plus that ankle's height. Falls back to the hips.
fn hip_height(d: &Definition) -> Option<f64> {
    let hips = position_of(&d.stance[d.has("Hips")?]?).y;
    let floor = [d.has("LeftFoot"), d.has("RightFoot")].into_iter().flatten().filter_map(|i| d.stance[i]).map(|m| position_of(&m).y).fold(f64::INFINITY, f64::min);
    let floor = d.has("Reference").and_then(|i| d.stance[i]).map(|m| position_of(&m).y).unwrap_or(if floor.is_finite() { floor.min(0.0) } else { 0.0 });
    let h = hips - floor;
    (h > 1e-9).then_some(h)
}

/// Movements from the source are multiplied by this.
pub fn action_scale(src: &Definition, dst: &Definition, o: &Options) -> f64 {
    if o.match_source {
        return 1.0;
    }
    match o.action_space_mode {
        0 => 1.0,
        2 => o.action_space / 100.0,
        _ => match (hip_height(src), hip_height(dst)) {
            (Some(a), Some(b)) => b / a,
            _ => 1.0,
        },
    }
}

/// Each source slot's turn in the world since its T-pose.
fn source_turns(src: &Definition, from: &State, mirror: bool) -> Vec<Option<DQuat>> {
    let mut out = vec![None; SLOT_COUNT];
    for i in 0..SLOT_COUNT {
        let (Some(rest), Some(now)) = (src.stance[i], from[i]) else { continue };
        let turn = rotation_of(&now) * rotation_of(&rest).inverse();
        if mirror {
            out[mirror_slot(i)] = Some(mirror_q(turn));
        } else {
            out[i] = Some(turn);
        }
    }
    out
}

/// The mapped members of a numbered chain, in order: Spine, Spine1 ... or Neck, Neck1 ...
fn chain_of(d: &Definition, word: &str) -> Vec<usize> {
    let mut out = Vec::new();
    if let Some(i) = d.has(word) {
        out.push(i);
    }
    for k in 1..=9 {
        if let Some(i) = d.has(&format!("{word}{k}")) {
            out.push(i);
        }
    }
    out
}

/// A spine or neck joint's turn, by where it sits along the chain, so joint counts may differ.
fn chain_turns(src: &Definition, dst: &Definition, turns: &[Option<DQuat>], word: &str, below: Option<DQuat>, out: &mut [Option<DQuat>]) {
    let (s, d) = (chain_of(src, word), chain_of(dst, word));
    if s.is_empty() {
        return;
    }
    let (n, m) = (s.len() as f64, d.len() as f64);
    for (i, &slot) in d.iter().enumerate() {
        let u = (i as f64 + 1.0) / m * n - 1.0;
        let turn_at = |k: i64| -> Option<DQuat> {
            if k < 0 {
                below
            } else {
                turns[s[(k as usize).min(s.len() - 1)]]
            }
        };
        let lo = u.floor() as i64;
        let f = u - lo as f64;
        let q = match (turn_at(lo), turn_at(lo + 1)) {
            (Some(a), Some(b)) => Some(a.slerp(b, f)),
            (a, b) => a.or(b),
        };
        if q.is_some() {
            out[slot] = q;
        }
    }
}

/// Places a whole body by its turns, parents first. `set` holds world rotations already decided, by slot.
fn walk(dst: &Definition, turns: &[Option<DQuat>], set: &[Option<DQuat>], hips_at: Option<(usize, DVec3)>) -> State {
    let mut out: State = vec![None; SLOT_COUNT];
    for r in &dst.rig {
        let Some(rest) = dst.stance[r.slot] else { continue };
        let rest_q = rotation_of(&rest);
        let rest_p = position_of(&rest);
        let (parent_q, parent_rest_q, parent_p, parent_rest_p) = match r.parent.and_then(|p| Some((out[p]?, dst.stance[p]?))) {
            Some((w, st)) => (rotation_of(&w), rotation_of(&st), position_of(&w), position_of(&st)),
            None => (DQuat::IDENTITY, DQuat::IDENTITY, DVec3::ZERO, DVec3::ZERO),
        };
        let parent_turn = parent_q * parent_rest_q.inverse();
        let q = match (set[r.slot], turns[r.slot]) {
            (Some(world), _) => world,
            (None, Some(turn)) => turn * rest_q,
            // No source: it follows its parent.
            (None, None) => parent_turn * rest_q,
        };
        let p = match hips_at {
            Some((slot, at)) if slot == r.slot => at,
            _ if r.parent.is_some() => parent_p + parent_turn * (rest_p - parent_rest_p),
            _ => rest_p,
        };
        out[r.slot] = Some(compose(q, p, &rest));
    }
    out
}

/// A two bone solve from the upper joint's place to `target`, keeping the plane the rotations gave the knee or elbow. Sets the
/// world rotations of the upper and lower joints.
fn two_bone(body: &State, dst: &Definition, chain: [usize; 3], target: DVec3, set: &mut [Option<DQuat>]) {
    let [up, mid, end] = chain;
    let (Some(wu), Some(wm), Some(we)) = (body[up], body[mid], body[end]) else { return };
    let (pu, pm, pe) = (position_of(&wu), position_of(&wm), position_of(&we));
    let (a, b) = ((pm - pu).length(), (pe - pm).length());
    if a < 1e-9 || b < 1e-9 {
        return;
    }
    let reach = target - pu;
    let c = reach.length().clamp((a - b).abs() + 1e-6, a + b - 1e-6);
    let dir = if reach.length() > 1e-9 { reach / reach.length() } else { (pe - pu).normalize_or_zero() };
    // Which way the joint bends: the rotations' answer, or at full stretch the T-pose's bend turned the same way.
    let line = (pe - pu).normalize_or_zero();
    let mut bend = (pm - pu) - line * (pm - pu).dot(line);
    if bend.length() < 1e-4 * a {
        let (Some(su), Some(sm), Some(se)) = (dst.stance[up], dst.stance[mid], dst.stance[end]) else { return };
        let (ru, rm, re) = (position_of(&su), position_of(&sm), position_of(&se));
        let rl = (re - ru).normalize_or_zero();
        let rest_bend = (rm - ru) - rl * (rm - ru).dot(rl);
        bend = (rotation_of(&wu) * rotation_of(&su).inverse()) * rest_bend;
        if bend.length() < 1e-9 {
            // Straight in the T-pose too: the knee goes the way the rotated joint's forward says.
            bend = rotation_of(&wu) * rotation_of(&su).inverse() * DVec3::Z;
        }
    }
    let side = bend - dir * bend.dot(dir);
    let side = if side.length() > 1e-9 { side / side.length() } else { dir.any_orthonormal_vector() };
    let along = (a * a - b * b + c * c) / (2.0 * c);
    let height = (a * a - along * along).max(0.0).sqrt();
    let new_mid = pu + dir * along + side * height;
    let new_end = pu + dir * c;
    let q_up = arc(pm - pu, new_mid - pu) * rotation_of(&wu);
    let turned_lower = arc(pm - pu, new_mid - pu) * (pe - pm);
    let q_mid = arc(turned_lower, new_end - new_mid) * arc(pm - pu, new_mid - pu) * rotation_of(&wm);
    set[up] = Some(q_up.normalize());
    set[mid] = Some(q_mid.normalize());
}

/// The target's world pose for the source's.
pub fn retarget(src: &Definition, from: &State, dst: &Definition, o: &Options) -> State {
    let scale = action_scale(src, dst, o);
    let turns = source_turns(src, from, o.mirror);
    // The target's turns: same slot, by position along the spine and neck.
    let mut mine: Vec<Option<DQuat>> = vec![None; SLOT_COUNT];
    for i in 0..SLOT_COUNT {
        if dst.stance[i].is_some() {
            mine[i] = turns[i];
        }
    }
    // The Reference stays where the target's is.
    mine[0] = None;
    let hips_slot = slot_index("Hips").unwrap();
    chain_turns(src, dst, &turns, "Spine", turns[hips_slot], &mut mine);
    let top_spine = chain_of(src, "Spine").last().and_then(|&i| turns[i]);
    chain_turns(src, dst, &turns, "Neck", top_spine, &mut mine);

    let moved = |slot: usize| -> Option<DVec3> {
        let from_slot = if o.mirror { mirror_slot(slot) } else { slot };
        let d = position_of(&from[from_slot]?) - position_of(&src.stance[from_slot]?);
        Some(if o.mirror { mirror_v(d) } else { d })
    };
    let rest_of = |slot: usize| dst.stance[slot].map(|m| position_of(&m));
    // Match Source: the same spots in the world as the source.
    let wanted = |slot: usize| -> Option<DVec3> {
        if o.match_source {
            let from_slot = if o.mirror { mirror_slot(slot) } else { slot };
            let p = position_of(&from[from_slot]?);
            return Some(if o.mirror { mirror_v(p) } else { p });
        }
        Some(rest_of(slot)? + moved(slot)? * scale)
    };
    let mut hips_at = wanted(hips_slot);
    if o.hips_level_mode == 2 {
        hips_at = hips_at.map(|p| p + DVec3::Y * o.hips_level);
    }
    let hips = hips_at.map(|p| (hips_slot, p));

    let mut set: Vec<Option<DQuat>> = vec![None; SLOT_COUNT];
    let body = walk(dst, &mine, &set, hips);
    // The ankles go where the source's say; with Ankle Height Comp off the legs stay as the rotations left them.
    if o.ankle_height_mode != 0 {
        for (side, sign) in [("Left", 1.0), ("Right", -1.0)] {
            let ids = [format!("{side}UpLeg"), format!("{side}Leg"), format!("{side}Foot")];
            let (Some(a), Some(b), Some(c)) = (dst.has(&ids[0]), dst.has(&ids[1]), dst.has(&ids[2])) else { continue };
            let Some(mut at) = wanted(c) else { continue };
            if o.ankle_height_mode == 2 {
                at.y += o.ankle_height;
            }
            if o.feet_spacing_mode == 2 {
                at.x += sign * o.feet_spacing;
            }
            two_bone(&body, dst, [a, b, c], at, &mut set);
        }
    }
    // Reach: a hand pulled toward where the source's goes, by the amount asked.
    for (k, side) in ["Left", "Right"].iter().enumerate() {
        let reach = (o.reach_hands[k] / 100.0).clamp(0.0, 1.0);
        if reach <= 0.0 {
            continue;
        }
        let ids = [format!("{side}Arm"), format!("{side}ForeArm"), format!("{side}Hand")];
        let (Some(a), Some(b), Some(c)) = (dst.has(&ids[0]), dst.has(&ids[1]), dst.has(&ids[2])) else { continue };
        let (Some(at), Some(now)) = (wanted(c), body[c]) else { continue };
        two_bone(&body, dst, [a, b, c], position_of(&now).lerp(at, reach), &mut set);
    }
    // The feet and hands keep their turns; what is under a solved joint follows from the new places.
    let mut out = walk(dst, &mine, &set, hips);
    // A slot with a turn of its own under a solved joint already kept it in walk; roll joints with none follow.
    let _ = slot_names();
    for r in &dst.rig {
        if out[r.slot].is_none() {
            out[r.slot] = dst.stance[r.slot];
        }
    }
    out
}

/// A target joint's local channels from the state: translate, rotate in degrees near `near`, as its parent's world says.
pub fn to_local(r: &RigSlot, world: &DMat4, parent_world: &DMat4, near: DVec3) -> (DVec3, DVec3) {
    use crate::transform::{euler_near, euler_to_matrix, RotateOrder};
    let up = *parent_world * r.between;
    let t = up.inverse().transform_point3(position_of(world));
    let local = rotation_of(&up).inverse() * rotation_of(world);
    let jo = DQuat::from_mat4(&euler_to_matrix(r.joint_orient, RotateOrder::Xyz));
    let ra = DQuat::from_mat4(&euler_to_matrix(r.rotate_axis, RotateOrder::Xyz));
    let q = jo.inverse() * local * ra.inverse();
    let e = euler_near(&DMat3::from_quat(q.normalize()), RotateOrder::from_index(r.rotate_order), near);
    (t, e)
}
