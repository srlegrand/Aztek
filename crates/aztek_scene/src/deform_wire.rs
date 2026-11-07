//! wire: points near a curve follow it. The base wire is the curve as it was when added, held as a polyline in world
//! space; each point finds its nearest place on that and goes with the same place on the curve as it is now.
//!
//! A deformer's compute can't reach into the scene, and the driver may be any shape, so the wire works from polylines
//! sampled by `driver_points`: an evaluate hook samples each driver before the graph evaluates and touches the node when the
//! result changed. A driver is anything with points, by way of `components::point_provider` (or a mesh, in point order).

use std::any::Any;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use aztek_ops::{fail, objects, ArgType, Args, CmdResult, Commands};
use glam::{DMat4, DQuat, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::float_attr_short;
use crate::components;
use crate::deform::{self, data_attr, f, flat, floats_of, input_mesh, insert_at, plug, set_floats, unflat, weight_at, Cmd, Floats, Place, BASE_COUNT, ENVELOPE, GEOM, IN, OUT, WEIGHTS};
use crate::graph::Graph;
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, NodeId, PlugRef, PlugType, PlugValue};
use crate::rig::world;
use crate::session::Session;
use crate::skin::{asked, modes};
use crate::transform::Transform;

const DROPOFF: usize = BASE_COUNT;
const ROTATION: usize = BASE_COUNT + 1;
const CROSSING: usize = BASE_COUNT + 2;
const TENSION: usize = BASE_COUNT + 3;
const LOCAL_INFLUENCE: usize = BASE_COUNT + 4;
/// One element per wire, connected from the driver's transform: which node drives it, and it dirties the wire when that moves.
const WIRE_MATRIX: usize = BASE_COUNT + 5;
const BASE_WIRE: usize = BASE_COUNT + 6;
/// One element per wire whose rest curve is a shape in the scene: which shape, for saving.
const BASE_WIRE_MATRIX: usize = BASE_COUNT + 7;

/// Wire nodes ever made in this process: scenes without any skip the hook's walk over the nodes.
static WIRES_MADE: AtomicUsize = AtomicUsize::new(0);

/// A node's points in world space, in order: whatever a wire's driver offers as a polyline.
pub fn driver_points(g: &mut Graph, node: NodeId) -> Vec<DVec3> {
    let xf = crate::dag::transform_of(g, node);
    let w = world(g, xf);
    // A curve drives along its length, not its hull.
    if let Some(c) = crate::curves::as_curve(g, node) {
        return crate::curves::world_curve(g, c).map(|k| k.tessellate(8)).unwrap_or_default();
    }
    if let Some(pn) = components::point_node(g, node) {
        let Some(points) = g.node(pn).and_then(components::point_provider).and_then(|pp| pp.points) else { return Vec::new() };
        let mut pts = points(g, pn);
        pts.sort_by_key(|(id, _)| *id);
        return pts.into_iter().map(|(_, p)| w.transform_point3(p)).collect();
    }
    let Some(mesh) = components::as_mesh(g, node) else { return Vec::new() };
    let Some(m) = crate::modifiers::cage(g, mesh) else { return Vec::new() };
    (0..m.point_capacity()).filter(|&p| m.point_valid(p)).map(|p| w.transform_point3(m.position(p))).collect()
}

/// The base wires, as stored: how many, each one's length, then all their points.
fn pack(wires: &[Vec<DVec3>]) -> Vec<f64> {
    let mut out = vec![wires.len() as f64];
    out.extend(wires.iter().map(|w| w.len() as f64));
    for w in wires {
        out.extend(flat(w));
    }
    out
}

fn unpack(data: &Floats) -> Vec<Vec<DVec3>> {
    let v = &data.0;
    let Some(&count) = v.first() else { return Vec::new() };
    let count = count as usize;
    let mut at = 1 + count;
    let mut out = Vec::with_capacity(count);
    for k in 0..count {
        let n = v.get(1 + k).copied().unwrap_or(0.0) as usize;
        let Some(part) = v.get(at..at + n * 3) else { break };
        out.push(unflat(part));
        at += n * 3;
    }
    out
}

