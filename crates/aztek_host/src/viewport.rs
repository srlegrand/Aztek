//! The 3D view: what to draw from the scene, its GPU buffers, picking, and the move/rotate/scale manipulators.

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use aztek_geo::bvh::MeshBvh;
use aztek_geo::render_mesh::{build_render_mesh, RenderMeshOptions};
use aztek_geo::{Index, Mesh};
use aztek_scene::component_pick::{self, Screen};
use aztek_scene::components::{self, ComponentMode};
use aztek_scene::draw_style as ds;
use aztek_scene::graph::Graph;
use aztek_scene::mesh_nodes::MeshNode;
use aztek_scene::plug::{NodeId, NO_NODE};
use aztek_scene::transform::DagNode;
use aztek_scene::Session;
use ash::vk;
use serde_json::Value;
use glam::{DMat3, DMat4, DVec2, DVec3, Vec3};

use aztek_scene::surfaces::{self, SurfaceShape};

use crate::camera::{CamKind, Camera};
use crate::gpu::{Buffer, Gpu};
use crate::render::{c8, Kind, LineVertex, MatPush, Push, Renderer, ShadedVertex};
use aztek_scene::shading::{self, DrawMaterial};

pub struct DrawItem {
    pub shape: NodeId,
    pub xf: NodeId,
    pub world: DMat4,
    pub mesh: Arc<Mesh>,
    /// Smooth preview levels (the 2 and 3 keys); 0 off.
    pub smooth: i32,
}

/// Whether an edge is on a border of the first UV set: on the mesh's border, or with its faces' UVs apart along it.
fn uv_border(m: &Mesh, uvs: &[[f32; 2]], e: Index, faces: usize) -> bool {
    if faces != 2 {
        return true;
    }
    if uvs.is_empty() {
        return false;
    }
    let mut ends: Vec<(Index, [f32; 2])> = Vec::new();
    m.for_each_edge_corner(e, |c| {
        for k in [c, m.corner_next(c)] {
            if let Some(uv) = uvs.get(k as usize) {
                ends.push((m.corner_point(k), *uv));
            }
        }
    });
    ends.iter().any(|(p, uv)| ends.iter().any(|(q, w)| p == q && ((uv[0] - w[0]).abs() > 1e-6 || (uv[1] - w[1]).abs() > 1e-6)))
}

/// Where an orthographic view's grid lies and what it covers: the view's right and up directions, the span of each
/// in the view, the spacing drawn and whether it is wider than the grid's own.
pub struct OrthoGrid {
    pub right: DVec3,
    pub up: DVec3,
    pub u: (f64, f64),
    pub v: (f64, f64),
    pub step: f64,
    pub subdivisions: bool,
    pub coarse: bool,
}

/// The grid command's settings a view draws with (its size and spacing are the view's grid_extent and grid_spacing).
#[derive(Clone, Debug, PartialEq)]
pub struct GridStyle {
    /// Subdivision lines between grid lines: a grid line every `divisions` subdivision lines.
    pub divisions: i32,
    pub axes: bool,
    pub axes_bold: bool,
    pub lines: bool,
    pub division_lines: bool,
    pub persp_labels: bool,
    pub ortho_labels: bool,
    /// Numbers along the grid's edge rather than along the axes.
    pub persp_label_edge: bool,
    pub ortho_label_edge: bool,
}

impl Default for GridStyle {
    fn default() -> Self {
        GridStyle { divisions: 5, axes: true, axes_bold: true, lines: true, division_lines: true, persp_labels: false, ortho_labels: false, persp_label_edge: false, ortho_label_edge: false }
    }
}

impl GridStyle {
    /// From the grid command's flags.
    pub fn from_json(j: &serde_json::Value) -> Self {
        let d = GridStyle::default();
        let b = |k: &str, def: bool| j.get(k).and_then(|v| v.as_bool().or_else(|| v.as_f64().map(|x| x != 0.0))).unwrap_or(def);
        let edge = |k: &str| j.get(k).and_then(|v| v.as_str()).is_some_and(|v| v == "edge");
        GridStyle {
            divisions: j.get("divisions").and_then(|v| v.as_f64()).map_or(d.divisions, |x| (x.round() as i32).max(1)),
            axes: b("displayAxes", d.axes),
            axes_bold: b("displayAxesBold", d.axes_bold),
            lines: b("displayGridLines", d.lines),
            division_lines: b("displayDivisionLines", d.division_lines),
            persp_labels: b("displayPerspectiveLabels", false),
            ortho_labels: b("displayOrthographicLabels", false),
            persp_label_edge: edge("perspectiveLabelPosition"),
            ortho_label_edge: edge("orthographicLabelPosition"),
        }
    }

    /// The axes', grid lines' and subdivision lines' colours: what displayRGBColor gives gridAxis, gridHighlight and
    /// gridColor, else the views' own.
    pub fn colors() -> [[u8; 4]; 3] {
        let get = |name: &str, def: [u8; 4]| match aztek_scene::draw_style::display_rgb(name) {
            Some(c) => [(c[0].clamp(0.0, 1.0) * 255.0).round() as u8, (c[1].clamp(0.0, 1.0) * 255.0).round() as u8, (c[2].clamp(0.0, 1.0) * 255.0).round() as u8, 255],
            None => def,
        };
        [get("gridAxis", [0, 0, 0, 255]), get("gridHighlight", [70, 70, 70, 255]), get("gridColor", [105, 105, 105, 255])]
    }
}

pub(crate) fn visible(g: &Graph, id: NodeId) -> bool {
    let mut n = id;
    while n != NO_NODE {
        let Some(node) = g.node(n) else { return false };
        if node.ty().dag && !node.bool(DagNode::VISIBILITY) {
            return false;
        }
        if aztek_scene::draw_style::hidden_by_override(g, n) {
            return false;
        }
        n = g.parent(n);
    }
    true
}

/// Every visible, non-intermediate mesh shape with its evaluated mesh, and each NURBS surface with its tessellation.
pub fn gather(g: &mut Graph) -> Vec<DrawItem> {
    g.evaluate();
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        if n.ty().is_a("nurbsSurface") && !n.bool(DagNode::INTERMEDIATE_OBJECT) && visible(g, id) {
            if let Some(mesh) = n.data().mesh(SurfaceShape::OUT_MESH).cloned() {
                let world = n.data().matrix(DagNode::WORLD_MATRIX);
                out.push(DrawItem { shape: id, xf: aztek_scene::dag::transform_of(g, id), world, mesh, smooth: 0 });
            }
            continue;
        }
        // A GPU cache draws its file's meshes at the scene's time, under its own transform.
        if n.ty().is_a(aztek_scene::gpu_cache::NODE_TYPE) && visible(g, id) {
            let Some(at) = n.ty().attr_index("outMesh") else { continue };
            if let Some(mesh) = n.data().mesh(at).cloned() {
                let world = n.data().matrix(DagNode::WORLD_MATRIX);
                out.push(DrawItem { shape: id, xf: aztek_scene::dag::transform_of(g, id), world, mesh, smooth: 0 });
            }
            continue;
        }
        if !n.ty().is_a("mesh") || n.bool(DagNode::INTERMEDIATE_OBJECT) || !visible(g, id) {
            continue;
        }
        let Some(mesh) = n.data().mesh(MeshNode::OUT_MESH).cloned() else { continue };
        let world = n.data().matrix(DagNode::WORLD_MATRIX);
        let smooth = MeshNode::preview_level(n) as i32;
        let mesh = without_hidden_faces(g, id, mesh);
        out.push(DrawItem { shape: id, xf: aztek_scene::dag::transform_of(g, id), world, mesh, smooth });
    }
    out
}

thread_local! {
    /// Per mesh shape: the mesh and hidden-faces version it was made from, and the mesh without those faces. Kept so the
    /// view sees the same mesh frame to frame and its caches hold.
    static MASKED: std::cell::RefCell<HashMap<NodeId, (Arc<Mesh>, u64, Arc<Mesh>)>> = Default::default();
}

/// The mesh as views show it: without the faces hidden on it (Display > Hide in a component mode).
fn without_hidden_faces(g: &Graph, id: NodeId, mesh: Arc<Mesh>) -> Arc<Mesh> {
    let hidden = aztek_scene::hidden_components::hidden(g, id);
    MASKED.with(|c| {
        let mut c = c.borrow_mut();
        if hidden.is_empty() {
            c.remove(&id);
            return mesh;
        }
        let v = g.hidden_faces_version;
        if let Some((src, ver, out)) = c.get(&id) {
            if Arc::ptr_eq(src, &mesh) && *ver == v {
                return out.clone();
            }
        }
        let out = Arc::new(aztek_scene::hidden_components::masked(&mesh, hidden));
        c.insert(id, (mesh, v, out.clone()));
        out
    })
}

/// How much of a subdivision's own edges shows in smooth preview's wire, where the cage's are solid.
const SUBDIVISION_LINE_ALPHA: u8 = 60;

struct GpuMesh {
    /// What's on the GPU. Held rather than known by its address: a new mesh can land at the address of a freed one, and
    /// the view would keep drawing the old shape until something else changed it.
    drawn: Option<Arc<Mesh>>,
    /// Counts changes to the mesh, and to its topology, for the caches built from it.
    mesh_gen: u64,
    topology_gen: u64,
    vertex_corner: Vec<Index>,
    smooth: i32,
    /// The subdivided mesh smooth preview draws.
    display: Option<Arc<Mesh>>,
    tris: Option<(Buffer, Buffer, u32)>,
    /// Per triangle source face, for the selected-face tint.
    tri_faces: Vec<Index>,
    tri_indices: Vec<Index>,
    bvh: Option<MeshBvh>,
    sel_key: u64,
    lines: Option<(Buffer, u32)>,
    points: Option<(Buffer, u32)>,
    tint: Option<(Buffer, u32)>,
    /// The same for faces on the mirror side.
    mirror_tint: Option<(Buffer, u32)>,
    /// One vertex per render vertex's source point, for the weight colours.
    vertex_point: Vec<Index>,
    weight_tris: Option<(Buffer, Buffer, u32)>,
    weight_key: u64,
    /// UVs, one per render vertex: the second vertex buffer of the material pipelines.
    uvs: Option<Buffer>,
    /// Per face of the smooth preview's mesh, the cage face it is part of.
    display_faces: Vec<Index>,
    /// Per edge of the smooth mesh: the cage edge it runs along, INVALID for one a subdivision added.
    display_edges: Vec<Index>,
    /// How each level of the smooth mesh's points comes from the one before: lets a deformed mesh go through without
    /// subdividing it again. Made the first time it's needed.
    stencils: Vec<aztek_geo::ops::subdivide::PointStencil>,
    /// The triangles again, gathered by shading group.
    batch_ib: Option<Buffer>,
    batch_indices: Vec<Index>,
    /// The cage face of each triangle of batch_indices.
    batch_faces: Vec<Index>,
    batches: Vec<Batch>,
    batch_key: u64,
}

/// A run of triangles one material draws. group is NO_NODE for a shape no shading group holds.
#[derive(Clone, Copy)]
struct Batch {
    group: NodeId,
    first: u32,
    count: u32,
}

/// One material's textures, this frame.
#[derive(Clone, Copy)]
struct MatSets {
    color: Option<vk::DescriptorSet>,
    alpha: Option<vk::DescriptorSet>,
    alpha_has_alpha: bool,
}

/// How the scene's materials draw, read when the scene changes.
#[derive(Default)]
struct MatTable {
    version: u64,
    materials: HashMap<NodeId, DrawMaterial>,
    members: HashMap<NodeId, Vec<(NodeId, Option<Vec<u32>>)>>,
    project: Option<String>,
    project_read: Option<std::time::Instant>,
    sets: HashMap<NodeId, MatSets>,
    /// lambert1: what draws a shading group whose shader Aztek doesn't know.
    fallback: Option<DrawMaterial>,
}

/// A see-through batch, drawn after everything solid.
struct Clear {
    shape: NodeId,
    world: DMat4,
    batch: usize,
    depth: f64,
    retopo: f32,
}

/// What a shape draws with when nothing shades it: the reference's green.
fn no_material() -> DrawMaterial {
    DrawMaterial {
        group: NO_NODE,
        material: NO_NODE,
        unlit: false,
        color: [0.0, 1.0, 0.0],
        ambient: [0.0; 3],
        incandescence: [0.0; 3],
        diffuse: 1.0,
        transparency: 0.0,
        color_map: None,
        alpha_map: None,
        specular: None,
        pbr: Default::default(),
    }
}

/// What a surface draws with under Use Default Material.
fn default_grey() -> DrawMaterial {
    DrawMaterial { color: [0.5, 0.5, 0.5], ..no_material() }
}

/// A draw's highlight and lights: two floats of the push constants the material shader has free.
fn stamp_lights(lit: &Option<crate::lighting::ViewLights>, mp: &mut MatPush, shape: NodeId, group: NodeId) {
    if let Some(l) = lit {
        mp.ambient[3] = l.index.get(&group).copied().unwrap_or(0) as f32;
        mp.base.n1[3] = l.mask(shape) as f32;
    }
}

fn material_push(base: Push, m: &DrawMaterial, sets: Option<&MatSets>) -> (MatPush, bool) {
    let color_set = sets.and_then(|x| x.color);
    let alpha_set = sets.and_then(|x| x.alpha);
    let placed = m.color_map.as_ref().filter(|_| color_set.is_some()).or(m.alpha_map.as_ref().filter(|_| alpha_set.is_some()));
    let mut flags = 0;
    let (mut uv, mut frame, mut rot, mut gain, mut offs) = ([1.0, 1.0, 0.0, 0.0], [1.0, 1.0, 0.0, 0.0], [0.0, 0.0], [1.0; 3], [0.0; 3]);
    if let Some(t) = placed {
        uv = [t.repeat[0], t.repeat[1], t.offset[0], t.offset[1]];
        frame = [t.coverage[0].max(1e-6), t.coverage[1].max(1e-6), t.translate_frame[0], t.translate_frame[1]];
        rot = [t.rotate_uv, t.rotate_frame];
        flags |= t.wrap[0] as i32 | (t.wrap[1] as i32) << 1 | (t.mirror[0] as i32) << 2 | (t.mirror[1] as i32) << 3;
        gain = t.gain;
        offs = t.color_offset;
    } else {
        flags |= 3;
    }
    if color_set.is_some() {
        flags |= 16;
    }
    let mut alpha_mode = 0.0;
    if alpha_set.is_some() {
        let lum = m.alpha_map.as_ref().is_some_and(|t| t.alpha_is_luminance) || !sets.is_some_and(|x| x.alpha_has_alpha);
        alpha_mode = if lum { 2.0 } else { 1.0 };
    }
    let alpha = 1.0 - m.transparency;
    let mut base = base;
    base.color = [m.color[0], m.color[1], m.color[2], alpha];
    let push = MatPush {
        base,
        uv,
        frame,
        misc: [rot[0], rot[1], flags as f32, alpha_mode],
        gain: [gain[0], gain[1], gain[2], if m.unlit { 0.0 } else { 1.0 }],
        offs: [offs[0], offs[1], offs[2], m.diffuse],
        ambient: [m.ambient[0], m.ambient[1], m.ambient[2], 0.0],
        incandescence: [m.incandescence[0], m.incandescence[1], m.incandescence[2], 0.0],
    };
    (push, alpha_mode > 0.0 || alpha < 0.999)
}

