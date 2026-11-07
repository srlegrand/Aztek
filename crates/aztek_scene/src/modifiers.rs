//! A non-destructive mesh modifier: inMesh in, outMesh out, keyable parameters. Modifiers chain between a mesh's
//! source (a generator or a stored base mesh) and the mesh node, so the stack is just DG history:
//!
//!   polyCube -> bevelModifier -> mirrorModifier -> Cube.inMesh

use std::any::Any;
use std::collections::BTreeMap;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, RwLock};

use aztek_geo::array::{array_mesh, ArrayLayout, ArrayMode, ArrayOptions};
use aztek_geo::normals::{point_normals, update_corner_normals, CornerNormalOptions, NormalWeight};
use aztek_geo::ops::{
    bevel_edges, flip_normals, mirror_mesh, subdivide, triangulate_faces, BevelOptions, MirrorDirection, MirrorOptions,
    SubdivideOptions,
};
use aztek_geo::reduce::{reduce_mesh, ReduceOptions};
use aztek_geo::topology::boundary_loops;
use aztek_geo::triangulate::TriangulateOptions;
use aztek_geo::{names, Index, Mesh, MeshRemap};
use aztek_ops::{fail, objects, ArgType, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::{DMat4, DVec3, IVec3};
use serde_json::{json, Value};

use crate::attr_defs::{in_category, mesh_attr, param, param_enum};
use crate::components::{self, ComponentMode};
use crate::graph::Graph;
use crate::instancing;
use crate::mesh_nodes::MeshNode;
use crate::node::{traits, types, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, from_json, AttrDef, MeshPtr, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::Transform;

/// What a modifier node answers about itself.
pub trait ModifierBehavior {
    fn in_attr(&self) -> usize;
    fn enabled_attr(&self) -> usize;
    fn out_attr(&self) -> usize;
    /// True while this modifier shows its input for editing: the cage becomes its input and it and later modifiers
    /// pass through.
    fn editing_input(&self, _data: &NodeData) -> bool {
        false
    }
    /// Why the last evaluation failed (the input passed through), empty when it succeeded.
    fn error(&self) -> &str;
    /// The modifier run again on another input, for export; None when it can't be re-run on its own.
    fn reapply(&self, _input: &Mesh, _data: &NodeData) -> Option<Result<Mesh, String>> {
        None
    }
    /// A deformer that deformed its points on the GPU last time, in float.
    fn ran_on_gpu(&self) -> bool {
        false
    }
}

pub type ApplyFn = Arc<dyn Fn(&Mesh, &NodeData) -> Result<Mesh, String> + Send + Sync>;

/// Runs f, turning a panic into an error message.
pub fn guarded<R>(f: impl FnOnce() -> Result<R, String>) -> Result<R, String> {
    match catch_unwind(AssertUnwindSafe(f)) {
        Ok(r) => r,
        Err(p) => Err(p
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
            .unwrap_or_else(|| "failed".into())),
    }
}

/// A modifier whose work is one function of its input and parameters.
#[derive(Clone)]
pub struct ModifierNode {
    pub apply: ApplyFn,
    pub in_attr: usize,
    pub enabled_attr: usize,
    pub out_attr: usize,
    pub error: String,
    pub name_for_log: String,
}

impl ModifierNode {
    pub fn new(apply: ApplyFn, out_attr: usize) -> Self {
        Self { apply, in_attr: 0, enabled_attr: 1, out_attr, error: String::new(), name_for_log: String::new() }
    }
}

/// The compute every simple modifier shares: the input passes through when disabled, bypassed or failing.
pub fn modifier_compute(
    d: &mut NodeData,
    in_attr: usize,
    enabled_attr: usize,
    out_attr: usize,
    error: &mut String,
    apply: impl FnOnce(&Mesh, &NodeData) -> Result<Mesh, String>,
) {
    error.clear();
    let input = d.mesh(in_attr).cloned();
    let Some(input) = input else {
        d.set_output(out_attr, PlugValue::Mesh(None));
        return;
    };
    if !d.bool(enabled_attr) || bypassed(Some(&input)) {
        d.set_output(out_attr, PlugValue::Mesh(Some(input)));
        return;
    }
    match guarded(|| apply(&input, d)) {
        Ok(m) => d.set_output(out_attr, PlugValue::Mesh(Some(Arc::new(m)))),
        Err(e) => {
            log::warn!("{}: {e}", d.ty().name);
            *error = e;
            d.set_output(out_attr, PlugValue::Mesh(Some(input)));
        }
    }
}

impl NodeBehavior for ModifierNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let apply = self.apply.clone();
        modifier_compute(d, self.in_attr, self.enabled_attr, self.out_attr, &mut self.error, |m, d| apply(m, d));
    }
    fn traits(&self) -> u32 {
        traits::MODIFIER
    }
    fn as_modifier(&self) -> Option<&dyn ModifierBehavior> {
        Some(self)
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl ModifierBehavior for ModifierNode {
    fn in_attr(&self) -> usize {
        self.in_attr
    }
    fn enabled_attr(&self) -> usize {
        self.enabled_attr
    }
    fn out_attr(&self) -> usize {
        self.out_attr
    }
    fn error(&self) -> &str {
        &self.error
    }
    fn reapply(&self, input: &Mesh, data: &NodeData) -> Option<Result<Mesh, String>> {
        Some(guarded(|| (self.apply)(input, data)))
    }
}

/// Holds a base mesh upstream of modifiers, so direct edits never disturb the stack.
#[derive(Clone, Default)]
pub struct MeshSourceNode;

impl MeshSourceNode {
    pub const MESH: usize = 0;
    pub const OUT_MESH: usize = 1;
}

impl NodeBehavior for MeshSourceNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let m = d.mesh(Self::MESH).cloned();
        d.set_output(Self::OUT_MESH, PlugValue::Mesh(m));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// Point layer marking a mesh that passes through the rest of its stack untouched (a modifier upstream is in edit
/// mode).
pub const BYPASS_LAYER: &str = "aztek:editBypass";

pub fn bypassed(mesh: Option<&MeshPtr>) -> bool {
    mesh.is_some_and(|m| m.point_attrs().has(BYPASS_LAYER))
}

pub fn is_modifier(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.behavior().traits() & traits::MODIFIER != 0)
}

/// A modifier's (in, enabled, out) plugs.
pub fn modifier_plugs(g: &Graph, id: NodeId) -> Option<(usize, usize, usize)> {
    let m = g.node(id)?.behavior().as_modifier()?;
    Some((m.in_attr(), m.enabled_attr(), m.out_attr()))
}

pub fn editing_input(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.behavior().as_modifier().is_some_and(|m| m.editing_input(n.data())))
}

fn num_of(d: &NodeData, a: &str) -> f64 {
    d.float(d.ty().attr_index(a).expect(a))
}
fn int_of(d: &NodeData, a: &str) -> i64 {
    d.int(d.ty().attr_index(a).expect(a))
}
fn flag_of(d: &NodeData, a: &str) -> bool {
    d.bool(d.ty().attr_index(a).expect(a))
}
fn vector_of(d: &NodeData, a: &str) -> DVec3 {
    d.vec3(d.ty().attr_index(a).expect(a))
}
fn matrix_of(d: &NodeData, a: &str) -> DMat4 {
    d.matrix(d.ty().attr_index(a).expect(a))
}

