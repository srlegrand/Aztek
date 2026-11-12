//! The FBX plug-in's settings, as the reference keeps them: one command a setting (FBXImportSkins -v true, FBXImportSkins
//! -q), the Autodesk Media & Entertainment preset's values to start with, FBXResetImport and FBXResetExport to go back
//! to them, FBXPushSettings and FBXPopSettings around a script's changes, and the take queries of a file read with
//! FBXRead. The import and export options windows set these same commands.

use std::collections::HashMap;
use std::sync::Mutex;

use aztek_ops::{fail, ArgType, Args, CmdResult, CommandDef, Commands, FlagDef};
use serde_json::{json, Value};

use crate::session::Session;

#[derive(Clone, Copy, Debug)]
pub enum Def {
    B(bool),
    S(&'static str),
    F(f64),
    I(i64),
}

/// A setting: its command, its preset value, the values a word setting takes, and what it does.
pub struct Opt {
    pub name: &'static str,
    pub default: Def,
    pub choices: &'static [&'static str],
    pub doc: &'static str,
}

const fn b(name: &'static str, v: bool, doc: &'static str) -> Opt {
    Opt { name, default: Def::B(v), choices: &[], doc }
}
const fn w(name: &'static str, v: &'static str, choices: &'static [&'static str], doc: &'static str) -> Opt {
    Opt { name, default: Def::S(v), choices, doc }
}
const fn f(name: &'static str, v: f64, doc: &'static str) -> Opt {
    Opt { name, default: Def::F(v), choices: &[], doc }
}
const fn i(name: &'static str, v: i64, doc: &'static str) -> Opt {
    Opt { name, default: Def::I(v), choices: &[], doc }
}

const UNITS: &[&str] = &["mm", "dm", "cm", "m", "km", "In", "ft", "yd", "mi"];

/// The import settings. An empty up axis or unit means the scene's.
pub const IMPORT: &[Opt] = &[
    w("FBXImportMode", "merge", &["add", "merge", "exmerge"], "File Content: add (Add), merge (Add and update animation), exmerge (Update animation)."),
    b("FBXImportMergeBackNullPivots", false, "Update Pivots from Nulls: pre and post rotation go to joint orient and rotate axis on every node, not only on joints."),
    b("FBXImportHardEdges", false, "Combine per-vertex Normals."),
    b("FBXImportUnlockNormals", false, "Unlock Normals: the file's normals are not kept; the mesh works its own out."),
    b("FBXImportSmoothingGroups", true, "Smoothing Groups: edges come in hard or soft as the file's smoothing says."),
    b("FBXImportAnimation", true, "Animation."),
    b("FBXImportFillTimeline", false, "Fill Timeline with the take's range."),
    b("FBXImportMergeAnimationLayers", true, "Bake Animation Layers."),
    b("FBXImportOpticalMarkers", false, "Optical Markers."),
    w("FBXImportQuaternion", "resample", &["quaternion", "euler", "resample"], "Quaternion Interpolation Mode."),
    w("FBXImportResamplingRateSource", "Scene", &["Scene", "File", "Custom"], "The key rate when resampling."),
    b("FBXImportProtectDrivenKeys", false, "Protect Driven Keys."),
    b("FBXImportConvertDeformingNullsToJoint", true, "Deforming Elements to Joints: a null a skin is bound to comes in as a joint."),
    b("FBXImportCacheFile", true, "Geometry Cache File(s)."),
    b("FBXImportShapes", true, "Blend Shapes."),
    b("FBXImportSkins", true, "Skins: skin clusters with the file's weights and bind matrices."),
    b("FBXImportPreNormalizeWeights", true, "Pre-Normalize Weights: each point's weights add up to one."),
    b("FBXImportConstraints", true, "Constraints."),
    w("FBXImportSkeletonType", "none", &["none", "fbik", "humanik"], "Skeleton Definitions."),
    b("FBXImportCameras", true, "Cameras."),
    b("FBXImportLights", true, "Lights."),
    b("FBXImportAudio", true, "Audio."),
    b("FBXImportAxisConversionEnable", true, "Axis conversion: a file with another up axis turns its top nodes."),
    w("FBXImportUpAxis", "", &["y", "z"], "The up axis the file is converted to; the scene's unless set."),
    w("FBXImportForcedFileAxis", "disabled", &["disabled", "y", "z"], "Read the file as if it said this up axis."),
    b("FBXImportScaleFactorEnable", false, "Use FBXImportScaleFactor rather than the file's unit."),
    f("FBXImportScaleFactor", 1.0, "The scale applied to the file's lengths when FBXImportScaleFactorEnable is on."),
    w("FBXImportConvertUnitString", "", UNITS, "File Units converted to; the scene's unit unless set."),
    b("FBXImportAutoUnits", true, "Automatic units: the file's unit is converted to the scene's."),
    b("FBXImportSetMayaFrameRate", false, "Take the file's frame rate."),
    b("FBXImportSetLockedAttribute", false, "Channels locked in the file stay locked."),
    b("FBXImportGenerateLog", true, "Generate log data."),
    b("FBXImportShowUI", true, "Show the warnings manager."),
    b("FBXImportRemoveBadPolysFromMesh", true, "Test meshes and remove bad polygons."),
    i("FBXImportSetTake", -1, "The take to import: -1 the last, 0 none."),
    b("FBXImportDxfWeldVertice", true, "DXF: weld vertices."),
    b("FBXImportDxfReferenceNode", false, "DXF: reference node."),
    w("FBXImportDxfObjectDerivation", "layer", &["layer", "entity", "block"], "DXF: object derivation."),
];

