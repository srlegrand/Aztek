//! Files from any of the old dynamics systems, read into the one solver. Before the scene is built, each old node is
//! rewritten in the document as the node it maps to, so the rest of the reader builds it as it builds ours:
//!
//! - `nucleus`, `rigidSolver`, `bulletSolverShape`: a solver (a transform with a `solver` shape for the first two).
//! - `nCloth`: a `cloth` deformer between the mesh it read and the mesh it drove; its transform is dropped.
//! - `nParticle`, `particle`: `particles`, with their stored positions as start positions.
//! - `nRigid`: a `collider`; `rigidBody` and `bulletRigidBodyShape`: a `rigidBody` moving its object.
//! - `pointEmitter` and the fields (`airField`, `dragField`, `gravityField`, `newtonField`, `radialField`,
//!   `turbulenceField`, `uniformField`, `vortexField`, `volumeAxisField`): a transform with an `emitter` or `field`.
//! - `dynamicConstraint`: held as one, without its components.
//! - `hairSystem` and other nodes of those systems this does not simulate: held as they were (`heldNode`), written
//!   back as they came.
//!
//! What a node set that is not used here is kept on it, in `fileAttributes`, as the lines the file had; connections
//! that mean nothing here are dropped and counted in a warning.

use super::*;

/// Old type: (our type, made from a transform-type node so it needs a shape, field kind).
fn mapping(ty: &str) -> Option<(&'static str, bool, i64)> {
    Some(match ty {
        "nucleus" => ("solver", true, 0),
        "rigidSolver" => ("solver", true, 0),
        "bulletSolverShape" => ("solver", false, 0),
        "nCloth" => ("cloth", false, 0),
        "nParticle" | "particle" => ("particles", false, 0),
        "nRigid" => ("collider", false, 0),
        "rigidBody" => ("rigidBody", false, 0),
        "bulletRigidBodyShape" => ("rigidBody", false, 0),
        "pointEmitter" => ("emitter", true, 0),
        "dynamicConstraint" => ("dynamicConstraint", false, 0),
        "gravityField" => ("field", true, 0),
        "airField" => ("field", true, 1),
        "dragField" => ("field", true, 2),
        "newtonField" => ("field", true, 3),
        "radialField" => ("field", true, 4),
        "turbulenceField" => ("field", true, 5),
        "uniformField" => ("field", true, 6),
        "vortexField" => ("field", true, 7),
        "volumeAxisField" => ("field", true, 8),
        _ => return None,
    })
}

/// Types of those systems held as they were.
fn is_held(ty: &str) -> bool {
    matches!(ty, "hairSystem" | "pfxHair" | "nComponent" | "fluidShape" | "fluidEmitter" | "fluidTexture3D" | "particleSamplerInfo" | "cacheFile" | "cacheBlend" | "nucleusField" | "dynGlobals" | "bulletRigidBodyConstraintShape" | "bulletSoftBodyShape" | "bulletSoftConstraintShape" | "spring" | "rigidConstraint" | "geoConnector" | "collisionModel")
        || ty.to_lowercase().starts_with("dmm")
}

/// Attribute renames where an old short name differs from ours: (old, our long name).
const RENAMES: &[(&str, &str)] = &[
    ("grdx", "gravityDirectionX"), ("grdy", "gravityDirectionY"), ("grdz", "gravityDirectionZ"),
    ("widx", "windDirectionX"), ("widy", "windDirectionY"), ("widz", "windDirectionZ"),
    ("npox", "planeOriginX"), ("npoy", "planeOriginY"), ("npoz", "planeOriginZ"),
    ("dx", "directionX"), ("dy", "directionY"), ("dz", "directionZ"),
    ("directionX", "directionX"), ("directionY", "directionY"), ("directionZ", "directionZ"),
    ("phx", "phaseX"), ("phy", "phaseY"), ("phz", "phaseZ"),
    ("ivx", "initialVelocityX"), ("ivy", "initialVelocityY"), ("ivz", "initialVelocityZ"),
    ("isx", "initialSpinX"), ("isy", "initialSpinY"), ("isz", "initialSpinZ"),
    ("max", "maxDistance"),
];

fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Num(v) => fmt_num(*v),
        Tok::Word(w) => w.clone(),
        Tok::Str(s) => quoted(s),
    }
}

/// A setAttr line as the file had it.
fn set_line(s: &SetAttr) -> String {
    let path: Vec<String> = s
        .path
        .iter()
        .map(|g| match g.range {
            Some((a, b)) if a == b => format!("{}[{a}]", g.name),
            Some((a, b)) => format!("{}[{a}:{b}]", g.name),
            None => g.name.clone(),
        })
        .collect();
    let ty = s.ty.as_ref().map(|t| format!(" -type {}", quoted(t))).unwrap_or_default();
    let values: Vec<String> = s.values.iter().map(tok_text).collect();
    format!("setAttr \".{}\"{ty} {};", path.join("."), values.join(" "))
}

fn string_set(short: &str, value: &str) -> SetAttr {
    SetAttr { path: vec![Seg { name: short.into(), range: None }], lock: None, values: vec![Tok::Str(value.into())], ty: Some("string".into()) }
}

fn num_set(short: &str, v: f64) -> SetAttr {
    SetAttr { path: vec![Seg { name: short.into(), range: None }], lock: None, values: vec![Tok::Num(v)], ty: None }
}

fn known(t: &NodeType, name: &str) -> bool {
    t.attrs.iter().any(|a| a.name == name || a.short_name == name || a.child_names.iter().any(|c| c == name) || a.child_short.iter().any(|c| c == name))
}

/// Splits sets into those `t` knows (renamed where needed) and the lines of the rest.
fn split(sets: Vec<SetAttr>, t: &NodeType) -> (Vec<SetAttr>, Vec<String>, Vec<String>) {
    let mut kept = Vec::new();
    let mut lines = Vec::new();
    let mut names = Vec::new();
    for mut s in sets {
        let Some(first) = s.path.first().map(|g| g.name.clone()) else { continue };
        if let Some((_, to)) = RENAMES.iter().find(|(from, _)| *from == first) {
            if known(t, to) {
                s.path[0].name = to.to_string();
            }
        }
        if s.path.len() == 1 && known(t, &s.path[0].name) {
            kept.push(s);
        } else {
            if !names.contains(&first) {
                names.push(first);
            }
            lines.push(set_line(&s));
        }
    }
    (kept, lines, names)
}

fn shape_name(name: &str) -> String {
    let base = name.trim_end_matches(|c: char| c.is_ascii_digit());
    format!("{base}Shape{}", &name[base.len()..])
}

/// Transform attributes stay on the transform when a transform-type node is split in two.
fn is_transform_attr(name: &str) -> bool {
    types::find("transform").is_some_and(|t| known(&t, name)) || matches!(name, "v" | "visibility")
}

