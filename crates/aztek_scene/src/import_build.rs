//! What the importers share: a file's nodes, meshes and materials in one plain form, and the scene made from them.
//! An importer reads its format into this, already in the scene's units and axes; `build` makes the nodes.

use std::collections::HashMap;
use std::sync::Arc;

use aztek_geo::mesh::Mesh;
use aztek_ops::{CmdResult, CommandError};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use crate::session::Session;

#[derive(Default)]
pub struct ImpMaterial {
    pub name: String,
    pub color: [f64; 3],
    /// A highlight: a blinn where a lambert would do otherwise.
    pub shiny: bool,
    /// Metalness and roughness, where the file's material has them: a standard surface.
    pub pbr: Option<(f64, f64)>,
    pub transparency: f64,
    /// The node type, where the file says (lambert, phong): it wins over `shiny` and `pbr`.
    pub ty: Option<String>,
    /// Further attributes set as they are: (attribute, values).
    pub attrs: Vec<(String, Vec<f64>)>,
    /// Textures into its attributes.
    pub textures: Vec<ImpTexture>,
}

/// How a texture feeds its attribute.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TexUse {
    /// outColor into a colour.
    Color,
    /// outAlpha into a number.
    Alpha,
    /// outTransparency into transparency.
    Transparency,
    /// A tangent space normal map through a bump2d into normalCamera.
    Normal,
    /// A height through a bump2d, with its depth.
    Bump(f64),
}

#[derive(Clone, Debug)]
pub struct ImpTexture {
    /// The texture's own name in the file, which the file node takes; one texture worn twice is one file node.
    pub name: String,
    /// Which of the file's textures: shared textures share a file node.
    pub key: i64,
    pub path: String,
    pub attr: String,
    pub usage: TexUse,
    pub repeat: [f64; 2],
    pub offset: [f64; 2],
    /// Degrees.
    pub rotate: f64,
    pub wrap: [bool; 2],
}

/// Which edges are hard.
pub enum Hardness {
    /// Where the file's normals at a point differ across the edge.
    FromNormals,
    /// By the edge that starts at each corner, in the order of `indices`: true is hard.
    ByCorner(Vec<bool>),
    /// Smoothing groups a face: faces that share none meet at a hard edge.
    Groups(Vec<i64>),
}

#[derive(Default)]
pub struct ImpMesh {
    pub points: Vec<DVec3>,
    /// Corners a face, and each face's points one after another.
    pub sizes: Vec<u32>,
    pub indices: Vec<u32>,
    /// UV sets by name, a value a corner in the order of `indices`.
    pub uvs: Vec<(String, Vec<[f32; 2]>)>,
    /// The material of each face, by its place in the materials; usize::MAX for none.
    pub face_material: Vec<usize>,
    /// Colour sets by name, a value a corner.
    pub colors: Vec<(String, Vec<[f32; 4]>)>,
    /// The file's normals, a value a corner, and whether they are kept (locked) or only tell hard edges from soft.
    pub normals: Option<Vec<[f32; 3]>>,
    pub keep_normals: bool,
    pub hardness: Option<Hardness>,
}

/// A node's channels, as the reference keeps them; angles in radians.
#[derive(Clone, Debug, Default)]
pub struct Parts {
    pub translate: DVec3,
    pub rotate: DVec3,
    /// The reference's rotate order, 0 xyz to 5 zyx.
    pub rotate_order: i64,
    pub joint_orient: DVec3,
    pub rotate_axis: DVec3,
    pub rotate_pivot: DVec3,
    pub rotate_pivot_translate: DVec3,
    pub scale_pivot: DVec3,
    pub scale_pivot_translate: DVec3,
    pub scale: DVec3,
    pub segment_scale_compensate: Option<bool>,
}

pub struct ImpNode {
    pub name: String,
    pub parent: Option<usize>,
    pub joint: bool,
    /// Its own matrix under its parent.
    pub matrix: DMat4,
    pub mesh: Option<ImpMesh>,
    /// Its channels one by one, where the file has them; otherwise the matrix is taken apart.
    pub parts: Option<Parts>,
    /// A node already in the scene this one is: nothing is made or changed for it.
    pub existing: Option<NodeId>,
}

/// A skin: the mesh node, its influences (node indices) with their bind pre-matrices, the mesh's world matrix at bind,
/// and each point's (influence, weight).
pub struct ImpSkin {
    pub mesh: usize,
    pub influences: Vec<usize>,
    pub bind_pre: Vec<DMat4>,
    pub geom: DMat4,
    pub weights: Vec<Vec<(u32, f64)>>,
    pub normalize: bool,
}

