//! The reference's scene commands with their full flag sets: hierarchy, naming, selection, sets, namespaces, undo and the small
//! commands scripts lean on. Older commands of the same name keep doing what they did; these add to them.

use aztek_ops::{fail, objects, run, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::ref_core::{asked, extend, f, matching_nodes};
use crate::node::types;
use crate::plug::{flags, NodeId, PlugRef, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::{DagNode, Transform};

type Cmd = CommandDef<Session>;
const B: ArgType = ArgType::Bool;
const S: ArgType = ArgType::String;
const A: ArgType = ArgType::Any;

/// A relative as listRelatives names it: its leaf name, even if that isn't unique, a path only when asked for.
fn relative_name(g: &Graph, id: NodeId, a: &Args) -> String {
    if a.bool("fullPath") {
        g.full_path(id)
    } else if a.bool("path") || !aztek_ops::units::user_call() {
        g.path(id)
    } else {
        g.node(id).map_or_else(String::new, |n| n.name().to_string())
    }
}

fn type_list(a: &Args, flag: &str) -> Vec<String> {
    match a.get(flag) {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn node_of(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    g.find(name)?.ok_or_else(|| aztek_ops::CommandError::new(format!("{cmd}: no object named {name}")))
}

fn list_relatives(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let types_wanted = type_list(a, "type");
    let ni = a.bool("noIntermediate");
    let mut out: Vec<NodeId> = Vec::new();
    for name in objects(s, a) {
        let id = node_of(g, &name, "listRelatives")?;
        let found: Vec<NodeId> = if a.bool("allParents") {
            // An instanced shape has a parent per instance.
            let shared = crate::instancing::one_shape(g, id);
            let all = if shared.len() > 1 { shared } else { vec![id] };
            all.into_iter().map(|m| g.parent(m)).filter(|&p| p != NO_NODE).collect()
        } else if a.bool("parent") {
            let p = g.parent(id);
            if p == NO_NODE { Vec::new() } else { vec![p] }
        } else if a.bool("allDescendents") {
            // Deepest first, as the reference lists them.
            let mut d = g.descendants(id);
            d.reverse();
            d
        } else if a.bool("shapes") {
            if crate::dag::is_shape(g, id) { vec![id] } else { g.children(id).to_vec() }
        } else {
            g.children(id).to_vec()
        };
        for x in found {
            if !out.contains(&x) {
                out.push(x);
            }
        }
    }
    out.retain(|&x| {
        let Some(n) = g.node(x) else { return false };
        (!a.bool("shapes") || crate::dag::is_shape(g, x))
            && (!ni || !n.ty().dag || !n.bool(DagNode::INTERMEDIATE_OBJECT))
            && (types_wanted.is_empty() || types_wanted.iter().any(|t| n.ty().is_a(t)))
    });
    Ok(json!(out.iter().map(|&x| relative_name(g, x, a)).collect::<Vec<_>>()))
}

/// listConnections: walking through unitConversion nodes when asked, a shape reported as its transform unless -sh.
fn list_connections(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let want_src = a.get("source").and_then(Value::as_bool).unwrap_or(true);
    let want_dst = a.get("destination").and_then(Value::as_bool).unwrap_or(true);
    let (plugs, pairs, shapes, skip) = (a.bool("plugs"), a.bool("connections"), a.bool("shapes"), a.bool("skipConversionNodes"));
    let (ty, exact) = (type_list(a, "type"), type_list(a, "exactType"));
    let conns = g.connections();
    let conversion = |id: NodeId| g.node(id).is_some_and(|n| n.ty().name == "unitConversion");
    let mut out: Vec<Value> = Vec::new();
    for name in objects(s, a) {
        let (node, attr): (NodeId, Option<PlugRef>) = if name.contains('.') {
            let p = g.plug(&name)?;
            (p.node, Some(p))
        } else {
            (node_of(g, &name, "listConnections")?, None)
        };
        let matches = |p: &PlugRef| {
            p.node == node
                && attr.as_ref().is_none_or(|q| {
                    p.attr == q.attr
                        && (q.element < 0 || p.element < 0 || p.element == q.element)
                        && (q.component < 0 || p.component < 0 || p.component == q.component)
                })
        };
        for (d, sp) in &conns {
            let mut hits = Vec::new();
            if want_dst && matches(sp) {
                hits.push((*sp, *d));
            }
            if want_src && matches(d) {
                hits.push((*d, *sp));
            }
            for (mine, mut other) in hits {
                if skip {
                    // Walk through conversion nodes to what is beyond them.
                    let upstream = other == *sp;
                    let mut guard = 0;
                    while conversion(other.node) && guard < 32 {
                        guard += 1;
                        let next = if upstream {
                            conns.iter().find(|(x, _)| x.node == other.node).map(|(_, y)| *y)
                        } else {
                            conns.iter().find(|(_, y)| y.node == other.node).map(|(x, _)| *x)
                        };
                        let Some(n) = next else { break };
                        other = n;
                    }
                }
                let Some(on) = g.node(other.node) else { continue };
                if (!ty.is_empty() && !ty.iter().any(|t| on.ty().is_a(t))) || (!exact.is_empty() && !exact.iter().any(|t| on.ty().name == *t)) {
                    continue;
                }
                if pairs {
                    out.push(json!(g.plug_name(&mine)));
                }
                let shown = if !shapes && crate::dag::is_shape(g, other.node) { crate::dag::transform_of(g, other.node) } else { other.node };
                let v = if plugs { json!(g.plug_name(&other)) } else { json!(g.path(shown)) };
                if pairs || !out.contains(&v) {
                    out.push(v);
                }
            }
        }
    }
    Ok(Value::Array(out))
}

fn is_constraint(t: &crate::node::NodeType) -> bool {
    t.name.ends_with("Constraint")
}

fn is_anim_curve(t: &crate::node::NodeType) -> bool {
    t.name.starts_with("animCurve")
}

fn delete_cmd(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    if a.bool("constructionHistory") {
        return prev(s, a);
    }
    let selective = ["channels", "expressions", "constraints", "staticChannels", "timeAnimationCurves", "unitlessAnimationCurves", "shape", "inputConnectionsAndNodes"].iter().any(|k| a.bool(k)) || a.has("attribute");
    if !selective {
        // What a plain delete leaves driving nothing goes too.
        let mut doomed = Vec::new();
        for n in objects(s, a) {
            if let Some(id) = s.graph.find(&n)? {
                doomed.push(id);
                doomed.extend(s.graph.descendants(id));
            }
        }
        let orphans = crate::expression::orphans(&s.graph, &doomed);
        let r = prev(s, a)?;
        for o in orphans {
            if s.graph.node(o).is_some() {
                s.graph.delete_node(o)?;
            }
        }
        return Ok(r);
    }
    let mut targets = Vec::new();
    for n in objects(s, a) {
        let id = if n.contains('.') { s.graph.plug(&n)?.node } else { node_of(&s.graph, &n, "delete")? };
        targets.push(id);
        if a.string("hierarchy", "none") == "below" || a.string("hierarchy", "none") == "both" {
            targets.extend(s.graph.descendants(id));
        }
    }
    let attrs = type_list(a, "attribute");
    let g = &mut s.graph;
    for id in targets {
        if a.bool("shape") {
            for sh in crate::dag::shapes(g, id, true) {
                g.delete_node(sh)?;
            }
            continue;
        }
        let mut doomed = Vec::new();
        for (d, sp) in g.connections() {
            if d.node != id {
                continue;
            }
            if !attrs.is_empty() {
                let def = g.attr(&d);
                let hit = attrs.iter().any(|w| *w == def.name || *w == def.short_name || (d.component >= 0 && (*w == def.child_names[d.component as usize] || *w == def.child_short[d.component as usize])));
                if !hit {
                    continue;
                }
            }
            let Some(src) = g.node(sp.node) else { continue };
            let t = src.ty();
            let curve = is_anim_curve(t);
            let driven_key = t.name.starts_with("animCurveU");
            let hit = (a.bool("expressions") && t.name == "expression")
                || (a.bool("constraints") && is_constraint(t))
                || (a.bool("channels") && (curve || t.name == "expression"))
                || (a.bool("timeAnimationCurves") && curve && !driven_key)
                || (a.bool("unitlessAnimationCurves") && driven_key)
                || (a.bool("staticChannels") && curve && crate::anim::keys_of(g, sp.node).keys.windows(2).all(|w| (w[0].v - w[1].v).abs() < 1e-9))
                || a.bool("inputConnectionsAndNodes")
                || (a.has("attribute") && !a.bool("channels") && !a.bool("expressions") && !a.bool("constraints"));
            if hit && !doomed.contains(&sp.node) {
                doomed.push(sp.node);
            }
        }
        // Constraint nodes parented under the object count as its constraints too.
        if a.bool("constraints") {
            for &c in g.children(id).to_vec().iter() {
                if g.node(c).is_some_and(|n| is_constraint(n.ty())) && !doomed.contains(&c) {
                    doomed.push(c);
                }
            }
        }
        for d in doomed {
            if g.node(d).is_some() {
                g.delete_node(d)?;
            }
        }
    }
    Ok(Value::Null)
}

fn select_cmd(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let g = &s.graph;
    let mut ids: Vec<NodeId> = Vec::new();
    let mut items: Vec<Value> = Vec::new();
    if a.bool("all") {
        // Every object, not the default cameras.
        ids = g.nodes().into_iter().filter(|&i| g.node(i).is_some_and(|n| n.ty().dag) && !crate::dag::is_shape(g, i) && !crate::cameras::is_default_camera(g, i)).collect();
    } else if a.bool("allDagObjects") {
        ids = g.roots().iter().copied().filter(|&i| g.node(i).is_some_and(|n| n.ty().dag)).collect();
    } else if a.bool("allDependencyNodes") {
        ids = g.nodes();
    }
    for name in a.named() {
        if crate::components::is_component_string(&name) {
            items.push(json!(name));
            continue;
        }
        let found = if name.contains('*') || name.contains('?') { matching_nodes(g, &name) } else { vec![node_of(g, &name, "select")?] };
        for id in found {
            // A set stands for its members, and those of the sets it holds, unless -ne.
            if !a.bool("noExpand") && g.node(id).is_some_and(crate::object_sets::is_set) {
                let list = crate::object_sets::flattened(g, id);
                for m in crate::object_sets::format(g, &list) {
                    items.push(json!(m));
                }
            } else {
                ids.push(id);
            }
        }
    }
    if a.bool("hierarchy") {
        let mut all = Vec::new();
        for &id in &ids {
            if !all.contains(&id) {
                all.push(id);
            }
            for d in g.descendants(id) {
                if !all.contains(&d) {
                    all.push(d);
                }
            }
        }
        ids = all;
    }
    if a.bool("visible") {
        ids.retain(|&i| crate::transform::effectively_visible(g, i));
    }
    let expanded = a.bool("all") || a.bool("allDagObjects") || a.bool("allDependencyNodes") || a.bool("hierarchy") || !a.named().is_empty();
    if a.bool("addFirst") {
        let mut front = ids.clone();
        front.extend(g.selection().iter().copied().filter(|i| !ids.contains(i)));
        s.graph.select(&front, SelectMode::Replace);
        return Ok(Value::Null);
    }
    if !expanded {
        return prev(s, a);
    }
    let mut call = a.clone();
    call.positional = ids.iter().map(|&i| json!(s.graph.path(i))).chain(items).collect();
    if call.positional.is_empty() && !a.bool("add") && !a.bool("deselect") && !a.bool("toggle") {
        s.graph.select(&[], SelectMode::Replace);
        return Ok(Value::Null);
    }
    if call.positional.is_empty() {
        return Ok(Value::Null);
    }
    prev(s, &call)
}

/// Copies a node's input connections onto its duplicate (duplicate -ic).
fn copy_inputs(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    for (d, sp) in g.connections() {
        if d.node == from {
            g.connect(&sp, &PlugRef { node: to, ..d }, true)?;
        }
    }
    Ok(())
}

/// Copies the non-DAG nodes feeding `from` and feeds `to` from the copies (duplicate -un).
fn copy_upstream(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    for (d, sp) in g.connections() {
        if d.node != from {
            continue;
        }
        let Some(ty) = g.node(sp.node).map(|n| n.type_arc().clone()) else { continue };
        if ty.dag {
            g.connect(&sp, &PlugRef { node: to, ..d }, true)?;
            continue;
        }
        let made = g.create_node(&ty, "", NO_NODE, true)?;
        for (i, def) in ty.attrs.iter().enumerate() {
            if def.has(flags::INPUT) && !def.is_array() {
                let v = g.node(sp.node).unwrap().value(i).clone();
                g.set_value(PlugRef::new(made, i as i32), v)?;
            }
        }
        copy_inputs(g, sp.node, made)?;
        g.connect(&PlugRef { node: made, ..sp }, &PlugRef { node: to, ..d }, true)?;
    }
    Ok(())
}

fn duplicate_cmd(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    if a.bool("smartTransform") {
        return run(s, "duplicateWithTransform", json!({}));
    }
    let names = objects(s, a);
    let sources: Vec<NodeId> = names.iter().filter_map(|n| s.graph.find(n).ok().flatten()).collect();
    let made = prev(s, a)?;
    let new: Vec<String> = made.as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
    let g = &mut s.graph;
    let mut out = Vec::new();
    for (k, n) in new.iter().enumerate() {
        let Some(id) = g.find(n)? else { continue };
        let source = sources.get(k % sources.len().max(1)).copied();
        if a.bool("parentOnly") {
            for c in g.children(id).to_vec() {
                if !crate::dag::is_shape(g, c) {
                    g.delete_node(c)?;
                }
            }
            for c in g.children(id).to_vec() {
                g.delete_node(c)?;
            }
        } else if a.bool("transformsOnly") {
            for d in g.descendants(id) {
                if g.node(d).is_some() && crate::dag::is_shape(g, d) {
                    g.delete_node(d)?;
                }
            }
        }
        if let Some(src) = source {
            if a.bool("inputConnections") {
                copy_inputs(g, src, id)?;
            } else if a.bool("upstreamNodes") {
                copy_upstream(g, src, id)?;
            }
        }
        if a.bool("renameChildren") {
            for d in g.descendants(id) {
                let stem = g.node(d).unwrap().name().to_string();
                let wanted = g.available_name(&stem, NO_NODE);
                let unique = unique_everywhere(g, &wanted);
                g.rename(d, &unique)?;
            }
        }
        out.push(json!(g.path(id)));
        if !a.bool("returnRootsOnly") && !a.bool("parentOnly") {
            for d in g.descendants(id) {
                if !crate::dag::is_shape(g, d) {
                    out.push(json!(g.path(d)));
                }
            }
        }
    }
    Ok(Value::Array(out))
}

/// A name no node in the scene holds, counting up from what was asked.
fn unique_everywhere(g: &Graph, wanted: &str) -> String {
    if g.find_all(wanted).is_empty() {
        return wanted.to_string();
    }
    let digits = wanted.len() - wanted.bytes().rev().take_while(u8::is_ascii_digit).count();
    let stem = &wanted[..digits];
    let width = wanted.len() - digits;
    let mut k: u64 = wanted[digits..].parse().unwrap_or(1);
    loop {
        let name = format!("{stem}{k:0width$}");
        if g.find_all(&name).is_empty() {
            return name;
        }
        k += 1;
    }
}

fn parent_cmd(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    let objs = objects(s, a);
    if a.bool("removeObject") {
        for n in &objs {
            let id = node_of(&s.graph, n, "parent")?;
            s.graph.delete_node(id)?;
        }
        return Ok(Value::Null);
    }
    if a.bool("addObject") && a.bool("shape") && objs.len() >= 2 {
        // A shape added under another transform: the reference makes it one shape with two parents. Aztek has no
        // shapes with two parents, so the shape itself moves under the new transform, as named, and its old transform
        // keeps a copy. A script holding the shape (to rename it, as rigging tools do next) holds the one under the new
        // transform.
        let p = node_of(&s.graph, objs.last().unwrap(), "parent")?;
        let mut out = Vec::new();
        for n in &objs[..objs.len() - 1] {
            let id = node_of(&s.graph, n, "parent")?;
            let shapes = if crate::dag::is_shape(&s.graph, id) { vec![id] } else { crate::dag::shapes(&s.graph, id, true) };
            for sh in shapes {
                let old_parent = s.graph.parent(sh);
                if old_parent != NO_NODE {
                    let holder = s.graph.path(old_parent);
                    let made = run(s, "duplicate", json!({"args": [holder]}))?;
                    let copy_root = node_of(&s.graph, made[0].as_str().unwrap_or_default(), "parent")?;
                    let short = s.graph.node(sh).map(|x| x.name().to_string()).unwrap_or_default();
                    // The copy of this shape is the one of the same place among the copied transform's shapes.
                    let place = crate::dag::shapes(&s.graph, old_parent, true).iter().position(|&x| x == sh);
                    let copies = crate::dag::shapes(&s.graph, copy_root, true);
                    if let Some(c) = place.and_then(|i| copies.get(i).copied()) {
                        s.graph.set_parent(c, old_parent, false, usize::MAX)?;
                        let _ = s.graph.rename(c, &format!("{short}Orig"));
                    }
                    s.graph.delete_node(copy_root)?;
                }
                s.graph.set_parent(sh, p, false, usize::MAX)?;
                out.push(json!(s.graph.path(sh)));
            }
        }
        return Ok(Value::Array(out));
    }
    if a.bool("addObject") && objs.len() >= 2 {
        let parent = objs.last().unwrap().clone();
        let mut out = Vec::new();
        for n in &objs[..objs.len() - 1] {
            let made = run(s, "instance", json!({"args": [n]}))?;
            let name = made[0].as_str().unwrap_or_default().to_string();
            let id = node_of(&s.graph, &name, "parent")?;
            let p = node_of(&s.graph, &parent, "parent")?;
            s.graph.set_parent(id, p, !a.bool("relative"), usize::MAX)?;
            out.push(json!(s.graph.path(id)));
        }
        return Ok(Value::Array(out));
    }
    if a.bool("shape") && objs.len() >= 2 {
        // Shapes move under another transform as they are: this is how curve shapes join into one control.
        let p = node_of(&s.graph, objs.last().unwrap(), "parent")?;
        let mut out = Vec::new();
        for n in &objs[..objs.len() - 1] {
            let id = node_of(&s.graph, n, "parent")?;
            for sh in if crate::dag::is_shape(&s.graph, id) { vec![id] } else { crate::dag::shapes(&s.graph, id, true) } {
                s.graph.set_parent(sh, p, false, usize::MAX)?;
                out.push(json!(s.graph.path(sh)));
            }
        }
        return Ok(Value::Array(out));
    }
    prev(s, a)
}

fn group_cmd(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    // A group goes under its members' lowest common parent unless -w or -p says otherwise.
    let mut call = a.clone();
    if !a.bool("world") && !a.has("parent") && !a.bool("empty") {
        let g = &s.graph;
        let names = aztek_ops::objects(s, a);
        let ids: Vec<NodeId> = names.iter().filter_map(|n| g.find(n).ok().flatten()).collect();
        let ancestors = |id: NodeId| {
            let mut v = Vec::new();
            let mut p = g.parent(id);
            while p != NO_NODE {
                v.push(p);
                p = g.parent(p);
            }
            v
        };
        if let Some(&first) = ids.first() {
            let common = ancestors(first).into_iter().find(|p| ids[1..].iter().all(|&i| ancestors(i).contains(p)));
            if let Some(p) = common {
                call.flags.insert("parent".into(), json!(g.path(p)));
            }
        }
    }
    call.flags.remove("world");
    call.flags.remove("absolute");
    call.flags.remove("relative");
    let made = prev(s, &call)?;
    // Its pivots sit at the world's origin, wherever the group ends up, unless it was made empty.
    if !a.bool("empty") {
        let name = made.as_str().unwrap_or_default().to_string();
        s.run("xform", json!({"args": [name], "worldSpace": true, "rotatePivot": [0.0, 0.0, 0.0], "scalePivot": [0.0, 0.0, 0.0]}))?;
    }
    Ok(made)
}

/// One question to polyEvaluate. Several in one call give a dictionary by long name in Python and an array in MEL.
fn poly_evaluate(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    const SINGLE: &[&str] = &["vertex", "edge", "face", "uvcoord", "triangle", "shell", "boundingBox", "boundingBox2d", "area", "worldArea", "vertexComponent", "edgeComponent", "faceComponent", "uvComponent", "triangleComponent", "boundingBoxComponent", "boundingBoxComponent2d", "uvShell"];
    let asked: Vec<&str> = SINGLE.iter().copied().filter(|f| a.bool(f)).collect();
    if asked.len() > 1 {
        let mut out = serde_json::Map::new();
        for f in asked {
            let mut one = a.clone();
            for g in SINGLE {
                one.flags.remove(*g);
            }
            one.flags.insert(f.to_string(), json!(true));
            out.insert(f.to_string(), poly_evaluate_one(s, &one, prev)?);
        }
        return Ok(Value::Object(out));
    }
    poly_evaluate_one(s, a, prev)
}

/// polyEvaluate's UV questions, over each mesh named or selected.
fn uv_answer(s: &mut Session, a: &Args, flag: &str) -> CmdResult {
    let shell = a.get("uvsInShell").and_then(Value::as_i64).unwrap_or(-1);
    let targets = crate::uv_ops::targets(s, a, "polyEvaluate")?;
    let mut sum = 0i64;
    let mut list = Vec::new();
    let mut bounds: Option<[f64; 4]> = None;
    for (node, t) in targets {
        let Some(m) = crate::components::mesh_of(&mut s.graph, node) else { continue };
        let name = s.graph.path(crate::dag::transform_of(&s.graph, node));
        match crate::uv_ops::evaluate(&m, &t, flag, shell, &name) {
            Value::Number(n) => sum += n.as_i64().unwrap_or(0),
            Value::Array(v) if flag.starts_with("boundingBox") => {
                let n = |i: usize, k: usize| v[i][k].as_f64().unwrap_or(0.0);
                let b = bounds.get_or_insert([f64::MAX, f64::MIN, f64::MAX, f64::MIN]);
                *b = [b[0].min(n(0, 0)), b[1].max(n(0, 1)), b[2].min(n(1, 0)), b[3].max(n(1, 1))];
            }
            Value::Array(v) => list.extend(v),
            _ => {}
        }
    }
    Ok(if flag.starts_with("boundingBox") {
        let b = bounds.unwrap_or([0.0; 4]);
        json!([[b[0], b[1]], [b[2], b[3]]])
    } else if matches!(flag, "uvShell" | "uvComponent") {
        json!(sum)
    } else {
        Value::Array(list)
    })
}

fn poly_evaluate_one(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
    for flag in ["uvShell", "uvComponent", "uvShellIds", "activeUVShells"] {
        if a.bool(flag) {
            return uv_answer(s, a, flag);
        }
    }
    if a.has("uvsInShell") {
        return uv_answer(s, a, "uvsInShell");
    }
    if a.bool("activeShells") {
        let comps = crate::components::component_strings(&s.graph, true);
        let g = &mut s.graph;
        let mut ids = Vec::new();
        for (mesh, points) in crate::skin::points_named(g, &comps) {
            let Some(m) = crate::components::mesh_of(g, mesh) else { continue };
            let (of, _) = aztek_geo::multibox::shells_of(&m);
            ids.extend(points.iter().filter_map(|&p| of.get(p as usize).copied()));
        }
        ids.sort_unstable();
        ids.dedup();
        return Ok(json!(ids));
    }
    // Lengths and areas read in the current unit.
    let unit = if a.bool("boundingBox") || a.bool("boundingBoxComponent") {
        aztek_ops::units::Unit::Linear
    } else if a.bool("area") || a.bool("worldArea") {
        aztek_ops::units::Unit::Area
    } else {
        aztek_ops::units::Unit::None
    };
    let mut v = poly_evaluate_counts(s, a, prev)?;
    if aztek_ops::units::user_call() {
        aztek_ops::units::json_to_user(unit, &mut v);
    }
    Ok(v)
}

fn poly_evaluate_counts(s: &mut Session, a: &Args, prev: &aztek_ops::RunFn<Session>) -> CmdResult {
            let names = objects(s, a);
            if a.bool("area") || a.bool("worldArea") {
                return Ok(json!(mesh_area(&mut s.graph, &names, a.bool("worldArea"))));
            }
            if a.bool("boundingBoxComponent") {
                let comps = crate::components::component_strings(&s.graph, true);
                let g = &mut s.graph;
                g.evaluate();
                let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
                for (mesh, ids) in crate::skin::points_named(g, &comps) {
                    let Some(m) = crate::components::mesh_of(g, mesh) else { continue };
                    let w = crate::rig::world(g, crate::dag::transform_of(g, mesh));
                    for p in ids {
                        let q = w.transform_point3(m.position(p));
                        lo = lo.min(q);
                        hi = hi.max(q);
                    }
                }
                return Ok(json!([[lo.x, hi.x], [lo.y, hi.y], [lo.z, hi.z]]));
            }
            if a.bool("vertexComponent") || a.bool("edgeComponent") || a.bool("faceComponent") {
                let kind = if a.bool("vertexComponent") { ".vtx[" } else if a.bool("edgeComponent") { ".e[" } else { ".f[" };
                let comps = crate::components::component_strings(&s.graph, true);
                return Ok(json!(comps.iter().filter(|c| c.contains(kind)).count()));
            }
            if a.bool("format") {
                let r = prev(s, &Args { positional: a.positional.clone(), ..Default::default() })?;
                let o = r.as_object().cloned().unwrap_or_default();
                return Ok(json!(o.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>().join(" ")));
            }
            if a.bool("triangleComponent") {
                let mut call = a.clone();
                call.flags.remove("triangleComponent");
                call.flags.insert("triangle".into(), json!(true));
                return prev(s, &call);
            }
            if a.bool("boundingBox2d") || a.bool("boundingBoxComponent2d") {
                return uv_answer(s, a, if a.bool("boundingBox2d") { "boundingBox2d" } else { "boundingBoxComponent2d" });
            }
            prev(s, a)
}

fn sets_cmd(s: &mut Session, a: &Args) -> CmdResult {
    use crate::object_sets as os;
    let g = &mut s.graph;
    let items = a.named();
    let set_named = |g: &Graph, name: &str| -> CmdResult<NodeId> {
        let id = node_of(g, name, "sets")?;
        if !g.node(id).is_some_and(os::is_set) {
            fail!("sets: {name} is not a set");
        }
        Ok(id)
    };
    let parse = |g: &Graph, list: &[String]| os::parse(g, list).map_err(aztek_ops::CommandError::new);
    if a.bool("query") {
        let Some(first) = items.first() else { fail!("sets: name a set") };
        let id = set_named(g, first)?;
        let list = os::members(g, id);
        if asked(a, "size") {
            return Ok(json!(list.len()));
        }
        return Ok(json!(os::format(g, &list)));
    }
    for (flag, add) in [("addElement", true), ("forceElement", true), ("include", true), ("remove", false)] {
        let Some(name) = a.get(flag).and_then(Value::as_str) else { continue };
        let id = set_named(g, name)?;
        let list = if items.is_empty() { os::from_selection(g) } else { parse(g, &items)? };
        if add {
            os::add(g, id, &list)?;
        } else {
            os::remove(g, id, &list)?;
        }
        return Ok(Value::Null);
    }
    if let Some(name) = a.get("clear").and_then(Value::as_str) {
        let id = set_named(g, name)?;
        let list = os::members(g, id);
        os::remove(g, id, &list)?;
        return Ok(Value::Null);
    }
    if let Some(name) = a.get("isMember").and_then(Value::as_str) {
        let id = set_named(g, name)?;
        // By component, not by how the member happens to be written: vtx[1] is in vtx[0:3].
        let wanted = if items.is_empty() { os::from_selection(g) } else { parse(g, &items)? };
        let wanted = if crate::deform_sets::deformer_of_set(g, id).is_some() { crate::deform_sets::as_points(g, &wanted) } else { wanted };
        let held = os::members(g, id);
        let inside = wanted.iter().all(|w| held.iter().any(|h| h.node == w.node && (h.kind == crate::components::ComponentMode::Object || (h.kind == w.kind && w.ids.iter().all(|i| h.ids.contains(i))))));
        return Ok(json!(inside));
    }
    if a.has("union") || a.has("intersection") || a.has("subtract") || a.has("isIntersecting") {
        let key = ["union", "intersection", "subtract", "isIntersecting"].into_iter().find(|k| a.has(k)).unwrap();
        let first = a.string(key, "");
        let mut names = vec![first];
        names.extend(items.iter().cloned());
        let mut lists = Vec::new();
        for n in &names {
            let id = set_named(g, n)?;
            lists.push(os::format(g, &os::members(g, id)));
        }
        let mut acc = lists[0].clone();
        for l in &lists[1..] {
            match key {
                "union" => {
                    for x in l {
                        if !acc.contains(x) {
                            acc.push(x.clone());
                        }
                    }
                }
                "subtract" => acc.retain(|x| !l.contains(x)),
                _ => acc.retain(|x| l.contains(x)),
            }
        }
        if key == "isIntersecting" {
            return Ok(json!(!acc.is_empty()));
        }
        return Ok(json!(acc));
    }
    // Create.
    let list = if a.bool("empty") {
        Vec::new()
    } else if items.is_empty() {
        os::from_selection(g)
    } else {
        parse(g, &items)?
    };
    let id = os::create(g, &a.string("name", "set1"), &list, false)?;
    Ok(json!(g.path(id)))
}

fn set_shape_names(g: &mut Graph, id: NodeId) -> CmdResult<()> {
    let name = g.node(id).unwrap().name().to_string();
    let short = name.rsplit(':').next().unwrap_or(&name).to_string();
    let ns = name.strip_suffix(&short).unwrap_or("").to_string();
    for sh in crate::dag::shapes(g, id, true) {
        // A shape keeps a name it was given by hand.
        if g.node(sh).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) {
            continue;
        }
        g.rename(sh, &crate::dag::shape_name(&format!("{ns}{short}")))?;
    }
    Ok(())
}

/// ":a:b" and "a:b" relative to the current namespace, as a full name without the leading colon. "" is the root.
fn absolute_namespace(g: &Graph, name: &str) -> String {
    if let Some(rest) = name.strip_prefix(':') {
        return rest.to_string();
    }
    if g.current_namespace.is_empty() {
        name.to_string()
    } else {
        format!("{}:{name}", g.current_namespace)
    }
}

fn namespace_exists(g: &Graph, full: &str) -> bool {
    full.is_empty()
        || g.namespaces.contains(full)
        || g.nodes().iter().any(|&i| g.node(i).is_some_and(|n| n.namespace() == full || n.namespace().starts_with(&format!("{full}:"))))
}

fn nodes_in(g: &Graph, full: &str) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&i| g.node(i).is_some_and(|n| n.namespace() == full)).collect()
}

fn child_namespaces(g: &Graph, full: &str, recurse: bool) -> Vec<String> {
    let prefix = if full.is_empty() { String::new() } else { format!("{full}:") };
    let mut all: Vec<String> = g.namespaces.iter().cloned().collect();
    for i in g.nodes() {
        if let Some(n) = g.node(i) {
            if !n.namespace().is_empty() && !all.iter().any(|x| x == n.namespace()) {
                all.push(n.namespace().to_string());
            }
        }
    }
    all.retain(|x| x.starts_with(&prefix) && x.len() > prefix.len() && (recurse || !x[prefix.len()..].contains(':')));
    all.sort();
    all
}

/// Renames a namespace's nodes to live under another (`to` is "" for the root).
fn move_namespace_nodes(g: &mut Graph, from: &str, to: &str) -> CmdResult<()> {
    let prefix = format!("{from}:");
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if let Some(rest) = n.name().strip_prefix(&prefix) {
            let new = if to.is_empty() { rest.to_string() } else { format!("{to}:{rest}") };
            let saved = std::mem::take(&mut g.current_namespace);
            let r = g.rename(id, &new);
            g.current_namespace = saved;
            r?;
        }
    }
    Ok(())
}

