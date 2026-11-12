//! FBX export: a binary FBX 7.4 file, written from the format's public descriptions with no library of its vendor's.
//!
//! What goes out: the transform hierarchy of the objects asked for; their meshes as polygons (quads stay quads) with
//! normals, every UV set and a material per face; each shading group's material with its colour and colour texture;
//! and skins: the joints as limb nodes, a bound mesh in its bind pose, a cluster a joint with its points and weights,
//! and the bind pose. Lengths are centimetres, as the file says of itself, and the scene's up axis is written in the
//! file's settings, so nothing is turned. A node's pivots and a joint's orientation are folded into its translation,
//! rotation and scale. Not written: animation, blend shapes, cameras and lights.

use std::collections::HashMap;
use std::path::Path;

use aztek_geo::normals::{corner_normals, CornerNormalOptions};
use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::gltf_export::{has_mesh_below, is_transform, leaf, skeletons, visible_shape};
use crate::graph::Graph;
use crate::plug::{NodeId, NO_NODE};
use crate::session::Session;
use crate::transform::Transform;

const VERSION: u32 = 7400;

/// One value of a record.
#[derive(Clone, Debug, PartialEq)]
pub enum Prop {
    I(i32),
    L(i64),
    D(f64),
    S(String),
    Raw(Vec<u8>),
    Ints(Vec<i32>),
    Doubles(Vec<f64>),
}

/// A record: a name, its values, and the records inside it.
#[derive(Clone, Debug, Default)]
pub struct Rec {
    pub name: String,
    pub props: Vec<Prop>,
    pub kids: Vec<Rec>,
}

impl Rec {
    fn new(name: &str, props: Vec<Prop>) -> Rec {
        Rec { name: name.to_string(), props, kids: Vec::new() }
    }
    fn with(mut self, kids: Vec<Rec>) -> Rec {
        self.kids = kids;
        self
    }
    fn push(&mut self, kid: Rec) {
        self.kids.push(kid);
    }
}

fn s(text: &str) -> Prop {
    Prop::S(text.to_string())
}

/// A name with its class, as the binary form joins them.
fn named(name: &str, class: &str) -> Prop {
    Prop::S(format!("{name}\u{0}\u{1}{class}"))
}

fn write_prop(out: &mut Vec<u8>, p: &Prop) {
    match p {
        Prop::I(v) => {
            out.push(b'I');
            out.extend_from_slice(&v.to_le_bytes());
        }
        Prop::L(v) => {
            out.push(b'L');
            out.extend_from_slice(&v.to_le_bytes());
        }
        Prop::D(v) => {
            out.push(b'D');
            out.extend_from_slice(&v.to_le_bytes());
        }
        Prop::S(v) => {
            out.push(b'S');
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(v.as_bytes());
        }
        Prop::Raw(v) => {
            out.push(b'R');
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(v);
        }
        // Arrays: how many, how packed (0 is plain), how many bytes, the values.
        Prop::Ints(v) => {
            out.push(b'i');
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&((v.len() * 4) as u32).to_le_bytes());
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
        Prop::Doubles(v) => {
            out.push(b'd');
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&((v.len() * 8) as u32).to_le_bytes());
            for x in v {
                out.extend_from_slice(&x.to_le_bytes());
            }
        }
    }
}

/// A record in the file: where it ends, how many values, their length, its name, the values, and the records inside
/// it closed by an empty one.
fn write_rec(out: &mut Vec<u8>, r: &Rec) {
    let start = out.len();
    out.extend_from_slice(&[0u8; 12]);
    out.push(r.name.len() as u8);
    out.extend_from_slice(r.name.as_bytes());
    let props_at = out.len();
    for p in &r.props {
        write_prop(out, p);
    }
    let props_len = (out.len() - props_at) as u32;
    if !r.kids.is_empty() {
        for k in &r.kids {
            write_rec(out, k);
        }
        out.extend_from_slice(&[0u8; 13]);
    }
    let end = out.len() as u32;
    out[start..start + 4].copy_from_slice(&end.to_le_bytes());
    out[start + 4..start + 8].copy_from_slice(&(r.props.len() as u32).to_le_bytes());
    out[start + 8..start + 12].copy_from_slice(&props_len.to_le_bytes());
}

/// The file's bytes: the header, the records, and the footer readers look for.
pub fn encode(top: &[Rec]) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(b"Kaydara FBX Binary  \x00\x1a\x00");
    out.extend_from_slice(&VERSION.to_le_bytes());
    for r in top {
        write_rec(&mut out, r);
    }
    out.extend_from_slice(&[0u8; 13]);
    out.extend_from_slice(&[0xfa, 0xbc, 0xab, 0x09, 0xd0, 0xc8, 0xd4, 0x66, 0xb1, 0x76, 0xfb, 0x83, 0x1c, 0xf7, 0x26, 0x7e]);
    out.extend_from_slice(&[0u8; 4]);
    let mut pad = out.len().div_ceil(16) * 16 - out.len();
    if pad == 0 {
        pad = 16;
    }
    out.extend(std::iter::repeat_n(0u8, pad));
    out.extend_from_slice(&VERSION.to_le_bytes());
    out.extend_from_slice(&[0u8; 120]);
    out.extend_from_slice(&[0xf8, 0x5a, 0x8c, 0x6a, 0xde, 0xf5, 0xd9, 0x7e, 0xec, 0xe9, 0x0c, 0xe3, 0x75, 0x8f, 0x29, 0x0b]);
    out
}

