//! glTF 2.0 export: `.gltf` (JSON with its buffer inside it) or `.glb` (one binary file).
//!
//! What goes out: the transform hierarchy of the objects asked for, their meshes as they stand (deformed, triangulated,
//! split along hard edges and UV seams), normals, every UV set, vertex colours, and each shading group's material with
//! its base colour, transparency, emission and colour texture, and skins: the joints as nodes, each bound mesh in its
//! bind pose with its four heaviest weights a point and the joints' inverse bind matrices. Lengths go from
//! centimetres to metres.
//!
//! The format is Y up. A Z-up scene is written Y up by changing the basis of everything in it (each node's matrix,
//! the points, the normals, the bind matrices), so no node carries a turn the scene did not have: a point at Z 2 goes
//! out at Y 2. `-upAxis z` writes Z up instead, which for a Z-up scene is the data as it is.
//! Not written: animation, blend shapes, cameras and lights.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use aztek_geo::render_mesh::{build_render_mesh, RenderMeshOptions};
use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DQuat, DVec3};
use serde_json::{json, Map, Value};

use crate::graph::Graph;
use crate::plug::{NodeId, NO_NODE};
use crate::session::Session;
use crate::transform::Transform;

/// Centimetres to metres.
const SCALE: f64 = 0.01;

const FLOAT: u32 = 5126;
const UINT: u32 = 5125;
const USHORT: u32 = 5123;
const ARRAY_BUFFER: u32 = 34962;
const ELEMENT_ARRAY_BUFFER: u32 = 34963;

#[derive(Default)]
struct Doc {
    bin: Vec<u8>,
    views: Vec<Value>,
    accessors: Vec<Value>,
    meshes: Vec<Value>,
    materials: Vec<Value>,
    textures: Vec<Value>,
    images: Vec<Value>,
    samplers: Vec<Value>,
    nodes: Vec<Value>,
    extensions: Vec<&'static str>,
    skins: Vec<Value>,
    /// From the scene's axes to the file's: identity when both have the same up axis.
    basis: DMat4,
    /// Transforms written though no mesh is under them: the joints skins use, and what is above them.
    needed: HashSet<NodeId>,
    /// The glTF node of each transform written.
    node_of: HashMap<NodeId, usize>,
    /// (glTF node carrying the mesh, skinCluster) for each bound mesh written with weights.
    bound: Vec<(usize, NodeId)>,
    material_of: HashMap<NodeId, usize>,
    image_of: HashMap<String, Option<usize>>,
    notes: Vec<String>,
}

impl Doc {
    fn view(&mut self, bytes: &[u8], target: Option<u32>) -> usize {
        while self.bin.len() % 4 != 0 {
            self.bin.push(0);
        }
        let mut v = json!({"buffer": 0, "byteOffset": self.bin.len(), "byteLength": bytes.len()});
        if let Some(t) = target {
            v["target"] = json!(t);
        }
        self.bin.extend_from_slice(bytes);
        self.views.push(v);
        self.views.len() - 1
    }

    fn floats(&mut self, data: &[f32], width: usize, kind: &str, bounds: bool) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|f| f.to_le_bytes()).collect();
        let view = self.view(&bytes, Some(ARRAY_BUFFER));
        let mut a = json!({"bufferView": view, "componentType": FLOAT, "count": data.len() / width, "type": kind});
        if bounds {
            let (mut lo, mut hi) = (vec![f32::MAX; width], vec![f32::MIN; width]);
            for row in data.chunks(width) {
                for (k, v) in row.iter().enumerate() {
                    lo[k] = lo[k].min(*v);
                    hi[k] = hi[k].max(*v);
                }
            }
            a["min"] = json!(lo);
            a["max"] = json!(hi);
        }
        self.accessors.push(a);
        self.accessors.len() - 1
    }

    /// A matrix of the scene's, in the file's axes and metres.
    fn matrix(&self, m: DMat4) -> DMat4 {
        let mut out = self.basis * m * self.basis.inverse();
        out.w_axis = (out.w_axis.truncate() * SCALE).extend(1.0);
        out
    }

    /// Data that isn't a vertex attribute: no buffer target.
    fn matrices(&mut self, data: &[DMat4]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|m| m.to_cols_array()).flat_map(|f| (f as f32).to_le_bytes()).collect();
        let view = self.view(&bytes, None);
        self.accessors.push(json!({"bufferView": view, "componentType": FLOAT, "count": data.len(), "type": "MAT4"}));
        self.accessors.len() - 1
    }

    fn joints(&mut self, data: &[[u16; 4]]) -> usize {
        let bytes: Vec<u8> = data.iter().flatten().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.view(&bytes, Some(ARRAY_BUFFER));
        self.accessors.push(json!({"bufferView": view, "componentType": USHORT, "count": data.len(), "type": "VEC4"}));
        self.accessors.len() - 1
    }

    fn indices(&mut self, data: &[u32]) -> usize {
        let bytes: Vec<u8> = data.iter().flat_map(|i| i.to_le_bytes()).collect();
        let view = self.view(&bytes, Some(ELEMENT_ARRAY_BUFFER));
        self.accessors.push(json!({"bufferView": view, "componentType": UINT, "count": data.len(), "type": "SCALAR"}));
        self.accessors.len() - 1
    }

    fn uses(&mut self, extension: &'static str) {
        if !self.extensions.contains(&extension) {
            self.extensions.push(extension);
        }
    }
}