/// The same draw's constants for the Realtime pipelines, which read the fields it has no use for as the OpenPBR
/// parameters: `frame` the roughness and weights, `offs.w` the base weight, `ambient` the specular colour, and `coat`
/// the coat and transmission.
fn realtime_push(mp: &MatPush, m: &DrawMaterial, highlight: f32) -> crate::render::RtPush {
    let mut base = mp.base;
    // A transmissive surface is drawn solid and taken apart by the screen-space refraction pass (#302): blending it
    // against the background here would show that background twice.
    if m.pbr.transmission_weight > 0.5 {
        base.color[3] = 1.0;
    }
    crate::render::RtPush {
        base,
        uv: mp.uv,
        frame: [m.pbr.metalness, m.pbr.specular_roughness, m.pbr.diffuse_roughness, m.pbr.specular_weight],
        misc: mp.misc,
        gain: mp.gain,
        offs: [mp.offs[0], mp.offs[1], mp.offs[2], m.pbr.base_weight],
        ambient: [m.pbr.specular_color[0], m.pbr.specular_color[1], m.pbr.specular_color[2], highlight],
        incandescence: [m.pbr.emission[0], m.pbr.emission[1], m.pbr.emission[2], m.pbr.specular_ior],
        coat: [m.pbr.coat_weight, m.pbr.coat_roughness, m.pbr.coat_ior, m.pbr.transmission_weight],
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    /// Show Manipulator: the handles of a node's own attributes.
    ShowManip,
    Move,
    Rotate,
    Scale,
    /// Click to place joints, each under the last.
    Joint,
    /// Paint skin weights: the brush on the selected skinned meshes.
    PaintSkin,
    /// Paint a deformer's per-point values: the brush on the selected deformed meshes.
    PaintAttr,
    /// CV Curve Tool: clicks place CVs; Enter makes the curve.
    CurveCv,
    /// EP Curve Tool: clicks place edit points; Enter makes the curve.
    CurveEp,
    Sculpt,
    /// The Interactive Groom tools on hair descriptions.
    Groom,
    InsertJoint,
    /// IK Handle Tool: click the start joint, then the end joint.
    IkHandle,
    /// IK Spline Handle Tool: the start joint, the end joint, then the curve when the tool doesn't make one.
    IkSplineHandle,
    MultiCut,
    InsertLoop,
    OffsetLoop,
    TargetWeld,
    SlideEdge,
    Crease,
    CreatePolygon,
    AppendPolygon,
    QuadDraw,
    /// The 3D Cut and Sew UV Tool: drag over edges to cut their UVs, Ctrl to sew them.
    CutSew,
    /// Drag a free-form outline; what's inside is selected.
    Lasso,
    /// Drag a brush over components to select them.
    PaintSelect,
    /// Click component after component: the shortest edge path between them.
    ShortestPath,
    /// Camera tools: the left button does what Alt+drag does.
    CamTumble,
    CamTrack,
    CamDolly,
    CamZoom,
    CamRoll,
    CamYawPitch,
    /// Drag a box to dolly in on it, or out with a drag the other way.
    CamBoxDolly,
    /// Left drag looks round, middle drag walks.
    CamWalk,
    /// Drag in the view to change the current time.
    ModelTime,
    /// Clicks add points to the selected curve; Enter adds them.
    CurveAddPt,
    /// Clicks and drags place Bezier anchors and their tangents; Enter makes the curve.
    CurveBezier,
    /// Two clicks make an arc; three clicks another.
    ArcTwo,
    ArcThree,
    /// Two clicks on border edges, or on faces, merge them.
    MergeEdge,
    MergeFacet,
    /// Drag on the grid for the base; a cube then takes its height from the mouse and a click.
    CreateCube,
    CreatePlane,
    /// Sphere, cylinder, cone or torus: drag for the radius; a cylinder or cone takes a height too.
    CreateRound,
    /// Two clicks: a point on the object to move, and where it goes.
    SnapTogether,
}

/// Radius of the Edit Pivot rings, in handle lengths: outside the arrows.
const PIVOT_RING: f32 = 1.22;

/// Translate channels picked in the channel box: the Move manipulator takes the parent's axes, whatever orientation it
/// is set to, as in the reference, until they are let go (#136).
static CHANNEL_SPACE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The Live Constraint Options: the wire drawn over a mesh built on the live surface and its colour, the mesh's own colour
/// and how see-through it is, how far off the surface new points sit, and whether points land on back faces.
#[derive(Clone, Debug)]
pub struct LiveOpts {
    pub wire: Option<[f32; 3]>,
    pub color: Option<[f32; 3]>,
    pub alpha: Option<f32>,
    pub offset: Option<f64>,
    pub backfaces: bool,
}

impl Default for LiveOpts {
    fn default() -> Self {
        LiveOpts { wire: Some([0.0; 3]), color: Some([0.2, 0.42, 0.95]), alpha: Some(0.75), offset: None, backfaces: false }
    }
}

/// "#rrggbb" as linear-ish 0 to 1 channels, as the toolkit's colour swatches send them.
pub fn hex_color(v: Option<&Value>) -> Option<[f32; 3]> {
    let s = v?.as_str()?.trim_start_matches('#');
    if s.len() != 6 {
        return None;
    }
    let c = |i: usize| u8::from_str_radix(&s[i..i + 2], 16).ok().map(|x| x as f32 / 255.0);
    Some([c(0)?, c(2)?, c(4)?])
}

impl LiveOpts {
    /// From the toolkit: each setting with its switch ("wireOverlay" and "wireColor", ...).
    pub fn set(&mut self, v: &Value) {
        let on = |k: &str, d: bool| v.get(k).and_then(Value::as_bool).unwrap_or(d);
        let d = LiveOpts::default();
        self.wire = on("wireOverlay", self.wire.is_some()).then(|| hex_color(v.get("wireColor")).or(self.wire).or(d.wire).unwrap());
        self.color = on("meshColorOn", self.color.is_some()).then(|| hex_color(v.get("meshColor")).or(self.color).or(d.color).unwrap());
        self.alpha = on("meshAlphaOn", self.alpha.is_some()).then(|| v.get("meshAlpha").and_then(Value::as_f64).map(|x| x as f32).or(self.alpha).or(d.alpha).unwrap().clamp(0.0, 1.0));
        self.offset = on("surfaceOffsetOn", self.offset.is_some()).then(|| v.get("surfaceOffset").and_then(Value::as_f64).or(self.offset).unwrap_or(0.0));
        self.backfaces = on("snapToBackfaces", self.backfaces);
    }
}

/// Whether it changed.
pub fn set_channel_space(on: bool) -> bool {
    CHANNEL_SPACE.swap(on, std::sync::atomic::Ordering::Relaxed) != on
}

pub fn channel_space() -> bool {
    CHANNEL_SPACE.load(std::sync::atomic::Ordering::Relaxed)
}

/// The handle each transform tool has active (Move, Rotate, Scale): the last one clicked, or the one its context's -ah
/// set. Every view shares it, as they share the tool, and it stays through selection changes ("Remember active handle"
/// is on in the reference's preferences). A middle drag that starts off the manipulator takes it (#128).
static ACTIVE_HANDLES: std::sync::Mutex<[Option<Handle>; 3]> = std::sync::Mutex::new([None; 3]);

fn active_slot(tool: Tool) -> Option<usize> {
    match tool {
        Tool::Move => Some(0),
        Tool::Rotate => Some(1),
        Tool::Scale => Some(2),
        _ => None,
    }
}

/// The handle `tool` has active, if one was clicked or set.
pub fn active_handle(tool: Tool) -> Option<Handle> {
    active_slot(tool).and_then(|i| ACTIVE_HANDLES.lock().map(|a| a[i]).unwrap_or(None))
}

/// Makes `h` `tool`'s active handle; None goes back to the default (the centre, or the view ring).
pub fn set_active_handle(tool: Tool, h: Option<Handle>) {
    if let (Some(i), Ok(mut a)) = (active_slot(tool), ACTIVE_HANDLES.lock()) {
        a[i] = h;
    }
}

/// A manip context's handle number as the reference numbers them: 0 X, 1 Y, 2 Z, 3 centre, then 4 XY, 5 YZ, 6 XZ.
pub fn handle_from_number(n: i64) -> Option<Handle> {
    match n {
        0 => Some(Handle::Axis(0)),
        1 => Some(Handle::Axis(1)),
        2 => Some(Handle::Axis(2)),
        4 => Some(Handle::Plane(2)),
        5 => Some(Handle::Plane(0)),
        6 => Some(Handle::Plane(1)),
        _ => None,
    }
}

/// The reverse of [`handle_from_number`]: what a query of -ah gives.
pub fn handle_number(h: Option<Handle>) -> i64 {
    match h {
        Some(Handle::Axis(i)) => i as i64,
        Some(Handle::Plane(2)) => 4,
        Some(Handle::Plane(0)) => 5,
        Some(Handle::Plane(1)) => 6,
        _ => 3,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Handle {
    Axis(usize),
    /// Move in the view plane / uniform scale.
    Center,
    /// Move or scale in the plane with this axis as its normal.
    Plane(usize),
    /// Rotate tool: the ring facing the camera.
    ViewRing,
    /// Rotate tool: the inside of the sphere turns freely.
    Trackball,
    /// Edit Pivot: the ring that turns the pivot's axes about this one.
    PivotRing(usize),
    /// Move tool, Ctrl+middle drag on components: each point along its own normal (#102).
    Normal,
}

/// What a view shows and how, as the panel's Shading and Show menus set it.
#[derive(Clone)]
pub struct Display {
    pub polygons: bool,
    pub surfaces: bool,
    pub curves: bool,
    pub joints: bool,
    pub ik_handles: bool,
    pub deformers: bool,
    pub locators: bool,
    pub cameras: bool,
    pub manipulators: bool,
    pub flat: bool,
    pub bounding_box: bool,
    pub xray: bool,
    /// Surfaces darken where others are close: creases, contacts, the insides of things.
    pub ambient_occlusion: bool,
    /// How far that reaches, in scene units, and how strong it is.
    pub ao_radius: f64,
    pub ao_intensity: f64,
    pub xray_joints: bool,
    pub xray_active: bool,
    pub backface: bool,
    pub film_gate: bool,
    pub res_gate: bool,
    pub safe_action: bool,
    pub highlight: bool,
    pub border_edges: bool,
    /// Polygon Display from the mesh being drawn (its displayMapBorders, displayCenter, displayTriangles,
    /// displayVertices): UV borders thick, face centres, the triangles faces are drawn with, vertices.
    pub texture_borders: bool,
    pub face_centers: bool,
    pub triangles: bool,
    pub vertices: bool,
    pub face_normals: bool,
    pub vertex_normals: bool,
    pub hard_edges: bool,
    pub crease_edges: bool,
    pub normal_size: f64,
    /// Polygon display options apply to the selection only (Display > Polygons > Limit to Selected).
    pub limit_to_selected: bool,
    /// Hardware texturing: the 6 key, modelEditor -displayTextures.
    pub textured: bool,
    /// Every surface in the default grey: modelEditor -useDefaultMaterial.
    pub default_material: bool,
    /// The Lighting menu: modelEditor -displayLights.
    pub light_mode: crate::lighting::LightMode,
    /// Back faces lit as front faces when real lights are in use.
    pub two_sided: bool,
    /// Kept for scripts and the menu: no shadows are drawn.
    pub shadows: bool,
    /// The Realtime renderer's bounce light: screen-space global illumination off what the view shows.
    pub bounce_light: bool,
    /// The camera's depth of field, as Harold renders it, when the camera has it on. Off by default.
    pub depth_of_field: bool,
    /// Show > Lights: the icons.
    pub lights: bool,
}

impl Default for Display {
    fn default() -> Display {
        Display {
            polygons: true,
            surfaces: true,
            curves: true,
            joints: true,
            ik_handles: true,
            deformers: true,
            locators: true,
            cameras: true,
            manipulators: true,
            flat: false,
            bounding_box: false,
            xray: false,
            ambient_occlusion: false,
            ao_radius: 2.0,
            ao_intensity: 1.5,
            xray_joints: false,
            xray_active: false,
            backface: false,
            film_gate: false,
            res_gate: false,
            safe_action: false,
            highlight: true,
            border_edges: false,
            texture_borders: false,
            face_centers: false,
            triangles: false,
            vertices: false,
            face_normals: false,
            vertex_normals: false,
            hard_edges: false,
            crease_edges: false,
            normal_size: 0.4,
            limit_to_selected: true,
            textured: false,
            default_material: false,
            light_mode: crate::lighting::LightMode::Default,
            two_sided: true,
            shadows: false,
            bounce_light: true,
            depth_of_field: false,
            lights: true,
        }
    }
}

pub struct Drag {
    pub handle: Handle,
    pub start: DVec2,
    pub pivot: DVec3,
    /// Whether a preview command has run and must be undone before the next.
    pub applied: bool,
    pub components: bool,
    /// Shift+middle-drag: the first few pixels choose the axis.
    pub choose_axis: bool,
    pub targets: Vec<String>,
    /// World points a point-snapping move can land on.
    pub snap_points: Vec<DVec3>,
    /// World segments a curve-snapping move can land on.
    pub snap_curves: Vec<(DVec3, DVec3)>,
    /// The lead object's rotate (degrees) and scale when the drag began, for absolute step snapping.
    pub start_rotate: Option<DVec3>,
    pub start_scale: Option<DVec3>,
}

/// A transform's rotate (in degrees) and scale channels, for snapping a drag to whole steps of them.
pub fn channels(g: &Graph, id: NodeId) -> Option<(DVec3, DVec3)> {
    let n = g.node(id)?;
    let t = n.ty();
    let r = n.data().value(t.attr_index("rotate")?).as_vec3();
    let sc = n.data().value(t.attr_index("scale")?).as_vec3();
    Some((DVec3::new(r.x.to_degrees(), r.y.to_degrees(), r.z.to_degrees()), sc))
}

pub struct Viewport {
    /// Lights and highlights for the material shader, made at the first frame.
    pub lit: Option<crate::lighting::ViewLights>,
    /// The camera shape the view looks through, for image planes shown only through their camera.
    pub look_camera: NodeId,
    /// Snap: 0 none, 1 grid (X), 2 point (V), 3 curve (C), 4 view plane, 5 live surface.
    pub snap: i32,
    /// The handle Ctrl-click made the default for middle drags.
    pub constraint: Option<Handle>,
    /// Tool Settings' step snaps; 0 off.
    pub step_move: f64,
    pub step_rotate: f64,
    pub step_scale: f64,
    /// Step snap by the offset (relative) or to whole steps of the world (absolute).
    pub snap_relative: bool,
    /// Tweak Mode: a drag off the handles picks what is under it and moves it.
    pub tweak: bool,
    /// Preserve Children: moving a parent leaves its children where they are.
    pub preserve_children: bool,
    /// Scale stops at a small positive size rather than turning inside out.
    pub prevent_negative: bool,
    /// Custom axis orientation's axes.
    pub custom_frame: DMat3,
    /// Keep Spacing: with a grid snap, the selection moves as one; off, each point lands on the grid itself.
    pub keep_spacing: bool,
    /// Snap Pivot Position: in pivot mode the pivot lands on points near the cursor.
    pub snap_pivot: bool,
    /// Tool Settings' Snap Pivot Orientation: a pivot moved onto the mesh takes the vertex's, edge's or face's axes.
    pub snap_pivot_ori: bool,
    /// Use Manipulator Pivot: objects turn and scale about the manipulator rather than each about its own.
    pub use_manip_pivot: bool,
    /// J held: rotate in discrete steps.
    pub discrete_hold: bool,
    pub disp: Display,
    /// The light the Realtime renderer's shadow map is drawn from, and its camera: read by `View` before the frame.
    pub shadow: Option<crate::lighting::ShadowPlan>,
    /// The ground plane's reflection pass draws with this view instead of the camera's (#302).
    pub planar: Option<DMat4>,
    /// The mesh that is the reflector, left out of the mirrored draw.
    pub planar_skip: NodeId,
    /// A flat colour for the mirrored pass instead of the material, for measuring its image (#302).
    pub planar_debug_color: Option<[f32; 3]>,
    /// Drawing the scene without its transmissive surfaces, for glass to see what it lets through (#302).
    pub back: bool,
    /// The sky dome the Realtime renderer lights from.
    pub environment: Option<aztek_scene::lights::EnvironmentLight>,
    /// The Realtime renderer's shadow map, bound as set 4 of its material pipelines.
    pub rt_shadow_set: Option<vk::DescriptorSet>,
    /// The contact shadow the post pass marches, when Realtime draws: view-space direction to its light, reach, strength.
    pub contact: Option<([f32; 3], f32, f32)>,
    /// Where a live-surface move lands, worked out before the drag's call.
    pub live_hit: Option<DVec3>,
    /// A live surface is set: moves snap onto it.
    pub live_auto: bool,
    /// The lasso's outline so far, in pixels.
    pub lasso: Vec<DVec2>,
    /// The select brush under the cursor, in pixels, and its radius.
    pub select_brush: Option<(DVec2, f64)>,
    /// Isolate Select: what this view shows.
    pub iso: aztek_scene::isolation::Set,
    /// D held (or Insert): the move manipulator moves pivots.
    pub pivot_mode: bool,
    /// Edit Pivot on objects: the manipulator has the rings that turn the pivot.
    pub pivot_rings: bool,
    pub cam: Camera,
    meshes: HashMap<NodeId, GpuMesh>,
    mats: MatTable,
    grid: Option<(Buffer, u32)>,
    /// The axes drawn thicker (Thicker Line for Axes), apart from the rest of the grid.
    grid_bold: Option<(Buffer, u32)>,
    /// The up axis the grid was built for: it lies on the ground, so it is rebuilt when the axis changes.
    grid_up_z: bool,
    /// The grid's colours it was built with: displayRGBColor changes them.
    grid_colors: [[u8; 4]; 3],
    pub wireframe: bool,
    pub wire_on_shaded: bool,
    pub tool: Tool,
    pub drag: Option<Drag>,
    pub hover: Option<Handle>,
    /// Viewport rect in physical pixels: x, y, w, h.
    pub rect: [f64; 4],
    pub marquee: Option<(DVec2, DVec2)>,
    /// Between subdivision lines, the finest the grid draws (and snaps to).
    pub grid_spacing: f64,
    /// How far the grid reaches each way from the origin.
    pub grid_extent: f64,
    /// What the grid command asks of the grid beyond its size and spacing.
    pub grid_style: GridStyle,
    pub show_grid: bool,
    pub manip_scale: f64,
    /// Smooth preview draws the cage too (the 2 key) rather than the smooth mesh alone (3).
    pub smooth_cage: bool,
    /// Multi-Cut's Force Cage + Smooth: the cage drawn with the smooth mesh while the tool is on (#140).
    pub force_cage: bool,
    /// Quad Draw's and Multi-Cut's Live Constraint Options: how a mesh built on the live surface draws, and where on it
    /// points go (#140).
    pub live: LiveOpts,
    /// The component under the cursor: (shape, kind, id).
    pub hover_comp: Option<(NodeId, ComponentMode, Index)>,
    /// Half the side of the square a click picks wires in, in device pixels: the reference's selectPref -clickBoxSize
    /// times the screen's scale.
    pub click_box: f64,
    /// UV topology per mesh shape, with the mesh it was worked out from.
    uv_topos: HashMap<NodeId, (usize, Arc<aztek_scene::uv_ops::UvTopo>)>,
    /// Ray trees for meshes not drawn here.
    loose_bvhs: HashMap<NodeId, (usize, MeshBvh)>,
    /// The paint brush under the cursor: world centre and normal.
    pub brush: Option<(DVec3, DVec3)>,
    /// A curve tool's points so far, and its degree.
    pub curve_points: Vec<DVec3>,
    /// The joint tool's joint while the button is down: where its parent is, if it has one, and where it would go.
    pub joint_preview: Option<(Option<DVec3>, DVec3)>,
    pub curve_degree: i64,
    /// Manipulator axis orientation: world, object, component, parent or normal.
    pub axis_mode: String,
    /// The manipulator's axes as columns.
    pub frame: DMat3,
    /// What the modelling tools draw over the scene, world space.
    pub tool_lines: Vec<(DVec3, DVec3, [u8; 4])>,
    pub tool_points: Vec<(DVec3, [u8; 4])>,
    /// What CreateRound makes: polySphere, polyCylinder, polyCone or polyTorus.
    pub round_kind: String,
    /// The Show Manipulator tool's lines, world space.
    pub node_lines: Vec<(DVec3, DVec3, [u8; 4])>,
}

const WIRE: [u8; 4] = JOINT_COLOR;
/// The live surface's wire: the reference's dark green.
const LIVE_WIRE: [u8; 4] = [22, 96, 34, 255];

/// What a click can pick: everything but the live surface, which the reference leaves out of selection.
fn pickable(s: &Session, items: &[DrawItem]) -> Vec<DrawItem> {
    items.iter().filter(|i| !s.live_surface.contains(&i.shape)).map(|i| DrawItem { shape: i.shape, xf: i.xf, world: i.world, mesh: i.mesh.clone(), smooth: i.smooth }).collect()
}
const VERT: [u8; 4] = [170, 30, 170, 255];
const SEL_COMP: [u8; 4] = [255, 160, 30, 255];
/// The mirror image of what was picked, with symmetry on.
const SEL_MIRROR: [u8; 4] = [80, 190, 255, 255];
const UNSEL_EDGE: [u8; 4] = [40, 70, 200, 255];
/// UVs shown in a view.
const UV_POINT: [u8; 4] = [200, 60, 220, 255];

impl Viewport {
    pub fn new(kind: CamKind) -> Viewport {
        Viewport {
            lit: None,
            shadow: None,
            environment: None,
            planar: None,
            planar_skip: NO_NODE,
            planar_debug_color: None,
            back: false,
            rt_shadow_set: None,
            contact: None,
            look_camera: NO_NODE,
            cam: Camera::new(kind),
            meshes: HashMap::new(),
            mats: Default::default(),
            grid: None,
            grid_bold: None,
            grid_up_z: false,
            grid_colors: GridStyle::colors(),
            wireframe: false,
            wire_on_shaded: false,
            tool: Tool::Select,
            drag: None,
            hover: None,
            rect: [0.0, 0.0, 1.0, 1.0],
            marquee: None,
            grid_spacing: 1.0,
            grid_extent: 12.0,
            grid_style: GridStyle::default(),
            snap: 0,
            constraint: None,
            step_move: 0.0,
            step_rotate: 0.0,
            step_scale: 0.0,
            discrete_hold: false,
            disp: Display::default(),
            live_hit: None,
            live_auto: false,
            lasso: Vec::new(),
            select_brush: None,
            iso: Default::default(),
            pivot_mode: false,
            pivot_rings: false,
            show_grid: true,
            manip_scale: 1.0,
            smooth_cage: false,
            force_cage: false,
            live: LiveOpts::default(),
            hover_comp: None,
            click_box: 4.0,
            uv_topos: HashMap::new(),
            loose_bvhs: HashMap::new(),
            brush: None,
            curve_points: Vec::new(),
            joint_preview: None,
            curve_degree: 3,
            axis_mode: "world".into(),
            frame: DMat3::IDENTITY,
            tool_lines: Vec::new(),
            tool_points: Vec::new(),
            node_lines: Vec::new(),
            round_kind: String::new(),
            snap_relative: true,
            tweak: false,
            preserve_children: false,
            prevent_negative: true,
            custom_frame: DMat3::IDENTITY,
            keep_spacing: true,
            snap_pivot: false,
            snap_pivot_ori: false,
            use_manip_pivot: false,
        }
    }

    pub fn size(&self) -> DVec2 {
        DVec2::new(self.rect[2].max(1.0), self.rect[3].max(1.0))
    }

    pub fn aspect(&self) -> f64 {
        self.rect[2].max(1.0) / self.rect[3].max(1.0)
    }

    pub fn view_proj(&self) -> DMat4 {
        self.cam.proj(self.aspect()) * self.cam.view()
    }

    pub fn screen(&self) -> Screen {
        Screen { view_proj: self.view_proj(), width: self.rect[2], height: self.rect[3] }
    }

    /// The reference's grid: on the ground (XZ, or XY when Z is up), and in the view's own plane for the orthographic
    /// cameras; reaching the size each way, a grid line every spacing and subdivision lines between, the axes through
    /// the origin. Each kind can be turned off and has its own colour. Returns the thin lines and the axes when they
    /// are drawn thicker.
    fn grid_lines(&self) -> (Vec<LineVertex>, Vec<LineVertex>) {
        if let Some(b) = self.ortho_grid() {
            return self.ortho_grid_lines(&b);
        }
        let st = &self.grid_style;
        let n = ((self.grid_extent / self.grid_spacing).round() as i32).clamp(1, 2000);
        let div = st.divisions.max(1);
        let s = self.grid_spacing as f32;
        let to = self.grid_frame();
        let [axis, major, minor] = self.grid_colors;
        let (mut v, mut bold) = (Vec::new(), Vec::new());
        // Each line in unit pieces: lines reaching far behind the camera clip badly on some drivers.
        for i in -n..=n {
            let (c, out) = if i == 0 {
                if !st.axes {
                    if !st.lines {
                        continue;
                    }
                    (major, &mut v)
                } else {
                    (axis, if st.axes_bold { &mut bold } else { &mut v })
                }
            } else if i % div == 0 {
                if !st.lines {
                    continue;
                }
                (major, &mut v)
            } else {
                if !st.division_lines {
                    continue;
                }
                (minor, &mut v)
            };
            let t = i as f32 * s;
            for k in -n..n {
                let (a, b) = (k as f32 * s, (k + 1) as f32 * s);
                out.push(LineVertex { pos: to(t, a), color: c8(c) });
                out.push(LineVertex { pos: to(t, b), color: c8(c) });
                out.push(LineVertex { pos: to(a, t), color: c8(c) });
                out.push(LineVertex { pos: to(b, t), color: c8(c) });
            }
        }
        (v, bold)
    }

    /// An orthographic view's grid: in the view's own plane through the origin, whatever way the camera faces, and
    /// with no edge: it covers what the view shows. Subdivision lines closer than a few pixels are left out, and then
    /// grid lines, so a zoomed out view stays readable.
    fn ortho_grid(&self) -> Option<OrthoGrid> {
        if !self.cam.orthographic {
            return None;
        }
        let view = self.cam.view();
        let right = view.row(0).truncate().normalize_or_zero();
        let up = view.row(1).truncate().normalize_or_zero();
        let hw = self.cam.ortho_width.max(1e-6) * 0.5;
        let hh = hw / self.aspect().max(1e-6);
        let c = self.cam.coi;
        let (cu, cv) = (c.dot(right), c.dot(up));
        let px_per_unit = self.rect[2].max(1.0) / (2.0 * hw);
        let div = self.grid_style.divisions.max(1) as f64;
        let mut step = self.grid_spacing;
        let mut subdivisions = self.grid_style.division_lines;
        // Subdivisions under 5 pixels apart go; grid lines under 5 apart widen by the divisions.
        if step * px_per_unit < 5.0 {
            subdivisions = false;
            step *= div;
            while step * px_per_unit < 5.0 {
                step *= div.max(2.0);
            }
        }
        Some(OrthoGrid { right, up, u: (cu - hw, cu + hw), v: (cv - hh, cv + hh), step, subdivisions, coarse: step > self.grid_spacing * 1.5 })
    }

    fn ortho_grid_lines(&self, b: &OrthoGrid) -> (Vec<LineVertex>, Vec<LineVertex>) {
        let st = &self.grid_style;
        let div = st.divisions.max(1) as i64;
        let [axis, major, minor] = self.grid_colors;
        let (mut v, mut bold) = (Vec::new(), Vec::new());
        let at = |u: f64, w: f64| (b.right * u + b.up * w).as_vec3().to_array();
        for (dir, (lo, hi), (olo, ohi)) in [(0, b.u, b.v), (1, b.v, b.u)] {
            let (k0, k1) = ((lo / b.step).floor() as i64, (hi / b.step).ceil() as i64);
            for k in k0..=k1 {
                let t = k as f64 * b.step;
                let (c, out) = if k == 0 && st.axes {
                    (axis, if st.axes_bold { &mut bold } else { &mut v })
                } else if b.coarse || k % div == 0 {
                    if !st.lines {
                        continue;
                    }
                    (major, &mut v)
                } else {
                    if !b.subdivisions {
                        continue;
                    }
                    (minor, &mut v)
                };
                let (p, q) = if dir == 0 { (at(t, olo), at(t, ohi)) } else { (at(olo, t), at(ohi, t)) };
                out.push(LineVertex { pos: p, color: c8(c) });
                out.push(LineVertex { pos: q, color: c8(c) });
            }
        }
        (v, bold)
    }

    /// Maps (u, v) in the grid plane to world, through the Y up frame.
    pub fn grid_frame(&self) -> impl Fn(f32, f32) -> [f32; 3] {
        let z_up = aztek_scene::world::up_z();
        let kind = self.cam.kind;
        move |u: f32, v: f32| -> [f32; 3] {
            let p = match kind {
                CamKind::Front => [u, v, 0.0],
                CamKind::Side => [0.0, v, -u],
                _ => [u, 0.0, v],
            };
            if z_up {
                [p[0], -p[2], p[1]]
            } else {
                p
            }
        }
    }

    /// Grid numbers to draw: each grid line's value (in the current linear unit) and where it is in the world, along
    /// the axes or along the grid's edge, when the grid command asks for them in this kind of view.
    pub fn grid_labels(&self) -> Vec<(String, DVec3)> {
        let st = &self.grid_style;
        let ortho = matches!(self.cam.kind, CamKind::Top | CamKind::Front | CamKind::Side);
        let (on, edge) = if ortho { (st.ortho_labels, st.ortho_label_edge) } else { (st.persp_labels, st.persp_label_edge) };
        if !self.show_grid || !on {
            return Vec::new();
        }
        let k = aztek_ops::units::cm_per_unit();
        let label = |value: f64| if (value - value.round()).abs() < 1e-6 { format!("{}", value.round() as i64) } else { format!("{value:.2}") };
        if let Some(b) = self.ortho_grid() {
            // Along the view's axes through the origin, or its left and bottom edges.
            let major = if b.coarse { b.step } else { b.step * st.divisions.max(1) as f64 };
            let (eu, ev) = if edge { (b.u.0 + (b.u.1 - b.u.0) * 0.01, b.v.0 + (b.v.1 - b.v.0) * 0.02) } else { (0.0, 0.0) };
            let mut out = Vec::new();
            for (dir, (lo, hi)) in [(0, b.u), (1, b.v)] {
                for kk in (lo / major).ceil() as i64..=(hi / major).floor() as i64 {
                    let t = kk as f64 * major;
                    if kk == 0 && dir == 1 {
                        continue;
                    }
                    let p = if dir == 0 { b.right * t + b.up * ev } else { b.right * eu + b.up * t };
                    out.push((label(t / k), p));
                }
            }
            return out;
        }
        let n = ((self.grid_extent / self.grid_spacing).round() as i32).clamp(1, 2000);
        let div = st.divisions.max(1);
        let s = self.grid_spacing as f32;
        let to = self.grid_frame();
        let along = if edge { -(n as f32) * s } else { 0.0 };
        let mut out = Vec::new();
        for i in (-n..=n).filter(|i| i % div == 0) {
            let t = i as f32 * s;
            let text = label(t as f64 / k);
            out.push((text.clone(), glam::Vec3::from(to(t, along)).as_dvec3()));
            if i != 0 {
                out.push((text, glam::Vec3::from(to(along, t)).as_dvec3()));
            }
        }
        out
    }

    /// Drops GPU buffers of shapes no longer drawn.
    fn prune(&mut self, gpu: &mut Gpu, items: &[DrawItem]) {
        let live: Vec<NodeId> = items.iter().map(|i| i.shape).collect();
        let dead: Vec<NodeId> = self.meshes.keys().copied().filter(|k| !live.contains(k)).collect();
        for k in dead {
            let m = self.meshes.remove(&k).unwrap();
            drop_mesh(gpu, m);
        }
    }

    /// Reads the materials when the scene has changed; finds this frame's textures. A missing or broken image is a
    /// warning in the script history, once, and the material draws with the file node's default colour.
    pub fn prepare_materials(&mut self, gpu: &Gpu, s: &mut Session) {
        let stale_project = self.mats.project_read.is_none_or(|t| t.elapsed().as_secs_f64() > 2.0);
        if stale_project {
            self.mats.project = shading::project_dir(s);
            self.mats.project_read = Some(std::time::Instant::now());
        }
        let g = &mut s.graph;
        if g.version() != self.mats.version || self.mats.materials.is_empty() {
            self.mats.members = shading::membership(g);
            let scene = s.scene_path.clone();
            let project = self.mats.project.clone();
            let mut materials = HashMap::new();
            for sg in shading::all_groups(&s.graph) {
                if let Some(m) = shading::draw_material(&mut s.graph, sg, scene.as_deref(), project.as_deref()) {
                    materials.insert(sg, m);
                }
            }
            let g = &mut s.graph;
            self.mats.fallback = g.find(shading::INITIAL_SG).ok().flatten().and_then(|sg| materials.get(&sg).cloned());
            self.mats.materials = materials;
            self.mats.version = g.version();
            crate::textures::with_cache(gpu, |c| {
                for m in self.mats.materials.values() {
                    for t in [&m.color_map, &m.alpha_map].into_iter().flatten() {
                        if t.resolved.is_none() && !t.written.is_empty() {
                            c.warn(format!("Cannot find texture {}", t.written));
                        }
                    }
                }
            });
        }
        self.mats.sets.clear();
        if !self.disp.textured || !r_supports(gpu) {
            return;
        }
        let mut found = Vec::new();
        crate::textures::with_cache(gpu, |c| {
            for (sg, m) in &self.mats.materials {
                let mut load = |t: &shading::TextureRef| match &t.baked {
                    Some(pic) => c.get_picture(gpu, pic),
                    None => t.resolved.as_ref().and_then(|p| c.get(gpu, p, t.srgb)),
                };
                let color = m.color_map.as_ref().and_then(|t| load(t).map(|l| (t.file, l)));
                let alpha = m.alpha_map.as_ref().and_then(|t| load(t).map(|l| (t.file, l)));
                self.mats.sets.insert(*sg, MatSets { color: color.map(|x| x.1.set), alpha: alpha.map(|x| x.1.set), alpha_has_alpha: alpha.is_some_and(|x| x.1.has_alpha) });
                found.extend(color);
                found.extend(alpha);
            }
        });
        // What the image says of itself, as the file node reports it. Not an edit: no undo.
        let g = &mut s.graph;
        for (file, l) in found {
            let Some(n) = g.node(file) else { continue };
            let t = n.type_arc().clone();
            let (Some(fha), Some(os)) = (t.attr_index("fileHasAlpha"), t.attr_index("outSize")) else { continue };
            let size = DVec3::new(l.size.0 as f64, l.size.1 as f64, 0.0);
            let same = n.bool(fha) == l.has_alpha && n.data().vec3(os) == size;
            if !same {
                g.without_undo(|g| {
                    let _ = g.set_value(aztek_scene::PlugRef::new(file, fha as i32), aztek_scene::PlugValue::Bool(l.has_alpha));
                    let _ = g.set_value(aztek_scene::PlugRef::new(file, os as i32), aztek_scene::PlugValue::Vec3(size));
                });
            }
        }
    }

    /// Whether any material in the scene lets light through, so the behind-glass draw is worth making (#302).
    pub fn needs_back(&self) -> bool {
        self.mats.materials.values().any(|m| m.pbr.transmission_weight > 0.5)
    }


    /// Whether any material in the scene is glossy enough to reflect, so the ground plane's mirrored draw is worth
    /// making (#302).
    /// The shapes whose shading group lets light through, for the shadow map to leave out: glass passes light, so an
    /// opaque shadow under it would be wrong, and the glass would refract that black floor.
    pub fn transmissive_shapes(&self, items: &[DrawItem]) -> std::collections::HashSet<NodeId> {
        let groups: Vec<NodeId> = self
            .mats
            .materials
            .iter()
            .filter(|(_, m)| m.pbr.transmission_weight > 0.5)
            .map(|(&sg, _)| sg)
            .collect();
        if groups.is_empty() {
            return Default::default();
        }
        let member = |node: NodeId| {
            groups.iter().any(|sg| {
                self.mats
                    .members
                    .get(sg)
                    .is_some_and(|ms| ms.iter().any(|(m, _)| *m == node))
            })
        };
        items
            .iter()
            .filter(|it| member(it.shape) || member(it.xf))
            .map(|it| it.shape)
            .collect()
    }

    pub fn needs_planar(&self) -> bool {
        self.mats.materials.values().any(|m| {
            m.pbr.metalness > 0.05 || (m.pbr.specular_weight > 0.0 && m.pbr.specular_roughness < 0.45)
        })
    }

    /// This frame's lights, highlights and links, as the view's Lighting menu asks. Runs before the frame's command
    /// buffer, so the shadow map it plans can be drawn first.
    pub fn prepare_lights(&mut self, gpu: &Gpu, r: &Renderer, s: &mut Session, items: &[DrawItem], realtime: bool) {
        if !r.materials {
            return;
        }
        let lit = self.lit.get_or_insert_with(|| crate::lighting::ViewLights::new(gpu, r));
        lit.write_speculars(self.mats.version, self.mats.materials.iter());
        let mode = self.disp.light_mode;
        let chosen = crate::lighting::chosen(&mut s.graph, mode, visible);
        let env = crate::lighting::chosen_environment(&mut s.graph, mode, visible, s.scene_path.as_deref(), self.mats.project.as_deref());
        lit.write_environment(gpu, env.as_ref());
        if !chosen.is_empty() {
            let used: Vec<NodeId> = chosen.iter().map(|l| l.shape).collect();
            lit.update_masks(&s.graph, &used, items.iter().map(|i| i.shape));
        }
        // The Realtime renderer draws one shadow map, for the first light that casts one, when Shadows is on.
        self.shadow = if realtime && self.disp.shadows { crate::lighting::shadow_plan(&chosen, items) } else { None };
        self.environment = env;
        let shadow_plan = self.shadow;
        let (view, proj) = (self.cam.view(), self.cam.proj(self.rect[2].max(1.0) / self.rect[3].max(1.0)));
        // The contact shadow goes the other way from the light's shine, and reaches a little way into the scene.
        self.contact = self.shadow.and_then(|p| {
            let l = chosen.get(p.light)?;
            let toward = view.transform_vector3(l.world.z_axis.truncate()).normalize_or_zero();
            let reach = (self.shadow.map_or(0.0, |p| p.texel) * 8.0).clamp(0.01, 0.5) as f32;
            Some((toward.as_vec3().to_array(), reach, 0.6))
        });
        // The block carries the shadow camera in view space: a 2D map's matrix, or a cube light's position.
        lit.write_block(mode, &chosen, &view, &proj, self.disp.two_sided, self.cam.orthographic, shadow_plan.map(|p| p.block(&view)));
    }

    /// This frame's lights again, in the planar reflection's own buffer: the same view and lights as the camera's, with
    /// no shadow map, since the mirrored geometry would sample it at the wrong place (#302).
    pub fn write_lights_for(&mut self, s: &mut Session, _items: &[DrawItem], view: DMat4, proj: DMat4, mode: crate::lighting::LightMode) {
        let Some(lit) = self.lit.as_mut() else { return };
        // The same lights as the camera's own pass, re-selected so they can go in the planar buffer without the shadow
        // map. The per-shape masks are already the ones prepare_lights worked out for those same lights.
        let chosen = crate::lighting::chosen(&mut s.graph, mode, visible);
        lit.write_planar_block(mode, &chosen, &view, &proj, self.disp.two_sided, self.cam.orthographic);
    }

    pub fn draw(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session, items: &[DrawItem], scale: f64, realtime: bool) {
        self.prune(gpu, items);
        let [x, y, w, h] = self.rect;
        if w < 2.0 || h < 2.0 {
            return;
        }
        r.set_viewport(gpu, x as f32, y as f32, w as f32, h as f32);
        let planar = self.planar.is_some();
        // The planar pass keeps the main camera: the models are mirrored instead.
        let view = self.cam.view();
        let vp = self.cam.proj((w / h.max(1.0)).max(1e-6)) * view;
        let colors = GridStyle::colors();
        // An orthographic view's grid follows what it shows.
        if self.grid.is_some() && (self.cam.orthographic || self.grid_up_z != aztek_scene::world::up_z() || self.grid_colors != colors) {
            self.reset_grid(gpu);
        }
        if self.grid.is_none() {
            self.grid_up_z = aztek_scene::world::up_z();
            self.grid_colors = colors;
            let (lines, bold) = self.grid_lines();
            // Bands that fade at their edges, smooth whatever the samples (#203).
            let (lines, bold) = (crate::render::smooth_lines(&lines), crate::render::smooth_lines(&bold));
            self.grid = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lines), lines.len() as u32));
            self.grid_bold = (!bold.is_empty()).then(|| (gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &bold), bold.len() as u32));
        }
        let g = &s.graph;
        let mode = components::mode(g);
        let sel = g.selection().to_vec();
        // The UV Editor's Checker: on the meshes whose UVs it shows, the selected ones.
        let checker_set = (r.materials && crate::ui_cmds::pref_bool("uvChecker", false)).then(|| crate::textures::with_cache(gpu, |c| c.checker(gpu)));
        let lead = sel.last().copied();
        let mut transparent: Vec<Clear> = Vec::new();
        let smooth_cage = self.smooth_cage || (self.force_cage && self.tool == Tool::MultiCut);
        for it in items {
            // The ground plane's own mesh is the reflector, not part of what it reflects (#302).
            if planar && it.shape == self.planar_skip {
                continue;
            }
            // The planar pass mirrors the model, not the camera: positions and normals follow, the view and lights
            // stay the main pass's (#302).
            let world = self.planar.map_or(it.world, |m| m * it.world);
            let entry = self.meshes.entry(it.shape).or_insert_with(|| GpuMesh {
                drawn: None,
                mesh_gen: 0,
                topology_gen: 0,
                vertex_corner: Vec::new(),
                smooth: 0,
                display: None,
                tris: None,
                tri_faces: Vec::new(),
                tri_indices: Vec::new(),
                bvh: None,
                sel_key: u64::MAX,
                lines: None,
                points: None,
                tint: None,
                mirror_tint: None,
                vertex_point: Vec::new(),
                weight_tris: None,
                weight_key: u64::MAX,
                uvs: None,
                display_faces: Vec::new(),
                display_edges: Vec::new(),
                stencils: Vec::new(),
                batch_ib: None,
                batch_indices: Vec::new(),
                batch_faces: Vec::new(),
                batches: Vec::new(),
                batch_key: u64::MAX,
            });
            let same_mesh = entry.drawn.as_ref().is_some_and(|d| Arc::ptr_eq(d, &it.mesh));
            let mesh_changed = !same_mesh || entry.smooth != it.smooth;
            // A deformer's result: only the points moved, so only the vertices need to go up.
            let mut only_points = mesh_changed
                && it.smooth == entry.smooth
                && !entry.vertex_corner.is_empty()
                && entry.tris.is_some()
                && entry.drawn.as_ref().is_some_and(|d| d.only_points_differ(&it.mesh));
            // Smooth preview: the smooth mesh's points by the stencils, its topology as it was.
            if only_points && it.smooth > 0 {
                let o = aztek_geo::ops::subdivide::SubdivideOptions { levels: it.smooth, ..Default::default() };
                if entry.stencils.is_empty() {
                    entry.stencils = aztek_geo::ops::subdivide::subdivide_stencils(&it.mesh, &o, None);
                }
                let mut points = it.mesh.positions().to_vec();
                for stencil in &entry.stencils {
                    points = stencil.apply(&points);
                }
                match entry.display.as_ref().filter(|d| !entry.stencils.is_empty() && d.positions().len() == points.len()) {
                    Some(d) => entry.display = Some(Arc::new(d.with_positions(points))),
                    None => only_points = false,
                }
            }
            if only_points {
                let opts = RenderMeshOptions { max_uv_sets: 1, ..Default::default() };
                let shown = entry.display.clone().unwrap_or_else(|| it.mesh.clone());
                let (positions, normals) = aztek_geo::render_mesh::moved_vertices(&shown, &entry.vertex_point, &entry.vertex_corner, &opts);
                let verts: Vec<ShadedVertex> = positions.iter().zip(&normals).map(|(p, n)| ShadedVertex { pos: p.to_array(), nrm: n.to_array() }).collect();
                if let Some((old, indices, count)) = entry.tris.take() {
                    gpu.drop_buffer(old);
                    entry.tris = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts), indices, count));
                }
                entry.bvh = None;
                entry.drawn = Some(it.mesh.clone());
                entry.mesh_gen += 1;
                entry.sel_key = u64::MAX;
            } else if mesh_changed {
                entry.smooth = it.smooth;
                entry.stencils.clear();
                let mut sub_map = aztek_geo::ops::subdivide::SubdivideMap::default();
                entry.display = (it.smooth > 0).then(|| {
                    let o = aztek_geo::ops::subdivide::SubdivideOptions { levels: it.smooth, ..Default::default() };
                    Arc::new(aztek_geo::ops::subdivide::subdivide(&it.mesh, &o, Some(&mut sub_map)))
                });
                entry.display_faces = sub_map.faces;
                entry.display_edges = sub_map.edges;
                if let Some((a, b, _)) = entry.tris.take() {
                    gpu.drop_buffer(a);
                    gpu.drop_buffer(b);
                }
                let shown = entry.display.clone().unwrap_or_else(|| it.mesh.clone());
                let rm = build_render_mesh(&shown, &RenderMeshOptions { max_uv_sets: 1, ..Default::default() });
                if let Some(b) = entry.uvs.take() {
                    gpu.drop_buffer(b);
                }
                let uvs: Vec<[f32; 2]> = rm.uvs.first().cloned().unwrap_or_else(|| vec![[0.0; 2]; rm.positions.len()]);
                if !uvs.is_empty() {
                    entry.uvs = Some(gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &uvs));
                }
                let verts: Vec<ShadedVertex> = rm
                    .positions
                    .iter()
                    .zip(&rm.normals)
                    .map(|(p, n)| ShadedVertex { pos: p.to_array(), nrm: n.to_array() })
                    .collect();
                if !rm.indices.is_empty() {
                    entry.tris = Some((
                        gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts),
                        gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &rm.indices),
                        rm.indices.len() as u32,
                    ));
                }
                entry.vertex_point = rm.vertex_point.clone();
                entry.tri_faces = rm.triangle_faces.clone();
                entry.tri_indices = rm.indices.clone();
                entry.vertex_corner = rm.vertex_corner.clone();
                entry.bvh = None;
                entry.drawn = Some(it.mesh.clone());
                entry.mesh_gen += 1;
                entry.topology_gen += 1;
                entry.sel_key = u64::MAX;
                // Tint reuses the triangle vertex buffer; rebuilt with the selection key below.
            }
            let style = ds::style(g, it.shape);
            let templated = style.kind == ds::DisplayKind::Template;
            // A surface's tessellation is only shaded: isoparms and CVs are drawn with the rig lines.
            let is_surface = surfaces::is_surface(g, it.shape);
            let selected = !is_surface && self.disp.highlight && (sel.contains(&it.xf) || sel.contains(&it.shape));
            // The live surface shows no components, whatever the selection type (#125).
            let hilited = selected && mode != ComponentMode::Object && !s.live_surface.contains(&it.shape);
            let checkered = !is_surface && (sel.contains(&it.xf) || sel.contains(&it.shape));
            let child_lit = !is_surface && self.disp.highlight && ds::select_state(g, it.shape, &sel) == ds::SelectState::Child;
            let wire_sel: &[NodeId] = if self.disp.highlight { &sel } else { &[] };
            // The live surface: shaded as anything else, with its dark green wire over it only when the view shows wires,
            // Wireframe on Shaded say, as the reference draws it (#116, #162).
            let live = s.live_surface.contains(&it.shape);
            let wire = if live { LIVE_WIRE } else { ds::wire_color(g, it.shape, wire_sel, "polymesh", WIRE) };
            let comps: &[u32] = if hilited { components::selected(g, it.shape, mode) } else { &[] };
            let comp_set: std::collections::HashSet<u32> = comps.iter().copied().collect();
            // Multi shows its vertices, edges and faces together; each kind's own selection, by kind.
            let multi = components::is_multi(g);
            let kind_set = |k: ComponentMode| -> std::collections::HashSet<u32> {
                if !hilited {
                    Default::default()
                } else if multi && matches!(k, ComponentMode::Vertex | ComponentMode::Edge | ComponentMode::Face) {
                    components::selected(g, it.shape, k).iter().copied().collect()
                } else if k == mode && !multi {
                    comp_set.clone()
                } else {
                    Default::default()
                }
            };
            let (v_set, e_set, f_set) = (kind_set(ComponentMode::Vertex), kind_set(ComponentMode::Edge), kind_set(ComponentMode::Face));
            let shows = |k: ComponentMode| hilited && (mode == k || (multi && matches!(k, ComponentMode::Vertex | ComponentMode::Edge | ComponentMode::Face)));
            let soft = aztek_scene::soft_select::state();
            let global_seeds = soft.0 && mode != ComponentMode::Object && aztek_scene::soft_select::falloff_name() == "global";
            let soft_on = soft.0 && ((hilited && !comps.is_empty()) || (global_seeds && mode != ComponentMode::Object));
            let mut h = DefaultHasher::new();
            (mode as i32, selected, lead == Some(it.xf), self.wireframe, self.wire_on_shaded, comps, smooth_cage, entry.smooth, templated).hash(&mut h);
            if multi && hilited {
                (components::selected(g, it.shape, ComponentMode::Vertex), components::selected(g, it.shape, ComponentMode::Edge), components::selected(g, it.shape, ComponentMode::Face)).hash(&mut h);
            }
            (wire, child_lit, style.shaded).hash(&mut h);
            (soft_on, soft.1.to_bits(), entry.mesh_gen).hash(&mut h);
            if soft.0 {
                aztek_scene::soft_select::signature(g, mode).hash(&mut h);
            }
            let sel_key = h.finish();
            if sel_key != entry.sel_key {
                entry.sel_key = sel_key;
                for b in [entry.lines.take(), entry.points.take(), entry.tint.take(), entry.mirror_tint.take()].into_iter().flatten() {
                    gpu.drop_buffer(b.0);
                }
                // Smooth preview: the cage's wire on the 2 key and in component modes, the smooth mesh's otherwise.
                let cage = entry.display.is_none() || smooth_cage || hilited;
                let wire_mesh = if cage { it.mesh.clone() } else { entry.display.clone().unwrap() };
                let m = &wire_mesh;
                let mirror_set: std::collections::HashSet<u32> = if hilited { components::mirrored_side(g, it.shape, mode, comps).into_iter().collect() } else { Default::default() };
                let edge_color = |e: Index| -> [u8; 4] {
                    if hilited {
                        if e_set.contains(&e) {
                            if mirror_set.contains(&e) { SEL_MIRROR } else { SEL_COMP }
                        } else {
                            UNSEL_EDGE
                        }
                    } else {
                        wire
                    }
                };
                let want_lines = !is_surface && ((selected && !live) || child_lit || !style.shaded || self.wireframe || self.wire_on_shaded || (entry.display.is_some() && smooth_cage));
                if want_lines {
                    let mut lv = Vec::new();
                    // Smooth preview's wire: the cage's edges follow the smooth surface, and those the subdivision added
                    // are fainter. Those go down first, so a cage edge is always drawn over them.
                    let added = |e: Index| !cage && entry.display_edges.get(e as usize).is_some_and(|&from| from == aztek_geo::INVALID);
                    for main in [false, true] {
                        for e in 0..m.edge_capacity() {
                            if !m.edge_valid(e) || added(e) == main {
                                continue;
                            }
                            let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
                            let mut c = edge_color(e);
                            if !main {
                                c[3] = SUBDIVISION_LINE_ALPHA;
                            }
                            lv.push(LineVertex { pos: m.position(a).as_vec3().to_array(), color: c8(c) });
                            lv.push(LineVertex { pos: m.position(b).as_vec3().to_array(), color: c8(c) });
                        }
                    }
                    if !lv.is_empty() {
                        entry.lines = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lv), lv.len() as u32));
                    }
                }
                if soft_on && aztek_scene::soft_select::false_color_enabled() {
                    // Soft selection's falloff in false colour, by softSelect's colour ramp: yellow at the selection
                    // through red to black, to begin with.
                    let pts = components::convert(&it.mesh, mode, comps, ComponentMode::Vertex, false);
                    let w = aztek_scene::mesh_edit::transform::soft_weights(g, &it.mesh, &world, &pts);
                    let ramp = aztek_scene::soft_select::FalseColors::current();
                    let mut pv = Vec::new();
                    for p in 0..it.mesh.point_capacity() {
                        if !it.mesh.point_valid(p) {
                            continue;
                        }
                        let x = w.get(p as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                        let c = if x <= 0.0 {
                            if mode == ComponentMode::Vertex && hilited { [10, 10, 10, 255] } else { continue }
                        } else {
                            ramp.at(x as f64)
                        };
                        pv.push(LineVertex { pos: it.mesh.position(p).as_vec3().to_array(), color: c8(c) });
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                } else if shows(ComponentMode::Vertex) {
                    let mut pv = Vec::new();
                    for p in 0..m.point_capacity() {
                        if m.point_valid(p) {
                            let c = if mirror_set.contains(&p) { SEL_MIRROR } else if v_set.contains(&p) { SEL_COMP } else { VERT };
                            pv.push(LineVertex { pos: m.position(p).as_vec3().to_array(), color: c8(c) });
                        }
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                } else if hilited && mode == ComponentMode::Uv {
                    // UVs at their points: a point is lit when one of its UVs is selected.
                    let lit: std::collections::HashSet<u32> = components::convert(&it.mesh, ComponentMode::Uv, comps, ComponentMode::Vertex, false).into_iter().collect();
                    let mut pv = Vec::new();
                    for p in 0..it.mesh.point_capacity() {
                        if it.mesh.point_valid(p) {
                            let c = if lit.contains(&p) { SEL_COMP } else { UV_POINT };
                            pv.push(LineVertex { pos: it.mesh.position(p).as_vec3().to_array(), color: c8(c) });
                        }
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                } else if hilited && mode == ComponentMode::VertexFace {
                    // A point a little way into each face at each of its corners.
                    let mut pv = Vec::new();
                    for c in 0..it.mesh.corner_capacity() {
                        if it.mesh.corner_valid(c) {
                            let col = if comp_set.contains(&c) { SEL_COMP } else { VERT };
                            pv.push(LineVertex { pos: Self::vertex_face_point(&it.mesh, c).as_vec3().to_array(), color: c8(col) });
                        }
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                }
                if shows(ComponentMode::Face) && !f_set.is_empty() {
                    let mut ti = Vec::new();
                    let mut mi = Vec::new();
                    for (t, &f) in entry.tri_faces.iter().enumerate() {
                        if f_set.contains(&f) {
                            let into = if mirror_set.contains(&f) { &mut mi } else { &mut ti };
                            into.extend_from_slice(&entry.tri_indices[t * 3..t * 3 + 3]);
                        }
                    }
                    if !mi.is_empty() {
                        entry.mirror_tint = Some((gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &mi), mi.len() as u32));
                    }
                    if !ti.is_empty() {
                        entry.tint = Some((gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &ti), ti.len() as u32));
                    }
                }
            }
            // The triangles by shading group, when either has changed.
            let group_list: Vec<(NodeId, Option<Vec<u32>>)> = {
                let held = self.mats.members.get(&it.shape).cloned();
                held.unwrap_or_else(|| {
                    let src = if g.node(it.shape).is_some_and(|n| n.ty().is_a("mesh")) { aztek_scene::instancing::source(g, it.shape) } else { NO_NODE };
                    self.mats.members.get(&src).cloned().unwrap_or_default()
                })
            };
            let entry = self.meshes.get_mut(&it.shape).unwrap();
            let mut bh = DefaultHasher::new();
            (entry.topology_gen, entry.smooth).hash(&mut bh);
            group_list.hash(&mut bh);
            let batch_key = bh.finish();
            if batch_key != entry.batch_key {
                entry.batch_key = batch_key;
                if let Some(b) = entry.batch_ib.take() {
                    gpu.drop_buffer(b);
                }
                let face_group = shading::face_groups(&it.mesh, &group_list);
                let cage_of = |t: usize| -> Index {
                    let f = entry.tri_faces[t];
                    if entry.display.is_some() {
                        entry.display_faces.get(f as usize).copied().unwrap_or(0)
                    } else {
                        f
                    }
                };
                let tri_count = entry.tri_faces.len();
                let mut order: Vec<(usize, u32)> = (0..tri_count).map(|t| (face_group.get(cage_of(t) as usize).copied().unwrap_or(usize::MAX), t as u32)).collect();
                order.sort_unstable();
                let (mut indices, mut faces, mut batches) = (Vec::with_capacity(tri_count * 3), Vec::with_capacity(tri_count), Vec::new());
                let mut k = 0;
                while k < order.len() {
                    let mut end = k;
                    while end < order.len() && order[end].0 == order[k].0 {
                        end += 1;
                    }
                    let group = group_list.get(order[k].0).map_or(NO_NODE, |x| x.0);
                    let first = indices.len() as u32;
                    for &(_, t) in &order[k..end] {
                        let t = t as usize;
                        indices.extend_from_slice(&entry.tri_indices[t * 3..t * 3 + 3]);
                        faces.push(cage_of(t));
                    }
                    batches.push(Batch { group, first, count: indices.len() as u32 - first });
                    k = end;
                }
                if !indices.is_empty() {
                    entry.batch_ib = Some(gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &indices));
                }
                entry.batch_indices = indices;
                entry.batch_faces = faces;
                entry.batches = batches;
            }
            let mvp = vp * world;
            // Paint skin weights: the painted influence's weight in grey, white for 1, as the reference's colour feedback.
            let mut painted = false;
            // The values in grey, white for 1, as the reference's colour feedback; a held influence reads blue.
            let feedback: Option<(Vec<f64>, u64, bool)> = if !selected || self.wireframe || entry.display.is_some() {
                None
            } else if self.tool == Tool::PaintSkin && s.paint_ctx.color_feedback && s.paint_ctx.skin_paint_mode != 0 {
                // Select mode picks components on the plain shaded mesh, as the reference's (#104).
                aztek_scene::skin::skin_of(g, it.shape).and_then(|sk| {
                    // With Skin Tools layers: the current layer's paint target.
                    if let Some(l) = aztek_scene::skin_layers::layers_of(g, sk) {
                        if let Some(id) = l.current {
                            if let Some(t) = aztek_scene::skin_layers::paint_target(g, sk, &l, id) {
                                let v = aztek_scene::skin_layers::target_values(&l, id, t);
                                let mut k = DefaultHasher::new();
                                (id, format!("{t:?}"), v.iter().map(|x| x.to_bits()).fold(0u64, |a, b| a.rotate_left(5) ^ b)).hash(&mut k);
                                return Some((v, k.finish(), false));
                            }
                        }
                    }
                    let (bind, inf) = (aztek_scene::skin::bind_of(g, sk)?, aztek_scene::skin::paint_influence(s, sk)?);
                    Some((aztek_scene::skin::point_weights(&bind, inf), inf as u64, bind.locks.get(inf as usize).copied().unwrap_or(false)))
                })
            } else if self.tool == Tool::PaintAttr && s.attr_ctx.paint.color_feedback {
                aztek_scene::deform_paint::current(s, it.shape).map(|(node, slot)| {
                    let mut k = DefaultHasher::new();
                    (node, slot).hash(&mut k);
                    (aztek_scene::deform_paint::read(g, node, slot, it.mesh.point_capacity() as usize), k.finish(), false)
                })
            } else {
                None
            };
            if let Some((w, slot_key, held)) = feedback {
                let mut h = DefaultHasher::new();
                for x in &w {
                    x.to_bits().hash(&mut h);
                }
                (entry.mesh_gen, slot_key, held, self.tool == Tool::PaintSkin && s.paint_ctx.use_color_ramp).hash(&mut h);
                let wk = h.finish();
                if wk != entry.weight_key || entry.weight_tris.is_none() {
                    entry.weight_key = wk;
                    if let Some((a, b, _)) = entry.weight_tris.take() {
                        gpu.drop_buffer(a);
                        gpu.drop_buffer(b);
                    }
                    let ramp = self.tool == Tool::PaintSkin && s.paint_ctx.use_color_ramp;
                    let verts: Vec<LineVertex> = entry
                        .vertex_point
                        .iter()
                        .map(|&pt| {
                            let x = w.get(pt as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
                            let color = if held {
                                [x * 0.55, x * 0.7, 0.35 + x * 0.65, 1.0]
                            } else if ramp {
                                weight_ramp(x)
                            } else {
                                [x, x, x, 1.0]
                            };
                            LineVertex { pos: it.mesh.position(pt).as_vec3().to_array(), color }
                        })
                        .collect();
                    if !verts.is_empty() && !entry.tri_indices.is_empty() {
                        entry.weight_tris = Some((
                            gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts),
                            gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &entry.tri_indices),
                            entry.tri_indices.len() as u32,
                        ));
                    }
                }
                if let Some((vb, ib, n)) = &entry.weight_tris {
                    r.bind(gpu, Kind::ColorTris);
                    r.push(gpu, &push(mvp, world, view, [1.0; 4], 0.0, 1.0));
                    r.draw(gpu, vb, Some((ib, *n)), 0);
                    painted = true;
                }
            }
            // Quad Draw's mesh sits on the live surface, chords and all, so it's pulled forward to stay in sight.
            // The mesh being built on a live surface: pulled forward to stay in sight, and seen through in blue as the
            // reference shows it (#127).
            let on_live = !s.live_surface.is_empty() && !s.live_surface.contains(&it.shape) && (sel.contains(&it.xf) || sel.contains(&it.shape) || self.tool == Tool::QuadDraw);
            let retopo = if on_live { 0.03 } else { 0.0 };
            // Bounding box display: twelve lines and nothing else.
            if self.disp.bounding_box || style.bounding_box || crate::ui_cmds::node_flag(&s.graph, "boundingBox", it.xf) {
                let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
                for p in 0..it.mesh.point_capacity() {
                    if it.mesh.point_valid(p) {
                        let q = it.mesh.position(p);
                        lo = lo.min(q);
                        hi = hi.max(q);
                    }
                }
                let c = c8(wire);
                let corner = |i: usize| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z }).as_vec3().to_array();
                let mut v = Vec::new();
                for i in 0..8usize {
                    for bit in [1usize, 2, 4] {
                        if i & bit == 0 {
                            v.push(LineVertex { pos: corner(i), color: c });
                            v.push(LineVertex { pos: corner(i | bit), color: c });
                        }
                    }
                }
                let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
                r.bind(gpu, Kind::Lines);
                r.set_line_width(gpu, 1.0);
                r.push(gpu, &push(mvp, world, view, [1.0; 4], 1e-3, 1.0));
                r.draw(gpu, &b, None, v.len() as u32);
                gpu.drop_buffer(b);
                continue;
            }
            // A partly isolated mesh draws only its shown faces.
            let shown_mask = self.iso.shown_faces(it.shape, &it.mesh);
            let shown_mask = if shown_mask.is_empty() { self.iso.shown_faces(it.xf, &it.mesh) } else { shown_mask };
            let partial = if shown_mask.is_empty() || entry.display.is_some() {
                None
            } else {
                let mut ti = Vec::new();
                for (t, &f) in entry.tri_faces.iter().enumerate() {
                    if shown_mask.get(f as usize).copied().unwrap_or(1) != 0 {
                        ti.extend_from_slice(&entry.tri_indices[t * 3..t * 3 + 3]);
                    }
                }
                (!ti.is_empty()).then(|| (gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &ti), ti.len() as u32))
            };
            if !self.wireframe && !painted && style.shaded {
                if let Some((vb, ib, n)) = &entry.tris {
                    let (ib, n) = match &partial {
                        Some((pb, pn)) => (pb, pn),
                        None => (ib, n),
                    };
                    // Per object too (Modeling Toolkit > Object > X-Ray, displaySurface -x; displayCull -bfc).
                    let own = |k: &str| [it.xf, it.shape].iter().any(|&n| s.extras.display_surface.get(&(n, k.to_string())).copied().unwrap_or(false));
                    let (obj_xray, obj_cull, obj_flip) = (own("xRay"), own("backFaceCulling"), own("flipNormals"));
                    // displaySurface -two off: single sided, so the back faces go.
                    let obj_cull = obj_cull || [it.xf, it.shape].iter().any(|&n| s.extras.display_surface.get(&(n, "twoSidedLighting".to_string())) == Some(&false));
                    let flip = |pc: &mut Push| {
                        if obj_flip {
                            for row in [&mut pc.n0, &mut pc.n1, &mut pc.n2] {
                                for v in row.iter_mut().take(3) {
                                    *v = -*v;
                                }
                            }
                        }
                    };
                    if on_live && self.disp.highlight && self.live.color.is_some() {
                        r.bind(gpu, Kind::Tint);
                        let c = self.live.color.unwrap_or([0.2, 0.42, 0.95]);
                        let mut pc = push(mvp, world, view, [c[0], c[1], c[2], self.live.alpha.unwrap_or(1.0)], retopo, 1.0);
                        pc.n2[3] = self.disp.flat as i32 as f32;
                        flip(&mut pc);
                        r.push(gpu, &pc);
                        r.draw(gpu, vb, Some((ib, *n)), 0);
                    } else if self.disp.xray || obj_xray {
                        // X-Ray: see-through, no sorting.
                        r.bind(gpu, Kind::Tint);
                        let mut pc = push(mvp, world, view, [0.55, 0.55, 0.55, 0.45], retopo, 1.0);
                        pc.n2[3] = self.disp.flat as i32 as f32;
                        flip(&mut pc);
                        r.push(gpu, &pc);
                        r.draw(gpu, vb, Some((ib, *n)), 0);
                    } else if self.disp.default_material && (self.disp.light_mode == crate::lighting::LightMode::Default || !r.materials) && !(checker_set.is_some() && checkered) {
                        r.bind(gpu, if self.disp.backface || obj_cull { Kind::ShadedCull } else { Kind::Shaded });
                        let mut pc = push(mvp, world, view, [0.5, 0.5, 0.5, 1.0], retopo, 1.0);
                        pc.n2[3] = self.disp.flat as i32 as f32;
                        flip(&mut pc);
                        r.push(gpu, &pc);
                        r.draw(gpu, vb, Some((ib, *n)), 0);
                    } else {
                        let batches = entry.batches.clone();
                        // A partly isolated mesh: its shown faces of each batch, for this frame.
                        let mut shown_batches: Option<(Buffer, Vec<Batch>)> = None;
                        if partial.is_some() {
                            let (mut ti, mut list) = (Vec::new(), Vec::new());
                            for b in &batches {
                                let first = ti.len() as u32;
                                for t in 0..(b.count / 3) as usize {
                                    let at = b.first as usize / 3 + t;
                                    if shown_mask.get(entry.batch_faces[at] as usize).copied().unwrap_or(1) != 0 {
                                        ti.extend_from_slice(&entry.batch_indices[at * 3..at * 3 + 3]);
                                    }
                                }
                                if ti.len() as u32 > first {
                                    list.push(Batch { group: b.group, first, count: ti.len() as u32 - first });
                                }
                            }
                            if !ti.is_empty() {
                                shown_batches = Some((gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &ti), list));
                            }
                        }
                        let (draw_ib, draw_list): (Option<&Buffer>, &[Batch]) = match &shown_batches {
                            Some((b, l)) => (Some(b), l),
                            None if partial.is_some() => (None, &[]),
                            None => (entry.batch_ib.as_ref(), &batches),
                        };
                        let depth = (view * world.w_axis).z;
                        for (bi, b) in draw_list.iter().enumerate() {
                            let green = no_material();
                            let grey = default_grey();
                            let white_mat = DrawMaterial { color: [1.0; 3], ..no_material() };
                            let checker_sets = checker_set.filter(|_| checkered).map(|set| MatSets { color: Some(set), alpha: None, alpha_has_alpha: false });
                            let plain = self.disp.default_material || checker_sets.is_some();
                            let m = if checker_sets.is_some() {
                                &white_mat
                            } else if plain {
                                &grey
                            } else if b.group == NO_NODE {
                                &green
                            } else {
                                self.mats.materials.get(&b.group).or(self.mats.fallback.as_ref()).unwrap_or(&green)
                            };
                            // A flat colour for measuring the mirrored pass; not used for the normal picture.
                            let flat = if planar { self.planar_debug_color.map(|c| DrawMaterial { unlit: true, color: c, ..no_material() }) } else { None };
                            let m = flat.as_ref().unwrap_or(m);
                            if self.back && m.pbr.transmission_weight > 0.5 {
                                continue;
                            }
                            // The mirrored vertex keeps the real model's normal: the planar image is the real scene's
                            // shading, mirrored about the floor, which is what the floor shows (#302).
                            let shade = if planar { it.world } else { world };
                            let mut pc = push(mvp, shade, view, [m.color[0], m.color[1], m.color[2], 1.0], retopo, 1.0);
                            pc.n2[3] = self.disp.flat as i32 as f32;
                            flip(&mut pc);
                            let (mut mp, clear) = material_push(pc, m, if checker_sets.is_some() { checker_sets.as_ref() } else if plain { None } else { self.mats.sets.get(&b.group).filter(|_| style.textured) });
                            stamp_lights(&self.lit, &mut mp, it.shape, if plain { NO_NODE } else { b.group });
                            // Glass is drawn with the solid surfaces when the Realtime renderer's screen-space refraction
                            // pass takes it apart (#302); the legacy renderer still blends it.
                            let refractive = realtime && r.realtime && m.pbr.transmission_weight > 0.5;
                            // The isolated draws are for this frame only, so see-through ones go now rather than later.
                            if clear && !refractive && shown_batches.is_none() && r.materials {
                                transparent.push(Clear { shape: it.shape, world, batch: bi, depth, retopo });
                                continue;
                            }
                            let (Some(ib), Some(uvs)) = (draw_ib, entry.uvs.as_ref()) else { continue };
                            if r.materials {
                                let sets = if checker_sets.is_some() { checker_sets.as_ref() } else if plain { None } else { self.mats.sets.get(&b.group).filter(|_| style.textured) };
                                let white = crate::textures::with_cache(gpu, |c| c.white(gpu));
                                let solid = !clear || refractive;
                                let kind = match (solid, self.disp.backface || obj_cull) {
                                    (true, false) => Kind::Material,
                                    (true, true) => Kind::MaterialCull,
                                    (false, false) => Kind::MaterialBlend,
                                    (false, true) => Kind::MaterialBlendCull,
                                };
                                // The planar draw has no shadow map: its shadow would be sampled at the mirrored point.
                                let lights = self.lit.as_ref().map_or(vk::DescriptorSet::null(), |l| if planar { l.planar_set } else { l.set });
                                if realtime && r.realtime {
                                    let env = self.lit.as_ref().and_then(|l| l.env_set).unwrap_or(white);
                                    let shadow = self.rt_shadow_set.unwrap_or(white);
                                    let rp = realtime_push(&mp, m, self.lit.as_ref().map_or(0.0, |l| l.index.get(&b.group).copied().unwrap_or(0) as f32));
                                    if planar || self.back {
                                        r.bind_realtime_planar(gpu, kind, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), lights, env, shadow);
                                    } else {
                                        r.bind_realtime(gpu, kind, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), lights, env, shadow);
                                    }
                                    r.push_realtime(gpu, &rp);
                                } else {
                                    r.bind_material(gpu, kind, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), lights);
                                    r.push_material(gpu, &mp);
                                }
                                r.draw_material(gpu, vb, uvs, ib, b.first, b.count);
                            } else {
                                // A device without room for the material constants: flat colours.
                                r.bind(gpu, if clear { Kind::Tint } else if self.disp.backface || obj_cull { Kind::ShadedCull } else { Kind::Shaded });
                                let mut pc = mp.base;
                                pc.color = [m.color[0] * m.diffuse, m.color[1] * m.diffuse, m.color[2] * m.diffuse, mp.base.color[3]];
                                r.push(gpu, &pc);
                                r.draw_range(gpu, vb, ib, b.first, b.count);
                            }
                        }
                        if let Some((b, _)) = shown_batches {
                            gpu.drop_buffer(b);
                        }
                    }
                    if !planar && !self.back {
                    if let Some((ti, tn)) = &entry.mirror_tint {
                        r.bind(gpu, Kind::Tint);
                        r.push(gpu, &push(mvp, world, view, [0.2, 0.6, 1.0, 0.45], 2e-4, 1.0));
                        r.draw(gpu, vb, Some((ti, *tn)), 0);
                    }
                    if let Some((ti, tn)) = &entry.tint {
                        r.bind(gpu, Kind::Tint);
                        r.push(gpu, &push(mvp, world, view, [1.0, 0.45, 0.05, 0.45], 2e-4, 1.0));
                        r.draw(gpu, vb, Some((ti, *tn)), 0);
                    }
                    }
                }
            }
            if let Some((b, _)) = partial {
                gpu.drop_buffer(b);
            }
            // Wireframe Overlay: a mesh built on the live surface draws its wire in the overlay's colour, unless its
            // components are showing.
            let overlay = if on_live && !hilited && !planar && !self.back { self.live.wire } else { None };
            if let Some(c) = overlay {
                let col = [c[0], c[1], c[2], 1.0];
                let m = &it.mesh;
                let mut lv = Vec::new();
                for e in (0..m.edge_capacity()).filter(|&e| m.edge_valid(e)) {
                    for k in 0..2 {
                        lv.push(LineVertex { pos: m.position(m.edge_point(e, k)).as_vec3().to_array(), color: col });
                    }
                }
                if !lv.is_empty() {
                    let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lv);
                    r.bind(gpu, Kind::Lines);
                    r.set_line_width(gpu, 1.0);
                    r.push(gpu, &push(mvp, world, view, [1.0; 4], 1.5e-3 + retopo, 1.0));
                    r.draw(gpu, &b, None, lv.len() as u32);
                    gpu.drop_buffer(b);
                }
            }
            if let Some((lb, ln)) = entry.lines.as_ref().filter(|_| overlay.is_none() && !planar && !self.back) {
                r.bind(gpu, if self.disp.xray_active && hilited { Kind::LinesOnTop } else { Kind::Lines });
                r.set_line_width(gpu, if selected && mode == ComponentMode::Object { 1.5 } else { 1.0 });
                r.push(gpu, &push(mvp, world, view, [1.0; 4], 1.5e-3 + retopo, 1.0));
                r.draw(gpu, lb, None, *ln);
            }
            if !planar && !self.back {
                if let Some((pb, pn)) = &entry.points {
                    r.bind(gpu, if self.disp.xray_active && hilited { Kind::PointsOnTop } else { Kind::Points });
                    r.push(gpu, &push(mvp, world, view, [1.0; 4], 2e-3, (5.0 * scale) as f32));
                    r.draw(gpu, pb, None, *pn);
                }
            }
        }
        // The grid reads the depth of solid geometry so it can cross through a surface.
        if self.show_grid && !planar && !self.back {
            r.bind(gpu, Kind::SmoothLines);
            let mut p = push(vp, DMat4::IDENTITY, view, [1.0; 4], 0.0, 1.0);
            p.n2 = [w as f32, h as f32, 0.5, 0.0];
            r.push(gpu, &p);
            let (gb, gn) = self.grid.as_ref().unwrap();
            if *gn > 0 {
                r.draw(gpu, gb, None, *gn);
            }
            if let Some((bb, bn)) = &self.grid_bold {
                p.n2[2] = 1.0;
                r.push(gpu, &p);
                r.draw(gpu, bb, None, *bn);
            }
        }
        // See-through materials last, the furthest first, reading the depth the solid ones wrote.
        transparent.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        for c in transparent.iter().filter(|_| !planar && !self.back) {
            let Some(entry) = self.meshes.get(&c.shape) else { continue };
            let (Some((vb, _, _)), Some(ib), Some(uvs)) = (&entry.tris, entry.batch_ib.as_ref(), entry.uvs.as_ref()) else { continue };
            let Some(b) = entry.batches.get(c.batch).copied() else { continue };
            let green = no_material();
            let m = self.mats.materials.get(&b.group).or(self.mats.fallback.as_ref()).unwrap_or(&green);
            let mut pc = push(vp * c.world, c.world, view, [m.color[0], m.color[1], m.color[2], 1.0], c.retopo, 1.0);
            pc.n2[3] = self.disp.flat as i32 as f32;
            let sets = self.mats.sets.get(&b.group);
            let (mut mp, _) = material_push(pc, m, sets);
            stamp_lights(&self.lit, &mut mp, c.shape, b.group);
            let white = crate::textures::with_cache(gpu, |c| c.white(gpu));
            let kind = if self.disp.backface { Kind::MaterialBlendCull } else { Kind::MaterialBlend };
            let lights = self.lit.as_ref().map_or(vk::DescriptorSet::null(), |l| l.set);
            if realtime && r.realtime {
                let env = self.lit.as_ref().and_then(|l| l.env_set).unwrap_or(white);
                let shadow = self.rt_shadow_set.unwrap_or(white);
                let rp = realtime_push(&mp, m, self.lit.as_ref().map_or(0.0, |l| l.index.get(&b.group).copied().unwrap_or(0) as f32));
                r.bind_realtime(gpu, kind, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), lights, env, shadow);
                r.push_realtime(gpu, &rp);
            } else {
                r.bind_material(gpu, kind, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), lights);
                r.push_material(gpu, &mp);
            }
            r.draw_material(gpu, vb, uvs, ib, b.first, b.count);
        }
        if planar || self.back {
            return;
        }
        crate::usd_view::draw(gpu, r, s, vp, view);
        self.draw_image_planes(gpu, r, s);
        self.draw_ghosts(gpu, r, s);
        self.draw_poly_display(gpu, r, s, items);
        self.draw_rig(gpu, r, s, scale);
        self.draw_strands(gpu, r, s);
        self.draw_hover(gpu, r, items, scale);
        self.draw_manip(gpu, r, s, scale);
        self.draw_node_lines(gpu, r);
        self.draw_brush(gpu, r, s);
        self.draw_curve_tool(gpu, r, scale);
        self.draw_tool_overlay(gpu, r, scale);
    }

    /// Ghosts: each ghosted object's meshes at its ghost frames, see-through in the ghost colours. Worked out again only
    /// when the scene has changed since.
    fn draw_ghosts(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session) {
        thread_local! {
            /// The scene's version before and after the ghosts were worked out (evaluating at other times moves it), and them.
            static GHOSTS: std::cell::RefCell<(u64, u64, Vec<aztek_scene::anim_extra::Ghost>)> = const { std::cell::RefCell::new((u64::MAX, u64::MAX, Vec::new())) };
        }
        let v = s.graph.version();
        let fresh = GHOSTS.with(|c| {
            let c = c.borrow();
            v == c.0 || v == c.1
        });
        if !fresh {
            let list = aztek_scene::anim_extra::ghosts(&mut s.graph);
            let after = s.graph.version();
            GHOSTS.with(|c| *c.borrow_mut() = (v, after, list));
        }
        let ghosts = GHOSTS.with(|c| c.borrow().2.clone());
        if ghosts.is_empty() {
            return;
        }
        let vp = self.view_proj();
        let view = self.cam.view();
        r.bind(gpu, Kind::Tint);
        for gh in &ghosts {
            let rm = build_render_mesh(&gh.mesh, &RenderMeshOptions { max_uv_sets: 0, ..Default::default() });
            if rm.indices.is_empty() {
                continue;
            }
            let verts: Vec<ShadedVertex> = rm.positions.iter().zip(&rm.normals).map(|(p, n)| ShadedVertex { pos: p.to_array(), nrm: n.to_array() }).collect();
            let vb = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts);
            let ib = gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &rm.indices);
            let c = gh.color;
            r.push(gpu, &push(vp * gh.world, gh.world, view, [c[0], c[1], c[2], gh.opacity], 0.0, 1.0));
            r.draw(gpu, &vb, Some((&ib, rm.indices.len() as u32)), 0);
            gpu.drop_buffer(vb);
            gpu.drop_buffer(ib);
        }
    }

    /// Hair and fur: each strand a ribbon turned to the eye, at least a pixel wide, tapering as its radius does, lit as
    /// a fibre (bright where it runs across the view, dim where it runs along it) and darker towards the root.
    fn draw_strands(&mut self, gpu: &mut Gpu, r: &Renderer, s: &Session) {
        let sets = aztek_scene::harold::strands::view_gather(&s.graph);
        if sets.is_empty() {
            return;
        }
        let (verts, idx) = strand_ribbons(&sets, self.cam.eye(), self.cam.ortho().then(|| self.cam.dir_to_eye()), self.pixel_world());
        if idx.is_empty() {
            return;
        }
        let vb = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts);
        let ib = gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &idx);
        r.bind(gpu, Kind::ColorTris);
        r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
        r.draw(gpu, &vb, Some((&ib, idx.len() as u32)), 0);
        gpu.drop_buffer(vb);
        gpu.drop_buffer(ib);
    }

    /// How wide a pixel is in world units at distance 1 from the eye (perspective), or anywhere (orthographic).
    fn pixel_world(&self) -> (f64, bool) {
        if self.cam.ortho() {
            (self.cam.ortho_width / self.rect[2].max(1.0), true)
        } else {
            (2.0 * (self.cam.vfov(self.aspect()) * 0.5).tan() / self.rect[3].max(1.0), false)
        }
    }

    fn draw_node_lines(&mut self, gpu: &mut Gpu, r: &Renderer) {
        if self.node_lines.is_empty() || !self.disp.manipulators {
            return;
        }
        let mut v = Vec::with_capacity(self.node_lines.len() * 2);
        for (a, b, c) in &self.node_lines {
            v.push(LineVertex { pos: a.as_vec3().to_array(), color: c8(*c) });
            v.push(LineVertex { pos: b.as_vec3().to_array(), color: c8(*c) });
        }
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
        r.bind(gpu, Kind::LinesOnTop);
        r.set_line_width(gpu, 2.0);
        r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
        r.draw(gpu, &b, None, v.len() as u32);
        gpu.drop_buffer(b);
    }

    fn draw_tool_overlay(&mut self, gpu: &mut Gpu, r: &Renderer, scale: f64) {
        if !self.tool_lines.is_empty() {
            let mut v = Vec::with_capacity(self.tool_lines.len() * 2);
            for (a, b, c) in &self.tool_lines {
                v.push(LineVertex { pos: a.as_vec3().to_array(), color: c8(*c) });
                v.push(LineVertex { pos: b.as_vec3().to_array(), color: c8(*c) });
            }
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
            r.bind(gpu, Kind::LinesOnTop);
            r.set_line_width(gpu, (1.5 * scale) as f32);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
            r.draw(gpu, &b, None, v.len() as u32);
            gpu.drop_buffer(b);
        }
        if !self.tool_points.is_empty() {
            let v: Vec<LineVertex> = self.tool_points.iter().map(|(p, c)| LineVertex { pos: p.as_vec3().to_array(), color: c8(*c) }).collect();
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
            r.bind(gpu, Kind::Points);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.05, (7.0 * scale) as f32));
            r.draw(gpu, &b, None, v.len() as u32);
            gpu.drop_buffer(b);
        }
    }

    /// Whether the tool has a manipulator.
    fn manipulates(&self) -> bool {
        self.disp.manipulators && matches!(self.tool, Tool::Move | Tool::Rotate | Tool::Scale)
    }

    /// Display > Polygons: border, hard and crease edges, face and vertex normals, as lines over the mesh.
    fn draw_poly_display(&mut self, gpu: &mut Gpu, r: &Renderer, s: &Session, items: &[DrawItem]) {
        let view = self.disp.clone();
        let sel = s.graph.selection().to_vec();
        for it in items {
            // The view's switches, for the selection when limited to it; and the mesh's own (polyOptions).
            let mut d = view.clone();
            if view.limit_to_selected && !sel.contains(&it.xf) && !sel.contains(&it.shape) {
                d.border_edges = false;
                d.face_normals = false;
                d.vertex_normals = false;
                d.hard_edges = false;
                d.crease_edges = false;
            }
            if let Some(n) = s.graph.node(it.shape) {
                use aztek_scene::mesh_nodes::MeshNode;
                d.border_edges |= MeshNode::display_switch(n, "displayBorders");
                d.texture_borders = MeshNode::display_switch(n, "displayMapBorders");
                d.face_centers = MeshNode::display_switch(n, "displayCenter");
                d.triangles = MeshNode::display_switch(n, "displayTriangles");
                d.vertices = MeshNode::display_switch(n, "displayVertices");
            }
            if !(d.border_edges || d.face_normals || d.vertex_normals || d.hard_edges || d.crease_edges || d.texture_borders || d.face_centers || d.triangles || d.vertices) {
                continue;
            }
            let m = &it.mesh;
            let mut wide = Vec::new();
            let mut thin = Vec::new();
            let mut dots = Vec::new();
            let uvs = m.corner_attrs().get::<[f32; 2]>(&aztek_geo::attribute::names::uv(0));
            for e in 0..m.edge_capacity() {
                if !m.edge_valid(e) {
                    continue;
                }
                let mut faces = 0;
                m.for_each_edge_corner(e, |_| faces += 1);
                let is_hard = edge_flag(m, aztek_geo::attribute::names::HARD, e);
                let is_crease = edge_flag(m, aztek_geo::attribute::names::CREASE, e);
                let c = if d.border_edges && faces == 1 {
                    Some([240, 240, 90, 255])
                } else if d.texture_borders && uv_border(m, uvs, e, faces) {
                    Some([240, 240, 240, 255])
                } else if d.crease_edges && is_crease {
                    Some([255, 100, 160, 255])
                } else if d.hard_edges && is_hard {
                    Some([20, 20, 20, 255])
                } else {
                    None
                };
                if let Some(c) = c {
                    wide.push(LineVertex { pos: m.position(m.edge_point(e, 0)).as_vec3().to_array(), color: c8(c) });
                    wide.push(LineVertex { pos: m.position(m.edge_point(e, 1)).as_vec3().to_array(), color: c8(c) });
                }
            }
            if d.face_normals {
                for f in 0..m.face_capacity() {
                    if !m.face_valid(f) {
                        continue;
                    }
                    let pts = m.face_points(f);
                    let c = pts.iter().map(|&p| m.position(p)).sum::<DVec3>() / pts.len().max(1) as f64;
                    let n = m.face_normal(f);
                    thin.push(LineVertex { pos: c.as_vec3().to_array(), color: c8([40, 230, 230, 255]) });
                    thin.push(LineVertex { pos: (c + n * d.normal_size).as_vec3().to_array(), color: c8([40, 230, 230, 255]) });
                }
            }
            if d.face_centers {
                for f in 0..m.face_capacity() {
                    if m.face_valid(f) {
                        dots.push(LineVertex { pos: m.face_centroid(f).as_vec3().to_array(), color: c8([60, 200, 255, 255]) });
                    }
                }
            }
            if d.vertices {
                for p in 0..m.point_capacity() {
                    if m.point_valid(p) {
                        dots.push(LineVertex { pos: m.position(p).as_vec3().to_array(), color: c8([200, 60, 220, 255]) });
                    }
                }
            }
            if d.triangles {
                // The diagonals the drawing adds: triangle sides that are not edges of the face.
                let opts = aztek_geo::triangulate::TriangulateOptions::default();
                let mut tri = Vec::new();
                for f in 0..m.face_capacity() {
                    if !m.face_valid(f) || m.face_points(f).len() < 4 {
                        continue;
                    }
                    tri.clear();
                    aztek_geo::triangulate::triangulate_face(m, f, &mut tri, &opts);
                    for t in tri.chunks(3) {
                        for k in 0..3 {
                            let (a, b) = (t[k], t[(k + 1) % 3]);
                            if m.corner_next(a) == b || m.corner_next(b) == a {
                                continue;
                            }
                            // Each diagonal belongs to two triangles: draw it once.
                            if a < b {
                                for c in [a, b] {
                                    thin.push(LineVertex { pos: m.position(m.corner_point(c)).as_vec3().to_array(), color: c8([150, 150, 150, 255]) });
                                }
                            }
                        }
                    }
                }
            }
            if d.vertex_normals {
                let mut acc = vec![DVec3::ZERO; m.point_capacity() as usize];
                for f in 0..m.face_capacity() {
                    if m.face_valid(f) {
                        let n = m.face_normal(f);
                        for p in m.face_points(f) {
                            acc[p as usize] += n;
                        }
                    }
                }
                for p in 0..m.point_capacity() {
                    if m.point_valid(p) {
                        let q = m.position(p);
                        thin.push(LineVertex { pos: q.as_vec3().to_array(), color: c8([230, 230, 60, 255]) });
                        thin.push(LineVertex { pos: (q + acc[p as usize].normalize_or_zero() * d.normal_size).as_vec3().to_array(), color: c8([230, 230, 60, 255]) });
                    }
                }
            }
            let mvp = self.view_proj() * it.world;
            for (v, w) in [(wide, 3.0f32), (thin, 1.0f32)] {
                if v.is_empty() {
                    continue;
                }
                let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
                r.bind(gpu, Kind::Lines);
                r.set_line_width(gpu, w);
                r.push(gpu, &push(mvp, it.world, self.cam.view(), [1.0; 4], 2e-3, 1.0));
                r.draw(gpu, &b, None, v.len() as u32);
                gpu.drop_buffer(b);
            }
            if !dots.is_empty() {
                let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &dots);
                r.bind(gpu, Kind::Points);
                r.push(gpu, &push(mvp, it.world, self.cam.view(), [1.0; 4], 2e-3, 5.0));
                r.draw(gpu, &b, None, dots.len() as u32);
                gpu.drop_buffer(b);
            }
        }
    }

    /// Whether the selection masks and display type let a click select this node.
    pub fn pickable(&self, g: &Graph, id: NodeId) -> bool {
        if !ds::pickable(g, id) {
            return false;
        }
        crate::select_cmds::pickable(g, id)
    }

    fn rig_shown(&self, g: &Graph, node: NodeId) -> bool {
        match type_category(g, node) {
            "joint" => self.disp.joints,
            "ikHandle" => self.disp.ik_handles,
            "deformer" => self.disp.deformers,
            "locator" => self.disp.locators,
            "curve" => self.disp.curves,
            "camera" => self.disp.cameras,
            "light" => self.disp.lights,
            _ => true,
        }
    }

    /// Does Isolate Select let this view show the node?
    pub fn node_shown(&self, g: &Graph, id: NodeId) -> bool {
        if !self.iso.active() {
            return true;
        }
        let mut n = id;
        while n != NO_NODE {
            if self.iso.nodes.contains(&n) {
                return true;
            }
            n = g.parent(n);
        }
        false
    }

    /// Items this view shows, from what `gather` found.
    pub fn filter_items(&self, g: &Graph, items: Vec<DrawItem>) -> Vec<DrawItem> {
        let (polys, surfs) = (self.disp.polygons, self.disp.surfaces);
        items
            .into_iter()
            .filter(|it| if surfaces::is_surface(g, it.shape) { surfs } else { polys })
            .filter(|it| self.node_shown(g, it.shape) || self.node_shown(g, it.xf))
            .collect()
    }

    /// A curve tool's points so far, with the curve they'd make; the joint tool's joint being dragged, with its bone.
    fn draw_curve_tool(&mut self, gpu: &mut Gpu, r: &Renderer, scale: f64) {
        if let (Tool::Joint, Some((from, at))) = (self.tool, self.joint_preview) {
            let col = c8([255, 255, 255, 255]);
            let pv = [LineVertex { pos: at.as_vec3().to_array(), color: col }];
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv);
            r.bind(gpu, Kind::Points);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, (8.0 * scale) as f32));
            r.draw(gpu, &b, None, 1);
            gpu.drop_buffer(b);
            if let Some(f) = from {
                let lv = [LineVertex { pos: f.as_vec3().to_array(), color: col }, LineVertex { pos: at.as_vec3().to_array(), color: col }];
                let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lv);
                r.bind(gpu, Kind::Lines);
                r.set_line_width(gpu, (1.5 * scale) as f32);
                r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, 1.0));
                r.draw(gpu, &b, None, 2);
                gpu.drop_buffer(b);
            }
            return;
        }
        if !matches!(self.tool, Tool::CurveCv | Tool::CurveEp) || self.curve_points.is_empty() {
            return;
        }
        let col = c8([255, 255, 255, 255]);
        let line = c8([120, 120, 255, 255]);
        let mut pv: Vec<LineVertex> = self.curve_points.iter().map(|p| LineVertex { pos: p.as_vec3().to_array(), color: col }).collect();
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv);
        r.bind(gpu, Kind::Points);
        r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, (6.0 * scale) as f32));
        r.draw(gpu, &b, None, pv.len() as u32);
        gpu.drop_buffer(b);
        pv.clear();
        if self.curve_points.len() >= 2 {
            let d = self.curve_degree.max(1) as usize;
            let curve = if self.tool == Tool::CurveEp { aztek_geo::nurbs::NurbsCurve::through_edit_points(d, &self.curve_points) } else { aztek_geo::nurbs::NurbsCurve::open(d, self.curve_points.clone()) };
            let pts = curve.tessellate(12);
            for w in pts.windows(2) {
                pv.push(LineVertex { pos: w[0].as_vec3().to_array(), color: line });
                pv.push(LineVertex { pos: w[1].as_vec3().to_array(), color: line });
            }
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv);
            r.bind(gpu, Kind::Lines);
            r.set_line_width(gpu, (1.5 * scale) as f32);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, 1.0));
            r.draw(gpu, &b, None, pv.len() as u32);
            gpu.drop_buffer(b);
        }
    }

    /// Image planes: each picture fitted into its gate (a camera's at its depth, or a free plane's own), unlit, with its
    /// colour gain and alpha gain; Outline draws the frame only, and so does a picture that can't be read.
    fn draw_image_planes(&mut self, gpu: &mut Gpu, r: &Renderer, s: &Session) {
        let frame = aztek_scene::anim::frame(&s.graph);
        let draws = aztek_scene::image_plane::draws(&s.graph, self.look_camera, frame);
        if draws.is_empty() {
            return;
        }
        let (vp, view) = (self.view_proj(), self.cam.view());
        for d in draws {
            let tex = if d.file.is_empty() || d.display_mode == 1 { None } else { crate::textures::with_cache(gpu, |c| c.get(gpu, &d.file, true)) };
            let (iw, ih) = tex.map_or((1, 1), |t| t.size);
            let ga = d.right.length() / d.up.length().max(1e-12);
            let (fx, fy) = aztek_scene::image_plane::fitted(d.fit, ga, iw as f64 / ih.max(1) as f64, d.to_size);
            let (rr, uu) = (d.right * fx, d.up * fy);
            let rot = d.rotate;
            let (cs, sn) = (rot.cos(), rot.sin());
            let (rx, ux) = (rr * cs + uu * sn, uu * cs - rr * sn);
            let corners = [d.center - rx - ux, d.center + rx - ux, d.center + rx + ux, d.center - rx + ux];
            let Some(t) = tex.filter(|_| r.materials) else {
                let col = c8([110, 110, 110, 255]);
                let mut v = Vec::new();
                for k in 0..4 {
                    v.push(LineVertex { pos: corners[k].as_vec3().to_array(), color: col });
                    v.push(LineVertex { pos: corners[(k + 1) % 4].as_vec3().to_array(), color: col });
                }
                let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
                r.bind(gpu, Kind::Lines);
                r.set_line_width(gpu, 1.0);
                r.push(gpu, &push(vp, DMat4::IDENTITY, view, [1.0; 4], 0.0, 1.0));
                r.draw(gpu, &b, None, v.len() as u32);
                gpu.drop_buffer(b);
                continue;
            };
            let n = rx.cross(ux).normalize_or_zero().as_vec3().to_array();
            let verts: Vec<crate::render::ShadedVertex> = corners.iter().map(|c| crate::render::ShadedVertex { pos: c.as_vec3().to_array(), nrm: n }).collect();
            let (u0, u1) = if d.flip.0 { (1.0, 0.0) } else { (0.0, 1.0) };
            let (v0, v1) = if d.flip.1 { (1.0, 0.0) } else { (0.0, 1.0) };
            let uvs: Vec<[f32; 2]> = vec![[u0, v0], [u1, v0], [u1, v1], [u0, v1]];
            let idx: Vec<u32> = vec![0, 1, 2, 0, 2, 3];
            let vb = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &verts);
            let ub = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &uvs);
            let ib = gpu.buffer_with(vk::BufferUsageFlags::INDEX_BUFFER, &idx);
            let with_alpha = matches!(d.display_mode, 3 | 5) && t.has_alpha;
            let m = DrawMaterial { unlit: true, color: d.gain, transparency: (1.0 - d.alpha_gain as f32).clamp(0.0, 1.0), ..no_material() };
            let sets = MatSets { color: Some(t.set), alpha: with_alpha.then_some(t.set), alpha_has_alpha: t.has_alpha };
            let pc = push(vp, DMat4::IDENTITY, view, [d.gain[0], d.gain[1], d.gain[2], 1.0], 0.0, 1.0);
            let (mp, clear) = material_push(pc, &m, Some(&sets));
            let white = crate::textures::with_cache(gpu, |c| c.white(gpu));
            r.bind_material(gpu, if clear { Kind::MaterialBlend } else { Kind::Material }, t.set, sets.alpha.unwrap_or(white), self.lit.as_ref().map_or(vk::DescriptorSet::null(), |l| l.set));
            r.push_material(gpu, &mp);
            r.draw_material(gpu, &vb, &ub, &ib, 0, 6);
            gpu.drop_buffer(vb);
            gpu.drop_buffer(ub);
            gpu.drop_buffer(ib);
        }
    }

    /// The brush's ring on the surface and its normal line, in red as the reference draws it.
    fn draw_brush(&mut self, gpu: &mut Gpu, r: &Renderer, s: &Session) {
        let Some((c, n)) = self.brush else { return };
        let radius = match self.tool {
            Tool::PaintSkin => s.paint_ctx.radius,
            Tool::PaintAttr => s.attr_ctx.paint.radius,
            Tool::Sculpt => s.sculpt_ctx.size,
            Tool::Groom => aztek_scene::xjen::groom::ctx().lock().unwrap().size * self.cam.units_per_px_at(c, self.rect[3], self.aspect()),
            _ => return,
        };
        let (u, w) = n.any_orthonormal_pair();
        let mut v = Vec::new();
        let col = c8([230, 40, 40, 255]);
        let steps = 48;
        for i in 0..steps {
            let a = i as f64 / steps as f64 * std::f64::consts::TAU;
            let b = (i + 1) as f64 / steps as f64 * std::f64::consts::TAU;
            v.push(LineVertex { pos: (c + (u * a.cos() + w * a.sin()) * radius).as_vec3().to_array(), color: col });
            v.push(LineVertex { pos: (c + (u * b.cos() + w * b.sin()) * radius).as_vec3().to_array(), color: col });
        }
        v.push(LineVertex { pos: c.as_vec3().to_array(), color: col });
        v.push(LineVertex { pos: (c + n * radius * 0.5).as_vec3().to_array(), color: col });
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
        r.bind(gpu, Kind::LinesOnTop);
        r.set_line_width(gpu, 1.5);
        r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
        r.draw(gpu, &b, None, v.len() as u32);
        gpu.drop_buffer(b);
    }

    /// The selected skinned mesh under a pixel, for the brush (any selected mesh when sculpting): (item, world point, world
    /// normal).
    pub fn brush_hit(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Option<(usize, DVec3, DVec3)> {
        let g = &s.graph;
        let sel = g.selection();
        let any = self.tool == Tool::Sculpt;
        // Grooming: the meshes the descriptions selected (or any, with none selected) grow on.
        let groomed: Vec<aztek_scene::plug::NodeId> = if self.tool == Tool::Groom { aztek_scene::xjen::brush_meshes(g) } else { Vec::new() };
        let picked: Vec<usize> = (0..items.len()).filter(|&i| if self.tool == Tool::Groom { groomed.contains(&items[i].shape) } else { (sel.contains(&items[i].xf) || sel.contains(&items[i].shape)) && (any || if self.tool == Tool::PaintAttr { aztek_scene::deform_paint::current(s, items[i].shape).is_some() } else { aztek_scene::skin::skin_of(g, items[i].shape).is_some() }) }).collect();
        let list: Vec<DrawItem> = picked
            .iter()
            .map(|&i| DrawItem { shape: items[i].shape, xf: items[i].xf, world: items[i].world, mesh: items[i].mesh.clone(), smooth: 0 })
            .collect();
        let (k, f, wp) = self.raycast(&list, px)?;
        let n = list[k].world.transform_vector3(list[k].mesh.face_normal(f)).normalize_or_zero();
        Some((picked[k], wp, n))
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Picking

    fn bvh_for(&mut self, it: &DrawItem) -> Option<&MeshBvh> {
        if !self.meshes.contains_key(&it.shape) {
            // Not drawn yet (or never, as in a script's view): a tree of its own, kept while the mesh is the same.
            let key = Arc::as_ptr(&it.mesh) as usize;
            let e = self.loose_bvhs.entry(it.shape).or_insert_with(|| (key, MeshBvh::new(&it.mesh)));
            if e.0 != key {
                *e = (key, MeshBvh::new(&it.mesh));
            }
            return Some(&e.1);
        }
        let e = self.meshes.get_mut(&it.shape)?;
        if e.bvh.is_none() {
            e.bvh = Some(MeshBvh::new(&it.mesh));
        }
        e.bvh.as_ref()
    }

    /// The object under the pointer for the right-click marking menu.
    pub fn marking_menu_target(&mut self, g: &mut Graph, px: DVec2) -> Option<NodeId> {
        let items = gather(g);
        if let Some(id) = self.pick_rig(g, px) {
            return Some(id);
        }
        let items = self.filter_items(g, items);
        self.raycast(&items, px).map(|(i, _, _)| items[i].xf)
    }

    /// The nearest mesh hit under a viewport pixel: (item index, face, world point).
    pub fn raycast(&mut self, items: &[DrawItem], px: DVec2) -> Option<(usize, Index, DVec3)> {
        self.raycast_faces(items, px, true)
    }

    /// As raycast; with `backfaces` off a face turned away from the eye is passed through.
    pub fn raycast_faces(&mut self, items: &[DrawItem], px: DVec2, backfaces: bool) -> Option<(usize, Index, DVec3)> {
        let (o, d) = self.cam.ray(px, self.size());
        let mut best: Option<(usize, Index, DVec3, f64)> = None;
        for (i, it) in items.iter().enumerate() {
            let inv = it.world.inverse();
            let lo = inv.transform_point3(o);
            let ld = inv.transform_vector3(d);
            let len = ld.length();
            if len < 1e-12 {
                continue;
            }
            let Some(bvh) = self.bvh_for(it) else { continue };
            if let Some(hit) = bvh.raycast_with(lo, ld / len, f64::MAX, backfaces) {
                let wp = it.world.transform_point3(lo + ld / len * hit.t);
                let dist = (wp - o).length();
                if best.as_ref().is_none_or(|b| dist < b.3) {
                    best = Some((i, hit.face, wp, dist));
                }
            }
        }
        best.map(|b| (b.0, b.1, b.2))
    }

    /// The component under a pixel on the hilited meshes: (item, kind, id). UVs come as map ids, vertex faces as corners;
    /// Multi gives whichever kind is nearest.
    pub fn pick_component(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Option<(usize, ComponentMode, Index)> {
        let g = &s.graph;
        if components::is_multi(g) {
            return self.pick_multi(s, items, px);
        }
        match components::mode(g) {
            ComponentMode::Object => None,
            ComponentMode::Uv => {
                // The UV of the face under the mouse at the nearest point, as the reference picks UVs in a view.
                let (i, _, p) = self.pick_basic(s, items, px, ComponentMode::Vertex)?;
                let m = items[i].mesh.clone();
                let face = self.pick_basic(s, items, px, ComponentMode::Face).filter(|f| f.0 == i).map(|f| f.2);
                let corner = face
                    .and_then(|f| m.face_corners(f).into_iter().find(|&c| m.corner_point(c) == p))
                    .or_else(|| m.point_faces(p).into_iter().flat_map(|f| m.face_corners(f)).find(|&c| m.corner_point(c) == p))?;
                let tp = self.uv_topo(items[i].shape, &m);
                let map = *tp.corner_map.get(corner as usize)?;
                (map != aztek_geo::INVALID).then_some((i, ComponentMode::Uv, map))
            }
            ComponentMode::VertexFace => self.pick_vertex_face(s, items, px),
            mode => self.pick_basic(s, items, px, mode),
        }
    }

    /// The UV topology of a mesh, kept while the mesh is the same.
    pub fn uv_topo(&mut self, shape: NodeId, m: &Arc<Mesh>) -> Arc<aztek_scene::uv_ops::UvTopo> {
        let key = Arc::as_ptr(m) as usize;
        if let Some((k, t)) = self.uv_topos.get(&shape) {
            if *k == key {
                return t.clone();
            }
        }
        let t = Arc::new(aztek_scene::uv_ops::topo(m));
        self.uv_topos.insert(shape, (key, t.clone()));
        t
    }

    /// Multi: over a face, its nearest vertex or edge when the mouse is close enough to it, else the face; off the
    /// surface, the nearest vertex or edge within reach.
    fn pick_multi(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Option<(usize, ComponentMode, Index)> {
        let screen = self.screen();
        if let Some((i, _, f)) = self.pick_basic(s, items, px, ComponentMode::Face) {
            let it = &items[i];
            let m = &it.mesh;
            let proj = |p: Index| screen.project(it.world.transform_point3(m.position(p)));
            let (mut bv, mut be): (Option<(f64, Index)>, Option<(f64, Index)>) = (None, None);
            for c in m.face_corners(f) {
                let p = m.corner_point(c);
                if let Some(q) = proj(p) {
                    let d = (q - px).length();
                    if bv.is_none_or(|b| d < b.0) {
                        bv = Some((d, p));
                    }
                }
                let e = m.corner_edge(c);
                if let (Some(a), Some(b)) = (proj(m.edge_point(e, 0)), proj(m.edge_point(e, 1))) {
                    let d = seg_distance(px, a, b);
                    if be.is_none_or(|b| d < b.0) {
                        be = Some((d, e));
                    }
                }
            }
            if let Some((d, p)) = bv.filter(|b| b.0 <= component_pick::radius(ComponentMode::Vertex)) {
                let _ = d;
                return Some((i, ComponentMode::Vertex, p));
            }
            if let Some((_, e)) = be.filter(|b| b.0 <= component_pick::radius(ComponentMode::Edge)) {
                return Some((i, ComponentMode::Edge, e));
            }
            return Some((i, ComponentMode::Face, f));
        }
        self.pick_basic(s, items, px, ComponentMode::Vertex).or_else(|| self.pick_basic(s, items, px, ComponentMode::Edge))
    }

    /// Where a vertex face is drawn and picked: a little way from its point into its face.
    pub fn vertex_face_point(m: &Mesh, c: Index) -> DVec3 {
        let f = m.corner_face(c);
        let pts = m.face_points(f);
        let centre = pts.iter().map(|&p| m.position(p)).sum::<DVec3>() / pts.len().max(1) as f64;
        let p = m.position(m.corner_point(c));
        p + (centre - p) * 0.25
    }

    /// The vertex face nearest the mouse: over a face, the nearest of its own; off it, the nearest within reach that no
    /// face hides.
    fn pick_vertex_face(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Option<(usize, ComponentMode, Index)> {
        let screen = self.screen();
        if let Some((i, _, f)) = self.pick_basic(s, items, px, ComponentMode::Face) {
            let it = &items[i];
            let m = &it.mesh;
            let best = m
                .face_corners(f)
                .into_iter()
                .filter_map(|c| screen.project(it.world.transform_point3(Self::vertex_face_point(m, c))).map(|q| ((q - px).length(), c)))
                .min_by(|a, b| a.0.total_cmp(&b.0))?;
            return Some((i, ComponentMode::VertexFace, best.1));
        }
        let g = &s.graph;
        let sel = g.selection();
        let reach = component_pick::radius(ComponentMode::Vertex);
        let mut best: Option<(f64, usize, Index)> = None;
        for (i, it) in items.iter().enumerate() {
            if !sel.contains(&it.xf) || surfaces::is_surface(g, it.shape) {
                continue;
            }
            let m = &it.mesh;
            for c in 0..m.corner_capacity() {
                if !m.corner_valid(c) {
                    continue;
                }
                let wp = it.world.transform_point3(Self::vertex_face_point(m, c));
                let Some(q) = screen.project(wp) else { continue };
                let d = (q - px).length();
                if d < reach && best.is_none_or(|b| d < b.0) {
                    let one = [DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 }];
                    if self.visible(&one, wp, &screen) {
                        best = Some((d, i, c));
                    }
                }
            }
        }
        best.map(|b| (b.1, ComponentMode::VertexFace, b.2))
    }

    /// The vertex, edge or face under a pixel on the hilited meshes.
    fn pick_basic(&mut self, s: &Session, items: &[DrawItem], px: DVec2, mode: ComponentMode) -> Option<(usize, ComponentMode, Index)> {
        let g = &s.graph;
        let sel = g.selection();
        let hil: Vec<usize> = (0..items.len()).filter(|&i| sel.contains(&items[i].xf) && !surfaces::is_surface(g, items[i].shape)).collect();
        if mode == ComponentMode::Face {
            let hit_items: Vec<DrawItem> = hil
                .iter()
                .map(|&i| DrawItem { shape: items[i].shape, xf: items[i].xf, world: items[i].world, mesh: items[i].mesh.clone(), smooth: 0 })
                .collect();
            return self.raycast(&hit_items, px).map(|(i, f, _)| (hil[i], mode, f));
        }
        let screen = self.screen();
        let hil_items: Vec<DrawItem> = hil
            .iter()
            .map(|&i| DrawItem { shape: items[i].shape, xf: items[i].xf, world: items[i].world, mesh: items[i].mesh.clone(), smooth: 0 })
            .collect();
        // Over a face: the nearest of its own vertices or edges, however far, as the reference pre-selects.
        if let Some((k, f, _)) = self.raycast(&hil_items, px) {
            let it = &hil_items[k];
            let m = &it.mesh;
            let proj = |p: Index| screen.project(it.world.transform_point3(m.position(p)));
            let mut best: Option<(f64, Index)> = None;
            for c in m.face_corners(f) {
                let (d, id) = if mode == ComponentMode::Vertex {
                    let p = m.corner_point(c);
                    let Some(q) = proj(p) else { continue };
                    ((q - px).length(), p)
                } else {
                    let e = m.corner_edge(c);
                    let (Some(a), Some(b)) = (proj(m.edge_point(e, 0)), proj(m.edge_point(e, 1))) else { continue };
                    (seg_distance(px, a, b), e)
                };
                if best.is_none_or(|b| d < b.0) {
                    best = Some((d, id));
                }
            }
            if let Some((_, id)) = best {
                return Some((hil[k], mode, id));
            }
        }
        // Off the surface: the nearest within reach that no face hides; with Highlight Backfaces on
        // (selectPref -preSelectBackfacing), hidden ones too.
        let backfacing = crate::ui_cmds::select_pref("preSelectBackfacing", false);
        let mut best: Option<(f64, usize, Index)> = None;
        for (k, &i) in hil.iter().enumerate() {
            let it = &items[i];
            for c in component_pick::nearby(&it.mesh, &it.world, mode, &screen, px, component_pick::radius(mode)) {
                let local = if mode == ComponentMode::Vertex {
                    it.mesh.position(c.id)
                } else {
                    (it.mesh.position(it.mesh.edge_point(c.id, 0)) + it.mesh.position(it.mesh.edge_point(c.id, 1))) * 0.5
                };
                let wp = it.world.transform_point3(local);
                if !backfacing && !self.visible(&hil_items[k..=k], wp, &screen) {
                    continue;
                }
                if best.is_none_or(|b| c.px < b.0) {
                    best = Some((c.px, i, c.id));
                }
                break;
            }
        }
        best.map(|b| (b.1, mode, b.2))
    }

    /// Whether a world point is in front of every surface of the items along its line of sight.
    pub fn visible(&mut self, items: &[DrawItem], wp: DVec3, screen: &Screen) -> bool {
        let Some(q) = screen.project(wp) else { return false };
        let (o, _) = self.cam.ray(q, self.size());
        let dist = (wp - o).length();
        match self.raycast(items, q) {
            Some((_, _, hit)) => (hit - o).length() >= dist - dist.max(1.0) * 1e-3,
            None => true,
        }
    }

    /// The spacing of the grid lines the view shows: subdivisions when they're drawn, else the grid lines; an
    /// orthographic view zoomed out shows coarser ones.
    pub fn snap_step(&self) -> f64 {
        let div = self.grid_style.divisions.max(1) as f64;
        match self.ortho_grid() {
            Some(b) if b.coarse || b.subdivisions => b.step,
            Some(b) => b.step * div,
            None if self.grid_style.division_lines => self.grid_spacing,
            None => self.grid_spacing * div,
        }
    }

    /// Where the cursor's ray meets the perspective grid's plane, in front of the camera.
    pub fn grid_point(&self, px: DVec2) -> Option<DVec3> {
        let (o, d) = self.cam.ray(px, self.size());
        let n = aztek_scene::world::up();
        let den = n.dot(d);
        if den.abs() < 1e-9 {
            return None;
        }
        let t = -n.dot(o) / den;
        (t > 0.0).then(|| o + d * t)
    }

    /// Where a click places a joint: on the view plane through the origin in the orthographic views, on the ground
    /// in perspective (or facing the camera at the centre of interest when looking along the ground).
    pub fn place_point(&self, px: DVec2) -> DVec3 {
        let (o, d) = self.cam.ray(px, self.size());
        let (n, p0) = match self.cam.kind {
            CamKind::Persp => {
                let up = aztek_scene::world::up();
                if d.dot(up).abs() > 0.05 {
                    (up, DVec3::ZERO)
                } else {
                    ((self.cam.eye() - self.cam.coi).normalize(), self.cam.coi)
                }
            }
            _ => ((self.cam.eye() - self.cam.coi).normalize(), DVec3::ZERO),
        };
        let den = n.dot(d);
        if den.abs() < 1e-9 {
            return self.cam.coi;
        }
        let mut t = n.dot(p0 - o) / den;
        // Looking above the horizon the ground is behind the camera: the plane through the centre of interest, facing
        // the view, takes the point instead.
        if t <= 0.0 {
            let n2 = (self.cam.eye() - self.cam.coi).normalize();
            let den2 = n2.dot(d);
            if den2.abs() < 1e-9 {
                return self.cam.coi;
            }
            t = n2.dot(self.cam.coi - o) / den2;
        }
        let mut p = o + d * t;
        if self.snap == 1 {
            // X held: to the grid.
            let step = self.snap_step();
            p = (p / step).round() * step;
        }
        // Snap tiny noise so ortho placements sit exactly in their plane.
        for i in 0..3 {
            if p[i].abs() < 1e-9 {
                p[i] = 0.0;
            }
        }
        p
    }

    /// The point on the curves' segments nearest px on screen, up to the given distance.
    fn nearest_on_curves(&self, segs: &[(DVec3, DVec3)], px: DVec2, max_dist: f64) -> Option<DVec3> {
        let screen = self.screen();
        let mut best: Option<(f64, DVec3)> = None;
        for &(a, b) in segs {
            let Some((pa, pb)) = screen.project_segment(a, b) else { continue };
            let ab = pb - pa;
            let t = if ab.length_squared() < 1e-9 { 0.0 } else { ((px - pa).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
            let dist = (pa + ab * t - px).length();
            if dist < max_dist && best.is_none_or(|x| dist < x.0) {
                best = Some((dist, a.lerp(b, t)));
            }
        }
        best.map(|(_, p)| p)
    }

    /// Where the joint tool puts a joint: with C held, on the curve nearest the cursor (curves from snap_curves), else
    /// as place_point does.
    pub fn place_joint_point(&self, curves: &[(DVec3, DVec3)], px: DVec2) -> DVec3 {
        if self.snap == 3 {
            if let Some(p) = self.nearest_on_curves(curves, px, 40.0) {
                return p;
            }
        }
        self.place_point(px)
    }

    /// The joint, locator or IK handle drawn through the click box around px, the nearest if several, as its transform.
    pub fn pick_rig(&self, g: &Graph, px: DVec2) -> Option<NodeId> {
        let screen = self.screen();
        let k = self.click_box.max(1.0);
        let mut best: Option<(f64, NodeId)> = None;
        for it in rig_items(g) {
            if !self.rig_shown(g, it.node) || !self.node_shown(g, it.node) || !self.pickable(g, it.shape) {
                continue;
            }
            // Inside a ball beats inside a bone, which beats a line nearby.
            if let Some(t) = it.fills.iter().find(|t| fill_hit(&screen, std::slice::from_ref(*t), px)) {
                let d = if g.node(it.node).is_some_and(|n| t[0] == n.data().matrix(DagNode::WORLD_MATRIX).w_axis.truncate()) { -2.0 } else { -1.0 };
                if best.is_none_or(|x| d < x.0) {
                    best = Some((d, it.node));
                }
                continue;
            }
            for (a, b) in &it.segs {
                let (Some(pa), Some(pb)) = (screen.project(*a), screen.project(*b)) else { continue };
                if !seg_hits_box(pa, pb, px, k) {
                    continue;
                }
                let d = seg_distance(px, pa, pb);
                if best.is_none_or(|x| d < x.0) {
                    best = Some((d, it.node));
                }
            }
        }
        best.map(|b| b.1)
    }

    fn draw_rig(&mut self, gpu: &mut Gpu, r: &Renderer, s: &Session, scale: f64) {
        let g = &s.graph;
        let sel = g.selection();
        let mut v = Vec::new();
        let mut joints_v = Vec::new();
        for it in rig_items(g) {
            if !self.rig_shown(g, it.node) || !self.node_shown(g, it.node) {
                continue;
            }
            let c = ds::selected_color(g, it.shape, sel).unwrap_or(it.color);
            let joint = g.node(it.node).is_some_and(|n| n.ty().is_a("joint"));
            let out = if joint && self.disp.xray_joints { &mut joints_v } else { &mut v };
            for (a, b) in &it.segs {
                out.push(LineVertex { pos: a.as_vec3().to_array(), color: c8(c) });
                out.push(LineVertex { pos: b.as_vec3().to_array(), color: c8(c) });
            }
        }
        if !joints_v.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &joints_v);
            r.bind(gpu, Kind::LinesOnTop);
            r.set_line_width(gpu, (1.5 * scale) as f32);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, 1.0));
            r.draw(gpu, &b, None, joints_v.len() as u32);
            gpu.drop_buffer(b);
        }
        if !v.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
            r.bind(gpu, Kind::Lines);
            r.set_line_width(gpu, (1.5 * scale) as f32);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, 1.0));
            r.draw(gpu, &b, None, v.len() as u32);
            gpu.drop_buffer(b);
        }
        // Display > Transform Display: local axes, pivots and selection handles.
        let mut marks = Vec::new();
        for flag in ["localAxis", "rotatePivot", "scalePivot", "selectHandle"] {
            for id in crate::ui_cmds::node_flag_nodes(g, flag) {
                let Some(n) = g.node(id) else { continue };
                let w = n.data().matrix(DagNode::WORLD_MATRIX);
                let o = w.w_axis.truncate();
                let k = self.units_per_px(o) * 30.0;
                let mut line = |a: DVec3, b: DVec3, c: [u8; 4]| {
                    marks.push(LineVertex { pos: a.as_vec3().to_array(), color: c8(c) });
                    marks.push(LineVertex { pos: b.as_vec3().to_array(), color: c8(c) });
                };
                match flag {
                    "localAxis" => {
                        line(o, o + w.x_axis.truncate().normalize_or_zero() * k, [230, 40, 40, 255]);
                        line(o, o + w.y_axis.truncate().normalize_or_zero() * k, [40, 230, 40, 255]);
                        line(o, o + w.z_axis.truncate().normalize_or_zero() * k, [40, 80, 255, 255]);
                    }
                    "selectHandle" => {
                        let t = k * 0.25;
                        line(o - DVec3::X * t, o + DVec3::X * t, [240, 240, 120, 255]);
                        line(o - DVec3::Y * t, o + DVec3::Y * t, [240, 240, 120, 255]);
                    }
                    _ => {
                        use aztek_scene::transform::Transform;
                        let attr = if flag == "rotatePivot" { Transform::ROTATE_PIVOT } else { Transform::SCALE_PIVOT };
                        let p = if n.ty().is_a("transform") { w.transform_point3(n.vec3(attr)) } else { o };
                        let t = k * 0.2;
                        let c = if flag == "rotatePivot" { [120, 220, 240, 255] } else { [240, 170, 90, 255] };
                        line(p - DVec3::X * t, p + DVec3::X * t, c);
                        line(p - DVec3::Y * t, p + DVec3::Y * t, c);
                        line(p - DVec3::Z * t, p + DVec3::Z * t, c);
                    }
                }
            }
        }
        marks.extend(joint_hints(g));
        if !marks.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &marks);
            r.bind(gpu, Kind::LinesOnTop);
            r.set_line_width(gpu, 1.5);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
            r.draw(gpu, &b, None, marks.len() as u32);
            gpu.drop_buffer(b);
        }
        // Surfaces draw isoparms rather than their tessellation's wire: when selected, in wireframe and wire on shaded.
        let mut wires = Vec::new();
        for id in g.nodes() {
            let Some(n) = g.node(id) else { continue };
            if !n.ty().is_a("nurbsSurface") || n.bool(DagNode::INTERMEDIATE_OBJECT) || !visible(g, id) {
                continue;
            }
            let xf = aztek_scene::dag::transform_of(g, id);
            let picked = sel.contains(&xf) || sel.contains(&id);
            let style = ds::style(g, id);
            if style.bounding_box {
                continue;
            }
            let lit = ds::select_state(g, id, sel) != ds::SelectState::None;
            if !lit && style.shaded && !self.wireframe && !self.wire_on_shaded {
                continue;
            }
            let c = if components::mode(g) != ComponentMode::Object && picked {
                UNSEL_EDGE
            } else {
                ds::wire_color(g, id, sel, "nurbsSurface", WIRE)
            };
            for (a, b) in surfaces::wire_lines(n) {
                wires.push(LineVertex { pos: a.as_vec3().to_array(), color: c8(c) });
                wires.push(LineVertex { pos: b.as_vec3().to_array(), color: c8(c) });
            }
        }
        if !wires.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &wires);
            r.bind(gpu, Kind::Lines);
            r.set_line_width(gpu, (1.5 * scale) as f32);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1.5e-3, 1.0));
            r.draw(gpu, &b, None, wires.len() as u32);
            gpu.drop_buffer(b);
        }
        // CVs of the selected curves and surfaces in a component mode, and of those with dispCV on, with the hull.
        let mode = components::mode(g);
        let mut points = Vec::new();
        let mut hull = Vec::new();
        for (shape, xf, world_cvs, stride) in curve_cvs(g) {
            let hilited = mode == ComponentMode::Vertex && (sel.contains(&xf) || sel.contains(&shape));
            let n = g.node(shape);
            let (disp_cv, disp_hull) = if stride.is_some() { (SurfaceShape::DISP_CV, SurfaceShape::DISP_HULL) } else { (aztek_scene::curves::CurveShape::DISP_CV, aztek_scene::curves::CurveShape::DISP_HULL) };
            let disp = n.is_some_and(|n| n.data().bool(disp_cv));
            let disp_hull = n.is_some_and(|n| n.data().bool(disp_hull));
            if !hilited && !disp {
                continue;
            }
            let picked = components::selected(g, shape, ComponentMode::Vertex);
            // Edit Point and Curve Point: a hilited curve shows its edit points, or the curve points picked, not its CVs.
            let kind = components::curve_kind(g);
            if hilited && stride.is_none() && kind != components::CurveKind::Cv {
                if let Some(c) = n.and_then(aztek_scene::curves::drawn_curve) {
                    if kind == components::CurveKind::EditPoint {
                        for (i, p) in c.edit_points().iter().enumerate() {
                            let on = picked.contains(&aztek_scene::nurbs_components::ep_id(i as u32));
                            points.push(LineVertex { pos: p.as_vec3().to_array(), color: c8(if on { SEL_COMP } else { VERT }) });
                        }
                    }
                    for &id in picked {
                        if kind == components::CurveKind::CurvePoint || !aztek_scene::nurbs_components::is_encoded(id) {
                            if let Some(p) = aztek_scene::nurbs_components::curve_component_point(c, id) {
                                points.push(LineVertex { pos: p.as_vec3().to_array(), color: c8(SEL_COMP) });
                            }
                        }
                    }
                }
                continue;
            }
            for (i, p) in world_cvs.iter().enumerate() {
                let c = if picked.contains(&(i as u32)) { SEL_COMP } else { VERT };
                points.push(LineVertex { pos: p.as_vec3().to_array(), color: c8(c) });
            }
            if disp_hull || hilited {
                let mut seg = |a: DVec3, b: DVec3| {
                    hull.push(LineVertex { pos: a.as_vec3().to_array(), color: c8([120, 60, 120, 255]) });
                    hull.push(LineVertex { pos: b.as_vec3().to_array(), color: c8([120, 60, 120, 255]) });
                };
                match (stride, n.and_then(surfaces::drawn_surface)) {
                    (Some(nv), Some(s)) => {
                        let nu = world_cvs.len() / nv.max(1);
                        let at = |u: usize, v: usize| world_cvs[u * nv + v];
                        for u in 0..nu {
                            let ends = if s.form_v == aztek_geo::nurbs::Form::Periodic { nv } else { nv.saturating_sub(1) };
                            for v in 0..ends {
                                seg(at(u, v), at(u, (v + 1) % nv));
                            }
                        }
                        for v in 0..nv {
                            let ends = if s.form_u == aztek_geo::nurbs::Form::Periodic { nu } else { nu.saturating_sub(1) };
                            for u in 0..ends {
                                seg(at(u, v), at((u + 1) % nu, v));
                            }
                        }
                    }
                    _ => {
                        for w in world_cvs.windows(2) {
                            seg(w[0], w[1]);
                        }
                    }
                }
            }
        }
        if !hull.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &hull);
            r.bind(gpu, Kind::Lines);
            r.set_line_width(gpu, 1.0);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 1e-3, 1.0));
            r.draw(gpu, &b, None, hull.len() as u32);
            gpu.drop_buffer(b);
        }
        if !points.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &points);
            r.bind(gpu, Kind::Points);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 2e-3, (6.0 * scale) as f32));
            r.draw(gpu, &b, None, points.len() as u32);
            gpu.drop_buffer(b);
        }
    }

    /// Edit Point or Curve Point mode: the edit point of a hilited curve nearest a pixel, or the place on the curve
    /// nearest it, as `curve1.ep[2]` or `curve1.u[0.37]`.
    fn pick_curve_component(&self, g: &Graph, px: DVec2) -> Option<String> {
        let screen = self.screen();
        let sel = g.selection();
        let kind = components::curve_kind(g);
        let mut best: Option<(f64, String)> = None;
        for (shape, xf, _, stride) in curve_cvs(g) {
            if stride.is_some() || (!sel.contains(&xf) && !sel.contains(&shape)) {
                continue;
            }
            let Some(c) = g.node(shape).and_then(aztek_scene::curves::drawn_curve) else { continue };
            let path = g.path(xf);
            if kind == components::CurveKind::EditPoint {
                for (i, p) in c.edit_points().iter().enumerate() {
                    let Some(q) = screen.project(*p) else { continue };
                    let d = (q - px).length();
                    if d < 14.0 && best.as_ref().is_none_or(|b| d < b.0) {
                        best = Some((d, format!("{path}.ep[{i}]")));
                    }
                }
            } else {
                // The nearest place on the curve on screen, then its parameter.
                let pts = c.tessellate(24);
                for w in pts.windows(2) {
                    let (Some(a), Some(b)) = (screen.project(w[0]), screen.project(w[1])) else { continue };
                    let ab = b - a;
                    let t = if ab.length_squared() > 0.0 { ((px - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) } else { 0.0 };
                    let d = (a + ab * t - px).length();
                    if d < 10.0 && best.as_ref().is_none_or(|x| d < x.0) {
                        let u = c.closest_param(w[0].lerp(w[1], t));
                        let text = format!("{u:.3}");
                        let text = text.trim_end_matches('0').trim_end_matches('.');
                        best = Some((d, format!("{path}.u[{text}]")));
                    }
                }
            }
        }
        best.map(|b| b.1)
    }

    /// The CV of a hilited curve or surface nearest a pixel.
    fn pick_cv(&self, g: &Graph, px: DVec2) -> Option<(NodeId, usize)> {
        let screen = self.screen();
        let sel = g.selection();
        let mut best: Option<(f64, NodeId, usize)> = None;
        for (shape, xf, cvs, _) in curve_cvs(g) {
            if !sel.contains(&xf) && !sel.contains(&shape) {
                continue;
            }
            for (i, p) in cvs.iter().enumerate() {
                let Some(q) = screen.project(*p) else { continue };
                let d = (q - px).length();
                if d < 14.0 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, shape, i));
                }
            }
        }
        best.map(|b| (b.1, b.2))
    }

    /// What a click at px picks, as select arguments: object names or component strings.
    pub fn pick(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Vec<String> {
        let kept = pickable(s, items);
        let items = &kept[..];
        let g = &s.graph;
        if components::mode(g) == ComponentMode::Object {
            if let Some(id) = self.pick_rig(g, px) {
                return vec![g.path(crate::ui_cmds::scoped(g, id))];
            }
            // The nearest object the masks allow.
            let allowed: Vec<DrawItem> = items
                .iter()
                .filter(|it| self.pickable(g, it.shape))
                .map(|it| DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 })
                .collect();
            let hit = self.raycast(&allowed, px);
            let (o, d) = self.cam.ray(px, self.size());
            if let Some(prim) = crate::usd_view::pick(g, o, d, hit.map(|h| h.2)) {
                return vec![prim];
            }
            return hit.map(|(i, _, _)| vec![g.path(crate::ui_cmds::scoped(g, allowed[i].xf))]).unwrap_or_default();
        }
        if components::mode(g) == ComponentMode::Vertex && components::curve_kind(g) != components::CurveKind::Cv {
            if let Some(hit) = self.pick_curve_component(g, px) {
                return vec![hit];
            }
        }
        if components::mode(g) == ComponentMode::Vertex && components::curve_kind(g) == components::CurveKind::Cv {
            if let Some((shape, i)) = self.pick_cv(g, px) {
                return vec![cv_name(&g.path(aztek_scene::dag::transform_of(g, shape)), i, surfaces::id_stride(g, shape).map(|s| s as usize))];
            }
            if let Some(hit) = self.pick_lattice_point(g, px) {
                return vec![hit];
            }
        }
        let shell = components::is_uv_shell(g);
        match self.pick_component(s, items, px) {
            Some((i, ComponentMode::Uv, id)) if shell => {
                // UV Shell: every UV of the shell the picked one is in.
                let m = items[i].mesh.clone();
                let tp = self.uv_topo(items[i].shape, &m);
                let shells: std::collections::BTreeSet<Index> = tp.maps.get(id as usize).into_iter().flatten().filter_map(|&c| tp.face_shell.get(m.corner_face(c) as usize).copied()).collect();
                let maps = uv_shell_maps(&m, &tp, &shells);
                let mut out = Vec::new();
                push_ranges(&mut out, &g.path(items[i].xf), "map", &maps);
                out
            }
            Some((i, ComponentMode::VertexFace, c)) => {
                let m = &items[i].mesh;
                vec![format!("{}.vtxFace[{}][{}]", g.path(items[i].xf), m.corner_point(c), m.corner_face(c))]
            }
            Some((i, kind, id)) => {
                let p = match kind {
                    ComponentMode::Vertex => "vtx",
                    ComponentMode::Edge => "e",
                    ComponentMode::Uv => "map",
                    _ => "f",
                };
                vec![format!("{}.{p}[{id}]", g.path(items[i].xf))]
            }
            None => {
                // Off the hilited objects, on another one: that object, which takes the selection back to object
                // mode, as the reference's click out of a component mode does (#124).
                let sel = g.selection();
                let lit = components::hilited_meshes(g);
                let others: Vec<DrawItem> = items
                    .iter()
                    .filter(|it| !sel.contains(&it.xf) && !sel.contains(&it.shape) && !lit.contains(&it.shape) && self.pickable(g, it.shape))
                    .map(|it| DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 })
                    .collect();
                if others.is_empty() {
                    return Vec::new();
                }
                // Only when nothing hilited is nearer along the ray.
                let all: Vec<DrawItem> = items.iter().map(|it| DrawItem { shape: it.shape, xf: it.xf, world: it.world, mesh: it.mesh.clone(), smooth: 0 }).collect();
                let eye = self.cam.eye();
                match (self.raycast(&others, px), self.raycast(&all, px)) {
                    (Some((i, _, p)), Some((_, _, q))) if (p - eye).length() <= (q - eye).length() + 1e-6 => vec![g.path(crate::ui_cmds::scoped(g, others[i].xf))],
                    _ => Vec::new(),
                }
            }
        }
    }

    /// The control point of a selected lattice nearest px, within a few pixels.
    fn pick_lattice_point(&self, g: &Graph, px: DVec2) -> Option<String> {
        let screen = self.screen();
        let mut best: Option<(f64, String)> = None;
        for (xf, shape, dims, pts) in selected_lattices(g) {
            for (i, p) in pts.iter().enumerate() {
                let Some(q) = screen.project(*p) else { continue };
                let d = (q - px).length();
                if d < 10.0 && best.as_ref().is_none_or(|b| d < b.0) {
                    let name = components::grid_strings(&g.path(xf), dims, &[i as u32], true);
                    best = name.into_iter().next().map(|n| (d, n));
                }
            }
            let _ = shape;
        }
        best.map(|b| b.1)
    }

    /// Draws the pre-selection highlight: the component under the cursor in red.
    fn draw_hover(&mut self, gpu: &mut Gpu, r: &Renderer, items: &[DrawItem], scale: f64) {
        let Some((shape, kind, id)) = self.hover_comp else { return };
        let Some(it) = items.iter().find(|i| i.shape == shape) else { return };
        let m = &it.mesh;
        let c = c8([255, 60, 60, 255]);
        let pos = |p: Index| m.position(p).as_vec3().to_array();
        let mut v = Vec::new();
        match kind {
            ComponentMode::Vertex if m.point_valid(id) => v.push(LineVertex { pos: pos(id), color: c }),
            ComponentMode::Edge if m.edge_valid(id) => {
                v.push(LineVertex { pos: pos(m.edge_point(id, 0)), color: c });
                v.push(LineVertex { pos: pos(m.edge_point(id, 1)), color: c });
            }
            ComponentMode::Face if m.face_valid(id) => {
                let pts = m.face_points(id);
                for k in 0..pts.len() {
                    v.push(LineVertex { pos: pos(pts[k]), color: c });
                    v.push(LineVertex { pos: pos(pts[(k + 1) % pts.len()]), color: c });
                }
            }
            ComponentMode::Uv => {
                // A UV shows at its point.
                let tp = aztek_scene::uv_ops::topo(m);
                let Some(&corner) = tp.maps.get(id as usize).and_then(|c| c.first()) else { return };
                v.push(LineVertex { pos: pos(m.corner_point(corner)), color: c });
            }
            ComponentMode::VertexFace if m.corner_valid(id) => v.push(LineVertex { pos: Self::vertex_face_point(m, id).as_vec3().to_array(), color: c }),
            _ => return,
        }
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
        let mvp = self.view_proj() * it.world;
        if matches!(kind, ComponentMode::Vertex | ComponentMode::Uv | ComponentMode::VertexFace) {
            r.bind(gpu, Kind::Points);
            r.push(gpu, &push(mvp, it.world, self.cam.view(), [1.0; 4], 2.5e-3, (7.0 * scale) as f32));
        } else {
            r.bind(gpu, Kind::Lines);
            r.set_line_width(gpu, (2.0 * scale) as f32);
            r.push(gpu, &push(mvp, it.world, self.cam.view(), [1.0; 4], 2.5e-3, 1.0));
        }
        r.draw(gpu, &b, None, v.len() as u32);
        gpu.drop_buffer(b);
    }

    /// Everything inside a viewport-pixel rectangle.
    pub fn pick_rect(&self, s: &Session, items: &[DrawItem], a: DVec2, b: DVec2) -> Vec<String> {
        let (lo, hi) = (a.min(b), a.max(b));
        let corners = [lo, DVec2::new(hi.x, lo.y), hi, DVec2::new(lo.x, hi.y)];
        let inside = |p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y;
        let crosses = |p: DVec2, q: DVec2| inside(p) || inside(q) || (0..4).any(|i| segments_cross(p, q, corners[i], corners[(i + 1) % 4]));
        self.pick_region(s, items, &inside, &crosses, (lo + hi) * 0.5)
    }

    /// Everything inside a free-form outline, in pixels.
    pub fn pick_lasso(&self, s: &Session, items: &[DrawItem], outline: &[DVec2]) -> Vec<String> {
        let inside = |p: DVec2| {
            let mut odd = false;
            let n = outline.len();
            for i in 0..n {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                    odd = !odd;
                }
            }
            odd
        };
        let n = outline.len();
        let crosses = |p: DVec2, q: DVec2| inside(p) || inside(q) || (0..n).any(|i| segments_cross(p, q, outline[i], outline[(i + 1) % n]));
        let probe = outline.first().copied().unwrap_or_default();
        self.pick_region(s, items, &inside, &crosses, probe)
    }

    /// Everything whose points the test takes in.
    pub fn pick_inside(&self, s: &Session, items: &[DrawItem], inside: &dyn Fn(DVec2) -> bool) -> Vec<String> {
        self.pick_region(s, items, inside, &|p, q| inside(p) || inside(q), DVec2::splat(f64::NAN))
    }

    /// Everything a drawn region takes in. Components count by their points (edges and faces by their centres); whole
    /// objects count when any part of them is in the region, as the reference's: a point inside, an edge crossing its
    /// outline, or a face lying over it (`probe` is a point of the region).
    pub fn pick_region(&self, s: &Session, items: &[DrawItem], inside: &dyn Fn(DVec2) -> bool, crosses: &dyn Fn(DVec2, DVec2) -> bool, probe: DVec2) -> Vec<String> {
        let kept = pickable(s, items);
        let items = &kept[..];
        let g = &s.graph;
        let mode = components::mode(g);
        let multi = components::is_multi(g);
        let shell = components::is_uv_shell(g);
        let screen = self.screen();
        let mut out = Vec::new();
        // Whole objects the region takes: what was hit, and what that selects.
        let mut picked: Vec<(NodeId, NodeId)> = Vec::new();
        if mode == ComponentMode::Vertex {
            for (xf, _, dims, pts) in selected_lattices(g) {
                let ids: Vec<u32> = (0..pts.len() as u32).filter(|&i| screen.project(pts[i as usize]).is_some_and(inside)).collect();
                if !ids.is_empty() {
                    out.extend(components::grid_strings(&g.path(xf), dims, &ids, false));
                }
            }
        }
        if mode == ComponentMode::Object {
            for it in rig_items(g) {
                if !self.rig_shown(g, it.node) || !self.node_shown(g, it.node) || !self.pickable(g, it.shape) {
                    continue;
                }
                if it.segs.iter().any(|(a, b)| match (screen.project(*a), screen.project(*b)) {
                    (Some(p), Some(q)) => crosses(p, q),
                    (Some(p), None) | (None, Some(p)) => inside(p),
                    _ => false,
                }) || (probe.is_finite() && fill_hit(&screen, &it.fills, probe))
                {
                    picked.push((it.node, crate::ui_cmds::scoped(g, it.node)));
                }
            }
        }
        if mode == ComponentMode::Vertex {
            let kind = components::curve_kind(g);
            for (shape, xf, cvs, stride) in curve_cvs(g) {
                if !g.selection().contains(&xf) && !g.selection().contains(&shape) {
                    continue;
                }
                // Edit points inside the box; curve points are picked by clicking only, as in the reference.
                if stride.is_none() && kind != components::CurveKind::Cv {
                    if kind == components::CurveKind::EditPoint {
                        if let Some(c) = g.node(shape).and_then(aztek_scene::curves::drawn_curve) {
                            for (i, p) in c.edit_points().iter().enumerate() {
                                if screen.project(*p).is_some_and(inside) {
                                    out.push(format!("{}.ep[{i}]", g.path(xf)));
                                }
                            }
                        }
                    }
                    continue;
                }
                for (i, p) in cvs.iter().enumerate() {
                    if screen.project(*p).is_some_and(inside) {
                        out.push(cv_name(&g.path(xf), i, stride));
                    }
                }
            }
        }
        // Camera Based Selection (selectPref -useDepth): components hidden behind a surface stay out.
        // With autoUseDepth, it is on in shaded views and off in wireframe whatever useDepth says.
        let use_depth = if crate::ui_cmds::select_pref("autoUseDepth", false) { !self.wireframe } else { crate::ui_cmds::select_pref("useDepth", false) };
        let depth = use_depth && !self.disp.xray && !self.wireframe;
        let trees: Vec<MeshBvh> = if depth { items.iter().map(|it| MeshBvh::new(&it.mesh)).collect() } else { Vec::new() };
        let size = self.size();
        let seen = |wp: DVec3| -> bool {
            if !depth {
                return true;
            }
            let Some(q) = screen.project(wp) else { return false };
            let (o, d) = self.cam.ray(q, size);
            let dist = (wp - o).length();
            let near = dist - dist.max(1.0) * 1e-3;
            !items.iter().zip(&trees).any(|(it, bvh)| {
                let inv = it.world.inverse();
                let (lo, ld) = (inv.transform_point3(o), inv.transform_vector3(d));
                let len = ld.length();
                len > 1e-12
                    && bvh.raycast_with(lo, ld / len, f64::MAX, true).is_some_and(|h| (it.world.transform_point3(lo + ld / len * h.t) - o).length() < near)
            })
        };
        for it in items {
            let m = &it.mesh;
            let proj = |p: Index| screen.project(it.world.transform_point3(m.position(p)));
            let on = g.selection().contains(&it.xf) && !surfaces::is_surface(g, it.shape);
            // Multi: the vertices, edges and faces it takes, each as its own mode would.
            let kinds: Vec<ComponentMode> = if multi && on { vec![ComponentMode::Vertex, ComponentMode::Edge, ComponentMode::Face] } else { vec![mode] };
            for mode in kinds {
            match mode {
                ComponentMode::Uv | ComponentMode::VertexFace if !on => {}
                ComponentMode::Uv => {
                    // The UVs at the points inside; in UV Shell, the shells they are in.
                    let pts: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.point_valid(p) && proj(p).is_some_and(inside)).collect();
                    let tp = aztek_scene::uv_ops::topo(m);
                    let mut maps = components::convert(m, ComponentMode::Vertex, &pts, ComponentMode::Uv, false);
                    if shell {
                        let shells = maps.iter().flat_map(|&mp| tp.maps.get(mp as usize).into_iter().flatten()).filter_map(|&c| tp.face_shell.get(m.corner_face(c) as usize).copied()).collect();
                        maps = uv_shell_maps(m, &tp, &shells);
                    }
                    push_ranges(&mut out, &g.path(it.xf), "map", &maps);
                }
                ComponentMode::VertexFace => {
                    for c in 0..m.corner_capacity() {
                        if m.corner_valid(c) && screen.project(it.world.transform_point3(Self::vertex_face_point(m, c))).is_some_and(inside) {
                            out.push(format!("{}.vtxFace[{}][{}]", g.path(it.xf), m.corner_point(c), m.corner_face(c)));
                        }
                    }
                }
                ComponentMode::Object => {
                    let touched = || {
                        (0..m.point_capacity()).any(|p| m.point_valid(p) && proj(p).is_some_and(inside))
                            || (0..m.edge_capacity()).any(|e| {
                                m.edge_valid(e)
                                    && match (proj(m.edge_point(e, 0)), proj(m.edge_point(e, 1))) {
                                        (Some(p), Some(q)) => crosses(p, q),
                                        _ => false,
                                    }
                            })
                            || (probe.is_finite()
                                && (0..m.face_capacity()).any(|f| {
                                    m.face_valid(f) && {
                                        let pts: Vec<DVec2> = m.face_points(f).iter().filter_map(|&p| proj(p)).collect();
                                        pts.len() >= 3 && (1..pts.len() - 1).any(|k| in_triangle(probe, pts[0], pts[k], pts[k + 1]))
                                    }
                                }))
                    };
                    if self.pickable(g, it.shape) && touched() {
                        picked.push((it.shape, crate::ui_cmds::scoped(g, it.xf)));
                    }
                }
                _ if !g.selection().contains(&it.xf) || surfaces::is_surface(g, it.shape) => {}
                ComponentMode::Vertex => {
                    let ids: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.point_valid(p) && proj(p).is_some_and(inside) && seen(it.world.transform_point3(m.position(p)))).collect();
                    push_ranges(&mut out, &g.path(it.xf), "vtx", &ids);
                }
                ComponentMode::Edge => {
                    let ids: Vec<Index> = (0..m.edge_capacity())
                        .filter(|&e| {
                            m.edge_valid(e) && {
                                let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
                                let wp = it.world.transform_point3((m.position(a) + m.position(b)) * 0.5);
                                screen.project(wp).is_some_and(inside) && seen(wp)
                            }
                        })
                        .collect();
                    push_ranges(&mut out, &g.path(it.xf), "e", &ids);
                }
                ComponentMode::Face => {
                    let ids: Vec<Index> = (0..m.face_capacity())
                        .filter(|&f| {
                            m.face_valid(f) && {
                                let pts = m.face_points(f);
                                let c = pts.iter().map(|&p| m.position(p)).sum::<DVec3>() / pts.len().max(1) as f64;
                                // A face's centre sits on it: seen from a hair in front.
                                let n = it.world.transform_vector3(m.face_normal(f)).normalize_or_zero();
                                let wp = it.world.transform_point3(c);
                                screen.project(wp).is_some_and(inside) && seen(wp + n * 1e-4 * wp.length().max(1.0))
                            }
                        })
                        .collect();
                    push_ranges(&mut out, &g.path(it.xf), "f", &ids);
                }
            }
            }
        }
        let mut objects = by_priority(g, &picked);
        // selectPref -singleBoxSelection: a marquee over several objects takes one.
        if crate::ui_cmds::select_pref("singleBoxSelection", false) {
            objects.truncate(1);
        }
        out.extend(objects);
        out
    }

    // -----------------------------------------------------------------------------------------------------------------
    // Manipulators

    /// Where the manipulator sits: the lead object's pivot, or the selected components' centre.
    pub fn manip_pivot(&self, s: &mut Session, items: &[DrawItem]) -> Option<DVec3> {
        if !self.manipulates() {
            return None;
        }
        if let Some(p) = crate::ctx_cmds::manip_pivot_override() {
            if !s.graph.selection().is_empty() {
                return Some(p);
            }
        }
        let mode = components::mode(&s.graph);
        // UVs and vertex faces have no handles in a view.
        if matches!(mode, ComponentMode::Uv | ComponentMode::VertexFace) {
            return None;
        }
        if mode == ComponentMode::Object {
            if let Some(p) = crate::usd_view::manip_pivot(s) {
                return Some(p);
            }
            let lead = *s.graph.selection().last()?;
            if !s.graph.node(lead).is_some_and(|n| n.ty().is_a("transform")) {
                return None;
            }
            return Some(aztek_scene::transform_ops::world_pivot(&mut s.graph, lead));
        }
        let g = &s.graph;
        let (mut sum, mut n) = (DVec3::ZERO, 0.0);
        for it in items {
            if !g.selection().contains(&it.xf) || surfaces::is_surface(g, it.shape) {
                continue;
            }
            let ids = components::selected(g, it.shape, mode);
            if ids.is_empty() {
                continue;
            }
            let pts = components::convert(&it.mesh, mode, ids, ComponentMode::Vertex, false);
            for p in pts {
                sum += it.world.transform_point3(it.mesh.position(p));
                n += 1.0;
            }
        }
        if mode == ComponentMode::Vertex {
            for (shape, _, cvs, _) in curve_cvs(g) {
                let curve = g.node(shape).and_then(aztek_scene::curves::drawn_curve);
                for &i in components::selected(g, shape, mode) {
                    let p = if aztek_scene::nurbs_components::is_encoded(i) { curve.and_then(|c| aztek_scene::nurbs_components::curve_component_point(c, i)) } else { cvs.get(i as usize).copied() };
                    if let Some(p) = p {
                        sum += p;
                        n += 1.0;
                    }
                }
            }
        }
        (n > 0.0).then(|| sum / n)
    }

    /// Works out the manipulator's axes for the axis orientation in Tool Settings. The move tool honours every mode; rotate
    /// and scale read them too.
    pub fn update_frame(&mut self, s: &mut Session) {
        let g = &mut s.graph;
        self.frame = DMat3::IDENTITY;
        // An object's pivot has axes of its own (pivotOrient). Edit Pivot shows them whatever the orientation chosen.
        let pivot_of = |g: &mut aztek_scene::graph::Graph| {
            let lead = *g.selection().last()?;
            (components::mode(g) == ComponentMode::Object && g.node(lead).is_some_and(|n| n.ty().is_a("transform") && !n.ty().is_a("joint"))).then_some(lead)
        };
        self.pivot_rings = false;
        if self.pivot_mode && self.manipulates() {
            if let Some(lead) = pivot_of(g) {
                self.pivot_rings = true;
                self.frame = aztek_scene::transform_ops::pivot_frame(g, lead);
                return;
            }
        }
        let channel_parent = channel_space() && self.eff_tool() == Tool::Move && components::mode(g) == ComponentMode::Object;
        let axis_mode = if channel_parent { "parent".to_string() } else { self.axis_mode.clone() };
        if axis_mode == "world" {
            return;
        }
        if axis_mode == "custom" {
            self.frame = self.custom_frame;
            return;
        }
        let Some(&lead) = g.selection().last() else { return };
        let ortho = |m: &DMat4| {
            let x = m.x_axis.truncate().normalize_or_zero();
            let y = m.y_axis.truncate().normalize_or_zero();
            let z = x.cross(y);
            if x == DVec3::ZERO || y == DVec3::ZERO {
                DMat3::IDENTITY
            } else {
                DMat3::from_cols(x, y, z)
            }
        };
        g.evaluate();
        let mode = components::mode(g);
        match axis_mode.as_str() {
            "object" => {
                self.frame = match pivot_of(g) {
                    Some(lead) => aztek_scene::transform_ops::pivot_frame(g, lead),
                    None => ortho(&aztek_scene::rig::world(g, lead)),
                }
            }
            "parent" => {
                let p = g.parent(lead);
                if p != NO_NODE {
                    self.frame = ortho(&aztek_scene::rig::world(g, p));
                }
            }
            // The object's world matrix already holds its rotate axis, as Ra comes before R.
            "rotationAxis" => self.frame = ortho(&aztek_scene::rig::world(g, lead)),
            // Gimbal: the rotate channels' own axes, each turned by the channels applied after it, so they need not be
            // at right angles: the last channel's axis is the parent's (and the joint orient's), the first turns with all.
            "gimbal" => {
                if let Some(f) = gimbal_frame(g, lead) {
                    self.frame = f;
                }
            }
            "liveObject" => {
                if let Some(&live) = s.live_surface.first() {
                    let xf = aztek_scene::dag::transform_of(&s.graph, live);
                    self.frame = ortho(&aztek_scene::rig::world(&mut s.graph, xf));
                }
            }
            "component" | "normal" => {
                if mode == ComponentMode::Object {
                    self.frame = ortho(&aztek_scene::rig::world(g, lead));
                } else {
                    self.frame = aztek_scene::mesh_edit::transform::selection_normal_frame(g);
                }
            }
            _ => {}
        }
    }

    /// World units per pixel at a point.
    fn units_per_px(&self, at: DVec3) -> f64 {
        self.cam.units_per_px_at(at, self.rect[3], self.aspect())
    }

    pub fn handle_len(&self, pivot: DVec3, scale: f64) -> f64 {
        self.units_per_px(pivot) * 100.0 * scale * self.manip_scale
    }

    fn plane_handle_offset(&self) -> f64 {
        // PROVISIONAL: fractions of the axis length estimated from the reference's illustrations.
        if self.eff_tool() == Tool::Scale { 0.76 } else { 0.62 }
    }

    fn plane_handle_visibility(&self, pivot: DVec3) -> [bool; 3] {
        let to_eye = if self.cam.ortho() { self.cam.dir_to_eye() } else { (self.cam.eye() - pivot).normalize_or_zero() };
        // Edge-on planes collapse onto a line and can cover an axis handle.
        std::array::from_fn(|i| {
            let a = self.frame.col((i + 1) % 3);
            let b = self.frame.col((i + 2) % 3);
            a.cross(b).normalize_or_zero().dot(to_eye).abs() > 1e-6
        })
    }

    fn rotation_arc(&self, axis: DVec3, pivot: DVec3, radius: f64) -> impl Iterator<Item = (DVec3, DVec3)> {
        let axis = axis.normalize_or_zero();
        let toward_eye = if self.cam.ortho() { self.cam.dir_to_eye() } else { (self.cam.eye() - pivot).normalize_or_zero() };
        let across = axis.cross(toward_eye);
        // Face-on rings have no back half. Keep them whole when both sides lie in the view plane.
        let (u, w, start, span) = if across.length_squared() < 1e-12 {
            let (u, w) = axis.normalize_or(DVec3::Z).any_orthonormal_pair();
            (u, w, 0.0, std::f64::consts::TAU)
        } else {
            let w = across.normalize();
            (w.cross(axis), w, -std::f64::consts::FRAC_PI_2, std::f64::consts::PI)
        };
        let point = move |k| {
            let angle = start + span * k as f64 / 64.0;
            pivot + (u * angle.cos() + w * angle.sin()) * radius
        };
        let segments = if axis == DVec3::ZERO { 0 } else { 64 };
        (0..segments).map(move |k| (point(k), point(k + 1)))
    }

    fn draw_manip(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session, scale: f64) {
        let items = gather(&mut s.graph);
        let Some(pivot) = self.manip_pivot(s, &items) else { return };
        let radius = self.handle_len(pivot, scale);
        let len = radius as f32;
        let p = pivot.as_vec3();
        let colors = [[230, 40, 40, 255], [40, 230, 40, 255], [40, 80, 255, 255]];
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hover);
        let constraint = self.constraint;
        let remembered = if self.pivot_mode { None } else { active_handle(self.eff_tool()) };
        // Handles lit: the hovered or dragged one, the tool's active one and the set constraint in yellow.
        let tint = |h: Handle, c: [u8; 4]| if active == Some(h) || remembered == Some(h) || constraint == Some(h) { [255, 255, 0, 255] } else { c };
        let mut v = Vec::new();
        // The solid parts: arrowheads, boxes, the centre and the plane squares, drawn under the lines.
        let mut tris: Vec<LineVertex> = Vec::new();
        let mut tri = |a: Vec3, b: Vec3, c: Vec3, col: [u8; 4]| {
            for q in [a, b, c] {
                tris.push(LineVertex { pos: q.to_array(), color: c8(col) });
            }
        };
        let shade = |c: [u8; 4], k: f32| [(c[0] as f32 * k).min(255.0) as u8, (c[1] as f32 * k).min(255.0) as u8, (c[2] as f32 * k).min(255.0) as u8, c[3]];
        let camera_dir = self.cam.dir_to_eye().as_vec3();
        let eye = self.cam.eye().as_vec3();
        let ortho = self.cam.ortho();
        // Solid handles draw over the scene, so hide their back faces here instead of using scene depth.
        let faces_eye = |a: Vec3, b: Vec3, c: Vec3| {
            let to_eye = if ortho { camera_dir } else { eye - a };
            (b - a).cross(c - a).dot(to_eye) > 0.0
        };
        let cube = |at: Vec3, x: Vec3, y: Vec3, z: Vec3, col: [u8; 4], tri: &mut dyn FnMut(Vec3, Vec3, Vec3, [u8; 4])| {
            // Each face a little lighter or darker, so the box reads as one.
            for (n, u, w, k) in [(x, y, z, 1.0), (-x, z, y, 0.8), (y, z, x, 1.15), (-y, x, z, 0.65), (z, x, y, 0.9), (-z, y, x, 0.75)] {
                let c = at + n;
                let q = [c + u + w, c - u + w, c - u - w, c + u - w];
                if faces_eye(q[0], q[1], q[2]) {
                    let col = shade(col, k);
                    tri(q[0], q[1], q[2], col);
                    tri(q[0], q[2], q[3], col);
                }
            }
        };
        let mut line = |a: Vec3, b: Vec3, c: [u8; 4]| {
            v.push(LineVertex { pos: a.to_array(), color: c8(c) });
            v.push(LineVertex { pos: b.to_array(), color: c8(c) });
        };
        self.update_frame(s);
        let axes = [self.frame.x_axis.as_vec3(), self.frame.y_axis.as_vec3(), self.frame.z_axis.as_vec3()];
        let view = self.cam.view();
        let right = Vec3::new(view.x_axis.x as f32, view.y_axis.x as f32, view.z_axis.x as f32);
        let up = Vec3::new(view.x_axis.y as f32, view.y_axis.y as f32, view.z_axis.y as f32);
        // Edit Pivot mode shows on the manipulator: a ring round its centre, so it is plain the pivot is what moves.
        if self.pivot_mode && self.manipulates() {
            let n = 32;
            for k in 0..n {
                let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
                let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
                for r in [len * 0.2, len * 0.24] {
                    line(p + (right * a0.cos() + up * a0.sin()) * r, p + (right * a1.cos() + up * a1.sin()) * r, [255, 215, 60, 255]);
                }
            }
            // Rings outside the arrows turn the pivot's axes.
            if self.pivot_rings {
                let n = 64;
                for (i, ax) in axes.iter().enumerate() {
                    let (u, w) = ax.any_orthonormal_pair();
                    let c = tint(Handle::PivotRing(i), colors[i]);
                    for k in 0..n {
                        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
                        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
                        line(p + (u * a0.cos() + w * a0.sin()) * len * PIVOT_RING, p + (u * a1.cos() + w * a1.sin()) * len * PIVOT_RING, c);
                    }
                }
            }
        }
        match self.eff_tool() {
            Tool::Rotate => {
                let view_color = if active.is_none() && remembered.is_none() {
                    [255, 255, 0, 255]
                } else {
                    tint(Handle::ViewRing, [90, 200, 230, 255])
                };
                let rim = |k| {
                    let angle = std::f32::consts::TAU * k as f32 / 64.0;
                    (right * angle.cos() + up * angle.sin()) * len
                };
                for k in 0..64 {
                    let (a, b) = (rim(k), rim(k + 1));
                    line(p + a * 1.25, p + b * 1.25, view_color);
                    line(p + a, p + b, [70, 70, 70, 255]);
                    if active == Some(Handle::Trackball) {
                        tri(p, p + a, p + b, [90, 90, 90, 96]);
                    }
                }
                for (i, c) in colors.into_iter().enumerate() {
                    let c = tint(Handle::Axis(i), c);
                    for (a, b) in self.rotation_arc(self.frame.col(i), pivot, radius) {
                        line(a.as_vec3(), b.as_vec3(), c);
                    }
                }
            }
            _ => {
                let scaling = self.eff_tool() == Tool::Scale;
                for (i, ax) in axes.iter().enumerate() {
                    let c = tint(Handle::Axis(i), colors[i]);
                    let tip = p + *ax * len;
                    line(p, tip, c);
                    let (u, w) = ax.any_orthonormal_pair();
                    let k = len * 0.08;
                    if scaling {
                        // A solid box at the tip.
                        cube(tip, *ax * k, u * k, w * k, c, &mut tri);
                    } else {
                        // A solid cone for the arrowhead.
                        let base = tip - *ax * len * 0.2;
                        let n = 12;
                        for j in 0..n {
                            let (a0, a1) = (j as f32 / n as f32 * std::f32::consts::TAU, (j + 1) as f32 / n as f32 * std::f32::consts::TAU);
                            let (e0, e1) = (base + (u * a0.cos() + w * a0.sin()) * k, base + (u * a1.cos() + w * a1.sin()) * k);
                            if faces_eye(tip, e0, e1) {
                                tri(tip, e0, e1, shade(c, 0.75 + 0.35 * a0.cos().abs()));
                            }
                            if faces_eye(base, e1, e0) {
                                tri(base, e1, e0, shade(c, 0.6));
                            }
                        }
                    }
                }
                // Plane handles: a small square between each pair of axes, coloured for the axis it locks.
                let offset = self.plane_handle_offset() as f32;
                let (l0, l1) = (len * (offset - 0.08), len * (offset + 0.08));
                for (i, visible) in self.plane_handle_visibility(pivot).into_iter().enumerate() {
                    if !visible {
                        continue;
                    }
                    let (a, b) = (axes[(i + 1) % 3], axes[(i + 2) % 3]);
                    let c = tint(Handle::Plane(i), colors[i]);
                    let q = [p + a * l0 + b * l0, p + a * l1 + b * l0, p + a * l1 + b * l1, p + a * l0 + b * l1];
                    for j in 0..4 {
                        line(q[j], q[(j + 1) % 4], c);
                    }
                    // Filled, see-through, so the square is a target and not a thin outline.
                    let fill = [c[0], c[1], c[2], 90];
                    tri(q[0], q[1], q[2], fill);
                    tri(q[0], q[2], q[3], fill);
                }
                let c = tint(Handle::Center, [230, 230, 120, 255]);
                let k = len * 0.1;
                if scaling {
                    // Scale's centre: a solid box, as in the reference.
                    let fwd = right.cross(up);
                    cube(p, right * k, up * k, fwd * k, c, &mut tri);
                } else {
                    // Move's centre: a solid square facing the view.
                    let corners = [p + (right + up) * k, p + (up - right) * k, p - (right + up) * k, p + (right - up) * k];
                    for i in 0..4 {
                        line(corners[i], corners[(i + 1) % 4], c);
                    }
                    let fill = [c[0], c[1], c[2], 200];
                    tri(corners[0], corners[1], corners[2], fill);
                    tri(corners[0], corners[2], corners[3], fill);
                }
            }
        }
        if !tris.is_empty() {
            let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &tris);
            r.bind(gpu, Kind::ColorTrisOnTop);
            r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
            r.draw(gpu, &b, None, tris.len() as u32);
            gpu.drop_buffer(b);
        }
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
        r.bind(gpu, Kind::LinesOnTop);
        r.set_line_width(gpu, 2.0);
        r.push(gpu, &push(self.view_proj(), DMat4::IDENTITY, self.cam.view(), [1.0; 4], 0.0, 1.0));
        r.draw(gpu, &b, None, v.len() as u32);
        gpu.drop_buffer(b);
    }

    /// The manipulator handle under a pixel.
    pub fn manip_hit(&self, pivot: DVec3, px: DVec2, scale: f64) -> Option<Handle> {
        // selectPref -manipClickBoxSize: how near an axis or ring a click takes it.
        let reach = crate::ui_cmds::pref_f64("manipClickBoxSize", 8.0).max(1.0);
        let screen = self.screen();
        let len = self.handle_len(pivot, scale);
        let c = screen.project(pivot)?;
        let rotate = self.eff_tool() == Tool::Rotate;
        // The centre square takes the whole of its inside and a few pixels round it, not only its outline, so it is
        // caught without aiming.
        if !rotate {
            let view = self.cam.view();
            let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
            let half = (screen.project(pivot + right * len * 0.1).unwrap_or(c) - c).length();
            if (px - c).abs().max_element() < half.max(10.0 * scale) + 5.0 * scale {
                return Some(Handle::Center);
            }
        }
        if rotate {
            // The view ring, on the outside.
            let radius_px = {
                let view = self.cam.view();
                let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
                (screen.project(pivot + right * len).unwrap_or(c) - c).length()
            };
            let d = (px - c).length();
            if (d - radius_px * 1.25).abs() < 7.0 * scale {
                return Some(Handle::ViewRing);
            }
        } else {
            let offset = len * self.plane_handle_offset();
            for (i, visible) in self.plane_handle_visibility(pivot).into_iter().enumerate() {
                if !visible {
                    continue;
                }
                let (a, b) = (self.frame.col((i + 1) % 3), self.frame.col((i + 2) % 3));
                let mid = pivot + (a + b) * offset;
                if let Some(q) = screen.project(mid) {
                    if (px - q).length() < 9.0 * scale {
                        return Some(Handle::Plane(i));
                    }
                }
            }
        }
        let mut best: Option<(f64, Handle)> = None;
        for i in 0..3 {
            let ax = self.frame.col(i);
            let d = if rotate {
                self.rotation_arc(ax, pivot, len)
                    .filter_map(|(a, b)| screen.project_segment(a, b))
                    .map(|(a, b)| seg_dist(px, a, b))
                    .fold(f64::MAX, f64::min)
            } else {
                let Some((pa, pb)) = screen.project_segment(pivot, pivot + ax * len) else { continue };
                seg_dist(px, pa, pb)
            };
            if d < reach * scale && best.is_none_or(|b| d < b.0) {
                best = Some((d, Handle::Axis(i)));
            }
        }
        if let Some((_, h)) = best {
            return Some(h);
        }
        if self.pivot_rings && !rotate {
            let r = len * PIVOT_RING as f64;
            let mut ring: Option<(f64, usize)> = None;
            for i in 0..3 {
                let (u, w) = self.frame.col(i).any_orthonormal_pair();
                let d = (0..48)
                    .filter_map(|k| {
                        let a = k as f64 / 48.0 * std::f64::consts::TAU;
                        let b = (k + 1) as f64 / 48.0 * std::f64::consts::TAU;
                        let (pa, pb) = screen.project_segment(pivot + (u * a.cos() + w * a.sin()) * r, pivot + (u * b.cos() + w * b.sin()) * r)?;
                        Some(seg_dist(px, pa, pb))
                    })
                    .fold(f64::MAX, f64::min);
                if d < reach * scale && ring.is_none_or(|b| d < b.0) {
                    ring = Some((d, i));
                }
            }
            if let Some((_, i)) = ring {
                return Some(Handle::PivotRing(i));
            }
        }
        if rotate {
            let view = self.cam.view();
            let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
            let radius_px = (screen.project(pivot + right * len).unwrap_or(c) - c).length();
            if (px - c).length() < radius_px {
                return Some(Handle::Trackball);
            }
        }
        None
    }

    /// What a Ctrl-click on a handle does to the constraint: an axis locks it (moves stay in the plane across it), a plane
    /// handle sets that plane, the centre goes back to the view plane.
    pub fn set_constraint(&mut self, h: Handle) {
        self.constraint = match h {
            Handle::Axis(i) => Some(Handle::Plane(i)),
            Handle::Plane(i) => Some(Handle::Plane(i)),
            _ => None,
        };
    }

    /// A move's offset with snapping on: the pivot lands on the grid, a vertex or curve near the cursor, or the live surface;
    /// along an axis handle only that axis moves, in a plane handle only that plane.
    fn snapped(&self, d: &Drag, world: DVec3, handle: Handle, px: DVec2) -> DVec3 {
        // A live surface snaps whatever moves onto it with no other snap on, as in the reference.
        let snap = if self.pivot_mode && self.snap_pivot && self.snap == 0 { 2 } else if self.snap == 0 && self.live_auto { 5 } else { self.snap };
        let target = match snap {
            1 => {
                // The visible grid's intersections, and on past its edge at the same spacing (#149). The centre
                // handle in a perspective view takes the grid point under the cursor, on the grid's plane.
                let s = self.snap_step();
                let t = d.pivot + world;
                match (handle, self.cam.orthographic, self.grid_point(px)) {
                    (Handle::Center, false, Some(p)) => (p / s).round() * s,
                    (_, true, _) => {
                        // In the view's plane: the depth stays where it was.
                        let r = (t / s).round() * s;
                        let n = (self.cam.eye() - self.cam.coi).normalize_or_zero();
                        r + n * (t - r).dot(n)
                    }
                    _ => (t / s).round() * s,
                }
            }
            2 => {
                let screen = self.screen();
                let mut best: Option<(f64, DVec3)> = None;
                for &p in &d.snap_points {
                    if let Some(q) = screen.project(p) {
                        let dist = (q - px).length();
                        // The nearest on screen, however far: a held V always lands on a point, as the reference's
                        // does (#149).
                        if best.is_none_or(|b| dist < b.0) {
                            best = Some((dist, p));
                        }
                    }
                }
                match best {
                    Some((_, p)) => p,
                    None => return world,
                }
            }
            3 => match self.nearest_on_curves(&d.snap_curves, px, f64::INFINITY) {
                Some(p) => p,
                None => return world,
            },
            4 => {
                // Stays in the plane through the pivot facing the camera.
                let n = (self.cam.eye() - d.pivot).normalize_or_zero();
                return world - n * world.dot(n);
            }
            5 => match self.live_hit {
                Some(p) => p,
                None => return world,
            },
            _ => {
                // Tool Settings' step, or a held J: whole steps of the offset, or of the world when absolute. The step
                // is in the current linear unit.
                let step = if self.step_move > 0.0 { self.step_move } else if self.discrete_hold { 1.0 } else { 0.0 } * aztek_ops::units::cm_per_unit();
                if step <= 0.0 {
                    return world;
                }
                if self.snap_relative {
                    return (world / step).round() * step;
                }
                let target = ((d.pivot + world) / step).round() * step;
                let off = target - d.pivot;
                return match handle {
                    Handle::Axis(i) => self.frame.col(i) * off.dot(self.frame.col(i)),
                    Handle::Plane(i) => off - self.frame.col(i) * off.dot(self.frame.col(i)),
                    _ => off,
                };
            }
        };
        let off = target - d.pivot;
        match handle {
            Handle::Axis(i) => {
                let a = self.frame.col(i);
                a * off.dot(a)
            }
            Handle::Plane(i) => {
                let n = self.frame.col(i);
                off - n * off.dot(n)
            }
            _ => off,
        }
    }

    /// Shift+middle-drag: the axis whose line on screen the drag runs along most closely.
    pub fn axis_along(&self, pivot: DVec3, delta: DVec2) -> Handle {
        let screen = self.screen();
        let Some(c) = screen.project(pivot) else { return Handle::Center };
        let unit = self.units_per_px(pivot) * 100.0;
        let mut best = (0.0, Handle::Center);
        for i in 0..3 {
            let Some(tip) = screen.project(pivot + self.frame.col(i) * unit) else { continue };
            let along = (tip - c).normalize_or_zero().dot(delta.normalize_or_zero()).abs();
            if along > best.0 {
                best = (along, Handle::Axis(i));
            }
        }
        best.1
    }

    /// The tool the manipulator acts as: pivot mode moves pivots whatever the tool.
    pub fn eff_tool(&self) -> Tool {
        if self.pivot_mode && self.manipulates() {
            Tool::Move
        } else {
            self.tool
        }
    }

    fn plane_hit(&self, pivot: DVec3, n: DVec3, px: DVec2) -> Option<DVec3> {
        let (o, d) = self.cam.ray(px, self.size());
        let den = d.dot(n);
        if den.abs() < 1e-6 {
            return None;
        }
        Some(o + d * ((pivot - o).dot(n) / den))
    }

    /// World Euler angles, in radians, for a rotation in degrees about a world axis.
    fn euler_about(axis: DVec3, deg: f64) -> [f64; 3] {
        let q = glam::DQuat::from_axis_angle(axis.normalize_or_zero(), deg.to_radians());
        let e = aztek_scene::transform::matrix_to_euler(&DMat3::from_quat(q), aztek_scene::transform::RotateOrder::Xyz);
        [e.x, e.y, e.z]
    }

    /// The command for a drag from d.start to px: (command, call).
    pub fn drag_call(&self, d: &Drag, px: DVec2, scale: f64) -> Option<(String, serde_json::Value)> {
        let screen = self.screen();
        let c = screen.project(d.pivot)?;
        let delta = px - d.start;
        let upp = self.units_per_px(d.pivot);
        let comps = d.components;
        let targets = serde_json::json!(d.targets);
        let view = self.cam.view();
        let right = DVec3::new(view.x_axis.x, view.y_axis.x, view.z_axis.x);
        let up = DVec3::new(view.x_axis.y, view.y_axis.y, view.z_axis.y);
        if let Handle::PivotRing(i) = d.handle {
            // The pivot's axes turn about this one by the angle swept on screen; the object stays.
            let ax = self.frame.col(i);
            let mut ang = ((px - c).to_angle() - (d.start - c).to_angle()).to_degrees();
            if ang > 180.0 {
                ang -= 360.0;
            } else if ang < -180.0 {
                ang += 360.0;
            }
            ang *= -(self.cam.eye() - d.pivot).dot(ax).signum();
            let step = if self.step_rotate > 0.0 { self.step_rotate } else if self.discrete_hold { 15.0 } else { 0.0 };
            if step > 0.0 {
                ang = (ang / step).round() * step;
            }
            return Some(("setPivot".into(), serde_json::json!({ "args": targets, "turnAxis": ax.to_array(), "turnAngle": ang })));
        }
        if d.handle == Handle::Normal {
            // Left and right push the points out along their normals and back, a pixel's worth at the pivot's depth.
            let dist = (px.x - d.start.x) * upp;
            if dist.abs() < 1e-12 {
                return None;
            }
            return Some(("moveComponents".into(), serde_json::json!({ "normal": dist })));
        }
        match self.eff_tool() {
            Tool::Move => {
                let handle = match d.handle {
                    Handle::Center => self.constraint.unwrap_or(Handle::Center),
                    h => h,
                };
                let world = match handle {
                    Handle::Axis(i) => {
                        let ax = self.frame.col(i);
                        let tip = screen.project(d.pivot + ax * upp * 100.0)?;
                        let sdir = tip - c;
                        let l2 = sdir.length_squared();
                        if l2 < 1e-6 {
                            return None;
                        }
                        ax * (delta.dot(sdir) / l2) * upp * 100.0
                    }
                    Handle::Plane(i) => {
                        let n = self.frame.col(i);
                        self.plane_hit(d.pivot, n, px)? - self.plane_hit(d.pivot, n, d.start)?
                    }
                    _ => (right * delta.x - up * delta.y) * upp,
                };
                let world = self.snapped(d, world, handle, px);
                let by = serde_json::json!(world.to_array());
                Some(if comps && self.snap == 1 && !self.keep_spacing && !self.pivot_mode {
                    // Each point on the grid by itself: the move before any rounding.
                    let raw = (right * delta.x - up * delta.y) * upp;
                    let raw = match handle {
                        Handle::Axis(i) => self.frame.col(i) * raw.dot(self.frame.col(i)),
                        Handle::Plane(i) => raw - self.frame.col(i) * raw.dot(self.frame.col(i)),
                        _ => raw,
                    };
                    ("moveComponents".into(), serde_json::json!({ "by": raw.to_array(), "snapEach": self.snap_step() }))
                } else if comps && self.axis_mode == "normal" && !self.pivot_mode {
                    // Each point along its own U, V and N, as the Normal orientation moves them.
                    let f = self.frame;
                    ("moveComponents".into(), serde_json::json!({ "uvn": [world.dot(f.x_axis), world.dot(f.y_axis), world.dot(f.z_axis)] }))
                } else if comps {
                    ("moveComponents".into(), serde_json::json!({ "by": by }))
                } else if self.pivot_mode {
                    let pivots: Vec<String> = d.targets.iter().flat_map(|t| [format!("{t}.scalePivot"), format!("{t}.rotatePivot")]).collect();
                    ("move".into(), serde_json::json!({ "args": pivots, "by": by }))
                } else if self.preserve_children {
                    ("move".into(), serde_json::json!({ "args": targets, "by": by, "preserveChildPosition": true }))
                } else {
                    ("move".into(), serde_json::json!({ "args": targets, "by": by }))
                })
            }
            Tool::Rotate => {
                let step = if self.step_rotate > 0.0 { self.step_rotate } else if self.discrete_hold { 15.0 } else { 0.0 };
                let snap = |a: f64| if step > 0.0 { (a / step).round() * step } else { a };
                let sweep = |ax: DVec3| {
                    // Angle swept about the pivot on screen; screen y runs down, so the sign follows the axis's facing.
                    let a0 = (d.start - c).to_angle();
                    let a1 = (px - c).to_angle();
                    let mut ang = (a1 - a0).to_degrees();
                    if ang > 180.0 {
                        ang -= 360.0;
                    } else if ang < -180.0 {
                        ang += 360.0;
                    }
                    ang * -(self.cam.eye() - d.pivot).dot(ax).signum()
                };
                // Absolute step snap in Gimbal: the channel turned lands on a whole step, not the angle swept. Gimbal's
                // axes are the channels' own, so the handle turns that channel alone.
                let absolute = !self.snap_relative && step > 0.0 && self.axis_mode == "gimbal" && d.targets.len() == 1 && !comps;
                let by: [f64; 3] = match d.handle {
                    Handle::Axis(i) => {
                        let ax = self.frame.col(i);
                        let ang = match (absolute, d.start_rotate) {
                            (true, Some(r0)) => {
                                let r0 = r0[i];
                                // The channel's sign against the handle's axis: a turn of +a about the axis is +a on it.
                                ((r0 + sweep(ax)) / step).round() * step - r0
                            }
                            _ => snap(sweep(ax)),
                        };
                        if self.frame == DMat3::IDENTITY {
                            let mut by = [0.0; 3];
                            by[i] = ang.to_radians();
                            by
                        } else {
                            Self::euler_about(ax, ang)
                        }
                    }
                    Handle::ViewRing => {
                        let ax = (self.cam.eye() - d.pivot).normalize_or_zero();
                        Self::euler_about(ax, snap(sweep(ax)))
                    }
                    Handle::Trackball => {
                        // Left and right turn about the view's up, up and down about its right.
                        let q = glam::DQuat::from_axis_angle(up, (delta.x * 0.5).to_radians()) * glam::DQuat::from_axis_angle(right, (delta.y * 0.5).to_radians());
                        let e = aztek_scene::transform::matrix_to_euler(&DMat3::from_quat(q), aztek_scene::transform::RotateOrder::Xyz);
                        [e.x, e.y, e.z]
                    }
                    _ => return None,
                };
                Some(if comps {
                    ("rotateComponents".into(), serde_json::json!({ "by": by, "pivot": d.pivot.to_array() }))
                } else if self.use_manip_pivot {
                    // The manipulator's pivot moves the objects themselves, by translate.
                    let mut call = serde_json::json!({ "args": targets, "by": by, "pivot": d.pivot.to_array(), "translate": true });
                    if self.preserve_children {
                        call["preserveChildPosition"] = serde_json::json!(true);
                    }
                    ("rotate".into(), call)
                } else if self.preserve_children {
                    ("rotate".into(), serde_json::json!({ "args": targets, "by": by, "preserveChildPosition": true }))
                } else {
                    ("rotate".into(), serde_json::json!({ "args": targets, "by": by }))
                })
            }
            Tool::Scale => {
                let mut f = 1.0 + (delta.x - delta.y) / (100.0 * scale);
                if self.prevent_negative {
                    f = f.max(0.001);
                }
                let step = if self.step_scale > 0.0 { self.step_scale } else if self.discrete_hold { 1.0 } else { 0.0 };
                // Absolute: the scale channel lands on a whole step (one object, an axis handle); relative: the factor.
                let absolute_axis = match d.handle {
                    Handle::Axis(i) if !self.snap_relative && step > 0.0 && d.targets.len() == 1 && !comps => d.start_scale.map(|s0| (i, s0[i])),
                    _ => None,
                };
                if let Some((_, s0)) = absolute_axis {
                    if s0.abs() > 1e-12 {
                        let target = ((s0 * f) / step).round() * step;
                        f = target / s0;
                    }
                } else if step > 0.0 {
                    f = (f / step).round() * step;
                    if self.prevent_negative {
                        f = f.max(0.001);
                    }
                }
                let by = match d.handle {
                    Handle::Axis(i) => {
                        let mut b = [1.0; 3];
                        b[i] = f;
                        b
                    }
                    Handle::Plane(i) => {
                        let mut b = [f; 3];
                        b[i] = 1.0;
                        b
                    }
                    _ => [f; 3],
                };
                Some(if comps {
                    // Component scale follows the manipulator's axes: world, object, normal.
                    let mut call = serde_json::json!({ "by": by, "pivot": d.pivot.to_array() });
                    if self.frame != DMat3::IDENTITY {
                        let e = aztek_scene::transform::matrix_to_euler(&self.frame, aztek_scene::transform::RotateOrder::Xyz);
                        call["orient"] = serde_json::json!([e.x, e.y, e.z]);
                    }
                    ("scaleComponents".into(), call)
                } else if self.use_manip_pivot {
                    let mut call = serde_json::json!({ "args": targets, "by": by, "pivot": d.pivot.to_array() });
                    if self.preserve_children {
                        call["preserveChildPosition"] = serde_json::json!(true);
                    }
                    ("scale".into(), call)
                } else if self.preserve_children {
                    ("scale".into(), serde_json::json!({ "args": targets, "by": by, "preserveChildPosition": true }))
                } else {
                    ("scale".into(), serde_json::json!({ "args": targets, "by": by }))
                })
            }
            _ => None,
        }
    }

    /// Frames the selection (or everything when nothing is selected or all is true).
    pub fn frame(&mut self, s: &mut Session, items: &[DrawItem], all: bool) {
        let usd_box = crate::usd_view::bounds(s, all);
        let g = &s.graph;
        let mode = components::mode(g);
        let mut lo = DVec3::splat(f64::MAX);
        let mut hi = DVec3::splat(f64::MIN);
        let sel = g.selection().to_vec();
        let mut add = |p: DVec3| {
            lo = lo.min(p);
            hi = hi.max(p);
        };
        let mut any = false;
        if let Some((l, h)) = usd_box {
            add(l);
            add(h);
            any = true;
        }
        for it in items {
            let picked = sel.contains(&it.xf);
            if !all && !sel.is_empty() && !picked {
                continue;
            }
            let ids: Vec<Index> = if !all && picked && mode != ComponentMode::Object {
                let c = components::selected(g, it.shape, mode);
                if c.is_empty() {
                    (0..it.mesh.point_capacity()).filter(|&p| it.mesh.point_valid(p)).collect()
                } else {
                    components::convert(&it.mesh, mode, c, ComponentMode::Vertex, false)
                }
            } else {
                (0..it.mesh.point_capacity()).filter(|&p| it.mesh.point_valid(p)).collect()
            };
            for p in ids {
                add(it.world.transform_point3(it.mesh.position(p)));
                any = true;
            }
        }
        if mode == ComponentMode::Object || all {
            for it in rig_items(g) {
                if !all && !sel.is_empty() && !sel.contains(&it.node) {
                    continue;
                }
                for (a, b) in &it.segs {
                    add(*a);
                    add(*b);
                    any = true;
                }
            }
        }
        if !any {
            if all || sel.is_empty() {
                let aspect = self.aspect();
                self.cam.frame(DVec3::ZERO, self.grid_spacing * 12.0, aspect);
            }
            return;
        }
        let aspect = self.aspect();
        self.cam.frame((lo + hi) * 0.5, ((hi - lo).length() * 0.5).max(1e-3), aspect);
    }

    /// Rebuilds the grid (after a camera change moves its plane).
    pub fn reset_grid(&mut self, gpu: &mut Gpu) {
        if let Some((b, _)) = self.grid.take() {
            gpu.drop_buffer(b);
        }
        if let Some((b, _)) = self.grid_bold.take() {
            gpu.drop_buffer(b);
        }
    }

    pub fn release(&mut self, gpu: &mut Gpu) {
        for (_, m) in self.meshes.drain() {
            drop_mesh(gpu, m);
        }
        if let Some((b, _)) = self.grid_bold.take() {
            gpu.drop_buffer(b);
        }
        if let Some((b, _)) = self.grid.take() {
            gpu.drop_buffer(b);
        }
        if let Some(l) = self.lit.take() {
            l.release(gpu);
        }
    }
}

