//! Joints, skinClusters and blend shapes through UsdSkel: Skeleton, SkelAnimation, BlendShape and the bindings on meshes.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_scene::deform_blend::{self, BlendData};
use aztek_scene::graph::Graph;
use aztek_scene::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use aztek_scene::session::Session;
use aztek_scene::transform::{decompose_full, DagNode};
use aztek_scene::{history, rig, skin};
use glam::{DMat4, DQuat, DVec3};
use openusd::gf;
use openusd::sdf::{Specifier, Value};
use openusd::tf::Token;

use crate::export::{Opts, Tracks};
use crate::export_geo::clean;
use crate::out::{Out, Track};
use crate::util::*;

pub struct SkelInfo {
    pub joints: Vec<NodeId>,
    pub tokens: Vec<String>,
    pub path: String,
    pub anim: String,
    pub bind: Vec<DMat4>,
    pub rest: Vec<DMat4>,
    /// The joints' parent above the skeleton's root, for the root's local matrix.
    pub above: NodeId,
    pub blend_names: Vec<String>,
    pub blend_plugs: Vec<PlugRef>,
    pub animated: bool,
}

pub struct Binding {
    pub shape: NodeId,
    pub prim: String,
    pub skeleton: usize,
    pub skin: Option<NodeId>,
    pub blend: Option<(NodeId, BlendData)>,
}

#[derive(Default)]
pub struct Plan {
    pub parent_override: HashMap<NodeId, String>,
    pub new_prims: Vec<(String, String)>,
    pub skel_roots: HashMap<NodeId, ()>,
    pub rest: HashMap<NodeId, Arc<Mesh>>,
    pub skels: Vec<SkelInfo>,
    pub bound: Vec<Binding>,
}

fn all_below(g: &Graph, roots: &[NodeId]) -> Vec<NodeId> {
    let mut out = Vec::new();
    for &r in roots {
        out.push(r);
        out.extend(g.descendants(r));
    }
    out
}

fn ancestors(g: &Graph, mut n: NodeId) -> Vec<NodeId> {
    let mut v = Vec::new();
    while n != NO_NODE {
        v.push(n);
        n = g.parent(n);
    }
    v.reverse();
    v
}

fn blend_of_mesh(g: &Graph, shape: NodeId) -> Option<NodeId> {
    history::history_of(g, shape).into_iter().find(|&h| g.node(h).is_some_and(|n| n.ty().name == "blendShape"))
}

/// The prim path a transform will have, given the overrides so far.
fn path_of(g: &Graph, o: &Opts, base: &str, node: NodeId, ov: &HashMap<NodeId, String>) -> String {
    if node == NO_NODE {
        return base.to_string();
    }
    let name = prim_name(g.node(node).map_or("", |n| n.name()), o.strip_namespaces);
    let parent = match ov.get(&node) {
        Some(p) => p.clone(),
        None => path_of(g, o, base, g.parent(node), ov),
    };
    format!("{parent}/{name}")
}

fn joint_tree(g: &Graph, j: NodeId, prefix: &str, o: &Opts, joints: &mut Vec<NodeId>, tokens: &mut Vec<String>) {
    let name = prim_name(g.node(j).map_or("", |n| n.name()), o.strip_namespaces);
    let token = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
    joints.push(j);
    tokens.push(token.clone());
    for &c in g.children(j) {
        if rig::is_joint(g, c) {
            joint_tree(g, c, &token, o, joints, tokens);
        }
    }
}