fn base64(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if c.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

/// A path as a URI relative to the folder the file is written in, when it is under it; the whole path otherwise.
fn uri_for(image: &Path, out_dir: &Path) -> String {
    let rel = image.strip_prefix(out_dir).unwrap_or(image);
    let text = rel.to_string_lossy().replace('\\', "/");
    let mut out = String::new();
    for b in text.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' | b':' => out.push(b as char),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

/// The index of the texture for an image file. None when the format can't hold it: glTF takes PNG and JPEG.
fn texture(doc: &mut Doc, t: &crate::shading::TextureRef, out_dir: &Path, embed: bool) -> Option<usize> {
    let path = PathBuf::from(t.resolved.clone()?);
    let key = path.to_string_lossy().into_owned();
    let image = match doc.image_of.get(&key) {
        Some(i) => *i,
        None => {
            let ext = path.extension().map(|e| e.to_string_lossy().to_ascii_lowercase()).unwrap_or_default();
            let mime = match ext.as_str() {
                "png" => Some("image/png"),
                "jpg" | "jpeg" => Some("image/jpeg"),
                _ => None,
            };
            let made = match mime {
                None => {
                    doc.notes.push(format!("{}: not PNG or JPEG, so left off its material", path.display()));
                    None
                }
                Some(mime) if embed => match std::fs::read(&path) {
                    Ok(bytes) => {
                        let view = doc.view(&bytes, None);
                        doc.images.push(json!({"bufferView": view, "mimeType": mime, "name": path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()}));
                        Some(doc.images.len() - 1)
                    }
                    Err(_) => {
                        doc.notes.push(format!("{}: could not be read", path.display()));
                        None
                    }
                },
                Some(_) => {
                    doc.images.push(json!({"uri": uri_for(&path, out_dir)}));
                    Some(doc.images.len() - 1)
                }
            };
            doc.image_of.insert(key, made);
            made
        }
    }?;
    // 10497 repeat, 33648 mirrored repeat, 33071 clamp to edge.
    let wrap = |k: usize| if t.mirror[k] { 33648 } else if t.wrap[k] { 10497 } else { 33071 };
    let sampler = json!({"magFilter": 9729, "minFilter": 9987, "wrapS": wrap(0), "wrapT": wrap(1)});
    let s = doc.samplers.iter().position(|x| *x == sampler).unwrap_or_else(|| {
        doc.samplers.push(sampler);
        doc.samplers.len() - 1
    });
    doc.textures.push(json!({"source": image, "sampler": s}));
    Some(doc.textures.len() - 1)
}

fn material(doc: &mut Doc, s: &mut Session, sg: NodeId, out_dir: &Path, embed: bool) -> Option<usize> {
    if let Some(i) = doc.material_of.get(&sg) {
        return Some(*i);
    }
    let (scene, project) = (s.scene_path.clone(), crate::shading::project_dir(s));
    let g = &mut s.graph;
    let d = crate::shading::draw_material(g, sg, scene.as_deref(), project.as_deref())?;
    let ty = g.node(d.material)?.ty().name.clone();
    let num = |g: &mut Graph, attr: &str| crate::shading::value_f(g, d.material, attr);
    // What the material types say of metal and roughness; the older ones say nothing, and a lambert is all rough.
    let (metallic, roughness) = match ty.as_str() {
        "standardSurface" => (num(g, "metalness").unwrap_or(0.0), num(g, "specularRoughness").unwrap_or(0.2)),
        "aiStandardSurface" => (num(g, "metalness").unwrap_or(0.0), num(g, "specular_roughness").unwrap_or(0.2)),
        "lambert" | "surfaceShader" => (0.0, 1.0),
        _ => (0.0, 0.5),
    };
    let alpha = (1.0 - d.transparency).clamp(0.0, 1.0);
    let mut pbr = Map::new();
    let mut base = [d.color[0], d.color[1], d.color[2], alpha];
    if let Some(t) = &d.color_map {
        if let Some(index) = texture(doc, t, out_dir, embed) {
            // The texture is the colour; the factor multiplies it.
            base = [1.0, 1.0, 1.0, alpha];
            let mut info = json!({"index": index});
            if t.repeat != [1.0, 1.0] || t.offset != [0.0, 0.0] {
                // UVs here run up from the bottom and glTF's down from the top: the same repeat, with the offset turned over.
                info["extensions"] = json!({"KHR_texture_transform": {"scale": t.repeat, "offset": [t.offset[0], 1.0 - t.repeat[1] - t.offset[1]]}});
                doc.uses("KHR_texture_transform");
            }
            pbr.insert("baseColorTexture".into(), info);
        }
    }
    pbr.insert("baseColorFactor".into(), json!(base));
    pbr.insert("metallicFactor".into(), json!(metallic.clamp(0.0, 1.0)));
    pbr.insert("roughnessFactor".into(), json!(roughness.clamp(0.0, 1.0)));
    let name = g.path(d.material);
    let mut m = json!({"name": name, "pbrMetallicRoughness": pbr, "doubleSided": true});
    if d.incandescence != [0.0, 0.0, 0.0] {
        m["emissiveFactor"] = json!(d.incandescence.map(|c| c.clamp(0.0, 1.0)));
    }
    if alpha < 1.0 {
        m["alphaMode"] = json!("BLEND");
    }
    if d.unlit {
        m["extensions"] = json!({"KHR_materials_unlit": {}});
        doc.uses("KHR_materials_unlit");
    }
    doc.materials.push(m);
    doc.material_of.insert(sg, doc.materials.len() - 1);
    Some(doc.materials.len() - 1)
}

/// A shape's mesh as a glTF mesh: one primitive per shading group, all on the same vertices.
/// The skin on a shape, with the mesh it deforms (the bind pose) when that has the points the shape ends up with.
fn bound(s: &mut Session, shape: NodeId, points: usize) -> Option<(NodeId, aztek_geo::mesh::Mesh, std::sync::Arc<crate::skin::SkinBind>)> {
    let sk = crate::skin::skin_of(&s.graph, shape)?;
    let bind = crate::skin::bind_of(&s.graph, sk)?;
    let input = s.graph.value(&crate::plug::PlugRef::new(sk, crate::skin::SK_IN as i32)).ok()?;
    let cage = input.as_mesh()?.clone();
    (cage.num_points() as usize == points && bind.weights.len() == points).then(|| (sk, (*cage).clone(), bind))
}

/// Writes a shape's mesh. The second value is its skinCluster when the mesh went out bound: in its bind pose, with
/// JOINTS_0 and WEIGHTS_0.
fn mesh(doc: &mut Doc, s: &mut Session, shape: NodeId, out_dir: &Path, embed: bool) -> Option<(usize, Option<NodeId>)> {
    let ptr = crate::modifiers::export_mesh(&mut s.graph, shape)?;
    let mut m = (*ptr).clone();
    if m.num_faces() == 0 {
        return None;
    }
    let skin = bound(s, shape, m.num_points() as usize);
    if crate::skin::skin_of(&s.graph, shape).is_some() && skin.is_none() {
        doc.notes.push(format!("{}: something after its skin changes its points, so it is written as it stands, without weights", s.graph.path(shape)));
    }
    if let Some((_, cage, _)) = &skin {
        // The bind pose is what the joints move.
        m = cage.clone();
    }
    let groups = crate::shading::membership_of(&s.graph, shape);
    let of_face = crate::shading::face_groups(&m, &groups);
    {
        let ids = m.face_attrs_mut().get_or_add::<i32>("material");
        for (f, id) in ids.iter_mut().enumerate() {
            *id = of_face.get(f).copied().filter(|k| *k != usize::MAX).map_or(-1, |k| k as i32);
        }
    }
    // The sets in their own order: the first is TEXCOORD_0, whichever is current.
    let layers: Vec<usize> = crate::uv_sets::all(&m).iter().filter_map(|n| crate::uv_sets::layer(&m, n)).filter_map(|l| l.strip_prefix("uv").and_then(|k| k.parse().ok())).collect();
    let r = build_render_mesh(&m, &RenderMeshOptions { wireframe: false, sections: true, colors: true, tangents: false, ..Default::default() });
    if r.positions.is_empty() || r.indices.is_empty() {
        return None;
    }
    let mut attributes = Map::new();
    let basis = doc.basis;
    let positions: Vec<f32> = r.positions.iter().flat_map(|p| (basis.transform_point3(p.as_dvec3()) * SCALE).as_vec3().to_array()).collect();
    attributes.insert("POSITION".into(), json!(doc.floats(&positions, 3, "VEC3", true)));
    if r.normals.len() == r.positions.len() {
        let normals: Vec<f32> = r.normals.iter().flat_map(|n| basis.transform_vector3(n.as_dvec3()).as_vec3().try_normalize().unwrap_or(glam::Vec3::Y).to_array()).collect();
        attributes.insert("NORMAL".into(), json!(doc.floats(&normals, 3, "VEC3", false)));
    }
    let mut skinned = None;
    if let Some((sk, _, bind)) = &skin {
        // Influences are numbered by their place in the skin's joint list; the four heaviest a point, summing to one.
        let order: HashMap<u32, u16> = crate::skin::influences(&s.graph, *sk).iter().enumerate().map(|(k, (element, _))| (*element as u32, k as u16)).collect();
        let (mut joints, mut weights) = (Vec::with_capacity(r.positions.len()), Vec::with_capacity(r.positions.len() * 4));
        let mut dropped = false;
        for &point in &r.vertex_point {
            let mut w: Vec<(u16, f64)> = bind.weights.get(point as usize).map(|w| w.iter().filter_map(|(e, x)| order.get(e).map(|k| (*k, *x))).collect()).unwrap_or_default();
            w.sort_by(|a, b| b.1.total_cmp(&a.1));
            dropped |= w.len() > 4;
            w.truncate(4);
            let sum: f64 = w.iter().map(|x| x.1).sum();
            let (mut j, mut x) = ([0u16; 4], [0f32; 4]);
            for (k, (joint, weight)) in w.iter().enumerate() {
                j[k] = *joint;
                x[k] = if sum > 0.0 { (*weight / sum) as f32 } else { 0.0 };
            }
            if sum <= 0.0 {
                x[0] = 1.0;
            }
            joints.push(j);
            weights.extend_from_slice(&x);
        }
        if dropped {
            doc.notes.push(format!("{}: points with more than four influences keep their four heaviest", s.graph.path(shape)));
        }
        attributes.insert("JOINTS_0".into(), json!(doc.joints(&joints)));
        attributes.insert("WEIGHTS_0".into(), json!(doc.floats(&weights, 4, "VEC4", false)));
        skinned = Some(*sk);
    }
    let mut slot = 0;
    for layer in layers {
        let Some(uv) = r.uvs.get(layer).filter(|u| u.len() == r.positions.len()) else { continue };
        // V runs down from the top in glTF.
        let flat: Vec<f32> = uv.iter().flat_map(|v| [v[0], 1.0 - v[1]]).collect();
        attributes.insert(format!("TEXCOORD_{slot}"), json!(doc.floats(&flat, 2, "VEC2", false)));
        slot += 1;
    }
    if r.colors.len() == r.positions.len() {
        let flat: Vec<f32> = r.colors.iter().flat_map(|c| c.map(|x| x.clamp(0.0, 1.0))).collect();
        attributes.insert("COLOR_0".into(), json!(doc.floats(&flat, 4, "VEC4", false)));
    }
    let mut primitives = Vec::new();
    for section in &r.sections {
        let (a, b) = (section.first_index as usize, (section.first_index + section.index_count) as usize);
        if a >= b || b > r.indices.len() {
            continue;
        }
        let mut p = json!({"attributes": attributes, "indices": doc.indices(&r.indices[a..b]), "mode": 4});
        let sg = usize::try_from(section.material).ok().and_then(|k| groups.get(k)).map(|g| g.0);
        if let Some(i) = sg.and_then(|sg| material(doc, s, sg, out_dir, embed)) {
            p["material"] = json!(i);
        }
        primitives.push(p);
    }
    if primitives.is_empty() {
        return None;
    }
    doc.meshes.push(json!({"name": s.graph.path(shape), "primitives": primitives}));
    Some((doc.meshes.len() - 1, skinned))
}

/// A node's own name, without the path above it or its namespace.
fn leaf(g: &Graph, id: NodeId) -> String {
    let path = g.path(id);
    path.rsplit('|').next().unwrap_or(&path).to_string()
}

fn is_transform(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("transform"))
}

fn visible_shape(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("mesh") && !n.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT))
}

