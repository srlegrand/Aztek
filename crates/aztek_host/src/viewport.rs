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
        if !n.ty().is_a("mesh") || n.bool(DagNode::INTERMEDIATE_OBJECT) || !visible(g, id) {
            continue;
        }
        let Some(mesh) = n.data().mesh(MeshNode::OUT_MESH).cloned() else { continue };
        let world = n.data().matrix(DagNode::WORLD_MATRIX);
        let smooth = MeshNode::preview_level(n) as i32;
        out.push(DrawItem { shape: id, xf: aztek_scene::dag::transform_of(g, id), world, mesh, smooth });
    }
    out
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
    DrawMaterial { group: NO_NODE, material: NO_NODE, unlit: false, color: [0.0, 1.0, 0.0], ambient: [0.0; 3], incandescence: [0.0; 3], diffuse: 1.0, transparency: 0.0, color_map: None, alpha_map: None, specular: None }
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
    InsertJoint,
    MultiCut,
    InsertLoop,
    OffsetLoop,
    TargetWeld,
    SlideEdge,
    Crease,
    CreatePolygon,
    AppendPolygon,
    QuadDraw,
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
}

pub struct Viewport {
    /// Lights and highlights for the material shader, made at the first frame.
    pub lit: Option<crate::lighting::ViewLights>,
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
    /// Multi component mode: a pick takes a vertex, edge or face, whichever is under the mouse.
    pub multi: bool,
    /// Keep Spacing: with a grid snap, the selection moves as one; off, each point lands on the grid itself.
    pub keep_spacing: bool,
    /// Snap Pivot Position: in pivot mode the pivot lands on points near the cursor.
    pub snap_pivot: bool,
    /// Use Manipulator Pivot: objects turn and scale about the manipulator rather than each about its own.
    pub use_manip_pivot: bool,
    /// J held: rotate in discrete steps.
    pub discrete_hold: bool,
    pub disp: Display,
    /// Where a live-surface move lands, worked out before the drag's call.
    pub live_hit: Option<DVec3>,
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
    /// The up axis the grid was built for: it lies on the ground, so it is rebuilt when the axis changes.
    grid_up_z: bool,
    pub wireframe: bool,
    pub wire_on_shaded: bool,
    pub tool: Tool,
    pub drag: Option<Drag>,
    pub hover: Option<Handle>,
    /// Viewport rect in physical pixels: x, y, w, h.
    pub rect: [f64; 4],
    pub marquee: Option<(DVec2, DVec2)>,
    pub grid_spacing: f64,
    /// How far the grid reaches each way from the origin.
    pub grid_extent: f64,
    pub show_grid: bool,
    pub manip_scale: f64,
    /// Smooth preview draws the cage too (the 2 key) rather than the smooth mesh alone (3).
    pub smooth_cage: bool,
    /// The component under the cursor: (shape, kind, id).
    pub hover_comp: Option<(NodeId, ComponentMode, Index)>,
    /// The paint brush under the cursor: world centre and normal.
    pub brush: Option<(DVec3, DVec3)>,
    /// A curve tool's points so far, and its degree.
    pub curve_points: Vec<DVec3>,
    pub curve_degree: i64,
    /// Manipulator axis orientation: world, object, component, parent or normal.
    pub axis_mode: String,
    /// The manipulator's axes as columns.
    pub frame: DMat3,
    /// What the modelling tools draw over the scene, world space.
    pub tool_lines: Vec<(DVec3, DVec3, [u8; 4])>,
    pub tool_points: Vec<(DVec3, [u8; 4])>,
    /// The handle a script made active (the contexts' -ah): what a middle drag takes when none is under the mouse.
    pub active_handle: Option<Handle>,
    /// What CreateRound makes: polySphere, polyCylinder, polyCone or polyTorus.
    pub round_kind: String,
    /// The Show Manipulator tool's lines, world space.
    pub node_lines: Vec<(DVec3, DVec3, [u8; 4])>,
}

const WIRE: [u8; 4] = [10, 10, 30, 255];
const VERT: [u8; 4] = [170, 30, 170, 255];
const SEL_COMP: [u8; 4] = [255, 160, 30, 255];
/// The mirror image of what was picked, with symmetry on.
const SEL_MIRROR: [u8; 4] = [80, 190, 255, 255];
const UNSEL_EDGE: [u8; 4] = [40, 70, 200, 255];

