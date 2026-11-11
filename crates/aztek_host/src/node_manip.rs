//! The Show Manipulator tool: handles in the view that write a node's own attributes. Which node, what handles it has
//! and what a drag does are worked out here with no GPU, so tests drive them with a Core and a Viewport. A drag previews
//! with silent setAttrs and runs one on release, so the history shows one line and undo takes one step.

use aztek_scene::components::{self, ComponentMode};
use aztek_scene::plug::{NodeId, PlugRef, PlugValue};
use aztek_scene::session::Session;
use glam::{DMat3, DMat4, DVec2, DVec3};
use serde_json::{json, Value};

use crate::core::Core;
use crate::viewport::{gather, Viewport};

const RED: [u8; 4] = [230, 40, 40, 255];
const GREEN: [u8; 4] = [40, 230, 40, 255];
const BLUE: [u8; 4] = [40, 80, 255, 255];
const CYAN: [u8; 4] = [90, 200, 230, 255];
const ACTIVE: [u8; 4] = [255, 255, 0, 255];
const AXES: [[u8; 4]; 3] = [RED, GREEN, BLUE];

/// What a handle looks like, in world space.
#[derive(Clone, Debug)]
pub enum Form {
    Arrow { from: DVec3, to: DVec3 },
    Cube { from: DVec3, to: DVec3 },
    Ring { centre: DVec3, axis: DVec3, radius: f64 },
    Dot { at: DVec3 },
}

/// What dragging a handle writes.
#[derive(Clone, Debug)]
pub enum Drive {
    /// start + factor * the world distance dragged along the axis.
    Along { axis: DVec3, start: f64, factor: f64, min: Option<f64>, max: Option<f64> },
    /// start * (1 + the distance dragged along the axis in handle lengths).
    Scale { axis: DVec3, start: f64, len: f64 },
    /// start + factor * the degrees swept round the axis.
    Turn { centre: DVec3, axis: DVec3, start: f64, factor: f64 },
    /// Switches the handles between local and world.
    Space,
}

#[derive(Clone, Debug)]
pub struct Handle {
    /// The attribute it writes, without the node.
    pub attr: String,
    pub form: Form,
    pub color: [u8; 4],
    pub drive: Drive,
}

#[derive(Clone, Debug, Default)]
pub struct Manip {
    pub node: String,
    pub handles: Vec<Handle>,
    /// Lines that are not handles: a falloff's outline, a mirror plane.
    pub lines: Vec<(DVec3, DVec3, [u8; 4])>,
}

struct Grab {
    handle: Handle,
    node: String,
    start: DVec2,
    applied: bool,
    last: Option<f64>,
}

/// The tool's state, one for all views.
#[derive(Default)]
pub struct State {
    /// The node asked for, and the selection it was asked for under: a new selection forgets it.
    target: Option<(String, Value)>,
    /// The extrude family's handles in world rather than local axes.
    pub world: bool,
    /// The channel highlighted: a middle drag anywhere takes its handle.
    pub active_attr: Option<String>,
    hover: Option<String>,
    grab: Option<Grab>,
}

fn get(s: &mut Session, node: &str, attr: &str) -> f64 {
    match s.run("getAttr", json!({"args": [format!("{node}.{attr}")]})) {
        Ok(Value::Bool(b)) => b as i32 as f64,
        Ok(v) => v.as_f64().or_else(|| v.get(0).and_then(Value::as_f64)).unwrap_or(0.0),
        Err(_) => 0.0,
    }
}

fn type_of(s: &Session, id: NodeId) -> String {
    s.graph.node(id).map(|n| n.ty().name.to_string()).unwrap_or_default()
}

/// Node types that take the extrude family's combined handles.
fn is_move_family(ty: &str) -> bool {
    matches!(ty, "polyExtrudeFace" | "polyExtrudeEdge" | "polyChipOff" | "polyMoveFace" | "polyMoveEdge" | "polyMoveVertex")
}

fn is_primitive(ty: &str) -> bool {
    matches!(ty, "polyCube" | "polySphere" | "polyCylinder" | "polyCone" | "polyTorus" | "polyPlane")
}

fn is_nonlinear(ty: &str) -> bool {
    matches!(ty, "bend" | "flare" | "sine" | "squash" | "twist" | "wave")
}

fn is_projection(ty: &str) -> bool {
    matches!(ty, "polyPlanarProj" | "polyCylProj" | "polySphProj")
}

/// Whether a node has handles or fields of its own, so a history walk stops at it.
pub fn has_manip(ty: &str) -> bool {
    is_move_family(ty) || is_primitive(ty) || is_nonlinear(ty) || is_projection(ty) || matches!(ty, "polyPoke" | "polyMirror" | "polyWedgeFace" | "polyExtrudeVertex" | "softMod" | "sculpt") || !fields_of(ty).is_empty()
}

