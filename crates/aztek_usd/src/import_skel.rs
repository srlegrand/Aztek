//! Skeleton prims as joints, UsdSkel bindings as skinClusters, BlendShape prims as a blendShape deformer, and SkelAnimation
//! as keys.

use std::collections::HashMap;

use aztek_scene::anim::{self, Tangent};
use aztek_scene::deform_blend::{self, BlendData, BlendItem, BlendTarget};
use aztek_scene::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use aztek_scene::node::types;
use aztek_scene::skin;
use aztek_scene::transform::{decompose_full, euler_near, RotateOrder};
use glam::{DMat3, DMat4, DQuat, DVec3};
use openusd::gf;
use openusd::tf::Token;
use openusd::usd::{Prim, PrimPredicate, TimeCode};
use serde_json::json;

use crate::import::Ctx;
use crate::mesh_read::{at, get_vec};
use crate::util::*;

struct Skel {
    tokens: Vec<String>,
    joints: Vec<NodeId>,
    bind: Vec<DMat4>,
    /// What the root joints stand under: the skeleton's space.
    holder: NodeId,
}

fn tokens_of(prim: &Prim, name: &str) -> Vec<String> {
    prim.attribute(name).get::<Vec<Token>>().ok().flatten().map(|v| v.iter().map(|t| t.as_str().to_string()).collect()).unwrap_or_default()
}

fn mats(prim: &Prim, name: &str) -> Vec<DMat4> {
    get_vec::<gf::Matrix4d>(&prim.attribute(name), None).map(|v| v.iter().map(from_gf).collect()).unwrap_or_default()
}

/// The nearest ancestor brought in as a node.
fn node_above(c: &Ctx, path: &openusd::sdf::Path) -> Option<NodeId> {
    let mut p = path.parent();
    while let Some(q) = p {
        if let Some(n) = c.nodes.get(q.as_str()) {
            return Some(*n);
        }
        p = q.parent();
    }
    None
}

fn set_joint(c: &mut Ctx, j: NodeId, m: &DMat4) {
    let (t, r, s, _) = decompose_full(m);
    let e = aztek_scene::transform::matrix_to_euler(&r, RotateOrder::Xyz);
    set_attr(&mut c.s.graph, j, "translate", PlugValue::Vec3(t));
    set_attr(&mut c.s.graph, j, "rotate", PlugValue::Vec3(e));
    set_attr(&mut c.s.graph, j, "scale", PlugValue::Vec3(s));
}

fn read_skeleton(c: &mut Ctx, prim: &Prim, parent: NodeId) -> Result<Skel, String> {
    let tokens = tokens_of(prim, "joints");
    let bind = mats(prim, "bindTransforms");
    let mut rest = mats(prim, "restTransforms");
    let jt = types::find("joint").ok_or("no joint type")?;
    // A skeleton's own transform gets a node: joints live in its space.
    let parent = if prim.attribute("xformOpOrder").has_authored_value().unwrap_or(false) {
        crate::import::read_group(c, prim, parent)?
    } else {
        parent
    };
    let mut joints = vec![NO_NODE; tokens.len()];
    for (i, tok) in tokens.iter().enumerate() {
        let (up, name) = match tok.rsplit_once('/') {
            Some((u, n)) => (tokens.iter().position(|t| t == u).map_or(parent, |k| joints[k]), n),
            None => (parent, tok.as_str()),
        };
        let j = c.s.graph.create_node(&jt, &node_name(name), up, false).map_err(|e| e.to_string())?;
        // USD has no scale compensation: a parent's scale reaches its children.
        set_attr(&mut c.s.graph, j, "segmentScaleCompensate", PlugValue::Bool(false));
        joints[i] = j;
    }
    // A file with no rest pose stands in its bind pose.
    if rest.len() != tokens.len() {
        rest = (0..tokens.len())
            .map(|i| {
                let up = tokens[i].rsplit_once('/').and_then(|(u, _)| tokens.iter().position(|t| t == u));
                let b = bind.get(i).copied().unwrap_or(DMat4::IDENTITY);
                up.and_then(|k| bind.get(k)).map_or(b, |pb| pb.inverse() * b)
            })
            .collect();
    }
    for (i, m) in rest.iter().enumerate() {
        set_joint(c, joints[i], m);
    }
    Ok(Skel { tokens, joints, bind, holder: parent })
}

