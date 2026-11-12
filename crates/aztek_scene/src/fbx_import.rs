//! FBX import: binary FBX files, read from the format's public descriptions with no library of its vendor's, made into
//! the scene as the reference's FBX plug-in makes them.
//!
//! Read: the model hierarchy with each node's channels (translation, rotation in its order, scale, pivots and offsets;
//! pre rotation as a joint's orient and post rotation as its rotate axis), meshes as polygons with their UV sets, colour
//! sets, normals, hard edges and geometric offset, limb nodes as joints, lambert and phong materials with their file
//! textures, skins (a skinCluster with the file's weights and bind matrices) and the bind pose. As the reference, the
//! file's axes and unit are converted on its top nodes: a Z-up file turns its top nodes into a Y-up scene, and a file in
//! metres scales them. The FBXImport settings (fbx_options) decide what comes in. Not read: ASCII FBX, animation, blend
//! shapes, cameras and lights (their nodes come in as transforms).

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::fbx_options as opts;
use crate::import_build::{build_all, Extras, Hardness, ImpMaterial, ImpMesh, ImpNode, ImpSkin, ImpTexture, Parts, TexUse};
use crate::session::Session;
use crate::transform::{euler_to_matrix, matrix_to_euler, RotateOrder};

#[derive(Clone, Debug, PartialEq)]
pub enum V {
    I(i64),
    D(f64),
    S(String),
    Ints(Vec<i64>),
    Doubles(Vec<f64>),
    Raw(Vec<u8>),
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
                    props.push(if code == b'S' { V::S(String::from_utf8_lossy(data).into_owned()) } else { V::Raw(data.to_vec()) });
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

/// The format's rotation order (XYZ, XZY, YZX, YXZ, ZXY, ZYX) as the reference's rotateOrder (xyz, yzx, zxy, xzy, yxz, zyx).
fn rotate_order(fbx: i64) -> i64 {
    match fbx {
        1 => 3,
        2 => 1,
        3 => 4,
        4 => 2,
        5 => 5,
        _ => 0,
    }
}

fn values_in(props: &Rec, name: &str) -> Option<Vec<f64>> {
    let p = props.kids.iter().find(|p| p.text(0) == name)?;
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

fn text_in(props: &Rec, name: &str) -> Option<String> {
    let p = props.kids.iter().find(|p| p.text(0) == name)?;
    p.props.get(4).and_then(|v| if let V::S(s) = v { Some(s.clone()) } else { None })
}

/// The property templates of the file's Definitions: an object's properties fall back on its kind's template.
struct Templates<'a>(HashMap<String, &'a Rec>);

impl<'a> Templates<'a> {
    fn of(top: &'a Rec) -> Self {
        let mut m = HashMap::new();
        if let Some(d) = top.kid("Definitions") {
            for t in d.all("ObjectType") {
                if let Some(p) = t.kid("PropertyTemplate").and_then(|p| p.kid("Properties70")) {
                    m.insert(t.text(0).to_string(), p);
                }
            }
        }
        Templates(m)
    }
    /// A property's numbers: the object's own, else its template's.
    fn get(&self, rec: &Rec, name: &str) -> Option<Vec<f64>> {
        rec.prop(name).or_else(|| self.0.get(&rec.name).and_then(|t| values_in(t, name)))
    }
    fn num(&self, rec: &Rec, name: &str) -> Option<f64> {
        self.get(rec, name).and_then(|v| v.first().copied())
    }
    fn vec(&self, rec: &Rec, name: &str, rest: f64) -> DVec3 {
        v3(self.get(rec, name), rest)
    }
    fn has(&self, rec: &Rec, name: &str) -> bool {
        self.get(rec, name).is_some_and(|v| !v.is_empty())
    }
    fn text(&self, rec: &Rec, name: &str) -> Option<String> {
        rec.kid("Properties70").and_then(|p| text_in(p, name)).or_else(|| self.0.get(&rec.name).and_then(|t| text_in(t, name)))
    }
}

/// A model's channels as the reference keeps them, and the matrix they make. Pre rotation is a joint's orient and post
/// rotation its rotate axis; a transform's pre rotation goes into its rotation, which comes to the same matrix.
fn model_parts(t: &Templates, model: &Rec, joint: bool) -> (Parts, DMat4) {
    let active = t.num(model, "RotationActive").is_none_or(|v| v != 0.0);
    let fbx_order = if active { t.num(model, "RotationOrder").unwrap_or(0.0) as i64 } else { 0 };
    let order_ix = rotate_order(fbx_order);
    let order = RotateOrder::from_index(order_ix);
    let lcl = turn(t.vec(model, "Lcl Rotation", 0.0), order);
    let pre = if active { turn(t.vec(model, "PreRotation", 0.0), RotateOrder::Xyz) } else { DMat4::IDENTITY };
    let post = if active { turn(t.vec(model, "PostRotation", 0.0), RotateOrder::Xyz) } else { DMat4::IDENTITY };
    let rad = |m: &DMat4, o: RotateOrder| matrix_to_euler(&DMat3::from_mat4(*m), o);
    let inherit = t.num(model, "InheritType").unwrap_or(1.0) as i64;
    let mut p = Parts {
        translate: t.vec(model, "Lcl Translation", 0.0),
        rotate_order: order_ix,
        rotate_pivot: t.vec(model, "RotationPivot", 0.0),
        rotate_pivot_translate: t.vec(model, "RotationOffset", 0.0),
        scale_pivot: t.vec(model, "ScalingPivot", 0.0),
        scale_pivot_translate: t.vec(model, "ScalingOffset", 0.0),
        scale: t.vec(model, "Lcl Scaling", 1.0),
        rotate_axis: rad(&post.inverse(), RotateOrder::Xyz),
        segment_scale_compensate: joint.then_some(inherit == 2),
        ..Default::default()
    };
    if joint {
        p.joint_orient = rad(&pre, RotateOrder::Xyz);
        p.rotate = rad(&lcl, order);
    } else {
        p.rotate = rad(&(pre * lcl), order);
    }
    (p.clone(), parts_matrix(&p, joint))
}

/// The matrix a node's channels make: T Rpt Rp JO R RA Rp⁻¹ Spt Sp S Sp⁻¹.
pub(crate) fn parts_matrix(p: &Parts, joint: bool) -> DMat4 {
    let jo = if joint { euler_to_matrix(p.joint_orient, RotateOrder::Xyz) } else { DMat4::IDENTITY };
    let r = euler_to_matrix(p.rotate, RotateOrder::from_index(p.rotate_order));
    let ra = euler_to_matrix(p.rotate_axis, RotateOrder::Xyz);
    let tr = DMat4::from_translation;
    tr(p.translate) * tr(p.rotate_pivot_translate) * tr(p.rotate_pivot) * jo * r * ra * tr(-p.rotate_pivot) * tr(p.scale_pivot_translate) * tr(p.scale_pivot) * DMat4::from_scale(p.scale) * tr(-p.scale_pivot)
}

/// A top node turned and scaled by `k` (a rotation and an even scale): the change goes into its translation, rotation
/// and scale, as the reference's axis and unit conversion turns and scales the top nodes. None where its pivots stand in
/// the way: its matrix is taken apart instead.
pub(crate) fn convert_top(p: &Parts, joint: bool, k: DMat4) -> Option<Parts> {
    let zero = |v: DVec3| v.length() < 1e-12;
    if !(zero(p.rotate_pivot) && zero(p.scale_pivot) && zero(p.rotate_pivot_translate) && zero(p.scale_pivot_translate)) {
        return None;
    }
    let u = k.x_axis.truncate().length();
    let c = DMat4::from_mat3(DMat3::from_mat4(k) * (1.0 / u));
    let jo = if joint { euler_to_matrix(p.joint_orient, RotateOrder::Xyz) } else { DMat4::IDENTITY };
    let order = RotateOrder::from_index(p.rotate_order);
    let r = jo.inverse() * c * jo * euler_to_matrix(p.rotate, order);
    let mut out = p.clone();
    out.translate = k.transform_point3(p.translate);
    out.rotate = matrix_to_euler(&DMat3::from_mat4(r), order);
    out.scale = p.scale * u;
    Some(out)
}

/// A layer's values, one a corner: by corner, by point, by face or one for all, straight or through an index.
fn layer_corners(layer: &Rec, data: &str, index: &str, width: usize, indices: &[u32], face_of: &[usize]) -> Vec<Option<Vec<f64>>> {
    let values = layer.kid(data).map(Rec::doubles).unwrap_or_default();
    let mapping = layer.kid("MappingInformationType").map_or("ByPolygonVertex", |r| r.text(0));
    let direct = layer.kid("ReferenceInformationType").is_none_or(|r| r.text(0) == "Direct");
    let picks = layer.kid(index).map(Rec::ints).unwrap_or_default();
    (0..indices.len())
        .map(|corner| {
            let at = match mapping {
                "ByVertice" | "ByVertex" | "ByControlPoint" => indices[corner] as usize,
                "ByPolygon" => face_of[corner],
                "AllSame" => 0,
                _ => corner,
            };
            let k = if direct { at as i64 } else { picks.get(at).copied().unwrap_or(-1) };
            usize::try_from(k).ok().and_then(|k| values.get(k * width..k * width + width)).map(<[f64]>::to_vec)
        })
        .collect()
}

/// The takes in a file: (name, start, stop) in seconds.
pub fn takes(top: &Rec) -> Vec<(String, f64, f64)> {
    const KTIME: f64 = 46_186_158_000.0;
    let Some(objects) = top.kid("Objects") else { return Vec::new() };
    objects
        .all("AnimationStack")
        .map(|st| {
            let t = |n: &str| st.prop(n).and_then(|v| v.first().copied()).unwrap_or(0.0) / KTIME;
            (st.label(), t("LocalStart"), t("LocalStop"))
        })
        .collect()
}

/// Where a texture's file is: its own path, its relative path from the FBX file's folder, the `<name>.fbm` folder beside
/// the FBX file, then the FBX file's folder. The path as the file has it when none of these is there.
fn texture_path(file: &str, relative: &str, fbx: &Path) -> String {
    let slash = |p: &str| p.replace('\\', "/");
    let (file, relative) = (slash(file), slash(relative));
    let dir = fbx.parent().unwrap_or(Path::new("."));
    let stem = fbx.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let base = Path::new(if file.is_empty() { &relative } else { &file }).file_name().map(|b| b.to_string_lossy().into_owned()).unwrap_or_default();
    let mut tries: Vec<PathBuf> = Vec::new();
    if !file.is_empty() {
        tries.push(PathBuf::from(&file));
    }
    if !relative.is_empty() && !Path::new(&relative).is_absolute() && !relative.contains(':') {
        tries.push(dir.join(&relative));
    }
    if !base.is_empty() {
        tries.push(dir.join(format!("{stem}.fbm")).join(&base));
        tries.push(dir.join(&base));
    }
    for t in &tries {
        if t.is_file() {
            return aztek_core::paths::to_utf8(&aztek_core::paths::normalize(t));
        }
    }
    if file.is_empty() { relative } else { file }
}

/// Writes a texture the file carries inside it to the `<name>.fbm` folder beside the FBX file, as the reference does,
/// unless it is there already. Returns where it is.
fn extract_media(content: &[u8], name: &str, fbx: &Path) -> Option<String> {
    if content.is_empty() || name.is_empty() {
        return None;
    }
    let dir = fbx.parent().unwrap_or(Path::new("."));
    let stem = fbx.file_stem()?.to_string_lossy().into_owned();
    let folder = dir.join(format!("{stem}.fbm"));
    let out = folder.join(Path::new(&name.replace('\\', "/")).file_name()?);
    if !out.is_file() {
        std::fs::create_dir_all(&folder).ok()?;
        std::fs::write(&out, content).ok()?;
    }
    Some(aztek_core::paths::to_utf8(&out))
}

/// The columns of an axis system: its right, up and front, from the file's settings.
fn axes(up: i64, up_sign: f64, front: i64, front_sign: f64, coord: i64, coord_sign: f64) -> DMat3 {
    let unit = |k: i64, s: f64| {
        let mut v = DVec3::ZERO;
        v[k.clamp(0, 2) as usize] = if s < 0.0 { -1.0 } else { 1.0 };
        v
    };
    DMat3::from_cols(unit(coord, coord_sign), unit(up, up_sign), unit(front, front_sign))
}

/// The reference's own axis systems: Y up (front +Z) and Z up (front -Y), both with +X to the right.
fn host_axes(z_up: bool) -> DMat3 {
    if z_up {
        axes(2, 1.0, 1, -1.0, 0, 1.0)
    } else {
        axes(1, 1.0, 2, 1.0, 0, 1.0)
    }
}

/// The turn from one of the reference's axis systems to the other.
pub(crate) fn axis_change(from_z: bool, to_z: bool) -> DMat3 {
    host_axes(to_z) * host_axes(from_z).inverse()
}

/// What a texture into a material's property becomes: the material's attribute and how it feeds it.
fn texture_use(prop: &str, phong: bool, bump: f64) -> Option<(&'static str, TexUse)> {
    Some(match prop {
        "DiffuseColor" | "Diffuse" => ("color", TexUse::Color),
        "DiffuseFactor" => ("diffuse", TexUse::Alpha),
        "TransparentColor" | "TransparencyFactor" => ("transparency", TexUse::Transparency),
        "AmbientColor" | "Ambient" => ("ambientColor", TexUse::Color),
        "EmissiveColor" | "Emissive" => ("incandescence", TexUse::Color),
        "NormalMap" => ("normalCamera", TexUse::Normal),
        "Bump" => ("normalCamera", TexUse::Bump(bump)),
        "SpecularColor" | "Specular" if phong => ("specularColor", TexUse::Color),
        "ShininessExponent" | "Shininess" if phong => ("cosinePower", TexUse::Alpha),
        "ReflectionColor" if phong => ("reflectedColor", TexUse::Color),
        "ReflectionFactor" if phong => ("reflectivity", TexUse::Alpha),
        _ => return None,
    })
}

pub fn import(s: &mut Session, path: &str) -> CmdResult {
    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => fail!("FBXImport: can't read {path}"),
    };
    let top = match parse(&bytes) {
        Ok(t) => t,
        Err(e) => fail!("FBXImport: {e}"),
    };
    let fbx_path = Path::new(path);
    let Some(objects) = top.kid("Objects") else { fail!("FBXImport: the file has no objects") };
    let tp = Templates::of(&top);
    let mode = opts::word("FBXImportMode");
    let mut notes: Vec<String> = Vec::new();

