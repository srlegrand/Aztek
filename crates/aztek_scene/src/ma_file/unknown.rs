//! Nodes of a type nobody here knows, as the reference keeps them: a plug-in's whose plug-in isn't loaded. Each becomes
//! an unknown node that keeps its type, its plug-in, its setAttr lines and its connections, and is written back as it
//! came.
//!
//! - In the hierarchy, before the scene is built: one with children, or that sets translate, rotate or scale, is an
//!   `unknownTransform` and moves its children; any other with a parent is an `unknownDag`. Their transform values,
//!   visibility and added attributes are read as any node's; the rest is kept as text. A connection to one of those, from
//!   a node that is read, is made; the rest are kept as text.
//! - Outside it, once the scene is built: what nothing read becomes an `unknown`. Its added attributes are made and set.
//!
//! Left out, as before: what the reader takes apart itself (groupParts, tweak and the like), renderers' nodes, and
//! anything that makes a shape's geometry, which is baked into the shape.

use super::dynamics::{set_line, string_set};
use super::*;
use crate::core_nodes::{UNKNOWN, UNKNOWN_DAG, UNKNOWN_TRANSFORM};

/// A connection into one of these, on a shape, carries the geometry that makes it.
const SHAPE_GEOMETRY: &[&str] = &["i", "inMesh", "cr", "create"];
/// A connection into one of these, anywhere, carries geometry being made or deformed.
const GEOMETRY_INPUTS: &[&str] = &["ip.ig", "input.inputGeometry", "ig", "inputGeometry", "ip", "inputPolymesh", "inputCurve", "inputSurface", "imsh", "inputMesh"];
/// Setting one of these says the node is a transform.
const TRANSFORM_SETS: &[&str] = &["t", "r", "s", "translate", "rotate", "scale", "tx", "ty", "tz", "rx", "ry", "rz", "sx", "sy", "sz", "rp", "sp", "ro"];

/// Nodes whose output feeds geometry: how a shape is made, which the shape holds baked.
fn geometry_chain(doc: &Doc) -> HashSet<usize> {
    let mut out = HashSet::new();
    for c in &doc.conns {
        let a = plain_attr(&c.dst_attr);
        let shape = matches!(doc.nodes[c.dst].ty.as_str(), "mesh" | "nurbsCurve" | "nurbsSurface");
        if (shape && SHAPE_GEOMETRY.contains(&a.as_str())) || GEOMETRY_INPUTS.contains(&a.as_str()) {
            out.insert(c.src);
        }
    }
    out
}

/// A node of the file nobody here reads or knows.
fn unread_type(doc: &Doc, i: usize, chain: &HashSet<usize>) -> bool {
    let n = &doc.nodes[i];
    !n.default
        && !n.ty.is_empty()
        && n.ty != "mesh"
        && types::find(&n.ty).is_none()
        && !QUIET_TYPES.contains(&n.ty.as_str())
        // Read by name, from a part of Aztek that may not be loaded (USD's shapes).
        && !PLAIN_SHAPES.contains(&n.ty.as_str())
        && !PLAIN_NODES.contains(&n.ty.as_str())
        && !is_render_type(&n.ty)
        && creator_attrs(&n.ty).is_none()
        && !chain.contains(&i)
}

fn in_hierarchy(doc: &Doc, i: usize) -> bool {
    let n = &doc.nodes[i];
    n.parent.is_some() || n.ext_parent.is_some() || doc.children(i).next().is_some() || n.sets.iter().any(|s| s.path.first().is_some_and(|g| TRANSFORM_SETS.contains(&g.name.as_str())))
}

/// The plug-in a file's requires lines give a type, and its version.
fn plugin_of(doc: &Doc, ty: &str) -> (String, String) {
    let Some((name, _)) = doc.requires.iter().find(|(_, ts)| ts.iter().any(|t| t == ty)) else { return (String::new(), String::new()) };
    (name.clone(), doc.plugin_versions.get(name).cloned().unwrap_or_default())
}

/// A node's name as a kept connection gives it; the scene's own nodes with a colon.
fn conn_name(doc: &Doc, i: usize) -> String {
    if doc.nodes[i].default {
        format!(":{}", doc.nodes[i].name)
    } else {
        doc.nodes[i].name.clone()
    }
}

/// One kept connection: source, a tab, destination. `@` stands for the node that keeps it, `%n` for the file's node n
/// until the scene is built (see `settle`).
fn conn_line(c: &Conn, owner: usize) -> String {
    let end = |i: usize, attr: &str| if i == owner { format!("@.{attr}") } else { format!("%{i}.{attr}") };
    format!("{}\t{}", end(c.src, &c.src_attr), end(c.dst, &c.dst_attr))
}

