//! Dynamics commands for bodies, the solver and constraints. Old systems' command names are kept as aliases so their
//! scripts run: `nCloth` and `createNCloth` make cloth, `makeCollideNCloth` a collider, `createNConstraint` a constraint.
//! Particles, emitters, fields and caches are in `dyn_particles`.

use aztek_dyn::{Anchor, Link};
use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::deform::{f, plug, Cmd, GEOM, IN};
use crate::dyn_solve::{BodyRef, Kind};
use crate::dynamics::{self as dy};
use crate::graph::Graph;
use crate::node::types;
use crate::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::Transform;

/// The playback range: (first frame, last frame).
pub(crate) fn playback_range() -> (f64, f64) {
    let r = crate::anim::range();
    (r.min, r.max)
}

/// The selection as names: objects, or mesh vertices as `pCube1Shape.vtx[3]`.
pub(crate) fn selection_strings(g: &mut Graph) -> Vec<String> {
    use crate::components::{self, ComponentMode};
    let mode = components::mode(g);
    let mut out = Vec::new();
    if mode != ComponentMode::Object {
        for (node, ids) in components::selected_all(g, mode).clone() {
            let Some(m) = components::as_mesh(g, node) else { continue };
            let Some(cage) = crate::modifiers::cage(g, m) else { continue };
            let path = g.path(m);
            for v in components::convert(&cage, mode, &ids, ComponentMode::Vertex, false) {
                out.push(format!("{path}.vtx[{v}]"));
            }
        }
    }
    if out.is_empty() {
        out = g.selection().iter().map(|&id| g.path(id)).collect();
    }
    out
}

pub(crate) fn q(c: Cmd) -> Cmd {
    c.flag(f("query", "q", ArgType::Bool, "Query a value."))
        .flag(f("edit", "e", ArgType::Bool, "Edit an existing node."))
        .flag(f("name", "n", ArgType::String, "Name of the new node."))
}

pub(crate) fn attr_plug(g: &Graph, id: NodeId, name: &str) -> CmdResult<PlugRef> {
    let Some(a) = g.node(id).and_then(|n| n.ty().attr_index(name)) else { fail!("{} has no {name}", g.path(id)) };
    Ok(PlugRef::new(id, a as i32))
}

pub(crate) fn set(g: &mut Graph, id: NodeId, name: &str, v: impl Into<PlugValue>) -> CmdResult<()> {
    let p = attr_plug(g, id, name)?;
    g.set_value(p, v.into())?;
    Ok(())
}

pub(crate) fn connect(g: &mut Graph, from: NodeId, out: &str, to: NodeId, input: &str) -> CmdResult<()> {
    let a = attr_plug(g, from, out)?;
    let b = attr_plug(g, to, input)?;
    g.connect(&a, &b, true)?;
    Ok(())
}

/// Connects into the next free element of an array input.
pub(crate) fn connect_next(g: &mut Graph, from: NodeId, out: &str, to: NodeId, array: &str) -> CmdResult<()> {
    let a = attr_plug(g, from, out)?;
    let arr = attr_plug(g, to, array)?;
    let els = g.elements(&arr);
    if els.iter().any(|e| g.source(e).is_some_and(|s| s.node == from)) {
        return Ok(());
    }
    let next = els.iter().map(|e| e.element + 1).max().unwrap_or(0);
    g.connect(&a, &PlugRef::element(to, arr.attr, next), true)?;
    Ok(())
}

/// A DAG shape of `ty` under a new transform called `name`: (transform, shape).
pub(crate) fn make_shape(g: &mut Graph, ty: &str, name: &str) -> CmdResult<(NodeId, NodeId)> {
    let xt = types::find("transform").unwrap();
    let st = types::find(ty).unwrap();
    let xf = g.create_node(&xt, name, NO_NODE, true)?;
    let stem = g.node(xf).unwrap().name().to_string();
    let shape_name = match stem.trim_end_matches(|c: char| c.is_ascii_digit()) {
        base if base.len() < stem.len() => format!("{base}Shape{}", &stem[base.len()..]),
        _ => format!("{stem}Shape"),
    };
    let shape = g.create_node(&st, &shape_name, xf, true)?;
    Ok((xf, shape))
}