    // The file's unit in centimetres, and its axes.
    let settings = top.kid("GlobalSettings");
    let setting = |name: &str, d: f64| settings.and_then(|g| g.prop(name)).and_then(|v| v.first().copied()).unwrap_or(d);
    let file_axes = match opts::word("FBXImportForcedFileAxis").as_str() {
        "y" => host_axes(false),
        "z" => host_axes(true),
        _ => axes(setting("UpAxis", 1.0) as i64, setting("UpAxisSign", 1.0), setting("FrontAxis", 2.0) as i64, setting("FrontAxisSign", 1.0), setting("CoordAxis", 0.0) as i64, setting("CoordAxisSign", 1.0)),
    };
    let target_z = match opts::word("FBXImportUpAxis").as_str() {
        "z" => true,
        "y" => false,
        _ => crate::world::up_z(),
    };
    let mut turn_by = DMat3::IDENTITY;
    if opts::on("FBXImportAxisConversionEnable") {
        let c = host_axes(target_z) * file_axes.inverse();
        if c.determinant() > 0.0 {
            turn_by = c;
        } else {
            notes.push("the file's axes are of the other handedness: they are not converted".to_string());
        }
    }
    let factor = if opts::on("FBXImportScaleFactorEnable") { opts::number("FBXImportScaleFactor") } else { setting("UnitScaleFactor", 1.0) };
    let factor = if factor > 0.0 { factor } else { 1.0 };
    // What the top nodes are turned and scaled by, and every world matrix the file holds with them.
    let k = DMat4::from_mat3(turn_by * factor);