/// arrayModifier's center enum.
const CENTER_ORIGIN: i64 = 0;
const CENTER_POINT: i64 = 1;
const CENTER_OBJECT: i64 = 2;

/// The array layout and weld settings an arrayModifier's plugs describe.
fn array_options(n: &NodeData) -> ArrayOptions {
    let mut o = ArrayOptions::default();
    let l: &mut ArrayLayout = &mut o.layout;
    l.mode = match int_of(n, "mode").clamp(0, 2) {
        0 => ArrayMode::Linear,
        1 => ArrayMode::Radial,
        _ => ArrayMode::Grid,
    };
    l.count = int_of(n, "count").clamp(1, 1000) as i32;
    l.grid_count = IVec3::new(l.count, int_of(n, "countY").clamp(1, 1000) as i32, int_of(n, "countZ").clamp(1, 1000) as i32);
    // Grids stay under the same total as rows so a stray count cannot hang the evaluation.
    while l.grid_count.x as i64 * l.grid_count.y as i64 * l.grid_count.z as i64 > 100000 {
        l.grid_count = (l.grid_count / 2).max(IVec3::ONE);
    }
    l.offset = match l.mode {
        ArrayMode::Grid => vector_of(n, "spacing"),
        ArrayMode::Linear => vector_of(n, "offset"),
        ArrayMode::Radial => DVec3::ZERO,
    };
    l.rotate = vector_of(n, "rotate");
    l.scale = vector_of(n, "scale");
    l.axis = int_of(n, "axis").clamp(0, 2) as i32;
    l.angle = num_of(n, "angle");
    l.step_angle = int_of(n, "angleMode") == 1;
    l.rise = num_of(n, "rise");
    match int_of(n, "center") {
        CENTER_POINT => l.frame = DMat4::from_translation(vector_of(n, "centerPoint")),
        CENTER_OBJECT => {
            // The object's frame relative to this mesh; scale is dropped so it orients and places the centre only.
            let mut rel = matrix_of(n, "targetMatrix").inverse() * matrix_of(n, "centerMatrix");
            for c in 0..3 {
                let col = rel.col(c).truncate();
                let len = col.length();
                if len > 1e-12 {
                    *rel.col_mut(c) /= len;
                }
            }
            l.frame = rel;
        }
        _ => {}
    }
    o.weld = flag_of(n, "weld");
    o.merge_ends = flag_of(n, "mergeEnds");
    o.tolerance = num_of(n, "tolerance").max(0.0);
    o
}

fn all_faces(m: &Mesh) -> Vec<Index> {
    (0..m.face_capacity()).filter(|&f| m.face_valid(f)).collect()
}

/// Edges whose faces meet sharper than the angle, or every hard edge when the angle is negative.
fn sharp_edges(m: &Mesh, degrees: f64) -> Vec<Index> {
    let hard = m.edge_attrs().get::<bool>(names::HARD);
    let cos_limit = degrees.to_radians().cos();
    let mut out = Vec::new();
    for e in 0..m.edge_capacity() {
        if !m.edge_valid(e) || m.edge_face_count(e) != 2 {
            continue;
        }
        if degrees < 0.0 {
            if (e as usize) < hard.len() && hard[e as usize] {
                out.push(e);
            }
            continue;
        }
        let c = m.edge_corner(e);
        let f0 = m.corner_face(c);
        let f1 = m.corner_face(m.corner_radial(c));
        if m.face_normal(f0).dot(m.face_normal(f1)) < cos_limit {
            out.push(e);
        }
    }
    out
}

fn solidify(input: &Mesh, thickness: f64) -> Mesh {
    let mut m = input.clone();
    m.compact(None);
    let normals = point_normals(&m, NormalWeight::Angle);
    let n = m.point_capacity();
    let mut inner = m.clone();
    for p in 0..n {
        inner.set_position(p, m.position(p) - normals[p as usize].as_dvec3() * thickness);
    }
    let faces = all_faces(&inner);
    flip_normals(&mut inner, &faces);
    let loops = boundary_loops(&m);
    let offset = m.append(&inner);
    // Rim quads join each boundary edge to its copy on the inner shell.
    for lp in &loops {
        for &e in &lp.edges {
            let c = m.edge_corner(e);
            let a = m.corner_point(c);
            let b = m.corner_next_point(c);
            m.add_face(&[b, a, a + offset, b + offset]);
        }
    }
    m
}

fn decimate(input: &Mesh, n: &NodeData) -> Mesh {
    let o = ReduceOptions {
        ratio: num_of(n, "ratio").clamp(0.0, 1.0),
        target_triangles: int_of(n, "targetTriangles").max(0) as Index,
        keep_borders: flag_of(n, "keepBorders"),
        keep_hard_edges: flag_of(n, "keepHardEdges"),
        symmetry_axis: int_of(n, "symmetry").clamp(0, 3) as i32 - 1,
        quads: flag_of(n, "quads"),
        ..Default::default()
    };
    reduce_mesh(input, &o).mesh
}

struct ModifierType {
    name: &'static str,
    doc: &'static str,
    params: Vec<AttrDef>,
    apply: ApplyFn,
}

fn ok(m: Mesh) -> Result<Mesh, String> {
    Ok(m)
}

