//! The node types rendering reads: render settings (`defaultRenderGlobals`, `defaultResolution`, `aiOptions` and its
//! AOV, driver and filter nodes), the full standard surface, the `ai*` shaders and lights scene files hold, and the
//! older materials and textures that are translated to the standard surface at render. Type and attribute names are
//! those files use, for compatibility; the interface calls the renderer Harold.

use std::any::Any;
use std::sync::Arc;

use glam::DVec3;

use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};

const IN: u32 = flags::INPUT | flags::STORABLE | flags::KEYABLE;
const STORE: u32 = flags::INPUT | flags::STORABLE;
const OUT: u32 = flags::OUTPUT;

fn base(name: &str, short: &str, ty: PlugType, default: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: default, flags: fl, ..Default::default() }
}
pub fn f(name: &str, short: &str, d: f64) -> AttrDef {
    base(name, short, PlugType::Float, PlugValue::Float(d), IN)
}
/// A float that is stored but not keyable: settings, not animation channels.
pub fn fs(name: &str, short: &str, d: f64) -> AttrDef {
    base(name, short, PlugType::Float, PlugValue::Float(d), STORE)
}
pub fn i(name: &str, short: &str, d: i64) -> AttrDef {
    base(name, short, PlugType::Int, PlugValue::Int(d), STORE)
}
pub fn b(name: &str, short: &str, d: bool) -> AttrDef {
    base(name, short, PlugType::Bool, PlugValue::Bool(d), STORE)
}
pub fn s(name: &str, short: &str, d: &str) -> AttrDef {
    base(name, short, PlugType::String, PlugValue::String(d.into()), STORE)
}
pub fn e(name: &str, short: &str, d: i64, names: &[&str]) -> AttrDef {
    let mut a = base(name, short, PlugType::Enum, PlugValue::Int(d), STORE);
    a.enum_names = names.iter().map(|x| x.to_string()).collect();
    a
}
/// A colour whose children are name + R, G, B and short + r, g, b.
pub fn c(name: &str, short: &str, d: [f64; 3], fl: u32) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::from_array(d)), fl | flags::COLOR);
    for (k, ch) in ["R", "G", "B"].iter().enumerate() {
        a.child_names[k] = format!("{name}{ch}");
        a.child_short[k] = format!("{short}{}", ch.to_lowercase());
    }
    a
}
pub fn v(name: &str, short: &str, fl: u32) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::ZERO), fl);
    for (k, ch) in ["X", "Y", "Z"].iter().enumerate() {
        a.child_names[k] = format!("{name}{ch}");
        a.child_short[k] = format!("{short}{}", ch.to_lowercase());
    }
    a
}
fn message() -> AttrDef {
    base("message", "msg", PlugType::Int, PlugValue::Int(0), OUT | flags::HIDDEN | flags::MESSAGE)
}
fn hidden(mut a: AttrDef) -> AttrDef {
    a.flags |= flags::HIDDEN;
    a
}
fn ranged(mut a: AttrDef, lo: f64, hi: f64) -> AttrDef {
    a.soft_min = Some(lo);
    a.soft_max = Some(hi);
    a
}
fn slots(name: &str, short: &str) -> AttrDef {
    base(name, short, PlugType::Int, PlugValue::Int(0), flags::INPUT | flags::ARRAY | flags::HIDDEN)
}

