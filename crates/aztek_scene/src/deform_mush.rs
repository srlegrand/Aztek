//! deltaMush and tension: deformers that smooth what comes in while keeping what the rest pose had.
//!
//! deltaMush smooths the rest mesh and the deformed mesh alike. What smoothing took off the rest pose, measured in a frame
//! on the smoothed surface, goes back on the smoothed deformed mesh. tension pulls each edge back to its rest length.

use std::any::Any;
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::{DMat3, DVec3};
use rayon::prelude::*;
use serde_json::json;

use crate::attr_defs::{bool_attr, float_attr_short};
use crate::deform::{self, data_attr, f, flat, floats_of, input_mesh, insert_at, int_attr, set_floats, unflat, weight_at, Cmd, Place, BASE_COUNT, ENVELOPE, IN, OUT, WEIGHTS};
use crate::graph::Graph;
use crate::node::{EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{AttrDef, NodeId, PlugValue};
use crate::session::Session;
use crate::skin::modes;

const ITERATIONS: usize = BASE_COUNT;
const STEP: usize = BASE_COUNT + 1;
const PIN: usize = BASE_COUNT + 2;
const INWARD: usize = BASE_COUNT + 3;
const OUTWARD: usize = BASE_COUNT + 4;
const REST: usize = BASE_COUNT + 5;
/// deltaMush's displacement, tension's squashConstraint.
const EXTRA_A: usize = BASE_COUNT + 6;
/// tension's stretchConstraint.
const EXTRA_B: usize = BASE_COUNT + 7;

/// A mesh's connectivity, kept while its topology stays the same.
#[derive(Default)]
struct Topo {
    id: u64,
    neighbours: Vec<Vec<u32>>,
    faces: Vec<Vec<u32>>,
    border: Vec<bool>,
}

impl Topo {
    fn of(m: &Mesh) -> Topo {
        let n = m.point_capacity();
        let mut neighbours = vec![Vec::new(); n as usize];
        let mut border = vec![false; n as usize];
        for p in 0..n {
            if m.point_valid(p) {
                m.for_each_point_edge(p, |e| neighbours[p as usize].push(m.edge_other_point(e, p)));
                border[p as usize] = m.is_boundary_point(p);
            }
        }
        let faces = (0..m.face_capacity()).filter(|&x| m.face_valid(x)).map(|x| m.face_points(x)).collect();
        Topo { id: m.topology_id(), neighbours, faces, border }
    }

    fn refresh(&mut self, m: &Mesh) {
        if self.id != m.topology_id() || self.neighbours.len() != m.point_capacity() as usize {
            *self = Topo::of(m);
        }
    }

    /// Laplacian smoothing, step towards the neighbours' middle, border points held when pinned.
    fn smooth(&self, points: &[DVec3], iterations: i64, step: f64, pin: bool) -> Vec<DVec3> {
        let mut now = points.to_vec();
        for _ in 0..iterations.max(0) {
            now = (0..now.len())
                .into_par_iter()
                .map(|p| {
                    let around = &self.neighbours[p];
                    if around.is_empty() || (pin && self.border[p]) {
                        return now[p];
                    }
                    let mid = around.iter().fold(DVec3::ZERO, |s, &q| s + now[q as usize]) / around.len() as f64;
                    now[p] + (mid - now[p]) * step
                })
                .collect();
        }
        now
    }

    /// Area-weighted normals of these positions.
    fn normals(&self, points: &[DVec3]) -> Vec<DVec3> {
        let mut out = vec![DVec3::ZERO; points.len()];
        for face in &self.faces {
            let mut n = DVec3::ZERO;
            for i in 0..face.len() {
                n += points[face[i] as usize].cross(points[face[(i + 1) % face.len()] as usize]);
            }
            for &p in face {
                out[p as usize] += n;
            }
        }
        out.iter().map(|n| n.normalize_or_zero()).collect()
    }

    /// A frame at each point: the normal, and a tangent towards the first neighbour.
    fn frames(&self, points: &[DVec3]) -> Vec<DMat3> {
        let normals = self.normals(points);
        (0..points.len())
            .map(|p| {
                let n = if normals[p] == DVec3::ZERO { DVec3::Y } else { normals[p] };
                let along = self.neighbours[p].first().map_or(DVec3::ZERO, |&q| points[q as usize] - points[p]);
                let mut t = (along - n * along.dot(n)).normalize_or_zero();
                if t == DVec3::ZERO {
                    t = n.any_orthonormal_vector();
                }
                DMat3::from_cols(t, n.cross(t), n)
            })
            .collect()
    }
}

/// Holds back moves along the normal: inward and outward from 0 (free) to 1 (none).
fn constrain(offset: DVec3, normal: DVec3, inward: f64, outward: f64) -> DVec3 {
    if inward == 0.0 && outward == 0.0 {
        return offset;
    }
    let along = offset.dot(normal);
    offset - normal * along * if along < 0.0 { inward } else { outward }
}

fn pass_through(d: &mut NodeData, input: Arc<Mesh>) {
    d.set_output(OUT, PlugValue::Mesh(Some(input)));
}

#[derive(Default)]
struct DeltaMush {
    topo: Topo,
    /// The rest mesh and settings the deltas came from.
    key: (Vec<f64>, i64, u64, bool),
    deltas: Vec<DVec3>,
}

impl NodeBehavior for DeltaMush {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let (iterations, step, pin) = (d.int(ITERATIONS), d.float(STEP), d.bool(PIN));
        let rest = floats_of(d, REST).map(|r| r.0.clone()).unwrap_or_default();
        if rest.len() != input.point_capacity() as usize * 3 {
            return pass_through(d, input);
        }
        self.topo.refresh(&input);
        let key = (rest, iterations, step.to_bits(), pin);
        if key != self.key || self.deltas.len() != input.point_capacity() as usize {
            let rest_points = unflat(&key.0);
            let smooth = self.topo.smooth(&rest_points, iterations, step, pin);
            let frames = self.topo.frames(&smooth);
            self.deltas = (0..rest_points.len()).map(|p| frames[p].transpose() * (rest_points[p] - smooth[p])).collect();
            self.key = key;
        }
        let smooth = self.topo.smooth(input.positions(), iterations, step, pin);
        let frames = self.topo.frames(&smooth);
        let normals = self.topo.normals(input.positions());
        let (env, displacement, inward, outward) = (d.float(ENVELOPE), d.float(EXTRA_A), d.float(INWARD), d.float(OUTWARD));
        let weights = floats_of(d, WEIGHTS);
        let mut m = (*input).clone();
        for p in 0..m.point_capacity() {
            let w = weight_at(weights, p, env);
            if w == 0.0 || !m.point_valid(p) {
                continue;
            }
            let i = p as usize;
            let at = m.position(p);
            let target = smooth[i] + frames[i] * (self.deltas[i] * displacement);
            m.set_position(p, at + constrain(target - at, normals[i], inward, outward) * w);
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[derive(Default)]
struct Tension {
    topo: Topo,
}

impl NodeBehavior for Tension {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let rest = floats_of(d, REST).map(|r| unflat(&r.0)).unwrap_or_default();
        if rest.len() != input.point_capacity() as usize {
            return pass_through(d, input);
        }
        self.topo.refresh(&input);
        let topo = &self.topo;
        let (iterations, step, pin) = (d.int(ITERATIONS), d.float(STEP), d.bool(PIN));
        let (squash, stretch) = (d.float(EXTRA_A), d.float(EXTRA_B));
        let mut now = input.positions().to_vec();
        for _ in 0..iterations.max(0) {
            now = (0..now.len())
                .into_par_iter()
                .map(|p| {
                    let around = &topo.neighbours[p];
                    if around.is_empty() || (pin && topo.border[p]) {
                        return now[p];
                    }
                    // Each edge asks for the place that gives it its rest length; squashed and stretched edges ask less as
                    // their constraints near 1.
                    let mut pull = DVec3::ZERO;
                    for &q in around {
                        let q = q as usize;
                        let v = now[p] - now[q];
                        let (len, wanted) = (v.length(), (rest[p] - rest[q]).length());
                        if len > 1e-12 {
                            let hold = if len < wanted { squash } else { stretch };
                            pull += v / len * (wanted - len) * (1.0 - hold);
                        }
                    }
                    now[p] + pull / around.len() as f64 * step
                })
                .collect();
        }
        let normals = topo.normals(input.positions());
        let (env, inward, outward) = (d.float(ENVELOPE), d.float(INWARD), d.float(OUTWARD));
        let weights = floats_of(d, WEIGHTS);
        let mut m = (*input).clone();
        for p in 0..m.point_capacity() {
            let w = weight_at(weights, p, env);
            if w == 0.0 || !m.point_valid(p) {
                continue;
            }
            let at = m.position(p);
            m.set_position(p, at + constrain(now[p as usize] - at, normals[p as usize], inward, outward) * w);
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn settings(extra: Vec<AttrDef>) -> Vec<AttrDef> {
    let mut v = vec![
        int_attr("smoothingIterations", "si", 10, "Smoothing passes.", Some(0.0)),
        float_attr_short("smoothingStep", "ss", 0.5, "How far each pass moves a point towards its neighbours.", Some(0.0), Some(1.0)),
        AttrDef { short_name: "pbv".into(), ..bool_attr("pinBorderVertices", true, "Border points stay out of the smoothing.") },
        float_attr_short("inwardConstraint", "ic", 0.0, "Stops points moving in along the normal: 0 free, 1 not at all.", Some(0.0), Some(1.0)),
        float_attr_short("outwardConstraint", "oc", 0.0, "Stops points moving out along the normal: 0 free, 1 not at all.", Some(0.0), Some(1.0)),
        data_attr("restPoints", "rp", "The mesh's points when the deformer was made."),
    ];
    v.extend(extra);
    v
}

pub(crate) fn register_types(gf: Arc<NodeType>) {
    crate::node::types::register_type(NodeType {
        name: "deltaMush".into(),
        base: Some(gf.clone()),
        icon: "ripple".into(),
        doc: "Smooths a deformation while keeping the rest pose's detail.".into(),
        own_attrs: settings(vec![float_attr_short("displacement", "dis", 1.0, "How much of the rest pose's detail goes back on.", None, None)]),
        create: Some(Arc::new(|| Box::<DeltaMush>::default())),
        ..Default::default()
    })
    .expect("deltaMush");
    crate::node::types::register_type(NodeType {
        name: "tension".into(),
        base: Some(gf),
        icon: "ripple".into(),
        doc: "Pulls edges back towards their rest lengths.".into(),
        own_attrs: settings(vec![
            float_attr_short("squashConstraint", "sqc", 0.0, "Stops squashed edges relaxing: 0 free, 1 not at all.", Some(0.0), Some(1.0)),
            float_attr_short("stretchConstraint", "stc", 0.0, "Stops stretched edges relaxing: 0 free, 1 not at all.", Some(0.0), Some(1.0)),
        ]),
        create: Some(Arc::new(|| Box::<Tension>::default())),
        ..Default::default()
    })
    .expect("tension");
}

/// The mesh as it reaches the node now becomes the rest pose.
pub(crate) fn capture_rest(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    let Some(input) = input_mesh(g, n) else { return Ok(()) };
    set_floats(g, n, REST, flat(input.positions()))
}

const MUSH_SETTINGS: &[&str] = &["smoothingIterations", "smoothingStep", "pinBorderVertices", "inwardConstraint", "outwardConstraint", "displacement", "envelope"];
const TENSION_SETTINGS: &[&str] = &["smoothingIterations", "smoothingStep", "pinBorderVertices", "inwardConstraint", "outwardConstraint", "squashConstraint", "stretchConstraint", "envelope"];

fn run(s: &mut Session, a: &Args, ty: &str, label: &str, list: &[&str]) -> CmdResult {
    if a.bool("query") || a.bool("edit") {
        let Some(name) = a.named().into_iter().next() else { fail!("{ty}: name a {ty}") };
        let n = crate::deform_cmds::deformer_named(&s.graph, &name, ty)?;
        if a.bool("query") {
            return deform::query_flags(&mut s.graph, n, a, list);
        }
        deform::apply_flags(&mut s.graph, n, a, list)?;
        return Ok(json!([s.graph.path(n)]));
    }
    let meshes = deform::targets(s, a)?;
    if meshes.is_empty() {
        fail!("{ty}: select meshes");
    }
    let g = &mut s.graph;
    let name = a.string("name", &format!("{ty}1"));
    g.open_chunk(label);
    let r = (|| -> CmdResult<NodeId> {
        let n = insert_at(g, meshes[0].0, ty, &name, Place::from_args(a))?;
        deform::apply_flags(g, n, a, list)?;
        if a.has("pin") {
            g.set_value(deform::plug(n, PIN), PlugValue::Bool(a.bool("pin")))?;
        }
        capture_rest(g, n)?;
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

fn shared_flags(c: Cmd) -> Cmd {
    modes(deform::common_flags(c))
        .positional("meshes or vertices (the selection when none); the deformer with -e and -q")
        .flag(f("smoothingIterations", "si", ArgType::Int, "Smoothing passes (10)."))
        .flag(f("smoothingStep", "ss", ArgType::Float, "How far each pass moves a point towards its neighbours (0.5)."))
        .flag(f("pinBorderVertices", "pbv", ArgType::Bool, "Border points stay out of the smoothing (on)."))
        .flag(f("pin", "pin", ArgType::Bool, "The same as -pinBorderVertices."))
        .flag(f("inwardConstraint", "ic", ArgType::Float, "Stops points moving in along the normal: 0 free, 1 not at all."))
        .flag(f("outwardConstraint", "oc", ArgType::Float, "Stops points moving out along the normal: 0 free, 1 not at all."))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        shared_flags(Cmd::new(
            "deltaMush",
            "Add a delta mush after the mesh's other deformers: deltaMush -si 10 -ss 0.5. The mesh as it is now is the rest pose. Returns \
             [deltaMush1].",
            |s, a| run(s, a, "deltaMush", "Delta Mush", MUSH_SETTINGS),
        ))
        .flag(f("displacement", "dis", ArgType::Float, "How much of the rest pose's detail goes back on (1).")),
    );
    c.register(
        shared_flags(Cmd::new(
            "tension",
            "Add a tension deformer: edges pull back towards their lengths in the mesh as it is now. Returns [tension1].",
            |s, a| run(s, a, "tension", "Tension", TENSION_SETTINGS),
        ))
        .flag(f("squashConstraint", "sqc", ArgType::Float, "Stops squashed edges relaxing: 0 free, 1 not at all."))
        .flag(f("stretchConstraint", "stc", ArgType::Float, "Stops stretched edges relaxing: 0 free, 1 not at all.")),
    );
}
