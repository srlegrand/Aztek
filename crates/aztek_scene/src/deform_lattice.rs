//! Lattices as the reference makes them: `lattice` creates an `ffd` deformer, a `lattice` shape under ffd1Lattice and a
//! `baseLattice` under ffd1Base. Control points are ffd1Lattice.pt[s][t][u], held in the unit box of the lattice's own
//! space, -0.5 to 0.5 on each axis. A point of the mesh takes its s, t, u from where it sits in the base lattice and
//! goes where the deformed lattice has that s, t, u.

use std::any::Any;
use std::sync::Arc;

use aztek_ops::{fail, ArgType, Args, CmdResult, Commands};
use glam::{DMat4, DVec3};
use serde_json::{json, Value};

use crate::attr_defs::{bool_attr, enum_attr, float_attr_short};
use crate::components::{self, ComponentMode, PointProvider};
use crate::deform::{self, data_attr, f, flat, floats_of, input_mesh, insert_at, int_attr, matrix_in, plug, set_floats, unflat, weight_at, Cmd, Place, BASE_COUNT, ENVELOPE, GEOM, IN, OUT, WEIGHTS};
use crate::graph::{Graph, SelectMode};
use crate::node::{types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, AttrDef, Data, NodeId, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::skin::{asked, modes};
use crate::transform::{Shape, Transform};

// ---------------------------------------------------------------------------------------------------------------------
// The grid

/// Control points by id: s + t * S + u * S * T.
#[derive(Debug, Clone)]
pub struct LatticeGrid {
    pub dims: [usize; 3],
    pub pts: Vec<DVec3>,
}

impl Data for LatticeGrid {
    fn kind(&self) -> &str {
        "latticePoints"
    }
    fn to_json(&self) -> Value {
        json!({"kind": "latticePoints", "dims": self.dims, "points": flat(&self.pts)})
    }
    fn bytes(&self) -> usize {
        self.pts.len() * 24
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Blending coefficients along one axis, for a parameter from 0 to 1 over n control points. k is the local influence:
/// each point takes the Bernstein blend of the k control points around it. k of 2 is linear; k of n or more is the whole
/// row. Between, windows centred on the two nearest control points blend across the span, so the result stays continuous
/// where the window moves on. All of these keep straight rows straight, so an undeformed lattice changes nothing.
fn axis_coeffs(x: f64, n: usize, k: usize, out: &mut Vec<(usize, f64)>) {
    out.clear();
    if n < 2 {
        out.push((0, 1.0));
        return;
    }
    let at = x * (n - 1) as f64;
    let i = (at.floor() as isize).clamp(0, n as isize - 2) as usize;
    let frac = at - i as f64;
    let k = k.clamp(2, n);
    if k == 2 {
        out.push((i, 1.0 - frac));
        out.push((i + 1, frac));
        return;
    }
    let window = |j: usize| (j as isize - (k as isize - 1) / 2).clamp(0, (n - k) as isize) as usize;
    let (a, b) = (window(i), window(i + 1));
    let mut push = |start: usize, scale: f64| {
        // Bernstein basis of degree k - 1, built up a degree at a time.
        let u = (at - start as f64) / (k - 1) as f64;
        let mut basis = vec![1.0];
        for _ in 1..k {
            let mut next = vec![0.0; basis.len() + 1];
            for (m, v) in basis.iter().enumerate() {
                next[m] += v * (1.0 - u);
                next[m + 1] += v * u;
            }
            basis = next;
        }
        for (m, v) in basis.into_iter().enumerate() {
            out.push((start + m, v * scale));
        }
    };
    if a == b {
        push(a, 1.0);
    } else {
        push(a, 1.0 - frac);
        push(b, frac);
    }
}

impl LatticeGrid {
    pub fn regular(dims: [usize; 3]) -> LatticeGrid {
        let c = |i: usize, n: usize| if n < 2 { 0.0 } else { i as f64 / (n - 1) as f64 - 0.5 };
        let mut pts = Vec::with_capacity(dims[0] * dims[1] * dims[2]);
        for u in 0..dims[2] {
            for t in 0..dims[1] {
                for s in 0..dims[0] {
                    pts.push(DVec3::new(c(s, dims[0]), c(t, dims[1]), c(u, dims[2])));
                }
            }
        }
        LatticeGrid { dims, pts }
    }

    pub fn id(&self, s: usize, t: usize, u: usize) -> usize {
        s + t * self.dims[0] + u * self.dims[0] * self.dims[1]
    }

    /// The grid's position at s, t, u, each from 0 to 1.
    pub fn eval(&self, stu: DVec3, influence: [usize; 3]) -> DVec3 {
        let (mut a, mut b, mut c) = (Vec::with_capacity(4), Vec::with_capacity(4), Vec::with_capacity(4));
        axis_coeffs(stu.x, self.dims[0], influence[0], &mut a);
        axis_coeffs(stu.y, self.dims[1], influence[1], &mut b);
        axis_coeffs(stu.z, self.dims[2], influence[2], &mut c);
        let mut sum = DVec3::ZERO;
        for &(k, wk) in &c {
            for &(j, wj) in &b {
                for &(i, wi) in &a {
                    let w = wi * wj * wk;
                    if w != 0.0 {
                        sum += self.pts[self.id(i, j, k)] * w;
                    }
                }
            }
        }
        sum
    }

    /// The same shape on other divisions.
    fn resampled(&self, dims: [usize; 3]) -> LatticeGrid {
        let mut out = LatticeGrid::regular(dims);
        for p in &mut out.pts {
            *p = self.eval(*p + DVec3::splat(0.5), [2, 2, 2]);
        }
        out
    }

    fn from_json(j: &Value) -> LatticeGrid {
        let d: Vec<usize> = j.get("dims").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).map(|x| x as usize).collect()).unwrap_or_default();
        let dims = if d.len() == 3 { [d[0], d[1], d[2]] } else { [2, 2, 2] };
        let v: Vec<f64> = j.get("points").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_f64).collect()).unwrap_or_default();
        let pts = unflat(&v);
        if pts.len() == dims[0] * dims[1] * dims[2] {
            LatticeGrid { dims, pts }
        } else {
            LatticeGrid::regular(dims)
        }
    }
}

