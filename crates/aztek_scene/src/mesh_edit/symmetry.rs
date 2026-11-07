//! Symmetry settings, checks and rebuilds.

use aztek_core::{PrefScope, PrefStore};
use aztek_geo::ops::{symmetrize_radial_topology, symmetrize_topology};
use aztek_geo::symmetry::{axis_plane, remove_symmetry_layers, RadialSymmetry, SymmetryMap, SymmetrySync, MAX_RADIAL_COUNT};
use aztek_geo::{Index, Mesh};
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::DVec3;
use serde_json::{json, Map, Value};

use super::*;
use crate::components::{self, ComponentMode};
use crate::graph::Graph;
use crate::mesh_nodes::{mesh_objects, symmetry_label, symmetry_offset, symmetry_spec, symmetry_state, MeshNode, SymmetryState, SYMMETRY_RADIAL};
use crate::modifiers;
use crate::plug::{NodeId, PlugRef, PlugValue};
use crate::session::Session;

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

/// The radial count the mesh's points fit: the largest that leaves none without an orbit, else the one leaving
/// fewest. 0 when no count fits any point, or the mesh is too dense to try them all.
fn best_radial_count(mesh: &Mesh, radial: &RadialSymmetry, tolerance: f64) -> i32 {
    if mesh.num_points() > 20000 {
        return 0;
    }
    let mut best = 0;
    let mut fewest = mesh.num_points();
    for count in (2..=MAX_RADIAL_COUNT).rev() {
        let r = RadialSymmetry::new(radial.origin(), radial.axis(), count, radial.mirror(), radial.reference());
        let unmatched = SymmetryMap::build_radial(mesh, &r, tolerance).unmatched_count();
        if unmatched == 0 {
            return count;
        }
        if unmatched < fewest {
            fewest = unmatched;
            best = count;
        }
    }
    best
}

/// Warns when a mesh just given symmetry is not symmetric about it, so edits will not mirror cleanly. For radial
/// symmetry it also names the count the mesh fits best: a count that does not match the topology round the axis is
/// the usual mistake.
fn warn_if_asymmetric(g: &mut Graph, id: NodeId) {
    g.evaluate();
    let Some(spec) = symmetry_spec(g, id) else { return };
    let Some(cage) = modifiers::cage(g, id) else { return };
    let mut m = (*cage).clone();
    let tolerance = symmetry_tolerance(&m);
    let map = SymmetryMap::persistent(&mut m, &spec, tolerance, None);
    let c = map.check(&m);
    if c.unmatched == 0 && c.max_asymmetry <= tolerance {
        return;
    }
    let path = components::display_path(g, id);
    let Some(radial) = &spec.radial else {
        log::warn!(
            "{path} is not symmetric in {}: {} points without a partner, max asymmetry {} cm",
            (b'X' + spec.axis as u8) as char,
            c.unmatched,
            c.max_asymmetry
        );
        return;
    };
    let best = best_radial_count(&m, radial, tolerance);
    let hint = if best > 0 && best != radial.count() { format!("; a count of {best} fits its topology best") } else { String::new() };
    log::warn!(
        "{path} does not fit {}: {} points without a full orbit (shown in red), max asymmetry {} cm{hint}",
        symmetry_label(&spec),
        c.unmatched,
        c.max_asymmetry
    );
}

