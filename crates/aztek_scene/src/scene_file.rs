//! The scene document's graph part: nodes, hierarchy, stored values, user attrs, array elements, locks, connections, selection and time.
//!
//! Not implemented yet, so neither written nor read: references, playback range, render settings, scene scripts, scene
//! data, poses, pickers, UI sections, the binary and ASCII encodings, and the "writer" block's real version numbers.

use std::collections::HashMap;

use serde_json::{json, Map, Value};

use crate::error::{GraphError, Result};
use crate::graph::{Graph, SelectMode};
use crate::plug::PlugValue;
use crate::node::types;
use crate::plug::{
    attr_def_from_json, attr_def_to_json, flags, from_json, from_json_all, to_json, to_json_all,
    NodeId, PlugRef, PlugType, NO_NODE,
};

/// Format version.
pub const VERSION: i64 = 1;

/// Depth-first so every parent is written before its children.
fn collect_dag(g: &Graph, id: NodeId, out: &mut Vec<NodeId>) {
    out.push(id);
    for &c in g.children(id) {
        collect_dag(g, c, out);
    }
}

/// The scene as a document. `packed` writes meshes in their packed form.
pub fn serialize(g: &Graph, packed: bool) -> Value {
    let mut order = Vec::new();
    for &r in g.roots() {
        collect_dag(g, r, &mut order);
    }
    for id in g.nodes() {
        if !g.node(id).unwrap().ty().dag {
            order.push(id);
        }
    }
    // Saved as the reference plus its edits.
    order.retain(|&id| g.node(id).unwrap().reference_id() == 0);

    // Values first, all at once, so meshes convert in parallel.
    let mut values = Vec::new();
    let mut stored: Vec<Vec<usize>> = vec![Vec::new(); order.len()];
    for (k, &id) in order.iter().enumerate() {
        let n = g.node(id).unwrap();
        for (i, a) in n.ty().attrs.iter().enumerate() {
            if !a.has(flags::STORABLE) || !a.has(flags::INPUT) || a.is_array() {
                continue;
            }
            if g.source(&PlugRef::new(id, i as i32)).is_some() {
                continue;
            }
            if n.value(i) != a.stored_default() {
                stored[k].push(i);
                values.push(n.value(i));
            }
        }
    }
    // Data that names nodes converts here, with the graph; the rest in parallel.
    let in_graph: Vec<Option<Value>> = values
        .iter()
        .map(|v| match v {
            PlugValue::Data(Some(d)) => Some(d.to_json_in(g)),
            _ => None,
        })
        .collect();
    let rest: Vec<&PlugValue> = values.iter().zip(&in_graph).filter(|(_, j)| j.is_none()).map(|(v, _)| *v).collect();
    let mut rest = to_json_all(&rest, packed).into_iter();
    let mut converted = in_graph.into_iter().map(|j| j.unwrap_or_else(|| rest.next().unwrap()));

    let mut nodes = Vec::new();
    for (k, &id) in order.iter().enumerate() {
        let n = g.node(id).unwrap();
        let mut attrs = Map::new();
        for &i in &stored[k] {
            attrs.insert(n.ty().attrs[i].name.clone(), converted.next().unwrap());
        }
        let mut node = Map::new();
        node.insert("name".into(), json!(n.name()));
        node.insert("type".into(), json!(n.ty().name));
        node.insert("uuid".into(), json!(n.uuid()));
        if !n.aliases().is_empty() {
            node.insert("aliases".into(), json!(n.aliases()));
        }
        let channels: Vec<Value> = n.channels.iter().map(|((a, c), (k, b))| json!([n.ty().attrs[*a as usize].name, c, k, b])).collect();
        if !channels.is_empty() {
            node.insert("channels".into(), Value::Array(channels));
        }
        let p = g.parent(id);
        if p != NO_NODE {
            node.insert("parent".into(), json!(g.full_path(p)));
        }
        let user: Vec<Value> = n
            .ty()
            .attrs
            .iter()
            .filter(|a| a.has(flags::USER))
            .map(attr_def_to_json)
            .collect();
        if !user.is_empty() {
            node.insert("userAttrs".into(), Value::Array(user));
        }
        for (i, a) in n.ty().attrs.iter().enumerate() {
            if a.is_array() && a.has(flags::STORABLE) && a.has(flags::INPUT) {
                let mut els = Map::new();
                for (index, value) in n.elements(i) {
                    if g.source(&PlugRef::element(id, i as i32, *index)).is_none() {
                        els.insert(index.to_string(), to_json(value));
                    }
                }
                if !els.is_empty() {
                    attrs.insert(a.name.clone(), Value::Object(els));
                }
            }
        }
        if !attrs.is_empty() {
            node.insert("attrs".into(), Value::Object(attrs));
        }
        let mut locked = Vec::new();
        for (i, a) in n.ty().attrs.iter().enumerate() {
            let mask = n.lock_mask(i);
            if mask == 0 {
                continue;
            }
            if a.ty != PlugType::Vec3 {
                locked.push(json!(a.name));
            } else {
                for c in 0..3 {
                    if mask & (1 << c) != 0 {
                        locked.push(json!(a.child_names[c]));
                    }
                }
            }
        }
        if !locked.is_empty() {
            node.insert("locked".into(), Value::Array(locked));
        }
        if n.is_locked() {
            node.insert("nodeLocked".into(), json!(true));
        }
        nodes.push(Value::Object(node));
    }

    let connections: Vec<Value> = g
        .connections()
        .iter()
        // A reference keeps what it connects to the scene with its edits.
        .filter(|(dst, src)| [dst, src].iter().all(|p| g.node(p.node).is_some_and(|n| n.reference_id() == 0)))
        .map(|(dst, src)| json!([g.full_plug_name(src), g.full_plug_name(dst)]))
        .collect();
    let selection: Vec<Value> = g
        .selection()
        .iter()
        .map(|&id| json!(g.full_path(id)))
        .collect();

    json!({
        "aztek": VERSION,
        "writer": {"version": env!("CARGO_PKG_VERSION"), "build": "aztek"},
        "upAxis": "y",
        "units": "cm",
        "time": g.time(),
        "nodes": nodes,
        "connections": connections,
        "selection": selection,
        "namespaces": g.namespaces.iter().collect::<Vec<_>>(),
    })
}