/// The standard surface's parameters: (camelCase long name, short name, snake_case name, colour?, default).
pub const STANDARD: &[(&str, &str, &str, bool, f64)] = &[
    ("base", "b", "base", false, 1.0),
    ("baseColor", "bc", "base_color", true, 0.8),
    ("diffuseRoughness", "dr", "diffuse_roughness", false, 0.0),
    ("metalness", "m", "metalness", false, 0.0),
    ("specular", "s", "specular", false, 1.0),
    ("specularColor", "sc", "specular_color", true, 1.0),
    ("specularRoughness", "sr2", "specular_roughness", false, 0.2),
    ("specularIOR", "sior", "specular_IOR", false, 1.5),
    ("specularAnisotropy", "sa", "specular_anisotropy", false, 0.0),
    ("specularRotation", "srot", "specular_rotation", false, 0.0),
    ("transmission", "t", "transmission", false, 0.0),
    ("transmissionColor", "tc", "transmission_color", true, 1.0),
    ("transmissionDepth", "td", "transmission_depth", false, 0.0),
    ("transmissionScatter", "ts", "transmission_scatter", true, 0.0),
    ("transmissionScatterAnisotropy", "tsa", "transmission_scatter_anisotropy", false, 0.0),
    ("transmissionDispersion", "tdis", "transmission_dispersion", false, 0.0),
    ("transmissionExtraRoughness", "ter", "transmission_extra_roughness", false, 0.0),
    ("subsurface", "sss", "subsurface", false, 0.0),
    ("subsurfaceColor", "sssc", "subsurface_color", true, 1.0),
    ("subsurfaceRadius", "sssr", "subsurface_radius", true, 1.0),
    ("subsurfaceScale", "ssss", "subsurface_scale", false, 1.0),
    ("subsurfaceAnisotropy", "sssa", "subsurface_anisotropy", false, 0.0),
    ("sheen", "sh", "sheen", false, 0.0),
    ("sheenColor", "shc", "sheen_color", true, 1.0),
    ("sheenRoughness", "shr", "sheen_roughness", false, 0.3),
    ("coat", "c", "coat", false, 0.0),
    ("coatColor", "cc", "coat_color", true, 1.0),
    ("coatRoughness", "cr", "coat_roughness", false, 0.1),
    ("coatAnisotropy", "ca", "coat_anisotropy", false, 0.0),
    ("coatRotation", "crot", "coat_rotation", false, 0.0),
    ("coatIOR", "cior", "coat_IOR", false, 1.5),
    ("coatAffectColor", "cac", "coat_affect_color", false, 0.0),
    ("coatAffectRoughness", "car", "coat_affect_roughness", false, 0.0),
    ("thinFilmThickness", "tft", "thin_film_thickness", false, 0.0),
    ("thinFilmIOR", "tfior", "thin_film_IOR", false, 1.5),
    ("emission", "e", "emission", false, 0.0),
    ("emissionColor", "ec", "emission_color", true, 1.0),
    ("opacity", "op", "opacity", true, 1.0),
];

/// The standard surface's attributes, camelCase (Maya's standardSurface) or snake_case (aiStandardSurface).
pub fn standard_attrs(snake: bool) -> Vec<AttrDef> {
    let mut out = Vec::new();
    for &(camel, short, sn, colour, d) in STANDARD {
        let (name, short) = if snake { (sn, sn) } else { (camel, short) };
        let mut a = if colour { c(name, short, [d; 3], IN) } else { f(name, short, d) };
        if !colour && !matches!(camel, "specularIOR" | "coatIOR" | "thinFilmIOR" | "thinFilmThickness" | "transmissionDepth" | "subsurfaceScale" | "emission") {
            a = ranged(a, 0.0, 1.0);
        }
        if colour && snake {
            for k in 0..3 {
                a.child_short[k] = a.child_names[k].clone();
            }
        }
        out.push(a);
    }
    if snake {
        out.push(b("thin_walled", "thin_walled", false));
        out.push(b("caustics", "caustics", false));
        out.push(b("internal_reflections", "internal_reflections", true));
        out.push(b("exit_to_background", "exit_to_background", false));
        out.push(f("indirect_diffuse", "indirect_diffuse", 1.0));
        out.push(f("indirect_specular", "indirect_specular", 1.0));
        out.push(v("normalCamera", "n", IN));
        out.push(v("coat_normal", "coat_normal", IN));
        out.push(v("tangent", "tangent", IN));
        out.push(s("aov_id1", "aov_id1", ""));
    } else {
        out.push(b("thinWalled", "tw", false));
        out.push(v("normalCamera", "n", IN));
        out.push(v("coatNormal", "cn", IN));
        out.push(v("tangentUCamera", "tu", IN));
    }
    out.push(c("outColor", "oc", [0.0; 3], OUT));
    out.push(c("outTransparency", "ot", [0.0; 3], OUT));
    out.push(message());
    out
}

struct Plain(fn(&mut NodeData));