pub(crate) fn grid_of(v: &PlugValue) -> Option<&LatticeGrid> {
    match v {
        PlugValue::Data(Some(x)) => x.as_any().downcast_ref::<LatticeGrid>(),
        _ => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Nodes

const SD: usize = Shape::COUNT;
const TD: usize = Shape::COUNT + 1;
const UD: usize = Shape::COUNT + 2;
/// The moved points; none means the regular grid.
pub(crate) const CONTROL_POINTS: usize = Shape::COUNT + 3;
const LATTICE_OUTPUT: usize = Shape::COUNT + 4;

struct LatticeShape;

impl NodeBehavior for LatticeShape {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let dims = [d.int(SD).max(2) as usize, d.int(TD).max(2) as usize, d.int(UD).max(2) as usize];
        let out = match grid_of(d.value(CONTROL_POINTS)) {
            Some(g) if g.dims == dims => g.clone(),
            Some(g) => g.resampled(dims),
            None => LatticeGrid::regular(dims),
        };
        d.set_output(LATTICE_OUTPUT, PlugValue::Data(Some(Arc::new(out))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct BaseLattice;

impl NodeBehavior for BaseLattice {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

const LIS: usize = BASE_COUNT;
const LOCAL: usize = BASE_COUNT + 3;
const OUTSIDE: usize = BASE_COUNT + 4;
const OFD: usize = BASE_COUNT + 5;
const FREEZE: usize = BASE_COUNT + 6;
pub(crate) const DLP: usize = BASE_COUNT + 7;
pub(crate) const DLM: usize = BASE_COUNT + 8;
pub(crate) const BLM: usize = BASE_COUNT + 9;
const STU: usize = BASE_COUNT + 10;

struct Ffd;

impl NodeBehavior for Ffd {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let Some(input) = d.mesh(IN).cloned() else {
            d.set_output(OUT, PlugValue::Mesh(None));
            return;
        };
        let env = d.float(ENVELOPE);
        let Some(grid) = grid_of(d.value(DLP)) else {
            d.set_output(OUT, PlugValue::Mesh(Some(input)));
            return;
        };
        let local = d.bool(LOCAL);
        let influence = |axis: usize| if local { d.int(LIS + axis).max(2) as usize } else { grid.dims[axis] };
        let influence = [influence(0), influence(1), influence(2)];
        let (mode, ofd) = (d.int(OUTSIDE), d.float(OFD));
        let geom = d.matrix(GEOM);
        let geom_inv = geom.inverse();
        let base = d.matrix(BLM);
        let to_base = base.inverse() * geom;
        let from_base = geom_inv * base;
        let from_lattice = geom_inv * d.matrix(DLM);
        let frozen = if d.bool(FREEZE) { floats_of(d, STU) } else { None };
        let weights = floats_of(d, WEIGHTS);
        let mut m = (*input).clone();
        for p in 0..m.point_capacity() {
            let w = weight_at(weights, p, env);
            if w == 0.0 || !m.point_valid(p) {
                continue;
            }
            let at = m.position(p);
            let stu = match frozen.and_then(|c| c.0.get(p as usize * 3..p as usize * 3 + 3)) {
                Some(c) => DVec3::new(c[0], c[1], c[2]),
                None => to_base.transform_point3(at) + DVec3::splat(0.5),
            };
            let clamped = stu.clamp(DVec3::ZERO, DVec3::ONE);
            let beyond = stu - clamped;
            let mut scale = 1.0;
            if beyond.length_squared() > 1e-18 {
                match mode {
                    0 => continue,
                    2 => {
                        let t = beyond.length() / ofd.max(1e-9);
                        if t >= 1.0 {
                            continue;
                        }
                        scale = 1.0 - t * t * (3.0 - 2.0 * t);
                    }
                    _ => {}
                }
            }
            // Past the box a point keeps its offset from the nearest place on it.
            let moved = from_lattice.transform_point3(grid.eval(clamped, influence) + beyond);
            let rest = from_base.transform_point3(stu - DVec3::splat(0.5));
            m.set_position(p, at + (moved - rest) * (w * scale));
        }
        d.set_output(OUT, PlugValue::Mesh(Some(Arc::new(m))));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub(crate) fn register_types(gf: Arc<NodeType>) {
    crate::plug::data::register_kind("latticePoints", Arc::new(|j| Ok(Arc::new(LatticeGrid::from_json(j)) as Arc<dyn Data>)));
    let shape = types::find("shape").expect("shape");
    let out_data = AttrDef { name: "latticeOutput".into(), short_name: "lo".into(), ty: PlugType::Data, default_value: PlugValue::Data(None), flags: flags::OUTPUT | flags::HIDDEN, doc: "The control points after any change of divisions.".into(), ..Default::default() };
    types::register_type(NodeType {
        name: "lattice".into(),
        base: Some(shape.clone()),
        dag: true,
        icon: "grid-dots".into(),
        doc: "A lattice's control points: move them to deform what the ffd node holds.".into(),
        own_attrs: vec![
            int_attr("sDivisions", "sd", 2, "Control points along S.", Some(2.0)),
            int_attr("tDivisions", "td", 5, "Control points along T.", Some(2.0)),
            int_attr("uDivisions", "ud", 2, "Control points along U.", Some(2.0)),
            data_attr("controlPoints", "cp", "The moved control points in the lattice's own space."),
            out_data,
        ],
        create: Some(Arc::new(|| Box::new(LatticeShape))),
        ..Default::default()
    })
    .expect("lattice");
    types::register_type(NodeType {
        name: "baseLattice".into(),
        base: Some(shape),
        dag: true,
        icon: "grid-dots".into(),
        doc: "Where the lattice started: moving it against the lattice deforms the mesh too.".into(),
        create: Some(Arc::new(|| Box::new(BaseLattice))),
        ..Default::default()
    })
    .expect("baseLattice");
    types::register_type(NodeType {
        name: "ffd".into(),
        base: Some(gf),
        icon: "grid-dots".into(),
        doc: "A free form deformation: the mesh follows a lattice's control points.".into(),
        own_attrs: vec![
            int_attr("localInfluenceS", "lis", 2, "Control points that move a point along S.", Some(2.0)),
            int_attr("localInfluenceT", "lit", 2, "Control points that move a point along T.", Some(2.0)),
            int_attr("localInfluenceU", "liu", 2, "Control points that move a point along U.", Some(2.0)),
            AttrDef { short_name: "lo".into(), ..bool_attr("local", true, "Use the local influences. Off, every control point has a say.") },
            AttrDef { short_name: "ol".into(), ..enum_attr("outsideLattice", &["Inside", "All", "Falloff"], 0, "Which points outside the base lattice deform.") },
            float_attr_short("outsideFalloffDist", "ofd", 1.0, "With falloff: how far outside the effect dies, in lattice widths.", Some(0.0), None),
            AttrDef { short_name: "fg".into(), ..bool_attr("freezeGeometry", false, "Keep each point's s, t, u from when it was frozen.") },
            data_attr("deformedLatticePoints", "dlp", "The lattice's control points."),
            matrix_in("deformedLatticeMatrix", "dlm", "The lattice's world matrix."),
            matrix_in("baseLatticeMatrix", "blm", "The base lattice's world matrix."),
            data_attr("stuCache", "stu", "Each point's frozen s, t, u."),
        ],
        create: Some(Arc::new(|| Box::new(Ffd))),
        ..Default::default()
    })
    .expect("ffd");
    components::register_point_provider(PointProvider {
        node_type: "lattice".into(),
        points: Some(Arc::new(|g, n| match current_grid(g, n) {
            Some(grid) => grid.pts.iter().enumerate().map(|(i, p)| (i as u32, *p)).collect(),
            None => Vec::new(),
        })),
        set_points: Some(Arc::new(|g, n, moves| {
            let Some(mut grid) = current_grid(g, n) else { return };
            for &(id, at) in moves {
                if let Some(p) = grid.pts.get_mut(id as usize) {
                    *p = at;
                }
            }
            let _ = g.set_value(plug(n, CONTROL_POINTS), PlugValue::Data(Some(Arc::new(grid))));
        })),
        edit_plug: Some(Arc::new(|g, n| {
            // A drag writes this plug, so it must already hold the points.
            if let Some(grid) = current_grid(g, n) {
                g.set_value(plug(n, CONTROL_POINTS), PlugValue::Data(Some(Arc::new(grid))))?;
            }
            Ok(plug(n, CONTROL_POINTS))
        })),
        bake: Some(Arc::new(|g, n, m| {
            let Some(mut grid) = current_grid(g, n) else { return };
            for p in &mut grid.pts {
                *p = m.transform_point3(*p);
            }
            let _ = g.set_value(plug(n, CONTROL_POINTS), PlugValue::Data(Some(Arc::new(grid))));
        })),
        grid: Some(Arc::new(|g, n| {
            let Some(d) = g.node(n) else { return [2, 2, 2] };
            [d.int(SD).max(2) as u32, d.int(TD).max(2) as u32, d.int(UD).max(2) as u32]
        })),
        ..Default::default()
    });
}

pub(crate) fn current_grid(g: &mut Graph, shape: NodeId) -> Option<LatticeGrid> {
    match g.value(&plug(shape, LATTICE_OUTPUT)) {
        Ok(v) => grid_of(&v).cloned(),
        Err(_) => None,
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Drawing, for the viewport

/// A lattice's points in world space, by id, and the grid's dimensions. Reads evaluated values.
pub fn world_points(g: &Graph, shape: NodeId) -> Option<([usize; 3], Vec<DVec3>)> {
    let n = g.node(shape)?;
    let grid = grid_of(n.data().value(LATTICE_OUTPUT))?;
    let w = n.data().matrix(crate::transform::DagNode::WORLD_MATRIX);
    Some((grid.dims, grid.pts.iter().map(|p| w.transform_point3(*p)).collect()))
}

/// The lines of a lattice's grid.
pub fn grid_lines(dims: [usize; 3], pts: &[DVec3]) -> Vec<(DVec3, DVec3)> {
    let id = |s: usize, t: usize, u: usize| s + t * dims[0] + u * dims[0] * dims[1];
    let mut out = Vec::new();
    for u in 0..dims[2] {
        for t in 0..dims[1] {
            for s in 0..dims[0] {
                let at = pts[id(s, t, u)];
                if s + 1 < dims[0] {
                    out.push((at, pts[id(s + 1, t, u)]));
                }
                if t + 1 < dims[1] {
                    out.push((at, pts[id(s, t + 1, u)]));
                }
                if u + 1 < dims[2] {
                    out.push((at, pts[id(s, t, u + 1)]));
                }
            }
        }
    }
    out
}

/// The lines of a base lattice: its box, in world space.
pub fn box_lines(w: &DMat4) -> Vec<(DVec3, DVec3)> {
    let grid = LatticeGrid::regular([2, 2, 2]);
    let pts: Vec<DVec3> = grid.pts.iter().map(|p| w.transform_point3(*p)).collect();
    grid_lines([2, 2, 2], &pts)
}

// ---------------------------------------------------------------------------------------------------------------------
// The command

/// The ffd a name stands for: the node, or its lattice or base lattice.
pub(crate) fn ffd_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    let id = crate::rig::find(g, name, "lattice")?;
    if g.node(id).is_some_and(|n| n.ty().is_a("ffd")) {
        return Ok(id);
    }
    match deform::driven_deformers(g, id).into_iter().find(|&d| g.node(d).is_some_and(|n| n.ty().is_a("ffd"))) {
        Some(d) => Ok(deform::leader_of(g, d)),
        None => fail!("lattice: {name} is not a lattice deformer"),
    }
}

/// The ffd's lattice shape and the two transforms.
pub(crate) fn parts(g: &Graph, n: NodeId) -> Option<(NodeId, NodeId, NodeId)> {
    let mut shape = g.source(&plug(n, DLP))?.node;
    // A flexor's jointLattice or boneLattice sits between the lattice and the ffd.
    while g.node(shape).is_some_and(|x| crate::rig_extra::is_flexor_type(x.ty().name.as_str())) {
        shape = g.source(&plug(shape, 0))?.node;
    }
    let lat = g.source(&plug(n, DLM))?.node;
    let base = g.source(&plug(n, BLM))?.node;
    Some((shape, lat, base))
}

/// Takes each point's s, t, u from where it sits now, for freezeGeometry.
pub(crate) fn capture_stu(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    let Some(input) = input_mesh(g, n) else { return Ok(()) };
    let (geom, base) = (g.node(n).unwrap().data().matrix(GEOM), g.node(n).unwrap().data().matrix(BLM));
    let to_base = base.inverse() * geom;
    let mut stu = Vec::with_capacity(input.point_capacity() as usize * 3);
    for p in 0..input.point_capacity() {
        let c = if input.point_valid(p) { to_base.transform_point3(input.position(p)) + DVec3::splat(0.5) } else { DVec3::splat(0.5) };
        stu.extend([c.x, c.y, c.z]);
    }
    set_floats(g, n, STU, stu)
}

pub(crate) fn is_frozen(g: &Graph, n: NodeId) -> bool {
    g.node(n).is_some_and(|x| x.bool(FREEZE))
}

fn set_vec3(g: &mut Graph, n: NodeId, attr: usize, v: Option<[f64; 3]>) -> CmdResult<()> {
    if let Some(v) = v {
        g.set_value(plug(n, attr), PlugValue::Vec3(DVec3::from_array(v)))?;
    }
    Ok(())
}

fn apply_settings(g: &mut Graph, n: NodeId, a: &Args) -> CmdResult<()> {
    if let Some(l) = a.vec3("ldivisions") {
        for i in 0..3 {
            g.set_value(plug(n, LIS + i), PlugValue::Int(l[i].max(2.0) as i64))?;
        }
    }
    if a.has("outsideLattice") {
        g.set_value(plug(n, OUTSIDE), PlugValue::Int(a.int("outsideLattice", 0).clamp(0, 2)))?;
    }
    if a.has("outsideFalloffDist") {
        g.set_value(plug(n, OFD), PlugValue::Float(a.float("outsideFalloffDist", 1.0)))?;
    }
    if a.has("envelope") {
        g.set_value(plug(n, ENVELOPE), PlugValue::Float(a.float("envelope", 1.0)))?;
    }
    Ok(())
}

fn set_divisions(g: &mut Graph, shape: NodeId, d: [f64; 3]) -> CmdResult<()> {
    for i in 0..3 {
        g.set_value(plug(shape, SD + i), PlugValue::Int(d[i].max(2.0) as i64))?;
    }
    Ok(())
}

fn set_freeze(g: &mut Graph, n: NodeId, on: bool) -> CmdResult<()> {
    for x in deform::group(g, n) {
        if on {
            capture_stu(g, x)?;
        }
    }
    g.set_value(plug(n, FREEZE), PlugValue::Bool(on))?;
    Ok(())
}

fn query(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.named().into_iter().next() else { fail!("lattice: name a lattice to query") };
    let g = &mut s.graph;
    let n = ffd_named(g, &name)?;
    if asked(a, "geometry") {
        return Ok(json!(deform::geometry_names(g, n)));
    }
    if asked(a, "divisions") {
        let Some((shape, _, _)) = parts(g, n) else { fail!("lattice: {name} has no lattice") };
        let d = g.node(shape).unwrap();
        return Ok(json!([d.int(SD), d.int(TD), d.int(UD)]));
    }
    let d = g.node(n).unwrap();
    if asked(a, "ldivisions") {
        return Ok(json!([d.int(LIS), d.int(LIS + 1), d.int(LIS + 2)]));
    }
    if asked(a, "outsideLattice") {
        return Ok(json!(d.int(OUTSIDE)));
    }
    if asked(a, "outsideFalloffDist") {
        return Ok(json!(d.float(OFD)));
    }
    if asked(a, "freezeMapping") {
        return Ok(json!(d.bool(FREEZE)));
    }
    fail!("lattice: nothing to query")
}

fn edit(s: &mut Session, a: &Args) -> CmdResult {
    let Some(name) = a.named().into_iter().next() else { fail!("lattice: name a lattice to edit") };
    let g = &mut s.graph;
    let n = ffd_named(g, &name)?;
    let Some((shape, lat, base)) = parts(g, n) else { fail!("lattice: {name} has no lattice") };
    if let Some(d) = a.vec3("divisions") {
        set_divisions(g, shape, d)?;
    }
    apply_settings(g, n, a)?;
    for t in [lat, base] {
        set_vec3(g, t, Transform::TRANSLATE, a.vec3("position"))?;
        set_vec3(g, t, Transform::ROTATE, a.vec3("rotation"))?;
        set_vec3(g, t, Transform::SCALE, a.vec3("scale"))?;
    }
    if a.bool("resetLattice") || a.bool("latticeReset") || a.bool("removeTweaks") {
        g.set_value(plug(shape, CONTROL_POINTS), PlugValue::Data(None))?;
    }
    if a.has("freezeMapping") {
        set_freeze(g, n, a.bool("freezeMapping"))?;
    }
    if a.bool("remap") {
        for x in deform::group(g, n) {
            capture_stu(g, x)?;
        }
    }
    Ok(json!([g.path(n)]))
}

fn create(s: &mut Session, a: &Args) -> CmdResult {
    let meshes = deform::targets(s, a)?;
    if meshes.is_empty() {
        fail!("lattice: select meshes or vertices");
    }
    let g = &mut s.graph;
    g.evaluate();
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for (m, pts) in &meshes {
        let (l, h) = deform::world_bounds_of(g, *m, pts.as_deref());
        lo = lo.min(l);
        hi = hi.max(h);
    }
    let centred = !a.has("objectCentered") || a.bool("objectCentered");
    let (centre, size) = if centred { ((lo + hi) * 0.5, (hi - lo).max(DVec3::splat(1e-3))) } else { (DVec3::ZERO, DVec3::ONE) };
    let divisions = a.vec3("divisions").unwrap_or([2.0, 2.0, 2.0]);
    let name = a.string("name", "ffd1");
    g.open_chunk("Lattice");
    let r = (|| -> CmdResult<(NodeId, NodeId, NodeId)> {
        let (first, pts) = &meshes[0];
        let n = insert_at(g, *first, "ffd", &name, Place::from_args(a))?;
        let stem = g.node(n).unwrap().name().to_string();
        let (lat, lat_shape) = crate::dag::create_with_shape(g, "lattice", &format!("{stem}Lattice"), NO_NODE)?;
        let (base, _) = crate::dag::create_with_shape(g, "baseLattice", &format!("{stem}Base"), NO_NODE)?;
        set_divisions(g, lat_shape, divisions)?;
        for t in [lat, base] {
            set_vec3(g, t, Transform::TRANSLATE, Some(a.vec3("position").unwrap_or(centre.to_array())))?;
            set_vec3(g, t, Transform::ROTATE, a.vec3("rotation"))?;
            set_vec3(g, t, Transform::SCALE, Some(a.vec3("scale").unwrap_or(size.to_array())))?;
        }
        g.connect(&plug(lat_shape, LATTICE_OUTPUT), &plug(n, DLP), false)?;
        g.connect(&plug(lat, Transform::WORLD_MATRIX), &plug(n, DLM), false)?;
        g.connect(&plug(base, Transform::WORLD_MATRIX), &plug(n, BLM), false)?;
        apply_settings(g, n, a)?;
        if let Some(input) = input_mesh(g, n) {
            deform::limit_to(g, n, input.point_capacity(), pts)?;
        }
        for (m, pts) in &meshes[1..] {
            deform::add_geometry(g, n, *m, pts)?;
        }
        if a.bool("freezeMapping") {
            set_freeze(g, n, true)?;
        }
        components::set_mode_undoable(g, ComponentMode::Object);
        g.select(&[lat], SelectMode::Replace);
        Ok((n, lat, base))
    })();
    g.close_chunk();
    let (n, lat, base) = r?;
    Ok(json!([g.path(n), g.path(lat), g.path(base)]))
}

pub(crate) fn register(c: &mut Commands<Session>) {
    c.register(
        modes(deform::common_flags(Cmd::new(
            "lattice",
            "Add a lattice to the selected meshes or vertices: lattice -dv 2 5 2 -oc true. Returns [ffd1, ffd1Lattice, ffd1Base]. \
             Move ffd1Lattice.pt[s][t][u] to deform. With -e on the ffd or lattice: -dv, -ldv, -ol, -ofd, -fm, -remap, -resetLattice. \
             With -q: -dv, -ldv, -ol, -ofd, -fm, -g.",
            |s, a| {
                if a.bool("query") {
                    return query(s, a);
                }
                if a.bool("edit") {
                    return edit(s, a);
                }
                create(s, a)
            },
        )))
        .positional("meshes or vertices (the selection when none); the ffd or lattice with -e and -q")
        .flag(f("divisions", "dv", ArgType::Vec3, "Control points in S, T and U (2 2 2)."))
        .flag(f("ldivisions", "ldv", ArgType::Vec3, "Local influence in S, T and U: how many control points move a point (2 2 2)."))
        .flag(f("objectCentered", "oc", ArgType::Bool, "Fit the lattice round the geometry (on). Off, a unit lattice at the origin."))
        .flag(f("outsideLattice", "ol", ArgType::Int, "0 only points inside deform, 1 all of them, 2 falling off with distance."))
        .flag(f("outsideFalloffDist", "ofd", ArgType::Float, "With -ol 2: how far the effect reaches, in lattice widths."))
        .flag(f("freezeMapping", "fm", ArgType::Bool, "Hold each point's place in the lattice from now on, wherever it goes."))
        .flag(f("position", "pos", ArgType::Vec3, "Where to put the lattice."))
        .flag(f("rotation", "ro", ArgType::Vec3, "Rotation of the lattice, degrees."))
        .flag(f("scale", "s", ArgType::Vec3, "Size of the lattice."))
        .flag(f("remap", "rmp", ArgType::Bool, "With -e: take the frozen mapping again from where the points are now."))
        .flag(f("resetLattice", "rl", ArgType::Bool, "With -e: put the control points back on the regular grid."))
        .flag(f("latticeReset", "ltr", ArgType::Bool, "With -e: the same as -resetLattice."))
        .flag(f("removeTweaks", "rt", ArgType::Bool, "With -e: the same as -resetLattice."))
        .flag(f("geometry", "g", ArgType::Any, "Query: the deformed shapes.")),
    );
}