/// The in-view editor's rows for a node type, in order: (attribute, label, kind, min, max). Kind is f, i or b.
pub fn fields_of(ty: &str) -> &'static [(&'static str, &'static str, char, f64, f64)] {
    match ty {
        "polyExtrudeFace" => &[("thickness", "Thickness", 'f', -10.0, 10.0), ("offset", "Offset", 'f', -10.0, 10.0), ("divisions", "Divisions", 'i', 1.0, 20.0), ("keepFacesTogether", "Keep Faces Together", 'b', 0.0, 1.0)],
        "polyExtrudeEdge" => &[("thickness", "Thickness", 'f', -10.0, 10.0), ("offset", "Offset", 'f', -10.0, 10.0), ("divisions", "Divisions", 'i', 1.0, 20.0)],
        "polyExtrudeVertex" => &[("length", "Length", 'f', 0.0, 10.0), ("width", "Width", 'f', 0.0, 1.0), ("divisions", "Divisions", 'i', 1.0, 20.0)],
        "polyBevel3" => &[("fraction", "Fraction", 'f', 0.0, 1.0), ("segments", "Segments", 'i', 1.0, 20.0), ("depth", "Depth", 'f', -1.0, 1.0)],
        "polyBevelFace" => &[("offset", "Offset", 'f', 0.0, 10.0), ("segments", "Segments", 'i', 1.0, 20.0)],
        "bend" => &[("curvature", "Curvature", 'f', -3.0, 3.0), ("lowBound", "Low Bound", 'f', -10.0, 0.0), ("highBound", "High Bound", 'f', 0.0, 10.0)],
        "twist" => &[("startAngle", "Start Angle", 'f', -360.0, 360.0), ("endAngle", "End Angle", 'f', -360.0, 360.0), ("lowBound", "Low Bound", 'f', -10.0, 0.0), ("highBound", "High Bound", 'f', 0.0, 10.0)],
        "squash" => &[("factor", "Factor", 'f', -2.0, 2.0), ("expand", "Expand", 'f', 0.0, 5.0), ("lowBound", "Low Bound", 'f', -10.0, 0.0), ("highBound", "High Bound", 'f', 0.0, 10.0)],
        "flare" => &[("startFlareX", "Start Flare X", 'f', 0.0, 5.0), ("startFlareZ", "Start Flare Z", 'f', 0.0, 5.0), ("endFlareX", "End Flare X", 'f', 0.0, 5.0), ("endFlareZ", "End Flare Z", 'f', 0.0, 5.0), ("curve", "Curve", 'f', -5.0, 5.0)],
        "sine" => &[("amplitude", "Amplitude", 'f', -5.0, 5.0), ("wavelength", "Wavelength", 'f', 0.1, 10.0), ("offset", "Offset", 'f', -10.0, 10.0)],
        "wave" => &[("amplitude", "Amplitude", 'f', -5.0, 5.0), ("wavelength", "Wavelength", 'f', 0.1, 10.0), ("offset", "Offset", 'f', -10.0, 10.0), ("minRadius", "Min Radius", 'f', 0.0, 10.0), ("maxRadius", "Max Radius", 'f', 0.0, 10.0)],
        "softMod" => &[("falloffRadius", "Falloff Radius", 'f', 0.0, 20.0), ("envelope", "Envelope", 'f', 0.0, 1.0)],
        "sculpt" => &[("maximumDisplacement", "Maximum Displacement", 'f', 0.0, 5.0), ("dropoffDistance", "Dropoff Distance", 'f', 0.0, 10.0), ("envelope", "Envelope", 'f', 0.0, 1.0)],
        "deltaMush" | "tension" | "jiggle" | "wrap" | "wire" | "textureDeformer" | "blendShape" | "cluster" | "ffd" => &[("envelope", "Envelope", 'f', 0.0, 1.0)],
        "polyPlanarProj" => &[("projectionWidth", "Width", 'f', 0.0, 20.0), ("projectionHeight", "Height", 'f', 0.0, 20.0)],
        "polyCylProj" => &[("projectionHeight", "Height", 'f', 0.0, 20.0), ("projectionHorizontalSweep", "Horizontal Sweep", 'f', 0.0, 360.0)],
        "polySphProj" => &[("projectionHorizontalSweep", "Horizontal Sweep", 'f', 0.0, 360.0)],
        "polyBridgeEdge" => &[("divisions", "Divisions", 'i', 0.0, 20.0), ("twist", "Twist", 'f', -180.0, 180.0), ("taper", "Taper", 'f', 0.0, 2.0)],
        "polyWedgeFace" => &[("wedgeAngle", "Wedge Angle", 'f', 0.0, 360.0), ("divisions", "Divisions", 'i', 1.0, 64.0)],
        "polySplitRing" => &[("weight", "Weight", 'f', 0.0, 1.0), ("divisions", "Divisions", 'i', 1.0, 20.0)],
        "polyDuplicateEdge" => &[("offset", "Offset", 'f', 0.0, 1.0)],
        "polySmoothFace" => &[("divisions", "Divisions", 'i', 0.0, 4.0)],
        "polyReduce" => &[("percentage", "Percentage", 'f', 0.0, 100.0)],
        "polyQuad" => &[("angle", "Angle", 'f', 0.0, 180.0)],
        "polyAverageVertex" => &[("iterations", "Iterations", 'i', 1.0, 20.0)],
        "polyMirror" => &[("mirrorPosition", "Position", 'f', -100.0, 100.0), ("mergeThreshold", "Threshold", 'f', 0.0, 1.0)],
        _ => &[],
    }
}

/// The in-view editor's description of the current node, for the panel.
pub fn fields_json(core: &mut Core) -> Value {
    let Some(id) = target(core) else { return Value::Null };
    let ty = type_of(&core.s, id);
    let name = core.s.graph.path(id);
    // A bevel made with a width rather than a fraction shows its width.
    let by_width = ty == "polyBevel3" && get(&mut core.s, &name, "offsetAsFraction") == 0.0;
    let rows: Vec<Value> = fields_of(&ty)
        .iter()
        .map(|&(attr, label, kind, min, max)| {
            let (attr, label, max) = if by_width && attr == "fraction" { ("offset", "Offset", 10.0) } else { (attr, label, max) };
            let value = get(&mut core.s, &name, attr);
            json!({"plug": format!("{name}.{attr}"), "label": label, "kind": kind.to_string(), "min": min, "max": max, "value": value})
        })
        .collect();
    json!({"node": name, "type": ty, "fields": rows})
}

