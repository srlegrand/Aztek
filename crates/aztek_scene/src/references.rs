//! File references, as the reference has them. `file -r` brings a file's nodes in under a namespace, marked as referenced; the scene
//! saves the path and its edits rather than the contents, so a change to the file reaches every scene that uses it.
//!
//! An entry keeps `edits`, this scene's changes to the nodes, and `file_edits`, the ones a referenced file keeps on a file it
//! references in turn. Edits have the reference's kinds and text (the reference node's `.ed`): 0 parent, 1 addAttr, 2 setAttr, 3
//! disconnectAttr, 5 a connection through the placeHolderList, 7 deleteAttr, 8 lock. While a reference is loaded the
//! scene is the truth: `gather` compares it with what the file gave. An edit that couldn't be applied is kept, marked
//! failed, for when it can. Loading one flushes undo, as in the reference.
//!
//! Nesting: a file's own references become entries with a `parent`, their namespaces and reference nodes prefixed by the
//! namespace above (`mid:bot`, `mid:botRN`). Only a top level reference has a node in a saved file; its `.ed` holds edits
//! for the ones below in sections by reference node, and `file -rdi 2` lines keep their load state.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, CommandError, Commands};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::ref_core::{asked, f};
use crate::node::{types, NodeType, Plain};
use crate::plug::{flags, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::system_cmds::q_flags;

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, String as S};

pub const REF_NODE_TYPE: &str = "reference";

/// One edit, in the form it has in a file. Paths are full DAG paths with namespaces.
#[derive(Clone, Debug, PartialEq)]
pub struct RefEdit {
    pub kind: i32,
    pub args: Vec<String>,
    /// Couldn't be applied when last loaded.
    pub failed: bool,
    /// Kept as written: the scene can't show it as a difference (`-k`, `-cb`, a value of a type not read).
    opaque: bool,
}

impl RefEdit {
    fn new(kind: i32, args: Vec<String>) -> Self {
        RefEdit { kind, args, failed: false, opaque: false }
    }

    fn command(&self) -> &'static str {
        match self.kind {
            0 => "parent",
            1 => "addAttr",
            2 => "setAttr",
            3 => "disconnectAttr",
            5 => "connectAttr",
            7 => "deleteAttr",
            8 => "lock",
            9 => "unlock",
            _ => "",
        }
    }

    /// As a MEL command, for `referenceQuery -es`.
    fn text(&self) -> String {
        let a = &self.args;
        let at = |i: usize| a.get(i).map(String::as_str).unwrap_or("");
        match self.kind {
            0 => format!("parent {} \"{}\" \"{}\"", at(2).trim(), at(0), at(1)),
            1 => format!("addAttr{} -ln \"{}\" -sn \"{}\" \"{}\"", at(3), at(1), at(2), at(0)),
            2 => format!("setAttr {}.{}{}", at(0), at(1), at(2)),
            3 => format!("disconnectAttr \"{}\" \"{}\"", at(0), at(1)),
            7 => format!("deleteAttr {}.{}", at(0), at(1)),
            8 => format!("lock {}.{}", at(0), at(1)),
            9 => format!("unlock {}.{}", at(0), at(1)),
            _ => format!("{} {}", self.kind, a.join(" ")),
        }
    }

    /// The nodes it's about, and the attribute where it has one: (node, attribute).
    fn targets(&self) -> Vec<(String, Option<String>)> {
        let a = &self.args;
        let split = |p: &str| match p.rsplit_once('.') {
            Some((n, at)) => (n.to_string(), Some(at.to_string())),
            None => (p.to_string(), None),
        };
        match self.kind {
            0 => a.first().map(|c| (c.clone(), None)).into_iter().collect(),
            1 | 2 | 7 | 8 | 9 => a.first().map(|n| (n.clone(), a.get(1).cloned())).into_iter().collect(),
            3 => a.iter().take(2).map(|p| split(p)).collect(),
            _ => Vec::new(),
        }
    }
}

/// A connection between a reference and anything else, kept by name while it can't be made.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RefConn {
    pub src: String,
    pub dst: String,
    /// Which ends are in a reference.
    pub src_ref: bool,
    pub dst_ref: bool,
    pub failed: bool,
}

#[derive(Default)]
struct Baseline {
    values: HashMap<(NodeId, u32, i32), PlugValue>,
    parents: HashMap<NodeId, NodeId>,
    conns: HashSet<(PlugRef, PlugRef)>,
    attrs: HashMap<NodeId, HashSet<String>>,
    locks: HashMap<(NodeId, u32), u8>,
}

#[derive(Default)]
pub struct RefEntry {
    pub id: u32,
    /// As written; may hold variables or a path from another machine.
    pub unresolved: String,
    /// Where it was found, or `unresolved` if it wasn't.
    pub path: String,
    pub namespace: String,
    /// Prefix of an old style `-rpr` reference, in place of a namespace.
    pub prefix: String,
    /// The reference node.
    pub rfn: String,
    pub loaded: bool,
    /// The reference above, 0 for a top level one.
    pub parent: u32,
    pub locked: bool,
    pub file_type: String,
    pub options: String,
    /// The scene's edits.
    pub edits: Vec<RefEdit>,
    /// Connections with the scene or another reference.
    pub connections: Vec<RefConn>,
    /// The edits and connections the referencing file holds on this one.
    pub file_edits: Vec<RefEdit>,
    pub file_conns: Vec<RefConn>,
    baseline: Baseline,
}

#[derive(Default)]
pub struct RefTable {
    pub entries: Vec<RefEntry>,
    next: u32,
    /// How many files are being read inside one another.
    depth: u32,
    /// The references being read, innermost last.
    stack: Vec<u32>,
    /// Edits read from a file, by reference node, before the entry exists or for one not loaded.
    pending: HashMap<String, Vec<RefEdit>>,
    pending_conns: HashMap<String, Vec<RefConn>>,
    pending_file: HashMap<String, Vec<RefEdit>>,
    /// Whether a reference starts unloaded, from `-rdi` lines and from the last time it was unloaded with its parent.
    load_state: HashMap<String, bool>,
    /// Entries read by the file now being opened, waiting for the scene's edits.
    fresh: Vec<u32>,
    /// The folders of the files read so far: where a relative path, and a texture, may be.
    pub search: Vec<PathBuf>,
    /// The nodes the last load made.
    last_new: Vec<NodeId>,
}

/// A `file -r` line as read.
#[derive(Clone, Debug, Default)]
pub struct FileRef {
    pub path: String,
    pub ns: String,
    pub rfn: Option<String>,
    pub deferred: bool,
    pub rpr: Option<String>,
    pub file_type: String,
    pub options: String,
    pub locked: bool,
    /// -mergeNamespacesOnClash: use the namespace even if it's there already.
    pub merge: bool,
}

// ---------------------------------------------------------------------------------------------------------------------
// Names

/// Puts a namespace on every step of a path: `|c:x|y.tx` under `a` is `|a:c:x|a:y.tx`.
pub(crate) fn prefix_path(path: &str, ns: &str) -> String {
    if ns.is_empty() || path.is_empty() {
        return path.to_string();
    }
    let (nodes, attr) = match path.rfind('|').map_or(path.find('.'), |bar| path[bar..].find('.').map(|d| d + bar)) {
        Some(d) => path.split_at(d),
        None => (path, ""),
    };
    let steps: Vec<String> = nodes.split('|').map(|s| if s.is_empty() { String::new() } else { format!("{ns}:{}", s.trim_start_matches(':')) }).collect();
    format!("{}{attr}", steps.join("|"))
}

fn prefix_edit(e: &mut RefEdit, ns: &str) {
    let paths: &[usize] = match e.kind {
        0 | 3 => &[0, 1],
        1 | 2 | 7 | 8 | 9 => &[0],
        _ => &[],
    };
    for &i in paths {
        if let Some(a) = e.args.get_mut(i) {
            *a = prefix_path(a, ns);
        }
    }
}

/// The node of a path that moved: by the whole path, else by its last step.
fn find_node(g: &Graph, path: &str) -> Option<NodeId> {
    if let Ok(Some(id)) = g.find(path) {
        return Some(id);
    }
    let leaf = path.rsplit('|').next()?;
    g.find(leaf).ok().flatten()
}

fn find_plug(g: &Graph, plug: &str) -> Option<PlugRef> {
    if let Ok(p) = g.plug(plug) {
        return Some(p);
    }
    let bar = plug.rfind('|')?;
    g.plug(&plug[bar + 1..]).ok()
}

/// A DAG node by its full path, anything else by its name, as edits name them.
fn edit_path(g: &Graph, n: NodeId) -> String {
    if g.node(n).is_some_and(|x| x.ty().dag) {
        g.full_path(n)
    } else {
        g.path(n)
    }
}

fn plug_full_name(g: &Graph, p: &PlugRef) -> String {
    let name = g.plug_name(p);
    let attr = name.split_once('.').map_or("", |x| x.1);
    format!("{}.{attr}", edit_path(g, p.node))
}