fn member<'a>(j: &'a Value, key: &str) -> &'a [Value] {
    j.get(key)
        .and_then(Value::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn member_obj<'a>(j: &'a Value, key: &str) -> Option<&'a Map<String, Value>> {
    j.get(key).and_then(Value::as_object)
}

/// Creates the document's nodes and connections in `g`. Returns the nodes created; problems go to `warnings`.
pub fn instantiate(g: &mut Graph, doc: &Value, warnings: &mut Vec<String>) -> Vec<NodeId> {
    let mut created = Vec::new();
    let mut legacy_nonlinear: Vec<NodeId> = Vec::new();
    let mut legacy_handles: Vec<(NodeId, String)> = Vec::new();
    // Documents name parents and plug nodes by full path ("|Body|Arm"); older ones by name, unique back then.
    let mut by_path: HashMap<String, NodeId> = HashMap::new();
    let mut path_of: HashMap<NodeId, String> = HashMap::new();
    let mut by_name: HashMap<String, Vec<NodeId>> = HashMap::new();
    let lookup = |g: &Graph,
                  by_path: &HashMap<String, NodeId>,
                  by_name: &HashMap<String, Vec<NodeId>>,
                  r: &str|
     -> Result<Option<NodeId>> {
        if r.is_empty() {
            return Ok(None);
        }
        let key = if r.starts_with('|') {
            r.to_string()
        } else {
            format!("|{r}")
        };
        if let Some(&id) = by_path.get(&key) {
            return Ok(Some(id));
        }
        if !r.contains('|') {
            if let Some([one]) = by_name.get(r).map(Vec::as_slice) {
                return Ok(Some(*one));
            }
        }
        g.find(r)
    };

    // Meshes decode up front, in parallel; the loop below picks them up by node and attr.
    let mut mesh_items = Vec::new();
    let mut mesh_keys = Vec::new();
    for (k, jn) in member(doc, "nodes").iter().enumerate() {
        let Some(t) = types::find(jn.get("type").and_then(Value::as_str).unwrap_or("")) else {
            continue;
        };
        if let Some(attrs) = member_obj(jn, "attrs") {
            for (attr, value) in attrs {
                if let Some((idx, None)) = t.file_attr_index(attr) {
                    if t.attrs[idx].ty == PlugType::Mesh {
                        mesh_items.push((t.attrs[idx].clone(), value));
                        mesh_keys.push((k, attr.clone()));
                    }
                }
            }
        }
    }
    let refs: Vec<_> = mesh_items.iter().map(|(d, v)| (d, *v)).collect();
    let mut mesh_values: HashMap<(usize, String), crate::plug::FromJsonResult> =
        mesh_keys.into_iter().zip(from_json_all(&refs)).collect();

    for (k, jn) in member(doc, "nodes").iter().enumerate() {
        let ty_name = jn.get("type").and_then(Value::as_str).unwrap_or("");
        let name = jn
            .get("name")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        // Older documents: a nonlinear deformer of its kind's own type, and a handle of type deformHandle. Read as the
        // reference has them: a nonLinear with the kind's settings added, and a transform over the kind's shape.
        let legacy_kind = crate::deform::NONLINEAR.contains(&ty_name).then_some(ty_name);
        let legacy_handle = ty_name == "deformHandle";
        let found = match (legacy_kind, legacy_handle) {
            (Some(_), _) => types::find("nonLinear"),
            (_, true) => types::find("transform"),
            _ => types::find(ty_name),
        };
        let Some(t) = found else {
            warnings.push(format!(
                "unknown node type '{ty_name}' ({name}) skipped; is an extension missing?"
            ));
            continue;
        };
        let mut parent = NO_NODE;
        let mut key = format!("|{name}");
        if let Some(r) = jn.get("parent").and_then(Value::as_str) {
            match lookup(g, &by_path, &by_name, r) {
                Ok(Some(p)) => {
                    parent = p;
                    let base = path_of.get(&p).cloned().unwrap_or_else(|| {
                        if r.starts_with('|') {
                            r.to_string()
                        } else {
                            format!("|{r}")
                        }
                    });
                    key = base + &key;
                }
                Ok(None) => warnings.push(format!("{name}: parent {r} not found")),
                Err(e) => {
                    warnings.push(format!("{name}: {e}"));
                    warnings.push(format!("{name}: parent {r} not found"));
                }
            }
        }
        let id = match g.create_node(&t, &name, parent, false) {
            Ok(id) => id,
            Err(e) => {
                warnings.push(format!("{name}: {e}"));
                continue;
            }
        };
        if let Some(u) = jn.get("uuid").and_then(Value::as_str) {
            g.set_uuid(id, u);
        }
        if let Some(kind) = legacy_kind {
            if let Err(e) = crate::deform::add_nonlinear_attrs(g, id, kind) {
                warnings.push(format!("{name}: {e}"));
            }
            legacy_nonlinear.push(id);
        }
        if legacy_handle {
            let kind = member_obj(jn, "attrs").and_then(|a| a.get("deformerType")).and_then(Value::as_str).unwrap_or("");
            legacy_handles.push((id, kind.to_string()));
        }
        by_path.insert(key.clone(), id);
        path_of.insert(id, key);
        by_name.entry(name.clone()).or_default().push(id);
        created.push(id);

        for def in member(jn, "userAttrs") {
            let r = attr_def_from_json(def).and_then(|d| g.add_attr(id, d));
            if let Err(e) = r {
                warnings.push(format!("{name}: user attribute {def} skipped: {e}"));
            }
        }
        let stored = member_obj(jn, "attrs");
        for (i, a) in t.attrs.iter().enumerate() {
            if let Some(fd) = &a.file_default {
                if !stored.is_some_and(|s| s.contains_key(&a.name)) {
                    if let Err(e) = g.set_value(PlugRef::new(id, i as i32), fd.clone()) {
                        warnings.push(format!("{name}.{}: {e}", a.name));
                    }
                }
            }
        }
        let nt = g.node(id).unwrap().type_arc().clone();
        for (attr, value) in stored.into_iter().flatten() {
            let Some((idx, element)) = nt.file_attr_index(attr) else {
                let legacy = (legacy_handle && attr == "deformerType") || (legacy_kind.is_some() && attr.starts_with("unused"));
                if !nt.is_retired(attr) && !legacy {
                    warnings.push(format!("{name}.{attr}: no such attribute, ignored"));
                }
                continue;
            };
            let def = &nt.attrs[idx];
            let r: Result<()> = (|| {
                if let Some(element) = element {
                    return g.set_value(
                        PlugRef::element(id, idx as i32, element),
                        from_json(def, value)?,
                    );
                }
                if def.is_array() {
                    for (index, ev) in value.as_object().into_iter().flatten() {
                        let index: i32 = index.parse().map_err(|_| {
                            GraphError::invalid(format!("bad element index '{index}'"))
                        })?;
                        g.set_value(PlugRef::element(id, idx as i32, index), from_json(def, ev)?)?;
                    }
                    return Ok(());
                }
                if let Some(m) = mesh_values.remove(&(k, attr.clone())) {
                    return match m.value {
                        Some(v) => g.set_value(PlugRef::new(id, idx as i32), v),
                        None => Err(GraphError::invalid(m.error)),
                    };
                }
                g.set_value(PlugRef::new(id, idx as i32), from_json(def, value)?)
            })();
            if let Err(e) = r {
                warnings.push(format!("{name}.{attr}: {e}"));
            }
        }
        for (alias, target) in member_obj(jn, "aliases").into_iter().flatten() {
            if let Some(t) = target.as_str() {
                g.set_alias(id, alias, Some(t.to_string()));
            }
        }
        for c in member(jn, "channels") {
            if let (Some(attr), Some(comp), Some(k), Some(b)) = (c[0].as_str(), c[1].as_i64(), c[2].as_bool(), c[3].as_bool()) {
                if let Some(i) = nt.attr_index(attr) {
                    g.set_channel_state(&PlugRef { node: id, attr: i as i32, component: comp as i32, element: -1 }, Some(k), Some(b));
                }
            }
        }
        if jn.get("nodeLocked").and_then(Value::as_bool) == Some(true) {
            g.set_node_locked(id, true);
        }
        // After the values, which a lock would otherwise refuse.
        for plug in member(jn, "locked") {
            let r = plug
                .as_str()
                .ok_or_else(|| GraphError::invalid("not a string"))
                .and_then(|p| g.plug(&format!("{}.{p}", g.path(id))))
                .and_then(|p| g.set_locked(&p, true));
            if let Err(e) = r {
                warnings.push(format!("{name}: can't lock {plug}: {e}"));
            }
        }
    }

    let map_plug = |g: &Graph, plug: &str| -> Result<String> {
        let dot = plug
            .find('.')
            .ok_or_else(|| GraphError::invalid(format!("expected node.attr, got '{plug}'")))?;
        let id = lookup(g, &by_path, &by_name, &plug[..dot])?
            .ok_or_else(|| GraphError::invalid(format!("no node named '{}'", &plug[..dot])))?;
        let n = g.node(id).unwrap();
        let node = g.path(id);
        let rest = &plug[dot + 1..];
        let end = rest.find(['.', '[']);
        let attr = &rest[..end.unwrap_or(rest.len())];
        if let Some((idx, _)) = n.ty().file_attr_index(attr) {
            if n.ty().attr_index(attr).is_none() {
                return Ok(format!(
                    "{node}.{}{}",
                    n.ty().attrs[idx].name,
                    end.map(|e| &rest[e..]).unwrap_or("")
                ));
            }
        }
        Ok(format!("{node}.{rest}"))
    };
    for c in member(doc, "connections") {
        let r: Result<()> = (|| {
            let (a, b) = match c.as_array().map(Vec::as_slice) {
                Some([a, b]) => (a.as_str().unwrap_or(""), b.as_str().unwrap_or("")),
                _ => return Err(GraphError::invalid(format!("bad connection {c}"))),
            };
            let src = g.plug(&map_plug(g, a)?)?;
            let dst = g.plug(&map_plug(g, b)?)?;
            g.connect(&src, &dst, false)?;
            Ok(())
        })();
        if let Err(e) = r {
            warnings.push(format!("connection skipped: {e}"));
        }
    }
    for (h, kind) in legacy_handles {
        if let Err(e) = crate::deform::add_handle_shape(g, h, &kind) {
            warnings.push(format!("{}: {e}", g.path(h)));
        }
    }
    for n in legacy_nonlinear {
        if crate::deform::leader_of(g, n) == n {
            let _ = crate::deform::link_handle_shape(g, n);
        }
    }
    created
}

/// Replaces the graph's contents with the document's. Undo is off while it loads and cleared after. Returns warnings.
pub fn deserialize(g: &mut Graph, doc: &Value) -> Result<Vec<String>> {
    if !doc.get("aztek").is_some_and(Value::is_i64) {
        return Err(GraphError::invalid("not an Aztek scene"));
    }
    let mut warnings = Vec::new();
    g.begin_batch();
    g.clear();
    g.undo_stack_mut().set_enabled(false);
    instantiate(g, doc, &mut warnings);
    g.namespaces = member(doc, "namespaces").iter().filter_map(Value::as_str).map(str::to_string).collect();

    let mut sel = Vec::new();
    for n in member(doc, "selection") {
        match g.find(n.as_str().unwrap_or("")) {
            Ok(Some(id)) => sel.push(id),
            Ok(None) => {}
            Err(e) => warnings.push(format!("selection: {e}")),
        }
    }
    g.select(&sel, SelectMode::Replace);
    g.set_time(doc.get("time").and_then(Value::as_f64).unwrap_or(0.0));
    g.undo_stack_mut().set_enabled(true);
    g.undo_stack_mut().clear();
    g.end_batch();
    for w in &warnings {
        log::warn!("{w}");
    }
    Ok(warnings)
}
