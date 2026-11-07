//! Asking about meshes and picking from them: polyInfo, polySelect, polySelectConstraint, polySelectSp,
//! polyListComponentConversion, filterExpand.

use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

use aztek_geo::topology::{boundary_loops, edge_loop, edge_ring};
use aztek_geo::{names, Index, Mesh};
use aztek_ops::{fail, ArgType as T, Args, CmdResult, Commands};
use serde_json::{json, Value};

use super::mesh_cmds::{lamina_faces, non_manifold_edges, non_manifold_points};
use super::*;
use crate::components::{self, ComponentMode as K};

// ---------------------------------------------------------------------------------------------------------------------
// Parsing what the mesh commands' arguments can name

/// A component string split up: (mesh, kind, first index range, second index range).
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Item {
    Mesh(NodeId),
    Vtx(NodeId, Vec<Index>),
    Edge(NodeId, Vec<Index>),
    Face(NodeId, Vec<Index>),
    Map(NodeId, Vec<Index>),
    /// vtxFace[v][f]: every pairing of the two ranges.
    VtxFace(NodeId, Vec<Index>, Vec<Index>),
    Cv(NodeId, Vec<Index>),
    Other(NodeId),
}

fn range(s: &str, count: Index) -> Vec<Index> {
    if s == "*" {
        return (0..count).collect();
    }
    match s.split_once(':') {
        Some((a, b)) => match (a.parse::<Index>(), b.parse::<Index>()) {
            (Ok(a), Ok(b)) => (a.min(b)..=a.max(b)).collect(),
            _ => Vec::new(),
        },
        None => s.parse().ok().into_iter().collect(),
    }
}

pub(crate) fn parse_item(g: &mut crate::graph::Graph, text: &str) -> CmdResult<Item> {
    let (head, tail) = match text.rfind('.').filter(|_| text.ends_with(']')) {
        Some(d) => (&text[..d], Some(&text[d + 1..])),
        None => (text, None),
    };
    let Some(id) = g.find(head)? else { fail!("no object named {head}") };
    let Some(tail) = tail else {
        return Ok(match components::as_mesh(g, id) {
            Some(m) => Item::Mesh(m),
            None => Item::Other(id),
        });
    };
    let open = tail.find('[').unwrap_or(tail.len());
    let kind = &tail[..open];
    let parts: Vec<&str> = tail[open..].trim_start_matches('[').trim_end_matches(']').split("][").collect();
    if kind == "cv" {
        return Ok(Item::Cv(id, range(parts[0], Index::MAX / 2)));
    }
    let Some(mesh) = components::as_mesh(g, id) else { fail!("{head} is not a mesh") };
    let Some(cage) = crate::modifiers::cage(g, mesh) else { fail!("{head} has no mesh") };
    let live = |list: Vec<Index>, valid: &dyn Fn(Index) -> bool| -> Vec<Index> { list.into_iter().filter(|&i| valid(i)).collect() };
    Ok(match kind {
        "vtx" | "pt" => Item::Vtx(mesh, live(range(parts[0], cage.point_capacity()), &|i| cage.point_valid(i))),
        "e" => Item::Edge(mesh, live(range(parts[0], cage.edge_capacity()), &|i| cage.edge_valid(i))),
        "f" => Item::Face(mesh, live(range(parts[0], cage.face_capacity()), &|i| cage.face_valid(i))),
        "map" => {
            let t = crate::uv_ops::topo(&cage);
            Item::Map(mesh, range(parts[0], t.maps.len() as Index))
        }
        "vtxFace" => Item::VtxFace(
            mesh,
            live(range(parts[0], cage.point_capacity()), &|i| cage.point_valid(i)),
            live(range(parts.get(1).copied().unwrap_or("*"), cage.face_capacity()), &|i| cage.face_valid(i)),
        ),
        _ => fail!("unknown component '{kind}' in {text}"),
    })
}

