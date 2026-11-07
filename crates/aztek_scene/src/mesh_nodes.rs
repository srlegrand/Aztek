//! The mesh node, the primitive generators, symmetry settings and the core mesh
//! commands.

use std::any::Any;
use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use aztek_geo::obj::{read_obj_file, write_obj_objects, ObjObject, ObjReadOptions, ObjWriteOptions};
use aztek_geo::primitives::*;
use aztek_geo::symmetry::{axis_radial, radial_reference, RadialSymmetry, SymmetryMap, SymmetrySpec, SymmetrySync, MAX_RADIAL_COUNT};
use aztek_geo::{names, Index, Mesh, INVALID};
use aztek_ops::{fail, objects, ArgType, Args, CmdResult, CommandDef, CommandError, Commands, FlagDef};
use glam::{DMat3, DQuat, DVec3};
use rayon::prelude::*;
use serde_json::{json, Value};

use crate::attr_defs::{float_attr, mesh_attr};
use crate::graph::{Graph, SelectMode};
use crate::modifiers;
use crate::node::{types, vec3_attr, EvalContext, NodeBehavior, NodeData, NodeType};
use crate::plug::{flags, from_json, to_json, AttrDef, NodeId, PlugRef, PlugType, PlugValue, NO_NODE};
use crate::session::Session;
use crate::transform::{matrix_to_euler, RotateOrder, Shape, Transform};

/// A polygon mesh in the hierarchy. `inMesh` holds the base mesh (or is fed by a generator such as polyCube);
/// `outMesh` is what gets drawn and exported once modifiers run.
#[derive(Clone, Default)]
pub struct MeshNode;

impl MeshNode {
    pub const IN_MESH: usize = Shape::COUNT;
    pub const OUT_MESH: usize = Shape::COUNT + 1;
    pub const SYMMETRY: usize = Shape::COUNT + 2;
    pub const SMOOTH_PREVIEW: usize = Shape::COUNT + 3;
    pub const RETOPO_DRAW_REFERENCE: usize = Shape::COUNT + 4;
    pub const RETOPO_DRAW_DOTS: usize = Shape::COUNT + 5;
    pub const SYMMETRY_PLANE: usize = Shape::COUNT + 6;
    pub const SYMMETRY_OFFSET: usize = Shape::COUNT + 7;
    pub const RADIAL_COUNT: usize = Shape::COUNT + 8;
    pub const RADIAL_AXIS: usize = Shape::COUNT + 9;
    pub const RADIAL_DIRECTION: usize = Shape::COUNT + 10;
    pub const RADIAL_MIRROR: usize = Shape::COUNT + 11;
    pub const RADIAL_PHASE: usize = Shape::COUNT + 12;
    pub const RADIAL_CENTER: usize = Shape::COUNT + 13;
    pub const RETOPO_GUIDES: usize = Shape::COUNT + 14;
    pub const RETOPO_CONSTRUCTS: usize = Shape::COUNT + 15;
    /// instObjGroups: connected to a shading group's dagSetMembers when the whole mesh is its member.
    pub const INST_OBJ_GROUPS: usize = Shape::COUNT + 16;
    /// instObjGroups.objectGroups: the same for some of its faces.
    pub const OBJECT_GROUPS: usize = Shape::COUNT + 17;
    pub const COUNT: usize = Shape::COUNT + 18;

    pub fn make_type(base: Arc<NodeType>) -> NodeType {
        let stored = flags::INPUT | flags::STORABLE;
        let enum_def = |name: &str, def: i64, names: &[&str], doc: &str| AttrDef {
            name: name.into(),
            ty: PlugType::Enum,
            default_value: PlugValue::Int(def),
            flags: stored,
            enum_names: names.iter().map(|s| s.to_string()).collect(),
            doc: doc.into(),
            ..Default::default()
        };
        let string_def = |name: &str, doc: &str, alias: &str| AttrDef {
            name: name.into(),
            ty: PlugType::String,
            default_value: PlugValue::String(String::new()),
            flags: stored | flags::HIDDEN,
            doc: doc.into(),
            file_alias: alias.into(),
            ..Default::default()
        };
        let mut retopo_ref = mesh_attr(
            "retopoDrawReference",
            "",
            flags::INPUT | flags::HIDDEN,
            "Retopo Draw's reference surface: connected from its outMesh when this mesh is being drawn over another.",
        );
        retopo_ref.file_alias = "quadDrawReference".into();
        NodeType {
            name: "mesh".into(),
            icon: "cube".into(),
            base: Some(base),
            dag: true,
            own_attrs: vec![
                mesh_attr("inMesh", "i", stored | flags::HIDDEN, "Base mesh, or the output of a generator."),
                mesh_attr("outMesh", "o", flags::OUTPUT | flags::HIDDEN, "Final mesh, as drawn and exported."),
                enum_def(
                    "symmetry",
                    0,
                    &["off", "x", "y", "z", "radial"],
                    "Mirror plane (object space) every component edit is applied across, or radial: every edit repeated round \
                     radialAxis.",
                ),
                AttrDef {
                    name: "smoothPreview".into(),
                    ty: PlugType::Int,
                    default_value: PlugValue::Int(0),
                    flags: stored,
                    doc: "Display-only Catmull-Clark levels (the 3 key); exports stay unsmoothed.".into(),
                    min_value: Some(0.0),
                    max_value: Some(3.0),
                    ..Default::default()
                },
                retopo_ref,
                string_def(
                    "retopoDrawDots",
                    "Retopo Draw's unconnected dots, a JSON list of object-space [x, y, z].",
                    "quadDrawDots",
                ),
                enum_def("symmetryPlane", 0, &["pivot", "custom"], "Where the symmetry plane lies: through the pivot, or at symmetryOffset."),
                AttrDef {
                    name: "symmetryOffset".into(),
                    ty: PlugType::Float,
                    default_value: PlugValue::Float(0.0),
                    flags: stored,
                    doc: "The custom symmetry plane's position along the symmetry axis, object space, cm.".into(),
                    ..Default::default()
                },
                AttrDef {
                    name: "radialCount".into(),
                    ty: PlugType::Int,
                    default_value: PlugValue::Int(6),
                    flags: stored,
                    doc: "Radial symmetry: how many wedges repeat round the axis.".into(),
                    min_value: Some(2.0),
                    max_value: Some(MAX_RADIAL_COUNT as f64),
                    ..Default::default()
                },
                enum_def(
                    "radialAxis",
                    1,
                    &["x", "y", "z", "custom"],
                    "Radial symmetry's axis (object space); custom uses radialDirection.",
                ),
                vec3_attr(
                    "radialDirection",
                    "rdd",
                    DVec3::Y,
                    stored,
                    "Radial symmetry's axis when radialAxis is custom, object space.",
                ),
                AttrDef {
                    name: "radialMirror".into(),
                    ty: PlugType::Bool,
                    default_value: PlugValue::Bool(false),
                    flags: stored,
                    doc: "Radial symmetry also mirrors each wedge across a plane through the axis (dihedral symmetry: rims, \
                          flowers)."
                        .into(),
                    ..Default::default()
                },
                AttrDef {
                    name: "radialPhase".into(),
                    ty: PlugType::Float,
                    default_value: PlugValue::Float(0.0),
                    flags: stored,
                    doc: "Radial symmetry: degrees the wedge boundaries (and mirror planes) turn about the axis.".into(),
                    soft_min: Some(-180.0),
                    soft_max: Some(180.0),
                    ..Default::default()
                },
                vec3_attr(
                    "radialCenter",
                    "rdc",
                    DVec3::ZERO,
                    stored,
                    "Where the radial axis passes when symmetryPlane is custom, object space, cm.",
                ),
                string_def("retopoGuides", "Topology guides drawn with the Pencil: JSON, object space.", ""),
                string_def(
                    "retopoConstructs",
                    "Live topology constructs (patches, strips, cylinders...) and their settings: JSON, object space.",
                    "",
                ),
                crate::shading::inst_obj_groups(),
                crate::shading::object_groups(),
            ]
            .into_iter()
            .chain(crate::harold_nodes::shape_render_attrs())
            .collect(),
            create: Some(Arc::new(|| Box::new(MeshNode))),
            ..Default::default()
        }
    }
}

impl NodeBehavior for MeshNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        Shape::compute_matrices(d);
        let m = d.mesh(Self::IN_MESH).cloned();
        d.set_output(Self::OUT_MESH, PlugValue::Mesh(m));
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

pub type BuildFn = Arc<dyn Fn(&NodeData) -> Mesh + Send + Sync>;

/// Procedural source: keyable parameters in, a mesh out. One behaviour serves every primitive type.
#[derive(Clone)]
pub struct GeneratorNode {
    pub build: BuildFn,
    pub out_attr: usize,
}

