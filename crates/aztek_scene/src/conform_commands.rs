//! Straighten, circularize, spherize,
//! conform to a shape or another object, and edit edge flow.

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use aztek_geo::conform::{
    self, CircleOptions, ConformMode, ConformOptions, PointMoves, RectangleOptions, ShapeSelection, Spacing, SphereFit, SphereRadius,
    SpherizeOptions, SpherizeResult, StraightenOptions,
};
use aztek_geo::loop_tools::{self, EdgeFlowOptions};
use aztek_geo::{topology, Index, Mesh};
use aztek_ops::{fail, ArgType as T, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::components::{self, ComponentMode, Ids};
use crate::graph::Graph;
use crate::last_op;
use crate::mesh_edit::transform::{deform_selected_components, PointTarget};
use crate::mesh_nodes::MeshNode;
use crate::node::Node;
use crate::plug::{MeshPtr, NodeId};
use crate::session::Session;
use crate::transform::{euler_to_matrix, RotateOrder, Transform};

/// The sphere the latest polySpherize used, in world space, for the viewport tool to draw and edit.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SphereState {
    pub center: DVec3,
    /// cm.
    pub radius: f64,
    /// Semi-axes relative to the radius, along the frame's axes.
    pub scale: DVec3,
    /// Euler degrees, XYZ: the world turn of the ellipsoid's axes.
    pub rotation: DVec3,
    /// Counts runs, so a reader can tell when it changed.
    pub serial: u64,
}

impl Default for SphereState {
    fn default() -> Self {
        Self { center: DVec3::ZERO, radius: 0.0, scale: DVec3::ONE, rotation: DVec3::ZERO, serial: 0 }
    }
}

pub fn last_sphere(g: &Graph) -> &SphereState {
    &g.last_sphere
}

/// Non-mesh nodes polyConformToObject can conform to. A provider turns such a node into world-space geometry; a target
/// without faces is used through its edges (a curve as a chain of edges: nearest point, or loop mode on a closed one).
pub mod conform_targets {
    use super::*;

    pub type Provider = Arc<dyn Fn(&mut Graph, NodeId) -> Option<Mesh> + Send + Sync>;

    pub(super) fn providers() -> &'static RwLock<HashMap<String, Provider>> {
        static P: OnceLock<RwLock<HashMap<String, Provider>>> = OnceLock::new();
        P.get_or_init(Default::default)
    }

    pub fn add(node_type: &str, p: Provider) {
        providers().write().unwrap().insert(node_type.to_string(), p);
    }

    /// The node's world-space geometry, or None when no provider takes its type.
    pub fn world_geometry(g: &mut Graph, n: NodeId) -> Option<Mesh> {
        let ty = g.node(n)?.ty().name.clone();
        let p = providers().read().unwrap().get(&ty).cloned()?;
        p(g, n)
    }
}