/// The nearest place on a polyline: segment, how far along it, distance.
fn nearest_on(line: &[DVec3], q: DVec3) -> (usize, f64, f64) {
    if line.len() < 2 {
        return (0, 0.0, line.first().map_or(f64::MAX, |p| (*p - q).length()));
    }
    let mut best = (0, 0.0, f64::MAX);
    for i in 0..line.len() - 1 {
        let (a, b) = (line[i], line[i + 1]);
        let ab = b - a;
        let t = ((q - a).dot(ab) / ab.length_squared().max(1e-18)).clamp(0.0, 1.0);
        let d = (a + ab * t - q).length();
        if d < best.2 {
            best = (i, t, d);
        }
    }
    best
}

fn at_place(line: &[DVec3], seg: usize, t: f64) -> DVec3 {
    match (line.get(seg), line.get(seg + 1)) {
        (Some(a), Some(b)) => *a + (*b - *a) * t,
        (Some(a), None) => *a,
        _ => DVec3::ZERO,
    }
}

fn tangent(line: &[DVec3], seg: usize) -> DVec3 {
    match (line.get(seg), line.get(seg + 1)) {
        (Some(a), Some(b)) => (*b - *a).normalize_or_zero(),
        _ => DVec3::ZERO,
    }
}

#[derive(Default)]
struct Wire {
    /// Each driver's polyline now, by element.
    live: Mutex<Vec<(i32, Vec<DVec3>)>>,
    /// The points and base wires the nearest places were worked out for.
    key: (Vec<DVec3>, Vec<f64>),
    nearest: Vec<Vec<(usize, f64, f64)>>,
}

