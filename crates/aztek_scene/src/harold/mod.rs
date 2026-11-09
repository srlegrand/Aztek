//! Harold, the renderer, from the scene: everything the viewport draws at the frame, translated into the renderer's
//! scene. Meshes are kept between translations by the identity of their evaluated mesh, so an interactive render after
//! a light or material edit reuses every mesh, and one after a deformation refits rather than rebuilds.

pub mod commands;
pub mod displace;
pub mod lights;
pub mod materials;
pub mod network;
pub mod settings;

use std::collections::HashMap;
use std::sync::Arc;

use aztek_geo::Mesh;
use aztek_render::geom::{vis, Instance, MeshData};
use aztek_render::scene::{Atmosphere, Scene};
use aztek_render::shader::{Input, Material};
use aztek_render::texture::TextureCache;
use glam::{Mat4, Vec2, Vec3};

use crate::graph::Graph;
use crate::mesh_nodes::MeshNode;
use crate::plug::{NodeId, NO_NODE};
use crate::session::Session;
use crate::surfaces::SurfaceShape;
use crate::transform::DagNode;
use network::{get_b, get_f, NetBuilder};

pub const FORMATS: &[&str] = &["exr", "png", "tif", "tiff", "jpg", "jpeg", "tga"];

/// What one render asks for beyond the scene's settings.
#[derive(Clone, Debug, Default)]
pub struct Request {
    pub camera: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    /// x0, y0, x1, y1 in pixels.
    pub region: Option<[u32; 4]>,
    /// Interactive: motion blur off unless asked, preview passes on.
    pub interactive: bool,
    /// AOVs to add to the scene's.
    pub aovs: Vec<String>,
}

/// One mesh as translated: the evaluated mesh it came from, and its subdivision, so the next translation can tell
/// whether it changed.
struct Cached {
    source: Arc<Mesh>,
    levels: u32,
    data: Arc<MeshData>,
    /// Per triangle, the cage face it came from.
    tri_faces: Vec<u32>,
}

/// What survives between translations: meshes and the texture cache.
#[derive(Default)]
pub struct Translator {
    meshes: HashMap<NodeId, Cached>,
    pub textures: Option<Arc<TextureCache>>,
    /// Statistics of the last translation.
    pub reused: usize,
    pub refit: usize,
    pub built: usize,
}

pub(crate) fn visible(g: &Graph, id: NodeId) -> bool {
    let mut n = id;
    while n != NO_NODE {
        let Some(node) = g.node(n) else { return false };
        if node.ty().dag && !node.bool(DagNode::VISIBILITY) {
            return false;
        }
        if crate::draw_style::hidden_by_override(g, n) {
            return false;
        }
        n = g.parent(n);
    }
    crate::draw_style::display_kind(g, id) != crate::draw_style::DisplayKind::Template
}

fn mat4(m: &glam::DMat4) -> Mat4 {
    m.as_mat4()
}

/// A shape's evaluated mesh and world matrix now.
fn shape_state(g: &Graph, id: NodeId) -> Option<(Arc<Mesh>, glam::DMat4, i32)> {
    let n = g.node(id)?;
    let world = n.data().matrix(DagNode::WORLD_MATRIX);
    if n.ty().is_a("nurbsSurface") {
        return Some((n.data().mesh(SurfaceShape::OUT_MESH).cloned()?, world, 0));
    }
    Some((n.data().mesh(MeshNode::OUT_MESH).cloned()?, world, MeshNode::render_level(n) as i32))
}

/// Shapes to render: visible, not intermediate, not templated meshes and NURBS surfaces.
pub fn render_shapes(g: &mut Graph) -> Vec<NodeId> {
    g.evaluate();
    let mut out = Vec::new();
    for id in g.nodes() {
        let Some(n) = g.node(id) else { continue };
        let t = n.ty();
        if !(t.is_a("mesh") || t.is_a("nurbsSurface")) || n.bool(DagNode::INTERMEDIATE_OBJECT) {
            continue;
        }
        if visible(g, id) {
            out.push(id);
        }
    }
    out
}