/// A body or solver named, or the one under a named transform, or the body acting on a named mesh.
pub(crate) fn resolve(g: &Graph, name: &str, ty: &[&str]) -> Option<NodeId> {
    let id = g.find(name).ok().flatten()?;
    let is = |id: NodeId| g.node(id).is_some_and(|n| ty.contains(&n.ty().name.as_str()));
    if is(id) {
        return Some(id);
    }
    for &k in g.children(id) {
        if is(k) {
            return Some(k);
        }
    }
    // A mesh or its transform: a cloth feeding it, or a body reading it.
    let shape = crate::components::as_mesh(g, id).unwrap_or(id);
    for n in g.nodes() {
        if !is(n) {
            continue;
        }
        let name = g.node(n).unwrap().ty().name.clone();
        if name == dy::CLOTH && crate::deform::mesh_fed_by(g, n) == Some(shape) {
            return Some(n);
        }
        if name == dy::RIGID && crate::dyn_solve::rigid_transform(g, n) == Some(crate::dag::transform_of(g, shape)) {
            return Some(n);
        }
        for input in ["inputMesh"] {
            if let Some(a) = g.node(n).unwrap().ty().attr_index(input) {
                if g.source(&PlugRef::new(n, a as i32)).is_some_and(|s| s.node == shape) {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// The solver named by -solver, else the one most recently made, else a new solver1.
pub(crate) fn solver_for(g: &mut Graph, a: &Args) -> CmdResult<NodeId> {
    if a.has("solver") {
        let name = a.string("solver", "");
        return resolve(g, &name, &[dy::SOLVER]).ok_or_else(|| aztek_ops::CommandError::new(format!("no solver {name}")));
    }
    if let Some(&s) = crate::dyn_solve::solvers(g).last() {
        return Ok(s);
    }
    let (_, shape) = make_shape(g, dy::SOLVER, "solver1")?;
    crate::dyn_solve::SOLVERS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let start = playback_range().0;
    set(g, shape, "startFrame", start)?;
    Ok(shape)
}

pub(crate) fn join(g: &mut Graph, body: NodeId, solver: NodeId) -> CmdResult<()> {
    connect(g, solver, "message", body, "solver")?;
    crate::dyn_solve::invalidate(g, solver);
    Ok(())
}

/// The solver a body or constraint belongs to.
pub fn solver_of(g: &Graph, id: NodeId) -> Option<NodeId> {
    let a = g.node(id)?.ty().attr_index("solver")?;
    g.source(&PlugRef::new(id, a as i32)).map(|s| s.node)
}

/// Meshes the command acts on: named, or selected.
fn meshes(s: &mut Session, a: &Args) -> CmdResult<Vec<NodeId>> {
    let names = a.named();
    let mut out = Vec::new();
    if names.is_empty() {
        for id in s.graph.selection().to_vec() {
            if let Some(m) = crate::components::as_mesh(&s.graph, id) {
                out.push(m);
            }
        }
    } else {
        for n in names {
            let Some(id) = s.graph.find(&n)? else { fail!("no object named {n}") };
            let Some(m) = crate::components::as_mesh(&s.graph, id) else { fail!("{n} is not a mesh") };
            out.push(m);
        }
    }
    out.dedup();
    Ok(out)
}

fn create_cloth(s: &mut Session, a: &Args) -> CmdResult {
    let targets = meshes(s, a)?;
    if targets.is_empty() {
        fail!("createCloth: select a mesh");
    }
    let g = &mut s.graph;
    let solver = solver_for(g, a)?;
    let mut made = Vec::new();
    for m in targets {
        if resolve(g, &g.path(m), &[dy::CLOTH]).is_some() {
            fail!("{} is cloth already", g.path(m));
        }
        let name = a.string("name", "cloth1");
        let c = crate::deform::insert_at(g, m, dy::CLOTH, &name, crate::deform::Place::After)?;
        join(g, c, solver)?;
        // New cloth starts as a material: cotton unless told.
        let preset = a.string("preset", "cotton");
        let Some((_, values)) = PRESETS.iter().find(|(n, _)| n.eq_ignore_ascii_case(&preset)) else { fail!("createCloth: no preset {preset}") };
        for (at, v) in values.iter() {
            set(g, c, at, *v)?;
        }
        g.evaluate();
        if let Some(mesh) = g.node(c).and_then(|n| n.data().mesh(IN).cloned()) {
            let (pts, faces) = dy::mesh_parts(&mesh);
            let t = aztek_dyn::topology::Topology::new(pts.len(), &faces);
            let w = crate::rig::world(g, crate::dag::transform_of(g, m));
            let world: Vec<DVec3> = pts.iter().map(|p| w.transform_point3(*p)).collect();
            // A tenth of an edge, between 2 mm and 1 cm of real size.
            let mpu = crate::dyn_solve::metres(g, solver);
            set(g, c, "thickness", (t.mean_edge(&world) * mpu * 0.1).clamp(0.002, 0.01) / mpu)?;
        }
        made.push(json!(g.path(c)));
    }
    Ok(Value::Array(made))
}

fn create_collider(s: &mut Session, a: &Args) -> CmdResult {
    let targets = meshes(s, a)?;
    if targets.is_empty() {
        fail!("collider: select a mesh");
    }
    let g = &mut s.graph;
    let solver = solver_for(g, a)?;
    let t = types::find(dy::COLLIDER).unwrap();
    let mut made = Vec::new();
    for m in targets {
        let name = a.string("name", "collider1");
        let c = g.create_node(&t, &name, NO_NODE, true)?;
        let out = g.node(m).unwrap().ty().attr_index("outMesh").unwrap();
        let input = attr_plug(g, c, "inputMesh")?;
        g.connect(&PlugRef::new(m, out as i32), &input, true)?;
        connect(g, m, "worldMatrix", c, "inputMatrix")?;
        if a.has("thickness") {
            set(g, c, "thickness", a.float("thickness", 0.0))?;
        }
        join(g, c, solver)?;
        made.push(json!(g.path(c)));
    }
    Ok(Value::Array(made))
}

const RIGID_FLAGS: &[(&str, &str)] = &[("mass", "mass"), ("bounciness", "bounciness"), ("staticFriction", "staticFriction"), ("dynamicFriction", "dynamicFriction"), ("standInObject", "standInObject"), ("particleResolution", "particleResolution")];

fn rigid_body(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    if a.bool("query") || a.bool("edit") {
        let Some(name) = a.named().into_iter().next() else { fail!("rigidBody: name a rigid body") };
        let Some(r) = resolve(g, &name, &[dy::RIGID]) else { fail!("rigidBody: {name} is not a rigid body") };
        if a.bool("query") {
            for (flag, at) in RIGID_FLAGS {
                if a.has(flag) {
                    return Ok(json!(crate::dyn_solve::num(g, r, at, 0.0)));
                }
            }
            if a.has("active") {
                return Ok(json!(g.node(r).unwrap().bool(g.node(r).unwrap().ty().attr_index("active").unwrap())));
            }
            fail!("rigidBody: query what?");
        }
        set_rigid_flags(g, r, a)?;
        if let Some(sv) = solver_of(g, r) {
            crate::dyn_solve::invalidate(g, sv);
        }
        return Ok(json!(g.path(r)));
    }
    let named = a.named();
    let objects: Vec<NodeId> = if named.is_empty() {
        g.selection().iter().filter_map(|&id| crate::components::as_mesh(g, id)).collect()
    } else {
        named.iter().filter_map(|n| g.find(n).ok().flatten()).filter_map(|id| crate::components::as_mesh(g, id)).collect()
    };
    if objects.is_empty() {
        fail!("rigidBody: select a mesh");
    }
    let solver = solver_for(g, a)?;
    let t = types::find(dy::RIGID).unwrap();
    let mut made = Vec::new();
    for m in objects {
        let xf = crate::dag::transform_of(g, m);
        let name = a.string("name", "rigidBody1");
        let r = g.create_node(&t, &name, NO_NODE, true)?;
        g.evaluate();
        let out = g.node(m).unwrap().ty().attr_index("outMesh").unwrap();
        let mesh = g.value(&PlugRef::new(m, out as i32))?;
        set(g, r, "inputMesh", mesh)?;
        set(g, r, "object", PlugValue::String(g.path(xf)))?;
        let w = crate::rig::world(g, xf);
        set(g, r, "initialMatrix", PlugValue::Matrix(w))?;
        let passive = a.bool("passive") || (a.has("active") && !a.bool("active"));
        set(g, r, "active", !passive)?;
        set_rigid_flags(g, r, a)?;
        if !passive {
            // The solver drives the object.
            connect(g, r, "outTranslate", xf, "translate")?;
            connect(g, r, "outRotate", xf, "rotate")?;
        }
        join(g, r, solver)?;
        made.push(json!(g.path(r)));
    }
    Ok(if made.len() == 1 { made.remove(0) } else { Value::Array(made) })
}

fn set_rigid_flags(g: &mut Graph, r: NodeId, a: &Args) -> CmdResult<()> {
    for (flag, at) in RIGID_FLAGS {
        if a.has(flag) {
            set(g, r, at, a.float(flag, 0.0))?;
        }
    }
    if let Some(v) = a.vec3("initialVelocity") {
        set(g, r, "initialVelocity", PlugValue::Vec3(DVec3::from(v)))?;
    }
    if let Some(v) = a.vec3("initialSpin") {
        set(g, r, "initialSpin", PlugValue::Vec3(DVec3::from(v)))?;
    }
    Ok(())
}

fn create_solver(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.string("name", "solver1");
    let (_, shape) = make_shape(&mut s.graph, dy::SOLVER, &name)?;
    crate::dyn_solve::SOLVERS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let start = playback_range().0;
    set(&mut s.graph, shape, "startFrame", start)?;
    Ok(json!(s.graph.path(shape)))
}

/// Material presets for cloth: (name, [(attribute, value)]).
/// Material presets in physical units: density kg/m^2; stretch and compression resistance in 100 N/m; bend resistance
/// in 1e-4 N m; friction; damp per second; drag as a coefficient over 20. Fabric barely resists compression, buckling
/// into folds finer than any mesh, so its compression resistance is about a hundredth of its stretch; rubber's is not.
pub const PRESETS: &[(&str, &[(&str, f64)])] = &[
    ("silk", &[("density", 0.04), ("stretchResistance", 3.0), ("compressionResistance", 0.03), ("bendResistance", 0.02), ("friction", 0.2), ("damp", 0.01), ("drag", 0.05)]),
    ("cotton", &[("density", 0.15), ("stretchResistance", 15.0), ("compressionResistance", 0.15), ("bendResistance", 0.15), ("friction", 0.4), ("damp", 0.02), ("drag", 0.05)]),
    ("leather", &[("density", 0.8), ("stretchResistance", 80.0), ("compressionResistance", 2.0), ("bendResistance", 10.0), ("friction", 0.6), ("damp", 0.05), ("drag", 0.04)]),
    ("heavyDenim", &[("density", 0.45), ("stretchResistance", 50.0), ("compressionResistance", 0.5), ("bendResistance", 3.0), ("friction", 0.5), ("damp", 0.03), ("drag", 0.05)]),
    ("rubber", &[("density", 1.1), ("stretchResistance", 2.0), ("compressionResistance", 2.0), ("bendResistance", 1.0), ("friction", 0.9), ("damp", 0.02), ("drag", 0.04)]),
];

fn cloth_preset(s: &mut Session, a: &Args) -> CmdResult {
    let name = a.string("preset", "");
    let Some((_, values)) = PRESETS.iter().find(|(n, _)| n.eq_ignore_ascii_case(&name)) else {
        let names: Vec<&str> = PRESETS.iter().map(|p| p.0).collect();
        fail!("clothPreset: -preset is one of {}", names.join(", "));
    };
    let mut targets = Vec::new();
    for n in a.named() {
        match resolve(&s.graph, &n, &[dy::CLOTH]) {
            Some(c) => targets.push(c),
            None => fail!("clothPreset: {n} is not cloth"),
        }
    }
    if targets.is_empty() {
        for id in s.graph.selection().to_vec() {
            if let Some(c) = resolve(&s.graph, &s.graph.path(id), &[dy::CLOTH]) {
                targets.push(c);
            }
        }
    }
    for &c in &targets {
        for (at, v) in values.iter() {
            set(&mut s.graph, c, at, *v)?;
        }
    }
    Ok(Value::Array(targets.iter().map(|&c| json!(s.graph.path(c))).collect()))
}

// ---------------------------------------------------------------------------------------------------------------------
// Constraints

fn create_constraint(s: &mut Session, a: &Args) -> CmdResult {
    let kind = a.string("type", "componentToComponent");
    let Some(k) = dy::CONSTRAINT_KINDS.iter().position(|x| x.eq_ignore_ascii_case(&kind)) else {
        fail!("dynamicConstraint: -type is one of {}", dy::CONSTRAINT_KINDS.join(", "));
    };
    let mut names = a.named();
    if names.is_empty() {
        names = selection_strings(&mut s.graph);
    }
    if names.is_empty() {
        fail!("dynamicConstraint: select components or objects");
    }
    // The last named item is the target where the kind takes one: a surface, or the other body.
    let two = matches!(k, 0 | 1 | 4 | 5 | 6 | 8) && names.len() > 1;
    let targets = if two { vec![names.pop().unwrap()] } else { Vec::new() };
    let g = &mut s.graph;
    let solver = solver_for(g, a)?;
    let t = types::find(dy::CONSTRAINT).unwrap();
    let name = a.string("name", "dynamicConstraint1");
    let c = g.create_node(&t, &name, NO_NODE, true)?;
    set(g, c, "constraintType", k as i64)?;
    set(g, c, "members", PlugValue::String(names.join(" ")))?;
    set(g, c, "targets", PlugValue::String(targets.join(" ")))?;
    if a.has("strength") {
        set(g, c, "strength", a.float("strength", 20.0))?;
    }
    if a.has("restLength") {
        set(g, c, "restLength", a.float("restLength", 0.0))?;
        set(g, c, "restLengthMethod", 1i64)?;
    }
    if a.has("glueStrength") {
        set(g, c, "glueStrength", a.float("glueStrength", 0.0))?;
    }
    join(g, c, solver)?;
    Ok(json!(g.path(c)))
}

/// Points named in a constraint, each as (body index, the body's points).
fn points_of(g: &mut Graph, text: &str, bodies: &[BodyRef], solver: &aztek_dyn::Solver) -> Vec<(usize, Vec<u32>)> {
    let names: Vec<String> = text.split_whitespace().map(str::to_string).collect();
    let mut out = Vec::new();
    let Ok(list) = crate::deform::named_targets(g, &names) else { return out };
    for (mesh, pts) in list {
        let Some(b) = bodies.iter().find(|b| {
            match b.kind {
                Kind::Cloth => crate::deform::mesh_fed_by(g, b.node) == Some(mesh),
                Kind::Rigid | Kind::Passive => crate::dyn_solve::rigid_transform(g, b.node) == Some(crate::dag::transform_of(g, mesh)),
                _ => g.node(b.node).and_then(|n| n.ty().attr_index("inputMesh")).and_then(|a| g.source(&PlugRef::new(b.node, a as i32))).is_some_and(|s| s.node == mesh),
            }
        }) else {
            continue;
        };
        let count = solver.bodies[b.index].rest.len() as u32;
        let pts = match (b.kind, pts) {
            (Kind::Rigid, _) | (_, None) => (0..count).collect(),
            (_, Some(p)) => p.into_iter().filter(|&p| p < count).collect(),
        };
        out.push((b.index, pts));
    }
    out
}

fn nearest(points: &[DVec3], to: DVec3) -> Option<usize> {
    points.iter().enumerate().min_by(|a, b| (*a.1 - to).length_squared().total_cmp(&(*b.1 - to).length_squared())).map(|(i, _)| i)
}

/// The links a solver's constraint nodes make, from the bodies as built.
pub(crate) fn links_of(g: &mut Graph, s: NodeId, bodies: &[BodyRef], solver: &aztek_dyn::Solver) -> Vec<Link> {
    let mut out = Vec::new();
    for c in crate::dyn_solve::members(g, s) {
        if g.node(c).is_none_or(|n| n.ty().name != dy::CONSTRAINT) {
            continue;
        }
        let num = |g: &Graph, n: &str, d: f64| crate::dyn_solve::num(g, c, n, d);
        if num(g, "enable", 1.0) == 0.0 {
            continue;
        }
        let kind = num(g, "constraintType", 0.0) as usize;
        let strength = num(g, "strength", 20.0).max(1e-6);
        let compliance = if matches!(kind, 2 | 3) { 0.0 } else { 1e-3 / (strength * strength) };
        let tear = num(g, "glueStrength", 0.0);
        let fixed = (num(g, "restLengthMethod", 0.0) as i64 == 1).then(|| num(g, "restLength", 0.0) * crate::dyn_solve::metres(g, s));
        let name = g.path(c);
        let members_text = g.node(c).map(|n| n.value(n.ty().attr_index("members").unwrap()).as_str().to_string()).unwrap_or_default();
        let targets_text = g.node(c).map(|n| n.value(n.ty().attr_index("targets").unwrap()).as_str().to_string()).unwrap_or_default();
        let members = points_of(g, &members_text, bodies, solver);
        let targets = points_of(g, &targets_text, bodies, solver);
        let rest_of = |b: usize| &solver.bodies[b].rest;
        let mut push = |a: Anchor, b: Anchor, pa: DVec3, pb: DVec3| {
            let rest = fixed.unwrap_or_else(|| if matches!(kind, 2 | 3 | 4) { 0.0 } else { (pa - pb).length() });
            out.push(Link { name: name.clone(), a, b, rest, compliance, tear: if kind == 3 && tear <= 0.0 { 0.05 } else { tear }, broken: false, enabled: true });
        };
        match kind {
            // Nail: each point held where it starts.
            7 => {
                for (b, pts) in &members {
                    for &p in pts {
                        let at = rest_of(*b)[p as usize];
                        push(Anchor::Particle(*b as u32, p), Anchor::World(at), at, at);
                    }
                }
            }
            // Point to surface: each point to the nearest place on the target's surface.
            1 => {
                let Some((tb, _)) = targets.first() else { continue };
                let tbody = &solver.bodies[*tb];
                for (b, pts) in &members {
                    for &p in pts {
                        let at = rest_of(*b)[p as usize];
                        let best = tbody.topo.triangles.iter().enumerate().map(|(ti, t)| {
                            let [x, y, z] = t.map(|k| tbody.rest[k as usize]);
                            let (w, _) = aztek_dyn::geom::closest_on_triangle(at, x, y, z);
                            (ti, w, (aztek_dyn::geom::at(w, x, y, z) - at).length())
                        }).min_by(|a, b| a.2.total_cmp(&b.2));
                        if let Some((ti, w, _)) = best {
                            let [x, y, z] = tbody.topo.triangles[ti].map(|k| tbody.rest[k as usize]);
                            push(Anchor::Particle(*b as u32, p), Anchor::Surface(*tb as u32, ti as u32, w), at, aztek_dyn::geom::at(w, x, y, z));
                        }
                    }
                }
            }
            // Rigid pin, hinge and spring: between the bodies' nearest particles at the point between them.
            4 | 5 | 6 => {
                let (Some((ba, _)), Some((bb, _))) = (members.first(), targets.first()) else { continue };
                let (ra, rb) = (rest_of(*ba), rest_of(*bb));
                let ca = ra.iter().sum::<DVec3>() / ra.len().max(1) as f64;
                let cb = rb.iter().sum::<DVec3>() / rb.len().max(1) as f64;
                let pivot = (ca + cb) * 0.5;
                let pivots: Vec<DVec3> = match kind {
                    5 => {
                        let half = (ca - cb).length().max(1e-3) * 0.25;
                        vec![pivot + DVec3::Y * half, pivot - DVec3::Y * half]
                    }
                    6 => vec![ca, cb],
                    _ => vec![pivot],
                };
                if kind == 6 {
                    if let (Some(ia), Some(ib)) = (nearest(ra, ca), nearest(rb, cb)) {
                        push(Anchor::Particle(*ba as u32, ia as u32), Anchor::Particle(*bb as u32, ib as u32), ra[ia], rb[ib]);
                    }
                    continue;
                }
                for pv in pivots {
                    if let (Some(ia), Some(ib)) = (nearest(ra, pv), nearest(rb, pv)) {
                        // Both ends at the pivot: the link holds the two particles together.
                        push(Anchor::Particle(*ba as u32, ia as u32), Anchor::Particle(*bb as u32, ib as u32), pv, pv);
                    }
                }
            }
            // Component to component, weld, tear, point to point: each point to the nearest target point; with no
            // target, the points of the members to each other where they start close (welding a seam).
            _ => {
                let pool: Vec<(usize, u32, DVec3)> = if targets.is_empty() { members.iter().flat_map(|(b, pts)| pts.iter().map(move |&p| (*b, p))).map(|(b, p)| (b, p, rest_of(b)[p as usize])).collect() } else { targets.iter().flat_map(|(b, pts)| pts.iter().map(move |&p| (*b, p))).map(|(b, p)| (b, p, rest_of(b)[p as usize])).collect() };
                for (b, pts) in &members {
                    for &p in pts {
                        let at = rest_of(*b)[p as usize];
                        let best = pool.iter().filter(|(ob, op, _)| !(ob == b && *op == p)).min_by(|x, y| (x.2 - at).length_squared().total_cmp(&(y.2 - at).length_squared()));
                        let Some(&(ob, op, pos)) = best else { continue };
                        // Each pair once.
                        if targets.is_empty() && (ob, op) < (*b, p) {
                            continue;
                        }
                        push(Anchor::Particle(*b as u32, p), Anchor::Particle(ob as u32, op), at, pos);
                    }
                }
            }
        }
    }
    out
}

/// Out of simulation counts and timings: dynInfo -q -oos solver1.
fn dyn_info(s: &mut Session, a: &Args) -> CmdResult {
    s.graph.evaluate();
    let names = a.named();
    let targets: Vec<NodeId> = if names.is_empty() { crate::dyn_solve::solvers(&s.graph) } else { names.iter().filter_map(|n| resolve(&s.graph, n, dy::TYPES)).collect() };
    let mut out = serde_json::Map::new();
    for id in targets {
        let g = &s.graph;
        let ty = g.node(id).unwrap().ty().name.clone();
        let count = crate::dyn_solve::num(g, id, "outOfSimulation", 0.0) as i64;
        if a.bool("outOfSimulation") {
            return Ok(json!(count));
        }
        let mut entry = json!({"type": ty, "outOfSimulation": count});
        if ty == dy::SOLVER {
            if let Some(stats) = crate::dyn_solve::with_state(g, id, |st| st.solver.as_ref().map(|sv| json!({"frame": st.last, "particles": sv.pt.len(), "contacts": sv.stats.contacts, "pairs": sv.stats.pairs, "fixed": sv.stats.fixed, "clamped": sv.stats.clamped, "left": sv.stats.left, "returned": sv.stats.returned, "longestOut": sv.stats.longest_out, "ms": st.solve_ms, "msPrepare": sv.stats.ms_prepare, "msConstraints": sv.stats.ms_constraints, "msCollide": sv.stats.ms_collide, "cachedFrames": st.cache.len(), "detail": sv.stats.detail.iter().map(|(k, v)| (k.to_string(), json!(v))).collect::<serde_json::Map<String, Value>>()}))).flatten() {
                entry["solve"] = stats;
            }
        }
        out.insert(g.path(id), entry);
    }
    Ok(Value::Object(out))
}

pub fn register(c: &mut Commands<Session>) {
    crate::dynamics::register_types();
    let cloth = |name: &str| {
        q(Cmd::new(name, "Make the selected (or named) meshes cloth in a solver: the mesh as it comes is the animation the cloth starts from and can be drawn to. Returns the cloth nodes.", create_cloth))
            .positional("meshes")
            .flag(f("solver", "sv", ArgType::String, "The solver to join (the newest, or a new one)."))
            .flag(f("preset", "p", ArgType::String, "The material it starts as: silk, cotton (the default), leather, heavyDenim or rubber.").choices(&["silk", "cotton", "leather", "heavyDenim", "rubber"]))
    };
    c.register(cloth("createCloth"));
    c.register(cloth("nCloth"));
    c.register(cloth("createNCloth"));
    let collider = |name: &str| {
        q(Cmd::new(name, "Make the selected meshes colliders: cloth, particles and rigid bodies hit them. Returns the collider nodes.", create_collider))
            .positional("meshes")
            .flag(f("solver", "sv", ArgType::String, "The solver to join."))
            .flag(f("thickness", "th", ArgType::Float, "Collision thickness."))
    };
    c.register(collider("collider"));
    c.register(collider("makeCollideNCloth"));
    c.register(
        q(Cmd::new("rigidBody", "Make the selected objects rigid bodies: active ones are moved by the solver, passive ones only push. -q/-e read and change one.", rigid_body))
            .positional("objects")
            .flag(f("active", "act", ArgType::Bool, "Moved by the solver (the default)."))
            .flag(f("passive", "pas", ArgType::Bool, "Only pushes others."))
            .flag(f("mass", "m", ArgType::Float, "Mass."))
            .flag(f("bounciness", "b", ArgType::Float, "Kept normal speed in contacts."))
            .flag(f("staticFriction", "sf", ArgType::Float, "Friction at rest."))
            .flag(f("dynamicFriction", "df", ArgType::Float, "Friction sliding."))
            .flag(f("standInObject", "si", ArgType::Int, "0 its mesh, 1 a box, 2 a sphere, 3 a capsule."))
            .flag(f("particleResolution", "prs", ArgType::Int, "Particles across the longest side."))
            .flag(f("initialVelocity", "iv", ArgType::Vec3, "Velocity at the start."))
            .flag(f("initialSpin", "is", ArgType::Vec3, "Spin at the start, degrees per second."))
            .flag(f("solver", "sv", ArgType::String, "The solver to join.")),
    );
    c.register(q(Cmd::new("createSolver", "Make a solver: solver1. Bodies made after it join it unless told another.", create_solver)));
    c.register(
        Cmd::new("clothPreset", "Set cloth to a material: -preset silk, cotton, leather, heavyDenim or rubber.", cloth_preset)
            .positional("cloth nodes or meshes")
            .flag(f("preset", "p", ArgType::String, "The material.").choices(&["silk", "cotton", "leather", "heavyDenim", "rubber"])),
    );
    let constraint = |name: &str| {
        q(Cmd::new(name, "Hold points of bodies: -type weld, componentToComponent, pointToSurface, tear, pin, hinge, spring, nail or pointToPoint. The last item named is the target where the type takes one.", create_constraint))
            .positional("components or objects")
            .flag(f("type", "t", ArgType::String, "What it holds.").choices(dy::CONSTRAINT_KINDS))
            .flag(f("strength", "s", ArgType::Float, "How stiff."))
            .flag(f("restLength", "rl", ArgType::Float, "A fixed rest length."))
            .flag(f("glueStrength", "gs", ArgType::Float, "Tear: breaks past this stretch."))
            .flag(f("solver", "sv", ArgType::String, "The solver to join."))
    };
    c.register(constraint("dynamicConstraint"));
    c.register(constraint("createNConstraint"));
    c.register(
        Cmd::new("dynInfo", "What the solvers are doing: vertices out of the simulation, contacts, timings. -oos returns just the count.", dyn_info)
            .positional("solvers or bodies")
            .flag(f("query", "q", ArgType::Bool, "Accepted."))
            .flag(f("outOfSimulation", "oos", ArgType::Bool, "Only how many vertices are out of the simulation."))
            .undoable(false),
    );
    let _ = (GEOM, plug(0, 0), Transform::WORLD_MATRIX);
}