/// The call's arguments, or the selection's components, or the selected objects, as strings.
fn items(s: &Session, a: &Args) -> Vec<String> {
    let names = a.named();
    if !names.is_empty() {
        return names;
    }
    let comps = components::component_strings(&s.graph, false);
    if !comps.is_empty() {
        return comps;
    }
    s.graph.selection().iter().map(|&i| s.graph.path(i)).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// polyInfo

/// A line as the reference's commands print them: the label, the id, then each id in a seven-wide column.
fn row(label: &str, id: Index, ids: &[Index], end: &str) -> String {
    let mut out = format!("{label} {id:>6}:");
    for i in ids {
        out.push_str(&format!(" {i:>6}"));
    }
    out.push_str(end);
    out
}

fn poly_info(s: &mut Session, a: &Args) -> CmdResult {
    let list = items(s, a);
    if list.is_empty() {
        fail!("polyInfo: name or select a mesh or components");
    }
    let mut lines = Vec::new();
    let mut names_found: Vec<String> = Vec::new();
    let mut listing = false;
    for text in list {
        let g = &mut s.graph;
        let item = parse_item(g, &text)?;
        let (mesh, kind, ids): (NodeId, K, Option<Vec<Index>>) = match item {
            Item::Mesh(m) => (m, K::Object, None),
            Item::Vtx(m, i) => (m, K::Vertex, Some(i)),
            Item::Edge(m, i) => (m, K::Edge, Some(i)),
            Item::Face(m, i) => (m, K::Face, Some(i)),
            _ => fail!("polyInfo: can't read {text}"),
        };
        let Some(m) = crate::modifiers::cage(g, mesh) else { continue };
        let of = |k: K| -> Vec<Index> {
            match &ids {
                Some(i) if kind == k => i.clone(),
                Some(i) => components::convert(&m, kind, i, k, false),
                None => components::all_ids(&m, k),
            }
        };
        if a.bool("faceToVertex") {
            for f in of(K::Face) {
                lines.push(row("FACE", f, &m.face_points(f), " \n"));
            }
        }
        if a.bool("faceToEdge") {
            for f in of(K::Face) {
                let es: Vec<Index> = m.face_corners(f).into_iter().map(|c| m.corner_edge(c)).collect();
                lines.push(row("FACE", f, &es, " \n"));
            }
        }
        if a.bool("edgeToVertex") {
            let hard = m.edge_attrs().get::<bool>(names::HARD);
            for e in of(K::Edge) {
                let flag = if hard.get(e as usize).copied().unwrap_or(false) { "  Hard\n" } else { " \n" };
                lines.push(row("EDGE", e, &[m.edge_point(e, 0), m.edge_point(e, 1)], flag));
            }
        }
        if a.bool("edgeToFace") {
            for e in of(K::Edge) {
                let mut fs = Vec::new();
                m.for_each_edge_corner(e, |c| fs.push(m.corner_face(c)));
                lines.push(row("EDGE", e, &fs, " \n"));
            }
        }
        if a.bool("vertexToEdge") {
            for p in of(K::Vertex) {
                let mut es = Vec::new();
                m.for_each_point_edge(p, |e| es.push(e));
                lines.push(row("VERTEX", p, &es, " \n"));
            }
        }
        if a.bool("vertexToFace") {
            for p in of(K::Vertex) {
                lines.push(row("VERTEX", p, &m.point_faces(p), " \n"));
            }
        }
        if a.bool("faceNormals") {
            for f in of(K::Face) {
                let n = m.face_normal(f);
                lines.push(format!("FACE_NORMAL {f:>6}: {:.6} {:.6} {:.6}\n", n.x, n.y, n.z));
            }
        }
        let g = &s.graph;
        if a.bool("nonManifoldEdges") {
            listing = true;
            names_found.extend(component_list(g, mesh, "e", &non_manifold_edges(&m), true));
        }
        if a.bool("nonManifoldVertices") {
            listing = true;
            names_found.extend(component_list(g, mesh, "vtx", &non_manifold_points(&m), true));
        }
        if a.bool("laminaFaces") {
            listing = true;
            names_found.extend(component_list(g, mesh, "f", &lamina_faces(&m), true));
        }
        if a.bool("invalidEdges") {
            listing = true;
            let bad: Vec<Index> = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.edge_face_count(e) == 0).collect();
            names_found.extend(component_list(g, mesh, "e", &bad, true));
        }
        if a.bool("invalidVertices") {
            listing = true;
            let bad: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.point_valid(p) && m.point_faces(p).is_empty()).collect();
            names_found.extend(component_list(g, mesh, "vtx", &bad, true));
        }
        if a.bool("nonManifoldUVEdges") || a.bool("nonManifoldUVs") {
            listing = true;
            let t = crate::uv_ops::topo(&m);
            let bad: Vec<Index> = (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && m.edge_face_count(e) > 2).collect();
            if a.bool("nonManifoldUVEdges") {
                names_found.extend(component_list(g, mesh, "e", &bad, true));
            } else {
                let mut maps = BTreeSet::new();
                for &e in &bad {
                    m.for_each_edge_corner(e, |c| {
                        maps.insert(t.corner_map[c as usize]);
                    });
                }
                names_found.extend(component_list(g, mesh, "map", &maps.into_iter().collect::<Vec<_>>(), true));
            }
        }
    }
    if listing {
        return Ok(json!(names_found));
    }
    Ok(json!(lines))
}

// ---------------------------------------------------------------------------------------------------------------------
// polySelect

fn all_uints(a: &Args, flag: &str) -> Vec<Index> {
    match a.get(flag) {
        Some(Value::Array(v)) => v.iter().filter_map(Value::as_u64).map(|i| i as Index).collect(),
        Some(v) => v.as_u64().map(|i| i as Index).into_iter().collect(),
        None => Vec::new(),
    }
}

/// The shortest run of edges between two points, by length.
fn shortest_edges(m: &Mesh, from: Index, to: Index) -> Vec<Index> {
    let mut dist: BTreeMap<Index, f64> = BTreeMap::from([(from, 0.0)]);
    let mut back: BTreeMap<Index, Index> = BTreeMap::new();
    #[derive(PartialEq)]
    struct Q(f64, Index);
    impl Eq for Q {}
    impl PartialOrd for Q {
        fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
            Some(self.cmp(o))
        }
    }
    impl Ord for Q {
        fn cmp(&self, o: &Self) -> std::cmp::Ordering {
            o.0.total_cmp(&self.0).then(o.1.cmp(&self.1))
        }
    }
    let mut heap = BinaryHeap::from([Q(0.0, from)]);
    while let Some(Q(d, p)) = heap.pop() {
        if p == to {
            break;
        }
        if d > dist[&p] {
            continue;
        }
        m.for_each_point_edge(p, |e| {
            let q = m.edge_other_point(e, p);
            let nd = d + m.edge_length(e);
            if dist.get(&q).is_none_or(|&x| nd < x) {
                dist.insert(q, nd);
                back.insert(q, e);
                heap.push(Q(nd, q));
            }
        });
    }
    let mut out = Vec::new();
    let mut p = to;
    while let Some(&e) = back.get(&p) {
        out.push(e);
        p = m.edge_other_point(e, p);
        if p == from {
            break;
        }
    }
    out.reverse();
    out
}