impl NodeBehavior for Wire {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let live = self.live.lock().unwrap().clone();
        let Some(base_data) = floats_of(d, BASE_WIRE).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let base = unpack(&base_data);
        if live.is_empty() || base.is_empty() {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        }
        let geom = d.matrix(GEOM);
        let gi = geom.inverse();
        let points: Vec<DVec3> = input.positions().iter().map(|p| geom.transform_point3(*p)).collect();
        if self.key.0 != points || self.key.1 != base_data.0 {
            self.nearest = base.iter().map(|line| points.iter().map(|q| nearest_on(line, *q)).collect()).collect();
            self.key = (points.clone(), base_data.0.clone());
        }
        let (rotation, crossing, tension, local) = (d.float(ROTATION), d.float(CROSSING), d.float(TENSION), d.float(LOCAL_INFLUENCE));
        let env = d.float(ENVELOPE);
        let weights = floats_of(d, WEIGHTS);
        let mut m = (*input).clone();
        for p in 0..m.point_capacity() {
            let w = weight_at(weights, p, env);
            if w == 0.0 || !m.point_valid(p) {
                continue;
            }
            let q = points[p as usize];
            let (mut added, mut blended, mut total) = (DVec3::ZERO, DVec3::ZERO, 0.0);
            for (i, now) in live.iter() {
                let k = *i as usize;
                let Some(line) = base.get(k) else { continue };
                // A driver that changed its point count no longer lines up with its base.
                if line.len() != now.len() {
                    continue;
                }
                let (seg, t, dist) = self.nearest[k][p as usize];
                let reach = d.element(DROPOFF, *i).map_or(1.0, |v| v.as_float());
                if dist >= reach {
                    continue;
                }
                let s = 1.0 - dist / reach.max(1e-12);
                let fall = (s * s * (3.0 - 2.0 * s)).powf(tension);
                let (from, to) = (at_place(line, seg, t), at_place(now, seg, t));
                let (t0, t1) = (tangent(line, seg), tangent(now, seg));
                let turn = if t0 != DVec3::ZERO && t1 != DVec3::ZERO { DQuat::IDENTITY.slerp(DQuat::from_rotation_arc(t0, t1), rotation) } else { DQuat::IDENTITY };
                let raw = to + turn * (q - from) - q;
                let delta = raw * fall;
                added += delta;
                let share = fall * ((1.0 - local) * raw.length() + local);
                blended += delta * share;
                total += share;
            }
            if total > 0.0 || added != DVec3::ZERO {
                let convolved = if total > 0.0 { blended / total } else { DVec3::ZERO };
                let moved = convolved + (added - convolved) * crossing;
                let at = m.position(p);
                m.set_position(p, at + gi.transform_vector3(moved) * w);
            }
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_types(gf: Arc<NodeType>) {
    types::register_type(NodeType {
        name: "wire".into(),
        base: Some(gf),
        icon: "line".into(),
        doc: "Points near a curve follow it.".into(),
        own_attrs: vec![
            AttrDef { name: "dropoffDistance".into(), short_name: "dds".into(), ty: PlugType::Float, default_value: PlugValue::Float(1.0), flags: flags::INPUT | flags::STORABLE | flags::ARRAY, doc: "How far each wire reaches, cm.".into(), ..Default::default() },
            float_attr_short("rotation", "rt", 1.0, "How much points turn with the wire: 0 not at all, 1 fully.", Some(0.0), Some(1.0)),
            float_attr_short("crossingEffect", "ce", 0.0, "Where wires cross: 0 they blend, 1 they add up.", Some(0.0), Some(1.0)),
            float_attr_short("tension", "tn", 1.0, "How tightly the effect gathers to the wire.", Some(0.0), None),
            float_attr_short("localInfluence", "li", 0.0, "Where wires cross: how much a still wire holds its points, 0 to 1.", Some(0.0), Some(1.0)),
            AttrDef { name: "wireMatrix".into(), short_name: "wm".into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: flags::INPUT | flags::HIDDEN | flags::ARRAY, doc: "Each wire's world matrix.".into(), ..Default::default() },
            data_attr("baseWire", "bw", "The wires as they were when added, world space."),
            AttrDef { name: "baseWireMatrix".into(), short_name: "bwm".into(), ty: PlugType::Matrix, default_value: PlugValue::Matrix(DMat4::IDENTITY), flags: flags::INPUT | flags::HIDDEN | flags::ARRAY, doc: "Each wire's rest curve, when it is a shape in the scene.".into(), ..Default::default() },
        ],
        create: Some(Arc::new(|| {
            WIRES_MADE.fetch_add(1, Ordering::Relaxed);
            Box::<Wire>::default()
        })),
        ..Default::default()
    })
    .expect("wire");
}

/// Samples each wire's drivers before the graph evaluates.
pub fn install(g: &mut Graph) {
    g.add_evaluate_hook(Arc::new(|g: &mut Graph, after: bool| {
        if after || WIRES_MADE.load(Ordering::Relaxed) == 0 {
            return;
        }
        let wires: Vec<NodeId> = g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().is_a("wire"))).collect();
        for w in wires {
            sample(g, w);
        }
    }));
}

fn sample(g: &mut Graph, w: NodeId) {
    let drivers: Vec<(i32, NodeId)> = g.elements(&plug(w, WIRE_MATRIX)).into_iter().filter_map(|e| g.source(&e).map(|s| (e.element, s.node))).collect();
    let now: Vec<(i32, Vec<DVec3>)> = drivers.into_iter().map(|(i, d)| (i, driver_points(g, d))).collect();
    let Some(wire) = g.node(w).and_then(|n| n.behavior().as_any().downcast_ref::<Wire>()) else { return };
    let mut live = wire.live.lock().unwrap();
    if *live != now {
        *live = now;
        drop(live);
        g.touch(w);
    }
}

