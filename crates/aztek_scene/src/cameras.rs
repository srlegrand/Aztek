//! Cameras: the camera shape with the reference's attributes and the camera command. The four default cameras (persp, top, front,
//! side) and the default sets belong to the application's scene, made by `make_defaults`.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, ArgType, CmdResult, CommandDef, Commands, FlagDef};
use glam::{DMat3, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, float_attr_short};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, NodeId, PlugValue, NO_NODE};
use crate::rig::plug;
use crate::session::Session;
use crate::transform::{DagNode, Shape, Transform};

pub const TYPE: &str = "camera";
pub const FOCAL_LENGTH: usize = Shape::COUNT;
pub const H_APERTURE: usize = Shape::COUNT + 1;
pub const V_APERTURE: usize = Shape::COUNT + 2;
pub const NEAR_CLIP: usize = Shape::COUNT + 3;
pub const FAR_CLIP: usize = Shape::COUNT + 4;
pub const ORTHOGRAPHIC: usize = Shape::COUNT + 5;
pub const ORTHO_WIDTH: usize = Shape::COUNT + 6;
pub const COI: usize = Shape::COUNT + 7;
pub const LENS_SQUEEZE: usize = Shape::COUNT + 8;
pub const RENDERABLE: usize = Shape::COUNT + 9;
pub const IMAGE_NAME: usize = Shape::COUNT + 10;
pub const DEPTH_NAME: usize = Shape::COUNT + 11;
pub const MASK_NAME: usize = Shape::COUNT + 12;
pub const HOME_COMMAND: usize = Shape::COUNT + 13;
/// Past the ones named here, up to this flag, are plain stored values.
pub const STARTUP: usize = Shape::COUNT + 55;

fn short_bool(name: &str, short: &str, value: bool) -> crate::plug::AttrDef {
    crate::plug::AttrDef { short_name: short.into(), flags: flags::INPUT | flags::STORABLE, ..bool_attr(name, value, "") }
}

fn short_text(name: &str, short: &str) -> crate::plug::AttrDef {
    crate::plug::AttrDef { name: name.into(), short_name: short.into(), ty: crate::plug::PlugType::String, default_value: PlugValue::String(String::new()), flags: flags::INPUT | flags::STORABLE, ..Default::default() }
}

/// Whether a camera is a startup camera, which can't be deleted until `camera -e -startupCamera off` says so.
pub fn is_startup(g: &Graph, id: NodeId) -> bool {
    shape_of(g, id).and_then(|sh| g.node(sh)).is_some_and(|n| n.bool(STARTUP))
}

/// The attributes beyond the view's own, for files: (attribute index, short name).
pub fn plain_attrs(g: &Graph, shape: NodeId) -> Vec<(usize, String)> {
    let Some(n) = g.node(shape) else { return Vec::new() };
    (RENDERABLE..STARTUP).map(|i| (i, n.ty().attrs[i].short_name.clone())).collect()
}

struct CameraShape;

