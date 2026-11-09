//! Lights: the reference's light node types, built on its abstract ones so `ls -type light` finds them all, what the viewport
//! reads of them, and how they draw as icons. Commands are in `light_cmds.rs`.
//!
//! Names, short names and defaults are from the node reference. Attributes for a renderer's sampling are there for scripts
//! and are hidden. Colours are scene linear. Angles are held in degrees, as getAttr gives them.

use std::any::Any;
use std::sync::Arc;

use glam::{DMat4, DVec3};

use crate::graph::Graph;
use crate::node::{types, EvalContext, Node, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::transform::{DagNode, Shape};

const KEY: u32 = flags::INPUT | flags::STORABLE | flags::KEYABLE;
const STORE: u32 = flags::INPUT | flags::STORABLE;
const SAMPLED: u32 = flags::INPUT | flags::HIDDEN;
const OUT: u32 = flags::OUTPUT | flags::HIDDEN;

pub const TYPES: &[&str] = &["ambientLight", "directionalLight", "pointLight", "spotLight", "areaLight", "volumeLight", "aiSkyDomeLight", "aiAreaLight", "aiMeshLight", "aiPhotometricLight"];
pub const DEFAULT_SET: &str = "defaultLightSet";
/// Holds a volume light's colorRange and penumbra ramps as the file wrote them: Aztek has no multi compounds to hold them.
pub const RAMP_LINES: &str = "aztekRampLines";

pub fn is_light_type(ty: &str) -> bool {
    TYPES.contains(&ty)
}

pub fn classification(ty: &str) -> Option<&'static str> {
    Some(match ty {
        "ambientLight" => "light:drawdb/light/ambientLight",
        "directionalLight" => "light:drawdb/light/directionalLight",
        "pointLight" => "light:drawdb/light/pointLight",
        "spotLight" => "light:drawdb/light/spotLight",
        "areaLight" => "light:drawdb/light/areaLight",
        "volumeLight" => "light:drawdb/light/volumeLight",
        "aiSkyDomeLight" | "aiAreaLight" | "aiMeshLight" | "aiPhotometricLight" => "light:rendernode/arnold/light",
        _ => return None,
    })
}

fn attr(name: &str, short: &str, ty: PlugType, default: PlugValue, fl: u32) -> AttrDef {
    AttrDef { name: name.into(), short_name: short.into(), ty, default_value: default, flags: fl, ..Default::default() }
}
fn num(name: &str, short: &str, d: f64, fl: u32) -> AttrDef {
    attr(name, short, PlugType::Float, PlugValue::Float(d), fl)
}
fn int(name: &str, short: &str, d: i64, fl: u32) -> AttrDef {
    attr(name, short, PlugType::Int, PlugValue::Int(d), fl)
}
fn on(name: &str, short: &str, d: bool, fl: u32) -> AttrDef {
    attr(name, short, PlugType::Bool, PlugValue::Bool(d), fl)
}
fn text(name: &str, short: &str, fl: u32) -> AttrDef {
    attr(name, short, PlugType::String, PlugValue::String(String::new()), fl)
}
fn choice(name: &str, short: &str, names: &[&str], d: i64) -> AttrDef {
    AttrDef { enum_names: names.iter().map(|s| s.to_string()).collect(), ..attr(name, short, PlugType::Enum, PlugValue::Int(d), STORE) }
}
/// Three children whose long and short names the reference gives one by one: they don't all follow the parent's.
fn triple(name: &str, short: &str, kids: [(&str, &str); 3], d: f64, fl: u32) -> AttrDef {
    let mut a = attr(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::splat(d)), fl);
    for (i, (l, s)) in kids.iter().enumerate() {
        a.child_names[i] = l.to_string();
        a.child_short[i] = s.to_string();
    }
    a
}
/// Two floats, held in a Vec3 whose third child has no name.
fn double(name: &str, short: &str, kids: [(&str, &str); 2], fl: u32) -> AttrDef {
    let mut a = attr(name, short, PlugType::Vec3, PlugValue::Vec3(DVec3::ZERO), fl);
    for (i, (l, s)) in kids.iter().enumerate() {
        a.child_names[i] = l.to_string();
        a.child_short[i] = s.to_string();
    }
    a
}
fn colour(name: &str, short: &str, kids: [(&str, &str); 3], d: f64, fl: u32) -> AttrDef {
    let mut a = triple(name, short, kids, d, fl);
    a.flags |= flags::COLOR;
    a
}
fn ranged(mut a: AttrDef, lo: f64, hi: f64) -> AttrDef {
    a.soft_min = Some(lo);
    a.soft_max = Some(hi);
    a
}
fn object_type() -> AttrDef {
    int("objectType", "ot", 0, STORE | flags::HIDDEN)
}
fn point_world() -> AttrDef {
    triple("pointWorld", "pw", [("pointWorldX", "tx"), ("pointWorldY", "ty"), ("pointWorldZ", "tz")], 1.0, STORE | flags::HIDDEN)
}
fn normal_camera() -> AttrDef {
    triple("normalCamera", "n", [("normalCameraX", "nx"), ("normalCameraY", "ny"), ("normalCameraZ", "nz")], 1.0, SAMPLED)
}
fn light_glow() -> AttrDef {
    int("lightGlow", "lg", 0, flags::INPUT | flags::HIDDEN)
}