/// A property line: name, type, label, flags, values.
fn p70(name: &str, ty: &str, label: &str, flags: &str, values: Vec<Prop>) -> Rec {
    let mut props = vec![s(name), s(ty), s(label), s(flags)];
    props.extend(values);
    Rec::new("P", props)
}

fn vec3(name: &str, ty: &str, flags: &str, v: DVec3) -> Rec {
    p70(name, ty, "", flags, vec![Prop::D(v.x), Prop::D(v.y), Prop::D(v.z)])
}

fn matrix(name: &str, m: &DMat4) -> Rec {
    Rec::new(name, vec![Prop::Doubles(m.to_cols_array().to_vec())])
}

#[derive(Default)]
struct Doc {
    next_id: i64,
    objects: Vec<Rec>,
    /// (kind, child, parent, property).
    links: Vec<(&'static str, i64, i64, Option<&'static str>)>,
    counts: HashMap<&'static str, i32>,
    model_of: HashMap<NodeId, i64>,
    material_of: HashMap<NodeId, i64>,
    /// (geometry, mesh's model, skinCluster) for each bound mesh.
    bound: Vec<(i64, i64, NodeId, NodeId)>,
    needed: std::collections::HashSet<NodeId>,
    notes: Vec<String>,
    meshes: usize,
    /// The turn and scale of the file's axes and unit, on the top nodes and every world matrix.
    k: DMat4,
    skins_on: bool,
    embed: bool,
    /// File nodes already written as textures.
    texture_of: HashMap<NodeId, i64>,
}

impl Doc {
    fn id(&mut self) -> i64 {
        self.next_id += 1;
        1_000_000 + self.next_id
    }
    fn add(&mut self, kind: &'static str, rec: Rec) {
        *self.counts.entry(kind).or_insert(0) += 1;
        self.objects.push(rec);
    }
    fn link(&mut self, child: i64, parent: i64) {
        self.links.push(("OO", child, parent, None));
    }
}

/// A file node as a texture (with its media), fed into a material's property. One file node is one texture.
fn texture(doc: &mut Doc, g: &mut Graph, file_node: NodeId, path: &str, offset: [f64; 2], repeat: [f64; 2], material: i64, property: &'static str) {
    if path.is_empty() {
        return;
    }
    let tex = match doc.texture_of.get(&file_node) {
        Some(t) => *t,
        None => {
            let tex_name = leaf(g, file_node);
            let relative = Path::new(path).file_name().map(|f| f.to_string_lossy().into_owned()).unwrap_or_default();
            let (tex, video) = (doc.id(), doc.id());
            let mut vrec = Rec::new("Video", vec![Prop::L(video), named(&tex_name, "Video"), s("Clip")]).with(vec![
                Rec::new("Type", vec![s("Clip")]),
                Rec::new("Properties70", vec![]).with(vec![p70("Path", "KString", "XRefUrl", "", vec![s(path)])]),
                Rec::new("UseMipMap", vec![Prop::I(0)]),
                Rec::new("Filename", vec![s(path)]),
                Rec::new("RelativeFilename", vec![s(&relative)]),
            ]);
            if doc.embed {
                match std::fs::read(path) {
                    Ok(bytes) => vrec.push(Rec::new("Content", vec![Prop::Raw(bytes)])),
                    Err(_) => doc.notes.push(format!("{path}: can't be read, so it is not embedded")),
                }
            }
            doc.add("Video", vrec);
            doc.add(
                "Texture",
                Rec::new("Texture", vec![Prop::L(tex), named(&tex_name, "Texture"), s("")]).with(vec![
                    Rec::new("Type", vec![s("TextureVideoClip")]),
                    Rec::new("Version", vec![Prop::I(202)]),
                    Rec::new("TextureName", vec![named(&tex_name, "Texture")]),
                    Rec::new("Properties70", vec![]).with(vec![
                        vec3("Translation", "Vector", "", DVec3::new(offset[0], offset[1], 0.0)),
                        vec3("Scaling", "Vector", "", DVec3::new(repeat[0], repeat[1], 1.0)),
                        p70("UVSet", "KString", "", "", vec![s("default")]),
                        p70("UseMaterial", "bool", "", "", vec![Prop::I(1)]),
                    ]),
                    Rec::new("Media", vec![named(&tex_name, "Video")]),
                    Rec::new("FileName", vec![s(path)]),
                    Rec::new("RelativeFilename", vec![s(&relative)]),
                    Rec::new("ModelUVTranslation", vec![Prop::D(0.0), Prop::D(0.0)]),
                    Rec::new("ModelUVScaling", vec![Prop::D(1.0), Prop::D(1.0)]),
                ]),
            );
            doc.link(video, tex);
            doc.texture_of.insert(file_node, tex);
            tex
        }
    };
    doc.links.push(("OP", tex, material, Some(property)));
}