/// Where everything goes, decided before the walk because a SkelRoot may have to be put above what it covers.
pub fn plan(s: &mut Session, o: &Opts, roots: &[NodeId], base: &str) -> Result<Plan, String> {
    let mut plan = Plan::default();
    let skels_on = o.skels != "none";
    let skin_on = o.skin != "none" || skels_on;
    if !skels_on && !o.export_blend_shapes {
        return Ok(plan);
    }
    let everything = all_below(&s.graph, roots);
    let g = &s.graph;
    let mut root_joints: Vec<NodeId> = if skels_on {
        everything.iter().copied().filter(|&n| rig::is_joint(g, n) && !rig::is_joint(g, g.parent(n))).collect()
    } else {
        Vec::new()
    };
    root_joints.dedup();
    let meshes: Vec<NodeId> = everything.iter().copied().filter(|&n| g.node(n).is_some_and(|x| x.ty().name == "mesh" && !x.bool(DagNode::INTERMEDIATE_OBJECT))).collect();

    // Meshes by the root joint that drives them.
    let mut by_root: HashMap<NodeId, Vec<NodeId>> = HashMap::new();
    let mut skin_of: HashMap<NodeId, NodeId> = HashMap::new();
    let mut blend_only: Vec<NodeId> = Vec::new();
    for &m in &meshes {
        let sk = if skin_on { skin::skin_of(g, m) } else { None };
        let bl = if o.export_blend_shapes { blend_of_mesh(g, m) } else { None };
        if let Some(sk) = sk {
            if let Some((_, j)) = skin::influences(g, sk).first().copied() {
                let mut top = j;
                while rig::is_joint(g, g.parent(top)) {
                    top = g.parent(top);
                }
                if root_joints.contains(&top) {
                    by_root.entry(top).or_default().push(m);
                    skin_of.insert(m, sk);
                    continue;
                }
            }
        }
        if bl.is_some() {
            blend_only.push(m);
        }
    }

    let mut ov: HashMap<NodeId, String> = HashMap::new();
    let mut skel_path: HashMap<NodeId, usize> = HashMap::new();
    for &r in &root_joints {
        // The SkelRoot sits on the lowest group over the joint and its meshes, or a new one when they share none.
        let mut lists: Vec<Vec<NodeId>> = vec![ancestors(g, g.parent(r))];
        for &m in by_root.get(&r).map_or(&[][..], |v| &v[..]) {
            lists.push(ancestors(g, g.parent(g.parent(m))));
        }
        let mut common = NO_NODE;
        for i in 0.. {
            let Some(first) = lists[0].get(i).copied() else { break };
            if lists.iter().all(|l| l.get(i) == Some(&first)) {
                common = first;
            } else {
                break;
            }
        }
        if common != NO_NODE {
            plan.skel_roots.insert(common, ());
        } else {
            let name = prim_name(g.node(r).map_or("", |n| n.name()), o.strip_namespaces);
            let holder = format!("{base}/{name}_skelRoot");
            plan.new_prims.push((holder.clone(), "SkelRoot".into()));
            ov.insert(r, holder.clone());
            for &m in by_root.get(&r).map_or(&[][..], |v| &v[..]) {
                let top = ancestors(g, m).first().copied().unwrap_or(NO_NODE);
                ov.insert(top, holder.clone());
            }
        }
        let path = path_of(g, o, base, r, &ov);
        let (mut joints, mut tokens) = (Vec::new(), Vec::new());
        joint_tree(g, r, "", o, &mut joints, &mut tokens);
        skel_path.insert(r, plan.skels.len());
        plan.skels.push(SkelInfo { anim: format!("{path}/Animation"), path, joints, tokens, bind: Vec::new(), rest: Vec::new(), above: g.parent(r), blend_names: Vec::new(), blend_plugs: Vec::new(), animated: false });
    }

    for &m in &blend_only {
        // Blend shapes need a SkelRoot and a Skeleton even with no joints.
        let x = g.parent(m);
        let name = prim_name(g.node(x).map_or("", |n| n.name()), o.strip_namespaces);
        let parent = path_of(g, o, base, g.parent(x), &ov);
        let holder = format!("{parent}/{name}_skelRoot");
        plan.new_prims.push((holder.clone(), "SkelRoot".into()));
        ov.insert(x, holder.clone());
        let path = format!("{holder}/Skel");
        plan.skels.push(SkelInfo { anim: format!("{path}/Animation"), path, joints: Vec::new(), tokens: Vec::new(), bind: Vec::new(), rest: Vec::new(), above: NO_NODE, blend_names: Vec::new(), blend_plugs: Vec::new(), animated: false });
        let idx = plan.skels.len() - 1;
        plan.bound.push(Binding { shape: m, prim: String::new(), skeleton: idx, skin: None, blend: None });
    }
    for (&r, list) in &by_root {
        for &m in list {
            plan.bound.push(Binding { shape: m, prim: String::new(), skeleton: skel_path[&r], skin: skin_of.get(&m).copied(), blend: None });
        }
    }
    plan.parent_override = ov.clone();

    // Rest points: deformers off for a moment.
    let bound: Vec<(NodeId, Option<NodeId>)> = plan.bound.iter().map(|b| (b.shape, b.skin)).collect();
    for (m, sk) in bound {
        let bl = if o.export_blend_shapes { blend_of_mesh(&s.graph, m) } else { None };
        let mut off: Vec<(NodeId, f64)> = Vec::new();
        for d in [sk, bl].into_iter().flatten() {
            off.push((d, get_f64(&mut s.graph, d, "envelope").unwrap_or(1.0)));
        }
        s.graph.without_undo(|g| {
            for (d, _) in &off {
                set_attr(g, *d, "envelope", PlugValue::Float(0.0));
            }
        });
        s.graph.evaluate();
        let rest = crate::util::evaluated_mesh(&mut s.graph, m).map(clean);
        s.graph.without_undo(|g| {
            for (d, v) in &off {
                set_attr(g, *d, "envelope", PlugValue::Float(*v));
            }
        });
        s.graph.evaluate();
        if let Some(r) = rest {
            plan.rest.insert(m, r);
        }
    }

    // Bind and rest poses, blend data, and each binding's prim.
    let pv: Vec<usize> = (0..plan.skels.len()).collect();
    for i in pv {
        let joints = plan.skels[i].joints.clone();
        let above = plan.skels[i].above;
        let space = if above == NO_NODE { DMat4::IDENTITY } else { rig::world(&mut s.graph, above) };
        let mut bind: Vec<DMat4> = joints.iter().map(|&j| rig::world(&mut s.graph, j)).collect();
        for b in plan.bound.iter().filter(|b| b.skeleton == i) {
            let Some(sk) = b.skin else { continue };
            let Some(data) = skin::bind_of(&s.graph, sk) else { continue };
            for (el, j) in skin::influences(&s.graph, sk) {
                if let (Some(pos), Some(pre)) = (joints.iter().position(|&x| x == j), data.bind_pre.get(el as usize)) {
                    bind[pos] = pre.inverse();
                }
            }
        }
        // USD's bind poses are in the skeleton's space.
        for b in &mut bind {
            *b = space.inverse() * *b;
        }
        // A joint's rest: its bind pose relative to its parent's.
        let rest: Vec<DMat4> = joints
            .iter()
            .enumerate()
            .map(|(k, &j)| {
                let up = s.graph.parent(j);
                match joints.iter().position(|&x| x == up) {
                    Some(pk) => bind[pk].inverse() * bind[k],
                    None => bind[k],
                }
            })
            .collect();
        plan.skels[i].bind = bind;
        plan.skels[i].rest = rest;
    }
    for b in 0..plan.bound.len() {
        let shape = plan.bound[b].shape;
        let g = &s.graph;
        let x = g.parent(shape);
        let shapes = crate::export::shapes_of(g, x);
        let xp = path_of(g, o, base, x, &plan.parent_override);
        plan.bound[b].prim = if shapes.len() == 1 && o.merge { xp } else { format!("{xp}/{}", prim_name(g.node(shape).map_or("", |n| n.name()), o.strip_namespaces)) };
        if o.export_blend_shapes {
            if let Some(bl) = blend_of_mesh(g, shape) {
                let data = deform_blend::blend_of(g, bl);
                let sk = plan.bound[b].skeleton;
                let attr = g.node(bl).and_then(|n| n.ty().attr_index("weight")).unwrap_or(0) as i32;
                let targets: Vec<(String, i32)> = data.targets.iter().map(|t| (t.name.clone(), t.index)).collect();
                for (name, idx) in targets {
                    plan.skels[sk].blend_names.push(prim_name(&name, false));
                    plan.skels[sk].blend_plugs.push(PlugRef::element(bl, attr, idx));
                }
                plan.bound[b].blend = Some((bl, data));
            }
        }
    }
    let _seen: HashSet<NodeId> = HashSet::new();
    Ok(plan)
}