/// The radial settings flags shared by setSymmetry and symmetrizeRadial, applied to a mesh node; true if any changed.
fn apply_radial_flags(g: &mut Graph, a: &Args, n: NodeId, cmd: &str) -> CmdResult<bool> {
    let mut changed = false;
    let mut set = |g: &mut Graph, attr: usize, v: PlugValue| -> CmdResult<()> {
        if g.node(n).unwrap().value(attr) != &v {
            g.set_value(PlugRef::new(n, attr as i32), v)?;
            changed = true;
        }
        Ok(())
    };
    if a.has("count") {
        let count = a.int("count", 0);
        if !(2..=MAX_RADIAL_COUNT as i64).contains(&count) {
            fail!("{cmd}: count must be 2 to {MAX_RADIAL_COUNT}");
        }
        set(g, MeshNode::RADIAL_COUNT, PlugValue::Int(count))?;
    }
    if a.has("radialAxis") {
        let v = match a.string("radialAxis", "").as_str() {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            "custom" => 3,
            _ => fail!("{cmd}: radialAxis must be x, y, z or custom"),
        };
        set(g, MeshNode::RADIAL_AXIS, PlugValue::Int(v))?;
    }
    if a.has("direction") {
        let d = DVec3::from_array(a.vec3("direction").unwrap_or([0.0; 3]));
        if d.length() < 1e-12 {
            fail!("{cmd}: direction must not be zero");
        }
        set(g, MeshNode::RADIAL_DIRECTION, PlugValue::Vec3(d))?;
        set(g, MeshNode::RADIAL_AXIS, PlugValue::Int(3))?;
    }
    if a.has("mirror") {
        set(g, MeshNode::RADIAL_MIRROR, PlugValue::Bool(a.bool("mirror")))?;
    }
    if a.has("phase") {
        set(g, MeshNode::RADIAL_PHASE, PlugValue::Float(a.float("phase", 0.0)))?;
    }
    Ok(changed)
}

fn radial_flags() -> Vec<FlagDef> {
    vec![
        flag("count", T::Int, Value::Null, "Radial: how many wedges repeat round the axis, 2 to 64; default: the mesh's own.")
            .min(2.0)
            .max(MAX_RADIAL_COUNT as f64),
        flag("radialAxis", T::String, Value::Null, "Radial: the axis, x, y, z or custom (direction=); default: the mesh's own.")
            .choices(&["x", "y", "z", "custom"]),
        flag("direction", T::Vec3, Value::Null, "Radial: a custom axis direction, object space."),
        flag("mirror", T::Bool, Value::Null, "Radial: also mirror each wedge across a plane through the axis (dihedral symmetry)."),
        flag("phase", T::Float, Value::Null, "Radial: degrees the wedge boundaries and mirror planes turn about the axis."),
    ]
}

fn int_of(g: &Graph, n: NodeId, attr: usize) -> i64 {
    g.node(n).unwrap().int(attr)
}

fn symmetric_mesh(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    let n = mesh_arg(g, name, cmd)?;
    if symmetry_spec(g, n).is_none() {
        fail!("{cmd}: {name} has symmetry off");
    }
    Ok(n)
}

/// Objects named or selected, else the meshes with selected components.
fn symmetry_targets(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<String>> {
    let mut names = if objects(s, a).is_empty() { Vec::new() } else { mesh_objects(s, a, cmd)? };
    let g = &s.graph;
    let mode = components::mode(g);
    if names.is_empty() && mode != ComponentMode::Object {
        for &node in components::selected_all(g, mode).keys() {
            if g.node(node).is_some() {
                names.push(components::display_path(g, node));
            }
        }
    }
    Ok(names)
}

/// Selected component points of mesh n in the current mode, as vertices.
fn selected_points(g: &Graph, n: NodeId, m: &Mesh) -> Vec<Index> {
    let from = components::mode(g);
    if from == ComponentMode::Object {
        return Vec::new();
    }
    components::convert(m, from, components::selected(g, n, from), ComponentMode::Vertex, false)
}

/// The wedge most of the selected components lie in, else the one holding most of the mesh (a single modeled wedge).
fn keep_wedge(g: &Graph, n: NodeId, m: &Mesh, r: &RadialSymmetry, tol: f64) -> i32 {
    let mut count = vec![0u32; r.order() as usize];
    let tally = |count: &mut Vec<u32>, p: Index| {
        if m.point_valid(p) && r.axis_distance(m.position(p)) > tol {
            count[r.wedge_of(m.position(p)) as usize] += 1;
        }
    };
    for p in selected_points(g, n, m) {
        tally(&mut count, p);
    }
    if count.iter().all(|&c| c == 0) {
        for p in 0..m.point_capacity() {
            tally(&mut count, p);
        }
    }
    // The first of the largest, as std::max_element.
    let mut best = 0;
    for (i, &c) in count.iter().enumerate() {
        if c > count[best] {
            best = i;
        }
    }
    best as i32
}

/// The side to keep: the one most of the selected components lie on, else the side holding nearly all of the mesh (a
/// half model), else positive.
fn keep_side(g: &Graph, n: NodeId, m: &Mesh, axis: i32, plane: f64, tol: f64) -> i32 {
    let a = axis as usize;
    let sum: f64 = selected_points(g, n, m).into_iter().filter(|&p| m.point_valid(p)).map(|p| m.position(p)[a] - plane).sum();
    if sum.abs() > tol {
        return if sum > 0.0 { 1 } else { -1 };
    }
    let (mut below, mut above) = (0u32, 0u32);
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            let d = m.position(p)[a] - plane;
            below += (d < -tol) as u32;
            above += (d > tol) as u32;
        }
    }
    if below > 4 * above {
        -1
    } else {
        1
    }
}

