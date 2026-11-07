//! Materials in .ma, as the reference lays them out. A shape in a shading group: shape.iog to SG.dsm -na. Some of its faces:
//! shape.iog.og[k] to SG.dsm -na, a groupId tying them together (.id to .iog.og[k].gid, .msg to SG.gn), SG.mwc to
//! .iog.og[k].gco, and the faces in .iog[0].og[k].gcl or a groupParts' .ic. Materials, textures and placements are plain
//! nodes written and read attribute by attribute; here are what sets them apart: default nodes, shading groups, and the
//! list nodes the reference's Hypershade reads, which Aztek has no node for and writes as connections alone.

use super::*;
use crate::components::ComponentMode;
use crate::object_sets::{self as os, Member};
use crate::shading::{self, SG_TYPE};

/// A plug as a .ma connection names it: default nodes rooted, short attribute names.
pub(super) fn short_plug(g: &Graph, p: &PlugRef) -> String {
    let n = g.node(p.node).unwrap();
    let a = g.attr(p);
    let attr = if p.component >= 0 {
        a.child_short[p.component as usize].clone()
    } else if p.element >= 0 {
        format!("{}[{}]", a.short_name, p.element)
    } else {
        a.short_name.clone()
    };
    format!("{}.{attr}", node_ref(g, n.id()))
}

/// Whether a node's setAttr lines use short names, as the reference's do for these.
pub(super) fn writes_short(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a(SG_TYPE) || n.ty().is_a("light") || shading::classification(&n.ty().name).is_some() || n.ty().name == "animLayer" || n.ty().name.starts_with("animBlendNode"))
}

fn node_ref(g: &Graph, id: NodeId) -> String {
    let n = g.node(id).unwrap();
    if shading::is_default_node(n.name()) {
        format!(":{}", n.name())
    } else {
        g.path(id)
    }
}

/// `2 "f[0:3]" "f[7]"`.
fn face_text(ids: &[u32]) -> String {
    let mut parts = Vec::new();
    let mut k = 0;
    while k < ids.len() {
        let mut end = k;
        while end + 1 < ids.len() && ids[end + 1] == ids[end] + 1 {
            end += 1;
        }
        parts.push(if end == k { format!("\"f[{}]\"", ids[k]) } else { format!("\"f[{}:{}]\"", ids[k], ids[end]) });
        k = end + 1;
    }
    format!("{} {}", parts.len(), parts.join(" "))
}