/// One frame of each skeleton's animation.
pub fn sample(s: &mut Session, _o: &Opts, plan: &Plan, time: Option<f64>, t: &mut Tracks) {
    for sk in &plan.skels {
        let n = sk.joints.len();
        let (mut tr, mut ro, mut sc) = (Vec::with_capacity(n), Vec::with_capacity(n), Vec::with_capacity(n));
        for &j in &sk.joints {
            let up = s.graph.parent(j);
            let w = rig::world(&mut s.graph, j);
            let pw = if up == NO_NODE { DMat4::IDENTITY } else { rig::world(&mut s.graph, up) };
            let (pos, rot, scale, _) = decompose_full(&(pw.inverse() * w));
            let q = DQuat::from_mat3(&rot).normalize();
            tr.push(v3f(pos));
            ro.push(gf::Quatf { w: q.w as f32, x: q.x as f32, y: q.y as f32, z: q.z as f32 });
            sc.push(half3(scale));
        }
        if n > 0 {
            t.put(&sk.anim, "translations", "float3[]", time, Value::Vec3fVec(tr));
            t.put(&sk.anim, "rotations", "quatf[]", time, Value::QuatfVec(ro));
            t.put(&sk.anim, "scales", "half3[]", time, Value::Vec3hVec(sc));
        }
        if !sk.blend_plugs.is_empty() {
            let w: Vec<f32> = sk.blend_plugs.iter().map(|p| s.graph.value(p).ok().and_then(|v| v.scalar()).unwrap_or(0.0) as f32).collect();
            t.put(&sk.anim, "blendShapeWeights", "float[]", time, Value::FloatVec(w));
        }
    }
}