fn modifier_types() -> Vec<ModifierType> {
    use PlugType as P;
    vec![
        ModifierType {
            name: "mirrorModifier",
            doc: "Adds a mirrored copy across a plane through the object's origin and welds the seam. Model one half of a \
                  symmetrical object and let this build the other.",
            params: vec![
                param_enum("axis", 0, "Object-space axis the mirror plane faces: x mirrors left to right.", &["x", "y", "z"]),
                param_enum(
                    "direction",
                    0,
                    "Which side builds the other: + to - keeps the positive side and replaces the negative one with its \
                     mirror, cutting faces that cross the plane; - to + the reverse; both mirrors the whole mesh.",
                    &["+ to -", "- to +", "both"],
                ),
                param("weld", P::Bool, true, "Merge points on the plane with their mirror images so the halves join.", None, None),
                param("tolerance", P::Float, 0.01, "Points this close to the plane are welded, cm.", Some(0.0), None),
            ],
            apply: Arc::new(|input, n| {
                let mut m = input.clone();
                let mut o = MirrorOptions::default();
                o.plane.normal = DVec3::ZERO;
                o.plane.normal[int_of(n, "axis").clamp(0, 2) as usize] = 1.0;
                o.direction = match int_of(n, "direction").clamp(0, 2) {
                    0 => MirrorDirection::Positive,
                    1 => MirrorDirection::Negative,
                    _ => MirrorDirection::Both,
                };
                o.weld = flag_of(n, "weld");
                o.tolerance = num_of(n, "tolerance");
                mirror_mesh(&mut m, &o);
                ok(m)
            }),
        },
        ModifierType {
            name: "subdivisionModifier",
            doc: "Smooths the mesh with Catmull-Clark subdivision. Use it to turn a low-poly cage into a smooth high-poly \
                  surface.",
            params: vec![
                param(
                    "levels",
                    P::Int,
                    1i64,
                    "Subdivision levels, 0 to 4. Each level about quadruples the face count; 0 passes the mesh through.",
                    Some(0.0),
                    Some(4.0),
                ),
                param("keepHardEdges", P::Bool, false, "Keep normals split along hard edges; off smooths the shading across them.", None, None),
            ],
            apply: Arc::new(|input, n| {
                let o = SubdivideOptions {
                    levels: int_of(n, "levels").clamp(0, 4) as i32,
                    keep_hard_edges: flag_of(n, "keepHardEdges"),
                    ..Default::default()
                };
                ok(if o.levels == 0 { input.clone() } else { subdivide(input, &o, None) })
            }),
        },
        ModifierType {
            name: "bevelModifier",
            doc: "Bevels every edge sharper than an angle so hard corners catch the light. A quick live bevel; Auto Bevel has \
                  finer edge rules and hand-picked edges.",
            params: vec![
                param("offset", P::Float, 1.0, "Bevel width, cm.", Some(0.0), None),
                param("segments", P::Int, 2i64, "Edges across each bevel, 1 to 32. More is rounder.", Some(1.0), Some(32.0)),
                param("profile", P::Float, 0.5, "Profile shape: 0.5 is a round arc, 0.25 a straight chamfer.", Some(0.0), Some(1.0)),
                param(
                    "angle",
                    P::Float,
                    30.0,
                    "Bevel edges whose faces meet sharper than this, degrees. Negative bevels hard edges only.",
                    None,
                    Some(180.0),
                ),
            ],
            apply: Arc::new(|input, n| {
                let mut m = input.clone();
                let o = BevelOptions {
                    offset: num_of(n, "offset"),
                    segments: int_of(n, "segments").clamp(1, 32) as i32,
                    profile: num_of(n, "profile"),
                    ..Default::default()
                };
                let edges = sharp_edges(&m, num_of(n, "angle"));
                bevel_edges(&mut m, &edges, &o);
                ok(m)
            }),
        },
        ModifierType {
            name: "arrayModifier",
            doc: "Repeats the mesh in a row, around a circle or in a grid, each copy stepped from the last. Use it for \
                  railings, stairs, tiles, gear teeth and bolts round a flange. Radial with weld on turns one modelled wedge \
                  into an exact N-fold whole.",
            params: vec![
                param_enum(
                    "mode",
                    0,
                    "linear steps each copy by offset; radial turns copies about the centre; grid fills rows, columns and layers.",
                    &["linear", "radial", "grid"],
                ),
                param("count", P::Int, 3i64, "Copies including the original, 1 to 1000. In grid mode, copies along x.", Some(1.0), Some(1000.0)),
                param("offset", P::Vec3, DVec3::new(110.0, 0.0, 0.0), "Linear: step from one copy to the next, in the centre's space, cm.", None, None),
                param("rotate", P::Vec3, DVec3::ZERO, "Extra rotation per copy about the centre, degrees. A linear array with a rotation curves.", None, None),
                param("scale", P::Vec3, DVec3::ONE, "Scale per copy about the centre: each copy is this much the size of the last.", None, None),
                in_category(param_enum("axis", 1, "The centre's axis radial copies turn about.", &["x", "y", "z"]), "Radial"),
                in_category(
                    param(
                        "angle",
                        P::Float,
                        360.0,
                        "Degrees the copies span, or between neighbours when angleMode is step. A whole turn closes the ring.",
                        None,
                        None,
                    ),
                    "Radial",
                ),
                in_category(
                    param_enum("angleMode", 0, "total spreads count copies over angle; step puts angle between neighbours.", &["total", "step"]),
                    "Radial",
                ),
                in_category(param("rise", P::Float, 0.0, "Distance each copy climbs along the axis, cm, for spirals and screw threads.", None, None), "Radial"),
                in_category(param("countY", P::Int, 3i64, "Grid copies along y, 1 to 1000.", Some(1.0), Some(1000.0)), "Grid"),
                in_category(param("countZ", P::Int, 1i64, "Grid copies along z, 1 to 1000.", Some(1.0), Some(1000.0)), "Grid"),
                in_category(
                    param("spacing", P::Vec3, DVec3::splat(110.0), "Distance between grid copies along each axis, in the centre's space, cm.", None, None),
                    "Grid",
                ),
                in_category(
                    param_enum(
                        "center",
                        0,
                        "What copies turn and scale about: the object's origin, centerPoint, or another object's transform (arrayCenter).",
                        &["origin", "point", "object"],
                    ),
                    "Center",
                ),
                in_category(param("centerPoint", P::Vec3, DVec3::ZERO, "Centre when center is point, object space, cm.", None, None), "Center"),
                in_category(
                    AttrDef {
                        name: "centerMatrix".into(),
                        ty: P::Matrix,
                        default_value: PlugValue::Matrix(DMat4::IDENTITY),
                        flags: flags::INPUT | flags::HIDDEN,
                        doc: "The centre object's worldMatrix, connected by arrayCenter.".into(),
                        ..Default::default()
                    },
                    "Center",
                ),
                in_category(
                    AttrDef {
                        name: "targetMatrix".into(),
                        ty: P::Matrix,
                        default_value: PlugValue::Matrix(DMat4::IDENTITY),
                        flags: flags::INPUT | flags::STORABLE | flags::HIDDEN,
                        doc: "This mesh's world matrix when the centre object was connected.".into(),
                        ..Default::default()
                    },
                    "Center",
                ),
                in_category(
                    param(
                        "weld",
                        P::Bool,
                        false,
                        "Merge points where neighbouring copies meet and drop the walls they share, so the copies join into one surface.",
                        None,
                        None,
                    ),
                    "Weld",
                ),
                in_category(
                    param("mergeEnds", P::Bool, false, "Weld the last copy to the first as well. A radial array spanning a whole turn always does.", None, None),
                    "Weld",
                ),
                in_category(param("tolerance", P::Float, 0.01, "Points of neighbouring copies this close are welded, cm.", Some(0.0), None), "Weld"),
            ],
            apply: Arc::new(|input, n| ok(array_mesh(input, &array_options(n)))),
        },
        ModifierType {
            name: "triangulateModifier",
            doc: "Splits every face into triangles. Use it to see, or lock in, the triangulation an engine will get.",
            params: vec![],
            apply: Arc::new(|input, _| {
                let mut m = input.clone();
                let faces = all_faces(&m);
                triangulate_faces(&mut m, &faces, &TriangulateOptions::default());
                ok(m)
            }),
        },
        ModifierType {
            name: "weightedNormalsModifier",
            doc: "Weights vertex normals by face area so big flat faces shade flat and the bevels take the curvature. Use it on \
                  bevelled hard-surface game meshes.",
            params: vec![param("keepHardEdges", P::Bool, true, "Keep normals split along hard edges instead of smoothing across them.", None, None)],
            apply: Arc::new(|input, n| {
                // Face-area weighting keeps big flat faces flat, the usual game-art bevel look.
                let mut m = input.clone();
                update_corner_normals(
                    &mut m,
                    &CornerNormalOptions { weight: NormalWeight::Area, use_hard_edges: flag_of(n, "keepHardEdges"), ..Default::default() },
                );
                ok(m)
            }),
        },
        ModifierType {
            name: "solidifyModifier",
            doc: "Gives a surface thickness: an inner shell offset along the normals, joined to the outer one at open borders. \
                  Use it to turn planes and open shells into solid panels.",
            params: vec![param("thickness", P::Float, 2.0, "Shell thickness, cm, offset inward along the normals.", None, None)],
            apply: Arc::new(|input, n| ok(solidify(input, num_of(n, "thickness")))),
        },
        ModifierType {
            name: "decimateModifier",
            doc: "Reduces the triangle count while keeping the shape, silhouette, UVs and normals. Use it for LODs or to \
                  lighten scans and sculpts.",
            params: vec![
                param("ratio", P::Float, 0.5, "Fraction of triangles to keep, 0 to 1.", Some(0.0), Some(1.0)),
                param("targetTriangles", P::Int, 0i64, "Triangles to keep; above 0 it overrides ratio.", Some(0.0), None),
                param("keepBorders", P::Bool, true, "Open borders stay exactly where they are.", None, None),
                param(
                    "keepHardEdges",
                    P::Bool,
                    true,
                    "Hard edges stay intact, as UV seams and material borders always do; off lets them collapse for a lower count.",
                    None,
                    None,
                ),
                param_enum(
                    "symmetry",
                    0,
                    "off, or reduce one half and mirror it so the result stays exactly symmetric across x, y or z.",
                    &["off", "x", "y", "z"],
                ),
                param("quads", P::Bool, false, "Join triangle pairs back into quads where they make good ones.", None, None),
            ],
            apply: Arc::new(|input, n| ok(decimate(input, n))),
        },
    ]
}

