//! Particle, emitter, field and cache commands, under the old systems' names so their scripts run: `particle`,
//! `nParticle`, `emitter`, `gravity`, `air`, `drag`, `newton`, `radial`, `turbulence`, `uniform`, `vortex`,
//! `volumeAxis`, `connectDynamic`, `getParticleAttr`, `setParticleAttr`, `dynExpression`; and `dynCache` for disk
//! caches.

use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::deform::{f, Cmd};
use crate::dyn_cmds::{attr_plug, connect, connect_next, join, make_shape, q, resolve, set, solver_for, solver_of};
use crate::dynamics::{self as dy};
use crate::graph::Graph;
use crate::plug::{NodeId, PlugValue};
use crate::session::Session;

fn particles_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let base = name.split('.').next().unwrap_or(name);
    resolve(g, base, &[dy::PARTICLES]).ok_or_else(|| aztek_ops::CommandError::new(format!("{name} is not particles")))
}

/// `particle1.pt[2:4]` -> [2, 3, 4]; None for the whole object.
fn particle_ids(name: &str) -> Option<Vec<usize>> {
    let at = name.find(".pt[")?;
    let inner = name[at + 4..].trim_end_matches(']');
    let mut out = Vec::new();
    for part in inner.split(',') {
        match part.split_once(':') {
            Some((a, b)) => {
                let (a, b): (usize, usize) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
                out.extend(a..=b);
            }
            None => out.push(part.trim().parse().ok()?),
        }
    }
    Some(out)
}

fn particle(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    if a.bool("query") {
        let Some(name) = a.named().into_iter().next() else { fail!("particle: name the particles") };
        let p = particles_named(g, &name)?;
        g.evaluate();
        let st = dy::particle_state(g, p).unwrap_or_default();
        if a.bool("count") {
            return Ok(json!(st.position.len()));
        }
        if a.has("attribute") {
            let at = a.string("attribute", "");
            let id = a.int("id", -1);
            let idx = st.id.iter().position(|&x| x as i64 == id).or((id < 0).then_some(0));
            let Some(i) = idx.filter(|&i| i < st.position.len()) else { fail!("particle: no particle {id}") };
            return Ok(particle_value(&st, &at, i));
        }
        fail!("particle: query what?");
    }
    let mut pts: Vec<f64> = Vec::new();
    if let Some(Value::Array(list)) = a.get("position") {
        for v in list {
            match v {
                Value::Array(xyz) if xyz.len() == 3 => pts.extend(xyz.iter().map(|x| x.as_f64().unwrap_or(0.0))),
                Value::Number(n) => pts.push(n.as_f64().unwrap_or(0.0)),
                _ => {}
            }
        }
    }
    // A grid: -ll lower left, -ur upper right, -grs spacing.
    if let (Some(lo), Some(hi)) = (a.vec3("lowerLeft"), a.vec3("upperRight")) {
        let step = a.float("gridSpacing", 1.0).max(1e-6);
        let (lo, hi) = (DVec3::from(lo), DVec3::from(hi));
        let n = ((hi - lo) / step).floor();
        for x in 0..=n.x as i64 {
            for y in 0..=n.y as i64 {
                for z in 0..=n.z as i64 {
                    let p = lo + DVec3::new(x as f64, y as f64, z as f64) * step;
                    pts.extend([p.x, p.y, p.z]);
                }
            }
        }
    }
    pts.truncate(pts.len() / 3 * 3);
    let solver = solver_for(g, a)?;
    let name = a.string("name", "particles1");
    let (xf, shape) = make_shape(g, dy::PARTICLES, &name)?;
    if !pts.is_empty() {
        crate::deform::set_floats(g, shape, attr_plug(g, shape, "startPositions")?.attr as usize, pts)?;
    }
    if a.has("conserve") {
        let _ = a.float("conserve", 1.0);
    }
    join(g, shape, solver)?;
    Ok(json!([g.path(xf), g.path(shape)]))
}