/// Where a reference's file is: as written, under the project and the folders of the scene and of the file referencing it,
/// then by its name in those. A path from another machine is found this way.
fn resolve(s: &mut Session, written: &str) -> Option<PathBuf> {
    let expanded = crate::shading::expand_env(written.trim()).replace('\\', "/");
    let expanded = expanded.split('{').next().unwrap_or(&expanded).to_string();
    let as_written = Path::new(&expanded);
    if as_written.is_file() {
        return Some(as_written.to_path_buf());
    }
    let mut dirs: Vec<PathBuf> = s.extras.references.search.iter().rev().cloned().collect();
    if let Some(d) = s.scene_path.as_deref().and_then(|p| Path::new(p).parent()) {
        dirs.push(d.to_path_buf());
    }
    if let Some(p) = crate::shading::project_dir(s) {
        dirs.push(PathBuf::from(&p).join("scenes"));
        dirs.push(PathBuf::from(p));
    }
    if !as_written.is_absolute() {
        for d in &dirs {
            let p = d.join(as_written);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let name = expanded.rsplit('/').next().filter(|n| !n.is_empty())?;
    dirs.iter().map(|d| d.join(name)).find(|p| p.is_file())
}

// ---------------------------------------------------------------------------------------------------------------------
// Edits as text

/// `.ed` tokens to sections: reference node, then edits. A token list is: owner, then (node, count, edits...)...
pub(crate) fn parse_edits(tok: &[String]) -> (Vec<(String, Vec<RefEdit>)>, Vec<String>) {
    let arity = |kind: i32| match kind {
        0 | 2 => Some(3),
        1 => Some(4),
        3 | 7 | 8 | 9 => Some(2),
        5 => Some(5),
        _ => None,
    };
    let mut out: Vec<(String, Vec<RefEdit>)> = Vec::new();
    let mut warnings = Vec::new();
    let mut i = 1;
    while i + 1 < tok.len() {
        let name = tok[i].clone();
        let count: usize = tok[i + 1].parse().unwrap_or(0);
        i += 2;
        let mut list = Vec::with_capacity(count);
        for _ in 0..count {
            let Some(kind) = tok.get(i).and_then(|k| k.parse::<i32>().ok()) else { break };
            let Some(n) = arity(kind) else {
                warnings.push(format!("{name}: reference edit of unknown kind {kind}; the rest were kept out"));
                i = tok.len();
                break;
            };
            let args: Vec<String> = tok[i + 1..(i + 1 + n).min(tok.len())].to_vec();
            i += 1 + n;
            let mut e = RefEdit::new(kind, args);
            if kind == 2 {
                let text = e.args.get(2).map(String::as_str).unwrap_or("");
                let flags_only = crate::ma_file::edit_is_flags_only(text);
                e.opaque = flags_only && !text.contains("-av") && !text.contains("-alteredValue") && !text.contains("-l ");
                if flags_only && !e.opaque && !text.contains("-l ") {
                    // -av alone only says the value was changed; the value has its own edit.
                    continue;
                }
            }
            list.push(e);
        }
        match out.iter_mut().find(|x| x.0 == name) {
            Some(x) => x.1.extend(list),
            None => out.push((name, list)),
        }
    }
    (out, warnings)
}

fn same_value(a: &PlugValue, b: &PlugValue) -> bool {
    match (a, b) {
        (PlugValue::Bool(x), PlugValue::Bool(y)) => x == y,
        (PlugValue::Int(x), PlugValue::Int(y)) => x == y,
        (PlugValue::Float(x), PlugValue::Float(y)) => (x - y).abs() <= 1e-9,
        (PlugValue::Vec3(x), PlugValue::Vec3(y)) => (*x - *y).abs().max_element() <= 1e-9,
        (PlugValue::Matrix(x), PlugValue::Matrix(y)) => x.abs_diff_eq(*y, 1e-9),
        (PlugValue::String(x), PlugValue::String(y)) => x == y,
        _ => true,
    }
}

fn escape(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n")
}

/// A value as setAttr's arguments, with the leading space the reference gives them.
fn value_text(a: &crate::plug::AttrDef, v: &PlugValue) -> Option<String> {
    Some(match v {
        PlugValue::Bool(x) => format!(" {}", *x as i32),
        PlugValue::Int(x) => format!(" {x}"),
        PlugValue::Float(x) => format!(" {}", aztek_ops::units::to_user(a.unit, *x)),
        PlugValue::Vec3(x) => {
            let ty = if a.has(flags::COLOR) { "float3" } else { "double3" };
            let u = |c: f64| aztek_ops::units::to_user(a.unit, c);
            format!(" -type \"{ty}\" {} {} {}", u(x.x), u(x.y), u(x.z))
        }
        PlugValue::String(x) => format!(" -type \"string\" \"{}\"", escape(x)),
        PlugValue::Matrix(m) => format!(" -type \"matrix\" {}", m.to_cols_array().iter().map(|c| c.to_string()).collect::<Vec<_>>().join(" ")),
        _ => return None,
    })
}

fn compared(a: &crate::plug::AttrDef) -> bool {
    a.has(flags::INPUT) && a.has(flags::STORABLE) && !matches!(a.ty, PlugType::Mesh | PlugType::Data)
}

fn attr_name(t: &NodeType, i: usize) -> String {
    let name = &t.attrs[i].name;
    match name.rsplit_once('.') {
        Some((_, leaf)) if t.attr_index(leaf) == Some(i) => leaf.to_string(),
        _ => name.clone(),
    }
}

fn add_attr_flags(a: &crate::plug::AttrDef) -> String {
    let mut out = String::from(" -ci 1");
    if a.has(flags::KEYABLE) {
        out.push_str(" -k 1");
    }
    match &a.default_value {
        PlugValue::Float(v) if *v != 0.0 => out.push_str(&format!(" -dv {v}")),
        PlugValue::Int(v) if *v != 0 => out.push_str(&format!(" -dv {v}")),
        PlugValue::Bool(true) => out.push_str(" -dv 1"),
        _ => {}
    }
    if let Some(v) = a.min_value {
        out.push_str(&format!(" -min {v}"));
    }
    if let Some(v) = a.max_value {
        out.push_str(&format!(" -max {v}"));
    }
    match a.ty {
        PlugType::Float => out.push_str(" -at \"double\""),
        PlugType::Int => out.push_str(" -at \"long\""),
        PlugType::Bool => out.push_str(" -at \"bool\""),
        PlugType::Enum => out.push_str(&format!(" -at \"enum\" -en \"{}\"", a.enum_names.join(":"))),
        PlugType::Vec3 => out.push_str(" -at \"double3\""),
        _ => out.push_str(" -dt \"string\""),
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// The scene against what the file gave

/// A connection as compared: a set's member slot moves when another member comes or goes, so which slot doesn't count.
fn conn_key(g: &Graph, src: PlugRef, dst: PlugRef) -> (PlugRef, PlugRef) {
    let slot = g.node(dst.node).is_some_and(|n| dst.attr >= 0 && matches!(n.ty().attrs[dst.attr as usize].name.as_str(), "dagSetMembers" | "dnSetMembers"));
    (src, if slot { PlugRef { element: -1, ..dst } } else { dst })
}

/// Nodes a file holds that belong to the scene: every scene has just one.
fn is_shared_node(g: &Graph, n: NodeId) -> bool {
    g.node(n).is_some_and(|x| matches!(x.ty().name.as_str(), "ikSystem" | "time" | "displayLayerManager" | "renderLayerManager" | "lightLinker"))
}

fn nodes_of(g: &Graph, id: u32) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&n| g.node(n).is_some_and(|x| x.reference_id() == id && x.ty().name != REF_NODE_TYPE)).collect()
}

fn snapshot(g: &mut Graph, id: u32) -> Baseline {
    g.evaluate();
    let ids = nodes_of(g, id);
    let set: HashSet<NodeId> = ids.iter().copied().collect();
    let mut b = Baseline::default();
    for &n in &ids {
        let node = g.node(n).unwrap();
        let t = node.type_arc().clone();
        let mut own = HashSet::new();
        for (i, a) in t.attrs.iter().enumerate() {
            if a.has(flags::USER) {
                own.insert(a.name.clone());
            }
            if !compared(a) {
                continue;
            }
            if a.is_array() {
                for (el, v) in node.elements(i) {
                    b.values.insert((n, i as u32, *el), v.clone());
                }
            } else {
                b.values.insert((n, i as u32, -1), node.value(i).clone());
            }
            let m = node.lock_mask(i);
            if m != 0 {
                b.locks.insert((n, i as u32), m);
            }
        }
        b.attrs.insert(n, own);
        if t.dag {
            b.parents.insert(n, g.parent(n));
        }
    }
    for (dst, src) in g.connections() {
        if set.contains(&dst.node) || set.contains(&src.node) {
            b.conns.insert(conn_key(g, src, dst));
        }
    }
    b
}

/// What differs on a loaded reference from what the file gave, plus edits kept for not being able to be shown that way.
fn gather(s: &mut Session, at: usize) -> (Vec<RefEdit>, Vec<RefConn>) {
    let (id, loaded) = (s.extras.references.entries[at].id, s.extras.references.entries[at].loaded);
    if !loaded {
        let e = &s.extras.references.entries[at];
        return (e.edits.clone(), e.connections.clone());
    }
    s.graph.evaluate();
    let g = &s.graph;
    let e = &s.extras.references.entries[at];
    let ids = nodes_of(g, id);
    let set: HashSet<NodeId> = ids.iter().copied().collect();
    let all: Vec<(PlugRef, PlugRef)> = g.connections().into_iter().map(|(dst, src)| (src, dst)).collect();
    let live: HashSet<(PlugRef, PlugRef)> = all.iter().map(|&(src, dst)| conn_key(g, src, dst)).collect();
    let (mut parents, mut adds, mut sets, mut disconnects, mut deletes, mut locks) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for &n in &ids {
        let node = g.node(n).unwrap();
        let t = node.type_arc().clone();
        let path = edit_path(g, n);
        if let Some(&was) = e.baseline.parents.get(&n) {
            let now = g.parent(n);
            if now != was {
                let to = if now == NO_NODE { String::new() } else { g.full_path(now) };
                parents.push(RefEdit::new(0, vec![path.clone(), to, "-s -r ".to_string()]));
            }
        }
        let empty = HashSet::new();
        let own = e.baseline.attrs.get(&n).unwrap_or(&empty);
        for (i, a) in t.attrs.iter().enumerate() {
            let is_new = a.has(flags::USER) && !own.contains(&a.name);
            if is_new {
                adds.push(RefEdit::new(1, vec![path.clone(), a.name.clone(), if a.short_name.is_empty() { a.name.clone() } else { a.short_name.clone() }, add_attr_flags(a)]));
            }
            // A constraint's constraintPosition and the like are what it worked out, not settings.
            if !compared(a) || a.name.starts_with("constraint") {
                continue;
            }
            let name = attr_name(&t, i);
            let mut one = |el: i32, v: &PlugValue| {
                let plug = PlugRef { node: n, attr: i as i32, component: -1, element: el };
                if g.source(&plug).is_some() {
                    return;
                }
                // Part of a compound driven: its value moves with that, and the other parts rarely get edited on their own.
                if matches!(v, PlugValue::Vec3(_)) && (0..3).any(|c| g.source(&PlugRef { component: c, ..plug }).is_some()) {
                    return;
                }
                let changed = match e.baseline.values.get(&(n, i as u32, el)) {
                    Some(b) => !same_value(b, v),
                    None => is_new || !same_value(&a.default_value, v),
                };
                if changed {
                    if let Some(text) = value_text(a, v) {
                        let attr = if el >= 0 { format!("{name}[{el}]") } else { name.clone() };
                        sets.push(RefEdit::new(2, vec![path.clone(), attr, text]));
                    }
                }
            };
            if a.is_array() {
                for (el, v) in node.elements(i) {
                    one(*el, v);
                }
            } else {
                one(-1, node.value(i));
            }
            let (now, was) = (node.lock_mask(i), e.baseline.locks.get(&(n, i as u32)).copied().unwrap_or(0));
            if now != was {
                locks.push(RefEdit::new(if now != 0 { 8 } else { 9 }, vec![path.clone(), name.clone()]));
            }
        }
        for gone in own.iter().filter(|x| t.attr_index(x).is_none()) {
            deletes.push(RefEdit::new(7, vec![path.clone(), gone.clone()]));
        }
    }
    let mut conns = Vec::new();
    for (src, dst) in &all {
        if !(set.contains(&src.node) || set.contains(&dst.node)) || e.baseline.conns.contains(&conn_key(g, *src, *dst)) {
            continue;
        }
        let referenced = |n: NodeId| g.node(n).is_some_and(|x| x.reference_id() != 0);
        conns.push(RefConn { src: plug_full_name(g, src), dst: plug_full_name(g, dst), src_ref: referenced(src.node), dst_ref: referenced(dst.node), failed: false });
    }
    for (src, dst) in &e.baseline.conns {
        // Leaving a shading group is said by joining another; a shared group's member list is rebuilt on every read.
        let member = g.node(dst.node).is_some_and(|n| dst.attr >= 0 && n.ty().attrs[dst.attr as usize].name == "dagSetMembers");
        if !member && !live.contains(&(*src, *dst)) && g.node(src.node).is_some() && g.node(dst.node).is_some() {
            disconnects.push(RefEdit::new(3, vec![plug_full_name(g, src), plug_full_name(g, dst)]));
        }
    }
    for list in [&mut parents, &mut adds, &mut sets, &mut disconnects, &mut deletes, &mut locks] {
        list.sort_by(|a, b| a.args.cmp(&b.args));
    }
    conns.sort_by(|a, b| (&a.src, &a.dst).cmp(&(&b.src, &b.dst)));
    let mut out = Vec::new();
    for list in [parents, adds, sets, disconnects, deletes, locks] {
        out.extend(list);
    }
    // The reference's own -k and -cb, and whatever hasn't been able to go in yet, stay.
    out.extend(e.edits.iter().filter(|x| x.failed || x.opaque).cloned());
    conns.extend(e.connections.iter().filter(|c| c.failed).cloned());
    (out, conns)
}

// ---------------------------------------------------------------------------------------------------------------------
// Applying

fn apply_edit(s: &mut Session, e: &mut RefEdit) {
    e.failed = !apply_one(s, e);
}

fn apply_one(s: &mut Session, e: &mut RefEdit) -> bool {
    let at = |i: usize| e.args.get(i).cloned().unwrap_or_default();
    match e.kind {
        0 => {
            let g = &mut s.graph;
            let Some(child) = find_node(g, &at(0)) else { return false };
            let parent = if at(1).is_empty() || at(1) == "|world" {
                NO_NODE
            } else {
                match find_node(g, &at(1)) {
                    Some(p) => p,
                    None => return false,
                }
            };
            g.parent(child) == parent || g.set_parent(child, parent, false, usize::MAX).is_ok()
        }
        1 => {
            let Some(n) = find_node(&s.graph, &at(0)) else { return false };
            crate::ma_file::add_attr_edit(&mut s.graph, n, &at(1), &at(2), &at(3))
        }
        2 => {
            let Some(n) = find_node(&s.graph, &at(0)) else { return false };
            let path = s.graph.path(n);
            let Ok(p) = s.graph.plug(&format!("{path}.{}", at(1))) else { return false };
            let r = crate::ma_file::set_attr_edit(&mut s.graph, p, &at(2));
            if r == crate::ma_file::EditResult::NotRead {
                e.opaque = true;
                return true;
            }
            r == crate::ma_file::EditResult::Done
        }
        3 => {
            let g = &mut s.graph;
            let (Some(a), Some(b)) = (find_plug(g, &at(0)), find_plug(g, &at(1))) else { return false };
            if g.source(&b) == Some(a) {
                g.disconnect(&b);
            }
            true
        }
        7 => {
            let Some(p) = find_node(&s.graph, &at(0)).and_then(|n| s.graph.plug(&format!("{}.{}", s.graph.path(n), at(1))).ok()) else { return false };
            s.graph.delete_attr(&p).is_ok()
        }
        8 | 9 => {
            let Some(p) = find_node(&s.graph, &at(0)).and_then(|n| s.graph.plug(&format!("{}.{}", s.graph.path(n), at(1))).ok()) else { return false };
            s.graph.set_locked(&p, e.kind == 8).is_ok()
        }
        _ => false,
    }
}

fn apply_conn(s: &mut Session, c: &mut RefConn) {
    // A shading group's member: the group has its own way in.
    if let Some((set, attr)) = c.dst.rsplit_once('.') {
        if matches!(attr.split('[').next().unwrap_or(attr), "dsm" | "dagSetMembers") {
            let member = c.src.split('.').next().unwrap_or(&c.src).to_string();
            let set = set.trim_start_matches(':').to_string();
            c.failed = s.run("sets", json!({"edit": true, "forceElement": set, "args": [member]})).is_err();
            return;
        }
    }
    let g = &mut s.graph;
    c.failed = match (find_plug(g, &c.src), find_plug(g, &c.dst)) {
        (Some(a), Some(b)) => g.connect(&a, &b, true).is_err(),
        _ => true,
    };
}

fn children_of(s: &Session, id: u32) -> Vec<usize> {
    s.extras.references.entries.iter().enumerate().filter(|(_, e)| e.parent == id).map(|(i, _)| i).collect()
}

fn at_of(s: &Session, id: u32) -> Option<usize> {
    s.extras.references.entries.iter().position(|e| e.id == id)
}

/// A reference and the ones below it, top first.
fn subtree(s: &Session, id: u32) -> Vec<u32> {
    let mut out = vec![id];
    let mut i = 0;
    while i < out.len() {
        let here = out[i];
        out.extend(s.extras.references.entries.iter().filter(|e| e.parent == here).map(|e| e.id));
        i += 1;
    }
    out
}

/// What the referencing files keep on a reference and those below it, inner first, then what the file gave is the baseline.
fn finish(s: &mut Session, id: u32) {
    let ids = subtree(s, id);
    for &r in ids.iter().rev() {
        let Some(at) = at_of(s, r) else { continue };
        if !s.extras.references.entries[at].loaded {
            continue;
        }
        let mut list = std::mem::take(&mut s.extras.references.entries[at].file_edits);
        for e in &mut list {
            apply_edit(s, e);
        }
        let mut conns = std::mem::take(&mut s.extras.references.entries[at].file_conns);
        for c in &mut conns {
            apply_conn(s, c);
        }
        let e = &mut s.extras.references.entries[at];
        e.file_edits = list;
        e.file_conns = conns;
    }
    for &r in &ids {
        let Some(at) = at_of(s, r) else { continue };
        if s.extras.references.entries[at].loaded {
            let b = snapshot(&mut s.graph, r);
            s.extras.references.entries[at].baseline = b;
        }
    }
}

/// This scene's edits on a reference and those below it.
fn apply_scene_edits(s: &mut Session, id: u32) {
    let was = s.graph.undo_stack().enabled();
    s.graph.undo_stack_mut().set_enabled(false);
    for r in subtree(s, id) {
        let Some(at) = at_of(s, r) else { continue };
        if !s.extras.references.entries[at].loaded {
            continue;
        }
        let mut list = std::mem::take(&mut s.extras.references.entries[at].edits);
        for e in &mut list {
            apply_edit(s, e);
        }
        s.extras.references.entries[at].edits = list;
    }
    for r in subtree(s, id) {
        let Some(at) = at_of(s, r) else { continue };
        if !s.extras.references.entries[at].loaded {
            continue;
        }
        let mut conns = std::mem::take(&mut s.extras.references.entries[at].connections);
        for c in &mut conns {
            apply_conn(s, c);
        }
        s.extras.references.entries[at].connections = conns;
    }
    s.graph.undo_stack_mut().set_enabled(was);
}

// ---------------------------------------------------------------------------------------------------------------------
// Loading

/// Reads a reference's file into the scene and brings in what's kept on it; `scene_edits` when the scene's own nodes are
/// there for its edits to reach. Those wait when the scene's file is still being read.
fn load(s: &mut Session, at: usize, scene_edits: bool) -> Result<Vec<String>, String> {
    if s.extras.references.depth > 16 {
        return Err("references nested too deep".to_string());
    }
    let (id, written, ns, prefix, rfn, locked) = {
        let e = &s.extras.references.entries[at];
        (e.id, e.unresolved.clone(), e.namespace.clone(), e.prefix.clone(), e.rfn.clone(), e.locked)
    };
    if s.extras.references.stack.contains(&id) {
        return Err(format!("{written} references itself"));
    }
    let Some(found) = resolve(s, &written) else { return Err(format!("could not find {written}")) };
    let path = found.to_string_lossy().replace('\\', "/");
    {
        let t = &mut s.extras.references;
        if let Some(dir) = found.parent().map(Path::to_path_buf) {
            if !t.search.contains(&dir) {
                t.search.push(dir.clone());
            }
            crate::shading::add_search_folder(&dir);
        }
        t.depth += 1;
        t.stack.push(id);
    }
    // The default nodes the file's will fall on are the scene's, and don't go in with the reference.
    crate::cameras::make_defaults(&mut s.graph);
    let before: HashSet<NodeId> = s.graph.nodes().into_iter().collect();
    let sel: Vec<NodeId> = s.graph.selection().to_vec();
    let was = s.graph.undo_stack().enabled();
    s.graph.undo_stack_mut().set_enabled(false);
    // A file's own time settings and the like don't change the scene's.
    let r = crate::ma_file::import_scene(s, &path);
    {
        let t = &mut s.extras.references;
        t.depth -= 1;
        t.stack.pop();
    }
    let result = r.map(|warnings| {
        let g = &mut s.graph;
        if !ns.is_empty() {
            g.set_namespace_known(&ns, true);
        }
        let new: Vec<NodeId> = g.nodes().into_iter().filter(|n| !before.contains(n) && g.node(*n).is_some_and(|x| x.reference_id() == 0) && !is_shared_node(g, *n)).collect();
        for &n in &new {
            let name = g.node(n).unwrap().name().to_string();
            if !ns.is_empty() {
                let _ = g.rename(n, &format!("{ns}:{name}"));
            } else if !prefix.is_empty() {
                let _ = g.rename(n, &format!("{prefix}_{name}"));
            }
            g.set_reference(n, id);
        }
        if g.find(&rfn).ok().flatten().is_none() {
            make_reference_node(g, &rfn, id, &found);
        }
        s.extras.references.last_new = new;
        let e = &mut s.extras.references.entries[at];
        e.path = path;
        e.loaded = true;
        warnings
    });
    if result.is_ok() && s.extras.references.depth == 0 {
        finish(s, id);
        if locked {
            lock_contents(s, id);
        }
        if scene_edits {
            apply_scene_edits(s, id);
        } else {
            s.extras.references.fresh.push(id);
        }
    }
    s.graph.select(&sel, crate::graph::SelectMode::Replace);
    s.graph.undo_stack_mut().set_enabled(was);
    result
}

fn make_reference_node(g: &mut Graph, rfn: &str, id: u32, file: &Path) {
    let Some(t) = types::find(REF_NODE_TYPE) else { return };
    if let Ok(n) = g.create_node(&t, rfn, NO_NODE, false) {
        g.set_reference(n, id);
        g.set_node_locked(n, true);
        if let Ok(p) = g.plug(&format!("{rfn}.fileNames[0]")) {
            let _ = g.set_value(p, PlugValue::String(file.to_string_lossy().to_string()));
        }
    }
}

fn lock_contents(s: &mut Session, id: u32) {
    let g = &mut s.graph;
    for n in nodes_of(g, id) {
        let t = g.node(n).unwrap().type_arc().clone();
        for (i, a) in t.attrs.iter().enumerate() {
            if compared(a) && !a.is_array() {
                let _ = g.set_locked(&PlugRef::new(n, i as i32), true);
            }
        }
        g.set_node_locked(n, true);
    }
    if let Some(at) = at_of(s, id) {
        let b = snapshot(&mut s.graph, id);
        s.extras.references.entries[at].baseline = b;
    }
}

/// Takes a reference's nodes out, those below it too, keeping what was done to them.
fn unload(s: &mut Session, at: usize) {
    let id = s.extras.references.entries[at].id;
    // Inner ones go first; those below keep their edits and load state under their names for when this one is back.
    for c in children_of(s, id).into_iter().map(|i| s.extras.references.entries[i].id).collect::<Vec<_>>() {
        let Some(ci) = at_of(s, c) else { continue };
        let was_loaded = s.extras.references.entries[ci].loaded;
        unload(s, ci);
        let Some(ci) = at_of(s, c) else { continue };
        let child = s.extras.references.entries.remove(ci);
        let t = &mut s.extras.references;
        t.load_state.insert(child.rfn.clone(), !was_loaded);
        t.pending.insert(child.rfn.clone(), child.edits);
        t.pending_conns.insert(child.rfn.clone(), child.connections);
        let g = &mut s.graph;
        if let Ok(Some(n)) = g.find(&child.rfn) {
            g.set_node_locked(n, false);
            let _ = g.without_undo(|g| g.delete_node(n));
        }
    }
    let Some(at) = at_of(s, id) else { return };
    let (edits, conns) = gather(s, at);
    let g = &mut s.graph;
    let was = g.undo_stack().enabled();
    g.undo_stack_mut().set_enabled(false);
    let ids = nodes_of(g, id);
    // The scene's own nodes parented under referenced ones wait at the world; what referenced nodes hold aren't deleted.
    let doomed: HashSet<NodeId> = ids.iter().copied().collect();
    for &n in &ids {
        if g.node(n).is_none() {
            continue;
        }
        for c in g.children(n).to_vec() {
            if !doomed.contains(&c) {
                let _ = g.set_parent(c, NO_NODE, false, usize::MAX);
            }
        }
    }
    for &n in &ids {
        if g.node(n).is_some() {
            g.set_node_locked(n, false);
            let _ = g.delete_node(n);
        }
    }
    g.undo_stack_mut().set_enabled(was);
    let e = &mut s.extras.references.entries[at];
    e.edits = edits;
    e.connections = conns;
    e.loaded = false;
    e.baseline = Baseline::default();
}

// ---------------------------------------------------------------------------------------------------------------------
// The table

fn add_entry(s: &mut Session, r: &FileRef) -> usize {
    let t = &mut s.extras.references;
    t.next += 1;
    let id = t.next;
    let parent = t.stack.last().copied().unwrap_or(0);
    let above = t.entries.iter().find(|e| e.id == parent).map(|e| e.namespace.clone()).unwrap_or_default();
    let join = |name: &str| if above.is_empty() { name.to_string() } else { format!("{above}:{name}") };
    let stem = Path::new(&r.path.replace('\\', "/")).file_stem().and_then(|x| x.to_str()).unwrap_or("reference").to_string();
    let root = r.ns == ":";
    let wanted = r.ns.trim_start_matches(':').to_string();
    let ns = if r.rpr.is_some() || root {
        above.clone()
    } else if wanted.is_empty() {
        join(&stem)
    } else {
        join(&wanted)
    };
    // A namespace already taken takes a number, as a second reference of a file does; a file's own stays as it wrote it.
    let mut unique = ns.clone();
    let mut k = 1;
    while !unique.is_empty() && !r.merge && r.rpr.is_none() && !root && t.entries.iter().any(|e| e.namespace == unique) {
        unique = format!("{ns}{k}");
        k += 1;
    }
    let mut rfn = match &r.rfn {
        Some(n) => join(n),
        None if unique.is_empty() => join(&format!("{stem}RN")),
        None => format!("{unique}RN"),
    };
    let base = rfn.clone();
    let mut k = 1;
    while t.entries.iter().any(|e| e.rfn == rfn) {
        rfn = format!("{base}{k}");
        k += 1;
    }
    let e = RefEntry {
        id,
        unresolved: r.path.clone(),
        path: r.path.clone(),
        namespace: unique,
        prefix: r.rpr.clone().unwrap_or_default(),
        edits: t.pending.remove(&rfn).unwrap_or_default(),
        connections: t.pending_conns.remove(&rfn).unwrap_or_default(),
        file_edits: t.pending_file.remove(&rfn).unwrap_or_default(),
        rfn,
        parent,
        locked: r.locked,
        file_type: r.file_type.clone(),
        options: r.options.clone(),
        ..Default::default()
    };
    t.entries.push(e);
    t.entries.len() - 1
}

/// `file -r`: the path as the reference returns it, with a copy number from the second reference of a file on.
pub fn create(s: &mut Session, path: &str, ns: &str, rfn: Option<&str>, deferred: bool) -> Result<String, String> {
    create_with(s, &FileRef { path: path.to_string(), ns: ns.to_string(), rfn: rfn.map(str::to_string), deferred, ..Default::default() }).map(|at| path_with_copy(s, at))
}

/// `file -r` with its flags.
pub(crate) fn file_reference(s: &mut Session, a: &Args, path: &str) -> CmdResult {
    let text = |flag: &str| a.get(flag).and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string);
    let r = FileRef {
        path: path.to_string(),
        ns: a.string("namespace", ""),
        rfn: text("referenceNode"),
        deferred: a.bool("deferReference"),
        rpr: text("renamingPrefix"),
        file_type: a.get("type").and_then(Value::as_str).unwrap_or("").to_string(),
        options: text("options").unwrap_or_default(),
        locked: a.bool("lockReference"),
        merge: a.bool("mergeNamespacesOnClash"),
    };
    s.extras.references.last_new.clear();
    let at = create_with(s, &r).map_err(CommandError::new)?;
    if a.bool("groupReference") || a.get("groupName").is_some() {
        group_contents(s, at, text("groupName"), a.bool("groupLocator"))?;
    }
    s.graph.undo_stack_mut().clear();
    Ok(if a.bool("returnNewNodes") { new_nodes(s) } else { json!(path_with_copy(s, at)) })
}

/// -groupReference: the reference's top nodes under one transform of the scene's, as parent edits.
fn group_contents(s: &mut Session, at: usize, name: Option<String>, locator: bool) -> CmdResult<()> {
    let (id, rfn) = (s.extras.references.entries[at].id, s.extras.references.entries[at].rfn.clone());
    let name = name.unwrap_or_else(|| format!("{rfn}group"));
    let made = s.run(if locator { "spaceLocator" } else { "group" }, if locator { json!({"name": name}) } else { json!({"empty": true, "name": name}) })?;
    let g = &mut s.graph;
    let holder = made.as_array().and_then(|l| l.first()).and_then(Value::as_str).map(str::to_string).or_else(|| made.as_str().map(str::to_string)).and_then(|n| g.find(&n).ok().flatten());
    let Some(holder) = holder else { fail!("file: couldn't make the group {name}") };
    let tops: Vec<NodeId> = nodes_of(g, id).into_iter().filter(|&n| g.node(n).is_some_and(|x| x.ty().dag) && g.parent(n) == NO_NODE).collect();
    for n in tops {
        g.set_parent(n, holder, false, usize::MAX).map_err(|e| CommandError::new(e.to_string()))?;
    }
    Ok(())
}

fn create_with(s: &mut Session, r: &FileRef) -> Result<usize, String> {
    if resolve(s, &r.path).is_none() {
        return Err(format!("file: {} not found", r.path));
    }
    let at = add_entry(s, r);
    let found = resolve(s, &r.path);
    if let Some(p) = found {
        s.extras.references.entries[at].path = p.to_string_lossy().replace('\\', "/");
    }
    if r.deferred {
        let e = &s.extras.references.entries[at];
        let (rfn, id) = (e.rfn.clone(), e.id);
        let file = PathBuf::from(&e.path);
        s.graph.without_undo(|g| make_reference_node(g, &rfn, id, &file));
    } else {
        // One evaluation at the end rather than one per constraint.
        s.graph.begin_batch();
        let r = load(s, at, true);
        s.graph.end_batch();
        if let Err(e) = r {
            s.extras.references.entries.remove(at);
            return Err(e);
        }
    }
    s.graph.undo_stack_mut().clear();
    Ok(at)
}

/// A reference by its path with a copy number, reference node, or namespace.
fn entry_of(s: &Session, name: &str) -> Option<usize> {
    let t = &s.extras.references.entries;
    if let Some((p, n)) = name.strip_suffix('}').and_then(|x| x.rsplit_once('{')) {
        let k: usize = n.parse().ok()?;
        return t.iter().enumerate().filter(|(_, e)| e.path == p || e.unresolved == p).nth(k).map(|(i, _)| i);
    }
    let short = |e: &RefEntry| Path::new(&e.path).file_name().and_then(|x| x.to_str()).map(str::to_string);
    t.iter().position(|e| e.rfn == name).or_else(|| t.iter().position(|e| e.path == name || e.unresolved == name)).or_else(|| t.iter().position(|e| !e.namespace.is_empty() && e.namespace == name.trim_start_matches(':'))).or_else(|| t.iter().position(|e| short(e).as_deref() == Some(name)))
}

/// The entry a node belongs to.
fn entry_of_node(s: &Session, name: &str) -> Option<usize> {
    let g = &s.graph;
    let obj = name.split('.').next().unwrap_or(name);
    let id = g.find(obj).ok().flatten()?;
    let rid = g.node(id)?.reference_id();
    if rid == 0 {
        return None;
    }
    at_of(s, rid)
}

fn path_with_copy(s: &Session, at: usize) -> String {
    let t = &s.extras.references.entries;
    let path = &t[at].path;
    let k = t[..at].iter().filter(|e| e.path == *path).count();
    if k == 0 {
        path.clone()
    } else {
        format!("{path}{{{k}}}")
    }
}

/// The name a query gives for a file, by the flags that shape it.
fn file_name_as_asked(s: &Session, at: usize, a: &Args) -> String {
    let e = &s.extras.references.entries[at];
    if asked(a, "shortName") {
        return Path::new(&e.path).file_name().and_then(|x| x.to_str()).unwrap_or("").to_string();
    }
    if asked(a, "unresolvedName") {
        return e.unresolved.clone();
    }
    if asked(a, "withoutCopyNumber") {
        return e.path.clone();
    }
    path_with_copy(s, at)
}

fn top_of(s: &Session, at: usize) -> usize {
    let mut i = at;
    while s.extras.references.entries[i].parent != 0 {
        match at_of(s, s.extras.references.entries[i].parent) {
            Some(p) => i = p,
            None => break,
        }
    }
    i
}

fn flush(s: &mut Session) {
    s.graph.undo_stack_mut().clear();
    s.graph.evaluate();
}

fn remove_entry(s: &mut Session, at: usize) -> Value {
    unload(s, at);
    let e = s.extras.references.entries.remove(at);
    let g = &mut s.graph;
    let was = g.undo_stack().enabled();
    g.undo_stack_mut().set_enabled(false);
    if let Ok(Some(n)) = g.find(&e.rfn) {
        g.set_node_locked(n, false);
        let _ = g.delete_node(n);
    }
    let used = !e.namespace.is_empty() && (s.extras.references.entries.iter().any(|x| x.namespace == e.namespace) || g.nodes().iter().any(|&n| g.node(n).is_some_and(|x| x.name().starts_with(&format!("{}:", e.namespace)))));
    if !used && !e.namespace.is_empty() {
        g.set_namespace_known(&e.namespace, false);
    }
    g.undo_stack_mut().set_enabled(was);
    json!(e.path)
}

/// The entry named by a flag's value, -rfn or the file argument.
fn named(s: &Session, a: &Args, flag: &str) -> Option<CmdResult<usize>> {
    let v = a.get(flag)?;
    let positional = a.pos_str(0).map(str::to_string);
    let name = v.as_str().map(str::to_string).filter(|n| !n.is_empty()).or_else(|| a.get("referenceNode").and_then(Value::as_str).map(str::to_string)).or_else(|| positional.clone()).unwrap_or_default();
    Some(entry_of(s, &name).ok_or_else(|| CommandError::new(format!("no reference {name}"))))
}

fn new_nodes(s: &Session) -> Value {
    let g = &s.graph;
    Value::Array(s.extras.references.last_new.iter().filter(|&&n| g.node(n).is_some()).map(|&n| json!(g.path(n))).collect())
}

/// Load, unload, remove and the rest of the flags of `file` and `reference`. None when none was asked for.
pub fn file_flags(s: &mut Session, a: &Args) -> Option<CmdResult> {
    let positional = a.pos_str(0).map(str::to_string);
    for flag in ["unloadReference", "flushReference"] {
        if let Some(at) = named(s, a, flag) {
            return Some(at.map(|at| {
                unload(s, at);
                flush(s);
                json!(path_with_copy(s, at))
            }));
        }
    }
    if let Some(at) = named(s, a, "loadReference").or_else(|| named(s, a, "loadReferencePreview")) {
        return Some(at.and_then(|at| {
            if s.extras.references.entries[at].loaded {
                unload(s, at);
            }
            // With a file as well, -lr replaces what the reference points at; its edits stay.
            if let Some(p) = positional.as_deref().filter(|p| entry_of(s, p) != Some(at)) {
                if resolve(s, p).is_none() {
                    fail!("file: {p} not found");
                }
                let e = &mut s.extras.references.entries[at];
                e.unresolved = p.to_string();
                e.path = p.to_string();
            }
            load(s, at, true).map_err(CommandError::new)?;
            flush(s);
            Ok(if a.bool("returnNewNodes") { new_nodes(s) } else { json!(path_with_copy(s, at)) })
        }));
    }
    if let Some(at) = named(s, a, "cleanReference") {
        return Some(at.and_then(|at| {
            if s.extras.references.entries[at].loaded {
                fail!("file: {} has to be unloaded to clean its edits", s.extras.references.entries[at].rfn);
            }
            let kinds: Vec<String> = a.get("editCommand").map(|v| v.as_array().cloned().unwrap_or_else(|| vec![v.clone()])).unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_string)).collect();
            let e = &mut s.extras.references.entries[at];
            e.edits.retain(|x| !kinds.is_empty() && !kinds.iter().any(|k| k == x.command()));
            if kinds.is_empty() || kinds.iter().any(|k| k == "connectAttr") {
                e.connections.clear();
            }
            Ok(Value::Null)
        }));
    }
    if a.bool("importReference") {
        let name = positional.or_else(|| a.get("referenceNode").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        return Some(import_reference(s, &name));
    }
    if a.bool("removeReference") {
        let name = positional.or_else(|| a.get("referenceNode").and_then(Value::as_str).map(str::to_string)).unwrap_or_default();
        return Some(match entry_of(s, &name) {
            Some(at) if s.extras.references.entries[at].parent != 0 => Err(CommandError::new(format!("file: {name} is inside another reference and can't be removed on its own"))),
            Some(at) => {
                let r = remove_entry(s, at);
                flush(s);
                Ok(r)
            }
            None => Err(CommandError::new(format!("no reference {name}"))),
        });
    }
    if a.bool("edit") {
        if let (Some(ns), Some(name)) = (a.get("namespace").and_then(Value::as_str), positional.as_deref().map(str::to_string).or_else(|| a.get("referenceNode").and_then(Value::as_str).map(str::to_string))) {
            return Some(rename_namespace(s, &name, ns));
        }
    }
    None
}