impl NodeBehavior for CameraShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub fn register_types() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        types::register_builtins();
        let shape = types::find("shape").expect("shape");
        let stored = |d: crate::plug::AttrDef| crate::plug::AttrDef { flags: flags::INPUT | flags::STORABLE, ..d };
        types::register_type(NodeType {
            name: TYPE.into(),
            base: Some(shape),
            dag: true,
            icon: "camera".into(),
            doc: "A camera: what a view looks through.".into(),
            own_attrs: vec![
                float_attr_short("focalLength", "fl", 35.0, "Focal length, mm.", Some(2.5), Some(100000.0)),
                stored(float_attr_short("horizontalFilmAperture", "hfa", 1.41732, "Film width, inches.", Some(0.0001), None)),
                stored(float_attr_short("verticalFilmAperture", "vfa", 0.94488, "Film height, inches.", Some(0.0001), None)),
                stored(float_attr_short("nearClipPlane", "ncp", 0.1, "Near clip distance.", Some(0.0), None)),
                stored(float_attr_short("farClipPlane", "fcp", 10000.0, "Far clip distance.", Some(0.0), None)),
                stored(crate::plug::AttrDef { short_name: "o".into(), ..bool_attr("orthographic", false, "No perspective.") }),
                stored(float_attr_short("orthographicWidth", "ow", 30.0, "View width when orthographic.", Some(1e-6), None)),
                stored(float_attr_short("centerOfInterest", "coi", 5.0, "Distance to what the camera tumbles about.", Some(1e-6), None)),
                stored(float_attr_short("lensSqueezeRatio", "lsr", 1.0, "Lens squeeze.", Some(0.0), None)),
                // Kept for scripts and files; the views read the ones above.
                short_bool("renderable", "rnd", true),
                short_text("imageName", "imn"),
                short_text("depthName", "den"),
                short_text("maskName", "man"),
                short_text("homeCommand", "hc"),
                short_bool("image", "img", true),
                short_bool("mask", "ma", true),
                short_bool("depth", "dep", false),
                short_bool("displayFilmGate", "dfg", false),
                short_bool("displayResolution", "dr", false),
                short_bool("displayGateMask", "dgm", true),
                short_bool("displaySafeAction", "dsa", false),
                short_bool("displaySafeTitle", "dst", false),
                short_bool("displayFieldChart", "dfc", false),
                short_bool("displayFilmOrigin", "dfo", false),
                short_bool("displayFilmPivot", "dfp", false),
                short_bool("displayCameraNearClip", "cnc", false),
                short_bool("displayCameraFarClip", "cfp", false),
                short_bool("displayCameraFrustum", "dcf", false),
                short_bool("depthOfField", "dof", false),
                short_bool("motionBlur", "mb", false),
                short_bool("journalCommand", "jc", false),
                short_bool("panZoomEnabled", "pze", false),
                short_bool("renderPanZoom", "rpz", false),
                short_bool("shakeEnabled", "se", false),
                short_bool("bestFitClippingPlanes", "bfc", true),
                short_bool("clippingPlanes", "cp", true),
                stored(float_attr_short("filmFit", "ff", 1.0, "Film fit.", None, None)),
                stored(float_attr_short("filmFitOffset", "ffo", 0.0, "Film fit offset.", None, None)),
                stored(float_attr_short("horizontalFilmOffset", "hfo", 0.0, "Film offset across.", None, None)),
                stored(float_attr_short("verticalFilmOffset", "vfo", 0.0, "Film offset up.", None, None)),
                stored(float_attr_short("fStop", "fs", 5.6, "Lens f-stop.", None, None)),
                stored(float_attr_short("focusDistance", "fd", 5.0, "Focus distance.", None, None)),
                stored(float_attr_short("focusRegionScale", "frs", 1.0, "Focus region scale.", None, None)),
                stored(float_attr_short("shutterAngle", "sa", 144.0, "Shutter angle, degrees.", None, None)),
                stored(float_attr_short("cameraScale", "cs", 1.0, "Camera scale.", None, None)),
                stored(float_attr_short("locatorScale", "lls", 1.0, "Icon scale.", None, None)),
                stored(float_attr_short("overscan", "ovr", 1.0, "How much past the gate a view shows.", None, None)),
                stored(float_attr_short("preScale", "psc", 1.0, "Pre scale.", None, None)),
                stored(float_attr_short("postScale", "ptsc", 1.0, "Post scale.", None, None)),
                stored(float_attr_short("horizontalPan", "hpn", 0.0, "Pan across.", None, None)),
                stored(float_attr_short("verticalPan", "vpn", 0.0, "Pan up.", None, None)),
                stored(float_attr_short("zoom", "zom", 1.0, "Pan and zoom's zoom.", None, None)),
                stored(float_attr_short("horizontalShake", "hs", 0.0, "Shake across.", None, None)),
                stored(float_attr_short("verticalShake", "vs", 0.0, "Shake up.", None, None)),
                stored(float_attr_short("threshold", "tthd", 0.9, "Depth threshold.", None, None)),
                // Not an attribute of the reference's: its startup flag is internal. Hidden, cleared by camera -e -sc off.
                crate::plug::AttrDef { flags: flags::INPUT | flags::HIDDEN, ..short_bool("aztekStartupCamera", "azsc", false) },
            ],
            create: Some(Arc::new(|| Box::new(CameraShape))),
            ..Default::default()
        })
        .expect(TYPE);
    });
}