fn half3(v: DVec3) -> gf::Vec3h {
    gf::vec3h(gf::f16::from_f64(v.x), gf::f16::from_f64(v.y), gf::f16::from_f64(v.z))
}

pub fn write(s: &mut Session, o: &Opts, plan: &Plan, out: &mut Out, _t: &mut Tracks) -> Result<(), String> {
    for sk in &plan.skels {
        out.prim(&sk.path, Specifier::Def, "Skeleton")?;
        out.apply_api(&sk.path, "SkelBindingAPI")?;
        let tokens: Vec<Token> = sk.tokens.iter().map(|t| t.as_str().into()).collect();
        out.uniform(&sk.path, "joints", "token[]", Value::TokenVec(tokens.clone()))?;
        let mats = |v: &[DMat4]| Value::Matrix4dVec(v.iter().map(to_gf).collect());
        out.uniform(&sk.path, "bindTransforms", "matrix4d[]", mats(&sk.bind))?;
        out.uniform(&sk.path, "restTransforms", "matrix4d[]", mats(&sk.rest))?;
        out.prim(&sk.anim, Specifier::Def, "SkelAnimation")?;
        out.uniform(&sk.anim, "joints", "token[]", Value::TokenVec(tokens))?;
        if !sk.blend_names.is_empty() {
            let names: Vec<Token> = sk.blend_names.iter().map(|t| t.as_str().into()).collect();
            out.uniform(&sk.anim, "blendShapes", "token[]", Value::TokenVec(names))?;
        }
        out.rel(&sk.path, "skel:animationSource", &[sk.anim.clone()])?;
    }
    for b in &plan.bound {
        if !out.has_prim(&b.prim) {
            continue;
        }
        let sk = &plan.skels[b.skeleton];
        out.apply_api(&b.prim, "SkelBindingAPI")?;
        out.rel(&b.prim, "skel:skeleton", &[sk.path.clone()])?;
        if let Some(sc) = b.skin {
            write_skin(s, o, sc, &b.prim, sk, out)?;
        }
        if let Some((_, data)) = &b.blend {
            write_blends(data, &b.prim, out)?;
        }
    }
    Ok(())
}