fn light_attrs() -> Vec<AttrDef> {
    vec![
        num("centerOfIllumination", "col", 5.0, STORE),
        colour("color", "cl", [("colorR", "cr"), ("colorG", "cg"), ("colorB", "cb")], 1.0, KEY),
        ranged(num("intensity", "in", 1.0, KEY), 0.0, 10.0),
        colour("shadowColor", "sc", [("shadColorR", "scr"), ("shadColorG", "scg"), ("shadColorB", "scb")], 0.0, KEY),
        on("useRayTraceShadows", "urs", true, KEY),
        num("locatorScale", "lls", 1.0, STORE),
        int("shadowRays", "shr", 1, STORE),
        int("rayDepthLimit", "rdl", 3, STORE),
        int("lightData", "ltd", 0, OUT),
        triple("lightDirection", "ld", [("lightDirectionX", "ldx"), ("lightDirectionY", "ldy"), ("lightDirectionZ", "ldz")], 0.0, OUT),
        colour("lightIntensity", "li", [("lightIntensityR", "lir"), ("lightIntensityG", "lig"), ("lightIntensityB", "lib")], 0.0, OUT),
        on("lightAmbient", "la", false, OUT),
        on("lightDiffuse", "ldf", false, OUT),
        on("lightSpecular", "ls", false, OUT),
        num("lightShadowFraction", "lsf", 0.0, OUT),
        num("preShadowIntensity", "psi", 0.0, OUT),
        int("lightBlindData", "lbl", 0, OUT),
        colour("opticalFXvisibility", "ov", [("opticalFXvisibilityR", "ovr"), ("opticalFXvisibilityG", "ovg"), ("opticalFXvisibilityB", "ovb")], 1.0, OUT),
        attr("matrixEyeToWorld", "etw", PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY), SAMPLED),
        attr("matrixWorldToEye", "wte", PlugType::Matrix, PlugValue::Matrix(DMat4::IDENTITY), SAMPLED),
        int("infoBits", "ib", 0, SAMPLED),
        int("objectId", "oi", 0, SAMPLED),
        int("primitiveId", "pi", 0, SAMPLED),
        int("rayDepth", "rd", 0, SAMPLED),
        int("raySampler", "rts", 0, SAMPLED),
        int("renderState", "rdst", 0, SAMPLED),
        triple("pointCamera", "p", [("pointCameraX", "px"), ("pointCameraY", "py"), ("pointCameraZ", "pz")], 1.0, SAMPLED | flags::STORABLE),
        double("uvCoord", "uv", [("uCoord", "uu"), ("vCoord", "vv")], SAMPLED),
        double("uvFilterSize", "fq", [("uvFilterSizeX", "fsx"), ("uvFilterSizeY", "fsy")], SAMPLED),
    ]
}

fn non_ambient_attrs() -> Vec<AttrDef> {
    vec![choice("decayRate", "de", &["No Decay", "Linear", "Quadratic", "Cubic"], 0), on("emitDiffuse", "edi", true, STORE), on("emitSpecular", "esp", true, STORE)]
}

