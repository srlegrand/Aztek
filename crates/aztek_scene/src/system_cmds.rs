//! The reference's system and query commands: scene up axis, units, display colours and preferences kept for scripts, evaluation
//! settings, node locks, and asking about attributes and nodes.
//!
//! Stubs, so scripts run: dgtimer returns zeros (nothing is timed). hitTest returns nothing: only the UI knows a panel's
//! camera and size. evaluationManager and threadCount hold what they are told; evaluation doesn't change.

use std::collections::{BTreeMap, HashSet};

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, Commands};
use glam::{DMat3, DVec3, EulerRot};
use serde_json::{json, Value};

use crate::ref_core::{asked, f};
use crate::plug::{flags, NodeId, PlugRef, PlugType, PlugValue};
use crate::session::Session;
use crate::transform::Transform;

type Cmd = CommandDef<Session>;
use ArgType::{Bool as B, Float as F, Int as I, List as L, String as S};

/// Scene and session settings the commands in this module and its neighbours hold.
#[derive(Default)]
pub struct Extras {
    /// False is Y up.
    pub up_axis_z: bool,
    pub cycle_check: Option<bool>,
    pub evaluation_mode: Option<String>,
    pub thread_count: Option<i64>,
    pub display_pref: BTreeMap<String, Value>,
    pub display_stats: BTreeMap<String, Value>,
    pub display_surface: BTreeMap<(NodeId, String), bool>,
    pub level_of_detail: bool,
    pub select_priority: BTreeMap<String, i64>,
    pub dg_timer_on: bool,
    /// animDisplay's settings.
    pub anim_display: BTreeMap<String, Value>,
    /// recordAttr: plugs marked for recording.
    pub recording: Vec<String>,
    /// symmetricModelling's settings.
    pub symmetry: crate::mesh_edit::ref_poly::SymmetryState,
    /// The scene's file references.
    pub references: crate::references::RefTable,
    /// The state of general_cmds and its neighbours.
    pub more: crate::general_cmds::MoreState,
    /// The renderer's running render, interactive state and kept images.
    pub harold: crate::harold::commands::State,
}

/// What a query asks for: the first flag -q names.
pub(crate) fn q_one(a: &Args) -> Option<String> {
    a.get("queried").and_then(Value::as_array).and_then(|v| v.first()).and_then(Value::as_str).map(str::to_string)
}

pub(crate) fn q_flags(c: Cmd) -> Cmd {
    c.flag(f("query", "q", B, "Query mode.")).flag(f("edit", "e", B, "Edit mode.")).flag(f("queried", "", L, "Flags asked about in query mode."))
}

fn on_off(v: &Value) -> bool {
    v.as_bool().unwrap_or(v.as_i64().unwrap_or(0) != 0)
}

// ---------------------------------------------------------------------------------------------------------------------
// upAxis

fn up_axis_cmd(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        return Ok(json!(if s.extras.up_axis_z { "z" } else { "y" }));
    }
    let Some(axis) = a.get("axis").and_then(Value::as_str) else { fail!("upAxis: use -axis y or z, or -q -axis") };
    let z = match axis {
        "y" | "Y" => false,
        "z" | "Z" => true,
        _ => fail!("upAxis: the axis must be y or z"),
    };
    if z != s.extras.up_axis_z {
        s.extras.up_axis_z = z;
        if a.get("rotateView").map_or(true, on_off) {
            rotate_cameras(s, z);
        }
    }
    Ok(json!(if z { "z" } else { "y" }))
}

/// The default cameras to how they sit under the new up axis. Others stay, as scene content.
pub fn rotate_cameras(s: &mut Session, z: bool) {
    let g = &mut s.graph;
    let turn = DMat3::from_rotation_x(if z { 90f64.to_radians() } else { (-90f64).to_radians() });
    g.without_undo(|g| {
        for name in ["persp", "top", "front", "side"] {
            let Ok(Some(id)) = g.find(name) else { continue };
            let n = g.node(id).unwrap();
            if !n.ty().is_a("transform") {
                continue;
            }
            let (t, r) = (n.vec3(Transform::TRANSLATE), n.vec3(Transform::ROTATE));
            // The Euler angles in xyz order: R = Rz Ry Rx.
            let m = DMat3::from_euler(EulerRot::ZYX, r.z.to_radians(), r.y.to_radians(), r.x.to_radians());
            let (rz, ry, rx) = (turn * m).to_euler(EulerRot::ZYX);
            let _ = g.set_value(PlugRef::new(id, Transform::TRANSLATE as i32), PlugValue::Vec3(turn * t));
            let _ = g.set_value(PlugRef::new(id, Transform::ROTATE as i32), PlugValue::Vec3(DVec3::new(rx, ry, rz) * (180.0 / std::f64::consts::PI)));
        }
    });
}

// ---------------------------------------------------------------------------------------------------------------------
// convertUnit

/// A unit's size in its family's base unit: (family, factor).
fn unit_of(u: &str) -> Option<(u8, f64)> {
    Some(match u {
        "mm" | "millimeter" => (0, 0.1),
        "cm" | "centimeter" => (0, 1.0),
        "m" | "meter" => (0, 100.0),
        "km" | "kilometer" => (0, 100_000.0),
        "in" | "inch" => (0, 2.54),
        "ft" | "foot" => (0, 30.48),
        "yd" | "yard" => (0, 91.44),
        "mi" | "mile" => (0, 160_934.4),
        "deg" | "degree" => (1, 1.0),
        "rad" | "radian" => (1, 180.0 / std::f64::consts::PI),
        "min" | "arcminute" => (1, 1.0 / 60.0),
        "sec" | "arcsecond" => (1, 1.0 / 3600.0),
        _ => return None,
    })
}

fn fmt_number(v: f64) -> String {
    let t = format!("{v:.6}");
    let t = t.trim_end_matches('0').trim_end_matches('.');
    if t.is_empty() || t == "-0" {
        "0".to_string()
    } else {
        t.to_string()
    }
}

fn convert_unit(_s: &mut Session, a: &Args) -> CmdResult {
    let text = match a.positional.first() {
        Some(Value::String(t)) => t.clone(),
        Some(Value::Number(n)) => n.to_string(),
        _ => fail!("convertUnit: give a value"),
    };
    let split = text.find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+')).unwrap_or(text.len());
    let (num, suffix) = text.split_at(split);
    let Ok(v) = num.parse::<f64>() else { fail!("convertUnit: {text} is not a value") };
    let from = a.get("fromUnit").and_then(Value::as_str).filter(|x| !x.is_empty()).map(str::to_string).or((!suffix.is_empty()).then(|| suffix.to_string())).unwrap_or("cm".to_string());
    let Some((fa, fb)) = unit_of(&from) else { fail!("convertUnit: unknown unit {from}") };
    let to = a.get("toUnit").and_then(Value::as_str).map(str::to_string).unwrap_or(if fa == 0 { "cm".to_string() } else { "deg".to_string() });
    let Some((ta, tb)) = unit_of(&to) else { fail!("convertUnit: unknown unit {to}") };
    if fa != ta {
        fail!("convertUnit: {from} and {to} don't measure the same thing");
    }
    Ok(json!(format!("{}{to}", fmt_number(v * fb / tb))))
}

