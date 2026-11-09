//! Centre pivot, freeze, bake pivot, hide and show, ungroup, reset, and listing
//! unfrozen transforms. Joints wait for the rig port: freezeTransforms and bakePivot refuse them, as setPivot does.

use std::collections::HashSet;
use std::sync::Arc;

use aztek_geo::Index;
use aztek_ops::{fail, objects, ArgType as T, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::{DMat3, DMat4, DVec3};
use serde_json::{json, Value};

use crate::components;
use crate::graph::Graph;
use crate::instancing;
use crate::modifiers;
use crate::node_bounds::{object_bounds, transformed};
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;
use crate::transform::{euler_to_matrix, RotateOrder, Transform};
use crate::transform_ops::{self as xf, Space};

fn flag(name: &str, ty: T, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

fn node_named(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    g.find(name)?.ok_or_else(|| CommandError::new(format!("{cmd}: no node named '{name}'")))
}

fn is_transform(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("transform"))
}

fn is_joint(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().is_a("joint"))
}

fn top_level(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<NodeId>> {
    let mut ids = Vec::new();
    for name in objects(s, a) {
        ids.push(node_named(&s.graph, &name, cmd)?);
    }
    Ok(xf::top_level(&s.graph, &ids))
}

fn set(g: &mut Graph, id: NodeId, attr: usize, v: impl Into<PlugValue>) -> CmdResult<()> {
    g.set_value(PlugRef::new(id, attr as i32), v.into())?;
    Ok(())
}

/// Bounds of the node and everything under it, in the node's object space.
fn hierarchy_bounds(g: &mut Graph, root: NodeId) -> aztek_geo::Bounds {
    let mut b = object_bounds(g, root);
    let to_root = g.node(root).unwrap().matrix(Transform::WORLD_MATRIX).inverse();
    for c in g.descendants(root) {
        let local = object_bounds(g, c);
        if local.valid() {
            let m = to_root * g.node(c).unwrap().matrix(Transform::WORLD_MATRIX);
            let t = transformed(&local, &m);
            b.extend(t.min);
            b.extend(t.max);
        }
    }
    b
}

/// Sets the node's local values so it stays at world under its parent as the parent is now.
fn keep_world(g: &mut Graph, id: NodeId, world: &DMat4) -> CmdResult<()> {
    g.evaluate();
    let local = g.node(id).unwrap().matrix(Transform::PARENT_MATRIX).inverse() * *world;
    Transform::set_local_matrix(g, id, &local)?;
    Ok(())
}

fn children_worlds(g: &Graph, id: NodeId) -> Vec<(NodeId, DMat4)> {
    g.children(id)
        .iter()
        .filter(|&&c| is_transform(g, c))
        .map(|&c| (c, g.node(c).unwrap().matrix(Transform::WORLD_MATRIX)))
        .collect()
}

/// Applies an object-space matrix to the node's geometry: a mesh's points (its faces flipped back when the matrix
/// mirrors), or a point provider's bake.
pub fn bake_into_geometry(g: &mut Graph, id: NodeId, bake: &DMat4) -> CmdResult<()> {
    // A transform bakes into the shapes under it.
    if is_transform(g, id) {
        for sh in crate::dag::shapes(g, id, true) {
            bake_into_geometry(g, sh, bake)?;
        }
        return Ok(());
    }
    let Some(n) = g.node(id) else { return Ok(()) };
    if n.ty().is_a("mesh") {
        // The points are shared with other objects that keep their own transforms, so this one stops sharing.
        if instancing::make_unique(g, id)? {
            log::warn!("{} shared its mesh; it has a copy of its own now", g.path(id));
        }
        let base = modifiers::edit_plug(g, id)?;
        if let PlugValue::Mesh(Some(m)) = g.stored_value(&base) {
            let mut copy = (**m).clone();
            for p in 0..copy.point_capacity() {
                if copy.point_valid(p) {
                    let at = bake.transform_point3(copy.position(p));
                    copy.set_position(p, at);
                }
            }
            // A mirroring bake turns faces inside out; flip them back.
            if DMat3::from_mat4(*bake).determinant() < 0.0 {
                for f in 0..copy.face_capacity() {
                    if copy.face_valid(f as Index) {
                        copy.flip_face(f);
                    }
                }
            }
            g.set_value(base, PlugValue::Mesh(Some(Arc::new(copy))))?;
        }
    } else if let Some(bake_fn) = components::point_provider(n).and_then(|pp| pp.bake) {
        bake_fn(g, id, bake);
    }
    Ok(())
}

pub fn register(c: &mut Commands<Session>) {
    c.register(
        CommandDef::<Session>::new(
            "movePivot",
            "Move objects' pivots without moving the objects: -by a world offset, or -to a world position. The reference spells it \
             move -r x y z obj.scalePivot obj.rotatePivot.",
            |s, a| {
                s.graph.evaluate();
                for name in objects(s, a) {
                    let id = node_named(&s.graph, &name, "movePivot")?;
                    if !is_transform(&s.graph, id) {
                        continue;
                    }
                    let now = crate::transform_ops::world_pivot(&mut s.graph, id);
                    let at = if let Some(t) = a.vec3("to") {
                        DVec3::from_array(t)
                    } else {
                        now + DVec3::from_array(a.vec3("by").unwrap_or([0.0; 3]))
                    };
                    xf::set_world_pivot(&mut s.graph, id, at)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(FlagDef::new("by", "r", T::Vec3, Value::Null, "World offset."))
        .flag(FlagDef::new("to", "", T::Vec3, Value::Null, "World position.")),
    );
    c.register(
        CommandDef::<Session>::new(
            "centerPivot",
            "Move pivots to the centre of each object's bounding box (mesh points, curve points and handles, and everything \
             parented under it, so a group centres on its children) without moving the object.",
            |s, a| {
                s.graph.evaluate();
                for name in objects(s, a) {
                    let id = node_named(&s.graph, &name, "centerPivot")?;
                    if !is_transform(&s.graph, id) {
                        continue;
                    }
                    let b = hierarchy_bounds(&mut s.graph, id);
                    let centre = if b.valid() { b.center() } else { DVec3::ZERO };
                    let world = s.graph.node(id).unwrap().matrix(Transform::WORLD_MATRIX).transform_point3(centre);
                    xf::set_world_pivot(&mut s.graph, id, world)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("objects"),
    );

    c.register(
        CommandDef::<Session>::new(
            "freezeTransforms",
            "Bake translate/rotate/scale into mesh or curve points, leaving identity values. World positions don't change. \
             With hierarchy (the default, like the reference) everything parented under the objects is frozen too; without it the \
             children's transforms absorb what their parent gives up. Joints keep their translate and take their rotation \
             into jointOrient, or with jointOrient line up with the world.",
            |s, a| {
                let mut ids = top_level(s, a, "freezeTransforms")?;
                // Parents first, so each child is frozen after taking on what its parent gave up.
                if a.get("hierarchy").and_then(Value::as_bool).unwrap_or(true) {
                    let roots = ids.len();
                    for i in 0..roots {
                        let below = s.graph.descendants(ids[i]);
                        ids.extend(below);
                    }
                }
                let flag_on = |name: &str| a.get(name).and_then(Value::as_bool).unwrap_or(true);
                let g = &mut s.graph;
                for id in ids {
                    g.evaluate();
                    if !is_transform(g, id) {
                        continue;
                    }
                    if is_joint(g, id) {
                        // The reference's makeIdentity on joints: rotate goes into jointOrient and scale to 1; translate stays,
                        // since a joint's place is its translate. Children keep their world placement.
                        let children = children_worlds(g, id);
                        let n = g.node(id).unwrap();
                        let (r, jo) = (n.vec3(Transform::ROTATE), n.vec3(crate::rig::Joint::JOINT_ORIENT));
                        if flag_on("rotate") {
                            let order = crate::transform::RotateOrder::from_index(n.int(Transform::ROTATE_ORDER));
                            let m = crate::transform::euler_to_matrix(jo, crate::transform::RotateOrder::Xyz) * crate::transform::euler_to_matrix(r, order);
                            let e = crate::transform::matrix_to_euler(&glam::DMat3::from_mat4(m), crate::transform::RotateOrder::Xyz);
                            set(g, id, crate::rig::Joint::JOINT_ORIENT, e)?;
                            set(g, id, Transform::ROTATE, DVec3::ZERO)?;
                        }
                        if flag_on("scale") {
                            set(g, id, Transform::SCALE, DVec3::ONE)?;
                        }
                        g.evaluate();
                        for (c, world) in children {
                            keep_world(g, c, &world)?;
                        }
                        continue;
                    }
                    let n = g.node(id).unwrap();
                    let before = n.matrix(Transform::MATRIX);
                    let custom_axes = n.vec3(Transform::PIVOT_ORIENT) != DVec3::ZERO;
                    // Children keep their world placement by absorbing what this node gives up.
                    let children = children_worlds(g, id);
                    let pivot = xf::world_pivot(g, id);
                    let pivot_axes = xf::pivot_frame(g, id);
                    if flag_on("translate") {
                        set(g, id, Transform::TRANSLATE, DVec3::ZERO)?;
                    }
                    if flag_on("rotate") {
                        set(g, id, Transform::ROTATE, DVec3::ZERO)?;
                    }
                    if flag_on("scale") {
                        set(g, id, Transform::SCALE, DVec3::ONE)?;
                    }
                    for attr in
                        [Transform::ROTATE_PIVOT, Transform::SCALE_PIVOT, Transform::ROTATE_PIVOT_TRANSLATE, Transform::SCALE_PIVOT_TRANSLATE]
                    {
                        set(g, id, attr, DVec3::ZERO)?;
                    }
                    g.evaluate();
                    let after = g.node(id).unwrap().matrix(Transform::MATRIX);
                    bake_into_geometry(g, id, &(after.inverse() * before))?;
                    // The pivot stays where it was in the world, like the reference, and so do axes the user oriented.
                    xf::set_world_pivot(g, id, pivot)?;
                    if custom_axes {
                        xf::set_world_pivot_frame(g, id, &pivot_axes)?;
                    }
                    g.evaluate();
                    for (c, world) in children {
                        keep_world(g, c, &world)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects")
        .flag(flag("apply", T::Bool, json!(true), "The reference's makeIdentity -a: bake the transform into the geometry (always on)."))
        .flag(FlagDef::new("normal", "n", T::Int, Value::Null, "Accepted for existing scripts."))
        .flag(FlagDef::new("preserveNormals", "pn", T::Bool, Value::Null, "Accepted for existing scripts."))
        .flag(FlagDef::new("translate", "t", T::Bool, json!(true), "Freeze translation."))
        .flag(FlagDef::new("rotate", "r", T::Bool, json!(true), "Freeze rotation."))
        .flag(FlagDef::new("scale", "s", T::Bool, json!(true), "Freeze scale."))
        .flag(flag("hierarchy", T::Bool, json!(true), "Freeze everything parented under the objects too."))
        .flag(flag("jointOrient", T::Bool, json!(false), "Also line joints' axes up with the world, zeroing their orientation.")),
    );
    c.set_adjustable("freezeTransforms", true);

    c.register(
        CommandDef::<Session>::new(
            "bakePivot",
            "Turn the objects onto their pivots' axes, counter-rotating their points, so the pivot orientation becomes the \
             object's own and scale works along it. Nothing moves in the world. Instances are made unique.",
            |s, a| {
                let ids = top_level(s, a, "bakePivot")?;
                let g = &mut s.graph;
                for id in ids {
                    g.evaluate();
                    if !is_transform(g, id) {
                        continue;
                    }
                    let orient = g.node(id).unwrap().vec3(Transform::PIVOT_ORIENT);
                    if is_joint(g, id) || orient == DVec3::ZERO {
                        continue;
                    }
                    let before = g.node(id).unwrap().matrix(Transform::MATRIX);
                    let children = children_worlds(g, id);
                    let pivot = xf::world_pivot(g, id);
                    let turn = DMat3::from_mat4(euler_to_matrix(orient, RotateOrder::Xyz));
                    let rotate = xf::rotated_euler(g, id, &turn, Space::Object);
                    set(g, id, Transform::ROTATE, rotate)?;
                    set(g, id, Transform::PIVOT_ORIENT, DVec3::ZERO)?;
                    g.evaluate();
                    let after = g.node(id).unwrap().matrix(Transform::MATRIX);
                    bake_into_geometry(g, id, &(after.inverse() * before))?;
                    xf::set_world_pivot(g, id, pivot)?;
                    g.evaluate();
                    for (c, world) in children {
                        keep_world(g, c, &world)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects"),
    );

    c.register(
        CommandDef::<Session>::new("hide", "Hide objects (visibility off). Defaults to the selection.", |s, a| {
            let mut hidden = Vec::new();
            for name in objects(s, a) {
                let id = node_named(&s.graph, &name, "hide")?;
                if is_transform(&s.graph, id) {
                    set(&mut s.graph, id, Transform::VISIBILITY, false)?;
                    hidden.push(id);
                }
            }
            if !hidden.is_empty() {
                s.graph.last_hidden = hidden;
            }
            Ok(Value::Null)
        })
        .positional("objects"),
    );

    c.register(CommandDef::<Session>::new(
        "showLastHidden",
        "Show the objects the last hide (or hiding toggle) hid. Returns them.",
        |s, _a| {
            let mut out = Vec::new();
            for id in s.graph.last_hidden.clone() {
                if is_transform(&s.graph, id) {
                    set(&mut s.graph, id, Transform::VISIBILITY, true)?;
                    out.push(Value::String(s.graph.path(id)));
                }
            }
            Ok(Value::Array(out))
        },
    ));

    c.register(
        CommandDef::<Session>::new(
            "toggleVisibility",
            "Toggle visibility (the H key). Defaults to the selection. Every object ends up the same: hidden if most were \
             visible (a tie hides), shown otherwise. Returns the new visibility.",
            |s, a| {
                let mut ids = Vec::new();
                let mut shown = 0;
                for name in objects(s, a) {
                    let id = node_named(&s.graph, &name, "toggleVisibility")?;
                    if is_transform(&s.graph, id) {
                        ids.push(id);
                        shown += s.graph.node(id).unwrap().bool(Transform::VISIBILITY) as usize;
                    }
                }
                if ids.is_empty() {
                    return Ok(Value::Null);
                }
                let show = shown * 2 < ids.len();
                for &id in &ids {
                    if s.graph.node(id).unwrap().bool(Transform::VISIBILITY) != show {
                        set(&mut s.graph, id, Transform::VISIBILITY, show)?;
                    }
                }
                if !show {
                    s.graph.last_hidden = ids;
                }
                Ok(Value::Bool(show))
            },
        )
        .positional("objects"),
    );

    c.register(
        CommandDef::<Session>::new("show", "Show objects. all=True shows every hidden object.", |s, a| {
            if a.bool("all") {
                for id in s.graph.nodes() {
                    if is_transform(&s.graph, id) && !s.graph.node(id).unwrap().bool(Transform::VISIBILITY) {
                        set(&mut s.graph, id, Transform::VISIBILITY, true)?;
                    }
                }
                return Ok(Value::Null);
            }
            for name in objects(s, a) {
                let id = node_named(&s.graph, &name, "show")?;
                if is_transform(&s.graph, id) {
                    set(&mut s.graph, id, Transform::VISIBILITY, true)?;
                }
            }
            Ok(Value::Null)
        })
        .positional("objects")
        .flag(flag("all", T::Bool, json!(false), "Show everything.")),
    );

    c.register(
        CommandDef::<Session>::new(
            "ungroup",
            "Move each group's children up to the group's parent, keeping where they are, then delete the group. Defaults to \
             the selection; returns the children.",
            |s, a| {
                let mut out = Vec::new();
                for name in objects(s, a) {
                    let id = node_named(&s.graph, &name, "ungroup")?;
                    let n = s.graph.node(id).unwrap();
                    if n.ty().name != "transform" {
                        fail!("ungroup: '{name}' is not a group");
                    }
                    if n.reference_id() != 0 {
                        fail!("ungroup: '{name}' comes from a reference");
                    }
                    let up = s.graph.parent(id);
                    for c in s.graph.children(id).to_vec() {
                        s.graph.set_parent(c, up, true, usize::MAX)?;
                        out.push(Value::String(s.graph.path(c)));
                    }
                    s.graph.delete_node(id)?;
                }
                Ok(Value::Array(out))
            },
        )
        .positional("groups"),
    );

    c.register(
        CommandDef::<Session>::new(
            "resetTransforms",
            "Put translate, rotate and scale back to their defaults (the object moves).",
            |s, a| {
                for name in objects(s, a) {
                    let id = node_named(&s.graph, &name, "resetTransforms")?;
                    if !is_transform(&s.graph, id) {
                        continue;
                    }
                    set(&mut s.graph, id, Transform::TRANSLATE, DVec3::ZERO)?;
                    set(&mut s.graph, id, Transform::ROTATE, DVec3::ZERO)?;
                    set(&mut s.graph, id, Transform::SCALE, DVec3::ONE)?;
                }
                Ok(Value::Null)
            },
        )
        .positional("objects"),
    );

    c.register(
        CommandDef::<Session>::new(
            "unfrozenTransforms",
            "List the objects whose translate, rotate or scale is not identity (0, 0, 0 and 1, 1, 1), with their values.\n\n\
             Checks the objects given or selected, or with scene every mesh, curve and group (joints, cameras, lights and \
             locators are placed by their transforms and are left out). With rotateStep, rotations that are whole multiples \
             of the step count as frozen; invert lists only rotations on the step instead. Returns one {name, translate, \
             rotate, scale, unfrozen} per object listed, unfrozen naming the channels that put it there.",
            |s, a| {
                s.graph.evaluate();
                let tol = a.float("tolerance", 1e-4);
                let step = a.float("rotateStep", 0.0);
                let invert = step > 0.0 && a.bool("invert");
                let on = |name: &str| a.get(name).and_then(Value::as_bool).unwrap_or(true);
                let g = &s.graph;
                // What a scene-wide or hierarchy check takes in: the things whose transforms are meant to be frozen.
                let checkable = |id: NodeId| {
                    g.node(id).is_some_and(|n| {
                        n.ty().is_a("transform") && !n.ty().is_a("joint") && !n.ty().is_a("locator") && !n.ty().is_a("light")
                    })
                };
                let mut ids = Vec::new();
                if a.bool("scene") {
                    ids.extend(g.nodes().into_iter().filter(|&id| checkable(id)));
                } else {
                    for name in objects(s, a) {
                        let id = node_named(g, &name, "unfrozenTransforms")?;
                        if is_transform(g, id) {
                            ids.push(id);
                        }
                        if a.bool("hierarchy") {
                            ids.extend(g.descendants(id).into_iter().filter(|&d| checkable(d)));
                        }
                    }
                }
                let near = |v: DVec3, to: DVec3| (v - to).abs().cmple(DVec3::splat(tol)).all();
                let on_step = |r: DVec3| near(r, (r / step).round() * step);
                let mut out = Vec::new();
                let mut seen = HashSet::new();
                for id in ids {
                    if !seen.insert(id) {
                        continue;
                    }
                    let n = g.node(id).unwrap();
                    let (t, r, sc) = (n.vec3(Transform::TRANSLATE), n.vec3(Transform::ROTATE), n.vec3(Transform::SCALE));
                    let mut unfrozen = Vec::new();
                    if on("translate") && !near(t, DVec3::ZERO) {
                        unfrozen.push("translate");
                    }
                    // Zero is on every step, so invert never lists an unrotated object.
                    if on("rotate") && !near(r, DVec3::ZERO) && (step <= 0.0 || on_step(r) == invert) {
                        unfrozen.push("rotate");
                    }
                    if on("scale") && !near(sc, DVec3::ONE) {
                        unfrozen.push("scale");
                    }
                    if !unfrozen.is_empty() {
                        out.push(json!({
                            "name": g.path(id),
                            "translate": t.to_array(),
                            "rotate": (if aztek_ops::units::user_call() { r / aztek_ops::units::rad_per_unit() } else { r }).to_array(),
                            "scale": sc.to_array(),
                            "unfrozen": unfrozen,
                        }));
                    }
                }
                Ok(Value::Array(out))
            },
        )
        .positional("objects")
        .flag(flag("scene", T::Bool, json!(false), "Check the whole scene instead of the objects."))
        .flag(flag("hierarchy", T::Bool, json!(false), "Also check everything parented under the objects."))
        .flag(flag("translate", T::Bool, json!(true), "List objects with translation."))
        .flag(flag("rotate", T::Bool, json!(true), "List objects with rotation."))
        .flag(flag("scale", T::Bool, json!(true), "List objects with scale."))
        .flag(flag("rotateStep", T::Float, json!(0.0), "Degrees: rotations on whole multiples of it count as frozen. 0 is off."))
        .flag(flag("invert", T::Bool, json!(false), "With rotateStep, list rotations on the step rather than off it."))
        .flag(flag("tolerance", T::Float, json!(1e-4), "How far from identity, or from the step, still counts as on it."))
        .undoable(false),
    );
}