/// Which border loop an edge lies on.
fn border_of(m: &Mesh, e: Index) -> Vec<Index> {
    boundary_loops(m).into_iter().find(|l| l.edges.contains(&e)).map(|l| l.edges).unwrap_or_default()
}

/// The stretch of a closed or open list of edges from one edge to another.
fn stretch(list: &[Index], a: Index, b: Index) -> Vec<Index> {
    let (Some(i), Some(j)) = (list.iter().position(|&x| x == a), list.iter().position(|&x| x == b)) else { return Vec::new() };
    let (lo, hi) = (i.min(j), i.max(j));
    list[lo..=hi].to_vec()
}

fn pattern(list: Vec<Index>, every: i64) -> Vec<Index> {
    if every <= 1 {
        return list;
    }
    list.into_iter().step_by(every as usize).collect()
}

fn poly_select(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    g.evaluate();
    let names = a.named();
    let mesh = match names.first() {
        Some(n) => crate::mesh_edit::mesh_arg(g, n.split('.').next().unwrap_or(n), "polySelect")?,
        None => crate::mesh_edit::mesh_flag(g, &Args::default(), "polySelect")?,
    };
    let Some(m) = crate::modifiers::cage(g, mesh) else { fail!("polySelect: no mesh") };
    let every = a.int("everyN", 1);
    let mut kind = K::Edge;
    let mut ids: Vec<Index> = Vec::new();
    let flat = |flag: &str| -> Vec<Index> { all_uints(a, flag) };
    let pairs = |flag: &str| -> Vec<[Index; 2]> {
        let mut nums = Vec::new();
        fn walk(v: &Value, out: &mut Vec<Index>) {
            match v {
                Value::Array(x) => x.iter().for_each(|y| walk(y, out)),
                y => out.extend(y.as_u64().map(|i| i as Index)),
            }
        }
        if let Some(v) = a.get(flag) {
            walk(v, &mut nums);
        }
        nums.chunks(2).filter(|c| c.len() == 2).map(|c| [c[0], c[1]]).collect()
    };
    for e in flat("edgeLoop") {
        ids.extend(pattern(edge_loop(&m, e), every));
    }
    for e in flat("edgeRing") {
        ids.extend(pattern(edge_ring(&m, e), every));
    }
    for e in flat("edgeBorder") {
        ids.extend(pattern(border_of(&m, e), every));
    }
    for e in flat("edgeLoopOrBorder") {
        ids.extend(pattern(if m.is_boundary_edge(e) { border_of(&m, e) } else { edge_loop(&m, e) }, every));
    }
    for e in flat("edgeUVLoopOrBorder") {
        ids.extend(pattern(if m.is_boundary_edge(e) { border_of(&m, e) } else { edge_loop(&m, e) }, every));
    }
    for [x, y] in pairs("edgeLoopPath") {
        ids.extend(stretch(&edge_loop(&m, x), x, y));
    }
    for [x, y] in pairs("edgeRingPath") {
        ids.extend(stretch(&edge_ring(&m, x), x, y));
    }
    for [x, y] in pairs("edgeBorderPath") {
        ids.extend(stretch(&border_of(&m, x), x, y));
    }
    // A pattern reads off every second edge from its two samples' gap along the list.
    for (flag, list_of) in [("edgeLoopPattern", 0), ("edgeRingPattern", 1), ("edgeBorderPattern", 2), ("edgeLoopOrBorderPattern", 3)] {
        for [x, y] in pairs(flag) {
            let list = match list_of {
                0 => edge_loop(&m, x),
                1 => edge_ring(&m, x),
                2 => border_of(&m, x),
                _ => {
                    if m.is_boundary_edge(x) {
                        border_of(&m, x)
                    } else {
                        edge_loop(&m, x)
                    }
                }
            };
            if let (Some(i), Some(j)) = (list.iter().position(|&e| e == x), list.iter().position(|&e| e == y)) {
                let gap = (j as i64 - i as i64).abs().max(1) as usize;
                for k in 0..list.len() {
                    if k % gap == i % gap {
                        ids.push(list[k]);
                    }
                }
            }
        }
    }
    for [x, y] in pairs("shortestEdgePath") {
        ids.extend(shortest_edges(&m, x, y));
    }
    for [x, y] in pairs("shortestEdgePathUV") {
        ids.extend(shortest_edges(&m, x, y));
    }
    for [x, y] in pairs("shortestFacePath") {
        kind = K::Face;
        // Breadth first through shared edges.
        let mut back: BTreeMap<Index, Index> = BTreeMap::new();
        let mut todo = std::collections::VecDeque::from([x]);
        let mut seen = BTreeSet::from([x]);
        while let Some(f) = todo.pop_front() {
            if f == y {
                break;
            }
            for c in m.face_corners(f) {
                m.for_each_edge_corner(m.corner_edge(c), |r| {
                    let o = m.corner_face(r);
                    if seen.insert(o) {
                        back.insert(o, f);
                        todo.push_back(o);
                    }
                });
            }
        }
        let mut path = vec![y];
        let mut f = y;
        while let Some(&b) = back.get(&f) {
            path.push(b);
            f = b;
        }
        path.reverse();
        ids.extend(path);
    }
    for f in flat("extendToShell") {
        kind = K::Face;
        ids.extend(aztek_geo::ops::regions(&m, &components::all_ids(&m, K::Face)).into_iter().find(|r| r.contains(&f)).unwrap_or_default());
    }
    if ids.is_empty() && !["edgeLoop", "edgeRing", "edgeBorder", "edgeLoopOrBorder", "extendToShell", "shortestEdgePath"].iter().any(|f| a.has(f)) {
        fail!("polySelect: say what to select, as -el or -er");
    }
    let mut seen = BTreeSet::new();
    ids.retain(|i| seen.insert(*i));

    if !a.bool("noSelection") {
        let how = if a.bool("add") || a.bool("addFirst") {
            components::Op::Add
        } else if a.bool("deselect") {
            components::Op::Remove
        } else if a.bool("toggle") {
            components::Op::Toggle
        } else {
            components::Op::Replace
        };
        components::set_mode(g, kind);
        components::select(g, mesh, kind, &ids, how);
    }
    if a.bool("asSelectString") {
        let prefix = if kind == K::Face { "f" } else { "e" };
        return Ok(json!(component_list(&s.graph, mesh, prefix, &ids, false)));
    }
    Ok(json!(ids))
}