fn r_supports(gpu: &Gpu) -> bool {
    let limits = unsafe { gpu.shared.instance.get_physical_device_properties(gpu.shared.phys).limits };
    limits.max_push_constants_size as usize >= std::mem::size_of::<MatPush>()
}

fn drop_mesh(gpu: &mut Gpu, m: GpuMesh) {
    if let Some(b) = m.uvs {
        gpu.drop_buffer(b);
    }
    if let Some(b) = m.batch_ib {
        gpu.drop_buffer(b);
    }
    if let Some((a, b, _)) = m.tris {
        gpu.drop_buffer(a);
        gpu.drop_buffer(b);
    }
    for b in [m.lines, m.points, m.tint, m.mirror_tint].into_iter().flatten() {
        gpu.drop_buffer(b.0);
    }
    if let Some((a, b, _)) = m.weight_tris {
        gpu.drop_buffer(a);
        gpu.drop_buffer(b);
    }
}

/// The UVs of the shells.
fn uv_shell_maps(m: &Mesh, tp: &aztek_scene::uv_ops::UvTopo, shells: &std::collections::BTreeSet<Index>) -> Vec<Index> {
    let mut maps = Vec::new();
    for &sh in shells {
        for &f in tp.shells.get(sh as usize).into_iter().flatten() {
            m.for_each_face_corner(f, |c| {
                if let Some(&mp) = tp.corner_map.get(c as usize) {
                    if mp != aztek_geo::INVALID {
                        maps.push(mp);
                    }
                }
            });
        }
    }
    maps.sort_unstable();
    maps.dedup();
    maps
}