// ---------------------------------------------------------------------------------------------------------------------
// copyAttr

fn copy_attr(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let [src, dst] = names.as_slice() else { fail!("copyAttr: name or select a source and a target") };
    let (Some(from), Some(to)) = (g.find(src)?, g.find(dst)?) else { fail!("copyAttr: no such object") };
    let only: Option<HashSet<String>> = a.get("attribute").map(|v| match v {
        Value::Array(l) => l.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        Value::String(t) => [t.clone()].into_iter().collect(),
        _ => HashSet::new(),
    });
    let wants = |at: &crate::plug::AttrDef| only.as_ref().is_none_or(|o| o.contains(&at.name) || o.contains(&at.short_name));
    let (values, ic, oc, keep) = (a.bool("values"), a.bool("inConnections"), a.bool("outConnections"), a.bool("keepSourceConnections"));
    let t_src = g.node(from).unwrap().type_arc().clone();
    let t_dst = g.node(to).unwrap().type_arc().clone();
    let pairs: Vec<(usize, usize)> = t_src.attrs.iter().enumerate().filter(|(_, x)| wants(x)).filter_map(|(i, x)| t_dst.attr_index(&x.name).map(|j| (i, j))).collect();
    g.open_chunk("copyAttr");
    let r = (|| -> CmdResult<()> {
        for &(i, j) in &pairs {
            let def = &t_src.attrs[i];
            if t_dst.attrs[j].ty != def.ty {
                continue;
            }
            let (ps, pd) = (PlugRef::new(from, i as i32), PlugRef::new(to, j as i32));
            if values && def.has(flags::INPUT) && !def.is_array() && g.source(&pd).is_none() && !g.is_locked(&pd) && !matches!(def.ty, PlugType::Mesh | PlugType::Data) {
                let v = g.value(&ps)?;
                g.set_value(pd, v)?;
            }
            // Components and elements keep their place on the new plug.
            if ic || oc {
                for (d, up) in g.connections() {
                    if ic && d.node == from && d.attr == i as i32 {
                        let mut to_plug = d;
                        to_plug.node = to;
                        to_plug.attr = j as i32;
                        g.connect(&up, &to_plug, true)?;
                        if !keep {
                            g.disconnect(&d);
                        }
                    }
                    if oc && !keep && up.node == from && up.attr == i as i32 && d.node != to {
                        let mut from_plug = up;
                        from_plug.node = to;
                        from_plug.attr = j as i32;
                        g.connect(&from_plug, &d, true)?;
                    }
                }
            }
        }
        Ok(())
    })();
    g.close_chunk();
    r?;
    g.evaluate();
    Ok(json!(true))
}

// ---------------------------------------------------------------------------------------------------------------------
// cycleCheck

/// A plug with its element and component, as connections name them.
type PlugKey = (NodeId, i32, i32, i32);

fn key(p: &PlugRef) -> PlugKey {
    (p.node, p.attr, p.element, p.component)
}

fn cycle_walk(at: PlugKey, next: &BTreeMap<PlugKey, Vec<PlugKey>>, state: &mut BTreeMap<PlugKey, u8>, stack: &mut Vec<PlugKey>) -> Option<Vec<PlugKey>> {
    match state.get(&at) {
        Some(1) => {
            let pos = stack.iter().position(|x| *x == at).unwrap_or(0);
            return Some(stack[pos..].to_vec());
        }
        Some(_) => return None,
        None => {}
    }
    state.insert(at, 1);
    stack.push(at);
    for &n in next.get(&at).map(Vec::as_slice).unwrap_or(&[]) {
        if let Some(c) = cycle_walk(n, next, state, stack) {
            return Some(c);
        }
    }
    stack.pop();
    state.insert(at, 2);
    None
}

/// Plugs on a dependency cycle reachable from the given nodes (every node when none): a connection feeds its destination
/// plug, and an input feeds whatever of its node's computed outputs drive something. A stored plug feeds only itself, so
/// tx through a node into ty is no cycle, as in the reference.
fn find_cycle(g: &crate::graph::Graph, from: Option<&[NodeId]>) -> Vec<PlugRef> {
    let conns = g.connections();
    let mut next: BTreeMap<PlugKey, Vec<PlugKey>> = BTreeMap::new();
    for (dst, src) in &conns {
        next.entry(key(src)).or_default().push(key(dst));
        for (_, out) in conns.iter().filter(|(_, o)| o.node == dst.node) {
            if g.attr(out).has(flags::OUTPUT) {
                next.entry(key(dst)).or_default().push(key(out));
            }
        }
    }
    let mut state = BTreeMap::new();
    let mut stack = Vec::new();
    let starts: Vec<PlugKey> = next.keys().filter(|k| from.is_none_or(|l| l.contains(&k.0))).copied().collect();
    for st in starts {
        if let Some(c) = cycle_walk(st, &next, &mut state, &mut stack) {
            let refs: Vec<PlugRef> = conns.iter().flat_map(|(d, s)| [*d, *s]).collect();
            return c.into_iter().filter_map(|k| refs.iter().find(|p| key(p) == k).copied()).collect();
        }
    }
    Vec::new()
}

fn cycle_check(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        return Ok(json!(s.extras.cycle_check.unwrap_or(false)));
    }
    if let Some(v) = a.get("evaluation") {
        s.extras.cycle_check = Some(on_off(v) || v.as_str() == Some("on"));
        return Ok(Value::Null);
    }
    // The reference's -e here is evaluation: cycleCheck -e on, or -e alone for on.
    if a.bool("edit") {
        let off = matches!(a.positional.first(), Some(v) if v.as_str() == Some("off") || v == &json!(false) || v == &json!(0));
        s.extras.cycle_check = Some(!off);
        return Ok(Value::Null);
    }
    let g = &s.graph;
    let names = a.named();
    let ids: Vec<NodeId> = names.iter().filter_map(|n| g.find(n).ok().flatten()).collect();
    let cycle = find_cycle(g, if a.bool("all") || ids.is_empty() { None } else { Some(&ids) });
    if a.bool("list") {
        return Ok(Value::Array(cycle.iter().map(|p| json!(g.plug_name(p))).collect()));
    }
    Ok(json!(!cycle.is_empty()))
}