// ---------------------------------------------------------------------------------------------------------------------
// polySelectConstraint

const CONSTRAINT_DEFAULT_ZERO: [&str; 24] = [
    "mode", "type", "size", "where", "propagate", "angle", "nonmanifold", "topology", "holes", "planarity", "convexity", "length",
    "geometricarea", "order", "random", "visibility", "smoothness", "textured", "texturedarea", "textureshared", "orient", "dist", "edgeDistance",
    "uvFaceOrientation",
];

fn c_int(st: &serde_json::Map<String, Value>, k: &str) -> i64 {
    st.get(k).and_then(|v| v.as_i64().or_else(|| v.as_bool().map(i64::from))).unwrap_or(0)
}

fn c_on(st: &serde_json::Map<String, Value>, k: &str) -> bool {
    st.get(k).and_then(|v| v.as_bool().or_else(|| v.as_i64().map(|i| i != 0))).unwrap_or(false)
}

fn c_bound(st: &serde_json::Map<String, Value>, k: &str, lo: f64, hi: f64) -> (f64, f64) {
    match st.get(k).and_then(Value::as_array) {
        Some(v) if v.len() >= 2 => (v[0].as_f64().unwrap_or(lo), v[1].as_f64().unwrap_or(hi)),
        _ => (lo, hi),
    }
}

/// Which kind a type mask names.
fn kind_of_type(t: i64) -> Option<K> {
    match t {
        0x0001 => Some(K::Vertex),
        0x8000 => Some(K::Edge),
        0x0008 => Some(K::Face),
        _ => None,
    }
}

/// Whether one component passes every constraint switched on.
fn passes(m: &Mesh, kind: K, id: Index, st: &serde_json::Map<String, Value>, nm_edges: &BTreeSet<Index>, nm_points: &BTreeSet<Index>) -> bool {
    let border = match kind {
        K::Vertex => m.is_boundary_point(id),
        K::Edge => m.is_boundary_edge(id),
        _ => m.face_corners(id).iter().any(|&c| m.is_boundary_edge(m.corner_edge(c))),
    };
    match c_int(st, "where") {
        1 if !border => return false,
        2 if border => return false,
        _ => {}
    }
    if c_on(st, "nonmanifold") {
        let bad = match kind {
            K::Edge => nm_edges.contains(&id),
            K::Vertex => nm_points.contains(&id),
            _ => m.face_corners(id).iter().any(|&c| nm_edges.contains(&m.corner_edge(c))),
        };
        if !bad {
            return false;
        }
    }
    if kind == K::Face {
        let n = m.face_size(id);
        let want = c_int(st, "size");
        if (want == 1 && n != 3) || (want == 2 && n != 4) || (want == 3 && n <= 4) {
            return false;
        }
        if c_int(st, "geometricarea") != 0 {
            let (lo, hi) = c_bound(st, "geometricareabound", 0.0, f64::MAX);
            let area = m.face_area(id);
            if area < lo || area > hi {
                return false;
            }
        }
        let topo = c_int(st, "topology");
        if topo == 2 || topo == 3 {
            let mut key = m.face_points(id);
            key.sort_unstable();
            let dup = (0..m.face_capacity()).any(|o| {
                if o == id || !m.face_valid(o) {
                    return false;
                }
                let mut other = m.face_points(o);
                other.sort_unstable();
                other == key
            });
            if !dup {
                return false;
            }
        }
        let planarity = c_int(st, "planarity");
        if planarity != 0 && m.face_size(id) > 3 {
            let pts = m.face_points(id);
            let n = m.face_normal(id);
            let flat = pts.iter().all(|&p| (m.position(p) - m.position(pts[0])).dot(n).abs() < 1e-6);
            if (planarity == 1 && flat) || (planarity == 2 && !flat) {
                return false;
            }
        } else if planarity == 1 {
            return false;
        }
    }
    if kind == K::Edge {
        if c_int(st, "length") != 0 {
            let (lo, hi) = c_bound(st, "lengthbound", 0.0, f64::MAX);
            let l = m.edge_length(id);
            if l < lo || l > hi {
                return false;
            }
        }
        if c_int(st, "angle") != 0 {
            let (lo, hi) = c_bound(st, "anglebound", 0.0, 180.0);
            let mut ns = Vec::new();
            m.for_each_edge_corner(id, |c| ns.push(m.face_normal(m.corner_face(c))));
            if ns.len() != 2 {
                return false;
            }
            let angle = ns[0].angle_between(ns[1]).to_degrees();
            if angle < lo || angle > hi {
                return false;
            }
        }
        if c_on(st, "crease") {
            let cr = m.edge_attrs().get::<f32>(aztek_geo::attribute::names::CREASE);
            if cr.get(id as usize).copied().unwrap_or(0.0) <= 0.0 {
                return false;
            }
        }
    }
    if kind == K::Vertex && c_int(st, "order") != 0 {
        let (lo, hi) = c_bound(st, "orderbound", 0.0, f64::MAX);
        let v = m.point_valence(id) as f64;
        if v < lo || v > hi {
            return false;
        }
    }
    true
}

