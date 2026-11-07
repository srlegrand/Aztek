//! Reading geometry prims into Aztek meshes: Mesh with its primvars, and the implicit shapes as polygons. Used by import
//! and by pulling prims into the scene.

use std::collections::HashMap;

use aztek_geo::attribute::{names, AttrType, INVALID};
use aztek_geo::primitives as prims;
use aztek_geo::{colors, Mesh};
use glam::DVec3;
use openusd::gf;
use openusd::tf::Token;
use openusd::usd::{Attribute, Prim, TimeCode};

/// A time to read at: the given one, or the earliest value when not animating.
pub fn at(t: Option<f64>) -> TimeCode {
    t.map_or(TimeCode::EARLIEST, TimeCode::new)
}

pub fn get_vec<T>(a: &Attribute, t: Option<f64>) -> Option<Vec<T>>
where
    Vec<T>: openusd::sdf::FromValue,
    <Vec<T> as TryFrom<openusd::sdf::Value>>::Error: Into<openusd::Error>,
{
    a.get_at::<Vec<T>>(at(t)).ok().flatten()
}

pub fn token_of(prim: &Prim, name: &str) -> Option<String> {
    prim.attribute(name).get_at::<Token>(at(None)).ok().flatten().map(|t| t.as_str().to_string())
}

pub fn f64_of(prim: &Prim, name: &str, t: Option<f64>) -> Option<f64> {
    let a = prim.attribute(name);
    if let Ok(Some(v)) = a.get_at::<f64>(at(t)) {
        return Some(v);
    }
    a.get_at::<f32>(at(t)).ok().flatten().map(f64::from)
}

fn v3(p: &gf::Vec3f) -> DVec3 {
    DVec3::new(p.x as f64, p.y as f64, p.z as f64)
}

#[derive(Clone, Debug, Default)]
pub struct ReadOpts {
    pub exclude_primvars: Vec<String>,
    pub remap_uv: Vec<(String, String)>,
}

/// A mesh read from a prim, and which of the file's faces each of its faces is: degenerate faces are left out, so a face
/// list from the file has to go through this.
pub struct ReadMesh {
    pub mesh: Mesh,
    /// File face -> mesh face, or INVALID.
    pub face_map: Vec<u32>,
    pub scheme: String,
    pub skipped_faces: usize,
}

/// A primvar's value per corner in face order, from any interpolation, or None when the counts don't fit.
fn per_corner<T: Clone>(values: &[T], indices: Option<&[i32]>, interp: &str, fvi: &[i32], counts: &[i32]) -> Option<Vec<T>> {
    let fetch = |i: usize| -> Option<T> {
        let k = match indices {
            Some(ix) => usize::try_from(*ix.get(i)?).ok()?,
            None => i,
        };
        values.get(k).cloned()
    };
    let mut out = Vec::with_capacity(fvi.len());
    match interp {
        "faceVarying" => {
            for i in 0..fvi.len() {
                out.push(fetch(i)?);
            }
        }
        "vertex" | "varying" => {
            for &p in fvi {
                out.push(fetch(usize::try_from(p).ok()?)?);
            }
        }
        "uniform" => {
            for (f, &c) in counts.iter().enumerate() {
                let v = fetch(f)?;
                for _ in 0..c {
                    out.push(v.clone());
                }
            }
        }
        _ => {
            let v = fetch(0)?;
            out.resize(fvi.len(), v);
        }
    }
    Some(out)
}

/// Primvar names with the kind of value: (attr name, primvar name, type name).
fn primvars_of(prim: &Prim) -> Vec<(Attribute, String, String)> {
    let mut out = Vec::new();
    let Ok(attrs) = prim.attributes_in_namespace("primvars") else { return out };
    for a in attrs {
        let full = a.name().to_string();
        let name = full.strip_prefix("primvars:").unwrap_or(&full).to_string();
        if name.ends_with(":indices") || name.starts_with("skel:") {
            continue;
        }
        let ty = a.type_name().ok().flatten().map(|t| t.to_string()).unwrap_or_default();
        out.push((a, name, ty));
    }
    out
}

