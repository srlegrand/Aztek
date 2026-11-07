//! Small helpers shared by the importer, the exporter and the stage commands.

use aztek_scene::graph::Graph;
use aztek_scene::plug::{NodeId, PlugRef, PlugValue, NO_NODE};
use glam::{DMat4, DVec3};
use openusd::gf;

/// The plug of a node's attribute by long or short name.
pub fn plug_of(g: &Graph, id: NodeId, attr: &str) -> Option<PlugRef> {
    g.node(id)?.ty().attr_index(attr).map(|i| PlugRef::new(id, i as i32))
}

pub fn get_f64(g: &mut Graph, id: NodeId, attr: &str) -> Option<f64> {
    let p = plug_of(g, id, attr)?;
    g.value(&p).ok()?.scalar()
}

pub fn get_v3(g: &mut Graph, id: NodeId, attr: &str) -> Option<DVec3> {
    let p = plug_of(g, id, attr)?;
    match g.value(&p).ok()? {
        PlugValue::Vec3(v) => Some(v),
        _ => None,
    }
}

pub fn get_str(g: &mut Graph, id: NodeId, attr: &str) -> Option<String> {
    let p = plug_of(g, id, attr)?;
    match g.value(&p).ok()? {
        PlugValue::String(s) => Some(s.to_string()),
        _ => None,
    }
}

/// Sets an attribute that exists; a missing one is not an error, since node types differ.
pub fn set_attr(g: &mut Graph, id: NodeId, attr: &str, v: PlugValue) {
    if let Some(p) = plug_of(g, id, attr) {
        if let Err(e) = g.set_value(p, v) {
            log::warn!("USD: can't set {attr}: {e}");
        }
    }
}

/// A mesh shape's evaluated output at the current time, deformers included.
pub fn evaluated_mesh(g: &mut Graph, shape: NodeId) -> Option<aztek_scene::plug::MeshPtr> {
    let root = aztek_scene::instancing::root(g, shape);
    let id = if root == NO_NODE { shape } else { root };
    let p = PlugRef::new(id, aztek_scene::mesh_nodes::MeshNode::OUT_MESH as i32);
    match g.value(&p).ok()? {
        PlugValue::Mesh(m) => m,
        _ => None,
    }
}

/// glam's columns and USD's rows hold the same sixteen numbers in the same order: both put translation in elements 12 to 14.
pub fn to_gf(m: &DMat4) -> gf::Matrix4d {
    gf::Matrix4d(m.to_cols_array())
}

pub fn from_gf(m: &gf::Matrix4d) -> DMat4 {
    DMat4::from_cols_array(&m.0)
}

pub fn v3f(v: DVec3) -> gf::Vec3f {
    gf::vec3f(v.x as f32, v.y as f32, v.z as f32)
}

/// A node name as a USD prim name: namespaces go or become underscores, and anything else USD refuses is replaced.
pub fn prim_name(name: &str, strip_namespaces: bool) -> String {
    let bare = if strip_namespaces { name.rsplit(':').next().unwrap_or(name) } else { name };
    let mut out: String = bare.chars().map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.is_empty() || out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// A USD name as a node name: Aztek's names take letters, digits and underscores, and a leading digit is not allowed.
pub fn node_name(name: &str) -> String {
    let mut out: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' }).collect();
    if out.is_empty() || out.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        out.insert(0, '_');
    }
    out
}

/// `target` written relative to the directory `base_dir`, when both are absolute and share a first component.
pub fn relative_to(target: &str, base_dir: &str) -> Option<String> {
    use std::path::{Component, Path};
    let (t, b) = (Path::new(target), Path::new(base_dir));
    if !t.is_absolute() || !b.is_absolute() {
        return None;
    }
    let tc: Vec<Component> = t.components().collect();
    let bc: Vec<Component> = b.components().collect();
    let common = tc.iter().zip(&bc).take_while(|(a, b)| a == b).count();
    if common == 0 {
        return None;
    }
    let mut parts: Vec<String> = Vec::new();
    for _ in common..bc.len() {
        parts.push("..".into());
    }
    for c in &tc[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    let joined = parts.join("/");
    Some(if joined.starts_with("..") { joined } else { format!("./{joined}") })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(prim_name("rig:arm_L", false), "rig_arm_L");
        assert_eq!(prim_name("rig:arm_L", true), "arm_L");
        assert_eq!(prim_name("1a", false), "_1a");
        assert_eq!(node_name("a.b"), "a_b");
    }

    #[test]
    fn relative() {
        assert_eq!(relative_to("/a/b/t.png", "/a/b").as_deref(), Some("./t.png"));
        assert_eq!(relative_to("/a/t.png", "/a/b").as_deref(), Some("../t.png"));
        assert_eq!(relative_to("t.png", "/a"), None);
    }

    #[test]
    fn matrix_layout() {
        let m = DMat4::from_translation(DVec3::new(1.0, 2.0, 3.0));
        let g = to_gf(&m);
        assert_eq!(&g.0[12..15], &[1.0, 2.0, 3.0]);
        assert_eq!(from_gf(&g), m);
    }
}
