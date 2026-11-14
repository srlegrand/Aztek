//! Deformers in .ma, as the reference lays them out: an intermediate Orig shape, then for each deformer a groupParts holding its
//! points, the deformer itself, and an objectSet; .wl for weights; blendShape targets under .it. A mesh's chain is read
//! and written source first. Node names are the reference's, with one difference inside: a deformer on several meshes is, in the reference,
//! one node with several geometry indices, and one chain node per mesh here. Files from the reference take that form on reading;
//! written files keep one node per mesh, with the followers' settings connected from the leader.
//!
//! What the reference's attributes can't hold (a wrap's binding, a softMod's points) goes in a string attribute, aztekState, which
//! The reference keeps as any added attribute. Without it a deformer reads from the reference's attributes alone.

use super::*;
use crate::deform::{self, Floats, DATA, GEOM, HANDLE, IN, OUT, WEIGHTS};
use crate::deform_blend::{self, BlendData, BlendItem, BlendTarget};
use crate::deform_lattice::{self, LatticeGrid};

const STATE: &[&str] = &["azs", "aztekState"];

/// The reference's nonlinear handle shapes and the deformers they stand for.
const HANDLE_SHAPES: &[(&str, &str)] = &[("clusterHandle", "cluster"), ("deformBend", "bend"), ("deformTwist", "twist"), ("deformSquash", "squash"), ("deformFlare", "flare"), ("deformSine", "sine"), ("deformWave", "wave")];

fn handle_kind(shape_ty: &str) -> Option<&'static str> {
    HANDLE_SHAPES.iter().find(|x| x.0 == shape_ty).map(|x| x.1)
}


/// Whether a node in a file is a deformer other than a skinCluster.
pub(super) fn is_file_deformer(ty: &str) -> bool {
    ty == "nonLinear" || deform::is_deformer(ty)
}

/// A file's transform that is a deformer's handle: the kind, from its shape.
pub(super) fn handle_under(doc: &Doc, i: usize) -> Option<(usize, &'static str)> {
    doc.children(i).find_map(|k| handle_kind(&doc.nodes[k].ty).map(|kind| (k, kind)))
}

fn state_of(n: &MaNode) -> Value {
    n.text(STATE).and_then(|t| serde_json::from_str(t).ok()).unwrap_or(Value::Null)
}

/// The type a file's deformer is made as. The reference's nonLinear says which by the handle shape feeding .dd.
fn make_type(doc: &Doc, di: usize) -> Option<String> {
    let n = &doc.nodes[di];
    if n.ty != "nonLinear" {
        return Some(n.ty.clone());
    }
    if let Some(t) = state_of(n).get("type").and_then(Value::as_str) {
        return Some(t.to_string());
    }
    let shape = doc.input(di, &["dd", "deformerData"])?.src;
    handle_kind(&doc.nodes[shape].ty).map(str::to_string)
}

/// `1 "vtx[*]"` or `2 "vtx[0:3]" "vtx[7]"`.
fn component_text(ids: &[i64]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let mut k = 0;
    while k < ids.len() {
        let mut end = k;
        while end + 1 < ids.len() && ids[end + 1] == ids[end] + 1 {
            end += 1;
        }
        parts.push(if end == k { format!("\"vtx[{}]\"", ids[k]) } else { format!("\"vtx[{}:{}]\"", ids[k], ids[end]) });
        k = end + 1;
    }
    format!("{} {}", parts.len(), parts.join(" "))
}

/// The points a componentList names, in order. `count` answers vtx[*].
fn component_ids(values: &[Tok], count: u32) -> Vec<u32> {
    let mut out = Vec::new();
    for t in values {
        let Some(text) = t.text() else { continue };
        if let Some((u, v)) = text.strip_prefix("cv[").and_then(|x| x.split_once("][")) {
            // A surface's CVs: only every one of them is understood here.
            if u == "*" && v == "*]" {
                out.extend(0..count);
            }
            continue;
        }
        let Some(inner) = text.strip_prefix("vtx[").or_else(|| text.strip_prefix("cp[")).or_else(|| text.strip_prefix("cv[")).and_then(|x| x.strip_suffix(']')) else { continue };
        if inner == "*" {
            out.extend(0..count);
        } else if let Some((a, b)) = inner.split_once(':') {
            if let (Ok(a), Ok(b)) = (a.parse::<u32>(), b.parse::<u32>()) {
                out.extend(a..=b.min(count.saturating_sub(1)));
            }
        } else if let Ok(a) = inner.parse() {
            out.push(a);
        }
    }
    out
}

/// A path as it is written under .it: the segment names and their first indices.
fn matches_path<'a>(s: &'a SetAttr, names: &[&[&str]]) -> Option<&'a [Seg]> {
    (s.path.len() == names.len() && s.path.iter().zip(names).all(|(seg, want)| want.contains(&seg.name.as_str()))).then_some(s.path.as_slice())
}

/// A tweak node's stored point moves, in either of the forms the reference writes.
pub(super) fn apply_vertex_tweaks(doc: &Doc, m: &mut Mesh, tweak: usize) {
    for s in &doc.nodes[tweak].sets {
        let Some(path) = matches_path(s, &[&["vl", "vlist"], &["vt", "vertex"]]).or_else(|| matches_path(s, &[&["pl", "plist"], &["cp", "controlPoints"]])) else { continue };
        let start = path[1].range.map_or(0, |r| r.0 as usize);
        for (k, c) in s.values.chunks_exact(3).enumerate() {
            let at = start + k;
            if at < m.point_capacity() as usize && m.point_valid(at as Index) {
                let p = m.position(at as Index) + tok_vec3(c);
                m.set_position(at as Index, p);
            }
        }
    }
}