fn read_animation(c: &mut Ctx, skel: &Skel, anim_prim: &Prim) {
    let order = tokens_of(anim_prim, "joints");
    let map: Vec<Option<NodeId>> = order.iter().map(|t| skel.tokens.iter().position(|x| x == t).map(|k| skel.joints[k])).collect();
    let (ta, ra, sa) = (anim_prim.attribute("translations"), anim_prim.attribute("rotations"), anim_prim.attribute("scales"));
    let mut times: Vec<f64> = Vec::new();
    for a in [&ta, &ra, &sa] {
        times.extend(a.time_sample_times().unwrap_or_default());
    }
    times.sort_by(f64::total_cmp);
    times.dedup();
    if let Some((lo, hi)) = c.o.frame_range {
        times.retain(|t| *t >= lo && *t <= hi);
    }
    let animated = c.o.read_anim && times.len() > 1;
    let at_times: Vec<Option<f64>> = if animated { times.iter().map(|t| Some(*t)).collect() } else { vec![None] };
    let mut prev: Vec<DVec3> = vec![DVec3::ZERO; order.len()];
    let mut series: Vec<Vec<(f64, DVec3, DVec3, DVec3)>> = vec![Vec::new(); order.len()];
    for t in &at_times {
        let tr = get_vec::<gf::Vec3f>(&ta, *t).unwrap_or_default();
        let ro = get_vec::<gf::Quatf>(&ra, *t).unwrap_or_default();
        let sc = get_vec::<gf::Vec3h>(&sa, *t).unwrap_or_default();
        for k in 0..order.len() {
            let pos = tr.get(k).map_or(DVec3::ZERO, |p| DVec3::new(p.x as f64, p.y as f64, p.z as f64));
            let q = ro.get(k).map_or(DQuat::IDENTITY, |q| DQuat::from_xyzw(q.x as f64, q.y as f64, q.z as f64, q.w as f64).normalize());
            let e = euler_near(&DMat3::from_quat(q), RotateOrder::Xyz, prev[k]);
            prev[k] = e;
            let s = sc.get(k).map_or(DVec3::ONE, |s| DVec3::new(s.x.to_f64(), s.y.to_f64(), s.z.to_f64()));
            series[k].push((t.unwrap_or(0.0), pos, e, s));
        }
    }
    for (k, node) in map.iter().enumerate() {
        let Some(j) = *node else { continue };
        for (attr, pick) in [("translate", 1usize), ("rotate", 2), ("scale", 3)] {
            let Some(p) = plug_of(&c.s.graph, j, attr) else { continue };
            let value = |e: &(f64, DVec3, DVec3, DVec3)| match pick {
                1 => e.1,
                2 => e.2,
                _ => e.3,
            };
            let first = value(&series[k][0]);
            for comp in 0..3 {
                let moves = animated && series[k].iter().any(|e| value(e)[comp] != first[comp]);
                let plug = PlugRef::component(j, p.attr, comp as i32);
                if moves {
                    for e in &series[k] {
                        let _ = anim::key_plug(&mut c.s.graph, &plug, e.0, Some(value(e)[comp]), Some(Tangent::Linear), Some(Tangent::Linear));
                    }
                } else {
                    let _ = c.s.graph.set_value(plug, PlugValue::Float(first[comp]));
                }
            }
        }
    }
    if animated {
        c.times.extend_from_slice(&times);
    }
}

/// The skeleton a prim is bound to, inherited down namespace.
fn bound_skeleton(prim: &Prim) -> Option<String> {
    let mut p = Some(prim.clone());
    while let Some(q) = p {
        if let Ok(t) = q.relationship("skel:skeleton").targets() {
            if let Some(first) = t.first() {
                return Some(first.as_str().to_string());
            }
        }
        p = q.parent();
    }
    None
}

fn bound_anim(stage_prim: &Prim) -> Option<String> {
    stage_prim.relationship("skel:animationSource").targets().ok().and_then(|t| t.first().map(|x| x.as_str().to_string()))
}