/// What some files have besides nodes and materials.
#[derive(Default)]
pub struct Extras {
    pub skins: Vec<ImpSkin>,
    /// The bind pose: nodes with their world matrices, in the scene's space.
    pub bind_pose: Vec<(usize, DMat4)>,
}

/// A name the scene takes: letters, digits and underscores, not starting with a digit.
pub fn clean_name(name: &str, fallback: &str) -> String {
    let mut out: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.is_empty() {
        out = fallback.to_string();
    }
    if out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    if Graph::valid_name(&out) {
        out
    } else {
        fallback.to_string()
    }
}

/// The mesh, and for each face asked for the face made (None for one the mesh refused: fewer than three points, or a
/// point named twice).
fn mesh_of(m: &ImpMesh) -> (Mesh, Vec<Option<u32>>) {
    let (mut mesh, made, corner_of) = mesh_faces(m);
    finish_mesh(&mut mesh, m, &corner_of);
    (mesh, made)
}

fn mesh_faces(m: &ImpMesh) -> (Mesh, Vec<Option<u32>>, Vec<Option<u32>>) {
    let mut mesh = Mesh::new();
    for p in &m.points {
        mesh.add_point(*p);
    }
    let mut made = Vec::with_capacity(m.sizes.len());
    let mut corner_of: Vec<Option<u32>> = vec![None; m.indices.len()];
    let mut at = 0usize;
    for &n in &m.sizes {
        let n = n as usize;
        let pts = &m.indices[at..(at + n).min(m.indices.len())];
        let f = if pts.len() == n { mesh.add_face(pts) } else { u32::MAX };
        if f == u32::MAX {
            made.push(None);
        } else {
            for (k, c) in mesh.face_corners(f).into_iter().enumerate() {
                corner_of[at + k] = Some(c);
            }
            made.push(Some(f));
        }
        at += n;
    }
    (mesh, made, corner_of)
}

/// The file's per-corner values onto the mesh's corners, and which edges are hard.
fn finish_mesh(mesh: &mut Mesh, m: &ImpMesh, corner_of: &[Option<u32>]) {
    let mut names = Vec::new();
    for (slot, (name, values)) in m.uvs.iter().enumerate() {
        let layer = format!("uv{slot}");
        let out = mesh.corner_attrs_mut().get_or_add::<[f32; 2]>(&layer);
        for (k, c) in corner_of.iter().enumerate() {
            if let (Some(c), Some(v)) = (c, values.get(k)) {
                if let Some(o) = out.get_mut(*c as usize) {
                    *o = *v;
                }
            }
        }
        names.push(if name.is_empty() { if slot == 0 { "map1".to_string() } else { format!("uvSet{slot}") } } else { clean_name(name, "map1") });
    }
    if !names.is_empty() {
        // The seams are where neighbouring faces disagree about a point's UV.
        crate::uv_sets::name_layers(mesh, &names, &names[0]);
    }
    for (slot, (name, values)) in m.colors.iter().enumerate() {
        let set = if name.is_empty() { format!("colorSet{}", slot + 1) } else { clean_name(name, "colorSet1") };
        let set = aztek_geo::colors::unique_name(mesh, &set);
        let out = aztek_geo::colors::add_white(mesh, &set);
        for (k, c) in corner_of.iter().enumerate() {
            if let (Some(c), Some(v)) = (c, values.get(k)) {
                if let Some(o) = out.get_mut(*c as usize) {
                    *o = *v;
                }
            }
        }
    }
    let corners = mesh.corner_capacity() as usize;
    let by_corner = |values: &[[f32; 3]]| -> Vec<[f32; 3]> {
        let mut out = vec![[0.0f32; 3]; corners];
        for (k, c) in corner_of.iter().enumerate() {
            if let (Some(c), Some(v)) = (c, values.get(k)) {
                out[*c as usize] = *v;
            }
        }
        out
    };
    match &m.hardness {
        Some(Hardness::FromNormals) => {
            if let Some(n) = &m.normals {
                let n = by_corner(n);
                hard_from_normals(mesh, &n);
            }
        }
        Some(Hardness::ByCorner(hard)) => {
            let mut set: Vec<(u32, bool)> = Vec::new();
            for (k, c) in corner_of.iter().enumerate() {
                if let (Some(c), Some(h)) = (c, hard.get(k)) {
                    set.push((mesh.corner_edge(*c), *h));
                }
            }
            let flags = mesh.edge_attrs_mut().get_or_add::<bool>(aztek_geo::attribute::names::HARD);
            for (e, h) in set {
                if let Some(x) = flags.get_mut(e as usize) {
                    *x = h;
                }
            }
        }
        Some(Hardness::Groups(groups)) => {
            let face_group: Vec<i64> = {
                let mut v = vec![0i64; mesh.face_capacity() as usize];
                let mut at = 0usize;
                for (f, &n) in m.sizes.iter().enumerate() {
                    if let Some(Some(c)) = corner_of.get(at) {
                        v[mesh.corner_face(*c) as usize] = groups.get(f).copied().unwrap_or(0);
                    }
                    at += n as usize;
                }
                v
            };
            let mut set = Vec::new();
            for e in 0..mesh.edge_capacity() {
                if !mesh.edge_valid(e) || !mesh.is_manifold_edge(e) {
                    continue;
                }
                let c = mesh.edge_corner(e);
                let (a, b) = (face_group[mesh.corner_face(c) as usize], face_group[mesh.corner_face(mesh.corner_radial(c)) as usize]);
                set.push((e, a & b == 0));
            }
            let flags = mesh.edge_attrs_mut().get_or_add::<bool>(aztek_geo::attribute::names::HARD);
            for (e, h) in set {
                flags[e as usize] = h;
            }
        }
        None => {}
    }
    if m.keep_normals {
        if let Some(n) = &m.normals {
            let n = by_corner(n);
            let out = mesh.corner_attrs_mut().get_or_add::<[f32; 3]>(aztek_geo::attribute::names::NORMAL);
            out.copy_from_slice(&n[..out.len().min(n.len())]);
        }
    }
}