/// The neighbours of a component of the same kind, through shared points.
fn neighbours(m: &Mesh, kind: K, id: Index) -> Vec<Index> {
    let mut out = Vec::new();
    match kind {
        K::Vertex => m.for_each_point_edge(id, |e| out.push(m.edge_other_point(e, id))),
        K::Edge => {
            for p in [m.edge_point(id, 0), m.edge_point(id, 1)] {
                m.for_each_point_edge(p, |e| {
                    if e != id {
                        out.push(e);
                    }
                });
            }
        }
        _ => {
            for c in m.face_corners(id) {
                out.extend(m.point_faces(m.corner_point(c)).into_iter().filter(|&f| f != id));
            }
        }
    }
    out.sort_unstable();
    out.dedup();
    out
}

fn propagate(m: &Mesh, kind: K, ids: Vec<Index>, how: i64) -> Vec<Index> {
    let set: BTreeSet<Index> = ids.iter().copied().collect();
    let on_rim = |i: Index| neighbours(m, kind, i).iter().any(|n| !set.contains(n));
    match how {
        1 => {
            let mut out = set.clone();
            for &i in &ids {
                out.extend(neighbours(m, kind, i));
            }
            out.into_iter().collect()
        }
        2 => ids.iter().copied().filter(|&i| !on_rim(i)).collect(),
        3 => ids.iter().copied().filter(|&i| on_rim(i)).collect(),
        4 | 5 if kind == K::Edge => {
            let mut out = set.clone();
            for &i in &ids {
                out.extend(edge_loop(m, i));
            }
            out.into_iter().collect()
        }
        _ => ids,
    }
}

/// Applies the state to some components of a mesh: shell and border extensions, each constraint, then propagation.
fn apply_constraint(m: &Mesh, kind: K, ids: Vec<Index>, st: &serde_json::Map<String, Value>) -> Vec<Index> {
    let nm_e: BTreeSet<Index> = non_manifold_edges(m).into_iter().collect();
    let nm_p: BTreeSet<Index> = non_manifold_points(m).into_iter().collect();
    let mut out: Vec<Index> = ids.iter().copied().filter(|&i| passes(m, kind, i, st, &nm_e, &nm_p)).collect();
    if c_on(st, "border") {
        out = match kind {
            K::Edge => {
                let mut all = BTreeSet::new();
                for &e in &out {
                    if m.is_boundary_edge(e) {
                        all.extend(border_of(m, e));
                    }
                }
                all.into_iter().collect()
            }
            K::Vertex => out.into_iter().filter(|&p| m.is_boundary_point(p)).collect(),
            _ => out.into_iter().filter(|&f| m.face_corners(f).iter().any(|&c| m.is_boundary_edge(m.corner_edge(c)))).collect(),
        };
    }
    if c_on(st, "shell") {
        let regions = aztek_geo::ops::regions(m, &components::all_ids(m, K::Face));
        let faces_here = components::convert(m, kind, &out, K::Face, true);
        let mut all = BTreeSet::new();
        for r in regions {
            if r.iter().any(|f| faces_here.contains(f)) {
                all.extend(components::convert(m, K::Face, &r, kind, false));
            }
        }
        out = all.into_iter().collect();
    }
    if c_on(st, "loopPropagation") && kind == K::Edge {
        let mut all = BTreeSet::new();
        for &e in &out {
            all.extend(edge_loop(m, e));
        }
        out = all.into_iter().collect();
    }
    if c_on(st, "ringPropagation") && kind == K::Edge {
        let mut all = BTreeSet::new();
        for &e in &out {
            all.extend(edge_ring(m, e));
        }
        out = all.into_iter().collect();
    }
    if c_int(st, "random") != 0 {
        let ratio = st.get("randomratio").and_then(Value::as_f64).unwrap_or(0.5).clamp(0.0, 1.0);
        out.retain(|&i| {
            let mut x = i.wrapping_mul(2654435761);
            x ^= x >> 15;
            (x as f64 / u32::MAX as f64) < ratio
        });
    }
    let pp = c_int(st, "propagate");
    if pp != 0 {
        out = propagate(m, kind, out, pp);
    }
    out
}

