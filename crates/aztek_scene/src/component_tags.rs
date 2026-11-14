//! Component tags, as the reference has them since its 2022 release: named groups of a mesh's vertices, edges or faces
//! that travel with the geometry through its history. A mesh shape adds its own (`componentTags`, gtag) to what flows
//! through it; a deformer acts on the points its componentTagExpression names. Also the expressions themselves, and the
//! componentTag and geometryAttrInfo commands.
//!
//! In the geometry a tag is a bool layer named `ctag:<name>` in the domain of its components.

use aztek_geo::Mesh;
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::DVec3;
use serde_json::{json, Value};

use crate::attr_defs::STORED_FLAGS;
use crate::deform::{f, Cmd};
use crate::graph::Graph;
use crate::node::NodeData;
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;

const PREFIX: &str = "ctag:";

/// What a tag's components are: vertices, edges, faces, or everything (`*`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cat {
    V,
    E,
    F,
    All,
}

impl Cat {
    fn letter(self) -> &'static str {
        match self {
            Cat::V => "v",
            Cat::E => "e",
            Cat::F => "f",
            Cat::All => "unknown",
        }
    }
    fn prefix(self) -> &'static str {
        match self {
            Cat::V | Cat::All => "vtx",
            Cat::E => "e",
            Cat::F => "f",
        }
    }
}

/// A set of components of one kind. `All` holds no list: it is every component.
#[derive(Clone, Debug)]
pub struct Sel {
    pub cat: Cat,
    pub on: Vec<bool>,
}

impl Sel {
    fn all() -> Sel {
        Sel { cat: Cat::All, on: Vec::new() }
    }
    fn empty(m: &Mesh, cat: Cat) -> Sel {
        Sel { cat, on: vec![false; capacity(m, cat)] }
    }
    pub fn indices(&self) -> Vec<u32> {
        self.on.iter().enumerate().filter(|x| *x.1).map(|x| x.0 as u32).collect()
    }
}

fn capacity(m: &Mesh, cat: Cat) -> usize {
    match cat {
        Cat::V | Cat::All => m.point_capacity() as usize,
        Cat::E => m.edge_capacity() as usize,
        Cat::F => m.face_capacity() as usize,
    }
}

