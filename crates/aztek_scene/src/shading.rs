//! Materials and textures: the reference's shading nodes, shading groups and who belongs to them.
//!
//! A shadingEngine is an objectSet. Its member list says which shapes, or which of a mesh's faces, it shades; the list
//! follows direct edits and undo as any set's does. It is mirrored in connections as the reference has them, shape.instObjGroups[0]
//! or shape.instObjGroups[0].objectGroups[k] into SG.dagSetMembers, so listConnections and connectAttr see what they see
//! in the reference. Only one shading group holds a given face, also as in the reference.
//!
//! Colours are scene linear. The viewport reads materials through `draw_materials` and `face_groups`.

use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Value};

use crate::components::ComponentMode;
use crate::graph::{Graph, SelectMode};
use crate::ref_core::extend;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::object_sets::{self as os, Member};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::DagNode;

pub const SG_TYPE: &str = "shadingEngine";
pub const INITIAL_SG: &str = "initialShadingGroup";
pub const DEFAULT_MATERIAL: &str = "lambert1";
pub const INITIAL_INFO: &str = "initialMaterialInfo";

const IN: u32 = flags::INPUT | flags::STORABLE | flags::KEYABLE;
const STORE: u32 = flags::INPUT | flags::STORABLE;
const OUT: u32 = flags::OUTPUT;

// ---------------------------------------------------------------------------------------------------------------------
// Attribute helpers

fn base(name: &str, short: &str, ty: PlugType, default: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: default, flags: fl, ..Default::default() }
}
fn fl(name: &str, short: &str, d: f64) -> AttrDef {
    base(name, short, PlugType::Float, PlugValue::Float(d), IN)
}
fn fl01(name: &str, short: &str, d: f64) -> AttrDef {
    let mut a = fl(name, short, d);
    a.soft_min = Some(0.0);
    a.soft_max = Some(1.0);
    a
}
fn fout(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Float, PlugValue::Float(0.0), OUT)
}
fn on(name: &str, short: &str, d: bool) -> AttrDef {
    base(name, short, PlugType::Bool, PlugValue::Bool(d), STORE)
}
fn text(name: &str, short: &str, d: &str) -> AttrDef {
    base(name, short, PlugType::String, PlugValue::String(d.into()), STORE)
}
fn message() -> AttrDef {
    base("message", "msg", PlugType::Int, PlugValue::Int(0), OUT | flags::HIDDEN | flags::MESSAGE)
}
/// A connection point of no value: its elements only ever hold connections.
fn slots(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::ARRAY | flags::HIDDEN)
}
/// A colour: child names as the reference's, which don't all follow the parent's short name.
fn rgb(name: &str, short: &str, kids: [&str; 3], d: [f64; 3], fl: u32) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::from_array(d)), fl | flags::COLOR);
    for (i, c) in ["R", "G", "B"].iter().enumerate() {
        a.child_names[i] = format!("{name}{c}");
        a.child_short[i] = kids[i].to_string();
    }
    a
}
fn col(name: &str, short: &str, kids: [&str; 3], d: f64) -> AttrDef {
    rgb(name, short, kids, [d; 3], IN)
}
fn col_out(name: &str, short: &str, kids: [&str; 3]) -> AttrDef {
    rgb(name, short, kids, [0.0; 3], OUT)
}
/// Two floats, held in a Vec3 whose third child has no name.
fn pair(name: &str, short: &str, kids: [(&str, &str); 2], d: [f64; 2], fl: u32) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::new(d[0], d[1], 0.0)), fl);
    for (i, (l, s)) in kids.iter().enumerate() {
        a.child_names[i] = l.to_string();
        a.child_short[i] = s.to_string();
    }
    a
}
fn vector(name: &str, short: &str, kids: [&str; 3], fl: u32) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::ZERO), fl);
    for (i, c) in ["X", "Y", "Z"].iter().enumerate() {
        a.child_names[i] = format!("{name}{c}");
        a.child_short[i] = kids[i].to_string();
    }
    a
}

/// instObjGroups on a shape: what a shading group's dagSetMembers takes when it shades the whole shape.
pub fn inst_obj_groups() -> AttrDef {
    base("instObjGroups", "iog", PlugType::Int, PlugValue::Int(0), OUT | flags::ARRAY | flags::HIDDEN)
}
/// instObjGroups[0].objectGroups: the same for some of the faces. The instance's index is not kept.
pub fn object_groups() -> AttrDef {
    base("instObjGroups.objectGroups", "iog.og", PlugType::Int, PlugValue::Int(0), OUT | flags::ARRAY | flags::HIDDEN)
}

// ---------------------------------------------------------------------------------------------------------------------
// Node types

type Compute = fn(&mut NodeData);

struct Shade(Compute);