fn import_reference(s: &mut Session, name: &str) -> CmdResult {
    let Some(at) = entry_of(s, name) else { fail!("no reference {name}") };
    if s.extras.references.entries[at].parent != 0 {
        fail!("file: {name} is inside another reference; import that one first");
    }
    if !s.extras.references.entries[at].loaded {
        load(s, at, true).map_err(CommandError::new)?;
    }
    let e = s.extras.references.entries.remove(at);
    for c in s.extras.references.entries.iter_mut().filter(|c| c.parent == e.id) {
        c.parent = 0;
    }
    let g = &mut s.graph;
    let was = g.undo_stack().enabled();
    g.undo_stack_mut().set_enabled(false);
    for n in g.nodes() {
        if g.node(n).is_some_and(|x| x.reference_id() == e.id) {
            g.set_reference(n, 0);
        }
    }
    if let Ok(Some(n)) = g.find(&e.rfn) {
        g.set_node_locked(n, false);
        let _ = g.delete_node(n);
    }
    g.undo_stack_mut().set_enabled(was);
    flush(s);
    Ok(json!(e.path))
}

/// `file -e -ns`: the nodes, the kept edits and those below all move.
fn rename_namespace(s: &mut Session, name: &str, new: &str) -> CmdResult {
    let Some(at) = entry_of(s, name) else { fail!("no reference {name}") };
    let new = new.trim_start_matches(':').to_string();
    if s.extras.references.entries.iter().any(|e| e.namespace == new) {
        fail!("file: the namespace {new} is in use");
    }
    let old = s.extras.references.entries[at].namespace.clone();
    if old.is_empty() {
        fail!("file: {name} uses no namespace");
    }
    let swap = |text: &str| -> String { text.split('|').map(|step| if step == old || step.starts_with(&format!("{old}:")) { format!("{new}{}", &step[old.len()..]) } else { step.to_string() }).collect::<Vec<_>>().join("|") };
    let g = &mut s.graph;
    let was = g.undo_stack().enabled();
    g.undo_stack_mut().set_enabled(false);
    g.set_namespace_known(&new, true);
    for n in g.nodes() {
        let Some(x) = g.node(n) else { continue };
        if x.name().starts_with(&format!("{old}:")) && x.reference_id() != 0 {
            let to = swap(x.name());
            g.set_node_locked(n, false);
            let _ = g.rename(n, &to);
            if x_is_reference_node(g, n) {
                g.set_node_locked(n, true);
            }
        }
    }
    g.set_namespace_known(&old, false);
    g.undo_stack_mut().set_enabled(was);
    for e in &mut s.extras.references.entries {
        e.namespace = if e.namespace == old || e.namespace.starts_with(&format!("{old}:")) { swap(&e.namespace) } else { e.namespace.clone() };
        e.rfn = swap(&e.rfn);
        for x in e.edits.iter_mut().chain(e.file_edits.iter_mut()) {
            let paths = match x.kind {
                0 | 3 => 2,
                1 | 2 | 7 | 8 | 9 => 1,
                _ => 0,
            };
            for a in x.args.iter_mut().take(paths) {
                *a = swap(a);
            }
        }
        for c in e.connections.iter_mut().chain(e.file_conns.iter_mut()) {
            c.src = swap(&c.src);
            c.dst = swap(&c.dst);
        }
    }
    flush(s);
    Ok(json!(new))
}