// ---------------------------------------------------------------------------------------------------------------------
// Extension points for modifier modules implemented separately (auto bevel, extrude, split rings, transfer, boolean)

pub type CreateHook = Arc<dyn Fn(&mut Graph, NodeId, NodeId) + Send + Sync>;
pub type RemapHook = Arc<dyn Fn(&mut Graph, NodeId, &MeshRemap, &Mesh) + Send + Sync>;

fn create_hooks() -> &'static RwLock<BTreeMap<String, CreateHook>> {
    static H: RwLock<BTreeMap<String, CreateHook>> = RwLock::new(BTreeMap::new());
    &H
}

fn remap_hooks() -> &'static RwLock<Vec<RemapHook>> {
    static H: RwLock<Vec<RemapHook>> = RwLock::new(Vec::new());
    &H
}

/// Runs as addModifier creates a modifier of the type, before it is wired in, so it can take context such as the
/// selected components. Arguments: graph, modifier, mesh.
pub fn set_create_hook(type_name: &str, hook: CreateHook) {
    create_hooks().write().unwrap().insert(type_name.into(), hook);
}

/// Registers what base_remapped tells a modifier module (autoBevel, extrudeModifier, splitRingsModifier).
pub fn add_remap_hook(hook: RemapHook) {
    remap_hooks().write().unwrap().push(hook);
}

/// A direct edit compacted the base mesh: modifiers holding component ids on it follow them. Records undo with the
/// edit.
pub fn base_remapped(g: &mut Graph, mesh: NodeId, remap: &MeshRemap, base: &Mesh) {
    let hooks: Vec<RemapHook> = remap_hooks().read().unwrap().clone();
    for h in hooks {
        h(g, mesh, remap, base);
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Stack queries

/// The output mesh plug of a modifier, generator or anything with an outMesh.
pub fn out_attr_of(g: &Graph, id: NodeId) -> Option<usize> {
    let n = g.node(id)?;
    if n.ty().is_a("mesh") {
        return Some(MeshNode::OUT_MESH);
    }
    if let Some(m) = n.behavior().as_modifier() {
        return Some(m.out_attr());
    }
    n.ty().attr_index("outMesh")
}

fn plug(node: NodeId, attr: usize) -> PlugRef {
    PlugRef::new(node, attr as i32)
}

/// Modifiers feeding a mesh node, first applied first.
pub fn stack(g: &Graph, mesh: NodeId) -> Vec<NodeId> {
    let mut out = Vec::new();
    let mut p = plug(mesh, MeshNode::IN_MESH);
    while let Some(src) = g.source(&p) {
        let Some((input, _, _)) = modifier_plugs(g, src.node) else { break };
        out.insert(0, src.node);
        p = plug(src.node, input);
    }
    out
}

/// The plug at the bottom of the modifier chain, and what feeds it.
pub fn chain_base(g: &Graph, mesh: NodeId) -> (PlugRef, Option<PlugRef>) {
    let mut p = plug(mesh, MeshNode::IN_MESH);
    while let Some(src) = g.source(&p) {
        let Some((input, _, _)) = modifier_plugs(g, src.node) else { return (p, Some(src)) };
        p = plug(src.node, input);
    }
    (p, None)
}

pub fn is_mesh_source_node(g: &Graph, id: NodeId) -> bool {
    is_mesh_source(g, id)
}

fn is_mesh_source(g: &Graph, id: NodeId) -> bool {
    g.node(id).is_some_and(|n| n.ty().name == "meshSource")
}

fn create(g: &mut Graph, type_name: &str) -> CmdResult<NodeId> {
    let t = types::find(type_name).ok_or_else(|| CommandError::new(format!("unknown node type '{type_name}'")))?;
    Ok(g.create_node(&t, "", NO_NODE, true)?)
}

/// The storable plug holding the base mesh direct edits change. Bakes a procedural source into a stored one.
pub fn edit_plug(g: &mut Graph, mesh: NodeId) -> CmdResult<PlugRef> {
    if let Some(n) = g.node(mesh) {
        if !n.ty().is_a("mesh") {
            if let Some(ep) = components::point_provider(n).and_then(|p| p.edit_plug) {
                return ep(g, mesh);
            }
        }
    }
    // An instance has no base of its own: edits land on its root's, which every member shares.
    let root = instancing::root(g, mesh);
    if root != mesh {
        return edit_plug(g, root);
    }
    for id in stack(g, mesh) {
        if editing_input(g, id) {
            fail!("{} is editing its edges: apply or cancel that first", g.path(id));
        }
    }
    let (base, src) = chain_base(g, mesh);
    let Some(src) = src else { return Ok(base) };
    let up = src.node;
    if is_mesh_source(g, up) {
        return Ok(plug(up, MeshSourceNode::MESH));
    }

    // A generator (or anything else) feeds the chain: freeze its current result into a stored source.
    g.evaluate();
    let frozen = g.value(&src)?;
    log::info!("{}: baked {} into the mesh to edit it; its settings no longer apply", g.path(mesh), g.path(up));
    // Other stacks it feeds (references) move onto the frozen source with this one, so they keep sharing it.
    let mut shared = Vec::new();
    for d in g.destinations(&src) {
        let Some(to) = g.node(d.node) else { continue };
        let chain_input = if to.ty().is_a("mesh") {
            d.attr == MeshNode::IN_MESH as i32
        } else {
            to.behavior().as_modifier().is_some_and(|m| d.attr == m.in_attr() as i32)
        };
        if chain_input && !(d.node == base.node && d.attr == base.attr) {
            shared.push(d);
        }
    }
    g.disconnect(&base);
    let up_dag = g.node(up).is_some_and(|n| n.ty().dag);
    if base.node == mesh && shared.is_empty() {
        g.set_value(base, frozen)?;
        if g.destinations(&src).is_empty() && !up_dag {
            g.delete_node(up)?;
        }
        return Ok(base);
    }
    let source = create(g, "meshSource")?;
    g.set_value(plug(source, MeshSourceNode::MESH), frozen)?;
    g.connect(&plug(source, MeshSourceNode::OUT_MESH), &base, false)?;
    for d in &shared {
        g.disconnect(d);
        g.connect(&plug(source, MeshSourceNode::OUT_MESH), d, false)?;
    }
    if g.destinations(&src).is_empty() && !up_dag {
        g.delete_node(up)?;
    }
    Ok(plug(source, MeshSourceNode::MESH))
}

/// The base mesh without modifiers (the cage), as components are picked and edited on it. Doesn't change the graph.
/// A transform stands for its mesh shape.
pub fn cage(g: &mut Graph, mesh: NodeId) -> Option<MeshPtr> {
    g.evaluate();
    let mesh = components::as_mesh(g, mesh)?;
    let root = instancing::root(g, mesh);
    if root != mesh {
        return cage(g, root);
    }
    for id in stack(g, mesh) {
        if editing_input(g, id) {
            let n = g.node(id)?;
            let input = n.behavior().as_modifier()?.in_attr();
            return n.data().mesh(input).cloned();
        }
    }
    let (base, src) = chain_base(g, mesh);
    let p = src.unwrap_or(base);
    g.value(&p).ok().and_then(|v| v.as_mesh().cloned())
}

/// The mesh as it was last evaluated, for readers with no way to evaluate: a drawing pass, say.
pub fn cage_cached(g: &Graph, mesh: NodeId) -> Option<MeshPtr> {
    let id = components::as_mesh(g, mesh)?;
    g.node(id)?.data().mesh(crate::mesh_nodes::MeshNode::OUT_MESH).cloned()
}

/// The mesh's component selection, mode and object selection, for an edit mode to put back when it ends.
pub fn selection_snapshot(g: &Graph, mesh: NodeId) -> Value {
    let names: Vec<String> = g.selection().iter().filter(|&&id| g.node(id).is_some()).map(|&id| g.path(id)).collect();
    json!({
        "mode": components::mode_name(components::mode(g)),
        "vertices": components::selected(g, mesh, ComponentMode::Vertex),
        "edges": components::selected(g, mesh, ComponentMode::Edge),
        "faces": components::selected(g, mesh, ComponentMode::Face),
        "selection": names,
    })
}

pub fn restore_selection(g: &mut Graph, mesh: NodeId, j: &Value) {
    if !j.is_object() {
        return;
    }
    let ids: Vec<NodeId> = j
        .get("selection")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|n| n.as_str()).filter_map(|n| g.find(n).ok().flatten()).collect())
        .unwrap_or_default();
    g.select(&ids, crate::graph::SelectMode::Replace);
    for (key, kind) in [("vertices", ComponentMode::Vertex), ("edges", ComponentMode::Edge), ("faces", ComponentMode::Face)] {
        let ids: Vec<u32> = j
            .get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_u64).map(|x| x as u32).collect())
            .unwrap_or_default();
        components::select(g, mesh, kind, &ids, components::Op::Replace);
    }
    if let Ok(m) = components::mode_from_name(j.get("mode").and_then(Value::as_str).unwrap_or("object")) {
        components::set_mode(g, m);
    }
}