/// The file node behind a material's normalCamera, through its bump2d: the node, its path, and whether it is a tangent
/// space normal map (else a height, with its depth).
fn bump_of(g: &mut Graph, material: NodeId, scene: Option<&str>, project: Option<&str>) -> Option<(NodeId, String, bool, f64)> {
    let dst = crate::shading::plug_named(g, material, "normalCamera")?;
    let bump = g.source(&dst)?.node;
    if g.node(bump).map(|n| n.ty().name.clone()).as_deref() != Some("bump2d") {
        return None;
    }
    let value = crate::shading::plug_named(g, bump, "bumpValue")?;
    let file = g.source(&value)?.node;
    if g.node(file).map(|n| n.ty().name.clone()).as_deref() != Some("file") {
        return None;
    }
    let interp = crate::shading::value_f(g, bump, "bumpInterp").unwrap_or(0.0);
    let depth = crate::shading::value_f(g, bump, "bumpDepth").unwrap_or(1.0);
    let name_plug = crate::shading::plug_named(g, file, "fileTextureName")?;
    let written = g.value(&name_plug).ok()?.as_str().to_string();
    let resolved = crate::shading::resolve_texture(&written, scene, project).map(|p| aztek_core::paths::to_utf8(&p)).unwrap_or(written);
    Some((file, resolved, interp >= 0.5, depth))
}

/// A shading group's material: lambert, or phong for the materials with a highlight, as the reference writes them, with
/// its colour, glow, transparency and highlight, and the images on its colour, transparency and normals.
fn material(doc: &mut Doc, sess: &mut Session, sg: NodeId) -> Option<i64> {
    if let Some(i) = doc.material_of.get(&sg) {
        return Some(*i);
    }
    let (scene, project) = (sess.scene_path.clone(), crate::shading::project_dir(sess));
    let g = &mut sess.graph;
    let d = crate::shading::draw_material(g, sg, scene.as_deref(), project.as_deref())?;
    let name = leaf(g, d.material);
    let ty = g.node(d.material).map(|n| n.ty().name.clone()).unwrap_or_default();
    let shiny = d.specular.is_some();
    let c = |v: [f32; 3]| DVec3::new(v[0] as f64, v[1] as f64, v[2] as f64);
    let mut props = vec![
        vec3("DiffuseColor", "Color", "A", c(d.color)),
        p70("DiffuseFactor", "Number", "", "A", vec![Prop::D(d.diffuse as f64)]),
        vec3("AmbientColor", "Color", "A", c(d.ambient)),
        p70("AmbientFactor", "Number", "", "A", vec![Prop::D(1.0)]),
        vec3("EmissiveColor", "Color", "A", c(d.incandescence)),
        p70("EmissiveFactor", "Number", "", "A", vec![Prop::D(1.0)]),
        vec3("TransparentColor", "Color", "A", DVec3::splat(d.transparency as f64)),
        p70("TransparencyFactor", "Number", "", "A", vec![Prop::D(1.0)]),
        p70("Opacity", "Number", "", "A", vec![Prop::D(1.0 - d.transparency as f64)]),
    ];
    if let Some(sp) = &d.specular {
        props.push(vec3("SpecularColor", "Color", "A", c(sp.color)));
        props.push(p70("SpecularFactor", "Number", "", "A", vec![Prop::D(1.0)]));
        let power = if ty == "phong" { sp.shininess as f64 } else { 20.0 };
        props.push(p70("ShininessExponent", "Number", "", "A", vec![Prop::D(power)]));
        props.push(p70("Shininess", "Number", "", "A", vec![Prop::D(power)]));
        if let Some(r) = crate::shading::value_f(g, d.material, "reflectivity") {
            props.push(p70("ReflectionFactor", "Number", "", "A", vec![Prop::D(r)]));
        }
    }
    let id = doc.id();
    doc.add(
        "Material",
        Rec::new("Material", vec![Prop::L(id), named(&name, "Material"), s("")]).with(vec![
            Rec::new("Version", vec![Prop::I(102)]),
            Rec::new("ShadingModel", vec![s(if shiny { "phong" } else { "lambert" })]),
            Rec::new("MultiLayer", vec![Prop::I(0)]),
            Rec::new("Properties70", vec![]).with(props),
        ]),
    );
    let f2 = |v: [f32; 2]| [v[0] as f64, v[1] as f64];
    for (map, property) in [(&d.color_map, "DiffuseColor"), (&d.alpha_map, "TransparentColor")] {
        if let Some(t) = map.as_ref().filter(|t| t.baked.is_none()) {
            let file = t.resolved.clone().unwrap_or_else(|| t.written.clone());
            texture(doc, g, t.file, &file, f2(t.offset), f2(t.repeat), id, property);
        }
    }
    if let Some((file, path, normal, depth)) = bump_of(g, d.material, scene.as_deref(), project.as_deref()) {
        if normal {
            texture(doc, g, file, &path, [0.0, 0.0], [1.0, 1.0], id, "NormalMap");
        } else {
            texture(doc, g, file, &path, [0.0, 0.0], [1.0, 1.0], id, "Bump");
            if let Some(Rec { kids, .. }) = doc.objects.iter_mut().rev().find(|r| r.name == "Material" && r.props.first() == Some(&Prop::L(id))) {
                if let Some(p) = kids.iter_mut().find(|k| k.name == "Properties70") {
                    p.push(p70("BumpFactor", "Number", "", "A", vec![Prop::D(depth)]));
                }
            }
        }
    }
    doc.material_of.insert(sg, id);
    Some(id)
}

