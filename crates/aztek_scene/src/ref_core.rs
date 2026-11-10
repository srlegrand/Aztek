//! The reference's core scene queries with their full flag sets: ls and friends. Later registrations replace the earlier, thinner
//! commands of the same name.

use aztek_ops::{ArgType, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::components::{self, ComponentMode};
use crate::graph::Graph;
use crate::plug::{NodeId, NO_NODE};
use crate::session::Session;
use crate::transform::DagNode;

type Cmd = CommandDef<Session>;

pub(crate) fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

/// Whether query mode asked for this flag.
pub(crate) fn asked(a: &aztek_ops::Args, name: &str) -> bool {
    a.get("queried").and_then(Value::as_array).is_some_and(|v| v.iter().any(|x| x.as_str() == Some(name))) || a.bool(name)
}

/// Replaces a registered command with one that has more flags and runs `wrap`, handed the old command to fall back on.
/// A flag named like an old one takes its place, so its default can change.
pub(crate) fn extend(
    c: &mut Commands<Session>,
    name: &str,
    flags: Vec<FlagDef>,
    wrap: impl Fn(&mut Session, &aztek_ops::Args, &aztek_ops::RunFn<Session>) -> CmdResult + Send + Sync + 'static,
) {
    let mut def = c.find(name).unwrap_or_else(|| panic!("no command {name}")).clone();
    let prev = def.run.clone();
    for fl in flags {
        def.flags.retain(|o| o.name != fl.name);
        def.flags.push(fl);
    }
    def.run = std::sync::Arc::new(move |s: &mut Session, a: &aztek_ops::Args| wrap(s, a, &prev));
    c.register(def);
}

/// Glob match: `*` any run, `?` any one character.
pub fn glob(pat: &str, text: &str) -> bool {
    let (p, t): (Vec<char>, Vec<char>) = (pat.chars().collect(), text.chars().collect());
    let (mut i, mut j, mut star, mut mark) = (0usize, 0usize, None, 0usize);
    while j < t.len() {
        if i < p.len() && (p[i] == '?' || p[i] == t[j]) {
            i += 1;
            j += 1;
        } else if i < p.len() && p[i] == '*' {
            star = Some(i);
            mark = j;
            i += 1;
        } else if let Some(s) = star {
            i = s + 1;
            mark += 1;
            j = mark;
        } else {
            return false;
        }
    }
    while i < p.len() && p[i] == '*' {
        i += 1;
    }
    i == p.len()
}

/// Nodes a name or pattern stands for: pCube*, |grp|pCube1, grp|*, *Shape.
pub fn matching_nodes(g: &Graph, pat: &str) -> Vec<NodeId> {
    if !pat.contains('*') && !pat.contains('?') {
        return g.find_all(pat);
    }
    let parts: Vec<&str> = pat.trim_start_matches('|').split('|').collect();
    let rooted = pat.starts_with('|');
    let mut out = Vec::new();
    for id in g.nodes() {
        if parts.len() == 1 && !rooted {
            if g.node(id).is_some_and(|n| glob(pat, &n.name)) {
                out.push(id);
            }
            continue;
        }
        // Walk up: the last part names the node, the rest its ancestors.
        let mut n = id;
        let mut ok = true;
        for (i, part) in parts.iter().enumerate().rev() {
            if n == NO_NODE || !g.node(n).is_some_and(|x| glob(part, &x.name)) {
                ok = false;
                break;
            }
            n = g.parent(n);
            if i == 0 && rooted && n != NO_NODE {
                ok = false;
            }
        }
        if ok {
            out.push(id);
        }
    }
    out
}

fn is_mesh_like(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("mesh") || n.ty().is_a("nurbsCurve") || n.ty().is_a("nurbsSurface") || n.ty().is_a("lattice"))
}

fn is_shape(g: &Graph, id: NodeId) -> bool {
    crate::dag::is_shape(g, id)
}

fn is_intermediate(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().dag && n.bool(DagNode::INTERMEDIATE_OBJECT))
}

fn visible(g: &Graph, id: NodeId) -> bool {
    let mut n = id;
    while n != NO_NODE {
        let Some(node) = g.node(n) else { return false };
        if node.ty().dag && !node.bool(DagNode::VISIBILITY) {
            return false;
        }
        n = g.parent(n);
    }
    true
}