fn valid(m: &Mesh, cat: Cat, i: u32) -> bool {
    match cat {
        Cat::V | Cat::All => m.point_valid(i),
        Cat::E => m.edge_valid(i),
        Cat::F => m.face_valid(i),
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Tags in the geometry

/// The tags on a mesh, by name.
pub fn names(m: &Mesh) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for set in [m.point_attrs(), m.edge_attrs(), m.face_attrs()] {
        for n in set.names() {
            if let Some(t) = n.strip_prefix(PREFIX) {
                if !out.iter().any(|x| x == t) {
                    out.push(t.to_string());
                }
            }
        }
    }
    out.sort();
    out
}

pub fn get(m: &Mesh, name: &str) -> Option<Sel> {
    let key = format!("{PREFIX}{name}");
    for (cat, set) in [(Cat::V, m.point_attrs()), (Cat::E, m.edge_attrs()), (Cat::F, m.face_attrs())] {
        if set.has(&key) {
            let mut on = set.get::<bool>(&key).to_vec();
            on.resize(capacity(m, cat), false);
            return Some(Sel { cat, on });
        }
    }
    None
}

pub fn remove(m: &mut Mesh, name: &str) {
    let key = format!("{PREFIX}{name}");
    m.point_attrs_mut().remove(&key);
    m.edge_attrs_mut().remove(&key);
    m.face_attrs_mut().remove(&key);
}

/// Sets a tag, in place of one of the same name.
pub fn set(m: &mut Mesh, name: &str, sel: &Sel) {
    remove(m, name);
    let key = format!("{PREFIX}{name}");
    let sel = if sel.cat == Cat::All { cast(m, sel, Cat::V) } else { sel.clone() };
    let layer = match sel.cat {
        Cat::E => m.edge_attrs_mut().get_or_add::<bool>(&key),
        Cat::F => m.face_attrs_mut().get_or_add::<bool>(&key),
        _ => m.point_attrs_mut().get_or_add::<bool>(&key),
    };
    for (i, x) in layer.iter_mut().enumerate() {
        *x = sel.on.get(i).copied().unwrap_or(false);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Component text

/// Components as the reference names them: `vtx[0:3]`, `f[2]`, `e[*]`, one or more, space separated. Kinds that differ
/// become vertices.
pub fn parse(m: &Mesh, text: &str) -> Option<Sel> {
    let mut out: Option<Sel> = None;
    for word in text.split(|c: char| c.is_whitespace() || c == ',').filter(|w| !w.is_empty()) {
        let word = word.rsplit_once('.').map_or(word, |x| x.1).trim_matches('"');
        let (kind, rest) = word.split_once('[')?;
        let cat = match kind {
            "vtx" | "cv" | "pt" => Cat::V,
            "e" => Cat::E,
            "f" => Cat::F,
            _ => return None,
        };
        let range = rest.strip_suffix(']')?;
        let mut sel = Sel::empty(m, cat);
        if range == "*" {
            sel.on.iter_mut().enumerate().for_each(|(i, x)| *x = valid(m, cat, i as u32));
        } else {
            let (a, b) = range.split_once(':').unwrap_or((range, range));
            let (a, b): (usize, usize) = (a.trim().parse().ok()?, b.trim().parse().ok()?);
            for i in a..=b.min(sel.on.len().saturating_sub(1)) {
                sel.on[i] = true;
            }
        }
        out = Some(match out {
            None => sel,
            Some(prev) => combine(m, &prev, &sel, Op::Union),
        });
    }
    out
}

/// Components as text: runs of indices, `vtx[0:3]`.
pub fn text(sel: &Sel) -> Vec<String> {
    if sel.cat == Cat::All {
        return vec!["vtx[*]".into()];
    }
    let mut out = Vec::new();
    let ids = sel.indices();
    let mut k = 0;
    while k < ids.len() {
        let mut e = k;
        while e + 1 < ids.len() && ids[e + 1] == ids[e] + 1 {
            e += 1;
        }
        out.push(if e == k { format!("{}[{}]", sel.cat.prefix(), ids[k]) } else { format!("{}[{}:{}]", sel.cat.prefix(), ids[k], ids[e]) });
        k = e + 1;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Casting between kinds

/// Faces and edges become their vertices; vertices become the edges and faces they wholly contain; faces become their
/// edges, and edges the faces they wholly bound.
pub fn cast(m: &Mesh, sel: &Sel, to: Cat) -> Sel {
    if sel.cat == to {
        return sel.clone();
    }
    let mut out = Sel::empty(m, if to == Cat::All { Cat::V } else { to });
    if sel.cat == Cat::All {
        let cat = out.cat;
        out.on.iter_mut().enumerate().for_each(|(i, x)| *x = valid(m, cat, i as u32));
        return out;
    }
    let on = |i: u32| sel.on.get(i as usize).copied().unwrap_or(false);
    match (sel.cat, out.cat) {
        (Cat::F, Cat::V) => {
            for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f) && on(f)) {
                for p in m.face_points(f) {
                    out.on[p as usize] = true;
                }
            }
        }
        (Cat::E, Cat::V) => {
            for e in (0..m.edge_capacity()).filter(|&e| m.edge_valid(e) && on(e)) {
                out.on[m.edge_point(e, 0) as usize] = true;
                out.on[m.edge_point(e, 1) as usize] = true;
            }
        }
        (Cat::V, Cat::E) => {
            for e in (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)) {
                out.on[e as usize] = on(m.edge_point(e, 0)) && on(m.edge_point(e, 1));
            }
        }
        (Cat::V, Cat::F) => {
            for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f)) {
                out.on[f as usize] = m.face_points(f).iter().all(|&p| on(p));
            }
        }
        (Cat::F, Cat::E) => {
            for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f) && on(f)) {
                for c in m.face_corners(f) {
                    out.on[m.corner_edge(c) as usize] = true;
                }
            }
        }
        (Cat::E, Cat::F) => {
            for f in (0..m.face_capacity()).filter(|&f| m.face_valid(f)) {
                out.on[f as usize] = m.face_corners(f).iter().all(|&c| on(m.corner_edge(c)));
            }
        }
        _ => {}
    }
    out
}