/// Hard where the two faces of an edge carry different normals at either of its points.
fn hard_from_normals(mesh: &mut Mesh, n: &[[f32; 3]]) {
    let differs = |a: [f32; 3], b: [f32; 3]| {
        let (a, b) = (glam::Vec3::from_array(a), glam::Vec3::from_array(b));
        a.normalize_or_zero().dot(b.normalize_or_zero()) < 0.9999
    };
    let mut set = Vec::new();
    for e in 0..mesh.edge_capacity() {
        if !mesh.edge_valid(e) {
            continue;
        }
        if !mesh.is_manifold_edge(e) {
            set.push((e, true));
            continue;
        }
        let c = mesh.edge_corner(e);
        let r = mesh.corner_radial(c);
        let mut hard = false;
        for a in [c, mesh.corner_next(c)] {
            let p = mesh.corner_point(a);
            let b = if mesh.corner_point(r) == p { r } else { mesh.corner_next(r) };
            if mesh.corner_point(b) == p && differs(n[a as usize], n[b as usize]) {
                hard = true;
            }
        }
        set.push((e, hard));
    }
    let flags = mesh.edge_attrs_mut().get_or_add::<bool>(aztek_geo::attribute::names::HARD);
    for (e, h) in set {
        flags[e as usize] = h;
    }
}

fn run(s: &mut Session, name: &str, call: Value) -> Result<Value, CommandError> {
    s.run(name, call)
}

/// Makes the nodes, meshes and materials, selects what was made at the top, and returns those nodes' names.
pub fn build(s: &mut Session, nodes: Vec<ImpNode>, materials: Vec<ImpMaterial>) -> CmdResult {
    build_all(s, nodes, materials, Extras::default())
}

fn v3(v: DVec3) -> [f64; 3] {
    let tidy = |x: f64| if x.abs() < 1e-10 { 0.0 } else { x };
    [tidy(v.x), tidy(v.y), tidy(v.z)]
}

fn set3(s: &mut Session, path: &str, attr: &str, v: DVec3, rest: f64) -> Result<(), CommandError> {
    if (v - DVec3::splat(rest)).length() > 1e-10 {
        let [x, y, z] = v3(v);
        run(s, "setAttr", json!({"args": [format!("{path}.{attr}"), x, y, z], "type": "double3"}))?;
    }
    Ok(())
}