// ---------------------------------------------------------------------------------------------------------------------
// Display colours

use crate::draw_style::{self, DISPLAY_COLORS};

fn display_color(_s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("list") {
        return Ok(Value::Array(DISPLAY_COLORS.iter().map(|c| json!(c.0)).collect()));
    }
    if a.bool("resetToFactory") || a.bool("resetToSaved") {
        draw_style::reset_display_indices();
        return Ok(Value::Null);
    }
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("displayColor: name a colour") };
    let active = a.bool("active");
    let known = DISPLAY_COLORS.iter().find(|c| c.0 == name);
    if a.bool("query") {
        let Some(i) = draw_style::display_index(&name, active) else { fail!("displayColor: no colour named {name}") };
        return Ok(json!(i));
    }
    let Some(i) = a.positional.get(1).and_then(Value::as_i64) else { fail!("displayColor: give an index, 1 to 31") };
    if !(0..=31).contains(&i) {
        fail!("displayColor: the index must be 0 to 31");
    }
    if known.is_none() && !a.bool("create") {
        fail!("displayColor: no colour named {name}; use -create");
    }
    draw_style::set_display_index(&name, active, i);
    Ok(Value::Null)
}

fn display_rgb(_s: &mut Session, a: &Args) -> CmdResult {
    const NAMED: &[(&str, [f64; 4])] = &[
        ("background", [0.63, 0.63, 0.63, 1.0]),
        ("backgroundTop", [0.54, 0.62, 0.70, 1.0]),
        ("backgroundBottom", [0.05, 0.05, 0.05, 1.0]),
        ("gridColor", [0.33, 0.33, 0.33, 1.0]),
        ("gridAxis", [0.0, 0.0, 0.0, 1.0]),
        ("gridHighlight", [0.0, 0.0, 0.0, 1.0]),
        ("lead", [0.4, 1.0, 0.647, 1.0]),
    ];
    if a.bool("list") {
        return Ok(Value::Array(NAMED.iter().map(|c| json!(format!("{} {} {} {}", c.0, c.1[0], c.1[1], c.1[2]))).collect()));
    }
    if a.bool("resetToFactory") || a.bool("resetToSaved") {
        draw_style::reset_display_rgb();
        return Ok(Value::Null);
    }
    let Some(name) = a.pos_str(0).map(str::to_string) else { fail!("displayRGBColor: name a colour") };
    let known = NAMED.iter().find(|c| c.0 == name).map(|c| c.1);
    if a.bool("query") {
        let Some(v) = draw_style::display_rgb(&name).or(known) else { fail!("displayRGBColor: no colour named {name}") };
        return Ok(if a.bool("alpha") { json!([v[0], v[1], v[2], v[3]]) } else { json!([v[0], v[1], v[2]]) });
    }
    let n: Vec<f64> = a.positional.iter().skip(1).filter_map(Value::as_f64).collect();
    if n.len() < 3 {
        fail!("displayRGBColor: give r g b");
    }
    if known.is_none() && !a.bool("create") {
        fail!("displayRGBColor: no colour named {name}; use -create");
    }
    draw_style::set_display_rgb(&name, [n[0], n[1], n[2], n.get(3).copied().unwrap_or(1.0)]);
    Ok(Value::Null)
}

fn color_index(_s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("resetToFactory") || a.bool("resetToSaved") {
        draw_style::reset_palette();
        return Ok(Value::Null);
    }
    let Some(i) = a.positional.first().and_then(Value::as_i64) else { fail!("colorIndex: give an index") };
    // Entry 0 means no colour: it can be asked about, not set.
    if a.bool("query") && (0..=31).contains(&i) {
        return Ok(json!(draw_style::palette(i)));
    }
    if !(1..=31).contains(&i) {
        fail!("colorIndex: the index must be 1 to 31");
    }
    let n: Vec<f64> = a.positional.iter().skip(1).filter_map(Value::as_f64).collect();
    if n.len() < 3 {
        fail!("colorIndex: give r g b");
    }
    draw_style::set_palette(i, [n[0], n[1], n[2]]);
    Ok(Value::Null)
}