fn flag(name: &str, ty: T, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

const SPACINGS: &[&str] = &["even", "keep", "project"];

fn strength_flag() -> FlagDef {
    flag("strength", T::Float, json!(1.0), "How far toward the shape, 0 to 1; soft selection fades it further.").min(0.0).max(1.0)
}

fn spacing_arg(a: &Args, fallback: &str) -> Spacing {
    match a.string("spacing", fallback).as_str() {
        "keep" => Spacing::Keep,
        "project" => Spacing::Project,
        _ => Spacing::Even,
    }
}

/// A world distance in the mesh's object space, by its average scale.
fn to_local_length(world: &DMat4, cm: f64) -> f64 {
    let scale = DMat3::from_mat4(*world).determinant().abs().cbrt();
    if scale > 1e-12 {
        cm / scale
    } else {
        cm
    }
}

fn shape_of(m: &Mesh, kind: ComponentMode, ids: &Ids) -> ShapeSelection {
    match kind {
        ComponentMode::Edge => conform::shape_of_edges(m, ids),
        ComponentMode::Face => conform::shape_of_faces(m, ids),
        _ => conform::shape_of_points(m, ids),
    }
}

/// Moved points go strength of the way to their target; soft-selected ones take the displacement of the nearest moved
/// point.
fn target_of_moves(m: &Mesh, moves: &PointMoves, strength: f64) -> PointTarget {
    let cap = m.point_capacity() as usize;
    let mut offset = vec![DVec3::ZERO; cap];
    let mut has = vec![false; cap];
    let mut rest_at = Vec::with_capacity(moves.len());
    let mut offset_at = Vec::with_capacity(moves.len());
    for mv in moves {
        let p = mv.point as usize;
        let off = (mv.position - m.position(mv.point)) * strength;
        offset[p] = off;
        has[p] = true;
        rest_at.push(m.position(mv.point));
        offset_at.push(off);
    }
    Arc::new(move |p, rest| {
        let p = p as usize;
        if p < has.len() && has[p] {
            return rest + offset[p];
        }
        let mut best = None;
        let mut best_d = f64::MAX;
        for (i, at) in rest_at.iter().enumerate() {
            let d = (*at - rest).length_squared();
            if d < best_d {
                best_d = d;
                best = Some(i);
            }
        }
        match best {
            Some(i) => rest + offset_at[i],
            None => rest,
        }
    })
}

/// Runs f on each mesh's selection seen as chains; the selection's face interiors follow its borders. allow_whole
/// takes whole selected meshes in object mode.
fn conform_selected(
    g: &mut Graph,
    command: &str,
    what: &str,
    strength: f64,
    f: &mut dyn FnMut(&Mesh, &DMat4, &ShapeSelection) -> PointMoves,
    allow_whole: bool,
) -> CmdResult<()> {
    let f = std::cell::RefCell::new(f);
    let edited = deform_selected_components(
        g,
        command,
        &|m, world, kind, ids| {
            let sel = shape_of(m, kind, ids);
            let mut moves = (f.borrow_mut())(m, world, &sel);
            if moves.is_empty() {
                return None;
            }
            conform::follow_interior(m, &sel.interior, &mut moves);
            Some(target_of_moves(m, &moves, strength))
        },
        allow_whole,
    )?;
    if edited == 0 {
        fail!("{command}: {what}");
    }
    Ok(())
}

/// With radius 0 (fit), the fitted radius becomes the op's radius, so adjusting it starts from the circle the loops
/// already make.
fn record_fitted_radius(s: &mut Session, a: &Args, radii: &[f64]) {
    if a.float("radius", 0.0) > 0.0 || radii.is_empty() {
        return;
    }
    // Loops of different sizes keep fitting each their own; one radius would force them all to it.
    let lo = radii.iter().copied().fold(f64::MAX, f64::min);
    let hi = radii.iter().copied().fold(f64::MIN, f64::max);
    if hi - lo > 1e-3 * hi {
        return;
    }
    last_op::limit_flag(s, "radius", json!((hi * 1000.0).round() / 1000.0));
}

fn conformable(n: &Node) -> bool {
    n.ty().is_a("mesh") || conform_targets::providers().read().unwrap().contains_key(&n.ty().name)
}

/// The conformable node a name or selected node stands for: itself, else its first conformable shape.
fn conform_node(g: &Graph, id: NodeId) -> Option<NodeId> {
    if g.node(id).is_some_and(conformable) {
        return Some(id);
    }
    crate::dag::shapes(g, id, false).into_iter().find(|&s| g.node(s).is_some_and(conformable))
}

fn has_components_selected(g: &Graph, id: NodeId) -> bool {
    components::selected_all(g, components::mode(g)).contains_key(&id)
}

/// The target= mesh or curve, else the one selected mesh or curve with no components selected in the current mode.
fn conform_target(g: &Graph, a: &Args) -> CmdResult<NodeId> {
    let name = a.string("target", "");
    if !name.is_empty() {
        return match g.find(&name)?.and_then(|id| conform_node(g, id)) {
            Some(id) => Ok(id),
            _ => fail!("polyConformToObject: {name} is not a mesh or curve"),
        };
    }
    let mut found = None;
    for &sel in g.selection() {
        let Some(id) = conform_node(g, sel) else { continue };
        if !has_components_selected(g, id) {
            found = Some(id);
        }
    }
    found.ok_or_else(|| CommandError::new("polyConformToObject: give target= or also select the mesh or curve to conform to"))
}

fn shape_moves(shape: &str, a: &Args, m: &Mesh, world: &DMat4, sel: &ShapeSelection) -> PointMoves {
    let radius = to_local_length(world, a.float("radius", 0.0).max(0.0));
    let spacing = spacing_arg(a, "even");
    let all = conform::shape_points(sel);
    match shape {
        "line" => {
            if sel.chains.is_empty() {
                conform::align_to_line(m, &all)
            } else {
                conform::straighten(m, sel, &StraightenOptions { spacing, fit_line: true })
            }
        }
        "square" | "rectangle" => conform::rectangularize(
            m,
            sel,
            &RectangleOptions { spacing, square: shape == "square", rotation: a.float("rotation", 0.0), flatten: true },
        ),
        "sphere" => conform::spherize(m, &all, &SpherizeOptions { radius, ..Default::default() }, None),
        "cylinder" => {
            let axis = match a.string("axis", "auto").as_str() {
                "x" => 0,
                "y" => 1,
                "z" => 2,
                _ => -1,
            };
            conform::cylinderize(m, &all, axis, radius)
        }
        "plane" => conform::flatten_to_plane(m, &all),
        _ => conform::circularize(
            m,
            sel,
            &CircleOptions { spacing, radius, rotation: a.float("rotation", 0.0), sides: a.int("sides", 0) as i32, flatten: true },
            None,
        ),
    }
}

fn vec_flag(a: &Args, name: &str, fallback: DVec3) -> DVec3 {
    a.vec3(name).map(DVec3::from_array).unwrap_or(fallback)
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "polyStraighten",
            "Straighten the selected edge chains (or chains of selected vertices) into lines between their ends. Several \
             separate chains straighten separately; closed loops are left alone. Vertices with no edges between them line up \
             on their best-fit line.",
            |s, a| {
                let o = StraightenOptions { spacing: spacing_arg(a, "keep"), fit_line: a.bool("fitLine") };
                conform_selected(
                    &mut s.graph,
                    "polyStraighten",
                    "select open edge chains or vertices to straighten",
                    a.float("strength", 1.0),
                    &mut |m, _w, sel| conform::straighten(m, sel, &o),
                    false,
                )?;
                Ok(Value::Null)
            },
        )
        .flag(
            flag("spacing", T::String, json!("keep"), "keep (relative distances along the chain), even or project (straight across).")
                .choices(SPACINGS),
        )
        .flag(flag("fitLine", T::Bool, json!(false), "Use the line through all of a chain's points instead of its ends."))
        .flag(strength_flag()),
    );

    c.register(
        CommandDef::<Session>::new(
            "polyCircularize",
            "Fit the selected edge loops, the border of each selected face region, or the selected vertices to a circle in \
             their best-fit plane, turned to move the points least. Points inside a face region follow the border smoothly, \
             so a rectangular face comes out round, ready to extrude into a pipe. Open chains become an arc between their ends.",
            |s, a| {
                let mut radii = Vec::new();
                conform_selected(
                    &mut s.graph,
                    "polyCircularize",
                    "select edge loops, faces or vertices to make round",
                    a.float("strength", 1.0),
                    &mut |m, world, sel| {
                        let o = CircleOptions {
                            spacing: spacing_arg(a, "even"),
                            radius: to_local_length(world, a.float("radius", 0.0).max(0.0)),
                            rotation: a.float("rotation", 0.0),
                            sides: a.int("sides", 0) as i32,
                            flatten: a.get("flatten").and_then(Value::as_bool).unwrap_or(true),
                        };
                        let mut local = Vec::new();
                        let moves = conform::circularize(m, sel, &o, Some(&mut local));
                        let unit = to_local_length(world, 1.0);
                        let to_world = if unit > 1e-12 { 1.0 / unit } else { 1.0 };
                        radii.extend(local.iter().map(|r| r * to_world));
                        moves
                    },
                    false,
                )?;
                record_fitted_radius(s, a, &radii);
                Ok(Value::Null)
            },
        )
        .flag(
            flag(
                "spacing",
                T::String,
                json!("even"),
                "even (equal angles), keep (relative distances along the loop) or project (straight out to the circle).",
            )
            .choices(SPACINGS),
        )
        .flag(flag("radius", T::Float, json!(0.0), "Radius, cm; 0 keeps the loop's average radius.").min(0.0))
        .flag(flag("rotation", T::Float, json!(0.0), "Extra turn about the circle's axis, degrees."))
        .flag(
            flag("sides", T::Int, json!(0), "3 or more: conform to a regular polygon with this many sides, its corners on the circle.")
                .min(0.0)
                .max(64.0),
        )
        .flag(flag("flatten", T::Bool, json!(true), "Put the points in the circle's plane; off keeps each one's height off it."))
        .flag(strength_flag()),
    );

    c.register(
        CommandDef::<Session>::new(
            "polySpherize",
            "Push the selected components (whole meshes in object mode) onto a sphere fitted to them, or onto an ellipsoid \
             with scale. center and radius override the fit (world space, cm); radiusMode picks the radius from the points \
             otherwise. With alongNormals each point slides along its own normal to the sphere. Returns the sphere used: \
             {center, radius, scale, rotation}, world space.",
            |s, a| {
                let scale = vec_flag(a, "scale", DVec3::ONE);
                let rotation = vec_flag(a, "rotation", DVec3::ZERO);
                let turn = DMat3::from_mat4(euler_to_matrix(rotation, RotateOrder::Xyz));
                let radius_mode = match a.string("radiusMode", "average").as_str() {
                    "fit" => SphereRadius::Fit,
                    "min" => SphereRadius::Min,
                    "max" => SphereRadius::Max,
                    "explicit" => SphereRadius::Explicit,
                    _ => SphereRadius::Average,
                };
                let fit = if a.string("fit", "best") == "center" { SphereFit::Center } else { SphereFit::Best };
                let mut state = SphereState::default();
                let mut any = false;
                conform_selected(
                    &mut s.graph,
                    "polySpherize",
                    "no components selected",
                    1.0,
                    &mut |m, world, sel| {
                        let mut linear = DMat3::from_mat4(*world);
                        for (i, col) in [&mut linear.x_axis, &mut linear.y_axis, &mut linear.z_axis].into_iter().enumerate() {
                            *col = if col.length() > 1e-12 { col.normalize() } else { DVec3::AXES[i] };
                        }
                        let o = SpherizeOptions {
                            fit,
                            radius_mode,
                            amount: a.float("amount", 1.0).clamp(0.0, 1.0),
                            radius: to_local_length(world, a.float("radius", 0.0).max(0.0)),
                            center: a.vec3("center").map(|c| world.inverse().transform_point3(DVec3::from_array(c))),
                            scale,
                            frame: linear.transpose() * turn,
                            along_normals: a.bool("alongNormals"),
                            normals: None,
                        };
                        let mut fitted = SpherizeResult::default();
                        let moves = conform::spherize(m, &conform::shape_points(sel), &o, Some(&mut fitted));
                        if !moves.is_empty() {
                            let local = to_local_length(world, 1.0);
                            state.center = world.transform_point3(fitted.center);
                            state.radius = if local > 1e-12 { fitted.radius / local } else { fitted.radius };
                            any = true;
                        }
                        moves
                    },
                    true,
                )?;
                if any {
                    state.scale = scale;
                    state.rotation = rotation;
                    state.serial = s.graph.last_sphere.serial + 1;
                    s.graph.last_sphere = state;
                }
                Ok(json!({
                    "center": state.center.to_array(),
                    "radius": state.radius,
                    "scale": scale.to_array(),
                    "rotation": rotation.to_array(),
                }))
            },
        )
        .flag(flag("amount", T::Float, json!(1.0), "How far toward the sphere, 0 to 1; soft selection fades it further.").min(0.0).max(1.0))
        .flag(
            flag("fit", T::String, json!("best"), "best (least-squares sphere) or center (around the points' centroid).")
                .choices(&["best", "center"]),
        )
        .flag(
            flag(
                "radiusMode",
                T::String,
                json!("average"),
                "Radius when radius is 0: average distance, fit (the fitted radius), min, max or explicit.",
            )
            .choices(&["average", "fit", "min", "max", "explicit"]),
        )
        .flag(flag("radius", T::Float, json!(0.0), "Radius, cm; above 0 it replaces radiusMode.").min(0.0))
        .flag(flag("center", T::Vec3, Value::Null, "Sphere centre, world space cm; default: the fitted centre."))
        .flag(flag("scale", T::Vec3, json!([1.0, 1.0, 1.0]), "Ellipsoid semi-axis factors along its X, Y and Z."))
        .flag(flag("rotation", T::Vec3, json!([0.0, 0.0, 0.0]), "Turn of the ellipsoid's axes in world space, degrees (XYZ)."))
        .flag(flag("alongNormals", T::Bool, json!(false), "Move each point along its own normal onto the sphere.")),
    );

    c.register(
        CommandDef::<Session>::new(
            "polyConformShape",
            "Conform the selected components to a fitted shape: line, circle, square or rectangle (per chain or loop, corners \
             on the points nearest the fitted rectangle's), sphere, cylinder (fitted axis) or plane (flatten). Face interiors \
             follow their borders for the loop shapes.",
            |s, a| {
                let shape = a.string("shape", "circle");
                conform_selected(
                    &mut s.graph,
                    "polyConformShape",
                    "the selection has nothing that shape fits",
                    a.float("strength", 1.0),
                    &mut |m, world, sel| shape_moves(&shape, a, m, world, sel),
                    false,
                )?;
                Ok(Value::Null)
            },
        )
        .flag(
            flag("shape", T::String, json!("circle"), "line, circle, square, rectangle, sphere, cylinder or plane.")
                .choices(&["line", "circle", "square", "rectangle", "sphere", "cylinder", "plane"]),
        )
        .flag(flag("spacing", T::String, json!("even"), "Line, circle, square, rectangle: even, keep or project.").choices(SPACINGS))
        .flag(flag("radius", T::Float, json!(0.0), "Circle, sphere, cylinder: radius, cm; 0 fits it.").min(0.0))
        .flag(flag("rotation", T::Float, json!(0.0), "Circle, square, rectangle: extra turn in the plane, degrees."))
        .flag(flag("sides", T::Int, json!(0), "Circle: 3 or more makes it a regular polygon.").min(0.0).max(64.0))
        .flag(
            flag("axis", T::String, json!("auto"), "Cylinder: auto (fitted), x, y or z (object axes).").choices(&["auto", "x", "y", "z"]),
        )
        .flag(strength_flag()),
    );

    c.register(
        CommandDef::<Session>::new(
            "polyConformToObject",
            "Conform the selected components onto another mesh or a curve. loop: each selected edge loop (or face region \
             border) lands one to one on the target's nearest closed loop (a cylinder's rim, a ring), at the turn that moves it \
             least, or spread along it when the counts differ; face interiors follow. surface: every point to the nearest \
             surface point. vertex: every point to the nearest target vertex. A curve target works through its line: loop \
             spreads a loop along a closed curve, surface puts each point on the nearest point of the curve.",
            |s, a| {
                let g = &mut s.graph;
                g.evaluate();
                let t = conform_target(g, a)?;
                let mut target: Option<MeshPtr> = None;
                let mut target_world = DMat4::IDENTITY;
                let is_mesh = g.node(t).unwrap().ty().is_a("mesh");
                if is_mesh {
                    let node = g.node(t).unwrap();
                    target = node.data().mesh(MeshNode::OUT_MESH).cloned();
                    target_world = node.matrix(Transform::WORLD_MATRIX);
                } else if let Some(w) = conform_targets::world_geometry(g, t) {
                    target = Some(Arc::new(w));
                }
                let path = components::display_path(g, t);
                let Some(target) = target.filter(|m| m.num_points() > 0) else {
                    fail!("polyConformToObject: {path} has no geometry");
                };
                let mode = match a.string("mode", "loop").as_str() {
                    "surface" => ConformMode::Surface,
                    "vertex" => ConformMode::Vertex,
                    _ => ConformMode::Loop,
                };
                let o = ConformOptions { mode, spacing: spacing_arg(a, "even") };
                if has_components_selected(g, t) {
                    fail!("polyConformToObject: {path} cannot conform to itself; deselect its components");
                }
                let strength = a.float("strength", 1.0);
                let edited = deform_selected_components(
                    g,
                    "polyConformToObject",
                    &|m, world, kind, ids| {
                        let mut local = (*target).clone();
                        let to_local = world.inverse() * target_world;
                        for p in 0..local.point_capacity() {
                            if local.point_valid(p) {
                                let at = to_local.transform_point3(local.position(p));
                                local.set_position(p, at);
                            }
                        }
                        let sel = shape_of(m, kind, ids);
                        let mut moves = conform::conform_to_mesh(m, &sel, &local, &o);
                        if moves.is_empty() {
                            return None;
                        }
                        if o.mode == ConformMode::Loop {
                            conform::follow_interior(m, &sel.interior, &mut moves);
                        }
                        Some(target_of_moves(m, &moves, strength))
                    },
                    false,
                )?;
                if edited == 0 {
                    fail!("polyConformToObject: nothing to conform; select components on another mesh");
                }
                Ok(Value::Null)
            },
        )
        .flag(flag(
            "target",
            T::String,
            Value::Null,
            "Mesh or curve to conform to; default: the selected mesh or curve that has no components selected.",
        ))
        .flag(flag("mode", T::String, json!("loop"), "loop, surface or vertex.").choices(&["loop", "surface", "vertex"]))
        .flag(flag("spacing", T::String, json!("even"), "Loop mode with unequal counts: even, keep or project.").choices(SPACINGS))
        .flag(strength_flag()),
    );

    c.register(
        CommandDef::<Session>::new(
            "polyEditEdgeFlow",
            "Move the selected edge loops onto the curvature of the surface around them (the reference's Edit Edge Flow). Each vertex \
             lands on a curve through the nearest unselected vertices across the loop, shaped by the vertices beyond them, and \
             neighbouring selected loops spread evenly between those, as subdividing would place them. Selected vertices off \
             any chain, and face selections, fit both directions across each vertex.",
            |s, a| {
                let o = EdgeFlowOptions {
                    amount: a.float("adjustEdgeFlow", 1.0),
                    edge_flow: a.get("edgeFlow").and_then(Value::as_bool).unwrap_or(true),
                    tension: a.float("tension", 0.0),
                    loop_smooth: a.float("loopSmooth", 0.0),
                    iterations: a.int("iterations", 1).clamp(1, 100) as i32,
                };
                let edited = deform_selected_components(
                    &mut s.graph,
                    "polyEditEdgeFlow",
                    &|m, _world, kind, ids| {
                        let (edges, loose): (Vec<Index>, Vec<Index>) = if kind == ComponentMode::Edge {
                            (ids.to_vec(), Vec::new())
                        } else {
                            let pts = if kind == ComponentMode::Face { topology::points_of_faces(m, ids) } else { ids.to_vec() };
                            loop_tools::flow_selection_of_points(m, &pts)
                        };
                        let moves = loop_tools::edit_edge_flow(m, &edges, &loose, &o);
                        if moves.is_empty() {
                            None
                        } else {
                            Some(target_of_moves(m, &moves, 1.0))
                        }
                    },
                    false,
                )?;
                if edited == 0 {
                    fail!("polyEditEdgeFlow: select edge loops, vertices or faces to adjust");
                }
                Ok(Value::Null)
            },
        )
        .flag(flag(
            "adjustEdgeFlow",
            T::Float,
            json!(1.0),
            "0 flat between the neighbours, 1 on the curve, above 1 exaggerated, negative inverted.",
        ))
        .flag(flag(
            "edgeFlow",
            T::Bool,
            json!(true),
            "Shape the curve from the vertices beyond the neighbours; off, from the surface normals at the neighbours.",
        ))
        .flag(flag("tension", T::Float, json!(0.0), "Toward 1 flattens the curve, negative makes it fuller."))
        .flag(flag("loopSmooth", T::Float, json!(0.0), "Also smooth each loop along its length, 0 to 1, keeping its spacing.").min(0.0).max(1.0))
        .flag(flag("iterations", T::Int, json!(1), "Passes of Smooth Along Loop.").min(1.0).max(100.0)),
    );

    for name in ["polyStraighten", "polyCircularize", "polySpherize", "polyConformShape", "polyConformToObject", "polyEditEdgeFlow"] {
        c.set_adjustable(name, true);
    }
    // Option layouts (ops::options overrides) are not implemented.
}