#[derive(Clone, Copy, Debug)]
enum Op {
    Union,
    Subtract,
    Intersect,
}

fn combine(m: &Mesh, a: &Sel, b: &Sel, op: Op) -> Sel {
    // Everything stays everything where it can.
    match (a.cat, b.cat, op) {
        (Cat::All, _, Op::Union) | (_, Cat::All, Op::Union) => return Sel::all(),
        (Cat::All, _, Op::Intersect) => return b.clone(),
        (_, Cat::All, Op::Intersect) => return a.clone(),
        _ => {}
    }
    let cat = match (a.cat, b.cat) {
        (Cat::All, c) | (c, Cat::All) => c,
        (x, y) if x == y => x,
        _ => Cat::V,
    };
    let (a, b) = (cast(m, a, cat), cast(m, b, cat));
    let on = a
        .on
        .iter()
        .zip(&b.on)
        .map(|(&x, &y)| match op {
            Op::Union => x || y,
            Op::Subtract => x && !y,
            Op::Intersect => x && y,
        })
        .collect();
    Sel { cat, on }
}

fn invert(m: &Mesh, a: &Sel) -> Sel {
    if a.cat == Cat::All {
        return Sel::empty(m, Cat::V);
    }
    let cat = a.cat;
    Sel { cat, on: a.on.iter().enumerate().map(|(i, &x)| !x && valid(m, cat, i as u32)).collect() }
}

// ---------------------------------------------------------------------------------------------------------------------
// Expressions

#[derive(Clone, Debug, PartialEq)]
enum Tok {
    Name(String),
    Op(char),
    Not,
    Open,
    Close,
    Comma,
}

fn tokens(expr: &str) -> Result<Vec<Tok>, String> {
    let mut out = Vec::new();
    let mut word = String::new();
    let flush = |w: &mut String, out: &mut Vec<Tok>| {
        if !w.is_empty() {
            out.push(Tok::Name(std::mem::take(w)));
        }
    };
    for c in expr.chars() {
        match c {
            '+' | '-' | '^' => {
                flush(&mut word, &mut out);
                out.push(Tok::Op(c));
            }
            '!' => {
                flush(&mut word, &mut out);
                out.push(Tok::Not);
            }
            '(' => {
                flush(&mut word, &mut out);
                out.push(Tok::Open);
            }
            ')' => {
                flush(&mut word, &mut out);
                out.push(Tok::Close);
            }
            ',' => {
                flush(&mut word, &mut out);
                out.push(Tok::Comma);
            }
            c if c.is_whitespace() => flush(&mut word, &mut out),
            c if c.is_alphanumeric() || matches!(c, '_' | '*' | '?' | ':' | '.') => word.push(c),
            c => return Err(format!("unexpected '{c}'")),
        }
    }
    flush(&mut word, &mut out);
    Ok(out)
}

fn glob(pattern: &str, name: &str) -> bool {
    fn go(p: &[char], n: &[char]) -> bool {
        match p.first() {
            None => n.is_empty(),
            Some('*') => (0..=n.len()).any(|k| go(&p[1..], &n[k..])),
            Some('?') => !n.is_empty() && go(&p[1..], &n[1..]),
            Some(c) => n.first() == Some(c) && go(&p[1..], &n[1..]),
        }
    }
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    go(&p, &n)
}