/// A node's channels: its parts as given, or its matrix taken apart.
fn place_node(s: &mut Session, path: &str, n: &ImpNode) -> Result<(), CommandError> {
    // Commands run here take angles as they are stored, in radians.
    match &n.parts {
        Some(p) => {
            if p.rotate_order != 0 {
                run(s, "setAttr", json!({"args": [format!("{path}.rotateOrder"), p.rotate_order]}))?;
            }
            set3(s, path, "translate", p.translate, 0.0)?;
            set3(s, path, "rotate", p.rotate, 0.0)?;
            set3(s, path, "scale", p.scale, 1.0)?;
            set3(s, path, "rotateAxis", p.rotate_axis, 0.0)?;
            set3(s, path, "rotatePivot", p.rotate_pivot, 0.0)?;
            set3(s, path, "rotatePivotTranslate", p.rotate_pivot_translate, 0.0)?;
            set3(s, path, "scalePivot", p.scale_pivot, 0.0)?;
            set3(s, path, "scalePivotTranslate", p.scale_pivot_translate, 0.0)?;
            if n.joint {
                set3(s, path, "jointOrient", p.joint_orient, 0.0)?;
                if let Some(c) = p.segment_scale_compensate {
                    run(s, "setAttr", json!({"args": [format!("{path}.segmentScaleCompensate"), c]}))?;
                }
            }
        }
        None => {
            let (scale, rotation, translation) = n.matrix.to_scale_rotation_translation();
            let e = crate::transform::matrix_to_euler(&DMat3::from_quat(rotation), crate::transform::RotateOrder::Xyz);
            set3(s, path, "translate", translation, 0.0)?;
            set3(s, path, "rotate", e, 0.0)?;
            set3(s, path, "scale", scale, 1.0)?;
        }
    }
    Ok(())
}

fn connect(s: &mut Session, from: String, to: String) -> Result<(), CommandError> {
    run(s, "connectAttr", json!({"args": [from, to], "force": true})).map(|_| ())
}

/// A file node for a texture, with its place2dTexture, as the reference makes them.
fn file_node(s: &mut Session, t: &ImpTexture) -> Result<String, CommandError> {
    let file = run(s, "shadingNode", json!({"args": ["file"], "asTexture": true, "name": clean_name(&t.name, "file1")}))?.as_str().unwrap_or_default().to_string();
    let place = run(s, "shadingNode", json!({"args": ["place2dTexture"], "asUtility": true}))?.as_str().unwrap_or_default().to_string();
    run(s, "defaultNavigation", json!({"connectToExisting": true, "source": place, "destination": file}))?;
    run(s, "setAttr", json!({"args": [format!("{file}.fileTextureName"), t.path], "type": "string"}))?;
    if t.repeat != [1.0, 1.0] {
        run(s, "setAttr", json!({"args": [format!("{place}.repeatUV"), t.repeat[0], t.repeat[1]], "type": "double2"}))?;
    }
    if t.offset != [0.0, 0.0] {
        run(s, "setAttr", json!({"args": [format!("{place}.offset"), t.offset[0], t.offset[1]], "type": "double2"}))?;
    }
    if t.rotate != 0.0 {
        run(s, "setAttr", json!({"args": [format!("{place}.rotateUV"), t.rotate.to_radians()]}))?;
    }
    for (k, attr) in ["wrapU", "wrapV"].iter().enumerate() {
        if !t.wrap[k] {
            run(s, "setAttr", json!({"args": [format!("{place}.{attr}"), false]}))?;
        }
    }
    Ok(file)
}