fn particle_value(st: &dy::ParticleState, at: &str, i: usize) -> Value {
    let v3 = |v: DVec3| json!([v.x, v.y, v.z]);
    match at {
        "position" | "worldPosition" => v3(st.position[i]),
        "velocity" | "worldVelocity" => v3(st.velocity[i]),
        "rgbPP" => v3(st.rgb[i]),
        "age" => json!(st.age[i]),
        "lifespanPP" => json!(st.lifespan[i]),
        "opacityPP" => json!(st.opacity[i]),
        "radiusPP" => json!(st.radius[i]),
        "particleId" | "id" => json!(st.id[i]),
        "mass" => json!(1.0),
        _ => Value::Null,
    }
}

/// getParticleAttr -at position -a true particle1: every particle's value; without -a, the mean and the spread
/// (largest minus smallest), as the old command gives.
fn get_particle_attr(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.named().into_iter().next() else { fail!("getParticleAttr: name particles") };
    let g = &mut s.graph;
    let p = particles_named(g, &name)?;
    g.evaluate();
    let st = dy::particle_state(g, p).unwrap_or_default();
    let at = a.string("attribute", "position");
    let ids: Vec<usize> = particle_ids(&name).unwrap_or_else(|| (0..st.position.len()).collect()).into_iter().filter(|&i| i < st.position.len()).collect();
    let values: Vec<Vec<f64>> = ids
        .iter()
        .map(|&i| match particle_value(&st, &at, i) {
            Value::Array(v) => v.iter().filter_map(Value::as_f64).collect(),
            Value::Number(n) => vec![n.as_f64().unwrap_or(0.0)],
            _ => Vec::new(),
        })
        .filter(|v| !v.is_empty())
        .collect();
    if a.bool("array") {
        return Ok(Value::Array(values.into_iter().flatten().map(|x| json!(x)).collect()));
    }
    let Some(width) = values.first().map(Vec::len) else { return Ok(json!([])) };
    let mut out = Vec::new();
    for k in 0..width {
        let col: Vec<f64> = values.iter().map(|v| v[k]).collect();
        out.push(col.iter().sum::<f64>() / col.len() as f64);
    }
    for k in 0..width {
        let col: Vec<f64> = values.iter().map(|v| v[k]).collect();
        out.push(col.iter().cloned().fold(f64::NEG_INFINITY, f64::max) - col.iter().cloned().fold(f64::INFINITY, f64::min));
    }
    Ok(json!(out))
}

/// setParticleAttr -at velocity -vv 0 1 0 particle1.pt[3]: changes particles in the solver now.
fn set_particle_attr(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.named().into_iter().next() else { fail!("setParticleAttr: name particles") };
    let g = &mut s.graph;
    let p = particles_named(g, &name)?;
    let Some(sv) = solver_of(g, p) else { fail!("setParticleAttr: {name} is in no solver") };
    let at = a.string("attribute", "position");
    let value = a.vec3("vectorValue").map(DVec3::from).unwrap_or_else(|| DVec3::splat(a.float("floatValue", 0.0)));
    let ids = particle_ids(&name);
    let n = g.node(sv).and_then(|n| n.behavior().as_any().downcast_ref::<dy::SolverNode>());
    let Some(n) = n else { fail!("setParticleAttr: no solver") };
    let mut st = n.state.lock().unwrap();
    let body = st.bodies.iter().find(|b| b.node == p).map(|b| b.index);
    let (Some(b), Some(solver)) = (body, st.solver.as_mut()) else { fail!("setParticleAttr: {name} has not been solved yet") };
    let range = solver.bodies[b].start..solver.bodies[b].start + solver.bodies[b].count;
    let mut changed = 0;
    for (k, i) in range.enumerate() {
        if ids.as_ref().is_some_and(|ids| !ids.contains(&k)) {
            continue;
        }
        let pt = &mut solver.pt;
        match at.as_str() {
            "position" => {
                pt.x[i] = value;
                pt.p[i] = value;
            }
            "velocity" => pt.v[i] = value,
            "rgbPP" => pt.rgb[i] = value,
            "opacityPP" => pt.opacity[i] = value.x,
            "lifespanPP" => pt.life[i] = value.x,
            "radiusPP" => pt.radius[i] = value.x,
            other => fail!("setParticleAttr: {other} is not a particle attribute here"),
        }
        changed += 1;
    }
    let last = st.last;
    if let Some(l) = last {
        st.cache.retain(|&k, _| (k as f64) < l * 1000.0);
    }
    drop(st);
    Ok(json!(changed))
}