/// The faces a componentList names. `count` answers f[*].
fn face_ids(values: &[Tok], count: u32) -> Vec<u32> {
    let mut out = Vec::new();
    for t in values {
        let Some(inner) = t.text().and_then(|x| x.strip_prefix("f[")).and_then(|x| x.strip_suffix(']')) else { continue };
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
    out.sort_unstable();
    out.dedup();
    out
}

fn is_group_plug(attr: &str) -> bool {
    matches!(attr, "iog.og" | "instObjGroups.objectGroups" | "iog.objectGroups" | "instObjGroups.og")
}

// ---------------------------------------------------------------------------------------------------------------------
// Reading

impl Build<'_> {
    /// The defaults the file names, and its shading groups. Before the plain nodes, which would take a default's name.
    pub(super) fn create_shading_groups(&mut self, import: bool) {
        shading::make_defaults(&mut self.s.graph);
        let doc = self.doc;
        for i in 0..doc.nodes.len() {
            let n = &doc.nodes[i];
            if shading::is_default_node(&n.name) {
                let Ok(Some(id)) = self.s.graph.find(&n.name) else { continue };
                self.ids[i] = Some(id);
                self.used[i] = true;
                // What another scene changed on its own lambert1 stays there when it is imported.
                if !import {
                    self.create_plain(i);
                }
            } else if n.ty == SG_TYPE {
                self.create_plain(i);
            }
        }
    }

    /// Who each shading group holds. After the meshes are built, since `f[*]` asks how many faces there are.
    pub(super) fn read_shading_members(&mut self) {
        let doc = self.doc;
        let mut held: BTreeMap<NodeId, Vec<Member>> = BTreeMap::new();
        for c in &doc.conns {
            if !matches!(plain_attr(&c.dst_attr).as_str(), "dsm" | "dagSetMembers") {
                continue;
            }
            let (Some(sg), Some(shape)) = (self.id(c.dst), self.id(c.src)) else { continue };
            let g = &mut self.s.graph;
            // An intermediate shape's place in a group is never drawn, and isn't written back.
            let drawn = g.node(shape).is_some_and(|n| (n.ty().is_a("mesh") || n.ty().is_a("nurbsSurface")) && !n.bool(DagNode::INTERMEDIATE_OBJECT));
            if !shading::is_shading_group(g, sg) || !drawn {
                continue;
            }
            let member = if is_group_plug(&plain_attr(&c.src_attr)) {
                let k = parse_path(&c.src_attr).last().and_then(|s| s.range).map_or(0, |r| r.0);
                let count = crate::components::mesh_of(g, shape).map_or(0, |m| m.face_capacity());
                match file_group_faces(doc, c.src, k, count) {
                    Some(ids) if !ids.is_empty() => Member { node: shape, kind: ComponentMode::Face, ids },
                    _ => Member::object(shape),
                }
            } else {
                Member::object(shape)
            };
            let list = held.entry(sg).or_default();
            match list.iter_mut().find(|m| m.node == member.node && m.kind == member.kind) {
                Some(m) => {
                    m.ids.extend(member.ids);
                    m.ids.sort_unstable();
                    m.ids.dedup();
                }
                None => list.push(member),
            }
        }
        for (sg, list) in held {
            let g = &mut self.s.graph;
            // A file adds to a group: lambert1's holds what the scene and every other file already put there.
            let mut all = os::members(g, sg);
            for m in list {
                if !all.iter().any(|x| x.node == m.node && x.kind == m.kind) {
                    all.push(m);
                }
            }
            let done = os::set_members_raw(g, sg, &all).and_then(|_| shading::wire(g, sg));
            if let Err(e) = done {
                self.warnings.push(format!("{}: {e}", self.s.graph.path(sg)));
            }
        }
    }
}

/// An object group's faces: on the shape, or on the groupParts its groupId feeds.
fn file_group_faces(doc: &Doc, shape: usize, k: i64, count: u32) -> Option<Vec<u32>> {
    let n = &doc.nodes[shape];
    for s in n.sets.iter().rev() {
        let [a, b, c] = s.path.as_slice() else { continue };
        if matches!(a.name.as_str(), "iog" | "instObjGroups") && matches!(b.name.as_str(), "og" | "objectGroups") && matches!(c.name.as_str(), "gcl" | "groupingComponentList" | "objectGrpCompList") && b.range.map(|r| r.0) == Some(k) && !s.values.is_empty() {
            return Some(face_ids(&s.values, count));
        }
    }
    let id = doc.conns.iter().find(|c| {
        c.dst == shape && matches!(plain_attr(&c.dst_attr).as_str(), "iog.og.gid" | "instObjGroups.objectGroups.objectGroupId") && parse_path(&c.dst_attr).get(1).and_then(|s| s.range).map(|r| r.0) == Some(k)
    })?;
    let parts = doc.conns.iter().find(|c| c.src == id.src && doc.nodes[c.dst].ty == "groupParts" && matches!(plain_attr(&c.dst_attr).as_str(), "gi" | "groupId"))?;
    let set = doc.nodes[parts.dst].set_of(&["ic", "inputComponents"])?;
    Some(face_ids(&set.values, count))
}

/// What the reference's file names without creating, so a reader that hasn't seen `select -ne` still connects to it.
pub(super) fn is_named_default(path: &str) -> bool {
    path.strip_prefix(':').is_some_and(shading::is_default_node)
}

// ---------------------------------------------------------------------------------------------------------------------
// Writing