fn push_ranges(out: &mut Vec<String>, path: &str, p: &str, ids: &[Index]) {
    if ids.is_empty() {
        return;
    }
    let kind = match p {
        "vtx" => ComponentMode::Vertex,
        "e" => ComponentMode::Edge,
        "map" => ComponentMode::Uv,
        _ => ComponentMode::Face,
    };
    for c in aztek_scene::history::format_components(kind, ids).split_whitespace() {
        out.push(format!("{path}.{c}"));
    }
}

fn seg_dist(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let d = b - a;
    let l2 = d.dot(d);
    let t = if l2 > 0.0 { ((p - a).dot(d) / l2).clamp(0.0, 1.0) } else { 0.0 };
    (a + d * t - p).length()
}

fn push(mvp: DMat4, model: DMat4, view: DMat4, color: [f32; 4], bias: f32, point: f32) -> Push {
    let nm = (view * model).inverse().transpose();
    let m = nm.as_mat4();
    // Rows of the upper 3x3.
    let row = |i: usize| [m.x_axis[i], m.y_axis[i], m.z_axis[i]];
    let (r0, r1, r2) = (row(0), row(1), row(2));
    Push {
        mvp: mvp.as_mat4().to_cols_array(),
        n0: [r0[0], r0[1], r0[2], bias],
        n1: [r1[0], r1[1], r1[2], point],
        n2: [r2[0], r2[1], r2[2], 0.0],
        color,
    }
}