const SUFFIX: &str = "Modifier";

/// Kinds addModifier can create ("bevel" for bevelModifier), from the type registry, so extension modifiers appear
/// too. Sorted.
pub fn kinds() -> Vec<String> {
    let mut out = Vec::new();
    for t in types::all() {
        let name = &t.name;
        let Some(create) = &t.create else { continue };
        if name.len() <= SUFFIX.len() || !name.ends_with(SUFFIX) {
            continue;
        }
        // Geometry-node modifiers need their group wired in, so they are added from the node editor instead.
        if t.attr_index("groupDefinition").is_some() {
            continue;
        }
        if create().as_modifier().is_none() {
            continue;
        }
        out.push(name[..name.len() - SUFFIX.len()].to_string());
    }
    out.sort();
    out
}

/// What a kind ("bevel") does, from its node type's doc. Empty for an unknown or undocumented kind.
pub fn kind_doc(kind: &str) -> String {
    types::find(&format!("{kind}{SUFFIX}")).map(|t| t.doc.clone()).unwrap_or_default()
}

/// What feeds the bottom of the stack: a generator, a meshSource, or NO_NODE when the mesh stores its own base.
pub fn stack_source(g: &Graph, mesh: NodeId) -> NodeId {
    let mods = stack(g, mesh);
    let bottom = match mods.first() {
        Some(&first) => plug(first, modifier_plugs(g, first).unwrap().0),
        None => plug(mesh, MeshNode::IN_MESH),
    };
    g.source(&bottom).map_or(NO_NODE, |s| s.node)
}

/// The mesh node at the top of a modifier's chain, NO_NODE if it feeds none.
pub fn mesh_of(g: &Graph, modifier: NodeId) -> NodeId {
    let mut cur = modifier;
    // A chain can't loop, but a malformed graph shouldn't hang the UI either.
    for _ in 0..4096 {
        let Some(n) = g.node(cur) else { return NO_NODE };
        if n.ty().is_a("mesh") {
            return cur;
        }
        let Some(out) = out_attr_of(g, cur) else { return NO_NODE };
        let dsts = g.destinations(&plug(cur, out));
        let Some(d) = dsts.first() else { return NO_NODE };
        cur = d.node;
    }
    NO_NODE
}

/// Whether a modifier is applied on export (its exportEnabled attr; true when the type has none).
pub fn export_enabled(g: &Graph, modifier: NodeId) -> bool {
    let Some(n) = g.node(modifier) else { return true };
    n.ty().attr_index("exportEnabled").map_or(true, |a| n.bool(a))
}

