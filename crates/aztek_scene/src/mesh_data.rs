//! Mesh <-> scene document.
//!
//! Points, faces (sizes + flat indices), material names, and every attribute layer; corner layers follow face order,
//! edge layers come with their point pairs so they survive edge renumbering.
//!
//! The packed form (`meshToJsonPacked`, numeric arrays as binary blobs for the binary scene file) and colour-set
//! adoption on load (`geo::colors::adopt`) come with the binary scene file and the colour sets module.

use std::sync::Arc;

use aztek_geo::{AttrType, Attribute, Domain, Index, Mesh, INVALID};
use glam::DVec3;
use serde_json::{json, Map, Value};

use crate::error::{GraphError, Result};
use crate::plug::MeshPtr;

fn num(v: f64) -> Value {
    serde_json::Number::from_f64(v)
        .map(Value::Number)
        .unwrap_or(Value::Null)
}

fn layer_data(a: &Attribute, order: &[Index]) -> Value {
    let n = a.ty().components();
    let mut data = Vec::new();
    for &i in order {
        let p = a.raw(i as usize);
        match a.ty() {
            AttrType::Int => data.push(json!(i32::from_ne_bytes(p[..4].try_into().unwrap()))),
            AttrType::Bool => data.push(json!(p[0] != 0)),
            _ => {
                for k in 0..n {
                    data.push(num(
                        f32::from_ne_bytes(p[k * 4..k * 4 + 4].try_into().unwrap()) as f64,
                    ));
                }
            }
        }
    }
    Value::Array(data)
}

fn type_from_name(s: &str) -> Result<AttrType> {
    use AttrType::*;
    [Float, Float2, Float3, Float4, Int, Bool]
        .into_iter()
        .find(|t| t.name() == s)
        .ok_or_else(|| GraphError::invalid(format!("unknown attribute type '{s}'")))
}

pub fn mesh_to_json(mesh: &Mesh) -> Value {
    let mut points = Vec::new();
    let mut point_order = Vec::new();
    let mut point_remap = vec![INVALID; mesh.point_capacity() as usize];
    for p in 0..mesh.point_capacity() {
        if mesh.point_valid(p) {
            point_remap[p as usize] = point_order.len() as Index;
            point_order.push(p);
            let v = mesh.position(p);
            points.push(json!([num(v.x), num(v.y), num(v.z)]));
        }
    }
    let (mut sizes, mut indices, mut face_order, mut corner_order) =
        (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for f in 0..mesh.face_capacity() {
        if !mesh.face_valid(f) {
            continue;
        }
        face_order.push(f);
        sizes.push(json!(mesh.face_size(f)));
        mesh.for_each_face_corner(f, |c| {
            corner_order.push(c);
            indices.push(json!(point_remap[mesh.corner_point(c) as usize]));
        });
    }
    let (mut edge_order, mut edges) = (Vec::new(), Vec::new());
    for e in 0..mesh.edge_capacity() {
        if mesh.edge_valid(e) {
            edge_order.push(e);
            edges.push(json!(point_remap[mesh.edge_point(e, 0) as usize]));
            edges.push(json!(point_remap[mesh.edge_point(e, 1) as usize]));
        }
    }
    let mut layers = Vec::new();
    for (domain, order) in [
        (Domain::Point, &point_order),
        (Domain::Edge, &edge_order),
        (Domain::Face, &face_order),
        (Domain::Corner, &corner_order),
    ] {
        for a in mesh.attributes(domain).layers() {
            layers.push(json!({"name": a.name(), "domain": domain.name(), "type": a.ty().name(), "data": layer_data(a, order)}));
        }
    }
    let mut out = Map::new();
    out.insert("points".into(), Value::Array(points));
    out.insert("faceSizes".into(), Value::Array(sizes));
    out.insert("faceIndices".into(), Value::Array(indices));
    if !edges.is_empty() {
        out.insert("edges".into(), Value::Array(edges));
    }
    if !layers.is_empty() {
        out.insert("layers".into(), Value::Array(layers));
    }
    if !mesh.material_names().is_empty() {
        out.insert("materials".into(), json!(mesh.material_names()));
    }
    let cs = mesh.color_set_info();
    if !cs.current.is_empty() || !cs.half.is_empty() {
        out.insert(
            "colorSets".into(),
            json!({"current": cs.current, "half": cs.half}),
        );
    }
    Value::Object(out)
}

fn numbers(v: Option<&Value>) -> Vec<f64> {
    v.and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|x| {
                    x.as_bool()
                        .map(|b| b as i32 as f64)
                        .or_else(|| x.as_f64())
                        .unwrap_or(0.0)
                })
                .collect()
        })
        .unwrap_or_default()
}