/// The shape under a transform, or the shape itself.
pub fn shape_of(g: &Graph, id: NodeId) -> Option<NodeId> {
    crate::dag::shape_of_type(g, id, TYPE)
}

pub fn create(g: &mut Graph, name: &str, parent: NodeId) -> CmdResult<(NodeId, NodeId)> {
    let (tr, sh) = crate::dag::create_with_shape(g, TYPE, name, parent)?;
    Ok((tr, sh))
}

/// A camera's rotate for a view direction: Euler XYZ with no roll, looking down -Z.
pub fn rotate_for(yaw: f64, pitch: f64) -> DVec3 {
    DVec3::new(-pitch, yaw, 0.0)
}

/// Where a default camera sits for a new scene, and `viewSet -home` puts it: (translate, rotate in radians, centre of
/// interest). The values the reference's own scene files hold, turned with the world when Z is up. Other names get the
/// perspective camera's.
pub fn home(name: &str) -> (DVec3, DVec3, f64) {
    home_for(name, crate::world::up_z())
}

/// `home` for a given up axis: a scene file's defaults are the ones of the axis it was saved in.
pub fn home_for(name: &str, z_up: bool) -> (DVec3, DVec3, f64) {
    let short = name.rsplit('|').next().unwrap_or(name);
    let (t, r, coi) = match short {
        "top" => (DVec3::new(0.0, 1000.1, 0.0), DVec3::new(-std::f64::consts::FRAC_PI_2, 0.0, 0.0), 1000.1),
        "front" => (DVec3::new(0.0, 0.0, 1000.1), DVec3::ZERO, 1000.1),
        "side" => (DVec3::new(1000.1, 0.0, 0.0), DVec3::new(0.0, std::f64::consts::FRAC_PI_2, 0.0), 1000.1),
        _ => (DVec3::new(28.0, 21.0, 28.0), DVec3::new((-27.938352729602379_f64).to_radians(), 45_f64.to_radians(), 0.0), 44.82186966202994),
    };
    if !z_up {
        return (t, r, coi);
    }
    (DVec3::new(t.x, -t.z, t.y), turn_rotation(r), coi)
}

/// A rotate (XYZ, radians) set up for Y up, turned with the world.
pub fn world_rotation(r: DVec3) -> DVec3 {
    if crate::world::up_z() {
        turn_rotation(r)
    } else {
        r
    }
}

/// A rotate (XYZ, radians) turned a quarter about X, from Y up to Z up.
fn turn_rotation(r: DVec3) -> DVec3 {
    let m = DMat3::from_rotation_x(std::f64::consts::FRAC_PI_2) * DMat3::from_euler(glam::EulerRot::ZYX, r.z, r.y, r.x);
    let (rz, ry, rx) = m.to_euler(glam::EulerRot::ZYX);
    let tidy = |v: f64| if v.abs() < 1e-12 { 0.0 } else { v };
    DVec3::new(tidy(rx), tidy(ry), tidy(rz))
}

/// Makes persp, top, front and side, hidden as the reference's are, plus the default sets. None of it goes on the undo stack.
pub fn make_defaults(g: &mut Graph) {
    crate::shading::make_defaults(g);
    if g.find("persp").ok().flatten().is_some() {
        crate::defaults::make(g);
        return;
    }
    g.without_undo(|g| {
        let mut made = |name: &str, t: DVec3, r: DVec3, ortho: bool, coi: f64, home: &str| {
            if let Ok((tr, sh)) = create(g, name, NO_NODE) {
                let _ = g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(t));
                let _ = g.set_value(plug(tr, Transform::ROTATE), PlugValue::Vec3(r));
                let _ = g.set_value(plug(tr, DagNode::VISIBILITY), PlugValue::Bool(false));
                let _ = g.set_value(plug(sh, ORTHOGRAPHIC), PlugValue::Bool(ortho));
                let _ = g.set_value(plug(sh, COI), PlugValue::Float(coi));
                let _ = g.set_value(plug(sh, STARTUP), PlugValue::Bool(true));
                if ortho {
                    let _ = g.set_value(plug(sh, RENDERABLE), PlugValue::Bool(false));
                }
                for (attr, text) in [(IMAGE_NAME, name.to_string()), (DEPTH_NAME, format!("{name}_depth")), (MASK_NAME, format!("{name}_mask")), (HOME_COMMAND, format!("viewSet {home} %camera"))] {
                    let _ = g.set_value(plug(sh, attr), PlugValue::String(text));
                }
            }
        };
        for (name, ortho, flag) in [("persp", false, "-p"), ("top", true, "-t"), ("front", true, "-f"), ("side", true, "-s")] {
            let (t, r, coi) = home(name);
            made(name, t, r, ortho, coi, flag);
        }
        for set in ["defaultLightSet", "defaultObjectSet"] {
            let _ = crate::object_sets::create(g, set, &[], false);
        }
        g.select(&[], SelectMode::Replace);
    });
    crate::defaults::make(g);
}