fn selection_now(core: &mut Core) -> Value {
    core.s.run("ls", json!({"selection": true})).unwrap_or(Value::Null)
}

/// Asks for a node's handles under the present selection.
pub fn show_node(core: &mut Core, name: &str) {
    let sel = selection_now(core);
    core.node_manip.target = Some((name.to_string(), sel));
    core.node_manip.active_attr = None;
}

/// After an edit run from a menu or hotkey: its node's handles, straight away, as the reference shows them. Returns whether it did.
pub fn after_action(core: &mut Core, result: &Value) -> bool {
    let first = result.as_str().map(str::to_string).or_else(|| result.as_array().and_then(|a| a.iter().rev().find_map(|v| v.as_str().map(str::to_string))));
    let Some(name) = first else { return false };
    let Ok(Some(id)) = core.s.graph.find(&name) else { return false };
    let ty = type_of(&core.s, id);
    // A primitive made from a menu leaves the tool alone; T brings its handles up.
    if !ty.starts_with("poly") || is_primitive(&ty) || !has_manip(&ty) {
        return false;
    }
    show_node(core, &name);
    crate::core::push_ui_request("setTool|showManip".to_string());
    true
}

/// The node the tool shows: the one asked for while the selection stands, else the newest with handles in the history of
/// what is selected, else what is selected.
pub fn target(core: &mut Core) -> Option<NodeId> {
    if core.node_manip.target.is_some() {
        let now = selection_now(core);
        if let Some((name, sel)) = &core.node_manip.target {
            if *sel == now {
                if let Ok(Some(id)) = core.s.graph.find(name) {
                    return Some(id);
                }
            }
        }
    }
    let g = &core.s.graph;
    let lead = *g.selection().last()?;
    // A deformer's handle shows the deformer's handles.
    if let Some(d) = aztek_scene::deform::driven_deformers(g, lead).into_iter().find(|&d| has_manip(&type_of(&core.s, d))) {
        return Some(d);
    }
    let shapes = if g.node(lead)?.ty().is_a("transform") { aztek_scene::dag::shapes(g, lead, false) } else { vec![lead] };
    for sh in shapes {
        if g.node(sh).is_some_and(|n| n.ty().is_a("mesh")) {
            for h in aztek_scene::history::history_of(g, sh) {
                if has_manip(&type_of(&core.s, h)) {
                    return Some(h);
                }
            }
        }
    }
    Some(lead)
}

fn pick(core: &mut Core, vp: &Viewport, dpr: f64) -> Option<Manip> {
    let id = target(core)?;
    build(&mut core.s, vp, id, core.node_manip.world, dpr)
}

fn owner(s: &mut Session, node: NodeId) -> Option<(NodeId, DMat4)> {
    let items = gather(&mut s.graph);
    items.iter().find(|it| aztek_scene::history::history_of(&s.graph, it.shape).contains(&node)).map(|it| (it.shape, it.world))
}

struct Frame {
    centre: DVec3,
    x: DVec3,
    y: DVec3,
    z: DVec3,
}

/// The components' centre and axes, the ones the edit's local values are in: z the normal, x along the first edge.
fn frame_of(m: &aztek_geo::Mesh, kind: ComponentMode, ids: &[u32]) -> Frame {
    if kind == ComponentMode::Face {
        let g = aztek_geo::ops::face_group(m, ids);
        let x = (g.tangent - g.normal * g.tangent.dot(g.normal)).normalize_or_zero();
        return Frame { centre: g.center, x, y: g.normal.cross(x), z: g.normal };
    }
    let pts = components::convert(m, kind, ids, ComponentMode::Vertex, false);
    let valid: Vec<u32> = pts.into_iter().filter(|&p| m.point_valid(p)).collect();
    let centre = valid.iter().map(|&p| m.position(p)).sum::<DVec3>() / valid.len().max(1) as f64;
    let n = valid.iter().flat_map(|&p| m.point_faces(p)).map(|f| m.face_area_vector(f)).sum::<DVec3>().normalize_or_zero();
    let n = if n == DVec3::ZERO { DVec3::Z } else { n };
    let (x, y) = n.any_orthonormal_pair();
    Frame { centre, x, y, z: n }
}

/// The history node's input mesh and components, in its kind.
fn input_of(s: &mut Session, node: NodeId, ty: &str) -> Option<(std::sync::Arc<aztek_geo::Mesh>, ComponentMode, Vec<u32>)> {
    let op = aztek_scene::history::op(ty)?;
    let v = s.graph.value(&PlugRef::new(node, aztek_scene::history::INPUT_POLYMESH as i32)).ok()?;
    let PlugValue::Mesh(Some(mesh)) = v else { return None };
    let text = s.graph.node(node)?.data().string(aztek_scene::history::INPUT_COMPONENTS).to_string();
    let mut ids = aztek_scene::history::parse_components(&text);
    if ids.is_empty() {
        ids = components::all_ids(&mesh, op.kind);
    }
    Some((mesh, op.kind, ids))
}

/// Puts a local axis in world: its direction, and attribute units per world unit along it.
fn out_axis(world: &DMat4, axis: DVec3) -> (DVec3, f64) {
    let v = DMat3::from_mat4(*world) * axis;
    let l = v.length().max(1e-9);
    (v / l, 1.0 / l)
}

fn arrow(m: &mut Manip, s: &mut Session, attr: &str, from: DVec3, axis: DVec3, per_world: f64, len: f64, color: [u8; 4], min: Option<f64>, max: Option<f64>) {
    let start = get(s, &m.node, attr);
    m.handles.push(Handle { attr: attr.into(), form: Form::Arrow { from, to: from + axis * len }, color, drive: Drive::Along { axis, start, factor: per_world, min, max } });
}