/// The export settings.
pub const EXPORT: &[Opt] = &[
    b("FBXExportSmoothingGroups", false, "Smoothing Groups."),
    b("FBXExportHardEdges", false, "Split per-vertex Normals."),
    b("FBXExportTangents", false, "Tangents and Binormals."),
    b("FBXExportSmoothMesh", true, "Smooth Mesh."),
    b("FBXExportSelectionSets", false, "Selection Sets."),
    b("FBXExportNullsAsLocators", false, "Convert to Null Objects."),
    b("FBXExportInstances", false, "Preserve Instances."),
    b("FBXExportReferencedAssetsContent", true, "Referenced Asset Content."),
    b("FBXExportTriangulate", false, "Triangulate."),
    w("FBXExportConvertNurbsSurfaceTo", "nurbs", &["nurbs", "interactiveDisplayMesh", "softwareRenderMesh"], "Convert NURBS surface to."),
    b("FBXExportAnimationOnly", false, "Animation only."),
    b("FBXExportAnimation", true, "Animation."),
    b("FBXExportUseSceneName", false, "Use scene name for the take."),
    b("FBXExportRemoveSingleKey", false, "Remove single key."),
    w("FBXExportQuaternion", "resample", &["quaternion", "euler", "resample"], "Quaternion interpolation mode."),
    b("FBXExportBakeComplexAnimation", false, "Bake Animation."),
    i("FBXExportBakeComplexStart", 1, "Bake start."),
    i("FBXExportBakeComplexEnd", 120, "Bake end."),
    i("FBXExportBakeComplexStep", 1, "Bake step."),
    b("FBXExportBakeResampleAnimation", false, "Resample All."),
    b("FBXExportShapes", true, "Blend Shapes."),
    b("FBXExportSkins", true, "Skins."),
    b("FBXExportCurveFilters", false, "Curve filters."),
    b("FBXExportApplyConstantKeyReducer", false, "Constant Key Reducer."),
    b("FBXExportCacheFile", false, "Geometry cache file(s)."),
    w("FBXExportQuickSelectSetAsCache", "", &[], "The set exported as a geometry cache."),
    b("FBXExportConstraints", false, "Constraints."),
    b("FBXExportSkeletonDefinitions", false, "Skeleton definitions."),
    b("FBXExportCameras", true, "Cameras."),
    b("FBXExportLights", true, "Lights."),
    b("FBXExportAudio", true, "Audio."),
    b("FBXExportEmbeddedTextures", false, "Embed Media: the texture files go inside the FBX file."),
    b("FBXExportIncludeChildren", true, "Include Children."),
    b("FBXExportInputConnections", true, "Input Connections."),
    w("FBXExportUpAxis", "", &["y", "z"], "The file's up axis; the scene's unless set."),
    w("FBXExportAxisConversionMethod", "convertAnimation", &["none", "convertAnimation", "addFbxRoot"], "How a change of up axis is made."),
    f("FBXExportScaleFactor", 1.0, "The scale factor."),
    w("FBXExportConvertUnitString", "", UNITS, "File units converted to; the scene's unit unless set."),
    b("FBXExportAutoUnits", true, "Automatic units."),
    b("FBXExportGenerateLog", true, "Generate log data."),
    b("FBXExportShowUI", true, "Show the warnings manager."),
    b("FBXExportInAscii", false, "ASCII rather than binary."),
    w("FBXExportFileVersion", "FBX202000", &["FBX202000", "FBX201900", "FBX201800", "FBX201600", "FBX201400", "FBX201300", "FBX201200", "FBX201100", "FBX201000", "FBX200900", "FBX200611"], "The FBX version written."),
    b("FBXExportColladaTriangulate", true, "Collada: triangulate."),
    b("FBXExportColladaSingleMatrix", true, "Collada: single matrix."),
    f("FBXExportColladaFrameRate", 24.0, "Collada: frame rate."),
    b("FBXExportDxfTriangulate", true, "DXF: triangulate."),
    b("FBXExportDxfDeformation", true, "DXF: deformation."),
    f("FBXResamplingRate", 24.0, "The rate keys are made at when animation is resampled."),
];