/// Subdivision levels for a shape: Arnold's subdivision settings, or the smooth preview's.
fn subdiv_levels(g: &mut Graph, id: NodeId, smooth: i32, ignore: bool) -> u32 {
    if ignore {
        return 0;
    }
    let ty = get_f(g, id, "aiSubdivType", 0.0) as i64;
    if ty == 1 {
        return get_f(g, id, "aiSubdivIterations", 1.0).clamp(0.0, 6.0) as u32;
    }
    if smooth >= 1 {
        return smooth as u32;
    }
    0
}

fn build_mesh(name: &str, mesh: &Mesh, levels: u32) -> (MeshData, Vec<u32>) {
    let (mesh, faces): (std::borrow::Cow<Mesh>, Option<Vec<u32>>) = if levels > 0 {
        let mut map = aztek_geo::ops::subdivide::SubdivideMap::default();
        let opts = aztek_geo::ops::subdivide::SubdivideOptions { levels: levels as i32, sharp_boundary: true, keep_hard_edges: false };
        let m = aztek_geo::ops::subdivide::subdivide(mesh, &opts, Some(&mut map));
        (std::borrow::Cow::Owned(m), Some(map.faces))
    } else {
        (std::borrow::Cow::Borrowed(mesh), None)
    };
    let opts = aztek_geo::render_mesh::RenderMeshOptions { max_uv_sets: 1, tangents: false, colors: false, wireframe: false, sections: false, ..Default::default() };
    let rm = aztek_geo::render_mesh::build_render_mesh(&mesh, &opts);
    let tris: Vec<[u32; 3]> = rm.indices.chunks(3).map(|c| [c[0], c[1], c[2]]).collect();
    let uvs: Vec<Vec2> = rm.uvs.first().map(|u| u.iter().map(|x| Vec2::new(x[0], x[1])).collect()).unwrap_or_default();
    let tri_faces: Vec<u32> = rm.triangle_faces.iter().map(|&f| faces.as_ref().map_or(f, |m| m.get(f as usize).copied().unwrap_or(f))).collect();
    let n = tris.len();
    (MeshData::new(name, rm.positions.clone(), rm.normals.clone(), uvs, tris, vec![0; n]), tri_faces)
}

impl Translator {
    /// Keeps a mesh whose shading groups were filled in, so the next translation reuses it as it is.
    fn store_shading(&mut self, shape: NodeId, data: Arc<MeshData>) {
        if let Some(c) = self.meshes.get_mut(&shape) {
            c.data = data;
        }
    }

    /// The renderer's mesh for a shape, from the cache when its evaluated mesh is the same, refit when only points moved.
    fn mesh_for(&mut self, shape: NodeId, name: &str, mesh: &Arc<Mesh>, levels: u32) -> (Arc<MeshData>, Vec<u32>) {
        if let Some(c) = self.meshes.get(&shape) {
            if Arc::ptr_eq(&c.source, mesh) && c.levels == levels {
                self.reused += 1;
                return (c.data.clone(), c.tri_faces.clone());
            }
            // Same topology, points moved: a cheaper path when no subdivision is involved.
            if levels == 0 && c.levels == 0 && c.source.only_points_differ(mesh) {
                let (fresh, tri_faces) = build_mesh(name, mesh, 0);
                if fresh.tris == c.data.tris {
                    let mut d = MeshData { bvh: c.data.bvh.clone(), ..fresh };
                    d.refit();
                    let data = Arc::new(d);
                    self.refit += 1;
                    self.meshes.insert(shape, Cached { source: mesh.clone(), levels, data: data.clone(), tri_faces: tri_faces.clone() });
                    return (data, tri_faces);
                }
            }
        }
        let (d, tri_faces) = build_mesh(name, mesh, levels);
        let data = Arc::new(d);
        self.built += 1;
        self.meshes.insert(shape, Cached { source: mesh.clone(), levels, data: data.clone(), tri_faces: tri_faces.clone() });
        (data, tri_faces)
    }
}