fn has_mesh_below(g: &Graph, id: NodeId) -> bool {
    g.children(id).iter().copied().any(|c| visible_shape(g, c) || (is_transform(g, c) && has_mesh_below(g, c)))
}

/// A transform and what is under it. `matrix` is the node's own, or its world one for the top of what is exported.
fn node(doc: &mut Doc, s: &mut Session, id: NodeId, top: bool, out_dir: &Path, embed: bool, counts: &mut (usize, usize)) -> usize {
    let n = s.graph.node(id).unwrap();
    // In the file's axes and metres: the same turn and scale seen from there, the offset a hundredth.
    let m: DMat4 = doc.matrix(n.matrix(if top { Transform::WORLD_MATRIX } else { Transform::MATRIX }));
    let name = leaf(&s.graph, id);
    let mut out = json!({"name": name});
    let (scale, rotation, translation) = m.to_scale_rotation_translation();
    // The change of axes leaves dust where a zero was: a hundred-millionth of a millimetre is nothing.
    let translation = translation.map(|v| if v.abs() < 1e-11 { 0.0 } else { v });
    let rebuilt = DMat4::from_scale_rotation_translation(scale, rotation, translation);
    let plain = (0..4).all(|c| (rebuilt.col(c) - m.col(c)).length() < 1e-6 * (1.0 + m.col(c).length()));
    if plain {
        if translation.length() > 0.0 {
            out["translation"] = json!(translation.to_array());
        }
        if rotation.angle_between(DQuat::IDENTITY) > 1e-9 {
            out["rotation"] = json!(rotation.normalize().to_array());
        }
        if (scale - DVec3::ONE).length() > 1e-9 {
            out["scale"] = json!(scale.to_array());
        }
    } else if m != DMat4::IDENTITY {
        // Sheared: a matrix says it where three values can't.
        out["matrix"] = json!(m.to_cols_array());
    }
    let children: Vec<NodeId> = s.graph.children(id).to_vec();
    let shapes: Vec<NodeId> = children.iter().copied().filter(|&c| visible_shape(&s.graph, c)).collect();
    let mut below = Vec::new();
    let mut own_skin = None;
    for (k, shape) in shapes.iter().enumerate() {
        let Some((mesh_index, skin)) = mesh(doc, s, *shape, out_dir, embed) else { continue };
        counts.1 += 1;
        if k == 0 {
            out["mesh"] = json!(mesh_index);
            own_skin = skin;
        } else {
            // glTF has one mesh to a node: a second shape rides on a node of its own.
            doc.nodes.push(json!({"name": leaf(&s.graph, *shape), "mesh": mesh_index}));
            below.push(doc.nodes.len() - 1);
            if let Some(sk) = skin {
                doc.bound.push((doc.nodes.len() - 1, sk));
            }
        }
    }
    for c in children {
        // What leads to a mesh, and the joints skins use. Cameras and lights have nothing to write yet.
        if is_transform(&s.graph, c) && (has_mesh_below(&s.graph, c) || doc.needed.contains(&c)) {
            below.push(node(doc, s, c, false, out_dir, embed, counts));
        }
    }
    if !below.is_empty() {
        out["children"] = json!(below);
    }
    counts.0 += 1;
    doc.nodes.push(out);
    let index = doc.nodes.len() - 1;
    doc.node_of.insert(id, index);
    if let Some(sk) = own_skin {
        doc.bound.push((index, sk));
    }
    index
}