/// Whether a screen segment passes through the square of half side k around c.
fn seg_hits_box(a: DVec2, b: DVec2, c: DVec2, k: f64) -> bool {
    // Liang-Barsky clipping against the square.
    let d = b - a;
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-d.x, a.x - (c.x - k)), (d.x, (c.x + k) - a.x), (-d.y, a.y - (c.y - k)), (d.y, (c.y + k) - a.y)] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

fn seg_distance(p: DVec2, a: DVec2, b: DVec2) -> f64 {
    let ab = b - a;
    let t = if ab.length_squared() < 1e-12 { 0.0 } else { ((p - a).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
    (p - (a + ab * t)).length()
}

/// Something drawn as lines in world space: joints, locators, IK handles.
pub struct RigItem {
    pub node: NodeId,
    /// The shape it draws: what its colour, display type and pick refusal are read from.
    pub shape: NodeId,
    pub color: [u8; 4],
    pub segs: Vec<(DVec3, DVec3)>,
    /// Triangles that pick as solidly as the lines: a joint's ball and bones.
    pub fills: Vec<[DVec3; 3]>,
}

/// Paint Skin Weights' colour ramp: black at 0, then blue, green, yellow and red at 1.
pub fn weight_ramp(x: f32) -> [f32; 4] {
    const STOPS: [(f32, [f32; 3]); 5] = [(0.0, [0.0, 0.0, 0.0]), (0.25, [0.0, 0.25, 1.0]), (0.5, [0.0, 0.85, 0.2]), (0.75, [1.0, 0.9, 0.0]), (1.0, [1.0, 0.1, 0.05])];
    let x = x.clamp(0.0, 1.0);
    for w in STOPS.windows(2) {
        let ((a, ca), (b, cb)) = (w[0], w[1]);
        if x <= b {
            let t = if b > a { (x - a) / (b - a) } else { 0.0 };
            return [ca[0] + (cb[0] - ca[0]) * t, ca[1] + (cb[1] - ca[1]) * t, ca[2] + (cb[2] - ca[2]) * t, 1.0];
        }
    }
    [1.0, 0.1, 0.05, 1.0]
}

/// Whether a point falls on any of the triangles as drawn.
fn fill_hit(screen: &Screen, fills: &[[DVec3; 3]], px: DVec2) -> bool {
    fills.iter().any(|t| match (screen.project(t[0]), screen.project(t[1]), screen.project(t[2])) {
        // Seen edge on, a triangle has no inside to pick.
        (Some(a), Some(b), Some(c)) => (b - a).perp_dot(c - a).abs() > 0.5 && in_triangle(px, a, b, c),
        _ => false,
    })
}

/// Where a kind of object stands when one region takes several kinds, as the reference's selection priority: handles,
/// then joints, then curves, then everything else. Lower goes first.
fn priority_rank(g: &Graph, id: NodeId) -> u8 {
    match type_category(g, id) {
        "ikHandle" | "deformer" => 0,
        "joint" => 1,
        "curve" => 2,
        _ => 3,
    }
}

/// What a region selects of the objects it takes: only those of the highest priority, and of joints only the
/// highest in each chain, as the reference's. Each pair is the object hit and what that selects.
fn by_priority(g: &Graph, picked: &[(NodeId, NodeId)]) -> Vec<String> {
    let Some(top) = picked.iter().map(|p| priority_rank(g, p.0)).min() else { return Vec::new() };
    // selectPref -ignoreSelectionPriority: every kind the region takes.
    let ignore = crate::ui_cmds::select_pref("ignoreSelectionPriority", false);
    let won: Vec<&(NodeId, NodeId)> = picked.iter().filter(|p| ignore || priority_rank(g, p.0) == top).collect();
    let hit: std::collections::HashSet<NodeId> = won.iter().map(|p| p.0).collect();
    let under_hit_joint = |id: NodeId| {
        let mut n = g.parent(id);
        while n != NO_NODE {
            if hit.contains(&n) && g.node(n).is_some_and(|x| x.ty().is_a("joint")) {
                return true;
            }
            n = g.parent(n);
        }
        false
    };
    let mut out: Vec<String> = Vec::new();
    for &&(node, sel) in &won {
        if g.node(node).is_some_and(|x| x.ty().is_a("joint")) && under_hit_joint(node) {
            continue;
        }
        let path = g.path(sel);
        if !out.contains(&path) {
            out.push(path);
        }
    }
    out
}

fn circle(c: DVec3, u: DVec3, v: DVec3, r: f64, out: &mut Vec<(DVec3, DVec3)>) {
    let n = 20;
    for i in 0..n {
        let a = i as f64 / n as f64 * std::f64::consts::TAU;
        let b = (i + 1) as f64 / n as f64 * std::f64::consts::TAU;
        out.push((c + (u * a.cos() + v * a.sin()) * r, c + (u * b.cos() + v * b.sin()) * r));
    }
}

/// A line item with its colour settled, or twelve lines round it at overrideLevelOfDetail's bounding box.
fn rig_item(g: &Graph, shape: NodeId, node: NodeId, kind: &str, factory: [u8; 4], segs: Vec<(DVec3, DVec3)>) -> RigItem {
    let st = ds::style(g, shape);
    let segs = if st.bounding_box { box_segs(&segs) } else { segs };
    RigItem { node, shape, color: st.color.unwrap_or_else(|| ds::dormant_color(kind, factory)), segs, fills: Vec::new() }
}

fn box_segs(segs: &[(DVec3, DVec3)]) -> Vec<(DVec3, DVec3)> {
    let (mut lo, mut hi) = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
    for (a, b) in segs {
        lo = lo.min(a.min(*b));
        hi = hi.max(a.max(*b));
    }
    let corner = |i: usize| DVec3::new(if i & 1 == 0 { lo.x } else { hi.x }, if i & 2 == 0 { lo.y } else { hi.y }, if i & 4 == 0 { lo.z } else { hi.z });
    let mut out = Vec::new();
    for i in 0..8usize {
        for bit in [1usize, 2, 4] {
            if i & bit == 0 {
                out.push((corner(i), corner(i | bit)));
            }
        }
    }
    out
}

/// A component string for a CV by its id: `cv[i]`, or `cv[u][v]` on a surface with `stride` CVs along V.
fn cv_name(path: &str, i: usize, stride: Option<usize>) -> String {
    match stride {
        Some(s) => format!("{path}.cv[{}][{}]", i / s.max(1), i % s.max(1)),
        None => format!("{path}.cv[{i}]"),
    }
}

/// The gimbal axes of a transform: its rotate channels' axes in world space, in the order X, Y, Z, as its rotate order
/// applies them (the first applied turns with the later ones).
fn gimbal_frame(g: &Graph, id: NodeId) -> Option<DMat3> {
    let n = g.node(id)?;
    let t = n.ty();
    let vec = |name: &str| t.attr_index(name).map(|i| n.data().value(i).as_vec3()).unwrap_or(DVec3::ZERO);
    let r = vec("rotate");
    let order = t.attr_index("rotateOrder").map(|i| n.data().value(i).as_int()).unwrap_or(0);
    let ortho = |m: DMat3| DMat3::from_cols(m.x_axis.normalize_or_zero(), m.y_axis.normalize_or_zero(), m.z_axis.normalize_or_zero());
    let parent = g.parent(id);
    let pw = match g.node(parent) {
        Some(p) if parent != NO_NODE => ortho(DMat3::from_mat4(p.data().matrix(DagNode::WORLD_MATRIX))),
        _ => DMat3::IDENTITY,
    };
    // A joint's orient sits outside its rotation.
    let jo = if t.is_a("joint") {
        let o = vec("jointOrient");
        DMat3::from_rotation_z(o.z) * DMat3::from_rotation_y(o.y) * DMat3::from_rotation_x(o.x)
    } else {
        DMat3::IDENTITY
    };
    // The axes in the order applied: xyz applies X first, so Z is outermost.
    let seq: [usize; 3] = match order {
        1 => [1, 2, 0],
        2 => [2, 0, 1],
        3 => [0, 2, 1],
        4 => [1, 0, 2],
        5 => [2, 1, 0],
        _ => [0, 1, 2],
    };
    let unit = |i: usize| [DVec3::X, DVec3::Y, DVec3::Z][i];
    let rot = |i: usize| DMat3::from_axis_angle(unit(i), r[i]);
    let outer = pw * jo;
    let mut cols = [DVec3::ZERO; 3];
    // Outermost first: its axis as it is, then each inner one turned by those outside it.
    let mut acc = outer;
    for k in (0..3).rev() {
        let i = seq[k];
        cols[i] = (acc * unit(i)).normalize_or_zero();
        acc *= rot(i);
    }
    Some(DMat3::from_cols(cols[0], cols[1], cols[2]))
}

/// Each visible curve and surface shape, its transform and its CVs in world space, one lap for a periodic direction. A
/// surface also gives its count along V, which divides the ids into cv[u][v].
fn curve_cvs(g: &Graph) -> Vec<(NodeId, NodeId, Vec<DVec3>, Option<usize>)> {
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        // Types first: only DAG nodes have the intermediate attribute.
        if !(n.ty().is_a("nurbsCurve") || n.ty().is_a("nurbsSurface")) || n.bool(DagNode::INTERMEDIATE_OBJECT) || !visible(g, id) {
            continue;
        }
        if n.ty().is_a("nurbsCurve") {
            let Some(c) = aztek_scene::curves::drawn_curve(n) else { continue };
            out.push((id, aztek_scene::dag::transform_of(g, id), c.distinct_cvs().to_vec(), None));
        } else if n.ty().is_a("nurbsSurface") {
            let Some((cvs, stride)) = surfaces::drawn_cvs(n) else { continue };
            out.push((id, aztek_scene::dag::transform_of(g, id), cvs, Some(stride)));
        }
    }
    out
}

/// The selected lattices: transform, shape, grid size and points in world space.
fn selected_lattices(g: &Graph) -> Vec<(NodeId, NodeId, [u32; 3], Vec<DVec3>)> {
    let mut out = Vec::new();
    for &xf in g.selection() {
        let Some(shape) = aztek_scene::dag::shape_of_type(g, xf, "lattice") else { continue };
        let Some((d, pts)) = aztek_scene::deform_lattice::world_points(g, shape) else { continue };
        out.push((xf, shape, [d[0] as u32, d[1] as u32, d[2] as u32], pts));
    }
    out
}

/// A joint's colour when nothing sets one: a mid blue, a shade darker than it was, nearer the reference's (#150).
/// Unverified against a measured value.
pub const JOINT_COLOR: [u8; 4] = [6, 9, 98, 255];

/// The world segments of the curves drawn in the view, for a curve snap (C) to land on.
pub fn snap_curves(g: &Graph) -> Vec<(DVec3, DVec3)> {
    rig_items(g).into_iter().filter(|it| type_category(g, it.node) == "curve").flat_map(|it| it.segs).collect()
}

pub fn rig_items(g: &Graph) -> Vec<RigItem> {
    use aztek_scene::rig::Joint;
    use aztek_scene::transform::{Locator, Transform};
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        let ty = n.ty();
        if !ty.dag || !visible(g, id) {
            continue;
        }
        let w = n.data().matrix(DagNode::WORLD_MATRIX);
        let c = w.w_axis.truncate();
        if ty.is_a("joint") {
            let radius = n.data().float(Joint::RADIUS) * 0.5 * crate::ui_cmds::pref_f64("jointScale", 1.0);
            let (x, y, z) = (w.x_axis.truncate().normalize_or_zero(), w.y_axis.truncate().normalize_or_zero(), w.z_axis.truncate().normalize_or_zero());
            let mut segs = Vec::new();
            let mut fills = Vec::new();
            circle(c, x, y, radius, &mut segs);
            circle(c, y, z, radius, &mut segs);
            circle(c, z, x, radius, &mut segs);
            for &k in g.children(id) {
                let Some(kn) = g.node(k) else { continue };
                if !kn.ty().is_a("joint") {
                    continue;
                }
                let tip = kn.data().matrix(DagNode::WORLD_MATRIX).w_axis.truncate();
                let d = tip - c;
                let len = d.length();
                if len <= radius {
                    continue;
                }
                let dir = d / len;
                let (u, v) = dir.any_orthonormal_pair();
                let base = c + dir * radius;
                let k = radius * 0.6;
                let corners = [base + u * k, base + v * k, base - u * k, base - v * k];
                for i in 0..4 {
                    segs.push((corners[i], corners[(i + 1) % 4]));
                    segs.push((corners[i], tip));
                    fills.push([corners[i], corners[(i + 1) % 4], tip]);
                }
                fills.push([corners[0], corners[1], corners[2]]);
                fills.push([corners[0], corners[2], corners[3]]);
            }
            // The ball picks across its whole face, not only on its rings.
            for (a, b) in segs.iter().take(60) {
                fills.push([c, *a, *b]);
            }
            let mut item = rig_item(g, id, id, "joint", JOINT_COLOR, segs);
            if !ds::style(g, id).bounding_box {
                item.fills = fills;
            }
            out.push(item);
        } else if let Some(kind) = aztek_scene::deform::handle_shape_kind(&ty.name) {
            // A cluster's handle draws at its shape's origin, which is the transform's pivot.
            let at = match ty.attr_index("origin") {
                Some(o) => w * glam::DMat4::from_translation(n.data().vec3(o)),
                None => w,
            };
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "cluster", [200, 120, 230, 255], aztek_scene::deform::handle_lines(kind, &at)));
        } else if let Some(segs) = aztek_scene::rig_misc::shape_lines(n) {
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "dimension", [60, 90, 230, 255], segs));
        } else if ty.is_a("nurbsCurve") {
            if n.bool(DagNode::INTERMEDIATE_OBJECT) {
                continue;
            }
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "curve", [10, 10, 110, 255], aztek_scene::curves::curve_lines(n)));
        } else if ty.is_a("light") {
            let factory = ds::palette_bytes(ds::display_index("light", false).unwrap_or(16));
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "light", factory, aztek_scene::lights::icon_lines(n, &w)));
        } else if ty.is_a("camera") {
            // A box and a lens, for cameras that show: the default views' are hidden. It scales with the transform and
            // with the shape's locatorScale, as the reference's does.
            let (x, y, z) = (w.x_axis.truncate(), w.y_axis.truncate(), w.z_axis.truncate());
            let k = 0.5 * ty.attr_index("locatorScale").map_or(1.0, |i| n.data().float(i));
            let corner = |a: f64, b: f64, d: f64| c + x * a * k + y * b * k + z * d * k;
            let mut segs = Vec::new();
            for d in [0.0, 1.5] {
                let q = [corner(-1.0, -0.7, d), corner(1.0, -0.7, d), corner(1.0, 0.7, d), corner(-1.0, 0.7, d)];
                for i in 0..4 {
                    segs.push((q[i], q[(i + 1) % 4]));
                    if d > 0.0 {
                        segs.push((q[i], corner(if i == 0 || i == 3 { -1.0 } else { 1.0 }, if i < 2 { -0.7 } else { 0.7 }, 0.0)));
                    }
                }
            }
            circle(c - z * k * 0.5, x, y, k * 0.5, &mut segs);
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "camera", [60, 90, 230, 255], segs));
        } else if ty.is_a("softModHandle") {
            let at = w * glam::DMat4::from_translation(n.data().vec3(aztek_scene::transform::Shape::COUNT));
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "cluster", [200, 120, 230, 255], aztek_scene::deform::handle_lines("softMod", &at)));
        } else if ty.is_a("baseLattice") {
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "lattice", [130, 70, 150, 255], aztek_scene::deform_lattice::box_lines(&w)));
        } else if ty.is_a("lattice") {
            let Some((dims, pts)) = aztek_scene::deform_lattice::world_points(g, id) else { continue };
            let mut segs = aztek_scene::deform_lattice::grid_lines(dims, &pts);
            let cross = |p: DVec3, k: f64, segs: &mut Vec<(DVec3, DVec3)>| {
                for ax in [DVec3::X, DVec3::Y, DVec3::Z] {
                    segs.push((p - ax * k, p + ax * k));
                }
            };
            // A control point's mark, scaled to the lattice.
            let k = pts.first().zip(pts.get(1)).map_or(0.05, |(a, b)| (*a - *b).length() * 0.04).max(1e-4);
            for p in &pts {
                cross(*p, k, &mut segs);
            }
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "lattice", [200, 120, 230, 255], segs));
            // Selected points show in vertex mode. No node: this is a highlight, and it mustn't pick or take selection colours.
            if components::mode(g) == ComponentMode::Vertex {
                let picked = components::selected(g, id, ComponentMode::Vertex);
                if !picked.is_empty() {
                    let mut segs = Vec::new();
                    for p in picked.iter().filter_map(|&i| pts.get(i as usize)) {
                        cross(*p, k * 2.5, &mut segs);
                    }
                    out.push(RigItem { node: aztek_scene::NO_NODE, shape: aztek_scene::NO_NODE, color: [255, 200, 60, 255], segs, fills: Vec::new() });
                }
            }
        } else if ty.is_a("motionTrailShape") {
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "dimension", [220, 200, 80, 255], aztek_scene::motion_path::trail_lines(n)));
        } else if ty.is_a("ikHandle") {
            let k = 0.4 * crate::ui_cmds::pref_f64("ikHandleScale", 1.0);
            let segs = vec![
                (c - DVec3::X * k, c + DVec3::X * k),
                (c - DVec3::Y * k, c + DVec3::Y * k),
                (c - DVec3::Z * k, c + DVec3::Z * k),
            ];
            out.push(rig_item(g, id, id, "ikHandle", [200, 200, 255, 255], segs));
        } else if let Some(items) = crate::dyn_draw::glyph(g, id) {
            out.extend(items);
        } else if ty.is_a("locator") {
            let lp = n.data().vec3(Locator::LOCAL_POSITION);
            let ls = n.data().vec3(Locator::LOCAL_SCALE);
            let p = w.transform_point3(lp);
            let mut segs = Vec::new();
            for (i, ax) in [w.x_axis, w.y_axis, w.z_axis].iter().enumerate() {
                let d = ax.truncate() * ls[i];
                segs.push((p - d, p + d));
            }
            out.push(rig_item(g, id, aztek_scene::dag::transform_of(g, id), "locator", [60, 90, 230, 255], segs));
        }
        let _ = Transform::TRANSLATE;
    }
    out.extend(crate::dyn_draw::cloth_marks(g));
    out
}