fn emitter(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    if a.bool("query") || a.bool("edit") {
        let Some(name) = a.named().into_iter().next() else { fail!("emitter: name an emitter") };
        let Some(e) = resolve(g, &name, &[dy::EMITTER]) else { fail!("emitter: {name} is not an emitter") };
        if a.bool("query") {
            for (flag, at) in [("rate", "rate"), ("speed", "speed"), ("speedRandom", "speedRandom"), ("spread", "spread")] {
                if a.has(flag) {
                    return Ok(json!(crate::dyn_solve::num(g, e, at, 0.0)));
                }
            }
            if a.has("type") {
                return Ok(json!(dy::EMITTER_KINDS[crate::dyn_solve::num(g, e, "emitterType", 1.0) as usize]));
            }
            fail!("emitter: query what?");
        }
        emitter_flags(g, e, a)?;
        return Ok(json!(g.path(e)));
    }
    // A mesh named or selected: emits off its surface.
    let named = a.named();
    let source = named.first().and_then(|n| g.find(n).ok().flatten()).or_else(|| g.selection().first().copied()).and_then(|id| crate::components::as_mesh(g, id));
    let solver = solver_for(g, a)?;
    let name = a.string("name", "emitter1");
    let (xf, shape) = make_shape(g, dy::EMITTER, &name)?;
    if let Some(p) = a.vec3("position") {
        let tp = attr_plug(g, xf, "translate")?;
        g.set_value(tp, PlugValue::Vec3(DVec3::from(p)))?;
    }
    if let Some(m) = source {
        set(g, shape, "emitterType", 2i64)?;
        let out = g.node(m).unwrap().ty().attr_index("outMesh").unwrap();
        let input = attr_plug(g, shape, "inputMesh")?;
        g.connect(&crate::plug::PlugRef::new(m, out as i32), &input, true)?;
        connect(g, m, "worldMatrix", shape, "inputMatrix")?;
    }
    emitter_flags(g, shape, a)?;
    // Feeds the particles named, or new ones.
    let particles = if a.has("particles") {
        particles_named(g, &a.string("particles", ""))?
    } else {
        let (_, p) = make_shape(g, dy::PARTICLES, "particles1")?;
        join(g, p, solver)?;
        p
    };
    connect_next(g, shape, "message", particles, "newParticles")?;
    if let Some(sv) = solver_of(g, particles) {
        crate::dyn_solve::invalidate(g, sv);
    }
    Ok(json!([g.path(xf), g.path(particles)]))
}

fn emitter_flags(g: &mut Graph, e: NodeId, a: &Args) -> CmdResult<()> {
    if a.has("type") {
        let t = a.string("type", "omni").to_lowercase();
        let k = match t.as_str() {
            "dir" | "direction" | "directional" => 0,
            "omni" => 1,
            "surf" | "surface" => 2,
            "curv" | "curve" => 3,
            "vol" | "volume" => 4,
            other => fail!("emitter: no type {other}"),
        };
        set(g, e, "emitterType", k as i64)?;
    }
    for (flag, at) in [("rate", "rate"), ("speed", "speed"), ("speedRandom", "speedRandom"), ("spread", "spread")] {
        if a.has(flag) {
            set(g, e, at, a.float(flag, 0.0))?;
        }
    }
    let d = DVec3::new(a.float("directionX", f64::NAN), a.float("directionY", f64::NAN), a.float("directionZ", f64::NAN));
    if d.is_finite() || a.has("directionX") || a.has("directionY") || a.has("directionZ") {
        let old = crate::dyn_solve::num(g, e, "emitterType", 1.0);
        let _ = old;
        let cur = g.node(e).unwrap().vec3(g.node(e).unwrap().ty().attr_index("direction").unwrap());
        let v = DVec3::new(if d.x.is_finite() { d.x } else { cur.x }, if d.y.is_finite() { d.y } else { cur.y }, if d.z.is_finite() { d.z } else { cur.z });
        set(g, e, "direction", PlugValue::Vec3(v))?;
    }
    if a.has("volumeShape") {
        set(g, e, "volumeShape", if a.string("volumeShape", "cube") == "sphere" { 1i64 } else { 0 })?;
    }
    Ok(())
}