/// The joints the skins under these objects use, each with everything above it.
fn skeletons(g: &Graph, tops: &[NodeId]) -> HashSet<NodeId> {
    let mut needed = HashSet::new();
    let mut stack: Vec<NodeId> = tops.to_vec();
    while let Some(id) = stack.pop() {
        for &c in g.children(id) {
            if is_transform(g, c) {
                stack.push(c);
            } else if visible_shape(g, c) {
                let Some(sk) = crate::skin::skin_of(g, c) else { continue };
                for (_, joint) in crate::skin::influences(g, sk) {
                    let mut at = joint;
                    while at != NO_NODE && needed.insert(at) {
                        at = g.parent(at);
                    }
                }
            }
        }
    }
    needed
}

/// The skin objects, once every joint has its node: the joints in the skin's own order and their inverse bind matrices.
fn write_skins(doc: &mut Doc, g: &Graph) {
    let mut written: HashMap<NodeId, Option<usize>> = HashMap::new();
    for (node_index, sk) in doc.bound.clone() {
        let index = match written.get(&sk) {
            Some(i) => *i,
            None => {
                let made = (|| {
                    let bind = crate::skin::bind_of(g, sk)?;
                    let mut joints = Vec::new();
                    let mut inverse = Vec::new();
                    for (element, joint) in crate::skin::influences(g, sk) {
                        joints.push(*doc.node_of.get(&joint)?);
                        // A point of the mesh at the bind, seen from the joint as it stood then.
                        let pre = bind.bind_pre.get(element as usize).copied().unwrap_or(DMat4::IDENTITY);
                        inverse.push(doc.matrix(pre * bind.geom));
                    }
                    if joints.is_empty() {
                        return None;
                    }
                    let accessor = doc.matrices(&inverse);
                    doc.skins.push(json!({"name": g.path(sk), "joints": joints, "inverseBindMatrices": accessor}));
                    Some(doc.skins.len() - 1)
                })();
                if made.is_none() {
                    doc.notes.push(format!("{}: a joint of it is not in what was exported, so its mesh went out without a skin", g.path(sk)));
                }
                written.insert(sk, made);
                made
            }
        };
        if let Some(i) = index {
            doc.nodes[node_index]["skin"] = json!(i);
        }
    }
}