/// A selected joint's transform hint, as the reference draws it: its axes, red X, green Y and blue Z, from the centre
/// out to its ball, with no letters (those are Local Rotation Axes). Only while the joint is selected (#138).
pub fn joint_hints(g: &Graph) -> Vec<LineVertex> {
    use aztek_scene::rig::Joint;
    let mut out = Vec::new();
    if components::mode(g) != ComponentMode::Object {
        return out;
    }
    let scale = crate::ui_cmds::pref_f64("jointScale", 1.0);
    for &id in g.selection() {
        let Some(n) = g.node(id) else { continue };
        if !n.ty().is_a("joint") || !visible(g, id) {
            continue;
        }
        let w = n.data().matrix(DagNode::WORLD_MATRIX);
        let o = w.w_axis.truncate();
        let r = n.data().float(Joint::RADIUS) * 0.5 * scale;
        for (axis, c) in [(w.x_axis, [220u8, 50, 50, 255]), (w.y_axis, [50, 200, 50, 255]), (w.z_axis, [50, 90, 240, 255])] {
            out.push(LineVertex { pos: o.as_vec3().to_array(), color: c8(c) });
            out.push(LineVertex { pos: (o + axis.truncate().normalize_or_zero() * r).as_vec3().to_array(), color: c8(c) });
        }
    }
    out
}