/// A shape's geometry, linked under its model with its materials in the order its faces name them.
fn geometry(doc: &mut Doc, sess: &mut Session, shape: NodeId, model: i64) -> Option<()> {
    let ptr = crate::modifiers::export_mesh(&mut sess.graph, shape)?;
    let mut m = (*ptr).clone();
    if m.num_faces() == 0 {
        return None;
    }
    // A bound mesh goes out in its bind pose: the clusters move it from there.
    let skin = crate::skin::skin_of(&sess.graph, shape).filter(|_| doc.skins_on).and_then(|sk| {
        let bind = crate::skin::bind_of(&sess.graph, sk)?;
        let input = sess.graph.value(&crate::plug::PlugRef::new(sk, crate::skin::SK_IN as i32)).ok()?;
        let cage = input.as_mesh()?.clone();
        (cage.num_points() == m.num_points() && bind.weights.len() == m.num_points() as usize).then(|| (sk, (*cage).clone()))
    });
    if doc.skins_on && crate::skin::skin_of(&sess.graph, shape).is_some() && skin.is_none() {
        doc.notes.push(format!("{}: something after its skin changes its points, so it is written as it stands, without weights", sess.graph.path(shape)));
    }
    if let Some((_, cage)) = &skin {
        m = cage.clone();
    }
    let groups = crate::shading::membership_of(&sess.graph, shape);
    let of_face = crate::shading::face_groups(&m, &groups);
    let normals = corner_normals(&m, &CornerNormalOptions::default());
    let sets = crate::uv_sets::all(&m);
    let layers: Vec<(String, String)> = sets.iter().filter_map(|n| crate::uv_sets::layer(&m, n).map(|l| (n.clone(), l))).collect();

    let vertices: Vec<f64> = m.positions().iter().flat_map(|p| p.to_array()).collect();
    let (mut index, mut normal, mut face_material) = (Vec::new(), Vec::new(), Vec::new());
    let mut uvs: Vec<(Vec<f64>, Vec<i32>)> = layers.iter().map(|_| (Vec::new(), Vec::new())).collect();
    // The materials in use, in the order the faces first name them.
    let mut used: Vec<usize> = Vec::new();
    for f in 0..m.num_faces() {
        let corners = m.face_corners(f);
        if corners.len() < 3 {
            continue;
        }
        for (k, &c) in corners.iter().enumerate() {
            let point = m.corner_point(c) as i32;
            // A polygon's last index is written as its complement: that is where the polygon ends.
            index.push(if k + 1 == corners.len() { -point - 1 } else { point });
            let n = normals.get(c as usize).copied().unwrap_or(glam::Vec3::Y);
            normal.extend([n.x as f64, n.y as f64, n.z as f64]);
            for (slot, (_, layer)) in layers.iter().enumerate() {
                let uv = m.corner_attrs().get::<[f32; 2]>(layer).get(c as usize).copied().unwrap_or([0.0, 0.0]);
                let at = (uvs[slot].0.len() / 2) as i32;
                uvs[slot].1.push(at);
                uvs[slot].0.extend([uv[0] as f64, uv[1] as f64]);
            }
        }
        let group = of_face.get(f as usize).copied().filter(|k| *k != usize::MAX);
        let slot = match group {
            Some(gk) => used.iter().position(|u| *u == gk).unwrap_or_else(|| {
                used.push(gk);
                used.len() - 1
            }),
            None => 0,
        };
        face_material.push(slot as i32);
    }
    if index.is_empty() {
        return None;
    }
    let name = leaf(&sess.graph, shape);
    let id = doc.id();
    let mut rec = Rec::new("Geometry", vec![Prop::L(id), named(&name, "Geometry"), s("Mesh")]).with(vec![
        Rec::new("Vertices", vec![Prop::Doubles(vertices)]),
        Rec::new("PolygonVertexIndex", vec![Prop::Ints(index)]),
        Rec::new("GeometryVersion", vec![Prop::I(124)]),
        Rec::new("LayerElementNormal", vec![Prop::I(0)]).with(vec![
            Rec::new("Version", vec![Prop::I(101)]),
            Rec::new("Name", vec![s("")]),
            Rec::new("MappingInformationType", vec![s("ByPolygonVertex")]),
            Rec::new("ReferenceInformationType", vec![s("Direct")]),
            Rec::new("Normals", vec![Prop::Doubles(normal)]),
        ]),
    ]);
    for (slot, ((set, _), (values, idx))) in layers.iter().zip(uvs).enumerate() {
        rec.push(Rec::new("LayerElementUV", vec![Prop::I(slot as i32)]).with(vec![
            Rec::new("Version", vec![Prop::I(101)]),
            Rec::new("Name", vec![s(set)]),
            Rec::new("MappingInformationType", vec![s("ByPolygonVertex")]),
            Rec::new("ReferenceInformationType", vec![s("IndexToDirect")]),
            Rec::new("UV", vec![Prop::Doubles(values)]),
            Rec::new("UVIndex", vec![Prop::Ints(idx)]),
        ]));
    }
    let one = used.len() <= 1;
    rec.push(Rec::new("LayerElementMaterial", vec![Prop::I(0)]).with(vec![
        Rec::new("Version", vec![Prop::I(101)]),
        Rec::new("Name", vec![s("")]),
        Rec::new("MappingInformationType", vec![s(if one { "AllSame" } else { "ByPolygon" })]),
        Rec::new("ReferenceInformationType", vec![s("IndexToDirect")]),
        Rec::new("Materials", vec![Prop::Ints(if one { vec![0] } else { face_material })]),
    ]));
    // The first layer has the normals, the materials and the first UV set; each further UV set has a layer of its own.
    let element = |ty: &str, k: i32| Rec::new("LayerElement", vec![]).with(vec![Rec::new("Type", vec![s(ty)]), Rec::new("TypedIndex", vec![Prop::I(k)])]);
    let mut first = vec![Rec::new("Version", vec![Prop::I(100)]), element("LayerElementNormal", 0), element("LayerElementMaterial", 0)];
    if !layers.is_empty() {
        first.push(element("LayerElementUV", 0));
    }
    rec.push(Rec::new("Layer", vec![Prop::I(0)]).with(first));
    for k in 1..layers.len() {
        rec.push(Rec::new("Layer", vec![Prop::I(k as i32)]).with(vec![Rec::new("Version", vec![Prop::I(100)]), element("LayerElementUV", k as i32)]));
    }
    doc.add("Geometry", rec);
    doc.link(id, model);
    for gk in used {
        if let Some(mat) = groups.get(gk).map(|x| x.0).and_then(|sg| material(doc, sess, sg)) {
            doc.link(mat, model);
        }
    }
    if let Some((sk, _)) = skin {
        doc.bound.push((id, model, sk, shape));
    }
    doc.meshes += 1;
    Some(())
}