fn write_skin(s: &mut Session, _o: &Opts, sc: NodeId, prim: &str, sk: &SkelInfo, out: &mut Out) -> Result<(), String> {
    let Some(bind) = skin::bind_of(&s.graph, sc) else { return Ok(()) };
    let inf = skin::influences(&s.graph, sc);
    let mut slot: HashMap<i32, i32> = HashMap::new();
    let mut names: Vec<Token> = Vec::new();
    for (k, (el, j)) in inf.iter().enumerate() {
        slot.insert(*el, k as i32);
        let tok = sk.joints.iter().position(|x| x == j).map_or_else(String::new, |p| sk.tokens[p].clone());
        names.push(tok.as_str().into());
    }
    let width = bind.weights.iter().map(Vec::len).max().unwrap_or(1).max(1);
    let (mut ji, mut jw) = (Vec::with_capacity(bind.weights.len() * width), Vec::with_capacity(bind.weights.len() * width));
    for pw in &bind.weights {
        let mut row: Vec<(u32, f64)> = pw.clone();
        row.sort_by(|a, b| b.1.total_cmp(&a.1));
        for k in 0..width {
            match row.get(k) {
                Some((i, w)) => {
                    ji.push(slot.get(&(*i as i32)).copied().unwrap_or(0));
                    jw.push(*w as f32);
                }
                None => {
                    ji.push(0);
                    jw.push(0.0);
                }
            }
        }
    }
    let ix = Track { default: Some(Value::IntVec(ji)), samples: Vec::new() };
    out.primvar(prim, "skel:jointIndices", "int[]", &ix, "vertex", None, width as i32)?;
    let wt = Track { default: Some(Value::FloatVec(jw)), samples: Vec::new() };
    out.primvar(prim, "skel:jointWeights", "float[]", &wt, "vertex", None, width as i32)?;
    let space = if sk.above == NO_NODE { DMat4::IDENTITY } else { rig::world(&mut s.graph, sk.above) };
    let geom = Track { default: Some(Value::Matrix4d(to_gf(&(space.inverse() * bind.geom)))), samples: Vec::new() };
    out.attr_track(prim, "primvars:skel:geomBindTransform", "matrix4d", &geom)?;
    out.uniform(prim, "skel:joints", "token[]", Value::TokenVec(names))?;
    Ok(())
}

fn write_blends(data: &BlendData, prim: &str, out: &mut Out) -> Result<(), String> {
    let mut names = Vec::new();
    let mut targets = Vec::new();
    for t in &data.targets {
        let name = prim_name(&t.name, false);
        let path = format!("{prim}/{name}");
        out.prim(&path, Specifier::Def, "BlendShape")?;
        // Inbetweens share the target's point list in USD, so it covers every item's points.
        let mut pts: Vec<u32> = t.items.iter().flat_map(|i| i.deltas.iter().map(|(p, _)| *p)).collect();
        pts.sort_unstable();
        pts.dedup();
        let at: HashMap<u32, usize> = pts.iter().enumerate().map(|(k, p)| (*p, k)).collect();
        let list = |deltas: &[(u32, DVec3)]| -> Vec<gf::Vec3f> {
            let mut v = vec![gf::vec3f(0.0, 0.0, 0.0); pts.len()];
            for (p, d) in deltas {
                v[at[p]] = v3f(*d);
            }
            v
        };
        for item in &t.items {
            if item.iti == 6000 {
                out.attr(&path, "offsets", "vector3f[]", Value::Vec3fVec(list(&item.deltas)))?;
            } else {
                let w = (item.iti - 5000) as f32 / 1000.0;
                let ib = format!("inbetweens:w{}", item.iti);
                let pp = out.attr(&path, &ib, "vector3f[]", Value::Vec3fVec(list(&item.deltas)))?;
                out.attr_field(&pp, "weight", Value::Float(w));
            }
        }
        out.attr(&path, "normalOffsets", "vector3f[]", Value::Vec3fVec(vec![gf::vec3f(0.0, 0.0, 0.0); pts.len()]))?;
        out.uniform(&path, "pointIndices", "int[]", Value::IntVec(pts.iter().map(|p| *p as i32).collect()))?;
        names.push(Token::from(name.as_str()));
        targets.push(path);
    }
    out.uniform(prim, "skel:blendShapes", "token[]", Value::TokenVec(names))?;
    out.rel(prim, "skel:blendShapeTargets", &targets)
}