fn axis_char(axis: i32) -> String {
    ((b'x' + axis.clamp(0, 2) as u8) as char).to_string()
}

fn set_base(g: &mut Graph, base: PlugRef, m: Mesh) -> CmdResult<()> {
    g.set_value(base, PlugValue::Mesh(Some(std::sync::Arc::new(m))))?;
    Ok(())
}

fn chunked(g: &mut Graph, label: &str, f: impl FnOnce(&mut Graph) -> CmdResult) -> CmdResult {
    g.open_chunk(label);
    let r = f(g);
    g.close_chunk();
    r
}

pub fn register(c: &mut Commands<Session>) {
    let mut set_symmetry = Cmd::new(
        "setSymmetry",
        "Object-space symmetry for component edits on meshes: 'off', a mirror plane 'x', 'y' or 'z', or 'radial': count \
         wedges repeated round radialAxis (with mirror=True also mirrored within each wedge), every edit applied to all of \
         them.",
        |s, a| {
            let v = match a.string("axis", "x").as_str() {
                "off" => 0,
                "x" => 1,
                "y" => 2,
                "z" => 3,
                "radial" => SYMMETRY_RADIAL,
                _ => fail!("setSymmetry: axis must be off, x, y, z or radial"),
            };
            for name in mesh_objects(s, a, "setSymmetry")? {
                let g = &mut s.graph;
                let n = mesh_arg(g, &name, "setSymmetry")?;
                // A custom plane (or radial centre) belongs to the setting it was set on.
                if v > 0 && v != int_of(g, n, MeshNode::SYMMETRY) && matches!(int_of(g, n, MeshNode::SYMMETRY_PLANE), 1 | crate::mesh_nodes::SYMMETRY_PLANE_WORLD) {
                    g.set_value(PlugRef::new(n, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(0))?;
                }
                let mut changed = v != int_of(g, n, MeshNode::SYMMETRY);
                if apply_radial_flags(g, a, n, "setSymmetry")? && v == SYMMETRY_RADIAL {
                    changed = true;
                }
                g.set_value(PlugRef::new(n, MeshNode::SYMMETRY as i32), PlugValue::Int(v))?;
                if v > 0 && changed {
                    warn_if_asymmetric(g, n);
                }
            }
            Ok(Value::Null)
        },
    )
    .positional("objects")
    .flag(flag("axis", T::String, json!("x"), "off, x, y, z or radial.").choices(&["off", "x", "y", "z", "radial"]));
    for f in radial_flags() {
        set_symmetry = set_symmetry.flag(f);
    }
    c.register(set_symmetry);

    c.register(
        Cmd::new(
            "setSymmetryPlane",
            "Set where meshes' symmetry plane lies: through the selected edges or vertices (which must lie flat across an \
             axis; that axis becomes the symmetry axis), or back through the pivot with pivot=True. With radial symmetry \
             the radial axis passes through the selection's centre instead. Returns the plane per mesh as [axis, offset], \
             or ['radial', centre].",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                g.evaluate();
                chunked(g, "Set Symmetry Plane", |g| {
                    let mut out = Map::new();
                    let forget = |g: &mut Graph, n: NodeId| -> CmdResult<()> {
                        // Pairs stored about the old plane pair the wrong points about the new one.
                        let base = base_plug(g, n)?;
                        let mut m = base_mesh(g, &base);
                        remove_symmetry_layers(&mut m);
                        set_base(g, base, m)
                    };
                    let plane_of = |g: &Graph, n: NodeId| -> Value {
                        if let Some(r) = symmetry_spec(g, n).and_then(|s| s.radial) {
                            let o = r.origin();
                            return json!(["radial", [o.x, o.y, o.z]]);
                        }
                        let axis = symmetry_axis(g, n).max(0);
                        json!([axis_char(axis), symmetry_offset(g, n, axis)])
                    };
                    if a.bool("pivot") {
                        let mut names = names.clone();
                        if names.is_empty() {
                            names = g.selection().iter().map(|&id| components::display_path(g, id)).collect();
                        }
                        for name in names {
                            let Some(n) = g.find(&name)?.and_then(|id| components::as_mesh(g, id)) else { continue };
                            if !components::is_mesh(g, n) || int_of(g, n, MeshNode::SYMMETRY_PLANE) == 0 {
                                continue;
                            }
                            g.set_value(PlugRef::new(n, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(0))?;
                            forget(g, n)?;
                            out.insert(components::display_path(g, n), plane_of(g, n));
                        }
                        return Ok(Value::Object(out));
                    }
                    let from = components::mode(g);
                    if from == ComponentMode::Object {
                        fail!("setSymmetryPlane: select edges or vertices lying on the plane");
                    }
                    for (id, ids) in components::selected_all(g, from).clone() {
                        if !components::is_mesh(g, id) {
                            continue;
                        }
                        let Some(cage) = modifiers::cage(g, id) else { continue };
                        let m = &*cage;
                        let (mut lo, mut hi, mut sum) = (DVec3::splat(1e300), DVec3::splat(-1e300), DVec3::ZERO);
                        let mut count = 0;
                        for p in components::convert(m, from, &ids, ComponentMode::Vertex, false) {
                            if m.point_valid(p) {
                                lo = lo.min(m.position(p));
                                hi = hi.max(m.position(p));
                                sum += m.position(p);
                                count += 1;
                            }
                        }
                        if count == 0 {
                            continue;
                        }
                        if int_of(g, id, MeshNode::SYMMETRY) == SYMMETRY_RADIAL {
                            // A ring of points round the axis: its centre is where the axis passes.
                            g.set_value(PlugRef::new(id, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(1))?;
                            g.set_value(PlugRef::new(id, MeshNode::RADIAL_CENTER as i32), PlugValue::Vec3(sum / count as f64))?;
                            forget(g, id)?;
                            out.insert(components::display_path(g, id), plane_of(g, id));
                            continue;
                        }
                        // The mesh's own axis when the selection is flat across it, else whichever axis it is
                        // flattest across.
                        let spread = hi - lo;
                        let mut axis = symmetry_axis(g, id);
                        if axis < 0 || spread[axis as usize] > symmetry_tolerance(m) {
                            axis = if spread.x <= spread.y && spread.x <= spread.z {
                                0
                            } else if spread.y <= spread.z {
                                1
                            } else {
                                2
                            };
                        }
                        if spread[axis as usize] > symmetry_tolerance(m).max(m.bounds().size().length() * 1e-3) {
                            fail!("setSymmetryPlane: the selection on {} does not lie flat across an axis", components::display_path(g, id));
                        }
                        let offset = sum[axis as usize] / count as f64;
                        g.set_value(PlugRef::new(id, MeshNode::SYMMETRY as i32), PlugValue::Int(axis as i64 + 1))?;
                        g.set_value(PlugRef::new(id, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(1))?;
                        g.set_value(PlugRef::new(id, MeshNode::SYMMETRY_OFFSET as i32), PlugValue::Float(offset))?;
                        forget(g, id)?;
                        out.insert(components::display_path(g, id), json!([axis_char(axis), offset]));
                    }
                    if out.is_empty() {
                        fail!("setSymmetryPlane: select edges or vertices lying on the plane");
                    }
                    Ok(Value::Object(out))
                })
            },
        )
        .positional("objects")
        .flag(flag("pivot", T::Bool, json!(false), "Put the plane (or radial axis) back through the pivot.")),
    );

    c.register(
        Cmd::new(
            "toggleSymmetry",
            "Symmetry off on the meshes if every one already has it on (on axis=, when given), else on for all of them. \
             Non-meshes are skipped. Returns the axis set, or 'off'.",
            |s, a| {
                let names = objects(s, a);
                let g = &mut s.graph;
                let mut ids = Vec::new();
                for name in names {
                    if let Some(id) = g.find(&name)?.and_then(|id| components::as_mesh(g, id)) {
                        if components::is_mesh(g, id) {
                            ids.push(id);
                        }
                    }
                }
                if ids.is_empty() {
                    fail!("toggleSymmetry: no meshes given or selected");
                }
                let st = symmetry_state(g, &ids);
                let state_axis = |axis: i32| if axis == SymmetryState::RADIAL_STATE { "radial".to_string() } else { axis_char(axis) };
                let mut axis = a.string("axis", "");
                if axis.is_empty() {
                    axis = if st.axis >= 0 { state_axis(st.axis) } else { PrefStore::get().string("general", "/symmetry/axis", "x") };
                }
                if !["x", "y", "z", "radial"].contains(&axis.as_str()) {
                    fail!("toggleSymmetry: axis must be x, y, z or radial");
                }
                let wanted = if axis == "radial" { SYMMETRY_RADIAL } else { (axis.as_bytes()[0] - b'x') as i64 + 1 };
                let off = st.on == st.meshes && (!a.has("axis") || (st.axis >= 0 && state_axis(st.axis) == axis));
                let v = if off { 0 } else { wanted };
                for id in ids {
                    let changed = int_of(g, id, MeshNode::SYMMETRY) != v;
                    g.set_value(PlugRef::new(id, MeshNode::SYMMETRY as i32), PlugValue::Int(v))?;
                    if !off && changed {
                        warn_if_asymmetric(g, id);
                    }
                }
                if !off {
                    PrefStore::get().set("general", "/symmetry/axis", json!(axis), PrefScope::User);
                }
                Ok(json!(if off { "off".to_string() } else { axis }))
            },
        )
        .positional("objects")
        .flag(flag(
            "axis",
            T::String,
            Value::Null,
            "x, y, z or radial to turn on with; default: the axis already in use, else the last one picked.",
        )),
    );

    c.register(
        Cmd::new(
            "symmetryCheck",
            "Report how symmetric meshes are across their symmetry plane (or round their radial axis): the largest \
             distance between a point and its partner's reflection (or its orbit's image), and the points without a \
             partner. For radial symmetry also the unmatched point ids and the count the mesh fits best. Changes nothing.",
            |s, a| {
                let names = symmetry_targets(s, a, "symmetryCheck")?;
                let g = &mut s.graph;
                g.evaluate();
                let mut out = Map::new();
                for name in names {
                    let n = symmetric_mesh(g, &name, "symmetryCheck")?;
                    let Some(cage) = modifiers::cage(g, n) else { continue };
                    let mut m = (*cage).clone();
                    let spec = symmetry_spec(g, n).unwrap();
                    let stored = m.point_attrs().has(&symmetry_layer(&spec));
                    let mut sync = SymmetrySync::default();
                    let tol = symmetry_tolerance(&m);
                    let map = SymmetryMap::persistent(&mut m, &spec, tol, Some(&mut sync));
                    let c = map.check(&m);
                    let mut r = json!({
                        "axis": if spec.radial.is_some() { "radial".to_string() } else { axis_char(spec.axis) },
                        "maxAsymmetry": c.max_asymmetry,
                        "worstPoint": if c.worst_point == aztek_geo::INVALID { Value::Null } else { json!(c.worst_point) },
                        "pairs": c.pairs,
                        "onPlane": c.on_plane,
                        "unmatched": c.unmatched,
                        "stored": stored,
                        "withoutStoredPartner": sync.topological + sync.by_position + sync.unmatched,
                    });
                    if let Some(radial) = &spec.radial {
                        r["count"] = json!(radial.count());
                        r["mirror"] = json!(radial.mirror());
                        let unmatched = map.unmatched_points(&m);
                        r["bestCount"] = json!(if unmatched.is_empty() { radial.count() } else { best_radial_count(&m, radial, tol) });
                        r["unmatchedPoints"] = json!(unmatched);
                    }
                    let path = components::display_path(g, n);
                    log::info!(
                        "{path}: symmetry {} max asymmetry {} cm, {} {}, {} {}, {} unmatched",
                        symmetry_name(&spec),
                        c.max_asymmetry,
                        c.pairs,
                        if spec.radial.is_some() { "orbits" } else { "pairs" },
                        c.on_plane,
                        if spec.radial.is_some() { "fixed" } else { "on the plane" },
                        c.unmatched
                    );
                    out.insert(path, r);
                }
                Ok(Value::Object(out))
            },
        )
        .positional("objects")
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "symmetrize",
            "Make meshes exactly symmetric: every point on one side of the symmetry plane is copied, reflected, onto its \
             partner, and on-plane points are put on the plane. With radial symmetry, one wedge's points are copied round \
             the axis onto their orbits.",
            |s, a| {
                let names = symmetry_targets(s, a, "symmetrize")?;
                let drive = if a.string("side", "positive") == "negative" { -1 } else { 1 };
                chunked(&mut s.graph, "Symmetrize", |g| {
                    for name in names {
                        let n = symmetric_mesh(g, &name, "symmetrize")?;
                        let base = base_plug(g, n)?;
                        let mut m = base_mesh(g, &base);
                        let sym = symmetry_for(g, n, &mut m).unwrap();
                        if sym.is_radial() {
                            let mut wedge = a.int("wedge", -1) as i32;
                            if wedge < 0 {
                                wedge = if components::mode(g) == ComponentMode::Object { 0 } else { sym.majority_wedge(&selected_points(g, n, &m)) };
                            }
                            if wedge >= sym.order() {
                                fail!("symmetrize: {} has wedges 0 to {}", components::display_path(g, n), sym.order() - 1);
                            }
                            sym.symmetrize_wedge(&mut m, wedge);
                        } else {
                            sym.symmetrize(&mut m, drive);
                        }
                        set_base(g, base, m)?;
                    }
                    Ok(Value::Null)
                })
            },
        )
        .positional("objects")
        .flag(flag("side", T::String, json!("positive"), "The side that is kept.").choices(&["positive", "negative"]))
        .flag(
            flag("wedge", T::Int, json!(-1), "Radial: the wedge that is kept; -1: the one most of the selected components are in, else 0.")
                .min(-1.0),
        ),
    );

    c.register(
        Cmd::new(
            "symmetryRebuild",
            "Rebuild meshes symmetric: the kept side is mirrored across the plane through the object's pivot, replacing \
             the other side, welded on the plane, and the symmetry pairs are stored again. A mesh with radial symmetry is \
             rebuilt from its kept wedge instead (see symmetrizeRadial). With pairsOnly, the geometry is left alone and \
             only the stored pairs are matched again, by position or, for meshes that are no longer symmetric, by \
             topology outward from the selected edge (which must lie on the plane; any edge for radial symmetry). \
             Returns the points left unmatched per mesh.",
            symmetry_rebuild,
        )
        .positional("objects")
        .flag(flag("axis", T::String, Value::Null, "x, y or z; default: the mesh's symmetry axis, else x."))
        .flag(
            flag(
                "side",
                T::String,
                json!("auto"),
                "The side that is kept; auto: the side of the selected components, else the side holding nearly all of \
                 the mesh, else positive.",
            )
            .choices(&["auto", "positive", "negative"]),
        )
        .flag(
            flag(
                "wedge",
                T::Int,
                json!(-1),
                "Radial: the wedge that is kept; -1: the one most of the selected components are in, else the one holding \
                 most of the mesh.",
            )
            .min(-1.0),
        )
        .flag(flag("pairsOnly", T::Bool, json!(false), "Only match the stored pairs again, leaving the geometry as it is."))
        .flag(flag("topological", T::Bool, json!(false), "With pairsOnly: match by topology from the selected edge."))
        .flag(flag("tolerance", T::Float, json!(0.0), "Weld or match distance, cm (0: scaled to the mesh).").min(0.0)),
    );

    let mut radial = Cmd::new(
        "symmetrizeRadial",
        "Rebuild meshes exactly radially symmetric: the kept wedge is repeated round the axis (and mirrored within each \
         wedge with mirror=True), replacing the rest, and welded along the seams and the axis; then the mesh's symmetry is \
         set to radial with these settings, so every edit after keeps it. Settings default to the mesh's own radial \
         symmetry. Model one wedge and run this to get the whole. Returns the faces per mesh.",
        |s, a| {
            let names = symmetry_targets(s, a, "symmetrizeRadial")?;
            s.graph.evaluate();
            if names.is_empty() {
                fail!("symmetrizeRadial: select a mesh");
            }
            chunked(&mut s.graph, "Symmetrize Radial", |g| {
                let mut out = Map::new();
                for name in names {
                    let n = mesh_arg(g, &name, "symmetrizeRadial")?;
                    apply_radial_flags(g, a, n, "symmetrizeRadial")?;
                    if int_of(g, n, MeshNode::SYMMETRY) != SYMMETRY_RADIAL {
                        // A custom mirror plane says nothing about where the radial axis goes.
                        if int_of(g, n, MeshNode::SYMMETRY_PLANE) == 1 {
                            g.set_value(PlugRef::new(n, MeshNode::SYMMETRY_PLANE as i32), PlugValue::Int(0))?;
                        }
                        g.set_value(PlugRef::new(n, MeshNode::SYMMETRY as i32), PlugValue::Int(SYMMETRY_RADIAL))?;
                    }
                    let spec = symmetry_spec(g, n).unwrap();
                    let r = spec.radial.clone().unwrap();
                    let base = base_plug(g, n)?;
                    let mut m = base_mesh(g, &base);
                    let tol = if a.float("tolerance", 0.0) > 0.0 { a.float("tolerance", 0.0) } else { symmetry_tolerance(&m) };
                    let wedge = if a.int("wedge", -1) >= 0 { a.int("wedge", -1) as i32 } else { keep_wedge(g, n, &m, &r, tol) };
                    if wedge >= r.order() {
                        fail!("symmetrizeRadial: {} has wedges 0 to {}", components::display_path(g, n), r.order() - 1);
                    }
                    symmetrize_radial_topology(&mut m, &r, wedge, tol);
                    if m.num_faces() == 0 {
                        fail!("symmetrizeRadial: {} has nothing in wedge {wedge}", components::display_path(g, n));
                    }
                    out.insert(components::display_path(g, n), json!(m.num_faces()));
                    set_base(g, base, m)?;
                }
                components::clear(g);
                Ok(Value::Object(out))
            })
        },
    )
    .positional("objects");
    for f in radial_flags() {
        radial = radial.flag(f);
    }
    radial = radial
        .flag(
            flag(
                "wedge",
                T::Int,
                json!(-1),
                "The wedge that is kept; -1: the one most of the selected components are in, else the one holding most of \
                 the mesh.",
            )
            .min(-1.0),
        )
        .flag(flag("tolerance", T::Float, json!(0.0), "Weld distance along the seams and the axis, cm (0: scaled to the mesh).").min(0.0));
    c.register(radial);
}

fn symmetry_rebuild(s: &mut Session, a: &Args) -> CmdResult {
    let side = a.string("side", "auto");
    if !["auto", "positive", "negative"].contains(&side.as_str()) {
        fail!("symmetryRebuild: side must be auto, positive or negative");
    }
    let axis_flag = a.string("axis", "");
    if !axis_flag.is_empty() && (axis_flag.len() != 1 || !(b'x'..=b'z').contains(&axis_flag.as_bytes()[0])) {
        fail!("symmetryRebuild: axis must be x, y or z");
    }
    let pairs_only = a.bool("pairsOnly") || a.bool("topological");
    let names = symmetry_targets(s, a, "symmetryRebuild")?;
    s.graph.evaluate();
    chunked(&mut s.graph, "Rebuild Symmetry", |g| {
        let mut out = Map::new();
        let mut rebuilt = false;
        for name in names {
            let n = if pairs_only && axis_flag.is_empty() {
                symmetric_mesh(g, &name, "symmetryRebuild")?
            } else {
                mesh_arg(g, &name, "symmetryRebuild")?
            };
            let spec = symmetry_spec(g, n);
            let radial = if axis_flag.is_empty() { spec.as_ref().and_then(|s| s.radial.clone()) } else { None };
            let axis = if !axis_flag.is_empty() { (axis_flag.as_bytes()[0] - b'x') as i32 } else { symmetry_axis(g, n).max(0) };
            let base = base_plug(g, n)?;
            let mut m = base_mesh(g, &base);
            let tol = if a.float("tolerance", 0.0) > 0.0 { a.float("tolerance", 0.0) } else { symmetry_tolerance(&m) };
            let path = components::display_path(g, n);
            let selected_edge = |g: &Graph, m: &Mesh| -> CmdResult<Index> {
                let from = components::mode(g);
                let sel = if from == ComponentMode::Object {
                    Vec::new()
                } else {
                    components::convert(m, from, components::selected(g, n, from), ComponentMode::Edge, false)
                };
                sel.first().copied().ok_or_else(|| {
                    CommandError::new(if radial.is_some() {
                        format!("symmetryRebuild: select an edge of {path}")
                    } else {
                        format!("symmetryRebuild: select an edge on the symmetry plane of {path}")
                    })
                })
            };
            let offset = symmetry_offset(g, n, axis);
            let map = if !pairs_only && radial.is_some() {
                let r = radial.as_ref().unwrap();
                let wedge = if a.int("wedge", -1) >= 0 { a.int("wedge", -1) as i32 } else { keep_wedge(g, n, &m, r, tol) };
                let map = symmetrize_radial_topology(&mut m, r, wedge, tol);
                if m.num_faces() == 0 {
                    fail!("symmetryRebuild: {path} has nothing in wedge {wedge}");
                }
                rebuilt = true;
                map
            } else if !pairs_only {
                let keep = match side.as_str() {
                    "positive" => 1,
                    "negative" => -1,
                    _ => keep_side(g, n, &m, axis, offset, tol),
                };
                let map = symmetrize_topology(&mut m, axis, keep, tol, offset);
                if m.num_faces() == 0 {
                    fail!(
                        "symmetryRebuild: {path} has nothing on the {} side of the {} plane",
                        if keep > 0 { "+" } else { "-" },
                        (b'X' + axis as u8) as char
                    );
                }
                rebuilt = true;
                map
            } else if a.bool("topological") {
                let e = selected_edge(g, &m)?;
                let map = match &radial {
                    Some(r) => SymmetryMap::build_radial_topological(&m, e, r),
                    None => SymmetryMap::build_topological(&m, e, &axis_plane(axis, offset)),
                };
                if map.is_empty() {
                    fail!("symmetryRebuild: the topology of {path} is not symmetric about that edge");
                }
                store_map(&map, &mut m, axis);
                map
            } else {
                let map = match &radial {
                    Some(r) => SymmetryMap::build_radial(&m, r, tol),
                    None => SymmetryMap::build(&m, &axis_plane(axis, offset), tol),
                };
                store_map(&map, &mut m, axis);
                map
            };
            out.insert(path, json!(map.unmatched_count()));
            set_base(g, base, m)?;
        }
        if rebuilt {
            components::clear(g);
        }
        Ok(Value::Object(out))
    })
}

/// Writes a rebuilt map into the mesh: a mirror into axis's layer, radial into its own.
fn store_map(map: &SymmetryMap, m: &mut Mesh, axis: i32) {
    if map.is_radial() {
        map.store(m);
    } else {
        map.store_axis(m, axis);
    }
}