/// A tweak node's stored moves for one geometry, by point.
pub(super) fn tweak_moves(doc: &Doc, tweak: usize, geometry: i64) -> Vec<(i64, DVec3)> {
    let mut out = Vec::new();
    for s in &doc.nodes[tweak].sets {
        let Some(path) = matches_path(s, &[&["vl", "vlist"], &["vt", "vertex"]]).or_else(|| matches_path(s, &[&["pl", "plist"], &["cp", "controlPoints"]])) else { continue };
        if path[0].range.map_or(0, |r| r.0) != geometry {
            continue;
        }
        let start = path[1].range.map_or(0, |r| r.0);
        for (k, c) in s.values.chunks_exact(3).enumerate() {
            out.push((start + k as i64, tok_vec3(c)));
        }
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Reading

impl Build<'_> {
    /// Makes the node for one geometry index of a file's deformer. The first is the leader.
    pub(super) fn create_deformer(&mut self, di: usize, k: i64, shape: NodeId) -> Option<NodeId> {
        let doc = self.doc;
        let n = &doc.nodes[di];
        let Some(ty) = make_type(doc, di) else {
            self.warnings.push(format!("{}: a nonlinear deformer with no handle shape", n.name));
            return None;
        };
        // A nonlinear kind is the one nonLinear type, with that kind's settings added.
        let kind = deform::NONLINEAR.contains(&ty.as_str()).then(|| ty.clone());
        let ty = if kind.is_some() { "nonLinear".to_string() } else { ty };
        let Some(t) = types::find(&ty).filter(|_| deform::is_deformer(&ty) || crate::scripted::is_plugin_deformer(&ty)) else {
            self.warnings.push(format!("{}: {ty} is not a deformer here", n.name));
            return None;
        };
        let name = if Graph::valid_name(&n.name) { n.name.clone() } else { String::new() };
        let id = match self.s.graph.create_node(&t, &name, NO_NODE, false) {
            Ok(id) => id,
            Err(e) => {
                self.warnings.push(format!("{}: {e}", n.name));
                return None;
            }
        };
        if let Some(kind) = &kind {
            if let Err(e) = deform::add_nonlinear_attrs(&mut self.s.graph, id, kind) {
                self.warnings.push(format!("{}: {e}", n.name));
            }
        }
        if self.ids[di].is_none() {
            self.ids[di] = Some(id);
        }
        self.used[di] = true;
        self.deformers.push(FileDeformer { doc: di, geometry: k, id, shape });
        Some(id)
    }

    /// A polyTweak below a deformer: points moved after it, as the reference keeps them.
    pub(super) fn create_chain_tweak(&mut self, i: usize) -> Option<NodeId> {
        let n = &self.doc.nodes[i];
        crate::history::register_tweak_type();
        let t = types::find(crate::history::TWEAK_TYPE)?;
        let id = self.s.graph.create_node(&t, &n.name, NO_NODE, false).ok()?;
        self.ids[i] = Some(id);
        self.used[i] = true;
        let points: Vec<Value> = n.elements(&["tk", "tweak"], 3).into_iter().map(|(at, c)| { let v = tok_vec3(c); json!([at, v.x, v.y, v.z]) }).collect();
        let _ = crate::history::set_tweaks_json(&mut self.s.graph, id, Some(&json!({"points": points})));
        Some(id)
    }

    /// Everything a deformer holds, once what it reads exists. Chain order, so each sees the input it binds to.
    pub(super) fn fill_deformers(&mut self) {
        for k in 0..self.deformers.len() {
            let d = self.deformers[k];
            self.fill_deformer(d);
        }
    }

    fn scalar_sets(&mut self, n: &MaNode, id: NodeId) {
        let path = self.s.graph.path(id);
        for set in &n.sets {
            let [seg] = set.path.as_slice() else { continue };
            if set.values.is_empty() || matches!(seg.name.as_str(), "azs" | "aztekState" | "wl" | "weightList" | "aal" | "attributeAliasList" | "gm" | "geomMatrix") {
                continue;
            }
            let (lo, hi) = seg.range.unwrap_or((-1, -1));
            let count = (hi - lo + 1).max(1) as usize;
            let stride = (set.values.len() / count).max(1);
            for (e, chunk) in set.values.chunks(stride).enumerate().take(count) {
                let text = if seg.range.is_some() { format!("{path}.{}[{}]", seg.name, lo + e as i64) } else { format!("{path}.{}", seg.name) };
                let Ok(p) = self.s.graph.plug(&text) else { continue };
                // A driven plug's value is what drives it now; setting one would hold it there.
                if self.s.graph.source(&p).is_some() {
                    continue;
                }
                let def = self.s.graph.attr(&p).clone();
                if let Some(v) = plug_value(&def, p.component, chunk, 1.0) {
                    let _ = self.s.graph.set_value(p, v);
                }
            }
        }
    }

    /// What a plug-in's deformer holds under its own compounds: `.bindData[0].sampleWeights[3]`.
    fn nested_sets(&mut self, n: &MaNode, id: NodeId) {
        let path = self.s.graph.path(id);
        for set in &n.sets {
            if set.values.is_empty() || set.path.len() < 2 || matches!(set.path[0].name.as_str(), "ip" | "input" | "wl" | "weightList") {
                continue;
            }
            let (lo, hi) = set.path.iter().rev().find_map(|s| s.range).unwrap_or((-1, -1));
            let ranged = set.path.iter().rposition(|s| s.range.is_some());
            let count = (hi - lo + 1).max(1) as usize;
            let stride = (set.values.len() / count).max(1);
            for (k, chunk) in set.values.chunks(stride).enumerate().take(count) {
                let text: Vec<String> = set
                    .path
                    .iter()
                    .enumerate()
                    .map(|(at, seg)| match seg.range {
                        Some(r) if Some(at) == ranged => format!("{}[{}]", seg.name, r.0 + k as i64),
                        Some(r) => format!("{}[{}]", seg.name, r.0),
                        None => seg.name.clone(),
                    })
                    .collect();
                let Ok(p) = self.s.graph.plug(&format!("{path}.{}", text.join("."))) else {
                    self.warnings.push(format!("{}.{}: no such attribute", n.name, text.join(".")));
                    continue;
                };
                if self.s.graph.source(&p).is_some() {
                    continue;
                }
                let def = self.s.graph.attr(&p).clone();
                if let Some(v) = plug_value(&def, p.component, chunk, 1.0) {
                    if let Err(e) = self.s.graph.set_value(p, v) {
                        self.warnings.push(format!("{}.{}: {e}", n.name, text.join(".")));
                    }
                }
            }
        }
    }

    fn fill_deformer(&mut self, d: FileDeformer) {
        let doc = self.doc;
        let n = &doc.nodes[d.doc];
        let state = state_of(n);
        let id = d.id;
        let t = self.s.graph.node(id).unwrap().type_arc().clone();
        let xf = crate::dag::transform_of(&self.s.graph, d.shape);
        let _ = self.s.graph.connect(&deform::plug(xf, Transform::WORLD_MATRIX), &deform::plug(id, GEOM), false);
        self.scalar_sets(n, id);
        if !t.owner.is_empty() {
            self.nested_sets(n, id);
        }

        // What only Aztek's nodes hold, by the attributes' short names.
        if let Some(plugs) = state.get("plugs").and_then(Value::as_object) {
            for (short, j) in plugs {
                let Some(a) = t.attrs.iter().position(|a| a.short_name == *short) else { continue };
                if let Ok(v) = crate::plug::from_json(&t.attrs[a], j) {
                    let _ = self.s.graph.set_value(PlugRef::new(id, a as i32), v);
                }
            }
        }

        // The set: the leader's, which a follower joins.
        let leader = state.get("leader").and_then(Value::as_str).and_then(|l| self.s.graph.find(l).ok().flatten());
        let first_of_node = self.deformers.iter().find(|x| x.doc == d.doc).is_some_and(|x| x.id == id);
        let lead = leader.or(if first_of_node { None } else { self.ids[d.doc] });
        let _ = crate::deform_sets::make_set(&mut self.s.graph, id, d.shape);
        if let Some(l) = lead {
            let _ = deform::link_follower(&mut self.s.graph, l, id);
        }

        let capacity = self.s.graph.value(&deform::plug(id, IN)).ok().and_then(|v| v.as_mesh().map(|m| m.point_capacity())).unwrap_or(0);
        let members = self.read_members(&d, capacity);
        self.read_weights(&d, capacity, members.as_deref());

        if t.is_a("nonLinear") {
            self.read_nonlinear(&d);
        }
        if t.name == "cluster" && state.get("plugs").and_then(|p| p.get("dd")).is_none() {
            let geom = n.matrix_element(&["gm", "geomMatrix"], d.geometry).unwrap_or_else(|| rig::world(&mut self.s.graph, xf));
            let bind = n.matrix_of(&["bp", "bindPreMatrix"]).unwrap_or(DMat4::IDENTITY);
            let _ = deform::set_data(&mut self.s.graph, id, json!({"geom": deform::mat_json(&geom), "bindPre": deform::mat_json(&bind)}));
        }
        if t.name == "blendShape" {
            self.read_blend(&d, &state, lead);
        }
        if t.name == "wire" {
            self.read_wires(&d);
        }
    }

    /// `.dw[k]` drives and `.bw[k]` is where it rested.
    fn read_wires(&mut self, d: &FileDeformer) {
        let doc = self.doc;
        let of = |this: &Self, names: [&str; 2], k: i64| {
            doc.conns.iter().find(|c| c.dst == d.doc && names.contains(&plain_attr(&c.dst_attr).as_str()) && first_index(&c.dst_attr).unwrap_or(0) == k).and_then(|c| this.id(c.src))
        };
        let indices: Vec<i64> = doc.conns.iter().filter(|c| c.dst == d.doc && matches!(plain_attr(&c.dst_attr).as_str(), "dw" | "deformedWire")).map(|c| first_index(&c.dst_attr).unwrap_or(0)).collect();
        for k in indices {
            let Some(driver) = of(self, ["dw", "deformedWire"], k) else { continue };
            let base = of(self, ["bw", "baseWire"], k);
            if let Err(e) = crate::deform_wire::add_file_wire(&mut self.s.graph, d.id, k as i32, driver, base) {
                self.warnings.push(format!("{}: {e}", doc.nodes[d.doc].name));
            }
        }
    }

    /// The points the deformer acts on, when that isn't all of them.
    fn read_members(&mut self, d: &FileDeformer, capacity: u32) -> Option<Vec<u32>> {
        let doc = self.doc;
        let conn = doc.conns.iter().find(|c| c.dst == d.doc && matches!(plain_attr(&c.dst_attr).as_str(), "ip.ig" | "input.inputGeometry") && first_index(&c.dst_attr).unwrap_or(0) == d.geometry)?;
        let parts = &doc.nodes[conn.src];
        if parts.ty != "groupParts" {
            return None;
        }
        let set = parts.set_of(&["ic", "inputComponents"])?;
        let ids = component_ids(&set.values, capacity);
        if ids.len() >= capacity as usize {
            return None;
        }
        let _ = crate::deform_sets::replace_members(&mut self.s.graph, d.id, d.shape, &ids);
        Some(ids)
    }

    fn read_weights(&mut self, d: &FileDeformer, capacity: u32, members: Option<&[u32]>) {
        let lists = read_weights(&self.doc.nodes[d.doc]);
        let list = lists.get(d.geometry as usize).filter(|l| !l.is_empty());
        if list.is_none() && members.is_none() {
            return;
        }
        // The reference's default for a point it doesn't list is 1, and a point out of the set doesn't move.
        let mut w = vec![if members.is_some() { 0.0 } else { 1.0 }; capacity as usize];
        for &p in members.unwrap_or(&[]) {
            if let Some(x) = w.get_mut(p as usize) {
                *x = 1.0;
            }
        }
        for &(p, v) in list.map_or(&[][..], |l| l.as_slice()) {
            if let Some(x) = w.get_mut(p as usize) {
                *x = v;
            }
        }
        let _ = deform::set_floats(&mut self.s.graph, d.id, WEIGHTS, w);
    }

    /// A nonlinear's settings: those set on the node, as the reference writes its added attributes, else those on the
    /// handle shape; and its geometry matrix on the node.
    fn read_nonlinear(&mut self, d: &FileDeformer) {
        let doc = self.doc;
        let n = &doc.nodes[d.doc];
        let t = self.s.graph.node(d.id).unwrap().type_arc().clone();
        let shape = doc.input(d.doc, &["dd", "deformerData"]).map(|c| c.src);
        for (a, def) in t.attrs.iter().enumerate() {
            if !def.has(flags::USER) || def.ty != PlugType::Float {
                continue;
            }
            let names = [def.short_name.as_str(), def.name.as_str()];
            if let Some(v) = n.num(&names).or_else(|| shape.and_then(|s| doc.nodes[s].num(&names))) {
                let v = v * if def.unit == units::Unit::Angle { doc.ang } else { 1.0 };
                let _ = self.s.graph.set_value(PlugRef::new(d.id, a as i32), PlugValue::Float(v));
            }
        }
        if deform::json_of(self.s.graph.node(d.id).unwrap().data(), DATA).get("geom").is_none() {
            let xf = crate::dag::transform_of(&self.s.graph, d.shape);
            let geom = n.matrix_element(&["gm", "geomMatrix"], d.geometry).unwrap_or_else(|| rig::world(&mut self.s.graph, xf));
            let _ = deform::set_data(&mut self.s.graph, d.id, json!({"geom": deform::mat_json(&geom)}));
        }
    }

    fn read_blend(&mut self, d: &FileDeformer, state: &Value, lead: Option<NodeId>) {
        let doc = self.doc;
        let n = &doc.nodes[d.doc];
        let Some(input) = deform::input_mesh(&mut self.s.graph, d.id) else { return };
        let capacity = input.point_capacity();
        let geo = |seg: &Seg| seg.range.map_or(0, |r| r.0) == d.geometry;
        let mut targets: BTreeMap<i32, BlendTarget> = BTreeMap::new();
        let mut items: BTreeMap<(i32, i32), (Vec<DVec3>, Vec<u32>)> = BTreeMap::new();
        let mut base_weights: Option<Vec<f64>> = None;
        const IT: &[&str] = &["it", "inputTarget"];
        const ITG: &[&str] = &["itg", "inputTargetGroup"];
        const ITI: &[&str] = &["iti", "inputTargetItem"];
        let fill = |w: &mut Option<Vec<f64>>, seg: &Seg, values: &[Tok]| {
            let list = w.get_or_insert_with(|| vec![1.0; capacity as usize]);
            let start = seg.range.map_or(0, |r| r.0 as usize);
            for (k, v) in values.iter().enumerate() {
                if let (Some(x), Some(v)) = (list.get_mut(start + k), v.num()) {
                    *x = v;
                }
            }
        };
        for s in &n.sets {
            if s.values.is_empty() {
                continue;
            }
            if let Some(p) = matches_path(s, &[IT, &["bw", "baseWeights"]]) {
                if geo(&p[0]) {
                    fill(&mut base_weights, &p[1], &s.values);
                }
            } else if let Some(p) = matches_path(s, &[IT, ITG, &["tw", "targetWeights"]]) {
                if geo(&p[0]) {
                    let i = p[1].range.map_or(0, |r| r.0) as i32;
                    fill(&mut targets.entry(i).or_default().weights, &p[2], &s.values);
                }
            } else if let Some(p) = matches_path(s, &[IT, ITG, ITI, &["ipt", "inputPointsTarget"]]) {
                if geo(&p[0]) {
                    let key = (p[1].range.map_or(0, |r| r.0) as i32, p[2].range.map_or(6000, |r| r.0) as i32);
                    // A count, then x y z w each.
                    let pts = s.values.get(1..).unwrap_or(&[]).chunks_exact(4).map(tok_vec3).collect();
                    items.entry(key).or_default().0 = pts;
                }
            } else if let Some(p) = matches_path(s, &[IT, ITG, ITI, &["ict", "inputComponentsTarget"]]) {
                if geo(&p[0]) {
                    let key = (p[1].range.map_or(0, |r| r.0) as i32, p[2].range.map_or(6000, |r| r.0) as i32);
                    items.entry(key).or_default().1 = component_ids(&s.values, capacity);
                }
            }
        }
        // Live targets have an item with no points yet.
        for c in doc.conns.iter().filter(|c| c.dst == d.doc) {
            if let Some((g, i, iti)) = live_target_path(&c.dst_attr) {
                if g == d.geometry {
                    items.entry((i, iti)).or_default();
                }
            }
        }
        let relative: Vec<(i64, i64)> = state.get("relative").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| Some((x.get(0)?.as_i64()?, x.get(1)?.as_i64()?))).collect()).unwrap_or_default();
        for (&(i, iti), (pts, ids)) in &items {
            let t = targets.entry(i).or_default();
            let deltas = ids.iter().copied().zip(pts.iter().copied()).collect();
            t.items.push(BlendItem { iti, relative: relative.contains(&(i as i64, iti as i64)), deltas });
        }
        // Names come from the aliases: {"smile", "weight[0]", ...}.
        let words: Vec<&str> = n.set_of(&["aal", "attributeAliasList"]).map(|s| s.values.iter().filter_map(|t| if let Tok::Str(x) = t { Some(x.as_str()) } else { None }).collect()).unwrap_or_default();
        let is_leader = lead.is_none();
        for pair in words.chunks_exact(2) {
            let Some(i) = pair[1].strip_prefix("weight[").or_else(|| pair[1].strip_prefix("w[")).and_then(|x| x.strip_suffix(']')).and_then(|x| x.parse::<i32>().ok()) else { continue };
            targets.entry(i).or_default().name = pair[0].to_string();
            if is_leader {
                self.s.graph.set_alias(d.id, pair[0], Some(format!("weight[{i}]")));
            }
        }
        let mut list: Vec<BlendTarget> = Vec::with_capacity(targets.len());
        for (i, mut t) in targets {
            t.index = i;
            if t.name.is_empty() {
                t.name = format!("weight[{i}]");
            }
            let w = PlugRef::element(d.id, deform_blend::WEIGHT as i32, i);
            let held = self.s.graph.elements(&deform::plug(d.id, deform_blend::WEIGHT)).iter().any(|e| e.element == i);
            if !held && lead.is_none() {
                let _ = self.s.graph.set_value(w, PlugValue::Float(0.0));
            }
            if let Some(l) = lead {
                let _ = self.s.graph.connect(&PlugRef::element(l, deform_blend::WEIGHT as i32, i), &w, true);
            }
            list.push(t);
        }
        if let Some(w) = base_weights {
            let _ = deform::set_floats(&mut self.s.graph, d.id, WEIGHTS, w);
        }
        let _ = deform_blend::set_blend(&mut self.s.graph, d.id, BlendData { rest: input.positions().to_vec(), targets: list });
    }

    /// Connections at a deformer, or at its handle shape, once all the file's nodes exist. Attributes the reference has and
    /// these nodes don't are passed over without comment.
    pub(super) fn deformer_connections(&mut self) {
        let doc = self.doc;
        // A nonlinear's settings on the handle shape are the node's here.
        let mut by_shape: HashMap<usize, usize> = HashMap::new();
        for c in &doc.conns {
            if doc.nodes[c.dst].ty == "nonLinear" && handle_kind(&doc.nodes[c.src].ty).is_some() {
                by_shape.insert(c.src, c.dst);
            }
        }
        let nodes = |this: &Self, i: usize| -> Vec<NodeId> { this.deformers.iter().filter(|d| d.doc == i).map(|d| d.id).collect() };
        for c in &doc.conns {
            let dst_doc = by_shape.get(&c.dst).copied().unwrap_or(c.dst);
            let list = nodes(self, dst_doc);
            let src_is_deformer = is_file_deformer(&doc.nodes[c.src].ty) || self.deformers.iter().any(|d| d.doc == c.src);
            if list.is_empty() && !src_is_deformer {
                continue;
            }
            let (sn, sa) = self.real_source(c);
            let plain_dst = plain_attr(&c.dst_attr);
            // A solver's or field's link to a cloth is a message connection that means something.
            let dyn_link = matches!(plain_dst.as_str(), "slv" | "solver" | "ifc" | "inputForce");
            // A plug-in's deformer may read a node's message (cvWrap's bindMesh): only it knows what for.
            let plugin_msg = crate::scripted::is_plugin_deformer(&doc.nodes[dst_doc].ty) && matches!(plain_attr(&sa).as_str(), "msg" | "message");
            if matches!(plain_dst.as_str(), "bw" | "baseWire" | "dw" | "deformedWire" | "ip.ig" | "ip.gi" | "input.inputGeometry" | "input.groupId" | "i" | "inMesh" | "dd" | "deformerData" | "gm" | "geomMatrix") || (!dyn_link && !plugin_msg && matches!(plain_attr(&sa).as_str(), "og" | "outputGeometry" | "msg" | "message") && !crate::rig_extra::is_flexor_type(&doc.nodes[sn].ty)) {
                continue;
            }
            let Some(src) = self.id(sn) else { continue };
            let from = format!("{}.{sa}", self.s.graph.path(src));
            if list.is_empty() {
                // From a deformer to anything else.
                let Some(dst) = self.id(c.dst) else { continue };
                let to = format!("{}.{}", self.s.graph.path(dst), c.dst_attr);
                self.connect_quietly(&from, &to);
                continue;
            }
            for id in list {
                let path = self.s.graph.path(id);
                if let Some((g, i, iti)) = live_target_path(&c.dst_attr) {
                    let key = i * 100_000 + iti;
                    let is_this = self.deformers.iter().any(|d| d.id == id && d.geometry == g);
                    if is_this {
                        let from = self.target_source(src, from.clone());
                        let to = format!("{path}.inputGeomTarget[{key}]");
                        if self.s.graph.plug(&from).is_err() {
                            self.warnings.push(format!("{to}: nothing to read at {from}"));
                        }
                        self.connect_quietly(&from, &to);
                        let xf = crate::dag::transform_of(&self.s.graph, src);
                        let m = self.s.graph.path(xf);
                        self.connect_quietly(&format!("{m}.worldMatrix"), &format!("{path}.inputTargetMatrix[{key}]"));
                    }
                    continue;
                }
                let attr = if matches!(plain_dst.as_str(), "ma" | "matrix") { "handleMatrix" } else { c.dst_attr.as_str() };
                // A wrap's driver may be a curve.
                let from = if matches!(plain_dst.as_str(), "dp" | "driverPoints") { self.target_source(src, from.clone()) } else { from.clone() };
                self.connect_quietly(&from, &format!("{path}.{attr}"));
            }
        }
        // A nonlinear's settings into its handle shape, as the reference has them.
        let nonlinear: Vec<NodeId> = self.deformers.iter().map(|d| d.id).filter(|&id| self.s.graph.node(id).is_some_and(|n| n.ty().is_a("nonLinear"))).collect();
        for id in nonlinear {
            if deform::leader_of(&self.s.graph, id) == id {
                let _ = deform::link_handle_shape(&mut self.s.graph, id);
            }
        }
    }

    fn connect_quietly(&mut self, a: &str, b: &str) {
        let g = &mut self.s.graph;
        if let (Ok(x), Ok(y)) = (g.plug(a), g.plug(b)) {
            // A nonlinear's settings into its handle's shape, which here are the one node.
            if x == y {
                return;
            }
            if let Err(e) = g.connect(&x, &y, true) {
                self.warnings.push(format!("{a} -> {b}: {e}"));
            }
        }
    }

    /// The lattice's points from `.cc`: s t u, a count, then x y z each.
    pub(super) fn read_lattices(&mut self) {
        let doc = self.doc;
        for i in 0..doc.nodes.len() {
            if doc.nodes[i].ty != "lattice" {
                continue;
            }
            let Some(id) = self.id(i) else { continue };
            let n = &doc.nodes[i];
            let Some(set) = n.set_of(&["cc", "cached"]) else { continue };
            let nums: Vec<f64> = set.values.iter().filter_map(Tok::num).collect();
            if nums.len() < 3 {
                continue;
            }
            let dims = [nums[0] as usize, nums[1] as usize, nums[2] as usize];
            let want = dims[0] * dims[1] * dims[2];
            let rest = &nums[3..];
            let coords = rest.len() % 3;
            let pts: Vec<DVec3> = rest[coords..].chunks_exact(3).map(|c| DVec3::new(c[0], c[1], c[2])).collect();
            if pts.len() == want {
                let a = self.s.graph.node(id).unwrap().ty().attr_index("controlPoints").unwrap();
                let _ = self.s.graph.set_value(PlugRef::new(id, a as i32), PlugValue::Data(Some(Arc::new(LatticeGrid { dims, pts }))));
            } else {
                self.warnings.push(format!("{}: lattice points don't match its divisions", n.name));
            }
        }
    }
}