fn cube(m: &mut Manip, s: &mut Session, attr: &str, from: DVec3, axis: DVec3, len: f64, color: [u8; 4]) {
    let start = get(s, &m.node, attr);
    m.handles.push(Handle { attr: attr.into(), form: Form::Cube { from, to: from + axis * len }, color, drive: Drive::Scale { axis, start, len } });
}

fn ring(m: &mut Manip, s: &mut Session, attr: &str, centre: DVec3, axis: DVec3, radius: f64, color: [u8; 4]) {
    let start = get(s, &m.node, attr);
    m.handles.push(Handle { attr: attr.into(), form: Form::Ring { centre, axis, radius }, color, drive: Drive::Turn { centre, axis, start, factor: 1.0 } });
}

/// A handle sitting out on a size: dragging it writes the attribute, whose world length is attr / per_world * share.
fn size(m: &mut Manip, s: &mut Session, attr: &str, centre: DVec3, axis: DVec3, per_world: f64, share: f64, color: [u8; 4]) {
    let start = get(s, &m.node, attr);
    let at = centre + axis * (start * share / per_world);
    m.handles.push(Handle { attr: attr.into(), form: Form::Dot { at }, color, drive: Drive::Along { axis, start, factor: per_world / share, min: Some(0.0), max: None } });
    m.lines.push((centre, at, color));
}

/// A node's handles, or None when it has none in this scene.
pub fn build(s: &mut Session, vp: &Viewport, node: NodeId, world_axes: bool, dpr: f64) -> Option<Manip> {
    let ty = type_of(s, node);
    let mut m = Manip { node: s.graph.path(node), ..Default::default() };
    if is_move_family(&ty) {
        move_family(s, vp, node, &ty, world_axes, dpr, &mut m)?;
    } else if ty == "polyExtrudeVertex" {
        let (shape_world, (mesh, kind, ids)) = (owner(s, node)?.1, input_of(s, node, &ty)?);
        let f = frame_of(&mesh, kind, &ids);
        let at = shape_world.transform_point3(f.centre);
        let (n, per) = out_axis(&shape_world, f.z);
        let len = vp.handle_len(at, dpr);
        arrow(&mut m, s, "length", at, n, per, len, BLUE, Some(0.0), None);
    } else if ty == "polyPoke" {
        let (shape_world, (mesh, kind, ids)) = (owner(s, node)?.1, input_of(s, node, &ty)?);
        let f = frame_of(&mesh, kind, &ids);
        let lt = DVec3::new(get(s, &m.node, "localTranslateX"), get(s, &m.node, "localTranslateY"), get(s, &m.node, "localTranslateZ"));
        let at = shape_world.transform_point3(f.centre + f.x * lt.x + f.y * lt.y + f.z * lt.z);
        let len = vp.handle_len(at, dpr);
        for (i, (axis, attr)) in [(f.x, "localTranslateX"), (f.y, "localTranslateY"), (f.z, "localTranslateZ")].into_iter().enumerate() {
            let (a, per) = out_axis(&shape_world, axis);
            arrow(&mut m, s, attr, at, a, per, len, AXES[i], None, None);
        }
    } else if ty == "polyMirror" {
        mirror(s, vp, node, dpr, &mut m)?;
    } else if ty == "polyWedgeFace" {
        let (shape_world, (mesh, kind, ids)) = (owner(s, node)?.1, input_of(s, node, &ty)?);
        let f = frame_of(&mesh, kind, &ids);
        let at = shape_world.transform_point3(f.centre);
        let (n, _) = out_axis(&shape_world, f.y);
        let len = vp.handle_len(at, dpr);
        ring(&mut m, s, "wedgeAngle", at, n, len, CYAN);
    } else if is_primitive(&ty) {
        primitive(s, node, &ty, &mut m)?;
    } else if is_nonlinear(&ty) {
        let h = deformer_handle(s, node)?;
        nonlinear(s, vp, h, &ty, dpr, &mut m);
    } else if ty == "softMod" {
        soft_mod(s, vp, dpr, &mut m);
    } else if is_projection(&ty) {
        projection(s, vp, node, &ty, dpr, &mut m)?;
    } else if ty == "sculpt" {
        let h = aztek_scene::deform::driver_of(&s.graph, node)?;
        let w = aztek_scene::rig::world(&mut s.graph, h);
        let at = w.w_axis.truncate();
        let r = DMat3::from_mat4(w).x_axis.length();
        let len = vp.handle_len(at, dpr) * 0.7;
        arrow(&mut m, s, "maximumDisplacement", at + DVec3::Y * r, DVec3::Y, 1.0, len, CYAN, Some(0.0), None);
        arrow(&mut m, s, "dropoffDistance", at + DVec3::X * r, DVec3::X, 1.0, len, CYAN, Some(0.0), None);
    } else if s.graph.node(node)?.ty().is_a("ikHandle") {
        ik_handle(s, vp, node, dpr, &mut m);
    } else if let Some(cam) = camera_shape(s, node) {
        camera(s, vp, node, cam, dpr, &mut m);
    } else if s.graph.node(node)?.ty().is_a("transform") {
        // A plain object: its translate, as a transform's default manipulator.
        let w = aztek_scene::rig::world(&mut s.graph, node);
        let at = w.w_axis.truncate();
        let len = vp.handle_len(at, dpr);
        for (i, attr) in ["translateX", "translateY", "translateZ"].into_iter().enumerate() {
            let axis = DMat3::from_mat4(w).col(i).normalize_or_zero();
            arrow(&mut m, s, attr, at, axis, 1.0, len, AXES[i], None, None);
        }
    } else {
        return None;
    }
    (!m.handles.is_empty()).then_some(m)
}