/// A setAttr kept as text: its lock, and nothing at all when it set neither a value nor a lock (the file's `-s` and
/// `-k` alone are not kept).
fn kept_set(s: &SetAttr) -> Option<String> {
    if s.values.is_empty() && s.lock.is_none() {
        return None;
    }
    let line = set_line(s);
    match s.lock {
        Some(l) => {
            let line = line.strip_suffix(" ;").map(|x| format!("{x};")).unwrap_or(line);
            Some(line.replacen("setAttr ", &format!("setAttr -l {} ", if l { "on" } else { "off" }), 1))
        }
        None => Some(line),
    }
}

fn names_added(n: &MaNode, name: &str) -> bool {
    n.adds.iter().any(|a| a.long == name || a.short == name)
}

fn plugin_note(plugin: &str) -> String {
    if plugin.is_empty() {
        String::new()
    } else {
        format!(" (plug-in {plugin})")
    }
}

/// Unknown nodes in the hierarchy, before the scene is built. Returns warnings.
pub(super) fn hold_dag(doc: &mut Doc) -> Vec<String> {
    crate::core_nodes::register_types();
    let chain = geometry_chain(doc);
    let mut held: HashMap<usize, Arc<NodeType>> = HashMap::new();
    for i in 0..doc.nodes.len() {
        if !unread_type(doc, i, &chain) || !in_hierarchy(doc, i) {
            continue;
        }
        // A deformer's handle shape: read with its deformer.
        if let Some(p) = doc.nodes[i].parent {
            if deformers::handle_under(doc, p).is_some_and(|(shape, _)| shape == i) {
                continue;
            }
        }
        let n = &doc.nodes[i];
        let transform = doc.children(i).next().is_some() || n.sets.iter().any(|s| s.path.first().is_some_and(|g| TRANSFORM_SETS.contains(&g.name.as_str())));
        let kind = if transform || n.parent.is_none() && n.ext_parent.is_none() { UNKNOWN_TRANSFORM } else { UNKNOWN_DAG };
        let Some(t) = types::find(kind) else { continue };
        held.insert(i, t);
    }
    if held.is_empty() {
        return Vec::new();
    }
    let mut by_type: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for (&i, t) in &held {
        let old = doc.nodes[i].ty.clone();
        let (plugin, version) = plugin_of(doc, &old);
        let n = &mut doc.nodes[i];
        let mut kept = Vec::new();
        let mut lines = Vec::new();
        for s in std::mem::take(&mut n.sets) {
            let name = s.path.first().map(|g| g.name.clone()).unwrap_or_default();
            if s.path.len() == 1 && s.path[0].range.is_none() && (channel(t, &name).is_some() || names_added(n, &name)) {
                kept.push(s);
            } else if s.values.is_empty() && s.lock.is_none() && t.attr_index(&name).is_some() {
                // `setAttr -k off ".v"` and the like, on what the node has here: written again with it.
            } else if let Some(l) = kept_set(&s) {
                lines.push(l);
            }
        }
        kept.push(string_set("fty", &old));
        kept.push(string_set("fat", &lines.join("\n")));
        kept.push(string_set("fpl", &plugin));
        kept.push(string_set("fpv", &version));
        n.sets = kept;
        n.ty = t.name.clone();
        let e = by_type.entry(old).or_insert((0, plugin));
        e.0 += 1;
    }

    // A connection with a node that is read, on a channel or an added attribute of the held one, is made; the rest are
    // kept as text on the held node (the destination, when both are held).
    let mut kept_lines: HashMap<usize, Vec<String>> = HashMap::new();
    let conns = std::mem::take(&mut doc.conns);
    let mut out = Vec::with_capacity(conns.len());
    for c in conns {
        let (hs, hd) = (held.get(&c.src), held.get(&c.dst));
        if hs.is_none() && hd.is_none() {
            out.push(c);
            continue;
        }
        let real = match (hs, hd) {
            (Some(t), None) => types::find(&doc.nodes[c.dst].ty).is_some() && (channel(t, &plain_attr(&c.src_attr)).is_some() || names_added(&doc.nodes[c.src], &plain_attr(&c.src_attr))),
            (None, Some(t)) => types::find(&doc.nodes[c.src].ty).is_some() && (channel(t, &plain_attr(&c.dst_attr)).is_some() || names_added(&doc.nodes[c.dst], &plain_attr(&c.dst_attr))),
            _ => false,
        };
        if real {
            out.push(c);
            continue;
        }
        let owner = if hd.is_some() { c.dst } else { c.src };
        kept_lines.entry(owner).or_default().push(conn_line(&c, owner));
    }
    doc.conns = out;
    for (i, lines) in kept_lines {
        doc.nodes[i].sets.push(string_set("fcn", &lines.join("\n")));
    }
    by_type
        .into_iter()
        .map(|(ty, (k, plugin))| format!("{ty}{}: {k} node(s) of a type that isn't here, kept as unknown nodes and written back as they came", plugin_note(&plugin)))
        .collect()
}