impl NodeBehavior for GeneratorNode {
    fn compute(&mut self, _ctx: &EvalContext, d: &mut NodeData) {
        let build = self.build.clone();
        match modifiers::guarded(|| Ok(build(d))) {
            Ok(m) => d.set_output(self.out_attr, PlugValue::Mesh(Some(Arc::new(m)))),
            Err(e) => {
                log::warn!("{}: {e}", d.ty().name);
                d.set_output(self.out_attr, PlugValue::Mesh(None));
            }
        }
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The `symmetry` attribute value for radial symmetry, after off, x, y and z.
pub const SYMMETRY_RADIAL: i64 = 4;

/// The `symmetry` attribute across a set of nodes, for toggles: how many are meshes, how many have it on, and on which
/// axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymmetryState {
    pub meshes: i32,
    pub on: i32,
    /// 0-2 (mirror) or RADIAL_STATE when every mesh that has it on agrees, else -1.
    pub axis: i32,
    /// With axis RADIAL_STATE: the count when they all agree, else 0.
    pub radial_count: i32,
    pub radial_mirror: bool,
}

impl SymmetryState {
    pub const RADIAL_STATE: i32 = 3;
}

impl Default for SymmetryState {
    fn default() -> Self {
        Self { meshes: 0, on: 0, axis: -1, radial_count: 0, radial_mirror: false }
    }
}

pub fn symmetry_state(g: &Graph, nodes: &[NodeId]) -> SymmetryState {
    let mut s = SymmetryState::default();
    let (mut mixed, mut mixed_radial) = (false, false);
    for &id in nodes {
        let Some(id) = crate::components::as_mesh(g, id) else { continue };
        let n = g.node(id).unwrap();
        s.meshes += 1;
        let v = n.int(MeshNode::SYMMETRY);
        if !(1..=SYMMETRY_RADIAL).contains(&v) {
            continue;
        }
        let axis = if v == SYMMETRY_RADIAL { SymmetryState::RADIAL_STATE } else { (v - 1) as i32 };
        let count = if v == SYMMETRY_RADIAL { n.int(MeshNode::RADIAL_COUNT) as i32 } else { 0 };
        let mirror = v == SYMMETRY_RADIAL && n.bool(MeshNode::RADIAL_MIRROR);
        mixed = mixed || (s.on > 0 && s.axis != axis);
        mixed_radial = mixed_radial || (s.on > 0 && (s.radial_count != count || s.radial_mirror != mirror));
        s.axis = axis;
        s.radial_count = count;
        s.radial_mirror = mirror;
        s.on += 1;
    }
    if mixed {
        s.axis = -1;
    }
    if mixed || mixed_radial {
        s.radial_count = 0;
        s.radial_mirror = false;
    }
    s
}

/// Where a mesh's symmetry plane for axis (0-2) lies along that axis, in object space: through the object's pivot, or
/// at symmetryOffset when symmetryPlane is custom (set from an edge selection).
pub fn symmetry_offset(g: &Graph, id: NodeId, axis: i32) -> f64 {
    if !(0..=2).contains(&axis) {
        return 0.0;
    }
    let Some(shape) = crate::components::as_mesh(g, id) else { return 0.0 };
    let mesh = g.node(shape).unwrap();
    if mesh.int(MeshNode::SYMMETRY_PLANE) == 1 {
        return mesh.float(MeshNode::SYMMETRY_OFFSET);
    }
    if mesh.int(MeshNode::SYMMETRY_PLANE) == SYMMETRY_PLANE_WORLD {
        return world_symmetry(mesh, axis).offset;
    }
    object_pivot(g, shape)[axis as usize]
}

/// The pivot of a mesh shape's transform, in the shape's object space.
pub fn object_pivot(g: &Graph, shape: NodeId) -> DVec3 {
    let xf = crate::dag::transform_of(g, shape);
    match g.node(xf) {
        Some(t) if t.ty().is_a("transform") => t.vec3(Transform::ROTATE_PIVOT),
        _ => DVec3::ZERO,
    }
}

/// symmetryPlane's value for a plane through the world origin across a world axis.
pub const SYMMETRY_PLANE_WORLD: i64 = 2;

/// The world plane across an axis, in the mesh's own space: on one of its axes when it can be, so edits still go into
/// history, else a plane at its angle.
fn world_symmetry(mesh: &crate::node::Node, axis: i32) -> SymmetrySpec {
    let w = mesh.matrix(Transform::WORLD_MATRIX);
    let mut n = glam::DVec3::ZERO;
    n[axis as usize] = 1.0;
    let linear = glam::DMat3::from_mat4(w);
    let local = linear.transpose() * n;
    let len = local.length().max(1e-12);
    let normal = local / len;
    let offset = -n.dot(w.w_axis.truncate()) / len;
    for k in 0..3 {
        if normal[k].abs() > 1.0 - 1e-9 {
            return SymmetrySpec { axis: k as i32, offset: offset * normal[k].signum(), radial: None, plane: None, free_seam: false };
        }
    }
    let a = normal.abs();
    let k = if a.x >= a.y && a.x >= a.z { 0 } else if a.y >= a.z { 1 } else { 2 };
    SymmetrySpec { axis: k, offset: 0.0, radial: None, plane: Some(aztek_geo::symmetry::SymmetryPlane { normal, offset }), free_seam: false }
}

/// A mesh's symmetry setting in object space, None when off: the mirror plane symmetry x/y/z sets, or the radial
/// group symmetry radial sets (radialCount turns about radialAxis through the pivot, or through radialCenter when
/// symmetryPlane is custom, with a mirror plane per wedge when radialMirror is on).
pub fn symmetry_spec(g: &Graph, id: NodeId) -> Option<SymmetrySpec> {
    let shape = crate::components::as_mesh(g, id)?;
    let mesh = g.node(shape)?;
    let v = mesh.int(MeshNode::SYMMETRY);
    if (1..=3).contains(&v) {
        let axis = (v - 1) as i32;
        if mesh.int(MeshNode::SYMMETRY_PLANE) == SYMMETRY_PLANE_WORLD {
            return Some(SymmetrySpec { free_seam: !g.move_opts.preserve_seam, ..world_symmetry(mesh, axis) });
        }
        return Some(SymmetrySpec { axis, offset: symmetry_offset(g, shape, axis), radial: None, plane: None, free_seam: !g.move_opts.preserve_seam });
    }
    if v != SYMMETRY_RADIAL {
        return None;
    }
    let count = mesh.int(MeshNode::RADIAL_COUNT).clamp(2, MAX_RADIAL_COUNT as i64) as i32;
    let mirror = mesh.bool(MeshNode::RADIAL_MIRROR);
    let phase = mesh.float(MeshNode::RADIAL_PHASE);
    let origin = if mesh.int(MeshNode::SYMMETRY_PLANE) == 1 {
        mesh.vec3(MeshNode::RADIAL_CENTER)
    } else {
        object_pivot(g, shape)
    };
    let axis = mesh.int(MeshNode::RADIAL_AXIS);
    if (0..=2).contains(&axis) {
        return Some(SymmetrySpec { axis: axis as i32, offset: 0.0, free_seam: false, plane: None, radial: Some(axis_radial(axis as i32, origin, count, mirror, phase)) });
    }
    let dir = mesh.vec3(MeshNode::RADIAL_DIRECTION);
    Some(SymmetrySpec {
        axis: -1,
        plane: None,
        free_seam: false,
        offset: 0.0,
        radial: Some(RadialSymmetry::new(origin, dir, count, mirror, radial_reference(dir, phase))),
    })
}

/// symmetry_spec(), or with symmetry off the x plane through the pivot: where a brush's own mirror reflects.
pub fn paint_symmetry_spec(g: &Graph, id: NodeId) -> SymmetrySpec {
    symmetry_spec(g, id).unwrap_or_else(|| SymmetrySpec { free_seam: false, plane: None, axis: 0, offset: symmetry_offset(g, id, 0), radial: None })
}

/// The mesh's persistent symmetry map, stored onto mesh if it has none yet; None when symmetry is off.
pub fn symmetry_map_for(g: &Graph, id: NodeId, mesh: &mut Mesh, sync: Option<&mut SymmetrySync>) -> Option<SymmetryMap> {
    let spec = symmetry_spec(g, id)?;
    let tolerance = (mesh.bounds().size().length() * 1e-5).max(1e-4);
    Some(SymmetryMap::persistent(mesh, &spec, tolerance, sync))
}

/// "x", "y", "z", or for radial symmetry e.g. "radial 6" / "radial 6 + mirror": for messages.
pub fn symmetry_label(spec: &SymmetrySpec) -> String {
    match &spec.radial {
        None => ((b'x' + spec.axis.clamp(0, 2) as u8) as char).to_string(),
        Some(r) => format!("radial {}{}", r.count(), if r.mirror() { " + mirror" } else { "" }),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UpAxis {
    #[default]
    Y,
    Z,
}

/// World up as the scene's up axis pref sets it: +Y, or +Z in a Z-up scene.
pub fn scene_up(g: &Graph) -> DVec3 {
    if g.up_axis == UpAxis::Z {
        DVec3::Z
    } else {
        DVec3::Y
    }
}

// ---------------------------------------------------------------------------------------------------------------------
// Primitives

/// A size whose default for new primitives (sized to fit a 10 cm cube) differs from what scene files leaving it out
/// mean.
fn size_attr(name: &str, def: f64, file_default: f64, doc: &str) -> AttrDef {
    AttrDef { file_default: Some(PlugValue::Float(file_default)), ..float_attr(name, def, doc, Some(0.0), None) }
}

/// lo matches the least the generator reads it as; hi keeps a typo from building a mesh that never finishes.
fn int_attr(name: &str, def: i64, doc: &str, lo: f64, hi: f64) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Int,
        default_value: PlugValue::Int(def),
        flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
        doc: doc.into(),
        min_value: Some(lo),
        max_value: Some(hi),
        soft_max: Some(hi.min(100.0)),
        ..Default::default()
    }
}

fn int1(name: &str, def: i64, doc: &str) -> AttrDef {
    int_attr(name, def, doc, 1.0, 1000.0)
}

/// attr answering to other names too; the first of them is what older scene files stored it as.
fn known_as(mut a: AttrDef, short: &str, names: &[&str]) -> AttrDef {
    a.short_name = short.into();
    a.aliases = names.iter().map(|s| s.to_string()).collect();
    if let Some(first) = names.first() {
        a.file_alias = first.to_string();
    }
    a
}

/// A primitive's extra flag that sets one of its attributes: another spelling of it.
struct FlagAlias {
    long: &'static str,
    short: &'static str,
    ty: ArgType,
}

fn int_alias(long: &'static str, short: &'static str) -> FlagAlias {
    FlagAlias { long, short, ty: ArgType::Int }
}

fn keyable_bool(name: &str, def: bool, doc: &str) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Bool,
        default_value: PlugValue::Bool(def),
        flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
        doc: doc.into(),
        ..Default::default()
    }
}