/// A component string expanded to one per item: pCube1.vtx[0:2] -> pCube1.vtx[0], [1], [2].
pub fn flatten_component(g: &mut Graph, s: &str) -> Vec<String> {
    let Some((obj, comp)) = s.split_once('.') else { return vec![s.to_string()] };
    let Some((kind, inner)) = comp.split_once('[') else { return vec![s.to_string()] };
    let inner = inner.trim_end_matches(']');
    let Ok(Some(id)) = g.find(obj) else { return vec![s.to_string()] };
    if let Some(list) = crate::nurbs_components::flatten(g, id, obj, kind, inner) {
        return list;
    }
    if let Some(sh) = crate::surfaces::as_surface(g, id) {
        g.evaluate();
        let Some(ids) = crate::surfaces::parse_cv_ranges(g, sh, inner) else { return vec![s.to_string()] };
        let stride = crate::surfaces::id_stride(g, sh).unwrap_or(1);
        return crate::surfaces::format_cvs_flat(&ids, stride).into_iter().map(|c| format!("{obj}.{c}")).collect();
    }
    if let Some(c) = crate::curves::as_curve(g, id) {
        let count = crate::curves::local_curve(g, c).map(|k| k.distinct_cvs().len() as u32).unwrap_or(0);
        let mut out = Vec::new();
        for part in inner.split(',') {
            if part == "*" {
                out.extend((0..count).map(|i| format!("{obj}.{kind}[{i}]")));
            } else if let Some((a, b)) = part.split_once(':') {
                let (a, b): (u32, u32) = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
                out.extend((a..=b).map(|i| format!("{obj}.{kind}[{i}]")));
            } else {
                out.push(format!("{obj}.{kind}[{part}]"));
            }
        }
        return out;
    }
    if let Some(strings) = grid_component(g, id, obj, kind, inner, true) {
        return strings;
    }
    let Some(mesh) = components::as_mesh(g, id) else { return vec![s.to_string()] };
    let Some(m) = components::mesh_of(g, mesh) else { return vec![s.to_string()] };
    let cap = match kind {
        "vtx" | "pt" => m.point_capacity(),
        "e" => m.edge_capacity(),
        "f" => m.face_capacity(),
        _ => return vec![s.to_string()],
    };
    let valid = |i: u32| match kind {
        "e" => m.edge_valid(i),
        "f" => m.face_valid(i),
        _ => m.point_valid(i),
    };
    let mut out = Vec::new();
    for part in inner.split(',') {
        if part == "*" {
            out.extend((0..cap).filter(|&i| valid(i)).map(|i| format!("{obj}.{kind}[{i}]")));
        } else if let Some((a, b)) = part.split_once(':') {
            let (a, b): (u32, u32) = (a.parse().unwrap_or(0), b.parse().unwrap_or(0));
            out.extend((a..=b).filter(|&i| i < cap && valid(i)).map(|i| format!("{obj}.{kind}[{i}]")));
        } else {
            out.push(format!("{obj}.{kind}[{part}]"));
        }
    }
    out
}

/// A lattice's pt[s][t][u] as component strings.
fn grid_component(g: &Graph, id: NodeId, obj: &str, kind: &str, inner: &str, flat: bool) -> Option<Vec<String>> {
    if kind != "pt" {
        return None;
    }
    let node = components::point_node(g, id)?;
    let dims = components::point_grid(g, node)?;
    Some(components::grid_strings(obj, dims, &components::grid_ids(dims, inner)?, flat))
}

fn compress_component(g: &mut Graph, s: &str) -> String {
    let Some((obj, comp)) = s.split_once('.') else { return s.to_string() };
    let Some((kind, inner)) = comp.split_once('[') else { return s.to_string() };
    if let Ok(Some(id)) = g.find(obj) {
        if let Some(strings) = grid_component(g, id, obj, kind, inner.trim_end_matches(']'), false) {
            if strings.len() == 1 {
                return strings.into_iter().next().unwrap();
            }
        }
    }
    if inner.starts_with('*') {
        let flat = flatten_component(g, s);
        if let (Some(a), Some(b)) = (flat.first(), flat.last()) {
            let idx = |x: &str| x.rsplit('[').next().map(|y| y.trim_end_matches(']').to_string()).unwrap_or_default();
            return format!("{obj}.{kind}[{}:{}]", idx(a), idx(b));
        }
    }
    s.to_string()
}