impl Build<'_> {
    /// Kept connections with the file's node numbers made into what lasts: a node that was built by its identity
    /// (`#uuid`), which a rename doesn't change; any other by its name in the file.
    fn settle(&self, text: &str) -> String {
        let end = |e: &str| -> String {
            let Some((node, attr)) = e.split_once('.') else { return e.to_string() };
            let Some(i) = node.strip_prefix('%').and_then(|k| k.parse::<usize>().ok()).filter(|&k| k < self.doc.nodes.len()) else { return e.to_string() };
            match self.ids[i].and_then(|id| self.s.graph.node(id)) {
                Some(n) => format!("#{}.{attr}", n.uuid()),
                None => format!("{}.{attr}", conn_name(self.doc, i)),
            }
        };
        text.lines().filter_map(|l| l.split_once('\t')).map(|(a, b)| format!("{}\t{}", end(a), end(b))).collect::<Vec<_>>().join("\n")
    }

    /// After the scene is built: what the held hierarchy nodes keep of the file, and the nodes outside it that nothing
    /// read, as unknown nodes.
    pub(super) fn hold_unused(&mut self) {
        let doc = self.doc;
        let keep_attrs = ["fty", "fat", "fcn", "fpl", "fpv"];
        for i in 0..doc.nodes.len() {
            if !matches!(doc.nodes[i].ty.as_str(), UNKNOWN_DAG | UNKNOWN_TRANSFORM) {
                continue;
            }
            let Some(id) = self.ids[i] else { continue };
            for a in keep_attrs {
                let mut v = doc.nodes[i].text(&[a]).unwrap_or("").to_string();
                if a == "fcn" {
                    v = self.settle(&v);
                }
                if let Some(at) = self.s.graph.node(id).and_then(|n| n.ty().attr_index(a)) {
                    let _ = self.s.graph.set_value(PlugRef::new(id, at as i32), PlugValue::String(v));
                }
            }
        }

        let chain = geometry_chain(doc);
        let held: Vec<usize> = (0..doc.nodes.len()).filter(|&i| !self.used[i] && self.ids[i].is_none() && unread_type(doc, i, &chain) && !in_hierarchy(doc, i)).collect();
        if held.is_empty() {
            return;
        }
        let Some(t) = types::find(UNKNOWN) else { return };
        let is_held: HashSet<usize> = held.iter().copied().collect();
        let mut by_type: BTreeMap<String, (usize, String)> = BTreeMap::new();
        for &i in &held {
            let n = &doc.nodes[i];
            let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
            let Ok(id) = self.s.graph.create_node(&t, &name, NO_NODE, false) else { continue };
            self.ids[i] = Some(id);
            self.used[i] = true;
            self.add_user_attrs(i);
            self.apply_channels(i);
            let lines: Vec<String> = n.sets.iter().filter(|s| !(s.path.len() == 1 && names_added(n, &s.path[0].name))).filter_map(kept_set).collect();
            // Kept on the destination when both ends are held.
            let conns: Vec<String> = doc.conns.iter().filter(|c| c.dst == i || (c.src == i && !is_held.contains(&c.dst))).map(|c| conn_line(c, i)).collect();
            let (plugin, version) = plugin_of(doc, &n.ty);
            let conns = self.settle(&conns.join("\n"));
            for (a, v) in [("fty", n.ty.clone()), ("fat", lines.join("\n")), ("fcn", conns), ("fpl", plugin.clone()), ("fpv", version)] {
                if let Some(at) = t.attr_index(a) {
                    let _ = self.s.graph.set_value(PlugRef::new(id, at as i32), PlugValue::String(v));
                }
            }
            if n.locked {
                self.s.graph.set_node_locked(id, true);
            }
            let e = by_type.entry(n.ty.clone()).or_insert((0, plugin));
            e.0 += 1;
        }
        for (ty, (k, plugin)) in by_type {
            self.warnings.push(format!("{ty}{}: {k} node(s) of a type that isn't here, kept as unknown nodes and written back as they came", plugin_note(&plugin)));
        }
    }
}