fn move_family(s: &mut Session, vp: &Viewport, node: NodeId, ty: &str, world_axes: bool, dpr: f64, m: &mut Manip) -> Option<()> {
    let (shape, shape_world) = owner(s, node)?;
    let (mesh, kind, ids) = input_of(s, node, ty)?;
    let away = aztek_scene::components::mirrored_side(&s.graph, shape, kind, &ids);
    let near: Vec<u32> = ids.iter().copied().filter(|i| !away.contains(i)).collect();
    let f = frame_of(&mesh, kind, if near.is_empty() { &ids } else { &near });
    let n = m.node.clone();
    let lt = DVec3::new(get(s, &n, "localTranslateX"), get(s, &n, "localTranslateY"), get(s, &n, "localTranslateZ"));
    let thick = get(s, &n, "thickness");
    let t = DVec3::new(get(s, &n, "translateX"), get(s, &n, "translateY"), get(s, &n, "translateZ"));
    let at = shape_world.transform_point3(f.centre + f.x * lt.x + f.y * lt.y + f.z * (lt.z + thick)) + t;
    let len = vp.handle_len(at, dpr);
    let (prefixes, axes): ([&str; 3], [(DVec3, f64); 3]) = if world_axes {
        (["translate", "rotate", "scale"], [(DVec3::X, 1.0), (DVec3::Y, 1.0), (DVec3::Z, 1.0)])
    } else {
        (["localTranslate", "localRotate", "localScale"], [out_axis(&shape_world, f.x), out_axis(&shape_world, f.y), out_axis(&shape_world, f.z)])
    };
    for (i, c) in ["X", "Y", "Z"].into_iter().enumerate() {
        let (a, per) = axes[i];
        arrow(m, s, &format!("{}{c}", prefixes[0]), at, a, per, len, AXES[i], None, None);
        cube(m, s, &format!("{}{c}", prefixes[2]), at, a, len * 0.7, AXES[i]);
        ring(m, s, &format!("{}{c}", prefixes[1]), at, a, len * 1.15, AXES[i]);
    }
    // The small box beside the handles that swaps local and world.
    let view = vp.cam.view();
    let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
    m.handles.push(Handle { attr: "world".into(), form: Form::Dot { at: at + right * len * 1.4 }, color: if world_axes { ACTIVE } else { CYAN }, drive: Drive::Space });
    Some(())
}

fn mirror(s: &mut Session, vp: &Viewport, node: NodeId, dpr: f64, m: &mut Manip) -> Option<()> {
    let (_, w) = owner(s, node)?;
    let axis = match get(s, &m.node, "axis") as i64 {
        0 => DVec3::X,
        1 => DVec3::Y,
        _ => DVec3::Z,
    };
    let pos = get(s, &m.node, "mirrorPosition");
    let at = w.w_axis.truncate() + axis * pos;
    let len = vp.handle_len(at, dpr);
    arrow(m, s, "mirrorPosition", at, axis, 1.0, len, if axis == DVec3::X { RED } else if axis == DVec3::Y { GREEN } else { BLUE }, None, None);
    let (u, v) = axis.any_orthonormal_pair();
    let k = len * 1.5;
    let q = [at + (u + v) * k, at + (v - u) * k, at - (u + v) * k, at + (u - v) * k];
    for i in 0..4 {
        m.lines.push((q[i], q[(i + 1) % 4], CYAN));
    }
    Some(())
}

fn primitive(s: &mut Session, node: NodeId, ty: &str, m: &mut Manip) -> Option<()> {
    let (_, w) = owner(s, node)?;
    let at = w.w_axis.truncate();
    let l = DMat3::from_mat4(w);
    let side = |i: usize| (l.col(i).normalize_or_zero(), 1.0 / l.col(i).length().max(1e-9));
    let (x, px) = side(0);
    let (y, py) = side(1);
    let (z, pz) = side(2);
    match ty {
        "polyCube" => {
            size(m, s, "width", at, x, px, 0.5, RED);
            size(m, s, "height", at, y, py, 0.5, GREEN);
            size(m, s, "depth", at, z, pz, 0.5, BLUE);
        }
        "polyPlane" => {
            size(m, s, "width", at, x, px, 0.5, RED);
            size(m, s, "height", at, z, pz, 0.5, BLUE);
        }
        "polySphere" => size(m, s, "radius", at, x, px, 1.0, RED),
        "polyCylinder" | "polyCone" => {
            size(m, s, "radius", at, x, px, 1.0, RED);
            size(m, s, "height", at, y, py, 0.5, GREEN);
        }
        "polyTorus" => {
            size(m, s, "radius", at, x, px, 1.0, RED);
            let r = get(s, &m.node, "radius");
            size(m, s, "sectionRadius", at + x * (r / px), y, py, 1.0, GREEN);
        }
        _ => {}
    }
    Some(())
}

fn deformer_handle(s: &Session, node: NodeId) -> Option<NodeId> {
    let n = s.graph.node(node)?;
    let i = n.ty().attr_index("handleMatrix")?;
    let src = s.graph.source(&PlugRef::new(node, i as i32))?;
    Some(aztek_scene::dag::transform_of(&s.graph, src.node))
}