impl NodeBehavior for Plain {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        (self.0)(d)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn none(_: &mut NodeData) {}

fn get_v(d: &NodeData, n: &str) -> DVec3 {
    d.ty().attr_index(n).map_or(DVec3::ZERO, |i| d.vec3(i))
}
fn get_f(d: &NodeData, n: &str) -> f64 {
    d.ty().attr_index(n).and_then(|i| d.value(i).scalar()).unwrap_or(0.0)
}
fn put(d: &mut NodeData, n: &str, val: impl Into<PlugValue>) {
    if let Some(i) = d.ty().attr_index(n) {
        d.set_output(i, val);
    }
}

/// The viewport's colour for an aiStandardSurface, as the standardSurface's.
fn ai_standard_out(d: &mut NodeData) {
    let col = get_v(d, "base_color") * get_f(d, "base");
    let o = get_v(d, "opacity");
    put(d, "outColor", col);
    put(d, "outTransparency", DVec3::ONE - o);
}

fn flat_out(d: &mut NodeData) {
    let col = get_v(d, "color");
    put(d, "outColor", col);
}

/// (name, classification, doc, attributes, compute). Types already registered are passed over.
fn table() -> Vec<(&'static str, &'static str, &'static str, Vec<AttrDef>, fn(&mut NodeData))> {
    let common_tex = || vec![v("uvCoord", "uv", flags::INPUT), c("outColor", "oc", [0.0; 3], OUT), base("outAlpha", "oa", PlugType::Float, PlugValue::Float(0.0), OUT), message()];
    let with = |mut a: Vec<AttrDef>, b: Vec<AttrDef>| {
        a.extend(b);
        a
    };
    let sampling = |name: &str, short: &str, d: i64| ranged(i(name, short, d), 0.0, 10.0);
    vec![
        ("aiStandardSurface", "shader/surface", "The physically based material, as files from other renderers name it: the same model as standardSurface.", standard_attrs(true), ai_standard_out),
        ("aiFlat", "shader/surface", "A flat colour, not lit.", vec![c("color", "color", [1.0; 3], IN), c("outColor", "oc", [0.0; 3], OUT), message()], flat_out),
        ("aiMixShader", "shader/surface", "Two materials blended by mix, or added.", vec![e("mode", "mode", 0, &["blend", "add"]), ranged(f("mix", "mix", 0.5), 0.0, 1.0), c("shader1", "shader1", [0.0; 3], IN), c("shader2", "shader2", [0.0; 3], IN), c("outColor", "oc", [0.0; 3], OUT), message()], none),
        ("aiShadowMatte", "shader/surface", "Invisible but for the shadows it receives: for putting renders over plates.", vec![b("background", "background", false), c("shadowColor", "shadowColor", [0.0; 3], IN), f("shadowOpacity", "shadowOpacity", 1.0), c("outColor", "oc", [0.0; 3], OUT), message()], none),
        ("aiUtility", "shader/surface", "A debug view: flat colour, normals, UVs or object ids.", vec![e("shadeMode", "shadeMode", 2, &["ndoteye", "lambert", "flat", "ambocc", "plastic", "metal"]), e("colorMode", "colorMode", 0, &["color", "ng", "ns", "n", "bary", "uv", "u", "v", "dpdu", "dpdv", "p", "prims", "wire", "polywire", "obj", "edgelength", "floatgrid", "reflectline", "bad_uvs", "nlights", "id"]), c("color", "color", [1.0; 3], IN), c("outColor", "oc", [0.0; 3], OUT), message()], flat_out),
        ("aiAmbientOcclusion", "shader/surface", "How open a point is: white where nothing is near, black in corners.", vec![sampling("samples", "samples", 3), f("spread", "spread", 1.0), f("falloff", "falloff", 0.0), f("nearClip", "nearClip", 0.0), f("farClip", "farClip", 100.0), c("white", "white", [1.0; 3], IN), c("black", "black", [0.0; 3], IN), b("invertNormals", "invertNormals", false), c("outColor", "oc", [0.0; 3], OUT), message()], none),
        ("aiCurvature", "texture/2d", "Convex and concave areas as a value, from rays near the point.", vec![e("output", "output", 0, &["convex", "concave", "both"]), sampling("samples", "samples", 3), f("radius", "radius", 0.1), f("spread", "spread", 1.0), f("threshold", "threshold", 0.0), f("bias", "bias", 0.5), f("multiply", "multiply", 1.0), c("outColor", "oc", [0.0; 3], OUT), base("outAlpha", "oa", PlugType::Float, PlugValue::Float(0.0), OUT), message()], none),
        ("aiImage", "texture/2d", "An image file as a texture, with its own colour space.", with(vec![s("filename", "filename", ""), s("colorSpace", "colorSpace", "sRGB"), e("filter", "filter", 3, &["closest", "bilinear", "bicubic", "smart_bicubic"]), b("ignoreMissingTextures", "ignoreMissingTextures", false), c("missingTextureColor", "missingTextureColor", [0.0; 3], IN), c("multiply", "multiply", [1.0; 3], IN), c("offset", "offset", [0.0; 3], IN), f("sscale", "sscale", 1.0), f("tscale", "tscale", 1.0), f("soffset", "soffset", 0.0), f("toffset", "toffset", 0.0), b("sflip", "sflip", false), b("tflip", "tflip", false), b("swapSt", "swapSt", false), s("uvset", "uvset", "")], common_tex()), none),
        ("aiNormalMap", "utility/general", "Bends the normal by a normal map's colours.", vec![v("input", "input", IN), v("tangent", "tangent", IN), v("normal", "normal", IN), b("order", "order", false), b("invertX", "invertX", false), b("invertY", "invertY", false), b("invertZ", "invertZ", false), b("colorToSigned", "colorToSigned", true), b("tangentSpace", "tangentSpace", true), f("strength", "strength", 1.0), v("outValue", "outValue", OUT), message()], none),
        ("aiBump2d", "utility/general", "Bends the normal by a height.", vec![f("bumpMap", "bumpMap", 0.0), f("bumpHeight", "bumpHeight", 1.0), v("normal", "normal", IN), v("outValue", "outValue", OUT), message()], none),
        ("aiAtmosphereVolume", "rendernode/arnold/shader/volume/atmosphere", "Fog through the whole scene, lit by the lights.", vec![f("density", "density", 0.0), sampling("samples", "samples", 5), c("color", "color", [1.0; 3], IN), f("anisotropy", "anisotropy", 0.0), message()], none),
        ("checker", "texture/2d", "Two colours in squares.", with(vec![c("color1", "c1", [1.0; 3], IN), c("color2", "c2", [0.0; 3], IN), ranged(f("contrast", "ct", 1.0), 0.0, 1.0), c("defaultColor", "dc", [0.5; 3], IN), c("colorGain", "cg", [1.0; 3], IN), c("colorOffset", "co", [0.0; 3], IN), f("alphaGain", "ag", 1.0), f("alphaOffset", "ao", 0.0), b("alphaIsLuminance", "ail", false), b("invert", "i", false)], common_tex()), none),
        ("noise", "texture/2d", "Fractal noise.", with(vec![ranged(f("threshold", "th", 0.0), 0.0, 1.0), ranged(f("amplitude", "a", 1.0), 0.0, 1.0), ranged(f("ratio", "ra", 0.707), 0.0, 1.0), f("frequencyRatio", "fr", 2.0), i("depthMax", "dm", 3), f("frequency", "fq", 8.0), b("inflection", "in", false), f("time", "ti", 0.0), e("noiseType", "nty", 1, &["Perlin Noise", "Billow", "Wave", "Wispy", "SpaceTime"]), c("defaultColor", "dc", [0.5; 3], IN), c("colorGain", "cg", [1.0; 3], IN), c("colorOffset", "co", [0.0; 3], IN), f("alphaGain", "ag", 1.0), f("alphaOffset", "ao", 0.0), b("invert", "i", false)], common_tex()), none),
        ("layeredTexture", "texture/2d", "Textures stacked, each with an alpha and a blend mode.", vec![c("inputs.color", "cs.c", [0.0; 3], IN | flags::ARRAY), f("inputs.alpha", "cs.a", 1.0), e("inputs.blendMode", "cs.bm", 1, &["None", "Over", "In", "Out", "Add", "Subtract", "Multiply", "Difference", "Lighten", "Darken", "Saturate", "Desaturate", "Illuminate"]), b("inputs.isVisible", "cs.iv", true), b("alphaIsLuminance", "ail", false), c("outColor", "oc", [0.0; 3], OUT), base("outAlpha", "oa", PlugType::Float, PlugValue::Float(0.0), OUT), message()], none),
        ("layeredShader", "shader/surface", "Materials stacked, each seen through the transparency of those above.", vec![c("inputs.color", "cs.c", [0.0; 3], IN | flags::ARRAY), c("inputs.transparency", "cs.t", [0.0; 3], IN | flags::ARRAY), c("inputs.glowColor", "cs.g", [0.0; 3], IN | flags::ARRAY), e("compositingFlag", "cmp", 0, &["Layer Shader", "Layer Texture"]), c("outColor", "oc", [0.0; 3], OUT), c("outTransparency", "ot", [0.0; 3], OUT), message()], none),
        ("rampShader", "shader/surface", "A material whose colour follows a ramp over how much light falls on it.", vec![c("color", "c", [0.5; 3], IN), ranged(f("diffuse", "dc", 0.8), 0.0, 1.0), c("transparency", "it", [0.0; 3], IN), c("incandescence", "ic", [0.0; 3], IN), f("specularity", "spl", 0.5), f("eccentricity", "ecc", 0.3), c("specularColor", "sc", [1.0; 3], IN), f("reflectivity", "rfl", 0.5), v("normalCamera", "n", IN), c("outColor", "oc", [0.0; 3], OUT), c("outTransparency", "ot", [0.0; 3], OUT), message()], none),
        ("displacementShader", "shader/displacement", "Moves a surface along its normal by a height, at render.", vec![f("displacement", "d", 0.0), v("vectorDisplacement", "vd", IN), f("scale", "sc", 1.0), e("vectorEncoding", "ven", 0, &["Floating Point Absolute", "Signed Encoding"]), e("vectorSpace", "vs", 1, &["World", "Object", "Tangent"]), f("aiDisplacementZeroValue", "aidisplacement_zero_value", 0.0), v("tangent", "tan", IN), message()], none),
        ("mia_material_x", "shader/surface", "An architectural material from an older renderer, read and translated.", mia_attrs(), none),
        ("mia_material", "shader/surface", "An architectural material from an older renderer, read and translated.", mia_attrs(), none),
        ("mia_material_x_passes", "shader/surface", "An architectural material from an older renderer, read and translated.", mia_attrs(), none),
    ]
}

fn mia_attrs() -> Vec<AttrDef> {
    vec![
        f("diffuse_weight", "diffuse_weight", 1.0),
        c("diffuse", "diffuse", [0.5; 3], IN),
        f("diffuse_roughness", "diffuse_roughness", 0.0),
        f("reflectivity", "reflectivity", 0.6),
        c("refl_color", "refl_color", [1.0; 3], IN),
        f("refl_gloss", "refl_gloss", 1.0),
        b("refl_is_metal", "refl_is_metal", false),
        f("transparency", "transparency", 0.0),
        c("refr_color", "refr_color", [1.0; 3], IN),
        f("refr_gloss", "refr_gloss", 1.0),
        f("refr_ior", "refr_ior", 1.4),
        f("translucency", "translucency", 0.0),
        c("translucency_color", "translucency_color", [0.7; 3], IN),
        f("anisotropy", "anisotropy", 1.0),
        f("anisotropy_rotation", "anisotropy_rotation", 0.0),
        b("brdf_fresnel", "brdf_fresnel", false),
        f("brdf_0_degree_refl", "brdf_0_degree_refl", 0.2),
        f("cutout_opacity", "cutout_opacity", 1.0),
        c("additional_color", "additional_color", [0.0; 3], IN),
        b("thin_walled", "thin_walled", false),
        v("bump", "bump", IN),
        c("outColor", "oc", [0.0; 3], OUT),
        message(),
    ]
}

/// Render settings nodes and Arnold's lights, which files name and Aztek keeps.
fn settings_table() -> Vec<(&'static str, &'static str, Vec<AttrDef>)> {
    let filters = ["blackman_harris", "box", "catrom", "closest", "cone", "cook", "cubic", "disk", "farthest", "gaussian", "heatmap", "mitnet", "sinc", "triangle", "variance", "video"];
    let _ = filters;
    vec![
        (
            "renderGlobals",
            "Render settings every scene has, as defaultRenderGlobals: output name, format, frames, which renderer.",
            vec![
                s("currentRenderer", "ren", "arnold"),
                s("imageFilePrefix", "ifp", ""),
                i("imageFormat", "outf", 51),
                s("imfPluginKey", "imfkey", "exr"),
                b("animation", "an", false),
                f("startFrame", "fs", 1.0),
                f("endFrame", "ef", 10.0),
                f("byFrameStep", "bfs", 1.0),
                i("extensionPadding", "pad", 4),
                b("putFrameBeforeExt", "peie", true),
                i("periodInExt", "peie2", 1),
                b("outFormatControl", "oft", false),
                b("useFrameExt", "ufe", false),
                b("useMayaFileName", "umfn", true),
                b("modifyExtension", "mex", false),
                b("enableDefaultLight", "edl", true),
                s("preMel", "prm", ""),
                s("postMel", "pom", ""),
                s("preRenderMel", "prrm", ""),
                s("postRenderMel", "porm", ""),
                b("motionBlur", "mb", false),
                b("multiCamNamingMode", "mcnm", false),
                s("bufferName", "bfn", ""),
                b("composite", "cpt", false),
                i("hyperShadeBinList", "hsbl", 0),
                hidden(i("renderVersion", "rv", 0)),
                message(),
            ],
        ),
        (
            "resolution",
            "The image size every scene has, as defaultResolution.",
            vec![i("width", "w", 960), i("height", "h", 540), f("deviceAspectRatio", "dar", 1.7777777), f("pixelAspect", "pa", 1.0), b("aspectLock", "al", false), f("dotsPerInch", "dpi", 72.0), b("lockDeviceAspectRatio", "ldar", false), message()],
        ),
        (
            "aiOptions",
            "Harold's render settings: samples per ray type, ray depths, adaptive sampling, clamping, motion blur and the rest.",
            vec![
                ranged(i("AASamples", "AASamples", 3), 1.0, 10.0),
                ranged(i("GIDiffuseSamples", "GIDiffuseSamples", 2), 0.0, 10.0),
                ranged(i("GISpecularSamples", "GISpecularSamples", 2), 0.0, 10.0),
                ranged(i("GITransmissionSamples", "GITransmissionSamples", 2), 0.0, 10.0),
                ranged(i("GISssSamples", "GISssSamples", 2), 0.0, 10.0),
                ranged(i("GIVolumeSamples", "GIVolumeSamples", 2), 0.0, 10.0),
                i("GITotalDepth", "GITotalDepth", 10),
                i("GIDiffuseDepth", "GIDiffuseDepth", 1),
                i("GISpecularDepth", "GISpecularDepth", 1),
                i("GITransmissionDepth", "GITransmissionDepth", 8),
                i("GIVolumeDepth", "GIVolumeDepth", 0),
                i("autoTransparencyDepth", "autoTransparencyDepth", 10),
                f("lowLightThreshold", "lowLightThreshold", 0.001),
                b("use_sample_clamp", "use_sample_clamp", false),
                b("use_sample_clamp_AOVs", "use_sample_clamp_AOVs", false),
                f("AASampleClamp", "AASampleClamp", 10.0),
                f("indirectSampleClamp", "indirectSampleClamp", 10.0),
                b("enableAdaptiveSampling", "enableAdaptiveSampling", false),
                i("AASamplesMax", "AASamplesMax", 20),
                f("AAAdaptiveThreshold", "AAAdaptiveThreshold", 0.015),
                i("AASeed", "AASeed", 1),
                b("lock_sampling_noise", "lock_sampling_noise", false),
                b("motion_blur_enable", "motion_blur_enable", false),
                b("mb_object_deform_enable", "mb_object_deform_enable", true),
                b("mb_camera_enable", "mb_camera_enable", false),
                i("motion_steps", "motion_steps", 2),
                f("motion_frames", "motion_frames", 0.5),
                e("range_type", "range_type", 1, &["Start On Frame", "Center On Frame", "End On Frame", "Custom"]),
                f("motion_start", "motion_start", -0.25),
                f("motion_end", "motion_end", 0.25),
                i("textureMaxMemoryMB", "textureMaxMemoryMB", 4096),
                b("textureAutomip", "textureAutomip", true),
                b("textureAcceptUntiled", "textureAcceptUntiled", true),
                b("autotx", "autotx", true),
                i("maxSubdivisions", "maxSubdivisions", 999),
                b("ignoreTextures", "ignoreTextures", false),
                b("ignoreShaders", "ignoreShaders", false),
                b("ignoreAtmosphere", "ignoreAtmosphere", false),
                b("ignoreLights", "ignoreLights", false),
                b("ignoreShadows", "ignoreShadows", false),
                b("ignoreSubdivision", "ignoreSubdivision", false),
                b("ignoreDisplacement", "ignoreDisplacement", false),
                b("ignoreBump", "ignoreBump", false),
                b("ignoreMotionBlur", "ignoreMotionBlur", false),
                b("ignoreDof", "ignoreDof", false),
                b("ignoreSss", "ignoreSss", false),
                b("ignoreSmoothing", "ignoreSmoothing", false),
                i("threads_autodetect", "threads_autodetect", 1),
                i("threads", "threads", 0),
                i("bucketSize", "bucketSize", 64),
                e("bucketScanning", "bucketScanning", 0, &["top", "left", "random", "spiral", "hilbert"]),
                b("abortOnError", "abortOnError", true),
                e("log_verbosity", "log_verbosity", 1, &["Errors", "Warnings", "Info", "Debug"]),
                s("log_filename", "log_filename", ""),
                b("aovMode", "aovMode", true),
                b("denoiseBeauty", "denoiseBeauty", false),
                c("background", "background", [0.0; 3], IN),
                hidden(slots("atmosphere", "atmosphere")),
                hidden(slots("aovList", "aovs")),
                hidden(slots("drivers", "drivers")),
                hidden(slots("drvr", "drvr")),
                hidden(slots("filt", "filt")),
                s("version", "version", ""),
                message(),
            ],
        ),
        ("aiAOV", "One output beside the beauty: its name, data type and which light group or lobe it holds.", vec![s("name", "name", ""), i("type", "type", 5), b("enabled", "enabled", true), s("lightGroupsList", "lightGroupsList", ""), b("lightGroups", "lightGroups", false), b("globalAov", "globalAov", true), hidden(slots("outputs", "outputs")), message()]),
        ("aiAOVDriver", "Where AOVs go: a file format and its settings, or the render view.", vec![s("ai_translator", "ai_translator", "exr"), e("exrCompression", "exrCompression", 3, &["none", "rle", "zips", "zip", "piz", "pxr24", "b44", "b44a", "dwaa", "dwab"]), b("halfPrecision", "halfPrecision", false), b("mergeAOVs", "mergeAOVs", false), b("tiled", "tiled", false), i("outputMode", "output_mode", 2), b("prefix", "prefix", false), i("quality", "quality", 100), message()]),
        ("aiAOVFilter", "How samples are weighted into pixels: the filter and its width.", vec![s("ai_translator", "ai_translator", "gaussian"), f("width", "width", 2.0), message()]),
    ]
}

/// The names of shape_render_attrs, for files.
pub const SHAPE_RENDER_NAMES: &[&str] = &[
    "castsShadows", "receiveShadows", "primaryVisibility", "visibleInReflections", "visibleInRefractions", "doubleSided", "smoothShading", "motionBlur", "aiSubdivType", "aiSubdivIterations", "aiSubdivAdaptiveMetric", "aiSubdivPixelError",
    "aiSubdivUvSmoothing", "aiSubdivSmoothDerivs", "aiOpaque", "aiMatte", "aiSelfShadows", "aiVisibleInDiffuseReflection", "aiVisibleInSpecularReflection", "aiVisibleInDiffuseTransmission", "aiVisibleInSpecularTransmission", "aiVisibleInVolume",
    "aiDispHeight", "aiDispZeroValue", "aiDispPadding", "aiDispAutobump", "aiTraceSets", "aiSssSetname", "aiExportTangents", "aiExportColors", "aiExportRefPoints", "aiExportRefNormals", "aiMotionVectorSource",
];

/// What renders need on a mesh or NURBS surface: Maya's render stats and the subdivision and visibility settings files
/// hold for Arnold. Added at the end of each type's attributes.
pub fn shape_render_attrs() -> Vec<AttrDef> {
    vec![
        b("castsShadows", "csh", true),
        b("receiveShadows", "rcsh", true),
        b("primaryVisibility", "vis", true),
        b("visibleInReflections", "vir", true),
        b("visibleInRefractions", "vif", true),
        b("doubleSided", "ds", true),
        b("smoothShading", "smo", true),
        b("motionBlur", "mb", true),
        e("aiSubdivType", "aisubdiv_type", 0, &["none", "catclark", "linear"]),
        ranged(i("aiSubdivIterations", "aisubdiv_iterations", 1), 0.0, 6.0),
        e("aiSubdivAdaptiveMetric", "aisubdiv_adaptive_metric", 0, &["auto", "edge_length", "flatness"]),
        fs("aiSubdivPixelError", "aisubdiv_pixel_error", 0.0),
        e("aiSubdivUvSmoothing", "aisubdiv_uv_smoothing", 0, &["pin_corners", "pin_borders", "linear", "smooth"]),
        b("aiSubdivSmoothDerivs", "aisubdiv_smooth_derivs", false),
        b("aiOpaque", "aiopaque", true),
        b("aiMatte", "aimatte", false),
        b("aiSelfShadows", "aiself_shadows", true),
        b("aiVisibleInDiffuseReflection", "aivisible_in_diffuse_reflection", true),
        b("aiVisibleInSpecularReflection", "aivisible_in_specular_reflection", true),
        b("aiVisibleInDiffuseTransmission", "aivisible_in_diffuse_transmission", true),
        b("aiVisibleInSpecularTransmission", "aivisible_in_specular_transmission", true),
        b("aiVisibleInVolume", "aivisible_in_volume", true),
        fs("aiDispHeight", "aidisp_height", 1.0),
        fs("aiDispZeroValue", "aidisp_zero_value", 0.0),
        fs("aiDispPadding", "aidisp_padding", 0.0),
        b("aiDispAutobump", "aidisp_autobump", false),
        s("aiTraceSets", "aitrace_sets", ""),
        s("aiSssSetname", "aisss_setname", ""),
        b("aiExportTangents", "aiexport_tangents", false),
        b("aiExportColors", "aiexport_colors", false),
        b("aiExportRefPoints", "aiexport_ref_points", false),
        b("aiExportRefNormals", "aiexport_ref_normals", false),
        b("aiMotionVectorSource", "aimotion_vector_source", false),
    ]
}

/// What renders read on every light, beside Maya's own attributes.
pub fn light_render_attrs() -> Vec<AttrDef> {
    vec![
        fs("aiExposure", "ai_exposure", 0.0),
        ranged(i("aiSamples", "ai_samples", 1), 0.0, 10.0),
        fs("aiRadius", "ai_radius", 0.0),
        fs("aiAngle", "ai_angle", 0.0),
        b("aiNormalize", "ai_normalize", true),
        b("aiCastShadows", "ai_cast_shadows", true),
        fs("aiShadowDensity", "ai_shadow_density", 1.0),
        b("aiCastVolumetricShadows", "ai_cast_volumetric_shadows", true),
        fs("aiDiffuse", "ai_diffuse", 1.0),
        fs("aiSpecular", "ai_specular", 1.0),
        fs("aiSss", "ai_sss", 1.0),
        fs("aiIndirect", "ai_indirect", 1.0),
        fs("aiVolume", "ai_volume", 1.0),
        fs("aiTransmission", "ai_transmission", 1.0),
        i("aiMaxBounces", "ai_max_bounces", 999),
        ranged(i("aiVolumeSamples", "ai_volume_samples", 2), 0.0, 10.0),
        s("aiAov", "ai_aov", "default"),
        fs("aiSpread", "ai_spread", 1.0),
        fs("aiRoundness", "ai_roundness", 0.0),
        fs("aiSoftEdge", "ai_soft_edge", 0.0),
        i("aiResolution", "ai_resolution", 512),
        e("aiDecayType", "ai_decay_type", 1, &["constant", "quadratic"]),
        fs("aiCamera", "ai_camera", 0.0),
    ]
}

/// Arnold's lights: (name, doc, own attributes). Their base is renderLight.
pub fn ai_lights() -> Vec<(&'static str, &'static str, Vec<AttrDef>)> {
    vec![
        ("aiSkyDomeLight", "Light from a sphere around the scene: a colour or an HDR image, importance sampled.", vec![e("format", "format", 2, &["mirrored_ball", "angular", "latlong"]), f("camera", "camera", 1.0), f("aiPortalMode", "portal_mode", 0.0), base("inMesh", "inMesh", PlugType::Mesh, PlugValue::Mesh(None), flags::INPUT | flags::HIDDEN)]),
        ("aiAreaLight", "Light from a quad, disc or cylinder.", vec![e("aiTranslator", "ai_translator", 0, &["quad", "disk", "cylinder"]), f("camera", "camera", 0.0)]),
        ("aiMeshLight", "Light from the surface of a mesh.", vec![b("lightVisible", "light_visible", false), base("inMesh", "inMesh", PlugType::Mesh, PlugValue::Mesh(None), flags::INPUT | flags::HIDDEN)]),
        ("aiPhotometricLight", "Light from a point shaped by a measured profile (IES file).", vec![s("aiFilename", "ai_filename", "")]),
    ]
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for (name, _class, doc, attrs, compute) in table() {
            if types::find(name).is_some() {
                continue;
            }
            types::register_type(NodeType { name: name.into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(move || Box::new(Plain(compute)))), ignores_added_attrs: true, ..Default::default() }).expect(name);
        }
        for (name, doc, attrs) in settings_table() {
            if types::find(name).is_some() {
                continue;
            }
            types::register_type(NodeType { name: name.into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(|| Box::new(Plain(none)))), ignores_added_attrs: true, ..Default::default() }).expect(name);
        }
    });
}