fn namespace_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    if a.has("validateName") {
        let name = a.string("validateName", "");
        let clean: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' || c == ':' { c } else { '_' }).collect();
        return Ok(json!(clean));
    }
    if a.bool("query") {
        if asked(a, "relativeNames") {
            return Ok(json!(false));
        }
        if asked(a, "isRootNamespace") {
            let n = a.named().first().cloned().or_else(|| a.get("isRootNamespace").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
            return Ok(json!(n == ":" || n.is_empty()));
        }
        if asked(a, "exists") || a.pos_str(0).is_some() {
            let n = a.pos_str(0).unwrap_or_default().to_string();
            let full = absolute_namespace(g, &n);
            return Ok(json!(namespace_exists(g, &full)));
        }
        return Ok(json!(if g.current_namespace.is_empty() { ":".to_string() } else { format!(":{}", g.current_namespace) }));
    }
    if let Some(n) = a.get("exists").and_then(Value::as_str) {
        let full = absolute_namespace(g, n);
        return Ok(json!(namespace_exists(g, &full)));
    }
    if a.bool("exists") {
        let n = a.pos_str(0).unwrap_or_default().to_string();
        let full = absolute_namespace(g, &n);
        return Ok(json!(namespace_exists(g, &full)));
    }
    if a.has("addNamespace") {
        let name = a.string("addNamespace", "");
        if !Graph::valid_name(&name) {
            fail!("namespace: invalid name '{name}'");
        }
        let full = match a.get("parent").and_then(Value::as_str) {
            Some(":") => name.clone(),
            Some(p) => format!("{}:{name}", p.trim_start_matches(':')),
            None => absolute_namespace(g, &name),
        };
        if namespace_exists(g, &full) {
            fail!("namespace: namespace {full} already exists");
        }
        g.set_namespace_known(&full, true);
        return Ok(json!(name));
    }
    if a.has("setNamespace") {
        let name = a.string("setNamespace", "");
        let full = if name == ":" { String::new() } else if name == ".." {
            g.current_namespace.rsplit_once(':').map(|x| x.0.to_string()).unwrap_or_default()
        } else {
            absolute_namespace(g, &name)
        };
        if !namespace_exists(g, &full) {
            fail!("namespace: namespace {full} doesn't exist");
        }
        g.current_namespace = full.clone();
        return Ok(json!(if full.is_empty() { ":".to_string() } else { full }));
    }
    if a.has("removeNamespace") {
        let name = a.string("removeNamespace", "");
        let full = absolute_namespace(g, &name);
        if !namespace_exists(g, &full) {
            fail!("namespace: namespace {full} doesn't exist");
        }
        let mut order = child_namespaces(g, &full, true);
        order.reverse();
        order.push(full.clone());
        let content = a.bool("deleteNamespaceContent");
        if a.bool("mergeNamespaceWithRoot") || a.bool("mergeNamespaceWithParent") {
            let to = if a.bool("mergeNamespaceWithRoot") { String::new() } else { full.rsplit_once(':').map(|x| x.0.to_string()).unwrap_or_default() };
            move_namespace_nodes(g, &full, &to)?;
        } else if content {
            let doomed: Vec<NodeId> = g.nodes().into_iter().filter(|&i| g.node(i).is_some_and(|n| n.namespace() == full || n.namespace().starts_with(&format!("{full}:")))).collect();
            for d in doomed {
                if g.node(d).is_some() {
                    g.delete_node(d)?;
                }
            }
        } else if !nodes_in(g, &full).is_empty() || !child_namespaces(g, &full, false).is_empty() {
            fail!("namespace: namespace {full} is not empty");
        }
        for n in order {
            g.set_namespace_known(&n, false);
        }
        if g.current_namespace == full || g.current_namespace.starts_with(&format!("{full}:")) {
            g.current_namespace = String::new();
        }
        return Ok(Value::Null);
    }
    if let Some(Value::Array(pair)) = a.get("rename").or(a.get("moveNamespace")) {
        let (Some(from), Some(to)) = (pair.first().and_then(Value::as_str), pair.get(1).and_then(Value::as_str)) else { fail!("namespace: expected two names") };
        let (from, to) = (absolute_namespace(g, from), absolute_namespace(g, to));
        if a.has("rename") {
            for child in child_namespaces(g, &from, true) {
                let new = format!("{to}{}", &child[from.len()..]);
                move_namespace_nodes(g, &child, &new)?;
                g.set_namespace_known(&child, false);
                g.set_namespace_known(&new, true);
            }
            move_namespace_nodes(g, &from, &to)?;
            g.set_namespace_known(&from, false);
            g.set_namespace_known(&to, true);
        } else {
            move_namespace_nodes(g, &from, &to)?;
        }
        return Ok(Value::Null);
    }
    Ok(Value::Null)
}