impl NodeBehavior for Shade {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        (self.0)(d)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn at(d: &NodeData, n: &str) -> usize {
    d.ty().attr_index(n).unwrap_or_else(|| panic!("{} has no {n}", d.ty().name))
}
fn v(d: &NodeData, n: &str) -> DVec3 {
    d.vec3(at(d, n))
}
fn x(d: &NodeData, n: &str) -> f64 {
    d.value(at(d, n)).scalar().unwrap_or(0.0)
}
fn put(d: &mut NodeData, n: &str, val: impl Into<PlugValue>) {
    let a = at(d, n);
    d.set_output(a, val);
}

fn no_compute(_: &mut NodeData) {}

fn pass_color(d: &mut NodeData) {
    let (c, t) = (v(d, "color"), v(d, "transparency"));
    put(d, "outColor", c);
    put(d, "outTransparency", t);
}

fn standard_surface(d: &mut NodeData) {
    let c = v(d, "baseColor") * x(d, "base");
    let o = v(d, "opacity");
    put(d, "outColor", c);
    put(d, "outTransparency", DVec3::ONE - o);
}

fn file_out(d: &mut NodeData) {
    // Sampling is the viewport's: downstream of the node reads the colour used where the image doesn't reach.
    let c = v(d, "defaultColor") * v(d, "colorGain") + v(d, "colorOffset");
    let a = x(d, "alphaGain") + x(d, "alphaOffset");
    put(d, "outColor", c);
    put(d, "outAlpha", a);
    put(d, "outTransparency", DVec3::splat(1.0 - a));
}

fn place_out(d: &mut NodeData) {
    let uv = v(d, "uvCoord");
    put(d, "outU", uv.x);
    put(d, "outV", uv.y);
    put(d, "outUV", uv);
}

fn bump_out(d: &mut NodeData) {
    let n = v(d, "normalCamera");
    put(d, "outNormal", n);
}

fn lambert_attrs() -> Vec<AttrDef> {
    vec![
        col("color", "c", ["cr", "cg", "cb"], 0.5),
        col("transparency", "it", ["itr", "itg", "itb"], 0.0),
        col("ambientColor", "ambc", ["acr", "acg", "acb"], 0.0),
        col("incandescence", "ic", ["ir", "ig", "ib"], 0.0),
        vector("normalCamera", "n", ["nx", "ny", "nz"], IN),
        fl01("diffuse", "dc", 0.8),
        fl01("translucence", "tc", 0.0),
        fl("translucenceDepth", "tcd", 0.5),
        fl01("translucenceFocus", "tcf", 0.5),
        fl("glowIntensity", "gi", 0.0),
        on("hideSource", "hsrc", false),
        on("refractions", "rfc", false),
        fl("refractiveIndex", "rfi", 1.0),
        fl("refractionLimit", "rfl2", 6.0),
        fl("lightAbsorbance", "abs", 0.0),
        fl("surfaceThickness", "thik", 0.0),
        fl01("shadowAttenuation", "sa", 0.5),
        on("chromaticAberration", "crab", false),
        base("matteOpacityMode", "mom", PlugType::Int, PlugValue::Int(2), STORE),
        fl01("matteOpacity", "mog", 1.0),
        col_out("outColor", "oc", ["ocr", "ocg", "ocb"]),
        col_out("outTransparency", "ot", ["otr", "otg", "otb"]),
        col_out("outGlowColor", "ogc", ["ogr", "ogg", "ogb"]),
        col_out("outMatteOpacity", "omo", ["omor", "omog", "omob"]),
        message(),
    ]
}

fn reflect_attrs() -> Vec<AttrDef> {
    vec![
        col("specularColor", "sc", ["sr", "sg", "sb"], 0.5),
        fl01("reflectivity", "rfl", 0.5),
        col("reflectedColor", "rc", ["rr", "rg", "rb"], 0.0),
        base("reflectionLimit", "rml", PlugType::Int, PlugValue::Int(1), STORE),
        fl01("reflectionSpecularity", "rspc", 1.0),
    ]
}

fn place_attrs() -> Vec<AttrDef> {
    vec![
        pair("coverage", "c", [("coverageU", "cu"), ("coverageV", "cv")], [1.0, 1.0], IN),
        pair("translateFrame", "tf", [("translateFrameU", "tfu"), ("translateFrameV", "tfv")], [0.0, 0.0], IN),
        fl("rotateFrame", "rf", 0.0),
        on("mirrorU", "mu", false),
        on("mirrorV", "mv", false),
        on("stagger", "s", false),
        on("wrapU", "wu", true),
        on("wrapV", "wv", true),
        pair("repeatUV", "re", [("repeatU", "reu"), ("repeatV", "rev")], [1.0, 1.0], IN),
        pair("offset", "of", [("offsetU", "ofu"), ("offsetV", "ofv")], [0.0, 0.0], IN),
        pair("noiseUV", "n", [("noiseU", "nu"), ("noiseV", "nv")], [0.0, 0.0], IN),
        pair("vertexUvOne", "vt1", [("vertexUvOneU", "vt1u"), ("vertexUvOneV", "vt1v")], [0.0, 0.0], STORE | flags::HIDDEN),
        pair("vertexUvTwo", "vt2", [("vertexUvTwoU", "vt2u"), ("vertexUvTwoV", "vt2v")], [0.0, 0.0], STORE | flags::HIDDEN),
        pair("vertexUvThree", "vt3", [("vertexUvThreeU", "vt3u"), ("vertexUvThreeV", "vt3v")], [0.0, 0.0], STORE | flags::HIDDEN),
        pair("vertexCameraOne", "vc1", [("vertexCameraOneX", "c1x"), ("vertexCameraOneY", "c1y")], [0.0, 0.0], STORE | flags::HIDDEN),
    ]
}

/// (name, base, doc, own attributes, compute). A None compute is abstract.
struct Spec(&'static str, &'static str, &'static str, Vec<AttrDef>, Option<Compute>);

fn table() -> Vec<Spec> {
    let mut lambert = lambert_attrs();
    let mut file = place_attrs();
    // The file node's rotateUV is .ro, place2dTexture's is .r.
    file.push(fl("rotateUV", "ro", 0.0));
    let mut own = vec![
        {
            let mut a = text("fileTextureName", "ftn", "");
            a.flags |= flags::FILE | flags::IMAGE;
            a
        },
        text("colorSpace", "cs", "sRGB"),
        on("ignoreColorSpaceFileRules", "ifr", false),
        // What defaultColorMgtGlobals passes every file texture, connected as the reference connects it.
        base("colorManagementEnabled", "cme", PlugType::Bool, PlugValue::Bool(false), flags::INPUT | flags::HIDDEN),
        base("colorManagementConfigFileEnabled", "cmcf", PlugType::Bool, PlugValue::Bool(false), flags::INPUT | flags::HIDDEN),
        base("colorManagementConfigFilePath", "cmcp", PlugType::String, PlugValue::String(String::new()), flags::INPUT | flags::HIDDEN),
        base("workingSpace", "ws", PlugType::String, PlugValue::String(String::new()), flags::INPUT | flags::HIDDEN),
        base("filterType", "ft", PlugType::Int, PlugValue::Int(3), STORE),
        fl("filter", "f", 1.0),
        fl("filterOffset", "fo", 0.0),
        on("invert", "i", false),
        on("alphaIsLuminance", "ail", false),
        col("colorGain", "cg", ["cgr", "cgg", "cgb"], 1.0),
        col("colorOffset", "co", ["cor", "cog", "cob"], 0.0),
        fl("alphaGain", "ag", 1.0),
        fl("alphaOffset", "ao", 0.0),
        col("defaultColor", "dc", ["dcr", "dcg", "dcb"], 0.5),
        pair("uvCoord", "uv", [("uCoord", "u"), ("vCoord", "v")], [0.0, 0.0], flags::INPUT),
        pair("uvFilterSize", "fs", [("uvFilterSizeX", "fsx"), ("uvFilterSizeY", "fsy")], [0.0, 0.0], flags::INPUT | flags::HIDDEN),
        on("useFrameExtension", "ufe", false),
        fl("frameExtension", "fe", 1.0),
        base("frameOffset", "fro", PlugType::Int, PlugValue::Int(0), STORE),
        base("uvTilingMode", "uvt", PlugType::Int, PlugValue::Int(0), STORE),
        fl("exposure", "exp", 0.0),
        on("fileHasAlpha", "fha", false),
        pair("outSize", "os", [("outSizeX", "osx"), ("outSizeY", "osy")], [0.0, 0.0], STORE | flags::HIDDEN),
        text("computedFileTextureNamePattern", "cfnp", ""),
        col_out("outColor", "oc", ["ocr", "ocg", "ocb"]),
        col_out("outTransparency", "ot", ["otr", "otg", "otb"]),
        fout("outAlpha", "oa"),
        message(),
    ];
    file.append(&mut own);
    let mut place = place_attrs();
    place.push(fl("rotateUV", "r", 0.0));
    place.extend([
        pair("uvCoord", "uv", [("uCoord", "u"), ("vCoord", "v")], [0.0, 0.0], flags::INPUT),
        fl("doTransform", "xf", 1.0),
        fout("outU", "ou"),
        fout("outV", "ov"),
        pair("outUV", "o", [("outU2", "ou2"), ("outV2", "ov2")], [0.0, 0.0], OUT),
        pair("outUvFilterSize", "ofs", [("outUvFilterSizeX", "ofsx"), ("outUvFilterSizeY", "ofsy")], [0.0, 0.0], OUT),
        message(),
    ]);
    lambert.shrink_to_fit();
    vec![
        Spec("lambert", "", "A matte material. The default material lambert1 is one.", lambert, Some(pass_color)),
        Spec("reflect", "lambert", "Materials with a highlight and reflections.", reflect_attrs(), None),
        Spec("phong", "reflect", "A material with a tight highlight.", vec![fl("cosinePower", "cp", 20.0)], Some(pass_color)),
        Spec("blinn", "reflect", "A material with a soft highlight.", vec![fl01("eccentricity", "ec", 0.3), fl01("specularRollOff", "sro", 0.7)], Some(pass_color)),
        Spec("phongE", "reflect", "A phong with a rough highlight.", vec![fl01("roughness", "rn", 0.5), fl01("highlightSize", "hs", 0.5), col("whiteness", "wn", ["wnr", "wng", "wnb"], 0.5)], Some(pass_color)),
        Spec("anisotropic", "reflect", "A material with a stretched highlight.", vec![fl("angle", "a", 0.0), fl("spreadX", "sx", 13.0), fl("spreadY", "sy", 3.0), fl01("roughness", "rn", 0.7), fl("fresnelRefractiveIndex", "fri", 6.0)], Some(pass_color)),
        Spec("surfaceShader", "", "A flat colour, not lit.", vec![col("outColor", "oc", ["ocr", "ocg", "ocb"], 0.0), col("outTransparency", "ot", ["otr", "otg", "otb"], 0.0), col("outGlowColor", "ogc", ["ogr", "ogg", "ogb"], 0.0), col("outMatteOpacity", "omo", ["omor", "omog", "omob"], 1.0), message()], Some(no_compute)),
        Spec("standardSurface", "", "A physically based material; the viewport draws its base colour.", crate::harold_nodes::standard_attrs(false), Some(standard_surface)),
        Spec("file", "", "An image on disk as a texture.", file, Some(file_out)),
        Spec("place2dTexture", "", "Where a 2D texture sits in UV space: repeat, offset, rotation, coverage.", place, Some(place_out)),
        Spec("bump2d", "", "Turns a texture into a bump. The viewport doesn't draw it.", vec![fl("bumpValue", "bv", 0.0), fl("bumpDepth", "bd", 1.0), base("bumpInterp", "bi", PlugType::Int, PlugValue::Int(0), STORE), vector("normalCamera", "n", ["nx", "ny", "nz"], IN), vector("outNormal", "o", ["ox", "oy", "oz"], OUT), message()], Some(bump_out)),
        Spec("materialInfo", "", "Ties a shading group to its material and textures, for the interface.", vec![base("shadingGroup", "sg", PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::HIDDEN), base("material", "m", PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::HIDDEN), slots("texture", "t"), base("textureChannel", "tc", PlugType::Int, PlugValue::Int(0), STORE | flags::HIDDEN), message()], Some(no_compute)),
    ]
}

/// A node type's classification: for shadingNode and getClassification. shadingEngine has none: files read it apart.
pub fn classification(type_name: &str) -> Option<&'static str> {
    match type_name {
        "lambert" | "phong" | "blinn" | "phongE" | "anisotropic" | "surfaceShader" | "standardSurface" => Some("shader/surface"),
        "file" => Some("texture/2d"),
        "place2dTexture" | "bump2d" => Some("utility/general"),
        "materialInfo" => Some("utility/shading"),
        _ => crate::harold_nodes::classification(type_name).or_else(|| crate::texture_nodes::classification(type_name)).or_else(|| crate::defaults::classification(type_name)),
    }
}

pub fn is_material_type(t: &NodeType) -> bool {
    t.is_a("lambert") || matches!(t.name.as_str(), "surfaceShader" | "standardSurface" | "openPBRSurface") || crate::harold_nodes::classification(&t.name) == Some("shader/surface")
}

/// Nodes whose attributes a texture can drive from the Attribute Editor: materials, textures, lights and the
/// rendering utilities.
pub fn takes_textures(t: &NodeType) -> bool {
    if is_material_type(t) || is_texture_type(t) || t.is_a("light") {
        return true;
    }
    matches!(classification(&t.name), Some(c) if c.starts_with("shader") || c.starts_with("texture") || c.starts_with("light"))
        || matches!(t.name.as_str(), "bump2d" | "aiBump2d" | "aiNormalMap" | "displacementShader")
}

pub fn is_texture_type(t: &NodeType) -> bool {
    t.name == "file" || t.name == "ramp" || crate::harold_nodes::classification(&t.name) == Some("texture/2d") || crate::texture_nodes::is_texture(&t.name)
}

pub fn register_types() {
    if types::find(SG_TYPE).is_some() {
        return;
    }
    for Spec(name, parent, doc, own_attrs, compute) in table() {
        let base_type = (!parent.is_empty()).then(|| types::find(parent).expect(parent));
        types::register_type(NodeType {
            name: name.into(),
            base: base_type,
            doc: doc.into(),
            own_attrs,
            create: compute.map(|c| Arc::new(move || Box::new(Shade(c)) as Box<dyn NodeBehavior>) as crate::node::CreateFn),
            ..Default::default()
        })
        .expect(name);
    }
    let set = types::find(os::TYPE_NAME).expect("objectSet");
    types::register_type(NodeType {
        name: SG_TYPE.into(),
        base: Some(set),
        doc: "A shading group: the set of shapes and faces a material draws on.".into(),
        own_attrs: vec![
            col("surfaceShader", "ss", ["ssr", "ssg", "ssb"], 0.0),
            col("volumeShader", "vs", ["vsr", "vsg", "vsb"], 0.0),
            fl("displacementShader", "ds", 0.0),
            col("imageShader", "is", ["isr", "isg", "isb"], 0.0),
            slots("dagSetMembers", "dsm"),
            slots("dnSetMembers", "dnsm"),
            slots("groupNodes", "gn"),
            base("memberWireframeColor", "mwc", PlugType::Int, PlugValue::Int(-1), STORE | flags::HIDDEN),
            on("renderableOnlySet", "ro", true),
            text("annotation", "an", ""),
            base("partition", "pa", PlugType::Int, PlugValue::Int(0), OUT | flags::HIDDEN),
            message(),
        ],
        create: Some(Arc::new(|| Box::new(Shade(no_compute)))),
        ..Default::default()
    })
    .expect(SG_TYPE);
    // A copy of a shape is drawn with what drew the original.
    crate::duplicate::add_fixup(Arc::new(|g, copies| {
        let shapes: Vec<(NodeId, NodeId)> = copies.iter().map(|(a, b)| (*a, *b)).filter(|(a, _)| is_shaded_shape(g, *a)).collect();
        for (from, to) in shapes {
            let _ = copy_membership(g, from, to);
        }
    }));
}

/// lambert1 and initialShadingGroup, in every scene. Off the undo stack.
pub fn make_defaults(g: &mut Graph) {
    types::register_builtins();
    g.without_undo(crate::harold_nodes::make_defaults);
    if g.find(INITIAL_SG).ok().flatten().is_some() {
        return;
    }
    g.without_undo(|g| {
        let made = (|| -> crate::error::Result<()> {
            let lambert = match g.find(DEFAULT_MATERIAL)? {
                Some(id) => id,
                None => g.create_node(&types::find("lambert").unwrap(), DEFAULT_MATERIAL, NO_NODE, false)?,
            };
            let sg = g.create_node(&types::find(SG_TYPE).unwrap(), INITIAL_SG, NO_NODE, false)?;
            let info = g.create_node(&types::find("materialInfo").unwrap(), INITIAL_INFO, NO_NODE, false)?;
            connect_material(g, lambert, sg)?;
            connect_info(g, info, sg, lambert)?;
            Ok(())
        })();
        if let Err(e) = made {
            log::warn!("default shading nodes: {e}");
        }
    });
}

/// Whether a node is one every scene has, which a file names but doesn't create and nothing may delete.
pub fn is_default_node(name: &str) -> bool {
    matches!(name, DEFAULT_MATERIAL | INITIAL_SG | INITIAL_INFO) || crate::harold_nodes::is_default_node(name) || crate::defaults::is_scene_node(name)
}

pub(crate) fn plug_named(g: &Graph, id: NodeId, attr: &str) -> Option<PlugRef> {
    g.node(id)?.ty().attr_index(attr).map(|i| PlugRef::new(id, i as i32))
}

fn connect_material(g: &mut Graph, mat: NodeId, sg: NodeId) -> crate::error::Result<()> {
    let (Some(a), Some(b)) = (plug_named(g, mat, "outColor"), plug_named(g, sg, "surfaceShader")) else {
        return Err(crate::error::GraphError::invalid("not a material and a shading group"));
    };
    g.connect(&a, &b, true)?;
    Ok(())
}

fn connect_info(g: &mut Graph, info: NodeId, sg: NodeId, mat: NodeId) -> crate::error::Result<()> {
    for (src, attr) in [(sg, "shadingGroup"), (mat, "material")] {
        if let (Some(a), Some(b)) = (plug_named(g, src, "message"), plug_named(g, info, attr)) {
            g.connect(&a, &b, true)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------------
// Membership

pub fn is_shading_group(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a(SG_TYPE))
}

pub fn all_groups(g: &Graph) -> Vec<NodeId> {
    let mut ids = g.nodes();
    ids.sort_unstable();
    ids.retain(|&id| is_shading_group(g, id));
    ids
}

fn is_shaded_shape(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| (n.ty().is_a("mesh") || n.ty().is_a("nurbsSurface")) && !n.bool(DagNode::INTERMEDIATE_OBJECT))
}

/// Shapes and faces, from names as they come: a transform stands for the shapes under it.
fn shaded_members(g: &Graph, list: &[Member]) -> CmdResult<Vec<Member>> {
    let mut out = Vec::new();
    for m in list {
        match m.kind {
            ComponentMode::Object => {
                if is_shaded_shape(g, m.node) {
                    out.push(Member::object(m.node));
                } else {
                    let below: Vec<NodeId> = g.descendants(m.node).into_iter().filter(|&d| is_shaded_shape(g, d)).collect();
                    if below.is_empty() && !g.node(m.node).is_some_and(|n| n.ty().is_a("transform")) {
                        fail!("{} can't take a material", g.path(m.node));
                    }
                    out.extend(below.into_iter().map(Member::object));
                }
            }
            ComponentMode::Face => out.push(m.clone()),
            _ => fail!("{}: only whole objects and faces take a material", g.path(m.node)),
        }
    }
    Ok(out)
}

fn all_faces(g: &mut Graph, shape: NodeId) -> Vec<u32> {
    match crate::components::mesh_of(g, shape) {
        Some(m) => (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect(),
        None => Vec::new(),
    }
}

/// Makes a shading group shade these, taking them from whichever group shades them now (sets -e -forceElement).
pub fn assign(g: &mut Graph, sg: NodeId, list: &[Member]) -> CmdResult<()> {
    let list = shaded_members(g, list)?;
    if list.is_empty() {
        return Ok(());
    }
    for other in all_groups(g) {
        if other == sg {
            continue;
        }
        let before = os::members(g, other);
        let mut after = before.clone();
        for m in &list {
            if m.kind == ComponentMode::Object {
                after.retain(|x| x.node != m.node);
                continue;
            }
            // Some faces leave a group that shades the whole mesh: it keeps the rest.
            if after.iter().any(|x| x.node == m.node && x.kind == ComponentMode::Object) {
                after.retain(|x| !(x.node == m.node && x.kind == ComponentMode::Object));
                after.push(Member { node: m.node, kind: ComponentMode::Face, ids: all_faces(g, m.node) });
            }
            for x in after.iter_mut().filter(|x| x.node == m.node && x.kind == ComponentMode::Face) {
                x.ids.retain(|i| m.ids.binary_search(i).is_err());
            }
        }
        after.retain(|x| x.kind == ComponentMode::Object || !x.ids.is_empty());
        if after != before {
            os::set_members_raw(g, other, &after)?;
            wire(g, other)?;
        }
    }
    let mut mine = os::members(g, sg);
    for m in &list {
        if m.kind == ComponentMode::Object {
            mine.retain(|x| x.node != m.node);
        }
    }
    let whole: Vec<NodeId> = mine.iter().filter(|x| x.kind == ComponentMode::Object).map(|x| x.node).collect();
    let extra: Vec<Member> = list.into_iter().filter(|m| !whole.contains(&m.node)).collect();
    mine.extend(extra);
    os::set_members_raw(g, sg, &mine)?;
    wire(g, sg)
}

/// Takes these out of a shading group (sets -e -remove). Some faces of a whole shape leave the rest behind.
pub fn unassign(g: &mut Graph, sg: NodeId, list: &[Member]) -> CmdResult<()> {
    let list = shaded_members(g, list)?;
    let mut after = os::members(g, sg);
    for m in &list {
        if m.kind == ComponentMode::Object {
            after.retain(|x| x.node != m.node);
            continue;
        }
        if after.iter().any(|x| x.node == m.node && x.kind == ComponentMode::Object) {
            after.retain(|x| !(x.node == m.node && x.kind == ComponentMode::Object));
            after.push(Member { node: m.node, kind: ComponentMode::Face, ids: all_faces(g, m.node) });
        }
        for x in after.iter_mut().filter(|x| x.node == m.node && x.kind == ComponentMode::Face) {
            x.ids.retain(|i| m.ids.binary_search(i).is_err());
        }
    }
    after.retain(|x| x.kind == ComponentMode::Object || !x.ids.is_empty());
    os::set_members_raw(g, sg, &after)?;
    wire(g, sg)
}

fn iog_attr(g: &Graph, shape: NodeId) -> Option<(i32, i32)> {
    let t = g.node(shape)?.type_arc().clone();
    Some((t.attr_index("instObjGroups")? as i32, t.attr_index("instObjGroups.objectGroups")? as i32))
}

/// Brings a shading group's dagSetMembers connections in line with its member list.
pub fn wire(g: &mut Graph, sg: NodeId) -> CmdResult<()> {
    let Some(dsm) = plug_named(g, sg, "dagSetMembers") else { return Ok(()) };
    let connected: Vec<(PlugRef, PlugRef)> = g.elements(&dsm).into_iter().filter_map(|e| g.source(&e).map(|s| (e, s))).collect();
    // By node and attribute, the first connected in element order: a group of thousands is matched in one pass.
    let mut first: HashMap<(NodeId, i32), PlugRef> = HashMap::with_capacity(connected.len());
    for (_, s) in &connected {
        first.entry((s.node, s.attr)).or_insert(*s);
    }
    let sources: HashSet<PlugRef> = connected.iter().map(|c| c.1).collect();
    let mut wanted: Vec<PlugRef> = Vec::new();
    for m in os::members(g, sg) {
        let Some((iog, og)) = iog_attr(g, m.node) else { continue };
        if m.kind == ComponentMode::Object {
            let p = first.get(&(m.node, iog)).copied();
            wanted.push(p.unwrap_or_else(|| PlugRef::element(m.node, iog, 0)));
        } else if let Some(p) = first.get(&(m.node, og)).copied() {
            wanted.push(p);
        } else {
            // The first object group this shape has free.
            let mut k = 0;
            while !g.destinations(&PlugRef::element(m.node, og, k)).is_empty() {
                k += 1;
            }
            wanted.push(PlugRef::element(m.node, og, k));
        }
    }
    let keep: HashSet<PlugRef> = wanted.iter().copied().collect();
    for (e, s) in &connected {
        if !keep.contains(s) {
            g.disconnect(e);
            let _ = g.remove_element(e);
        }
    }
    for s in wanted {
        if !sources.contains(&s) {
            g.connect(&s, &dsm, true)?;
        }
    }
    Ok(())
}

/// A script connected or disconnected dagSetMembers by hand: the member list follows the connections.
fn follow_connections(g: &mut Graph, sg: NodeId) -> CmdResult<()> {
    let Some(dsm) = plug_named(g, sg, "dagSetMembers") else { return Ok(()) };
    let sources: Vec<PlugRef> = g.elements(&dsm).into_iter().filter_map(|e| g.source(&e)).collect();
    let mut keep = Vec::new();
    let mut joined = Vec::new();
    for m in os::members(g, sg) {
        let Some((iog, og)) = iog_attr(g, m.node) else { continue };
        let want = if m.kind == ComponentMode::Object { iog } else { og };
        if sources.iter().any(|s| s.node == m.node && s.attr == want) {
            keep.push(m);
        }
    }
    for s in &sources {
        let Some((iog, _)) = iog_attr(g, s.node) else { continue };
        if s.attr == iog && !keep.iter().any(|m| m.node == s.node && m.kind == ComponentMode::Object) {
            joined.push(Member::object(s.node));
        }
    }
    os::set_members_raw(g, sg, &keep)?;
    if !joined.is_empty() {
        assign(g, sg, &joined)?;
    }
    wire(g, sg)
}

/// The shading groups that hold a shape, or any of its faces.
pub fn groups_of(g: &Graph, shape: NodeId) -> Vec<NodeId> {
    all_groups(g).into_iter().filter(|&sg| os::members(g, sg).iter().any(|m| m.node == shape)).collect()
}

fn copy_membership(g: &mut Graph, from: NodeId, to: NodeId) -> CmdResult<()> {
    for sg in all_groups(g) {
        let held: Vec<Member> = os::members(g, sg).into_iter().filter(|m| m.node == from).map(|m| Member { node: to, ..m }).collect();
        if !held.is_empty() {
            assign(g, sg, &held)?;
        }
    }
    Ok(())
}

/// At the end of a command, still in its undo step: new meshes that nothing has shaded go in the default group.
pub(crate) fn adopt_new_meshes(s: &mut Session) {
    let Some(from) = s.new_nodes_from.take() else { return };
    let g = &mut s.graph;
    let to = g.next_node_id();
    if from >= to {
        return;
    }
    let Ok(Some(init)) = g.find(INITIAL_SG) else { return };
    if !is_shading_group(g, init) {
        return;
    }
    let shapes: Vec<NodeId> = (from..to).filter(|&id| is_shaded_shape(g, id) && crate::instancing::source(g, id) == NO_NODE).collect();
    if shapes.is_empty() {
        return;
    }
    // Which of them a group already shades, asked of each group's bound entries: with thousands of meshes in the
    // default group, naming every member for every new mesh made creating them slower with each one.
    let mut shaded: Vec<NodeId> = Vec::new();
    let mut quick = true;
    for sg in all_groups(g) {
        match os::holds_any(g, sg, &shapes) {
            Some(held) => shaded.extend(held),
            None => quick = false,
        }
    }
    let fresh: Vec<NodeId> = if quick { shapes.into_iter().filter(|id| !shaded.contains(id)).collect() } else { shapes.into_iter().filter(|&id| groups_of(g, id).is_empty()).collect() };
    if fresh.is_empty() {
        return;
    }
    let r = if quick { adopt(g, init, &fresh) } else { assign(g, init, &fresh.iter().map(|&id| Member::object(id)).collect::<Vec<_>>()) };
    if let Err(e) = r {
        log::warn!("default shading group: {e}");
    }
}

/// Whole shapes no group shades join one: its member list and its dagSetMembers each gain them, as `assign` leaves
/// both, without going over the members the group already has.
fn adopt(g: &mut Graph, sg: NodeId, shapes: &[NodeId]) -> CmdResult<()> {
    os::append_objects_raw(g, sg, shapes)?;
    let Some(dsm) = plug_named(g, sg, "dagSetMembers") else { return Ok(()) };
    for &shape in shapes {
        let Some((iog, _)) = iog_attr(g, shape) else { continue };
        let from = PlugRef::element(shape, iog, 0);
        if !g.destinations(&from).iter().any(|d| d.node == sg) {
            g.connect(&from, &dsm, true)?;
        }
    }
    Ok(())
}

/// A reader builds what the file says: nothing it made joins the default group afterwards.
pub(crate) fn scene_was_read(s: &mut Session) {
    s.new_nodes_from = None;
}

// ---------------------------------------------------------------------------------------------------------------------
// Materials

/// A material's shading group; it makes one, `<material>SG` with its materialInfo, if there is none.
pub fn group_of_material(g: &mut Graph, mat: NodeId, make: bool) -> CmdResult<Option<NodeId>> {
    if is_shading_group(g, mat) {
        return Ok(Some(mat));
    }
    let Some(out) = plug_named(g, mat, "outColor") else { fail!("{} is not a material", g.path(mat)) };
    if let Some(d) = g.destinations(&out).into_iter().find(|d| is_shading_group(g, d.node) && g.attr(d).name == "surfaceShader") {
        return Ok(Some(d.node));
    }
    if !make {
        return Ok(None);
    }
    let name = format!("{}SG", g.node(mat).unwrap().name().rsplit(':').next().unwrap_or(""));
    let sg = g.create_node(&types::find(SG_TYPE).unwrap(), &name, NO_NODE, true)?;
    let info = g.create_node(&types::find("materialInfo").unwrap(), "materialInfo1", NO_NODE, true)?;
    connect_material(g, mat, sg)?;
    connect_info(g, info, sg, mat)?;
    Ok(Some(sg))
}

/// The material on a shading group's surfaceShader.
pub fn material_of_group(g: &Graph, sg: NodeId) -> Option<NodeId> {
    let p = plug_named(g, sg, "surfaceShader")?;
    g.source(&p).map(|s| s.node)
}

/// What a set of names selects, as members; an empty list reads the selection. Vertices and edges count for their object.
fn targets(g: &Graph, names: &[String]) -> CmdResult<Vec<Member>> {
    let list = if names.is_empty() { os::from_selection(g) } else { os::parse(g, names).map_err(CommandError::new)? };
    Ok(list.into_iter().map(|m| if matches!(m.kind, ComponentMode::Vertex | ComponentMode::Edge) { Member::object(m.node) } else { m }).collect())
}

fn node_named(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name)? {
        Some(id) => Ok(id),
        None => fail!("{cmd}: No object matches name: {name}"),
    }
}

/// Everything upstream of a node, itself first.
fn upstream(g: &Graph, from: NodeId) -> Vec<NodeId> {
    let conns = g.connections();
    let mut out = vec![from];
    let mut k = 0;
    while k < out.len() {
        let at = out[k];
        k += 1;
        for (dst, src) in &conns {
            if dst.node == at && !out.contains(&src.node) && !is_shaded_shape(g, src.node) {
                out.push(src.node);
            }
        }
    }
    out
}

fn hyper_shade(s: &mut Session, a: &Args) -> CmdResult {
    make_defaults(&mut s.graph);
    if a.bool("duplicate") {
        let names: Vec<String> = s.graph.selection().iter().map(|&m| s.graph.path(m)).collect();
        return s.run("duplicate", json!({"args": names, "upstreamNodes": true}));
    }
    let g = &mut s.graph;
    if let Some(name) = a.get("assign").and_then(Value::as_str) {
        let mat = node_named(g, name, "hyperShade")?;
        let list = targets(g, &a.named())?;
        if list.is_empty() {
            fail!("hyperShade: nothing selected to assign {name} to");
        }
        let sg = group_of_material(g, mat, true)?.unwrap();
        assign(g, sg, &list)?;
        return Ok(Value::Null);
    }
    if let Some(name) = a.get("objects").and_then(Value::as_str) {
        let mat = if name.is_empty() {
            let picked = g.selection().iter().copied().find(|&id| g.node(id).is_some_and(|n| is_material_type(n.ty()) || n.ty().is_a(SG_TYPE)));
            match picked {
                Some(m) => m,
                None => fail!("hyperShade: select a material"),
            }
        } else {
            node_named(g, name, "hyperShade")?
        };
        match group_of_material(g, mat, false)? {
            Some(sg) => os::select(g, &[sg], false),
            None => g.select(&[], SelectMode::Replace),
        }
        return Ok(Value::Null);
    }
    if a.bool("shaderNetworksSelectMaterialNodes") {
        let mut shapes = Vec::new();
        for id in g.selection().to_vec() {
            if is_shaded_shape(g, id) {
                shapes.push(id);
            } else {
                shapes.extend(g.descendants(id).into_iter().filter(|&d| is_shaded_shape(g, d)));
            }
        }
        let mut mats = Vec::new();
        for sh in shapes {
            for sg in groups_of(g, sh) {
                if let Some(m) = material_of_group(g, sg).filter(|m| !mats.contains(m)) {
                    mats.push(m);
                }
            }
        }
        crate::components::clear(g);
        crate::components::set_mode(g, ComponentMode::Object);
        g.select(&mats, SelectMode::Replace);
        return Ok(Value::Null);
    }
    if let Some(name) = a.get("listUpstreamNodes").or(a.get("listUpstreamShaderNodes")).and_then(Value::as_str) {
        let id = node_named(g, name, "hyperShade")?;
        let shaders_only = a.has("listUpstreamShaderNodes");
        let list: Vec<String> = upstream(g, id).into_iter().skip(1).filter(|&n| !shaders_only || is_shading_node(g, n)).map(|n| g.path(n)).collect();
        return Ok(json!(list));
    }
    // The network under the selected materials.
    if a.bool("shaderNetwork") {
        let mut net = Vec::new();
        for m in g.selection().to_vec() {
            for n in upstream(g, m) {
                if !net.contains(&n) && is_shading_node(g, n) {
                    net.push(n);
                }
            }
        }
        g.select(&net, SelectMode::Replace);
        return Ok(Value::Null);
    }
    if let Some(name) = a.get("listDownstreamNodes").or(a.get("listDownstreamShaderNodes")).and_then(Value::as_str) {
        let shaders_only = a.has("listDownstreamShaderNodes");
        let id = node_named(g, name, "hyperShade")?;
        let conns = g.connections();
        let mut out = vec![id];
        let mut k = 0;
        while k < out.len() {
            let at = out[k];
            k += 1;
            for (dst, src) in &conns {
                if src.node == at && !out.contains(&dst.node) {
                    out.push(dst.node);
                }
            }
        }
        return Ok(json!(out.into_iter().skip(1).filter(|&n| !shaders_only || is_shading_node(g, n)).map(|n| g.path(n)).collect::<Vec<_>>()));
    }
    Ok(Value::Null)
}

/// A node of a shading network: not the scene's time, a transform or a shape it happens to read from.
fn is_shading_node(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| !n.ty().dag && n.ty().name != "time" && !n.ty().name.starts_with("animCurve"))
}

/// The plug defaultNavigation connects from when only the node is named.
fn default_out(g: &Graph, src: NodeId, dst: Option<&PlugRef>) -> Option<PlugRef> {
    let scalar = dst.is_some_and(|d| d.component >= 0 || g.attr(d).ty == PlugType::Float);
    let transparent = dst.is_some_and(|d| g.attr(d).name == "transparency");
    let names: &[&str] = match g.node(src)?.ty().name.as_str() {
        "file" if scalar => &["outAlpha"],
        "file" if transparent => &["outTransparency"],
        "place2dTexture" => &["outUV"],
        _ if transparent => &["outTransparency", "outColor"],
        _ if scalar => &["outValue", "outAlpha", "output"],
        _ => &["outColor", "output"],
    };
    names.iter().find_map(|n| plug_named(g, src, n))
}

/// What place2dTexture feeds a file node, the reference's connections in the reference's order: (source attribute, file attribute).
pub const PLACE_TO_FILE: &[(&str, &str)] = &[
    ("coverage", "coverage"),
    ("translateFrame", "translateFrame"),
    ("rotateFrame", "rotateFrame"),
    ("mirrorU", "mirrorU"),
    ("mirrorV", "mirrorV"),
    ("stagger", "stagger"),
    ("wrapU", "wrapU"),
    ("wrapV", "wrapV"),
    ("repeatUV", "repeatUV"),
    ("offset", "offset"),
    ("rotateUV", "rotateUV"),
    ("noiseUV", "noiseUV"),
    ("vertexUvOne", "vertexUvOne"),
    ("vertexUvTwo", "vertexUvTwo"),
    ("vertexUvThree", "vertexUvThree"),
    ("vertexCameraOne", "vertexCameraOne"),
    ("outUV", "uvCoord"),
    ("outUvFilterSize", "uvFilterSize"),
];

/// Wires a place2dTexture into a file node, every plug as the reference's fileTexture does.
pub fn connect_placement(g: &mut Graph, place: NodeId, file: NodeId) -> CmdResult<()> {
    for (a, b) in PLACE_TO_FILE {
        if let (Some(x), Some(y)) = (plug_named(g, place, a), plug_named(g, file, b)) {
            g.connect(&x, &y, true)?;
        }
    }
    Ok(())
}

fn default_navigation(s: &mut Session, a: &Args) -> CmdResult {
    let g = &mut s.graph;
    if !a.bool("connectToExisting") {
        // createNew and the traversal flags answer questions the interface asks of itself.
        return Ok(json!([]));
    }
    let (Some(src), Some(dst)) = (a.get("source").and_then(Value::as_str), a.get("destination").and_then(Value::as_str)) else {
        fail!("defaultNavigation: give -source and -destination");
    };
    let dst_named = dst.contains('.');
    let src_named = src.contains('.');
    let src_id = if src_named { g.plug(src)?.node } else { node_named(g, src, "defaultNavigation")? };
    let dst_id = if dst_named { g.plug(dst)?.node } else { node_named(g, dst, "defaultNavigation")? };
    if !src_named && !dst_named && g.node(src_id).is_some_and(|n| n.ty().name == "place2dTexture") && g.node(dst_id).is_some_and(|n| n.ty().name == "file") {
        connect_placement(g, src_id, dst_id)?;
        return Ok(json!([]));
    }
    let dst_plug = if dst_named {
        g.plug(dst)?
    } else {
        let t = g.node(dst_id).unwrap().type_arc().clone();
        let name = if t.is_a(SG_TYPE) {
            "surfaceShader"
        } else if t.name == "standardSurface" {
            "baseColor"
        } else if t.name == "surfaceShader" {
            "outColor"
        } else {
            "color"
        };
        match plug_named(g, dst_id, name) {
            Some(p) => p,
            None => fail!("defaultNavigation: {dst} has no default input"),
        }
    };
    let src_plug = if src_named {
        g.plug(src)?
    } else {
        match default_out(g, src_id, Some(&dst_plug)) {
            Some(p) => p,
            None => fail!("defaultNavigation: {src} has no default output"),
        }
    };
    g.connect(&src_plug, &dst_plug, true)?;
    Ok(json!([]))
}

// ---------------------------------------------------------------------------------------------------------------------
// Texture paths

pub(crate) fn expand_env(path: &str) -> String {
    let mut out = String::new();
    let mut rest = path;
    while let Some(i) = rest.find('$') {
        out.push_str(&rest[..i]);
        let tail = &rest[i + 1..];
        let (name, after) = if let Some(t) = tail.strip_prefix('{') {
            match t.split_once('}') {
                Some((n, a)) => (n, a),
                None => (t, ""),
            }
        } else {
            let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(tail.len());
            (&tail[..end], &tail[end..])
        };
        match std::env::var(name) {
            Ok(v) => out.push_str(&v),
            Err(_) => {
                out.push('$');
                out.push_str(name);
            }
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A file in a folder by its name, whatever its case: a path written on Windows doesn't say.
fn in_folder(dir: &Path, name: &str) -> Option<PathBuf> {
    let direct = dir.join(name);
    if direct.is_file() {
        return Some(direct);
    }
    let lower = name.to_lowercase();
    std::fs::read_dir(dir).ok()?.flatten().find(|e| e.file_name().to_string_lossy().to_lowercase() == lower && e.path().is_file()).map(|e| e.path())
}

/// Finds an image as the reference does: the path as written, relative to the project and then the scene; then by what follows
/// sourceimages in the path, then by file name, in the scene's folder and the sourceimages folders.
/// Folders of referenced files: their textures tend to sit beside them, not beside the scene using them.
static SEARCH_FOLDERS: std::sync::Mutex<Vec<PathBuf>> = std::sync::Mutex::new(Vec::new());

pub(crate) fn add_search_folder(dir: &Path) {
    if let Ok(mut f) = SEARCH_FOLDERS.lock() {
        if !f.iter().any(|d| d == dir) {
            f.push(dir.to_path_buf());
        }
    }
}

pub fn resolve_texture_uncached(written: &str, scene: Option<&str>, project: Option<&str>) -> Option<PathBuf> {
    let written = expand_env(written.trim()).replace('\\', "/");
    if written.is_empty() {
        return None;
    }
    let as_written = Path::new(&written);
    if as_written.is_file() {
        return Some(as_written.to_path_buf());
    }
    let scene_dir = scene.and_then(|s| Path::new(s).parent()).map(Path::to_path_buf);
    let project = project.filter(|p| !p.is_empty()).map(PathBuf::from);
    if !as_written.is_absolute() {
        for dir in project.iter().chain(scene_dir.iter()) {
            let p = dir.join(as_written);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    // Where to look by name: next to the scene, then in the sourceimages a project would have.
    let mut folders: Vec<PathBuf> = Vec::new();
    if let Some(d) = &scene_dir {
        folders.push(d.clone());
    }
    if let Some(p) = &project {
        folders.push(p.join("sourceimages"));
    }
    if let Ok(f) = SEARCH_FOLDERS.lock() {
        for d in f.iter() {
            folders.push(d.clone());
            folders.push(d.join("sourceimages"));
        }
    }
    if let Some(d) = &scene_dir {
        folders.push(d.join("sourceimages"));
        if let Some(up) = d.parent() {
            folders.push(up.join("sourceimages"));
        }
        // Rigs handed round as a folder often keep their images in one of these beside the scene.
        folders.push(d.join("Textures"));
        folders.push(d.join("textures"));
    }
    if let Some(i) = written.to_lowercase().find("sourceimages/") {
        let sub = &written[i + "sourceimages/".len()..];
        for f in folders.iter().filter(|f| f.ends_with("sourceimages")) {
            let p = f.join(sub);
            if p.is_file() {
                return Some(p);
            }
        }
    }
    let name = written.rsplit('/').next().filter(|n| !n.is_empty())?;
    folders.iter().find_map(|f| in_folder(f, name))
}

/// `resolve_texture_uncached`, asking the disk at most every two seconds per path: the viewport reads this on many frames.
pub fn resolve_texture(written: &str, scene: Option<&str>, project: Option<&str>) -> Option<PathBuf> {
    type Memo = HashMap<(String, Option<String>, Option<String>), (std::time::Instant, Option<PathBuf>)>;
    static MEMO: std::sync::Mutex<Option<Memo>> = std::sync::Mutex::new(None);
    let key = (written.to_string(), scene.map(str::to_string), project.map(str::to_string));
    let mut guard = MEMO.lock().unwrap();
    let memo = guard.get_or_insert_with(HashMap::new);
    if let Some((when, found)) = memo.get(&key) {
        if when.elapsed().as_secs_f64() < 2.0 {
            return found.clone();
        }
    }
    let found = resolve_texture_uncached(written, scene, project);
    memo.insert(key, (std::time::Instant::now(), found.clone()));
    found
}

/// The project folder, from the workspace command in force.
pub fn project_dir(s: &mut Session) -> Option<String> {
    s.run("workspace", json!({"query": true, "queried": ["rootDirectory"]})).ok().and_then(|v| v.as_str().map(str::to_string))
}

// ---------------------------------------------------------------------------------------------------------------------
// For the viewport

/// A file texture as the viewport samples it, its placement read through the file node's connected plugs.
#[derive(Clone, Debug, PartialEq)]
pub struct TextureRef {
    /// A network worked out into a picture, when the colour does not come straight from one image file. `resolved`
    /// is then the picture's key, not a path.
    pub baked: Option<std::sync::Arc<crate::harold::swatch::Picture>>,
    pub file: NodeId,
    pub written: String,
    pub resolved: Option<String>,
    /// colorSpace says the image is sRGB encoded rather than linear.
    pub srgb: bool,
    pub coverage: [f32; 2],
    pub translate_frame: [f32; 2],
    pub rotate_frame: f32,
    pub repeat: [f32; 2],
    pub offset: [f32; 2],
    pub rotate_uv: f32,
    pub wrap: [bool; 2],
    pub mirror: [bool; 2],
    pub default_color: [f32; 3],
    pub gain: [f32; 3],
    pub color_offset: [f32; 3],
    pub invert: bool,
    pub alpha_is_luminance: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SpecularModel {
    /// A cosine power.
    Phong,
    /// Eccentricity and roll off.
    Blinn,
}

/// A material's highlight, in the shader's terms.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DrawSpecular {
    pub model: SpecularModel,
    pub color: [f32; 3],
    /// The cosine power, or the eccentricity.
    pub shininess: f32,
    pub roll_off: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DrawMaterial {
    pub group: NodeId,
    pub material: NodeId,
    /// Not lit: a surfaceShader.
    pub unlit: bool,
    pub color: [f32; 3],
    pub ambient: [f32; 3],
    pub incandescence: [f32; 3],
    pub diffuse: f32,
    /// Mean of the transparency channels, 0 to 1.
    pub transparency: f32,
    pub color_map: Option<TextureRef>,
    /// The texture on transparency.
    pub alpha_map: Option<TextureRef>,
    /// Drawn when real lights are in use.
    pub specular: Option<DrawSpecular>,
}

/// A material's highlight. phong and blinn draw theirs. phongE, anisotropic and standardSurface draw a blinn of the same
/// width: their own shapes of highlight aren't drawn.
fn draw_specular(g: &mut Graph, material: NodeId, ty: &str) -> Option<DrawSpecular> {
    let color = |g: &mut Graph, a: &str| f3(value_v(g, material, a).unwrap_or(DVec3::splat(0.5)));
    let num = |g: &mut Graph, a: &str, d: f64| value_f(g, material, a).unwrap_or(d) as f32;
    let blinn = |color: [f32; 3], ecc: f32, roll_off: f32| Some(DrawSpecular { model: SpecularModel::Blinn, color, shininess: ecc.max(1e-3), roll_off });
    match ty {
        "phong" => Some(DrawSpecular { model: SpecularModel::Phong, color: color(g, "specularColor"), shininess: num(g, "cosinePower", 20.0).max(1.0), roll_off: 1.0 }),
        "blinn" => {
            let (c, e, r) = (color(g, "specularColor"), num(g, "eccentricity", 0.3), num(g, "specularRollOff", 0.7));
            blinn(c, e, r)
        }
        "phongE" => {
            let (c, e) = (color(g, "specularColor"), num(g, "highlightSize", 0.5) * 0.5);
            blinn(c, e, 1.0)
        }
        "anisotropic" => {
            let (c, e) = (color(g, "specularColor"), num(g, "roughness", 0.7) * 0.5);
            blinn(c, e, 1.0)
        }
        "aiStandardSurface" => {
            let (mut c, k, rough) = (color(g, "specular_color"), num(g, "specular", 1.0), num(g, "specular_roughness", 0.2));
            for x in &mut c {
                *x *= k;
            }
            blinn(c, rough, 1.0)
        }
        "standardSurface" => {
            let (mut c, k, rough) = (color(g, "specularColor"), num(g, "specular", 1.0), num(g, "specularRoughness", 0.2));
            for x in &mut c {
                *x *= k;
            }
            blinn(c, rough, 1.0)
        }
        _ => None,
    }
}

fn f3(v: DVec3) -> [f32; 3] {
    v.as_vec3().to_array()
}

pub(crate) fn value_v(g: &mut Graph, id: NodeId, attr: &str) -> Option<DVec3> {
    let p = plug_named(g, id, attr)?;
    match g.value(&p).ok()? {
        PlugValue::Vec3(x) => Some(x),
        _ => None,
    }
}

pub(crate) fn value_f(g: &mut Graph, id: NodeId, attr: &str) -> Option<f64> {
    let p = plug_named(g, id, attr)?;
    g.value(&p).ok()?.scalar()
}

/// A file node at or just upstream of a plug, through whatever utility nodes lie between. Asked often: it reads only
/// the plugs of the nodes on the way.
fn file_feeding(g: &Graph, id: NodeId, attr: &str) -> Option<NodeId> {
    let i = g.node(id)?.ty().attr_index(attr)? as i32;
    let mut seen: Vec<NodeId> = Vec::new();
    for p in [PlugRef::new(id, i), PlugRef::component(id, i, 0), PlugRef::component(id, i, 1), PlugRef::component(id, i, 2)] {
        if let Some(s) = g.source(&p) {
            seen.push(s.node);
        }
    }
    let mut k = 0;
    while k < seen.len() && seen.len() < 64 {
        let at = seen[k];
        k += 1;
        let Some(n) = g.node(at) else { continue };
        if n.ty().name == "file" {
            return Some(at);
        }
        for (a, def) in n.ty().attrs.iter().enumerate() {
            if !def.has(flags::INPUT) || def.is_array() {
                continue;
            }
            let plugs = [PlugRef::new(at, a as i32), PlugRef::component(at, a as i32, 0), PlugRef::component(at, a as i32, 1), PlugRef::component(at, a as i32, 2)];
            let count = if def.ty == PlugType::Vec3 { 4 } else { 1 };
            for p in &plugs[..count] {
                if let Some(s) = g.source(p) {
                    if !seen.contains(&s.node) && !is_shaded_shape(g, s.node) {
                        seen.push(s.node);
                    }
                }
            }
        }
    }
    None
}

fn texture_ref(g: &mut Graph, file: NodeId, scene: Option<&str>, project: Option<&str>) -> TextureRef {
    let written = plug_named(g, file, "fileTextureName").and_then(|p| g.value(&p).ok()).and_then(|v| if let PlugValue::String(x) = v { Some(x) } else { None }).unwrap_or_default();
    let space = plug_named(g, file, "colorSpace").and_then(|p| g.value(&p).ok()).and_then(|v| if let PlugValue::String(x) = v { Some(x) } else { None }).unwrap_or_default();
    let two = |g: &mut Graph, a: &str, d: f32| value_v(g, file, a).map_or([d, d], |x| [x.x as f32, x.y as f32]);
    let flag = |g: &mut Graph, a: &str, d: bool| value_f(g, file, a).map_or(d, |x| x != 0.0);
    TextureRef {
        baked: None,
        file,
        resolved: resolve_texture(&written, scene, project).map(|p| p.display().to_string()),
        written,
        srgb: !matches!(space.to_lowercase().as_str(), "raw" | "linear" | "scene-linear rec.709-srgb" | "scene-linear rec 709/srgb" | "utility - raw"),
        coverage: two(g, "coverage", 1.0),
        translate_frame: two(g, "translateFrame", 0.0),
        rotate_frame: value_f(g, file, "rotateFrame").unwrap_or(0.0) as f32,
        repeat: two(g, "repeatUV", 1.0),
        offset: two(g, "offset", 0.0),
        rotate_uv: value_f(g, file, "rotateUV").unwrap_or(0.0) as f32,
        wrap: [flag(g, "wrapU", true), flag(g, "wrapV", true)],
        mirror: [flag(g, "mirrorU", false), flag(g, "mirrorV", false)],
        default_color: f3(value_v(g, file, "defaultColor").unwrap_or(DVec3::splat(0.5))),
        gain: f3(value_v(g, file, "colorGain").unwrap_or(DVec3::ONE)),
        color_offset: f3(value_v(g, file, "colorOffset").unwrap_or(DVec3::ZERO)),
        invert: flag(g, "invert", false),
        alpha_is_luminance: flag(g, "alphaIsLuminance", false),
    }
}

/// What feeds an attribute as the views draw it: the image file connected straight into it, with its placement, or
/// the network connected into it worked out into a picture over the UV square.
fn map_feeding(g: &mut Graph, id: NodeId, attr: &str, scene: Option<&str>, project: Option<&str>) -> Option<TextureRef> {
    let i = g.node(id)?.ty().attr_index(attr)? as i32;
    let whole = g.source(&PlugRef::new(id, i));
    let any = whole.or_else(|| (0..3).find_map(|k| g.source(&PlugRef::component(id, i, k))))?;
    if g.node(any.node).is_some_and(|n| n.ty().name == "file") {
        return Some(texture_ref(g, any.node, scene, project));
    }
    // One channel of a node into one channel of the colour: the node's colour is still the closest picture of it.
    let src = match whole {
        Some(s) => s,
        None => {
            let t = g.node(any.node)?.type_arc().clone();
            let out = ["outColor", "output", "outValue"].iter().find_map(|a| t.attr_index(a)).map_or(any.attr, |a| a as i32);
            PlugRef::new(any.node, out)
        }
    };
    let Some(pic) = crate::harold::swatch::flat_cached(g, &src, 256, scene, project) else {
        // Nothing the renderer shades: an image file further up, as before.
        return file_feeding(g, id, attr).map(|f| texture_ref(g, f, scene, project));
    };
    Some(TextureRef {
        resolved: Some(pic.key.clone()),
        baked: Some(pic),
        file: NO_NODE,
        written: String::new(),
        srgb: true,
        coverage: [1.0, 1.0],
        translate_frame: [0.0, 0.0],
        rotate_frame: 0.0,
        repeat: [1.0, 1.0],
        offset: [0.0, 0.0],
        rotate_uv: 0.0,
        wrap: [true, true],
        mirror: [false, false],
        default_color: [0.5; 3],
        gain: [1.0; 3],
        color_offset: [0.0; 3],
        invert: false,
        alpha_is_luminance: true,
    })
}

/// How one shading group draws. None when its surface shader is nothing Aztek knows: the shape draws as lambert1 would.
pub fn draw_material(g: &mut Graph, sg: NodeId, scene: Option<&str>, project: Option<&str>) -> Option<DrawMaterial> {
    let material = material_of_group(g, sg)?;
    let t = g.node(material)?.type_arc().clone();
    if !is_material_type(&t) {
        return None;
    }
    let (color_attr, trans_attr, unlit) = match t.name.as_str() {
        "surfaceShader" => ("outColor", "outTransparency", true),
        "standardSurface" => ("baseColor", "opacity", false),
        "aiStandardSurface" => ("base_color", "opacity", false),
        _ => ("color", "transparency", false),
    };
    let mut color = value_v(g, material, color_attr).unwrap_or(DVec3::splat(0.5));
    let mut transparency = value_v(g, material, trans_attr).unwrap_or(DVec3::ZERO);
    if t.name == "standardSurface" || t.name == "aiStandardSurface" {
        transparency = DVec3::ONE - transparency;
        color *= value_f(g, material, "base").unwrap_or(1.0);
        // Glass: drawn in its transmission colour, partly see-through, rather than as its (black) base.
        let snake = t.name == "aiStandardSurface";
        let trans = value_f(g, material, "transmission").unwrap_or(0.0).clamp(0.0, 1.0);
        if trans > 0.0 {
            let tc = value_v(g, material, if snake { "transmission_color" } else { "transmissionColor" }).unwrap_or(DVec3::ONE);
            color = color * (1.0 - trans) + tc * trans * 0.6;
            transparency = transparency.max(DVec3::splat(0.6 * trans));
        }
    }
    let color_map = map_feeding(g, material, color_attr, scene, project);
    let alpha_map = map_feeding(g, material, trans_attr, scene, project);
    // Without textures shown, a mapped colour is seen as the map's mean: the attribute alone is one sample of it.
    if let Some(pic) = color_map.as_ref().and_then(|t| t.baked.as_ref()) {
        let weight = if t.name == "standardSurface" || t.name == "aiStandardSurface" { value_f(g, material, "base").unwrap_or(1.0) } else { 1.0 };
        color = DVec3::new(pic.mean[0] as f64, pic.mean[1] as f64, pic.mean[2] as f64) * weight;
    }
    let specular = draw_specular(g, material, &t.name);
    Some(DrawMaterial {
        specular,
        group: sg,
        material,
        unlit,
        color: f3(color),
        ambient: f3(value_v(g, material, "ambientColor").unwrap_or(DVec3::ZERO)),
        incandescence: f3(value_v(g, material, "incandescence").unwrap_or(DVec3::ZERO)),
        diffuse: if unlit { 1.0 } else { value_f(g, material, "diffuse").unwrap_or(1.0) as f32 },
        transparency: if alpha_map.is_some() { 0.0 } else { (transparency.element_sum() / 3.0).clamp(0.0, 1.0) as f32 },
        color_map,
        alpha_map,
    })
}

/// Every shape's shading: for each its groups, with None for the whole shape or the faces they hold.
pub fn membership(g: &Graph) -> HashMap<NodeId, Vec<(NodeId, Option<Vec<u32>>)>> {
    let mut out: HashMap<NodeId, Vec<(NodeId, Option<Vec<u32>>)>> = HashMap::new();
    for sg in all_groups(g) {
        for m in os::members(g, sg) {
            let held = (m.kind == ComponentMode::Face).then_some(m.ids);
            out.entry(m.node).or_default().push((sg, held));
        }
    }
    out
}

/// One shape's shading groups, with what `membership` gives for each.
pub fn membership_of(g: &Graph, shape: NodeId) -> Vec<(NodeId, Option<Vec<u32>>)> {
    let mut out = Vec::new();
    for sg in all_groups(g) {
        for m in os::members(g, sg) {
            if m.node == shape {
                out.push((sg, (m.kind == ComponentMode::Face).then_some(m.ids)));
            }
        }
    }
    out
}

/// For every face of a mesh, the index into `groups` that draws it; usize::MAX where there are no groups at all. A face
/// no group names, as an edit's new face is, takes the group of a neighbour.
pub fn face_groups(mesh: &Mesh, groups: &[(NodeId, Option<Vec<u32>>)]) -> Vec<usize> {
    let count = mesh.face_capacity() as usize;
    let mut out = vec![usize::MAX; count];
    // A group holding the whole shape first, so that a file naming faces of it as well keeps them.
    for (k, (_, held)) in groups.iter().enumerate() {
        if held.is_none() {
            out.iter_mut().for_each(|x| *x = k);
        }
    }
    for (k, (_, held)) in groups.iter().enumerate() {
        for &f in held.iter().flatten() {
            if let Some(x) = out.get_mut(f as usize) {
                *x = k;
            }
        }
    }
    if groups.is_empty() {
        return out;
    }
    // The group holding the fewest faces spreads first: what an edit grew from a few assigned faces, as an extrusion's
    // sides, goes with them and not with the rest of the mesh.
    let mut order: Vec<usize> = (0..groups.len()).collect();
    order.sort_by_key(|&k| groups[k].1.as_ref().map_or(usize::MAX, Vec::len));
    for k in order {
        let mut front: Vec<u32> = (0..count as u32).filter(|&f| mesh.face_valid(f) && out[f as usize] == k).collect();
        while let Some(f) = front.pop() {
            for c in mesh.face_corners(f) {
                let mut r = mesh.corner_radial(c);
                while r != c && r != aztek_geo::INVALID {
                    let n = mesh.corner_face(r);
                    if out[n as usize] == usize::MAX {
                        out[n as usize] = k;
                        front.push(n);
                    }
                    r = mesh.corner_radial(r);
                }
            }
        }
    }
    for x in out.iter_mut().filter(|x| **x == usize::MAX) {
        *x = 0;
    }
    out
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

pub fn register(c: &mut Commands<Session>) {
    type Cmd = CommandDef<Session>;
    c.register(
        Cmd::new(
            "hyperShade",
            "Materials and what wears them: -assign gives the selection (objects or faces) a material, making its shading group if need be; -objects selects what wears one; -smn selects the materials of the selection; -lun and -ldn list a network's nodes.",
            hyper_shade,
        )
        .positional("objects, for -assign")
        .flag(f("assign", "a", ArgType::String, "The material to give the selection."))
        .flag(f("objects", "o", ArgType::String, "The material whose objects to select; empty for the selected one."))
        .flag(f("shaderNetworksSelectMaterialNodes", "smn", ArgType::Bool, "Select the materials of the selected objects."))
        .flag(f("listUpstreamNodes", "lun", ArgType::String, "The nodes upstream of this one."))
        .flag(f("listDownstreamNodes", "ldn", ArgType::String, "The nodes downstream of this one."))
        .flag(f("listUpstreamShaderNodes", "lus", ArgType::String, "The shading nodes upstream of this one."))
        .flag(f("listDownstreamShaderNodes", "lds", ArgType::String, "The shading nodes downstream of this one."))
        .flag(f("shaderNetwork", "snw", ArgType::Bool, "Select the networks of the selected materials."))
        .flag(f("duplicate", "d", ArgType::Bool, "Copy the selected materials with what feeds them."))
        .flag(f("resetGraphPosition", "rgp", ArgType::Bool, "Accepted: the Node Editor lays out for itself."))
        .flag(f("resetSwatch", "rs", ArgType::Bool, "Accepted: swatches redraw on change."))
        .flag(f("incremental", "inc", ArgType::Bool, "Accepted for existing scripts."))
        .flag(f("clearWorkArea", "cwa", ArgType::Bool, "Accepted: the Node Editor has its own work area."))
        .flag(f("networks", "n", ArgType::Bool, "Accepted for existing scripts.")),
    );
    c.register(
        Cmd::new("defaultNavigation", "Connects as dropping one node on another does: -connectToExisting -source file1 -destination lambert2.color. Plugs left out take the node's usual one.", default_navigation)
            .flag(f("connectToExisting", "ce", ArgType::Bool, "Connect the source to the destination."))
            .flag(f("source", "s", ArgType::String, "Node or plug to connect from."))
            .flag(f("destination", "d", ArgType::String, "Node or plug to connect to."))
            .flag(f("force", "f", ArgType::Bool, "Accepted: a connection is always replaced."))
            .flag(f("quiet", "qt", ArgType::Bool, "Accepted for scripts."))
            .flag(f("createNew", "cn", ArgType::Bool, "Accepted for scripts."))
            .flag(f("defaultTraversal", "dtv", ArgType::Bool, "Accepted for scripts."))
            .flag(f("defaultAttribute", "da", ArgType::Bool, "Accepted for scripts."))
            .flag(f("navigatorDecisionString", "nds", ArgType::String, "Accepted for scripts."))
            .flag(f("relatedNodes", "ren", ArgType::Bool, "Accepted for scripts."))
            .flag(f("ignore", "i", ArgType::Bool, "Accepted for scripts."))
            .flag(f("unignore", "u", ArgType::Bool, "Accepted for scripts."))
            .flag(f("delete", "del", ArgType::Bool, "Accepted for scripts.")),
    );
    extend(
        c,
        "sets",
        vec![f("noSurfaceShader", "nss", ArgType::Bool, "With -renderable: leave the new shading group with no material, rather than lambert1."), f("renderable", "r", ArgType::Bool, "Make a shading group; with -q, whether the set is one.")],
        |s, a, prev| {
            let edits = ["addElement", "forceElement", "include", "remove", "clear", "isMember", "isIntersecting", "union", "intersection", "subtract"];
            if a.bool("query") {
                let asks_renderable = a.bool("renderable") || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == "renderable"));
                if asks_renderable {
                    let Some(first) = a.named().into_iter().next() else { fail!("sets: name a set") };
                    let id = node_named(&s.graph, &first, "sets")?;
                    return Ok(json!(is_shading_group(&s.graph, id)));
                }
                return prev(s, a);
            }
            // Whether a transform is in a shading group is whether its shapes are.
            if let Some(name) = a.get("isMember").and_then(Value::as_str) {
                let g = &s.graph;
                let id = node_named(g, name, "sets")?;
                if is_shading_group(g, id) {
                    let items = a.named();
                    let wanted = shaded_members(g, &if items.is_empty() { os::from_selection(g) } else { os::parse(g, &items).map_err(CommandError::new)? })?;
                    let held = os::members(g, id);
                    let inside = !wanted.is_empty() && wanted.iter().all(|w| held.iter().any(|h| h.node == w.node && (h.kind == ComponentMode::Object || (h.kind == w.kind && w.ids.iter().all(|i| h.ids.contains(i))))));
                    return Ok(json!(inside));
                }
                return prev(s, a);
            }
            if !a.bool("renderable") || a.bool("edit") || edits.iter().any(|k| a.has(k)) {
                return prev(s, a);
            }
            make_defaults(&mut s.graph);
            let g = &mut s.graph;
            let t = types::find(SG_TYPE).unwrap();
            let sg = g.create_node(&t, &a.string("name", "set1"), NO_NODE, true)?;
            if !a.bool("noSurfaceShader") {
                if let Some(l) = g.find(DEFAULT_MATERIAL)? {
                    connect_material(g, l, sg)?;
                }
            }
            let items = a.named();
            if !a.bool("empty") {
                let list = if items.is_empty() { os::from_selection(g) } else { os::parse(g, &items).map_err(CommandError::new)? };
                assign(g, sg, &list)?;
            }
            Ok(json!(g.path(sg)))
        },
    );
    extend(
        c,
        "listSets",
        vec![
            f("type", "t", ArgType::Int, "1 for rendering sets, 2 for deformer sets."),
            f("allSets", "as", ArgType::Bool, "Every set, rendering sets too."),
            f("extendToShape", "ets", ArgType::Bool, "With -object: a transform stands for its shapes."),
        ],
        |s, a, prev| {
            let g = &s.graph;
            let plain = !a.has("type") && !a.bool("allSets") && !a.has("object");
            // An object can be a face: pCube1.f[2].
            let object = a.get("object").and_then(Value::as_str).map(str::to_string);
            if plain || a.bool("info") || a.has("quickSelect") {
                let r = prev(s, a)?;
                let g = &s.graph;
                return Ok(match r {
                    Value::Array(v) if !a.bool("info") => Value::Array(v.into_iter().filter(|x| !x.as_str().is_some_and(|n| g.find(n).ok().flatten().is_some_and(|id| is_shading_group(g, id)))).collect()),
                    r => r,
                });
            }
            let mut holding: Option<Vec<NodeId>> = None;
            if let Some(name) = &object {
                let nodes: Vec<NodeId> = match g.find(name).ok().flatten() {
                    Some(id) => {
                        let mut v = vec![id];
                        if a.bool("extendToShape") && !is_shaded_shape(g, id) {
                            v.extend(crate::dag::shapes(g, id, false));
                        }
                        v
                    }
                    None => os::parse(g, std::slice::from_ref(name)).map_err(CommandError::new)?.into_iter().map(|m| m.node).collect(),
                };
                holding = Some(nodes.iter().flat_map(|&n| os::sets_of(g, n)).collect());
            }
            let kind = a.get("type").and_then(Value::as_i64);
            let mut out = Vec::new();
            for set in os::all(g, None) {
                if holding.as_ref().is_some_and(|h| !h.contains(&set)) {
                    continue;
                }
                let render = is_shading_group(g, set);
                let deform = crate::deform_sets::deformer_of_set(g, set).is_some();
                let fits = match kind {
                    Some(1) => render,
                    Some(2) => deform,
                    _ => true,
                };
                if fits {
                    out.push(json!(g.path(set)));
                }
            }
            Ok(Value::Array(out))
        },
    );
    for name in ["connectAttr", "disconnectAttr"] {
        extend(c, name, vec![], |s, a, prev| {
            let r = prev(s, a)?;
            let g = &mut s.graph;
            // The destination is the last name given.
            let names = a.named();
            let sg = names.last().and_then(|n| g.plug(n).ok()).filter(|p| is_shading_group(g, p.node) && g.attr(p).name == "dagSetMembers").map(|p| p.node);
            if let Some(sg) = sg {
                follow_connections(g, sg)?;
            }
            Ok(r)
        });
    }
    extend(c, "delete", vec![], |s, a, prev| {
        // Named or selected: a node every scene has stays. History deletion leaves the nodes named alone.
        if !a.bool("constructionHistory") {
            let names = if a.named().is_empty() { s.graph.selection().iter().map(|&id| s.graph.path(id)).collect() } else { a.named() };
            for name in names {
                let g = &s.graph;
                let refused = g.find(&name).ok().flatten().and_then(|id| crate::defaults::refusal(g, id, "delete"));
                if let Some(why) = refused {
                    fail!("{why}");
                }
            }
        }
        prev(s, a)
    });
    extend(c, "rename", vec![], |s, a, prev| {
        let names = a.named();
        let target = if names.len() > 1 { names.first().cloned() } else { s.graph.selection().first().map(|&id| s.graph.path(id)) };
        if let Some(name) = target {
            let g = &s.graph;
            if let Some(why) = g.find(&name).ok().flatten().and_then(|id| crate::defaults::refusal(g, id, "rename")) {
                fail!("{why}");
            }
        }
        prev(s, a)
    });
    extend(c, "shadingNode", vec![], |s, a, prev| {
        if a.pos_str(0).is_some_and(|t| classification(t).is_some()) {
            make_defaults(&mut s.graph);
        }
        prev(s, a)
    });
}