    // What is linked under what, in the file's order, and what textures feed which property.
    let mut under: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut parent_of: HashMap<i64, i64> = HashMap::new();
    let mut feeds: Vec<(i64, i64, String)> = Vec::new();
    for c in top.kid("Connections").map(|c| c.kids.as_slice()).unwrap_or(&[]) {
        match c.text(0) {
            "OO" => {
                under.entry(c.int(2)).or_default().push(c.int(1));
                parent_of.entry(c.int(1)).or_insert(c.int(2));
            }
            "OP" => feeds.push((c.int(1), c.int(2), c.text(3).to_string())),
            _ => {}
        }
    }
    let below = |id: i64| under.get(&id).cloned().unwrap_or_default();
    let models: Vec<&Rec> = objects.all("Model").collect();
    let index_of: HashMap<i64, usize> = models.iter().enumerate().map(|(k, m)| (m.int(0), k)).collect();
    let geometries: HashMap<i64, &Rec> = objects.all("Geometry").filter(|g| g.text(2) == "Mesh").map(|g| (g.int(0), g)).collect();
    let attributes: HashMap<i64, &Rec> = objects.all("NodeAttribute").map(|a| (a.int(0), a)).collect();
    let deformers: HashMap<i64, &Rec> = objects.all("Deformer").map(|d| (d.int(0), d)).collect();