fn non_extended_attrs() -> Vec<AttrDef> {
    vec![
        on("castSoftShadows", "cw", false, STORE),
        on("dmapFrameExt", "uf", false, STORE),
        on("dmapLightName", "ul", true, STORE),
        on("dmapSceneName", "um", false, STORE),
        num("dmapBias", "db", 0.001, STORE),
        num("dmapFarClipPlane", "fcp", 10000.0, STORE),
        int("dmapFilterSize", "fs", 1, STORE),
        num("dmapFocus", "df", 90.0, STORE),
        text("dmapName", "smn", STORE),
        num("dmapNearClipPlane", "nc", 0.001, STORE),
        text("dmapUseMacro", "dc", STORE),
        int("dmapResolution", "dr", 512, STORE),
        num("dmapWidthFocus", "dw", 100.0, STORE),
        int("fogShadowIntensity", "fsi", 1, STORE),
        text("lastWrittenDmapAnimExtName", "lw", STORE),
        int("volumeShadowSamples", "nv", 20, STORE),
        on("receiveShadows", "gs", true, STORE),
        on("reuseDmap", "du", false, STORE),
        num("lightRadius", "lr", 0.0, STORE),
        on("useDmapAutoClipping", "uc", true, STORE),
        on("useDmapAutoFocus", "af", true, STORE),
        on("useDepthMapShadows", "dms", false, STORE),
        on("useMidDistDmap", "md", true, STORE),
        on("useOnlySingleDmap", "us", true, STORE),
        on("useXMinusDmap", "xn", true, STORE),
        on("useXPlusDmap", "xp", true, STORE),
        on("useYMinusDmap", "yn", true, STORE),
        on("useYPlusDmap", "yp", true, STORE),
        on("useZMinusDmap", "zn", true, STORE),
        on("useZPlusDmap", "zp", true, STORE),
        on("writeDmap", "ws", false, STORE),
    ]
}

fn ambient_attrs() -> Vec<AttrDef> {
    vec![ranged(num("ambientShade", "as", 0.45, STORE), 0.0, 1.0), on("castSoftShadows", "cw", false, STORE), normal_camera(), object_type(), on("receiveShadows", "gs", true, STORE), num("shadowRadius", "sr", 0.0, STORE)]
}

fn directional_attrs() -> Vec<AttrDef> {
    vec![object_type(), point_world(), num("lightAngle", "lang", 0.0, STORE), on("useLightPosition", "up", false, STORE)]
}

fn point_attrs() -> Vec<AttrDef> {
    vec![
        triple("farPointWorld", "fw", [("farPointWorldX", "fwx"), ("farPointWorldY", "fwy"), ("farPointWorldZ", "fwz")], 1.0, STORE | flags::HIDDEN),
        int("fogGeometry", "fg", 0, flags::INPUT | flags::HIDDEN),
        num("fogIntensity", "fin", 1.0, STORE),
        num("fogRadius", "fr", 1.0, STORE),
        choice("fogType", "ft", &["Normal", "Linear", "Exponential"], 0),
        light_glow(),
        object_type(),
        point_world(),
    ]
}

fn spot_attrs() -> Vec<AttrDef> {
    vec![
        on("barnDoors", "bd", false, STORE),
        num("bottomBarnDoor", "bbd", 20.0, STORE),
        ranged(num("coneAngle", "ca", 40.0, KEY), 0.006, 179.994),
        ranged(num("dropoff", "dro", 0.0, KEY), 0.0, 255.0),
        num("endDistance1", "ed1", 2.0, STORE),
        num("endDistance2", "ed2", 6.0, STORE),
        num("endDistance3", "ed3", 10.0, STORE),
        triple("farPointWorld", "fw", [("farPointWorldX", "fx"), ("farPointWorldY", "fy"), ("farPointWorldZ", "fz")], 1.0, STORE | flags::HIDDEN),
        int("fogGeometry", "fg", 0, flags::INPUT | flags::HIDDEN),
        num("fogIntensity", "fin", 1.0, STORE),
        num("fogSpread", "fsp", 1.0, STORE),
        num("leftBarnDoor", "lbd", 20.0, STORE),
        light_glow(),
        int("psIllumSamples", "pis", 1, STORE),
        object_type(),
        ranged(num("penumbraAngle", "pa", 0.0, KEY), -10.0, 10.0),
        point_world(),
        triple("rayDirection", "rad", [("rayDirectionX", "rdx"), ("rayDirectionY", "rdy"), ("rayDirectionZ", "rdz")], 0.0, SAMPLED),
        num("rightBarnDoor", "rbd", 20.0, STORE),
        num("startDistance1", "sd1", 1.0, STORE),
        num("startDistance2", "sd2", 3.0, STORE),
        num("startDistance3", "sd3", 8.0, STORE),
        num("topBarnDoor", "tbd", 20.0, STORE),
        on("useDecayRegions", "udr", false, STORE),
    ]
}