/// The selection mask and Show menu category of a node: its transform takes its shape's.
pub fn type_category(g: &Graph, id: NodeId) -> &'static str {
    let Some(n) = g.node(id) else { return "other" };
    let ty = n.ty();
    if ty.is_a("joint") {
        return "joint";
    }
    if ty.is_a("ikHandle") {
        return "ikHandle";
    }
    if aztek_scene::deform::handle_shape_kind(&ty.name).is_some() || ty.is_a("softModHandle") || ty.is_a("lattice") || ty.is_a("baseLattice") {
        return "deformer";
    }
    if ty.is_a("nurbsSurface") {
        return "surface";
    }
    if ty.is_a("mesh") {
        return "polymesh";
    }
    if ty.is_a("nurbsCurve") {
        return "curve";
    }
    if ty.is_a("locator") {
        return "locator";
    }
    if ty.is_a("camera") {
        return "camera";
    }
    if ty.is_a("light") {
        return "light";
    }
    if ty.is_a("transform") {
        if let Some(&s) = aztek_scene::dag::shapes(g, id, false).first() {
            if s != id {
                return type_category(g, s);
            }
        }
        return "group";
    }
    "other"
}

/// Whether an edge attribute holds a set value on this edge.
fn edge_flag(m: &Mesh, name: &str, e: Index) -> bool {
    if name == aztek_geo::attribute::names::HARD {
        m.edge_attrs().get::<bool>(name).get(e as usize).copied().unwrap_or(false)
    } else {
        m.edge_attrs().get::<f32>(name).get(e as usize).copied().unwrap_or(0.0) > 0.0
    }
}