/// Rewrites the document's old dynamics nodes as ours. Returns warnings.
pub(super) fn translate(doc: &mut Doc) -> Vec<String> {
    crate::dynamics::register_types();
    let mut warnings = Vec::new();
    let count = doc.nodes.len();
    // Old index -> the index of the node it became (a shape for transform-type nodes).
    let mut became: HashMap<usize, usize> = HashMap::new();
    let mut held: Vec<usize> = Vec::new();
    let mut dropped_parents = Vec::new();
    for i in 0..count {
        let old = doc.nodes[i].ty.clone();
        if is_held(&old) {
            held.push(i);
            continue;
        }
        let Some((ours, split_shape, field)) = mapping(&old) else { continue };
        // Ours share these names and sit under no transform; the other systems' are shapes under one.
        if matches!(old.as_str(), "dynamicConstraint" | "rigidBody") && doc.nodes[i].parent.is_none() {
            continue;
        }
        let t = types::find(ours).expect(ours);
        let name = doc.nodes[i].name.clone();
        let sets = std::mem::take(&mut doc.nodes[i].sets);
        let (on_transform, rest): (Vec<SetAttr>, Vec<SetAttr>) = if split_shape { sets.into_iter().partition(|s| s.path.first().is_some_and(|g| is_transform_attr(&g.name))) } else { (Vec::new(), sets) };
        let (mut kept, lines, names) = split(rest, &t);
        // Settings that mean something different here.
        if old == "nucleus" {
            for s in kept.iter_mut().filter(|s| s.path[0].name == "sstp" || s.path[0].name == "subSteps") {
                if let Some(v) = s.values.first().and_then(Tok::num) {
                    s.values = vec![Tok::Num((v * 4.0).clamp(8.0, 64.0))];
                }
            }
        }
        kept.push(string_set("fty", &old));
        // The old solvers count a unit as a metre unless told otherwise; read so, the file plays as it did. Their cloth
        // weighs by the point.
        if old == "nucleus" && !kept.iter().any(|s| matches!(s.path[0].name.as_str(), "spsc" | "spaceScale")) {
            kept.push(num_set("spsc", 1.0));
        }
        if old == "nCloth" {
            kept.push(num_set("dens", 0.0));
        }
        if !lines.is_empty() {
            kept.push(string_set("fat", &lines.join("\n")));
            warnings.push(format!("{name} ({old}): read as {ours}; not used here, kept: {}", names.join(", ")));
        }
        if ours == "field" {
            kept.push(num_set("fdt", field as f64));
        }
        if split_shape {
            doc.nodes[i].ty = "transform".into();
            doc.nodes[i].sets = on_transform;
            let shape = doc.add_node(MaNode { ty: ours.into(), name: shape_name(&name), parent: Some(i), sets: kept, ..Default::default() });
            became.insert(i, shape);
        } else {
            doc.nodes[i].ty = ours.into();
            doc.nodes[i].sets = kept;
            became.insert(i, i);
            // A DG node here under a transform that only held it: the transform goes.
            if matches!(ours, "cloth" | "collider" | "dynamicConstraint" | "rigidBody") {
                if let Some(p) = doc.nodes[i].parent.take() {
                    let only = (0..count).filter(|&k| doc.nodes[k].parent == Some(p)).count() == 0;
                    if ours == "rigidBody" {
                        // A rigid body moves the object it sat under.
                        let path = doc.nodes[p].name.clone();
                        doc.nodes[i].sets.push(string_set("obj", &path));
                    } else if only && doc.nodes[p].ty == "transform" {
                        dropped_parents.push(p);
                    }
                }
            }
        }
        if ours == "dynamicConstraint" {
            warnings.push(format!("{name}: a constraint from another system is held, but what it held is not read: make it again with dynamicConstraint"));
        }
    }
    for p in dropped_parents {
        doc.nodes[p].ty = String::new();
    }

    // Connections.
    let is_ours = |_: &Doc, i: usize| became.values().any(|&b| b == i);
    let ty_of = |doc: &Doc, i: usize| doc.nodes[i].ty.clone();
    let mut conns = std::mem::take(&mut doc.conns);
    let mut out: Vec<Conn> = Vec::new();
    let mut dropped: BTreeMap<String, usize> = BTreeMap::new();
    let mut held_lines: HashMap<usize, Vec<String>> = HashMap::new();
    let mut solver_links: Vec<(usize, usize)> = Vec::new();
    for mut c in conns.drain(..) {
        // Held nodes keep their connections as text, written back as they came.
        if held.contains(&c.src) || held.contains(&c.dst) {
            let name = |i: usize| if doc.nodes[i].default { format!(":{}", doc.nodes[i].name) } else { doc.nodes[i].name.clone() };
            let line = format!("connectAttr {} {};", quoted(&format!("{}.{}", name(c.src), c.src_attr)), quoted(&format!("{}.{}", name(c.dst), c.dst_attr)));
            let owner = if held.contains(&c.dst) { c.dst } else { c.src };
            held_lines.entry(owner).or_default().push(line);
            continue;
        }
        if let Some(&b) = became.get(&c.src) {
            if !is_transform_attr(&plain_attr(&c.src_attr)) {
                c.src = b;
            }
        }
        if let Some(&b) = became.get(&c.dst) {
            if !is_transform_attr(&plain_attr(&c.dst_attr)) {
                c.dst = b;
            }
        }
        if !is_ours(doc, c.src) && !is_ours(doc, c.dst) {
            out.push(c);
            continue;
        }
        let (st, dt) = (ty_of(doc, c.src), ty_of(doc, c.dst));
        let (sa, da) = (plain_attr(&c.src_attr), plain_attr(&c.dst_attr));
        let body = |t: &str| matches!(t, "cloth" | "particles" | "collider" | "rigidBody");
        // Any connection between a solver and a body: the body belongs to it.
        if (st == "solver" && body(&dt)) || (dt == "solver" && body(&st)) {
            let (s, b) = if st == "solver" { (c.src, c.dst) } else { (c.dst, c.src) };
            if !solver_links.contains(&(s, b)) {
                solver_links.push((s, b));
            }
            continue;
        }
        // A field's force into a body.
        if st == "field" && body(&dt) {
            if matches!(da.as_str(), "ifc" | "inputForce") {
                out.push(Conn { src: c.src, src_attr: "msg".into(), dst: c.dst, dst_attr: c.dst_attr.replace("inputForce", "ifc") });
            }
            continue;
        }
        // An emitter's new particles.
        if st == "emitter" && dt == "particles" {
            if matches!(da.as_str(), "npt" | "newParticles") {
                out.push(Conn { src: c.src, src_attr: "msg".into(), dst: c.dst, dst_attr: c.dst_attr.replace("newParticles", "npt") });
            }
            continue;
        }
        // The mesh a cloth reads goes through the chain as a deformer's input.
        if dt == "cloth" && matches!(da.as_str(), "imsh" | "inputMesh") {
            out.push(Conn { dst_attr: "ip[0].ig".into(), ..c });
            continue;
        }
        if st == "cloth" && matches!(sa.as_str(), "omsh" | "outputMesh") {
            out.push(Conn { src_attr: "og[0]".into(), ..c });
            continue;
        }
        // A collider or surface emitter's mesh, in world space there; here object space and the matrix beside it.
        if matches!(dt.as_str(), "collider" | "emitter") && matches!(da.as_str(), "imsh" | "inputMesh" | "ig" | "inputGeometry") && doc.nodes[c.src].ty == "mesh" {
            out.push(Conn { src: c.src, src_attr: "o".into(), dst: c.dst, dst_attr: "imsh".into() });
            out.push(Conn { src: c.src, src_attr: "wm[0]".into(), dst: c.dst, dst_attr: "imx".into() });
            continue;
        }
        // What is known at both ends stays.
        let ok_end = |i: usize, attr: &str| -> bool {
            let t = doc.nodes[i].ty.as_str();
            if !body(t) && !matches!(t, "solver" | "field" | "emitter" | "dynamicConstraint") {
                return true;
            }
            types::find(t).is_some_and(|ty| known(&ty, attr) || attr == "msg" || attr == "message")
        };
        if ok_end(c.src, &sa) && ok_end(c.dst, &da) {
            out.push(c);
        } else {
            *dropped.entry(format!("{} ({})", doc.nodes[if is_ours(doc, c.dst) { c.dst } else { c.src }].name, if is_ours(doc, c.dst) { dt } else { st })).or_default() += 1;
        }
    }
    for (s, b) in solver_links {
        out.push(Conn { src: s, src_attr: "msg".into(), dst: b, dst_attr: "slv".into() });
    }
    // A rigid body the solver moves drives its object's translate and rotate.
    for (&_, &now) in &became {
        if doc.nodes[now].ty != "rigidBody" {
            continue;
        }
        let active = doc.nodes[now].truth(&["act", "active"]).unwrap_or(true);
        let obj = doc.nodes[now].text(&["obj"]).map(str::to_string);
        if let (true, Some(o)) = (active, obj.and_then(|o| doc.find(&o))) {
            out.push(Conn { src: now, src_attr: "otr".into(), dst: o, dst_attr: "t".into() });
            out.push(Conn { src: now, src_attr: "oro".into(), dst: o, dst_attr: "r".into() });
        }
    }
    doc.conns = out;
    for (n, k) in dropped {
        warnings.push(format!("{n}: {k} connection(s) from another system not used here"));
    }

    // Held nodes: everything kept as text.
    for i in held {
        let n = &mut doc.nodes[i];
        let old = std::mem::take(&mut n.ty);
        let lines: Vec<String> = n.sets.iter().map(set_line).collect();
        let parent = n.parent.take();
        n.sets = vec![string_set("fty", &old), string_set("fat", &lines.join("\n"))];
        n.ty = "heldNode".into();
        let parent_name = parent.map(|p| doc.nodes[p].name.clone());
        let n = &mut doc.nodes[i];
        if let Some(p) = parent_name {
            n.sets.push(string_set("fpr", &p));
        }
        if let Some(l) = held_lines.remove(&i) {
            n.sets.push(string_set("fcn", &l.join("\n")));
        }
        warnings.push(format!("{} ({old}): not simulated here; held as it was and written back", n.name));
    }
    warnings
}