fn f(n: &NodeData, a: &str) -> f64 {
    n.float(n.ty().attr_index(a).unwrap())
}
fn count(n: &NodeData, a: &str, lo: i64) -> i32 {
    n.int(n.ty().attr_index(a).unwrap()).max(lo) as i32
}
fn b(n: &NodeData, a: &str) -> bool {
    n.bool(n.ty().attr_index(a).unwrap())
}

fn height_fraction() -> AttrDef {
    float_attr(
        "fraction",
        1.0,
        "Portion of the height kept, from the top: 1 is whole, 0.5 a dome open at the bottom.",
        Some(0.01),
        Some(1.0),
    )
}

fn sweep_fraction() -> AttrDef {
    float_attr("fraction", 1.0, "Portion of a full turn swept around the up axis: 1 is whole, 0.5 half.", Some(0.01), Some(1.0))
}

fn cut_caps() -> AttrDef {
    keyable_bool("cutCaps", true, "Close the flat ends a partial sweep leaves.")
}

fn enum_keyable(name: &str, def: i64, names: &[&str], doc: &str, file_default: i64) -> AttrDef {
    AttrDef {
        name: name.into(),
        ty: PlugType::Enum,
        default_value: PlugValue::Int(def),
        flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
        enum_names: names.iter().map(|s| s.to_string()).collect(),
        doc: doc.into(),
        file_default: Some(PlugValue::Int(file_default)),
        ..Default::default()
    }
}

fn cap_type_attr() -> AttrDef {
    // Scene files saved before capType had n-gon caps.
    enum_keyable(
        "capType",
        0,
        &["quads", "fan", "ngon", "none"],
        "How each flat end is filled inside its rings: quads (a grid joined to the rim by a ring, one triangle when \
         subdivisionsAxis is odd), fan (triangles to the centre), ngon (one polygon) or none (left open).",
        2,
    )
}

fn disc_cap_type_attr() -> AttrDef {
    enum_keyable(
        "capType",
        0,
        &["quads", "fan", "ngon"],
        "How the innermost ring is filled: quads (a grid joined to it by a ring, one triangle when subdivisionsAxis is \
         odd), fan (triangles to the centre) or ngon (one polygon). subdivisionsRings 0 is always one n-gon.",
        1,
    )
}

fn cap_fill(n: &NodeData) -> CapFill {
    match n.int(n.ty().attr_index("capType").unwrap()).clamp(0, 3) {
        0 => CapFill::Quads,
        1 => CapFill::Fan,
        2 => CapFill::Ngon,
        _ => CapFill::None,
    }
}

fn frac(n: &NodeData) -> f64 {
    f(n, "fraction").clamp(0.01, 1.0)
}

pub const UP_AXES: [&str; 6] = ["x", "y", "z", "-x", "-y", "-z"];

fn up_axis_attr() -> AttrDef {
    AttrDef {
        name: "upAxis".into(),
        ty: PlugType::Enum,
        default_value: PlugValue::Int(1),
        flags: flags::INPUT | flags::STORABLE | flags::KEYABLE,
        enum_names: UP_AXES.iter().map(|s| s.to_string()).collect(),
        doc: "The direction the primitive's up (its height, poles or face normal) points in object space.".into(),
        ..Default::default()
    }
}

/// Turns a mesh built up +Y so that its up points along axis (an upAxis index), keeping it a rotation so winding
/// holds.
fn orient_up(m: &mut Mesh, axis: i64) {
    if axis == 1 || !(0..UP_AXES.len() as i64).contains(&axis) {
        return;
    }
    let turn = |p: DVec3| match axis {
        0 => DVec3::new(p.y, -p.x, p.z),
        2 => DVec3::new(p.x, -p.z, p.y),
        3 => DVec3::new(-p.y, p.x, p.z),
        4 => DVec3::new(p.x, -p.y, -p.z),
        _ => DVec3::new(p.x, p.z, -p.y),
    };
    for p in 0..m.point_capacity() {
        if m.point_valid(p) {
            m.set_position(p, turn(m.position(p)));
        }
    }
}

struct Primitive {
    /// Other flags of its command, each an alias of an attribute.
    flags: Vec<FlagAlias>,
    /// Node type, also the command name.
    ty: &'static str,
    /// Default object name.
    display: &'static str,
    attrs: Vec<AttrDef>,
    build: BuildFn,
    /// Its fraction cuts the height, so a partial one is placed on a surface by its open base.
    domed: bool,
}