fn interp_of(a: &Attribute) -> String {
    a.get_metadata::<Token>("interpolation").ok().flatten().map_or_else(|| "constant".to_string(), |t| t.as_str().to_string())
}

fn indices_of(prim: &Prim, full: &str, t: Option<f64>) -> Option<Vec<i32>> {
    get_vec::<i32>(&prim.attribute(format!("{full}:indices").as_str()), t)
}

/// Reads a Mesh prim. None when it has no points.
pub fn read_mesh(prim: &Prim, t: Option<f64>, o: &ReadOpts) -> Option<ReadMesh> {
    let points = get_vec::<gf::Vec3f>(&prim.attribute("points"), t)?;
    let counts = get_vec::<i32>(&prim.attribute("faceVertexCounts"), t).unwrap_or_default();
    let fvi = get_vec::<i32>(&prim.attribute("faceVertexIndices"), t).unwrap_or_default();
    let left = token_of(prim, "orientation").as_deref() == Some("leftHanded");
    let scheme = token_of(prim, "subdivisionScheme").unwrap_or_else(|| "catmullClark".to_string());

    let mut m = Mesh::new();
    for p in &points {
        m.add_point(v3(p));
    }
    let mut face_map = Vec::with_capacity(counts.len());
    // For each mesh corner, its position in the file's face-varying arrays.
    let mut corner_src: Vec<(u32, u32)> = Vec::with_capacity(fvi.len());
    let mut start = 0usize;
    let mut skipped = 0;
    for &c in &counts {
        let c = c.max(0) as usize;
        if start + c > fvi.len() {
            break;
        }
        let mut pts: Vec<u32> = fvi[start..start + c].iter().map(|&i| i as u32).collect();
        let mut order: Vec<u32> = (start as u32..(start + c) as u32).collect();
        if left {
            pts.reverse();
            order.reverse();
        }
        let f = m.add_face(&pts);
        face_map.push(f);
        if f == INVALID {
            skipped += 1;
        } else {
            for (corner, src) in m.face_corners(f).into_iter().zip(order) {
                corner_src.push((corner, src));
            }
        }
        start += c;
    }

    if let Some(holes) = get_vec::<i32>(&prim.attribute("holeIndices"), t) {
        let flags = m.face_attrs_mut().get_or_add::<bool>("hole");
        for h in holes {
            if let Some(&f) = face_map.get(h as usize) {
                if f != INVALID {
                    flags[f as usize] = true;
                }
            }
        }
    }

    read_creases(prim, t, &mut m);
    read_primvars(prim, t, o, &mut m, &fvi, &counts, &corner_src);

    // Authored normals, kept when the mesh isn't subdivided: a subdivision surface makes its own.
    if scheme == "none" {
        let a = prim.attribute("normals");
        if let Some(n) = get_vec::<gf::Vec3f>(&a, t) {
            let interp = a.get_metadata::<Token>("interpolation").ok().flatten().map_or_else(|| "vertex".to_string(), |x| x.as_str().to_string());
            if let Some(list) = per_corner(&n, None, &interp, &fvi, &counts) {
                let dst = m.corner_attrs_mut().get_or_add::<[f32; 3]>(names::NORMAL);
                for (corner, src) in &corner_src {
                    let v = list[*src as usize];
                    dst[*corner as usize] = [v.x, v.y, v.z];
                }
            }
        }
    }
    Some(ReadMesh { mesh: m, face_map, scheme, skipped_faces: skipped })
}

fn read_creases(prim: &Prim, t: Option<f64>, m: &mut Mesh) {
    let (Some(ix), Some(len), Some(sharp)) = (
        get_vec::<i32>(&prim.attribute("creaseIndices"), t),
        get_vec::<i32>(&prim.attribute("creaseLengths"), t),
        get_vec::<f32>(&prim.attribute("creaseSharpnesses"), t),
    ) else {
        return;
    };
    // One sharpness per crease, or one per edge of each.
    let per_crease = sharp.len() == len.len();
    let mut pairs: Vec<(u32, u32, f32)> = Vec::new();
    let (mut at, mut si) = (0usize, 0usize);
    for (k, &n) in len.iter().enumerate() {
        let n = n.max(0) as usize;
        for j in 1..n {
            let (Some(a), Some(b)) = (ix.get(at + j - 1), ix.get(at + j)) else { break };
            let v = if per_crease { sharp[k] } else { sharp.get(si).copied().unwrap_or(0.0) };
            si += 1;
            pairs.push((*a as u32, *b as u32, v));
        }
        at += n;
    }
    let edges: Vec<(u32, f32)> = pairs.iter().filter_map(|(a, b, v)| { let e = m.find_edge(*a, *b); (e != INVALID).then_some((e, *v)) }).collect();
    if edges.is_empty() {
        return;
    }
    let dst = m.edge_attrs_mut().get_or_add::<f32>(names::CREASE);
    for (e, v) in edges {
        dst[e as usize] = v;
    }
}