fn select_constraint(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        if a.bool("stateString") || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == "stateString")) {
            let st = &s.poly_state.constraint;
            let mut out = String::from("polySelectConstraint");
            for (k, v) in st {
                out.push_str(&format!(" -{k} {}", if let Value::Array(l) = v { l.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(" ") } else { v.to_string() }));
            }
            return Ok(json!(out));
        }
        let asked_list: Vec<String> = a.get("queried").and_then(Value::as_array).map(|q| q.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default();
        let name = asked_list.first().cloned().or_else(|| CONSTRAINT_DEFAULT_ZERO.iter().find(|n| a.has(n)).map(|n| n.to_string())).unwrap_or("mode".to_string());
        let st = &s.poly_state.constraint;
        return Ok(st.get(&name).cloned().unwrap_or_else(|| {
            if ["border", "shell", "crease", "loopPropagation", "ringPropagation", "uvShell", "wholeSensitive"].contains(&name.as_str()) {
                json!(false)
            } else {
                json!(0)
            }
        }));
    }
    if a.bool("disable") {
        let keep: Vec<(String, Value)> = s.poly_state.constraint.iter().filter(|(k, _)| k.ends_with("bound") || k.as_str() == "randomratio").map(|(k, v)| (k.clone(), v.clone())).collect();
        s.poly_state.constraint.clear();
        s.poly_state.constraint.extend(keep);
    }
    let return_only = a.bool("returnSelection");
    for (k, v) in &a.flags {
        if matches!(k.as_str(), "disable" | "returnSelection" | "query" | "queried" | "edit" | "constructionHistory" | "name" | "caching" | "nodeState" | "frozen") {
            continue;
        }
        s.poly_state.constraint.insert(k.clone(), v.clone());
    }
    let st = s.poly_state.constraint.clone();
    let mode = c_int(&st, "mode");
    if mode < 2 {
        // Off and Next change no selection now; propagate alone still works on the current one.
        if c_int(&st, "propagate") == 0 {
            return Ok(if return_only { json!([]) } else { Value::Null });
        }
    }
    let g = &mut s.graph;
    g.evaluate();
    let kind = kind_of_type(c_int(&st, "type")).unwrap_or_else(|| match components::mode(g) {
        K::Object => K::Face,
        k => k,
    });
    let mut results: BTreeMap<NodeId, Vec<Index>> = BTreeMap::new();
    let selected_mode = components::mode(g);
    let hilited = components::hilited_meshes(g);
    if mode == 3 {
        let mut scope: BTreeSet<NodeId> = hilited.into_iter().collect();
        if selected_mode != K::Object {
            scope.extend(components::selected_all(g, selected_mode).keys().copied());
        }
        for n in scope {
            let Some(m) = crate::modifiers::cage(g, n) else { continue };
            results.insert(n, apply_constraint(&m, kind, components::all_ids(&m, kind), &st));
        }
    } else if selected_mode != K::Object {
        for (n, ids) in components::selected_all(g, selected_mode).clone() {
            let Some(m) = crate::modifiers::cage(g, n) else { continue };
            let converted = components::convert(&m, selected_mode, &ids, kind, false);
            results.insert(n, apply_constraint(&m, kind, converted, &st));
        }
    }
    if return_only {
        let mut out = Vec::new();
        let prefix = match kind {
            K::Vertex => "vtx",
            K::Edge => "e",
            _ => "f",
        };
        for (n, ids) in &results {
            out.extend(component_list(g, *n, prefix, ids, false));
        }
        return Ok(json!(out));
    }
    let empty = BTreeMap::new();
    let picks = |k: K| if k == kind { results.clone() } else { empty.clone() };
    components::set_all(g, picks(K::Vertex), picks(K::Edge), picks(K::Face));
    components::set_mode_undoable(g, kind);
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// polyListComponentConversion

/// The corners holding some UVs.
fn corners_of_maps(t: &crate::uv_ops::UvTopo, maps: &[Index]) -> Vec<Index> {
    let mut out = BTreeSet::new();
    for &mp in maps {
        out.extend(t.maps.get(mp as usize).into_iter().flatten().copied());
    }
    out.into_iter().collect()
}