/// A transform as a model, with what is under it. The top of what is exported takes its place in the world.
fn model(doc: &mut Doc, sess: &mut Session, id: NodeId, parent: i64, top: bool) {
    let n = sess.graph.node(id).unwrap();
    let local: DMat4 = n.matrix(Transform::MATRIX);
    let joint = n.ty().is_a("joint");
    // A top node with a parent in the scene takes its place in the world.
    let folded = top && sess.graph.parent(id) != NO_NODE;
    let m = if folded { n.matrix(Transform::WORLD_MATRIX) } else { local };
    let children: Vec<NodeId> = sess.graph.children(id).to_vec();
    let shapes: Vec<NodeId> = children.iter().copied().filter(|&c| visible_shape(&sess.graph, c)).collect();
    let kind = if joint { "LimbNode" } else if shapes.is_empty() { "Null" } else { "Mesh" };
    let name = leaf(&sess.graph, id);
    let deg = |v: DVec3| DVec3::new(v.x.to_degrees(), v.y.to_degrees(), v.z.to_degrees());
    // The node's own channels, as the reference writes them: joint orient as pre rotation, rotate axis as post rotation.
    let g = &mut sess.graph;
    let ch = |g: &mut Graph, a: &str, rest: f64| crate::shading::value_v(g, id, a).unwrap_or(DVec3::splat(rest));
    let mut parts = crate::import_build::Parts {
        translate: ch(g, "translate", 0.0),
        rotate: ch(g, "rotate", 0.0),
        rotate_order: crate::shading::value_f(g, id, "rotateOrder").unwrap_or(0.0) as i64,
        joint_orient: if joint { ch(g, "jointOrient", 0.0) } else { DVec3::ZERO },
        rotate_axis: ch(g, "rotateAxis", 0.0),
        rotate_pivot: ch(g, "rotatePivot", 0.0),
        rotate_pivot_translate: ch(g, "rotatePivotTranslate", 0.0),
        scale_pivot: ch(g, "scalePivot", 0.0),
        scale_pivot_translate: ch(g, "scalePivotTranslate", 0.0),
        scale: ch(g, "scale", 1.0),
        segment_scale_compensate: joint.then(|| crate::shading::value_f(g, id, "segmentScaleCompensate").unwrap_or(1.0) != 0.0),
    };
    let built = crate::fbx_import::parts_matrix(&parts, joint);
    let close = |a: &DMat4, b: &DMat4| (0..4).all(|c| (a.col(c) - b.col(c)).length() <= 1e-6 * (1.0 + b.col(c).length()));
    let mut by_parts = !folded && close(&built, &local);
    if by_parts && top && doc.k != DMat4::IDENTITY {
        match crate::fbx_import::convert_top(&parts, joint, doc.k) {
            Some(p) => parts = p,
            None => by_parts = false,
        }
    }
    let mut props = Vec::new();
    if by_parts {
        let fbx_order = [0, 2, 4, 1, 3, 5][parts.rotate_order.clamp(0, 5) as usize];
        let ra = crate::transform::euler_to_matrix(parts.rotate_axis, crate::transform::RotateOrder::Xyz);
        let post = crate::transform::matrix_to_euler(&DMat3::from_mat4(ra.inverse()), crate::transform::RotateOrder::Xyz);
        props.push(p70("RotationActive", "bool", "", "", vec![Prop::I(1)]));
        if fbx_order != 0 {
            props.push(p70("RotationOrder", "enum", "", "", vec![Prop::I(fbx_order)]));
        }
        props.push(p70("InheritType", "enum", "", "", vec![Prop::I(if parts.segment_scale_compensate == Some(true) { 2 } else { 1 })]));
        for (prop, v) in [("RotationOffset", parts.rotate_pivot_translate), ("RotationPivot", parts.rotate_pivot), ("ScalingOffset", parts.scale_pivot_translate), ("ScalingPivot", parts.scale_pivot)] {
            if v.length() > 1e-12 {
                props.push(vec3(prop, "Vector3D", "", v));
            }
        }
        if parts.joint_orient.length() > 1e-12 {
            props.push(vec3("PreRotation", "Vector3D", "", deg(parts.joint_orient)));
        }
        if post.length() > 1e-12 {
            props.push(vec3("PostRotation", "Vector3D", "", deg(post)));
        }
        props.push(vec3("Lcl Translation", "Lcl Translation", "A", parts.translate));
        props.push(vec3("Lcl Rotation", "Lcl Rotation", "A", deg(parts.rotate)));
        props.push(vec3("Lcl Scaling", "Lcl Scaling", "A", parts.scale));
    } else {
        let m = if top { doc.k * m } else { m };
        let (scale, rotation, translation) = m.to_scale_rotation_translation();
        let euler = crate::transform::matrix_to_euler(&DMat3::from_quat(rotation), crate::transform::RotateOrder::Xyz);
        let rebuilt = DMat4::from_scale_rotation_translation(scale, rotation, translation);
        if !close(&rebuilt, &m) {
            doc.notes.push(format!("{}: its shear is not written", sess.graph.path(id)));
        }
        props.push(vec3("Lcl Translation", "Lcl Translation", "A", translation));
        props.push(vec3("Lcl Rotation", "Lcl Rotation", "A", deg(euler)));
        props.push(vec3("Lcl Scaling", "Lcl Scaling", "A", scale));
    }
    let mid = doc.id();
    doc.add(
        "Model",
        Rec::new("Model", vec![Prop::L(mid), named(&name, "Model"), s(kind)]).with(vec![
            Rec::new("Version", vec![Prop::I(232)]),
            Rec::new("Properties70", vec![]).with(props),
            Rec::new("Shading", vec![s("Y")]),
            Rec::new("Culling", vec![s("CullingOff")]),
        ]),
    );
    doc.link(mid, parent);
    doc.model_of.insert(id, mid);
    if joint {
        let attr = doc.id();
        doc.add("NodeAttribute", Rec::new("NodeAttribute", vec![Prop::L(attr), named(&name, "NodeAttribute"), s("LimbNode")]).with(vec![Rec::new("TypeFlags", vec![s("Skeleton")])]));
        doc.link(attr, mid);
    }
    for (k, shape) in shapes.iter().enumerate() {
        if k == 0 {
            geometry(doc, sess, *shape, mid);
        } else {
            // One geometry to a model: a second shape rides on a model of its own.
            let extra = doc.id();
            let shape_name = leaf(&sess.graph, *shape);
            doc.add("Model", Rec::new("Model", vec![Prop::L(extra), named(&shape_name, "Model"), s("Mesh")]).with(vec![Rec::new("Version", vec![Prop::I(232)])]));
            doc.link(extra, mid);
            geometry(doc, sess, *shape, extra);
        }
    }
    for c in children {
        // What leads to a mesh, the joints skins use, and a skeleton's own joints: a chain asked for goes out whole.
        let bone = sess.graph.node(c).is_some_and(|n| n.ty().is_a("joint"));
        if is_transform(&sess.graph, c) && (has_mesh_below(&sess.graph, c) || doc.needed.contains(&c) || bone) {
            model(doc, sess, c, mid, false);
        }
    }
}

