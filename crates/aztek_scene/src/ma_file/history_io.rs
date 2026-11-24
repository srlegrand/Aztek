//! A mesh's construction history in files, as the reference keeps it (#209): the creator (polyCube), each edit
//! (polyBevel3, polyExtrudeFace, polyPlanarProj, polyTweak, transformGeometry...) and the shape, connected in order, with
//! the shape holding no geometry of its own. Read, every node on the way that has a node type here is made as that node
//! and connected as the file connects it; written, the same chain goes out instead of the baked mesh.
//!
//! Settings go by the reference's attribute names, short or long. The few settings a node has here and not in the
//! reference go as hidden added attributes, which the reference keeps without using, so a file written here reads back
//! with every setting. Component lists (`.ics`) name components as the mesh numbers them at that step, so the chain
//! is written only where reading it back numbers them the same: from a creator, or from a stored mesh with no gaps in
//! its numbering. Anything else is baked, as before, with a warning saying why.

use super::*;
use crate::history::{self, INPUT_POLYMESH};

/// A node a mesh's chain is made with as history here: a modelling or UV edit with a node type of its own.
pub(super) fn history_type(ty: &str) -> Option<Arc<NodeType>> {
    let t = types::find(ty)?;
    let edit = t.is_a(history::BASE_TYPE) || crate::uv_ops::is_uv_node(ty);
    (edit && t.attr_index("inputPolymesh").is_some() && t.attr_index("output").is_some()).then_some(t)
}

/// A node that makes a mesh from nothing: polyCube, polyPipe, polyPlatonicSolid...
pub(super) fn creator_type(ty: &str) -> bool {
    creator_attrs(ty).is_some()
        || (ty.starts_with("poly") && types::find(ty).is_some_and(|t| !t.dag && t.attr_index("outMesh").is_some() && t.attr_index("inputPolymesh").is_none() && !t.is_a(history::BASE_TYPE)))
}

/// One step of a mesh's chain, source first.
#[derive(Debug)]
pub(super) enum Step {
    History(usize),
    Tweak(usize),
    Skin(usize),
    Deformer(usize, i64),
    Transform(usize),
    /// An intermediate mesh the chain passes through, holding no geometry: the original before deformers.
    Mesh(usize),
}

#[derive(Debug)]
pub(super) struct Ordered {
    pub creator: Option<usize>,
    /// A mesh upstream with stored geometry, when no creator starts the chain.
    pub base: Option<usize>,
    pub steps: Vec<Step>,
}

/// The chain feeding a mesh shape, step by step, when it holds history that is made as history here and nothing that
/// can't be. None otherwise: the chain is read as before, baked where it has to be.
pub(super) fn walk_ordered(doc: &Doc, shape: usize, stored: &HashMap<usize, MaMesh>) -> Option<Ordered> {
    let mut o = Ordered { creator: None, base: None, steps: Vec::new() };
    let mut seen = HashSet::new();
    let mut at = shape;
    let mut through = 0;
    let file_deformer = |t: &str| deformers::is_file_deformer(t) || crate::scripted::is_plugin_deformer(t);
    while seen.insert(at) {
        let plain = |c: &&Conn| c.dst == at && INPUT_ATTRS.contains(&plain_attr(&c.dst_attr).as_str());
        let by_index = doc.conns_in(at).filter(plain).find(|c| first_index(&c.dst_attr).unwrap_or(0) == through);
        let Some(conn) = by_index.or_else(|| doc.conns_in(at).find(plain)) else { break };
        let next = conn.src;
        let index = first_index(&conn.src_attr).unwrap_or(0);
        let n = &doc.nodes[next];
        through = if file_deformer(&n.ty) || n.ty == "skinCluster" || n.ty == "tweak" { index } else { 0 };
        match n.ty.as_str() {
            "mesh" => {
                if stored.get(&next).is_some_and(MaMesh::has_geometry) {
                    o.base = Some(next);
                    break;
                }
                o.steps.push(Step::Mesh(next));
            }
            "groupParts" | "groupId" | "unitConversion" => {}
            "polyTweak" => o.steps.push(Step::Tweak(next)),
            "skinCluster" => o.steps.push(Step::Skin(next)),
            "transformGeometry" => o.steps.push(Step::Transform(next)),
            t if file_deformer(t) => o.steps.push(Step::Deformer(next, index)),
            // A cut through a face isn't read as one.
            "polySplit" if file_split_points(n).is_none() && !n.adds.iter().any(|a| a.long == "points") => return None,
            t if creator_type(t) => {
                o.creator = Some(next);
                break;
            }
            t if history_type(t).is_some() => o.steps.push(Step::History(next)),
            _ => return None,
        }
        at = next;
    }
    o.steps.reverse();
    // Only a chain with an edit, or an original fed by history, that wasn't read before: cuts alone are read as they
    // were (#180).
    let has_edit = o.steps.iter().any(|s| matches!(s, Step::History(h) if doc.nodes[*h].ty != "polySplit") || matches!(s, Step::Mesh(_)));
    let starts = o.creator.is_some() || o.base.is_some();
    (has_edit && starts).then_some(o)
}