fn x_is_reference_node(g: &Graph, n: NodeId) -> bool {
    g.node(n).is_some_and(|x| x.ty().name == REF_NODE_TYPE)
}

/// `file -q`: -r the paths, -rfn the reference node, -ns, -dr and the rest.
pub fn file_query(s: &Session, a: &Args, path: Option<&str>) -> Option<Value> {
    if asked(a, "reference") {
        let parent = path.and_then(|p| entry_of(s, p)).map_or(0, |at| s.extras.references.entries[at].id);
        return Some(Value::Array((0..s.extras.references.entries.len()).filter(|&i| s.extras.references.entries[i].parent == parent).map(|i| json!(file_name_as_asked(s, i, a))).collect()));
    }
    let at = path.and_then(|p| entry_of(s, p));
    let e = |at: Option<usize>| at.map(|i| &s.extras.references.entries[i]);
    if asked(a, "referenceNode") {
        return Some(json!(e(at)?.rfn));
    }
    if asked(a, "namespace") {
        return Some(json!(e(at)?.namespace));
    }
    if asked(a, "deferReference") {
        return Some(json!(!e(at)?.loaded));
    }
    if asked(a, "usingNamespaces") {
        return Some(json!(!e(at)?.namespace.is_empty()));
    }
    if asked(a, "renamingPrefix") {
        return Some(json!(e(at)?.prefix));
    }
    if at.is_some() && (asked(a, "unresolvedName") || asked(a, "withoutCopyNumber") || asked(a, "expandName")) {
        return Some(json!(file_name_as_asked(s, at?, a)));
    }
    None
}