fn read_primvars(prim: &Prim, t: Option<f64>, o: &ReadOpts, m: &mut Mesh, fvi: &[i32], counts: &[i32], corner_src: &[(u32, u32)]) {
    let mut first_uv = true;
    let list = primvars_of(prim);
    // Find st first, so the primary set lands in uv0 whatever order properties list in.
    let mut uvs: Vec<&(Attribute, String, String)> = list.iter().filter(|(_, _, ty)| ty.starts_with("texCoord2") || ty == "float2[]").collect();
    uvs.sort_by_key(|(_, n, _)| (n != "st", n.clone()));
    for (a, name, _) in uvs {
        if o.exclude_primvars.iter().any(|x| x == name) {
            continue;
        }
        let Some(values) = get_vec::<gf::Vec2f>(a, t) else { continue };
        let full = a.name().to_string();
        let ix = indices_of(prim, &full, t);
        let Some(vals) = per_corner(&values, ix.as_deref(), &interp_of(a), fvi, counts) else {
            log::warn!("USD: primvar {name} on {} doesn't fit the mesh", prim.path());
            continue;
        };
        let target = o.remap_uv.iter().find(|(from, _)| from == name).map_or_else(|| name.clone(), |(_, to)| to.clone());
        let layer = if first_uv { names::uv(0) } else { format!("uvset:{target}") };
        {
            let dst = m.corner_attrs_mut().get_or_add::<[f32; 2]>(&layer);
            for (corner, src) in corner_src {
                let v = vals[*src as usize];
                dst[*corner as usize] = [v.x, v.y];
            }
        }
        if first_uv {
            let set = if target == "st" { "map1".to_string() } else { target };
            if set != "map1" {
                m.face_attrs_mut().add(&format!("uvcurrent:{set}"), AttrType::Bool);
            }
            prims::mark_uv_seams(m, &names::uv(0));
            first_uv = false;
        }
    }

    // Colour sets. A constant or uniform display colour says what to shade with instead of being painted on points.
    let opacity = list.iter().find(|(_, n, _)| n == "displayOpacity");
    for (a, name, ty) in &list {
        if o.exclude_primvars.iter().any(|x| x == name) || !(ty == "color3f[]" || ty == "color4f[]") {
            continue;
        }
        let interp = interp_of(a);
        if name == "displayColor" && (interp == "constant" || interp == "uniform") {
            continue;
        }
        let full = a.name().to_string();
        let ix = indices_of(prim, &full, t);
        let rgba: Option<Vec<[f32; 4]>> = if ty == "color3f[]" {
            get_vec::<gf::Vec3f>(a, t).and_then(|v| per_corner(&v, ix.as_deref(), &interp, fvi, counts)).map(|v| v.iter().map(|c| [c.x, c.y, c.z, 1.0]).collect())
        } else {
            get_vec::<gf::Vec4f>(a, t).and_then(|v| per_corner(&v, ix.as_deref(), &interp, fvi, counts)).map(|v| v.iter().map(|c| [c.x, c.y, c.z, c.w]).collect())
        };
        let Some(mut rgba) = rgba else { continue };
        if name == "displayColor" {
            if let Some((oa, _, _)) = opacity {
                if let Some(op) = get_vec::<f32>(oa, t).and_then(|v| per_corner(&v, None, &interp_of(oa), fvi, counts)) {
                    for (c, x) in rgba.iter_mut().zip(op) {
                        c[3] = x;
                    }
                }
            }
        }
        colors::add_white(m, name);
        let dst = m.corner_attrs_mut().get_or_add::<[f32; 4]>(&colors::layer_name(name));
        for (corner, src) in corner_src {
            dst[*corner as usize] = rgba[*src as usize];
        }
    }
}