/// The camera to render through: one named, else the first renderable non-default camera, else persp.
pub fn find_camera(g: &Graph, name: Option<&str>) -> Option<NodeId> {
    if let Some(n) = name {
        let id = g.find(n).ok().flatten()?;
        return crate::cameras::shape_of(g, id);
    }
    let cams: Vec<NodeId> = g.nodes().into_iter().filter(|&id| g.node(id).is_some_and(|n| n.ty().name == crate::cameras::TYPE)).collect();
    let renderable = |id: NodeId| g.node(id).and_then(|n| n.ty().attr_index("renderable").map(|a| n.value(a).scalar().unwrap_or(1.0) != 0.0)).unwrap_or(true);
    cams.iter().copied().find(|&c| renderable(c) && !crate::cameras::is_startup(g, c)).or_else(|| cams.iter().copied().find(|&c| renderable(c))).or_else(|| g.find("perspShape").ok().flatten())
}

fn camera_at(g: &mut Graph, shape: NodeId) -> aztek_render::camera::Camera {
    use aztek_render::camera::{Camera, FilmFit};
    let world = g.node(shape).map(|n| n.data().matrix(DagNode::WORLD_MATRIX)).unwrap_or_default();
    let f = |g: &mut Graph, a: &str, d: f64| get_f(g, shape, a, d) as f32;
    // Remove scale from the camera's matrix: a scaled camera still looks the same way.
    let (_, r, t) = world.to_scale_rotation_translation();
    let m = Mat4::from_rotation_translation(r.as_quat(), t.as_vec3());
    Camera {
        name: g.node(shape).map(|n| n.name().to_string()).unwrap_or_default(),
        xforms: vec![m],
        orthographic: get_b(g, shape, "orthographic", false),
        ortho_width: f(g, "orthographicWidth", 30.0),
        focal_mm: f(g, "focalLength", 35.0),
        h_aperture: f(g, "horizontalFilmAperture", 1.41732),
        v_aperture: f(g, "verticalFilmAperture", 0.94488),
        film_offset: Vec2::new(f(g, "horizontalFilmOffset", 0.0), f(g, "verticalFilmOffset", 0.0)),
        film_fit: FilmFit::from_index(get_f(g, shape, "filmFit", 1.0) as i64),
        lens_squeeze: f(g, "lensSqueezeRatio", 1.0),
        dof: get_b(g, shape, "depthOfField", false),
        f_stop: f(g, "fStop", 5.6),
        focus_distance: f(g, "focusDistance", 5.0) * f(g, "focusRegionScale", 1.0),
        units_per_cm: 1.0,
        near: 0.0,
    }
}

/// Visibility bits from Maya's render stats and Arnold's visibility settings.
fn visibility_of(g: &mut Graph, id: NodeId) -> u8 {
    let mut m = 0u8;
    let on = |g: &mut Graph, a: &str| get_b(g, id, a, true);
    if on(g, "primaryVisibility") {
        m |= vis::CAMERA;
    }
    if on(g, "castsShadows") {
        m |= vis::SHADOW;
    }
    if on(g, "visibleInReflections") {
        if on(g, "aiVisibleInDiffuseReflection") {
            m |= vis::DIFFUSE_REFLECT;
        }
        if on(g, "aiVisibleInSpecularReflection") {
            m |= vis::SPECULAR_REFLECT;
        }
    }
    if on(g, "visibleInRefractions") {
        if on(g, "aiVisibleInDiffuseTransmission") {
            m |= vis::DIFFUSE_TRANSMIT;
        }
        if on(g, "aiVisibleInSpecularTransmission") {
            m |= vis::SPECULAR_TRANSMIT;
        }
    }
    if on(g, "aiVisibleInVolume") {
        m |= vis::VOLUME;
    }
    m | vis::SUBSURFACE
}

/// The times, in frames, a render samples for motion blur: the frame alone without it.
fn motion_times(frame: f64, m: &settings::Motion) -> Vec<f64> {
    if !m.enabled {
        return vec![frame];
    }
    (0..m.steps).map(|k| frame + m.start + (m.end - m.start) * k as f64 / (m.steps - 1) as f64).collect()
}