/// Whether a node is one of the four default cameras (or a shape of one).
pub fn is_default_camera(g: &Graph, id: NodeId) -> bool {
    let tr = crate::dag::transform_of(g, id);
    g.node(tr).is_some_and(|n| matches!(n.name.as_str(), "persp" | "top" | "front" | "side")) && shape_of(g, tr).is_some()
}

type Cmd = CommandDef<Session>;

fn f(name: &str, short: &str, ty: ArgType, doc: &str) -> FlagDef {
    FlagDef::new(name, short, ty, Value::Null, doc)
}

fn asked(a: &aztek_ops::Args, name: &str) -> bool {
    a.bool(name) || a.get("queried").and_then(Value::as_array).is_some_and(|q| q.iter().any(|x| x == name))
}

const NUMS: &[(&str, usize)] = &[
    ("focalLength", FOCAL_LENGTH),
    ("horizontalFilmAperture", H_APERTURE),
    ("verticalFilmAperture", V_APERTURE),
    ("nearClipPlane", NEAR_CLIP),
    ("farClipPlane", FAR_CLIP),
    ("orthographicWidth", ORTHO_WIDTH),
    ("centerOfInterest", COI),
    ("lensSqueezeRatio", LENS_SQUEEZE),
];

pub fn register(c: &mut Commands<Session>) {
    register_types();
    c.register(
        Cmd::new(
            "camera",
            "Create a camera, or query and edit one: -fl focal length, -hfa -vfa film, -ncp -fcp clips, -o orthographic, -ow width, \
             -coi, -p position, -rot rotation, -wci world centre of interest. Returns [camera1, cameraShape1].",
            |s, a| {
                let names = a.named();
                let g = &mut s.graph;
                if asked(a, "startupCamera") || a.get("startupCamera").is_some() {
                    let Some(sh) = names.first().and_then(|n| g.find(n).ok().flatten()).and_then(|id| shape_of(g, id)) else { fail!("camera: name a camera") };
                    if a.bool("query") {
                        return Ok(json!(g.node(sh).unwrap().bool(STARTUP)));
                    }
                    if a.bool("edit") {
                        let on = a.bool("startupCamera");
                        g.without_undo(|g| g.set_value(plug(sh, STARTUP), PlugValue::Bool(on))).map_err(|e| aztek_ops::CommandError::new(e.to_string()))?;
                        return Ok(Value::Null);
                    }
                }
                if a.bool("query") || a.bool("edit") {
                    let n = match names.first() {
                        Some(n) => n.clone(),
                        None => match g.selection().last() {
                            Some(&i) => g.path(i),
                            None => fail!("camera: name a camera"),
                        },
                    };
                    let id = crate::rig::find(g, &n, "camera")?;
                    let Some(sh) = shape_of(g, id) else { fail!("camera: {n} is not a camera") };
                    let tr = crate::dag::transform_of(g, sh);
                    if a.bool("query") {
                        for &(flag, at) in NUMS {
                            if asked(a, flag) {
                                return Ok(json!(g.node(sh).unwrap().data().float(at)));
                            }
                        }
                        if asked(a, "orthographic") {
                            return Ok(json!(g.node(sh).unwrap().data().bool(ORTHOGRAPHIC)));
                        }
                        g.evaluate();
                        let w = crate::rig::world(g, tr);
                        if asked(a, "position") {
                            let p = w.w_axis.truncate();
                            return Ok(json!([p.x, p.y, p.z]));
                        }
                        if asked(a, "rotation") {
                            let r = g.node(tr).unwrap().data().vec3(Transform::ROTATE);
                            return Ok(json!([r.x, r.y, r.z]));
                        }
                        if asked(a, "worldCenterOfInterest") {
                            let d = g.node(sh).unwrap().data().float(COI);
                            let p = w.w_axis.truncate() - w.z_axis.truncate().normalize_or_zero() * d;
                            return Ok(json!([p.x, p.y, p.z]));
                        }
                        return Ok(Value::Null);
                    }
                    return apply(g, a, tr, sh).map(|_| Value::Null);
                }
                g.open_chunk("Camera");
                let r = (|| -> CmdResult<(NodeId, NodeId)> {
                    let (tr, sh) = create(g, &a.string("name", "camera1"), NO_NODE)?;
                    apply(g, a, tr, sh)?;
                    Ok((tr, sh))
                })();
                g.close_chunk();
                let (tr, sh) = r?;
                g.select(&[tr], SelectMode::Replace);
                Ok(json!([g.path(tr), g.path(sh)]))
            },
        )
        .positional("a camera, with -q and -e")
        .flag(f("focalLength", "fl", ArgType::Float, "Focal length, mm."))
        .flag(f("horizontalFilmAperture", "hfa", ArgType::Float, "Film width, inches."))
        .flag(f("verticalFilmAperture", "vfa", ArgType::Float, "Film height, inches."))
        .flag(f("nearClipPlane", "ncp", ArgType::Float, "Near clip."))
        .flag(f("farClipPlane", "fcp", ArgType::Float, "Far clip."))
        .flag(f("orthographic", "o", ArgType::Bool, "No perspective."))
        .flag(f("startupCamera", "sc", ArgType::Bool, "Query or edit: a startup camera, which can't be deleted."))
        .flag(f("orthographicWidth", "ow", ArgType::Float, "View width when orthographic."))
        .flag(f("centerOfInterest", "coi", ArgType::Float, "Distance to what it tumbles about."))
        .flag(f("lensSqueezeRatio", "lsr", ArgType::Float, "Lens squeeze."))
        .flag(f("position", "p", ArgType::Vec3, "World position."))
        .flag(f("rotation", "rot", ArgType::Vec3, "Rotation, degrees."))
        .flag(f("worldCenterOfInterest", "wci", ArgType::Vec3, "The point it looks at."))
        .flag(f("worldUp", "wup", ArgType::Vec3, "Accepted for existing scripts."))
        .flag(f("name", "n", ArgType::String, "Name."))
        .flag(f("query", "q", ArgType::Bool, "Query mode."))
        .flag(f("edit", "e", ArgType::Bool, "Edit mode."))
        .flag(f("queried", "", ArgType::List, "Flags asked about in query mode.")),
    );
}