/// Whether two segments in the screen cross.
fn segments_cross(a: DVec2, b: DVec2, c: DVec2, d: DVec2) -> bool {
    let side = |p: DVec2, q: DVec2, r: DVec2| (q - p).perp_dot(r - p);
    let (d1, d2, d3, d4) = (side(c, d, a), side(c, d, b), side(a, b, c), side(a, b, d));
    ((d1 > 0.0) != (d2 > 0.0)) && ((d3 > 0.0) != (d4 > 0.0))
}

/// Whether a point is inside a screen triangle, either winding.
fn in_triangle(p: DVec2, a: DVec2, b: DVec2, c: DVec2) -> bool {
    let (d1, d2, d3) = ((b - a).perp_dot(p - a), (c - b).perp_dot(p - b), (a - c).perp_dot(p - c));
    !((d1 < 0.0 || d2 < 0.0 || d3 < 0.0) && (d1 > 0.0 || d2 > 0.0 || d3 > 0.0))
}

/// Ribbons for strands in world space: two vertices a point, two triangles a segment. `eye` and, for an orthographic
/// view, the direction to it; `px` the size of a pixel (per unit of distance in perspective).
pub fn strand_ribbons(sets: &[aztek_scene::harold::strands::Strands], eye: DVec3, ortho_dir: Option<DVec3>, px: (f64, bool)) -> (Vec<LineVertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idx: Vec<u32> = Vec::new();
    for st in sets {
        let mut start = 0usize;
        for &n in &st.counts {
            let n = n as usize;
            if n < 2 || start + n > st.points.len() {
                start += n;
                continue;
            }
            let pts: Vec<DVec3> = st.points[start..start + n].iter().map(|p| st.world.transform_point3(p.as_dvec3())).collect();
            let base = verts.len() as u32;
            for i in 0..n {
                let p = pts[i];
                let t = if i + 1 < n { pts[i + 1] - pts[i] } else { pts[i] - pts[i - 1] }.normalize_or_zero();
                let to_eye = ortho_dir.unwrap_or_else(|| (eye - p).normalize_or_zero());
                let dist = if px.1 { 1.0 } else { (eye - p).length() };
                let r = st.radius.get(start + i).copied().unwrap_or(0.0) as f64 * st.world.x_axis.truncate().length();
                // At least a pixel across, so fine hair stays visible far off.
                let half = r.max(0.5 * px.0 * dist);
                let side = t.cross(to_eye).normalize_or_zero() * half;
                // Fibre lighting with the light at the eye: bright across the view, dim along it; a highlight on top.
                let along = t.dot(to_eye).abs();
                let diffuse = (1.0 - along * along).max(0.0).sqrt();
                let spec = diffuse.powf(60.0) * 0.35;
                let root = 0.55 + 0.45 * (i as f64 / (n - 1) as f64);
                let shade = |k: f64| -> [f32; 4] {
                    let c = |x: f32| ((x as f64 * (0.3 + 0.7 * diffuse) * root * k + spec).clamp(0.0, 1.0)) as f32;
                    [c(st.color[0]), c(st.color[1]), c(st.color[2]), 1.0]
                };
                verts.push(LineVertex { pos: (p - side).as_vec3().to_array(), color: shade(0.8) });
                verts.push(LineVertex { pos: (p + side).as_vec3().to_array(), color: shade(1.0) });
            }
            for i in 0..n as u32 - 1 {
                let a = base + i * 2;
                idx.extend_from_slice(&[a, a + 1, a + 3, a, a + 3, a + 2]);
            }
            start += n;
        }
    }
    (verts, idx)
}

#[cfg(test)]
mod shadow_cast_tests {
    use super::*;

    /// Glass lets the light through, so the shadow pass leaves it out: an opaque shadow under a transmissive shape would
    /// be refracted by the glass and blacken the floor it stands on.
    #[test]
    fn a_transmissive_material_keeps_its_shapes_out_of_the_shadow_map() {
        let mut vp = Viewport::new(CamKind::Persp);
        let group: NodeId = 1;
        let glass: NodeId = 2;
        let mut material = no_material();
        material.pbr.transmission_weight = 1.0;
        vp.mats.materials.insert(group, material);
        vp.mats.members.insert(group, vec![(glass, None)]);
        let item = |shape: NodeId| DrawItem { shape, xf: 9, world: DMat4::IDENTITY, mesh: Arc::new(Mesh::default()), smooth: 0 };
        let skip = vp.transmissive_shapes(&[item(glass), item(3)]);
        assert!(skip.contains(&glass));
        assert_eq!(skip.len(), 1);
    }
}
