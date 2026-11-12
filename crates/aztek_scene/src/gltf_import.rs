//! glTF 2.0 import: `.gltf` (JSON, its buffers inside it or in files beside it) and `.glb`.
//!
//! Read: the node hierarchy, triangle meshes with their first UV set, and each material's base colour, metalness and
//! roughness as a standard surface. The file is metres and Y up: lengths come in as centimetres, and into a Z-up scene
//! everything changes basis, so nothing carries a turn the file did not have. Nodes a skin names come in as joints.
//! Not read: skin weights, animation, textures, cameras, lights, morph targets, sparse accessors, lines and points.

use std::path::Path;

use aztek_ops::{fail, CmdResult, CommandDef, Commands};
use glam::{DMat4, DQuat, DVec3};
use serde_json::Value;

use crate::import_build::{build, ImpMaterial, ImpMesh, ImpNode};
use crate::session::Session;

/// Metres to centimetres.
const SCALE: f64 = 100.0;

fn unbase64(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let (mut acc, mut bits) = (0u32, 0u32);
    for c in text.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => continue,
        };
        acc = acc << 6 | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

struct File {
    doc: Value,
    buffers: Vec<Vec<u8>>,
}

fn load(path: &str) -> Result<File, String> {
    let raw = std::fs::read(path).map_err(|_| format!("can't read {path}"))?;
    let (doc, mut bin): (Value, Option<Vec<u8>>) = if raw.starts_with(b"glTF") {
        let word = |at: usize| raw.get(at..at + 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()) as usize);
        let n = word(12).ok_or("the file is cut short")?;
        let json = raw.get(20..20 + n).ok_or("the file is cut short")?;
        let doc = serde_json::from_slice(json).map_err(|e| format!("its JSON does not read: {e}"))?;
        let at = 20 + n;
        let bin = word(at).and_then(|m| raw.get(at + 8..at + 8 + m)).map(<[u8]>::to_vec);
        (doc, bin)
    } else {
        (serde_json::from_slice(&raw).map_err(|e| format!("its JSON does not read: {e}"))?, None)
    };
    let dir = Path::new(path).parent().map(Path::to_path_buf).unwrap_or_default();
    let mut buffers = Vec::new();
    for b in doc["buffers"].as_array().map(Vec::as_slice).unwrap_or(&[]) {
        buffers.push(match b["uri"].as_str() {
            Some(uri) if uri.starts_with("data:") => unbase64(uri.split_once(',').map_or("", |x| x.1)),
            Some(uri) => std::fs::read(dir.join(uri)).map_err(|_| format!("its buffer {uri} is not beside it"))?,
            // A .glb's own buffer.
            None => bin.take().unwrap_or_default(),
        });
    }
    Ok(File { doc, buffers })
}

impl File {
    /// An accessor's values as rows of numbers.
    fn rows(&self, accessor: &Value) -> Option<Vec<Vec<f64>>> {
        let a = &self.doc["accessors"][accessor.as_u64()? as usize];
        let view = &self.doc["bufferViews"][a["bufferView"].as_u64()? as usize];
        let buffer = self.buffers.get(view["buffer"].as_u64().unwrap_or(0) as usize)?;
        let width = match a["type"].as_str()? {
            "SCALAR" => 1,
            "VEC2" => 2,
            "VEC3" => 3,
            "VEC4" => 4,
            "MAT4" => 16,
            _ => return None,
        };
        let (size, kind) = match a["componentType"].as_u64()? {
            5120 => (1, 0),
            5121 => (1, 1),
            5122 => (2, 2),
            5123 => (2, 3),
            5125 => (4, 4),
            5126 => (4, 5),
            _ => return None,
        };
        let start = (view["byteOffset"].as_u64().unwrap_or(0) + a["byteOffset"].as_u64().unwrap_or(0)) as usize;
        let stride = view["byteStride"].as_u64().map_or(size * width, |s| s as usize);
        let normalized = a["normalized"].as_bool().unwrap_or(false);
        let mut out = Vec::new();
        for i in 0..a["count"].as_u64()? as usize {
            let mut row = Vec::with_capacity(width);
            for k in 0..width {
                let at = start + i * stride + k * size;
                let b = buffer.get(at..at + size)?;
                row.push(match kind {
                    0 => b[0] as i8 as f64 / if normalized { 127.0 } else { 1.0 },
                    1 => b[0] as f64 / if normalized { 255.0 } else { 1.0 },
                    2 => i16::from_le_bytes([b[0], b[1]]) as f64 / if normalized { 32767.0 } else { 1.0 },
                    3 => u16::from_le_bytes([b[0], b[1]]) as f64 / if normalized { 65535.0 } else { 1.0 },
                    4 => u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                    _ => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
                });
            }
            out.push(row);
        }
        Some(out)
    }
}