/// A default for a settings command.
#[derive(Clone, Copy)]
pub(crate) enum Def {
    B(bool),
    I(i64),
    F(f64),
    S(&'static str),
}

impl Def {
    fn json(self) -> Value {
        match self {
            Def::B(v) => json!(v),
            Def::I(v) => json!(v),
            Def::F(v) => json!(v),
            Def::S(v) => json!(v),
        }
    }
}

type Setting = (&'static str, &'static str, ArgType, Def);

/// A settings command: each flag of the list holds a value; -q reads it back, falling on the default.
pub(crate) fn setting_store(c: &mut Commands<Session>, name: &'static str, doc: &'static str, list: &'static [Setting], field: fn(&mut Session) -> &mut BTreeMap<String, Value>) {
    let mut cmd = q_flags(Cmd::new(name, doc, move |s: &mut Session, a: &Args| {
        let map = field(s);
        if a.bool("query") {
            let Some(q) = q_one(a).or_else(|| list.iter().find(|x| a.has(x.0)).map(|x| x.0.to_string())) else { fail!("{name}: query what?") };
            let Some(item) = list.iter().find(|x| x.0 == q) else { fail!("{name}: can't query {q}") };
            return Ok(map.get(&q).cloned().unwrap_or(item.3.json()));
        }
        for item in list {
            if let Some(v) = a.get(item.0) {
                map.insert(item.0.to_string(), v.clone());
            }
        }
        Ok(Value::Null)
    }))
    .undoable(false);
    for &(long, short, ty, _) in list {
        cmd = cmd.flag(f(long, short, ty, "A setting."));
    }
    c.register(cmd);
}

const DISPLAY_PREF: &[Setting] = &[
    ("activeObjectPivots", "aop", B, Def::B(false)),
    ("defaultFontSize", "dfs", I, Def::I(9)),
    ("displayAffected", "da", B, Def::B(true)),
    ("displayGradient", "dgr", B, Def::B(true)),
    ("fontSettingMode", "fm", I, Def::I(0)),
    ("lineWidth", "lw", F, Def::F(1.0)),
    ("materialLoadingMode", "mld", S, Def::S("immediate")),
    ("maxHardwareTextureResolution", "mhl", B, Def::B(false)),
    ("maxTextureResolution", "mtr", I, Def::I(4096)),
    ("purgeExistingTextures", "pet", B, Def::B(false)),
    ("regionOfEffect", "roe", B, Def::B(false)),
    ("shadeTemplates", "st", B, Def::B(false)),
    ("smallFontSize", "sfs", I, Def::I(9)),
    ("textureDrawPixel", "tdp", B, Def::B(false)),
    ("wireframeOnShadedActive", "wsa", S, Def::S("full")),
    ("wireframeOnBackgroundFaces", "wbf", B, Def::B(false)),
    ("workspaceMode", "wsm", S, Def::S("")),
    ("xrayActiveComponents", "xac", B, Def::B(false)),
];

const DISPLAY_STATS: &[Setting] = &[
    ("frameRate", "fr", B, Def::B(false)),
    ("headsUpDisplay", "hud", B, Def::B(false)),
    ("polyCount", "pc", B, Def::B(false)),
    ("cameraName", "cn", B, Def::B(true)),
    ("viewAxis", "va", B, Def::B(true)),
    ("selectionDetails", "sd", B, Def::B(false)),
];

/// (flag, short, the reference's default priority.)
const SELECT_PRIORITY: &[(&str, &str, i64)] = &[
    ("allComponents", "alc", 1), ("allObjects", "alo", 1), ("animBreakdown", "abd", 3), ("animCurve", "ac", 3), ("animInTangent", "ait", 3), ("animKeyframe", "ak", 3),
    ("animOutTangent", "aot", 3), ("byName", "bn", 2), ("camera", "ca", 1), ("cluster", "cl", 1), ("collisionModel", "clm", 1), ("controlVertex", "cv", 5),
    ("curve", "c", 1), ("curveKnot", "ck", 5), ("curveOnSurface", "cos", 1), ("curveParameterPoint", "cpp", 5), ("dimension", "dim", 1), ("dynamicConstraint", "dc", 1),
    ("edge", "eg", 5), ("editPoint", "ep", 5), ("emitter", "em", 1), ("facet", "fc", 5), ("field", "fi", 1), ("flexor", "fl", 1), ("hull", "hl", 5),
    ("ikEndEffector", "iee", 2), ("ikHandle", "ikh", 2), ("imagePlane", "ip", 1), ("implicitGeometry", "ig", 1), ("isoparm", "iso", 5), ("joint", "j", 3),
    ("jointPivot", "jp", 3), ("lattice", "la", 1), ("latticePoint", "lp", 5), ("light", "lt", 1), ("localRotationAxis", "ra", 3), ("locator", "lc", 1),
    ("locatorUV", "luv", 1), ("locatorXYZ", "lxyz", 1), ("nCloth", "ncl", 1), ("nParticle", "npr", 1), ("nRigid", "nr", 1), ("nurbsCurve", "nc", 1), ("nurbsSurface", "ns", 1),
    ("orientationLocator", "ol", 1), ("particle", "pr", 1), ("plane", "pl", 1), ("polymesh", "p", 1), ("polymeshEdge", "pe", 5), ("polymeshFace", "pf", 5),
    ("polymeshFreeEdge", "pfe", 5), ("polymeshUV", "puv", 5), ("polymeshVertex", "pv", 5), ("polymeshVtxFace", "pvf", 5), ("queueTangent", "qt", 3), ("rigidBody", "rb", 1),
    ("rigidConstraint", "rc", 1), ("rotatePivot", "rp", 3), ("scalePivot", "sp", 3), ("sculpt", "sc", 1), ("selectHandle", "sh", 2), ("spring", "spr", 1), ("springComponent", "spc", 5),
    ("stroke", "str", 1), ("subdiv", "sd", 1), ("surfaceEdge", "se", 5), ("surfaceFace", "sf", 5), ("surfaceKnot", "skn", 5), ("surfaceParameterPoint", "spp", 5),
    ("surfaceRange", "srn", 5), ("surfaceUV", "suv", 5), ("texture", "tx", 1), ("vertex", "v", 5),
];

fn select_priority(s: &mut Session, a: &Args) -> CmdResult {
    if a.bool("query") {
        let Some(q) = q_one(a) else { fail!("selectPriority: query what?") };
        let Some(e) = SELECT_PRIORITY.iter().find(|x| x.0 == q) else { fail!("selectPriority: no {q}") };
        return Ok(json!(s.extras.select_priority.get(&q).copied().unwrap_or(e.2)));
    }
    for e in SELECT_PRIORITY {
        if a.has(e.0) {
            s.extras.select_priority.insert(e.0.to_string(), a.int(e.0, e.2));
        }
    }
    Ok(Value::Null)
}

// ---------------------------------------------------------------------------------------------------------------------
// Asking about nodes and attributes

fn attribute_name(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let Some(text) = a.pos_str(0) else { fail!("attributeName: name a plug") };
    let p = g.plug(text)?;
    let def = g.attr(&p);
    let comp = (p.component >= 0 && p.component < 3).then_some(p.component as usize);
    let long = comp.map_or(def.name.clone(), |c| def.child_names[c].clone());
    let short = comp.map_or(def.short_name.clone(), |c| def.child_short[c].clone());
    if a.bool("long") {
        return Ok(json!(long));
    }
    if a.bool("short") {
        return Ok(json!(if short.is_empty() { long } else { short }));
    }
    // A compound's child reads "Translate X" whole; -leaf leaves the parent's part out.
    if let (Some(c), true) = (comp, a.bool("leaf")) {
        let child = &def.child_names[c];
        return Ok(json!(crate::ref_attrs::nice(child.strip_prefix(def.name.as_str()).unwrap_or(child))));
    }
    Ok(json!(if !def.nice_name.is_empty() && comp.is_none() { def.nice_name.clone() } else { crate::ref_attrs::nice(&long) }))
}

fn attribute_info(s: &mut Session, a: &Args) -> CmdResult {
    let t = if let Some(ty) = a.get("type").and_then(Value::as_str) {
        let Some(t) = crate::node::types::find(ty) else { fail!("attributeInfo: no node type {ty}") };
        t
    } else {
        let names = objects(s, a);
        let g = &s.graph;
        let Some(n) = names.first() else { fail!("attributeInfo: name an object or -type") };
        let Some(id) = g.find(n)? else { fail!("attributeInfo: no object {n}") };
        g.node(id).unwrap().type_arc().clone()
    };
    let and = a.bool("logicalAnd");
    let want = |at: &crate::plug::AttrDef| -> bool {
        let list: [(&str, bool); 7] = [
            ("bool", at.ty == PlugType::Bool),
            ("enumerated", at.ty == PlugType::Enum),
            ("hidden", at.has(flags::HIDDEN)),
            ("inherited", t.base.as_ref().is_some_and(|b| b.attr_index(&at.name).is_some())),
            ("leaf", at.ty != PlugType::Vec3),
            ("multi", at.is_array()),
            ("writable", at.has(flags::INPUT)),
        ];
        let on: Vec<bool> = list.iter().filter(|(flag, _)| a.has(flag)).map(|(flag, has)| a.bool(flag) == *has).collect();
        if on.is_empty() {
            return true;
        }
        if and {
            on.iter().all(|x| *x)
        } else {
            on.iter().any(|x| *x)
        }
    };
    let short = a.bool("short");
    let out: Vec<Value> = t.attrs.iter().filter(|x| want(x)).map(|x| json!(if short && !x.short_name.is_empty() { &x.short_name } else { &x.name })).collect();
    Ok(Value::Array(out))
}

fn same_transform(g: &crate::graph::Graph, a: NodeId, b: NodeId, root: bool) -> bool {
    let local = |n: NodeId| g.node(n).filter(|x| x.ty().is_a("transform")).map(|x| x.matrix(Transform::MATRIX));
    let close = match (local(a), local(b)) {
        (Some(p), Some(q)) => p.abs_diff_eq(q, 1e-6),
        _ => false,
    };
    if !close {
        return false;
    }
    if root {
        return true;
    }
    let kids = |n: NodeId| -> Vec<NodeId> { g.children(n).iter().copied().filter(|&c| crate::dag::is_transform(g, c)).collect() };
    let (ka, kb) = (kids(a), kids(b));
    ka.len() == kb.len() && ka.iter().zip(&kb).all(|(&p, &q)| same_transform(g, p, q, false))
}

fn transform_compare(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    g.evaluate();
    let [x, y] = names.as_slice() else { fail!("transformCompare: name or select two transforms") };
    let (Some(x), Some(y)) = (g.find(x)?, g.find(y)?) else { fail!("transformCompare: no such object") };
    Ok(json!(if same_transform(g, x, y, a.bool("root")) { 0 } else { 1 }))
}

fn lock_node(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let mut ids = Vec::new();
    for n in &names {
        let Some(id) = g.find(n)? else { fail!("lockNode: no object {n}") };
        ids.push(id);
    }
    if a.bool("query") {
        return Ok(Value::Array(ids.iter().map(|&i| json!(g.node(i).unwrap().is_locked())).collect()));
    }
    let on = a.get("lock").map_or(true, on_off);
    g.open_chunk("lockNode");
    for &i in &ids {
        g.set_node_locked(i, on);
    }
    g.close_chunk();
    Ok(Value::Null)
}

fn is_true(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.pos_str(0) else { fail!("isTrue: name a condition") };
    let g = &s.graph;
    Ok(json!(match name {
        "SomethingSelected" => !g.selection().is_empty(),
        "MayaInitialized" | "opengl" => true,
        "BatchMode" | "playingBack" | "playingBackAuto" | "delete" | "dynamicsIsRunning" | "readingFile" | "writingFile" => false,
        "autoKeyframeState" | "autoKeyframe" => crate::anim::auto_key(),
        _ => fail!("isTrue: no condition {name}"),
    }))
}

fn encode_string(a: &Args) -> CmdResult {
    let Some(t) = a.pos_str(0) else { fail!("encodeString: give a string") };
    let mut out = String::with_capacity(t.len());
    for c in t.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c if (c as u32) < 32 || c as u32 == 127 => out.push_str(&format!("\\{:03o}", c as u32)),
            c => out.push(c),
        }
    }
    Ok(json!(out))
}