/// classification for shadingNode and the Hypershade.
pub fn classification(ty: &str) -> Option<&'static str> {
    table().into_iter().find(|t| t.0 == ty).map(|t| t.1)
}

/// Default nodes rendering keeps in every scene, created once with the scene's other defaults.
pub const DEFAULT_NODES: &[(&str, &str)] = &[("defaultRenderGlobals", "renderGlobals"), ("defaultResolution", "resolution"), ("defaultArnoldRenderOptions", "aiOptions"), ("defaultArnoldFilter", "aiAOVFilter"), ("defaultArnoldDriver", "aiAOVDriver"), ("defaultArnoldDisplayDriver", "aiAOVDriver")];

/// Written with createNode -s, as Maya writes nodes a plug-in shares between scenes.
pub fn is_shared_node(name: &str) -> bool {
    name.starts_with("defaultArnold") && is_default_node(name)
}

pub fn is_default_node(name: &str) -> bool {
    DEFAULT_NODES.iter().any(|d| d.0 == name)
}

/// Makes the render settings nodes if the scene hasn't them yet.
pub fn make_defaults(g: &mut crate::graph::Graph) {
    register_types();
    for (name, ty) in DEFAULT_NODES {
        if g.find(name).ok().flatten().is_some() {
            continue;
        }
        if let Some(t) = types::find(ty) {
            let _ = g.create_node(&t, name, crate::plug::NO_NODE, false);
        }
    }
}