fn ls(s: &mut Session, a: &aztek_ops::Args) -> CmdResult {
    let flatten = a.bool("flatten");
    let long = a.bool("long");
    let names = a.named();
    let g = &mut s.graph;
    let mut comps: Vec<String> = Vec::new();
    let mut ids: Vec<NodeId> = Vec::new();
    let filters = ["referencedNodes", "type", "exactType", "dag", "dagObjects", "shapes", "transforms", "geometry", "objectsOnly", "visible", "invisible", "cameras", "lights", "leaf", "assemblies", "noIntermediate", "intermediateObjects", "materials", "partition", "sets", "textures"];
    let filtered = filters.iter().any(|x| a.has(x) && a.get(x) != Some(&Value::Bool(false)));
    if a.bool("selection") || a.bool("orderedSelection") {
        if !a.bool("objectsOnly") {
            comps = components::component_strings(g, false);
        }
        let sel = g.selection().to_vec();
        if comps.is_empty() || a.bool("objectsOnly") {
            ids.extend(sel);
        } else {
            // Objects selected alongside components. One with components of its own is only hilited.
            for id in sel {
                let path = g.path(id);
                let name = g.node(id).map_or(String::new(), |n| n.name().to_string());
                let has_comps = comps.iter().any(|c| c.split('.').next().is_some_and(|o| o == path || o == name));
                if !has_comps {
                    ids.push(id);
                }
            }
        }
    } else if names.is_empty() {
        ids = g.nodes();
    }
    if a.bool("hilite") {
        // Nothing is hilited in object mode, as in the reference.
        ids = if components::mode(g) == components::ComponentMode::Object { Vec::new() } else { components::hilited_meshes(g) };
    }
    for n in &names {
        if components::is_component_string(n) {
            comps.push(n.clone());
        } else if n.contains('.') {
            // A plug stays as it is.
            if g.plug(n).is_ok() {
                comps.push(n.clone());
            }
        } else {
            for id in matching_nodes(g, n) {
                if !ids.contains(&id) {
                    ids.push(id);
                }
            }
        }
    }
    if a.bool("selection") && !names.is_empty() {
        let sel = g.selection().to_vec();
        ids.retain(|i| sel.contains(i));
        comps.clear();
    }
    if a.bool("dagObjects") {
        let mut all = Vec::new();
        for &id in &ids {
            if g.node(id).is_some_and(|n| n.ty().dag) {
                all.push(id);
                all.extend(g.descendants(id));
            }
        }
        let mut seen = std::collections::HashSet::new();
        ids = all.into_iter().filter(|x| seen.insert(*x)).collect();
    }
    let types: Vec<String> = match a.get("type") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    let exact: Vec<String> = match a.get("exactType") {
        Some(Value::String(t)) => vec![t.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    };
    ids.retain(|&id| {
        let Some(n) = g.node(id) else { return false };
        let ty = n.ty();
        (types.is_empty() || types.iter().any(|t| ty.is_a(t)))
            && (exact.is_empty() || exact.iter().any(|t| ty.name == *t))
            && (!a.bool("dag") || ty.dag)
            && (!a.bool("referencedNodes") || (g.node(id).is_some_and(|n| n.reference_id() != 0) && ty.name != crate::references::REF_NODE_TYPE))
            && (!a.bool("shapes") || is_shape(g, id))
            && (!a.bool("transforms") || (ty.is_a("transform") && !is_shape(g, id)))
            && (!a.bool("geometry") || is_mesh_like(g, id))
            && (!a.bool("cameras") || ty.is_a("camera"))
            && (!a.bool("lights") || ty.is_a("light"))
            && (!a.bool("visible") || visible(g, id))
            && (!a.bool("invisible") || !visible(g, id))
            && (!a.bool("assemblies") || (ty.dag && g.parent(id) == NO_NODE))
            && (!a.bool("leaf") || g.children(id).is_empty())
            && (!a.bool("noIntermediate") || !is_intermediate(g, id))
            && (!a.bool("intermediateObjects") || is_intermediate(g, id))
            && (!a.bool("sets") || ty.is_a("objectSet"))
            && (!a.bool("materials") || crate::shading::is_material_type(ty))
            && (!a.bool("textures") || crate::shading::is_texture_type(ty))
    });
    if a.bool("selection") && !a.bool("objectsOnly") && comps.is_empty() && false {
        let _ = ComponentMode::Object;
    }
    let mut out: Vec<String> = Vec::new();
    let name_of = |g: &Graph, id: NodeId| if long { g.full_path(id) } else { g.path(id) };
    let mut flat_comps = Vec::new();
    for c in &comps {
        if flatten {
            flat_comps.extend(flatten_component(g, c));
        } else {
            flat_comps.push(compress_component(g, c));
        }
    }
    if !filtered {
        out.extend(flat_comps);
    }
    out.extend(ids.iter().map(|&id| name_of(g, id)));
    if let Some(h) = a.get("head").and_then(Value::as_i64) {
        out.truncate(h.max(0) as usize);
    }
    if let Some(t) = a.get("tail").and_then(Value::as_i64) {
        let n = out.len().saturating_sub(t.max(0) as usize);
        out.drain(..n);
    }
    if a.bool("showType") {
        let with: Vec<Value> = out
            .iter()
            .flat_map(|name| {
                let ty = g.find(name).ok().flatten().and_then(|id| g.node(id).map(|n| n.ty().name.clone())).unwrap_or_default();
                [json!(name), json!(ty)]
            })
            .collect();
        return Ok(Value::Array(with));
    }
    Ok(json!(out))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new("ls", "List nodes and components: names, wildcards (pCube*), components (pCube1.vtx[*]), filtered by the flags.", ls)
            .undoable(false)
            .positional("names or patterns")
            .flag(f("selection", "sl", ArgType::Bool, "The selection."))
            .flag(f("orderedSelection", "os", ArgType::Bool, "The selection, in order."))
            .flag(f("flatten", "fl", ArgType::Bool, "Components one per item."))
            .flag(f("long", "l", ArgType::Bool, "Full DAG paths."))
            .flag(f("shortNames", "sn", ArgType::Bool, "Shortest unique names (the default)."))
            .flag(f("type", "typ", ArgType::Any, "Only nodes of this type or derived; a list of types too."))
            .flag(f("exactType", "et", ArgType::Any, "Only nodes of exactly this type."))
            .flag(f("dag", "dag", ArgType::Bool, "Only DAG nodes."))
            .flag(f("referencedNodes", "rn", ArgType::Bool, "Only nodes from references."))
            .flag(f("dagObjects", "do", ArgType::Bool, "DAG nodes and everything below them."))
            .flag(f("shapes", "s", ArgType::Bool, "Only shapes."))
            .flag(f("transforms", "tr", ArgType::Bool, "Only transforms."))
            .flag(f("geometry", "g", ArgType::Bool, "Only geometry shapes."))
            .flag(f("cameras", "ca", ArgType::Bool, "Only cameras."))
            .flag(f("lights", "lt", ArgType::Bool, "Only lights."))
            .flag(f("objectsOnly", "o", ArgType::Bool, "Objects, not their components."))
            .flag(f("visible", "v", ArgType::Bool, "Only visible objects."))
            .flag(f("invisible", "iv", ArgType::Bool, "Only hidden objects."))
            .flag(f("assemblies", "assemblies", ArgType::Bool, "Only top level DAG nodes."))
            .flag(f("leaf", "lf", ArgType::Bool, "Only nodes with no children."))
            .flag(f("noIntermediate", "ni", ArgType::Bool, "No intermediate objects."))
            .flag(f("intermediateObjects", "io", ArgType::Bool, "Only intermediate objects."))
            .flag(f("hilite", "hl", ArgType::Bool, "The hilited objects."))
            .flag(f("sets", "set", ArgType::Bool, "Only sets."))
            .flag(f("materials", "mat", ArgType::Bool, "Only materials."))
            .flag(f("textures", "tex", ArgType::Bool, "Only textures."))
            .flag(f("showType", "st", ArgType::Bool, "Each name followed by its type."))
            .flag(f("head", "hd", ArgType::Int, "Only the first n."))
            .flag(f("tail", "tl", ArgType::Int, "Only the last n.")),
    );
}