/// A material and what feeds it.
fn make_material(s: &mut Session, im: &ImpMaterial, files: &mut HashMap<i64, String>) -> Result<String, CommandError> {
    let ty = im.ty.clone().unwrap_or_else(|| (if im.pbr.is_some() { "standardSurface" } else if im.shiny { "blinn" } else { "lambert" }).to_string());
    let made = run(s, "shadingNode", json!({"args": [ty], "asShader": true, "name": clean_name(&im.name, "material1")}))?;
    let name = made.as_str().unwrap_or_default().to_string();
    let colour = if im.pbr.is_some() { "baseColor" } else { "color" };
    run(s, "setAttr", json!({"args": [format!("{name}.{colour}"), im.color[0], im.color[1], im.color[2]], "type": "double3"}))?;
    if let Some((metal, rough)) = im.pbr {
        run(s, "setAttr", json!({"args": [format!("{name}.metalness"), metal]}))?;
        run(s, "setAttr", json!({"args": [format!("{name}.specularRoughness"), rough]}))?;
        if im.transparency > 0.0 {
            let o = 1.0 - im.transparency;
            run(s, "setAttr", json!({"args": [format!("{name}.opacity"), o, o, o], "type": "double3"}))?;
        }
    } else if im.transparency > 0.0 {
        let t = im.transparency;
        run(s, "setAttr", json!({"args": [format!("{name}.transparency"), t, t, t], "type": "double3"}))?;
    }
    for (attr, v) in &im.attrs {
        let call = match v.len() {
            3 => json!({"args": [format!("{name}.{attr}"), v[0], v[1], v[2]], "type": "double3"}),
            _ => json!({"args": [format!("{name}.{attr}"), v.first().copied().unwrap_or(0.0)]}),
        };
        run(s, "setAttr", call)?;
    }
    for t in &im.textures {
        let file = match files.get(&t.key) {
            Some(f) => f.clone(),
            None => {
                let f = file_node(s, t)?;
                files.insert(t.key, f.clone());
                f
            }
        };
        match t.usage {
            TexUse::Color => connect(s, format!("{file}.outColor"), format!("{name}.{}", t.attr))?,
            TexUse::Alpha => connect(s, format!("{file}.outAlpha"), format!("{name}.{}", t.attr))?,
            TexUse::Transparency => connect(s, format!("{file}.outTransparency"), format!("{name}.{}", t.attr))?,
            TexUse::Normal | TexUse::Bump(_) => {
                let bump = run(s, "shadingNode", json!({"args": ["bump2d"], "asUtility": true}))?.as_str().unwrap_or_default().to_string();
                match t.usage {
                    // Tangent Space Normals.
                    TexUse::Normal => run(s, "setAttr", json!({"args": [format!("{bump}.bumpInterp"), 1]}))?,
                    TexUse::Bump(depth) => run(s, "setAttr", json!({"args": [format!("{bump}.bumpDepth"), depth]}))?,
                    _ => Value::Null,
                };
                connect(s, format!("{file}.outAlpha"), format!("{bump}.bumpValue"))?;
                connect(s, format!("{bump}.outNormal"), format!("{name}.normalCamera"))?;
            }
        }
    }
    Ok(name)
}