fn nonlinear(s: &mut Session, vp: &Viewport, h: NodeId, ty: &str, dpr: f64, m: &mut Manip) {
    let w = aztek_scene::rig::world(&mut s.graph, h);
    let at = w.w_axis.truncate();
    let l = DMat3::from_mat4(w);
    let (x, px) = (l.x_axis.normalize_or_zero(), 1.0 / l.x_axis.length().max(1e-9));
    let (y, py) = (l.y_axis.normalize_or_zero(), 1.0 / l.y_axis.length().max(1e-9));
    let (z, pz) = (l.z_axis.normalize_or_zero(), 1.0 / l.z_axis.length().max(1e-9));
    let len = vp.handle_len(at, dpr) * 0.6;
    let on_y = |v: f64| at + y * (v / py);
    if ty == "wave" {
        let (lo, hi) = (get(s, &m.node, "minRadius"), get(s, &m.node, "maxRadius"));
        arrow(m, s, "minRadius", at + x * (lo / px), x, px, len, RED, Some(0.0), None);
        arrow(m, s, "maxRadius", at + x * (hi / px), x, px, len, RED, Some(0.0), None);
        arrow(m, s, "amplitude", at, y, py, len, GREEN, None, None);
        arrow(m, s, "wavelength", at, z, pz, len, BLUE, Some(0.0), None);
        return;
    }
    let (lo, hi) = (get(s, &m.node, "lowBound"), get(s, &m.node, "highBound"));
    arrow(m, s, "lowBound", on_y(lo), -y, -py, len, GREEN, None, Some(0.0));
    arrow(m, s, "highBound", on_y(hi), y, py, len, GREEN, Some(0.0), None);
    match ty {
        "bend" => arrow(m, s, "curvature", at, x, px, len, RED, None, None),
        "twist" => {
            ring(m, s, "startAngle", on_y(lo), y, len * 1.5, RED);
            ring(m, s, "endAngle", on_y(hi), y, len * 1.5, BLUE);
        }
        "squash" => {
            arrow(m, s, "factor", at + z * len * 0.5, y, py, len, RED, None, None);
            arrow(m, s, "expand", at, x, px, len, BLUE, Some(0.0), None);
        }
        "flare" => {
            arrow(m, s, "startFlareX", on_y(lo), x, px, len, RED, Some(0.0), None);
            arrow(m, s, "startFlareZ", on_y(lo), z, pz, len, BLUE, Some(0.0), None);
            arrow(m, s, "endFlareX", on_y(hi), x, px, len, RED, Some(0.0), None);
            arrow(m, s, "endFlareZ", on_y(hi), z, pz, len, BLUE, Some(0.0), None);
        }
        "sine" => {
            arrow(m, s, "amplitude", at, x, px, len, RED, None, None);
            arrow(m, s, "wavelength", at, z, pz, len, BLUE, Some(0.0), None);
        }
        _ => {}
    }
}

/// A projection's frame, its centre and size handles, and rings for its turn.
fn projection(s: &mut Session, vp: &Viewport, node: NodeId, ty: &str, dpr: f64, m: &mut Manip) -> Option<()> {
    let (_, world) = owner(s, node)?;
    let n = m.node.clone();
    let c = DVec3::new(get(s, &n, "projectionCenterX"), get(s, &n, "projectionCenterY"), get(s, &n, "projectionCenterZ"));
    let r = DVec3::new(get(s, &n, "rotateX"), get(s, &n, "rotateY"), get(s, &n, "rotateZ"));
    let axes = DMat3::from_quat(glam::DQuat::from_euler(glam::EulerRot::ZYX, r.z, r.y, r.x));
    let at = world.transform_point3(c);
    let len = vp.handle_len(at, dpr);
    for (i, attr) in ["projectionCenterX", "projectionCenterY", "projectionCenterZ"].into_iter().enumerate() {
        let (a, per) = out_axis(&world, DVec3::AXES[i]);
        arrow(m, s, attr, at, a, per, len, AXES[i], None, None);
    }
    for (i, attr) in ["rotateX", "rotateY", "rotateZ"].into_iter().enumerate() {
        let (a, _) = out_axis(&world, axes.col(i));
        ring(m, s, attr, at, a, len * 1.2, AXES[i]);
    }
    let (ax, px) = out_axis(&world, axes.x_axis);
    let (ay, py) = out_axis(&world, axes.y_axis);
    if ty == "polyPlanarProj" {
        size(m, s, "projectionWidth", at, ax, px, 0.5, CYAN);
        size(m, s, "projectionHeight", at, ay, py, 0.5, CYAN);
        let (w, h) = (get(s, &n, "projectionWidth") * 0.5 / px, get(s, &n, "projectionHeight") * 0.5 / py);
        let q = [at + ax * w + ay * h, at - ax * w + ay * h, at - ax * w - ay * h, at + ax * w - ay * h];
        for i in 0..4 {
            m.lines.push((q[i], q[(i + 1) % 4], CYAN));
        }
    } else {
        let h = get(s, &n, "projectionHeight") * 0.5 / py;
        let radius = get(s, &n, "projectionWidth") * 0.5 / px;
        if ty == "polyCylProj" {
            size(m, s, "projectionHeight", at, ay, py, 0.5, CYAN);
        }
        // The shape it wraps round.
        let reach = if ty == "polyCylProj" { h } else { 0.0 };
        for centre in [at + ay * reach, at - ay * reach] {
            for w in ring_points(centre, ay, radius.max(1e-3), 48).windows(2) {
                m.lines.push((w[0], w[1], CYAN));
            }
        }
    }
    Some(())
}