struct Parser<'a> {
    m: &'a Mesh,
    toks: Vec<Tok>,
    at: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.at)
    }
    /// Left to right: the reference gives no precedence beyond brackets. Two operands side by side, or with a comma
    /// between, are a union.
    fn expr(&mut self) -> Result<Sel, String> {
        let mut acc = self.unary()?;
        loop {
            let op = match self.peek() {
                Some(Tok::Op('+')) | Some(Tok::Comma) => Op::Union,
                Some(Tok::Op('-')) => Op::Subtract,
                Some(Tok::Op('^')) => Op::Intersect,
                Some(Tok::Name(_)) | Some(Tok::Not) | Some(Tok::Open) => {
                    let b = self.unary()?;
                    acc = combine(self.m, &acc, &b, Op::Union);
                    continue;
                }
                _ => return Ok(acc),
            };
            self.at += 1;
            let b = self.unary()?;
            acc = combine(self.m, &acc, &b, op);
        }
    }
    fn unary(&mut self) -> Result<Sel, String> {
        match self.toks.get(self.at).cloned() {
            Some(Tok::Not) => {
                self.at += 1;
                let a = self.unary()?;
                Ok(invert(self.m, &a))
            }
            Some(Tok::Open) => {
                self.at += 1;
                let a = self.expr()?;
                if self.peek() != Some(&Tok::Close) {
                    return Err("missing ')'".into());
                }
                self.at += 1;
                Ok(a)
            }
            Some(Tok::Name(n)) => {
                self.at += 1;
                // v(), e(), f(): the members as vertices, edges or faces.
                if matches!(n.as_str(), "v" | "e" | "f") && self.peek() == Some(&Tok::Open) {
                    self.at += 1;
                    let a = self.expr()?;
                    if self.peek() != Some(&Tok::Close) {
                        return Err("missing ')'".into());
                    }
                    self.at += 1;
                    let cat = match n.as_str() {
                        "v" => Cat::V,
                        "e" => Cat::E,
                        _ => Cat::F,
                    };
                    return Ok(cast(self.m, &a, cat));
                }
                if n == "*" {
                    return Ok(Sel::all());
                }
                let mut out: Option<Sel> = None;
                for t in names(self.m).iter().filter(|t| glob(&n, t)) {
                    let s = get(self.m, t).unwrap();
                    out = Some(match out {
                        None => s,
                        Some(p) => combine(self.m, &p, &s, Op::Union),
                    });
                }
                Ok(out.unwrap_or_else(|| Sel::empty(self.m, Cat::V)))
            }
            other => Err(format!("expected a tag name, found {other:?}")),
        }
    }
}

/// The components an expression names on a mesh.
pub fn eval(m: &Mesh, expr: &str) -> Result<Sel, String> {
    let mut p = Parser { m, toks: tokens(expr)?, at: 0 };
    if p.toks.is_empty() {
        return Ok(Sel::all());
    }
    let s = p.expr()?;
    if p.at < p.toks.len() {
        return Err(format!("unexpected {:?}", p.toks[p.at]));
    }
    Ok(s)
}

/// The points a deformer's expression names, or None when it names them all or is empty (the deformer's set decides).
pub fn points(m: &Mesh, expr: &str) -> Option<Vec<bool>> {
    if expr.trim().is_empty() {
        return None;
    }
    let s = eval(m, expr).unwrap_or_else(|_| Sel::empty(m, Cat::V));
    if s.cat == Cat::All {
        return None;
    }
    Some(cast(m, &s, Cat::V).on)
}

// ---------------------------------------------------------------------------------------------------------------------
// On shapes

/// componentTags: the tags a shape adds to the geometry that passes through it.
pub(crate) fn shape_attrs() -> Vec<AttrDef> {
    let kid = |name: &str, short: &str, doc: &str| AttrDef {
        name: format!("componentTags.{name}"),
        short_name: format!("gtag.{short}"),
        ty: PlugType::String,
        default_value: PlugValue::String(String::new()),
        flags: STORED_FLAGS | flags::ARRAY | flags::HIDDEN,
        doc: doc.into(),
        ..Default::default()
    };
    vec![
        kid("componentTagName", "gtagnm", "Each tag's name."),
        kid("componentTagContents", "gtagcmp", "Each tag's components: vtx[0:3] f[2]..."),
    ]
}

/// The tags a node holds in componentTags, by element: (index, name, contents).
pub(crate) fn node_tags(d: &NodeData) -> Vec<(i32, String, String)> {
    let t = d.ty();
    let (Some(na), Some(ca)) = (t.attr_index("componentTags.componentTagName"), t.attr_index("componentTags.componentTagContents")) else { return Vec::new() };
    let mut out: Vec<(i32, String, String)> = Vec::new();
    for (k, v) in d.elements(na) {
        if let PlugValue::String(n) = v {
            if !n.is_empty() {
                let c = match d.element_or(ca, *k) {
                    PlugValue::String(c) => c.clone(),
                    _ => String::new(),
                };
                out.push((*k, n.clone(), c));
            }
        }
    }
    out
}