fn namespace_info(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let target = match a.pos_str(0) {
        Some(":") => String::new(),
        Some(n) => absolute_namespace(g, n),
        None => g.current_namespace.clone(),
    };
    if a.bool("currentNamespace") {
        return Ok(json!(if target.is_empty() { ":".to_string() } else if a.bool("absoluteName") { format!(":{target}") } else { target }));
    }
    if a.bool("baseName") {
        return Ok(json!(target.rsplit(':').next().unwrap_or("")));
    }
    if a.bool("fullName") {
        return Ok(json!(target));
    }
    if a.bool("absoluteName") {
        return Ok(json!(format!(":{target}")));
    }
    if a.bool("parent") {
        return Ok(json!(target.rsplit_once(':').map(|x| x.0.to_string()).unwrap_or(":".to_string())));
    }
    if a.bool("isRootNamespace") {
        return Ok(json!(target.is_empty()));
    }
    let recurse = a.bool("recurse");
    let nodes = |g: &Graph| -> Vec<String> {
        let mut out: Vec<NodeId> = nodes_in(g, &target);
        if recurse {
            for c in child_namespaces(g, &target, true) {
                out.extend(nodes_in(g, &c));
            }
        }
        out.iter().map(|&i| if a.bool("dagPath") { g.full_path(i) } else { g.path(i) }).collect()
    };
    let spaces = child_namespaces(g, &target, recurse);
    if a.bool("listOnlyNamespaces") {
        return Ok(json!(spaces));
    }
    if a.bool("listOnlyDependencyNodes") {
        return Ok(json!(nodes(g)));
    }
    if a.bool("listNamespace") {
        let mut out = spaces;
        out.extend(nodes(g));
        return Ok(json!(out));
    }
    Ok(json!(if target.is_empty() { ":".to_string() } else { target }))
}

