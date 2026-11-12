//! FBX import: binary FBX files, read from the format's public descriptions with no library of its vendor's.
//!
//! Read: the model hierarchy with each node's place (translation, rotation in its order, scale, pre and post rotation,
//! pivots and offsets, folded into one transform), meshes as polygons with their UV sets and geometric offset, limb
//! nodes as joints, and materials' colour with which faces wear them. The file's unit and up axis are honoured: lengths
//! come in as centimetres and a file with another up axis changes basis. Not read: ASCII FBX, skin weights, animation,
//! blend shapes, textures, cameras and lights (their nodes come in as transforms), normals (the mesh's own are used).

use std::collections::HashMap;

use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat4, DVec3};
use serde_json::Value;

use crate::import_build::{build, ImpMaterial, ImpMesh, ImpNode};
use crate::session::Session;
use crate::transform::{euler_to_matrix, RotateOrder};

#[derive(Clone, Debug, PartialEq)]
pub enum V {
    I(i64),
    D(f64),
    S(String),
    Ints(Vec<i64>),
    Doubles(Vec<f64>),
    Raw,
}

#[derive(Clone, Debug, Default)]
pub struct Rec {
    pub name: String,
    pub props: Vec<V>,
    pub kids: Vec<Rec>,
}

impl Rec {
    fn kid(&self, name: &str) -> Option<&Rec> {
        self.kids.iter().find(|k| k.name == name)
    }
    fn all<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Rec> {
        self.kids.iter().filter(move |k| k.name == name)
    }
    fn text(&self, k: usize) -> &str {
        match self.props.get(k) {
            Some(V::S(s)) => s,
            _ => "",
        }
    }
    fn int(&self, k: usize) -> i64 {
        match self.props.get(k) {
            Some(V::I(v)) => *v,
            Some(V::D(v)) => *v as i64,
            _ => 0,
        }
    }
    fn doubles(&self) -> Vec<f64> {
        match self.props.first() {
            Some(V::Doubles(v)) => v.clone(),
            Some(V::Ints(v)) => v.iter().map(|x| *x as f64).collect(),
            _ => Vec::new(),
        }
    }
    fn ints(&self) -> Vec<i64> {
        match self.props.first() {
            Some(V::Ints(v)) => v.clone(),
            Some(V::Doubles(v)) => v.iter().map(|x| *x as i64).collect(),
            _ => Vec::new(),
        }
    }
    /// A name without the class the binary form joins to it.
    fn label(&self) -> String {
        self.text(1).split('\u{0}').next().unwrap_or("").to_string()
    }
    /// A property's numbers, from the node's Properties70.
    fn prop(&self, name: &str) -> Option<Vec<f64>> {
        let p = self.kid("Properties70")?.kids.iter().find(|p| p.text(0) == name)?;
        Some(
            p.props
                .iter()
                .skip(4)
                .filter_map(|v| match v {
                    V::D(x) => Some(*x),
                    V::I(x) => Some(*x as f64),
                    _ => None,
                })
                .collect(),
        )
    }
}

struct Reader<'a> {
    b: &'a [u8],
    /// Offsets are 64 bits from version 7500 on.
    wide: bool,
}