fn add_wire(g: &mut Graph, n: NodeId, name: &str) -> CmdResult<()> {
    let id = crate::rig::find(g, name, "wire")?;
    let xf = crate::dag::transform_of(g, id);
    if g.elements(&plug(n, WIRE_MATRIX)).iter().any(|e| g.source(e).is_some_and(|s| s.node == xf)) {
        return Ok(());
    }
    g.evaluate();
    let line = driver_points(g, xf);
    if line.is_empty() {
        fail!("wire: {name} has no points to follow");
    }
    // A wire's element index is its place in the base list.
    let mut base = floats_of(g.node(n).unwrap().data(), BASE_WIRE).map(unpack).unwrap_or_default();
    let index = base.len() as i32;
    g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(n, WIRE_MATRIX as i32, index), false)?;
    g.set_value(PlugRef::element(n, DROPOFF as i32, index), PlugValue::Float(1.0))?;
    base.push(line);
    set_floats(g, n, BASE_WIRE, pack(&base))
}

/// One wire from a file: the curve that drives and the curve holding where it rested, as the reference keeps them. Keeps a dropoff the
/// file already set.
pub(crate) fn add_file_wire(g: &mut Graph, n: NodeId, index: i32, driver: NodeId, base_shape: Option<NodeId>) -> CmdResult<()> {
    g.evaluate();
    let xf = crate::dag::transform_of(g, driver);
    g.connect(&plug(xf, Transform::WORLD_MATRIX), &PlugRef::element(n, WIRE_MATRIX as i32, index), false)?;
    if g.node(n).unwrap().element(DROPOFF, index).is_none() {
        g.set_value(PlugRef::element(n, DROPOFF as i32, index), PlugValue::Float(1.0))?;
    }
    let mut base = floats_of(g.node(n).unwrap().data(), BASE_WIRE).map(unpack).unwrap_or_default();
    if base.len() <= index as usize {
        base.resize(index as usize + 1, Vec::new());
    }
    match base_shape {
        Some(shape) => {
            let line = driver_points(g, shape);
            if line.is_empty() {
                fail!("wire: the rest curve has no points");
            }
            base[index as usize] = line;
            // Kept so saving can name it again.
            let at = PlugRef::element(n, BASE_WIRE_MATRIX as i32, index);
            g.connect(&plug(crate::dag::transform_of(g, shape), Transform::WORLD_MATRIX), &at, false)?;
        }
        // A file of ours may name no rest curve: its state holds the line. With neither, it rests where it is now.
        None if base[index as usize].is_empty() => base[index as usize] = driver_points(g, driver),
        None => {}
    }
    set_floats(g, n, BASE_WIRE, pack(&base))
}

/// -dds index distance, any number of pairs.
fn set_dropoffs(g: &mut Graph, n: NodeId, a: &Args) -> CmdResult<()> {
    fn numbers(v: &Value, out: &mut Vec<f64>) {
        match v {
            Value::Array(a) => a.iter().for_each(|x| numbers(x, out)),
            v => out.extend(v.as_f64()),
        }
    }
    let Some(v) = a.get("dropoffDistance") else { return Ok(()) };
    let mut flat = Vec::new();
    numbers(v, &mut flat);
    for pair in flat.chunks_exact(2) {
        g.set_value(PlugRef::element(n, DROPOFF as i32, pair[0] as i32), PlugValue::Float(pair[1]))?;
    }
    Ok(())
}