pub fn import(s: &mut Session, path: &str) -> CmdResult {
    let file = match load(path) {
        Ok(f) => f,
        Err(e) => fail!("importGltf: {e}"),
    };
    let doc = &file.doc;
    // The file's axes to the scene's: Y up to Z up is a quarter turn about X.
    let basis = if crate::world::up_z() { DMat4::from_rotation_x(std::f64::consts::FRAC_PI_2) } else { DMat4::IDENTITY };
    let to_scene = |m: DMat4| {
        let mut out = basis * m * basis.inverse();
        out.w_axis = (out.w_axis.truncate() * SCALE).extend(1.0);
        out
    };
    let list = |v: &Value| v.as_array().cloned().unwrap_or_default();
    let materials: Vec<ImpMaterial> = list(&doc["materials"])
        .iter()
        .enumerate()
        .map(|(k, m)| {
            let pbr = &m["pbrMetallicRoughness"];
            let c: Vec<f64> = list(&pbr["baseColorFactor"]).iter().filter_map(Value::as_f64).collect();
            ImpMaterial {
                name: m["name"].as_str().map_or_else(|| format!("material{}", k + 1), str::to_string),
                color: if c.len() >= 3 { [c[0], c[1], c[2]] } else { [1.0, 1.0, 1.0] },
                shiny: true,
                pbr: Some((pbr["metallicFactor"].as_f64().unwrap_or(1.0), pbr["roughnessFactor"].as_f64().unwrap_or(1.0))),
                transparency: if m["alphaMode"].as_str() == Some("BLEND") { 1.0 - c.get(3).copied().unwrap_or(1.0) } else { 0.0 },
                ..Default::default()
            }
        })
        .collect();
    let mut notes: Vec<String> = Vec::new();
    let mesh_of = |index: usize, notes: &mut Vec<String>| -> Option<ImpMesh> {
        let mut out = ImpMesh::default();
        let mut uv: Vec<[f32; 2]> = Vec::new();
        let mut any_uv = false;
        for p in list(&doc["meshes"][index]["primitives"]) {
            if p["mode"].as_u64().unwrap_or(4) != 4 {
                notes.push(format!("mesh {index}: a primitive that is not triangles was left out"));
                continue;
            }
            let Some(positions) = file.rows(&p["attributes"]["POSITION"]) else { continue };
            let base = out.points.len() as u32;
            out.points.extend(positions.iter().map(|r| basis.transform_point3(DVec3::new(r[0], r[1], r[2])) * SCALE));
            let coords = file.rows(&p["attributes"]["TEXCOORD_0"]);
            let indices: Vec<u32> = match file.rows(&p["indices"]) {
                Some(rows) => rows.iter().map(|r| r[0] as u32).collect(),
                None => (0..positions.len() as u32).collect(),
            };
            let material = p["material"].as_u64().map_or(usize::MAX, |m| m as usize);
            for tri in indices.chunks_exact(3) {
                out.sizes.push(3);
                out.face_material.push(material);
                for &i in tri {
                    out.indices.push(base + i);
                    // V runs down from the top in the file.
                    let st = coords.as_ref().and_then(|c| c.get(i as usize)).map(|r| [r[0] as f32, 1.0 - r[1] as f32]);
                    any_uv |= st.is_some();
                    uv.push(st.unwrap_or([0.0, 0.0]));
                }
            }
        }
        if any_uv {
            out.uvs.push(("map1".to_string(), uv));
        }
        (!out.sizes.is_empty()).then_some(out)
    };
    let gltf_nodes = list(&doc["nodes"]);
    let mut parent: Vec<Option<usize>> = vec![None; gltf_nodes.len()];
    for (k, n) in gltf_nodes.iter().enumerate() {
        for c in list(&n["children"]) {
            if let Some(slot) = c.as_u64().and_then(|c| parent.get_mut(c as usize)) {
                *slot = Some(k);
            }
        }
    }
    let mut joints = vec![false; gltf_nodes.len()];
    for skin in list(&doc["skins"]) {
        for j in list(&skin["joints"]) {
            if let Some(slot) = j.as_u64().and_then(|j| joints.get_mut(j as usize)) {
                *slot = true;
            }
        }
    }
    if !list(&doc["skins"]).is_empty() {
        notes.push("skin weights are not read: the joints and the meshes came in unbound".to_string());
    }
    if !list(&doc["animations"]).is_empty() {
        notes.push("animation is not read".to_string());
    }
    let mut nodes = Vec::new();
    for (k, n) in gltf_nodes.iter().enumerate() {
        let nums = |v: &Value| -> Vec<f64> { list(v).iter().filter_map(Value::as_f64).collect() };
        let m = nums(&n["matrix"]);
        let matrix = if m.len() == 16 {
            DMat4::from_cols_array(&m.try_into().unwrap())
        } else {
            let (t, r, sc) = (nums(&n["translation"]), nums(&n["rotation"]), nums(&n["scale"]));
            DMat4::from_scale_rotation_translation(
                if sc.len() == 3 { DVec3::new(sc[0], sc[1], sc[2]) } else { DVec3::ONE },
                if r.len() == 4 { DQuat::from_xyzw(r[0], r[1], r[2], r[3]).normalize() } else { DQuat::IDENTITY },
                if t.len() == 3 { DVec3::new(t[0], t[1], t[2]) } else { DVec3::ZERO },
            )
        };
        nodes.push(ImpNode {
            name: n["name"].as_str().map_or_else(|| format!("node{}", k + 1), str::to_string),
            parent: parent[k],
            joint: joints[k],
            matrix: to_scene(matrix),
            mesh: n["mesh"].as_u64().and_then(|m| mesh_of(m as usize, &mut notes)),
            parts: None,
            existing: None,
        });
    }
    if nodes.is_empty() {
        fail!("importGltf: the file has no nodes");
    }
    s.graph.open_chunk("Import glTF");
    let made = build(s, nodes, materials);
    s.graph.close_chunk();
    let made = made?;
    for n in notes {
        log::warn!("importGltf: {n}");
    }
    Ok(made)
}

/// The command the glTF plug-in brings for reading.
pub const COMMANDS: &[&str] = &["importGltf"];

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "importGltf",
            "Import a glTF 2.0 file (.gltf or .glb): its node hierarchy, triangle meshes with their first UV set, and \
             materials as standard surfaces. Lengths come in as centimetres and the file's Y up becomes the scene's up \
             axis. Skin weights, animation, textures, cameras and lights are not read. Returns the top nodes made.",
            |s, a| {
                let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("importGltf: path required") };
                import(s, &path)
            },
        )
        .positional("path"),
    );
}