/// The attribute of a history node or creator a file names, short or long: its own names, or the reference's flags.
pub(super) fn setting_index(t: &NodeType, seg: &str) -> Option<usize> {
    if let Some(k) = t.attrs.iter().position(|a| a.name == seg || (!a.short_name.is_empty() && a.short_name == seg)) {
        return Some(k);
    }
    let long = match history::op(&t.name) {
        Some(op) => history::reference_flags(&op).find(|r| r.1 == seg).map(|r| r.0),
        None => crate::mesh_edit::ref_poly::flag_table::table(&t.name).iter().find(|r| r.1 == seg).map(|r| r.0),
    }?;
    t.attrs.iter().position(|a| a.name == long)
}

/// An `.ics` componentList as the string a history node here holds: `"f[0:3] e[5]"`.
fn component_string(values: &[Tok]) -> String {
    values.iter().filter_map(|t| if let Tok::Str(s) = t { Some(s.as_str()) } else { None }).collect::<Vec<_>>().join(" ")
}

impl Build<'_> {
    /// A mesh made from its file chain, step by step, every edit as history.
    pub(super) fn build_ordered(&mut self, i: usize, shape: NodeId, o: Ordered) {
        let doc = self.doc;
        let n = &doc.nodes[i];
        let parent = self.s.graph.parent(shape);
        let start = if let Some(c) = o.creator {
            self.create_generator(c).map(|gen| {
                let out = self.s.graph.node(gen).unwrap().ty().attr_index("outMesh").unwrap();
                PlugRef::new(gen, out as i32)
            })
        } else if let Some(b) = o.base {
            self.used[b] = true;
            let mut warnings = Vec::new();
            let mut m = self.meshes[&b].build(&doc.nodes[b].name, &mut warnings);
            apply_file_tags(&doc.nodes[b], &mut m);
            self.warnings.append(&mut warnings);
            let mt = types::find("mesh").unwrap();
            let name = if Graph::valid_name(&doc.nodes[b].name) { doc.nodes[b].name.clone() } else { String::new() };
            match self.s.graph.create_node(&mt, &name, parent, false) {
                Ok(id) => {
                    self.ids[b] = Some(id);
                    let g = &mut self.s.graph;
                    let _ = g.set_value(PlugRef::new(id, DagNode::INTERMEDIATE_OBJECT as i32), PlugValue::Bool(true));
                    let _ = g.set_value(PlugRef::new(id, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(m))));
                    Some(PlugRef::new(id, MeshNode::OUT_MESH as i32))
                }
                Err(e) => {
                    self.warnings.push(format!("{}: {e}", doc.nodes[b].name));
                    None
                }
            }
        } else {
            None
        };
        let Some(mut prev) = start else {
            self.warnings.push(format!("{}: no geometry found", n.name));
            return;
        };
        for step in &o.steps {
            let made = match *step {
                Step::History(h) => self.create_history(h),
                Step::Tweak(t) => self.create_chain_tweak(t),
                Step::Skin(s) => self.create_skin(s),
                Step::Deformer(d, k) => self.create_deformer(d, k, shape),
                // Made with the plain nodes, matrix and all.
                Step::Transform(t) => {
                    self.used[t] = true;
                    self.ids[t]
                }
                Step::Mesh(m) => {
                    let mt = types::find("mesh").unwrap();
                    let name = if Graph::valid_name(&doc.nodes[m].name) { doc.nodes[m].name.clone() } else { String::new() };
                    let made = self.s.graph.create_node(&mt, &name, parent, false).ok();
                    if let Some(id) = made {
                        self.ids[m] = Some(id);
                        self.used[m] = true;
                        let _ = self.s.graph.set_value(PlugRef::new(id, DagNode::INTERMEDIATE_OBJECT as i32), PlugValue::Bool(true));
                        self.read_node_tags(m, id);
                    }
                    made
                }
            };
            let Some(id) = made else {
                self.warnings.push(format!("{}: a step of its history could not be made; the chain stops there", n.name));
                break;
            };
            let t = self.s.graph.node(id).unwrap().type_arc().clone();
            let (ip, out) = if t.name == "transformGeometry" {
                (t.attr_index("inputGeometry"), t.attr_index("outputGeometry"))
            } else if t.is_a("mesh") {
                (Some(MeshNode::IN_MESH), Some(MeshNode::OUT_MESH))
            } else {
                (t.attr_index("inputPolymesh"), t.attr_index("output"))
            };
            let (Some(ip), Some(out)) = (ip, out) else { break };
            if let Err(e) = self.s.graph.connect(&prev, &PlugRef::new(id, ip as i32), false) {
                self.warnings.push(format!("{}: {e}", self.s.graph.path(id)));
            }
            prev = PlugRef::new(id, out as i32);
        }
        if let Err(e) = self.s.graph.connect(&prev, &PlugRef::new(shape, MeshNode::IN_MESH as i32), false) {
            self.warnings.push(format!("{}: {e}", n.name));
        }
        self.read_node_tags(i, shape);
    }

    /// A mesh node's `.gtag` as the shape's own tags, added to what its history makes.
    fn read_node_tags(&mut self, i: usize, id: NodeId) {
        let n = &self.doc.nodes[i];
        let mut tags: BTreeMap<i64, (String, String)> = BTreeMap::new();
        for set in &n.sets {
            let [g, field] = set.path.as_slice() else { continue };
            if !matches!(g.name.as_str(), "gtag" | "componentTags") {
                continue;
            }
            let Some((k, _)) = g.range else { continue };
            let e = tags.entry(k).or_default();
            match field.name.as_str() {
                "gtagnm" | "componentTagName" => e.0 = set.values.first().and_then(Tok::text).unwrap_or_default().to_string(),
                "gtagcmp" | "componentTagContents" => e.1 = component_string(&set.values),
                _ => {}
            }
        }
        let t = self.s.graph.node(id).unwrap().type_arc().clone();
        let (Some(na), Some(ca)) = (t.attr_index("componentTags.componentTagName"), t.attr_index("componentTags.componentTagContents")) else { return };
        for (k, (name, contents)) in tags {
            if name.is_empty() {
                continue;
            }
            let _ = self.s.graph.set_value(PlugRef::element(id, na as i32, k as i32), PlugValue::String(name));
            let _ = self.s.graph.set_value(PlugRef::element(id, ca as i32, k as i32), PlugValue::String(contents));
        }
    }

    /// A file's history node as the node of that type here, with its settings.
    fn create_history(&mut self, i: usize) -> Option<NodeId> {
        let n = &self.doc.nodes[i];
        let t = history_type(&n.ty)?;
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let id = match self.s.graph.create_node(&t, &name, NO_NODE, false) {
            Ok(id) => id,
            Err(e) => {
                self.warnings.push(format!("{}: {e}", n.name));
                return None;
            }
        };
        self.ids[i] = Some(id);
        self.used[i] = true;
        self.history_settings(i, id, &t);
        Some(id)
    }

    /// The setAttr lines of a history node or creator: by the reference's names, short or long, the component list
    /// as the string the node holds, the matrix the edit was made under as its inverse here.
    pub(super) fn history_settings(&mut self, i: usize, id: NodeId, t: &NodeType) {
        let doc = self.doc;
        let n = &doc.nodes[i];
        let op = history::op(&t.name);
        let creator = t.attr_index("upAxis").is_some() && op.is_none();
        let resolve = |seg: &str| setting_index(t, seg);
        let path = self.s.graph.path(id);
        for set in &n.sets {
            if set.values.is_empty() || set.path.is_empty() {
                continue;
            }
            let first = set.path[0].name.as_str();
            // A creator's axis and its other settings the reader takes by the reference's names.
            if creator && (matches!(first, "ax" | "axis") || creator_attrs(&t.name).is_some_and(|c| c.iter().any(|(short, ours)| *short == first || *ours == first))) {
                continue;
            }
            // The world matrix the edit was made under: here its inverse.
            if matches!(first, "ix" | "inputMatrix") && set.path.len() == 1 {
                if let (Some(k), Some(m)) = (t.attr_index("worldInverse"), matrix_from(&set.values)) {
                    let text = Value::Array(m.inverse().to_cols_array().iter().map(|x| json!(x)).collect()).to_string();
                    let _ = self.s.graph.set_value(PlugRef::new(id, k as i32), PlugValue::String(text));
                }
                continue;
            }
            let Some(k) = resolve(first) else {
                self.note_drop(&n.ty, first, "setAttr");
                continue;
            };
            let def = t.attrs[k].clone();
            if !def.has(flags::INPUT) {
                continue;
            }
            if set.path.len() == 1 && set.path[0].range.is_none() {
                let v = if def.name == "inputComponents" {
                    Some(PlugValue::String(component_string(&set.values)))
                } else if def.ty == PlugType::Data && def.category.is_empty() && crate::array_data::of(&def.default_value).is_none() {
                    // A setting kept here as data, written as its JSON in an added string attribute.
                    set.values.first().and_then(Tok::text).and_then(|s| serde_json::from_str::<Value>(s).ok()).and_then(|j| {
                        let kind = j.get("kind")?.as_str()?.to_string();
                        let load = crate::plug::data::find(&kind)?;
                        load(j.get("value")?).ok()
                    }).map(|d| PlugValue::Data(Some(d)))
                } else if def.ty == PlugType::Vec3 && set.values.len() >= 3 {
                    Some(PlugValue::Vec3(tok_vec3(&set.values)))
                } else {
                    plug_value(&def, -1, &set.values, 1.0)
                };
                if let Some(v) = v {
                    if let Err(e) = self.s.graph.set_value(PlugRef::new(id, k as i32), v) {
                        self.warnings.push(format!("{}.{first}: {e}", n.name));
                    }
                }
                continue;
            }
            // Elements and children, by name: .tcp[0].tcpv.
            let (lo, hi) = set.path.last().and_then(|s| s.range).or(set.path.first().and_then(|s| s.range)).unwrap_or((-1, -1));
            let count = (hi - lo + 1).max(1) as usize;
            let stride = (set.values.len() / count).max(1);
            for (e, chunk) in set.values.chunks(stride).enumerate().take(count) {
                let text: Vec<String> = set
                    .path
                    .iter()
                    .enumerate()
                    .map(|(s, seg)| {
                        let name = if s == 0 { def.name.clone() } else { seg.name.clone() };
                        match seg.range {
                            Some(r) => format!("{name}[{}]", r.0 + e as i64),
                            None => name,
                        }
                    })
                    .collect();
                let Ok(p) = self.s.graph.plug(&format!("{path}.{}", text.join("."))) else {
                    self.note_drop(&n.ty, &text.join("."), "setAttr");
                    continue;
                };
                let d = self.s.graph.attr(&p).clone();
                if let Some(v) = plug_value(&d, p.component, chunk, 1.0) {
                    let _ = self.s.graph.set_value(p, v);
                }
            }
        }
        // A file of the reference's: the cut's points from its edges and fractions, when no setting here gave them.
        if t.name == "polySplit" && !n.adds.iter().any(|a| a.long == "points") {
            if let (Some(pts), Some(k)) = (file_split_points(n), t.attr_index("points")) {
                let _ = self.s.graph.set_value(PlugRef::new(id, k as i32), PlugValue::String(Value::Array(pts).to_string()));
            }
        }
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing

/// Where a written chain starts.
pub(super) enum Base {
    Creator(NodeId),
    /// A mesh held as geometry: an intermediate shape or a mesh source node, and its mesh.
    Stored(NodeId, Arc<Mesh>),
}

pub(super) struct Chain {
    pub base: Base,
    /// History, tweaks, transformGeometry and pass-through intermediate meshes, source first.
    pub steps: Vec<NodeId>,
}

/// Whether a mesh written as stored geometry reads back numbered as it is here: no gaps, edges in order and direction.
fn reads_back(m: &Mesh) -> bool {
    if m.num_points() != m.point_capacity() || m.num_faces() != m.face_capacity() || m.num_edges() != m.edge_capacity() {
        return false;
    }
    let st = stored_mesh(m);
    st.edges.iter().enumerate().all(|(e, &(a, b, _))| m.edge_point(e as Index, 0) as i64 == a && m.edge_point(e as Index, 1) as i64 == b)
}

/// The value of a node's attribute that isn't the reference's, as added-attribute lines: hidden, so the reference keeps
/// it without showing it. Data goes as its JSON.
fn hidden_lines(a: &AttrDef, short: &str, v: &PlugValue) -> String {
    let mut o = String::new();
    let add = |o: &mut String, kind: &str| {
        let _ = writeln!(o, "\taddAttr -ci true -h true -sn {} -ln {} {kind};", quoted(short), quoted(&a.name));
    };
    match v {
        PlugValue::Bool(b) => {
            add(&mut o, "-at \"bool\"");
            let _ = writeln!(o, "\tsetAttr \".{short}\" {};", if *b { "yes" } else { "no" });
        }
        PlugValue::Int(x) => {
            add(&mut o, "-at \"long\"");
            let _ = writeln!(o, "\tsetAttr \".{short}\" {x};");
        }
        PlugValue::Float(x) => {
            add(&mut o, "-at \"double\"");
            let _ = writeln!(o, "\tsetAttr \".{short}\" {};", fmt_num(*x));
        }
        PlugValue::Vec3(x) => {
            add(&mut o, "-at \"double3\"");
            for axis in ["X", "Y", "Z"] {
                let _ = writeln!(o, "\taddAttr -ci true -h true -sn {} -ln {} -at \"double\" -p {};", quoted(&format!("{short}{}", axis.to_lowercase())), quoted(&format!("{}{axis}", a.name)), quoted(&a.name));
            }
            let _ = writeln!(o, "\tsetAttr \".{short}\" -type \"double3\" {} ;", vec_text(*x));
        }
        PlugValue::String(s) => {
            add(&mut o, "-dt \"string\"");
            let _ = writeln!(o, "\tsetAttr \".{short}\" -type \"string\" {};", quoted(s));
        }
        PlugValue::Data(Some(d)) => {
            add(&mut o, "-dt \"string\"");
            let text = json!({"kind": d.kind(), "value": d.to_json()}).to_string();
            let _ = writeln!(o, "\tsetAttr \".{short}\" -type \"string\" {};", quoted(&text));
        }
        _ => {}
    }
    o
}

/// Attributes every node has, in the reference as here.
const NODE_ATTRS: &[&str] = &["caching", "frozen", "isHistoricallyInteresting", "nodeState", "binMembership"];

impl Writer<'_> {
    /// The chain feeding `input` back to where it starts, when all of it can go in the file as history. Ok(None) when
    /// nothing feeds it; Err with what is in the way when something can't.
    pub(super) fn history_chain(&self, input: PlugRef) -> Result<Option<Chain>, String> {
        let g = &self.s.graph;
        let mut steps = Vec::new();
        let mut p = input;
        let base = loop {
            let Some(src) = g.source(&p) else {
                if steps.is_empty() {
                    return Ok(None);
                }
                return Err("an edit has nothing feeding it".into());
            };
            let Some(n) = g.node(src.node) else { return Err("a missing node".into()) };
            let t = n.type_arc().clone();
            let id = src.node;
            if steps.contains(&id) {
                return Err("its history loops".into());
            }
            if history_type(&t.name).is_some() || t.name == history::TWEAK_TYPE {
                steps.push(id);
                p = PlugRef::new(id, INPUT_POLYMESH as i32);
            } else if t.name == "transformGeometry" {
                steps.push(id);
                p = PlugRef::new(id, t.attr_index("inputGeometry").unwrap() as i32);
            } else if t.is_a("mesh") && n.bool(DagNode::INTERMEDIATE_OBJECT) {
                let inner = PlugRef::new(id, MeshNode::IN_MESH as i32);
                if g.source(&inner).is_some() {
                    steps.push(id);
                    p = inner;
                } else {
                    let Some(m) = n.value(MeshNode::IN_MESH).as_mesh().cloned() else { return Err(format!("{} holds no mesh", g.path(id))) };
                    break Base::Stored(id, m);
                }
            } else if t.name == "meshSource" {
                let Some(m) = n.value(t.attr_index("mesh").unwrap()).as_mesh().cloned() else { return Err("a mesh source holds no mesh".into()) };
                break Base::Stored(id, m);
            } else if creator_type(&t.name) {
                break Base::Creator(id);
            } else {
                return Err(format!("{} ({}) has no node in the reference's files", g.path(id), t.name));
            }
        };
        if let Base::Stored(id, m) = &base {
            if !reads_back(m) {
                return Err(format!("{} is numbered with gaps, which its history's component lists would not survive", g.path(*id)));
            }
        }
        steps.reverse();
        Ok(Some(Chain { base, steps }))
    }

    /// The chain as the reference's nodes, into `dest`. DAG lines (intermediate meshes) go out now under `parent`; the
    /// other nodes after the DAG.
    pub(super) fn write_history(&mut self, chain: &Chain, shape_path: &str, parent: NodeId, dest: &str) {
        let mut prev = match &chain.base {
            // A creator feeding more than one mesh is written once.
            Base::Creator(id) if self.history_written.contains(id) => format!("{}.out", self.s.graph.path(*id)),
            Base::Creator(id) => {
                let text = self.creator_text(*id);
                self.pending_nodes_push(text);
                self.history_written.insert(*id);
                format!("{}.out", self.s.graph.path(*id))
            }
            Base::Stored(id, _) if self.history_written.contains(id) => format!("{}.o", self.s.graph.path(*id)),
            Base::Stored(id, m) => {
                // An intermediate shape keeps its name; a mesh source node becomes one.
                let (name, path) = match self.s.graph.node(*id) {
                    Some(n) if n.ty().is_a("mesh") => (n.name().to_string(), self.s.graph.path(*id)),
                    _ => {
                        let name = self.fresh("polySurfaceShape1");
                        (name.clone(), name)
                    }
                };
                self.create_line("mesh", &name, parent);
                let _ = writeln!(self.out, "\tsetAttr -k off \".v\";");
                let _ = writeln!(self.out, "\tsetAttr \".io\" yes;");
                write_stored(&mut self.out, &stored_mesh(m));
                self.history_written.insert(*id);
                format!("{path}.o")
            }
        };
        for &id in &chain.steps {
            let t = self.s.graph.node(id).unwrap().type_arc().clone();
            let path = self.s.graph.path(id);
            // Shared with a mesh written before: already there, connected to what feeds it.
            if !self.history_written.insert(id) {
                prev = if t.is_a("mesh") {
                    format!("{path}.o")
                } else if t.name == "transformGeometry" {
                    format!("{path}.og")
                } else {
                    format!("{path}.out")
                };
                continue;
            }
            if t.is_a("mesh") {
                let name = self.s.graph.node(id).unwrap().name().to_string();
                self.create_line("mesh", &name, parent);
                let _ = writeln!(self.out, "\tsetAttr -k off \".v\";");
                let _ = writeln!(self.out, "\tsetAttr \".io\" yes;");
                self.node_tag_lines(id);
                self.conn(&prev, &format!("{path}.i"));
                prev = format!("{path}.o");
            } else if t.name == "transformGeometry" {
                let mut text = format!("createNode transformGeometry -n {};\n", quoted(&path));
                text.push_str(&self.plain_attr_lines(id));
                self.pending_nodes_push(text);
                self.conn(&prev, &format!("{path}.ig"));
                prev = format!("{path}.og");
            } else if t.name == history::TWEAK_TYPE {
                let mut text = format!("createNode polyTweak -n {};\n", quoted(&path));
                let saved = history::tweaks_json(&self.s.graph, id);
                let mut points: Vec<(i64, DVec3)> = saved.get("points").and_then(Value::as_array).into_iter().flatten().map(|e| {
                    let c = |k: usize| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                    (c(0) as i64, DVec3::new(c(1), c(2), c(3)))
                }).collect();
                points.sort_by_key(|p| p.0);
                if let Some(last) = points.last() {
                    let _ = writeln!(text, "\tsetAttr -s {} \".tk\";", last.0 + 1);
                }
                for (p, v) in points {
                    let _ = writeln!(text, "\tsetAttr \".tk[{p}]\" -type \"float3\" {};", vec_text(v));
                }
                self.pending_nodes_push(text);
                self.conn(&prev, &format!("{path}.ip"));
                prev = format!("{path}.out");
            } else {
                let text = self.history_text(id);
                self.pending_nodes_push(text);
                self.conn(&prev, &format!("{path}.ip"));
                if t.attr_index("worldInverse").is_some() {
                    self.conn(&format!("{shape_path}.wm"), &format!("{path}.mp"));
                }
                prev = format!("{path}.out");
            }
        }
        self.conn(&prev, dest);
    }

    /// A shape's own component tags, which it adds to the geometry its history makes: `.gtag` as the reference has them.
    pub(super) fn node_tag_lines(&mut self, id: NodeId) {
        let Some(n) = self.s.graph.node(id) else { return };
        let tags = crate::component_tags::node_tags(n.data());
        if tags.is_empty() {
            return;
        }
        let _ = writeln!(self.out, "\tsetAttr -s {} \".gtag\";", tags.len());
        for (k, (_, name, contents)) in tags.iter().enumerate() {
            let _ = writeln!(self.out, "\tsetAttr \".gtag[{k}].gtagnm\" -type \"string\" {};", quoted(name));
            let list: Vec<String> = contents.split_whitespace().map(quoted).collect();
            let _ = writeln!(self.out, "\tsetAttr \".gtag[{k}].gtagcmp\" -type \"componentList\" {} {};", list.len(), list.join(" "));
        }
    }

    /// A connection at a node written as history: the mesh down the chain went with the chain; anything else (keys on
    /// a setting) by attribute name.
    pub(super) fn history_connection(&mut self, src: PlugRef, dst: PlugRef) {
        let g = &self.s.graph;
        let geometry = |p: &PlugRef| {
            let a = g.attr(p);
            a.ty == PlugType::Mesh || (g.node(p.node).is_some_and(|n| n.ty().name == "transformGeometry") && matches!(a.name.as_str(), "inputGeometry" | "outputGeometry"))
        };
        if geometry(&src) || geometry(&dst) {
            return;
        }
        let written = |id: NodeId| {
            let Some(n) = g.node(id) else { return false };
            let ty = n.ty().name.as_str();
            self.history_written.contains(&id) || (self.keep.contains(&id) && (self.dag_written(id) || anim::is_curve(g, id) || DRIVEN_CURVES.contains(&ty) || is_plain_node(ty) || ty == "unitConversion"))
        };
        if !written(src.node) || !written(dst.node) {
            return;
        }
        let from = if anim::is_curve(g, src.node) { format!("{}.o", g.path(src.node)) } else { g.plug_name(&src) };
        let to = g.plug_name(&dst);
        self.conn(&from, &to);
    }

    /// The name an attribute of a written history node or creator goes by: the reference's short name when it has
    /// one, else its long name. None for one the reference doesn't have.
    fn reference_name(&self, t: &NodeType, a: &AttrDef) -> Option<String> {
        if NODE_ATTRS.contains(&a.name.as_str()) {
            return Some(if a.short_name.is_empty() { a.name.clone() } else { a.short_name.clone() });
        }
        if let Some((short, _)) = creator_attrs(&t.name).and_then(|c| c.iter().find(|(_, ours)| *ours == a.name)) {
            return Some(short.to_string());
        }
        let table = crate::mesh_edit::ref_poly::flag_table::table;
        let row = match history::op(&t.name) {
            Some(op) => history::reference_flags(&op).find(|r| r.0 == a.name).copied(),
            None => table(&t.name).iter().find(|r| r.0 == a.name).copied(),
        };
        // UV nodes here are made with the reference's attributes; uvEdit is this application's own.
        let uv = crate::uv_ops::is_uv_node(&t.name) && a.name != "uvEdit";
        if row.is_none() && !uv {
            return None;
        }
        let short = row.map(|r| r.1).filter(|s| !s.is_empty()).unwrap_or(&a.short_name);
        Some(if short.is_empty() { a.name.clone() } else { short.to_string() })
    }

    /// A history node's createNode and settings.
    fn history_text(&self, id: NodeId) -> String {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let t = n.type_arc().clone();
        let mut o = format!("createNode {} -n {};\n", t.name, quoted(&g.path(id)));
        for (k, a) in t.attrs.iter().enumerate() {
            if !a.has(flags::INPUT) || !a.has(flags::STORABLE) || a.ty == PlugType::Mesh || a.is_array() {
                continue;
            }
            let p = PlugRef::new(id, k as i32);
            if g.source(&p).is_some() {
                continue;
            }
            let v = n.value(k);
            if a.name == "inputComponents" {
                if let PlugValue::String(s) = v {
                    let parts: Vec<String> = s.split_whitespace().map(quoted).collect();
                    if !parts.is_empty() {
                        let _ = writeln!(o, "\tsetAttr \".ics\" -type \"componentList\" {} {};", parts.len(), parts.join(" "));
                    }
                }
                continue;
            }
            if a.name == "worldInverse" {
                if let PlugValue::String(s) = v {
                    let cols: Vec<f64> = serde_json::from_str::<Vec<f64>>(s).unwrap_or_default();
                    if cols.len() == 16 {
                        let world = DMat4::from_cols_array(&cols.try_into().unwrap()).inverse();
                        let _ = writeln!(o, "\tsetAttr \".ix\" -type \"matrix\" {};", matrix_text(&world));
                    }
                }
                continue;
            }
            if v == &a.default_value {
                continue;
            }
            match self.reference_name(&t, a) {
                Some(name) => o.push_str(&value_line(&name, a, v)),
                None => o.push_str(&hidden_lines(a, &a.name, v)),
            }
        }
        // The reference's form of a cut along edges: each point's fraction in .e and its edge in .d.
        if t.name == "polySplit" {
            let pts = t.attr_index("points").and_then(|k| match n.value(k) {
                PlugValue::String(s) => serde_json::from_str::<Vec<Value>>(s).ok(),
                _ => None,
            });
            if let Some(pts) = pts.filter(|p| !p.is_empty()) {
                let edges: Option<Vec<(i64, f64)>> = pts.iter().map(|p| Some((p.get("edge")?.as_i64()?, p.get("t")?.as_f64()?))).collect();
                if let Some(edges) = edges {
                    let last = edges.len() - 1;
                    let fr: Vec<String> = edges.iter().map(|e| fmt_num(e.1)).collect();
                    let codes: Vec<String> = edges.iter().map(|e| (e.0 - (1i64 << 31)).to_string()).collect();
                    let _ = writeln!(o, "\tsetAttr -s {} \".e[0:{last}]\"  {};", edges.len(), fr.join(" "));
                    let _ = writeln!(o, "\tsetAttr -s {} \".d[0:{last}]\"  {};", edges.len(), codes.join(" "));
                }
            }
        }
        o
    }

    /// A creator's createNode and settings: the up axis as the reference's axis vector.
    fn creator_text(&self, id: NodeId) -> String {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let t = n.type_arc().clone();
        let mut o = format!("createNode {} -n {};\n", t.name, quoted(&g.path(id)));
        for (k, a) in t.attrs.iter().enumerate() {
            if !a.has(flags::INPUT) || !a.has(flags::STORABLE) || a.ty == PlugType::Mesh || a.is_array() {
                continue;
            }
            if g.source(&PlugRef::new(id, k as i32)).is_some() {
                continue;
            }
            let v = n.value(k);
            // A primitive's own settings go out even at their defaults here, which need not be the reference's
            // (createUVs); the node's general ones only when changed.
            let own = !NODE_ATTRS.contains(&a.name.as_str()) && self.reference_name(&t, a).is_some();
            if v == &a.default_value && !(own && a.name != "upAxis") {
                continue;
            }
            if a.name == "upAxis" {
                // A turn of its own given as a vector wins, as it does in the mesh.
                if t.attr_index("axis").is_some_and(|x| n.value(x) != &t.attrs[x].default_value) {
                    continue;
                }
                let i = v.as_int();
                let mut axis = DVec3::ZERO;
                axis[(i % 3) as usize] = if i >= 3 { -1.0 } else { 1.0 };
                let _ = writeln!(o, "\tsetAttr \".ax\" -type \"double3\" {} ;", vec_text(axis));
                continue;
            }
            if a.name == "axis" {
                // Only a turn of its own; the up axis gives the usual ones.
                if let (PlugValue::Vec3(x), false) = (v, v == &a.default_value) {
                    let _ = writeln!(o, "\tsetAttr \".ax\" -type \"double3\" {} ;", vec_text(*x));
                }
                continue;
            }
            match self.reference_name(&t, a) {
                Some(name) => o.push_str(&value_line(&name, a, v)),
                None => o.push_str(&hidden_lines(a, &a.name, v)),
            }
        }
        o
    }
}

/// One setAttr line for a value.
fn value_line(name: &str, a: &AttrDef, v: &PlugValue) -> String {
    match v {
        PlugValue::Bool(b) => format!("\tsetAttr \".{name}\" {};\n", if *b { "yes" } else { "no" }),
        PlugValue::Int(x) => format!("\tsetAttr \".{name}\" {x};\n"),
        PlugValue::Float(x) => format!("\tsetAttr \".{name}\" {};\n", fmt_num(units::to_user(a.unit, *x))),
        PlugValue::Vec3(x) => format!("\tsetAttr \".{name}\" -type \"double3\" {} ;\n", vec_text(*x)),
        PlugValue::String(s) => format!("\tsetAttr \".{name}\" -type \"string\" {};\n", quoted(s)),
        PlugValue::Matrix(m) => format!("\tsetAttr \".{name}\" -type \"matrix\" {};\n", matrix_text(m)),
        PlugValue::Data(Some(_)) => hidden_lines(a, name, v),
        _ => String::new(),
    }
}