fn index_list(v: Option<&Value>) -> Vec<Index> {
    numbers(v).into_iter().map(|d| d as Index).collect()
}

/// Fills a layer from its saved values; entry k goes to element order[k], or to element k when identity is set.
fn set_layer_data(a: &mut Attribute, values: &[f64], order: &[Index], identity: bool) {
    let n = a.ty().components();
    // Every entry, dropped or not, takes the same number of values, so each one's offset is known up front.
    let stride = if matches!(a.ty(), AttrType::Int | AttrType::Bool) {
        1
    } else {
        n
    };
    let entries = if identity { a.len() } else { order.len() };
    let ty = a.ty();
    for k in 0..entries {
        let i = if identity { k as Index } else { order[k] };
        if i == INVALID {
            continue;
        }
        let at = k * stride;
        let get = |o: usize| values.get(at + o).copied();
        match ty {
            AttrType::Int => {
                a.values_mut::<i32>()[i as usize] = get(0).map(|v| v as i32).unwrap_or(0)
            }
            AttrType::Bool => a.values_mut::<bool>()[i as usize] = get(0).is_some_and(|v| v != 0.0),
            AttrType::Float => a.values_mut::<f32>()[i as usize] = get(0).unwrap_or(0.0) as f32,
            AttrType::Float2 => {
                a.values_mut::<[f32; 2]>()[i as usize] =
                    std::array::from_fn(|c| get(c).unwrap_or(0.0) as f32)
            }
            AttrType::Float3 => {
                a.values_mut::<[f32; 3]>()[i as usize] =
                    std::array::from_fn(|c| get(c).unwrap_or(0.0) as f32)
            }
            AttrType::Float4 => {
                a.values_mut::<[f32; 4]>()[i as usize] =
                    std::array::from_fn(|c| get(c).unwrap_or(0.0) as f32)
            }
        }
    }
}