fn find_type(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &s.graph;
    let ty = a.string("type", "");
    if ty.is_empty() {
        fail!("findType: give -type");
    }
    let fits = |id: NodeId| g.node(id).is_some_and(|n| if a.bool("exact") { n.ty().name == ty } else { n.ty().is_a(&ty) });
    let conns = g.connections();
    let (forward, deep) = (a.bool("forward"), a.bool("deep"));
    let mut out = Vec::new();
    for name in names {
        let Some(id) = g.find(&name)? else { continue };
        let mut seen: HashSet<NodeId> = HashSet::from([id]);
        let mut front = std::collections::VecDeque::from([id]);
        'walk: while let Some(n) = front.pop_front() {
            let near: Vec<NodeId> = conns.iter().filter_map(|(d, s)| if forward { (s.node == n).then_some(d.node) } else { (d.node == n).then_some(s.node) }).collect();
            for x in near {
                if !seen.insert(x) {
                    continue;
                }
                if fits(x) {
                    out.push(json!(g.path(x)));
                    if !deep {
                        break 'walk;
                    }
                }
                front.push_back(x);
            }
        }
    }
    Ok(Value::Array(out))
}

fn object_center(s: &mut Session, a: &Args) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    g.evaluate();
    let Some(name) = names.first() else { fail!("objectCenter: name or select an object") };
    let Some(id) = g.find(name)? else { fail!("objectCenter: no object {name}") };
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    let mut under = g.descendants(id);
    under.push(id);
    for u in under {
        if !crate::dag::is_shape(g, u) || g.node(u).is_some_and(|n| n.bool(crate::transform::DagNode::INTERMEDIATE_OBJECT)) {
            continue;
        }
        let b = crate::node_bounds::world_bounds(g, u);
        if b.valid() {
            lo = lo.min(b.min);
            hi = hi.max(b.max);
        }
    }
    let world = g.node(id).map_or(glam::DMat4::IDENTITY, |n| if n.ty().dag { n.matrix(Transform::WORLD_MATRIX) } else { glam::DMat4::IDENTITY });
    let mut c = if lo.x <= hi.x { (lo + hi) * 0.5 } else { world.w_axis.truncate() };
    if a.bool("local") {
        c = world.inverse().transform_point3(c);
    }
    Ok(if a.bool("x") {
        json!(c.x)
    } else if a.bool("y") {
        json!(c.y)
    } else if a.bool("z") {
        json!(c.z)
    } else {
        json!([c.x, c.y, c.z])
    })
}

fn list_node_types(a: &Args) -> CmdResult {
    let Some(class) = a.pos_str(0) else { fail!("listNodeTypes: give a classification") };
    let exclude = a.get("exclude").and_then(Value::as_str).map(str::to_string);
    let mut out = Vec::new();
    for t in crate::node::types::all() {
        let Some(c) = crate::utility_nodes::classification(&t.name) else { continue };
        let ok = c == class || c.starts_with(&format!("{class}/")) || c.starts_with(&format!("{class}:")) || (class.contains('*') && crate::ref_core::glob(class, c));
        let no = exclude.as_ref().is_some_and(|e| c == e || c.starts_with(&format!("{e}/")));
        if ok && !no {
            out.push(t.name.clone());
        }
    }
    out.sort();
    Ok(json!(out))
}