fn wire_names(a: &Args) -> Vec<String> {
    match a.get("wire") {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(v)) => v.iter().filter_map(|x| x.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

const SETTINGS: &[&str] = &["rotation", "crossingEffect", "tension", "localInfluence", "envelope"];

fn create(s: &mut Session, a: &Args) -> CmdResult {
    let mut names = objects(s, a);
    let mut wires = wire_names(a);
    // Without -w, the wire is named last, as the selection for the menu item has it.
    if wires.is_empty() && names.len() > 1 {
        wires.push(names.pop().unwrap());
    }
    let g = &mut s.graph;
    let meshes = deform::named_targets(g, &names)?;
    if meshes.is_empty() {
        fail!("wire: select the meshes to deform, then the wire");
    }
    let name = a.string("name", "wire1");
    g.open_chunk("Wire");
    let r = (|| -> CmdResult<NodeId> {
        let n = insert_at(g, meshes[0].0, "wire", &name, Place::from_args(a))?;
        deform::apply_flags(g, n, a, SETTINGS)?;
        for w in &wires {
            add_wire(g, n, w)?;
        }
        set_dropoffs(g, n, a)?;
        if let Some(input) = input_mesh(g, n) {
            deform::limit_to(g, n, input.point_capacity(), &meshes[0].1)?;
        }
        for (m, pts) in &meshes[1..] {
            deform::add_geometry(g, n, *m, pts)?;
        }
        Ok(n)
    })();
    g.close_chunk();
    let n = r?;
    Ok(json!([g.path(n)]))
}

fn wire_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let n = crate::deform_cmds::deformer_named(g, name, "wire")?;
    if !g.node(n).is_some_and(|x| x.ty().is_a("wire")) {
        fail!("wire: {name} is not a wire");
    }
    Ok(deform::leader_of(g, n))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        modes(deform::common_flags(Cmd::new(
            "wire",
            "Make points follow a curve: wire -w curve1 -dds 0 2.0 -en 1 -ce 0 -li 0 -n name mesh. Returns [wire1]. With -e on a wire: -w adds a \
             wire, -dds and the settings change. With -q: -w and the settings.",
            |s, a| {
                if a.bool("query") {
                    let Some(name) = a.named().into_iter().next() else { fail!("wire: name a wire to query") };
                    let n = wire_named(&s.graph, &name)?;
                    let g = &mut s.graph;
                    if asked(a, "wire") {
                        return Ok(json!(g.elements(&plug(n, WIRE_MATRIX)).iter().filter_map(|e| g.source(e)).map(|d| g.path(d.node)).collect::<Vec<_>>()));
                    }
                    if asked(a, "dropoffDistance") {
                        return Ok(json!(g.node(n).unwrap().elements(DROPOFF).iter().map(|(_, v)| v.as_float()).collect::<Vec<_>>()));
                    }
                    return deform::query_flags(g, n, a, SETTINGS);
                }
                if a.bool("edit") {
                    let Some(name) = a.named().into_iter().next() else { fail!("wire: name a wire to edit") };
                    let g = &mut s.graph;
                    let n = wire_named(g, &name)?;
                    deform::apply_flags(g, n, a, SETTINGS)?;
                    for x in deform::group(g, n) {
                        for w in wire_names(a) {
                            add_wire(g, x, &w)?;
                        }
                        set_dropoffs(g, x, a)?;
                    }
                    return Ok(json!([g.path(n)]));
                }
                create(s, a)
            },
        )))
        .positional("meshes to deform, then the wire unless -w names it (the selection when none); the wire with -e and -q")
        .flag(f("wire", "w", ArgType::Any, "The curve to follow, or several. Any shape with points will do."))
        .flag(f("dropoffDistance", "dds", ArgType::Any, "Wire index, then how far it reaches, cm (1)."))
        .flag(f("groupWithBase", "gw", ArgType::Bool, "Accepted and ignored: the base wire is held on the node, so there is nothing to group."))
        .flag(f("rotation", "rt", ArgType::Float, "How much points turn with the wire: 0 not at all, 1 fully (1)."))
        .flag(f("crossingEffect", "ce", ArgType::Float, "Where wires cross: 0 they blend, 1 they add up (0)."))
        .flag(f("tension", "tn", ArgType::Float, "How tightly the effect gathers to the wire (1)."))
        .flag(f("localInfluence", "li", ArgType::Float, "Where wires cross: how much a still wire holds its points, 0 to 1 (0).")),
    );
}