impl Reader<'_> {
    fn bytes(&self, at: usize, n: usize) -> Result<&[u8], String> {
        self.b.get(at..at + n).ok_or_else(|| "the file is cut short".to_string())
    }
    fn u32(&self, at: usize) -> Result<usize, String> {
        Ok(u32::from_le_bytes(self.bytes(at, 4)?.try_into().unwrap()) as usize)
    }
    fn word(&self, at: usize) -> Result<usize, String> {
        if self.wide {
            Ok(u64::from_le_bytes(self.bytes(at, 8)?.try_into().unwrap()) as usize)
        } else {
            self.u32(at)
        }
    }

    /// An array: how many, how packed (1 is deflated), how many bytes, the values.
    fn array(&self, at: usize, width: usize) -> Result<(Vec<u8>, usize), String> {
        let (n, packed, len) = (self.u32(at)?, self.u32(at + 4)?, self.u32(at + 8)?);
        let data = self.bytes(at + 12, len)?;
        let plain = match packed {
            0 => data.to_vec(),
            1 => miniz_oxide::inflate::decompress_to_vec_zlib(data).map_err(|_| "a packed array does not unpack".to_string())?,
            other => return Err(format!("an array is packed in a way not known ({other})")),
        };
        if plain.len() != n * width {
            return Err("an array is not as long as it says".to_string());
        }
        Ok((plain, at + 12 + len))
    }

    /// One record at `at`, or None at the empty record that closes a list. Returns where the next begins.
    fn rec(&self, at: usize) -> Result<(Option<Rec>, usize), String> {
        let w = if self.wide { 8 } else { 4 };
        let (end, count) = (self.word(at)?, self.word(at + w)?);
        let name_len = *self.b.get(at + 3 * w).ok_or("the file is cut short")? as usize;
        if end == 0 {
            return Ok((None, at + 3 * w + 1));
        }
        let name = String::from_utf8_lossy(self.bytes(at + 3 * w + 1, name_len)?).into_owned();
        let mut p = at + 3 * w + 1 + name_len;
        let mut props = Vec::with_capacity(count);
        for _ in 0..count {
            let code = *self.b.get(p).ok_or("the file is cut short")?;
            p += 1;
            match code {
                b'C' | b'B' => {
                    props.push(V::I(self.bytes(p, 1)?[0] as i64));
                    p += 1;
                }
                b'Y' => {
                    props.push(V::I(i16::from_le_bytes(self.bytes(p, 2)?.try_into().unwrap()) as i64));
                    p += 2;
                }
                b'I' => {
                    props.push(V::I(i32::from_le_bytes(self.bytes(p, 4)?.try_into().unwrap()) as i64));
                    p += 4;
                }
                b'L' => {
                    props.push(V::I(i64::from_le_bytes(self.bytes(p, 8)?.try_into().unwrap())));
                    p += 8;
                }
                b'F' => {
                    props.push(V::D(f32::from_le_bytes(self.bytes(p, 4)?.try_into().unwrap()) as f64));
                    p += 4;
                }
                b'D' => {
                    props.push(V::D(f64::from_le_bytes(self.bytes(p, 8)?.try_into().unwrap())));
                    p += 8;
                }
                b'S' | b'R' => {
                    let n = self.u32(p)?;
                    let data = self.bytes(p + 4, n)?;
                    props.push(if code == b'S' { V::S(String::from_utf8_lossy(data).into_owned()) } else { V::Raw });
                    p += 4 + n;
                }
                b'd' | b'f' | b'i' | b'l' | b'b' | b'c' => {
                    let width = match code {
                        b'd' | b'l' => 8,
                        b'f' | b'i' => 4,
                        _ => 1,
                    };
                    let (data, next) = self.array(p, width)?;
                    p = next;
                    props.push(match code {
                        b'd' => V::Doubles(data.chunks(8).map(|c| f64::from_le_bytes(c.try_into().unwrap())).collect()),
                        b'f' => V::Doubles(data.chunks(4).map(|c| f32::from_le_bytes(c.try_into().unwrap()) as f64).collect()),
                        b'i' => V::Ints(data.chunks(4).map(|c| i32::from_le_bytes(c.try_into().unwrap()) as i64).collect()),
                        b'l' => V::Ints(data.chunks(8).map(|c| i64::from_le_bytes(c.try_into().unwrap())).collect()),
                        _ => V::Ints(data.iter().map(|x| *x as i64).collect()),
                    });
                }
                other => return Err(format!("{name}: a value of a kind not known ({})", other as char)),
            }
        }
        let mut kids = Vec::new();
        while p < end {
            let (k, next) = self.rec(p)?;
            p = next;
            match k {
                Some(k) => kids.push(k),
                None => break,
            }
        }
        Ok((Some(Rec { name, props, kids }), end))
    }
}