/// Makes the nodes, meshes, materials, skins and bind pose; selects what was made at the top and returns those nodes' names.
pub fn build_all(s: &mut Session, nodes: Vec<ImpNode>, materials: Vec<ImpMaterial>, extras: Extras) -> CmdResult {
    let mut ids: Vec<NodeId> = vec![NO_NODE; nodes.len()];
    let mut faces_made: Vec<Vec<Option<u32>>> = (0..nodes.len()).map(|_| Vec::new()).collect();
    let transform = crate::node::types::find("transform").expect("transform");
    let joint = crate::node::types::find("joint").expect("joint");
    // Parents before children, whatever order the file had them in.
    let mut order: Vec<usize> = Vec::with_capacity(nodes.len());
    let mut placed = vec![false; nodes.len()];
    while order.len() < nodes.len() {
        let before = order.len();
        for (k, n) in nodes.iter().enumerate() {
            if !placed[k] && n.parent.is_none_or(|p| p >= nodes.len() || placed[p]) {
                placed[k] = true;
                order.push(k);
            }
        }
        if order.len() == before {
            return Err(CommandError::new("import: the file's hierarchy loops"));
        }
    }
    for &k in &order {
        let n = &nodes[k];
        if let Some(id) = n.existing {
            ids[k] = id;
            continue;
        }
        let parent = n.parent.filter(|p| *p < nodes.len()).map_or(NO_NODE, |p| ids[p]);
        let wanted = clean_name(&n.name, if n.joint { "joint1" } else if n.mesh.is_some() { "polySurface1" } else { "transform1" });
        let g = &mut s.graph;
        let id = match &n.mesh {
            Some(m) => {
                // Names as the file has them: unique among their siblings, as the reference keeps a file's names.
                let xf = g.create_node(&transform, &wanted, parent, false)?;
                let shape_type = crate::node::types::find("mesh").expect("mesh");
                let sh_name = crate::dag::shape_name(g.node(xf).unwrap().name());
                let shape = g.create_node(&shape_type, &sh_name, xf, false)?;
                let (mesh, made) = mesh_of(m);
                faces_made[k] = made;
                g.set_value(PlugRef::new(shape, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(mesh))))?;
                xf
            }
            None => g.create_node(if n.joint { &joint } else { &transform }, &wanted, parent, false)?,
        };
        ids[k] = id;
        let path = s.graph.path(id);
        place_node(s, &path, n)?;
    }
    // Materials, made when a face first wears one.
    let mut material_names: Vec<Option<String>> = vec![None; materials.len()];
    let mut files: HashMap<i64, String> = HashMap::new();
    for &k in &order {
        let Some(m) = &nodes[k].mesh else { continue };
        if nodes[k].existing.is_some() {
            continue;
        }
        let path = s.graph.path(ids[k]);
        let mut by_material: Vec<(usize, Vec<u32>)> = Vec::new();
        for (f, made) in faces_made[k].iter().enumerate() {
            let (Some(face), Some(&mat)) = (made, m.face_material.get(f)) else { continue };
            if mat >= materials.len() {
                continue;
            }
            match by_material.iter_mut().find(|x| x.0 == mat) {
                Some(x) => x.1.push(*face),
                None => by_material.push((mat, vec![*face])),
            }
        }
        let whole = by_material.len() == 1 && by_material[0].1.len() == faces_made[k].iter().flatten().count();
        for (mat, faces) in by_material {
            let name = match &material_names[mat] {
                Some(n) => n.clone(),
                None => {
                    let name = make_material(s, &materials[mat], &mut files)?;
                    material_names[mat] = Some(name.clone());
                    name
                }
            };
            // The object, or runs of its faces.
            let mut what: Vec<String> = Vec::new();
            if whole {
                what.push(path.clone());
            } else {
                let mut run_start = faces[0];
                let mut last = faces[0];
                for &f in faces.iter().skip(1).chain(std::iter::once(&u32::MAX)) {
                    if f != last.wrapping_add(1) {
                        what.push(if run_start == last { format!("{path}.f[{last}]") } else { format!("{path}.f[{run_start}:{last}]") });
                        run_start = f;
                    }
                    last = f;
                }
            }
            run(s, "select", json!({"args": what}))?;
            run(s, "hyperShade", json!({"assign": name}))?;
        }
    }
    // The bind pose: each member's channels at bind, worked out from the world matrices.
    let mut pose: Vec<(NodeId, Value)> = Vec::new();
    if !extras.bind_pose.is_empty() {
        s.graph.evaluate();
        let world_at: HashMap<usize, DMat4> = extras.bind_pose.iter().copied().collect();
        for &(k, w) in &extras.bind_pose {
            if k >= nodes.len() || !nodes[k].joint {
                continue;
            }
            let parent_world = match nodes[k].parent {
                Some(p) if p < nodes.len() => world_at.get(&p).copied().unwrap_or_else(|| crate::rig::world(&mut s.graph, ids[p])),
                _ => DMat4::IDENTITY,
            };
            let local = parent_world.inverse() * w;
            let (sc, rot, t) = local.to_scale_rotation_translation();
            let parts = nodes[k].parts.clone().unwrap_or_default();
            let jo = crate::transform::euler_to_matrix(parts.joint_orient, crate::transform::RotateOrder::Xyz);
            let ra = crate::transform::euler_to_matrix(parts.rotate_axis, crate::transform::RotateOrder::Xyz);
            let r = jo.inverse() * DMat4::from_quat(rot) * ra.inverse();
            let e = crate::transform::matrix_to_euler(&DMat3::from_mat4(r), order_of(parts.rotate_order));
            pose.push((ids[k], json!({"t": v3(t), "r": v3(e), "s": v3(sc)})));
        }
    }
    for sk in extras.skins {
        if sk.mesh >= nodes.len() || nodes[sk.mesh].existing.is_some() {
            continue;
        }
        let joints: Vec<NodeId> = sk.influences.iter().filter_map(|&i| ids.get(i).copied()).collect();
        if joints.len() != sk.influences.len() || joints.is_empty() {
            continue;
        }
        crate::skin::bind_imported(&mut s.graph, ids[sk.mesh], &joints, sk.bind_pre, sk.geom, sk.weights, sk.normalize, &pose)?;
    }
    let tops: Vec<NodeId> = order.iter().filter(|&&k| nodes[k].parent.is_none_or(|p| p >= nodes.len()) && nodes[k].existing.is_none()).map(|&k| ids[k]).collect();
    let names: Vec<String> = tops.iter().map(|&id| s.graph.path(id)).collect();
    if names.is_empty() {
        run(s, "select", json!({"clear": true}))?;
    } else {
        run(s, "select", json!({"args": names}))?;
    }
    Ok(json!(names))
}

fn order_of(k: i64) -> crate::transform::RotateOrder {
    use crate::transform::RotateOrder::*;
    match k {
        1 => Yzx,
        2 => Zxy,
        3 => Xzy,
        4 => Yxz,
        5 => Zyx,
        _ => Xyz,
    }
}