const FIELD_COMMANDS: &[(&str, &str)] = &[("gravity", "gravityField"), ("air", "airField"), ("drag", "dragField"), ("newton", "newtonField"), ("radial", "radialField"), ("turbulence", "turbulenceField"), ("uniform", "uniformField"), ("vortex", "vortexField"), ("volumeAxis", "volumeAxisField")];

fn field_command(s: &mut Session, a: &Args, kind: usize) -> CmdResult {
    let g = &mut s.graph;
    if a.bool("query") || a.bool("edit") {
        let Some(name) = a.named().into_iter().next() else { fail!("{}: name a field", FIELD_COMMANDS[kind].0) };
        let Some(fl) = resolve(g, &name, &[dy::FIELD]) else { fail!("{name} is not a field") };
        if a.bool("query") {
            for (flag, at) in FIELD_FLAGS {
                if a.has(flag) {
                    return Ok(json!(crate::dyn_solve::num(g, fl, at, 0.0)));
                }
            }
            fail!("{}: query what?", FIELD_COMMANDS[kind].0);
        }
        field_flags(g, fl, a, kind)?;
        return Ok(json!(g.path(fl)));
    }
    let name = a.string("name", &format!("{}1", FIELD_COMMANDS[kind].1));
    let (xf, shape) = make_shape(g, dy::FIELD, &name)?;
    set(g, shape, "fieldType", kind as i64)?;
    let def = aztek_dyn::Field::new(match kind {
        1 => aztek_dyn::FieldKind::Air,
        2 => aztek_dyn::FieldKind::Drag,
        3 => aztek_dyn::FieldKind::Newton,
        4 => aztek_dyn::FieldKind::Radial,
        5 => aztek_dyn::FieldKind::Turbulence,
        6 => aztek_dyn::FieldKind::Uniform,
        7 => aztek_dyn::FieldKind::Vortex,
        8 => aztek_dyn::FieldKind::VolumeAxis,
        _ => aztek_dyn::FieldKind::Gravity,
    });
    set(g, shape, "magnitude", def.magnitude)?;
    set(g, shape, "attenuation", def.attenuation)?;
    set(g, shape, "useMaxDistance", def.use_max_distance)?;
    set(g, shape, "direction", PlugValue::Vec3(def.direction))?;
    if let Some(p) = a.vec3("position") {
        let tp = attr_plug(g, xf, "translate")?;
        g.set_value(tp, PlugValue::Vec3(DVec3::from(p)))?;
    }
    field_flags(g, shape, a, kind)?;
    // Acts on the bodies selected.
    let named = a.named();
    let targets: Vec<NodeId> = if named.is_empty() { g.selection().to_vec() } else { named.iter().filter_map(|n| g.find(n).ok().flatten()).collect() };
    for t in targets {
        if let Some(b) = resolve(g, &g.path(t), &[dy::CLOTH, dy::PARTICLES, dy::RIGID]) {
            connect_next(g, shape, "message", b, "inputForce")?;
        }
    }
    Ok(json!([g.path(xf)]))
}