/// The file's records under one nameless top.
pub fn parse(bytes: &[u8]) -> Result<Rec, String> {
    if !bytes.starts_with(b"Kaydara FBX Binary  \x00") {
        if bytes.starts_with(b"; FBX") || bytes.windows(18).take(400).any(|w| w == b"FBXHeaderExtension") {
            return Err("this is an ASCII FBX file, which is not read: save it as binary FBX".to_string());
        }
        return Err("not an FBX file".to_string());
    }
    let version = bytes.get(23..27).map(|b| u32::from_le_bytes(b.try_into().unwrap())).ok_or("the file is cut short")?;
    let r = Reader { b: bytes, wide: version >= 7500 };
    let mut top = Rec::default();
    let mut at = 27;
    while at < bytes.len() {
        let (rec, next) = r.rec(at)?;
        at = next;
        match rec {
            Some(rec) => top.kids.push(rec),
            None => break,
        }
    }
    Ok(top)
}

fn v3(p: Option<Vec<f64>>, rest: f64) -> DVec3 {
    match p {
        Some(v) if v.len() >= 3 => DVec3::new(v[0], v[1], v[2]),
        _ => DVec3::splat(rest),
    }
}

fn turn(degrees: DVec3, order: RotateOrder) -> DMat4 {
    euler_to_matrix(DVec3::new(degrees.x.to_radians(), degrees.y.to_radians(), degrees.z.to_radians()), order)
}

/// A model's own matrix under its parent, as the format builds it: translation, rotation offset and pivot, pre
/// rotation, rotation, post rotation, then scale about its pivot with its offset.
fn local_matrix(model: &Rec) -> DMat4 {
    let t = DMat4::from_translation(v3(model.prop("Lcl Translation"), 0.0));
    let s = DMat4::from_scale(v3(model.prop("Lcl Scaling"), 1.0));
    let order = match model.prop("RotationOrder").and_then(|v| v.first().copied()).unwrap_or(0.0) as i64 {
        1 => RotateOrder::Xzy,
        2 => RotateOrder::Yzx,
        3 => RotateOrder::Yxz,
        4 => RotateOrder::Zxy,
        5 => RotateOrder::Zyx,
        _ => RotateOrder::Xyz,
    };
    let r = turn(v3(model.prop("Lcl Rotation"), 0.0), order);
    let pre = turn(v3(model.prop("PreRotation"), 0.0), RotateOrder::Xyz);
    let post = turn(v3(model.prop("PostRotation"), 0.0), RotateOrder::Xyz);
    let (rp, sp) = (v3(model.prop("RotationPivot"), 0.0), v3(model.prop("ScalingPivot"), 0.0));
    let (ro, so) = (v3(model.prop("RotationOffset"), 0.0), v3(model.prop("ScalingOffset"), 0.0));
    t * DMat4::from_translation(ro + rp) * pre * r * post.inverse() * DMat4::from_translation(-rp) * DMat4::from_translation(so + sp) * s * DMat4::from_translation(-sp)
}