/// The handle's pole vector, from its start joint, and its twist.
fn ik_handle(s: &mut Session, vp: &Viewport, node: NodeId, dpr: f64, m: &mut Manip) {
    let w = aztek_scene::rig::world(&mut s.graph, node);
    let at = w.w_axis.truncate();
    let len = vp.handle_len(at, dpr);
    for (i, attr) in ["poleVectorX", "poleVectorY", "poleVectorZ"].into_iter().enumerate() {
        arrow(m, s, attr, at, DVec3::AXES[i], 1.0, len, AXES[i], None, None);
    }
    let n = m.node.clone();
    let pole = DVec3::new(get(s, &n, "poleVectorX"), get(s, &n, "poleVectorY"), get(s, &n, "poleVectorZ"));
    m.lines.push((at, at + pole.normalize_or_zero() * len * 1.5, CYAN));
    ring(m, s, "twist", at, DVec3::Y, len * 1.3, CYAN);
}

fn camera_shape(s: &Session, node: NodeId) -> Option<NodeId> {
    let g = &s.graph;
    if g.node(node)?.ty().is_a("camera") {
        return Some(node);
    }
    aztek_scene::dag::shapes(g, node, false).into_iter().find(|&c| g.node(c).is_some_and(|n| n.ty().is_a("camera")))
}

/// The eye's translate, and the centre of interest out along the view.
fn camera(s: &mut Session, _vp: &Viewport, node: NodeId, shape: NodeId, _dpr: f64, m: &mut Manip) {
    let xf = aztek_scene::dag::transform_of(&s.graph, node);
    let w = aztek_scene::rig::world(&mut s.graph, xf);
    let at = w.w_axis.truncate();
    let forward = -w.z_axis.truncate().normalize_or_zero();
    let shape_name = s.graph.path(shape);
    let coi = get(s, &shape_name, "centerOfInterest");
    let mark = at + forward * coi;
    m.node = shape_name;
    m.lines.push((at, mark, CYAN));
    m.handles.push(Handle { attr: "centerOfInterest".into(), form: Form::Dot { at: mark }, color: CYAN, drive: Drive::Along { axis: forward, start: coi, factor: 1.0, min: Some(0.001), max: None } });
}

fn soft_mod(s: &mut Session, vp: &Viewport, dpr: f64, m: &mut Manip) {
    let n = m.node.clone();
    let c = DVec3::new(get(s, &n, "falloffCenterX"), get(s, &n, "falloffCenterY"), get(s, &n, "falloffCenterZ"));
    let r = get(s, &n, "falloffRadius");
    let len = vp.handle_len(c, dpr);
    for (i, (axis, attr)) in [(DVec3::X, "falloffCenterX"), (DVec3::Y, "falloffCenterY"), (DVec3::Z, "falloffCenterZ")].into_iter().enumerate() {
        arrow(m, s, attr, c, axis, 1.0, len, AXES[i], None, None);
    }
    let view = vp.cam.view();
    let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
    let up = DVec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
    size(m, s, "falloffRadius", c, right, 1.0, 1.0, CYAN);
    for k in 0..48 {
        let (a, b) = (k as f64 / 48.0 * std::f64::consts::TAU, (k + 1) as f64 / 48.0 * std::f64::consts::TAU);
        m.lines.push((c + (right * a.cos() + up * a.sin()) * r, c + (right * b.cos() + up * b.sin()) * r, CYAN));
    }
}

fn ring_points(centre: DVec3, axis: DVec3, radius: f64, n: usize) -> Vec<DVec3> {
    let (u, v) = axis.any_orthonormal_pair();
    (0..=n)
        .map(|k| {
            let a = k as f64 / n as f64 * std::f64::consts::TAU;
            centre + (u * a.cos() + v * a.sin()) * radius
        })
        .collect()
}