// ---------------------------------------------------------------------------------------------------------------------
// referenceQuery and referenceEdit

fn edits_for_query(s: &mut Session, at: usize) -> Vec<RefEdit> {
    let (mut list, conns) = gather(s, at);
    for c in conns {
        let mut e = RefEdit::new(5, vec![c.src, c.dst]);
        e.failed = c.failed;
        list.push(e);
    }
    list
}

fn conn_text(e: &RefEdit) -> String {
    if e.kind == 5 {
        format!("connectAttr \"{}\" \"{}\"", e.args[0], e.args[1])
    } else {
        e.text()
    }
}

fn wanted_kinds(a: &Args) -> Vec<String> {
    a.get("editCommand").map(|v| v.as_array().cloned().unwrap_or_else(|| vec![v.clone()])).unwrap_or_default().iter().filter_map(|x| x.as_str().map(str::to_string)).collect()
}

/// Whether an edit concerns what the argument names: a node, or a node's attribute.
fn edit_matches(g: &Graph, e: &RefEdit, target: &str) -> bool {
    let (node, attr) = match target.split_once('.') {
        Some((n, at)) => (n, Some(at)),
        None => (target, None),
    };
    let wanted = find_node(g, node);
    let sides: Vec<(String, Option<String>)> = if e.kind == 5 {
        e.args.iter().take(2).map(|p| p.rsplit_once('.').map_or((p.clone(), None), |(n, at)| (n.to_string(), Some(at.to_string())))).collect()
    } else {
        e.targets()
    };
    sides.iter().any(|(n, at)| {
        let same = n == node || n.rsplit('|').next() == Some(node) || (wanted.is_some() && find_node(g, n) == wanted);
        same && attr.is_none_or(|w| at.as_deref() == Some(w) || at.as_deref().is_some_and(|x| long_of(g, wanted, w).as_deref() == Some(x)))
    })
}