pub fn import(s: &mut Session, path: &str) -> CmdResult {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => fail!("importFbx: can't read {path}"),
    };
    let top = match parse(&bytes) {
        Ok(t) => t,
        Err(e) => fail!("importFbx: {e}"),
    };
    let Some(objects) = top.kid("Objects") else { fail!("importFbx: the file has no objects") };
    // The file's unit in centimetres, and its up axis: 1 is Y, 2 is Z.
    let settings = top.kid("GlobalSettings");
    let setting = |name: &str, d: f64| settings.and_then(|g| g.prop(name)).and_then(|v| v.first().copied()).unwrap_or(d);
    let unit = setting("UnitScaleFactor", 1.0);
    let file_up_z = setting("UpAxis", 1.0) as i64 == 2;
    let basis = match (file_up_z, crate::world::up_z()) {
        (true, false) => DMat4::from_rotation_x(-std::f64::consts::FRAC_PI_2),
        (false, true) => DMat4::from_rotation_x(std::f64::consts::FRAC_PI_2),
        _ => DMat4::IDENTITY,
    };
    let to_scene = |m: DMat4| {
        let mut out = basis * m * basis.inverse();
        out.w_axis = (out.w_axis.truncate() * unit).extend(1.0);
        out
    };
    // What is linked under what, in the file's order.
    let mut under: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut parent_of: HashMap<i64, i64> = HashMap::new();
    for c in top.kid("Connections").map(|c| c.kids.as_slice()).unwrap_or(&[]) {
        if c.text(0) == "OO" {
            under.entry(c.int(2)).or_default().push(c.int(1));
            parent_of.insert(c.int(1), c.int(2));
        }
    }
    let models: Vec<&Rec> = objects.all("Model").collect();
    let index_of: HashMap<i64, usize> = models.iter().enumerate().map(|(k, m)| (m.int(0), k)).collect();
    let geometries: HashMap<i64, &Rec> = objects.all("Geometry").filter(|g| g.text(2) == "Mesh").map(|g| (g.int(0), g)).collect();
    let material_recs: Vec<&Rec> = objects.all("Material").collect();
    let material_index: HashMap<i64, usize> = material_recs.iter().enumerate().map(|(k, m)| (m.int(0), k)).collect();
    let materials: Vec<ImpMaterial> = material_recs
        .iter()
        .map(|m| {
            let c = v3(m.prop("DiffuseColor").or_else(|| m.prop("Diffuse")), 0.5);
            let factor = m.prop("DiffuseFactor").and_then(|v| v.first().copied()).unwrap_or(1.0);
            let _ = factor;
            ImpMaterial {
                name: m.label(),
                color: [c.x, c.y, c.z],
                shiny: m.kid("ShadingModel").is_some_and(|k| k.text(0).eq_ignore_ascii_case("phong")),
                pbr: None,
                transparency: m.prop("Opacity").and_then(|v| v.first().copied()).map_or(0.0, |o| (1.0 - o).clamp(0.0, 1.0)),
            }
        })
        .collect();
    let mut notes: Vec<String> = Vec::new();
    let mut nodes = Vec::new();
    for m in &models {
        let id = m.int(0);
        let below = under.get(&id).cloned().unwrap_or_default();
        let slots: Vec<usize> = below.iter().filter_map(|c| material_index.get(c).copied()).collect();
        let mesh = below.iter().find_map(|c| geometries.get(c)).and_then(|geo| {
            let vertices = geo.kid("Vertices")?.doubles();
            let index = geo.kid("PolygonVertexIndex")?.ints();
            // A mesh's offset from its node, which children do not inherit.
            let offset = DMat4::from_translation(v3(m.prop("GeometricTranslation"), 0.0)) * turn(v3(m.prop("GeometricRotation"), 0.0), RotateOrder::Xyz) * DMat4::from_scale(v3(m.prop("GeometricScaling"), 1.0));
            let mut out = ImpMesh { points: vertices.chunks_exact(3).map(|p| basis.transform_point3(offset.transform_point3(DVec3::new(p[0], p[1], p[2]))) * unit).collect(), ..Default::default() };
            let mut size = 0u32;
            for &i in &index {
                size += 1;
                // The complement closes a polygon.
                out.indices.push(if i < 0 { (-i - 1) as u32 } else { i as u32 });
                if i < 0 {
                    out.sizes.push(size);
                    size = 0;
                }
            }
            for layer in geo.all("LayerElementUV") {
                let values = layer.kid("UV").map(Rec::doubles).unwrap_or_default();
                let direct = layer.kid("ReferenceInformationType").is_none_or(|r| r.text(0) == "Direct");
                let by_corner = layer.kid("MappingInformationType").is_none_or(|r| r.text(0) == "ByPolygonVertex");
                let picks = layer.kid("UVIndex").map(Rec::ints).unwrap_or_default();
                let uv: Vec<[f32; 2]> = (0..out.indices.len())
                    .map(|corner| {
                        // By corner or by point, straight or through an index.
                        let at = if by_corner { corner } else { out.indices[corner] as usize };
                        let k = if direct { at as i64 } else { picks.get(at).copied().unwrap_or(-1) };
                        match usize::try_from(k).ok().and_then(|k| values.get(k * 2..k * 2 + 2)) {
                            Some(v) => [v[0] as f32, v[1] as f32],
                            None => [0.0, 0.0],
                        }
                    })
                    .collect();
                out.uvs.push((layer.kid("Name").map_or(String::new(), |n| n.text(0).to_string()), uv));
            }
            let by_face = geo.kid("LayerElementMaterial");
            let all_same = by_face.and_then(|l| l.kid("MappingInformationType")).is_none_or(|r| r.text(0) != "ByPolygon");
            let picks = by_face.and_then(|l| l.kid("Materials")).map(Rec::ints).unwrap_or_default();
            out.face_material = (0..out.sizes.len())
                .map(|f| {
                    let slot = if all_same { picks.first().copied().unwrap_or(0) } else { picks.get(f).copied().unwrap_or(0) };
                    usize::try_from(slot).ok().and_then(|k| slots.get(k)).copied().unwrap_or(usize::MAX)
                })
                .collect();
            Some(out)
        });
        let kind = m.text(2);
        if matches!(kind, "Camera" | "Light") {
            notes.push(format!("{}: a {} comes in as a transform", m.label(), kind.to_lowercase()));
        }
        nodes.push(ImpNode {
            name: m.label(),
            parent: parent_of.get(&id).and_then(|p| index_of.get(p)).copied(),
            joint: matches!(kind, "LimbNode" | "Limb" | "Root"),
            matrix: to_scene(local_matrix(m)),
            mesh,
        });
    }
    if nodes.is_empty() {
        fail!("importFbx: the file has no models");
    }
    if objects.all("Deformer").next().is_some() {
        notes.push("skin weights are not read: the joints and the meshes came in unbound".to_string());
    }
    if objects.all("AnimationCurve").next().is_some() {
        notes.push("animation is not read".to_string());
    }
    s.graph.open_chunk("Import FBX");
    let made = build(s, nodes, materials);
    s.graph.close_chunk();
    let made = made?;
    for n in notes {
        log::warn!("importFbx: {n}");
    }
    Ok(made)
}