/// The geometry with a node's own tags added, each in place of one of the same name.
pub(crate) fn with_node_tags(d: &NodeData, m: Option<crate::plug::MeshPtr>) -> Option<crate::plug::MeshPtr> {
    let tags = node_tags(d);
    let m = m?;
    if tags.is_empty() {
        return Some(m);
    }
    let mut out = (*m).clone();
    for (_, name, contents) in tags {
        let sel = parse(&out, &contents).unwrap_or_else(|| Sel::empty(&out, Cat::V));
        set(&mut out, &name, &sel);
    }
    Some(std::sync::Arc::new(out))
}

/// Moves a shape's own tags to another node, as a deformer's original shape takes them from the shape it is made for.
pub(crate) fn move_node_tags(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    let Some(n) = g.node(from) else { return Ok(()) };
    let tags = node_tags(n.data());
    if tags.is_empty() || from == to {
        return Ok(());
    }
    let (fna, fca) = tag_plugs(g, from)?;
    let (na, ca) = tag_plugs(g, to)?;
    let mut next = node_tags(g.node(to).unwrap().data()).iter().map(|x| x.0 + 1).max().unwrap_or(0);
    for (k, name, contents) in tags {
        g.set_value(PlugRef::element(to, na as i32, next), PlugValue::String(name))?;
        g.set_value(PlugRef::element(to, ca as i32, next), PlugValue::String(contents))?;
        next += 1;
        g.remove_element(&PlugRef::element(from, fna as i32, k))?;
        g.remove_element(&PlugRef::element(from, fca as i32, k))?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

/// Where a shape's tags are edited: its original shape when it is deformed, else the shape.
fn injection_node(g: &Graph, shape: NodeId) -> NodeId {
    crate::deform_sets::orig_of(g, shape).unwrap_or(shape)
}

fn shape_of(s: &Session, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let id = crate::rig::find(&s.graph, name, cmd)?;
    crate::components::as_mesh(&s.graph, id).ok_or_else(|| aztek_ops::CommandError::new(format!("{cmd}: {name} is not a mesh")))
}

fn tag_plugs(g: &Graph, id: NodeId) -> CmdResult<(usize, usize)> {
    let t = g.node(id).unwrap().type_arc().clone();
    match (t.attr_index("componentTags.componentTagName"), t.attr_index("componentTags.componentTagContents")) {
        (Some(a), Some(b)) => Ok((a, b)),
        _ => fail!("componentTag: {} holds no component tags", g.path(id)),
    }
}

fn component_tag(s: &mut Session, a: &Args) -> CmdResult {
    let given = objects(s, a);
    let wanted: Vec<String> = match a.get("tagName") {
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Some(Value::String(x)) => vec![x.clone()],
        _ => Vec::new(),
    };
    let new_name = a.string("newTagName", "");
    // Components named with the shape (`cyl.f[0:17]`) say which shape; the rest is the shape itself.
    let mut by_shape: Vec<(NodeId, String)> = Vec::new();
    for n in &given {
        let (obj, comp) = match n.find(".") {
            Some(i) if n[i + 1..].contains('[') => (&n[..i], n[i + 1..].to_string()),
            _ => (n.as_str(), String::new()),
        };
        let shape = shape_of(s, obj, "componentTag")?;
        match by_shape.iter_mut().find(|x| x.0 == shape) {
            Some(x) => {
                x.1.push(' ');
                x.1.push_str(&comp);
            }
            None => by_shape.push((shape, comp)),
        }
    }
    let Some(&(shape, ref comps)) = by_shape.first() else { fail!("componentTag: name or select a mesh or its components") };
    let comps = comps.trim().to_string();
    let g = &mut s.graph;
    let at = injection_node(g, shape);
    let (na, ca) = tag_plugs(g, at)?;
    let held = node_tags(g.node(at).unwrap().data());
    let flowing = g.value(&PlugRef::new(shape, crate::mesh_nodes::MeshNode::OUT_MESH as i32)).ok().and_then(|v| v.as_mesh().cloned());
    let geometry = flowing.clone().map(|m| (*m).clone()).unwrap_or_default();
    let all_names = names(&geometry);
    let matching = |pattern: &str| -> Vec<String> { all_names.iter().filter(|t| glob(pattern, t)).cloned().collect() };
    let set_tag = |g: &mut Graph, name: &str, contents: &str| -> CmdResult<()> {
        let held = node_tags(g.node(at).unwrap().data());
        let k = held.iter().find(|x| x.1 == name).map(|x| x.0).unwrap_or_else(|| held.iter().map(|x| x.0 + 1).max().unwrap_or(0));
        g.set_value(PlugRef::element(at, na as i32, k), PlugValue::String(name.to_string()))?;
        g.set_value(PlugRef::element(at, ca as i32, k), PlugValue::String(contents.to_string()))?;
        Ok(())
    };
    if a.bool("create") {
        let mut name = if new_name.is_empty() { "componentTag1".to_string() } else { new_name.clone() };
        if a.bool("uniqueTagName") || new_name.is_empty() {
            let base = name.trim_end_matches(|c: char| c.is_ascii_digit()).to_string();
            let mut k = 1;
            while all_names.contains(&name) || held.iter().any(|x| x.1 == name) {
                name = format!("{base}{k}");
                k += 1;
            }
        }
        let contents = if comps.is_empty() { "vtx[*]".to_string() } else { comps };
        set_tag(g, &name, &contents)?;
        return Ok(json!(name));
    }
    if a.bool("delete") {
        for p in &wanted {
            for t in matching(p).into_iter().chain(held.iter().filter(|x| glob(p, &x.1)).map(|x| x.1.clone())) {
                match held.iter().find(|x| x.1 == t) {
                    Some(x) => {
                        g.remove_element(&PlugRef::element(at, na as i32, x.0))?;
                        g.remove_element(&PlugRef::element(at, ca as i32, x.0))?;
                    }
                    // Made upstream: an empty one of the same name hides it here.
                    None => set_tag(g, &t, "")?,
                }
            }
        }
        return Ok(json!(true));
    }
    if a.bool("rename") {
        let Some(from) = wanted.first() else { fail!("componentTag: -rename needs -tagName") };
        if new_name.is_empty() {
            fail!("componentTag: -rename needs -newTagName");
        }
        let current = flowing.as_ref().and_then(|m| get(m, from)).map(|s| text(&s).join(" "));
        let Some(contents) = current.or_else(|| held.iter().find(|x| &x.1 == from).map(|x| x.2.clone())) else { fail!("componentTag: no tag {from}") };
        if let Some(x) = held.iter().find(|x| &x.1 == from) {
            g.set_value(PlugRef::element(at, na as i32, x.0), PlugValue::String(new_name.clone()))?;
        } else {
            set_tag(g, &new_name, &contents)?;
        }
        return Ok(json!(true));
    }
    if a.has("modify") {
        let mode = a.string("modify", "replace");
        let Some(pattern) = wanted.first() else { fail!("componentTag: -modify needs -tagName") };
        for t in matching(pattern).into_iter().chain(held.iter().filter(|x| glob(pattern, &x.1)).map(|x| x.1.clone())).collect::<std::collections::BTreeSet<_>>() {
            let now = get(&geometry, &t).unwrap_or_else(|| Sel::empty(&geometry, Cat::V));
            let given = parse(&geometry, &comps).unwrap_or_else(|| Sel::empty(&geometry, now.cat));
            let result = match mode.as_str() {
                "add" => combine(&geometry, &now, &given, Op::Union),
                "remove" => combine(&geometry, &now, &given, Op::Subtract),
                "clear" => Sel::empty(&geometry, now.cat),
                _ => given,
            };
            set_tag(g, &t, &text(&result).join(" "))?;
        }
        return Ok(json!(true));
    }
    if a.bool("query") || a.bool("queryEdit") {
        return Ok(json!({"create": true, "delete": true, "rename": true, "modify": true, "injectionLocation": g.path(at)}));
    }
    fail!("componentTag: give -create, -delete, -rename or -modify")
}

/// The geometry at a plug: `shape.outMesh`, `cluster1.outputGeometry[0]`, or a shape's name.
fn geometry_at(g: &mut Graph, name: &str) -> CmdResult<(PlugRef, std::sync::Arc<Mesh>)> {
    let p = match g.plug(name) {
        Ok(p) => p,
        Err(_) => {
            let id = crate::rig::find(g, name, "geometryAttrInfo")?;
            let m = crate::components::as_mesh(g, id).ok_or_else(|| aztek_ops::CommandError::new(format!("geometryAttrInfo: {name} is not geometry")))?;
            PlugRef::new(m, crate::mesh_nodes::MeshNode::OUT_MESH as i32)
        }
    };
    // A deformer's outputGeometry here is its output.
    let p = match g.node(p.node).and_then(|n| n.ty().attr_index("output")) {
        Some(o) if crate::deform::is_deformer(g.node(p.node).unwrap().ty().name.as_str()) => PlugRef::new(p.node, o as i32),
        _ => p,
    };
    let m = g.value(&p).ok().and_then(|v| v.as_mesh().cloned());
    let Some(m) = m else { fail!("geometryAttrInfo: no geometry at {name}") };
    Ok((p, m))
}

fn geometry_attr_info(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("geometryAttrInfo: name a geometry plug") };
    let g = &mut s.graph;
    let (p, m) = geometry_at(g, &name)?;
    let expr = a.string("componentTagExpression", "");
    let mut sel = if expr.is_empty() { Sel::all() } else { eval(&m, &expr).map_err(|e| aztek_ops::CommandError::new(format!("geometryAttrInfo: {e}")))? };
    for (flag, cat) in [("castToVerts", Cat::V), ("castToEdges", Cat::E), ("castToFaces", Cat::F)] {
        if a.bool(flag) {
            sel = cast(&m, &sel, cat);
        }
    }
    let verts = cast(&m, &sel, Cat::V);
    if a.bool("componentTagNames") {
        return Ok(json!(names(&m)));
    }
    if a.bool("componentTagCategory") {
        return Ok(json!(sel.cat.letter()));
    }
    if a.bool("components") {
        let shown = if sel.cat == Cat::All { cast(&m, &sel, Cat::V) } else { sel.clone() };
        return Ok(json!(text(&shown)));
    }
    if a.bool("elementCount") {
        let shown = if sel.cat == Cat::All { cast(&m, &sel, Cat::V) } else { sel.clone() };
        return Ok(json!(shown.indices().len()));
    }
    if a.bool("pointCount") {
        return Ok(json!(verts.indices().len()));
    }
    if a.bool("pointIndices") {
        return Ok(json!(verts.indices()));
    }
    if a.bool("points") {
        return Ok(json!(verts.indices().iter().flat_map(|&i| {
            let q = m.position(i);
            [q.x, q.y, q.z]
        }).collect::<Vec<f64>>()));
    }
    if a.bool("boundingBox") {
        let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
        for i in verts.indices() {
            lo = lo.min(m.position(i));
            hi = hi.max(m.position(i));
        }
        if lo.x > hi.x {
            return Ok(json!([0.0, 0.0, 0.0, 0.0, 0.0, 0.0]));
        }
        return Ok(json!([lo.x, hi.x, lo.y, hi.y, lo.z, hi.z]));
    }
    if a.bool("subsetState") {
        let n = verts.indices().len();
        let total = (0..m.point_capacity()).filter(|&i| m.point_valid(i)).count();
        return Ok(json!(if n == 0 { 0 } else if n == total { 2 } else { 1 }));
    }
    if a.bool("nodeChain") || a.bool("deformerChain") || a.bool("originalGeometry") {
        // Upstream from the plug: each node's input mesh in turn.
        let mut chain: Vec<NodeId> = Vec::new();
        let mut at = p.node;
        loop {
            chain.insert(0, at);
            let Some(n) = g.node(at) else { break };
            let t = n.type_arc().clone();
            let input = ["inputPolymesh", "inMesh", "inputMesh", "inputGeometry"].iter().find_map(|x| t.attr_index(x));
            let Some(src) = input.and_then(|i| g.source(&PlugRef::new(at, i as i32))) else { break };
            at = src.node;
        }
        if a.bool("originalGeometry") {
            let orig = chain.iter().rev().find(|&&id| crate::deform_sets::is_original(g, id));
            return Ok(json!(orig.map(|&o| format!("{}.outMesh", g.path(o))).unwrap_or_default()));
        }
        if a.bool("deformerChain") {
            chain.retain(|&id| g.node(id).is_some_and(|n| crate::deform::is_deformer(n.ty().name.as_str())));
        }
        return Ok(json!(chain.iter().map(|&id| g.path(id)).collect::<Vec<_>>()));
    }
    if a.bool("matrix") {
        let xf = crate::dag::transform_of(g, p.node);
        let w = crate::rig::world(g, xf);
        return Ok(json!(w.to_cols_array().to_vec()));
    }
    fail!("geometryAttrInfo: give a flag saying what to return")
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "componentTag",
            "Make, change, rename or delete the component tags of a mesh: named groups of its vertices, edges or faces. They are \
             held on the mesh's original shape when it is deformed, else on the mesh. -cr returns the new tag's name.",
            component_tag,
        )
        .positional("a mesh, or its components")
        .flag(f("create", "cr", ArgType::Bool, "Make a tag of the components given (all of them when only the mesh is named)."))
        .flag(f("delete", "d", ArgType::Bool, "Delete the tags -tagName names (wildcards allowed)."))
        .flag(f("rename", "rn", ArgType::Bool, "Rename the tag -tagName to -newTagName."))
        .flag(f("modify", "m", ArgType::String, "replace, add, remove or clear the components of the tags -tagName names."))
        .flag(f("tagName", "tn", ArgType::Any, "The tags to change; wildcards allowed."))
        .flag(f("newTagName", "ntn", ArgType::String, "The new tag's name, or the renamed one's."))
        .flag(f("uniqueTagName", "utn", ArgType::Bool, "With -cr: number the name if it is taken."))
        .flag(f("injectionLocation", "il", ArgType::String, "Where the tag is edited (the original shape, or the mesh)."))
        .flag(f("queryEdit", "qe", ArgType::Bool, "What edits are allowed."))
        .flag(f("query", "q", ArgType::Bool, "Query.")),
    );
    c.register(
        Cmd::new(
            "geometryAttrInfo",
            "What geometry at a plug holds: its tags (-cnm), the components a tag expression names (-cex with -cmp, -pi, -pc, -pnt, -bb, -ec, \
             -ccy), and the nodes it came through (-nch, -dch, -og).",
            geometry_attr_info,
        )
        .positional("a geometry plug, or a shape")
        .flag(f("componentTagExpression", "cex", ArgType::String, "Limit the results to what the expression names."))
        .flag(f("componentTagNames", "cnm", ArgType::Bool, "The tags' names."))
        .flag(f("componentTagCategory", "ccy", ArgType::Bool, "v, e, f, or unknown."))
        .flag(f("components", "cmp", ArgType::Bool, "The components."))
        .flag(f("elementCount", "ec", ArgType::Bool, "How many components."))
        .flag(f("pointCount", "pc", ArgType::Bool, "How many points."))
        .flag(f("pointIndices", "pi", ArgType::Bool, "The points' indices."))
        .flag(f("points", "pnt", ArgType::Bool, "The points' positions, x y z each."))
        .flag(f("boundingBox", "bb", ArgType::Bool, "xmin xmax ymin ymax zmin zmax."))
        .flag(f("subsetState", "sbs", ArgType::Bool, "0 none of the points, 1 some, 2 all."))
        .flag(f("castToVerts", "ctv", ArgType::Bool, "As vertices."))
        .flag(f("castToEdges", "cte", ArgType::Bool, "As edges."))
        .flag(f("castToFaces", "ctf", ArgType::Bool, "As faces."))
        .flag(f("nodeChain", "nch", ArgType::Bool, "The nodes the geometry came through."))
        .flag(f("deformerChain", "dch", ArgType::Bool, "The deformers it came through."))
        .flag(f("originalGeometry", "og", ArgType::Bool, "The original shape's output."))
        .flag(f("matrix", "mtx", ArgType::Bool, "The shape's world matrix.")),
    );
}