    // Skins: the geometry a skin deforms, its clusters, and the model each cluster follows.
    let skins_on = opts::on("FBXImportSkins");
    let mut deforming: Vec<i64> = Vec::new();
    for d in deformers.values().filter(|d| d.text(2) == "Cluster") {
        for m in below(d.int(0)) {
            if index_of.contains_key(&m) {
                deforming.push(m);
            }
        }
    }

    // Materials.
    let material_recs: Vec<&Rec> = objects.all("Material").collect();
    let material_index: HashMap<i64, usize> = material_recs.iter().enumerate().map(|(k, m)| (m.int(0), k)).collect();
    let textures: HashMap<i64, &Rec> = objects.all("Texture").map(|t| (t.int(0), t)).collect();
    let videos: HashMap<i64, &Rec> = objects.all("Video").map(|v| (v.int(0), v)).collect();
    let mut materials: Vec<ImpMaterial> = Vec::with_capacity(material_recs.len());
    for m in &material_recs {
        let phong = m.kid("ShadingModel").is_some_and(|k| k.text(0).eq_ignore_ascii_case("phong")) || tp.text(m, "ShadingModel").is_some_and(|t| t.eq_ignore_ascii_case("phong"));
        let colour = |names: &[&str], factor: &str| -> Option<[f64; 3]> {
            let c = names.iter().find_map(|n| tp.get(m, n).filter(|v| v.len() >= 3))?;
            let f = tp.num(m, factor).unwrap_or(1.0);
            Some([c[0] * f, c[1] * f, c[2] * f])
        };
        let c = names_colour(&tp, m, &["DiffuseColor", "Diffuse"]).unwrap_or([0.5, 0.5, 0.5]);
        let mut im = ImpMaterial { name: m.label(), color: c, ty: Some(if phong { "phong" } else { "lambert" }.to_string()), ..Default::default() };
        if let Some(f) = tp.num(m, "DiffuseFactor") {
            im.attrs.push(("diffuse".into(), vec![f]));
        }
        if tp.has(m, "TransparentColor") || tp.has(m, "TransparencyFactor") {
            let t = colour(&["TransparentColor"], "TransparencyFactor").unwrap_or([0.0; 3]);
            im.attrs.push(("transparency".into(), t.to_vec()));
        } else if let Some(o) = tp.num(m, "Opacity") {
            im.transparency = (1.0 - o).clamp(0.0, 1.0);
        }
        if let Some(a) = colour(&["AmbientColor", "Ambient"], "AmbientFactor") {
            im.attrs.push(("ambientColor".into(), a.to_vec()));
        }
        if let Some(e) = colour(&["EmissiveColor", "Emissive"], "EmissiveFactor") {
            im.attrs.push(("incandescence".into(), e.to_vec()));
        }
        if phong {
            if let Some(sp) = colour(&["SpecularColor", "Specular"], "SpecularFactor") {
                im.attrs.push(("specularColor".into(), sp.to_vec()));
            }
            if let Some(x) = tp.num(m, "ShininessExponent").or_else(|| tp.num(m, "Shininess")) {
                im.attrs.push(("cosinePower".into(), vec![x.clamp(2.0, 100.0)]));
            }
            if let Some(x) = tp.num(m, "ReflectionFactor") {
                im.attrs.push(("reflectivity".into(), vec![x]));
            }
            if let Some(rc) = colour(&["ReflectionColor"], "__none__") {
                im.attrs.push(("reflectedColor".into(), rc.to_vec()));
            }
        }
        let bump = tp.num(m, "BumpFactor").unwrap_or(1.0);
        for (tex, _, prop) in feeds.iter().filter(|f| f.1 == m.int(0)) {
            let Some(t) = textures.get(tex) else {
                if objects.all("LayeredTexture").any(|l| l.int(0) == *tex) {
                    notes.push(format!("{}: layered textures are not read", m.label()));
                }
                continue;
            };
            let Some((attr, usage)) = texture_use(prop, phong, bump) else {
                notes.push(format!("{}: a texture into {prop} is not read", m.label()));
                continue;
            };
            let file = t.kid("FileName").map(|r| r.text(0).to_string()).unwrap_or_default();
            let relative = t.kid("RelativeFilename").map(|r| r.text(0).to_string()).unwrap_or_default();
            // Media inside the file are written out beside it first.
            let embedded = below(t.int(0)).iter().filter_map(|v| videos.get(v)).find_map(|v| match v.kid("Content").and_then(|c| c.props.first()) {
                Some(V::Raw(b)) if !b.is_empty() => extract_media(b, if relative.is_empty() { &file } else { &relative }, fbx_path),
                _ => None,
            });
            let place = |n: &str, rest: f64| tp.vec(t, n, rest);
            let (tr, ro, sc) = (place("Translation", 0.0), place("Rotation", 0.0), place("Scaling", 1.0));
            im.textures.push(ImpTexture {
                name: t.label(),
                key: *tex,
                path: embedded.unwrap_or_else(|| texture_path(&file, &relative, fbx_path)),
                attr: attr.to_string(),
                usage,
                repeat: [sc.x, sc.y],
                offset: [tr.x, tr.y],
                rotate: ro.z,
                wrap: [tp.num(t, "WrapModeU").unwrap_or(0.0) == 0.0, tp.num(t, "WrapModeV").unwrap_or(0.0) == 0.0],
            });
        }
        materials.push(im);
    }

