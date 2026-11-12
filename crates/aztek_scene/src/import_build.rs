//! What the importers share: a file's nodes, meshes and materials in one plain form, and the scene made from them.
//! An importer reads its format into this, already in the scene's units and axes; `build` makes the nodes.

use std::sync::Arc;

use aztek_geo::mesh::Mesh;
use aztek_ops::{CmdResult, CommandError};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use crate::session::Session;

pub struct ImpMaterial {
    pub name: String,
    pub color: [f64; 3],
    /// A highlight: a blinn where a lambert would do otherwise.
    pub shiny: bool,
    /// Metalness and roughness, where the file's material has them: a standard surface.
    pub pbr: Option<(f64, f64)>,
    pub transparency: f64,
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
}

pub struct ImpNode {
    pub name: String,
    pub parent: Option<usize>,
    pub joint: bool,
    /// Its own matrix under its parent.
    pub matrix: DMat4,
    pub mesh: Option<ImpMesh>,
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
        crate::uv_sets::name_layers(&mut mesh, &names, &names[0]);
    }
    (mesh, made)
}

fn run(s: &mut Session, name: &str, call: Value) -> Result<Value, CommandError> {
    s.run(name, call)
}

/// Makes the nodes, meshes and materials, selects what was made at the top, and returns those nodes' names.
pub fn build(s: &mut Session, nodes: Vec<ImpNode>, materials: Vec<ImpMaterial>) -> CmdResult {
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
        let parent = n.parent.filter(|p| *p < nodes.len()).map_or(NO_NODE, |p| ids[p]);
        let wanted = clean_name(&n.name, if n.joint { "joint1" } else if n.mesh.is_some() { "polySurface1" } else { "transform1" });
        let g = &mut s.graph;
        let id = match &n.mesh {
            Some(m) => {
                let (xf, shape) = crate::dag::create_with_shape(g, "mesh", &wanted, parent)?;
                let (mesh, made) = mesh_of(m);
                faces_made[k] = made;
                g.set_value(PlugRef::new(shape, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(mesh))))?;
                xf
            }
            None => g.create_node(if n.joint { &joint } else { &transform }, &wanted, parent, true)?,
        };
        ids[k] = id;
        let (scale, rotation, translation) = n.matrix.to_scale_rotation_translation();
        let e = crate::transform::matrix_to_euler(&DMat3::from_quat(rotation), crate::transform::RotateOrder::Xyz);
        let path = s.graph.path(id);
        let tidy = |v: f64| if v.abs() < 1e-10 { 0.0 } else { v };
        // Commands run here take angles as they are stored, in radians.
        for (attr, v) in [("translate", translation), ("rotate", e), ("scale", scale)] {
            let rest = if attr == "scale" { 1.0 } else { 0.0 };
            if (v - DVec3::splat(rest)).length() > 1e-10 {
                run(s, "setAttr", json!({"args": [format!("{path}.{attr}"), tidy(v.x), tidy(v.y), tidy(v.z)], "type": "double3"}))?;
            }
        }
    }
    // Materials, made when a face first wears one.
    let mut material_names: Vec<Option<String>> = vec![None; materials.len()];
    for &k in &order {
        let Some(m) = &nodes[k].mesh else { continue };
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
                    let im = &materials[mat];
                    let ty = if im.pbr.is_some() { "standardSurface" } else if im.shiny { "blinn" } else { "lambert" };
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
    let tops: Vec<NodeId> = order.iter().filter(|&&k| nodes[k].parent.is_none_or(|p| p >= nodes.len())).map(|&k| ids[k]).collect();
    let names: Vec<String> = tops.iter().map(|&id| s.graph.path(id)).collect();
    if names.is_empty() {
        run(s, "select", json!({"clear": true}))?;
    } else {
        run(s, "select", json!({"args": names}))?;
    }
    Ok(json!(names))
}