/// `name value` for optionVar: MEL leaves the value positional, Python gives a pair.
fn option_pair(a: &Args, flag: &str) -> Option<(String, Value)> {
    match a.get(flag)? {
        Value::Array(v) if v.len() == 2 => Some((v[0].as_str()?.to_string(), v[1].clone())),
        Value::String(n) => Some((n.clone(), a.positional.first().cloned().unwrap_or(Value::Null))),
        _ => None,
    }
}

fn option_var(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let name = a.pos_str(0).unwrap_or_default();
        return Ok(s.option_vars.get(name).cloned().unwrap_or(json!(0)));
    }
    if let Some(n) = a.get("exists").and_then(Value::as_str) {
        return Ok(json!(s.option_vars.contains_key(n)));
    }
    if let Some(n) = a.get("remove").and_then(Value::as_str) {
        s.option_vars.remove(n);
        return Ok(Value::Null);
    }
    if let Some(n) = a.get("clearArray").and_then(Value::as_str) {
        s.option_vars.insert(n.to_string(), json!([]));
        return Ok(Value::Null);
    }
    if a.bool("list") {
        return Ok(json!(s.option_vars.keys().collect::<Vec<_>>()));
    }
    for (flag, kind) in [("intValue", 0), ("floatValue", 1), ("stringValue", 2)] {
        if let Some((n, v)) = option_pair(a, flag) {
            let v = match kind {
                0 => json!(v.as_f64().unwrap_or(0.0) as i64),
                1 => json!(v.as_f64().unwrap_or(0.0)),
                _ => v,
            };
            s.option_vars.insert(n, v);
        }
    }
    for flag in ["intValueAppend", "floatValueAppend", "stringValueAppend"] {
        if let Some((n, v)) = option_pair(a, flag) {
            let e = s.option_vars.entry(n).or_insert(json!([]));
            if !e.is_array() {
                *e = json!([e.clone()]);
            }
            e.as_array_mut().unwrap().push(v);
        }
    }
    Ok(Value::Null)
}

fn about(_s: &mut Session, a: &Args) -> CmdResult {
    let (win, linux, mac) = (cfg!(windows), cfg!(target_os = "linux"), cfg!(target_os = "macos"));
    Ok(if a.bool("version") {
        json!(aztek_core::REFERENCE_VERSION)
    } else if a.bool("apiVersion") {
        json!(20250000)
    } else if a.bool("operatingSystem") {
        json!(if win { "win64" } else if mac { "mac" } else { "linux64" })
    } else if a.bool("windows") || a.bool("nt") {
        json!(win)
    } else if a.bool("linux") {
        json!(linux)
    } else if a.bool("macOS") {
        json!(mac)
    } else if a.bool("batch") {
        json!(aztek_core::batch_mode())
    } else if a.bool("is64") {
        json!(true)
    } else if a.bool("uiLanguage") || a.bool("codeset") {
        json!("en_US")
    } else if a.bool("product") || a.bool("application") {
        json!("Aztek")
    } else {
        fail!("about: nothing asked")
    })
}

fn bounds_of(g: &mut Graph, names: &[String], ignore_invisible: bool) -> CmdResult<Option<(DVec3, DVec3)>> {
    g.evaluate();
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for name in names {
        let id = node_of(g, name, "exactWorldBoundingBox")?;
        let mut under = g.descendants(id);
        under.push(id);
        for u in under {
            if !crate::dag::is_shape(g, u) || (ignore_invisible && !crate::transform::effectively_visible(g, u)) || g.node(u).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) {
                continue;
            }
            let b = crate::node_bounds::world_bounds(g, u);
            if b.valid() {
                lo = lo.min(b.min);
                hi = hi.max(b.max);
            }
        }
    }
    Ok((lo.x <= hi.x).then_some((lo, hi)))
}