/// The skins, once every joint has its model: a cluster a joint with the points it moves, and the bind pose.
fn skins(doc: &mut Doc, g: &Graph) {
    let mut pose: Vec<(i64, DMat4)> = Vec::new();
    for (geo, mesh_model, sk, shape) in doc.bound.clone() {
        let Some(bind) = crate::skin::bind_of(g, sk) else { continue };
        let influences = crate::skin::influences(g, sk);
        if influences.iter().any(|(_, j)| !doc.model_of.contains_key(j)) {
            doc.notes.push(format!("{}: a joint of it is not in what was exported, so its mesh went out without a skin", g.path(sk)));
            continue;
        }
        let skin = doc.id();
        doc.add(
            "Deformer",
            Rec::new("Deformer", vec![Prop::L(skin), named(&leaf(g, sk), "Deformer"), s("Skin")])
                .with(vec![Rec::new("Version", vec![Prop::I(101)]), Rec::new("Link_DeformAcuracy", vec![Prop::D(50.0)])]),
        );
        doc.link(skin, geo);
        let geom = doc.k * bind.geom;
        if !pose.iter().any(|p| p.0 == mesh_model) {
            pose.push((mesh_model, geom));
        }
        for (element, joint) in influences {
            let (mut points, mut weights) = (Vec::new(), Vec::new());
            for (p, w) in bind.weights.iter().enumerate() {
                if let Some(x) = w.iter().find(|x| x.0 == element as u32) {
                    points.push(p as i32);
                    weights.push(x.1);
                }
            }
            let pre = bind.bind_pre.get(element as usize).copied().unwrap_or(DMat4::IDENTITY);
            // Where the joint stood at the bind, and the mesh seen from there.
            let link = doc.k * pre.inverse();
            let joint_model = doc.model_of[&joint];
            let cluster = doc.id();
            let mut rec = Rec::new("Deformer", vec![Prop::L(cluster), named(&leaf(g, joint), "SubDeformer"), s("Cluster")])
                .with(vec![Rec::new("Version", vec![Prop::I(100)]), Rec::new("UserData", vec![s(""), s("")])]);
            // A joint that moves nothing still has its cluster, so the skeleton reads whole.
            if !points.is_empty() {
                rec.push(Rec::new("Indexes", vec![Prop::Ints(points)]));
                rec.push(Rec::new("Weights", vec![Prop::Doubles(weights)]));
            }
            // The mesh's and the joint's world matrices at the bind, as the format's own library writes them.
            rec.push(matrix("Transform", &geom));
            rec.push(matrix("TransformLink", &link));
            doc.add("Deformer", rec);
            doc.link(cluster, skin);
            doc.link(joint_model, cluster);
            if !pose.iter().any(|p| p.0 == joint_model) {
                pose.push((joint_model, link));
            }
        }
        let _ = shape;
    }
    if pose.is_empty() {
        return;
    }
    let id = doc.id();
    let mut rec = Rec::new("Pose", vec![Prop::L(id), named("bindPose1", "Pose"), s("BindPose")]).with(vec![
        Rec::new("Type", vec![s("BindPose")]),
        Rec::new("Version", vec![Prop::I(100)]),
        Rec::new("NbPoseNodes", vec![Prop::I(pose.len() as i32)]),
    ]);
    for (node, m) in pose {
        rec.push(Rec::new("PoseNode", vec![]).with(vec![Rec::new("Node", vec![Prop::L(node)]), matrix("Matrix", &m)]));
    }
    doc.add("Pose", rec);
}