impl Writer<'_> {
    /// A shape's place in its shading groups: its connections, and for faces their group ids. `first_group` is the first
    /// object group index its deformers leave free. The shape's createNode is the current node in the file.
    pub(super) fn write_membership(&mut self, shape: NodeId, path: &str, first_group: usize) {
        let g = &self.s.graph;
        let mut held: Vec<(NodeId, Option<Vec<u32>>)> = shading::membership_of(g, shape);
        // An instance draws what its source draws, and is written through it.
        if held.is_empty() {
            if crate::instancing::source(g, shape) != NO_NODE {
                return;
            }
            // A scene with no shading group of the shape's: the default one, as a bare mesh has.
            self.shading.push(format!("connectAttr {} \":initialShadingGroup.dsm\" -na;", quoted(&format!("{path}.iog"))));
            return;
        }
        // Ids in the file count the faces it writes.
        let g = &mut self.s.graph;
        let mesh = crate::components::mesh_of(g, shape);
        let rank: Vec<u32> = mesh.as_ref().map_or(Vec::new(), |m| {
            let mut next = 0;
            (0..m.face_capacity())
                .map(|f| {
                    let r = next;
                    if m.face_valid(f) {
                        next += 1;
                    }
                    r
                })
                .collect()
        });
        let groups = held.iter().filter(|h| h.1.is_some()).count();
        if groups > 0 {
            let _ = writeln!(self.out, "\tsetAttr -s {} \".iog[0].og\";", first_group + groups);
        }
        let mut k = first_group;
        for (sg, faces) in held.iter_mut() {
            let sg_ref = node_ref(&self.s.graph, *sg);
            let Some(faces) = faces else {
                self.shading.push(format!("connectAttr {} {} -na;", quoted(&format!("{path}.iog")), quoted(&format!("{sg_ref}.dsm"))));
                continue;
            };
            let ids: Vec<u32> = faces.iter().filter_map(|&f| rank.get(f as usize).copied()).collect();
            let _ = writeln!(self.out, "\tsetAttr \".iog[0].og[{k}].gcl\" -type \"componentList\" {};", face_text(&ids));
            let id = self.fresh("groupId");
            self.pending.push(format!("createNode groupId -n {};\n\tsetAttr \".ihi\" 0;\n", quoted(&id)));
            self.conn(&format!("{id}.id"), &format!("{path}.iog.og[{k}].gid"));
            self.conn(&format!("{sg_ref}.mwc"), &format!("{path}.iog.og[{k}].gco"));
            self.shading.push(format!("connectAttr {} {} -na;", quoted(&format!("{path}.iog.og[{k}]")), quoted(&format!("{sg_ref}.dsm"))));
            self.shading.push(format!("connectAttr {} {} -na;", quoted(&format!("{id}.msg")), quoted(&format!("{sg_ref}.gn"))));
            k += 1;
        }
    }

    /// The renderer's shared settings nodes, in a fixed order, as `createNode type -s -n name`.
    pub(super) fn write_render_settings(&mut self) {
        for (name, ty) in crate::harold_nodes::DEFAULT_NODES {
            if !crate::harold_nodes::is_shared_node(name) {
                continue;
            }
            // A scene that never made them still gets them, as a scene from Maya has them.
            let lines = self.s.graph.find(name).ok().flatten().map(|id| self.plain_attr_lines(id)).unwrap_or_default();
            let _ = write!(self.out, "createNode {ty} -s -n {};\n{lines}", quoted(name));
        }
    }

    /// A shading group or a default node, in the scene's list of nodes. False for anything else.
    pub(super) fn write_shading_node(&mut self, id: NodeId) -> bool {
        let g = &self.s.graph;
        let n = g.node(id).unwrap();
        let (name, ty) = (n.name().to_string(), n.ty().name.clone());
        if crate::harold_nodes::is_shared_node(&name) {
            // Written together by write_render_settings, in a fixed order.
            return true;
        }
        if shading::is_default_node(&name) {
            let lines = self.plain_attr_lines(id);
            if !lines.is_empty() && !shading::is_shading_group(&self.s.graph, id) {
                let _ = write!(self.out, "select -ne :{name};\n{lines}");
            }
            return true;
        }
        if ty == SG_TYPE {
            let lines = self.attr_lines(id, |_, a| a.name != "renderableOnlySet");
            let _ = write!(self.out, "createNode shadingEngine -n {};\n\tsetAttr \".ihi\" 0;\n\tsetAttr \".ro\" yes;\n{lines}", quoted(&self.s.graph.path(id)));
            let p = self.s.graph.path(id);
            self.shading.push(format!("connectAttr {} \":renderPartition.st\" -na;", quoted(&format!("{p}.pa"))));
            self.needs_links.push(p);
            return true;
        }
        let list = match ty.as_str() {
            "file" => Some(":defaultTextureList1.tx"),
            "place2dTexture" | "bump2d" => Some(":defaultRenderUtilityList1.u"),
            _ if shading::is_material_type(self.s.graph.node(id).unwrap().ty()) => Some(":defaultShaderList1.s"),
            _ => None,
        };
        if let Some(list) = list {
            let p = self.s.graph.path(id);
            self.shading.push(format!("connectAttr {} {} -na;", quoted(&format!("{p}.msg")), quoted(list)));
        }
        false
    }

    /// A connection with a shading node at either end, in the reference's short names. True when it is dealt with.
    pub(super) fn shading_connection(&mut self, src: PlugRef, dst: PlugRef) -> bool {
        let g = &self.s.graph;
        let (st, dt) = (g.node(src.node).unwrap().ty(), g.node(dst.node).unwrap().ty());
        // Membership is written with the shape.
        if dt.is_a(SG_TYPE) && g.attr(&dst).name == "dagSetMembers" {
            return true;
        }
        // Between defaults: every scene starts with these.
        if shading::is_default_node(g.node(src.node).unwrap().name()) && shading::is_default_node(g.node(dst.node).unwrap().name()) && g.attr(&dst).name != "surfaceShader" {
            return true;
        }
        if g.attr(&dst).name == "surfaceShader" && g.node(src.node).unwrap().name() == shading::DEFAULT_MATERIAL && g.node(dst.node).unwrap().name() == shading::INITIAL_SG {
            return true;
        }
        let ours = |t: &NodeType| t.is_a(SG_TYPE) || shading::classification(&t.name).is_some();
        if !ours(st) && !ours(dt) {
            return false;
        }
        let (a, b) = (short_plug(g, &src), short_plug(g, &dst));
        self.conn(&a, &b);
        true
    }

    /// The lines that make the shading groups render, as every file from the reference has them.
    pub(super) fn write_light_links(&mut self) {
        if self.needs_links.is_empty() {
            return;
        }
        let _ = writeln!(self.out, "createNode lightLinker -s -n \"lightLinker1\";");
        let mut lines = String::new();
        for kind in ["link", "shadowLink"] {
            let _ = writeln!(lines, "relationship \"{kind}\" \":lightLinker1\" \":initialShadingGroup.message\" \":defaultLightSet.message\";");
            for p in &self.needs_links {
                let _ = writeln!(lines, "relationship \"{kind}\" \":lightLinker1\" {} \":defaultLightSet.message\";", quoted(&format!("{p}.message")));
            }
        }
        self.links_text = lines;
    }
}

/// What exporting a shape takes with it: its shading groups and what feeds them.
pub(super) fn keep_shading(g: &Graph, keep: &mut HashSet<NodeId>) {
    let shapes: Vec<NodeId> = keep.iter().copied().filter(|&n| g.node(n).is_some_and(|x| x.ty().is_a("mesh") || x.ty().is_a("nurbsSurface"))).collect();
    let conns = g.connections();
    for sh in shapes {
        for sg in shading::groups_of(g, sh) {
            let mut stack = vec![sg];
            while let Some(at) = stack.pop() {
                if keep.insert(at) {
                    for (d, s) in &conns {
                        if d.node == at && g.node(s.node).is_some_and(|n| !n.ty().dag) {
                            stack.push(s.node);
                        }
                    }
                }
            }
        }
    }
}