const FIELD_FLAGS: &[(&str, &str)] = &[("magnitude", "magnitude"), ("attenuation", "attenuation"), ("maxDistance", "maxDistance"), ("speed", "speed"), ("inheritVelocity", "inheritVelocity"), ("frequency", "frequency"), ("minDistance", "minDistance")];

fn field_flags(g: &mut Graph, fl: NodeId, a: &Args, _kind: usize) -> CmdResult<()> {
    for (flag, at) in FIELD_FLAGS {
        if a.has(flag) {
            set(g, fl, at, a.float(flag, 0.0))?;
            if *flag == "maxDistance" {
                set(g, fl, "useMaxDistance", true)?;
            }
        }
    }
    if a.has("useMaxDistance") {
        set(g, fl, "useMaxDistance", a.bool("useMaxDistance"))?;
    }
    if a.has("directionX") || a.has("directionY") || a.has("directionZ") {
        let cur = g.node(fl).unwrap().vec3(g.node(fl).unwrap().ty().attr_index("direction").unwrap());
        let v = DVec3::new(a.float("directionX", cur.x), a.float("directionY", cur.y), a.float("directionZ", cur.z));
        set(g, fl, "direction", PlugValue::Vec3(v))?;
    }
    Ok(())
}

/// connectDynamic -f gravity1 -em emitter1 -c collider1 particles1: what acts on the bodies named. -d disconnects.
fn connect_dynamic(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    let list = |a: &Args, flag: &str| -> Vec<String> {
        match a.get(flag) {
            Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
            Some(Value::String(x)) => vec![x.clone()],
            _ => Vec::new(),
        }
    };
    let delete = a.bool("delete");
    let mut done = Vec::new();
    for name in a.named() {
        let Some(b) = resolve(g, &name, &[dy::CLOTH, dy::PARTICLES, dy::RIGID]) else { fail!("connectDynamic: {name} is not a dynamic body") };
        // A body from a system with no solver node joins one.
        if solver_of(g, b).is_none() {
            let sv = solver_for(g, a)?;
            join(g, b, sv)?;
        }
        for fname in list(a, "fields") {
            let Some(fl) = resolve(g, &fname, &[dy::FIELD]) else { fail!("connectDynamic: {fname} is not a field") };
            if delete {
                let arr = attr_plug(g, b, "inputForce")?;
                for e in g.elements(&arr) {
                    if g.source(&e).is_some_and(|s| s.node == fl) {
                        g.disconnect(&e);
                    }
                }
            } else {
                connect_next(g, fl, "message", b, "inputForce")?;
            }
        }
        for ename in list(a, "emitters") {
            let Some(em) = resolve(g, &ename, &[dy::EMITTER]) else { fail!("connectDynamic: {ename} is not an emitter") };
            if delete {
                let arr = attr_plug(g, b, "newParticles")?;
                for e in g.elements(&arr) {
                    if g.source(&e).is_some_and(|s| s.node == em) {
                        g.disconnect(&e);
                    }
                }
            } else {
                connect_next(g, em, "message", b, "newParticles")?;
            }
        }
        for cname in list(a, "collisions") {
            // A collider in the same solver collides with everything in it already; joining it to the body's solver is
            // all there is to do.
            let Some(sv) = solver_of(g, b) else { continue };
            let c = match resolve(g, &cname, &[dy::COLLIDER, dy::CLOTH, dy::RIGID, dy::PARTICLES]) {
                Some(c) => c,
                None => {
                    let made = s_collider(g, &cname)?;
                    made
                }
            };
            if !delete && solver_of(g, c) != Some(sv) {
                join(g, c, sv)?;
            }
        }
        if let Some(sv) = solver_of(g, b) {
            crate::dyn_solve::invalidate(g, sv);
        }
        done.push(json!(g.path(b)));
    }
    Ok(Value::Array(done))
}