/// The mesh as exporters should write it: outMesh, or the stack re-run without the modifiers turned off for export.
pub fn export_mesh(g: &mut Graph, mesh: NodeId) -> Option<MeshPtr> {
    g.evaluate();
    let mesh = components::as_mesh(g, mesh)?;
    let root = instancing::root(g, mesh);
    if root != mesh {
        return export_mesh(g, root);
    }
    let mods = stack(g, mesh);
    let ran_in_viewport = |g: &Graph, id: NodeId| {
        let n = g.node(id).unwrap();
        let (input, enabled, _) = modifier_plugs(g, id).unwrap();
        n.bool(enabled) && !bypassed(n.data().mesh(input))
    };
    let ran_on_gpu = |g: &Graph, id: NodeId| g.node(id).unwrap().behavior().as_modifier().unwrap().ran_on_gpu();
    // Evaluated results stay valid up to the first modifier whose export state differs from what the viewport ran,
    // or that ran on the GPU in float: exports get the CPU reference.
    let Some(split) = mods.iter().position(|&id| ran_in_viewport(g, id) != export_enabled(g, id) || ran_on_gpu(g, id)) else {
        return g.node(mesh)?.data().mesh(MeshNode::OUT_MESH).cloned();
    };
    let first = mods[split];
    let mut cur = g.node(first)?.data().mesh(modifier_plugs(g, first)?.0).cloned();
    for &id in &mods[split..] {
        if !export_enabled(g, id) {
            continue;
        }
        let Some(input) = cur.clone() else { continue };
        let n = g.node(id)?;
        match n.behavior().as_modifier()?.reapply(&input, n.data()) {
            None => log::warn!("{}: can't re-run for export after an earlier modifier changed; exported without it", g.path(id)),
            Some(Ok(m)) => cur = Some(Arc::new(m)),
            Some(Err(e)) => log::warn!("{}: {e}", g.path(id)),
        }
    }
    cur
}

// ---------------------------------------------------------------------------------------------------------------------
// Types

pub fn register_types() {
    if types::find("meshSource").is_some() {
        return;
    }
    let source = NodeType {
        name: "meshSource".into(),
        icon: "database".into(),
        doc: "Holds the base mesh under a modifier stack, where direct component edits land.".into(),
        own_attrs: vec![
            mesh_attr("mesh", "", flags::INPUT | flags::STORABLE | flags::HIDDEN, "Base mesh under a modifier stack."),
            mesh_attr("outMesh", "", flags::OUTPUT | flags::HIDDEN, ""),
        ],
        create: Some(Arc::new(|| Box::new(MeshSourceNode))),
        ..Default::default()
    };
    types::register_type(source).expect("meshSource");

    for m in modifier_types() {
        let mut own = vec![
            mesh_attr("inMesh", "", flags::INPUT | flags::HIDDEN, ""),
            param("enabled", PlugType::Bool, true, "Turn the modifier off without removing it.", None, None),
            AttrDef {
                name: "exportEnabled".into(),
                ty: PlugType::Bool,
                default_value: PlugValue::Bool(true),
                flags: flags::INPUT | flags::STORABLE,
                doc: "Apply the modifier when exporting.".into(),
                ..Default::default()
            },
        ];
        for mut p in m.params {
            if p.ty == PlugType::Vec3 {
                for k in 0..3 {
                    p.child_names[k] = format!("{}{}", p.name, (b'X' + k as u8) as char);
                }
            }
            own.push(p);
        }
        own.push(mesh_attr("outMesh", "", flags::OUTPUT | flags::HIDDEN, ""));
        let out = own.len() - 1;
        let apply = m.apply;
        let t = NodeType {
            name: m.name.into(),
            doc: m.doc.into(),
            icon: "stack-2".into(),
            own_attrs: own,
            create: Some(Arc::new(move || Box::new(ModifierNode::new(apply.clone(), out)))),
            ..Default::default()
        };
        types::register_type(t).expect("modifier type");
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, "", ty, default, doc)
}

fn modifier_mesh(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name)?.and_then(|id| components::as_mesh(g, id)) {
        Some(id) => Ok(id),
        None => fail!("{cmd}: {name} is not a mesh"),
    }
}

fn modifier_named(g: &Graph, name: &str, cmd: &str) -> CmdResult<NodeId> {
    match g.find(name)? {
        Some(id) if is_modifier(g, id) => Ok(id),
        _ => fail!("{cmd}: {name} is not a modifier"),
    }
}

/// Removes a modifier, reconnecting the stack around it.
fn unlink(g: &mut Graph, m: NodeId) -> CmdResult<()> {
    let (input, _, out) = modifier_plugs(g, m).unwrap();
    let src = g.source(&plug(m, input));
    let dsts = g.destinations(&plug(m, out));
    for d in &dsts {
        g.disconnect(d);
    }
    if let Some(src) = src {
        for d in &dsts {
            g.connect(&src, d, false)?;
        }
    }
    g.delete_node(m)?;
    Ok(())
}

/// Deletes a node nothing reads any more, unless it lives in the hierarchy.
fn drop_if_orphan(g: &mut Graph, id: NodeId) -> CmdResult<()> {
    let Some(n) = g.node(id) else { return Ok(()) };
    if n.ty().dag {
        return Ok(());
    }
    match out_attr_of(g, id) {
        Some(out) if !g.destinations(&plug(id, out)).is_empty() => Ok(()),
        _ => Ok(g.delete_node(id)?),
    }
}

/// A shape fed through a "create" data plug (a curve with history): stores what the history makes and drops the nodes
/// that fed only it.
fn bake_create_history(g: &mut Graph, n: NodeId) -> CmdResult<()> {
    let Some(create) = g.node(n).and_then(|x| x.ty().attr_index("create")) else { return Ok(()) };
    if g.node(n).unwrap().ty().attrs[create].ty != PlugType::Data {
        return Ok(());
    }
    let input = plug(n, create);
    let Some(src) = g.source(&input) else { return Ok(()) };
    g.evaluate();
    let baked = g.value(&input)?;
    g.disconnect(&input);
    g.set_value(input, baked)?;
    // Upstream history nodes nothing else reads go too.
    let mut ids = vec![src.node];
    while let Some(id) = ids.pop() {
        let Some(up) = g.node(id) else { continue };
        if up.ty().dag {
            continue;
        }
        let used = (0..up.ty().attrs.len()).any(|a| !g.destinations(&plug(id, a)).is_empty());
        if used {
            continue;
        }
        for (dst, from) in g.connections() {
            if dst.node == id {
                ids.push(from.node);
            }
        }
        g.delete_node(id)?;
    }
    Ok(())
}

fn describe(g: &Graph, id: NodeId) -> Value {
    let n = g.node(id).unwrap();
    let mut j = json!({"name": g.path(id), "type": n.ty().name});
    if let Some(m) = n.behavior().as_modifier() {
        j["enabled"] = json!(n.bool(m.enabled_attr()));
        j["exportEnabled"] = json!(export_enabled(g, id));
        j["error"] = json!(m.error());
    }
    j
}

