//! The reference's procedural textures and the nodes that place them: the 2D patterns (bulge, cloth, fractal, grid,
//! mountain, water), the 3D ones (brownian, cloud, crater, granite, leather, marble, rock, snow, solidFractal, stucco,
//! volumeNoise, wood), `place3dTexture`, and the utilities `samplerInfo`, `gammaCorrect`, `contrast` and `bump3d`.
//! Attribute names, short names and defaults are the reference's node documentation's (2025 pages, read 12/11/2026),
//! so scene files load into them. Harold shades them per sample; in the graph their outputs are their default colour.

use std::any::Any;
use std::sync::Arc;

use glam::{DMat4, DVec3};

use crate::harold_nodes::{b, c, e, f, i, v};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, PlugType, PlugValue};

const IN: u32 = flags::INPUT | flags::STORABLE | flags::KEYABLE;
const STORE: u32 = flags::INPUT | flags::STORABLE;
const OUT: u32 = flags::OUTPUT;

fn base(name: &str, short: &str, ty: PlugType, default: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: default, flags: fl, ..Default::default() }
}

fn message() -> AttrDef {
    base("message", "msg", PlugType::Int, PlugValue::Int(0), OUT | flags::HIDDEN | flags::MESSAGE)
}

/// Two numbers under one name, each with its own.
fn pair(name: &str, short: &str, kids: [(&str, &str); 2], d: [f64; 2], fl: u32) -> AttrDef {
    let mut a = base(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::new(d[0], d[1], 0.0)), fl);
    for (k, (long, sh)) in kids.iter().enumerate() {
        a.child_names[k] = long.to_string();
        a.child_short[k] = sh.to_string();
    }
    a
}

/// Three numbers under one name, with a default.
fn triple(name: &str, short: &str, d: [f64; 3], fl: u32) -> AttrDef {
    let mut a = v(name, short, fl);
    a.default_value = PlugValue::Vec3(DVec3::from_array(d));
    a
}

fn fd(name: &str, short: &str, d: f64, lo: f64, hi: f64) -> AttrDef {
    let mut a = f(name, short, d);
    a.soft_min = Some(lo);
    a.soft_max = Some(hi);
    a
}

/// What every texture has: the colour balance, and its two outputs.
fn balance() -> Vec<AttrDef> {
    vec![
        c("defaultColor", "dc", [0.5; 3], IN),
        c("colorGain", "cg", [1.0; 3], IN),
        c("colorOffset", "co", [0.0; 3], IN),
        f("alphaGain", "ag", 1.0),
        f("alphaOffset", "ao", 0.0),
        b("alphaIsLuminance", "ail", false),
        b("invert", "i", false),
        base("filter", "f", PlugType::Float, PlugValue::Float(1.0), STORE | flags::HIDDEN),
        base("filterOffset", "fo", PlugType::Float, PlugValue::Float(0.0), STORE | flags::HIDDEN),
        c("outColor", "oc", [0.0; 3], OUT),
        base("outAlpha", "oa", PlugType::Float, PlugValue::Float(0.0), OUT),
        message(),
    ]
}

fn tex2d(mut own: Vec<AttrDef>) -> Vec<AttrDef> {
    own.extend(balance());
    own.push(pair("uvCoord", "uv", [("uCoord", "u"), ("vCoord", "v")], [0.0, 0.0], flags::INPUT));
    own.push(pair("uvFilterSize", "fs", [("uvFilterSizeX", "fsx"), ("uvFilterSizeY", "fsy")], [0.0, 0.0], flags::INPUT | flags::HIDDEN));
    own
}

fn tex3d(mut own: Vec<AttrDef>) -> Vec<AttrDef> {
    own.extend(balance());
    own.push(b("local", "lo", false));
    own.push(b("wrap", "w", true));
    own.push(f("blend", "b", 0.0));
    own.push(base("placementMatrix", "pm", PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY), flags::INPUT | flags::STORABLE));
    own
}