/// The values polyEvaluate's area flags ask for: object space, or world.
fn mesh_area(g: &mut Graph, names: &[String], world: bool) -> f64 {
    let mut total = 0.0;
    for n in names {
        let Ok(Some(id)) = g.find(n) else { continue };
        let Some(mesh) = crate::components::as_mesh(g, id) else { continue };
        let Some(m) = crate::components::mesh_of(g, mesh) else { continue };
        let w = crate::rig::world(g, crate::dag::transform_of(g, mesh));
        for face in 0..m.face_capacity() {
            if !m.face_valid(face) {
                continue;
            }
            let pts: Vec<DVec3> = m.face_points(face).into_iter().map(|p| if world { w.transform_point3(m.position(p)) } else { m.position(p) }).collect();
            for k in 1..pts.len().saturating_sub(1) {
                total += (pts[k] - pts[0]).cross(pts[k + 1] - pts[0]).length() * 0.5;
            }
        }
    }
    total
}

fn toggle_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let attr = [("localAxis", Transform::DISPLAY_LOCAL_AXIS), ("rotatePivot", Transform::DISPLAY_ROTATE_PIVOT), ("scalePivot", Transform::DISPLAY_SCALE_PIVOT), ("selectHandle", Transform::DISPLAY_HANDLE), ("template", DagNode::TEMPLATE)];
    let chosen = attr.iter().find(|(n, _)| a.bool(n) || asked(a, n));
    let Some(&(_, index)) = chosen else { fail!("toggle: say what: -la, -rp, -sp, -sh, -te") };
    let names = objects(s, a);
    let mut last = false;
    for n in &names {
        let id = node_of(&s.graph, n, "toggle")?;
        if !crate::dag::is_transform(&s.graph, id) && index != DagNode::TEMPLATE {
            continue;
        }
        let g = &mut s.graph;
        let now = g.node(id).unwrap().bool(index);
        if a.bool("query") {
            return Ok(json!(now));
        }
        let next = if a.has("state") { a.bool("state") } else { !now };
        g.set_value(PlugRef::new(id, index as i32), PlugValue::Bool(next))?;
        last = next;
    }
    Ok(json!(last))
}

fn align_cmd(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    if names.len() < 2 {
        fail!("align: select at least two objects");
    }
    let g = &mut s.graph;
    let mut boxes = Vec::new();
    for n in &names {
        let Some(b) = bounds_of(g, std::slice::from_ref(n), false)? else { fail!("align: {n} has no bounds") };
        boxes.push(b);
    }
    for (axis, flag) in ["alignX", "alignY", "alignZ"].iter().enumerate() {
        let Some(mode) = a.get(flag).and_then(Value::as_str) else { continue };
        let at = |b: &(DVec3, DVec3)| match mode {
            "Min" | "min" => b.0[axis],
            "Max" | "max" => b.1[axis],
            _ => (b.0[axis] + b.1[axis]) * 0.5,
        };
        let target = if a.bool("alignToLead") {
            at(&boxes[names.len() - 1])
        } else {
            match mode {
                "Min" | "min" => boxes.iter().map(|b| b.0[axis]).fold(f64::MAX, f64::min),
                "Max" | "max" => boxes.iter().map(|b| b.1[axis]).fold(f64::MIN, f64::max),
                _ => {
                    let lo = boxes.iter().map(|b| b.0[axis]).fold(f64::MAX, f64::min);
                    let hi = boxes.iter().map(|b| b.1[axis]).fold(f64::MIN, f64::max);
                    (lo + hi) * 0.5
                }
            }
        };
        for (n, b) in names.iter().zip(boxes.iter()) {
            let id = node_of(g, n, "align")?;
            let mut delta = DVec3::ZERO;
            delta[axis] = target - at(b);
            crate::transform_ops::move_by(g, &[id], delta, crate::transform_ops::Space::World)?;
        }
    }
    Ok(json!(true))
}

fn search_replace(s: &mut Session, a: &Args) -> CmdResult {
    let (Some(from), Some(to)) = (a.pos_str(0).map(str::to_string), a.pos_str(1).map(str::to_string)) else { fail!("searchReplaceNames: expected (search, replace, scope)") };
    let scope = a.pos_str(2).unwrap_or("all").to_string();
    let g = &mut s.graph;
    let mut ids: Vec<NodeId> = match scope.as_str() {
        "selected" => g.selection().to_vec(),
        "hierarchy" => {
            let mut out = Vec::new();
            for &id in g.selection() {
                out.push(id);
                out.extend(g.descendants(id));
            }
            out
        }
        _ => g.nodes(),
    };
    // Children first: a renamed parent changes nothing for them, but paths stay valid this way round.
    ids.reverse();
    let mut count = 0;
    for id in ids {
        let name = g.node(id).unwrap().name().to_string();
        if name.contains(&from) {
            g.rename(id, &name.replace(&from, &to))?;
            count += 1;
        }
    }
    Ok(json!(count))
}