/// The records of a file for these objects, with what was written and what was left out.
pub fn build(sess: &mut Session, objects: &[NodeId]) -> Result<(Vec<Rec>, Value), String> {
    sess.graph.evaluate();
    let mut tops: Vec<NodeId> = Vec::new();
    for &id in objects {
        let t = if is_transform(&sess.graph, id) { id } else { crate::dag::transform_of(&sess.graph, id) };
        if is_transform(&sess.graph, t) && !tops.contains(&t) {
            tops.push(t);
        }
    }
    tops.retain(|&t| has_mesh_below(&sess.graph, t) || sess.graph.node(t).is_some_and(|n| n.ty().is_a("joint")));
    let all = tops.clone();
    tops.retain(|&t| {
        let mut p = sess.graph.parent(t);
        while p != NO_NODE {
            if all.contains(&p) {
                return false;
            }
            p = sess.graph.parent(p);
        }
        true
    });
    // The file's axes and unit, by the FBXExport settings: the top nodes are turned and scaled into them.
    let scene_z = crate::world::up_z();
    let up_z = match crate::fbx_options::word("FBXExportUpAxis").as_str() {
        "z" => true,
        "y" => false,
        _ => scene_z,
    };
    // Automatic: the scene's working unit.
    let unit_cm = if crate::fbx_options::on("FBXExportAutoUnits") { aztek_ops::units::cm_per_unit() } else { crate::fbx_options::unit_cm(&crate::fbx_options::word("FBXExportConvertUnitString")).unwrap_or(1.0) };
    let scale = crate::fbx_options::number("FBXExportScaleFactor");
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let turn = crate::fbx_import::axis_change(scene_z, up_z);
    let k = DMat4::from_mat3(turn * (scale / unit_cm));
    let mut doc = Doc {
        needed: skeletons(&sess.graph, &tops),
        k,
        skins_on: crate::fbx_options::on("FBXExportSkins"),
        embed: crate::fbx_options::on("FBXExportEmbeddedTextures"),
        ..Default::default()
    };
    let mut skeleton_tops: Vec<NodeId> = doc.needed.iter().copied().filter(|&j| sess.graph.parent(j) == NO_NODE).collect();
    skeleton_tops.sort_unstable();
    for t in skeleton_tops {
        if !tops.contains(&t) {
            tops.push(t);
        }
    }
    for t in tops {
        if !doc.model_of.contains_key(&t) {
            model(&mut doc, sess, t, 0, true);
        }
    }
    if doc.meshes == 0 {
        return Err("no meshes to export".into());
    }
    skins(&mut doc, &sess.graph);

    let int = |name: &str, v: i32| p70(name, "int", "Integer", "", vec![Prop::I(v)]);
    let settings = Rec::new("GlobalSettings", vec![]).with(vec![
        Rec::new("Version", vec![Prop::I(1000)]),
        Rec::new("Properties70", vec![]).with(vec![
            // The scene's own axes: Y up with Z to the front, or Z up with Y to the back.
            int("UpAxis", if up_z { 2 } else { 1 }),
            int("UpAxisSign", 1),
            int("FrontAxis", if up_z { 1 } else { 2 }),
            int("FrontAxisSign", if up_z { -1 } else { 1 }),
            int("CoordAxis", 0),
            int("CoordAxisSign", 1),
            int("OriginalUpAxis", if up_z { 2 } else { 1 }),
            int("OriginalUpAxisSign", 1),
            // Centimetres, or the unit asked for.
            p70("UnitScaleFactor", "double", "Number", "", vec![Prop::D(unit_cm)]),
            p70("OriginalUnitScaleFactor", "double", "Number", "", vec![Prop::D(unit_cm)]),
        ]),
    ]);
    let mut kinds: Vec<(&str, i32)> = doc.counts.iter().map(|(k, v)| (*k, *v)).collect();
    kinds.sort();
    let total: i32 = 1 + kinds.iter().map(|k| k.1).sum::<i32>();
    let mut definitions = vec![
        Rec::new("Version", vec![Prop::I(100)]),
        Rec::new("Count", vec![Prop::I(total)]),
        Rec::new("ObjectType", vec![s("GlobalSettings")]).with(vec![Rec::new("Count", vec![Prop::I(1)])]),
    ];
    for (kind, count) in &kinds {
        definitions.push(Rec::new("ObjectType", vec![s(kind)]).with(vec![Rec::new("Count", vec![Prop::I(*count)])]));
    }
    let connections: Vec<Rec> = doc
        .links
        .iter()
        .map(|(kind, child, parent, property)| {
            let mut props = vec![s(kind), Prop::L(*child), Prop::L(*parent)];
            if let Some(p) = property {
                props.push(s(p));
            }
            Rec::new("C", props)
        })
        .collect();
    let creator = format!("Aztek {}", env!("CARGO_PKG_VERSION"));
    let top = vec![
        Rec::new("FBXHeaderExtension", vec![]).with(vec![
            Rec::new("FBXHeaderVersion", vec![Prop::I(1003)]),
            Rec::new("FBXVersion", vec![Prop::I(VERSION as i32)]),
            Rec::new("EncryptionType", vec![Prop::I(0)]),
            Rec::new("Creator", vec![s(&creator)]),
        ]),
        Rec::new("FileId", vec![Prop::Raw(vec![0x28, 0xb3, 0x2a, 0xeb, 0xb6, 0x24, 0xcc, 0xc2, 0xbf, 0xc8, 0xb0, 0x2a, 0xa9, 0x2b, 0xfc, 0xf1])]),
        Rec::new("CreationTime", vec![s("1970-01-01 10:00:00:000")]),
        Rec::new("Creator", vec![s(&creator)]),
        settings,
        Rec::new("Documents", vec![]).with(vec![
            Rec::new("Count", vec![Prop::I(1)]),
            Rec::new("Document", vec![Prop::L(999_999), s("Scene"), s("Scene")]).with(vec![Rec::new("RootNode", vec![Prop::L(0)])]),
        ]),
        Rec::new("Definitions", vec![]).with(definitions),
        Rec::new("Objects", vec![]).with(std::mem::take(&mut doc.objects)),
        Rec::new("Connections", vec![]).with(connections),
        Rec::new("Takes", vec![]).with(vec![Rec::new("Current", vec![s("")])]),
    ];
    let count = |k: &str| doc.counts.get(k).copied().unwrap_or(0);
    Ok((top, json!({"models": count("Model"), "meshes": doc.meshes, "materials": count("Material"), "textures": count("Texture"), "skins": doc.bound.len(), "notes": doc.notes})))
}