impl Viewport {
    pub fn new(kind: CamKind) -> Viewport {
        Viewport {
            lit: None,
            cam: Camera::new(kind),
            meshes: HashMap::new(),
            mats: Default::default(),
            grid: None,
            grid_up_z: false,
            wireframe: false,
            wire_on_shaded: false,
            tool: Tool::Select,
            drag: None,
            hover: None,
            rect: [0.0, 0.0, 1.0, 1.0],
            marquee: None,
            grid_spacing: 1.0,
            grid_extent: 12.0,
            snap: 0,
            constraint: None,
            step_move: 0.0,
            step_rotate: 0.0,
            step_scale: 0.0,
            discrete_hold: false,
            disp: Display::default(),
            live_hit: None,
            lasso: Vec::new(),
            select_brush: None,
            iso: Default::default(),
            pivot_mode: false,
            pivot_rings: false,
            show_grid: true,
            manip_scale: 1.0,
            smooth_cage: false,
            hover_comp: None,
            brush: None,
            curve_points: Vec::new(),
            curve_degree: 3,
            axis_mode: "world".into(),
            frame: DMat3::IDENTITY,
            tool_lines: Vec::new(),
            tool_points: Vec::new(),
            active_handle: None,
            node_lines: Vec::new(),
            round_kind: String::new(),
            snap_relative: true,
            tweak: false,
            preserve_children: false,
            prevent_negative: true,
            custom_frame: DMat3::IDENTITY,
            multi: false,
            keep_spacing: true,
            snap_pivot: false,
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

    /// The reference's grid: 12 units each way, a line per unit, darker every 5, black axes; on the ground (XZ, or XY when
    /// Z is up), and in the view's own plane for the orthographic cameras.
    fn grid_lines(&self) -> Vec<LineVertex> {
        let n = ((self.grid_extent / self.grid_spacing).round() as i32).clamp(1, 200);
        let s = self.grid_spacing as f32;
        let ext = n as f32 * s;
        // Maps (u, v) in the grid plane to world, through the Y up frame.
        let z_up = aztek_scene::world::up_z();
        let to = |u: f32, v: f32| -> [f32; 3] {
            let p = match self.cam.kind {
                CamKind::Front => [u, v, 0.0],
                CamKind::Side => [0.0, v, -u],
                _ => [u, 0.0, v],
            };
            if z_up {
                [p[0], -p[2], p[1]]
            } else {
                p
            }
        };
        let mut v = Vec::new();
        // Each line in unit pieces: lines reaching far behind the camera clip badly on some drivers.
        for i in -n..=n {
            let t = i as f32 * s;
            let c = if i == 0 { [0, 0, 0, 255] } else if i % 5 == 0 { [70, 70, 70, 255] } else { [105, 105, 105, 255] };
            for k in -n..n {
                let (a, b) = (k as f32 * s, (k + 1) as f32 * s);
                v.push(LineVertex { pos: to(t, a), color: c8(c) });
                v.push(LineVertex { pos: to(t, b), color: c8(c) });
                v.push(LineVertex { pos: to(a, t), color: c8(c) });
                v.push(LineVertex { pos: to(b, t), color: c8(c) });
            }
        }
        let _ = ext;
        v
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
    fn prepare_materials(&mut self, gpu: &Gpu, s: &mut Session) {
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
                let color = m.color_map.as_ref().and_then(|t| t.resolved.as_ref().and_then(|p| c.get(gpu, p, t.srgb)).map(|l| (t.file, l)));
                let alpha = m.alpha_map.as_ref().and_then(|t| t.resolved.as_ref().and_then(|p| c.get(gpu, p, t.srgb)).map(|l| (t.file, l)));
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

    /// This frame's lights, highlights and links, as the view's Lighting menu asks.
    fn prepare_lights(&mut self, gpu: &Gpu, r: &Renderer, s: &mut Session, items: &[DrawItem]) {
        if !r.materials {
            return;
        }
        let lit = self.lit.get_or_insert_with(|| crate::lighting::ViewLights::new(gpu, r));
        lit.write_speculars(self.mats.version, self.mats.materials.iter());
        let mode = self.disp.light_mode;
        let chosen = crate::lighting::chosen(&mut s.graph, mode, visible);
        let (view, proj) = (self.cam.view(), self.cam.proj(self.rect[2].max(1.0) / self.rect[3].max(1.0)));
        lit.write_block(mode, &chosen, &view, &proj, self.disp.two_sided, self.cam.orthographic);
        if !chosen.is_empty() {
            let used: Vec<NodeId> = chosen.iter().map(|l| l.shape).collect();
            lit.update_masks(&s.graph, &used, items.iter().map(|i| i.shape));
        }
    }

    pub fn draw(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session, items: &[DrawItem], scale: f64) {
        self.prune(gpu, items);
        self.prepare_materials(gpu, s);
        self.prepare_lights(gpu, r, s, items);
        let [x, y, w, h] = self.rect;
        if w < 2.0 || h < 2.0 {
            return;
        }
        r.set_viewport(gpu, x as f32, y as f32, w as f32, h as f32);
        let vp = self.view_proj();
        let view = self.cam.view();
        if self.grid.is_some() && self.grid_up_z != aztek_scene::world::up_z() {
            self.reset_grid(gpu);
        }
        if self.grid.is_none() {
            self.grid_up_z = aztek_scene::world::up_z();
            let lines = self.grid_lines();
            self.grid = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &lines), lines.len() as u32));
        }
        if self.show_grid {
            r.bind(gpu, Kind::Lines);
            r.set_line_width(gpu, 1.0);
            r.push(gpu, &push(vp, DMat4::IDENTITY, view, [1.0; 4], 0.0, 1.0));
            let (gb, gn) = self.grid.as_ref().unwrap();
            r.draw(gpu, gb, None, *gn);
        }

        let g = &s.graph;
        let mode = components::mode(g);
        let sel = g.selection().to_vec();
        // The UV Editor's Checker: on the meshes whose UVs it shows, the selected ones.
        let checker_set = (r.materials && crate::ui_cmds::pref_bool("uvChecker", false)).then(|| crate::textures::with_cache(gpu, |c| c.checker(gpu)));
        let lead = sel.last().copied();
        let mut transparent: Vec<Clear> = Vec::new();
        for it in items {
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
            let hilited = selected && mode != ComponentMode::Object;
            let checkered = !is_surface && (sel.contains(&it.xf) || sel.contains(&it.shape));
            let child_lit = !is_surface && self.disp.highlight && ds::select_state(g, it.shape, &sel) == ds::SelectState::Child;
            let wire_sel: &[NodeId] = if self.disp.highlight { &sel } else { &[] };
            let wire = ds::wire_color(g, it.shape, wire_sel, "polymesh", WIRE);
            let comps: &[u32] = if hilited { components::selected(g, it.shape, mode) } else { &[] };
            let comp_set: std::collections::HashSet<u32> = comps.iter().copied().collect();
            let soft = aztek_scene::soft_select::state();
            let global_seeds = soft.0 && mode != ComponentMode::Object && aztek_scene::soft_select::falloff_name() == "global";
            let soft_on = soft.0 && ((hilited && !comps.is_empty()) || (global_seeds && mode != ComponentMode::Object));
            let mut h = DefaultHasher::new();
            (mode as i32, selected, lead == Some(it.xf), self.wireframe, self.wire_on_shaded, comps, self.smooth_cage, entry.smooth, templated).hash(&mut h);
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
                let cage = entry.display.is_none() || self.smooth_cage || hilited;
                let wire_mesh = if cage { it.mesh.clone() } else { entry.display.clone().unwrap() };
                let m = &wire_mesh;
                let mirror_set: std::collections::HashSet<u32> = if hilited { components::mirrored_side(g, it.shape, mode, comps).into_iter().collect() } else { Default::default() };
                let edge_color = |e: Index| -> [u8; 4] {
                    if hilited {
                        if mode == ComponentMode::Edge && comp_set.contains(&e) {
                            if mirror_set.contains(&e) { SEL_MIRROR } else { SEL_COMP }
                        } else {
                            UNSEL_EDGE
                        }
                    } else {
                        wire
                    }
                };
                let want_lines = !is_surface && (selected || child_lit || !style.shaded || self.wireframe || self.wire_on_shaded || (entry.display.is_some() && self.smooth_cage));
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
                if soft_on {
                    // Soft selection's falloff, the reference's colours: yellow at the selection through red to black.
                    let pts = components::convert(&it.mesh, mode, comps, ComponentMode::Vertex, false);
                    let w = aztek_scene::mesh_edit::transform::soft_weights(g, &it.mesh, &it.world, &pts);
                    let mut pv = Vec::new();
                    for p in 0..it.mesh.point_capacity() {
                        if !it.mesh.point_valid(p) {
                            continue;
                        }
                        let x = w.get(p as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0);
                        let c = if x >= 0.999 {
                            [255, 255, 0, 255]
                        } else if x <= 0.0 {
                            if mode == ComponentMode::Vertex && hilited { [10, 10, 10, 255] } else { continue }
                        } else if x > 0.5 {
                            [255, (255.0 * (x - 0.5) * 2.0) as u8, 0, 255]
                        } else {
                            [(80.0 + 175.0 * x * 2.0) as u8, 0, 0, 255]
                        };
                        pv.push(LineVertex { pos: it.mesh.position(p).as_vec3().to_array(), color: c8(c) });
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                } else if hilited && mode == ComponentMode::Vertex {
                    let mut pv = Vec::new();
                    for p in 0..m.point_capacity() {
                        if m.point_valid(p) {
                            let c = if mirror_set.contains(&p) { SEL_MIRROR } else if comp_set.contains(&p) { SEL_COMP } else { VERT };
                            pv.push(LineVertex { pos: m.position(p).as_vec3().to_array(), color: c8(c) });
                        }
                    }
                    if !pv.is_empty() {
                        entry.points = Some((gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &pv), pv.len() as u32));
                    }
                }
                if hilited && mode == ComponentMode::Face && !comps.is_empty() {
                    let mut ti = Vec::new();
                    let mut mi = Vec::new();
                    for (t, &f) in entry.tri_faces.iter().enumerate() {
                        if comp_set.contains(&f) {
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
            let mvp = vp * it.world;
            // Paint skin weights: the painted influence's weight in grey, white for 1, as the reference's colour feedback.
            let mut painted = false;
            // The values in grey, white for 1, as the reference's colour feedback; a held influence reads blue.
            let feedback: Option<(Vec<f64>, u64, bool)> = if !selected || self.wireframe || entry.display.is_some() {
                None
            } else if self.tool == Tool::PaintSkin && s.paint_ctx.color_feedback {
                aztek_scene::skin::skin_of(g, it.shape).and_then(|sk| {
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
                (entry.mesh_gen, slot_key, held).hash(&mut h);
                let wk = h.finish();
                if wk != entry.weight_key || entry.weight_tris.is_none() {
                    entry.weight_key = wk;
                    if let Some((a, b, _)) = entry.weight_tris.take() {
                        gpu.drop_buffer(a);
                        gpu.drop_buffer(b);
                    }
                    let verts: Vec<LineVertex> = entry
                        .vertex_point
                        .iter()
                        .map(|&pt| {
                            let x = w.get(pt as usize).copied().unwrap_or(0.0).clamp(0.0, 1.0) as f32;
                            LineVertex { pos: it.mesh.position(pt).as_vec3().to_array(), color: if held { [x * 0.55, x * 0.7, 0.35 + x * 0.65, 1.0] } else { [x, x, x, 1.0] } }
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
                    r.push(gpu, &push(mvp, it.world, view, [1.0; 4], 0.0, 1.0));
                    r.draw(gpu, vb, Some((ib, *n)), 0);
                    painted = true;
                }
            }
            // Quad Draw's mesh sits on the live surface, chords and all, so it's pulled forward to stay in sight.
            let retopo = if self.tool == Tool::QuadDraw && !s.live_surface.contains(&it.shape) && !s.live_surface.is_empty() { 0.03 } else { 0.0 };
            // Bounding box display: twelve lines and nothing else.
            if self.disp.bounding_box || style.bounding_box || crate::ui_cmds::node_flag("boundingBox", it.xf) {
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
                r.push(gpu, &push(mvp, it.world, view, [1.0; 4], 1e-3, 1.0));
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
                    if self.disp.xray {
                        // X-Ray: see-through, no sorting.
                        r.bind(gpu, Kind::Tint);
                        let mut pc = push(mvp, it.world, view, [0.55, 0.55, 0.55, 0.45], retopo, 1.0);
                        pc.n2[3] = self.disp.flat as i32 as f32;
                        r.push(gpu, &pc);
                        r.draw(gpu, vb, Some((ib, *n)), 0);
                    } else if self.disp.default_material && (self.disp.light_mode == crate::lighting::LightMode::Default || !r.materials) && !(checker_set.is_some() && checkered) {
                        r.bind(gpu, if self.disp.backface { Kind::ShadedCull } else { Kind::Shaded });
                        let mut pc = push(mvp, it.world, view, [0.5, 0.5, 0.5, 1.0], retopo, 1.0);
                        pc.n2[3] = self.disp.flat as i32 as f32;
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
                        let depth = (view * it.world.w_axis).z;
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
                            let mut pc = push(mvp, it.world, view, [m.color[0], m.color[1], m.color[2], 1.0], retopo, 1.0);
                            pc.n2[3] = self.disp.flat as i32 as f32;
                            let (mut mp, clear) = material_push(pc, m, if checker_sets.is_some() { checker_sets.as_ref() } else if plain { None } else { self.mats.sets.get(&b.group).filter(|_| style.textured) });
                            stamp_lights(&self.lit, &mut mp, it.shape, if plain { NO_NODE } else { b.group });
                            // The isolated draws are for this frame only, so see-through ones go now rather than later.
                            if clear && shown_batches.is_none() && r.materials {
                                transparent.push(Clear { shape: it.shape, world: it.world, batch: bi, depth, retopo });
                                continue;
                            }
                            let (Some(ib), Some(uvs)) = (draw_ib, entry.uvs.as_ref()) else { continue };
                            if r.materials {
                                let sets = if checker_sets.is_some() { checker_sets.as_ref() } else if plain { None } else { self.mats.sets.get(&b.group).filter(|_| style.textured) };
                                let white = crate::textures::with_cache(gpu, |c| c.white(gpu));
                                let kind = match (clear, self.disp.backface) {
                                    (false, false) => Kind::Material,
                                    (false, true) => Kind::MaterialCull,
                                    (true, false) => Kind::MaterialBlend,
                                    (true, true) => Kind::MaterialBlendCull,
                                };
                                r.bind_material(gpu, kind, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), self.lit.as_ref().map_or(vk::DescriptorSet::null(), |l| l.set));
                                r.push_material(gpu, &mp);
                                r.draw_material(gpu, vb, uvs, ib, b.first, b.count);
                            } else {
                                // A device without room for the material constants: flat colours.
                                r.bind(gpu, if clear { Kind::Tint } else if self.disp.backface { Kind::ShadedCull } else { Kind::Shaded });
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
                    if let Some((ti, tn)) = &entry.mirror_tint {
                        r.bind(gpu, Kind::Tint);
                        r.push(gpu, &push(mvp, it.world, view, [0.2, 0.6, 1.0, 0.45], 2e-4, 1.0));
                        r.draw(gpu, vb, Some((ti, *tn)), 0);
                    }
                    if let Some((ti, tn)) = &entry.tint {
                        r.bind(gpu, Kind::Tint);
                        r.push(gpu, &push(mvp, it.world, view, [1.0, 0.45, 0.05, 0.45], 2e-4, 1.0));
                        r.draw(gpu, vb, Some((ti, *tn)), 0);
                    }
                }
            }
            if let Some((b, _)) = partial {
                gpu.drop_buffer(b);
            }
            if let Some((lb, ln)) = &entry.lines {
                r.bind(gpu, if self.disp.xray_active && hilited { Kind::LinesOnTop } else { Kind::Lines });
                r.set_line_width(gpu, if selected && mode == ComponentMode::Object { 1.5 } else { 1.0 });
                r.push(gpu, &push(mvp, it.world, view, [1.0; 4], 1.5e-3 + retopo, 1.0));
                r.draw(gpu, lb, None, *ln);
            }
            if let Some((pb, pn)) = &entry.points {
                r.bind(gpu, if self.disp.xray_active && hilited { Kind::PointsOnTop } else { Kind::Points });
                r.push(gpu, &push(mvp, it.world, view, [1.0; 4], 2e-3, (5.0 * scale) as f32));
                r.draw(gpu, pb, None, *pn);
            }
        }
        // See-through materials last, the furthest first, reading the depth the solid ones wrote.
        transparent.sort_by(|a, b| a.depth.total_cmp(&b.depth));
        for c in &transparent {
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
            r.bind_material(gpu, if self.disp.backface { Kind::MaterialBlendCull } else { Kind::MaterialBlend }, sets.and_then(|x| x.color).unwrap_or(white), sets.and_then(|x| x.alpha).unwrap_or(white), self.lit.as_ref().map_or(vk::DescriptorSet::null(), |l| l.set));
            r.push_material(gpu, &mp);
            r.draw_material(gpu, vb, uvs, ib, b.first, b.count);
        }
        crate::usd_view::draw(gpu, r, s, vp, view);
        self.draw_poly_display(gpu, r, s, items);
        self.draw_rig(gpu, r, s, scale);
        self.draw_hover(gpu, r, items, scale);
        self.draw_manip(gpu, r, s, scale);
        self.draw_node_lines(gpu, r);
        self.draw_brush(gpu, r, s);
        self.draw_curve_tool(gpu, r, scale);
        self.draw_tool_overlay(gpu, r, scale);
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
        let d = self.disp.clone();
        if !(d.border_edges || d.face_normals || d.vertex_normals || d.hard_edges || d.crease_edges) {
            return;
        }
        let sel = s.graph.selection().to_vec();
        for it in items {
            if d.limit_to_selected && !sel.contains(&it.xf) && !sel.contains(&it.shape) {
                continue;
            }
            let m = &it.mesh;
            let mut wide = Vec::new();
            let mut thin = Vec::new();
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
        }
    }

    /// Whether the selection masks and display type let a click select this node.
    pub fn pickable(&self, g: &Graph, id: NodeId) -> bool {
        if !ds::pickable(g, id) {
            return false;
        }
        crate::ui_cmds::mask_allows(type_category(g, id))
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

    /// A curve tool's points so far, with the curve they'd make.
    fn draw_curve_tool(&mut self, gpu: &mut Gpu, r: &Renderer, scale: f64) {
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
            let curve = if self.tool == Tool::CurveEp { aztek_geo::nurbs::NurbsCurve::through(d, &self.curve_points) } else { aztek_geo::nurbs::NurbsCurve::open(d, self.curve_points.clone()) };
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

    /// The brush's ring on the surface and its normal line, in red as the reference draws it.
    fn draw_brush(&mut self, gpu: &mut Gpu, r: &Renderer, s: &Session) {
        let Some((c, n)) = self.brush else { return };
        let radius = match self.tool {
            Tool::PaintSkin => s.paint_ctx.radius,
            Tool::PaintAttr => s.attr_ctx.paint.radius,
            Tool::Sculpt => s.sculpt_ctx.size,
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
        let picked: Vec<usize> = (0..items.len()).filter(|&i| (sel.contains(&items[i].xf) || sel.contains(&items[i].shape)) && (any || if self.tool == Tool::PaintAttr { aztek_scene::deform_paint::current(s, items[i].shape).is_some() } else { aztek_scene::skin::skin_of(g, items[i].shape).is_some() })).collect();
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
        let e = self.meshes.get_mut(&it.shape)?;
        if e.bvh.is_none() {
            e.bvh = Some(MeshBvh::new(&it.mesh));
        }
        e.bvh.as_ref()
    }

    /// The nearest mesh hit under a viewport pixel: (item index, face, world point).
    pub fn raycast(&mut self, items: &[DrawItem], px: DVec2) -> Option<(usize, Index, DVec3)> {
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
            if let Some(hit) = bvh.raycast_with(lo, ld / len, f64::MAX, true) {
                let wp = it.world.transform_point3(lo + ld / len * hit.t);
                let dist = (wp - o).length();
                if best.as_ref().is_none_or(|b| dist < b.3) {
                    best = Some((i, hit.face, wp, dist));
                }
            }
        }
        best.map(|b| (b.0, b.1, b.2))
    }

    /// What a click at px picks, as select arguments: object names or component strings.
    /// The component under a pixel on the hilited meshes: (item, kind, id).
    pub fn pick_component(&mut self, s: &Session, items: &[DrawItem], px: DVec2) -> Option<(usize, ComponentMode, Index)> {
        let g = &s.graph;
        let mode = components::mode(g);
        if mode == ComponentMode::Object {
            return None;
        }
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
        // Off the surface: the nearest within reach that no face hides.
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
                if !self.visible(&hil_items[k..=k], wp, &screen) {
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
    fn visible(&mut self, items: &[DrawItem], wp: DVec3, screen: &Screen) -> bool {
        let Some(q) = screen.project(wp) else { return false };
        let (o, _) = self.cam.ray(q, self.size());
        let dist = (wp - o).length();
        match self.raycast(items, q) {
            Some((_, _, hit)) => (hit - o).length() >= dist - dist.max(1.0) * 1e-3,
            None => true,
        }
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
        let t = n.dot(p0 - o) / den;
        let mut p = o + d * t;
        if self.snap == 1 {
            // X held: to the grid.
            p = (p / self.grid_spacing).round() * self.grid_spacing;
        }
        // Snap tiny noise so ortho placements sit exactly in their plane.
        for i in 0..3 {
            if p[i].abs() < 1e-9 {
                p[i] = 0.0;
            }
        }
        p
    }

    /// The joint, locator or IK handle drawn nearest px (within a few pixels), as its transform.
    pub fn pick_rig(&self, g: &Graph, px: DVec2) -> Option<NodeId> {
        let screen = self.screen();
        let mut best: Option<(f64, NodeId)> = None;
        for it in rig_items(g) {
            if !self.rig_shown(g, it.node) || !self.node_shown(g, it.node) || !self.pickable(g, it.shape) {
                continue;
            }
            for (a, b) in &it.segs {
                let (Some(pa), Some(pb)) = (screen.project(*a), screen.project(*b)) else { continue };
                let d = seg_distance(px, pa, pb);
                if d < 6.0 && best.is_none_or(|x| d < x.0) {
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
            for id in crate::ui_cmds::node_flag_nodes(flag) {
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
        if components::mode(g) == ComponentMode::Vertex {
            if let Some((shape, i)) = self.pick_cv(g, px) {
                return vec![cv_name(&g.path(aztek_scene::dag::transform_of(g, shape)), i, surfaces::id_stride(g, shape).map(|s| s as usize))];
            }
            if let Some(hit) = self.pick_lattice_point(g, px) {
                return vec![hit];
            }
        }
        match self.pick_component(s, items, px) {
            Some((i, kind, id)) => {
                let p = match kind {
                    ComponentMode::Vertex => "vtx",
                    ComponentMode::Edge => "e",
                    _ => "f",
                };
                vec![format!("{}.{p}[{id}]", g.path(items[i].xf))]
            }
            None => Vec::new(),
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
            _ => return,
        }
        let b = gpu.buffer_with(vk::BufferUsageFlags::VERTEX_BUFFER, &v);
        let mvp = self.view_proj() * it.world;
        if kind == ComponentMode::Vertex {
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
        self.pick_inside(s, items, &|p: DVec2| p.x >= lo.x && p.x <= hi.x && p.y >= lo.y && p.y <= hi.y)
    }

    /// Everything inside a free-form outline, in pixels.
    pub fn pick_lasso(&self, s: &Session, items: &[DrawItem], outline: &[DVec2]) -> Vec<String> {
        self.pick_inside(s, items, &|p: DVec2| {
            let mut odd = false;
            let n = outline.len();
            for i in 0..n {
                let (a, b) = (outline[i], outline[(i + 1) % n]);
                if (a.y > p.y) != (b.y > p.y) && p.x < a.x + (p.y - a.y) / (b.y - a.y) * (b.x - a.x) {
                    odd = !odd;
                }
            }
            odd
        })
    }

    /// Everything whose points the test takes in.
    pub fn pick_inside(&self, s: &Session, items: &[DrawItem], inside: &dyn Fn(DVec2) -> bool) -> Vec<String> {
        let g = &s.graph;
        let mode = components::mode(g);
        let screen = self.screen();
        let mut out = Vec::new();
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
                if it.segs.iter().any(|(a, b)| screen.project(*a).is_some_and(inside) || screen.project(*b).is_some_and(inside)) {
                    let path = g.path(crate::ui_cmds::scoped(g, it.node));
                    if !out.contains(&path) {
                        out.push(path);
                    }
                }
            }
        }
        if mode == ComponentMode::Vertex {
            for (shape, xf, cvs, stride) in curve_cvs(g) {
                if !g.selection().contains(&xf) && !g.selection().contains(&shape) {
                    continue;
                }
                for (i, p) in cvs.iter().enumerate() {
                    if screen.project(*p).is_some_and(inside) {
                        out.push(cv_name(&g.path(xf), i, stride));
                    }
                }
            }
        }
        for it in items {
            let m = &it.mesh;
            let proj = |p: Index| screen.project(it.world.transform_point3(m.position(p)));
            match mode {
                ComponentMode::Object => {
                    if self.pickable(g, it.shape) && (0..m.point_capacity()).any(|p| m.point_valid(p) && proj(p).is_some_and(inside)) {
                        let path = g.path(crate::ui_cmds::scoped(g, it.xf));
                        if !out.contains(&path) {
                            out.push(path);
                        }
                    }
                }
                _ if !g.selection().contains(&it.xf) || surfaces::is_surface(g, it.shape) => {}
                ComponentMode::Vertex => {
                    let ids: Vec<Index> = (0..m.point_capacity()).filter(|&p| m.point_valid(p) && proj(p).is_some_and(inside)).collect();
                    push_ranges(&mut out, &g.path(it.xf), "vtx", &ids);
                }
                ComponentMode::Edge => {
                    let ids: Vec<Index> = (0..m.edge_capacity())
                        .filter(|&e| {
                            m.edge_valid(e) && {
                                let (a, b) = (m.edge_point(e, 0), m.edge_point(e, 1));
                                screen.project(it.world.transform_point3((m.position(a) + m.position(b)) * 0.5)).is_some_and(inside)
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
                                screen.project(it.world.transform_point3(c)).is_some_and(inside)
                            }
                        })
                        .collect();
                    push_ranges(&mut out, &g.path(it.xf), "f", &ids);
                }
            }
        }
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
                for &i in components::selected(g, shape, mode) {
                    if let Some(p) = cvs.get(i as usize) {
                        sum += *p;
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
        if self.axis_mode == "world" {
            return;
        }
        if self.axis_mode == "custom" {
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
        match self.axis_mode.as_str() {
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

    fn draw_manip(&mut self, gpu: &mut Gpu, r: &Renderer, s: &mut Session, scale: f64) {
        let items = gather(&mut s.graph);
        let Some(pivot) = self.manip_pivot(s, &items) else { return };
        let len = self.handle_len(pivot, scale) as f32;
        let p = pivot.as_vec3();
        let colors = [[230, 40, 40, 255], [40, 230, 40, 255], [40, 80, 255, 255]];
        let active = self.drag.as_ref().map(|d| d.handle).or(self.hover);
        let constraint = self.constraint;
        // Handles lit: the hovered or dragged one, and the set constraint in yellow.
        let tint = |h: Handle, c: [u8; 4]| if active == Some(h) || constraint == Some(h) { [255, 255, 0, 255] } else { c };
        let mut v = Vec::new();
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
                let ring = |u: Vec3, w: Vec3, r: f32, c: [u8; 4], line: &mut dyn FnMut(Vec3, Vec3, [u8; 4])| {
                    let n = 64;
                    for k in 0..n {
                        let a0 = k as f32 / n as f32 * std::f32::consts::TAU;
                        let a1 = (k + 1) as f32 / n as f32 * std::f32::consts::TAU;
                        line(p + (u * a0.cos() + w * a0.sin()) * r, p + (u * a1.cos() + w * a1.sin()) * r, c);
                    }
                };
                for (i, ax) in axes.iter().enumerate() {
                    let (u, w) = ax.any_orthonormal_pair();
                    ring(u, w, len, tint(Handle::Axis(i), colors[i]), &mut line);
                }
                // The outer ring turns about the view axis; the sphere's outline shows the trackball.
                ring(right, up, len * 1.25, tint(Handle::ViewRing, [90, 200, 230, 255]), &mut line);
                ring(right, up, len, tint(Handle::Trackball, [140, 140, 140, 255]), &mut line);
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
                        // A small box at the tip.
                        let q = [tip + (u + w) * k, tip + (w - u) * k, tip - (u + w) * k, tip + (u - w) * k];
                        for j in 0..4 {
                            line(q[j], q[(j + 1) % 4], c);
                        }
                    } else {
                        let base = tip - *ax * len * 0.2;
                        for d in [u, -u, w, -w] {
                            line(tip, base + d * k, c);
                        }
                    }
                }
                // Plane handles: a small square between each pair of axes, coloured for the axis it locks.
                for i in 0..3 {
                    let (a, b) = (axes[(i + 1) % 3], axes[(i + 2) % 3]);
                    let c = tint(Handle::Plane(i), colors[i]);
                    let (l0, l1) = (len * 0.22, len * 0.38);
                    let q = [p + a * l0 + b * l0, p + a * l1 + b * l0, p + a * l1 + b * l1, p + a * l0 + b * l1];
                    for j in 0..4 {
                        line(q[j], q[(j + 1) % 4], c);
                    }
                }
                let c = tint(Handle::Center, [230, 230, 120, 255]);
                let k = len * 0.1;
                let corners = [p + (right + up) * k, p + (up - right) * k, p - (right + up) * k, p + (right - up) * k];
                for i in 0..4 {
                    line(corners[i], corners[(i + 1) % 4], c);
                }
            }
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
        let screen = self.screen();
        let len = self.handle_len(pivot, scale);
        let c = screen.project(pivot)?;
        let rotate = self.eff_tool() == Tool::Rotate;
        if !rotate && (px - c).abs().max_element() < 10.0 * scale {
            return Some(Handle::Center);
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
            for i in 0..3 {
                let (a, b) = (self.frame.col((i + 1) % 3), self.frame.col((i + 2) % 3));
                let mid = pivot + (a + b) * len * 0.3;
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
                let (u, w) = ax.any_orthonormal_pair();
                (0..48)
                    .filter_map(|k| {
                        let a = k as f64 / 48.0 * std::f64::consts::TAU;
                        let b = (k + 1) as f64 / 48.0 * std::f64::consts::TAU;
                        let (pa, pb) = screen.project_segment(pivot + (u * a.cos() + w * a.sin()) * len, pivot + (u * b.cos() + w * b.sin()) * len)?;
                        Some(seg_dist(px, pa, pb))
                    })
                    .fold(f64::MAX, f64::min)
            } else {
                let Some((pa, pb)) = screen.project_segment(pivot, pivot + ax * len) else { continue };
                seg_dist(px, pa, pb)
            };
            if d < 8.0 * scale && best.is_none_or(|b| d < b.0) {
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
                if d < 8.0 * scale && ring.is_none_or(|b| d < b.0) {
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
        let snap = if self.pivot_mode && self.snap_pivot && self.snap == 0 { 2 } else { self.snap };
        let target = match snap {
            1 => {
                let s = self.grid_spacing;
                let t = d.pivot + world;
                (t / s).round() * s
            }
            2 => {
                let screen = self.screen();
                let mut best: Option<(f64, DVec3)> = None;
                for &p in &d.snap_points {
                    if let Some(q) = screen.project(p) {
                        let dist = (q - px).length();
                        if dist < 25.0 && best.is_none_or(|b| dist < b.0) {
                            best = Some((dist, p));
                        }
                    }
                }
                match best {
                    Some((_, p)) => p,
                    None => return world,
                }
            }
            3 => {
                let screen = self.screen();
                let mut best: Option<(f64, DVec3)> = None;
                for &(a, b) in &d.snap_curves {
                    let Some((pa, pb)) = screen.project_segment(a, b) else { continue };
                    let ab = pb - pa;
                    let t = if ab.length_squared() < 1e-9 { 0.0 } else { ((px - pa).dot(ab) / ab.length_squared()).clamp(0.0, 1.0) };
                    let dist = (pa + ab * t - px).length();
                    if dist < 40.0 && best.is_none_or(|x| dist < x.0) {
                        best = Some((dist, a.lerp(b, t)));
                    }
                }
                match best {
                    Some((_, p)) => p,
                    None => return world,
                }
            }
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
                    ("moveComponents".into(), serde_json::json!({ "by": raw.to_array(), "snapEach": self.grid_spacing }))
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
                let by: [f64; 3] = match d.handle {
                    Handle::Axis(i) => {
                        let ax = self.frame.col(i);
                        let ang = snap(sweep(ax));
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
                    let mut call = serde_json::json!({ "args": targets, "by": by, "pivot": d.pivot.to_array() });
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
                if step > 0.0 {
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
    }

    pub fn release(&mut self, gpu: &mut Gpu) {
        for (_, m) in self.meshes.drain() {
            drop_mesh(gpu, m);
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

fn push_ranges(out: &mut Vec<String>, path: &str, p: &str, ids: &[Index]) {
    if ids.is_empty() {
        return;
    }
    let kind = match p {
        "vtx" => ComponentMode::Vertex,
        "e" => ComponentMode::Edge,
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
    RigItem { node, shape, color: st.color.unwrap_or_else(|| ds::dormant_color(kind, factory)), segs }
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
                }
            }
            out.push(rig_item(g, id, id, "joint", [70, 110, 220, 255], segs));
        } else if ty.is_a("deformHandle") {
            let kind = ty.attr_index("deformerType").map(|i| n.data().string(i).to_string()).unwrap_or_default();
            // A cluster's handle sits on its pivot.
            let at = w * glam::DMat4::from_translation(n.data().vec3(Transform::ROTATE_PIVOT));
            out.push(rig_item(g, id, id, "cluster", [200, 120, 230, 255], aztek_scene::deform::handle_lines(&kind, &at)));
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
            // A box and a lens, for cameras that show: the default views' are hidden.
            let (x, y, z) = (w.x_axis.truncate().normalize_or_zero(), w.y_axis.truncate().normalize_or_zero(), w.z_axis.truncate().normalize_or_zero());
            let k = 0.5;
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
                    out.push(RigItem { node: aztek_scene::NO_NODE, shape: aztek_scene::NO_NODE, color: [255, 200, 60, 255], segs });
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
    if ty.is_a("deformHandle") || ty.is_a("softModHandle") || ty.is_a("lattice") || ty.is_a("baseLattice") {
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