pub fn register(c: &mut Commands<Session>) {
    extend(
        c,
        "listRelatives",
        vec![
            f("children", "c", B, "Children (the default)."),
            f("shapes", "s", B, "Only shapes."),
            f("allParents", "ap", B, "Every parent."),
            f("fullPath", "f", B, "Full paths."),
            f("path", "pa", B, "Shortest unique paths."),
            f("noIntermediate", "ni", B, "Leave out intermediate objects."),
            f("type", "typ", A, "Only this type or these."),
        ],
        |s, a, _| list_relatives(s, a),
    );
    extend(
        c,
        "listConnections",
        vec![
            f("exactType", "et", A, "With -type: exactly this type, not derived ones."),
            f("shapes", "sh", B, "A connected shape is named, not its transform."),
            f("skipConversionNodes", "scn", B, "Report what is beyond unitConversion nodes."),
            f("type", "t", A, "Only nodes of this type."),
        ],
        |s, a, _| list_connections(s, a),
    );
    extend(c, "objExists", vec![], |s, a, prev| {
        if let Some(n) = a.pos_str(0) {
            if n.contains('.') {
                return Ok(json!(s.graph.plug(n).is_ok()));
            }
        }
        prev(s, a)
    });
    extend(
        c,
        "nodeType",
        vec![
            f("inherited", "i", B, "Its types, base first."),
            f("apiType", "api", B, "The API type name."),
            f("derived", "d", B, "Types derived from it."),
            f("isTypeName", "itn", B, "The argument is a type, not a node."),
        ],
        |s, a, prev| {
            let name = a.pos_str(0).map(str::to_string).or_else(|| objects(s, a).first().cloned());
            let Some(name) = name else { fail!("nodeType: no object") };
            let ty = if a.bool("isTypeName") {
                let Some(t) = types::find(&name) else { fail!("nodeType: unknown type {name}") };
                t
            } else {
                let id = node_of(&s.graph, &name, "nodeType")?;
                s.graph.node(id).unwrap().type_arc().clone()
            };
            if a.bool("inherited") {
                let mut chain = Vec::new();
                let mut t = Some(ty.as_ref());
                while let Some(x) = t {
                    chain.push(x.name.clone());
                    t = x.base.as_deref();
                }
                chain.reverse();
                return Ok(json!(chain));
            }
            if a.bool("derived") {
                let out: Vec<String> = types::all().iter().filter(|t| t.is_a(&ty.name)).map(|t| t.name.clone()).collect();
                return Ok(json!(out));
            }
            if a.bool("apiType") {
                let mut cs = ty.name.chars();
                let cap: String = cs.next().map(|c| c.to_uppercase().collect::<String>() + cs.as_str()).unwrap_or_default();
                return Ok(json!(format!("k{cap}")));
            }
            if a.bool("isTypeName") {
                return Ok(json!(ty.name));
            }
            prev(s, a)
        },
    );
    extend(
        c,
        "objectType",
        vec![f("isAType", "isa", S, "Whether it is this type or derived from it."), f("isType", "i", S, "Whether it is exactly this type.")],
        |s, a, prev| {
            if let Some(t) = a.get("isAType").and_then(Value::as_str) {
                let Some(name) = a.pos_str(0) else { fail!("objectType: name an object") };
                let id = node_of(&s.graph, name, "objectType")?;
                return Ok(json!(s.graph.node(id).unwrap().ty().is_a(t)));
            }
            prev(s, a)
        },
    );
    extend(
        c,
        "rename",
        vec![f("ignoreShape", "is", B, "Leave the shape's name alone."), f("uuid", "uid", B, "The first argument is a uuid.")],
        |s, a, prev| {
            let mut call = a.clone();
            if a.bool("uuid") {
                let Some(u) = a.pos_str(0) else { fail!("rename: name a uuid") };
                let found = s.graph.nodes().into_iter().find(|&i| s.graph.node(i).is_some_and(|n| n.uuid() == u));
                let Some(id) = found else { fail!("rename: no node with uuid {u}") };
                call.positional[0] = json!(s.graph.path(id));
            }
            let r = prev(s, &call)?;
            if !a.bool("ignoreShape") {
                if let Some(id) = r.as_str().and_then(|n| s.graph.find(n).ok().flatten()) {
                    if crate::dag::is_transform(&s.graph, id) {
                        let before: Vec<String> = crate::dag::shapes(&s.graph, id, false).iter().map(|&x| s.graph.path(x)).collect();
                        if !before.is_empty() {
                            set_shape_names(&mut s.graph, id)?;
                        }
                    }
                }
            }
            Ok(json!(r.as_str().and_then(|n| s.graph.find(n).ok().flatten()).map(|i| s.graph.path(i)).unwrap_or_default()))
        },
    );
    extend(c, "createNode", vec![f("shared", "s", B, "Reuse a node of that name when there is one.")], |s, a, prev| {
        if a.bool("shared") && a.has("name") {
            if let Some(id) = s.graph.find(&a.string("name", ""))? {
                return Ok(json!(s.graph.path(id)));
            }
        }
        let r = prev(s, a)?;
        if let Some(id) = r.as_str().and_then(|n| s.graph.find(n).ok().flatten()) {
            crate::utility_nodes::seed(&mut s.graph, id)?;
        }
        Ok(r)
    });
    c.register(
        Cmd::new("shadingNode", "Create a rendering or utility node: shadingNode -asUtility multiplyDivide -n name.", |s, a| {
            let Some(ty) = a.pos_str(0) else { fail!("shadingNode: name a node type") };
            let Some(t) = types::find(ty) else { fail!("shadingNode: unknown type {ty}") };
            if !(a.bool("asRendering") || a.bool("asUtility") || a.bool("asShader") || a.bool("asTexture") || a.bool("asLight") || a.bool("asPostProcess") || a.bool("asRendering")) {
                fail!("shadingNode: give -asUtility, -asShader, -asTexture or -asLight");
            }
            if t.dag && !t.is_a("transform") {
                fail!("shadingNode: {ty} is not a shading node");
            }
            if a.bool("shared") && a.has("name") {
                if let Some(id) = s.graph.find(&a.string("name", ""))? {
                    return Ok(json!(s.graph.path(id)));
                }
            }
            let id = s.graph.create_node(&t, &a.string("name", ""), NO_NODE, true)?;
            crate::utility_nodes::seed(&mut s.graph, id)?;
            Ok(json!(s.graph.path(id)))
        })
        .positional("type")
        .flag(f("isColorManaged", "icm", B, "The node's colours are colour managed. Accepted: textures read their colour space here."))
        .flag(f("asRendering", "ar", B, "A rendering node."))
        .flag(f("asUtility", "au", B, "A utility."))
        .flag(f("asShader", "as", B, "A shader."))
        .flag(f("asTexture", "at", B, "A texture."))
        .flag(f("asLight", "al", B, "A light."))
        .flag(f("asPostProcess", "app", B, "A post process."))
        .flag(f("asRendering", "ar", B, "A rendering node."))
        .flag(f("name", "n", S, "Its name."))
        .flag(f("shared", "s", B, "Reuse a node of that name."))
        .flag(f("skipSelect", "ss", B, "Accepted: shading nodes don't change the selection.")),
    );
    c.register(
        Cmd::new("getClassification", "A node type's classification strings: utility/general, and so on.", |_s, a| {
            let Some(ty) = a.pos_str(0) else { fail!("getClassification: name a node type") };
            let classes: Vec<String> = crate::utility_nodes::classification(ty).map(str::to_string).into_iter().collect();
            if let Some(want) = a.get("satisfies").and_then(Value::as_str) {
                return Ok(json!(classes.iter().any(|x| x == want || x.starts_with(&format!("{want}/")) || x.starts_with(&format!("{want}:")))));
            }
            Ok(json!(classes))
        })
        .positional("type")
        .flag(f("satisfies", "sat", S, "Whether the type's classification satisfies this."))
        .undoable(false),
    );
    {
        let mut v = Vec::new();
        for (name, short, ty, doc) in [
            ("channels", "c", B, "Animation curves and expressions on the channels."), ("expressions", "e", B, "Expressions on the channels."), ("constraints", "cn", B, "Constraints on the object."),
            ("hierarchy", "hi", S, "below or both: the objects under it too."), ("shape", "s", B, "Only the shapes."), ("staticChannels", "sc", B, "Curves whose keys all hold one value."),
            ("timeAnimationCurves", "tac", B, "Curves over time."), ("unitlessAnimationCurves", "uac", B, "Driven key curves."), ("attribute", "at", A, "Only on these attributes."), ("inputConnectionsAndNodes", "icn", B, "Everything feeding the channels."),
            ("controlPoints", "cp", B, "Accepted for existing scripts."), ("motionPaths", "mp", B, "Accepted for existing scripts."),
        ] {
            v.push(f(name, short, ty, doc));
        }
        extend(c, "delete", v, delete_cmd);
    }
    extend(
        c,
        "parent",
        vec![
            f("absolute", "a", B, "Keep world placement (the default)."),
            f("addObject", "add", B, "Add as an instance under the parent."),
            f("removeObject", "rm", B, "Remove this instance."),
            f("shape", "s", B, "Move the shapes, not the transforms."),
            f("noConnections", "nc", B, "Accepted for existing scripts."),
            f("noInvScale", "nis", B, "Accepted for existing scripts."),
        ],
        parent_cmd,
    );
    extend(
        c,
        "group",
        vec![f("world", "w", B, "Under the world, not the members' shared parent."), f("absolute", "a", B, "Keep world placement (the default)."), f("relative", "r", B, "Keep local placement."), f("useAsGroup", "uag", S, "Accepted for existing scripts.")],
        group_cmd,
    );
    extend(
        c,
        "ungroup",
        vec![f("world", "w", B, "Move the children to the world."), f("parent", "p", S, "Move the children here."), f("absolute", "a", B, "Keep world placement (the default)."), f("relative", "r", B, "Keep local placement.")],
        |s, a, prev| {
            if !a.bool("world") && !a.has("parent") && !a.bool("relative") {
                return prev(s, a);
            }
            let up = if a.has("parent") { node_of(&s.graph, &a.string("parent", ""), "ungroup")? } else { NO_NODE };
            let keep = !a.bool("relative");
            let mut out = Vec::new();
            for name in objects(s, a) {
                let id = node_of(&s.graph, &name, "ungroup")?;
                let to = if a.bool("world") || a.has("parent") { up } else { s.graph.parent(id) };
                for k in s.graph.children(id).to_vec() {
                    s.graph.set_parent(k, to, keep, usize::MAX)?;
                    out.push(json!(s.graph.path(k)));
                }
                s.graph.delete_node(id)?;
            }
            Ok(Value::Array(out))
        },
    );
    {
        let mut v = Vec::new();
        for (name, short, doc) in [
            ("returnRootsOnly", "rr", "Only the top nodes in the result."), ("upstreamNodes", "un", "Copy the nodes feeding it too."), ("parentOnly", "po", "Only the node itself, no children."),
            ("inputConnections", "ic", "The copy takes the same inputs."), ("renameChildren", "rc", "Give the children new unique names."), ("smartTransform", "st", "Repeat the last duplicate's step."),
            ("transformsOnly", "to", "Leave out the shapes."), ("instanceLeaf", "ilf", "Accepted for existing scripts."),
        ] {
            v.push(f(name, short, B, doc));
        }
        extend(c, "duplicate", v, duplicate_cmd);
    }
    extend(c, "instance", vec![f("leaf", "lf", B, "Accepted for existing scripts."), f("smartTransform", "st", B, "Accepted for existing scripts.")], |s, a, prev| prev(s, a));
    extend(
        c,
        "select",
        vec![
            f("replace", "r", B, "Replace the selection (the default)."),
            f("noExpand", "ne", B, "A set itself, not its members."),
            f("hierarchy", "hi", B, "Everything below too."),
            f("addFirst", "af", B, "Add at the front of the list."),
            f("allDagObjects", "ado", B, "Every top level object."),
            f("allDependencyNodes", "adn", B, "Every node."),
            f("visible", "vis", B, "Only visible objects."),
            f("all", "all", B, "Every object."),
            f("symmetry", "sym", B, "Accepted for existing scripts."),
            f("symmetrySide", "sys", ArgType::Int, "Accepted for existing scripts."),
        ],
        select_cmd,
    );
    c.register(
        Cmd::new("selected", "The selection: objects, or components in a component mode.", |s, _| run(s, "ls", json!({"selection": true}))).undoable(false),
    );
    extend(c, "pickWalk", vec![f("direction", "d", S, "up, down, left or right."), f("type", "typ", S, "Accepted for existing scripts.")], |s, a, prev| {
        prev(s, a)?;
        run(s, "ls", json!({"selection": true}))
    });
    {
        let mut sets = Cmd::new("sets", "The reference's object set command: create, add to, take from, list and combine sets.", sets_cmd).positional("items, or a set with -q");
        sets = sets.flag(f("query", "q", B, "List a set's members.")).flag(f("edit", "e", B, "Edit.")).flag(f("queried", "", ArgType::List, "What -q asks for."));
        for (name, short, ty, doc) in [
            ("name", "n", S, "The new set's name."), ("empty", "em", B, "Hold nothing."), ("addElement", "add", S, "Add the items to this set."), ("forceElement", "fe", S, "Add the items to this set."),
            ("include", "in", S, "Add the items to this set."), ("remove", "rm", S, "Take the items out of this set."), ("clear", "cl", S, "Empty this set."), ("isMember", "im", S, "Whether the items are in this set."),
            ("isIntersecting", "ii", S, "Whether this set and the others share members."), ("union", "un", S, "This set's members and the others'."), ("intersection", "int", S, "What this set shares with the others."), ("subtract", "sub", S, "This set less the others."),
            ("size", "s", B, "With -q: how many."), ("flatten", "fl", B, "Accepted for existing scripts."), ("noWarnings", "nw", B, "Accepted for existing scripts."), ("renderable", "r", B, "Accepted for existing scripts."), ("vertices", "v", B, "Accepted for existing scripts."), ("edges", "ed", B, "Accepted for existing scripts."), ("facets", "fc", B, "Accepted for existing scripts."), ("text", "t", S, "Accepted for existing scripts."),
        ] {
            sets = sets.flag(f(name, short, ty, doc));
        }
        c.register(sets);
    }
    extend(
        c,
        "undoInfo",
        vec![
            f("query", "q", B, "Query."),
            f("queried", "", ArgType::List, "What -q asks for."),
            f("state", "st", B, "Turn undo on or off; off flushes the queue, as in the reference."),
            f("stateWithoutFlush", "swf", B, "Turn undo on or off, keeping the queue."),
            f("undoName", "un", S, "With -q: what undo would undo."),
            f("redoName", "rn", S, "With -q: what redo would redo."),
            f("undoQueueEmpty", "uqe", B, "With -q: whether there is nothing to undo."),
            f("redoQueueEmpty", "rqe", B, "With -q: whether there is nothing to redo."),
            f("length", "l", ArgType::Int, "How many steps to keep."),
            f("infinity", "infinity", B, "Keep every step."),
            f("printQueue", "pq", B, "Accepted for existing scripts."),
        ],
        |s, a, prev| {
            if a.bool("query") {
                let u = s.graph.undo_stack();
                let has = |x: &str| asked(a, x);
                return Ok(if has("undoName") {
                    json!(u.undo_label())
                } else if has("redoName") {
                    json!(u.redo_label())
                } else if has("undoQueueEmpty") {
                    json!(!u.can_undo())
                } else if has("redoQueueEmpty") {
                    json!(u.redo_count() == 0)
                } else if has("length") {
                    json!(u.undo_count())
                } else if has("infinity") {
                    json!(true)
                } else {
                    json!(u.enabled())
                });
            }
            if a.has("stateWithoutFlush") {
                s.graph.undo_stack_mut().set_enabled(a.bool("stateWithoutFlush"));
            }
            if a.has("length") {
                s.graph.undo_stack_mut().set_limit(a.int("length", 0).max(0) as usize);
            }
            if a.bool("infinity") {
                s.graph.undo_stack_mut().set_limit(0);
            }
            let mut call = a.clone();
            call.flags.remove("stateWithoutFlush");
            prev(s, &call)
        },
    );
    {
        let mut ab = Cmd::new("about", "About the application and the system: -v, -os, -win, -linux, -mac, -batch, -api.", about).undoable(false);
        for (name, short) in [("version", "v"), ("apiVersion", "api"), ("operatingSystem", "os"), ("windows", "win"), ("nt", "nt"), ("linux", "li"), ("macOS", "mac"), ("batch", "b"), ("is64", "x64"), ("uiLanguage", "uil"), ("codeset", "cs"), ("product", "p"), ("application", "a")] {
            ab = ab.flag(f(name, short, B, "What to ask."));
        }
        c.register(ab);
    }
    {
        let mut ov = Cmd::new("optionVar", "Named values that last the session: -iv, -fv, -sv, -q, -ex, -rm.", option_var).positional("a name with -q; a value in MEL").undoable(false);
        ov = ov.flag(f("query", "q", B, "The value of the name given.")).flag(f("exists", "ex", S, "Whether this name is set.")).flag(f("remove", "rm", S, "Unset this name.")).flag(f("clearArray", "ca", S, "Empty this array.")).flag(f("list", "l", B, "Every name."));
        for (name, short) in [("intValue", "iv"), ("floatValue", "fv"), ("stringValue", "sv"), ("intValueAppend", "iva"), ("floatValueAppend", "fva"), ("stringValueAppend", "sva")] {
            ov = ov.flag(f(name, short, A, "Name and value."));
        }
        c.register(ov);
    }
    c.register(
        Cmd::new("getenv", "An environment variable's value, or \"\".", |_s, a| {
            let Some(n) = a.pos_str(0) else { fail!("getenv: name a variable") };
            Ok(json!(std::env::var(n).unwrap_or_default()))
        })
        .positional("name")
        .undoable(false),
    );
    c.register(
        Cmd::new("putenv", "Sets an environment variable for this process.", |_s, a| {
            let (Some(n), Some(v)) = (a.pos_str(0), a.pos_str(1)) else { fail!("putenv: expected (name, value)") };
            // Commands run on the main thread, before anything else reads the environment concurrently.
            unsafe { std::env::set_var(n, v) };
            Ok(Value::Null)
        })
        .positional("name value")
        .undoable(false),
    );
    c.register(
        Cmd::new("env", "Every environment variable, as NAME=value lines.", |_s, _a| Ok(json!(std::env::vars().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>()))).undoable(false),
    );
    c.register(
        Cmd::new("workspace", "The project: -q -rd or -fn gives its folder, -o and -dir change to one.", |s, a| {
            if a.has("openWorkspace") || a.has("directory") {
                let d = a.string(if a.has("openWorkspace") { "openWorkspace" } else { "directory" }, "");
                s.option_vars.insert("__workspace".into(), json!(d));
                return Ok(json!(d));
            }
            let root = s.option_vars.get("__workspace").and_then(Value::as_str).map(str::to_string).or_else(|| aztek_core::env::get("MAYA_PROJECT")).unwrap_or_else(|| std::env::current_dir().map(|d| d.display().to_string()).unwrap_or_default());
            let with_slash = if root.ends_with('/') { root.clone() } else { format!("{root}/") };
            Ok(if asked(a, "rootDirectory") || asked(a, "directory") { json!(with_slash) } else { json!(root.trim_end_matches('/')) })
        })
        .positional("[path]")
        .flag(f("query", "q", B, "Query."))
        .flag(f("queried", "", ArgType::List, "What -q asks for."))
        .flag(f("rootDirectory", "rd", S, "The project folder, with a slash."))
        .flag(f("fullName", "fn", S, "The project folder."))
        .flag(f("directory", "dir", S, "Change to this folder."))
        .flag(f("openWorkspace", "o", S, "Open this project."))
        .undoable(false),
    );
    {
        let mut tg = Cmd::new("toggle", "Turns display switches on the objects on and off: -la, -rp, -sp, -sh, -te; -st for a value, -q to ask.", toggle_cmd).positional("objects");
        for (name, short) in [("localAxis", "la"), ("rotatePivot", "rp"), ("scalePivot", "sp"), ("selectHandle", "sh"), ("template", "te")] {
            tg = tg.flag(f(name, short, B, "What to switch."));
        }
        c.register(tg.flag(f("state", "st", B, "On or off, rather than the other way round.")).flag(f("query", "q", B, "Query.")).flag(f("queried", "", ArgType::List, "What -q asks for.")));
    }
    extend(
        c,
        "hide",
        vec![f("clearSelection", "cs", B, "Accepted for existing scripts."), f("returnHidden", "rh", B, "Return what was hidden."), f("invertComponents", "ic", B, "Accepted for existing scripts."), f("testVisibility", "tv", B, "Whether they are hidden, changing nothing.")],
        |s, a, prev| {
            if a.bool("testVisibility") {
                let names = objects(s, a);
                return Ok(json!(names.iter().filter_map(|n| s.graph.find(n).ok().flatten()).all(|i| !crate::transform::effectively_visible(&s.graph, i))));
            }
            let names = objects(s, a);
            let r = prev(s, a)?;
            Ok(if a.bool("returnHidden") { json!(names) } else { r })
        },
    );
    c.register(
        Cmd::new("showHidden", "Show objects: named, -a every one, -b those below the named.", |s, a| {
            let g = &mut s.graph;
            let mut ids = Vec::new();
            if a.bool("all") {
                ids = g.nodes().into_iter().filter(|&i| g.node(i).is_some_and(|n| n.ty().dag)).collect();
            }
            for n in a.named() {
                let id = node_of(g, &n, "showHidden")?;
                if !a.bool("below") {
                    ids.push(id);
                }
                if a.bool("below") {
                    ids.extend(g.descendants(id));
                }
                if a.bool("above") {
                    let mut p = g.parent(id);
                    while p != NO_NODE {
                        ids.push(p);
                        p = g.parent(p);
                    }
                }
            }
            for id in ids {
                if !g.node(id).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) {
                    g.set_value(PlugRef::new(id, DagNode::VISIBILITY as i32), PlugValue::Bool(true))?;
                }
            }
            Ok(Value::Null)
        })
        .positional("objects")
        .flag(f("all", "a", B, "Every hidden object."))
        .flag(f("below", "b", B, "The objects under those named."))
        .flag(f("above", "ab", B, "The objects above those named.")),
    );
    c.register(
        Cmd::new("displayCull", "Back face culling per object, kept for scripts that ask: -bfc on or off, -q.", |s, a| {
            let names = objects(s, a);
            let key = |n: &str| format!("__cull:{n}");
            if a.bool("query") {
                let Some(n) = names.first() else { fail!("displayCull: name an object") };
                return Ok(s.option_vars.get(&key(n)).cloned().unwrap_or(json!(false)));
            }
            if a.has("backFaceCulling") {
                for n in &names {
                    s.option_vars.insert(key(n), json!(a.bool("backFaceCulling")));
                }
            }
            Ok(Value::Null)
        })
        .positional("objects")
        .flag(f("backFaceCulling", "bfc", B, "On or off."))
        .flag(f("query", "q", B, "Query."))
        .flag(f("queried", "", ArgType::List, "What -q asks for."))
        .undoable(false),
    );
    c.register(
        Cmd::new("refresh", "Brings the scene up to date; the viewports draw on their own.", |s, _| {
            s.graph.evaluate();
            Ok(Value::Null)
        })
        .flag(f("force", "f", B, "Accepted for existing scripts."))
        .flag(f("suspend", "su", B, "Accepted for existing scripts."))
        .flag(f("currentView", "cv", B, "Accepted for existing scripts."))
        .undoable(false),
    );
    c.register(
        Cmd::new("dgdirty", "Marks nodes for recompute: named, or -a every one.", |s, a| {
            let ids: Vec<NodeId> = if a.bool("allPlugs") {
                s.graph.nodes()
            } else {
                let mut v = Vec::new();
                for n in objects(s, a) {
                    v.push(if n.contains('.') { s.graph.plug(&n)?.node } else { node_of(&s.graph, &n, "dgdirty")? });
                }
                v
            };
            for id in ids {
                s.graph.touch(id);
            }
            Ok(Value::Null)
        })
        .positional("objects or plugs")
        .flag(f("allPlugs", "a", B, "Every node."))
        .flag(f("clean", "c", B, "Accepted for existing scripts."))
        .flag(f("verbose", "v", B, "Accepted for existing scripts."))
        .undoable(false),
    );
    c.register(
        Cmd::new("dgeval", "Brings nodes or plugs up to date.", |s, a| {
            for n in a.named() {
                if n.contains('.') {
                    let p = s.graph.plug(&n)?;
                    s.graph.value(&p)?;
                }
            }
            s.graph.evaluate();
            Ok(Value::Null)
        })
        .positional("objects or plugs")
        .flag(f("verbose", "v", B, "Accepted for existing scripts."))
        .undoable(false),
    );
    c.register(
        Cmd::new("reorder", "Moves objects among their siblings: -f front, -b back, -r n places.", |s, a| {
            let g = &mut s.graph;
            for n in objects_of(a, g) {
                let id = node_of(g, &n, "reorder")?;
                let p = g.parent(id);
                let sibs = if p == NO_NODE { g.roots().to_vec() } else { g.children(p).to_vec() };
                let at = sibs.iter().position(|&x| x == id).unwrap_or(0) as i64;
                let to = if a.bool("front") {
                    0
                } else if a.bool("back") {
                    sibs.len() as i64 - 1
                } else {
                    (at + a.int("relative", 0)).clamp(0, sibs.len() as i64 - 1)
                };
                g.set_parent(id, p, false, to as usize)?;
            }
            Ok(Value::Null)
        })
        .positional("objects")
        .flag(f("front", "f", B, "First among its siblings."))
        .flag(f("back", "b", B, "Last among its siblings."))
        .flag(f("relative", "r", ArgType::Int, "This many places along.")),
    );
    extend(
        c,
        "selectMode",
        vec![
            f("query", "q", B, "Query."),
            f("queried", "", ArgType::List, "What -q asks for."),
            f("object", "o", B, "Object mode."),
            f("component", "co", B, "Component mode."),
            f("hierarchical", "h", B, "Hierarchy mode: objects here."),
            f("preset", "p", B, "Accepted for existing scripts."),
            f("root", "r", B, "Accepted for existing scripts."),
            f("leaf", "l", B, "Accepted for existing scripts."),
            f("template", "t", B, "Accepted for existing scripts."),
        ],
        |s, a, prev| {
            let plain = Args::default();
            if a.bool("query") {
                let now = prev(s, &plain)?;
                let object = now == "object";
                return Ok(if asked(a, "component") { json!(!object) } else if asked(a, "object") || asked(a, "hierarchical") { json!(object) } else { now });
            }
            if a.bool("object") || a.bool("hierarchical") {
                let call = Args { positional: vec![json!("object")], ..Default::default() };
                return prev(s, &call);
            }
            if a.bool("component") {
                let now = prev(s, &plain)?;
                if now == "object" {
                    let call = Args { positional: vec![json!("vertex")], ..Default::default() };
                    return prev(s, &call);
                }
                return Ok(now);
            }
            prev(s, a)
        },
    );
    c.register(
        Cmd::new("hilite", "Hilite objects for component work: named, -r to replace, -u to take away, -tgl.", |s, a| {
            let names = a.named();
            let mode = if a.bool("unHilite") { "deselect" } else if a.bool("toggle") { "toggle" } else if a.bool("replace") { "" } else { "add" };
            let mut call = json!({"args": names});
            if !mode.is_empty() {
                call[mode] = json!(true);
            }
            run(s, "select", call)
        })
        .positional("objects")
        .flag(f("replace", "r", B, "Replace what is hilited."))
        .flag(f("toggle", "tgl", B, "Toggle."))
        .flag(f("unHilite", "u", B, "Take away.")),
    );
    c.register(
        Cmd::new("align", "Lines objects' bounding boxes up: align -x Mid a b c. Min, Mid or Max per axis; -atl to the last object.", align_cmd)
            .positional("objects")
            .flag(f("alignX", "x", S, "Min, Mid or Max."))
            .flag(f("alignY", "y", S, "Min, Mid or Max."))
            .flag(f("alignZ", "z", S, "Min, Mid or Max."))
            .flag(f("alignToLead", "atl", B, "To the last object rather than the whole set.")),
    );
    {
        let mut sn = Cmd::new("snapMode", "Move snapping switches: -grid, -point, -curve; -q to ask.", |s, a| {
            for key in ["grid", "point", "curve", "liveFace", "viewPlane", "meshCenter"] {
                if a.bool("query") {
                    if asked(a, key) {
                        return Ok(json!(s.snap_modes.get(key).copied().unwrap_or(false)));
                    }
                } else if a.has(key) {
                    s.snap_modes.insert(key.to_string(), a.bool(key));
                }
            }
            Ok(Value::Null)
        })
        .undoable(false);
        for (name, short) in [("grid", "gr"), ("point", "p"), ("curve", "c"), ("liveFace", "lf"), ("viewPlane", "vp"), ("meshCenter", "mc")] {
            sn = sn.flag(f(name, short, B, "This snap."));
        }
        c.register(sn.flag(f("query", "q", B, "Query.")).flag(f("queried", "", ArgType::List, "What -q asks for.")));
    }
    c.register(
        Cmd::new("angleBetween", "The rotation from -v1 to -v2: axis and angle in degrees, or -er Euler angles.", |_s, a| {
            let (Some(x), Some(y)) = (a.vec3("vector1"), a.vec3("vector2")) else { fail!("angleBetween: give -v1 and -v2") };
            let (x, y) = (DVec3::from_array(x).normalize_or_zero(), DVec3::from_array(y).normalize_or_zero());
            let angle = x.angle_between(y);
            let axis = x.cross(y).normalize_or_zero();
            if a.bool("euler") {
                let q = if axis == DVec3::ZERO { glam::DQuat::IDENTITY } else { glam::DQuat::from_axis_angle(axis, angle) };
                let e = crate::transform::matrix_to_euler(&glam::DMat3::from_quat(q), crate::transform::RotateOrder::Xyz);
                let k = if aztek_ops::units::user_call() { 1.0 / aztek_ops::units::rad_per_unit() } else { 1.0 };
                return Ok(json!([e.x * k, e.y * k, e.z * k]));
            }
            let angle = if aztek_ops::units::user_call() { aztek_ops::units::to_user(aztek_ops::units::Unit::Angle, angle) } else { angle };
            Ok(json!([axis.x, axis.y, axis.z, angle]))
        })
        .flag(f("vector1", "v1", ArgType::Vec3, "From."))
        .flag(f("vector2", "v2", ArgType::Vec3, "To."))
        .flag(f("euler", "er", B, "Euler angles instead."))
        .flag(f("constructionHistory", "ch", B, "Accepted for existing scripts."))
        .undoable(false),
    );
    c.register(
        Cmd::new("getModifiers", "Which modifier keys are down, as bits: none, when a script asks.", |_s, _a| Ok(json!(0))).undoable(false),
    );
    c.register(
        Cmd::new("exactWorldBoundingBox", "World bounds of objects with what is under them: xmin ymin zmin xmax ymax zmax.", |s, a| {
            let names = objects(s, a);
            let found = bounds_of(&mut s.graph, &names, a.bool("ignoreInvisible"))?;
            let (lo, hi) = found.unwrap_or((DVec3::ZERO, DVec3::ZERO));
            Ok(json!([lo.x, lo.y, lo.z, hi.x, hi.y, hi.z]))
        })
        .positional("objects")
        .flag(f("ignoreInvisible", "ii", B, "Leave out hidden shapes."))
        .flag(f("calculateExactly", "ce", B, "Accepted: bounds are always exact here."))
        .undoable(false),
    );
    extend(
        c,
        "polyEvaluate",
        vec![
            f("area", "a", B, "Area, object space."),
            f("worldArea", "wa", B, "Area, world space."),
            f("boundingBoxComponent", "bc", B, "World bounds of the selected components."),
            f("boundingBoxComponent2d", "bc2", B, "UV bounds of the selected components."),
            f("boundingBox2d", "b2", B, "UV bounds."),
            f("format", "fmt", B, "The counts as text."),
            f("accurateEvaluation", "ae", B, "Accepted: counts are always exact here."),
            f("ac", "ac", B, "Accepted for existing scripts."),
            f("triangleComponent", "tc", B, "Triangle count of the selected components."),
            f("usu", "usu", B, "Accepted for existing scripts."),
            f("vertexComponent", "vc", B, "Selected vertex count."),
            f("edgeComponent", "ec", B, "Selected edge count."),
            f("faceComponent", "fc", B, "Selected face count."),
            f("uvComponent", "uvc", B, "Selected UV count."),
            f("uvShell", "us", B, "UV shell count."),
            f("uvShellIds", "uid", B, "The UV shell of each selected UV."),
            f("uvsInShell", "uis", ArgType::Int, "The UVs in this UV shell."),
            f("activeUVShells", "aus", B, "The UV shells the selection touches."),
            f("activeShells", "as", B, "The mesh shells the selection touches."),
            f("uvSetName", "uvs", S, "The UV set: the current one here."),
            f("displayStats", "ds", B, "Accepted for existing scripts."),
        ],
        poly_evaluate,
    );
    c.register(Cmd::new("searchReplaceNames", "searchReplaceNames(search, replace, 'all'|'hierarchy'|'selected'): renames what holds search. Returns how many.", search_replace).positional("search replace scope"));
    c.register(
        Cmd::new("prefixHierarchy", "prefixHierarchy(root, prefix): puts prefix in front of the name of root and all under it.", |s, a| {
            let (Some(root), Some(prefix)) = (a.pos_str(0).map(str::to_string), a.pos_str(1).map(str::to_string)) else { fail!("prefixHierarchy: expected (root, prefix)") };
            let g = &mut s.graph;
            let id = node_of(g, &root, "prefixHierarchy")?;
            let mut ids = g.descendants(id);
            ids.reverse();
            ids.push(id);
            let mut out = Vec::new();
            for x in ids {
                let name = g.node(x).unwrap().name().to_string();
                g.rename(x, &format!("{prefix}{name}"))?;
                out.push(json!(g.path(x)));
            }
            out.reverse();
            Ok(Value::Array(out))
        })
        .positional("root prefix"),
    );
    c.register(
        Cmd::new("pwd", "The current working directory.", |_s, _a| {
            std::env::current_dir().map(|d| json!(d.to_string_lossy().replace('\\', "/"))).map_err(|e| aztek_ops::CommandError::new(e.to_string()))
        })
        .undoable(false),
    );
    c.register(
        Cmd::new("chdir", "Change the current working directory.", |_s, a| {
            let Some(d) = a.pos_str(0) else { fail!("chdir: give a directory") };
            std::env::set_current_dir(d).map_err(|e| aztek_ops::CommandError::new(format!("chdir: {d}: {e}")))?;
            Ok(Value::Null)
        })
        .positional("directory")
        .undoable(false),
    );
    extend(c, "ls", vec![f("uuid", "uid", B, "UUIDs rather than names.")], |s, a, prev| {
        // A UUID given stands for its node.
        let mut call = a.clone();
        let is_uuid = |x: &str| x.len() == 36 && x.as_bytes().get(8) == Some(&b'-');
        let mut uuid_named = false;
        for v in call.positional.iter_mut() {
            if let Some(u) = v.as_str().filter(|x| is_uuid(x)).map(str::to_string) {
                uuid_named = true;
                if let Some(id) = s.graph.nodes().into_iter().find(|&i| s.graph.node(i).is_some_and(|n| n.uuid() == u)) {
                    *v = json!(s.graph.path(id));
                } else {
                    *v = json!("__no_such_node__");
                }
            }
        }
        call.flags.remove("uuid");
        let r = prev(s, &call)?;
        if !a.bool("uuid") || uuid_named {
            return Ok(r);
        }
        let names: Vec<String> = r.as_array().map(|v| v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let mut out = Vec::new();
        for n in names {
            if let Ok(Some(id)) = s.graph.find(&n) {
                out.push(json!(s.graph.node(id).unwrap().uuid()));
            }
        }
        Ok(Value::Array(out))
    });
    {
        let mut ns = Cmd::new("namespace", "The reference's namespace command: -add, -set, -rm, -ex, -q, -mnr, -dnc, -ren.", namespace_cmd).positional("[namespace]");
        ns = ns.flag(f("query", "q", B, "Query.")).flag(f("queried", "", ArgType::List, "What -q asks for."));
        for (name, short, ty) in [
            ("addNamespace", "add", S), ("setNamespace", "set", S), ("removeNamespace", "rm", S), ("parent", "p", S), ("exists", "ex", A), ("relativeNames", "rel", B), ("isRootNamespace", "ir", A), ("validateName", "vn", S),
            ("mergeNamespaceWithRoot", "mnr", B), ("mergeNamespaceWithParent", "mnp", B), ("deleteNamespaceContent", "dnc", B), ("rename", "ren", A), ("moveNamespace", "mv", A), ("force", "f", B), ("recurse", "r", B), ("absoluteName", "an", B),
        ] {
            ns = ns.flag(f(name, short, ty, "As in the reference."));
        }
        c.register(ns);
    }
    {
        let mut ni = Cmd::new("namespaceInfo", "What a namespace holds: -cur, -lon, -lod, -ls, -r, -dp, -bn, -p.", namespace_info).positional("[namespace]").undoable(false);
        for (name, short) in [("currentNamespace", "cur"), ("listOnlyNamespaces", "lon"), ("listOnlyDependencyNodes", "lod"), ("listNamespace", "ls"), ("recurse", "r"), ("dagPath", "dp"), ("fullName", "fn"), ("absoluteName", "an"), ("baseName", "bn"), ("parent", "p"), ("isRootNamespace", "ir"), ("shortName", "sn")] {
            ni = ni.flag(f(name, short, B, "As in the reference."));
        }
        c.register(ni);
    }
}

fn objects_of(a: &Args, g: &Graph) -> Vec<String> {
    let named = a.named();
    if named.is_empty() {
        g.selection().iter().map(|&i| g.path(i)).collect()
    } else {
        named
    }
}