/// Shading groups of a shape and their materials' ids.
fn shader_table(mb: &mut materials::MatBuilder, shape: NodeId) -> (Vec<u32>, Vec<(NodeId, Option<Vec<u32>>)>) {
    let groups = crate::shading::membership_of(mb.nb.g, shape);
    let mut ids = Vec::new();
    for (sg, _) in &groups {
        let mat = crate::shading::material_of_group(mb.nb.g, *sg);
        ids.push(match mat {
            Some(m) => mb.material(m),
            None => 0,
        });
    }
    (ids, groups)
}

/// Builds the renderer's scene from the session at its current frame.
pub fn translate(s: &mut Session, tr: &mut Translator, req: &Request) -> Result<(Scene, settings::Common), String> {
    let mut warnings = Vec::new();
    let (mut opts, common, mut motion) = settings::read(&mut s.graph, &mut warnings);
    if let Some(w) = req.width {
        opts.width = w.max(1);
    }
    if let Some(h) = req.height {
        opts.height = h.max(1);
    }
    opts.region = req.region;
    for a in &req.aovs {
        if !opts.aovs.contains(a) {
            opts.aovs.push(a.clone());
        }
    }
    if req.interactive {
        motion.enabled = false;
    }
    let scene_path = s.scene_path.clone();
    let project = crate::shading::project_dir(s);
    let textures = match &tr.textures {
        Some(t) => t.clone(),
        None => {
            let t = Arc::new(TextureCache::new(opts.texture_max_memory_mb));
            tr.textures = Some(t.clone());
            t
        }
    };
    let mut scene = Scene::new(opts.clone());
    scene.textures = textures;
    tr.reused = 0;
    tr.refit = 0;
    tr.built = 0;

    let g = &mut s.graph;
    let cam_shape = find_camera(g, req.camera.as_deref()).ok_or("no camera to render through")?;
    let fps = crate::anim::fps();
    let frame = g.time() * fps;
    let times = motion_times(frame, &motion);
    let shapes = render_shapes(g);

    // Per time, each shape's evaluated mesh and matrix, and the camera's.
    let mut states: Vec<HashMap<NodeId, (Arc<Mesh>, glam::DMat4, i32)>> = Vec::new();
    let mut cams: Vec<Mat4> = Vec::new();
    let original = g.time();
    for &t in &times {
        if times.len() > 1 {
            g.set_time(t / fps);
            g.evaluate();
        }
        states.push(shapes.iter().filter_map(|&id| shape_state(g, id).map(|st| (id, st))).collect());
        cams.push(camera_at(g, cam_shape).xforms[0]);
    }
    if times.len() > 1 {
        g.set_time(original);
        g.evaluate();
    }
    scene.camera = camera_at(g, cam_shape);
    if motion.enabled && motion.camera {
        scene.camera.xforms = cams;
    }

    let mut net = std::mem::take(&mut scene.network);
    let mut mats = std::mem::take(&mut scene.materials);
    let mut names = std::mem::take(&mut scene.material_names);
    {
        let mut nb = NetBuilder { g, net: &mut net, built: HashMap::new(), warnings: Vec::new(), scene_path: scene_path.clone(), project: project.clone(), ignore_textures: opts.ignore_textures, textures: scene.textures.clone() };
        let mut mb = materials::MatBuilder { nb: &mut nb, materials: &mut mats, names: &mut names, built: HashMap::new() };
        let mid = times.len() / 2;
        let mut instance_shapes: Vec<NodeId> = Vec::new();
        let ignore_subdiv = mb.nb.g.find(settings::OPTIONS).ok().flatten().is_some_and(|o| get_b(mb.nb.g, o, "ignoreSubdivision", false));
        let ignore_disp = mb.nb.g.find(settings::OPTIONS).ok().flatten().is_some_and(|o| get_b(mb.nb.g, o, "ignoreDisplacement", false));
        for (k, &shape) in shapes.iter().enumerate() {
            let Some((mesh, world, smooth)) = states[mid].get(&shape).cloned() else { continue };
            if mesh.num_faces() == 0 {
                continue;
            }
            let name = mb.nb.g.path(shape);
            let levels = subdiv_levels(mb.nb.g, shape, smooth, ignore_subdiv);
            let (data, tri_faces) = tr.mesh_for(shape, &name, &mesh, levels);
            let (shader_ids, groups) = shader_table(&mut mb, shape);
            // Per triangle, which of the shape's shading groups: through the cage face.
            let face_group = crate::shading::face_groups(&mesh, &groups);
            let tri_shader: Vec<u16> = tri_faces.iter().map(|&f| face_group.get(f as usize).copied().filter(|&x| x != usize::MAX).unwrap_or(0) as u16).collect();
            // Deformation blur: the points at each time, when the topology holds.
            let mut keys = Vec::new();
            if times.len() > 1 && motion.deform {
                for st in &states {
                    let Some((m, _, _)) = st.get(&shape) else { break };
                    if !Arc::ptr_eq(m, &mesh) && !m.only_points_differ(&mesh) {
                        break;
                    }
                    let pts = if Arc::ptr_eq(m, &mesh) { data.keys[0].clone() } else { build_mesh(&name, m, levels).0.keys[0].clone() };
                    if pts.len() != data.keys[0].len() {
                        break;
                    }
                    keys.push(pts);
                }
            }
            let moving = keys.len() == times.len() && keys.windows(2).any(|w| w[0] != w[1]);
            // Displacement, by shading group, unless switched off.
            let disps: Vec<Option<displace::Disp>> = if ignore_disp { Vec::new() } else { groups.iter().map(|(sg, _)| displace::of_group(mb.nb, *sg)).collect() };
            let displaced = disps.iter().any(Option::is_some);
            let data = if data.tri_shader == tri_shader && !moving && !displaced {
                data
            } else {
                let mut d = (*data).clone();
                d.tri_shader = tri_shader;
                if moving {
                    d.keys = keys;
                    d.normals.clear();
                    d.build();
                }
                if displaced {
                    let h = get_f(mb.nb.g, shape, "aiDispHeight", 1.0) as f32;
                    displace::displace(&mut d, &disps, h, mb.nb.net, &mb.nb.textures);
                }
                let d = Arc::new(d);
                if !moving && !displaced {
                    tr.store_shading(shape, d.clone());
                }
                d
            };
            let mesh_index = scene.geo.meshes.len() as u32;
            scene.geo.meshes.push(data);
            let mut inst = Instance::new(&name, mesh_index, mat4(&world), if shader_ids.is_empty() { vec![0] } else { shader_ids.clone() });
            if times.len() > 1 {
                let xs: Vec<Mat4> = states.iter().map(|st| st.get(&shape).map_or(mat4(&world), |x| mat4(&x.1))).collect();
                if xs.windows(2).any(|w| w[0] != w[1]) {
                    inst.set_xforms(xs);
                }
            }
            let g = &mut *mb.nb.g;
            inst.visibility = visibility_of(g, shape);
            inst.matte = get_b(g, shape, "aiMatte", false);
            inst.receive_shadows = get_b(g, shape, "receiveShadows", true);
            inst.self_shadows = get_b(g, shape, "aiSelfShadows", true);
            inst.id = crate::harold::object_id(&name);
            let opaque_attr = get_b(g, shape, "aiOpaque", true);
            let all_opaque = inst.shaders.iter().all(|&m| material_opaque(&mats_view(&mb), m));
            inst.opaque = opaque_attr && all_opaque;
            let _ = k;
            scene.geo.instances.push(inst);
            instance_shapes.push(shape);
        }
        // Atmosphere: what the options node's atmosphere plug connects.
        let opts_node = mb.nb.g.find(settings::OPTIONS).ok().flatten();
        if let Some(o) = opts_node {
            if let Some(p) = network::attr(mb.nb.g, o, "atmosphere") {
                let src = (0..4).find_map(|k| mb.nb.g.source(&crate::plug::PlugRef::element(o, p.attr, k))).or_else(|| mb.nb.g.source(&p));
                if let Some(src) = src {
                    let a = src.node;
                    let g = &mut *mb.nb.g;
                    scene.atmosphere = Some(Atmosphere { density: get_f(g, a, "density", 0.0) as f32, color: network::get_v(g, a, "color", Vec3::ONE), anisotropy: get_f(g, a, "anisotropy", 0.0) as f32 });
                }
            }
        }
        // Lights after materials: mesh lights need their instances.
        let mut lw = Vec::new();
        let light_list = lights::translate(&mut mb, &mut scene, &mut lw);
        // Light linking: a light breaks off from the shapes the link table says it doesn't light.
        let table = crate::light_cmds::LinkTable::read(mb.nb.g);
        if !table.is_empty() {
            for (k, (_, light_shape)) in light_list.iter().enumerate().take(64) {
                for (inst, &shape) in scene.geo.instances.iter_mut().zip(&instance_shapes) {
                    if !table.lights(mb.nb.g, *light_shape, shape) {
                        inst.light_mask &= !(1u64 << k);
                    }
                }
            }
        }
        scene.lights = light_list.into_iter().map(|(l, _)| l).collect();
        warnings.extend(lw);
        warnings.extend(std::mem::take(&mut mb.nb.warnings));
    }
    scene.network = net;
    scene.materials = mats;
    scene.material_names = names;
    // A default light when there are none and the globals ask for one, as Maya's own default light. Glowing surfaces
    // don't count: they are lights only to the renderer.
    let no_lights = scene.lights.is_empty();
    lights::emissive(&mut scene);
    if no_lights && default_light_on(&mut s.graph) {
        let cam = scene.camera.xforms[0];
        scene.lights.extend(default_lights(&cam));
        warnings.push("no lights: rendered with the viewport's default lighting".into());
    }
    scene.prepare();
    scene.warnings = warnings;
    Ok((scene, common))
}