/// How far a pixel is from a handle, in pixels; None when it can't be seen.
fn distance(vp: &Viewport, h: &Handle, px: DVec2) -> Option<f64> {
    let screen = vp.screen();
    let seg = |a: DVec2, b: DVec2| {
        let ab = b - a;
        let t = if ab.length_squared() < 1e-9 { 0.0 } else { ((px - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
        (a + ab * t - px).length()
    };
    match &h.form {
        // Only the tips: the middle is crowded where handles share a centre, and a scale box sits inside its arrow.
        Form::Arrow { from, to } => {
            let (a, b) = screen.project_segment(*from, *to)?;
            Some(seg(a.lerp(b, 0.8), b))
        }
        Form::Cube { from, to } => {
            let (a, b) = screen.project_segment(*from, *to)?;
            Some(seg(a.lerp(b, 0.85), b.lerp(a, -0.15)))
        }
        Form::Dot { at } => Some((screen.project(*at)? - px).length()),
        Form::Ring { centre, axis, radius } => {
            let pts = ring_points(*centre, *axis, *radius, 48);
            pts.windows(2).filter_map(|w| screen.project_segment(w[0], w[1])).map(|(a, b)| seg(a, b)).reduce(f64::min)
        }
    }
}

/// The handle under a pixel. Dots and tips before rings, as they are the smaller targets.
pub fn hit<'a>(vp: &Viewport, m: &'a Manip, px: DVec2, dpr: f64) -> Option<&'a Handle> {
    let mut best: Option<(f64, &Handle)> = None;
    for h in &m.handles {
        let Some(d) = distance(vp, h, px) else { continue };
        let d = if matches!(h.form, Form::Ring { .. }) { d + 3.0 * dpr } else { d };
        if d < 9.0 * dpr && best.is_none_or(|b| d < b.0) {
            best = Some((d, h));
        }
    }
    best.map(|b| b.1)
}

/// The value a drag from start to px gives.
pub fn value_at(vp: &Viewport, h: &Handle, start: DVec2, px: DVec2) -> Option<f64> {
    let screen = vp.screen();
    let delta = px - start;
    let along = |axis: DVec3, from: DVec3, scale: f64| -> Option<f64> {
        let c = screen.project(from)?;
        let tip = screen.project(from + axis * scale)?;
        let d = tip - c;
        (d.length_squared() > 1e-6).then(|| delta.dot(d) / d.length_squared() * scale)
    };
    let from = match &h.form {
        Form::Arrow { from, .. } | Form::Cube { from, .. } => *from,
        Form::Dot { at } => *at,
        Form::Ring { centre, .. } => *centre,
    };
    let unit = vp.handle_len(from, 1.0);
    match &h.drive {
        Drive::Along { axis, start: v0, factor, min, max } => {
            let mut v = v0 + factor * along(*axis, from, unit)?;
            if let Some(lo) = min {
                v = v.max(*lo);
            }
            if let Some(hi) = max {
                v = v.min(*hi);
            }
            Some(v)
        }
        Drive::Scale { axis, start: v0, len } => Some(v0 * (1.0 + along(*axis, from, unit)? / len).max(0.001)),
        Drive::Turn { centre, axis, start: v0, factor } => {
            let c = screen.project(*centre)?;
            let mut a = (px - c).to_angle() - (start - c).to_angle();
            if a > std::f64::consts::PI {
                a -= std::f64::consts::TAU;
            } else if a < -std::f64::consts::PI {
                a += std::f64::consts::TAU;
            }
            Some(v0 + factor * a * -(vp.cam.eye() - *centre).dot(*axis).signum())
        }
        Drive::Space => None,
    }
}

/// The tool's mouse: kind 0 press, 1 move, else release. None leaves it to the view, which selects as usual.
pub fn mouse(core: &mut Core, vp: &Viewport, kind: i32, button: i32, px: DVec2, buttons: i32, dpr: f64) -> Option<bool> {
    match kind {
        0 => {
            let m = pick(core, vp, dpr)?;
            let chosen = match button {
                1 => hit(vp, &m, px, dpr).cloned(),
                // A middle drag takes the handle under the mouse or the highlighted channel's.
                2 => hit(vp, &m, px, dpr).cloned().or_else(|| {
                    let a = core.node_manip.active_attr.clone()?;
                    m.handles.iter().find(|h| h.attr == a).cloned()
                }),
                _ => None,
            }?;
            if matches!(chosen.drive, Drive::Space) {
                core.node_manip.world = !core.node_manip.world;
                core.version += 1;
                return Some(true);
            }
            core.node_manip.grab = Some(Grab { handle: chosen, node: m.node, start: px, applied: false, last: None });
            Some(true)
        }
        1 => {
            if let Some(g) = core.node_manip.grab.take() {
                let mut g = g;
                if let Some(v) = value_at(vp, &g.handle, g.start, px) {
                    if g.applied {
                        core.retract_preview();
                    }
                    let plug = format!("{}.{}", g.node, g.handle.attr);
                    g.applied = core.exec_silent("setAttr", json!({"args": [plug, v]})).is_ok();
                    g.last = g.applied.then_some(v);
                }
                core.node_manip.grab = Some(g);
                return Some(true);
            }
            if buttons == 0 {
                let attr = pick(core, vp, dpr).and_then(|m| hit(vp, &m, px, dpr).map(|h| h.attr.clone()));
                if attr != core.node_manip.hover {
                    core.node_manip.hover = attr;
                    return Some(true);
                }
            }
            None
        }
        _ => {
            let g = core.node_manip.grab.take()?;
            if g.applied {
                core.retract_preview();
                if let Some(v) = g.last {
                    let plug = format!("{}.{}", g.node, g.handle.attr);
                    let _ = core.exec("setAttr", json!({"args": [plug, v]}));
                }
            }
            Some(true)
        }
    }
}

/// The lines to draw, world space, with the hovered or dragged handle and the highlighted channel in yellow.
pub fn draw_lines(core: &mut Core, vp: &Viewport, dpr: f64) -> Vec<(DVec3, DVec3, [u8; 4])> {
    let Some(m) = pick(core, vp, dpr) else { return Vec::new() };
    let st = &core.node_manip;
    let lit = st.grab.as_ref().map(|g| g.handle.attr.clone()).or_else(|| st.hover.clone()).or_else(|| st.active_attr.clone());
    let mut out = m.lines.clone();
    for h in &m.handles {
        let c = if lit.as_deref() == Some(h.attr.as_str()) { ACTIVE } else { h.color };
        match &h.form {
            Form::Arrow { from, to } => {
                out.push((*from, *to, c));
                let d = *to - *from;
                let (u, v) = d.normalize_or_zero().any_orthonormal_pair();
                let k = d.length() * 0.08;
                let base = *to - d * 0.2;
                for w in [u, -u, v, -v] {
                    out.push((*to, base + w * k, c));
                }
            }
            Form::Cube { from, to } => {
                out.push((*from, *to, c));
                push_box(&mut out, *to, (*to - *from).length() * 0.08, c);
            }
            Form::Ring { centre, axis, radius } => {
                for w in ring_points(*centre, *axis, *radius, 48).windows(2) {
                    out.push((w[0], w[1], c));
                }
            }
            Form::Dot { at } => push_box(&mut out, *at, vp.handle_len(*at, dpr) * 0.06, c),
        }
    }
    out
}

fn push_box(out: &mut Vec<(DVec3, DVec3, [u8; 4])>, at: DVec3, k: f64, c: [u8; 4]) {
    let corner = |i: usize| at + DVec3::new(if i & 1 == 0 { -k } else { k }, if i & 2 == 0 { -k } else { k }, if i & 4 == 0 { -k } else { k });
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0 {
                out.push((corner(i), corner(i | bit), c));
            }
        }
    }
}