pub fn register(c: &mut Commands<Session>) {
    let kind_list: String = kinds().iter().map(|k| format!("\n- {k}: {}", kind_doc(k))).collect();
    c.register(
        Cmd::new(
            "addModifier",
            &format!(
                "Add a modifier to the top of a mesh's stack. params sets its parameters. Returns its name. \
                 modifierKinds(docs=True) also lists kinds extensions add later.\n\nKinds:{kind_list}"
            ),
            |s, a| {
                let objs = objects(s, a);
                let (Some(last), true) = (objs.last(), a.has("kind")) else { fail!("addModifier: give a mesh and kind=") };
                let g = &mut s.graph;
                // On an instance it goes on the root's stack, which every member shares.
                let mesh = instancing::root(g, modifier_mesh(g, last, "addModifier")?);
                let kind = a.string("kind", "");
                let Some(t) = types::find(&format!("{kind}{SUFFIX}")) else { fail!("addModifier: unknown kind '{kind}'") };
                let top = plug(mesh, MeshNode::IN_MESH);
                let src = g.source(&top);
                let m = g.create_node(&t, "", NO_NODE, true)?;
                let (input, _, out) = modifier_plugs(g, m).ok_or_else(|| CommandError::new(format!("addModifier: {kind} is not a modifier")))?;
                let hook = create_hooks().read().unwrap().get(&t.name).cloned();
                if let Some(hook) = hook {
                    hook(g, m, mesh);
                }
                if let Some(src) = src {
                    g.disconnect(&top);
                    g.connect(&src, &plug(m, input), false)?;
                } else {
                    // The mesh's stored base moves into a source node so it stays editable under the stack.
                    let source = create(g, "meshSource")?;
                    let stored = g.stored_value(&top).clone();
                    g.set_value(plug(source, MeshSourceNode::MESH), stored)?;
                    g.set_value(top, PlugValue::Mesh(None))?;
                    g.connect(&plug(source, MeshSourceNode::OUT_MESH), &plug(m, input), false)?;
                }
                g.connect(&plug(m, out), &top, false)?;
                if let Some(params) = a.get("params").and_then(Value::as_object) {
                    for (k, val) in params {
                        let p = g.plug(&format!("{}.{k}", g.path(m)))?;
                        let mut def = g.attr(&p).clone();
                        if p.component >= 0 {
                            def.ty = PlugType::Float;
                        }
                        let v = from_json(&def, val)?;
                        g.set_value(p, v)?;
                    }
                }
                Ok(json!(g.path(m)))
            },
        )
        .positional("mesh")
        .flag(flag("kind", T::String, Value::Null, "Modifier kind, such as bevel or mirror (see Kinds)."))
        .flag(flag("params", T::Any, Value::Null, "{param: value} to set on the new modifier.")),
    );

    c.register(
        Cmd::new(
            "arrayCenter",
            "Set what an array modifier's copies turn and scale about. Give an object to centre on its transform (its \
             position and axes, kept live as it moves; run again after moving the arrayed mesh), pivot=True to copy the \
             mesh's pivot into centerPoint, or neither to go back to the object's origin.",
            |s, a| {
                let objs = objects(s, a);
                let g = &mut s.graph;
                let m = match objs.first() {
                    Some(name) => g.find(name)?,
                    None => None,
                };
                let Some(m) = m.filter(|&id| g.node(id).is_some_and(|n| n.ty().name == "arrayModifier")) else {
                    fail!("arrayCenter: give an array modifier first")
                };
                let at = |g: &Graph, name: &str| plug(m, g.node(m).unwrap().ty().attr_index(name).unwrap());
                let mesh = mesh_of(g, m);
                let center_matrix = at(g, "centerMatrix");
                if g.source(&center_matrix).is_some() {
                    g.disconnect(&center_matrix);
                }
                if objs.len() > 1 {
                    let last = objs.last().unwrap();
                    let Some(center) = g.find(last)?.filter(|&id| g.node(id).is_some_and(|n| n.ty().is_a("transform"))) else {
                        fail!("arrayCenter: {last} is not an object")
                    };
                    if center == mesh {
                        fail!("arrayCenter: centre on another object; pivot=True uses the mesh's own pivot");
                    }
                    g.connect(&plug(center, Transform::WORLD_MATRIX), &center_matrix, true)?;
                    if g.node(mesh).is_some() {
                        let w = g.node(mesh).unwrap().matrix(Transform::WORLD_MATRIX);
                        g.set_value(at(g, "targetMatrix"), PlugValue::Matrix(w))?;
                    }
                    g.set_value(at(g, "center"), PlugValue::Int(CENTER_OBJECT))?;
                } else if a.has("pivot") && a.bool("pivot") {
                    if g.node(mesh).is_none() {
                        fail!("arrayCenter: the modifier feeds no mesh");
                    }
                    let pivot = crate::mesh_nodes::object_pivot(g, mesh);
                    g.set_value(at(g, "centerPoint"), PlugValue::Vec3(pivot))?;
                    g.set_value(at(g, "center"), PlugValue::Int(CENTER_POINT))?;
                } else {
                    g.set_value(at(g, "center"), PlugValue::Int(CENTER_ORIGIN))?;
                }
                Ok(Value::Null)
            },
        )
        .positional("modifier, [object]")
        .flag(flag("pivot", T::Bool, Value::Null, "Centre on the arrayed mesh's current pivot.")),
    );

    c.register(
        Cmd::new("modifiers", "A mesh's modifier stack, bottom (applied first) to top.", |s, a| {
            let objs = objects(s, a);
            let Some(last) = objs.last() else { fail!("modifiers: no mesh") };
            let g = &s.graph;
            let mesh = modifier_mesh(g, last, "modifiers")?;
            Ok(Value::Array(stack(g, mesh).into_iter().map(|id| json!(g.path(id))).collect()))
        })
        .positional("mesh")
        .undoable(false),
    );

    c.register(
        Cmd::new("removeModifier", "Remove modifiers, reconnecting the stack around them.", |s, a| {
            for name in objects(s, a) {
                let m = modifier_named(&s.graph, &name, "removeModifier")?;
                unlink(&mut s.graph, m)?;
            }
            Ok(Value::Null)
        })
        .positional("modifiers"),
    );

    c.register(
        Cmd::new("applyModifier", "Bake a modifier (and everything below it in the stack) into the base mesh.", |s, a| {
            for name in objects(s, a) {
                let g = &mut s.graph;
                let m = modifier_named(g, &name, "applyModifier")?;
                let (input, _, out) = modifier_plugs(g, m).unwrap();
                g.evaluate();
                let baked = g.node(m).unwrap().value(out).clone();
                // Walk down to the source and drop every modifier below this one along the way.
                let mut p = plug(m, input);
                let mut below = Vec::new();
                let mut src = g.source(&p);
                while let Some(s) = src.clone() {
                    let Some((down_in, _, _)) = modifier_plugs(g, s.node) else { break };
                    below.push(s.node);
                    p = plug(s.node, down_in);
                    src = g.source(&p);
                }
                let dsts = g.destinations(&plug(m, out));
                for d in &dsts {
                    g.disconnect(d);
                }
                let source = match &src {
                    Some(s) if is_mesh_source(g, s.node) => s.node,
                    _ => {
                        if src.is_some() {
                            g.disconnect(&p);
                        }
                        create(g, "meshSource")?
                    }
                };
                g.set_value(plug(source, MeshSourceNode::MESH), baked)?;
                for d in &dsts {
                    g.connect(&plug(source, MeshSourceNode::OUT_MESH), d, false)?;
                }
                g.delete_node(m)?;
                for id in below {
                    g.delete_node(id)?;
                }
            }
            Ok(Value::Null)
        })
        .positional("modifier"),
    );

    c.register(
        Cmd::new(
            "moveModifier",
            "Move a modifier up or down its stack by a number of steps (positive is up, applied later).",
            |s, a| {
                let objs = objects(s, a);
                let Some(last) = objs.last() else { fail!("moveModifier: no modifier") };
                let g = &mut s.graph;
                let n = modifier_named(g, last, "moveModifier")?;
                // Find the mesh at the top of this chain.
                let mut cur = n;
                let top = loop {
                    let dsts = out_attr_of(g, cur).map(|o| g.destinations(&plug(cur, o))).unwrap_or_default();
                    let Some(d) = dsts.first().cloned() else { fail!("moveModifier: modifier is not connected to a mesh") };
                    if components::is_mesh(g, d.node) {
                        break d;
                    }
                    cur = d.node;
                };
                let mut order = stack(g, top.node);
                let from = order.iter().position(|&id| id == n).unwrap() as i64;
                let to = (from + a.int("by", 1)).clamp(0, order.len() as i64 - 1);
                if from == to {
                    return Ok(Value::Null);
                }
                order.remove(from as usize);
                order.insert(to as usize, n);
                // Rewire the chain in the new order.
                let current = stack(g, top.node);
                let first = current[0];
                let base = g.source(&plug(first, modifier_plugs(g, first).unwrap().0));
                for &id in &current {
                    let input = modifier_plugs(g, id).unwrap().0;
                    g.disconnect(&plug(id, input));
                }
                g.disconnect(&top);
                let mut prev = base;
                for &id in &order {
                    let (input, _, out) = modifier_plugs(g, id).unwrap();
                    if let Some(p) = &prev {
                        g.connect(p, &plug(id, input), false)?;
                    }
                    prev = Some(plug(id, out));
                }
                g.connect(&prev.unwrap(), &top, false)?;
                Ok(Value::Null)
            },
        )
        .positional("modifier")
        .flag(flag("by", T::Int, json!(1), "Steps.")),
    );

    register_stack_commands(c);
}