/// The commands the FBX plug-in brings for reading.
pub const COMMANDS: &[&str] = &["importFbx", "FBXImport"];

pub fn register(c: &mut Commands<Session>) {
    let doc = "Import a binary FBX file: its model hierarchy, meshes as polygons with their UV sets, limb nodes as joints, and \
               materials' colours with the faces that wear them. Lengths come in as centimetres and the file's up axis becomes \
               the scene's. ASCII FBX, skin weights, animation, textures, cameras and lights are not read. Returns the top \
               nodes made.";
    c.register(
        CommandDef::<Session>::new("importFbx", doc, |s, a| {
            let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("importFbx: path required") };
            import(s, &path)
        })
        .positional("path"),
    );
    c.register(
        CommandDef::<Session>::new("FBXImport", "Import an FBX file as scripts written for the reference ask: FBXImport -f \"path\".", |s, a| {
            let path = a.string("file", "");
            if path.is_empty() {
                fail!("FBXImport: give -f and a path");
            }
            import(s, &path)
        })
        .flag(FlagDef::new("file", "f", ArgType::String, Value::Null, "The file to read.")),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(name: &str, props: &[u8], count: u32, wide: bool, at: usize) -> Vec<u8> {
        let w = if wide { 8 } else { 4 };
        let end = at + 3 * w + 1 + name.len() + props.len();
        let mut out = Vec::new();
        for v in [end as u64, count as u64, props.len() as u64] {
            if wide {
                out.extend_from_slice(&v.to_le_bytes());
            } else {
                out.extend_from_slice(&(v as u32).to_le_bytes());
            }
        }
        out.push(name.len() as u8);
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(props);
        out
    }

    // Files from other writers pack their arrays and, from version 7500, use 64-bit offsets.
    #[test]
    fn packed_arrays_and_wide_offsets_are_read() {
        let values: Vec<f64> = (0..300).map(|k| k as f64 * 0.5).collect();
        let plain: Vec<u8> = values.iter().flat_map(|v| v.to_le_bytes()).collect();
        let packed = miniz_oxide::deflate::compress_to_vec_zlib(&plain, 6);
        assert!(packed.len() < plain.len());
        let mut props = vec![b'd'];
        props.extend_from_slice(&(values.len() as u32).to_le_bytes());
        props.extend_from_slice(&1u32.to_le_bytes());
        props.extend_from_slice(&(packed.len() as u32).to_le_bytes());
        props.extend_from_slice(&packed);
        // Also a float array, plain, and a short and a flag.
        props.push(b'f');
        props.extend_from_slice(&2u32.to_le_bytes());
        props.extend_from_slice(&0u32.to_le_bytes());
        props.extend_from_slice(&8u32.to_le_bytes());
        props.extend_from_slice(&1.5f32.to_le_bytes());
        props.extend_from_slice(&(-2.0f32).to_le_bytes());
        props.extend_from_slice(&[b'Y', 7, 0, b'C', 1]);
        for (version, wide) in [(7400u32, false), (7700, true)] {
            let mut file = b"Kaydara FBX Binary  \x00\x1a\x00".to_vec();
            file.extend_from_slice(&version.to_le_bytes());
            let rec = record("Vertices", &props, 4, wide, file.len());
            file.extend_from_slice(&rec);
            file.extend(std::iter::repeat_n(0u8, if wide { 25 } else { 13 }));
            let top = parse(&file).unwrap_or_else(|e| panic!("version {version}: {e}"));
            let r = &top.kids[0];
            assert_eq!(r.name, "Vertices");
            assert_eq!(r.props[0], V::Doubles(values.clone()));
            assert_eq!(r.props[1], V::Doubles(vec![1.5, -2.0]));
            assert_eq!((r.props[2].clone(), r.props[3].clone()), (V::I(7), V::I(1)));
        }
        assert!(parse(b"; FBX 7.4.0 project file\nFBXHeaderExtension:  {").unwrap_err().contains("ASCII"));
        assert!(parse(b"not this").is_err());
    }

    #[test]
    fn a_models_place_is_built_from_its_pivots_and_pre_rotation() {
        let p = |name: &str, v: [f64; 3]| Rec { name: "P".into(), props: vec![V::S(name.into()), V::S(String::new()), V::S(String::new()), V::S(String::new()), V::D(v[0]), V::D(v[1]), V::D(v[2])], kids: vec![] };
        let model = Rec {
            name: "Model".into(),
            props: vec![],
            kids: vec![Rec { name: "Properties70".into(), props: vec![], kids: vec![p("Lcl Translation", [1.0, 2.0, 3.0]), p("PreRotation", [0.0, 0.0, 90.0]), p("Lcl Rotation", [0.0, 0.0, 0.0]), p("RotationPivot", [1.0, 0.0, 0.0])] }],
        };
        let m = local_matrix(&model);
        // A quarter turn about a pivot one along X: the origin swings to (1, -1, 0), then the translation.
        assert!((m.transform_point3(DVec3::ZERO) - DVec3::new(2.0, 1.0, 3.0)).length() < 1e-9, "{:?}", m.transform_point3(DVec3::ZERO));
        // The pivot itself stays where it is.
        assert!((m.transform_point3(DVec3::X) - DVec3::new(2.0, 2.0, 3.0)).length() < 1e-9);
    }
}