pub fn mesh_from_json(j: &Value) -> Result<MeshPtr> {
    let member = |k: &str| j.get(k);
    let points: Vec<DVec3> = member("points")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| {
                    let c = |i: usize| p.get(i).and_then(Value::as_f64).unwrap_or(0.0);
                    DVec3::new(c(0), c(1), c(2))
                })
                .collect()
        })
        .unwrap_or_default();
    let sizes = index_list(member("faceSizes"));
    let indices = index_list(member("faceIndices"));
    let mut mesh = Mesh::from_polygons(&points, &sizes, &indices);

    // Faces that from_polygons rejected leave holes in the order; match by walking the input.
    let (mut face_order, mut corner_order) = (Vec::new(), Vec::new());
    if mesh.face_capacity() as usize != sizes.len()
        || mesh.corner_capacity() as usize != indices.len()
    {
        let (mut f, mut at) = (0, 0usize);
        for &s in &sizes {
            let first = indices.get(at).copied();
            at += s as usize;
            let mut found = INVALID;
            if f < mesh.face_capacity()
                && mesh.face_valid(f)
                && mesh.face_size(f) == s
                && Some(mesh.corner_point(mesh.face_corner(f))) == first
            {
                found = f;
                f += 1;
            }
            face_order.push(found);
            if found == INVALID {
                corner_order.extend(std::iter::repeat_n(INVALID, s as usize));
            } else {
                mesh.for_each_face_corner(found, |c| corner_order.push(c));
            }
        }
    }

    let edges = index_list(member("edges"));
    // A mesh saved by this app rebuilds with the same edge ids, which saves a lookup per edge.
    let same_edges = edges.len() == mesh.edge_capacity() as usize * 2
        && (0..mesh.edge_capacity()).all(|e| {
            mesh.edge_point(e, 0) == edges[e as usize * 2]
                && mesh.edge_point(e, 1) == edges[e as usize * 2 + 1]
        });
    let mut edge_order = Vec::new();
    if !same_edges {
        edge_order = (0..edges.len() / 2)
            .map(|i| mesh.find_edge(edges[i * 2], edges[i * 2 + 1]))
            .collect();
        // Loose edges belong to no face, so from_polygons doesn't rebuild them.
        for i in 0..edge_order.len() {
            let (a, b) = (edges[i * 2], edges[i * 2 + 1]);
            if edge_order[i] == INVALID
                && a != b
                && a < mesh.point_capacity()
                && b < mesh.point_capacity()
            {
                edge_order[i] = mesh.add_edge(a, b);
            }
        }
    }

    let no_layers = Vec::new();
    let point_order: Vec<Index> = Vec::new();
    for layer in member("layers")
        .and_then(Value::as_array)
        .unwrap_or(&no_layers)
    {
        let domain_name = layer.get("domain").and_then(Value::as_str).unwrap_or("");
        let (domain, order) = match domain_name {
            "edge" => (Domain::Edge, &edge_order),
            "face" => (Domain::Face, &face_order),
            "corner" => (Domain::Corner, &corner_order),
            _ => (Domain::Point, &point_order),
        };
        let name = layer.get("name").and_then(Value::as_str).unwrap_or("");
        let ty = type_from_name(layer.get("type").and_then(Value::as_str).unwrap_or(""))?;
        let values = numbers(layer.get("data"));
        let identity = match domain {
            Domain::Point => true,
            Domain::Edge => same_edges,
            _ => face_order.is_empty(),
        };
        let a = mesh.attributes_mut(domain).add(name, ty);
        set_layer_data(a, &values, order, identity);
    }
    if let Some(m) = member("materials").and_then(Value::as_array) {
        *mesh.material_names_mut() = m
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect();
    }
    if let Some(cs) = member("colorSets").and_then(Value::as_object) {
        let info = mesh.color_set_info_mut();
        info.current = cs
            .get("current")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        info.half = cs
            .get("half")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
    }
    Ok(Arc::new(mesh))
}

#[cfg(test)]
mod tests {
    use super::*;
    use aztek_geo::primitives::{make_cylinder, CylinderOptions};

    #[test]
    fn round_trip_keeps_topology_and_layers() {
        let mut m = make_cylinder(&CylinderOptions {
            fraction: 0.5,
            ..Default::default()
        });
        m.material_names_mut().push("steel".into());
        m.face_attrs_mut().get_or_add::<i32>("material")[3] = 0;
        // A deleted face and a loose edge both survive the trip.
        m.remove_face(2);
        let (a, b) = (
            m.add_point(DVec3::new(0.0, 500.0, 0.0)),
            m.add_point(DVec3::new(1.0, 500.0, 0.0)),
        );
        m.add_edge(a, b);
        let j = mesh_to_json(&m);
        let back = mesh_from_json(&j).unwrap();
        assert_eq!(back.num_faces(), m.num_faces());
        assert_eq!(back.num_edges(), m.num_edges());
        assert_eq!(back.material_names(), m.material_names());
        let k = mesh_to_json(&back);
        for key in ["points", "faceSizes", "faceIndices", "materials"] {
            assert_eq!(k[key], j[key], "{key}");
        }
        // Edge ids may renumber on reload; the set of edges may not.
        let pairs = |v: &Value| {
            let a: Vec<u64> = v["edges"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_u64().unwrap())
                .collect();
            let mut p: Vec<(u64, u64)> = a
                .chunks(2)
                .map(|c| (c[0].min(c[1]), c[0].max(c[1])))
                .collect();
            p.sort_unstable();
            p
        };
        assert_eq!(pairs(&k), pairs(&j));
        assert!(back.validate(aztek_geo::validate::STRUCTURE).is_empty());
    }
}