type Values = HashMap<&'static str, Value>;

static VALUES: Mutex<Option<Values>> = Mutex::new(None);
static PUSHED: Mutex<Vec<Values>> = Mutex::new(Vec::new());
/// FBXExportSplitAnimationIntoTakes: (name, start, end).
static TAKES: Mutex<Vec<(String, f64, f64)>> = Mutex::new(Vec::new());
/// The file FBXRead read, for the take queries.
static READ: Mutex<Option<String>> = Mutex::new(None);

fn def_value(d: Def) -> Value {
    match d {
        Def::B(v) => json!(v),
        Def::S(v) => json!(v),
        Def::F(v) => json!(v),
        Def::I(v) => json!(v),
    }
}

fn with<R>(f: impl FnOnce(&mut Values) -> R) -> R {
    let mut g = VALUES.lock().unwrap();
    let v = g.get_or_insert_with(defaults);
    f(v)
}

fn defaults() -> Values {
    IMPORT.iter().chain(EXPORT).map(|o| (o.name, def_value(o.default))).collect()
}

fn opt(name: &str) -> Option<&'static Opt> {
    IMPORT.iter().chain(EXPORT).find(|o| o.name == name)
}

/// A setting's value now.
pub fn get(name: &str) -> Value {
    with(|v| v.get(name).cloned()).unwrap_or(Value::Null)
}

pub fn on(name: &str) -> bool {
    get(name).as_bool().unwrap_or(false)
}

pub fn word(name: &str) -> String {
    get(name).as_str().unwrap_or("").to_string()
}

pub fn number(name: &str) -> f64 {
    get(name).as_f64().unwrap_or(0.0)
}

/// Sets a setting from a script's word or number.
pub fn set(name: &str, given: &Value) -> Result<(), String> {
    let Some(o) = opt(name) else { return Err(format!("{name}: no such setting")) };
    let v = match o.default {
        Def::B(_) => json!(truth(given).ok_or_else(|| format!("{name}: give true or false"))?),
        Def::F(_) => json!(given.as_f64().or_else(|| given.as_str().and_then(|s| s.trim().parse().ok())).ok_or_else(|| format!("{name}: give a number"))?),
        Def::I(_) => json!(given.as_f64().or_else(|| given.as_str().and_then(|s| s.trim().parse().ok())).ok_or_else(|| format!("{name}: give a number"))? as i64),
        Def::S(_) => {
            let s = given.as_str().map(str::to_string).unwrap_or_else(|| given.to_string());
            let s = match o.choices.iter().find(|c| c.eq_ignore_ascii_case(&s)) {
                Some(c) => c.to_string(),
                None if o.choices.is_empty() || name == "FBXExportFileVersion" => s,
                None => return Err(format!("{name}: give one of {}", o.choices.join(", "))),
            };
            json!(s)
        }
    };
    with(|m| m.insert(o.name, v));
    Ok(())
}