impl Build<'_> {
    /// After the nodes exist: particles' stored positions, and rigid bodies' meshes.
    pub(super) fn read_dynamics(&mut self) {
        let doc = self.doc;
        for i in 0..doc.nodes.len() {
            let n = &doc.nodes[i];
            if n.ty != "particles" {
                continue;
            }
            let Some(id) = self.id(i) else { continue };
            // vectorArray: the count, then x y z for each.
            let lines = n.text(&["fat"]).unwrap_or("");
            let mut pos: Vec<f64> = Vec::new();
            for line in lines.lines() {
                let is_pos0 = line.starts_with("setAttr \".pos0\"") || line.starts_with("setAttr \".position0\"");
                let is_pos = line.starts_with("setAttr \".pos\"") || line.starts_with("setAttr \".position\"");
                if !(is_pos0 || (is_pos && pos.is_empty())) {
                    continue;
                }
                let nums: Vec<f64> = line.split_whitespace().filter_map(|w| w.trim_end_matches(';').parse::<f64>().ok()).collect();
                if let Some((&count, rest)) = nums.split_first() {
                    let k = (count.max(0.0) as usize * 3).min(rest.len());
                    pos = rest[..k].to_vec();
                }
            }
            if !pos.is_empty() {
                let g = &mut self.s.graph;
                if let Some(a) = g.node(id).and_then(|n| n.ty().attr_index("startPositions")) {
                    let _ = crate::deform::set_floats(g, id, a, pos);
                }
            }
        }
        if doc.nodes.iter().any(|n| n.ty == "solver") {
            crate::dyn_solve::SOLVERS_MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl Writer<'_> {
    /// A held node, as the file it came from had it.
    pub(super) fn write_held(&mut self, id: NodeId) {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let get = |name: &str| n.ty().attr_index(name).map(|a| n.value(a).as_str().to_string()).unwrap_or_default();
        let (ty, lines, parent, conns) = (get("fileType"), get("fileAttributes"), get("fileParent"), get("fileConnections"));
        if ty.is_empty() {
            return;
        }
        let p = if parent.is_empty() { String::new() } else { format!(" -p {}", quoted(&parent)) };
        let mut text = format!("createNode {ty} -n {}{p};\n", quoted(&g.path(id)));
        for l in lines.lines() {
            let _ = writeln!(text, "\t{l}");
        }
        self.out.push_str(&text);
        self.conns.extend(conns.lines().map(str::to_string));
    }
}