fn apply(g: &mut Graph, a: &aztek_ops::Args, tr: NodeId, sh: NodeId) -> CmdResult<()> {
    for &(flag, at) in NUMS {
        if let Some(v) = a.get(flag).and_then(Value::as_f64) {
            g.set_value(plug(sh, at), PlugValue::Float(v))?;
        }
    }
    if a.has("orthographic") {
        g.set_value(plug(sh, ORTHOGRAPHIC), PlugValue::Bool(a.bool("orthographic")))?;
    }
    if let Some(p) = a.vec3("position") {
        g.set_value(plug(tr, Transform::TRANSLATE), PlugValue::Vec3(DVec3::from_array(p)))?;
    }
    if let Some(r) = a.vec3("rotation") {
        g.set_value(plug(tr, Transform::ROTATE), PlugValue::Vec3(DVec3::from_array(r)))?;
    }
    if let Some(t) = a.vec3("worldCenterOfInterest") {
        g.evaluate();
        let eye = crate::rig::world(g, tr).w_axis.truncate();
        let d = eye - DVec3::from_array(t);
        let len = d.length();
        if len > 1e-9 {
            let z = d / len;
            let yaw = z.x.atan2(z.z);
            let pitch = z.y.clamp(-1.0, 1.0).asin();
            g.set_value(plug(tr, Transform::ROTATE), PlugValue::Vec3(rotate_for(yaw, pitch)))?;
            g.set_value(plug(sh, COI), PlugValue::Float(len))?;
        }
    }
    let _ = DMat3::IDENTITY;
    Ok(())
}