/// Writes the objects (and what is under them) to `path`: `.glb` is binary, anything else JSON. Returns what was
/// written and what was left out.
pub fn export(s: &mut Session, path: &str, objects: &[NodeId], up_z: bool) -> CmdResult {
    let out = PathBuf::from(path);
    let glb = out.extension().is_some_and(|e| e.eq_ignore_ascii_case("glb"));
    let out_dir = out.parent().map(Path::to_path_buf).unwrap_or_default();
    s.graph.evaluate();
    // Each object once, and not one that another being exported already holds.
    let mut tops: Vec<NodeId> = Vec::new();
    for &id in objects {
        let t = if is_transform(&s.graph, id) { id } else { crate::dag::transform_of(&s.graph, id) };
        if !is_transform(&s.graph, t) || tops.contains(&t) {
            continue;
        }
        tops.push(t);
    }
    tops.retain(|&t| has_mesh_below(&s.graph, t));
    let all = tops.clone();
    tops.retain(|&t| {
        let mut p = s.graph.parent(t);
        while p != NO_NODE {
            if all.contains(&p) {
                return false;
            }
            p = s.graph.parent(p);
        }
        true
    });
    // From the scene's up axis to the file's: a quarter turn about X one way or the other, or nothing.
    let basis = match (crate::world::up_z(), up_z) {
        (true, false) => DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2),
        (false, true) => DMat4::from_rotation_x(std::f64::consts::FRAC_PI_2),
        _ => DMat4::IDENTITY,
    };
    // Skeletons go out with the meshes bound to them: a joint's top-most parent is exported if nothing asked for holds it.
    let mut doc = Doc { basis, needed: skeletons(&s.graph, &tops), ..Default::default() };
    let mut skeleton_tops: Vec<NodeId> = doc.needed.iter().copied().filter(|&j| s.graph.parent(j) == NO_NODE).collect();
    skeleton_tops.sort_unstable();
    for t in skeleton_tops {
        if !tops.contains(&t) {
            tops.push(t);
        }
    }
    let mut counts = (0usize, 0usize);
    let mut roots = Vec::new();
    for t in tops {
        // A joint under something that was asked for is written there, in its place.
        if doc.node_of.contains_key(&t) {
            continue;
        }
        roots.push(node(&mut doc, s, t, true, &out_dir, glb, &mut counts));
    }
    if counts.1 == 0 {
        fail!("exportGltf: no meshes to export");
    }
    write_skins(&mut doc, &s.graph);
    if up_z {
        doc.notes.push("written Z up: the format is Y up, and a reader that follows it will turn the scene".to_string());
    }
    while doc.bin.len() % 4 != 0 {
        doc.bin.push(0);
    }
    let mut buffer = json!({"byteLength": doc.bin.len()});
    if !glb {
        buffer["uri"] = json!(format!("data:application/octet-stream;base64,{}", base64(&doc.bin)));
    }
    let mut root = json!({
        "asset": {"version": "2.0", "generator": format!("Aztek {}", env!("CARGO_PKG_VERSION"))},
        "scene": 0,
        "scenes": [{"nodes": roots}],
        "nodes": doc.nodes,
        "meshes": doc.meshes,
        "buffers": [buffer],
        "bufferViews": doc.views,
        "accessors": doc.accessors,
    });
    for (key, list) in [("materials", &doc.materials), ("textures", &doc.textures), ("images", &doc.images), ("samplers", &doc.samplers), ("skins", &doc.skins)] {
        if !list.is_empty() {
            root[key] = json!(list);
        }
    }
    if !doc.extensions.is_empty() {
        root["extensionsUsed"] = json!(doc.extensions);
    }
    let bytes = if glb {
        let mut text = root.to_string().into_bytes();
        while text.len() % 4 != 0 {
            text.push(b' ');
        }
        let total = 12 + 8 + text.len() + 8 + doc.bin.len();
        let mut file = Vec::with_capacity(total);
        file.extend_from_slice(b"glTF");
        file.extend_from_slice(&2u32.to_le_bytes());
        file.extend_from_slice(&(total as u32).to_le_bytes());
        file.extend_from_slice(&(text.len() as u32).to_le_bytes());
        file.extend_from_slice(b"JSON");
        file.extend_from_slice(&text);
        file.extend_from_slice(&(doc.bin.len() as u32).to_le_bytes());
        file.extend_from_slice(b"BIN\0");
        file.extend_from_slice(&doc.bin);
        file
    } else {
        serde_json::to_vec_pretty(&root).unwrap_or_default()
    };
    if std::fs::write(&out, bytes).is_err() {
        fail!("exportGltf: can't write {path}");
    }
    Ok(json!({"path": path, "nodes": counts.0, "meshes": counts.1, "materials": doc.materials.len(), "textures": doc.textures.len(), "skins": doc.skins.len(), "notes": doc.notes}))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "exportGltf",
            "Export to glTF 2.0: a .glb file (binary, textures inside it) or a .gltf file (JSON, textures by path). The objects \
             named, or the selection, with what is under them: hierarchy, meshes as they stand, normals, UV sets, vertex \
             colours, materials, colour textures and skins (joints, weights, bind matrices), in metres. The file is Y up, \
             as the format is: a Z-up scene is converted, with no node added to carry the turn. -upAxis z writes Z up \
             instead. Animation, blend shapes, cameras and lights are not written. -all exports every top-level object.",
            |s, a| {
                let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("exportGltf: path required") };
                let mut ids = Vec::new();
                for k in 1..a.positional.len() {
                    let Some(name) = a.pos_str(k) else { continue };
                    match s.graph.find(name) {
                        Ok(Some(id)) => ids.push(id),
                        _ => fail!("exportGltf: no object named {name}"),
                    }
                }
                if a.bool("all") {
                    let g = &s.graph;
                    let mut every = g.nodes();
                    every.sort_unstable();
                    ids = every.into_iter().filter(|&id| is_transform(g, id) && g.parent(id) == NO_NODE).collect();
                } else if ids.is_empty() {
                    ids = s.graph.selection().to_vec();
                }
                if ids.is_empty() {
                    fail!("exportGltf: nothing selected");
                }
                let up = a.string("upAxis", "y").to_ascii_lowercase();
                if up != "y" && up != "z" {
                    fail!("exportGltf: -upAxis is y or z");
                }
                export(s, &path, &ids, up == "z")
            },
        )
        .positional("path objects...")
        .flag(FlagDef::new("all", "all", ArgType::Bool, Value::Null, "Every top-level object, not the selection."))
        .flag(FlagDef::new("upAxis", "up", ArgType::String, Value::Null, "The file's up axis: y (the format's, the default) or z."))
        .undoable(false),
    );
}