pub fn export(sess: &mut Session, path: &str, objects: &[NodeId]) -> CmdResult {
    if crate::fbx_options::on("FBXExportInAscii") {
        fail!("exportFbx: ASCII FBX files are not written: turn FBXExportInAscii off");
    }
    let (top, mut report) = match build(sess, objects) {
        Ok(x) => x,
        Err(e) => fail!("exportFbx: {e}"),
    };
    if std::fs::write(path, encode(&top)).is_err() {
        fail!("exportFbx: can't write {path}");
    }
    report["path"] = json!(path);
    Ok(report)
}

fn objects_for(sess: &Session, names: &[String], all: bool, cmd: &str) -> Result<Vec<NodeId>, String> {
    let g = &sess.graph;
    if all {
        let mut every = g.nodes();
        every.sort_unstable();
        return Ok(every.into_iter().filter(|&id| is_transform(g, id) && g.parent(id) == NO_NODE).collect());
    }
    let mut ids = Vec::new();
    for n in names {
        match g.find(n) {
            Ok(Some(id)) => ids.push(id),
            _ => return Err(format!("{cmd}: no object named {n}")),
        }
    }
    if ids.is_empty() {
        ids = g.selection().to_vec();
    }
    if ids.is_empty() {
        return Err(format!("{cmd}: nothing selected"));
    }
    Ok(ids)
}

/// The commands the FBX plug-in brings: `exportFbx`, and `FBXExport` as scripts written for the reference call it.
pub const COMMANDS: &[&str] = &["exportFbx", "FBXExport"];

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "exportFbx",
            "Export to FBX (binary, 7.4): the objects named, or the selection, with what is under them. Hierarchy, meshes as \
             polygons with normals, UV sets and materials, colour textures by path, and skins (joints, clusters, bind pose), \
             in centimetres with the scene's up axis. Animation, blend shapes, cameras and lights are not written. -all \
             exports every top-level object.",
            |s, a| {
                let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("exportFbx: path required") };
                let names: Vec<String> = (1..a.positional.len()).filter_map(|k| a.pos_str(k).map(str::to_string)).collect();
                let ids = match objects_for(s, &names, a.bool("all"), "exportFbx") {
                    Ok(i) => i,
                    Err(e) => fail!("{e}"),
                };
                export(s, &path, &ids)
            },
        )
        .positional("path objects...")
        .flag(FlagDef::new("all", "all", ArgType::Bool, Value::Null, "Every top-level object, not the selection."))
        .undoable(false),
    );
    c.register(
        CommandDef::<Session>::new("FBXExport", "Export to FBX as scripts written for the reference ask: FBXExport -f \"path\" writes everything, with -s the selection.", |s, a| {
            let path = a.string("file", "");
            if path.is_empty() {
                fail!("FBXExport: give -f and a path");
            }
            let ids = match objects_for(s, &[], !a.bool("selected"), "FBXExport") {
                Ok(i) => i,
                Err(e) => fail!("{e}"),
            };
            export(s, &path, &ids)
        })
        .flag(FlagDef::new("file", "f", ArgType::String, Value::Null, "The file to write."))
        .flag(FlagDef::new("selected", "s", ArgType::Bool, Value::Null, "Only the selection."))
        .undoable(false),
    );
}