fn truth(v: &Value) -> Option<bool> {
    match v {
        Value::Bool(b) => Some(*b),
        Value::Number(n) => n.as_f64().map(|x| x != 0.0),
        Value::String(s) => match s.trim().to_ascii_lowercase().as_str() {
            "true" | "on" | "yes" | "1" => Some(true),
            "false" | "off" | "no" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Centimetres in one of the units the FBX commands name.
pub fn unit_cm(u: &str) -> Option<f64> {
    Some(match u.to_ascii_lowercase().as_str() {
        "mm" => 0.1,
        "dm" => 10.0,
        "cm" => 1.0,
        "m" => 100.0,
        "km" => 100000.0,
        "in" => 2.54,
        "ft" => 30.48,
        "yd" => 91.44,
        "mi" => 160934.4,
        _ => return None,
    })
}

pub fn reset_import() {
    with(|m| {
        for o in IMPORT {
            m.insert(o.name, def_value(o.default));
        }
    });
}

pub fn reset_export() {
    with(|m| {
        for o in EXPORT {
            m.insert(o.name, def_value(o.default));
        }
    });
    TAKES.lock().unwrap().clear();
}

/// The value given: -v, or the first word after the command.
fn given(a: &Args) -> Option<Value> {
    a.get("value").cloned().or_else(|| a.positional.first().cloned())
}

fn setting_cmd(name: &'static str) -> impl Fn(&mut Session, &Args) -> CmdResult + Send + Sync + 'static {
    move |_s, a| {
        if a.bool("query") || given(a).is_none() {
            return Ok(match name {
                "FBXImportUpAxis" | "FBXExportUpAxis" if word(name).is_empty() => json!(if crate::world::up_z() { "z" } else { "y" }),
                _ => get(name),
            });
        }
        let v = given(a).unwrap();
        if let Err(e) = set(name, &v) {
            fail!("{e}");
        }
        Ok(Value::Null)
    }
}

/// FBXImportConvertUnitString and FBXExportConvertUnitString: set the unit and give how many of it make a centimetre.
fn unit_cmd(name: &'static str) -> impl Fn(&mut Session, &Args) -> CmdResult + Send + Sync + 'static {
    move |_s, a| {
        if a.bool("query") || given(a).is_none() {
            return Ok(get(name));
        }
        let v = given(a).unwrap();
        let u = v.as_str().unwrap_or("").to_string();
        let Some(cm) = unit_cm(&u) else { fail!("{name}: give one of {}", UNITS.join(", ")) };
        if let Err(e) = set(name, &v) {
            fail!("{e}");
        }
        Ok(json!(1.0 / cm))
    }
}

/// The takes in a file: (name, start, stop) in seconds.
pub fn takes_of(path: &str) -> Result<Vec<(String, f64, f64)>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{path}: {e}"))?;
    let top = crate::fbx_import::parse(&bytes)?;
    Ok(crate::fbx_import::takes(&top))
}

fn read_takes() -> Result<Vec<(String, f64, f64)>, String> {
    let Some(path) = READ.lock().unwrap().clone() else { return Err("no file has been read: use FBXRead -f first".to_string()) };
    takes_of(&path)
}

fn take_at(a: &Args, cmd: &str) -> Result<(String, f64, f64), String> {
    let takes = read_takes()?;
    let k = a.positional.first().and_then(|v| v.as_i64().or_else(|| v.as_str().and_then(|s| s.parse().ok()))).unwrap_or(0);
    usize::try_from(k - 1).ok().and_then(|k| takes.get(k).cloned()).ok_or_else(|| format!("{cmd}: No such take"))
}

fn fps() -> f64 {
    crate::anim::fps()
}

pub fn register(c: &mut Commands<Session>) {
    let val = || FlagDef::new("value", "v", ArgType::Any, Value::Null, "The value.");
    let q = || FlagDef::new("query", "q", ArgType::Bool, Value::Null, "Give the value.");
    for o in IMPORT.iter().chain(EXPORT).filter(|o| o.name != "FBXImportSetTake") {
        let doc = format!("{} -v value sets it, -q gives it. Starts as {}.", o.doc, def_value(o.default));
        let cmd = if o.name.ends_with("ConvertUnitString") {
            CommandDef::<Session>::new(o.name, &doc, unit_cmd(o.name))
        } else {
            CommandDef::<Session>::new(o.name, &doc, setting_cmd(o.name))
        };
        c.register(cmd.positional("value").flag(val()).flag(q()).undoable(false));
    }
    // -ti is the take's own flag.
    c.register(
        CommandDef::<Session>::new("FBXImportSetTake", "The take FBXImport reads: -ti index, 1 the first, -1 the last, 0 none.", |_s, a| {
            if a.bool("query") {
                return Ok(get("FBXImportSetTake"));
            }
            let v = a.get("takeIndex").cloned().or_else(|| given(a)).unwrap_or(json!(-1));
            set("FBXImportSetTake", &v).map_err(aztek_ops::CommandError::new)?;
            Ok(Value::Null)
        })
        .positional("index")
        .flag(FlagDef::new("takeIndex", "ti", ArgType::Int, Value::Null, "The take."))
        .flag(val())
        .flag(q())
        .undoable(false),
    );
    let plain = |name: &str, doc: &str, f: fn(&mut Session, &Args) -> CmdResult| CommandDef::<Session>::new(name, doc, f).undoable(false);
    c.register(plain("FBXResetImport", "Every import setting back to the Autodesk Media & Entertainment preset.", |_s, _a| {
        reset_import();
        Ok(Value::Null)
    }));
    c.register(plain("FBXResetExport", "Every export setting back to the Autodesk Media & Entertainment preset.", |_s, _a| {
        reset_export();
        Ok(Value::Null)
    }));
    c.register(plain("FBXPushSettings", "Keep every import and export setting, for FBXPopSettings to bring back.", |_s, _a| {
        let now = with(|m| m.clone());
        PUSHED.lock().unwrap().push(now);
        Ok(Value::Null)
    }));
    c.register(plain("FBXPopSettings", "Bring back the settings FBXPushSettings kept.", |_s, _a| {
        if let Some(v) = PUSHED.lock().unwrap().pop() {
            *VALUES.lock().unwrap() = Some(v);
        }
        Ok(Value::Null)
    }));
    for (name, doc) in [("FBXLoadImportPresetFile", "Load an import preset file: -f path."), ("FBXLoadExportPresetFile", "Load an export preset file: -f path.")] {
        c.register(
            CommandDef::<Session>::new(name, doc, |_s, a| {
                let path = a.string("file", "");
                if !std::path::Path::new(&path).is_file() {
                    fail!("FBX preset: {path}: no such file");
                }
                log::warn!("FBX preset files are not read: the settings stay as they are. Set them with the FBXImport and FBXExport commands.");
                Ok(Value::Null)
            })
            .flag(FlagDef::new("file", "f", ArgType::String, Value::Null, "The preset file."))
            .undoable(false),
        );
    }
    c.register(plain("FBXLoadMBImportPresetFile", "The MotionBuilder import preset: constraints and skeleton definitions on.", |_s, _a| {
        reset_import();
        let _ = set("FBXImportConstraints", &json!(true));
        let _ = set("FBXImportSkeletonType", &json!("humanik"));
        Ok(Value::Null)
    }));
    c.register(plain("FBXLoadMBExportPresetFile", "The MotionBuilder export preset: constraints and skeleton definitions on.", |_s, _a| {
        reset_export();
        let _ = set("FBXExportConstraints", &json!(true));
        let _ = set("FBXExportSkeletonDefinitions", &json!(true));
        Ok(Value::Null)
    }));
    c.register(
        CommandDef::<Session>::new("FBXExportSplitAnimationIntoTakes", "Takes to write: -v name start end adds one, -c clears them, -q lists them.", |_s, a| {
            if a.bool("clear") {
                TAKES.lock().unwrap().clear();
                return Ok(Value::Null);
            }
            if a.bool("query") {
                return Ok(json!(TAKES.lock().unwrap().iter().map(|(n, s, e)| json!([n, s, e])).collect::<Vec<_>>()));
            }
            let mut parts: Vec<Value> = Vec::new();
            if let Some(v) = a.get("value") {
                parts.push(v.clone());
            }
            parts.extend(a.positional.iter().cloned());
            let num = |v: Option<&Value>| v.and_then(|x| x.as_f64().or_else(|| x.as_str().and_then(|s| s.parse().ok())));
            let (Some(name), Some(s), Some(e)) = (parts.first().and_then(Value::as_str), num(parts.get(1)), num(parts.get(2))) else {
                fail!("FBXExportSplitAnimationIntoTakes: give -v \"name\" start end");
            };
            TAKES.lock().unwrap().push((name.to_string(), s, e));
            Ok(Value::Null)
        })
        .positional("start end")
        .flag(val())
        .flag(FlagDef::new("clear", "c", ArgType::Bool, Value::Null, "Clear the takes."))
        .flag(q())
        .undoable(false),
    );
    c.register(
        CommandDef::<Session>::new("FBXRead", "Read a file's takes without importing it: -f path. The take commands then ask about it.", |_s, a| {
            let path = a.string("file", "");
            match takes_of(&path) {
                Ok(_) => *READ.lock().unwrap() = Some(path),
                Err(e) => {
                    *READ.lock().unwrap() = None;
                    fail!("FBXRead: {e}");
                }
            }
            Ok(Value::Null)
        })
        .flag(FlagDef::new("file", "f", ArgType::String, Value::Null, "The file."))
        .undoable(false),
    );
    c.register(plain("FBXClose", "Let go of the file FBXRead read.", |_s, _a| {
        *READ.lock().unwrap() = None;
        Ok(Value::Null)
    }));
    c.register(plain("FBXGetTakeCount", "How many takes the file FBXRead read has.", |_s, _a| Ok(json!(read_takes().map(|t| t.len()).unwrap_or(0)))));
    c.register(
        CommandDef::<Session>::new("FBXGetTakeName", "The name of a take of the file FBXRead read, by index from 1.", |_s, a| take_at(a, "FBXGetTakeName").map(|t| json!(t.0)).map_err(aztek_ops::CommandError::new))
            .positional("index")
            .undoable(false),
    );
    c.register(
        CommandDef::<Session>::new("FBXGetTakeComment", "A take's comment, by index from 1.", |_s, a| take_at(a, "FBXGetTakeComment").map(|_| json!("")).map_err(aztek_ops::CommandError::new))
            .positional("index")
            .undoable(false),
    );
    for name in ["FBXGetTakeLocalTimeSpan", "FBXGetTakeReferenceTimeSpan"] {
        c.register(
            CommandDef::<Session>::new(name, "A take's start and stop, in frames at the scene's rate, by index from 1.", move |_s, a| {
                take_at(a, name).map(|t| json!([t.1 * fps(), t.2 * fps()])).map_err(aztek_ops::CommandError::new)
            })
            .positional("index")
            .undoable(false),
        );
    }
    c.register(
        CommandDef::<Session>::new("FBXGetTakeIndex", "The index, from 1, of the first take with this name.", |_s, a| {
            let takes = read_takes().map_err(aztek_ops::CommandError::new)?;
            let name = a.pos_str(0).unwrap_or("");
            match takes.iter().position(|t| t.0 == name) {
                Some(k) => Ok(json!(k + 1)),
                None => fail!("FBXGetTakeIndex: No such take"),
            }
        })
        .positional("name")
        .undoable(false),
    );
    c.register(
        CommandDef::<Session>::new("FBXExtPlugin", "FBX extension plug-ins: -l lists them. None are installed.", |_s, _a| Ok(json!([])))
            .positional("plugin")
            .flag(FlagDef::new("list", "l", ArgType::Bool, Value::Null, "List them."))
            .flag(FlagDef::new("state", "state", ArgType::String, Value::Null, "on or off."))
            .flag(FlagDef::new("opts", "opts", ArgType::String, Value::Null, "Options."))
            .flag(q())
            .undoable(false),
    );
}

/// Every command this module registers.
pub fn command_names() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = IMPORT.iter().chain(EXPORT).map(|o| o.name).collect();
    v.extend([
        "FBXResetImport",
        "FBXResetExport",
        "FBXPushSettings",
        "FBXPopSettings",
        "FBXLoadImportPresetFile",
        "FBXLoadExportPresetFile",
        "FBXLoadMBImportPresetFile",
        "FBXLoadMBExportPresetFile",
        "FBXExportSplitAnimationIntoTakes",
        "FBXRead",
        "FBXClose",
        "FBXGetTakeCount",
        "FBXGetTakeName",
        "FBXGetTakeComment",
        "FBXGetTakeLocalTimeSpan",
        "FBXGetTakeReferenceTimeSpan",
        "FBXGetTakeIndex",
        "FBXExtPlugin",
    ]);
    v.sort_unstable();
    v.dedup();
    v
}