/// `it[g].itg[i].iti[k].igt` as its three indices.
fn live_target_path(attr: &str) -> Option<(i64, i32, i32)> {
    let p = parse_path(attr);
    let [a, b, c, e] = p.as_slice() else { return None };
    let ok = matches!(a.name.as_str(), "it" | "inputTarget") && matches!(b.name.as_str(), "itg" | "inputTargetGroup") && matches!(c.name.as_str(), "iti" | "inputTargetItem") && matches!(e.name.as_str(), "igt" | "inputGeomTarget");
    ok.then(|| (a.range.map_or(0, |r| r.0), b.range.map_or(0, |r| r.0) as i32, c.range.map_or(6000, |r| r.0) as i32))
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing

impl Writer<'_> {
    /// The deformers next to a mesh, source first. History above them is baked into the Orig shape.
    pub(super) fn deformer_chain(&self, shape: NodeId) -> Vec<NodeId> {
        let g = &self.s.graph;
        let mut out = Vec::new();
        let mut at = PlugRef::new(shape, MeshNode::IN_MESH as i32);
        while let Some(src) = g.source(&at) {
            let Some(n) = g.node(src.node) else { break };
            let tweak = n.ty().name == crate::history::TWEAK_TYPE;
            if !(tweak || rig::is_rig_chain_node(n.ty().name.as_str())) || !self.keep.contains(&src.node) {
                break;
            }
            out.insert(0, src.node);
            let Some(ip) = n.ty().attr_index("inputPolymesh") else { break };
            at = PlugRef::new(src.node, ip as i32);
        }
        // Tweaks with no deformer above them are part of the geometry the first deformer reads.
        while out.first().is_some_and(|&d| g.node(d).is_some_and(|n| n.ty().name == crate::history::TWEAK_TYPE)) {
            out.remove(0);
        }
        out
    }

    pub(super) fn write_chain(&mut self, chain: &[NodeId], shape_path: &str, orig: &str, stored: &Stored) {
        let last = self.write_chain_from(chain, shape_path, format!("{orig}.w"), stored, false);
        self.conn(&last, &format!("{shape_path}.i"));
    }

    /// The deformers from `first`. Returns the plug that comes out of them. On a curve or surface a tweak is the reference's tweak node.
    pub(super) fn write_chain_from(&mut self, chain: &[NodeId], shape_path: &str, first: String, stored: &Stored, nurbs: bool) -> String {
        let mut prev = first;
        for (k, &d) in chain.iter().enumerate() {
            let dp = self.s.graph.path(d);
            let is_skin = self.s.graph.node(d).is_some_and(|n| n.ty().name == skin::SKIN_TYPE);
            let is_tweak = self.s.graph.node(d).is_some_and(|n| n.ty().name == crate::history::TWEAK_TYPE);
            if is_tweak && nurbs {
                self.conn(&prev, &format!("{dp}.ip[0].ig"));
                let mut text = format!("createNode tweak -n {};\n", quoted(&dp));
                let saved = crate::history::tweaks_json(&self.s.graph, d);
                for e in saved.get("points").and_then(Value::as_array).into_iter().flatten() {
                    let c = |k: usize| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
                    let _ = writeln!(text, "\tsetAttr \".pl[0].cp[{}]\" -type \"double3\" {} ;", c(0) as i64, vec_text(DVec3::new(c(1), c(2), c(3))));
                }
                self.pending_nodes_push(text);
                prev = format!("{dp}.og[0]");
                continue;
            }
            if is_tweak {
                self.conn(&prev, &format!("{dp}.ip"));
                let text = self.chain_tweak_text(d, stored);
                self.pending_nodes_push(text);
                prev = format!("{dp}.out");
                continue;
            }
            let mut text = String::new();
            if is_skin {
                self.conn(&prev, &format!("{dp}.ip[0].ig"));
                text = self.skin_text(d, stored);
            } else {
                let (parts, id) = (self.fresh(&format!("{dp}GroupParts")), self.fresh(&format!("{dp}GroupId")));
                let _ = writeln!(text, "createNode groupId -n {};\n\tsetAttr \".ihi\" 0;", quoted(&id));
                let _ = writeln!(text, "createNode groupParts -n {};\n\tsetAttr \".ihi\" 0;\n\tsetAttr \".ic\" -type \"componentList\" {};", quoted(&parts), self.member_text(d, stored));
                self.conn(&prev, &format!("{parts}.ig"));
                self.conn(&format!("{id}.id"), &format!("{parts}.gi"));
                self.conn(&format!("{parts}.og"), &format!("{dp}.ip[0].ig"));
                self.conn(&format!("{id}.id"), &format!("{dp}.ip[0].gi"));
                self.conn(&format!("{id}.id"), &format!("{shape_path}.iog.og[{k}].gid"));
                if let Some(set) = crate::deform_sets::set_of(&self.s.graph, d) {
                    let sp = self.s.graph.path(set);
                    self.conn(&format!("{sp}.mwc"), &format!("{shape_path}.iog.og[{k}].gco"));
                    if deform::leader_of(&self.s.graph, d) == d {
                        let _ = writeln!(text, "createNode objectSet -n {};\n\tsetAttr \".ihi\" 0;\n\tsetAttr \".vo\" yes;", quoted(&sp));
                        self.conn(&format!("{dp}.msg"), &format!("{sp}.ub[0]"));
                    }
                }
                text.push_str(&self.deformer_text(d, stored));
            }
            self.chain_written.insert(d);
            self.pending_nodes_push(text);
            prev = format!("{dp}.og[0]");
        }
        prev
    }

    fn chain_tweak_text(&self, d: NodeId, stored: &Stored) -> String {
        let mut o = format!("createNode polyTweak -n {};\n", quoted(&self.s.graph.path(d)));
        let saved = crate::history::tweaks_json(&self.s.graph, d);
        for e in saved.get("points").and_then(Value::as_array).into_iter().flatten() {
            let c = |k: usize| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
            if let Some(&p) = stored.point_of.get(c(0) as usize).filter(|p| **p >= 0) {
                let _ = writeln!(o, "\tsetAttr \".tk[{p}]\" -type \"float3\" {};", vec_text(DVec3::new(c(1), c(2), c(3))));
            }
        }
        o
    }

    fn member_text(&self, d: NodeId, stored: &Stored) -> String {
        let mask = crate::deform_sets::member_mask(&self.s.graph, d, stored.point_of.len());
        match mask {
            None => "1 \"vtx[*]\"".to_string(),
            Some(m) if m.iter().enumerate().all(|(p, &on)| on || stored.point_of[p] < 0) => "1 \"vtx[*]\"".to_string(),
            Some(m) => {
                let ids: Vec<i64> = m.iter().enumerate().filter(|x| *x.1 && stored.point_of[x.0] >= 0).map(|x| stored.point_of[x.0] as i64).collect();
                component_text(&ids)
            }
        }
    }

    fn deformer_text(&mut self, d: NodeId, stored: &Stored) -> String {
        let mut o = String::new();
        let name = self.s.graph.path(d);
        let t = self.s.graph.node(d).unwrap().type_arc().clone();
        let nonlinear = t.is_a("nonLinear");
        let _ = writeln!(o, "createNode {} -n {};", if nonlinear { "nonLinear" } else { t.name.as_str() }, quoted(&name));

        let identity = stored.point_of.iter().enumerate().all(|(p, &d)| d as i64 == p as i64);
        let mut state = serde_json::Map::new();
        let mut plugs = serde_json::Map::new();
        for (a, def) in t.attrs.iter().enumerate() {
            let is_data = def.ty == PlugType::Data && !def.is_array() && def.has(flags::INPUT);
            if !is_data || a == WEIGHTS || def.name == "blendData" || def.name == "sculptState" {
                continue;
            }
            let p = PlugRef::new(d, a as i32);
            if self.s.graph.source(&p).is_some() {
                continue;
            }
            if let PlugValue::Data(Some(x)) = self.s.graph.node(d).unwrap().value(a) {
                plugs.insert(def.short_name.clone(), x.to_json());
            }
        }
        if !plugs.is_empty() {
            if !identity {
                self.warnings.push(format!("{name}: the mesh has deleted points, so what this deformer holds per point may not line up when read back"));
            }
            state.insert("plugs".into(), Value::Object(plugs));
        }
        if let Some(kind) = deform::nonlinear_kind(&self.s.graph, d).filter(|_| nonlinear) {
            state.insert("type".into(), json!(kind));
        }
        let leader = deform::leader_of(&self.s.graph, d);
        if leader != d {
            state.insert("leader".into(), json!(self.s.graph.path(leader)));
        }

        let blend = (t.name == "blendShape").then(|| deform_blend::blend_of(&self.s.graph, d));
        if let Some(b) = &blend {
            let rel: Vec<Value> = b.targets.iter().flat_map(|t| t.items.iter().filter(|i| i.relative).map(|i| json!([t.index, i.iti]))).collect();
            if !rel.is_empty() {
                state.insert("relative".into(), Value::Array(rel));
            }
        }
        if !state.is_empty() {
            let _ = writeln!(o, "\taddAttr -ci true -sn \"azs\" -ln \"aztekState\" -dt \"string\";");
            let _ = writeln!(o, "\tsetAttr \".azs\" -type \"string\" {};", quoted(&Value::Object(state).to_string()));
        }

        // A nonlinear's settings, added to it as the reference adds them.
        if nonlinear {
            for a in t.attrs.iter().filter(|a| a.has(flags::USER)) {
                o.push_str(&attr_add_lines(a));
            }
        }
        o.push_str(&self.attr_lines(d, |_, a| !a.has(flags::HIDDEN)));

        // Weights per written point.
        let weights = self.s.graph.node(d).and_then(|n| deform::floats_of(n.data(), WEIGHTS)).map(|f: &Floats| f.0.clone());
        let by_point = |w: &[f64]| -> Vec<f64> {
            let mut out = vec![1.0; stored.points.len()];
            for (p, &v) in w.iter().enumerate() {
                if let Some(&x) = stored.point_of.get(p) {
                    if x >= 0 {
                        out[x as usize] = v;
                    }
                }
            }
            out
        };
        let values = |w: &[f64]| w.iter().map(f64::to_string).collect::<Vec<_>>().join(" ");
        if let Some(w) = &weights {
            let w = by_point(w);
            if !w.is_empty() {
                if blend.is_some() {
                    let _ = writeln!(o, "\tsetAttr -s {} \".it[0].bw\";\n\tsetAttr \".it[0].bw[0:{}]\" {};", w.len(), w.len() - 1, values(&w));
                } else {
                    let _ = writeln!(o, "\tsetAttr -s {} \".wl[0].w\";\n\tsetAttr \".wl[0].w[0:{}]\" {};", w.len(), w.len() - 1, values(&w));
                }
            }
        }

        let gm = self.s.graph.value(&deform::plug(d, GEOM)).ok().and_then(|v| if let PlugValue::Matrix(m) = v { Some(m) } else { None }).unwrap_or(DMat4::IDENTITY);
        let _ = writeln!(o, "\tsetAttr \".gm[0]\" -type \"matrix\" {};", matrix_text(&gm));
        if t.name == "cluster" {
            let data = deform::json_of(self.s.graph.node(d).unwrap().data(), DATA);
            let bind = deform::mat_from(data.get("bindPre").unwrap_or(&Value::Null));
            let _ = writeln!(o, "\tsetAttr \".bp\" -type \"matrix\" {};", matrix_text(&bind));
        }

        if let Some(b) = &blend {
            let mut alias = Vec::new();
            for t in &b.targets {
                alias.push(format!("{},\"weight[{}]\"", quoted(&t.name), t.index));
                if let Some(w) = &t.weights {
                    let w = by_point(w);
                    if !w.is_empty() {
                        let _ = writeln!(o, "\tsetAttr -s {} \".it[0].itg[{}].tw\";\n\tsetAttr \".it[0].itg[{}].tw[0:{}]\" {};", w.len(), t.index, t.index, w.len() - 1, values(&w));
                    }
                }
                for item in &t.items {
                    let moved: Vec<(i64, DVec3)> = item.deltas.iter().filter_map(|&(p, v)| stored.point_of.get(p as usize).filter(|x| **x >= 0).map(|x| (*x as i64, v))).collect();
                    let pts: Vec<String> = moved.iter().map(|x| format!("{} {} {} 1", x.1.x, x.1.y, x.1.z)).collect();
                    let ids: Vec<i64> = moved.iter().map(|x| x.0).collect();
                    let _ = writeln!(o, "\tsetAttr \".it[0].itg[{}].iti[{}].ipt\" -type \"pointArray\" {} {};", t.index, item.iti, pts.len(), pts.join(" "));
                    let _ = writeln!(o, "\tsetAttr \".it[0].itg[{}].iti[{}].ict\" -type \"componentList\" {};", t.index, item.iti, component_text(&ids));
                }
            }
            if !alias.is_empty() {
                let _ = writeln!(o, "\tsetAttr \".aal\" -type \"attributeAlias\" {{{}}} ;", alias.join(","));
            }
        }
        o
    }

    /// What a handle's shape connects to: a nonlinear's deformerData into its deformer, a cluster's transforms into the
    /// cluster, as the reference has them.
    pub(super) fn write_handle_shape(&mut self, shape: NodeId) {
        let g = &self.s.graph;
        let h = g.parent(shape);
        if h == NO_NODE {
            return;
        }
        let ty = g.node(shape).map(|n| n.ty().name.clone()).unwrap_or_default();
        let dests = g.destinations(&PlugRef::new(h, Transform::WORLD_MATRIX as i32));
        let sp = g.path(shape);
        let lines: Vec<(String, String)> = dests
            .iter()
            .filter(|p| p.attr as usize == HANDLE && self.keep.contains(&p.node) && deform::leader_of(g, p.node) == p.node)
            .filter_map(|p| {
                let dt = g.node(p.node)?.ty().name.clone();
                let dp = g.path(p.node);
                match (ty.as_str(), dt.as_str()) {
                    (_, "nonLinear") if ty.starts_with("deform") => Some((format!("{sp}.dd"), format!("{dp}.dd"))),
                    ("clusterHandle", "cluster") => Some((format!("{sp}.x"), format!("{dp}.x"))),
                    _ => None,
                }
            })
            .collect();
        for (a, b) in lines {
            self.conn(&a, &b);
        }
    }

    pub(super) fn write_lattice_points(&mut self, shape: NodeId) {
        let Some(grid) = deform_lattice::current_grid(&mut self.s.graph, shape) else { return };
        let pts: Vec<String> = grid.pts.iter().map(|p| vec_text(*p)).collect();
        let _ = writeln!(self.out, "\tsetAttr \".cc\" -type \"lattice\" {} {} {} {} {};", grid.dims[0], grid.dims[1], grid.dims[2], pts.len(), pts.join(" "));
    }

    /// A connection at a deformer, under the names the reference has for it. The mesh chain is written with the chain.
    pub(super) fn deformer_connection(&mut self, src: PlugRef, dst: PlugRef) {
        let g = &self.s.graph;
        let (dt, st) = (g.node(dst.node).unwrap().type_arc().clone(), g.node(src.node).unwrap().type_arc().clone());
        let is_def = |t: &NodeType| deform::is_deformer(&t.name) || crate::scripted::is_plugin_deformer(&t.name);
        if is_def(&dt) && matches!(dst.attr as usize, IN | GEOM | deform::DEFORMER_SET) {
            return;
        }
        // A curve as a blend target: it goes through nurbsToPoints here and straight in as the reference has it.
        if st.name == crate::nurbs_nodes::TO_POINTS {
            if dt.name == "blendShape" && dt.attrs[dst.attr as usize].name == "inputGeomTarget" {
                if let Some(shape) = g.source(&PlugRef::new(src.node, 0)) {
                    let (a, dp) = (g.path(shape.node), g.path(dst.node));
                    let (i, iti) = (dst.element / 100_000, dst.element % 100_000);
                    self.conn(&format!("{a}.ws"), &format!("{dp}.it[0].itg[{i}].iti[{iti}].igt"));
                }
            }
            return;
        }
        // A wire's driver and rest curve, when it knows them, as the reference's deformedWire and baseWire.
        if dt.name == "wire" && matches!(dt.attrs[dst.attr as usize].name.as_str(), "wireMatrix" | "baseWireMatrix") {
            if let Some(shape) = crate::curves::as_curve(g, src.node) {
                let short = if dt.attrs[dst.attr as usize].name == "wireMatrix" { "dw" } else { "bw" };
                let (a, dp) = (g.path(shape), g.path(dst.node));
                self.conn(&format!("{a}.ws"), &format!("{dp}.{short}[{}]", dst.element));
                return;
            }
        }
        if is_def(&st) && src.attr as usize == OUT {
            return;
        }
        if dt.name == "blendShape" && dt.attrs[dst.attr as usize].name == "inputTargetMatrix" {
            return;
        }
        if st.is_a("mesh") && g.node(src.node).is_some_and(|n| n.bool(DagNode::INTERMEDIATE_OBJECT)) {
            return;
        }
        let written = |id: NodeId, t: &NodeType| self.dag_written(id) || is_def(t) || anim::is_curve(g, id) || DRIVEN_CURVES.contains(&t.name.as_str()) || is_plain_node(&t.name) || t.name == "unitConversion";
        if !written(src.node, &st) || !written(dst.node, &dt) {
            return;
        }
        let name = |p: &PlugRef| {
            let text = g.plug_name(p);
            let one_of = matches!(g.attr(p).name.as_str(), "worldMatrix" | "worldInverseMatrix" | "parentMatrix" | "parentInverseMatrix");
            if one_of && p.element < 0 {
                format!("{text}[0]")
            } else {
                text
            }
        };
        let a = name(&src);
        let dp = g.path(dst.node);
        let attr = &dt.attrs[dst.attr as usize];
        let b = if dt.name == "blendShape" && attr.name == "inputGeomTarget" {
            let (i, iti) = (dst.element / 100_000, dst.element % 100_000);
            format!("{dp}.it[0].itg[{i}].iti[{iti}].igt")
        } else if is_def(&dt) && dst.attr as usize == HANDLE {
            format!("{dp}.ma")
        } else {
            name(&dst)
        };
        self.conn(&a, &b);
    }
}