fn long_of(g: &Graph, n: Option<NodeId>, attr: &str) -> Option<String> {
    let t = g.node(n?)?.type_arc().clone();
    Some(t.attrs[t.attr_index(attr)?].name.clone())
}

fn reference_query(s: &mut Session, a: &Args) -> CmdResult {
    let name = objects(s, a).into_iter().next().unwrap_or_default();
    let by_node = entry_of_node(s, &name).filter(|_| s.graph.find(name.split('.').next().unwrap_or("")).ok().flatten().is_some_and(|n| !x_is_reference_node(&s.graph, n)));
    let is_rfn = s.extras.references.entries.iter().any(|e| e.rfn == name);
    if a.bool("isNodeReferenced") {
        return Ok(json!(by_node.is_some() && !is_rfn));
    }
    let Some(at) = by_node.or_else(|| entry_of(s, &name)) else { fail!("referenceQuery: {name} is not part of a reference") };
    let (id, parent) = (s.extras.references.entries[at].id, s.extras.references.entries[at].parent);
    let related: Option<Vec<usize>> = if a.bool("child") {
        Some(children_of(s, id))
    } else if a.bool("parent") {
        Some(at_of(s, parent).into_iter().collect())
    } else if a.bool("topReference") {
        Some(vec![top_of(s, at)])
    } else {
        None
    };
    let scalar = !a.bool("child");
    if a.bool("filename") {
        let names = |s: &Session, list: &[usize]| list.iter().map(|&i| file_name_as_asked(s, i, a)).collect::<Vec<_>>();
        return Ok(match related {
            Some(list) if scalar => list.first().map_or(Value::Null, |&i| json!(file_name_as_asked(s, i, a))),
            Some(list) => json!(names(s, &list)),
            None => json!(file_name_as_asked(s, at, a)),
        });
    }
    if a.bool("referenceNode") {
        return Ok(match related {
            Some(list) if scalar => list.first().map_or(Value::Null, |&i| json!(s.extras.references.entries[i].rfn)),
            Some(list) => json!(list.iter().map(|&i| s.extras.references.entries[i].rfn.clone()).collect::<Vec<_>>()),
            None => json!(s.extras.references.entries[at].rfn),
        });
    }
    if a.bool("nodes") {
        let g = &s.graph;
        return Ok(Value::Array(nodes_of(g, id).into_iter().map(|n| json!(if a.bool("dagPath") { g.full_path(n) } else { g.path(n) })).collect()));
    }
    let e = &s.extras.references.entries[at];
    if a.bool("namespace") {
        let ns = &e.namespace;
        return Ok(json!(if a.bool("shortName") { ns.rsplit(':').next().unwrap_or("").to_string() } else { format!(":{ns}") }));
    }
    if a.bool("parentNamespace") {
        return Ok(json!(e.namespace.rsplit_once(':').map_or("", |x| x.0)));
    }
    if a.bool("isLoaded") {
        return Ok(json!(e.loaded));
    }
    if a.bool("isExportEdits") || a.bool("isPreviewOnly") {
        return Ok(json!(false));
    }
    if a.bool("editStrings") || a.bool("editNodes") || a.bool("editAttrs") {
        let failed = a.bool("failedEdits");
        let success = a.get("successfulEdits").map_or(!failed, |v| v.as_bool().unwrap_or(true));
        let kinds = wanted_kinds(a);
        let list = edits_for_query(s, at);
        let g = &s.graph;
        let by_rfn = is_rfn || by_node.is_none();
        let chosen: Vec<&RefEdit> = list
            .iter()
            .filter(|e| (e.failed && failed) || (!e.failed && success))
            .filter(|e| kinds.is_empty() || kinds.iter().any(|k| k == e.command()))
            .filter(|e| by_rfn || edit_matches(g, e, &name))
            .collect();
        if a.bool("editStrings") {
            return Ok(Value::Array(chosen.iter().map(|e| json!(conn_text(e))).collect()));
        }
        let mut out = Vec::new();
        for e in chosen {
            let sides = if e.kind == 5 { e.args.iter().take(2).map(|p| p.rsplit_once('.').map_or((p.clone(), None), |(n, at)| (n.to_string(), Some(at.to_string())))).collect() } else { e.targets() };
            for (n, at) in sides {
                out.push(match (a.bool("editNodes"), a.bool("editAttrs")) {
                    (true, true) => json!(format!("{n}.{}", at.unwrap_or_default())),
                    (true, false) => json!(n),
                    _ => json!(at.unwrap_or_default()),
                });
            }
        }
        return Ok(Value::Array(out));
    }
    fail!("referenceQuery: ask for something: -f, -n, -ns, -rfn, -il, -inr, -es, -en or -ea")
}