fn primitives() -> Vec<Primitive> {
    let prim = |ty, display, attrs, build: BuildFn, domed| Primitive { flags: Vec::new(), ty, display, attrs, build, domed };
    let sxyz = || vec![int_alias("subdivisionsX", "sx"), int_alias("subdivisionsY", "sy"), int_alias("subdivisionsZ", "sz")];
    let sxy = || vec![int_alias("subdivisionsX", "sx"), int_alias("subdivisionsY", "sy")];
    let list: Vec<Primitive> = vec![
        prim(
            "polyCube",
            "Cube",
            vec![
                size_attr("width", 1.0, 100.0, "Size along X, cm."),
                size_attr("height", 1.0, 100.0, "Size along the up axis, cm."),
                size_attr("depth", 1.0, 100.0, "Size along Z, cm."),
                known_as(int1("subdivisionsWidth", 1, "Segments across."), "sw", &["subdivisionsX", "sx"]),
                known_as(int1("subdivisionsHeight", 1, "Segments up."), "sh", &["subdivisionsY", "sy"]),
                known_as(int1("subdivisionsDepth", 1, "Segments deep."), "sd", &["subdivisionsZ", "sz"]),
                keyable_bool("baseAtOrigin", false, "Sit on the ground instead of centring."),
            ],
            Arc::new(|n| {
                make_cube(&CubeOptions {
                    width: f(n, "width"),
                    height: f(n, "height"),
                    depth: f(n, "depth"),
                    subdiv_x: count(n, "subdivisionsWidth", 1),
                    subdiv_y: count(n, "subdivisionsHeight", 1),
                    subdiv_z: count(n, "subdivisionsDepth", 1),
                    base_at_origin: b(n, "baseAtOrigin"),
                })
            }),
            false,
        ),
        prim(
            "polySphere",
            "Sphere",
            vec![
                size_attr("radius", 1.0, 50.0, "cm."),
                known_as(int_attr("subdivisionsAxis", 20, "Segments around.", 3.0, 1000.0), "sa", &["subdivisionsX", "sx"]),
                known_as(int_attr("subdivisionsHeight", 20, "Segments pole to pole.", 2.0, 1000.0), "sh", &["subdivisionsY", "sy"]),
                keyable_bool("baseAtOrigin", false, "Sit on the ground."),
                height_fraction(),
            ],
            Arc::new(|n| {
                make_sphere(&SphereOptions {
                    radius: f(n, "radius"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_height: count(n, "subdivisionsHeight", 2),
                    base_at_origin: b(n, "baseAtOrigin"),
                    fraction: frac(n),
                })
            }),
            true,
        ),
        prim(
            "polyCylinder",
            "Cylinder",
            vec![
                size_attr("radius", 1.0, 50.0, "cm."),
                size_attr("height", 2.0, 100.0, "cm."),
                known_as(int_attr("subdivisionsAxis", 20, "Segments around.", 3.0, 1000.0), "sa", &["subdivisionsX", "sx"]),
                known_as(int1("subdivisionsHeight", 1, "Segments up."), "sh", &["subdivisionsY", "sy"]),
                known_as(int_attr("subdivisionsCaps", 0, "Rings of quads around each cap's fill; 0 and 1 both leave just the fill.", 0.0, 1000.0), "sc", &["subdivisionsZ", "sz"]),
                keyable_bool("roundCap", false, "Accepted; the caps stay flat."),
                cap_type_attr(),
                keyable_bool("caps", true, "Close the ends; off leaves them open whatever capType says."),
                keyable_bool("baseAtOrigin", false, "Sit on the ground."),
                sweep_fraction(),
                cut_caps(),
            ],
            Arc::new(|n| {
                make_cylinder(&CylinderOptions {
                    radius: f(n, "radius"),
                    height: f(n, "height"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_height: count(n, "subdivisionsHeight", 1),
                    subdiv_caps: count(n, "subdivisionsCaps", 0),
                    caps: b(n, "caps"),
                    base_at_origin: b(n, "baseAtOrigin"),
                    fraction: frac(n),
                    cut_caps: b(n, "cutCaps"),
                    cap_fill: cap_fill(n),
                })
            }),
            false,
        ),
        prim(
            "polyCapsule",
            "Capsule",
            vec![
                size_attr("radius", 1.0, 50.0, "cm."),
                size_attr("height", 2.0, 200.0, "End to end, caps included, cm."),
                int_attr("subdivisionsAxis", 20, "Segments around.", 3.0, 1000.0),
                int1("subdivisionsHeight", 1, "Segments along the straight part."),
                int1("subdivisionsCaps", 6, "Rings on each rounded end."),
                keyable_bool("baseAtOrigin", false, "Sit on the ground."),
            ],
            Arc::new(|n| {
                make_capsule(&CapsuleOptions {
                    radius: f(n, "radius"),
                    height: f(n, "height"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_height: count(n, "subdivisionsHeight", 1),
                    subdiv_caps: count(n, "subdivisionsCaps", 1),
                    base_at_origin: b(n, "baseAtOrigin"),
                })
            }),
            false,
        ),
        prim(
            "polyCone",
            "Cone",
            vec![
                size_attr("radius", 1.0, 50.0, "cm."),
                size_attr("height", 2.0, 100.0, "cm."),
                known_as(int_attr("subdivisionsAxis", 20, "Segments around.", 3.0, 1000.0), "sa", &["subdivisionsX", "sx"]),
                known_as(int1("subdivisionsHeight", 1, "Segments up."), "sh", &["subdivisionsY", "sy"]),
                known_as(int_attr("subdivisionsCap", 0, "Rings of quads around the base's fill; 0 and 1 both leave just the fill.", 0.0, 1000.0), "sc", &["subdivisionsZ", "sz"]),
                keyable_bool("roundCap", false, "Accepted; the base stays flat."),
                cap_type_attr(),
                keyable_bool("baseAtOrigin", false, "Sit on the ground."),
                sweep_fraction(),
                cut_caps(),
                keyable_bool("capTip", false, "Cut the point off at tipRadius and fill it as capType, so quads leave no triangles at the tip."),
                float_attr("tipRadius", 0.05, "Radius of the capped tip, cm; small keeps it looking pointed.", Some(0.0), None),
            ],
            Arc::new(|n| {
                make_cone(&ConeOptions {
                    radius: f(n, "radius"),
                    height: f(n, "height"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_height: count(n, "subdivisionsHeight", 1),
                    subdiv_cap: count(n, "subdivisionsCap", 0),
                    base_at_origin: b(n, "baseAtOrigin"),
                    fraction: frac(n),
                    cut_caps: b(n, "cutCaps"),
                    cap_fill: cap_fill(n),
                    cap_tip: b(n, "capTip"),
                    tip_radius: f(n, "tipRadius"),
                })
            }),
            false,
        ),
        prim(
            "polyPlane",
            "Plane",
            vec![
                size_attr("width", 1.0, 100.0, "Size along X, cm."),
                known_as(size_attr("height", 1.0, 100.0, "Size along Z, cm."), "h", &["depth"]),
                known_as(int1("subdivisionsWidth", 10, "Segments across."), "sw", &["subdivisionsX", "sx"]),
                known_as(int1("subdivisionsHeight", 10, "Segments deep."), "sh", &["subdivisionsZ", "subdivisionsY", "sy", "sz"]),
            ],
            Arc::new(|n| {
                make_plane(&PlaneOptions {
                    width: f(n, "width"),
                    depth: f(n, "height"),
                    subdiv_x: count(n, "subdivisionsWidth", 1),
                    subdiv_z: count(n, "subdivisionsHeight", 1),
                })
            }),
            false,
        ),
        prim(
            "polyTorus",
            "Torus",
            vec![
                size_attr("radius", 1.0, 50.0, "Ring radius, cm."),
                size_attr("sectionRadius", 0.5, 20.0, "Tube radius, cm."),
                known_as(int_attr("subdivisionsAxis", 20, "Segments around the ring.", 3.0, 1000.0), "sa", &["subdivisionsX", "sx"]),
                known_as(int_attr("subdivisionsHeight", 20, "Segments around the tube.", 3.0, 1000.0), "sh", &["subdivisionsY", "sy"]),
                known_as(float_attr("twist", 0.0, "Turns the tube's sections about the ring, degrees.", None, None), "tw", &[]),
                sweep_fraction(),
                cut_caps(),
            ],
            Arc::new(|n| {
                let mut m = make_torus(&TorusOptions {
                    radius: f(n, "radius"),
                    section_radius: f(n, "sectionRadius"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_height: count(n, "subdivisionsHeight", 3),
                    fraction: frac(n),
                    cut_caps: b(n, "cutCaps"),
                });
                let twist = f(n, "twist");
                if twist != 0.0 {
                    let radius = f(n, "radius");
                    for p in 0..m.point_capacity() {
                        if m.point_valid(p) {
                            let q = m.position(p);
                            let out = DVec3::new(q.x, 0.0, q.z).normalize_or_zero();
                            let center = out * radius;
                            let turn = DQuat::from_axis_angle(DVec3::Y.cross(out).normalize_or_zero(), twist.to_radians());
                            m.set_position(p, center + turn * (q - center));
                        }
                    }
                }
                m
            }),
            false,
        ),
        prim(
            "polyDisc",
            "Disc",
            vec![
                size_attr("radius", 1.0, 50.0, "cm."),
                known_as(int_attr("subdivisionsAxis", 20, "Segments around.", 3.0, 1000.0), "sa", &["sides"]),
                known_as(int_attr("subdivisionsRings", 1, "Rings; 0 is one n-gon.", 0.0, 1000.0), "sr", &["subdivisions"]),
                sweep_fraction(),
                disc_cap_type_attr(),
            ],
            Arc::new(|n| {
                make_disc(&DiscOptions {
                    radius: f(n, "radius"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_rings: count(n, "subdivisionsRings", 0),
                    fraction: frac(n),
                    cap_fill: cap_fill(n),
                })
            }),
            false,
        ),
        prim(
            "polyPipe",
            "Pipe",
            vec![
                size_attr("radius", 1.0, 50.0, "Outer radius, cm."),
                size_attr("thickness", 0.5, 10.0, "Wall thickness, cm."),
                size_attr("height", 2.0, 100.0, "cm."),
                known_as(int_attr("subdivisionsAxis", 20, "Segments around.", 3.0, 1000.0), "sa", &["subdivisionsX", "sx"]),
                known_as(int1("subdivisionsHeight", 1, "Segments up."), "sh", &["subdivisionsY", "sy"]),
                known_as(int1("subdivisionsCaps", 1, "Accepted; each end is one ring."), "sc", &["subdivisionsZ", "sz"]),
                keyable_bool("roundCap", false, "Accepted; the ends stay flat."),
                keyable_bool("baseAtOrigin", false, "Sit on the ground."),
                sweep_fraction(),
                cut_caps(),
            ],
            Arc::new(|n| {
                make_pipe(&PipeOptions {
                    radius: f(n, "radius"),
                    thickness: f(n, "thickness"),
                    height: f(n, "height"),
                    subdiv_axis: count(n, "subdivisionsAxis", 3),
                    subdiv_height: count(n, "subdivisionsHeight", 1),
                    base_at_origin: b(n, "baseAtOrigin"),
                    fraction: frac(n),
                    cut_caps: b(n, "cutCaps"),
                })
            }),
            false,
        ),
        prim(
            "polyQuadSphere",
            "QuadSphere",
            vec![size_attr("radius", 1.0, 50.0, "cm."), int1("subdivisions", 3, "Segments per cube face edge."), height_fraction()],
            Arc::new(|n| {
                make_quad_sphere(&QuadSphereOptions { radius: f(n, "radius"), subdiv: count(n, "subdivisions", 1), fraction: frac(n) })
            }),
            true,
        ),
        prim(
            "polyIcoSphere",
            "IcoSphere",
            vec![
                size_attr("radius", 1.0, 50.0, "cm."),
                int_attr("subdivisions", 3, "1 is an icosahedron; each level splits every triangle in four.", 1.0, 7.0),
                keyable_bool("baseAtOrigin", false, "Sit on the ground."),
                height_fraction(),
            ],
            Arc::new(|n| {
                make_ico_sphere(&IcoSphereOptions {
                    radius: f(n, "radius"),
                    subdivisions: count(n, "subdivisions", 1),
                    base_at_origin: b(n, "baseAtOrigin"),
                    fraction: frac(n),
                })
            }),
            true,
        ),
        prim(
            "polyPrism",
            "Prism",
            vec![
                known_as(size_attr("length", 2.0, 2.0, "Height, cm."), "l", &[]),
                known_as(size_attr("sideLength", 2.0, 2.0, "Length of each side of the ends, cm."), "w", &["width"]),
                known_as(int_attr("numberOfSides", 3, "Sides.", 3.0, 1000.0), "ns", &["numderOfSides", "nsi"]),
                known_as(int1("subdivisionsHeight", 1, "Segments up."), "sh", &[]),
                known_as(int_attr("subdivisionsCaps", 2, "Rings on each end.", 0.0, 1000.0), "sc", &[]),
            ],
            Arc::new(|n| {
                make_prism(f(n, "length"), f(n, "sideLength"), count(n, "numberOfSides", 3), count(n, "subdivisionsHeight", 1), count(n, "subdivisionsCaps", 0))
            }),
            false,
        ),
        prim(
            "polyPyramid",
            "Pyramid",
            vec![
                known_as(size_attr("sideLength", 1.0, 1.0, "Length of each side, cm."), "w", &["width"]),
                known_as(int_attr("numberOfSides", 4, "Sides of the base.", 3.0, 1000.0), "ns", &["numderOfSides", "nsi"]),
                known_as(int1("subdivisionsHeight", 1, "Segments up."), "sh", &[]),
                known_as(int_attr("subdivisionsCaps", 0, "Rings on the base.", 0.0, 1000.0), "sc", &[]),
            ],
            Arc::new(|n| make_pyramid(f(n, "sideLength"), count(n, "numberOfSides", 3), count(n, "subdivisionsHeight", 1), count(n, "subdivisionsCaps", 0))),
            false,
        ),
        prim(
            "polyHelix",
            "Helix",
            vec![
                known_as(float_attr("coils", 3.0, "Turns.", Some(0.0), None), "c", &[]),
                known_as(size_attr("height", 2.0, 2.0, "cm."), "h", &[]),
                known_as(size_attr("width", 2.0, 2.0, "Across the coil, cm."), "w", &[]),
                known_as(size_attr("radius", 0.4, 0.4, "Tube radius, cm."), "r", &[]),
                known_as(int_attr("direction", 1, "0 clockwise, 1 counterclockwise.", 0.0, 1.0), "d", &[]),
                known_as(int_attr("subdivisionsAxis", 8, "Segments round the tube.", 3.0, 1000.0), "sa", &[]),
                known_as(int1("subdivisionsCoil", 50, "Segments per coil."), "sco", &[]),
                known_as(int_attr("subdivisionsCaps", 0, "Rings on each end.", 0.0, 1000.0), "sc", &[]),
                keyable_bool("roundCap", false, "Accepted; the ends stay flat."),
            ],
            Arc::new(|n| {
                make_helix(
                    f(n, "coils"),
                    f(n, "height"),
                    f(n, "width"),
                    f(n, "radius"),
                    count(n, "direction", 0),
                    count(n, "subdivisionsAxis", 3),
                    count(n, "subdivisionsCoil", 1),
                    count(n, "subdivisionsCaps", 0),
                )
            }),
            false,
        ),
        prim(
            "polyPlatonicSolid",
            "Solid",
            vec![
                known_as(size_attr("radius", 1.0, 1.0, "Centre to each point, cm."), "r", &[]),
                known_as(size_attr("sideLength", 1.0, 1.0, "Accepted; the radius sets the size."), "l", &[]),
                known_as(int_attr("solidType", 0, "0 dodecahedron, 1 icosahedron, 2 octahedron, 3 tetrahedron.", 0.0, 3.0), "st", &[]),
            ],
            Arc::new(|n| make_platonic(count(n, "solidType", 0), f(n, "radius"))),
            false,
        ),
        prim(
            "polyPrimitive",
            "Solid",
            vec![
                known_as(size_attr("radius", 1.0, 1.0, "Centre to each point, cm."), "r", &[]),
                known_as(size_attr("sideLength", 1.0, 1.0, "Accepted; the radius sets the size."), "l", &[]),
                known_as(int_attr("polyType", 0, "0 soccer ball.", 0.0, 0.0), "pt", &[]),
            ],
            Arc::new(|n| make_soccer_ball(f(n, "radius"))),
            false,
        ),
        prim(
            "polySoccerBall",
            "SoccerBall",
            vec![known_as(size_attr("radius", 1.0, 1.0, "Centre to each point, cm."), "r", &[])],
            Arc::new(|n| make_soccer_ball(f(n, "radius"))),
            false,
        ),
        prim(
            "polySuperShape",
            "SuperShape",
            vec![
                known_as(size_attr("radius", 1.0, 1.0, "cm."), "r", &[]),
                known_as(int_attr("horizontalDivisions", 20, "Segments around.", 3.0, 1000.0), "hd", &[]),
                known_as(int_attr("verticalDivisions", 20, "Segments pole to pole.", 2.0, 1000.0), "vd", &[]),
                known_as(float_attr("ultraM", 4.0, "Lobes around.", None, None), "um", &[]),
                known_as(float_attr("ultraN1", 2.0, "Shape around: first exponent.", Some(0.01), None), "un1", &[]),
                known_as(float_attr("ultraN2", 2.0, "Shape around: second exponent.", None, None), "un2", &[]),
                known_as(float_attr("ultraN3", 2.0, "Shape around: third exponent.", None, None), "un3", &[]),
                known_as(float_attr("ultraVM", 4.0, "Lobes pole to pole.", None, None), "uvm", &[]),
                known_as(float_attr("ultraVN1", 2.0, "Shape pole to pole: first exponent.", Some(0.01), None), "uvn1", &[]),
                known_as(float_attr("ultraVN2", 2.0, "Shape pole to pole: second exponent.", None, None), "uvn2", &[]),
                known_as(float_attr("ultraVN3", 2.0, "Shape pole to pole: third exponent.", None, None), "uvn3", &[]),
            ],
            Arc::new(|n| {
                make_super_shape(
                    f(n, "radius"),
                    count(n, "horizontalDivisions", 3),
                    count(n, "verticalDivisions", 2),
                    [f(n, "ultraM"), f(n, "ultraN1"), f(n, "ultraN2"), f(n, "ultraN3")],
                    [f(n, "ultraVM"), f(n, "ultraVN1"), f(n, "ultraVN2"), f(n, "ultraVN3")],
                )
            }),
            false,
        ),
    ];
    list.into_iter()
        .map(|mut p| {
            p.flags = match p.ty {
                "polyCube" | "polyCylinder" | "polyCone" | "polyPipe" => sxyz(),
                "polySphere" | "polyTorus" => sxy(),
                "polyPlane" => {
                    let mut v = sxyz();
                    v.push(FlagAlias { long: "depth", short: "", ty: ArgType::Float });
                    v
                }
                "polyDisc" => vec![int_alias("sides", "s"), int_alias("subdivisions", "sd"), int_alias("subdivisionMode", "sm")],
                _ => Vec::new(),
            };
            p
        })
        .collect()
}

pub fn register_types() {
    if types::find("mesh").is_some() {
        return;
    }
    let shape = types::find("shape").expect("shape registered first");
    types::register_type(MeshNode::make_type(shape)).expect("mesh");
    for p in primitives() {
        let mut own = p.attrs;
        own.push(up_axis_attr());
        own.push(vec3_attr("axis", "ax", DVec3::Y, flags::INPUT | flags::STORABLE | flags::KEYABLE, "The direction the primitive's up points, as a vector."));
        own.push(int_attr("createUVs", 1, "0 leaves the mesh without UVs.", 0.0, 4.0));
        own.push(int_attr("texture", 1, "How UVs are laid out; accepted.", 0.0, 2.0));
        own.push(mesh_attr("outMesh", "o", flags::OUTPUT | flags::HIDDEN, "Generated mesh."));
        let out = own.len() - 1;
        let shape = p.build;
        let build: BuildFn = Arc::new(move |n| {
            let mut m = shape(n);
            let axis = n.vec3(n.ty().attr_index("axis").unwrap());
            if (axis - DVec3::Y).length_squared() > 1e-12 && axis.length_squared() > 1e-12 {
                let turn = DQuat::from_rotation_arc(DVec3::Y, axis.normalize());
                for p in 0..m.point_capacity() {
                    if m.point_valid(p) {
                        m.set_position(p, turn * m.position(p));
                    }
                }
            } else {
                orient_up(&mut m, n.int(n.ty().attr_index("upAxis").unwrap()));
            }
            if n.int(n.ty().attr_index("createUVs").unwrap()) == 0 {
                m.corner_attrs_mut().remove(names::uv(0).as_str());
            }
            m
        });
        let t = NodeType {
            name: p.ty.into(),
            icon: "cube-plus".into(),
            doc: format!(
                "Builds the {} primitive from these settings. Editing its components bakes it into a stored base mesh.",
                p.display
            ),
            own_attrs: own,
            create: Some(Arc::new(move || Box::new(GeneratorNode { build: build.clone(), out_attr: out }))),
            ..Default::default()
        };
        types::register_type(t).expect("primitive");
    }
}

/// The meshes a whole-object command acts on: the positional objects (or the selection), each mesh as itself and each
/// other node (a group) as the meshes under it, without repeats. Selected non-meshes with no meshes under them are
/// skipped; a named one fails, as does an empty result.
pub fn mesh_objects(s: &Session, a: &Args, cmd: &str) -> CmdResult<Vec<String>> {
    let g = &s.graph;
    let named = !a.positional.is_empty();
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for name in objects(s, a) {
        let Some(id) = g.find(&name)? else { fail!("{cmd}: no object named {name}") };
        // Named through their transforms, as the reference names objects.
        let mut add = |id: NodeId, out: &mut Vec<String>| {
            let id = crate::dag::transform_of(g, id);
            if seen.insert(id) {
                out.push(g.path(id));
            }
        };
        if g.node(id).unwrap().ty().is_a("mesh") {
            add(id, &mut out);
            continue;
        }
        let mut any = false;
        for d in g.descendants(id) {
            if g.node(d).is_some_and(|m| m.ty().is_a("mesh")) {
                add(d, &mut out);
                any = true;
            }
        }
        if !any && named {
            fail!("{cmd}: {name} is not a mesh and has no meshes under it");
        }
    }
    if out.is_empty() {
        fail!("{cmd}: select a mesh or a group of meshes");
    }
    Ok(out)
}

/// Stands a mesh on a surface point: its object space scene up (scene_up) turned to normal (left alone when normal is
/// zero) and its origin on position, or, for a sphere primitive cut short by its fraction, its open base on position.
pub fn place_primitive(g: &mut Graph, xform: NodeId, position: DVec3, normal: DVec3) -> CmdResult<()> {
    let xform = crate::dag::transform_of(g, xform);
    let mesh = crate::dag::shape_of_type(g, xform, "mesh").unwrap_or(xform);
    let Some(node) = g.node(xform) else { return Ok(()) };
    let mut at = position;
    if normal.length() > 1e-12 {
        let up = normal.normalize();
        let r = DMat3::from_quat(DQuat::from_rotation_arc(scene_up(g), up));
        let order = RotateOrder::from_index(node.int(Transform::ROTATE_ORDER));
        g.set_value(PlugRef::new(xform, Transform::ROTATE as i32), PlugValue::Vec3(matrix_to_euler(&r, order)))?;
        let src = g.source(&PlugRef::new(mesh, MeshNode::IN_MESH as i32));
        let domed_types: Vec<&str> = primitives().into_iter().filter(|p| p.domed).map(|p| p.ty).collect();
        if let Some(src) = src {
            let gen = g.node(src.node).unwrap();
            let domed = domed_types.contains(&gen.ty().name.as_str());
            if domed && gen.float(gen.ty().attr_index("fraction").unwrap()) < 1.0 {
                g.evaluate();
                if let Some(m) = g.node(src.node).unwrap().value(src.attr as usize).as_mesh() {
                    at -= up * m.bounds().min[if g.up_axis == UpAxis::Z { 2 } else { 1 }];
                }
            }
        }
    }
    g.set_value(PlugRef::new(xform, Transform::TRANSLATE as i32), PlugValue::Vec3(at))?;
    Ok(())
}

/// A point buffer (Python's numpy array) for the setMeshPoints call that passes the returned token.
static STASHED_POINTS: Mutex<(u64, u64, Vec<DVec3>)> = Mutex::new((1, 0, Vec::new()));

/// Hands a point buffer to the setMeshPoints call that passes the returned token.
pub fn stash_points(points: Vec<DVec3>) -> u64 {
    let mut s = STASHED_POINTS.lock().unwrap();
    let token = s.0;
    s.0 += 1;
    s.1 = token;
    s.2 = points;
    token
}

// ---------------------------------------------------------------------------------------------------------------------
// Commands

type T = ArgType;
type Cmd = CommandDef<Session>;

fn flag(name: &str, ty: ArgType, default: Value, doc: &str) -> FlagDef {
    FlagDef::new(name, ref_short(name), ty, default, doc)
}

/// The reference's short flag names for the primitive commands.
fn ref_short(name: &str) -> &'static str {
    match name {
        "width" => "w",
        "height" => "h",
        "depth" => "d",
        "radius" => "r",
        "subdivisionsAxis" => "sa",
        "subdivisionsHeight" => "sh",
        "subdivisionsCaps" | "subdivisionsCap" => "sc",
        "subdivisionsDepth" => "sd",
        "subdivisionsWidth" => "sw",
        "subdivisionsCoil" => "sco",
        "sideLength" => "w",
        "length" => "l",
        "numberOfSides" => "ns",
        "solidType" => "st",
        "polyType" => "pt",
        "coils" => "c",
        "direction" => "d",
        "sectionRadius" => "sr",
        "twist" => "tw",
        "roundCap" => "rcp",
        "thickness" => "t",
        _ => "",
    }
}

fn node_named(g: &Graph, name: &str) -> CmdResult<NodeId> {
    g.find(name)?.ok_or_else(|| CommandError::new(format!("no node named '{name}'")))
}

fn vec3_arg(a: &Args, name: &str) -> DVec3 {
    DVec3::from_array(a.vec3(name).unwrap_or([0.0; 3]))
}

fn point_of(p: &Value) -> CmdResult<DVec3> {
    let c = |i: usize| p.get(i).and_then(Value::as_f64).ok_or_else(|| CommandError::new(format!("expected [x, y, z], got {p}")));
    Ok(DVec3::new(c(0)?, c(1)?, c(2)?))
}

/// A transform with a mesh shape under it, as the reference makes them. Returns (transform, shape).
pub fn create_mesh_node(g: &mut Graph, name: &str) -> CmdResult<(NodeId, NodeId)> {
    crate::dag::create_with_shape(g, "mesh", name, NO_NODE)
}

/// The transform stem the reference gives a primitive: polyCube -> pCube.
pub fn primitive_stem(ty: &str) -> String {
    if matches!(ty, "polyPlatonicSolid" | "polyPrimitive") {
        return "pSolid".into();
    }
    format!("p{}", ty.strip_prefix("poly").unwrap_or(ty))
}

pub fn register(c: &mut Commands<Session>) {
    for p in primitives() {
        let ty = p.ty.to_string();
        let mut cmd = Cmd::new(
            p.ty,
            &format!("Create a {} mesh. Returns [mesh, generator] (generator is None with generator=False).", p.display),
            move |s, a| {
                let g = &mut s.graph;
                let default_up = if g.up_axis == UpAxis::Z { "z" } else { "y" };
                // The reference gives the axis as a vector (-ax 1 0 0).
                let up_flag = a.get("axis").filter(|v| v.is_string()).or(a.get("upAxis"));
                let by_vector = a.get("axis").is_some_and(Value::is_array);
                let up = match up_flag {
                    _ if by_vector => "y".to_string(),
                    Some(Value::Array(v)) if v.len() == 3 => {
                        let c: Vec<f64> = v.iter().map(|x| x.as_f64().unwrap_or(0.0)).collect();
                        let i = (0..3).max_by(|&i, &j| c[i].abs().total_cmp(&c[j].abs())).unwrap();
                        format!("{}{}", if c[i] < 0.0 { "-" } else { "" }, ["x", "y", "z"][i])
                    }
                    Some(Value::String(u)) => u.clone(),
                    _ => default_up.to_string(),
                };
                if !UP_AXES.contains(&up.as_str()) {
                    fail!("{ty}: upAxis must be x, y, z, -x, -y or -z");
                }
                // Recorded so adjusting the op later keeps the axis it was made with, whatever the scene up is then.
                crate::last_op::limit_flag(s, "upAxis", json!(up));
                let g = &mut s.graph;
                let gen_type = types::find(&ty).unwrap();
                let gen = g.create_node(&gen_type, "", NO_NODE, true)?;
                let mut values = a.flags.clone();
                values.insert("upAxis".into(), json!(up));
                if !by_vector {
                    values.remove("axis");
                }
                // Attribute names first: a flag that is another name for one (subdivisionsX) has the last word over
                // the attribute's own default.
                let mut order: Vec<(&String, &Value)> = values.iter().collect();
                order.sort_by_key(|(k, _)| gen_type.attr_index(k).is_some_and(|i| gen_type.attrs[i].name != **k));
                for (flag, value) in order {
                    if value.is_null() {
                        continue;
                    }
                    if let Some(idx) = gen_type.attr_index(flag) {
                        let v = from_json(&gen_type.attrs[idx], value)?;
                        g.set_value(PlugRef::new(gen, idx as i32), v)?;
                    }
                }
                let (xform, mesh) = create_mesh_node(g, &a.string("name", &format!("{}1", primitive_stem(&ty))))?;
                let out = gen_type.attr_index("outMesh").unwrap();
                g.connect(&PlugRef::new(gen, out as i32), &PlugRef::new(mesh, MeshNode::IN_MESH as i32), false)?;
                if a.has("rotation") {
                    g.set_value(PlugRef::new(xform, Transform::ROTATE as i32), PlugValue::Vec3(vec3_arg(a, "rotation")))?;
                }
                if a.has("normal") {
                    let position = if a.has("position") { vec3_arg(a, "position") } else { DVec3::ZERO };
                    place_primitive(g, xform, position, vec3_arg(a, "normal"))?;
                } else if a.has("position") {
                    g.set_value(PlugRef::new(xform, Transform::TRANSLATE as i32), PlugValue::Vec3(vec3_arg(a, "position")))?;
                }
                let mut gen_name = json!(g.path(gen));
                let keep = (!a.has("generator") || a.bool("generator")) && (!a.has("constructionHistory") || a.bool("constructionHistory"));
                if !keep {
                    g.evaluate();
                    let baked = g.node(mesh).unwrap().value(MeshNode::OUT_MESH).clone();
                    let input = PlugRef::new(mesh, MeshNode::IN_MESH as i32);
                    g.disconnect(&input);
                    g.set_value(input, baked)?;
                    g.delete_node(gen)?;
                    gen_name = Value::Null;
                }
                g.select(&[xform], SelectMode::Replace);
                if gen_name.is_null() && a.has("constructionHistory") {
                    return Ok(json!([g.path(xform)]));
                }
                Ok(json!([g.path(xform), gen_name]))
            },
        )
        .flag(FlagDef::new("name", "n", T::String, Value::Null, "Name of the mesh; made unique."))
        .flag(flag("position", T::Vec3, Value::Null, "World position."))
        .flag(flag("rotation", T::Vec3, Value::Null, "Rotation in degrees."))
        .flag(flag(
            "normal",
            T::Vec3,
            Value::Null,
            "Turn the up axis to this direction, as for placing on a surface; a dome cut by fraction then sits on position by \
             its open base.",
        ))
        .flag(flag("generator", T::Bool, json!(true), "Keep the generator node so its parameters stay editable."))
            .flag(FlagDef::new("object", "o", T::Bool, Value::Null, "Accepted: the transform and shape are always made."))
            .flag(FlagDef::new("createUVs", "cuv", T::Int, Value::Null, "0 leaves the mesh without UVs."))
            .flag(FlagDef::new("texture", "tx", T::Int, Value::Null, "How UVs are laid out; accepted."))
            .flag(FlagDef::new("axis", "ax", T::Any, Value::Null, "The direction the primitive's up points, as a vector."))
            .flag(FlagDef::new("caching", "cch", T::Bool, Value::Null, "Accepted."))
            .flag(FlagDef::new("nodeState", "nds", T::Int, Value::Null, "Accepted."))
            .flag(FlagDef::new("constructionHistory", "ch", T::Bool, Value::Null, "False: no generator node, as generator=False."));
        for a in &p.attrs {
            if a.ty == PlugType::Enum {
                let def = a.enum_names[a.default_value.as_int() as usize].clone();
                let choices: Vec<&str> = a.enum_names.iter().map(String::as_str).collect();
                cmd = cmd.flag(flag(&a.name, T::String, json!(def), &a.doc).choices(&choices));
            } else {
                let ty = match a.ty {
                    PlugType::Bool => T::Bool,
                    PlugType::Int => T::Int,
                    _ => T::Float,
                };
                let mut fd = flag(&a.name, ty, to_json(&a.default_value), &a.doc);
                if fd.short_name.is_empty() {
                    fd.short_name = a.short_name.clone();
                }
                if let Some(lo) = a.min_value {
                    fd = fd.min(lo);
                }
                if let Some(hi) = a.max_value {
                    fd = fd.max(hi);
                }
                cmd = cmd.flag(fd);
            }
        }
        for x in &p.flags {
            cmd = cmd.flag(FlagDef::new(x.long, x.short, x.ty, Value::Null, "Another name for the attribute it sets."));
        }
        cmd = cmd
            .flag(
                FlagDef::new(
                    "upAxis",
                    "",
                    T::Any,
                    Value::Null,
                    "Direction the primitive's up points: x, y, z, -x, -y or -z (or a vector, as the reference's -ax 1 0 0); the scene up \
                     axis when left out.",
                ),
            )
            .adjustable(true);
        c.register(cmd);
    }

    c.register(
        Cmd::new(
            "smoothPreview",
            "Display meshes subdivided without changing them (the 1/2/3 keys), including every mesh under a given group. \
             level 0 is off.",
            |s, a| {
                let level = a.int("level", 2).clamp(0, 3);
                let names = objects(s, a);
                let g = &mut s.graph;
                let mut ids = Vec::new();
                for name in names {
                    let id = node_named(g, &name)?;
                    ids.push(id);
                    ids.extend(g.descendants(id));
                }
                ids.sort_unstable();
                ids.dedup();
                // A display toggle, like the reference's: undo steps over it.
                let was = g.undo_stack().enabled();
                g.undo_stack_mut().set_enabled(false);
                let mut result = Ok(Value::Null);
                for id in ids {
                    if g.node(id).is_some_and(|n| n.ty().is_a("mesh")) {
                        if let Err(e) = g.set_value(PlugRef::new(id, MeshNode::SMOOTH_PREVIEW as i32), PlugValue::Int(level)) {
                            result = Err(e.into());
                            break;
                        }
                    }
                }
                g.undo_stack_mut().set_enabled(was);
                result
            },
        )
        .positional("objects")
        .flag(flag("level", T::Int, json!(2), "Subdivision levels to display (0 to 3).").min(0.0).max(3.0))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "meshInfo",
            "Counts and health of a mesh: points, edges, faces, triangles, tri/quad/ngon faces, open border edges and loops, \
             non-manifold edges, shells, closed, object and world bounds, symmetry, smooth preview, modifiers, material \
             slots (and the cage's counts under modifiers).",
            |s, a| {
                let objs = objects(s, a);
                let Some(first) = objs.first() else { fail!("meshInfo: no object") };
                let g = &mut s.graph;
                let Some(n) = crate::components::as_mesh(g, node_named(g, first)?) else {
                    fail!("meshInfo: {first} is not a mesh");
                };
                Ok(crate::component_query::mesh_stats(g, n))
            },
        )
        .positional("object")
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "meshData",
            "A mesh's points and faces as plain lists: {points: [[x,y,z]...], faces: [[i,j,k,...]...]}. base=True reads the \
             editable base mesh (under modifiers) instead of the final result.",
            |s, a| {
                let objs = objects(s, a);
                let Some(last) = objs.last() else { fail!("meshData: no mesh") };
                let g = &mut s.graph;
                let Some(n) = crate::components::as_mesh(g, node_named(g, last)?) else {
                    fail!("meshData: {last} is not a mesh");
                };
                g.evaluate();
                let m = if a.bool("base") {
                    modifiers::cage(g, n)
                } else {
                    g.node(n).unwrap().data().mesh(MeshNode::OUT_MESH).cloned()
                };
                let Some(m) = m else { return Ok(json!({"points": [], "faces": []})) };
                let w = if a.bool("worldSpace") {
                    g.node(n).unwrap().matrix(Transform::WORLD_MATRIX)
                } else {
                    glam::DMat4::IDENTITY
                };
                let mut remap = vec![INVALID; m.point_capacity() as usize];
                let mut points = Vec::new();
                for p in 0..m.point_capacity() {
                    if m.point_valid(p) {
                        remap[p as usize] = points.len() as Index;
                        let q = w.transform_point3(m.position(p));
                        points.push(json!([q.x, q.y, q.z]));
                    }
                }
                let mut faces = Vec::new();
                for f in 0..m.face_capacity() {
                    if m.face_valid(f) {
                        let mut face = Vec::new();
                        m.for_each_face_corner(f, |c| face.push(remap[m.corner_point(c) as usize]));
                        faces.push(json!(face));
                    }
                }
                Ok(json!({"points": points, "faces": faces}))
            },
        )
        .positional("mesh")
        .flag(flag("base", T::Bool, json!(false), "The base mesh, not the modified result."))
        .flag(flag("worldSpace", T::Bool, json!(false), "Points in world space."))
        .undoable(false),
    );

    c.register(
        Cmd::new(
            "setMeshPoints",
            "Move every point of a mesh's base (under its modifiers) at once, in meshData's point order, in object space. \
             Topology stays. One undo step. Python's Node.set_points takes a numpy array, faster.",
            |s, a| {
                let objs = objects(s, a);
                let Some(last) = objs.last() else { fail!("setMeshPoints: no mesh") };
                let token = a.int("buffer", 0) as u64;
                let pts: Vec<DVec3> = if token != 0 {
                    let mut st = STASHED_POINTS.lock().unwrap();
                    if token != st.1 {
                        fail!("setMeshPoints: the buffer is gone; pass points= instead");
                    }
                    st.1 = 0;
                    std::mem::take(&mut st.2)
                } else {
                    let list = a.get("points").and_then(Value::as_array).cloned().unwrap_or_default();
                    list.iter().map(point_of).collect::<CmdResult<_>>()?
                };
                let g = &mut s.graph;
                let Some(n) = crate::components::as_mesh(g, node_named(g, last)?) else {
                    fail!("setMeshPoints: {last} is not a mesh");
                };
                let base = modifiers::edit_plug(g, n)?;
                g.evaluate();
                let Some(cur) = g.stored_value(&base).as_mesh().cloned() else { fail!("setMeshPoints: {last} has no mesh") };
                if pts.len() != cur.num_points() as usize {
                    fail!("setMeshPoints: {last} has {} points, got {}", cur.num_points(), pts.len());
                }
                let mut edited = (*cur).clone();
                if edited.num_points() == edited.point_capacity() {
                    edited.positions_mut().par_iter_mut().zip(pts.par_iter()).for_each(|(d, s)| *d = *s);
                } else {
                    let mut k = 0;
                    for p in 0..edited.point_capacity() {
                        if edited.point_valid(p) {
                            edited.set_position(p, pts[k]);
                            k += 1;
                        }
                    }
                }
                g.set_value(base, PlugValue::Mesh(Some(Arc::new(edited))))?;
                Ok(json!(pts.len()))
            },
        )
        .positional("mesh")
        .flag(flag("points", T::List, json!([]), "[[x, y, z], ...], one per point, in cm."))
        .flag(flag("buffer", T::Int, json!(0), "Internal: points handed over from Python (Node.set_points).")),
    );

    c.register(
        Cmd::new("createMesh", "Make a mesh from points and faces (lists of point indices, any size). Returns its name.", |s, a| {
            if !a.has("points") || !a.has("faces") {
                fail!("createMesh: give points= and faces=");
            }
            let pts: Vec<DVec3> = a.get("points").unwrap().as_array().unwrap().iter().map(point_of).collect::<CmdResult<_>>()?;
            let mut sizes = Vec::new();
            let mut idx = Vec::new();
            for face in a.get("faces").unwrap().as_array().unwrap() {
                let face = face.as_array().cloned().unwrap_or_default();
                sizes.push(face.len() as Index);
                for i in face {
                    let v = i.as_u64().ok_or_else(|| CommandError::new(format!("createMesh: bad face index {i}")))? as Index;
                    if v as usize >= pts.len() {
                        fail!("createMesh: face index {v} is out of range");
                    }
                    idx.push(v);
                }
            }
            let mut mesh = Mesh::from_polygons(&pts, &sizes, &idx);
            if a.has("uvs") {
                let uvs = a.get("uvs").unwrap().as_array().cloned().unwrap_or_default();
                if uvs.len() != idx.len() {
                    fail!("createMesh: {} uvs for {} face corners", uvs.len(), idx.len());
                }
                if mesh.num_faces() as usize != sizes.len() {
                    fail!("createMesh: uvs need every face to be valid");
                }
                let mut values = Vec::new();
                let mut at = 0;
                for fi in 0..mesh.num_faces() {
                    let mut c = mesh.face_corner(fi);
                    for _ in 0..sizes[fi as usize] {
                        let u = &uvs[at];
                        let uv = [u.get(0).and_then(Value::as_f64).unwrap_or(0.0) as f32, u.get(1).and_then(Value::as_f64).unwrap_or(0.0) as f32];
                        values.push((c, uv));
                        at += 1;
                        c = mesh.corner_next(c);
                    }
                }
                let layer = mesh.corner_attrs_mut().get_or_add::<[f32; 2]>(&names::uv(0));
                for (c, uv) in values {
                    layer[c as usize] = uv;
                }
            }
            let g = &mut s.graph;
            let (xform, n) = create_mesh_node(g, &a.string("name", ""))?;
            g.set_value(PlugRef::new(n, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(mesh))))?;
            g.select(&[xform], SelectMode::Replace);
            Ok(json!(g.path(xform)))
        })
        .flag(flag("points", T::List, Value::Null, "[[x, y, z], ...] in cm."))
        .flag(flag("faces", T::List, Value::Null, "[[i, j, k, ...], ...]."))
        .flag(flag("uvs", T::List, Value::Null, "[[u, v], ...]: one per face corner, face by face in faces' order. Fills uv0."))
        .flag(FlagDef::new("name", "n", T::String, Value::Null, "Name; made unique.")),
    );

    c.register(
        Cmd::new("importObj", "Import an OBJ file; one mesh per object. Returns the new mesh names.", |s, a| {
            let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("importObj: path required") };
            let r = read_obj_file(&path, &ObjReadOptions::default());
            if !r.ok() {
                fail!("importObj: {}", r.error);
            }
            let g = &mut s.graph;
            g.begin_batch();
            let mut names = Vec::new();
            let mut ids = Vec::new();
            let mut result = Ok(());
            for o in r.objects {
                let wanted = if Graph::valid_name(&o.name) { o.name.clone() } else { String::new() };
                match create_mesh_node(g, &wanted) {
                    Ok((xform, n)) => {
                        if let Err(e) = g.set_value(PlugRef::new(n, MeshNode::IN_MESH as i32), PlugValue::Mesh(Some(Arc::new(o.mesh)))) {
                            result = Err(CommandError::from(e));
                            break;
                        }
                        names.push(json!(g.path(xform)));
                        ids.push(xform);
                    }
                    Err(e) => {
                        result = Err(e);
                        break;
                    }
                }
            }
            g.select(&ids, SelectMode::Replace);
            g.end_batch();
            result?;
            Ok(Value::Array(names))
        })
        .positional("path"),
    );

    c.register(
        Cmd::new("exportObj", "Export meshes to an OBJ file in world space. Defaults to the selection.", |s, a| {
            let Some(path) = a.pos_str(0).map(str::to_string) else { fail!("exportObj: path required") };
            let mut names: Vec<String> = (1..a.positional.len()).filter_map(|k| a.pos_str(k).map(str::to_string)).collect();
            let g = &mut s.graph;
            if names.is_empty() {
                names = g.selection().iter().map(|&id| g.path(id)).collect();
            }
            g.evaluate();
            let mut objects = Vec::new();
            for name in names {
                let Some(n) = crate::components::as_mesh(g, node_named(g, &name)?) else {
                    continue;
                };
                let Some(m) = modifiers::export_mesh(g, n) else { continue };
                let mut o = ObjObject { name: name.clone(), mesh: (*m).clone(), ..Default::default() };
                let world = g.node(n).unwrap().matrix(Transform::WORLD_MATRIX);
                for p in 0..o.mesh.point_capacity() {
                    if o.mesh.point_valid(p) {
                        let q = world.transform_point3(o.mesh.position(p));
                        o.mesh.set_position(p, q);
                    }
                }
                objects.push(o);
            }
            if objects.is_empty() {
                fail!("exportObj: no meshes to export");
            }
            let text = write_obj_objects(&objects, &ObjWriteOptions::default());
            if std::fs::write(&path, text).is_err() {
                fail!("exportObj: can't write {path}");
            }
            Ok(json!(path))
        })
        .positional("path objects...")
        .undoable(false),
    );
}