fn conversion(s: &mut Session, a: &Args) -> CmdResult {
    let to_kinds: Vec<&str> = ["toVertex", "toEdge", "toFace", "toUV", "toVertexFace"].into_iter().filter(|f| a.bool(f)).collect();
    if to_kinds.is_empty() {
        fail!("polyListComponentConversion: give -tv, -te, -tf, -tuv or -tvf");
    }
    let from_any = ["fromVertex", "fromEdge", "fromFace", "fromUV", "fromVertexFace"].iter().any(|f| a.bool(f));
    let mut out = Vec::new();
    for text in items(s, a) {
        let g = &mut s.graph;
        let item = parse_item(g, &text)?;
        // Components of a kind the -f flags leave out stay out.
        let allowed = |flag: &str| !from_any || a.bool(flag);
        let (mesh, from, ids): (NodeId, K, Vec<Index>) = match item {
            Item::Mesh(m) => {
                let Some(c) = crate::modifiers::cage(g, m) else { continue };
                let f = if a.bool("fromVertex") {
                    K::Vertex
                } else if a.bool("fromEdge") {
                    K::Edge
                } else {
                    K::Face
                };
                let ids = components::all_ids(&c, f);
                (m, f, ids)
            }
            Item::Vtx(m, i) if allowed("fromVertex") => (m, K::Vertex, i),
            Item::Edge(m, i) if allowed("fromEdge") => (m, K::Edge, i),
            Item::Face(m, i) if allowed("fromFace") => (m, K::Face, i),
            Item::Map(m, i) if allowed("fromUV") => {
                let Some(c) = crate::modifiers::cage(g, m) else { continue };
                let t = crate::uv_ops::topo(&c);
                let corners = corners_of_maps(&t, &i);
                let p: Vec<Index> = corners.iter().map(|&k| c.corner_point(k)).collect();
                (m, K::Vertex, p)
            }
            Item::VtxFace(m, v, f) if allowed("fromVertexFace") => {
                let Some(c) = crate::modifiers::cage(g, m) else { continue };
                let p: Vec<Index> = v.into_iter().filter(|&x| f.iter().any(|&y| c.find_corner(y, x) != aztek_geo::INVALID)).collect();
                (m, K::Vertex, p)
            }
            _ => continue,
        };
        let Some(c) = crate::modifiers::cage(g, mesh) else { continue };
        let inside = a.bool("internal");
        for to in &to_kinds {
            let result_kind = match *to {
                "toVertex" => K::Vertex,
                "toEdge" => K::Edge,
                "toFace" => K::Face,
                _ => K::Object,
            };
            if result_kind != K::Object {
                // The reference's default takes all a component touches; -in only what it holds entirely.
                let mut got = components::convert(&c, from, &ids, result_kind, !inside);
                if a.bool("border") {
                    let set: BTreeSet<Index> = got.iter().copied().collect();
                    got.retain(|&i| neighbours(&c, result_kind, i).iter().any(|n| !set.contains(n)) || (result_kind == K::Edge && c.is_boundary_edge(i)));
                }
                let prefix = match result_kind {
                    K::Vertex => "vtx",
                    K::Edge => "e",
                    _ => "f",
                };
                out.extend(component_list(&s.graph, mesh, prefix, &got, false));
                continue;
            }
            // Per corner of the faces at those points.
            let pts = components::convert(&c, from, &ids, K::Vertex, false);
            let face_set: Option<BTreeSet<Index>> = (from == K::Face).then(|| ids.iter().copied().collect());
            if *to == "toUV" {
                let t = crate::uv_ops::topo(&c);
                let mut maps = BTreeSet::new();
                for &p in &pts {
                    for f in c.point_faces(p) {
                        if face_set.as_ref().is_some_and(|x| !x.contains(&f)) {
                            continue;
                        }
                        maps.insert(t.corner_map[c.find_corner(f, p) as usize]);
                    }
                }
                let list: Vec<Index> = maps.into_iter().collect();
                out.extend(component_list(&s.graph, mesh, "map", &list, false));
            } else {
                let path = components::display_path(&s.graph, mesh);
                for &p in &pts {
                    for f in c.point_faces(p) {
                        if face_set.as_ref().is_some_and(|x| !x.contains(&f)) {
                            continue;
                        }
                        out.push(format!("{path}.vtxFace[{p}][{f}]"));
                    }
                }
            }
        }
    }
    Ok(json!(out))
}

// ---------------------------------------------------------------------------------------------------------------------
// filterExpand

fn filter_expand(s: &mut Session, a: &Args) -> CmdResult {
    let masks: Vec<i64> = match a.get("selectionMask") {
        Some(Value::Array(v)) => v.iter().filter_map(Value::as_i64).collect(),
        Some(v) => v.as_i64().into_iter().collect(),
        None => Vec::new(),
    };
    let wants = |m: i64| masks.is_empty() || masks.contains(&m);
    let expand = on(a, "expand", true);
    let full = a.bool("fullPath");
    let mut out = Vec::new();
    for text in items(s, a) {
        let g = &mut s.graph;
        let item = parse_item(g, &text)?;
        let path_of = |g: &crate::graph::Graph, n: NodeId| if full { g.full_path(n) } else { components::display_path(g, n) };
        let mut comp = |g: &crate::graph::Graph, n: NodeId, prefix: &str, ids: &[Index]| {
            let head = path_of(g, n);
            let mut v = ids.to_vec();
            v.sort_unstable();
            v.dedup();
            if expand {
                out.extend(v.into_iter().map(|i| format!("{head}.{prefix}[{i}]")));
            } else {
                let short = component_list(g, n, prefix, &v, false);
                let base = components::display_path(g, n);
                out.extend(short.into_iter().map(|c| c.replacen(&base, &head, 1)));
            }
        };
        match item {
            Item::Vtx(n, i) if wants(31) => comp(g, n, "vtx", &i),
            Item::Edge(n, i) if wants(32) => comp(g, n, "e", &i),
            Item::Face(n, i) if wants(34) => comp(g, n, "f", &i),
            Item::Map(n, i) if wants(35) => comp(g, n, "map", &i),
            Item::Cv(n, i) if wants(28) => comp(g, n, "cv", &i),
            Item::VtxFace(n, v, f) if wants(70) => {
                let head = path_of(g, n);
                for p in &v {
                    for x in &f {
                        out.push(format!("{head}.vtxFace[{p}][{x}]"));
                    }
                }
            }
            Item::Mesh(n) if wants(12) => out.push(if full { g.full_path(crate::dag::transform_of(g, n)) } else { g.path(crate::dag::transform_of(g, n)) }),
            Item::Other(n) => {
                let ty = g.node(n).map(|x| x.ty().name.clone()).unwrap_or_default();
                let curve = crate::curves::as_curve(g, n).is_some();
                let joint = ty == "joint";
                let locator = ty == "locator" || g.children(n).iter().any(|&k| g.node(k).is_some_and(|x| x.ty().name == "locator"));
                let hit = (curve && wants(9)) || (joint && wants(0)) || (locator && wants(22)) || (masks.is_empty());
                if hit {
                    out.push(if full { g.full_path(n) } else { g.path(n) });
                }
            }
            _ => {}
        }
    }
    Ok(if out.is_empty() { Value::Null } else { json!(out) })
}