fn area_attrs() -> Vec<AttrDef> {
    vec![on("legacyIntensity", "lgi", false, STORE), light_glow(), normal_camera(), on("normalize", "nrm", true, STORE), object_type(), point_world()]
}

fn volume_attrs() -> Vec<AttrDef> {
    vec![
        num("arc", "ac", 360.0, STORE),
        num("coneEndRadius", "cer", 0.0, STORE),
        on("emitAmbient", "ema", false, STORE),
        choice("lightShape", "lis", &["Box", "Sphere", "Cylinder", "Cone"], 1),
        num("lightAngle", "lang", 0.0, STORE),
        choice("volumeLightDir", "vld", &["Outward", "Inward", "Down Axis"], 0),
        text(RAMP_LINES, "azrl", flags::INPUT | flags::HIDDEN),
    ]
}

struct LightShape;

impl NodeBehavior for LightShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let t = d.ty();
        let (Some(c), Some(i), Some(li)) = (t.attr_index("color"), t.attr_index("intensity"), t.attr_index("lightIntensity")) else { return };
        let out = d.vec3(c) * d.value(i).scalar().unwrap_or(1.0);
        d.set_output(li, out);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let table: Vec<(&str, &str, &str, Vec<AttrDef>, bool)> = vec![
            ("light", "shape", "What every light has.", light_attrs(), false),
            ("renderLight", "light", "A light a renderer samples.", vec![int("rayInstance", "ryi", 0, SAMPLED)], false),
            ("nonAmbientLightShapeNode", "renderLight", "A light with a direction at each point: diffuse and specular.", non_ambient_attrs(), false),
            ("nonExtendedLightShapeNode", "nonAmbientLightShapeNode", "A light with depth map and soft shadow settings.", non_extended_attrs(), false),
            ("ambientLight", "renderLight", "Light from everywhere, ambientShade of it from the light's position.", ambient_attrs(), true),
            ("directionalLight", "nonExtendedLightShapeNode", "Parallel light along the transform's -Z.", directional_attrs(), true),
            ("pointLight", "nonExtendedLightShapeNode", "Light from a point, in all directions.", point_attrs(), true),
            ("spotLight", "nonExtendedLightShapeNode", "A cone of light along the transform's -Z.", spot_attrs(), true),
            ("areaLight", "nonExtendedLightShapeNode", "Light from one side of a rectangle two units across.", area_attrs(), true),
            ("volumeLight", "pointLight", "Light inside a box, sphere, cylinder or cone.", volume_attrs(), true),
        ];
        let mut table = table;
        table[0].3.extend(crate::harold_nodes::light_render_attrs());
        for (name, doc, attrs) in crate::harold_nodes::ai_lights() {
            table.push((name, "renderLight", doc, attrs, true));
        }
        for (name, parent, doc, own_attrs, concrete) in table {
            types::register_type(NodeType {
                name: name.into(),
                base: types::find(parent),
                dag: true,
                icon: if concrete { name.into() } else { String::new() },
                doc: doc.into(),
                own_attrs,
                create: concrete.then(|| Arc::new(|| Box::new(LightShape) as Box<dyn NodeBehavior>) as crate::node::CreateFn),
                ..Default::default()
            })
            .expect(name);
        }
        // A copy of a light is in the default set when the original was.
        crate::duplicate::add_fixup(Arc::new(|g, copies| {
            let pairs: Vec<(NodeId, NodeId)> = copies.iter().map(|(a, b)| (*a, *b)).filter(|(a, _)| g.node(*a).is_some_and(|n| n.ty().is_a("light"))).collect();
            for (from, to) in pairs {
                if illuminates_by_default(g, from) {
                    let _ = set_illuminates_by_default(g, to, true);
                }
            }
        }));
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// The default set