/// The plug-ins the kept unknown nodes came from, with the versions their files gave and the types kept from each.
pub(super) fn plugins(s: &Session, keep: &HashSet<NodeId>) -> BTreeMap<String, (String, BTreeSet<String>)> {
    let g = &s.graph;
    let mut out = BTreeMap::new();
    for &id in keep {
        let Some(n) = g.node(id) else { continue };
        if !crate::core_nodes::is_unknown(&n.ty().name) {
            continue;
        }
        let get = |a: &str| n.ty().attr_index(a).map(|i| n.value(i).as_str().to_string()).unwrap_or_default();
        let p = get("filePlugin");
        if !p.is_empty() {
            let e = out.entry(p).or_insert_with(|| (get("filePluginVersion"), BTreeSet::new()));
            e.1.insert(get("fileType"));
        }
    }
    out
}

impl Writer<'_> {
    fn unknown_text(&self, id: NodeId, attr: &str) -> String {
        let Some(n) = self.s.graph.node(id) else { return String::new() };
        n.ty().attr_index(attr).map(|i| n.value(i).as_str().to_string()).unwrap_or_default()
    }

    /// What an unknown node kept of its file, after its createNode line and channels.
    fn unknown_rest(&mut self, id: NodeId) {
        for l in self.unknown_text(id, "fileAttributes").lines() {
            let _ = writeln!(self.out, "\t{l}");
        }
        if self.s.graph.node(id).is_some_and(|n| n.is_locked()) {
            let _ = writeln!(self.out, "\tlockNode -l 1 ;");
        }
        let me = self.s.graph.path(id);
        let kept = self.unknown_text(id, "fileConnections");
        let by_uuid: HashMap<String, NodeId> = if kept.contains('#') { self.s.graph.nodes().into_iter().filter_map(|n| Some((self.s.graph.node(n)?.uuid().to_string(), n))).collect() } else { HashMap::new() };
        for line in kept.lines() {
            let Some((a, b)) = line.split_once('\t') else { continue };
            let (Some(a), Some(b)) = (self.kept_end(a, &me, &by_uuid), self.kept_end(b, &me, &by_uuid)) else { continue };
            self.conns.push(format!("connectAttr {} {};", quoted(&a), quoted(&b)));
        }
    }

    /// One end of a kept connection as it is now: `@` is the node itself, `#uuid` a node by its identity, anything else
    /// a node by its name; the end of a node that isn't written is none.
    fn kept_end(&self, end: &str, me: &str, by_uuid: &HashMap<String, NodeId>) -> Option<String> {
        let (node, attr) = end.split_once('.')?;
        if node == "@" {
            return Some(format!("{me}.{attr}"));
        }
        let g = &self.s.graph;
        let id = match node.strip_prefix('#') {
            Some(u) => *by_uuid.get(u)?,
            None => g.find(node.trim_start_matches(':')).ok().flatten()?,
        };
        let written = self.keep.contains(&id) || g.node(id).is_some_and(|n| n.reference_id() != 0 || (n.namespace().is_empty() && crate::defaults::is_scene_node(n.name())));
        if !written {
            return None;
        }
        let path = if g.node(id).is_some_and(|n| n.reference_id() != 0) { g.full_path(id) } else { g.path(id) };
        let scene_node = g.node(id).is_some_and(|n| n.namespace().is_empty() && crate::defaults::is_scene_node(n.name()));
        let path = if node.starts_with(':') || scene_node { format!(":{}", path.trim_start_matches('|')) } else { path };
        Some(format!("{path}.{attr}"))
    }

    /// An unknown node outside the hierarchy, as its file had it.
    pub(super) fn write_unknown(&mut self, id: NodeId) {
        let ty = self.unknown_text(id, "fileType");
        if ty.is_empty() {
            return;
        }
        let path = self.s.graph.path(id);
        let _ = writeln!(self.out, "createNode {ty} -n {};", quoted(&path));
        self.write_channels(id);
        self.unknown_rest(id);
    }

    /// An unknown node in the hierarchy: its type from the file, its transform values and added attributes as they are
    /// now, then what else the file had.
    pub(super) fn write_unknown_dag(&mut self, id: NodeId, name: &str, parent: NodeId) {
        let mut ty = self.unknown_text(id, "fileType");
        if ty.is_empty() {
            ty = if self.s.graph.node(id).is_some_and(|n| n.ty().is_a("transform")) { "transform".into() } else { "locator".into() };
        }
        self.create_line(&ty, name, parent);
        self.write_channels(id);
        self.unknown_rest(id);
    }
}