fn dg_info(s: &mut Session, a: &Args) -> CmdResult {
    let g = &s.graph;
    let names = a.named();
    let ids: Vec<NodeId> = if names.is_empty() || a.bool("allNodes") { g.nodes() } else { names.iter().filter_map(|n| g.find(n).ok().flatten()).collect() };
    let ty = a.get("type").and_then(Value::as_str).map(str::to_string);
    let mut out: Vec<String> = Vec::new();
    if a.bool("connections") {
        for (d, sr) in g.connections() {
            if ids.contains(&d.node) || ids.contains(&sr.node) {
                out.push(format!("{} -> {}", g.plug_name(&sr), g.plug_name(&d)));
            }
        }
    } else {
        for id in ids {
            let n = g.node(id).unwrap();
            if ty.as_ref().is_some_and(|t| !n.ty().is_a(t)) || (a.has("dirty") && a.bool("dirty") != n.is_dirty()) || (a.bool("nonDeletable") && !n.is_locked()) {
                continue;
            }
            out.push(g.path(id));
        }
    }
    if let Some(path) = a.get("outputFile").and_then(Value::as_str) {
        std::fs::write(path, out.join("\n")).map_err(|e| aztek_ops::CommandError::new(format!("dgInfo: {path}: {e}")))?;
    }
    Ok(json!(out))
}

/// getModulePath -mn: where a module's .mod file says it lives, searched along AZTEK_MODULE_PATH and MAYA_MODULE_PATH.
fn module_path(a: &Args) -> CmdResult {
    let name = a.string("moduleName", "");
    if name.is_empty() {
        fail!("getModulePath: give -moduleName");
    }
    for var in ["AZTEK_MODULE_PATH", "MAYA_MODULE_PATH"] {
        let Ok(list) = std::env::var(var) else { continue };
        for dir in std::env::split_paths(&list) {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                if e.path().extension().and_then(|x| x.to_str()) != Some("mod") {
                    continue;
                }
                let Ok(t) = std::fs::read_to_string(e.path()) else { continue };
                for line in t.lines() {
                    let parts: Vec<&str> = line.split_whitespace().collect();
                    if parts.len() >= 4 && parts[0] == "+" && parts[1] == name {
                        let p = parts[parts.len() - 1];
                        return Ok(json!(if p.starts_with('.') { dir.join(p).to_string_lossy().to_string() } else { p.to_string() }));
                    }
                }
            }
        }
    }
    fail!("getModulePath: no module named {name}")
}