pub(super) fn register(c: &mut Commands<Session>) {
    c.register(
        ref_cmd(
            "polyInfo",
            "Topology as text. Each of -fv -fe -ev -ef -ve -vf -fn gives one line per component: FACE, EDGE or VERTEX, its id, a colon, its \
             vertices, edges or faces. -nme -nmv -lf -ie -iv give the components at fault.",
            &[],
            poly_info,
        )
        .undoable(false),
    );
    c.register(
        ref_cmd(
            "polySelect",
            "Select along the mesh: -el edge loop, -er ring, -eb border, -elb loop or border, -elp -erp -ebp a stretch between two edges, -lpt -rpt -bpt every nth, \
             -sep shortest path, -sfp between faces, -ets a shell. -r -add -d -tgl say how; -ns only reports; -ass gives component strings, else ids.",
            &[],
            poly_select,
        )
        .undoable(true),
    );
    c.register(
        query_flags(Cmd::new(
            "polySelectConstraint",
            "Constrain what selects: a state kept until changed. -m 0 off, 1 next, 2 current and next, 3 all and next; -t what kind; then -where, -sz, -bo, -sh, \
             -cr, -l -lb, -a -ab, -ga -gab, -nm, -tp, -p, -order, -r -rr and more. -pp 1 grows, 2 shrinks, 3 borders. -rs reports instead of selecting. -dis clears.",
            select_constraint,
        ))
        .flag(fl("mode", "m", T::Int, Value::Null, "0 off, 1 next, 2 current and next, 3 all and next.")),
    );
    if let Some(cmd) = c.find("polySelectConstraint").cloned() {
        let cmd = with_table(cmd, "polySelectConstraint", &[]);
        c.register(cmd);
    }
    c.register(
        Cmd::new(
            "polySelectSp",
            "Select from what is selected: -loop extends to edge loops, -ring to rings. Returns what was selected.",
            |s, a| {
                let mode = components::mode(&s.graph);
                if mode == K::Object {
                    fail!("polySelectSp: select edges");
                }
                let g = &mut s.graph;
                let mut result: BTreeMap<NodeId, Vec<Index>> = BTreeMap::new();
                for (n, ids) in components::selected_all(g, mode).clone() {
                    let Some(m) = crate::modifiers::cage(g, n) else { continue };
                    let list = components::convert(&m, mode, &ids, K::Edge, false);
                    let mut all = BTreeSet::new();
                    for e in list {
                        all.extend(if a.bool("ring") { edge_ring(&m, e) } else { edge_loop(&m, e) });
                    }
                    result.insert(n, all.into_iter().collect());
                }
                let mut out = Vec::new();
                for (n, ids) in &result {
                    out.extend(component_list(g, *n, "e", ids, false));
                }
                components::set_all(g, BTreeMap::new(), result, BTreeMap::new());
                components::set_mode(g, K::Edge);
                Ok(json!(out))
            },
        )
        .positional("edges")
        .flag(fl("loop", "lp", T::Bool, Value::Null, "Edge loops."))
        .flag(fl("ring", "rg", T::Bool, Value::Null, "Edge rings.")),
    );
    c.register(
        with_table(
            Cmd::new(
                "polyListComponentConversion",
                "Convert components without selecting them: -fv -fe -ff -fuv -fvf from, -tv -te -tf -tuv -tvf to. By default whatever the components touch; -in only what \
                 they hold entirely, -bo only the border of the result. Returns component strings.",
                conversion,
            )
            .flag(fl("internal", "in", T::Bool, Value::Null, "Only what the components hold entirely.")),
            "polyListComponentConversion",
            &[],
        )
        .undoable(false),
    );
    c.register(
        ref_cmd(
            "filterExpand",
            "The selection (or arguments) filtered by selection mask and flattened: -sm 31 vertices, 32 edges, 34 faces, 35 UVs, 70 vertex faces, 28 CVs, 12 meshes, 9 curves, \
             22 locators. -ex 0 keeps ranges. -fp full paths. None when nothing matches.",
            &[],
            filter_expand,
        )
        .undoable(false),
    );
}