fn depth(lo: f64, hi: f64) -> AttrDef {
    pair("depth", "d", [("depthMin", "dmn"), ("depthMax", "dmx")], [lo, hi], IN)
}

fn ripples() -> AttrDef {
    triple("ripples", "r", [1.0; 3], IN)
}

struct Plain;

impl NodeBehavior for Plain {
    /// The graph's value of a texture is its default colour: what it is at a point is the renderer's to say.
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let ty = d.ty().clone();
        if let (Some(dc), Some(oc)) = (ty.attr_index("defaultColor"), ty.attr_index("outColor")) {
            let c = d.vec3(dc);
            d.set_output(oc, PlugValue::Vec3(c));
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// (name, classification, what it is, attributes).
fn table() -> Vec<(&'static str, &'static str, &'static str, Vec<AttrDef>)> {
    vec![
        ("bulge", "texture/2d", "A raised middle with a level border, tile after tile: for bump and displacement.", tex2d(vec![fd("uWidth", "uw", 0.1, 0.0, 1.0), fd("vWidth", "vw", 0.1, 0.0, 1.0)])),
        (
            "cloth",
            "texture/2d",
            "Woven threads: a colour for those along U, one for those along V, one for the gaps.",
            tex2d(vec![
                c("gapColor", "gc", [0.0; 3], IN),
                c("uColor", "uc", [1.0; 3], IN),
                c("vColor", "vc", [0.5; 3], IN),
                fd("uWidth", "uwi", 0.75, 0.0, 1.0),
                fd("vWidth", "vwi", 0.75, 0.0, 1.0),
                fd("uWave", "uwa", 0.0, 0.0, 0.5),
                fd("vWave", "vwa", 0.0, 0.0, 0.5),
                fd("randomness", "r", 0.0, 0.0, 1.0),
                fd("widthSpread", "ws", 0.0, 0.0, 1.0),
                fd("brightSpread", "bs", 0.0, 0.0, 1.0),
            ]),
        ),
        (
            "fractal",
            "texture/2d",
            "Fractal noise over UV: sand, rust, cloud.",
            tex2d(vec![
                fd("amplitude", "a", 1.0, 0.0, 1.0),
                fd("threshold", "th", 0.0, 0.0, 1.0),
                fd("ratio", "ra", 0.707, 0.0, 1.0),
                fd("frequencyRatio", "fr", 2.0, 1.0, 10.0),
                fd("levelMin", "lmn", 0.0, 0.0, 25.0),
                fd("levelMax", "lmx", 9.0, 0.0, 25.0),
                fd("bias", "bs", 0.0, -1.0, 1.0),
                b("inflection", "in", false),
                b("animated", "an", false),
                f("time", "ti", 0.0),
                fd("timeRatio", "tr", 2.0, 1.0, 10.0),
            ]),
        ),
        ("grid", "texture/2d", "Lines of one colour over a filler of another.", tex2d(vec![c("lineColor", "lc", [1.0; 3], IN), c("fillerColor", "fc", [0.0; 3], IN), fd("uWidth", "uw", 0.1, 0.0, 1.0), fd("vWidth", "vw", 0.1, 0.0, 1.0), fd("contrast", "c", 1.0, 0.0, 1.0)])),
        (
            "mountain",
            "texture/2d",
            "Craggy rock with snow on what is high and not too steep. Its alpha is the height: use it for the bump as well.",
            tex2d(vec![
                c("snowColor", "sc", [1.0; 3], IN),
                c("rockColor", "rc", [0.262, 0.102, 0.0], IN),
                fd("amplitude", "a", 1.0, 0.0, 1.0),
                fd("snowRoughness", "sr", 0.4, 0.0, 1.0),
                fd("rockRoughness", "rr", 0.707, 0.0, 1.0),
                fd("boundary", "bo", 1.0, 0.0, 1.0),
                fd("snowAltitude", "sa", 0.5, 0.0, 1.0),
                fd("snowDropoff", "sd", 2.0, 0.0, 10.0),
                fd("snowSlope", "ss", 0.8, 0.0, 3.0),
                fd("depthMax", "dmx", 20.0, 0.0, 40.0),
            ]),
        ),
        (
            "water",
            "texture/2d",
            "Waves in a line and ripples in rings from a drop: for bump and displacement.",
            tex2d(vec![
                i("numberOfWaves", "nw", 8),
                f("waveTime", "wt", 0.0),
                fd("waveVelocity", "wv", 1.0, 0.0, 1.0),
                fd("waveAmplitude", "wa", 0.05, 0.0, 1.0),
                fd("waveFrequency", "wf", 4.0, 0.0, 20.0),
                fd("subWaveFrequency", "swf", 0.125, 0.0, 1.0),
                fd("smoothness", "s", 2.0, 0.0, 5.0),
                pair("windUV", "wi", [("windU", "wiu"), ("windV", "wiv")], [1.0, 0.0], IN),
                f("rippleTime", "rt", 0.0),
                fd("rippleFrequency", "rf", 25.0, 0.0, 20.0),
                fd("rippleAmplitude", "ra", 0.05, 0.0, 1.0),
                fd("dropSize", "ds", 0.3, 0.0, 1.0),
                pair("rippleOrigin", "rc", [("rippleOriginU", "rcu"), ("rippleOriginV", "rcv")], [0.5, 0.5], IN),
                fd("groupVelocity", "gv", 1.0, 0.0, 1.0),
                fd("phaseVelocity", "pv", 2.5, 0.0, 10.0),
                fd("spreadStart", "ss", 0.005, 0.0, 1.0),
                fd("spreadRate", "sr", 0.3, 0.0, 1.0),
                b("reflectionBox", "rb", false),
                pair("boxMin", "bmn", [("boxMinU", "bmnu"), ("boxMinV", "bmnv")], [0.0, 0.0], IN),
                pair("boxMax", "bmx", [("boxMaxU", "bmxu"), ("boxMaxV", "bmxv")], [1.0, 1.0], IN),
                b("fast", "fa", true),
            ]),
        ),
        ("brownian", "texture/3d", "Rough, bumpy noise through space: mostly for bump.", tex3d(vec![fd("lacunarity", "l", 4.0, 0.0, 10.0), fd("increment", "ic", 0.1, 0.0, 1.0), fd("octaves", "oct", 3.0, 0.0, 10.0), triple("weight3d", "w3", [1.0; 3], IN)])),
        (
            "cloud",
            "texture/3d",
            "Cloud, smoke or steam through space, for colour or transparency.",
            tex3d(vec![
                c("color1", "c1", [0.0; 3], IN),
                c("color2", "c2", [1.0; 3], IN),
                fd("contrast", "c", 0.5, 0.0, 1.0),
                fd("amplitude", "a", 1.0, 0.0, 1.0),
                depth(0.0, 8.0),
                ripples(),
                b("softEdges", "se", true),
                fd("edgeThresh", "et", 1.0, 0.0, 2.0),
                fd("centerThresh", "ct", 0.0, -1.0, 1.0),
                fd("transpRange", "tr", 0.5, 0.0, 1.0),
                fd("ratio", "ra", 0.707, 0.0, 1.0),
            ]),
        ),
        (
            "crater",
            "texture/3d",
            "Plateaus and craters in three colours through space.",
            tex3d(vec![
                fd("shaker", "sh", 1.5, 0.0, 20.0),
                c("channel1", "c1", [1.0, 0.0, 0.0], IN),
                c("channel2", "c2", [0.0, 1.0, 0.0], IN),
                c("channel3", "c3", [0.0, 0.0, 1.0], IN),
                fd("melt", "m", 0.0, 0.0, 1.0),
                fd("balance", "ba", 0.0, 0.0, 1.0),
                fd("frequency", "fr", 2.0, 0.0, 10.0),
                fd("normDepth", "nd", 5.0, 0.0, 10.0),
                fd("normMelt", "nm", 0.0, 0.0, 1.0),
                fd("normBalance", "nb", 1.0, 0.0, 1.0),
                fd("normFrequency", "nf", 1.0, 0.0, 10.0),
                v("normalCamera", "n", flags::INPUT | flags::HIDDEN),
                v("outNormal", "o", OUT),
            ]),
        ),
        (
            "granite",
            "texture/3d",
            "Stone of three minerals held in a filler.",
            tex3d(vec![
                c("color1", "c1", [0.0; 3], IN),
                c("color2", "c2", [0.549, 0.784, 0.392], IN),
                c("color3", "c3", [0.627, 0.824, 0.823], IN),
                c("fillerColor", "fc", [0.588, 0.294, 0.196], IN),
                fd("cellSize", "cs", 0.15, 0.0, 1.0),
                fd("density", "dy", 1.0, 0.0, 1.0),
                fd("mixRatio", "mr", 0.5, 0.0, 1.0),
                fd("spottyness", "s", 0.3, 0.0, 1.0),
                fd("randomness", "ra", 1.0, 0.0, 1.0),
                fd("threshold", "th", 0.5, 0.0, 1.0),
                b("creases", "c", true),
            ]),
        ),
        (
            "leather",
            "texture/3d",
            "Hide: cells of one colour in creases of another.",
            tex3d(vec![
                c("cellColor", "ce", [0.373, 0.157, 0.059], IN),
                c("creaseColor", "cr", [0.235, 0.118, 0.0], IN),
                fd("cellSize", "cs", 0.5, 0.0, 1.0),
                fd("density", "d", 1.0, 0.0, 1.0),
                fd("spottyness", "s", 0.1, 0.0, 1.0),
                fd("randomness", "r", 0.5, 0.0, 1.0),
                fd("threshold", "th", 0.83, 0.0, 1.0),
                b("creases", "c", true),
            ]),
        ),
        (
            "marble",
            "texture/3d",
            "Veins in a filler, their colour spreading into it.",
            tex3d(vec![
                c("fillerColor", "fc", [1.0; 3], IN),
                c("veinColor", "vc", [0.298, 0.0, 0.0], IN),
                fd("veinWidth", "vw", 0.1, 0.0, 1.0),
                fd("diffusion", "di", 0.5, 0.0, 1.0),
                fd("contrast", "c", 0.5, 0.0, 1.0),
                fd("amplitude", "a", 1.5, 0.0, 10.0),
                fd("ratio", "ra", 0.707, 0.0, 1.0),
                ripples(),
                depth(0.0, 20.0),
            ]),
        ),
        ("rock", "texture/3d", "Two colours of grain mixed, as rock is.", tex3d(vec![c("color1", "c1", [0.235, 0.0, 0.0], IN), c("color2", "c2", [1.0; 3], IN), fd("grainSize", "gs", 0.01, 0.0, 0.1), fd("diffusion", "di", 1.0, 0.0, 1.0), fd("mixRatio", "mr", 0.5, 0.0, 1.0)])),
        (
            "snow",
            "texture/3d",
            "Snow lying on what faces up and is not too steep.",
            tex3d(vec![c("snowColor", "sn", [1.0; 3], IN), c("surfaceColor", "su", [0.5, 0.0, 0.0], IN), fd("threshold", "th", 0.5, 0.0, 1.0), fd("depthDecay", "dd", 5.0, 0.0, 10.0), fd("thickness", "tn", 1.0, 0.0, 1.0), v("normalCamera", "n", flags::INPUT | flags::HIDDEN)]),
        ),
        (
            "solidFractal",
            "texture/3d",
            "Fractal noise through space: what many other patterns are made from.",
            tex3d(vec![
                fd("threshold", "th", 0.0, 0.0, 1.0),
                fd("amplitude", "a", 1.0, 0.0, 1.0),
                fd("ratio", "ra", 0.707, 0.0, 1.0),
                fd("frequencyRatio", "fr", 2.0, 1.0, 10.0),
                ripples(),
                depth(0.0, 8.0),
                fd("bias", "bs", 0.0, -1.0, 1.0),
                b("inflection", "in", false),
                b("animated", "an", false),
                f("time", "ti", 0.0),
                fd("timeRatio", "tr", 2.0, 1.0, 10.0),
            ]),
        ),
        (
            "stucco",
            "texture/3d",
            "Two colours blended at random: stains, clouds, and a stucco finish as a bump.",
            tex3d(vec![fd("shaker", "sh", 20.0, 0.0, 100.0), c("channel1", "c1", [1.0, 0.0, 0.0], IN), c("channel2", "c2", [0.0, 0.0, 1.0], IN), fd("normalDepth", "nd", 1.0, 0.0, 10.0), fd("normalMelt", "nm", 0.1, 0.0, 1.0), v("normalCamera", "n", flags::INPUT | flags::HIDDEN), v("outNormal", "on", OUT)]),
        ),
        (
            "volumeNoise",
            "texture/3d",
            "The 2D noise's patterns through space: Perlin, billow, waves, wisps.",
            tex3d(vec![
                fd("threshold", "th", 0.0, 0.0, 1.0),
                fd("amplitude", "a", 1.0, 0.0, 1.0),
                fd("ratio", "ra", 0.707, 0.0, 1.0),
                fd("frequencyRatio", "fr", 2.0, 1.0, 10.0),
                i("depthMax", "dm", 3),
                b("inflection", "in", false),
                f("time", "ti", 0.0),
                fd("frequency", "fq", 8.0, 0.0, 100.0),
                triple("scale", "sc", [1.0; 3], IN),
                triple("origin", "orr", [0.0; 3], IN),
                fd("implode", "imp", 0.0, -1.0, 1.0),
                triple("implodeCenter", "imc", [0.0; 3], IN),
                e("noiseType", "nty", 1, &["Perlin Noise", "Billow", "VolumeWave", "Wispy", "SpaceTime"]),
                fd("density", "d", 1.0, 0.0, 1.0),
                fd("spottyness", "sp", 0.1, 0.0, 1.0),
                fd("sizeRand", "sr", 0.0, 0.0, 1.0),
                fd("randomness", "rn", 1.0, 0.0, 1.0),
                e("falloff", "fof", 2, &["Linear", "Smooth", "Fast", "Bubble"]),
                i("numWaves", "nw", 5),
            ]),
        ),
        (
            "wood",
            "texture/3d",
            "Rings of vein and filler through a block, so what is cut from it shows the same wood.",
            tex3d(vec![
                c("fillerColor", "fc", [0.824, 0.627, 0.471], IN),
                c("veinColor", "vc", [0.157, 0.078, 0.039], IN),
                fd("veinSpread", "v", 0.25, 0.0, 3.0),
                fd("layerSize", "ls", 0.05, 0.0, 0.5),
                fd("randomness", "rd", 0.5, 0.0, 1.0),
                fd("age", "a", 20.0, 0.0, 100.0),
                c("grainColor", "gc", [0.118, 0.039, 0.0], IN),
                fd("grainContrast", "gx", 0.5, 0.0, 1.0),
                fd("grainSpacing", "gs", 0.01, 0.002, 0.1),
                pair("center", "c", [("centerU", "cu"), ("centerV", "cv")], [0.5, -0.5], IN),
                fd("amplitudeX", "ax", 0.0, 0.0, 1.0),
                fd("amplitudeY", "ay", 0.0, 0.0, 1.0),
                fd("ratio", "ra", 0.35, 0.0, 1.0),
                ripples(),
                depth(0.0, 8.0),
            ]),
        ),
        (
            "samplerInfo",
            "utility/general",
            "What is known of the point being shaded: where it is, which way it faces, its UV.",
            vec![
                base("facingRatio", "fr", PlugType::Float, PlugValue::Float(0.0), OUT),
                base("flippedNormal", "fn", PlugType::Bool, PlugValue::Bool(false), OUT),
                v("normalCamera", "n", OUT),
                v("pointCamera", "p", OUT),
                v("pointObj", "po", OUT),
                v("pointWorld", "pw", OUT),
                v("rayDirection", "r", OUT),
                v("tangentUCamera", "tu", OUT),
                v("tangentVCamera", "tv", OUT),
                pair("uvCoord", "uv", [("uCoord", "u"), ("vCoord", "v")], [0.0, 0.0], OUT),
                pair("pixelCenter", "pc", [("pixelCenterX", "pcx"), ("pixelCenterY", "pcy")], [0.0, 0.0], OUT),
                message(),
            ],
        ),
        ("gammaCorrect", "utility/color", "A gamma for each channel: new is old to the power of 1 over gamma.", vec![c("value", "v", [0.0; 3], IN), triple("gamma", "g", [1.0; 3], IN), base("renderPassMode", "arp", PlugType::Int, PlugValue::Int(1), STORE | flags::HIDDEN), v("outValue", "o", OUT), message()]),
        ("contrast", "utility/color", "More or less contrast, each channel about its own middle point.", vec![c("value", "v", [0.0; 3], IN), triple("contrast", "c", [2.0; 3], IN), triple("bias", "b", [0.5; 3], IN), base("renderPassMode", "arp", PlugType::Int, PlugValue::Int(1), STORE | flags::HIDDEN), v("outValue", "o", OUT), message()]),
        (
            "bump3d",
            "utility/general",
            "Turns a 3D texture's value into a bump.",
            vec![f("bumpValue", "bv", 0.0), fd("bumpDepth", "bd", 1.0, -5.0, 5.0), base("bumpFilter", "bf", PlugType::Float, PlugValue::Float(1.0), STORE), base("bumpFilterOffset", "bfo", PlugType::Float, PlugValue::Float(0.0), STORE), triple("normalCamera", "n", [0.0, 0.0, 1.0], flags::INPUT), triple("outNormal", "o", [0.0, 0.0, 1.0], OUT), message()],
        ),
    ]
}

/// classification for shadingNode and the Hypershade.
pub fn classification(ty: &str) -> Option<&'static str> {
    if ty == "place3dTexture" {
        return Some("utility/general");
    }
    table().into_iter().filter(|t| !TAKEN.contains(&t.0)).find(|t| t.0 == ty).map(|t| t.1)
}

/// Names Aztek already uses for something else: its cloth simulation node is called `cloth`, which is the reference's
/// name for the woven texture. The texture is written and tested but not registered until the two are told apart.
const TAKEN: &[&str] = &["cloth"];

pub fn is_texture(ty: &str) -> bool {
    classification(ty).is_some_and(|c| c.starts_with("texture"))
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        for (name, _class, doc, attrs) in table() {
            if types::find(name).is_some() || TAKEN.contains(&name) {
                continue;
            }
            types::register_type(NodeType { name: name.into(), doc: doc.into(), own_attrs: attrs, create: Some(Arc::new(|| Box::new(Plain))), ignores_added_attrs: true, ..Default::default() }).expect(name);
        }
        // A transform with nothing of its own: its world inverse matrix is what a 3D texture's placement reads.
        if types::find("place3dTexture").is_none() {
            let xf = types::find("transform").expect("transform");
            types::register_type(NodeType { name: "place3dTexture".into(), base: Some(xf.clone()), dag: true, icon: "box".into(), doc: "Where a 3D texture sits in the scene: move, turn and scale it to place the pattern. Its worldInverseMatrix goes into the texture's placementMatrix.".into(), create: xf.create.clone(), ..Default::default() })
                .expect("place3dTexture");
        }
    });
}