pub fn default_set(g: &Graph) -> Option<NodeId> {
    g.find(DEFAULT_SET).ok().flatten()
}

/// Whether a light (its transform or shape) is in defaultLightSet: Illuminates by Default.
pub fn illuminates_by_default(g: &Graph, light: NodeId) -> bool {
    let tr = crate::dag::transform_of(g, light);
    default_set(g).is_some_and(|set| crate::object_sets::nodes_of(g, set).contains(&tr))
}

pub fn set_illuminates_by_default(g: &mut Graph, light: NodeId, on: bool) -> aztek_ops::CmdResult<()> {
    let tr = crate::dag::transform_of(g, light);
    let set = match default_set(g) {
        Some(set) => set,
        None => g.without_undo(|g| crate::object_sets::create(g, DEFAULT_SET, &[], false))?,
    };
    let member = [crate::object_sets::Member::object(tr)];
    if on {
        crate::object_sets::add(g, set, &member)
    } else {
        crate::object_sets::remove(g, set, &member)
    }
}

/// The shape of a light named by itself or its transform.
pub fn shape_of(g: &Graph, id: NodeId) -> Option<NodeId> {
    crate::dag::shape_of_type(g, id, "light")
}

pub fn all_shapes(g: &Graph) -> Vec<NodeId> {
    g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().is_a("light"))).collect()
}

// ---------------------------------------------------------------------------------------------------------------------
// What the viewport reads

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Ambient,
    Directional,
    Point,
    Spot,
    Area,
    Volume,
}

impl Kind {
    pub fn of(ty: &str) -> Option<Kind> {
        Some(match ty {
            "ambientLight" => Kind::Ambient,
            "directionalLight" => Kind::Directional,
            "pointLight" => Kind::Point,
            "spotLight" => Kind::Spot,
            "areaLight" => Kind::Area,
            "volumeLight" => Kind::Volume,
            _ => return None,
        })
    }
}

/// One light as it shades, in world space.
#[derive(Clone, Debug)]
pub struct SceneLight {
    pub shape: NodeId,
    pub transform: NodeId,
    pub kind: Kind,
    pub world: DMat4,
    /// Colour times intensity.
    pub color: [f32; 3],
    pub diffuse: bool,
    pub specular: bool,
    /// 0 none, 1 linear, 2 quadratic, 3 cubic.
    pub decay: i32,
    pub cone_deg: f64,
    pub penumbra_deg: f64,
    pub dropoff: f64,
    pub ambient_shade: f64,
    /// An area light's intensity follows its size: normalize off.
    pub by_area: bool,
    /// 0 box, 1 sphere, 2 cylinder, 3 cone.
    pub volume_shape: i32,
    /// 0 outward, 1 inward, 2 down the axis.
    pub volume_dir: i32,
    pub volume_ambient: bool,
}

fn read_f(g: &mut Graph, id: NodeId, name: &str, d: f64) -> f64 {
    let Some(i) = g.node(id).and_then(|n| n.ty().attr_index(name)) else { return d };
    g.value(&PlugRef::new(id, i as i32)).ok().and_then(|v| v.scalar()).unwrap_or(d)
}

fn read_c(g: &mut Graph, id: NodeId, name: &str) -> DVec3 {
    let Some(i) = g.node(id).and_then(|n| n.ty().attr_index(name)) else { return DVec3::ONE };
    match g.value(&PlugRef::new(id, i as i32)) {
        Ok(PlugValue::Vec3(v)) => v,
        _ => DVec3::ONE,
    }
}