fn s_collider(g: &mut Graph, name: &str) -> CmdResult<NodeId> {
    let Some(id) = g.find(name)? else { fail!("connectDynamic: no object named {name}") };
    let Some(m) = crate::components::as_mesh(g, id) else { fail!("connectDynamic: {name} is not a mesh") };
    let t = crate::node::types::find(dy::COLLIDER).unwrap();
    let c = g.create_node(&t, "collider1", crate::plug::NO_NODE, true)?;
    let out = g.node(m).unwrap().ty().attr_index("outMesh").unwrap();
    let input = attr_plug(g, c, "inputMesh")?;
    g.connect(&crate::plug::PlugRef::new(m, out as i32), &input, true)?;
    connect(g, m, "worldMatrix", c, "inputMatrix")?;
    Ok(c)
}

/// dynExpression -s "rgbPP = <<1,0,0>>;" -c particlesShape1: kept on the particles; -q -s reads it back.
fn dyn_expression(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.named().into_iter().next() else { fail!("dynExpression: name particles") };
    let g = &mut s.graph;
    let p = particles_named(g, &name)?;
    let at = if a.bool("creation") { "creationExpression" } else { "runtimeExpression" };
    if a.bool("query") {
        return Ok(json!(g.node(p).unwrap().value(g.node(p).unwrap().ty().attr_index(at).unwrap()).as_str()));
    }
    set(g, p, at, PlugValue::String(a.string("string", "")))?;
    Ok(json!(g.path(p)))
}

/// Disk caches: -create solves the bodies from the start frame to the end and writes every frame; -append adds frames
/// from the end of the cache to the current frame; -replace writes a new cache over the old; -delete removes it.
fn dyn_cache(s: &mut Session, a: &Args) -> CmdResult {
    let mut bodies = Vec::new();
    for n in a.named() {
        match resolve(&s.graph, &n, &[dy::CLOTH]) {
            Some(b) => bodies.push(b),
            None => fail!("dynCache: {n} is not cloth"),
        }
    }
    if bodies.is_empty() {
        for id in s.graph.selection().to_vec() {
            if let Some(b) = resolve(&s.graph, &s.graph.path(id), &[dy::CLOTH]) {
                bodies.push(b);
            }
        }
    }
    if bodies.is_empty() {
        fail!("dynCache: select cloth");
    }
    let g = &mut s.graph;
    if a.bool("delete") {
        for &b in &bodies {
            let path = crate::dyn_solve::num(g, b, "", 0.0);
            let _ = path;
            let file = g.node(b).unwrap().value(g.node(b).unwrap().ty().attr_index("diskCache").unwrap()).as_str().to_string();
            if !file.is_empty() && a.bool("deleteFile") {
                let _ = std::fs::remove_file(&file);
            }
            set(g, b, "diskCache", PlugValue::String(String::new()))?;
        }
        crate::dyn_solve::forget_disk(g);
        return Ok(json!(bodies.iter().map(|&b| g.path(b)).collect::<Vec<_>>()));
    }
    let (lo, hi) = crate::dyn_cmds::playback_range();
    let start = a.float("startTime", lo);
    let end = a.float("endTime", hi);
    let append = a.bool("append");
    let file_for = |g: &Graph, b: NodeId| -> String {
        if a.has("file") {
            return a.string("file", "");
        }
        let dir = std::env::temp_dir();
        dir.join(format!("{}.azdc", g.node(b).unwrap().name())).to_string_lossy().into_owned()
    };
    let files: Vec<(NodeId, String)> = bodies.iter().map(|&b| (b, file_for(g, b))).collect();
    // Solving to write: the bodies play live while the cache is made.
    for (b, _) in &files {
        set(g, *b, "diskCache", PlugValue::String(String::new()))?;
    }
    crate::dyn_solve::forget_disk(g);
    if !append {
        for (_, f) in &files {
            let _ = std::fs::remove_file(f);
        }
    }
    let first = if append { crate::anim::frame(g).max(start) } else { start };
    let was = crate::anim::frame(g);
    let mut frame = first;
    let mut written = 0;
    while frame <= end + 1e-6 {
        crate::anim::set_frame(g, frame);
        g.evaluate();
        for (b, f) in &files {
            let Some(mesh) = g.node(*b).and_then(|n| n.data().mesh(crate::deform::OUT).cloned()) else { continue };
            let name = g.path(*b);
            aztek_dyn::cache::Cache::append(f, frame, &[(name, mesh.positions().to_vec())]).map_err(aztek_ops::CommandError::new)?;
        }
        written += 1;
        frame += 1.0;
    }
    for (b, f) in &files {
        set(g, *b, "diskCache", PlugValue::String(f.clone()))?;
    }
    crate::dyn_solve::forget_disk(g);
    crate::anim::set_frame(g, was);
    Ok(json!({"frames": written, "files": files.iter().map(|x| x.1.clone()).collect::<Vec<_>>()}))
}