fn reference_edit(s: &mut Session, a: &Args) -> CmdResult {
    let args: Vec<String> = objects(s, a);
    let target = args.first().cloned().unwrap_or_default();
    let on = a.get("onReferenceNode").and_then(Value::as_str).map(str::to_string);
    let at = on.as_deref().and_then(|n| entry_of(s, n)).or_else(|| entry_of(s, &target)).or_else(|| {
        // An unloaded node has no entry to look up by node: its namespace says.
        let node = target.split('.').next().unwrap_or(&target);
        let ns = node.rsplit('|').next().unwrap_or(node).rsplit_once(':').map(|x| x.0.to_string())?;
        s.extras.references.entries.iter().position(|e| e.namespace == ns)
    });
    let Some(at) = at.or_else(|| entry_of_node(s, &target)) else { fail!("referenceEdit: {target} is not part of a reference") };
    if let Some(v) = a.get("changeEditTarget") {
        let list: Vec<String> = v.as_array().map(|x| x.iter().filter_map(|y| y.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let [old, new] = list.as_slice() else { fail!("referenceEdit: -changeEditTarget takes the old and new targets") };
        let (edits, _) = gather(s, at);
        let swap = |p: &str| {
            let (n, rest) = p.split_once('.').map_or((p, ""), |(n, r)| (n, r));
            let leaf = n.rsplit('|').next().unwrap_or(n);
            if n == old || leaf == old {
                if rest.is_empty() { new.clone() } else { format!("{new}.{rest}") }
            } else {
                p.to_string()
            }
        };
        let e = &mut s.extras.references.entries[at];
        e.edits = edits;
        for x in &mut e.edits {
            let paths = match x.kind {
                0 | 3 => 2,
                1 | 2 | 7 | 8 | 9 => 1,
                _ => 0,
            };
            for p in x.args.iter_mut().take(paths) {
                *p = swap(p);
            }
        }
        return Ok(Value::Null);
    }
    if a.bool("applyFailedEdits") {
        if !s.extras.references.entries[at].loaded {
            fail!("referenceEdit: {} has to be loaded to apply edits", s.extras.references.entries[at].rfn);
        }
        let was = s.graph.undo_stack().enabled();
        s.graph.undo_stack_mut().set_enabled(false);
        let mut list = std::mem::take(&mut s.extras.references.entries[at].edits);
        for e in list.iter_mut().filter(|e| e.failed) {
            apply_edit(s, e);
        }
        s.extras.references.entries[at].edits = list;
        s.graph.undo_stack_mut().set_enabled(was);
        return Ok(Value::Null);
    }
    if a.bool("removeEdits") {
        if s.extras.references.entries[at].loaded {
            fail!("referenceEdit: {} has to be unloaded to remove its edits", s.extras.references.entries[at].rfn);
        }
        let kinds = wanted_kinds(a);
        let by_rfn = entry_of(s, &target).is_some();
        let e = &s.extras.references.entries[at];
        let g = &s.graph;
        let keep: Vec<RefEdit> = e
            .edits
            .iter()
            .filter(|x| {
                let chosen = (kinds.is_empty() || kinds.iter().any(|k| k == x.command())) && (by_rfn || edit_on(x, &target, args.get(1).map(String::as_str), g));
                !chosen
            })
            .cloned()
            .collect();
        let conns: Vec<RefConn> = if kinds.is_empty() || kinds.iter().any(|k| k == "connectAttr") {
            e.connections.iter().filter(|c| !(by_rfn || c.src.contains(&target) || c.dst.contains(&target))).cloned().collect()
        } else {
            e.connections.clone()
        };
        let e = &mut s.extras.references.entries[at];
        e.edits = keep;
        e.connections = conns;
        return Ok(Value::Null);
    }
    fail!("referenceEdit: ask for something: -r, -cet or -afe")
}

fn edit_on(e: &RefEdit, target: &str, other: Option<&str>, _g: &Graph) -> bool {
    let (node, attr) = target.split_once('.').map_or((target, None), |(n, at)| (n, Some(at)));
    let leaf = |p: &str| p.rsplit('|').next().unwrap_or(p).to_string();
    if let Some(o) = other {
        return e.args.first().map(String::as_str).map(leaf).as_deref() == Some(leaf(target).as_str()) && e.args.get(1).map(String::as_str).map(leaf).as_deref() == Some(leaf(o).as_str());
    }
    e.targets().iter().any(|(n, at)| leaf(n) == leaf(node) && attr.is_none_or(|w| at.as_deref() == Some(w)))
}

// ---------------------------------------------------------------------------------------------------------------------
// Files

/// For the app's own format.
pub fn to_doc(s: &mut Session) -> Vec<Value> {
    (0..s.extras.references.entries.len())
        .map(|at| {
            let (edits, conns) = gather(s, at);
            let e = &s.extras.references.entries[at];
            json!({"path": e.unresolved, "namespace": e.namespace, "rfn": e.rfn, "loaded": e.loaded, "parent": e.parent, "prefix": e.prefix,
                   "edits": edits.iter().map(|x| json!([x.kind, x.args, x.failed, x.opaque])).collect::<Vec<_>>(),
                   "connections": conns.iter().map(|c| json!([c.src, c.dst, c.src_ref, c.dst_ref])).collect::<Vec<_>>()})
        })
        .collect()
}

/// What to bring back after a scene's own nodes are read from the app's own format. Those below a top level one come with it.
pub fn restore(s: &mut Session, list: &[Value]) -> Vec<String> {
    let mut warnings = Vec::new();
    let edit_of = |x: &Value| RefEdit {
        kind: x[0].as_i64().unwrap_or(-1) as i32,
        args: x[1].as_array().map(|a| a.iter().filter_map(|y| y.as_str().map(str::to_string)).collect()).unwrap_or_default(),
        failed: x[2].as_bool().unwrap_or(false),
        opaque: x[3].as_bool().unwrap_or(false),
    };
    let conn_of = |x: &Value| RefConn { src: x[0].as_str().unwrap_or("").to_string(), dst: x[1].as_str().unwrap_or("").to_string(), src_ref: x[2].as_bool().unwrap_or(false), dst_ref: x[3].as_bool().unwrap_or(false), failed: false };
    for r in list {
        let rfn = r["rfn"].as_str().unwrap_or("").to_string();
        let t = &mut s.extras.references;
        t.pending.insert(rfn.clone(), r["edits"].as_array().into_iter().flatten().map(edit_of).collect());
        t.pending_conns.insert(rfn.clone(), r["connections"].as_array().into_iter().flatten().map(conn_of).collect());
        t.load_state.insert(rfn, !r["loaded"].as_bool().unwrap_or(true));
    }
    for r in list.iter().filter(|r| r["parent"].as_u64().unwrap_or(0) == 0) {
        let rfn = r["rfn"].as_str().map(str::to_string);
        let f = FileRef { path: r["path"].as_str().unwrap_or("").to_string(), ns: r["namespace"].as_str().unwrap_or("").to_string(), rfn, rpr: r["prefix"].as_str().filter(|p| !p.is_empty()).map(str::to_string), ..Default::default() };
        if let Err(w) = from_ma(s, &f) {
            warnings.push(w);
        }
    }
    end_read(s, &mut warnings);
    warnings
}

/// A scene being read: its folder is where its relative references are.
pub(crate) fn note_scene_folder(s: &mut Session, path: &str) {
    if let Some(dir) = Path::new(path).parent().map(Path::to_path_buf) {
        let t = &mut s.extras.references;
        if !t.search.contains(&dir) {
            t.search.push(dir);
        }
    }
}

/// The namespace of the reference being read, for names in its file that lie outside it.
pub(crate) fn reading_namespace(s: &Session) -> String {
    let t = &s.extras.references;
    t.stack.last().and_then(|id| t.entries.iter().find(|e| e.id == *id)).map(|e| e.namespace.clone()).unwrap_or_default()
}

/// Called once a file's text is parsed, before its references come in: the edits its reference nodes keep, and which
/// start unloaded.
pub(crate) fn begin_read(s: &mut Session, edits: &[(String, Vec<String>)], load_state: &[(String, bool)], warnings: &mut Vec<String>) {
    let t = &mut s.extras.references;
    let above = t.stack.last().and_then(|id| t.entries.iter().find(|e| e.id == *id)).map(|e| e.namespace.clone()).unwrap_or_default();
    let nested = !t.stack.is_empty();
    for (name, deferred) in load_state {
        let key = if above.is_empty() { name.clone() } else { format!("{above}:{name}") };
        // What the scene being opened says wins over what a file it references says.
        t.load_state.entry(key).or_insert(*deferred);
    }
    for (_, tokens) in edits {
        let (sections, w) = parse_edits(tokens);
        warnings.extend(w);
        for (rfn, mut list) in sections {
            let key = if above.is_empty() { rfn.clone() } else { format!("{above}:{rfn}") };
            if !above.is_empty() {
                list.iter_mut().for_each(|e| prefix_edit(e, &above));
            }
            if nested {
                t.pending_file.entry(key).or_default().extend(list);
            } else {
                t.pending.entry(key).or_default().extend(list);
            }
        }
    }
}

/// A `file -r` line while a file is read: an entry, loaded unless it should wait or can't be found.
pub fn from_ma(s: &mut Session, r: &FileRef) -> Result<(), String> {
    let at = add_entry(s, r);
    let rfn = s.extras.references.entries[at].rfn.clone();
    let deferred = s.extras.references.load_state.get(&rfn).copied().unwrap_or(r.deferred);
    let mk = |s: &mut Session| {
        let (id, file) = (s.extras.references.entries[at].id, PathBuf::from(&s.extras.references.entries[at].path));
        s.graph.without_undo(|g| {
            if g.find(&rfn).ok().flatten().is_none() {
                make_reference_node(g, &rfn, id, &file);
            }
        });
    };
    if deferred {
        if let Some(p) = resolve(s, &r.path) {
            s.extras.references.entries[at].path = p.to_string_lossy().replace('\\', "/");
        }
        mk(s);
        return Ok(());
    }
    match load(s, at, false) {
        Ok(_) => Ok(()),
        Err(e) => {
            mk(s);
            Err(e)
        }
    }
}

/// A connection in a file's text with a referenced end. `src_ref`/`dst_ref` where a placeholder says so; else by name.
pub(crate) fn file_connection(s: &mut Session, a: &str, b: &str, src_ref: bool, dst_ref: bool, owner: Option<&str>) {
    let t = &mut s.extras.references;
    let above = t.stack.last().and_then(|id| t.entries.iter().find(|e| e.id == *id)).map(|e| e.namespace.clone()).unwrap_or_default();
    let nested = !t.stack.is_empty();
    let (src, dst) = (prefix_path(a, &above), prefix_path(b, &above));
    let by_ns = |t: &RefTable, plug: &str| -> Option<u32> {
        let node = plug.split('.').next().unwrap_or(plug);
        let leaf = node.rsplit('|').next().unwrap_or(node);
        let ns = leaf.rsplit_once(':')?.0;
        t.entries.iter().find(|e| e.namespace == ns).map(|e| e.id)
    };
    let id = owner
        .map(|o| if above.is_empty() { o.to_string() } else { format!("{above}:{o}") })
        .and_then(|o| t.entries.iter().find(|e| e.rfn == o).map(|e| e.id))
        .or_else(|| by_ns(t, &src))
        .or_else(|| by_ns(t, &dst));
    let Some(e) = id.and_then(|i| t.entries.iter_mut().find(|e| e.id == i)) else { return };
    let c = RefConn { src, dst, src_ref, dst_ref, failed: false };
    if nested {
        e.file_conns.push(c);
    } else {
        e.connections.push(c);
    }
}

/// When a file has been read: at the top, the scene's nodes exist, so the scene's edits can go on.
pub(crate) fn end_read(s: &mut Session, warnings: &mut Vec<String>) {
    if s.extras.references.depth > 0 {
        return;
    }
    let _ = warnings;
    let fresh = std::mem::take(&mut s.extras.references.fresh);
    for id in fresh {
        apply_scene_edits(s, id);
    }
    // Connections to a reference that isn't loaded wait, and connections it was given while reading a deferred one do too.
    let t = &mut s.extras.references;
    t.pending_file.clear();
}

/// Whether a scene node drives or is driven by a reference that isn't loaded: it's still needed.
pub fn held_by_unloaded(s: &Session, name: &str) -> bool {
    let start = format!("{name}.");
    s.extras.references.entries.iter().filter(|e| !e.loaded).any(|e| e.connections.iter().any(|c| c.src.trim_start_matches('|').starts_with(&start) || c.dst.trim_start_matches('|').starts_with(&start)))
}

fn side_of(c: &RefConn, src: bool) -> (&str, bool) {
    if src { (&c.src, c.src_ref) } else { (&c.dst, c.dst_ref) }
}

/// The `.ma` lines for a scene's references: the `file` lines for the head, and for after the nodes the reference nodes
/// with their edits, then the connections through their placeholders.
pub fn ma_statements(s: &mut Session) -> (String, String) {
    let mut head = String::new();
    let mut tail = String::new();
    let mut connect_lines = String::new();
    let quoted = |t: &str| format!("\"{}\"", escape(t));
    // Depth first, as the reference lists them.
    fn walk(s: &Session, parent: u32, depth: u32, out: &mut Vec<(usize, u32)>) {
        for (i, e) in s.extras.references.entries.iter().enumerate() {
            if e.parent == parent {
                out.push((i, depth));
                walk(s, e.id, depth + 1, out);
            }
        }
    }
    let mut order = Vec::new();
    walk(s, 0, 1, &mut order);
    for &(at, depth) in &order {
        let e = &s.extras.references.entries[at];
        let leaf_ns = e.namespace.rsplit(':').next().unwrap_or("");
        let ns = if !e.prefix.is_empty() { format!("-rpr {}", quoted(&e.prefix)) } else if e.namespace.is_empty() { "-ns \":\"".to_string() } else { format!("-ns {}", quoted(leaf_ns)) };
        let dr = if e.loaded { "" } else { " -dr 1" };
        let typ = if e.file_type.is_empty() { "mayaAscii" } else { &e.file_type };
        head.push_str(&format!("file -rdi {depth} {ns}{dr} -rfn {} -op \"v=0;\" -typ {} {};\n", quoted(&e.rfn), quoted(typ), quoted(&e.unresolved)));
    }
    for &(at, depth) in &order {
        if depth != 1 {
            continue;
        }
        let e = &s.extras.references.entries[at];
        let leaf_ns = if e.namespace.is_empty() { String::new() } else { e.namespace.clone() };
        let ns = if !e.prefix.is_empty() { format!("-rpr {}", quoted(&e.prefix)) } else if leaf_ns.is_empty() { "-ns \":\"".to_string() } else { format!("-ns {}", quoted(&leaf_ns)) };
        let typ = if e.file_type.is_empty() { "mayaAscii" } else { &e.file_type };
        head.push_str(&format!("file -r {ns} -dr 1 -rfn {} -op \"v=0;\" -typ {} {};\n", quoted(&e.rfn), quoted(typ), quoted(&e.unresolved)));
    }
    for &(at, depth) in &order {
        if depth != 1 {
            continue;
        }
        let (rfn, id) = (s.extras.references.entries[at].rfn.clone(), s.extras.references.entries[at].id);
        // Sections: those below, then this one twice as the reference writes it.
        let ids = subtree(s, id);
        let mut sections: Vec<(String, Vec<RefEdit>, Vec<RefConn>)> = Vec::new();
        for &r in ids.iter().rev() {
            let Some(i) = at_of(s, r) else { continue };
            let (edits, conns) = gather(s, i);
            sections.push((s.extras.references.entries[i].rfn.clone(), edits, conns));
        }
        // Below an unloaded reference: what's kept for those that aren't here now.
        let below = format!("{}:", s.extras.references.entries[at].namespace);
        let t = &s.extras.references;
        for (name, list) in &t.pending {
            if name.starts_with(&below) && !sections.iter().any(|x| x.0 == *name) {
                sections.insert(0, (name.clone(), list.clone(), t.pending_conns.get(name).cloned().unwrap_or_default()));
            }
        }
        let mut body = format!("\t\t{}\n", quoted(&rfn));
        let mut slot = 0;
        let mut slots = String::new();
        for (name, edits, conns) in &sections {
            if *name == rfn {
                body.push_str(&format!("\t\t{} 0\n", quoted(&rfn)));
            }
            let mut lines: Vec<String> = edits.iter().map(edit_line).collect();
            let mut seen_conn = HashSet::new();
            for c in conns {
                if !seen_conn.insert((c.src.clone(), c.dst.clone())) {
                    continue;
                }
                let mut ends = [c.src.clone(), c.dst.clone()];
                for (k, is_src) in [(0usize, true), (1, false)] {
                    let (plug, referenced) = side_of(c, is_src);
                    if !referenced {
                        continue;
                    }
                    slot += 1;
                    slots.push_str(&format!("\tsetAttr \".phl[{slot}]\" 0;\n"));
                    lines.push(format!("5 {} {} {} {} \"\"", if is_src { 3 } else { 4 }, quoted(&rfn), quoted(plug), quoted(&format!("{rfn}.placeHolderList[{slot}]"))));
                    ends[k] = format!("{rfn}.phl[{slot}]");
                }
                let na = if matches!(c.dst.rsplit('.').next().unwrap_or("").split('[').next(), Some("dsm") | Some("dagSetMembers")) { " -na" } else { "" };
                connect_lines.push_str(&format!("connectAttr {} {}{na};\n", quoted(&ends[0]), quoted(&ends[1])));
            }
            body.push_str(&format!("\t\t{} {}\n", quoted(name), lines.len()));
            for l in lines {
                body.push_str(&format!("\t\t{l}\n"));
            }
        }
        let body = body.trim_end_matches('\n');
        tail.push_str(&format!("createNode reference -n {};\n", quoted(&rfn)));
        if slot > 0 {
            tail.push_str(&format!("\tsetAttr -s {slot} \".phl\";\n{slots}"));
        }
        tail.push_str(&format!("\tsetAttr \".ed\" -type \"dataReferenceEdits\" \n{body};\n\tsetAttr \".ptag\" -type \"string\" \"\";\nlockNode -l 1 ;\n"));
    }
    tail.push_str(&connect_lines);
    (head, tail)
}

fn edit_line(e: &RefEdit) -> String {
    let mut out = e.kind.to_string();
    for a in &e.args {
        out.push_str(&format!(" \"{}\"", escape(a)));
    }
    out
}

/// One line per reference for the Reference Editor.
pub fn ui_list(s: &mut Session) -> Value {
    let mut out = Vec::new();
    for at in 0..s.extras.references.entries.len() {
        let (edits, conns) = gather(s, at);
        let e = &s.extras.references.entries[at];
        out.push(json!({
            "rfn": e.rfn, "namespace": e.namespace, "path": e.path, "unresolved": e.unresolved, "loaded": e.loaded, "parent": e.parent, "id": e.id, "locked": e.locked,
            "found": Path::new(&e.path).is_file(),
            "edits": edits.iter().filter(|x| !x.failed).count() + conns.iter().filter(|c| !c.failed).count(),
            "failed": edits.iter().filter(|x| x.failed).count() + conns.iter().filter(|c| c.failed).count(),
            "strings": edits.iter().map(|x| json!({"text": x.text(), "kind": x.command(), "failed": x.failed})).chain(conns.iter().map(|c| json!({"text": format!("connectAttr \"{}\" \"{}\"", c.src, c.dst), "kind": "connectAttr", "failed": c.failed}))).collect::<Vec<_>>(),
        }));
    }
    Value::Array(out)
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let text = |name: &str, short: &str, array: bool| crate::plug::AttrDef {
            name: name.into(),
            short_name: short.into(),
            ty: PlugType::String,
            default_value: PlugValue::String(String::new()),
            flags: flags::INPUT | flags::STORABLE | if array { flags::ARRAY } else { 0 },
            ..Default::default()
        };
        let _ = types::register_type(NodeType {
            name: REF_NODE_TYPE.into(),
            doc: "A file reference: the scene's record of where its nodes come from.".into(),
            create: Some(std::sync::Arc::new(|| Box::new(Plain))),
            own_attrs: vec![text("fileNames", "fn", true), text("proxyTag", "ptag", false), crate::attr_defs::bool_attr("locked", false, "Set when the reference is brought in.")],
            ..Default::default()
        });
    });
}

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        q_flags(Cmd::new(
            "reference",
            "File references: -f path -ns namespace brings one in; -ur, -lr, -rr unload, load and remove; -q -f lists the files.",
            |s, a| {
                if a.bool("query") {
                    return Ok(file_query(s, a, a.pos_str(0)).unwrap_or(Value::Null));
                }
                if let Some(r) = file_flags(s, a) {
                    return r;
                }
                let path = a.get("filename").and_then(Value::as_str).or(a.pos_str(0)).map(str::to_string);
                let Some(path) = path else { fail!("reference: give a file") };
                let r = create(s, &path, &a.string("namespace", ""), a.get("referenceNode").and_then(Value::as_str), a.bool("deferReference")).map_err(CommandError::new)?;
                Ok(json!(r))
            },
        ))
        .positional("file")
        .flag(f("filename", "f", S, "The file to reference."))
        .flag(f("reference", "r", B, "With -q: the referenced files."))
        .flag(f("namespace", "ns", S, "The namespace to put it under."))
        .flag(f("referenceNode", "rfn", S, "The reference node's name."))
        .flag(f("deferReference", "dr", B, "Add it unloaded."))
        .flag(f("loadReference", "lr", S, "Load this one."))
        .flag(f("unloadReference", "ur", S, "Unload this one."))
        .flag(f("removeReference", "rr", B, "Remove this one."))
        .undoable(false),
    );
    c.register(
        Cmd::new(
            "referenceQuery",
            "Ask about references, given a node, a reference node or a file: -inr isNodeReferenced, -f the file, -n nodes, -ns namespace, -rfn reference node, -il loaded, -es its edits.",
            reference_query,
        )
        .positional("node, reference node or file")
        .flag(f("isNodeReferenced", "inr", B, "Whether the node comes from a reference."))
        .flag(f("filename", "f", B, "The file."))
        .flag(f("nodes", "n", B, "The nodes it brought in."))
        .flag(f("namespace", "ns", B, "Its namespace, from the root."))
        .flag(f("parentNamespace", "pns", B, "The namespace above its own."))
        .flag(f("referenceNode", "rfn", B, "Its reference node."))
        .flag(f("isLoaded", "il", B, "Whether it's loaded."))
        .flag(f("isExportEdits", "iee", B, "Whether it's an edits file. None are made here."))
        .flag(f("isPreviewOnly", "ipo", B, "Whether it's only a preview. None are here."))
        .flag(f("shortName", "shn", B, "The file's name without its folder, or the namespace's last part."))
        .flag(f("withoutCopyNumber", "wcn", B, "The file without its {n}."))
        .flag(f("unresolvedName", "un", B, "The file as written."))
        .flag(f("dagPath", "dp", B, "Full paths from -n."))
        .flag(f("parent", "p", B, "With -rfn or -f, the reference above it."))
        .flag(f("child", "ch", B, "With -rfn or -f, those directly below it."))
        .flag(f("topReference", "tr", B, "With -rfn or -f, the top one above it."))
        .flag(f("editStrings", "es", B, "Its edits, as commands."))
        .flag(f("editNodes", "en", B, "The nodes its edits touch."))
        .flag(f("editAttrs", "ea", B, "The attributes its edits touch."))
        .flag(f("editCommand", "ec", ArgType::Any, "Only edits of these commands."))
        .flag(f("failedEdits", "fld", B, "Include edits that failed on the last load."))
        .flag(f("successfulEdits", "scs", B, "Include edits that worked; the default."))
        .flag(f("onReferenceNode", "orn", S, "Only edits stored on this reference node."))
        .flag(f("liveEdits", "le", B, "Accepted for scripts."))
        .flag(f("showDagPath", "sdp", B, "Accepted for scripts; edits show full paths."))
        .flag(f("showFullPath", "sfp", B, "Accepted for scripts; edits show full paths."))
        .flag(f("showNamespace", "sns", B, "Accepted for scripts; edits show namespaces."))
        .undoable(false),
    );
    c.register(
        Cmd::new("referenceEdit", "Change a reference's kept edits: -r removes them (unloaded), -cet retargets them, -afe tries the failed ones again.", reference_edit)
            .positional("reference node, node or plug")
            .flag(f("removeEdits", "r", B, "Remove the edits on this target, from an unloaded reference."))
            .flag(f("changeEditTarget", "cet", ArgType::Any, "Point edits on one target at another: old, new."))
            .flag(f("applyFailedEdits", "afe", B, "Try the edits that failed again."))
            .flag(f("editCommand", "ec", ArgType::Any, "Only edits of these commands."))
            .flag(f("failedEdits", "fld", B, "Act on failed edits; the default."))
            .flag(f("successfulEdits", "scs", B, "Act on successful edits."))
            .flag(f("onReferenceNode", "orn", S, "Only edits stored on this reference node."))
            .undoable(false),
    );
}