/// A light shape as it shades. Follows connections and keys.
pub fn scene_light(g: &mut Graph, shape: NodeId) -> Option<SceneLight> {
    let kind = Kind::of(&g.node(shape)?.ty().name)?;
    let color = read_c(g, shape, "color") * read_f(g, shape, "intensity", 1.0);
    let flag = |g: &mut Graph, n: &str| read_f(g, shape, n, 1.0) != 0.0;
    let world = g.node(shape)?.data().matrix(DagNode::WORLD_MATRIX);
    Some(SceneLight {
        shape,
        transform: crate::dag::transform_of(g, shape),
        kind,
        world,
        color: color.as_vec3().to_array(),
        diffuse: flag(g, "emitDiffuse"),
        specular: kind != Kind::Ambient && flag(g, "emitSpecular"),
        decay: read_f(g, shape, "decayRate", 0.0) as i32,
        cone_deg: read_f(g, shape, "coneAngle", 40_f64.to_radians()).to_degrees(),
        penumbra_deg: read_f(g, shape, "penumbraAngle", 0.0).to_degrees(),
        dropoff: read_f(g, shape, "dropoff", 0.0),
        ambient_shade: read_f(g, shape, "ambientShade", 0.45),
        by_area: kind == Kind::Area && !flag(g, "normalize"),
        volume_shape: read_f(g, shape, "lightShape", 1.0) as i32,
        volume_dir: read_f(g, shape, "volumeLightDir", 0.0) as i32,
        volume_ambient: read_f(g, shape, "emitAmbient", 0.0) != 0.0,
    })
}

// ---------------------------------------------------------------------------------------------------------------------
// The light model

/// Falloff with distance: none at rate 0, else one over distance to the rate (1 linear, 2 quadratic, 3 cubic).
pub fn decay_factor(rate: i32, d: f64) -> f64 {
    if rate <= 0 {
        1.0
    } else {
        1.0 / d.max(0.001).powi(rate)
    }
}

/// A spot light's edge: nothing outside the outer cone, all inside the inner, linear between, then cos^dropoff.
pub fn spot_factor(l: &SceneLight, cos_axis: f64) -> f64 {
    let (outer, inner) = spot_cones(l);
    let edge = ((cos_axis - outer) / (inner - outer).max(1e-5)).clamp(0.0, 1.0);
    edge * cos_axis.max(0.0).powf(l.dropoff)
}

/// Cosines of a spot's outer and inner cone: a positive penumbra widens the edge outside the cone, a negative one inside.
pub fn spot_cones(l: &SceneLight) -> (f64, f64) {
    let half = l.cone_deg * 0.5;
    let (inner, outer) = if l.penumbra_deg >= 0.0 { (half, half + l.penumbra_deg) } else { (half + l.penumbra_deg, half) };
    (outer.clamp(0.0, 180.0).to_radians().cos(), inner.clamp(0.0, 180.0).to_radians().cos())
}

/// How far through a volume light's shape a point in its unit space is: 0 at its middle, 1 at its boundary.
pub fn volume_reach(u: DVec3, shape: i32) -> f64 {
    match shape {
        0 => u.x.abs().max(u.y.abs()).max(u.z.abs()),
        1 => u.length(),
        _ => {
            let radius = if shape == 2 { 1.0 } else { (1.0 - (u.z + 1.0) * 0.5).clamp(0.0, 1.0) };
            (u.truncate().length() / radius.max(0.001)).max(u.z.abs())
        }
    }
}