/// closestPointOnMesh and nearestPointOnMesh: a node on the mesh, answering in world space.
fn closest_cmd(s: &mut Session, a: &Args, ty: &str) -> CmdResult {
    let names = objects(s, a);
    let g = &mut s.graph;
    let Some(name) = names.first() else { fail!("{ty}: name or select a mesh") };
    let Some(id) = g.find(name)? else { fail!("{ty}: no object {name}") };
    let Some(mesh) = crate::components::as_mesh(g, id) else { fail!("{ty}: {name} is not a mesh") };
    let pos = a.vec3("inPosition").map_or(DVec3::ZERO, DVec3::from_array);
    let t = crate::node::types::find(ty).or_else(|| crate::node::types::find("closestPointOnMesh")).expect("closestPointOnMesh");
    g.open_chunk(ty);
    let r = (|| -> CmdResult<NodeId> {
        let n = g.create_node(&t, &a.string("name", &format!("{ty}1")), crate::plug::NO_NODE, true)?;
        let at = |k: &str| PlugRef::new(n, t.attr_index(k).unwrap() as i32);
        g.set_value(at("inPosition"), PlugValue::Vec3(pos))?;
        g.connect(&PlugRef::new(mesh, crate::mesh_nodes::MeshNode::OUT_MESH as i32), &at("inMesh"), false)?;
        g.connect(&PlugRef::new(mesh, Transform::WORLD_MATRIX as i32), &at("inputMatrix"), false)?;
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    g.evaluate();
    Ok(json!(g.path(n)))
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        q_flags(Cmd::new("upAxis", "The scene's up axis: -ax y or z, -rv moves the default cameras with it (on by default). -q -ax asks.", up_axis_cmd))
            .flag(f("axis", "ax", ArgType::Any, "y or z."))
            .flag(f("rotateView", "rv", B, "Turn the default cameras too."))
            .undoable(false),
    );
    c.register(
        Cmd::new("convertUnit", "Convert a value between units: convertUnit -f inch -t cm \"1\". Returns the value with its unit.", convert_unit)
            .positional("value")
            .flag(f("fromUnit", "f", S, "The unit of the value (centimeters, or as the value spells it)."))
            .flag(f("toUnit", "t", S, "The unit wanted."))
            .undoable(false),
    );
    c.register(
        Cmd::new("copyAttr", "Copy attributes between nodes: -v values, -ic and -oc move connections (-ksc leaves the source's), -at which attributes.", copy_attr)
            .positional("source target")
            .flag(f("attribute", "at", ArgType::Any, "Attributes to copy (all that both nodes have)."))
            .flag(f("values", "v", B, "Copy values."))
            .flag(f("inConnections", "ic", B, "Move incoming connections."))
            .flag(f("outConnections", "oc", B, "Move outgoing connections."))
            .flag(f("keepSourceConnections", "ksc", B, "Leave the source its connections."))
            .flag(f("containerParentChild", "cpc", B, "Accepted for existing scripts."))
            .flag(f("renameTargetContainer", "rtc", B, "Accepted for existing scripts.")),
    );
    c.register(
        q_flags(Cmd::new("cycleCheck", "Look for dependency cycles: -evaluation on|off the warning, -q asks; objects or -all: true when one is found, -l the plugs on it.", cycle_check))
            .positional("objects")
            .flag(f("evaluation", "ev", ArgType::Any, "Warn about cycles while evaluating."))
            .flag(f("all", "all", B, "Look through the whole scene."))
            .flag(f("list", "l", B, "List the plugs on a cycle."))
            .flag(f("firstCycleOnly", "fco", B, "Accepted: the first found is what's listed."))
            .flag(f("children", "c", B, "Accepted for existing scripts."))
            .flag(f("parents", "p", B, "Accepted for existing scripts."))
            .flag(f("dag", "dag", B, "Accepted for existing scripts."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("evaluationManager", "The evaluation mode, as a setting scripts can read: -mode off|serial|parallel|emp. It doesn't change how the graph evaluates.", |s, a| {
            if a.bool("query") {
                let m = s.extras.evaluation_mode.clone().unwrap_or("off".to_string());
                if asked(a, "enabled") {
                    return Ok(json!(m != "off"));
                }
                if asked(a, "manipulation") || asked(a, "invalidate") || asked(a, "idleBuild") {
                    return Ok(json!(false));
                }
                return Ok(json!([m]));
            }
            if let Some(m) = a.get("mode").and_then(Value::as_str) {
                if !["off", "serial", "serialUncached", "parallel", "emp", "dg"].contains(&m) {
                    fail!("evaluationManager: unknown mode {m}");
                }
                s.extras.evaluation_mode = Some(if m == "dg" { "off".to_string() } else { m.to_string() });
            }
            Ok(Value::Null)
        }))
        .flag(f("mode", "m", ArgType::Any, "off, serial, parallel or emp."))
        .flag(f("enabled", "en", B, "Whether a mode other than off is set."))
        .flag(f("invalidate", "inv", B, "Accepted for existing scripts."))
        .flag(f("manipulation", "man", B, "Accepted for existing scripts."))
        .flag(f("idleBuild", "ib", B, "Accepted for existing scripts."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("threadCount", "Threads to evaluate with, kept for scripts: -n count, 0 for the machine's; -q -n asks.", |s, a| {
            if a.bool("query") {
                let n = s.extras.thread_count.unwrap_or(0);
                return Ok(json!(if n > 0 { n } else { std::thread::available_parallelism().map_or(1, |x| x.get() as i64) }));
            }
            s.extras.thread_count = Some(a.int("numberOfThreads", 0));
            Ok(Value::Null)
        }))
        .flag(f("numberOfThreads", "n", I, "How many."))
        .undoable(false),
    );
    c.register(
        Cmd::new("dgInfo", "The dependency graph as text: -nd nodes, -c connections, -nt a type, -d dirty ones, -of a file.", dg_info)
            .positional("objects")
            .flag(f("allNodes", "all", B, "Every node."))
            .flag(f("nodes", "nd", B, "List nodes (what it lists anyway)."))
            .flag(f("connections", "c", B, "List connections."))
            .flag(f("dirty", "d", B, "Only nodes in this dirty state."))
            .flag(f("nonDeletable", "nod", B, "Only locked nodes."))
            .flag(f("outputFile", "of", S, "Write to this file."))
            .flag(f("propagation", "p", B, "Accepted for existing scripts."))
            .flag(f("short", "s", B, "Accepted for existing scripts."))
            .flag(f("subgraph", "sub", B, "Accepted for existing scripts."))
            .flag(f("type", "nt", S, "Only this type."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("dgtimer", "A stub: nothing is timed here. -on, -off, -reset are accepted, every result is 0.", |s, a| {
            if a.has("timerOn") {
                s.extras.dg_timer_on = a.get("timerOn").is_some_and(on_off);
            }
            if a.bool("timerOff") {
                s.extras.dg_timer_on = false;
            }
            if a.bool("query") && asked(a, "timerOn") {
                return Ok(json!(s.extras.dg_timer_on));
            }
            Ok(json!(0.0))
        }))
        .flag(f("timerOn", "on", ArgType::Any, "Turn the timer on or off."))
        .flag(f("timerOff", "off", B, "Accepted."))
        .flag(f("reset", "r", B, "Accepted."))
        .flag(f("outputFile", "o", S, "Accepted."))
        .flag(f("maxDisplay", "m", I, "Accepted."))
        .flag(f("sortMetric", "sm", S, "Accepted."))
        .flag(f("sortType", "st", S, "Accepted."))
        .flag(f("threshold", "th", F, "Accepted."))
        .flag(f("timerType", "tt", S, "Accepted."))
        .flag(f("uniqueName", "un", B, "Accepted."))
        .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("displayColor", "A display colour's index: name index, -a active or dormant, -q asks, -l lists, -c creates, -rf resets.", display_color))
            .positional("name [index]")
            .flag(f("active", "a", B, "The active colour."))
            .flag(f("dormant", "d", B, "The dormant colour (the default)."))
            .flag(f("create", "c", B, "Create the name."))
            .flag(f("list", "l", B, "List the names."))
            .flag(f("queryIndex", "qi", I, "Accepted for existing scripts."))
            .flag(f("resetToFactory", "rf", B, "Back to the factory colours."))
            .flag(f("resetToSaved", "rs", B, "Back to the saved colours."))
            .flag(f("userDefined", "ud", I, "Accepted for existing scripts."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("displayRGBColor", "A display colour's RGB: name r g b [a], -q asks, -l lists, -c creates.", display_rgb))
            .positional("name r g b [a]")
            .flag(f("alpha", "a", B, "With alpha."))
            .flag(f("create", "c", B, "Create the name."))
            .flag(f("hueSaturationValue", "hsv", B, "Accepted for existing scripts."))
            .flag(f("list", "l", B, "List the names."))
            .flag(f("resetToFactory", "rf", B, "Back to the factory colours."))
            .flag(f("resetToSaved", "rs", B, "Back to the saved colours."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("colorIndex", "A palette entry's colour: index r g b, 1 to 31, -q asks.", color_index))
            .positional("index [r g b]")
            .flag(f("hueSaturationValue", "hsv", B, "Accepted for existing scripts."))
            .flag(f("resetToFactory", "rf", B, "Back to the factory colours."))
            .flag(f("resetToSaved", "rs", B, "Back to the saved colours."))
            .undoable(false),
    );
    c.register(
        q_flags(Cmd::new("displayLevelOfDetail", "Level of detail display, as a setting scripts can read: -lod on|off.", |s, a| {
            if a.bool("query") {
                return Ok(json!(s.extras.level_of_detail));
            }
            if let Some(v) = a.positional.first().or(a.get("levelOfDetail")) {
                s.extras.level_of_detail = on_off(v);
            }
            Ok(Value::Null)
        }))
        .positional("on or off")
        .flag(f("levelOfDetail", "lod", ArgType::Any, "On or off."))
        .undoable(false),
    );
    setting_store(c, "displayPref", "Viewport display preferences, kept for scripts: each flag sets, -q reads.", DISPLAY_PREF, |s| &mut s.extras.display_pref);
    setting_store(c, "displayStats", "Which statistics a viewport shows, kept for scripts: each flag sets, -q reads.", DISPLAY_STATS, |s| &mut s.extras.display_stats);
    c.register(
        q_flags(Cmd::new("displaySurface", "How a surface draws, kept for scripts: -xr X-ray, -tsl two-sided lighting, -fn flip normals. On named or selected objects; -q reads the first.", |s, a| {
            let names = objects(s, a);
            let ids: Vec<NodeId> = names.iter().filter_map(|n| s.graph.find(n).ok().flatten()).collect();
            if a.bool("query") {
                let Some(q) = q_one(a) else { fail!("displaySurface: query what?") };
                let Some(&id) = ids.first() else { fail!("displaySurface: name an object") };
                let def = q == "twoSidedLighting";
                return Ok(json!([s.extras.display_surface.get(&(id, q)).copied().unwrap_or(def)]));
            }
            for flag in ["xRay", "twoSidedLighting", "flipNormals"] {
                if a.has(flag) {
                    let on = a.bool(flag);
                    for &id in &ids {
                        s.extras.display_surface.insert((id, flag.to_string()), on);
                    }
                }
            }
            Ok(Value::Null)
        }))
        .positional("objects")
        .flag(f("xRay", "xr", B, "Draw see-through."))
        .flag(f("twoSidedLighting", "tsl", B, "Light both sides (on by default)."))
        .flag(f("flipNormals", "fn", B, "Flip the normals drawn."))
        .undoable(false),
    );
    c.register(
        Cmd::new("listNodeTypes", "Node types of a classification: listNodeTypes \"utility/general\", -ex leaves one out.", |_s, a| list_node_types(a))
            .positional("classification")
            .flag(f("exclude", "ex", S, "A classification to leave out."))
            .undoable(false),
    );
    let mut sp = q_flags(Cmd::new("selectPriority", "Selection priorities by component and object type: -cv 5, -q -cv. Kept for scripts.", select_priority)).undoable(false);
    for e in SELECT_PRIORITY {
        sp = sp.flag(f(e.0, e.1, I, "The priority."));
    }
    c.register(sp);
    c.register(
        Cmd::new("selectedNodes", "The selection as full paths: -do leaves out what isn't in the DAG.", |s, a| {
            let g = &s.graph;
            let mut out = Vec::new();
            for &id in g.selection() {
                let n = g.node(id).unwrap();
                if a.bool("dagObjects") && !n.ty().dag {
                    continue;
                }
                out.push(g.full_path(id));
            }
            Ok(json!(out))
        })
        .flag(f("dagObjects", "do", B, "Only DAG objects."))
        .undoable(false),
    );
    c.register(
        Cmd::new("findType", "Nodes of a type upstream of the objects (-fwd downstream), the nearest unless -d. -e exact type.", find_type)
            .positional("objects")
            .flag(f("type", "t", S, "The type."))
            .flag(f("deep", "d", B, "Every one, not just the nearest."))
            .flag(f("exact", "e", B, "This type exactly."))
            .flag(f("forward", "fwd", B, "Look downstream."))
            .undoable(false),
    );
    c.register(
        Cmd::new("objectCenter", "An object's bounding box centre: world (-gl, the default) or -l its own space; -x -y -z one value.", object_center)
            .positional("object")
            .flag(f("gl", "gl", B, "World space (the default)."))
            .flag(f("local", "l", B, "The object's space."))
            .flag(f("x", "x", B, "X alone."))
            .flag(f("y", "y", B, "Y alone."))
            .flag(f("z", "z", B, "Z alone."))
            .undoable(false),
    );
    for name in ["closestPointOnMesh", "nearestPointOnMesh"] {
        c.register(
            Cmd::new(name, "Make a node on a mesh that answers the point on it closest to -ip. Returns the node.", move |s, a| closest_cmd(s, a, name))
                .positional("mesh")
                .flag(f("inPosition", "ip", ArgType::Vec3, "The position."))
                .flag(f("name", "n", S, "The node's name.")),
        );
    }
    c.register(Cmd::new("hitTest", "A stub outside the UI: returns nothing, since panels and their cameras live there.", |_s, _a| Ok(json!([]))).positional("control x y").undoable(false));
    c.register(
        Cmd::new("isDirty", "Whether a plug's node needs evaluating: -c through connections, -d the datablock.", |s, a| {
            let g = &s.graph;
            let Some(t) = a.pos_str(0) else { fail!("isDirty: name a plug") };
            let p = g.plug(t)?;
            Ok(json!(g.node(p.node).is_some_and(|n| n.is_dirty())))
        })
        .positional("plug")
        .flag(f("connection", "c", B, "Accepted: dirt is per node here."))
        .flag(f("datablock", "d", B, "Accepted: dirt is per node here."))
        .undoable(false),
    );
    c.register(Cmd::new("isTrue", "Is a named condition true: SomethingSelected, playingBack, autoKeyframeState, ...", is_true).positional("condition").undoable(false));
    c.register(Cmd::new("getModulePath", "Where a module lives: -mn name, along AZTEK_MODULE_PATH and MAYA_MODULE_PATH.", |_s, a| module_path(a)).flag(f("moduleName", "mn", S, "The module.")).undoable(false));
    c.register(Cmd::new("encodeString", "A string with quotes, backslashes and control characters escaped, as MEL would need them.", |_s, a| encode_string(a)).positional("string").undoable(false));
    c.register(
        Cmd::new("attributeName", "An attribute's name: -n nice (the default), -l long, -s short, -lf the leaf.", attribute_name)
            .positional("plug")
            .flag(f("nice", "n", B, "The name editors show."))
            .flag(f("long", "l", B, "The long name."))
            .flag(f("short", "s", B, "The short name."))
            .flag(f("leaf", "lf", B, "Without its parent's name."))
            .undoable(false),
    );
    c.register(
        Cmd::new("attributeInfo", "A node's attributes filtered: -b boolean, -e enumerated, -h hidden, -w writable, -m multi, -lf leaf, -inh inherited; -t a type in place of a node; -s short names.", attribute_info)
            .positional("object")
            .flag(f("allAttributes", "all", B, "All (what no filter gives)."))
            .flag(f("bool", "b", B, "Booleans."))
            .flag(f("enumerated", "e", B, "Enums."))
            .flag(f("hidden", "h", B, "Hidden ones."))
            .flag(f("inherited", "inh", B, "Inherited from the base type."))
            .flag(f("internal", "i", B, "Internal ones: none here."))
            .flag(f("leaf", "lf", B, "Not compounds."))
            .flag(f("logicalAnd", "la", B, "All filters must hold, not any."))
            .flag(f("multi", "m", B, "Arrays."))
            .flag(f("short", "s", B, "Short names."))
            .flag(f("type", "t", S, "A type rather than a node."))
            .flag(f("userInterface", "ui", B, "Accepted for existing scripts."))
            .flag(f("writable", "w", B, "Writable ones."))
            .undoable(false),
    );
    c.register(Cmd::new("transformCompare", "0 when two transforms match, with what's under them, 1 otherwise. -r the root alone.", transform_compare).positional("two transforms").flag(f("root", "r", B, "Only the root.")).undoable(false));
    c.register(
        q_flags(Cmd::new("lockNode", "Lock nodes against deleting and renaming: -l on or off (on with no flag); -q -l reads each.", lock_node))
            .positional("objects")
            .flag(f("lock", "l", ArgType::Any, "On or off."))
            .flag(f("lockName", "ln", B, "Accepted: a locked node keeps its name anyway."))
            .flag(f("lockUnpublished", "lu", B, "Accepted for existing scripts."))
            .flag(f("ignoreComponents", "ic", B, "Accepted for existing scripts.")),
    );
}