fn read_skin(c: &mut Ctx, prim: &Prim, shape: NodeId, skel: &Skel) -> Result<(), String> {
    let ji_attr = prim.attribute("primvars:skel:jointIndices");
    let (Some(ji), Some(jw)) = (get_vec::<i32>(&ji_attr, None), get_vec::<f32>(&prim.attribute("primvars:skel:jointWeights"), None)) else {
        return Ok(());
    };
    let width = ji_attr.get_metadata::<i32>("elementSize").ok().flatten().unwrap_or(1).max(1) as usize;
    let rigid = ji_attr.get_metadata::<Token>("interpolation").ok().flatten().is_some_and(|t| t.as_str() == "constant");
    // Indices count into the mesh's own joint list when it has one, else the skeleton's.
    let own = tokens_of(prim, "skel:joints");
    let order: Vec<usize> = if own.is_empty() {
        (0..skel.tokens.len()).collect()
    } else {
        own.iter().filter_map(|t| skel.tokens.iter().position(|x| x == t)).collect()
    };
    let used: Vec<usize> = {
        let mut u: Vec<usize> = ji.iter().filter_map(|&i| order.get(i.max(0) as usize).copied()).collect();
        u.sort_unstable();
        u.dedup();
        u
    };
    if used.is_empty() {
        return Ok(());
    }
    let mut args: Vec<String> = used.iter().map(|&k| c.s.graph.path(skel.joints[k])).collect();
    args.push(c.s.graph.path(shape));
    c.s.run("skinCluster", json!({"args": args, "toSelectedBones": true})).map_err(|e| format!("{}: {e}", prim.path()))?;
    let sk = skin::skin_of(&c.s.graph, shape).ok_or_else(|| format!("{}: no skinCluster made", prim.path()))?;
    let inf = skin::influences(&c.s.graph, sk);
    let element_of: HashMap<NodeId, i32> = inf.iter().map(|(e, j)| (*j, *e)).collect();
    let mut b = (*skin::bind_of(&c.s.graph, sk).ok_or("no bind")?).clone();
    let npoints = b.weights.len();
    // USD poses points in the skeleton's space; a skinCluster works in the world's. The same matrices stand once the
    // skeleton's own world is on both sides of them.
    c.s.graph.evaluate();
    let space = if skel.holder == NO_NODE { DMat4::IDENTITY } else { c.s.graph.node(skel.holder).map_or(DMat4::IDENTITY, |n| n.matrix(aztek_scene::transform::Transform::WORLD_MATRIX)) };
    for (el, j) in &inf {
        if let Some(k) = skel.joints.iter().position(|x| x == j) {
            if let (Some(bm), Some(slot)) = (skel.bind.get(k), b.bind_pre.get_mut(*el as usize)) {
                *slot = (space * *bm).inverse();
            }
        }
    }
    let geom = prim.attribute("primvars:skel:geomBindTransform").get::<gf::Matrix4d>().ok().flatten().map_or(DMat4::IDENTITY, |g| from_gf(&g));
    b.geom = space * geom;
    for p in 0..npoints {
        let base = if rigid { 0 } else { p * width };
        let mut row = Vec::with_capacity(width);
        for w in 0..width {
            let (Some(&i), Some(&v)) = (ji.get(base + w), jw.get(base + w)) else { continue };
            if v <= 0.0 {
                continue;
            }
            let Some(&k) = order.get(i.max(0) as usize) else { continue };
            if let Some(&el) = element_of.get(&skel.joints[k]) {
                row.push((el as u32, v as f64));
            }
        }
        b.weights[p] = row;
    }
    skin::commit_paint(&mut c.s.graph, sk, b).map_err(|e| e.to_string())?;
    Ok(())
}

fn read_blends(c: &mut Ctx, prim: &Prim, shape: NodeId, anim_prim: Option<&Prim>) -> Result<(), String> {
    let names = tokens_of(prim, "skel:blendShapes");
    let targets = prim.relationship("skel:blendShapeTargets").targets().unwrap_or_default();
    if names.is_empty() || targets.is_empty() {
        return Ok(());
    }
    let mesh_name = c.s.graph.path(shape);
    let made = c.s.run("blendShape", json!({"args": [mesh_name], "name": "blendShape1"})).map_err(|e| format!("{}: {e}", prim.path()))?;
    let bl_name = match &made {
        serde_json::Value::Array(a) => a.first().and_then(|x| x.as_str()).unwrap_or("").to_string(),
        serde_json::Value::String(x) => x.clone(),
        _ => String::new(),
    };
    let bl = deform_blend::blend_named(&c.s.graph, &bl_name, "mayaUSDImport").map_err(|e| e.to_string())?;
    let rest: Vec<DVec3> = crate::util::evaluated_mesh(&mut c.s.graph, shape).map(|m| m.positions().to_vec()).unwrap_or_default();
    let mut data = BlendData { rest, targets: Vec::new() };
    for (i, (name, target)) in names.iter().zip(&targets).enumerate() {
        let Ok(tp) = c.stage.prim(target.clone()) else { continue };
        let pts: Vec<u32> = get_vec::<i32>(&tp.attribute("pointIndices"), None).map(|v| v.iter().map(|&x| x as u32).collect()).unwrap_or_else(|| (0..data.rest.len() as u32).collect());
        let list = |attr: &str| -> Vec<(u32, DVec3)> {
            get_vec::<gf::Vec3f>(&tp.attribute(attr), None)
                .map(|o| pts.iter().zip(o.iter()).map(|(p, d)| (*p, DVec3::new(d.x as f64, d.y as f64, d.z as f64))).filter(|x| x.1.length_squared() > 0.0).collect())
                .unwrap_or_default()
        };
        let mut items = vec![BlendItem { iti: 6000, relative: false, deltas: list("offsets") }];
        for a in tp.attributes_in_namespace("inbetweens").unwrap_or_default() {
            let weight = a.get_metadata::<f32>("weight").ok().flatten().unwrap_or(0.5);
            let iti = 5000 + (weight * 1000.0).round() as i32;
            items.push(BlendItem { iti, relative: false, deltas: list(a.name()) });
        }
        items.sort_by_key(|x| x.iti);
        let alias = node_name(name);
        data.targets.push(BlendTarget { index: i as i32, name: alias.clone(), items, weights: None });
        c.s.graph.set_alias(bl, &alias, Some(format!("weight[{i}]")));
    }
    deform_blend::set_blend(&mut c.s.graph, bl, data).map_err(|e| e.to_string())?;
    let attr = plug_of(&c.s.graph, bl, "weight").map(|p| p.attr).ok_or("no weight")?;

    // Weights from the animation.
    if let Some(ap) = anim_prim {
        let order = tokens_of(ap, "blendShapes");
        let wa = ap.attribute("blendShapeWeights");
        let mut times = wa.time_sample_times().unwrap_or_default();
        if let Some((lo, hi)) = c.o.frame_range {
            times.retain(|t| *t >= lo && *t <= hi);
        }
        if c.o.read_anim && times.len() > 1 {
            for &f in &times {
                let w = wa.get_at::<Vec<f32>>(TimeCode::new(f)).ok().flatten().unwrap_or_default();
                for (k, v) in w.iter().enumerate() {
                    let Some(i) = order.get(k).and_then(|n| names.iter().position(|x| x == n)) else { continue };
                    let p = PlugRef::element(bl, attr, i as i32);
                    let _ = anim::key_plug(&mut c.s.graph, &p, f, Some(*v as f64), Some(Tangent::Linear), Some(Tangent::Linear));
                }
            }
            c.times.extend_from_slice(&times);
        } else if let Ok(Some(w)) = wa.get_at::<Vec<f32>>(at(None)) {
            for (k, v) in w.iter().enumerate() {
                let Some(i) = order.get(k).and_then(|n| names.iter().position(|x| x == n)) else { continue };
                let _ = c.s.graph.set_value(PlugRef::element(bl, attr, i as i32), PlugValue::Float(*v as f64));
            }
        }
    }
    Ok(())
}