/// The constant display colour a prim authors, for shading a prim with no material.
pub fn constant_display_color(prim: &Prim) -> Option<[f32; 3]> {
    let a = prim.attribute("primvars:displayColor");
    let i = interp_of(&a);
    if i != "constant" {
        return None;
    }
    let c = get_vec::<gf::Vec3f>(&a, None)?;
    let c = c.first()?;
    Some([c.x, c.y, c.z])
}

/// An implicit shape as a polygon mesh in its own space.
pub fn read_gprim(prim: &Prim, type_name: &str, t: Option<f64>) -> Option<Mesh> {
    let double = |name: &str, d: f64| f64_of(prim, name, t).unwrap_or(d);
    let axis = token_of(prim, "axis").unwrap_or_else(|| "Y".to_string());
    let mut m = match type_name {
        "Cube" => {
            let s = double("size", 2.0);
            prims::make_cube(&prims::CubeOptions { width: s, height: s, depth: s, ..Default::default() })
        }
        "Sphere" => prims::make_sphere(&prims::SphereOptions { radius: double("radius", 1.0), subdiv_axis: 24, subdiv_height: 16, ..Default::default() }),
        "Cylinder" => prims::make_cylinder(&prims::CylinderOptions { radius: double("radius", 1.0), height: double("height", 2.0), subdiv_axis: 24, ..Default::default() }),
        "Cone" => prims::make_cone(&prims::ConeOptions { radius: double("radius", 1.0), height: double("height", 2.0), subdiv_axis: 24, ..Default::default() }),
        "Capsule" => prims::make_capsule(&prims::CapsuleOptions { radius: double("radius", 0.5), height: double("height", 1.0), subdiv_axis: 24, ..Default::default() }),
        "Plane" => prims::make_plane(&prims::PlaneOptions { width: double("width", 2.0), depth: double("length", 2.0), subdiv_x: 1, subdiv_z: 1 }),
        _ => return None,
    };
    // Aztek's primitives stand on Y; USD's take their axis.
    if axis != "Y" && type_name != "Cube" && type_name != "Sphere" {
        let (from, to) = (DVec3::Y, if axis == "X" { DVec3::X } else { DVec3::Z });
        let q = glam::DQuat::from_rotation_arc(from, to);
        for p in m.positions_mut() {
            *p = q * *p;
        }
    }
    Some(m)
}

/// A resolved asset as a file: one inside a package, `file.usdz[0/tex.png]`, is copied out to a temporary directory, once.
pub fn asset_on_disk(path: &str) -> String {
    let Some((pkg, inner)) = path.strip_suffix(']').and_then(|p| p.split_once('[')) else { return path.to_string() };
    let dir = std::env::temp_dir().join(format!("aztek_usdz_{}", std::process::id()));
    let stem = std::path::Path::new(pkg).file_stem().and_then(|x| x.to_str()).unwrap_or("package");
    let out = dir.join(stem).join(inner);
    if out.exists() {
        return out.to_string_lossy().into_owned();
    }
    let read = || -> Result<(), String> {
        let mut a = openusd::usdz::Archive::open(pkg).map_err(|e| e.to_string())?;
        let bytes = a.entry(inner).map_err(|e| e.to_string())?;
        std::fs::create_dir_all(out.parent().ok_or("no parent")?).map_err(|e| e.to_string())?;
        std::fs::write(&out, &*bytes).map_err(|e| e.to_string())
    };
    match read() {
        Ok(()) => out.to_string_lossy().into_owned(),
        Err(e) => {
            log::warn!("USD: {path}: {e}");
            path.to_string()
        }
    }
}

/// Authored primvars the mesh lacks a place for are ignored. The names read, for tests and the prim panel.
pub fn primvar_names(prim: &Prim) -> Vec<String> {
    primvars_of(prim).into_iter().map(|(_, n, _)| n).collect()
}

pub(crate) type SubsetFaces = HashMap<String, Vec<u32>>;
