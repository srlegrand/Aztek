//! STL and PLY import and export.

use std::path::Path;
use std::sync::Arc;

use aztek_geo::ply::{read_ply_file, write_ply, PlyReadOptions, PlyWriteOptions};
use aztek_geo::stl::{read_stl_file, write_stl, StlReadOptions, StlSolid, StlWriteOptions};
use aztek_geo::{names, Mesh};
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec4, Mat3, Vec3};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::graph::{Graph, SelectMode};
use crate::mesh_nodes::{MeshNode, UpAxis};
use crate::modifiers;

use crate::plug::{PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

/// Centimetres per file unit.
fn unit_scale(unit: &str, cmd: &str) -> CmdResult<f64> {
    Ok(match unit {
        "mm" => 0.1,
        "cm" => 1.0,
        "m" => 100.0,
        "in" => 2.54,
        _ => fail!("{cmd}: unit must be mm, cm, m or in"),
    })
}

/// File space to the scene's: unit scale plus the turn between the file's up axis and the scene's.
fn file_to_scene(g: &Graph, a: &Args, cmd: &str) -> CmdResult<DMat4> {
    let s = unit_scale(&a.string("unit", ""), cmd)?;
    let up = a.string("upAxis", "z");
    if up != "y" && up != "z" {
        fail!("{cmd}: upAxis must be y or z");
    }
    let (file_z, scene_z) = (up == "z", g.up_axis == UpAxis::Z);
    let cols = |c: [[f64; 4]; 4]| DMat4::from_cols(DVec4::from(c[0]), DVec4::from(c[1]), DVec4::from(c[2]), DVec4::from(c[3]));
    Ok(if file_z && !scene_z {
        // (x, y, z) -> (x, z, -y)
        cols([[s, 0.0, 0.0, 0.0], [0.0, 0.0, -s, 0.0], [0.0, s, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    } else if !file_z && scene_z {
        // (x, y, z) -> (x, -z, y)
        cols([[s, 0.0, 0.0, 0.0], [0.0, 0.0, s, 0.0], [0.0, -s, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0]])
    } else {
        DMat4::from_diagonal(DVec4::new(s, s, s, 1.0))
    })
}

/// Positions by m, file normals (corner and point) by its inverse transpose.
fn transform_mesh(mesh: &mut Mesh, m: &DMat4) {
    let valid: Vec<bool> = (0..mesh.point_capacity()).map(|p| mesh.point_valid(p)).collect();
    mesh.positions_mut().par_iter_mut().enumerate().for_each(|(p, pos)| {
        if valid[p] {
            *pos = m.transform_point3(*pos);
        }
    });
    let linear = DMat3::from_mat4(*m);
    let n = linear.inverse().transpose().as_mat3();
    let flip = linear.determinant() < 0.0;
    let fix = |normals: &mut [[f32; 3]], n: Mat3| {
        normals.par_iter_mut().for_each(|v| {
            let x = Vec3::from_array(*v);
            if x.dot(x) > 0.0 {
                *v = (n * x).normalize().to_array();
            }
        });
    };
    if mesh.corner_attrs().has(names::NORMAL) {
        fix(mesh.corner_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL), n);
    }
    if mesh.point_attrs().has(names::NORMAL) {
        fix(mesh.point_attrs_mut().get_mut::<[f32; 3]>(names::NORMAL), n);
    }
    if flip {
        for f in 0..mesh.face_capacity() {
            if mesh.face_valid(f) {
                mesh.flip_face(f);
            }
        }
    }
}

fn read_path(a: &Args, cmd: &str) -> CmdResult<String> {
    match a.pos_str(0) {
        Some(p) => Ok(p.to_string()),
        None => fail!("{cmd}: path required"),
    }
}

fn add_meshes(g: &mut Graph, meshes: Vec<(String, Mesh)>) -> CmdResult {
    g.begin_batch();
    let r = (|| -> CmdResult {
        let mut names = Vec::new();
        let mut ids = Vec::new();
        for (name, mesh) in meshes {
            let (xf, n) = crate::mesh_nodes::create_mesh_node(g, if Graph::valid_name(&name) { &name } else { "" })?;
            g.set_value(PlugRef::new(n, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(mesh))))?;
            names.push(json!(g.path(xf)));
            ids.push(xf);
        }
        g.select(&ids, SelectMode::Replace);
        Ok(Value::Array(names))
    })();
    g.end_batch();
    r
}

/// The evaluated meshes of the named nodes and the meshes under them (the selection by default), in file space.
fn gather_meshes(g: &mut Graph, a: &Args, cmd: &str, scene_to_file: &DMat4) -> CmdResult<Vec<(String, Mesh)>> {
    let mut roots = Vec::new();
    for k in 1..a.positional.len() {
        let name = a.pos_str(k).unwrap_or("");
        match g.find(name)? {
            Some(id) => roots.push(id),
            None => fail!("{cmd}: no node named '{name}'"),
        }
    }
    if roots.is_empty() {
        roots = g.selection().to_vec();
    }
    let mut nodes = Vec::new();
    for r in roots {
        nodes.push(r);
        nodes.extend(g.descendants(r));
    }
    g.evaluate();
    let mut out = Vec::new();
    let mut seen = Vec::new();
    for id in nodes {
        if !g.node(id).is_some_and(|n| n.ty().is_a("mesh")) || seen.contains(&id) {
            continue;
        }
        seen.push(id);
        let Some(m) = modifiers::export_mesh(g, id) else { continue };
        let mut mesh = (*m).clone();
        let n = g.node(id).unwrap();
        transform_mesh(&mut mesh, &(*scene_to_file * n.matrix(Transform::WORLD_MATRIX)));
        out.push((n.name().to_string(), mesh));
    }
    if out.is_empty() {
        fail!("{cmd}: no meshes to export");
    }
    Ok(out)
}

fn write_file(path: &str, data: &[u8], cmd: &str) -> CmdResult<()> {
    if std::fs::write(path, data).is_err() {
        fail!("{cmd}: can't write {path}");
    }
    Ok(())
}

fn unit_flag(fallback: &str, doc: &str) -> FlagDef {
    FlagDef::new("unit", "", ArgType::String, json!(fallback), doc).choices(&["mm", "cm", "m", "in"])
}

fn up_flag() -> FlagDef {
    FlagDef::new("upAxis", "", ArgType::String, json!("z"), "The file's up axis; converted to the scene's.").choices(&["y", "z"])
}

fn stem(path: &str) -> String {
    Path::new(path).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_default()
}

pub fn register(c: &mut Commands<Session>) {
    type T = ArgType;
    type Cmd = CommandDef<Session>;
    let flag = |name: &str, ty, def: Value, doc: &str| FlagDef::new(name, "", ty, def, doc);

    c.register(
        Cmd::new(
            "importStl",
            "Import an STL file, binary or ASCII: one mesh per solid. Triangles sharing positions weld into a connected \
             mesh, edges bent past hardAngle are hard, and binary per-triangle colors (VisCAM and Materialise) become a \
             color set. Returns the new mesh names.",
            |s, a| {
                let path = read_path(a, "importStl")?;
                let m = file_to_scene(&s.graph, a, "importStl")?;
                let mut r = read_stl_file(&path, &StlReadOptions { weld: a.bool("weld"), hard_angle: a.float("hardAngle", 30.0) });
                if !r.ok() {
                    fail!("importStl: {}", r.error);
                }
                for solid in &mut r.solids {
                    transform_mesh(&mut solid.mesh, &m);
                }
                let st = stem(&path);
                let meshes = r
                    .solids
                    .into_iter()
                    .map(|sd| (if sd.name.is_empty() || r.binary { st.clone() } else { sd.name }, sd.mesh))
                    .collect();
                add_meshes(&mut s.graph, meshes)
            },
        )
        .positional("path")
        .flag(unit_flag("mm", "The file's length unit. STL has none; 3D printing assumes mm."))
        .flag(up_flag())
        .flag(flag("weld", T::Bool, json!(true), "Merge identical positions into shared points."))
        .flag(flag("hardAngle", T::Float, json!(30.0), "Edges bent more than this many degrees are hard; -1 leaves all soft.").min(-1.0).max(180.0)),
    );

    c.register(
        Cmd::new(
            "exportStl",
            "Export meshes to STL in world space, triangulated, defaulting to the selection and the meshes under it. \
             Binary merges them into one solid; ASCII writes a solid per mesh. Returns the path.",
            |s, a| {
                let path = read_path(a, "exportStl")?;
                let to_file = file_to_scene(&s.graph, a, "exportStl")?.inverse();
                let solids: Vec<StlSolid> =
                    gather_meshes(&mut s.graph, a, "exportStl", &to_file)?.into_iter().map(|(name, mesh)| StlSolid { name, mesh }).collect();
                let data = write_stl(&solids, &StlWriteOptions { binary: a.bool("binary"), colors: a.bool("colors"), ..Default::default() });
                write_file(&path, &data, "exportStl")?;
                Ok(json!(path))
            },
        )
        .positional("path objects...")
        .flag(unit_flag("mm", "The file's length unit. 3D printing assumes mm."))
        .flag(up_flag())
        .flag(flag("binary", T::Bool, json!(true), "Binary STL; off writes ASCII."))
        .flag(flag("colors", T::Bool, json!(false), "Binary only: the current color set as per-triangle VisCAM colors."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "importPly",
            "Import a PLY file (ASCII or binary, either byte order) as one mesh: polygons with normals, UVs (per vertex or \
             MeshLab's per-corner texcoord), vertex colors as colorSet and face colors as faceColor. Other numeric vertex and \
             face properties (quality, intensity, confidence...) become float attributes of the same name. A file with only \
             vertices comes in as a point cloud: a mesh of loose points. Returns the new mesh names, or with report=True a \
             dict of nodes, pointCloud, textureFiles and skipped.",
            |s, a| {
                let path = read_path(a, "importPly")?;
                let m = file_to_scene(&s.graph, a, "importPly")?;
                let mut r = read_ply_file(&path, &PlyReadOptions { keep_normals: a.bool("normals"), ..Default::default() });
                if !r.ok() {
                    fail!("importPly: {}", r.error);
                }
                transform_mesh(&mut r.mesh, &m);
                let names = add_meshes(&mut s.graph, vec![(stem(&path), std::mem::take(&mut r.mesh))])?;
                if !a.bool("report") {
                    return Ok(names);
                }
                Ok(json!({"nodes": names, "pointCloud": r.point_cloud, "textureFiles": r.texture_files, "skipped": r.skipped}))
            },
        )
        .positional("path")
        .flag(unit_flag("m", "The file's length unit. Scanners and photogrammetry write metres."))
        .flag(up_flag())
        .flag(flag("normals", T::Bool, json!(true), "Keep the file's normals."))
        .flag(flag("report", T::Bool, json!(false), "Return the import report instead of just the names.")),
    );

    c.register(
        Cmd::new(
            "exportPly",
            "Export meshes to PLY in world space, merged into one mesh, defaulting to the selection and the meshes under \
             it: polygons as they are, normals, uv0 and the current color set per vertex (points split where those differ). \
             Loose points and edges are written too. Returns the path.",
            |s, a| {
                let path = read_path(a, "exportPly")?;
                let to_file = file_to_scene(&s.graph, a, "exportPly")?.inverse();
                let mut meshes = gather_meshes(&mut s.graph, a, "exportPly", &to_file)?.into_iter();
                let mut merged = meshes.next().unwrap().1;
                for (_, m) in meshes {
                    merged.append(&m);
                }
                let data = write_ply(
                    &merged,
                    &PlyWriteOptions {
                        binary: a.bool("binary"),
                        normals: a.bool("normals"),
                        uvs: a.bool("uvs"),
                        colors: a.bool("colors"),
                        ..Default::default()
                    },
                );
                write_file(&path, &data, "exportPly")?;
                Ok(json!(path))
            },
        )
        .positional("path objects...")
        .flag(unit_flag("m", "The file's length unit."))
        .flag(up_flag())
        .flag(flag("binary", T::Bool, json!(true), "Binary little endian; off writes ASCII."))
        .flag(flag("normals", T::Bool, json!(true), "Write normals."))
        .flag(flag("uvs", T::Bool, json!(true), "Write uv0 as s, t."))
        .flag(flag("colors", T::Bool, json!(true), "Write the current color set as 8-bit RGBA."))
        .undoable(false),
    );
}