pub fn read_all(c: &mut Ctx, _roots: &[NodeId]) -> Result<(), String> {
    let stage = c.stage.clone();
    let mut found: Vec<Prim> = Vec::new();
    stage
        .traverse(PrimPredicate::DEFAULT, |p| {
            if let Ok(pr) = stage.prim(p.clone()) {
                if pr.type_name().ok().flatten().is_some_and(|t| t.as_str() == "Skeleton") {
                    found.push(pr);
                }
            }
        })
        .map_err(|e| e.to_string())?;
    let mut skels: HashMap<String, Skel> = HashMap::new();
    let mut anims: HashMap<String, Prim> = HashMap::new();
    for prim in found {
        // Only skeletons under what was asked for.
        let Some(parent) = node_above(c, prim.path()) else {
            let wanted = c.o.prim_path.is_empty() || c.o.prim_path == "/";
            if !wanted && !prim.path().as_str().starts_with(&c.o.prim_path) {
                continue;
            }
            let sk = read_skeleton(c, &prim, NO_NODE)?;
            finish_skeleton(c, &prim, sk, &mut skels, &mut anims);
            continue;
        };
        let sk = read_skeleton(c, &prim, parent)?;
        finish_skeleton(c, &prim, sk, &mut skels, &mut anims);
    }
    // Meshes bound to them. Blend shapes go first, nearer the mesh, as the reference orders them.
    let meshes: Vec<(String, NodeId)> = c.meshes.iter().map(|(k, v)| (k.clone(), v.0)).collect();
    for (path, shape) in meshes {
        let prim = stage.prim(path.as_str()).map_err(|e| e.to_string())?;
        let skel_path = bound_skeleton(&prim);
        let anim_prim = skel_path.as_ref().and_then(|p| anims.get(p)).cloned();
        read_blends(c, &prim, shape, anim_prim.as_ref())?;
        if let Some(p) = skel_path {
            if let Some(sk) = skels.remove(&p) {
                let r = read_skin(c, &prim, shape, &sk);
                skels.insert(p, sk);
                r?;
            }
        }
    }
    Ok(())
}

fn finish_skeleton(c: &mut Ctx, prim: &Prim, sk: Skel, skels: &mut HashMap<String, Skel>, anims: &mut HashMap<String, Prim>) {
    if let Some(a) = bound_anim(prim) {
        if let Ok(ap) = c.stage.prim(a.as_str()) {
            read_animation(c, &sk, &ap);
            anims.insert(prim.path().as_str().to_string(), ap);
        }
    }
    skels.insert(prim.path().as_str().to_string(), sk);
}