/// The viewport's default lighting, which follows the camera: a key and a fill each lighting both ways (the viewport
/// takes the absolute cosine), and a little all round. Unshadowed, as the viewport's.
pub fn default_lights(camera: &Mat4) -> Vec<aztek_render::lights::Light> {
    use aztek_render::lights::{Light, Shape};
    let pi = std::f32::consts::PI;
    let key = camera.transform_vector3(Vec3::new(-0.45, 0.6, 0.65).normalize()).normalize();
    let fill = camera.transform_vector3(Vec3::new(0.6, -0.2, 0.75).normalize()).normalize();
    let mut out = Vec::new();
    for (name, dir, k) in [("defaultKey", key, 0.95 * 0.75), ("defaultKeyBack", -key, 0.95 * 0.75), ("defaultFill", fill, 0.95 * 0.3), ("defaultFillBack", -fill, 0.95 * 0.3)] {
        let mut l = Light::new(name, Shape::Distant { dir, cos_half: 1.0 }, Vec3::splat(pi * k));
        l.cast_shadows = false;
        out.push(l);
    }
    let pos = camera.transform_point3(Vec3::ZERO);
    out.push(Light::new("defaultAmbient", Shape::Ambient { pos, shade: 0.0 }, Vec3::splat(pi * 0.12)));
    out
}

fn default_light_on(g: &mut Graph) -> bool {
    g.find(settings::GLOBALS).ok().flatten().is_none_or(|r| get_b(g, r, "enableDefaultLight", true))
}

fn mats_view<'a>(mb: &'a materials::MatBuilder) -> &'a [Material] {
    mb.materials
}

/// Whether a material is solid everywhere: then shadow rays need not shade it.
fn material_opaque(mats: &[Material], m: u32) -> bool {
    match mats.get(m as usize) {
        Some(Material::Standard(s)) => matches!(s.opacity, Input::Const(v) if v.min_element() >= 1.0),
        Some(Material::Flat { opacity, .. }) => matches!(opacity, Input::Const(v) if v.min_element() >= 1.0),
        Some(Material::Mix { a, b, .. }) => *a != m && *b != m && material_opaque(mats, *a) && material_opaque(mats, *b),
        Some(Material::ShadowMatte { .. }) => false,
        None => true,
    }
}

/// A stable id for an object, from its path.
pub fn object_id(path: &str) -> u32 {
    let mut h: u32 = 2166136261;
    for b in path.bytes() {
        h = (h ^ b as u32).wrapping_mul(16777619);
    }
    h
}