pub fn register(c: &mut Commands<Session>) {
    for name in ["particle", "nParticle"] {
        c.register(
            q(Cmd::new(name, "Make particles: -p x y z for each one, or a grid with -ll, -ur and -grs. -q -ct counts them; -q -at position -id 3 reads one. Returns [transform, shape].", particle))
                .positional("particles (query)")
                .flag(f("position", "p", ArgType::List, "A particle there: x y z. Repeat it.").multi(3))
                .flag(f("lowerLeft", "ll", ArgType::Vec3, "Grid: one corner."))
                .flag(f("upperRight", "ur", ArgType::Vec3, "Grid: the other corner."))
                .flag(f("gridSpacing", "grs", ArgType::Float, "Grid: spacing."))
                .flag(f("conserve", "c", ArgType::Float, "Accepted for old scripts."))
                .flag(f("count", "ct", ArgType::Bool, "Query: how many."))
                .flag(f("attribute", "at", ArgType::String, "Query: a per particle value."))
                .flag(f("id", "id", ArgType::Int, "Query: which particle."))
                .flag(f("solver", "sv", ArgType::String, "The solver to join.")),
        );
    }
    c.register(
        q(Cmd::new("emitter", "Make an emitter feeding particles: -type omni, dir, surf, curve or vol, -r rate, -spd speed. With a mesh selected it emits off its surface. Returns [emitter, particles].", emitter))
            .positional("a mesh to emit from")
            .flag(f("type", "typ", ArgType::String, "omni, dir, surf, curve or vol."))
            .flag(f("rate", "r", ArgType::Float, "Particles per second."))
            .flag(f("speed", "spd", ArgType::Float, "Speed."))
            .flag(f("speedRandom", "srn", ArgType::Float, "How much speeds vary."))
            .flag(f("spread", "sp", ArgType::Float, "Directional: 0 to 1."))
            .flag(f("directionX", "dx", ArgType::Float, "Direction X."))
            .flag(f("directionY", "dy", ArgType::Float, "Direction Y."))
            .flag(f("directionZ", "dz", ArgType::Float, "Direction Z."))
            .flag(f("position", "pos", ArgType::Vec3, "Where it is."))
            .flag(f("volumeShape", "vsh", ArgType::String, "Volume: cube or sphere."))
            .flag(f("particles", "pt", ArgType::String, "Particles to feed (new ones by default)."))
            .flag(f("solver", "sv", ArgType::String, "The solver new particles join.")),
    );
    for (k, (name, _)) in FIELD_COMMANDS.iter().enumerate() {
        c.register(
            q(Cmd::new(name, &format!("Make a {name} field acting on the selected bodies, or -q/-e one. Returns [field]."), move |s, a| field_command(s, a, k)))
                .positional("bodies to act on")
                .flag(f("magnitude", "m", ArgType::Float, "Strength."))
                .flag(f("attenuation", "att", ArgType::Float, "How fast it weakens with distance."))
                .flag(f("maxDistance", "mxd", ArgType::Float, "How far it reaches (turns the limit on)."))
                .flag(f("useMaxDistance", "umd", ArgType::Bool, "Limit its reach."))
                .flag(f("directionX", "dx", ArgType::Float, "Direction X."))
                .flag(f("directionY", "dy", ArgType::Float, "Direction Y."))
                .flag(f("directionZ", "dz", ArgType::Float, "Direction Z."))
                .flag(f("speed", "s", ArgType::Float, "Air: wind speed."))
                .flag(f("inheritVelocity", "iv", ArgType::Float, "Air: kept motion."))
                .flag(f("frequency", "f", ArgType::Float, "Turbulence: noise frequency."))
                .flag(f("minDistance", "mnd", ArgType::Float, "Newton: least distance."))
                .flag(f("position", "pos", ArgType::Vec3, "Where it is.")),
        );
    }
    c.register(
        Cmd::new("connectDynamic", "Connect fields (-f), emitters (-em) and colliders (-c) to the bodies named; -d disconnects.", connect_dynamic)
            .positional("bodies")
            .flag(f("fields", "f", ArgType::List, "Fields."))
            .flag(f("emitters", "em", ArgType::List, "Emitters."))
            .flag(f("collisions", "c", ArgType::List, "Colliders."))
            .flag(f("delete", "d", ArgType::Bool, "Disconnect instead."))
            .flag(f("solver", "sv", ArgType::String, "The solver a body with none joins.")),
    );
    c.register(
        Cmd::new("getParticleAttr", "Per particle values: -at position -a true gives every one; without -a, the mean then the spread.", get_particle_attr)
            .positional("particles or particle1.pt[0:3]")
            .flag(f("attribute", "at", ArgType::String, "position, velocity, rgbPP, opacityPP, lifespanPP, age, radiusPP, particleId."))
            .flag(f("array", "a", ArgType::Bool, "Every value."))
            .undoable(false),
    );
    c.register(
        Cmd::new("setParticleAttr", "Set per particle values now: -at velocity -vv 0 1 0 particle1.pt[3].", set_particle_attr)
            .positional("particles or particle1.pt[0:3]")
            .flag(f("attribute", "at", ArgType::String, "position, velocity, rgbPP, opacityPP, lifespanPP, radiusPP."))
            .flag(f("vectorValue", "vv", ArgType::Vec3, "A vector."))
            .flag(f("floatValue", "fv", ArgType::Float, "A number."))
            .undoable(false),
    );
    c.register(
        Cmd::new("dynExpression", "Keep a creation (-c) or runtime (-rbd, -rad) expression on particles: -s the text.", dyn_expression)
            .positional("particles")
            .flag(f("string", "s", ArgType::String, "The expression."))
            .flag(f("creation", "c", ArgType::Bool, "Run for each new particle."))
            .flag(f("runtimeBeforeDynamics", "rbd", ArgType::Bool, "Run every frame."))
            .flag(f("runtimeAfterDynamics", "rad", ArgType::Bool, "Run every frame."))
            .flag(f("query", "q", ArgType::Bool, "Read it back.")),
    );
    c.register(
        Cmd::new("dynCache", "Disk caches for cloth: -create (cr) solves the range and writes it, -append (ap) adds frames to the current one, -replace (rp) writes over, -delete (d) stops using it.", dyn_cache)
            .positional("cloth")
            .flag(f("create", "cr", ArgType::Bool, "Make a cache over the range."))
            .flag(f("replace", "rp", ArgType::Bool, "Write a new cache over the old."))
            .flag(f("append", "ap", ArgType::Bool, "Add frames from the cache's end to now."))
            .flag(f("delete", "d", ArgType::Bool, "Stop using the cache."))
            .flag(f("deleteFile", "df", ArgType::Bool, "With -d: remove the file too."))
            .flag(f("file", "f", ArgType::String, "The file (in the temporary folder by default)."))
            .flag(f("startTime", "st", ArgType::Float, "First frame (the playback start)."))
            .flag(f("endTime", "et", ArgType::Float, "Last frame (the playback end)."))
            .undoable(false),
    );
}