/// What diffuse takes from one light at a point with normal n (facing the viewer's side): the viewport's lighting, and
/// the renderer's direct lighting on a matte surface with no shadow, are this times the surface's colour and diffuse.
/// The viewport's shader (`aztek_host` render.rs) is the same formula in GLSL; keep the two in step.
pub fn viewport_diffuse(l: &SceneLight, p: DVec3, n: DVec3) -> DVec3 {
    let pos = l.world.w_axis.truncate();
    let axis = (-l.world.z_axis.truncate()).normalize_or_zero();
    let mut c = DVec3::from_array(l.color.map(f64::from));
    let mut diff = DVec3::ZERO;
    let lv;
    match l.kind {
        Kind::Ambient => {
            lv = (pos - p).normalize_or_zero();
            diff += c * (1.0 - l.ambient_shade);
            c *= l.ambient_shade;
        }
        Kind::Directional => lv = -axis,
        Kind::Point => {
            let to = pos - p;
            let d = to.length();
            lv = to / d.max(1e-4);
            c *= decay_factor(l.decay, d);
        }
        Kind::Spot => {
            let to = pos - p;
            let d = to.length();
            lv = to / d.max(1e-4);
            c *= decay_factor(l.decay, d) * spot_factor(l, (-lv).dot(axis));
        }
        Kind::Area => {
            // The rectangle's nearest point to the surface stands for all of it, lit from one side.
            let (u, v) = (l.world.x_axis.truncate(), l.world.y_axis.truncate());
            let r = p - pos;
            let q = pos + u * (r.dot(u) / u.length_squared().max(1e-6)).clamp(-1.0, 1.0) + v * (r.dot(v) / v.length_squared().max(1e-6)).clamp(-1.0, 1.0);
            let to = q - p;
            let d = to.length();
            lv = to / d.max(1e-4);
            c *= decay_factor(l.decay, d) * (-lv).dot(axis).max(0.0);
            if l.by_area {
                c *= u.length() * v.length();
            }
        }
        Kind::Volume => {
            let unit = l.world.inverse().transform_point3(p);
            let fade = (1.0 - volume_reach(unit, l.volume_shape)).clamp(0.0, 1.0);
            c *= fade;
            let to = pos - p;
            let mut dir = if to.length() > 1e-4 { to.normalize() } else { n };
            if l.volume_dir == 2 {
                dir = -axis;
            } else if l.volume_dir == 1 {
                dir = -dir;
            }
            lv = dir;
            if l.volume_ambient {
                diff += c;
                c = DVec3::ZERO;
            }
        }
    }
    // The all-round parts above are taken whatever emitDiffuse says, as in the shader.
    let nl = n.dot(lv);
    if nl > 0.0 && l.diffuse {
        diff += c * nl;
    }
    diff
}

// ---------------------------------------------------------------------------------------------------------------------
// Icons

fn ring(c: DVec3, u: DVec3, v: DVec3, r: f64, n: usize, out: &mut Vec<(DVec3, DVec3)>) {
    let at = |i: usize| {
        let a = i as f64 / n as f64 * std::f64::consts::TAU;
        c + u * (a.cos() * r) + v * (a.sin() * r)
    };
    for i in 0..n {
        out.push((at(i), at(i + 1)));
    }
}

fn arrow(from: DVec3, to: DVec3, side: DVec3, out: &mut Vec<(DVec3, DVec3)>) {
    out.push((from, to));
    let d = to - from;
    let back = to - d * 0.3;
    let s = side.normalize_or_zero() * d.length() * 0.15;
    out.push((to, back + s));
    out.push((to, back - s));
}