/// duplicateModifier, applyAllModifiers, bakeModifierStack, modifierStack.
pub fn register_stack_commands(c: &mut Commands<Session>) {
    c.register(
        Cmd::new(
            "modifierStack",
            "A mesh's stack as data: its source (generator, meshSource or null) and each modifier, applied first first, \
             with name, type, enabled, exportEnabled and its last evaluation error.",
            |s, a| {
                let objs = objects(s, a);
                let Some(last) = objs.last() else { fail!("modifierStack: no mesh") };
                let g = &mut s.graph;
                let mesh = modifier_mesh(g, last, "modifierStack")?;
                g.evaluate();
                let mods: Vec<Value> = stack(g, mesh).into_iter().map(|id| describe(g, id)).collect();
                let src = stack_source(g, mesh);
                Ok(json!({
                    "mesh": g.path(mesh),
                    "source": if src != NO_NODE { describe(g, src) } else { Value::Null },
                    "modifiers": mods,
                }))
            },
        )
        .positional("mesh")
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "modifierKinds",
            "Every modifier kind addModifier accepts, including ones extensions registered. docs=True returns {kind: \
             description} instead.",
            |_, a| {
                if !a.bool("docs") {
                    return Ok(json!(kinds()));
                }
                let mut out = serde_json::Map::new();
                for k in kinds() {
                    out.insert(k.clone(), json!(kind_doc(&k)));
                }
                Ok(Value::Object(out))
            },
        )
        .flag(flag("docs", T::Bool, json!(false), "Return each kind's description too."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "duplicateModifier",
            "Copy a modifier with its settings and insert the copy right after it in the stack. Returns the copy's name.",
            |s, a| {
                let objs = objects(s, a);
                let Some(last) = objs.last() else { fail!("duplicateModifier: no modifier") };
                let g = &mut s.graph;
                let orig = modifier_named(g, last, "duplicateModifier")?;
                let t = g.node(orig).unwrap().type_arc().clone();
                let (orig_in, _, orig_out) = modifier_plugs(g, orig).unwrap();
                let copy = g.create_node(&t, "", NO_NODE, true)?;
                let (copy_in, _, copy_out) = modifier_plugs(g, copy).unwrap();
                for i in 0..t.attrs.len() {
                    let def = &t.attrs[i];
                    if i == orig_in || def.flags & flags::INPUT == 0 {
                        continue;
                    }
                    // Driven inputs (keys, geometry-node groups, object info) share the same driver.
                    if def.flags & flags::ARRAY != 0 {
                        g.copy_elements(orig, copy, i as i32, true)?;
                    } else if let Some(src) = g.source(&plug(orig, i)) {
                        g.connect(&src, &plug(copy, i), false)?;
                    } else if def.ty != PlugType::Mesh {
                        let v = g.node(orig).unwrap().value(i).clone();
                        g.set_value(plug(copy, i), v)?;
                    }
                }
                let out = plug(orig, orig_out);
                let dsts = g.destinations(&out);
                for d in &dsts {
                    g.disconnect(d);
                }
                g.connect(&out, &plug(copy, copy_in), false)?;
                for d in &dsts {
                    g.connect(&plug(copy, copy_out), d, false)?;
                }
                Ok(json!(g.path(copy)))
            },
        )
        .positional("modifier"),
    );

    c.register(
        Cmd::new(
            "applyAllModifiers",
            "Bake every modifier of a mesh into its base mesh, leaving an empty stack over an editable source.",
            |s, a| {
                for name in objects(s, a) {
                    let mesh = modifier_mesh(&s.graph, &name, "applyAllModifiers")?;
                    let mods = stack(&s.graph, mesh);
                    let Some(&top) = mods.last() else { continue };
                    let src = stack_source(&s.graph, mesh);
                    let path = s.graph.path(top);
                    aztek_ops::run(s, "applyModifier", json!({"args": [path]}))?;
                    if src != NO_NODE && !is_mesh_source(&s.graph, src) {
                        drop_if_orphan(&mut s.graph, src)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("meshes"),
    );

    c.register(
        Cmd::new(
            "bakeModifierStack",
            "Bake meshes to their current result, deleting the generator and modifiers feeding them. Curves likewise keep \
             what their history makes and lose the history; other objects are skipped.",
            |s, a| {
                for name in objects(s, a) {
                    let g = &mut s.graph;
                    if let Some(id) = g.find(&name)? {
                        if components::as_mesh(g, id).is_none() {
                            bake_create_history(g, id)?;
                            continue;
                        }
                    }
                    let mesh = modifier_mesh(g, &name, "bakeModifierStack")?;
                    let mods = stack(g, mesh);
                    for &id in &mods {
                        if editing_input(g, id) {
                            fail!("bakeModifierStack: {} is editing its edges: apply or cancel that first", g.path(id));
                        }
                    }
                    let input = plug(mesh, MeshNode::IN_MESH);
                    if g.source(&input).is_none() {
                        continue;
                    }
                    g.evaluate();
                    let baked = g.node(mesh).unwrap().value(MeshNode::OUT_MESH).clone();
                    let src = stack_source(g, mesh);
                    g.disconnect(&input);
                    g.set_value(input, baked)?;
                    for &id in mods.iter().rev() {
                        g.delete_node(id)?;
                    }
                    if src != NO_NODE {
                        drop_if_orphan(g, src)?;
                    }
                }
                Ok(Value::Null)
            },
        )
        .positional("objects"),
    );
}