    // Nodes.
    let cameras = opts::on("FBXImportCameras");
    let lights = opts::on("FBXImportLights");
    let to_joint = opts::on("FBXImportConvertDeformingNullsToJoint");
    let kind_of = |m: &Rec| -> String {
        let own = m.text(2).to_string();
        if own != "Null" && !own.is_empty() {
            return own;
        }
        below(m.int(0)).iter().filter_map(|a| attributes.get(a)).map(|a| a.text(2).to_string()).find(|t| !t.is_empty()).unwrap_or(own)
    };
    let keep: Vec<bool> = models
        .iter()
        .map(|m| match kind_of(m).as_str() {
            "Camera" | "CameraSwitcher" => cameras,
            "Light" => lights,
            _ => true,
        })
        .collect();
    let skipped = keep.iter().filter(|k| !**k).count();
    if skipped > 0 {
        notes.push(format!("{skipped} cameras and lights left out, as the settings say"));
    }
    // A node's parent: the nearest model above it that is kept.
    let parent_index = |id: i64| -> Option<usize> {
        let mut at = id;
        loop {
            let p = *parent_of.get(&at)?;
            let k = *index_of.get(&p)?;
            if keep[k] {
                return Some(k);
            }
            at = p;
        }
    };
    let mut nodes: Vec<ImpNode> = Vec::new();
    let mut node_of: HashMap<i64, usize> = HashMap::new();
    let mut parts_of: Vec<Parts> = Vec::new();
    let mut skinned_geometry: Vec<i64> = Vec::new();
    if skins_on {
        for d in deformers.values().filter(|d| d.text(2) == "Skin") {
            if let Some(g) = parent_of.get(&d.int(0)) {
                skinned_geometry.push(*g);
            }
        }
    }
    let unlock = opts::on("FBXImportUnlockNormals");
    let smoothing = opts::on("FBXImportSmoothingGroups");
    for (mk, m) in models.iter().enumerate() {
        if !keep[mk] {
            continue;
        }
        let id = m.int(0);
        let kind = kind_of(m);
        let joint = matches!(kind.as_str(), "LimbNode" | "Limb" | "Root") || (to_joint && skins_on && deforming.contains(&id) && kind == "Null");
        if matches!(kind.as_str(), "Camera" | "Light") {
            notes.push(format!("{}: a {} comes in as a transform", m.label(), kind.to_lowercase()));
        }
        let below_m = below(id);
        let slots: Vec<usize> = below_m.iter().filter_map(|c| material_index.get(c).copied()).collect();
        let geo_id = below_m.iter().find(|c| geometries.contains_key(c)).copied();
        let mesh = geo_id.and_then(|gid| {
            let geo = geometries[&gid];
            let vertices = geo.kid("Vertices")?.doubles();
            let index = geo.kid("PolygonVertexIndex")?.ints();
            // A mesh's offset from its node, which children do not inherit, goes into its points.
            let offset = DMat4::from_translation(tp.vec(m, "GeometricTranslation", 0.0)) * turn(tp.vec(m, "GeometricRotation", 0.0), RotateOrder::Xyz) * DMat4::from_scale(tp.vec(m, "GeometricScaling", 1.0));
            let normal_turn = DMat3::from_mat4(offset).inverse().transpose();
            let mut out = ImpMesh { points: vertices.chunks_exact(3).map(|p| offset.transform_point3(DVec3::new(p[0], p[1], p[2]))).collect(), ..Default::default() };
            let mut size = 0u32;
            let mut face_of = Vec::with_capacity(index.len());
            for &i in &index {
                size += 1;
                face_of.push(out.sizes.len());
                // The complement closes a polygon.
                out.indices.push(if i < 0 { (-i - 1) as u32 } else { i as u32 });
                if i < 0 {
                    out.sizes.push(size);
                    size = 0;
                }
            }
            for layer in geo.all("LayerElementUV") {
                let uv = layer_corners(layer, "UV", "UVIndex", 2, &out.indices, &face_of).into_iter().map(|v| v.map_or([0.0, 0.0], |v| [v[0] as f32, v[1] as f32])).collect();
                out.uvs.push((layer.kid("Name").map_or(String::new(), |n| n.text(0).to_string()), uv));
            }
            for layer in geo.all("LayerElementColor") {
                let c = layer_corners(layer, "Colors", "ColorIndex", 4, &out.indices, &face_of).into_iter().map(|v| v.map_or([1.0; 4], |v| [v[0] as f32, v[1] as f32, v[2] as f32, v[3] as f32])).collect();
                out.colors.push((layer.kid("Name").map_or(String::new(), |n| n.text(0).to_string()), c));
            }
            if let Some(layer) = geo.all("LayerElementNormal").next() {
                let n: Vec<[f32; 3]> = layer_corners(layer, "Normals", "NormalsIndex", 3, &out.indices, &face_of)
                    .into_iter()
                    .map(|v| v.map_or([0.0, 1.0, 0.0], |v| (normal_turn * DVec3::new(v[0], v[1], v[2])).normalize_or_zero().as_vec3().to_array()))
                    .collect();
                out.normals = Some(n);
                out.keep_normals = !unlock && !skinned_geometry.contains(&gid);
                out.hardness = Some(Hardness::FromNormals);
            }
            if smoothing {
                if let Some(layer) = geo.all("LayerElementSmoothing").next() {
                    let values = layer.kid("Smoothing").map(Rec::ints).unwrap_or_default();
                    match layer.kid("MappingInformationType").map_or("", |r| r.text(0)) {
                        "ByPolygon" => out.hardness = Some(Hardness::Groups(values)),
                        "ByEdge" => {
                            // An edge starts at a corner; 0 is hard.
                            let edges = geo.kid("Edges").map(Rec::ints).unwrap_or_default();
                            let mut hard = vec![false; out.indices.len()];
                            for (e, &corner) in edges.iter().enumerate() {
                                if let Some(h) = usize::try_from(corner).ok().and_then(|c| hard.get_mut(c)) {
                                    *h = values.get(e).copied().unwrap_or(1) == 0;
                                }
                            }
                            out.hardness = Some(Hardness::ByCorner(hard));
                        }
                        _ => {}
                    }
                }
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
        let parent = parent_index(id);
        let (mut parts, mut matrix) = model_parts(&tp, m, joint);
        let mut use_parts = true;
        if parent.is_none() && k != DMat4::IDENTITY {
            match convert_top(&parts, joint, k) {
                Some(p) => {
                    parts = p;
                    matrix = parts_matrix(&parts, joint);
                }
                None => {
                    matrix = k * matrix;
                    use_parts = false;
                }
            }
        }
        node_of.insert(id, nodes.len());
        parts_of.push(parts.clone());
        nodes.push(ImpNode { name: m.label(), parent: None, joint, matrix, mesh, parts: use_parts.then_some(parts), existing: None });
    }
    // Parents by the nodes' own order.
    for (mk, m) in models.iter().enumerate() {
        if !keep[mk] {
            continue;
        }
        let me = node_of[&m.int(0)];
        nodes[me].parent = parent_index(m.int(0)).and_then(|p| node_of.get(&models[p].int(0)).copied());
    }
    if nodes.is_empty() {
        fail!("FBXImport: the file has no models");
    }

    // Add and update: what the scene has already by the same name, in the same place, is not made again.
    if mode != "add" {
        let mut order: Vec<usize> = (0..nodes.len()).collect();
        order.sort_by_key(|&i| depth(&nodes, i));
        for i in order {
            let name = crate::import_build::clean_name(&nodes[i].name, "");
            if name.is_empty() {
                continue;
            }
            let wanted = match nodes[i].parent {
                None => format!("|{name}"),
                Some(p) => match nodes[p].existing {
                    Some(pid) => format!("{}|{name}", s.graph.path(pid)),
                    None => continue,
                },
            };
            if let Ok(Some(id)) = s.graph.find(&wanted) {
                let same = s.graph.node(id).is_some_and(|n| (n.ty().name == "joint") == nodes[i].joint);
                if same {
                    nodes[i].existing = Some(id);
                }
            }
        }
        if mode == "exmerge" {
            notes.push("Update animation: only animation would change what is in the scene, and animation is not read".to_string());
            return Ok(json!([]));
        }
    }

    // Skins.
    let mut extras = Extras::default();
    if skins_on {
        for (mk, m) in models.iter().enumerate() {
            if !keep[mk] {
                continue;
            }
            let Some(&me) = node_of.get(&m.int(0)) else { continue };
            let Some(gid) = below(m.int(0)).into_iter().find(|c| geometries.contains_key(c)) else { continue };
            let points = nodes[me].mesh.as_ref().map_or(0, |x| x.points.len());
            for skin in below(gid).iter().filter_map(|d| deformers.get(d)).filter(|d| d.text(2) == "Skin") {
                let mut influences = Vec::new();
                let mut bind_pre = Vec::new();
                let mut geom = None;
                let mut weights: Vec<Vec<(u32, f64)>> = vec![Vec::new(); points];
                for cl in below(skin.int(0)).iter().filter_map(|c| deformers.get(c)).filter(|c| c.text(2) == "Cluster") {
                    let Some(link) = below(cl.int(0)).into_iter().find_map(|x| node_of.get(&x).copied()) else { continue };
                    let mat = |name: &str| cl.kid(name).map(Rec::doubles).filter(|v| v.len() == 16).map(|v| DMat4::from_cols_array(&v.try_into().unwrap()));
                    let inf = influences.len() as u32;
                    influences.push(link);
                    bind_pre.push((k * mat("TransformLink").unwrap_or(DMat4::IDENTITY)).inverse());
                    if geom.is_none() {
                        geom = mat("Transform").map(|t| k * t);
                    }
                    let idx = cl.kid("Indexes").map(Rec::ints).unwrap_or_default();
                    let wts = cl.kid("Weights").map(Rec::doubles).unwrap_or_default();
                    for (i, w) in idx.iter().zip(wts.iter()) {
                        if let Some(p) = usize::try_from(*i).ok().and_then(|i| weights.get_mut(i)) {
                            if *w != 0.0 {
                                p.push((inf, *w));
                            }
                        }
                    }
                }
                if influences.is_empty() {
                    continue;
                }
                // The joints a skin is bound to are joints.
                for &i in &influences {
                    if !nodes[i].joint && to_joint {
                        nodes[i].joint = true;
                    }
                }
                extras.skins.push(ImpSkin { mesh: me, influences, bind_pre, geom: geom.unwrap_or(DMat4::IDENTITY), weights, normalize: opts::on("FBXImportPreNormalizeWeights") });
            }
        }
        for pose in objects.all("Pose").filter(|p| p.text(2) == "BindPose") {
            for pn in pose.all("PoseNode") {
                let id = pn.kid("Node").map_or(0, |n| n.int(0));
                let Some(&me) = node_of.get(&id) else { continue };
                let Some(m) = pn.kid("Matrix").map(Rec::doubles).filter(|v| v.len() == 16) else { continue };
                if !extras.bind_pose.iter().any(|x| x.0 == me) {
                    extras.bind_pose.push((me, k * DMat4::from_cols_array(&m.try_into().unwrap())));
                }
            }
        }
    } else if objects.all("Deformer").any(|d| d.text(2) == "Skin") {
        notes.push("skins left out, as FBXImportSkins says".to_string());
    }
    if objects.all("Deformer").any(|d| d.text(2) == "BlendShape") {
        notes.push("blend shapes are not read".to_string());
    }
    if objects.all("AnimationCurve").next().is_some() {
        notes.push("animation is not read".to_string());
    }
    s.graph.open_chunk("Import FBX");
    let made = build_all(s, nodes, materials, extras);
    s.graph.close_chunk();
    let made = made?;
    for n in notes {
        log::warn!("FBXImport: {n}");
    }
    Ok(made)
}

fn names_colour(tp: &Templates, m: &Rec, names: &[&str]) -> Option<[f64; 3]> {
    let c = names.iter().find_map(|n| tp.get(m, n).filter(|v| v.len() >= 3))?;
    Some([c[0], c[1], c[2]])
}

fn depth(nodes: &[ImpNode], mut i: usize) -> usize {
    let mut d = 0;
    while let Some(p) = nodes[i].parent {
        d += 1;
        i = p;
        if d > nodes.len() {
            break;
        }
    }
    d
}

/// The commands the FBX plug-in brings for reading.
pub const COMMANDS: &[&str] = &["importFbx", "FBXImport"];

pub fn register(c: &mut Commands<Session>) {
    let doc = "Import a binary FBX file as the reference's FBX plug-in does, by the FBXImport settings: the model hierarchy \
               with each node's channels (pre rotation as a joint's orient), meshes as polygons with UV sets, colour sets, \
               normals and hard edges, limb nodes as joints, lambert and phong materials with their file textures (place2dTexture \
               each, normal maps through a bump2d), skins with the file's weights and bind matrices, and the bind pose. The \
               file's axes and unit turn and scale its top nodes. Textures are looked for where the file says, beside it and \
               in its .fbm folder; textures inside the file are written to the .fbm folder. Not read: ASCII FBX, animation, \
               blend shapes, cameras and lights (their nodes come in as transforms). Returns the top nodes made.";
    c.register(
        CommandDef::<Session>::new("importFbx", doc, |s, a| {
            let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("importFbx: path required") };
            import(s, &path)
        })
        .positional("path"),
    );
    c.register(
        CommandDef::<Session>::new("FBXImport", "Import an FBX file by the FBXImport settings: FBXImport -f \"path\" [-t take].", |s, a| {
            let path = a.string("file", "");
            if path.is_empty() {
                fail!("FBXImport: give -f and a path");
            }
            import(s, &path)
        })
        .flag(FlagDef::new("file", "f", ArgType::String, Value::Null, "The file to read."))
        .flag(FlagDef::new("take", "t", ArgType::Int, Value::Null, "The take: 0 none, -1 the last.")),
    );
    crate::fbx_options::register(c);
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
        let (_, m) = model_parts(&Templates(HashMap::new()), &model, false);
        // A quarter turn about a pivot one along X: the origin swings to (1, -1, 0), then the translation.
        assert!((m.transform_point3(DVec3::ZERO) - DVec3::new(2.0, 1.0, 3.0)).length() < 1e-9, "{:?}", m.transform_point3(DVec3::ZERO));
        // The pivot itself stays where it is.
        assert!((m.transform_point3(DVec3::X) - DVec3::new(2.0, 2.0, 3.0)).length() < 1e-9);
    }
}