/// A light's icon in world space, from its world matrix. Own drawings: rays, arrows and outlines that say what it does.
pub fn icon_lines(n: &Node, w: &DMat4) -> Vec<(DVec3, DVec3)> {
    let t = n.ty();
    let idx = |name: &str, d: f64| t.attr_index(name).and_then(|i| n.value(i).scalar()).unwrap_or(d);
    let c = w.w_axis.truncate();
    let (x, y, z) = (w.x_axis.truncate().normalize_or_zero(), w.y_axis.truncate().normalize_or_zero(), w.z_axis.truncate().normalize_or_zero());
    let k = idx("locatorScale", 1.0);
    let mut s = Vec::new();
    match Kind::of(&t.name) {
        Some(Kind::Directional) => {
            for (a, b) in [(-0.5, 0.0), (0.0, 0.0), (0.5, 0.0)] {
                let from = c + x * (a * k) + y * (b * k);
                arrow(from, from - z * (1.6 * k), x, &mut s);
            }
            s.push((c - x * (0.5 * k), c + x * (0.5 * k)));
        }
        Some(Kind::Point) => {
            ring(c, x, y, 0.25 * k, 16, &mut s);
            ring(c, y, z, 0.25 * k, 16, &mut s);
            ring(c, z, x, 0.25 * k, 16, &mut s);
            for d in [x, -x, y, -y, z, -z, (x + y).normalize_or_zero(), (x - y).normalize_or_zero(), -(x + y).normalize_or_zero(), (y - x).normalize_or_zero()] {
                s.push((c + d * (0.35 * k), c + d * (0.7 * k)));
            }
        }
        Some(Kind::Ambient) => {
            ring(c, x, y, 0.45 * k, 20, &mut s);
            ring(c, x, z, 0.45 * k, 20, &mut s);
            // The letter's two legs and bar, standing in the light's middle.
            let foot = 0.22 * k;
            s.push((c - y * foot - x * foot, c + y * foot));
            s.push((c - y * foot + x * foot, c + y * foot));
            s.push((c - x * (foot * 0.5), c + x * (foot * 0.5)));
        }
        Some(Kind::Spot) => {
            let half = idx("coneAngle", 40_f64.to_radians()) * 0.5;
            let len = 1.0 * k;
            let r = half.tan() * len;
            let end = c - z * len;
            ring(end, x, y, r, 24, &mut s);
            for (u, v) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                s.push((c, end + x * (u * r) + y * (v * r)));
            }
            let pen = idx("penumbraAngle", 0.0);
            if pen != 0.0 {
                let r2 = (half + pen).clamp(0.0, 1.55).tan() * len;
                ring(end, x, y, r2, 24, &mut s);
            }
            arrow(c, c - z * (len * 1.6), x, &mut s);
            ring(c, x, y, 0.12 * k, 12, &mut s);
        }
        Some(Kind::Area) => {
            let corner = |a: f64, b: f64| w.transform_point3(DVec3::new(a, b, 0.0));
            let q = [corner(-1.0, -1.0), corner(1.0, -1.0), corner(1.0, 1.0), corner(-1.0, 1.0)];
            for i in 0..4 {
                s.push((q[i], q[(i + 1) % 4]));
            }
            s.push((q[0], q[2]));
            s.push((q[1], q[3]));
            arrow(c, c - z * k, x, &mut s);
        }
        Some(Kind::Volume) => {
            let p = |a: f64, b: f64, d: f64| w.transform_point3(DVec3::new(a, b, d));
            let circle = |d: f64, r: f64, s: &mut Vec<(DVec3, DVec3)>| {
                for i in 0..32 {
                    let (a0, a1) = (i as f64 / 32.0 * std::f64::consts::TAU, (i + 1) as f64 / 32.0 * std::f64::consts::TAU);
                    s.push((p(a0.cos() * r, a0.sin() * r, d), p(a1.cos() * r, a1.sin() * r, d)));
                }
            };
            match idx("lightShape", 1.0) as i32 {
                0 => {
                    for i in 0..8usize {
                        for bit in [1usize, 2, 4] {
                            if i & bit == 0 {
                                let at = |j: usize| p(if j & 1 == 0 { -1.0 } else { 1.0 }, if j & 2 == 0 { -1.0 } else { 1.0 }, if j & 4 == 0 { -1.0 } else { 1.0 });
                                s.push((at(i), at(i | bit)));
                            }
                        }
                    }
                }
                2 | 3 => {
                    let cone = idx("lightShape", 1.0) as i32 == 3;
                    let end = if cone { idx("coneEndRadius", 0.0) } else { 1.0 };
                    circle(-1.0, 1.0, &mut s);
                    if end > 0.0 {
                        circle(1.0, end, &mut s);
                    }
                    for (u, v) in [(1.0, 0.0), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                        s.push((p(u, v, -1.0), p(u * end, v * end, 1.0)));
                    }
                }
                _ => {
                    circle(0.0, 1.0, &mut s);
                    for i in 0..32 {
                        let (a0, a1) = (i as f64 / 32.0 * std::f64::consts::TAU, (i + 1) as f64 / 32.0 * std::f64::consts::TAU);
                        s.push((p(a0.cos(), 0.0, a0.sin()), p(a1.cos(), 0.0, a1.sin())));
                        s.push((p(0.0, a0.cos(), a0.sin()), p(0.0, a1.cos(), a1.sin())));
                    }
                }
            }
        }
        None => {}
    }
    s
}
